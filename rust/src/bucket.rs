//! A partition's cells (`tools/partition.py`), mapped: each cell's own rows and the rows that moved into it, f16 unit
//! vectors zero-padded to n. With a fit's row order (`ord{c}.u32`, `clcell.npy` and `clpos.npy`, written by
//! `fit::fit`), any fit row is read without holding the vectors in memory.

use crate::util::try_read_npy;

/// Unit f16 vectors by row, in the order of a fit's `cstart` and `perm`: its Xs.npy (`RealSource`), or its partition
/// (`Buckets`).
pub trait FitRows: Sync {
    fn n(&self) -> usize;
    fn row(&self, r: usize) -> &[u16];
}

impl FitRows for crate::index::RealSource {
    fn n(&self) -> usize {
        self.n
    }
    fn row(&self, r: usize) -> &[u16] {
        crate::index::RealSource::row(self, r)
    }
}

/// The file mapped read-only (None for a missing or empty file, which cannot be mapped).
fn map(path: &str) -> Result<Option<memmap2::Mmap>, String> {
    let f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("cannot open {path}: {e}")),
    };
    if f.metadata().map_err(|e| format!("cannot read {path}: {e}"))?.len() == 0 {
        return Ok(None);
    }
    // the partition's and the fit's files are written once and only read afterwards
    Ok(Some(unsafe { memmap2::Mmap::map(&f).map_err(|e| format!("cannot map {path}: {e}"))? }))
}

fn words<T>(m: &Option<memmap2::Mmap>) -> &[T] {
    // a map starts on a page boundary, so it is aligned for any T here (u16, u32, u64)
    m.as_ref().map_or(&[], |m| unsafe { std::slice::from_raw_parts(m.as_ptr() as *const T, m.len() / std::mem::size_of::<T>()) })
}

/// One cell: its own rows (`cell{c}.*`) and the rows moved into it (`cell{c}.in.*`). Local row i is own row i below
/// `own`, else moved-in row i - own. Own rows assigned to another cell's cluster are still counted here, and skipped
/// by the fit.
pub struct Cell {
    vec: Option<memmap2::Mmap>,
    ids: Option<memmap2::Mmap>,
    cl: Option<memmap2::Mmap>,
    in_vec: Option<memmap2::Mmap>,
    in_ids: Option<memmap2::Mmap>,
    in_cl: Option<memmap2::Mmap>,
    pub own: usize,
}

impl Cell {
    pub fn rows(&self) -> usize {
        self.own + words::<u32>(&self.in_cl).len()
    }
    pub fn vector(&self, n: usize, i: usize) -> &[u16] {
        let (v, j) = if i < self.own { (words::<u16>(&self.vec), i) } else { (words::<u16>(&self.in_vec), i - self.own) };
        &v[j * n..(j + 1) * n]
    }
    pub fn id(&self, i: usize) -> u64 {
        if i < self.own { words::<u64>(&self.ids)[i] } else { words::<u64>(&self.in_ids)[i - self.own] }
    }
    pub fn cluster(&self, i: usize) -> u32 {
        if i < self.own { words::<u32>(&self.cl)[i] } else { words::<u32>(&self.in_cl)[i - self.own] }
    }
}

pub struct Partition {
    pub n: usize,
    pub n_orig: usize,
    pub rows: usize,
    pub k: usize,
    pub cells: Vec<Cell>,
    pub cluster_cell: Vec<u32>,
}

/// The partition's manifest.json: its bytes and its contents, with every file it lists checked to exist at the length
/// it lists (an error naming the first that does not).
fn manifest(dir: &str) -> Result<(Vec<u8>, serde_json::Value), String> {
    let (b, m, missing) = manifest_missing(dir)?;
    if let Some((name, e)) = missing.into_iter().next() {
        return Err(format!("{dir}/{name}, which the partition's manifest lists, cannot be read ({e}); rerun tools/partition.py"));
    }
    Ok((b, m))
}

/// As `manifest`, with the listed files that do not exist (each with its error) returned instead of refused; a listed
/// file of another length, or one that exists but cannot be read, is still an error.
fn manifest_missing(dir: &str) -> Result<(Vec<u8>, serde_json::Value, Vec<(String, std::io::Error)>), String> {
    let path = format!("{dir}/manifest.json");
    let b = std::fs::read(&path).map_err(|e| format!("cannot read {path}: {e}; run tools/partition.py first"))?;
    let m: serde_json::Value = serde_json::from_slice(&b).map_err(|e| format!("{path}: {e}; rerun tools/partition.py"))?;
    let files = m["files"].as_object().ok_or(format!("{path} does not list the partition's files (an older tools/partition.py wrote it); rerun tools/partition.py"))?;
    let mut missing = Vec::new();
    for (name, len) in files {
        let want = len.as_u64().ok_or(format!("{path}: the length of {name} is not a number; rerun tools/partition.py"))?;
        if name.is_empty() || name.contains('/') || name.starts_with('.') {
            return Err(format!("{path} lists the file {name:?}, which is not a name in the partition's directory; rerun tools/partition.py"));
        }
        match std::fs::metadata(format!("{dir}/{name}")) {
            Ok(md) if md.len() == want => {}
            Ok(md) => return Err(format!("{dir}/{name} is {} bytes, the partition's manifest says {want}: the partition changed after it was written; rerun tools/partition.py", md.len())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => missing.push((name.clone(), e)),
            Err(e) => return Err(format!("{dir}/{name}, which the partition's manifest lists, cannot be read ({e}); rerun tools/partition.py")),
        }
    }
    Ok((b, m, missing))
}

impl Partition {
    /// The partition in `dir`: the cell files its manifest lists (with the lengths it lists), mapped. A file the
    /// manifest does not list is not read (a cell with no moved-in rows has no `in` files).
    pub fn open(dir: &str) -> Result<Partition, String> {
        let path = format!("{dir}/manifest.json");
        let (_, m) = manifest(dir)?;
        let us = |k: &str| m[k].as_u64().map(|x| x as usize).ok_or(format!("{path} has no {k}; rerun tools/partition.py"));
        let (n, nc) = (us("n")?, us("cells")?);
        let mut cells = Vec::with_capacity(nc);
        for c in 0..nc {
            let f = |x: &str| {
                let name = format!("cell{c}.{x}");
                if m["files"].get(&name).is_some() { map(&format!("{dir}/{name}")) } else { Ok(None) }
            };
            let cell = Cell { vec: f("f16")?, ids: f("ids")?, cl: f("cl")?, in_vec: f("in.f16")?, in_ids: f("in.ids")?, in_cl: f("in.cl")?, own: 0 };
            let own = words::<u64>(&cell.ids).len();
            let nin = words::<u32>(&cell.in_cl).len();
            if own.checked_mul(n) != Some(words::<u16>(&cell.vec).len()) || words::<u32>(&cell.cl).len() != own || nin.checked_mul(n) != Some(words::<u16>(&cell.in_vec).len()) || words::<u64>(&cell.in_ids).len() != nin {
                return Err(format!("{dir}/cell{c}: its vectors, ids and clusters disagree on the number of rows; rerun tools/partition.py"));
            }
            cells.push(Cell { own, ..cell });
        }
        let cluster_cell: Vec<u32> = try_read_npy(&format!("{dir}/cluster_cell.npy"))?.u32s();
        if cluster_cell.iter().any(|&c| c as usize >= nc) {
            return Err(format!("{dir}/cluster_cell.npy names a cell beyond its {nc}; rerun tools/partition.py"));
        }
        Ok(Partition { n, n_orig: us("n_orig")?, rows: us("N")?, k: cluster_cell.len(), cells, cluster_cell })
    }
}

/// The identity of the partition in `dir`, which a fit records (meta.json "partition_id") and a build and an open
/// check: a fixed hash of its manifest.json, after checking that every file the manifest lists has the length it
/// lists. tools/partition.py removes the manifest before it writes anything and writes it last, with the partition's
/// source, sizes, seed, files and lengths, and the moment and a random token of that run, so a partition written again
/// into the same directory has another identity. Only the content counts: a copy, a move or a chmod of the partition
/// keeps it (the fit records the manifest's file identity too, and a build warns when that changed). Reads the
/// manifest and the lengths of its files: O(cells), never the rows.
pub fn partition_id(dir: &str) -> Result<String, String> {
    let (b, _) = manifest(dir)?;
    Ok(id_of(&b))
}

/// As `partition_id`, with the files the manifest lists that do not exist named instead of refused (an open, which
/// reads no cell, takes their absence like a missing partition's): the identity of the manifest's content, and the
/// missing files. A listed file of another length is still an error.
pub fn partition_id_missing(dir: &str) -> Result<(String, Vec<String>), String> {
    let (b, _, missing) = manifest_missing(dir)?;
    Ok((id_of(&b), missing.into_iter().map(|(n, _)| n).collect()))
}

fn id_of(manifest: &[u8]) -> String {
    let mut h = crate::v2::store::Fx::default();
    h.str("vexact partition 2");
    h.bytes(manifest);
    format!("{:016x}", h.finish())
}

/// A fit's rows read from its partition: row i of the fit's cluster c is local row `ord[cell][pos + i]` of its cell,
/// with `cell` and `pos` the cluster's (`clcell.npy`, `clpos.npy`).
pub struct Buckets {
    pub part: Partition,
    cstart: Vec<usize>,
    clcell: Vec<usize>,
    clpos: Vec<usize>,
    ord: Vec<Option<memmap2::Mmap>>,
}

impl Buckets {
    pub fn open(partition: &str, fit: &str) -> Result<Buckets, String> {
        let part = Partition::open(partition)?;
        let r = |name: &str| -> Result<Vec<usize>, String> { Ok(try_read_npy(&format!("{fit}/{name}.npy"))?.i64s().iter().map(|&x| x as usize).collect()) };
        let (cstart, clcell, clpos) = (r("cstart")?, r("clcell")?, r("clpos")?);
        let ord: Vec<Option<memmap2::Mmap>> = (0..part.cells.len()).map(|c| map(&format!("{fit}/ord{c}.u32"))).collect::<Result<_, _>>()?;
        let refit = || format!("{fit} lists rows that {partition} does not hold; refit it");
        if cstart.len() != clcell.len() + 1 || clpos.len() != clcell.len() || cstart.first() != Some(&0) || cstart.windows(2).any(|w| w[0] > w[1]) {
            return Err(refit());
        }
        for c in 0..clcell.len() {
            let cell = clcell[c];
            let end = clpos[c].checked_add(cstart[c + 1] - cstart[c]).ok_or_else(refit)?;
            if cell >= part.cells.len() || end > words::<u32>(&ord[cell]).len() {
                return Err(refit());
            }
            let rows = part.cells[cell].rows();
            if words::<u32>(&ord[cell])[clpos[c]..end].iter().any(|&i| i as usize >= rows) {
                return Err(refit());
            }
        }
        Ok(Buckets { part, cstart, clcell, clpos, ord })
    }
    /// The number of fit rows (the fit's cstart's last entry).
    pub fn rows(&self) -> usize {
        self.cstart.last().copied().unwrap_or(0)
    }
    /// The cell and local row of fit row r.
    fn local(&self, r: usize) -> (usize, usize) {
        let c = self.cstart.partition_point(|&s| s <= r) - 1;
        let cell = self.clcell[c];
        (cell, words::<u32>(&self.ord[cell])[self.clpos[c] + r - self.cstart[c]] as usize)
    }
    /// The source row (entity) of fit row r.
    pub fn id(&self, r: usize) -> u64 {
        let (cell, i) = self.local(r);
        self.part.cells[cell].id(i)
    }
}

impl FitRows for Buckets {
    fn n(&self) -> usize {
        self.part.n
    }
    fn row(&self, r: usize) -> &[u16] {
        let (cell, i) = self.local(r);
        self.part.cells[cell].vector(self.part.n, i)
    }
}
