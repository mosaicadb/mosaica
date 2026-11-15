//! A partition's cells (`tools/partition.py`), mapped: each cell's own rows and the rows that moved into it, f16 unit
//! vectors zero-padded to n. With a fit's row order (`ord{c}.u32` and `cellstart.npy`, written by `vexact fit`), any
//! fit row is read without holding the vectors in memory.

use crate::util::read_npy;

/// Unit f16 vectors by row, in the order of a fit's `cstart` and `perm`.
pub trait Rows: Sync {
    fn n(&self) -> usize;
    fn row(&self, r: usize) -> &[u16];
}

impl Rows for crate::index::RealSource {
    fn n(&self) -> usize {
        self.n
    }
    fn row(&self, r: usize) -> &[u16] {
        crate::index::RealSource::row(self, r)
    }
}

fn map(path: &str) -> Option<memmap2::Mmap> {
    let f = std::fs::File::open(path).ok()?;
    if f.metadata().ok()?.len() == 0 {
        return None;
    }
    Some(unsafe { memmap2::Mmap::map(&f).unwrap_or_else(|e| panic!("cannot map {path}: {e}")) })
}

fn words<T>(m: &Option<memmap2::Mmap>) -> &[T] {
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

impl Partition {
    pub fn open(dir: &str) -> Partition {
        let path = format!("{dir}/manifest.json");
        let m: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}; run tools/partition.py first"))).unwrap();
        let us = |k: &str| m[k].as_u64().unwrap() as usize;
        let (n, nc) = (us("n"), us("cells"));
        let cells: Vec<Cell> = (0..nc).map(|c| {
            let f = |x: &str| map(&format!("{dir}/cell{c}.{x}"));
            let cell = Cell { vec: f("f16"), ids: f("ids"), cl: f("cl"), in_vec: f("in.f16"), in_ids: f("in.ids"), in_cl: f("in.cl"), own: 0 };
            let own = words::<u64>(&cell.ids).len();
            assert!(words::<u16>(&cell.vec).len() == own * n && words::<u32>(&cell.cl).len() == own, "{dir}/cell{c}: its vectors, ids and clusters disagree on the number of rows; rerun tools/partition.py");
            Cell { own, ..cell }
        }).collect();
        let cluster_cell: Vec<u32> = read_npy(&format!("{dir}/cluster_cell.npy")).u32s();
        Partition { n, n_orig: us("n_orig"), rows: us("N"), k: cluster_cell.len(), cells, cluster_cell }
    }
}

/// A fit's rows read from its partition: fit row r is local row `ord[c][r - cellstart[c]]` of cell c.
pub struct Buckets {
    pub part: Partition,
    cellstart: Vec<usize>,
    ord: Vec<Option<memmap2::Mmap>>,
}

impl Buckets {
    pub fn open(partition: &str, fit: &str) -> Buckets {
        let part = Partition::open(partition);
        let cellstart: Vec<usize> = read_npy(&format!("{fit}/cellstart.npy")).i64s().iter().map(|&x| x as usize).collect();
        assert_eq!(cellstart.len(), part.cells.len() + 1, "{fit} was fitted to another partition than {partition}; refit it");
        let ord: Vec<Option<memmap2::Mmap>> = (0..part.cells.len()).map(|c| map(&format!("{fit}/ord{c}.u32"))).collect();
        for c in 0..part.cells.len() {
            assert_eq!(words::<u32>(&ord[c]).len(), cellstart[c + 1] - cellstart[c], "{fit}/ord{c}.u32 does not list the fit's rows of cell {c}; refit it");
        }
        Buckets { part, cellstart, ord }
    }
}

impl Rows for Buckets {
    fn n(&self) -> usize {
        self.part.n
    }
    fn row(&self, r: usize) -> &[u16] {
        let c = self.cellstart.partition_point(|&s| s <= r) - 1;
        let i = words::<u32>(&self.ord[c])[r - self.cellstart[c]] as usize;
        self.part.cells[c].vector(self.part.n, i)
    }
}
