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
    pub qvd2: Vec<f32>,    // ndl x n: qv * D_l^2
    pub qdt2: Vec<f32>,    // J x n: qv * dt^2 on the coordinates of block j, else 0
    pub wq: Vec<f32>,      // det: qv*sig by coordinate; rand: R_j (qv*dt) by position
    pub cq: [f64; NDL_MAX],    // sum qv^2 D_l^2
    pub qvt_pos: Vec<f64>, // qv*dt in position order
    pub a2: [f64; J],
    pub l1det: f64,        // |wq| sums (weight rounding bound)
    pub l1j: [f64; J],
    pub cyc: (u64, u64),   // basis product, the rest of the group setup (rotations included when done per group)
    pub gtab: Vec<OnceLock<GTab>>,     // f32 tables of wq per level, combined per cluster with the centre's
}

pub struct Prep2 {
    pub q16: Vec<u16>,
    pub q32: Vec<f32>,
    pub q64: Vec<f64>,
    pub qn: f64,
    pub qhi: Vec<i16>,
    pub qlo: Vec<i16>,
    /// The nearest anchor, found on first use: a plan that scans never reads the anchors.
    anc: OnceLock<Anc>,
    /// q.m of the clusters asked for on first use (`qms`), NaN for the rest.
    qm: OnceLock<Vec<f64>>,
    pub gq: Vec<OnceLock<GQ2>>,
    pub dial: Vec<f32>, // per level, factor on the item intervals' upper half-width (1: certified; below: approximate)
}

/// The anchor p with the largest q.p, beta = q.p exactly, and rho the norm of the rest of q (rounded up).
#[derive(Clone, Copy, Debug)]
pub struct Anc {
    pub a: usize,
    pub beta: f64,
    pub rho: f64,
}

impl Prep2 {
    /// The nearest anchor (searched over every anchor on the first call).
    pub fn anc(&self, ix: &Index2) -> Anc {
        self.anc_among(ix, None)
    }
    /// The nearest anchor; on the first call with `among` (clusters, when anchor a is cluster a's centre: anc_centres), the
    /// nearest of those clusters' anchors. Any anchor gives sound bounds, and a query over a few clusters needs the
    /// one nearest to them. The search ranks the anchors cheaply (by q.m / |m| from `qms` when the anchors are the
    /// centres' directions, else in f32) and takes q.p in f64 of each within the ranking's error of the best, so the
    /// anchor is the one with the largest f64 q.p (the first of equals).
    pub fn anc_among(&self, ix: &Index2, among: Option<&[usize]>) -> Anc {
        *self.anc.get_or_init(|| {
            let n = ix.n();
            let all: Vec<usize>;
            let centres = ix.anc_centres && ix.nanch == ix.k();
            let cand: &[usize] = match among.filter(|_| centres) {
                Some(c) if !c.is_empty() => c,
                _ => {
                    all = (0..ix.nanch).collect();
                    &all
                }
            };
            let score: Vec<f64> = if centres {
                self.qms(ix, Some(cand));
                cand.iter().map(|&a| self.qm(ix, a) / ix.cl[a].mn).collect()
            } else {
                cand.iter().map(|&a| unsafe { dot32(&self.q32, &ix.anchors32[a * n..(a + 1) * n]) }).collect()
            };
            // the ranking's error bound on each score: f64 rounding of q.m and of the division, or the f32 product of
            // the rounded anchor (64 u for the sum, 2^-24 for the rounding, relative to |q| |p| = |q|)
            let err = if centres { 1e-12 * self.qn + 1e-300 } else { 70.0 * U * self.qn + 1e-300 };
            let top = score.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let (mut best, mut ba) = (f64::NEG_INFINITY, 0);
            for (i, &a) in cand.iter().enumerate() {
                if score[i] >= top - 2.0 * err {
                    let b = unsafe { dot64v(&self.q64, &ix.anchors[a * n..(a + 1) * n]) };
                    if b > best {
                        best = b;
                        ba = a;
                    }
                }
            }
            anc_of(self.qn, ba, best)
        })
    }

    /// q.m of every cluster of `among` (every cluster when None), computed on the first call (in parallel; NaN for the
    /// clusters left out); later calls return the same, whatever they ask for, so `qm` falls back for the rest.
    pub fn qms(&self, ix: &Index2, among: Option<&[usize]>) -> &[f64] {
        self.qm.get_or_init(|| {
            let mut v = vec![f64::NAN; ix.k()];
            match among {
                None => v.par_iter_mut().enumerate().for_each(|(ci, x)| *x = ix.qm(&self.q64, ci)),
                Some(c) => {
                    let got: Vec<f64> = c.par_iter().map(|&ci| ix.qm(&self.q64, ci)).collect();
                    for (&ci, x) in c.iter().zip(got) {
                        v[ci] = x;
                    }
                }
            }
            v
        })
    }

    /// q.m of cluster ci (from `qms` when it holds it).
    #[inline]
    pub fn qm(&self, ix: &Index2, ci: usize) -> f64 {
        match self.qm.get().map(|v| v[ci]) {
            Some(x) if !x.is_nan() => x,
            _ => ix.qm(&self.q64, ci),
        }
    }
}

fn anc_of(qn: f64, a: usize, beta: f64) -> Anc {
    Anc { a, beta, rho: (qn * qn - beta * beta).max(0.0).sqrt() * (1.0 + 1e-12) + 1e-12 }
}

/// The query rounded to f16, as f16 and as f32, and its norm.
pub fn query_f16(q: &[f32]) -> (Vec<u16>, Vec<f32>, f64) {
    let q16: Vec<u16> = q.iter().map(|&x| f32_to_f16(x)).collect();
    let q32: Vec<f32> = q16.iter().map(|&h| f16_to_f32(h)).collect();
    let qn = q32.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt();
    (q16, q32, qn)
}

pub fn prep2(ix: &Index2, q: &[f32]) -> Prep2 {
    let (q16, q32, qn) = query_f16(q);
    let q64: Vec<f64> = q32.iter().map(|&x| x as f64).collect();
    let fx: Vec<i64> = q16.iter().map(|&h| f16_fixed(h)).collect();
    let qhi = fx.iter().map(|&v| (v >> 13) as i16).collect();
    let qlo = fx.iter().map(|&v| (v & 8191) as i16).collect();
    Prep2 { q16, q32, q64, qn, qhi, qlo, anc: OnceLock::new(), qm: OnceLock::new(), gq: (0..ix.groups.len()).map(|_| OnceLock::new()).collect(), dial: vec![1.0; ix.meta.nl()] }
}

/// V_q^T q exactly: int8 basis x (q * 2^24 split into 13-bit halves), i32 sums (VNNI), then one rounding to f64.
/// Four basis vectors at a time share the query loads; the next four are prefetched meanwhile (setup::qv_exact_v).
#[cfg(target_feature = "avx512f")]
pub unsafe fn qv_exact(v8t: &[i8], vsc: &[f64], n: usize, qhi: &[i16], qlo: &[i16], out: &mut [f64]) {
    super::setup::qv_exact_v::<4, 1>(v8t, vsc, n, qhi, qlo, out)
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
    // cq[l] = sum qv^2 D_l^2 in four partial sums (any order is within the 1e-12 cq slack of cluster_q2w), with
    // qvd2 = qv D_l^2 in the same pass
    let ndl = ix.meta.ndl();
    let mut cq = [0f64; NDL_MAX];
    let mut qvd2 = vec![0f32; ndl * n];
    for l in 0..ndl {
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
    GQ2 { qv, qvd2, qdt2, wq, cq, qvt_pos, a2, l1det, l1j, cyc: (t1 - t0, t3 - t1), gtab: (0..ix.meta.nl()).map(|_| OnceLock::new()).collect() }
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
    pub dwa: [f32; NDL_MAX],
    pub nd: [f32; J],
    pub fixed: f32,
    pub ewdet: f32, // weight-rounding bound, det (times s)
    pub ewj: [f32; J], // per block (times kappa)
    pub w: Vec<f32>,
    pub luts: Vec<Option<LevelLut>>,
    pub gtab: bool,   // tables combined from the group's and the centre's
    pub repack: f32,  // compact by the index's per-level occupancy thresholds times this (0: at 0.6)
    pub tol: f64,     // threshold queries take an item in once its lower bound reaches t - tol (0: exact)
    pub ddec: u32,    // threshold queries decide at deterministic level l only when bit l is set (the plan's, else the index's Meta2::ddec)
    pub int: Option<IntDet>,
}

/// The integer deterministic levels of one (query, cluster): the common table step, and per level the margins and
/// the sums through that level of the units' minima (B) and of the tables' error bounds (e).
pub struct IntDet {
    pub sigma: f32,
    pub rt: Vec<Option<RTab>>,
    pub b: [f64; NDL_MAX],
    pub e: [f64; NDL_MAX],
    pub q: [f64; NDL_MAX], // bound on the largest Theta through each level
}

impl CQ2 {
    /// Decide the deterministic levels of threshold queries on the integer path.
    /// With combined tables (set `gtab` first), the common step comes from the units' range bounds: the group's sum
    /// and largest plus |alpha| times the centre's.
    pub fn enable_int(&mut self, ix: &Index2, ci: usize, gq: &GQ2) {
        let gp = &ix.groups[ix.cl[ci].g];
        let ndl = ix.meta.ndl();
        let sigma = if self.gtab {
            let ts: Vec<&GTab> = (0..ndl).map(|l| gq.gtab[l].get_or_init(|| unsafe { group_tab(&gq.wq, &gp.levels[l], &ix.etab) })).collect();
            let (s, m) = unsafe { det_spans(&ts, &ix.ctab[ci][..ndl], &gp.levels[..ndl], self.alr) };
            let units: usize = gp.levels[..ndl].iter().map(|l| l.units).sum();
            let budget = (256 * IHMAX - 3 * units as i64) as f64;
            ((s * (1.0 + 1.0 / 512.0) / budget).max(m * (1.0 + 1.0 / 512.0) / 65533.0) as f32).max(1e-30)
        } else {
            int_sigma(&self.w, &gp.levels[..ndl], &ix.etab)
        };
        self.int = Some(IntDet { sigma, rt: vec![None; ndl], b: [0.0; NDL_MAX], e: [0.0; NDL_MAX], q: [0.0; NDL_MAX] });
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
    cluster_q2w(ix, ci, gq, pr, qm, true)
}

/// As cluster_q2; without `want_w` the combined weights w are left empty (tables combined from the group's and the
/// centre's, `gtab`, never read them).
pub fn cluster_q2w(ix: &Index2, ci: usize, gq: &GQ2, pr: &Prep2, qm: f64, want_w: bool) -> CQ2 {
    let c = &ix.cl[ci];
    let n = ix.n();
    let gp = &ix.groups[c.g];
    // f32 products of f32-rounded factors: each sum is within (n/64 + 8) u sqrt(sum a^2 sum b^2) of the exact one
    let e = ((n / 64 + 8) as f64).max(70.0) * U;
    let mut ab = [0f64; J];
    for j in 0..J {
        ab[j] = unsafe { dot32(&c.mv32, &gq.qdt2[j * n..(j + 1) * n]) };
    }
    let alr = (ab.iter().sum::<f64>() / c.btot) as f32;
    let a = alr as f64;
    let ndl = ix.meta.ndl();
    let mut dwa = [0f32; NDL_MAX];
    for l in 0..ndl {
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
        w.extend(gq.wq[..n].iter().zip(c.mv32.iter()).zip(&gp.sig).map(|((&q, &m), &sg)| q - al * (m * sg)));
        w.extend(gq.wq[n..].iter().zip(c.wmr.iter()).map(|(&q, &m)| q - al * m));
    }
    let mut ewj = [0f32; J];
    for j in 0..J {
        ewj[j] = (4.0 * U * (gq.l1j[j] + a.abs() * c.l1m[1 + j] as f64) + 1e-30) as f32;
    }
    // |v_c| < 3.5 for a deterministic coordinate (outer tree centroid plus residual stages)
    let ewdet = (12.0 * U * (gq.l1det + a.abs() * c.l1m[0] as f64) + 1e-30) as f32;
    let fixed = a.abs() * c.mn * DA + gp.e_inv * (pr.qn + a.abs() * c.mn) * c.rmax + 4e-6;
    CQ2 { qm: qm as f32, alr, dwa, nd, fixed: fixed as f32, ewdet, ewj, w, luts: (0..ix.meta.nl()).map(|_| None).collect(), gtab: false, repack: 0.0, tol: 0.0, ddec: ix.meta.ddec, int: None }
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
        let qmv: f64 = gq.qv.iter().zip(c.mv32.iter()).map(|(a, &b)| a * b as f64).sum();
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
    let Anc { a, beta: b, rho } = pr.anc(ix);
    let (hi, lo) = (ix.hi[a * k + ci] as f64, ix.lo[a * k + ci] as f64);
    let (bh, bl) = if b >= 0.0 { (b * hi, b * lo) } else { (b * lo, b * hi) };
    let pm = if ix.apm.is_empty() { dot64(&ix.anchors[a * n..(a + 1) * n], &c.mhat) } else { ix.apm[a * k + ci] };
    let rm = cm - b * pm;
    let rp = (rho * rho - rm * rm).max(0.0).sqrt() * (1.0 + 1e-12) + 1e-12;
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
            let r = ix.row_of_ent(e);
            bits[r / 64] |= 1 << (r % 64);
        }
        let cl_cnt = ix.cl.iter().map(|c| count_bits(&bits, c.r0, c.r1) as u32).collect();
        let mut rows: Vec<u32> = bm.iter().map(|e| ix.row_of_ent(e) as u32).collect();
        rows.sort_unstable();
        Vis2 { bits, cl_cnt, rows }
    }
    #[inline]
    pub fn has(&self, r: usize) -> bool {
        self.bits[r / 64] >> (r % 64) & 1 == 1
    }
    /// Calls `f` with every visible row of [a, b), ascending, a word of the bit set at a time: the cost follows the
    /// visible rows and the words, not every row of the range.
    #[inline]
    pub fn each(&self, a: usize, b: usize, mut f: impl FnMut(usize)) {
        if a >= b {
            return;
        }
        let (wa, wb) = (a / 64, (b - 1) / 64);
        for w in wa..=wb {
            let mut x = self.bits[w];
            if w == wa {
                x &= !0u64 << (a % 64);
            }
            if w == wb {
                x &= !0u64 >> (63 - (b - 1) % 64);
            }
            while x != 0 {
                f(w * 64 + x.trailing_zeros() as usize);
                x &= x - 1;
            }
        }
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
/// Bound on |f32 product - exact product| of a stored vector and a query of norm qn (the dimensions past the last
/// multiple of 16 are added one at a time).
pub fn full_err_qn(ix: &Index2, qn: f64) -> f64 {
    ((ix.n() / 16 + 6 + ix.n() % 16) as f64) * U * qn * ix.x2.sqrt() * 1.01 + 1e-12
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

/// What the bounds of a randomised level read that is fixed for the level, and which of the survivors' state arrays
/// it writes (those a later level reads, all of them in verify mode).
struct RandLv<'a> {
    r0: usize, // the cluster's first row
    l: usize,
    lr: usize,
    coded: u32, // bit j: block j coded at this level
    rise: u32,  // bit j: block j rises here (its kappa and tan are decoded)
    row: [usize; J],
    kb8: u32, // bit j: an 8-bit kappa code
    ktab: [&'a [f32; 16]; J],
    khl: [&'a [[f32; 16]; 2]; J],
    ttab: [&'a [f32; 16]; J],
    yhl: &'a [[[f32; 16]; 2]; J],
    ypredw: [f32; J],
    nd: [f32; J],
    inb: [f32; J],   // 1 / (block size - 1)
    dk: [f32; J],    // kappa rounding bound and the estimate's 16 u
    ekw: [f32; J],   // table and weight error per unit kappa
    csgl: f32,
    dial: f32,
    alr: f32,
    qm: f32,
    edw: f32, // edet + ewdet
    fixed: f32,
    st_aj: u32,
    st_kap: u32,
    st_yh: bool,
    st_c0: bool,
}

/// The lanes of half `H` of a block whose 4-bit code in row `p` is 15 (the low nibbles of the row's 16 bytes are
/// slots 0..16, the high nibbles slots 16..32).
#[inline(always)]
unsafe fn esc15<const H: usize>(r: __m128i) -> u16 {
    let m = _mm_set1_epi8(if H == 0 { 0x0f } else { 0xf0u8 as i8 });
    _mm_movemask_epi8(_mm_cmpeq_epi8(_mm_and_si128(r, m), m)) as u16
}
/// The 4-bit codes of half `H` of a block in row `r`, as table indices (only their low 4 bits are read).
#[inline(always)]
unsafe fn nibs<const H: usize>(r: __m128i) -> __m512i {
    let v = _mm512_cvtepu8_epi32(r);
    if H == 0 { v } else { _mm512_srli_epi32(v, 4) }
}
#[inline(always)]
unsafe fn row16(sp: *const u8, row: usize) -> __m128i {
    _mm_loadu_si128(sp.add(row * 16) as *const _)
}

/// A randomised level's bounds of the 16 slots of half `H` of a block (lanes `hm`, slots o..o+16): |y| decoded at the
/// first such level, kappa and tan at a block's rise, the accumulators advanced by the level's part values `pv`.
/// Returns (lower bound, upper bound, estimate, half-width's deterministic part, sd).
#[inline(always)]
unsafe fn rand_half<const H: usize>(ix: &Index2, rl: &RandLv, sv: &mut State, sp: *const u8, pv: &[[__m512; 2]; J + 1], o: usize, hm: u16) -> (__m512, __m512, __m512, __m512, __m512) {
    let fix = |v: __m512, em: u16, slot: usize, idx: &[u32]| {
        let mut r = arr(v);
        for i in 0..16 {
            if em >> i & 1 == 1 {
                r[i] = ix.esc.get(rl.l as u8, slot as u8, (rl.r0 + idx[o + i] as usize) as u32);
            }
        }
        _mm512_loadu_ps(r.as_ptr())
    };
    let mut yh = [_mm512_setzero_ps(); J];
    let first = rl.lr == 0;
    if first {
        let s = ld(&sv.s, o);
        for (j, yj) in yh.iter_mut().enumerate() {
            let (a, b) = (row16(sp, 2 * j), row16(sp, 2 * j + 1));
            let pr_ = _mm512_mul_ps(_mm512_set1_ps(rl.ypredw[j]), s);
            let tv = _mm512_mul_ps(_mm512_permutexvar_ps(nibs::<H>(b), _mm512_loadu_ps(rl.yhl[j][1].as_ptr())), _mm512_permutexvar_ps(nibs::<H>(a), _mm512_loadu_ps(rl.yhl[j][0].as_ptr())));
            let mut y = _mm512_mul_ps(pr_, tv);
            let em = esc15::<H>(_mm_and_si128(a, b)) & hm;
            if em != 0 {
                y = fix(y, em, 1 + j, &sv.idx);
            }
            if rl.st_yh {
                _mm512_storeu_ps(sv.yh[j].as_mut_ptr().add(o), y);
            }
            *yj = y;
        }
    }
    // the constant parts, once at the first randomised level (the same f32 operations as before)
    let (c0, h0) = if first {
        let s = ld(&sv.s, o);
        let alrmr = _mm512_fmadd_ps(_mm512_set1_ps(rl.alr), ld(&sv.mr, o), _mm512_set1_ps(rl.qm));
        let sa = _mm512_mul_ps(s, ld(&sv.adet, o));
        let c0 = _mm512_add_ps(alrmr, sa);
        let fl = _mm512_add_ps(_mm512_abs_ps(alrmr), _mm512_abs_ps(sa));
        let h0 = _mm512_fmadd_ps(_mm512_set1_ps((16.0 * U) as f32), fl, _mm512_fmadd_ps(s, _mm512_set1_ps(rl.edw), _mm512_set1_ps(rl.fixed)));
        if rl.st_c0 {
            _mm512_storeu_ps(sv.c0.as_mut_ptr().add(o), c0);
            _mm512_storeu_ps(sv.h0.as_mut_ptr().add(o), h0);
        }
        (c0, h0)
    } else {
        (ld(&sv.c0, o), ld(&sv.h0, o))
    };
    let mut e = c0;
    let mut hpar = h0;
    let mut var = _mm512_setzero_ps();
    for j in 0..J {
        let yv = if first { yh[j] } else { ld(&sv.yh[j], o) };
        if rl.coded >> j & 1 == 0 {
            hpar = _mm512_fmadd_ps(yv, _mm512_set1_ps(rl.nd[j]), hpar);
            continue;
        }
        let (kap, vj, aj);
        if rl.rise >> j & 1 == 1 {
            let a = _mm512_add_ps(ld(&sv.aj[j], o), pv[1 + j][H]);
            if rl.st_aj >> j & 1 == 1 {
                _mm512_storeu_ps(sv.aj[j].as_mut_ptr().add(o), a);
            }
            aj = a;
            // a 4-bit code reads the table's first 16 entries; an 8-bit one is the product of its nibbles' entries (an
            // escape's value is replaced below)
            let row = rl.row[j];
            let kl = row16(sp, row);
            let (kv, kx, tr) = if rl.kb8 >> j & 1 == 1 {
                let kh = row16(sp, row + 1);
                let v = _mm512_mul_ps(_mm512_permutexvar_ps(nibs::<H>(kh), _mm512_loadu_ps(rl.khl[j][1].as_ptr())), _mm512_permutexvar_ps(nibs::<H>(kl), _mm512_loadu_ps(rl.khl[j][0].as_ptr())));
                (v, _mm_and_si128(kl, kh), row16(sp, row + 2))
            } else {
                (_mm512_permutexvar_ps(nibs::<H>(kl), _mm512_loadu_ps(rl.ktab[j].as_ptr())), kl, row16(sp, row + 1))
            };
            let mut k = _mm512_mul_ps(yv, kv);
            let mut th = _mm512_permutexvar_ps(nibs::<H>(tr), _mm512_loadu_ps(rl.ttab[j].as_ptr()));
            let ek = esc15::<H>(kx) & hm;
            let et = esc15::<H>(tr) & hm;
            if ek != 0 {
                k = fix(k, ek, 8 + j, &sv.idx);
            }
            if et != 0 {
                th = fix(th, et, 16 + j, &sv.idx);
            }
            let ab = _mm512_mul_ps(_mm512_mul_ps(_mm512_mul_ps(yv, th), _mm512_set1_ps(1.0 + 1e-6)), _mm512_set1_ps(rl.nd[j]));
            let v = _mm512_mul_ps(_mm512_mul_ps(ab, ab), _mm512_set1_ps(rl.inb[j]));
            if rl.st_kap >> j & 1 == 1 {
                _mm512_storeu_ps(sv.kap[j].as_mut_ptr().add(o), k);
                _mm512_storeu_ps(sv.var[j].as_mut_ptr().add(o), v);
            }
            (kap, vj) = (k, v);
        } else {
            (kap, vj, aj) = (ld(&sv.kap[j], o), ld(&sv.var[j], o), ld(&sv.aj[j], o));
        }
        let eb = _mm512_mul_ps(kap, aj);
        e = _mm512_add_ps(e, eb);
        // the kappa rounding bound and the estimate's 16 u share of |eb| in one product
        hpar = _mm512_fmadd_ps(_mm512_set1_ps(rl.dk[j]), _mm512_abs_ps(eb), hpar);
        hpar = _mm512_fmadd_ps(kap, _mm512_set1_ps(rl.ekw[j]), hpar);
        var = _mm512_add_ps(var, vj);
    }
    let sdv = _mm512_sqrt_ps(var);
    let hh = _mm512_mul_ps(_mm512_fmadd_ps(_mm512_set1_ps(rl.csgl), sdv, hpar), _mm512_set1_ps(1.0 + 1e-6));
    (_mm512_sub_ps(e, hh), _mm512_add_ps(e, _mm512_mul_ps(hh, _mm512_set1_ps(rl.dial))), e, hpar, sdv)
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
        let rho = if code == 15 { ix.esc.get(l as u8, 0u8, (c.r0 + sv.idx[k] as usize) as u32) as f64 } else { gp.pconst[l] as f64 * s * gp.ntab[l][code] as f64 };
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
pub unsafe fn run_cluster(ix: &Index2, ci: usize, pr: &Prep2, gq: &GQ2, cq: &mut CQ2, cands: &[u32], t: f64, mode: Mode2, verify: bool, sv: &mut State, st: &mut Stats, out: &mut RunOut) {
    let ti = tick();
    let c = &ix.cl[ci];
    let gp = &ix.groups[c.g];
    let nl = ix.meta.nl();
    let ndl = ix.meta.ndl();
    let n = ix.n();
    let nr = nl - ndl;
    // dense candidates keep their item positions (slot k = item a0 + k, holes for the rest), so blocks are source
    // blocks and need no re-packing; sparse ones are compacted
    let rp = cq.repack;
    let occ = |l: usize| if rp > 0.0 { (ix.repack[l] * rp).min(0.95) } else { 0.6 };
    let a0 = cands[0] / 32 * 32;
    let span = ((cands[cands.len() - 1] + 1 - a0) as usize + BLK - 1) / BLK * BLK;
    let mut aligned = cands.len() as f32 >= span as f32 * occ(0);
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
    let intm = cq.int.is_some() && !matches!(mode, Mode2::Interval(_));
    if intm {
        // Tout is shared by the deterministic levels, so the fixed term takes their largest factor; a larger factor
        // only lowers Tout. An item taken in must carry its interval (out.iv), which the integer levels do not give,
        // so with out.iv they take nothing in (Tin clamps to TMAX) and leave it to the randomised levels.
        let dmax = pr.dial[..ndl].iter().fold(0f32, |a, &d| a.max(d)) as f64;
        let (qm, alr, fixed) = (cq.qm as f64, cq.alr as f64, cq.fixed as f64);
        let sigma = cq.int.as_ref().unwrap().sigma as f64;
        let cin = if out.iv.is_some() { f64::INFINITY } else { t - cq.tol + fixed - qm };
        int_thresholds(&sv.s, &sv.mr, nsurv, cin, t - dmax * fixed - qm, alr, t.abs() + fixed.abs() + qm.abs(), sigma, &mut sv.ist);
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
    let mut edet = 0f32; // cumulative table error of the deterministic part
    let mut ej = [0f32; J];
    let mut g = Gather::new();
    // survivors entering the level (slots held in place may be holes)
    let mut live = cands.len();
    for l in 0..nl {
        if nsurv == 0 {
            break;
        }
        let lp = &gp.levels[l];
        // a deterministic level that does not decide (Meta2::ddec): its sums advance and nothing is decided, so every
        // live slot is kept (verify: its bounds are computed and checked, not used); a later level's bound does not
        // depend on whether an earlier one decided
        let dnodec = l < ndl && mode == Mode2::Thr && cq.ddec >> l & 1 == 0;
        let tl1 = tick();
        if intm && l == ndl {
            // into the randomised levels: the deterministic sum as a float, within e plus its rounding to f32
            let id = cq.int.as_ref().unwrap();
            let (b, sg) = (id.b[ndl - 1], id.sigma as f64);
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
            edet = ((id.e[ndl - 1] + (b.abs() + sg * THETA_MAX as f64) * 2f64.powi(-23)) * (1.0 + 1e-6)) as f32;
        }
        let tt = tick();
        st.cyc[P_LVSETUP] += tt - tl1;
        let lut: &LevelLut = {
            if cq.luts[l].is_none() {
                st.cyc[C_TABLES] += 1;
                st.cyc[C_UNITS] += lp.units as u64;
                st.cyc[C_TSURV] += live as u64;
                st.cyc[C_TSMALL] += (live <= 8) as u64;
                let mut lut = sv.lut();
                let step = if intm && l < ndl { Some(cq.int.as_ref().unwrap().sigma) } else { None };
                if cq.gtab {
                    let gt = gq.gtab[l].get_or_init(|| group_tab(&gq.wq, lp, &ix.etab));
                    combine_lut(gt, &ix.ctab[ci][l], lp, cq.alr, step, &mut lut);
                } else {
                    build_lut(&cq.w, lp, &ix.etab, step, &mut lut);
                }
                cq.luts[l] = Some(lut);
            }
            cq.luts[l].as_ref().unwrap()
        };
        st.cyc[P_TABLES] += tick() - tt;
        let tls = tick();
        for pl in &lut.parts {
            match pl.j {
                None => edet += pl.err,
                Some(j) => ej[j] += pl.err,
            }
        }
        if intm && l < ndl {
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
        let w64: Vec<f64> = if verify && l < ndl {
            if cq.gtab {
                let wm = c.wm(gp);
                gq.wq.iter().zip(&wm).map(|(&q, &m)| q as f64 - cq.alr as f64 * m as f64).collect()
            } else {
                cq.w.iter().map(|&x| x as f64).collect()
            }
        } else {
            Vec::new()
        };
        if verify && l < ndl {
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
        let lr = l.wrapping_sub(ndl);
        if l >= ndl {
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
        if l >= ndl && !last {
            need.yh = true;
            need.c0 = true;
            for j in 0..J {
                if gp.rates[l - ndl][j] > 0.0 {
                    need.aj |= 1 << j;
                    if !gp.thchg[l + 1 - ndl].contains(&j) {
                        need.kap |= 1 << j;
                    }
                }
            }
        }
        let rl = (l >= ndl).then(|| {
            let bits = |f: &dyn Fn(usize) -> bool| (0..J).fold(0u32, |m, j| m | (f(j) as u32) << j);
            // a block's accumulator advances where it rises (rand_half): every part of the level must be of a block
            // that rises here
            let parts = lut.parts.iter().fold(0u32, |m, p| m | 1 << p.j.unwrap());
            assert_eq!(parts & !bits(&|j| gp.thchg[lr].contains(&j)), 0, "a randomised level has a part of a block that does not rise there");
            let mut row = [0; J];
            for (slot, &j) in gp.thchg[lr].iter().enumerate() {
                row[j] = gp.rrow[lr][slot];
            }
            RandLv {
                r0: c.r0,
                l,
                lr,
                coded: bits(&|j| gp.rates[lr][j] > 0.0),
                rise: bits(&|j| gp.thchg[lr].contains(&j)),
                row,
                kb8: bits(&|j| gp.kbits[lr][j] == 8),
                ktab: std::array::from_fn(|j| gp.ktab[lr][j][..16].try_into().unwrap()),
                khl: std::array::from_fn(|j| &gp.khl[lr][j]),
                ttab: std::array::from_fn(|j| &gp.ttab[lr][j]),
                yhl: &gp.yhl,
                ypredw: gp.ypredw,
                nd: cq.nd,
                inb: std::array::from_fn(|j| 1.0 / (gp.nb[j] - 1) as f32),
                dk: std::array::from_fn(|j| dmxj[j] + (16.0 * U) as f32),
                ekw: std::array::from_fn(|j| ej[j] + cq.ewj[j]),
                csgl: gp.csgl[lr],
                dial: pr.dial[l],
                alr: cq.alr,
                qm: cq.qm,
                edw: edet + cq.ewdet,
                fixed: cq.fixed,
                st_aj: if verify { !0 } else { need.aj },
                st_kap: if verify { !0 } else { need.kap },
                st_yh: verify || need.yh,
                st_c0: verify || need.c0,
            }
        });
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
                let k1 = k0 + PF * BLK;
                let live_ahead = || {
                    let (x0, x1) = (_mm512_loadu_si512(sv.idx.as_ptr().add(k1) as *const _), _mm512_loadu_si512(sv.idx.as_ptr().add(k1 + 16) as *const _));
                    _mm512_cmpeq_epi32_mask(x0, none) & _mm512_cmpeq_epi32_mask(x1, none) != 0xffff
                };
                if b + PF < nb && live_ahead() {
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
            if verify && l < ndl {
                for i in 0..32 {
                    if act >> i & 1 == 1 {
                        sv.a64[k0 + i] += tab64.iter().enumerate().map(|(u, tv)| tv[nib(cp, u, i)]).sum::<f64>();
                    }
                }
            }
            if intm && l < ndl {
                let rt = cq.int.as_ref().unwrap().rt[l].as_ref().unwrap();
                let (inm, outm) = int_level(cp, lut, sp, rt, sv.ist.as_mut_ptr().add(ip(k0, 0)), !dnodec || verify);
                let t2 = tick();
                st.cyc[P_DECODE] += t2 - t1;
                let (inm, outm) = (inm & act, outm & act);
                if verify {
                    check_det(ix, ci, pr, cq, l, t, sv, k0, sp, act, inm, outm, st);
                }
                let k = if sweep || dnodec { act } else { act & !(inm | outm) };
                if !dnodec {
                    st.dec_in[l] += inm.count_ones() as u64;
                    st.dec_out[l] += outm.count_ones() as u64;
                }
                if !sweep && !dnodec && inm != 0 {
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
                let (s0, s1) = part_sums2(cp.add(pl.g0 * 64), lut.hi.as_ptr().add(pl.g0 * 64), lut.lo.as_ptr().add(pl.g0 * 64), pl.groups);
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
                let (lb, ub, est, hn, sd);
                if l < ndl {
                    let s = ld(&sv.s, o);
                    let alrmr = _mm512_fmadd_ps(_mm512_set1_ps(cq.alr), ld(&sv.mr, o), _mm512_set1_ps(cq.qm));
                    let adet = _mm512_add_ps(ld(&sv.adet, o), pv[0][h]);
                    _mm512_storeu_ps(sv.adet.as_mut_ptr().add(o), adet);
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
                                r[i] = ix.esc.get(l as u8, 0u8, row);
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
                    let rl = rl.as_ref().unwrap();
                    (lb, ub, est, hn, sd) = if h == 0 { rand_half::<0>(ix, rl, sv, sp, &pv, o, hm) } else { rand_half::<1>(ix, rl, sv, sp, &pv, o, hm) };
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
                            if l < ndl { st.viol_det += 1 } else { st.viol_rand += 1 }
                        }
                        if l >= ndl && sds[i] > 0.0 {
                            let zz = ((sc - es[i] as f64).abs() - hs[i] as f64) / sds[i] as f64;
                            if zz > st.max_sd {
                                st.max_sd = zz;
                            }
                        }
                    }
                }
                if verify && l >= ndl {
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
                let inm = _mm512_cmp_ps_mask::<_CMP_GE_OQ>(lb, _mm512_set1_ps(tin)) & hm;
                let outm = _mm512_cmp_ps_mask::<_CMP_LT_OQ>(ub, _mm512_set1_ps(tout)) & hm;
                if verify && l < ndl {
                    let sh = 16 * h;
                    check_det(ix, ci, pr, cq, l, t, sv, k0, sp, (hm as u32) << sh, (inm as u32) << sh, (outm as u32) << sh, st);
                }
                if dnodec {
                    keep[h] = hm;
                    continue;
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
                if l < ndl {
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
                        if l < ndl {
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
    let ndl = ix.meta.ndl();
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
                let (a, b) = part_sums2(cp.add(pl.g0 * 64), lut.hi.as_ptr().add(pl.g0 * 64), lut.lo.as_ptr().add(pl.g0 * 64), pl.groups);
                acc = _mm512_add_ps(acc, _mm512_add_ps(a, b));
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
        let narr = if l < ndl { 4 } else { 3 + 4 * J };
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
    /// The rule (vectors in memory) or the cost model (vectors on file, their reads priced): see `plan2`.
    Auto,
    /// Every cluster the support table leaves open is cut and run on the index path.
    Index,
    /// Every cluster the support table leaves open is cut and its candidates decided from their full vectors.
    Flat,
    /// Every query scans the visible set (every row without one); for experiments.
    Scan,
}

/// The planner's rule in memory, in rows: the index path on a cluster of S rows holding v visible rows (all S without a
/// visible set) costs about A + g * d * S + B * c, c of its candidates after the cut (d * v of them), A the cluster's
/// setup and cut; scoring its v rows from their vectors costs v rows of a scan. So a cluster is scored directly when
/// v <= R0 + kappa * S (`r_rows`, `r_kappa`), and the whole query scans when the visible rows number at most the sum of
/// that over the clusters holding one. Ratios of costs of the same kind (a pass over memory per row), so they do not
/// move with the machine's speed the way each cost does. The index path's cost grows with S even where few rows are
/// visible, and not through the candidate lists (built a word of the visible set at a time): most likely because
/// candidates dense enough are run in place, on whole blocks of the cluster's range with their holes. So kappa does
/// not vanish. Fitted on 1M BioCLIP-2 refit at 1,000, 4,000 and 16,129 rows a cluster (text queries at five
/// thresholds, random visible sets of 1-50%): forced index path and forced scan cross at 322, 1,053 and 4,048 visible
/// rows a cluster on one thread, 388, 1,293 and 5,000 on four, a line of 72 + 0.246 S and 78 + 0.305 S (within 6
/// rows of each point). With the AVX-512 kernels the index path got faster: at 1,000 rows a cluster the crossover fell
/// to 221 visible rows on one thread and 308 on four. R0 = 64 and kappa = 0.186 put it at 250 rows at 1,000 rows a
/// cluster (436 at 2,000), between the two (the owner's choice). Without a visible set, a cluster is scored directly
/// when it holds at most 78 rows (64 / 0.814), so the clusters of a full index take the index path and an index
/// compacted to a small visible set is scanned (REPORT_BUILD, "The planner without timing").
pub const R_ROWS: usize = 64;
/// The rule's rows per row of a cluster (kappa above).
pub const R_KAPPA: f64 = 0.186;
/// The exact k-NN seed in the same rows, saved when a filtered k-NN scans first: measured 1.18-1.45 ms against 0.19 us
/// a row on 1M BioCLIP-2 (k 10 and 100), 6,200-7,600 rows; the lower end. The seed is the anchor search over the
/// anchors, a probe of the 64 clusters with the largest bound to 2,000 items at 16 bytes (`seed_level`), and 2k + 8 exact products:
/// it grows with the number of anchors and with k, not with the clusters' rows. So at large k (beyond the 100
/// measured) this constant underestimates it, which leans toward the index path (the seed is paid, not saved).
pub const R_SEED: usize = 6000;

/// The planner's choices. In memory it needs no timed constant: the support table decides what it can, and the rule in
/// rows (`r_rows`, `r_kappa`: `R_ROWS`) decides between a scan, direct scoring and the index path, with or without a
/// visible set; on clusters of more than 78 rows and no visible set that is the index path. "In memory" means the
/// reads are not priced (`disk::ReadPolicy::priced` unset): vectors mapped from a file that are not resident count as
/// in memory too. With the full vectors on file and their reads priced
/// (`disk::ReadPolicy::priced`), a cost model in microseconds decides, whose read costs (`io_us`, `io_gbps`) are timed
/// once per machine and device (`disk_constants`) and whose compute costs are the fixed `COSTS`.
#[derive(Clone, Copy, Debug)]
pub struct Plan {
    pub mode: PlanMode,
    pub r_rows: usize,     // R0: a cluster of S rows is scored directly when it holds at most r_rows + r_kappa * S visible rows (every row without a visible set); the query scans when all of them are
    pub r_kappa: f64,      // kappa: the rule's rows per row of the cluster
    pub r_seed: usize,     // exact k-NN: the seed, in rows, that a scan first also saves
    pub int_det: bool,     // threshold queries decide the deterministic levels on the integer path
    pub gtab: bool,        // tables per (group, level), combined per cluster with the centre's stored tables
    pub repack: bool,      // per-level occupancy thresholds for compacting survivors held in place (Index2::repack)
    pub repack_mul: f32,   // their factor: the decisions' cost per block, which the timing leaves out, raises them
    pub tol: f64,          // the tolerance d: an item whose lower bound reaches t - d is taken in (0: exact)
    pub pscan: bool,       // a scan of the visible set copies their vectors into a buffer, a block at a time, and scans that
    // Full vectors on file with their reads priced: one read with QD reads in flight, per read (us), and the bandwidth
    // of large reads (GB/s). `Plan::default`'s are placeholders from another machine: where reads are priced, take
    // this machine's from `disk_constants` (or a plan file); `v2::open` does not, since pricing is set after the open.
    pub io_us: f64,
    pub io_gbps: f64,
    /// Threshold queries decide at deterministic level l only when bit l is set (u32::MAX: every level); None: the
    /// index's own choice (`Meta2::ddec`, from `BuildOptions::ddec`). A per-query choice: plans with different values
    /// run on one opened index. `Plan::check` refuses a level the index does not have.
    pub ddec: Option<u32>,
}
impl Default for Plan {
    fn default() -> Self {
        Plan { mode: PlanMode::Auto, r_rows: R_ROWS, r_kappa: R_KAPPA, r_seed: R_SEED, int_det: true, gtab: true, repack: true, repack_mul: 1.0, tol: 0.0, pscan: false, io_us: 5.7, io_gbps: 3.3, ddec: None }
    }
}

/// The compute costs of the cost model for vectors on file, and of `knn2_fast`'s choice per cluster, in microseconds on
/// one thread: fixed, as timed on 1M BioCLIP-2 (n = 768, AVX-512) by the calibration this planner replaced. With the
/// vectors on file the reads (`Plan::io_us`, `io_gbps`, timed per machine) dominate them.
#[derive(Clone, Copy, Debug)]
pub struct Costs {
    pub full_us: f64,  // exact decision from a full vector, per candidate
    pub item_us0: f64, // index path per candidate: item_us0 + item_us1 * d
    pub item_us1: f64,
    pub cl_us0: f64,   // index path per cluster (setup and tables): cl_us0 + cl_us1 * d
    pub cl_us1: f64,
    pub grp_us: f64,   // index path per group (exact basis product, rotated weights)
    pub scan_us: f64,  // scan of a visible set's rows, scattered over the index, per visible item
    pub cscan_us: f64, // scan of rows in order (every row of an index), per row
    pub anc_us: f64,   // the search for the query's nearest anchor, per query
    pub dir_us: f64,   // a cluster scored directly from its visible rows, per cluster (beyond its rows at scan cost)
    pub cut_us: f64,   // cluster cut and sorted range, per cluster holding a visible item
}
pub const COSTS: Costs = Costs { full_us: 0.19, item_us0: 0.0133, item_us1: 0.053, cl_us0: 2.67, cl_us1: 13.3, grp_us: 93.0, scan_us: 0.19, cscan_us: 0.18, anc_us: 430.0, dir_us: 1.08, cut_us: 2.45 };
impl Costs {
    /// Modelled index-path cost of one cluster with `ncand` candidates (group setup not included). d is the fraction
    /// of the visible items of opened clusters that the cut and the sorted range leave undecided.
    pub fn index_us(&self, ncand: usize, d: f64) -> f64 {
        ncand as f64 * (self.item_us0 + self.item_us1 * d) + self.cl_us0 + self.cl_us1 * d
    }
    /// Modelled cost of a scan, per row: of a visible set's rows (`filtered`), or of every row in order.
    pub fn scan_cost_us(&self, filtered: bool) -> f64 {
        if filtered { self.scan_us } else { self.cscan_us }
    }
}

impl Plan {
    /// This plan's choices against the index `ix`: an error when `ddec` names a deterministic level beyond `ix`'s.
    pub fn check(&self, ix: &Index2) -> Result<(), String> {
        match self.ddec {
            Some(d) if d != u32::MAX && d >> ix.meta.ndl() != 0 => {
                Err(format!("ddec {d:#b} names a deterministic level beyond this index's {} (levels 0 to {})", ix.meta.ndl(), ix.meta.ndl() - 1))
            }
            _ => Ok(()),
        }
    }
    /// The planner's constants as `name=value` lines: the rule's rows in memory and the disk constants.
    pub fn to_kv(&self) -> String {
        format!("r_rows={}\nr_kappa={}\nr_seed={}\nio_us={}\nio_gbps={}\n", self.r_rows, self.r_kappa, self.r_seed, self.io_us, self.io_gbps)
    }
    /// `base` with the constants of `name=value` lines (as `to_kv` writes them); an error names a line that is not one,
    /// or a value out of its range: rows are whole numbers, `r_kappa` a number of at least 0, the disk constants
    /// numbers above 0.
    pub fn from_kv(base: Plan, text: &str) -> Result<Plan, String> {
        let mut p = base;
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let (k, v) = line.split_once('=').ok_or_else(|| format!("plan line {line:?} is not name=value"))?;
            let (k, v) = (k.trim(), v.trim());
            let rows = || v.parse::<usize>().map_err(|_| format!("{k}={v}: rows are a whole number of at least 0"));
            let num = |pos: bool| match v.parse::<f64>() {
                Ok(x) if x.is_finite() && (x > 0.0 || (!pos && x == 0.0)) => Ok(x),
                _ => Err(format!("{k}={v}: {} a number {}", if k == "r_kappa" { "kappa is" } else { "the disk constant is" }, if pos { "above 0" } else { "of at least 0" })),
            };
            match k {
                "r_rows" => p.r_rows = rows()?,
                "r_seed" => p.r_seed = rows()?,
                "r_kappa" => p.r_kappa = num(false)?,
                "io_us" => p.io_us = num(true)?,
                "io_gbps" => p.io_gbps = num(true)?,
                k => return Err(format!("{k}={v} is not a plan constant (r_rows, r_kappa, r_seed, io_us, io_gbps); the planner no longer takes timed compute costs")),
            }
        }
        Ok(p)
    }
    /// Whether `name=value` lines (as `to_kv` writes them) give both disk constants, so that none need be timed.
    pub fn kv_gives_disk(text: &str) -> bool {
        let has = |name: &str| text.lines().any(|l| l.split_once('=').is_some_and(|(k, _)| k.trim() == name));
        has("io_us") && has("io_gbps")
    }

    /// The most visible rows a cluster of `rows` rows may hold and be scored directly (R0 + kappa * S, rounded down).
    pub fn direct_max(&self, rows: usize) -> usize {
        self.r_rows.saturating_add((self.r_kappa * rows as f64) as usize)
    }

    /// Full vectors on disk: rows this many bytes apart or closer are read with one read, the gap included, because
    /// reading the gap costs less than another read.
    pub fn io_gap(&self) -> usize {
        (self.io_us * self.io_gbps * 1e3) as usize
    }

    /// Modelled cost of reading `ncand` full vectors (of `rb` bytes) lying in a span of `span` rows from file: one read
    /// each, or the span in one sequential read when that is cheaper.
    pub fn disk_us(&self, ncand: usize, span: usize, rb: usize) -> f64 {
        if ncand == 0 {
            return 0.0;
        }
        (ncand as f64 * self.io_us).min(self.io_us + (span * rb) as f64 / (self.io_gbps * 1e3))
    }

    /// The disk constants (io_us, io_gbps) timed through the reader on `ix`'s full-vector file, from an empty cache:
    /// `io_us` from 4,096 single rows spread over the file, `io_gbps` from 32 MiB of consecutive rows, each the fastest
    /// of three timings. The file's pages are dropped from the cache before each and after (`disk::Rows::evict`), so
    /// timing leaves the vectors file out of the page cache: the first queries after it read their vectors from the
    /// device. None in memory, when the rows are not read through io_uring (`disk::ReadPolicy::uring`), or when no
    /// io_uring can be set up here.
    pub fn calibrate_disk(ix: &Index2) -> Option<(f64, f64)> {
        let rows = ix.own_rows()?;
        let n = ix.n();
        let time1 = |sel: Vec<u32>, gap: usize| -> Option<(f64, u64)> {
            rows.evict();
            let mut rd = super::disk::Reader::<()>::new(rows, n, gap)?;
            let tm = Timer::new();
            rd.push((), sel);
            let b = rd.pop().unwrap();
            let el = tm.ms() * 1e3;
            std::hint::black_box(b.row(0, n)[0]);
            rd.recycle(b);
            Some((el, rd.bytes))
        };
        let time = |sel: Vec<u32>, gap: usize| -> Option<(f64, u64)> {
            (0..3).map(|_| time1(sel.clone(), gap)).try_fold((f64::MAX, 0), |a, x| x.map(|x| if x.0 < a.0 { x } else { a }))
        };
        const NR: usize = 4096;
        let spread: Vec<u32> = (0..NR).map(|i| ((i as u64 * 2 + 1) * ix.nrows as u64 / (2 * NR as u64)) as u32).collect();
        let (t_rand, _) = time(spread, 0)?;
        let nseq = ((32usize << 20) / (2 * n)).min(ix.nrows);
        let (t_seq, bytes) = time((0..nseq as u32).collect(), usize::MAX / 2)?;
        rows.evict();
        Some((t_rand / NR as f64, bytes as f64 / t_seq / 1e3))
    }

    /// Exact k-NN on `ix`, before anything is read: a scan of the visible set (every row without one) when it holds at
    /// most the rule's rows (`direct_max`) summed over the clusters holding a visible row, plus the seed's `r_seed`
    /// rows (always under `Scan`, never under `Index` or `Flat`).
    pub fn scan_first(&self, ix: &Index2, vis: Option<&Vis2>) -> bool {
        match self.mode {
            PlanMode::Scan => true,
            PlanMode::Auto => {
                let (nvis, lim) = rule_rows(ix, vis, *self);
                nvis <= lim.saturating_add(self.r_seed)
            }
            _ => false,
        }
    }
}

/// The rows of the rule in memory on `ix`: the visible rows (every row without a visible set), and the most of them
/// that the clusters holding one may hold and each be scored directly (`Plan::direct_max`, summed).
pub fn rule_rows(ix: &Index2, vis: Option<&Vis2>, pl: Plan) -> (usize, usize) {
    let (mut nvis, mut lim) = (0usize, 0usize);
    for (ci, c) in ix.cl.iter().enumerate() {
        let s = c.r1 - c.r0;
        let v = vis.map_or(s, |v| v.cl_cnt[ci] as usize);
        if v > 0 {
            nvis += v;
            lim = lim.saturating_add(pl.direct_max(s));
        }
    }
    (nvis, lim)
}

/// The disk-constant cache's file in an index directory.
pub const DISK_CACHE: &str = "disk-constants.txt";
/// The version of the disk-constant cache's key: changed when the timing or the key changes, so older caches are timed
/// again.
pub const DISK_KEY_VERSION: u32 = 2;

/// The key the disk constants are cached under: the key's version; the machine (/etc/machine-id, else the host name);
/// the CPU (its model name, or on machines whose /proc/cpuinfo has none, such as aarch64, its implementer, part,
/// variant and revision); the device of the file `file` lies on (major:minor, its name, and the model and serial of
/// the device, or of the whole disk for a partition, where /sys gives them); the file system's type; and the read
/// policy the timing reads with (`policy`: reads in flight, the O_DIRECT and RWF_DONTCACHE thresholds). One line.
pub fn disk_key(file: &std::fs::File, policy: &super::disk::ReadPolicy) -> Result<String, String> {
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::io::AsRawFd;
    use std::sync::atomic::Ordering::Relaxed;
    let clean = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let read = |p: &str| std::fs::read_to_string(p).ok().map(|s| clean(&s)).filter(|s| !s.is_empty());
    let machine = read("/etc/machine-id").map(|m| format!("id {m}")).or_else(|| read("/proc/sys/kernel/hostname").map(|h| format!("host {h}"))).unwrap_or_else(|| "unknown".into());
    let info = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let field = |name: &str| info.lines().find(|l| l.split(':').next().is_some_and(|k| k.trim() == name)).and_then(|l| l.split_once(':')).map(|x| clean(x.1)).filter(|v| !v.is_empty());
    let cpu = field("model name").or_else(|| {
        let parts: Vec<String> = ["CPU implementer", "CPU architecture", "CPU variant", "CPU part", "CPU revision"].iter().filter_map(|f| field(f).map(|v| format!("{} {v}", f.trim_start_matches("CPU ")))).collect();
        (!parts.is_empty()).then(|| parts.join(" "))
    }).or_else(|| field("Hardware")).or_else(|| field("Processor")).unwrap_or_else(|| "unknown".into());
    let dev = file.metadata().map_err(|e| format!("the vectors file: {e}"))?.dev();
    let (maj, min) = (libc::major(dev), libc::minor(dev));
    let sys = format!("/sys/dev/block/{maj}:{min}");
    let name = std::fs::read_to_string(format!("{sys}/uevent")).ok().and_then(|s| s.lines().find_map(|l| l.strip_prefix("DEVNAME=").map(|x| x.to_string())));
    // a partition's model and serial are its disk's
    let disk = if std::path::Path::new(&format!("{sys}/partition")).exists() {
        std::fs::canonicalize(&sys).ok().and_then(|p| p.parent().map(|q| q.to_string_lossy().into_owned())).unwrap_or(sys.clone())
    } else {
        sys.clone()
    };
    let model = read(&format!("{disk}/device/model"));
    let serial = read(&format!("{disk}/device/serial")).or_else(|| read(&format!("{disk}/serial")));
    let mut st: libc::statfs = unsafe { std::mem::zeroed() };
    let fs = if unsafe { libc::fstatfs(file.as_raw_fd(), &mut st) } == 0 { format!("{:#x}", st.f_type as u64) } else { "unknown".into() };
    let lim = |x: usize| if x == usize::MAX { "none".to_string() } else { x.to_string() };
    Ok(format!(
        "v{DISK_KEY_VERSION}; machine {machine}; cpu {cpu}; device {maj}:{min}{}{}{}; fs {fs}; read qd {} direct {} dontcache {}",
        name.map_or(String::new(), |n| format!(" ({n})")),
        model.map_or(String::new(), |m| format!(" model {m}")),
        serial.map_or(String::new(), |m| format!(" serial {m}")),
        policy.qd.load(Relaxed),
        lim(policy.direct_min.load(Relaxed)),
        lim(policy.dontcache_min.load(Relaxed)),
    ))
}

/// The disk constants cached at `path` under `key`, when it holds them.
fn read_disk_cache(path: &std::path::Path, key: &str) -> Result<(f64, f64), String> {
    let Ok(s) = std::fs::read_to_string(path) else { return Err(format!("no cache at {}", path.display())) };
    let get = |name: &str| s.lines().find_map(|l| l.strip_prefix(name).and_then(|v| v.strip_prefix('='))).map(|v| v.trim().to_string());
    let parsed = (|| {
        let (us, gb) = (get("io_us")?.parse::<f64>().ok()?, get("io_gbps")?.parse::<f64>().ok()?);
        (us.is_finite() && us > 0.0 && gb.is_finite() && gb > 0.0).then_some(())?;
        Some((get("key")?, us, gb))
    })();
    match parsed {
        Some((k, us, gb)) if k == key => Ok((us, gb)),
        Some((k, ..)) => Err(format!("{} was for another machine, device or read policy ({k})", path.display())),
        None => Err(format!("{} is not a cache of disk constants", path.display())),
    }
}

/// An exclusive lock (flock) on the file `path`, created if need be, held until the result is dropped; None when the
/// file cannot be created or locked.
fn lock_file(path: &std::path::Path) -> Option<std::fs::File> {
    use std::os::unix::io::AsRawFd;
    let f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(path).ok()?;
    (unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX) } == 0).then_some(f)
}

/// Writes `text` to `path` through a temporary file of a name of its own (the process id and the time in nanoseconds)
/// in the same directory, synced, then renamed over `path`: a reader sees the old file or the new one, whole.
fn write_atomic(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let ns = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let name = path.file_name().map_or("cache".into(), |n| n.to_string_lossy().into_owned());
    let tmp = path.with_file_name(format!(".{name}.{}.{ns}.tmp", std::process::id()));
    let r = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r
}

/// The disk constants (io_us, io_gbps) cached at `path` under `key`, else `time()`'s, saved there under `key`; and
/// how they were found. A cache that is missing, unreadable or under another key is timed again, under an exclusive
/// lock on `path` with the extension `lock`, so that two opens do not time at once (each would skew the other's
/// timing): one that waited for the lock reads what the other saved. The cache is written whole (`write_atomic`). One
/// that cannot be locked or written is reported and the timed constants are used all the same. None when they could
/// be neither read nor timed.
pub fn cached_disk_constants(path: &std::path::Path, key: &str, time: impl FnOnce() -> Option<(f64, f64)>) -> (Option<(f64, f64)>, String) {
    cached_disk_constants_or(path, None, key, time)
}

/// `cached_disk_constants`, saving to `fallback` (the per-user cache) when `path`'s directory refuses the write
/// (permission, a read-only file system, not a directory), so that the next open reads them instead of timing again.
pub fn cached_disk_constants_or(path: &std::path::Path, fallback: Option<&std::path::Path>, key: &str, time: impl FnOnce() -> Option<(f64, f64)>) -> (Option<(f64, f64)>, String) {
    let why = match read_disk_cache(path, key) {
        Ok(c) => return (Some(c), format!("read from {} (key {key})", path.display())),
        Err(e) => e,
    };
    let lock = lock_file(&path.with_extension("lock"));
    // another open may have timed them while this one waited for the lock
    if lock.is_some() {
        if let Ok(c) = read_disk_cache(path, key) {
            return (Some(c), format!("read from {} (key {key}), saved by another open while this one waited", path.display()));
        }
    }
    let why = if why.starts_with("no cache") { format!("timed: {why}") } else { format!("timed again: {why}") };
    let why = if lock.is_none() { format!("{why} (not locked)") } else { why };
    let Some((us, gb)) = time() else { return (None, format!("{why}; but they cannot be timed here (no io_uring reader)")) };
    let text = format!("key={key}\nio_us={us}\nio_gbps={gb}\n");
    let why = match write_atomic(path, &text) {
        Ok(()) => format!("{why}; saved under key {key}"),
        Err(e) => match fallback.filter(|_| matches!(e.raw_os_error(), Some(libc::EACCES) | Some(libc::EPERM) | Some(libc::EROFS) | Some(libc::ENOTDIR))) {
            Some(f) => {
                if let Some(d) = f.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                match write_atomic(f, &text) {
                    Ok(()) => format!("{why}; not saved in {} ({e}), saved in {} under key {key}", path.display(), f.display()),
                    Err(e2) => format!("{why}; not saved ({e}; nor in {}: {e2})", f.display()),
                }
            }
            None => format!("{why}; not saved ({e})"),
        },
    };
    drop(lock);
    (Some((us, gb)), why)
}

/// FNV-1a, 64 bits: a hash that is the same in every build, for a cache file's name.
fn fnv1a64(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf29ce484222325u64, |h, &x| (h ^ x as u64).wrapping_mul(0x100000001b3))
}

/// The per-user cache directory for disk constants: $XDG_CACHE_HOME/vexact, else ~/.cache/vexact.
pub fn user_cache_dir() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME").filter(|v| !v.is_empty()).map(std::path::PathBuf::from).or_else(|| std::env::var_os("HOME").filter(|v| !v.is_empty()).map(|h| std::path::PathBuf::from(h).join(".cache")))?;
    Some(base.join("vexact"))
}

/// The per-user cache file for disk constants under `key` in the directory `user`.
pub fn user_cache_file(user: &std::path::Path, key: &str) -> std::path::PathBuf {
    user.join(format!("disk-constants-{:016x}.txt", fnv1a64(key.as_bytes())))
}

/// Whether a file can be created in `dir`: a probe file created and removed (the permission that matters for the
/// cache's atomic write, which a lock file opened for writing does not show).
fn dir_writable(dir: &std::path::Path) -> bool {
    let ns = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let probe = dir.join(format!(".vexact-probe.{}.{ns}", std::process::id()));
    match std::fs::OpenOptions::new().write(true).create_new(true).open(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// Where the disk constants under `key` are cached: `DISK_CACHE` in the index directory `dir`, unless it does not hold
/// them under `key` and either the per-user directory `user` (when given) does, or `dir` cannot be written (a
/// read-only index); then the per-user directory, in a file named by the key.
pub fn disk_cache_path(dir: &str, key: &str, user: Option<&std::path::Path>) -> std::path::PathBuf {
    let main = std::path::Path::new(dir).join(DISK_CACHE);
    match user {
        Some(u) if read_disk_cache(&main, key).is_err() => {
            let uf = user_cache_file(u, key);
            if read_disk_cache(&uf, key).is_ok() || !dir_writable(std::path::Path::new(dir)) {
                let _ = std::fs::create_dir_all(u);
                uf
            } else {
                main
            }
        }
        _ => main,
    }
}

/// Restores an atomic flag when dropped (also on a panic).
struct RestoreFlag<'a>(&'a std::sync::atomic::AtomicBool, bool);
impl Drop for RestoreFlag<'_> {
    fn drop(&mut self) {
        self.0.store(self.1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// `base` with the disk constants of this machine and the device of `ix`'s vectors file: read from `DISK_CACHE` in
/// `dir` (the index directory), or timed (`Plan::calibrate_disk`, a few seconds, reading through io_uring whatever the
/// rows' policy, and leaving the vectors file out of the page cache) and saved there when it is missing or its key
/// differs (`disk_key`). Where `dir` cannot be written (a read-only index), the per-user cache (`user_cache_dir`) is
/// used instead, under a file named by the key. `base` unchanged when the vectors are not on file or cannot be timed;
/// never an error. Logged.
///
/// A library user who sets `disk::ReadPolicy::priced` calls this (or sets `Plan::io_us` and `io_gbps`): the plan's
/// own are `Plan::default`'s placeholders.
pub fn disk_constants(ix: &Index2, base: Plan, dir: &str, log: &mut dyn FnMut(String)) -> Plan {
    disk_constants_in(ix, base, dir, user_cache_dir().as_deref(), log)
}

/// `disk_constants` with the per-user cache directory `user` (None: none).
pub fn disk_constants_in(ix: &Index2, base: Plan, dir: &str, user: Option<&std::path::Path>, log: &mut dyn FnMut(String)) -> Plan {
    let Some(super::disk::Rows::Map { file, policy, .. }) = ix.own_rows() else { return base };
    let key = match disk_key(file, policy) {
        Ok(k) => k,
        Err(e) => {
            log(format!("[plan] disk constants not timed ({e}); io_us={} io_gbps={} as given", base.io_us, base.io_gbps));
            return base;
        }
    };
    let t = Timer::new();
    let path = disk_cache_path(dir, &key, user);
    // the index directory's cache, falling back to the per-user one when the index directory refuses the write
    let fallback = user.filter(|_| path == std::path::Path::new(dir).join(DISK_CACHE)).map(|u| user_cache_file(u, &key));
    let (got, why) = cached_disk_constants_or(&path, fallback.as_deref(), &key, || {
        let _restore = RestoreFlag(&policy.uring, policy.uring.swap(true, std::sync::atomic::Ordering::Relaxed));
        Plan::calibrate_disk(ix)
    });
    let p = got.map_or(base, |(io_us, io_gbps)| Plan { io_us, io_gbps, ..base });
    log(format!("[plan] disk constants io_us={:.3} io_gbps={:.3} (rows up to {} bytes apart read together): {why}, in {:.1}s", p.io_us, p.io_gbps, p.io_gap(), t.s()));
    p
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
/// Scan chunks whose full vectors are requested ahead of the one being scanned (`--willneed`).
const SCAN_AHEAD: usize = 4;

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
    let nch = scan_chunks(ix, vis);
    // every row's full vector on file: read through io_uring, each thread's chunks read while it decides the earlier
    // ones (a filtered scan's scattered rows are read faster by madvise(WILLNEED) on the mapping, and stay cached)
    let fr = match ix.own_rows() {
        Some(fr @ super::disk::Rows::Map { .. }) if vis.is_none() && !packed && fr.uring() => Some(fr),
        _ => None,
    };
    let parts: Vec<ThrOut2> = if let Some(fr) = fr {
        let decide = |acc: &mut ThrOut2, rd: &mut super::disk::Reader<()>| {
            let b = rd.pop().unwrap();
            assert!(ix.vrow.is_empty(), "a reader batch holds file rows, which are answer rows only on an index that reads its own vectors (Index2::own_rows)");
            decide_full_from(ix, pr, t, &b.rows, want_exact, acc, |k| b.row(k, ix.n()));
            rd.recycle(b);
        };
        (0..nch).into_par_iter().fold(
            // no reader where io_uring cannot be set up: the rows are read through the mapping
            || (ThrOut2 { st: Stats::new(nl), npow: ix.meta.npow, ..Default::default() }, super::disk::Reader::<()>::new(fr, ix.n(), 0)),
            |(mut acc, mut rd), ch| {
                let mut rows = Vec::new();
                scan_rows(ix, vis, ch, &mut rows);
                match rd.as_mut() {
                    Some(r) if !fr.cached(&rows, ix.n()) => {
                        r.push((), rows);
                        while r.full() {
                            decide(&mut acc, r);
                        }
                    }
                    _ => decide_full(ix, pr, t, &rows, want_exact, &mut acc),
                }
                (acc, rd)
            },
        ).map(|(mut acc, rd)| {
            if let Some(mut rd) = rd {
                while !rd.is_empty() {
                    decide(&mut acc, &mut rd);
                }
            }
            acc
        }).collect()
    } else {
        (0..nch).into_par_iter().map(|ch| {
            let mut acc = ThrOut2 { st: Stats::new(nl), npow: ix.meta.npow, ..Default::default() };
            let mut rows = Vec::with_capacity(4096);
            // with willneed in the read policy, reading runs SCAN_AHEAD chunks ahead of the scan
            if let Some(fr) = ix.own_rows() {
                if fr.willneed_on() {
                    let lo = if ch == 0 { 1 } else { ch + SCAN_AHEAD };
                    for c2 in lo..=(ch + SCAN_AHEAD).min(nch - 1) {
                        scan_rows(ix, vis, c2, &mut rows);
                        fr.willneed(&rows, ix.n());
                        rows.clear();
                    }
                }
            }
            scan_rows(ix, vis, ch, &mut rows);
            if packed && vis.is_some() {
                decide_full_packed(ix, pr, t, &rows, want_exact, &mut acc);
            } else {
                decide_full(ix, pr, t, &rows, want_exact, &mut acc);
            }
            acc
        }).collect()
    };
    let mut out = ThrOut2 { st: Stats::new(nl), npow: ix.meta.npow, scanned: true, ..Default::default() };
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
    if let Full2::Real { .. } = &ix.full {
        if let Some(rows) = ix.own_rows() {
            rows.willneed(pend, ix.n());
        }
        decide_full_from(ix, pr, t, pend, want_exact, acc, |k| ix.full_row_f16(pend[k] as usize));
        return;
    }
    for &r in pend {
        if exact_ge2(ix, pr, r as usize, t, &mut acc.st) {
            acc.rows.push(r);
            if want_exact {
                acc.exact.push((r, exact_value2(ix, pr, r as usize)));
            }
        }
    }
}

/// decide_full over stored vectors given by `get(k)` for row `pend[k]`.
pub fn decide_full_from<'a>(ix: &Index2, pr: &Prep2, t: f64, pend: &[u32], want_exact: bool, acc: &mut ThrOut2, get: impl Fn(usize) -> &'a [u16]) {
    let e = full_err2(ix, pr);
    let mut k = 0;
    while k < pend.len() {
        let m = (pend.len() - k).min(4);
        let sc = unsafe { crate::kernel::dot_f16_x4(&pr.q32, core::array::from_fn(|q| get(k + q.min(m - 1)))) };
        for q in 0..m {
            acc.st.full_reads += 1;
            let s = sc[q] as f64;
            let x = get(k + q);
            let inn = if s >= t + e {
                true
            } else if s < t - e {
                false
            } else {
                acc.st.exact_fallbacks += 1;
                exact_ge(exact_dot(&pr.q16, x), t)
            };
            if inn {
                acc.rows.push(pend[k + q]);
                if want_exact {
                    acc.exact.push((pend[k + q], exact_to_f64(exact_dot(&pr.q16, x))));
                }
            }
        }
        k += m;
    }
}

/// decide_full over scattered rows by blocks of 32: each block's vectors are copied into a contiguous buffer (the next
/// block's rows prefetched meanwhile) and scored there.
pub fn decide_full_packed(ix: &Index2, pr: &Prep2, t: f64, rows: &[u32], want_exact: bool, acc: &mut ThrOut2) {
    const B: usize = 32;
    let n = ix.n();
    let e = full_err2(ix, pr);
    if let Some(fr) = ix.own_rows() {
        fr.willneed(rows, n);
    }
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
    /// every visible row scored from its vector, without a cut
    Direct,
    Open { qm: f64, u0: usize, u1: usize, d0: usize, d1: usize, ncand: usize },
}

/// What the planner chose for a threshold query: a scan of the visible set, or per cluster the index path or full
/// vectors after the cut.
pub enum Planned {
    Scan,
    Run { pre: Vec<Pre>, use_idx: Vec<bool>, cut_cyc: u64 },
}

/// The visible rows of [d0, d1) as offsets in cluster `ci`, ascending (from the bit set a word at a time).
fn cands_of(ix: &Index2, ci: usize, d0: usize, d1: usize, vis: Option<&Vis2>) -> Vec<u32> {
    let c = &ix.cl[ci];
    match vis {
        None => ((d0 - c.r0) as u32..(d1 - c.r0) as u32).collect(),
        Some(v) => {
            let mut out = Vec::new();
            v.each(d0, d1, |r| out.push((r - c.r0) as u32));
            out
        }
    }
}

/// A cluster decided by the support table alone, from the certified bounds of the query's anchor: wholly below t
/// (skipped) or wholly above (taken in); None when neither, or under the norm sort, whose cut decides both. A cluster
/// with no visible row is skipped.
fn support_pre(ix: &Index2, pr: &Prep2, t: f64, nvis: usize, ci: usize, nusort: bool) -> Option<Pre> {
    if nvis == 0 {
        return Some(Pre::Skip);
    }
    if nusort {
        return None;
    }
    let k = ix.k();
    let rr = ix.x2.sqrt();
    let Anc { a, beta: b, rho } = pr.anc(ix);
    let (hi, lo) = (ix.hi[a * k + ci] as f64, ix.lo[a * k + ci] as f64);
    let (bh, bl) = if b >= 0.0 { (b * hi, b * lo) } else { (b * lo, b * hi) };
    if bh + rho * rr < t - 1e-9 {
        return Some(Pre::Skip);
    }
    if bl - rho * rr >= t + 1e-9 {
        return Some(Pre::In);
    }
    None
}

/// The cluster's cut and sorted range (under the norm sort, its groups set up first).
fn cut_pre(ix: &Index2, pr: &Prep2, t: f64, vis: Option<&Vis2>, ci: usize, nusort: bool) -> Pre {
    let c = &ix.cl[ci];
    let qm = pr.qm(ix, ci);
    let (u0, u1, d0, d1) = if nusort {
        let nc = NuCut::new(ix, ci, pr, pr.gq[c.g].get().unwrap(), qm);
        let (u0, u1, d0, d1) = nu_range(ix, ci, &nc, t);
        if u0 == c.r1 {
            return Pre::Skip;
        }
        if d0 == c.r1 && u0 == c.r0 {
            return Pre::In;
        }
        (u0, u1, d0, d1)
    } else {
        sorted_range2(ix, ci, &cut_params2(ix, ci, pr, qm), t)
    };
    let ncand = match vis {
        None => d1 - d0,
        Some(v) => count_bits(&v.bits, d0, d1),
    };
    Pre::Open { qm, u0, u1, d0, d1, ncand }
}

/// The planner's choice for a threshold query. Which path decides a row never changes the answer.
///
/// Vectors in memory (reads not priced), under `Auto`, in rows (`Plan::r_rows` R0 and `r_kappa` kappa; `R_ROWS`), with
/// or without a visible set (without one, a cluster's visible rows are all its rows): the query scans the visible rows
/// when they number at most the sum of R0 + kappa * S over the clusters holding one (S a cluster's rows), a test made
/// first, before the anchors and the support table, since it needs neither. Otherwise each cluster is decided by the
/// support table where it can be (wholly out or wholly in); one it leaves open is scored directly from its visible
/// rows when it holds at most R0 + kappa * S of them, else cut to its sorted range and run on the index path. So a
/// cluster scored directly is one whose index path would cost more than its rows; on clusters of more than 78 rows
/// (the default R0 / (1 - kappa)) without a visible set every open cluster takes the index path (measured: within
/// noise of the best choice in every cell, and 1.6-6x faster than a scan; REPORT_BUILD), while an index compacted to a small visible set
/// (`Index2::compact`) is scanned, as its visible set would be.
///
/// Vectors on file whose reads are priced (`disk::ReadPolicy::priced`), under `Auto`: the cost model (`plan2_priced`).
/// `Index` and `Flat` cut every cluster the support table leaves open (the index path, or full vectors); `Scan` scans.
pub fn plan2(ix: &Index2, pr: &Prep2, t: f64, o: &Opts2) -> Planned {
    let pl = o.plan;
    let auto = match pl.mode {
        PlanMode::Scan => return Planned::Scan,
        PlanMode::Auto => true,
        PlanMode::Index | PlanMode::Flat => false,
    };
    // reads are priced only where this index's own row ids address vectors on file whose policy prices them (not a
    // compacted copy in memory, nor a parent's rows reached through vrow)
    if auto && ix.own_rows().is_some_and(|r| r.priced()) {
        return plan2_priced(ix, pr, t, o);
    }
    let k = ix.k();
    let nvis_cl = |ci: usize| o.vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize);
    let live: Vec<usize> = (0..k).filter(|&ci| nvis_cl(ci) > 0).collect();
    // the whole query scans when every cluster holding a visible row could be scored directly (in sum), before the
    // anchors are searched or the support table read
    if auto {
        let (nvis, lim) = rule_rows(ix, o.vis, pl);
        if nvis <= lim {
            return Planned::Scan;
        }
    }
    let tc0 = tick();
    let nusort = !ix.nukey.is_empty();
    if nusort {
        let mut gs: Vec<usize> = live.iter().map(|&ci| ix.cl[ci].g).collect();
        gs.sort_unstable();
        gs.dedup();
        setup_groups(ix, pr, &gs, &mut Stats::new(ix.meta.nl()));
    }
    pr.anc_among(ix, Some(&live));
    let pre: Vec<Pre> = (0..k).into_par_iter().map(|ci| {
        let nv = nvis_cl(ci);
        let direct = auto && nv <= pl.direct_max(ix.cl[ci].r1 - ix.cl[ci].r0);
        support_pre(ix, pr, t, nv, ci, nusort).unwrap_or_else(|| if direct { Pre::Direct } else { cut_pre(ix, pr, t, o.vis, ci, nusort) })
    }).collect();
    let cut_cyc = tick() - tc0;
    let flat = matches!(pl.mode, PlanMode::Flat);
    let use_idx: Vec<bool> = pre.iter().map(|p| matches!(p, Pre::Open { ncand, .. } if *ncand > 0) && !flat).collect();
    Planned::Run { pre, use_idx, cut_cyc }
}

/// The planner with the full vectors on file and their reads priced: a cost model in microseconds, the reads at the
/// timed `Plan::io_us` and `io_gbps`, the compute at the fixed `COSTS`. Each cluster holding a visible row is decided
/// by the support table where it can be; the rest are either scored directly from their visible rows' vectors, when a
/// cut would not pay for itself, or cut to their sorted range and run on the index path or on full vectors; and the
/// whole query scans the visible rows when the model says that is cheaper than all of it.
fn plan2_priced(ix: &Index2, pr: &Prep2, t: f64, o: &Opts2) -> Planned {
    let k = ix.k();
    let pl = o.plan;
    let cm = COSTS;
    let nvis_cl = |ci: usize| o.vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize);
    let live: Vec<usize> = (0..k).filter(|&ci| nvis_cl(ci) > 0).collect();
    let nvis: usize = live.iter().map(|&ci| nvis_cl(ci)).sum();
    // a row scored by a scan, or directly in its cluster: scattered under a visible set, in order without one; and its
    // read, in order
    let rb = 2 * ix.n();
    let row_us = cm.scan_cost_us(o.vis.is_some()) + rb as f64 / (pl.io_gbps * 1e3);
    let scan_cost = nvis as f64 * row_us;
    // a live cluster that the support table leaves open costs at least the cheaper of its cut and scoring its rows
    let floor = |ci: usize| cm.cut_us.min(nvis_cl(ci) as f64 * row_us + cm.dir_us);
    // before the anchors are read: when the scan costs no more than cutting every live cluster, or than the anchor
    // search with each live cluster at the cheaper of its cut and its rows (no cluster decided by the support table)
    if scan_cost <= live.len() as f64 * cm.cut_us || scan_cost <= cm.anc_us + live.iter().map(|&ci| floor(ci)).sum::<f64>() {
        return Planned::Scan;
    }
    let tc0 = tick();
    let nusort = !ix.nukey.is_empty();
    if nusort {
        let mut gs: Vec<usize> = live.iter().map(|&ci| ix.cl[ci].g).collect();
        gs.sort_unstable();
        gs.dedup();
        setup_groups(ix, pr, &gs, &mut Stats::new(ix.meta.nl()));
    }
    let cut = |ci: usize| cut_pre(ix, pr, t, o.vis, ci, nusort);
    pr.anc_among(ix, Some(&live));
    let mut pre: Vec<Option<Pre>> = (0..k).map(|ci| support_pre(ix, pr, t, nvis_cl(ci), ci, nusort)).collect();
    let open: Vec<usize> = live.iter().copied().filter(|&ci| pre[ci].is_none()).collect();
    if scan_cost <= open.iter().map(|&ci| floor(ci)).sum::<f64>() {
        return Planned::Scan;
    }
    // a sample of one open cluster in SAMPLE is cut first: the share d of their visible rows left as candidates
    // stands in for the rest
    const SAMPLE: usize = 8;
    let (mut nc_s, mut nv_s) = (0usize, 0usize);
    let sampled = open.len() >= 8 * SAMPLE;
    if sampled {
        for &ci in open.iter().step_by(SAMPLE) {
            let p = cut(ci);
            nv_s += nvis_cl(ci);
            if let Pre::Open { ncand, .. } = p {
                nc_s += ncand;
            }
            pre[ci] = Some(p);
        }
    }
    let idx_us = |nc: usize, d: f64| cm.index_us(nc, d);
    // a candidate's vector read: scattered over the index under a visible set, like a scan's
    let full_us = if o.vis.is_some() { cm.full_us.max(cm.scan_us) } else { cm.full_us };
    // nc candidates' full vectors, lying in a span of `span` rows, and their reads
    let cand_us = |nc: usize, span: usize| nc as f64 * full_us + pl.disk_us(nc, span, rb);
    // a cluster's candidates at share d lie in about d of its rows
    let span_d = |ci: usize, d: f64| (d * (ix.cl[ci].r1 - ix.cl[ci].r0) as f64).ceil() as usize;
    // a cluster scored directly: when its rows cost no more than its cut and the candidates the cut would leave
    let direct = |ci: usize, d: f64| {
        let nv = nvis_cl(ci) as f64;
        let nc = d * nv;
        nv * row_us + cm.dir_us <= cm.cut_us + cand_us(nc.round() as usize, span_d(ci, d)).min(idx_us(nc.round() as usize, d))
    };
    if sampled {
        let d = nc_s as f64 / nv_s.max(1) as f64;
        // every open cluster at its cheapest path (direct, or the cut and the cheaper of full vectors and the index,
        // with the setup of each group the index path opens), or at its cheapest without the index path
        let mut groups = vec![false; ix.groups.len()];
        let (mut est, mut est_flat) = (0.0, 0.0);
        for &ci in &open {
            let nv = nvis_cl(ci) as f64;
            let nc = d * nv;
            let (dir, full, idx) = (nv * row_us + cm.dir_us, cm.cut_us + cand_us(nc.round() as usize, span_d(ci, d)), cm.cut_us + idx_us(nc.round() as usize, d));
            est_flat += dir.min(full);
            if idx < dir.min(full) {
                est += idx;
                groups[ix.cl[ci].g] = true;
            } else {
                est += dir.min(full);
            }
        }
        let est = est_flat.min(est + groups.iter().filter(|&&g| g).count() as f64 * cm.grp_us);
        if scan_cost <= est {
            return Planned::Scan;
        }
        for &ci in &open {
            if pre[ci].is_none() && direct(ci, d) {
                pre[ci] = Some(Pre::Direct);
            }
        }
    } else {
        // no estimate of d: direct only where the rows cost no more than the cut alone
        for &ci in &open {
            if direct(ci, 0.0) {
                pre[ci] = Some(Pre::Direct);
            }
        }
    }
    let pre: Vec<Pre> = pre.into_par_iter().enumerate().map(|(ci, p)| p.unwrap_or_else(|| cut(ci))).collect();
    let cut_cyc = tick() - tc0;
    // per cluster the index path or full vectors, a group's setup only if its clusters' gains pay for it
    let mut use_idx = vec![false; k];
    let (mut ncand_all, mut nvis_all, mut ndirect) = (0usize, 0usize, 0usize);
    for (ci, p) in pre.iter().enumerate() {
        match p {
            Pre::Open { ncand, .. } => {
                ncand_all += ncand;
                nvis_all += nvis_cl(ci);
            }
            Pre::Direct => ndirect += nvis_cl(ci),
            _ => {}
        }
    }
    let d = ncand_all as f64 / nvis_all.max(1) as f64;
    let mut grp_gain = vec![0f64; ix.groups.len()];
    let mut full_all = 0f64;
    for (ci, p) in pre.iter().enumerate() {
        if let Pre::Open { ncand, d0, d1, .. } = p {
            if *ncand == 0 {
                continue;
            }
            let full = cand_us(*ncand, d1 - d0);
            full_all += full;
            let gain = full - idx_us(*ncand, d);
            use_idx[ci] = gain > 0.0;
            if use_idx[ci] {
                grp_gain[ix.cl[ci].g] += gain;
            }
        }
    }
    for ci in 0..k {
        if use_idx[ci] && grp_gain[ix.cl[ci].g] < cm.grp_us {
            use_idx[ci] = false;
        }
    }
    // what remains costs every candidate's and direct row's full vector less the kept gains; a scan may be cheaper
    let gains: f64 = (0..ix.groups.len()).filter(|&g| grp_gain[g] >= cm.grp_us).map(|g| grp_gain[g] - cm.grp_us).sum();
    let ndc = pre.iter().filter(|p| matches!(p, Pre::Direct)).count();
    if full_all + ndirect as f64 * row_us + ndc as f64 * cm.dir_us - gains > scan_cost {
        return Planned::Scan;
    }
    Planned::Run { pre, use_idx, cut_cyc }
}

pub fn threshold2(ix: &Index2, pr: &Prep2, t: f64, o: &Opts2) -> ThrOut2 {
    match plan2(ix, pr, t, o) {
        Planned::Scan => scan_visible(ix, pr, t, o.vis, o.want_iv, o.plan.pscan),
        Planned::Run { pre, use_idx, cut_cyc } => run_planned(ix, pr, t, o, &pre, &use_idx, cut_cyc),
    }
}

/// A cluster's per-query setup for a threshold query under plan `pl`, on the integer deterministic levels when `int`.
pub fn open_cluster(ix: &Index2, ci: usize, gq: &GQ2, pr: &Prep2, qm: f64, pl: Plan, int: bool) -> CQ2 {
    let gt = pl.gtab;
    let mut cq = cluster_q2w(ix, ci, gq, pr, qm, !gt);
    cq.gtab = gt;
    cq.repack = if pl.repack { pl.repack_mul } else { 0.0 };
    cq.tol = pl.tol;
    if let Some(d) = pl.ddec {
        cq.ddec = d;
    }
    if int {
        cq.enable_int(ix, ci, gq);
    }
    cq
}

/// The threshold query over the clusters as planned.
pub fn run_planned(ix: &Index2, pr: &Prep2, t: f64, o: &Opts2, pre: &[Pre], use_idx: &[bool], cut_cyc: u64) -> ThrOut2 {
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
    let reader = || ix.own_rows().and_then(|rows| super::disk::Reader::<()>::new(rows, ix.n(), pl.io_gap()));
    // with a reader, the rows of a cluster that need full vectors are read while the next clusters run their index path
    let decide_batch = |acc: &mut ThrOut2, rd: &mut super::disk::Reader<()>| {
        let b = rd.pop().unwrap();
        assert!(ix.vrow.is_empty(), "a reader batch holds file rows, which are answer rows only on an index that reads its own vectors (Index2::own_rows)");
        let tf = tick();
        decide_full_from(ix, pr, t, &b.rows, o.want_iv, acc, |k| b.row(k, ix.n()));
        acc.st.cyc[P_FULL] += tick() - tf;
        rd.recycle(b);
    };
    let parts: Vec<ThrOut2> = (0..k).into_par_iter().fold(
        || (ThrOut2 { st: Stats::new(nl), npow: ix.meta.npow, ..Default::default() }, State::default(), reader()),
        |(mut acc, mut sv, mut rd), ci| {
            let tcl = tick();
            let c = &ix.cl[ci];
            // the visible rows of [r0, r1), ascending
            let each = |r0: usize, r1: usize, f: &mut dyn FnMut(usize)| match o.vis {
                None => (r0..r1).for_each(|r| f(r)),
                Some(v) => v.each(r0, r1, f),
            };
            let push = |acc: &mut ThrOut2, r0: usize, r1: usize, iv: Option<(f64, f64)>| {
                if o.vis.is_none() && iv.is_none() {
                    acc.rows.extend(r0 as u32..r1 as u32);
                    return;
                }
                each(r0, r1, &mut |r| {
                    acc.rows.push(r as u32);
                    if let Some(x) = iv {
                        acc.iv.push((r as u32, x.0 as f32, x.1 as f32, 255));
                    }
                });
            };
            let nvis_in = |r0: usize, r1: usize| o.vis.map_or(r1 - r0, |v| count_bits(&v.bits, r0, r1));
            match pre[ci] {
                Pre::Skip => {
                    acc.st.clusters_out += 1;
                }
                Pre::In => {
                    acc.st.clusters_in += 1;
                    let iv = if o.want_iv { Some((-1.0, 1.0)) } else { None };
                    push(&mut acc, c.r0, c.r1, iv);
                    if o.want_iv && !ix.nukey.is_empty() {
                        let qm = pr.qm(ix, ci);
                        let (lb, ub) = NuCut::new(ix, ci, pr, get_gq2(ix, pr, c.g, &mut acc.st), qm).bounds(c.numax);
                        let nn = acc.iv.len();
                        let cnt = nvis_in(c.r0, c.r1);
                        for x in &mut acc.iv[nn - cnt..] {
                            x.1 = lb as f32;
                            x.2 = ub as f32;
                        }
                    } else if o.want_iv {
                        // interval from the table alone
                        let Anc { a, beta: b, rho } = pr.anc(ix);
                        let (hi, lo) = (ix.hi[a * k + ci] as f64, ix.lo[a * k + ci] as f64);
                        let (bh, bl) = if b >= 0.0 { (b * hi, b * lo) } else { (b * lo, b * hi) };
                        let nn = acc.iv.len();
                        let cnt = nvis_in(c.r0, c.r1);
                        for x in &mut acc.iv[nn - cnt..] {
                            x.1 = (bl - rho * rr) as f32;
                            x.2 = (bh + rho * rr) as f32;
                        }
                    }
                }
                Pre::Direct => {
                    acc.st.clusters_open += 1;
                    let pend: Vec<u32> = match o.vis {
                        None => (c.r0 as u32..c.r1 as u32).collect(),
                        Some(v) => v.rows[v.rows.partition_point(|&r| (r as usize) < c.r0)..v.rows.partition_point(|&r| (r as usize) < c.r1)].to_vec(),
                    };
                    acc.flat_clusters += 1;
                    acc.flat_items += pend.len() as u64;
                    acc.st.touched += pend.len() as u64;
                    let tf = tick();
                    decide_full(ix, pr, t, &pend, o.want_iv, &mut acc);
                    acc.st.cyc[P_FULL] += tick() - tf;
                }
                Pre::Open { qm, u0, u1, d0, d1, .. } => {
                    acc.st.clusters_open += 1;
                    acc.st.range_out += ((u0 - c.r0) + (c.r1 - u1)) as u64;
                    acc.st.range_in += ((d0 - u0) + (u1 - d1)) as u64;
                    if o.want_iv {
                        let nc = if ix.nukey.is_empty() { None } else { Some(NuCut::new(ix, ci, pr, get_gq2(ix, pr, c.g, &mut acc.st), qm)) };
                        let cu = if nc.is_none() { Some(cut_params2(ix, ci, pr, qm)) } else { None };
                        let mut take = |r: usize| {
                            acc.rows.push(r as u32);
                            let (lb, ub) = match &nc {
                                Some(nc) => nc.bounds(nc.nu_of(ix.nukey[r])),
                                None => key_interval2(ix, cu.as_ref().unwrap(), r),
                            };
                            acc.iv.push((r as u32, lb as f32, ub as f32, 254));
                        };
                        each(u0, d0, &mut take);
                        each(d1, u1, &mut take);
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
                            let mut cq = open_cluster(ix, ci, gq, pr, qm, pl, pl.int_det);
                            acc.st.cyc[P_CLSETUP] += tick() - tq;
                            let mut ivv = Vec::new();
                            let mut out = RunOut { rows_in: &mut acc.rows, pend: &mut pend, iv: if o.want_iv { Some(&mut ivv) } else { None } };
                            unsafe { run_cluster(ix, ci, pr, gq, &mut cq, &cands, t, Mode2::Thr, o.verify, &mut sv, &mut acc.st, &mut out) };
                            sv.recycle(&mut cq);
                            acc.iv.extend(ivv);
                        } else {
                            acc.flat_clusters += 1;
                            acc.flat_items += cands.len() as u64;
                            pend.extend(cands.iter().map(|&i| (c.r0 + i as usize) as u32));
                        }
                        // rows already in the page cache are decided from the mapping, without reads
                        match rd.as_mut() {
                            Some(r) if !pend.is_empty() && !ix.full_cached(&pend) => {
                                r.push((), pend);
                                while r.full() {
                                    decide_batch(&mut acc, r);
                                }
                            }
                            _ => {
                                let tf = tick();
                                decide_full(ix, pr, t, &pend, o.want_iv, &mut acc);
                                acc.st.cyc[P_FULL] += tick() - tf;
                            }
                        }
                    }
                }
            }
            acc.st.cyc[P_CLUSTER] += tick() - tcl;
            (acc, sv, rd)
        },
    ).map(|(mut acc, _, rd)| {
        if let Some(mut r) = rd {
            while !r.is_empty() {
                decide_batch(&mut acc, &mut r);
            }
        }
        acc
    }).collect();
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
    if rows.len() * 64 >= ix.nent {
        let mut bits = vec![0u64; ix.nent / 64 + 1];
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
    pub t_seed: f64,
    pub t_sweep: f64,
    pub t_sel: f64,
    pub npow: u32, // the index's guarantee N, as 2^npow (Meta2::npow)
}

/// Intervals of rows at levels for the k-NN selection: per call, the rows grouped by cluster and level, each
/// (cluster, level) run once over its rows; each cluster set up once, its tables kept for later calls. At level nl and
/// beyond, a row's exact score (lower = upper).
pub struct Refiner<'a> {
    ix: &'a Index2,
    pr: &'a Prep2,
    cqs: std::collections::HashMap<usize, CQ2>,
    sv: State,
}
impl<'a> Refiner<'a> {
    pub fn new(ix: &'a Index2, pr: &'a Prep2) -> Refiner<'a> {
        Refiner { ix, pr, cqs: Default::default(), sv: State::default() }
    }
    /// The interval of each (row, level) of `req`, in its order.
    pub fn refine(&mut self, req: &[(u32, usize)], st: &mut Stats) -> Vec<(f64, f64)> {
        let (ix, pr) = (self.ix, self.pr);
        let nl = ix.meta.nl();
        let mut out = vec![(0.0, 0.0); req.len()];
        // (cluster, level, row, position in req), so that a (cluster, level) is one run over ascending rows
        let mut by: Vec<(usize, usize, u32, usize)> = Vec::with_capacity(req.len());
        for (i, &(r, lv)) in req.iter().enumerate() {
            if lv >= nl {
                st.full_reads += 1;
                let v = exact_value2(ix, pr, r as usize);
                out[i] = (v, v);
            } else {
                by.push((ix.cl.partition_point(|c| c.r1 <= r as usize), lv, r, i));
            }
        }
        by.sort_unstable();
        let mut iv = Vec::new();
        for g in by.chunk_by(|a, b| (a.0, a.1) == (b.0, b.1)) {
            let (ci, lv) = (g[0].0, g[0].1);
            let c = &ix.cl[ci];
            let gq = get_gq2(ix, pr, c.g, st);
            let cq = self.cqs.entry(ci).or_insert_with(|| cluster_q2(ix, ci, gq, pr, pr.qm(ix, ci)));
            let cands: Vec<u32> = g.iter().map(|x| x.2 - c.r0 as u32).collect();
            let (mut rin, mut pend) = (Vec::new(), Vec::new());
            iv.clear();
            let mut o = RunOut { rows_in: &mut rin, pend: &mut pend, iv: Some(&mut iv) };
            unsafe { run_cluster(ix, ci, pr, gq, cq, &cands, f64::NAN, Mode2::Interval(lv), false, &mut self.sv, st, &mut o) };
            // nothing is decided at a threshold of NaN, so every row's interval comes back, in row order
            assert_eq!(iv.len(), g.len(), "an interval for every row refined");
            for (x, y) in g.iter().zip(&iv) {
                debug_assert_eq!(x.2, y.0);
                out[x.3] = (y.1 as f64, y.2 as f64);
            }
        }
        out
    }
}

/// Clusters in descending order of q.m^ (their centre direction).
pub fn by_centre(ix: &Index2, pr: &Prep2, clusters: Vec<usize>) -> Vec<usize> {
    let mut byc: Vec<(f64, usize)> = clusters.into_iter().map(|ci| (pr.qm(ix, ci) / ix.cl[ci].mn, ci)).collect();
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
        let qm = pr.qm(ix, ci);
        let mut cq = cluster_q2(ix, ci, gq, pr, qm);
        let (mut rin, mut pend) = (Vec::new(), Vec::new());
        let mut out = RunOut { rows_in: &mut rin, pend: &mut pend, iv: Some(&mut iv) };
        unsafe { run_cluster(ix, ci, pr, gq, &mut cq, &cands, f64::NAN, Mode2::Interval(level), false, &mut sv, st, &mut out) };
        if nvis >= budget {
            break;
        }
    }
    iv.sort_by(|x, y| ((y.1 + y.2) as f64).partial_cmp(&((x.1 + x.2) as f64)).unwrap());
    iv
}

/// The approximate mode's cluster order: clusters by descending q.m + b(q) rad, the centre score plus a bonus for the
/// cluster's RMS radius about its centre (an optimistic estimate of its best item's score).
/// `Radius`: b(q) = f, fitted on corpus pseudo-queries. `Spread`: b(q) = f sd(q) / sd(rad), sd(q) the standard
/// deviation of q.m over the clusters; it does not change when the query is scaled or moved along a direction in which
/// the centres do not vary, as out-of-distribution queries are (bench/suite/notes/visit-order).
/// f of the `Spread` order when no queries are given to fit it on: chosen by leave-one-dataset-out over five datasets,
/// in and out of distribution (bench/suite/notes/visit-order, `loo_summary.txt`). Corpus pseudo-queries are not used to
/// fit it, since they are in distribution by construction.
pub const SPREAD_F: f64 = 0.4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OrderKind {
    Radius,
    Spread,
}

impl OrderKind {
    pub fn parse(s: &str) -> Result<OrderKind, String> {
        match s {
            "radius" => Ok(OrderKind::Radius),
            "spread" => Ok(OrderKind::Spread),
            _ => Err(format!("the order is radius or spread, not {s:?}")),
        }
    }
    /// The values `Approx::fit` chooses f from.
    fn grid(self) -> &'static [f64] {
        match self {
            OrderKind::Radius => &[0.0, 0.1, 0.2, 0.35, 0.5, 0.75],
            OrderKind::Spread => &[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.8],
        }
    }
}

#[derive(Clone)]
pub struct Approx {
    pub kind: OrderKind,
    pub f: f64,
    pub rad: Vec<f64>,
    /// The standard deviation of rad over the clusters.
    pub sd_rad: f64,
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
    /// Every cluster's RMS radius about its stored centre, from the stored vectors.
    pub fn radii(ix: &Index2) -> Vec<f64> {
        let n = ix.n();
        (0..ix.k()).into_par_iter().map(|ci| {
            let c = &ix.cl[ci];
            let m = ix.centre_f64(ci);
            let mut x = vec![0f64; n];
            let mut s = 0.0;
            for r in c.r0..c.r1 {
                ix.full_row_f64(r, &mut x);
                s += x.iter().zip(&m).map(|(a, b)| (a - b) * (a - b)).sum::<f64>();
            }
            (s / (c.r1 - c.r0).max(1) as f64).sqrt()
        }).collect()
    }

    pub fn new(kind: OrderKind, f: f64, rad: Vec<f64>) -> Approx {
        let k = rad.len().max(1) as f64;
        let mean = rad.iter().sum::<f64>() / k;
        let sd_rad = (rad.iter().map(|r| (r - mean) * (r - mean)).sum::<f64>() / k).sqrt();
        Approx { kind, f, rad, sd_rad }
    }

    /// The order with no radius bonus: by centre score alone.
    pub fn centres(ix: &Index2) -> Approx {
        Approx::new(OrderKind::Radius, 0.0, vec![0.0; ix.k()])
    }

    /// The bonus per unit of radius for a query with these centre scores (q.m of every cluster).
    pub fn bonus(&self, qm: &[f64]) -> f64 {
        match self.kind {
            OrderKind::Radius => self.f,
            OrderKind::Spread => {
                if self.sd_rad <= 0.0 || qm.is_empty() {
                    return 0.0;
                }
                let k = qm.len() as f64;
                let mean = qm.iter().sum::<f64>() / k;
                let sd = (qm.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / k).sqrt();
                self.f * sd / self.sd_rad
            }
        }
    }

    /// The first `take` clusters of the order (all when `take` is 0), from q.m of every cluster: only those are
    /// sorted (by descending key, the lower index first among equal keys).
    pub fn order_qm(&self, ix: &Index2, qm: &[f64], vis: Option<&Vis2>, take: usize) -> Vec<usize> {
        let b = self.bonus(qm);
        let mut keys: Vec<(f64, usize)> = (0..qm.len()).filter(|&ci| vis.map_or(ix.cl[ci].r1 > ix.cl[ci].r0, |v| v.cl_cnt[ci] > 0))
            .map(|ci| (qm[ci] + b * self.rad[ci], ci)).collect();
        let by = |x: &(f64, usize), y: &(f64, usize)| y.0.partial_cmp(&x.0).unwrap().then(x.1.cmp(&y.1));
        if take > 0 && take < keys.len() {
            keys.select_nth_unstable_by(take - 1, by);
            keys.truncate(take);
        }
        keys.sort_unstable_by(by);
        keys.into_iter().map(|x| x.1).collect()
    }
    /// Clusters holding a visible item, in order.
    pub fn order(&self, ix: &Index2, q64: &[f64], vis: Option<&Vis2>) -> Vec<usize> {
        let qm: Vec<f64> = (0..ix.k()).map(|ci| ix.qm(q64, ci)).collect();
        self.order_qm(ix, &qm, vis, 0)
    }

    /// f for `kind` from its grid (`OrderKind::grid`): the one that reaches mean recall@10 0.95 on the queries (exact
    /// scan of the probed clusters, same number of clusters for every query) with the fewest items scanned; radii from
    /// the stored vectors. Also returns the mean items scanned at each f.
    pub fn fit(ix: &Index2, pseudo: &[(usize, Vec<f32>, Vec<u32>)], kind: OrderKind) -> (Approx, Vec<(f64, f64)>) {
        let rad = Approx::radii(ix);
        let k = ix.k();
        let row_cl: Vec<u32> = ix.cl.iter().enumerate().flat_map(|(ci, c)| std::iter::repeat(ci as u32).take(c.r1 - c.r0)).collect();
        let mut res = Vec::new();
        for &f in kind.grid() {
            let ap = Approx::new(kind, f, rad.clone());
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
                    found[rank[row_cl[ix.row_of_ent(e)] as usize] + 1] += 1.0 / gt.len() as f64;
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
        (Approx::new(kind, best.0, rad), res)
    }
}

/// The confidence dial, fitted on corpus pseudo-queries. It cuts only the upper side of each item interval, the side
/// that prunes; lower bounds stay certified. Each level's factor for a miss rate eps is the (1 - eps) quantile of
/// (true - estimate) / half-width over the intervals that level computed, in a verified threshold query (index path),
/// for the items scoring at least the threshold, clipped to [0, 1]; eps = 0 gives 1 everywhere, the exact mode.
/// A pseudo-query whose row is past the index is a held-out query, not a stored row.
/// The fit is for knn2_fast at k = kk: the threshold is the pseudo-query's max(kk, 10)-th best score, every level gets
/// a factor, and with `recall_too` each eps's recall@kk is that of knn2_fast over every cluster.
pub struct Dial {
    pub eps: Vec<f64>,
    pub factors: Vec<Vec<f32>>, // per eps, per level
    pub recall: Vec<f64>,       // per eps, on the pseudo-queries
}
impl Dial {
    pub fn for_eps(&self, e: f64) -> &[f32] {
        let i = self.eps.iter().position(|&x| (x - e).abs() <= 1e-12 * e.max(1e-12)).expect("no dial fitted for that eps");
        &self.factors[i]
    }
    pub fn fit(ix: &Index2, pseudo: &[(usize, Vec<f32>, Vec<u32>)], plan: Plan, recall_too: bool, kk: usize, ap: &Approx) -> Dial {
        let nl = ix.meta.nl();
        let pidx = Plan { mode: PlanMode::Index, ..plan };
        let kthr = kk.max(10);
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
                by_level[e.0 as usize].push(e.1);
            }
        }
        for v in &mut by_level {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        }
        let eps = vec![0.0, 1e-5, 1e-4, 1e-3, 3e-3, 1e-2, 3e-2, 0.1, 0.2];
        let factors: Vec<Vec<f32>> = eps.iter().map(|&e| by_level.iter().map(|v| {
            if e == 0.0 || v.is_empty() { 1.0 } else { v[(((1.0 - e) * v.len() as f64) as usize).min(v.len() - 1)].clamp(0.0, 1.0) }
        }).collect()).collect();
        // recall over every cluster, per eps, when asked
        let mut recall = Vec::new();
        for f in factors.iter().filter(|_| recall_too) {
            let rec: f64 = pseudo.par_iter().zip(&per).map(|((row, q, _), (_, gt))| {
                let me = ix.row_ent.get(*row).copied().unwrap_or(u32::MAX);
                let out = knn2_fast(ix, ap, q, kk + 1, None, ix.k(), Some(f), plan);
                let got: Vec<u32> = out.ents.iter().map(|x| x.1).filter(|&e| e != me).take(kk).collect();
                got.iter().filter(|e| gt.contains(e)).count() as f64 / gt.len() as f64
            }).sum::<f64>() / pseudo.len() as f64;
            recall.push(rec);
        }
        Dial { eps, factors, recall }
    }
}

/// The level the seed reads: the deepest that ends within 16 bytes an item, else the first.
pub fn seed_level(ix: &Index2) -> usize {
    (0..ix.meta.nl()).rev().find(|&l| ix.meta.level_bytes(l) <= 16).unwrap_or(0)
}

/// A first k-th best score: the visible items of the 64 clusters with the largest bound, taken by centre score until
/// 2,000 are read at the seed's level (`seed_level`), and the best 2k + 8 of those by interval midpoint scored exactly.
pub fn seed2(ix: &Index2, pr: &Prep2, kk: usize, vis: Option<&Vis2>, st: &mut Stats) -> f64 {
    let k = ix.k();
    let rr = ix.x2.sqrt();
    let live: Vec<usize> = (0..k).filter(|&ci| vis.map_or(ix.cl[ci].r1 > ix.cl[ci].r0, |v| v.cl_cnt[ci] > 0)).collect();
    let mut cand: Vec<(f64, usize)> = if ix.nukey.is_empty() {
        let Anc { a, beta, rho } = pr.anc(ix);
        live.iter().map(|&ci| {
            let (hi, lo) = (ix.hi[a * k + ci] as f64, ix.lo[a * k + ci] as f64);
            let bh = if beta >= 0.0 { beta * hi } else { beta * lo };
            (bh + rho * rr, ci)
        }).collect()
    } else {
        let mut gs: Vec<usize> = live.iter().map(|&ci| ix.cl[ci].g).collect();
        gs.sort_unstable();
        gs.dedup();
        setup_groups(ix, pr, &gs, st);
        live.iter().map(|&ci| {
            let qm = pr.qm(ix, ci);
            (NuCut::new(ix, ci, pr, pr.gq[ix.cl[ci].g].get().unwrap(), qm).bounds(ix.cl[ci].numax).1, ci)
        }).collect()
    };
    cand.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
    cand.truncate(64);
    let clusters: Vec<usize> = cand.iter().map(|&(_, ci)| ci).collect();
    let seed_iv = probe(ix, pr, by_centre(ix, pr, clusters), vis, seed_level(ix), 2000, st);
    let take = (2 * kk + 8).min(seed_iv.len());
    let mut ex: Vec<f64> = seed_iv[..take].iter().map(|x| exact_value2(ix, pr, x.0 as usize)).collect();
    st.full_reads += take as u64;
    ex.sort_by(|a, b| b.partial_cmp(a).unwrap());
    if ex.len() >= kk { ex[kk - 1] } else { f64::NEG_INFINITY }
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
/// the index path's per-cluster setup, by the fixed `COSTS` (at most about 129 visible items), has every visible item
/// scored, batched with other such clusters in row order. That cut-off was about 240 under the timed constants this
/// replaced, so clusters of 130-240 visible items now take the index path, where `dial` applies: approximate results
/// with a dial can differ from before (without one they are exact up to the f32 products either way). No dial and every
/// cluster is exact up to the f32 products.
pub fn knn2_fast(ix: &Index2, ap: &Approx, q: &[f32], kk: usize, vis: Option<&Vis2>, nprobe: usize, dial: Option<&[f32]>, plan: Plan) -> KnnOut2 {
    assert!(kk > 0, "knn2_fast needs k of at least 1");
    let tm = Timer::new();
    let nl = ix.meta.nl();
    let t0 = tick();
    // q.m of every cluster, for the order, the anchor and each probed cluster
    let mut pr = prep2(ix, q);
    if let Some(d) = dial {
        pr.dial = d.to_vec();
    }
    let qm = pr.qms(ix, None);
    let mut st = Stats::new(nl);
    let order = ap.order_qm(ix, &qm, vis, nprobe);
    st.cyc[P_PLAN] += tick() - t0;
    // the groups of the probed clusters with too many visible rows to be scored directly, set up together: each
    // rotation is read once for all of them, not once per group
    let mut gs: Vec<usize> = order.iter().filter(|&&ci| {
        let nv = vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize);
        nv as f64 * COSTS.full_us > COSTS.index_us(nv, 1.0)
    }).map(|&ci| ix.cl[ci].g).collect();
    gs.sort_unstable();
    gs.dedup();
    setup_groups(ix, &pr, &gs, &mut st);
    let mut top = TopK { kk, h: std::collections::BinaryHeap::with_capacity(kk + 1) };
    let mut sv = State::default();
    let (mut rows_in, mut pend) = (Vec::new(), Vec::new());
    let stored = matches!(ix.full, Full2::Real { .. });
    assert!(stored, "knn2_fast scores from stored vectors");
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
        if (top.full() && nv as f64 * COSTS.full_us <= COSTS.index_us(nv, 1.0)) || (!top.full() && top.h.len() + flat.len() + nv <= kk) {
            match vis {
                None => flat.extend(c.r0 as u32..c.r1 as u32),
                Some(_) => flat.extend_from_slice(vis_rows(c.r0, c.r1)),
            }
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
            match vis {
                None => pend.extend((u0 as u32..d0 as u32).chain(d1 as u32..u1 as u32)),
                Some(v) => {
                    v.each(u0, d0, |r| pend.push(r as u32));
                    v.each(d1, u1, |r| pend.push(r as u32));
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
            if top.full() && cands.len() as f64 * COSTS.full_us > COSTS.index_us(cands.len(), d) {
                st.clusters_open += 1;
                let gq = get_gq2(ix, &pr, c.g, &mut st);
                let tq = tick();
                let mut cq = open_cluster(ix, ci, gq, &pr, qm, plan, plan.int_det);
                st.cyc[P_CLSETUP] += tick() - tq;
                let mut out = RunOut { rows_in: &mut rows_in, pend: &mut pend, iv: None };
                unsafe { run_cluster(ix, ci, &pr, gq, &mut cq, &cands, t, Mode2::Thr, false, &mut sv, &mut st, &mut out) };
                sv.recycle(&mut cq);
            } else {
                pend.extend(cands.iter().map(|&i| (c.r0 + i as usize) as u32));
            }
        }
        let tf = tick();
        top.score(ix, &pr, &rows_in, &mut st);
        top.score(ix, &pr, &pend, &mut st);
        st.cyc[P_FULL] += tick() - tf;
    }
    flush(&mut flat, &mut top, &mut st);
    st.cyc[P_QUERY] += tick() - t0;
    let mut ents: Vec<(f64, u32)> = top.h.into_iter().map(|x| (x.0 .0, x.0 .1)).collect();
    ents.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(&b.1)));
    KnnOut2 { ents, st, t_seed: 0.0, t_sweep: tm.ms(), t_sel: 0.0, npow: ix.meta.npow }
}

/// Exact k-NN. The planner may send the query to a scan of the visible set (every row without one) before anything
/// else (at most the rule's rows of the clusters holding one, plus the seed's `r_seed`: `Plan::scan_first`), or after
/// the seed (`plan2`'s rule at the seed's k-th score); otherwise the threshold query at the seed's k-th score runs as
/// planned and the selection refines the survivors' intervals until the k best are certain. Unfiltered on clusters of
/// more than 78 rows (by default) it does not scan; an index compacted to a small visible set does.
pub fn knn2(ix: &Index2, q: &[f32], kk: usize, vis: Option<&Vis2>, plan: Plan) -> KnnOut2 {
    let tm = Timer::new();
    let nl = ix.meta.nl();
    let stored = matches!(ix.full, Full2::Real { .. });
    let scan = |st: Stats, t_seed: f64| {
        let (q16, q32, qn) = query_f16(q);
        let ents = scan_knn(ix, &q16, &q32, qn, kk, vis);
        let mut st = st;
        st.full_reads += vis.map_or(ix.nrows, |v| v.rows.len()) as u64;
        KnnOut2 { ents, st, t_seed, t_sweep: tm.ms() - t_seed, t_sel: 0.0, npow: ix.meta.npow }
    };
    // the rule in rows before the seed: always under a visible set (as before the rule covered unfiltered queries), and
    // without one only in memory, where the cost model does not decide
    if stored && (vis.is_some() || !ix.own_rows().is_some_and(|r| r.priced())) && plan.scan_first(ix, vis) {
        return scan(Stats::new(nl), 0.0);
    }
    let pr = prep2(ix, q);
    let mut st = Stats::new(nl);
    let tau1 = seed2(ix, &pr, kk, vis, &mut st);
    let t_seed = tm.ms();
    let o = Opts2 { vis, verify: false, want_iv: true, plan };
    let sw = match plan2(ix, &pr, tau1, &o) {
        Planned::Scan if stored => return scan(st, t_seed),
        Planned::Scan => scan_visible(ix, &pr, tau1, vis, true, plan.pscan),
        Planned::Run { pre, use_idx, cut_cyc } => run_planned(ix, &pr, tau1, &o, &pre, &use_idx, cut_cyc),
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
    let mut rf = Refiner::new(ix, &pr);
    let mut fin: Vec<(f64, u32)> = select_knn(cands, kk, nl, |req| rf.refine(req, &mut st), |r| exact_value2(ix, &pr, r as usize))
        .into_iter().map(|(v, r)| (v, ix.row_ent[r as usize])).collect();
    fin.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(&b.1)));
    fin.truncate(kk);
    let t_sel = tm.ms() - t_seed - t_sweep;
    KnnOut2 { ents: fin, st, t_seed, t_sweep, t_sel, npow: ix.meta.npow }
}

/// The k best of `cands` (row, lower bound, upper bound, next level, exact), refining intervals a level at a time:
/// `refine` takes the (row, level) pairs of a round and gives each row's interval at that level, its exact score
/// (lower = upper) at level `nl`. A candidate
/// is dropped when at least k other candidates have a lower bound above its upper bound, and certified when at most
/// k - 1 others have an upper bound at or above its lower bound; an exact score replaces the interval. Intervals are
/// intersected across levels. Returns the survivors with their exact scores, or +inf where certified without one; the
/// caller keeps the k largest. The answer is exact when every upper bound of the true k best holds and every lower bound of every
/// other candidate holds, whatever the other sides do; the candidates must contain the k best.
pub fn select_knn(mut cands: Vec<(u32, f64, f64, usize, bool)>, kk: usize, nl: usize, mut refine: impl FnMut(&[(u32, usize)]) -> Vec<(f64, f64)>, mut exact: impl FnMut(u32) -> f64) -> Vec<(f64, u32)> {
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
        let todo: Vec<usize> = (0..cands.len()).filter(|&i| !cands[i].4 && !certified(&cands[i], &ubs)).collect();
        if todo.is_empty() {
            break;
        }
        let got = refine(&todo.iter().map(|&i| (cands[i].0, cands[i].3)).collect::<Vec<_>>());
        for (&i, (l, h)) in todo.iter().zip(got) {
            let c = &mut cands[i];
            *c = if c.3 >= nl { (c.0, l, h, c.3 + 1, true) } else { (c.0, l.max(c.1), h.min(c.2), c.3 + 1, false) };
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
            let refine = |req: &[(u32, usize)]| req.iter().map(|&(r, lv)| if lv >= l { (s[r as usize], s[r as usize]) } else { iv[r as usize][lv] }).collect();
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
        pr.qm(ix, c) + dot64(&gq.qv, &z)
    }
    pub fn x_f64(&self, ix: &Index2, row: usize, out: &mut [f64]) {
        let c = self.row_cl[row] as usize;
        let n = self.n;
        let mut z = vec![0f64; n];
        self.gen(c, self.ext[row], &mut z);
        let v = &self.vq[self.grp[c]];
        let m = ix.centre_f64(c);
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
    fn size(&self, c: usize, _p: &Params) -> usize {
        self.sy.sizes[c]
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
        Items2 { ah: key.iter().map(|k| k.0).collect(), z: zs, ent: key.iter().map(|k| k.1).collect(), ext: key.iter().map(|k| k.1).collect(), x2max, nu: Vec::new(), x16: Vec::new() }
    }
}
