//! The W2 width's stored forms: what the approximate k-NN needs at build time to predict, per query, the spread of a
//! row's error at each deterministic level of a cluster (`adaptive::WidthRule::W2`).
//!
//! Notation (the theory session's, as `study_w_rule` replayed it): at level l of group g a row's error vector in the
//! group's weighted coordinates is u = (z - s sig val_l) / d_l (z the row in the group's basis, s its scale, val_l the
//! level's decoded value), so a query with D = (V_q^T q) d_l has the error D.u. S_c = mean u u' over cluster c's rows
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

/// An orthonormal basis of the span of `vs` (Gram-Schmidt, twice; vectors left with no length dropped).
pub fn orthonormal(vs: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let mut qb: Vec<Vec<f64>> = Vec::new();
    for v in vs {
        let mut b = v.clone();
        for _ in 0..2 {
            for w in &qb {
                let dt: f64 = b.iter().zip(w).map(|(x, y)| x * y).sum();
                b.iter_mut().zip(w).for_each(|(x, y)| *x -= dt * y);
            }
        }
        let nb = b.iter().map(|x| x * x).sum::<f64>().sqrt();
        if nb > 1e-9 {
            qb.push(b.iter().map(|x| x / nb).collect());
        }
    }
    qb
}

/// Cluster c's rows' error vectors per level (nc x n, index row order), the encoding replayed from the index's stored
/// rows with its group's inverse basis `vinv` and its centre `centre`, as the build encoded them.
pub fn errs(ix: &Index2, vinv: &[f64], centre: &[f32], c: usize) -> Vec<Vec<f64>> {
    let (n, ndl) = (ix.n(), ix.meta.ndl());
    let cl = &ix.cl[c];
    let gp = &ix.groups[cl.g];
    let nc = cl.r1 - cl.r0;
    let mut rmat = vec![0f64; nc * n];
    for i in 0..nc {
        let x = ix.full_row_f16(cl.r0 + i);
        for t in 0..n {
            rmat[i * n + t] = crate::util::f16_to_f32(x[t]) as f64 - centre[t] as f64;
        }
    }
    let mut z = vec![0f64; nc * n];
    unsafe {
        matrixmultiply::dgemm(nc, n, n, 1.0, rmat.as_ptr(), n as isize, 1, vinv.as_ptr(), 1, n as isize, 0.0, z.as_mut_ptr(), n as isize, 1);
    }
    let mut u = vec![vec![0f64; nc * n]; ndl];
    let mut val = vec![0f64; n];
    for i in 0..nc {
        let s = crate::index::item_scale(ix.ahat[cl.r0 + i], cl.mn32, gp.lamsum) as f64;
        val.iter_mut().for_each(|v| *v = 0.0);
        let zi = &z[i * n..(i + 1) * n];
        for (l, ul) in u.iter_mut().enumerate() {
            for f in &gp.levels[l].parts[0].fields {
                let cc = f.widx as usize;
                let uu = zi[cc] / (gp.sig[cc] as f64 * s);
                let code = match f.kind {
                    FK::Tree { to } => ix.codes.index(to as usize, uu),
                    _ => (uu - val[cc] >= 0.0) as usize,
                };
                val[cc] += ix.etab.e[f.eid as usize][code << f.off] as f64;
            }
            for cc in 0..n {
                ul[i * n + cc] = (zi[cc] - s * gp.sig[cc] as f64 * val[cc]) / gp.d[l][cc] as f64;
            }
        }
    }
    u
}

/// Cluster c's order directions p_j (data space, as stored: int8 rows times their scale) and their images
/// b_j = (V_q^T p_j) d_l at level l.
pub fn order_dirs(ix: &Index2, c: usize, l: usize) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
    let n = ix.n();
    let st = ix.order.as_ref().unwrap();
    let gp = &ix.groups[ix.cl[c].g];
    let ps: Vec<Vec<f64>> = (0..st.r).map(|j| st.dirs(c, st.r, n)[j * n..(j + 1) * n].iter().map(|&x| x as f64 * st.dsc[c * st.r + j] as f64).collect()).collect();
    let bs = ps.iter().map(|pd| (0..n).map(|t| gp.vsc[t] * gp.v8t[t * n..(t + 1) * n].iter().zip(pd).map(|(&v, &x)| v as f64 * x).sum::<f64>() * gp.d[l][t] as f64).collect()).collect();
    (ps, bs)
}

/// B~ = B (P'P)^-1 (r vectors of n) and an orthonormal basis of span B, for cluster c at level l. A direction the
/// order state left zero (a cluster too small to span r) has a zero column in B~ (its coefficient is 0 too), and
/// P'P is inverted over the others.
fn dual_dirs(ix: &Index2, c: usize, l: usize) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
    let n = ix.n();
    let dot = |x: &[f64], y: &[f64]| x.iter().zip(y).map(|(a, b)| a * b).sum::<f64>();
    let (ps, bs) = order_dirs(ix, c, l);
    let live: Vec<usize> = (0..ps.len()).filter(|&j| ps[j].iter().any(|&x| x != 0.0)).collect();
    let r = live.len();
    let ptp: Vec<f64> = (0..r * r).map(|i| dot(&ps[live[i / r]], &ps[live[i % r]])).collect();
    let pinv = gauss_jordan_inverse(&ptp, r);
    let mut bt = vec![vec![0f64; n]; ps.len()];
    for (jj, &j) in live.iter().enumerate() {
        bt[j] = (0..n).map(|t| (0..r).map(|kk| bs[live[kk]][t] * pinv[kk * r + jj]).sum()).collect();
    }
    (bt, orthonormal(&bs))
}

/// The forms of `ix` (which holds stored vectors and the order state), with each group's inverse basis `vinv` (as
/// the build's `GroupPlan2::vinv`) and the clusters' centres `centres` (k x n). Every row of every cluster: each
/// cluster's errors replayed once (`errs`), from which M, tr S_c and tr S_c within span B, and G's sums; then N and
/// gamma from G.
pub fn build(ix: &Index2, vinv: &[Vec<f64>], centres: &[f32]) -> WForms {
    use rayon::prelude::*;
    let (n, ndl, k, ng) = (ix.n(), ix.meta.ndl(), ix.k(), ix.groups.len());
    let r = ix.order.as_ref().expect("the W2 forms read the order state").r;
    // per cluster and level: M (r x r), tr S, tr S within span B; and the cluster's sums of u_t^2 per level
    struct One {
        m: Vec<Vec<f64>>,
        trs: Vec<f64>,
        trq: Vec<f64>,
        usq: Vec<Vec<f64>>,
    }
    let ones: Vec<One> = (0..k).into_par_iter().map(|c| {
        let cl = &ix.cl[c];
        let nc = cl.r1 - cl.r0;
        if nc == 0 {
            return One { m: vec![vec![0.0; r * r]; ndl], trs: vec![0.0; ndl], trq: vec![0.0; ndl], usq: vec![vec![0.0; n]; ndl] };
        }
        let u = errs(ix, &vinv[cl.g], &centres[c * n..(c + 1) * n], c);
        let mut one = One { m: Vec::new(), trs: Vec::new(), trq: Vec::new(), usq: Vec::new() };
        for (l, ul) in u.iter().enumerate() {
            let (bt, qb) = dual_dirs(ix, c, l);
            // Y = U B~ (nc x r), M = Y'Y / nc; the same for an orthonormal basis of span B
            let proj = |vs: &[Vec<f64>]| -> Vec<f64> {
                let w: Vec<f64> = (0..n).flat_map(|t| vs.iter().map(move |v| v[t])).collect();
                let mut y = vec![0f64; nc * vs.len()];
                unsafe {
                    matrixmultiply::dgemm(nc, n, vs.len(), 1.0, ul.as_ptr(), n as isize, 1, w.as_ptr(), vs.len() as isize, 1, 0.0, y.as_mut_ptr(), vs.len() as isize, 1);
                }
                y
            };
            let (y, yq) = (proj(&bt), proj(&qb));
            let mut m = vec![0f64; r * r];
            for i in 0..nc {
                let yi = &y[i * r..(i + 1) * r];
                for a in 0..r {
                    for b in 0..r {
                        m[a * r + b] += yi[a] * yi[b];
                    }
                }
            }
            m.iter_mut().for_each(|x| *x /= nc as f64);
            one.m.push(m);
            one.trq.push(yq.iter().map(|x| x * x).sum::<f64>() / nc as f64);
            let mut usq = vec![0f64; n];
            for i in 0..nc {
                for t in 0..n {
                    usq[t] += ul[i * n + t] * ul[i * n + t];
                }
            }
            one.trs.push(usq.iter().sum::<f64>() / nc as f64);
            one.usq.push(usq);
        }
        one
    }).collect();
    // G per group and level: mean u_t^2 over the group's rows
    let mut g = vec![0f64; ng * ndl * n];
    let mut rows = vec![0f64; ng];
    for (c, one) in ones.iter().enumerate() {
        let gi = ix.cl[c].g;
        rows[gi] += (ix.cl[c].r1 - ix.cl[c].r0) as f64;
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
        let (cl, one) = (&ix.cl[c], &ones[c]);
        let nc = cl.r1 - cl.r0;
        let mut out = Vec::with_capacity(ndl * WForms::rec(r));
        for l in 0..ndl {
            let gd = &g[(cl.g * ndl + l) * n..(cl.g * ndl + l + 1) * n];
            let gq = |x: &[f64], y: &[f64]| -> f64 { (0..n).map(|t| x[t] * gd[t] * y[t]).sum() };
            let (bt, qb) = if nc > 0 { dual_dirs(ix, c, l) } else { (vec![vec![0.0; n]; r], Vec::new()) };
            let nn: Vec<f64> = (0..r * r).map(|i| gq(&bt[i / r], &bt[i % r])).collect();
            let den = gd.iter().sum::<f64>() - qb.iter().map(|w| gq(w, w)).sum::<f64>();
            let gamma = if den > 0.0 { ((one.trs[l] - one.trq[l]) / den).max(0.0) } else { 0.0 };
            let uu = if nc > 0 { (cl.r0..cl.r1).map(|row| det_norm(ix, c, l, row).powi(2)).sum::<f64>() / nc as f64 } else { 0.0 };
            out.extend(pack(&one.m[l], r));
            out.extend(pack(&nn, r));
            out.extend([gamma as f32, uu as f32]);
        }
        out
    }).collect();
    WForms { r, g: g.iter().map(|&x| x as f32).collect(), c: recs.concat() }
}
