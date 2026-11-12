//! AVX-512 kernels. A block is 16 consecutive items of one cluster; lane i is item i. Each stored bit of the code is one
//! u16 bit-plane over the block, so a level of b bytes per item is 8b u16 words per block. Decoding a coordinate that goes
//! from depth bo to bn assembles each lane's bn-bit cell index from its planes (earlier planes sit in earlier levels'
//! arrays of the same block) and looks the step up in a 16/32-entry register table (vpermps / vpermt2ps; a gather above
//! 32 entries). Accumulators: A = sum of query weights x steps, B = sum of centre weights x steps (query-independent, so
//! shared by every query of a batch), and for the randomised blocks also N = sum of squared-centroid steps.
use crate::index::*;
use crate::query::CQ;
use std::arch::x86_64::*;

pub const MAXQ: usize = 64;

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
}
impl Stats {
    pub fn new(nl: usize) -> Stats {
        Stats { enter: vec![0; nl + 1], dec_in: vec![0; nl + 1], dec_out: vec![0; nl + 1], blocks: vec![0; nl + 1], ..Default::default() }
    }
    pub fn add(&mut self, o: &Stats) {
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
    }
}

/// What the block kernel does at each level.
#[derive(Clone, Copy)]
pub enum Mode {
    /// decide against thresholds; stop each lane at its first decisive level
    Thr,
    /// no decisions; report every lane's interval at this level (k-NN seed and selection)
    Interval(usize),
}

pub struct BlockOut<'a> {
    pub rows_in: &'a mut Vec<u32>,
    pub pend: &'a mut Vec<(u32, u8)>, // (row, query slot) for the full vector
    pub iv: Option<&'a mut Vec<(u32, u8, f32, f32, u8)>>, // (row, slot, lb, ub, level) of decided-in items, or Interval output
    pub truth: Option<&'a [f64]>, // per query slot x 16 lanes exact scores (verification)
    pub rec: Option<&'a mut Vec<(u8, u32, u16)>>, // (level, global block, lanes) entering each level
}

/// All ops of one level part, accumulated into four partial sums per query (and the squared norm, randomised parts).
/// Loops are unrolled by four with constant accumulator indices so the partial sums stay in registers.
#[inline(always)]
unsafe fn ops_part<const NQ: usize>(lo: &LvOps, pl: *const u16, stp: *mut u8, one: __m512i, tree: &Tree, cq: &[&CQ; NQ], wo: usize, act: &[u16; NQ], acc: &mut [[__m512; 4]; NQ], mut nj: Option<&mut __m512>) {
    let mut a = *acc;
    let dt = tree.dtab.as_ptr();
    let ds = tree.dsq.as_ptr();
    macro_rules! addw {
        ($k:expr, $m:expr, $wi:expr) => {
            for q in 0..NQ {
                if NQ == 1 || act[q] != 0 {
                    let w = _mm512_set1_ps(*cq[q].w.get_unchecked($wi));
                    a[q][$k] = _mm512_mask_add_ps(a[q][$k], $m, a[q][$k], w);
                }
            }
        };
    }
    macro_rules! fmaw {
        ($k:expr, $v:expr, $wi:expr) => {
            for q in 0..NQ {
                if NQ == 1 || act[q] != 0 {
                    let w = _mm512_set1_ps(*cq[q].w.get_unchecked($wi));
                    a[q][$k] = _mm512_fmadd_ps($v, w, a[q][$k]);
                }
            }
        };
    }
    macro_rules! ss {
        ($k:expr, $i:expr, $wi:expr) => {{
            let (p, sl) = *lo.ss.get_unchecked($i);
            let m = *pl.add(p as usize);
            _mm512_mask_cvtepi32_storeu_epi8(stp.add(sl as usize * 16) as *mut _, 0xffff, _mm512_maskz_mov_epi32(m, one));
            addw!($k, m, $wi);
        }};
    }
    macro_rules! sp {
        ($k:expr, $i:expr, $wi:expr) => {{
            let m = *pl.add(*lo.sp.get_unchecked($i) as usize);
            addw!($k, m, $wi);
        }};
    }
    macro_rules! m1 {
        ($k:expr, $i:expr, $wi:expr) => {{
            let op = lo.m1.get_unchecked($i);
            let sp = stp.add(op.slot as usize * 16);
            let m = *pl.add(op.pl as usize);
            let mut idx = _mm512_cvtepu8_epi32(_mm_loadu_si128(sp as *const _));
            idx = _mm512_add_epi32(idx, idx);
            idx = _mm512_mask_or_epi32(idx, m, idx, one);
            if op.store {
                _mm512_mask_cvtepi32_storeu_epi8(sp as *mut _, 0xffff, idx);
            }
            let ti = op.bold as usize * 9 + op.bnew as usize;
            let v = _mm512_permutexvar_ps(idx, _mm512_loadu_ps((*dt.add(ti)).as_ptr()));
            if let Some(n) = nj.as_deref_mut() {
                *n = _mm512_add_ps(*n, _mm512_permutexvar_ps(idx, _mm512_loadu_ps((*ds.add(ti)).as_ptr())));
            }
            fmaw!($k, v, $wi);
        }};
    }
    macro_rules! mg {
        ($k:expr, $i:expr, $wi:expr) => {{
            let op = lo.mg.get_unchecked($i);
            let (bn, idx) = refine(op, pl, stp, one);
            let ti = op.bold as usize * 9 + bn;
            let v = tab_at(bn, idx, &*dt.add(ti));
            if let Some(n) = nj.as_deref_mut() {
                *n = _mm512_add_ps(*n, tab_at(bn, idx, &*ds.add(ti)));
            }
            fmaw!($k, v, $wi);
        }};
    }
    macro_rules! run4 {
        ($mac:ident, $len:expr, $wi:expr) => {{
            let len = $len;
            let mut i = 0;
            while i + 4 <= len {
                $mac!(0, i, $wi + i);
                $mac!(1, i + 1, $wi + i + 1);
                $mac!(2, i + 2, $wi + i + 2);
                $mac!(3, i + 3, $wi + i + 3);
                i += 4;
            }
            if i < len {
                $mac!(0, i, $wi + i);
            }
            if i + 1 < len {
                $mac!(1, i + 1, $wi + i + 1);
            }
            if i + 2 < len {
                $mac!(2, i + 2, $wi + i + 2);
            }
        }};
    }
    let mut wi = wo;
    run4!(ss, lo.ss.len(), wi);
    wi += lo.ss.len();
    run4!(sp, lo.sp.len(), wi);
    wi += lo.sp.len();
    run4!(m1, lo.m1.len(), wi);
    wi += lo.m1.len();
    run4!(mg, lo.mg.len(), wi);
    *acc = a;
}

/// Extend a coordinate's cell index by the level's new bits (state kept per block, one byte per lane).
#[inline(always)]
unsafe fn refine(op: &MOp, pl: *const u16, stp: *mut u8, one: __m512i) -> (usize, __m512i) {
    let bn = op.bnew as usize;
    let sp = stp.add(op.slot as usize * 16);
    let mut idx = if op.bold == 0 { _mm512_setzero_si512() } else { _mm512_cvtepu8_epi32(_mm_loadu_si128(sp as *const _)) };
    for d in 0..(op.bnew - op.bold) as usize {
        let m = *pl.add(op.pl as usize + d);
        idx = _mm512_add_epi32(idx, idx);
        idx = _mm512_mask_or_epi32(idx, m, idx, one);
    }
    if op.store {
        _mm512_mask_cvtepi32_storeu_epi8(sp as *mut _, 0xffff, idx);
    }
    (bn, idx)
}

#[allow(dead_code)]
#[inline(always)]
unsafe fn lookup_idx(op: &Op, ptrs: &[*const u16; 32]) -> __m512i {
    let bn = op.bnew as usize;
    let mut idx = _mm512_setzero_si512();
    for d in 0..bn {
        let w = *ptrs[op.lv[d] as usize].add(op.pl[d] as usize);
        idx = _mm512_mask_or_epi32(idx, w, idx, _mm512_set1_epi32(1 << (bn - 1 - d)));
    }
    idx
}
#[inline(always)]
unsafe fn tab_at(bn: usize, idx: __m512i, tab: &[f32; 256]) -> __m512 {
    if bn <= 4 {
        _mm512_permutexvar_ps(idx, _mm512_loadu_ps(tab.as_ptr()))
    } else if bn == 5 {
        _mm512_permutex2var_ps(_mm512_loadu_ps(tab.as_ptr()), idx, _mm512_loadu_ps(tab.as_ptr().add(16)))
    } else {
        _mm512_i32gather_ps::<4>(idx, tab.as_ptr() as *const _)
    }
}

#[inline(always)]
unsafe fn nibbles(w: u64) -> __m512i {
    let v = _mm512_set1_epi64(w as i64);
    // lane i takes bits 4i..4i+3: shift the 64-bit word right per lane (two halves of 8 lanes)
    let sh_lo = _mm512_setr_epi64(0, 4, 8, 12, 16, 20, 24, 28);
    let sh_hi = _mm512_setr_epi64(32, 36, 40, 44, 48, 52, 56, 60);
    let a = _mm512_and_si512(_mm512_srlv_epi64(v, sh_lo), _mm512_set1_epi64(15));
    let b = _mm512_and_si512(_mm512_srlv_epi64(v, sh_hi), _mm512_set1_epi64(15));
    let a32 = _mm512_cvtepi64_epi32(a);
    let b32 = _mm512_cvtepi64_epi32(b);
    _mm512_inserti64x4(_mm512_castsi256_si512(a32), b32, 1)
}

#[inline(always)]
unsafe fn arr(v: __m512) -> [f32; 16] {
    let mut o = [0f32; 16];
    _mm512_storeu_ps(o.as_mut_ptr(), v);
    o
}

#[inline(always)]
unsafe fn expand(v: __m512, fb: u16, sp: &Sparse, slot: usize) -> __m512 {
    if fb == 0 {
        return v;
    }
    _mm512_mask_expandloadu_ps(v, fb, sp.val.as_ptr().add(*sp.off.get_unchecked(slot) as usize))
}

/// Run one block for NQ queries that share it (NQ = 1 is the single-query path). act0[q] are the lanes undecided and
/// visible for query q. Decisions go to `out`.
#[target_feature(enable = "avx512f,avx512bw,avx512dq,avx512vl")]
pub unsafe fn run_block<const NQ: usize>(
    ix: &Index,
    ci: usize,
    b: usize,
    cq: &[&CQ; NQ],
    t: &[f32; NQ],
    act0: &[u16; NQ],
    mode: Mode,
    st: &mut Stats,
    out: &mut BlockOut,
) {
    let c = &ix.cl[ci];
    let gp = &ix.groups[c.g];
    let nl = ix.meta.nl();
    let row0 = c.r0 + b * LANES;
    let nvalid = (c.r1 - row0).min(LANES);
    let vmask: u16 = if nvalid == 16 { 0xffff } else { ((1u32 << nvalid) - 1) as u16 };
    let mut act = *act0;
    let mut any: u16 = act.iter().fold(0, |a, &x| a | x);
    if any == 0 {
        return;
    }
    // per-lane scalars from the stored key (same f32 operations as the encoder)
    let ah = if nvalid == 16 {
        _mm512_cvtepi32_ps(_mm512_cvtepi16_epi32(_mm256_loadu_si256(ix.ahat.as_ptr().add(row0) as *const _)))
    } else {
        let mut v = [0f32; 16];
        for i in 0..nvalid {
            v[i] = ix.ahat[row0 + i] as f32;
        }
        _mm512_loadu_ps(v.as_ptr())
    };
    let mn = _mm512_set1_ps(c.mn32);
    let a = _mm512_div_ps(ah, _mm512_set1_ps(32767.0));
    let s2 = _mm512_sub_ps(_mm512_add_ps(_mm512_set1_ps(1.0), _mm512_mul_ps(mn, mn)), _mm512_mul_ps(_mm512_mul_ps(_mm512_set1_ps(2.0), mn), a));
    let s = _mm512_sqrt_ps(_mm512_div_ps(_mm512_max_ps(s2, _mm512_set1_ps(1e-12)), _mm512_set1_ps(gp.lamsum)));
    let mr = _mm512_sub_ps(_mm512_mul_ps(mn, a), _mm512_mul_ps(mn, mn));
    let mut ptrs: [*const u16; 32] = [std::ptr::null(); 32];
    for l in 0..nl {
        ptrs[l] = ix.planes[l].as_ptr().add(ix.lvl_off[l][ci] + b * gp.nplanes[l]);
    }
    let blk = c.blk0 + b;
    let z = _mm512_setzero_ps();
    let mut state = std::mem::MaybeUninit::<[u8; MAXSLOTS * 16]>::uninit();
    let stp = state.as_mut_ptr() as *mut u8;
    let one = _mm512_set1_epi32(1);
    let mut aq = [[z; 4]; NQ];
    let mut ad = [z; NQ]; // s * A at the last deterministic level
    let mut aj = [[[z; 2]; J]; NQ];
    let mut nj = [z; J];
    let mut yh = [z; J];
    let mut yfbm = [0u16; J];
    let mut th = [z; J];
    let mut sh = [z; J];
    let mut dmx = [z; J];
    let alrmr: [__m512; NQ] = core::array::from_fn(|q| _mm512_add_ps(_mm512_set1_ps(cq[q].qm), _mm512_mul_ps(_mm512_set1_ps(cq[q].alr), mr)));
    let ryd = (ry() * (1.0 + 2e-6)) as f32;
    let rhoyd = (rho_y() * (1.0 + 2e-6)) as f32;
    let rt2 = (rho_t() * rho_t()) as f32;
    let csg = _mm512_set1_ps(ix.meta.csg as f32);
    let stop = match mode {
        Mode::Interval(l) => l,
        Mode::Thr => usize::MAX,
    };
    for l in 0..nl {
        if any == 0 {
            break;
        }
        st.enter[l] += act.iter().map(|x| x.count_ones() as u64).sum::<u64>();
        st.blocks[l] += 1;
        if let Some(r) = out.rec.as_deref_mut() {
            r.push((l as u8, blk as u32, act[0]));
        }
        let mut lbv = [z; NQ];
        let mut ubv = [z; NQ];
        let mut est_r = [z; NQ];
        let mut hnr = [z; NQ];
        let mut sdv = [z; NQ];
        let pl = ptrs[l];
        if l < NDL {
            let lo = &gp.lops[l][0];
            let wo = gp.woff[l][0];
            for q in 0..NQ {
                aq[q][0] = _mm512_add_ps(aq[q][0], _mm512_set1_ps(*cq[q].base.get_unchecked(l)));
            }
            let mut acc: [[__m512; 4]; NQ] = aq;
            ops_part::<NQ>(lo, pl, stp, one, &ix.tree, cq, wo, &act, &mut acc, None);
            aq = acc;
            let wd = ix.nrm[l][blk];
            let nib = nibbles(wd);
            let ptab = _mm512_loadu_ps(gp.ntab[l].as_ptr());
            let pred = _mm512_mul_ps(_mm512_set1_ps(gp.pconst[l]), s);
            let rho0 = _mm512_mul_ps(pred, _mm512_permutexvar_ps(nib, ptab));
            let escm = _mm512_cmpeq_epi32_mask(nib, _mm512_set1_epi32(15));
            let rho = expand(rho0, escm, &ix.nesc[l], blk);
            for q in 0..NQ {
                if act[q] == 0 {
                    continue;
                }
                let k = &cq[q];
                let asum = _mm512_add_ps(_mm512_add_ps(aq[q][0], aq[q][1]), _mm512_add_ps(aq[q][2], aq[q][3]));
                let sa = _mm512_mul_ps(s, asum);
                let est = _mm512_add_ps(alrmr[q], sa);
                let h = _mm512_add_ps(_mm512_mul_ps(_mm512_mul_ps(_mm512_set1_ps(k.dwa[l]), rho), _mm512_set1_ps(1.0 + 1e-6)), _mm512_set1_ps(k.slk[l]));
                lbv[q] = _mm512_sub_ps(est, h);
                ubv[q] = _mm512_add_ps(est, h);
                if l == NDL - 1 {
                    ad[q] = sa;
                }
            }
        } else {
            let lr = l - NDL;
            if lr == 0 {
                for j in 0..J {
                    let pr = _mm512_mul_ps(_mm512_set1_ps(gp.ypredw[j]), s);
                    let mut codes = [0i32; 16];
                    for i in 0..nvalid {
                        codes[i] = *ix.yh.get_unchecked((row0 + i) * J + j) as i32;
                    }
                    let cv = _mm512_loadu_si512(codes.as_ptr() as *const _);
                    let fb = _mm512_cmpeq_epi32_mask(cv, _mm512_set1_epi32(255)) & vmask;
                    let tv = _mm512_i32gather_ps::<4>(_mm512_min_epi32(cv, _mm512_set1_epi32(254)), gp.ytab[j].as_ptr() as *const _);
                    yh[j] = expand(_mm512_mul_ps(pr, tv), fb, &ix.yfb, blk * J + j);
                    yfbm[j] = fb;
                }
            }
            for j in 0..J {
                let lo = &gp.lops[l][j];
                if lo.signs.is_empty() && lo.multi.is_empty() {
                    continue;
                }
                let wo = gp.woff[l][j];
                let bi = NDL + lr * J + j;
                nj[j] = _mm512_add_ps(nj[j], _mm512_set1_ps(gp.nsc[lr][j]));
                for q in 0..NQ {
                    aj[q][j][0] = _mm512_add_ps(aj[q][j][0], _mm512_set1_ps(*cq[q].base.get_unchecked(bi)));
                }
                let mut acc: [[__m512; 4]; NQ] = core::array::from_fn(|q| [aj[q][j][0], aj[q][j][1], z, z]);
                ops_part::<NQ>(lo, pl, stp, one, &ix.tree, cq, wo, &act, &mut acc, Some(&mut nj[j]));
                for q in 0..NQ {
                    aj[q][j][0] = _mm512_add_ps(acc[q][0], acc[q][2]);
                    aj[q][j][1] = _mm512_add_ps(acc[q][1], acc[q][3]);
                }
            }
            for (slot, &j) in gp.thchg[lr].iter().enumerate() {
                let wi = ix.th_off[lr][ci] + b * gp.thchg[lr].len() + slot;
                let nib = nibbles(*ix.th[lr].get_unchecked(wi));
                let tv = _mm512_permutexvar_ps(nib, _mm512_loadu_ps(gp.ttab[lr][j].as_ptr()));
                let fb = _mm512_cmpeq_epi32_mask(nib, _mm512_set1_epi32(15));
                th[j] = expand(tv, fb, &ix.tfb[lr], wi);
                // the scale and the scalar-rounding factor change only when the block's rate rises
                let t2 = _mm512_mul_ps(th[j], th[j]);
                let r1 = _mm512_sqrt_ps(_mm512_add_ps(_mm512_set1_ps(1.0), t2));
                sh[j] = _mm512_div_ps(_mm512_mul_ps(yh[j], r1), _mm512_sqrt_ps(nj[j]));
                let rr = _mm512_mask_blend_ps(yfbm[j], _mm512_set1_ps(ryd), _mm512_set1_ps(rhoyd));
                let den = _mm512_sqrt_ps(_mm512_add_ps(_mm512_set1_ps(1.0), _mm512_div_ps(t2, _mm512_set1_ps(rt2))));
                dmx[j] = _mm512_add_ps(_mm512_sub_ps(_mm512_div_ps(_mm512_mul_ps(rr, r1), den), _mm512_set1_ps(1.0)), _mm512_set1_ps(1e-5));
            }
            let rates = &gp.rates[lr];
            for q in 0..NQ {
                if act[q] == 0 {
                    continue;
                }
                let k = &cq[q];
                let mut est = _mm512_add_ps(alrmr[q], ad[q]);
                let mut hdet = z;
                let mut hpar = z;
                let mut var = z;
                for j in 0..J {
                    let nd = _mm512_set1_ps(k.nd[j]);
                    if rates[j] == 0.0 {
                        hdet = _mm512_fmadd_ps(yh[j], nd, hdet);
                        continue;
                    }
                    let eb = _mm512_mul_ps(sh[j], _mm512_add_ps(aj[q][j][0], aj[q][j][1]));
                    est = _mm512_add_ps(est, eb);
                    hpar = _mm512_fmadd_ps(dmx[j], _mm512_abs_ps(eb), hpar);
                    hpar = _mm512_fmadd_ps(sh[j], _mm512_set1_ps(*k.eacc.get_unchecked(lr * J + j)), hpar);
                    let ab = _mm512_mul_ps(_mm512_mul_ps(yh[j], th[j]), nd);
                    var = _mm512_fmadd_ps(_mm512_mul_ps(ab, ab), _mm512_set1_ps(gp.inv_ms[j]), var);
                }
                let sd = _mm512_sqrt_ps(var);
                let hn = _mm512_add_ps(_mm512_add_ps(hdet, hpar), _mm512_set1_ps(k.slkr));
                let h = _mm512_mul_ps(_mm512_fmadd_ps(csg, sd, hn), _mm512_set1_ps(1.0 + 1e-6));
                lbv[q] = _mm512_sub_ps(est, h);
                ubv[q] = _mm512_add_ps(est, h);
                est_r[q] = est;
                hnr[q] = hn;
                sdv[q] = sd;
            }
        }
        // verification: every interval must contain the exact score
        if let Some(tr) = out.truth {
            for q in 0..NQ {
                if act[q] == 0 {
                    continue;
                }
                let (lb, ub) = (arr(lbv[q]), arr(ubv[q]));
                let (er, hn, sd) = (arr(est_r[q]), arr(hnr[q]), arr(sdv[q]));
                for i in 0..16 {
                    if act[q] >> i & 1 == 0 {
                        continue;
                    }
                    let sc = tr[q * 16 + i];
                    st.checked += 1;
                    if sc < lb[i] as f64 - 1e-12 || sc > ub[i] as f64 + 1e-12 {
                        if l < NDL { st.viol_det += 1 } else { st.viol_rand += 1 }
                    }
                    if l >= NDL && sd[i] > 0.0 {
                        let zz = ((sc - er[i] as f64).abs() - hn[i] as f64) / sd[i] as f64;
                        if zz > st.max_sd {
                            st.max_sd = zz;
                        }
                    }
                }
            }
        }
        if l == stop {
            if let Some(iv) = out.iv.as_deref_mut() {
                for q in 0..NQ {
                    let (lb, ub) = (arr(lbv[q]), arr(ubv[q]));
                    for i in 0..16 {
                        if act[q] >> i & 1 == 1 {
                            iv.push(((row0 + i) as u32, q as u8, lb[i], ub[i], l as u8));
                        }
                    }
                }
            }
            return;
        }
        if let Mode::Thr = mode {
            any = 0;
            for q in 0..NQ {
                if act[q] == 0 {
                    continue;
                }
                let tv = _mm512_set1_ps(t[q]);
                let inm = _mm512_cmp_ps_mask::<_CMP_GE_OQ>(lbv[q], tv) & act[q];
                let outm = _mm512_cmp_ps_mask::<_CMP_LT_OQ>(ubv[q], tv) & act[q];
                if inm != 0 {
                    st.dec_in[l] += inm.count_ones() as u64;
                    let lbs = if out.iv.is_some() { Some((arr(lbv[q]), arr(ubv[q]))) } else { None };
                    for i in 0..16 {
                        if inm >> i & 1 == 1 {
                            out.rows_in.push((row0 + i) as u32 | ((q as u32) << 26));
                            if let (Some(iv), Some((lb, ub))) = (out.iv.as_deref_mut(), lbs.as_ref()) {
                                iv.push(((row0 + i) as u32, q as u8, lb[i], ub[i], l as u8));
                            }
                        }
                    }
                }
                st.dec_out[l] += outm.count_ones() as u64;
                act[q] &= !(inm | outm);
                any |= act[q];
            }
        }
    }
    for q in 0..NQ {
        for i in 0..16 {
            if act[q] >> i & 1 == 1 {
                out.pend.push(((row0 + i) as u32, q as u8));
            }
        }
        st.enter[nl] += act[q].count_ones() as u64;
    }
}

// ---------------- flat scans ----------------

/// f16 x f32 dot, 16 lanes, f32 accumulation. |error| <= (n/16 + 4) * 2^-24 * sum |q_i x_i|.
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
    [_mm512_reduce_add_ps(s[0]), _mm512_reduce_add_ps(s[1]), _mm512_reduce_add_ps(s[2]), _mm512_reduce_add_ps(s[3])]
}

/// int8 x int16 dot via vpmaddwd (exactly as the review's baseline).
#[target_feature(enable = "avx512f,avx512bw,avx512dq,avx512vl")]
pub unsafe fn dot_i8(q: &[i16], x: &[i8]) -> i32 {
    let n = q.len();
    let mut s = _mm512_setzero_si512();
    let mut i = 0;
    while i + 32 <= n {
        let xv = _mm512_cvtepi8_epi16(_mm256_loadu_si256(x.as_ptr().add(i) as *const _));
        let qv = _mm512_loadu_si512(q.as_ptr().add(i) as *const _);
        s = _mm512_add_epi32(s, _mm512_madd_epi16(xv, qv));
        i += 32;
    }
    _mm512_reduce_add_epi32(s)
}

/// QV = V^T q for several queries in one pass over a transposed basis (row c of vt is column c of V); f32 products,
/// f64 accumulation.
#[target_feature(enable = "avx512f,avx512bw,avx512dq,avx512vl")]
pub unsafe fn basis_matvec(vt: &[f32], n: usize, qs: &[&[f64]], out: &mut [Vec<f64>]) {
    let nq = qs.len();
    for c in 0..n {
        let row = vt.as_ptr().add(c * n);
        let mut q0 = 0;
        while q0 < nq {
            let nb = (nq - q0).min(4);
            let mut acc = [[_mm512_setzero_pd(); 2]; 4];
            let mut i = 0;
            while i + 16 <= n {
                let v0 = _mm512_cvtps_pd(_mm256_loadu_ps(row.add(i)));
                let v1 = _mm512_cvtps_pd(_mm256_loadu_ps(row.add(i + 8)));
                for b in 0..nb {
                    let qp = qs[q0 + b].as_ptr();
                    acc[b][0] = _mm512_fmadd_pd(v0, _mm512_loadu_pd(qp.add(i)), acc[b][0]);
                    acc[b][1] = _mm512_fmadd_pd(v1, _mm512_loadu_pd(qp.add(i + 8)), acc[b][1]);
                }
                i += 16;
            }
            for b in 0..nb {
                let mut r = _mm512_reduce_add_pd(_mm512_add_pd(acc[b][0], acc[b][1]));
                for k in i..n {
                    r += *row.add(k) as f64 * qs[q0 + b][k];
                }
                out[q0 + b][c] = r;
            }
            q0 += 4;
        }
    }
}
