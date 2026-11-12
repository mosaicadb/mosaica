//! Query paths: per-query and per-cluster setup, the cluster cut and sorted range, the threshold query (single and
//! batched), exact k-NN (seed, sweep, selection), flat scans and brute-force truth.
use crate::index::*;
use crate::kernel::*;
use crate::util::*;
use rayon::prelude::*;
use roaring::RoaringBitmap;
use std::sync::OnceLock;

pub const U: f64 = 1.0 / 16777216.0; // f32 unit roundoff

/// Per query, per group.
pub struct GQ {
    pub wa: Vec<f32>, // det: QV*sig by coordinate; rand: R_j (QV*dt)[idx_j] by position
    pub qv: Vec<f64>,
    pub cq: [f64; NDL],
    pub qvt: Vec<f64>,
    pub qvt_pos: Vec<f64>,
    pub a2: [f64; J],
    pub qvn: f64,
}

/// Per query, per opened cluster: the constants of the bounds and the op-ordered combined weights
/// w = (Q_V - alpha M_V) * sig (deterministic coordinates) or R_j(...) (randomised positions).
pub struct CQ {
    pub qm: f32,
    pub alr: f32,
    pub dwa: [f32; NDL],
    pub slk: [f32; NDL],
    pub nd: [f32; J],
    pub slkr: f32,
    pub eacc: Vec<f32>, // NR x J
    pub w: Vec<f32>,
    pub base: Vec<f32>, // NDL + NR x J
    pub infl: [f32; NDL], // dwa(alpha used) / dwa(alpha optimal for the level), diagnostics
}

pub struct Prep {
    pub q32: Vec<f32>,
    pub q16: Vec<u16>,
    pub q64: Vec<f64>,
    pub qn: f64,
    pub anchor: usize,
    pub beta: f64,
    pub rho: f64,
    pub gq: Vec<OnceLock<GQ>>,
}

pub fn prep(ix: &Index, q: &[f32]) -> Prep {
    let n = ix.n();
    let q16: Vec<u16> = q.iter().map(|&x| f32_to_f16(x)).collect();
    let q32: Vec<f32> = q16.iter().map(|&h| f16_to_f32(h)).collect();
    let q64: Vec<f64> = q32.iter().map(|&x| x as f64).collect();
    let qn = q64.iter().map(|x| x * x).sum::<f64>().sqrt();
    let (mut best, mut ba) = (f64::NEG_INFINITY, 0);
    for a in 0..ix.nanch {
        let b = dot64(&q64, &ix.anchors[a * n..(a + 1) * n]);
        if b > best {
            best = b;
            ba = a;
        }
    }
    let rho = (qn * qn - best * best).max(0.0).sqrt() * (1.0 + 1e-12) + 1e-12;
    Prep { q32, q16, q64, qn, anchor: ba, beta: best, rho, gq: (0..ix.groups.len()).map(|_| OnceLock::new()).collect() }
}

pub fn group_q(ix: &Index, g: usize, q32: &[f32], qv: Vec<f64>) -> GQ {
    let n = ix.n();
    let gp = &ix.groups[g];
    let _ = q32;
    let mut wa = vec![0f32; 2 * n];
    for k in 0..n {
        wa[k] = (qv[k] * gp.sig[k] as f64) as f32;
    }
    let mut cq = [0f64; NDL];
    let qv2: Vec<f64> = qv.iter().map(|x| x * x).collect();
    for l in 0..NDL {
        let d2: Vec<f64> = gp.d[l].iter().map(|&d| d as f64 * d as f64).collect();
        cq[l] = dot64(&qv2, &d2);
    }
    let qvt: Vec<f64> = (0..n).map(|k| qv[k] * gp.dt[k] as f64).collect();
    let mut a2 = [0f64; J];
    for j in 0..J {
        let nbj = gp.nb[j];
        let r = &ix.rots[j];
        let xs: Vec<f64> = (0..nbj).map(|b| qvt[gp.ridx[gp.boff[j] + b] as usize]).collect();
        a2[j] = xs.iter().map(|x| x * x).sum();
        for a in 0..nbj {
            wa[n + gp.boff[j] + a] = dot64(&r[a * nbj..(a + 1) * nbj], &xs) as f32;
        }
    }
    let qvn = qv.iter().map(|x| x * x).sum::<f64>().sqrt();
    let qvt_pos: Vec<f64> = (0..n).map(|p| qvt[gp.ridx[p] as usize]).collect();
    GQ { wa, qv, cq, qvt, qvt_pos, a2, qvn }
}

pub fn get_gq<'a>(ix: &Index, pr: &'a Prep, g: usize, st: &mut Stats) -> &'a GQ {
    pr.gq[g].get_or_init(|| {
        st.groups += 1;
        let n = ix.n();
        let mut qv = vec![vec![0f64; n]];
        unsafe { basis_matvec(ix_v(ix, g), n, &[&pr.q64], &mut qv) };
        group_q(ix, g, &pr.q32, qv.pop().unwrap())
    })
}

pub fn ix_v(ix: &Index, g: usize) -> &[f32] {
    let n = ix.n();
    &ix.vt32[g * n * n..(g + 1) * n * n]
}

pub fn cluster_q(ix: &Index, ci: usize, gq: &GQ, pr: &Prep, qm: f64) -> CQ {
    let c = &ix.cl[ci];
    let n = ix.n();
    let gp = &ix.groups[c.g];
    let nr = ix.meta.rlv.len();
    let nl = NDL + nr;
    // one alpha for every level: the optimum of the randomised (whitened) profile
    let mut ab = [0f64; J];
    for j in 0..J {
        let (p0, p1) = (gp.boff[j], gp.boff[j + 1]);
        ab[j] = dot32_64(&c.mvt_pos[p0..p1], &gq.qvt_pos[p0..p1]);
    }
    let alr32 = (ab.iter().sum::<f64>() / c.btot) as f32;
    let a = alr32 as f64;
    let mvn = c.mv.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt();
    let basis = gp.e_basis * (pr.qn + a.abs() * c.mn) * c.rmax + 4.0 * U * (gq.qvn + a.abs() * mvn) * c.rmax;
    let fixed = a.abs() * c.mn * DA + basis + 4e-6;
    // weights in op order, sign bases, accumulation-error sums
    let t0 = ix.tree.dtab[1][0] as f64;
    let t1 = ix.tree.dtab[1][1] as f64;
    let mut w = vec![0f32; gp.nw];
    let mut base = vec![0f32; NDL + nr * J];
    let mut sabs = vec![0f64; NDL + nr * J];
    let mut cnt = vec![0f64; NDL + nr * J];
    let (mut sd, mut cd) = (0.0, 0.0);
    let mut sj = [0f64; J];
    let mut cj = [0f64; J];
    for l in 0..nl {
        let off = if l < NDL { 0 } else { n };
        for (part, lo) in gp.lops[l].iter().enumerate() {
            let wo = gp.woff[l][part];
            let ns = lo.nsign;
            let nwp = lo.wcoord.len();
            let mut bsv = [0f64; 4];
            let mut sav = [0f64; 4];
            for (i, &k) in lo.wcoord.iter().enumerate() {
                let k = off + k as usize;
                let wk = gq.wa[k] as f64 - a * c.wb[k] as f64;
                if i < ns {
                    bsv[i & 3] += wk;
                    w[wo + i] = (wk * (t1 - t0)) as f32;
                    sav[i & 3] += wk.abs();
                } else {
                    w[wo + i] = wk as f32;
                    sav[i & 3] += wk.abs() * lo.mdmax[i - ns] as f64 / (3.0 * t1);
                }
            }
            let bs = (bsv[0] + bsv[1] + bsv[2] + bsv[3]) * t0;
            let sa = (sav[0] + sav[1] + sav[2] + sav[3]) * 3.0 * t1;
            let ca = (2 * ns + (nwp - ns)) as f64;
            if l < NDL {
                base[l] = bs as f32;
                sd += sa + bs.abs();
                cd += ca + 1.0;
            } else {
                let i = NDL + (l - NDL) * J + part;
                base[i] = bs as f32;
                sj[part] += sa + bs.abs();
                cj[part] += ca + 1.0;
            }
        }
        if l < NDL {
            sabs[l] = sd;
            cnt[l] = cd;
        } else {
            for j in 0..J {
                sabs[NDL + (l - NDL) * J + j] = sj[j];
                cnt[NDL + (l - NDL) * J + j] = cj[j];
            }
        }
    }
    let mut dwa = [0f32; NDL];
    let mut slk = [0f32; NDL];
    let mut infl = [0f32; NDL];
    for l in 0..NDL {
        let aqm = dot32_64(&c.mvd2[l * n..(l + 1) * n], &gq.qv);
        let dw2 = gq.cq[l] - 2.0 * a * aqm + a * a * c.bm[l];
        let opt = (gq.cq[l] - aqm * aqm / c.bm[l]).max(0.0);
        dwa[l] = ((dw2.max(0.0) + 1e-12 * gq.cq[l]).sqrt() * (1.0 + 1e-7)) as f32;
        infl[l] = (dw2.max(0.0) / opt.max(1e-300)).sqrt() as f32;
        let acc = (cnt[l] + 4.0) * U * c.smax as f64 * sabs[l];
        slk[l] = (fixed + acc) as f32;
    }
    let mut nd = [0f32; J];
    for j in 0..J {
        let v = gq.a2[j] - 2.0 * a * ab[j] + a * a * c.b2[j];
        nd[j] = ((v.max(0.0) + 1e-12 * gq.a2[j]).sqrt() * (1.0 + 1e-7)) as f32;
    }
    let slkr = (fixed + (cnt[NDL - 1] + 4.0) * U * c.smax as f64 * sabs[NDL - 1]) as f32;
    let mut eacc = vec![0f32; nr * J];
    for i in 0..nr * J {
        eacc[i] = ((cnt[NDL + i] + 4.0) * U * sabs[NDL + i] * 1.01) as f32;
    }
    CQ { qm: qm as f32, alr: alr32, dwa, slk, nd, slkr, eacc, w, base, infl }
}

// ---------------- the cluster cut and the sorted range ----------------

/// a-interval where p*a + r*sqrt(R^2 - a^2) >= t (r >= 0, a in [-R, R]); None if empty.
fn sup_set(p: f64, r: f64, rr: f64, t: f64) -> Option<(f64, f64)> {
    let qn = (p * p + r * r).sqrt();
    if qn == 0.0 {
        return if t <= 0.0 { Some((-rr, rr)) } else { None };
    }
    let tau = t / (qn * rr);
    if tau > 1.0 {
        return None;
    }
    if tau <= -1.0 {
        return Some((-rr, rr));
    }
    let phi = r.atan2(p); // in [0, pi]
    let psi = tau.acos();
    let th_lo = (phi - psi).max(0.0);
    let th_hi = (phi + psi).min(std::f64::consts::PI);
    if th_lo > th_hi {
        return None;
    }
    Some((rr * th_hi.cos(), rr * th_lo.cos()))
}
/// a-interval where p*a - r*sqrt(R^2 - a^2) < t.
fn inf_set(p: f64, r: f64, rr: f64, t: f64) -> Option<(f64, f64)> {
    let qn = (p * p + r * r).sqrt();
    if qn == 0.0 {
        return if 0.0 < t { Some((-rr, rr)) } else { None };
    }
    let tau = t / (qn * rr);
    if tau > 1.0 {
        return Some((-rr, rr));
    }
    if tau <= -1.0 {
        return None;
    }
    let phi = r.atan2(p);
    let psi = tau.acos();
    // theta + phi in (psi, 2 pi - psi), theta in [0, pi]
    let lo = (psi - phi).max(0.0);
    let hi = (2.0 * std::f64::consts::PI - psi - phi).min(std::f64::consts::PI);
    if lo >= hi {
        return None;
    }
    Some((rr * hi.cos(), rr * lo.cos()))
}
fn isect(a: Option<(f64, f64)>, b: Option<(f64, f64)>) -> Option<(f64, f64)> {
    match (a, b) {
        (Some(x), Some(y)) => {
            let lo = x.0.max(y.0);
            let hi = x.1.min(y.1);
            if lo <= hi { Some((lo, hi)) } else { None }
        }
        _ => None,
    }
}

pub struct Cut {
    pub cm: f64,
    pub s1: f64,
    pub bh: f64,
    pub bl: f64,
    pub rm: f64,
    pub rp: f64,
}

pub fn cut_params(ix: &Index, ci: usize, pr: &Prep, qm: f64) -> Cut {
    let n = ix.n();
    let c = &ix.cl[ci];
    let k = ix.k();
    let cm = qm / c.mn;
    let s1 = (pr.qn * pr.qn - cm * cm).max(0.0).sqrt() * (1.0 + 1e-12) + 1e-12;
    let a = pr.anchor;
    let (hi, lo) = (ix.hi[a * k + ci] as f64, ix.lo[a * k + ci] as f64);
    let b = pr.beta;
    let (bh, bl) = if b >= 0.0 { (b * hi, b * lo) } else { (b * lo, b * hi) };
    let pm = dot64(&ix.anchors[a * n..(a + 1) * n], &c.mhat);
    let rm = cm - b * pm;
    let rp = (pr.rho * pr.rho - rm * rm).max(0.0).sqrt() * (1.0 + 1e-12) + 1e-12;
    Cut { cm, s1, bh, bl, rm, rp }
}

/// Rows of the cluster: [u0,u1) may satisfy the upper bound; [d0,d1) within it is undecided; the rest of [u0,u1) is
/// certified in.
pub fn sorted_range(ix: &Index, ci: usize, cu: &Cut, t: f64) -> (usize, usize, usize, usize) {
    let c = &ix.cl[ci];
    let rr = ix.x2.sqrt();
    let eps = 1e-9 + DA;
    let uset = isect(sup_set(cu.cm, cu.s1, rr, t - 1e-9), sup_set(cu.rm, cu.rp, rr, t - cu.bh - 1e-9));
    let lset = isect(inf_set(cu.cm, cu.s1, rr, t + 1e-9), inf_set(cu.rm, cu.rp, rr, t - cu.bl + 1e-9));
    let ah = &ix.ahat[c.r0..c.r1];
    let rows = |iv: (f64, f64)| -> (usize, usize) {
        let (lo, hi) = (iv.0 - eps, iv.1 + eps);
        let a = ah.partition_point(|&x| (x as f64 / 32767.0) < lo);
        let b = ah.partition_point(|&x| (x as f64 / 32767.0) <= hi);
        (c.r0 + a, c.r0 + b.max(a))
    };
    match uset {
        None => (c.r0, c.r0, c.r0, c.r0),
        Some(u) => {
            let (u0, u1) = rows(u);
            match lset {
                None => (u0, u1, u0, u0),
                Some(l) => {
                    let (l0, l1) = rows(l);
                    let d0 = l0.max(u0);
                    let d1 = l1.min(u1);
                    if d0 >= d1 { (u0, u1, u0, u0) } else { (u0, u1, d0, d1) }
                }
            }
        }
    }
}

/// Interval for one item from the stored key alone (annulus and table-plus-rho bounds), for k-NN selection.
pub fn key_interval(ix: &Index, ci: usize, cu: &Cut, row: usize) -> (f64, f64) {
    let rr = ix.x2.sqrt();
    let a = ix.ahat[row] as f64 / 32767.0;
    let (lo, hi) = ((a - DA).max(-rr), (a + DA).min(rr));
    let _ = ci;
    let f = |p: f64, r: f64, off: f64, sgn: f64| -> f64 {
        // sup (sgn=+1) or inf (sgn=-1) over [lo,hi] of off + p a + sgn r sqrt(R^2-a^2)
        let qn = (p * p + r * r).sqrt().max(1e-300);
        let astar = sgn * p * rr / qn;
        let aa = astar.clamp(lo, hi);
        off + p * aa + sgn * r * (rr * rr - aa * aa).max(0.0).sqrt()
    };
    let ub = f(cu.cm, cu.s1, 0.0, 1.0).min(f(cu.rm, cu.rp, cu.bh, 1.0)) + 1e-9;
    let lb = f(cu.cm, cu.s1, 0.0, -1.0).max(f(cu.rm, cu.rp, cu.bl, -1.0)) - 1e-9;
    (lb, ub)
}

// ---------------- visibility ----------------

pub struct Vis {
    pub bits: Vec<u64>, // over rows
    pub cl_cnt: Vec<u32>,
    pub bm: RoaringBitmap, // over entity ids
    pub probe: bool,       // test each touched item against the entity bitmap instead of the row bitset
}
impl Vis {
    /// The viewer's entity bitmap permuted into row order (once per session).
    pub fn new(ix: &Index, bm: RoaringBitmap, probe: bool) -> Vis {
        let mut bits = vec![0u64; (ix.nrows + 64) / 64 + 1];
        for e in bm.iter() {
            let r = ix.ent_row[e as usize] as usize;
            bits[r / 64] |= 1 << (r % 64);
        }
        let cl_cnt = ix.cl.iter().map(|c| (c.r0..c.r1).filter(|&r| bits[r / 64] >> (r % 64) & 1 == 1).count() as u32).collect();
        Vis { bits, cl_cnt, bm, probe }
    }
    #[inline]
    pub fn mask16(&self, ix: &Index, row0: usize) -> u16 {
        if self.probe {
            let mut m = 0u16;
            for i in 0..16 {
                let r = row0 + i;
                if r < ix.nrows && self.bm.contains(ix.row_ent[r]) {
                    m |= 1 << i;
                }
            }
            return m;
        }
        let w = row0 / 64;
        let o = row0 % 64;
        let lo = self.bits[w] >> o;
        let hi = if o > 48 { self.bits[w + 1] << (64 - o) } else { 0 };
        ((lo | hi) & 0xffff) as u16
    }
    #[inline]
    pub fn has(&self, ix: &Index, r: usize) -> bool {
        if self.probe {
            return self.bm.contains(ix.row_ent[r]);
        }
        self.bits[r / 64] >> (r % 64) & 1 == 1
    }
}

fn range_mask(row0: usize, a: usize, b: usize) -> u16 {
    let mut m = 0u16;
    for i in 0..16 {
        let r = row0 + i;
        if r >= a && r < b {
            m |= 1 << i;
        }
    }
    m
}

// ---------------- exact full-vector decisions ----------------

pub fn full_score(ix: &Index, pr: &Prep, row: usize, st: &mut Stats) -> f64 {
    st.full_reads += 1;
    match &ix.full {
        Full::Real { .. } => {
            let x = ix.full_row_f16(row);
            unsafe { dot_f16(&pr.q32, x) as f64 }
        }
        Full::Synth(s) => s.score(ix, pr, row),
    }
}
/// bound on |f32 score - exact score|
pub fn full_err(ix: &Index, pr: &Prep) -> f64 {
    ((ix.n() / 16 + 6) as f64) * U * pr.qn * ix.x2.sqrt() * 1.01 + 1e-12
}
pub fn exact_ge_row(ix: &Index, pr: &Prep, row: usize, t: f64, st: &mut Stats) -> bool {
    let s = full_score(ix, pr, row, st);
    let e = full_err(ix, pr);
    if s >= t + e {
        return true;
    }
    if s < t - e {
        return false;
    }
    st.exact_fallbacks += 1;
    match &ix.full {
        Full::Real { .. } => exact_ge(exact_dot(&pr.q16, ix.full_row_f16(row)), t),
        Full::Synth(sy) => sy.exact_score(ix, pr, row) >= t,
    }
}
pub fn exact_value(ix: &Index, pr: &Prep, row: usize) -> f64 {
    match &ix.full {
        Full::Real { .. } => exact_to_f64(exact_dot(&pr.q16, ix.full_row_f16(row))),
        Full::Synth(sy) => sy.exact_score(ix, pr, row),
    }
}

// ---------------- the threshold query ----------------

pub struct ThrOut {
    pub rows: Vec<u32>,
    pub st: Stats,
    pub iv: Vec<(u32, u8, f32, f32, u8)>,
    pub exact: Vec<(u32, f64)>,
    pub rec: Vec<(u8, u32, u16)>,
    pub full_rows: Vec<u32>,
}

pub struct Opts<'a> {
    pub vis: Option<&'a Vis>,
    pub verify: bool,
    pub want_iv: bool,
    pub rec: bool,
}

fn truth_block(ix: &Index, pr: &[&Prep], row0: usize, nvalid: usize) -> Vec<f64> {
    let n = ix.n();
    let mut tr = vec![0f64; pr.len() * 16];
    let mut x = vec![0f64; n];
    for i in 0..nvalid {
        ix.full_row_f64(row0 + i, &mut x);
        for (q, p) in pr.iter().enumerate() {
            tr[q * 16 + i] = match &ix.full {
                Full::Real { .. } => dot64(&p.q64, &x),
                Full::Synth(sy) => sy.exact_score(ix, p, row0 + i),
            };
        }
    }
    tr
}

/// One cluster of a single-query threshold query.
pub fn thr_cluster(ix: &Index, ci: usize, pr: &Prep, t: f64, o: &Opts, acc: &mut ThrOut) {
    let c = &ix.cl[ci];
    let nl = ix.meta.nl();
    if let Some(v) = o.vis {
        if v.cl_cnt[ci] == 0 {
            acc.st.clusters_novis += 1;
            return;
        }
    }
    let k = ix.k();
    let a = pr.anchor;
    let (hi, lo) = (ix.hi[a * k + ci] as f64, ix.lo[a * k + ci] as f64);
    let b = pr.beta;
    let (bh, bl) = if b >= 0.0 { (b * hi, b * lo) } else { (b * lo, b * hi) };
    let rr = ix.x2.sqrt();
    if bh + pr.rho * rr < t - 1e-9 {
        acc.st.clusters_out += 1;
        return;
    }
    let push_rows = |acc: &mut ThrOut, r0: usize, r1: usize| {
        for r in r0..r1 {
            if o.vis.map_or(true, |v| v.has(ix, r)) {
                acc.rows.push(r as u32);
            }
        }
    };
    if bl - pr.rho * rr >= t + 1e-9 {
        acc.st.clusters_in += 1;
        push_rows(acc, c.r0, c.r1);
        if o.want_iv {
            for r in c.r0..c.r1 {
                if o.vis.map_or(true, |v| v.has(ix, r)) {
                    acc.iv.push((r as u32, 0, (bl - pr.rho * rr) as f32, (bh + pr.rho * rr) as f32, 255));
                }
            }
        }
        return;
    }
    acc.st.clusters_open += 1;
    let qm = dot64(&pr.q64, &ix.centres_f64(ci));
    let cu = cut_params(ix, ci, pr, qm);
    let (u0, u1, d0, d1) = sorted_range(ix, ci, &cu, t);
    acc.st.range_out += ((u0 - c.r0) + (c.r1 - u1)) as u64;
    acc.st.range_in += ((d0 - u0) + (u1 - d1)) as u64;
    push_rows(acc, u0, d0);
    push_rows(acc, d1, u1);
    if o.want_iv {
        for r in (u0..d0).chain(d1..u1) {
            if o.vis.map_or(true, |v| v.has(ix, r)) {
                let (l, h) = key_interval(ix, ci, &cu, r);
                acc.iv.push((r as u32, 0, l as f32, h as f32, 254));
            }
        }
    }
    if d0 >= d1 {
        return;
    }
    let gq = get_gq(ix, pr, c.g, &mut acc.st);
    let cq = cluster_q(ix, ci, gq, pr, qm);
    let b0 = (d0 - c.r0) / LANES;
    let b1 = (d1 - c.r0 + LANES - 1) / LANES;
    let mut pend = Vec::new();
    let t32 = t as f32;
    // decisions in f32 must be conservative: lb >= t32 must imply lb >= t, ub < t32 imply ub < t
    let t_in = if (t32 as f64) < t { f32::from_bits(t32.to_bits() + if t32 >= 0.0 { 1 } else { u32::MAX }) } else { t32 };
    let _ = t_in;
    let tin = next_up_if_below(t32, t);
    for bk in b0..b1 {
        let row0 = c.r0 + bk * LANES;
        let mut m = range_mask(row0, d0, d1);
        if let Some(v) = o.vis {
            m &= v.mask16(ix, row0);
        }
        if m == 0 {
            continue;
        }
        acc.st.touched += m.count_ones() as u64;
        let truth = if o.verify { Some(truth_block(ix, &[pr], row0, (c.r1 - row0).min(16))) } else { None };
        let mut ivv = Vec::new();
        {
            let mut out = BlockOut { rows_in: &mut acc.rows, pend: &mut pend, iv: if o.want_iv { Some(&mut ivv) } else { None }, truth: truth.as_deref(), rec: if o.rec { Some(&mut acc.rec) } else { None } };
            unsafe { run_block::<1>(ix, ci, bk, &[&cq], &[tin], &[m], Mode::Thr, &mut acc.st, &mut out) };
        }
        acc.iv.extend(ivv);
    }
    let _ = nl;
    for (r, _) in pend {
        if o.rec {
            acc.full_rows.push(r);
        }
        let s = full_score(ix, pr, r as usize, &mut acc.st);
        let e = full_err(ix, pr);
        let inn = if s >= t + e {
            true
        } else if s < t - e {
            false
        } else {
            acc.st.exact_fallbacks += 1;
            exact_value(ix, pr, r as usize) >= t
        };
        if inn {
            acc.rows.push(r);
            if o.want_iv {
                let v = exact_value(ix, pr, r as usize);
                acc.exact.push((r, v));
            }
        }
    }
}

/// f32 threshold such that (x >= result in f32) implies x >= t and (x < result) implies x < t for f32 x.
pub fn next_up_if_below(t32: f32, t: f64) -> f32 {
    if (t32 as f64) >= t {
        // rounding went up: x >= t32 implies x >= t; x < t32 means x <= prev(t32) which may still be >= t.
        // prev(t32) < t holds since t32 is the nearest f32 to t from above unless t is exactly representable.
        t32
    } else {
        // t32 < t: step up one ulp
        if t32 >= 0.0 { f32::from_bits(t32.to_bits() + 1) } else { f32::from_bits(t32.to_bits() - 1) }
    }
}

pub fn threshold(ix: &Index, q: &[f32], t: f64, o: &Opts) -> ThrOut {
    let pr = prep(ix, q);
    threshold_prepped(ix, &pr, t, o)
}

pub fn threshold_prepped(ix: &Index, pr: &Prep, t: f64, o: &Opts) -> ThrOut {
    let nl = ix.meta.nl();
    let parts: Vec<ThrOut> = (0..ix.k())
        .into_par_iter()
        .fold(
            || ThrOut { rows: Vec::new(), st: Stats::new(nl), iv: Vec::new(), exact: Vec::new(), rec: Vec::new(), full_rows: Vec::new() },
            |mut acc, ci| {
                thr_cluster(ix, ci, pr, t, o, &mut acc);
                acc
            },
        )
        .collect();
    let mut out = ThrOut { rows: Vec::new(), st: Stats::new(nl), iv: Vec::new(), exact: Vec::new(), rec: Vec::new(), full_rows: Vec::new() };
    for p in parts {
        out.rows.extend(p.rows);
        out.st.add(&p.st);
        out.iv.extend(p.iv);
        out.exact.extend(p.exact);
        out.rec.extend(p.rec);
        out.full_rows.extend(p.full_rows);
    }
    out
}

/// Rows to a result bitmap over entity ids.
pub fn to_bitmap(ix: &Index, rows: &[u32]) -> RoaringBitmap {
    let mut e: Vec<u32> = rows.iter().map(|&r| ix.row_ent[(r & 0x3ff_ffff) as usize]).collect();
    e.sort_unstable();
    RoaringBitmap::from_sorted_iter(e.into_iter()).unwrap()
}

// ---------------- batched threshold queries (one pass over the resident levels) ----------------

pub fn threshold_batch<const B: usize>(ix: &Index, qs: &[Vec<f32>], ts: &[f64]) -> (Vec<Vec<u32>>, Stats) {
    assert_eq!(qs.len(), B);
    let n = ix.n();
    let nl = ix.meta.nl();
    let preps: Vec<Prep> = qs.iter().map(|q| prep(ix, q)).collect();
    // group setup for all queries, one pass over each basis
    let gqs: Vec<Vec<GQ>> = (0..ix.groups.len())
        .into_par_iter()
        .map(|g| {
            let mut qv = vec![vec![0f64; n]; B];
            let qrefs: Vec<&[f64]> = preps.iter().map(|p| p.q64.as_slice()).collect();
            unsafe { basis_matvec(ix_v(ix, g), n, &qrefs, &mut qv) };
            qv.into_iter().map(|v| group_q(ix, g, &preps[0].q32, v)).collect()
        })
        .collect();
    let parts: Vec<(Vec<Vec<u32>>, Stats)> = (0..ix.k())
        .into_par_iter()
        .fold(
            || (vec![Vec::new(); B], Stats::new(nl)),
            |(mut rows, mut st), ci| {
                let c = &ix.cl[ci];
                let k = ix.k();
                let mut d = [(0usize, 0usize); B];
                let mut cqs: Vec<CQ> = Vec::with_capacity(B);
                let mut opened = false;
                let qm_all: Vec<f64> = preps.iter().map(|p| dot64(&p.q64, &ix.centres_f64(ci))).collect();
                for q in 0..B {
                    let pr = &preps[q];
                    let t = ts[q];
                    let a = pr.anchor;
                    let (hi, lo) = (ix.hi[a * k + ci] as f64, ix.lo[a * k + ci] as f64);
                    let bt = pr.beta;
                    let (bh, bl) = if bt >= 0.0 { (bt * hi, bt * lo) } else { (bt * lo, bt * hi) };
                    let rr = ix.x2.sqrt();
                    let gq = &gqs[c.g][q];
                    cqs.push(cluster_q(ix, ci, gq, pr, qm_all[q]));
                    if bh + pr.rho * rr < t - 1e-9 {
                        continue;
                    }
                    if bl - pr.rho * rr >= t + 1e-9 {
                        rows[q].extend((c.r0..c.r1).map(|r| r as u32));
                        continue;
                    }
                    opened = true;
                    let cu = cut_params(ix, ci, pr, qm_all[q]);
                    let (u0, u1, d0, d1) = sorted_range(ix, ci, &cu, t);
                    rows[q].extend((u0..d0).chain(d1..u1).map(|r| r as u32));
                    d[q] = (d0, d1);
                }
                if !opened {
                    return (rows, st);
                }
                st.clusters_open += 1;
                let lo = d.iter().filter(|x| x.1 > x.0).map(|x| x.0).min();
                let hi = d.iter().filter(|x| x.1 > x.0).map(|x| x.1).max();
                if let (Some(lo), Some(hi)) = (lo, hi) {
                    let cref: [&CQ; B] = core::array::from_fn(|q| &cqs[q]);
                    let tv: [f32; B] = core::array::from_fn(|q| next_up_if_below(ts[q] as f32, ts[q]));
                    let mut pend = Vec::new();
                    let mut rin = Vec::new();
                    for bk in (lo - c.r0) / LANES..(hi - c.r0 + LANES - 1) / LANES {
                        let row0 = c.r0 + bk * LANES;
                        let act: [u16; B] = core::array::from_fn(|q| range_mask(row0, d[q].0, d[q].1));
                        let mut out = BlockOut { rows_in: &mut rin, pend: &mut pend, iv: None, truth: None, rec: None };
                        unsafe { run_block::<B>(ix, ci, bk, &cref, &tv, &act, Mode::Thr, &mut st, &mut out) };
                    }
                    for r in rin {
                        rows[(r >> 26) as usize].push(r & 0x3ff_ffff);
                    }
                    for (r, q) in pend {
                        if exact_ge_row(ix, &preps[q as usize], r as usize, ts[q as usize], &mut st) {
                            rows[q as usize].push(r);
                        }
                    }
                }
                (rows, st)
            },
        )
        .collect();
    let mut rows = vec![Vec::new(); B];
    let mut st = Stats::new(nl);
    for (r, s) in parts {
        for q in 0..B {
            rows[q].extend_from_slice(&r[q]);
        }
        st.add(&s);
    }
    (rows, st)
}

// ---------------- exact k-NN ----------------

pub struct KnnOut {
    pub ents: Vec<(f64, u32)>,
    pub st: Stats,
    pub tau1: f64,
    pub cands: usize,
    pub rounds: usize,
    pub t_seed: f64,
    pub t_sweep: f64,
    pub t_sel: f64,
}

/// Interval of one row at a given level (decoding from scratch), or exact at level nl.
fn row_interval(ix: &Index, pr: &Prep, row: usize, level: usize, st: &mut Stats) -> (f64, f64) {
    let nl = ix.meta.nl();
    if level >= nl {
        let v = exact_value(ix, pr, row);
        st.full_reads += 1;
        return (v, v);
    }
    let ci = ix.row_cluster(row);
    let c = &ix.cl[ci];
    let gq = get_gq(ix, pr, c.g, st);
    let qm = dot64(&pr.q64, &ix.centres_f64(ci));
    let cq = cluster_q(ix, ci, gq, pr, qm);
    let bk = (row - c.r0) / LANES;
    let lane = (row - c.r0) % LANES;
    let mut iv = Vec::new();
    let mut pend = Vec::new();
    let mut rin = Vec::new();
    let mut out = BlockOut { rows_in: &mut rin, pend: &mut pend, iv: Some(&mut iv), truth: None, rec: None };
    unsafe { run_block::<1>(ix, ci, bk, &[&cq], &[f32::NAN], &[1u16 << lane], Mode::Interval(level), st, &mut out) };
    let x = iv[0];
    (x.2 as f64, x.3 as f64)
}

pub fn knn(ix: &Index, q: &[f32], kk: usize, vis: Option<&Vis>) -> KnnOut {
    let tm = Timer::new();
    let nl = ix.meta.nl();
    let k = ix.k();
    let pr = prep(ix, q);
    let mut st = Stats::new(nl);
    let rr = ix.x2.sqrt();
    // ---- seed
    let a = pr.anchor;
    let mut cand: Vec<(f64, usize)> = (0..k)
        .filter(|&ci| vis.map_or(true, |v| v.cl_cnt[ci] > 0))
        .map(|ci| {
            let (hi, lo) = (ix.hi[a * k + ci] as f64, ix.lo[a * k + ci] as f64);
            let bh = if pr.beta >= 0.0 { pr.beta * hi } else { pr.beta * lo };
            (bh + pr.rho * rr, ci)
        })
        .collect();
    cand.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
    cand.truncate(64);
    let mut byc: Vec<(f64, usize)> = cand.iter().map(|&(_, ci)| (dot64(&pr.q64, &ix.centres_f64(ci)) / ix.cl[ci].mn, ci)).collect();
    byc.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
    let mut seed_iv: Vec<(u32, u8, f32, f32, u8)> = Vec::new();
    let mut nvis = 0usize;
    for &(_, ci) in &byc {
        let c = &ix.cl[ci];
        let gq = get_gq(ix, &pr, c.g, &mut st);
        let qm = dot64(&pr.q64, &ix.centres_f64(ci));
        let cq = cluster_q(ix, ci, gq, &pr, qm);
        for bk in 0..c.nblk {
            let row0 = c.r0 + bk * LANES;
            let mut m = range_mask(row0, c.r0, c.r1);
            if let Some(v) = vis {
                m &= v.mask16(ix, row0);
            }
            if m == 0 {
                continue;
            }
            nvis += m.count_ones() as usize;
            let mut pend = Vec::new();
            let mut rin = Vec::new();
            let mut out = BlockOut { rows_in: &mut rin, pend: &mut pend, iv: Some(&mut seed_iv), truth: None, rec: None };
            unsafe { run_block::<1>(ix, ci, bk, &[&cq], &[f32::NAN], &[m], Mode::Interval(3), &mut st, &mut out) };
        }
        if nvis >= 2000 {
            break;
        }
    }
    seed_iv.sort_by(|x, y| ((y.2 + y.3) as f64).partial_cmp(&((x.2 + x.3) as f64)).unwrap());
    let take = (2 * kk + 8).min(seed_iv.len());
    let mut ex: Vec<f64> = seed_iv[..take].iter().map(|x| exact_value(ix, &pr, x.0 as usize)).collect();
    st.full_reads += take as u64;
    ex.sort_by(|a, b| b.partial_cmp(a).unwrap());
    let tau1 = if ex.len() >= kk { ex[kk - 1] } else { f64::NEG_INFINITY };
    let t_seed = tm.ms();
    // ---- sweep at tau1 (items with score >= tau1 are candidates; tau1 <= true k-th best)
    let o = Opts { vis, verify: false, want_iv: true, rec: false };
    let sw = threshold_prepped(ix, &pr, tau1, &o);
    st.add(&sw.st);
    let t_sweep = tm.ms() - t_seed;
    // ---- selection
    // candidate: (row, lb, ub, next level to read, exact)
    let mut cands: Vec<(u32, f64, f64, usize, bool)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for &(r, v) in &sw.exact {
        if seen.insert(r) {
            cands.push((r, v, v, nl, true));
        }
    }
    for &(r, _, lb, ub, lv) in &sw.iv {
        if !seen.insert(r) {
            continue;
        }
        let next = if lv >= 254 { 0usize } else { lv as usize + 1 };
        cands.push((r, lb as f64, ub as f64, next, false));
    }
    let ncand = cands.len();
    let mut rounds = 0;
    loop {
        rounds += 1;
        if cands.len() <= kk {
            break;
        }
        let mut lbs: Vec<f64> = cands.iter().map(|c| c.1).collect();
        lbs.sort_by(|a, b| b.partial_cmp(a).unwrap());
        let tau = lbs[kk - 1];
        cands.retain(|c| c.2 >= tau);
        if cands.len() <= kk {
            break;
        }
        let mut ubs: Vec<f64> = cands.iter().map(|c| c.2).collect();
        ubs.sort_by(|a, b| b.partial_cmp(a).unwrap());
        let mut refined = 0;
        for i in 0..cands.len() {
            let c = cands[i];
            if c.4 {
                continue;
            }
            // certified in: at most k items (itself included) can reach its lower bound
            let higher = ubs.partition_point(|&u| u >= c.1);
            if higher <= kk {
                continue;
            }
            refined += 1;
            let (l, h) = row_interval(ix, &pr, c.0 as usize, c.3, &mut st);
            let ex = c.3 >= nl;
            cands[i] = (c.0, l.max(c.1), h.min(c.2), c.3 + 1, ex);
        }
        if refined == 0 {
            break;
        }
    }
    // result: certified items, then exact ones by score (ties by entity id); a plain set
    let mut ubs: Vec<f64> = cands.iter().map(|c| c.2).collect();
    ubs.sort_by(|a, b| b.partial_cmp(a).unwrap());
    let mut fin: Vec<(f64, u32)> = cands.iter().map(|c| {
        let cert = !c.4 && ubs.partition_point(|&u| u >= c.1) <= kk;
        let v = if c.4 { c.1 } else if cert { f64::INFINITY } else { exact_value(ix, &pr, c.0 as usize) };
        (v, ix.row_ent[c.0 as usize])
    }).collect();
    fin.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(&b.1)));
    fin.truncate(kk);
    let t_sel = tm.ms() - t_seed - t_sweep;
    KnnOut { ents: fin, st, tau1, cands: ncand, rounds, t_seed, t_sweep, t_sel }
}

// ---------------- flat scans and truth ----------------

pub struct Flat<'a> {
    pub map: &'a [u8],
    pub off: usize,
    pub n: usize,
    pub nrows: usize,
    pub ent: &'a [i64], // numpy row -> entity
}
impl<'a> Flat<'a> {
    #[inline]
    pub fn row(&self, r: usize) -> &[u16] {
        let b = &self.map[self.off + r * self.n * 2..self.off + (r + 1) * self.n * 2];
        unsafe { std::slice::from_raw_parts(b.as_ptr() as *const u16, self.n) }
    }
    /// all f32 scores in numpy row order
    pub fn scores(&self, q32: &[f32]) -> Vec<f32> {
        let mut out = vec![0f32; self.nrows];
        out.par_chunks_mut(4096).enumerate().for_each(|(ch, o)| {
            let r0 = ch * 4096;
            let mut i = 0;
            while i + 4 <= o.len() {
                let s = unsafe { dot_f16_x4(q32, [self.row(r0 + i), self.row(r0 + i + 1), self.row(r0 + i + 2), self.row(r0 + i + 3)]) };
                o[i..i + 4].copy_from_slice(&s);
                i += 4;
            }
            while i < o.len() {
                o[i] = unsafe { dot_f16(q32, self.row(r0 + i)) };
                i += 1;
            }
        });
        out
    }
    /// exact threshold result (entity ids, sorted)
    pub fn threshold(&self, q16: &[u16], q32: &[f32], t: f64, err: f64, vis: Option<&RoaringBitmap>, ent_xs: &[u32]) -> Vec<u32> {
        let decide = |r: usize, s: f32| -> bool {
            let s = s as f64;
            if s >= t + err { true } else if s < t - err { false } else { exact_ge(exact_dot(q16, self.row(r)), t) }
        };
        let rows: Vec<u32> = match vis {
            None => Vec::new(),
            Some(bm) => bm.iter().map(|e| ent_xs[e as usize]).collect(),
        };
        let nr = if vis.is_some() { rows.len() } else { self.nrows };
        let rowof = |i: usize| if vis.is_some() { rows[i] as usize } else { i };
        let mut res: Vec<u32> = (0..(nr + 1023) / 1024)
            .into_par_iter()
            .flat_map_iter(|ch| {
                let (a, b) = (ch * 1024, ((ch + 1) * 1024).min(nr));
                let mut out = Vec::new();
                let mut i = a;
                while i + 4 <= b {
                    let rr = [rowof(i), rowof(i + 1), rowof(i + 2), rowof(i + 3)];
                    let s = unsafe { dot_f16_x4(q32, [self.row(rr[0]), self.row(rr[1]), self.row(rr[2]), self.row(rr[3])]) };
                    for k in 0..4 {
                        if decide(rr[k], s[k]) {
                            out.push(self.ent[rr[k]] as u32);
                        }
                    }
                    i += 4;
                }
                while i < b {
                    let r = rowof(i);
                    if decide(r, unsafe { dot_f16(q32, self.row(r)) }) {
                        out.push(self.ent[r] as u32);
                    }
                    i += 1;
                }
                out
            })
            .collect();
        res.sort_unstable();
        res
    }
    /// exact k-NN (score desc, entity asc)
    pub fn knn(&self, q16: &[u16], q32: &[f32], k: usize, err: f64, vis: Option<&RoaringBitmap>, ent_xs: &[u32]) -> Vec<(f64, u32)> {
        let rows: Vec<u32> = match vis {
            None => Vec::new(),
            Some(bm) => bm.iter().map(|e| ent_xs[e as usize]).collect(),
        };
        let nr = if vis.is_some() { rows.len() } else { self.nrows };
        let keep = k + 32;
        let rowof = |i: usize| if vis.is_some() { rows[i] as usize } else { i };
        let tops: Vec<Vec<(f32, u32)>> = (0..(nr + 1023) / 1024)
            .into_par_iter()
            .fold(Vec::new, |mut v: Vec<(f32, u32)>, ch| {
                let (a, b) = (ch * 1024, ((ch + 1) * 1024).min(nr));
                let mut push = |s: f32, r: usize, v: &mut Vec<(f32, u32)>| {
                    if v.len() < keep || s > v[v.len() - 1].0 {
                        let pos = v.partition_point(|x| x.0 >= s);
                        v.insert(pos, (s, r as u32));
                        if v.len() > keep {
                            v.pop();
                        }
                    }
                };
                let mut i = a;
                while i + 4 <= b {
                    let rr = [rowof(i), rowof(i + 1), rowof(i + 2), rowof(i + 3)];
                    let s = unsafe { dot_f16_x4(q32, [self.row(rr[0]), self.row(rr[1]), self.row(rr[2]), self.row(rr[3])]) };
                    for k in 0..4 {
                        push(s[k], rr[k], &mut v);
                    }
                    i += 4;
                }
                while i < b {
                    let r = rowof(i);
                    push(unsafe { dot_f16(q32, self.row(r)) }, r, &mut v);
                    i += 1;
                }
                v
            })
            .collect();
        let mut all: Vec<(f32, u32)> = tops.into_iter().flatten().collect();
        all.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        if all.is_empty() {
            return Vec::new();
        }
        let kth = all[(k - 1).min(all.len() - 1)].0 as f64;
        let mut ex: Vec<(f64, u32)> = all
            .iter()
            .filter(|x| x.0 as f64 >= kth - 2.0 * err)
            .map(|x| (exact_to_f64(exact_dot(q16, self.row(x.1 as usize))), self.ent[x.1 as usize] as u32))
            .collect();
        ex.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(&b.1)));
        ex.truncate(k);
        ex
    }
}

/// thresholds for answer sizes (fractions of N), exact: midpoint between the k-th and (k+1)-th exact scores
pub fn thresholds_for(fl: &Flat, q16: &[u16], scores: &[f32], err: f64, sizes: &[usize]) -> Vec<f64> {
    let kmax = *sizes.iter().max().unwrap() + 1;
    let mut s: Vec<(f32, u32)> = scores.iter().enumerate().map(|(i, &v)| (v, i as u32)).collect();
    let nth = kmax.min(s.len() - 1);
    s.select_nth_unstable_by(nth, |a, b| b.0.partial_cmp(&a.0).unwrap());
    let cut = s[nth].0 as f64 - 2.0 * err;
    let mut ex: Vec<f64> = scores
        .par_iter()
        .enumerate()
        .filter(|(_, &v)| v as f64 >= cut)
        .map(|(i, _)| exact_to_f64(exact_dot(q16, fl.row(i))))
        .collect();
    ex.sort_by(|a, b| b.partial_cmp(a).unwrap());
    sizes.iter().map(|&k| (ex[k - 1] + ex[k]) / 2.0).collect()
}
