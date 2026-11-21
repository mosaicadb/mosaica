//! The W2 width's stored forms: what the approximate k-NN needs at build time to predict, per query, the spread of a
//! row's error at each deterministic level of a cluster (`KnnReader::with_width`).
//!
//! Notation (the theory session's): at level l of group g a row's error vector in the group's weighted coordinates is
//! u = (z - s sig val_l) / d_l (z the row in the group's basis, s its scale, val_l the level's decoded value), as the
//! encoder computes it (`encode_cluster`), so a query with D = (V_q^T q) d_l has the error D.u. S_c = mean u u' over cluster c's rows
//! (uncentred), G the group's diagonal of it over all its rows. p_j are the cluster's r order directions (int8 rows
//! times their scales, as the order state holds them) and b_j = (V_q^T p_j) d_l their images; B~ = B (P'P)^-1, so that
//! B~ a is the image of the query's part in the directions' span, a = P'q the coefficients the reader holds. Stored per
//! cluster and level: M = B~' S_c B~ and N = B~' G B~ (r x r, symmetric, packed), gamma = tr(S_c outside span B) /
//! tr(G outside span B) (the complement orthogonal in these coordinates), and U = mean over the cluster's rows of the
//! squared stored norm (`q::det_norm`, the code term of the row's half-width per unit of `CQ2::dwa`). At query time
//! tau^2 = a'Ma + gamma max(0, T - a'Na), T = D'GD, and the level reads at psi tau / (dwa sqrt U) of each row's
//! half-width (at most the certified width).

use super::q::*;
use super::*;

/// The files of a saved index that hold the forms: per group and level G (n values), and per cluster and level the
/// record `WForms::rec`.
pub const FILES: [&str; 2] = ["wrule_g.bin", "wrule_c.bin"];

/// The stored forms of an index whose clusters hold `r` order directions.
pub struct WForms {
    pub r: usize,
    /// per group and level, G (ng x ndl x n)
    pub g: Vec<f32>,
    /// per cluster and level, M and N packed (upper triangle, row by row), gamma and U (k x ndl x `rec(r)`)
    pub c: Vec<f32>,
}

impl WForms {
    /// The values of one (cluster, level) record: M and N packed, then gamma and U.
    pub fn rec(r: usize) -> usize {
        r * (r + 1) + 2
    }

    /// Cluster ci's record at level l.
    pub fn of(&self, ndl: usize, ci: usize, l: usize) -> &[f32] {
        let w = Self::rec(self.r);
        &self.c[(ci * ndl + l) * w..(ci * ndl + l + 1) * w]
    }

    /// Group g's G at level l.
    pub fn gdiag(&self, n: usize, ndl: usize, g: usize, l: usize) -> &[f32] {
        &self.g[(g * ndl + l) * n..(g * ndl + l + 1) * n]
    }

    /// Whether the forms fit an index of `ng` groups and `k` clusters at `ndl` levels of `n` dimensions, every value
    /// finite: what a build must make and an open must find.
    pub fn check(&self, ng: usize, k: usize, ndl: usize, n: usize) -> Result<(), String> {
        if self.g.len() != ng * ndl * n || self.c.len() != k * ndl * Self::rec(self.r) {
            return Err(format!("{} and {} do not hold the W2 forms of {ng} groups and {k} clusters at {ndl} levels", FILES[0], FILES[1]));
        }
        match self.g.iter().chain(&self.c).position(|x| !x.is_finite()) {
            Some(i) => Err(format!("the W2 forms hold a value that is not a number (at {i})")),
            None => Ok(()),
        }
    }
}

/// The factor W2 puts on a row's upper half-width at a deterministic level: min(1, psi tau / (dwa sqrt U)), tau^2 the
/// predicted spread (`CQ2::set_w2`), dwa the query's factor on the rows' stored norms and U their mean square. The
/// certified width (1) wherever that is not a number below 1: tau^2 not at least 0 (a NaN), dwa or U at 0, or psi
/// infinite.
pub fn factor(psi: f64, tau2: f64, dwa: f64, u: f64) -> f32 {
    let f = psi * tau2.sqrt() / (dwa * u.sqrt());
    if tau2 >= 0.0 && f < 1.0 { f as f32 } else { 1.0 }
}

/// a'Xa for X symmetric r x r, packed as its upper triangle row by row.
#[inline]
pub fn packed_quad(x: &[f32], a: &[f32]) -> f64 {
    let r = a.len();
    let (mut s, mut k) = (0f64, 0);
    for i in 0..r {
        let ai = a[i] as f64;
        let mut row = x[k] as f64 * ai;
        for j in i + 1..r {
            row += 2.0 * x[k + j - i] as f64 * a[j] as f64;
        }
        s += ai * row;
        k += r - i;
    }
    s
}

fn pack(m: &[f64], r: usize) -> impl Iterator<Item = f32> + '_ {
    (0..r).flat_map(move |i| (i..r).map(move |j| m[i * r + j] as f32))
}

/// One level's B~ (r vectors of n) and orthonormal basis of span B (`dual_dirs`).
type Dual = (Vec<Vec<f64>>, Vec<Vec<f64>>);

/// Per level of a cluster with order directions `dirs8` (r rows of n int8) and scales `dsc` (p_j = dirs8_j dsc_j, as
/// the order state holds them) in group `gp`: B~ = B (P'P)^-1 (r vectors of n), b_j = (V_q^T p_j) d_l, and an
/// orthonormal basis of span B. V_q^T P is one product per cluster (the int8 basis taken to f64 a block of its rows
/// at a time), scaled by d_l at each level; P'P is inverted once. Both P's rank and B's take the order state's rule
/// (`order::orthonormal64`): a direction left with under 1e-6 of the largest one's norm once the others are taken out
/// is in their span. Such a p_j (the order state leaves zero directions where a cluster is too small to span r) has a
/// zero column in B~, and P'P is inverted over the others; such a b_j adds no column to Q.
fn dual_dirs(gp: &GroupPlan2, dirs8: &[i8], dsc: &[f32], n: usize, ndl: usize) -> Vec<Dual> {
    const ROWS: usize = 256;
    let r = dsc.len();
    let p: Vec<f64> = (0..r * n).map(|i| dirs8[i] as f64 * dsc[i / n] as f64).collect();
    // vp[j n + t] = (V_q^T p_j)_t = vsc_t sum_k v8t[t n + k] p_j[k]
    let mut vp = vec![0f64; r * n];
    let mut blk = Vec::with_capacity(ROWS * n);
    for t0 in (0..n).step_by(ROWS) {
        let t1 = (t0 + ROWS).min(n);
        blk.clear();
        blk.extend(gp.v8t[t0 * n..t1 * n].iter().map(|&v| v as f64));
        unsafe {
            matrixmultiply::dgemm(r, n, t1 - t0, 1.0, p.as_ptr(), n as isize, 1, blk.as_ptr(), 1, n as isize, 0.0, vp.as_mut_ptr().add(t0), n as isize, 1);
        }
    }
    for j in 0..r {
        for t in 0..n {
            vp[j * n + t] *= gp.vsc[t];
        }
    }
    let dot = |x: &[f64], y: &[f64]| x.iter().zip(y).map(|(a, b)| a * b).sum::<f64>();
    let row = |j: usize| &p[j * n..(j + 1) * n];
    let live = basis(&(0..r).map(row).collect::<Vec<_>>(), n).0;
    let rl = live.len();
    let ptp: Vec<f64> = (0..rl * rl).map(|i| dot(row(live[i / rl]), row(live[i % rl]))).collect();
    let pinv = gauss_jordan_inverse(&ptp, rl);
    (0..ndl).map(|l| {
        let bs: Vec<Vec<f64>> = (0..r).map(|j| (0..n).map(|t| vp[j * n + t] * gp.d[l][t] as f64).collect()).collect();
        let mut bt = vec![vec![0f64; n]; r];
        for (jj, &j) in live.iter().enumerate() {
            bt[j] = (0..n).map(|t| (0..rl).map(|kk| bs[live[kk]][t] * pinv[kk * rl + jj]).sum()).collect();
        }
        (bt, basis(&bs.iter().map(|b| &b[..]).collect::<Vec<_>>(), n).1)
    }).collect()
}

/// The columns among `cols` (each n values) whose orthonormal basis column (`order::orthonormal64`) is not zero, and
/// those basis columns.
fn basis(cols: &[&[f64]], n: usize) -> (Vec<usize>, Vec<Vec<f64>>) {
    let m = cols.len();
    let y: Vec<f64> = (0..n).flat_map(|t| cols.iter().map(move |c| c[t])).collect();
    let q = super::order::orthonormal64(&y, n, m);
    let keep: Vec<usize> = (0..m).filter(|&j| (0..n).any(|t| q[t * m + j] != 0.0)).collect();
    let vs = keep.iter().map(|&j| (0..n).map(|t| q[t * m + j]).collect()).collect();
    (keep, vs)
}

/// `dual_dirs` of cluster c of `ix`, from its order state.
fn dual_of(ix: &Index2, c: usize) -> Vec<Dual> {
    let st = ix.order.as_ref().unwrap();
    dual_dirs(&ix.groups[ix.cl[c].g], st.dirs(c, st.r, ix.n()), &st.dsc[c * st.r..(c + 1) * st.r], ix.n(), ix.meta.ndl())
}

/// What W2 needs of one cluster's rows' coding errors, summed as the build encodes them (`encode_cluster`, which hands
/// each row's error vector u at each deterministic level to `add`): per level, over the rows, the sums of (B~'u)(B~'u)'
/// (r x r), of |Q'u|^2 (Q an orthonormal basis of span B) and of u_t^2 per coordinate. The rows wait in a block of
/// `Sums::ROWS` per level, taken in by one product each.
pub struct Sums {
    /// per level: B~ as n x r and Q as n x rq (row-major, for the products), and the rows waiting
    bt: Vec<Vec<f64>>,
    qb: Vec<Vec<f64>>,
    rq: Vec<usize>,
    wait: Vec<Vec<f64>>,
    n: usize,
    r: usize,
    yy: Vec<Vec<f64>>,
    qq: Vec<f64>,
    usq: Vec<Vec<f64>>,
    rows: usize,
    /// seconds spent in `new` and taking rows in
    secs: f64,
}

impl Sums {
    const ROWS: usize = 256;

    /// The sums of a cluster with order directions `dirs8` and scales `dsc` in group `gp`, none taken yet.
    pub fn new(gp: &GroupPlan2, dirs8: &[i8], dsc: &[f32], n: usize, ndl: usize) -> Sums {
        let t = Timer::new();
        let r = dsc.len();
        let (mut bt, mut qb, mut rq) = (Vec::new(), Vec::new(), Vec::new());
        for (b, q) in dual_dirs(gp, dirs8, dsc, n, ndl) {
            bt.push((0..n).flat_map(|t| b.iter().map(move |v| v[t])).collect());
            rq.push(q.len());
            qb.push((0..n).flat_map(|t| q.iter().map(move |v| v[t])).collect());
        }
        Sums { bt, qb, rq, wait: vec![Vec::with_capacity(Self::ROWS * n); ndl], n, r, yy: vec![vec![0.0; r * r]; ndl], qq: vec![0.0; ndl], usq: vec![vec![0.0; n]; ndl], rows: 0, secs: t.s() }
    }

    /// Row error vector `u` (n values) at level l; level 0 counts the row.
    pub fn add(&mut self, l: usize, u: &[f64]) {
        self.rows += (l == 0) as usize;
        self.wait[l].extend_from_slice(u);
        if self.wait[l].len() == Self::ROWS * self.n {
            self.take(l);
        }
    }

    /// The rows waiting at level l, taken into the sums.
    fn take(&mut self, l: usize) {
        let t = Timer::new();
        let (n, r, rq) = (self.n, self.r, self.rq[l]);
        let w = std::mem::take(&mut self.wait[l]);
        let m = w.len() / n;
        let prod = |b: &[f64], k: usize| -> Vec<f64> {
            let mut y = vec![0f64; m * k];
            unsafe {
                matrixmultiply::dgemm(m, n, k, 1.0, w.as_ptr(), n as isize, 1, b.as_ptr(), k as isize, 1, 0.0, y.as_mut_ptr(), k as isize, 1);
            }
            y
        };
        let (y, yq) = (prod(&self.bt[l], r), prod(&self.qb[l], rq));
        for i in 0..m {
            let yi = &y[i * r..(i + 1) * r];
            for a in 0..r {
                for b in 0..r {
                    self.yy[l][a * r + b] += yi[a] * yi[b];
                }
            }
            for t in 0..n {
                self.usq[l][t] += w[i * n + t] * w[i * n + t];
            }
        }
        self.qq[l] += yq.iter().map(|x| x * x).sum::<f64>();
        self.wait[l] = w;
        self.wait[l].clear();
        self.secs += t.s();
    }

    /// The sums once every row is in (what the forms keep of the cluster: the bases are dropped).
    pub fn done(mut self) -> Summed {
        for l in 0..self.wait.len() {
            self.take(l);
        }
        Summed { yy: self.yy, qq: self.qq, usq: self.usq, rows: self.rows, secs: self.secs }
    }
}

/// A cluster's `Sums`, every row in, and the seconds they took.
pub struct Summed {
    yy: Vec<Vec<f64>>,
    qq: Vec<f64>,
    usq: Vec<Vec<f64>>,
    rows: usize,
    pub secs: f64,
}

/// The forms of `ix` (which holds stored vectors and the order state) from each cluster's sums (`Sums`, taken as it was
/// encoded): M and the traces of S_c from them, G over each group's rows, then N and gamma from G (B~ and Q formed
/// again from the order state, one product per cluster, rather than all clusters' kept), and U from the stored norms.
pub fn finish(ix: &Index2, sums: &[Summed]) -> WForms {
    use rayon::prelude::*;
    let (n, ndl, k, ng) = (ix.n(), ix.meta.ndl(), ix.k(), ix.groups.len());
    let r = ix.order.as_ref().expect("the W2 forms read the order state").r;
    // G per group and level: mean u_t^2 over the group's rows
    let mut g = vec![0f64; ng * ndl * n];
    let mut rows = vec![0f64; ng];
    for (c, one) in sums.iter().enumerate() {
        let gi = ix.cl[c].g;
        rows[gi] += one.rows as f64;
        for l in 0..ndl {
            for t in 0..n {
                g[(gi * ndl + l) * n + t] += one.usq[l][t];
            }
        }
    }
    for (i, x) in g.iter_mut().enumerate() {
        *x /= rows[i / (ndl * n)].max(1.0);
    }
    let recs: Vec<Vec<f32>> = (0..k).into_par_iter().map(|c| {
        let (cl, one) = (&ix.cl[c], &sums[c]);
        let nc = one.rows;
        let dual = if nc > 0 { dual_of(ix, c) } else { Vec::new() };
        let mut out = Vec::with_capacity(ndl * WForms::rec(r));
        for l in 0..ndl {
            let gd = &g[(cl.g * ndl + l) * n..(cl.g * ndl + l + 1) * n];
            let gq = |x: &[f64], y: &[f64]| -> f64 { (0..n).map(|t| x[t] * gd[t] * y[t]).sum() };
            let zero = (vec![vec![0.0; n]; r], Vec::new());
            let (bt, qb) = if nc > 0 { &dual[l] } else { &zero };
            let m: Vec<f64> = one.yy[l].iter().map(|x| x / nc.max(1) as f64).collect();
            let nn: Vec<f64> = (0..r * r).map(|i| gq(&bt[i / r], &bt[i % r])).collect();
            let (trs, trq) = (one.usq[l].iter().sum::<f64>() / nc.max(1) as f64, one.qq[l] / nc.max(1) as f64);
            let den = gd.iter().sum::<f64>() - qb.iter().map(|w| gq(w, w)).sum::<f64>();
            let gamma = if den > 0.0 { ((trs - trq) / den).max(0.0) } else { 0.0 };
            let uu = if nc > 0 { (cl.r0..cl.r1).map(|row| det_norm(ix, c, l, row).powi(2)).sum::<f64>() / nc as f64 } else { 0.0 };
            out.extend(pack(&m, r));
            out.extend(pack(&nn, r));
            out.extend([gamma as f32, uu as f32]);
        }
        out
    }).collect();
    WForms { r, g: g.iter().map(|&x| x as f32).collect(), c: recs.concat() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The factor is psi tau / (dwa sqrt U) below 1, and 1 (the certified width) at 1 and above, at an infinite psi,
    /// at dwa or U 0, and at a tau^2 that is not a number; 0 at tau 0 and a finite psi.
    #[test]
    fn the_w2_factor_is_the_certified_width_where_it_cannot_be_less() {
        assert_eq!(factor(2.0, 0.25, 4.0, 1.0), 0.25);
        assert_eq!(factor(2.0, 4.0, 1.0, 1.0), 1.0);
        assert_eq!(factor(2.0, 0.0, 1.0, 1.0), 0.0);
        for (psi, tau2, dwa, u) in [(f64::INFINITY, 0.25, 4.0, 1.0), (f64::INFINITY, 0.0, 1.0, 1.0), (2.0, 0.25, 4.0, 0.0), (2.0, 0.25, 0.0, 1.0), (2.0, 0.0, 0.0, 0.0), (2.0, f64::NAN, 4.0, 1.0), (2.0, -1e-30, 4.0, 1.0)] {
            assert_eq!(factor(psi, tau2, dwa, u), 1.0, "psi {psi}, tau^2 {tau2}, dwa {dwa}, U {u}");
        }
    }
}
