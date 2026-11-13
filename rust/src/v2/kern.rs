//! Fast-scan pieces: per-(query, cluster, level) 16-entry tables quantised to u8 (or two u8 halves of a u16) with a
//! bound on the quantisation error, re-packing of survivors into dense 32-item blocks (masked byte permutes over the
//! transposed nibble rows), and the shuffle-and-add accumulation of a part of a level over one block.
//!
//! Block layout: 32 items; a row is 16 bytes, byte p holds item p in the low nibble and item 16 + p in the high nibble.
//! A level of a cluster stores, per block, its code rows (one per 4-bit unit) contiguously.
use super::*;
use std::arch::x86_64::*;

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
#[target_feature(enable = "avx512f,avx512bw,avx512dq,avx512vl,avx512vbmi")]
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

#[inline(always)]
fn rep16(m: u16) -> u64 {
    let m = m as u64;
    m | m << 16 | m << 32 | m << 48
}

/// Plan the gather of 32 survivors (local item indices, ascending; u32::MAX for empty lanes).
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

/// Nibble of every lane of a scalar row (lanes 0..15 low nibbles, 16..31 high nibbles) as i32.
#[inline(always)]
pub unsafe fn row_nibbles(p: *const u8) -> (__m512i, __m512i) {
    let v = _mm512_cvtepu8_epi32(_mm_loadu_si128(p as *const _));
    (_mm512_and_si512(v, _mm512_set1_epi32(15)), _mm512_srli_epi32(v, 4))
}
