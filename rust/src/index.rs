//! The stored index: build parameters (learned in numpy, loaded), per-group code plans, the per-item encoder (here, in
//! Rust), and the stored layout: structure-of-arrays per level, bit-planes over blocks of 16 items within a cluster.
use crate::util::*;
use rayon::prelude::*;
use std::collections::HashMap;

pub const NDL: usize = 6; // deterministic levels 4, 8, 12, 16, 24, 32 B
pub const J: usize = 4; // randomised blocks (n = 768: 96 / 128 / 256 / 288)
pub const OFF: [i32; 15] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 18];
pub const DA: f64 = 0.5 / 32767.0 + 1e-7;
pub const SEED: u64 = 20261112;
pub const LANES: usize = 16;

pub fn ry() -> f64 {
    2f64.powf(1.0 / 256.0)
}
pub fn rho_y() -> f64 {
    2f64.powf(1.0 / 4096.0)
}
pub fn rho_t() -> f64 {
    2f64.powf(1.0 / 20.0)
}

/// The item scale from the stored sort key; build and query compute it with the same f32 operations.
#[inline]
pub fn item_scale(ah: i16, mn: f32, lamsum: f32) -> f32 {
    let a = ah as f32 / 32767.0f32;
    let s2 = (1.0f32 + mn * mn) - 2.0f32 * mn * a;
    (s2.max(1e-12f32) / lamsum).sqrt()
}

pub struct Meta {
    pub n: usize,
    pub k: usize,
    pub g: usize,
    pub dlv: Vec<usize>,
    pub rlv: Vec<usize>,
    pub bounds: Vec<usize>,
    pub csg: f64,
}
impl Meta {
    pub fn nl(&self) -> usize {
        NDL + self.rlv.len()
    }
    pub fn level_bytes(&self, l: usize) -> usize {
        if l < NDL { self.dlv[l] } else { self.rlv[l - NDL] }
    }
}

/// Exported numpy build parameters.
pub struct Params {
    pub meta: Meta,
    pub cstart: Vec<usize>,
    pub grp: Vec<usize>,
    pub centres: Vec<f32>, // K x n, the stored 8-bit centres (int8 * scale) as f32 values
    pub lam: Vec<f64>,
    pub v: Vec<f32>, // G x n x n, z = V^T r
    pub frb: Vec<u8>, // G x 6 x n cumulative bits per coordinate
    pub dl: Vec<f64>, // G x 6 x n
    pub dt: Vec<f64>, // G x n
    pub ridx: Vec<i32>, // G x n
    pub rates: Vec<f64>, // G x NR x J
    pub tree_edges: Vec<f64>, // 9 x 256
    pub tree_cent: Vec<f64>,
    pub gtree: Vec<f64>,
    pub perm: Vec<i64>,
    pub qi: Vec<i64>,
    pub text_q: Vec<f32>,
    pub xs_path: String,
}

pub fn load_params(dir: &str) -> Params {
    let meta: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(format!("{dir}/meta.json")).unwrap()).unwrap();
    let us = |k: &str| meta[k].as_u64().unwrap() as usize;
    let vu = |k: &str| meta[k].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize).collect::<Vec<_>>();
    let r = |k: &str| read_npy(&format!("{dir}/{k}.npy"));
    let mq8 = r("mq8").i8s();
    let msc = r("msc").f64s();
    let n = us("n");
    let k = us("K");
    let centres = (0..k * n).map(|i| (mq8[i] as f64 * msc[i / n]) as f32).collect();
    Params {
        meta: Meta { n, k, g: us("G"), dlv: vu("DLV"), rlv: vu("RLV"), bounds: vu("BOUNDS"), csg: meta["CSG"].as_f64().unwrap() },
        cstart: r("cstart").i64s().iter().map(|&x| x as usize).collect(),
        grp: r("grp").i64s().iter().map(|&x| x as usize).collect(),
        centres,
        lam: r("lam").f64s(),
        v: r("V").f32s(),
        frb: r("frb").u8s(),
        dl: r("dl").f64s(),
        dt: r("dt").f64s(),
        ridx: r("ridx").i32s(),
        rates: r("rates").f64s(),
        tree_edges: r("tree_edges").f64s(),
        tree_cent: r("tree_cent").f64s(),
        gtree: r("gtree").f64s(),
        perm: r("perm").i64s(),
        qi: r("qi").i64s(),
        text_q: r("text_q").f32s(),
        xs_path: meta["xs"].as_str().unwrap().to_string(),
    }
}

/// Replace the exported profile-dependent parameters by ones derived from a profile file (old tree code).
/// Returns how many entries differ from the exported ones (frb, rates), for checking the port.
pub fn apply_profile(p: &mut Params, prof_path: &str) -> (usize, usize) {
    let prof = crate::derive::load_profile(prof_path);
    let m = &p.meta;
    let tof = crate::derive::tof_tree(&p.gtree);
    let d = crate::derive::derive(&p.lam, &prof, m.n, m.g, &m.dlv, &m.rlv, &m.bounds, m.csg, &p.gtree, &tof, 1);
    let dfrb = d.frb.iter().zip(&p.frb).filter(|(a, b)| a != b).count();
    let drates = d.rates.iter().zip(&p.rates).filter(|(a, b)| a != b).count();
    p.frb = d.frb;
    p.dl = d.dl;
    p.dt = d.dt;
    p.ridx = d.ridx;
    p.rates = d.rates;
    (dfrb, drates)
}

/// Nested tree quantiser tables. dtab[bo*9+bn][idx] = cent_bn[idx] - cent_bo[idx >> (bn-bo)] (cent_0 = 0), f32;
/// dsq the same for squared centroids. The reconstruction of a coordinate is defined as the sum of the f32 steps.
pub struct Tree {
    pub edges: Vec<f64>,
    pub cent: Vec<f64>,
    pub g: Vec<f64>,
    pub dtab: Vec<[f32; 256]>,
    pub dsq: Vec<[f32; 256]>,
    pub dmax: Vec<f32>, // max |dtab| per transition
}
impl Tree {
    pub fn new(p: &Params) -> Tree {
        let mut dtab = vec![[0f32; 256]; 81];
        let mut dsq = vec![[0f32; 256]; 81];
        let mut dmax = vec![0f32; 81];
        let cent = |b: usize, i: usize| if b == 0 { 0.0 } else { p.tree_cent[b * 256 + i] };
        for bo in 0..9 {
            for bn in bo + 1..9 {
                for i in 0..(1usize << bn) {
                    let c1 = cent(bn, i);
                    let c0 = cent(bo, i >> (bn - bo));
                    dtab[bo * 9 + bn][i] = (c1 - c0) as f32;
                    dsq[bo * 9 + bn][i] = (c1 * c1 - c0 * c0) as f32;
                    dmax[bo * 9 + bn] = dmax[bo * 9 + bn].max(((c1 - c0) as f32).abs());
                }
            }
        }
        Tree { edges: p.tree_edges.clone(), cent: p.tree_cent.clone(), g: p.gtree.clone(), dtab, dsq, dmax }
    }
    /// cell index at depth b (searchsorted, side left: number of edges strictly below v)
    #[inline]
    pub fn index(&self, b: usize, v: f64) -> usize {
        let e = &self.edges[b * 256..b * 256 + (1 << b) - 1];
        e.partition_point(|&x| x < v)
    }
}

/// Ops of one level and part: fresh signs (depth 0 -> 1, one plane in this level; step is +-t1, independent of
/// earlier bits) and multi-bit steps (table lookup on the full cell index).
#[derive(Clone, Default, Debug)]
pub struct LvOps {
    pub signs: Vec<u16>,
    pub sign_coord: Vec<u16>,
    pub sign_slot: Vec<u16>, // per-block index-state slot, or NOSLOT if the coordinate is never refined
    pub multi: Vec<Op>,
    pub mops: Vec<MOp>,
    // kernel lists, weights in this order: ss, sp, m1, mg
    pub ss: Vec<(u16, u16)>, // fresh sign that will be refined: (plane, slot)
    pub sp: Vec<u16>,        // fresh sign never refined: plane
    pub m1: Vec<MOp>,        // one new bit, depth <= 4
    pub mg: Vec<MOp>,        // the rest
    pub wcoord: Vec<u16>,    // coordinate of each weight
    pub nsign: usize,
    pub mdmax: Vec<f32>,     // max |step| of each multi in weight order
}

pub const NOSLOT: u16 = u16::MAX;
pub const MAXSLOTS: usize = 1024;

/// A refinement step: the cell index (kept per block as state) gains bnew - bold bits from consecutive planes of the
/// current level, then one table lookup.
#[derive(Clone, Copy, Debug)]
pub struct MOp {
    pub pl: u16,
    pub slot: u16,
    pub bold: u8,
    pub bnew: u8,
    pub store: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Op {
    pub coord: u16, // det: coordinate in the local basis; rand: position in the rotated blocks
    pub bold: u8,
    pub bnew: u8,
    pub lv: [u8; 8], // level of each plane, depth 0 (top bit) first
    pub pl: [u16; 8],
}

pub struct GroupPlan {
    pub sig: Vec<f32>,
    pub lamsum: f32,
    pub d: Vec<Vec<f32>>, // NDL x n
    pub pconst: [f32; NDL],
    pub kmin: [i32; NDL],
    pub ntab: [[f32; 16]; NDL],
    pub dt: Vec<f32>,
    pub ridx: Vec<u16>, // position -> coordinate
    pub nb: [usize; J],
    pub boff: [usize; J + 1],
    pub ypredw: [f32; J],
    pub yk0: [i32; J],
    pub ytab: Vec<[f32; 256]>, // J
    pub rates: Vec<[f64; J]>,
    pub rmax: [usize; J],
    pub thchg: Vec<Vec<usize>>, // per rand level: blocks whose rate rose (th stored)
    pub tk0: Vec<[i32; J]>,
    pub ttab: Vec<[[f32; 16]; J]>,
    pub ops: Vec<Vec<Op>>, // per level
    pub lops: Vec<Vec<LvOps>>, // per level, per part (det: 1 part; rand: one per block)
    pub woff: Vec<Vec<usize>>, // offset of each part's weights in the op-ordered weight vector
    pub nw: usize,
    pub inv_ms: [f32; J],
    pub nsc: Vec<[f32; J]>, // per rand level: squared-norm contribution of that level's sign ops
    pub nplanes: Vec<usize>,
    pub rbits_used: Vec<usize>,
    pub e_basis: f64,
    pub vf64: Vec<f64>,
}

fn depth_rand(r: f64, k: usize, nb: usize) -> u8 {
    if r < 1.0 {
        if k < (r * nb as f64) as usize { 1 } else { 0 }
    } else {
        r as u8
    }
}

impl GroupPlan {
    pub fn new(p: &Params, tree: &Tree, g: usize) -> GroupPlan {
        let m = &p.meta;
        let n = m.n;
        let lam = &p.lam[g * n..(g + 1) * n];
        let sig: Vec<f32> = lam.iter().map(|&l| l.sqrt() as f32).collect();
        let lamsum = lam.iter().sum::<f64>() as f32;
        let frb = |l: usize, c: usize| p.frb[(g * NDL + l) * n + c];
        let d: Vec<Vec<f32>> = (0..NDL).map(|l| (0..n).map(|c| p.dl[(g * NDL + l) * n + c] as f32).collect()).collect();
        let mut pconst = [0f32; NDL];
        for l in 0..NDL {
            let mut s = 0.0;
            for c in 0..n {
                let gv = tree.g[frb(l, c) as usize];
                let dd = d[l][c] as f64;
                s += lam[c] * gv / (dd * dd);
            }
            pconst[l] = (s / lamsum as f64).sqrt() as f32;
        }
        let dt: Vec<f32> = (0..n).map(|c| p.dt[g * n + c] as f32).collect();
        let ridx: Vec<u16> = (0..n).map(|c| p.ridx[g * n + c] as u16).collect();
        let mut nb = [0; J];
        let mut boff = [0; J + 1];
        for j in 0..J {
            boff[j] = m.bounds[j];
            nb[j] = m.bounds[j + 1] - m.bounds[j];
        }
        boff[J] = n;
        let nr = m.rlv.len();
        let rates: Vec<[f64; J]> = (0..nr).map(|lr| core::array::from_fn(|j| p.rates[(g * nr + lr) * J + j])).collect();
        let mut rmax = [1usize; J];
        for r in &rates {
            for j in 0..J {
                rmax[j] = rmax[j].max(r[j].max(1.0) as usize);
            }
        }
        let mut ypredw = [0f32; J];
        for j in 0..J {
            let mut s = 0.0;
            for k in 0..nb[j] {
                let c = ridx[boff[j] + k] as usize;
                let lres = lam[c] * tree.g[frb(NDL - 1, c) as usize];
                let dd = dt[c] as f64;
                s += lres / (dd * dd);
            }
            ypredw[j] = (s.max(1e-30) / lamsum as f64).sqrt() as f32;
        }
        // ops and plane numbering
        let nl = m.nl();
        let mut hist: Vec<Vec<(u8, u16)>> = vec![Vec::new(); n]; // det coord -> planes by depth
        let mut rhist: Vec<Vec<(u8, u16)>> = vec![Vec::new(); n]; // rand position -> planes
        let mut ops = vec![Vec::new(); nl];
        let mut nplanes = vec![0usize; nl];
        let mut thchg = vec![Vec::new(); nr];
        let mut rbits_used = vec![0usize; nr];
        for l in 0..nl {
            let mut np = 0u16;
            if l < NDL {
                for c in 0..n {
                    let bo = if l == 0 { 0 } else { frb(l - 1, c) };
                    let bn = frb(l, c);
                    if bn > bo {
                        for _ in bo..bn {
                            hist[c].push((l as u8, np));
                            np += 1;
                        }
                        ops[l].push(mk_op(c, bo, bn, &hist[c]));
                    }
                }
            } else {
                let lr = l - NDL;
                for j in 0..J {
                    let ro = if lr == 0 { 0.0 } else { rates[lr - 1][j] };
                    let rn = rates[lr][j];
                    if rn > ro {
                        thchg[lr].push(j);
                    }
                    for k in 0..nb[j] {
                        let bo = depth_rand(ro, k, nb[j]);
                        let bn = depth_rand(rn, k, nb[j]);
                        if bn > bo {
                            let pos = boff[j] + k;
                            for _ in bo..bn {
                                rhist[pos].push((l as u8, np));
                                np += 1;
                            }
                            ops[l].push(mk_op(pos, bo, bn, &rhist[pos]));
                        }
                    }
                }
                rbits_used[lr] = np as usize;
            }
            nplanes[l] = np as usize;
        }
        // index-state slots: every coordinate / position that is ever refined past its first bit
        let mut final_det = vec![0u8; n];
        let mut final_rand = vec![0u8; n];
        let mut last_det = vec![0usize; n];
        let mut last_rand = vec![0usize; n];
        for l in 0..nl {
            for op in &ops[l] {
                let c = op.coord as usize;
                if l < NDL {
                    final_det[c] = final_det[c].max(op.bnew);
                    last_det[c] = l;
                } else {
                    final_rand[c] = final_rand[c].max(op.bnew);
                    last_rand[c] = l;
                }
            }
        }
        let mut slot_det = vec![NOSLOT; n];
        let mut slot_rand = vec![NOSLOT; n];
        let mut nslots = 0u16;
        for c in 0..n {
            if final_det[c] >= 2 {
                slot_det[c] = nslots;
                nslots += 1;
            }
        }
        for c in 0..n {
            if final_rand[c] >= 2 {
                slot_rand[c] = nslots;
                nslots += 1;
            }
        }
        assert!((nslots as usize) <= MAXSLOTS, "too many index-state slots");
        let mut lops = Vec::new();
        let mut woff = Vec::new();
        let mut nw = 0usize;
        let mut nsc = vec![[0f32; J]; nr];
        let t1sq = tree.dsq[1][1];
        for l in 0..nl {
            let parts = if l < NDL { 1 } else { J };
            let mut v = vec![LvOps::default(); parts];
            for op in &ops[l] {
                let part = if l < NDL { 0 } else { (0..J).find(|&j| (op.coord as usize) < boff[j + 1]).unwrap() };
                let c = op.coord as usize;
                let (slot, last) = if l < NDL { (slot_det[c], last_det[c]) } else { (slot_rand[c], last_rand[c]) };
                if op.bold == 0 && op.bnew == 1 {
                    v[part].signs.push(op.pl[0]);
                    v[part].sign_coord.push(op.coord);
                    v[part].sign_slot.push(slot);
                    if l >= NDL {
                        nsc[l - NDL][part] += t1sq;
                    }
                } else {
                    v[part].multi.push(*op);
                    for d in op.bold as usize..op.bnew as usize {
                        assert!(op.lv[d] as usize == l && op.pl[d] == op.pl[op.bold as usize] + (d - op.bold as usize) as u16);
                    }
                    v[part].mops.push(MOp { pl: op.pl[op.bold as usize], slot, bold: op.bold, bnew: op.bnew, store: last != l && slot != NOSLOT });
                }
            }
            for pt in v.iter_mut() {
                for (i, &pl) in pt.signs.iter().enumerate() {
                    if pt.sign_slot[i] != NOSLOT {
                        pt.ss.push((pl, pt.sign_slot[i]));
                        pt.wcoord.push(pt.sign_coord[i]);
                    }
                }
                for (i, &pl) in pt.signs.iter().enumerate() {
                    if pt.sign_slot[i] == NOSLOT {
                        pt.sp.push(pl);
                        pt.wcoord.push(pt.sign_coord[i]);
                    }
                }
                pt.nsign = pt.wcoord.len();
                let mo: Vec<(MOp, Op)> = pt.mops.iter().copied().zip(pt.multi.iter().copied()).collect();
                for (m, o) in &mo {
                    if m.bnew - m.bold == 1 && m.bnew <= 4 && m.bold >= 1 {
                        pt.m1.push(*m);
                        pt.wcoord.push(o.coord);
                        pt.mdmax.push(tree.dmax[m.bold as usize * 9 + m.bnew as usize]);
                    }
                }
                for (m, o) in &mo {
                    if !(m.bnew - m.bold == 1 && m.bnew <= 4 && m.bold >= 1) {
                        pt.mg.push(*m);
                        pt.wcoord.push(o.coord);
                        pt.mdmax.push(tree.dmax[m.bold as usize * 9 + m.bnew as usize]);
                    }
                }
            }
            let mut wo = Vec::new();
            for pt in &v {
                wo.push(nw);
                nw += pt.signs.len() + pt.multi.len();
            }
            lops.push(v);
            woff.push(wo);
        }
        // basis orthogonality defect ||I - V^T V||_F (an upper bound on the spectral norm)
        let vg = &p.v[g * n * n..(g + 1) * n * n];
        let vf64: Vec<f64> = vg.iter().map(|&x| x as f64).collect();
        let mut vtv = vec![0f64; n * n];
        unsafe {
            matrixmultiply::dgemm(n, n, n, 1.0, vf64.as_ptr(), 1, n as isize, vf64.as_ptr(), n as isize, 1, 0.0, vtv.as_mut_ptr(), n as isize, 1);
        }
        let mut ef = 0.0;
        for i in 0..n {
            for k in 0..n {
                let e = vtv[i * n + k] - if i == k { 1.0 } else { 0.0 };
                ef += e * e;
            }
        }
        let e_basis = ef.sqrt() * (1.0 + 1e-6) + 1e-12;
        GroupPlan {
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
            yk0: [0; J],
            ytab: vec![[0.0; 256]; J],
            rates,
            rmax,
            thchg,
            tk0: vec![[0; J]; nr],
            ttab: vec![[[0.0; 16]; J]; nr],
            ops,
            lops,
            woff,
            nw,
            nsc,
            inv_ms: core::array::from_fn(|j| 1.0 / (nb[j] - 1) as f32),
            nplanes,
            rbits_used,
            e_basis,
            vf64,
        }
    }
    pub fn set_calibration(&mut self, kmin: [i32; NDL], yk0: [i32; J], tk0: Vec<[i32; J]>) {
        self.kmin = kmin;
        for l in 0..NDL {
            for c in 0..15 {
                self.ntab[l][c] = 2f64.powf((OFF[c] + kmin[l]) as f64 / 16.0) as f32;
            }
        }
        self.yk0 = yk0;
        for j in 0..J {
            for c in 0..255 {
                self.ytab[j][c] = 2f64.powf((c as i32 + yk0[j]) as f64 / 256.0) as f32;
            }
        }
        for (lr, k0) in tk0.iter().enumerate() {
            for j in 0..J {
                for c in 0..15 {
                    self.ttab[lr][j][c] = 2f64.powf((c as i32 + k0[j]) as f64 / 20.0) as f32;
                }
            }
        }
        self.tk0 = tk0;
    }
}

fn mk_op(coord: usize, bo: u8, bn: u8, h: &[(u8, u16)]) -> Op {
    let mut op = Op { coord: coord as u16, bold: bo, bnew: bn, lv: [0; 8], pl: [0; 8] };
    for d in 0..bn as usize {
        op.lv[d] = h[d].0;
        op.pl[d] = h[d].1;
    }
    op
}

/// Fallback values stored apart from the fixed-rate stream: for each word slot, the values of its fallback lanes in lane
/// order (read with an AVX-512 expand-load). Counted as the extra byte(s) the design charges.
#[derive(Default)]
pub struct Sparse {
    pub off: Vec<u32>,
    pub val: Vec<f32>,
}
impl Sparse {
    pub fn len(&self) -> usize {
        self.val.len()
    }
}

/// Query-independent per-cluster quantities.
pub struct Cl {
    pub g: usize,
    pub r0: usize,
    pub r1: usize,
    pub blk0: usize,
    pub nblk: usize,
    pub mn: f64,
    pub mn32: f32,
    pub mhat: Vec<f64>,
    pub mv: Vec<f32>,   // V^T m
    pub wb: Vec<f32>,   // det: mv*sig by coordinate (n), then rand: R_j (mv*dt)[idx_j] by position (n)
    pub mvd2: Vec<f32>, // NDL x n: mv * D_l^2
    pub bm: [f64; NDL],
    pub mvt: Vec<f32>, // mv * dt
    pub mvt_pos: Vec<f32>, // mv * dt in rotated-block position order
    pub b2: [f64; J],
    pub btot: f64,
    pub rmax: f64, // upper bound on ||r|| over the cluster's items
    pub smax: f32,
}

/// Where full vectors come from.
pub enum Full {
    Real { map: memmap2::Mmap, off: usize, xs_row: Vec<u32> },
    Synth(crate::synth::Synth),
}

pub struct Index {
    pub meta: Meta,
    pub tree: Tree,
    pub groups: Vec<GroupPlan>,
    pub cl: Vec<Cl>,
    pub centres: Vec<f32>,
    pub centres64: Vec<f64>,
    pub v32: Vec<f32>,
    pub vt32: Vec<f32>, // per group, transposed: row c = basis vector c
    pub rots: Vec<Vec<f64>>, // J global Haar rotations (row-major nb x nb)
    pub anchors: Vec<f64>,   // A x n unit
    pub hi: Vec<f32>,        // A x K
    pub lo: Vec<f32>,
    pub nanch: usize,
    pub ahat: Vec<i16>,
    pub row_ent: Vec<u32>,
    pub ent_row: Vec<u32>,
    pub planes: Vec<Vec<u16>>,
    pub lvl_off: Vec<Vec<usize>>,
    pub nrm: Vec<Vec<u64>>,
    pub nesc: Vec<Sparse>, // per det level, slot = global block
    pub yh: Vec<u8>,
    pub yfb: Sparse, // slot = global block * J + j
    pub th: Vec<Vec<u64>>,
    pub th_off: Vec<Vec<usize>>,
    pub tfb: Vec<Sparse>, // per rand level, slot = th word index
    pub x2: f64, // upper bound on ||x||^2
    pub full: Full,
    pub nrows: usize,
}

impl Index {
    pub fn n(&self) -> usize {
        self.meta.n
    }
    pub fn k(&self) -> usize {
        self.cl.len()
    }
    pub fn stored_bytes_per_item(&self) -> Vec<f64> {
        // cumulative stored bytes per level (code + scalars + 2 B key), averaged over items
        let nl = self.meta.nl();
        let mut out = vec![0.0; nl];
        let mut tot = 2.0 * self.nrows as f64;
        for l in 0..nl {
            let mut b = 0.0;
            for c in &self.cl {
                let gp = &self.groups[c.g];
                let ni = (c.r1 - c.r0) as f64;
                b += ni * gp.nplanes[l] as f64 / 8.0;
                if l < NDL {
                    b += ni * 0.5;
                } else {
                    let lr = l - NDL;
                    if lr == 0 {
                        b += ni * J as f64;
                    }
                    b += ni * 0.5 * gp.thchg[lr].len() as f64;
                }
            }
            tot += b;
            out[l] = tot / self.nrows as f64;
        }
        out
    }
}

/// Items of one cluster as the encoder sees them: sorted by stored key.
pub struct Items {
    pub ah: Vec<i16>,
    pub z: Vec<f64>, // nc x n, local-basis residual
    pub ent: Vec<u32>,
    pub ext: Vec<u32>, // source row (real: numpy Xs row; synthetic: item seed)
    pub x2max: f64,    // max | ||x||^2 - 1 |
    pub rmax: f64,
}

pub struct Calib {
    kpool: Vec<Vec<f64>>,
    ypool: Vec<Vec<f64>>,
    tpool: Vec<Vec<Vec<f64>>>, // lr x j
}

pub struct ClusterCode {
    planes: Vec<Vec<u16>>,
    nrm: Vec<Vec<u64>>,
    nesc: Vec<Vec<(u32, f32)>>, // local row
    yh: Vec<u8>,
    yfb: Vec<(u32, f32)>,
    th: Vec<Vec<u64>>,
    tfb: Vec<(u32, u8, u8, f32)>,
    items: Items,
    calib: Option<Calib>,
}

fn roundup_abs(v: f64, steps: f64, floor_log: f64) -> f64 {
    // 2^(ceil(log2(max(v, 2^floor_log)) * steps) / steps), verified >= v
    let lv = v.max(2f64.powf(floor_log)).log2();
    let mut r = 2f64.powf((lv * steps).ceil() / steps);
    while r < v {
        r *= 2f64.powf(1.0 / steps);
    }
    r
}

pub fn encode_cluster(c: usize, mut it: Items, gp: &GroupPlan, tree: &Tree, meta: &Meta, rots: &[Vec<f64>], mn32: f32, calib: bool) -> ClusterCode {
    let n = meta.n;
    let nc = it.ah.len();
    let nblk = (nc + LANES - 1) / LANES;
    let nl = meta.nl();
    let nr = meta.rlv.len();
    let _ = c;
    let mut planes: Vec<Vec<u16>> = (0..nl).map(|l| vec![0u16; nblk * gp.nplanes[l]]).collect();
    let mut nrm: Vec<Vec<u64>> = vec![vec![0u64; nblk]; NDL];
    let mut nesc = vec![Vec::new(); NDL];
    let mut yh = vec![0u8; nc * J];
    let mut yfb = Vec::new();
    let mut th: Vec<Vec<u64>> = (0..nr).map(|lr| vec![0u64; nblk * gp.thchg[lr].len()]).collect();
    let mut tfb = Vec::new();
    let mut cal = if calib {
        Some(Calib { kpool: vec![Vec::new(); NDL], ypool: vec![Vec::new(); J], tpool: vec![vec![Vec::new(); J]; nr] })
    } else {
        None
    };
    let setbit = |planes: &mut Vec<Vec<u16>>, i: usize, l: usize, pl: u16| {
        planes[l][(i / LANES) * gp.nplanes[l] + pl as usize] |= 1u16 << (i % LANES);
    };
    // ---- phase 1: deterministic levels; whitened residual of the 32 B prefix, in block order
    let mut wmat = vec![0f64; nc * n]; // row i: position p holds w[ridx[p]]
    let mut val = vec![0f64; n];
    let mut e = vec![0f64; n];
    let svec: Vec<f32> = it.ah.iter().map(|&a| item_scale(a, mn32, gp.lamsum)).collect();
    for i in 0..nc {
        let blk = i / LANES;
        let lane = i % LANES;
        let z = &it.z[i * n..(i + 1) * n];
        let s = svec[i];
        val.iter_mut().for_each(|x| *x = 0.0);
        for l in 0..NDL {
            for op in &gp.ops[l] {
                let cc = op.coord as usize;
                let bn = op.bnew as usize;
                let u = z[cc] / (gp.sig[cc] as f64 * s as f64);
                let idx = tree.index(bn, u);
                val[cc] += tree.dtab[op.bold as usize * 9 + bn][idx] as f64;
                for d in op.bold as usize..bn {
                    if (idx >> (bn - 1 - d)) & 1 == 1 {
                        setbit(&mut planes, i, l, op.pl[d]);
                    }
                }
            }
            let mut rho2 = 0.0;
            for k in 0..n {
                e[k] = z[k] - s as f64 * gp.sig[k] as f64 * val[k];
                let q = e[k] / gp.d[l][k] as f64;
                rho2 += q * q;
            }
            let rho = rho2.sqrt();
            let need = rho * (1.0 + 1e-6) + 1e-12;
            let pred = gp.pconst[l] * s;
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
                nesc[l].push((i as u32, v));
            }
            nrm[l][blk] |= (code as u64) << (4 * lane);
        }
        for p in 0..n {
            let k = gp.ridx[p] as usize;
            wmat[i * n + p] = e[k] / gp.dt[k] as f64;
        }
    }
    it.z = Vec::new();
    // ---- phase 2: rotate each block (one GEMM per block)
    let mut ymat = vec![0f64; nc * n];
    for j in 0..J {
        let nbj = gp.nb[j];
        let o = gp.boff[j];
        unsafe {
            matrixmultiply::dgemm(nc, nbj, nbj, 1.0, wmat.as_ptr().add(o), n as isize, 1, rots[j].as_ptr(), 1, nbj as isize, 0.0, ymat.as_mut_ptr().add(o), n as isize, 1);
        }
    }
    drop(wmat);
    // ---- phase 3: randomised levels
    let mut rval = vec![0f64; n];
    for i in 0..nc {
        let blk = i / LANES;
        let lane = i % LANES;
        let s = svec[i];
        let y = &ymat[i * n..(i + 1) * n];
        let mut yn = [0f64; J];
        let mut yhv = [0f64; J];
        for j in 0..J {
            yn[j] = y[gp.boff[j]..gp.boff[j + 1]].iter().map(|v| v * v).sum::<f64>().sqrt();
            let pr = gp.ypredw[j] * s;
            let k = ((yn[j].max(1e-30) / pr as f64).log2() * 256.0).ceil();
            if let Some(cal) = cal.as_mut() {
                cal.ypool[j].push(k);
                yhv[j] = roundup_abs(yn[j] * (1.0 + 1e-7), 4096.0, -60.0) as f32 as f64;
                continue;
            }
            let code = k.max(-1e9) as i64 - gp.yk0[j] as i64;
            let mut ok = (0..=254).contains(&code);
            if ok {
                let v = (pr * gp.ytab[j][code as usize]) as f64;
                if v >= yn[j] * (1.0 + 1e-7) && v <= yn[j] * ry() * (1.0 + 1e-6) {
                    yh[i * J + j] = code as u8;
                    yhv[j] = v;
                } else {
                    ok = false;
                }
            }
            if !ok {
                let mut v = roundup_abs(yn[j] * (1.0 + 1e-7) + 1e-30, 4096.0, -60.0) as f32;
                while (v as f64) < yn[j] * (1.0 + 1e-7) {
                    v *= 1.0 + 2e-7;
                }
                yh[i * J + j] = 255;
                yfb.push((i as u32, j as u8, v));
                yhv[j] = v as f64;
            }
        }
        rval.iter_mut().for_each(|x| *x = 0.0);
        for lr in 0..nr {
            let l = NDL + lr;
            let mut jc = 0usize;
            for op in &gp.ops[l] {
                let p = op.coord as usize;
                while p >= gp.boff[jc + 1] {
                    jc += 1;
                }
                let bn = op.bnew as usize;
                let v = y[p] / yhv[jc] * (gp.nb[jc] as f64).sqrt();
                let idx = tree.index(bn, v);
                rval[p] += tree.dtab[op.bold as usize * 9 + bn][idx] as f64;
                for d in op.bold as usize..bn {
                    if (idx >> (bn - 1 - d)) & 1 == 1 {
                        setbit(&mut planes, i, l, op.pl[d]);
                    }
                }
            }
            for (slot, &j) in gp.thchg[lr].iter().enumerate() {
                let (mut dot, mut cn) = (0.0, 0.0);
                for p in gp.boff[j]..gp.boff[j + 1] {
                    dot += rval[p] * y[p];
                    cn += rval[p] * rval[p];
                }
                let cosf = (dot / (cn.sqrt() * yn[j] + 1e-30)).clamp(1e-9, 1.0);
                let tanf = (1.0 - cosf * cosf).max(0.0).sqrt() / cosf;
                let kt = (tanf.max(2f64.powf(-12.8)).log2() * 20.0).ceil();
                if let Some(cal) = cal.as_mut() {
                    cal.tpool[lr][j].push(kt);
                    continue;
                }
                let k0 = gp.tk0[lr][j];
                let code = kt as i64 - k0 as i64;
                let mut code_u = 15u64;
                if (0..=14).contains(&code) && (gp.ttab[lr][j][code as usize] as f64) >= tanf * (1.0 + 1e-7) {
                    code_u = code as u64;
                } else {
                    let mut v = roundup_abs(tanf * (1.0 + 1e-7), 20.0, -12.8) as f32;
                    while (v as f64) < tanf * (1.0 + 1e-7) {
                        v *= 1.0 + 2e-7;
                    }
                    tfb.push((i as u32, lr as u8, j as u8, v));
                }
                th[lr][blk * gp.thchg[lr].len() + slot] |= code_u << (4 * lane);
            }
        }
    }
    let yfb2 = yfb.into_iter().map(|(i, j, v)| (i * J as u32 + j as u32, v)).collect();
    ClusterCode { planes, nrm, nesc, yh, yfb: yfb2, th, tfb, items: it, calib: cal }
}

/// Query-independent per-cluster data.
pub fn cluster_info(c: usize, cstart: (usize, usize), blk0: usize, gidx: usize, gp: &GroupPlan, m: &[f32], rots: &[Vec<f64>], tree: &Tree, it: &Items, x2: f64) -> Cl {
    let n = m.len();
    let mn = m.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt();
    let mhat: Vec<f64> = m.iter().map(|&x| x as f64 / mn).collect();
    let mut mv64 = vec![0f64; n];
    for i in 0..n {
        let mi = m[i] as f64;
        let row = &gp.vf64[i * n..(i + 1) * n];
        for k in 0..n {
            mv64[k] += mi * row[k];
        }
    }
    let mv: Vec<f32> = mv64.iter().map(|&x| x as f32).collect();
    let mut wb = vec![0f32; 2 * n];
    for k in 0..n {
        wb[k] = (mv64[k] * gp.sig[k] as f64) as f32;
    }
    let mvt64: Vec<f64> = (0..n).map(|k| mv64[k] * gp.dt[k] as f64).collect();
    let mut b2 = [0f64; J];
    for j in 0..J {
        let nbj = gp.nb[j];
        for a in 0..nbj {
            let mut s = 0.0;
            for b in 0..nbj {
                s += rots[j][a * nbj + b] * mvt64[gp.ridx[gp.boff[j] + b] as usize];
            }
            wb[n + gp.boff[j] + a] = s as f32;
        }
        b2[j] = (gp.boff[j]..gp.boff[j + 1]).map(|p| mvt64[gp.ridx[p] as usize].powi(2)).sum();
    }
    let mut mvd2 = vec![0f32; NDL * n];
    let mut bm = [0f64; NDL];
    for l in 0..NDL {
        for k in 0..n {
            let dd = gp.d[l][k] as f64;
            mvd2[l * n + k] = (mv64[k] * dd * dd) as f32;
            bm[l] += mv64[k] * mv64[k] * dd * dd;
        }
    }
    let mn32 = mn as f32;
    let smax = it.ah.iter().map(|&a| item_scale(a, mn32, gp.lamsum)).fold(0f32, f32::max);
    let rmax = it.ah.iter().map(|&a| {
        let al = a as f64 / 32767.0 - DA;
        (x2 + mn * mn - 2.0 * mn * al).max(0.0).sqrt()
    }).fold(0f64, f64::max);
    let _ = (c, tree);
    let btot = b2.iter().sum();
    let nc = cstart.1 - cstart.0;
    Cl { g: gidx, r0: cstart.0, r1: cstart.1, blk0, nblk: (nc + LANES - 1) / LANES, mn, mn32, mhat, mv, wb, mvd2, bm, mvt: mvt64.iter().map(|&x| x as f32).collect(), mvt_pos: (0..n).map(|p| mvt64[gp.ridx[p] as usize] as f32).collect(), b2, btot, rmax, smax }
}

/// Source of items for the build.
pub trait Source: Sync {
    fn items(&self, c: usize, p: &Params, gp: &GroupPlan, mhat: &[f64]) -> Items;
    fn full(self: Box<Self>) -> Full;
}

pub struct RealSource {
    pub map: memmap2::Mmap,
    pub off: usize,
    pub n: usize,
}
impl RealSource {
    pub fn open(path: &str) -> RealSource {
        let f = std::fs::File::open(path).unwrap();
        let map = unsafe { memmap2::Mmap::map(&f).unwrap() };
        let (hdr, off) = npy_header(&map);
        let sh = npy_shape(&hdr);
        assert!(hdr.contains("<f2"));
        RealSource { map, off, n: sh[1] }
    }
    pub fn row(&self, r: usize) -> &[u16] {
        let n = self.n;
        let b = &self.map[self.off + r * n * 2..self.off + (r + 1) * n * 2];
        unsafe { std::slice::from_raw_parts(b.as_ptr() as *const u16, n) }
    }
}
impl Source for RealSource {
    fn items(&self, c: usize, p: &Params, gp: &GroupPlan, mhat: &[f64]) -> Items {
        let n = self.n;
        let (r0, r1) = (p.cstart[c], p.cstart[c + 1]);
        let m = &p.centres[c * n..(c + 1) * n];
        let mut key: Vec<(i16, usize)> = (r0..r1).map(|r| {
            let x = self.row(r);
            let a: f64 = x.iter().zip(mhat).map(|(&h, &mh)| f16_to_f32(h) as f64 * mh).sum();
            ((a.clamp(-1.0, 1.0) * 32767.0).round() as i16, r)
        }).collect();
        key.sort();
        let nc = r1 - r0;
        let mut rmat = vec![0f64; nc * n];
        let mut x2max = 0f64;
        for (i, &(_, r)) in key.iter().enumerate() {
            let x = self.row(r);
            let mut nx = 0.0;
            for k in 0..n {
                let xv = f16_to_f32(x[k]) as f64;
                nx += xv * xv;
                rmat[i * n + k] = xv - m[k] as f64;
            }
            x2max = x2max.max((nx - 1.0).abs());
        }
        let mut z = vec![0f64; nc * n];
        unsafe {
            matrixmultiply::dgemm(nc, n, n, 1.0, rmat.as_ptr(), n as isize, 1, gp.vf64.as_ptr(), n as isize, 1, 0.0, z.as_mut_ptr(), n as isize, 1);
        }
        Items {
            ah: key.iter().map(|k| k.0).collect(),
            z,
            ent: key.iter().map(|k| p.perm[k.1] as u32).collect(),
            ext: key.iter().map(|k| k.1 as u32).collect(),
            x2max,
            rmax: 0.0,
        }
    }
    fn full(self: Box<Self>) -> Full {
        let s = *self;
        Full::Real { map: s.map, off: s.off, xs_row: Vec::new() }
    }
}

/// Build the whole index from parameters and a source of items.
pub fn build(p: &Params, src: Box<dyn Source>, nanch: usize, log: &mut dyn FnMut(String)) -> Index {
    let t = Timer::new();
    let meta = Meta { n: p.meta.n, k: p.meta.k, g: p.meta.g, dlv: p.meta.dlv.clone(), rlv: p.meta.rlv.clone(), bounds: p.meta.bounds.clone(), csg: p.meta.csg };
    let n = meta.n;
    let k = meta.k;
    let tree = Tree::new(p);
    let rots: Vec<Vec<f64>> = (0..J).map(|j| haar_rotation(meta.bounds[j + 1] - meta.bounds[j], SEED, j as u64)).collect();
    let mut groups: Vec<GroupPlan> = (0..meta.g).into_par_iter().map(|g| GroupPlan::new(p, &tree, g)).collect();
    log(format!("plans {:.1}s; max basis defect {:.2e}", t.s(), groups.iter().map(|g| g.e_basis).fold(0.0, f64::max)));
    let mhats: Vec<Vec<f64>> = (0..k).map(|c| {
        let m = &p.centres[c * n..(c + 1) * n];
        let mn = m.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt();
        m.iter().map(|&x| x as f64 / mn).collect()
    }).collect();
    let mn32: Vec<f32> = (0..k).map(|c| p.centres[c * n..(c + 1) * n].iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt() as f32).collect();
    // calibration pass on every 8th cluster of each group
    let mut calc: Vec<usize> = Vec::new();
    for g in 0..meta.g {
        let ks: Vec<usize> = (0..k).filter(|&c| p.grp[c] == g).collect();
        // numpy semantics: every (len/8)-th cluster of the group; VEXACT_CAL_PER_GROUP overrides the 8
        let per: usize = std::env::var("VEXACT_CAL_PER_GROUP").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
        let step = (ks.len() / per).max(1);
        calc.extend(ks.iter().step_by(step));
    }
    let src_ref = &*src;
    let cals: Vec<(usize, Calib)> = calc.par_iter().map(|&c| {
        let g = p.grp[c];
        let it = src_ref.items(c, p, &groups[g], &mhats[c]);
        let cc = encode_cluster(c, it, &groups[g], &tree, &meta, &rots, mn32[c], true);
        (g, cc.calib.unwrap())
    }).collect();
    let nr = meta.rlv.len();
    for g in 0..meta.g {
        let mine: Vec<&Calib> = cals.iter().filter(|x| x.0 == g).map(|x| &x.1).collect();
        let mut kmin = [0i32; NDL];
        for l in 0..NDL {
            let mut v: Vec<f64> = mine.iter().flat_map(|c| c.kpool[l].iter().copied()).collect();
            kmin[l] = quantile(&mut v, 0.003).floor() as i32;
        }
        let mut yk0 = [0i32; J];
        for j in 0..J {
            let mut v: Vec<f64> = mine.iter().flat_map(|c| c.ypool[j].iter().copied()).collect();
            yk0[j] = quantile(&mut v, 0.002).floor() as i32;
        }
        let mut tk0 = vec![[0i32; J]; nr];
        for lr in 0..nr {
            for j in 0..J {
                let mut v: Vec<f64> = mine.iter().flat_map(|c| c.tpool[lr][j].iter().copied()).collect();
                if v.is_empty() {
                    continue;
                }
                let lo = quantile(&mut v, 0.002);
                let hi = quantile(&mut v, 0.998);
                tk0[lr][j] = lo.max(hi - 14.0) as i32;
            }
        }
        groups[g].set_calibration(kmin, yk0, tk0);
    }
    log(format!("calibrated {:.1}s", t.s()));
    // encode every cluster
    let codes: Vec<ClusterCode> = (0..k).into_par_iter().map(|c| {
        let g = p.grp[c];
        let it = src_ref.items(c, p, &groups[g], &mhats[c]);
        encode_cluster(c, it, &groups[g], &tree, &meta, &rots, mn32[c], false)
    }).collect();
    log(format!("encoded {:.1}s", t.s()));
    let x2 = 1.0 + codes.iter().map(|c| c.items.x2max).fold(0.0, f64::max) * (1.0 + 1e-9) + 1e-12;
    // assemble
    let nl = meta.nl();
    let mut row0 = 0usize;
    let mut blk0 = 0usize;
    let mut cl = Vec::with_capacity(k);
    let mut ahat = Vec::new();
    let mut row_ent = Vec::new();
    let mut xs_row = Vec::new();
    let mut planes: Vec<Vec<u16>> = vec![Vec::new(); nl];
    let mut lvl_off: Vec<Vec<usize>> = vec![Vec::new(); nl];
    let mut nrm: Vec<Vec<u64>> = vec![Vec::new(); NDL];
    let mut nesc: Vec<HashMap<u32, f32>> = vec![HashMap::new(); NDL];
    let mut yh = Vec::new();
    let mut yfb = HashMap::new();
    let mut th: Vec<Vec<u64>> = vec![Vec::new(); nr];
    let mut th_off: Vec<Vec<usize>> = vec![Vec::new(); nr];
    let mut tfb = HashMap::new();
    let infos: Vec<Cl> = codes.par_iter().enumerate().map(|(c, cc)| {
        let g = p.grp[c];
        cluster_info(c, (0, cc.items.ah.len()), 0, g, &groups[g], &p.centres[c * n..(c + 1) * n], &rots, &tree, &cc.items, x2)
    }).collect();
    for (c, (cc, mut info)) in codes.into_iter().zip(infos).enumerate() {
        let nc = cc.items.ah.len();
        info.r0 = row0;
        info.r1 = row0 + nc;
        info.blk0 = blk0;
        for l in 0..nl {
            lvl_off[l].push(planes[l].len());
            planes[l].extend_from_slice(&cc.planes[l]);
        }
        for l in 0..NDL {
            nrm[l].extend_from_slice(&cc.nrm[l]);
            for &(i, v) in &cc.nesc[l] {
                nesc[l].insert((row0 + i as usize) as u32, v);
            }
        }
        yh.extend_from_slice(&cc.yh);
        for &(key, v) in &cc.yfb {
            yfb.insert((row0 * J) as u32 + key, v);
        }
        for lr in 0..nr {
            th_off[lr].push(th[lr].len());
            th[lr].extend_from_slice(&cc.th[lr]);
        }
        for &(i, lr, j, v) in &cc.tfb {
            tfb.insert(((row0 + i as usize) as u32, lr, j), v);
        }
        ahat.extend_from_slice(&cc.items.ah);
        row_ent.extend_from_slice(&cc.items.ent);
        xs_row.extend_from_slice(&cc.items.ext);
        row0 += nc;
        blk0 += info.nblk;
        let _ = c;
        cl.push(info);
    }
    let nrows = row0;
    let maxent = row_ent.iter().copied().max().unwrap_or(0) as usize;
    let mut ent_row = vec![u32::MAX; maxent + 1];
    for (r, &e) in row_ent.iter().enumerate() {
        ent_row[e as usize] = r as u32;
    }
    let mut full = src.full();
    if let Full::Real { xs_row: xr, .. } = &mut full {
        *xr = xs_row.clone();
    }
    if let Full::Synth(sy) = &mut full {
        sy.ext = xs_row.clone();
        sy.row_cl = cl.iter().enumerate().flat_map(|(ci, c)| std::iter::repeat(ci as u32).take(c.r1 - c.r0)).collect();
    }
    // fallbacks to lane-ordered sparse arrays
    let nblk_tot = blk0;
    let mut nesc_s = Vec::new();
    for l in 0..NDL {
        let mut sp = Sparse { off: Vec::with_capacity(nblk_tot + 1), val: Vec::new() };
        for c in &cl {
            for b in 0..c.nblk {
                sp.off.push(sp.val.len() as u32);
                let w = nrm[l][c.blk0 + b];
                for i in 0..LANES {
                    if (w >> (4 * i)) & 15 == 15 {
                        sp.val.push(*nesc[l].get(&((c.r0 + b * LANES + i) as u32)).unwrap());
                    }
                }
            }
        }
        sp.off.push(sp.val.len() as u32);
        nesc_s.push(sp);
    }
    let mut yfb_s = Sparse { off: Vec::with_capacity(nblk_tot * J + 1), val: Vec::new() };
    for c in &cl {
        for b in 0..c.nblk {
            for j in 0..J {
                yfb_s.off.push(yfb_s.val.len() as u32);
                for i in 0..LANES {
                    let r = c.r0 + b * LANES + i;
                    if r < c.r1 && yh[r * J + j] == 255 {
                        yfb_s.val.push(*yfb.get(&((r * J + j) as u32)).unwrap());
                    }
                }
            }
        }
    }
    yfb_s.off.push(yfb_s.val.len() as u32);
    let mut tfb_s = Vec::new();
    for lr in 0..nr {
        let mut sp = Sparse { off: Vec::with_capacity(th[lr].len() + 1), val: Vec::new() };
        for (ci, c) in cl.iter().enumerate() {
            let gp = &groups[c.g];
            let ns = gp.thchg[lr].len();
            for b in 0..c.nblk {
                for (slot, &j) in gp.thchg[lr].iter().enumerate() {
                    debug_assert_eq!(sp.off.len(), th_off[lr][ci] + b * ns + slot);
                    sp.off.push(sp.val.len() as u32);
                    let w = th[lr][th_off[lr][ci] + b * ns + slot];
                    for i in 0..LANES {
                        if (w >> (4 * i)) & 15 == 15 {
                            sp.val.push(*tfb.get(&((c.r0 + b * LANES + i) as u32, lr as u8, j as u8)).unwrap());
                        }
                    }
                }
            }
        }
        sp.off.push(sp.val.len() as u32);
        tfb_s.push(sp);
    }
    let (nesc, yfb, tfb) = (nesc_s, yfb_s, tfb_s);
    let mut idx = Index {
        meta,
        tree,
        groups,
        cl,
        centres: p.centres.clone(),
        centres64: p.centres.iter().map(|&x| x as f64).collect(),
        v32: Vec::new(),
        vt32: {
            let n = p.meta.n;
            let mut t = vec![0f32; p.v.len()];
            for g in 0..p.meta.g {
                for i in 0..n {
                    for c in 0..n {
                        t[g * n * n + c * n + i] = p.v[g * n * n + i * n + c];
                    }
                }
            }
            t
        },
        rots,
        anchors: Vec::new(),
        hi: Vec::new(),
        lo: Vec::new(),
        nanch: 0,
        ahat,
        row_ent,
        ent_row,
        planes,
        lvl_off,
        nrm,
        nesc,
        yh,
        yfb,
        th,
        th_off,
        tfb,
        x2,
        full,
        nrows,
    };
    log(format!("assembled {:.1}s; x2 bound {:.3e}", t.s(), x2 - 1.0));
    if let Full::Real { .. } = idx.full {
        build_table(&mut idx, nanch, &mhats);
        log(format!("support table ({} anchors) {:.1}s", nanch, t.s()));
    }
    idx
}

/// Anchors: an evenly spaced subset of the cluster centres. Support table: exact max/min of p.x over each cluster,
/// rounded outward to 2^-15.
pub fn build_table(idx: &mut Index, nanch: usize, mhats: &[Vec<f64>]) {
    let n = idx.n();
    let k = idx.k();
    let step = k as f64 / nanch as f64;
    let sel: Vec<usize> = (0..nanch).map(|a| (a as f64 * step) as usize).collect();
    let mut anchors = vec![0f64; nanch * n];
    for (a, &c) in sel.iter().enumerate() {
        anchors[a * n..(a + 1) * n].copy_from_slice(&mhats[c]);
    }
    let cols: Vec<(Vec<f32>, Vec<f32>)> = (0..k).into_par_iter().map(|c| {
        let cl = &idx.cl[c];
        let nc = cl.r1 - cl.r0;
        let mut x = vec![0f64; nc * n];
        for i in 0..nc {
            idx.full_row_f64(cl.r0 + i, &mut x[i * n..(i + 1) * n]);
        }
        let mut px = vec![0f64; nc * nanch];
        unsafe {
            matrixmultiply::dgemm(nc, n, nanch, 1.0, x.as_ptr(), n as isize, 1, anchors.as_ptr(), 1, n as isize, 0.0, px.as_mut_ptr(), nanch as isize, 1);
        }
        let mut hi = vec![f32::NEG_INFINITY; nanch];
        let mut lo = vec![f32::INFINITY; nanch];
        for a in 0..nanch {
            let (mut mx, mut mi) = (f64::NEG_INFINITY, f64::INFINITY);
            for i in 0..nc {
                mx = mx.max(px[i * nanch + a]);
                mi = mi.min(px[i * nanch + a]);
            }
            hi[a] = (((mx + 1e-12) * 32768.0).ceil() / 32768.0) as f32;
            lo[a] = (((mi - 1e-12) * 32768.0).floor() / 32768.0) as f32;
        }
        (hi, lo)
    }).collect();
    idx.hi = vec![0f32; nanch * k];
    idx.lo = vec![0f32; nanch * k];
    for c in 0..k {
        for a in 0..nanch {
            idx.hi[a * k + c] = cols[c].0[a];
            idx.lo[a * k + c] = cols[c].1[a];
        }
    }
    idx.anchors = anchors;
    idx.nanch = nanch;
}

impl Index {
    pub fn centres_f64(&self, ci: usize) -> &[f64] {
        let n = self.n();
        &self.centres64[ci * n..(ci + 1) * n]
    }
    pub fn row_cluster(&self, row: usize) -> usize {
        self.cl.partition_point(|c| c.r1 <= row)
    }
    /// The stored vector of a row as f64 (exact for f16; for synthetic items, regenerated).
    pub fn full_row_f64(&self, row: usize, out: &mut [f64]) {
        match &self.full {
            Full::Real { map, off, xs_row } => {
                let n = self.n();
                let r = xs_row[row] as usize;
                let b = &map[off + r * n * 2..off + (r + 1) * n * 2];
                for k in 0..n {
                    out[k] = f16_to_f32(u16::from_le_bytes([b[2 * k], b[2 * k + 1]])) as f64;
                }
            }
            Full::Synth(s) => s.x_f64(self, row, out),
        }
    }
    pub fn full_row_f16(&self, row: usize) -> &[u16] {
        match &self.full {
            Full::Real { map, off, xs_row } => {
                let n = self.n();
                let r = xs_row[row] as usize;
                let b = &map[off + r * n * 2..off + (r + 1) * n * 2];
                unsafe { std::slice::from_raw_parts(b.as_ptr() as *const u16, n) }
            }
            Full::Synth(_) => panic!("synthetic rows have no stored f16 vector"),
        }
    }
}
