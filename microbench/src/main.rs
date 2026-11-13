//! Go/no-go microbenchmark: v2's float level step against integer-domain decisions, on synthetic data in v2's block
//! layout. Usage: microbench [reps] [clusters] (defaults 7 and 1000; a few clusters keep the data in cache).
mod kern;
use kern::*;
use std::time::Instant;

pub const UNITS: usize = 64;
pub const NL: usize = 2;
const OFF: [i32; 15] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 18];


struct Rng(u64);
impl Rng {
    fn u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    fn f(&mut self) -> f64 {
        (self.u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn n(&mut self) -> f64 {
        let (a, b) = (self.f().max(1e-300), self.f());
        (-2.0 * a.ln()).sqrt() * (2.0 * std::f64::consts::PI * b).cos()
    }
}

pub struct Cl {
    pub n: usize,
    pub b0: usize,
    pub mn: f32,
    pub qm: f32,
    pub alr: f32,
    pub bin: [(f64, f64); 16],
    pub tab: [Vec<f32>; NL],
}

pub struct Data {
    pub cl: Vec<Cl>,
    pub nblk: usize,
    pub codes: [Vec<u8>; NL],
    pub scal: [Vec<u8>; NL],
    pub ahat: Vec<i16>,
    pub ntab: [f32; 16],
    pub p: [f32; NL],
    pub fixed: f32,
    pub lamsum: f32,
}

fn setnib(buf: &mut [u8], blk: usize, stride: usize, row: usize, i: usize, v: u8) {
    let p = blk * stride + row * 16 + (i & 15);
    if i < 16 {
        buf[p] = (buf[p] & 0xf0) | v;
    } else {
        buf[p] = (buf[p] & 0x0f) | (v << 4);
    }
}
fn getnib(buf: &[u8], blk: usize, stride: usize, row: usize, i: usize) -> usize {
    let b = buf[blk * stride + row * 16 + (i & 15)];
    (if i < 16 { b & 15 } else { b >> 4 }) as usize
}

/// Level-l amplitude of the table weights (std of the level's decoded sum), the norm-code spread, the key spread.
const AMP: [f64; NL] = [0.064, 0.0256];
const RHO_LOGSD: f64 = 0.3;
const KEY_SD: f64 = 0.08;

fn gen(seed: u64, ncl: usize) -> Data {
    let mut r = Rng(seed);
    // centres of 16 equiprobable cells of N(0, 1)
    let mut smp: Vec<f64> = (0..1 << 20).map(|_| r.n()).collect();
    smp.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let cent: Vec<f64> = (0..16).map(|k| smp[k << 16..(k + 1) << 16].iter().sum::<f64>() / 65536.0).collect();
    let mut ntab = [0f32; 16];
    for c in 0..15 {
        ntab[c] = 2f64.powf((OFF[c] - 7) as f64 / 16.0) as f32;
    }
    ntab[15] = ntab[14];
    let sig: Vec<f64> = (0..UNITS).map(|u| 1.0 / (1.0 + u as f64 / 8.0).sqrt()).collect();
    let snorm = sig.iter().map(|x| x * x).sum::<f64>().sqrt();
    let mut cl = Vec::with_capacity(ncl);
    let mut b0 = 0;
    for _ in 0..ncl {
        let n = 800 + (r.u64() % 401) as usize;
        let mn = 0.55 + 0.25 * r.f();
        let qm = 0.15 + 0.05 * r.n();
        let alr = 0.5 * qm / (mn * mn);
        let tab: [Vec<f32>; NL] = core::array::from_fn(|l| {
            let mut t = vec![0f32; UNITS * 16];
            for u in 0..UNITS {
                let w = AMP[l] * sig[u] / snorm * r.n();
                for e in 0..16 {
                    t[u * 16 + e] = (w * cent[e]) as f32;
                }
            }
            t
        });
        cl.push(Cl { n, b0, mn: mn as f32, qm: qm as f32, alr: alr as f32, bin: [(0.0, 0.0); 16], tab });
        b0 += (n + 31) / 32;
    }
    let nblk = b0;
    let mut codes: [Vec<u8>; NL] = core::array::from_fn(|_| vec![0u8; nblk * LROW]);
    let mut scal: [Vec<u8>; NL] = core::array::from_fn(|_| vec![0u8; nblk * SROW]);
    let mut ahat = vec![0i16; nblk * 32 + 2];
    for c in cl.iter_mut() {
        let mut keys: Vec<i16> = (0..c.n).map(|_| ((c.mn as f64 + KEY_SD * r.n()).clamp(-0.999, 0.999) * 32767.0).round() as i16).collect();
        keys.sort();
        for b in 0..16 {
            let (i0, i1) = (b * c.n / 16, (b + 1) * c.n / 16);
            c.bin[b] = (keys[i0] as f64 / 32767.0, keys[i1 - 1] as f64 / 32767.0);
        }
        for i in 0..c.n {
            let blk = c.b0 + i / 32;
            let li = i % 32;
            ahat[c.b0 * 32 + i] = keys[i];
            for l in 0..NL {
                for u in 0..UNITS {
                    setnib(&mut codes[l], blk, LROW, u, li, (r.u64() & 15) as u8);
                }
                let z = (RHO_LOGSD * r.n()).exp() as f32;
                let code = (0..15).find(|&k| ntab[k] >= z).unwrap_or(14);
                setnib(&mut scal[l], blk, SROW, 0, li, code as u8);
                setnib(&mut scal[l], blk, SROW, 1, li, (i * 16 / c.n) as u8);
            }
        }
    }
    Data { cl, nblk, codes, scal, ahat, ntab, p: [0.0; NL], fixed: 2e-4, lamsum: 1.0 }
}

fn item_scale(ah: i16, mn: f32, lamsum: f32) -> f32 {
    let a = ah as f32 / 32767.0f32;
    let s2 = (1.0f32 + mn * mn) - 2.0f32 * mn * a;
    (s2.max(1e-12f32) / lamsum).sqrt()
}

/// Per item (global slot): validity, s, the key term, the cumulative exact decoded sums, the norm codes.
struct Exact {
    valid: Vec<bool>,
    s: Vec<f64>,
    base: Vec<f64>,
    dsum: [Vec<f64>; NL],
    nc: [Vec<u8>; NL],
}
fn exact(d: &Data) -> Exact {
    let ns = d.nblk * 32;
    let mut e = Exact { valid: vec![false; ns], s: vec![0.0; ns], base: vec![0.0; ns], dsum: core::array::from_fn(|_| vec![0.0; ns]), nc: core::array::from_fn(|_| vec![0; ns]) };
    for c in &d.cl {
        let (mn, qm, alr) = (c.mn as f64, c.qm as f64, c.alr as f64);
        for i in 0..c.n {
            let g = c.b0 * 32 + i;
            let (blk, li) = (c.b0 + i / 32, i % 32);
            let ah = d.ahat[g];
            let a = ah as f64 / 32767.0;
            e.valid[g] = true;
            e.s[g] = item_scale(ah, c.mn, d.lamsum) as f64;
            e.base[g] = qm + alr * (mn * a - mn * mn);
            let mut acc = 0f64;
            for l in 0..NL {
                for u in 0..UNITS {
                    acc += c.tab[l][u * 16 + getnib(&d.codes[l], blk, LROW, u, li)] as f64;
                }
                e.dsum[l][g] = acc;
                e.nc[l][g] = getnib(&d.scal[l], blk, SROW, 0, li) as u8;
            }
        }
    }
    e
}
impl Exact {
    fn bounds(&self, d: &Data, l: usize, g: usize) -> (f64, f64) {
        let est = self.base[g] + self.s[g] * self.dsum[l][g];
        let h = self.s[g] * d.p[l] as f64 * d.ntab[self.nc[l][g] as usize] as f64 + d.fixed as f64;
        (est - h, est + h)
    }
    fn undecided(&self, d: &Data, l: usize, t: f64) -> usize {
        (0..self.valid.len()).filter(|&g| self.valid[g] && (0..=l).all(|k| {
            let (lb, ub) = self.bounds(d, k, g);
            lb < t && ub >= t
        })).count()
    }
}

#[derive(Clone, Copy, PartialEq)]
enum K {
    Cur,
    Int8,
    Int16,
}
impl K {
    fn name(self) -> &'static str {
        match self {
            K::Cur => "current (v2 float, 16-bit tables)",
            K::Int8 => "integer, 8-bit tables",
            K::Int16 => "integer, 16-bit split tables",
        }
    }
}

struct Run {
    setup: f64,
    l1: f64,
    l2: f64,
}

struct Ctx {
    cl: Box<CurLut>,
    il: Box<IntLut>,
    cs: CurState,
    is: IntState,
    rec: Rec,
}

fn run(d: &Data, k: K, t: f64, knn: bool, verify: bool, cx: &mut Ctx) -> Run {
    cx.rec.clear();
    let (mut setup, mut tl) = (0f64, [0f64; 2]);
    for ci in 0..d.cl.len() {
        let c = &d.cl[ci];
        let t0 = Instant::now();
        match k {
            K::Cur => cur_setup(c, &mut cx.cl),
            K::Int8 => int_setup(d, c, t, false, &mut cx.il),
            K::Int16 => int_setup(d, c, t, true, &mut cx.il),
        }
        setup += t0.elapsed().as_nanos() as f64;
        unsafe {
            match (k, knn) {
                (K::Cur, _) => {
                    let q = CurQ { t: next_up_if_below(t as f32, t), qm: c.qm, alr: c.alr, fixed: d.fixed, ewdet: 1e-7, dwa: [1.0; NL] };
                    cur_cluster(d, ci, &cx.cl, &q, &mut cx.cs, &mut cx.rec, verify, &mut tl)
                }
                (K::Int8, false) => int_cluster::<false, false>(d, ci, &cx.il, &mut cx.is, &mut cx.rec, verify, &mut tl),
                (K::Int8, true) => int_cluster::<false, true>(d, ci, &cx.il, &mut cx.is, &mut cx.rec, verify, &mut tl),
                (K::Int16, false) => int_cluster::<true, false>(d, ci, &cx.il, &mut cx.is, &mut cx.rec, verify, &mut tl),
                (K::Int16, true) => int_cluster::<true, true>(d, ci, &cx.il, &mut cx.is, &mut cx.rec, verify, &mut tl),
            }
        }
    }
    Run { setup, l1: tl[0], l2: tl[1] }
}

/// The smallest f32 at or above t (so "lb >= t32" implies "lb >= t").
fn next_up_if_below(t32: f32, t: f64) -> f32 {
    if (t32 as f64) < t { f32::from_bits(if t32 >= 0.0 { t32.to_bits() + 1 } else { t32.to_bits() - 1 }) } else { t32 }
}

struct Check {
    in1: usize,
    und1: usize,
    in2: usize,
    und2: usize,
    viol: usize,
}
fn check(d: &Data, ex: &Exact, rec: &Rec, t: f64) -> Check {
    let ns = d.nblk * 32;
    let mut st1 = vec![0u8; ns]; // 0 out, 1 in, 2 undecided
    let mut st2 = vec![0u8; ns];
    for &g in &rec.in_rows[0] {
        st1[g as usize] = 1;
    }
    for &g in &rec.surv1 {
        st1[g as usize] = 2;
    }
    for &g in &rec.in_rows[1] {
        st2[g as usize] = 1;
    }
    for &g in &rec.pend {
        st2[g as usize] = 2;
    }
    let mut viol = 0;
    for g in 0..ns {
        if !ex.valid[g] {
            continue;
        }
        let (lb, ub) = ex.bounds(d, 0, g);
        viol += match st1[g] {
            1 => (lb < t) as usize,
            0 => (ub >= t) as usize,
            _ => 0,
        };
        if st1[g] == 2 {
            let (lb, ub) = ex.bounds(d, 1, g);
            viol += match st2[g] {
                1 => (lb < t) as usize,
                0 => (ub >= t) as usize,
                _ => 0,
            };
        }
    }
    Check { in1: rec.in_rows[0].len(), und1: rec.surv1.len(), in2: rec.in_rows[1].len(), und2: rec.pend.len(), viol }
}

unsafe fn stream(b: &[u8]) -> u64 {
    use std::arch::x86_64::*;
    let mut acc = [_mm256_setzero_si256(); 4];
    for c in b.chunks_exact(128) {
        for k in 0..4 {
            acc[k] = _mm256_xor_si256(acc[k], _mm256_loadu_si256(c.as_ptr().add(32 * k) as *const _));
        }
    }
    let a = _mm256_xor_si256(_mm256_xor_si256(acc[0], acc[1]), _mm256_xor_si256(acc[2], acc[3]));
    _mm256_extract_epi64(a, 0) as u64
}

fn stats(v: &mut Vec<f64>) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[v.len() / 2], v[0], v[v.len() - 1])
}

fn main() {
    let reps: usize = std::env::args().nth(1).map(|s| s.parse().unwrap()).unwrap_or(7);
    let tg = Instant::now();
    let ncl: usize = std::env::args().nth(2).map(|s| s.parse().unwrap()).unwrap_or(1000);
    let mut d = gen(42, ncl);
    let ex = exact(&d);
    let nitems: usize = d.cl.iter().map(|c| c.n).sum();
    let ns = d.nblk * 32;
    println!("generated {} items in {} clusters, {} blocks, in {:.1}s", nitems, d.cl.len(), d.nblk, tg.elapsed().as_secs_f64());
    // threshold at the 1% answer of the level-two estimate
    let mut mids: Vec<f64> = (0..ns).filter(|&g| ex.valid[g]).map(|g| ex.base[g] + ex.s[g] * ex.dsum[1][g]).collect();
    mids.sort_by(|a, b| b.partial_cmp(a).unwrap());
    let t = mids[nitems / 100];
    // level-one half-width scale chosen so the exact bound leaves 15% undecided; level two at 0.45 of it
    let (mut lo, mut hi) = (1e-4f64, 1.0f64);
    for _ in 0..40 {
        let m = (lo * hi).sqrt();
        d.p = [m as f32, (0.45 * m) as f32];
        if ex.undecided(&d, 0, t) as f64 / nitems as f64 > 0.15 { hi = m } else { lo = m }
    }
    d.p = [lo as f32, (0.45 * lo) as f32];
    let mut lbs: Vec<f64> = (0..ns).filter(|&g| ex.valid[g]).map(|g| ex.bounds(&d, 1, g).0).collect();
    lbs.sort_by(|a, b| b.partial_cmp(a).unwrap());
    let tau = lbs[9];
    println!("t = {:.5} (1% answer), P = {:?}, k-NN tau = {:.5} (10th best exact level-two lower bound)", t, d.p, tau);
    println!("std of s*D: level one {:.4}, level two increment {:.4}; mean s {:.3}", AMP[0] * 0.7, AMP[1] * 0.7, ex.s.iter().zip(&ex.valid).filter(|x| *x.1).map(|x| x.0).sum::<f64>() / nitems as f64);

    let maxn = d.cl.iter().map(|c| c.n).max().unwrap() + 64;
    let mut cx = Ctx { cl: CurLut::new(), il: IntLut::new(), cs: CurState::new(maxn), is: IntState::new(maxn), rec: Rec::default() };
    for knn in [false, true] {
        let tt = if knn { tau } else { t };
        let mode = if knn { "k-NN (k = 10, one-sided for the integer kernel)" } else { "threshold (1% answer)" };
        println!("\n== {mode}");
        println!("exact float bound: undecided after level one {}, after level two {}", ex.undecided(&d, 0, tt), ex.undecided(&d, 1, tt));
        for k in [K::Cur, K::Int8, K::Int16] {
            run(&d, k, tt, knn, true, &mut cx);
            let c = check(&d, &ex, &cx.rec, tt);
            let (mut s, mut a, mut b, mut e) = (vec![], vec![], vec![], vec![]);
            for _ in 0..reps {
                let r = run(&d, k, tt, knn, false, &mut cx);
                s.push(r.setup / nitems as f64);
                a.push(r.l1 / nitems as f64);
                b.push(r.l2 / nitems as f64);
                e.push((r.l1 + r.l2) / nitems as f64);
            }
            let (s, a, b, e) = (stats(&mut s), stats(&mut a), stats(&mut b), stats(&mut e));
            println!("{:<36} L1 {:6.3} ns/item [{:.3}-{:.3}]  L1+L2 {:6.3} [{:.3}-{:.3}]  (L2 part {:.3})  setup {:.3}", k.name(), a.0, a.1, a.2, e.0, e.1, e.2, b.0, s.0);
            println!("{:<36} in1 {:7} undecided1 {:7}  in2 {:6} undecided2 {:6}  candidates kept {:7}  violations {}", "", c.in1, c.und1, c.in2, c.und2, c.in1 + c.in2 + c.und2, c.viol);
        }
    }
    // ceiling: one 8-bit fast-scan pass over level one, sums stored, no decisions
    let mut out = vec![0u16; ns + 64];
    let mut v = vec![];
    for _ in 0..reps {
        let mut tot = 0f64;
        for ci in 0..d.cl.len() {
            int_setup(&d, &d.cl[ci], t, false, &mut cx.il);
            let t0 = Instant::now();
            unsafe { fastscan_cluster(&d, ci, &cx.il, &mut out) };
            tot += t0.elapsed().as_nanos() as f64;
        }
        v.push(tot / nitems as f64);
    }
    let c = stats(&mut v);
    println!("\nceiling: fast-scan pass, level one, no decisions: {:.3} ns/item [{:.3}-{:.3}] (checksum {})", c.0, c.1, c.2, out.iter().map(|&x| x as u64).sum::<u64>());
    // memory floor: read level one's codes and nothing else
    let mut v = vec![];
    let mut x = 0u64;
    for _ in 0..reps {
        let t0 = Instant::now();
        x ^= unsafe { stream(&d.codes[0]) };
        v.push(t0.elapsed().as_nanos() as f64 / nitems as f64);
    }
    let c = stats(&mut v);
    println!("memory floor: read level one's codes only: {:.3} ns/item [{:.3}-{:.3}] ({:.1} GB/s at the median; {x})", c.0, c.1, c.2, (d.codes[0].len() as f64 / nitems as f64) / c.0);
}
