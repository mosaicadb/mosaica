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
//! - the mean-recall dial's table: per spread setting and table set, mean recall against price for k-NN bands and
//!   answer-share bands, each at several visible shares (`Band`), from the process's own decision (`process::decide`,
//!   hand-overs and the support table's verdicts included) on held-out pseudo-queries.
//!
//! Pseudo-queries are corpus rows spread over the index, each used plain and shifted: q' = normalise(a q + v), v a
//! unit vector in the directions in which the centres do not vary (the least-variance directions of the centres
//! holding `SHIFT_SHARE` of their variance) and a in `SHIFT_SCALES`. A pseudo-query's own row is left out of its
//! counts. Half of them fit the tables (all rows visible); the other half, under visible shares `VIS_SHARES` (fixed
//! random subsets of the rows), fit the dial, the margin and the build's report.
//!
//! The rows' scores come from blocks of rows against every pseudo-query at once (one f32 product per block); each
//! view's thresholds and counts are exact (rows within the f32 products' error of a threshold are scored by the exact
//! integer product of the query rounded to f16), and every sum over pseudo-queries, clusters or rows runs in a fixed
//! order with separate multiplies and adds, so the calibration is the same bits whatever the instruction set and the
//! thread count, and a build and a rebuild store the same.

use super::process::{self, Act, Model, Outcome, ProcInfo, Spread, Stop, Target, Units};
use super::q::{prep2, query_f16, support_pre, Pre};
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
/// k of the mean-recall dial's k-NN bands.
pub const KNN_BANDS: [usize; 4] = [1, 10, 100, 1000];
/// Answer shares (of the visible rows) of the dial's threshold bands.
pub const THR_BANDS: [f64; 4] = [1e-4, 1e-3, 1e-2, 1e-1];
/// The visible shares the dial's bands are fitted under (fixed random subsets of the rows; 1: every row).
pub const VIS_SHARES: [f64; 3] = [1.0, 0.1, 0.01];
/// The dial's grid of prices (recall per microsecond): 10^(1 - i/4), i = 0..LAM_STEPS.
pub const LAM_STEPS: usize = 33;
/// The saved index's files of the group covariances: RMS residuals, variances, scales and directions.
pub const FILES: [&str; 4] = ["calib_grad.bin", "calib_glam.bin", "calib_gdsc.bin", "calib_gdirs.bin"];
/// Sampled rows per block of the scoring passes, pseudo-queries per product, and bins of the first pass's histogram
/// of each view's scores.
const BLOCK_ROWS: usize = 256;
const Q_CHUNK: usize = 32;
const HIST_BINS: usize = 2048;
/// Pseudo-queries of the fit half per chunk of features; the first chunk's answer-holding zeta place the grid.
const FIT_CHUNK: usize = 64;

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
        self.ln_p_slope(zeta, lr).0
    }

    /// ln P and its derivative in zeta (0 where it is constant or clamped). Written without branches (selects only),
    /// so that a loop over units vectorises: below the grid the first segment at its start, above it the last at its
    /// end plus the tail.
    #[inline(always)]
    pub fn ln_p_slope(&self, zeta: f64, lr: f64) -> (f64, f64) {
        let last = (NG - 1) as f64;
        let x = (zeta - self.lo) / self.step - 0.5;
        let xc = x.max(0.0).min(last);
        let i = (xc as usize).min(NG - 2);
        let f = xc - i as f64;
        let g: &[f64; NG] = self.g[..NG].try_into().expect("a table holds NG values");
        let (g0, g1) = (g[i], g[i + 1]);
        let over = (x - last).max(0.0);
        let v = g0 + f * (g1 - g0) + self.slope * over * self.step;
        let d = if x <= 0.0 { 0.0 } else if x >= last { self.slope } else { (g1 - g0) / self.step };
        let l = v + self.beta * lr;
        let inside = l <= 0.0 && l >= LN_P_MIN;
        (l.clamp(LN_P_MIN, 0.0), if inside { d } else { 0.0 })
    }

    /// The largest zeta at which ln P (for log relative radius `lr`) is still at least `y`; None when it never is,
    /// or always is.
    pub fn zeta_at(&self, y: f64, lr: f64) -> Option<f64> {
        if y > 0.0 || y <= LN_P_MIN {
            return None;
        }
        let gt = y - self.beta * lr;
        if self.g[0] < gt {
            return None;
        }
        let x = |i: usize| self.lo + (i as f64 + 0.5) * self.step;
        if gt < self.g[NG - 1] {
            return (self.slope < 0.0).then(|| x(NG - 1) + (gt - self.g[NG - 1]) / self.slope);
        }
        // the last grid point still at or above gt (g non-increasing), and the segment after it
        let i = self.g.partition_point(|&v| v >= gt) - 1;
        if i + 1 >= NG {
            return Some(x(NG - 1));
        }
        let (a, b) = (self.g[i], self.g[i + 1]);
        Some(if a > b { x(i) + (a - gt) / (a - b) * self.step } else { x(i + 1) })
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

/// One band of the mean-recall dial: mean recall on the held-out pseudo-queries against ln price, along the price
/// grid (prices falling, recall rising), for k-NN at k = `size` or threshold queries answered by a share `size` of
/// the visible rows, with a visible share `vis` of the rows.
#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    pub knn: bool,
    pub size: f64,
    pub vis: f64,
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
        serde_json::json!({ "knn": self.knn, "size": self.size, "vis": self.vis, "ln_lam": self.ln_lam, "recall": self.recall })
    }

    fn from_json(v: &serde_json::Value) -> Option<Band> {
        let f = |k: &str| -> Option<Vec<f64>> { v[k].as_array()?.iter().map(|x| x.as_f64()).collect() };
        let (ln_lam, recall) = (f("ln_lam")?, f("recall")?);
        (ln_lam.len() == recall.len() && !ln_lam.is_empty()).then_some(())?;
        Some(Band { knn: v["knn"].as_bool()?, size: v["size"].as_f64()?, vis: v["vis"].as_f64()?, ln_lam, recall })
    }
}

/// The price for mean recall `r` from the band nearest the query's visible share (in log), then its k or answer
/// share; 0 (read everything) without bands.
pub fn band_price(bands: &[Band], knn: bool, size: f64, vis: f64, r: f64) -> f64 {
    let d = |a: f64, b: f64| (a.max(1e-12).ln() - b.max(1e-12).ln()).abs();
    bands.iter().filter(|b| b.knn == knn).min_by(|a, b| d(a.vis, vis).total_cmp(&d(b.vis, vis)).then(d(a.size, size).total_cmp(&d(b.size, size)))).map_or(0.0, |b| b.price(r))
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

/// The variance a covariance of RMS radius `rad` leaves after its directions' variances `lam`, per other spanned
/// dimension (of `n0`).
pub fn rest(rad: f64, lam: &[f32], n0: usize) -> f64 {
    let r = lam.len();
    let top: f64 = lam.iter().map(|&l| l as f64).sum();
    (rad * rad - top).max(0.0) / (n0.max(r + 1) - r) as f64
}

/// sigma(q) = sqrt(q' S q) for S its directions' variances `lam` and an isotropic `rest`, from the query's raw
/// products `p` with the int8 directions (scales `dsc`) and |q|^2 `qq`.
pub fn sigma(p: &[f64], lam: &[f32], dsc: &[f32], rest: f64, qq: f64) -> f64 {
    let (mut v, mut pp) = (0f64, 0f64);
    for ((&pj, &l), &sc) in p.iter().zip(lam).zip(dsc) {
        let x = pj * sc as f64;
        v += l as f64 * x * x;
        pp += x * x;
    }
    (v + rest * (qq - pp).max(0.0)).max(0.0).sqrt()
}

/// Each cluster's ln(rad / median rad).
pub fn lr_of(rad: &[f64], med: f64) -> Vec<f64> {
    rad.iter().map(|&r| (r.max(1e-12 * med) / med).ln()).collect()
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
    /// The mean-recall dial's bands, per spread setting (`Spread::ALL`) and table set (main, control): index 2 s + p.
    pub bands: Vec<Vec<Band>>,
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

    /// The mean-recall dial's bands for a spread setting and table set.
    pub fn bands(&self, spread: Spread, plain: bool) -> &[Band] {
        let s = Spread::ALL.iter().position(|&x| x == spread).unwrap();
        self.bands.get(2 * s + plain as usize).map_or(&[], |b| &b[..])
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
        let bands: Vec<Vec<Band>> = v["bands"].as_array()?.iter().map(|x| x.as_array()?.iter().map(Band::from_json).collect()).collect::<Option<_>>()?;
        (bands.len() == 2 * Spread::ALL.len()).then_some(())?;
        let gcov = GroupCov { r: v["gcov"]["r"].as_u64()? as usize, n0: v["gcov"]["n0"].as_u64()? as usize, rad, lam, dsc, dirs8 };
        Some(Calib { tables: tabs("tables")?, plain: tabs("plain")?, delta: v["delta"].as_f64()?, bands, gcov, med_rad: v["med_rad"].as_f64()?, report: v["report"].clone() })
    }

    /// Hashes everything stored (`Index2::store_hash`).
    pub fn hash(&self, h: &mut store::Fx) {
        let mut t = |x: &Table| {
            h.pod(&[x.lo, x.step, x.beta, x.slope]);
            h.pod(&x.g);
        };
        self.tables.iter().chain(&self.plain).for_each(&mut t);
        h.pod(&[self.delta, self.med_rad]);
        h.u64(self.bands.len() as u64);
        for b in self.bands.iter().flatten() {
            h.usizes(&[b.knn as usize, b.ln_lam.len()]);
            h.pod(&[b.size, b.vis]);
            h.pod(&b.ln_lam);
            h.pod(&b.recall);
        }
        h.usizes(&[self.gcov.r, self.gcov.n0]);
        h.pod(&self.gcov.rad[..]);
        h.pod(&self.gcov.lam[..]);
        h.pod(&self.gcov.dsc[..]);
        h.big_pod(&self.gcov.dirs8[..]);
    }
}

/// The raw products of `q` with `rows` int8 rows of n, each summed over the dimensions in order in f64.
fn proj_f64(dirs8: &[i8], q: &[f64], rows: usize, n: usize) -> Vec<f64> {
    (0..rows).map(|j| dirs8[j * n..(j + 1) * n].iter().zip(q).fold(0f64, |a, (&d, &x)| a + d as f64 * x)).collect()
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

/// The rows the scoring passes score: every row of a cluster up to `order::SAMPLE`, else that many spread over it,
/// each standing for its share of the cluster; in cluster order.
struct Sample {
    row: Vec<u32>,
    w: Vec<f64>,
    cl: Vec<u32>,
    /// per visible share (`VIS_SHARES`), whether each sampled row is visible
    vis: Vec<Vec<bool>>,
}

/// Whether row `row` is in the fixed random subset holding a share `s` of the rows (a hash of the row).
fn in_share(row: u32, s: f64) -> bool {
    if s >= 1.0 {
        return true;
    }
    let mut z = (row as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    ((z >> 11) as f64) * (1.0 / 9_007_199_254_740_992.0) < s
}

/// A pseudo-query: its f16-rounded forms, its own row (left out of its counts) and that row's place in the sample.
struct Pq {
    q16: Vec<u16>,
    q32: Vec<f32>,
    qn: f64,
    own: usize,
    own_s: Option<usize>,
    shifted: bool,
    /// fits the tables (else held out)
    fit: bool,
}

/// A view: pseudo-query `q` with the rows of visible share `VIS_SHARES[share]`, counted at `ranks` (rows above the
/// threshold, in the sample's weights).
struct View {
    q: usize,
    share: usize,
    ranks: Vec<usize>,
}

/// A view's thresholds at its ranks (the exact score of the rank-th visible row), the visible rows at or above each
/// (weighted), and their count per cluster (ascending cluster).
struct Scored {
    tau: Vec<f64>,
    tot: Vec<f64>,
    cnt: Vec<Vec<(u32, f64)>>,
}

/// Every view's thresholds and counts, in two passes over the sampled rows in blocks, each block scored against every
/// pseudo-query by one f32 product per chunk of them: the first histograms each view's f32 scores (finding the bin
/// of each rank's threshold), the second counts per cluster the rows certainly above each threshold (above its bin by
/// twice the f32 products' error bound) and keeps the rows within it, whose exact products then fix each threshold
/// and its counts. The f32 products only narrow the search: every threshold and count is the exact one.
fn score_views(ix: &Index2, s: &Sample, qs: &[Pq], views: &[View]) -> Vec<Scored> {
    let n = ix.n();
    let ns = s.row.len();
    let xr = ix.x2.sqrt();
    // the f32 products' error: (n + 2) roundings of at most 2^-24 |q| |x| each (any order of the sums)
    let err: Vec<f64> = qs.iter().map(|q| 1.01 * (n as f64 + 2.0) * f64::powi(2.0, -24) * q.qn * xr + 1e-12).collect();
    let smax: Vec<f64> = qs.iter().map(|q| q.qn * xr * 1.0001 + 1e-9).collect();
    let qmat: Vec<f32> = qs.iter().flat_map(|q| q.q32.iter().copied()).collect();
    let mut qviews: Vec<Vec<usize>> = vec![Vec::new(); qs.len()];
    for (vi, v) in views.iter().enumerate() {
        qviews[v.q].push(vi);
    }
    let blocks: Vec<(usize, usize)> = (0..ns).step_by(BLOCK_ROWS).map(|a| (a, (a + BLOCK_ROWS).min(ns))).collect();
    let xblock = |a: usize, b: usize| -> Vec<f32> { (a..b).flat_map(|i| ix.full_row_f16(s.row[i] as usize).iter().map(|&h| f16_to_f32(h))).collect() };
    // each chunk of pseudo-queries' scores for one block, then `f` on each (pseudo-query, its state)
    fn by_chunks<T: Send>(qmat: &[f32], n: usize, x: &[f32], rows: usize, states: &mut [T], f: &(dyn Fn(usize, &[f32], &mut T) + Sync)) {
        states.par_chunks_mut(Q_CHUNK).enumerate().for_each(|(ci, chunk)| {
            let (q0, m) = (ci * Q_CHUNK, chunk.len());
            let mut c = vec![0f32; m * rows];
            // SAFETY: the matrices hold m x n, rows x n and m x rows values at the strides given
            unsafe { matrixmultiply::sgemm(m, n, rows, 1.0, qmat[q0 * n..].as_ptr(), n as isize, 1, x.as_ptr(), 1, n as isize, 0.0, c.as_mut_ptr(), rows as isize, 1) };
            for (a, st) in chunk.iter_mut().enumerate() {
                f(q0 + a, &c[a * rows..(a + 1) * rows], st);
            }
        });
    }
    let bin = |q: usize, sc: f64| (((sc + smax[q]) / (2.0 * smax[q]) * HIST_BINS as f64).floor().max(0.0) as usize).min(HIST_BINS - 1);
    let edge = |q: usize, b: usize| -smax[q] + 2.0 * smax[q] * b as f64 / HIST_BINS as f64;
    // pass 1: per pseudo-query, a histogram per view
    let mut hist: Vec<Vec<Vec<f64>>> = qviews.iter().map(|v| vec![vec![0f64; HIST_BINS]; v.len()]).collect();
    for &(a, b) in &blocks {
        let x = xblock(a, b);
        by_chunks(&qmat, n, &x, b - a, &mut hist, &|q, c, h: &mut Vec<Vec<f64>>| {
            for (hv, &vi) in h.iter_mut().zip(&qviews[q]) {
                let vis = &s.vis[views[vi].share];
                for (r, &sc) in c.iter().enumerate() {
                    let i = a + r;
                    if vis[i] && qs[q].own_s != Some(i) {
                        hv[bin(q, sc as f64)] += s.w[i];
                    }
                }
            }
        });
    }
    // each view's window per rank: the bin where the weight from the top reaches the rank, widened by twice the error
    let windows: Vec<Vec<(f64, f64)>> = views.iter().enumerate().map(|(vi, v)| {
        let h = &hist[v.q][qviews[v.q].iter().position(|&x| x == vi).unwrap()];
        let e = err[v.q];
        v.ranks.iter().map(|&r| {
            let mut acc = 0.0;
            let mut b = HIST_BINS - 1;
            loop {
                acc += h[b];
                if acc >= r as f64 || b == 0 {
                    break;
                }
                b -= 1;
            }
            (edge(v.q, b) - 2.0 * e, edge(v.q, b + 1) + 2.0 * e)
        }).collect()
    }).collect();
    drop(hist);
    // pass 2: per view and rank, the weight certainly above, its counts per cluster, and the rows within the window
    #[derive(Default)]
    struct St {
        cur: Vec<u32>,
        acc: Vec<Vec<f64>>,
        cert: Vec<Vec<Vec<(u32, f64)>>>,
        above: Vec<Vec<f64>>,
        cand: Vec<Vec<Vec<u32>>>,
    }
    let mut sts: Vec<St> = qviews.iter().map(|vs| St {
        cur: vec![u32::MAX; vs.len()],
        acc: vs.iter().map(|&vi| vec![0.0; views[vi].ranks.len()]).collect(),
        cert: vs.iter().map(|&vi| vec![Vec::new(); views[vi].ranks.len()]).collect(),
        above: vs.iter().map(|&vi| vec![0.0; views[vi].ranks.len()]).collect(),
        cand: vs.iter().map(|&vi| vec![Vec::new(); views[vi].ranks.len()]).collect(),
    }).collect();
    fn flush(st: &mut St, k: usize) {
        for j in 0..st.acc[k].len() {
            if st.acc[k][j] > 0.0 {
                let c = st.cur[k];
                st.cert[k][j].push((c, st.acc[k][j]));
                st.above[k][j] += st.acc[k][j];
                st.acc[k][j] = 0.0;
            }
        }
    }
    for &(a, b) in &blocks {
        let x = xblock(a, b);
        by_chunks(&qmat, n, &x, b - a, &mut sts, &|q, c, st: &mut St| {
            for (k, &vi) in qviews[q].iter().enumerate() {
                let (vis, win) = (&s.vis[views[vi].share], &windows[vi]);
                for (r, &sc) in c.iter().enumerate() {
                    let i = a + r;
                    if !vis[i] || qs[q].own_s == Some(i) {
                        continue;
                    }
                    if s.cl[i] != st.cur[k] {
                        flush(st, k);
                        st.cur[k] = s.cl[i];
                    }
                    let sc = sc as f64;
                    // windows fall with the rank: from the lowest up, stop at the first the score is under
                    for j in (0..win.len()).rev() {
                        if sc < win[j].0 {
                            break;
                        }
                        if sc > win[j].1 {
                            st.acc[k][j] += s.w[i];
                        } else {
                            st.cand[k][j].push(i as u32);
                        }
                    }
                }
            }
        });
    }
    for st in sts.iter_mut() {
        for k in 0..st.cur.len() {
            flush(st, k);
        }
    }
    // each view's exact thresholds and counts
    let per_view: Vec<(usize, usize)> = qviews.iter().enumerate().flat_map(|(q, vs)| (0..vs.len()).map(move |k| (q, k))).collect();
    let mut out: Vec<Option<Scored>> = (0..views.len()).map(|_| None).collect();
    let done: Vec<(usize, Scored)> = per_view.par_iter().map(|&(q, k)| {
        let st = &sts[q];
        let vi = qviews[q][k];
        let v = &views[vi];
        let (mut tau, mut tot, mut cnt) = (Vec::new(), Vec::new(), Vec::new());
        for (j, &r) in v.ranks.iter().enumerate() {
            let mut cs: Vec<(f64, u32)> = st.cand[k][j].iter().map(|&i| (exact_to_f64(exact_dot(&qs[q].q16, ix.full_row_f16(s.row[i as usize] as usize))), i)).collect();
            cs.sort_by(|x, y| y.0.total_cmp(&x.0).then(s.row[x.1 as usize].cmp(&s.row[y.1 as usize])));
            let mut acc = st.above[k][j];
            let mut t = windows[vi][j].0;
            for &(e, i) in &cs {
                acc += s.w[i as usize];
                if acc >= r as f64 {
                    t = e;
                    break;
                }
            }
            let mut counts: Vec<(u32, f64)> = st.cert[k][j].clone();
            let mut sum = st.above[k][j];
            for &(e, i) in cs.iter().take_while(|x| x.0 >= t) {
                counts.push((s.cl[i as usize], s.w[i as usize]));
                sum += s.w[i as usize];
                let _ = e;
            }
            counts.sort_by_key(|x| x.0);
            let mut merged: Vec<(u32, f64)> = Vec::with_capacity(counts.len());
            for (c, w) in counts {
                match merged.last_mut() {
                    Some(m) if m.0 == c => m.1 += w,
                    _ => merged.push((c, w)),
                }
            }
            tau.push(t);
            tot.push(sum);
            cnt.push(merged);
        }
        (vi, Scored { tau, tot, cnt })
    }).collect();
    for (vi, sc) in done {
        out[vi] = Some(sc);
    }
    out.into_iter().map(|x| x.unwrap()).collect()
}

/// A pseudo-query's features: every cluster's centre score and its spread under each tier, in f64 from the stored
/// int8 directions.
struct Feats {
    mu: Vec<f64>,
    s: [Vec<f64>; 2],
}

fn features(ix: &Index2, gcov: &GroupCov, grest: &[f64], orest: &[f64], q32: &[f32]) -> Feats {
    let st = ix.order.as_ref().unwrap();
    let (n, k) = (ix.n(), ix.k());
    let q64: Vec<f64> = q32.iter().map(|&x| x as f64).collect();
    let qq: f64 = q64.iter().map(|x| x * x).sum();
    let mu: Vec<f64> = (0..k).map(|c| ix.qm(&q64, c)).collect();
    let (gr, r) = (gcov.r, st.r);
    let sg: Vec<f64> = (0..ix.groups.len()).map(|g| sigma(&proj_f64(&gcov.dirs8[g * gr * n..(g + 1) * gr * n], &q64, gr, n), &gcov.lam[g * gr..(g + 1) * gr], &gcov.dsc[g * gr..(g + 1) * gr], grest[g], qq)).collect();
    let s1: Vec<f64> = (0..k).map(|c| {
        let g = ix.cl[c].g;
        (sg[g] * st.rad[c] / gcov.rad[g].max(1e-300)).max(1e-12)
    }).collect();
    let s2: Vec<f64> = (0..k).map(|c| sigma(&proj_f64(st.dirs(c, r, n), &q64, r, n), &st.lam[c * r..(c + 1) * r], &st.dsc[c * r..(c + 1) * r], orest[c], qq).max(1e-12)).collect();
    Feats { mu, s: [s1, s2] }
}

/// The ranks a view is counted at: for every row visible, the shares of `FRACTIONS` and of the bands; for a visible
/// share, the bands' k and answer shares; always at most half the visible rows.
fn ranks(nvis: usize, full: bool) -> Vec<usize> {
    let of = |f: f64| ((f * nvis as f64).round() as usize).max(1);
    let fr: &[f64] = if full { &FRACTIONS } else { &[] };
    let mut r: Vec<usize> = fr.iter().chain(&THR_BANDS).map(|&f| of(f)).chain(KNN_BANDS).filter(|&x| x <= nvis / 2).collect();
    r.sort_unstable();
    r.dedup();
    r
}

/// The binned triples (zeta, cluster, exposure, count) of one table's fit: per bin of zeta the answers observed and per
/// cluster the exposure, and the answers' sum of lr; triples above the grid left out (the tail's slope extrapolates).
struct Bins {
    lo: f64,
    step: f64,
    obs: Vec<f64>,
    a: Vec<f64>,
    nlr: f64,
    k: usize,
}

impl Bins {
    /// The grid from the zeta of triples holding answers: their 5% and 99.9% quantiles, widened by 5% above (the first
    /// bin takes every zeta below, where a unit holds most of its rows above tau).
    fn new(mut zp: Vec<f64>, k: usize) -> Bins {
        if zp.is_empty() {
            zp.push(0.0);
        }
        let (lo, hi) = (quantile(&mut zp, 0.05), quantile(&mut zp, 0.999));
        let hi = hi.max(lo + 1e-6);
        let hi = hi + 0.05 * (hi - lo);
        Bins { lo, step: (hi - lo) / NG as f64, obs: vec![0.0; NG], a: vec![0.0; NG * k], nlr: 0.0, k }
    }

    fn add(&mut self, z: f64, c: usize, ex: f64, nn: f64, lr: f64) {
        let x = ((z - self.lo) / self.step).floor();
        if x >= NG as f64 || x.is_nan() {
            return;
        }
        let b = x.max(0.0) as usize;
        self.obs[b] += nn;
        self.a[b * self.k + c] += ex;
        self.nlr += nn * lr;
    }

    /// The table: Poisson regression of the counts on exposure exp(g(zeta) + beta lr), g non-increasing on the bins
    /// by weighted isotonic regression of their rates alternated with Newton steps on beta; above the last bin
    /// holding answers g continues at the slope fitted (least squares, weighted by answers) to the last eight bins
    /// holding answers.
    fn fit(&self, lr: &[f64]) -> Table {
        let k = self.k;
        let grid: Vec<f64> = (0..NG).map(|i| self.lo + (i as f64 + 0.5) * self.step).collect();
        let mut beta = 0f64;
        let mut g = vec![0f64; NG];
        let expo = |beta: f64| -> Vec<f64> { (0..NG).map(|b| (0..k).map(|c| self.a[b * k + c] * (beta * lr[c]).exp()).sum()).collect() };
        for _ in 0..8 {
            let e = expo(beta);
            let rate: Vec<f64> = (0..NG).map(|b| if e[b] > 0.0 { self.obs[b] / e[b].max(1e-300) } else { 0.0 }).collect();
            g = pava_dec(&rate, &e).iter().map(|&r| r.max(1e-12).ln()).collect();
            for _ in 0..3 {
                let (mut m1, mut m2) = (0f64, 0f64);
                for b in 0..NG {
                    let eg = g[b].exp();
                    for c in 0..k {
                        let m = self.a[b * k + c] * (beta * lr[c]).exp() * eg;
                        m1 += m * lr[c];
                        m2 += m * lr[c] * lr[c];
                    }
                }
                beta += ((self.nlr - m1) / m2.max(1e-30)).clamp(-1.0, 1.0);
            }
        }
        let held: Vec<usize> = (0..NG).filter(|&b| self.obs[b] > 0.0).collect();
        let mut slope = 0.0;
        if let Some(&jl) = held.last() {
            let tail = &held[held.len().saturating_sub(8)..];
            let w: f64 = tail.iter().map(|&b| self.obs[b]).sum();
            let xm = tail.iter().map(|&b| self.obs[b] * grid[b]).sum::<f64>() / w;
            let ym = tail.iter().map(|&b| self.obs[b] * g[b]).sum::<f64>() / w;
            let sxx: f64 = tail.iter().map(|&b| self.obs[b] * (grid[b] - xm) * (grid[b] - xm)).sum();
            let sxy: f64 = tail.iter().map(|&b| self.obs[b] * (grid[b] - xm) * (g[b] - ym)).sum();
            slope = if sxx > 0.0 { (sxy / sxx).min(0.0) } else { 0.0 };
            for b in jl + 1..NG {
                g[b] = g[jl] + slope * (grid[b] - grid[jl]);
            }
        }
        Table { lo: self.lo, step: self.step, g, beta, slope }
    }
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
    let lr = lr_of(&st.rad, med_rad);
    let (gr, r) = (gcov.r, st.r);
    let grest: Vec<f64> = (0..ix.groups.len()).map(|g| rest(gcov.rad[g], &gcov.lam[g * gr..(g + 1) * gr], gcov.n0)).collect();
    let orest: Vec<f64> = (0..k).map(|c| rest(st.rad[c], &st.lam[c * r..(c + 1) * r], st.n0)).collect();
    let grp: Vec<usize> = ix.cl.iter().map(|c| c.g).collect();
    // the sample, and every cluster's visible rows under each visible share
    let mut smp = Sample { row: Vec::new(), w: Vec::new(), cl: Vec::new(), vis: Vec::new() };
    for (c, cl) in ix.cl.iter().enumerate() {
        let m = cl.r1 - cl.r0;
        let take = spread(m, order::SAMPLE);
        let w = m as f64 / take.len().max(1) as f64;
        for i in take {
            smp.row.push((cl.r0 + i) as u32);
            smp.w.push(w);
            smp.cl.push(c as u32);
        }
    }
    smp.vis = VIS_SHARES.iter().map(|&f| smp.row.iter().map(|&row| in_share(row, f)).collect()).collect();
    let nvs: Vec<Vec<f64>> = VIS_SHARES.iter().map(|&f| ix.cl.iter().map(|c| (c.r0..c.r1).filter(|&row| in_share(row as u32, f)).count() as f64).collect()).collect();
    let row_cl: Vec<u32> = ix.cl.iter().enumerate().flat_map(|(c, cl)| std::iter::repeat_n(c as u32, cl.r1 - cl.r0)).collect();
    // the pseudo-queries: rows spread over the index, each plain and shifted
    let rows: Vec<usize> = spread(nrows, NQ_ROWS);
    let mut qs: Vec<Pq> = Vec::new();
    for (i, &row) in rows.iter().enumerate() {
        let x: Vec<f64> = ix.full_row_f16(row).iter().map(|&h| f16_to_f32(h) as f64).collect();
        let xn = x.iter().map(|v| v * v).sum::<f64>().sqrt();
        if xn < 0.5 {
            continue;
        }
        let own_s = smp.row.binary_search(&(row as u32)).ok();
        let mut push = |v: &[f64], shifted: bool| {
            let s = v.iter().map(|a| a * a).sum::<f64>().sqrt();
            let q: Vec<f32> = v.iter().map(|a| (a / s) as f32).collect();
            let (q16, q32, qn) = query_f16(&q);
            qs.push(Pq { q16, q32, qn, own: row, own_s, shifted, fit: i % 2 == 0 });
        };
        let plain: Vec<f64> = x.iter().map(|v| v / xn).collect();
        push(&plain, false);
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
            push(&sh, true);
        }
    }
    // the views: every pseudo-query with every row visible; the held-out ones under each other visible share too
    let nvis_share: Vec<usize> = nvs.iter().map(|v| v.iter().sum::<f64>() as usize - 1).collect();
    let mut views: Vec<View> = Vec::new();
    for (q, p) in qs.iter().enumerate() {
        for (si, _) in VIS_SHARES.iter().enumerate().filter(|&(si, _)| si == 0 || !p.fit) {
            let rk = ranks(nvis_share[si], si == 0);
            if !rk.is_empty() {
                views.push(View { q, share: si, ranks: rk });
            }
        }
    }
    let scored = score_views(ix, &smp, &qs, &views);
    let t_score = t.s() - t_cov - t_basis;
    let exposure = |q: usize, share: usize, c: usize| nvs[share][c] - (row_cl[qs[q].own] as usize == c && in_share(qs[q].own as u32, VIS_SHARES[share])) as usize as f64;
    let full: Vec<usize> = (0..views.len()).filter(|&v| views[v].share == 0).collect();
    // the fit half, a chunk at a time: features, then every triple binned (the first chunk places the grids)
    let fit_views: Vec<usize> = full.iter().copied().filter(|&v| qs[views[v].q].fit).collect();
    let mut bins: Vec<Bins> = Vec::new(); // [main group, main own, plain group, plain own]
    let mut dense = vec![0f64; k];
    for (ci, chunk) in fit_views.chunks(FIT_CHUNK).enumerate() {
        let feats: Vec<Feats> = chunk.par_iter().map(|&v| features(ix, &gcov, &grest, &orest, &qs[views[v].q].q32)).collect();
        if ci == 0 {
            for set in 0..2 {
                for tier in 0..2 {
                    let mut zp = Vec::new();
                    for (&v, f) in chunk.iter().zip(&feats) {
                        if set == 1 && qs[views[v].q].shifted {
                            continue;
                        }
                        for (j, &tv) in scored[v].tau.iter().enumerate() {
                            zp.extend(scored[v].cnt[j].iter().map(|&(c, _)| (tv - f.mu[c as usize]) / f.s[tier][c as usize]));
                        }
                    }
                    bins.push(Bins::new(zp, k));
                }
            }
        }
        for (&v, f) in chunk.iter().zip(&feats) {
            let q = views[v].q;
            for (j, &tv) in scored[v].tau.iter().enumerate() {
                for &(c, w) in &scored[v].cnt[j] {
                    dense[c as usize] = w;
                }
                for c in 0..k {
                    let ex = exposure(q, 0, c);
                    if ex > 0.0 {
                        for tier in 0..2 {
                            let z = (tv - f.mu[c]) / f.s[tier][c];
                            bins[tier].add(z, c, ex, dense[c], lr[c]);
                            if !qs[q].shifted {
                                bins[2 + tier].add(z, c, ex, dense[c], lr[c]);
                            }
                        }
                    }
                }
                for &(c, _) in &scored[v].cnt[j] {
                    dense[c as usize] = 0.0;
                }
            }
        }
    }
    let tables = [bins[0].fit(&lr), bins[1].fit(&lr)];
    let plain = [bins[2].fit(&lr), bins[3].fit(&lr)];
    drop(bins);
    // the held-out half: features (kept for the dial), the tables' reliability and count factor, the margin
    let held: Vec<usize> = (0..qs.len()).filter(|&q| !qs[q].fit).collect();
    let hfeat: Vec<Feats> = held.par_iter().map(|&q| features(ix, &gcov, &grest, &orest, &qs[q].q32)).collect();
    let feat_of = |q: usize| &hfeat[held.binary_search(&q).unwrap()];
    let held_full: Vec<usize> = full.iter().copied().filter(|&v| !qs[views[v].q].fit).collect();
    let mut report = serde_json::Map::new();
    let rk0 = &views[*held_full.first()?].ranks;
    let k10 = rk0.iter().enumerate().min_by(|a, b| (*a.1 as f64 / 10.0).ln().abs().total_cmp(&(*b.1 as f64 / 10.0).ln().abs())).map(|x| x.0).unwrap_or(0);
    for (tier, name) in [(0usize, "group"), (1, "own")] {
        let mut o = serde_json::Map::new();
        for (kind, shifted) in [("plain", false), ("shifted", true)] {
            let (mut ob, mut eb) = (vec![0f64; 31], vec![0f64; 31]);
            let mut kap = Vec::new();
            for &v in held_full.iter().filter(|&&v| qs[views[v].q].shifted == shifted) {
                let q = views[v].q;
                let f = feat_of(q);
                for (j, &tv) in scored[v].tau.iter().enumerate() {
                    for &(c, w) in &scored[v].cnt[j] {
                        dense[c as usize] = w;
                    }
                    let mut m = 0.0;
                    for c in 0..k {
                        let ex = exposure(q, 0, c);
                        if ex > 0.0 {
                            let lp = tables[tier].ln_p((tv - f.mu[c]) / f.s[tier][c], lr[c]);
                            let b = ((lp / std::f64::consts::LN_10 * 3.0).floor() + 30.0).clamp(0.0, 30.0) as usize;
                            ob[b] += dense[c];
                            eb[b] += ex * lp.exp();
                            m += ex * lp.exp();
                        }
                    }
                    if j == k10 {
                        kap.push((m.max(1e-300) / scored[v].tot[j]).ln());
                    }
                    for &(c, _) in &scored[v].cnt[j] {
                        dense[c as usize] = 0.0;
                    }
                }
            }
            let (d, tot) = reliability(&ob, &eb);
            kap.sort_by(|a, b| a.total_cmp(b));
            let qf = |f: f64| kap.get(((kap.len() as f64 - 1.0) * f).round() as usize).copied().unwrap_or(f64::NAN);
            o.insert(kind.into(), serde_json::json!({ "delta": d, "ln_obs_pred": tot, "ln_kappa_rank": rk0[k10], "ln_kappa_10_50_90": [qf(0.1), qf(0.5), qf(0.9)] }));
        }
        report.insert(name.into(), o.into());
    }
    let mut ds: Vec<(f64, f64)> = Vec::new();
    for &v in &held_full {
        let q = views[v].q;
        let f = feat_of(q);
        for (j, &tv) in scored[v].tau.iter().enumerate() {
            let w = |tier: usize| -> Vec<f64> { (0..k).map(|c| exposure(q, 0, c) * tables[tier].ln_p((tv - f.mu[c]) / f.s[tier][c], lr[c]).exp()).collect() };
            let (w1, w2) = (w(0), w(1));
            let (s1, s2): (f64, f64) = (w1.iter().sum(), w2.iter().sum());
            for &(c, cnt) in &scored[v].cnt[j] {
                let c = c as usize;
                ds.push((((w2[c] / s2).ln() - (w1[c] / s1).ln()).abs(), cnt));
            }
        }
    }
    let delta = wquantile(&mut ds, 0.95);
    // the mean-recall dial: per held-out view, band, spread setting and table set, the recall of the process's own
    // decision at each price of the grid
    let cm = process::Costs1::for_dim(n);
    let all_cl: Vec<usize> = (0..k).filter(|&c| ix.cl[c].r1 > ix.cl[c].r0).collect();
    let held_views: Vec<usize> = (0..views.len()).filter(|&v| !qs[views[v].q].fit).collect();
    // (setting, plain, share, knn, size) of each band, in a fixed order
    let mut specs: Vec<(usize, bool, usize, bool, f64)> = Vec::new();
    for si in 0..Spread::ALL.len() {
        for plain_set in [false, true] {
            for share in 0..VIS_SHARES.len() {
                for &kk in &KNN_BANDS {
                    specs.push((si, plain_set, share, true, kk as f64));
                }
                for &f in &THR_BANDS {
                    specs.push((si, plain_set, share, false, f));
                }
            }
        }
    }
    let per_view: Vec<Vec<Option<Vec<f64>>>> = held_views.par_iter().map(|&v| {
        let (q, share) = (views[v].q, views[v].share);
        let f = feat_of(q);
        let nvis: f64 = (0..k).map(|c| exposure(q, share, c)).sum();
        let pr = prep2(ix, &qs[q].q32);
        if ix.nukey.is_empty() && ix.nanch > 0 {
            pr.anc_among(ix, Some(&all_cl));
        }
        specs.iter().map(|&(si, plain_set, sh, knn, size)| {
            if sh != share {
                return None;
            }
            let rank = if knn { size as usize } else { ((size * nvis_share[share] as f64).round() as usize).max(1) };
            let j = views[v].ranks.iter().position(|&x| x == rank)?;
            let (tv, tot) = (scored[v].tau[j], scored[v].tot[j]);
            let mut cnt = vec![0f64; k];
            for &(c, w) in &scored[v].cnt[j] {
                cnt[c as usize] = w;
            }
            let target = if knn { Target::Knn(rank) } else { Target::Thr(tv) };
            let spread = Spread::ALL[si];
            let tier0 = if spread == Spread::OwnAll { Tier::Own } else { Tier::Group };
            let tabs = if plain_set { [&plain[0], &plain[1]] } else { [&tables[0], &tables[1]] };
            let model = Model { tabs, delta, cm, bands: &[], vis_share: VIS_SHARES[share], nvis };
            let anchors = if knn || !ix.nukey.is_empty() { 0 } else { ix.nanch };
            if process::early_scan(&cm, k, anchors, nvis as usize, target) {
                return Some(vec![1.0; LAM_STEPS]);
            }
            let mut base = Units::default();
            for c in 0..k {
                let ex = exposure(q, share, c);
                if ex <= 0.0 {
                    continue;
                }
                let verdict = if knn || anchors == 0 { None } else { support_pre(ix, &pr, tv, ex as usize, c, false) };
                if matches!(verdict, Some(Pre::Skip)) {
                    continue;
                }
                base.push(c, grp[c], ex, f.mu[c], lr[c], f.s[tier0 as usize][c], tier0, true, matches!(verdict, Some(Pre::In)));
            }
            if base.cl.is_empty() {
                return Some(vec![1.0; LAM_STEPS]);
            }
            Some((0..LAM_STEPS).map(|s| {
                let lam = 10f64.powf(1.0 - s as f64 / 4.0);
                let mut u = base.clone();
                let mut fill = |u: &mut Units, idx: &[usize], tier: Tier| -> usize {
                    for &i in idx {
                        u.s[i] = f.s[tier as usize][u.cl[i]];
                        u.tier[i] = tier;
                        u.exact[i] = true;
                    }
                    0
                };
                let mut info = ProcInfo { scan_cost: nvis * cm.full, ..ProcInfo::default() };
                match process::decide(&model, &mut u, &mut fill, target, Stop::Price(lam), spread, &mut info) {
                    Outcome::Scan | Outcome::Exact => 1.0,
                    Outcome::Plan { b, act } => {
                        let got: f64 = (0..b.v.len()).filter(|&i| act[i] != Act::Skip).map(|i| cnt[u.cl[b.units[i]]]).sum::<f64>()
                            + (0..u.cl.len()).filter(|&i| u.sure[i]).map(|i| cnt[u.cl[i]]).sum::<f64>();
                        (got / tot).min(1.0)
                    }
                }
            }).collect())
        }).collect()
    }).collect();
    let ln_lam: Vec<f64> = (0..LAM_STEPS).map(|s| (1.0 - s as f64 / 4.0) * std::f64::consts::LN_10).collect();
    let mut bands: Vec<Vec<Band>> = vec![Vec::new(); 2 * Spread::ALL.len()];
    for (bi, &(si, plain_set, share, knn, size)) in specs.iter().enumerate() {
        let curves: Vec<&Vec<f64>> = per_view.iter().filter_map(|pv| pv[bi].as_ref()).collect();
        if curves.is_empty() {
            continue;
        }
        let m = curves.len() as f64;
        let recall: Vec<f64> = (0..LAM_STEPS).map(|s| curves.iter().map(|c| c[s]).sum::<f64>() / m).collect();
        bands[2 * si + plain_set as usize].push(Band { knn, size, vis: VIS_SHARES[share], ln_lam: ln_lam.clone(), recall });
    }
    report.insert("pseudo_queries".into(), serde_json::json!({ "rows": rows.len(), "fit": qs.iter().filter(|p| p.fit).count(), "held_out": held.len(), "views": views.len(), "shift_directions": basis.len(), "ranks": rk0 }));
    log(format!("calibration in {:.1}s (group covariances {t_cov:.1}s, shift basis {t_basis:.1}s of {} directions, {} pseudo-queries in {} views scored {t_score:.1}s, fits and dial {:.1}s): beta {:.3} / {:.3}, margin {delta:.3}; held out {}",
                t.s(), basis.len(), qs.len(), views.len(), t.s() - t_cov - t_basis - t_score, tables[0].beta, tables[1].beta, serde_json::Value::Object(report.clone())));
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

/// delta: the largest |ln(observed / predicted)| over bins of predicted P (thirds of a decade) holding 95% of the
/// answers; and ln(observed / predicted) over all.
fn reliability(o: &[f64], e: &[f64]) -> (f64, f64) {
    let tot: f64 = o.iter().sum();
    let mut idx: Vec<usize> = (0..o.len()).collect();
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
