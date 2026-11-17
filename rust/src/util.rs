//! Small shared pieces: .npy reading, f16 helpers, exact similarity, a reproducible Haar rotation generator.
use half::f16;
use std::time::Instant;

pub struct Npy {
    pub shape: Vec<usize>,
    pub descr: String,
    pub data: Vec<u8>,
}

pub fn read_npy(path: &str) -> Npy {
    try_read_npy(path).unwrap_or_else(|e| panic!("{e}"))
}

/// An .npy file read whole, its header checked (magic, version, length, a C-ordered array with a type and a shape);
/// an error naming the file otherwise.
pub fn try_read_npy(path: &str) -> Result<Npy, String> {
    let buf = std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let bad = |what: &str| format!("{path}: not a readable .npy file ({what})");
    if buf.len() < 10 || &buf[..6] != b"\x93NUMPY" {
        return Err(bad("no .npy magic"));
    }
    let (hl, start): (usize, usize) = match buf[6] {
        1 => (u16::from_le_bytes([buf[8], buf[9]]) as usize, 10),
        2 | 3 if buf.len() >= 12 => (u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]) as usize, 12),
        _ => return Err(bad("unknown version")),
    };
    let off = start.checked_add(hl).filter(|&e| e <= buf.len()).ok_or_else(|| bad("header past the end"))?;
    let hdr = String::from_utf8_lossy(&buf[start..off]).to_string();
    if hdr.contains("'fortran_order': True") {
        return Err(bad("Fortran order"));
    }
    let descr = try_field(&hdr, "descr").ok_or_else(|| bad("no descr"))?;
    let shape = try_shape(&hdr).ok_or_else(|| bad("no shape"))?;
    Ok(Npy { shape, descr, data: buf[off..].to_vec() })
}

fn try_field(hdr: &str, key: &str) -> Option<String> {
    let i = hdr.find(&format!("'{key}'"))?;
    let rest = hdr.get(i + key.len() + 3..)?;
    let a = rest.find('\'')?;
    let b = rest[a + 1..].find('\'')?;
    Some(rest[a + 1..a + 1 + b].to_string())
}

fn try_shape(hdr: &str) -> Option<Vec<usize>> {
    let i = hdr.find("'shape'")?;
    let a = hdr[i..].find('(')? + i;
    let b = hdr[a..].find(')')? + a;
    Some(hdr[a + 1..b].split(',').filter_map(|s| s.trim().parse().ok()).collect())
}

/// The bytes of an .npy header (version 1) for C-ordered data of type `descr` and shape `shape`.
pub fn npy_header_bytes(descr: &str, shape: &[usize]) -> Vec<u8> {
    let dims: Vec<String> = shape.iter().map(|d| d.to_string()).collect();
    let tuple = if dims.len() == 1 { format!("({},)", dims[0]) } else { format!("({})", dims.join(", ")) };
    let mut hdr = format!("{{'descr': '{descr}', 'fortran_order': False, 'shape': {tuple}, }}");
    while (10 + hdr.len() + 1) % 64 != 0 {
        hdr.push(' ');
    }
    hdr.push('\n');
    let mut out = b"\x93NUMPY\x01\x00".to_vec();
    out.extend_from_slice(&(hdr.len() as u16).to_le_bytes());
    out.extend_from_slice(hdr.as_bytes());
    out
}

/// Write a little-endian .npy file (`descr` as numpy writes it, e.g. "<i4", "<f8").
pub fn write_npy(path: &str, descr: &str, shape: &[usize], data: &[u8]) {
    let mut out = npy_header_bytes(descr, shape);
    out.extend_from_slice(data);
    std::fs::write(path, out).unwrap_or_else(|e| panic!("{path}: {e}"));
}

pub fn npy_header(buf: &[u8]) -> (String, usize) {
    assert_eq!(&buf[..6], b"\x93NUMPY");
    let (hl, start) = if buf[6] == 1 {
        (u16::from_le_bytes([buf[8], buf[9]]) as usize, 10)
    } else {
        (u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]) as usize, 12)
    };
    (String::from_utf8_lossy(&buf[start..start + hl]).to_string(), start + hl)
}

pub fn npy_shape(hdr: &str) -> Vec<usize> {
    let i = hdr.find("'shape'").unwrap();
    let a = hdr[i..].find('(').unwrap() + i;
    let b = hdr[a..].find(')').unwrap() + a;
    hdr[a + 1..b].split(',').filter_map(|s| s.trim().parse().ok()).collect()
}

macro_rules! conv {
    ($name:ident, $t:ty, $sz:expr) => {
        pub fn $name(&self) -> Vec<$t> {
            self.data.chunks_exact($sz).map(|c| <$t>::from_le_bytes(c.try_into().unwrap())).collect()
        }
    };
}
impl Npy {
    conv!(f64s, f64, 8);
    conv!(f32s, f32, 4);
    conv!(i64s, i64, 8);
    conv!(i32s, i32, 4);
    conv!(u16s, u16, 2);
    conv!(u32s, u32, 4);
    /// A 2-D array of little-endian floats (f2, f4 or f8) as f32 values, row-major, with its row width; an error
    /// naming `what` (the file) on any other type or shape.
    pub fn rows_f32(&self, what: &str) -> Result<(Vec<f32>, usize), String> {
        if self.shape.len() != 2 {
            return Err(format!("{what}: expected a 2-D array (rows x dimensions), found shape {:?}", self.shape));
        }
        let v: Vec<f32> = match self.descr.as_str() {
            "<f8" => self.f64s().into_iter().map(|x| x as f32).collect(),
            "<f4" => self.f32s(),
            "<f2" => self.u16s().into_iter().map(f16_to_f32).collect(),
            d => return Err(format!("{what}: expected little-endian floats (<f2, <f4 or <f8), found {d}")),
        };
        Ok((v, self.shape[1]))
    }
    pub fn i8s(&self) -> Vec<i8> {
        self.data.iter().map(|&b| b as i8).collect()
    }
}

pub fn f16_to_f32(h: u16) -> f32 {
    f16::from_bits(h).to_f32()
}
pub fn f32_to_f16(x: f32) -> u16 {
    f16::from_f32(x).to_bits()
}

/// x * 2^24 as an exact integer, for an f16 with |x| <= 2 (holds for unit vectors).
#[inline]
pub fn f16_fixed(h: u16) -> i64 {
    let s = (h >> 15) & 1;
    let e = ((h >> 10) & 31) as i64;
    let m = (h & 1023) as i64;
    assert!(e < 31, "f16 inf/nan");
    let v = if e == 0 { m } else { (1024 + m) << (e - 1) };
    if s == 1 { -v } else { v }
}

/// The similarity, defined exactly: s(q,x) = 2^-48 * sum_i Q_i X_i with Q_i, X_i the f16 values times 2^24.
/// Returned as the exact integer numerator.
pub fn exact_dot(q16: &[u16], x16: &[u16]) -> i128 {
    #[cfg(target_feature = "avx512f")]
    {
        if let Some(v) = unsafe { exact_dot_512(q16, x16) } {
            return v;
        }
    }
    exact_dot_scalar(q16, x16)
}

fn exact_dot_scalar(q16: &[u16], x16: &[u16]) -> i128 {
    q16.iter().zip(x16).map(|(&a, &b)| f16_fixed(a) as i128 * f16_fixed(b) as i128).sum()
}

/// exact_dot sixteen elements at a time. A finite f16 times 2^24 is M 2^S, M = m (e = 0) or 1024 + m below 2^11 and
/// S = max(e - 1, 0), so a product is M_q M_x 2^(S_q + S_x) with M_q M_x below 2^22. Each product goes to the bucket
/// of its shift's high three bits, shifted by the low three (below 2^29); a bucket's i64 lanes hold at most n / 16
/// such terms, and the i128 total is the buckets' sums shifted by 8 a. None when an element is inf or NaN (the scalar
/// path refuses those).
#[cfg(target_feature = "avx512f")]
#[target_feature(enable = "avx512f,avx512bw,avx512dq,avx512vl")]
unsafe fn exact_dot_512(q16: &[u16], x16: &[u16]) -> Option<i128> {
    use std::arch::x86_64::*;
    let n = q16.len().min(x16.len());
    assert!(n <= 1 << 24, "exact_dot: {n} elements overflow the 64-bit buckets");
    let z = _mm512_setzero_si512();
    let mut acc = [[z; 2]; 8];
    let (f31, m1023, one, seven) = (_mm512_set1_epi32(31), _mm512_set1_epi32(1023), _mm512_set1_epi32(1), _mm512_set1_epi32(7));
    // (M, S, sign bit) of 16 f16 values; the mask of lanes that are inf or NaN
    let split = |h: __m512i| {
        let e = _mm512_and_si512(_mm512_srli_epi32(h, 10), f31);
        let m = _mm512_and_si512(h, m1023);
        let mm = _mm512_mask_or_epi32(m, _mm512_test_epi32_mask(e, e), m, _mm512_set1_epi32(1024));
        (mm, _mm512_max_epi32(_mm512_sub_epi32(e, one), z), _mm512_srli_epi32(h, 15), _mm512_cmpeq_epi32_mask(e, f31))
    };
    let mut i = 0;
    while i + 16 <= n {
        let (mq, sq, gq, bq) = split(_mm512_cvtepu16_epi32(_mm256_loadu_si256(q16.as_ptr().add(i) as *const _)));
        let (mx, sx, gx, bx) = split(_mm512_cvtepu16_epi32(_mm256_loadu_si256(x16.as_ptr().add(i) as *const _)));
        if bq | bx != 0 {
            return None;
        }
        let sh = _mm512_add_epi32(sq, sx);
        let mut v = _mm512_sllv_epi32(_mm512_mullo_epi32(mq, mx), _mm512_and_si512(sh, seven));
        v = _mm512_mask_sub_epi32(v, _mm512_test_epi32_mask(_mm512_xor_si512(gq, gx), one), z, v);
        let a = _mm512_srli_epi32(sh, 3);
        let (lo, hi) = (_mm512_cvtepi32_epi64(_mm512_castsi512_si256(v)), _mm512_cvtepi32_epi64(_mm512_extracti64x4_epi64(v, 1)));
        for (b, ab) in acc.iter_mut().enumerate() {
            let k = _mm512_cmpeq_epi32_mask(a, _mm512_set1_epi32(b as i32));
            if k != 0 {
                ab[0] = _mm512_mask_add_epi64(ab[0], k as u8, ab[0], lo);
                ab[1] = _mm512_mask_add_epi64(ab[1], (k >> 8) as u8, ab[1], hi);
            }
        }
        i += 16;
    }
    let mut r: i128 = 0;
    for (b, ab) in acc.iter().enumerate() {
        r += (_mm512_reduce_add_epi64(_mm512_add_epi64(ab[0], ab[1])) as i128) << (8 * b);
    }
    for j in i..n {
        if (q16[j] >> 10) & 31 == 31 || (x16[j] >> 10) & 31 == 31 {
            return None;
        }
        r += f16_fixed(q16[j]) as i128 * f16_fixed(x16[j]) as i128;
    }
    Some(r)
}
/// Decide s(q,x) >= t exactly.
pub fn exact_ge(num: i128, t: f64) -> bool {
    // s = num * 2^-48 >= t  <=>  num >= t * 2^48 (exact scaling), num integer so compare with ceil.
    num >= (t * 281474976710656.0).ceil() as i128
}
pub fn exact_to_f64(num: i128) -> f64 {
    num as f64 / 281474976710656.0
}

pub struct Timer(Instant);
impl Timer {
    pub fn new() -> Self {
        Timer(Instant::now())
    }
    pub fn s(&self) -> f64 {
        self.0.elapsed().as_secs_f64()
    }
    pub fn ms(&self) -> f64 {
        self.0.elapsed().as_secs_f64() * 1e3
    }
}

pub fn now_s() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64()
}

// ---------------- reproducible Haar rotations ----------------
// Only IEEE-754 +, -, *, / and sqrt are used (all correctly rounded), in a fixed order, so the rotation is bit-identical on
// any conforming machine. ln is computed here from those operations rather than taken from the platform's libm.

const LN2: f64 = 0.693_147_180_559_945_3;
fn det_ln(x: f64) -> f64 {
    assert!(x > 0.0);
    let bits = x.to_bits();
    let mut e = ((bits >> 52) & 0x7ff) as i64 - 1023;
    let mut m = f64::from_bits((bits & 0x000f_ffff_ffff_ffff) | 0x3ff0_0000_0000_0000); // [1,2)
    if m > 1.414_213_562_373_095 {
        m *= 0.5;
        e += 1;
    }
    let y = (m - 1.0) / (m + 1.0);
    let y2 = y * y;
    let mut term = y;
    let mut sum = 0.0;
    for k in 0..40 {
        sum += term / (2 * k + 1) as f64;
        term *= y2;
    }
    2.0 * sum + e as f64 * LN2
}

/// Standard normals from ChaCha20 by the Marsaglia polar method.
pub struct DetGauss {
    rng: rand_chacha::ChaCha20Rng,
    spare: Option<f64>,
}
impl DetGauss {
    pub fn new(seed: u64, stream: u64) -> Self {
        use rand::SeedableRng;
        let mut rng = rand_chacha::ChaCha20Rng::seed_from_u64(seed);
        rng.set_stream(stream);
        DetGauss { rng, spare: None }
    }
    fn unif(&mut self) -> f64 {
        use rand::RngCore;
        (self.rng.next_u64() >> 11) as f64 * (1.0 / 9007199254740992.0) * 2.0 - 1.0
    }
    pub fn next(&mut self) -> f64 {
        if let Some(s) = self.spare.take() {
            return s;
        }
        loop {
            let u = self.unif();
            let v = self.unif();
            let s = u * u + v * v;
            if s > 0.0 && s < 1.0 {
                let f = (-2.0 * det_ln(s) / s).sqrt();
                self.spare = Some(v * f);
                return u * f;
            }
        }
    }
}

/// Haar-distributed m x m rotation (row-major): Householder QR of a Gaussian matrix, Q multiplied by sign(diag R).
pub fn haar_rotation(m: usize, seed: u64, stream: u64) -> Vec<f64> {
    let mut g = DetGauss::new(seed, stream);
    let mut a: Vec<f64> = (0..m * m).map(|_| g.next()).collect(); // row-major A
    // Householder on columns; accumulate Q explicitly.
    let mut q = vec![0.0f64; m * m];
    for i in 0..m {
        q[i * m + i] = 1.0;
    }
    let mut rdiag = vec![0.0f64; m];
    for k in 0..m {
        let mut nrm = 0.0;
        for i in k..m {
            nrm += a[i * m + k] * a[i * m + k];
        }
        let nrm = nrm.sqrt();
        let akk = a[k * m + k];
        let alpha = if akk >= 0.0 { -nrm } else { nrm };
        let mut v = vec![0.0f64; m];
        for i in k..m {
            v[i] = a[i * m + k];
        }
        v[k] -= alpha;
        let mut vn = 0.0;
        for i in k..m {
            vn += v[i] * v[i];
        }
        if vn > 0.0 {
            for j in k..m {
                let mut d = 0.0;
                for i in k..m {
                    d += v[i] * a[i * m + j];
                }
                let f = 2.0 * d / vn;
                for i in k..m {
                    a[i * m + j] -= f * v[i];
                }
            }
            // Q <- Q H
            for r in 0..m {
                let mut d = 0.0;
                for i in k..m {
                    d += q[r * m + i] * v[i];
                }
                let f = 2.0 * d / vn;
                for i in k..m {
                    q[r * m + i] -= f * v[i];
                }
            }
        }
        rdiag[k] = a[k * m + k];
    }
    for r in 0..m {
        for c in 0..m {
            if rdiag[c] < 0.0 {
                q[r * m + c] = -q[r * m + c];
            }
        }
    }
    q
}

/// f64 dot product with eight independent partial sums (vectorises; a different but fixed summation order).
/// dot64 with `b` held as f32: each element widened exactly, the same operations in the same order, so the result
/// equals dot64 of the widened vector bit for bit.
pub fn dot64_32(a: &[f64], b: &[f32]) -> f64 {
    let n = a.len().min(b.len());
    let mut s = [0f64; 8];
    let mut i = 0;
    while i + 8 <= n {
        for k in 0..8 {
            s[k] += a[i + k] * b[i + k] as f64;
        }
        i += 8;
    }
    let mut r = (s[0] + s[1]) + (s[2] + s[3]) + ((s[4] + s[5]) + (s[6] + s[7]));
    while i < n {
        r += a[i] * b[i] as f64;
        i += 1;
    }
    r
}

pub fn dot64(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    let mut s = [0f64; 8];
    let mut i = 0;
    while i + 8 <= n {
        for k in 0..8 {
            s[k] += a[i + k] * b[i + k];
        }
        i += 8;
    }
    let mut r = (s[0] + s[1]) + (s[2] + s[3]) + ((s[4] + s[5]) + (s[6] + s[7]));
    while i < n {
        r += a[i] * b[i];
        i += 1;
    }
    r
}

pub fn quantile(v: &mut Vec<f64>, q: f64) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if v.is_empty() {
        return 0.0;
    }
    let pos = q * (v.len() - 1) as f64;
    let i = pos.floor() as usize;
    let fr = pos - i as f64;
    if i + 1 < v.len() { v[i] * (1.0 - fr) + v[i + 1] * fr } else { v[i] }
}

pub fn median(v: &[f64]) -> f64 {
    let mut w = v.to_vec();
    quantile(&mut w, 0.5)
}
pub fn mean(v: &[f64]) -> f64 {
    if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 }
}

extern "C" {
    fn sched_setaffinity(pid: i32, size: usize, mask: *const u64) -> i32;
}

/// Pin the calling thread to one logical CPU.
pub fn pin(cpu: usize) {
    let mut mask = [0u64; 16];
    mask[cpu / 64] = 1 << (cpu % 64);
    assert_eq!(unsafe { sched_setaffinity(0, std::mem::size_of_val(&mask), mask.as_ptr()) }, 0, "cannot pin to CPU {cpu}");
}

/// The process's anonymous resident memory in MB (`RssAnon`; 0 where /proc is absent): the memory a build holds that
/// the kernel cannot drop, unlike the mapped files it reads and writes.
pub fn anon_mb() -> f64 {
    std::fs::read_to_string("/proc/self/status").ok().and_then(|s| s.lines().find(|l| l.starts_with("RssAnon:")).and_then(|l| l.split_whitespace().nth(1)?.parse::<f64>().ok())).map_or(0.0, |kb| kb / 1024.0)
}

/// Fix glibc's mmap threshold at `bytes` for the rest of the process (no effect off glibc). Its default rises to 32 MB
/// as large blocks are freed; a build encodes cluster after cluster in buffers of a few MB, and above the risen
/// threshold glibc keeps the freed ones in its arenas, which grew a 4M-row build by 38 bytes a row. The setting is
/// process-wide and stays, so only a process that does nothing but build should set it (`BuildOptions::tune_malloc`).
pub fn fix_mmap_threshold(bytes: i32) {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        extern "C" {
            fn mallopt(param: i32, value: i32) -> i32;
        }
        const M_MMAP_THRESHOLD: i32 = -3;
        // SAFETY: mallopt only sets glibc's allocator parameters; it takes no pointers
        unsafe { mallopt(M_MMAP_THRESHOLD, bytes) };
    }
    let _ = bytes;
}

/// Tests: a new directory under the system's temporary directory, removed when dropped (also when a test panics).
#[cfg(test)]
pub struct TmpDir(pub std::path::PathBuf);
#[cfg(test)]
impl TmpDir {
    pub fn new(tag: &str) -> TmpDir {
        let d = std::env::temp_dir().join(format!("vexact-{tag}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&d).unwrap();
        TmpDir(d)
    }
}
#[cfg(test)]
impl std::ops::Deref for TmpDir {
    type Target = std::path::Path;
    fn deref(&self) -> &std::path::Path {
        &self.0
    }
}
#[cfg(test)]
impl AsRef<std::path::Path> for TmpDir {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}
#[cfg(test)]
impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}


#[cfg(test)]
mod exact_dot_tests {
    use super::*;
    use rand::{Rng, SeedableRng};

    /// Random finite f16 bit patterns (subnormals, zeros of both signs, the largest values) at lengths around the
    /// 16-element blocks: the vector path equals the scalar sum.
    #[test]
    fn exact_dot_equals_the_scalar_sum() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(5);
        let finite = |r: &mut rand_chacha::ChaCha8Rng| loop {
            let h: u16 = match r.gen_range(0..4) {
                0 => r.gen(),
                1 => r.gen::<u16>() & 0x83ff,
                2 => 0x7bff | (r.gen::<u16>() & 0x8000),
                _ => r.gen::<u16>() & 0x8000,
            };
            if (h >> 10) & 31 != 31 {
                return h;
            }
        };
        for n in [0, 1, 15, 16, 17, 64, 767, 768, 1024, 4096] {
            for _ in 0..20 {
                let q: Vec<u16> = (0..n).map(|_| finite(&mut rng)).collect();
                let x: Vec<u16> = (0..n).map(|_| finite(&mut rng)).collect();
                assert_eq!(exact_dot(&q, &x), exact_dot_scalar(&q, &x), "n {n}");
            }
        }
        // every element at the largest magnitude, all signs alike: the largest bucket sums
        let big = vec![0x7bffu16; 4096];
        assert_eq!(exact_dot(&big, &big), exact_dot_scalar(&big, &big));
    }

    #[test]
    #[should_panic(expected = "inf/nan")]
    fn exact_dot_refuses_inf() {
        let mut q = vec![0x3c00u16; 32];
        q[20] = 0x7c00;
        exact_dot(&q, &vec![0x3c00u16; 32]);
    }
}
