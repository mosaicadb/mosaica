// The per-query work each cluster order adds, alone, on random data at a given shape: q.m of every cluster (as
// today's dot64_32, and as an f16 GEMV), H's projections at rank 2, 4 and 16 (f16 and int8 directions), H r16 for a
// few candidate clusters only, a shared basis, and the rad-aware model's tau. PLAN.md has the figures.
// usage: RUSTFLAGS="-C target-cpu=native" cargo build --release && taskset -c 3 target/release/mb K D [warm|cold]
use std::arch::x86_64::*;
use std::hint::black_box;
use std::time::Instant;

fn rng(s: &mut u64) -> f32 { *s ^= *s << 13; *s ^= *s >> 7; *s ^= *s << 17; ((*s >> 40) as f32 / (1u64 << 24) as f32) - 0.5 }

// current: f64 query x f32 centre, scalar per cluster (as dot64_32)
fn qm64(q: &[f64], m: &[f32], d: usize, out: &mut [f64]) {
    for (c, o) in out.iter_mut().enumerate() {
        let r = &m[c * d..(c + 1) * d];
        let mut s = [0f64; 8];
        let mut i = 0;
        while i + 8 <= d { for k in 0..8 { s[k] += q[i + k] * r[i + k] as f64; } i += 8; }
        *o = (s[0] + s[1]) + (s[2] + s[3]) + ((s[4] + s[5]) + (s[6] + s[7]));
    }
}
// f32 query x f16 rows, AVX-512, 4 rows at a time (one GEMV over a [rows x d] f16 matrix)
#[target_feature(enable = "avx512f,f16c")]
unsafe fn gemv_f16(q: &[f32], m: &[u16], d: usize, out: &mut [f32]) {
    let n = out.len();
    let mut c = 0;
    while c + 4 <= n {
        let mut a = [_mm512_setzero_ps(); 4];
        let mut i = 0;
        while i < d {
            let qv = _mm512_loadu_ps(q.as_ptr().add(i));
            for j in 0..4 {
                let h = _mm256_loadu_si256(m.as_ptr().add((c + j) * d + i) as *const __m256i);
                a[j] = _mm512_fmadd_ps(qv, _mm512_cvtph_ps(h), a[j]);
            }
            i += 16;
        }
        for j in 0..4 { out[c + j] = _mm512_reduce_add_ps(a[j]); }
        c += 4;
    }
}
// f32 query x int8 rows (per-row scale applied after), AVX-512
#[target_feature(enable = "avx512f,avx512bw")]
unsafe fn gemv_i8(q: &[f32], m: &[i8], d: usize, out: &mut [f32]) {
    let n = out.len();
    let mut c = 0;
    while c + 4 <= n {
        let mut a = [_mm512_setzero_ps(); 4];
        let mut i = 0;
        while i < d {
            let qv = _mm512_loadu_ps(q.as_ptr().add(i));
            for j in 0..4 {
                let b = _mm_loadu_si128(m.as_ptr().add((c + j) * d + i) as *const __m128i);
                a[j] = _mm512_fmadd_ps(qv, _mm512_cvtepi32_ps(_mm512_cvtepi8_epi32(b)), a[j]);
            }
            i += 16;
        }
        for j in 0..4 { out[c + j] = _mm512_reduce_add_ps(a[j]); }
        c += 4;
    }
}
// the H key from projections: sum_j lam_j p_j^2 + s^2 (|q|^2 - sum p_j^2), sqrt, plus mu and rad terms
fn hkey(mu: &[f32], p: &[f32], lam: &[f32], s2: &[f32], z: &[f32], radw: &[f32], r: usize, qq: f32, out: &mut [f32]) {
    for c in 0..mu.len() {
        let (mut v, mut pp) = (0f32, 0f32);
        for j in 0..r { let x = p[c * r + j]; v += lam[c * r + j] * x * x; pp += x * x; }
        v += s2[c] * (qq - pp);
        out[c] = mu[c] + z[c] * v.max(0.0).sqrt() + radw[c];
    }
}

static mut COLD: bool = false;
fn time<F: FnMut()>(name: &str, reps: usize, mut f: F) {
    let cold = unsafe { COLD };
    let reps = if cold { reps.min(100) } else { reps };
    let mut junk = vec![1u8; if cold { 64 << 20 } else { 0 }];
    for _ in 0..reps / 10 + 3 { f(); }
    let mut v = Vec::with_capacity(reps);
    for _ in 0..reps {
        if cold { for i in (0..junk.len()).step_by(64) { junk[i] = junk[i].wrapping_add(1); } black_box(&junk); }
        let t = Instant::now(); f(); v.push(t.elapsed().as_nanos() as f64 / 1e3); }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!("{:<44} median {:8.1} us   p90 {:8.1} us", name, v[reps / 2], v[reps * 9 / 10]);
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (k, d): (usize, usize) = (a[1].parse().unwrap(), a[2].parse().unwrap());
    let cold = a.get(3).map_or(false, |s| s == "cold");
    let mut s = 0x1234_5678_9abc_def1u64;
    let q32: Vec<f32> = (0..d).map(|_| rng(&mut s)).collect();
    let q64: Vec<f64> = q32.iter().map(|&x| x as f64).collect();
    let cen: Vec<f32> = (0..k * d).map(|_| rng(&mut s)).collect();
    let cen16: Vec<u16> = cen.iter().map(|&x| f32_to_f16(x)).collect();
    let mut o64 = vec![0f64; k];
    let mut o32 = vec![0f32; k];
    // a buffer bigger than L2+L3 share, walked between reps in "cold" mode to evict the per-cluster state
    unsafe { COLD = cold; }
    println!("K {k}  d {d}  {}", if cold { "cold (state evicted before each rep)" } else { "warm" });
    time("q.m, f64 x f32 (as today)", 2000, || { qm64(&q64, black_box(&cen), d, &mut o64); black_box(&o64); });
    time("q.m, f32 x f16 AVX-512", 2000, || { unsafe { gemv_f16(&q32, black_box(&cen16), d, &mut o32) }; black_box(&o32); });
    for r in [2usize, 4, 16] {
        let u16m: Vec<u16> = (0..k * r * d).map(|_| f32_to_f16(rng(&mut s))).collect();
        let u8m: Vec<i8> = (0..k * r * d).map(|_| (rng(&mut s) * 250.0) as i8).collect();
        let lam: Vec<f32> = (0..k * r).map(|_| rng(&mut s).abs()).collect();
        let (mu, s2, z, rw) = (vec![0.1f32; k], vec![0.01f32; k], vec![2.3f32; k], vec![0.05f32; k]);
        let mut p = vec![0f32; k * r];
        let mut key = vec![0f32; k];
        time(&format!("H r{r}: projections f16 ({} KB)", k * r * d * 2 >> 10), 1000, || {
            unsafe { gemv_f16(&q32, black_box(&u16m), d, &mut p) };
            hkey(&mu, &p, &lam, &s2, &z, &rw, r, 1.0, &mut key); black_box(&key); });
        time(&format!("H r{r}: projections int8 ({} KB)", k * r * d >> 10), 1000, || {
            unsafe { gemv_i8(&q32, black_box(&u8m), d, &mut p) };
            hkey(&mu, &p, &lam, &s2, &z, &rw, r, 1.0, &mut key); black_box(&key); });
    }
    for m in [64usize, 128] {
        let r = 16;
        let u16m: Vec<u16> = (0..m * r * d).map(|_| f32_to_f16(rng(&mut s))).collect();
        let mut p = vec![0f32; m * r];
        time(&format!("H r16 projections for {m} candidates only"), 1000, || {
            unsafe { gemv_f16(&q32, black_box(&u16m), d, &mut p) }; black_box(&p); });
    }
    // shared basis: R global directions (one small GEMV), then per cluster a K x R weighted sum
    for big_r in [32usize, 64] {
        let b16: Vec<u16> = (0..big_r * d).map(|_| f32_to_f16(rng(&mut s))).collect();
        let w: Vec<f32> = (0..k * big_r).map(|_| rng(&mut s).abs()).collect();
        let mut p = vec![0f32; big_r];
        let mut key = vec![0f32; k];
        time(&format!("shared basis R{big_r}"), 2000, || {
            unsafe { gemv_f16(&q32, black_box(&b16), d, &mut p) };
            for c in 0..k { let mut v = 0f32; for j in 0..big_r { v += w[c * big_r + j] * p[j] * p[j]; } key[c] = v.sqrt(); }
            black_box(&key); });
    }
    // tau by bisection on a table-driven exceedance model (40 steps x K lookups), then K probabilities
    let tab: Vec<f32> = (0..65).map(|i| (-(i as f32) * 0.2).exp()).collect();
    let (mu, sg, lnn): (Vec<f32>, Vec<f32>, Vec<f32>) = ((0..k).map(|_| rng(&mut s) * 0.2).collect(), vec![0.05; k], vec![1000.0; k]);
    let isg: Vec<f32> = sg.iter().map(|x| 1.0 / x).collect();
    let mut out = vec![0f32; k];
    time("rad-model: tau bisection + K probabilities", 2000, || {
        let look = |zz: f32| { let x = ((zz + 2.0) * 8.0).clamp(0.0, 63.999); let i = x as usize; tab[i] + (tab[i + 1] - tab[i]) * (x - i as f32) };
        let (mut lo, mut hi) = (-1f32, 1f32);
        for _ in 0..40 { let t = 0.5 * (lo + hi); let e: f32 = (0..k).map(|c| lnn[c] * look((t - mu[c]) * isg[c])).sum(); if e > 10.0 { lo = t } else { hi = t } }
        for c in 0..k { out[c] = look((lo - mu[c]) * isg[c]); }
        black_box(&out); });
}

fn f32_to_f16(x: f32) -> u16 { unsafe { let v = _mm_cvtps_ph(_mm_set1_ps(x), 0); _mm_extract_epi16(v, 0) as u16 } }
