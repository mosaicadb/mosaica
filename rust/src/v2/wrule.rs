//! The W2 width's stored forms: what the approximate k-NN needs at build time to predict, per query, the spread of a
//! row's error at each deterministic level of a cluster (`KnnReader::with_width`).
//!
//! Notation (the theory session's): at level l of group g a row's error vector in the group's weighted coordinates is
//! u = (z - s sig val_l) / d_l (z the row in the group's basis, s its scale, val_l the level's decoded value), as the
//! encoder computes it (`encode_cluster`). The reader's estimate takes off the cluster's centre term (alr times the
//! centre m, `CQ2::alr`), so its error on a query is D.u with D = (V_q^T q - alr mv) d_l, mv = V_q^T m (`Cl2::mv32`).
//! S_c = mean u u' over cluster c's rows (uncentred), G the group's diagonal of it over all its rows. p_j are the
//! cluster's r order directions (int8 rows times their scales, as the order state holds them) and b_j = (V_q^T p_j) d_l
//! their images; B~ = B (P'P)^-1, so that B~ a is the image of a vector's part in the directions' span, a = P'x its
//! coefficients. Stored per cluster and level: M = B~' S_c B~ and N = B~' G B~ (r x r, symmetric, packed, the
//! off-diagonals doubled), gamma = tr(S_c outside span B) / tr(G outside span B) (the complement orthogonal in these
//! coordinates), and U = mean over the cluster's rows of the squared stored norm (`q::det_norm`, the code term of the
//! row's half-width per unit of `CQ2::dwa`); per cluster, P'm; per group and level, h = d_l^2 G. At query time
//! a = P'q - alr P'm (the coefficients the reader holds, less the centre's), T = D'GD, tau^2 = a'Ma + gamma max(0,
//! T - a'Na) (`WForms::tau2`), and the level reads at psi tau / (dwa sqrt U) of each row's half-width (at most the
//! certified width).

use super::q::*;
use super::*;

/// The files of a saved index that hold the forms: per group and level h = d_l^2 G (n values), and per cluster its
/// record (`WForms::stride` values: each level's `WForms::rec`, then P'm).
pub const FILES: [&str; 2] = ["wrule_h.bin", "wrule_c.bin"];

/// The stored forms of an index whose clusters hold `r` order directions.
pub struct WForms {
    pub r: usize,
    /// per group and level, h = d_l^2 G (ng x ndl x n): T = D'GD' is sum_j (qv_j - alr mv_j)^2 h_j
    pub h: Vec<f32>,
    /// per cluster, one contiguous record (`stride`): at each level M and N packed (upper triangle, row by row, the
    /// off-diagonals doubled), gamma and U (`rec(r)` values); then its centre's coefficients on its order directions,
    /// P'm (r values, `OrderState::coefs_of`)
    pub c: Vec<f32>,
}

impl WForms {
    /// The values of one (cluster, level) record: M and N packed, then gamma and U.
    pub fn rec(r: usize) -> usize {
        r * (r + 1) + 2
    }

    /// The values of one cluster's record: `rec(r)` per level, then P'm.
    pub fn stride(r: usize, ndl: usize) -> usize {
        ndl * Self::rec(r) + r
    }

    /// Cluster ci's record at level l.
    pub fn of(&self, ndl: usize, ci: usize, l: usize) -> &[f32] {
        let (w, s) = (Self::rec(self.r), Self::stride(self.r, ndl));
        &self.c[ci * s + l * w..ci * s + (l + 1) * w]
    }

    /// Cluster ci's P'm (r values).
    pub fn pm(&self, ndl: usize, ci: usize) -> &[f32] {
        let s = Self::stride(self.r, ndl);
        &self.c[(ci + 1) * s - self.r..(ci + 1) * s]
    }

    /// Cluster ci's predicted spread squared at level l for the coefficients `a` (r values, the centre's taken off) and
    /// T: a'Ma + gamma max(0, T - a'Na) (`tau2_outer` of a's `outer`).
    pub fn tau2(&self, ndl: usize, ci: usize, l: usize, a: &[f32], t: f64) -> f64 {
        self.tau2_outer(ndl, ci, l, &outer(a), t)
    }

    /// `tau2` from a's packed outer product `o` (`outer`), formed once for every level of a cluster.
    pub fn tau2_outer(&self, ndl: usize, ci: usize, l: usize, o: &[f32], t: f64) -> f64 {
        let (rec, rp) = (self.of(ndl, ci, l), self.r * (self.r + 1) / 2);
        quad(&rec[..rp], &o[..rp]) + rec[2 * rp] as f64 * (t - quad(&rec[rp..2 * rp], &o[..rp])).max(0.0)
    }

    /// Cluster ci's mean squared stored row norm at level l, U.
    pub fn u(&self, ndl: usize, ci: usize, l: usize) -> f64 {
        self.of(ndl, ci, l)[self.r * (self.r + 1) + 1] as f64
    }

    /// Group g's h = d_l^2 G at level l.
    pub fn h(&self, n: usize, ndl: usize, g: usize, l: usize) -> &[f32] {
        &self.h[(g * ndl + l) * n..(g * ndl + l + 1) * n]
    }

    /// Whether the forms fit an index of `ng` groups and `k` clusters at `ndl` levels of `n` dimensions, every value
    /// finite and h, gamma and U at least 0 (each a mean of squares or a ratio of them; a negative one would narrow
    /// the width): what a build must make and an open must find.
    pub fn check(&self, ng: usize, k: usize, ndl: usize, n: usize) -> Result<(), String> {
        let (r, rp) = (self.r, self.r * (self.r + 1) / 2);
        if self.h.len() != ng * ndl * n || self.c.len() != k * Self::stride(r, ndl) {
            return Err(format!("{} and {} do not hold the W2 forms of {ng} groups and {k} clusters at {ndl} levels", FILES[0], FILES[1]));
        }
        if let Some(i) = self.h.iter().chain(&self.c).position(|x| !x.is_finite()) {
            return Err(format!("the W2 forms hold a value that is not a number (at {i})"));
        }
        if let Some(i) = self.h.iter().position(|&x| x < 0.0) {
            return Err(format!("the W2 forms hold a negative h (at {i})"));
        }
        for ci in 0..k {
            for l in 0..ndl {
                let rec = self.of(ndl, ci, l);
                if rec[2 * rp] < 0.0 || rec[2 * rp + 1] < 0.0 {
                    return Err(format!("the W2 forms hold a negative gamma or U (cluster {ci}, level {l})"));
                }
            }
        }
        Ok(())
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

/// The most values a packed r x r symmetric matrix holds (r at most `order::R_MAX`).
pub const RP_MAX: usize = super::order::R_MAX * (super::order::R_MAX + 1) / 2;

/// The packed outer product of `a` (r values): a_i a_j for i <= j, row by row (the first r(r + 1) / 2 values).
pub fn outer(a: &[f32]) -> [f32; RP_MAX] {
    let (mut o, mut k) = ([0f32; RP_MAX], 0);
    for i in 0..a.len() {
        for &aj in &a[i..] {
            o[k] = a[i] * aj;
            k += 1;
        }
    }
    o
}

/// a'Xa for X symmetric, stored packed with its off-diagonals doubled (`pack`), from a's packed outer product `o`.
#[inline]
pub fn quad(x: &[f32], o: &[f32]) -> f64 {
    unsafe { dot32(o, x) }
}

/// X symmetric r x r packed as its upper triangle row by row, the off-diagonals doubled (each stands for X_ij and
/// X_ji).
fn pack(m: &[f64], r: usize) -> impl Iterator<Item = f32> + '_ {
    (0..r).flat_map(move |i| (i..r).map(move |j| (if i == j { m[i * r + j] } else { 2.0 * m[i * r + j] }) as f32))
}

/// One level's B~ (r vectors of n) and orthonormal basis of span B (`dual_dirs`).
type Dual = (Vec<Vec<f64>>, Vec<Vec<f64>>);

/// Per level of a cluster with order directions `dirs8` (r rows of n int8) and scales `dsc` (p_j = dirs8_j dsc_j, as
/// the order state holds them) in group `gp`: B~ = B (P'P)^-1 (r vectors of n), b_j = (V_q^T p_j) d_l, and an
/// orthonormal basis of span B. V_q^T P is formed once per cluster, exactly: each entry an int8 dot product of the
/// basis's row with the direction's, summed in i32, times the two scales (so `Sums::new` and `finish`, which both form
/// it, get the same bits); it is scaled by d_l at each level, and P'P is inverted once. Both P's rank and B's take the
/// order state's rule (`order::orthonormal64`): a direction left with under 1e-6 of the largest one's norm once the
/// others are taken out is in their span. Such a p_j (the order state leaves zero directions where a cluster is too
/// small to span r) has a zero column in B~, and P'P is inverted over the others; such a b_j adds no column to Q.
fn dual_dirs(gp: &GroupPlan2, dirs8: &[i8], dsc: &[f32], n: usize, ndl: usize) -> Vec<Dual> {
    let r = dsc.len();
    let p: Vec<f64> = (0..r * n).map(|i| dirs8[i] as f64 * dsc[i / n] as f64).collect();
    // vp[j n + t] = (V_q^T p_j)_t = vsc_t dsc_j sum_k v8t[t n + k] dirs8[j n + k], the sum exact in i32 (|sum| <= n 127^2)
    let mut vp = vec![0f64; r * n];
    for t in 0..n {
        let row = &gp.v8t[t * n..(t + 1) * n];
        for j in 0..r {
            let s: i32 = row.iter().zip(&dirs8[j * n..(j + 1) * n]).map(|(&a, &b)| a as i32 * b as i32).sum();
            vp[j * n + t] = s as f64 * gp.vsc[t] * dsc[j] as f64;
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
/// each row's error vector u at each deterministic level to `add`, and adds u_t^2 per coordinate into `usq_mut` as it
/// forms u): per level, over the rows, the sums of (B~'u)(B~'u)' (r x r), of |Q'u|^2 (Q an orthonormal basis of span
/// B) and of u_t^2 per coordinate. The rows wait in a block of `Sums::ROWS` per level (small enough to stay in L2),
/// taken in by one product against [B~ | Q].
pub struct Sums {
    /// per level: [B~ | Q] as n x (r + rq) (row-major, for the product), its width, and the rows waiting
    bq: Vec<Vec<f64>>,
    wid: Vec<usize>,
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
    const ROWS: usize = 64;

    /// The sums of a cluster with order directions `dirs8` and scales `dsc` in group `gp`, none taken yet.
    pub fn new(gp: &GroupPlan2, dirs8: &[i8], dsc: &[f32], n: usize, ndl: usize) -> Sums {
        let t = Timer::new();
        let r = dsc.len();
        let (mut bq, mut wid) = (Vec::new(), Vec::new());
        for (b, q) in dual_dirs(gp, dirs8, dsc, n, ndl) {
            wid.push(r + q.len());
            bq.push((0..n).flat_map(|t| b.iter().chain(&q).map(move |v| v[t])).collect());
        }
        Sums { bq, wid, wait: vec![Vec::with_capacity(Self::ROWS * n); ndl], n, r, yy: vec![vec![0.0; r * r]; ndl], qq: vec![0.0; ndl], usq: vec![vec![0.0; n]; ndl], rows: 0, secs: t.s() }
    }

    /// The sums of u_t^2 at level l, for the encoder to add each row's into as it forms u.
    pub fn usq_mut(&mut self, l: usize) -> &mut [f64] {
        &mut self.usq[l]
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
        let (n, r, k) = (self.n, self.r, self.wid[l]);
        let w = std::mem::take(&mut self.wait[l]);
        let m = w.len() / n;
        let mut y = vec![0f64; m * k];
        unsafe {
            matrixmultiply::dgemm(m, n, k, 1.0, w.as_ptr(), n as isize, 1, self.bq[l].as_ptr(), k as isize, 1, 0.0, y.as_mut_ptr(), k as isize, 1);
        }
        for yi in y.chunks_exact(k) {
            for a in 0..r {
                for b in 0..r {
                    self.yy[l][a * r + b] += yi[a] * yi[b];
                }
            }
            self.qq[l] += yi[r..].iter().map(|x| x * x).sum::<f64>();
        }
        self.wait[l] = w;
        self.wait[l].clear();
        self.secs += t.s();
    }

    /// What the forms keep of the cluster once every row is in: M packed, the traces of S_c and of S_c within span B,
    /// and the rows (`Summed`); and the sums of u_t^2 per level, for its group's G (`GroupSums::add`).
    pub fn done(mut self) -> (Summed, Vec<Vec<f64>>) {
        for l in 0..self.wait.len() {
            self.take(l);
        }
        let (r, nc) = (self.r, self.rows as f64);
        let mut s = Summed { m: Vec::new(), trs: Vec::new(), trq: Vec::new(), rows: self.rows, secs: self.secs };
        if self.rows > 0 {
            for l in 0..self.yy.len() {
                let m: Vec<f64> = self.yy[l].iter().map(|x| x / nc).collect();
                s.m.extend(pack(&m, r));
                s.trs.push(self.usq[l].iter().sum::<f64>() / nc);
                s.trq.push(self.qq[l] / nc);
            }
        }
        (s, self.usq)
    }
}

/// What the forms keep of a cluster's `Sums` (none of its rows' bases or u_t^2 sums): per level M packed (`pack`), tr S_c
/// and tr S_c within span B; its rows; and the seconds the sums took.
pub struct Summed {
    m: Vec<f32>,
    trs: Vec<f64>,
    trq: Vec<f64>,
    rows: usize,
    pub secs: f64,
}

/// Each group's sums of u_t^2 per level and its rows, for G: the build adds each cluster's in cluster order
/// (`add`), so that they do not depend on the threads, and drops them.
pub struct GroupSums {
    g: Vec<f64>,
    rows: Vec<f64>,
    ndl: usize,
    n: usize,
}

impl GroupSums {
    /// No rows yet, for `ng` groups at `ndl` levels of `n` dimensions.
    pub fn new(ng: usize, ndl: usize, n: usize) -> GroupSums {
        GroupSums { g: vec![0.0; ng * ndl * n], rows: vec![0.0; ng], ndl, n }
    }

    /// A cluster of group g: its sums of u_t^2 per level (`Sums::done`) and its rows (`summed`).
    pub fn add(&mut self, g: usize, summed: &Summed, usq: &[Vec<f64>]) {
        self.rows[g] += summed.rows as f64;
        for (l, u) in usq.iter().enumerate() {
            for (x, &y) in self.g[(g * self.ndl + l) * self.n..(g * self.ndl + l + 1) * self.n].iter_mut().zip(u) {
                *x += y;
            }
        }
    }
}

/// The forms of `ix` (which holds stored vectors and the order state) from each cluster's sums (`Summed`, taken as it
/// was encoded) and its group's (`GroupSums`): G over each group's rows, then N and gamma from G (B~ and Q formed again
/// from the order state, one product per cluster, rather than all clusters' kept), U from the stored norms, and P'm.
pub fn finish(ix: &Index2, sums: &[Summed], gs: GroupSums) -> WForms {
    use rayon::prelude::*;
    let (n, ndl, k, ng) = (ix.n(), ix.meta.ndl(), ix.k(), ix.groups.len());
    let r = ix.order.as_ref().expect("the W2 forms read the order state").r;
    // G per group and level: mean u_t^2 over the group's rows
    let mut g = gs.g;
    for (i, x) in g.iter_mut().enumerate() {
        *x /= gs.rows[i / (ndl * n)].max(1.0);
    }
    let recs: Vec<Vec<f32>> = (0..k).into_par_iter().map(|c| {
        let (cl, one) = (&ix.cl[c], &sums[c]);
        let nc = one.rows;
        let mut out = Vec::with_capacity(WForms::stride(r, ndl));
        // a cluster with no rows: no errors, so M, N, gamma and U are 0
        let dual = if nc > 0 { dual_of(ix, c) } else { Vec::new() };
        if nc == 0 {
            out.resize(ndl * WForms::rec(r), 0.0);
        }
        for (l, (bt, qb)) in dual.iter().enumerate() {
            let gd = &g[(cl.g * ndl + l) * n..(cl.g * ndl + l + 1) * n];
            let gq = |x: &[f64], y: &[f64]| -> f64 { (0..n).map(|t| x[t] * gd[t] * y[t]).sum() };
            // the upper triangle alone (`pack` reads no other)
            let nn: Vec<f64> = (0..r * r).map(|i| if i % r >= i / r { gq(&bt[i / r], &bt[i % r]) } else { 0.0 }).collect();
            let (trs, trq) = (one.trs[l], one.trq[l]);
            let den = gd.iter().sum::<f64>() - qb.iter().map(|w| gq(w, w)).sum::<f64>();
            let gamma = if den > 0.0 { ((trs - trq) / den).max(0.0) } else { 0.0 };
            let uu = (cl.r0..cl.r1).map(|row| det_norm(ix, c, l, row).powi(2)).sum::<f64>() / nc as f64;
            let rp = r * (r + 1) / 2;
            out.extend_from_slice(&one.m[l * rp..(l + 1) * rp]);
            out.extend(pack(&nn, r));
            out.extend([gamma as f32, uu as f32]);
        }
        out.extend_from_slice(&ix.order.as_ref().unwrap().coefs_of(c, n, &ix.centres32[c * n..(c + 1) * n])[..r]);
        out
    }).collect();
    let h = (0..ng * ndl * n).map(|i| {
        let (gl, t) = (i / n, i % n);
        ((ix.groups[gl / ndl].d[gl % ndl][t] as f64).powi(2) * g[i]) as f32
    }).collect();
    WForms { r, h, c: recs.concat() }
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
