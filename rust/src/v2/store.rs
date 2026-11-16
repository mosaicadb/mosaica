//! The encoded index on disk. A build writes it; opening it maps the per-row arrays (codes, scalars, keys, row maps,
//! full vectors) and reads the per-group and per-cluster data, so no item is encoded again.
//!
//! A saved index is a directory of files: `code{l}.bin` and `scal{l}.bin` per level, `ahat.bin`, `row_ent.bin`,
//! `ent_row.bin`, `nukey.bin`, `vectors.bin` (the f16 vectors in index row order, padded to whole 4,096-byte pages so
//! that O_DIRECT reads of the last rows are not short), `offsets.bin`, `esc_key.bin` and `esc_val.bin` (the escapes),
//! `clusters.bin` (the per-cluster records and centre tables), `anchors.bin`, `hi.bin` and `lo.bin` (the support
//! table), and `index.json` (sizes, the per-group calibration, constants and basis defect, and the key of the fit and
//! options it was built from).
//!
//! A rebuild holds an exclusive lock on `encoded.lock` beside the index directory for its whole run, writes into a
//! scratch directory of its own (`encoded-KEY.tmp-PID-NANOS`), syncs every file and the directory, and then swaps the
//! complete directory into place (an atomic exchange where the file system has one, else the old directory is renamed
//! aside first; it is removed afterwards). An open reads every file through one descriptor of the index directory, so
//! it sees one complete build, never files of two.

use super::*;
use std::io::Write;
use std::ops::Deref;
use std::os::unix::io::AsRawFd;

/// The version of the saved index; any other is refused.
pub const STORE_FORMAT: u64 = 5;

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

fn put_cluster(w: &mut Wr, c: &Cl2, ctab: &[kern::CTab]) {
    for x in [c.g, c.r0, c.r1, c.nblk] {
        w.put(x as u64);
    }
    for x in [c.mn, c.btot, c.mvn, c.rmax, c.numax, c.bmin, c.bmax] {
        w.put(x);
    }
    w.put(c.mn32);
    w.arr(&c.bm);
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

fn get_cluster(r: &mut Rd) -> Result<(Cl2, Vec<kern::CTab>), String> {
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
    let bm: [f64; NDL] = r.arr::<f64>()?.try_into().map_err(|_| "clusters.bin: a record of another level count; rebuild the index")?;
    let b2: [f64; J] = r.arr::<f64>()?.try_into().map_err(|_| "clusters.bin: a record of another block count; rebuild the index")?;
    let l1m: [f32; J + 1] = r.arr::<f32>()?.try_into().map_err(|_| "clusters.bin: a record of another block count; rebuild the index")?;
    let (mhat, mv32, wmr) = (r.arr()?, r.arr()?, r.arr()?);
    let nt = r.get::<u64>()? as usize;
    let ctab = (0..nt).map(|_| Ok(kern::CTab { th: r.arr()?, tmax: r.arr()?, step: r.arr()?, off: r.arr()?, err: r.arr()? })).collect::<Result<_, String>>()?;
    Ok((Cl2 { g, r0, r1, nblk, mn, mn32, mhat: Arc::new(mhat), mv32: Arc::new(mv32), wmr: Arc::new(wmr), bm, b2, l1m, btot, mvn, rmax, numax, bmin, bmax }, ctab))
}

fn cal_json(c: &CalIn) -> serde_json::Value {
    let fin = c.y0.iter().chain(&c.ys).chain(c.k0.iter().flatten()).chain(c.ks.iter().flatten()).all(|x| x.is_finite()) && c.tq.iter().flatten().flatten().all(|x| x.is_finite());
    assert!(fin, "a calibration value is not finite and cannot be saved in JSON: {c:?}");
    serde_json::json!({
        "kmin": c.kmin, "y0": c.y0, "ys": c.ys,
        "tq": c.tq.iter().map(|a| a.iter().map(|b| b.to_vec()).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "k0": c.k0, "ks": c.ks, "kb": c.kb,
    })
}

fn cal_from(v: &serde_json::Value) -> Result<CalIn, String> {
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
    let mut kmin = [0i32; NDL];
    for (l, y) in kmin.iter_mut().enumerate() {
        *y = v["kmin"][l].as_i64().ok_or(BAD)? as i32;
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

/// `index.json`, written to a new file and renamed into place (it is written twice: without a key while the build
/// runs, then with its key and hash when the index is complete).
fn index_json(ix: &Index2, dir: &str, key: Option<&str>, store_hash: Option<u64>) {
    let groups: Vec<serde_json::Value> = ix.groups.iter().map(|g| serde_json::json!({
        "e_inv": g.e_inv, "csgl": g.csgl, "cal": g.cal.as_ref().map(cal_json),
    })).collect();
    let meta = serde_json::json!({
        "format": STORE_FORMAT, "index_format": INDEX_FORMAT, "key": key, "lib": LIB_FINGERPRINT, "n": ix.n(), "k": ix.k(), "nrows": ix.nrows,
        "npow": ix.meta.npow, "x2": ix.x2, "nanch": ix.nanch, "nent": ix.nent, "code_hash": format!("{:016x}", ix.code_hash()),
        "store_hash": store_hash.map(|h| format!("{h:016x}")),
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
        put_cluster(&mut w, c, t);
    }
    write_file(&f("clusters.bin"), &w.0);
    index_json(ix, dir, None, None);
}

/// Finish the index built into `dir`: write its support table, give `index.json` the key it answers to and the
/// index's `store_hash` (written last, so a directory whose `index.json` has a key is complete), and sync the
/// directory. Returns the store hash.
pub fn seal(ix: &Index2, dir: &str, key: &str) -> u64 {
    let f = |name: &str| format!("{dir}/{name}");
    write_file(&f("anchors.bin"), &ix.anchors);
    write_file(&f("hi.bin"), &ix.hi);
    write_file(&f("lo.bin"), &ix.lo);
    let h = ix.store_hash();
    index_json(ix, dir, Some(key), Some(h));
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

/// An exclusive lock on `encoded.lock` in the directory that holds the saved indexes, held by a rebuild for its whole
/// run: a second rebuild there waits for the first (and says so). Released when dropped.
pub struct Lock(#[allow(dead_code)] std::fs::File);
impl Lock {
    pub fn take(parent: &str, log: &mut dyn FnMut(String)) -> Lock {
        let path = format!("{parent}/encoded.lock");
        let f = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&path).unwrap_or_else(|e| panic!("cannot open the lock file {path}: {e}"));
        unsafe {
            if libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) != 0 {
                let e = std::io::Error::last_os_error();
                if e.raw_os_error() != Some(libc::EWOULDBLOCK) {
                    panic!("cannot lock {path}: {e}");
                }
                log(format!("waiting for another rebuild in {parent} (it holds {path})"));
                let t = Timer::new();
                while libc::flock(f.as_raw_fd(), libc::LOCK_EX) != 0 {
                    let e = std::io::Error::last_os_error();
                    if e.raw_os_error() != Some(libc::EINTR) {
                        panic!("cannot lock {path}: {e}");
                    }
                }
                log(format!("the other rebuild finished; waited {:.1}s", t.s()));
            }
        }
        Lock(f)
    }
}

/// A new scratch directory beside `edir`, named for this process and moment, for a rebuild to write into.
pub fn scratch_dir(edir: &str) -> String {
    let tmp = format!("{edir}.tmp-{}-{}", std::process::id(), nanos());
    std::fs::create_dir(&tmp).unwrap_or_else(|e| panic!("cannot create {tmp}: {e}"));
    tmp
}

/// Put the complete, synced index directory `tmp` in place as `edir`, replacing any index there: an atomic exchange
/// of the two directories where the file system has one (an open sees the old index or the new one, whole), else
/// the old one is renamed aside first (an open meanwhile finds no index and says to rebuild). The old directory is
/// removed afterwards; indexes already open keep the files they mapped.
pub fn install(tmp: &str, edir: &str) {
    let parent = parent_of(edir);
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
        std::fs::remove_dir_all(&o).unwrap_or_else(|e| panic!("cannot remove the replaced index {o}: {e}"));
    }
}

/// Log every other `encoded-*` directory in `parent` (other keys' indexes, now stale for this fit and these options,
/// and rebuilds' scratch directories) with its size. Nothing is removed.
pub fn log_stale(parent: &str, keep: &str, log: &mut dyn FnMut(String)) {
    let keep = std::path::Path::new(keep).file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
    let Ok(rd) = std::fs::read_dir(parent) else { return };
    let mut found: Vec<(String, u64)> = rd.filter_map(|e| e.ok()).filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("encoded-") && *n != keep)
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
/// per-row arrays and the vectors are mapped, the plans are derived from the fit as the build derives them, and the
/// calibration, constants, basis defects, per-cluster records and support table are read. Every file is read through
/// one descriptor of `dir`, so all come from the same build even if a rebuild replaces `dir` meanwhile. A missing,
/// stale or damaged index is an error that says to rebuild. The repack thresholds are left at their default.
pub fn open(p: &Params, prof: &[f64], o: &BuildOptions, design_c: Option<&[f64]>, key: &str, dir: &str, log: &mut dyn FnMut(String)) -> Result<Index2, String> {
    let t = Timer::new();
    let missing = |e: String| format!("no index of this fit and these options is saved in {dir} ({e}); build it with --rebuild (v2::rebuild)");
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
        Some(k) => return Err(format!("the index in {dir} was built from another fit, other options or another version of the library (key {k}, these give {key}); rebuild it")),
        None => return Err(format!("{dir} holds an unfinished index (no key); rebuild it")),
    }
    let us = |k: &str| v[k].as_u64().map(|x| x as usize).ok_or(format!("{dir}/index.json has no {k}; rebuild the index"));
    let (n, k, nrows, nanch, nent) = (us("n")?, us("k")?, us("nrows")?, us("nanch")?, us("nent")?);
    if n != p.meta.n || k != p.meta.k {
        return Err(format!("{dir} was built with {k} clusters of {n} dimensions, the fit has {} of {}; rebuild it", p.meta.k, p.meta.n));
    }
    let Plans { meta, rots, codes, et, mut groups, .. } = plans(p, prof, nrows, o, design_c, false, log);
    if Some(meta.npow as u64) != v["npow"].as_u64() {
        return Err(format!("{dir} was built for a guarantee of 2^{} rows, these options give 2^{}; rebuild it", v["npow"], meta.npow));
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
            let cal = cal_from(&gv["cal"]).map_err(|e| format!("{dir}/{e}"))?;
            if cal.tq.len() != gp.rates.len() {
                return Err(format!("{dir}/index.json: a group's calibration has {} randomised levels, the plan {}; rebuild the index", cal.tq.len(), gp.rates.len()));
            }
            gp.set_calibration(cal);
        }
    }
    let nl = meta.nl();
    let cbuf = std::fs::read(f("clusters.bin")).map_err(|e| format!("cannot read {dir}/clusters.bin: {e}; rebuild the index"))?;
    let mut r = Rd(&cbuf);
    let (cl, ctab): (Vec<Cl2>, Vec<Vec<kern::CTab>>) = (0..k).map(|_| get_cluster(&mut r)).collect::<Result<Vec<_>, String>>().map_err(|e| format!("{dir}/{e}"))?.into_iter().unzip();
    if !r.0.is_empty() {
        return Err(format!("{dir}/clusters.bin has data past its {k} clusters; rebuild the index"));
    }
    for (c, (x, t)) in cl.iter().zip(&ctab).enumerate() {
        let r0 = if c == 0 { 0 } else { cl[c - 1].r1 };
        if x.g >= groups.len() || x.r0 != r0 || x.r1 < x.r0 || x.nblk != (x.r1 - x.r0).div_ceil(BLK) || t.len() != nl || x.mhat.len() != n {
            return Err(format!("{dir}/clusters.bin: cluster {c}'s record does not fit the index; rebuild it"));
        }
    }
    let offs: Arr<u64> = Arr::map(&f("offsets.bin")).map_err(ours)?;
    if offs.len() != 2 * nl * k {
        return Err(format!("{dir}/offsets.bin does not hold {nl} levels of {k} clusters; rebuild the index"));
    }
    let off = |i: usize| -> Vec<Vec<usize>> { (0..nl).map(|l| offs[(i * nl + l) * k..(i * nl + l + 1) * k].iter().map(|&o| o as usize).collect()).collect() };
    let (ek, ev): (Arr<u64>, Arr<f32>) = (Arr::map(&f("esc_key.bin")).map_err(ours)?, Arr::map(&f("esc_val.bin")).map_err(ours)?);
    if ek.len() != ev.len() {
        return Err(format!("{dir}: esc_key.bin and esc_val.bin differ in length; rebuild the index"));
    }
    let esc = ek.iter().zip(ev.iter()).map(|(&k, &v)| (((k >> 40) as u8, (k >> 32) as u8, k as u32), v)).collect();
    let anchors: Vec<f64> = Arr::<f64>::map(&f("anchors.bin")).map_err(ours)?.to_vec();
    let (hi, lo): (Vec<f32>, Vec<f32>) = (Arr::<f32>::map(&f("hi.bin")).map_err(ours)?.to_vec(), Arr::<f32>::map(&f("lo.bin")).map_err(ours)?.to_vec());
    if anchors.len() != nanch * n || hi.len() != nanch * k || lo.len() != nanch * k {
        return Err(format!("{dir}: the support table does not hold {nanch} anchors of {n} and their bounds over {k} clusters; rebuild the index"));
    }
    let lcode: Vec<Arr<u8>> = (0..nl).map(|l| Arr::map(&f(&format!("code{l}.bin")))).collect::<Result<_, _>>().map_err(ours)?;
    let lscal: Vec<Arr<u8>> = (0..nl).map(|l| Arr::map(&f(&format!("scal{l}.bin")))).collect::<Result<_, _>>().map_err(ours)?;
    let (coff, soff) = (off(0), off(1));
    for l in 0..nl {
        // each cluster's blocks lie inside its level's arrays, which end in 128 bytes of slack
        for (c, x) in cl.iter().enumerate() {
            let lp = &groups[x.g].levels[l];
            if coff[l][c] + x.nblk * lp.units * 16 + 128 > lcode[l].len() || soff[l][c] + x.nblk * lp.srows * 16 + 128 > lscal[l].len() {
                return Err(format!("{dir}: level {l}'s codes or scalars are shorter than its offsets; rebuild the index"));
            }
        }
    }
    let (ahat, row_ent, ent_row, nukey) = (Arr::map(&f("ahat.bin")).map_err(ours)?, Arr::map(&f("row_ent.bin")).map_err(ours)?, Arr::map(&f("ent_row.bin")).map_err(ours)?, Arr::map(&f("nukey.bin")).map_err(ours)?);
    if ahat.len() != nrows + 32 || row_ent.len() != nrows || ent_row.len() != nent || !(nukey.is_empty() || nukey.len() == nrows) || cl.last().map_or(0, |c| c.r1) != nrows {
        return Err(format!("{dir}: the row arrays do not hold {nrows} rows; rebuild the index"));
    }
    if row_ent.iter().any(|&e| e as usize >= nent) || ent_row.iter().any(|&r| r != u32::MAX && r as usize >= nrows) {
        return Err(format!("{dir}: the row and entity maps point outside the index; rebuild it"));
    }
    if nukey.is_empty() == o.nusort {
        return Err(format!("{dir} was built {} the norm sort, the options ask {}; rebuild it", if o.nusort { "without" } else { "with" }, if o.nusort { "for it" } else { "for none" }));
    }
    let rows = disk::Rows::open(&f("vectors.bin"), nrows * n, "normal").map_err(|e| format!("{}; rebuild the index", ours(e)))?;
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
    log(format!("opened {dir} in {:.1}s: {nrows} rows, {k} clusters; when built: code hash {}, store hash {}", t.s(), v["code_hash"].as_str().unwrap_or("?"), v["store_hash"].as_str().unwrap_or("?")));
    Ok(ix)
}

/// The key of the index the fit `p` (with the options applied), the profile `prof` and the options `o` build: a saved
/// index is opened only when its key is this one. It covers every input of the build: the fit's parameters, the
/// profile, the identity of the vectors file, the options that change what is built, the formats of the encoding and
/// of the files, and the library's source fingerprint (`LIB_FINGERPRINT`: any edit to the library makes saved indexes
/// stale). It is a fixed hash (`Fx`), the same on every Rust release, and does not depend on how the fitted
/// directory's path is spelled or where the directory is.
///
/// The vectors file (meta.json's `xs`) is identified, not read (at a billion items it is terabytes): by its name as
/// meta.json gives it, its length, its modification and status-change times (to the nanosecond), its device and its
/// inode. Writing the file in place changes its status-change time (ctime), which `touch` and `cp -p` cannot set back;
/// a copied or moved-across-file-systems file has another inode, so it needs a rebuild (the safe direction). What is
/// assumed: nobody rewrites the file in place and restores its ctime, which only setting the system clock back as
/// root can do.
pub fn build_key(p: &Params, prof: &[f64], o: &BuildOptions) -> String {
    build_key_with(p, prof, o, LIB_FINGERPRINT)
}

/// `build_key` with the library fingerprint `lib`.
pub fn build_key_with(p: &Params, prof: &[f64], o: &BuildOptions, lib: &str) -> String {
    use std::os::unix::fs::MetadataExt;
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
    h.pod(&p.perm);
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
    h.pod(prof);
    let md = std::fs::metadata(&p.xs_path).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.xs_path));
    h.str(&p.xs_name);
    for x in [md.len(), md.mtime() as u64, md.mtime_nsec() as u64, md.ctime() as u64, md.ctime_nsec() as u64, md.dev(), md.ino()] {
        h.u64(x);
    }
    // the options not already in the fit and profile above (the clusters, randomised levels and profile are)
    h.usizes(&[o.design_exact as usize, o.nusort as usize, o.csg_plain as usize, o.cal_per_group]);
    match o.nguar {
        Some(x) => h.pod(&[1.0, x]),
        None => h.u64(0),
    }
    format!("{:016x}", h.finish())
}

impl Index2 {
    /// A fixed hash (`Fx`) of everything the index holds and a query reads: the sizes, every per-row array (codes,
    /// scalars, keys, row and entity maps, norm keys, compaction's row map), the offsets and escapes, every
    /// per-cluster record and centre table, the plans and calibrated tables of every group, the deterministic and
    /// expanded codebooks, centres, rotations, the support table and the stored vectors. Only the repack thresholds,
    /// which are timed at each open, are left out. A built index and the same index opened hash alike.
    pub fn store_hash(&self) -> u64 {
        let mut h = Fx::default();
        h.str("vexact index");
        h.usizes(&[self.n(), self.k(), self.nrows, self.nent, self.nanch, self.meta.npow as usize]);
        h.pod(&[self.x2]);
        h.usizes(&self.meta.dlv);
        h.usizes(&self.meta.rlv);
        for v in self.lcode.iter().chain(&self.lscal) {
            h.bytes(v);
        }
        h.pod(&self.ahat);
        h.pod(&self.row_ent);
        h.pod(&self.ent_row);
        h.pod(&self.nukey);
        h.pod(&self.vrow);
        for o in self.coff.iter().chain(&self.soff) {
            h.usizes(o);
        }
        let mut e: Vec<(u8, u8, u32, u32)> = self.esc.iter().map(|(k, v)| (k.0, k.1, k.2, v.to_bits())).collect();
        e.sort_unstable();
        h.u64(e.len() as u64);
        for (a, b, c, d) in e {
            h.u64(((a as u64) << 40) | ((b as u64) << 32) | c as u64);
            h.u64(d as u64);
        }
        for (c, t) in self.cl.iter().zip(self.ctab.iter()) {
            let mut w = Wr::default();
            put_cluster(&mut w, c, t);
            h.bytes(&w.0);
        }
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
            h.str(&format!("{:?}", g.cal));
            h.u64(g.levels.len() as u64);
            for lp in &g.levels {
                h.usizes(&[lp.units, lp.srows, lp.parts.len()]);
                for part in &lp.parts {
                    h.u64(part.j.map_or(u64::MAX, |j| j as u64));
                    h.pod(&part.ufields);
                    h.str(&format!("{:?}", part.fields));
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
        h.pod(&self.anchors);
        h.pod(&self.hi);
        h.pod(&self.lo);
        match &self.full {
            Full2::Real { rows } if self.vrow.is_empty() => h.bytes(rows.as_bytes(self.nrows * self.n())),
            // a compaction without its own vectors reads its parent's (hashed with the parent)
            Full2::Real { .. } => h.u64(0),
            Full2::Synth(s) => {
                h.pod(&s.ext);
                h.pod(&s.row_cl);
            }
        }
        h.finish()
    }
}

#[cfg(test)]
mod tests {
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

    /// The key does not depend on how the fitted directory's path is spelled or where the directory is; it changes
    /// when the vectors file is rewritten in place even with its length and modification time kept, and with the
    /// library's fingerprint.
    #[test]
    fn the_key_follows_the_fit_and_the_library_not_the_path() {
        let root = tmpdir("key");
        let a = root.join("a");
        copy_fit(&a);
        let o = BuildOptions::default();
        let key = |d: &str| {
            let (p, prof) = prepare(d, &o);
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
        assert_ne!(key(bs), k0, "the vectors file was rewritten in place with its length and mtime kept");
        // the library's fingerprint
        let (p, prof) = prepare(bs, &o);
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
        let text = serde_json::to_string(&cal_json(&cal)).unwrap();
        let back = cal_from(&serde_json::from_str(&text).unwrap()).unwrap();
        assert_eq!(format!("{:?}", back), format!("{:?}", cal));
        let bits = |c: &CalIn| -> Vec<u64> {
            c.y0.iter().chain(&c.ys).chain(c.k0.iter().flatten()).chain(c.ks.iter().flatten()).map(|x| x.to_bits())
                .chain(c.tq.iter().flatten().flatten().map(|x| x.to_bits() as u64)).collect()
        };
        assert_eq!(bits(&back), bits(&cal));
    }
}
