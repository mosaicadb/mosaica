//! Python bindings over the fast-scan index for ANN-Benchmarks, VIBE and `bench/suite`: open a directory written by
//! `rust/tools/fit.py` and answer k-NN queries (exact or approximate) and threshold queries one at a time, on one thread.
use numpy::{PyArray1, PyReadonlyArray1, PyReadonlyArray2};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use roaring::RoaringBitmap;
use vexact::index::Params;
use std::sync::atomic::Ordering::Relaxed;
use vexact::v2::adaptive::{measured_constants, typical_rows, Adaptive, AdaptiveInfo, PSI};
use vexact::v2::q::{disk_constants, knn2, knn2_fast, prep2, threshold2, to_bitmap2, Approx, KnnOut2, Opts2, Plan, PlanMode, Vis2};
use vexact::v2::{disk, open, rebuild, saved_options, BuildOptions, Full2, Index2, OpenOptions};

#[pyclass(module = "vexact_py")]
struct Index {
    _p: Params,
    ix: Index2,
    plan: Plan,
    /// The legacy approximate order (`spread`, by nprobe), kept as the comparison arm until the owner's benchmark has
    /// run.
    approx: Approx,
    vis: Option<Vis2>,
    pool: rayon::ThreadPool,
    /// The approximate queries (`v2::adaptive`) with this machine's measured constants (timed at the first open on
    /// this machine and cached in the index directory), or why they cannot run.
    adaptive: Result<Adaptive, String>,
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
    for (key, x) in [("visited", i.read.len()), ("direct", i.direct), ("index", i.index), ("groups", i.groups), ("spreads", i.spreads)] {
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
    /// save the index there (`v2::rebuild`). Queries run on one thread, pinned to `core` when given. The approximate
    /// queries' cost model is read from the index directory's cache, or timed there on that core (a few seconds, once
    /// per machine, library and index: `adaptive::measured_constants`).
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
    /// `rule`: the exact planner's rule in rows in memory, "fixed" (`Plan::default`'s R0 and kappa) or "measured"
    /// (from this machine's measured constants, `adaptive::Measured::rule` at the index's typical cluster size).
    #[new]
    #[pyo3(signature = (dir, core=None, plan="auto", dlv=None, ddec=None, full_file=None, advice="random",
                        read="uring", willneed=true, direct_min=Some(8192), dontcache_min=None, priced=None, rule="fixed"))]
    #[allow(clippy::too_many_arguments)]
    fn new(py: Python<'_>, dir: String, core: Option<usize>, plan: &str, dlv: Option<Vec<usize>>, ddec: Option<u32>,
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
            let adaptive = pool.install(|| measured_constants(&ix, &dir, &mut log)).and_then(|m| Adaptive::new(&ix, m));
            if rule == "measured" {
                let ad = adaptive.as_ref().map_err(|e| PyValueError::new_err(format!("the measured rule: {e}")))?;
                (plan.r_rows, plan.r_kappa) = ad.m.rule(typical_rows(&ix)).unwrap_or((usize::MAX, 0.0));
            }
            Ok(Index { _p: p, ix, plan, approx, vis: None, pool, adaptive })
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

    /// The approximate k nearest (`v2::adaptive`): the exact query (`knn`) when psi is inf or when the exact planner
    /// scans the visible rows first (sparse visible sets); else the clusters holding a visible row read in an order that
    /// does not depend on psi, by mu + z sigma (centre score plus z(1, v) = Phi^-1(v / (v + 1)) times the spread of the
    /// cluster's scores along the query, v its visible rows), the next 8 of a stream priced ahead; psi moves only the
    /// stop, when the largest mu + psi sigma of those 8 is below the running k-th best (minus infinity until k items
    /// are held). The stop is a heuristic: a cluster not yet priced may still have a mu + psi sigma above the k-th
    /// best. Each cluster is read by the exact planner's rule in rows (directly when it holds few visible rows, else on
    /// the index path); no cost is modelled. `psi` is the knob, a number of at least 0: larger reads more; 4.5 by
    /// default, the recall it gives to be measured. Returns k ids whenever k items are visible (-1 only where fewer
    /// are).
    #[pyo3(signature = (q, k, psi=PSI))]
    fn knn_approx<'py>(&self, py: Python<'py>, q: PyReadonlyArray1<'py, f32>, k: usize, psi: f64) -> PyResult<Bound<'py, PyArray1<i64>>> {
        let q = q.as_slice()?;
        let ad = self.adaptive_args(q, Some(k), psi)?;
        let (ix, plan, vis) = (&self.ix, self.plan, self.vis.as_ref());
        let ents = py.detach(|| self.pool.install(|| ad.knn(ix, q, k, vis, psi, plan).0.ents));
        Ok(ids(py, &ents, k))
    }

    /// The approximate threshold query (`v2::adaptive`): the visible entity ids, ascending, scoring at least t among the
    /// clusters the support table takes in and those whose mu + psi sigma reaches t, read exactly; a subset of
    /// `threshold`'s answer, and all of it when the query hands over (to a scan, or to the exact query, when either costs
    /// no more than those reads), when psi is inf, and on a visible set the exact query scans first (`threshold` itself
    /// from the start). `psi` as for `knn_approx`.
    #[pyo3(signature = (q, t, psi=PSI))]
    fn threshold_approx<'py>(&self, py: Python<'py>, q: PyReadonlyArray1<'py, f32>, t: f64, psi: f64) -> PyResult<Bound<'py, PyArray1<i64>>> {
        let q = q.as_slice()?;
        let ad = self.adaptive_args(q, None, psi)?;
        let (ix, plan, vis) = (&self.ix, self.plan, self.vis.as_ref());
        let ents = py.detach(|| self.pool.install(|| {
            let r = ad.threshold(ix, q, t, vis, psi, plan).0;
            to_bitmap2(ix, &r.rows).iter().map(|e| e as i64).collect::<Vec<i64>>()
        }));
        Ok(PyArray1::from_vec(py, ents))
    }

    /// What one approximate query did, as a dict: its path ("probe", "race", "scan", "exact", "sparse", "empty"), the clusters
    /// visited, read directly and on the index path, the groups set up, the clusters given a spread, the modelled
    /// costs (the order's, the whole query's, a scan's, and for a threshold query the exact query's); and rows touched,
    /// full vectors read, clusters on the index path and the time in ms. With `t`, the threshold query at t, else
    /// k-NN at k.
    #[pyo3(signature = (q, k=10, psi=PSI, t=None))]
    fn explain_approx<'py>(&self, py: Python<'py>, q: PyReadonlyArray1<'py, f32>, k: usize, psi: f64, t: Option<f64>) -> PyResult<Bound<'py, pyo3::types::PyDict>> {
        let q = q.as_slice()?;
        let ad = self.adaptive_args(q, if t.is_none() { Some(k) } else { None }, psi)?;
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
        let ad = self.adaptive_args(q, if t.is_none() { Some(k) } else { None }, psi)?;
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
        let m = self.adaptive()?.m;
        let d = pyo3::types::PyDict::new(py);
        for line in m.to_kv().lines() {
            if let Some((key, x)) = line.split_once('=') {
                d.set_item(key, x.parse::<f64>().unwrap_or(f64::NAN))?;
            }
        }
        d.set_item("rule", m.rule(typical_rows(&self.ix)))?;
        d.set_item("plan_rule", (self.plan.r_rows, self.plan.r_kappa))?;
        Ok(d)
    }

    /// The exact k nearest entity ids by cosine, best first (-1 where fewer than k are visible).
    fn knn<'py>(&self, py: Python<'py>, q: PyReadonlyArray1<'py, f32>, k: usize) -> PyResult<Bound<'py, PyArray1<i64>>> {
        let q = q.as_slice()?;
        let (ix, plan, vis) = (&self.ix, self.plan, self.vis.as_ref());
        let ents = py.detach(|| self.pool.install(|| knn2(ix, q, k, vis, plan).ents));
        Ok(ids(py, &ents, k))
    }

    /// Legacy comparison arm, kept until the owner's benchmark has run: the approximate k nearest in the `spread` order
    /// by nprobe (`knn2_fast`), exact search of the visible items of the first `nprobe` clusters.
    fn knn_nprobe<'py>(&self, py: Python<'py>, q: PyReadonlyArray1<'py, f32>, k: usize, nprobe: usize) -> PyResult<Bound<'py, PyArray1<i64>>> {
        let q = q.as_slice()?;
        let (ix, ap, plan, vis) = (&self.ix, &self.approx, self.plan, self.vis.as_ref());
        let ents = py.detach(|| self.pool.install(|| knn2_fast(ix, ap, q, k, vis, nprobe, None, plan).ents));
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
    #[pyo3(signature = (q, k, nprobe=None))]
    fn explain(&self, py: Python<'_>, q: PyReadonlyArray1<'_, f32>, k: usize, nprobe: Option<usize>) -> PyResult<(u64, u64, u64, f64)> {
        let q = q.as_slice()?;
        let (ix, ap, plan, vis) = (&self.ix, &self.approx, self.plan, self.vis.as_ref());
        let t = std::time::Instant::now();
        let o: KnnOut2 = py.detach(|| self.pool.install(|| match nprobe {
            None => knn2(ix, q, k, vis, plan),
            Some(np) => knn2_fast(ix, ap, q, k, vis, np, None, plan),
        }));
        Ok((o.st.touched, o.st.full_reads, o.st.clusters_open, t.elapsed().as_secs_f64() * 1e3))
    }

    /// Where one query of the legacy arm (`knn_nprobe`) spends its time: the time-stamp-counter cycles of each phase
    /// (`Stats::cyc`, indexed by the `P_*` constants of `v2::q`; `P_QUERY` is the whole call), with the time stamp
    /// counter's cycles per second.
    fn profile(&self, py: Python<'_>, q: PyReadonlyArray1<'_, f32>, k: usize, nprobe: usize) -> PyResult<(Vec<u64>, f64)> {
        let q = q.as_slice()?;
        let (ix, ap, plan, vis) = (&self.ix, &self.approx, self.plan, self.vis.as_ref());
        let (cyc, hz) = py.detach(|| self.pool.install(|| {
            vexact::v2::q::PROF.store(true, Relaxed);
            let (t, c0) = (std::time::Instant::now(), unsafe { core::arch::x86_64::_rdtsc() });
            let mut o = knn2_fast(ix, ap, q, k, vis, nprobe, None, plan);
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
    /// The approximate queries' state, or why they cannot run.
    fn adaptive(&self) -> PyResult<&Adaptive> {
        self.adaptive.as_ref().map_err(|e| PyValueError::new_err(e.clone()))
    }

    /// The approximate queries' state after checking the query, k (k-NN) and psi.
    fn adaptive_args(&self, q: &[f32], k: Option<usize>, psi: f64) -> PyResult<&Adaptive> {
        check_query(&self.ix, q, k)?;
        if psi.is_nan() || psi < 0.0 {
            return Err(PyValueError::new_err(format!("psi is a number of at least 0 (inf reads every cluster), not {psi}")));
        }
        self.adaptive()
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
