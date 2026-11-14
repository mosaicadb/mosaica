//! The columnar store of the deterministic code: each coordinate's code at its final depth (the tree index once, then
//! its residual stages) in 4-bit rows across the cluster's items. Coordinates are packed into units of whole rows
//! (most carry 1 to 3 bits, so a unit holds several), read together, so that a query can read the units in any order.
//! The values are those of the level stream exactly (the stream re-stores a tree index at each depth; the column
//! stores it once), so after every unit the item stands where the stream stands after its last deterministic level, and
//! the randomised levels follow unchanged.
//!
//! The bound after reading any set R of the coded coordinates D (nonD the rest), with w_c = (qv_c - alpha mv_c) sigma_c
//! (the weight of u_c = z_c / (s sigma_c)), v_c the code's value and e_c = z_c - s sigma_c v_c (e_c = z_c off D):
//!
//!   (q - alpha m).r - s sum_R w_c v_c = sum_{R + nonD} qv'_c e_c + s sum_{D - R} w_c u_c
//!   |first| <= |(qv' d5)_{R + nonD}| rho5           (Cauchy-Schwarz; rho5 = |e / d5| over every coordinate, stored)
//!   |second| <= s sum_b |(w h)_{b - R}| sqrt(N_b^2 - |(u / h)_{b R}|^2)
//!
//! with N_b >= |(u / h)_b| the stored norm of block b of D. The energy read is bounded below from the code values:
//! |(u / h)_{b R}| >= |(v / h)_{b R}| - kappa_b rho5 / s, kappa_b = max over b R of d5_c / (sigma_c h_c), since
//! |(e / (s sigma h))_{b R}| <= kappa_b |e / d5| / s; |(v / h)_{b R}|^2 is summed from tables rounded down. The single
//! stored norm N >= |(u / h)_D| gives the second term as |(w h)_{D - R}| sqrt(N^2 - |(u / h)_R|^2).
use super::*;

pub const MAXB: usize = 8;

/// A coded coordinate of a group, in the group's fixed order.
#[derive(Clone, Debug)]
pub struct ColCoord {
    pub c: u16,
    pub j: u8,
    pub r: u8,
    pub blk: u8,
    pub cap: f64, // 1 - D(j, r): the share of the coordinate's variance its code captures
    pub h: f64,   // the norm weight
    pub tree: (u16, u8),        // (row, bit offset) of the tree index
    pub stages: Vec<(u16, u8)>, // of each residual stage
}

/// A field of a row: coordinate k's bits at offset `off`, with its values by field code.
#[derive(Clone, Debug)]
pub struct ColField {
    pub k: u16,
    pub off: u8,
    pub nb: u8,
    pub tree: bool,
    pub fv: [f64; 16],
}

/// A 4-bit row: its fields and the lower bounds of sum (u / h)^2 over its tree fields, by nibble, in steps of its
/// block's energy step.
#[derive(Clone, Debug)]
pub struct ColRow {
    pub fields: Vec<ColField>,
    pub ex: Vec<(u16, [f32; 16], f64)>, // per field: coordinate, values by nibble (f32), largest |value|
    pub en: [u8; 16],
    pub blk: u8,
}

/// Coordinates read together: coords[c0..c1] (fixed order), rows row0..row0 + nrow.
#[derive(Clone, Debug)]
pub struct ColUnit {
    pub c0: u16,
    pub c1: u16,
    pub row0: u16,
    pub nrow: u8,
    pub blk: u8,
}

#[derive(Clone, Debug, Default)]
pub struct ColPlan {
    pub coords: Vec<ColCoord>,
    pub rows: Vec<ColRow>,
    pub units: Vec<ColUnit>,
    pub nb: usize,
    pub bstart: Vec<usize>, // first coordinate (fixed order) of each block, then |D|
    pub estep: Vec<f64>,    // per block
    pub nond: Vec<u16>,     // coordinates without code
    pub d5: Vec<f64>,       // per coordinate (all n): the weights of the last stored norm
    // norm grids (set when the index is assembled): 0 the single norm, 1 + b block b
    pub nu_base: Vec<f64>,
    pub nu_st: Vec<f64>,
    pub nu2: Vec<[f32; 256]>, // decoded squares, rounded up
    pub nuv: Vec<[f32; 256]>, // decoded values, rounded up
}

/// Block edges in the fixed order (`VEXACT_COLB`, comma-separated coordinate counts).
pub fn block_edges(nd: usize) -> Vec<usize> {
    let s = std::env::var("VEXACT_COLB").unwrap_or_else(|_| "0,4,12,32,64".to_string());
    let mut e: Vec<usize> = s.split(',').map(|x| x.trim().parse::<usize>().unwrap().min(nd)).collect();
    e.push(nd);
    e.dedup();
    assert!(e.len() - 1 <= MAXB, "at most {MAXB} norm blocks");
    e
}

/// Pack fields (sizes 1, 2, 4) into rows: fours alone, twos in pairs, an odd two with two ones, ones in fours; the last
/// row may be padded. Returns per field (row, offset) and the number of rows.
fn pack_fields(nbs: &[u8]) -> (Vec<(u16, u8)>, usize) {
    let mut at = vec![(0u16, 0u8); nbs.len()];
    let by = |b: u8| -> Vec<usize> { (0..nbs.len()).filter(|&i| nbs[i] == b).collect() };
    let (fours, twos, ones) = (by(4), by(2), by(1));
    assert!(fours.len() + twos.len() + ones.len() == nbs.len(), "fields of 3 bits");
    let mut rows = 0u16;
    for &i in &fours {
        at[i] = (rows, 0);
        rows += 1;
    }
    let mut k = 0;
    while k + 1 < twos.len() {
        at[twos[k]] = (rows, 0);
        at[twos[k + 1]] = (rows, 2);
        rows += 1;
        k += 2;
    }
    let mut oi = 0;
    if k < twos.len() {
        at[twos[k]] = (rows, 0);
        let mut off = 2;
        while oi < ones.len() && off < 4 {
            at[ones[oi]] = (rows, off);
            oi += 1;
            off += 1;
        }
        rows += 1;
    }
    while oi < ones.len() {
        for off in 0..4 {
            if oi < ones.len() {
                at[ones[oi]] = (rows, off);
                oi += 1;
            }
        }
        rows += 1;
    }
    (at, rows as usize)
}

/// The group's columnar plan from its final deterministic states and level fields.
pub fn col_plan(n: usize, lam: &[f64], pq: &[f64], states: &[(u8, u8)], levels: &[LevelPlan], d5: &[f32], et: &ExpTab, codes: &DetTab) -> ColPlan {
    // the stream's fields of each coordinate, in order
    let mut fl: Vec<Vec<Field>> = vec![Vec::new(); n];
    for lp in &levels[..NDL] {
        for f in &lp.parts[0].fields {
            fl[f.widx as usize].push(*f);
        }
    }
    let fval = |f: &Field, k: usize| et.e[f.eid as usize][k << f.off] as f64;
    let mut d: Vec<usize> = (0..n).filter(|&c| states[c].0 > 0).collect();
    let bits = |c: usize| (states[c].0 + states[c].1) as f64;
    let gain = |c: usize| pq[c] * lam[c] * (1.0 - codes.d[states[c].0 as usize][states[c].1 as usize]) / bits(c);
    d.sort_by(|&a, &b| gain(b).partial_cmp(&gain(a)).unwrap().then(a.cmp(&b)));
    let be = block_edges(d.len());
    let nb = be.len() - 1;
    let mut coords: Vec<ColCoord> = Vec::new();
    let mut rows: Vec<ColRow> = Vec::new();
    let mut units: Vec<ColUnit> = Vec::new();
    // per coordinate: its field sizes and value tables (tree index first)
    let mut cf: Vec<Vec<(u8, [f64; 16])>> = Vec::new();
    for (k, &c) in d.iter().enumerate() {
        let (j, r) = (states[c].0 as usize, states[c].1 as usize);
        let fs = &fl[c];
        let trees: Vec<&Field> = fs.iter().filter(|f| matches!(f.kind, FK::Tree { .. })).collect();
        let stages: Vec<&Field> = fs.iter().filter(|f| matches!(f.kind, FK::Resid { .. })).collect();
        assert_eq!(stages.len(), r);
        assert!(matches!(trees.last().unwrap().kind, FK::Tree { to, .. } if to as usize == j));
        let h = (pq[c] * lam[c] + 1e-30).powf(-0.25);
        let mut tv = [0f64; 16];
        for idx in 0..1usize << j {
            for f in &trees {
                if let FK::Tree { to, .. } = f.kind {
                    tv[idx] += fval(f, idx >> (j - to as usize));
                }
            }
        }
        let mut fields = vec![(j as u8, tv)];
        for f in &stages {
            let mut v = [0f64; 16];
            v[0] = fval(f, 0);
            v[1] = fval(f, 1);
            fields.push((1, v));
        }
        cf.push(fields);
        let blk = (0..nb).find(|&b| k < be[b + 1]).unwrap();
        coords.push(ColCoord { c: c as u16, j: j as u8, r: r as u8, blk: blk as u8, cap: 1.0 - codes.d[j][r], h, tree: (0, 0), stages: Vec::new() });
    }
    // units: coordinates in fixed order within a block until their bits fill whole rows
    let nd = d.len();
    let mut k0 = 0;
    while k0 < nd {
        let blk = coords[k0].blk;
        let mut k1 = k0;
        let mut tb = 0usize;
        while k1 < nd && coords[k1].blk == blk {
            tb += (coords[k1].j + coords[k1].r) as usize;
            k1 += 1;
            if tb % 4 == 0 {
                break;
            }
        }
        let mut nbs = Vec::new();
        let mut who = Vec::new();
        for k in k0..k1 {
            for (fi, f) in cf[k].iter().enumerate() {
                nbs.push(f.0);
                who.push((k, fi));
            }
        }
        let (at, nr) = pack_fields(&nbs);
        let row0 = rows.len();
        for _ in 0..nr {
            rows.push(ColRow { fields: Vec::new(), ex: Vec::new(), en: [0; 16], blk });
        }
        for (i, &(k, fi)) in who.iter().enumerate() {
            let (rr, off) = at[i];
            let row = row0 + rr as usize;
            rows[row].fields.push(ColField { k: k as u16, off, nb: nbs[i], tree: fi == 0, fv: cf[k][fi].1 });
            if fi == 0 {
                coords[k].tree = (row as u16, off);
            } else {
                coords[k].stages.push((row as u16, off));
            }
        }
        units.push(ColUnit { c0: k0 as u16, c1: k1 as u16, row0: row0 as u16, nrow: nr as u8, blk });
        k0 = k1;
    }
    // energy tables: per row, for each coordinate whose tree index it holds, a lower bound of (v / h)^2 from the row's
    // own fields of that coordinate, the stages held elsewhere at their largest magnitude
    let row_lo2 = |r: usize, e: usize| -> f64 {
        let row = &rows[r];
        let mut tot = 0.0;
        for f in row.fields.iter().filter(|f| f.tree) {
            let k = f.k as usize;
            let mut v = 0.0;
            for g in row.fields.iter().filter(|g| g.k as usize == k) {
                v += g.fv[(e >> g.off) & ((1 << g.nb) - 1)];
            }
            let mut out = 0.0;
            for &(sr, soff) in &coords[k].stages {
                if sr as usize != r {
                    let g = rows[sr as usize].fields.iter().find(|g| g.k as usize == k && !g.tree && g.off == soff).unwrap();
                    out += g.fv[0].abs().max(g.fv[1].abs());
                }
            }
            let m = (v.abs() - out).max(0.0);
            tot += (m / coords[k].h).powi(2);
        }
        tot * (1.0 - 1e-9)
    };
    let mut estep = vec![1e-30f64; nb];
    for (r, row) in rows.iter().enumerate() {
        let m = (0..16).map(|e| row_lo2(r, e)).fold(0.0, f64::max);
        estep[row.blk as usize] = estep[row.blk as usize].max(m / 255.0 * (1.0 + 1e-9));
    }
    let ens: Vec<[u8; 16]> = (0..rows.len()).map(|r| core::array::from_fn(|e| ((row_lo2(r, e) / estep[rows[r].blk as usize]) * (1.0 - 1e-9)).floor().clamp(0.0, 255.0) as u8)).collect();
    for (row, en) in rows.iter_mut().zip(ens) {
        row.en = en;
        row.ex = row.fields.iter().map(|f| {
            let m = (1usize << f.nb) - 1;
            (f.k, core::array::from_fn(|e| f.fv[(e >> f.off) & m] as f32), f.fv.iter().fold(0f64, |a, b| a.max(b.abs())))
        }).collect();
    }
    let ind: std::collections::HashSet<usize> = d.iter().copied().collect();
    let nond = (0..n).filter(|c| !ind.contains(c)).map(|c| c as u16).collect();
    ColPlan { coords, rows, units, nb, bstart: be, estep, nond, d5: d5.iter().map(|&x| x as f64).collect(), ..Default::default() }
}

impl ColPlan {
    pub fn nrows(&self) -> usize {
        self.rows.len()
    }
    /// Value of a row's nibble e with the coordinates' weights w.
    pub fn row_value(&self, r: usize, e: usize, w: &[f64]) -> f64 {
        self.rows[r].fields.iter().map(|f| w[f.k as usize] * f.fv[(e >> f.off) & ((1 << f.nb) - 1)]).sum()
    }
    /// Value of coordinate k from its tree index and stage bits (the encoder's sum).
    pub fn value(&self, k: usize, tid: u8, sb: u8) -> f64 {
        let cc = &self.coords[k];
        let tf = self.rows[cc.tree.0 as usize].fields.iter().find(|f| f.k as usize == k && f.tree).unwrap();
        let mut v = tf.fv[tid as usize];
        for (i, &(r, off)) in cc.stages.iter().enumerate() {
            let f = self.rows[r as usize].fields.iter().find(|f| f.k as usize == k && !f.tree && f.off == off).unwrap();
            v += f.fv[(sb >> i) as usize & 1];
        }
        v
    }
    /// Norm grids from the values of every item of the group (columns: single norm, then each block).
    pub fn set_grids(&mut self, vals: &[Vec<f64>]) {
        self.nu_base.clear();
        self.nu_st.clear();
        self.nu2.clear();
        self.nuv.clear();
        for v in vals {
            let lo = v.iter().cloned().filter(|&x| x > 0.0).fold(f64::INFINITY, f64::min);
            let hi = v.iter().cloned().fold(0.0, f64::max);
            let (lo, hi) = if lo.is_finite() { (lo, hi.max(lo)) } else { (1e-12, 1e-12) };
            let st = (255.0 / (hi / lo).log2().max(1e-9)).min(32.0).floor().max(1.0);
            let base = lo.log2();
            let mut t2 = [0f32; 256];
            let mut tv = [0f32; 256];
            for k in 0..256 {
                let x = 2f64.powf(base + k as f64 / st) * (1.0 + 1e-6);
                tv[k] = (x * (1.0 + 1e-7)) as f32;
                t2[k] = (x * x * (1.0 + 1e-6)) as f32;
            }
            self.nu_base.push(base);
            self.nu_st.push(st);
            self.nu2.push(t2);
            self.nuv.push(tv);
        }
    }
    /// The code of norm value x on grid i (decoded value at least x).
    pub fn nu_code(&self, i: usize, x: f64) -> u8 {
        if x <= 0.0 {
            return 0;
        }
        let mut k = ((x.log2() - self.nu_base[i]) * self.nu_st[i]).ceil().max(0.0) as i64;
        while k < 255 && (self.nuv[i][k as usize] as f64) < x {
            k += 1;
        }
        assert!(k <= 255 && self.nuv[i][k as usize] as f64 >= x, "a norm past its grid");
        k as u8
    }
}

/// The columnar store of the index: per cluster the rows (row r of a cluster of nblk blocks at coff + r nblk 16, block
/// b's 32 items in 16 bytes as the level stream lays them out) and the norm codes (one byte per item; row i of a
/// cluster at noff + i nblk 32).
#[derive(Default)]
pub struct ColStore {
    pub code: Vec<u8>,
    pub coff: Vec<usize>,
    pub nrm: Vec<u8>,
    pub noff: Vec<usize>,
}

/// Nibble i of row `row` of a cluster's columnar code (nblk blocks).
#[inline]
pub fn setnib_col(buf: &mut [u8], nblk: usize, i: usize, row: usize, val: u8) {
    let (blk, lane) = (i / BLK, i % BLK);
    let byte = row * nblk * 16 + blk * 16 + (lane % 16);
    if lane < 16 {
        buf[byte] |= val & 15;
    } else {
        buf[byte] |= (val & 15) << 4;
    }
}

// ---------------- the query side ----------------

use super::q::*;
use crate::kernel::Stats;
use crate::simd::*;

/// The checks of the columnar path, in rows read (`VEXACT_COLCK`, comma-separated; the last check is always after
/// every coded coordinate).
pub fn checkpoints() -> &'static [usize] {
    static CK: std::sync::OnceLock<Vec<usize>> = std::sync::OnceLock::new();
    CK.get_or_init(|| {
        let s = std::env::var("VEXACT_COLCK").unwrap_or_else(|_| "0,8,16,24,32,48,64,96,128".to_string());
        s.split(',').map(|x| x.trim().parse().unwrap()).collect()
    })
}

/// One block of 32 candidate items of a cluster on the columnar path.
#[derive(Clone)]
pub struct ColSlot {
    pub bi: u32,
    pub live: u32,
    pub nread: u32, // lanes whose block norms have been read
    pub th: [i32; 32],
    pub en: [[i32; 32]; MAXB],
    pub s: [f32; 32],
    pub mr: [f32; 32],
    pub truth: [f64; 32],
    pub a64: [f64; 32],
    pub rho: [f32; 32],              // the last stored norm (z units)
    pub rs: [f32; 32],               // rho / s, rounded up
    pub n2: [[f32; 32]; MAXB + 1],   // the single norm, then each block's, squared and rounded up
    pub nv: [[f32; 32]; MAXB + 1],   // and not squared
}

/// Per (query, cluster): the read order and the constants of each check.
#[derive(Default)]
pub struct ColQ {
    pub w: Vec<f64>,       // per coordinate (plan order)
    pub ord: Vec<u16>,     // coordinates in read order
    pub ck: Vec<usize>,    // coordinates read at each check
    pub dr: Vec<f32>,      // |(qv' d5)_{R + nonD}| at each check
    pub wb: Vec<[f32; MAXB]>, // |(w h)_{b - R}|
    pub wt: Vec<f32>,      // |(w h)_{D - R}|
    pub kap: Vec<[f32; MAXB]>, // kappa_b: the largest d5_c / (sigma_c h_c) read in block b
    pub kapt: Vec<f32>,        // over every block
    pub step: f64,         // table step
    // a stage's tables: per pair 32 bytes each of high, low and energy entries, the two rows and the block
    pub thi: Vec<[u8; 32]>,
    pub tlo: Vec<[u8; 32]>,
    pub ten: Vec<[u8; 32]>,
    pub prow: Vec<(u32, u32, u8)>,
    pub groups: Vec<(u8, usize, usize)>, // (block, first pair, end)
}

/// The order and the per-check constants. `qorder`: units by the query's gain sum w_c^2 (1 - D_c) per row, else the
/// fixed order.
pub fn col_setup(ix: &Index2, ci: usize, gq: &GQ2, cq: &CQ2, qorder: bool, o: &mut ColQ) {
    let c = &ix.cl[ci];
    let gp = &ix.groups[c.g];
    let cp = gp.col.as_ref().unwrap();
    let nu = cp.units.len();
    let a = cq.alr as f64;
    // weights; the stored mv32 is within 2^-24 |mv| of V^T m, which the slacks below and ewdet cover
    o.w.clear();
    let mut mvh = 0f64;
    for cc in &cp.coords {
        let k = cc.c as usize;
        let mv = c.mv32[k] as f64;
        o.w.push((gq.qv[k] - a * mv) * gp.sig[k] as f64);
        mvh += (mv * gp.sig[k] as f64 * cc.h).powi(2);
    }
    let mut amax = 0f64;
    for r in &cp.rows {
        let span: f64 = r.fields.iter().map(|f| {
            let m = (1usize << f.nb) - 1;
            let (lo, hi) = f.fv[..=m].iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)));
            o.w[f.k as usize].abs() * (hi - lo)
        }).sum();
        amax = amax.max(span);
    }
    o.step = (amax / 65535.0 * (1.0 + 1e-6)).max(1e-30);
    // order
    o.ord.clear();
    o.ord.extend(0..nu as u16);
    if qorder {
        let mut keys: Vec<u64> = o.ord.iter().map(|&u| {
            let un = &cp.units[u as usize];
            let g: f64 = if keyh() {
                (un.c0..un.c1).map(|k| (o.w[k as usize] * cp.coords[k as usize].h).powi(2)).sum::<f64>() / un.nrow as f64
            } else {
                (un.c0..un.c1).map(|k| o.w[k as usize].powi(2) * cp.coords[k as usize].cap).sum::<f64>() / un.nrow as f64
            };
            ((!(g as f32).to_bits()) as u64) << 16 | u as u64
        }).collect();
        keys.sort_unstable();
        for (x, kk) in o.ord.iter_mut().zip(&keys) {
            *x = (*kk & 0xffff) as u16;
        }
    }
    // checks: after the first units holding at least each listed number of rows, and after all
    o.ck.clear();
    let mut rows = 0usize;
    let cks = checkpoints();
    let mut ci_ = 0;
    for (pos, &u) in o.ord.iter().enumerate() {
        while ci_ < cks.len() && rows >= cks[ci_] {
            if o.ck.last() != Some(&pos) {
                o.ck.push(pos);
            }
            ci_ += 1;
        }
        rows += cp.units[u as usize].nrow as usize;
    }
    if o.ck.last() != Some(&nu) {
        o.ck.push(nu);
    }
    let sl_w = a.abs() * 2f64.powi(-23) * mvh.sqrt();
    let up = |x: f64| (x * (1.0 + 1e-6)) as f32;
    let nck = o.ck.len();
    o.dr.clear();
    o.wb.clear();
    o.wt.clear();
    o.dr.resize(nck, 0.0);
    o.wb.resize(nck, [0.0; MAXB]);
    o.wt.resize(nck, 0.0);
    // prefix of the read part (nonD always) and of kappa; suffix per block of the unread part
    let mut nond2 = 0f64;
    let mut mvd = 0f64;
    for &k in &cp.nond {
        let k = k as usize;
        let mv = c.mv32[k] as f64;
        let x = (gq.qv[k] - a * mv) * cp.d5[k];
        nond2 += x * x;
        mvd += (mv * cp.d5[k]).powi(2);
    }
    for cc in &cp.coords {
        mvd += (c.mv32[cc.c as usize] as f64 * cp.d5[cc.c as usize]).powi(2);
    }
    let sl_d = a.abs() * 2f64.powi(-23) * mvd.sqrt();
    o.kap.clear();
    o.kap.resize(nck, [0.0; MAXB]);
    o.kapt.clear();
    o.kapt.resize(nck, 0.0);
    let mut pre = nond2;
    let mut kb = [0f64; MAXB];
    let mut kc = 0;
    for pos in 0..=nu {
        while kc < nck && o.ck[kc] == pos {
            o.dr[kc] = up(pre.sqrt() * (1.0 + 1e-9) + sl_d);
            for b in 0..cp.nb {
                o.kap[kc][b] = up(kb[b]);
            }
            o.kapt[kc] = up(kb.iter().cloned().fold(0.0, f64::max));
            kc += 1;
        }
        if pos < nu {
            let un = &cp.units[o.ord[pos] as usize];
            for k in un.c0..un.c1 {
                let cc = &cp.coords[k as usize];
                let sg = gp.sig[cc.c as usize] as f64;
                let x = o.w[k as usize] / sg * cp.d5[cc.c as usize];
                pre += x * x;
                kb[cc.blk as usize] = kb[cc.blk as usize].max(cp.d5[cc.c as usize] / (sg * cc.h));
            }
        }
    }
    let mut suf = [0f64; MAXB];
    let mut kc = nck as isize - 1;
    for pos in (0..=nu).rev() {
        if pos < nu {
            let un = &cp.units[o.ord[pos] as usize];
            for k in un.c0..un.c1 {
                let cc = &cp.coords[k as usize];
                suf[cc.blk as usize] += (o.w[k as usize] * cc.h).powi(2);
            }
        }
        while kc >= 0 && o.ck[kc as usize] == pos {
            let mut tot = 0.0;
            for b in 0..cp.nb {
                o.wb[kc as usize][b] = if suf[b] > 0.0 { up(suf[b].sqrt() * (1.0 + 1e-9) + sl_w) } else { 0.0 };
                tot += suf[b];
            }
            o.wt[kc as usize] = if tot > 0.0 { up(tot.sqrt() * (1.0 + 1e-9) + sl_w) } else { 0.0 };
            kc -= 1;
        }
    }
}

/// The tables of the units ord[p0..p1], their rows paired by block. Returns (sum of the rows' minima, error bound, rows).
#[target_feature(enable = "avx2,fma")]
unsafe fn stage_tables(ix: &Index2, ci: usize, cp: &ColPlan, o: &mut ColQ, p0: usize, p1: usize) -> (f64, f64, usize) {
    let c = &ix.cl[ci];
    let cs = ix.col.as_ref().unwrap();
    let nblk = c.nblk;
    let base = cs.coff[ci] as u32;
    let mut rs: Vec<(u8, u16)> = Vec::with_capacity(2 * (p1 - p0));
    for &u in &o.ord[p0..p1] {
        let un = &cp.units[u as usize];
        for r in 0..un.nrow as u16 {
            rs.push((un.blk, un.row0 + r));
        }
    }
    rs.sort_unstable();
    o.thi.clear();
    o.tlo.clear();
    o.ten.clear();
    o.prow.clear();
    o.groups.clear();
    let (mut off, mut err) = (0f64, 0f64);
    let inv = 1.0 / o.step;
    let step = o.step;
    let w = &o.w;
    let tab = |r: u16, half: usize, thi: &mut [u8; 32], tlo: &mut [u8; 32], ten: &mut [u8; 32]| -> (f64, f64) {
        let row = &cp.rows[r as usize];
        // f32 sums of the fields' values: within (fields + 3) 2^-24 sum |w| max|value| of the f64 sum
        let (mut a0, mut a1) = (_mm256_setzero_ps(), _mm256_setzero_ps());
        let mut sab = 0f64;
        for (k, ex, mx) in &row.ex {
            let wk = w[*k as usize];
            let wv = _mm256_set1_ps(wk as f32);
            a0 = _mm256_fmadd_ps(wv, _mm256_loadu_ps(ex.as_ptr()), a0);
            a1 = _mm256_fmadd_ps(wv, _mm256_loadu_ps(ex.as_ptr().add(8)), a1);
            sab += wk.abs() * mx;
        }
        let mn = _mm256_min_ps(a0, a1);
        let mut mm = [0f32; 8];
        _mm256_storeu_ps(mm.as_mut_ptr(), mn);
        let m = mm.iter().cloned().fold(f32::INFINITY, f32::min);
        let iv = _mm256_set1_ps(inv as f32);
        let q = |a: __m256| _mm256_min_epi32(_mm256_max_epi32(_mm256_cvtps_epi32(_mm256_mul_ps(_mm256_sub_ps(a, _mm256_set1_ps(m)), iv)), _mm256_setzero_si256()), _mm256_set1_epi32(65535));
        let mut qq = [0i32; 16];
        _mm256_storeu_si256(qq.as_mut_ptr() as *mut _, q(a0));
        _mm256_storeu_si256(qq.as_mut_ptr().add(8) as *mut _, q(a1));
        for e in 0..16 {
            thi[16 * half + e] = (qq[e] >> 8) as u8;
            tlo[16 * half + e] = (qq[e] & 255) as u8;
            ten[16 * half + e] = row.en[e];
        }
        // rounding to the grid (half a step, and the f32 product with 1/step), the f32 sums
        // (the product (v - m) / step is within 2^-23 of at most 65535 steps: 0.008 of a step)
        let e = step * 0.51 + (row.ex.len() as f64 + 6.0) * 2f64.powi(-24) * sab + 1e-30;
        (m as f64, e)
    };
    let mut i = 0;
    while i < rs.len() {
        let b = rs[i].0;
        let g0 = o.prow.len();
        while i < rs.len() && rs[i].0 == b {
            let (mut h, mut l, mut e) = ([0u8; 32], [0u8; 32], [0u8; 32]);
            let ra = rs[i].1;
            let (m, er) = tab(ra, 0, &mut h, &mut l, &mut e);
            off += m;
            err += er;
            let rb = if i + 1 < rs.len() && rs[i + 1].0 == b {
                let rb = rs[i + 1].1;
                let (m, er) = tab(rb, 1, &mut h, &mut l, &mut e);
                off += m;
                err += er;
                i += 2;
                rb
            } else {
                i += 1;
                ra // second half: zero tables
            };
            o.thi.push(h);
            o.tlo.push(l);
            o.ten.push(e);
            o.prow.push((base + (ra as usize * nblk * 16) as u32, base + (rb as usize * nblk * 16) as u32, b));
        }
        o.groups.push((b, g0, o.prow.len()));
    }
    (off, err, rs.len())
}

#[inline(always)]
unsafe fn acc4(a: &mut [__m256i; 4], t: __m256i, lo: __m256i, hi: __m256i) {
    let r0 = _mm256_shuffle_epi8(t, lo);
    let r1 = _mm256_shuffle_epi8(t, hi);
    a[0] = _mm256_add_epi16(a[0], r0);
    a[1] = _mm256_add_epi16(a[1], _mm256_srli_epi16::<8>(r0));
    a[2] = _mm256_add_epi16(a[2], r1);
    a[3] = _mm256_add_epi16(a[3], _mm256_srli_epi16::<8>(r1));
}

/// Item-order sums (items 0..15, 16..31) as u16 from the raw accumulators.
#[inline(always)]
unsafe fn fin4(a: &[__m256i; 4]) -> (__m256i, __m256i) {
    let f = |x: __m256i| _mm_add_epi16(_mm256_castsi256_si128(x), _mm256_extracti128_si256::<1>(x));
    let o16 = |e: __m256i, o: __m256i| {
        let (e, o) = (f(e), f(o));
        _mm256_set_m128i(_mm_unpackhi_epi16(e, o), _mm_unpacklo_epi16(e, o))
    };
    (o16(_mm256_sub_epi16(a[0], _mm256_slli_epi16::<8>(a[1])), a[1]), o16(_mm256_sub_epi16(a[2], _mm256_slli_epi16::<8>(a[3])), a[3]))
}

#[inline(always)]
unsafe fn add_u16_to_i32(dst: &mut [i32; 32], x: (__m256i, __m256i), mul: i32) {
    for (h, v) in [x.0, x.1].into_iter().enumerate() {
        let lo = _mm256_cvtepu16_epi32(_mm256_castsi256_si128(v));
        let hi = _mm256_cvtepu16_epi32(_mm256_extracti128_si256::<1>(v));
        let m = _mm256_set1_epi32(mul);
        for (q, y) in [lo, hi].into_iter().enumerate() {
            let p = dst.as_mut_ptr().add(16 * h + 8 * q) as *mut __m256i;
            _mm256_storeu_si256(p, _mm256_add_epi32(_mm256_loadu_si256(p), _mm256_mullo_epi32(y, m)));
        }
    }
}

/// Accumulate a stage's pairs over one slot (pairs of at most 120 between finalisations).
#[target_feature(enable = "avx2,fma")]
unsafe fn stage_acc(code: *const u8, o: &ColQ, sl: &mut ColSlot, ne: bool) {
    let m0f = _mm256_set1_epi8(0x0f);
    let z = _mm256_setzero_si256();
    let bo = sl.bi as usize * 16;
    let mut ah = [z; 4];
    let mut al = [z; 4];
    let mut npe = 0;
    for &(b, g0, g1) in &o.groups {
        let mut ae = [z; 4];
        let mut npg = 0;
        for p in g0..g1 {
            let (ra, rb, _) = o.prow[p];
            let c = _mm256_set_m128i(_mm_loadu_si128(code.add(rb as usize + bo) as *const _), _mm_loadu_si128(code.add(ra as usize + bo) as *const _));
            let lo = _mm256_and_si256(c, m0f);
            let hi = _mm256_and_si256(_mm256_srli_epi16::<4>(c), m0f);
            acc4(&mut ah, _mm256_loadu_si256(o.thi[p].as_ptr() as *const _), lo, hi);
            acc4(&mut al, _mm256_loadu_si256(o.tlo[p].as_ptr() as *const _), lo, hi);
            if !ne {
                acc4(&mut ae, _mm256_loadu_si256(o.ten[p].as_ptr() as *const _), lo, hi);
            }
            npe += 1;
            npg += 1;
            if npe == 120 {
                add_u16_to_i32(&mut sl.th, fin4(&ah), 256);
                add_u16_to_i32(&mut sl.th, fin4(&al), 1);
                ah = [z; 4];
                al = [z; 4];
                npe = 0;
            }
            if npg == 120 && !ne {
                add_u16_to_i32(&mut sl.en[b as usize], fin4(&ae), 1);
                ae = [z; 4];
                npg = 0;
            }
        }
        if npg > 0 && !ne {
            add_u16_to_i32(&mut sl.en[b as usize], fin4(&ae), 1);
        }
    }
    if npe > 0 {
        add_u16_to_i32(&mut sl.th, fin4(&ah), 256);
        add_u16_to_i32(&mut sl.th, fin4(&al), 1);
    }
}

/// The per-cluster constants of a check.
struct Chk {
    qm: f32,
    alr: f32,
    off: f32,
    offa: f32,
    step: f32,
    hb: f32,  // fixed + slack-free constant part of the half-width (fixed)
    se: f32,  // per unit s: ewdet + table error
    dr: f32,
    wt: f32,
    wb: [f32; MAXB],
    kap: [f32; MAXB],
    kapt: f32,
    tin: f32,
}

/// Decide the live lanes of a slot at one check. Returns (in, out) masks; sets nread for lanes that read their block
/// norms. `rho5` the decoded last stored norm per lane (z units).
#[target_feature(enable = "avx2,fma")]
unsafe fn check_slot(cp: &ColPlan, sl: &mut ColSlot, k: &Chk, lazy: bool, verify: bool, t: f64, st: &mut Stats, kidx: usize) -> (u32, u32) {
    let (mut inm, mut outm) = (0u32, 0u32);
    let u32x = _mm256_set1_ps((32.0 * U) as f32);
    let one = |x: f32| _mm256_set1_ps(x);
    let gscale = (1.0 + 2f32.powi(-20), 1.0 - 2f32.powi(-20));
    let nb = cp.nb;
    for g in 0..4 {
        let l8 = (sl.live >> (8 * g)) & 255;
        if l8 == 0 {
            continue;
        }
        let o = 8 * g;
        let s = _mm256_loadu_ps(sl.s.as_ptr().add(o));
        let mr = _mm256_loadu_ps(sl.mr.as_ptr().add(o));
        let th = _mm256_cvtepi32_ps(_mm256_loadu_si256(sl.th.as_ptr().add(o) as *const _));
        let qa = _mm256_fmadd_ps(one(k.alr), mr, one(k.qm));
        let ep = _mm256_fmadd_ps(one(k.step), th, one(k.off));
        let est = _mm256_fmadd_ps(s, ep, qa);
        let abs = |x: __m256| _mm256_andnot_ps(one(-0.0), x);
        let fl = _mm256_mul_ps(u32x, _mm256_add_ps(abs(qa), _mm256_mul_ps(s, _mm256_fmadd_ps(one(k.step), th, one(k.offa)))));
        let rho = _mm256_loadu_ps(sl.rho.as_ptr().add(o));
        // the half-width without the unread part
        let hb = _mm256_add_ps(_mm256_add_ps(one(k.hb), fl), _mm256_fmadd_ps(s, one(k.se), _mm256_mul_ps(_mm256_mul_ps(one(k.dr), rho), one(1.0 + 1e-6))));
        let ldn = |q: usize| _mm256_loadu_ps(sl.n2[q].as_ptr().add(o));
        let enb = |b: usize| _mm256_mul_ps(_mm256_cvtepi32_ps(_mm256_loadu_si256(sl.en[b].as_ptr().add(o) as *const _)), one((cp.estep[b] as f32) * gscale.1));
        // rho5 / s, rounded up
        let rs = _mm256_loadu_ps(sl.rs.as_ptr().add(o));
        // the energy read, from the code values less what rho5 allows: (sqrt(E) - kappa rho5 / s)^2, rounded down
        let ne = noen();
        let rd = |e: __m256, kp: f32| {
            if ne {
                return _mm256_setzero_ps();
            }
            let x = _mm256_max_ps(_mm256_setzero_ps(), _mm256_fnmadd_ps(one(kp), rs, _mm256_mul_ps(_mm256_sqrt_ps(e), one(1.0 - 1e-6))));
            _mm256_mul_ps(_mm256_mul_ps(x, x), one(1.0 - 1e-6))
        };
        let unb = || -> __m256 {
            let mut u = _mm256_setzero_ps();
            for b in 0..nb {
                if k.wb[b] == 0.0 {
                    continue;
                }
                let r = _mm256_max_ps(_mm256_setzero_ps(), _mm256_sub_ps(ldn(1 + b), rd(enb(b), k.kap[b])));
                u = _mm256_fmadd_ps(one(k.wb[b]), _mm256_sqrt_ps(r), u);
            }
            u
        };
        let decide = |u: __m256| -> (u32, u32, __m256, __m256) {
            let hw = _mm256_fmadd_ps(_mm256_mul_ps(s, u), one(1.0 + 1e-5), hb);
            let lb = _mm256_sub_ps(est, hw);
            let ub = _mm256_add_ps(est, hw);
            let i = _mm256_movemask_ps(_mm256_cmp_ps::<_CMP_GE_OQ>(lb, one(k.tin))) as u32;
            let ot = _mm256_movemask_ps(_mm256_cmp_ps::<_CMP_LT_OQ>(ub, one(k.tin))) as u32;
            (i & l8, ot & l8, lb, ub)
        };
        let nr8 = (sl.nread >> o) & 255 & l8;
        let (mut i8, mut o8, mut lb, mut ub);
        if ne {
            // linear in the stored norms: s (wt N or sum_b wb N_b)
            let lin = |q: usize| _mm256_loadu_ps(sl.nv[q].as_ptr().add(o));
            let ub_ = || {
                let mut u = _mm256_setzero_ps();
                for b in 0..nb {
                    if k.wb[b] != 0.0 {
                        u = _mm256_fmadd_ps(one(k.wb[b]), lin(1 + b), u);
                    }
                }
                u
            };
            (i8, o8, lb, ub) = decide(if k.wt == 0.0 { _mm256_setzero_ps() } else if lazy { _mm256_mul_ps(one(k.wt), lin(0)) } else { ub_() });
            if lazy && k.wt != 0.0 {
                let need = (l8 & !(i8 | o8)) | nr8;
                if need != 0 {
                    let u1 = _mm256_mul_ps(one(k.wt), lin(0));
                    let u = _mm256_blendv_ps(u1, _mm256_min_ps(u1, ub_()), mask8_ps(need));
                    (i8, o8, lb, ub) = decide(u);
                    let newly = need & !nr8;
                    st.col_nread += newly.count_ones() as u64;
                    st.col_bytes += (newly.count_ones() as usize * nb) as f64;
                    sl.nread |= newly << o;
                }
            }
        } else if k.wt == 0.0 {
            (i8, o8, lb, ub) = decide(_mm256_setzero_ps());
        } else if lazy {
            let mut et = _mm256_setzero_ps();
            for b in 0..nb {
                et = _mm256_add_ps(et, enb(b));
            }
            let u1 = _mm256_mul_ps(one(k.wt), _mm256_sqrt_ps(_mm256_max_ps(_mm256_setzero_ps(), _mm256_sub_ps(ldn(0), rd(et, k.kapt)))));
            (i8, o8, lb, ub) = decide(u1);
            let need = (l8 & !(i8 | o8)) | nr8;
            if need != 0 {
                let u2 = unb();
                let nm = mask8_ps(need);
                let u = _mm256_blendv_ps(u1, _mm256_min_ps(u1, u2), nm);
                (i8, o8, lb, ub) = decide(u);
                let newly = need & !nr8;
                st.col_nread += newly.count_ones() as u64;
                st.col_bytes += (newly.count_ones() as usize * nb) as f64;
                sl.nread |= newly << o;
            }
        } else {
            (i8, o8, lb, ub) = decide(unb());
        }
        if dbg() {
            let ar = |x: __m256| { let mut a = [0f32; 8]; _mm256_storeu_ps(a.as_mut_ptr(), x); a };
            let (h0, rr, sv, es, lbv) = (ar(_mm256_add_ps(_mm256_add_ps(one(k.hb), fl), _mm256_mul_ps(s, one(k.se)))), ar(_mm256_mul_ps(one(k.dr), rho)), ar(s), ar(est), ar(lb));
            let hk = st.col_hw.len();
            if hk <= kidx { st.col_hw.resize(kidx + 1, [0.0; 5]); }
            let _ = hk;
            for i in 0..8 {
                if l8 >> i & 1 == 0 { continue; }
                let hw = es[i] - lbv[i];
                let x = &mut st.col_hw[kidx];
                x[0] += 1.0;
                x[1] += h0[i] as f64;
                x[2] += rr[i] as f64;
                x[3] += (hw - h0[i] - rr[i]) as f64;
                x[4] += (es[i] as f64 - t).abs();
                let mut et = 0.0;
                for b in 0..nb { et += sl.en[b][o + i] as f64 * cp.estep[b]; }
                x[1] += et / sl.n2[0][o + i] as f64;
                let _ = sv;
            }
        }
        if verify {
            let (mut lbs, mut ubs) = ([0f32; 8], [0f32; 8]);
            _mm256_storeu_ps(lbs.as_mut_ptr(), lb);
            _mm256_storeu_ps(ubs.as_mut_ptr(), ub);
            for i in 0..8 {
                if l8 >> i & 1 == 0 {
                    continue;
                }
                let tr = sl.truth[o + i];
                st.checked += 1;
                if tr < lbs[i] as f64 - 1e-12 || tr > ubs[i] as f64 + 1e-12 {
                    st.viol_det += 1;
                }
                if (i8 >> i & 1 == 1 && tr < t) || (o8 >> i & 1 == 1 && tr >= t) {
                    st.imp_fail += 1;
                }
                st.imp_checked += ((i8 | o8) >> i & 1) as u64;
            }
        }
        inm |= i8 << o;
        outm |= o8 << o;
    }
    (inm, outm)
}

/// No energy accounting unless `VEXACT_COLEN` is set: the unread part is bounded by the stored norms alone (the
/// energy read changes the bytes read by under 0.1% at 50k, and costs a third of the decode and the square roots).
pub fn noen() -> bool {
    static D: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *D.get_or_init(|| std::env::var("VEXACT_COLEN").is_err())
}

/// The query order's key: |w h|^2 per row (the unread bound's weights; `VEXACT_COLKEY=h`), else w^2 (1 - D) per row.
pub fn keyh() -> bool {
    static D: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *D.get_or_init(|| std::env::var("VEXACT_COLKEY").map_or(false, |v| v == "h"))
}

pub fn dbg() -> bool {
    static D: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *D.get_or_init(|| std::env::var("VEXACT_COLDBG").is_ok())
}

#[inline(always)]
unsafe fn mask8_ps(m: u32) -> __m256 {
    let bits = _mm256_setr_epi32(1, 2, 4, 8, 16, 32, 64, 128);
    _mm256_castsi256_ps(_mm256_cmpeq_epi32(_mm256_and_si256(_mm256_set1_epi32(m as i32), bits), bits))
}

/// The threshold query's deterministic part of one cluster on the columnar store, then the randomised levels for the
/// items it leaves undecided (run_cluster_from).
#[target_feature(enable = "avx2,fma")]
pub unsafe fn run_col(ix: &Index2, ci: usize, pr: &Prep2, gq: &GQ2, cq: &mut CQ2, cands: &[u32], t: f64, qorder: bool, lazy: bool, verify: bool, sv: &mut State, st: &mut Stats, out: &mut RunOut) {
    let c = &ix.cl[ci];
    let gp = &ix.groups[c.g];
    let cp = gp.col.as_ref().unwrap();
    let cs = ix.col.as_ref().unwrap();
    let n = ix.n();
    let t0 = tick();
    let mut o = std::mem::take(&mut sv.colq);
    col_setup(ix, ci, gq, cq, qorder, &mut o);
    let t1 = tick();
    st.cyc[P_SEL] += t1 - t0;
    let nrm = cs.nrm.as_ptr().add(cs.noff[ci]);
    let nstride = c.nblk * 32;
    let rho_base = ix.lscal[NDL - 1].as_ptr().add(ix.soff[NDL - 1][ci]);
    let pc5 = gp.pconst[NDL - 1];
    let ntab5 = gp.ntab[NDL - 1];
    // slots: the blocks holding candidates
    let mut slots = std::mem::take(&mut sv.cslots);
    let mut ns = 0;
    {
        let mut i = 0;
        while i < cands.len() {
            let bi = cands[i] / 32;
            let mut live = 0u32;
            while i < cands.len() && cands[i] / 32 == bi {
                live |= 1 << (cands[i] % 32);
                i += 1;
            }
            if ns == slots.len() {
                slots.push(ColSlot { bi: 0, live: 0, nread: 0, th: [0; 32], en: [[0; 32]; MAXB], s: [0.0; 32], mr: [0.0; 32], truth: [0.0; 32], a64: [0.0; 32], rho: [0.0; 32], rs: [0.0; 32], n2: [[0.0; 32]; MAXB + 1], nv: [[0.0; 32]; MAXB + 1] });
            }
            let sl = &mut slots[ns];
            sl.bi = bi;
            sl.live = live;
            sl.nread = 0;
            sl.th = [0; 32];
            if !noen() {
                for b in 0..cp.nb {
                    sl.en[b] = [0; 32];
                }
            }
            // the encoder's f32 item scale, m.r from the key, the last stored norm (pconst s first, as the encoder
            // checked it) and the stored norms, 8 lanes at a time
            let rrow = _mm_loadu_si128(rho_base.add(bi as usize * 16) as *const _);
            let one = |x: f32| _mm256_set1_ps(x);
            let tlo = _mm256_loadu_ps(ntab5.as_ptr());
            let thi = _mm256_loadu_ps(ntab5.as_ptr().add(8));
            for g in 0..4 {
                if (live >> (8 * g)) & 255 == 0 {
                    continue;
                }
                let o = 8 * g;
                let ii = bi as usize * 32 + o;
                let ah = _mm256_cvtepi16_epi32(_mm_loadu_si128(ix.ahat.as_ptr().add(c.r0 + ii) as *const _));
                let a = _mm256_div_ps(_mm256_cvtepi32_ps(ah), one(32767.0));
                let s2 = _mm256_sub_ps(one(1.0f32 + c.mn32 * c.mn32), _mm256_mul_ps(one(2.0f32 * c.mn32), a));
                let sc = _mm256_sqrt_ps(_mm256_div_ps(_mm256_max_ps(s2, one(1e-12)), one(gp.lamsum)));
                _mm256_storeu_ps(sl.s.as_mut_ptr().add(o), sc);
                _mm256_storeu_ps(sl.mr.as_mut_ptr().add(o), _mm256_sub_ps(_mm256_mul_ps(one(c.mn32), a), one(c.mn32 * c.mn32)));
                let bytes = if g < 2 { _mm_srli_si128::<0>(rrow) } else { rrow };
                let b8 = _mm256_cvtepu8_epi32(if g % 2 == 0 { bytes } else { _mm_srli_si128::<8>(bytes) });
                let code = if g < 2 { _mm256_and_si256(b8, _mm256_set1_epi32(15)) } else { _mm256_srli_epi32::<4>(b8) };
                let tv = _mm256_blendv_ps(_mm256_permutevar8x32_ps(tlo, code), _mm256_permutevar8x32_ps(thi, code), _mm256_castsi256_ps(_mm256_cmpgt_epi32(code, _mm256_set1_epi32(7))));
                let r = _mm256_mul_ps(_mm256_mul_ps(one(pc5), sc), tv);
                _mm256_storeu_ps(sl.rho.as_mut_ptr().add(o), r);
                let esc = _mm256_movemask_ps(_mm256_castsi256_ps(_mm256_cmpeq_epi32(code, _mm256_set1_epi32(15)))) as u32 & (live >> o);
                if esc != 0 {
                    for i in 0..8 {
                        if esc >> i & 1 == 1 {
                            sl.rho[o + i] = ix.esc[&((NDL - 1) as u8, 0u8, (c.r0 + ii + i) as u32)];
                        }
                    }
                }
                let rr = _mm256_loadu_ps(sl.rho.as_ptr().add(o));
                _mm256_storeu_ps(sl.rs.as_mut_ptr().add(o), _mm256_mul_ps(_mm256_div_ps(rr, sc), one(1.0 + 1e-6)));
                for q in 0..=cp.nb {
                    let codes = _mm256_cvtepu8_epi32(_mm_loadl_epi64(nrm.add(q * nstride + ii) as *const _));
                    _mm256_storeu_ps(sl.nv[q].as_mut_ptr().add(o), _mm256_i32gather_ps::<4>(cp.nuv[q].as_ptr(), codes));
                    _mm256_storeu_ps(sl.n2[q].as_mut_ptr().add(o), _mm256_mul_ps(_mm256_i32gather_ps::<4>(cp.nu2[q].as_ptr(), codes), one(1.0 + 2f32.powi(-20))));
                }
            }
            if verify {
                for lane in 0..32 {
                    let ii = bi as usize * 32 + lane;
                    if live >> lane & 1 == 0 {
                        continue;
                    }
                    assert_eq!(sl.s[lane], crate::index::item_scale(ix.ahat[c.r0 + ii], c.mn32, gp.lamsum));
                    let mut x = vec![0f64; n];
                    ix.full_row_f64(c.r0 + ii, &mut x);
                    sl.truth[lane] = x.iter().zip(&pr.q64).map(|(a, b)| a * b).sum();
                    sl.a64[lane] = 0.0;
                }
            }
            ns += 1;
        }
    }
    let ncand = cands.len();
    let mut lv: Vec<u32> = (0..ns as u32).collect();
    st.cyc[P_INIT] += tick() - t1;
    let t32 = t as f32;
    let tin = crate::query::next_up_if_below(t32, t);
    let code = cs.code.as_ptr();
    let (mut off, mut err) = (0f64, 0f64);
    // bytes: the key, the last stored norm, and the single norm or every block norm
    st.col_bytes += ncand as f64 * (2.0 + 0.5 + if lazy { 1.0 } else { cp.nb as f64 });
    if !lazy {
        st.col_nread += ncand as u64;
    }
    let nck = o.ck.len();
    if st.col_enter.len() < nck {
        st.col_enter.resize(nck, 0);
        st.col_dec.resize(nck, 0);
    }
    let ne = noen();
    let mut prevpos = 0;
    for kk in 0..nck {
        if ns == 0 {
            break;
        }
        let pos = o.ck[kk];
        if pos > prevpos {
            let ta = tick();
            let (m, e, nrows) = stage_tables(ix, ci, cp, &mut o, prevpos, pos);
            off += m;
            err += e;
            let tb = tick();
            st.cyc[P_TABLES] += tb - ta;
            let mut live_items = 0;
            for &si in &lv[..ns] {
                let sl = &mut slots[si as usize];
                stage_acc(code, &o, sl, ne);
                live_items += sl.live.count_ones() as usize;
                if verify {
                    for lane in 0..32 {
                        if sl.live >> lane & 1 == 0 {
                            continue;
                        }
                        for &u in &o.ord[prevpos..pos] {
                            let un = &cp.units[u as usize];
                            for r in un.row0 as usize..un.row0 as usize + un.nrow as usize {
                                let p = code.add(cs.coff[ci] + r * c.nblk * 16 + sl.bi as usize * 16 + lane % 16);
                                let e = if lane < 16 { *p & 15 } else { *p >> 4 };
                                sl.a64[lane] += cp.row_value(r, e as usize, &o.w);
                            }
                        }
                    }
                }
            }
            st.col_bytes += live_items as f64 * nrows as f64 * 0.5;
            st.cyc[P_DECODE] += tick() - tb;
        }
        prevpos = pos;
        let tc = tick();
        let k = Chk {
            qm: cq.qm,
            alr: cq.alr,
            off: off as f32,
            offa: off.abs() as f32,
            step: o.step as f32,
            hb: cq.fixed,
            se: cq.ewdet + (err * (1.0 + 1e-6)) as f32,
            dr: o.dr[kk],
            wt: o.wt[kk],
            wb: o.wb[kk],
            kap: o.kap[kk],
            kapt: o.kapt[kk],
            tin,
        };
        let mut w = 0;
        for si in 0..ns {
            let sl = &mut slots[lv[si] as usize];
            st.col_enter[kk] += sl.live.count_ones() as u64;
            if verify {
                for lane in 0..32 {
                    if sl.live >> lane & 1 == 1 {
                        let ep = off + o.step * sl.th[lane] as f64;
                        st.imp_checked += 1;
                        if (ep - sl.a64[lane]).abs() > err * (1.0 + 1e-6) + 1e-9 * (1.0 + sl.a64[lane].abs()) {
                            st.cert_viol += 1;
                        }
                    }
                }
            }
            let (inm, outm) = check_slot(cp, sl, &k, lazy, verify, t, st, kk);
            st.col_dec[kk] += (inm | outm).count_ones() as u64;
            if inm != 0 {
                for lane in 0..32 {
                    if inm >> lane & 1 == 1 {
                        out.rows_in.push((c.r0 + sl.bi as usize * 32 + lane) as u32);
                    }
                }
            }
            sl.live &= !(inm | outm);
            if sl.live != 0 {
                lv[w] = lv[si];
                w += 1;
            }
        }
        ns = w;
        st.cyc[P_BOUNDS] += tick() - tc;
    }
    // survivors: into the level walk's state, at the first randomised level
    let mut nsurv = 0;
    for sl in lv[..ns].iter().map(|&i| &slots[i as usize]) {
        nsurv += sl.live.count_ones() as usize;
    }
    if nsurv > 0 {
        let ti = tick();
        // held in place (holes) while occupancy is high, as the level walk would, else compacted
        let (b0, b1) = (lv[..ns].iter().map(|&i| &slots[i as usize]).map(|x| x.bi).min().unwrap(), lv[..ns].iter().map(|&i| &slots[i as usize]).map(|x| x.bi).max().unwrap());
        let span = (b1 - b0 + 1) as usize * 32;
        let occ = if cq.repack > 0.0 { (ix.repack[NDL] * cq.repack).min(0.95) } else { 0.6 };
        let inplace = nsurv as f32 >= span as f32 * occ;
        let cap = if inplace { span } else { nsurv };
        sv.reset(cap);
        if inplace {
            sv.idx[..span].fill(u32::MAX);
        }
        let mut k = 0;
        for sl in lv[..ns].iter().map(|&i| &slots[i as usize]) {
            if inplace {
                let k0 = (sl.bi - b0) as usize * 32;
                sv.s[k0..k0 + 32].copy_from_slice(&sl.s);
                sv.mr[k0..k0 + 32].copy_from_slice(&sl.mr);
                for g in 0..4 {
                    let th = _mm256_cvtepi32_ps(_mm256_loadu_si256(sl.th.as_ptr().add(8 * g) as *const _));
                    // f32 of off + step theta: within the slack added to edet below
                    _mm256_storeu_ps(sv.adet.as_mut_ptr().add(k0 + 8 * g), _mm256_fmadd_ps(_mm256_set1_ps(o.step as f32), th, _mm256_set1_ps(off as f32)));
                }
                for lane in 0..32 {
                    if sl.live >> lane & 1 == 1 {
                        sv.idx[k0 + lane] = sl.bi * 32 + lane as u32;
                        if verify {
                            sv.truth[k0 + lane] = sl.truth[lane];
                        }
                    }
                }
                continue;
            }
            for lane in 0..32 {
                if sl.live >> lane & 1 == 0 {
                    continue;
                }
                sv.idx[k] = sl.bi * 32 + lane as u32;
                sv.s[k] = sl.s[lane];
                sv.mr[k] = sl.mr[lane];
                sv.adet[k] = (off + o.step * sl.th[lane] as f64) as f32;
                if verify {
                    sv.truth[k] = sl.truth[lane];
                }
                k += 1;
            }
        }
        for kk in cap..(cap + BLK).min(sv.idx.len()) {
            sv.idx[kk] = u32::MAX;
        }
        // adet as f32: within its f64 value's rounding
        let amax = lv[..ns].iter().map(|&i| &slots[i as usize]).flat_map(|sl| sl.th.iter()).map(|&x| x as f64).fold(0.0, f64::max);
        // adet in f32 (an fma of f32-rounded step and off on an f32 theta): within 4 2^-24 (|off| + step amax)
        // of off + step theta, beyond err
        let edet = ((err + (off.abs() + o.step * amax) * 2f64.powi(-21)) * (1.0 + 1e-6)) as f32;
        st.cyc[P_INIT] += tick() - ti;
        sv.colq = o;
        sv.cslots = slots;
        run_cluster_from(ix, ci, pr, gq, cq, &[], t, Mode2::Thr, 0, verify, sv, st, out, Some((cap, edet, if inplace { Some(b0 * 32) } else { None })));
    } else {
        sv.colq = o;
        sv.cslots = slots;
    }
}
