//! Python bindings over the fast-scan index for ANN-Benchmarks, VIBE and `bench/suite`: open a directory written by
//! `rust/tools/fit.py` and answer k-NN queries (exact or approximate) and threshold queries one at a time, on one thread.
use numpy::{PyArray1, PyReadonlyArray1, PyReadonlyArray2};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use roaring::RoaringBitmap;
use std::collections::HashMap;
use vexact::index::Params;
use vexact::v2::q::{knn2, knn2_fast, prep2, pseudo_queries, threshold2, to_bitmap2, Approx, Dial, KnnOut2, Opts2, Plan, PlanMode, Vis2};
use vexact::v2::{open, rebuild, BuildOptions, Index2};

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
    /// Encode the fitted directory `dir` (from `rust/tools/fit.py`) and save the index beside it, where `v2::open` finds
    /// it, or open the one already saved there for the same fit and options (`v2::open`, which maps it and encodes
    /// nothing). Then fit the approximate mode's cluster order on `pseudo` corpus pseudo-queries. Queries run on one
    /// thread, pinned to `core` when given.
    /// `plan`: "auto" (the planner: in memory, the support table and the rule in rows; nothing is timed), "index" (every
    /// cluster the support table leaves open on the index path), "flat" (decided from full vectors) or "scan".
    /// `dlv`: the deterministic levels' cumulative bytes, in place of the fit's (a build input, e.g. [16, 32]).
    /// `ddec`: the deterministic levels that decide in threshold queries, as a bit mask (None: all).
    #[new]
    #[pyo3(signature = (dir, core=None, pseudo=1000, plan="auto", dlv=None, ddec=None))]
    fn new(py: Python<'_>, dir: String, core: Option<usize>, pseudo: usize, plan: &str, dlv: Option<Vec<usize>>, ddec: Option<u32>) -> PyResult<Index> {
        let mode = match plan {
            "auto" => PlanMode::Auto,
            "index" => PlanMode::Index,
            "flat" => PlanMode::Flat,
            "scan" => PlanMode::Scan,
            _ => return Err(PyValueError::new_err(format!("plan is auto, index, flat or scan, not {plan:?}"))),
        };
        py.detach(|| {
            let o = BuildOptions { dlv, ..BuildOptions::default() };
            let mut log = |s: String| eprintln!("{s}");
            let (p, ix) = match open(&dir, &o, &mut log) {
                Ok(x) => x,
                Err(_) => rebuild(&dir, &o, &mut log).map_err(PyValueError::new_err)?,
            };
            let plan = Plan { mode, ddec, ..Plan::default() };
            plan.check(&ix).map_err(PyValueError::new_err)?;
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
