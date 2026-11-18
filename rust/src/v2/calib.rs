//! The approximate process's calibration (`process`), fitted by the build on corpus pseudo-queries (never on real
//! queries) and saved with the index:
//!
//! - the exceedance table of a cluster: ln P(tau), the share of its rows scoring at least tau, as a non-increasing
//!   function g of the standardised gap zeta = (tau - q.m) / s(q) plus beta ln(rad / median rad), for each of two
//!   spread statistics s (`Tier`): the cluster's group's residual covariance scaled by the cluster's radius, and the
//!   cluster's own covariance (the order state's directions, `order::OrderState`). A second pair of tables, fitted on
//!   the plain pseudo-queries alone, is kept as the control for the shifted ones;
//! - each group's residual covariance: 16 principal directions (int8 with a scale each), their variances and the
//!   group's RMS residual;
//! - the margin within which the group tier's ranking of a cluster can differ from its own tier's (`Calib::delta`);
//! - the price dial's table from mean recall to price, per k band and per answer-size band (`Band`).
//!
//! Pseudo-queries are corpus rows spread over the index, each used plain and shifted: q' = normalise(a q + v), v a
//! unit vector in the directions in which the centres do not vary (the least-variance directions of the centres
//! holding `SHIFT_SHARE` of their variance) and a in `SHIFT_SCALES`. A pseudo-query's own row is left out of its
//! counts. Half of them fit the tables, the other half the price dial's table, the margin and the build's report.
//!
//! The rows' scores are exact (f32 products, and the exact integer product of the query rounded to f16 wherever the
//! f32 product cannot decide), and every sum over pseudo-queries, clusters or rows runs in a fixed order with separate
//! multiplies and adds, so the calibration is the same bits whatever the instruction set and the thread count, and a
//! build and a rebuild store the same.

use super::process::{plan_at, solve_tau, Act, Blocks, Costs1};
use super::q::{full_err_qn, query_f16};
use super::*;
use rand::SeedableRng;
use rand_distr::{Distribution, StandardNormal};

/// Values of the exceedance table's grid of zeta.
pub const NG: usize = 65;
/// The fewest rows an index needs for the build to fit a calibration (none below: the approximate process refuses it).
pub const MIN_ROWS: usize = 64;
/// ln P is never below this, so that every cluster holding a visible row keeps a positive value.
pub const LN_P_MIN: f64 = -700.0;
/// Corpus rows used as pseudo-queries (each plain and shifted).
pub const NQ_ROWS: usize = 320;
/// The shifted pseudo-queries' weights of the corpus row against the shift.
pub const SHIFT_SCALES: [f64; 3] = [1.0, 0.5, 0.25];
/// The shift's directions: the centres' least-variance directions holding this share of their variance (at least one).
pub const SHIFT_SHARE: f64 = 1e-3;
/// Thresholds per pseudo-query, as shares of the rows above them.
pub const FRACTIONS: [f64; 6] = [1e-6, 1e-5, 1e-4, 1e-3, 1e-2, 1e-1];
/// k of the price dial's k-NN bands.
pub const KNN_BANDS: [usize; 4] = [1, 10, 100, 1000];
/// Answer shares of the price dial's threshold bands.
pub const THR_BANDS: [f64; 4] = [1e-4, 1e-3, 1e-2, 1e-1];
/// The price dial's grid of prices (recall per microsecond): 10^(1 - i/4), i = 0..LAM_STEPS.
pub const LAM_STEPS: usize = 45;
/// The saved index's files of the group covariances: RMS residuals, variances, scales and directions.
pub const FILES: [&str; 4] = ["calib_grad.bin", "calib_glam.bin", "calib_gdsc.bin", "calib_gdirs.bin"];

/// The spread statistic of a cluster's standardised gap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    /// s_c(q) = sigma_g(q) rad_c / rad_g: the cluster's group's residual covariance, scaled by the cluster's radius.
    Group = 0,
    /// s_c(q) = sigma_c(q): the cluster's own covariance (16 directions and an isotropic rest).
    Own = 1,
}

/// One exceedance table: ln P(zeta, lr) = min(g(zeta) + beta lr, 0), g piecewise linear on the grid lo + (i + 1/2)
/// step (i < NG), constant below it and continued at `slope` (at most 0) above it; never below `LN_P_MIN`.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub lo: f64,
    pub step: f64,
    pub g: Vec<f64>,
    pub beta: f64,
    pub slope: f64,
}

impl Table {
    /// ln P at standardised gap `zeta` for a cluster of log relative radius `lr`; non-increasing in zeta.
    #[inline]
    pub fn ln_p(&self, zeta: f64, lr: f64) -> f64 {
        let x = (zeta - self.lo) / self.step - 0.5;
        let last = (NG - 1) as f64;
        let v = if x >= last {
            self.g[NG - 1] + self.slope * (x - last) * self.step
        } else if x > 0.0 {
            let i = x as usize;
            let f = x - i as f64;
            self.g[i] + f * (self.g[i + 1] - self.g[i])
        } else {
            self.g[0]
        };
        (v + self.beta * lr).clamp(LN_P_MIN, 0.0)
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({ "lo": self.lo, "step": self.step, "g": self.g, "beta": self.beta, "slope": self.slope })
    }

    fn from_json(v: &serde_json::Value) -> Option<Table> {
        let g: Vec<f64> = v["g"].as_array()?.iter().map(|x| x.as_f64()).collect::<Option<_>>()?;
        (g.len() == NG).then_some(())?;
        Some(Table { lo: v["lo"].as_f64()?, step: v["step"].as_f64()?, g, beta: v["beta"].as_f64()?, slope: v["slope"].as_f64()? })
    }
}

/// One band of the price dial's table: mean recall on the held-out pseudo-queries against ln price, along the price
/// grid (prices falling, recall rising), for k-NN at k = `size` or threshold queries answered by a share `size` of
/// the rows.
#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    pub knn: bool,
    pub size: f64,
    pub ln_lam: Vec<f64>,
    pub recall: Vec<f64>,
}

impl Band {
    /// The price whose mean recall on the pseudo-queries is `r` (log-linear between the grid's prices); the grid's
    /// lowest price when no price reaches it.
    pub fn price(&self, r: f64) -> f64 {
        let i = self.recall.iter().position(|&x| x >= r).unwrap_or(self.recall.len() - 1);
        if i == 0 || self.recall[i] < r {
            return self.ln_lam[i].exp();
        }
        let (r0, r1) = (self.recall[i - 1], self.recall[i]);
        let f = if r1 > r0 { (r - r0) / (r1 - r0) } else { 1.0 };
        (self.ln_lam[i - 1] + f * (self.ln_lam[i] - self.ln_lam[i - 1])).exp()
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({ "knn": self.knn, "size": self.size, "ln_lam": self.ln_lam, "recall": self.recall })
    }

    fn from_json(v: &serde_json::Value) -> Option<Band> {
        let f = |k: &str| -> Option<Vec<f64>> { v[k].as_array()?.iter().map(|x| x.as_f64()).collect() };
        let (ln_lam, recall) = (f("ln_lam")?, f("recall")?);
        (ln_lam.len() == recall.len() && !ln_lam.is_empty()).then_some(())?;
        Some(Band { knn: v["knn"].as_bool()?, size: v["size"].as_f64()?, ln_lam, recall })
    }
}

/// Each group's residual covariance (its rows about their own clusters' centres): its first `r` principal
/// directions as int8 rows with a scale each, their variances, and its RMS residual (`rad`).
pub struct GroupCov {
    pub r: usize,
    /// The dimensions the data spans (`order::spanned`).
    pub n0: usize,
    pub rad: Arr<f64>,
    pub lam: Arr<f32>,
    pub dsc: Arr<f32>,
    pub dirs8: Arr<i8>,
}

impl GroupCov {
    /// The variance left after group g's r directions, per other spanned dimension.
    pub fn rest(&self, g: usize) -> f64 {
        let top: f64 = self.lam[g * self.r..(g + 1) * self.r].iter().map(|&l| l as f64).sum();
        (self.rad[g] * self.rad[g] - top).max(0.0) / (self.n0.max(self.r + 1) - self.r) as f64
    }
}

/// The calibration as stored.
pub struct Calib {
    /// Per tier, fitted on plain and shifted pseudo-queries.
    pub tables: [Table; 2],
    /// Per tier, fitted on the plain pseudo-queries alone (the control).
    pub plain: [Table; 2],
    /// The 95% quantile, over clusters weighted by the answers they hold, of |ln| of the ratio of a cluster's share
    /// of the answer under the two tiers: where the group tier's value is within this of the plan's margin, the
    /// own tier can change the plan.
    pub delta: f64,
    /// Per tier, the price dial's bands.
    pub bands: [Vec<Band>; 2],
    pub gcov: GroupCov,
    /// The median of the clusters' radii (`order::OrderState::rad`).
    pub med_rad: f64,
    /// The build's checks of the tables on the held-out pseudo-queries (not read by queries).
    pub report: serde_json::Value,
}

impl Calib {
    /// The table of `tier` (the control's with `plain`).
    pub fn table(&self, tier: Tier, plain: bool) -> &Table {
        if plain { &self.plain[tier as usize] } else { &self.tables[tier as usize] }
    }

    /// The price at mean recall `r` for a k-NN query of k `kk` (`knn`) or a threshold query answered by a share
    /// `size` of the rows, from the band nearest in log size.
    pub fn price(&self, tier: Tier, knn: bool, size: f64, r: f64) -> f64 {
        let bands = &self.bands[tier as usize];
        let near = bands.iter().filter(|b| b.knn == knn).min_by(|a, b| (a.size.ln() - size.max(1e-12).ln()).abs().total_cmp(&(b.size.ln() - size.max(1e-12).ln()).abs()));
        near.map_or(0.0, |b| b.price(r))
    }

    /// Everything but the group covariances' arrays, for index.json.
    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "tables": self.tables.iter().map(|t| t.json()).collect::<Vec<_>>(),
            "plain": self.plain.iter().map(|t| t.json()).collect::<Vec<_>>(),
            "delta": self.delta, "med_rad": self.med_rad,
            "bands": self.bands.iter().map(|b| b.iter().map(|x| x.json()).collect::<Vec<_>>()).collect::<Vec<_>>(),
            "gcov": { "r": self.gcov.r, "n0": self.gcov.n0 },
            "report": self.report,
        })
    }

    /// The calibration from index.json's `v` and the group covariances' arrays; None when `v` is not one.
    pub fn from_json(v: &serde_json::Value, rad: Arr<f64>, lam: Arr<f32>, dsc: Arr<f32>, dirs8: Arr<i8>) -> Option<Calib> {
        let tabs = |k: &str| -> Option<[Table; 2]> {
            let a = v[k].as_array()?;
            (a.len() == 2).then_some(())?;
            Some([Table::from_json(&a[0])?, Table::from_json(&a[1])?])
        };
        let b = v["bands"].as_array()?;
        (b.len() == 2).then_some(())?;
        let band = |x: &serde_json::Value| -> Option<Vec<Band>> { x.as_array()?.iter().map(Band::from_json).collect() };
        let gcov = GroupCov { r: v["gcov"]["r"].as_u64()? as usize, n0: v["gcov"]["n0"].as_u64()? as usize, rad, lam, dsc, dirs8 };
        Some(Calib { tables: tabs("tables")?, plain: tabs("plain")?, delta: v["delta"].as_f64()?, bands: [band(&b[0])?, band(&b[1])?], gcov, med_rad: v["med_rad"].as_f64()?, report: v["report"].clone() })
    }

    /// Hashes everything stored (`Index2::store_hash`).
    pub fn hash(&self, h: &mut store::Fx) {
        let mut t = |x: &Table| {
            h.pod(&[x.lo, x.step, x.beta, x.slope]);
            h.pod(&x.g);
        };
        self.tables.iter().chain(&self.plain).for_each(&mut t);
        h.pod(&[self.delta, self.med_rad]);
        for b in self.bands.iter().flatten() {
            h.usizes(&[b.knn as usize, b.ln_lam.len()]);
            h.pod(&[b.size]);
            h.pod(&b.ln_lam);
            h.pod(&b.recall);
        }
        h.usizes(&[self.gcov.r, self.gcov.n0]);
        h.pod(&self.gcov.rad[..]);
        h.pod(&self.gcov.lam[..]);
        h.pod(&self.gcov.dsc[..]);
        h.big_pod(&self.gcov.dirs8[..]);
    }

    /// Each cluster's ln(rad / median rad), from the order state's radii.
    pub fn lr(&self, ix: &Index2) -> Vec<f64> {
        let st = ix.order.as_ref().expect("the calibration needs the order state's radii");
        st.rad.iter().map(|&r| (r.max(1e-12 * self.med_rad) / self.med_rad).ln()).collect()
    }
}

/// sigma_g(q) of group g, in f64 from the stored int8 directions (the build's features).
pub fn group_sigma_f64(gc: &GroupCov, g: usize, q: &[f64], qq: f64, n: usize) -> f64 {
    let r = gc.r;
    let (mut v, mut pp) = (0f64, 0f64);
    for j in 0..r {
        let d = &gc.dirs8[(g * r + j) * n..(g * r + j + 1) * n];
        let x = d.iter().zip(q).fold(0f64, |a, (&di, &qi)| a + di as f64 * qi) * gc.dsc[g * r + j] as f64;
        v += gc.lam[g * r + j] as f64 * x * x;
        pp += x * x;
    }
    (v + gc.rest(g) * (qq - pp).max(0.0)).max(0.0).sqrt()
}

/// sigma_c(q) of cluster c from the order state's directions, in f64 (the build's features).
pub fn own_sigma_f64(st: &order::OrderState, c: usize, q: &[f64], qq: f64, n: usize) -> f64 {
    let r = st.r;
    let (mut v, mut pp, mut top) = (0f64, 0f64, 0f64);
    for j in 0..r {
        let d = &st.dirs8[(c * r + j) * n..(c * r + j + 1) * n];
        let x = d.iter().zip(q).fold(0f64, |a, (&di, &qi)| a + di as f64 * qi) * st.dsc[c * r + j] as f64;
        let l = st.lam[c * r + j] as f64;
        v += l * x * x;
        pp += x * x;
        top += l;
    }
    let rest = (st.rad[c] * st.rad[c] - top).max(0.0) / (st.n0.max(r + 1) - r) as f64;
    (v + rest * (qq - pp).max(0.0)).max(0.0).sqrt()
}

/// Group covariances from every row of each group about its own cluster's centre: the directions from up to
/// `order::SAMPLE` rows spread over the group's rows, as `order::cluster` finds a cluster's.
fn group_cov(ix: &Index2) -> GroupCov {
    let (n, k, ng) = (ix.n(), ix.k(), ix.groups.len());
    let r = order::r_of(n);
    let zero = vec![0f32; n];
    let parts: Vec<order::ClusterOrder> = (0..ng).into_par_iter().map(|g| {
        let cls: Vec<usize> = (0..k).filter(|&c| ix.cl[c].g == g && ix.cl[c].r1 > ix.cl[c].r0).collect();
        let mut start = vec![0usize];
        for &c in &cls {
            start.push(start.last().unwrap() + ix.cl[c].r1 - ix.cl[c].r0);
        }
        let row = |i: usize, out: &mut [f32]| {
            let j = start.partition_point(|&s| s <= i) - 1;
            let c = cls[j];
            let x = ix.full_row_f16(ix.cl[c].r0 + i - start[j]);
            for ((o, &h), &m) in out.iter_mut().zip(x).zip(ix.centre(c)) {
                *o = f16_to_f32(h) - m;
            }
        };
        order::cluster(*start.last().unwrap(), &row, &zero, r, order::SAMPLE, order::PASSES, g as u64)
    }).collect();
    let (mut rad, mut lam, mut dsc, mut dirs8) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for p in parts {
        rad.push(p.rad);
        lam.extend(p.lam);
        dsc.extend(p.dsc);
        dirs8.extend(p.dirs8);
    }
    GroupCov { r, n0: order::spanned(&ix.centres32, n), rad: rad.into(), lam: lam.into(), dsc: dsc.into(), dirs8: dirs8.into() }
}

/// Unit vectors (length n) spanning the directions in which the centres vary least: the eigenvectors of their
/// covariance over the spanned dimensions, from the smallest eigenvalue up, while their sum stays within
/// `SHIFT_SHARE` of the trace (at least one).
fn shift_basis(ix: &Index2) -> Vec<Vec<f64>> {
    let (n, k) = (ix.n(), ix.k());
    let dims: Vec<usize> = (0..n).filter(|&i| (0..k).any(|c| ix.centres32[c * n + i] != 0.0)).collect();
    let d = dims.len();
    if d == 0 || k == 0 {
        return Vec::new();
    }
    let mut mean = vec![0f64; d];
    for c in 0..k {
        for (m, &i) in mean.iter_mut().zip(&dims) {
            *m += ix.centres32[c * n + i] as f64;
        }
    }
    mean.iter_mut().for_each(|m| *m /= k as f64);
    let mut cov = vec![0f64; d * d];
    let mut x = vec![0f64; d];
    for c in 0..k {
        for (j, &i) in dims.iter().enumerate() {
            x[j] = ix.centres32[c * n + i] as f64 - mean[j];
        }
        for a in 0..d {
            let xa = x[a];
            for (o, &xb) in cov[a * d..(a + 1) * d].iter_mut().zip(&x) {
                *o += xa * xb;
            }
        }
    }
    cov.iter_mut().for_each(|v| *v /= k as f64);
    let (vals, vecs) = sym_eigen(&cov, d);
    let trace: f64 = vals.iter().map(|v| v.max(0.0)).sum();
    let mut out = Vec::new();
    let mut held = 0.0;
    for (&val, vec) in vals.iter().zip(&vecs) {
        let v = val.max(0.0);
        if !out.is_empty() && held + v > SHIFT_SHARE * trace {
            break;
        }
        held += v;
        let mut u = vec![0f64; n];
        for (a, &i) in dims.iter().enumerate() {
            u[i] = vec[a];
        }
        out.push(u);
    }
    out
}

/// The eigenvalues of the symmetric d x d matrix `a` (row-major), smallest first, with their unit eigenvectors: by
/// Householder reduction to tridiagonal form and the implicit QL iteration (as EISPACK's tred2 and tql2), in f64 with
/// every loop in a fixed order.
fn sym_eigen(a: &[f64], d: usize) -> (Vec<f64>, Vec<Vec<f64>>) {
    if d == 0 {
        return (Vec::new(), Vec::new());
    }
    let mut v: Vec<Vec<f64>> = (0..d).map(|i| a[i * d..(i + 1) * d].to_vec()).collect();
    let (mut dg, mut e) = (vec![0f64; d], vec![0f64; d]);
    // Householder reduction: v becomes the orthogonal transformation, dg and e the tridiagonal matrix
    dg.copy_from_slice(&v[d - 1]);
    for i in (1..d).rev() {
        let scale: f64 = dg[..i].iter().map(|x| x.abs()).sum();
        let mut h = 0.0;
        if scale == 0.0 {
            e[i] = dg[i - 1];
            for j in 0..i {
                dg[j] = v[i - 1][j];
                v[i][j] = 0.0;
                v[j][i] = 0.0;
            }
        } else {
            for x in dg[..i].iter_mut() {
                *x /= scale;
                h += *x * *x;
            }
            let mut f = dg[i - 1];
            let mut g = if f > 0.0 { -h.sqrt() } else { h.sqrt() };
            e[i] = scale * g;
            h -= f * g;
            dg[i - 1] = f - g;
            e[..i].fill(0.0);
            for j in 0..i {
                f = dg[j];
                v[j][i] = f;
                g = e[j] + v[j][j] * f;
                for k in j + 1..i {
                    g += v[k][j] * dg[k];
                    e[k] += v[k][j] * f;
                }
                e[j] = g;
            }
            f = 0.0;
            for j in 0..i {
                e[j] /= h;
                f += e[j] * dg[j];
            }
            let hh = f / (h + h);
            for j in 0..i {
                e[j] -= hh * dg[j];
            }
            for j in 0..i {
                f = dg[j];
                g = e[j];
                for k in j..i {
                    v[k][j] -= f * e[k] + g * dg[k];
                }
                dg[j] = v[i - 1][j];
                v[i][j] = 0.0;
            }
        }
        dg[i] = h;
    }
    for i in 0..d - 1 {
        v[d - 1][i] = v[i][i];
        v[i][i] = 1.0;
        let h = dg[i + 1];
        if h != 0.0 {
            for k in 0..=i {
                dg[k] = v[k][i + 1] / h;
            }
            for j in 0..=i {
                let mut g = 0.0;
                for k in 0..=i {
                    g += v[k][i + 1] * v[k][j];
                }
                for k in 0..=i {
                    v[k][j] -= g * dg[k];
                }
            }
        }
        for row in v.iter_mut().take(i + 1) {
            row[i + 1] = 0.0;
        }
    }
    for j in 0..d {
        dg[j] = v[d - 1][j];
        v[d - 1][j] = 0.0;
    }
    v[d - 1][d - 1] = 1.0;
    e[0] = 0.0;
    // the QL iteration on the tridiagonal matrix, its rotations applied to the columns of v (held as the rows of vt)
    let mut vt: Vec<Vec<f64>> = (0..d).map(|j| (0..d).map(|k| v[k][j]).collect()).collect();
    drop(v);
    for i in 1..d {
        e[i - 1] = e[i];
    }
    e[d - 1] = 0.0;
    let (mut f, mut tst1) = (0f64, 0f64);
    let eps = f64::EPSILON;
    for l in 0..d {
        tst1 = tst1.max(dg[l].abs() + e[l].abs());
        let mut m = l;
        while m < d - 1 && e[m].abs() > eps * tst1 {
            m += 1;
        }
        if m > l {
            for _ in 0..64 {
                let mut g = dg[l];
                let mut p = (dg[l + 1] - g) / (2.0 * e[l]);
                let mut r = p.hypot(1.0);
                if p < 0.0 {
                    r = -r;
                }
                dg[l] = e[l] / (p + r);
                dg[l + 1] = e[l] * (p + r);
                let dl1 = dg[l + 1];
                let mut h = g - dg[l];
                for x in dg[l + 2..].iter_mut() {
                    *x -= h;
                }
                f += h;
                p = dg[m];
                let (mut c, mut c2, mut c3) = (1.0, 1.0, 1.0);
                let el1 = e[l + 1];
                let (mut s, mut s2) = (0.0, 0.0);
                for i in (l..m).rev() {
                    c3 = c2;
                    c2 = c;
                    s2 = s;
                    g = c * e[i];
                    h = c * p;
                    r = p.hypot(e[i]);
                    e[i + 1] = s * r;
                    s = e[i] / r;
                    c = p / r;
                    p = c * dg[i] - s * g;
                    dg[i + 1] = h + s * (c * g + s * dg[i]);
                    let (lo, hi) = vt.split_at_mut(i + 1);
                    for (x, y) in lo[i].iter_mut().zip(hi[0].iter_mut()) {
                        let t = *y;
                        *y = s * *x + c * t;
                        *x = c * *x - s * t;
                    }
                }
                p = -s * s2 * c3 * el1 * e[l] / dl1;
                e[l] = s * p;
                dg[l] = c * p;
                if e[l].abs() <= eps * tst1 {
                    break;
                }
            }
        }
        dg[l] += f;
        e[l] = 0.0;
    }
    let mut o: Vec<usize> = (0..d).collect();
    o.sort_by(|&a, &b| dg[a].total_cmp(&dg[b]).then(a.cmp(&b)));
    (o.iter().map(|&i| dg[i]).collect(), o.iter().map(|&i| vt[i].clone()).collect())
}

/// One pseudo-query's thresholds, counts and features.
struct Pq {
    shifted: bool,
    /// Fits the tables (else held out).
    fit: bool,
    own: usize,
    /// Per rank: tau, the visible rows at or above it (weighted), and their count per cluster.
    tau: Vec<f64>,
    tot: Vec<f64>,
    cnt: Vec<Vec<(u32, f64)>>,
    mu: Vec<f64>,
    s: [Vec<f64>; 2],
}

/// The ranks (rows above the threshold) the pseudo-queries are counted at: the shares of `FRACTIONS` and the bands'
/// k and shares of the rows, at most half of them.
fn ranks(nrows: usize) -> Vec<usize> {
    let of = |f: f64| ((f * nrows as f64).round() as usize).max(1);
    let mut r: Vec<usize> = FRACTIONS.iter().chain(&THR_BANDS).map(|&f| of(f)).chain(KNN_BANDS).filter(|&x| x <= nrows / 2).collect();
    r.sort_unstable();
    r.dedup();
    r
}

/// The calibration of `ix` (None without stored vectors and the order state, or with fewer than `MIN_ROWS` rows).
pub fn fit(ix: &Index2, log: &mut dyn FnMut(String)) -> Option<Calib> {
    let st = ix.order.as_ref()?;
    if !matches!(ix.full, Full2::Real { .. }) || ix.nrows < MIN_ROWS || ix.k() == 0 {
        return None;
    }
    let t = Timer::new();
    let (n, k, nrows) = (ix.n(), ix.k(), ix.nrows);
    let gcov = group_cov(ix);
    let t_cov = t.s();
    let basis = shift_basis(ix);
    let t_basis = t.s() - t_cov;
    let mut rads: Vec<f64> = (0..k).filter(|&c| ix.cl[c].r1 > ix.cl[c].r0).map(|c| st.rad[c]).collect();
    rads.sort_by(|a, b| a.total_cmp(b));
    let med_rad = rads.get(rads.len() / 2).copied().unwrap_or(1.0).max(1e-12);
    let lr: Vec<f64> = st.rad.iter().map(|&r| (r.max(1e-12 * med_rad) / med_rad).ln()).collect();
    let size: Vec<f64> = ix.cl.iter().map(|c| (c.r1 - c.r0) as f64).collect();
    let rk = ranks(nrows);
    // the rows scored: all of a cluster up to order::SAMPLE, else that many spread over it, each standing for its share
    let (mut srow, mut sw, mut scl) = (Vec::new(), Vec::new(), Vec::new());
    for (c, cl) in ix.cl.iter().enumerate() {
        let m = cl.r1 - cl.r0;
        let take = spread(m, order::SAMPLE);
        let w = m as f64 / take.len().max(1) as f64;
        for i in take {
            srow.push((cl.r0 + i) as u32);
            sw.push(w);
            scl.push(c as u32);
        }
    }
    let row_cl: Vec<u32> = ix.cl.iter().enumerate().flat_map(|(c, cl)| std::iter::repeat_n(c as u32, cl.r1 - cl.r0)).collect();
    // the pseudo-queries: rows spread over the index, each plain and shifted
    let rows: Vec<usize> = spread(nrows, NQ_ROWS);
    let mut qs: Vec<(Vec<f32>, usize, bool, bool)> = Vec::new();
    for (i, &row) in rows.iter().enumerate() {
        let x: Vec<f64> = ix.full_row_f16(row).iter().map(|&h| f16_to_f32(h) as f64).collect();
        let xn = x.iter().map(|v| v * v).sum::<f64>().sqrt();
        if xn < 0.5 {
            continue;
        }
        let unit = |v: &[f64]| -> Vec<f32> {
            let s = v.iter().map(|a| a * a).sum::<f64>().sqrt();
            v.iter().map(|a| (a / s) as f32).collect()
        };
        let plain: Vec<f64> = x.iter().map(|v| v / xn).collect();
        qs.push((unit(&plain), row, false, i % 2 == 0));
        if !basis.is_empty() {
            let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0x5eed_0000 + i as u64);
            let mut v = vec![0f64; n];
            for b in &basis {
                let c: f64 = StandardNormal.sample(&mut rng);
                v.iter_mut().zip(b).for_each(|(o, &bi)| *o += c * bi);
            }
            let vn = v.iter().map(|a| a * a).sum::<f64>().sqrt().max(1e-300);
            let a = SHIFT_SCALES[i % SHIFT_SCALES.len()];
            let sh: Vec<f64> = plain.iter().zip(&v).map(|(p, vi)| a * p + vi / vn).collect();
            qs.push((unit(&sh), row, true, i % 2 == 0));
        }
    }
    let wmax = *rk.last()? as f64;
    let pqs: Vec<Pq> = qs.par_iter().map(|(q, own, shifted, fitq)| {
        let (q16, q32, qn) = query_f16(q);
        let err = full_err_qn(ix, qn);
        let mut sc = vec![0f32; srow.len()];
        let mut ch = srow.chunks_exact(4).zip(sc.chunks_exact_mut(4));
        for (r, o) in &mut ch {
            o.copy_from_slice(&unsafe { crate::kernel::dot_f16_x4(&q32, [0, 1, 2, 3].map(|i| ix.full_row_f16(r[i] as usize))) });
        }
        let done = srow.len() / 4 * 4;
        for i in done..srow.len() {
            sc[i] = unsafe { crate::kernel::dot_f16(&q32, ix.full_row_f16(srow[i] as usize)) };
        }
        if let Ok(i) = srow.binary_search(&(*own as u32)) {
            sc[i] = f32::NEG_INFINITY;
        }
        // the rows that can be among the wmax best (weights are at least 1): every row within 2 err of the f32 cut,
        // then their exact scores
        let mtop = (wmax as usize).min(srow.len() - 1).max(1);
        let mut tmp = sc.clone();
        tmp.select_nth_unstable_by(mtop - 1, |a, b| b.total_cmp(a));
        let cut = tmp[mtop - 1] as f64 - 2.0 * err;
        let mut cand: Vec<(f64, usize)> = (0..srow.len()).filter(|&i| sc[i] > f32::NEG_INFINITY && sc[i] as f64 >= cut)
            .map(|i| (exact_to_f64(exact_dot(&q16, ix.full_row_f16(srow[i] as usize))), i)).collect();
        cand.sort_by(|a, b| b.0.total_cmp(&a.0).then(srow[a.1].cmp(&srow[b.1])));
        let mut cum = Vec::with_capacity(cand.len());
        let mut acc = 0f64;
        for &(_, i) in &cand {
            acc += sw[i];
            cum.push(acc);
        }
        let (mut tau, mut tot, mut cnt) = (Vec::new(), Vec::new(), Vec::new());
        let mut dense = vec![0f64; k];
        let mut touched: Vec<u32> = Vec::new();
        let mut end = 0;
        for &r in &rk {
            let p = cum.partition_point(|&c| c < r as f64).min(cand.len() - 1);
            let tv = cand[p].0;
            let e = cand.partition_point(|x| x.0 >= tv);
            while end < e {
                let i = cand[end].1;
                let c = scl[i] as usize;
                if dense[c] == 0.0 {
                    touched.push(c as u32);
                }
                dense[c] += sw[i];
                end += 1;
            }
            tau.push(tv);
            tot.push(cum[e - 1]);
            let mut sp: Vec<(u32, f64)> = touched.iter().map(|&c| (c, dense[c as usize])).collect();
            sp.sort_unstable_by_key(|x| x.0);
            cnt.push(sp);
        }
        let q64: Vec<f64> = q32.iter().map(|&x| x as f64).collect();
        let qq: f64 = q64.iter().map(|x| x * x).sum();
        let mu: Vec<f64> = (0..k).map(|c| ix.qm(&q64, c)).collect();
        let sg: Vec<f64> = (0..ix.groups.len()).map(|g| group_sigma_f64(&gcov, g, &q64, qq, n)).collect();
        let s1: Vec<f64> = (0..k).map(|c| {
            let g = ix.cl[c].g;
            (sg[g] * st.rad[c] / gcov.rad[g].max(1e-300)).max(1e-12)
        }).collect();
        let s2: Vec<f64> = (0..k).map(|c| own_sigma_f64(st, c, &q64, qq, n).max(1e-12)).collect();
        Pq { shifted: *shifted, fit: *fitq, own: row_cl[*own] as usize, tau, tot, cnt, mu, s: [s1, s2] }
    }).collect();
    let t_pq = t.s() - t_cov - t_basis;
    // (query, rank, cluster) triples of a set of pseudo-queries for a tier: zeta, cluster, exposure, count
    let triples = |sel: &dyn Fn(&Pq) -> bool, tier: usize| -> (Vec<f64>, Vec<u32>, Vec<f64>, Vec<f64>) {
        let (mut z, mut cs, mut ex, mut nn) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut dense = vec![0f64; k];
        for p in pqs.iter().filter(|p| sel(p)) {
            for (j, &tv) in p.tau.iter().enumerate() {
                for &(c, v) in &p.cnt[j] {
                    dense[c as usize] = v;
                }
                for c in 0..k {
                    let e = size[c] - (c == p.own) as usize as f64;
                    if e > 0.0 {
                        z.push((tv - p.mu[c]) / p.s[tier][c]);
                        cs.push(c as u32);
                        ex.push(e);
                        nn.push(dense[c]);
                    }
                }
                for &(c, _) in &p.cnt[j] {
                    dense[c as usize] = 0.0;
                }
            }
        }
        (z, cs, ex, nn)
    };
    let fit_t = |sel: &dyn Fn(&Pq) -> bool, tier: usize| -> Table {
        let (z, cs, ex, nn) = triples(sel, tier);
        fit_table(&z, &cs, &ex, &nn, &lr, k)
    };
    let tables = [fit_t(&|p: &Pq| p.fit, 0), fit_t(&|p: &Pq| p.fit, 1)];
    let plain = [fit_t(&|p: &Pq| p.fit && !p.shifted, 0), fit_t(&|p: &Pq| p.fit && !p.shifted, 1)];
    // the held-out half: reliability (delta) and the per-query count factor, per tier and kind of pseudo-query
    let mut report = serde_json::Map::new();
    let k10 = rk.iter().enumerate().min_by(|a, b| (*a.1 as f64 / 10.0).ln().abs().total_cmp(&(*b.1 as f64 / 10.0).ln().abs())).map(|x| x.0).unwrap_or(0);
    for (tier, name) in [(0usize, "group"), (1, "own")] {
        let mut o = serde_json::Map::new();
        for (kind, shifted) in [("plain", false), ("shifted", true)] {
            let sel = |p: &Pq| !p.fit && p.shifted == shifted;
            let (z, cs, ex, nn) = triples(&sel, tier);
            let d = reliability(&tables[tier], &z, &cs, &ex, &nn, &lr);
            let mut kap: Vec<f64> = pqs.iter().filter(|p| sel(p)).map(|p| {
                let m: f64 = (0..k).map(|c| (size[c] - (c == p.own) as usize as f64) * tables[tier].ln_p((p.tau[k10] - p.mu[c]) / p.s[tier][c], lr[c]).exp()).sum();
                (m.max(1e-300) / p.tot[k10]).ln()
            }).collect();
            kap.sort_by(|a, b| a.total_cmp(b));
            let qf = |f: f64| kap.get(((kap.len() as f64 - 1.0) * f).round() as usize).copied().unwrap_or(f64::NAN);
            o.insert(kind.into(), serde_json::json!({ "delta": d.0, "ln_obs_pred": d.1, "ln_kappa_rank": rk[k10], "ln_kappa_10_50_90": [qf(0.1), qf(0.5), qf(0.9)] }));
        }
        report.insert(name.into(), o.into());
    }
    // the margin between the tiers' shares, over the held-out pseudo-queries' clusters weighted by their answers
    let mut ds: Vec<(f64, f64)> = Vec::new();
    for p in pqs.iter().filter(|p| !p.fit) {
        for (j, &tv) in p.tau.iter().enumerate() {
            let w = |tier: usize| -> Vec<f64> { (0..k).map(|c| size[c] * tables[tier].ln_p((tv - p.mu[c]) / p.s[tier][c], lr[c]).exp()).collect() };
            let (w1, w2) = (w(0), w(1));
            let (s1, s2): (f64, f64) = (w1.iter().sum(), w2.iter().sum());
            for &(c, cnt) in &p.cnt[j] {
                let c = c as usize;
                ds.push((((w2[c] / s2).ln() - (w1[c] / s1).ln()).abs(), cnt));
            }
        }
    }
    let delta = wquantile(&mut ds, 0.95);
    // the price dial: per tier and band, mean recall on the held-out pseudo-queries along the price grid
    let cm = Costs1::for_dim(n);
    let bands: [Vec<Band>; 2] = [0, 1].map(|tier| {
        let mut out = Vec::new();
        let specs: Vec<(bool, f64, usize)> = KNN_BANDS.iter().filter_map(|&kk| rk.iter().position(|&r| r == kk).map(|j| (true, kk as f64, j)))
            .chain(THR_BANDS.iter().filter_map(|&f| {
                let r = ((f * nrows as f64).round() as usize).max(1);
                rk.iter().position(|&x| x == r).map(|j| (false, f, j))
            })).collect();
        for (knn, sz, j) in specs {
            let held: Vec<&Pq> = pqs.iter().filter(|p| !p.fit).collect();
            let curves: Vec<Vec<f64>> = held.par_iter().map(|p| {
                let units: Vec<usize> = (0..k).filter(|&c| size[c] - (c == p.own) as usize as f64 > 0.0).collect();
                let ex: Vec<f64> = units.iter().map(|&c| size[c] - (c == p.own) as usize as f64).collect();
                let lnp = |tv: f64, i: usize| tables[tier].ln_p((tv - p.mu[units[i]]) / p.s[tier][units[i]], lr[units[i]]);
                let tv = if knn {
                    let mass = |tv: f64| (0..units.len()).map(|i| ex[i] * lnp(tv, i).exp()).sum::<f64>();
                    let top = units.iter().map(|&c| p.mu[c]).fold(f64::NEG_INFINITY, f64::max);
                    let step = units.iter().map(|&c| p.s[tier][c]).fold(0.0, f64::max);
                    solve_tau(&mass, top, step, sz).0
                } else {
                    p.tau[j]
                };
                let w: Vec<f64> = (0..units.len()).map(|i| ex[i] * lnp(tv, i).exp()).collect();
                let norm = if knn { w.iter().sum::<f64>() } else { w.iter().sum::<f64>().max(1.0) };
                let mut cnt = vec![0f64; k];
                for &(c, v) in &p.cnt[j] {
                    cnt[c as usize] = v;
                }
                let vals: Vec<f64> = w.iter().map(|x| x / norm).collect();
                let blocks = Blocks::new(ix, &(0..units.len()).collect::<Vec<_>>(), &units, &vals, &ex, &cm);
                let mut act = Vec::new();
                (0..LAM_STEPS).map(|s| {
                    let lam = 10f64.powf(1.0 - s as f64 / 4.0);
                    plan_at(&blocks, lam, cm.grp, &mut act);
                    let got: f64 = blocks.units.iter().zip(&act).filter(|x| *x.1 != Act::Skip).map(|(&i, _)| cnt[units[i]]).sum();
                    (got / p.tot[j]).min(1.0)
                }).collect()
            }).collect();
            let m = curves.len().max(1) as f64;
            let recall: Vec<f64> = (0..LAM_STEPS).map(|s| curves.iter().map(|c| c[s]).sum::<f64>() / m).collect();
            let ln_lam: Vec<f64> = (0..LAM_STEPS).map(|s| (1.0 - s as f64 / 4.0) * std::f64::consts::LN_10).collect();
            out.push(Band { knn, size: sz, ln_lam, recall });
        }
        out
    });
    report.insert("pseudo_queries".into(), serde_json::json!({ "rows": rows.len(), "fit": pqs.iter().filter(|p| p.fit).count(), "held_out": pqs.iter().filter(|p| !p.fit).count(), "shift_directions": basis.len(), "ranks": rk }));
    log(format!("calibration in {:.1}s (group covariances {t_cov:.1}s, shift basis {t_basis:.1}s of {} directions, {} pseudo-queries {t_pq:.1}s): beta {:.3} / {:.3}, margin {delta:.3}; held out {}",
                t.s(), basis.len(), pqs.len(), tables[0].beta, tables[1].beta, serde_json::Value::Object(report.clone())));
    Some(Calib { tables, plain, delta, bands, gcov, med_rad, report: report.into() })
}

/// The `p` quantile of values weighted by their second element (0 when there are none).
fn wquantile(v: &mut [(f64, f64)], p: f64) -> f64 {
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    let tot: f64 = v.iter().map(|x| x.1).sum();
    let mut acc = 0.0;
    for x in v.iter() {
        acc += x.1;
        if acc >= p * tot {
            return x.0;
        }
    }
    v.last().map_or(0.0, |x| x.0)
}

/// The `p` quantile of `v` (linear between order statistics).
fn quantile(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let x = p * (v.len() - 1) as f64;
    let i = x as usize;
    let f = x - i as f64;
    if i + 1 < v.len() { v[i] + f * (v[i + 1] - v[i]) } else { v[i] }
}

/// Weighted isotonic regression, non-increasing (pool adjacent violators); a block of zero weight takes the larger
/// of the values it pools.
fn pava_dec(y: &[f64], w: &[f64]) -> Vec<f64> {
    let (mut vals, mut wts, mut cnt): (Vec<f64>, Vec<f64>, Vec<usize>) = (Vec::new(), Vec::new(), Vec::new());
    for (&yi, &wi) in y.iter().zip(w) {
        vals.push(-yi);
        wts.push(wi);
        cnt.push(1);
        while vals.len() > 1 && vals[vals.len() - 2] > vals[vals.len() - 1] {
            let (b, a) = (vals.pop().unwrap(), vals.pop().unwrap());
            let (wb, wa) = (wts.pop().unwrap(), wts.pop().unwrap());
            let (cb, ca) = (cnt.pop().unwrap(), cnt.pop().unwrap());
            let ww = wa + wb;
            vals.push(if ww > 0.0 { (a * wa + b * wb) / ww } else { a.max(b) });
            wts.push(ww);
            cnt.push(ca + cb);
        }
    }
    vals.iter().zip(&cnt).flat_map(|(&v, &c)| std::iter::repeat_n(-v, c)).collect()
}

/// The table fitted to triples (zeta, cluster, exposure, count) by Poisson regression of the count on
/// exposure exp(g(zeta) + beta lr): g a non-increasing function on NG values of zeta between the 5% and the 99.9%
/// quantiles of the zeta of triples holding answers (widened by 5% above; the first bin takes every zeta below, where
/// a unit holds most of its rows above tau, so that the bins resolve the fall of P and its tail), by weighted isotonic
/// regression of the bins' rates, alternated with Newton steps on beta; past the last bin holding answers, g continues
/// at the slope of the eight bins before it.
fn fit_table(z: &[f64], cs: &[u32], ex: &[f64], nn: &[f64], lr: &[f64], k: usize) -> Table {
    let mut zp: Vec<f64> = z.iter().zip(nn).filter(|x| *x.1 > 0.0).map(|x| *x.0).collect();
    if zp.is_empty() {
        zp = z.to_vec();
    }
    let (lo, hi) = (quantile(&mut zp, 0.05), quantile(&mut zp, 0.999));
    let hi = hi.max(lo + 1e-6);
    let hi = hi + 0.05 * (hi - lo);
    let step = (hi - lo) / NG as f64;
    let grid: Vec<f64> = (0..NG).map(|i| lo + (i as f64 + 0.5) * step).collect();
    // per bin: observed answers, and per cluster the exposure (so beta's updates are O(NG K))
    let mut obs = vec![0f64; NG];
    let mut a = vec![0f64; NG * k];
    let mut nlr = 0f64;
    for i in 0..z.len() {
        let b = (((z[i] - lo) / step).floor().max(0.0) as usize).min(NG - 1);
        obs[b] += nn[i];
        a[b * k + cs[i] as usize] += ex[i];
        nlr += nn[i] * lr[cs[i] as usize];
    }
    let mut beta = 0f64;
    let mut g = vec![0f64; NG];
    let expo = |beta: f64| -> Vec<f64> { (0..NG).map(|b| (0..k).map(|c| a[b * k + c] * (beta * lr[c]).exp()).sum()).collect() };
    for _ in 0..8 {
        let e = expo(beta);
        let rate: Vec<f64> = (0..NG).map(|b| if e[b] > 0.0 { obs[b] / e[b].max(1e-300) } else { 0.0 }).collect();
        g = pava_dec(&rate, &e).iter().map(|&r| r.max(1e-12).ln()).collect();
        for _ in 0..3 {
            let (mut m1, mut m2) = (0f64, 0f64);
            for b in 0..NG {
                let eg = g[b].exp();
                for c in 0..k {
                    let m = a[b * k + c] * (beta * lr[c]).exp() * eg;
                    m1 += m * lr[c];
                    m2 += m * lr[c] * lr[c];
                }
            }
            beta += ((nlr - m1) / m2.max(1e-30)).clamp(-1.0, 1.0);
        }
    }
    let mut slope = 0.0;
    if let Some(jl) = (0..NG).rev().find(|&b| obs[b] > 0.0) {
        let j0 = jl.saturating_sub(8);
        slope = ((g[jl] - g[j0]) / (grid[jl] - grid[j0]).max(1e-12)).min(0.0);
        for b in jl + 1..NG {
            g[b] = g[jl] + slope * (grid[b] - grid[jl]);
        }
    }
    Table { lo, step, g, beta, slope }
}

/// delta: the largest |ln(observed / predicted)| over bins of predicted P (thirds of a decade) holding 95% of the
/// answers; and ln(observed / predicted) over all.
fn reliability(t: &Table, z: &[f64], cs: &[u32], ex: &[f64], nn: &[f64], lr: &[f64]) -> (f64, f64) {
    let (mut o, mut e) = (vec![0f64; 31], vec![0f64; 31]);
    for i in 0..z.len() {
        let lp = t.ln_p(z[i], lr[cs[i] as usize]);
        let b = ((lp / std::f64::consts::LN_10 * 3.0).floor() + 30.0).clamp(0.0, 30.0) as usize;
        o[b] += nn[i];
        e[b] += ex[i] * lp.exp();
    }
    let tot: f64 = o.iter().sum();
    let mut idx: Vec<usize> = (0..31).collect();
    idx.sort_by(|&a, &b| o[b].total_cmp(&o[a]).then(a.cmp(&b)));
    let (mut acc, mut d) = (0f64, 0f64);
    for &b in &idx {
        d = d.max((o[b].max(1e-9) / e[b].max(1e-9)).ln().abs());
        acc += o[b];
        if acc >= 0.95 * tot {
            break;
        }
    }
    (d, (tot.max(1e-9) / e.iter().sum::<f64>().max(1e-9)).ln())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng};

    /// The eigenpairs of symmetric matrices, full rank and of low rank (as the centres' covariance with fewer centres
    /// than dimensions): A v = lambda v, orthonormal vectors, smallest first, the null space at 0.
    #[test]
    fn sym_eigen_finds_the_eigenpairs() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(17);
        for (d, rank) in [(1, 1), (2, 2), (5, 5), (12, 3), (40, 40), (64, 20), (97, 97)] {
            let g: Vec<f64> = (0..rank * d).map(|_| rng.gen_range(-1.0..1.0)).collect();
            let a: Vec<f64> = (0..d * d).map(|x| (0..rank).map(|r| g[r * d + x / d] * g[r * d + x % d]).sum()).collect();
            let (vals, vecs) = sym_eigen(&a, d);
            let top = vals.iter().fold(0f64, |m, &v| m.max(v.abs())).max(1e-300);
            assert!(vals.windows(2).all(|w| w[0] <= w[1]), "d {d}: not ascending");
            for (k, (&l, v)) in vals.iter().zip(&vecs).enumerate() {
                for i in 0..d {
                    let av: f64 = (0..d).map(|j| a[i * d + j] * v[j]).sum();
                    assert!((av - l * v[i]).abs() < 1e-10 * top, "d {d} rank {rank}: A v != lambda v at pair {k}");
                }
                for (m, w) in vecs.iter().enumerate() {
                    let dot: f64 = v.iter().zip(w).map(|(x, y)| x * y).sum();
                    assert!((dot - if k == m { 1.0 } else { 0.0 }).abs() < 1e-10, "d {d}: vectors {k} and {m} not orthonormal ({dot})");
                }
            }
            if rank < d {
                assert!(vals[..d - rank].iter().all(|&l| l.abs() < 1e-10 * top), "d {d} rank {rank}: the null space");
            }
        }
    }
}
