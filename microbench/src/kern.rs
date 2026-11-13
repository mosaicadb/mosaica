//! AVX2 kernels over v2's block layout: 32 items per block, a row is 16 bytes, byte p holds item p in the low nibble
//! and item 16 + p in the high nibble. A level is 64 rows (one per 4-bit unit, 1024 bytes per block); the scalar rows
//! of a level are [norm code, key bin] (32 bytes per block). Tables are in pair layout: 32 bytes per two units, lane 0
//! the even unit's 16 entries, lane 1 the odd unit's.
use crate::{Cl, Data, NL, UNITS};
use std::arch::x86_64::*;
use std::time::Instant;

pub const LROW: usize = UNITS * 16; // bytes of one level per block
pub const SROW: usize = 32; // bytes of the scalar rows per block
const PAIRS: usize = UNITS / 2;

/// Output of one kernel over the corpus (global slot numbers).
#[derive(Default)]
pub struct Rec {
    pub in_rows: [Vec<u32>; NL],
    pub surv1: Vec<u32>,
    pub pend: Vec<u32>,
}
impl Rec {
    pub fn clear(&mut self) {
        self.in_rows[0].clear();
        self.in_rows[1].clear();
        self.surv1.clear();
        self.pend.clear();
    }
}

// ---------------- shared primitives ----------------

/// Table sums of `PAIRS` row pairs over one block: u16 (even items, odd items); lane 0 items 0..15, lane 1 16..31.
#[inline(always)]
pub unsafe fn unit_sums(codes: *const u8, tab: *const u8) -> (__m256i, __m256i) {
    let m = _mm256_set1_epi8(0x0f);
    let z = _mm256_setzero_si256();
    let (mut a0, mut b0, mut a1, mut b1) = (z, z, z, z);
    for g in 0..PAIRS {
        let c = _mm256_loadu_si256(codes.add(g * 32) as *const _);
        let t = _mm256_loadu_si256(tab.add(g * 32) as *const _);
        let r0 = _mm256_shuffle_epi8(t, _mm256_and_si256(c, m));
        let r1 = _mm256_shuffle_epi8(t, _mm256_and_si256(_mm256_srli_epi16(c, 4), m));
        a0 = _mm256_add_epi16(a0, r0);
        b0 = _mm256_add_epi16(b0, _mm256_srli_epi16(r0, 8));
        a1 = _mm256_add_epi16(a1, r1);
        b1 = _mm256_add_epi16(b1, _mm256_srli_epi16(r1, 8));
    }
    let e0 = _mm256_sub_epi16(a0, _mm256_slli_epi16(b0, 8));
    let e1 = _mm256_sub_epi16(a1, _mm256_slli_epi16(b1, 8));
    let ev = _mm256_add_epi16(_mm256_permute2x128_si256(e0, e1, 0x20), _mm256_permute2x128_si256(e0, e1, 0x31));
    let od = _mm256_add_epi16(_mm256_permute2x128_si256(b0, b1, 0x20), _mm256_permute2x128_si256(b0, b1, 0x31));
    (ev, od)
}

/// Nibbles of a scalar row as shuffle indices in item order (lane 0 items 0..15, lane 1 items 16..31).
#[inline(always)]
unsafe fn nib_idx(row: *const u8) -> __m256i {
    let r = _mm_loadu_si128(row as *const _);
    let m = _mm_set1_epi8(0x0f);
    _mm256_set_m128i(_mm_and_si128(_mm_srli_epi16(r, 4), m), _mm_and_si128(r, m))
}

/// u16 per item from the low-byte and high-byte lookups, as (even items, odd items).
#[inline(always)]
unsafe fn split16(xl: __m256i, xh: __m256i) -> (__m256i, __m256i) {
    let ev = _mm256_or_si256(_mm256_and_si256(xl, _mm256_set1_epi16(0x00ff)), _mm256_slli_epi16(xh, 8));
    let od = _mm256_or_si256(_mm256_srli_epi16(xl, 8), _mm256_and_si256(xh, _mm256_set1_epi16(0xff00u16 as i16)));
    (ev, od)
}

/// Item-order bit mask from (even, odd) u16 compare results.
#[inline(always)]
unsafe fn eo_mask(e: __m256i, o: __m256i) -> u32 {
    let m = _mm256_movemask_epi8(_mm256_packs_epi16(e, o)) as u32;
    let ev = (m & 0xff) | ((m >> 8) & 0xff00);
    let od = ((m >> 8) & 0xff) | ((m >> 16) & 0xff00);
    _pdep_u32(ev, 0x5555_5555) | _pdep_u32(od, 0xaaaa_aaaa)
}

/// Position of item i of a block in a stored (even, odd) u16 pair of vectors.
#[inline(always)]
pub fn eo_pos(i: usize) -> usize {
    (i >> 4) * 8 + ((i & 15) >> 1) + (i & 1) * 16
}

/// (even, odd) layout from 32 u16 in item order.
#[inline(always)]
unsafe fn to_eo(v: *const u16) -> (__m256i, __m256i) {
    let a = _mm256_loadu_si256(v as *const _);
    let b = _mm256_loadu_si256(v.add(16) as *const _);
    let lo = _mm256_set1_epi32(0xffff);
    let e = _mm256_packus_epi32(_mm256_and_si256(a, lo), _mm256_and_si256(b, lo));
    let o = _mm256_packus_epi32(_mm256_srli_epi32(a, 16), _mm256_srli_epi32(b, 16));
    (_mm256_permute4x64_epi64(e, 0xd8), _mm256_permute4x64_epi64(o, 0xd8))
}

/// Where a re-packed block's bytes come from: per distinct source block, the byte masks of the dest bytes whose low
/// (items 0..15) and high (16..31) nibble items it supplies, and per dest byte the source byte and nibble side.
pub struct Gather {
    pub ident: Option<usize>,
    pub nsrc: usize,
    pub src: [u32; 32],
    pub mlo: [[u8; 32]; 32],
    pub mhi: [[u8; 32]; 32],
    pub idxlo: [u8; 32],
    pub idxhi: [u8; 32],
    pub shlo: [u8; 32],
    pub shhi: [u8; 32],
}
impl Gather {
    pub fn new() -> Gather {
        Gather { ident: None, nsrc: 0, src: [0; 32], mlo: [[0; 32]; 32], mhi: [[0; 32]; 32], idxlo: [0; 32], idxhi: [0; 32], shlo: [0; 32], shhi: [0; 32] }
    }
}

/// Plan the re-pack of `lanes` (ascending local item indices, at most 32).
pub fn plan_gather(lanes: &[u32], g: &mut Gather) {
    let cnt = lanes.len();
    if cnt == 32 && lanes[0] % 32 == 0 && lanes[31] == lanes[0] + 31 {
        g.ident = Some(lanes[0] as usize / 32);
        return;
    }
    g.ident = None;
    g.idxlo = [0; 32];
    g.idxhi = [0; 32];
    g.shlo = [0; 32];
    g.shhi = [0; 32];
    let mut ns = 0;
    let mut cur = u32::MAX;
    for (d, &p) in lanes.iter().enumerate() {
        let b = p >> 5;
        if b != cur {
            cur = b;
            g.src[ns] = b;
            g.mlo[ns] = [0; 32];
            g.mhi[ns] = [0; 32];
            ns += 1;
        }
        let q = p & 31;
        let byte = d & 15;
        let sel = (q & 15) as u8;
        if d < 16 {
            let sh = if q >= 16 { 0xff } else { 0 };
            for x in [byte, byte + 16] {
                g.mlo[ns - 1][x] = 0xff;
                g.idxlo[x] = sel;
                g.shlo[x] = sh;
            }
        } else {
            let sh = if q < 16 { 0xff } else { 0 };
            for x in [byte, byte + 16] {
                g.mhi[ns - 1][x] = 0xff;
                g.idxhi[x] = sel;
                g.shhi[x] = sh;
            }
        }
    }
    g.nsrc = ns;
}

/// Assemble `pairs` row pairs of the re-packed block from source blocks at `base` (block stride `stride`) into dst.
#[inline(always)]
pub unsafe fn gather_rows(g: &Gather, base: *const u8, stride: usize, pairs: usize, dst: *mut u8) {
    let ld = |p: &[u8; 32]| _mm256_loadu_si256(p.as_ptr() as *const _);
    let (il, ih, sl, sh) = (ld(&g.idxlo), ld(&g.idxhi), ld(&g.shlo), ld(&g.shhi));
    let m0f = _mm256_set1_epi8(0x0f);
    let mf0 = _mm256_set1_epi8(0xf0u8 as i8);
    for gi in 0..pairs {
        let mut t1 = _mm256_setzero_si256();
        let mut t2 = _mm256_setzero_si256();
        for s in 0..g.nsrc {
            let v = _mm256_loadu_si256(base.add(*g.src.get_unchecked(s) as usize * stride + gi * 32) as *const _);
            t1 = _mm256_blendv_epi8(t1, _mm256_shuffle_epi8(v, il), ld(g.mlo.get_unchecked(s)));
            t2 = _mm256_blendv_epi8(t2, _mm256_shuffle_epi8(v, ih), ld(g.mhi.get_unchecked(s)));
        }
        let lo = _mm256_and_si256(_mm256_blendv_epi8(t1, _mm256_srli_epi16(t1, 4), sl), m0f);
        let hi = _mm256_and_si256(_mm256_blendv_epi8(t2, _mm256_slli_epi16(t2, 4), sh), mf0);
        _mm256_storeu_si256(dst.add(gi * 32) as *mut _, _mm256_or_si256(lo, hi));
    }
}

#[inline(always)]
fn push_bits(m: u32, base: u32, idx: impl Fn(usize) -> u32, out: &mut Vec<u32>) {
    let mut m = m;
    while m != 0 {
        let i = m.trailing_zeros() as usize;
        out.push(base + idx(i));
        m &= m - 1;
    }
}

fn quant_pair_pos(u: usize, e: usize) -> usize {
    (u / 2) * 32 + (u % 2) * 16 + e
}

// ---------------- current kernel: v2's level step ----------------

pub struct CurLut {
    pub hi: [[u8; LROW]; NL],
    pub lo: [[u8; LROW]; NL],
    pub off: [f32; NL],
    pub scale: [f32; NL],
    pub err: [f32; NL],
}
impl CurLut {
    pub fn new() -> Box<CurLut> {
        Box::new(CurLut { hi: [[0; LROW]; NL], lo: [[0; LROW]; NL], off: [0.0; NL], scale: [0.0; NL], err: [0.0; NL] })
    }
}

/// v2's 16-bit (split) tables: per level one scale, entries shifted by the unit minimum, error bound as build_lut's.
pub fn cur_setup(c: &Cl, out: &mut CurLut) {
    for l in 0..NL {
        let tab = &c.tab[l];
        let mut rmax = 0f32;
        let mut mins = [0f32; UNITS];
        let mut sabs = 0f64;
        let mut off = 0f64;
        for u in 0..UNITS {
            let t = &tab[u * 16..u * 16 + 16];
            let (mut lo, mut hi, mut am) = (f32::MAX, f32::MIN, 0f32);
            for &x in t {
                lo = lo.min(x);
                hi = hi.max(x);
                am = am.max(x.abs());
            }
            mins[u] = lo;
            rmax = rmax.max(hi - lo);
            sabs += am as f64;
            off += lo as f64;
        }
        let scale = if rmax > 0.0 { rmax / 65535.0 } else { 1e-30 };
        let inv = 1.0 / scale;
        for u in 0..UNITS {
            for e in 0..16 {
                let q = ((tab[u * 16 + e] - mins[u]) * inv).round().clamp(0.0, 65535.0) as u32;
                let p = quant_pair_pos(u, e);
                out.hi[l][p] = (q >> 8) as u8;
                out.lo[l][p] = q as u8;
            }
        }
        let u32_ = 1.0 / 16777216.0;
        out.err[l] = (UNITS as f64 * scale as f64 * 0.5 * (1.0 + 1e-6) + (48.0 + 2.0 * UNITS as f64) * u32_ * sabs + 1e-30) as f32;
        out.off[l] = off as f32;
        out.scale[l] = scale;
    }
}

pub struct CurState {
    pub idx: Vec<i32>,
    pub s: Vec<f32>,
    pub mr: Vec<f32>,
    pub adet: Vec<f32>,
    pub tmp: Vec<u8>,
    pub stmp: Vec<u8>,
    pub perm: Box<[[i32; 8]; 256]>,
    pub g: Box<Gather>,
}
impl CurState {
    pub fn new(cap: usize) -> CurState {
        let mut perm = Box::new([[0i32; 8]; 256]);
        for m in 0..256 {
            let mut k = 0;
            for i in 0..8 {
                if m >> i & 1 == 1 {
                    perm[m][k] = i as i32;
                    k += 1;
                }
            }
        }
        let n = cap + 64;
        CurState { idx: vec![-1; n], s: vec![0.0; n], mr: vec![0.0; n], adet: vec![0.0; n], tmp: vec![0; LROW + 64], stmp: vec![0; SROW + 64], perm, g: Box::new(Gather::new()) }
    }
}

#[inline(always)]
unsafe fn ld8(v: &[f32], i: usize) -> __m256 {
    _mm256_loadu_ps(v.as_ptr().add(i))
}

/// Keep lanes m of v[o..o+8] at v[w..]; a full 8-lane store (w <= o).
#[inline(always)]
unsafe fn cs(v: &mut [f32], perm: &[i32; 8], w: usize, o: usize) {
    let x = _mm256_loadu_ps(v.as_ptr().add(o));
    let p = _mm256_loadu_si256(perm.as_ptr() as *const _);
    _mm256_storeu_ps(v.as_mut_ptr().add(w), _mm256_permutevar8x32_ps(x, p));
}
#[inline(always)]
unsafe fn csi(v: &mut [i32], perm: &[i32; 8], w: usize, o: usize) {
    let x = _mm256_loadu_si256(v.as_ptr().add(o) as *const _);
    let p = _mm256_loadu_si256(perm.as_ptr() as *const _);
    _mm256_storeu_si256(v.as_mut_ptr().add(w) as *mut _, _mm256_permutevar8x32_epi32(x, p));
}

/// u16 (even, odd) sums as f32 in item order: items 0..7, 8..15, 16..23, 24..31.
#[inline(always)]
unsafe fn to_f32_items(e: __m256i, o: __m256i) -> [__m256; 4] {
    let x0 = _mm256_unpacklo_epi16(e, o);
    let x1 = _mm256_unpackhi_epi16(e, o);
    let f = |x: __m128i| _mm256_cvtepi32_ps(_mm256_cvtepu16_epi32(x));
    [f(_mm256_castsi256_si128(x0)), f(_mm256_castsi256_si128(x1)), f(_mm256_extracti128_si256(x0, 1)), f(_mm256_extracti128_si256(x1, 1))]
}

#[cold]
#[inline(never)]
fn escape_values(_d: &Data, _l: usize, rho: __m256) -> __m256 {
    rho
}

pub struct CurQ {
    pub t: f32,
    pub qm: f32,
    pub alr: f32,
    pub fixed: f32,
    pub ewdet: f32,
    pub dwa: [f32; NL],
}

/// Decode and decide one 32-item block at level l (det-level bound of v2). Returns the kept lanes.
#[inline(always)]
unsafe fn cur_block(d: &Data, lut: &CurLut, q: &CurQ, edet: f32, l: usize, cp: *const u8, sp: *const u8, k0: usize, act: u32, sv: &mut CurState, row0: u32, rin: &mut Vec<u32>) -> u32 {
    let (he, ho) = unit_sums(cp, lut.hi[l].as_ptr());
    let (le, lo) = unit_sums(cp, lut.lo[l].as_ptr());
    let hf = to_f32_items(he, ho);
    let lf = to_f32_items(le, lo);
    let (off, sc) = (_mm256_set1_ps(lut.off[l]), _mm256_set1_ps(lut.scale[l]));
    let ntl = _mm256_loadu_ps(d.ntab.as_ptr());
    let nth = _mm256_loadu_ps(d.ntab.as_ptr().add(8));
    let pconst = _mm256_set1_ps(d.p[l]);
    let one6 = _mm256_set1_ps(1.0 + 1e-6);
    let u16x = _mm256_set1_ps(16.0 / 16777216.0);
    let absm = _mm256_castsi256_ps(_mm256_set1_epi32(0x7fff_ffff));
    let tv = _mm256_set1_ps(q.t);
    let mut keep = 0u32;
    for h in 0..4 {
        let hm = (act >> (8 * h)) & 0xff;
        if hm == 0 {
            continue;
        }
        let o = k0 + 8 * h;
        let pv = _mm256_fmadd_ps(_mm256_fmadd_ps(hf[h], _mm256_set1_ps(256.0), lf[h]), sc, off);
        let adet = _mm256_add_ps(ld8(&sv.adet, o), pv);
        _mm256_storeu_ps(sv.adet.as_mut_ptr().add(o), adet);
        let s = ld8(&sv.s, o);
        let alrmr = _mm256_fmadd_ps(_mm256_set1_ps(q.alr), ld8(&sv.mr, o), _mm256_set1_ps(q.qm));
        let raw = _mm256_cvtepu8_epi32(_mm_loadl_epi64(sp.add(8 * (h & 1)) as *const _));
        let code = if h < 2 { _mm256_and_si256(raw, _mm256_set1_epi32(15)) } else { _mm256_srli_epi32(raw, 4) };
        let nt = _mm256_blendv_ps(_mm256_permutevar8x32_ps(ntl, code), _mm256_permutevar8x32_ps(nth, code), _mm256_castsi256_ps(_mm256_slli_epi32(code, 28)));
        let mut rho = _mm256_mul_ps(_mm256_mul_ps(pconst, s), nt);
        let em = _mm256_movemask_ps(_mm256_castsi256_ps(_mm256_cmpeq_epi32(code, _mm256_set1_epi32(15)))) as u32 & hm;
        if em != 0 {
            rho = escape_values(d, l, rho);
        }
        let sa = _mm256_mul_ps(s, adet);
        let est = _mm256_add_ps(alrmr, sa);
        let fl = _mm256_mul_ps(u16x, _mm256_add_ps(_mm256_and_ps(alrmr, absm), _mm256_and_ps(sa, absm)));
        let hh = _mm256_add_ps(
            _mm256_mul_ps(_mm256_mul_ps(_mm256_set1_ps(q.dwa[l]), rho), one6),
            _mm256_add_ps(_mm256_set1_ps(q.fixed), _mm256_fmadd_ps(s, _mm256_set1_ps(edet + q.ewdet), fl)),
        );
        let lb = _mm256_sub_ps(est, hh);
        let ub = _mm256_add_ps(est, hh);
        let inm = _mm256_movemask_ps(_mm256_cmp_ps::<_CMP_GE_OQ>(lb, tv)) as u32 & hm;
        let outm = _mm256_movemask_ps(_mm256_cmp_ps::<_CMP_LT_OQ>(ub, tv)) as u32 & hm;
        if inm != 0 {
            let ix = &sv.idx;
            push_bits(inm, row0, |i| ix[o + i] as u32, rin);
        }
        keep |= (hm & !(inm | outm)) << (8 * h);
    }
    keep
}

#[inline(always)]
unsafe fn act_of(idx: &[i32], k0: usize) -> u32 {
    let none = _mm256_set1_epi32(-1);
    let mut a = 0u32;
    for h in 0..4 {
        let v = _mm256_loadu_si256(idx.as_ptr().add(k0 + 8 * h) as *const _);
        a |= ((!_mm256_movemask_ps(_mm256_castsi256_ps(_mm256_cmpeq_epi32(v, none))) as u32) & 0xff) << (8 * h);
    }
    a
}

/// v2's run_cluster for all items of cluster ci: init (key gather, scale with a square root), levels with holes while
/// occupancy stays at 60% or more, then compaction and re-packing. Adds level times (ns) to tl.
pub unsafe fn cur_cluster(d: &Data, ci: usize, lut: &CurLut, q: &CurQ, sv: &mut CurState, rec: &mut Rec, verify: bool, tl: &mut [f64; 2]) {
    let t0 = Instant::now();
    let c = &d.cl[ci];
    let nb0 = (c.n + 31) / 32;
    let slots = nb0 * 32;
    let row0 = (c.b0 * 32) as u32;
    for k in 0..slots + 32 {
        sv.idx[k] = if k < c.n { k as i32 } else { -1 };
    }
    sv.adet[..slots + 32].fill(0.0);
    {
        let ahp = d.ahat.as_ptr().add(c.b0 * 32) as *const i32;
        let mn = _mm256_set1_ps(c.mn);
        let one_mn2 = _mm256_set1_ps(1.0 + c.mn * c.mn);
        let two_mn = _mm256_set1_ps(2.0 * c.mn);
        let mn2 = _mm256_set1_ps(c.mn * c.mn);
        let none = _mm256_set1_epi32(-1);
        let mut k = 0;
        while k < slots {
            let iv0 = _mm256_loadu_si256(sv.idx.as_ptr().add(k) as *const _);
            let m = _mm256_andnot_si256(_mm256_cmpeq_epi32(iv0, none), none);
            let iv = _mm256_and_si256(iv0, m);
            let raw = _mm256_mask_i32gather_epi32::<2>(_mm256_setzero_si256(), ahp, iv, m);
            let ah = _mm256_srai_epi32(_mm256_slli_epi32(raw, 16), 16);
            let a = _mm256_div_ps(_mm256_cvtepi32_ps(ah), _mm256_set1_ps(32767.0));
            let s2 = _mm256_sub_ps(one_mn2, _mm256_mul_ps(two_mn, a));
            let sc = _mm256_sqrt_ps(_mm256_div_ps(_mm256_max_ps(s2, _mm256_set1_ps(1e-12)), _mm256_set1_ps(d.lamsum)));
            _mm256_storeu_ps(sv.s.as_mut_ptr().add(k), sc);
            _mm256_storeu_ps(sv.mr.as_mut_ptr().add(k), _mm256_sub_ps(_mm256_mul_ps(mn, a), mn2));
            k += 8;
        }
    }
    let mut nsurv = slots;
    let mut aligned = true;
    let mut edet = 0f32;
    let mut t_mid = t0;
    for l in 0..NL {
        if l == 1 {
            t_mid = Instant::now();
            tl[0] += (t_mid - t0).as_nanos() as f64;
        }
        if nsurv == 0 {
            continue;
        }
        edet += lut.err[l];
        let nb = (nsurv + 31) / 32;
        let cbase = d.codes[l].as_ptr().add(c.b0 * LROW);
        let sbase = d.scal[l].as_ptr().add(c.b0 * SROW);
        let mut kept_tot = 0usize;
        let mut wpos = 0usize;
        let mut lanes = [0u32; 32];
        for b in 0..nb {
            let k0 = b * 32;
            let act = act_of(&sv.idx, k0);
            if act == 0 {
                continue;
            }
            let (cp, sp) = if aligned {
                (cbase.add(b * LROW), sbase.add(b * SROW))
            } else {
                let cnt = act.count_ones() as usize;
                for i in 0..cnt {
                    lanes[i] = sv.idx[k0 + i] as u32;
                }
                plan_gather(&lanes[..cnt], &mut sv.g);
                match sv.g.ident {
                    Some(s) => (cbase.add(s * LROW), sbase.add(s * SROW)),
                    None => {
                        gather_rows(&sv.g, cbase, LROW, PAIRS, sv.tmp.as_mut_ptr());
                        gather_rows(&sv.g, sbase, SROW, 1, sv.stmp.as_mut_ptr());
                        (sv.tmp.as_ptr(), sv.stmp.as_ptr())
                    }
                }
            };
            let keep = cur_block(d, lut, q, edet, l, cp, sp, k0, act, sv, row0, &mut rec.in_rows[l]);
            if aligned {
                for h in 0..4 {
                    let hm = (act >> (8 * h)) & 0xff;
                    let gone = hm & !(keep >> (8 * h));
                    if gone & 0xff != 0 {
                        let lanes_m = _mm256_cmpgt_epi32(_mm256_and_si256(_mm256_set1_epi32(gone as i32), _mm256_setr_epi32(1, 2, 4, 8, 16, 32, 64, 128)), _mm256_setzero_si256());
                        _mm256_maskstore_epi32(sv.idx.as_mut_ptr().add(k0 + 8 * h), lanes_m, _mm256_set1_epi32(-1));
                    }
                }
                kept_tot += keep.count_ones() as usize;
                continue;
            }
            for h in 0..4 {
                let m = (keep >> (8 * h)) & 0xff;
                if m == 0 {
                    continue;
                }
                let o = k0 + 8 * h;
                if wpos == o && m == 0xff {
                    wpos += 8;
                    continue;
                }
                let p = &sv.perm[m as usize];
                csi(&mut sv.idx, p, wpos, o);
                cs(&mut sv.s, p, wpos, o);
                cs(&mut sv.mr, p, wpos, o);
                cs(&mut sv.adet, p, wpos, o);
                wpos += m.count_ones() as usize;
            }
        }
        if aligned {
            if kept_tot * 10 < nsurv * 6 {
                let mut w = 0;
                let mut o = 0;
                while o < nsurv {
                    let a = act_of(&sv.idx, o & !31) >> (o & 31) & 0xff;
                    if a != 0 {
                        let p = &sv.perm[a as usize];
                        csi(&mut sv.idx, p, w, o);
                        cs(&mut sv.s, p, w, o);
                        cs(&mut sv.mr, p, w, o);
                        cs(&mut sv.adet, p, w, o);
                        w += a.count_ones() as usize;
                    }
                    o += 8;
                }
                nsurv = w;
                aligned = false;
                sv.idx[nsurv..nsurv + 32].fill(-1);
            }
        } else {
            nsurv = wpos;
            sv.idx[nsurv..nsurv + 32].fill(-1);
        }
        if verify && l == 0 {
            for k in 0..nsurv {
                if sv.idx[k] >= 0 {
                    rec.surv1.push(row0 + sv.idx[k] as u32);
                }
            }
        }
    }
    for k in 0..nsurv {
        if sv.idx[k] >= 0 {
            rec.pend.push(row0 + sv.idx[k] as u32);
        }
    }
    tl[1] += t_mid.elapsed().as_nanos() as f64;
}

// ---------------- integer kernel ----------------

pub struct IntLut {
    pub hi: [[u8; LROW]; NL], // 8-bit tables, or the high bytes of 16-bit ones
    pub lo: [[u8; LROW]; NL], // low bytes of 16-bit tables
    pub tin: [[__m256i; 2]; NL], // per key bin: lowest (value - R) that is in; low and high bytes
    pub tout: [[__m256i; 2]; NL], // per key bin: (value + R) below this is out
    pub r: [[__m256i; 2]; NL], // per norm code: half-width in table steps
}
impl IntLut {
    pub fn new() -> Box<IntLut> {
        unsafe {
            let z = _mm256_setzero_si256();
            Box::new(IntLut { hi: [[0; LROW]; NL], lo: [[0; LROW]; NL], tin: [[z; 2]; NL], tout: [[z; 2]; NL], r: [[z; 2]; NL] })
        }
    }
}

fn bytes16(v: &[i32; 16]) -> [__m256i; 2] {
    let mut lo = [0u8; 32];
    let mut hi = [0u8; 32];
    for i in 0..16 {
        lo[i] = v[i] as u8;
        lo[i + 16] = v[i] as u8;
        hi[i] = (v[i] >> 8) as u8;
        hi[i + 16] = (v[i] >> 8) as u8;
    }
    unsafe { [_mm256_loadu_si256(lo.as_ptr() as *const _), _mm256_loadu_si256(hi.as_ptr() as *const _)] }
}

/// max (hi = true) or min of K(a) = (tt - qm - alr (mn a - mn^2)) / s(a) over [a0, a1], s(a) = sqrt((1 + mn^2 - 2 mn a) / lamsum).
fn k_extreme(c: &Cl, lamsum: f64, tt: f64, a0: f64, a1: f64, hi: bool) -> f64 {
    let (mn, qm, alr) = (c.mn as f64, c.qm as f64, c.alr as f64);
    let k = |a: f64| (tt - qm - alr * (mn * a - mn * mn)) / ((1.0 + mn * mn - 2.0 * mn * a).max(1e-12) / lamsum).sqrt();
    let (aa, bb, cc, dd) = (tt - qm + alr * mn * mn, alr * mn, 1.0 + mn * mn, 2.0 * mn);
    let mut v = [k(a0), k(a1)];
    let mut best = if hi { v[0].max(v[1]) } else { v[0].min(v[1]) };
    if (bb * dd).abs() > 1e-300 {
        let ac = (2.0 * bb * cc - aa * dd) / (bb * dd);
        if ac > a0 && ac < a1 {
            v[0] = k(ac);
            best = if hi { best.max(v[0]) } else { best.min(v[0]) };
        }
    }
    best + if hi { 1.0 } else { -1.0 } * (best.abs() * 1e-6 + 1e-12)
}

/// Fold every per-cluster term into the integer tables: one step size for both levels (the widest unit range over
/// 255, or over 65280 in 16-bit split form); per key bin the in and out thresholds; per norm code the half-width.
pub fn int_setup(d: &Data, c: &Cl, t: f64, split: bool, out: &mut IntLut) {
    let mut rmax = 0f64;
    for l in 0..NL {
        for u in 0..UNITS {
            let t_ = &c.tab[l][u * 16..u * 16 + 16];
            let lo = t_.iter().fold(f32::MAX, |a, &x| a.min(x));
            let hi = t_.iter().fold(f32::MIN, |a, &x| a.max(x));
            rmax = rmax.max((hi - lo) as f64);
        }
    }
    let sig = (rmax / 255.0).max(1e-30) * (1.0 + 1e-9);
    let step = if split { sig / 256.0 } else { sig };
    let qmax = if split { 65535.0 } else { 255.0 };
    let (mut offc, mut elo, mut ehi) = (0f64, 0f64, 0f64);
    let lamsum = d.lamsum as f64;
    for l in 0..NL {
        let tab = &c.tab[l];
        let mut err = 0f64;
        for u in 0..UNITS {
            let t_ = &tab[u * 16..u * 16 + 16];
            let mn = t_.iter().fold(f32::MAX, |a, &x| a.min(x)) as f64;
            offc += mn;
            let mut eu = 0f64;
            for e in 0..16 {
                let x = t_[e] as f64 - mn;
                let q = (x / step).round().clamp(0.0, qmax);
                eu = eu.max((x - q * step).abs());
                let qi = q as u32;
                let p = quant_pair_pos(u, e);
                if split {
                    out.hi[l][p] = (qi >> 8) as u8;
                    out.lo[l][p] = qi as u8;
                } else {
                    out.hi[l][p] = qi as u8;
                }
            }
            err += eu;
        }
        // the 16-bit form keeps floor(sum of low bytes / 256): up to one step below the value
        let err = err * (1.0 + 1e-9) + 1e-15;
        elo += err;
        ehi += err + if split { sig } else { 0.0 };
        let mut tin = [0i32; 16];
        let mut tout = [0i32; 16];
        for b in 0..16 {
            let (a0, a1) = c.bin[b];
            let kin = k_extreme(c, lamsum, t + d.fixed as f64, a0, a1, true);
            let kout = k_extreme(c, lamsum, t - d.fixed as f64, a0, a1, false);
            tin[b] = ((kin - offc + elo) / sig).ceil().clamp(0.0, 32767.0) as i32;
            tout[b] = ((kout - offc - ehi) / sig).ceil().clamp(0.0, 32767.0) as i32;
        }
        let mut r = [32767i32; 16];
        for cd in 0..15 {
            r[cd] = ((d.p[l] as f64 * d.ntab[cd] as f64 / sig) * (1.0 + 1e-9)).ceil().min(32767.0) as i32;
        }
        out.tin[l] = bytes16(&tin);
        out.tout[l] = bytes16(&tout);
        out.r[l] = bytes16(&r);
    }
}

/// One block at level l: adds the level's sums to (se, so); returns the new sums and the in and out masks (item order).
#[inline(always)]
unsafe fn int_block<const SPLIT: bool, const KNN: bool>(lut: &IntLut, l: usize, cp: *const u8, sp: *const u8, se: __m256i, so: __m256i) -> (__m256i, __m256i, u32, u32) {
    let (mut e, mut o) = unit_sums(cp, lut.hi[l].as_ptr());
    if SPLIT {
        // H + floor(L / 256): 256 H + L does not fit in 16 bits, so the sum stays in coarse steps
        let (le, lo) = unit_sums(cp, lut.lo[l].as_ptr());
        e = _mm256_add_epi16(e, _mm256_srli_epi16(le, 8));
        o = _mm256_add_epi16(o, _mm256_srli_epi16(lo, 8));
    }
    let se = _mm256_add_epi16(se, e);
    let so = _mm256_add_epi16(so, o);
    let ni = nib_idx(sp);
    let ki = nib_idx(sp.add(16));
    let (re, ro) = split16(_mm256_shuffle_epi8(lut.r[l][0], ni), _mm256_shuffle_epi8(lut.r[l][1], ni));
    let (oe, oo) = split16(_mm256_shuffle_epi8(lut.tout[l][0], ki), _mm256_shuffle_epi8(lut.tout[l][1], ki));
    let he = _mm256_adds_epi16(se, re);
    let ho = _mm256_adds_epi16(so, ro);
    let outm = eo_mask(_mm256_cmpgt_epi16(oe, he), _mm256_cmpgt_epi16(oo, ho));
    if KNN {
        return (se, so, 0, outm);
    }
    let (ie, io) = split16(_mm256_shuffle_epi8(lut.tin[l][0], ki), _mm256_shuffle_epi8(lut.tin[l][1], ki));
    let le = _mm256_subs_epi16(se, re);
    let lo = _mm256_subs_epi16(so, ro);
    let notin = eo_mask(_mm256_cmpgt_epi16(ie, le), _mm256_cmpgt_epi16(io, lo));
    (se, so, !notin, outm)
}

pub struct IntState {
    pub sc: Vec<u16>,    // level-one sums per slot, (even, odd) layout per block
    pub live: Vec<u32>,  // live mask per block after level one
    pub surv: Vec<u32>,  // survivors (local item index)
    pub tmp: Vec<u8>,
    pub stmp: Vec<u8>,
    pub g: Box<Gather>,
}
impl IntState {
    pub fn new(cap: usize) -> IntState {
        IntState { sc: vec![0; cap + 64], live: vec![0; cap / 32 + 2], surv: Vec::with_capacity(cap), tmp: vec![0; LROW + 64], stmp: vec![0; SROW + 64], g: Box::new(Gather::new()) }
    }
}

/// The integer kernel for all items of cluster ci. Level two keeps the blocks with live masks when at least half the
/// lanes survive, and re-packs otherwise. Adds level times (ns) to tl.
pub unsafe fn int_cluster<const SPLIT: bool, const KNN: bool>(d: &Data, ci: usize, lut: &IntLut, sv: &mut IntState, rec: &mut Rec, verify: bool, tl: &mut [f64; 2]) {
    let t0 = Instant::now();
    let c = &d.cl[ci];
    let nb = (c.n + 31) / 32;
    let row0 = (c.b0 * 32) as u32;
    let z = _mm256_setzero_si256();
    let cbase = d.codes[0].as_ptr().add(c.b0 * LROW);
    let sbase = d.scal[0].as_ptr().add(c.b0 * SROW);
    sv.surv.clear();
    for b in 0..nb {
        let left = c.n - b * 32;
        let act = if left >= 32 { u32::MAX } else { (1u32 << left) - 1 };
        let (se, so, inm, outm) = int_block::<SPLIT, KNN>(lut, 0, cbase.add(b * LROW), sbase.add(b * SROW), z, z);
        _mm256_storeu_si256(sv.sc.as_mut_ptr().add(b * 32) as *mut _, se);
        _mm256_storeu_si256(sv.sc.as_mut_ptr().add(b * 32 + 16) as *mut _, so);
        let inm = inm & act;
        if inm != 0 {
            push_bits(inm, row0 + (b * 32) as u32, |i| i as u32, &mut rec.in_rows[0]);
        }
        let keep = act & !(inm | outm);
        sv.live[b] = keep;
        if keep != 0 {
            push_bits(keep, (b * 32) as u32, |i| i as u32, &mut sv.surv);
        }
    }
    let nsurv = sv.surv.len();
    if verify {
        rec.surv1.extend(sv.surv.iter().map(|&i| row0 + i));
    }
    let t1 = Instant::now();
    tl[0] += (t1 - t0).as_nanos() as f64;
    if nsurv == 0 {
        tl[1] += t1.elapsed().as_nanos() as f64;
        return;
    }
    let cbase = d.codes[1].as_ptr().add(c.b0 * LROW);
    let sbase = d.scal[1].as_ptr().add(c.b0 * SROW);
    if nsurv * 2 >= nb * 32 {
        for b in 0..nb {
            let live = sv.live[b];
            if live == 0 {
                continue;
            }
            let se = _mm256_loadu_si256(sv.sc.as_ptr().add(b * 32) as *const _);
            let so = _mm256_loadu_si256(sv.sc.as_ptr().add(b * 32 + 16) as *const _);
            let (_, _, inm, outm) = int_block::<SPLIT, KNN>(lut, 1, cbase.add(b * LROW), sbase.add(b * SROW), se, so);
            let inm = inm & live;
            if inm != 0 {
                push_bits(inm, row0 + (b * 32) as u32, |i| i as u32, &mut rec.in_rows[1]);
            }
            let keep = live & !(inm | outm);
            if keep != 0 {
                push_bits(keep, row0 + (b * 32) as u32, |i| i as u32, &mut rec.pend);
            }
        }
    } else {
        let mut sbuf = [0u16; 32];
        let mut k0 = 0;
        while k0 < nsurv {
            let cnt = (nsurv - k0).min(32);
            let lanes = &sv.surv[k0..k0 + cnt];
            plan_gather(lanes, &mut sv.g);
            let (cp, sp) = match sv.g.ident {
                Some(s) => (cbase.add(s * LROW), sbase.add(s * SROW)),
                None => {
                    gather_rows(&sv.g, cbase, LROW, PAIRS, sv.tmp.as_mut_ptr());
                    gather_rows(&sv.g, sbase, SROW, 1, sv.stmp.as_mut_ptr());
                    (sv.tmp.as_ptr(), sv.stmp.as_ptr())
                }
            };
            for i in 0..32 {
                sbuf[i] = if i < cnt {
                    let p = lanes[i] as usize;
                    *sv.sc.get_unchecked((p & !31) + eo_pos(p & 31))
                } else {
                    0
                };
            }
            let (se, so) = to_eo(sbuf.as_ptr());
            let act = if cnt == 32 { u32::MAX } else { (1u32 << cnt) - 1 };
            let (_, _, inm, outm) = int_block::<SPLIT, KNN>(lut, 1, cp, sp, se, so);
            let inm = inm & act;
            if inm != 0 {
                push_bits(inm, row0, |i| lanes[i], &mut rec.in_rows[1]);
            }
            let keep = act & !(inm | outm);
            if keep != 0 {
                push_bits(keep, row0, |i| lanes[i], &mut rec.pend);
            }
            k0 += 32;
        }
    }
    tl[1] += t1.elapsed().as_nanos() as f64;
}

/// The speed ceiling: 8-bit table sums of level one over every block of cluster ci, stored, no decisions.
pub unsafe fn fastscan_cluster(d: &Data, ci: usize, lut: &IntLut, out: &mut [u16]) {
    let c = &d.cl[ci];
    let nb = (c.n + 31) / 32;
    let cbase = d.codes[0].as_ptr().add(c.b0 * LROW);
    for b in 0..nb {
        let (e, o) = unit_sums(cbase.add(b * LROW), lut.hi[0].as_ptr());
        let p = out.as_mut_ptr().add((c.b0 + b) * 32);
        _mm256_storeu_si256(p as *mut _, e);
        _mm256_storeu_si256(p.add(16) as *mut _, o);
    }
}
