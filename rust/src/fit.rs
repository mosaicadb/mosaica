//! The fit: from a partition of the vectors (`partition::partition`, or `tools/partition.py`), the parameters the build reads, written as the
//! directory `tools/fit.py` writes, without its copy of the vectors: the build reads them from the partition in the
//! fit's row order (`ord{c}.u32`, `clcell.npy`, `clpos.npy`; `bucket::Buckets`). Every pass goes cell by cell or
//! cluster by cluster; nothing holds the vectors in memory.

use crate::util::*;
use faer::linalg::solvers::DenseSolveCore;
use faer::{Mat, Side};
use rand::distributions::WeightedIndex;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use std::io::Write;

// Design constants (not fitted), as tools/fit.py: the guarantee's N and epsilon for the rate design, the
// deterministic level ends, the randomised level ends up to 256 B, randomised block boundaries as fractions of n, the
// H25 hedge weight, the fit format, the ridge of Sigma_Q.
const NTARGET: f64 = 1e9;
const EPS: f64 = 1e-9;
const DLV: [usize; 6] = [4, 8, 12, 16, 24, 32];
const RLV_BASE: [usize; 4] = [56, 96, 160, 256];
const BLOCK_FRACS: [f64; 3] = [0.125, 0.29167, 0.625];
const HEDGE: f64 = 0.25;
const FORMAT: u64 = 2;
const RIDGE: f64 = 0.1;

/// How the fit divides its clusters into groups (each with a basis of its own).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Groups {
    /// min(64, K / 8, N / (20 n)), at least 1: enough items per group for a basis, at most 64 groups and 8 clusters per
    /// group on average (the default before research/levels-clusters' rule).
    ByK8,
    /// min(64, K, N / (20 n)), at least 1: research/levels-clusters' rule, with its cluster size by N
    /// (`per_cluster_by_n`, given to tools/partition.py as `--per-cluster by-n`); the default, as tools/fit.py's.
    ByN,
    /// This many groups, at most K.
    Fixed(usize),
}
impl Groups {
    /// The number of groups for k clusters of nrow items of n0 dimensions.
    pub fn count(&self, nrow: usize, k: usize, n0: usize) -> usize {
        match *self {
            Groups::ByK8 => 1.max(64.min(k / 8).min(nrow / (20 * n0))),
            Groups::ByN => 1.max(64.min(k).min(nrow / (20 * n0))),
            Groups::Fixed(g) => 1.max(g.min(k)),
        }
    }
    /// by-k8, by-n or a positive number.
    pub fn parse(s: &str) -> Result<Groups, String> {
        match s {
            "by-k8" => Ok(Groups::ByK8),
            "by-n" => Ok(Groups::ByN),
            _ => s.parse::<usize>().ok().filter(|&g| g > 0).map(Groups::Fixed).ok_or(format!("groups is by-k8, by-n or a positive number, not {s}")),
        }
    }
}

/// Items per cluster by research/levels-clusters' rule for N items: 2,000 x sqrt(N / 1M), rounded in log2 to a
/// power-of-two multiple of 2,000 (ties down), at least 2,000 (2,000 up to 2M items, 4,000 up to 8M, 8,000 up to 32M,
/// 64,000 at 1B). The partition (`partition::partition`, tools/partition.py) and tools/fit.py apply it with
/// `--per-cluster by-n`, their default.
pub fn per_cluster_by_n(nrow: usize) -> usize {
    2000 << ((nrow as f64 / 1e6).log2() / 2.0 - 0.5).ceil().max(0.0) as u32
}

/// What a fit takes besides the partition.
pub struct FitOptions {
    /// "dual" (the default) or "pca".
    pub basis: String,
    /// Queries that shape Sigma_Q with the corpus items (the dual basis only).
    pub query_sample: Option<String>,
    pub seed: u64,
    /// Seconds the fit may take to certify the randomised levels' constant when the shipped table has none for its
    /// layout (`certify::certify_layout`; 0: not certified, not even from the table; the build then uses the larger
    /// closed-form bounds).
    pub cert_time: f64,
    /// The support table's anchors (`write_anchors`): `image` from the centre directions, `text_count` from the
    /// prompts in `text_anchors`.
    pub image: usize,
    pub text_anchors: Option<String>,
    pub text_count: usize,
    /// How the clusters are grouped (the default `Groups::ByN`).
    pub groups: Groups,
}
impl Default for FitOptions {
    fn default() -> FitOptions {
        FitOptions { basis: "dual".into(), query_sample: None, seed: 20261112, cert_time: 600.0, image: 1024, text_anchors: None, text_count: 256, groups: Groups::ByN }
    }
}

fn save<T: Copy>(dir: &str, name: &str, descr: &str, shape: &[usize], v: &[T]) -> Result<(), String> {
    // SAFETY: T is a plain number type here (i8, i64, f32, f64); its bytes are its little-endian values
    let b = unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) };
    let path = format!("{dir}/{name}.npy");
    let mut out = npy_header_bytes(descr, shape);
    out.extend_from_slice(b);
    write_new(&path, &out)
}

/// Write `bytes` as `path` through a new file beside it, synced and renamed over it: a reader never sees a part
/// written, and a file `path` named before (or another name of it, a hard link of a `cp -al` copy) is replaced,
/// never written.
fn write_new(path: &str, bytes: &[u8]) -> Result<(), String> {
    let p = std::path::Path::new(path);
    let tmp = beside(p, "tmp");
    let r = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        std::fs::rename(&tmp, p)
    })();
    r.map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("cannot write {path}: {e}")
    })
}

/// A name beside `path` for a temporary file of this process: `.NAME.KIND-PID-NANOS` (`log_stale` reports leftovers).
fn beside(path: &std::path::Path, kind: &str) -> std::path::PathBuf {
    let name = path.file_name().map_or(String::new(), |n| n.to_string_lossy().to_string());
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    path.with_file_name(format!(".{name}.{kind}-{}-{nanos}", std::process::id()))
}

#[cfg(test)]
thread_local! {
    /// Tests: make `replace_files`' step with this index (0 the first rename or removal) fail, on this thread only.
    pub static FAIL_STEP: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// Replace the files `items` (path, new contents; None removes the file) together: every new content is written to a
/// file beside its path and synced first, every old file is kept under a second name (a hard link, so another
/// directory's link to it is never touched), then the renames and removals run back to back. If one fails, the old
/// files are put back and the error says whether that worked; on success the kept names are removed.
fn replace_files(items: &[(String, Option<Vec<u8>>)]) -> Result<(), String> {
    use std::path::{Path, PathBuf};
    let mut tmps: Vec<Option<PathBuf>> = Vec::new();
    let clean = |tmps: &[Option<PathBuf>], baks: &[Option<PathBuf>]| {
        for t in tmps.iter().chain(baks).flatten() {
            let _ = std::fs::remove_file(t);
        }
    };
    for (path, new) in items {
        let t = match new {
            Some(bytes) => {
                let t = beside(Path::new(path), "tmp");
                let r = (|| {
                    let mut f = std::fs::File::create(&t)?;
                    f.write_all(bytes)?;
                    f.sync_all()
                })();
                if let Err(e) = r {
                    let _ = std::fs::remove_file(&t);
                    clean(&tmps, &[]);
                    return Err(format!("cannot write {path}: {e}; nothing was replaced"));
                }
                Some(t)
            }
            None => None,
        };
        tmps.push(t);
    }
    let mut baks: Vec<Option<PathBuf>> = Vec::new();
    for (path, _) in items {
        let p = Path::new(path);
        let b = if p.exists() {
            let b = beside(p, "bak");
            if let Err(e) = std::fs::hard_link(p, &b) {
                clean(&tmps, &baks);
                return Err(format!("cannot keep {path} aside: {e}; nothing was replaced"));
            }
            Some(b)
        } else {
            None
        };
        baks.push(b);
    }
    for (i, (path, _)) in items.iter().enumerate() {
        #[cfg(test)]
        let injected = FAIL_STEP.with(|f| f.get()) == Some(i);
        #[cfg(not(test))]
        let injected = false;
        let r = if injected {
            Err(std::io::Error::other("injected failure"))
        } else {
            match &tmps[i] {
                Some(t) => std::fs::rename(t, path),
                None if Path::new(path).exists() => std::fs::remove_file(path),
                None => Ok(()),
            }
        };
        if let Err(e) = r {
            // put back what steps 0..i changed (step i changed nothing)
            let mut lost = Vec::new();
            for (j, (pj, _)) in items.iter().enumerate().take(i) {
                let back = match &baks[j] {
                    Some(b) => std::fs::rename(b, pj),
                    None => std::fs::remove_file(pj),
                };
                if let Err(e2) = back {
                    lost.push(format!("{pj} ({e2})"));
                }
            }
            clean(&tmps, &baks);
            return Err(if lost.is_empty() {
                format!("cannot replace {path}: {e}; the files were put back as they were")
            } else {
                format!("cannot replace {path}: {e}; and could not put back {}: the files are not as they were", lost.join(", "))
            });
        }
    }
    clean(&[], &baks);
    Ok(())
}

/// The certificate's layout for vectors of n0 dimensions, as the fit makes it (`schedule`, on n0 padded to a multiple
/// of 32 as tools/partition.py pads it): the randomised blocks' dimensions (each block's width less one) and the
/// number of randomised levels, which sets the budget (`certify::ln_delta`).
pub fn cert_layout(n0: usize) -> Result<(Vec<usize>, usize), String> {
    if n0 < 50 {
        return Err(format!("the fit needs at least 50 dimensions, not {n0}"));
    }
    let (rlv, bounds) = schedule(n0, n0.div_ceil(32) * 32)?;
    Ok((block_dims(&bounds), rlv.len()))
}

/// The dimensions of the randomised blocks between `bounds` (certify.py `block_dims`).
fn block_dims(bounds: &[usize]) -> Vec<usize> {
    bounds.windows(2).map(|w| w[1] - w[0] - 1).collect()
}

/// Randomised level ends and block bounds, as tools/fit.py `schedule`.
fn schedule(n0: usize, n: usize) -> Result<(Vec<usize>, Vec<usize>), String> {
    let cap = (0.4 * 2.0 * n0 as f64).min(256.0 * (n0 as f64 / 768.0).max(1.0)).min((DLV[5] + n) as f64);
    let mut rlv: Vec<usize> = RLV_BASE.iter().copied().filter(|&b| b as f64 <= cap).collect();
    if rlv.len() < RLV_BASE.len() {
        let top = (cap as usize / 8 * 8).max(DLV[5] + 8.min(n));
        let prev = *rlv.last().unwrap_or(&DLV[5]);
        if top > prev {
            let mut mids: Vec<usize> = Vec::new();
            loop {
                let last = *mids.last().unwrap_or(&prev);
                if top as f64 / last as f64 <= 1.8 {
                    break;
                }
                let m = ((last as f64 * top as f64).sqrt() / 8.0).round_ties_even() as usize * 8;
                if m <= last || m >= top {
                    break;
                }
                mids.push(m);
            }
            rlv.extend(mids);
            rlv.push(top);
        }
    } else {
        let mut b = RLV_BASE[3] as f64;
        loop {
            b = (b * 1.6 / 32.0).round_ties_even() * 32.0;
            if b > cap {
                break;
            }
            rlv.push(b as usize);
        }
    }
    if rlv.is_empty() {
        return Err(format!("no randomised level fits vectors of {n0} dimensions"));
    }
    let mut bounds = vec![0usize];
    bounds.extend(BLOCK_FRACS.iter().map(|f| (f * n as f64 / 16.0).round_ties_even() as usize * 16));
    bounds.push(n);
    for j in 1..bounds.len() {
        bounds[j] = bounds[j].max(bounds[j - 1] + 16);
    }
    if bounds[bounds.len() - 2] + 16 > n {
        return Err(format!("n = {n} is too small for four randomised blocks"));
    }
    Ok((rlv, bounds))
}

/// Spherical k-means of the rows of `s` (m x n, unit) to k centres, as combined/core.py `kmeans`.
fn kmeans(s: &[f32], n: usize, k: usize, rng: &mut ChaCha8Rng, iters: usize) -> Vec<f32> {
    let m = s.len() / n;
    let pick = rand::seq::index::sample(rng, m, k).into_vec();
    let mut c: Vec<f32> = pick.iter().flat_map(|&i| s[i * n..(i + 1) * n].iter().copied()).collect();
    for _ in 0..iters {
        let a = argmax_rows(s, &c, n);
        let mut sum = vec![0f64; k * n];
        let mut cnt = vec![0usize; k];
        for (i, &j) in a.iter().enumerate() {
            cnt[j] += 1;
            for t in 0..n {
                sum[j * n + t] += s[i * n + t] as f64;
            }
        }
        for j in 0..k {
            if cnt[j] == 0 {
                let r = rng.gen_range(0..m);
                for t in 0..n {
                    sum[j * n + t] = s[r * n + t] as f64;
                }
            }
            let nr = sum[j * n..(j + 1) * n].iter().map(|x| x * x).sum::<f64>().sqrt();
            for t in 0..n {
                c[j * n + t] = (sum[j * n + t] / nr) as f32;
            }
        }
    }
    c
}

/// For each row of `s`, the index of the row of `c` with the largest inner product.
fn argmax_rows(s: &[f32], c: &[f32], n: usize) -> Vec<usize> {
    let k = c.len() / n;
    s.par_chunks(n * 1024).flat_map_iter(|blk| {
        let m = blk.len() / n;
        let mut d = vec![0f32; m * k];
        unsafe { matrixmultiply::sgemm(m, n, k, 1.0, blk.as_ptr(), n as isize, 1, c.as_ptr(), 1, n as isize, 0.0, d.as_mut_ptr(), k as isize, 1) };
        (0..m).map(move |i| (0..k).fold(0, |b, j| if d[i * k + j] > d[i * k + b] { j } else { b }))
    }).collect()
}

fn mat(a: &[f64], n: usize) -> Mat<f64> {
    Mat::from_fn(n, n, |i, j| a[i * n + j])
}

/// Eigenvalues and eigenvectors (columns, row-major n x n) of a symmetric matrix, largest first.
fn eigh_desc(a: &[f64], n: usize) -> Result<(Vec<f64>, Vec<f64>), String> {
    if a.iter().any(|x| !x.is_finite()) {
        return Err("a covariance has a non-finite entry (non-finite vectors?)".into());
    }
    let e = mat(a, n).self_adjoint_eigen(Side::Lower).map_err(|e| format!("symmetric eigendecomposition failed: {e:?}"))?;
    let (s, u) = (e.S().column_vector(), e.U());
    let mut o: Vec<usize> = (0..n).collect();
    o.sort_by(|&x, &y| s[y].total_cmp(&s[x]));
    let vals = o.iter().map(|&i| s[i]).collect();
    let mut vecs = vec![0f64; n * n];
    for (c, &i) in o.iter().enumerate() {
        for r in 0..n {
            vecs[r * n + c] = u[(r, i)];
        }
    }
    Ok((vals, vecs))
}

fn matmul(a: &[f64], b: &[f64], n: usize) -> Vec<f64> {
    let mut c = vec![0f64; n * n];
    unsafe { matrixmultiply::dgemm(n, n, n, 1.0, a.as_ptr(), n as isize, 1, b.as_ptr(), n as isize, 1, 0.0, c.as_mut_ptr(), n as isize, 1) };
    c
}

/// The f64 basis the build makes from V (f32, row-major, columns are basis vectors): its int8 copy with one scale per
/// column.
fn int8_basis64(v: &[f32], n: usize) -> Vec<f64> {
    let (v8t, vsc) = crate::v2::int8_basis(v, n);
    let mut q = vec![0f64; n * n];
    for i in 0..n {
        for c in 0..n {
            q[i * n + c] = v8t[c * n + i] as f64 * vsc[c];
        }
    }
    q
}

/// Covariance of the offsets q - (q.m^) m^ of unit queries `qs` from 40 centres each, drawn with weights `w`, as
/// tools/fit.py `offset_cov`.
fn offset_cov(qs: &[f32], mh: &[f32], n: usize, w: &WeightedIndex<usize>, rng: &mut ChaCha8Rng) -> Vec<f64> {
    let nq = qs.len() / n;
    let draws: Vec<usize> = (0..nq * 40).map(|_| w.sample(rng)).collect();
    let parts: Vec<Vec<f64>> = (0..nq).collect::<Vec<_>>().par_chunks(200).map(|qi| {
        let mut d = vec![0f64; qi.len() * 40 * n];
        for (a, &i) in qi.iter().enumerate() {
            let q = &qs[i * n..(i + 1) * n];
            for t in 0..40 {
                let m = &mh[draws[i * 40 + t] * n..(draws[i * 40 + t] + 1) * n];
                let dot: f32 = q.iter().zip(m).map(|(a, b)| a * b).sum();
                let row = &mut d[(a * 40 + t) * n..(a * 40 + t + 1) * n];
                for k in 0..n {
                    row[k] = (q[k] - dot * m[k]) as f64;
                }
            }
        }
        let rows = qi.len() * 40;
        let mut s = vec![0f64; n * n];
        unsafe { matrixmultiply::dgemm(n, rows, n, 1.0, d.as_ptr(), 1, n as isize, d.as_ptr(), n as isize, 1, 0.0, s.as_mut_ptr(), n as isize, 1) };
        s
    }).collect();
    let mut s = vec![0f64; n * n];
    for p in parts {
        s.iter_mut().zip(p).for_each(|(a, b)| *a += b);
    }
    let c = (nq * 40) as f64;
    s.iter_mut().for_each(|x| *x /= c);
    s
}

fn trace(a: &[f64], n: usize) -> f64 {
    (0..n).map(|i| a[i * n + i]).sum()
}

/// Fit the partition in `pdir` (written by `partition::partition`, or `python3 -I tools/partition.py VECTORS.npy DIR`) into the directory `out`,
/// which the build then reads (`v2::rebuild`).
pub fn fit(out: &str, pdir: &str, o: &FitOptions, logf: &mut dyn FnMut(String)) -> Result<(), String> {
    let t = Timer::new();
    let mut log = |_: &Timer, s: String| logf(format!("[{:6.0}s] {s}", t.s()));
    let pdir = pdir.to_string();
    let basis = o.basis.clone();
    if basis != "pca" && basis != "dual" {
        return Err(format!("the basis is pca or dual, not {basis}"));
    }
    let sample = o.query_sample.clone();
    if sample.is_some() && basis != "dual" {
        return Err("a query sample shapes Sigma_Q, which only the dual basis uses".into());
    }
    let (seed, cert_time) = (o.seed, o.cert_time);
    std::fs::create_dir_all(out).map_err(|e| format!("cannot create {out}: {e}"))?;
    // meta.json is written last: until then the directory is not a fit (an earlier fit's is removed first)
    let meta_path = format!("{out}/meta.json");
    if std::path::Path::new(&meta_path).exists() {
        std::fs::remove_file(&meta_path).map_err(|e| format!("cannot remove {meta_path}: {e}"))?;
    }
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    let part = crate::bucket::Partition::open(&pdir)?;
    // the partition's identity (its content) and its manifest's file identity (a build warns when only that changed)
    let partition_file = crate::v2::store::XsId::of(&format!("{pdir}/manifest.json"))?;
    let partition_id = crate::bucket::partition_id(&pdir)?;
    let (nrow, n0, n) = (part.rows, part.n_orig, part.n);
    if n0 < 50 {
        return Err(format!("the vectors have {n0} dimensions; the randomised levels need at least 50 (the first ends at 40 bytes, which must be within 40% of the f16 vector). Pad the vectors with zero columns to 50 or more."));
    }
    let ncell = part.cells.len();
    log(&t, format!("N {nrow} n {n0} (padded to {n}); {} fine centres in {ncell} cell(s) from {pdir}", part.k));

    // pass 1, cell by cell: each cell's rows listed by fine cluster (`ord{c}.u32`, a cluster's rows in cell order), and
    // every cluster's exact mean, stored int8 with one scale
    let mut cnt0 = vec![0usize; part.k];
    let mut pos0 = vec![0usize; part.k];
    let mut mean0: Vec<(Vec<i8>, f64)> = vec![(Vec::new(), 1.0); part.k];
    let mut placed = 0usize;
    for (c, cell) in part.cells.iter().enumerate() {
        if let Some(i) = (0..cell.rows()).find(|&i| cell.cluster(i) as usize >= part.k) {
            return Err(format!("{pdir}/cell{c}: row {i} names cluster {}, beyond the partition's {}; partition the vectors again", cell.cluster(i), part.k));
        }
        let mut loc: Vec<(u32, u32)> = (0..cell.rows()).filter_map(|i| {
            let k = cell.cluster(i);
            (part.cluster_cell[k as usize] as usize == c).then_some((k, i as u32))
        }).collect();
        loc.sort_unstable();
        let ord: Vec<u8> = loc.iter().flat_map(|x| x.1.to_le_bytes()).collect();
        let path = format!("{out}/ord{c}.u32");
        std::fs::write(&path, ord).map_err(|e| format!("cannot write {path}: {e}"))?;
        placed += loc.len();
        let mut runs = Vec::new();
        let mut s = 0;
        while s < loc.len() {
            let e = s + loc[s..].partition_point(|x| x.0 == loc[s].0);
            runs.push((loc[s].0 as usize, s, e));
            s = e;
        }
        let means: Vec<(usize, usize, (Vec<i8>, f64))> = runs.par_iter().map(|&(k, s, e)| {
            let mut sum = vec![0f64; n];
            for &(_, i) in &loc[s..e] {
                sum.iter_mut().zip(cell.vector(n, i as usize)).for_each(|(a, &h)| *a += f16_to_f32(h) as f64);
            }
            let mean: Vec<f64> = sum.iter().map(|v| v / (e - s) as f64).collect();
            let mut sc = mean.iter().fold(0f64, |a, v| a.max(v.abs())) / 127.0;
            if sc == 0.0 {
                sc = 1.0;
            }
            (k, e - s, (mean.iter().map(|v| (v / sc).round_ties_even() as i8).collect(), sc))
        }).collect();
        for (i, (k, m, q)) in means.into_iter().enumerate() {
            (cnt0[k], pos0[k], mean0[k]) = (m, runs[i].1, q);
        }
    }
    if placed != nrow {
        return Err(format!("the partition in {pdir} places {placed} rows in its cells' clusters, not its {nrow} rows; partition the vectors again"));
    }
    let keep: Vec<usize> = (0..part.k).filter(|&c| cnt0[c] > 0).collect();
    let k = keep.len();
    let mq8: Vec<i8> = keep.iter().flat_map(|&c| mean0[c].0.iter().copied()).collect();
    let msc: Vec<f64> = keep.iter().map(|&c| mean0[c].1).collect();
    drop(mean0);
    let mq: Vec<f32> = (0..k * n).map(|i| (mq8[i] as f64 * msc[i / n]) as f32).collect();
    let mh: Vec<f32> = mq.chunks(n).flat_map(|m| {
        let nr = m.iter().map(|&v| v * v).sum::<f32>().sqrt();
        m.iter().map(move |&v| if nr > 0.0 { v / nr } else { v })
    }).collect();

    // groups (`Groups`): by default enough items per group for a basis (20 n), at most 64 groups and 8 clusters per
    // group on average
    let g0 = o.groups.count(nrow, k, n0);
    let gc = kmeans(&mh, n, g0, &mut rng, 20);
    let graw = argmax_rows(&mh, &gc, n);
    let mut gk: Vec<usize> = graw.clone();
    gk.sort();
    gk.dedup();
    let mut gmap = vec![usize::MAX; g0];
    gk.iter().enumerate().for_each(|(i, &g)| gmap[g] = i);
    let ng = gk.len();
    let grp: Vec<usize> = graw.iter().map(|&g| gmap[g]).collect();

    // clusters numbered group by group (a query visits clusters in this order and builds each one's tables from its
    // group's), rows of a cluster in cell order; the fit's rows are found through each cluster's cell and position
    let mut order: Vec<usize> = (0..k).collect();
    order.sort_by_key(|&i| (grp[i], i));
    let grp: Vec<usize> = order.iter().map(|&i| grp[i]).collect();
    let mq8: Vec<i8> = order.iter().flat_map(|&i| mq8[i * n..(i + 1) * n].iter().copied()).collect();
    let msc: Vec<f64> = order.iter().map(|&i| msc[i]).collect();
    let mq: Vec<f32> = order.iter().flat_map(|&i| mq[i * n..(i + 1) * n].iter().copied()).collect();
    let mh: Vec<f32> = order.iter().flat_map(|&i| mh[i * n..(i + 1) * n].iter().copied()).collect();
    let fine: Vec<usize> = order.iter().map(|&i| keep[i]).collect();
    let cnt: Vec<usize> = fine.iter().map(|&f| cnt0[f]).collect();
    let mut cstart = vec![0i64; k + 1];
    for c in 0..k {
        cstart[c + 1] = cstart[c] + cnt[c] as i64;
    }
    save(out, "cstart", "<i8", &[k + 1], &cstart)?;
    save(out, "clcell", "<i8", &[k], &fine.iter().map(|&f| part.cluster_cell[f] as i64).collect::<Vec<_>>())?;
    save(out, "clpos", "<i8", &[k], &fine.iter().map(|&f| pos0[f] as i64).collect::<Vec<_>>())?;
    log(&t, format!("K {k} clusters (sizes {}..{}), G {ng} groups", cnt.iter().min().unwrap_or(&0), cnt.iter().max().unwrap_or(&0)));

    // pass 2: per group, residual covariance and mean |r|^2, residuals r = x - (stored centre), rows read through the
    // fit's row order
    let rows = crate::bucket::Buckets::open(&pdir, out)?;
    use crate::bucket::FitRows;
    {
        let path = format!("{out}/perm.npy");
        let werr = |e: std::io::Error| format!("cannot write {path}: {e}");
        let mut w = std::io::BufWriter::with_capacity(1 << 22, std::fs::File::create(&path).map_err(werr)?);
        w.write_all(&npy_header_bytes("<i8", &[nrow])).map_err(werr)?;
        for r0 in (0..nrow).step_by(1 << 20) {
            let b: Vec<u8> = (r0..nrow.min(r0 + (1 << 20))).flat_map(|r| (rows.id(r) as i64).to_le_bytes()).collect();
            w.write_all(&b).map_err(werr)?;
        }
        w.flush().map_err(werr)?;
    }
    // each group's sums over its clusters in cluster order (groups in parallel), so the fit does not depend on the
    // thread count or on scheduling
    let acc: Vec<(Vec<f64>, f64)> = (0..ng).into_par_iter().map(|g| {
        let (mut cvg, mut r2g) = (vec![0f64; n * n], 0f64);
        for c in (0..k).filter(|&c| grp[c] == g) {
            let (r0, r1) = (cstart[c] as usize, cstart[c + 1] as usize);
            let m = &mq[c * n..(c + 1) * n];
            let mut rm = vec![0f64; (r1 - r0) * n];
            for (i, r) in (r0..r1).enumerate() {
                for (t, &h) in rows.row(r).iter().enumerate() {
                    let v = f16_to_f32(h) as f64 - m[t] as f64;
                    rm[i * n + t] = v;
                    r2g += v * v;
                }
            }
            // SAFETY: rm is (r1 - r0) x n and cvg n x n, row-major, with the strides given; cvg += rm^T rm
            unsafe { matrixmultiply::dgemm(n, r1 - r0, n, 1.0, rm.as_ptr(), 1, n as isize, rm.as_ptr(), n as isize, 1, 1.0, cvg.as_mut_ptr(), n as isize, 1) };
        }
        (cvg, r2g)
    }).collect();
    let gsize: Vec<f64> = (0..ng).map(|g| (0..k).filter(|&c| grp[c] == g).map(|c| cnt[c]).sum::<usize>().max(1) as f64).collect();
    let covs: Vec<(Vec<f64>, f64)> = acc.into_iter().enumerate().map(|(g, (mut cv, r2))| {
        cv.iter_mut().for_each(|v| *v /= gsize[g]);
        (cv, r2 / gsize[g])
    }).collect();
    let r2: Vec<f64> = covs.iter().map(|c| c.1).collect();

    // rows of the fit, sampled, as unit f32
    let sample_rows = |rng: &mut ChaCha8Rng, m: usize| -> Vec<f32> {
        let mut idx = rand::seq::index::sample(rng, nrow, m.min(nrow)).into_vec();
        idx.sort();
        idx.iter().flat_map(|&r| rows.row(r).iter().map(|&h| f16_to_f32(h))).collect()
    };

    let mut lam = vec![0f64; ng * n];
    let mut vb = vec![0f32; ng * n * n];
    let mut cond_max = 0f64;
    let sigma_q_desc;
    if basis == "pca" {
        sigma_q_desc = String::new();
        let res: Vec<(Vec<f64>, Vec<f64>)> = covs.par_iter().map(|(cv, _)| eigh_desc(cv, n)).collect::<Result<_, _>>()?;
        for (g, (l, v)) in res.into_iter().enumerate() {
            for c in 0..n {
                lam[g * n + c] = l[c].max(1e-12);
            }
            vb[g * n * n..(g + 1) * n * n].iter_mut().zip(&v).for_each(|(a, &b)| *a = b as f32);
        }
    } else {
        // Sigma_Q: offsets of 3,000 corpus items (and the queries') from 40 centres each, drawn by cluster size
        let mut rq = ChaCha8Rng::seed_from_u64(3);
        let w = WeightedIndex::new(&cnt).map_err(|e| format!("cluster sizes {e}"))?;
        let qs = sample_rows(&mut rq, 3000);
        let mut sq = offset_cov(&qs, &mh, n, &w, &mut rq);
        sigma_q_desc = match &sample {
            None => "corpus items".to_string(),
            Some(path) => {
                let q = read_npy(path);
                let qn0 = q.shape[1];
                if qn0 > n {
                    return Err(format!("{path} has {qn0} dimensions, more than the vectors' {n0}"));
                }
                let qv: Vec<f32> = match q.descr.as_str() {
                    "<f4" => q.f32s(),
                    "<f8" => q.f64s().iter().map(|&v| v as f32).collect(),
                    "<f2" => q.u16s().iter().map(|&h| f16_to_f32(h)).collect(),
                    d => return Err(format!("{path} holds {d}; write the queries as float16, float32 or float64")),
                };
                let mut qs2 = vec![0f32; q.shape[0] * n];
                for i in 0..q.shape[0] {
                    let row = &mut qs2[i * n..(i + 1) * n];
                    row[..qn0].copy_from_slice(&qv[i * qn0..(i + 1) * qn0]);
                    let nr = row.iter().map(|&v| v as f64 * v as f64).sum::<f64>().sqrt() as f32;
                    row.iter_mut().for_each(|v| *v /= nr);
                }
                let st = offset_cov(&qs2, &mh, n, &w, &mut rq);
                let (ts, tt) = (trace(&sq, n), trace(&st, n));
                sq.iter_mut().zip(&st).for_each(|(a, &b)| *a = 0.5 * (*a / ts + b / tt));
                log(&t, format!("Sigma_Q: 3000 corpus items and {} queries from {path}", q.shape[0]));
                format!("corpus items + queries {}", std::fs::canonicalize(path).map_or(path.clone(), |p| p.display().to_string()))
            }
        };
        let ridge = RIDGE * trace(&sq, n) / n as f64;
        (0..n).for_each(|i| sq[i * n + i] += ridge);
        let (lq, uq) = eigh_desc(&sq, n)?;
        let scaled = |f: &dyn Fn(f64) -> f64| -> Vec<f64> {
            let mut us = uq.clone();
            for r in 0..n {
                for c in 0..n {
                    us[r * n + c] *= f(lq[c]);
                }
            }
            let mut ut = vec![0f64; n * n];
            for r in 0..n {
                for c in 0..n {
                    ut[c * n + r] = uq[r * n + c];
                }
            }
            matmul(&us, &ut, n)
        };
        let (a, ai) = (scaled(&|l| l.sqrt()), scaled(&|l| 1.0 / l.sqrt()));
        let res: Vec<(Vec<f32>, Vec<f64>, f64)> = covs.par_iter().map(|(cv, _)| -> Result<(Vec<f32>, Vec<f64>, f64), String> {
            let (gam, wv) = eigh_desc(&matmul(&matmul(&a, cv, n), &a, n), n)?;
            let g0 = gam[0];
            let mut v = matmul(&ai, &wv, n);
            for r in 0..n {
                for c in 0..n {
                    v[r * n + c] *= gam[c].max(1e-12 * g0).powf(0.25);
                }
            }
            let v32: Vec<f32> = v.iter().map(|&x| x as f32).collect();
            // the coded coordinates z = V_Q8^-1 r under the build's int8 basis: their mean squares from the covariance
            let vq = int8_basis64(&v32, n);
            let vm = mat(&vq, n);
            let sv = vm.singular_values().map_err(|e| format!("singular values of the int8 basis: {e:?}"))?;
            let cond = sv.iter().cloned().fold(0.0, f64::max) / sv.iter().cloned().fold(f64::INFINITY, f64::min);
            let inv = vm.partial_piv_lu().inverse();
            let invr: Vec<f64> = (0..n * n).map(|i| inv[(i / n, i % n)]).collect();
            let ic = matmul(&invr, cv, n);
            let l: Vec<f64> = (0..n).map(|i| (0..n).map(|j| ic[i * n + j] * invr[i * n + j]).sum::<f64>().max(1e-30)).collect();
            Ok((v32, l, cond))
        }).collect::<Result<_, _>>()?;
        for (g, (v, l, cond)) in res.into_iter().enumerate() {
            vb[g * n * n..(g + 1) * n * n].copy_from_slice(&v);
            lam[g * n..(g + 1) * n].copy_from_slice(&l);
            cond_max = cond_max.max(cond);
        }
    }
    drop(covs);
    log(&t, format!("bases done ({basis}); group sizes {}..{}", gsize.iter().cloned().fold(f64::INFINITY, f64::min), gsize.iter().cloned().fold(0.0, f64::max)));

    // profile: corpus items as pseudo-queries against their 4 nearest centres (by q.m with the stored centres), the mean
    // square of (q - m) in each group's coordinates; groups with 50 pairs or fewer take lam
    let profile = |pq: &[f32], proj: &(dyn Fn(usize) -> Vec<f64> + Sync)| -> Vec<f64> {
        let np = pq.len() / n;
        let mut d = vec![0f32; np * k];
        unsafe { matrixmultiply::sgemm(np, n, k, 1.0, pq.as_ptr(), n as isize, 1, mq.as_ptr(), 1, n as isize, 0.0, d.as_mut_ptr(), k as isize, 1) };
        let mut pairs: Vec<Vec<(usize, usize)>> = vec![Vec::new(); ng];
        for i in 0..np {
            let mut o: Vec<usize> = (0..k).collect();
            let row = &d[i * k..(i + 1) * k];
            let top = 4.min(k);
            o.select_nth_unstable_by(top - 1, |&a, &b| row[b].total_cmp(&row[a]));
            for &c in &o[..top] {
                pairs[grp[c]].push((i, c));
            }
        }
        let rows: Vec<Vec<f64>> = (0..ng).into_par_iter().map(|g| {
            if pairs[g].len() <= 50 {
                return lam[g * n..(g + 1) * n].to_vec();
            }
            let v = proj(g);
            let m = pairs[g].len();
            let mut dm = vec![0f64; m * n];
            for (j, &(i, c)) in pairs[g].iter().enumerate() {
                for t in 0..n {
                    dm[j * n + t] = (pq[i * n + t] - mq[c * n + t]) as f64;
                }
            }
            let mut p = vec![0f64; m * n];
            unsafe { matrixmultiply::dgemm(m, n, n, 1.0, dm.as_ptr(), n as isize, 1, v.as_ptr(), n as isize, 1, 0.0, p.as_mut_ptr(), n as isize, 1) };
            (0..n).map(|c| (0..m).map(|j| p[j * n + c] * p[j * n + c]).sum::<f64>() / m as f64).collect()
        }).collect();
        rows.concat()
    };
    let pq = sample_rows(&mut rng, 5000);
    let fitj;
    let prof: Vec<f64> = if basis == "pca" {
        let qi = profile(&pq, &|g| vb[g * n * n..(g + 1) * n * n].iter().map(|&x| x as f64).collect());
        fitj = serde_json::json!({"basis": "pca", "profile": "h25"});
        (0..ng * n).map(|i| {
            let (g, c) = (i / n, i % n);
            let iso = if c < n0 { qi[g * n..(g + 1) * n].iter().sum::<f64>() / n0 as f64 } else { 0.0 };
            ((1.0 - HEDGE) * qi[i].sqrt() + HEDGE * iso.sqrt()).powi(2)
        }).collect()
    } else {
        let mut r2g = ChaCha8Rng::seed_from_u64(seed + 2);
        let pq2 = sample_rows(&mut r2g, 5000);
        fitj = serde_json::json!({"basis": "dual", "profile": "qi", "ridge": RIDGE, "sigma_q": sigma_q_desc, "cond_max": cond_max});
        profile(&pq2, &|g| int8_basis64(&vb[g * n * n..(g + 1) * n * n], n))
    };
    let _ = pq;

    let (rlv, bounds) = schedule(n0, n)?;
    // the rate design's constant (two-sided, N = 1e9); the query-time constants follow the index size (src/v2/tail.rs)
    let l1 = (2.0 * NTARGET * rlv.len() as f64 / EPS).ln();
    let tt: serde_json::Value = serde_json::from_str(include_str!("tree_tables.json")).unwrap();
    let tab = |key: &str| -> Vec<f64> { tt[key].as_array().unwrap().iter().flat_map(|r| r.as_array().unwrap().iter().map(|x| x.as_f64().unwrap())).collect() };
    let gtree: Vec<f64> = tt["gtree"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    save(out, "grp", "<i8", &[k], &grp.iter().map(|&g| g as i64).collect::<Vec<_>>())?;
    save(out, "mq8", "|i1", &[k, n], &mq8)?;
    save(out, "msc", "<f8", &[k], &msc)?;
    save(out, "lam", "<f8", &[ng, n], &lam)?;
    save(out, "V", "<f4", &[ng, n, n], &vb)?;
    save(out, "r2", "<f8", &[ng], &r2)?;
    save(out, "profile", "<f8", &[ng, n], &prof)?;
    save(out, "tree_edges", "<f8", &[9, 256], &tab("tree_edges"))?;
    save(out, "tree_cent", "<f8", &[9, 256], &tab("tree_cent"))?;
    save(out, "gtree", "<f8", &[gtree.len()], &gtree)?;
    let mut qi = rand::seq::index::sample(&mut rng, nrow, 200.min(nrow)).into_vec().into_iter().map(|r| r as i64).collect::<Vec<_>>();
    qi.sort();
    save(out, "qi", "<i8", &[qi.len()], &qi)?;
    // the anchors from the stored centres (as `load_centres` gives them), then meta.json, last, through a new file
    let na = anchors_into(out, n, k, &mq, o.image, o.text_anchors.as_deref(), o.text_count, seed)?;
    let meta = serde_json::json!({
        "N": nrow, "n": n, "n_orig": n0, "K": k, "G": ng, "metric": "angular", "DLV": DLV, "RLV": rlv, "BOUNDS": bounds,
        "CSG": (2.0 * l1).sqrt(), "L1": l1, "EPS": EPS,
        "partition": std::fs::canonicalize(&pdir).map_or(pdir.clone(), |p| p.display().to_string()),
        "partition_id": partition_id,
        "partition_file": partition_file.json(),
        "format": FORMAT, "fit": fitj,
    });
    // recorded only when not the default, so a default fit's meta.json is as before
    let mut meta = meta;
    if o.groups != Groups::ByK8 {
        meta["clusters"] = serde_json::json!({ "groups": format!("{:?}", o.groups) });
    }
    // the randomised levels' certified constant (meta "CERTS", as tools/certify.py records it): the shipped table's for
    // this layout and budget, else certified here within cert_time (0 skips it); without one the build uses the
    // closed-form bounds, which are larger
    if cert_time > 0.0 {
        let ds = block_dims(&bounds);
        let lnd = crate::certify::ln_delta(nrow, rlv.len(), EPS);
        let tc = Timer::new();
        let (x, why) = match crate::certify::lookup(&ds, lnd) {
            Some(x) => (x, "the shipped table".to_string()),
            None => (crate::certify::certify_layout(&ds, lnd, cert_time, &mut |s| log(&t, s)), format!("certification within {cert_time:.0}s")),
        };
        match x {
            Some(x) => {
                meta["CERTS"] = serde_json::json!([{ "ds": ds, "lnd": lnd, "X": x }]);
                log(&t, format!("certificate for {ds:?} at ln delta {lnd:.4}: X {x:.3}, from {why} in {:.1}s", tc.s()));
            }
            None => log(&t, format!("no certificate for {ds:?} at ln delta {lnd:.4} from {why}; the build uses the convex-order and Chernoff bounds")),
        }
    }
    let tmp = format!("{meta_path}.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(&meta).unwrap()).map_err(|e| format!("cannot write {tmp}: {e}"))?;
    std::fs::rename(&tmp, &meta_path).map_err(|e| format!("cannot move {tmp} to {meta_path}: {e}"))?;
    log(&t, format!("saved {out}; {na}"));
    Ok(())
}

/// The support table's anchors for the fit in `dir`: the directions of all K centres when `image` >= K (one anchor per
/// cluster), else `image` anchors by spherical k-means on the centre directions; then, from `text` (training prompts,
/// rows of n_orig or n, f16, f32 or f64), `ntext` more by k-means on the prompts. A query's bound uses its nearest
/// anchor, so what sets its cost is how densely the anchors cover the directions queries come from, not K: a fixed A
/// keeps the table K x A at any N. Written to `dir/anchors.npy` (A x n, unit, f64), except when they are exactly the
/// centres (`image` >= K and no prompts): then no file, which the build reads as one anchor per cluster, its centre's
/// direction (and an existing `anchors.npy` is removed). Returns what was written, for the log.
pub fn write_anchors(dir: &str, image: usize, text: Option<&str>, ntext: usize, seed: u64) -> Result<String, String> {
    let (n, k, centres) = crate::index::load_centres(dir)?;
    anchors_into(dir, n, k, &centres, image, text, ntext, seed)
}

/// The anchors `write_anchors` would write for the fit in `dir`, without writing them: None when they are the centres
/// (no file). `put_anchors` writes them; `v2::rebuild` does so only once the index made with them is installed.
pub fn make_anchors(dir: &str, image: usize, text: Option<&str>, ntext: usize, seed: u64) -> Result<Option<Vec<f64>>, String> {
    let (n, k, centres) = crate::index::load_centres(dir)?;
    anchors_for(n, k, &centres, image, text, ntext, seed)
}

/// Write anchors from `make_anchors` into the fit in `dir` (None: remove its anchors.npy and anchors.json). Returns
/// what was written, for the log.
pub fn put_anchors(dir: &str, anchors: Option<&[f64]>) -> Result<String, String> {
    let (n, k, centres) = crate::index::load_centres(dir)?;
    put_anchors_of(dir, n, k, &centres, anchors)
}

/// `write_anchors` for the fit in `dir` with these centres (K x n, as `index::load_centres` gives them).
#[allow(clippy::too_many_arguments)]
fn anchors_into(dir: &str, n: usize, k: usize, centres: &[f32], image: usize, text: Option<&str>, ntext: usize, seed: u64) -> Result<String, String> {
    let a = anchors_for(n, k, centres, image, text, ntext, seed)?;
    put_anchors_of(dir, n, k, centres, a.as_deref())
}

/// `make_anchors` with these centres.
#[allow(clippy::too_many_arguments)]
fn anchors_for(n: usize, k: usize, centres: &[f32], image: usize, text: Option<&str>, ntext: usize, seed: u64) -> Result<Option<Vec<f64>>, String> {
    let text = match text {
        Some(path) => {
            let (v, w) = try_read_npy(path)?.rows_f32(path)?;
            if w > n {
                return Err(format!("{path} has rows of {w} dimensions, more than the fit's {n}"));
            }
            Some((v, w))
        }
        None => None,
    };
    anchors_of(centres, n, k, image, text.as_ref().map(|(v, w)| (&v[..], *w)), ntext, seed)
}

/// `put_anchors` with these centres.
fn put_anchors_of(dir: &str, n: usize, k: usize, centres: &[f32], anchors: Option<&[f64]>) -> Result<String, String> {
    let path = format!("{dir}/anchors.npy");
    match anchors {
        None => {
            // the sidecar first: a file left without it is refused, never taken for this fit's
            replace_files(&[(format!("{dir}/anchors.json"), None), (path.clone(), None)]).map_err(|e| format!("the fit's anchors: {e}"))?;
            Ok(format!("no anchors.npy: the support table's anchors are the {k} centres"))
        }
        Some(a) => {
            save_anchors(dir, n, k, centres, a)?;
            Ok(format!("{} support-table anchors in {path}", a.len() / n))
        }
    }
}

/// Write `anchors` (A x n, unit) as the fit's `anchors.npy`, with the sidecar `anchors.json` that records the fit they
/// were made for (`index::centres_hash` of its centres) and their own hash, which the build and the open check
/// (`index::load_anchors`): a refit, or anchors.npy replaced alone, is refused rather than built with.
pub fn save_anchors(dir: &str, n: usize, k: usize, centres: &[f32], anchors: &[f64]) -> Result<(), String> {
    crate::index::check_anchors(anchors, n)?;
    // both written and synced beside the fit's, then renamed over the old ones back to back (a hard link of a
    // `cp -al` copy keeps the other directory's file); anchors.npy first, so the sidecar never names a file that is not
    // there yet; a failure puts the old pair back (`replace_files`)
    let sb = unsafe { std::slice::from_raw_parts(anchors.as_ptr() as *const u8, std::mem::size_of_val(anchors)) };
    let mut npy = npy_header_bytes("<f8", &[anchors.len() / n, n]);
    npy.extend_from_slice(sb);
    let j = serde_json::json!({"fit": crate::index::centres_hash(n, k, centres), "anchors": crate::index::anchors_hash(anchors)});
    replace_files(&[(format!("{dir}/anchors.npy"), Some(npy)), (format!("{dir}/anchors.json"), Some(j.to_string().into_bytes()))]).map_err(|e| format!("the fit's anchors: {e}"))
}

/// The anchors of `write_anchors` (A x n, unit, f64) from the centres (K x n) and the prompts (rows of w <= n values,
/// zero-padded to n); None when they are exactly the centres. `image` = 0, or `ntext` = 0 or no prompts, leaves out
/// that set; at least one anchor must remain.
fn anchors_of(centres: &[f32], n: usize, k: usize, image: usize, text: Option<(&[f32], usize)>, ntext: usize, seed: u64) -> Result<Option<Vec<f64>>, String> {
    let unit64 = |v: &[f32]| -> Vec<f64> {
        let nr = v.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt().max(1e-300);
        v.iter().map(|&x| x as f64 / nr).collect()
    };
    let text = text.filter(|(v, _)| !v.is_empty() && ntext > 0);
    if image >= k && k > 0 && text.is_none() {
        return Ok(None);
    }
    let dirs: Vec<f32> = (0..k).flat_map(|c| unit64(&centres[c * n..(c + 1) * n]).into_iter().map(|x| x as f32)).collect();
    let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 0xa4c4);
    let mut out: Vec<f64> = if image == 0 {
        Vec::new()
    } else if image >= k {
        (0..k).flat_map(|c| unit64(&centres[c * n..(c + 1) * n])).collect()
    } else {
        kmeans(&dirs, n, image, &mut rng, 25).chunks(n).flat_map(unit64).collect()
    };
    if let Some((v, w)) = text {
        let rows: Vec<f32> = v.chunks(w).flat_map(|r| {
            let mut x = r.to_vec();
            x.resize(n, 0.0);
            unit64(&x).into_iter().map(|y| y as f32)
        }).collect();
        let m = rows.len() / n;
        out.extend(kmeans(&rows, n, ntext.min(m), &mut rng, 25).chunks(n).flat_map(unit64));
    }
    if out.is_empty() {
        return Err("no support-table anchors: with no anchors from the centres, give prompts and a text count above 0".into());
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_rows(m: usize, n: usize, seed: u64) -> Vec<f32> {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        (0..m).flat_map(|_| {
            let v: Vec<f32> = (0..n).map(|_| rng.gen::<f32>() - 0.5).collect();
            let nr = v.iter().map(|&x| x * x).sum::<f32>().sqrt();
            v.into_iter().map(move |x| x / nr)
        }).collect()
    }

    /// Either set of anchors can be left out with a count of 0; with both left out there are none, which is refused;
    /// all K centres and no prompts are the centres themselves (no file).
    #[test]
    fn anchor_sets_can_be_empty_but_not_both() {
        let (n, k) = (8, 20);
        let c = unit_rows(k, n, 1);
        let t = unit_rows(30, 6, 2);
        let a = |image, text: Option<(&[f32], usize)>, nt| anchors_of(&c, n, k, image, text, nt, 3).unwrap().map(|v| v.len() / n);
        assert_eq!(a(0, Some((&t, 6)), 5), Some(5));
        assert_eq!(a(4, Some((&t, 6)), 0), Some(4));
        assert_eq!(a(4, None, 5), Some(4));
        assert_eq!(a(50, Some((&t, 6)), 100), Some(k + 30));
        assert_eq!(a(4, Some((&t[..0], 6)), 5), Some(4));
        assert_eq!(a(k, None, 5), None);
        assert_eq!(a(50, Some((&t, 6)), 0), None);
        let all = anchors_of(&c, n, k, 4, Some((&t, 6)), 5, 3).unwrap().unwrap();
        crate::index::check_anchors(&all, n).unwrap();
        assert!(anchors_of(&c, n, k, 0, Some((&t, 6)), 0, 3).is_err());
        assert!(anchors_of(&c, n, k, 0, None, 5, 3).is_err());
        assert!(crate::index::check_anchors(&[1.0, 1e-5], 2).is_err());
        assert!(crate::index::check_anchors(&[0.6, 0.8, 1.0], 2).is_err());
    }

    /// The cluster-size rule (as tools/fit.py's and tools/partition.py's `by-n`) and the group rules.
    #[test]
    fn cluster_size_and_group_rules() {
        let pc: Vec<usize> = [0, 500_000, 1_000_000, 2_000_000, 2_100_000, 8_000_000, 8_100_000, 32_000_000, 33_000_000, 1_000_000_000].iter().map(|&n| per_cluster_by_n(n)).collect();
        assert_eq!(pc, [2000, 2000, 2000, 2000, 4000, 4000, 8000, 8000, 16000, 64000]);
        // 1M items of 768 dimensions: 1,000 clusters by default, 500 by the rule
        assert_eq!(Groups::ByK8.count(1_000_000, 1000, 768), 64);
        assert_eq!(Groups::ByK8.count(1_000_000, 500, 768), 62);
        assert_eq!(Groups::ByN.count(1_000_000, 500, 768), 64);
        assert_eq!(Groups::ByN.count(60_000, 30, 768), 3);
        assert_eq!(Groups::ByK8.count(60_000, 60, 768), 3);
        assert_eq!(Groups::ByN.count(1_000, 4, 768), 1);
        assert_eq!(Groups::Fixed(10).count(1_000_000, 4, 768), 4);
        assert_eq!(Groups::parse("by-n"), Ok(Groups::ByN));
        assert_eq!(Groups::parse("12"), Ok(Groups::Fixed(12)));
        assert!(Groups::parse("0").is_err() && Groups::parse("k8").is_err());
    }

    /// tools/rules.py's `per_cluster` and `groups_for` (which tools/fit.py and tools/partition.py use) give what
    /// `per_cluster_by_n` and `Groups::count` give, at the rule's boundaries. Needs python3; skipped with a message
    /// without it.
    #[test]
    fn python_rules_agree() {
        let tools = concat!(env!("CARGO_MANIFEST_DIR"), "/tools");
        let ns: [usize; 13] = [1_000, 60_000, 1_000_000, 1_999_999, 2_000_000, 2_000_001, 8_000_000, 8_000_001, 32_000_000, 32_000_001, 128_000_000, 128_000_001, 1_000_000_000];
        let gs: [(usize, usize, usize); 7] = [(1_000_000, 500, 768), (1_000_000, 1000, 768), (60_000, 30, 768), (60_000, 60, 768), (1_000, 4, 768), (1_000_000_000, 15_625, 768), (4_000, 32, 64)];
        let script = format!(
            "import sys\nsys.path.insert(0, {tools:?})\nimport rules\n\
             for m in ('fit', 'partition'):\n    try:\n        mod = __import__(m)\n    except ImportError as e:\n        print('skip', m, e)\n        continue\n    \
             assert mod.per_cluster is rules.per_cluster, m\n    print('shared', m)\n\
             print(*[rules.per_cluster(n) for n in {ns:?}])\n\
             for r in ('by-k8', 'by-n', '10'):\n    print(*[rules.groups_for(r, N, K, n0) for N, K, n0 in {gs:?}])\n");
        let out = match std::process::Command::new("python3").arg("-I").arg("-c").arg(&script).output() {
            Ok(o) => o,
            Err(e) => {
                eprintln!("python_rules_agree skipped: python3 cannot run ({e})");
                return;
            }
        };
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let text = String::from_utf8(out.stdout).unwrap();
        let mut lines: Vec<&str> = text.lines().collect();
        for l in lines.iter().filter(|l| l.starts_with("skip ")) {
            eprintln!("python_rules_agree: {l} (rules.py checked alone)");
        }
        lines.retain(|l| !l.starts_with("skip ") && !l.starts_with("shared "));
        let nums = |l: &str| l.split_whitespace().map(|x| x.parse::<usize>().unwrap()).collect::<Vec<_>>();
        assert_eq!(nums(lines[0]), ns.iter().map(|&n| per_cluster_by_n(n)).collect::<Vec<_>>());
        assert_eq!(nums(lines[0])[4..9], [2000, 4000, 4000, 8000, 8000]);
        assert_eq!(*nums(lines[0]).last().unwrap(), 64000);
        for (l, g) in lines[1..].iter().zip([Groups::ByK8, Groups::ByN, Groups::Fixed(10)]) {
            assert_eq!(nums(l), gs.iter().map(|&(n, k, n0)| g.count(n, k, n0)).collect::<Vec<_>>(), "{g:?}");
        }
        assert_eq!(lines.len(), 4);
    }

    /// Prompts in f16 read as the same values as in f32; other types and shapes are refused.
    #[test]
    fn prompt_files_of_any_float_width() {
        let dir = crate::util::TmpDir::new("anch");
        let v: Vec<f32> = vec![0.5, -0.25, 1.0, 0.125, -2.0, 0.75];
        let p = |s: &str| dir.join(s).to_str().unwrap().to_string();
        write_npy(&p("a2.npy"), "<f2", &[2, 3], &v.iter().flat_map(|&x| f32_to_f16(x).to_le_bytes()).collect::<Vec<_>>());
        write_npy(&p("a4.npy"), "<f4", &[2, 3], &v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<_>>());
        write_npy(&p("a8.npy"), "<f8", &[2, 3], &v.iter().flat_map(|&x| (x as f64).to_le_bytes()).collect::<Vec<_>>());
        write_npy(&p("i4.npy"), "<i4", &[2, 3], &[0u8; 24]);
        write_npy(&p("f1d.npy"), "<f4", &[6], &v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<_>>());
        for f in ["a2.npy", "a4.npy", "a8.npy"] {
            assert_eq!(read_npy(&p(f)).rows_f32(f).unwrap(), (v.clone(), 3), "{f}");
        }
        for f in ["i4.npy", "f1d.npy"] {
            assert!(read_npy(&p(f)).rows_f32(f).is_err(), "{f}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
