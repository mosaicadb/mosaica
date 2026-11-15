//! Brute force over the stored f16 vectors (the flat scan, its top-k and the thresholds that give each answer size),
//! the cluster cut's parameters, and the f32 rounding of a threshold.
use crate::kernel::*;
use crate::util::*;
use rayon::prelude::*;
use roaring::RoaringBitmap;

// ---------------- the cluster cut and the sorted range ----------------

pub struct Cut {
    pub cm: f64,
    pub s1: f64,
    pub bh: f64,
    pub bl: f64,
    pub rm: f64,
    pub rp: f64,
}

// ---------------- visibility ----------------

// ---------------- exact full-vector decisions ----------------

// ---------------- the threshold query ----------------

/// f32 threshold such that (x >= result in f32) implies x >= t and (x < result) implies x < t for f32 x.
pub fn next_up_if_below(t32: f32, t: f64) -> f32 {
    if (t32 as f64) >= t {
        // rounding went up: x >= t32 implies x >= t; x < t32 means x <= prev(t32) which may still be >= t.
        // prev(t32) < t holds since t32 is the nearest f32 to t from above unless t is exactly representable.
        t32
    } else {
        // t32 < t: step up one ulp
        if t32 >= 0.0 { f32::from_bits(t32.to_bits() + 1) } else { f32::from_bits(t32.to_bits() - 1) }
    }
}

// ---------------- batched threshold queries (one pass over the resident levels) ----------------

// ---------------- exact k-NN ----------------

// ---------------- flat scans and truth ----------------

pub struct Flat<'a> {
    pub map: &'a [u8],
    pub off: usize,
    pub n: usize,
    pub nrows: usize,
    pub ent: &'a [i64], // numpy row -> entity
}
impl<'a> Flat<'a> {
    #[inline]
    pub fn row(&self, r: usize) -> &[u16] {
        let b = &self.map[self.off + r * self.n * 2..self.off + (r + 1) * self.n * 2];
        unsafe { std::slice::from_raw_parts(b.as_ptr() as *const u16, self.n) }
    }
    /// all f32 scores in numpy row order
    pub fn scores(&self, q32: &[f32]) -> Vec<f32> {
        let mut out = vec![0f32; self.nrows];
        out.par_chunks_mut(4096).enumerate().for_each(|(ch, o)| {
            let r0 = ch * 4096;
            let mut i = 0;
            while i + 4 <= o.len() {
                let s = unsafe { dot_f16_x4(q32, [self.row(r0 + i), self.row(r0 + i + 1), self.row(r0 + i + 2), self.row(r0 + i + 3)]) };
                o[i..i + 4].copy_from_slice(&s);
                i += 4;
            }
            while i < o.len() {
                o[i] = unsafe { dot_f16(q32, self.row(r0 + i)) };
                i += 1;
            }
        });
        out
    }
    /// exact threshold result (entity ids, sorted)
    pub fn threshold(&self, q16: &[u16], q32: &[f32], t: f64, err: f64, vis: Option<&RoaringBitmap>, ent_xs: &[u32]) -> Vec<u32> {
        let decide = |r: usize, s: f32| -> bool {
            let s = s as f64;
            if s >= t + err { true } else if s < t - err { false } else { exact_ge(exact_dot(q16, self.row(r)), t) }
        };
        let rows: Vec<u32> = match vis {
            None => Vec::new(),
            Some(bm) => bm.iter().map(|e| ent_xs[e as usize]).collect(),
        };
        let nr = if vis.is_some() { rows.len() } else { self.nrows };
        let rowof = |i: usize| if vis.is_some() { rows[i] as usize } else { i };
        let mut res: Vec<u32> = (0..(nr + 1023) / 1024)
            .into_par_iter()
            .flat_map_iter(|ch| {
                let (a, b) = (ch * 1024, ((ch + 1) * 1024).min(nr));
                let mut out = Vec::new();
                let mut i = a;
                while i + 4 <= b {
                    let rr = [rowof(i), rowof(i + 1), rowof(i + 2), rowof(i + 3)];
                    let s = unsafe { dot_f16_x4(q32, [self.row(rr[0]), self.row(rr[1]), self.row(rr[2]), self.row(rr[3])]) };
                    for k in 0..4 {
                        if decide(rr[k], s[k]) {
                            out.push(self.ent[rr[k]] as u32);
                        }
                    }
                    i += 4;
                }
                while i < b {
                    let r = rowof(i);
                    if decide(r, unsafe { dot_f16(q32, self.row(r)) }) {
                        out.push(self.ent[r] as u32);
                    }
                    i += 1;
                }
                out
            })
            .collect();
        res.sort_unstable();
        res
    }
    /// exact k-NN (score desc, entity asc)
    pub fn knn(&self, q16: &[u16], q32: &[f32], k: usize, err: f64, vis: Option<&RoaringBitmap>, ent_xs: &[u32]) -> Vec<(f64, u32)> {
        const CH: usize = 1024;
        let rows: Option<Vec<u32>> = vis.map(|bm| bm.iter().map(|e| ent_xs[e as usize]).collect());
        let nr = rows.as_ref().map_or(self.nrows, |r| r.len());
        topk_f16(q16, q32, k, err, (nr + CH - 1) / CH, |ch, out| {
            let (a, b) = (ch * CH, ((ch + 1) * CH).min(nr));
            match &rows {
                Some(r) => out.extend_from_slice(&r[a..b]),
                None => out.extend(a as u32..b as u32),
            }
        }, |r| self.row(r as usize), |r| self.ent[r as usize] as u32)
    }
}

/// Exact k-NN by brute force over rows produced chunk by chunk (`chunk(c, out)` appends the rows of chunk c):
/// f32 scores keep, per chunk, every row within two error bounds of the running k-th best, then exact products decide
/// among those within two error bounds of the k-th. Returns (exact score, id) by descending score, then ascending id.
pub fn topk_f16<'a>(q16: &[u16], q32: &[f32], k: usize, err: f64, nchunks: usize, chunk: impl Fn(usize, &mut Vec<u32>) + Sync,
                    row: impl Fn(u32) -> &'a [u16] + Sync, id: impl Fn(u32) -> u32 + Sync) -> Vec<(f64, u32)> {
    let tops: Vec<Vec<(f32, u32)>> = (0..nchunks)
        .into_par_iter()
        .fold(|| (Vec::new(), Vec::new()), |(mut v, mut rr): (Vec<(f32, u32)>, Vec<u32>), ch| {
            rr.clear();
            chunk(ch, &mut rr);
            // every row whose f32 score is within 2 err of the running k-th best stays: one of them may be a true
            // top-k item however many rows tie with it
            let push = |s: f32, r: u32, v: &mut Vec<(f32, u32)>| {
                if v.len() < k || s as f64 >= v[k - 1].0 as f64 - 2.0 * err {
                    let pos = v.partition_point(|x| x.0 >= s);
                    v.insert(pos, (s, r));
                    if v.len() > k {
                        let floor = v[k - 1].0 as f64 - 2.0 * err;
                        while v.len() > k && (v[v.len() - 1].0 as f64) < floor {
                            v.pop();
                        }
                    }
                }
            };
            let mut i = 0;
            while i + 4 <= rr.len() {
                let r4 = [rr[i], rr[i + 1], rr[i + 2], rr[i + 3]];
                let s = unsafe { dot_f16_x4(q32, r4.map(|r| row(r))) };
                for j in 0..4 {
                    push(s[j], r4[j], &mut v);
                }
                i += 4;
            }
            for &r in &rr[i..] {
                push(unsafe { dot_f16(q32, row(r)) }, r, &mut v);
            }
            (v, rr)
        })
        .map(|x| x.0)
        .collect();
    let mut all: Vec<(f32, u32)> = tops.into_iter().flatten().collect();
    all.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    if all.is_empty() {
        return Vec::new();
    }
    let kth = all[(k - 1).min(all.len() - 1)].0 as f64;
    let mut ex: Vec<(f64, u32)> = all
        .iter()
        .filter(|x| x.0 as f64 >= kth - 2.0 * err)
        .map(|x| (exact_to_f64(exact_dot(q16, row(x.1))), id(x.1)))
        .collect();
    ex.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(&b.1)));
    ex.truncate(k);
    ex
}

/// thresholds for answer sizes (fractions of N), exact: midpoint between the k-th and (k+1)-th exact scores
pub fn thresholds_for(fl: &Flat, q16: &[u16], scores: &[f32], err: f64, sizes: &[usize]) -> Vec<f64> {
    let kmax = *sizes.iter().max().unwrap() + 1;
    let mut s: Vec<(f32, u32)> = scores.iter().enumerate().map(|(i, &v)| (v, i as u32)).collect();
    let nth = kmax.min(s.len() - 1);
    s.select_nth_unstable_by(nth, |a, b| b.0.partial_cmp(&a.0).unwrap());
    let cut = s[nth].0 as f64 - 2.0 * err;
    let mut ex: Vec<f64> = scores
        .par_iter()
        .enumerate()
        .filter(|(_, &v)| v as f64 >= cut)
        .map(|(i, _)| exact_to_f64(exact_dot(q16, fl.row(i))))
        .collect();
    ex.sort_by(|a, b| b.partial_cmp(a).unwrap());
    sizes.iter().map(|&k| (ex[k - 1] + ex[k]) / 2.0).collect()
}
