//! Per-query setup kernels: the exact int8 basis products of several groups, and a microbenchmark of their variants.
use crate::simd::*;

#[inline(always)]
unsafe fn hsum_epi32(v: __m256i) -> i32 {
    let x = _mm_add_epi32(_mm256_castsi256_si128(v), _mm256_extracti128_si256::<1>(v));
    let x = _mm_add_epi32(x, _mm_shuffle_epi32::<0x4e>(x));
    let x = _mm_add_epi32(x, _mm_shuffle_epi32::<0xb1>(x));
    _mm_cvtsi128_si32(x)
}

/// V_q^T q exactly, as q::qv_exact (int8 basis rows times the query's 13-bit halves, i32 sums, one rounding to f64), R
/// basis rows at a time sharing the query loads, the rows of the next block prefetched `PFD` bytes ahead.
#[target_feature(enable = "avx2,fma")]
pub unsafe fn qv_exact_r<const R: usize, const PFD: usize>(v8t: &[i8], vsc: &[f64], n: usize, qhi: &[i16], qlo: &[i16], out: &mut [f64]) {
    assert!(n % 32 == 0 && n % R == 0 && v8t.len() >= n * n);
    assert!(n <= 2048, "the exact basis product sums in i32 and overflows past n = 2048; this index has n = {n}");
    let base = v8t.as_ptr();
    let end = base.add(n * n);
    let mut c = 0;
    while c < n {
        let mut ah = [_mm256_setzero_si256(); R];
        let mut al = [_mm256_setzero_si256(); R];
        let rows = base.add(c * n);
        let mut i = 0;
        while i < n {
            if PFD > 0 && i % 64 == 0 {
                // the block's bytes PFD ahead in the contiguous stream of rows
                for r in 0..R {
                    let p = if PFD == 1 { rows.add((R + r) * n + i) } else { rows.add(r * n + i + PFD) };
                    if p < end {
                        _mm_prefetch::<_MM_HINT_T0>(p as *const i8);
                    }
                }
            }
            let h = _mm256_loadu_si256(qhi.as_ptr().add(i) as *const _);
            let l = _mm256_loadu_si256(qlo.as_ptr().add(i) as *const _);
            for r in 0..R {
                let x = _mm256_cvtepi8_epi16(_mm_loadu_si128(rows.add(r * n + i) as *const _));
                ah[r] = _mm256_add_epi32(ah[r], _mm256_madd_epi16(x, h));
                al[r] = _mm256_add_epi32(al[r], _mm256_madd_epi16(x, l));
            }
            i += 16;
        }
        for r in 0..R {
            let h = hsum_epi32(ah[r]) as i64;
            let l = hsum_epi32(al[r]) as i64;
            *out.get_unchecked_mut(c + r) = vsc[c + r] * ((h * 8192 + l) as f64) * (1.0 / 16777216.0);
        }
        c += R;
    }
}

/// Times the basis-product kernels on `g` random n x n int8 bases (cold: more bytes than the last-level cache).
pub fn qvbench(n: usize, g: usize, reps: usize) {
    use crate::util::Timer;
    let mut s = 0x9E3779B97F4A7C15u64;
    let mut rnd = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    let bases: Vec<Vec<i8>> = (0..g).map(|_| (0..n * n).map(|_| (rnd() % 255) as i64 as i8).map(|x| x.max(-127)).collect()).collect();
    let vsc = vec![1.0f64; n];
    let qhi: Vec<i16> = (0..n).map(|_| ((rnd() % 4096) as i64 - 2048) as i16).collect();
    let qlo: Vec<i16> = (0..n).map(|_| (rnd() % 8192) as i16).collect();
    let mut out = vec![0f64; n];
    let mut refo = vec![vec![0f64; n]; g];
    for (gi, b) in bases.iter().enumerate() {
        unsafe { qv_exact_r::<4, 0>(b, &vsc, n, &qhi, &qlo, &mut refo[gi]) };
    }
    let run = |name: &str, f: &dyn Fn(&[i8], &mut [f64])| {
        let mut ts = Vec::new();
        let mut o = vec![0f64; n];
        for _ in 0..reps {
            let t = Timer::new();
            for b in &bases {
                f(b, &mut o);
                std::hint::black_box(&o);
            }
            ts.push(t.ms());
        }
        for (gi, b) in bases.iter().enumerate() {
            f(b, &mut o);
            assert_eq!(o, refo[gi], "{name} differs from qv_exact");
        }
        ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!("qvbench {name}: {:.3} ms for {g} groups ({:.1} MB)", ts[ts.len() / 2], (g * n * n) as f64 / 1e6);
    };
    let _ = &mut out;
    run("q::qv_exact", &|b, o| unsafe { crate::v2::q::qv_exact(b, &vsc, n, &qhi, &qlo, o) });
    run("r4 (the earlier kernel)", &|b, o| unsafe { qv_exact_r::<4, 0>(b, &vsc, n, &qhi, &qlo, o) });
    run("r4 pf512", &|b, o| unsafe { qv_exact_r::<4, 512>(b, &vsc, n, &qhi, &qlo, o) });
    run("r4 pf1024", &|b, o| unsafe { qv_exact_r::<4, 1024>(b, &vsc, n, &qhi, &qlo, o) });
    run("r8", &|b, o| unsafe { qv_exact_r::<8, 0>(b, &vsc, n, &qhi, &qlo, o) });
    run("r8 pf512", &|b, o| unsafe { qv_exact_r::<8, 512>(b, &vsc, n, &qhi, &qlo, o) });
    run("r8 pf2048", &|b, o| unsafe { qv_exact_r::<8, 2048>(b, &vsc, n, &qhi, &qlo, o) });
    run("r6 pf1024", &|b, o| unsafe { qv_exact_r::<6, 1024>(b, &vsc, n, &qhi, &qlo, o) });
    run("r4 next", &|b, o| unsafe { qv_exact_r::<4, 1>(b, &vsc, n, &qhi, &qlo, o) });
    run("r8 next", &|b, o| unsafe { qv_exact_r::<8, 1>(b, &vsc, n, &qhi, &qlo, o) });
    run("r2", &|b, o| unsafe { qv_exact_r::<2, 0>(b, &vsc, n, &qhi, &qlo, o) });
}
