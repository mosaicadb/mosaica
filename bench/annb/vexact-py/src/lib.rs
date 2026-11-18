//! Python bindings over the fast-scan index for ANN-Benchmarks, VIBE and `bench/suite`: open a directory written by
//! `rust/tools/fit.py` and answer k-NN queries (exact or approximate) and threshold queries one at a time, on one thread.
use numpy::{PyArray1, PyReadonlyArray1, PyReadonlyArray2};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use roaring::RoaringBitmap;
use std::collections::HashMap;
use vexact::index::Params;
use std::sync::atomic::Ordering::Relaxed;
use vexact::v2::q::{disk_constants, knn2, knn2_fast, prep2, pseudo_queries, threshold2, to_bitmap2, Approx, OrderKind, Dial, KnnOut2, Opts2, Plan, PlanMode, Vis2};
use vexact::v2::{disk, open, rebuild, saved_options, BuildOptions, Full2, Index2, OpenOptions};

#[pyclass(module = "vexact_py")]
struct Index {
    _p: Params,
    ix: Index2,
    plan: Plan,
    approx: Approx,
    pseudo: Vec<(usize, Vec<f32>, Vec<u32>)>,
    dials: HashMap<usize, Dial>,
    vis: Option<Vis2>,
    pool: rayon::ThreadPool,
}

#[pymethods]
impl Index {
    /// Open the index saved in the fitted directory `dir` (from `rust/tools/fit.py`) when it was built with these options
    /// (`v2::saved_options`) and is not stale (`v2::open`, which maps it and encodes nothing); else encode the fit and
    /// save the index there (`v2::rebuild`). Then fit the approximate mode's cluster order on `pseudo` corpus pseudo-queries. Queries run on one
    /// thread, pinned to `core` when given.
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
    /// `order` is the approximate mode's cluster order (`OrderKind`): "radius", with f fitted on `pseudo` corpus
    /// pseudo-queries, "spread" or "h4", "h16", "h4x8" (H with that many directions per cluster, and optionally
    /// its candidates per probed cluster, `OrderKind::H`), with f at
    /// `OrderKind::default_f`. `fit_queries` fits f for any of them on real queries.
    #[new]
    #[pyo3(signature = (dir, core=None, pseudo=1000, plan="auto", dlv=None, ddec=None, full_file=None, advice="random",
                        read="uring", willneed=true, direct_min=Some(8192), dontcache_min=None, priced=None, order="spread"))]
    #[allow(clippy::too_many_arguments)]
    fn new(py: Python<'_>, dir: String, core: Option<usize>, pseudo: usize, plan: &str, dlv: Option<Vec<usize>>, ddec: Option<u32>,
           full_file: Option<String>, advice: &str, read: &str, willneed: bool, direct_min: Option<usize>, dontcache_min: Option<usize>,
           priced: Option<bool>, order: &str) -> PyResult<Index> {
        let kind = OrderKind::parse(order).map_err(PyValueError::new_err)?;
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
            let pseudo = pseudo_queries(&ix, pseudo, 10);
            let approx = match kind.default_f() {
                Some(f) => Approx::new(&ix, kind, f, Approx::radii(&ix)),
                None if pseudo.is_empty() => Approx::centres(&ix),
                None => Approx::fit(&ix, &pseudo, kind).0,
            };
            let pool = rayon::ThreadPoolBuilder::new().num_threads(1).build().map_err(|e| PyValueError::new_err(e.to_string()))?;
            if let Some(cpu) = core {
                pool.install(|| vexact::util::pin(cpu));
            }
            Ok(Index { _p: p, ix, plan, approx, pseudo, dials: HashMap::new(), vis: None, pool })
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

    /// Refit the approximate mode's cluster order, and later dials, on a sample of real queries instead of corpus
    /// pseudo-queries: `qs` (one query per row, normalised here) and `gt`, each query's nearest entity ids, best first
    /// (the first 10 are used). Dials fitted before are dropped.
    fn fit_queries(&mut self, py: Python<'_>, qs: PyReadonlyArray2<'_, f32>, gt: PyReadonlyArray2<'_, i64>) -> PyResult<()> {
        let (qa, ga) = (qs.as_array(), gt.as_array());
        if qa.nrows() != ga.nrows() || qa.nrows() == 0 || qa.ncols() != self.ix.n() {
            return Err(PyValueError::new_err(format!(
                "need as many neighbour rows as queries, at least one query, and {} columns; got {:?} and {:?}",
                self.ix.n(), qa.shape(), ga.shape())));
        }
        let n = self.ix.nrows as i64;
        let mut pseudo = Vec::with_capacity(qa.nrows());
        for (q, g) in qa.outer_iter().zip(ga.outer_iter()) {
            let qn = q.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt();
            if qn < 1e-12 {
                continue;
            }
            if let Some(&bad) = g.iter().find(|&&e| e < 0 || e >= n) {
                return Err(PyValueError::new_err(format!("entity id {bad} is outside 0..{n}")));
            }
            let q: Vec<f32> = q.iter().map(|&x| (x as f64 / qn) as f32).collect();
            // a row past the index marks a held-out query, not a stored row
            pseudo.push((usize::MAX, q, g.iter().take(10).map(|&e| e as u32).collect::<Vec<u32>>()));
        }
        let ix = &self.ix;
        let kind = self.approx.kind;
        self.approx = py.detach(|| Approx::fit(ix, &pseudo, kind).0);
        self.pseudo = pseudo;
        self.dials.clear();
        Ok(())
    }

    /// Switch the approximate mode's cluster order (as `order` at open). The radii are kept. Dials fitted before are
    /// dropped.
    fn set_order(&mut self, py: Python<'_>, order: &str) -> PyResult<()> {
        let kind = OrderKind::parse(order).map_err(PyValueError::new_err)?;
        let rad = std::mem::take(&mut self.approx.rad);
        let (ix, pseudo) = (&self.ix, &self.pseudo);
        self.approx = match kind.default_f() {
            Some(f) => py.detach(|| Approx::new(ix, kind, f, if rad.is_empty() { Approx::radii(ix) } else { rad })),
            None if pseudo.is_empty() => Approx::centres(ix),
            None => py.detach(|| Approx::fit(ix, pseudo, kind).0),
        };
        self.dials.clear();
        Ok(())
    }

    /// The approximate mode's cluster order and its f.
    #[getter]
    fn order(&self) -> (String, f64) {
        let name = match self.approx.kind {
            OrderKind::H { r, m } => format!("h{r}x{m}"),
            k => format!("{k:?}").to_lowercase(),
        };
        (name, self.approx.f)
    }

    /// Fit the approximate mode's dial for k (once per k; it is used when eps > 0).
    fn fit_dial(&mut self, py: Python<'_>, k: usize) {
        if self.pseudo.is_empty() || self.dials.contains_key(&k) {
            return;
        }
        let (ix, ps, plan, ap) = (&self.ix, &self.pseudo[..self.pseudo.len().min(200)], self.plan, &self.approx);
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

    /// The approximate k nearest: exact search of the visible items of the first `nprobe` clusters, with the dial
    /// fitted for miss rate `eps` per level (0: off).
    #[pyo3(signature = (q, k, nprobe, eps=0.0))]
    fn knn_approx<'py>(&mut self, py: Python<'py>, q: PyReadonlyArray1<'py, f32>, k: usize, nprobe: usize, eps: f64) -> PyResult<Bound<'py, PyArray1<i64>>> {
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
    /// exact (nprobe None) or approximate.
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

    /// Where one approximate query's time goes: the time-stamp-counter cycles of each phase (`Stats::cyc`, indexed by
    /// the `P_*` constants of `v2::q`; `P_QUERY` is the whole call), with the time stamp counter's cycles per second.
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
