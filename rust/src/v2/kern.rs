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
    pub lo: Vec<u8>, // low bytes of 16-bit tables (split)
}

/// Build the level's tables from the cluster's combined weights w (det by coordinate, rand by n + position).
#[target_feature(enable = "avx2,fma")]
pub unsafe fn build_lut(w: &[f32], lp: &LevelPlan, et: &ExpTab, split: bool, out: &mut LevelLut) {
    let qmax: f32 = if split { 65535.0 } else { 255.0 };
    out.parts.clear();
    out.hi.clear();
    out.lo.clear();
    out.hi.resize(lp.units * 16, 0);
    if split {
        out.lo.resize(lp.units * 16, 0);
    }
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
            sabs += sa as f64;
            off += lo as f64;
            tabs.push(_mm512_sub_ps(t, _mm512_set1_ps(lo)));
        }
        let scale = if rmax > 0.0 { rmax / qmax } else { 1e-30 };
        let inv = _mm512_set1_ps(1.0 / scale);
        for (u, t) in tabs.iter().enumerate() {
            let q = _mm512_min_epi32(_mm512_max_epi32(_mm512_cvtps_epi32(_mm512_mul_ps(*t, inv)), _mm512_setzero_si512()), _mm512_set1_epi32(qmax as i32));
            let gl = (g0 + u / 4) * 64 + (u % 4) * 16;
            if split {
                _mm_storeu_si128(out.hi.as_mut_ptr().add(gl) as *mut _, _mm512_cvtepi32_epi8(_mm512_srli_epi32(q, 8)));
                _mm_storeu_si128(out.lo.as_mut_ptr().add(gl) as *mut _, _mm512_cvtepi32_epi8(_mm512_and_si512(q, _mm512_set1_epi32(255))));
            } else {
                _mm_storeu_si128(out.hi.as_mut_ptr().add(gl) as *mut _, _mm512_cvtepi32_epi8(q));
            }
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
    let m0f = _mm256_set1_epi8(0x0f);
    let (mut a0, mut b0, mut a1, mut b1) = (_mm256_setzero_si256(), _mm256_setzero_si256(), _mm256_setzero_si256(), _mm256_setzero_si256());
    for h in 0..2 * groups {
        let c = _mm256_loadu_si256(codes.add(h * 32) as *const _);
        let t = _mm256_loadu_si256(tab.add(h * 32) as *const _);
        let r0 = _mm256_shuffle_epi8(t, _mm256_and_si256(c, m0f));
        let r1 = _mm256_shuffle_epi8(t, _mm256_and_si256(_mm256_srli_epi16::<4>(c), m0f));
        a0 = _mm256_add_epi16(a0, r0);
        b0 = _mm256_add_epi16(b0, _mm256_srli_epi16::<8>(r0));
        a1 = _mm256_add_epi16(a1, r1);
        b1 = _mm256_add_epi16(b1, _mm256_srli_epi16::<8>(r1));
    }
    (lane_sum(_mm256_sub_epi16(a0, _mm256_slli_epi16::<8>(b0)), b0), lane_sum(_mm256_sub_epi16(a1, _mm256_slli_epi16::<8>(b1)), b1))
}

/// e: words = sums of even bytes per 128-bit lane, o: odd; sum the two lanes and interleave to item order.
#[cfg(not(target_feature = "avx512f"))]
#[inline(always)]
unsafe fn lane_sum(e: __m256i, o: __m256i) -> __m512 {
    let f = |x: __m256i| _mm_add_epi16(_mm256_castsi256_si128(x), _mm256_extracti128_si256::<1>(x));
    let (e, o) = (f(e), f(o));
    let a = _mm256_cvtepu16_epi32(_mm_unpacklo_epi16(e, o));
    let b = _mm256_cvtepu16_epi32(_mm_unpackhi_epi16(e, o));
    __m512(_mm256_cvtepi32_ps(a), _mm256_cvtepi32_ps(b))
}

/// Nibble of every lane of a scalar row (lanes 0..15 low nibbles, 16..31 high nibbles) as i32.
#[inline(always)]
pub unsafe fn row_nibbles(p: *const u8) -> (__m512i, __m512i) {
    let v = _mm512_cvtepu8_epi32(_mm_loadu_si128(p as *const _));
    (_mm512_and_si512(v, _mm512_set1_epi32(15)), _mm512_srli_epi32(v, 4))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nib(buf: &[u8], row: usize, item: usize) -> u8 {
        let b = buf[row * 16 + item % 16];
        if item < 16 { b & 15 } else { b >> 4 }
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

    #[test]
    fn compress_keeps_order() {
        let v: Vec<i32> = (0..16).map(|i| 100 + i).collect();
        for m in [0u16, 1, 0x8000, 0xffff, 0x5a5a, 0x00f0, 0x0f0f] {
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
