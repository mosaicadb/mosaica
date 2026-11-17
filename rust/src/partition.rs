//! The partition: a file of vectors cut into cells for the fit and the build (`fit::fit`, `bucket::Partition`), the
//! algorithm and the output of `tools/partition.py` without Python. `v2::create` runs it into DIR/partition;
//! `vexact --vectors V partition --out DIR [--cells C] [--cross T] ...` runs it alone.
//!
//! Coarse cells (C, default 1) by spherical k-means on a sample of `cell_sample` rows per cell; one pass over the
//! vectors appends each unit row (f16, zero-padded to n, a multiple of 32) to its cell's file; K = max(1, round(N /
//! per_cluster)) fine centres, in proportion to each cell's rows, by spherical k-means on up to `sample_per_centre`
//! rows per centre of the cell; then every row goes to its nearest fine centre among those of its cell's `cross`
//! nearest cells, and a row whose centre lies in another cell is appended to that cell's moved-in files. The files are
//! those `tools/partition.py` writes, by the same names and with the same contents' layout (its header lists them):
//! `cell{c}.f16|ids|cl`, `cell{c}.in.f16|ids|cl`, `centres.npy`, `cluster_cell.npy`, and `manifest.json` last (through
//! `manifest.json.tmp`, synced and renamed). A fit takes a partition from either.
//!
//! Nothing holds the vectors: they are read through a mapping, a chunk of rows at a time, and the cells through
//! mappings of their files; the heap holds a chunk of rows, one cell's training sample (f16, as the cell stores it) and
//! the centres.
//!
//! The k-means is faiss's `Kmeans(spherical=True, min_points_per_centroid=1, max_points_per_centroid=huge)`, which
//! partition.py calls: k distinct sample rows to start; each iteration assigns every sample row to the centre of
//! largest inner product (the lowest index on a tie), sets each centre to the mean of its rows, splits a cluster for
//! each empty one (`split_empty`, faiss's `split_clusters`), and normalises the centres. What differs from faiss: the
//! random numbers (ChaCha8 from the seed, as elsewhere in the crate; the split draws from the k-means' own generator,
//! where faiss reseeds it with 1234 on every call), the centre sums (f64 over a cluster's rows in row order, where
//! faiss sums in f32), and the unit rows (norm in f64). A row that is not finite is refused (partition.py made it a
//! zero row or NaNs). Every parallel loop writes per-row or per-cluster results in a fixed order, and the blocks a GEMM
//! sees do not depend on the number of threads, so the same seed and input give the same files at any thread count.

use crate::util::{npy_header, npy_shape, npy_header_bytes, Timer};
use crate::v2::PerCluster;
use faer::{Accum, MatMut, MatRef, Par};
use half::f16;
use half::slice::{HalfBitsSliceExt, HalfFloatSliceExt};
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use std::io::Write;

/// What a partition takes besides the vectors and the directory (`tools/partition.py`'s flags, with its defaults).
#[derive(Clone, Debug, PartialEq)]
pub struct PartitionOptions {
    /// Coarse cells (1: one k-means over a sample of the whole set).
    pub cells: usize,
    /// Each row goes to its nearest fine centre among those of its cell's `cross` nearest cells (>= cells: exact).
    pub cross: usize,
    /// Items per cluster on average, or the rule by N (`fit::per_cluster_by_n`).
    pub per_cluster: PerCluster,
    /// Fine k-means: at most this many sample rows of a cell per centre.
    pub sample_per_centre: usize,
    /// Coarse k-means: this many sample rows per cell.
    pub cell_sample: usize,
    /// k-means iterations.
    pub niter: usize,
    pub seed: u64,
}
impl Default for PartitionOptions {
    fn default() -> PartitionOptions {
        PartitionOptions { cells: 1, cross: 1, per_cluster: PerCluster::ByN, sample_per_centre: 200, cell_sample: 1000, niter: 20, seed: 20261112 }
    }
}

/// What a partition made: rows, fine centres, cells, rows moved to a centre of another cell, the mean inner product of
/// each unit row with its centre (the k-means objective, the mean cosine), and each fine cluster's rows.
#[derive(Clone, Debug)]
pub struct Summary {
    pub rows: usize,
    pub k: usize,
    pub cells: usize,
    pub moved: usize,
    pub objective: f64,
    pub sizes: Vec<usize>,
}

/// Rows read per chunk of the first pass (the f16 rows of a chunk are on the heap at once).
const CHUNK: usize = 16384;
/// Rows of a GEMM block, and centres per GEMM: fixed, so the products do not depend on the number of threads.
const BR: usize = 256;
const KB: usize = 1024;

/// An N x n0 .npy of little-endian f16, f32 or f64, mapped.
pub struct Vectors {
    map: memmap2::Mmap,
    off: usize,
    pub rows: usize,
    pub n0: usize,
    width: usize,
}

impl Vectors {
    /// The file mapped, its header checked: a C-ordered 2-D array of floats, N, n0 > 0, as long as its shape says.
    pub fn open(path: &str) -> Result<Vectors, String> {
        let f = std::fs::File::open(path).map_err(|e| format!("cannot open {path}: {e}"))?;
        let len = f.metadata().map_err(|e| format!("cannot read {path}: {e}"))?.len();
        let bad = |what: String| format!("{path}: expected N x n float16/32/64 vectors (.npy), found {what}");
        if len < 12 {
            return Err(bad(format!("a file of {len} bytes")));
        }
        // SAFETY: the vectors are only read; a file changed while it is read gives other rows, never memory unsafety
        // beyond what any mapped read gives (the manifest records its length and modification time)
        let map = unsafe { memmap2::Mmap::map(&f).map_err(|e| format!("cannot map {path}: {e}"))? };
        if &map[..6] != b"\x93NUMPY" || !matches!(map[6], 1..=3) {
            return Err(bad("no .npy header".into()));
        }
        let hl = if map[6] == 1 { u16::from_le_bytes([map[8], map[9]]) as usize + 10 } else { u32::from_le_bytes([map[8], map[9], map[10], map[11]]) as usize + 12 };
        if hl > map.len() {
            return Err(bad("a header past the end".into()));
        }
        let (hdr, off) = npy_header(&map);
        if hdr.contains("'fortran_order': True") || !hdr.contains("'shape'") {
            return Err(bad(format!("the header {}", hdr.trim())));
        }
        let shape = npy_shape(&hdr);
        let width = if hdr.contains("'<f2'") { 2 } else if hdr.contains("'<f4'") { 4 } else if hdr.contains("'<f8'") { 8 } else { 0 };
        if width == 0 || shape.len() != 2 || shape[0] == 0 || shape[1] == 0 {
            return Err(bad(format!("the header {}", hdr.trim())));
        }
        let (rows, n0) = (shape[0], shape[1]);
        if rows.checked_mul(n0).and_then(|x| x.checked_mul(width)).and_then(|x| x.checked_add(off)).map_or(true, |e| e > map.len()) {
            return Err(bad(format!("{} bytes of data for a shape of {rows} x {n0}", map.len() - off)));
        }
        Ok(Vectors { map, off, rows, n0, width })
    }

    /// Row i as a unit vector in `out` (n >= n0 values, zero past n0; a zero row stays zero); false when the row is
    /// not finite.
    fn unit(&self, i: usize, out: &mut [f32]) -> bool {
        let (n0, w) = (self.n0, self.width);
        let b = &self.map[self.off + i * n0 * w..self.off + (i + 1) * n0 * w];
        let x = |j: usize| -> f64 {
            match w {
                2 => f16::from_bits(u16::from_le_bytes([b[2 * j], b[2 * j + 1]])).to_f64(),
                4 => f32::from_le_bytes(b[4 * j..4 * j + 4].try_into().unwrap()) as f64,
                _ => f64::from_le_bytes(b[8 * j..8 * j + 8].try_into().unwrap()),
            }
        };
        let ss: f64 = (0..n0).map(|j| x(j) * x(j)).sum();
        if !ss.is_finite() {
            return false;
        }
        let nr = ss.sqrt();
        out.fill(0.0);
        if nr > 0.0 {
            for j in 0..n0 {
                out[j] = (x(j) / nr) as f32;
            }
        }
        true
    }
}

/// Rows a k-means and a nearest-centre search read, each as n f32 values.
trait Rows: Sync {
    fn len(&self) -> usize;
    fn get(&self, i: usize, out: &mut [f32]);
}
struct F32Rows<'a>(&'a [f32], usize);
impl Rows for F32Rows<'_> {
    fn len(&self) -> usize {
        self.0.len() / self.1
    }
    fn get(&self, i: usize, out: &mut [f32]) {
        out.copy_from_slice(&self.0[i * self.1..(i + 1) * self.1]);
    }
}
struct F16Rows<'a>(&'a [u16], usize);
impl Rows for F16Rows<'_> {
    fn len(&self) -> usize {
        self.0.len() / self.1
    }
    fn get(&self, i: usize, out: &mut [f32]) {
        self.0[i * self.1..(i + 1) * self.1].reinterpret_cast::<f16>().convert_to_f32_slice(out);
    }
}

/// For the r rows of `x` (r x n), the centre of `c` (k x n) of largest inner product, the lowest index on a tie, and
/// that product, into `lab` and `best`. GEMMs of r rows and at most KB centres, sequential (the caller is parallel).
fn block_nearest(x: &[f32], n: usize, c: &[f32], lab: &mut [u32], best: &mut [f32]) {
    let (r, k) = (lab.len(), c.len() / n);
    best.fill(f32::NEG_INFINITY);
    lab.fill(0);
    let mut d = vec![0f32; r * KB.min(k)];
    for j0 in (0..k).step_by(KB) {
        let kb = KB.min(k - j0);
        let d = &mut d[..r * kb];
        let a = MatRef::from_row_major_slice(x, r, n);
        let b = MatRef::from_row_major_slice(&c[j0 * n..(j0 + kb) * n], kb, n).transpose();
        faer::linalg::matmul::matmul(MatMut::from_row_major_slice_mut(d, r, kb), Accum::Replace, a, b, 1.0f32, Par::Seq);
        for i in 0..r {
            for (j, &v) in d[i * kb..(i + 1) * kb].iter().enumerate() {
                if v > best[i] {
                    best[i] = v;
                    lab[i] = (j0 + j) as u32;
                }
            }
        }
    }
}

/// For every row, its nearest centre of `c` (largest inner product, the lowest index on a tie) and that product; in
/// parallel over fixed blocks of BR rows.
fn nearest(rows: &dyn Rows, n: usize, c: &[f32]) -> (Vec<u32>, Vec<f32>) {
    let m = rows.len();
    let (mut lab, mut best) = (vec![0u32; m], vec![0f32; m]);
    lab.par_chunks_mut(BR).zip(best.par_chunks_mut(BR)).enumerate().for_each(|(b, (l, s))| {
        let mut x = vec![0f32; l.len() * n];
        for (i, xi) in x.chunks_mut(n).enumerate() {
            rows.get(b * BR + i, xi);
        }
        block_nearest(&x, n, c, l, s);
    });
    (lab, best)
}

/// Normalised to unit length (in f64); a zero vector stays zero (faiss's `fvec_renorm_L2`).
fn normalise(v: &mut [f32]) {
    let nr = v.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt();
    if nr > 0.0 {
        for x in v.iter_mut() {
            *x = (*x as f64 / nr) as f32;
        }
    }
}

/// faiss's `split_clusters`: for each empty cluster ci in turn, a cluster cj is drawn by walking 0, 1, ... (cyclically)
/// and taking each with probability (size - 1) / (m - k); ci takes cj's centre, the two moved apart by a relative
/// 1/1024 in alternate dimensions (ci up in the even ones, cj up in the odd ones), and cj's size is shared between them
/// (its rows are divided by the next assignment). `size` holds the clusters' sizes; m rows, k clusters, m >= k. With
/// m = k (where faiss divides by zero) a cluster of two or more rows is taken. Returns the number of splits.
fn split_empty(c: &mut [f32], size: &mut [f64], n: usize, m: usize, rng: &mut ChaCha8Rng) -> usize {
    const EPS: f32 = 1.0 / 1024.0;
    let k = size.len();
    let mut splits = 0;
    for ci in 0..k {
        if size[ci] != 0.0 {
            continue;
        }
        let mut cj = 0;
        loop {
            let p = if m > k { (size[cj] - 1.0) / (m - k) as f64 } else if size[cj] > 1.0 { 1.0 } else { 0.0 };
            if rng.gen::<f64>() < p {
                break;
            }
            cj = (cj + 1) % k;
        }
        let src: Vec<f32> = c[cj * n..(cj + 1) * n].to_vec();
        for j in 0..n {
            let (up, down) = (src[j] * (1.0 + EPS), src[j] * (1.0 - EPS));
            if j % 2 == 0 {
                c[ci * n + j] = up;
                c[cj * n + j] = down;
            } else {
                c[ci * n + j] = down;
                c[cj * n + j] = up;
            }
        }
        size[ci] = size[cj] / 2.0;
        size[cj] -= size[ci];
        splits += 1;
    }
    splits
}

/// Spherical k-means of `rows` (m rows of n, unit) to k unit centres (1 <= k <= m), as faiss's: k distinct rows drawn
/// with `seed` to start, then `niter` rounds of assignment, means, splits of a cluster for each empty one
/// (`split_empty`) and normalisation. Deterministic for a seed at any thread count.
fn kmeans(rows: &dyn Rows, n: usize, k: usize, seed: u64, niter: usize) -> Vec<f32> {
    let m = rows.len();
    assert!(k >= 1 && k <= m, "k-means of {m} rows to {k} centres");
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut c = vec![0f32; k * n];
    for (j, i) in rand::seq::index::sample(&mut rng, m, k).into_iter().enumerate() {
        rows.get(i, &mut c[j * n..(j + 1) * n]);
    }
    for _ in 0..niter {
        let (lab, _) = nearest(rows, n, &c);
        // each centre's rows, in row order
        let mut start = vec![0usize; k + 1];
        for &l in &lab {
            start[l as usize + 1] += 1;
        }
        for j in 0..k {
            start[j + 1] += start[j];
        }
        let mut fill = start.clone();
        let mut mem = vec![0u32; m];
        for (i, &l) in lab.iter().enumerate() {
            mem[fill[l as usize]] = i as u32;
            fill[l as usize] += 1;
        }
        c.par_chunks_mut(n).enumerate().for_each(|(j, cj)| {
            let ms = &mem[start[j]..start[j + 1]];
            let (mut sum, mut x) = (vec![0f64; n], vec![0f32; n]);
            for &i in ms {
                rows.get(i as usize, &mut x);
                for (s, &v) in sum.iter_mut().zip(&x) {
                    *s += v as f64;
                }
            }
            for (o, s) in cj.iter_mut().zip(&sum) {
                *o = if ms.is_empty() { 0.0 } else { (s / ms.len() as f64) as f32 };
            }
        });
        let mut size: Vec<f64> = (0..k).map(|j| (start[j + 1] - start[j]) as f64).collect();
        split_empty(&mut c, &mut size, n, m, &mut rng);
        c.chunks_mut(n).for_each(normalise);
    }
    c
}

/// Whether `name` is one tools/partition.py writes besides manifest.json (`cell{c}.f16|ids|cl`,
/// `cell{c}.in.f16|ids|cl`, `centres.npy`, `cluster_cell.npy`, `manifest.json.tmp`): the only names a partition removes.
pub fn own_name(name: &str) -> bool {
    if matches!(name, "centres.npy" | "cluster_cell.npy" | "manifest.json.tmp") {
        return true;
    }
    let Some(rest) = name.strip_prefix("cell") else { return false };
    let digits = rest.bytes().take_while(|b| b.is_ascii_digit()).count();
    let tail = &rest[digits..];
    digits > 0 && matches!(tail, ".f16" | ".ids" | ".cl" | ".in.f16" | ".in.ids" | ".in.cl")
}

/// Remove an earlier partition in `out`, as tools/partition.py's `clear_partition`: its manifest first (until the new
/// one is written last, `out` is no partition, so no cell can change under a manifest), then the regular files of the
/// names a partition writes (`own_name`; a cell it had and this run does not write would be stale). Nothing else, and
/// never the input `vectors` (a name that is it, or a link to it, stays).
pub fn clear_partition(out: &str, vectors: &str) -> Result<(), String> {
    let keep = std::fs::canonicalize(vectors).ok();
    let man = format!("{out}/manifest.json");
    if std::fs::symlink_metadata(&man).is_ok() && std::fs::canonicalize(&man).ok() != keep {
        std::fs::remove_file(&man).map_err(|e| format!("cannot remove {man}: {e}"))?;
    }
    let mut names: Vec<String> = std::fs::read_dir(out).map_err(|e| format!("cannot read {out}: {e}"))?.filter_map(|e| e.ok()?.file_name().into_string().ok()).collect();
    names.sort();
    for name in names.iter().filter(|nm| own_name(nm)) {
        let path = format!("{out}/{name}");
        if std::fs::metadata(&path).is_ok_and(|m| m.is_file()) && std::fs::canonicalize(&path).ok() != keep {
            std::fs::remove_file(&path).map_err(|e| format!("cannot remove {path}: {e}"))?;
        }
    }
    Ok(())
}

/// The bytes of a slice of plain numbers (little-endian, as every partition file is).
fn bytes<T: Copy>(v: &[T]) -> &[u8] {
    // SAFETY: T is u16, u32, u64 or f32 here: plain values without padding
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}

/// A file being written: buffered, synced when finished.
struct Out {
    path: String,
    w: std::io::BufWriter<std::fs::File>,
}
impl Out {
    fn create(path: String) -> Result<Out, String> {
        let f = std::fs::File::create(&path).map_err(|e| format!("cannot create {path}: {e}"))?;
        Ok(Out { w: std::io::BufWriter::with_capacity(1 << 16, f), path })
    }
    fn put(&mut self, b: &[u8]) -> Result<(), String> {
        self.w.write_all(b).map_err(|e| format!("cannot write {}: {e}", self.path))
    }
    fn finish(self) -> Result<(), String> {
        let path = self.path;
        let f = self.w.into_inner().map_err(|e| format!("cannot write {path}: {}", e.error()))?;
        f.sync_all().map_err(|e| format!("cannot write {path}: {e}"))
    }
}

/// A finished file mapped read-only (None when empty).
fn map(path: &str) -> Result<Option<memmap2::Mmap>, String> {
    let f = std::fs::File::open(path).map_err(|e| format!("cannot open {path}: {e}"))?;
    if f.metadata().map_err(|e| format!("cannot read {path}: {e}"))?.len() == 0 {
        return Ok(None);
    }
    // SAFETY: written by this run and finished; nothing writes it again while mapped
    Ok(Some(unsafe { memmap2::Mmap::map(&f).map_err(|e| format!("cannot map {path}: {e}"))? }))
}
fn words<T>(m: &Option<memmap2::Mmap>) -> &[T] {
    // a map starts on a page boundary, so it is aligned for u16 and u64
    m.as_ref().map_or(&[], |m| unsafe { std::slice::from_raw_parts(m.as_ptr() as *const T, m.len() / std::mem::size_of::<T>()) })
}

/// numpy's (and Python's) round: to nearest, ties to even.
fn round_even(x: f64) -> f64 {
    x.round_ties_even()
}

/// JSON as Python's `json.dump(..., indent=1)` lays it out, keys in the order given (the manifest's, as
/// tools/partition.py writes it).
enum J {
    Raw(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}
impl J {
    fn write(&self, ind: usize, s: &mut String) {
        let pad = |d: usize| " ".repeat(d);
        match self {
            J::Raw(x) => s.push_str(x),
            J::Arr(v) if v.is_empty() => s.push_str("[]"),
            J::Obj(v) if v.is_empty() => s.push_str("{}"),
            J::Arr(v) => {
                s.push('[');
                for (i, x) in v.iter().enumerate() {
                    s.push_str(if i > 0 { ",\n" } else { "\n" });
                    s.push_str(&pad(ind + 1));
                    x.write(ind + 1, s);
                }
                s.push('\n');
                s.push_str(&pad(ind));
                s.push(']');
            }
            J::Obj(v) => {
                s.push('{');
                for (i, (k, x)) in v.iter().enumerate() {
                    s.push_str(if i > 0 { ",\n" } else { "\n" });
                    s.push_str(&pad(ind + 1));
                    s.push_str(&serde_json::Value::from(k.as_str()).to_string());
                    s.push_str(": ");
                    x.write(ind + 1, s);
                }
                s.push('\n');
                s.push_str(&pad(ind));
                s.push('}');
            }
        }
    }
}
fn raw<T: Into<serde_json::Value>>(x: T) -> J {
    J::Raw(x.into().to_string())
}

/// Partition the vectors in `vectors` (N x n0 .npy, f16/f32/f64; row i is entity i) into the directory `out` (made if
/// missing), as `tools/partition.py VECTORS OUT` does with the same options (module comment). The input is checked
/// before anything in `out` is touched; then an earlier partition there is removed (`clear_partition`) and the new one
/// written, its manifest last. An input that is itself one of the files a partition writes in `out` is refused.
pub fn partition(vectors: &str, out: &str, o: &PartitionOptions, log: &mut dyn FnMut(String)) -> Result<Summary, String> {
    let t = Timer::new();
    if o.cells == 0 || o.cross == 0 || o.sample_per_centre == 0 || o.cell_sample == 0 {
        return Err(format!("the partition's cells, cross, sample per centre and cell sample are at least 1, not {}, {}, {}, {}", o.cells, o.cross, o.sample_per_centre, o.cell_sample));
    }
    if matches!(o.per_cluster, PerCluster::Items(0)) {
        return Err("the cluster size is a positive number of items or by-n, not 0".into());
    }
    // the input first: nothing in `out` is touched unless it is a matrix of vectors
    let x = Vectors::open(vectors)?;
    let (nn, n0) = (x.rows, x.n0);
    let n = n0.div_ceil(32) * 32;
    let pc = match o.per_cluster {
        PerCluster::ByN => crate::fit::per_cluster_by_n(nn),
        PerCluster::Items(m) => m,
    };
    std::fs::create_dir_all(out).map_err(|e| format!("cannot create {out}: {e}"))?;
    if let Ok(src) = std::fs::canonicalize(vectors) {
        let rd = std::fs::read_dir(out).map_err(|e| format!("cannot read {out}: {e}"))?;
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if (own_name(&name) || name == "manifest.json") && std::fs::canonicalize(e.path()).is_ok_and(|p| p == src) {
                return Err(format!("{vectors} is {out}/{name}, a file the partition writes; move the vectors or partition into another directory"));
            }
        }
    }
    clear_partition(out, vectors)?;
    let k_all = (round_even(nn as f64 / pc as f64) as usize).max(1);
    let nc = o.cells.min(k_all).max(1);
    let f = |name: &str| format!("{out}/{name}");
    let mut srng = ChaCha8Rng::seed_from_u64(o.seed);
    srng.set_stream(1);
    log(format!("N {nn} n {n0} (padded to {n}): K {k_all} clusters ({pc} per cluster, {}) in {nc} cell(s), cross {}", o.per_cluster.arg(), o.cross));

    // coarse cells from a sample, then every row to its cell's files
    let coarse: Vec<f32> = if nc > 1 {
        let mut idx = rand::seq::index::sample(&mut srng, nn, nn.min(o.cell_sample * nc)).into_vec();
        idx.sort_unstable();
        let mut s = vec![0f32; idx.len() * n];
        let bad: Option<usize> = s.par_chunks_mut(n).zip(&idx).filter_map(|(r, &i)| (!x.unit(i, r)).then_some(i)).min();
        if let Some(i) = bad {
            return Err(format!("{vectors}: row {i} is not finite"));
        }
        kmeans(&F32Rows(&s, n), n, nc, o.seed, o.niter)
    } else {
        Vec::new()
    };
    let mut files: Vec<(Out, Out)> = (0..nc).map(|c| Ok((Out::create(f(&format!("cell{c}.f16")))?, Out::create(f(&format!("cell{c}.ids")))?))).collect::<Result<_, String>>()?;
    let mut rows = vec![0usize; nc];
    let mut u16s = vec![0u16; CHUNK * n];
    let mut cell = vec![0u32; CHUNK];
    for i0 in (0..nn).step_by(CHUNK) {
        let m = CHUNK.min(nn - i0);
        let bad: Option<usize> = u16s[..m * n].par_chunks_mut(BR * n).zip(cell[..m].par_chunks_mut(BR)).enumerate().filter_map(|(b, (h, cl))| {
            let r = cl.len();
            let mut u = vec![0f32; r * n];
            for (j, uj) in u.chunks_mut(n).enumerate() {
                if !x.unit(i0 + b * BR + j, uj) {
                    return Some(i0 + b * BR + j);
                }
            }
            h.reinterpret_cast_mut::<f16>().convert_from_f32_slice(&u);
            if nc > 1 {
                block_nearest(&u, n, &coarse, cl, &mut vec![0f32; r]);
            }
            None
        }).min();
        if let Some(i) = bad {
            return Err(format!("{vectors}: row {i} is not finite"));
        }
        if nc == 1 {
            let ids: Vec<u64> = (i0 as u64..(i0 + m) as u64).collect();
            files[0].0.put(bytes(&u16s[..m * n]))?;
            files[0].1.put(bytes(&ids))?;
            rows[0] += m;
        } else {
            for j in 0..m {
                let c = cell[j] as usize;
                files[c].0.put(bytes(&u16s[j * n..(j + 1) * n]))?;
                files[c].1.put(&((i0 + j) as u64).to_le_bytes())?;
                rows[c] += 1;
            }
        }
    }
    drop((u16s, cell));
    for (fv, fi) in files {
        fv.finish()?;
        fi.finish()?;
    }
    log(format!("pass 1: cells of {}..{} rows ({:.1}s)", rows.iter().min().unwrap(), rows.iter().max().unwrap(), t.s()));

    // fine centres per cell, in proportion to its rows, from a sample of the cell
    let kc: Vec<usize> = rows.iter().map(|&r| if r > 0 { (round_even((k_all as u128 * r as u128) as f64 / nn as f64) as usize).max(1) } else { 0 }).collect();
    let mut cent: Vec<f32> = Vec::new();
    let mut ccell: Vec<u32> = Vec::new();
    for c in 0..nc {
        if kc[c] == 0 {
            continue;
        }
        let m = rows[c].min(o.sample_per_centre * kc[c]);
        let mut sidx = rand::seq::index::sample(&mut srng, rows[c], m).into_vec();
        sidx.sort_unstable();
        let cm = map(&f(&format!("cell{c}.f16")))?;
        let all = words::<u16>(&cm);
        let mut s = vec![0u16; m * n];
        for (d, &i) in s.chunks_mut(n).zip(&sidx) {
            d.copy_from_slice(&all[i * n..(i + 1) * n]);
        }
        let kk = kc[c].min(m);
        cent.extend(kmeans(&F16Rows(&s, n), n, kk, o.seed + 1 + c as u64, o.niter));
        ccell.extend(std::iter::repeat(c as u32).take(kk));
    }
    cent.chunks_mut(n).for_each(normalise);
    let k = ccell.len();
    let first: Vec<usize> = (0..=nc).map(|c| ccell.partition_point(|&x| (x as usize) < c)).collect();
    log(format!("{k} fine centres, {}..{} per cell ({:.1}s)", kc.iter().min().unwrap(), kc.iter().max().unwrap(), t.s()));

    // every row to its nearest fine centre among its cell's `cross` nearest cells; rows of another cell's centre move
    // there
    let near: Vec<Vec<usize>> = if nc > 1 {
        (0..nc).map(|a| {
            let sims: Vec<f64> = (0..nc).map(|b| (0..n).map(|t| coarse[a * n + t] as f64 * coarse[b * n + t] as f64).sum()).collect();
            let mut order: Vec<usize> = (0..nc).collect();
            order.sort_by(|&p, &q| sims[q].total_cmp(&sims[p]));
            order.truncate(o.cross);
            order.sort_unstable();
            order
        }).collect()
    } else {
        vec![vec![0]]
    };
    let mut moved = 0usize;
    let mut outs: std::collections::BTreeMap<usize, [Out; 3]> = std::collections::BTreeMap::new();
    let mut sizes = vec![0usize; k];
    let mut objective = 0f64;
    for c in 0..nc {
        let cand: Vec<u32> = near[c].iter().flat_map(|&d| first[d] as u32..first[d + 1] as u32).collect();
        let r = rows[c];
        let mut cl = vec![0u32; r];
        let cm = map(&f(&format!("cell{c}.f16")))?;
        if r > 0 {
            if cand.is_empty() {
                return Err(format!("cell {c} has {r} rows and no fine centre among its nearest cells"));
            }
            let sub: Vec<f32> = cand.iter().flat_map(|&j| cent[j as usize * n..(j as usize + 1) * n].iter().copied()).collect();
            let rr = F16Rows(words::<u16>(&cm), n);
            let sums: Vec<f64> = cl.par_chunks_mut(BR).enumerate().map(|(b, l)| {
                let mut xb = vec![0f32; l.len() * n];
                for (i, xi) in xb.chunks_mut(n).enumerate() {
                    rr.get(b * BR + i, xi);
                }
                let mut best = vec![0f32; l.len()];
                block_nearest(&xb, n, &sub, l, &mut best);
                for v in l.iter_mut() {
                    *v = cand[*v as usize];
                }
                best.iter().map(|&s| s as f64).sum::<f64>()
            }).collect();
            objective += sums.iter().sum::<f64>();
        }
        for &j in &cl {
            sizes[j as usize] += 1;
        }
        let mut w = Out::create(f(&format!("cell{c}.cl")))?;
        w.put(bytes(&cl))?;
        w.finish()?;
        let away: Vec<usize> = (0..r).filter(|&i| ccell[cl[i] as usize] as usize != c).collect();
        if !away.is_empty() {
            let im = map(&f(&format!("cell{c}.ids")))?;
            let (ids, vecs) = (words::<u64>(&im), words::<u16>(&cm));
            let mut dests: Vec<usize> = away.iter().map(|&i| ccell[cl[i] as usize] as usize).collect();
            dests.sort_unstable();
            dests.dedup();
            for d in dests {
                if !outs.contains_key(&d) {
                    outs.insert(d, [Out::create(f(&format!("cell{d}.in.f16")))?, Out::create(f(&format!("cell{d}.in.ids")))?, Out::create(f(&format!("cell{d}.in.cl")))?]);
                }
                let w = outs.get_mut(&d).unwrap();
                for &i in away.iter().filter(|&&i| ccell[cl[i] as usize] as usize == d) {
                    w[0].put(bytes(&vecs[i * n..(i + 1) * n]))?;
                    w[1].put(&ids[i].to_le_bytes())?;
                    w[2].put(&cl[i].to_le_bytes())?;
                }
            }
            moved += away.len();
        }
    }
    let dests: Vec<usize> = outs.keys().copied().collect();
    for (_, ws) in outs {
        for w in ws {
            w.finish()?;
        }
    }
    let objective = objective / nn as f64;
    let mut sorted = sizes.clone();
    sorted.sort_unstable();
    let mean = nn as f64 / k as f64;
    let sd = (sizes.iter().map(|&s| (s as f64 - mean).powi(2)).sum::<f64>() / k as f64).sqrt();
    log(format!("pass 2: {moved} rows ({:.2}%) moved to a centre in another cell; mean cosine to the centre {objective:.6}; cluster sizes {}..{} (median {}, sd/mean {:.3}) ({:.1}s)",
        100.0 * moved as f64 / nn as f64, sorted[0], sorted[k - 1], sorted[k / 2], sd / mean, t.s()));

    let mut c_npy = npy_header_bytes("<f4", &[k, n]);
    c_npy.extend_from_slice(bytes(&cent));
    let mut w = Out::create(f("centres.npy"))?;
    w.put(&c_npy)?;
    w.finish()?;
    let mut cc_npy = npy_header_bytes("<u4", &[k]);
    cc_npy.extend_from_slice(bytes(&ccell));
    let mut w = Out::create(f("cluster_cell.npy"))?;
    w.put(&cc_npy)?;
    w.finish()?;
    let src = std::path::absolute(vectors).map_err(|e| format!("{vectors}: {e}"))?.to_string_lossy().to_string();
    let st = std::fs::metadata(vectors).map_err(|e| format!("cannot read {vectors}: {e}"))?;
    let mtime = st.modified().ok().and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0.0, |d| d.as_secs_f64());
    let names: Vec<String> = (0..nc).flat_map(|c| ["f16", "ids", "cl"].map(|x| format!("cell{c}.{x}"))).chain(dests.iter().flat_map(|d| ["f16", "ids", "cl"].map(|x| format!("cell{d}.in.{x}")))).chain(["centres.npy".to_string(), "cluster_cell.npy".to_string()]).collect();
    let mut flist = Vec::new();
    for nm in &names {
        let len = std::fs::metadata(f(nm)).map_err(|e| format!("cannot read {}: {e}", f(nm)))?.len();
        flist.push((nm.clone(), raw(len)));
    }
    let created = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()) as u64;
    let man = J::Obj(vec![
        ("N".into(), raw(nn as u64)),
        ("n".into(), raw(n as u64)),
        ("n_orig".into(), raw(n0 as u64)),
        ("K".into(), raw(k as u64)),
        ("cells".into(), raw(nc as u64)),
        ("cross".into(), raw(o.cross as u64)),
        ("seed".into(), raw(o.seed)),
        ("source".into(), raw(src.as_str())),
        ("source_bytes".into(), raw(st.len())),
        ("source_mtime".into(), raw(mtime)),
        ("per_cluster".into(), raw(o.per_cluster.arg())),
        ("rows".into(), J::Arr(rows.iter().map(|&r| raw(r as u64)).collect())),
        ("moved".into(), raw(moved as u64)),
        ("files".into(), J::Obj(flist)),
        ("created".into(), raw(created)),
        ("token".into(), raw(format!("{:016x}", rand::random::<u64>()))),
    ]);
    let mut text = String::new();
    man.write(0, &mut text);
    let mut w = Out::create(f("manifest.json.tmp"))?;
    w.put(text.as_bytes())?;
    w.finish()?;
    std::fs::rename(f("manifest.json.tmp"), f("manifest.json")).map_err(|e| format!("cannot write {}: {e}", f("manifest.json")))?;
    if let Ok(d) = std::fs::File::open(out) {
        let _ = d.sync_all();
    }
    log(format!("wrote {out} ({:.1}s)", t.s()));
    Ok(Summary { rows: nn, k, cells: nc, moved, objective, sizes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::{write_npy, TmpDir};

    /// m rows of n0 dimensions around `g` directions (row i in group i % g), f32, deterministic.
    fn mixture(m: usize, n0: usize, g: usize, spread: f32, seed: u64) -> (Vec<f32>, Vec<usize>) {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let dirs: Vec<f32> = (0..g * n0).map(|_| rng.gen::<f32>() - 0.5).collect();
        let mut x = Vec::with_capacity(m * n0);
        for i in 0..m {
            let d = i % g;
            x.extend((0..n0).map(|t| dirs[d * n0 + t] + spread * (rng.gen::<f32>() - 0.5)));
        }
        (x, (0..m).map(|i| i % g).collect())
    }

    fn save_f32(path: &std::path::Path, x: &[f32], n0: usize) {
        write_npy(path.to_str().unwrap(), "<f4", &[x.len() / n0, n0], bytes(x));
    }

    /// Row `r` of `x` as the partition stores it: unit (f64 norm), zero-padded to n, f16.
    fn unit_f16(x: &[f32], n0: usize, n: usize, r: usize) -> Vec<u16> {
        let row = &x[r * n0..(r + 1) * n0];
        let nr = row.iter().map(|&v| v as f64 * v as f64).sum::<f64>().sqrt();
        (0..n).map(|t| f16::from_f32(if t < n0 && nr > 0.0 { (row[t] as f64 / nr) as f32 } else { 0.0 }).to_bits()).collect()
    }

    /// The files of a directory with their contents; the manifest without its moment and token.
    fn files_of(dir: &std::path::Path) -> Vec<(String, Vec<u8>)> {
        let mut v: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir).unwrap().map(|e| {
            let e = e.unwrap();
            let name = e.file_name().into_string().unwrap();
            let mut b = std::fs::read(e.path()).unwrap();
            if name == "manifest.json" {
                let s = String::from_utf8(b).unwrap();
                b = s.lines().filter(|l| !l.contains("\"created\"") && !l.contains("\"token\"")).collect::<Vec<_>>().join("\n").into_bytes();
            }
            (name, b)
        }).collect();
        v.sort();
        v
    }

    fn run(threads: usize, vectors: &std::path::Path, out: &std::path::Path, o: &PartitionOptions) -> Summary {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        pool.install(|| partition(vectors.to_str().unwrap(), out.to_str().unwrap(), o, &mut |_| {})).unwrap()
    }

    /// The same seed and input give the same files with 1 and 4 threads (the manifest differs only in its moment and
    /// token), and the run moves rows between cells (so the moved-in files are compared too).
    #[test]
    fn the_same_partition_at_any_thread_count() {
        let tmp = TmpDir::new("part-threads");
        let (x, _) = mixture(3000, 40, 60, 1.5, 1);
        let v = tmp.join("v.npy");
        save_f32(&v, &x, 40);
        let o = PartitionOptions { cells: 3, cross: 2, per_cluster: PerCluster::Items(100), seed: 9, ..Default::default() };
        let a = run(1, &v, &tmp.join("a"), &o);
        let b = run(4, &v, &tmp.join("b"), &o);
        assert!(a.moved > 0 && a.cells == 3 && a.k == 30, "{a:?}");
        assert_eq!((a.moved, a.objective.to_bits(), &a.sizes), (b.moved, b.objective.to_bits(), &b.sizes));
        let (fa, fb) = (files_of(&tmp.join("a")), files_of(&tmp.join("b")));
        assert!(fa.iter().any(|(n, _)| n.contains(".in.")), "{:?}", fa.iter().map(|f| &f.0).collect::<Vec<_>>());
        assert_eq!(fa, fb);
        // a partition written again into the same directory: the same files, another identity
        let id = crate::bucket::partition_id(tmp.join("a").to_str().unwrap()).unwrap();
        run(4, &v, &tmp.join("a"), &o);
        assert_eq!(files_of(&tmp.join("a")), fa);
        assert_ne!(crate::bucket::partition_id(tmp.join("a").to_str().unwrap()).unwrap(), id);
        // another seed, another partition
        run(4, &v, &tmp.join("c"), &PartitionOptions { seed: 10, ..o });
        assert_ne!(files_of(&tmp.join("c")), fa);
    }

    /// What the partition writes reads back through `bucket::Partition`: every source row once among the cells' own
    /// rows, stored as its unit f16 vector padded to n, in the cluster of the nearest centre of its cell's nearest
    /// cells; a row whose cluster is in another cell is in that cell's moved-in rows too, with the same vector and
    /// cluster. Then a fit of a partition reads every fit row through `bucket::Buckets` as the entity's unit vector.
    #[test]
    fn a_partition_reads_back_through_buckets() {
        let tmp = TmpDir::new("part-format");
        let (n0, n) = (40, 64);
        let (x, _) = mixture(3000, n0, 60, 1.5, 2);
        let v = tmp.join("v.npy");
        save_f32(&v, &x, n0);
        let out = tmp.join("p");
        let s = run(4, &v, &out, &PartitionOptions { cells: 3, cross: 2, per_cluster: PerCluster::Items(100), seed: 3, ..Default::default() });
        let p = crate::bucket::Partition::open(out.to_str().unwrap()).unwrap();
        assert_eq!((p.n, p.n_orig, p.rows, p.k, p.cells.len()), (n, n0, 3000, s.k, 3));
        let cent = crate::util::read_npy(out.join("centres.npy").to_str().unwrap());
        assert_eq!(cent.shape, vec![s.k, n]);
        let c32 = cent.f32s();
        assert!(c32.chunks(n).all(|r| (r.iter().map(|&v| v as f64 * v as f64).sum::<f64>() - 1.0).abs() < 1e-5));
        assert!(p.cluster_cell.windows(2).all(|w| w[0] <= w[1]), "centres numbered cell by cell");
        let dot = |e: usize, j: usize| -> f64 {
            let u = unit_f16(&x, n0, n, e);
            (0..n).map(|t| f16::from_bits(u[t]).to_f64() * c32[j * n + t] as f64).sum()
        };
        let mut seen = vec![0u32; 3000];
        let (mut moved_out, mut moved_in) = (Vec::new(), Vec::new());
        let mut cand_cells = vec![std::collections::BTreeSet::new(); 3];
        for (c, cell) in p.cells.iter().enumerate() {
            for i in 0..cell.own {
                let e = cell.id(i) as usize;
                seen[e] += 1;
                assert_eq!(cell.vector(n, i), &unit_f16(&x, n0, n, e)[..], "entity {e}");
                let j = cell.cluster(i) as usize;
                let home = p.cluster_cell[j] as usize;
                cand_cells[c].insert(home);
                if home != c {
                    moved_out.push((home, e, j as u32));
                }
            }
        }
        assert!(seen.iter().all(|&s| s == 1));
        for (c, cell) in p.cells.iter().enumerate() {
            assert!(cand_cells[c].len() <= 2 && (cell.own == 0 || cand_cells[c].contains(&c) || cand_cells[c].len() == 1), "cell {c}: {:?}", cand_cells[c]);
            for i in 0..cell.own {
                // no centre of the cells its rows went to is nearer (f16 rows against the centres, in f64)
                let (e, j) = (cell.id(i) as usize, cell.cluster(i) as usize);
                assert!((0..s.k).filter(|&q| cand_cells[c].contains(&(p.cluster_cell[q] as usize))).all(|q| dot(e, q) <= dot(e, j) + 1e-4), "entity {e}");
            }
            for i in cell.own..cell.rows() {
                let e = cell.id(i) as usize;
                assert_eq!(cell.vector(n, i), &unit_f16(&x, n0, n, e)[..]);
                assert_eq!(p.cluster_cell[cell.cluster(i) as usize] as usize, c);
                moved_in.push((c, e, cell.cluster(i)));
            }
        }
        moved_out.sort();
        moved_in.sort();
        assert!(s.moved > 0);
        assert_eq!((moved_out.len(), &moved_out), (s.moved, &moved_in));

        // fitted: every fit row is its entity's unit vector
        let synth = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k/Xs.npy");
        let out = tmp.join("ps");
        run(4, std::path::Path::new(synth), &out, &PartitionOptions { cells: 2, cross: 2, per_cluster: PerCluster::Items(125), ..Default::default() });
        let fit = tmp.join("fit");
        let fo = crate::fit::FitOptions { cert_time: 0.0, ..crate::fit::FitOptions::default() };
        crate::fit::fit(fit.to_str().unwrap(), out.to_str().unwrap(), &fo, &mut |_| {}).unwrap();
        let b = crate::bucket::Buckets::open(out.to_str().unwrap(), fit.to_str().unwrap()).unwrap();
        let xs = Vectors::open(synth).unwrap();
        assert_eq!(b.rows(), 4000);
        let mut seen = vec![false; 4000];
        let mut u = vec![0f32; 64];
        for r in 0..4000 {
            use crate::bucket::FitRows;
            let e = b.id(r) as usize;
            assert!(!seen[e]);
            seen[e] = true;
            assert!(xs.unit(e, &mut u));
            let want: Vec<u16> = u.iter().map(|&v| f16::from_f32(v).to_bits()).collect();
            assert_eq!(b.row(r), &want[..], "fit row {r}");
        }
    }

    /// On well-separated groups the k-means finds the groups whenever its starting rows fall in every group (which a
    /// k-means without restarts needs, faiss's too), and some seeds do.
    #[test]
    fn kmeans_finds_separated_groups() {
        let (n0, g, m) = (32, 4, 400);
        let (x, grp) = mixture(m, n0, g, 0.05, 4);
        let mut u = x.clone();
        u.chunks_mut(n0).for_each(normalise);
        let rows = F32Rows(&u, n0);
        let mut covered = 0;
        for seed in 0..40u64 {
            let start = rand::seq::index::sample(&mut ChaCha8Rng::seed_from_u64(seed), m, g).into_vec();
            let mut gs: Vec<usize> = start.iter().map(|&i| grp[i]).collect();
            gs.sort();
            gs.dedup();
            if gs.len() < g {
                continue;
            }
            covered += 1;
            let c = kmeans(&rows, n0, g, seed, 20);
            let (lab, best) = nearest(&rows, n0, &c);
            for q in 0..g {
                let mut ls: Vec<u32> = (0..m).filter(|&i| grp[i] == q).map(|i| lab[i]).collect();
                ls.sort();
                ls.dedup();
                assert_eq!(ls.len(), 1, "seed {seed}: group {q} in clusters {ls:?}");
            }
            let mut all = lab.clone();
            all.sort();
            all.dedup();
            assert_eq!(all.len(), g, "seed {seed}");
            assert!(best.iter().all(|&b| b > 0.99), "seed {seed}");
        }
        assert!(covered > 0);
    }

    /// An empty cluster is split as faiss splits one: the chosen cluster's centre is copied with a relative 1/1024 up
    /// and down in alternate dimensions, its size shared; with as many rows as clusters, a cluster of two or more is
    /// taken. Through the k-means: a set with many equal rows (which leave clusters empty) ends with unit, finite
    /// centres, the same for a seed.
    #[test]
    fn empty_clusters_are_split() {
        let e = 1.0f32 / 1024.0;
        let mut c = vec![1.0f32, 2.0, 3.0, 4.0, 0.0, 0.0, 0.0, 0.0, 9.0, 9.0, 9.0, 9.0];
        let mut size = vec![5.0, 0.0, 1.0];
        let mut rng = ChaCha8Rng::seed_from_u64(0);
        // m - k = 3: cluster 0 is taken with probability 4/3, cluster 2 never
        assert_eq!(split_empty(&mut c, &mut size, 4, 6, &mut rng), 1);
        assert_eq!(&c[4..8], &[1.0 * (1.0 + e), 2.0 * (1.0 - e), 3.0 * (1.0 + e), 4.0 * (1.0 - e)]);
        assert_eq!(&c[0..4], &[1.0 * (1.0 - e), 2.0 * (1.0 + e), 3.0 * (1.0 - e), 4.0 * (1.0 + e)]);
        assert_eq!(&c[8..], &[9.0; 4]);
        assert_eq!(size, vec![2.5, 2.5, 1.0]);
        // as many rows as clusters: the cluster of two
        let mut c = vec![1.0f32, 0.0, 0.0, 1.0, 0.0, 0.0];
        let mut size = vec![1.0, 2.0, 0.0];
        assert_eq!(split_empty(&mut c, &mut size, 2, 3, &mut rng), 1);
        assert_eq!((&c[2..], &size[..]), (&[0.0, 1.0 + e, 0.0, 1.0 - e][..], &[1.0, 1.0, 1.0][..]));

        // 8 equal rows and 2 others, 4 centres: the equal rows leave clusters empty at every iteration
        let n0 = 8;
        let mut u = vec![0f32; 10 * n0];
        for r in 0..8 {
            u[r * n0] = 1.0;
        }
        u[8 * n0 + 1] = 1.0;
        u[9 * n0 + 2] = 1.0;
        let rows = F32Rows(&u, n0);
        let unit = |c: &[f32]| c.chunks(n0).all(|r| r.iter().all(|v| v.is_finite()) && (r.iter().map(|&v| v as f64 * v as f64).sum::<f64>() - 1.0).abs() < 1e-6);
        for seed in 0..6 {
            let c = kmeans(&rows, n0, 4, seed, 10);
            assert!(unit(&c), "seed {seed}: {c:?}");
            // the equal rows have their centre (the two others, orthogonal to everything, may share one: a local
            // optimum of any k-means)
            assert!(c.chunks(n0).any(|r| r[0] > 0.99), "seed {seed}: no centre at the equal rows: {c:?}");
            assert_eq!(kmeans(&rows, n0, 4, seed, 10), c);
        }
        // as many rows as centres, two of them equal
        assert!(unit(&kmeans(&F32Rows(&u[6 * n0..], n0), n0, 4, 1, 5)));
    }

    /// `clear_partition` removes the manifest and the regular files of a partition's names, and nothing else: not a
    /// name it does not write, not a directory, not the input (by name or through a link). A partition whose input is
    /// one of its own files is refused before anything is removed; one into a directory with other files keeps them
    /// and removes a stale cell.
    #[test]
    fn clearing_removes_only_the_partitions_own_files() {
        let tmp = TmpDir::new("part-clear");
        let d = tmp.join("p");
        std::fs::create_dir_all(d.join("cell3.ids")).unwrap();
        let (x, _) = mixture(500, 16, 4, 0.3, 5);
        let mut input = npy_header_bytes("<f4", &[500, 16]);
        input.extend_from_slice(bytes(&x));
        std::fs::write(d.join("cell1.f16"), &input).unwrap();
        std::os::unix::fs::symlink(d.join("cell1.f16"), d.join("cell2.f16")).unwrap();
        let own = ["cell0.f16", "cell0.ids", "cell0.cl", "cell12.in.cl", "cell7.in.f16", "centres.npy", "cluster_cell.npy", "manifest.json.tmp", "manifest.json"];
        let foreign = ["cell0.f16.bak", "cellx.f16", "cell.f16", "cell0.f32", "cell0.in.npy", "notes.txt", "centres.npy.old", "Cell0.f16", "manifest.json.old"];
        for f in own.iter().chain(&foreign) {
            std::fs::write(d.join(f), b"x").unwrap();
        }
        let input_path = d.join("cell1.f16");
        let (ds, ip) = (d.to_str().unwrap(), input_path.to_str().unwrap());
        let e = partition(ip, ds, &PartitionOptions::default(), &mut |_| {}).unwrap_err();
        assert!(e.contains("a file the partition writes"), "{e}");
        for f in own.iter().chain(&foreign) {
            assert!(d.join(f).exists(), "{f} removed by a refused partition");
        }
        clear_partition(ds, ip).unwrap();
        for f in own {
            assert!(!d.join(f).exists(), "{f} kept");
        }
        for f in foreign {
            assert!(d.join(f).exists(), "{f} removed");
        }
        assert!(d.join("cell3.ids").is_dir());
        assert_eq!(std::fs::read(d.join("cell1.f16")).unwrap(), input);
        assert!(d.join("cell2.f16").exists(), "the link to the input removed");

        // the input elsewhere: a partition into the directory keeps the foreign files and removes stale cells
        let v = tmp.join("v.npy");
        std::fs::write(&v, &input).unwrap();
        std::fs::remove_file(d.join("cell2.f16")).unwrap();
        std::fs::remove_file(d.join("cell1.f16")).unwrap();
        std::fs::remove_dir(d.join("cell3.ids")).unwrap();
        for f in ["cell5.f16", "cell5.in.ids"] {
            std::fs::write(d.join(f), b"stale").unwrap();
        }
        partition(v.to_str().unwrap(), ds, &PartitionOptions { per_cluster: PerCluster::Items(100), ..Default::default() }, &mut |_| {}).unwrap();
        for f in foreign {
            assert!(d.join(f).exists(), "{f} removed");
        }
        assert!(!d.join("cell5.f16").exists() && !d.join("cell5.in.ids").exists() && !d.join("manifest.json.tmp").exists());
        assert_eq!(std::fs::read(&v).unwrap(), input);
        crate::bucket::partition_id(ds).unwrap();
    }

    /// An input that is not N x n floats, or has a row that is not finite, is refused, and an earlier partition in the
    /// directory stays as it was (the input is read before anything is removed). f16, f32 and f64 inputs of the same
    /// values give the same partition.
    #[test]
    fn inputs_are_checked_first_and_any_float_width_is_read() {
        let tmp = TmpDir::new("part-bad");
        let (x, _) = mixture(300, 8, 3, 0.3, 6);
        let v = tmp.join("v.npy");
        save_f32(&v, &x, 8);
        let d = tmp.join("p");
        let ds = d.to_str().unwrap();
        let o = PartitionOptions { per_cluster: PerCluster::Items(100), ..Default::default() };
        partition(v.to_str().unwrap(), ds, &o, &mut |_| {}).unwrap();
        let id = crate::bucket::partition_id(ds).unwrap();
        let bad = |name: &str, descr: &str, shape: &[usize], data: &[u8], says: &str| {
            let p = tmp.join(name);
            write_npy(p.to_str().unwrap(), descr, shape, data);
            let e = partition(p.to_str().unwrap(), ds, &o, &mut |_| {}).unwrap_err();
            assert!(e.contains(says), "{name}: {e}");
        };
        bad("i4.npy", "<i4", &[4, 2], &[0u8; 32], "expected N x n");
        bad("flat.npy", "<f4", &[8], &[0u8; 32], "expected N x n");
        bad("short.npy", "<f4", &[4, 2], &[0u8; 16], "bytes of data");
        bad("empty.npy", "<f4", &[0, 2], &[], "expected N x n");
        std::fs::write(tmp.join("text.npy"), b"not an npy file").unwrap();
        assert!(partition(tmp.join("text.npy").to_str().unwrap(), ds, &o, &mut |_| {}).unwrap_err().contains("expected N x n"));
        assert!(partition(tmp.join("missing.npy").to_str().unwrap(), ds, &o, &mut |_| {}).unwrap_err().contains("cannot open"));
        assert_eq!(crate::bucket::partition_id(ds).unwrap(), id);
        let mut y = x.clone();
        y[5 * 8 + 3] = f32::NAN;
        let p = tmp.join("nan.npy");
        save_f32(&p, &y, 8);
        let e = partition(p.to_str().unwrap(), ds, &o, &mut |_| {}).unwrap_err();
        assert!(e.contains("row 5 is not finite"), "{e}");
        // the same values in f16, f32 and f64
        let w16: Vec<u8> = x.iter().flat_map(|&v| f16::from_f32(v).to_le_bytes()).collect();
        let x16: Vec<f32> = x.iter().map(|&v| f16::from_f32(v).to_f32()).collect();
        write_npy(tmp.join("h.npy").to_str().unwrap(), "<f2", &[300, 8], &w16);
        save_f32(&tmp.join("h32.npy"), &x16, 8);
        let w64: Vec<u8> = x.iter().flat_map(|&v| (v as f64).to_le_bytes()).collect();
        write_npy(tmp.join("d.npy").to_str().unwrap(), "<f8", &[300, 8], &w64);
        let cells = |name: &str, out: &str| {
            partition(tmp.join(name).to_str().unwrap(), tmp.join(out).to_str().unwrap(), &o, &mut |_| {}).unwrap();
            ["cell0.f16", "cell0.ids", "cell0.cl", "centres.npy"].map(|f| std::fs::read(tmp.join(out).join(f)).unwrap())
        };
        assert_eq!(cells("h.npy", "ph"), cells("h32.npy", "ph32"));
        assert_eq!(cells("d.npy", "pd"), cells("v.npy", "pv"));
    }
}
