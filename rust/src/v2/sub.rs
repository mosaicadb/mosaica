//! Sub-clusters under the clusters (experimental: nothing the default build or the bindings' default queries run reads
//! this module). Each cluster is split into `s` sub-clusters by k-means on its rows (`kmeans`, on the rows in key
//! order, so that every layout below has the same sub-clusters), held in one of three layouts:
//!
//! - `Masked` (a): the rows stay in key order; a sub-cluster is a label per row, and reading one passes over the
//!   cluster's rows (or its visible rows, or its sorted range) keeping those of its label;
//! - `KeyRanges` (b): the rows stay in key order and the sub-clusters are `s` contiguous ranges of it of equal size
//!   (no k-means: free to read, not coherent in space);
//! - `Runs` (c): the build orders each cluster's rows by (sub-cluster, key) (`BuildOptions::sub`), so each sub-cluster
//!   is one contiguous key-sorted run (`Index2::runs`), and the exact paths search every run.
//!
//! `SubState` holds what the approximate k-NN with sub-clusters (`knn_sub`) reads: per sub-cluster an int8 centre, its
//! rows, its RMS radius, and the variances of its rows along its cluster's stored directions (`order::OrderState`).
//! `read_costs` times a sub-cluster's read against its cluster's.

use super::adaptive::*;
use super::q::*;
use super::*;
use crate::kernel::Stats;
use rand::{Rng, SeedableRng};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Sub-clusters per cluster, as in the replays (`hierarchy/replay.md`).
pub const S: usize = 16;
/// Lloyd iterations of `kmeans` at most (it stops earlier when no label changes).
pub const KM_ITERS: usize = 20;

/// Labels in 0..s of the m rows `x` (m x n, f32) by k-means: k-means++ seeding from a generator seeded by `seed`, then
/// Lloyd's iterations, the distances from one f32 product of the rows with the centres; a centre left with no row keeps
/// its place. Deterministic for the same rows in the same order on the same machine. With m <= s, row i is label i.
pub fn kmeans(x: &[f32], n: usize, s: usize, seed: u64) -> Vec<u8> {
    assert!((1..=255).contains(&s), "1 to 255 sub-clusters, not {s}");
    let m = if n == 0 { 0 } else { x.len() / n };
    if m <= s {
        return (0..m).map(|i| i as u8).collect();
    }
    let row = |i: usize| &x[i * n..(i + 1) * n];
    let d2 = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(&u, &v)| (u as f64 - v as f64) * (u as f64 - v as f64)).sum::<f64>();
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5b);
    let mut cen = vec![0f32; s * n];
    let i0 = rng.gen_range(0..m);
    cen[..n].copy_from_slice(row(i0));
    let mut near: Vec<f64> = (0..m).map(|i| d2(row(i), &cen[..n])).collect();
    for j in 1..s {
        let tot: f64 = near.iter().sum();
        let pick = if tot > 0.0 {
            let u = rng.gen::<f64>() * tot;
            let mut acc = 0.0;
            near.iter().position(|&d| {
                acc += d;
                acc >= u
            }).unwrap_or(m - 1)
        } else {
            rng.gen_range(0..m)
        };
        cen[j * n..(j + 1) * n].copy_from_slice(row(pick));
        for (i, d) in near.iter_mut().enumerate() {
            *d = d.min(d2(row(i), &cen[j * n..(j + 1) * n]));
        }
    }
    let mut lab = vec![u8::MAX; m];
    let mut dots = vec![0f32; m * s];
    for _ in 0..KM_ITERS {
        let cn: Vec<f32> = cen.chunks_exact(n).map(|c| c.iter().map(|&v| v * v).sum()).collect();
        // SAFETY: x is m x n, cen s x n (read as its n x s transpose) and dots m x s, row-major with the strides given
        unsafe {
            matrixmultiply::sgemm(m, n, s, 1.0, x.as_ptr(), n as isize, 1, cen.as_ptr(), 1, n as isize, 0.0, dots.as_mut_ptr(), s as isize, 1);
        }
        let mut changed = false;
        for i in 0..m {
            let d = &dots[i * s..(i + 1) * s];
            let mut best = 0;
            for j in 1..s {
                if cn[j] - 2.0 * d[j] < cn[best] - 2.0 * d[best] {
                    best = j;
                }
            }
            if lab[i] != best as u8 {
                lab[i] = best as u8;
                changed = true;
            }
        }
        if !changed {
            break;
        }
        let mut sum = vec![0f64; s * n];
        let mut cnt = vec![0usize; s];
        for i in 0..m {
            let j = lab[i] as usize;
            cnt[j] += 1;
            for (o, &v) in sum[j * n..(j + 1) * n].iter_mut().zip(row(i)) {
                *o += v as f64;
            }
        }
        for j in 0..s {
            if cnt[j] > 0 {
                for (o, &v) in cen[j * n..(j + 1) * n].iter_mut().zip(&sum[j * n..(j + 1) * n]) {
                    *o = (v / cnt[j] as f64) as f32;
                }
            }
        }
    }
    lab
}

/// The rows by (label, position): item j of the order is row perm[j], each label's rows keeping their order; and each
/// label's count.
pub fn run_order(lab: &[u8], s: usize) -> (Vec<usize>, Vec<usize>) {
    let mut cnt = vec![0usize; s];
    for &l in lab {
        cnt[l as usize] += 1;
    }
    let mut perm: Vec<usize> = (0..lab.len()).collect();
    perm.sort_by_key(|&i| lab[i]);
    (perm, cnt)
}

/// How the sub-clusters sit in the rows (the module's comment).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Masked,
    KeyRanges,
    Runs,
}

impl Layout {
    pub fn parse(s: &str) -> Result<Layout, String> {
        match s {
            "masked" | "a" => Ok(Layout::Masked),
            "keyranges" | "b" => Ok(Layout::KeyRanges),
            "runs" | "c" => Ok(Layout::Runs),
            _ => Err(format!("a sub-cluster layout is masked, keyranges or runs, not {s:?}")),
        }
    }
}

/// The sub-clusters of every cluster (the module's comment). Sub-cluster j of cluster ci is entry ci * s + j.
pub struct SubState {
    pub layout: Layout,
    pub s: usize,
    /// per row, its sub-cluster in its cluster
    pub lab: Vec<u8>,
    /// `KeyRanges` and `Runs`: k x (s + 1) row offsets, sub-cluster j of cluster ci the rows
    /// [ranges[ci (s + 1) + j], ranges[ci (s + 1) + j + 1]); empty for `Masked`
    pub ranges: Vec<u32>,
    /// int8 centres (k s x n) and their scales
    pub cen8: Vec<i8>,
    pub csc: Vec<f32>,
    /// rows
    pub cnt: Vec<u32>,
    /// RMS radius about the int8 centre
    pub rad: Vec<f64>,
    /// k s x r: the variance of the sub-cluster's rows about its centre along each of its cluster's r stored directions
    pub lam: Vec<f32>,
    /// the variance per other spanned dimension: what the radius holds beyond those, over the n0 - r others
    pub rest: Vec<f64>,
    /// seconds it took to compute
    pub secs: f64,
    /// what expanding a cluster costs on this machine, in microseconds: its sub-clusters' rows counted and their
    /// centres scored with the core's caches evicted first (`measure_expansion`), as a query meets a cluster it has not
    /// read
    pub e_us: f64,
}

impl SubState {
    /// The sub-clusters of `ix` in `layout`, `s` per cluster (`Runs`: the index's own, which must be `s` per cluster).
    pub fn new(ix: &Index2, layout: Layout, s: usize) -> Result<SubState, String> {
        let t = Timer::new();
        let (n, k) = (ix.n(), ix.k());
        let st = ix.order.as_ref().ok_or("sub-clusters need the order state (an index with stored vectors)")?;
        let r = st.r;
        if layout == Layout::Runs {
            match ix.runs(0) {
                Some(w) if w.len() == s + 1 => {}
                _ => return Err(format!("layout runs needs an index built with {s} sub-clusters per cluster (BuildOptions::sub)")),
            }
        }
        if !(1..=32).contains(&s) {
            return Err(format!("1 to 32 sub-clusters (a read's set of them is a 32-bit mask), not {s}"));
        }
        struct Part {
            lab: Vec<u8>,
            ranges: Vec<u32>,
            cen8: Vec<i8>,
            csc: Vec<f32>,
            cnt: Vec<u32>,
            rad: Vec<f64>,
            lam: Vec<f32>,
            rest: Vec<f64>,
        }
        let parts: Vec<Part> = (0..k).into_par_iter().map(|ci| {
            let c = &ix.cl[ci];
            let m = c.r1 - c.r0;
            let mut x = vec![0f32; m * n];
            for (i, xi) in x.chunks_exact_mut(n.max(1)).enumerate() {
                for (o, &h) in xi.iter_mut().zip(ix.full_row_f16(c.r0 + i)) {
                    *o = f16_to_f32(h);
                }
            }
            let (lab, ranges) = match layout {
                Layout::Masked => (kmeans(&x, n, s, ci as u64), Vec::new()),
                Layout::KeyRanges => {
                    let b: Vec<u32> = (0..=s).map(|j| (c.r0 + j * m / s) as u32).collect();
                    let mut lab = vec![0u8; m];
                    for j in 0..s {
                        lab[b[j] as usize - c.r0..b[j + 1] as usize - c.r0].fill(j as u8);
                    }
                    (lab, b)
                }
                Layout::Runs => {
                    let b = ix.runs(ci).unwrap().to_vec();
                    let mut lab = vec![0u8; m];
                    for j in 0..s {
                        lab[b[j] as usize - c.r0..b[j + 1] as usize - c.r0].fill(j as u8);
                    }
                    (lab, b)
                }
            };
            // each sub-cluster's mean, in row order, as int8 with a scale
            let mut sum = vec![0f64; s * n];
            let mut cnt = vec![0u32; s];
            for i in 0..m {
                let j = lab[i] as usize;
                cnt[j] += 1;
                for (o, &v) in sum[j * n..(j + 1) * n].iter_mut().zip(&x[i * n..(i + 1) * n]) {
                    *o += v as f64;
                }
            }
            let (mut cen8, mut csc, mut cq) = (vec![0i8; s * n], vec![0f32; s], vec![0f64; s * n]);
            for j in 0..s {
                let mean: Vec<f64> = sum[j * n..(j + 1) * n].iter().map(|&v| v / (cnt[j].max(1)) as f64).collect();
                let sc = mean.iter().fold(0f64, |a, &v| a.max(v.abs())) / 127.0;
                csc[j] = sc as f32;
                for (d, &v) in mean.iter().enumerate() {
                    let q = if sc > 0.0 { (v / sc).round().clamp(-127.0, 127.0) } else { 0.0 };
                    cen8[j * n + d] = q as i8;
                    cq[j * n + d] = q * csc[j] as f64;
                }
            }
            // the radius about the int8 centre, and the variances along the cluster's directions
            let dirs: Vec<f64> = (0..r).flat_map(|d| {
                let sc = st.dsc[ci * r + d] as f64;
                st.dirs(ci, r, n)[d * n..(d + 1) * n].iter().map(move |&v| v as f64 * sc).collect::<Vec<_>>()
            }).collect();
            let (mut ss, mut lam) = (vec![0f64; s], vec![0f64; s * r]);
            let mut y = vec![0f64; n];
            for i in 0..m {
                let j = lab[i] as usize;
                for ((o, &v), &u) in y.iter_mut().zip(&x[i * n..(i + 1) * n]).zip(&cq[j * n..(j + 1) * n]) {
                    *o = v as f64 - u;
                }
                ss[j] += y.iter().map(|v| v * v).sum::<f64>();
                for d in 0..r {
                    let p: f64 = y.iter().zip(&dirs[d * n..(d + 1) * n]).map(|(a, b)| a * b).sum();
                    lam[j * r + d] += p * p;
                }
            }
            let mut rad = vec![0f64; s];
            let mut rest = vec![0f64; s];
            for j in 0..s {
                let w = cnt[j].max(1) as f64;
                rad[j] = (ss[j] / w).sqrt();
                let mut top = 0.0;
                for d in 0..r {
                    lam[j * r + d] /= w;
                    top += lam[j * r + d];
                }
                rest[j] = (ss[j] / w - top).max(0.0) / (st.n0.max(r + 1) - r) as f64;
            }
            Part { lab, ranges, cen8, csc, cnt, rad, lam: lam.iter().map(|&v| v as f32).collect(), rest }
        }).collect();
        let mut out = SubState { layout, s, lab: Vec::with_capacity(ix.nrows), ranges: Vec::new(), cen8: Vec::new(), csc: Vec::new(), cnt: Vec::new(), rad: Vec::new(), lam: Vec::new(), rest: Vec::new(), secs: 0.0, e_us: 0.0 };
        for p in parts {
            out.lab.extend(p.lab);
            out.ranges.extend(p.ranges);
            out.cen8.extend(p.cen8);
            out.csc.extend(p.csc);
            out.cnt.extend(p.cnt);
            out.rad.extend(p.rad);
            out.lam.extend(p.lam);
            out.rest.extend(p.rest);
        }
        out.e_us = out.measure_expansion(ix);
        out.secs = t.s();
        Ok(out)
    }

    /// The cost of expanding a cluster (`e_us`): for 32 clusters spread over the index, the fastest of 3 timings of
    /// counting its sub-clusters' rows and scoring their centres for a corpus row as the query, each after reading
    /// twice the core's L2; the median over the clusters, at least 0.01 us.
    fn measure_expansion(&self, ix: &Index2) -> f64 {
        let (n, k, s) = (ix.n(), ix.k(), self.s);
        if k == 0 || ix.nrows == 0 {
            return 0.01;
        }
        let evict: Vec<u64> = (0..evict_bytes() as u64 / 8).collect();
        let q: Vec<f32> = ix.full_row_f16(ix.nrows / 2).iter().map(|&h| f16_to_f32(h)).collect();
        let (mut nvs, mut sco) = (vec![0usize; s], vec![0f32; s]);
        let mut t: Vec<f64> = (0..32.min(k)).map(|i| i * k / 32.min(k)).map(|ci| {
            (0..3).map(|_| {
                std::hint::black_box(evict.iter().fold(0u64, |a, &x| a.wrapping_add(x)));
                let t0 = std::time::Instant::now();
                self.counts(ix, ci, None, &mut nvs);
                self.scores(ci, n, &q, &mut sco);
                std::hint::black_box((&nvs, &sco));
                t0.elapsed().as_secs_f64() * 1e6
            }).fold(f64::INFINITY, f64::min)
        }).collect();
        t.sort_by(f64::total_cmp);
        t[t.len() / 2].max(0.01)
    }

    /// The bytes of what the layout needs beyond the default index: the labels (`Masked`, at 4 bits a row for 16),
    /// the row offsets (`KeyRanges`, `Runs`), and per sub-cluster its int8 centre and scale, rows, radius, and its
    /// variances along its cluster's directions with their rest (the approximate queries' state).
    pub fn bytes(&self, n: usize) -> (usize, usize) {
        let layout = match self.layout {
            Layout::Masked => self.lab.len() * (8 - (self.s - 1).leading_zeros() as usize % 8).max(1) / 8,
            _ => self.ranges.len() * 4,
        };
        let r = if self.cnt.is_empty() { 0 } else { self.lam.len() / self.cnt.len() };
        (layout, self.cnt.len() * (n + 4 + 4 + 8 + 4 * r + 8))
    }

    /// The row range of sub-cluster j of cluster ci (`KeyRanges`, `Runs`).
    #[inline]
    pub fn range(&self, ci: usize, j: usize) -> (usize, usize) {
        let w = self.s + 1;
        (self.ranges[ci * w + j] as usize, self.ranges[ci * w + j + 1] as usize)
    }

    /// Visible rows of each sub-cluster of cluster ci.
    pub fn counts(&self, ix: &Index2, ci: usize, vis: Option<&Vis2>, out: &mut [usize]) {
        let s = self.s;
        match (vis, self.layout) {
            (None, _) => out.iter_mut().zip(&self.cnt[ci * s..(ci + 1) * s]).for_each(|(o, &c)| *o = c as usize),
            (Some(v), Layout::Masked) => {
                out.fill(0);
                let c = &ix.cl[ci];
                v.each(c.r0, c.r1, |r| out[self.lab[r] as usize] += 1);
            }
            (Some(v), _) => {
                for (j, o) in out.iter_mut().enumerate() {
                    let (a, b) = self.range(ci, j);
                    *o = v.count(a, b);
                }
            }
        }
    }

    /// The sub-centres' scores q.m_j of cluster ci (f32 products of the int8 rows).
    pub fn scores(&self, ci: usize, n: usize, q32: &[f32], out: &mut [f32]) {
        let s = self.s;
        unsafe { crate::kernel::gemv_i8(q32, &self.cen8[ci * s * n..(ci + 1) * s * n], n, &mut out[..s]) };
        for (o, &c) in out[..s].iter_mut().zip(&self.csc[ci * s..(ci + 1) * s]) {
            *o *= c;
        }
    }

    /// Calls `f` with the selection that reads sub-cluster j of cluster ci.
    pub fn with_sel<T>(&self, ix: &Index2, ci: usize, j: usize, f: impl FnOnce(Sel) -> T) -> T {
        self.with_set(ix, ci, 1 << j, f)
    }

    /// Calls `f` with the selection that reads the sub-clusters `set` (bit j for sub-cluster j) of cluster ci in one
    /// read: under a mask the cluster's rows of those labels; else their ranges (adjacent key ranges of `KeyRanges`
    /// merged, which are key-sorted together; the runs of `Runs` are not).
    pub fn with_set<T>(&self, ix: &Index2, ci: usize, set: u32, f: impl FnOnce(Sel) -> T) -> T {
        match self.layout {
            Layout::Masked => {
                let c = &ix.cl[ci];
                f(Sel { ranges: &[(c.r0, c.r1)], mask: Some((&self.lab, set)) })
            }
            _ => {
                let mut r: Vec<(usize, usize)> = Vec::with_capacity(self.s);
                for j in (0..self.s).filter(|&j| set >> j & 1 == 1) {
                    let (a, b) = self.range(ci, j);
                    match r.last_mut() {
                        Some(x) if self.layout == Layout::KeyRanges && x.1 == a => x.1 = b,
                        _ => r.push((a, b)),
                    }
                }
                f(Sel { ranges: &r, mask: None })
            }
        }
    }

    /// The rows a read of sub-cluster j passes over on the index path (the cluster's under a mask).
    pub fn span(&self, ix: &Index2, ci: usize, j: usize) -> usize {
        self.span_set(ix, ci, 1 << j)
    }

    /// The rows a read of the sub-clusters `set` passes over on the index path (the cluster's under a mask).
    pub fn span_set(&self, ix: &Index2, ci: usize, set: u32) -> usize {
        match self.layout {
            Layout::Masked => ix.cl[ci].r1 - ix.cl[ci].r0,
            _ => (0..self.s).filter(|&j| set >> j & 1 == 1).map(|j| {
                let (a, b) = self.range(ci, j);
                b - a
            }).sum(),
        }
    }
}

/// How a sub-cluster's spread along the query is taken (`knn_sub`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SigmaKind {
    /// its cluster's sigma_c(q) times rad_s / rad_c: no state beyond the radius
    Scaled,
    /// its own variances along its cluster's stored directions and its rest: sigma_s^2 = sum_d lam_sd p_d^2 +
    /// rest_s (|q|^2 - sum_d p_d^2), p the query's projections on the cluster's directions (computed for sigma_c)
    Own,
}

impl SigmaKind {
    pub fn parse(s: &str) -> Result<SigmaKind, String> {
        match s {
            "scaled" => Ok(SigmaKind::Scaled),
            "own" => Ok(SigmaKind::Own),
            _ => Err(format!("a sub-cluster's spread is scaled or own, not {s:?}")),
        }
    }
}

/// What `knn_sub` does with a cluster it splits (`knn_sub`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitMode {
    /// its sub-clusters whose key reaches the running k-th best join the probe, read one by one best-first
    BestFirst,
    /// as `BestFirst`, unless reading those one by one would cost at least what the whole cluster does (read whole)
    Unsplit,
    /// those sub-clusters read at once, in one read of the cluster (the others never), unless that is all of them
    /// (the cluster read whole)
    Filter,
}

impl SplitMode {
    pub fn parse(s: &str) -> Result<SplitMode, String> {
        match s {
            "bestfirst" => Ok(SplitMode::BestFirst),
            "unsplit" => Ok(SplitMode::Unsplit),
            "filter" => Ok(SplitMode::Filter),
            _ => Err(format!("a split is bestfirst, unsplit or filter, not {s:?}")),
        }
    }
}

/// What one `knn_sub` did beyond `AdaptiveInfo`: clusters split, sub-clusters read, sub-centres scored, and clusters
/// whose split did not pay (scored, then read whole).
#[derive(Clone, Debug, Default)]
pub struct SubInfo {
    pub base: AdaptiveInfo,
    pub split: usize,
    pub sub_reads: usize,
    pub sub_scored: usize,
    pub unsplit: usize,
}

/// A sub-cluster's key in a max-heap (entry ci * s + j), the lower entry first among equal keys.
#[derive(PartialEq)]
struct SKey(f64, Reverse<usize>);
impl Eq for SKey {}
impl PartialOrd for SKey {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for SKey {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&o.0).then(self.1.cmp(&o.1))
    }
}

/// The adaptive k-NN (`Adaptive::knn`) with sub-clusters: the same stream, window, stop and race, except that a
/// cluster taken from the window is split when the part of its whole read that a split could save (its cost less the
/// least fixed cost of any read, `Measured::dir_us` or `cl_us`) exceeds what expanding it costs (`SubState::e_us`,
/// measured): the expansion rule of `hierarchy/theory.md` 3.2 (a node holding no more than its expansion costs is read
/// whole), by the measured costs. A split cluster's sub-clusters get the key mu_s + psi sigma_s (`SigmaKind`); those
/// holding a visible row whose key reaches the running k-th best are read as `SplitMode` says: best-first, from a heap
/// beside the window (the probe reads the largest key of the window and that heap, and stops when both are below the
/// running k-th best), or at once. A sub-cluster's read (or a set's) costs what a cluster's of its visible rows does
/// (the index path: its per-cluster part once per cluster, its per-row part over the rows it passes,
/// `SubState::span`). The race scans every row not read, sub-clusters included.
#[allow(clippy::too_many_arguments)]
pub fn knn_sub(ad: &Adaptive, ix: &Index2, ss: &SubState, q: &[f32], kk: usize, vis: Option<&Vis2>, psi: f64, plan: Plan, kind: SigmaKind, mode: SplitMode) -> (KnnOut2, SubInfo) {
    knn_sub_with(ad, ix, ss, q, kk, vis, psi, plan, kind, mode, false)
}

/// `knn_sub`, with `force` splitting every cluster taken from the window and reading its sub-clusters (for tests).
#[allow(clippy::too_many_arguments)]
fn knn_sub_with(ad: &Adaptive, ix: &Index2, ss: &SubState, q: &[f32], kk: usize, vis: Option<&Vis2>, psi: f64, plan: Plan, kind: SigmaKind, mode: SplitMode, force: bool) -> (KnnOut2, SubInfo) {
    assert!(kk > 0 && psi >= 0.0 && psi.is_finite(), "k at least 1 and psi finite and at least 0");
    let tm = Timer::new();
    let t0 = tick();
    let (k, nl, n, m) = (ix.k(), ix.meta.nl(), ix.n(), &ad.m);
    let s = ss.s;
    let pr = prep2(ix, q);
    let mut st = Stats::new(nl);
    let nv_of = |ci: usize| vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize);
    let live: Vec<usize> = (0..k).filter(|&ci| nv_of(ci) > 0).collect();
    let nvis: usize = live.iter().map(|&ci| nv_of(ci)).sum();
    let fixed = m.i8_us * (k + ad.st.r) as f64;
    let mut info = SubInfo { base: AdaptiveInfo { fixed, scan_cost: m.scan(nvis), ..AdaptiveInfo::default() }, ..SubInfo::default() };
    let out = |ents: Vec<(f64, u32)>, mut st: Stats| {
        st.cyc[P_QUERY] += tick() - t0;
        KnnOut2 { ents, st, t_seed: 0.0, t_sweep: tm.ms(), t_sel: 0.0, npow: ix.meta.npow }
    };
    if nvis == 0 {
        info.base.path = "empty";
        return (out(Vec::new(), st), info);
    }
    let cs = info.base.scan_cost;
    if nvis <= kk || fixed >= cs {
        info.base.path = "scan";
        info.base.cost = cs;
        st.full_reads += nvis as u64;
        st.touched += nvis as u64;
        return (out(scan_knn(ix, &pr.q16, &pr.q32, pr.qn, kk, vis), st), info);
    }
    let qm = ad.centres.centre_scores(ix, &pr.q32);
    let qq: f64 = pr.q32.iter().map(|&x| x as f64 * x as f64).sum();
    let r = ad.st.r;
    let mut p = vec![0f32; r];
    let sscale = ad.stream_scale(&pr.q32, &mut p);
    let mut stream: BinaryHeap<Key> = live.iter().map(|&ci| Key(qm[ci] + psi * ad.st.rad[ci] * sscale, Reverse(ci))).collect();
    let mut window: BinaryHeap<Key> = BinaryHeap::with_capacity(LOOKAHEAD);
    let mut subs: BinaryHeap<SKey> = BinaryHeap::new();
    // per priced cluster, its sigma and the query's projections on its directions
    let mut priced: std::collections::HashMap<usize, (f64, Vec<f32>)> = std::collections::HashMap::new();
    let sc = ad.sigma_cost();
    let e = ss.e_us;
    // the least fixed cost of any one read: what a split cannot save
    let fmin = m.dir_us.min(m.cl_us);
    let mut spent = fixed;
    let mut ch = Charges::new(m, ix, &pr, !ix.nukey.is_empty());
    let mut visited = vec![false; k];
    // per split cluster, its sub-clusters holding a visible row not yet read (bit j), and each one's visible rows
    let mut unread: std::collections::HashMap<usize, (u32, Vec<u32>)> = std::collections::HashMap::new();
    let mut setup = vec![false; k];
    let mut raced = false;
    let (mut nvs, mut sco) = (vec![0usize; s], vec![0f32; s]);
    let mut rd = KnnReader::new(ix, &pr, vis, kk, plan, st);
    let anchor = |ch: &mut Charges, read: &[u32]| {
        if !ch.anchored {
            let among: Vec<usize> = read.iter().map(|&c| c as usize).collect();
            pr.anc_among(ix, Some(&among));
            ch.anchored = true;
        }
    };
    loop {
        let kth = rd.kth();
        while window.len() < LOOKAHEAD && !stream.is_empty() {
            if spent + sc > cs {
                raced = true;
                break;
            }
            let Key(_, Reverse(ci)) = stream.pop().unwrap();
            let sg = ad.sigma(ix, ci, &pr.q32, qq, &mut p);
            priced.insert(ci, (sg, p.clone()));
            spent += sc;
            info.base.spreads += 1;
            info.base.priced.push(ci as u32);
            window.push(Key(qm[ci] + psi * sg, Reverse(ci)));
        }
        if raced {
            break;
        }
        let (wk, sk) = (window.peek().map_or(f64::NEG_INFINITY, |x| x.0), subs.peek().map_or(f64::NEG_INFINITY, |x| x.0));
        if (window.is_empty() && subs.is_empty()) || kth > wk.max(sk) {
            break;
        }
        if sk > wk || (sk == wk && !subs.is_empty() && window.is_empty()) {
            let SKey(_, Reverse(id)) = subs.pop().unwrap();
            let (ci, j) = (id / s, id % s);
            let nvj = unread[&ci].1[j] as usize;
            let span = ss.span(ix, ci, j);
            let idx = if setup[ci] { 0.0 } else { m.cl_us } + m.size_us * span as f64 + m.vis_us * nvj as f64
                + if ch.anchored { 0.0 } else { m.anc_us * Adaptive::anchors(ix, info.base.read.len() + 1) as f64 };
            let (path, cost, gnew) = ch.choose_with(ci, idx, m.direct(nvj));
            if spent + cost > cs {
                raced = true;
                break;
            }
            if path == KnnPath::Index {
                anchor(&mut ch, &info.base.read);
                setup[ci] = true;
                info.base.index += 1;
                info.base.groups += gnew as usize;
            } else {
                info.base.direct += 1;
            }
            spent += cost;
            info.sub_reads += 1;
            if let Some(u) = unread.get_mut(&ci) {
                u.0 &= !(1u32 << j);
            }
            ss.with_sel(ix, ci, j, |sel| rd.visit_sel(ci, sel, path, true));
            continue;
        }
        let Key(_, Reverse(ci)) = window.pop().unwrap();
        let nv = nv_of(ci);
        let rows = ix.cl[ci].r1 - ix.cl[ci].r0;
        let whole = m.direct(nv).min(m.index(rows, nv));
        if (whole - fmin > e && s > 1) || force {
            if spent + e > cs {
                raced = true;
                break;
            }
            spent += e;
            info.sub_scored += s;
            ss.counts(ix, ci, vis, &mut nvs);
            ss.scores(ci, n, &pr.q32, &mut sco);
            let (sg, pc) = &priced[&ci];
            let pp: f64 = pc.iter().enumerate().map(|(d, &x)| {
                let y = x as f64 * ad.st.dsc[ci * r + d] as f64;
                y * y
            }).sum();
            let mut cand: Vec<(f64, usize)> = Vec::new();
            let mut all = 0u32;
            for j in 0..s {
                if nvs[j] == 0 {
                    continue;
                }
                all |= 1 << j;
                let e_ = ci * s + j;
                let sig = match kind {
                    SigmaKind::Scaled => if ad.st.rad[ci] > 0.0 { sg * ss.rad[e_] / ad.st.rad[ci] } else { 0.0 },
                    SigmaKind::Own => {
                        let mut v = 0f64;
                        for (d, &x) in pc.iter().enumerate() {
                            let y = x as f64 * ad.st.dsc[ci * r + d] as f64;
                            v += ss.lam[e_ * r + d] as f64 * y * y;
                        }
                        (v + ss.rest[e_] * (qq - pp).max(0.0)).max(0.0).sqrt()
                    }
                };
                let u = sco[j] as f64 + psi * sig;
                if u >= kth {
                    cand.push((u, j));
                }
            }
            if mode == SplitMode::Filter {
                let set = cand.iter().fold(0u32, |a, &(_, j)| a | 1 << j);
                if set != all || force {
                    // the sub-clusters whose key reaches the k-th best, in one read; the rest never
                    let nvset: usize = cand.iter().map(|&(_, j)| nvs[j]).sum();
                    info.split += 1;
                    unread.insert(ci, (all, nvs.iter().map(|&v| v as u32).collect()));
                    if set == 0 {
                        continue;
                    }
                    let idx = m.cl_us + m.size_us * ss.span_set(ix, ci, set) as f64 + m.vis_us * nvset as f64
                        + if ch.anchored { 0.0 } else { m.anc_us * Adaptive::anchors(ix, info.base.read.len() + 1) as f64 };
                    let (path, cost, gnew) = ch.choose_with(ci, idx, m.direct(nvset));
                    if spent + cost > cs {
                        raced = true;
                        break;
                    }
                    if path == KnnPath::Index {
                        anchor(&mut ch, &info.base.read);
                        info.base.index += 1;
                        info.base.groups += gnew as usize;
                    } else {
                        info.base.direct += 1;
                    }
                    spent += cost;
                    info.sub_reads += cand.len();
                    unread.get_mut(&ci).unwrap().0 &= !set;
                    ss.with_set(ix, ci, set, |sel| rd.visit_sel(ci, sel, path, false));
                    continue;
                }
            }
            // the sub-clusters' reads one by one against the whole cluster's (each sub-cluster at its cheaper path; the
            // per-cluster setup of the index path once)
            let mut first = !setup[ci];
            let cost_subs: f64 = cand.iter().map(|&(_, j)| {
                let idx = if first { m.cl_us } else { 0.0 } + m.size_us * ss.span(ix, ci, j) as f64 + m.vis_us * nvs[j] as f64;
                let d = m.direct(nvs[j]);
                if idx < d {
                    first = false;
                }
                idx.min(d)
            }).sum();
            if mode == SplitMode::BestFirst || (mode == SplitMode::Unsplit && cost_subs < whole) || force {
                info.split += 1;
                unread.insert(ci, (all, nvs.iter().map(|&v| v as u32).collect()));
                for (u, j) in cand {
                    subs.push(SKey(u, Reverse(ci * s + j)));
                }
                continue;
            }
            info.unsplit += 1;
        }
        let (path, cost, gnew) = ch.choose(ci, nv, Adaptive::anchors(ix, info.base.read.len() + 1));
        if spent + cost > cs {
            raced = true;
            break;
        }
        info.base.read.push(ci as u32);
        if path == KnnPath::Index {
            anchor(&mut ch, &info.base.read);
            info.base.index += 1;
            info.base.groups += gnew as usize;
        } else {
            info.base.direct += 1;
        }
        spent += cost;
        visited[ci] = true;
        rd.visit(ci, path);
    }
    info.base.path = "probe";
    if raced {
        info.base.path = "race";
        for &ci in &live {
            if visited[ci] {
                continue;
            }
            match unread.get(&ci) {
                None => {
                    rd.visit(ci, KnnPath::Direct);
                    spent += m.scan(nv_of(ci));
                }
                Some(&(u, ref nvj)) => {
                    if u != 0 {
                        ss.with_set(ix, ci, u, |sel| rd.visit_sel(ci, sel, KnnPath::Direct, false));
                        spent += m.scan((0..s).filter(|&j| u >> j & 1 == 1).map(|j| nvj[j] as usize).sum());
                    }
                }
            }
        }
    }
    info.base.cost = spent;
    let (ents, st) = rd.finish();
    (out(ents, st), info)
}

/// One sample of `read_costs`: a cluster near the query, read whole and by its sub-clusters, each timing the fastest
/// of `T_REPS` after evicting the core's caches (as `adaptive::measure` times reads): `rows` visible rows of the cluster,
/// `whole` its direct and index-path read in microseconds; `one_*` the sub-cluster of best centre score; `four_*` the
/// four of best centre score read one after another (the index path's per-cluster setup kept between them).
#[derive(Clone, Debug, Default)]
pub struct ReadSample {
    pub ci: usize,
    pub rows: usize,
    pub whole_dir: f64,
    pub whole_idx: f64,
    pub one_rows: usize,
    pub one_dir: f64,
    pub one_idx: f64,
    pub four_rows: usize,
    pub four_dir: f64,
    pub four_idx: f64,
}

const T_REPS: usize = 3;

fn evict_bytes() -> usize {
    let size = |i: usize| -> Option<(u32, usize)> {
        let p = format!("/sys/devices/system/cpu/cpu0/cache/index{i}");
        let level = std::fs::read_to_string(format!("{p}/level")).ok()?.trim().parse().ok()?;
        let s = std::fs::read_to_string(format!("{p}/size")).ok()?;
        let s = s.trim();
        let (num, mul) = match s.strip_suffix('K') {
            Some(x) => (x, 1 << 10),
            None => match s.strip_suffix('M') {
                Some(x) => (x, 1 << 20),
                None => (s, 1),
            },
        };
        Some((level, num.parse::<usize>().ok()? * mul))
    };
    2 * (0..8).filter_map(size).filter(|&(l, _)| l == 2).map(|x| x.1).max().unwrap_or(2 << 20)
}

/// The read costs of sub-clusters against their clusters' (`ReadSample`), for each query over the `top` clusters of
/// largest centre score holding a visible row, at the running k-th best of a k-NN at k = 10 over those clusters (as
/// `adaptive::measure` takes it). The groups and the anchor are set up before any timing.
pub fn read_costs(ix: &Index2, ss: &SubState, queries: &[Vec<f32>], vis: Option<&Vis2>, top: usize) -> Vec<ReadSample> {
    let (n, k, nl, s) = (ix.n(), ix.k(), ix.meta.nl(), ss.s);
    let plan = Plan::default();
    let evict: Vec<u64> = (0..evict_bytes() as u64 / 8).collect();
    let nv_of = |ci: usize| vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize);
    let fastest = |f: &mut dyn FnMut() -> f64| (0..T_REPS).map(|_| f()).fold(f64::INFINITY, f64::min);
    let mut out = Vec::new();
    for q in queries {
        let pr = prep2(ix, q);
        let mut live: Vec<(f64, usize)> = (0..k).filter(|&ci| nv_of(ci) > 0).map(|ci| (ix.qm(&pr.q64, ci), ci)).collect();
        live.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let cl: Vec<usize> = live.iter().take(top).map(|x| x.1).collect();
        let mut rd = KnnReader::new(ix, &pr, vis, 10, plan, Stats::new(nl));
        for &ci in &cl {
            rd.visit(ci, KnnPath::Direct);
        }
        let t = rd.finish().0.last().map_or(f64::NEG_INFINITY, |x| x.0);
        pr.qms(ix, Some(&cl));
        pr.anc_among(ix, Some(&cl));
        let mut gs: Vec<usize> = cl.iter().map(|&ci| ix.cl[ci].g).collect();
        gs.sort_unstable();
        gs.dedup();
        setup_groups(ix, &pr, &gs, &mut Stats::new(nl));
        let mut sco = vec![0f32; s];
        let mut nvs = vec![0usize; s];
        for &ci in &cl {
            ss.counts(ix, ci, vis, &mut nvs);
            ss.scores(ci, n, &pr.q32, &mut sco);
            let mut by: Vec<usize> = (0..s).filter(|&j| nvs[j] > 0).collect();
            by.sort_by(|&a, &b| sco[b].total_cmp(&sco[a]).then(a.cmp(&b)));
            by.truncate(4);
            let time = |f: &mut dyn FnMut(&mut KnnReader)| -> f64 {
                fastest(&mut || {
                    std::hint::black_box(evict.iter().fold(0u64, |a, &x| a.wrapping_add(x)));
                    let mut r = KnnReader::new(ix, &pr, vis, 10, plan, Stats::new(nl));
                    r.prime(t);
                    let t1 = std::time::Instant::now();
                    f(&mut r);
                    r.kth();
                    t1.elapsed().as_secs_f64() * 1e6
                })
            };
            let mut smp = ReadSample { ci, rows: nv_of(ci), ..ReadSample::default() };
            smp.whole_dir = time(&mut |r| r.visit(ci, KnnPath::Direct));
            smp.whole_idx = time(&mut |r| r.visit(ci, KnnPath::Index));
            if let Some(&j) = by.first() {
                smp.one_rows = nvs[j];
                smp.one_dir = time(&mut |r| ss.with_sel(ix, ci, j, |sel| r.visit_sel(ci, sel, KnnPath::Direct, true)));
                smp.one_idx = time(&mut |r| ss.with_sel(ix, ci, j, |sel| r.visit_sel(ci, sel, KnnPath::Index, true)));
                smp.four_rows = by.iter().map(|&j| nvs[j]).sum();
                smp.four_dir = time(&mut |r| for &j in &by {
                    ss.with_sel(ix, ci, j, |sel| r.visit_sel(ci, sel, KnnPath::Direct, true));
                });
                smp.four_idx = time(&mut |r| for &j in &by {
                    ss.with_sel(ix, ci, j, |sel| r.visit_sel(ci, sel, KnnPath::Index, true));
                });
            }
            out.push(smp);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use roaring::RoaringBitmap;

    fn synth(sub: usize) -> Index2 {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
        build(dir, &BuildOptions { sub, ..BuildOptions::default() }, &mut |_| {}).unwrap().1
    }

    /// Queries near stored rows and in random directions, and visible sets (random shares, whole clusters, three
    /// entities, none), as `e2e::exact_lines` draws them.
    #[allow(clippy::type_complexity)]
    fn queries_and_sets(ix: &Index2) -> (Vec<Vec<f32>>, Vec<Option<RoaringBitmap>>) {
        let n = ix.n();
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(99);
        let unit = |v: Vec<f32>| {
            let s = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            v.into_iter().map(|x| x / s).collect::<Vec<f32>>()
        };
        let mut qs = Vec::new();
        for _ in 0..6 {
            let r = rng.gen_range(0..ix.nrows);
            let x: Vec<f32> = ix.full_row_f16(r).iter().map(|&h| f16_to_f32(h)).collect();
            qs.push(unit(x.iter().map(|v| v + 0.5 * rng.gen_range(-1.0f32..1.0) / (n as f32).sqrt()).collect()));
        }
        for _ in 0..3 {
            qs.push(unit((0..n).map(|_| rng.gen::<f32>() - 0.5).collect()));
        }
        let mut sets: Vec<Option<RoaringBitmap>> = vec![None];
        for p in [0.01, 0.1, 0.5] {
            sets.push(Some((0..ix.nent as u32).filter(|_| rng.gen::<f64>() < p).collect()));
        }
        sets.push(Some((0..ix.k()).filter(|c| c % 3 == 1).flat_map(|c| ix.cl[c].r0..ix.cl[c].r1).map(|r| ix.row_ent[r]).collect()));
        sets.push(Some([5u32, 77, 2000].into_iter().collect()));
        sets.push(Some(RoaringBitmap::new()));
        (qs, sets)
    }

    fn plans() -> [Plan; 5] {
        [Plan::default(), Plan { mode: PlanMode::Index, ..Plan::default() }, Plan { mode: PlanMode::Flat, ..Plan::default() }, Plan { mode: PlanMode::Scan, ..Plan::default() }, Plan { r_rows: 0, r_kappa: 0.0, r_seed: 0, ..Plan::default() }]
    }

    /// Both layouts' exact answers at one query: k-NN at k 1, 10 and 100 (entities and scores) and threshold queries
    /// at answer sizes 1 to 1,000 and beyond every row (entities), every plan, every decided row's bounds checked
    /// against its vector. Counts the threshold queries and the clusters cut into runs.
    #[allow(clippy::too_many_arguments)]
    fn same_answers(a: &Index2, c: &Index2, va: Option<&Vis2>, vc: Option<&Vis2>, q: &[f32], what: &str, nruns: &mut usize, nthr: &mut usize) {
        for kk in [1, 10, 100] {
            for pl in plans() {
                let (x, y) = (knn2(a, q, kk, va, pl), knn2(c, q, kk, vc, pl));
                assert_eq!(x.ents, y.ents, "{what}: k {kk}, plan {:?}", pl.mode);
            }
        }
        let pr = prep2(a, q);
        let mut sc: Vec<f64> = (0..a.nrows).filter(|&r| va.is_none_or(|v| v.has(r))).map(|r| exact_value2(a, &pr, r)).collect();
        sc.sort_by(|x, y| y.total_cmp(x));
        let mut ths: Vec<f64> = [0usize, 9, 99, 999].iter().filter_map(|&i| sc.get(i).copied()).collect();
        ths.extend([-2.0, 2.0]);
        for th in ths {
            for pl in plans() {
                let (pa, pc) = (prep2(a, q), prep2(c, q));
                let o = |v| Opts2 { vis: v, verify: true, want_iv: true, plan: pl };
                let (x, y) = (threshold2(a, &pa, th, &o(va)), threshold2(c, &pc, th, &o(vc)));
                assert_eq!(to_bitmap2(a, &x.rows), to_bitmap2(c, &y.rows), "{what}: t {th}, plan {:?}", pl.mode);
                assert_eq!((y.st.viol_det, y.st.viol_rand), (0, 0), "{what}: t {th}: a bound failed");
                *nthr += 1;
                if let Planned::Run { pre, .. } = plan2(c, &pc, th, &Opts2 { vis: vc, verify: false, want_iv: false, plan: pl }) {
                    *nruns += pre.iter().filter(|p| matches!(p, Pre::Runs { ncand, .. } if *ncand > 0)).count();
                }
            }
        }
    }

    /// Layout (c) holds the default layout's rows, each run key-sorted, and its exact paths give the default layout's
    /// answers (`same_answers`) under every visible set, and so do both layouts compacted to a visible set's rows.
    #[test]
    fn layout_c_answers_as_the_default_layout() {
        let (a, c) = (synth(0), synth(S));
        assert!(a.runs(0).is_none());
        let mut ents_a: Vec<u32> = a.row_ent.to_vec();
        let mut ents_c: Vec<u32> = c.row_ent.to_vec();
        ents_a.sort_unstable();
        ents_c.sort_unstable();
        assert_eq!(ents_a, ents_c);
        for ci in 0..c.k() {
            let runs = c.runs(ci).unwrap();
            assert_eq!((runs[0] as usize, runs[S] as usize), (c.cl[ci].r0, c.cl[ci].r1));
            assert_eq!((a.cl[ci].r0, a.cl[ci].r1), (c.cl[ci].r0, c.cl[ci].r1));
            for w in runs.windows(2) {
                assert!(c.ahat[w[0] as usize..w[1] as usize].windows(2).all(|x| x[0] <= x[1]), "cluster {ci}: a run out of key order");
            }
            assert!(runs[S] - runs[0] < 2 || runs.windows(2).filter(|w| w[1] > w[0]).count() > 1, "cluster {ci}: one run");
        }
        let (qs, sets) = queries_and_sets(&a);
        let (mut nruns, mut nthr) = (0, 0);
        for (si, s) in sets.iter().enumerate() {
            let (va, vc) = (s.as_ref().map(|b| Vis2::new(&a, b)), s.as_ref().map(|b| Vis2::new(&c, b)));
            for (qi, q) in qs.iter().enumerate() {
                same_answers(&a, &c, va.as_ref(), vc.as_ref(), q, &format!("set {si}, query {qi}"), &mut nruns, &mut nthr);
            }
            if let (Some(va), Some(vc)) = (&va, &vc) {
                let (ca, cc) = (a.compact(&va.rows, false), c.compact(&vc.rows, false));
                assert!(cc.runs(0).is_some());
                for (qi, q) in qs.iter().enumerate().take(3) {
                    same_answers(&ca, &cc, None, None, q, &format!("set {si} compacted, query {qi}"), &mut nruns, &mut nthr);
                }
            }
        }
        assert!(nruns > 0 && nthr > 0, "no cluster cut into runs");
    }

    /// The masked layout's k-means on the default index puts each entity in the sub-cluster layout (c) puts it in; the
    /// key ranges are equal splits of each cluster's rows.
    #[test]
    fn every_layout_holds_the_same_subclusters() {
        let (a, c) = (synth(0), synth(S));
        let (sa, sc) = (SubState::new(&a, Layout::Masked, S).unwrap(), SubState::new(&c, Layout::Runs, S).unwrap());
        let mut lab_c = vec![0u8; c.nent];
        for r in 0..c.nrows {
            lab_c[c.row_ent[r] as usize] = sc.lab[r];
        }
        for r in 0..a.nrows {
            assert_eq!(sa.lab[r], lab_c[a.row_ent[r] as usize], "row {r}");
        }
        assert_eq!(sa.cnt, sc.cnt);
        for (x, y) in sa.rad.iter().zip(&sc.rad) {
            assert!((x - y).abs() <= 1e-12 * (1.0 + x.abs()), "{x} {y}");
        }
        let sb = SubState::new(&a, Layout::KeyRanges, S).unwrap();
        for ci in 0..a.k() {
            let m = a.cl[ci].r1 - a.cl[ci].r0;
            for j in 0..S {
                let (x, y) = sb.range(ci, j);
                assert!((y - x) * S <= m + S && (y - x + 1) * S >= m);
                assert!((x..y).all(|r| sb.lab[r] == j as u8));
            }
        }
        assert!(SubState::new(&a, Layout::Runs, S).is_err());
    }

    /// A cluster read by its sub-clusters one after another (directly, and on the index path at a running k-th best
    /// with the cluster's setup kept between them) finds what a whole read finds, in every layout, unfiltered and
    /// under visible sets.
    #[test]
    fn subcluster_reads_find_what_whole_reads_find() {
        let (a, c) = (synth(0), synth(S));
        let (qs, sets) = queries_and_sets(&a);
        let mut n = 0;
        for (ix, layouts) in [(&a, vec![Layout::Masked, Layout::KeyRanges]), (&c, vec![Layout::Runs])] {
            for layout in layouts {
                let ss = SubState::new(ix, layout, S).unwrap();
                for s in &sets {
                    let vis = s.as_ref().map(|b| Vis2::new(ix, b));
                    for q in &qs {
                        let pr = prep2(ix, q);
                        for ci in 0..ix.k() {
                            let mut whole = KnnReader::new(ix, &pr, vis.as_ref(), 5, Plan::default(), Stats::new(ix.meta.nl()));
                            whole.visit(ci, KnnPath::Direct);
                            let w = whole.finish().0;
                            // a running k-th best below the cluster's 5th best (or none)
                            let t = w.last().map(|x| x.0 - 0.05);
                            for path in [KnnPath::Direct, KnnPath::Index] {
                                let mut rd = KnnReader::new(ix, &pr, vis.as_ref(), 5, Plan::default(), Stats::new(ix.meta.nl()));
                                if let Some(t) = t {
                                    rd.prime(t);
                                }
                                for j in 0..S {
                                    ss.with_sel(ix, ci, j, |sel| rd.visit_sel(ci, sel, path, true));
                                }
                                let got: Vec<u32> = rd.finish().0.into_iter().map(|x| x.1).filter(|&e| e != u32::MAX).collect();
                                assert_eq!(got, w.iter().map(|x| x.1).collect::<Vec<_>>(), "{layout:?} cluster {ci} {path:?}");
                                n += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!(n > 0);
    }

    /// The approximate k-NN with sub-clusters returns k items whenever k are visible, and the exact answer when psi is
    /// so large that every sub-cluster holding a visible row is read, in every layout and either spread, unfiltered and
    /// under visible sets, with the index path dear and cheap; the adaptive k-NN on layout (c) reads every cluster's
    /// runs (psi infinite: exact).
    #[test]
    fn knn_sub_is_exact_at_large_psi() {
        let (a, c) = (synth(0), synth(S));
        let m = Measured { i8_us: 0.017, row_us: 0.047, dir_us: 1.2, cl_us: 15.0, size_us: 0.005, vis_us: 0.014, grp_us: 56.0, anc_us: 0.07 };
        let cheap = Measured { cl_us: 0.01, size_us: 0.0, vis_us: 1e-4, grp_us: 0.01, anc_us: 1e-4, ..m };
        let (qs, sets) = queries_and_sets(&a);
        let ids = |v: &[(f64, u32)]| v.iter().map(|x| x.1).collect::<Vec<_>>();
        let (mut split, mut subidx, mut raced) = (0, 0, 0);
        for (ix, layouts) in [(&a, vec![Layout::Masked, Layout::KeyRanges]), (&c, vec![Layout::Runs])] {
            for mm in [m, cheap] {
                let ad = Adaptive::new(ix, mm).unwrap();
                for layout in &layouts {
                    let ss = SubState::new(ix, *layout, S).unwrap();
                    for s in &sets {
                        let vis = s.as_ref().map(|b| Vis2::new(ix, b));
                        let nv = vis.as_ref().map_or(ix.nrows, |v| v.rows.len());
                        for q in &qs {
                            let exact = knn2(ix, q, 10, vis.as_ref(), Plan::default()).ents;
                            for (kind, mode) in [(SigmaKind::Scaled, SplitMode::BestFirst), (SigmaKind::Own, SplitMode::Unsplit), (SigmaKind::Scaled, SplitMode::Filter)] {
                                for force in [false, true] {
                                    let (o, i) = knn_sub_with(&ad, ix, &ss, q, 10, vis.as_ref(), 1e9, Plan::default(), kind, mode, force);
                                    subidx += (i.sub_reads > 0 && i.base.index > 0) as usize;
                                    assert_eq!(ids(&o.ents), ids(&exact), "{layout:?} {kind:?} force {force}, path {}", i.base.path);
                                    // every read dear: the race scans what is left, split clusters' sub-clusters included
                                    let dear = Adaptive::new(ix, Measured { dir_us: 60.0, cl_us: 60.0, ..mm }).unwrap();
                                    let (o, i) = knn_sub_with(&dear, ix, &ss, q, 10, vis.as_ref(), 4.5, Plan::default(), kind, mode, force);
                                    raced += (i.base.path == "race" && i.split > 0) as usize;
                                    if i.base.path == "race" {
                                        assert_eq!(ids(&o.ents), ids(&exact), "{layout:?} {kind:?} force {force}, raced");
                                    }
                                }
                                for psi in [0.0, 2.0, 4.5] {
                                    let (o, i) = knn_sub(&ad, ix, &ss, q, 10, vis.as_ref(), psi, Plan::default(), kind, mode);
                                    split += i.split;
                                    assert_eq!(o.ents.len(), nv.min(10));
                                }
                            }
                            if *layout == Layout::Runs {
                                let (o, _) = ad.knn(ix, q, 10, vis.as_ref(), f64::INFINITY, Plan::default());
                                assert_eq!(ids(&o.ents), ids(&exact));
                            }
                        }
                    }
                }
            }
        }
        assert!(split > 0 && subidx > 0 && raced > 0, "{split} clusters split, {subidx} queries read a sub-cluster on the index path, {raced} raced after a split");
    }
}
