//! The 16-lane vector operations of the v2 kernels. With AVX-512 these are the native intrinsics; without it, each
//! 16-lane vector is two AVX2 registers and each 16-bit mask a `u16`, under the same names and with the same results
//! for the inputs the kernels give them (noted where an operation is narrower than its AVX-512 namesake).
#![allow(non_camel_case_types, clippy::missing_safety_doc)]

#[cfg(target_feature = "avx512f")]
pub use std::arch::x86_64::*;

#[cfg(not(target_feature = "avx512f"))]
pub use emu::*;

#[cfg(not(target_feature = "avx512f"))]
mod emu {
    // the emulations below shadow their AVX-512 namesakes
    pub use std::arch::x86_64::*;

    #[derive(Clone, Copy)]
    #[repr(C)]
    pub struct __m512(pub __m256, pub __m256);
    #[derive(Clone, Copy)]
    #[repr(C)]
    pub struct __m512i(pub __m256i, pub __m256i);

    /// Lane masks of 8 lanes from the bits of `m`.
    #[inline(always)]
    pub unsafe fn mask8(m: u32) -> __m256i {
        let bits = _mm256_setr_epi32(1, 2, 4, 8, 16, 32, 64, 128);
        _mm256_cmpeq_epi32(_mm256_and_si256(_mm256_set1_epi32(m as i32), bits), bits)
    }
    #[inline(always)]
    unsafe fn mm(m: u16) -> (__m256i, __m256i) {
        (mask8(m as u32 & 255), mask8(m as u32 >> 8))
    }
    #[inline(always)]
    unsafe fn bits8(v: __m256i) -> u16 {
        _mm256_movemask_ps(_mm256_castsi256_ps(v)) as u16
    }

    macro_rules! ps2 {
        ($name:ident, $op:ident) => {
            #[inline(always)]
            pub unsafe fn $name(a: __m512, b: __m512) -> __m512 {
                __m512($op(a.0, b.0), $op(a.1, b.1))
            }
        };
    }
    macro_rules! si2 {
        ($name:ident, $op:ident) => {
            #[inline(always)]
            pub unsafe fn $name(a: __m512i, b: __m512i) -> __m512i {
                __m512i($op(a.0, b.0), $op(a.1, b.1))
            }
        };
    }
    ps2!(_mm512_add_ps, _mm256_add_ps);
    ps2!(_mm512_sub_ps, _mm256_sub_ps);
    ps2!(_mm512_mul_ps, _mm256_mul_ps);
    ps2!(_mm512_div_ps, _mm256_div_ps);
    ps2!(_mm512_max_ps, _mm256_max_ps);
    si2!(_mm512_add_epi32, _mm256_add_epi32);
    si2!(_mm512_min_epi32, _mm256_min_epi32);
    si2!(_mm512_max_epi32, _mm256_max_epi32);
    si2!(_mm512_and_si512, _mm256_and_si256);

    #[inline(always)]
    pub unsafe fn _mm512_setzero_ps() -> __m512 {
        __m512(_mm256_setzero_ps(), _mm256_setzero_ps())
    }
    #[inline(always)]
    pub unsafe fn _mm512_set1_ps(a: f32) -> __m512 {
        __m512(_mm256_set1_ps(a), _mm256_set1_ps(a))
    }
    #[inline(always)]
    pub unsafe fn _mm512_loadu_ps(p: *const f32) -> __m512 {
        __m512(_mm256_loadu_ps(p), _mm256_loadu_ps(p.add(8)))
    }
    #[inline(always)]
    pub unsafe fn _mm512_storeu_ps(p: *mut f32, a: __m512) {
        _mm256_storeu_ps(p, a.0);
        _mm256_storeu_ps(p.add(8), a.1);
    }
    #[inline(always)]
    pub unsafe fn _mm512_fmadd_ps(a: __m512, b: __m512, c: __m512) -> __m512 {
        __m512(_mm256_fmadd_ps(a.0, b.0, c.0), _mm256_fmadd_ps(a.1, b.1, c.1))
    }
    #[inline(always)]
    pub unsafe fn _mm512_sqrt_ps(a: __m512) -> __m512 {
        __m512(_mm256_sqrt_ps(a.0), _mm256_sqrt_ps(a.1))
    }
    #[inline(always)]
    pub unsafe fn _mm512_abs_ps(a: __m512) -> __m512 {
        let m = _mm256_castsi256_ps(_mm256_set1_epi32(0x7fff_ffff));
        __m512(_mm256_and_ps(a.0, m), _mm256_and_ps(a.1, m))
    }
    #[inline(always)]
    pub unsafe fn _mm512_cvtepi32_ps(a: __m512i) -> __m512 {
        __m512(_mm256_cvtepi32_ps(a.0), _mm256_cvtepi32_ps(a.1))
    }
    #[inline(always)]
    pub unsafe fn _mm512_cvtps_epi32(a: __m512) -> __m512i {
        __m512i(_mm256_cvtps_epi32(a.0), _mm256_cvtps_epi32(a.1))
    }
    /// Lanes of a 16-entry table at indices 0..=15.
    #[inline(always)]
    pub unsafe fn _mm512_permutexvar_ps(idx: __m512i, t: __m512) -> __m512 {
        let half = |i: __m256i| {
            let hi = _mm256_castsi256_ps(_mm256_slli_epi32(i, 28));
            _mm256_blendv_ps(_mm256_permutevar8x32_ps(t.0, i), _mm256_permutevar8x32_ps(t.1, i), hi)
        };
        __m512(half(idx.0), half(idx.1))
    }
    #[inline(always)]
    pub unsafe fn _mm512_i32gather_ps<const SCALE: i32>(idx: __m512i, p: *const f32) -> __m512 {
        __m512(_mm256_i32gather_ps::<SCALE>(p, idx.0), _mm256_i32gather_ps::<SCALE>(p, idx.1))
    }
    #[inline(always)]
    pub unsafe fn _mm512_mask_i32gather_epi32<const SCALE: i32>(src: __m512i, m: u16, idx: __m512i, p: *const i32) -> __m512i {
        let (m0, m1) = mm(m);
        __m512i(_mm256_mask_i32gather_epi32::<SCALE>(src.0, p, idx.0, m0), _mm256_mask_i32gather_epi32::<SCALE>(src.1, p, idx.1, m1))
    }
    #[inline(always)]
    pub unsafe fn _mm512_cmp_ps_mask<const P: i32>(a: __m512, b: __m512) -> u16 {
        _mm256_movemask_ps(_mm256_cmp_ps::<P>(a.0, b.0)) as u16 | (_mm256_movemask_ps(_mm256_cmp_ps::<P>(a.1, b.1)) as u16) << 8
    }

    #[inline(always)]
    pub unsafe fn _mm512_setzero_si512() -> __m512i {
        __m512i(_mm256_setzero_si256(), _mm256_setzero_si256())
    }
    #[inline(always)]
    pub unsafe fn _mm512_set1_epi32(a: i32) -> __m512i {
        __m512i(_mm256_set1_epi32(a), _mm256_set1_epi32(a))
    }
    #[inline(always)]
    pub unsafe fn _mm512_loadu_si512(p: *const __m512i) -> __m512i {
        let p = p as *const __m256i;
        __m512i(_mm256_loadu_si256(p), _mm256_loadu_si256(p.add(1)))
    }
    #[inline(always)]
    pub unsafe fn _mm512_storeu_si512(p: *mut __m512i, a: __m512i) {
        let p = p as *mut __m256i;
        _mm256_storeu_si256(p, a.0);
        _mm256_storeu_si256(p.add(1), a.1);
    }
    /// Lanes outside `m` are neither read nor written.
    #[inline(always)]
    pub unsafe fn _mm512_maskz_loadu_epi32(m: u16, p: *const i32) -> __m512i {
        let (m0, m1) = mm(m);
        __m512i(_mm256_maskload_epi32(p, m0), _mm256_maskload_epi32(p.add(8), m1))
    }
    #[inline(always)]
    pub unsafe fn _mm512_mask_storeu_epi32(p: *mut i32, m: u16, a: __m512i) {
        let (m0, m1) = mm(m);
        _mm256_maskstore_epi32(p, m0, a.0);
        _mm256_maskstore_epi32(p.add(8), m1, a.1);
    }
    #[inline(always)]
    pub unsafe fn _mm512_maskz_mov_epi32(m: u16, a: __m512i) -> __m512i {
        let (m0, m1) = mm(m);
        __m512i(_mm256_and_si256(a.0, m0), _mm256_and_si256(a.1, m1))
    }
    #[inline(always)]
    pub unsafe fn _mm512_cmpeq_epi32_mask(a: __m512i, b: __m512i) -> u16 {
        bits8(_mm256_cmpeq_epi32(a.0, b.0)) | bits8(_mm256_cmpeq_epi32(a.1, b.1)) << 8
    }
    #[inline(always)]
    pub unsafe fn _mm512_slli_epi32(a: __m512i, k: u32) -> __m512i {
        let c = _mm_cvtsi32_si128(k as i32);
        __m512i(_mm256_sll_epi32(a.0, c), _mm256_sll_epi32(a.1, c))
    }
    #[inline(always)]
    pub unsafe fn _mm512_srli_epi32(a: __m512i, k: u32) -> __m512i {
        let c = _mm_cvtsi32_si128(k as i32);
        __m512i(_mm256_srl_epi32(a.0, c), _mm256_srl_epi32(a.1, c))
    }
    #[inline(always)]
    pub unsafe fn _mm512_srai_epi32(a: __m512i, k: u32) -> __m512i {
        let c = _mm_cvtsi32_si128(k as i32);
        __m512i(_mm256_sra_epi32(a.0, c), _mm256_sra_epi32(a.1, c))
    }
    #[inline(always)]
    pub unsafe fn _mm512_cvtepu8_epi32(a: __m128i) -> __m512i {
        __m512i(_mm256_cvtepu8_epi32(a), _mm256_cvtepu8_epi32(_mm_srli_si128::<8>(a)))
    }
    /// The low byte of every lane, for lanes in 0..=255 (the AVX-512 instruction truncates any value).
    #[inline(always)]
    pub unsafe fn _mm512_cvtepi32_epi8(a: __m512i) -> __m128i {
        let w = _mm256_permute4x64_epi64::<0xd8>(_mm256_packus_epi32(a.0, a.1));
        _mm_packus_epi16(_mm256_castsi256_si128(w), _mm256_extracti128_si256::<1>(w))
    }

    /// Index vectors that move the lanes of an 8-bit mask to the front, in order.
    const fn compress_lut() -> [[i32; 8]; 256] {
        let mut t = [[0i32; 8]; 256];
        let mut m = 0;
        while m < 256 {
            let mut k = 0;
            let mut i = 0;
            while i < 8 {
                if m >> i & 1 == 1 {
                    t[m][k] = i as i32;
                    k += 1;
                }
                i += 1;
            }
            m += 1;
        }
        t
    }
    static COMPRESS: [[i32; 8]; 256] = compress_lut();

    #[inline(always)]
    unsafe fn compress8(a: __m256i, m: u32) -> __m256i {
        let v = _mm256_permutevar8x32_epi32(a, _mm256_loadu_si256(COMPRESS[m as usize].as_ptr() as *const _));
        let keep = _mm256_cmpgt_epi32(_mm256_set1_epi32(m.count_ones() as i32), _mm256_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7));
        _mm256_and_si256(v, keep)
    }
    #[inline(always)]
    pub unsafe fn _mm512_maskz_compress_epi32(m: u16, a: __m512i) -> __m512i {
        let (lo, hi) = (m as u32 & 255, m as u32 >> 8);
        let mut buf = [0i32; 24];
        _mm256_storeu_si256(buf.as_mut_ptr() as *mut _, compress8(a.0, lo));
        _mm256_storeu_si256(buf.as_mut_ptr().add(lo.count_ones() as usize) as *mut _, compress8(a.1, hi));
        _mm512_loadu_si512(buf.as_ptr() as *const _)
    }
    #[inline(always)]
    pub unsafe fn _mm512_maskz_compress_ps(m: u16, a: __m512) -> __m512 {
        let r = _mm512_maskz_compress_epi32(m, __m512i(_mm256_castps_si256(a.0), _mm256_castps_si256(a.1)));
        __m512(_mm256_castsi256_ps(r.0), _mm256_castsi256_ps(r.1))
    }
}
