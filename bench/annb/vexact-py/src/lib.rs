//! Python bindings over the fast-scan index for ANN-Benchmarks, VIBE and `bench/suite`: open a directory written by
//! `rust/tools/fit.py` and answer k-NN queries (exact or approximate) and threshold queries one at a time, on one thread.
use numpy::{PyArray1, PyReadonlyArray1, PyReadonlyArray2};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use roaring::RoaringBitmap;
use std::collections::HashMap;
use vexact::index::Params;
use std::sync::atomic::Ordering::Relaxed;
use vexact::v2::adaptive::{measured_constants, Adaptive, AdaptiveInfo, PSI};
use vexact::v2::q::{disk_constants, knn2, knn2_fast, prep2, pseudo_queries, threshold2, to_bitmap2, Approx, Dial, KnnOut2, Opts2, Plan, PlanMode, Vis2};
use vexact::v2::{disk, open, rebuild, saved_options, BuildOptions, Full2, Index2, OpenOptions};

/// Corpus pseudo-queries: (row, unit query, its nearest other entities).
type Pseudo = Vec<(usize, Vec<f32>, Vec<u32>)>;

#[pyclass(module = "vexact_py")]
struct Index {
    _p: Params,
    ix: Index2,
    plan: Plan,
    /// The legacy approximate order (`spread`, by nprobe), kept as the comparison arm until the owner's benchmark has
    /// run.
    approx: Approx,
    /// Corpus pseudo-queries for the legacy arm's dial (`eps`), this many, made when a dial is first fitted.
    npseudo: usize,
    pseudo: std::sync::OnceLock<Pseudo>,
    dials: HashMap<usize, Dial>,
    vis: Option<Vis2>,
    pool: rayon::ThreadPool,
    /// The approximate queries (`v2::adaptive`), set up on first use with this machine's measured constants, or why
    /// not.
    adaptive: std::sync::OnceLock<Result<Adaptive, String>>,
    dir: String,
}

/// The query's length against the index's, and k at least 1 (k-NN), as errors to raise.
fn check_query(ix: &Index2, q: &[f32], k: Option<usize>) -> PyResult<()> {
    if q.len() != ix.n() {
        return Err(PyValueError::new_err(format!("the query has {} values, the index {}", q.len(), ix.n())));
    }
    if k == Some(0) {
        return Err(PyValueError::new_err("k must be at least 1"));
    }
    Ok(())
}

/// What an approximate query did, as a dict.
fn info_dict<'py>(py: Python<'py>, i: &AdaptiveInfo) -> PyResult<Bound<'py, pyo3::types::PyDict>> {
    let d = pyo3::types::PyDict::new(py);
    d.set_item("path", i.path)?;
    for (key, x) in [("visited", i.visited), ("direct", i.direct), ("index", i.index), ("groups", i.groups), ("spreads", i.spreads)] {
        d.set_item(key, x)?;
    }
    for (key, x) in [("fixed", i.fixed), ("cost", i.cost), ("scan_cost", i.scan_cost), ("exact_cost", i.exact_cost)] {
        d.set_item(key, x)?;
    }
    Ok(d)
}

#[pymethods]
impl Index {
    /// Open the index saved in the fitted directory `dir` (from `rust/tools/fit.py`) when it was built with these options
    /// (`v2::saved_options`) and is not stale (`v2::open`, which maps it and encodes nothing); else encode the fit and
    /// save the index there (`v2::rebuild`). Queries run on one thread, pinned to `core` when given.
    /// `plan`: "auto" (the planner: in memory, the support table and the rule in rows; nothing is timed), "index" (every
    /// cluster the support table leaves open on the index path), "flat" (decided from full vectors) or "scan".
    /// `dlv`: the deterministic levels' cumulative bytes (a build input, e.g. [16, 32]); None: the library's default
    /// (`BuildOptions::default()`, two levels at 16 and 32 B); [] (empty): the fit's six levels (4 / 8 / 12 / 16 / 24 /
    /// 32 B), the bindings' default before the benchmarks moved to the library's.
    /// One directory holds one index (`dir/encoded`): two `Index(dir)` with different `dlv` replace each other's saved
    /// index, each rebuilding it; give each variant a directory of its own (`cp -al` of the fit).
    /// `ddec`: the deterministic levels that decide in threshold queries, as a bit mask (None: all).
    ///
    /// Disk mode, for runs under a memory cap (as `rust/src/run2.rs` `--full-file` and its options): `full_file` writes
    /// the full vectors to a new file there (`Rows::to_file`: never inside a saved index's `encoded-*` directory, never
    /// over the fit's vectors or a file this process maps) and maps it with `advice` (normal, random, sequential),
    /// evicted from the page cache. `read` "uring" reads the rows the index path leaves with pread through io_uring (and
    /// the planner prices those reads: its disk constants are read from the index directory's cache or timed once,
    /// `disk_constants`), "mmap" through the mapping (not priced). `willneed` asks the kernel to start reading a batch's
    /// pages before it is decided; `direct_min` (bytes; None: never) reads extents at least that large with O_DIRECT;
    /// `dontcache_min` (bytes; None: never) reads extents from that size up to `direct_min` with RWF_DONTCACHE.
    /// `priced` overrides whether the planner prices the reads. Without `full_file` an opened index reads its vectors
    /// through the saved index's own mapping (normal advice, not priced).
    /// `pseudo`: the corpus pseudo-queries the legacy arm's dial is fitted on (`knn_nprobe` with eps > 0), made then.
    /// `rule`: the exact planner's rule in rows in memory, "fixed" (`Plan::default`'s R0 and kappa) or "measured"
    /// (from this machine's measured constants, `adaptive::Measured::rule`, timed on first use and cached in `dir`).
    #[new]
    #[pyo3(signature = (dir, core=None, pseudo=1000, plan="auto", dlv=None, ddec=None, full_file=None, advice="random",
                        read="uring", willneed=true, direct_min=Some(8192), dontcache_min=None, priced=None, rule="fixed"))]
    #[allow(clippy::too_many_arguments)]
    fn new(py: Python<'_>, dir: String, core: Option<usize>, pseudo: usize, plan: &str, dlv: Option<Vec<usize>>, ddec: Option<u32>,
           full_file: Option<String>, advice: &str, read: &str, willneed: bool, direct_min: Option<usize>, dontcache_min: Option<usize>,
           priced: Option<bool>, rule: &str) -> PyResult<Index> {
        if rule != "fixed" && rule != "measured" {
            return Err(PyValueError::new_err(format!("rule is fixed or measured, not {rule:?}")));
        }
        if read != "uring" && read != "mmap" {
            return Err(PyValueError::new_err(format!("read is uring or mmap, not {read:?}")));
        }
        let mode = match plan {
            "auto" => PlanMode::Auto,
            "index" => PlanMode::Index,
            "flat" => PlanMode::Flat,
            "scan" => PlanMode::Scan,
            _ => return Err(PyValueError::new_err(format!("plan is auto, index, flat or scan, not {plan:?}"))),
        };
        py.detach(|| {
            let dlv = match dlv {
                None => BuildOptions::default().dlv,
                Some(v) if v.is_empty() => None,
                v => v,
            };
            let o = BuildOptions { dlv, ..BuildOptions::default() };
            let mut log = |s: String| eprintln!("{s}");
            let same = saved_options(&dir).is_ok_and(|s| s.encoding_json() == o.encoding_json());
            let (p, mut ix) = match same.then(|| open(&dir, &OpenOptions::default(), &mut log)) {
                Some(Ok(x)) => x,
                _ => rebuild(&dir, &o, &mut log).map_err(PyValueError::new_err)?,
            };
            let mut plan = Plan { mode, ddec, ..Plan::default() };
            plan.check(&ix).map_err(PyValueError::new_err)?;
            if let Some(ff) = &full_file {
                let Full2::Real { rows } = &mut ix.full else { return Err(PyValueError::new_err("full_file needs stored vectors")) };
                let rows = std::sync::Arc::get_mut(rows).ok_or_else(|| PyValueError::new_err("the full vectors are shared; cannot move them to a file"))?;
                rows.to_file(ff, advice, &p.vector_files().iter().map(|s| s.as_str()).collect::<Vec<_>>()).map_err(PyValueError::new_err)?;
                let pol = rows.policy().ok_or_else(|| PyValueError::new_err("the full vectors were not mapped from the file"))?;
                pol.willneed.store(willneed, Relaxed);
                pol.uring.store(read == "uring", Relaxed);
                pol.priced.store(priced.unwrap_or(read == "uring"), Relaxed);
                pol.direct_min.store(direct_min.unwrap_or(usize::MAX), Relaxed);
                pol.dontcache_min.store(dontcache_min.unwrap_or(usize::MAX), Relaxed);
                if pol.priced.load(Relaxed) {
                    plan = disk_constants(&ix, plan, &dir, &mut log);
                }
            }
            let approx = Approx::spread(&ix);
            let pool = rayon::ThreadPoolBuilder::new().num_threads(1).build().map_err(|e| PyValueError::new_err(e.to_string()))?;
            if let Some(cpu) = core {
                pool.install(|| vexact::util::pin(cpu));
            }
            let adaptive = std::sync::OnceLock::new();
            if rule == "measured" {
                let m = pool.install(|| measured_constants(&ix, &dir, &mut log)).ok_or_else(|| PyValueError::new_err("the measured rule needs stored vectors to time"))?;
                (plan.r_rows, plan.r_kappa) = m.rule().unwrap_or((usize::MAX, 0.0));
                let _ = adaptive.set(Adaptive::new(&ix, m));
            }
            let (npseudo, pseudo) = (pseudo, std::sync::OnceLock::new());
            Ok(Index { _p: p, ix, plan, approx, npseudo, pseudo, dials: HashMap::new(), vis: None, pool, adaptive, dir })
        })
    }

    /// Restrict every later query to these entity ids (any order); None lifts the restriction.
    #[pyo3(signature = (ids=None))]
    fn set_visible(&mut self, ids: Option<PyReadonlyArray1<'_, i64>>) -> PyResult<()> {
        self.vis = match ids {
            None => None,
            Some(a) => {
                let n = self.ix.nrows as i64;
                let a = a.as_slice()?;
                if let Some(&bad) = a.iter().find(|&&e| e < 0 || e >= n) {
                    return Err(PyValueError::new_err(format!("entity id {bad} is outside 0..{n}")));
                }
                let bm: RoaringBitmap = a.iter().map(|&e| e as u32).collect();
                Some(Vis2::new(&self.ix, &bm))
            }
        };
        Ok(())
    }

    /// The legacy arm's dial factors for k at miss rate `eps` (None at 0), fitted on first use.
    fn eps_dial(&mut self, py: Python<'_>, k: usize, eps: f64) -> PyResult<Option<Vec<f32>>> {
        if eps <= 0.0 {
            return Ok(None);
        }
        self.fit_dial(py, k);
        match self.dials.get(&k) {
            Some(d) if d.eps.iter().any(|&x| (x - eps).abs() <= 1e-12 * eps) => Ok(Some(d.for_eps(eps).to_vec())),
            Some(d) => Err(PyValueError::new_err(format!("eps {eps} has no fitted dial; use one of {:?}", d.eps))),
            None => Err(PyValueError::new_err("no dial could be fitted: the index was opened with pseudo=0")),
        }
    }

    /// The approximate k nearest (`v2::adaptive`): clusters in the order of mu + psi sigma (centre score plus psi times
    /// the spread of the cluster's scores along the query) until the next one's falls below the running k-th best,
    /// racing a scan of the visible rows (exact, at most twice the scan's modelled cost). `psi` is the knob: 4.5 by
    /// default (recall@10 about 0.85-0.96 unfiltered on glove, yahoo and LAION, more under visible sets), larger reads
    /// more, inf reads every cluster (exact). Each cluster is read directly or on the index path by this machine's
    /// measured constants (timed on first use and cached in the index directory). Returns k ids whenever k items are
    /// visible (-1 only where fewer are).
    #[pyo3(signature = (q, k, psi=PSI))]
    fn knn_approx<'py>(&self, py: Python<'py>, q: PyReadonlyArray1<'py, f32>, k: usize, psi: f64) -> PyResult<Bound<'py, PyArray1<i64>>> {
        let q = q.as_slice()?;
        let ad = self.adaptive_args(py, q, Some(k), psi)?;
        let (ix, plan, vis) = (&self.ix, self.plan, self.vis.as_ref());
        let ents = py.detach(|| self.pool.install(|| ad.knn(ix, q, k, vis, psi, plan).0.ents));
        Ok(ids(py, &ents, k))
    }

    /// The approximate threshold query (`v2::adaptive`): the visible entity ids, ascending, scoring at least t among the
    /// clusters the support table takes in and those whose mu + psi sigma reaches t, read exactly; a subset of
    /// `threshold`'s answer, and all of it when the query hands over (to a scan, or to the exact query, when either costs
    /// no more than those reads) or psi is inf. `psi` as for `knn_approx`.
    #[pyo3(signature = (q, t, psi=PSI))]
    fn threshold_approx<'py>(&self, py: Python<'py>, q: PyReadonlyArray1<'py, f32>, t: f64, psi: f64) -> PyResult<Bound<'py, PyArray1<i64>>> {
        let q = q.as_slice()?;
        let ad = self.adaptive_args(py, q, None, psi)?;
        let (ix, plan, vis) = (&self.ix, self.plan, self.vis.as_ref());
        let ents = py.detach(|| self.pool.install(|| {
            let r = ad.threshold(ix, q, t, vis, psi, plan).0;
            to_bitmap2(ix, &r.rows).iter().map(|e| e as i64).collect::<Vec<i64>>()
        }));
        Ok(PyArray1::from_vec(py, ents))
    }

    /// What one approximate query did, as a dict: its path ("probe", "race", "scan", "exact", "empty"), the clusters
    /// visited, read directly and on the index path, the groups set up, the clusters given a spread, the modelled
    /// costs (the order's, the whole query's, a scan's, and for a threshold query the exact query's); and rows touched,
    /// full vectors read, clusters on the index path and the time in ms. With `t`, the threshold query at t, else
    /// k-NN at k.
    #[pyo3(signature = (q, k=10, psi=PSI, t=None))]
    fn explain_approx<'py>(&self, py: Python<'py>, q: PyReadonlyArray1<'py, f32>, k: usize, psi: f64, t: Option<f64>) -> PyResult<Bound<'py, pyo3::types::PyDict>> {
        let q = q.as_slice()?;
        let ad = self.adaptive_args(py, q, if t.is_none() { Some(k) } else { None }, psi)?;
        let (ix, plan, vis) = (&self.ix, self.plan, self.vis.as_ref());
        let tm = std::time::Instant::now();
        let (i, st) = py.detach(|| self.pool.install(|| match t {
            Some(t) => {
                let (r, i) = ad.threshold(ix, q, t, vis, psi, plan);
                (i, r.st)
            }
            None => {
                let (r, i) = ad.knn(ix, q, k, vis, psi, plan);
                (i, r.st)
            }
        }));
        let ms = tm.elapsed().as_secs_f64() * 1e3;
        let d = info_dict(py, &i)?;
        d.set_item("touched", st.touched)?;
        d.set_item("full_reads", st.full_reads)?;
        d.set_item("clusters_open", st.clusters_open)?;
        d.set_item("ms", ms)?;
        Ok(d)
    }

    /// Where one approximate query spends its time, as `profile`: the cycles of each phase (`Stats::cyc`; `P_PLAN` the
    /// centre scores, `P_SPREAD` the spreads, `P_ORDER` the order (keys, heap, stop), `P_CHOICE` the path choice, the
    /// anchor search and the race's accounting (a threshold query's support verdicts and pricing), `P_QUERY` the whole
    /// call; the counts C_SPREADS (directions projected), C_DIRECT, C_INDEX, C_GROUPS, C_VISITED) and the time stamp
    /// counter's cycles per second. With `t`, the threshold query at t.
    #[pyo3(signature = (q, k=10, psi=PSI, t=None))]
    fn profile_approx(&self, py: Python<'_>, q: PyReadonlyArray1<'_, f32>, k: usize, psi: f64, t: Option<f64>) -> PyResult<(Vec<u64>, f64)> {
        let q = q.as_slice()?;
        let ad = self.adaptive_args(py, q, if t.is_none() { Some(k) } else { None }, psi)?;
        let (ix, plan, vis) = (&self.ix, self.plan, self.vis.as_ref());
        Ok(py.detach(|| self.pool.install(|| {
            vexact::v2::q::PROF.store(true, Relaxed);
            let (tm, c0) = (std::time::Instant::now(), unsafe { core::arch::x86_64::_rdtsc() });
            let mut st = match t {
                Some(t) => ad.threshold(ix, q, t, vis, psi, plan).0.st,
                None => ad.knn(ix, q, k, vis, psi, plan).0.st,
            };
            let c1 = unsafe { core::arch::x86_64::_rdtsc() };
            vexact::v2::q::PROF.store(false, Relaxed);
            st.cyc[vexact::v2::q::P_QUERY] = c1 - c0;
            (st.cyc.to_vec(), (c1 - c0) as f64 / tm.elapsed().as_secs_f64())
        })))
    }

    /// The approximate queries' measured constants (`adaptive::Measured`, microseconds), with the exact planner's rule
    /// in rows they give (r_rows, r_kappa; None where every cluster would be scored directly) and the plan's own.
    #[getter]
    fn measured_costs<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, pyo3::types::PyDict>> {
        let m = self.adaptive(py)?.m;
        let d = pyo3::types::PyDict::new(py);
        for line in m.to_kv().lines() {
            if let Some((key, x)) = line.split_once('=') {
                d.set_item(key, x.parse::<f64>().unwrap_or(f64::NAN))?;
            }
        }
        d.set_item("rule", m.rule())?;
        d.set_item("plan_rule", (self.plan.r_rows, self.plan.r_kappa))?;
        Ok(d)
    }

    /// Fit the legacy arm's dial for k (once per k; it is used when eps > 0), on `npseudo` corpus pseudo-queries made
    /// on the first fit.
    fn fit_dial(&mut self, py: Python<'_>, k: usize) {
        if self.npseudo == 0 || self.dials.contains_key(&k) {
            return;
        }
        let (ix, plan, ap, n) = (&self.ix, self.plan, &self.approx, self.npseudo);
        let pseudo = self.pseudo.get_or_init(|| py.detach(|| pseudo_queries(ix, n, 10)));
        let ps = &pseudo[..pseudo.len().min(200)];
        let d = py.detach(|| Dial::fit(ix, ps, plan, false, k, ap));
        self.dials.insert(k, d);
    }

    /// The exact k nearest entity ids by cosine, best first (-1 where fewer than k are visible).
    fn knn<'py>(&self, py: Python<'py>, q: PyReadonlyArray1<'py, f32>, k: usize) -> PyResult<Bound<'py, PyArray1<i64>>> {
        let q = q.as_slice()?;
        let (ix, plan, vis) = (&self.ix, self.plan, self.vis.as_ref());
        let ents = py.detach(|| self.pool.install(|| knn2(ix, q, k, vis, plan).ents));
        Ok(ids(py, &ents, k))
    }

    /// Legacy comparison arm, kept until the owner's benchmark has run: the approximate k nearest in the `spread` order
    /// by nprobe (`knn2_fast`), exact search of the visible items of the first `nprobe` clusters, with the dial fitted
    /// for miss rate `eps` per level (0: off).
    #[pyo3(signature = (q, k, nprobe, eps=0.0))]
    fn knn_nprobe<'py>(&mut self, py: Python<'py>, q: PyReadonlyArray1<'py, f32>, k: usize, nprobe: usize, eps: f64) -> PyResult<Bound<'py, PyArray1<i64>>> {
        if eps > 0.0 {
            self.fit_dial(py, k);
        }
        let dial = match self.dials.get(&k) {
            Some(d) if eps > 0.0 => {
                if !d.eps.iter().any(|&x| (x - eps).abs() <= 1e-12 * eps) {
                    return Err(PyValueError::new_err(format!("eps {eps} has no fitted dial; use one of {:?}", d.eps)));
                }
                Some(d.for_eps(eps))
            }
            _ => None,
        };
        let q = q.as_slice()?;
        let (ix, ap, plan, vis) = (&self.ix, &self.approx, self.plan, self.vis.as_ref());
        let ents = py.detach(|| self.pool.install(|| knn2_fast(ix, ap, q, k, vis, nprobe, dial, plan).ents));
        Ok(ids(py, &ents, k))
    }

    /// The threshold query: every visible entity id whose stored vector x has q.x >= t, ascending (int64). `q` is taken
    /// as given (pass a unit vector for cosine thresholds); the index stores unit vectors. The answer is exact: items the
    /// index's bounds cannot decide are checked on their full vectors, as `threshold2` does through the planner
    /// (as in `run2.rs` dbcmp's `index`). `verify` also checks every decided item's bounds against its full vector
    /// (slow; for tests): a bound that failed raises an error.
    #[pyo3(signature = (q, t, verify=false))]
    fn threshold<'py>(&self, py: Python<'py>, q: PyReadonlyArray1<'py, f32>, t: f64, verify: bool) -> PyResult<Bound<'py, PyArray1<i64>>> {
        let q = q.as_slice()?;
        if q.len() != self.ix.n() {
            return Err(PyValueError::new_err(format!("the query has {} values, the index {}", q.len(), self.ix.n())));
        }
        let (ix, plan, vis) = (&self.ix, self.plan, self.vis.as_ref());
        let (ents, viol) = py.detach(|| {
            self.pool.install(|| {
                let pr = prep2(ix, q);
                let r = threshold2(ix, &pr, t, &Opts2 { vis, verify, want_iv: false, plan });
                let ents: Vec<i64> = to_bitmap2(ix, &r.rows).iter().map(|e| e as i64).collect();
                (ents, (r.st.viol_det, r.st.viol_rand))
            })
        });
        if verify && viol != (0, 0) {
            return Err(PyValueError::new_err(format!("bounds violated: {} deterministic, {} randomised", viol.0, viol.1)));
        }
        Ok(PyArray1::from_vec(py, ents))
    }

    /// Exact k-NN of every row of `qs` on all threads (for checks, not for timing).
    fn knn_batch<'py>(&self, py: Python<'py>, qs: PyReadonlyArray2<'py, f32>, k: usize) -> PyResult<Vec<Bound<'py, PyArray1<i64>>>> {
        use rayon::prelude::*;
        let a = qs.as_array();
        let rows: Vec<Vec<f32>> = a.outer_iter().map(|r| r.to_vec()).collect();
        let (ix, plan, vis) = (&self.ix, self.plan, self.vis.as_ref());
        let out: Vec<Vec<(f64, u32)>> = py.detach(|| rows.par_iter().map(|q| knn2(ix, q, k, vis, plan).ents).collect());
        Ok(out.iter().map(|e| ids(py, e, k)).collect())
    }

    /// The planner's constants, one `name=value` per line (the rule in rows; the disk constants are unused in memory).
    #[getter]
    fn plan(&self) -> String {
        self.plan.to_kv()
    }

    /// What one query did: rows touched, full vectors read, clusters opened on the index path, and the time in ms,
    /// exact (nprobe None) or by the legacy arm (`knn_nprobe`).
    #[pyo3(signature = (q, k, nprobe=None, eps=0.0))]
    fn explain(&mut self, py: Python<'_>, q: PyReadonlyArray1<'_, f32>, k: usize, nprobe: Option<usize>, eps: f64) -> PyResult<(u64, u64, u64, f64)> {
        if eps > 0.0 {
            self.fit_dial(py, k);
        }
        let dial = if eps > 0.0 { self.dials.get(&k).map(|d| d.for_eps(eps)) } else { None };
        let q = q.as_slice()?;
        let (ix, ap, plan, vis) = (&self.ix, &self.approx, self.plan, self.vis.as_ref());
        let t = std::time::Instant::now();
        let o: KnnOut2 = py.detach(|| self.pool.install(|| match nprobe {
            None => knn2(ix, q, k, vis, plan),
            Some(np) => knn2_fast(ix, ap, q, k, vis, np, dial, plan),
        }));
        Ok((o.st.touched, o.st.full_reads, o.st.clusters_open, t.elapsed().as_secs_f64() * 1e3))
    }

    /// Where one query of the legacy arm (`knn_nprobe`) spends its time: the time-stamp-counter cycles of each phase
    /// (`Stats::cyc`, indexed by the `P_*` constants of `v2::q`; `P_QUERY` is the whole call), with the time stamp
    /// counter's cycles per second.
    #[pyo3(signature = (q, k, nprobe, eps=0.0))]
    fn profile(&mut self, py: Python<'_>, q: PyReadonlyArray1<'_, f32>, k: usize, nprobe: usize, eps: f64) -> PyResult<(Vec<u64>, f64)> {
        use std::sync::atomic::Ordering::Relaxed;
        if eps > 0.0 {
            self.fit_dial(py, k);
        }
        let dial = if eps > 0.0 { self.dials.get(&k).map(|d| d.for_eps(eps)) } else { None };
        let q = q.as_slice()?;
        let (ix, ap, plan, vis) = (&self.ix, &self.approx, self.plan, self.vis.as_ref());
        let (cyc, hz) = py.detach(|| self.pool.install(|| {
            vexact::v2::q::PROF.store(true, Relaxed);
            let (t, c0) = (std::time::Instant::now(), unsafe { core::arch::x86_64::_rdtsc() });
            let mut o = knn2_fast(ix, ap, q, k, vis, nprobe, dial, plan);
            let c1 = unsafe { core::arch::x86_64::_rdtsc() };
            vexact::v2::q::PROF.store(false, Relaxed);
            o.st.cyc[vexact::v2::q::P_QUERY] = c1 - c0;
            (o.st.cyc.to_vec(), (c1 - c0) as f64 / t.elapsed().as_secs_f64())
        }));
        Ok((cyc, hz))
    }

    /// Drops the full vectors (when on file, from `full_file`) from this process's mappings and from the page cache;
    /// the codes stay. Returns the bytes still resident afterwards.
    fn evict_full(&self) -> usize {
        match self.ix.own_rows() {
            Some(r) => {
                r.evict();
                r.resident()
            }
            None => 0,
        }
    }

    /// Bytes of the full vectors resident in the page cache (0 when they are in memory, not on file).
    fn full_resident(&self) -> usize {
        self.ix.own_rows().map_or(0, |r| r.resident())
    }

    /// The file the full vectors are mapped from, if any.
    #[getter]
    fn full_path(&self) -> Option<String> {
        self.ix.own_rows().and_then(|r| r.file_path()).map(|p| p.to_string_lossy().to_string())
    }

    /// The process-wide counters of full-vector reads through the reader (disk mode, read="uring"): reads issued,
    /// bytes read, and nanoseconds spent waiting for them (`disk::NREADS`, `NBYTES`, `WAIT_NS`). Reads through the
    /// mapping (faults) are not counted here.
    #[staticmethod]
    fn io_counters() -> (u64, u64, u64) {
        (disk::NREADS.load(Relaxed), disk::NBYTES.load(Relaxed), disk::WAIT_NS.load(Relaxed))
    }

    /// Whether io_uring can be used here.
    #[staticmethod]
    fn uring_available() -> bool {
        disk::uring_available()
    }

    /// The dimension the index takes queries in (the fit's, padded to a multiple of 32).
    #[getter]
    fn dim(&self) -> usize {
        self.ix.n()
    }

    #[getter]
    fn n(&self) -> usize {
        self.ix.nrows
    }

    #[getter]
    fn clusters(&self) -> usize {
        self.ix.k()
    }
}

impl Index {
    /// The approximate queries' state, set up on first use: the constants read from the index directory's cache or
    /// timed on this index's core and saved there (`adaptive::measured_constants`).
    fn adaptive(&self, py: Python<'_>) -> PyResult<&Adaptive> {
        let (ix, dir, pool) = (&self.ix, &self.dir, &self.pool);
        let r = self.adaptive.get_or_init(|| py.detach(|| pool.install(|| {
            let mut log = |s: String| eprintln!("{s}");
            measured_constants(ix, dir, &mut log)
                .ok_or_else(|| "the approximate queries' cost model is timed on stored vectors, and this index has none; query it exactly".to_string())
                .and_then(|m| Adaptive::new(ix, m))
        })));
        r.as_ref().map_err(|e| PyValueError::new_err(e.clone()))
    }

    /// The approximate queries' state after checking the query, k (k-NN) and psi.
    fn adaptive_args(&self, py: Python<'_>, q: &[f32], k: Option<usize>, psi: f64) -> PyResult<&Adaptive> {
        check_query(&self.ix, q, k)?;
        if psi.is_nan() {
            return Err(PyValueError::new_err("psi is a number (inf reads every cluster), not NaN"));
        }
        self.adaptive(py)
    }
}

fn ids<'py>(py: Python<'py>, ents: &[(f64, u32)], k: usize) -> Bound<'py, PyArray1<i64>> {
    let mut v = vec![-1i64; k];
    for (o, &(_, e)) in v.iter_mut().zip(ents) {
        *o = e as i64;
    }
    PyArray1::from_vec(py, v)
}

#[pymodule]
fn vexact_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Index>()?;
    Ok(())
}
