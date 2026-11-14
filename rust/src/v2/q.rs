//! Queries on the fast-scan index: exact integer basis products, per-group and per-cluster setup, the threshold query
//! (cluster cut, sorted range, then breadth-first levels over compacted survivors with re-packed blocks), the planner,
//! intervals and exact k-NN, and synthetic items for the scale test.
use super::kern::*;
use super::*;
use crate::kernel::Stats;
use crate::query::Cut;
use roaring::RoaringBitmap;
use crate::simd::*;
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
pub const P_SEL: usize = 12;
/// Enclosing totals, from which the breakdown derives the time between the phases above: all of run_cluster, all of
/// one cluster's work in run_planned, all of a query (set by the caller), and the merge of the clusters' outputs.
pub const P_RUNCL: usize = 13;
pub const P_CLUSTER: usize = 14;
pub const P_QUERY: usize = 15;
pub const P_MERGE: usize = 16;
/// Compaction of the survivors' state after each block and level (held-in-place holes or moved lanes).
pub const P_COMPACT: usize = 17;
/// Per (cluster, level) work outside the tables and blocks: the move to the randomised levels, the integer levels'
/// thresholds, the scalar steps.
pub const P_LVSETUP: usize = 18;
/// The group setup after the basis product: weights, norms and the per-level and per-block query vectors.
pub const P_GPOST: usize = 19;
/// Counts kept in Stats::cyc after the phases: (cluster, level) tables built, their units, the survivors entering the
/// levels whose tables were built, and the tables built for at most 8 survivors.
pub const C_TABLES: usize = 20;
pub const C_UNITS: usize = 21;
pub const C_TSURV: usize = 22;
pub const C_TSMALL: usize = 23;
/// Inside bounds+decisions at the randomised levels: the scalars' decode (|y| at the first, kappa and tan at rises).
pub const P_RKAP: usize = 24;
pub static PROF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// In verify mode, record (true - estimate) / half-width of the checked intervals of items in the answer (true score
/// at least the threshold) in Stats::errs.
pub static ERRS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Study only (VEXACT_DUMP): in verify mode, one record per (item, randomised level) checked, tagged with DUMP_TAG.
pub static DUMP: std::sync::Mutex<Option<std::io::BufWriter<std::fs::File>>> = std::sync::Mutex::new(None);
pub static DUMP_TAG: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
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
    pub cyc: (u64, u64),   // basis product, the rest of the group setup (rotations included when done per group)
    pub glut: Vec<OnceLock<LevelLut>>, // tables shared by the group's clusters (alpha = 0)
    pub gtab: Vec<OnceLock<GTab>>,     // f32 tables of wq per level, combined per cluster with the centre's
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
    pub dial: Vec<f32>, // per level, factor on the item intervals' upper half-width (1: certified; below: approximate)
}

/// The query rounded to f16, as f16 and as f32, and its norm.
pub fn query_f16(q: &[f32]) -> (Vec<u16>, Vec<f32>, f64) {
    let q16: Vec<u16> = q.iter().map(|&x| f32_to_f16(x)).collect();
    let q32: Vec<f32> = q16.iter().map(|&h| f16_to_f32(h)).collect();
    let qn = q32.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt();
    (q16, q32, qn)
}

pub fn prep2(ix: &Index2, q: &[f32]) -> Prep2 {
    prep2_qm(ix, q, None)
}

/// prep2, with q.m of every cluster given when the anchors are the clusters' centre directions (one per cluster): the
/// anchor is the cluster with the largest q.m / |m|, and only its product is taken again.
pub fn prep2_qm(ix: &Index2, q: &[f32], qm: Option<&[f64]>) -> Prep2 {
    let n = ix.n();
    let (q16, q32, qn) = query_f16(q);
    let q64: Vec<f64> = q32.iter().map(|&x| x as f64).collect();
    let fx: Vec<i64> = q16.iter().map(|&h| f16_fixed(h)).collect();
    let qhi = fx.iter().map(|&v| (v >> 13) as i16).collect();
    let qlo = fx.iter().map(|&v| (v & 8191) as i16).collect();
    let (mut best, mut ba) = (f64::NEG_INFINITY, 0);
    match qm {
        Some(qm) if ix.nanch == ix.k() && ix.nanch > 0 => {
            for (c, &v) in qm.iter().enumerate() {
                if v / ix.cl[c].mn > best {
                    best = v / ix.cl[c].mn;
                    ba = c;
                }
            }
            best = unsafe { dot64v(&q64, &ix.anchors[ba * n..(ba + 1) * n]) };
        }
        _ => {
            for a in 0..ix.nanch {
                let b = unsafe { dot64v(&q64, &ix.anchors[a * n..(a + 1) * n]) };
                if b > best {
                    best = b;
                    ba = a;
                }
            }
        }
    }
    let rho = (qn * qn - best * best).max(0.0).sqrt() * (1.0 + 1e-12) + 1e-12;
    Prep2 { q16, q32, q64, qn, qhi, qlo, anchor: ba, beta: best, rho, gq: (0..ix.groups.len()).map(|_| OnceLock::new()).collect(), dial: vec![1.0; ix.meta.nl()] }
}

/// V_q^T q exactly: int8 basis x (q * 2^24 split into 13-bit halves), i32 sums (VNNI), then one rounding to f64.
/// Four basis vectors at a time share the query loads.
#[cfg(target_feature = "avx512f")]
#[target_feature(enable = "avx512f,avx512bw,avx512dq,avx512vl,avx512vnni")]
pub unsafe fn qv_exact(v8t: &[i8], vsc: &[f64], n: usize, qhi: &[i16], qlo: &[i16], out: &mut [f64]) {
    assert!(n % 32 == 0 && n % 4 == 0);
    // each i32 lane sums n/16 products of an int8 and a 13-bit half, up to 128 * 8191 * 2 each
    assert!(n <= 2048, "the exact basis product sums in i32 and overflows past n = 2048; this index has n = {n}");
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
#[cfg(target_feature = "avx512f")]
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
#[cfg(target_feature = "avx512f")]
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

/// V_q^T q exactly: int8 basis x (q * 2^24 split into 13-bit halves), i32 sums (vpmaddwd), then one rounding to f64.
/// Four basis vectors at a time share the query loads; the next four are prefetched meanwhile (setup::qv_exact_r).
#[cfg(not(target_feature = "avx512f"))]
pub unsafe fn qv_exact(v8t: &[i8], vsc: &[f64], n: usize, qhi: &[i16], qlo: &[i16], out: &mut [f64]) {
    super::setup::qv_exact_r::<4, 1>(v8t, vsc, n, qhi, qlo, out)
}

#[cfg(not(target_feature = "avx512f"))]
#[inline(always)]
unsafe fn hsum_pd(v: __m256d) -> f64 {
    let x = _mm_add_pd(_mm256_castpd256_pd128(v), _mm256_extractf128_pd::<1>(v));
    _mm_cvtsd_f64(_mm_add_sd(x, _mm_unpackhi_pd(x, x)))
}

/// f64 dot product, four 4-lane accumulators.
#[cfg(not(target_feature = "avx512f"))]
#[target_feature(enable = "avx2,fma")]
pub unsafe fn dot64v(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    let mut s = [_mm256_setzero_pd(); 4];
    let mut i = 0;
    while i + 16 <= n {
        for k in 0..4 {
            s[k] = _mm256_fmadd_pd(_mm256_loadu_pd(a.as_ptr().add(i + 4 * k)), _mm256_loadu_pd(b.as_ptr().add(i + 4 * k)), s[k]);
        }
        i += 16;
    }
    let mut r = hsum_pd(_mm256_add_pd(_mm256_add_pd(s[0], s[1]), _mm256_add_pd(s[2], s[3])));
    while i < n {
        r += a[i] * b[i];
        i += 1;
    }
    r
}

/// y = R x for a row-major square f64 matrix.
#[cfg(not(target_feature = "avx512f"))]
#[target_feature(enable = "avx2,fma")]
pub unsafe fn matvec64(r: &[f64], x: &[f64], y: &mut [f64]) {
    let n = x.len();
    for a in 0..n {
        y[a] = dot64v(&r[a * n..(a + 1) * n], x);
    }
}

/// f32 dot product (eight 8-lane accumulators); |result - exact| <= 64 u sum |a_i b_i| for n <= 4096.
#[cfg(not(target_feature = "avx512f"))]
#[target_feature(enable = "avx2,fma")]
pub unsafe fn dot32(a: &[f32], b: &[f32]) -> f64 {
    let n = a.len();
    let mut acc = [_mm256_setzero_ps(); 8];
    let mut i = 0;
    while i + 64 <= n {
        for k in 0..8 {
            acc[k] = _mm256_fmadd_ps(_mm256_loadu_ps(a.as_ptr().add(i + 8 * k)), _mm256_loadu_ps(b.as_ptr().add(i + 8 * k)), acc[k]);
        }
        i += 64;
    }
    while i + 16 <= n {
        acc[0] = _mm256_fmadd_ps(_mm256_loadu_ps(a.as_ptr().add(i)), _mm256_loadu_ps(b.as_ptr().add(i)), acc[0]);
        acc[1] = _mm256_fmadd_ps(_mm256_loadu_ps(a.as_ptr().add(i + 8)), _mm256_loadu_ps(b.as_ptr().add(i + 8)), acc[1]);
        i += 16;
    }
    let mut t = [_mm256_setzero_ps(); 4];
    for k in 0..4 {
        t[k] = _mm256_add_ps(acc[k], acc[k + 4]);
    }
    let a8 = _mm256_add_ps(_mm256_add_ps(t[0], t[1]), _mm256_add_ps(t[2], t[3]));
    let mut r = hsum_pd(_mm256_add_pd(_mm256_cvtps_pd(_mm256_castps256_ps128(a8)), _mm256_cvtps_pd(_mm256_extractf128_ps::<1>(a8))));
    while i < n {
        r += a[i] as f64 * b[i] as f64;
        i += 1;
    }
    r
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
    // cq[l] = sum qv^2 D_l^2 in four partial sums (any order is within the 1e-12 cq slack of cluster_q2a), with
    // qvd2 = qv D_l^2 in the same pass
    let mut cq = [0f64; NDL];
    let mut qvd2 = vec![0f32; NDL * n];
    for l in 0..NDL {
        let d = &gp.d[l];
        let mut acc = [0f64; 4];
        for c4 in (0..n).step_by(4) {
            for k in 0..4 {
                let c = c4 + k;
                let d2 = (d[c] as f64) * (d[c] as f64);
                acc[k] += qv[c] * qv[c] * d2;
                qvd2[l * n + c] = (qv[c] * d2) as f32;
            }
        }
        cq[l] = (acc[0] + acc[1]) + (acc[2] + acc[3]);
    }
    let qvt_pos: Vec<f64> = (0..n).map(|pp| {
        let c = gp.ridx[pp] as usize;
        qv[c] * gp.dt[c] as f64
    }).collect();
    let mut a2 = [0f64; J];
    let mut l1j = [0f64; J];
    let mut qdt2 = vec![0f32; J * n];
    for j in 0..J {
        let nbj = gp.nb[j];
        let xs = &qvt_pos[gp.boff[j]..gp.boff[j + 1]];
        a2[j] = xs.iter().map(|x| x * x).sum();
        for pp in gp.boff[j]..gp.boff[j + 1] {
            let c = gp.ridx[pp] as usize;
            qdt2[j * n + c] = (qv[c] * (gp.dt[c] as f64).powi(2)) as f32;
        }
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
    let l1det = wq[..n].iter().map(|x| x.abs() as f64).sum();
    let t3 = tick();
    GQ2 { qv, qvd2, qdt2, wq, cq, qvt_pos, a2, l1det, l1j, cyc: (t1 - t0, t3 - t1), glut: (0..ix.meta.nl()).map(|_| OnceLock::new()).collect(), gtab: (0..ix.meta.nl()).map(|_| OnceLock::new()).collect() }
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
        st.cyc[P_BASIS] += gq.cyc.0;
        st.cyc[P_GPOST] += gq.cyc.1;
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
        st.cyc[P_GPOST] += gq.cyc.1;
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
    pub gtab: bool,   // tables combined from the group's and the centre's
    pub repack: f32,  // compact by the index's per-level occupancy thresholds times this (0: at 0.6)
    pub bail: f32,    // a threshold query sends its survivors to full vectors before a level of more than bail x live units (0: never)
    pub rdec: u32,    // threshold queries decide at randomised level lr only if bit lr is set (the others only read)
    pub tol: f64,     // threshold queries take an item in once its lower bound reaches t - tol (0: exact)
    pub int: Option<IntDet>,
}

/// The integer deterministic levels of one (query, cluster): the common table step, and per level the margins and
/// the sums through that level of the units' minima (B) and of the tables' error bounds (e).
pub struct IntDet {
    pub sigma: f32,
    pub rt: Vec<Option<RTab>>,
    pub b: [f64; NDL],
    pub e: [f64; NDL],
    pub q: [f64; NDL], // bound on the largest Theta through each level
}

impl CQ2 {
    /// Decide the deterministic levels of threshold queries on the integer path.
    /// With combined tables (set `gtab` first), the common step comes from the units' range bounds: the group's sum
    /// and largest plus |alpha| times the centre's.
    pub fn enable_int(&mut self, ix: &Index2, ci: usize, gq: &GQ2) {
        assert!(!self.shared, "the integer levels need the cluster's own tables");
        let gp = &ix.groups[ix.cl[ci].g];
        let sigma = if self.gtab {
            let ts: Vec<&GTab> = (0..NDL).map(|l| gq.gtab[l].get_or_init(|| unsafe { group_tab(&gq.wq, &gp.levels[l], &ix.etab) })).collect();
            let (s, m) = unsafe { det_spans(&ts, &ix.ctab[ci][..NDL], &gp.levels[..NDL], self.alr) };
            let units: usize = gp.levels[..NDL].iter().map(|l| l.units).sum();
            let budget = (256 * IHMAX - 3 * units as i64) as f64;
            ((s * (1.0 + 1.0 / 512.0) / budget).max(m * (1.0 + 1.0 / 512.0) / 65533.0) as f32).max(1e-30)
        } else {
            int_sigma(&self.w, &gp.levels[..NDL], &ix.etab)
        };
        self.int = Some(IntDet { sigma, rt: vec![None; NDL], b: [0.0; NDL], e: [0.0; NDL], q: [0.0; NDL] });
    }
}

/// f32 dot product (four 16-lane accumulators); |result - exact| <= 64 u sum |a_i b_i| for n <= 4096.
#[cfg(target_feature = "avx512f")]
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
    cluster_q2w(ix, ci, gq, pr, qm, alpha0, true)
}

/// As cluster_q2a; without `want_w` the combined weights w are left empty (tables combined from the group's and the
/// centre's, `gtab`, never read them).
pub fn cluster_q2w(ix: &Index2, ci: usize, gq: &GQ2, pr: &Prep2, qm: f64, alpha0: bool, want_w: bool) -> CQ2 {
    let c = &ix.cl[ci];
    let n = ix.n();
    let gp = &ix.groups[c.g];
    // f32 products of f32-rounded factors: each sum is within (n/64 + 8) u sqrt(sum a^2 sum b^2) of the exact one
    let e = ((n / 64 + 8) as f64).max(70.0) * U;
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
    let mut w = Vec::with_capacity(if want_w { 2 * n } else { 0 });
    if want_w {
        w.extend(gq.wq[..n].iter().zip(&c.mv32).zip(&gp.sig).map(|((&q, &m), &sg)| q - al * (m * sg)));
        w.extend(gq.wq[n..].iter().zip(&c.wmr).map(|(&q, &m)| q - al * m));
    }
    let mut ewj = [0f32; J];
    for j in 0..J {
        ewj[j] = (4.0 * U * (gq.l1j[j] + a.abs() * c.l1m[1 + j] as f64) + 1e-30) as f32;
    }
    // |v_c| < 3.5 for a deterministic coordinate (outer tree centroid plus residual stages)
    let ewdet = (12.0 * U * (gq.l1det + a.abs() * c.l1m[0] as f64) + 1e-30) as f32;
    let fixed = a.abs() * c.mn * DA + gp.e_inv * (pr.qn + a.abs() * c.mn) * c.rmax + 4e-6;
    CQ2 { qm: qm as f32, alr, dwa, nd, fixed: fixed as f32, ewdet, ewj, w, luts: (0..ix.meta.nl()).map(|_| None).collect(), shared: alpha0, gtab: false, repack: 0.0, bail: 0.0, rdec: u32::MAX, tol: 0.0, int: None }
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

/// The norm-sort bound of one cluster: q.x = q.m + alpha m.r + (qv - alpha mv).z + (q - alpha m).eps with |eps| <=
/// e_inv |r|, so for alpha in {qv.mv / |mv|^2, q.m / |m|^2, 0}, with b = m.r in [bmin, bmax] and nu >= |z|:
/// q.m + alpha b - D nu - E <= q.x <= q.m + alpha b + D nu + E, D >= |qv - alpha mv|, E >= e_inv (|q| + |alpha| |m|) rmax.
/// Both ends are monotone in nu, so the items decided in or out form a prefix of the cluster in sort order.
pub struct NuCut {
    pub qm: f64,
    pub al: [(f64, f64, f64); 3], // (alpha, D, E)
    pub bmin: f64,
    pub bmax: f64,
    pub numax: f64,
}
impl NuCut {
    pub fn new(ix: &Index2, ci: usize, pr: &Prep2, gq: &GQ2, qm: f64) -> NuCut {
        let c = &ix.cl[ci];
        let gp = &ix.groups[c.g];
        let qq: f64 = gq.qv.iter().map(|x| x * x).sum();
        let qmv: f64 = gq.qv.iter().zip(&c.mv32).map(|(a, &b)| a * b as f64).sum();
        let mm: f64 = c.mv32.iter().map(|&x| x as f64 * x as f64).sum();
        let al = [qmv / mm.max(1e-300), qm / (c.mn * c.mn), 0.0].map(|a: f64| {
            // mv32 is mv rounded to f32: |mv - mv32| <= 2^-24 |mv| per coordinate
            let d = (qq - 2.0 * a * qmv + a * a * mm).max(0.0).sqrt() * (1.0 + 1e-12) + 1e-12 * qq.sqrt() + a.abs() * c.mvn * 2f64.powi(-22);
            let e = gp.e_inv * (pr.qn + a.abs() * c.mn) * c.rmax + 1e-9;
            (a, d, e)
        });
        NuCut { qm, al, bmin: c.bmin, bmax: c.bmax, numax: c.numax }
    }
    /// (lower, upper) bound of q.x for an item with |z| <= nu.
    pub fn bounds(&self, nu: f64) -> (f64, f64) {
        let (mut lo, mut hi) = (f64::NEG_INFINITY, f64::INFINITY);
        for &(a, d, e) in &self.al {
            let (b0, b1) = (a * self.bmin, a * self.bmax);
            hi = hi.min(self.qm + b0.max(b1) + d * nu + e);
            lo = lo.max(self.qm + b0.min(b1) - d * nu - e);
        }
        (lo, hi)
    }
    /// The norms below which items are decided out (upper < t) and at or below which they are decided in (lower >= t).
    pub fn cutoffs(&self, t: f64) -> (f64, f64) {
        let (mut co, mut cin) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for &(a, d, e) in &self.al {
            let (b0, b1) = (a * self.bmin, a * self.bmax);
            co = co.max((t - self.qm - b0.max(b1) - e) / d.max(1e-300));
            cin = cin.max((self.qm + b0.min(b1) - e - t) / d.max(1e-300));
        }
        (co, cin)
    }
    #[inline]
    pub fn nu_of(&self, code: u16) -> f64 {
        self.numax * code as f64 / 65535.0
    }
}

/// The norm-sort cut and range of a cluster, as sorted_range2 returns them: rows [r0, u0) and [u1, r1) out, [u0, d0)
/// and [d1, u1) in, [d0, d1) undecided.
pub fn nu_range(ix: &Index2, ci: usize, nc: &NuCut, t: f64) -> (usize, usize, usize, usize) {
    let c = &ix.cl[ci];
    let key = &ix.nukey[c.r0..c.r1];
    let (co, cin) = nc.cutoffs(t);
    // a margin for the f64 evaluation of the cutoffs
    let kout = key.partition_point(|&k| nc.nu_of(k) < co - 1e-9 * (1.0 + co.abs()));
    if kout > 0 {
        return (c.r0 + kout, c.r1, c.r0 + kout, c.r1);
    }
    let kin = key.partition_point(|&k| nc.nu_of(k) <= cin - 1e-9 * (1.0 + cin.abs()));
    (c.r0, c.r1, c.r0 + kin, c.r1)
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
    pub rows: Vec<u32>, // the visible rows, ascending
}
impl Vis2 {
    pub fn new(ix: &Index2, bm: &RoaringBitmap) -> Vis2 {
        let mut bits = vec![0u64; (ix.nrows + 64) / 64 + 1];
        for e in bm.iter() {
            let r = ix.ent_row[e as usize] as usize;
            bits[r / 64] |= 1 << (r % 64);
        }
        let cl_cnt = ix.cl.iter().map(|c| count_bits(&bits, c.r0, c.r1) as u32).collect();
        let mut rows: Vec<u32> = bm.iter().map(|e| ix.ent_row[e as usize]).collect();
        rows.sort_unstable();
        Vis2 { bits, cl_cnt, rows }
    }
    /// The visible rows of `clusters` only (every row of them when vis is None), and the visible rows listed in
    /// `extra` for those clusters (a second assignment), once each.
    pub fn restrict(ix: &Index2, vis: Option<&Vis2>, clusters: &[usize], extra: Option<&[Vec<u32>]>) -> Vis2 {
        let mut bits = vec![0u64; (ix.nrows + 64) / 64 + 1];
        let mut rows: Vec<u32> = Vec::new();
        for &ci in clusters {
            let c = &ix.cl[ci];
            match vis {
                Some(v) => {
                    let a = v.rows.partition_point(|&r| (r as usize) < c.r0);
                    let b = v.rows.partition_point(|&r| (r as usize) < c.r1);
                    rows.extend_from_slice(&v.rows[a..b]);
                }
                None => rows.extend(c.r0 as u32..c.r1 as u32),
            }
        }
        for &r in &rows {
            bits[r as usize / 64] |= 1 << (r % 64);
        }
        if let Some(ex) = extra {
            for &ci in clusters {
                for &r in &ex[ci] {
                    let w = &mut bits[r as usize / 64];
                    if vis.map_or(true, |v| v.has(r as usize)) && *w >> (r % 64) & 1 == 0 {
                        *w |= 1 << (r % 64);
                        rows.push(r);
                    }
                }
            }
        }
        rows.sort_unstable();
        let mut cl_cnt = vec![0u32; ix.k()];
        let mut ci = 0;
        for &r in &rows {
            while ix.cl[ci].r1 <= r as usize {
                ci += 1;
            }
            cl_cnt[ci] += 1;
        }
        Vis2 { bits, cl_cnt, rows }
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
    full_err_qn(ix, pr.qn)
}
/// Bound on |f32 product - exact product| of a stored vector and a query of norm qn.
pub fn full_err_qn(ix: &Index2, qn: f64) -> f64 {
    ((ix.n() / 16 + 6) as f64) * U * qn * ix.x2.sqrt() * 1.01 + 1e-12
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
    /// decide at every level up to this one but keep every candidate (timing of the decision kernel)
    Sweep(usize),
}

/// Survivor state, compacted after every level (structure of arrays, padded to whole blocks).
#[derive(Default)]
pub struct State {
    pub idx: Vec<u32>,
    pub s: Vec<f32>,
    pub mr: Vec<f32>,
    pub adet: Vec<f32>,
    /// From the first randomised level on, in place of s, mr and adet: the estimate's constant part
    /// qm + alr mr + s adet, and the half-width's (fixed + s (edet + ewdet) + 16 u (|qm + alr mr| + |s adet|)).
    pub c0: Vec<f32>,
    pub h0: Vec<f32>,
    pub aj: [Vec<f32>; J],
    pub yh: [Vec<f32>; J],
    pub kap: [Vec<f32>; J],
    /// The block's variance term at its latest rise: (|y| tan (1 + 1e-6) nd)^2 / (nb - 1).
    pub var: [Vec<f32>; J],
    pub truth: Vec<f64>,
    pub ist: Vec<i16>,     // integer levels, by block (kern::ip): Hs, Ls, TinH, TinL, ToutH, ToutL
    pub a64: Vec<f64>,     // verify: the real deterministic table sum
    pub tmp: Vec<u8>,
    pub stmp: Vec<u8>,
    pub colq: super::col::ColQ,
    pub cslots: Vec<super::col::ColSlot>,
    pub luts: Vec<LevelLut>, // tables of finished clusters, their storage reused by the next
}
impl State {
    /// A table to build into (storage of an earlier one when there is one).
    pub fn lut(&mut self) -> LevelLut {
        self.luts.pop().unwrap_or_default()
    }
    /// Keep a finished cluster's tables for reuse.
    pub fn recycle(&mut self, cq: &mut CQ2) {
        self.luts.extend(cq.luts.iter_mut().filter_map(|x| x.take()));
    }
    /// Room for `cap` candidates; accumulators zeroed, lanes past the candidates empty. Everything else is written
    /// before it is read.
    pub fn reset(&mut self, cap: usize) {
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
        grow(&mut self.c0);
        grow(&mut self.h0);
        for j in 0..J {
            grow(&mut self.aj[j]);
            grow(&mut self.yh[j]);
            grow(&mut self.kap[j]);
            grow(&mut self.var[j]);
        }
        if self.truth.len() < full {
            self.truth.resize(full, 0.0);
            self.a64.resize(full, 0.0);
        }
        if self.ist.len() < full * IROWS {
            self.ist.resize(full * IROWS, 0);
        }
        let used = (cap + BLK - 1) / BLK * BLK + BLK;
        self.adet[..used].fill(0.0);
        for b in 0..used / BLK {
            self.ist[b * BLK * IROWS..b * BLK * IROWS + 2 * BLK].fill(0);
        }
        self.a64[..used].fill(0.0);
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

/// Append r0 + idx[i] for the lanes i of `m` (16 slots from `idx`), in order, with one compressed store.
#[inline(always)]
unsafe fn push_rows(v: &mut Vec<u32>, idx: *const u32, m: u16, r0: u32) {
    v.reserve(16);
    let x = _mm512_add_epi32(_mm512_loadu_si512(idx as *const _), _mm512_set1_epi32(r0 as i32));
    _mm512_storeu_si512(v.as_mut_ptr().add(v.len()) as *mut _, _mm512_maskz_compress_epi32(m, x));
    v.set_len(v.len() + m.count_ones() as usize);
}

/// Blocks ahead whose codes are prefetched on the aligned path.
const PF: usize = 8;

/// Nibble of slot i of a block in row u.
#[inline(always)]
unsafe fn nib(p: *const u8, u: usize, i: usize) -> usize {
    let b = *p.add(u * 16 + (i & 15));
    (if i < 16 { b & 15 } else { b >> 4 }) as usize
}

/// Compact the deterministic state of 16 slots: the integer accumulators and thresholds on the integer path, else the
/// float sum; in verify mode also the f64 sum.
#[inline(always)]
unsafe fn compact_det(sv: &mut State, int: bool, verify: bool, w: usize, m: u16, o: usize) {
    if int {
        cs_int(&mut sv.ist, w, m, o);
    } else {
        cs(&mut sv.adet, w, m, o);
    }
    if verify {
        let mut k = w;
        for i in 0..16 {
            if m >> i & 1 == 1 {
                sv.a64[k] = sv.a64[o + i];
                k += 1;
            }
        }
    }
}

/// Which randomised-level arrays of the survivors' state a compaction keeps (bit j: block j).
#[derive(Default, Clone, Copy)]
struct RandNeed {
    aj: u32,
    kap: u32, // kappa and the variance term
    yh: bool,
    c0: bool, // the constant parts c0 and h0
}

/// Compact the randomised-level state of 16 slots that a later level reads.
#[inline(always)]
unsafe fn compact_rand(sv: &mut State, w: usize, m: u16, o: usize, need: RandNeed) {
    for j in 0..J {
        if need.aj >> j & 1 == 1 {
            cs(&mut sv.aj[j], w, m, o);
        }
        if need.yh {
            cs(&mut sv.yh[j], w, m, o);
        }
        if need.kap >> j & 1 == 1 {
            cs(&mut sv.kap[j], w, m, o);
            cs(&mut sv.var[j], w, m, o);
        }
    }
    if need.c0 {
        cs(&mut sv.c0, w, m, o);
        cs(&mut sv.h0, w, m, o);
    }
}

/// Verify mode, a deterministic level of one block (bit i: slot k0 + i): count the decisions (in, out) that the
/// real-valued certificate, evaluated in f64 from the f64 table sum, does not imply, and the active lanes whose exact
/// score it does not contain.
unsafe fn check_det(ix: &Index2, ci: usize, pr: &Prep2, cq: &CQ2, l: usize, t: f64, sv: &State, k0: usize, sp: *const u8, act: u32, inm: u32, outm: u32, st: &mut Stats) {
    let c = &ix.cl[ci];
    let gp = &ix.groups[c.g];
    let (qm, alr, fixed, ew, dwa, d) = (cq.qm as f64, cq.alr as f64, cq.fixed as f64, cq.ewdet as f64, cq.dwa[l] as f64, pr.dial[l] as f64);
    for i in 0..32 {
        if act >> i & 1 == 0 {
            continue;
        }
        let k = k0 + i;
        let s = sv.s[k] as f64;
        let code = nib(sp, 0, i);
        let rho = if code == 15 { ix.esc[&(l as u8, 0u8, (c.r0 + sv.idx[k] as usize) as u32)] as f64 } else { gp.pconst[l] as f64 * s * gp.ntab[l][code] as f64 };
        let hh = dwa * rho * (1.0 + 1e-6) + fixed + s * ew;
        let est = qm + alr * sv.mr[k] as f64 + s * sv.a64[k];
        let tr = sv.truth[k];
        if tr < est - hh - 1e-12 || tr > est + hh + 1e-12 {
            st.cert_viol += 1;
        }
        if tr >= t && ERRS.load(std::sync::atomic::Ordering::Relaxed) {
            st.errs.push((l as u8, ((tr - est) / hh.max(1e-30)) as f32));
        }
        if inm >> i & 1 == 1 {
            st.imp_checked += 1;
            if !(est - hh >= t - cq.tol) {
                st.imp_fail += 1;
            }
        }
        if outm >> i & 1 == 1 {
            st.imp_checked += 1;
            if !(est + d * hh < t) {
                st.imp_fail += 1;
            }
        }
    }
}

/// Run the index path for candidates `cands` (local item indices, ascending) of one cluster. Decisions in Thr mode;
/// in Interval(L) mode every candidate's interval at level L goes to out.iv.
#[target_feature(enable = "avx2,fma")]
pub unsafe fn run_cluster(ix: &Index2, ci: usize, pr: &Prep2, gq: &GQ2, cq: &mut CQ2, cands: &[u32], t: f64, mode: Mode2, first_decide: usize, verify: bool, sv: &mut State, st: &mut Stats, out: &mut RunOut) {
    run_cluster_from(ix, ci, pr, gq, cq, cands, t, mode, first_decide, verify, sv, st, out, None)
}

/// As run_cluster; with `resume = Some((nsurv, edet, a0))` (a0: slots held in place from item a0, holes u32::MAX) the deterministic part is already done (the columnar path): the
/// state holds nsurv compacted survivors with idx, s, mr, adet (and truth in verify mode) set, edet bounds the error of
/// adet, and the walk starts at the first randomised level.
#[target_feature(enable = "avx2,fma")]
pub unsafe fn run_cluster_from(ix: &Index2, ci: usize, pr: &Prep2, gq: &GQ2, cq: &mut CQ2, cands: &[u32], t: f64, mode: Mode2, first_decide: usize, verify: bool, sv: &mut State, st: &mut Stats, out: &mut RunOut, resume: Option<(usize, f32, Option<u32>)>) {
    let ti = tick();
    let c = &ix.cl[ci];
    let gp = &ix.groups[c.g];
    let nl = ix.meta.nl();
    let n = ix.n();
    let nr = nl - NDL;
    // dense candidates keep their item positions (slot k = item a0 + k, holes for the rest), so blocks are source
    // blocks and need no re-packing; sparse ones are compacted
    let rp = cq.repack;
    let occ = |l: usize| if rp > 0.0 { (ix.repack[l] * rp).min(0.95) } else { 0.6 };
    let (a0, span) = if let Some((ns, _, a)) = resume { (a.unwrap_or(0), ns) } else {
        let a0 = cands[0] / 32 * 32;
        (a0, ((cands[cands.len() - 1] + 1 - a0) as usize + BLK - 1) / BLK * BLK)
    };
    let mut aligned = match resume { Some((_, _, a)) => a.is_some(), None => cands.len() as f32 >= span as f32 * occ(0) };
    let mut nsurv;
    if let Some((ns, _, _)) = resume {
        nsurv = ns;
    } else if aligned {
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
    if resume.is_none() {
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
            // 16-bit keys read as the low half of a 32-bit gather at a 2-byte stride (the key array is padded); slots
            // held in place are items a0 + k.., read in one load
            let r = c.r0 + a0 as usize + k;
            let raw = if aligned && r + 16 <= ix.ahat.len() {
                _mm512_maskz_mov_epi32(m, _mm512_cvtepi16_epi32(_mm256_loadu_si256(ix.ahat.as_ptr().add(r) as *const _)))
            } else {
                _mm512_mask_i32gather_epi32::<2>(_mm512_setzero_si512(), m, iv, ahp as *const _)
            };
            let ah = _mm512_srai_epi32(_mm512_slli_epi32(raw, 16), 16);
            let a = _mm512_div_ps(_mm512_cvtepi32_ps(ah), _mm512_set1_ps(32767.0));
            let s2 = _mm512_sub_ps(one_mn2, _mm512_mul_ps(two_mn, a));
            let sc = _mm512_sqrt_ps(_mm512_div_ps(_mm512_max_ps(s2, _mm512_set1_ps(1e-12)), _mm512_set1_ps(gp.lamsum)));
            _mm512_storeu_ps(sv.s.as_mut_ptr().add(k), sc);
            _mm512_storeu_ps(sv.mr.as_mut_ptr().add(k), _mm512_sub_ps(_mm512_mul_ps(mn, a), mn2));
            k += 16;
        }
    }
    if verify && resume.is_none() {
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
    let intm = resume.is_none() && cq.int.is_some() && !matches!(mode, Mode2::Interval(_)) && out.iv.is_none();
    if intm {
        // Tout is shared by the deterministic levels, so the fixed term takes their largest factor; a larger factor
        // only lowers Tout
        let dmax = pr.dial[..NDL].iter().fold(0f32, |a, &d| a.max(d)) as f64;
        let (qm, alr, fixed) = (cq.qm as f64, cq.alr as f64, cq.fixed as f64);
        let sigma = cq.int.as_ref().unwrap().sigma as f64;
        int_thresholds(&sv.s, &sv.mr, nsurv, t - cq.tol + fixed - qm, t - dmax * fixed - qm, alr, t.abs() + fixed.abs() + qm.abs(), sigma, &mut sv.ist);
    }
    st.cyc[P_INIT] += tick() - ti;
    let t32 = t as f32;
    let tout = crate::query::next_up_if_below(t32, t);
    // in once the lower bound reaches t - tol, out once the upper bound is below t
    let tin = if cq.tol > 0.0 && mode == Mode2::Thr { crate::query::next_up_if_below((t - cq.tol) as f32, t - cq.tol) } else { tout };
    let stop = match mode {
        Mode2::Interval(l) | Mode2::Sweep(l) => l,
        Mode2::Thr => usize::MAX,
    };
    let interval = matches!(mode, Mode2::Interval(_));
    let sweep = matches!(mode, Mode2::Sweep(_));
    let one6 = _mm512_set1_ps(1.0 + 1e-6);
    let u16x = _mm512_set1_ps((16.0 * U) as f32);
    let mut dmxj = [0f32; J]; // relative rounding bound of each block's latest kappa
    let mut edet = resume.map_or(0f32, |r| r.1); // cumulative table error of the deterministic part
    let mut ej = [0f32; J];
    let mut g = Gather::new();
    // survivors entering the level (slots held in place may be holes)
    let mut live = if resume.is_some() { nsurv } else { cands.len() };
    for l in if resume.is_some() { NDL } else { 0 }..nl {
        if nsurv == 0 {
            break;
        }
        let lp = &gp.levels[l];
        // few survivors left: their full vectors cost less than this level's table
        if cq.bail > 0.0 && mode == Mode2::Thr && l > 0 && cq.luts[l].is_none() && (lp.units as f32) > cq.bail * live as f32 {
            break;
        }
        let tl1 = tick();
        if intm && l == NDL {
            // into the randomised levels: the deterministic sum as a float, within e plus its rounding to f32
            let id = cq.int.as_ref().unwrap();
            let (b, sg) = (id.b[NDL - 1], id.sigma as f64);
            // four slots at a time (the rows of a block of 32 are contiguous; the state is padded to whole blocks)
            let (vb, vs, v256) = (_mm256_set1_pd(b), _mm256_set1_pd(sg), _mm256_set1_pd(256.0));
            let mut k = 0;
            while k < nsurv {
                let h = _mm256_cvtepi32_pd(_mm_cvtepi16_epi32(_mm_loadl_epi64(sv.ist.as_ptr().add(ip(k, 0)) as *const _)));
                let lo = _mm256_cvtepi32_pd(_mm_cvtepi16_epi32(_mm_loadl_epi64(sv.ist.as_ptr().add(ip(k, 1)) as *const _)));
                let v = _mm256_add_pd(vb, _mm256_mul_pd(vs, _mm256_add_pd(_mm256_mul_pd(v256, h), lo)));
                _mm_storeu_ps(sv.adet.as_mut_ptr().add(k), _mm256_cvtpd_ps(v));
                k += 4;
            }
            edet = ((id.e[NDL - 1] + (b.abs() + sg * THETA_MAX as f64) * 2f64.powi(-23)) * (1.0 + 1e-6)) as f32;
        }
        let tt = tick();
        st.cyc[P_LVSETUP] += tt - tl1;
        let lut: &LevelLut = if cq.shared {
            gq.glut[l].get_or_init(|| {
                let mut lut = LevelLut::default();
                build_lut(&gq.wq, lp, &ix.etab, ix.split, None, &mut lut);
                lut
            })
        } else {
            if cq.luts[l].is_none() {
                st.cyc[C_TABLES] += 1;
                st.cyc[C_UNITS] += lp.units as u64;
                st.cyc[C_TSURV] += live as u64;
                st.cyc[C_TSMALL] += (live <= 8) as u64;
                let mut lut = sv.lut();
                let step = if intm && l < NDL { Some(cq.int.as_ref().unwrap().sigma) } else { None };
                if cq.gtab {
                    let gt = gq.gtab[l].get_or_init(|| group_tab(&gq.wq, lp, &ix.etab));
                    combine_lut(gt, &ix.ctab[ci][l], lp, cq.alr, step, &mut lut);
                } else {
                    build_lut(&cq.w, lp, &ix.etab, ix.split, step, &mut lut);
                }
                cq.luts[l] = Some(lut);
            }
            cq.luts[l].as_ref().unwrap()
        };
        st.cyc[P_TABLES] += tick() - tt;
        let tls = tick();
        let skip = l < first_decide && !verify && mode == Mode2::Thr;
        // a randomised level that only reads: its bounds are not computed (verify: computed and checked, not used)
        let nodec = l >= NDL && mode == Mode2::Thr && !rdec_has(cq.rdec, l - NDL);
        let skipb = skip || (nodec && !verify);
        // blocks whose kappa this level stores and a later rise replaces before the next level that decides: unused
        let mut kap_unused = 0u32;
        if nodec && !verify && l >= NDL {
            let lr = l - NDL;
            let nd = (lr + 1..nl - NDL).find(|&x| rdec_has(cq.rdec, x));
            for &j in &gp.thchg[lr] {
                let again = match nd {
                    None => true,
                    Some(d) => (lr + 1..=d).any(|x| gp.thchg[x].contains(&j)),
                };
                if again {
                    kap_unused |= 1 << j;
                }
            }
        }
        for pl in &lut.parts {
            match pl.j {
                None => edet += pl.err,
                Some(j) => ej[j] += pl.err,
            }
        }
        if intm && l < NDL {
            let id = cq.int.as_mut().unwrap();
            let (b0, e0) = if l > 0 { (id.b[l - 1], id.e[l - 1]) } else { (0.0, 0.0) };
            id.q[l] = (if l > 0 { id.q[l - 1] } else { 0.0 }) + lut.qsum;
            assert!(id.q[l] <= (256 * IHMAX) as f64, "integer tables overflow the accumulators");
            id.b[l] = b0 + lut.parts.iter().map(|p| p.off as f64).sum::<f64>();
            id.e[l] = e0 + lut.parts.iter().map(|p| p.err as f64).sum::<f64>() * (1.0 + 1e-6);
            if id.rt[l].is_none() {
                let (b, e, sg, d) = (id.b[l], id.e[l], id.sigma as f64, pr.dial[l] as f64);
                let dw = cq.dwa[l] as f64 * gp.pconst[l] as f64 * (1.0 + 1e-6);
                let mut rin = [None; 16];
                let mut rout = [None; 16];
                for n in 0..15 {
                    let x = dw * gp.ntab[l][n] as f64 + cq.ewdet as f64;
                    rin[n] = Some((x + e - b) / sg);
                    rout[n] = Some((b + e + d * x) / sg);
                }
                id.rt[l] = Some(rtab(&rin, &rout));
            }
        }
        // verify: the level's tables in f64 from the same weights, unit by unit in row order
        let mut tab64: Vec<[f64; 16]> = Vec::new();
        let w64: Vec<f64> = if verify && l < NDL {
            if cq.gtab {
                let wm = c.wm(gp);
                gq.wq.iter().zip(&wm).map(|(&q, &m)| q as f64 - cq.alr as f64 * m as f64).collect()
            } else {
                cq.w.iter().map(|&x| x as f64).collect()
            }
        } else {
            Vec::new()
        };
        if verify && l < NDL {
            for part in &lp.parts {
                for fs in &part.ufields {
                    let mut tv = [0f64; 16];
                    for &fi in fs.iter().take_while(|&&f| f != u16::MAX) {
                        let f = &part.fields[fi as usize];
                        let w = w64[f.widx as usize];
                        for e in 0..16 {
                            tv[e] += w * ix.etab.e[f.eid as usize][e] as f64;
                        }
                    }
                    tab64.push(tv);
                }
            }
        }
        let tl0 = _rdtsc();
        st.cyc[P_LVSETUP] += tick() - tls;
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
        if aligned {
            let sb = a0 as usize / BLK;
            let pc = cbase.add(sb * units * 16);
            for x in (0..(units * 16 * PF.min(nb))).step_by(64) {
                _mm_prefetch::<_MM_HINT_T0>(pc.add(x) as *const i8);
            }
        }
        // what a later level reads of the survivors' state: nothing after the last level but their slots; the
        // accumulators of blocks coded so far; kappa and tan of a coded block that does not rise at the next level
        // (a rise recomputes them before any use); |y| from the first randomised level on
        let last = l + 1 == nl;
        let mut need = RandNeed::default();
        if l >= NDL && !last {
            need.yh = true;
            need.c0 = true;
            for j in 0..J {
                if gp.rates[l - NDL][j] > 0.0 {
                    need.aj |= 1 << j;
                    if !gp.thchg[l + 1 - NDL].contains(&j) {
                        need.kap |= 1 << j;
                    }
                }
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
                let sb = (a0 as usize + k0) / BLK;
                g.ident = Some(sb);
                // the level's codes of a cluster span a few pages, which the hardware prefetcher ramps up on late
                if b + PF < nb {
                    let pc = cbase.add((sb + PF) * units * 16);
                    for x in (0..units * 16).step_by(64) {
                        _mm_prefetch::<_MM_HINT_T0>(pc.add(x) as *const i8);
                    }
                    _mm_prefetch::<_MM_HINT_T0>(sbase.add((sb + PF) * srows * 16) as *const i8);
                }
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
            if verify && l < NDL {
                for i in 0..32 {
                    if act >> i & 1 == 1 {
                        sv.a64[k0 + i] += tab64.iter().enumerate().map(|(u, tv)| tv[nib(cp, u, i)]).sum::<f64>();
                    }
                }
            }
            if intm && l < NDL {
                let rt = cq.int.as_ref().unwrap().rt[l].as_ref().unwrap();
                let (inm, outm) = int_level(cp, lut, sp, rt, sv.ist.as_mut_ptr().add(ip(k0, 0)), !skip);
                let t2 = tick();
                st.cyc[P_DECODE] += t2 - t1;
                let (inm, outm) = (inm & act, outm & act);
                if verify && !skip {
                    check_det(ix, ci, pr, cq, l, t, sv, k0, sp, act, inm, outm, st);
                }
                let k = if skip || sweep { act } else { act & !(inm | outm) };
                if !skip {
                    st.dec_in[l] += inm.count_ones() as u64;
                    st.dec_out[l] += outm.count_ones() as u64;
                }
                if !skip && !sweep && inm != 0 {
                    for h in 0..2 {
                        let m = (inm >> (16 * h)) as u16;
                        if m != 0 {
                            push_rows(out.rows_in, sv.idx.as_ptr().add(k0 + 16 * h), m, c.r0 as u32);
                        }
                    }
                }
                keep = [k as u16, (k >> 16) as u16];
                st.cyc[P_BOUNDS] += tick() - t2;
            } else {
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
            // the randomised levels' scalars, both halves of the block from each row: |y| at the first, kappa and tan
            // at a block's rise
            if l >= NDL {
                let tk = tick();
                let hms = [act as u16, (act >> 16) as u16];
                    if lr == 0 {
                        for j in 0..J {
                            let (a0, a1) = row_nibbles(sp.add(2 * j * 16));
                            let (b0, b1) = row_nibbles(sp.add((2 * j + 1) * 16));
                            for h in 0..2 {
                            let hm = hms[h];
                            if hm == 0 {
                                continue;
                            }
                            let o = k0 + 16 * h;
                            let s = ld(&sv.s, o);
                            let (lo, hi) = if h == 0 { (a0, b0) } else { (a1, b1) };
                            let pr_ = _mm512_mul_ps(_mm512_set1_ps(gp.ypredw[j]), s);
                            let tv = _mm512_mul_ps(_mm512_permutexvar_ps(hi, _mm512_loadu_ps(gp.yhl[j][1].as_ptr())), _mm512_permutexvar_ps(lo, _mm512_loadu_ps(gp.yhl[j][0].as_ptr())));
                            let mut y = _mm512_mul_ps(pr_, tv);
                            let em = _mm512_cmpeq_epi32_mask(_mm512_and_si512(lo, hi), _mm512_set1_epi32(15)) & hm;
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
                    }
                    for (slot, &j) in gp.thchg[lr].iter().enumerate() {
                        if kap_unused >> j & 1 == 1 {
                            continue;
                        }
                        let row = gp.rrow[lr][slot];
                        let kb = gp.kbits[lr][j] as usize;
                        let (a0, a1) = row_nibbles(sp.add(row * 16));
                        let (t0, t1) = row_nibbles(sp.add((row + kb / 4) * 16));
                        let (mut b0, mut b1) = (_mm512_setzero_si512(), _mm512_setzero_si512());
                        if kb == 8 {
                            (b0, b1) = row_nibbles(sp.add((row + 1) * 16));
                        }
                        for h in 0..2 {
                        let hm = hms[h];
                        if hm == 0 {
                            continue;
                        }
                        let o = k0 + 16 * h;
                        // a 4-bit code reads the table's first 16 entries; an 8-bit one is the product of its nibbles'
                        // entries (an escape's value is replaced below)
                        let kl = if h == 0 { a0 } else { a1 };
                        let (kv, kx) = if kb == 8 {
                            let kh = if h == 0 { b0 } else { b1 };
                            let v = _mm512_mul_ps(_mm512_permutexvar_ps(kh, _mm512_loadu_ps(gp.khl[lr][j][1].as_ptr())), _mm512_permutexvar_ps(kl, _mm512_loadu_ps(gp.khl[lr][j][0].as_ptr())));
                            (v, _mm512_and_si512(kl, kh))
                        } else {
                            (_mm512_permutexvar_ps(kl, _mm512_loadu_ps(gp.ktab[lr][j].as_ptr())), kl)
                        };
                        let tc = if h == 0 { t0 } else { t1 };
                        let yv = ld(&sv.yh[j], o);
                        let mut kap = _mm512_mul_ps(yv, kv);
                        let mut th = _mm512_permutexvar_ps(tc, _mm512_loadu_ps(gp.ttab[lr][j].as_ptr()));
                        let ek = _mm512_cmpeq_epi32_mask(kx, _mm512_set1_epi32(15)) & hm;
                        let et = _mm512_cmpeq_epi32_mask(tc, _mm512_set1_epi32(15)) & hm;
                        if ek | et != 0 {
                            let (mut rk, mut rt) = (arr(kap), arr(th));
                            for i in 0..16 {
                                if (ek | et) >> i & 1 == 0 {
                                    continue;
                                }
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
                        let ab = _mm512_mul_ps(_mm512_mul_ps(_mm512_mul_ps(yv, th), _mm512_set1_ps(1.0 + 1e-6)), _mm512_set1_ps(cq.nd[j]));
                        _mm512_storeu_ps(sv.var[j].as_mut_ptr().add(o), _mm512_mul_ps(_mm512_mul_ps(ab, ab), _mm512_set1_ps(1.0 / (gp.nb[j] - 1) as f32)));
                        }
                    }
                st.cyc[P_RKAP] += tick() - tk;
            }
            for h in 0..2 {
                let hm: u16 = (act >> (16 * h)) as u16;
                if hm == 0 {
                    continue;
                }
                let o = k0 + 16 * h;
                let (lb, ub, est, hn, sd);
                if l < NDL {
                    let s = ld(&sv.s, o);
                    let alrmr = _mm512_fmadd_ps(_mm512_set1_ps(cq.alr), ld(&sv.mr, o), _mm512_set1_ps(cq.qm));
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
                    ub = _mm512_add_ps(est, _mm512_mul_ps(hh, _mm512_set1_ps(pr.dial[l])));
                    hn = hh;
                    sd = _mm512_setzero_ps();
                } else {
                    for pl in &lut.parts {
                        let j = pl.j.unwrap();
                        let a = _mm512_add_ps(ld(&sv.aj[j], o), pv[1 + j][h]);
                        _mm512_storeu_ps(sv.aj[j].as_mut_ptr().add(o), a);
                    }
                    // the constant parts, once at the first randomised level (the same f32 operations as before)
                    let (c0, h0) = if lr == 0 {
                        let s = ld(&sv.s, o);
                        let alrmr = _mm512_fmadd_ps(_mm512_set1_ps(cq.alr), ld(&sv.mr, o), _mm512_set1_ps(cq.qm));
                        let sa = _mm512_mul_ps(s, ld(&sv.adet, o));
                        let c0 = _mm512_add_ps(alrmr, sa);
                        let fl = _mm512_add_ps(_mm512_abs_ps(alrmr), _mm512_abs_ps(sa));
                        let h0 = _mm512_fmadd_ps(u16x, fl, _mm512_fmadd_ps(s, _mm512_set1_ps(edet + cq.ewdet), _mm512_set1_ps(cq.fixed)));
                        _mm512_storeu_ps(sv.c0.as_mut_ptr().add(o), c0);
                        _mm512_storeu_ps(sv.h0.as_mut_ptr().add(o), h0);
                        (c0, h0)
                    } else {
                        (ld(&sv.c0, o), ld(&sv.h0, o))
                    };
                    if skipb {
                        keep[h] = hm;
                        continue;
                    }
                    let mut e = c0;
                    let mut hpar = h0;
                    let mut var = _mm512_setzero_ps();
                    let rates = &gp.rates[lr];
                    for j in 0..J {
                        if rates[j] == 0.0 {
                            hpar = _mm512_fmadd_ps(ld(&sv.yh[j], o), _mm512_set1_ps(cq.nd[j]), hpar);
                            continue;
                        }
                        let kap = ld(&sv.kap[j], o);
                        let eb = _mm512_mul_ps(kap, ld(&sv.aj[j], o));
                        e = _mm512_add_ps(e, eb);
                        // the kappa rounding bound and the estimate's 16 u share of |eb| in one product
                        hpar = _mm512_fmadd_ps(_mm512_set1_ps(dmxj[j] + (16.0 * U) as f32), _mm512_abs_ps(eb), hpar);
                        hpar = _mm512_fmadd_ps(kap, _mm512_set1_ps(ej[j] + cq.ewj[j]), hpar);
                        var = _mm512_add_ps(var, ld(&sv.var[j], o));
                    }
                    let sdv = _mm512_sqrt_ps(var);
                    let hnv = hpar;
                    let hh = _mm512_mul_ps(_mm512_fmadd_ps(_mm512_set1_ps(gp.csgl[lr]), sdv, hnv), one6);
                    est = e;
                    lb = _mm512_sub_ps(e, hh);
                    ub = _mm512_add_ps(e, _mm512_mul_ps(hh, _mm512_set1_ps(pr.dial[l])));
                    hn = hnv;
                    sd = sdv;
                }
                lbh[h] = lb;
                ubh[h] = ub;
                if verify {
                    let (lbs, ubs, es, hs, sds) = (arr(lb), arr(ub), arr(est), arr(hn), arr(sd));
                    let collect = ERRS.load(std::sync::atomic::Ordering::Relaxed);
                    for i in 0..16 {
                        if hm >> i & 1 == 0 {
                            continue;
                        }
                        let sc = sv.truth[o + i];
                        if collect && sc >= t {
                            // the share of the half-width above the estimate an item in the answer needed
                            st.errs.push((l as u8, ((sc - es[i] as f64) / ((es[i] - lbs[i]) as f64).max(1e-30)) as f32));
                        }
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
                if verify && l >= NDL {
                    let mut dg = DUMP.lock().unwrap();
                    if let Some(f) = dg.as_mut() {
                        use std::io::Write;
                        let es = arr(est);
                        let (hs, sds) = (arr(hn), arr(sd));
                        let tag = DUMP_TAG.load(std::sync::atomic::Ordering::Relaxed);
                        for i in 0..16 {
                            if hm >> i & 1 == 0 {
                                continue;
                            }
                            let k = o + i;
                            let mut r = [0f32; 24];
                            r[0] = f32::from_bits(tag);
                            r[1] = f32::from_bits((c.r0 + sv.idx[k] as usize) as u32);
                            r[2] = lr as f32;
                            r[3] = t as f32;
                            r[4] = (sv.truth[k] - t) as f32;
                            r[5] = (es[i] as f64 - t) as f32;
                            r[6] = hs[i];
                            r[7] = sds[i];
                            for j in 0..J {
                                let nd = cq.nd[j];
                                r[8 + 4 * j] = if gp.rates[lr][j] > 0.0 { 1.0 } else { 0.0 };
                                r[9 + 4 * j] = if gp.rates[lr][j] > 0.0 { (sv.var[j][k] * (gp.nb[j] - 1) as f32).sqrt() } else { 0.0 };
                                r[10 + 4 * j] = sv.yh[j][k] * nd;
                                r[11 + 4 * j] = if gp.rates[lr][j] > 0.0 { sv.kap[j][k] * sv.aj[j][k] } else { 0.0 };
                            }
                            let b: Vec<u8> = r.iter().flat_map(|x| x.to_le_bytes()).collect();
                            f.write_all(&b).unwrap();
                        }
                    }
                }
                if interval && l == stop {
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
                if nodec {
                    keep[h] = hm;
                    continue;
                }
                let inm = _mm512_cmp_ps_mask::<_CMP_GE_OQ>(lb, _mm512_set1_ps(tin)) & hm;
                let outm = _mm512_cmp_ps_mask::<_CMP_LT_OQ>(ub, _mm512_set1_ps(tout)) & hm;
                if verify && l < NDL {
                    let sh = 16 * h;
                    check_det(ix, ci, pr, cq, l, t, sv, k0, sp, (hm as u32) << sh, (inm as u32) << sh, (outm as u32) << sh, st);
                }
                if sweep {
                    st.dec_in[l] += inm.count_ones() as u64;
                    st.dec_out[l] += outm.count_ones() as u64;
                    keep[h] = hm;
                    continue;
                }
                if inm != 0 {
                    st.dec_in[l] += inm.count_ones() as u64;
                    push_rows(out.rows_in, sv.idx.as_ptr().add(o), inm, c.r0 as u32);
                    if let Some(iv) = out.iv.as_deref_mut() {
                        let (lbs, ubs) = (arr(lb), arr(ub));
                        for i in 0..16 {
                            if inm >> i & 1 == 1 {
                                iv.push(((c.r0 + sv.idx[o + i] as usize) as u32, lbs[i], ubs[i], l as u8));
                            }
                        }
                    }
                }
                st.dec_out[l] += outm.count_ones() as u64;
                keep[h] = hm & !(inm | outm);
            }
            let _ = (lbh, ubh);
            st.cyc[P_BOUNDS] += tick() - t2;
            }
            if interval && l == stop {
                continue;
            }
            let tc = tick();
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
                st.cyc[P_COMPACT] += tick() - tc;
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
                if l < NDL {
                    cs(&mut sv.s, w, m, o);
                    cs(&mut sv.mr, w, m, o);
                    compact_det(sv, intm, verify, w, m, o);
                }
                compact_rand(sv, w, m, o, need);
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
            st.cyc[P_COMPACT] += tick() - tc;
        }
        if l == stop {
            st.lcyc[l] += _rdtsc() - tl0;
            st.cyc[P_RUNCL] += tick() - ti;
            return;
        }
        let tc = tick();
        live = if aligned { kept_tot } else { wpos };
        if aligned {
            if kept_tot == 0 {
                nsurv = 0;
            } else if (kept_tot as f32) < nsurv as f32 * occ((l + 1).min(nl - 1)) {
                // occupancy fell: compact once, then re-pack from here on
                let mut w = 0;
                let mut o = 0;
                while o < nsurv {
                    let iv = _mm512_loadu_si512(sv.idx.as_ptr().add(o) as *const _);
                    let m = !_mm512_cmpeq_epi32_mask(iv, _mm512_set1_epi32(-1));
                    if m != 0 {
                        _mm512_storeu_si512(sv.idx.as_mut_ptr().add(w) as *mut _, _mm512_maskz_compress_epi32(m, iv));
                        if l < NDL {
                            cs(&mut sv.s, w, m, o);
                            cs(&mut sv.mr, w, m, o);
                            compact_det(sv, intm, verify, w, m, o);
                        }
                        compact_rand(sv, w, m, o, need);
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
        st.cyc[P_COMPACT] += tick() - tc;
        st.lcyc[l] += _rdtsc() - tl0;
        // single-pass rule: decide once at first_decide, the rest go to their full vectors
        if mode == Mode2::Thr && first_decide > 0 && l == first_decide {
            break;
        }
        let _ = nr;
    }
    let mut np = 0;
    for k in 0..nsurv {
        if sv.idx[k] != u32::MAX {
            out.pend.push((c.r0 + sv.idx[k] as usize) as u32);
            np += 1;
        }
    }
    st.enter[nl] += np;
    st.cyc[P_RUNCL] += tick() - ti;
}

/// The occupancy of slots held in place (with holes) below which they are compacted before each level, from timings on
/// this index's largest cluster: a level of one block in place, against one assembled from two source blocks at half
/// occupancy plus the compaction of the survivors' state. In place, every live item costs the block's decode over the
/// occupancy f; gathered, the gather and decode of a full block plus its compaction. Compacting pays when f is below
/// their ratio. The decisions' cost per block is the same either way and is left out.
pub fn calibrate_repack(ix: &Index2) -> Vec<f32> {
    let ci = (0..ix.k()).max_by_key(|&c| ix.cl[c].nblk).unwrap();
    let c = &ix.cl[ci];
    let gp = &ix.groups[c.g];
    let nb = c.nblk & !1;
    let mut out = Vec::new();
    let time = |f: &mut dyn FnMut()| -> f64 {
        f();
        let t = Timer::new();
        let mut reps = 0;
        while t.ms() < 2.0 {
            f();
            reps += 1;
        }
        t.ms() * 1e6 / reps as f64
    };
    for l in 0..ix.meta.nl() {
        let lp = &gp.levels[l];
        let mut lut = LevelLut::default();
        lut.hi = vec![1; lp.units * 16];
        lut.lo = vec![1; lp.units * 16];
        let mut g0 = 0;
        for p in &lp.parts {
            lut.parts.push(PartLut { j: p.j, g0, groups: p.units() / 4, off: 0.0, scale: 1.0, err: 0.0 });
            g0 += p.units() / 4;
        }
        let (units, srows) = (lp.units, lp.srows);
        let cbase = unsafe { ix.lcode[l].as_ptr().add(ix.coff[l][ci]) };
        let sbase = unsafe { ix.lscal[l].as_ptr().add(ix.soff[l][ci]) };
        let decode = |cp: *const u8| unsafe {
            let mut acc = _mm512_setzero_ps();
            for pl in &lut.parts {
                let (a, b) = part_sums(cp.add(pl.g0 * 64), lut.hi.as_ptr().add(pl.g0 * 64), pl.groups);
                let (x, y) = part_sums(cp.add(pl.g0 * 64), lut.lo.as_ptr().add(pl.g0 * 64), pl.groups);
                acc = _mm512_add_ps(acc, _mm512_add_ps(_mm512_add_ps(a, b), _mm512_add_ps(x, y)));
            }
            std::hint::black_box(arr(acc));
        };
        let t_ip = time(&mut || {
            for b in 0..nb {
                decode(unsafe { cbase.add(b * units * 16) });
            }
        }) / nb as f64;
        let (mut tmp, mut stmp) = (vec![0u8; units * 16 + 64], vec![0u8; srows * 16 + 64]);
        let mut g = Gather::new();
        let t_pk = time(&mut || unsafe {
            for d in 0..nb / 2 {
                let lanes: [u32; 32] = core::array::from_fn(|i| (64 * d + 2 * i) as u32);
                plan_gather(&lanes, &mut g);
                gather_rows(&g, cbase, units, tmp.as_mut_ptr());
                gather_rows(&g, sbase, srows, stmp.as_mut_ptr());
                decode(tmp.as_ptr());
            }
        }) / (nb / 2) as f64;
        // the survivors' state: 6 integer rows on the integer deterministic levels, 3 + 4 J float arrays after
        let narr = if l < NDL { 4 } else { 3 + 4 * J };
        let mut st = vec![vec![0f32; 64 * nb + 32]; narr];
        let t_c = time(&mut || unsafe {
            let mut w = 0;
            for o in (0..32 * nb).step_by(16) {
                for v in st.iter_mut() {
                    cs(v, w, 0x5555, o);
                }
                w += 8;
            }
        }) / (16 * nb) as f64;
        out.push((t_ip / (t_pk + 32.0 * t_c)).clamp(0.05, 0.95) as f32);
    }
    out
}

// ---------------- the threshold query with the planner ----------------

#[derive(Clone, Copy, Debug)]
pub enum PlanMode {
    Auto,
    Index,
    Flat,
}

/// Cost model of the planner, in microseconds on one thread. d is the fraction of the visible items of opened
/// clusters that the cut and the sorted range leave undecided, a proxy for how deep candidates will go, known before
/// any code is read. `Plan::default` holds constants for 1M BioCLIP (n = 768, AVX-512); `Plan::calibrate` times them
/// on the index at hand.
#[derive(Clone, Copy, Debug)]
pub struct Plan {
    pub mode: PlanMode,
    pub full_us: f64,      // exact decision from a full vector, per candidate
    pub item_us0: f64,     // index path per candidate: item_us0 + item_us1 * d
    pub item_us1: f64,
    pub cl_us0: f64,       // index path per cluster (setup and tables): cl_us0 + cl_us1 * d
    pub cl_us1: f64,
    pub grp_us: f64,       // index path per group (exact basis product, rotated weights)
    pub scan_us: f64,      // scan of the visible set, per visible item
    pub cut_us: f64,       // cluster cut and sorted range, per cluster holding a visible item
    pub seed_us: f64,      // the k-NN seed (query setup, probing, first full vectors), per query
    pub alpha0: bool,      // tables shared per group (no centre multiple taken out of the query)
    pub jump_level: usize, // when most items stay undecided (d > jump_d): no decisions before this level
    pub jump_d: f64,
    pub int_det: bool,     // threshold queries decide the deterministic levels on the integer path
    pub gtab: bool,        // tables per (group, level), combined per cluster with the centre's stored tables
    pub repack: bool,      // per-level occupancy thresholds for compacting survivors held in place (Index2::repack)
    pub repack_mul: f32,   // their factor: the decisions' cost per block, which the timing leaves out, raises them
    pub bail: f32,         // threshold queries: survivors go to full vectors before a level whose table has more than bail units per survivor (0: off)
    pub rdec: u32,         // threshold queries decide at randomised level lr only when bit lr is set
    pub tol: f64,          // the tolerance d: an item whose lower bound reaches t - d is taken in (0: exact)
    pub scan_lo: f64,      // a threshold query whose answer share of the visible set, estimated from a sample, is in
    pub scan_hi: f64,      // [scan_lo, scan_hi] scans the visible set (scan_lo > 1: never)
    pub scan_cal: bool,    // Plan::calibrate also times the scan band (about 9 s at 1M; off: the band is left as given)
    pub pscan: bool,       // a scan of the visible set copies their vectors into a buffer, a block at a time, and scans that
    pub col: bool,         // threshold queries decide the deterministic part on the columnar store (fixed order)
    pub colq: bool,        // in an order chosen per (query, cluster)
    pub collazy: bool,     // with the single stored norm first, block norms only for the items it leaves undecided
}
impl Default for Plan {
    fn default() -> Self {
        Plan { mode: PlanMode::Auto, full_us: 0.20, item_us0: 0.02, item_us1: 0.08, cl_us0: 4.0, cl_us1: 20.0, grp_us: 140.0, scan_us: 0.20, cut_us: 2.5, seed_us: 300.0, alpha0: false, jump_level: 0, jump_d: 0.8, int_det: false, gtab: false, repack: false, repack_mul: 1.0, bail: 0.0, rdec: u32::MAX, tol: 0.0, scan_lo: 2.0, scan_hi: 2.0, scan_cal: false, pscan: false, col: false, colq: false, collazy: false }
    }
}
impl Plan {
    /// The timed constants as `name=value` lines.
    pub fn to_kv(&self) -> String {
        format!("full_us={}\nitem_us0={}\nitem_us1={}\ncl_us0={}\ncl_us1={}\ngrp_us={}\nscan_us={}\ncut_us={}\nseed_us={}\nscan_lo={}\nscan_hi={}\n",
                self.full_us, self.item_us0, self.item_us1, self.cl_us0, self.cl_us1, self.grp_us, self.scan_us, self.cut_us, self.seed_us, self.scan_lo, self.scan_hi)
    }
    /// `base` with the constants of `name=value` lines (as `to_kv` writes them).
    pub fn from_kv(base: Plan, text: &str) -> Plan {
        let mut p = base;
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let (k, v) = line.split_once('=').unwrap_or_else(|| panic!("plan line {line:?} is not name=value"));
            let v: f64 = v.trim().parse().unwrap_or_else(|_| panic!("plan value {v:?} is not a number"));
            *match k.trim() {
                "full_us" => &mut p.full_us,
                "item_us0" => &mut p.item_us0,
                "item_us1" => &mut p.item_us1,
                "cl_us0" => &mut p.cl_us0,
                "cl_us1" => &mut p.cl_us1,
                "grp_us" => &mut p.grp_us,
                "scan_us" => &mut p.scan_us,
                "cut_us" => &mut p.cut_us,
                "seed_us" => &mut p.seed_us,
                "scan_lo" => &mut p.scan_lo,
                "scan_hi" => &mut p.scan_hi,
                k => panic!("unknown plan constant {k:?}; the names are those of Plan::to_kv"),
            } = v;
        }
        p
    }

    /// Modelled index-path cost of one cluster with `ncand` candidates (group setup not included).
    pub fn index_us(&self, ncand: usize, d: f64) -> f64 {
        ncand as f64 * (self.item_us0 + self.item_us1 * d) + self.cl_us0 + self.cl_us1 * d
    }

    /// The constants re-timed on this index and machine, on one thread, from corpus pseudo-queries (`nq` stored rows
    /// spread over the index) at their 10th best score, unfiltered and over a fixed random eighth of the rows: the
    /// visible scan, the cut, full vectors after the cut, the k-NN seed, and one factor scaling every index-path
    /// constant (least squares of measured against modelled time). Data only: no example queries.
    /// `cpu`: the CPU to pin the timing thread to.
    pub fn calibrate(ix: &Index2, base: Plan, nq: usize, cpu: Option<usize>) -> Plan {
        assert!(nq >= 1, "calibration needs at least one pseudo-query; pass --calib 1 or more, or --plan-file");
        let pool = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap();
        pool.install(|| {
            if let Some(c) = cpu {
                crate::util::pin(c);
            }
            let ents: Vec<u32> = (0..ix.nrows).filter(|&r| (r as u64).wrapping_mul(0x9E3779B97F4A7C15) >> 60 < 2).map(|r| ix.row_ent[r]).collect();
            let mut ents = ents;
            ents.sort_unstable();
            let tenth = Vis2::new(ix, &RoaringBitmap::from_sorted_iter(ents).unwrap());
            let (mut scan_t, mut scan_n, mut cut_t, mut cut_n, mut full_t, mut full_n, mut seed_t) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
            let (mut tm_idx, mut mm_idx) = (0.0, 0.0);
            for i in 0..nq {
                let row = (2 * i + 1) * ix.nrows / (2 * nq);
                let q: Vec<f32> = ix.full_row_f16(row).iter().map(|&h| f16_to_f32(h)).collect();
                let qn = q.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt();
                let q: Vec<f32> = q.iter().map(|&x| (x as f64 / qn) as f32).collect();
                let pr = prep2(ix, &q);
                let tm = Timer::new();
                let mut st = Stats::new(ix.meta.nl());
                std::hint::black_box(seed2(ix, &prep2(ix, &q), 10, None, &mut st));
                seed_t += tm.ms() * 1e3;
                for vis in [None, Some(&tenth)] {
                    let nvis = vis.map_or(ix.nrows, |v: &Vis2| v.cl_cnt.iter().map(|&x| x as usize).sum());
                    let tm = Timer::new();
                    let top = scan_knn(ix, &pr.q16, &pr.q32, pr.qn, 10, vis);
                    scan_t += tm.ms() * 1e3;
                    scan_n += nvis as f64;
                    let t = top.last().map_or(f64::NEG_INFINITY, |x| x.0);
                    for mode in [PlanMode::Flat, PlanMode::Index] {
                        let o = Opts2 { vis, verify: false, want_iv: true, plan: Plan { mode, ..base } };
                        let tm = Timer::new();
                        let Planned::Run { pre, use_idx, first_decide, cut_cyc } = plan2(ix, &pr, t, &o, 0.0) else { unreachable!() };
                        let tc = tm.ms() * 1e3;
                        let tm = Timer::new();
                        std::hint::black_box(run_planned(ix, &pr, t, &o, &pre, &use_idx, first_decide, cut_cyc).rows.len());
                        let tr = tm.ms() * 1e3;
                        let (mut ncand, mut nv) = (0usize, 0usize);
                        let mut groups = vec![false; ix.groups.len()];
                        for (ci, p) in pre.iter().enumerate() {
                            if let Pre::Open { ncand: c, .. } = p {
                                ncand += c;
                                nv += vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize);
                                if *c > 0 {
                                    groups[ix.cl[ci].g] = true;
                                }
                            }
                        }
                        let d = ncand as f64 / nv.max(1) as f64;
                        match mode {
                            PlanMode::Flat => {
                                cut_t += tc;
                                cut_n += vis.map_or(ix.k(), |v| v.cl_cnt.iter().filter(|&&x| x > 0).count()) as f64;
                                full_t += tr;
                                full_n += ncand as f64;
                            }
                            _ => {
                                let mut m = groups.iter().filter(|&&g| g).count() as f64 * base.grp_us;
                                for p in &pre {
                                    if let Pre::Open { ncand: c, .. } = p {
                                        if *c > 0 {
                                            m += base.index_us(*c, d);
                                        }
                                    }
                                }
                                tm_idx += tr * m;
                                mm_idx += m * m;
                            }
                        }
                    }
                }
            }
            let s = tm_idx / mm_idx.max(1e-30);
            let (scan_lo, scan_hi) = if base.scan_cal { Plan::scan_band(ix, base, nq.min(3)) } else { (base.scan_lo, base.scan_hi) };
            Plan {
                scan_lo,
                scan_hi,
                scan_us: scan_t / scan_n,
                cut_us: cut_t / cut_n,
                full_us: full_t / full_n.max(1.0),
                seed_us: seed_t / nq as f64,
                item_us0: base.item_us0 * s,
                item_us1: base.item_us1 * s,
                cl_us0: base.cl_us0 * s,
                cl_us1: base.cl_us1 * s,
                grp_us: base.grp_us * s,
                ..base
            }
        })
    }

    /// The answer shares at which a scan of every row beats the index path, timed on `nq` stored rows as queries at
    /// thresholds of 5 to 99% answers: (lo, hi) spans the shares where the index was slower, extended halfway to the
    /// neighbouring shares; (2, 2) when it never was.
    pub fn scan_band(ix: &Index2, base: Plan, nq: usize) -> (f64, f64) {
        const F: [f64; 10] = [0.05, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.75, 0.9, 0.99];
        let plan = Plan { scan_lo: 2.0, scan_hi: 2.0, ..base };
        let o = Opts2 { vis: None, verify: false, want_iv: false, plan };
        let mut slower = [0usize; F.len()];
        for i in 0..nq.max(1) {
            let row = (2 * i + 1) * ix.nrows / (2 * nq.max(1)) + 1;
            let q: Vec<f32> = ix.full_row_f16(row).iter().map(|&h| f16_to_f32(h)).collect();
            let qn = q.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt();
            let q: Vec<f32> = q.iter().map(|&x| (x as f64 / qn) as f32).collect();
            let pr = prep2(ix, &q);
            let mut sc: Vec<f32> = (0..ix.nrows).map(|r| unsafe { crate::kernel::dot_f16(&pr.q32, ix.full_row_f16(r)) }).collect();
            sc.sort_unstable_by(|a, b| b.partial_cmp(a).unwrap());
            for (k, &f) in F.iter().enumerate() {
                let t = sc[((ix.nrows as f64 * f) as usize).min(ix.nrows - 1)] as f64;
                let (mut ti, mut ts) = (f64::MAX, f64::MAX);
                for _ in 0..2 {
                    let tm = Timer::new();
                    std::hint::black_box(threshold2(ix, &pr, t, &o).rows.len());
                    ti = ti.min(tm.ms());
                    let tm = Timer::new();
                    std::hint::black_box(scan_visible(ix, &pr, t, None, false, false).rows.len());
                    ts = ts.min(tm.ms());
                }
                if ti > ts {
                    slower[k] += 1;
                }
            }
        }
        let sl: Vec<usize> = (0..F.len()).filter(|&k| 2 * slower[k] > nq.max(1)).collect();
        match (sl.first(), sl.last()) {
            (Some(&a), Some(&b)) => (if a == 0 { 0.0 } else { (F[a - 1] + F[a]) / 2.0 }, if b + 1 == F.len() { 1.0 } else { (F[b] + F[b + 1]) / 2.0 }),
            _ => (2.0, 2.0),
        }
    }

    /// Before anything is read: a scan of the visible set costs no more than cutting every cluster that holds a
    /// visible item, plus `fixed_us` of other index-path work that the scan avoids.
    pub fn scan_first(&self, vis: &Vis2, fixed_us: f64) -> bool {
        let nvis = vis.rows.len();
        let nvc = vis.cl_cnt.iter().filter(|&&x| x > 0).count();
        matches!(self.mode, PlanMode::Auto) && nvis as f64 * self.scan_us <= nvc as f64 * self.cut_us + fixed_us
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
    pub npow: u32,     // the index's guarantee N, as 2^npow (Meta2::npow)
}

const SCAN_ROWS: usize = 1024;

/// Number of scan chunks over the visible rows (all rows when vis is None), SCAN_ROWS each.
fn scan_chunks(ix: &Index2, vis: Option<&Vis2>) -> usize {
    (vis.map_or(ix.nrows, |v| v.rows.len()) + SCAN_ROWS - 1) / SCAN_ROWS
}
/// Appends the visible rows of scan chunk `ch`, ascending.
fn scan_rows(ix: &Index2, vis: Option<&Vis2>, ch: usize, out: &mut Vec<u32>) {
    let nr = vis.map_or(ix.nrows, |v| v.rows.len());
    let (a, b) = (ch * SCAN_ROWS, ((ch + 1) * SCAN_ROWS).min(nr));
    match vis {
        None => out.extend(a as u32..b as u32),
        Some(v) => out.extend_from_slice(&v.rows[a..b]),
    }
}

/// Certified scan of the visible set (every row when vis is None): every visible row's full vector, exact near the
/// threshold.
pub fn scan_visible(ix: &Index2, pr: &Prep2, t: f64, vis: Option<&Vis2>, want_exact: bool, packed: bool) -> ThrOut2 {
    let nl = ix.meta.nl();
    let parts: Vec<ThrOut2> = (0..scan_chunks(ix, vis)).into_par_iter().map(|ch| {
        let mut acc = ThrOut2 { st: Stats::new(nl), npow: ix.meta.npow, ..Default::default() };
        let mut rows = Vec::with_capacity(4096);
        scan_rows(ix, vis, ch, &mut rows);
        if packed && vis.is_some() {
            decide_full_packed(ix, pr, t, &rows, want_exact, &mut acc);
        } else {
            decide_full(ix, pr, t, &rows, want_exact, &mut acc);
        }
        acc
    }).collect();
    let mut out = ThrOut2 { st: Stats::new(nl), scanned: true, npow: ix.meta.npow, ..Default::default() };
    for p in parts {
        out.rows.extend(p.rows);
        out.st.add(&p.st);
        out.exact.extend(p.exact);
    }
    out.flat_items = out.st.full_reads;
    out
}

/// Exact k-NN by a scan of the visible set's stored vectors (every row when vis is None): (score, entity), best first.
pub fn scan_knn(ix: &Index2, q16: &[u16], q32: &[f32], qn: f64, kk: usize, vis: Option<&Vis2>) -> Vec<(f64, u32)> {
    crate::query::topk_f16(q16, q32, kk, full_err_qn(ix, qn), scan_chunks(ix, vis), |ch, out| scan_rows(ix, vis, ch, out),
                           |r| ix.full_row_f16(r as usize), |r| ix.row_ent[r as usize])
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

/// decide_full over scattered rows by blocks of 32: each block's vectors are copied into a contiguous buffer (the next
/// block's rows prefetched meanwhile) and scored there.
pub fn decide_full_packed(ix: &Index2, pr: &Prep2, t: f64, rows: &[u32], want_exact: bool, acc: &mut ThrOut2) {
    const B: usize = 32;
    let n = ix.n();
    let e = full_err2(ix, pr);
    let mut buf = vec![0u16; B * n];
    let pf = |r: u32| unsafe {
        let p = ix.full_row_f16(r as usize).as_ptr() as *const i8;
        for x in (0..2 * n).step_by(64) {
            _mm_prefetch::<_MM_HINT_T0>(p.add(x));
        }
    };
    for &r in rows.iter().take(B) {
        pf(r);
    }
    for (bi, blk) in rows.chunks(B).enumerate() {
        for &r in rows.iter().skip((bi + 1) * B).take(B) {
            pf(r);
        }
        for (i, &r) in blk.iter().enumerate() {
            buf[i * n..(i + 1) * n].copy_from_slice(ix.full_row_f16(r as usize));
        }
        let mut k = 0;
        while k < blk.len() {
            let m = (blk.len() - k).min(4);
            let rr: [&[u16]; 4] = core::array::from_fn(|q| &buf[(k + q.min(m - 1)) * n..(k + q.min(m - 1) + 1) * n]);
            let sc = unsafe { crate::kernel::dot_f16_x4(&pr.q32, rr) };
            for q in 0..m {
                acc.st.full_reads += 1;
                let s = sc[q] as f64;
                let row = blk[k + q];
                let inn = if s >= t + e {
                    true
                } else if s < t - e {
                    false
                } else {
                    acc.st.exact_fallbacks += 1;
                    exact_ge(exact_dot(&pr.q16, ix.full_row_f16(row as usize)), t)
                };
                if inn {
                    acc.rows.push(row);
                    if want_exact {
                        acc.exact.push((row, exact_value2(ix, pr, row as usize)));
                    }
                }
            }
            k += m;
        }
    }
}

#[derive(Clone, Copy)]
pub enum Pre {
    Skip,
    In,
    Open { qm: f64, u0: usize, u1: usize, d0: usize, d1: usize, ncand: usize },
}

/// What the planner chose for a threshold query: a scan of the visible set, or per cluster the index path or full
/// vectors after the cut.
pub enum Planned {
    Scan,
    Run { pre: Vec<Pre>, use_idx: Vec<bool>, first_decide: usize, cut_cyc: u64 },
}

fn cands_of(ix: &Index2, ci: usize, d0: usize, d1: usize, vis: Option<&Vis2>) -> Vec<u32> {
    let c = &ix.cl[ci];
    match vis {
        None => ((d0 - c.r0) as u32..(d1 - c.r0) as u32).collect(),
        Some(v) => (d0..d1).filter(|&r| v.has(r)).map(|r| (r - c.r0) as u32).collect(),
    }
}

/// Whether a decision mask (Plan::rdec) decides at randomised level lr; all ones decides at every level.
#[inline(always)]
fn rdec_has(rdec: u32, lr: usize) -> bool {
    rdec == u32::MAX || (lr < 32 && rdec >> lr & 1 == 1)
}

/// Rows sampled to estimate a threshold query's answer share.
pub const SHARE_SAMPLE: usize = 256;

/// The share of `ns` visible rows (every row when vis is None), evenly spaced, whose f32 score from the stored vector
/// is at least t: an estimate of the answer's share of the visible set (planning only).
pub fn answer_share(ix: &Index2, pr: &Prep2, t: f64, vis: Option<&Vis2>, ns: usize) -> f64 {
    if !matches!(ix.full, Full2::Real { .. }) {
        return 0.0;
    }
    let nvis = vis.map_or(ix.nrows, |v| v.rows.len());
    let ns = ns.min(nvis);
    if ns == 0 {
        return 0.0;
    }
    let mut hit = 0usize;
    for i in 0..ns {
        let k = (2 * i + 1) * nvis / (2 * ns);
        let row = vis.map_or(k, |v| v.rows[k] as usize);
        if unsafe { crate::kernel::dot_f16(&pr.q32, ix.full_row_f16(row)) } as f64 >= t {
            hit += 1;
        }
    }
    hit as f64 / ns as f64
}

/// The cluster cut and sorted range at t, then the planner's choice. `fixed_us` is index-path work already committed
/// to by the caller that a scan would have avoided (nothing for a threshold query).
pub fn plan2(ix: &Index2, pr: &Prep2, t: f64, o: &Opts2, fixed_us: f64) -> Planned {
    let k = ix.k();
    let rr = ix.x2.sqrt();
    let pl = o.plan;
    if let Some(v) = o.vis {
        if pl.scan_first(v, fixed_us) {
            return Planned::Scan;
        }
    }
    // pass 1: cut and range per cluster (the norm sort needs every group's basis product first)
    let tc0 = tick();
    let nusort = !ix.nukey.is_empty();
    if nusort {
        let mut gs: Vec<usize> = (0..k).filter(|&ci| o.vis.map_or(true, |v| v.cl_cnt[ci] > 0)).map(|ci| ix.cl[ci].g).collect();
        gs.sort_unstable();
        gs.dedup();
        setup_groups(ix, pr, &gs, &mut Stats::new(ix.meta.nl()));
    }
    let pre_of = |ci: usize| -> Pre {
        if let Some(v) = o.vis {
            if v.cl_cnt[ci] == 0 {
                return Pre::Skip;
            }
        }
        if nusort {
            let c = &ix.cl[ci];
            let qm = dot64(&pr.q64, ix.centres_f64(ci));
            let nc = NuCut::new(ix, ci, pr, pr.gq[c.g].get().unwrap(), qm);
            let (u0, u1, d0, d1) = nu_range(ix, ci, &nc, t);
            if u0 == c.r1 {
                return Pre::Skip;
            }
            if d0 == c.r1 && u0 == c.r0 {
                return Pre::In;
            }
            let ncand = match o.vis {
                None => d1 - d0,
                Some(v) => count_bits(&v.bits, d0, d1),
            };
            return Pre::Open { qm, u0, u1, d0, d1, ncand };
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
    };
    let nvis_cl = |ci: usize| o.vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize);
    let mut pre: Vec<Option<Pre>> = vec![None; k];
    // a sample of one cluster in SAMPLE first: when the scan already beats the extrapolated cost of the rest of the
    // cut and of every candidate's cheaper path, the cut stops there
    const SAMPLE: usize = 8;
    let live: Vec<usize> = (0..k).filter(|&ci| nvis_cl(ci) > 0).collect();
    // when most visible items are candidates (d), an answer share in the scan band (estimated from a sample) scans
    let in_band = |d: f64| {
        matches!(pl.mode, PlanMode::Auto) && pl.scan_lo <= 1.0 && d >= 0.25 && {
            let p = answer_share(ix, pr, t, o.vis, SHARE_SAMPLE);
            p >= pl.scan_lo && p <= pl.scan_hi
        }
    };
    if matches!(pl.mode, PlanMode::Auto) && live.len() >= 8 * SAMPLE {
        let (mut nv_s, mut nc_s, mut nvo_s) = (0usize, 0usize, 0usize);
        for &ci in live.iter().step_by(SAMPLE) {
            let p = pre_of(ci);
            nv_s += nvis_cl(ci);
            if let Pre::Open { ncand, .. } = p {
                nc_s += ncand;
                nvo_s += nvis_cl(ci);
            }
            pre[ci] = Some(p);
        }
        let d = nc_s as f64 / nvo_s.max(1) as f64;
        if in_band(d) {
            return Planned::Scan;
        }
        let cost_s: f64 = live.iter().step_by(SAMPLE).map(|&ci| match pre[ci] {
            Some(Pre::Open { ncand, .. }) if ncand > 0 => (ncand as f64 * pl.full_us).min(pl.index_us(ncand, d)),
            _ => 0.0,
        }).sum();
        let nvis = o.vis.map_or(ix.nrows, |v| v.rows.len());
        let rest = live.len() - (live.len() + SAMPLE - 1) / SAMPLE;
        if nvis as f64 * pl.scan_us < rest as f64 * pl.cut_us + cost_s * nvis as f64 / nv_s.max(1) as f64 {
            return Planned::Scan;
        }
    }
    let pre: Vec<Pre> = pre.into_par_iter().enumerate().map(|(ci, p)| p.unwrap_or_else(|| pre_of(ci))).collect();
    let cut_cyc = tick() - tc0;
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
    // too few clusters for the sample above: the same check on the whole cut
    if live.len() < 8 * SAMPLE && in_band(d) {
        return Planned::Scan;
    }
    let first_decide = if matches!(pl.mode, PlanMode::Auto) && pl.jump_level > 0 && d > pl.jump_d { pl.jump_level } else { 0 };
    let mut grp_gain = vec![0f64; ix.groups.len()];
    for (ci, p) in pre.iter().enumerate() {
        if let Pre::Open { ncand, .. } = p {
            if *ncand == 0 {
                continue;
            }
            let gain = *ncand as f64 * pl.full_us - pl.index_us(*ncand, d);
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
        let nvis = o.vis.map_or(ix.nrows, |v| v.cl_cnt.iter().map(|&x| x as usize).sum());
        let gains: f64 = (0..ix.groups.len()).filter(|&g| grp_gain[g] >= pl.grp_us).map(|g| grp_gain[g] - pl.grp_us).sum();
        if ncand_all as f64 * pl.full_us - gains > nvis as f64 * pl.scan_us {
            return Planned::Scan;
        }
    }
    Planned::Run { pre, use_idx, first_decide, cut_cyc }
}

pub fn threshold2(ix: &Index2, pr: &Prep2, t: f64, o: &Opts2) -> ThrOut2 {
    match plan2(ix, pr, t, o, 0.0) {
        Planned::Scan => scan_visible(ix, pr, t, o.vis, o.want_iv, o.plan.pscan),
        Planned::Run { pre, use_idx, first_decide, cut_cyc } => run_planned(ix, pr, t, o, &pre, &use_idx, first_decide, cut_cyc),
    }
}

/// A cluster's per-query setup for a threshold query under plan `pl`, on the integer deterministic levels when `int`.
pub fn open_cluster(ix: &Index2, ci: usize, gq: &GQ2, pr: &Prep2, qm: f64, pl: Plan, int: bool) -> CQ2 {
    let gt = pl.gtab && !pl.alpha0;
    let mut cq = cluster_q2w(ix, ci, gq, pr, qm, pl.alpha0, !gt);
    cq.gtab = gt;
    cq.repack = if pl.repack { pl.repack_mul } else { 0.0 };
    cq.bail = pl.bail;
    assert!(pl.rdec == u32::MAX || ix.meta.rlv.len() <= 32, "rdec can name 32 randomised levels and this index has {}; use rdec=all", ix.meta.rlv.len());
    cq.rdec = pl.rdec;
    cq.tol = pl.tol;
    if int {
        cq.enable_int(ix, ci, gq);
    }
    cq
}

/// The threshold query over the clusters as planned.
pub fn run_planned(ix: &Index2, pr: &Prep2, t: f64, o: &Opts2, pre: &[Pre], use_idx: &[bool], first_decide: usize, cut_cyc: u64) -> ThrOut2 {
    let nl = ix.meta.nl();
    let k = ix.k();
    let rr = ix.x2.sqrt();
    let pl = o.plan;
    let mut gst = Stats::new(nl);
    {
        let mut need: Vec<usize> = (0..k).filter(|&ci| use_idx[ci]).map(|ci| ix.cl[ci].g).collect();
        need.sort_unstable();
        need.dedup();
        setup_groups(ix, pr, &need, &mut gst);
    }
    let parts: Vec<ThrOut2> = (0..k).into_par_iter().fold(
        || (ThrOut2 { st: Stats::new(nl), npow: ix.meta.npow, ..Default::default() }, State::default()),
        |(mut acc, mut sv), ci| {
            let tcl = tick();
            let c = &ix.cl[ci];
            let push = |acc: &mut ThrOut2, r0: usize, r1: usize, iv: Option<(f64, f64)>| {
                if o.vis.is_none() && iv.is_none() {
                    acc.rows.extend(r0 as u32..r1 as u32);
                    return;
                }
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
                    if o.want_iv && !ix.nukey.is_empty() {
                        let qm = dot64(&pr.q64, ix.centres_f64(ci));
                        let (lb, ub) = NuCut::new(ix, ci, pr, get_gq2(ix, pr, c.g, &mut acc.st), qm).bounds(c.numax);
                        let nn = acc.iv.len();
                        let cnt = (c.r0..c.r1).filter(|&r| o.vis.map_or(true, |v| v.has(r))).count();
                        for x in &mut acc.iv[nn - cnt..] {
                            x.1 = lb as f32;
                            x.2 = ub as f32;
                        }
                    } else if o.want_iv {
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
                        let nc = if ix.nukey.is_empty() { None } else { Some(NuCut::new(ix, ci, pr, get_gq2(ix, pr, c.g, &mut acc.st), qm)) };
                        let cu = if nc.is_none() { Some(cut_params2(ix, ci, pr, qm)) } else { None };
                        for r in (u0..d0).chain(d1..u1) {
                            if o.vis.map_or(true, |v| v.has(r)) {
                                acc.rows.push(r as u32);
                                let (lb, ub) = match &nc {
                                    Some(nc) => nc.bounds(nc.nu_of(ix.nukey[r])),
                                    None => key_interval2(ix, cu.as_ref().unwrap(), r),
                                };
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
                            let colp = pl.col && !o.want_iv && ix.col.is_some();
                            let mut cq = open_cluster(ix, ci, gq, pr, qm, pl, pl.int_det && !o.want_iv && !colp);
                            acc.st.cyc[P_CLSETUP] += tick() - tq;
                            let mut ivv = Vec::new();
                            let mut out = RunOut { rows_in: &mut acc.rows, pend: &mut pend, iv: if o.want_iv { Some(&mut ivv) } else { None } };
                            if colp {
                                unsafe { super::col::run_col(ix, ci, pr, gq, &mut cq, &cands, t, pl.colq, pl.collazy, o.verify, &mut sv, &mut acc.st, &mut out) };
                            } else {
                                unsafe { run_cluster(ix, ci, pr, gq, &mut cq, &cands, t, Mode2::Thr, first_decide, o.verify, &mut sv, &mut acc.st, &mut out) };
                            }
                            sv.recycle(&mut cq);
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
            acc.st.cyc[P_CLUSTER] += tick() - tcl;
            (acc, sv)
        },
    ).map(|x| x.0).collect();
    let tm = tick();
    let mut out = ThrOut2 { st: Stats::new(nl), npow: ix.meta.npow, ..Default::default() };
    out.st.cyc[P_CUT] += cut_cyc;
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
    out.st.cyc[P_MERGE] += tick() - tm;
    out
}

pub fn to_bitmap2(ix: &Index2, rows: &[u32]) -> RoaringBitmap {
    // many rows: set the entities' bits in a plain bitset and build the bitmap from its bytes (no sort)
    if rows.len() * 64 >= ix.ent_row.len() {
        let mut bits = vec![0u64; ix.ent_row.len() / 64 + 1];
        for &r in rows {
            let e = ix.row_ent[r as usize] as usize;
            bits[e / 64] |= 1 << (e % 64);
        }
        let bytes = unsafe { std::slice::from_raw_parts(bits.as_ptr() as *const u8, bits.len() * 8) };
        return RoaringBitmap::from_lsb0_bytes(0, bytes);
    }
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
    pub npow: u32, // the index's guarantee N, as 2^npow (Meta2::npow)
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

/// Clusters in descending order of q.m^ (their centre direction).
pub fn by_centre(ix: &Index2, pr: &Prep2, clusters: Vec<usize>) -> Vec<usize> {
    let mut byc: Vec<(f64, usize)> = clusters.into_iter().map(|ci| (dot64(&pr.q64, ix.centres_f64(ci)) / ix.cl[ci].mn, ci)).collect();
    byc.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
    byc.into_iter().map(|x| x.1).collect()
}

/// Intervals at `level` of the visible items of `clusters`, taken in order until `budget` items are read; sorted by
/// descending midpoint.
pub fn probe(ix: &Index2, pr: &Prep2, clusters: Vec<usize>, vis: Option<&Vis2>, level: usize, budget: usize, st: &mut Stats) -> Vec<(u32, f32, f32, u8)> {
    let mut iv: Vec<(u32, f32, f32, u8)> = Vec::new();
    let mut nvis = 0;
    let mut sv = State::default();
    for ci in clusters {
        let c = &ix.cl[ci];
        let cands = cands_of(ix, ci, c.r0, c.r1, vis);
        if cands.is_empty() {
            continue;
        }
        nvis += cands.len();
        st.touched += cands.len() as u64;
        let gq = get_gq2(ix, pr, c.g, st);
        let qm = dot64(&pr.q64, ix.centres_f64(ci));
        let mut cq = cluster_q2(ix, ci, gq, pr, qm);
        let (mut rin, mut pend) = (Vec::new(), Vec::new());
        let mut out = RunOut { rows_in: &mut rin, pend: &mut pend, iv: Some(&mut iv) };
        unsafe { run_cluster(ix, ci, pr, gq, &mut cq, &cands, f64::NAN, Mode2::Interval(level), 0, false, &mut sv, st, &mut out) };
        if nvis >= budget {
            break;
        }
    }
    iv.sort_by(|x, y| ((y.1 + y.2) as f64).partial_cmp(&((x.1 + x.2) as f64)).unwrap());
    iv
}

/// The approximate mode's cluster order: q.m + f r, the centre score plus f times the cluster's RMS radius about its
/// centre (an optimistic estimate of its best item's score), with f fitted on corpus pseudo-queries.
pub struct Approx {
    pub f: f64,
    pub rad: Vec<f64>,
    pub second: Vec<Vec<u32>>, // per cluster, the rows assigned to it second (SOAR); empty unless computed
}

/// Corpus rows used as pseudo-queries (spread over the index, near-zero rows left out), as unit queries, with their
/// exact k nearest other rows.
pub fn pseudo_queries(ix: &Index2, nq: usize, kk: usize) -> Vec<(usize, Vec<f32>, Vec<u32>)> {
    let rows: Vec<usize> = (0..ix.nrows).filter(|&r| (r as u64 ^ 0x5bd1e995).wrapping_mul(0x9E3779B97F4A7C15) % ix.nrows as u64 * 2 < 3 * nq as u64).collect();
    let mut out: Vec<(usize, Vec<f32>, Vec<u32>)> = rows.par_iter().filter_map(|&row| {
        let q: Vec<f32> = ix.full_row_f16(row).iter().map(|&h| f16_to_f32(h)).collect();
        let qn = q.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt();
        if qn < 0.5 {
            return None;
        }
        let q: Vec<f32> = q.iter().map(|&x| (x as f64 / qn) as f32).collect();
        let (q16, q32, qn) = query_f16(&q);
        let me = ix.row_ent[row];
        let gt: Vec<u32> = scan_knn(ix, &q16, &q32, qn, kk + 1, None).into_iter().map(|x| x.1).filter(|&e| e != me).take(kk).collect();
        Some((row, q, gt))
    }).collect();
    out.truncate(nq);
    out
}

impl Approx {
    /// A second cluster for every row in the style of SOAR (lambda = 1): the centre c other than its own (m) that
    /// minimises |x - c|^2 + (r . (x - c))^2 / |r|^2 with r = x - m, so that the second cluster covers the queries
    /// along r, where the first is weakest. Rows are listed under their second cluster.
    pub fn second_assignment(ix: &Index2) -> Vec<Vec<u32>> {
        let (n, k) = (ix.n(), ix.k());
        let cen: Vec<f32> = ix.centres64.iter().map(|&x| x as f32).collect();
        let cn2: Vec<f32> = (0..k).map(|ci| cen[ci * n..(ci + 1) * n].iter().map(|x| x * x).sum()).collect();
        let dot = |a: &[f32], b: &[f32]| -> f32 {
            let mut acc = [0f32; 8];
            for (x, y) in a.chunks_exact(8).zip(b.chunks_exact(8)) {
                for i in 0..8 {
                    acc[i] += x[i] * y[i];
                }
            }
            acc.iter().sum()
        };
        let sec: Vec<u32> = ix.cl.par_iter().enumerate().flat_map_iter(|(c1, c)| {
            let (cen, cn2) = (&cen, &cn2);
            (c.r0..c.r1).map(move |row| {
                let x: Vec<f32> = ix.full_row_f16(row).iter().map(|&h| f16_to_f32(h)).collect();
                let m = &cen[c1 * n..(c1 + 1) * n];
                let r: Vec<f32> = x.iter().zip(m).map(|(a, b)| a - b).collect();
                let (x2, r2, rx) = (dot(&x, &x), dot(&r, &r).max(1e-30), dot(&r, &x));
                let mut best = (f32::INFINITY, c1);
                for ci in 0..k {
                    if ci == c1 {
                        continue;
                    }
                    let cv = &cen[ci * n..(ci + 1) * n];
                    let (xc, rc) = (dot(&x, cv), dot(&r, cv));
                    let par = rx - rc;
                    let loss = x2 - 2.0 * xc + cn2[ci] + par * par / r2;
                    if loss < best.0 {
                        best = (loss, ci);
                    }
                }
                best.1 as u32
            })
        }).collect();
        let mut out = vec![Vec::new(); k];
        for (row, &ci) in sec.iter().enumerate() {
            out[ci as usize].push(row as u32);
        }
        out
    }
    /// As `order`, from q.m of every cluster.
    pub fn order_qm(&self, qm: &[f64], vis: Option<&Vis2>) -> Vec<usize> {
        let mut keys: Vec<(f64, usize)> = (0..qm.len()).filter(|&ci| vis.map_or(true, |v| v.cl_cnt[ci] > 0))
            .map(|ci| (qm[ci] + self.f * self.rad[ci], ci)).collect();
        keys.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
        keys.into_iter().map(|x| x.1).collect()
    }
    /// Clusters holding a visible item, by descending q.m + f r.
    pub fn order(&self, ix: &Index2, q64: &[f64], vis: Option<&Vis2>) -> Vec<usize> {
        let mut keys: Vec<(f64, usize)> = (0..ix.k()).filter(|&ci| vis.map_or(true, |v| v.cl_cnt[ci] > 0))
            .map(|ci| (dot64(q64, ix.centres_f64(ci)) + self.f * self.rad[ci], ci)).collect();
        keys.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
        keys.into_iter().map(|x| x.1).collect()
    }

    /// Radii from the stored vectors; f from the grid {0, 0.1, 0.2, 0.35, 0.5, 0.75}, the one that reaches mean
    /// recall@10 0.95 on the pseudo-queries (exact scan of the probed clusters, same number of clusters for every
    /// query) with the fewest items scanned.
    pub fn fit(ix: &Index2, pseudo: &[(usize, Vec<f32>, Vec<u32>)]) -> (Approx, Vec<(f64, f64)>) {
        let n = ix.n();
        let rad: Vec<f64> = (0..ix.k()).into_par_iter().map(|ci| {
            let c = &ix.cl[ci];
            let m = ix.centres_f64(ci);
            let mut x = vec![0f64; n];
            let mut s = 0.0;
            for r in c.r0..c.r1 {
                ix.full_row_f64(r, &mut x);
                s += x.iter().zip(m).map(|(a, b)| (a - b) * (a - b)).sum::<f64>();
            }
            (s / (c.r1 - c.r0).max(1) as f64).sqrt()
        }).collect();
        let k = ix.k();
        let row_cl: Vec<u32> = ix.cl.iter().enumerate().flat_map(|(ci, c)| std::iter::repeat(ci as u32).take(c.r1 - c.r0)).collect();
        let mut res = Vec::new();
        for f in [0.0, 0.1, 0.2, 0.35, 0.5, 0.75] {
            let ap = Approx { f, rad: rad.clone(), second: Vec::new() };
            // per query: items scanned and items found by the first p clusters, p = 1..k
            let per: Vec<(Vec<f64>, Vec<f64>)> = pseudo.par_iter().map(|(_, q, gt)| {
                let pr = prep2(ix, q);
                let ord = ap.order(ix, &pr.q64, None);
                let mut rank = vec![0usize; k];
                for (i, &ci) in ord.iter().enumerate() {
                    rank[ci] = i;
                }
                let mut found = vec![0f64; k + 1];
                for &e in gt {
                    found[rank[row_cl[ix.ent_row[e as usize] as usize] as usize] + 1] += 1.0 / gt.len() as f64;
                }
                let mut items = vec![0f64; k + 1];
                for p in 1..=k {
                    found[p] += found[p - 1];
                    items[p] = items[p - 1] + (ix.cl[ord[p - 1]].r1 - ix.cl[ord[p - 1]].r0) as f64;
                }
                (found, items)
            }).collect();
            let nq = per.len() as f64;
            let p95 = (1..=k).find(|&p| per.iter().map(|x| x.0[p]).sum::<f64>() / nq >= 0.95).unwrap_or(k);
            res.push((f, per.iter().map(|x| x.1[p95]).sum::<f64>() / nq));
        }
        let best = res.iter().cloned().fold((0.0, f64::INFINITY), |a, b| if b.1 < a.1 { b } else { a });
        (Approx { f: best.0, rad, second: Vec::new() }, res)
    }
}

/// The confidence dial, fitted on corpus pseudo-queries. It cuts only the upper side of each item interval, the side
/// that prunes; lower bounds stay certified. Each level's factor for a miss rate eps is the (1 - eps) quantile of
/// (true - estimate) / half-width over the intervals that level computed, in a verified threshold query (index path),
/// for the items scoring at least the threshold, clipped to [0, 1]; eps = 0 gives 1 everywhere, the exact mode.
/// A pseudo-query whose row is past the index is a held-out query, not a stored row.
/// `fast` fits the best-first mode for k = kk: the threshold is the pseudo-query's max(kk, 10)-th best score, every
/// level gets a factor, and recall@kk is that of knn2_fast over every cluster. Without it, the fit is the one knn2_dial
/// was tuned with: the 1,000th best score, factor 1 on the deterministic levels, and recall@10 of knn2d. Then for each
/// target recall the largest eps that reaches it on the pseudo-queries.
pub struct Dial {
    pub eps: Vec<f64>,
    pub factors: Vec<Vec<f32>>, // per eps, per level
    pub recall: Vec<f64>,       // per eps, on the pseudo-queries
    pub fits: Vec<(f64, usize)>, // (target recall, index into eps)
}
impl Dial {
    pub fn for_target(&self, t: f64) -> &[f32] {
        let i = self.fits.iter().find(|x| (x.0 - t).abs() < 1e-9).expect("no dial fitted for that target").1;
        &self.factors[i]
    }
    pub fn for_eps(&self, e: f64) -> &[f32] {
        let i = self.eps.iter().position(|&x| (x - e).abs() <= 1e-12 * e.max(1e-12)).expect("no dial fitted for that eps");
        &self.factors[i]
    }
    pub fn fit(ix: &Index2, pseudo: &[(usize, Vec<f32>, Vec<u32>)], plan: Plan, targets: &[f64], kk: usize, fast: Option<&Approx>) -> Dial {
        let nl = ix.meta.nl();
        let pidx = Plan { mode: PlanMode::Index, ..plan };
        let kthr = if fast.is_some() { kk.max(10) } else { 1000 };
        // per pseudo-query: its errors and its kk nearest other rows
        ERRS.store(true, std::sync::atomic::Ordering::Relaxed);
        let per: Vec<(Vec<(u8, f32)>, Vec<u32>)> = pseudo.par_iter().map(|(row, q, _)| {
            let pr = prep2(ix, q);
            let best = scan_knn(ix, &pr.q16, &pr.q32, pr.qn, kthr.max(kk + 1), None);
            let t = best[kthr - 1].0;
            let me = ix.row_ent.get(*row).copied().unwrap_or(u32::MAX);
            let gt: Vec<u32> = best.iter().map(|x| x.1).filter(|&e| e != me).take(kk).collect();
            let o = Opts2 { vis: None, verify: true, want_iv: false, plan: pidx };
            (threshold2(ix, &pr, t, &o).st.errs, gt)
        }).collect();
        ERRS.store(false, std::sync::atomic::Ordering::Relaxed);
        let mut by_level: Vec<Vec<f32>> = vec![Vec::new(); nl];
        for (errs, _) in &per {
            for e in errs {
                if fast.is_some() || e.0 as usize >= NDL {
                    by_level[e.0 as usize].push(e.1);
                }
            }
        }
        for v in &mut by_level {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        }
        let eps = vec![0.0, 1e-5, 1e-4, 1e-3, 3e-3, 1e-2, 3e-2, 0.1, 0.2];
        let factors: Vec<Vec<f32>> = eps.iter().map(|&e| by_level.iter().map(|v| {
            if e == 0.0 || v.is_empty() { 1.0 } else { v[(((1.0 - e) * v.len() as f64) as usize).min(v.len() - 1)].clamp(0.0, 1.0) }
        }).collect()).collect();
        let mut recall = Vec::new();
        for f in &factors {
            let rec: f64 = pseudo.par_iter().zip(&per).map(|((row, q, _), (_, gt))| {
                let me = ix.row_ent.get(*row).copied().unwrap_or(u32::MAX);
                let out = match fast {
                    Some(ap) => knn2_fast(ix, ap, q, kk + 1, None, ix.k(), Some(f), plan),
                    None => knn2d(ix, q, kk + 1, None, pidx, Some(f)),
                };
                let got: Vec<u32> = out.ents.iter().map(|x| x.1).filter(|&e| e != me).take(kk).collect();
                got.iter().filter(|e| gt.contains(e)).count() as f64 / gt.len() as f64
            }).sum::<f64>() / pseudo.len() as f64;
            recall.push(rec);
        }
        let fits = targets.iter().map(|&t| (t, (0..eps.len()).filter(|&i| recall[i] >= t).max().unwrap_or(0))).collect();
        Dial { eps, factors, recall, fits }
    }
}

/// Approximate k-NN (IVF with a re-rank): the visible items of the first `nprobe` clusters in the order `ap` gives
/// (by centre direction when `ap` is None), scored by their interval at `level`; the `rerank` best by midpoint are
/// scored exactly from full vectors.
pub fn knn2_approx(ix: &Index2, ap: Option<&Approx>, q: &[f32], kk: usize, vis: Option<&Vis2>, nprobe: usize, rerank: usize, level: usize) -> KnnOut2 {
    let tm = Timer::new();
    let pr = prep2(ix, q);
    let mut st = Stats::new(ix.meta.nl());
    let mut order = match ap {
        Some(ap) => ap.order(ix, &pr.q64, vis),
        None => by_centre(ix, &pr, (0..ix.k()).filter(|&ci| vis.map_or(true, |v| v.cl_cnt[ci] > 0)).collect()),
    };
    order.truncate(nprobe);
    let iv = probe(ix, &pr, order, vis, level, usize::MAX, &mut st);
    let take = rerank.max(kk).min(iv.len());
    st.full_reads += take as u64;
    let mut fin: Vec<(f64, u32)> = iv[..take].iter().map(|x| (exact_value2(ix, &pr, x.0 as usize), ix.row_ent[x.0 as usize])).collect();
    fin.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(&b.1)));
    fin.truncate(kk);
    KnnOut2 { ents: fin, st, cands: iv.len(), t_seed: tm.ms(), t_sweep: 0.0, t_sel: 0.0, npow: ix.meta.npow }
}

/// A first k-th best score: the visible items of the 64 clusters with the largest bound, taken by centre score until
/// 2,000 are read at level 3, and the best 2k + 8 of those by interval midpoint scored exactly.
pub fn seed2(ix: &Index2, pr: &Prep2, kk: usize, vis: Option<&Vis2>, st: &mut Stats) -> f64 {
    let k = ix.k();
    let rr = ix.x2.sqrt();
    let a = pr.anchor;
    let live: Vec<usize> = (0..k).filter(|&ci| vis.map_or(true, |v| v.cl_cnt[ci] > 0)).collect();
    let mut cand: Vec<(f64, usize)> = if ix.nukey.is_empty() {
        live.iter().map(|&ci| {
            let (hi, lo) = (ix.hi[a * k + ci] as f64, ix.lo[a * k + ci] as f64);
            let bh = if pr.beta >= 0.0 { pr.beta * hi } else { pr.beta * lo };
            (bh + pr.rho * rr, ci)
        }).collect()
    } else {
        let mut gs: Vec<usize> = live.iter().map(|&ci| ix.cl[ci].g).collect();
        gs.sort_unstable();
        gs.dedup();
        setup_groups(ix, pr, &gs, st);
        live.iter().map(|&ci| {
            let qm = dot64(&pr.q64, ix.centres_f64(ci));
            (NuCut::new(ix, ci, pr, pr.gq[ix.cl[ci].g].get().unwrap(), qm).bounds(ix.cl[ci].numax).1, ci)
        }).collect()
    };
    cand.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
    cand.truncate(64);
    let clusters: Vec<usize> = cand.iter().map(|&(_, ci)| ci).collect();
    let seed_iv = probe(ix, pr, by_centre(ix, pr, clusters), vis, 3, 2000, st);
    let take = (2 * kk + 8).min(seed_iv.len());
    let mut ex: Vec<f64> = seed_iv[..take].iter().map(|x| exact_value2(ix, pr, x.0 as usize)).collect();
    st.full_reads += take as u64;
    ex.sort_by(|a, b| b.partial_cmp(a).unwrap());
    if ex.len() >= kk { ex[kk - 1] } else { f64::NEG_INFINITY }
}

/// Exact k-NN. The planner may send the query to a scan of the visible set before anything else (a small visible
/// set) or after the seed and the cut (nothing prunes); otherwise the threshold query at the seed's k-th score runs
/// as planned and the selection refines the survivors' intervals until the k best are certain.
pub fn knn2(ix: &Index2, q: &[f32], kk: usize, vis: Option<&Vis2>, plan: Plan) -> KnnOut2 {
    knn2d(ix, q, kk, vis, plan, None)
}

/// Approximate k-NN: exact k-NN over the visible items of the first `nprobe` clusters in the order of `ap` (with
/// `soar`, also the visible items assigned to them second), with each level's item upper half-widths scaled by `dial`
/// (per level). Only visible items are read; nprobe = all clusters and no
/// dial is the exact mode.
pub fn knn2_dial(ix: &Index2, ap: &Approx, q: &[f32], kk: usize, vis: Option<&Vis2>, nprobe: usize, dial: Option<&[f32]>, soar: bool, plan: Plan) -> KnnOut2 {
    let (_, q32, _) = query_f16(q);
    let q64: Vec<f64> = q32.iter().map(|&x| x as f64).collect();
    let mut order = ap.order(ix, &q64, vis);
    order.truncate(nprobe);
    let sub = Vis2::restrict(ix, vis, &order, if soar { Some(&ap.second) } else { None });
    knn2d(ix, q, kk, Some(&sub), plan, dial)
}

/// The k best (score, entity) seen so far, as a min-heap on score.
struct TopK {
    kk: usize,
    h: std::collections::BinaryHeap<std::cmp::Reverse<Scored>>,
}
#[derive(PartialEq)]
struct Scored(f64, u32);
impl Eq for Scored {}
impl PartialOrd for Scored {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Scored {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&o.0).then(o.1.cmp(&self.1))
    }
}
impl TopK {
    fn full(&self) -> bool {
        self.h.len() >= self.kk
    }
    /// The k-th best score, or -inf until k are held.
    fn kth(&self) -> f64 {
        if self.full() { self.h.peek().unwrap().0 .0 } else { f64::NEG_INFINITY }
    }
    fn push(&mut self, s: f64, e: u32) {
        if !self.full() {
            self.h.push(std::cmp::Reverse(Scored(s, e)));
        } else if s > self.kth() {
            self.h.pop();
            self.h.push(std::cmp::Reverse(Scored(s, e)));
        }
    }
    /// Scores `rows` from their stored vectors (f32 products) and keeps the k best.
    fn score(&mut self, ix: &Index2, pr: &Prep2, rows: &[u32], st: &mut Stats) {
        st.full_reads += rows.len() as u64;
        let mut ch = rows.chunks_exact(4);
        for r in &mut ch {
            let sc = unsafe { crate::kernel::dot_f16_x4(&pr.q32, [0, 1, 2, 3].map(|i| ix.full_row_f16(r[i] as usize))) };
            for i in 0..4 {
                self.push(sc[i] as f64, ix.row_ent[r[i] as usize]);
            }
        }
        for &r in ch.remainder() {
            self.push(unsafe { crate::kernel::dot_f16(&pr.q32, ix.full_row_f16(r as usize)) } as f64, ix.row_ent[r as usize]);
        }
    }
}

/// Approximate k-NN, best-first over the first `nprobe` clusters in the order of `ap`, reading only visible rows.
/// Until k items are held, a cluster's visible items are scored from their stored vectors. After that, each cluster
/// runs as a threshold query at the running k-th best score: the key range and the levels drop an item once its upper
/// bound falls below it, each level's half-width scaled by `dial`, and the items left (or taken in by a certified
/// lower bound) are scored from their stored vectors into the k best. A cluster with too few visible items to pay for
/// the index path's per-cluster setup, by the plan's costs, has every visible item scored, batched with other such
/// clusters in row order. No dial and every cluster is exact up to the f32
/// products.
pub fn knn2_fast(ix: &Index2, ap: &Approx, q: &[f32], kk: usize, vis: Option<&Vis2>, nprobe: usize, dial: Option<&[f32]>, plan: Plan) -> KnnOut2 {
    let tm = Timer::new();
    let nl = ix.meta.nl();
    let t0 = tick();
    // q.m of every cluster, for the order, the anchor and each probed cluster
    let q64: Vec<f64> = query_f16(q).1.iter().map(|&x| x as f64).collect();
    let qm: Vec<f64> = (0..ix.k()).map(|ci| dot64(&q64, ix.centres_f64(ci))).collect();
    let mut pr = prep2_qm(ix, q, Some(&qm));
    if let Some(d) = dial {
        pr.dial = d.to_vec();
    }
    let mut st = Stats::new(nl);
    let mut order = ap.order_qm(&qm, vis);
    order.truncate(nprobe);
    st.cyc[P_PLAN] += tick() - t0;
    let mut top = TopK { kk, h: std::collections::BinaryHeap::with_capacity(kk + 1) };
    let mut sv = State::default();
    let (mut rows_in, mut pend) = (Vec::new(), Vec::new());
    let stored = matches!(ix.full, Full2::Real { .. });
    assert!(stored, "knn2_fast scores from stored vectors");
    let mut ncand = 0;
    // visible rows of clusters too small for the index path, scored together in row order
    let mut flat: Vec<u32> = Vec::new();
    let flush = |flat: &mut Vec<u32>, top: &mut TopK, st: &mut Stats| {
        let tf = tick();
        flat.sort_unstable();
        top.score(ix, &pr, flat, st);
        flat.clear();
        st.cyc[P_FULL] += tick() - tf;
    };
    let vis_rows = |a: usize, b: usize| -> &[u32] {
        let v = vis.unwrap();
        let i = v.rows.partition_point(|&r| (r as usize) < a);
        &v.rows[i..i + v.rows[i..].partition_point(|&r| (r as usize) < b)]
    };
    for ci in order {
        let c = &ix.cl[ci];
        let nv = vis.map_or(c.r1 - c.r0, |v| v.cl_cnt[ci] as usize);
        if (top.full() && nv as f64 * plan.full_us <= plan.index_us(nv, 1.0)) || (!top.full() && top.h.len() + flat.len() + nv <= kk) {
            match vis {
                None => flat.extend(c.r0 as u32..c.r1 as u32),
                Some(_) => flat.extend_from_slice(vis_rows(c.r0, c.r1)),
            }
            ncand += nv;
            st.touched += nv as u64;
            if flat.len() >= 256 || !top.full() {
                flush(&mut flat, &mut top, &mut st);
            }
            continue;
        }
        flush(&mut flat, &mut top, &mut st);
        let t = top.kth();
        rows_in.clear();
        pend.clear();
        let qm = qm[ci];
        let (u0, u1, d0, d1) = if top.full() && ix.nukey.is_empty() { sorted_range2(ix, ci, &cut_params2(ix, ci, &pr, qm), t) } else { (c.r0, c.r1, c.r0, c.r1) };
        // the key alone takes these in
        if u0 < d0 || d1 < u1 {
            for r in (u0..d0).chain(d1..u1) {
                if vis.map_or(true, |v| v.has(r)) {
                    pend.push(r as u32);
                }
            }
        }
        // the visible rows of [d0, d1) from the sorted list, not by testing every row
        let mut cands: Vec<u32> = match vis {
            None => (0..(d1 - d0) as u32).map(|i| i + (d0 - c.r0) as u32).collect(),
            Some(_) => vis_rows(d0, d1).iter().map(|&r| r - c.r0 as u32).collect(),
        };
        if !top.full() && cands.len() > kk - top.h.len() {
            // enough here to fill the k best: score the items nearest the centre direction (largest key), then run the
            // rest against the k-th of those
            let first = cands.split_off(cands.len() - (kk - top.h.len()));
            let rows: Vec<u32> = first.iter().map(|&i| (c.r0 + i as usize) as u32).collect();
            st.touched += rows.len() as u64;
            ncand += rows.len();
            top.score(ix, &pr, &rows, &mut st);
        }
        let t = top.kth();
        st.range_in += pend.len() as u64;
        st.range_out += ((u0 - c.r0) + (c.r1 - u1)) as u64;
        st.touched += (cands.len() + pend.len()) as u64;
        if !cands.is_empty() {
            let nv = vis.map_or(c.r1 - c.r0, |v| v.cl_cnt[ci] as usize).max(1);
            let d = cands.len() as f64 / nv as f64;
            // the group's setup is left out: charged to the first cluster of each group, it sends that cluster's items
            // to their full vectors, which reads far more and saves no time
            if top.full() && cands.len() as f64 * plan.full_us > plan.index_us(cands.len(), d) {
                st.clusters_open += 1;
                let gq = get_gq2(ix, &pr, c.g, &mut st);
                let tq = tick();
                let mut cq = open_cluster(ix, ci, gq, &pr, qm, plan, plan.int_det);
                st.cyc[P_CLSETUP] += tick() - tq;
                let mut out = RunOut { rows_in: &mut rows_in, pend: &mut pend, iv: None };
                unsafe { run_cluster(ix, ci, &pr, gq, &mut cq, &cands, t, Mode2::Thr, 0, false, &mut sv, &mut st, &mut out) };
                sv.recycle(&mut cq);
            } else {
                pend.extend(cands.iter().map(|&i| (c.r0 + i as usize) as u32));
            }
        }
        ncand += rows_in.len() + pend.len();
        let tf = tick();
        top.score(ix, &pr, &rows_in, &mut st);
        top.score(ix, &pr, &pend, &mut st);
        st.cyc[P_FULL] += tick() - tf;
    }
    flush(&mut flat, &mut top, &mut st);
    st.cyc[P_QUERY] += tick() - t0;
    let mut ents: Vec<(f64, u32)> = top.h.into_iter().map(|x| (x.0 .0, x.0 .1)).collect();
    ents.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(&b.1)));
    KnnOut2 { ents, st, cands: ncand, t_seed: 0.0, t_sweep: tm.ms(), t_sel: 0.0 }
}

/// knn2 with each level's item upper half-widths scaled by `dial`.
pub fn knn2d(ix: &Index2, q: &[f32], kk: usize, vis: Option<&Vis2>, plan: Plan, dial: Option<&[f32]>) -> KnnOut2 {
    let tm = Timer::new();
    let nl = ix.meta.nl();
    let stored = matches!(ix.full, Full2::Real { .. });
    let scan = |st: Stats, t_seed: f64| {
        let (q16, q32, qn) = query_f16(q);
        let ents = scan_knn(ix, &q16, &q32, qn, kk, vis);
        let mut st = st;
        st.full_reads += vis.map_or(ix.nrows, |v| v.rows.len()) as u64;
        KnnOut2 { ents, st, cands: 0, t_seed, t_sweep: tm.ms() - t_seed, t_sel: 0.0, npow: ix.meta.npow }
    };
    if let Some(v) = vis {
        if stored && plan.scan_first(v, plan.seed_us) {
            return scan(Stats::new(nl), 0.0);
        }
    }
    let mut pr = prep2(ix, q);
    if let Some(d) = dial {
        pr.dial = d.to_vec();
    }
    let mut st = Stats::new(nl);
    let tau1 = seed2(ix, &pr, kk, vis, &mut st);
    let t_seed = tm.ms();
    let o = Opts2 { vis, verify: false, want_iv: true, plan };
    let sw = match plan2(ix, &pr, tau1, &o, 0.0) {
        Planned::Scan if stored => return scan(st, t_seed),
        Planned::Scan => scan_visible(ix, &pr, tau1, vis, true, plan.pscan),
        Planned::Run { pre, use_idx, first_decide, cut_cyc } => run_planned(ix, &pr, tau1, &o, &pre, &use_idx, first_decide, cut_cyc),
    };
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
    let mut fin: Vec<(f64, u32)> = select_knn(cands, kk, nl, |r, lv| row_interval2(ix, &pr, r as usize, lv, &mut st), |r| exact_value2(ix, &pr, r as usize))
        .into_iter().map(|(v, r)| (v, ix.row_ent[r as usize])).collect();
    fin.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(&b.1)));
    fin.truncate(kk);
    let t_sel = tm.ms() - t_seed - t_sweep;
    KnnOut2 { ents: fin, st, cands: ncand, t_seed, t_sweep, t_sel, npow: ix.meta.npow }
}

/// The k best of `cands` (row, lower bound, upper bound, next level, exact), refining intervals a level at a time:
/// `refine(row, level)` is the row's interval at that level, its exact score (lower = upper) at level `nl`. A candidate
/// is dropped when at least k other candidates have a lower bound above its upper bound, and certified when at most
/// k - 1 others have an upper bound at or above its lower bound; an exact score replaces the interval. Intervals are
/// intersected across levels. Returns the survivors with their exact scores, or +inf where certified without one; the
/// caller keeps the k largest. The answer is exact when every upper bound of the true k best holds and every lower bound of every
/// other candidate holds, whatever the other sides do; the candidates must contain the k best.
pub fn select_knn(mut cands: Vec<(u32, f64, f64, usize, bool)>, kk: usize, nl: usize, mut refine: impl FnMut(u32, usize) -> (f64, f64), mut exact: impl FnMut(u32) -> f64) -> Vec<(f64, u32)> {
    let desc = |v: &mut Vec<f64>| v.sort_by(|a, b| b.partial_cmp(a).unwrap());
    // the candidate's own bound is left out of each count
    let certified = |c: &(u32, f64, f64, usize, bool), ubs: &[f64]| ubs.partition_point(|&u| u >= c.1) - ((c.2 >= c.1) as usize) < kk;
    loop {
        if cands.len() <= kk {
            break;
        }
        let mut lbs: Vec<f64> = cands.iter().map(|c| c.1).collect();
        desc(&mut lbs);
        cands.retain(|c| lbs.partition_point(|&l| l > c.2) - ((c.1 > c.2) as usize) < kk);
        if cands.len() <= kk {
            break;
        }
        let mut ubs: Vec<f64> = cands.iter().map(|c| c.2).collect();
        desc(&mut ubs);
        let mut refined = 0;
        for c in cands.iter_mut() {
            if c.4 || certified(c, &ubs) {
                continue;
            }
            refined += 1;
            let (l, h) = refine(c.0, c.3);
            *c = if c.3 >= nl { (c.0, l, h, c.3 + 1, true) } else { (c.0, l.max(c.1), h.min(c.2), c.3 + 1, false) };
        }
        if refined == 0 {
            break;
        }
    }
    let mut ubs: Vec<f64> = cands.iter().map(|c| c.2).collect();
    desc(&mut ubs);
    cands.iter().map(|c| {
        let v = if c.4 { c.1 } else if certified(c, &ubs) { f64::INFINITY } else { exact(c.0) };
        (v, c.0)
    }).collect()
}

#[cfg(test)]
mod knn_tests {
    use super::select_knn;
    use rand::{Rng, SeedableRng};
    use rand_distr::{Distribution, StandardNormal};

    /// reports/bounds3/knn_onesided.py: random scores, L levels of intervals of half-width 0.6 x 0.45^l around a
    /// shifted centre; with `harmful_only` the true k best have valid upper bounds and invalid lower bounds as often as
    /// not, and every other item the reverse. Returns the number of wrong answers.
    fn wrong_answers(harmful_only: bool, trials: usize, seed: u64) -> usize {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
        let mut bad = 0;
        for _ in 0..trials {
            let n = rng.gen_range(8..40usize);
            let k = rng.gen_range(1..6usize);
            let l = rng.gen_range(1..5usize);
            let s: Vec<f64> = (0..n).map(|_| StandardNormal.sample(&mut rng)).collect();
            let mut ord: Vec<usize> = (0..n).collect();
            ord.sort_by(|&a, &b| s[b].partial_cmp(&s[a]).unwrap());
            let mut top: Vec<u32> = ord[..k].iter().map(|&i| i as u32).collect();
            top.sort();
            let iv: Vec<Vec<(f64, f64)>> = (0..n).map(|i| (0..l).map(|lv| {
                let h = 0.6 * 0.45f64.powi(lv as i32);
                let e = if !harmful_only {
                    rng.gen_range(-h..h)
                } else if top.contains(&(i as u32)) {
                    rng.gen_range(-h..3.0 * h)
                } else {
                    rng.gen_range(-3.0 * h..h)
                };
                (s[i] + e - h, s[i] + e + h)
            }).collect()).collect();
            let cands = (0..n).map(|i| (i as u32, iv[i][0].0, iv[i][0].1, 1, false)).collect();
            let refine = |r: u32, lv: usize| if lv >= l { (s[r as usize], s[r as usize]) } else { iv[r as usize][lv] };
            let mut fin = select_knn(cands, k, l, refine, |r| s[r as usize]);
            fin.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
            let mut got: Vec<u32> = fin[..k.min(fin.len())].iter().map(|x| x.1).collect();
            got.sort();
            bad += (got != top) as usize;
        }
        bad
    }

    #[test]
    fn exact_when_only_the_harmful_side_of_each_interval_holds() {
        assert_eq!(wrong_answers(false, 4000, 1), 0);
        assert_eq!(wrong_answers(true, 4000, 2), 0);
    }
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
    fn rows(&self, _p: &Params) -> usize {
        self.sy.sizes.iter().sum()
    }
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
        Items2 { ah: key.iter().map(|k| k.0).collect(), z: zs, ent: key.iter().map(|k| k.1).collect(), ext: key.iter().map(|k| k.1).collect(), x2max, nu: Vec::new() }
    }
}
