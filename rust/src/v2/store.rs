//! The encoded index on disk. A build writes it; opening it maps the per-row arrays (codes, scalars, keys, row maps,
//! full vectors) and reads the per-group and per-cluster data, so no item is encoded again.
//!
//! A saved index is a directory of files: `code{l}.bin` and `scal{l}.bin` per level, `ahat.bin`, `row_ent.bin`,
//! `ent_row.bin`, `nukey.bin`, `vectors.bin` (the f16 vectors in index row order, padded to whole 4,096-byte pages so
//! that O_DIRECT reads of the last rows are not short), `offsets.bin`, `esc_key.bin` and `esc_val.bin` (the escapes),
//! `clusters.bin` (the per-cluster records and centre tables), `anchors.bin`, `hi.bin` and `lo.bin` (the support
//! table), and `index.json` (sizes, the per-group calibration, constants and basis defect, the build options that
//! decide the encoding, and the key of the fit and those options it was built from).
//!
//! A saved index is `DIR/encoded` in its index directory DIR (the fit). A rebuild holds an exclusive lock on
//! `encoded.lock` beside it for its whole run, writes into a scratch directory of its own (`encoded.tmp-PID-NANOS`),
//! syncs every file and the directory, and then swaps the complete directory into place (an atomic exchange where the
//! file system has one, else the old directory is renamed aside first; it is removed afterwards). An open reads
//! every file through one descriptor of the index directory, so it sees one complete build, never files of two.

use super::*;
use std::io::Write;
use std::ops::Deref;
use std::os::unix::io::AsRawFd;

/// The version of the saved index; any other is refused.
pub const STORE_FORMAT: u64 = 6;

/// Element types that are stored as their bytes.
pub unsafe trait Plain: Copy + Send + Sync + 'static {}
unsafe impl Plain for u8 {}
unsafe impl Plain for i8 {}
unsafe impl Plain for u16 {}
unsafe impl Plain for i16 {}
unsafe impl Plain for u32 {}
unsafe impl Plain for u64 {}
unsafe impl Plain for f32 {}
unsafe impl Plain for f64 {}
unsafe impl Plain for [u16; 16] {}

fn bytes<T: Plain>(v: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}

/// The bytes of numeric values and fixed arrays of them, for hashing. Only for types without padding (primitive
/// numbers, and arrays of them).
fn pod<T: Copy>(v: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}

/// A fixed 64-bit hash over little-endian 8-byte words (each step a multiply by an odd constant and an xor-shift, both
/// invertible, so inputs that differ in one word always hash apart), the same on every Rust release and machine, unlike
/// `DefaultHasher`. Byte strings are hashed with their length. build.rs hashes the library's sources the same way.
pub struct Fx(u64);
impl Default for Fx {
    fn default() -> Fx {
        Fx(0)
    }
}
impl Fx {
    #[inline(always)]
    pub fn u64(&mut self, x: u64) {
        self.0 = (self.0 ^ x).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        self.0 ^= self.0 >> 29;
    }
    pub fn bytes(&mut self, b: &[u8]) {
        self.u64(b.len() as u64);
        let mut c = b.chunks_exact(8);
        for w in &mut c {
            self.u64(u64::from_le_bytes(w.try_into().unwrap()));
        }
        if !c.remainder().is_empty() {
            let mut t = [0u8; 8];
            t[..c.remainder().len()].copy_from_slice(c.remainder());
            self.u64(u64::from_le_bytes(t));
        }
    }
    /// Numeric values (or fixed arrays of them) as their bytes.
    pub fn pod<T: Copy>(&mut self, v: &[T]) {
        self.bytes(pod(v));
    }
    pub fn str(&mut self, s: &str) {
        self.bytes(s.as_bytes());
    }
    /// A long byte string, hashed in parallel and deterministically: its length, then the hash of each fixed 4 MiB
    /// chunk (each by `bytes` from a fresh state), in order. Short strings are hashed as `bytes` hashes them.
    pub fn big(&mut self, b: &[u8]) {
        use rayon::prelude::*;
        const CH: usize = 4 << 20;
        if b.len() <= CH {
            return self.bytes(b);
        }
        self.u64(u64::MAX);
        self.u64(b.len() as u64);
        let hs: Vec<u64> = b.par_chunks(CH).map(|c| {
            let mut h = Fx::default();
            h.bytes(c);
            h.finish()
        }).collect();
        hs.into_iter().for_each(|x| self.u64(x));
    }
    /// `big` over numeric values as their bytes.
    pub fn big_pod<T: Copy>(&mut self, v: &[T]) {
        self.big(pod(v));
    }
    pub fn usizes(&mut self, v: &[usize]) {
        self.u64(v.len() as u64);
        v.iter().for_each(|&x| self.u64(x as u64));
    }
    pub fn finish(&self) -> u64 {
        // SplitMix64's finaliser
        let mut z = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// The fingerprint of the library's sources (build.rs): part of every saved index's key, so that an index saved by
/// another version of the encoder is never opened.
pub const LIB_FINGERPRINT: &str = env!("VEXACT_LIB_FINGERPRINT");

/// A per-row array: on the heap after a build (and for a compacted index, which copies its rows), mapped from its
/// file once the index is saved and opened. It reads as a slice without a branch on where it is.
pub struct Arr<T: Plain> {
    ptr: *const T,
    len: usize,
    _keep: Keep<T>,
}
#[allow(dead_code)] // held only so that the buffer or mapping lives as long as the array
enum Keep<T> {
    Heap(Vec<T>),
    Map(memmap2::Mmap),
}
// the pointer is into the vector's heap buffer or the mapping, which the array owns and never changes
unsafe impl<T: Plain> Send for Arr<T> {}
unsafe impl<T: Plain> Sync for Arr<T> {}
impl<T: Plain> Deref for Arr<T> {
    type Target = [T];
    #[inline(always)]
    fn deref(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
}
impl<T: Plain> Default for Arr<T> {
    fn default() -> Self {
        Vec::new().into()
    }
}
impl<T: Plain> From<Vec<T>> for Arr<T> {
    fn from(v: Vec<T>) -> Self {
        Arr { ptr: v.as_ptr(), len: v.len(), _keep: Keep::Heap(v) }
    }
}
impl<T: Plain> FromIterator<T> for Arr<T> {
    fn from_iter<I: IntoIterator<Item = T>>(it: I) -> Self {
        it.into_iter().collect::<Vec<T>>().into()
    }
}

impl<T: Plain> Arr<T> {
    /// The array in `path`, mapped.
    pub fn map(path: &str) -> Result<Arr<T>, String> {
        let f = std::fs::File::open(path).map_err(|e| format!("cannot open {path}: {e}"))?;
        let len = f.metadata().map_err(|e| format!("cannot read {path}: {e}"))?.len() as usize;
        if len % std::mem::size_of::<T>() != 0 {
            return Err(format!("{path} is {len} bytes, not a whole number of elements; rebuild the index"));
        }
        if len == 0 {
            return Ok(Arr::default());
        }
        let m = unsafe { memmap2::Mmap::map(&f).map_err(|e| format!("cannot map {path}: {e}"))? };
        Ok(Arr { ptr: m.as_ptr() as *const T, len: len / std::mem::size_of::<T>(), _keep: Keep::Map(m) })
    }
}

/// Write `v` to the new file `path` and sync it.
fn write_file<T: Plain>(path: &str, v: &[T]) {
    let mut f = std::io::BufWriter::with_capacity(1 << 22, std::fs::File::create(path).unwrap_or_else(|e| panic!("cannot create {path}: {e}")));
    f.write_all(bytes(v)).unwrap_or_else(|e| panic!("cannot write {path}: {e}"));
    let f = f.into_inner().unwrap_or_else(|e| panic!("cannot write {path}: {e}"));
    f.sync_all().unwrap_or_else(|e| panic!("cannot sync {path}: {e}"));
}

fn sync_dir(dir: &str) {
    disk::sync_dir(std::path::Path::new(dir)).unwrap_or_else(|e| panic!("cannot sync the directory {dir}: {e}"));
}

/// A byte buffer of length-prefixed arrays and plain values.
#[derive(Default)]
struct Wr(Vec<u8>);
impl Wr {
    fn put<T: Plain>(&mut self, x: T) {
        self.0.extend_from_slice(bytes(std::slice::from_ref(&x)));
    }
    fn arr<T: Plain>(&mut self, v: &[T]) {
        self.put(v.len() as u64);
        self.0.extend_from_slice(bytes(v));
    }
}
const SHORT: &str = "clusters.bin ends inside a record; rebuild the index";
struct Rd<'a>(&'a [u8]);
impl Rd<'_> {
    fn get<T: Plain>(&mut self) -> Result<T, String> {
        let s = std::mem::size_of::<T>();
        if self.0.len() < s {
            return Err(SHORT.into());
        }
        let x = unsafe { std::ptr::read_unaligned(self.0.as_ptr() as *const T) };
        self.0 = &self.0[s..];
        Ok(x)
    }
    fn arr<T: Plain>(&mut self) -> Result<Vec<T>, String> {
        let len = self.get::<u64>()? as usize;
        let s = len.checked_mul(std::mem::size_of::<T>()).filter(|&s| s <= self.0.len()).ok_or(SHORT)?;
        let mut v = Vec::<T>::with_capacity(len);
        unsafe {
            std::ptr::copy_nonoverlapping(self.0.as_ptr(), v.as_mut_ptr() as *mut u8, s);
            v.set_len(len);
        }
        self.0 = &self.0[s..];
        Ok(v)
    }
}

fn put_cluster(w: &mut Wr, c: &Cl2, ctab: &[kern::CTab], ndl: usize) {
    for x in [c.g, c.r0, c.r1, c.nblk] {
        w.put(x as u64);
    }
    for x in [c.mn, c.btot, c.mvn, c.rmax, c.numax, c.bmin, c.bmax] {
        w.put(x);
    }
    w.put(c.mn32);
    w.arr(&c.bm[..ndl]);
    w.arr(&c.b2);
    w.arr(&c.l1m);
    w.arr(&c.mhat);
    w.arr(&c.mv32);
    w.arr(&c.wmr);
    w.put(ctab.len() as u64);
    for t in ctab {
        w.arr(&t.th);
        w.arr(&t.tmax);
        w.arr(&t.step);
        w.arr(&t.off);
        w.arr(&t.err);
    }
}

fn get_cluster(r: &mut Rd, ndl: usize) -> Result<(Cl2, Vec<kern::CTab>), String> {
    let mut u = [0usize; 4];
    for x in u.iter_mut() {
        *x = r.get::<u64>()? as usize;
    }
    let [g, r0, r1, nblk] = u;
    let mut f = [0f64; 7];
    for x in f.iter_mut() {
        *x = r.get::<f64>()?;
    }
    let [mn, btot, mvn, rmax, numax, bmin, bmax] = f;
    let mn32 = r.get::<f32>()?;
    let bmv = r.arr::<f64>()?;
    if bmv.len() != ndl {
        return Err("clusters.bin: a record of another level count; rebuild the index".into());
    }
    let mut bm = [0f64; NDL_MAX];
    bm[..ndl].copy_from_slice(&bmv);
    let b2: [f64; J] = r.arr::<f64>()?.try_into().map_err(|_| "clusters.bin: a record of another block count; rebuild the index")?;
    let l1m: [f32; J + 1] = r.arr::<f32>()?.try_into().map_err(|_| "clusters.bin: a record of another block count; rebuild the index")?;
    let (mhat, mv32, wmr) = (r.arr()?, r.arr()?, r.arr()?);
    let nt = r.get::<u64>()? as usize;
    let ctab = (0..nt).map(|_| Ok(kern::CTab { th: r.arr()?, tmax: r.arr()?, step: r.arr()?, off: r.arr()?, err: r.arr()? })).collect::<Result<_, String>>()?;
    Ok((Cl2 { g, r0, r1, nblk, mn, mn32, mhat: Arc::new(mhat), mv32: Arc::new(mv32), wmr: Arc::new(wmr), bm, b2, l1m, btot, mvn, rmax, numax, bmin, bmax }, ctab))
}

fn cal_json(c: &CalIn, ndl: usize) -> serde_json::Value {
    let fin = c.y0.iter().chain(&c.ys).chain(c.k0.iter().flatten()).chain(c.ks.iter().flatten()).all(|x| x.is_finite()) && c.tq.iter().flatten().flatten().all(|x| x.is_finite());
    assert!(fin, "a calibration value is not finite and cannot be saved in JSON: {c:?}");
    serde_json::json!({
        "kmin": c.kmin[..ndl], "y0": c.y0, "ys": c.ys,
        "tq": c.tq.iter().map(|a| a.iter().map(|b| b.to_vec()).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "k0": c.k0, "ks": c.ks, "kb": c.kb,
    })
}

fn cal_from(v: &serde_json::Value, ndl: usize) -> Result<CalIn, String> {
    const BAD: &str = "index.json: a calibration value is missing or not a number; rebuild the index";
    let f = |x: &serde_json::Value| x.as_f64().ok_or(BAD);
    let arr_f = |x: &serde_json::Value| -> Result<[f64; J], &str> {
        let mut a = [0f64; J];
        for (j, y) in a.iter_mut().enumerate() {
            *y = f(&x[j])?;
        }
        Ok(a)
    };
    let lr = v["k0"].as_array().map_or(0, |a| a.len());
    if v["kmin"].as_array().map(|a| a.len()) != Some(ndl) {
        return Err(BAD.into());
    }
    let mut kmin = [0i32; NDL_MAX];
    for (l, y) in kmin.iter_mut().enumerate().take(ndl) {
        *y = v["kmin"][l].as_i64().and_then(|x| i32::try_from(x).ok()).ok_or(BAD)?;
    }
    let mut c = CalIn { kmin, y0: arr_f(&v["y0"])?, ys: arr_f(&v["ys"])?, tq: Vec::new(), k0: Vec::new(), ks: Vec::new(), kb: Vec::new() };
    for r in 0..lr {
        let mut tq = [[0f32; 15]; J];
        let mut kb = [0u8; J];
        for j in 0..J {
            for (cc, y) in tq[j].iter_mut().enumerate() {
                *y = f(&v["tq"][r][j][cc])? as f32;
            }
            kb[j] = v["kb"][r][j].as_u64().ok_or(BAD)? as u8;
        }
        c.tq.push(tq);
        c.kb.push(kb);
        c.k0.push(arr_f(&v["k0"][r])?);
        c.ks.push(arr_f(&v["ks"][r])?);
    }
    Ok(c)
}

/// The identity of the fit's vectors file, not its contents (at a billion items it is terabytes): length,
/// modification and status-change times (seconds and nanoseconds), device and inode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XsId {
    pub len: u64,
    pub mtime: (i64, i64),
    pub ctime: (i64, i64),
    pub dev: u64,
    pub ino: u64,
}
impl XsId {
    pub fn of(path: &str) -> Result<XsId, String> {
        use std::os::unix::fs::MetadataExt;
        let md = std::fs::metadata(path).map_err(|e| format!("cannot read {path}: {e}"))?;
        Ok(XsId { len: md.len(), mtime: (md.mtime(), md.mtime_nsec()), ctime: (md.ctime(), md.ctime_nsec()), dev: md.dev(), ino: md.ino() })
    }
    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({ "len": self.len, "mtime": [self.mtime.0, self.mtime.1], "ctime": [self.ctime.0, self.ctime.1], "dev": self.dev, "ino": self.ino })
    }
    pub fn from_json(v: &serde_json::Value) -> Option<XsId> {
        let pair = |x: &serde_json::Value| Some((x[0].as_i64()?, x[1].as_i64()?));
        Some(XsId { len: v["len"].as_u64()?, mtime: pair(&v["mtime"])?, ctime: pair(&v["ctime"])?, dev: v["dev"].as_u64()?, ino: v["ino"].as_u64()? })
    }
}

/// What a sealed index records besides its arrays: the key, the hashes of what it holds and of what an open derives
/// again, and the identity of the vectors file it was built from.
pub struct Seal<'a> {
    pub key: &'a str,
    /// The build options that decide the encoding (`BuildOptions::encoding_json`), from which an open derives the key.
    pub options: &'a serde_json::Value,
    pub store_hash: u64,
    pub plan_hash: u64,
    pub xs: Option<XsId>,
    /// The fit's perm.npy as the build read it: the hash of its entities (`perm_hash`) and its file's identity.
    pub perm: Option<(u64, XsId)>,
}

/// A fixed hash of a fit's perm.npy entities (fit row -> entity), which the build records; parallel in fixed chunks.
pub fn perm_hash(perm: &[i64]) -> u64 {
    let mut h = Fx::default();
    h.str("vexact perm");
    h.big_pod(perm);
    h.finish()
}

/// `index.json`, written to a new file and renamed into place (it is written twice: without a key while the build
/// runs, then with its key and hashes when the index is complete).
fn index_json(ix: &Index2, dir: &str, seal: Option<&Seal>) {
    let groups: Vec<serde_json::Value> = ix.groups.iter().map(|g| serde_json::json!({
        "e_inv": g.e_inv, "csgl": g.csgl, "cal": g.cal.as_ref().map(|c| cal_json(c, ix.meta.ndl())),
    })).collect();
    let meta = serde_json::json!({
        "format": STORE_FORMAT, "index_format": INDEX_FORMAT, "key": seal.map(|s| s.key), "lib": LIB_FINGERPRINT, "n": ix.n(), "k": ix.k(), "nrows": ix.nrows,
        "npow": ix.meta.npow, "dlv": ix.meta.dlv, "rlv": ix.meta.rlv, "x2": ix.x2, "nanch": ix.nanch, "anc_centres": ix.anc_centres, "nent": ix.nent, "code_hash": format!("{:016x}", ix.code_hash()),
        "store_hash": seal.map(|s| format!("{:016x}", s.store_hash)), "plan_hash": seal.map(|s| format!("{:016x}", s.plan_hash)),
        "xs": seal.and_then(|s| s.xs).map(|x| x.json()),
        "options": seal.map(|s| s.options), "perm": seal.and_then(|s| s.perm).map(|(h, id)| serde_json::json!({ "hash": format!("{h:016x}"), "file": id.json() })),
        "groups": groups,
    });
    let (tmp, path) = (format!("{dir}/index.json.tmp"), format!("{dir}/index.json"));
    write_file(&tmp, serde_json::to_string_pretty(&meta).unwrap().as_bytes());
    std::fs::rename(&tmp, &path).unwrap_or_else(|e| panic!("cannot move {tmp} to {path}: {e}"));
}

/// The file `dir/name` created at `bytes` bytes and mapped for writing (no map when empty, which cannot be mapped).
fn create_mapped(dir: &str, name: &str, bytes: usize) -> (String, std::fs::File, Option<memmap2::MmapMut>) {
    let path = format!("{dir}/{name}");
    let f = std::fs::OpenOptions::new().read(true).write(true).create_new(true).open(&path).unwrap_or_else(|e| panic!("cannot create {path}: {e}"));
    f.set_len(bytes as u64).unwrap_or_else(|e| panic!("cannot size {path}: {e}"));
    let map = (bytes > 0).then(|| unsafe { memmap2::MmapMut::map_mut(&f).unwrap_or_else(|e| panic!("cannot map {path}: {e}")) });
    (path, f, map)
}

/// A file being written through a writable map: flushed and synced when finished, then read through the same map.
struct OnFile {
    path: String,
    file: std::fs::File,
    map: Option<memmap2::MmapMut>,
}
impl OnFile {
    fn sync(&self) {
        if let Some(m) = &self.map {
            m.flush().unwrap_or_else(|e| panic!("cannot write {}: {e}", self.path));
        }
        self.file.sync_all().unwrap_or_else(|e| panic!("cannot sync {}: {e}", self.path));
    }
}

/// A per-row array the build writes: on the heap, or one of a saved index's files created at full size and written
/// through a writable map. Clusters write disjoint ranges of it from several threads at once (`put`, unsafe: the
/// caller keeps concurrent writes apart); reading it (`get`) needs exclusive access, so no write can be in flight.
pub struct Out<T: Plain> {
    heap: Vec<T>,
    on: Option<OnFile>,
    ptr: *mut T,
    len: usize,
}
// shared between threads only to `put`, whose contract keeps concurrent writes to disjoint ranges
unsafe impl<T: Plain> Send for Out<T> {}
unsafe impl<T: Plain> Sync for Out<T> {}

impl<T: Plain> Out<T> {
    /// `len` zeros, on the heap without `dir`, else in the new file `dir/name`.
    pub fn new(dir: Option<&str>, name: &str, len: usize) -> Out<T> {
        let Some(dir) = dir else {
            let mut heap = vec![unsafe { std::mem::zeroed::<T>() }; len];
            let ptr = heap.as_mut_ptr();
            return Out { heap, on: None, ptr, len };
        };
        let (path, file, mut map) = create_mapped(dir, name, len * std::mem::size_of::<T>());
        let ptr = map.as_mut().map_or(std::ptr::NonNull::dangling().as_ptr(), |m| m.as_mut_ptr() as *mut T);
        Out { heap: Vec::new(), on: Some(OnFile { path, file, map }), ptr, len }
    }
    /// Write `v` at `off` (checked against the array's length).
    ///
    /// # Safety
    /// Writes that may run at the same time, from any threads, must be to disjoint ranges. (Reading needs `&mut self`,
    /// so no read overlaps a write.)
    pub unsafe fn put(&self, off: usize, v: &[T]) {
        assert!(off.checked_add(v.len()).is_some_and(|e| e <= self.len), "a write of {} at {off} runs past an array of {}", v.len(), self.len);
        std::ptr::copy_nonoverlapping(v.as_ptr(), self.ptr.add(off), v.len());
    }
    /// Write `v` at `off`, with no other write possible meanwhile.
    pub fn put_mut(&mut self, off: usize, v: &[T]) {
        // exclusive access: no other write is in flight
        unsafe { self.put(off, v) }
    }
    /// What has been written so far.
    pub fn get(&mut self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
    /// The finished array: the heap vector, or the file flushed, synced and read through the map it was written by.
    pub fn finish(self) -> Arr<T> {
        match self.on {
            Some(on) => {
                on.sync();
                let OnFile { path, map, .. } = on;
                match map {
                    None => Arr::default(),
                    Some(m) => {
                        let m = m.make_read_only().unwrap_or_else(|e| panic!("cannot map {path} for reading: {e}"));
                        Arr { ptr: m.as_ptr() as *const T, len: self.len, _keep: Keep::Map(m) }
                    }
                }
            }
            None => self.heap.into(),
        }
    }
}

/// The f16 vectors the build writes, in index row order: on the heap starting on a cache line (as `F16Rows`), or the
/// saved index's `vectors.bin`, padded to whole pages and written through a writable map. Written as `Out` is.
pub struct OutRows {
    mem: F16Rows,
    on: Option<OnFile>,
    ptr: *mut u16,
    len: usize,
}
// shared between threads only to `put`, whose contract keeps concurrent writes to disjoint ranges
unsafe impl Send for OutRows {}
unsafe impl Sync for OutRows {}

impl OutRows {
    /// `len` zero values, on the heap without `dir`, else in the new file `dir/name`.
    pub fn new(dir: Option<&str>, name: &str, len: usize) -> OutRows {
        let Some(dir) = dir else {
            let mut mem = F16Rows::zeroed(len);
            let ptr = mem.as_mut_ptr();
            return OutRows { mem, on: None, ptr, len };
        };
        let (path, file, mut map) = create_mapped(dir, name, (len * 2 + 4095) & !4095);
        let ptr = map.as_mut().map_or(std::ptr::NonNull::dangling().as_ptr(), |m| m.as_mut_ptr() as *mut u16);
        OutRows { mem: F16Rows::default(), on: Some(OnFile { path, file, map }), ptr, len }
    }
    /// Write `v` at `off` (checked against the array's length).
    ///
    /// # Safety
    /// As `Out::put`: writes that may run at the same time must be to disjoint ranges.
    pub unsafe fn put(&self, off: usize, v: &[u16]) {
        assert!(off.checked_add(v.len()).is_some_and(|e| e <= self.len), "a write of {} at {off} runs past an array of {}", v.len(), self.len);
        std::ptr::copy_nonoverlapping(v.as_ptr(), self.ptr.add(off), v.len());
    }
    /// The finished rows: the heap copy, or the file flushed, synced and opened through the descriptor it was
    /// written by (as an opened index opens it).
    pub fn finish(self) -> disk::Rows {
        match self.on {
            Some(on) => {
                on.sync();
                let OnFile { path, file, map } = on;
                drop(map);
                disk::Rows::from_file(file, &path, self.len, "normal").unwrap_or_else(|e| panic!("{e}"))
            }
            None => disk::Rows::Mem(self.mem),
        }
    }
}

/// Write the parts of `ix` the build does not write row by row: offsets, escapes, per-cluster records and
/// `index.json` without a key, so that `dir` is not taken for a finished index.
pub fn save_rest(ix: &Index2, dir: &str) {
    let f = |name: &str| format!("{dir}/{name}");
    let offs: Vec<u64> = ix.coff.iter().chain(&ix.soff).flatten().map(|&o| o as u64).collect();
    write_file(&f("offsets.bin"), &offs);
    let mut esc: Vec<(u64, f32)> = ix.esc.iter().map(|(&(l, s, r), &v)| (((l as u64) << 40) | ((s as u64) << 32) | r as u64, v)).collect();
    esc.sort_unstable_by_key(|e| e.0);
    write_file(&f("esc_key.bin"), &esc.iter().map(|e| e.0).collect::<Vec<_>>());
    write_file(&f("esc_val.bin"), &esc.iter().map(|e| e.1).collect::<Vec<_>>());
    let mut w = Wr::default();
    for (c, t) in ix.cl.iter().zip(ix.ctab.iter()) {
        put_cluster(&mut w, c, t, ix.meta.ndl());
    }
    write_file(&f("clusters.bin"), &w.0);
    index_json(ix, dir, None);
}

/// Finish the index built into `dir`: write its support table's anchors, give `index.json` the key it answers to, the index's
/// `store_hash` and `plan_hash` and the identity `xs` of the vectors file it was built from (written last, so a
/// directory whose `index.json` has a key is complete), and sync the directory. Returns the store hash.
pub fn seal(ix: &Index2, dir: &str, key: &str, options: &serde_json::Value, xs: Option<XsId>, perm: Option<(u64, XsId)>) -> u64 {
    let f = |name: &str| format!("{dir}/{name}");
    // (the build wrote the table's bounds, hi.bin and lo.bin, while encoding)
    write_file(&f("anchors.bin"), &ix.anchors);
    let h = ix.store_hash();
    index_json(ix, dir, Some(&Seal { key, options, store_hash: h, plan_hash: ix.plan_hash(), xs, perm }));
    sync_dir(dir);
    h
}

fn nanos() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos())
}

/// The directory that holds the index directory `edir`.
pub fn parent_of(edir: &str) -> String {
    match std::path::Path::new(edir).parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_string_lossy().to_string(),
        _ => ".".to_string(),
    }
}

/// A lock on a file beside the saved indexes, released when dropped. Two are used:
/// - `encoded.lock`, exclusive, held by a rebuild for its whole run: a second rebuild there waits for the first (and
///   says so);
/// - `encoded.install.lock`, held exclusively by `install` only while it swaps an index into place and removes the old
///   one, and shared by `open` only while it opens and maps its files: an open never sees a directory being replaced
///   or removed, and never waits behind an encode.
pub struct Lock(#[allow(dead_code)] std::fs::File);
impl Lock {
    /// The lock file `path`: opened read-only when it exists (flock needs only read access, so a lock file another
    /// user created works), else created. None when it is missing and cannot be created.
    fn file(path: &str) -> std::io::Result<Option<std::fs::File>> {
        match std::fs::File::open(path) {
            Ok(f) => Ok(Some(f)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => match std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(path) {
                Ok(f) => Ok(Some(f)),
                Err(e) if matches!(e.raw_os_error(), Some(libc::EROFS) | Some(libc::EACCES) | Some(libc::EPERM)) => Ok(None),
                Err(e) => Err(e),
            },
            Err(e) => Err(e),
        }
    }
    fn flock(f: &std::fs::File, op: i32) -> std::io::Result<()> {
        loop {
            if unsafe { libc::flock(f.as_raw_fd(), op) } == 0 {
                return Ok(());
            }
            let e = std::io::Error::last_os_error();
            if e.raw_os_error() != Some(libc::EINTR) {
                return Err(e);
            }
        }
    }
    /// The rebuild lock of `parent`. Returns, with the lock, whether this rebuild had to wait for another one.
    pub fn take(parent: &str, log: &mut dyn FnMut(String)) -> (Lock, bool) {
        let path = format!("{parent}/encoded.lock");
        let f = Lock::file(&path).unwrap_or_else(|e| panic!("cannot open the lock file {path}: {e}")).unwrap_or_else(|| panic!("cannot create the lock file {path}; a rebuild needs to write in {parent}"));
        let mut waited = false;
        if let Err(e) = Lock::flock(&f, libc::LOCK_EX | libc::LOCK_NB) {
            if e.raw_os_error() != Some(libc::EWOULDBLOCK) {
                panic!("cannot lock {path}: {e}");
            }
            waited = true;
            log(format!("waiting for another rebuild in {parent} (it holds {path})"));
            let t = Timer::new();
            Lock::flock(&f, libc::LOCK_EX).unwrap_or_else(|e| panic!("cannot lock {path}: {e}"));
            log(format!("the other rebuild finished; waited {:.1}s", t.s()));
        }
        (Lock(f), waited)
    }
    /// The install lock of `parent`, exclusive (to install) or shared (to open). An existing lock file is opened
    /// read-only, whoever made it. An open where the lock file is missing and cannot be created (a read-only
    /// directory) goes ahead without it: nothing can install there either.
    pub fn install(parent: &str, exclusive: bool) -> Result<Option<Lock>, String> {
        let path = format!("{parent}/encoded.install.lock");
        let f = match Lock::file(&path) {
            Ok(Some(f)) => f,
            Ok(None) if !exclusive => return Ok(None),
            Ok(None) => return Err(format!("cannot create the lock file {path}")),
            Err(e) => return Err(format!("cannot open the lock file {path}: {e}")),
        };
        Lock::flock(&f, if exclusive { libc::LOCK_EX } else { libc::LOCK_SH }).map_err(|e| format!("cannot lock {path}: {e}"))?;
        Ok(Some(Lock(f)))
    }
}

/// A new scratch directory beside `edir`, named for this process and moment, for a rebuild to write into.
pub fn scratch_dir(edir: &str) -> String {
    let tmp = format!("{edir}.tmp-{}-{}", std::process::id(), nanos());
    std::fs::create_dir(&tmp).unwrap_or_else(|e| panic!("cannot create {tmp}: {e}"));
    tmp
}

/// Put the complete, synced index directory `tmp` in place as `edir`, replacing any index there, under the install
/// lock (so no open is reading meanwhile): an atomic exchange of the two directories where the file system has one,
/// else the old one is renamed aside first. The old directory is removed afterwards; if that fails (a busy NFS file,
/// say) it is left with a warning, and `log_stale` reports it later. Indexes already open keep the files they mapped.
pub fn install(tmp: &str, edir: &str, log: &mut dyn FnMut(String)) {
    let parent = parent_of(edir);
    let _lock = Lock::install(&parent, true).unwrap_or_else(|e| panic!("{e}"));
    let c = |s: &str| std::ffi::CString::new(s).unwrap();
    let old = if std::path::Path::new(edir).exists() {
        let r = unsafe { libc::renameat2(libc::AT_FDCWD, c(tmp).as_ptr(), libc::AT_FDCWD, c(edir).as_ptr(), libc::RENAME_EXCHANGE) };
        if r == 0 {
            // tmp now names the old index
            Some(tmp.to_string())
        } else {
            let aside = format!("{edir}.old-{}-{}", std::process::id(), nanos());
            std::fs::rename(edir, &aside).unwrap_or_else(|e| panic!("cannot move {edir} aside to {aside}: {e}"));
            std::fs::rename(tmp, edir).unwrap_or_else(|e| panic!("cannot move {tmp} to {edir}: {e}"));
            Some(aside)
        }
    } else {
        std::fs::rename(tmp, edir).unwrap_or_else(|e| panic!("cannot move {tmp} to {edir}: {e}"));
        None
    };
    sync_dir(&parent);
    if let Some(o) = old {
        if let Err(e) = std::fs::remove_dir_all(&o) {
            log(format!("WARNING: cannot remove the replaced index {o} ({e}); it is left in place, remove it by hand"));
        }
    }
}

/// Log every other index directory in `parent` (`encoded-KEY` ones saved by earlier versions, stale now, and rebuilds'
/// scratch directories) with its size. Nothing is removed.
pub fn log_stale(parent: &str, keep: &str, log: &mut dyn FnMut(String)) {
    let keep = std::path::Path::new(keep).file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
    let Ok(rd) = std::fs::read_dir(parent) else { return };
    let mut found: Vec<(String, u64)> = rd.filter_map(|e| e.ok()).filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("encoded") && *n != keep)
        .map(|n| {
            let size = std::fs::read_dir(format!("{parent}/{n}")).map_or(0, |d| d.filter_map(|e| e.ok()).filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum());
            (n, size)
        }).collect();
    found.sort();
    for (n, size) in found {
        let what = if n.contains(".tmp-") || n.contains(".old-") { "a rebuild's scratch directory (one running now, or left by an interrupted one)" } else { "saved for another fit, other options or another version; stale for this one" };
        log(format!("{parent}/{n} ({:.1} MB) is {what}; remove it if nothing uses it", size as f64 / 1e6));
    }
}

/// Open the index saved in `dir` for the fit `p`, profile `prof` and options `o` (with `design_c` derived from them),
/// which must be the ones it was built from: its `index.json` must hold `key` (`build_key`). Nothing is encoded: the
/// per-row arrays and the vectors are mapped, the plans are derived from the fit as the build derives them (and must
/// hash as they did at the build, `plan_hash`), and the calibration, constants, basis defects, per-cluster records and
/// support table are read. Every file is read through one descriptor of `dir`, under the shared install lock, so all
/// come from one complete build. Every check is over clusters, levels and tables, never over rows (`check_maps` checks
/// the row maps, with `--hash` and `verify`). A missing, stale or damaged index is an error that says to rebuild.
///
/// The fit's vectors file and its perm.npy are not read (the index holds its own vectors and row-to-entity map); their
/// identities are compared with the ones the index was built from (`XsId`, `check_file`): another length or
/// modification time is an error unless `oo.keep_index`, which turns it into a warning; another status-change time,
/// device or inode only (chmod, a copy with its times, a restore, a hard link) is a warning; a missing file is a
/// warning. A partitioned fit's partition is compared by content (`check_partition`, O(cells)). Nothing in an open is
/// O(rows) except the map of escapes (a few per hundred rows). The repack thresholds are left at their default.
pub fn open(p: &Params, prof: &[f64], o: &BuildOptions, oo: &OpenOptions, design_c: Option<&[f64]>, key: &str, dir: &str, log: &mut dyn FnMut(String)) -> Result<Index2, String> {
    let t = Timer::new();
    let missing = |e: String| format!("no index is saved in {dir} ({e}); rebuild it (v2::rebuild; vexact --v2 --index FITDIR --rebuild)");
    let damaged = |what: &str| format!("{dir}: {what}; rebuild the index");
    let nl = p.meta.dlv.len() + p.meta.rlv.len();
    // every file opened and mapped through one descriptor of `dir`, under the shared install lock (held only for this:
    // the checks and the plans below need no lock, so an open never holds up an install for long)
    let lock = Lock::install(&parent_of(dir), false)?;
    let dfd = std::fs::File::open(dir).map_err(|e| missing(e.to_string()))?;
    let base = format!("/proc/self/fd/{}", dfd.as_raw_fd());
    let f = |name: &str| format!("{base}/{name}");
    let ours = |e: String| e.replace(&base, dir);
    let text = std::fs::read_to_string(f("index.json")).map_err(|e| missing(e.to_string()))?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{dir}/index.json: {e}; rebuild the index"))?;
    if v["format"].as_u64() != Some(STORE_FORMAT) {
        return Err(format!("{dir} holds an index of format {}, this build reads format {STORE_FORMAT}; rebuild it", v["format"]));
    }
    match v["key"].as_str() {
        Some(k) if k == key => {}
        Some(k) => return Err(format!("the index in {dir} is stale: it was built from another fit or by another version of the library (key {k}; the fit and the options it records now give {key}); rebuild it (v2::rebuild; vexact --v2 --index FITDIR --rebuild)")),
        None => return Err(format!("{dir} holds an unfinished index (no key); rebuild it")),
    }
    let us = |k: &str| v[k].as_u64().map(|x| x as usize).ok_or(format!("{dir}/index.json has no {k}; rebuild the index"));
    let (n, k, nrows, nanch, nent) = (us("n")?, us("k")?, us("nrows")?, us("nanch")?, us("nent")?);
    let cbuf = std::fs::read(f("clusters.bin")).map_err(|e| format!("cannot read {dir}/clusters.bin: {e}; rebuild the index"))?;
    let offs: Arr<u64> = Arr::map(&f("offsets.bin")).map_err(ours)?;
    let (ek, ev): (Arr<u64>, Arr<f32>) = (Arr::map(&f("esc_key.bin")).map_err(ours)?, Arr::map(&f("esc_val.bin")).map_err(ours)?);
    let anchors: Vec<f64> = Arr::<f64>::map(&f("anchors.bin")).map_err(ours)?.to_vec();
    // the support table mapped, not read: K x A bounds, of which a query touches its anchor's row
    let (hi, lo): (Arr<f32>, Arr<f32>) = (Arr::map(&f("hi.bin")).map_err(ours)?, Arr::map(&f("lo.bin")).map_err(ours)?);
    let lcode: Vec<Arr<u8>> = (0..nl).map(|l| Arr::map(&f(&format!("code{l}.bin")))).collect::<Result<_, _>>().map_err(ours)?;
    let lscal: Vec<Arr<u8>> = (0..nl).map(|l| Arr::map(&f(&format!("scal{l}.bin")))).collect::<Result<_, _>>().map_err(ours)?;
    let (ahat, row_ent, ent_row, nukey): (Arr<i16>, Arr<u32>, Arr<u32>, Arr<u16>) = (Arr::map(&f("ahat.bin")).map_err(ours)?, Arr::map(&f("row_ent.bin")).map_err(ours)?, Arr::map(&f("ent_row.bin")).map_err(ours)?, Arr::map(&f("nukey.bin")).map_err(ours)?);
    let rows = disk::Rows::open(&f("vectors.bin"), nrows.checked_mul(n).ok_or(damaged("nrows x n overflows"))?, "normal").map_err(|e| format!("{}; rebuild the index", ours(e)))?;
    drop(dfd);
    drop(lock);
    // the fit's vectors (its Xs.npy, or its partition) and its perm.npy, as they were when the index was built: by
    // identity and the partition's manifest, never by reading them
    let xs_was = XsId::from_json(&v["xs"]);
    if p.xs_path.is_empty() {
        check_partition(p, xs_was, oo.keep_index, log)?;
    } else {
        check_file("the fit's vectors file", &p.xs_path, xs_was, "the index holds its own vectors", oo.keep_index, dir, log)?;
    }
    check_file("the fit's perm.npy", &p.perm_path, XsId::from_json(&v["perm"]["file"]), "the index holds its own row-to-entity map", oo.keep_index, dir, log)?;
    if n != p.meta.n || k != p.meta.k {
        return Err(format!("{dir} was built with {k} clusters of {n} dimensions, the fit has {} of {}; rebuild it", p.meta.k, p.meta.n));
    }
    let Plans { meta, rots, codes, et, mut groups, .. } = plans(p, prof, nrows, o, design_c, false, log);
    if Some(meta.npow as u64) != v["npow"].as_u64() {
        return Err(format!("{dir} was built for a guarantee of 2^{} rows, these options give 2^{}; rebuild it", v["npow"], meta.npow));
    }
    // the deterministic levels (in the key too; checked against what this open derived). Which of them decide
    // (`ddec`) is this open's choice, not the build's
    let dlv_was: Option<Vec<usize>> = v["dlv"].as_array().and_then(|a| a.iter().map(|x| x.as_u64().map(|y| y as usize)).collect());
    if dlv_was.as_ref() != Some(&meta.dlv) {
        return Err(format!("{dir} was built with deterministic levels {}, these options give {:?}; rebuild it", v["dlv"], meta.dlv));
    }
    let gv = v["groups"].as_array().filter(|a| a.len() == groups.len()).ok_or(format!("{dir}/index.json does not hold {} groups; rebuild the index", groups.len()))?;
    for (gp, gv) in groups.iter_mut().zip(gv) {
        gp.e_inv = gv["e_inv"].as_f64().ok_or(format!("{dir}/index.json: a group has no e_inv; rebuild the index"))?;
        let csgl: Vec<f32> = gv["csgl"].as_array().map_or(Vec::new(), |a| a.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect());
        if csgl.len() != gp.csgl.len() {
            return Err(format!("{dir}/index.json: a group has {} randomised constants, the plan {}; rebuild the index", csgl.len(), gp.csgl.len()));
        }
        gp.csgl = csgl;
        if !gv["cal"].is_null() {
            let cal = cal_from(&gv["cal"], meta.ndl()).map_err(|e| format!("{dir}/{e}"))?;
            if cal.tq.len() != gp.rates.len() {
                return Err(format!("{dir}/index.json: a group's calibration has {} randomised levels, the plan {}; rebuild the index", cal.tq.len(), gp.rates.len()));
            }
            gp.set_calibration(cal);
        }
    }
    if meta.nl() != nl || groups.iter().any(|g| g.levels.len() != nl) {
        return Err(damaged("a group's plan has another level count"));
    }
    let mut r = Rd(&cbuf);
    let (cl, ctab): (Vec<Cl2>, Vec<Vec<kern::CTab>>) = (0..k).map(|_| get_cluster(&mut r, meta.ndl())).collect::<Result<Vec<_>, String>>().map_err(|e| format!("{dir}/{e}"))?.into_iter().unzip();
    if !r.0.is_empty() {
        return Err(format!("{dir}/clusters.bin has data past its {k} clusters; rebuild the index"));
    }
    // every length a query reads unchecked: per cluster, its rows, its centre's vectors and each level's centre table
    for (c, (x, t)) in cl.iter().zip(&ctab).enumerate() {
        let r0 = if c == 0 { 0 } else { cl[c - 1].r1 };
        if x.g >= groups.len() || x.r0 != r0 || x.r1 < x.r0 || x.nblk != (x.r1 - x.r0).div_ceil(BLK) || t.len() != nl || x.mhat.len() != n || x.mv32.len() != n || x.wmr.len() != n {
            return Err(damaged(&format!("cluster {c}'s record does not fit the index")));
        }
        for (l, (ct, lp)) in t.iter().zip(&groups[x.g].levels).enumerate() {
            // the table holds units / 4 * 4 per part (centre_tab) and queries read all units: equal as every part
            // has a multiple of 4 units (Part::pack)
            if lp.parts.iter().any(|pt| pt.units() % 4 != 0) {
                return Err(damaged(&format!("a part of level {l} has a unit count that is not a multiple of 4")));
            }
            let units: usize = lp.parts.iter().map(|pt| pt.units()).sum();
            let np = lp.parts.len();
            if ct.th.len() != units || ct.tmax.len() != units || ct.step.len() != np || ct.off.len() != np || ct.err.len() != np {
                return Err(damaged(&format!("cluster {c}'s centre table at level {l} does not fit the plan")));
            }
        }
    }
    if offs.len() != 2 * nl * k {
        return Err(format!("{dir}/offsets.bin does not hold {nl} levels of {k} clusters; rebuild the index"));
    }
    let off = |i: usize| -> Vec<Vec<usize>> { (0..nl).map(|l| offs[(i * nl + l) * k..(i * nl + l + 1) * k].iter().map(|&o| o as usize).collect()).collect() };
    if ek.len() != ev.len() {
        return Err(format!("{dir}: esc_key.bin and esc_val.bin differ in length; rebuild the index"));
    }
    // O(escapes), not O(clusters): the in-memory escape map queries look up (a few per hundred rows)
    let esc = ek.iter().zip(ev.iter()).map(|(&k, &v)| (((k >> 40) as u8, (k >> 32) as u8, k as u32), v)).collect();
    if nanch.checked_mul(n) != Some(anchors.len()) || nanch.checked_mul(k) != Some(hi.len()) || nanch.checked_mul(k) != Some(lo.len()) {
        return Err(format!("{dir}: the support table does not hold {nanch} anchors of {n} and their bounds over {k} clusters; rebuild the index"));
    }
    // whether anchor a is cluster a's centre direction: what the build did without the fit's anchors.npy (and not
    // inferred from the count, which other anchors can match by coincidence)
    let anc_centres = v["anc_centres"].as_bool().ok_or(format!("{dir}/index.json does not say whether its anchors are the centres; rebuild the index"))?;
    if anc_centres != (!o.nusort && p.anchors.is_empty()) || (anc_centres && nanch != k) {
        return Err(format!("{dir} was built {} the fit's anchors.npy, the fit now has {}; rebuild it", if anc_centres { "without" } else { "with" }, if p.anchors.is_empty() { "none" } else { "one" }));
    }
    let (coff, soff) = (off(0), off(1));
    for l in 0..nl {
        // each cluster's blocks lie inside its level's arrays, which end in 128 bytes of slack
        for (c, x) in cl.iter().enumerate() {
            let lp = &groups[x.g].levels[l];
            let end = |o: usize, rows: usize| x.nblk.checked_mul(rows).and_then(|b| b.checked_mul(16)).and_then(|b| b.checked_add(o)).and_then(|b| b.checked_add(128));
            let fits = |o: usize, rows: usize, len: usize| end(o, rows).is_some_and(|e| e <= len);
            if !fits(coff[l][c], lp.units, lcode[l].len()) || !fits(soff[l][c], lp.srows, lscal[l].len()) {
                return Err(damaged(&format!("level {l}'s codes or scalars are shorter than its offsets")));
            }
        }
    }
    if ahat.len() != nrows + 32 || row_ent.len() != nrows || ent_row.len() != nent || !(nukey.is_empty() || nukey.len() == nrows) || cl.last().map_or(0, |c| c.r1) != nrows {
        return Err(damaged(&format!("the row arrays do not hold {nrows} rows")));
    }
    if nukey.is_empty() == o.nusort {
        return Err(format!("{dir} was built {} the norm sort, the options ask {}; rebuild it", if o.nusort { "without" } else { "with" }, if o.nusort { "for it" } else { "for none" }));
    }
    let ix = Index2 {
        nukey,
        repack: vec![0.6; nl],
        ctab: Arc::new(ctab),
        meta,
        groups: Arc::new(groups),
        codes: Arc::new(codes),
        etab: Arc::new(et),
        cl,
        centres64: Arc::new(p.centres[..k * n].iter().map(|&x| x as f64).collect()),
        rots: Arc::new(rots),
        anchors: Arc::new(anchors),
        hi: Arc::new(hi),
        lo: Arc::new(lo),
        nanch,
        anc_centres,
        ahat,
        row_ent,
        ent_row,
        lcode,
        lscal,
        coff,
        soff,
        esc,
        x2: v["x2"].as_f64().ok_or(format!("{dir}/index.json has no x2; rebuild the index"))?,
        full: Full2::Real { rows: Arc::new(rows) },
        nrows,
        vrow: Vec::new(),
        nent,
    };
    // what this open derived again (plans, codebooks, rotations, centres; with this library and its dependencies) must
    // be what the build derived
    let ph = format!("{:016x}", ix.plan_hash());
    match v["plan_hash"].as_str() {
        Some(h) if h == ph => {}
        h => return Err(format!("{dir}: the plans this build derives (hash {ph}) are not the ones the index was built with ({}); rebuild the index", h.unwrap_or("none recorded"))),
    }
    log(format!("opened {dir} in {:.3}s: {nrows} rows, {k} clusters; when built: code hash {}, store hash {}", t.s(), v["code_hash"].as_str().unwrap_or("?"), v["store_hash"].as_str().unwrap_or("?")));
    Ok(ix)
}

/// `path` now against its identity `was` when the index in `dir` was built: another length or modification time is an
/// error unless `keep` (then a warning); another status-change time, device or inode only (chmod, a copy keeping its
/// times, a restore, a hard link) is a warning; a missing file, or none recorded, is a warning (`holds` says why the
/// index does not need it).
fn check_file(what: &str, path: &str, was: Option<XsId>, holds: &str, keep: bool, dir: &str, log: &mut dyn FnMut(String)) -> Result<(), String> {
    match (was, XsId::of(path)) {
        (_, Err(e)) => log(format!("WARNING: {what} is missing or unreadable ({e}); {holds}, opening it")),
        (None, Ok(_)) => log(format!("WARNING: {dir}/index.json does not record {what} it was built from; opening it")),
        (Some(was), Ok(now)) if was != now => {
            if was.len != now.len || was.mtime != now.mtime {
                let changed = format!("{path} changed since this index was built (length {} -> {}, modified {:?} -> {:?})", was.len, now.len, was.mtime, now.mtime);
                if !keep {
                    return Err(format!("{changed}; --rebuild, or --keep-index to use the index as built"));
                }
                log(format!("WARNING: {changed}; --keep-index: using the index as built ({holds})"));
            } else {
                log(format!("WARNING: {path}: same length and modification time as when this index was built, but another {} (a chmod, a copy keeping its times, a restore, a hard link, or an in-place rewrite with its time set back); opening the index ({holds})", id_diff(&was, &now)));
            }
        }
        _ => {}
    }
    Ok(())
}

/// What differs between two identities of a file with the same length and modification time.
fn id_diff(was: &XsId, now: &XsId) -> String {
    let mut which = Vec::new();
    if was.ctime != now.ctime {
        which.push("status-change time (ctime)");
    }
    if was.dev != now.dev {
        which.push("device");
    }
    if was.ino != now.ino {
        which.push("inode");
    }
    which.join(", ")
}

/// A partitioned fit's partition against the one fitted (`partition_id`, its manifest's content, and the files'
/// lengths it lists; O(cells)): another is an error unless `keep` (then a warning); the same content under another
/// file identity of its manifest than at the build (`was`: a copy, a move, a chmod) is a warning; a missing partition,
/// or the fitted manifest with some of the files it lists deleted, is a warning (the index holds its own vectors; the
/// cells may be deleted after the build).
fn check_partition(p: &Params, was: Option<XsId>, keep: bool, log: &mut dyn FnMut(String)) -> Result<(), String> {
    let man = format!("{}/manifest.json", p.partition);
    let holds = "the index holds its own vectors";
    let now = match XsId::of(&man) {
        Err(e) => {
            log(format!("WARNING: the fit's partition is missing or unreadable ({e}); {holds}, opening it"));
            return Ok(());
        }
        Ok(now) => now,
    };
    match crate::bucket::partition_id_missing(&p.partition) {
        // the manifest is the fitted one, but cell files it lists are gone: as a missing partition
        Ok((id, missing)) if id == p.partition_id && !missing.is_empty() => {
            log(format!("WARNING: the fit's partition {} lacks {} of the files its manifest lists ({}{}); {holds}, opening it", p.partition, missing.len(), missing[..missing.len().min(3)].join(", "), if missing.len() > 3 { ", ..." } else { "" }));
            Ok(())
        }
        Ok((id, _)) if id == p.partition_id => {
            if let Some(was) = was.filter(|w| *w != now) {
                log(format!("WARNING: the partition {} holds what was fitted, but its manifest.json is another file than when this index was built (length {} -> {}, modified {:?} -> {:?}{}{}: a copy, a move or a chmod); opening the index", p.partition, was.len, now.len, was.mtime, now.mtime, if id_diff(&was, &now).is_empty() { "" } else { ", another " }, id_diff(&was, &now)));
            }
            Ok(())
        }
        r => {
            let changed = format!("the partition {} changed since the fit this index was built from ({})", p.partition, match r { Ok((id, _)) => format!("identity {} when fitted, {id} now", p.partition_id), Err(e) => e });
            if !keep {
                return Err(format!("{changed}; refit and --rebuild, or --keep-index to use the index as built"));
            }
            log(format!("WARNING: {changed}; --keep-index: using the index as built ({holds})"));
            Ok(())
        }
    }
}

/// The key of the index the fit `p` (with the options applied), the profile `prof` and the options `o` build: a saved
/// index is opened only when its key is this one. It covers what the build reads besides the vectors: the fit's
/// parameters, the profile, the name of the vectors file or of the partition as meta.json gives it (relative to the
/// fitted directory unless absolute), the options that change what is built, the formats of the encoding and of the files, and the
/// library's source fingerprint (`LIB_FINGERPRINT`). It is a fixed hash (`Fx`), the same on every Rust release, and
/// does not depend on how the fitted directory's path is spelled or where the directory is. The vectors file's
/// identity is not in the key: an index holds its own vectors, and `open` compares the identity recorded at the build.
pub fn build_key(p: &Params, prof: &[f64], o: &BuildOptions) -> String {
    build_key_with(p, prof, o, LIB_FINGERPRINT)
}

/// `build_key` with the library fingerprint `lib`.
pub fn build_key_with(p: &Params, prof: &[f64], o: &BuildOptions, lib: &str) -> String {
    let mut h = Fx::default();
    h.str("vexact saved index key");
    h.u64(STORE_FORMAT);
    h.u64(INDEX_FORMAT as u64);
    h.str(lib);
    let m = &p.meta;
    h.usizes(&[m.n, m.k, m.g]);
    h.usizes(&m.dlv);
    h.usizes(&m.rlv);
    h.usizes(&m.bounds);
    h.pod(&[m.csg, m.eps]);
    h.u64(m.certs.len() as u64);
    for c in &m.certs {
        h.usizes(&c.ds);
        h.pod(&[c.lnd, c.x]);
    }
    h.usizes(&p.cstart);
    h.usizes(&p.grp);
    // not perm.npy's rows (8 bytes a row; an open reads none): the build records their hash and the file's identity,
    // which an open checks as it checks the vectors file's
    h.str(&p.fit);
    h.pod(&p.centres);
    h.pod(&p.v);
    for v in [&p.lam, &p.tree_edges, &p.tree_cent, &p.gtree] {
        h.pod(v);
    }
    match &p.r2 {
        Some(r2) => {
            h.u64(1);
            h.pod(r2);
        }
        None => h.u64(0),
    }
    // the support table's anchors (the norm sort builds none)
    if !o.nusort {
        h.u64(p.anchors.len() as u64);
        h.pod(&p.anchors);
    }
    h.pod(prof);
    h.str(&p.xs_name);
    h.str(&p.partition_name);
    h.str(&p.partition_id);
    // the options not already in the fit and profile above (the clusters, randomised levels and profile are)
    h.usizes(&[o.design_exact as usize, o.nusort as usize, o.csg_plain as usize, o.cal_items]);
    match o.nguar {
        Some(x) => h.pod(&[1.0, x]),
        None => h.u64(0),
    }
    format!("{:016x}", h.finish())
}

impl Index2 {
    /// A fixed hash (`Fx`) of what an open derives again instead of reading: the meta, the plans and calibrated
    /// tables of every group, the deterministic and expanded codebooks, the rotations, the centres and whether the
    /// support table's anchors are the centres (an open derives it from the fit and checks the build's). The build
    /// records it and an open must derive the same (another library or dependency version could derive others).
    /// The repack thresholds are timed, not derived, and left out.
    pub fn plan_hash(&self) -> u64 {
        let mut h = Fx::default();
        h.str("vexact plans");
        h.usizes(&[self.n(), self.k(), self.meta.npow as usize]);
        h.usizes(&self.meta.dlv);
        h.usizes(&self.meta.rlv);
        // (not which deterministic levels decide, `Meta2::ddec`: a query-time choice, not stored)
        for g in self.groups.iter() {
            h.pod(&g.sig);
            h.pod(&[g.lamsum]);
            g.d.iter().for_each(|d| h.pod(d));
            h.pod(&g.pconst);
            h.pod(&g.kmin);
            h.pod(&g.ntab);
            h.pod(&g.dt);
            h.pod(&g.ridx);
            h.usizes(&g.nb);
            h.usizes(&g.boff);
            h.pod(&g.ypredw);
            h.pod(&g.ytab);
            h.pod(&g.csgl);
            h.pod(&g.yhl);
            h.pod(&g.rates);
            g.thchg.iter().for_each(|t| h.usizes(t));
            h.pod(&g.ttab);
            h.pod(&g.ktab);
            h.pod(&g.khl);
            h.pod(&g.kstep);
            h.pod(&g.kbits);
            g.rrow.iter().for_each(|t| h.usizes(t));
            h.pod(&g.ystep);
            g.rdepth.iter().for_each(|t| h.pod(t));
            h.pod(&[g.e_inv]);
            h.pod(&g.v8t);
            h.pod(&g.vsc);
            match &g.cal {
                None => h.u64(0),
                Some(c) => {
                    h.u64(1);
                    h.pod(&c.kmin);
                    h.pod(&c.y0);
                    h.pod(&c.ys);
                    h.pod(&c.tq);
                    h.pod(&c.k0);
                    h.pod(&c.ks);
                    h.pod(&c.kb);
                }
            }
            h.u64(g.levels.len() as u64);
            for lp in &g.levels {
                h.usizes(&[lp.units, lp.srows, lp.parts.len()]);
                for part in &lp.parts {
                    h.u64(part.j.map_or(u64::MAX, |j| j as u64));
                    h.pod(&part.ufields);
                    h.u64(part.fields.len() as u64);
                    for fl in &part.fields {
                        let kind = match fl.kind {
                            FK::Tree { to } => (1u64 << 8) | to as u64,
                            FK::Resid => 2 << 8,
                            FK::Rand { d } => (3u64 << 8) | d as u64,
                        };
                        h.usizes(&[fl.widx as usize, fl.nb as usize, fl.unit as usize, fl.off as usize, kind as usize, fl.eid as usize]);
                    }
                }
            }
        }
        self.codes.cent.iter().for_each(|v| h.pod(v));
        self.codes.edges.iter().for_each(|v| h.pod(v));
        h.pod(&self.codes.delta);
        h.pod(&self.codes.d);
        h.pod(&self.etab.e);
        h.pod(&self.etab.lo);
        h.pod(&self.etab.hi);
        h.pod(&self.centres64);
        self.rots.iter().for_each(|r| h.pod(r));
        h.u64(self.anc_centres as u64);
        h.finish()
    }

    /// A fixed hash (`Fx`) of everything the index holds and a query reads: `plan_hash`, the sizes and `x2`, every
    /// per-row array (codes, scalars, keys, row and entity maps, norm keys, compaction's row map), the offsets and
    /// escapes, every per-cluster record and centre table, the support table and the stored vectors. Only the repack
    /// thresholds, which are timed, are left out. The large arrays are hashed in parallel in fixed chunks (`Fx::big`),
    /// so the value does not depend on the thread count. A built index and the same index opened hash alike.
    pub fn store_hash(&self) -> u64 {
        let mut h = Fx::default();
        h.str("vexact index");
        h.u64(self.plan_hash());
        h.usizes(&[self.n(), self.k(), self.nrows, self.nent, self.nanch, self.anc_centres as usize]);
        h.pod(&[self.x2]);
        for v in self.lcode.iter().chain(&self.lscal) {
            h.big(v);
        }
        h.big_pod(&self.ahat[..]);
        h.big_pod(&self.row_ent[..]);
        h.big_pod(&self.ent_row[..]);
        h.big_pod(&self.nukey[..]);
        h.big_pod(&self.vrow[..]);
        for o in self.coff.iter().chain(&self.soff) {
            h.usizes(o);
        }
        let mut e: Vec<(u8, u8, u32, u32)> = self.esc.iter().map(|(k, v)| (k.0, k.1, k.2, v.to_bits())).collect();
        e.sort_unstable();
        let e: Vec<u64> = e.into_iter().flat_map(|(a, b, c, d)| [((a as u64) << 40) | ((b as u64) << 32) | c as u64, d as u64]).collect();
        h.big_pod(&e);
        for (c, t) in self.cl.iter().zip(self.ctab.iter()) {
            let mut w = Wr::default();
            put_cluster(&mut w, c, t, self.meta.ndl());
            h.bytes(&w.0);
        }
        h.pod(&self.anchors[..]);
        h.pod(&self.hi[..]);
        h.pod(&self.lo[..]);
        match &self.full {
            Full2::Real { rows } if self.vrow.is_empty() => h.big(rows.as_bytes(self.nrows * self.n())),
            // a compaction without its own vectors reads its parent's (hashed with the parent)
            Full2::Real { .. } => h.u64(0),
            Full2::Synth(s) => {
                h.big_pod(&s.ext);
                h.big_pod(&s.row_cl);
            }
        }
        h.finish()
    }

    /// The row and entity maps checked against each other and the index's size: O(rows), so not part of `open`; run
    /// with `--hash` and `verify`.
    pub fn check_maps(&self) -> Result<(), String> {
        if self.row_ent.iter().any(|&e| e as usize >= self.nent) {
            return Err("an entity id in row_ent is out of range; rebuild the index".into());
        }
        for (e, &r) in self.ent_row.iter().enumerate() {
            if r != u32::MAX && (r as usize >= self.nrows || self.row_ent[r as usize] as usize != e) {
                return Err(format!("ent_row and row_ent disagree at entity {e}; rebuild the index"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vexact-{tag}-{}-{}", std::process::id(), nanos()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A copy of testdata/synth4k in `dst` (the tests never write into testdata).
    pub(crate) fn copy_fit(dst: &Path) {
        let src = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
        std::fs::create_dir_all(dst).unwrap();
        for e in std::fs::read_dir(src).unwrap() {
            let e = e.unwrap();
            if e.file_type().unwrap().is_file() {
                std::fs::copy(e.path(), dst.join(e.file_name())).unwrap();
            }
        }
    }

    /// A file-backed output array holds what a heap one holds, read back as an opened index reads it.
    #[test]
    fn output_arrays_on_disk_hold_what_the_heap_holds() {
        let dir = tmpdir("out");
        let d = dir.to_str().unwrap();
        let (heap, file) = (Out::<u32>::new(None, "", 1000), Out::<u32>::new(Some(d), "a.bin", 1000));
        let v: Vec<u32> = (0..300).map(|i| i * 7 + 1).collect();
        for o in [&heap, &file] {
            unsafe {
                o.put(0, &v[..100]);
                o.put(700, &v[100..]);
            }
        }
        let (a, b) = (heap.finish(), file.finish());
        assert_eq!(&a[..], &b[..]);
        assert_eq!(&b[700..900], &v[100..]);
        assert!(b[100..700].iter().chain(&b[900..]).all(|&x| x == 0));
        assert_eq!(&Arr::<u32>::map(&format!("{d}/a.bin")).unwrap()[..], &b[..], "the file holds what was written");
        // the vectors: on the heap and on file padded to whole pages, the same rows
        let (heap, file) = (OutRows::new(None, "", 1000), OutRows::new(Some(d), "v.bin", 1000));
        let w: Vec<u16> = (0..500).map(|i| i * 3 + 1).collect();
        for o in [&heap, &file] {
            unsafe { o.put(250, &w) };
        }
        assert_eq!(std::fs::metadata(dir.join("v.bin")).unwrap().len(), 4096);
        let (a, b) = (heap.finish(), file.finish());
        assert!(matches!(a, disk::Rows::Mem(_)) && matches!(b, disk::Rows::Map { .. }));
        for r in 0..4 {
            assert_eq!(a.row(r, 250), b.row(r, 250));
        }
        assert_eq!(b.row(1, 250), &w[..250]);
        // an output file is never one that exists (a rebuild writes only into its own scratch directory)
        assert!(std::panic::catch_unwind(|| Out::<u8>::new(Some(d), "a.bin", 10)).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Writes from many threads into disjoint ranges, on file and on the heap, give what one thread writing them gives.
    #[test]
    fn output_arrays_take_disjoint_writes_from_many_threads() {
        use rayon::prelude::*;
        let dir = tmpdir("outmt");
        let d = dir.to_str().unwrap();
        let (chunks, w) = (257usize, 1031usize);
        let val = |c: usize, i: usize| (c * 2_654_435_761 + i * 40_503) as u32;
        let (heap, file) = (Out::<u32>::new(None, "", chunks * w + 5), Out::<u32>::new(Some(d), "m.bin", chunks * w + 5));
        let (rheap, rfile) = (OutRows::new(None, "", chunks * w), OutRows::new(Some(d), "r.bin", chunks * w));
        (0..chunks).into_par_iter().for_each(|c| {
            let v: Vec<u32> = (0..w).map(|i| val(c, i)).collect();
            let h: Vec<u16> = v.iter().map(|&x| x as u16).collect();
            // SAFETY: chunk c writes only [c w, (c + 1) w)
            unsafe {
                heap.put(c * w, &v);
                file.put(c * w, &v);
                rheap.put(c * w, &h);
                rfile.put(c * w, &h);
            }
        });
        let want: Vec<u32> = (0..chunks).flat_map(|c| (0..w).map(move |i| val(c, i))).chain([0; 5]).collect();
        let (a, b) = (heap.finish(), file.finish());
        assert_eq!(&a[..], &want[..]);
        assert_eq!(&b[..], &want[..]);
        let (ra, rb) = (rheap.finish(), rfile.finish());
        assert_eq!(ra.as_bytes(chunks * w), rb.as_bytes(chunks * w));
        assert_eq!(rb.row(3, w)[7], want[3 * w + 7] as u16);
        drop((b, rb));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    #[should_panic(expected = "runs past")]
    fn output_arrays_refuse_writes_past_their_end() {
        unsafe { Out::<u8>::new(None, "", 10).put(8, &[1, 2, 3]) };
    }

    /// The key does not depend on how the fitted directory's path is spelled or where the directory is, nor on the
    /// vectors file's identity (`open` checks that); it changes with the library's fingerprint.
    #[test]
    fn the_key_follows_the_fit_and_the_library_not_the_path() {
        let root = tmpdir("key");
        let a = root.join("a");
        copy_fit(&a);
        let o = BuildOptions::default();
        let key = |d: &str| {
            let (p, prof) = prepare(d, &o).unwrap();
            build_key(&p, &prof, &o)
        };
        let abs = a.to_str().unwrap().to_string();
        let k0 = key(&abs);
        assert_eq!(key(&format!("{}/./a", root.display())), k0, "spelled with ./");
        assert_eq!(key(&format!("{}//a/", root.display())), k0, "spelled with // and a trailing /");
        let cwd = std::env::current_dir().unwrap();
        let rel = format!("{}{}", "../".repeat(cwd.components().count() - 1), abs.trim_start_matches('/'));
        assert!(!rel.starts_with('/') && Path::new(&rel).join("meta.json").exists());
        assert_eq!(key(&rel), k0, "spelled relative to the working directory");
        let b = root.join("b");
        std::fs::rename(&a, &b).unwrap();
        let bs = b.to_str().unwrap();
        assert_eq!(key(bs), k0, "the fitted directory moved");
        // rows of Xs.npy swapped in place, same length, modification time set back (as touch -r does)
        let xs = b.join("Xs.npy");
        let md = std::fs::metadata(&xs).unwrap();
        let (len, mtime) = (md.len(), md.modified().unwrap());
        std::thread::sleep(std::time::Duration::from_millis(20));
        {
            use std::io::{Seek, SeekFrom};
            let mut buf = std::fs::read(&xs).unwrap();
            let row = 64 * 4;
            let end = buf.len();
            let (x, y) = buf[end - 2 * row..].split_at_mut(row);
            x.swap_with_slice(y);
            let mut f = std::fs::OpenOptions::new().write(true).open(&xs).unwrap();
            f.seek(SeekFrom::Start((end - 2 * row) as u64)).unwrap();
            f.write_all(&buf[end - 2 * row..]).unwrap();
            f.set_modified(mtime).unwrap();
        }
        let md2 = std::fs::metadata(&xs).unwrap();
        assert_eq!((md2.len(), md2.modified().unwrap()), (len, mtime));
        assert_eq!(key(bs), k0, "the vectors file's identity is not part of the key");
        // the library's fingerprint
        let (p, prof) = prepare(bs, &o).unwrap();
        assert_eq!(build_key(&p, &prof, &o), build_key_with(&p, &prof, &o, LIB_FINGERPRINT));
        assert_ne!(build_key_with(&p, &prof, &o, "0000000000000000"), build_key_with(&p, &prof, &o, "0000000000000001"));
        assert_eq!(LIB_FINGERPRINT.len(), 16);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// --full-file: refused inside a saved index, for the fit's vectors and for a file this process maps; another name
    /// of a file (a hard link) is replaced, never truncated or written.
    #[test]
    fn a_full_vector_copy_never_writes_another_file() {
        use std::os::unix::fs::MetadataExt;
        let root = tmpdir("ff");
        let rd = root.to_str().unwrap();
        let fresh = || {
            let o = OutRows::new(None, "", 4096);
            unsafe { o.put(0, &(0..4096).map(|i| i as u16).collect::<Vec<_>>()) };
            o.finish()
        };
        std::fs::create_dir_all(root.join("encoded-0123")).unwrap();
        let inside = format!("{rd}/encoded-0123/vectors.bin");
        assert!(fresh().to_file(&inside, "normal", &[]).is_err());
        assert!(!Path::new(&inside).exists());
        // a hard link to another file: the link is replaced, the other file keeps its bytes
        let orig = format!("{rd}/orig.bin");
        std::fs::write(&orig, vec![7u8; 10000]).unwrap();
        let link = format!("{rd}/link.bin");
        std::fs::hard_link(&orig, &link).unwrap();
        // the same file as one to avoid (the fit's vectors), under another name
        assert!(fresh().to_file(&link, "normal", &[&orig]).is_err());
        // a file this process maps, under another name
        let mapped = Arr::<u8>::map(&orig).unwrap();
        assert!(fresh().to_file(&link, "normal", &[]).is_err());
        drop(mapped);
        let mut r = fresh();
        r.to_file(&link, "normal", &[]).unwrap();
        assert_eq!(std::fs::read(&orig).unwrap(), vec![7u8; 10000], "the other name's file was written");
        assert_ne!(std::fs::metadata(&orig).unwrap().ino(), std::fs::metadata(&link).unwrap().ino());
        assert_eq!(r.row(5, 16)[3], 83);
        assert_eq!(std::fs::metadata(&link).unwrap().len(), 8192);
        drop(r);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// The calibration an opened index sets its grids from is the build's, bit for bit.
    #[test]
    fn calibration_round_trips_exactly() {
        use rand::{Rng, SeedableRng};
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(5);
        let mut f = || -> f64 { rng.gen::<f64>() * 10f64.powi(rng.gen_range(-30..30)) * if rng.gen() { -1.0 } else { 1.0 } };
        let nr = 4;
        let cal = CalIn {
            kmin: core::array::from_fn(|l| l as i32 * 7 - 20),
            y0: core::array::from_fn(|_| f()),
            ys: core::array::from_fn(|_| f()),
            tq: (0..nr).map(|_| core::array::from_fn(|_| core::array::from_fn(|_| f() as f32))).collect(),
            k0: (0..nr).map(|_| core::array::from_fn(|_| f())).collect(),
            ks: (0..nr).map(|_| core::array::from_fn(|_| f())).collect(),
            kb: (0..nr).map(|_| core::array::from_fn(|j| if j % 2 == 0 { 4 } else { 8 })).collect(),
        };
        let text = serde_json::to_string(&cal_json(&cal, NDL_MAX)).unwrap();
        let back = cal_from(&serde_json::from_str(&text).unwrap(), NDL_MAX).unwrap();
        // with fewer deterministic levels only theirs are stored, and another count is refused
        let few = CalIn { kmin: core::array::from_fn(|l| if l < 2 { cal.kmin[l] } else { 0 }), ..cal.clone() };
        let t2 = serde_json::to_string(&cal_json(&few, 2)).unwrap();
        assert_eq!(format!("{:?}", cal_from(&serde_json::from_str(&t2).unwrap(), 2).unwrap()), format!("{few:?}"));
        assert!(cal_from(&serde_json::from_str(&t2).unwrap(), 3).is_err());
        assert_eq!(format!("{:?}", back), format!("{:?}", cal));
        let bits = |c: &CalIn| -> Vec<u64> {
            c.y0.iter().chain(&c.ys).chain(c.k0.iter().flatten()).chain(c.ks.iter().flatten()).map(|x| x.to_bits())
                .chain(c.tq.iter().flatten().flatten().map(|x| x.to_bits() as u64)).collect()
        };
        assert_eq!(bits(&back), bits(&cal));
    }
}
