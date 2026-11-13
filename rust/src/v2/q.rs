//! Queries on the fast-scan index: exact integer basis products, per-group and per-cluster setup, the threshold query
//! (cluster cut, sorted range, then breadth-first levels over compacted survivors with re-packed blocks), the planner,
//! intervals and exact k-NN, and synthetic items for the scale test.
use super::kern::*;
use super::*;
use crate::kernel::Stats;
use crate::query::Cut;
use roaring::RoaringBitmap;
use std::arch::x86_64::*;
use std::sync::OnceLock;

pub const U: f64 = 1.0 / 16777216.0;

/// Phases of the time breakdown (Stats::cyc), counted only when PROF is set.
pub const P_ANCHOR: usize = 0;
pub const P_CUT: usize = 1;
pub const P_BASIS: usize = 2;
pub const P_ROT: usize = 3;
pub const P_CLSETUP: usize = 4;
pub const P_TABLES: usize = 5;
pub const P_REPACK: usize = 6;
pub const P_DECODE: usize = 7;
pub const P_BOUNDS: usize = 8;
pub const P_FULL: usize = 9;
pub const P_INIT: usize = 10;
pub const P_PLAN: usize = 11;
pub static PROF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[inline(always)]
pub fn tick() -> u64 {
    if PROF.load(std::sync::atomic::Ordering::Relaxed) { unsafe { core::arch::x86_64::_rdtsc() } } else { 0 }
}

pub struct GQ2 {
    pub qv: Vec<f64>,      // V_q^T q, exact up to one rounding
    pub qvd2: Vec<f32>,    // NDL x n: qv * D_l^2
    pub qdt2: Vec<f32>,    // J x n: qv * dt^2 on the coordinates of block j, else 0
    pub wq: Vec<f32>,      // det: qv*sig by coordinate; rand: R_j (qv*dt) by position
    pub cq: [f64; NDL],    // sum qv^2 D_l^2
    pub qvt_pos: Vec<f64>, // qv*dt in position order
    pub a2: [f64; J],
    pub l1det: f64,        // |wq| sums (weight rounding bound)
    pub l1j: [f64; J],
    pub cyc: (u64, u64),   // basis product, rotations
    pub glut: Vec<OnceLock<LevelLut>>, // tables shared by the group's clusters (alpha = 0)
}

pub struct Prep2 {
    pub q16: Vec<u16>,
    pub q32: Vec<f32>,
    pub q64: Vec<f64>,
    pub qn: f64,
    pub qhi: Vec<i16>,
    pub qlo: Vec<i16>,
    pub anchor: usize,
    pub beta: f64,
    pub rho: f64,
    pub gq: Vec<OnceLock<GQ2>>,
}

pub fn prep2(ix: &Index2, q: &[f32]) -> Prep2 {
    let n = ix.n();
    let q16: Vec<u16> = q.iter().map(|&x| f32_to_f16(x)).collect();
    let q32: Vec<f32> = q16.iter().map(|&h| f16_to_f32(h)).collect();
    let q64: Vec<f64> = q32.iter().map(|&x| x as f64).collect();
    let qn = q64.iter().map(|x| x * x).sum::<f64>().sqrt();
    let fx: Vec<i64> = q16.iter().map(|&h| f16_fixed(h)).collect();
    let qhi = fx.iter().map(|&v| (v >> 13) as i16).collect();
    let qlo = fx.iter().map(|&v| (v & 8191) as i16).collect();
    let (mut best, mut ba) = (f64::NEG_INFINITY, 0);
    let mut pa = vec![0f64; ix.nanch];
    for a in 0..ix.nanch {
        pa[a] = unsafe { dot64v(&q64, &ix.anchors[a * n..(a + 1) * n]) };
    }
    for (a, &b) in pa.iter().enumerate() {
        if b > best {
            best = b;
            ba = a;
        }
    }
    let _ = n;
    let rho = (qn * qn - best * best).max(0.0).sqrt() * (1.0 + 1e-12) + 1e-12;
    Prep2 { q16, q32, q64, qn, qhi, qlo, anchor: ba, beta: best, rho, gq: (0..ix.groups.len()).map(|_| OnceLock::new()).collect() }
}

/// V_q^T q exactly: int8 basis x (q * 2^24 split into 13-bit halves), i32 sums (VNNI), then one rounding to f64.
/// Four basis vectors at a time share the query loads.
#[target_feature(enable = "avx512f,avx512bw,avx512dq,avx512vl,avx512vnni")]
pub unsafe fn qv_exact(v8t: &[i8], vsc: &[f64], n: usize, qhi: &[i16], qlo: &[i16], out: &mut [f64]) {
    assert!(n % 32 == 0 && n % 4 == 0);
    let mut c = 0;
    while c < n {
        let mut ah = [_mm512_setzero_si512(); 4];
        let mut al = [_mm512_setzero_si512(); 4];
        let mut i = 0;
        while i < n {
            let h = _mm512_loadu_si512(qhi.as_ptr().add(i) as *const _);
            let l = _mm512_loadu_si512(qlo.as_ptr().add(i) as *const _);
            for r in 0..4 {
                let x = _mm512_cvtepi8_epi16(_mm256_loadu_si256(v8t.as_ptr().add((c + r) * n + i) as *const _));
                ah[r] = _mm512_dpwssd_epi32(ah[r], x, h);
                al[r] = _mm512_dpwssd_epi32(al[r], x, l);
            }
            i += 32;
        }
        for r in 0..4 {
            let h = _mm512_reduce_add_epi32(ah[r]) as i64;
            let l = _mm512_reduce_add_epi32(al[r]) as i64;
            out[c + r] = vsc[c + r] * ((h * 8192 + l) as f64) * (1.0 / 16777216.0);
        }
        c += 4;
    }
}

/// f64 dot product, two 8-lane accumulators.
#[target_feature(enable = "avx512f,avx512bw,avx512dq,avx512vl")]
pub unsafe fn dot64v(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    let (mut s0, mut s1) = (_mm512_setzero_pd(), _mm512_setzero_pd());
    let mut i = 0;
    while i + 16 <= n {
        s0 = _mm512_fmadd_pd(_mm512_loadu_pd(a.as_ptr().add(i)), _mm512_loadu_pd(b.as_ptr().add(i)), s0);
        s1 = _mm512_fmadd_pd(_mm512_loadu_pd(a.as_ptr().add(i + 8)), _mm512_loadu_pd(b.as_ptr().add(i + 8)), s1);
        i += 16;
    }
    let mut r = _mm512_reduce_add_pd(_mm512_add_pd(s0, s1));
    while i < n {
        r += a[i] * b[i];
        i += 1;
    }
    r
}

/// y = R x for a row-major square f64 matrix (n a multiple of 8).
#[target_feature(enable = "avx512f,avx512bw,avx512dq,avx512vl")]
pub unsafe fn matvec64(r: &[f64], x: &[f64], y: &mut [f64]) {
    let n = x.len();
    for a in 0..n {
        let row = r.as_ptr().add(a * n);
        let (mut s0, mut s1) = (_mm512_setzero_pd(), _mm512_setzero_pd());
        let mut i = 0;
        while i + 16 <= n {
            s0 = _mm512_fmadd_pd(_mm512_loadu_pd(row.add(i)), _mm512_loadu_pd(x.as_ptr().add(i)), s0);
            s1 = _mm512_fmadd_pd(_mm512_loadu_pd(row.add(i + 8)), _mm512_loadu_pd(x.as_ptr().add(i + 8)), s1);
            i += 16;
        }
        while i + 8 <= n {
            s0 = _mm512_fmadd_pd(_mm512_loadu_pd(row.add(i)), _mm512_loadu_pd(x.as_ptr().add(i)), s0);
            i += 8;
        }
        let mut v = _mm512_reduce_add_pd(_mm512_add_pd(s0, s1));
        while i < n {
            v += *row.add(i) * x[i];
            i += 1;
        }
        y[a] = v;
    }
}

pub fn group_q2(ix: &Index2, g: usize, pr: &Prep2) -> GQ2 {
    group_q2r(ix, g, pr, true)
}

/// The group setup; with rot = false the rotated weights are left for a batched product (setup_groups).
pub fn group_q2r(ix: &Index2, g: usize, pr: &Prep2, rot: bool) -> GQ2 {
    let n = ix.n();
    let gp = &ix.groups[g];
    let t0 = tick();
    let mut qv = vec![0f64; n];
    unsafe { qv_exact(&gp.v8t, &gp.vsc, n, &pr.qhi, &pr.qlo, &mut qv) };
    let t1 = tick();
    let mut wq = vec![0f32; 2 * n];
    for c in 0..n {
        wq[c] = (qv[c] * gp.sig[c] as f64) as f32;
    }
    let mut cq = [0f64; NDL];
    for l in 0..NDL {
        cq[l] = (0..n).map(|c| qv[c] * qv[c] * (gp.d[l][c] as f64).powi(2)).sum();
    }
    let qvt_pos: Vec<f64> = (0..n).map(|pp| {
        let c = gp.ridx[pp] as usize;
        qv[c] * gp.dt[c] as f64
    }).collect();
    let mut a2 = [0f64; J];
    let mut l1j = [0f64; J];
    for j in 0..J {
        let nbj = gp.nb[j];
        let xs = &qvt_pos[gp.boff[j]..gp.boff[j + 1]];
        a2[j] = xs.iter().map(|x| x * x).sum();
        if !rot {
            continue;
        }
        let mut y = vec![0f64; nbj];
        unsafe { matvec64(&ix.rots[j], xs, &mut y) };
        for a in 0..nbj {
            wq[n + gp.boff[j] + a] = y[a] as f32;
            l1j[j] += y[a].abs();
        }
    }
    let t2 = tick();
    let l1det = wq[..n].iter().map(|x| x.abs() as f64).sum();
    let mut qvd2 = vec![0f32; NDL * n];
    for l in 0..NDL {
        for c in 0..n {
            qvd2[l * n + c] = (qv[c] * (gp.d[l][c] as f64).powi(2)) as f32;
        }
    }
    let mut qdt2 = vec![0f32; J * n];
    for j in 0..J {
        for pp in gp.boff[j]..gp.boff[j + 1] {
            let c = gp.ridx[pp] as usize;
            qdt2[j * n + c] = (qv[c] * (gp.dt[c] as f64).powi(2)) as f32;
        }
    }
    let t3 = tick();
    GQ2 { qv, qvd2, qdt2, wq, cq, qvt_pos, a2, l1det, l1j, cyc: (t1 - t0 + t3 - t2, t2 - t1), glut: (0..ix.meta.nl()).map(|_| OnceLock::new()).collect() }
}

/// Set up several groups at once: exact basis products in parallel, then one product per randomised block with the
/// rotation of all groups' query weights (each rotation is read once, not once per group).
pub fn setup_groups(ix: &Index2, pr: &Prep2, gs: &[usize], st: &mut Stats) {
    let gs: Vec<usize> = gs.iter().copied().filter(|&g| pr.gq[g].get().is_none()).collect();
    if gs.is_empty() {
        return;
    }
    let n = ix.n();
    let mut gqs: Vec<GQ2> = gs.par_iter().map(|&g| group_q2r(ix, g, pr, false)).collect();
    for gq in &gqs {
        st.cyc[P_BASIS] += gq.cyc.0 + gq.cyc.1;
    }
    let t0 = tick();
    let m = gs.len();
    let gp0 = &ix.groups[gs[0]];
    let ys: Vec<Vec<f64>> = (0..J).into_par_iter().map(|j| {
        let nbj = gp0.nb[j];
        let mut x = vec![0f64; nbj * m];
        for (i, gq) in gqs.iter().enumerate() {
            let gp = &ix.groups[gs[i]];
            for b in 0..nbj {
                x[b * m + i] = gq.qvt_pos[gp.boff[j] + b];
            }
        }
        let mut y = vec![0f64; nbj * m];
        unsafe {
            matrixmultiply::dgemm(nbj, nbj, m, 1.0, ix.rots[j].as_ptr(), nbj as isize, 1, x.as_ptr(), m as isize, 1, 0.0, y.as_mut_ptr(), m as isize, 1);
        }
        y
    }).collect();
    for (i, gq) in gqs.iter_mut().enumerate() {
        let gp = &ix.groups[gs[i]];
        for j in 0..J {
            let nbj = gp.nb[j];
            gq.l1j[j] = 0.0;
            for a in 0..nbj {
                let v = ys[j][a * m + i];
                gq.wq[n + gp.boff[j] + a] = v as f32;
                gq.l1j[j] += v.abs();
            }
        }
    }
    st.cyc[P_ROT] += tick() - t0;
    st.groups += m as u64;
    for (g, gq) in gs.into_iter().zip(gqs) {
        let _ = pr.gq[g].set(gq);
    }
}

pub fn get_gq2<'a>(ix: &Index2, pr: &'a Prep2, g: usize, st: &mut Stats) -> &'a GQ2 {
    pr.gq[g].get_or_init(|| {
        st.groups += 1;
        let gq = group_q2(ix, g, pr);
        st.cyc[P_BASIS] += gq.cyc.0;
        st.cyc[P_ROT] += gq.cyc.1;
        gq
    })
}

/// Per query, per opened cluster.
pub struct CQ2 {
    pub qm: f32,
    pub alr: f32,
    pub dwa: [f32; NDL],
    pub nd: [f32; J],
    pub fixed: f32,
    pub ewdet: f32, // weight-rounding bound, det (times s)
    pub ewj: [f32; J], // per block (times kappa)
    pub w: Vec<f32>,
    pub luts: Vec<Option<LevelLut>>,
    pub shared: bool, // tables from the group (alpha = 0)
}

/// f32 dot product (four 16-lane accumulators); |result - exact| <= 64 u sum |a_i b_i| for n <= 4096.
#[target_feature(enable = "avx512f,avx512bw,avx512dq,avx512vl")]
pub unsafe fn dot32(a: &[f32], b: &[f32]) -> f64 {
    let n = a.len();
    let mut acc = [_mm512_setzero_ps(); 4];
    let mut i = 0;
    while i + 64 <= n {
        for k in 0..4 {
            acc[k] = _mm512_fmadd_ps(_mm512_loadu_ps(a.as_ptr().add(i + 16 * k)), _mm512_loadu_ps(b.as_ptr().add(i + 16 * k)), acc[k]);
        }
        i += 64;
    }
    while i + 16 <= n {
        acc[0] = _mm512_fmadd_ps(_mm512_loadu_ps(a.as_ptr().add(i)), _mm512_loadu_ps(b.as_ptr().add(i)), acc[0]);
        i += 16;
    }
    let a4 = _mm512_add_ps(_mm512_add_ps(acc[0], acc[1]), _mm512_add_ps(acc[2], acc[3]));
    let lo = _mm512_cvtps_pd(_mm512_castps512_ps256(a4));
    let hi = _mm512_cvtps_pd(_mm256_castpd_ps(_mm512_extractf64x4_pd(_mm512_castps_pd(a4), 1)));
    let mut r = _mm512_reduce_add_pd(_mm512_add_pd(lo, hi));
    while i < n {
        r += a[i] as f64 * b[i] as f64;
        i += 1;
    }
    r
}

pub fn cluster_q2(ix: &Index2, ci: usize, gq: &GQ2, pr: &Prep2, qm: f64) -> CQ2 {
    cluster_q2a(ix, ci, gq, pr, qm, false)
}

/// alpha0: no multiple of the centre is taken out of the query, so every cluster of a group shares its tables.
pub fn cluster_q2a(ix: &Index2, ci: usize, gq: &GQ2, pr: &Prep2, qm: f64, alpha0: bool) -> CQ2 {
    let c = &ix.cl[ci];
    let n = ix.n();
    let gp = &ix.groups[c.g];
    // f32 products of f32-rounded factors: each sum is within 70 u sqrt(sum a^2 sum b^2) of the exact one
    let e = 70.0 * U;
    let mut ab = [0f64; J];
    for j in 0..J {
        ab[j] = unsafe { dot32(&c.mv32, &gq.qdt2[j * n..(j + 1) * n]) };
    }
    let alr = if alpha0 { 0.0 } else { (ab.iter().sum::<f64>() / c.btot) as f32 };
    let a = alr as f64;
    let mut dwa = [0f32; NDL];
    for l in 0..NDL {
        let aqm = unsafe { dot32(&c.mv32, &gq.qvd2[l * n..(l + 1) * n]) };
        let dw2 = gq.cq[l] - 2.0 * a * aqm + a * a * c.bm[l] + 2.0 * a.abs() * e * (c.bm[l] * gq.cq[l]).sqrt();
        dwa[l] = ((dw2.max(0.0) + 1e-12 * gq.cq[l]).sqrt() * (1.0 + 1e-7)) as f32;
    }
    let mut nd = [0f32; J];
    for j in 0..J {
        let v = gq.a2[j] - 2.0 * a * ab[j] + a * a * c.b2[j] + 2.0 * a.abs() * e * (gq.a2[j] * c.b2[j]).sqrt();
        nd[j] = ((v.max(0.0) + 1e-12 * gq.a2[j]).sqrt() * (1.0 + 1e-7)) as f32;
    }
    let al = alr;
    let mut w = Vec::with_capacity(2 * n);
    w.extend(gq.wq[..n].iter().zip(&c.mv32).zip(&gp.sig).map(|((&q, &m), &sg)| q - al * (m * sg)));
    w.extend(gq.wq[n..].iter().zip(&c.wmr).map(|(&q, &m)| q - al * m));
    let mut ewj = [0f32; J];
    for j in 0..J {
        ewj[j] = (4.0 * U * (gq.l1j[j] + a.abs() * c.l1m[1 + j] as f64) + 1e-30) as f32;
    }
    // |v_c| < 3.5 for a deterministic coordinate (outer tree centroid plus residual stages)
    let ewdet = (12.0 * U * (gq.l1det + a.abs() * c.l1m[0] as f64) + 1e-30) as f32;
    let fixed = a.abs() * c.mn * DA + gp.e_inv * (pr.qn + a.abs() * c.mn) * c.rmax + 4e-6;
    CQ2 { qm: qm as f32, alr, dwa, nd, fixed: fixed as f32, ewdet, ewj, w, luts: (0..ix.meta.nl()).map(|_| None).collect(), shared: alpha0 }
}

// ---------------- cut and range (as the tree index) ----------------

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
    let phi = r.atan2(p);
    let psi = tau.acos();
    let lo = (phi - psi).max(0.0);
    let hi = (phi + psi).min(std::f64::consts::PI);
    if lo > hi {
        return None;
    }
    Some((rr * hi.cos(), rr * lo.cos()))
}
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

pub fn cut_params2(ix: &Index2, ci: usize, pr: &Prep2, qm: f64) -> Cut {
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

pub fn sorted_range2(ix: &Index2, ci: usize, cu: &Cut, t: f64) -> (usize, usize, usize, usize) {
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

pub fn key_interval2(ix: &Index2, cu: &Cut, row: usize) -> (f64, f64) {
    let rr = ix.x2.sqrt();
    let a = ix.ahat[row] as f64 / 32767.0;
    let (lo, hi) = ((a - DA).max(-rr), (a + DA).min(rr));
    let f = |p: f64, r: f64, off: f64, sgn: f64| -> f64 {
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

pub struct Vis2 {
    pub bits: Vec<u64>,
    pub cl_cnt: Vec<u32>,
}
impl Vis2 {
    pub fn new(ix: &Index2, bm: &RoaringBitmap) -> Vis2 {
        let mut bits = vec![0u64; (ix.nrows + 64) / 64 + 1];
        for e in bm.iter() {
            let r = ix.ent_row[e as usize] as usize;
            bits[r / 64] |= 1 << (r % 64);
        }
        let cl_cnt = ix.cl.iter().map(|c| count_bits(&bits, c.r0, c.r1) as u32).collect();
        Vis2 { bits, cl_cnt }
    }
    #[inline]
    pub fn has(&self, r: usize) -> bool {
        self.bits[r / 64] >> (r % 64) & 1 == 1
    }
}
fn count_bits(bits: &[u64], a: usize, b: usize) -> usize {
    if a >= b {
        return 0;
    }
    let (wa, wb) = (a / 64, (b - 1) / 64);
    let lo = !0u64 << (a % 64);
    let hi = !0u64 >> (63 - (b - 1) % 64);
    if wa == wb {
        return (bits[wa] & lo & hi).count_ones() as usize;
    }
    let mut n = (bits[wa] & lo).count_ones() as usize + (bits[wb] & hi).count_ones() as usize;
    for w in wa + 1..wb {
        n += bits[w].count_ones() as usize;
    }
    n
}

// ---------------- exact full-vector decisions ----------------

pub fn full_err2(ix: &Index2, pr: &Prep2) -> f64 {
    ((ix.n() / 16 + 6) as f64) * U * pr.qn * ix.x2.sqrt() * 1.01 + 1e-12
}
pub fn exact_value2(ix: &Index2, pr: &Prep2, row: usize) -> f64 {
    match &ix.full {
        Full2::Real { .. } => exact_to_f64(exact_dot(&pr.q16, ix.full_row_f16(row))),
        Full2::Synth(s) => s.exact_score(ix, pr, row),
    }
}
/// Exact decision s(q, x) >= t from the full vector (f32 dot, exact integer product near the threshold).
pub fn exact_ge2(ix: &Index2, pr: &Prep2, row: usize, t: f64, st: &mut Stats) -> bool {
    st.full_reads += 1;
    match &ix.full {
        Full2::Real { .. } => {
            let s = unsafe { crate::kernel::dot_f16(&pr.q32, ix.full_row_f16(row)) } as f64;
            let e = full_err2(ix, pr);
            if s >= t + e {
                true
            } else if s < t - e {
                false
            } else {
                st.exact_fallbacks += 1;
                exact_ge(exact_dot(&pr.q16, ix.full_row_f16(row)), t)
            }
        }
        Full2::Synth(s) => s.exact_score(ix, pr, row) >= t,
    }
}

// ---------------- the level loop ----------------

#[derive(Clone, Copy, PartialEq)]
pub enum Mode2 {
    Thr,
    Interval(usize),
}

/// Survivor state, compacted after every level (structure of arrays, padded to whole blocks).
#[derive(Default)]
pub struct State {
    pub idx: Vec<u32>,
    pub s: Vec<f32>,
    pub mr: Vec<f32>,
    pub adet: Vec<f32>,
    pub aj: [Vec<f32>; J],
    pub yh: [Vec<f32>; J],
    pub kap: [Vec<f32>; J],
    pub eps: [Vec<f32>; J],
    pub truth: Vec<f64>,
    pub tmp: Vec<u8>,
    pub stmp: Vec<u8>,
}
impl State {
    /// Room for `cap` candidates; accumulators zeroed, lanes past the candidates empty. Everything else is written
    /// before it is read.
    fn reset(&mut self, cap: usize) {
        let full = (cap + 2 * BLK) / BLK * BLK;
        let grow = |v: &mut Vec<f32>| {
            if v.len() < full {
                v.resize(full, 0.0);
            }
        };
        if self.idx.len() < full {
            self.idx.resize(full, u32::MAX);
        }
        grow(&mut self.s);
        grow(&mut self.mr);
        grow(&mut self.adet);
        for j in 0..J {
            grow(&mut self.aj[j]);
            grow(&mut self.yh[j]);
            grow(&mut self.kap[j]);
            grow(&mut self.eps[j]);
        }
        if self.truth.len() < full {
            self.truth.resize(full, 0.0);
        }
        let used = (cap + BLK - 1) / BLK * BLK + BLK;
        self.adet[..used].fill(0.0);
        for j in 0..J {
            self.aj[j][..used].fill(0.0);
        }
        self.idx[cap..used].fill(u32::MAX);
    }
}

pub struct RunOut<'a> {
    pub rows_in: &'a mut Vec<u32>,
    pub pend: &'a mut Vec<u32>,
    pub iv: Option<&'a mut Vec<(u32, f32, f32, u8)>>,
}

#[inline(always)]
unsafe fn ld(v: &[f32], i: usize) -> __m512 {
    _mm512_loadu_ps(v.as_ptr().add(i))
}
/// Move the kept lanes of v[o..o+16] to v[w..]; a full 16-lane store, so lanes past the kept count are overwritten
/// (w <= o: only lanes already read).
#[inline(always)]
unsafe fn cs(v: &mut [f32], w: usize, m: u16, o: usize) {
    let x = _mm512_loadu_ps(v.as_ptr().add(o));
    _mm512_storeu_ps(v.as_mut_ptr().add(w), _mm512_maskz_compress_ps(m, x));
}
#[inline(always)]
unsafe fn arr(v: __m512) -> [f32; 16] {
    let mut o = [0f32; 16];
    _mm512_storeu_ps(o.as_mut_ptr(), v);
    o
}

/// Run the index path for candidates `cands` (local item indices, ascending) of one cluster. Decisions in Thr mode;
/// in Interval(L) mode every candidate's interval at level L goes to out.iv.
#[target_feature(enable = "avx512f,avx512bw,avx512dq,avx512vl,avx512vbmi")]
pub unsafe fn run_cluster(ix: &Index2, ci: usize, pr: &Prep2, gq: &GQ2, cq: &mut CQ2, cands: &[u32], t: f64, mode: Mode2, first_decide: usize, verify: bool, sv: &mut State, st: &mut Stats, out: &mut RunOut) {
    let ti = tick();
    let c = &ix.cl[ci];
    let gp = &ix.groups[c.g];
    let nl = ix.meta.nl();
    let n = ix.n();
    let nr = nl - NDL;
    // dense candidates keep their item positions (slot k = item a0 + k, holes for the rest), so blocks are source
    // blocks and need no re-packing; sparse ones are compacted
    let a0 = cands[0] / 32 * 32;
    let span = ((cands[cands.len() - 1] + 1 - a0) as usize + BLK - 1) / BLK * BLK;
    let mut aligned = cands.len() * 10 >= span * 6;
    let mut nsurv;
    if aligned {
        sv.reset(span);
        sv.idx[..span].fill(u32::MAX);
        for &i in cands {
            sv.idx[(i - a0) as usize] = i;
        }
        nsurv = span;
    } else {
        sv.reset(cands.len());
        sv.idx[..cands.len()].copy_from_slice(cands);
        nsurv = cands.len();
    }
    let mut xbuf = vec![0f64; if verify { n } else { 0 }];
    {
        // item scale and m.r from the stored key, with the encoder's f32 operations
        let ahp = ix.ahat.as_ptr().add(c.r0) as *const i32;
        let mn = _mm512_set1_ps(c.mn32);
        let one_mn2 = _mm512_set1_ps(1.0f32 + c.mn32 * c.mn32);
        let two_mn = _mm512_set1_ps(2.0f32 * c.mn32);
        let mn2 = _mm512_set1_ps(c.mn32 * c.mn32);
        let mut k = 0;
        while k < nsurv {
            let m0: u16 = if nsurv - k >= 16 { 0xffff } else { ((1u32 << (nsurv - k)) - 1) as u16 };
            let iv0 = _mm512_maskz_loadu_epi32(m0, sv.idx.as_ptr().add(k) as *const _);
            let m = m0 & !_mm512_cmpeq_epi32_mask(iv0, _mm512_set1_epi32(-1));
            let iv = _mm512_maskz_mov_epi32(m, iv0);
            // 16-bit keys read as the low half of a 32-bit gather at a 2-byte stride (the key array is padded)
            let raw = _mm512_mask_i32gather_epi32::<2>(_mm512_setzero_si512(), m, iv, ahp as *const _);
            let ah = _mm512_srai_epi32(_mm512_slli_epi32(raw, 16), 16);
            let a = _mm512_div_ps(_mm512_cvtepi32_ps(ah), _mm512_set1_ps(32767.0));
            let s2 = _mm512_sub_ps(one_mn2, _mm512_mul_ps(two_mn, a));
            let sc = _mm512_sqrt_ps(_mm512_div_ps(_mm512_max_ps(s2, _mm512_set1_ps(1e-12)), _mm512_set1_ps(gp.lamsum)));
            _mm512_storeu_ps(sv.s.as_mut_ptr().add(k), sc);
            _mm512_storeu_ps(sv.mr.as_mut_ptr().add(k), _mm512_sub_ps(_mm512_mul_ps(mn, a), mn2));
            k += 16;
        }
    }
    if verify {
        for k in 0..nsurv {
            let i = sv.idx[k];
            if i == u32::MAX {
                continue;
            }
            let row = c.r0 + i as usize;
            assert_eq!(sv.s[k], item_scale(ix.ahat[row], c.mn32, gp.lamsum));
            sv.truth[k] = match &ix.full {
                Full2::Real { .. } => {
                    ix.full_row_f64(row, &mut xbuf);
                    dot64(&pr.q64, &xbuf)
                }
                Full2::Synth(s) => s.exact_score(ix, pr, row),
            };
        }
    }
    st.cyc[P_INIT] += tick() - ti;
    let t32 = t as f32;
    let tin = crate::query::next_up_if_below(t32, t);
    let stop = if let Mode2::Interval(l) = mode { l } else { usize::MAX };
    let csg = _mm512_set1_ps(ix.meta.csg as f32);
    let one6 = _mm512_set1_ps(1.0 + 1e-6);
    let u16x = _mm512_set1_ps((16.0 * U) as f32);
    let mut dmxj = [0f32; J]; // relative rounding bound of each block's latest kappa
    let mut edet = 0f32; // cumulative table error of the deterministic part
    let mut ej = [0f32; J];
    let mut g = Gather { ident: None, nsrc: 0, src: [0; 32], mlo: [0; 32], mhi: [0; 32], idxlo: _mm512_setzero_si512(), idxhi: _mm512_setzero_si512(), shlo: 0, shhi: 0 };
    for l in 0..nl {
        if nsurv == 0 {
            break;
        }
        let lp = &gp.levels[l];
        let tt = tick();
        let lut: &LevelLut = if cq.shared {
            gq.glut[l].get_or_init(|| {
                let mut lut = LevelLut::default();
                build_lut(&gq.wq, lp, &ix.etab, ix.split, &mut lut);
                lut
            })
        } else {
            if cq.luts[l].is_none() {
                let mut lut = LevelLut::default();
                build_lut(&cq.w, lp, &ix.etab, ix.split, &mut lut);
                cq.luts[l] = Some(lut);
            }
            cq.luts[l].as_ref().unwrap()
        };
        st.cyc[P_TABLES] += tick() - tt;
        let skip = l < first_decide && !verify && mode == Mode2::Thr;
        for pl in &lut.parts {
            match pl.j {
                None => edet += pl.err,
                Some(j) => ej[j] += pl.err,
            }
        }
        let nb = (nsurv + BLK - 1) / BLK;
        let mut kept_tot = 0usize;
        st.blocks[l] += nb as u64;
        let cbase = ix.lcode[l].as_ptr().add(ix.coff[l][ci]);
        let sbase = ix.lscal[l].as_ptr().add(ix.soff[l][ci]);
        let (units, srows) = (lp.units, lp.srows);
        if sv.tmp.len() < units * 16 + 64 {
            sv.tmp.resize(units * 16 + 64, 0);
        }
        if sv.stmp.len() < srows * 16 + 64 {
            sv.stmp.resize(srows * 16 + 64, 0);
        }
        let lr = l.wrapping_sub(NDL);
        if l >= NDL {
            for &j in &gp.thchg[lr] {
                let e = eta_steps(gp.kstep[lr][j]);
                dmxj[j] = (e / (1.0 - e) + 1e-6) as f32;
            }
        }
        let mut wpos = 0usize;
        for b in 0..nb {
            let k0 = b * BLK;
            let t0 = tick();
            // the state arrays hold u32::MAX past the survivors, so a block can be read whole
            let l0 = _mm512_loadu_si512(sv.idx.as_ptr().add(k0) as *const _);
            let l1 = _mm512_loadu_si512(sv.idx.as_ptr().add(k0 + 16) as *const _);
            let none = _mm512_set1_epi32(-1);
            let act: u32 = (!_mm512_cmpeq_epi32_mask(l0, none)) as u32 | ((!_mm512_cmpeq_epi32_mask(l1, none)) as u32) << 16;
            st.enter[l] += act.count_ones() as u64;
            if act == 0 {
                continue;
            }
            if aligned {
                g.ident = Some((a0 as usize + k0) / BLK);
            } else {
                let mut lanes = [u32::MAX; 32];
                _mm512_storeu_si512(lanes.as_mut_ptr() as *mut _, l0);
                _mm512_storeu_si512(lanes.as_mut_ptr().add(16) as *mut _, l1);
                plan_gather(&lanes, &mut g);
            }
            let (cp, sp) = match g.ident {
                Some(s) => (cbase.add(s * units * 16), sbase.add(s * srows * 16)),
                None => {
                    gather_rows(&g, cbase, units, sv.tmp.as_mut_ptr());
                    gather_rows(&g, sbase, srows, sv.stmp.as_mut_ptr());
                    (sv.tmp.as_ptr(), sv.stmp.as_ptr())
                }
            };
            let t1 = tick();
            st.cyc[P_REPACK] += t1 - t0;
            let mut keep = [0u16; 2];
            let mut lbh = [_mm512_setzero_ps(); 2];
            let mut ubh = [_mm512_setzero_ps(); 2];
            // part values
            let mut pv: [[__m512; 2]; J + 1] = [[_mm512_setzero_ps(); 2]; J + 1];
            for pl in &lut.parts {
                let (mut s0, mut s1) = part_sums(cp.add(pl.g0 * 64), lut.hi.as_ptr().add(pl.g0 * 64), pl.groups);
                if ix.split {
                    let (l0, l1) = part_sums(cp.add(pl.g0 * 64), lut.lo.as_ptr().add(pl.g0 * 64), pl.groups);
                    s0 = _mm512_fmadd_ps(s0, _mm512_set1_ps(256.0), l0);
                    s1 = _mm512_fmadd_ps(s1, _mm512_set1_ps(256.0), l1);
                }
                let k = match pl.j {
                    None => 0,
                    Some(j) => 1 + j,
                };
                let (o, sc) = (_mm512_set1_ps(pl.off), _mm512_set1_ps(pl.scale));
                pv[k] = [_mm512_fmadd_ps(s0, sc, o), _mm512_fmadd_ps(s1, sc, o)];
            }
            let t2 = tick();
            st.cyc[P_DECODE] += t2 - t1;
            for h in 0..2 {
                let hm: u16 = (act >> (16 * h)) as u16;
                if hm == 0 {
                    continue;
                }
                let o = k0 + 16 * h;
                let s = ld(&sv.s, o);
                let alrmr = _mm512_fmadd_ps(_mm512_set1_ps(cq.alr), ld(&sv.mr, o), _mm512_set1_ps(cq.qm));
                let (lb, ub, est, hn, sd);
                if l < NDL {
                    let adet = _mm512_add_ps(ld(&sv.adet, o), pv[0][h]);
                    _mm512_storeu_ps(sv.adet.as_mut_ptr().add(o), adet);
                    if skip {
                        keep[h] = hm;
                        continue;
                    }
                    let (n0, n1) = row_nibbles(sp);
                    let code = if h == 0 { n0 } else { n1 };
                    let ntab = _mm512_loadu_ps(gp.ntab[l].as_ptr());
                    let mut rho = _mm512_mul_ps(_mm512_mul_ps(_mm512_set1_ps(gp.pconst[l]), s), _mm512_permutexvar_ps(code, ntab));
                    let em = _mm512_cmpeq_epi32_mask(code, _mm512_set1_epi32(15)) & hm;
                    if em != 0 {
                        let mut r = arr(rho);
                        for i in 0..16 {
                            if em >> i & 1 == 1 {
                                let row = (c.r0 + sv.idx[o + i] as usize) as u32;
                                r[i] = ix.esc[&(l as u8, 0u8, row)];
                            }
                        }
                        rho = _mm512_loadu_ps(r.as_ptr());
                    }
                    let sa = _mm512_mul_ps(s, adet);
                    est = _mm512_add_ps(alrmr, sa);
                    let fl = _mm512_mul_ps(u16x, _mm512_add_ps(_mm512_abs_ps(alrmr), _mm512_abs_ps(sa)));
                    let hh = _mm512_add_ps(_mm512_mul_ps(_mm512_mul_ps(_mm512_set1_ps(cq.dwa[l]), rho), one6), _mm512_add_ps(_mm512_set1_ps(cq.fixed), _mm512_fmadd_ps(s, _mm512_set1_ps(edet + cq.ewdet), fl)));
                    lb = _mm512_sub_ps(est, hh);
                    ub = _mm512_add_ps(est, hh);
                    hn = hh;
                    sd = _mm512_setzero_ps();
                } else {
                    if lr == 0 {
                        for j in 0..J {
                            let (a0, a1) = row_nibbles(sp.add(2 * j * 16));
                            let (b0, b1) = row_nibbles(sp.add((2 * j + 1) * 16));
                            let code = if h == 0 { _mm512_add_epi32(a0, _mm512_slli_epi32(b0, 4)) } else { _mm512_add_epi32(a1, _mm512_slli_epi32(b1, 4)) };
                            let pr_ = _mm512_mul_ps(_mm512_set1_ps(gp.ypredw[j]), s);
                            let tv = _mm512_i32gather_ps::<4>(_mm512_min_epi32(code, _mm512_set1_epi32(254)), gp.ytab[j].as_ptr() as *const _);
                            let mut y = _mm512_mul_ps(pr_, tv);
                            let em = _mm512_cmpeq_epi32_mask(code, _mm512_set1_epi32(255)) & hm;
                            if em != 0 {
                                let mut r = arr(y);
                                for i in 0..16 {
                                    if em >> i & 1 == 1 {
                                        r[i] = ix.esc[&(l as u8, 1 + j as u8, (c.r0 + sv.idx[o + i] as usize) as u32)];
                                    }
                                }
                                y = _mm512_loadu_ps(r.as_ptr());
                            }
                            _mm512_storeu_ps(sv.yh[j].as_mut_ptr().add(o), y);
                        }
                    }
                    for pl in &lut.parts {
                        let j = pl.j.unwrap();
                        let a = _mm512_add_ps(ld(&sv.aj[j], o), pv[1 + j][h]);
                        _mm512_storeu_ps(sv.aj[j].as_mut_ptr().add(o), a);
                    }
                    for (slot, &j) in gp.thchg[lr].iter().enumerate() {
                        let row = gp.rrow[lr][slot];
                        let kb = gp.kbits[lr][j] as usize;
                        let (a0, a1) = row_nibbles(sp.add(row * 16));
                        let (t0, t1) = row_nibbles(sp.add((row + kb / 4) * 16));
                        let mut kc = if h == 0 { a0 } else { a1 };
                        if kb == 8 {
                            let (b0, b1) = row_nibbles(sp.add((row + 1) * 16));
                            kc = _mm512_add_epi32(kc, _mm512_slli_epi32(if h == 0 { b0 } else { b1 }, 4));
                        }
                        let kesc = if kb == 8 { 255 } else { 15 };
                        let tc = if h == 0 { t0 } else { t1 };
                        let yv = ld(&sv.yh[j], o);
                        let mut kap = _mm512_mul_ps(yv, _mm512_i32gather_ps::<4>(_mm512_min_epi32(kc, _mm512_set1_epi32(kesc - 1)), gp.ktab[lr][j].as_ptr() as *const _));
                        let mut th = _mm512_permutexvar_ps(tc, _mm512_loadu_ps(gp.ttab[lr][j].as_ptr()));
                        let ek = _mm512_cmpeq_epi32_mask(kc, _mm512_set1_epi32(kesc)) & hm;
                        let et = _mm512_cmpeq_epi32_mask(tc, _mm512_set1_epi32(15)) & hm;
                        if ek | et != 0 {
                            let (mut rk, mut rt) = (arr(kap), arr(th));
                            for i in 0..16 {
                                let row = (c.r0 + sv.idx[o + i] as usize) as u32;
                                if ek >> i & 1 == 1 {
                                    rk[i] = ix.esc[&(l as u8, 8 + j as u8, row)];
                                }
                                if et >> i & 1 == 1 {
                                    rt[i] = ix.esc[&(l as u8, 16 + j as u8, row)];
                                }
                            }
                            kap = _mm512_loadu_ps(rk.as_ptr());
                            th = _mm512_loadu_ps(rt.as_ptr());
                        }
                        _mm512_storeu_ps(sv.kap[j].as_mut_ptr().add(o), kap);
                        _mm512_storeu_ps(sv.eps[j].as_mut_ptr().add(o), _mm512_mul_ps(_mm512_mul_ps(yv, th), _mm512_set1_ps(1.0 + 1e-6)));
                    }
                    if skip {
                        keep[h] = hm;
                        continue;
                    }
                    let sa = _mm512_mul_ps(s, ld(&sv.adet, o));
                    let mut e = _mm512_add_ps(alrmr, sa);
                    let mut hdet = _mm512_setzero_ps();
                    let mut hpar = _mm512_fmadd_ps(s, _mm512_set1_ps(edet + cq.ewdet), _mm512_set1_ps(cq.fixed));
                    let mut fl = _mm512_add_ps(_mm512_abs_ps(alrmr), _mm512_abs_ps(sa));
                    let mut var = _mm512_setzero_ps();
                    let rates = &gp.rates[lr];
                    for j in 0..J {
                        let ndv = _mm512_set1_ps(cq.nd[j]);
                        if rates[j] == 0.0 {
                            hdet = _mm512_fmadd_ps(ld(&sv.yh[j], o), ndv, hdet);
                            continue;
                        }
                        let kap = ld(&sv.kap[j], o);
                        let eb = _mm512_mul_ps(kap, ld(&sv.aj[j], o));
                        e = _mm512_add_ps(e, eb);
                        fl = _mm512_add_ps(fl, _mm512_abs_ps(eb));
                        hpar = _mm512_fmadd_ps(_mm512_set1_ps(dmxj[j]), _mm512_abs_ps(eb), hpar);
                        hpar = _mm512_fmadd_ps(kap, _mm512_set1_ps(ej[j] + cq.ewj[j]), hpar);
                        let ab = _mm512_mul_ps(ld(&sv.eps[j], o), ndv);
                        var = _mm512_fmadd_ps(_mm512_mul_ps(ab, ab), _mm512_set1_ps(1.0 / (gp.nb[j] - 1) as f32), var);
                    }
                    let sdv = _mm512_sqrt_ps(var);
                    let hnv = _mm512_add_ps(_mm512_add_ps(hdet, hpar), _mm512_mul_ps(u16x, fl));
                    let hh = _mm512_mul_ps(_mm512_fmadd_ps(csg, sdv, hnv), one6);
                    est = e;
                    lb = _mm512_sub_ps(e, hh);
                    ub = _mm512_add_ps(e, hh);
                    hn = hnv;
                    sd = sdv;
                }
                lbh[h] = lb;
                ubh[h] = ub;
                if verify {
                    let (lbs, ubs, es, hs, sds) = (arr(lb), arr(ub), arr(est), arr(hn), arr(sd));
                    for i in 0..16 {
                        if hm >> i & 1 == 0 {
                            continue;
                        }
                        let sc = sv.truth[o + i];
                        st.checked += 1;
                        if sc < lbs[i] as f64 - 1e-12 || sc > ubs[i] as f64 + 1e-12 {
                            if l < NDL { st.viol_det += 1 } else { st.viol_rand += 1 }
                        }
                        if l >= NDL && sds[i] > 0.0 {
                            let zz = ((sc - es[i] as f64).abs() - hs[i] as f64) / sds[i] as f64;
                            if zz > st.max_sd {
                                st.max_sd = zz;
                            }
                        }
                    }
                }
                if l == stop {
                    if let Some(iv) = out.iv.as_deref_mut() {
                        let (lbs, ubs) = (arr(lb), arr(ub));
                        for i in 0..16 {
                            if hm >> i & 1 == 1 {
                                iv.push(((c.r0 + sv.idx[o + i] as usize) as u32, lbs[i], ubs[i], l as u8));
                            }
                        }
                    }
                    continue;
                }
                let tv = _mm512_set1_ps(tin);
                let inm = _mm512_cmp_ps_mask::<_CMP_GE_OQ>(lb, tv) & hm;
                let outm = _mm512_cmp_ps_mask::<_CMP_LT_OQ>(ub, tv) & hm;
                if inm != 0 {
                    st.dec_in[l] += inm.count_ones() as u64;
                    let (lbs, ubs) = (arr(lb), arr(ub));
                    for i in 0..16 {
                        if inm >> i & 1 == 1 {
                            let row = (c.r0 + sv.idx[o + i] as usize) as u32;
                            out.rows_in.push(row);
                            if let Some(iv) = out.iv.as_deref_mut() {
                                iv.push((row, lbs[i], ubs[i], l as u8));
                            }
                        }
                    }
                }
                st.dec_out[l] += outm.count_ones() as u64;
                keep[h] = hm & !(inm | outm);
            }
            let _ = (lbh, ubh);
            st.cyc[P_BOUNDS] += tick() - t2;
            if l == stop {
                continue;
            }
            if aligned {
                // decided lanes become holes
                for h in 0..2 {
                    let hm: u16 = (act >> (16 * h)) as u16;
                    let gone = hm & !keep[h];
                    if gone != 0 {
                        _mm512_mask_storeu_epi32(sv.idx.as_mut_ptr().add(k0 + 16 * h) as *mut _, gone, _mm512_set1_epi32(-1));
                    }
                    kept_tot += keep[h].count_ones() as usize;
                }
                continue;
            }
            // compact the survivors' state (writes never pass the block being read)
            for h in 0..2 {
                let m = keep[h];
                if m == 0 {
                    continue;
                }
                let o = k0 + 16 * h;
                let w = wpos;
                if w == o && m == 0xffff {
                    wpos += 16;
                    continue;
                }
                _mm512_storeu_si512(sv.idx.as_mut_ptr().add(w) as *mut _, _mm512_maskz_compress_epi32(m, _mm512_loadu_si512(sv.idx.as_ptr().add(o) as *const _)));
                cs(&mut sv.s, w, m, o);
                cs(&mut sv.mr, w, m, o);
                cs(&mut sv.adet, w, m, o);
                if l >= NDL {
                    for j in 0..J {
                        cs(&mut sv.aj[j], w, m, o);
                        cs(&mut sv.yh[j], w, m, o);
                        cs(&mut sv.kap[j], w, m, o);
                        cs(&mut sv.eps[j], w, m, o);
                    }
                }
                if verify {
                    let mut k = w;
                    for i in 0..16 {
                        if m >> i & 1 == 1 {
                            sv.truth[k] = sv.truth[o + i];
                            k += 1;
                        }
                    }
                }
                wpos += m.count_ones() as usize;
            }
        }
        if l == stop {
            return;
        }
        if aligned {
            if kept_tot == 0 {
                nsurv = 0;
            } else if kept_tot * 10 < nsurv * 6 {
                // occupancy fell: compact once, then re-pack from here on
                let mut w = 0;
                let mut o = 0;
                while o < nsurv {
                    let iv = _mm512_loadu_si512(sv.idx.as_ptr().add(o) as *const _);
                    let m = !_mm512_cmpeq_epi32_mask(iv, _mm512_set1_epi32(-1));
                    if m != 0 {
                        _mm512_storeu_si512(sv.idx.as_mut_ptr().add(w) as *mut _, _mm512_maskz_compress_epi32(m, iv));
                        cs(&mut sv.s, w, m, o);
                        cs(&mut sv.mr, w, m, o);
                        cs(&mut sv.adet, w, m, o);
                        for j in 0..J {
                            cs(&mut sv.aj[j], w, m, o);
                            cs(&mut sv.yh[j], w, m, o);
                            cs(&mut sv.kap[j], w, m, o);
                            cs(&mut sv.eps[j], w, m, o);
                        }
                        if verify {
                            let mut kk = w;
                            for i in 0..16 {
                                if m >> i & 1 == 1 {
                                    sv.truth[kk] = sv.truth[o + i];
                                    kk += 1;
                                }
                            }
                        }
                        w += m.count_ones() as usize;
                    }
                    o += 16;
                }
                nsurv = w;
                aligned = false;
                for k in nsurv..(nsurv + BLK).min(sv.idx.len()) {
                    sv.idx[k] = u32::MAX;
                }
            }
        } else {
            nsurv = wpos;
            for k in nsurv..(nsurv + BLK).min(sv.idx.len()) {
                sv.idx[k] = u32::MAX;
            }
        }
        // single-pass rule: decide once at first_decide, the rest go to their full vectors
        if mode == Mode2::Thr && first_decide > 0 && l == first_decide {
            break;
        }
        let _ = nr;
    }
    let _ = ti;
    let mut np = 0;
    for k in 0..nsurv {
        if sv.idx[k] != u32::MAX {
            out.pend.push((c.r0 + sv.idx[k] as usize) as u32);
            np += 1;
        }
    }
    st.enter[nl] += np;
}

// ---------------- the threshold query with the planner ----------------

#[derive(Clone, Copy, Debug)]
pub enum PlanMode {
    Auto,
    Index,
    Flat,
}

/// Cost model of the planner, in microseconds, measured at 1M on one thread. d is the fraction of the visible items
/// of opened clusters that the cut and the sorted range leave undecided: a proxy for how deep candidates will go
/// (text-like queries leave most items undecided), known before any code is read.
#[derive(Clone, Copy, Debug)]
pub struct Plan {
    pub mode: PlanMode,
    pub full_us: f64,      // exact decision from a full vector, per candidate
    pub item_us0: f64,     // index path per candidate: item_us0 + item_us1 * d
    pub item_us1: f64,
    pub cl_us0: f64,       // index path per cluster (setup and tables): cl_us0 + cl_us1 * d
    pub cl_us1: f64,
    pub grp_us: f64,       // index path per group (exact basis product, rotated weights)
    pub scan_us: f64,      // certified scan of the visible set, per visible item
    pub cut_us: f64,       // cluster cut and sorted range, per cluster holding a visible item
    pub alpha0: bool,      // tables shared per group (no centre multiple taken out of the query)
    pub jump_level: usize, // when most items stay undecided (d > jump_d): no decisions before this level
    pub jump_d: f64,
}
impl Default for Plan {
    fn default() -> Self {
        Plan { mode: PlanMode::Auto, full_us: 0.20, item_us0: 0.02, item_us1: 0.08, cl_us0: 4.0, cl_us1: 20.0, grp_us: 140.0, scan_us: 0.20, cut_us: 2.5, alpha0: false, jump_level: 0, jump_d: 0.8 }
    }
}

pub struct Opts2<'a> {
    pub vis: Option<&'a Vis2>,
    pub verify: bool,
    pub want_iv: bool,
    pub plan: Plan,
}

#[derive(Default)]
pub struct ThrOut2 {
    pub rows: Vec<u32>,
    pub st: Stats,
    pub iv: Vec<(u32, f32, f32, u8)>,
    pub exact: Vec<(u32, f64)>,
    pub idx_clusters: u64,
    pub flat_clusters: u64,
    pub flat_items: u64,
    pub scanned: bool, // the visible set was scanned directly
}

/// Certified scan of the visible set: every visible row's full vector, exact near the threshold.
pub fn scan_visible(ix: &Index2, pr: &Prep2, t: f64, vis: &Vis2, want_exact: bool) -> ThrOut2 {
    let nl = ix.meta.nl();
    let nw = vis.bits.len();
    let chunk = 256;
    let parts: Vec<ThrOut2> = (0..(nw + chunk - 1) / chunk).into_par_iter().map(|ch| {
        let mut acc = ThrOut2 { st: Stats::new(nl), ..Default::default() };
        let mut rows = Vec::with_capacity(4096);
        for w in ch * chunk..((ch + 1) * chunk).min(nw) {
            let mut b = vis.bits[w];
            while b != 0 {
                let r = w * 64 + b.trailing_zeros() as usize;
                if r < ix.nrows {
                    rows.push(r as u32);
                }
                b &= b - 1;
            }
        }
        decide_full(ix, pr, t, &rows, want_exact, &mut acc);
        acc
    }).collect();
    let mut out = ThrOut2 { st: Stats::new(nl), scanned: true, ..Default::default() };
    for p in parts {
        out.rows.extend(p.rows);
        out.st.add(&p.st);
        out.exact.extend(p.exact);
    }
    out.flat_items = out.st.full_reads;
    out
}

/// Exact decisions from full vectors (four rows per pass of the query; exact integer products only near t).
pub fn decide_full(ix: &Index2, pr: &Prep2, t: f64, pend: &[u32], want_exact: bool, acc: &mut ThrOut2) {
    let mut k4 = 0;
    if let Full2::Real { .. } = &ix.full {
        let e = full_err2(ix, pr);
        while k4 + 4 <= pend.len() {
            let rr = [pend[k4], pend[k4 + 1], pend[k4 + 2], pend[k4 + 3]];
            let sc = unsafe { crate::kernel::dot_f16_x4(&pr.q32, rr.map(|r| ix.full_row_f16(r as usize))) };
            for q in 0..4 {
                acc.st.full_reads += 1;
                let s = sc[q] as f64;
                let inn = if s >= t + e {
                    true
                } else if s < t - e {
                    false
                } else {
                    acc.st.exact_fallbacks += 1;
                    exact_ge(exact_dot(&pr.q16, ix.full_row_f16(rr[q] as usize)), t)
                };
                if inn {
                    acc.rows.push(rr[q]);
                    if want_exact {
                        acc.exact.push((rr[q], exact_value2(ix, pr, rr[q] as usize)));
                    }
                }
            }
            k4 += 4;
        }
    }
    for &r in &pend[k4..] {
        if exact_ge2(ix, pr, r as usize, t, &mut acc.st) {
            acc.rows.push(r);
            if want_exact {
                acc.exact.push((r, exact_value2(ix, pr, r as usize)));
            }
        }
    }
}

enum Pre {
    Skip,
    In,
    Open { qm: f64, u0: usize, u1: usize, d0: usize, d1: usize, ncand: usize },
}

fn cands_of(ix: &Index2, ci: usize, d0: usize, d1: usize, vis: Option<&Vis2>) -> Vec<u32> {
    let c = &ix.cl[ci];
    (d0..d1).filter(|&r| vis.map_or(true, |v| v.has(r))).map(|r| (r - c.r0) as u32).collect()
}

pub fn threshold2(ix: &Index2, pr: &Prep2, t: f64, o: &Opts2) -> ThrOut2 {
    let nl = ix.meta.nl();
    let k = ix.k();
    let rr = ix.x2.sqrt();
    let pl = o.plan;
    if let (Some(v), PlanMode::Auto) = (o.vis, pl.mode) {
        let nvis: usize = v.cl_cnt.iter().map(|&x| x as usize).sum();
        let nvc = v.cl_cnt.iter().filter(|&&x| x > 0).count();
        if nvis as f64 * pl.scan_us <= nvc as f64 * pl.cut_us {
            return scan_visible(ix, pr, t, v, o.want_iv);
        }
    }
    // pass 1: cut and range per cluster
    let tc0 = tick();
    let pre: Vec<Pre> = (0..k).into_par_iter().map(|ci| {
        if let Some(v) = o.vis {
            if v.cl_cnt[ci] == 0 {
                return Pre::Skip;
            }
        }
        let a = pr.anchor;
        let (hi, lo) = (ix.hi[a * k + ci] as f64, ix.lo[a * k + ci] as f64);
        let b = pr.beta;
        let (bh, bl) = if b >= 0.0 { (b * hi, b * lo) } else { (b * lo, b * hi) };
        if bh + pr.rho * rr < t - 1e-9 {
            return Pre::Skip;
        }
        if bl - pr.rho * rr >= t + 1e-9 {
            return Pre::In;
        }
        let qm = dot64(&pr.q64, ix.centres_f64(ci));
        let cu = cut_params2(ix, ci, pr, qm);
        let (u0, u1, d0, d1) = sorted_range2(ix, ci, &cu, t);
        let ncand = match o.vis {
            None => d1 - d0,
            Some(v) => count_bits(&v.bits, d0, d1),
        };
        Pre::Open { qm, u0, u1, d0, d1, ncand }
    }).collect();
    let tc1 = tick();
    // the planner: per cluster the index path or full vectors, a group's setup only if its clusters' gains pay for it
    let mut use_idx = vec![false; k];
    let (mut ncand_all, mut nvis_all) = (0usize, 0usize);
    for (ci, p) in pre.iter().enumerate() {
        if let Pre::Open { ncand, .. } = p {
            ncand_all += ncand;
            nvis_all += o.vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize);
        }
    }
    let d = ncand_all as f64 / nvis_all.max(1) as f64;
    let first_decide = if matches!(pl.mode, PlanMode::Auto) && pl.jump_level > 0 && d > pl.jump_d { pl.jump_level } else { 0 };
    let mut grp_gain = vec![0f64; ix.groups.len()];
    for (ci, p) in pre.iter().enumerate() {
        if let Pre::Open { ncand, .. } = p {
            if *ncand == 0 {
                continue;
            }
            let gain = *ncand as f64 * (pl.full_us - pl.item_us0 - pl.item_us1 * d) - (pl.cl_us0 + pl.cl_us1 * d);
            use_idx[ci] = match pl.mode {
                PlanMode::Index => true,
                PlanMode::Flat => false,
                PlanMode::Auto => gain > 0.0,
            };
            if use_idx[ci] {
                grp_gain[ix.cl[ci].g] += gain;
            }
        }
    }
    if let PlanMode::Auto = pl.mode {
        for ci in 0..k {
            if use_idx[ci] && grp_gain[ix.cl[ci].g] < pl.grp_us {
                use_idx[ci] = false;
            }
        }
        // what remains costs every candidate's full vector less the kept gains; a plain scan may be cheaper
        if let Some(v) = o.vis {
            let nvis: usize = v.cl_cnt.iter().map(|&x| x as usize).sum();
            let gains: f64 = (0..ix.groups.len()).filter(|&g| grp_gain[g] >= pl.grp_us).map(|g| grp_gain[g] - pl.grp_us).sum();
            // (full vectors cluster by cluster cost about a quarter more per item than one scan)
            if ncand_all as f64 * pl.full_us - gains > 0.8 * nvis as f64 * pl.scan_us {
                let mut out = scan_visible(ix, pr, t, v, o.want_iv);
                // the cut pass already ran; its decisions are not needed
                out.st.clusters_open = 0;
                return out;
            }
        }
    }
    let mut gst = Stats::new(nl);
    {
        let mut need: Vec<usize> = (0..k).filter(|&ci| use_idx[ci]).map(|ci| ix.cl[ci].g).collect();
        need.sort_unstable();
        need.dedup();
        setup_groups(ix, pr, &need, &mut gst);
    }
    let parts: Vec<ThrOut2> = (0..k).into_par_iter().fold(
        || (ThrOut2 { st: Stats::new(nl), ..Default::default() }, State::default()),
        |(mut acc, mut sv), ci| {
            let c = &ix.cl[ci];
            let push = |acc: &mut ThrOut2, r0: usize, r1: usize, iv: Option<(f64, f64)>| {
                for r in r0..r1 {
                    if o.vis.map_or(true, |v| v.has(r)) {
                        acc.rows.push(r as u32);
                        if let Some(x) = iv {
                            acc.iv.push((r as u32, x.0 as f32, x.1 as f32, 255));
                        }
                    }
                }
            };
            match pre[ci] {
                Pre::Skip => {
                    acc.st.clusters_out += 1;
                }
                Pre::In => {
                    acc.st.clusters_in += 1;
                    let iv = if o.want_iv { Some((-1.0, 1.0)) } else { None };
                    push(&mut acc, c.r0, c.r1, iv);
                    if o.want_iv {
                        // interval from the table alone
                        let a = pr.anchor;
                        let (hi, lo) = (ix.hi[a * k + ci] as f64, ix.lo[a * k + ci] as f64);
                        let b = pr.beta;
                        let (bh, bl) = if b >= 0.0 { (b * hi, b * lo) } else { (b * lo, b * hi) };
                        let nn = acc.iv.len();
                        let cnt = (c.r0..c.r1).filter(|&r| o.vis.map_or(true, |v| v.has(r))).count();
                        for x in &mut acc.iv[nn - cnt..] {
                            x.1 = (bl - pr.rho * rr) as f32;
                            x.2 = (bh + pr.rho * rr) as f32;
                        }
                    }
                }
                Pre::Open { qm, u0, u1, d0, d1, .. } => {
                    acc.st.clusters_open += 1;
                    acc.st.range_out += ((u0 - c.r0) + (c.r1 - u1)) as u64;
                    acc.st.range_in += ((d0 - u0) + (u1 - d1)) as u64;
                    if o.want_iv {
                        let cu = cut_params2(ix, ci, pr, qm);
                        for r in (u0..d0).chain(d1..u1) {
                            if o.vis.map_or(true, |v| v.has(r)) {
                                acc.rows.push(r as u32);
                                let (lb, ub) = key_interval2(ix, &cu, r);
                                acc.iv.push((r as u32, lb as f32, ub as f32, 254));
                            }
                        }
                    } else {
                        push(&mut acc, u0, d0, None);
                        push(&mut acc, d1, u1, None);
                    }
                    if d0 < d1 {
                        let cands = cands_of(ix, ci, d0, d1, o.vis);
                        acc.st.touched += cands.len() as u64;
                        let mut pend = Vec::new();
                        if use_idx[ci] {
                            acc.idx_clusters += 1;
                            let gq = get_gq2(ix, pr, c.g, &mut acc.st);
                            let tq = tick();
                            let mut cq = cluster_q2a(ix, ci, gq, pr, qm, pl.alpha0);
                            acc.st.cyc[P_CLSETUP] += tick() - tq;
                            let mut ivv = Vec::new();
                            let mut out = RunOut { rows_in: &mut acc.rows, pend: &mut pend, iv: if o.want_iv { Some(&mut ivv) } else { None } };
                            unsafe { run_cluster(ix, ci, pr, gq, &mut cq, &cands, t, Mode2::Thr, first_decide, o.verify, &mut sv, &mut acc.st, &mut out) };
                            acc.iv.extend(ivv);
                        } else {
                            acc.flat_clusters += 1;
                            acc.flat_items += cands.len() as u64;
                            pend.extend(cands.iter().map(|&i| (c.r0 + i as usize) as u32));
                        }
                        let tf = tick();
                        decide_full(ix, pr, t, &pend, o.want_iv, &mut acc);
                        acc.st.cyc[P_FULL] += tick() - tf;
                    }
                }
            }
            (acc, sv)
        },
    ).map(|x| x.0).collect();
    let mut out = ThrOut2 { st: Stats::new(nl), ..Default::default() };
    out.st.cyc[P_CUT] += tc1 - tc0;
    out.st.add(&gst);
    for p in parts {
        out.rows.extend(p.rows);
        out.st.add(&p.st);
        out.iv.extend(p.iv);
        out.exact.extend(p.exact);
        out.idx_clusters += p.idx_clusters;
        out.flat_clusters += p.flat_clusters;
        out.flat_items += p.flat_items;
    }
    out
}

pub fn to_bitmap2(ix: &Index2, rows: &[u32]) -> RoaringBitmap {
    let mut e: Vec<u32> = rows.iter().map(|&r| ix.row_ent[r as usize]).collect();
    e.sort_unstable();
    RoaringBitmap::from_sorted_iter(e.into_iter()).unwrap()
}

// ---------------- exact k-NN ----------------

pub struct KnnOut2 {
    pub ents: Vec<(f64, u32)>,
    pub st: Stats,
    pub cands: usize,
    pub t_seed: f64,
    pub t_sweep: f64,
    pub t_sel: f64,
}

/// Interval of one row at a level (from scratch), or exact at level nl.
pub fn row_interval2(ix: &Index2, pr: &Prep2, row: usize, level: usize, st: &mut Stats) -> (f64, f64) {
    let nl = ix.meta.nl();
    if level >= nl {
        st.full_reads += 1;
        let v = exact_value2(ix, pr, row);
        return (v, v);
    }
    let ci = ix.cl.partition_point(|c| c.r1 <= row);
    let c = &ix.cl[ci];
    let gq = get_gq2(ix, pr, c.g, st);
    let qm = dot64(&pr.q64, ix.centres_f64(ci));
    let mut cq = cluster_q2(ix, ci, gq, pr, qm);
    let mut iv = Vec::new();
    let (mut rin, mut pend) = (Vec::new(), Vec::new());
    let mut sv = State::default();
    let mut out = RunOut { rows_in: &mut rin, pend: &mut pend, iv: Some(&mut iv) };
    unsafe { run_cluster(ix, ci, pr, gq, &mut cq, &[(row - c.r0) as u32], f64::NAN, Mode2::Interval(level), 0, false, &mut sv, st, &mut out) };
    (iv[0].1 as f64, iv[0].2 as f64)
}

pub fn knn2(ix: &Index2, q: &[f32], kk: usize, vis: Option<&Vis2>, plan: Plan) -> KnnOut2 {
    let tm = Timer::new();
    let nl = ix.meta.nl();
    let k = ix.k();
    let pr = prep2(ix, q);
    let mut st = Stats::new(nl);
    let rr = ix.x2.sqrt();
    let a = pr.anchor;
    let mut cand: Vec<(f64, usize)> = (0..k).filter(|&ci| vis.map_or(true, |v| v.cl_cnt[ci] > 0)).map(|ci| {
        let (hi, lo) = (ix.hi[a * k + ci] as f64, ix.lo[a * k + ci] as f64);
        let bh = if pr.beta >= 0.0 { pr.beta * hi } else { pr.beta * lo };
        (bh + pr.rho * rr, ci)
    }).collect();
    cand.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
    cand.truncate(64);
    let mut byc: Vec<(f64, usize)> = cand.iter().map(|&(_, ci)| (dot64(&pr.q64, ix.centres_f64(ci)) / ix.cl[ci].mn, ci)).collect();
    byc.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
    let mut seed_iv: Vec<(u32, f32, f32, u8)> = Vec::new();
    let mut nvis = 0;
    let mut sv = State::default();
    for &(_, ci) in &byc {
        let c = &ix.cl[ci];
        let cands = cands_of(ix, ci, c.r0, c.r1, vis);
        if cands.is_empty() {
            continue;
        }
        nvis += cands.len();
        let gq = get_gq2(ix, &pr, c.g, &mut st);
        let qm = dot64(&pr.q64, ix.centres_f64(ci));
        let mut cq = cluster_q2(ix, ci, gq, &pr, qm);
        let (mut rin, mut pend) = (Vec::new(), Vec::new());
        let mut out = RunOut { rows_in: &mut rin, pend: &mut pend, iv: Some(&mut seed_iv) };
        unsafe { run_cluster(ix, ci, &pr, gq, &mut cq, &cands, f64::NAN, Mode2::Interval(3), 0, false, &mut sv, &mut st, &mut out) };
        if nvis >= 2000 {
            break;
        }
    }
    seed_iv.sort_by(|x, y| ((y.1 + y.2) as f64).partial_cmp(&((x.1 + x.2) as f64)).unwrap());
    let take = (2 * kk + 8).min(seed_iv.len());
    let mut ex: Vec<f64> = seed_iv[..take].iter().map(|x| exact_value2(ix, &pr, x.0 as usize)).collect();
    st.full_reads += take as u64;
    ex.sort_by(|a, b| b.partial_cmp(a).unwrap());
    let tau1 = if ex.len() >= kk { ex[kk - 1] } else { f64::NEG_INFINITY };
    let t_seed = tm.ms();
    let o = Opts2 { vis, verify: false, want_iv: true, plan };
    let sw = threshold2(ix, &pr, tau1, &o);
    st.add(&sw.st);
    let t_sweep = tm.ms() - t_seed;
    let mut cands: Vec<(u32, f64, f64, usize, bool)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for &(r, v) in &sw.exact {
        if seen.insert(r) {
            cands.push((r, v, v, nl, true));
        }
    }
    for &(r, lb, ub, lv) in &sw.iv {
        if !seen.insert(r) {
            continue;
        }
        let next = if lv >= 254 { 0usize } else { lv as usize + 1 };
        cands.push((r, lb as f64, ub as f64, next, false));
    }
    let ncand = cands.len();
    loop {
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
            if ubs.partition_point(|&u| u >= c.1) <= kk {
                continue;
            }
            refined += 1;
            let (l, h) = row_interval2(ix, &pr, c.0 as usize, c.3, &mut st);
            cands[i] = (c.0, l.max(c.1), h.min(c.2), c.3 + 1, c.3 >= nl);
        }
        if refined == 0 {
            break;
        }
    }
    let mut ubs: Vec<f64> = cands.iter().map(|c| c.2).collect();
    ubs.sort_by(|a, b| b.partial_cmp(a).unwrap());
    let mut fin: Vec<(f64, u32)> = cands.iter().map(|c| {
        let cert = !c.4 && ubs.partition_point(|&u| u >= c.1) <= kk;
        let v = if c.4 { c.1 } else if cert { f64::INFINITY } else { exact_value2(ix, &pr, c.0 as usize) };
        (v, ix.row_ent[c.0 as usize])
    }).collect();
    fin.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(&b.1)));
    fin.truncate(kk);
    let t_sel = tm.ms() - t_seed - t_sweep;
    KnnOut2 { ents: fin, st, cands: ncand, t_seed, t_sweep, t_sel }
}

// ---------------- synthetic items (scale test) ----------------

/// Synthetic items defined in the int8 basis: x = m + V_q z with z drawn per item from the group's spectrum and a real
/// residual norm, scaled so that |m|^2 + 2 t mv.z + t^2 |z|^2 = 1 (|V_q z|^2 differs from |z|^2 by at most e_G |z|^2,
/// which the stored norm bound covers). q.x = q.m + (V_q^T q).z with V_q^T q exact.
pub struct Synth2 {
    pub sizes: Vec<usize>,
    pub rnorms: Vec<Vec<f32>>,
    pub mv: Vec<Vec<f64>>,
    pub mn: Vec<f64>,
    pub lamsc: Vec<Vec<f64>>,
    pub eg: Vec<f64>,
    pub ext: Vec<u32>,
    pub row_cl: Vec<u32>,
    pub n: usize,
    pub coff: Vec<usize>,
    pub grp: Vec<usize>,
    pub vq: Vec<Vec<f64>>, // per group, row-major V_q (for item queries)
}
impl Synth2 {
    pub fn gen(&self, c: usize, sid: u32, z: &mut [f64]) -> (i16, f64) {
        use rand::{Rng, SeedableRng};
        use rand_distr::StandardNormal;
        let g = self.grp[c];
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0x5eed_0000_0000 ^ sid as u64);
        let rn = &self.rnorms[c];
        let r = rn[rng.gen_range(0..rn.len())] as f64;
        let ls = &self.lamsc[g];
        let (mut zz, mut mz) = (0.0, 0.0);
        let mv = &self.mv[c];
        for k in 0..self.n {
            let xi: f64 = rng.sample(StandardNormal);
            z[k] = r * ls[k] * xi;
            zz += z[k] * z[k];
            mz += mv[k] * z[k];
        }
        let mn = self.mn[c];
        let (a2, b, cc) = (zz, 2.0 * mz, mn * mn - 1.0);
        let t = (-b + (b * b - 4.0 * a2 * cc).max(0.0).sqrt()) / (2.0 * a2);
        let mut mz2 = 0.0;
        for k in 0..self.n {
            z[k] *= t;
            mz2 += mv[k] * z[k];
        }
        let a = (mn * mn + mz2) / mn;
        ((a.clamp(-1.0, 1.0) * 32767.0).round() as i16, self.eg[g] * zz * t * t)
    }
    pub fn exact_score(&self, ix: &Index2, pr: &Prep2, row: usize) -> f64 {
        let c = self.row_cl[row] as usize;
        let mut st = Stats::new(0);
        let gq = get_gq2(ix, pr, ix.cl[c].g, &mut st);
        let mut z = vec![0f64; self.n];
        self.gen(c, self.ext[row], &mut z);
        dot64(&pr.q64, ix.centres_f64(c)) + dot64(&gq.qv, &z)
    }
    pub fn x_f64(&self, ix: &Index2, row: usize, out: &mut [f64]) {
        let c = self.row_cl[row] as usize;
        let n = self.n;
        let mut z = vec![0f64; n];
        self.gen(c, self.ext[row], &mut z);
        let v = &self.vq[self.grp[c]];
        let m = ix.centres_f64(c);
        for i in 0..n {
            out[i] = m[i] + (0..n).map(|k| v[i * n + k] * z[k]).sum::<f64>();
        }
    }
}

pub struct SynthSrc<'a> {
    pub sy: &'a Synth2,
}
impl<'a> Source2 for SynthSrc<'a> {
    fn items(&self, c: usize, _p: &Params, _gp: &GroupPlan2, _mhat: &[f64]) -> Items2 {
        let n = self.sy.n;
        let nc = self.sy.sizes[c];
        let mut z = vec![0f64; nc * n];
        let mut key: Vec<(i16, u32, usize)> = Vec::with_capacity(nc);
        let mut x2max = 0f64;
        let mut tmp = vec![0f64; n];
        for i in 0..nc {
            let sid = (self.sy.coff[c] + i) as u32;
            let (ah, dev) = self.sy.gen(c, sid, &mut tmp);
            x2max = x2max.max(dev + 1e-12);
            z[i * n..(i + 1) * n].copy_from_slice(&tmp);
            key.push((ah, sid, i));
        }
        key.sort();
        let mut zs = vec![0f64; nc * n];
        for (j, k) in key.iter().enumerate() {
            zs[j * n..(j + 1) * n].copy_from_slice(&z[k.2 * n..(k.2 + 1) * n]);
        }
        Items2 { ah: key.iter().map(|k| k.0).collect(), z: zs, ent: key.iter().map(|k| k.1).collect(), ext: key.iter().map(|k| k.1).collect(), x2max }
    }
}
