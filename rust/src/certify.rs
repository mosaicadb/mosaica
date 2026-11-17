//! The randomised levels' worst-case constant certified for a block layout: the method of `tools/certify.py`, ported
//! so that the fit needs no Python, plus a shipped table of certificates for common layouts (`certs_table.json`).
//!
//! Statement certified, for block dimensions d_j = nb_j - 1 and a ln delta:
//!   P[sum_j a_j U_j >= X] <= delta  for every a >= 0 with sum_j a_j^2 / d_j = 1,
//! U_j a coordinate of a uniform unit vector in R^{d_j}, independent. The build (`v2::tail::level_constant`) uses X as
//! the constant of every level whose coded blocks are a subset of these, at every budget at least as loose.
//!
//! Method, as certify.py (reports/bounds3/REPORT.txt, R3):
//!  (1) the tail is non-decreasing in every a_j, so shares may be rounded up: at grid step h = 1/K every share vector
//!      lies in a cell whose corner dominates it; cells whose corner fails are split and retested at 2K, up to Kmax;
//!  (2) a_j U_j <= eta ceil(a_j U_j / eta), so the tail is at most that of a lattice sum;
//!  (3) the lattice tail is an exact finite sum, evaluated through the discrete Fourier transform of the exponentially
//!      tilted probabilities; the frequencies left out are bounded by the block with the smallest remainder.
//! Margins REL (relative) and ABS (absolute) cover quadrature and rounding.
//!
//! The arithmetic follows numpy's where it is cheap to: lgamma is cephes' (scipy's `gammaln`, bit for bit), sums of
//! vectors are numpy's pairwise sums, complex products numpy's formula, and the quadrature nodes are numpy's
//! `leggauss(8)` values. The FFT (rustfft rather than pocketfft) and the dot products of the four-block cells (BLAS in
//! numpy) differ in their last bits; that moves a cell's log bound by ~1e-10 at most, so only a cell within that of the
//! budget could pass in one and fail in the other. tools/certs_table.py certifies every shipped entry with both and
//! stores only those where they agree; the tests compare against certify.py's values on a grid of layouts.
use rayon::prelude::*;
use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The margins: relative, on the bound, and absolute, on the lattice sum.
pub const REL: f64 = 1e-6;
pub const ABS: f64 = 1e-11;
/// numpy.polynomial.legendre.leggauss(8), as numpy computes them.
const GLX: [f64; 8] = [
    -0.9602898564975362, -0.7966664774136267, -0.525532409916329, -0.18343464249564978,
    0.18343464249564978, 0.525532409916329, 0.7966664774136267, 0.9602898564975362,
];
const GLW: [f64; 8] = [
    0.10122853629037706, 0.22238103445337443, 0.3137066458778869, 0.36268378337836166,
    0.36268378337836166, 0.3137066458778869, 0.22238103445337443, 0.10122853629037706,
];

fn polevl(x: f64, c: &[f64]) -> f64 {
    c[1..].iter().fold(c[0], |a, &v| a * x + v)
}
fn p1evl(x: f64, c: &[f64]) -> f64 {
    c[1..].iter().fold(x + c[0], |a, &v| a * x + v)
}

/// ln Gamma(x) for x > 0, as cephes `lgam` (scipy.special.gammaln) computes it.
pub fn lgamma(x: f64) -> f64 {
    const A: [f64; 5] = [8.11614167470508450300E-4, -5.95061904284301438324E-4, 7.93650340457716943945E-4, -2.77777777730099687205E-3, 8.33333333333331927722E-2];
    const B: [f64; 6] = [-1.37825152569120859100E3, -3.88016315134637840924E4, -3.31612992738871184744E5, -1.16237097492762307383E6, -1.72173700820839662146E6, -8.53555664245765465627E5];
    const C: [f64; 6] = [-3.51815701436523470549E2, -1.70642106651881159223E4, -2.20528590553854454839E5, -1.13933444367982507207E6, -2.53252307177582951285E6, -2.01889141433532773231E6];
    const LS2PI: f64 = 0.91893853320467274178;
    assert!(x > 0.0, "lgamma of {x}");
    if x < 13.0 {
        let (mut z, mut p, mut u) = (1.0, 0.0, x);
        while u >= 3.0 {
            p -= 1.0;
            u = x + p;
            z *= u;
        }
        while u < 2.0 {
            z /= u;
            p += 1.0;
            u = x + p;
        }
        if u == 2.0 {
            return z.ln();
        }
        p -= 2.0;
        let x = x + p;
        return z.ln() + x * polevl(x, &B) / p1evl(x, &C);
    }
    let mut q = (x - 0.5) * x.ln() - x + LS2PI;
    if x > 1.0e8 {
        return q;
    }
    let p = 1.0 / (x * x);
    if x >= 1000.0 {
        q += ((7.9365079365079365079365e-4 * p - 2.7777777777777777777778e-3) * p + 0.0833333333333333333333) / x;
    } else {
        q += polevl(p, &A) / x;
    }
    q
}

/// The sum of `a` as numpy sums a vector of float64 (pairwise, blocks of 128 unrolled by 8).
pub fn np_sum(a: &[f64]) -> f64 {
    let n = a.len();
    if n < 8 {
        a.iter().fold(-0.0, |r, &v| r + v)
    } else if n <= 128 {
        let mut r = [0.0; 8];
        r.copy_from_slice(&a[..8]);
        let mut i = 8;
        while i < n - n % 8 {
            for j in 0..8 {
                r[j] += a[i + j];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        while i < n {
            res += a[i];
            i += 1;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        np_sum(&a[..n2]) + np_sum(&a[n2..])
    }
}

/// q(k) = P[Y = k eta] exp(lam k eta) for Y = eta ceil(a U / eta), U a coordinate of a uniform unit vector in R^d,
/// k = kmin..kmax: (kmin, q). Each lattice cell's probability by 8-point Gauss-Legendre.
pub fn tilted_pmf(d: usize, a: f64, eta: f64, lam: f64) -> (i64, Vec<f64>) {
    if a <= 0.0 {
        return (0, vec![1.0]);
    }
    let kmax = (a / eta).ceil() as i64;
    let kmin = (-a / eta).ceil() as i64;
    let df = d as f64;
    let lc = lgamma(df / 2.0) - 0.5 * std::f64::consts::PI.ln() - lgamma((df - 1.0) / 2.0);
    let e = (df - 3.0) / 2.0;
    let le = lam * eta;
    let q = (kmin..=kmax).map(|k| {
        let lo = (((k - 1) as f64 * eta) / a).clamp(-1.0, 1.0);
        let hi = ((k as f64 * eta) / a).clamp(-1.0, 1.0);
        let (mid, half) = ((lo + hi) / 2.0, (hi - lo) / 2.0);
        let mut t = [0.0; 8];
        for i in 0..8 {
            let s = mid + half * GLX[i];
            let lf = lc + e * (-s * s).ln_1p();
            t[i] = (lf + le * k as f64).exp() * GLW[i];
        }
        np_sum(&t) * half
    }).collect();
    (kmin, q)
}

/// numpy's complex product.
#[inline]
fn cmul(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}

/// One block's row for one share: ln of its tilted mass, the normalised transform's first k0 + 1 frequencies, and
/// the bound on what the frequencies left out contribute.
struct Row {
    lnm: f64,
    re: Vec<f64>,
    im: Vec<f64>,
    r: f64,
}

/// The constants of one certification.
#[derive(Clone, Copy, Debug)]
pub struct Grid {
    /// The first and the finest share grid (1/K0 .. 1/Kmax).
    pub k0_grid: usize,
    pub kmax: usize,
    /// The lattice step.
    pub eta: f64,
    /// The transform length and the frequencies summed exactly.
    pub m: usize,
    pub k0: usize,
}
impl Grid {
    /// certify.py's defaults: M = 65536, k0 = 400, or 131072 and 800 above 1,100 dimensions in all.
    pub fn for_dims(ds: &[usize]) -> Grid {
        let big = ds.iter().sum::<usize>() > 1100;
        Grid { k0_grid: 64, kmax: 4096, eta: 0.001, m: if big { 131072 } else { 65536 }, k0: if big { 800 } else { 400 } }
    }
}

/// Per-block rows built on demand, keyed by the share's numerator over Kmax.
struct Tables<'a> {
    ds: &'a [usize],
    g: Grid,
    lam: f64,
    absk: Vec<f64>,
    fft: Arc<dyn RealToComplex<f64>>,
    key: Vec<HashMap<i64, usize>>,
    rows: Vec<Vec<Row>>,
}
impl<'a> Tables<'a> {
    fn row_for(&self, j: usize, f: i64) -> Row {
        let (m, k0) = (self.g.m, self.g.k0);
        let a = ((f as f64 / self.g.kmax as f64) * self.ds[j] as f64).sqrt();
        let (kmin, q) = tilted_pmf(self.ds[j], a, self.g.eta, self.lam);
        let tot = np_sum(&q);
        let mut arr = vec![0.0; m];
        for (i, &v) in q.iter().enumerate() {
            arr[(kmin + i as i64).rem_euclid(m as i64) as usize] = v / tot;
        }
        let mut out = self.fft.make_output_vec();
        self.fft.process(&mut arr, &mut out).expect("rfft");
        let tail: Vec<f64> = (k0 + 1..m / 2).map(|t| out[t].re.hypot(out[t].im) * self.absk[t]).collect();
        let r = (2.0 * np_sum(&tail) + out[m / 2].re.hypot(out[m / 2].im) * self.absk[m / 2]) / m as f64;
        Row { lnm: tot.ln(), re: out[..=k0].iter().map(|c| c.re).collect(), im: out[..=k0].iter().map(|c| c.im).collect(), r }
    }
    /// Build the rows of block j for these numerators on the grid 1/K (in parallel; each row is independent).
    fn ensure(&mut self, j: usize, k: usize, idx: &[bool]) {
        let mult = (self.g.kmax / k) as i64;
        let new: Vec<i64> = (0..idx.len() as i64).filter(|&i| idx[i as usize]).map(|i| i * mult).filter(|f| !self.key[j].contains_key(f)).collect();
        let built: Vec<Row> = new.par_iter().map(|&f| self.row_for(j, f)).collect();
        for (f, r) in new.into_iter().zip(built) {
            self.key[j].insert(f, self.rows[j].len());
            self.rows[j].push(r);
        }
    }
    fn get(&self, j: usize, k: usize, i: i64) -> &Row {
        &self.rows[j][self.key[j][&(i * (self.g.kmax / k) as i64)]]
    }
}

/// The layouts and budgets a certificate is for: 2 to 4 blocks of at least 4 dimensions each (the coordinate density
/// (1 - s^2)^((d - 3) / 2) is singular or degenerate at the ends below that; the fit's blocks have at least 15), and a
/// ln delta that is a negative number (a NaN would let every cell pass).
fn check_layout(ds: &[usize], lnd: f64) -> Result<(), String> {
    if !(2..=4).contains(&ds.len()) {
        return Err(format!("a certificate is for 2 to 4 blocks, not {}", ds.len()));
    }
    if let Some(d) = ds.iter().find(|&&d| d < 4) {
        return Err(format!("a block has {d} dimensions; a certificate is for blocks of at least 4"));
    }
    if !(lnd < 0.0) || !lnd.is_finite() {
        return Err(format!("ln delta is a negative number, not {lnd}"));
    }
    Ok(())
}

/// Whether the lattice upper bound of the tail at X is at most exp(lnd) on every cell of the share simplex: Ok(false)
/// when a cell still fails at Kmax or the deadline passes; an error when the grid cannot hold the layout (certify.py
/// asserts). A cell whose sum is not a number fails here (in numpy it would pass); none has been seen.
pub fn certify(ds: &[usize], lnd: f64, x: f64, deadline: Instant, g: &Grid) -> Result<bool, String> {
    check_layout(ds, lnd)?;
    let j = ds.len();
    if !x.is_finite() || x <= 0.0 {
        return Err(format!("X is a positive number, not {x}"));
    }
    let (eta, m, k0) = (g.eta, g.m, g.k0);
    let lam = x;
    let mx = (x / eta).round_ties_even() as i64;
    if (mx as f64 * eta - x).abs() >= 1e-9 {
        return Err(format!("X {x} is not a multiple of {eta}"));
    }
    if !(((ds.iter().sum::<usize>() as f64) * (1.0 + (j - 1) as f64 / g.k0_grid as f64)).sqrt() / eta + (j as f64) < (m / 2) as f64) {
        return Err(format!("the lattice window ({m}) is too small for blocks {ds:?}"));
    }
    let fft = RealFftPlanner::<f64>::new().plan_fft_forward(m);
    let mut om = vec![0.0; m];
    for mm in mx..(m / 2) as i64 {
        om[mm as usize] = (-lam * (mm - mx) as f64 * eta).exp();
    }
    let mut kap = fft.make_output_vec();
    fft.process(&mut om, &mut kap).expect("rfft");
    // conj
    let kap: Vec<Complex<f64>> = kap.into_iter().map(|c| Complex::new(c.re, -c.im)).collect();
    let absk: Vec<f64> = kap.iter().map(|c| c.re.hypot(c.im)).collect();
    let kw: Vec<(f64, f64)> = (0..=k0).map(|t| {
        let w = if t == 0 { 1.0 } else { 2.0 };
        (kap[t].re * w / m as f64, kap[t].im * w / m as f64)
    }).collect();
    let mut tb = Tables { ds, g: *g, lam, absk, fft, key: vec![HashMap::new(); j], rows: (0..j).map(|_| Vec::new()).collect() };
    let base = -lam * mx as f64 * eta + REL.ln_1p();
    // a cell's log bound from its lattice sum, its remainder and its log masses; a sum that is not a number fails
    let lt_of = |s: f64, rmin: f64, ls: f64| -> f64 {
        let v = s + rmin + ABS;
        if v.is_nan() { f64::INFINITY } else { v.max(1e-300).ln() + ls + base }
    };
    let mut k = g.k0_grid;
    let mut bx: Vec<(i64, i64)> = vec![(0, (k + j - 1) as i64); j - 1];
    loop {
        if Instant::now() > deadline {
            return Ok(false);
        }
        let top = (k + j - 1) as i64;
        // on the finest grid one failing cell settles it: the rest of the cells are skipped once one is found
        let finest = k >= g.kmax;
        let stop = AtomicBool::new(false);
        let stop = &stop;
        // the failing cells' extent in each free coordinate (all that the refinement needs of them)
        let fails: Fails = if j == 4 {
            let ((lo1, hi1), (lo2, hi2), (lo3, hi3)) = (bx[0], bx[1], bx[2]);
            // per s = i1 + i2: the i1 and the i3 that fit
            let groups: Vec<(i64, (i64, i64), (i64, i64))> = (lo1 + lo2..=hi1 + hi2).filter_map(|s| {
                let r1 = (lo1.max(s - hi2), hi1.min(s - lo2));
                let r3 = (lo3, hi3.min(top - s - 1));
                if r1.0 > r1.1 || r3.0 > r3.1 { None } else { Some((s, r1, r3)) }
            }).collect();
            let mut need = vec![vec![false; top as usize + 1]; 4];
            for &(s, (a0, a1), (c0, c1)) in &groups {
                for v in a0..=a1 {
                    need[0][v as usize] = true;
                    need[1][(s - v) as usize] = true;
                }
                for v in c0..=c1 {
                    need[2][v as usize] = true;
                    need[3][(top - s - v) as usize] = true;
                }
            }
            for (b, nd) in need.iter().enumerate() {
                tb.ensure(b, k, nd);
            }
            let tb = &tb;
            let kw = &kw;
            groups.par_iter().map(|&(s, (a0, a1), (c0, c1))| {
                if finest && stop.load(Ordering::Relaxed) {
                    return Fails::new(3);
                }
                // A = P1 P2 kw by i1, B = P3 P4 by i3 (real and imaginary parts, rows of k0 + 1); then, as numpy,
                // S = Re A Re B^T - Im A Im B^T, in blocks of i1
                let n = k0 + 1;
                let n3 = (c1 - c0 + 1) as usize;
                let (mut bre, mut bim, mut lb, mut rb) = (vec![0.0; n3 * n], vec![0.0; n3 * n], vec![0.0; n3], vec![0.0; n3]);
                for (ib, v) in (c0..=c1).enumerate() {
                    let (r3, r4) = (tb.get(2, k, v), tb.get(3, k, top - s - v));
                    for t in 0..n {
                        let p = cmul((r3.re[t], r3.im[t]), (r4.re[t], r4.im[t]));
                        bre[ib * n + t] = p.0;
                        bim[ib * n + t] = p.1;
                    }
                    lb[ib] = r3.lnm + r4.lnm;
                    rb[ib] = r3.r.min(r4.r);
                }
                let mut f = Fails::new(3);
                let mut b0 = a0;
                while b0 <= a1 && !(finest && stop.load(Ordering::Relaxed)) {
                    let b1 = a1.min(b0 + 127);
                    let n1 = (b1 - b0 + 1) as usize;
                    let (mut are, mut aim, mut la, mut ra) = (vec![0.0; n1 * n], vec![0.0; n1 * n], vec![0.0; n1], vec![0.0; n1]);
                    for (ia, v) in (b0..=b1).enumerate() {
                        let (r1, r2) = (tb.get(0, k, v), tb.get(1, k, s - v));
                        for t in 0..n {
                            let p = cmul(cmul((r1.re[t], r1.im[t]), (r2.re[t], r2.im[t])), kw[t]);
                            are[ia * n + t] = p.0;
                            aim[ia * n + t] = p.1;
                        }
                        la[ia] = r1.lnm + r2.lnm;
                        ra[ia] = r1.r.min(r2.r);
                    }
                    let (sr, si) = (gemm_nt(&are, &bre, n1, n3, n), gemm_nt(&aim, &bim, n1, n3, n));
                    for ia in 0..n1 {
                        for ib in 0..n3 {
                            // as numpy: log(S + min R + ABS) + (L1 + L2) + (L3 + L4) + base
                            let v = sr[ia * n3 + ib] - si[ia * n3 + ib] + ra[ia].min(rb[ib]) + ABS;
                            let lt = if v.is_nan() { f64::INFINITY } else { v.max(1e-300).ln() + la[ia] + lb[ib] + base };
                            if lt > lnd {
                                let i1 = b0 + ia as i64;
                                f.add(&[i1, s - i1, c0 + ib as i64]);
                                if finest {
                                    stop.store(true, Ordering::Relaxed);
                                }
                            }
                        }
                    }
                    b0 = b1 + 1;
                }
                f
            }).reduce(|| Fails::new(3), Fails::merge)
        } else {
            // the tuples of the free coordinates in the box whose last share is >= 1, by their first coordinate
            let mut need = vec![vec![false; top as usize + 1]; j];
            let each = |first: i64, f: &mut dyn FnMut(&[i64], i64)| {
                let mut p = vec![first];
                p.extend(bx[1..].iter().map(|b| b.0));
                loop {
                    let last = top - p.iter().sum::<i64>();
                    if last >= 1 {
                        f(&p, last);
                    }
                    // the next tuple, the last free coordinate fastest (itertools.product order)
                    let mut c = j - 2;
                    loop {
                        if c == 0 {
                            return;
                        }
                        if p[c] < bx[c].1 {
                            p[c] += 1;
                            break;
                        }
                        p[c] = bx[c].0;
                        c -= 1;
                    }
                }
            };
            for first in bx[0].0..=bx[0].1 {
                each(first, &mut |p, last| {
                    for (b, &v) in p.iter().enumerate() {
                        need[b][v as usize] = true;
                    }
                    need[j - 1][last as usize] = true;
                });
            }
            for (b, nd) in need.iter().enumerate() {
                tb.ensure(b, k, nd);
            }
            let tb = &tb;
            let kw = &kw;
            (bx[0].0..=bx[0].1).into_par_iter().map(|first| {
                let mut f = Fails::new(j - 1);
                let mut ps: Vec<(f64, f64)> = Vec::with_capacity(k0 + 1);
                let mut re = vec![0.0; k0 + 1];
                each(first, &mut |p, last| {
                    if finest && stop.load(Ordering::Relaxed) {
                        return;
                    }
                    ps.clear();
                    ps.extend_from_slice(kw);
                    let (mut ls, mut rs) = (0.0, f64::INFINITY);
                    for b in 0..j {
                        let r = tb.get(b, k, if b < j - 1 { p[b] } else { last });
                        for (t, v) in ps.iter_mut().enumerate() {
                            *v = cmul(*v, (r.re[t], r.im[t]));
                        }
                        ls += r.lnm;
                        rs = if b == 0 { r.r } else { rs.min(r.r) };
                    }
                    for (t, v) in ps.iter().enumerate() {
                        re[t] = v.0;
                    }
                    if lt_of(np_sum(&re), rs, ls) > lnd {
                        f.add(p);
                        if finest {
                            stop.store(true, Ordering::Relaxed);
                        }
                    }
                });
                f
            }).reduce(|| Fails::new(j - 1), Fails::merge)
        };
        if fails.count == 0 {
            return Ok(true);
        }
        if finest {
            return Ok(false);
        }
        bx = fails.lo.iter().zip(&fails.hi).map(|(&lo, &hi)| (0.max(2 * lo - 1), 2 * hi)).collect();
        k *= 2;
    }
}

/// How many cells failed, and their least and greatest index in each free coordinate.
struct Fails {
    count: usize,
    lo: Vec<i64>,
    hi: Vec<i64>,
}
impl Fails {
    fn new(c: usize) -> Fails {
        Fails { count: 0, lo: vec![i64::MAX; c], hi: vec![i64::MIN; c] }
    }
    fn add(&mut self, p: &[i64]) {
        self.count += 1;
        for (c, &v) in p.iter().enumerate() {
            self.lo[c] = self.lo[c].min(v);
            self.hi[c] = self.hi[c].max(v);
        }
    }
    fn merge(mut a: Fails, b: Fails) -> Fails {
        a.count += b.count;
        for c in 0..a.lo.len() {
            a.lo[c] = a.lo[c].min(b.lo[c]);
            a.hi[c] = a.hi[c].max(b.hi[c]);
        }
        a
    }
}

/// A B^T for A (m x k) and B (n x k), row-major.
fn gemm_nt(a: &[f64], b: &[f64], m: usize, n: usize, k: usize) -> Vec<f64> {
    let mut c = vec![0.0; m * n];
    // SAFETY: a holds m x k, b n x k and c m x n values, row-major, as the strides say
    unsafe { matrixmultiply::dgemm(m, k, n, 1.0, a.as_ptr(), k as isize, 1, b.as_ptr(), 1, k as isize, 0.0, c.as_mut_ptr(), n as isize, 1) };
    c
}

/// The continued fraction of the regularised incomplete beta function (modified Lentz).
fn betacf(a: f64, b: f64, x: f64) -> f64 {
    const FPMIN: f64 = 1e-300;
    let (qab, qap, qam) = (a + b, a + 1.0, a - 1.0);
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < FPMIN {
        d = FPMIN;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..100_000 {
        let m = m as f64;
        let m2 = 2.0 * m;
        for aa in [m * (b - m) * x / ((qam + m2) * (a + m2)), -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2))] {
            d = 1.0 + aa * d;
            if d.abs() < FPMIN {
                d = FPMIN;
            }
            c = 1.0 + aa / c;
            if c.abs() < FPMIN {
                c = FPMIN;
            }
            d = 1.0 / d;
            h *= d * c;
        }
        // the last factor d c
        if (d * c - 1.0).abs() <= f64::EPSILON {
            break;
        }
    }
    h
}

/// ln B(a, b). With the larger argument at least 20, lgamma(big) - lgamma(big + small) from Stirling's series of each
/// (the large terms cancelled analytically), since as a difference of two lgammas of ~10^4 it would lose ~1e-12.
fn ln_beta(a: f64, b: f64) -> f64 {
    let (big, small) = if a >= b { (a, b) } else { (b, a) };
    if big < 20.0 {
        return lgamma(a) + lgamma(b) - lgamma(a + b);
    }
    // the remainder of Stirling's series, lgamma(x) - ((x - 1/2) ln x - x + ln sqrt(2 pi)), for x >= 20 (to ~1e-17)
    let corr = |x: f64| {
        let x2 = x * x;
        (1.0 / 12.0 - (1.0 / 360.0 - (1.0 / 1260.0 - (1.0 / 1680.0 - 1.0 / (1188.0 * x2)) / x2) / x2) / x2) / x
    };
    let s = big + small;
    lgamma(small) - (big - 0.5) * (small / big).ln_1p() - small * s.ln() + small + corr(big) - corr(s)
}

/// ln I_x(a, b), the regularised incomplete beta function, accurate in relative terms however small it is.
pub fn ln_betainc(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if x >= 1.0 {
        return 0.0;
    }
    let lbt = -ln_beta(a, b) + a * x.ln() + b * (-x).ln_1p();
    if x < (a + 1.0) / (a + b + 2.0) {
        lbt + betacf(a, b, x).ln() - a.ln()
    } else {
        (-(lbt + betacf(b, a, 1.0 - x).ln() - b.ln()).exp()).ln_1p()
    }
}

/// I_x(a, b).
pub fn betainc(a: f64, b: f64, x: f64) -> f64 {
    ln_betainc(a, b, x).exp()
}

/// x with I_x(a, b) = y (bisection on ln I to the last representable step).
pub fn betaincinv(a: f64, b: f64, y: f64) -> f64 {
    if y <= 0.0 {
        return 0.0;
    }
    if y >= 1.0 {
        return 1.0;
    }
    let ly = y.ln();
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    loop {
        let mid = 0.5 * (lo + hi);
        if mid <= lo || mid >= hi {
            break;
        }
        if ln_betainc(a, b, mid) < ly {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    // the nearer end
    if (ln_betainc(a, b, lo) - ly).abs() <= (ln_betainc(a, b, hi) - ly).abs() { lo } else { hi }
}

/// c with P[U >= c / sqrt(D)] = exp(lnd) for a coordinate U of a uniform unit vector in R^D.
pub fn sphere_constant(dd: usize, lnd: f64) -> f64 {
    (1.0 - betaincinv((dd as f64 - 1.0) / 2.0, 0.5, 2.0 * lnd.exp())).sqrt() * (dd as f64).sqrt()
}

/// The budget the build uses for an index of `items` items with `levels` randomised levels: ln of EPS / (N L), N the
/// item count rounded up to a power of two (certify.py `ln_delta`).
pub fn ln_delta(items: usize, levels: usize, eps: f64) -> f64 {
    let n = items.max(1).next_power_of_two();
    -(((n * levels) as f64) / eps).ln()
}

/// The smallest X on a 0.001 lattice, from the one-sphere prediction in sum_j (d_j + 2) - 2 dimensions plus 0.008,
/// that certifies (six tries, 0.002 apart), or None within `time_cap` seconds (or when the grid cannot hold the
/// layout).
pub fn certify_layout(ds: &[usize], lnd: f64, time_cap: f64, log: &mut dyn FnMut(String)) -> Option<f64> {
    if let Err(e) = check_layout(ds, lnd) {
        log(format!("certify {ds:?} ln delta {lnd:.4}: {e}"));
        return None;
    }
    if !(time_cap >= 0.0) {
        log(format!("certify {ds:?}: the time cap is a number of seconds, at least 0, not {time_cap}"));
        return None;
    }
    let t0 = Instant::now();
    let deadline = t0 + Duration::from_secs_f64(time_cap.min(1e9));
    let g = Grid::for_dims(ds);
    let pred = sphere_constant(ds.iter().sum::<usize>() + 2 * ds.len() - 2, lnd);
    let m0 = ((pred + 0.008) * 1000.0).ceil() as i64;
    for i in 0..6 {
        let x = (m0 + 2 * i) as f64 / 1000.0;
        let ok = match certify(ds, lnd, x, deadline, &g) {
            Ok(ok) => ok,
            Err(e) => {
                log(format!("certify {ds:?} ln delta {lnd:.4}: {e}"));
                return None;
            }
        };
        log(format!("certify {ds:?} ln delta {lnd:.4}: X {x:.3} {} [{:.0}s]", if ok { "certified" } else { "not certified" }, t0.elapsed().as_secs_f64()));
        if ok {
            return Some(x);
        }
        if Instant::now() > deadline {
            return None;
        }
    }
    None
}

/// The shipped certificates (`certs_table.json`, made by tools/certs_table.py with both this port and certify.py):
/// (ds, lnd, X), X None where both searches find no certificate.
pub fn table() -> &'static [(Vec<usize>, f64, Option<f64>)] {
    static T: std::sync::OnceLock<Vec<(Vec<usize>, f64, Option<f64>)>> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let v: serde_json::Value = serde_json::from_str(include_str!("certs_table.json")).expect("certs_table.json");
        v.as_array().expect("certs_table.json is a list").iter().map(|c| {
            let ds = c["ds"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize).collect();
            (ds, c["lnd"].as_f64().unwrap(), if c["X"].is_null() { None } else { Some(c["X"].as_f64().unwrap()) })
        }).collect()
    })
}

/// The shipped entry for exactly these blocks and this budget, if there is one: Some(X), or Some(None) when the search
/// finds no certificate for them.
pub fn lookup(ds: &[usize], lnd: f64) -> Option<Option<f64>> {
    table().iter().find(|(d, l, _)| d == ds && l.to_bits() == lnd.to_bits()).map(|e| e.2)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// scipy's values and certify.py's certificates (tools/certify_fixture.py).
    fn fixture() -> serde_json::Value {
        let p = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/certify_fixture.json");
        serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
    }
    fn rows(v: &serde_json::Value) -> Vec<Vec<f64>> {
        v.as_array().unwrap().iter().map(|r| r.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect()).collect()
    }

    #[test]
    fn lgamma_is_scipys_gammaln() {
        let f = fixture();
        for r in rows(&f["lgamma"]) {
            assert_eq!(lgamma(r[0]).to_bits(), r[1].to_bits(), "lgamma({}) = {} not {}", r[0], lgamma(r[0]), r[1]);
        }
    }

    #[test]
    fn betainc_and_its_inverse_match_scipy() {
        let f = fixture();
        for r in rows(&f["betainc"]) {
            let got = betainc(r[0], r[1], r[2]);
            // (values below the doubles' range are 0 in both; a tiny value carries the rounding of its log, |ln I| ulps,
            // and the continued fraction's terms add up to ~1e-13 near its switch point x = (a + 1) / (a + b + 2))
            assert!(if r[3] == 0.0 { got == 0.0 } else { (got.ln() - r[3].ln()).abs() < 5e-13 + 2e-15 * r[3].ln().abs() }, "I_{}({}, {}) = {got:e} not {:e}", r[2], r[0], r[1], r[3]);
        }
        for r in rows(&f["betaincinv"]) {
            // the inverse is checked where it is used, through 1 - x (the share of the tail), and through x
            let got = betaincinv(r[0], r[1], r[2]);
            assert!(((1.0 - got) - (1.0 - r[3])).abs() <= 1e-13 * (1.0 - r[3]).max(1e-3), "inverse at a {} y {:e}: {got} not {}", r[0], r[2], r[3]);
            assert!((got - r[3]).abs() <= 1e-13 * r[3], "inverse at a {} y {:e}: {got} not {}", r[0], r[2], r[3]);
        }
        for r in rows(&f["sphere"]) {
            let got = sphere_constant(r[0] as usize, r[1]);
            assert!(((got - r[2]) / r[2]).abs() < 1e-12, "sphere constant at D {} lnd {}: {got} not {}", r[0], r[1], r[2]);
        }
    }

    #[test]
    fn numpy_pairwise_sum() {
        // a sum whose value depends on the order: numpy gives 1.0 (pairwise) where a running sum gives 0.0
        let mut a = vec![1.0, 1e16, -1e16];
        a.extend(vec![0.0; 6]);
        assert_eq!(np_sum(&a[..3]), 0.0);
        let mut b = vec![0.0; 16];
        b[0] = 1e16;
        b[1] = 1.0;
        b[8] = -1e16;
        // r0 = 1e16 + -1e16 = 0, r1 = 1: ((0 + 1) + ...) = 1
        assert_eq!(np_sum(&b), 1.0);
        let c: Vec<f64> = (0..1000).map(|i| (i as f64 * 0.37).sin()).collect();
        assert!((np_sum(&c) - c.iter().sum::<f64>()).abs() < 1e-12);
    }

    #[test]
    fn the_tilted_lattice_law_is_a_probability() {
        // lam = 0: the cells' probabilities add up to one; a <= 0: all the mass at 0
        for (d, a) in [(15, 3.0), (95, 9.7), (287, 16.9)] {
            let (kmin, q) = tilted_pmf(d, a, 0.001, 0.0);
            assert_eq!(kmin, (-a / 0.001f64).ceil() as i64);
            assert!((np_sum(&q) - 1.0).abs() < 1e-12, "d {d}: {}", np_sum(&q));
        }
        assert_eq!(tilted_pmf(95, 0.0, 0.001, 7.0), (0, vec![1.0]));
    }

    /// Rust X equals certify.py X on the fixture's grid (four-, three- and two-block layouts at N = 2^10, 2^20, 2^30).
    #[test]
    fn certificates_match_certify_py() {
        let f = fixture();
        let grid = f["certs"].as_array().unwrap();
        assert!(grid.len() >= 27);
        let mut diff = Vec::new();
        for c in grid {
            let ds: Vec<usize> = c["ds"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize).collect();
            let lnd = c["lnd"].as_f64().unwrap();
            let got = certify_layout(&ds, lnd, 1e6, &mut |_| {});
            if got != c["X"].as_f64() {
                diff.push(format!("{ds:?} at {lnd}: {got:?} here, {} in certify.py", c["X"]));
            }
        }
        assert!(diff.is_empty(), "{diff:#?}");
    }

    #[test]
    fn certify_refuses_what_it_cannot_hold() {
        let far = Instant::now() + Duration::from_secs(60);
        let g = Grid::for_dims(&[15, 15]);
        assert!(certify(&[15], -30.0, 6.0, far, &g).is_err());
        assert!(certify(&[15, 15, 15, 15, 15], -30.0, 6.0, far, &g).is_err());
        assert!(certify(&[15, 15], -30.0, 6.0005, far, &g).is_err());
        assert!(certify(&[9000, 9000], -30.0, 6.0, far, &g).is_err());
        // a deadline already passed certifies nothing
        assert_eq!(certify(&[15, 15], -30.0, 6.0, Instant::now() - Duration::from_secs(1), &g), Ok(false));
        assert_eq!(certify_layout(&[15, 15], -30.0, 0.0, &mut |_| {}), None);
        // blocks too small for the density, budgets and X that are not numbers, and time caps that are not: refused
        // with an error or a log line, never a panic
        for ds in [&[1usize, 15][..], &[15, 3], &[2, 2, 2]] {
            assert!(certify(ds, -30.0, 6.0, far, &g).is_err(), "{ds:?}");
            let mut said = Vec::new();
            assert_eq!(certify_layout(ds, -30.0, 60.0, &mut |s| said.push(s)), None);
            assert!(said.iter().any(|s| s.contains("at least 4")), "{said:?}");
        }
        assert_eq!(certify_layout(&[], -30.0, 60.0, &mut |_| {}), None);
        for lnd in [f64::NAN, 0.0, 1.0, f64::NEG_INFINITY] {
            assert!(certify(&[15, 15], lnd, 6.0, far, &g).is_err(), "{lnd}");
            assert_eq!(certify_layout(&[15, 15], lnd, 60.0, &mut |_| {}), None);
        }
        assert!(certify(&[15, 15], -30.0, f64::NAN, far, &g).is_err());
        for cap in [f64::NAN, -1.0] {
            let mut said = Vec::new();
            assert_eq!(certify_layout(&[15, 15], -30.0, cap, &mut |s| said.push(s)), None);
            assert!(said.iter().any(|s| s.contains("time cap")), "{said:?}");
        }
        assert_eq!(certify_layout(&[15, 15], -28.0, f64::INFINITY, &mut |_| {}), Some(5.089));
    }

    /// The failing side: certify() at a fixed X just below a pass/fail cut-off in ln delta (fails) and just above it
    /// (certifies), for 2, 3 and 4 blocks, as certify.py answers there; the points are 5e-7 outside the interval that
    /// holds the cut-off, half the shift the REL margin makes, so a port without the margin, or one that certifies too
    /// easily, fails here. And two searches that fail at their first X and certify at a later step, as in certify.py.
    #[test]
    fn the_failing_side_matches_certify_py() {
        let f = fixture();
        let far = Instant::now() + Duration::from_secs(3600);
        let cuts = f["cutoffs"].as_array().unwrap();
        assert_eq!(cuts.len(), 6);
        let blocks = |c: &serde_json::Value| -> Vec<usize> { c["ds"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize).collect() };
        for c in cuts {
            let ds = blocks(c);
            let (lnd, x, ok) = (c["lnd"].as_f64().unwrap(), c["X"].as_f64().unwrap(), c["ok"].as_bool().unwrap());
            assert_eq!(certify(&ds, lnd, x, far, &Grid::for_dims(&ds)), Ok(ok), "{ds:?} at X {x}, ln delta {lnd}");
        }
        // each cut-off has its failing point and its certifying point
        for n in [2, 3, 4] {
            let oks: Vec<bool> = cuts.iter().filter(|c| blocks(c).len() == n).map(|c| c["ok"].as_bool().unwrap()).collect();
            assert_eq!(oks, [false, true], "{n} blocks");
        }
        for c in f["later"].as_array().unwrap() {
            let ds = blocks(c);
            let lnd = c["lnd"].as_f64().unwrap();
            let mut tries = 0;
            let got = certify_layout(&ds, lnd, 1e6, &mut |_| tries += 1);
            assert_eq!((got, tries), (c["X"].as_f64(), c["tries"].as_u64().unwrap() as usize), "{ds:?} at {lnd}");
            assert!(tries > 1);
        }
    }

    #[test]
    fn budget_and_table() {
        // N rounded up to a power of two; the 1M BioCLIP-2 fit's (four levels) and synth4k's (one level, 4,000 items)
        assert_eq!(ln_delta(1_000_000, 4, 1e-9), -35.972503809265206);
        assert_eq!(ln_delta(4000, 1, 1e-9), -29.041032003665755);
        assert_eq!(ln_delta(1 << 20, 4, 1e-9), ln_delta((1 << 19) + 1, 4, 1e-9));
        // the table holds an entry for the fit's layout of every dimension of tools/certs_table.py's list at every
        // N = 2^10 .. 2^34, at the budget as the fit computes it, and nothing else; the lookup is exact
        let t = table();
        let dims = [64, 96, 128, 192, 256, 384, 512, 768, 1024, 1152, 1536, 2048, 3072, 4096];
        let want: Vec<(Vec<usize>, f64)> = dims.iter().flat_map(|&n0| {
            let (ds, levels) = crate::fit::cert_layout(n0).unwrap();
            (10..=34).map(move |p| (ds.clone(), ln_delta(1 << p, levels, 1e-9)))
        }).collect();
        assert_eq!(t.len(), want.len());
        for (ds, lnd) in &want {
            assert!(lookup(ds, *lnd).is_some(), "no entry for {ds:?} at {lnd}");
        }
        for (ds, lnd, x) in t {
            assert_eq!(lookup(ds, *lnd), Some(*x));
            assert_eq!(lookup(ds, lnd + 1e-12), None);
            if let Some(x) = x {
                assert!(*x > 5.0 && *x < 10.0 && (x * 1000.0).round() / 1000.0 == *x, "{x}");
            }
        }
    }
}
