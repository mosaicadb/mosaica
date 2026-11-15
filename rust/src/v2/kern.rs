//! Fast-scan pieces: per-(query, cluster, level) 16-entry tables quantised to u8 (or two u8 halves of a u16) with a
//! bound on the quantisation error, re-packing of survivors into dense 32-item blocks (masked byte permutes over the
//! transposed nibble rows), and the shuffle-and-add accumulation of a part of a level over one block.
//!
//! Block layout: 32 items; a row is 16 bytes, byte p holds item p in the low nibble and item 16 + p in the high nibble.
//! A level of a cluster stores, per block, its code rows (one per 4-bit unit) contiguously.
use super::*;
use crate::simd::*;

pub const U32: f64 = 1.0 / 16777216.0;

#[derive(Clone, Debug, Default)]
pub struct PartLut {
    pub j: Option<usize>,
    pub g0: usize,     // first group (of 4 units) of this part within the level
    pub groups: usize,
    pub off: f32,      // sum of the units' minima
    pub scale: f32,
    pub err: f32,      // bound on |exact part value - (off + scale * S)|
}

#[derive(Default)]
pub struct LevelLut {
    pub parts: Vec<PartLut>,
    pub hi: Vec<u8>, // per group 64 bytes: lane j = unit 4g + j, 16 entries
    pub lo: Vec<u8>, // low bytes of the 16-bit tables
    pub qsum: f64,   // with a common step: a bound on the sum over units of the largest entry
}

/// Build the level's tables from the cluster's combined weights w (det by coordinate, rand by n + position).
#[target_feature(enable = "avx2,fma")]
/// With `step`, every part uses that table step (the integer deterministic levels).
pub unsafe fn build_lut(w: &[f32], lp: &LevelPlan, et: &ExpTab, step: Option<f32>, out: &mut LevelLut) {
    let qmax: f32 = 65535.0;
    out.qsum = 0.0;
    out.parts.clear();
    out.hi.clear();
    out.lo.clear();
    out.hi.resize(lp.units * 16, 0);
    out.lo.resize(lp.units * 16, 0);
    let mut g0 = 0;
    let mut tabs: Vec<__m512> = Vec::new();
    for part in &lp.parts {
        let nu = part.units();
        tabs.clear();
        let mut rmax = 0f32;
        let mut sabs = 0f64;
        let mut off = 0f64;
        for fs in &part.ufields {
            // fields occupy disjoint bits, so the unit's extremes are the sums of the fields' extremes
            let mut t = _mm512_setzero_ps();
            let (mut sa, mut lo, mut hi) = (0f32, 0f32, 0f32);
            for &fi in fs {
                if fi == u16::MAX {
                    break;
                }
                let f = part.fields.get_unchecked(fi as usize);
                let w = *w.get_unchecked(f.widx as usize);
                let e = f.eid as usize;
                t = _mm512_fmadd_ps(_mm512_set1_ps(w), _mm512_loadu_ps(et.e.get_unchecked(e).as_ptr()), t);
                let (el, eh) = (*et.lo.get_unchecked(e), *et.hi.get_unchecked(e));
                let (a, b) = (w * el, w * eh);
                lo += a.min(b);
                hi += a.max(b);
                sa += w.abs() * eh.abs().max(el.abs());
            }
            // the computed minimum may differ from the f32 table's by rounding: covered by the f32 error term
            rmax = rmax.max(hi - lo);
            if let Some(st) = step {
                out.qsum += ((hi - lo) as f64 / st as f64 * (1.0 + 1e-5)).ceil() + 1.0;
            }
            sabs += sa as f64;
            off += lo as f64;
            tabs.push(_mm512_sub_ps(t, _mm512_set1_ps(lo)));
        }
        let scale = match step {
            Some(st) => st,
            None if rmax > 0.0 => rmax / qmax,
            None => 1e-30,
        };
        let inv = _mm512_set1_ps(1.0 / scale);
        for (u, t) in tabs.iter().enumerate() {
            let q = _mm512_min_epi32(_mm512_max_epi32(_mm512_cvtps_epi32(_mm512_mul_ps(*t, inv)), _mm512_setzero_si512()), _mm512_set1_epi32(qmax as i32));
            let gl = (g0 + u / 4) * 64 + (u % 4) * 16;
            _mm_storeu_si128(out.hi.as_mut_ptr().add(gl) as *mut _, _mm512_cvtepi32_epi8(_mm512_srli_epi32(q, 8)));
            _mm_storeu_si128(out.lo.as_mut_ptr().add(gl) as *mut _, _mm512_cvtepi32_epi8(_mm512_and_si512(q, _mm512_set1_epi32(255))));
        }
        // rounding to the grid (half a step per unit), the f32 error of weights, products, sums, the shift by the
        // minimum and the product with 1/scale (all relative to sum |w| max|exp| per unit), and of off + scale * S
        let err = nu as f64 * scale as f64 * 0.5 * (1.0 + 1e-6) + (48.0 + 2.0 * nu as f64) * U32 * sabs + 1e-30;
        out.parts.push(PartLut { j: part.j, g0, groups: nu / 4, off: off as f32, scale, err: err as f32 });
        g0 += nu / 4;
    }
}

/// Where a dest block's rows come from: the distinct source blocks (at most 32) with, for each, the dest bytes whose
/// low-nibble and high-nibble items it supplies; and for each dest byte the source byte of those items.
#[cfg(target_feature = "avx512f")]
pub struct Gather {
    pub ident: Option<usize>, // the whole block is source block s in order
    pub nsrc: usize,
    pub src: [u32; 32],
    pub mlo: [u64; 32],
    pub mhi: [u64; 32],
    pub idxlo: __m512i,
    pub idxhi: __m512i,
    pub shlo: u64,
    pub shhi: u64,
}

#[cfg(target_feature = "avx512f")]
#[inline(always)]
fn rep16(m: u16) -> u64 {
    let m = m as u64;
    m | m << 16 | m << 32 | m << 48
}

/// Plan the gather of 32 survivors (local item indices, ascending; u32::MAX for empty lanes).
#[cfg(target_feature = "avx512f")]
#[target_feature(enable = "avx512f,avx512bw,avx512dq,avx512vl,avx512vbmi")]
pub unsafe fn plan_gather(lanes: &[u32; 32], g: &mut Gather) {
    let l0 = _mm512_loadu_si512(lanes.as_ptr() as *const _);
    let l1 = _mm512_loadu_si512(lanes.as_ptr().add(16) as *const _);
    let first = lanes[0];
    if first != u32::MAX && first % 32 == 0 {
        let iota0 = _mm512_add_epi32(_mm512_set1_epi32(first as i32), _mm512_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15));
        let iota1 = _mm512_add_epi32(iota0, _mm512_set1_epi32(16));
        if _mm512_cmpeq_epi32_mask(l0, iota0) == 0xffff && _mm512_cmpeq_epi32_mask(l1, iota1) == 0xffff {
            g.ident = Some(first as usize / 32);
            return;
        }
    }
    g.ident = None;
    let none = _mm512_set1_epi32(-1);
    let v0 = !_mm512_cmpeq_epi32_mask(l0, none);
    let v1 = !_mm512_cmpeq_epi32_mask(l1, none);
    let b0 = _mm512_srli_epi32(l0, 5);
    let b1 = _mm512_srli_epi32(l1, 5);
    // survivors ascend, so the source blocks are runs: peel them off in order
    let mut left: u32 = v0 as u32 | (v1 as u32) << 16;
    let mut ns = 0;
    while left != 0 {
        let d = left.trailing_zeros() as usize;
        let b = lanes[d] >> 5;
        let bv = _mm512_set1_epi32(b as i32);
        let m0 = _mm512_cmpeq_epi32_mask(b0, bv) & v0;
        let m1 = _mm512_cmpeq_epi32_mask(b1, bv) & v1;
        g.src[ns] = b;
        g.mlo[ns] = rep16(m0);
        g.mhi[ns] = rep16(m1);
        ns += 1;
        left &= !(m0 as u32 | (m1 as u32) << 16);
    }
    g.nsrc = ns;
    let f = _mm512_set1_epi32(15);
    let s16 = _mm512_set1_epi32(16);
    let ilo = _mm512_cvtepi32_epi8(_mm512_and_si512(l0, f));
    let ihi = _mm512_cvtepi32_epi8(_mm512_and_si512(l1, f));
    let shlo = _mm512_test_epi32_mask(l0, s16) & v0;
    let shhi = !_mm512_test_epi32_mask(l1, s16) & v1;
    let lane_off = _mm512_set_epi8(48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    g.idxlo = _mm512_add_epi8(_mm512_broadcast_i32x4(ilo), lane_off);
    g.idxhi = _mm512_add_epi8(_mm512_broadcast_i32x4(ihi), lane_off);
    g.shlo = rep16(shlo);
    g.shhi = rep16(shhi);
}

/// Assemble `rows` rows of a dest block from the source blocks at `base` (row stride `rows` x 16 per block) into dst.
#[cfg(target_feature = "avx512f")]
#[target_feature(enable = "avx512f,avx512bw,avx512dq,avx512vl,avx512vbmi")]
pub unsafe fn gather_rows(g: &Gather, base: *const u8, rows: usize, dst: *mut u8) {
    let stride = rows * 16;
    let m0f = _mm512_set1_epi8(0x0f);
    let mf0 = _mm512_set1_epi8(0xf0u8 as i8);
    let ng = (rows + 3) / 4;
    for gi in 0..ng {
        let mut t1 = _mm512_setzero_si512();
        let mut t2 = _mm512_setzero_si512();
        for s in 0..g.nsrc {
            let (ml, mh) = (*g.mlo.get_unchecked(s), *g.mhi.get_unchecked(s));
            if ml | mh == 0 {
                continue;
            }
            let v = _mm512_loadu_si512(base.add(*g.src.get_unchecked(s) as usize * stride + gi * 64) as *const _);
            t1 = _mm512_mask_permutexvar_epi8(t1, ml, g.idxlo, v);
            t2 = _mm512_mask_permutexvar_epi8(t2, mh, g.idxhi, v);
        }
        let lo = _mm512_and_si512(_mm512_mask_blend_epi8(g.shlo, t1, _mm512_srli_epi16(t1, 4)), m0f);
        let hi = _mm512_and_si512(_mm512_mask_blend_epi8(g.shhi, t2, _mm512_slli_epi16(t2, 4)), mf0);
        _mm512_storeu_si512(dst.add(gi * 64) as *mut _, _mm512_or_si512(lo, hi));
    }
}

#[cfg(target_feature = "avx512f")]
#[inline(always)]
unsafe fn lane_sum(e: __m512i, o: __m512i) -> (__m512, __m512) {
    // e: words = sums of even bytes per 128-bit lane, o: odd; sum the four lanes and interleave to item order
    let f = |x: __m512i| {
        let x = _mm512_add_epi16(x, _mm512_shuffle_i64x2(x, x, 0x4e));
        _mm512_castsi512_si128(_mm512_add_epi16(x, _mm512_shuffle_i64x2(x, x, 0xb1)))
    };
    let (e, o) = (f(e), f(o));
    let a = _mm_unpacklo_epi16(e, o);
    let b = _mm_unpackhi_epi16(e, o);
    let v = _mm256_set_m128i(b, a);
    let w = _mm512_cvtepu16_epi32(v);
    (_mm512_cvtepi32_ps(w), _mm512_setzero_ps())
}

/// Integer table sums of one part over one block: S for items 0..15 and 16..31.
#[cfg(target_feature = "avx512f")]
#[target_feature(enable = "avx512f,avx512bw,avx512dq,avx512vl,avx512vbmi")]
pub unsafe fn part_sums(codes: *const u8, tab: *const u8, groups: usize) -> (__m512, __m512) {
    let m0f = _mm512_set1_epi8(0x0f);
    let (mut a0, mut b0, mut a1, mut b1) = (_mm512_setzero_si512(), _mm512_setzero_si512(), _mm512_setzero_si512(), _mm512_setzero_si512());
    for g in 0..groups {
        let c = _mm512_loadu_si512(codes.add(g * 64) as *const _);
        let t = _mm512_loadu_si512(tab.add(g * 64) as *const _);
        let lo = _mm512_and_si512(c, m0f);
        let hi = _mm512_and_si512(_mm512_srli_epi16(c, 4), m0f);
        let r0 = _mm512_shuffle_epi8(t, lo);
        let r1 = _mm512_shuffle_epi8(t, hi);
        a0 = _mm512_add_epi16(a0, r0);
        b0 = _mm512_add_epi16(b0, _mm512_srli_epi16(r0, 8));
        a1 = _mm512_add_epi16(a1, r1);
        b1 = _mm512_add_epi16(b1, _mm512_srli_epi16(r1, 8));
    }
    let (s0, _) = lane_sum(_mm512_sub_epi16(a0, _mm512_slli_epi16(b0, 8)), b0);
    let (s1, _) = lane_sum(_mm512_sub_epi16(a1, _mm512_slli_epi16(b1, 8)), b1);
    (s0, s1)
}

#[cfg(target_feature = "avx512f")]
impl Gather {
    pub fn new() -> Gather {
        let z = unsafe { _mm512_setzero_si512() };
        Gather { ident: None, nsrc: 0, src: [0; 32], mlo: [0; 32], mhi: [0; 32], idxlo: z, idxhi: z, shlo: 0, shhi: 0 }
    }
}

/// Where a dest block's rows come from: the distinct source blocks (at most 32) with, for each, byte masks of the dest
/// bytes whose low-nibble and high-nibble items it supplies; and for each dest byte the source byte of those items and
/// whether each sits in the other nibble of its source byte.
#[cfg(not(target_feature = "avx512f"))]
pub struct Gather {
    pub ident: Option<usize>, // the whole block is source block s in order
    pub nsrc: usize,
    pub src: [u32; 32],
    pub mlo: [[u8; 16]; 32],
    pub mhi: [[u8; 16]; 32],
    pub idxlo: [u8; 16],
    pub idxhi: [u8; 16],
    pub shlo: [u8; 16], // the low-nibble item is in the high nibble of its source byte
    pub shhi: [u8; 16], // the high-nibble item is in the low nibble of its source byte
}

#[cfg(not(target_feature = "avx512f"))]
impl Gather {
    pub fn new() -> Gather {
        Gather { ident: None, nsrc: 0, src: [0; 32], mlo: [[0; 16]; 32], mhi: [[0; 16]; 32], idxlo: [0; 16], idxhi: [0; 16], shlo: [0; 16], shhi: [0; 16] }
    }
}

/// Plan the gather of 32 survivors (local item indices, ascending; u32::MAX for empty lanes).
#[cfg(not(target_feature = "avx512f"))]
pub unsafe fn plan_gather(lanes: &[u32; 32], g: &mut Gather) {
    let first = lanes[0];
    if first != u32::MAX && first % 32 == 0 && (1..32).all(|d| lanes[d] == first + d as u32) {
        g.ident = Some(first as usize / 32);
        return;
    }
    g.ident = None;
    g.idxlo = [0; 16];
    g.idxhi = [0; 16];
    g.shlo = [0; 16];
    g.shhi = [0; 16];
    // survivors ascend, so the source blocks are runs
    let mut ns = 0;
    for d in 0..32 {
        let v = lanes[d];
        if v == u32::MAX {
            continue;
        }
        let b = v >> 5;
        if ns == 0 || g.src[ns - 1] != b {
            g.src[ns] = b;
            g.mlo[ns] = [0; 16];
            g.mhi[ns] = [0; 16];
            ns += 1;
        }
        let (src_byte, src_hi) = ((v & 15) as u8, v & 16 != 0);
        if d < 16 {
            g.mlo[ns - 1][d] = 0xff;
            g.idxlo[d] = src_byte;
            g.shlo[d] = if src_hi { 0xff } else { 0 };
        } else {
            g.mhi[ns - 1][d - 16] = 0xff;
            g.idxhi[d - 16] = src_byte;
            g.shhi[d - 16] = if src_hi { 0 } else { 0xff };
        }
    }
    g.nsrc = ns;
}

#[cfg(not(target_feature = "avx512f"))]
#[inline(always)]
unsafe fn bcast16(p: &[u8; 16]) -> __m256i {
    _mm256_broadcastsi128_si256(_mm_loadu_si128(p.as_ptr() as *const _))
}

/// Assemble `rows` rows of a dest block from the source blocks at `base` (row stride `rows` x 16 per block) into dst,
/// two rows per pass (dst gets whole pairs of rows).
#[cfg(not(target_feature = "avx512f"))]
#[target_feature(enable = "avx2,fma")]
pub unsafe fn gather_rows(g: &Gather, base: *const u8, rows: usize, dst: *mut u8) {
    let stride = rows * 16;
    let m0f = _mm256_set1_epi8(0x0f);
    let mf0 = _mm256_set1_epi8(0xf0u8 as i8);
    let (ilo, ihi, slo, shi) = (bcast16(&g.idxlo), bcast16(&g.idxhi), bcast16(&g.shlo), bcast16(&g.shhi));
    let np = (rows + 1) / 2;
    for gi in 0..np {
        let mut t1 = _mm256_setzero_si256();
        let mut t2 = _mm256_setzero_si256();
        for s in 0..g.nsrc {
            let v = _mm256_loadu_si256(base.add(*g.src.get_unchecked(s) as usize * stride + gi * 32) as *const _);
            t1 = _mm256_blendv_epi8(t1, _mm256_shuffle_epi8(v, ilo), bcast16(g.mlo.get_unchecked(s)));
            t2 = _mm256_blendv_epi8(t2, _mm256_shuffle_epi8(v, ihi), bcast16(g.mhi.get_unchecked(s)));
        }
        let lo = _mm256_and_si256(_mm256_blendv_epi8(t1, _mm256_srli_epi16::<4>(t1), slo), m0f);
        let hi = _mm256_and_si256(_mm256_blendv_epi8(t2, _mm256_slli_epi16::<4>(t2), shi), mf0);
        _mm256_storeu_si256(dst.add(gi * 32) as *mut _, _mm256_or_si256(lo, hi));
    }
}

/// Integer table sums of one part over one block: S for items 0..15 and 16..31.
#[cfg(not(target_feature = "avx512f"))]
#[target_feature(enable = "avx2,fma")]
pub unsafe fn part_sums(codes: *const u8, tab: *const u8, groups: usize) -> (__m512, __m512) {
    let [(a, b)] = sums16::<1>(codes, [tab], groups);
    let f = |v: __m256i| {
        let lo = _mm256_cvtepu16_epi32(_mm256_castsi256_si128(v));
        let hi = _mm256_cvtepu16_epi32(_mm256_extracti128_si256::<1>(v));
        __m512(_mm256_cvtepi32_ps(lo), _mm256_cvtepi32_ps(hi))
    };
    (f(a), f(b))
}

/// Table sums of one part over one block for each of NT tables (the same codes), as u16 in item order: items 0..15
/// and items 16..31. Each sum is exact while it is below 65,536.
#[inline(always)]
pub unsafe fn sums16<const NT: usize>(codes: *const u8, tabs: [*const u8; NT], groups: usize) -> [(__m256i, __m256i); NT] {
    let m0f = _mm256_set1_epi8(0x0f);
    let z = _mm256_setzero_si256();
    let mut acc = [[z; 4]; NT];
    for h in 0..2 * groups {
        let c = _mm256_loadu_si256(codes.add(h * 32) as *const _);
        let lo = _mm256_and_si256(c, m0f);
        let hi = _mm256_and_si256(_mm256_srli_epi16::<4>(c), m0f);
        for k in 0..NT {
            let t = _mm256_loadu_si256(tabs[k].add(h * 32) as *const _);
            let r0 = _mm256_shuffle_epi8(t, lo);
            let r1 = _mm256_shuffle_epi8(t, hi);
            let a = &mut acc[k];
            a[0] = _mm256_add_epi16(a[0], r0);
            a[1] = _mm256_add_epi16(a[1], _mm256_srli_epi16::<8>(r0));
            a[2] = _mm256_add_epi16(a[2], r1);
            a[3] = _mm256_add_epi16(a[3], _mm256_srli_epi16::<8>(r1));
        }
    }
    core::array::from_fn(|k| {
        let a = &acc[k];
        (order16(_mm256_sub_epi16(a[0], _mm256_slli_epi16::<8>(a[1])), a[1]), order16(_mm256_sub_epi16(a[2], _mm256_slli_epi16::<8>(a[3])), a[3]))
    })
}

/// e: words = sums of even bytes per 128-bit lane, o: odd; sum the two lanes and interleave to item order.
#[inline(always)]
unsafe fn order16(e: __m256i, o: __m256i) -> __m256i {
    let f = |x: __m256i| _mm_add_epi16(_mm256_castsi256_si128(x), _mm256_extracti128_si256::<1>(x));
    let (e, o) = (f(e), f(o));
    _mm256_set_m128i(_mm_unpackhi_epi16(e, o), _mm_unpacklo_epi16(e, o))
}

// ---------------- integer deterministic levels ----------------
//
// A cluster's deterministic levels share one table step sigma, chosen so that the units' largest entries theta_u(e)
// (each at most 65,535, stored as a high and a low byte) sum to at most 256 IHMAX over all deterministic levels. The
// item's accumulators Hs (sum of high bytes) and Ls (sum of low bytes) run over every deterministic level:
// Theta = 256 Hs + Ls, Hs <= IHMAX, Ls <= 64 x 255. With B_l the sum of the
// units' minima and e_l the tables' error bound through level l, |sigma Theta + B_l - A| <= e_l for the real table sum
// A. Per item, from its key: Tin = ceil(Kin / sigma), Tout = floor(Kout / sigma), Kin = (t + fixed - qm - alr mr) / s,
// Kout = (t - D fixed - qm - alr mr) / s. Per level and norm code n, with X(n) = dwa pconst ntab(n) (1 + 1e-6) + ewdet:
// Rin(n) = ceil((X(n) + e_l - B_l) / sigma), Rout(n) = ceil((B_l + e_l + D X(n)) / sigma).
// IN iff Theta - Rin >= Tin; OUT iff Theta + Rout < Tout. Both imply the real-valued bound (dividing it by s > 0).
//
// Ranges. Tin and Tout are clamped to [TMIN, TMAX], so their high words lie in [-24000, 24000] and Hs - TH is exact in
// i16. Clamping is sound: raising Tin or lowering Tout only withholds decisions; lowering Tin to TMAX cannot make an
// IN, because Theta - Rin <= THETA_MAX - RIN_MIN < TMAX; raising Tout to TMIN cannot make an OUT, because
// Theta + Rout >= ROUT_MIN > TMIN. A margin outside [RIN_MIN or ROUT_MIN, RSENT) is replaced by RSENT, which never
// decides (THETA_MAX - RSENT < TMIN and RSENT > TMAX); so is the escape code 15. The rest of each compare saturates in
// the order (Hs - TH), then -/+ RH, then + (g >> 8), with g = Ls - TL -/+ RL in [-510, 16575]: a saturated word has
// magnitude at least 32,768, which g >> 8 (in [-2, 64]) cannot bring across zero.

pub const IHMAX: i64 = 8128;
pub const THETA_MAX: i64 = 256 * IHMAX + 64 * 255;
pub const TMIN: i64 = -24000 * 256;
pub const TMAX: i64 = 24000 * 256 + 255;
pub const RSENT: i64 = 32767 * 256;
pub const RIN_MIN: i64 = THETA_MAX - TMAX + 1;
pub const ROUT_MIN: i64 = TMIN;

/// The table step of a cluster's deterministic levels: the sum of the units' ranges (each the sum of its fields'
/// ranges, as `build_lut` finds it) over the budget 256 IHMAX, or the widest unit range over 65,535 if that is larger,
/// with margins for rounding.
pub fn int_sigma(w: &[f32], levels: &[LevelPlan], et: &ExpTab) -> f32 {
    let mut rmax = 0f64;
    let mut rsum = 0f64;
    let mut units = 0;
    for lp in levels {
        units += lp.units;
        for part in &lp.parts {
            for fs in &part.ufields {
                let mut r = 0f64;
                for &fi in fs.iter().take_while(|&&f| f != u16::MAX) {
                    let f = &part.fields[fi as usize];
                    r += (w[f.widx as usize] as f64).abs() * (et.hi[f.eid as usize] as f64 - et.lo[f.eid as usize] as f64).abs();
                }
                rmax = rmax.max(r);
                rsum += r;
            }
        }
    }
    assert!(units <= 64, "{units} deterministic units overflow the 16-bit low-byte sums; at most 64 fit");
    let budget = (256 * IHMAX - 3 * units as i64) as f64;
    ((rsum * (1.0 + 1.0 / 512.0) / budget).max(rmax * (1.0 + 1.0 / 512.0) / 65535.0) as f32).max(1e-30)
}

/// A level's margins over the norm code: in and out, each as a signed high word (low- and high-byte tables) and a low
/// byte, every table repeated in both 128-bit lanes for byte shuffles.
#[derive(Clone)]
pub struct RTab {
    pub inh: [[u8; 32]; 2],
    pub inl: [u8; 32],
    pub outh: [[u8; 32]; 2],
    pub outl: [u8; 32],
    pub sent: u32, // codes whose in (bit n) or out (bit 16 + n) margin is the sentinel
}

/// Margins in fine steps (real values; None for the escape code), rounded up and checked against their ranges.
pub fn rtab(rin: &[Option<f64>; 16], rout: &[Option<f64>; 16]) -> RTab {
    let mut r = RTab { inh: [[0; 32]; 2], inl: [0; 32], outh: [[0; 32]; 2], outl: [0; 32], sent: 0 };
    let up = |x: f64| (x + (x.abs() + 1.0) * 1e-12).ceil();
    for n in 0..16 {
        let ri = match rin[n] {
            Some(x) if up(x) >= RIN_MIN as f64 && up(x) < RSENT as f64 => up(x) as i64,
            _ => {
                r.sent |= 1 << n;
                RSENT
            }
        };
        let ro = match rout[n] {
            Some(x) if up(x) >= ROUT_MIN as f64 && up(x) < RSENT as f64 => up(x) as i64,
            _ => {
                r.sent |= 1 << (16 + n);
                RSENT
            }
        };
        for lane in [n, n + 16] {
            let (hi, lo) = ((ri >> 8) as i16, (ri & 255) as u8);
            r.inh[0][lane] = hi as u16 as u8;
            r.inh[1][lane] = (hi as u16 >> 8) as u8;
            r.inl[lane] = lo;
            let (hi, lo) = ((ro >> 8) as i16, (ro & 255) as u8);
            r.outh[0][lane] = hi as u16 as u8;
            r.outh[1][lane] = (hi as u16 >> 8) as u8;
            r.outl[lane] = lo;
        }
    }
    r
}

/// The integer state of the slots is stored by block of 32: six rows of 32 words, Hs, Ls, TinH, TinL, ToutH, ToutL.
pub const IROWS: usize = 6;
/// Position of field f of slot k in the integer state.
#[inline(always)]
pub fn ip(k: usize, f: usize) -> usize {
    (k / 32) * (32 * IROWS) + f * 32 + (k & 31)
}

/// Per-item thresholds for slots 0..n (rounded up to 4; s and mr are read that far) from s and mr:
/// kin = (cin - alr mr) / s and kout = (cout - alr mr) / s, divided by sigma and rounded outward with a slack covering
/// the f64 evaluation (`mag` bounds |t| + |fixed| + |qm|).
#[target_feature(enable = "avx2,fma")]
pub unsafe fn int_thresholds(s: &[f32], mr: &[f32], n: usize, cin: f64, cout: f64, alr: f64, mag: f64, sigma: f64, ist: &mut [i16]) {
    let eps = 2f64.powi(-46);
    let (vcin, vcout, valr, vmag) = (_mm256_set1_pd(cin), _mm256_set1_pd(cout), _mm256_set1_pd(alr), _mm256_set1_pd(mag));
    let (vinv, veps, vse) = (_mm256_set1_pd(1.0 / sigma), _mm256_set1_pd(eps), _mm256_set1_pd(eps / sigma));
    let (lo, hi) = (_mm256_set1_pd(TMIN as f64), _mm256_set1_pd(TMAX as f64));
    let sign = _mm256_set1_pd(-0.0);
    let abs = |x: __m256d| _mm256_andnot_pd(sign, x);
    let put = |v: __m256d, k: usize, f: usize, ist: &mut [i16]| {
        let v = _mm256_cvtpd_epi32(_mm256_min_pd(_mm256_max_pd(v, lo), hi));
        let h = _mm_packs_epi32(_mm_srai_epi32::<8>(v), v);
        let l = _mm_packs_epi32(_mm_and_si128(v, _mm_set1_epi32(255)), v);
        _mm_storel_epi64(ist.as_mut_ptr().add(ip(k, f)) as *mut _, h);
        _mm_storel_epi64(ist.as_mut_ptr().add(ip(k, f + 1)) as *mut _, l);
    };
    for k in (0..n).step_by(4) {
        let r = _mm256_div_pd(_mm256_set1_pd(1.0), _mm256_cvtps_pd(_mm_loadu_ps(s.as_ptr().add(k))));
        let a = _mm256_mul_pd(valr, _mm256_cvtps_pd(_mm_loadu_ps(mr.as_ptr().add(k))));
        let slack = _mm256_mul_pd(_mm256_mul_pd(_mm256_add_pd(vmag, abs(a)), r), vse);
        let yi = _mm256_mul_pd(_mm256_mul_pd(_mm256_sub_pd(vcin, a), r), vinv);
        let yo = _mm256_mul_pd(_mm256_mul_pd(_mm256_sub_pd(vcout, a), r), vinv);
        let ti = _mm256_ceil_pd(_mm256_add_pd(_mm256_add_pd(yi, slack), _mm256_mul_pd(abs(yi), veps)));
        let to = _mm256_floor_pd(_mm256_sub_pd(_mm256_sub_pd(yo, slack), _mm256_mul_pd(abs(yo), veps)));
        put(ti, k, 2, ist);
        put(to, k, 4, ist);
    }
}

/// The 16 i16 words looked up from a high-word table pair (low, high bytes) by the nibbles in idx (lane 0: items 0..15,
/// lane 1: items 16..31), as (items 0..15, items 16..31).
#[inline(always)]
unsafe fn look16(tlo: __m256i, thi: __m256i, idx: __m256i) -> (__m256i, __m256i) {
    let a = _mm256_shuffle_epi8(tlo, idx);
    let b = _mm256_shuffle_epi8(thi, idx);
    let u0 = _mm256_unpacklo_epi8(a, b);
    let u1 = _mm256_unpackhi_epi8(a, b);
    (_mm256_permute2x128_si256::<0x20>(u0, u1), _mm256_permute2x128_si256::<0x31>(u0, u1))
}

#[inline(always)]
unsafe fn ld32(p: &[u8; 32]) -> __m256i {
    _mm256_loadu_si256(p.as_ptr() as *const _)
}

/// The two-word compares for 16 slots: in lanes where Theta - Rin >= Tin, out lanes where Theta + Rout < Tout (all ones).
#[inline(always)]
pub unsafe fn cmp2w(hs: __m256i, ls: __m256i, t: [__m256i; 4], rin: (__m256i, __m256i), rout: (__m256i, __m256i)) -> (__m256i, __m256i) {
    let d = _mm256_subs_epi16(_mm256_subs_epi16(hs, t[0]), rin.0);
    let g = _mm256_sub_epi16(_mm256_sub_epi16(ls, t[1]), rin.1);
    let vin = _mm256_adds_epi16(d, _mm256_srai_epi16::<8>(g));
    let d = _mm256_adds_epi16(_mm256_subs_epi16(hs, t[2]), rout.0);
    let g = _mm256_sub_epi16(_mm256_add_epi16(ls, rout.1), t[3]);
    let vout = _mm256_adds_epi16(d, _mm256_srai_epi16::<8>(g));
    (_mm256_cmpgt_epi16(vin, _mm256_set1_epi16(-1)), _mm256_cmpgt_epi16(_mm256_setzero_si256(), vout))
}

/// 32-bit item-order mask from 16-bit lane masks of items 0..15 and 16..31.
#[inline(always)]
unsafe fn mask32(a: __m256i, b: __m256i) -> u32 {
    _mm256_movemask_epi8(_mm256_permute4x64_epi64::<0xd8>(_mm256_packs_epi16(a, b))) as u32
}

/// One deterministic level of one block of 32 slots on the integer path: adds the level's sums to Hs and Ls of the
/// block's state (`ist`, its six rows) and, when `decide`, returns the (in, out) masks from its thresholds and the
/// margins by the norm code in the scalar row sp.
#[target_feature(enable = "avx2,fma")]
pub unsafe fn int_level(codes: *const u8, lut: &LevelLut, sp: *const u8, rt: &RTab, ist: *mut i16, decide: bool) -> (u32, u32) {
    let (hs, ls) = (ist, ist.add(32));
    let mut acc = [[_mm256_setzero_si256(); 2]; 2];
    for pl in &lut.parts {
        let [(h0, h1), (l0, l1)] = sums16::<2>(codes.add(pl.g0 * 64), [lut.hi.as_ptr().add(pl.g0 * 64), lut.lo.as_ptr().add(pl.g0 * 64)], pl.groups);
        acc[0][0] = _mm256_add_epi16(acc[0][0], h0);
        acc[0][1] = _mm256_add_epi16(acc[0][1], h1);
        acc[1][0] = _mm256_add_epi16(acc[1][0], l0);
        acc[1][1] = _mm256_add_epi16(acc[1][1], l1);
    }
    let mut hv = [_mm256_setzero_si256(); 2];
    let mut lv = [_mm256_setzero_si256(); 2];
    for h in 0..2 {
        hv[h] = _mm256_add_epi16(_mm256_loadu_si256(hs.add(16 * h) as *const _), acc[0][h]);
        lv[h] = _mm256_add_epi16(_mm256_loadu_si256(ls.add(16 * h) as *const _), acc[1][h]);
        _mm256_storeu_si256(hs.add(16 * h) as *mut _, hv[h]);
        _mm256_storeu_si256(ls.add(16 * h) as *mut _, lv[h]);
    }
    if !decide {
        return (0, 0);
    }
    let r = _mm_loadu_si128(sp as *const _);
    let m = _mm_set1_epi8(0x0f);
    let idx = _mm256_set_m128i(_mm_and_si128(_mm_srli_epi16::<4>(r), m), _mm_and_si128(r, m));
    let z = _mm256_setzero_si256();
    let rih = look16(ld32(&rt.inh[0]), ld32(&rt.inh[1]), idx);
    let ril = look16(ld32(&rt.inl), z, idx);
    let roh = look16(ld32(&rt.outh[0]), ld32(&rt.outh[1]), idx);
    let rol = look16(ld32(&rt.outl), z, idx);
    let ldt = |k: usize, h: usize| _mm256_loadu_si256(ist.add(64 + 32 * k + 16 * h) as *const _);
    let (i0, o0) = cmp2w(hv[0], lv[0], [ldt(0, 0), ldt(1, 0), ldt(2, 0), ldt(3, 0)], (rih.0, ril.0), (roh.0, rol.0));
    let (i1, o1) = cmp2w(hv[1], lv[1], [ldt(0, 1), ldt(1, 1), ldt(2, 1), ldt(3, 1)], (rih.1, ril.1), (roh.1, rol.1));
    (mask32(i0, i1), mask32(o0, o1))
}

const fn compress16_lut() -> [[u8; 16]; 256] {
    let mut t = [[0x80u8; 16]; 256];
    let mut m = 0;
    while m < 256 {
        let mut k = 0;
        let mut i = 0;
        while i < 8 {
            if m >> i & 1 == 1 {
                t[m][2 * k] = 2 * i as u8;
                t[m][2 * k + 1] = 2 * i as u8 + 1;
                k += 1;
            }
            i += 1;
        }
        m += 1;
    }
    t
}
static C16: [[u8; 16]; 256] = compress16_lut();

/// Move the kept slots of o..o+16 (o a multiple of 16) to slots w.. of the integer state, every row (w <= o: only
/// slots already read are overwritten).
#[inline(always)]
pub unsafe fn cs_int(v: &mut [i16], w: usize, m: u16, o: usize) {
    let (m0, m1) = ((m & 255) as usize, (m >> 8) as usize);
    let (c0, n) = (m0.count_ones() as usize, m.count_ones() as usize);
    let n1 = n.min(32 - (w & 31));
    let mut buf = [0i16; 24];
    for f in 0..IROWS {
        let x = _mm256_loadu_si256(v.as_ptr().add(ip(o, f)) as *const _);
        let a = _mm_shuffle_epi8(_mm256_castsi256_si128(x), _mm_loadu_si128(C16[m0].as_ptr() as *const _));
        let b = _mm_shuffle_epi8(_mm256_extracti128_si256::<1>(x), _mm_loadu_si128(C16[m1].as_ptr() as *const _));
        _mm_storeu_si128(buf.as_mut_ptr() as *mut _, a);
        _mm_storeu_si128(buf.as_mut_ptr().add(c0) as *mut _, b);
        std::ptr::copy_nonoverlapping(buf.as_ptr(), v.as_mut_ptr().add(ip(w, f)), n1);
        if n > n1 {
            std::ptr::copy_nonoverlapping(buf.as_ptr().add(n1), v.as_mut_ptr().add(ip(w + n1, f)), n - n1);
        }
    }
}

/// Nibble of every lane of a scalar row (lanes 0..15 low nibbles, 16..31 high nibbles) as i32.
#[inline(always)]
pub unsafe fn row_nibbles(p: *const u8) -> (__m512i, __m512i) {
    let v = _mm512_cvtepu8_epi32(_mm_loadu_si128(p as *const _));
    (_mm512_and_si512(v, _mm512_set1_epi32(15)), _mm512_srli_epi32(v, 4))
}

// ---------------- tables per (group, level) and the per-cluster combine ----------------
//
// A cluster's weights are w = wq - alpha wm (wq per query and group, wm the centre's, per cluster), and a unit's table
// is linear in the weights: tau_u(e) = Tq_u(e) - alpha tau^m_u(e). Tq is built in f32 once per (query, group, level);
// tau^m is stored per (cluster, level) as 16-bit entries theta^m on a step sigma_m per part, shifted to a minimum of 0:
// |sigma_m theta^m_u(e) + lo^m_u - tau^m_u(e)| summed over a part's units is at most err_m. The combine quantises
// v_u(e) = Tq_u(e) - alpha sigma_m theta^m_u(e) on the cluster's step sigma_c from mu_u, a lower bound of v_u known
// without reading the entries: theta_u(e) = round((v_u(e) - mu_u) / sigma_c), so that the table value is
// sigma_c theta_u(e) + mu_u - alpha lo^m_u, and the part's offset is the sum of mu_u - alpha lo^m_u.

/// The f32 tables of the query weights of one (group, level): per unit 16 entries, their extremes, and per part the
/// bound on the f32 build error (summed over the part's units).
#[derive(Default)]
pub struct GTab {
    pub t: Vec<[f32; 16]>,
    pub tmin: Vec<f32>,
    pub tmax: Vec<f32>,
    pub err: Vec<f64>,
}

/// The centre's tables of one (cluster, level): per unit 16 entries theta^m and their largest, and per part the step,
/// the sum of the units' minima lo^m and the error bound.
#[derive(Default, Clone)]
pub struct CTab {
    pub th: Vec<[u16; 16]>,
    pub tmax: Vec<u16>,
    pub step: Vec<f32>,
    pub off: Vec<f64>,
    pub err: Vec<f64>,
}

/// f32 tables of weights w over a level's units.
#[target_feature(enable = "avx2,fma")]
pub unsafe fn group_tab(w: &[f32], lp: &LevelPlan, et: &ExpTab) -> GTab {
    let mut g = GTab { t: Vec::with_capacity(lp.units), tmin: Vec::with_capacity(lp.units), tmax: Vec::with_capacity(lp.units), err: Vec::new() };
    for part in &lp.parts {
        let mut sabs = 0f64;
        for fs in &part.ufields {
            let mut t = _mm512_setzero_ps();
            for &fi in fs.iter().take_while(|&&f| f != u16::MAX) {
                let f = part.fields.get_unchecked(fi as usize);
                let w = *w.get_unchecked(f.widx as usize);
                let e = f.eid as usize;
                t = _mm512_fmadd_ps(_mm512_set1_ps(w), _mm512_loadu_ps(et.e.get_unchecked(e).as_ptr()), t);
                sabs += w.abs() as f64 * (*et.hi.get_unchecked(e)).abs().max((*et.lo.get_unchecked(e)).abs()) as f64;
            }
            let mut a = [0f32; 16];
            _mm512_storeu_ps(a.as_mut_ptr(), t);
            g.t.push(a);
        }
        // each entry: at most 4 fields, a rounded product and sum each
        g.err.push(8.0 * U32 * sabs * (1.0 + 1e-6));
    }
    // the units' extremes, eight at a time
    let nu = g.t.len();
    g.tmin.resize(nu + 8, 0.0);
    g.tmax.resize(nu + 8, 0.0);
    let mut u0 = 0;
    while u0 < nu {
        let mut mn = [_mm256_setzero_ps(); 8];
        let mut mx = [_mm256_setzero_ps(); 8];
        for i in 0..8 {
            let p = g.t.get_unchecked((u0 + i).min(nu - 1)).as_ptr();
            let (a, b) = (_mm256_loadu_ps(p), _mm256_loadu_ps(p.add(8)));
            mn[i] = _mm256_min_ps(a, b);
            mx[i] = _mm256_max_ps(a, b);
        }
        _mm256_storeu_ps(g.tmin.as_mut_ptr().add(u0), reduce8::<false>(&mn));
        _mm256_storeu_ps(g.tmax.as_mut_ptr().add(u0), reduce8::<true>(&mx));
        u0 += 8;
    }
    g.tmin.truncate(nu);
    g.tmax.truncate(nu);
    g
}

/// The centre's tables of one (cluster, level) from its weights wm, as build_lut quantises them (16-bit).
pub fn centre_tab(wm: &[f32], lp: &LevelPlan, et: &ExpTab) -> CTab {
    let mut lut = LevelLut::default();
    unsafe { build_lut(wm, lp, et, None, &mut lut) };
    let mut c = CTab::default();
    for pl in &lut.parts {
        c.step.push(pl.scale);
        c.off.push(pl.off as f64);
        c.err.push(pl.err as f64);
        for u in 0..pl.groups * 4 {
            let gl = (pl.g0 + u / 4) * 64 + (u % 4) * 16;
            let th: [u16; 16] = core::array::from_fn(|e| (lut.hi[gl + e] as u16) << 8 | lut.lo[gl + e] as u16);
            c.tmax.push(*th.iter().max().unwrap());
            c.th.push(th);
        }
    }
    c
}

/// A unit's combined values v = Tq - alpha sigma_m theta^m in f32 (entries 0..7, 8..15) and their extremes.
#[inline(always)]
pub unsafe fn unit_v(gt: &GTab, ct: &CTab, u: usize, am: __m256) -> (__m256, __m256, f32, f32) {
    let tq = gt.t.get_unchecked(u).as_ptr();
    let tm = _mm256_loadu_si256(ct.th.get_unchecked(u).as_ptr() as *const _);
    let m0 = _mm256_cvtepi32_ps(_mm256_cvtepu16_epi32(_mm256_castsi256_si128(tm)));
    let m1 = _mm256_cvtepi32_ps(_mm256_cvtepu16_epi32(_mm256_extracti128_si256::<1>(tm)));
    let v0 = _mm256_fnmadd_ps(m0, am, _mm256_loadu_ps(tq));
    let v1 = _mm256_fnmadd_ps(m1, am, _mm256_loadu_ps(tq.add(8)));
    let hred = |x: __m256, f: &dyn Fn(__m128, __m128) -> __m128| {
        let a = f(_mm256_castps256_ps128(x), _mm256_extractf128_ps::<1>(x));
        let b = f(a, _mm_movehl_ps(a, a));
        _mm_cvtss_f32(f(b, _mm_shuffle_ps::<1>(b, b)))
    };
    let lo = hred(_mm256_min_ps(v0, v1), &|a, b| _mm_min_ps(a, b));
    let hi = hred(_mm256_max_ps(v0, v1), &|a, b| _mm_max_ps(a, b));
    (v0, v1, lo, hi)
}

/// Sum and largest of the combined units' ranges over the deterministic levels, for the integer step.
#[target_feature(enable = "avx2,fma")]
pub unsafe fn det_spans(gts: &[&GTab], cts: &[CTab], levels: &[LevelPlan], alpha: f32) -> (f64, f64) {
    let (mut s, mut m) = (0f64, 0f64);
    for ((gt, ct), lp) in gts.iter().zip(cts).zip(levels) {
        let mut u = 0;
        for (p, part) in lp.parts.iter().enumerate() {
            let am = _mm256_set1_ps(alpha * ct.step[p]);
            for _ in 0..part.units() {
                let (_, _, lo, hi) = unit_v(gt, ct, u, am);
                let r = (hi - lo) as f64;
                s += r;
                m = m.max(r);
                u += 1;
            }
        }
    }
    (s, m)
}

/// Lane u of the result: the smallest (MAX: largest) of the 8 lanes of x[u].
#[inline(always)]
unsafe fn reduce8<const MAX: bool>(x: &[__m256; 8]) -> __m256 {
    let f = |a: __m256, b: __m256| if MAX { _mm256_max_ps(a, b) } else { _mm256_min_ps(a, b) };
    let t01 = f(_mm256_unpacklo_ps(x[0], x[1]), _mm256_unpackhi_ps(x[0], x[1]));
    let t23 = f(_mm256_unpacklo_ps(x[2], x[3]), _mm256_unpackhi_ps(x[2], x[3]));
    let t45 = f(_mm256_unpacklo_ps(x[4], x[5]), _mm256_unpackhi_ps(x[4], x[5]));
    let t67 = f(_mm256_unpacklo_ps(x[6], x[7]), _mm256_unpackhi_ps(x[6], x[7]));
    let u03 = f(_mm256_shuffle_ps::<0x44>(t01, t23), _mm256_shuffle_ps::<0xee>(t01, t23));
    let u47 = f(_mm256_shuffle_ps::<0x44>(t45, t67), _mm256_shuffle_ps::<0xee>(t45, t67));
    f(_mm256_permute2f128_ps::<0x20>(u03, u47), _mm256_permute2f128_ps::<0x31>(u03, u47))
}

/// The cluster's level tables from the group's f32 tables and the centre's stored ones, with `step` (the integer
/// deterministic levels) or a step per part. Writes the same LevelLut as build_lut. Each unit is shifted by mu_u, the
/// smallest of its computed values, so its entries run from 0. The units' extremes are reduced eight at a time; the
/// tables equal combine_lut_unitwise's.
#[target_feature(enable = "avx2,fma")]
pub unsafe fn combine_lut(gt: &GTab, ct: &CTab, lp: &LevelPlan, alpha: f32, step: Option<f32>, out: &mut LevelLut) {
    out.qsum = 0.0;
    out.parts.clear();
    out.hi.clear();
    out.lo.clear();
    out.hi.resize(lp.units * 16, 0);
    out.lo.resize(lp.units * 16, 0);
    let mut g0 = 0;
    let mut u0 = 0;
    // the units' extremes, eight at a time (on the stack unless a part is long)
    let maxu = lp.parts.iter().map(|p| p.units()).max().unwrap_or(0) + 8;
    let (mut ls, mut hs) = ([0f32; 136], [0f32; 136]);
    let (mut lv, mut hv) = (Vec::new(), Vec::new());
    let (los, his): (&mut [f32], &mut [f32]) = if maxu <= 136 {
        (&mut ls, &mut hs)
    } else {
        lv.resize(maxu, 0f32);
        hv.resize(maxu, 0f32);
        (&mut lv, &mut hv)
    };
    for (p, part) in lp.parts.iter().enumerate() {
        let nu = part.units();
        let sm = ct.step[p];
        let am = _mm256_set1_ps(alpha * sm);
        let mut rmaxv = _mm256_setzero_ps();
        let mut k0 = 0;
        while k0 < nu {
            let mut mn = [_mm256_setzero_ps(); 8];
            let mut mx = [_mm256_setzero_ps(); 8];
            for i in 0..8 {
                // past the part's units, repeat its last one
                let u = u0 + (k0 + i).min(nu - 1);
                let tq = gt.t.get_unchecked(u).as_ptr();
                let tm = _mm256_loadu_si256(ct.th.get_unchecked(u).as_ptr() as *const _);
                let m0 = _mm256_cvtepi32_ps(_mm256_cvtepu16_epi32(_mm256_castsi256_si128(tm)));
                let m1 = _mm256_cvtepi32_ps(_mm256_cvtepu16_epi32(_mm256_extracti128_si256::<1>(tm)));
                let v0 = _mm256_fnmadd_ps(m0, am, _mm256_loadu_ps(tq));
                let v1 = _mm256_fnmadd_ps(m1, am, _mm256_loadu_ps(tq.add(8)));
                mn[i] = _mm256_min_ps(v0, v1);
                mx[i] = _mm256_max_ps(v0, v1);
            }
            let lo = reduce8::<false>(&mn);
            let hi = reduce8::<true>(&mx);
            _mm256_storeu_ps(los.as_mut_ptr().add(k0), lo);
            _mm256_storeu_ps(his.as_mut_ptr().add(k0), hi);
            rmaxv = _mm256_max_ps(rmaxv, _mm256_sub_ps(hi, lo));
            k0 += 8;
        }
        let rmax = reduce8::<true>(&[rmaxv; 8]);
        let rmax = _mm_cvtss_f32(_mm256_castps256_ps128(rmax));
        let sc = match step {
            Some(s) => s,
            None => (rmax / 65533.0).max(1e-30),
        };
        let inv = 1.0 / sc;
        let a = _mm256_set1_ps(inv);
        let (mut off, mut ey) = (0f64, 0f64);
        let asm = (alpha as f64 * sm as f64).abs();
        for k in 0..nu {
            let u = u0 + k;
            let tq = gt.t.get_unchecked(u).as_ptr();
            let tm = _mm256_loadu_si256(ct.th.get_unchecked(u).as_ptr() as *const _);
            let m0 = _mm256_cvtepi32_ps(_mm256_cvtepu16_epi32(_mm256_castsi256_si128(tm)));
            let m1 = _mm256_cvtepi32_ps(_mm256_cvtepu16_epi32(_mm256_extracti128_si256::<1>(tm)));
            let v0 = _mm256_fnmadd_ps(m0, am, _mm256_loadu_ps(tq));
            let v1 = _mm256_fnmadd_ps(m1, am, _mm256_loadu_ps(tq.add(8)));
            let (lo, hi) = (*los.get_unchecked(k), *his.get_unchecked(k));
            off += lo as f64;
            if step.is_some() {
                out.qsum += ((hi - lo) as f64 / sc as f64 * (1.0 + 1e-6)).ceil() + 1.0;
            }
            // f32 error of v (the product alpha sigma_m theta^m and the fused subtraction) and of (v - mu) / sc, in steps
            let mag = (gt.tmin[u].abs().max(gt.tmax[u].abs()) as f64 + asm * ct.tmax[u] as f64 + (lo.abs().max(hi.abs())) as f64) / sc as f64;
            ey += 4.0 * U32 * mag + 3.0 * U32 * 65536.0;
            let mu = _mm256_set1_ps(lo);
            let y0 = _mm256_mul_ps(_mm256_sub_ps(v0, mu), a);
            let y1 = _mm256_mul_ps(_mm256_sub_ps(v1, mu), a);
            // to u16 (saturating; the step keeps every value within 0..65535), then high and low bytes
            let v = _mm256_permute4x64_epi64::<0xd8>(_mm256_packus_epi32(_mm256_cvtps_epi32(y0), _mm256_cvtps_epi32(y1)));
            let hl = _mm256_permute4x64_epi64::<0xd8>(_mm256_packus_epi16(_mm256_srli_epi16::<8>(v), _mm256_and_si256(v, _mm256_set1_epi16(255))));
            let gl = (g0 + k / 4) * 64 + (k % 4) * 16;
            _mm_storeu_si128(out.hi.as_mut_ptr().add(gl) as *mut _, _mm256_castsi256_si128(hl));
            _mm_storeu_si128(out.lo.as_mut_ptr().add(gl) as *mut _, _mm256_extracti128_si256::<1>(hl));
        }
        off -= alpha as f64 * ct.off[p];
        let err = gt.err[p] + (alpha as f64).abs() * ct.err[p] + sc as f64 * (nu as f64 * 0.5 * (1.0 + 1e-6) + ey) + 4.0 * U32 * off.abs() + 1e-30;
        out.parts.push(PartLut { j: part.j, g0, groups: nu / 4, off: off as f32, scale: sc, err: err as f32 });
        g0 += nu / 4;
        u0 += nu;
    }
}

/// combine_lut one unit at a time (each unit's extremes by its own horizontal reduction); kept as its reference.
#[cfg(test)]
/// The cluster's level tables from the group's f32 tables and the centre's stored ones, with `step` (the integer
/// deterministic levels) or a step per part. Writes the same LevelLut as build_lut. Each unit is shifted by mu_u, the
/// smallest of its computed values, so its entries run from 0.
#[target_feature(enable = "avx2,fma")]
pub unsafe fn combine_lut_unitwise(gt: &GTab, ct: &CTab, lp: &LevelPlan, alpha: f32, step: Option<f32>, out: &mut LevelLut) {
    out.qsum = 0.0;
    out.parts.clear();
    out.hi.clear();
    out.lo.clear();
    out.hi.resize(lp.units * 16, 0);
    out.lo.resize(lp.units * 16, 0);
    let mut g0 = 0;
    let mut u0 = 0;
    let mut vs: Vec<(__m256, __m256, f32, f32)> = Vec::with_capacity(64);
    for (p, part) in lp.parts.iter().enumerate() {
        let nu = part.units();
        let sm = ct.step[p];
        let am = _mm256_set1_ps(alpha * sm);
        vs.clear();
        let mut rmax = 0f32;
        for u in u0..u0 + nu {
            let x = unit_v(gt, ct, u, am);
            rmax = rmax.max(x.3 - x.2);
            vs.push(x);
        }
        let sc = match step {
            Some(s) => s,
            None => (rmax / 65533.0).max(1e-30),
        };
        let inv = 1.0 / sc;
        let a = _mm256_set1_ps(inv);
        let (mut off, mut ey) = (0f64, 0f64);
        let asm = (alpha as f64 * sm as f64).abs();
        for (k, u) in (u0..u0 + nu).enumerate() {
            let (v0, v1, lo, hi) = vs[k];
            off += lo as f64;
            if step.is_some() {
                out.qsum += ((hi - lo) as f64 / sc as f64 * (1.0 + 1e-6)).ceil() + 1.0;
            }
            // f32 error of v (the product alpha sigma_m theta^m and the fused subtraction) and of (v - mu) / sc, in steps
            let mag = (gt.tmin[u].abs().max(gt.tmax[u].abs()) as f64 + asm * ct.tmax[u] as f64 + (lo.abs().max(hi.abs())) as f64) / sc as f64;
            ey += 4.0 * U32 * mag + 3.0 * U32 * 65536.0;
            let mu = _mm256_set1_ps(lo);
            let y0 = _mm256_mul_ps(_mm256_sub_ps(v0, mu), a);
            let y1 = _mm256_mul_ps(_mm256_sub_ps(v1, mu), a);
            // to u16 (saturating; the step keeps every value within 0..65535), then high and low bytes
            let v = _mm256_permute4x64_epi64::<0xd8>(_mm256_packus_epi32(_mm256_cvtps_epi32(y0), _mm256_cvtps_epi32(y1)));
            let hl = _mm256_permute4x64_epi64::<0xd8>(_mm256_packus_epi16(_mm256_srli_epi16::<8>(v), _mm256_and_si256(v, _mm256_set1_epi16(255))));
            let gl = (g0 + k / 4) * 64 + (k % 4) * 16;
            _mm_storeu_si128(out.hi.as_mut_ptr().add(gl) as *mut _, _mm256_castsi256_si128(hl));
            _mm_storeu_si128(out.lo.as_mut_ptr().add(gl) as *mut _, _mm256_extracti128_si256::<1>(hl));
        }
        off -= alpha as f64 * ct.off[p];
        let err = gt.err[p] + (alpha as f64).abs() * ct.err[p] + sc as f64 * (nu as f64 * 0.5 * (1.0 + 1e-6) + ey) + 4.0 * U32 * off.abs() + 1e-30;
        out.parts.push(PartLut { j: part.j, g0, groups: nu / 4, off: off as f32, scale: sc, err: err as f32 });
        g0 += nu / 4;
        u0 += nu;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nib(buf: &[u8], row: usize, item: usize) -> u8 {
        let b = buf[row * 16 + item % 16];
        if item < 16 { b & 15 } else { b >> 4 }
    }

    /// The two-word compares against i64 arithmetic over the full ranges, with the extremes over-sampled; thresholds
    /// beyond the clamp must never give a decision the true threshold would not.
    #[test]
    fn two_word_compare_matches_i64() {
        let mut x = 0x9E3779B97F4A7C15u64;
        let mut rnd = |lo: i64, hi: i64| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            match x % 8 {
                0 => lo,
                1 => hi,
                2 => lo + (x >> 8) as i64 % 3,
                3 => hi - (x >> 8) as i64 % 3,
                _ => lo + ((x >> 8) % (hi - lo + 1) as u64) as i64,
            }
        };
        let split = |v: i64| ((v >> 8) as i16, (v & 255) as i16);
        let mut wrong = 0;
        for _ in 0..200_000 {
            let mut hs = [0i16; 16];
            let mut ls = [0i16; 16];
            let mut t = [[0i16; 16]; 4];
            let mut rr = [[0i16; 16]; 4];
            let mut want = [(false, false); 16];
            for i in 0..16 {
                let (h, l) = (rnd(0, IHMAX), rnd(0, 64 * 255));
                hs[i] = h as i16;
                ls[i] = l as i16;
                let theta = 256 * h + l;
                // true thresholds may lie far outside the clamp
                let (tin, tout) = (rnd(-3 * THETA_MAX - TMAX, 3 * TMAX), rnd(-3 * TMAX, 3 * TMAX));
                let rin = if rnd(0, 4) == 0 { RSENT } else { rnd(RIN_MIN, RSENT - 1) };
                let rout = if rnd(0, 6) == 0 { RSENT } else { rnd(ROUT_MIN, RSENT - 1) };
                let ci = split(tin.clamp(TMIN, TMAX));
                let co = split(tout.clamp(TMIN, TMAX));
                (t[0][i], t[1][i], t[2][i], t[3][i]) = (ci.0, ci.1, co.0, co.1);
                let (a, b) = split(rin);
                (rr[0][i], rr[1][i]) = (a, b);
                let (a, b) = split(rout);
                (rr[2][i], rr[3][i]) = (a, b);
                let tin_c = tin.clamp(TMIN, TMAX);
                let tout_c = tout.clamp(TMIN, TMAX);
                let got = (theta - rin >= tin_c && rin != RSENT, theta + rout < tout_c && rout != RSENT);
                // the clamped decision must be implied by the true one, and equal the exact clamped arithmetic
                assert!(!got.0 || theta - rin >= tin, "clamped IN not implied");
                assert!(!got.1 || theta + rout < tout, "clamped OUT not implied");
                assert!(!(rin == RSENT && theta - rin >= tin_c) && !(rout == RSENT && theta + rout < tout_c), "sentinel decided");
                want[i] = got;
            }
            let ld = |a: &[i16; 16]| unsafe { _mm256_loadu_si256(a.as_ptr() as *const _) };
            let (mi, mo) = unsafe { cmp2w(ld(&hs), ld(&ls), [ld(&t[0]), ld(&t[1]), ld(&t[2]), ld(&t[3])], (ld(&rr[0]), ld(&rr[1])), (ld(&rr[2]), ld(&rr[3]))) };
            let (mut gi, mut go) = ([0i16; 16], [0i16; 16]);
            unsafe {
                _mm256_storeu_si256(gi.as_mut_ptr() as *mut _, mi);
                _mm256_storeu_si256(go.as_mut_ptr() as *mut _, mo);
            }
            for i in 0..16 {
                if (gi[i] != 0, go[i] != 0) != want[i] {
                    wrong += 1;
                }
            }
        }
        assert_eq!(wrong, 0);
    }

    #[test]
    #[ignore]
    fn int_level_speed() {
        let groups = 2;
        let nb: usize = std::env::var("NB").map(|v| v.parse().unwrap()).unwrap_or(4096);
        let codes: Vec<u8> = (0..nb * groups * 64).map(|i| (i * 37 % 251) as u8).collect();
        let lut = LevelLut { parts: vec![PartLut { j: None, g0: 0, groups, off: 0.0, scale: 1.0, err: 0.0 }], hi: vec![3; groups * 64], lo: vec![7; groups * 64], qsum: 0.0 };
        let rt = rtab(&[Some(100.0); 16], &[Some(100.0); 16]);
        let sp: Vec<u8> = (0..nb * 16).map(|i| (i * 13 % 256) as u8).collect();
        let mut ist = vec![3i16; 64 * 32 * IROWS];
        let mut x = 0u32;
        let t = std::time::Instant::now();
        let reps = (200_000 / nb).max(3);
        for _ in 0..reps {
            for b in 0..nb {
                let (i, o) = unsafe { int_level(codes.as_ptr().add(b * groups * 64), &lut, sp.as_ptr().add(b * 16), &rt, ist.as_mut_ptr().add(b % 64 * 32 * IROWS), true) };
                x = x.wrapping_add(i ^ o);
            }
        }
        let ns = t.elapsed().as_nanos() as f64 / (reps * nb * 32) as f64;
        println!("int_level, 8 units: {ns:.3} ns per item ({x})");
    }

    #[test]
    fn cs_int_keeps_order() {
        for (w, o) in [(3usize, 16usize), (25, 32), (31, 48), (40, 48), (0, 0)] {
            for m in [0u16, 1, 0x8000, 0xffff, 0x5a5a, 0x00f0, 0x0f0f, 0x1234] {
                let mut v: Vec<i16> = (0..4 * 32 * IROWS).map(|i| i as i16).collect();
                let src: Vec<Vec<i16>> = (0..IROWS).map(|f| (0..16).filter(|&i| m >> i & 1 == 1).map(|i| v[ip(o + i, f)]).collect()).collect();
                unsafe { cs_int(&mut v, w, m, o) };
                for f in 0..IROWS {
                    let got: Vec<i16> = (0..src[f].len()).map(|k| v[ip(w + k, f)]).collect();
                    assert_eq!(got, src[f], "w {w} o {o} m {m:#x} row {f}");
                }
            }
        }
    }

    #[test]
    fn part_sums_match_scalar() {
        let groups = 5;
        let codes: Vec<u8> = (0..groups * 64).map(|i| (i * 37 % 251) as u8).collect();
        let tab: Vec<u8> = (0..groups * 64).map(|i| (i * 91 % 256) as u8).collect();
        let (s0, s1) = unsafe { part_sums(codes.as_ptr(), tab.as_ptr(), groups) };
        let mut got = [0f32; 32];
        unsafe {
            _mm512_storeu_ps(got.as_mut_ptr(), s0);
            _mm512_storeu_ps(got.as_mut_ptr().add(16), s1);
        }
        for item in 0..32 {
            let want: u32 = (0..groups * 4).map(|row| tab[row * 16 + nib(&codes, row, item) as usize] as u32).sum();
            assert_eq!(got[item], want as f32, "item {item}");
        }
    }

    #[test]
    fn part_sums_at_the_largest_part() {
        // 64 groups (256 units, the most Part::pack allows) of all-255 tables: 65,280, the largest 16-bit sum
        let groups = 64;
        let codes: Vec<u8> = (0..groups * 64).map(|i| (i * 37 % 251) as u8).collect();
        let tab = vec![255u8; groups * 64];
        let (s0, s1) = unsafe { part_sums(codes.as_ptr(), tab.as_ptr(), groups) };
        let mut got = [0f32; 32];
        unsafe {
            _mm512_storeu_ps(got.as_mut_ptr(), s0);
            _mm512_storeu_ps(got.as_mut_ptr().add(16), s1);
        }
        assert!(got.iter().all(|&x| x == (255 * 4 * groups) as f32), "{got:?}");
    }

    #[test]
    fn gather_rows_match_scalar() {
        let rows = 5;
        let nblk = 6;
        let src: Vec<u8> = (0..nblk * rows * 16 + 128).map(|i| (i * 53 % 256) as u8).collect();
        let mut lanes = [u32::MAX; 32];
        let picks = [1u32, 2, 17, 31, 32, 40, 63, 64, 65, 66, 80, 95, 100, 120, 130, 150, 160, 161, 170, 175, 180, 190];
        lanes[..picks.len()].copy_from_slice(&picks);
        let mut g = Gather::new();
        unsafe { plan_gather(&lanes, &mut g) };
        let mut dst = vec![0u8; rows * 16 + 64];
        unsafe { gather_rows(&g, src.as_ptr(), rows, dst.as_mut_ptr()) };
        for (d, &v) in lanes.iter().enumerate() {
            for row in 0..rows {
                let want = if v == u32::MAX { 0 } else { nib(&src[(v as usize / 32) * rows * 16..], row, v as usize % 32) };
                assert_eq!(nib(&dst, row, d), want, "lane {d} row {row}");
            }
        }
    }

    /// The combined tables against f64 tables of the weights wq - alpha wm: for random weights, signs of alpha and
    /// codes, every part's value off + scale S is within its error bound of the exact sum, on a step per part and on a
    /// common step.
    #[test]
    fn combine_lut_within_bound() {
        let mut x = 0x2545F4914F6CDD1Du64;
        let mut rnd = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut et = ExpTab::default();
        let n = 64;
        // 2-bit fields over n coordinates, then 1-bit fields: 2 parts
        let mk = |j: Option<usize>, nb: u8, et: &mut ExpTab, r: &mut dyn FnMut() -> f64| {
            let fields: Vec<Field> = (0..n).map(|c| Field { widx: c as u16, nb, unit: 0, off: 0, kind: FK::Rand { d: 0 }, eid: 0 }).collect();
            let vals: Vec<[f32; 16]> = (0..n).map(|_| core::array::from_fn(|k| if k < 1 << nb { (r() * 4.0 - 2.0) as f32 } else { 0.0 })).collect();
            Part::pack(j, fields, vals, et)
        };
        let parts = vec![mk(None, 2, &mut et, &mut rnd), mk(Some(1), 1, &mut et, &mut rnd)];
        let units = parts.iter().map(|p| p.units()).sum();
        let lp = LevelPlan { parts, units, srows: 0 };
        for trial in 0..200 {
            let wq: Vec<f32> = (0..n).map(|_| (rnd() * 2.0 - 1.0) as f32 * 10f32.powi(trial % 3 - 1)).collect();
            let wm: Vec<f32> = (0..n).map(|_| (rnd() * 2.0 - 1.0) as f32).collect();
            let alpha = ((rnd() * 2.0 - 1.0) * 3.0) as f32;
            let gt = unsafe { group_tab(&wq, &lp, &et) };
            let ct = centre_tab(&wm, &lp, &et);
            let (_, rmax) = unsafe { det_spans(&[&gt], std::slice::from_ref(&ct), std::slice::from_ref(&lp), alpha) };
            for step in [None, Some((rmax / 60000.0 * (1.0 + rnd())) as f32)] {
                let mut lut = LevelLut::default();
                unsafe { combine_lut(&gt, &ct, &lp, alpha, step, &mut lut) };
                let mut lr = LevelLut::default();
                unsafe { combine_lut_unitwise(&gt, &ct, &lp, alpha, step, &mut lr) };
                assert!(lut.hi == lr.hi && lut.lo == lr.lo && lut.qsum == lr.qsum);
                for (a, b) in lut.parts.iter().zip(&lr.parts) {
                    assert!(a.off == b.off && a.scale == b.scale && a.err == b.err && a.g0 == b.g0);
                }
                for _ in 0..20 {
                    let codes: Vec<u8> = (0..units).map(|_| (rnd() * 16.0) as u8 & 15).collect();
                    for (pl, part) in lut.parts.iter().zip(&lp.parts) {
                        let (mut exact, mut sum) = (0f64, 0f64);
                        for (k, fs) in part.ufields.iter().enumerate() {
                            let u = pl.g0 * 4 + k;
                            let e = codes[u] as usize;
                            for &fi in fs.iter().take_while(|&&f| f != u16::MAX) {
                                let f = &part.fields[fi as usize];
                                exact += (wq[f.widx as usize] as f64 - alpha as f64 * wm[f.widx as usize] as f64) * et.e[f.eid as usize][e] as f64;
                            }
                            let gl = (pl.g0 + k / 4) * 64 + (k % 4) * 16;
                            sum += ((lut.hi[gl + e] as u32) << 8 | lut.lo[gl + e] as u32) as f64;
                        }
                        let got = pl.off as f64 + pl.scale as f64 * sum;
                        assert!((got - exact).abs() <= pl.err as f64, "trial {trial} step {step:?}: |{got} - {exact}| > {}", pl.err);
                    }
                }
            }
        }
    }

    #[test]
    fn compress_keeps_order() {
        let v: Vec<i32> = (0..16).map(|i| 100 + i).collect();
        for m in 0..=u16::MAX {
            let mut out = [0i32; 16];
            unsafe {
                let x = _mm512_loadu_si512(v.as_ptr() as *const _);
                _mm512_storeu_si512(out.as_mut_ptr() as *mut _, _mm512_maskz_compress_epi32(m, x));
            }
            let want: Vec<i32> = (0..16).filter(|&i| m >> i & 1 == 1).map(|i| 100 + i).collect();
            assert_eq!(&out[..want.len()], &want[..]);
            assert!(out[want.len()..].iter().all(|&x| x == 0));
        }
    }
}
