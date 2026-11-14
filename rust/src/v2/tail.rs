//! The randomised levels' constant c per (group, level), from the exact law of the error.
//!
//! In a coded block of nb positions the estimator's error is A <u, P Delta> with u uniform on the unit sphere of
//! y-perp, a space of d = nb - 1 dimensions, so the error over the coded blocks is X = sum_j a_j U_j with U_j a
//! coordinate of a uniform unit vector in R^{d_j}, independent across blocks, and sigma^2 = sum_j a_j^2 / d_j. The
//! sub-Gaussian bound P[X >= c sigma] <= exp(-c^2 / 2) uses only the variance; the bound here uses the law itself.
//!
//! - One coded block: P[U >= s] is the exact tail, so c = s* sqrt(d) with P[U >= s*] = delta.
//! - Several: Chernoff with each coordinate's exact log moment generating function psi_d (convex, increasing on
//!   x >= 0): P[X >= h] <= exp(-lambda h + sum_j psi_dj(lambda a_j)) for every lambda > 0, which may depend on the
//!   a_j (the law of u is the same given A). With shares w_j = a_j^2 / (d_j sigma^2), h / sigma =
//!   min_lambda (ln(1/delta) + sum_j psi_dj(lambda sqrt(w_j d_j))) / lambda. The constant must bound its largest value
//!   over the shares, and max_w min_lambda <= min_lambda max_w. For a fixed lambda the sum is separable: each share is
//!   rounded up to a multiple of 1/K (psi is increasing, so the rounded share bounds every share in its cell) and the
//!   largest sum over rounded shares that add up to between 1 and 1 + (J - 1)/K is found by dynamic programming.
//!
//! delta is the per-side failure probability of the sub-Gaussian bound it replaces, exp(-c^2 / 2), so the union
//! bound over (item, level) events is unchanged. psi is tabulated by numerical integration and read by linear
//! interpolation, an upper bound for a convex function; the result carries a 1e-6 relative margin.
use std::collections::HashMap;

/// ln of the integral of the coordinate density on [-1, 1] times exp(x s), by Simpson's rule on `m` intervals, with
/// the density's normaliser; the density is proportional to (1 - s^2)^((d - 3) / 2).
fn log_mgf(d: usize, x: f64, m: usize) -> f64 {
    let e = (d as f64 - 3.0) / 2.0;
    let h = 2.0 / m as f64;
    let g = |s: f64| if s.abs() >= 1.0 { f64::NEG_INFINITY } else { e * (1.0 - s * s).ln() + x * s };
    let mx = (0..=m).map(|i| g(-1.0 + i as f64 * h)).fold(f64::NEG_INFINITY, f64::max);
    let mut sum = 0.0;
    let mut sum0 = 0.0;
    for i in 0..=m {
        let s = -1.0 + i as f64 * h;
        let w = if i == 0 || i == m { 1.0 } else if i % 2 == 1 { 4.0 } else { 2.0 };
        sum += w * (g(s) - mx).exp();
        sum0 += w * (if s.abs() >= 1.0 { 0.0 } else { e * (1.0 - s * s).ln() }).exp();
    }
    mx + sum.ln() - sum0.ln()
}

/// psi_d on 0, dx, 2 dx, ... up to xmax.
struct Psi {
    dx: f64,
    v: Vec<f64>,
}
impl Psi {
    fn new(d: usize, xmax: f64, dx: f64) -> Psi {
        let n = (xmax / dx).ceil() as usize + 2;
        Psi { dx, v: (0..n).map(|k| log_mgf(d, k as f64 * dx, 8000)).collect() }
    }
    /// Linear interpolation (at least psi, which is convex).
    fn at(&self, x: f64) -> f64 {
        let f = x / self.dx;
        let k = (f.floor() as usize).min(self.v.len() - 2);
        let t = f - k as f64;
        self.v[k] * (1.0 - t) + self.v[k + 1] * t
    }
}

/// s with P[U >= s] = exp(-l1) for a coordinate U of a uniform unit vector in R^d (bisection on the tail integral).
fn tail_s(d: usize, l1: f64) -> f64 {
    let e = (d as f64 - 3.0) / 2.0;
    let m = 200_000;
    let h = 2.0 / m as f64;
    // log density on the grid, normalised by the trapezoid total, then the upper tail by cumulative sums from s = 1
    let lg: Vec<f64> = (0..=m).map(|i| {
        let s = -1.0 + i as f64 * h;
        if s.abs() >= 1.0 { f64::NEG_INFINITY } else { e * (1.0 - s * s).ln() }
    }).collect();
    let tot: f64 = lg.iter().map(|v| v.exp()).sum::<f64>() * h;
    let target = (-l1).exp() * tot / h;
    // smallest grid s whose tail (including the cell below it) is at most the target: rounding up keeps it sound
    let mut acc = 0.0;
    let mut i = m;
    while i > 0 {
        let a = acc + lg[i - 1].exp().max(lg[i].exp());
        if a > target {
            break;
        }
        acc = a;
        i -= 1;
    }
    -1.0 + i as f64 * h
}

/// c for the coded blocks of dimensions `ds` (each nb - 1) and l1 = c_subgaussian^2 / 2.
pub fn level_constant(ds: &[usize], l1: f64) -> f64 {
    let csub = (2.0 * l1).sqrt();
    if ds.is_empty() {
        return csub;
    }
    if ds.len() == 1 {
        return (tail_s(ds[0], l1) * (ds[0] as f64).sqrt() * (1.0 + 1e-6)).min(csub);
    }
    // max over shares of min over lambda is at most min over lambda of max over shares (lambda fixed, the sum is
    // separable): per lambda, each share rounded up to a multiple of 1/K, the largest sum over shares whose rounded
    // values sum to between K and K + J - 1 multiples, by dynamic programming over the blocks
    let j = ds.len();
    let lams: Vec<f64> = (0..64).map(|i| csub * 0.7 * (3.0f64 / 0.7).powf(i as f64 / 63.0)).collect();
    let lmax = lams[lams.len() - 1];
    let psis: Vec<Psi> = ds.iter().map(|&d| Psi::new(d, lmax * (d as f64 * 1.01).sqrt() + 1.0, 0.05)).collect();
    let k = 1000usize;
    let top = k + j - 1;
    let mut best = f64::INFINITY;
    for &lam in &lams {
        let mut acc = vec![f64::NEG_INFINITY; top + 1];
        acc[0] = 0.0;
        for b in 0..j {
            let g: Vec<f64> = (0..=k).map(|i| if i == 0 { 0.0 } else { psis[b].at(lam * (i as f64 / k as f64 * ds[b] as f64).sqrt()) }).collect();
            let mut nx = vec![f64::NEG_INFINITY; top + 1];
            for s0 in 0..=top {
                if acc[s0] == f64::NEG_INFINITY {
                    continue;
                }
                for i in 0..=k.min(top - s0) {
                    let v = acc[s0] + g[i];
                    if v > nx[s0 + i] {
                        nx[s0 + i] = v;
                    }
                }
            }
            acc = nx;
        }
        let v = acc[k..=top].iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        best = best.min((l1 + v) / lam);
    }
    (best * (1.0 + 1e-6)).min(csub)
}

/// Per randomised level, c for each coded-block set (cached by the set).
pub struct Constants {
    cache: HashMap<Vec<usize>, f64>,
    l1: f64,
}
impl Constants {
    pub fn new(csg: f64) -> Constants {
        Constants { cache: HashMap::new(), l1: csg * csg / 2.0 }
    }
    pub fn get(&mut self, ds: &[usize]) -> f64 {
        let l1 = self.l1;
        *self.cache.entry(ds.to_vec()).or_insert_with(|| level_constant(ds, l1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn single_block_matches_the_beta_tail() {
        // P[U >= s] = I_{1 - s^2}((d - 1) / 2, 1 / 2) / 2; at d = 95 and l1 = 43.526, s* = 0.76056 (scipy betaincinv)
        let s = tail_s(95, 43.525973215572655);
        assert!(s >= 0.76056 && s < 0.7606 + 1e-4, "{s}");
    }
    #[test]
    fn psi_is_below_the_subgaussian_bound() {
        for &x in &[1.0, 10.0, 50.0] {
            let p = log_mgf(127, x, 8000);
            assert!(p <= x * x / (2.0 * 127.0) + 1e-9 && p > 0.0);
        }
    }
}
