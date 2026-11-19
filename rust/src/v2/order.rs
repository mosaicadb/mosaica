//! The approximate queries' cluster state (the spreads of `adaptive::Adaptive`, the radii of `q::Approx`), computed
//! by the build from each cluster's rows and saved with the index: every cluster's RMS radius about its centre, and
//! its top `R_MAX` principal directions about the centre (int8 rows with a scale each) with their variances.
//!
//! The directions come from a fixed sample of the cluster's rows (`SAMPLE`, spread evenly over them) by a randomised
//! subspace iteration on the sample's covariance about the centre, finished by a Rayleigh-Ritz step; the radius is over
//! every row. Every operation runs in a fixed order with separate multiplies and adds, so the state is the same bits
//! whatever the instruction set, the thread count or whether it is computed at build or from a saved index's rows.

use super::*;
use rand::{Rng, SeedableRng};

/// Directions stored per cluster (fewer when the vectors have fewer dimensions).
pub const R_MAX: usize = 16;
/// The rows a cluster's directions are estimated from: all of them up to this many, else this many spread evenly over
/// the cluster's rows in index order.
pub const SAMPLE: usize = 4096;
/// Columns of the subspace iteration: `R_MAX` and as many again, so the `R_MAX`-th direction's error shrinks by about
/// lambda_33 / lambda_16 per pass.
const BLOCK: usize = 32;
/// Passes of the subspace iteration after the first product with the sample's covariance.
pub const PASSES: usize = 4;
/// The saved index's files of `OrderState::rad`, `lam`, `dsc` and `dirs8`.
pub const FILES: [&str; 4] = ["order_rad.bin", "order_lam.bin", "order_dsc.bin", "order_dirs.bin"];

/// The order state of every cluster, cluster by cluster: on the heap after a build in memory, mapped from the saved
/// index's `order_*.bin` once saved or opened.
pub struct OrderState {
    /// Directions stored per cluster.
    pub r: usize,
    /// The dimensions the data spans: those in which some centre is not zero (the fit pads the vectors with zero
    /// columns).
    pub n0: usize,
    /// Each cluster's RMS radius about its stored centre, over all its rows.
    pub rad: Arr<f64>,
    /// k x r: the variance of the cluster's sample along each direction, largest first.
    pub lam: Arr<f32>,
    /// k x r: each direction's scale (its int8 row times this is the unit direction).
    pub dsc: Arr<f32>,
    /// k x r x n: the directions as int8 rows.
    pub dirs8: Arr<i8>,
}

/// One cluster's part of `OrderState`.
pub struct ClusterOrder {
    pub rad: f64,
    pub lam: Vec<f32>,
    pub dsc: Vec<f32>,
    pub dirs8: Vec<i8>,
}

/// The dimensions the centres span (at least 1).
pub fn spanned(centres: &[f32], n: usize) -> usize {
    (0..n).filter(|&i| centres.chunks_exact(n).any(|c| c[i] != 0.0)).count().max(1)
}

/// The directions stored per cluster for vectors of n dimensions.
pub fn r_of(n: usize) -> usize {
    R_MAX.min(n)
}

/// A cluster's state from its `rows` rows (`row(i, out)` gives row i's vector) about `centre`: its RMS radius over
/// every row, and its top r principal directions and their variances over the `sample` rows `spread` over it, after
/// `passes` passes of the subspace iteration started from random signs drawn from `seed`.
pub fn cluster(rows: usize, row: &dyn Fn(usize, &mut [f32]), centre: &[f32], r: usize, sample: usize, passes: usize, seed: u64) -> ClusterOrder {
    let n = centre.len();
    assert!(r <= BLOCK.min(n), "{r} directions of {n} dimensions do not fit a block of {BLOCK}");
    let mut x = vec![0f32; n];
    // the radius in f64 over every row, row by row as `Approx::radii` sums it
    let mut ss = 0f64;
    for i in 0..rows {
        row(i, &mut x);
        ss += x.iter().zip(centre).map(|(&a, &b)| (a as f64 - b as f64) * (a as f64 - b as f64)).sum::<f64>();
    }
    let rad = (ss / rows.max(1) as f64).sqrt();
    let keep = spread(rows, sample);
    let s = keep.len();
    let mut xs = vec![0f32; s * n];
    for (&i, xi) in keep.iter().zip(xs.chunks_exact_mut(n.max(1))) {
        row(i, &mut x);
        for ((o, &a), &b) in xi.iter_mut().zip(&x).zip(centre) {
            *o = a - b;
        }
    }
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
    let mut q: Vec<f32> = (0..n * BLOCK).map(|_| if rng.gen::<bool>() { 1.0 } else { -1.0 }).collect();
    let (mut z, mut y) = (vec![0f32; s * BLOCK], vec![0f32; n * BLOCK]);
    for _ in 0..=passes {
        times_q(&xs, n, &q, &mut z);
        transpose_times_z(&xs, n, &z, &mut y);
        q = orthonormal(&y, n);
    }
    // Rayleigh-Ritz: the sample's covariance restricted to the subspace, diagonalised
    times_q(&xs, n, &q, &mut z);
    let mut b = vec![0f64; BLOCK * BLOCK];
    for zi in z.as_chunks::<BLOCK>().0 {
        for a in 0..BLOCK {
            let za = zi[a] as f64;
            for (c, o) in b[a * BLOCK..(a + 1) * BLOCK].iter_mut().enumerate() {
                *o += za * zi[c] as f64;
            }
        }
    }
    b.iter_mut().for_each(|v| *v /= s.max(1) as f64);
    let (vals, vecs) = jacobi_desc(&mut b, BLOCK);
    let (mut lam, mut dsc, mut dirs8) = (Vec::with_capacity(r), Vec::with_capacity(r), Vec::with_capacity(r * n));
    for k in 0..r {
        let u: Vec<f64> = (0..n).map(|j| (0..BLOCK).map(|c| q[j * BLOCK + c] as f64 * vecs[c * BLOCK + k]).sum()).collect();
        let sc = u.iter().fold(0f64, |a, &v| a.max(v.abs())) / 127.0;
        dirs8.extend(u.iter().map(|&v| if sc > 0.0 { (v / sc).round() as i8 } else { 0 }));
        dsc.push(sc as f32);
        lam.push(vals[k].max(0.0) as f32);
    }
    ClusterOrder { rad, lam, dsc, dirs8 }
}

/// z = x q for the s x n rows x and the n x BLOCK matrix q (row-major), each output a sum over the dimensions in order.
fn times_q(x: &[f32], n: usize, q: &[f32], z: &mut [f32]) {
    let n = n.max(1);
    let q = q.as_chunks::<BLOCK>().0;
    let z = z.as_chunks_mut::<BLOCK>().0;
    // four rows at a time, each row's sums kept apart
    let (x4, x1) = x.split_at(x.len() / (4 * n) * 4 * n);
    let (z4, z1) = z.split_at_mut(x4.len() / n);
    for (xs, zs) in x4.chunks_exact(4 * n).zip(z4.chunks_exact_mut(4)) {
        let mut acc = [[0f32; BLOCK]; 4];
        for (j, qj) in q.iter().enumerate().take(n) {
            for (k, a) in acc.iter_mut().enumerate() {
                let xv = xs[k * n + j];
                for c in 0..BLOCK {
                    a[c] += xv * qj[c];
                }
            }
        }
        zs.copy_from_slice(&acc);
    }
    for (xi, zi) in x1.chunks_exact(n).zip(z1) {
        let mut acc = [0f32; BLOCK];
        for (&a, qj) in xi.iter().zip(q) {
            for c in 0..BLOCK {
                acc[c] += a * qj[c];
            }
        }
        *zi = acc;
    }
}

/// y = x' z for the s x n rows x and the s x BLOCK matrix z, each output a sum over the rows in order.
fn transpose_times_z(x: &[f32], n: usize, z: &[f32], y: &mut [f32]) {
    let n = n.max(1);
    y.fill(0.0);
    let y = y.as_chunks_mut::<BLOCK>().0;
    let z = z.as_chunks::<BLOCK>().0;
    // four rows at a time, added in row order
    let (x4, x1) = x.split_at(x.len() / (4 * n) * 4 * n);
    let (z4, z1) = z.split_at(x4.len() / n);
    for (xs, zs) in x4.chunks_exact(4 * n).zip(z4.chunks_exact(4)) {
        for (j, yj) in y.iter_mut().enumerate() {
            let a = [xs[j], xs[n + j], xs[2 * n + j], xs[3 * n + j]];
            for c in 0..BLOCK {
                let mut t = yj[c];
                t += a[0] * zs[0][c];
                t += a[1] * zs[1][c];
                t += a[2] * zs[2][c];
                t += a[3] * zs[3][c];
                yj[c] = t;
            }
        }
    }
    for (xi, zi) in x1.chunks_exact(n).zip(z1) {
        for (&a, yj) in xi.iter().zip(y.iter_mut()) {
            for c in 0..BLOCK {
                yj[c] += a * zi[c];
            }
        }
    }
}

/// An orthonormal basis of the columns of the n x BLOCK matrix y (row-major), by modified Gram-Schmidt run twice, in
/// f64. A column left with under 1e-6 of the largest column's norm is in the span of those before it (the sample has
/// fewer rows or dimensions than the block) and becomes zero.
fn orthonormal(y: &[f32], n: usize) -> Vec<f32> {
    let y: Vec<f64> = y.iter().map(|&v| v as f64).collect();
    orthonormal64(&y, n).iter().map(|&v| v as f32).collect()
}

/// `orthonormal` of an f64 matrix, in f64.
fn orthonormal64(y: &[f64], n: usize) -> Vec<f64> {
    let mut cols: Vec<Vec<f64>> = (0..BLOCK).map(|c| (0..n).map(|j| y[j * BLOCK + c]).collect()).collect();
    let norm = |v: &[f64]| v.iter().map(|x| x * x).sum::<f64>().sqrt();
    let top = cols.iter().map(|c| norm(c)).fold(0f64, f64::max);
    for k in 0..BLOCK {
        let (done, rest) = cols.split_at_mut(k);
        let v = &mut rest[0];
        for _ in 0..2 {
            for u in done.iter() {
                let d: f64 = u.iter().zip(v.iter()).map(|(a, b)| a * b).sum();
                v.iter_mut().zip(u).for_each(|(x, &a)| *x -= d * a);
            }
        }
        let nv = norm(v);
        if nv > 1e-6 * top {
            v.iter_mut().for_each(|x| *x /= nv);
        } else {
            v.fill(0.0);
        }
    }
    let mut q = vec![0f64; n * BLOCK];
    for (c, col) in cols.iter().enumerate() {
        for (j, &v) in col.iter().enumerate() {
            q[j * BLOCK + c] = v;
        }
    }
    q
}

/// The eigenvalues of the symmetric b x b matrix a (row-major; overwritten), largest first, and the eigenvectors as
/// the columns of a row-major b x b matrix, by cyclic Jacobi rotations.
fn jacobi_desc(a: &mut [f64], b: usize) -> (Vec<f64>, Vec<f64>) {
    let mut v = vec![0f64; b * b];
    (0..b).for_each(|i| v[i * b + i] = 1.0);
    for _ in 0..64 {
        let off: f64 = (0..b).flat_map(|p| (p + 1..b).map(move |q| (p, q))).map(|(p, q)| a[p * b + q] * a[p * b + q]).sum();
        let diag: f64 = (0..b).map(|p| a[p * b + p] * a[p * b + p]).sum();
        if off <= 1e-30 * diag || off == 0.0 {
            break;
        }
        for p in 0..b {
            for q in p + 1..b {
                let apq = a[p * b + q];
                if apq == 0.0 {
                    continue;
                }
                // the rotation that zeroes a[p][q]: tan of its angle t solves t^2 + 2 theta t - 1 = 0
                let theta = (a[q * b + q] - a[p * b + p]) / (2.0 * apq);
                let t = (if theta >= 0.0 { 1.0 } else { -1.0 }) / (theta.abs() + (theta * theta + 1.0).sqrt());
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..b {
                    let (x, y) = (a[k * b + p], a[k * b + q]);
                    a[k * b + p] = c * x - s * y;
                    a[k * b + q] = s * x + c * y;
                }
                for k in 0..b {
                    let (x, y) = (a[p * b + k], a[q * b + k]);
                    a[p * b + k] = c * x - s * y;
                    a[q * b + k] = s * x + c * y;
                }
                for k in 0..b {
                    let (x, y) = (v[k * b + p], v[k * b + q]);
                    v[k * b + p] = c * x - s * y;
                    v[k * b + q] = s * x + c * y;
                }
            }
        }
    }
    let mut o: Vec<usize> = (0..b).collect();
    o.sort_by(|&x, &y| a[y * b + y].total_cmp(&a[x * b + x]).then(x.cmp(&y)));
    let vals = o.iter().map(|&i| a[i * b + i]).collect();
    let mut vecs = vec![0f64; b * b];
    for (c, &i) in o.iter().enumerate() {
        for k in 0..b {
            vecs[k * b + c] = v[k * b + i];
        }
    }
    (vals, vecs)
}

impl OrderState {
    /// The state from its clusters' parts, in cluster order.
    pub fn from_clusters(parts: Vec<ClusterOrder>, r: usize, n0: usize) -> OrderState {
        let (mut rad, mut lam, mut dsc, mut dirs8) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for c in parts {
            rad.push(c.rad);
            lam.extend(c.lam);
            dsc.extend(c.dsc);
            dirs8.extend(c.dirs8);
        }
        OrderState { r, n0, rad: rad.into(), lam: lam.into(), dsc: dsc.into(), dirs8: dirs8.into() }
    }

    /// The state computed from the index's rows (as the build computes it from the rows it encodes).
    pub fn compute(ix: &Index2) -> OrderState {
        let n = ix.n();
        let r = r_of(n);
        let parts = (0..ix.k()).into_par_iter().map(|ci| {
            let c = &ix.cl[ci];
            let row = |i: usize, out: &mut [f32]| {
                let mut x = vec![0f64; n];
                ix.full_row_f64(c.r0 + i, &mut x);
                out.iter_mut().zip(&x).for_each(|(o, &v)| *o = v as f32);
            };
            cluster(c.r1 - c.r0, &row, ix.centre(ci), r, SAMPLE, PASSES, ci as u64)
        }).collect();
        OrderState::from_clusters(parts, r, spanned(&ix.centres32, n))
    }

    /// Cluster ci's first `r` directions as int8 rows (r x n).
    pub fn dirs(&self, ci: usize, r: usize, n: usize) -> &[i8] {
        &self.dirs8[ci * self.r * n..(ci * self.r + r) * n]
    }

    /// Cluster ci's variance per spanned dimension its directions leave: what its radius holds beyond its directions'
    /// variances, over the n0 - r other spanned dimensions.
    pub fn rest(&self, ci: usize) -> f64 {
        let r = self.r;
        let top: f64 = self.lam[ci * r..(ci + 1) * r].iter().map(|&l| l as f64).sum();
        (self.rad[ci] * self.rad[ci] - top).max(0.0) / (self.n0.max(r + 1) - r) as f64
    }
}

/// Passes of the subspace iteration on the pooled covariance's low-rank part (`Pooled::new`).
const POOL_PASSES: usize = 24;

/// The pooled within-cluster covariance C = sum_c n_c S_c / N of the clusters' stored covariances S_c (each its
/// directions and variances, `OrderState::lam`, and its isotropic rest, `OrderState::rest`), held as its top r
/// directions (int8 rows with a scale each), their variances and an isotropic rest, as `OrderState` holds a cluster's.
///
/// C = sum_c w_c U_c (L_c - rest_c) U_c' + (sum_c w_c rest_c) I, w_c = n_c / N, each column of U_c a cluster's
/// int8 direction scaled to unit length. Its top directions are those of the first term, an n x n matrix formed row by row in f64 (each entry summed over the clusters and directions in order),
/// by a subspace iteration of `POOL_PASSES` passes from random signs and a Rayleigh-Ritz step; its trace is
/// sum_c w_c rad_c^2, and the rest is what the trace holds beyond the top variances over the n0 - r other spanned
/// dimensions. Every operation runs in a fixed order with separate multiplies and adds, so it is the same bits
/// whatever the instruction set or the thread count. It costs about n^2 k r multiply-adds, rows in parallel (2.1e9
/// for 500 clusters of 512 dimensions), and no pass over the vectors, so it is computed when the approximate queries
/// open rather than stored.
pub struct Pooled {
    pub lam: Vec<f64>,
    pub dsc: Vec<f32>,
    pub dirs8: Vec<i8>,
    pub rest: f64,
    pub tr: f64,
}

impl Pooled {
    /// C of the state `st` of vectors of n dimensions whose clusters hold `sizes` rows.
    pub fn new(st: &OrderState, sizes: &[usize], n: usize) -> Pooled {
        let (k, r) = (sizes.len(), st.r);
        assert!(r <= BLOCK.min(n), "{r} directions of {n} dimensions do not fit a block of {BLOCK}");
        let total: usize = sizes.iter().sum();
        let w: Vec<f64> = sizes.iter().map(|&s| s as f64 / total.max(1) as f64).collect();
        let rests: Vec<f64> = (0..k).map(|c| st.rest(c)).collect();
        let tr: f64 = (0..k).map(|c| w[c] * st.rad[c] * st.rad[c]).sum();
        let iso: f64 = (0..k).map(|c| w[c] * rests[c]).sum();
        // each int8 direction's scale to unit length (the direction it rounds), so that the low-rank part's trace is
        // the clusters' variances, whatever the rounding
        let unit: Vec<f64> = st.dirs8[..k * r * n].chunks_exact(n.max(1)).map(|u| {
            let nn: f64 = u.iter().map(|&x| x as f64 * x as f64).sum();
            if nn > 0.0 { 1.0 / nn.sqrt() } else { 0.0 }
        }).collect();
        // the low-rank part, row by row
        let m: Vec<f64> = (0..n).into_par_iter().flat_map_iter(|i| {
            let mut row = vec![0f64; n];
            for c in (0..k).filter(|&c| w[c] > 0.0) {
                for j in 0..r {
                    let d = st.dirs(c, r, n);
                    let (u, sc) = (&d[j * n..(j + 1) * n], unit[c * r + j]);
                    if u[i] == 0 {
                        continue;
                    }
                    let a = w[c] * (st.lam[c * r + j] as f64 - rests[c]) * (u[i] as f64 * sc) * sc;
                    for (o, &x) in row.iter_mut().zip(u) {
                        *o += a * x as f64;
                    }
                }
            }
            row
        }).collect();
        let times = |x: &[f64]| -> Vec<f64> {
            let mut y = vec![0f64; n * BLOCK];
            for (mi, yi) in m.chunks_exact(n).zip(y.chunks_exact_mut(BLOCK)) {
                for (&a, xj) in mi.iter().zip(x.chunks_exact(BLOCK)) {
                    for c in 0..BLOCK {
                        yi[c] += a * xj[c];
                    }
                }
            }
            y
        };
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0);
        let mut x: Vec<f64> = (0..n * BLOCK).map(|_| if rng.gen::<bool>() { 1.0 } else { -1.0 }).collect();
        for _ in 0..POOL_PASSES {
            x = orthonormal64(&times(&x), n);
        }
        let y = times(&x);
        let mut b = vec![0f64; BLOCK * BLOCK];
        for (xi, yi) in x.chunks_exact(BLOCK).zip(y.chunks_exact(BLOCK)) {
            for a in 0..BLOCK {
                for (c, o) in b[a * BLOCK..(a + 1) * BLOCK].iter_mut().enumerate() {
                    *o += xi[a] * yi[c];
                }
            }
        }
        // symmetric up to rounding: its two halves averaged
        for a in 0..BLOCK {
            for c in a + 1..BLOCK {
                let v = 0.5 * (b[a * BLOCK + c] + b[c * BLOCK + a]);
                b[a * BLOCK + c] = v;
                b[c * BLOCK + a] = v;
            }
        }
        let (vals, vecs) = jacobi_desc(&mut b, BLOCK);
        let (mut lam, mut dsc, mut dirs8) = (Vec::with_capacity(r), Vec::with_capacity(r), Vec::with_capacity(r * n));
        for j in 0..r {
            let u: Vec<f64> = (0..n).map(|i| (0..BLOCK).map(|c| x[i * BLOCK + c] * vecs[c * BLOCK + j]).sum()).collect();
            let sc = u.iter().fold(0f64, |a, &v| a.max(v.abs())) / 127.0;
            dirs8.extend(u.iter().map(|&v| if sc > 0.0 { (v / sc).round() as i8 } else { 0 }));
            dsc.push(sc as f32);
            lam.push((vals[j] + iso).max(0.0));
        }
        let rest = (tr - lam.iter().sum::<f64>()).max(0.0) / (st.n0.max(r + 1) - r) as f64;
        Pooled { lam, dsc, dirs8, rest, tr }
    }

    /// q' C q for the query's f32 values `q32` and |q|^2 `qq` (`p` scratch of r values): one product with the r
    /// int8 rows.
    pub fn quad(&self, q32: &[f32], qq: f64, p: &mut [f32]) -> f64 {
        let r = self.lam.len();
        unsafe { crate::kernel::gemv_i8(q32, &self.dirs8, q32.len(), &mut p[..r]) };
        let (mut v, mut pp) = (0f64, 0f64);
        for ((&pj, &sc), &l) in p[..r].iter().zip(&self.dsc).zip(&self.lam) {
            let x = pj as f64 * sc as f64;
            v += l * x * x;
            pp += x * x;
        }
        (v + self.rest * (qq - pp).max(0.0)).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cluster's top r principal directions about its centre (r x n, unit rows), their variances and its covariance
    /// (n x n), from all its rows and the full eigendecomposition, in f64.
    struct Exact {
        lam: Vec<f64>,
        u: Vec<f64>,
        cov: Vec<f64>,
    }

    fn exact(rows: usize, row: &dyn Fn(usize, &mut [f32]), centre: &[f32], r: usize) -> Exact {
        use faer::{Accum, Mat, Par, Side};
        let n = centre.len();
        let mut x = Mat::<f64>::zeros(rows.max(1), n);
        let mut buf = vec![0f32; n];
        for i in 0..rows {
            row(i, &mut buf);
            for j in 0..n {
                x[(i, j)] = buf[j] as f64 - centre[j] as f64;
            }
        }
        let mut cov = Mat::<f64>::zeros(n, n);
        faer::linalg::matmul::matmul(cov.as_mut(), Accum::Replace, x.transpose(), x.as_ref(), 1.0 / rows.max(1) as f64, Par::Seq);
        let e = cov.self_adjoint_eigen(Side::Lower).unwrap();
        let (vals, u) = (e.S().column_vector(), e.U());
        let mut o: Vec<usize> = (0..n).collect();
        o.sort_by(|&a, &b| vals[b].total_cmp(&vals[a]));
        Exact {
            lam: o[..r].iter().map(|&j| vals[j]).collect(),
            u: o[..r].iter().flat_map(|&j| (0..n).map(move |i| u[(i, j)])).collect(),
            cov: (0..n * n).map(|i| cov[(i / n, i % n)]).collect(),
        }
    }

    /// How far a cluster's state is from the exact one: the relative error of each variance (of those above 1e-9 of the
    /// first), and for the first j directions, j in `spans`, the largest principal angle (radians) between the exact
    /// and the estimated spans, and the share of the exact first j's variance the estimated span captures.
    struct Errs {
        rel: Vec<f64>,
        ang: Vec<f64>,
        cap: Vec<f64>,
    }

    fn errors(co: &ClusterOrder, ex: &Exact, n: usize, spans: &[usize]) -> Errs {
        let rel = ex.lam.iter().zip(&co.lam).map(|(&e, &a)| if e > 1e-9 * ex.lam[0] { (a as f64 - e).abs() / e } else { 0.0 }).collect();
        // an orthonormal basis of the int8 directions' first j, for every j (Gram-Schmidt in order)
        let mut est: Vec<Vec<f64>> = Vec::new();
        for d in co.dirs8.chunks_exact(n) {
            let mut v: Vec<f64> = d.iter().map(|&x| x as f64).collect();
            for u in &est {
                let p: f64 = u.iter().zip(&v).map(|(a, b)| a * b).sum();
                v.iter_mut().zip(u).for_each(|(x, &a)| *x -= p * a);
            }
            let s = v.iter().map(|x| x * x).sum::<f64>().sqrt();
            est.push(if s > 0.0 { v.iter().map(|x| x / s).collect() } else { v });
        }
        let ang = spans.iter().map(|&j| {
            // cosines of the principal angles: the singular values of U_exact U_est' (j x j)
            let mut m = vec![0f64; j * j];
            for a in 0..j {
                for b in 0..j {
                    m[a * j + b] = (0..n).map(|i| ex.u[a * n + i] * est[b][i]).sum();
                }
            }
            let mut mtm = vec![0f64; j * j];
            for a in 0..j {
                for b in 0..j {
                    mtm[a * j + b] = (0..j).map(|c| m[c * j + a] * m[c * j + b]).sum();
                }
            }
            let (s, _) = jacobi_desc(&mut mtm, j);
            s[j - 1].clamp(0.0, 1.0).sqrt().acos()
        }).collect();
        let var = |v: &[f64]| -> f64 { (0..n).map(|a| v[a] * (0..n).map(|b| ex.cov[a * n + b] * v[b]).sum::<f64>()).sum() };
        let cap = spans.iter().map(|&j| est[..j].iter().map(|v| var(v)).sum::<f64>() / ex.lam[..j].iter().sum::<f64>().max(1e-300)).collect();
        Errs { rel, ang, cap }
    }

    /// The products taken four rows at a time give the bits of the plain sums, each over the dimensions (x q) or the
    /// rows (x' z) in order.
    #[test]
    fn the_blocked_products_are_the_plain_sums() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(9);
        for (s, n) in [(1, 1), (3, 5), (4, 64), (7, 33), (9, 64), (13, 100)] {
            let x: Vec<f32> = (0..s * n).map(|_| rng.gen_range(-1.0..1.0)).collect();
            let q: Vec<f32> = (0..n * BLOCK).map(|_| rng.gen_range(-1.0..1.0)).collect();
            let zr: Vec<f32> = (0..s * BLOCK).map(|_| rng.gen_range(-1.0..1.0)).collect();
            let (mut z, mut y) = (vec![0f32; s * BLOCK], vec![0f32; n * BLOCK]);
            times_q(&x, n, &q, &mut z);
            transpose_times_z(&x, n, &zr, &mut y);
            for i in 0..s {
                for c in 0..BLOCK {
                    let want = (0..n).fold(0f32, |a, j| a + x[i * n + j] * q[j * BLOCK + c]);
                    assert_eq!(z[i * BLOCK + c].to_bits(), want.to_bits(), "x q, {s} x {n}, row {i}");
                }
            }
            for j in 0..n {
                for c in 0..BLOCK {
                    let want = (0..s).fold(0f32, |a, i| a + x[i * n + j] * zr[i * BLOCK + c]);
                    assert_eq!(y[j * BLOCK + c].to_bits(), want.to_bits(), "x' z, {s} x {n}, dimension {j}");
                }
            }
        }
    }

    /// Jacobi's eigenpairs of a symmetric matrix satisfy A v = lambda v, largest first, with orthonormal vectors.
    #[test]
    fn jacobi_finds_the_eigenpairs() {
        let b = 12;
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(5);
        let g: Vec<f64> = (0..b * b).map(|_| rng.gen_range(-1.0..1.0)).collect();
        let a: Vec<f64> = (0..b * b).map(|x| (0..b).map(|c| g[(x / b) * b + c] * g[(x % b) * b + c]).sum()).collect();
        let (vals, vecs) = jacobi_desc(&mut a.clone(), b);
        assert!(vals.windows(2).all(|w| w[0] >= w[1]));
        for k in 0..b {
            for i in 0..b {
                let av: f64 = (0..b).map(|j| a[i * b + j] * vecs[j * b + k]).sum();
                assert!((av - vals[k] * vecs[i * b + k]).abs() < 1e-10, "A v != lambda v at pair {k}");
            }
            for l in 0..b {
                let d: f64 = (0..b).map(|i| vecs[i * b + k] * vecs[i * b + l]).sum();
                assert!((d - if k == l { 1.0 } else { 0.0 }).abs() < 1e-10);
            }
        }
    }

    /// On synth4k, whose clusters are smaller than the sample, the stored state (every row; randomised subspace
    /// iteration) has the exact covariance's variances and directions: the first 16 variances within 5e-3, the first
    /// direction and the span of the first 4 within the int8 rounding (0.03 rad), and the first 16 capturing all but
    /// 1e-3 of the exact first 16's variance (synth4k's clusters are nearly isotropic past their first directions, so
    /// the 16th is barely apart from the 17th). On its clusters of at least 150 rows, from a sample of half of them,
    /// the first variance is within 25% and the first direction within 0.25 rad.
    #[test]
    fn the_stored_directions_are_the_principal_ones() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
        let (_, ix) = build(dir, &BuildOptions::default(), &mut |_| {}).unwrap();
        let st = ix.order.as_ref().unwrap();
        let (n, r) = (ix.n(), st.r);
        let (mut rel_max, mut ang_max, mut cap_min, mut half_rel, mut half_ang) = (0f64, [0f64; 2], 1f64, 0f64, 0f64);
        for ci in 0..ix.k() {
            let c = &ix.cl[ci];
            let rows = c.r1 - c.r0;
            let row = |i: usize, out: &mut [f32]| out.iter_mut().zip(ix.full_row_f16(c.r0 + i)).for_each(|(o, &h)| *o = f16_to_f32(h));
            let ex = exact(rows, &row, ix.centre(ci), r);
            let co = ClusterOrder { rad: st.rad[ci], lam: st.lam[ci * r..(ci + 1) * r].to_vec(), dsc: st.dsc[ci * r..(ci + 1) * r].to_vec(), dirs8: st.dirs(ci, r, n).to_vec() };
            let e = errors(&co, &ex, n, &[1, 4, 16]);
            rel_max = e.rel.iter().fold(rel_max, |a, &b| a.max(b));
            if rows > r {
                ang_max = [ang_max[0].max(e.ang[0]), ang_max[1].max(e.ang[1])];
                cap_min = cap_min.min(e.cap[2]);
            }
            if rows >= 150 {
                let half = errors(&cluster(rows, &row, ix.centre(ci), r, rows / 2, PASSES, ci as u64), &ex, n, &[1]);
                (half_rel, half_ang) = (half_rel.max(half.rel[0]), half_ang.max(half.ang[0]));
            }
        }
        eprintln!("synth4k: variances' largest relative error {rel_max:.1e}; largest angle of the first direction {:.3}, of the first 4 {:.3}; least share of the first 16's variance captured {cap_min:.6}; from half the rows, first variance {half_rel:.3}, first direction {half_ang:.3}", ang_max[0], ang_max[1]);
        assert!(rel_max < 5e-3 && ang_max[0] < 0.03 && ang_max[1] < 0.03 && cap_min > 1.0 - 1e-3, "{rel_max} {ang_max:?} {cap_min}");
        assert!(half_rel < 0.25 && half_ang < 0.25, "{half_rel} {half_ang}");
    }

    /// The sampled randomised state against the exact one on the clusters of a real fit (`VEXACT_ORDER_FIT`: a fitted
    /// directory with an Xs.npy; `VEXACT_ORDER_CLUSTERS`: how many clusters, spread over the fit, default 40), for
    /// several sample sizes and passes (`VEXACT_ORDER_RUNS`, e.g. 4096x4,1024x4): the relative error of variances 1,
    /// 4 and 16, the largest principal angle of the first 4 and 16 directions and the share of their variance missed,
    /// the relative error of sigma_c(q) with 4 and 16 directions for 64 centres as queries (the exact rest from the
    /// exact trace, the estimated one from the radius), and the time a cluster takes.
    #[test]
    #[ignore]
    fn sampled_directions_against_the_exact_ones_on_a_real_fit() {
        let Ok(dir) = std::env::var("VEXACT_ORDER_FIT") else { return };
        let p = crate::index::load_params(&dir).unwrap();
        let xs = crate::index::RealSource::try_open(&p.xs_path).unwrap();
        let (n, k) = (p.meta.n, p.meta.k);
        let r = r_of(n);
        let n0 = spanned(&p.centres[..k * n], n);
        let want: usize = std::env::var("VEXACT_ORDER_CLUSTERS").map_or(40, |s| s.parse().unwrap());
        let cls = spread(k, want);
        let med = |mut v: Vec<f64>| -> (f64, f64, f64) {
            v.sort_by(|a, b| a.total_cmp(b));
            (v[v.len() / 2], v[(v.len() * 9) / 10], v[v.len() - 1])
        };
        let queries: Vec<Vec<f64>> = spread(k, 64).iter().map(|&c| {
            let m = &p.centres[c * n..(c + 1) * n];
            let s = m.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt();
            m.iter().map(|&x| x as f64 / s).collect()
        }).collect();
        // sigma^2 = sum lam_j (u_j.q)^2 + rest (|q|^2 - sum (u_j.q)^2), the rest from the trace (as `adaptive` has it)
        let sigma = |lam: &[f64], u: &[Vec<f64>], tr: f64, q: &[f64]| -> f64 {
            let j = lam.len();
            let rest = (tr - lam.iter().sum::<f64>()).max(0.0) / (n0.max(j + 1) - j) as f64;
            let (mut v, mut pp) = (0.0, 0.0);
            for (l, uj) in lam.iter().zip(u) {
                let x: f64 = uj.iter().zip(q).map(|(a, b)| a * b).sum();
                v += l * x * x;
                pp += x * x;
            }
            (v + rest * (1.0 - pp).max(0.0)).sqrt()
        };
        let rows_of = |ci: usize| (p.cstart[ci], p.cstart[ci + 1] - p.cstart[ci]);
        let t = crate::util::Timer::new();
        let exacts: Vec<Exact> = cls.iter().map(|&ci| {
            let (r0, m) = rows_of(ci);
            let row = |i: usize, out: &mut [f32]| out.iter_mut().zip(xs.row(r0 + i)).for_each(|(o, &h)| *o = f16_to_f32(h));
            exact(m, &row, &p.centres[ci * n..(ci + 1) * n], r)
        }).collect();
        let mean_rows = cls.iter().map(|&c| rows_of(c).1).sum::<usize>() as f64 / cls.len() as f64;
        eprintln!("{dir}: {} of {k} clusters, {mean_rows:.0} rows on average, n {n} (spanning {n0}); exact {:.3}s a cluster on one thread", cls.len(), t.s() / cls.len() as f64);
        let runs: Vec<(usize, usize)> = match std::env::var("VEXACT_ORDER_RUNS") {
            Ok(s) => s.split(',').map(|x| x.split_once('x').map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap())).expect("runs as SAMPLExPASSES,...")).collect(),
            Err(_) => vec![(SAMPLE, PASSES), (SAMPLE, 2), (SAMPLE, 1), (SAMPLE, 8), (1024, PASSES), (512, PASSES), (256, PASSES)],
        };
        for (sample, passes) in runs {
            let t = crate::util::Timer::new();
            let mut acc: Vec<Vec<f64>> = vec![Vec::new(); 9];
            for (&ci, ex) in cls.iter().zip(&exacts) {
                let (r0, m) = rows_of(ci);
                let row = |i: usize, out: &mut [f32]| out.iter_mut().zip(xs.row(r0 + i)).for_each(|(o, &h)| *o = f16_to_f32(h));
                let co = cluster(m, &row, &p.centres[ci * n..(ci + 1) * n], r, sample, passes, ci as u64);
                let e = errors(&co, ex, n, &[4, 16]);
                for (v, x) in acc.iter_mut().zip([e.rel[0], e.rel[3], e.rel[15], e.ang[0], e.ang[1], 1.0 - e.cap[0], 1.0 - e.cap[1]]) {
                    v.push(x);
                }
                let ue: Vec<Vec<f64>> = ex.u.chunks_exact(n).map(|x| x.to_vec()).collect();
                let ua: Vec<Vec<f64>> = co.dirs8.chunks_exact(n).zip(&co.dsc).map(|(d, &s)| d.iter().map(|&x| x as f64 * s as f64).collect()).collect();
                let la: Vec<f64> = co.lam.iter().map(|&x| x as f64).collect();
                let tr: f64 = (0..n).map(|i| ex.cov[i * n + i]).sum();
                for q in &queries {
                    for (slot, j) in [(7, 4), (8, 16)] {
                        let (se, sa) = (sigma(&ex.lam[..j], &ue[..j], tr, q), sigma(&la[..j], &ua[..j], co.rad * co.rad, q));
                        acc[slot].push((sa - se).abs() / se);
                    }
                }
            }
            let secs = t.s() / cls.len() as f64;
            let names = ["rel err lambda1", "lambda4", "lambda16", "angle(4)", "angle(16)", "1 - captured(4)", "1 - captured(16)", "rel err sigma4(q)", "sigma16(q)"];
            let line: Vec<String> = names.iter().zip(acc).map(|(nm, v)| {
                let (a, b, c) = med(v);
                format!("{nm} {a:.4} / {b:.4} / {c:.4}")
            }).collect();
            eprintln!("sample {sample}, passes {passes}: {secs:.3}s a cluster on one thread; median / 90% / max: {}", line.join("; "));
        }
    }
}
