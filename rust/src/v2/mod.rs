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
pub mod disk;
#[cfg(test)]
mod e2e;
pub mod kern;
pub mod q;
pub mod setup;
pub mod store;
pub mod tail;

/// How an index is built from a fitted directory. The default is the build SYSTEM.md describes.
#[derive(Clone, Debug)]
pub struct BuildOptions {
    /// The query profile, in place of the directory's profile.npy.
    pub profile: Option<String>,
    /// Build only the first this many clusters (a quick check on a subset).
    pub clusters: Option<usize>,
    /// Another randomised level schedule (cumulative bytes, rising from above the last deterministic level); the union
    /// bound's constant follows the number of levels.
    pub rlv: Option<Vec<usize>>,
    /// Design the randomised rates with the exact-law constant of each set of coded blocks, not the sub-Gaussian one.
    pub design_exact: bool,
    /// Sort each cluster's rows by their residual norm (the norm sort) instead of by key.
    pub nusort: bool,
    /// N in the randomised levels' budget, in place of the row count rounded up to a power of two.
    pub nguar: Option<f64>,
    /// Keep the sub-Gaussian constant at every randomised level instead of the exact-law constants of `tail`.
    pub csg_plain: bool,
    /// Clusters of each group the encoder's calibration samples.
    pub cal_per_group: usize,
}
impl Default for BuildOptions {
    fn default() -> BuildOptions {
        BuildOptions { profile: None, clusters: None, rlv: None, design_exact: false, nusort: false, nguar: None, csg_plain: false, cal_per_group: 8 }
    }
}

use crate::derive;
use crate::index::{item_scale, Params, DA, SEED};
use crate::util::*;
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
pub use store::Arr;

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

/// The randomised code's tables: `VALUE[depth][code]` is the decoded value of a code's first `depth` bits, and
/// `CODE[depth][cell]` the code of the greedy expansion that ends in `cell` (cells numbered from -1 upwards, each
/// 2^(1 - depth) wide; the expansion's first step is the cell index's top bit and the code's bit 0).
static RAND_TABLES: std::sync::LazyLock<([[f64; 256]; 9], [[u8; 256]; 9])> = std::sync::LazyLock::new(|| {
    let mut value = [[0f64; 256]; 9];
    let mut code = [[0u8; 256]; 9];
    for d in 0..9 {
        for p in 0..256usize {
            value[d][p] = (0..d).map(|b| if p >> b & 1 == 1 { 0.5f64.powi(b as i32 + 1) } else { -0.5f64.powi(b as i32 + 1) }).sum();
        }
        for cell in 0..1usize << d {
            code[d][cell] = if d == 0 { 0 } else { (cell as u8).reverse_bits() >> (8 - d) };
        }
    }
    (value, code)
});

/// Greedy nested binary expansion of v (already divided by lambda rms) to `depth` bits; bit d set: t_d = +1. The
/// expansion's decisions compare v with dyadic points exactly, so it is the cell of v on the grid of step 2^(1 - depth).
#[inline]
pub fn rand_encode(v: f64, depth: usize) -> u8 {
    if depth == 0 {
        return 0;
    }
    let half = (1u32 << (depth - 1)) as f64;
    let cell = ((v * half).floor() + half).clamp(0.0, 2.0 * half - 1.0) as usize;
    RAND_TABLES.1[depth][cell]
}
#[inline]
pub fn rand_value(p: u8, depth: usize) -> f64 {
    RAND_TABLES.0[depth][p as usize & ((1usize << depth) - 1)]
}

/// The most rises a block can have: one per randomised level.
pub const MAX_RISES: usize = 16;

/// Encode a block: positions to their final depth `maxd`, with the encoder scale (in units of the rms) chosen per block
/// from LAMS to minimise the sum over the block's rises of log tan^2 (any scale is sound: kappa is stored). Writes the
/// chosen code to `out` and, per rise, the decoded estimate's dot product with y and squared norm to `dc`.
pub fn rand_choose(y: &[f64], maxd: &[usize], rises: &[&[u8]], out: &mut [u8], tmp: &mut [u8], dc: &mut [(f64, f64)]) {
    let m = y.len();
    let nr = rises.len();
    assert!(nr <= MAX_RISES, "a block rises at {nr} levels; at most {MAX_RISES} are supported");
    let yn2: f64 = y.iter().map(|v| v * v).sum();
    let rms = (yn2 / m as f64).sqrt().max(1e-300);
    let mut best = f64::INFINITY;
    let value = &RAND_TABLES.0;
    for &lam in LAMS.iter() {
        for p in 0..m {
            tmp[p] = rand_encode(y[p] / (lam * rms), maxd[p]);
        }
        // every rise's sums in one pass over the positions, each summed in position order
        let mut dot = [0f64; MAX_RISES];
        let mut cn = [0f64; MAX_RISES];
        for p in 0..m {
            let (t, yp) = (tmp[p] as usize, y[p]);
            for k in 0..nr {
                let d = rises[k][p] as usize;
                let v = value[d][t & ((1 << d) - 1)];
                dot[k] += v * yp;
                cn[k] += v * v;
            }
        }
        let mut obj = 0.0;
        for k in 0..nr {
            obj += ((cn[k] * yn2 - dot[k] * dot[k]).max(1e-300) / (dot[k] * dot[k]).max(1e-300)).ln();
        }
        if obj < best {
            best = obj;
            out[..m].copy_from_slice(&tmp[..m]);
            for k in 0..nr {
                dc[k] = (dot[k], cn[k]);
            }
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
            let mut dc = [(0f64, 0f64); MAX_RISES];
            rand_choose(&y, &maxd, &rises, &mut pat, &mut tmp, &mut dc);
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
    Tree { to: u8 },
    Resid,
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
    pub vq: Vec<f64>,   // V_q row-major n x n (build only)
    pub cal: Option<CalIn>, // the calibration the scalar grids were set from (none for a group with no items)
}

/// The scalar grids' calibration: the deterministic norm codes' offsets, the |y| grids (start and steps per octave in
/// log2), the tan(phi) levels, and the kappa grids (start, steps per octave, bits) per randomised level and block.
#[derive(Clone, Debug)]
pub struct CalIn {
    pub kmin: [i32; NDL],
    pub y0: [f64; J],
    pub ys: [f64; J],
    pub tq: Vec<[[f32; 15]; J]>,
    pub k0: Vec<[f64; J]>,
    pub ks: Vec<[f64; J]>,
    pub kb: Vec<[u8; J]>,
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

/// The int8 basis: one scale per basis vector (column); row c of `v8t` is basis vector c in int8, `vsc[c]` its scale.
fn int8_basis(v: &[f32], n: usize) -> (Vec<i8>, Vec<f64>) {
    let mut vsc = vec![0f64; n];
    let mut v8t = vec![0i8; n * n];
    for c in 0..n {
        let mx = (0..n).map(|i| (v[i * n + c] as f64).abs()).fold(0.0, f64::max);
        vsc[c] = mx / 127.0;
        for i in 0..n {
            v8t[c * n + i] = (v[i * n + c] as f64 / vsc[c]).round().clamp(-127.0, 127.0) as i8;
        }
    }
    (v8t, vsc)
}

/// The int8 basis and its inverse by Newton-Schulz, started from the transpose when the basis is near orthogonal and
/// from a Gauss-Jordan inverse otherwise (the dual basis); also the basis as f64 and a bound on the inverse's defect.
fn quant_basis(v: &[f32], n: usize) -> (Vec<i8>, Vec<f64>, Vec<f64>, Vec<f64>, f64) {
    let (v8t, vsc) = int8_basis(v, n);
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
    fn new(p: &Params, prof: &[f64], g: usize, dt_: &DetTab, et: &mut ExpTab, tof: &dyn Fn(f64) -> f64, design_c: Option<&[f64]>) -> GroupPlan2 {
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
        let rv = match design_c {
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
                    (Field { widx: c as u16, nb: to, unit: 0, off: 0, kind: FK::Tree { to }, eid: 0 }, exp)
                }
                Move::Resid { c, j, r } => {
                    let dl = dt_.delta[j as usize][r as usize];
                    let mut exp = [0f32; 16];
                    exp[0] = -dl;
                    exp[1] = dl;
                    (Field { widx: c as u16, nb: 1, unit: 0, off: 0, kind: FK::Resid, eid: 0 }, exp)
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
        let mut gp = GroupPlan2 {
            sig,
            lamsum,
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
            cal: None,
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
    pub fn set_calibration(&mut self, cal: CalIn) {
        let CalIn { kmin, y0, ys, tq, k0, ks, kb } = cal.clone();
        self.cal = Some(cal);
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

/// `k` values that round up the samples `v` at or below their `top` quantile with the least mean square ratio of the
/// rounded value to the sample, the largest being that quantile (the rest escape): dynamic programming over up to
/// 1,024 candidate values at quantiles of `v`.
pub fn roundup_levels(v: &mut Vec<f64>, k: usize, top: f64) -> Vec<f64> {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let hi = quantile(v, top);
    let xs: Vec<f64> = v.iter().copied().filter(|&x| x <= hi && x > 0.0).collect();
    let m = xs.len().min(1024);
    if m < k {
        return (0..k).map(|c| quantile(v, top * (c + 1) as f64 / k as f64)).collect();
    }
    // candidate b is the largest sample of the b-th of m equal-count runs; prefix sums of 1/x^2 and counts by candidate
    let ends: Vec<usize> = (1..=m).map(|b| b * xs.len() / m).collect();
    let mut cand: Vec<f64> = ends.iter().map(|&e| xs[e - 1]).collect();
    cand[m - 1] = hi;
    let mut pinv = vec![0f64; m + 1];
    let mut cnt = vec![0f64; m + 1];
    for b in 0..m {
        let a = if b == 0 { 0 } else { ends[b - 1] };
        pinv[b + 1] = pinv[b] + xs[a..ends[b]].iter().map(|x| 1.0 / (x * x)).sum::<f64>();
        cnt[b + 1] = ends[b] as f64;
    }
    // cost of rounding the samples of runs a..=b (a = 0: from the bottom) up to cand[b]
    let cost = |a: usize, b: usize| cand[b] * cand[b] * (pinv[b + 1] - pinv[a]) - (cnt[b + 1] - cnt[a]);
    let mut f: Vec<f64> = (0..m).map(|b| cost(0, b)).collect();
    let mut from = vec![vec![usize::MAX; m]; k];
    for l in 1..k {
        let mut g = vec![f64::INFINITY; m];
        for b in l..m {
            for a in l - 1..b {
                let c = f[a] + cost(a + 1, b);
                if c < g[b] {
                    g[b] = c;
                    from[l][b] = a;
                }
            }
        }
        f = g;
    }
    let mut out = vec![0f64; k];
    let mut b = m - 1;
    for l in (0..k).rev() {
        out[l] = cand[b];
        if l > 0 {
            b = from[l][b];
        }
    }
    out
}

#[cfg(test)]
mod rand_code_tests {
    use super::*;

    /// The greedy expansion and its decoder as first written: one decision per bit against the running estimate.
    fn greedy(v: f64, depth: usize) -> u8 {
        let (mut c, mut p) = (0.0, 0u8);
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
    fn value(p: u8, depth: usize) -> f64 {
        (0..depth).map(|d| if p >> d & 1 == 1 { 0.5f64.powi(d as i32 + 1) } else { -0.5f64.powi(d as i32 + 1) }).sum()
    }

    #[test]
    fn tables_match_the_greedy_expansion() {
        let mut vs: Vec<f64> = (-3000..=3000).map(|i| i as f64 / 1024.0).collect();
        // every cell edge at every depth, and the values either side of it
        for d in 1..=8 {
            for k in -(1i64 << d)..=(1i64 << d) {
                let e = k as f64 / (1u64 << (d - 1)) as f64;
                vs.extend([e, e.next_down(), e.next_up()]);
            }
        }
        vs.extend([f64::INFINITY, f64::NEG_INFINITY, f64::NAN, 1e300, -1e300, 0.0, -0.0]);
        for &v in &vs {
            for d in 0..=8 {
                let p = greedy(v, d);
                assert_eq!(rand_encode(v, d), p, "v {v} depth {d}");
                for e in 0..=d {
                    assert_eq!(rand_value(p, e).to_bits(), value(p, e).to_bits(), "prefix {e} of v {v} depth {d}");
                }
            }
        }
    }
}

#[cfg(test)]
mod roundup_tests {
    use super::*;
    #[test]
    fn optimal_levels_round_up_no_worse_than_equal_mass_quantiles() {
        use rand::SeedableRng;
        use rand_distr::{Distribution, LogNormal};
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(5);
        let d = LogNormal::new(-1.0, 0.25).unwrap();
        let mut v: Vec<f64> = (0..20_000).map(|_| d.sample(&mut rng)).collect();
        let lv = roundup_levels(&mut v, 15, 0.998);
        assert!(lv.windows(2).all(|w| w[0] < w[1]) && lv[14] == quantile(&mut v, 0.998));
        let q: Vec<f64> = (0..15).map(|c| quantile(&mut v, 0.002 + 0.996 * c as f64 / 14.0)).collect();
        let ex = |l: &[f64]| v.iter().filter(|&&x| x <= l[14]).map(|&x| (l[l.partition_point(|&y| y < x)] / x).powi(2) - 1.0).sum::<f64>();
        assert!(ex(&lv) < 0.8 * ex(&q), "{} {}", ex(&lv), ex(&q));
    }
}

/// largest relative error of a kappa decoded from its nearest code on a grid of `steps` per octave, with f32 slack
pub fn eta_steps(steps: f64) -> f64 {
    2f64.powf(0.5 / steps) * (1.0 + 1e-6) - 1.0
}

// ---------------- the index ----------------

#[derive(Clone)]
pub struct Meta2 {
    pub n: usize,
    pub dlv: Vec<usize>,
    pub rlv: Vec<usize>,
    /// The randomised levels' constants are for an index of 2^npow items (all rows, whatever any viewer sees, rounded
    /// up, unless `--nguar` gave N). Every result carries it; a result cache must not mix constants inside one result
    /// and must drop its results when npow rises (a fall is safe). The budget is one-sided, so a result is valid for
    /// its own (query, threshold) or its own k-NN query only: reusing an item's intervals for another threshold, or a
    /// threshold's intervals for a later k-NN query, needs the two-sided budget (ln(1/delta) larger by ln 2).
    pub npow: u32,
}
impl Meta2 {
    pub fn nl(&self) -> usize {
        NDL + self.rlv.len()
    }
    pub fn level_bytes(&self, l: usize) -> usize {
        if l < NDL { self.dlv[l] } else { self.rlv[l - NDL] }
    }
}

#[derive(Clone)]
pub struct Cl2 {
    pub g: usize,
    pub r0: usize,
    pub r1: usize,
    pub nblk: usize,
    pub mn: f64,
    pub mn32: f32,
    pub mhat: Arc<Vec<f64>>,
    pub mv32: Arc<Vec<f32>>, // V_q^T m
    pub wmr: Arc<Vec<f32>>,  // R_j (mv*dt) by position
    pub bm: [f64; NDL],
    pub b2: [f64; J],
    pub l1m: [f32; J + 1], // |wm| sums: det, then per block
    pub btot: f64,
    pub mvn: f64,
    pub rmax: f64,
    pub numax: f64,      // norm sort: the largest |z|, the grid of the stored codes (nu = numax code / 65535)
    pub bmin: f64,       // norm sort: bounds of m.r over the cluster
    pub bmax: f64,
}

#[repr(C, align(64))]
#[derive(Clone, Copy)]
struct Line([u16; 32]);

/// f16 values starting on a cache line, so that rows of a multiple of 32 values each start on one (a row that does not
/// costs a scan about a third more).
#[derive(Default)]
pub struct F16Rows {
    lines: Vec<Line>,
    len: usize,
}
impl F16Rows {
    pub fn zeroed(len: usize) -> F16Rows {
        F16Rows { lines: vec![Line([0; 32]); (len + 31) / 32], len }
    }
}
impl std::ops::Deref for F16Rows {
    type Target = [u16];
    fn deref(&self) -> &[u16] {
        unsafe { std::slice::from_raw_parts(self.lines.as_ptr() as *const u16, self.len) }
    }
}
impl std::ops::DerefMut for F16Rows {
    fn deref_mut(&mut self) -> &mut [u16] {
        unsafe { std::slice::from_raw_parts_mut(self.lines.as_mut_ptr() as *mut u16, self.len) }
    }
}

/// Full vectors: `Real` holds them in index row order (a copy the build fills, or the saved index's file), so a scan of
/// the rows in order reads memory in order. A compacted index shares its parent's copy (through `Index2::vrow`) or holds
/// its own.
pub enum Full2 {
    Real { rows: Arc<disk::Rows> },
    Synth(q::Synth2),
}

/// The query-independent parts that do not depend on which rows a cluster holds are shared (Arc) with the indexes
/// compacted from this one (`Index2::compact`).
pub struct Index2 {
    pub meta: Meta2,
    pub groups: Arc<Vec<GroupPlan2>>,
    pub codes: Arc<DetTab>,
    pub etab: Arc<ExpTab>,
    pub cl: Vec<Cl2>,
    pub centres64: Arc<Vec<f64>>,
    pub rots: Arc<Vec<Vec<f64>>>,
    pub anchors: Arc<Vec<f64>>,
    pub hi: Arc<Vec<f32>>,
    pub lo: Arc<Vec<f32>>,
    pub nanch: usize,
    // the per-row arrays: on the heap after a build, mapped from the files of a saved index after an open
    pub ahat: Arr<i16>,
    pub row_ent: Arr<u32>,
    pub ent_row: Arr<u32>,
    pub lcode: Vec<Arr<u8>>, // per level: code rows, clusters concatenated
    pub lscal: Vec<Arr<u8>>, // per level: scalar rows
    pub coff: Vec<Vec<usize>>, // per level per cluster: byte offset into lcode
    pub soff: Vec<Vec<usize>>,
    pub esc: HashMap<(u8, u8, u32), f32>, // (level, slot, row) -> value: det norm (slot 0), yh (1 + j), kappa (8 + j), tan (16 + j)
    pub x2: f64,
    pub full: Full2,
    pub nrows: usize,
    pub ctab: Arc<Vec<Vec<kern::CTab>>>, // per cluster, per level: the centre's tables (query-independent)
    pub repack: Vec<f32>,
    pub vrow: Vec<u32>,  // a compacted index sharing its parent's vectors: each row's row in Full2::Real (empty: the row itself)
    pub nent: usize,     // entity ids are below this
    pub nukey: Arr<u16>, // norm sort: per row, |z| as a code on its cluster's grid, rounded up (empty without the sort)            // per level: occupancy of slots held in place below which they are compacted first
}

/// The format of the encoded index (codes, scalars, centre tables), which this build derives when it opens a fitted
/// directory.
pub const INDEX_FORMAT: u32 = 3;

impl Cl2 {
    /// The centre's weights, as the query side combines them: det by coordinate mv sig (f32), rand by position.
    pub fn wm(&self, gp: &GroupPlan2) -> Vec<f32> {
        let mut w: Vec<f32> = self.mv32.iter().zip(&gp.sig).map(|(&m, &s)| m * s).collect();
        w.extend_from_slice(&self.wmr);
        w
    }
}

impl Index2 {
    /// A hash of what the encoder writes per row: the codes and scalars of every level, the 16-bit keys (`ahat`), the
    /// row-to-entity map (`row_ent`, which fixes the row order) and the escapes. It does not cover the norm keys
    /// (`nukey`), the entity-to-row map (`ent_row`), the stored vectors, the `x2` bound, the per-cluster records and
    /// centre tables, the plans or the calibration; `store_hash` covers everything. It uses `DefaultHasher`, whose values
    /// may change between Rust releases; it is kept as it is because reference hashes were recorded with it.
    pub fn code_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        // hashed as the build's vectors were before the arrays could be mapped: lengths, then elements
        for v in [&self.lcode, &self.lscal] {
            v.len().hash(&mut h);
            for a in v {
                a[..].hash(&mut h);
            }
        }
        self.ahat[..].hash(&mut h);
        self.row_ent[..].hash(&mut h);
        let mut e: Vec<(u8, u8, u32, u32)> = self.esc.iter().map(|(k, v)| (k.0, k.1, k.2, v.to_bits())).collect();
        e.sort_unstable();
        e.hash(&mut h);
        h.finish()
    }
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
    /// The stored vectors, when this index's row ids address them directly: not when it shares a parent's through
    /// `vrow`, whose rows the reader and the page-cache hints would then miss.
    pub fn own_rows(&self) -> Option<&disk::Rows> {
        match &self.full {
            Full2::Real { rows, .. } if self.vrow.is_empty() => Some(rows),
            _ => None,
        }
    }
    /// Whether the stored vectors of `rows` are most likely in memory (always, unless they are on file).
    pub fn full_cached(&self, rows: &[u32]) -> bool {
        self.own_rows().is_none_or(|fr| fr.cached(rows, self.n()))
    }
    pub fn full_row_f16(&self, row: usize) -> &[u16] {
        match &self.full {
            Full2::Real { rows, .. } => {
                let n = self.n();
                let row = if self.vrow.is_empty() { row } else { self.vrow[row] as usize };
                rows.row(row, n)
            }
            Full2::Synth(_) => panic!("synthetic rows have no stored f16 vector"),
        }
    }
    /// The index restricted to the rows `rows` (ascending; the visible rows of a session): each cluster keeps those of
    /// its rows, in their order, with their keys, codes, scalars and escapes copied into blocks of their own, and with
    /// `vectors` their f16 vectors too (without, a row reads its parent's vector). Everything that does not depend on
    /// which rows a cluster holds is shared, and every bound a cluster stores holds for any subset of its rows, so each
    /// row is decided as it is in the parent. Row i of the result is row rows[i] of this index.
    pub fn compact(&self, rows: &[u32], vectors: bool) -> Index2 {
        assert!(rows.windows(2).all(|w| w[0] < w[1]), "compact needs ascending rows");
        let Full2::Real { rows: frows } = &self.full else { panic!("a synthetic index cannot be compacted") };
        let nl = self.meta.nl();
        let n = self.n();
        let k = self.k();
        let start: Vec<usize> = (0..=k).map(|ci| rows.partition_point(|&r| (r as usize) < if ci < k { self.cl[ci].r0 } else { self.nrows })).collect();
        let cl: Vec<Cl2> = (0..k).map(|ci| {
            let mut c = self.cl[ci].clone();
            c.r0 = start[ci];
            c.r1 = start[ci + 1];
            c.nblk = (c.r1 - c.r0 + BLK - 1) / BLK;
            c
        }).collect();
        // one level's nibble rows of a cluster: item i of the source to item j of the copy
        let copy = |src: &[u8], dst: &mut [u8], nr: usize, i: usize, j: usize| {
            let (sb, sl, db, dl) = (i / BLK * nr * 16 + i % 16, i % BLK >= 16, j / BLK * nr * 16 + j % 16, j % BLK >= 16);
            for r in 0..nr {
                let v = src[sb + r * 16];
                let v = if sl { v >> 4 } else { v & 15 };
                dst[db + r * 16] |= if dl { v << 4 } else { v };
            }
        };
        let per: Vec<Vec<(Vec<u8>, Vec<u8>)>> = (0..k).into_par_iter().map(|ci| {
            let (c, d) = (&self.cl[ci], &cl[ci]);
            let gp = &self.groups[c.g];
            (0..nl).map(|l| {
                let (u, sr) = (gp.levels[l].units, gp.levels[l].srows);
                let mut lc = vec![0u8; d.nblk * u * 16];
                let mut ls = vec![0u8; d.nblk * sr * 16];
                let (cs, ss) = (&self.lcode[l][self.coff[l][ci]..], &self.lscal[l][self.soff[l][ci]..]);
                for (j, &r) in rows[d.r0..d.r1].iter().enumerate() {
                    let i = r as usize - c.r0;
                    copy(cs, &mut lc, u, i, j);
                    copy(ss, &mut ls, sr, i, j);
                }
                (lc, ls)
            }).collect()
        }).collect();
        let (mut lcode, mut lscal, mut coff, mut soff) = (vec![Vec::new(); nl], vec![Vec::new(); nl], vec![Vec::new(); nl], vec![Vec::new(); nl]);
        for l in 0..nl {
            for ci in 0..k {
                coff[l].push(lcode[l].len());
                lcode[l].extend_from_slice(&per[ci][l].0);
                soff[l].push(lscal[l].len());
                lscal[l].extend_from_slice(&per[ci][l].1);
            }
            lcode[l].extend_from_slice(&[0u8; 128]);
            lscal[l].extend_from_slice(&[0u8; 128]);
        }
        drop(per);
        let mut ahat: Vec<i16> = rows.iter().map(|&r| self.ahat[r as usize]).collect();
        ahat.extend_from_slice(&[0i16; 32]);
        let mut esc = HashMap::new();
        for (&(l, slot, r), &v) in &self.esc {
            if let Ok(i) = rows.binary_search(&r) {
                esc.insert((l, slot, i as u32), v);
            }
        }
        let (full, vrow) = if vectors {
            let mut v = F16Rows::zeroed(rows.len() * n);
            v.par_chunks_mut(n).zip(rows.par_iter()).for_each(|(d, &r)| d.copy_from_slice(self.full_row_f16(r as usize)));
            (Full2::Real { rows: Arc::new(disk::Rows::Mem(v)) }, Vec::new())
        } else {
            let vr = rows.iter().map(|&r| if self.vrow.is_empty() { r } else { self.vrow[r as usize] }).collect();
            (Full2::Real { rows: frows.clone() }, vr)
        };
        Index2 {
            meta: self.meta.clone(),
            groups: self.groups.clone(),
            codes: self.codes.clone(),
            etab: self.etab.clone(),
            cl,
            centres64: self.centres64.clone(),
            rots: self.rots.clone(),
            anchors: self.anchors.clone(),
            hi: self.hi.clone(),
            lo: self.lo.clone(),
            nanch: self.nanch,
            ahat: ahat.into(),
            row_ent: rows.iter().map(|&r| self.row_ent[r as usize]).collect(),
            ent_row: Arr::default(),
            lcode: lcode.into_iter().map(Arr::from).collect(),
            lscal: lscal.into_iter().map(Arr::from).collect(),
            coff,
            soff,
            esc,
            x2: self.x2,
            full,
            nrows: rows.len(),
            ctab: self.ctab.clone(),
            repack: self.repack.clone(),
            vrow,
            nent: self.nent,
            nukey: if self.nukey.is_empty() { Arr::default() } else { rows.iter().map(|&r| self.nukey[r as usize]).collect() },
        }
    }
    /// Bytes this index holds of its own (codes, scalars, keys, ids, escapes and its own vectors), not the parts it
    /// shares with the index it was compacted from.
    pub fn own_bytes(&self) -> usize {
        let full = match &self.full {
            Full2::Real { rows, .. } if self.vrow.is_empty() => match &**rows {
                disk::Rows::Mem(v) => v.len() * 2,
                disk::Rows::Map { .. } => 0,
            },
            _ => 0,
        };
        self.lcode.iter().chain(&self.lscal).map(|v| v.len()).sum::<usize>() + self.ahat.len() * 2 + self.row_ent.len() * 4
            + self.vrow.len() * 4 + self.nukey.len() * 2 + self.esc.len() * 24 + self.cl.len() * std::mem::size_of::<Cl2>() + full
    }
    /// The row of entity e. A compacted index has no entity-to-row map (it would cost 4 bytes per entity of the
    /// parent): map an entity through the index it was compacted from, then to the compacted row by the row list.
    pub fn row_of_ent(&self, e: u32) -> usize {
        assert!(!self.ent_row.is_empty() || self.nrows == 0, "this index is compacted to a visible set and has no entity-to-row map; map entity ids through the index it was compacted from");
        self.ent_row[e as usize] as usize
    }
    /// Whether this index was compacted to a visible set (Index2::compact).
    pub fn compacted(&self) -> bool {
        self.ent_row.is_empty()
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
    pub x16: Vec<u16>, // the f16 vectors in the same order (empty for a source without stored vectors)
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
        let x16 = if self.x16.is_empty() { Vec::new() } else { o.iter().flat_map(|&k| self.x16[k * n..(k + 1) * n].iter().copied()).collect() };
        Items2 { ah: o.iter().map(|&k| self.ah[k]).collect(), z, ent: o.iter().map(|&k| self.ent[k]).collect(), ext: o.iter().map(|&k| self.ext[k]).collect(), x2max: self.x2max, nu: o.iter().map(|&k| nu[k]).collect(), x16 }
    }
}

pub trait Source2: Sync {
    fn items(&self, c: usize, p: &Params, gp: &GroupPlan2, mhat: &[f64]) -> Items2;
    /// The number of items of cluster c (what `items` returns for it).
    fn size(&self, c: usize, p: &Params) -> usize {
        p.cstart[c + 1] - p.cstart[c]
    }
    /// The number of rows the build encodes from this source.
    fn rows(&self, p: &Params) -> usize {
        p.cstart[p.meta.k]
    }
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
        let x16 = key.iter().flat_map(|k| self.src.row(k.1).iter().copied()).collect();
        Items2 { ah: key.iter().map(|k| k.0).collect(), z, ent: key.iter().map(|k| p.perm[k.1] as u32).collect(), ext: key.iter().map(|k| k.1 as u32).collect(), x2max, nu: Vec::new(), x16 }
    }
}

struct Calib {
    kpool: Vec<Vec<f64>>,
    ypool: Vec<Vec<f64>>,
    tpool: Vec<Vec<Vec<f64>>>,
    kapool: Vec<Vec<Vec<f64>>>,
}

pub struct ClusterCode2 {
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
    for i in 0..nc {
        let z = &it.z[i * n..(i + 1) * n];
        let s = svec[i] as f64;
        val.iter_mut().for_each(|v| *v = 0.0);
        for l in 0..NDL {
            let lp = &gp.levels[l];
            for f in &lp.parts[0].fields {
                let c = f.widx as usize;
                let u = z[c] / (gp.sig[c] as f64 * s);
                let code = match f.kind {
                    FK::Tree { to, .. } => codes.index(to as usize, u),
                    _ => (u - val[c] >= 0.0) as usize,
                };
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
    // per block: the randomised levels at which it rises, and their depths over the block
    let rise_lr: Vec<Vec<usize>> = (0..J).map(|j| (0..nr).filter(|&lr| gp.thchg[lr].contains(&j)).collect()).collect();
    let rises: Vec<Vec<&[u8]>> = (0..J).map(|j| rise_lr[j].iter().map(|&lr| &gp.rdepth[lr][gp.boff[j]..gp.boff[j + 1]]).collect()).collect();
    // per (block, randomised level): the chosen code's dot product with y and squared norm
    let mut rdc = vec![(0f64, 0f64); J * nr];
    let mut dc = [(0f64, 0f64); MAX_RISES];
    for i in 0..nc {
        let s = svec[i];
        let y = &ymat[i * n..(i + 1) * n];
        let mut yn = [0f64; J];
        let mut yhv = [0f64; J];
        for j in 0..J {
            yn[j] = y[gp.boff[j]..gp.boff[j + 1]].iter().map(|v| v * v).sum::<f64>().sqrt();
            let (b0, b1) = (gp.boff[j], gp.boff[j + 1]);
            rand_choose(&y[b0..b1], &maxd[b0..b1], &rises[j], &mut rp[b0..b1], &mut rtmp, &mut dc);
            for (k, &lr) in rise_lr[j].iter().enumerate() {
                rdc[j * nr + lr] = dc[k];
            }
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
                let (dot, cn) = rdc[j * nr + lr];
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
                // the decoded scale must bound the true one: tan(phi) here, |y| at the first randomised level
                let tdec = if tc < 15 { gp.ttab[lr][j][tc as usize] } else { esc.last().unwrap().3 };
                assert!(tdec as f64 >= tanf && yhv[j] >= yn[j], "item {i} block {j} level {l}: decoded scale below the true one (tan {tdec} < {tanf} or |y| {} < {})", yhv[j], yn[j]);
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
    ClusterCode2 { codes: lc, scal: ls, esc, items: it, calib: cal }
}

/// Query-independent per-cluster data.
fn cluster_info(gp: &GroupPlan2, m: &[f32], rots: &[Vec<f64>], ah: &[i16], x2: f64, n: usize) -> Cl2 {
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
    let rmax = ah.iter().map(|&a| {
        let al = a as f64 / 32767.0 - DA;
        (x2 + mn * mn - 2.0 * mn * al).max(0.0).sqrt()
    }).fold(0f64, f64::max);
    let nc = ah.len();
    let mut l1m = [0f32; J + 1];
    l1m[0] = (wm[..n].iter().map(|x| x.abs() as f64).sum::<f64>() * 1.001) as f32;
    for j in 0..J {
        l1m[1 + j] = (wm[n + gp.boff[j]..n + gp.boff[j + 1]].iter().map(|x| x.abs() as f64).sum::<f64>() * 1.001) as f32;
    }
    let mv32: Vec<f32> = mv.iter().map(|&x| x as f32).collect();
    let wmr = wm[n..].to_vec();
    Cl2 { g: 0, r0: 0, r1: nc, nblk: (nc + BLK - 1) / BLK, mn, mn32, mhat: Arc::new(mhat), mv32: Arc::new(mv32), wmr: Arc::new(wmr), bm, b2, l1m, btot: b2.iter().sum(), mvn: mv.iter().map(|x| x * x).sum::<f64>().sqrt(), rmax, numax: 0.0, bmin: 0.0, bmax: 0.0 }
}

/// What the build and the opening of a saved index both derive from the fit: the level metadata, the rotations, the
/// code tables and every group's plan with its int8 basis, for an index of `rows` rows. With `inverse` (the build) also
/// each basis's inverse and its defect; an open reads the defect from the saved index instead.
pub struct Plans {
    pub meta: Meta2,
    pub rots: Vec<Vec<f64>>,
    pub codes: DetTab,
    pub et: ExpTab,
    pub groups: Vec<GroupPlan2>,
    /// the guarantee's N
    pub nguar: f64,
}

pub fn plans(p: &Params, prof: &[f64], rows: usize, o: &BuildOptions, design_c: Option<&[f64]>, inverse: bool, log: &mut dyn FnMut(String)) -> Plans {
    let n = p.meta.n;
    // the guarantee's N: the item count rounded up to a power of two, or as given
    let nguar = o.nguar.unwrap_or_else(|| (rows.max(1) as f64).log2().ceil().exp2());
    if nguar < rows as f64 {
        log(format!("the guarantee's N, {nguar}, is below the {rows} rows built: the guarantee covers only {nguar} rows"));
    }
    let meta = Meta2 { n, dlv: p.meta.dlv.clone(), rlv: p.meta.rlv.clone(), npow: nguar.log2().ceil() as u32 };
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
        groups.push(GroupPlan2::new(p, prof, g, &codes, &mut et, &tof, design_c));
    }
    groups.par_iter_mut().enumerate().for_each(|(g, gp)| {
        let v = &p.v[g * n * n..(g + 1) * n * n];
        if inverse {
            let (v8t, vsc, vinv, vq, e) = quant_basis(v, n);
            gp.v8t = v8t;
            gp.vsc = vsc;
            gp.vinv = vinv;
            gp.vq = vq;
            gp.e_inv = e;
        } else {
            (gp.v8t, gp.vsc) = int8_basis(v, n);
        }
    });
    Plans { meta, rots, codes, et, groups, nguar }
}

/// Build the fast-scan index from `src`: in memory, or, with `dir`, straight into the files of a saved index there
/// (sealed afterwards by `store::seal`). `prof` is the G x n profile. With `nusort`, each cluster is sorted by the
/// residual's norm in the metric instead of by x.m^ (see Items2::sort_by_nu).
pub fn build2(p: &Params, prof: &[f64], src: &dyn Source2, full: Full2, o: &BuildOptions, design_c: Option<&[f64]>, dir: Option<&str>, log: &mut dyn FnMut(String)) -> Index2 {
    let nusort = o.nusort;
    let t = Timer::new();
    let n = p.meta.n;
    let k = p.meta.k;
    let rows = src.rows(p);
    let Plans { meta, rots, codes, et, mut groups, nguar } = plans(p, prof, rows, o, design_c, true, log);
    {
        // the randomised levels' constants, one per distinct set of coded blocks
        let tc = Timer::new();
        let set = |gp: &GroupPlan2, lr: usize| -> Vec<usize> { (0..J).filter(|&j| gp.rates[lr][j] > 0.0).map(|j| gp.nb[j] - 1).collect() };
        let mut sets: Vec<Vec<usize>> = groups.iter().flat_map(|gp| (0..gp.rates.len()).map(move |lr| set(gp, lr))).collect();
        sets.sort();
        sets.dedup();
        let plain = o.csg_plain;
        // one event per (item, level), on the side that would make the item's decision wrong: delta = eps / (N L)
        let l1 = (nguar * meta.rlv.len() as f64 / p.meta.eps).ln();
        log(format!("query budget: N = {nguar:.0} (2^{}; {} items), L = {}, eps = {:e}, one side per event: ln(1/delta) = {l1:.4}", meta.npow, rows, meta.rlv.len(), p.meta.eps));
        let vals: HashMap<Vec<usize>, (f64, tail::Source)> = sets.par_iter().map(|s| (s.clone(), if plain { ((2.0 * l1).sqrt(), tail::Source::SubGaussian) } else { tail::level_constant(s, l1, &p.meta.certs) })).collect();
        for gp in groups.iter_mut() {
            for lr in 0..gp.rates.len() {
                gp.csgl[lr] = vals[&set(gp, lr)].0 as f32;
            }
        }
        log(format!("randomised constants {:.1}s (sub-Gaussian c = {:.3}): {:?}", tc.s(), (2.0 * l1).sqrt(), sets.iter().map(|s| (s.clone(), (vals[s].0 * 1000.0).round() / 1000.0, vals[s].1)).collect::<Vec<_>>()));
    }
    log(format!("plans {:.1}s; residual-stage distortion / tree {:?}; largest inverse defect {:.2e}; rates g0 {:?}", t.s(), TREE_DEPTHS.iter().map(|&j| (1..=MAX_STAGES).map(|r| (codes.d[j][r] / p.gtree[j + r] * 1000.0).round() / 1000.0).collect::<Vec<_>>()).collect::<Vec<_>>(), groups.iter().map(|g| g.e_inv).fold(0.0, f64::max), groups[0].rates));
    let mhats: Vec<Vec<f64>> = (0..k).map(|c| {
        let m = &p.centres[c * n..(c + 1) * n];
        let mn = m.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt();
        m.iter().map(|&x| x as f64 / mn).collect()
    }).collect();
    let mn32: Vec<f32> = (0..k).map(|c| p.centres[c * n..(c + 1) * n].iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt() as f32).collect();
    let nr = meta.rlv.len();
    // calibration on o.cal_per_group clusters of each group
    let mut calc = Vec::new();
    for g in 0..p.meta.g {
        let ks: Vec<usize> = (0..k).filter(|&c| p.grp[c] == g).collect();
        calc.extend(ks.iter().step_by((ks.len() / o.cal_per_group.max(1)).max(1)));
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
                    let lv = roundup_levels(&mut v, 15, 0.998);
                    for cc in 0..15 {
                        tq[lr][j][cc] = (lv[cc] * (1.0 + 1e-6)) as f32;
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
        groups[g].set_calibration(CalIn { kmin, y0, ys, tq, k0, ks: ksv, kb: kbv });
    }
    log(format!("calibrated {:.1}s", t.s()));
    let nl = meta.nl();
    let real = matches!(full, Full2::Real { .. });
    if let Some(d) = dir {
        assert!(real, "a synthetic index has no stored vectors and is built in memory only, not in {d}");
    }
    // every cluster's place in every array, from its size and its group's plan, before anything is encoded
    let sizes: Vec<usize> = (0..k).map(|c| src.size(c, p)).collect();
    let mut row0 = vec![0usize; k + 1];
    for c in 0..k {
        row0[c + 1] = row0[c] + sizes[c];
    }
    let nrows = row0[k];
    let (mut coff, mut soff) = (vec![vec![0usize; k]; nl], vec![vec![0usize; k]; nl]);
    let (mut ctot, mut stot) = (vec![0usize; nl], vec![0usize; nl]);
    for l in 0..nl {
        for c in 0..k {
            let (nb, lp) = (sizes[c].div_ceil(BLK), &groups[p.grp[c]].levels[l]);
            (coff[l][c], soff[l][c]) = (ctot[l], stot[l]);
            ctot[l] += nb * lp.units * 16;
            stot[l] += nb * lp.srows * 16;
        }
    }
    // 128 bytes of slack after each level and 32 keys after the last row, so masked and full vector loads past an
    // array's end stay inside it
    let lcode: Vec<store::Out<u8>> = (0..nl).map(|l| store::Out::new(dir, &format!("code{l}.bin"), ctot[l] + 128)).collect();
    let lscal: Vec<store::Out<u8>> = (0..nl).map(|l| store::Out::new(dir, &format!("scal{l}.bin"), stot[l] + 128)).collect();
    let mut ahat = store::Out::<i16>::new(dir, "ahat.bin", nrows + 32);
    let row_ent = store::Out::<u32>::new(dir, "row_ent.bin", nrows);
    let nukey = store::Out::<u16>::new(dir, "nukey.bin", if nusort { nrows } else { 0 });
    let vecs = store::OutRows::new(dir, "vectors.bin", if real { nrows * n } else { 0 });
    let ext = store::Out::<u32>::new(None, "", if real { 0 } else { nrows });
    // what each cluster leaves for the assembly: its escapes (local rows), largest |x|^2 - 1 and the norm sort's grid
    struct Done {
        esc: Vec<(u8, u8, u32, f32)>,
        x2max: f64,
        numax: f64,
    }
    let done: Vec<Done> = (0..k).into_par_iter().map(|c| {
        let gp = &groups[p.grp[c]];
        let it = src.items(c, p, gp, &mhats[c]);
        let it = if nusort { it.sort_by_nu(n) } else { it };
        assert_eq!(it.ah.len(), sizes[c], "cluster {c} gave {} items where the source gave its size as {}", it.ah.len(), sizes[c]);
        let cc = encode_cluster(it, gp, &codes, &et, &meta, &rots, mn32[c], false);
        let r0 = row0[c];
        // SAFETY (each put below): cluster c writes only its own ranges, fixed above from the sizes before encoding
        // (rows row0[c]..row0[c + 1], level bytes coff/soff[l][c] for its blocks), so concurrent writes are disjoint
        for l in 0..nl {
            unsafe {
                lcode[l].put(coff[l][c], &cc.codes[l]);
                lscal[l].put(soff[l][c], &cc.scal[l]);
            }
        }
        let it = &cc.items;
        unsafe {
            ahat.put(r0, &it.ah);
            row_ent.put(r0, &it.ent);
        }
        let numax = if nusort { (it.nu.iter().cloned().fold(0.0, f64::max) * (1.0 + 1e-12)).max(1e-30) } else { 0.0 };
        if nusort {
            // codes rounded up on the cluster's grid: nondecreasing in sort order
            let key: Vec<u16> = it.nu.iter().map(|&v| ((v / numax * 65535.0).ceil() as u32).min(65535) as u16).collect();
            unsafe { nukey.put(r0, &key) };
        }
        if real {
            assert_eq!(it.x16.len(), sizes[c] * n, "cluster {c}: the source gave no f16 vectors for a real index");
            unsafe { vecs.put(r0 * n, &it.x16) };
        } else {
            unsafe { ext.put(r0, &it.ext) };
        }
        Done { esc: cc.esc, x2max: it.x2max, numax }
    }).collect();
    log(format!("encoded {:.1}s", t.s()));
    let x2 = 1.0 + done.iter().map(|d| d.x2max).fold(0.0, f64::max) * (1.0 + 1e-9) + 1e-12;
    // the per-cluster records, from the written keys
    let ah_all = ahat.get();
    let cl: Vec<Cl2> = (0..k).into_par_iter().map(|c| {
        let ah = &ah_all[row0[c]..row0[c + 1]];
        let mut info = cluster_info(&groups[p.grp[c]], &p.centres[c * n..(c + 1) * n], &rots, ah, x2, n);
        (info.g, info.r0, info.r1) = (p.grp[c], row0[c], row0[c + 1]);
        if nusort {
            // m.r = mn a - mn^2 from the key, within mn DA
            info.numax = done[c].numax;
            let mr = |a: i16| info.mn * (a as f64 / 32767.0) - info.mn * info.mn;
            info.bmax = ah.iter().map(|&a| mr(a)).fold(f64::NEG_INFINITY, f64::max) + info.mn * DA + 1e-12;
            info.bmin = ah.iter().map(|&a| mr(a)).fold(f64::INFINITY, f64::min) - info.mn * DA - 1e-12;
        }
        info
    }).collect();
    let mut esc = HashMap::new();
    for (c, d) in done.iter().enumerate() {
        for &(l, slot, i, v) in &d.esc {
            esc.insert((l, slot, (row0[c] + i as usize) as u32), v);
        }
    }
    drop(done);
    for gp in groups.iter_mut() {
        gp.vinv = Vec::new();
        gp.vq = Vec::new();
    }
    let row_ent = row_ent.finish();
    let maxent = row_ent.iter().copied().max().unwrap_or(0) as usize;
    let mut ent_row = store::Out::<u32>::new(dir, "ent_row.bin", maxent + 1);
    ent_row.put_mut(0, &vec![u32::MAX; maxent + 1]);
    for (r, &e) in row_ent.iter().enumerate() {
        // k-NN counts rows, which is exact only with one row per entity
        assert!(ent_row.get()[e as usize] == u32::MAX, "entity {e} has two rows; the index needs one row per entity");
        ent_row.put_mut(e as usize, &[r as u32]);
    }
    let full = match full {
        Full2::Real { .. } => Full2::Real { rows: Arc::new(vecs.finish()) },
        Full2::Synth(mut sy) => {
            sy.ext = ext.finish().to_vec();
            sy.row_cl = (0..k).flat_map(|c| std::iter::repeat(c as u32).take(sizes[c])).collect();
            Full2::Synth(sy)
        }
    };
    let ctab: Vec<Vec<kern::CTab>> = cl.par_iter().map(|c| {
        let gp = &groups[c.g];
        let wm = c.wm(gp);
        gp.levels.iter().map(|lp| kern::centre_tab(&wm, lp, &et)).collect()
    }).collect();
    let ix = Index2 {
        nukey: nukey.finish(),
        repack: vec![0.6; meta.nl()],
        ctab: Arc::new(ctab),
        meta,
        groups: Arc::new(groups),
        codes: Arc::new(codes),
        etab: Arc::new(et),
        cl,
        centres64: Arc::new(p.centres[..k * n].iter().map(|&x| x as f64).collect()),
        rots: Arc::new(rots),
        anchors: Arc::new(Vec::new()),
        hi: Arc::new(Vec::new()),
        lo: Arc::new(Vec::new()),
        vrow: Vec::new(),
        nent: maxent + 1,
        nanch: 0,
        ahat: ahat.finish(),
        row_ent,
        ent_row: ent_row.finish(),
        lcode: lcode.into_iter().map(|o| o.finish()).collect(),
        lscal: lscal.into_iter().map(|o| o.finish()).collect(),
        coff,
        soff,
        esc,
        x2,
        full,
        nrows,
    };
    if let Some(d) = dir {
        store::save_rest(&ix, d);
    }
    let mut ek = [0usize; 4];
    for (key, _) in ix.esc.iter() {
        ek[match key.1 { 0 => 0, 1..=7 => 1, 8..=15 => 2, _ => 3 }] += 1;
    }
    log(format!("assembled {:.1}s; x2 bound {:.3e}; escapes: det norms {}, |y| {}, kappa {}, tan {}; code hash {:016x}", t.s(), x2 - 1.0, ek[0], ek[1], ek[2], ek[3], ix.code_hash()));
    ix
}


/// The fit in `dir` with the options `o` applied, and the profile: the inputs of a build, and of the key of the index
/// they build.
pub fn prepare(dir: &str, o: &BuildOptions) -> (Params, Vec<f64>) {
    let mut p = crate::index::load_params(dir);
    if let Some(nv) = o.nguar {
        assert!(nv > 0.0, "the guarantee's N is {nv}; give the number of rows the guarantee should cover, or none to use the row count");
    }
    if let Some(new) = &o.rlv {
        assert!(!new.is_empty() && new.windows(2).all(|w| w[0] < w[1]) && new[0] > *p.meta.dlv.last().unwrap(), "the randomised levels {new:?} must rise from above the last deterministic level, {}", p.meta.dlv.last().unwrap());
        let l1 = p.meta.csg * p.meta.csg / 2.0 + (new.len() as f64 / p.meta.rlv.len() as f64).ln();
        p.meta.csg = (2.0 * l1).sqrt();
        p.meta.rlv = new.clone();
    }
    if let Some(kk) = o.clusters.filter(|&kk| kk < p.meta.k) {
        let n = p.meta.n;
        p.meta.k = kk;
        p.cstart.truncate(kk + 1);
        p.grp.truncate(kk);
        p.centres.truncate(kk * n);
    }
    let prof = derive::load_profile(o.profile.as_deref().unwrap_or(&format!("{dir}/profile.npy")));
    (p, prof)
}

/// The randomised levels' design constants by coded set when `o` designs the rates with the exact-law constants.
pub fn design_constants(p: &Params, o: &BuildOptions, log: &mut dyn FnMut(String)) -> Option<Vec<f64>> {
    o.design_exact.then(|| {
        let b = &p.meta.bounds;
        let l1 = p.meta.csg * p.meta.csg / 2.0;
        let cs: Vec<f64> = (0..1u32 << (b.len() - 1)).into_par_iter().map(|set| {
            let ds: Vec<usize> = (0..b.len() - 1).filter(|&j| set >> j & 1 == 1).map(|j| b[j + 1] - b[j] - 1).collect();
            tail::level_constant(&ds, l1, &[]).0
        }).collect();
        log(format!("[design] constants by coded set {:?}", cs.iter().map(|x| (x * 1000.0).round() / 1000.0).collect::<Vec<_>>()));
        cs
    })
}

/// Where the index of the fit in `dir` built with `o` is saved: `dir/encoded-KEY`, KEY its `store::build_key`.
pub fn saved_dir(dir: &str, o: &BuildOptions) -> String {
    let (p, prof) = prepare(dir, o);
    format!("{dir}/encoded-{}", store::build_key(&p, &prof, o))
}

/// Encode the fit in `p` (from `prepare`): the codes and, without the norm sort, the support table, in memory or, with
/// `dir`, into the files of a saved index there (not yet sealed).
fn encode(p: &Params, prof: &[f64], design_c: Option<&[f64]>, o: &BuildOptions, dir: Option<&str>, log: &mut dyn FnMut(String)) -> Index2 {
    let src = Real2 { src: crate::index::RealSource::open(&p.xs_path) };
    let mut ix = build2(p, prof, &src, Full2::Real { rows: Default::default() }, o, design_c, dir, &mut |s| log(format!("[build2] {s}")));
    if !o.nusort {
        build_table2(&mut ix, p.meta.k);
    }
    ix
}

/// The repack thresholds, timed on this machine (they choose how the query side packs slots, not what it reads).
fn set_repack(ix: &mut Index2, log: &mut dyn FnMut(String)) {
    let t = Timer::new();
    ix.repack = q::calibrate_repack(ix);
    log(format!("[repack] occupancy thresholds by level {:?} (timed in {:.1}s)", ix.repack.iter().map(|x| (x * 100.0).round() / 100.0).collect::<Vec<_>>(), t.s()));
}

/// The fitted directory `dir` built into a queryable index in memory (codes, repack thresholds and, without the norm
/// sort, the support table), with the parameters it was built from. Nothing is saved.
pub fn build(dir: &str, o: &BuildOptions, log: &mut dyn FnMut(String)) -> (Params, Index2) {
    let (p, prof) = prepare(dir, o);
    let design_c = design_constants(&p, o, log);
    let mut ix = encode(&p, &prof, design_c.as_deref(), o, None, log);
    log(format!("[build2] store hash {:016x}", ix.store_hash()));
    set_repack(&mut ix, log);
    (p, ix)
}

/// The index of the fitted directory `dir` built with `o`, opened from where it is saved (`saved_dir`) without encoding
/// anything, with the parameters it was built from. A missing index, or one built from another fit or other options,
/// is an error that says to rebuild it (`rebuild`).
pub fn open(dir: &str, o: &BuildOptions, log: &mut dyn FnMut(String)) -> Result<(Params, Index2), String> {
    open_in(dir, None, o, log)
}

/// As `open`, the index saved in `edir` (wherever `rebuild_at` saved it).
pub fn open_at(dir: &str, edir: &str, o: &BuildOptions, log: &mut dyn FnMut(String)) -> Result<(Params, Index2), String> {
    open_in(dir, Some(edir), o, log)
}

fn open_in(dir: &str, edir: Option<&str>, o: &BuildOptions, log: &mut dyn FnMut(String)) -> Result<(Params, Index2), String> {
    let (p, prof) = prepare(dir, o);
    let key = store::build_key(&p, &prof, o);
    let edir = edir.map_or_else(|| format!("{dir}/encoded-{key}"), |e| e.to_string());
    let edir = edir.as_str();
    store::log_stale(&store::parent_of(edir), edir, &mut |s| log(format!("[open] {s}")));
    let design_c = design_constants(&p, o, log);
    let mut ix = store::open(&p, &prof, o, design_c.as_deref(), &key, edir, &mut |s| log(format!("[open] {s}")))?;
    set_repack(&mut ix, log);
    Ok((p, ix))
}

/// Encode the fitted directory `dir` with `o` and save the index where `open` finds it (`saved_dir`), replacing any
/// index saved there; returns it, with the parameters it was built from.
pub fn rebuild(dir: &str, o: &BuildOptions, log: &mut dyn FnMut(String)) -> (Params, Index2) {
    rebuild_in(dir, None, o, log)
}

/// As `rebuild`, saved in `edir`. The rebuild holds `encoded.lock` beside `edir` for its whole run (a second rebuild
/// there waits), writes into a scratch directory of its own beside `edir` and swaps it into place once complete and
/// synced (`store::install`).
pub fn rebuild_at(dir: &str, edir: &str, o: &BuildOptions, log: &mut dyn FnMut(String)) -> (Params, Index2) {
    rebuild_in(dir, Some(edir), o, log)
}

fn rebuild_in(dir: &str, edir: Option<&str>, o: &BuildOptions, log: &mut dyn FnMut(String)) -> (Params, Index2) {
    let (p, prof) = prepare(dir, o);
    let key = store::build_key(&p, &prof, o);
    let edir = edir.map_or_else(|| format!("{dir}/encoded-{key}"), |e| e.to_string());
    let edir = edir.as_str();
    let parent = store::parent_of(edir);
    std::fs::create_dir_all(&parent).unwrap_or_else(|e| panic!("cannot create {parent}: {e}"));
    let lock = store::Lock::take(&parent, &mut |s| log(format!("[build2] {s}")));
    store::log_stale(&parent, edir, &mut |s| log(format!("[build2] {s}")));
    let design_c = design_constants(&p, o, log);
    // built into the files of a scratch directory of this rebuild's own, sealed with its key, then moved into place
    let tmp = store::scratch_dir(edir);
    let mut ix = encode(&p, &prof, design_c.as_deref(), o, Some(&tmp), log);
    let h = store::seal(&ix, &tmp, &key);
    store::install(&tmp, edir);
    drop(lock);
    log(format!("[build2] saved to {edir}; store hash {h:016x}"));
    set_repack(&mut ix, log);
    (p, ix)
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
    let mut hi = vec![0f32; nanch * k];
    let mut lo = vec![0f32; nanch * k];
    for c in 0..k {
        for a in 0..nanch {
            hi[a * k + c] = cols[c].0[a];
            lo[a * k + c] = cols[c].1[a];
        }
    }
    ix.hi = Arc::new(hi);
    ix.lo = Arc::new(lo);
    ix.anchors = Arc::new(anchors);
    ix.nanch = nanch;
}
