//! A query's statistics, and the f16 and int8 dot products of full-vector scoring (AVX-512, or AVX2 without it).
use std::arch::x86_64::*;

#[derive(Clone, Default, Debug)]
pub struct Stats {
    pub enter: Vec<u64>, // items entering each level (index nl = full vector)
    pub dec_in: Vec<u64>,
    pub dec_out: Vec<u64>,
    pub blocks: Vec<u64>, // blocks processed per level
    pub clusters_open: u64,
    pub clusters_in: u64,
    pub clusters_out: u64,
    pub clusters_novis: u64,
    pub range_in: u64,
    pub range_out: u64,
    pub touched: u64,
    pub full_reads: u64,
    pub exact_fallbacks: u64,
    pub groups: u64,
    pub viol_det: u64,
    pub viol_rand: u64,
    pub checked: u64,
    pub max_sd: f64,
    pub imp_checked: u64, // deterministic decisions checked against the f64 certificate (verify)
    pub imp_fail: u64,    // of which the f64 certificate does not imply the decision
    pub cert_viol: u64,   // lanes whose exact score lies outside the f64 certificate
    pub cyc: [u64; 32], // time-stamp-counter cycles by phase (v2 breakdown), then counts (v2::q::C_*)
    pub lcyc: Vec<u64>, // time-stamp-counter cycles of each level's blocks, tables not included
    pub errs: Vec<(u8, f32)>, // (level, |true - estimate| / half-width), when v2::q::ERRS is set
}
impl Stats {
    pub fn new(nl: usize) -> Stats {
        Stats { lcyc: vec![0; nl + 1], enter: vec![0; nl + 1], dec_in: vec![0; nl + 1], dec_out: vec![0; nl + 1], blocks: vec![0; nl + 1], ..Default::default() }
    }
    pub fn add(&mut self, o: &Stats) {
        self.errs.extend_from_slice(&o.errs);
        for (a, b) in self.enter.iter_mut().zip(&o.enter) {
            *a += b;
        }
        for (a, b) in self.dec_in.iter_mut().zip(&o.dec_in) {
            *a += b;
        }
        for (a, b) in self.dec_out.iter_mut().zip(&o.dec_out) {
            *a += b;
        }
        for (a, b) in self.blocks.iter_mut().zip(&o.blocks) {
            *a += b;
        }
        self.clusters_open += o.clusters_open;
        self.clusters_in += o.clusters_in;
        self.clusters_out += o.clusters_out;
        self.clusters_novis += o.clusters_novis;
        self.range_in += o.range_in;
        self.range_out += o.range_out;
        self.touched += o.touched;
        self.full_reads += o.full_reads;
        self.exact_fallbacks += o.exact_fallbacks;
        self.groups += o.groups;
        self.viol_det += o.viol_det;
        self.viol_rand += o.viol_rand;
        self.checked += o.checked;
        self.max_sd = self.max_sd.max(o.max_sd);
        self.imp_checked += o.imp_checked;
        self.imp_fail += o.imp_fail;
        self.cert_viol += o.cert_viol;
        for k in 0..self.cyc.len() {
            self.cyc[k] += o.cyc[k];
        }
        for (a, b) in self.lcyc.iter_mut().zip(&o.lcyc) {
            *a += b;
        }
    }
}

// ---------------- flat scans ----------------

/// f16 x f32 dot, 16 lanes, f32 accumulation. |error| <= (n/16 + 4) * 2^-24 * sum |q_i x_i|.
#[cfg(target_feature = "avx512f")]
#[target_feature(enable = "avx512f,avx512bw,avx512dq,avx512vl")]
pub unsafe fn dot_f16(q: &[f32], x: &[u16]) -> f32 {
    let n = q.len();
    let mut s0 = _mm512_setzero_ps();
    let mut s1 = _mm512_setzero_ps();
    let mut i = 0;
    while i + 32 <= n {
        let x0 = _mm512_cvtph_ps(_mm256_loadu_si256(x.as_ptr().add(i) as *const _));
        let x1 = _mm512_cvtph_ps(_mm256_loadu_si256(x.as_ptr().add(i + 16) as *const _));
        s0 = _mm512_fmadd_ps(x0, _mm512_loadu_ps(q.as_ptr().add(i)), s0);
        s1 = _mm512_fmadd_ps(x1, _mm512_loadu_ps(q.as_ptr().add(i + 16)), s1);
        i += 32;
    }
    while i + 16 <= n {
        let x0 = _mm512_cvtph_ps(_mm256_loadu_si256(x.as_ptr().add(i) as *const _));
        s0 = _mm512_fmadd_ps(x0, _mm512_loadu_ps(q.as_ptr().add(i)), s0);
        i += 16;
    }
    let mut r = _mm512_reduce_add_ps(_mm512_add_ps(s0, s1));
    while i < n {
        r += crate::util::f16_to_f32(x[i]) * q[i];
        i += 1;
    }
    r
}

/// Four rows at a time (more loads in flight).
#[cfg(target_feature = "avx512f")]
#[target_feature(enable = "avx512f,avx512bw,avx512dq,avx512vl")]
pub unsafe fn dot_f16_x4(q: &[f32], x: [&[u16]; 4]) -> [f32; 4] {
    let n = q.len();
    let mut s = [_mm512_setzero_ps(); 4];
    let mut i = 0;
    while i + 16 <= n {
        let qv = _mm512_loadu_ps(q.as_ptr().add(i));
        for r in 0..4 {
            let xv = _mm512_cvtph_ps(_mm256_loadu_si256(x[r].as_ptr().add(i) as *const _));
            s[r] = _mm512_fmadd_ps(xv, qv, s[r]);
        }
        i += 16;
    }
    let mut r = [_mm512_reduce_add_ps(s[0]), _mm512_reduce_add_ps(s[1]), _mm512_reduce_add_ps(s[2]), _mm512_reduce_add_ps(s[3])];
    // the dimensions past the last multiple of 16, as dot_f16 takes them
    while i < n {
        for k in 0..4 {
            r[k] += crate::util::f16_to_f32(x[k][i]) * q[i];
        }
        i += 1;
    }
    r
}

#[cfg(not(target_feature = "avx512f"))]
#[inline(always)]
unsafe fn hsum_ps(v: __m256) -> f32 {
    let x = _mm_add_ps(_mm256_castps256_ps128(v), _mm256_extractf128_ps::<1>(v));
    let x = _mm_add_ps(x, _mm_movehl_ps(x, x));
    _mm_cvtss_f32(_mm_add_ss(x, _mm_movehdup_ps(x)))
}

/// f16 x f32 dot, four 8-lane accumulators (32 lanes, as the AVX-512 kernel), f32 accumulation.
/// |error| <= (n/16 + 4) * 2^-24 * sum |q_i x_i|.
#[cfg(not(target_feature = "avx512f"))]
#[target_feature(enable = "avx2,fma,f16c")]
pub unsafe fn dot_f16(q: &[f32], x: &[u16]) -> f32 {
    let n = q.len();
    let mut s = [_mm256_setzero_ps(); 4];
    let mut i = 0;
    while i + 32 <= n {
        for k in 0..4 {
            let xv = _mm256_cvtph_ps(_mm_loadu_si128(x.as_ptr().add(i + 8 * k) as *const _));
            s[k] = _mm256_fmadd_ps(xv, _mm256_loadu_ps(q.as_ptr().add(i + 8 * k)), s[k]);
        }
        i += 32;
    }
    while i + 16 <= n {
        for k in 0..2 {
            let xv = _mm256_cvtph_ps(_mm_loadu_si128(x.as_ptr().add(i + 8 * k) as *const _));
            s[k] = _mm256_fmadd_ps(xv, _mm256_loadu_ps(q.as_ptr().add(i + 8 * k)), s[k]);
        }
        i += 16;
    }
    let mut r = hsum_ps(_mm256_add_ps(_mm256_add_ps(s[0], s[2]), _mm256_add_ps(s[1], s[3])));
    while i < n {
        r += crate::util::f16_to_f32(x[i]) * q[i];
        i += 1;
    }
    r
}

/// Four rows at a time (more loads in flight); n a multiple of 16, two 8-lane accumulators per row.
#[cfg(not(target_feature = "avx512f"))]
#[target_feature(enable = "avx2,fma,f16c")]
pub unsafe fn dot_f16_x4(q: &[f32], x: [&[u16]; 4]) -> [f32; 4] {
    let n = q.len();
    let mut s = [[_mm256_setzero_ps(); 2]; 4];
    let mut i = 0;
    while i + 16 <= n {
        let q0 = _mm256_loadu_ps(q.as_ptr().add(i));
        let q1 = _mm256_loadu_ps(q.as_ptr().add(i + 8));
        for r in 0..4 {
            let x0 = _mm256_cvtph_ps(_mm_loadu_si128(x[r].as_ptr().add(i) as *const _));
            let x1 = _mm256_cvtph_ps(_mm_loadu_si128(x[r].as_ptr().add(i + 8) as *const _));
            s[r][0] = _mm256_fmadd_ps(x0, q0, s[r][0]);
            s[r][1] = _mm256_fmadd_ps(x1, q1, s[r][1]);
        }
        i += 16;
    }
    let mut r: [f32; 4] = core::array::from_fn(|r| hsum_ps(_mm256_add_ps(s[r][0], s[r][1])));
    // the dimensions past the last multiple of 16, as dot_f16 takes them
    while i < n {
        for k in 0..4 {
            r[k] += crate::util::f16_to_f32(x[k][i]) * q[i];
        }
        i += 1;
    }
    r
}

/// int8 x int16 dot via vpmaddwd.
#[cfg(not(target_feature = "avx512f"))]
#[target_feature(enable = "avx2,fma")]
pub unsafe fn dot_i8(q: &[i16], x: &[i8]) -> i32 {
    let n = q.len();
    let mut s = _mm256_setzero_si256();
    let mut i = 0;
    while i + 16 <= n {
        let xv = _mm256_cvtepi8_epi16(_mm_loadu_si128(x.as_ptr().add(i) as *const _));
        let qv = _mm256_loadu_si256(q.as_ptr().add(i) as *const _);
        s = _mm256_add_epi32(s, _mm256_madd_epi16(xv, qv));
        i += 16;
    }
    let v = _mm_add_epi32(_mm256_castsi256_si128(s), _mm256_extracti128_si256::<1>(s));
    let v = _mm_add_epi32(v, _mm_shuffle_epi32::<0x4e>(v));
    let v = _mm_add_epi32(v, _mm_shuffle_epi32::<0xb1>(v));
    _mm_cvtsi128_si32(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::*;

    /// dot_f16 and dot_f16_x4 against the exact product, within the bound the scans use: (n/16 + 6) u |q| |x|.
    #[test]
    fn f16_dots_within_their_bound() {
        let mut z = 12345u64;
        let mut rnd = || {
            z = z.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((z >> 33) as f64 / (1u64 << 31) as f64 * 2.0 - 1.0) as f32
        };
        for n in [32usize, 96, 128, 256, 768, 2048] {
            let q16: Vec<u16> = (0..n).map(|_| f32_to_f16(rnd())).collect();
            let q32: Vec<f32> = q16.iter().map(|&h| f16_to_f32(h)).collect();
            let qn = q32.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt();
            let xs: Vec<Vec<u16>> = (0..8).map(|i| (0..n).map(|_| f32_to_f16(rnd() * if i % 2 == 0 { 1.0 } else { 1e-3 })).collect()).collect();
            let bound = |x: &[u16]| {
                let xn = x.iter().map(|&h| f16_to_f32(h) as f64 * f16_to_f32(h) as f64).sum::<f64>().sqrt();
                (n / 16 + 6) as f64 * (1.0 / 16777216.0) * qn * xn
            };
            for x in &xs {
                let e = exact_to_f64(exact_dot(&q16, x));
                let d = unsafe { dot_f16(&q32, x) } as f64;
                assert!((d - e).abs() <= bound(x), "n {n}: dot_f16 off by {} > {}", (d - e).abs(), bound(x));
            }
            for c in xs.chunks(4) {
                let d4 = unsafe { dot_f16_x4(&q32, [&c[0], &c[1], &c[2], &c[3]]) };
                for k in 0..4 {
                    let e = exact_to_f64(exact_dot(&q16, &c[k]));
                    assert!((d4[k] as f64 - e).abs() <= bound(&c[k]), "n {n}: dot_f16_x4 lane {k} off by {}", (d4[k] as f64 - e).abs());
                }
            }
        }
    }
}

/// out[r] = sum_i q_i x[r n + i] over the int8 rows of x, in f32, four rows at a time (each row's products in 16 lanes,
/// then its tail in order).
#[cfg(target_feature = "avx512f")]
#[target_feature(enable = "avx512f,avx512bw")]
pub unsafe fn gemv_i8(q: &[f32], x: &[i8], n: usize, out: &mut [f32]) {
    let rows = out.len();
    assert!(q.len() >= n && x.len() >= rows * n);
    let body = n - n % 16;
    let tail = |r: usize| (body..n).map(|i| q[i] * x[r * n + i] as f32).sum::<f32>();
    let mut r = 0;
    while r + 4 <= rows {
        let mut s = [_mm512_setzero_ps(); 4];
        let mut i = 0;
        while i < body {
            let qv = _mm512_loadu_ps(q.as_ptr().add(i));
            for (j, sj) in s.iter_mut().enumerate() {
                let b = _mm_loadu_si128(x.as_ptr().add((r + j) * n + i) as *const __m128i);
                *sj = _mm512_fmadd_ps(qv, _mm512_cvtepi32_ps(_mm512_cvtepi8_epi32(b)), *sj);
            }
            i += 16;
        }
        for (j, sj) in s.iter().enumerate() {
            out[r + j] = _mm512_reduce_add_ps(*sj) + tail(r + j);
        }
        r += 4;
    }
    while r < rows {
        let mut s = _mm512_setzero_ps();
        let mut i = 0;
        while i < body {
            let b = _mm_loadu_si128(x.as_ptr().add(r * n + i) as *const __m128i);
            s = _mm512_fmadd_ps(_mm512_loadu_ps(q.as_ptr().add(i)), _mm512_cvtepi32_ps(_mm512_cvtepi8_epi32(b)), s);
            i += 16;
        }
        out[r] = _mm512_reduce_add_ps(s) + tail(r);
        r += 1;
    }
}

/// As the AVX-512 version, with two 8-lane sums per row.
#[cfg(not(target_feature = "avx512f"))]
#[target_feature(enable = "avx2,fma")]
pub unsafe fn gemv_i8(q: &[f32], x: &[i8], n: usize, out: &mut [f32]) {
    let rows = out.len();
    assert!(q.len() >= n && x.len() >= rows * n);
    let body = n - n % 16;
    for (r, o) in out.iter_mut().enumerate() {
        let (mut s0, mut s1) = (_mm256_setzero_ps(), _mm256_setzero_ps());
        let mut i = 0;
        while i < body {
            let b = _mm_loadu_si128(x.as_ptr().add(r * n + i) as *const __m128i);
            let lo = _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(b));
            let hi = _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(_mm_srli_si128::<8>(b)));
            s0 = _mm256_fmadd_ps(_mm256_loadu_ps(q.as_ptr().add(i)), lo, s0);
            s1 = _mm256_fmadd_ps(_mm256_loadu_ps(q.as_ptr().add(i + 8)), hi, s1);
            i += 16;
        }
        *o = hsum_ps(_mm256_add_ps(s0, s1)) + (body..n).map(|i| q[i] * x[r * n + i] as f32).sum::<f32>();
    }
}

#[cfg(test)]
mod gemv_tests {
    use super::*;
    use rand::{Rng, SeedableRng};

    #[test]
    fn gemv_i8_matches_the_f64_products() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(3);
        for (rows, n) in [(1, 16), (3, 32), (4, 64), (9, 100), (13, 512)] {
            let q: Vec<f32> = (0..n).map(|_| rng.gen_range(-1.0f32..1.0)).collect();
            let x: Vec<i8> = (0..rows * n).map(|_| rng.gen_range(-127i8..=127)).collect();
            let mut out = vec![0f32; rows];
            unsafe { gemv_i8(&q, &x, n, &mut out) };
            for r in 0..rows {
                let want: f64 = (0..n).map(|i| q[i] as f64 * x[r * n + i] as f64).sum();
                let bound: f64 = (0..n).map(|i| (q[i] as f64 * x[r * n + i] as f64).abs()).sum::<f64>() * 1e-6;
                assert!((out[r] as f64 - want).abs() <= bound, "rows {rows} n {n} row {r}: {} against {want}", out[r]);
            }
        }
    }
}
