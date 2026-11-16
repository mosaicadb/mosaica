//! Python bindings over the fast-scan index for ANN-Benchmarks and VIBE: open a directory written by `rust/tools/fit.py`
//! and answer k-NN queries one at a time, on one thread, exact or approximate.
use numpy::{PyArray1, PyReadonlyArray1, PyReadonlyArray2};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use roaring::RoaringBitmap;
use std::collections::HashMap;
use vexact::index::Params;
use vexact::v2::q::{knn2, knn2_fast, pseudo_queries, Approx, Dial, KnnOut2, Plan, PlanMode, Vis2};
use vexact::v2::{open, BuildOptions, Index2};

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
    /// Build the index from a fitted directory, time the planner's constants on one thread (pinned to `core` when
    /// given), and fit the approximate mode's cluster order on `pseudo` corpus pseudo-queries. `plan` is "auto" (the
    /// planner decides), "index" (full vectors and scans priced out, so every cluster takes the index path, in the
    /// approximate mode too) or "flat" (the planner's flat mode).
    #[new]
    #[pyo3(signature = (dir, core=None, pseudo=1000, calib=16, plan="auto"))]
    fn new(py: Python<'_>, dir: String, core: Option<usize>, pseudo: usize, calib: usize, plan: &str) -> PyResult<Index> {
        let force = match plan {
            "auto" | "index" | "flat" => plan.to_string(),
            _ => return Err(PyValueError::new_err(format!("plan is auto, index or flat, not {plan:?}"))),
        };
        py.detach(|| {
            let (p, ix) = open(&dir, &BuildOptions::default(), &mut |s| eprintln!("{s}"));
            let mut plan = Plan::calibrate(&ix, Plan::default(), calib, core);
            match force.as_str() {
                "index" => {
                    plan.mode = PlanMode::Index;
                    (plan.full_us, plan.scan_us, plan.cscan_us) = (1e3, 1e3, 1e3);
                }
                "flat" => plan.mode = PlanMode::Flat,
                _ => {}
            }
            let pseudo = pseudo_queries(&ix, pseudo, 10);
            let approx = if pseudo.is_empty() { Approx { f: 0.0, rad: Vec::new() } } else { Approx::fit(&ix, &pseudo).0 };
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
        self.approx = py.detach(|| Approx::fit(ix, &pseudo).0);
        self.pseudo = pseudo;
        self.dials.clear();
        Ok(())
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

    /// Exact k-NN of every row of `qs` on all threads (for checks, not for timing).
    fn knn_batch<'py>(&self, py: Python<'py>, qs: PyReadonlyArray2<'py, f32>, k: usize) -> PyResult<Vec<Bound<'py, PyArray1<i64>>>> {
        use rayon::prelude::*;
        let a = qs.as_array();
        let rows: Vec<Vec<f32>> = a.outer_iter().map(|r| r.to_vec()).collect();
        let (ix, plan, vis) = (&self.ix, self.plan, self.vis.as_ref());
        let out: Vec<Vec<(f64, u32)>> = py.detach(|| rows.par_iter().map(|q| knn2(ix, q, k, vis, plan).ents).collect());
        Ok(out.iter().map(|e| ids(py, e, k)).collect())
    }

    /// The planner's constants, one `name=value` per line.
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
