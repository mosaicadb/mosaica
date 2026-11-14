//! The fast-scan code: every level is a list of self-contained 4-bit units, so a level is scored with 16-entry tables
//! over 32-item blocks with transposed nibbles.
//!
//! Deterministic levels: coordinate c of the local basis is coded additively, one bit per stage: at depth b its
//! normalised value is v = sum_{d<=b} delta_{c,d} t_d, t_d = +-1, so each new bit adds +-delta independently of the
//! earlier bits. The deltas are fitted per bit trajectory (which depth the coordinate has at each level) and the
//! encoder picks the bit pattern minimising a weighted sum of the errors of every prefix. The stored weighted error
//! norm makes any code sound.
//!
//! Randomised levels: each rotated position is a nested uniform binary expansion (delta_d = 2^-d) of y_p / (lambda rms), lambda chosen per item and block;
//! the estimator scale kappa = |y|^2 / <c, y> is stored per rise (8 bits relative to the stored |y|), so |c| never has
//! to be decoded; tan(phi) is stored per rise as before.
//!
//! The basis of each group is stored in int8 (one scale per basis vector). The encoder codes z = V_q^-1 r, so
//! q.r = (V_q^T q).z + q.eps with |eps| <= e_inv |r|, and V_q^T q is computed exactly in integers.
pub mod col;
pub mod kern;
pub mod q;
pub mod setup;
pub mod tail;

/// Keep the sub-Gaussian constant at every randomised level (`--csg-plain`) instead of the exact-law constants of `tail`.
pub static CSG_PLAIN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Design the randomised rates with the constants of `tail` for each set of coded blocks (`--design-exact`), indexed
/// by the set's bits.
pub static DESIGN_C: std::sync::OnceLock<Vec<f64>> = std::sync::OnceLock::new();

use crate::derive;
use crate::index::{item_scale, Params, DA, SEED};
use crate::util::*;
use rayon::prelude::*;
use std::collections::HashMap;

pub const NDL: usize = 6;
pub const J: usize = 4;
pub const BLK: usize = 32;
pub const LAMS: [f64; 6] = [2.0, 2.3, 2.6, 3.0, 3.5, 4.2];
pub const OFF: [i32; 15] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 18];
pub const QUANTUM: usize = 16;

// ---------------- the deterministic code ----------------

/// Acklam's inverse normal CDF (relative error ~1e-9).
pub fn inv_norm(p: f64) -> f64 {
    let a = [-3.969683028665376e+01, 2.209460984245205e+02, -2.759285104469687e+02, 1.383577518672690e+02, -3.066479806614716e+01, 2.506628277459239e+00];
    let b = [-5.447609879822406e+01, 1.615858368580409e+02, -1.556989798598866e+02, 6.680131188771972e+01, -1.328068155288572e+01];
    let c = [-7.784894002430293e-03, -3.223964580411365e-01, -2.400758277161838e+00, -2.549732539343734e+00, 4.374664141464968e+00, 2.938163982698783e+00];
    let d = [7.784695709041462e-03, 3.224671290700398e-01, 2.445134137142996e+00, 3.754408661907416e+00];
    let pl = 0.02425;
    if p < pl {
        let q = (-2.0 * p.ln()).sqrt();
        (((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5]) / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0)
    } else if p <= 1.0 - pl {
        let q = p - 0.5;
        let r = q * q;
        (((((a[0] * r + a[1]) * r + a[2]) * r + a[3]) * r + a[4]) * r + a[5]) * q / (((((b[0] * r + b[1]) * r + b[2]) * r + b[3]) * r + b[4]) * r + 1.0)
    } else {
        -inv_norm(1.0 - p)
    }
}

/// The deterministic code's tables: nested tree centroids to depth 4 (a field re-stores a coordinate's full index, so
/// the step from depth j to k, cent_k[i] - cent_j[i >> (k - j)], depends on the field alone) and 1-bit residual stages
/// after a tree depth (+-delta, delta = E|residual|), with model distortions on N(0,1).
pub struct DetTab {
    pub cent: Vec<Vec<f32>>,  // depth 0..=4
    pub edges: Vec<Vec<f64>>, // depth 0..=4
    pub delta: [[f32; 4]; 5], // [tree depth][stage]
    pub d: [[f64; 5]; 5],     // [tree depth][stages done] distortion
}
pub const TREE_DEPTHS: [usize; 3] = [1, 2, 4];
pub const MAX_STAGES: usize = 4;

impl DetTab {
    pub fn new(p: &Params) -> DetTab {
        let mut cent = vec![vec![0f32]];
        let mut edges = vec![Vec::new()];
        for b in 1..=4 {
            cent.push((0..1usize << b).map(|i| p.tree_cent[b * 256 + i] as f32).collect());
            edges.push((0..(1usize << b) - 1).map(|i| p.tree_edges[b * 256 + i]).collect());
        }
        let ns = 200000;
        let xs: Vec<f64> = (0..ns).map(|i| inv_norm((i as f64 + 0.5) / ns as f64)).collect();
        let mut delta = [[0f32; 4]; 5];
        let mut d = [[1f64; 5]; 5];
        for &j in &TREE_DEPTHS {
            let mut r: Vec<f64> = xs.iter().map(|&x| x - cent[j][edges[j].partition_point(|&e| e < x)] as f64).collect();
            d[j][0] = r.iter().map(|v| v * v).sum::<f64>() / ns as f64;
            for st in 0..MAX_STAGES {
                let dl = (r.iter().map(|v| v.abs()).sum::<f64>() / ns as f64) as f32;
                delta[j][st] = dl;
                for v in r.iter_mut() {
                    *v -= if *v >= 0.0 { dl as f64 } else { -(dl as f64) };
                }
                d[j][st + 1] = r.iter().map(|v| v * v).sum::<f64>() / ns as f64;
            }
        }
        DetTab { cent, edges, delta, d }
    }
    #[inline]
    pub fn index(&self, b: usize, u: f64) -> usize {
        self.edges[b].partition_point(|&e| e < u)
    }
}

/// A deterministic move of the nested allocation.
#[derive(Clone, Copy, Debug)]
pub enum Move {
    Tree { c: usize, from: u8, to: u8 },
    Resid { c: usize, j: u8, r: u8 },
}

/// Greedy nested allocation on sum sqrt(lam Q D(state)): a tree field to depth k in {1, 2, 4} (costs k bits, re-stores
/// the full index; not after residual stages) or a 1-bit residual stage; largest gain per bit, first maximum on ties.
/// Returns the moves of each level and the state (tree depth, stages) of every coordinate after each level.
pub fn det_alloc_tree(lam: &[f64], q: &[f64], t: &DetTab, dlv: &[usize]) -> (Vec<Vec<Move>>, Vec<Vec<(u8, u8)>>) {
    let n = lam.len();
    let a: Vec<f64> = (0..n).map(|c| (q[c] * lam[c]).sqrt()).collect();
    let mut st = vec![(0u8, 0u8); n];
    let sd = |s: (u8, u8)| t.d[s.0 as usize][s.1 as usize].sqrt();
    let mut used = 0;
    let mut moves = Vec::new();
    let mut states = Vec::new();
    for &lb in dlv {
        let mut mv = Vec::new();
        while used < 8 * lb {
            let rem = 8 * lb - used;
            let mut best: Option<(f64, Move, usize)> = None;
            for &k in &TREE_DEPTHS {
                if k > rem {
                    continue;
                }
                for c in 0..n {
                    let s = st[c];
                    if s.0 as usize >= k || s.1 > 0 {
                        continue;
                    }
                    let gpb = a[c] * (sd(s) - t.d[k][0].sqrt()) / k as f64;
                    if best.map_or(true, |b| gpb > b.0) {
                        best = Some((gpb, Move::Tree { c, from: s.0, to: k as u8 }, k));
                    }
                }
            }
            for c in 0..n {
                let s = st[c];
                if s.0 == 0 || s.1 as usize >= MAX_STAGES {
                    continue;
                }
                let gpb = a[c] * (sd(s) - sd((s.0, s.1 + 1)));
                if best.map_or(true, |b| gpb > b.0) {
                    best = Some((gpb, Move::Resid { c, j: s.0, r: s.1 }, 1));
                }
            }
            let (_, m, k) = best.expect("no deterministic move fits");
            match m {
                Move::Tree { c, to, .. } => st[c] = (to, 0),
                Move::Resid { c, .. } => st[c].1 += 1,
            }
            used += k;
            mv.push(m);
        }
        moves.push(mv);
        states.push(st.clone());
    }
    (moves, states)
}

// ---------------- the randomised code ----------------

/// Greedy nested binary expansion of v (already divided by lambda rms) to `depth` bits; bit d set: t_d = +1.
#[inline]
pub fn rand_encode(v: f64, depth: usize) -> u8 {
    let mut c = 0.0;
    let mut p = 0u8;
    for d in 0..depth {
        let dl = 0.5f64.powi(d as i32 + 1);
        if v - c >= 0.0 {
            p |= 1 << d;
            c += dl;
        } else {
            c -= dl;
        }
    }
    p
}
#[inline]
pub fn rand_value(p: u8, depth: usize) -> f64 {
    (0..depth).map(|d| if p >> d & 1 == 1 { 0.5f64.powi(d as i32 + 1) } else { -0.5f64.powi(d as i32 + 1) }).sum()
}

/// Encode a block: positions to their final depth `maxd`, with the encoder scale (in units of the rms) chosen per block
/// from LAMS to minimise the sum over the block's rises of log tan^2 (any scale is sound: kappa is stored).
pub fn rand_choose(y: &[f64], maxd: &[usize], rises: &[&[u8]], out: &mut [u8], tmp: &mut [u8]) {
    let m = y.len();
    let yn2: f64 = y.iter().map(|v| v * v).sum();
    let rms = (yn2 / m as f64).sqrt().max(1e-300);
    let mut best = f64::INFINITY;
    for &lam in LAMS.iter() {
        for p in 0..m {
            tmp[p] = rand_encode(y[p] / (lam * rms), maxd[p]);
        }
        let mut obj = 0.0;
        for r in rises {
            let (mut dot, mut cn) = (0.0, 0.0);
            for p in 0..m {
                let v = rand_value(tmp[p], r[p] as usize);
                dot += v * y[p];
                cn += v * v;
            }
            obj += ((cn * yn2 - dot * dot).max(1e-300) / (dot * dot).max(1e-300)).ln();
        }
        if obj < best {
            best = obj;
            out[..m].copy_from_slice(&tmp[..m]);
        }
    }
}

/// Mean tan^2 of the randomised code at each integer rate (simulated on Gaussian vectors), for the rate design.
pub fn rand_tan2() -> [f64; 9] {
    use rand::SeedableRng;
    use rand_distr::{Distribution, StandardNormal};
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(11);
    let m = 128;
    let mut out = [0f64; 9];
    let nv = 2000;
    for _ in 0..nv {
        let y: Vec<f64> = (0..m).map(|_| StandardNormal.sample(&mut rng)).collect();
        let rms = (y.iter().map(|v| v * v).sum::<f64>() / m as f64).sqrt();
        let yn = rms * (m as f64).sqrt();
        let mut pats = Vec::new();
        for fin in 5..9 {
            // a block whose final rate is `fin` is encoded for its rises 1..=fin
            let maxd = vec![fin; m];
            let deps: Vec<Vec<u8>> = (1..=fin).map(|b| vec![b as u8; m]).collect();
            let rises: Vec<&[u8]> = deps.iter().map(|v| v.as_slice()).collect();
            let mut pat = vec![0u8; m];
            let mut tmp = vec![0u8; m];
            rand_choose(&y, &maxd, &rises, &mut pat, &mut tmp);
            pats.push(pat);
        }
        for b in 1..9 {
            let pat = &pats[b.max(5) - 5];
            let c: Vec<f64> = pat.iter().map(|&p| rand_value(p, b)).collect();
            let dot: f64 = c.iter().zip(&y).map(|(a, b)| a * b).sum();
            let cn = c.iter().map(|v| v * v).sum::<f64>().sqrt();
            let cs = dot / (cn * yn);
            out[b] += (1.0 - cs * cs) / (cs * cs) / nv as f64;
        }
    }
    out
}

// ---------------- plans ----------------

/// A field: a coordinate's (det) or a position's (rand) new bits at one level, packed at bit offset `off` of nibble
/// `unit`; `exp[k]` is the field's value for nibble value k.
#[derive(Clone, Copy, Debug)]
pub struct Field {
    pub widx: u16, // det: coordinate; rand: n + position
    pub nb: u8,
    pub unit: u16,
    pub off: u8,
    pub kind: FK,
    pub eid: u16, // the field's values by nibble value, in the shared ExpTab
}

/// Distinct expanded field codebooks: values by nibble value, and their extremes.
#[derive(Default)]
pub struct ExpTab {
    pub e: Vec<[f32; 16]>,
    pub lo: Vec<f32>,
    pub hi: Vec<f32>,
    map: HashMap<[u32; 16], u16>,
}
impl ExpTab {
    pub fn id(&mut self, v: [f32; 16], nb: u8) -> u16 {
        let key = v.map(|x| x.to_bits());
        if let Some(&i) = self.map.get(&key) {
            return i;
        }
        let i = self.e.len() as u16;
        self.e.push(v);
        self.lo.push(v.iter().cloned().fold(f32::INFINITY, f32::min));
        self.hi.push(v.iter().cloned().fold(f32::NEG_INFINITY, f32::max));
        let _ = nb;
        self.map.insert(key, i);
        i
    }
}
#[derive(Clone, Copy, Debug)]
pub enum FK {
    Tree { from: u8, to: u8 },
    Resid { j: u8, r: u8 },
    Rand { d: u8 },
}

/// One part of a level: the deterministic code (j = None) or randomised block j. Fields in encoding order; units list
/// the fields of each nibble.
#[derive(Clone, Debug)]
pub struct Part {
    pub j: Option<usize>,
    pub fields: Vec<Field>,
    pub ufields: Vec<[u16; 4]>, // field indices of each unit (u16::MAX: none)
}
impl Part {
    pub fn units(&self) -> usize {
        self.ufields.len()
    }
    /// Pack fields of 1, 2 and 4 bits exactly into nibbles.
    /// `vals[i]`: field i's values by field value (2^nb entries).
    pub fn pack(j: Option<usize>, mut fields: Vec<Field>, vals: Vec<[f32; 16]>, et: &mut ExpTab) -> Part {
        let total: usize = fields.iter().map(|f| f.nb as usize).sum();
        assert!(total % 16 == 0, "a part has {total} bits");
        let mut by: [Vec<usize>; 5] = Default::default();
        for (i, f) in fields.iter().enumerate() {
            by[f.nb as usize].push(i);
        }
        assert!(by[3].is_empty());
        let mut ufields: Vec<Vec<u16>> = Vec::new();
        for &i in &by[4] {
            ufields.push(vec![i as u16]);
        }
        let (twos, ones) = (&by[2], &by[1]);
        let mut oi = 0;
        let mut k = 0;
        while k + 1 < twos.len() {
            ufields.push(vec![twos[k] as u16, twos[k + 1] as u16]);
            k += 2;
        }
        if k < twos.len() {
            ufields.push(vec![twos[k] as u16, ones[oi] as u16, ones[oi + 1] as u16]);
            oi += 2;
        }
        while oi < ones.len() {
            ufields.push(ones[oi..oi + 4].iter().map(|&x| x as u16).collect());
            oi += 4;
        }
        for (u, fs) in ufields.iter().enumerate() {
            let mut off = 0u8;
            for &fi in fs {
                let f = &mut fields[fi as usize];
                f.unit = u as u16;
                f.off = off;
                off += f.nb;
            }
            assert_eq!(off, 4);
        }
        for (f, v) in fields.iter_mut().zip(&vals) {
            let mask = (1usize << f.nb) - 1;
            f.eid = et.id(core::array::from_fn(|k| v[(k >> f.off) & mask]), f.nb);
        }
        let ufields: Vec<[u16; 4]> = ufields.iter().map(|v| core::array::from_fn(|i| if i < v.len() { v[i] } else { u16::MAX })).collect();
        // part_sums adds the units' table bytes (at most 255 each) in 16 bits
        assert!(ufields.len() <= 257, "a part of {} units can overflow its 16-bit table sums; split it into parts of at most 257 units", ufields.len());
        Part { j, fields, ufields }
    }
}

pub struct LevelPlan {
    pub parts: Vec<Part>,
    pub units: usize,
    pub srows: usize, // scalar nibble rows
}

pub struct GroupPlan2 {
    pub sig: Vec<f32>,
    pub lamsum: f32,
    pub states: Vec<Vec<(u8, u8)>>, // per det level, per coordinate: (tree depth, residual stages)
    pub d: Vec<Vec<f32>>,
    pub pconst: [f32; NDL],
    pub kmin: [i32; NDL],
    pub ntab: [[f32; 16]; NDL],
    pub dt: Vec<f32>,
    pub ridx: Vec<u16>,
    pub nb: [usize; J],
    pub boff: [usize; J + 1],
    pub ypredw: [f32; J],
    pub ytab: Vec<[f32; 256]>,
    /// per randomised level: the bound's constant for this group's coded blocks there (`tail`)
    pub csgl: Vec<f32>,
    /// ytab[c] = yhl[1][c >> 4] * yhl[0][c & 15] in f32, so a code is decoded with two 16-entry permutes
    pub yhl: [[[f32; 16]; 2]; J],
    pub rates: Vec<[f64; J]>,
    pub thchg: Vec<Vec<usize>>,
    pub ttab: Vec<[[f32; 16]; J]>,
    pub ktab: Vec<[[f32; 256]; J]>,
    /// 8-bit kappa codes: ktab[c] = khl[1][c >> 4] * khl[0][c & 15] in f32 (4-bit codes read ktab[..16])
    pub khl: Vec<[[[f32; 16]; 2]; J]>,
    pub kstep: Vec<[f64; J]>, // kappa code steps per octave, per rise
    pub kbits: Vec<[u8; J]>,  // kappa code bits (4 or 8), per rise
    pub rrow: Vec<Vec<usize>>, // per rand level, per rising block (thchg order): first scalar row of that rise
    pub ystep: [f64; J],
    pub rdepth: Vec<Vec<u8>>,   // per rand level, per position: depth
    pub levels: Vec<LevelPlan>,
    pub e_inv: f64,
    pub v8t: Vec<i8>,  // row c = basis vector c (int8)
    pub vsc: Vec<f64>, // scale of basis vector c
    pub vinv: Vec<f64>, // V_q^-1 (build only)
    pub col: Option<col::ColPlan>, // the columnar store's plan (built with BUILD_COL)
    pub vq: Vec<f64>,   // V_q row-major n x n (build only)
}

fn rate_depth(r: f64, k: usize, nb: usize) -> u8 {
    if r < 1.0 {
        if k < derive::bits_of(r, nb, QUANTUM) { 1 } else { 0 }
    } else {
        r as u8
    }
}

/// Inverse of a row-major n x n matrix by Gauss-Jordan elimination with partial pivoting.
fn gauss_jordan_inverse(a: &[f64], n: usize) -> Vec<f64> {
    let w = 2 * n;
    let mut m = vec![0f64; n * w];
    for i in 0..n {
        m[i * w..i * w + n].copy_from_slice(&a[i * n..(i + 1) * n]);
        m[i * w + n + i] = 1.0;
    }
    for col in 0..n {
        let piv = (col..n).max_by(|&p, &q| m[p * w + col].abs().partial_cmp(&m[q * w + col].abs()).unwrap()).unwrap();
        assert!(m[piv * w + col].abs() > 1e-300, "the int8 basis is singular; refit the index");
        if piv != col {
            for k in 0..w {
                m.swap(piv * w + k, col * w + k);
            }
        }
        let inv = 1.0 / m[col * w + col];
        for k in 0..w {
            m[col * w + k] *= inv;
        }
        let prow: Vec<f64> = m[col * w..(col + 1) * w].to_vec();
        for r in 0..n {
            let f = m[r * w + col];
            if r != col && f != 0.0 {
                for (x, &p) in m[r * w..(r + 1) * w].iter_mut().zip(&prow) {
                    *x -= f * p;
                }
            }
        }
    }
    (0..n * n).map(|i| m[(i / n) * w + n + i % n]).collect()
}

/// int8 basis: one scale per basis vector (column), and its inverse by Newton-Schulz, started from the transpose when
/// the basis is near orthogonal and from a Gauss-Jordan inverse otherwise (the dual basis).
fn quant_basis(v: &[f32], n: usize) -> (Vec<i8>, Vec<f64>, Vec<f64>, Vec<f64>, f64) {
    let mut vsc = vec![0f64; n];
    let mut v8t = vec![0i8; n * n];
    for c in 0..n {
        let mx = (0..n).map(|i| (v[i * n + c] as f64).abs()).fold(0.0, f64::max);
        vsc[c] = mx / 127.0;
        for i in 0..n {
            v8t[c * n + i] = (v[i * n + c] as f64 / vsc[c]).round().clamp(-127.0, 127.0) as i8;
        }
    }
    let mut vq = vec![0f64; n * n];
    for i in 0..n {
        for c in 0..n {
            vq[i * n + c] = v8t[c * n + i] as f64 * vsc[c];
        }
    }
    // X = V_q^T, X <- X (2I - V_q X)
    let mut x = vec![0f64; n * n];
    for i in 0..n {
        for c in 0..n {
            x[c * n + i] = vq[i * n + c];
        }
    }
    let mm = |a: &[f64], b: &[f64], out: &mut [f64]| unsafe {
        matrixmultiply::dgemm(n, n, n, 1.0, a.as_ptr(), n as isize, 1, b.as_ptr(), n as isize, 1, 0.0, out.as_mut_ptr(), n as isize, 1);
    };
    let mut t = vec![0f64; n * n];
    let mut t2 = vec![0f64; n * n];
    let defect = |t: &[f64]| -> f64 {
        let mut s = 0.0;
        for i in 0..n {
            for k in 0..n {
                let e = t[i * n + k] - if i == k { 1.0 } else { 0.0 };
                s += e * e;
            }
        }
        s.sqrt()
    };
    mm(&vq, &x, &mut t);
    if defect(&t) > 0.5 {
        x = gauss_jordan_inverse(&vq, n);
    }
    for _ in 0..8 {
        mm(&vq, &x, &mut t);
        if defect(&t) < 1e-13 {
            break;
        }
        for i in 0..n {
            for k in 0..n {
                t[i * n + k] = if i == k { 2.0 } else { 0.0 } - t[i * n + k];
            }
        }
        mm(&x, &t, &mut t2);
        std::mem::swap(&mut x, &mut t2);
    }
    mm(&vq, &x, &mut t);
    let e_inv = defect(&t) * (1.0 + 1e-6) + 1e-12;
    (v8t, vsc, x, vq, e_inv)
}

impl GroupPlan2 {
    fn new_inner(p: &Params, prof: &[f64], g: usize, dt_: &DetTab, et: &mut ExpTab, tof: &dyn Fn(f64) -> f64) -> GroupPlan2 {
        let m = &p.meta;
        let n = m.n;
        let lam = &p.lam[g * n..(g + 1) * n];
        let pq = &prof[g * n..(g + 1) * n];
        let sig: Vec<f32> = lam.iter().map(|&l| l.sqrt() as f32).collect();
        // the item scale divides |r|^2 by its group mean, which is the sum of the coordinates' variances only in an
        // orthogonal basis
        let lamsum = p.r2.as_ref().map_or(lam.iter().sum::<f64>(), |r| r[g]) as f32;
        let (moves, states) = det_alloc_tree(lam, pq, dt_, &m.dlv);
        let gc = |l: usize, c: usize| -> f64 {
            let s = states[l][c];
            dt_.d[s.0 as usize][s.1 as usize]
        };
        let d: Vec<Vec<f32>> = (0..NDL).map(|l| (0..n).map(|c| (((lam[c] * gc(l, c) + 1e-10) / (pq[c] + 1e-10)).powf(0.25)) as f32).collect()).collect();
        let mut pconst = [0f32; NDL];
        for l in 0..NDL {
            let mut s = 0.0;
            for c in 0..n {
                let dd = d[l][c] as f64;
                s += lam[c] * gc(l, c) / (dd * dd);
            }
            pconst[l] = (s / lamsum as f64).sqrt() as f32;
        }
        // randomised design on the fitted final distortion
        let lres: Vec<f64> = (0..n).map(|c| lam[c] * gc(NDL - 1, c)).collect();
        let dt: Vec<f32> = (0..n).map(|c| (((lres[c] + 1e-10) / (pq[c] + 1e-10)).powf(0.25)) as f32).collect();
        let ai: Vec<f64> = (0..n).map(|c| (lres[c] * pq[c]).sqrt()).collect();
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| ai[b].partial_cmp(&ai[a]).unwrap());
        let ridx: Vec<u16> = order.iter().map(|&c| c as u16).collect();
        let mut nb = [0; J];
        let mut boff = [0; J + 1];
        for j in 0..J {
            boff[j] = m.bounds[j];
            nb[j] = m.bounds[j + 1] - m.bounds[j];
        }
        boff[J] = n;
        let sj: Vec<f64> = (0..J).map(|j| order[boff[j]..boff[j + 1]].iter().map(|&c| ai[c]).sum()).collect();
        let budgets: Vec<usize> = m.rlv.iter().map(|&b| 8 * (b - m.dlv[NDL - 1])).collect();
        let rv = match DESIGN_C.get() {
            Some(cs) => derive::design_by(&sj, &nb, &budgets, &|set| cs[set as usize], tof, QUANTUM),
            None => derive::design(&sj, &nb, &budgets, m.csg, tof, QUANTUM),
        };
        let rates: Vec<[f64; J]> = rv.iter().map(|r| core::array::from_fn(|j| r[j])).collect();
        let nr = rates.len();
        let mut ypredw = [0f32; J];
        for j in 0..J {
            let s: f64 = (boff[j]..boff[j + 1]).map(|pp| {
                let c = ridx[pp] as usize;
                lres[c] / (dt[c] as f64).powi(2)
            }).sum();
            ypredw[j] = (s.max(1e-30) / lamsum as f64).sqrt() as f32;
        }
        // levels: deterministic parts
        let mut levels = Vec::new();
        for l in 0..NDL {
            let fv: Vec<(Field, [f32; 16])> = moves[l].iter().map(|mv| match *mv {
                Move::Tree { c, from, to } => {
                    let mut exp = [0f32; 16];
                    for i in 0..1usize << to {
                        exp[i] = dt_.cent[to as usize][i] - dt_.cent[from as usize][i >> (to - from)];
                    }
                    (Field { widx: c as u16, nb: to, unit: 0, off: 0, kind: FK::Tree { from, to }, eid: 0 }, exp)
                }
                Move::Resid { c, j, r } => {
                    let dl = dt_.delta[j as usize][r as usize];
                    let mut exp = [0f32; 16];
                    exp[0] = -dl;
                    exp[1] = dl;
                    (Field { widx: c as u16, nb: 1, unit: 0, off: 0, kind: FK::Resid { j, r }, eid: 0 }, exp)
                }
            }).collect();
            let (fields, vals): (Vec<Field>, Vec<[f32; 16]>) = fv.into_iter().unzip();
            let part = Part::pack(None, fields, vals, et);
            let units = part.units();
            levels.push(LevelPlan { parts: vec![part], units, srows: 1 });
        }
        let mut thchg = vec![Vec::new(); nr];
        let mut rdepth = vec![vec![0u8; n]; nr];
        for lr in 0..nr {
            let mut parts = Vec::new();
            for j in 0..J {
                let ro = if lr == 0 { 0.0 } else { rates[lr - 1][j] };
                let rn = rates[lr][j];
                let mut fields = Vec::new();
                let mut vals = Vec::new();
                for k in 0..nb[j] {
                    let pos = boff[j] + k;
                    let bo = rate_depth(ro, k, nb[j]) as usize;
                    let bn = rate_depth(rn, k, nb[j]) as usize;
                    rdepth[lr][pos] = bn as u8;
                    for dd in bo..bn {
                        let dl = 0.5f32.powi(dd as i32 + 1);
                        let mut exp = [0f32; 16];
                        exp[0] = -dl;
                        exp[1] = dl;
                        fields.push(Field { widx: (n + pos) as u16, nb: 1, unit: 0, off: 0, kind: FK::Rand { d: dd as u8 }, eid: 0 });
                        vals.push(exp);
                    }
                }
                if rn > ro {
                    thchg[lr].push(j);
                    assert!(!fields.is_empty(), "rise of block {j} at rand level {lr} without bits");
                    parts.push(Part::pack(Some(j), fields, vals, et));
                }
            }
            let units = parts.iter().map(|p| p.units()).sum();
            levels.push(LevelPlan { parts, units, srows: 0 });
        }
        let (v8t, vsc, vinv, vq, e_inv) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), 0.0);
        let colp = if BUILD_COL.load(std::sync::atomic::Ordering::Relaxed) { Some(col::col_plan(n, lam, pq, &states[NDL - 1], &levels, &d[NDL - 1], et, dt_)) } else { None };
        let mut gp = GroupPlan2 {
            col: colp,
            sig,
            lamsum,
            states,
            d,
            pconst,
            kmin: [0; NDL],
            ntab: [[0.0; 16]; NDL],
            dt,
            ridx,
            nb,
            boff,
            ypredw,
            ytab: vec![[0.0; 256]; J],
            csgl: vec![p.meta.csg as f32; nr],
            yhl: [[[0.0; 16]; 2]; J],
            ystep: [256.0; J],
            rates,
            thchg,
            ttab: vec![[[0.0; 16]; J]; nr],
            ktab: vec![[[0.0; 256]; J]; nr],
            khl: vec![[[[0.0; 16]; 2]; J]; nr],
            kstep: vec![[256.0; J]; nr],
            kbits: vec![[8; J]; nr],
            rrow: Vec::new(),
            rdepth,
            levels,
            e_inv,
            v8t,
            vsc,
            vinv,
            vq,
        };
        gp.set_rows();
        gp
    }
    /// Scalar rows of each randomised level: |y| (2 rows per block) at the first, then per rise kappa (1 or 2 rows)
    /// and tan(phi) (1 row).
    pub fn set_rows(&mut self) {
        let nr = self.rates.len();
        self.rrow = vec![Vec::new(); nr];
        for lr in 0..nr {
            let mut r = if lr == 0 { 2 * J } else { 0 };
            for &j in &self.thchg[lr] {
                self.rrow[lr].push(r);
                r += self.kbits[lr][j] as usize / 4 + 1;
            }
            self.levels[NDL + lr].srows = r;
        }
    }
    /// Scalar code tables from calibration: deterministic norms (offset), |y| and kappa (geometric grids with an
    /// origin and a step per octave), tan(phi) (15 values, rounded up to).
    pub fn set_calibration(&mut self, kmin: [i32; NDL], y0: [f64; J], ys: [f64; J], tq: Vec<[[f32; 15]; J]>, k0: Vec<[f64; J]>, ks: Vec<[f64; J]>, kb: Vec<[u8; J]>) {
        self.kmin = kmin;
        for l in 0..NDL {
            for c in 0..15 {
                self.ntab[l][c] = 2f64.powf((OFF[c] + kmin[l]) as f64 / 16.0) as f32;
            }
        }
        for j in 0..J {
            self.ystep[j] = ys[j];
            for x in 0..16 {
                self.yhl[j][0][x] = 2f64.powf(x as f64 / ys[j]) as f32;
                self.yhl[j][1][x] = 2f64.powf(y0[j] + (16 * x) as f64 / ys[j]) as f32;
            }
            for c in 0..256 {
                self.ytab[j][c] = self.yhl[j][1][c >> 4] * self.yhl[j][0][c & 15];
            }
        }
        for lr in 0..tq.len() {
            for j in 0..J {
                for c in 0..15 {
                    self.ttab[lr][j][c] = tq[lr][j][c];
                }
                self.kstep[lr][j] = ks[lr][j];
                if kb[lr][j] == 8 {
                    for x in 0..16 {
                        self.khl[lr][j][0][x] = 2f64.powf(x as f64 / ks[lr][j]) as f32;
                        self.khl[lr][j][1][x] = 2f64.powf(k0[lr][j] + (16 * x) as f64 / ks[lr][j]) as f32;
                    }
                    for c in 0..256 {
                        self.ktab[lr][j][c] = self.khl[lr][j][1][c >> 4] * self.khl[lr][j][0][c & 15];
                    }
                } else {
                    for c in 0..256 {
                        self.ktab[lr][j][c] = 2f64.powf(k0[lr][j] + c as f64 / ks[lr][j]) as f32;
                    }
                }
            }
        }
        self.kbits = kb;
        self.set_rows();
    }
}

/// largest relative error of a kappa decoded from its nearest code on a grid of `steps` per octave, with f32 slack
pub fn eta_steps(steps: f64) -> f64 {
    2f64.powf(0.5 / steps) * (1.0 + 1e-6) - 1.0
}

// ---------------- the index ----------------

pub struct Meta2 {
    pub n: usize,
    pub dlv: Vec<usize>,
    pub rlv: Vec<usize>,
    pub csg: f64,
}
impl Meta2 {
    pub fn nl(&self) -> usize {
        NDL + self.rlv.len()
    }
    pub fn level_bytes(&self, l: usize) -> usize {
        if l < NDL { self.dlv[l] } else { self.rlv[l - NDL] }
    }
}

pub struct Cl2 {
    pub g: usize,
    pub r0: usize,
    pub r1: usize,
    pub nblk: usize,
    pub mn: f64,
    pub mn32: f32,
    pub mhat: Vec<f64>,
    pub mv32: Vec<f32>, // V_q^T m
    pub wmr: Vec<f32>,  // R_j (mv*dt) by position
    pub bm: [f64; NDL],
    pub b2: [f64; J],
    pub l1m: [f32; J + 1], // |wm| sums: det, then per block
    pub btot: f64,
    pub mvn: f64,
    pub rmax: f64,
    pub smax: f32,
    pub numax: f64,      // norm sort: the largest |z|, the grid of the stored codes (nu = numax code / 65535)
    pub bmin: f64,       // norm sort: bounds of m.r over the cluster
    pub bmax: f64,
}

/// Full vectors: `Real` maps the f16 file and holds a copy in index row order (filled by the build), so a scan of the
/// rows in order reads memory in order.
pub enum Full2 {
    Real { map: memmap2::Mmap, off: usize, rows: Vec<u16> },
    Synth(q::Synth2),
}

pub struct Index2 {
    pub meta: Meta2,
    pub groups: Vec<GroupPlan2>,
    pub codes: DetTab,
    pub etab: ExpTab,
    pub cl: Vec<Cl2>,
    pub centres64: Vec<f64>,
    pub rots: Vec<Vec<f64>>,
    pub anchors: Vec<f64>,
    pub hi: Vec<f32>,
    pub lo: Vec<f32>,
    pub nanch: usize,
    pub ahat: Vec<i16>,
    pub row_ent: Vec<u32>,
    pub ent_row: Vec<u32>,
    pub lcode: Vec<Vec<u8>>, // per level: code rows, clusters concatenated
    pub lscal: Vec<Vec<u8>>, // per level: scalar rows
    pub coff: Vec<Vec<usize>>, // per level per cluster: byte offset into lcode
    pub soff: Vec<Vec<usize>>,
    pub esc: HashMap<(u8, u8, u32), f32>, // (level, slot, row) -> value: det norm (slot 0), yh (1 + j), kappa (8 + j), tan (16 + j)
    pub x2: f64,
    pub full: Full2,
    pub nrows: usize,
    pub split: bool, // 16-bit tables (two shuffles) instead of 8-bit
    pub ctab: Vec<Vec<kern::CTab>>, // per cluster, per level: the centre's tables (query-independent)
    pub repack: Vec<f32>,
    pub col: Option<col::ColStore>,
    pub nukey: Vec<u16>, // norm sort: per row, |z| as a code on its cluster's grid, rounded up (empty without the sort)            // per level: occupancy of slots held in place below which they are compacted first
}

/// The format of the encoded index (codes, scalars, centre tables), which this build derives when it opens a fitted
/// directory.
pub const INDEX_FORMAT: u32 = 3;

/// Build the columnar store of the deterministic code as well (index::col).
pub static BUILD_COL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl Cl2 {
    /// The centre's weights, as the query side combines them: det by coordinate mv sig (f32), rand by position.
    pub fn wm(&self, gp: &GroupPlan2) -> Vec<f32> {
        let mut w: Vec<f32> = self.mv32.iter().zip(&gp.sig).map(|(&m, &s)| m * s).collect();
        w.extend_from_slice(&self.wmr);
        w
    }
}

impl Index2 {
    pub fn n(&self) -> usize {
        self.meta.n
    }
    pub fn k(&self) -> usize {
        self.cl.len()
    }
    /// cumulative stored bytes per item by level: code, scalars and the 2-byte key (with the norm sort, also the
    /// 2-byte norm code)
    pub fn stored_bytes_per_item(&self) -> Vec<f64> {
        let nl = self.meta.nl();
        let mut out = vec![0.0; nl];
        let mut tot = if self.nukey.is_empty() { 2.0 } else { 4.0 } * self.nrows as f64;
        for l in 0..nl {
            let mut b = 0.0;
            for c in &self.cl {
                let lp = &self.groups[c.g].levels[l];
                b += (c.r1 - c.r0) as f64 * (lp.units as f64 + lp.srows as f64) / 2.0;
            }
            tot += b;
            out[l] = tot / self.nrows as f64;
        }
        out
    }
    pub fn centres_f64(&self, ci: usize) -> &[f64] {
        let n = self.n();
        &self.centres64[ci * n..(ci + 1) * n]
    }
    pub fn full_row_f16(&self, row: usize) -> &[u16] {
        match &self.full {
            Full2::Real { rows, .. } => {
                let n = self.n();
                &rows[row * n..(row + 1) * n]
            }
            Full2::Synth(_) => panic!("synthetic rows have no stored f16 vector"),
        }
    }
    pub fn full_row_f64(&self, row: usize, out: &mut [f64]) {
        match &self.full {
            Full2::Real { .. } => {
                for (o, &h) in out.iter_mut().zip(self.full_row_f16(row)) {
                    *o = f16_to_f32(h) as f64;
                }
            }
            Full2::Synth(s) => s.x_f64(self, row, out),
        }
    }
}

/// Items of one cluster in sort order with z = V_q^-1 r.
pub struct Items2 {
    pub ah: Vec<i16>,
    pub z: Vec<f64>,
    pub ent: Vec<u32>,
    pub ext: Vec<u32>,
    pub x2max: f64, // largest |x|^2 - 1 (the bounds need |x| from above only)
    pub nu: Vec<f64>, // with the norm sort: |z|, the residual's norm in the metric, rounded up
}

impl Items2 {
    /// The items in ascending order of |z| (z = V_q^-1 r, the residual in the metric of the dual pair), with nu set.
    pub fn sort_by_nu(self, n: usize) -> Items2 {
        let nc = self.ah.len();
        let nu: Vec<f64> = (0..nc).map(|i| self.z[i * n..(i + 1) * n].iter().map(|v| v * v).sum::<f64>().sqrt() * (1.0 + 1e-9) + 1e-12).collect();
        let mut o: Vec<usize> = (0..nc).collect();
        o.sort_by(|&a, &b| nu[a].partial_cmp(&nu[b]).unwrap());
        let mut z = vec![0f64; nc * n];
        for (i, &k) in o.iter().enumerate() {
            z[i * n..(i + 1) * n].copy_from_slice(&self.z[k * n..(k + 1) * n]);
        }
        Items2 { ah: o.iter().map(|&k| self.ah[k]).collect(), z, ent: o.iter().map(|&k| self.ent[k]).collect(), ext: o.iter().map(|&k| self.ext[k]).collect(), x2max: self.x2max, nu: o.iter().map(|&k| nu[k]).collect() }
    }
}

pub trait Source2: Sync {
    fn items(&self, c: usize, p: &Params, gp: &GroupPlan2, mhat: &[f64]) -> Items2;
}

pub struct Real2 {
    pub src: crate::index::RealSource,
}
impl Source2 for Real2 {
    fn items(&self, c: usize, p: &Params, gp: &GroupPlan2, mhat: &[f64]) -> Items2 {
        let n = self.src.n;
        let (r0, r1) = (p.cstart[c], p.cstart[c + 1]);
        let m = &p.centres[c * n..(c + 1) * n];
        let mut key: Vec<(i16, usize)> = (r0..r1).map(|r| {
            let x = self.src.row(r);
            let a: f64 = x.iter().zip(mhat).map(|(&h, &mh)| f16_to_f32(h) as f64 * mh).sum();
            ((a.clamp(-1.0, 1.0) * 32767.0).round() as i16, r)
        }).collect();
        key.sort();
        let nc = r1 - r0;
        let mut rmat = vec![0f64; nc * n];
        let mut x2max = 0f64;
        for (i, &(_, r)) in key.iter().enumerate() {
            let x = self.src.row(r);
            let mut nx = 0.0;
            for k in 0..n {
                let xv = f16_to_f32(x[k]) as f64;
                nx += xv * xv;
                rmat[i * n + k] = xv - m[k] as f64;
            }
            x2max = x2max.max(nx - 1.0);
        }
        // z^T = r^T M^T: z_i = M r_i
        let mut z = vec![0f64; nc * n];
        unsafe {
            matrixmultiply::dgemm(nc, n, n, 1.0, rmat.as_ptr(), n as isize, 1, gp.vinv.as_ptr(), 1, n as isize, 0.0, z.as_mut_ptr(), n as isize, 1);
        }
        Items2 { ah: key.iter().map(|k| k.0).collect(), z, ent: key.iter().map(|k| p.perm[k.1] as u32).collect(), ext: key.iter().map(|k| k.1 as u32).collect(), x2max, nu: Vec::new() }
    }
}

struct Calib {
    kpool: Vec<Vec<f64>>,
    ypool: Vec<Vec<f64>>,
    tpool: Vec<Vec<Vec<f64>>>,
    kapool: Vec<Vec<Vec<f64>>>,
}

pub struct ClusterCode2 {
    ccode: Vec<u8>,       // columnar rows (empty without the columnar store)
    cnrm: Vec<f64>,       // per item: the single norm, then each block's
    codes: Vec<Vec<u8>>,
    scal: Vec<Vec<u8>>,
    esc: Vec<(u8, u8, u32, f32)>, // (level, slot, local row, value)
    items: Items2,
    calib: Option<Calib>,
}

#[inline]
fn setnib(buf: &mut [u8], nrows: usize, i: usize, row: usize, val: u8) {
    let blk = i / BLK;
    let lane = i % BLK;
    let byte = blk * nrows * 16 + row * 16 + (lane % 16);
    if lane < 16 {
        buf[byte] |= val & 15;
    } else {
        buf[byte] |= (val & 15) << 4;
    }
}

fn roundup_abs(v: f64, steps: f64, floor_log: f64) -> f64 {
    let lv = v.max(2f64.powf(floor_log)).log2();
    let mut r = 2f64.powf((lv * steps).ceil() / steps);
    while r < v {
        r *= 2f64.powf(1.0 / steps);
    }
    r
}

fn encode_cluster(it: Items2, gp: &GroupPlan2, codes: &DetTab, et: &ExpTab, meta: &Meta2, rots: &[Vec<f64>], mn32: f32, calib: bool) -> ClusterCode2 {
    let n = meta.n;
    let nc = it.ah.len();
    let nblk = (nc + BLK - 1) / BLK;
    let nl = meta.nl();
    let nr = meta.rlv.len();
    let mut lc: Vec<Vec<u8>> = (0..nl).map(|l| vec![0u8; nblk * gp.levels[l].units * 16]).collect();
    let mut ls: Vec<Vec<u8>> = (0..nl).map(|l| vec![0u8; nblk * gp.levels[l].srows * 16]).collect();
    let mut esc = Vec::new();
    let mut cal = if calib {
        Some(Calib { kpool: vec![Vec::new(); NDL], ypool: vec![Vec::new(); J], tpool: vec![vec![Vec::new(); J]; nr], kapool: vec![vec![Vec::new(); J]; nr] })
    } else {
        None
    };
    // which (coordinate or position, stage) bit sits at which (level, unit, bit)
    let svec: Vec<f32> = it.ah.iter().map(|&a| item_scale(a, mn32, gp.lamsum)).collect();
    let mut wmat = vec![0f64; nc * n];
    let mut val = vec![0f64; n];
    let cp = if calib { None } else { gp.col.as_ref() };
    let ncr = cp.map_or(0, |p| p.nrows());
    let ncn = cp.map_or(0, |p| p.nb + 1);
    let mut ccode = vec![0u8; ncr * nblk * 16];
    let mut cnrm = vec![0f64; nc * ncn];
    let (mut tid, mut sb, mut sc) = (vec![0u8; n], vec![0u8; n], vec![0u8; n]);
    for i in 0..nc {
        let z = &it.z[i * n..(i + 1) * n];
        let s = svec[i] as f64;
        val.iter_mut().for_each(|v| *v = 0.0);
        if cp.is_some() {
            sb.iter_mut().for_each(|v| *v = 0);
            sc.iter_mut().for_each(|v| *v = 0);
        }
        for l in 0..NDL {
            let lp = &gp.levels[l];
            for f in &lp.parts[0].fields {
                let c = f.widx as usize;
                let u = z[c] / (gp.sig[c] as f64 * s);
                let code = match f.kind {
                    FK::Tree { to, .. } => codes.index(to as usize, u),
                    _ => (u - val[c] >= 0.0) as usize,
                };
                if cp.is_some() {
                    match f.kind {
                        FK::Tree { .. } => tid[c] = code as u8,
                        _ => {
                            sb[c] |= (code as u8) << sc[c];
                            sc[c] += 1;
                        }
                    }
                }
                val[c] += et.e[f.eid as usize][code << f.off] as f64;
                setnib(&mut lc[l], lp.units, i, f.unit as usize, (code << f.off) as u8);
            }
            let mut rho2 = 0.0;
            for c in 0..n {
                let e = z[c] - s * gp.sig[c] as f64 * val[c];
                let q = e / gp.d[l][c] as f64;
                rho2 += q * q;
            }
            let rho = rho2.sqrt();
            let need = rho * (1.0 + 1e-6) + 1e-12;
            let pred = gp.pconst[l] * svec[i];
            let kraw = ((rho.max(1e-30) / pred as f64).log2() * 16.0).ceil();
            if let Some(cal) = cal.as_mut() {
                cal.kpool[l].push(kraw);
                continue;
            }
            let k = (kraw.max(-1e6) as i64 - gp.kmin[l] as i64) as i32;
            let mut code = 15usize;
            if k <= OFF[14] {
                let mut cc = OFF.iter().position(|&o| o >= k).unwrap();
                while cc < 15 && ((pred * gp.ntab[l][cc]) as f64) < need {
                    cc += 1;
                }
                code = cc;
            }
            if code == 15 {
                let mut v = roundup_abs(need, 16.0, -60.0) as f32;
                while (v as f64) < need {
                    v *= 1.0001;
                }
                esc.push((l as u8, 0, i as u32, v));
            }
            setnib(&mut ls[l], 1, i, 0, code as u8);
        }
        for pp in 0..n {
            let k = gp.ridx[pp] as usize;
            let e = z[k] - s * gp.sig[k] as f64 * val[k];
            wmat[i * n + pp] = e / gp.dt[k] as f64;
        }
        if let Some(cp) = cp {
            let mut nb2 = [0f64; col::MAXB];
            for (k, cc) in cp.coords.iter().enumerate() {
                let c = cc.c as usize;
                let v = cp.value(k, tid[c], sb[c]);
                assert!((v - val[c]).abs() <= 1e-12 * (1.0 + val[c].abs()), "the columnar code of coordinate {c} decodes to {v}, the stream's to {}", val[c]);
                col::setnib_col(&mut ccode, nblk, i, cc.tree.0 as usize, tid[c] << cc.tree.1);
                for (q, &(row, off)) in cc.stages.iter().enumerate() {
                    col::setnib_col(&mut ccode, nblk, i, row as usize, ((sb[c] >> q) & 1) << off);
                }
                let u = z[c] / (gp.sig[c] as f64 * s) / cc.h;
                nb2[cc.blk as usize] += u * u;
            }
            let tot: f64 = nb2.iter().sum();
            cnrm[i * ncn] = tot.sqrt() * (1.0 + 1e-9);
            for b in 0..cp.nb {
                cnrm[i * ncn + 1 + b] = nb2[b].sqrt() * (1.0 + 1e-9);
            }
        }
    }
    let mut ymat = vec![0f64; nc * n];
    for j in 0..J {
        let nbj = gp.nb[j];
        let o = gp.boff[j];
        unsafe {
            matrixmultiply::dgemm(nc, nbj, nbj, 1.0, wmat.as_ptr().add(o), n as isize, 1, rots[j].as_ptr(), 1, nbj as isize, 0.0, ymat.as_mut_ptr().add(o), n as isize, 1);
        }
    }
    drop(wmat);
    let maxd: Vec<usize> = (0..n).map(|pp| gp.rdepth.iter().map(|r| r[pp] as usize).max().unwrap_or(0)).collect();
    let mut rp = vec![0u8; n];
    let mut rtmp = vec![0u8; n];
    for i in 0..nc {
        let s = svec[i];
        let y = &ymat[i * n..(i + 1) * n];
        let mut yn = [0f64; J];
        let mut yhv = [0f64; J];
        for j in 0..J {
            yn[j] = y[gp.boff[j]..gp.boff[j + 1]].iter().map(|v| v * v).sum::<f64>().sqrt();
            let (b0, b1) = (gp.boff[j], gp.boff[j + 1]);
            let rises: Vec<&[u8]> = gp.thchg.iter().enumerate().filter(|(_, ch)| ch.contains(&j)).map(|(lr, _)| &gp.rdepth[lr][b0..b1]).collect();
            rand_choose(&y[b0..b1], &maxd[b0..b1], &rises, &mut rp[b0..b1], &mut rtmp);
            let pr = gp.ypredw[j] * s;
            let need = yn[j] * (1.0 + 1e-7);
            if let Some(cal) = cal.as_mut() {
                cal.ypool[j].push((yn[j].max(1e-30) / pr as f64).log2());
                yhv[j] = roundup_abs(need, 4096.0, -60.0) as f32 as f64;
                continue;
            }
            // smallest grid value at or above |y| (values below the grid take its first value)
            let mut code = (((yn[j].max(1e-30) / pr as f64).log2() - (gp.ytab[j][0] as f64).log2()) * gp.ystep[j]).ceil().max(0.0) as i64;
            while code <= 254 && ((pr * gp.ytab[j][code as usize]) as f64) < need {
                code += 1;
            }
            if code <= 254 {
                setnib(&mut ls[NDL], gp.levels[NDL].srows, i, 2 * j, (code & 15) as u8);
                setnib(&mut ls[NDL], gp.levels[NDL].srows, i, 2 * j + 1, (code >> 4) as u8);
                yhv[j] = (pr * gp.ytab[j][code as usize]) as f64;
            } else {
                let mut v = roundup_abs(need + 1e-30, 4096.0, -60.0) as f32;
                while (v as f64) < need {
                    v *= 1.0 + 2e-7;
                }
                setnib(&mut ls[NDL], gp.levels[NDL].srows, i, 2 * j, 15);
                setnib(&mut ls[NDL], gp.levels[NDL].srows, i, 2 * j + 1, 15);
                esc.push((NDL as u8, 1 + j as u8, i as u32, v));
                yhv[j] = v as f64;
            }
        }
        for lr in 0..nr {
            let l = NDL + lr;
            let lp = &gp.levels[l];
            let mut ubase = 0;
            for part in &lp.parts {
                for f in &part.fields {
                    let pp = f.widx as usize - n;
                    if let FK::Rand { d } = f.kind {
                        if rp[pp] >> d & 1 == 1 {
                            setnib(&mut lc[l], lp.units, i, ubase + f.unit as usize, 1 << f.off);
                        }
                    }
                }
                ubase += part.units();
            }
            for (slot, &j) in gp.thchg[lr].iter().enumerate() {
                let (mut dot, mut cn) = (0.0, 0.0);
                for pp in gp.boff[j]..gp.boff[j + 1] {
                    let v = rand_value(rp[pp], gp.rdepth[lr][pp] as usize);
                    dot += v * y[pp];
                    cn += v * v;
                }
                let cosf = (dot / (cn.sqrt() * yn[j] + 1e-300)).clamp(1e-9, 1.0);
                let tanf = (1.0 - cosf * cosf).max(0.0).sqrt() / cosf;
                let kappa = yn[j] * yn[j] / dot.max(1e-300);
                let yd32 = yhv[j] as f32;
                if let Some(cal) = cal.as_mut() {
                    cal.tpool[lr][j].push(tanf);
                    cal.kapool[lr][j].push((kappa / yd32 as f64).log2());
                    continue;
                }
                let row = gp.rrow[lr][slot];
                let kb = gp.kbits[lr][j] as usize;
                let kmaxc: i64 = if kb == 4 { 14 } else { 254 };
                let tneed = tanf * (1.0 + 1e-7);
                let tc = match (0..15).find(|&c| gp.ttab[lr][j][c] as f64 >= tneed) {
                    Some(c) => c as u8,
                    None => {
                        let mut v = roundup_abs(tneed, 20.0, -40.0) as f32;
                        while (v as f64) < tneed {
                            v *= 1.0 + 2e-7;
                        }
                        esc.push((l as u8, 16 + j as u8, i as u32, v));
                        15
                    }
                };
                setnib(&mut ls[l], lp.srows, i, row + kb / 4, tc);
                // kappa, nearest on the grid; decoded as yh_decoded * ktab[code] in f32
                let ks = gp.kstep[lr][j];
                let eta = eta_steps(ks);
                let mut kc = kmaxc as u8 + 1;
                let base = (((kappa / yd32 as f64).log2() - (gp.ktab[lr][j][0] as f64).log2()) * ks).round() as i64;
                for cand in [base, base - 1, base + 1] {
                    if (0..=kmaxc).contains(&cand) {
                        let v = (yd32 * gp.ktab[lr][j][cand as usize]) as f64;
                        if (v / kappa - 1.0).abs() <= eta * 0.999 {
                            kc = cand as u8;
                            break;
                        }
                    }
                }
                if kc as i64 == kmaxc + 1 {
                    esc.push((l as u8, 8 + j as u8, i as u32, kappa as f32));
                }
                setnib(&mut ls[l], lp.srows, i, row, kc & 15);
                if kb == 8 {
                    setnib(&mut ls[l], lp.srows, i, row + 1, kc >> 4);
                }
            }
        }
    }
    let mut it = it;
    it.z = Vec::new();
    ClusterCode2 { ccode, cnrm, codes: lc, scal: ls, esc, items: it, calib: cal }
}

/// Query-independent per-cluster data.
fn cluster_info(gp: &GroupPlan2, m: &[f32], rots: &[Vec<f64>], it: &Items2, x2: f64, n: usize) -> Cl2 {
    let mn = m.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt();
    assert!(mn > 0.0, "a cluster centre is zero (its items cancel out or are all zero); refit with more clusters or drop the zero vectors");
    let mhat: Vec<f64> = m.iter().map(|&x| x as f64 / mn).collect();
    // mv = V_q^T m
    let mut mv = vec![0f64; n];
    for c in 0..n {
        let row = &gp.v8t[c * n..(c + 1) * n];
        mv[c] = gp.vsc[c] * row.iter().zip(m).map(|(&v, &x)| v as f64 * x as f64).sum::<f64>();
    }
    let mut wm = vec![0f32; 2 * n];
    for c in 0..n {
        wm[c] = (mv[c] * gp.sig[c] as f64) as f32;
    }
    let mvt: Vec<f64> = (0..n).map(|c| mv[c] * gp.dt[c] as f64).collect();
    let mvt_pos: Vec<f64> = (0..n).map(|pp| mvt[gp.ridx[pp] as usize]).collect();
    let mut b2 = [0f64; J];
    for j in 0..J {
        let nbj = gp.nb[j];
        for a in 0..nbj {
            let s: f64 = (0..nbj).map(|b| rots[j][a * nbj + b] * mvt_pos[gp.boff[j] + b]).sum();
            wm[n + gp.boff[j] + a] = s as f32;
        }
        b2[j] = (gp.boff[j]..gp.boff[j + 1]).map(|pp| mvt_pos[pp].powi(2)).sum();
    }
    let mut bm = [0f64; NDL];
    for l in 0..NDL {
        for c in 0..n {
            let dd = gp.d[l][c] as f64;
            bm[l] += mv[c] * mv[c] * dd * dd;
        }
    }
    let mn32 = mn as f32;
    let smax = it.ah.iter().map(|&a| item_scale(a, mn32, gp.lamsum)).fold(0f32, f32::max);
    let rmax = it.ah.iter().map(|&a| {
        let al = a as f64 / 32767.0 - DA;
        (x2 + mn * mn - 2.0 * mn * al).max(0.0).sqrt()
    }).fold(0f64, f64::max);
    let nc = it.ah.len();
    let mut l1m = [0f32; J + 1];
    l1m[0] = (wm[..n].iter().map(|x| x.abs() as f64).sum::<f64>() * 1.001) as f32;
    for j in 0..J {
        l1m[1 + j] = (wm[n + gp.boff[j]..n + gp.boff[j + 1]].iter().map(|x| x.abs() as f64).sum::<f64>() * 1.001) as f32;
    }
    let mv32: Vec<f32> = mv.iter().map(|&x| x as f32).collect();
    let wmr = wm[n..].to_vec();
    Cl2 { g: 0, r0: 0, r1: nc, nblk: (nc + BLK - 1) / BLK, mn, mn32, mhat, mv32, wmr, bm, b2, l1m, btot: b2.iter().sum(), mvn: mv.iter().map(|x| x * x).sum::<f64>().sqrt(), rmax, smax, numax: 0.0, bmin: 0.0, bmax: 0.0 }
}

pub fn calib_quantile(v: &mut Vec<f64>, q: f64) -> f64 {
    quantile(v, q)
}

/// Build the fast-scan index. `prof` is the G x n profile.
/// With `nusort`, each cluster is sorted by the residual's norm in the metric instead of by x.m^ (see Items2::sort_by_nu).
pub fn build2(p: &Params, prof: &[f64], src: &dyn Source2, full: Full2, nanch: usize, split: bool, nusort: bool, log: &mut dyn FnMut(String)) -> Index2 {
    let t = Timer::new();
    let n = p.meta.n;
    let k = p.meta.k;
    let meta = Meta2 { n, dlv: p.meta.dlv.clone(), rlv: p.meta.rlv.clone(), csg: p.meta.csg };
    let rots: Vec<Vec<f64>> = (0..J).map(|j| haar_rotation(p.meta.bounds[j + 1] - p.meta.bounds[j], SEED, j as u64)).collect();
    let t2 = rand_tan2();
    log(format!("randomised code tan^2 by rate {:?}", t2.iter().map(|x| (x * 1e4).round() / 1e4).collect::<Vec<_>>()));
    let tof = move |r: f64| -> f64 {
        if r == 0.0 { f64::INFINITY } else if r < 1.0 { std::f64::consts::PI / (2.0 * r) - 1.0 } else { t2[r as usize] }
    };
    let codes = DetTab::new(p);
    let mut et = ExpTab::default();
    let mut groups: Vec<GroupPlan2> = Vec::new();
    for g in 0..p.meta.g {
        groups.push(GroupPlan2::new_light(p, prof, g, &codes, &mut et, &tof));
    }
    groups.par_iter_mut().enumerate().for_each(|(g, gp)| {
        let (v8t, vsc, vinv, vq, e) = quant_basis(&p.v[g * n * n..(g + 1) * n * n], n);
        gp.v8t = v8t;
        gp.vsc = vsc;
        gp.vinv = vinv;
        gp.vq = vq;
        gp.e_inv = e;
    });
    {
        // the randomised levels' constants, one per distinct set of coded blocks
        let tc = Timer::new();
        let set = |gp: &GroupPlan2, lr: usize| -> Vec<usize> { (0..J).filter(|&j| gp.rates[lr][j] > 0.0).map(|j| gp.nb[j] - 1).collect() };
        let mut sets: Vec<Vec<usize>> = groups.iter().flat_map(|gp| (0..gp.rates.len()).map(move |lr| set(gp, lr))).collect();
        sets.sort();
        sets.dedup();
        let plain = CSG_PLAIN.load(std::sync::atomic::Ordering::Relaxed);
        // one event per (item, level), on the side that would make the item's decision wrong: delta = eps / (N L)
        let l1 = p.meta.csg * p.meta.csg / 2.0 - 2f64.ln();
        let vals: HashMap<Vec<usize>, (f64, tail::Source)> = sets.par_iter().map(|s| (s.clone(), if plain { ((2.0 * l1).sqrt(), tail::Source::SubGaussian) } else { tail::level_constant(s, l1, &p.meta.certs) })).collect();
        for gp in groups.iter_mut() {
            for lr in 0..gp.rates.len() {
                gp.csgl[lr] = vals[&set(gp, lr)].0 as f32;
            }
        }
        log(format!("randomised constants {:.1}s (sub-Gaussian c = {:.3}): {:?}", tc.s(), p.meta.csg, sets.iter().map(|s| (s.clone(), (vals[s].0 * 1000.0).round() / 1000.0, vals[s].1)).collect::<Vec<_>>()));
    }
    log(format!("plans {:.1}s; residual-stage distortion / tree {:?}; largest inverse defect {:.2e}; rates g0 {:?}", t.s(), TREE_DEPTHS.iter().map(|&j| (1..=MAX_STAGES).map(|r| (codes.d[j][r] / p.gtree[j + r] * 1000.0).round() / 1000.0).collect::<Vec<_>>()).collect::<Vec<_>>(), groups.iter().map(|g| g.e_inv).fold(0.0, f64::max), groups[0].rates));
    let mhats: Vec<Vec<f64>> = (0..k).map(|c| {
        let m = &p.centres[c * n..(c + 1) * n];
        let mn = m.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt();
        m.iter().map(|&x| x as f64 / mn).collect()
    }).collect();
    let mn32: Vec<f32> = (0..k).map(|c| p.centres[c * n..(c + 1) * n].iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt() as f32).collect();
    let nr = meta.rlv.len();
    // calibration on every 8th cluster of each group
    let mut calc = Vec::new();
    for g in 0..p.meta.g {
        let ks: Vec<usize> = (0..k).filter(|&c| p.grp[c] == g).collect();
        let per: usize = std::env::var("VEXACT_CAL_PER_GROUP").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
        calc.extend(ks.iter().step_by((ks.len() / per).max(1)));
    }
    let cals: Vec<(usize, Calib)> = calc.par_iter().map(|&c| {
        let g = p.grp[c];
        let it = src.items(c, p, &groups[g], &mhats[c]);
        (g, encode_cluster(it, &groups[g], &codes, &et, &meta, &rots, mn32[c], true).calib.unwrap())
    }).collect();
    for g in 0..p.meta.g {
        let mine: Vec<&Calib> = cals.iter().filter(|x| x.0 == g).map(|x| &x.1).collect();
        if mine.is_empty() {
            continue;
        }
        let pool = |f: &dyn Fn(&Calib) -> &Vec<f64>| -> Vec<f64> { mine.iter().flat_map(|c| f(c).iter().copied()).collect() };
        let kmin: [i32; NDL] = core::array::from_fn(|l| quantile(&mut pool(&|c| &c.kpool[l]), 0.003).floor() as i32);
        // a geometric grid covering [q_lo, q_hi] of log2 values with at most 254 steps, at most `smax` per octave
        let grid = |v: &mut Vec<f64>, smax: f64| -> (f64, f64) {
            if v.is_empty() {
                return (0.0, smax);
            }
            let lo = quantile(v, 0.001);
            let hi = quantile(v, 0.999);
            let st = (253.0 / (hi - lo).max(1e-9)).min(smax).floor().max(1.0);
            (lo - 0.5 / st, st)
        };
        let mut y0 = [0f64; J];
        let mut ys = [256f64; J];
        for j in 0..J {
            let (a, b) = grid(&mut pool(&|c| &c.ypool[j]), 256.0);
            y0[j] = a;
            ys[j] = b;
        }
        let mut tq = vec![[[1e30f32; 15]; J]; nr];
        let mut k0 = vec![[0f64; J]; nr];
        let mut ksv = vec![[256f64; J]; nr];
        let mut kbv = vec![[8u8; J]; nr];
        for lr in 0..nr {
            for j in 0..J {
                let mut v = pool(&|c| &c.tpool[lr][j]);
                if !v.is_empty() {
                    for cc in 0..15 {
                        tq[lr][j][cc] = (quantile(&mut v, 0.002 + 0.996 * cc as f64 / 14.0) * (1.0 + 1e-6)) as f32;
                    }
                }
                let (a, b) = grid(&mut pool(&|c| &c.kapool[lr][j]), 256.0);
                k0[lr][j] = a;
                ksv[lr][j] = b;
                // 4 bits when 14 steps over the spread round kappa to within 5% of a low quantile of tan(phi)
                let mut kv = pool(&|c| &c.kapool[lr][j]);
                if !v.is_empty() && !kv.is_empty() {
                    let lo = quantile(&mut kv, 0.001);
                    let hi = quantile(&mut kv, 0.999);
                    let s4 = 13.0 / (hi - lo).max(1e-9);
                    if eta_steps(s4) <= 0.05 * quantile(&mut v, 0.05) {
                        kbv[lr][j] = 4;
                        k0[lr][j] = lo - 0.5 / s4;
                        ksv[lr][j] = s4;
                    }
                }
            }
        }
        groups[g].set_calibration(kmin, y0, ys, tq, k0, ksv, kbv);
    }
    log(format!("calibrated {:.1}s", t.s()));
    let ccodes: Vec<ClusterCode2> = (0..k).into_par_iter().map(|c| {
        let g = p.grp[c];
        let it = src.items(c, p, &groups[g], &mhats[c]);
        let it = if nusort { it.sort_by_nu(n) } else { it };
        encode_cluster(it, &groups[g], &codes, &et, &meta, &rots, mn32[c], false)
    }).collect();
    log(format!("encoded {:.1}s", t.s()));
    let x2 = 1.0 + ccodes.iter().map(|c| c.items.x2max).fold(0.0, f64::max) * (1.0 + 1e-9) + 1e-12;
    let infos: Vec<Cl2> = ccodes.par_iter().enumerate().map(|(c, cc)| {
        let mut info = cluster_info(&groups[p.grp[c]], &p.centres[c * n..(c + 1) * n], &rots, &cc.items, x2, n);
        info.g = p.grp[c];
        if nusort {
            // m.r = mn a - mn^2 from the key, within mn DA
            let it = &cc.items;
            info.numax = (it.nu.iter().cloned().fold(0.0, f64::max) * (1.0 + 1e-12)).max(1e-30);
            let mr = |a: i16| info.mn * (a as f64 / 32767.0) - info.mn * info.mn;
            info.bmax = it.ah.iter().map(|&a| mr(a)).fold(f64::NEG_INFINITY, f64::max) + info.mn * DA + 1e-12;
            info.bmin = it.ah.iter().map(|&a| mr(a)).fold(f64::INFINITY, f64::min) - info.mn * DA - 1e-12;
        }
        info
    }).collect();
    let nl = meta.nl();
    // the columnar store's norm grids per group, from every item of the group
    let build_col = groups[0].col.is_some();
    if build_col {
        for g in 0..groups.len() {
            let nn = groups[g].col.as_ref().unwrap().nb + 1;
            let mut vals = vec![Vec::new(); nn];
            for (c, cc) in ccodes.iter().enumerate() {
                if p.grp[c] != g {
                    continue;
                }
                for (i, x) in cc.cnrm.iter().enumerate() {
                    vals[i % nn].push(*x);
                }
            }
            groups[g].col.as_mut().unwrap().set_grids(&vals);
        }
    }
    if build_col {
        let cps: Vec<&col::ColPlan> = groups.iter().map(|g| g.col.as_ref().unwrap()).collect();
        let nd: Vec<usize> = cps.iter().map(|c| c.coords.len()).collect();
        let nr: Vec<usize> = cps.iter().map(|c| c.rows.len()).collect();
        let nu: Vec<usize> = cps.iter().map(|c| c.units.len()).collect();
        log(format!("columnar store: units per group mean {:.1}", nu.iter().sum::<usize>() as f64 / nu.len() as f64));
        let mut hist = [0usize; 9];
        for c in &cps {
            for cc in &c.coords {
                hist[(cc.j + cc.r) as usize] += 1;
            }
        }
        log(format!("columnar store: coded coordinates per group {}..{} (mean {:.0}), rows {}..{} (mean {:.0}, {:.1} B); bits per coordinate (count over groups, 0..8) {:?}; blocks {:?} of group 0",
            nd.iter().min().unwrap(), nd.iter().max().unwrap(), nd.iter().sum::<usize>() as f64 / nd.len() as f64, nr.iter().min().unwrap(), nr.iter().max().unwrap(), nr.iter().sum::<usize>() as f64 / nr.len() as f64, nr.iter().sum::<usize>() as f64 / nr.len() as f64 / 2.0, hist, cps[0].bstart));
    }
    let mut cst = col::ColStore::default();
    let mut row0 = 0;
    let mut cl = Vec::with_capacity(k);
    let (mut ahat, mut row_ent, mut xs_row, mut nukey) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut lcode: Vec<Vec<u8>> = vec![Vec::new(); nl];
    let mut lscal: Vec<Vec<u8>> = vec![Vec::new(); nl];
    let mut coff: Vec<Vec<usize>> = vec![Vec::new(); nl];
    let mut soff: Vec<Vec<usize>> = vec![Vec::new(); nl];
    let mut esc = HashMap::new();
    for (cc, mut info) in ccodes.into_iter().zip(infos) {
        let nc = cc.items.ah.len();
        info.r0 = row0;
        info.r1 = row0 + nc;
        for l in 0..nl {
            coff[l].push(lcode[l].len());
            lcode[l].extend_from_slice(&cc.codes[l]);
            soff[l].push(lscal[l].len());
            lscal[l].extend_from_slice(&cc.scal[l]);
        }
        for &(l, slot, i, v) in &cc.esc {
            esc.insert((l, slot, (row0 + i as usize) as u32), v);
        }
        if build_col {
            let cp = groups[info.g].col.as_ref().unwrap();
            let nn = cp.nb + 1;
            let nblk = (nc + BLK - 1) / BLK;
            cst.coff.push(cst.code.len());
            cst.code.extend_from_slice(&cc.ccode);
            cst.noff.push(cst.nrm.len());
            let base = cst.nrm.len();
            cst.nrm.resize(base + nn * nblk * BLK, 0);
            for i in 0..nc {
                for q in 0..nn {
                    cst.nrm[base + q * nblk * BLK + i] = cp.nu_code(q, cc.cnrm[i * nn + q]);
                }
            }
        }
        ahat.extend_from_slice(&cc.items.ah);
        // codes rounded up on the cluster's grid: nondecreasing in sort order
        nukey.extend(cc.items.nu.iter().map(|&v| ((v / info.numax * 65535.0).ceil() as u32).min(65535) as u16));
        row_ent.extend_from_slice(&cc.items.ent);
        xs_row.extend_from_slice(&cc.items.ext);
        row0 += nc;
        cl.push(info);
    }
    ahat.extend_from_slice(&[0i16; 32]);
    // 64 bytes of slack so masked and full vector loads past a level's end stay inside the allocation
    for l in 0..nl {
        lcode[l].extend_from_slice(&[0u8; 128]);
        lscal[l].extend_from_slice(&[0u8; 128]);
    }
    cst.code.extend_from_slice(&[0u8; 128]);
    cst.nrm.extend_from_slice(&[0u8; 128]);
    for gp in groups.iter_mut() {
        gp.vinv = Vec::new();
        gp.vq = Vec::new();
    }
    let nrows = row0;
    let maxent = row_ent.iter().copied().max().unwrap_or(0) as usize;
    let mut ent_row = vec![u32::MAX; maxent + 1];
    for (r, &e) in row_ent.iter().enumerate() {
        ent_row[e as usize] = r as u32;
    }
    let mut full = full;
    match &mut full {
        Full2::Real { map, off, rows } => {
            let src = unsafe { std::slice::from_raw_parts(map[*off..].as_ptr() as *const u16, (map.len() - *off) / 2) };
            *rows = vec![0u16; nrows * n];
            rows.par_chunks_mut(n).zip(xs_row.par_iter()).for_each(|(d, &r)| d.copy_from_slice(&src[r as usize * n..(r as usize + 1) * n]));
        }
        Full2::Synth(sy) => {
            sy.ext = xs_row;
            sy.row_cl = cl.iter().enumerate().flat_map(|(ci, c)| std::iter::repeat(ci as u32).take(c.r1 - c.r0)).collect();
        }
    }
    let ctab: Vec<Vec<kern::CTab>> = cl.par_iter().map(|c| {
        let gp = &groups[c.g];
        let wm = c.wm(gp);
        gp.levels.iter().map(|lp| kern::centre_tab(&wm, lp, &et)).collect()
    }).collect();
    let ix = Index2 {
        col: if build_col { Some(cst) } else { None },
        nukey,
        repack: vec![0.6; meta.nl()],
        ctab,
        meta,
        groups,
        codes,
        etab: et,
        cl,
        centres64: p.centres[..k * n].iter().map(|&x| x as f64).collect(),
        rots,
        anchors: Vec::new(),
        hi: Vec::new(),
        lo: Vec::new(),
        nanch: 0,
        ahat,
        row_ent,
        ent_row,
        lcode,
        lscal,
        coff,
        soff,
        esc,
        x2,
        full,
        nrows,
        split,
    };
    let mut ek = [0usize; 4];
    for (key, _) in ix.esc.iter() {
        ek[match key.1 { 0 => 0, 1..=7 => 1, 8..=15 => 2, _ => 3 }] += 1;
    }
    log(format!("assembled {:.1}s; x2 bound {:.3e}; escapes: det norms {}, |y| {}, kappa {}, tan {}", t.s(), x2 - 1.0, ek[0], ek[1], ek[2], ek[3]));
    ix
}

impl GroupPlan2 {
    /// The plan without the basis inverse (filled in by the caller).
    pub fn new_light(p: &Params, prof: &[f64], g: usize, codes: &DetTab, et: &mut ExpTab, tof: &dyn Fn(f64) -> f64) -> GroupPlan2 {
        GroupPlan2::new_inner(p, prof, g, codes, et, tof)
    }
}

/// Support table over cluster-centre anchors (data only): exact max/min of p.x per cluster, rounded outward.
pub fn build_table2(ix: &mut Index2, nanch: usize) {
    let n = ix.n();
    let k = ix.k();
    let step = k as f64 / nanch as f64;
    let mut anchors = vec![0f64; nanch * n];
    for a in 0..nanch {
        let c = (a as f64 * step) as usize;
        anchors[a * n..(a + 1) * n].copy_from_slice(&ix.cl[c].mhat);
    }
    let cols: Vec<(Vec<f32>, Vec<f32>)> = (0..k).into_par_iter().map(|c| {
        let cl = &ix.cl[c];
        let nc = cl.r1 - cl.r0;
        let mut x = vec![0f64; nc * n];
        for i in 0..nc {
            ix.full_row_f64(cl.r0 + i, &mut x[i * n..(i + 1) * n]);
        }
        let mut px = vec![0f64; nc * nanch];
        unsafe {
            matrixmultiply::dgemm(nc, n, nanch, 1.0, x.as_ptr(), n as isize, 1, anchors.as_ptr(), 1, n as isize, 0.0, px.as_mut_ptr(), nanch as isize, 1);
        }
        let mut hi = vec![0f32; nanch];
        let mut lo = vec![0f32; nanch];
        for a in 0..nanch {
            let (mut mx, mut mi) = (f64::NEG_INFINITY, f64::INFINITY);
            for i in 0..nc {
                mx = mx.max(px[i * nanch + a]);
                mi = mi.min(px[i * nanch + a]);
            }
            hi[a] = (((mx + 1e-9) * 32768.0).ceil() / 32768.0) as f32;
            lo[a] = (((mi - 1e-9) * 32768.0).floor() / 32768.0) as f32;
        }
        (hi, lo)
    }).collect();
    ix.hi = vec![0f32; nanch * k];
    ix.lo = vec![0f32; nanch * k];
    for c in 0..k {
        for a in 0..nanch {
            ix.hi[a * k + c] = cols[c].0[a];
            ix.lo[a * k + c] = cols[c].1[a];
        }
    }
    ix.anchors = anchors;
    ix.nanch = nanch;
}
