//! The randomised levels' constant c per (group, level), from the exact law of the error.
//!
//! In a coded block of nb positions the estimator's error is A <u, P Delta> with u uniform on the unit sphere of
//! y-perp, a space of d = nb - 1 dimensions, so the error over the coded blocks is X = sum_j a_j U_j with U_j a
//! coordinate of a uniform unit vector in R^{d_j}, independent across blocks, and sigma^2 = sum_j a_j^2 / d_j. A
//! half-width c sigma is valid at per-event probability delta when P[X >= c sigma] <= delta for every share vector
//! w_j = a_j^2 / (d_j sigma^2); the tail is non-decreasing in every a_j, so rounded-up scales are covered. Any upper
//! bound of the exact quantile is valid, and so is the smallest of several.
//!
//! - One coded block: P[U >= s] is the exact tail, so c = s* sqrt(d) with P[U >= s*] = delta.
//! - Several, three bounds, the smallest taken:
//!   - a certificate from the index's meta (`crate::certify`, or tools/certify.py): the worst case over shares of the
//!     exact tail, certified for the index's block dimensions at a ln delta at most this one. A certificate for a set
//!     of blocks covers every subset of it (the other scales are zero).
//!   - convex order against one large sphere: with D = sum_j d_j, m_j = E sqrt(Beta(d_j / 2, (D - d_j) / 2)) and
//!     s^2 = sum_j a_j^2 / m_j^2, P[X >= x] <= inf_{t < x} E (s U_D - t)_+ / (x - t), and s <= theta sqrt(D) sigma
//!     with theta^2 = max_j d_j / (D m_j^2).
//!   - Chernoff with each coordinate's exact log moment generating function psi_d (convex, increasing on x >= 0):
//!     P[X >= h] <= exp(-lambda h + sum_j psi_dj(lambda a_j)) for every lambda > 0. The constant must bound
//!     max_w min_lambda, which is at most min_lambda max_w. For a fixed lambda the sum is separable: each share is
//!     rounded up to a multiple of 1/K (psi is increasing, so the rounded share bounds every share in its cell) and the
//!     largest sum over rounded shares that add up to between 1 and 1 + (J - 1)/K is found by dynamic programming.
//!     psi is tabulated by numerical integration and read by linear interpolation, an upper bound for a convex
//!     function.
//!
//! Each result carries a 1e-6 relative margin and is capped by the sub-Gaussian sqrt(2 ln(1/delta)).

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

/// ln Gamma(x) for x > 0: Stirling's series after shifting x above 10 (absolute error below 1e-13).
fn ln_gamma(mut x: f64) -> f64 {
    let mut acc = 0.0;
    while x < 10.0 {
        acc -= x.ln();
        x += 1.0;
    }
    let x2 = x * x;
    acc + (x - 0.5) * x.ln() - x + 0.5 * (2.0 * std::f64::consts::PI).ln()
        + 1.0 / (12.0 * x) - 1.0 / (360.0 * x * x2) + 1.0 / (1260.0 * x2 * x2 * x) - 1.0 / (1680.0 * x2 * x2 * x2 * x)
}

/// ln E (U - t)_+ for a coordinate U of a uniform unit vector in R^d, 0 <= t < 1: Simpson's rule for the integral of
/// (u - t) (1 - u^2)^((d - 3) / 2) over [t, 1], times the density's normaliser.
fn ln_stoploss(d: usize, t: f64) -> f64 {
    let e = (d as f64 - 3.0) / 2.0;
    let lc = ln_gamma(d as f64 / 2.0) - 0.5 * std::f64::consts::PI.ln() - ln_gamma((d as f64 - 1.0) / 2.0);
    let m = 20_000;
    let h = (1.0 - t) / m as f64;
    let g = |u: f64| if u >= 1.0 || u <= t { f64::NEG_INFINITY } else { (u - t).ln() + e * (1.0 - u * u).ln() };
    let mx = (0..=m).map(|i| g(t + i as f64 * h)).fold(f64::NEG_INFINITY, f64::max);
    let sum: f64 = (0..=m).map(|i| {
        let w = if i == 0 || i == m { 1.0 } else if i % 2 == 1 { 4.0 } else { 2.0 };
        w * (g(t + i as f64 * h) - mx).exp()
    }).sum();
    lc + mx + (sum * h / 3.0).ln()
}

/// The convex-order bound for coded blocks of dimensions `ds` at P = exp(-l1), in units of sigma.
pub fn convex_order(ds: &[usize], l1: f64) -> f64 {
    let dd: usize = ds.iter().sum();
    let df = dd as f64;
    let theta2 = ds.iter().map(|&d| {
        let dj = d as f64;
        let m = (ln_gamma(dj / 2.0 + 0.5) + ln_gamma(df / 2.0) - ln_gamma(dj / 2.0) - ln_gamma(df / 2.0 + 0.5)).exp();
        dj / (df * m * m)
    }).fold(0.0, f64::max);
    // ln inf_t E (U_D - t)_+ / (x - t): any t gives a valid bound, so a grid in t refined by golden section suffices
    let bound = |x: f64| -> f64 {
        let f = |t: f64| ln_stoploss(dd, t) - (x - t).ln();
        let ts: Vec<f64> = (1..40).map(|i| x * i as f64 / 40.0).collect();
        let i = (0..ts.len()).min_by(|&a, &b| f(ts[a]).partial_cmp(&f(ts[b])).unwrap()).unwrap();
        let (mut lo, mut hi) = (if i == 0 { 0.0 } else { ts[i - 1] }, if i + 1 < ts.len() { ts[i + 1] } else { x * 0.999 });
        let gr = (5f64.sqrt() - 1.0) / 2.0;
        for _ in 0..40 {
            let (c, d) = (hi - gr * (hi - lo), lo + gr * (hi - lo));
            if f(c) < f(d) { hi = d } else { lo = c }
        }
        f((lo + hi) / 2.0).min(f(ts[i]))
    };
    // x <= 1: U_D never exceeds 1, so P[X >= s] = 0, and every t searched stays below 1
    let (mut lo, mut hi) = (1.0 / df.sqrt(), ((2.0 * l1 / df).sqrt() * 1.2).min(1.0));
    for _ in 0..50 {
        let mid = (lo + hi) / 2.0;
        if bound(mid) > -l1 { lo = mid } else { hi = mid }
    }
    hi * df.sqrt() * theta2.sqrt()
}

/// Chernoff with the exact moment generating functions, min over lambda of max over shares, in units of sigma.
fn chernoff(ds: &[usize], l1: f64) -> f64 {
    let csub = (2.0 * l1).sqrt();
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
    best
}

/// A certified worst-case constant: P[sum_j a_j U_j >= x sigma] <= exp(lnd) for every a >= 0 over blocks of
/// dimensions `ds` (`crate::certify`, tools/certify.py).
#[derive(Clone, Debug)]
pub struct Cert {
    pub ds: Vec<usize>,
    pub lnd: f64,
    pub x: f64,
}

/// Which bound gave a level's constant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Source {
    SubGaussian,
    ExactTail,
    Certificate,
    ConvexOrder,
    Chernoff,
}

/// c for the coded blocks of dimensions `ds` (each nb - 1) at per-event probability exp(-l1), and its source.
pub fn level_constant(ds: &[usize], l1: f64, certs: &[Cert]) -> (f64, Source) {
    let csub = (2.0 * l1).sqrt();
    let mut best = (csub, Source::SubGaussian);
    let mut take = |c: f64, s: Source| if c * (1.0 + 1e-6) < best.0 { best = (c * (1.0 + 1e-6), s) };
    match ds.len() {
        0 => {}
        1 => take(tail_s(ds[0], l1) * (ds[0] as f64).sqrt(), Source::ExactTail),
        _ => {
            // a certificate covers the subsets of its blocks, in block order
            let covers = |c: &Cert| {
                let mut it = c.ds.iter();
                ds.iter().all(|d| it.any(|e| e == d))
            };
            for c in certs.iter().filter(|c| c.lnd <= -l1 + 1e-12 && covers(c)) {
                take(c.x, Source::Certificate);
            }
            take(convex_order(ds, l1), Source::ConvexOrder);
            take(chernoff(ds, l1), Source::Chernoff);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    // ln(1/delta) at eps = 1e-9, L = 4: two-sided at N = 1e9, one-sided at N = 1e9 and N = 1e6
    const TWO_1E9: f64 = 43.525973215572655;
    const ONE_1E9: f64 = 42.832826035012715;
    const ONE_1E6: f64 = 35.92507075603058;
    const DS: [usize; 4] = [95, 127, 255, 287];
    #[test]
    fn single_block_matches_the_beta_tail() {
        // P[U >= s] = I_{1 - s^2}((d - 1) / 2, 1 / 2) / 2; at d = 95 and l1 = 43.526, s* = 0.76056 (scipy betaincinv)
        let s = tail_s(95, TWO_1E9);
        assert!(s >= 0.76056 && s < 0.7606 + 1e-4, "{s}");
        for (l1, want) in [(ONE_1E9, 7.373), (ONE_1E6, 6.927)] {
            let (c, src) = level_constant(&[95], l1, &[]);
            assert!(src == Source::ExactTail && (c - want).abs() < 1.5e-3, "{c} {want}");
        }
    }
    #[test]
    fn psi_is_below_the_subgaussian_bound() {
        for &x in &[1.0, 10.0, 50.0] {
            let p = log_mgf(127, x, 8000);
            assert!(p <= x * x / (2.0 * 127.0) + 1e-9 && p > 0.0);
        }
    }
    #[test]
    fn ln_gamma_matches_known_values() {
        for (x, want) in [(0.5f64, std::f64::consts::PI.sqrt().ln()), (1.0, 0.0), (5.0, 24f64.ln()), (383.5, 1896.0162305991828)] {
            assert!((ln_gamma(x) - want).abs() < 1e-9 * (1.0 + want.abs()), "{x}");
        }
    }
    #[test]
    fn four_block_constants_match_the_report() {
        // convex order 8.885 two-sided and 8.815 one-sided; Chernoff 9.086 and 9.018 (REPORT.txt section 3)
        for (l1, cx, ch) in [(TWO_1E9, 8.885, 9.086), (ONE_1E9, 8.815, 9.018)] {
            let a = convex_order(&DS, l1);
            assert!((a - cx).abs() < 1.5e-3, "convex order {a} against {cx}");
            let b = chernoff(&DS, l1);
            assert!((b - ch).abs() < 1.5e-3, "Chernoff {b} against {ch}");
            assert_eq!(level_constant(&DS, l1, &[]).1, Source::ConvexOrder);
        }
    }
    /// Blocks of so few dimensions that sqrt(2 l1 / D) passes 1: the bound is finite and at most theta sqrt(D) (x = 1,
    /// which U_D never exceeds), and the level's constant no larger than the sub-Gaussian one.
    #[test]
    fn convex_order_of_small_blocks_is_finite() {
        for ds in [vec![3usize, 7], vec![1, 1], vec![2, 5, 9], vec![15, 15]] {
            let a = convex_order(&ds, ONE_1E9);
            let d = ds.iter().sum::<usize>() as f64;
            assert!(a.is_finite() && a > 0.0, "{ds:?}: {a}");
            let theta2 = ds.iter().map(|&dj| {
                let dj = dj as f64;
                let m = (ln_gamma(dj / 2.0 + 0.5) + ln_gamma(d / 2.0) - ln_gamma(dj / 2.0) - ln_gamma(d / 2.0 + 0.5)).exp();
                dj / (d * m * m)
            }).fold(0.0, f64::max);
            assert!(a <= (d * theta2).sqrt() * (1.0 + 1e-9), "{ds:?}: {a} past x = 1");
            let (c, _) = level_constant(&ds, ONE_1E9, &[]);
            assert!(c.is_finite() && c <= (2.0 * ONE_1E9).sqrt(), "{ds:?}: {c}");
        }
    }
    #[test]
    fn a_certificate_covers_its_subsets_at_its_budget_or_looser() {
        let cert = Cert { ds: DS.to_vec(), lnd: -ONE_1E9, x: 8.704 };
        let (c, src) = level_constant(&DS, ONE_1E9, std::slice::from_ref(&cert));
        assert!(src == Source::Certificate && (c - 8.704).abs() < 1e-4);
        // which sets a certificate covers (its value here is not a real one)
        let low = Cert { x: 5.0, ..cert };
        let by = |ds: &[usize], l1: f64| level_constant(ds, l1, std::slice::from_ref(&low)).1;
        assert_eq!(by(&[95, 255, 287], ONE_1E9), Source::Certificate);
        assert_eq!(by(&DS, ONE_1E6), Source::Certificate);
        // a tighter budget than the certificate's, or blocks it does not have, fall back
        assert_ne!(by(&DS, TWO_1E9), Source::Certificate);
        assert_ne!(by(&[95, 128], ONE_1E9), Source::Certificate);
        assert_ne!(by(&[255, 95], ONE_1E9), Source::Certificate);
    }
}
