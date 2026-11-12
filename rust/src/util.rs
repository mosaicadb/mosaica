//! Small shared pieces: .npy reading, f16 helpers, exact similarity, a reproducible Haar rotation generator.
use half::f16;
use std::fs::File;
use std::io::Read;
use std::time::Instant;

pub struct Npy {
    pub shape: Vec<usize>,
    pub descr: String,
    pub data: Vec<u8>,
}

pub fn read_npy(path: &str) -> Npy {
    let mut f = File::open(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).unwrap();
    let (hdr, off) = npy_header(&buf);
    let descr = field(&hdr, "descr");
    assert!(!hdr.contains("'fortran_order': True"), "{path}: fortran order");
    Npy { shape: npy_shape(&hdr), descr, data: buf[off..].to_vec() }
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

fn field(hdr: &str, key: &str) -> String {
    let i = hdr.find(&format!("'{key}'")).unwrap();
    let rest = &hdr[i + key.len() + 3..];
    let a = rest.find('\'').unwrap();
    let b = rest[a + 1..].find('\'').unwrap();
    rest[a + 1..a + 1 + b].to_string()
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
    pub fn i8s(&self) -> Vec<i8> {
        self.data.iter().map(|&b| b as i8).collect()
    }
    pub fn u8s(&self) -> Vec<u8> {
        self.data.clone()
    }
    pub fn len(&self) -> usize {
        self.shape.iter().product()
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
    q16.iter().zip(x16).map(|(&a, &b)| f16_fixed(a) as i128 * f16_fixed(b) as i128).sum()
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
/// f32 x f64 dot product, f64 accumulation, eight partial sums.
pub fn dot32_64(a: &[f32], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    let mut s = [0f64; 8];
    let mut i = 0;
    while i + 8 <= n {
        for k in 0..8 {
            s[k] += a[i + k] as f64 * b[i + k];
        }
        i += 8;
    }
    let mut r = (s[0] + s[1]) + (s[2] + s[3]) + ((s[4] + s[5]) + (s[6] + s[7]));
    while i < n {
        r += a[i] as f64 * b[i];
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
