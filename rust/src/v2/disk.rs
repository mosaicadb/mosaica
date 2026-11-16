//! Full vectors read from a file under a memory cap: the row-ordered f16 copy written to a file and mapped, how the
//! mapping is advised, a self-imposed cgroup cap once the build is done, and the counters that show what the cap did.

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// The files this process maps (device and inode of each, from /proc/self/maps). On btrfs subvolumes and overlayfs the
/// device /proc/self/maps shows can differ from the one stat reports for the same file, so a match may be missed
/// there; it is a second line of defence (to_file never writes the file it replaces in any case).
pub fn mapped_files() -> std::collections::HashSet<(u64, u64)> {
    let mut out = std::collections::HashSet::new();
    for line in std::fs::read_to_string("/proc/self/maps").unwrap_or_default().lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 6 {
            continue;
        }
        let (Some((ma, mi)), Ok(ino)) = (f[3].split_once(':'), f[4].parse::<u64>()) else { continue };
        let (Ok(ma), Ok(mi)) = (u64::from_str_radix(ma, 16), u64::from_str_radix(mi, 16)) else { continue };
        if ino != 0 {
            // dev_t as glibc encodes it (what MetadataExt::dev returns)
            out.insert((((ma & 0xfff) << 8) | ((ma & !0xfff) << 32) | (mi & 0xff) | ((mi & !0xff) << 12), ino));
        }
    }
    out
}

/// fsync a directory, so that the entries created or renamed in it are durable.
pub fn sync_dir(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::File::open(dir)?.sync_all()
}

/// How the rows on file are read. A benchmark may change it between runs, so the fields are atomic.
pub struct ReadPolicy {
    /// Read the rows the index path leaves with pread through io_uring, not by faults on the mapping.
    pub uring: AtomicBool,
    /// Before deciding a list of rows from full vectors, ask the kernel to start reading their pages.
    pub willneed: AtomicBool,
    /// Extents of at least this many bytes are read with O_DIRECT, past the page cache (usize::MAX: never).
    pub direct_min: AtomicUsize,
    /// Extents of at least this many bytes, and below `direct_min`, are buffered reads with RWF_DONTCACHE: pages
    /// already cached serve them, and pages read in are dropped once copied (usize::MAX: never).
    pub dontcache_min: AtomicUsize,
    /// Reads a `Reader` keeps in flight (at most RING / 2).
    pub qd: AtomicUsize,
    /// The planner prices the reads of these rows (io_us, io_gbps in the plan).
    pub priced: AtomicBool,
}
impl Default for ReadPolicy {
    fn default() -> ReadPolicy {
        ReadPolicy { uring: AtomicBool::new(false), willneed: AtomicBool::new(false), direct_min: AtomicUsize::new(usize::MAX), dontcache_min: AtomicUsize::new(usize::MAX), qd: AtomicUsize::new(64), priced: AtomicBool::new(false) }
    }
}

/// The stored f16 vectors in index row order: in memory (the build's copy) or a mapped file.
pub enum Rows {
    Mem(super::F16Rows),
    // rfd: read with pread, no readahead; dfd: the same file opened with O_DIRECT (plainly where the file system does
    // not support it)
    Map { map: memmap2::Mmap, file: std::fs::File, rfd: std::fs::File, dfd: std::fs::File, policy: ReadPolicy },
}

impl Default for Rows {
    fn default() -> Rows {
        Rows::Mem(Default::default())
    }
}

impl Rows {
    /// Row `r`'s stored vector (rows of `n` f16).
    #[inline(always)]
    pub fn row(&self, r: usize, n: usize) -> &[u16] {
        match self {
            Rows::Mem(v) => &v[r * n..(r + 1) * n],
            Rows::Map { map, .. } => {
                let o = r * 2 * n;
                assert!(o + 2 * n <= map.len());
                unsafe { std::slice::from_raw_parts(map.as_ptr().add(o) as *const u16, n) }
            }
        }
    }

    /// Writes the rows to `path` (from memory, or from the file they are mapped from), syncs, drops the copy and maps
    /// the file with `advice` (normal, random or sequential). The rows go to a new file beside `path` that is renamed
    /// over it once complete, so a file `path` named before (or another name of it, a hard link) is never truncated
    /// or written. Refused: a `path` inside a saved index (a directory named `encoded-*`), and a `path` that is the
    /// file of any of `avoid` or of any file this process maps (the index's own vectors) under another name.
    pub fn to_file(&mut self, path: &str, advice: &str, avoid: &[&str]) -> Result<(), String> {
        use std::os::unix::fs::MetadataExt;
        let p = std::path::Path::new(path);
        let name = p.file_name().ok_or(format!("--full-file {path} names no file"))?.to_string_lossy().to_string();
        let parent = p.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(std::path::Path::new("."));
        let cparent = std::fs::canonicalize(parent).map_err(|e| format!("--full-file {path}: cannot resolve its directory: {e}"))?;
        if cparent.components().any(|c| c.as_os_str().to_string_lossy().starts_with("encoded-")) {
            return Err(format!("--full-file {path} is inside a saved index ({}); give a path outside every encoded-* directory", cparent.display()));
        }
        if let Ok(md) = std::fs::metadata(path) {
            let id = (md.dev(), md.ino());
            for a in avoid {
                if std::fs::metadata(a).is_ok_and(|m| (m.dev(), m.ino()) == id) {
                    return Err(format!("--full-file {path} is {a} (the same file); give a path of its own"));
                }
            }
            if mapped_files().contains(&id) {
                return Err(format!("--full-file {path} is a file this process maps (a saved index's vectors, under this or another name); give a path of its own"));
            }
        }
        let tmp = cparent.join(format!(".{name}.tmp-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos())));
        let f = {
            let bytes = match self {
                Rows::Mem(v) => unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 2) },
                Rows::Map { map, .. } => &map[..],
            };
            let file = std::fs::OpenOptions::new().read(true).write(true).create_new(true).open(&tmp).map_err(|e| format!("cannot create {}: {e}", tmp.display()))?;
            let mut f = std::io::BufWriter::with_capacity(64 << 20, file);
            f.write_all(bytes).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
            let f = f.into_inner().map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
            // whole pages, so that O_DIRECT reads of the last rows are not short
            let len = f.metadata().map_err(|e| format!("{}: {e}", tmp.display()))?.len();
            f.set_len((len + 4095) & !4095).map_err(|e| format!("{}: {e}", tmp.display()))?;
            f.sync_all().map_err(|e| format!("cannot sync {}: {e}", tmp.display()))?;
            f
        };
        std::fs::rename(&tmp, path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("cannot move {} to {path}: {e}", tmp.display())
        })?;
        sync_dir(&cparent).map_err(|e| format!("cannot sync {}: {e}", cparent.display()))?;
        let len = f.metadata().map_err(|e| format!("{path}: {e}"))?.len() as usize / 2;
        *self = Rows::from_file(f, path, len, advice)?;
        self.evict();
        Ok(())
    }

    /// Maps the rows in the existing file `path` (at least `len` f16 values, headerless, as `to_file` and a saved index
    /// write them) with `advice` (normal, random or sequential), for the mapping, the reader and the planner as rows
    /// written by `to_file`, under the default read policy (read through the mapping).
    pub fn open(path: &str, len: usize, advice: &str) -> Result<Rows, String> {
        let file = std::fs::File::open(path).map_err(|e| format!("cannot open {path}: {e}"))?;
        Rows::from_file(file, path, len, advice)
    }

    /// As `open`, the rows in the open file `file` (named `path` in messages). The reader's descriptors are opened on
    /// that same file through /proc/self/fd, never by its name, so they read what `file` holds even if the name has
    /// since moved to another file.
    pub fn from_file(file: std::fs::File, path: &str, len: usize, advice: &str) -> Result<Rows, String> {
        let flen = file.metadata().map_err(|e| format!("cannot read {path}: {e}"))?.len() as usize;
        if flen < 2 * len {
            return Err(format!("{path} holds {flen} bytes, fewer than the {} of its rows", 2 * len));
        }
        let a = match advice {
            "normal" => libc::MADV_NORMAL,
            "random" => libc::MADV_RANDOM,
            "sequential" => libc::MADV_SEQUENTIAL,
            _ => return Err(format!("--full-advice is normal, random or sequential, not {advice}")),
        };
        // an empty file cannot be mapped; map a page of zeros in its place
        let map = if flen == 0 {
            memmap2::MmapOptions::new().len(4096).map_anon().and_then(|m| m.make_read_only())
        } else {
            unsafe { memmap2::Mmap::map(&file) }
        }.map_err(|e| format!("cannot map {path}: {e}"))?;
        unsafe {
            if libc::madvise(map.as_ptr() as *mut libc::c_void, map.len(), a) != 0 {
                return Err(format!("cannot advise the mapping of {path}: {}", std::io::Error::last_os_error()));
            }
        }
        let same = format!("/proc/self/fd/{}", file.as_raw_fd());
        let rfd = std::fs::File::open(&same).map_err(|e| format!("cannot open {path} again: {e}"))?;
        unsafe {
            libc::posix_fadvise(rfd.as_raw_fd(), 0, 0, libc::POSIX_FADV_RANDOM);
        }
        let dfd = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_DIRECT).open(&same).or_else(|_| std::fs::File::open(&same)).map_err(|e| format!("cannot open {path} again: {e}"))?;
        Ok(Rows::Map { map, file, rfd, dfd, policy: ReadPolicy::default() })
    }

    /// The first `len` f16 values of the rows as bytes.
    pub fn as_bytes(&self, len: usize) -> &[u8] {
        let b: &[u8] = match self {
            Rows::Mem(v) => unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 2) },
            Rows::Map { map, .. } => map,
        };
        &b[..2 * len]
    }

    /// The file the rows are mapped from, when they are on file.
    pub fn file_path(&self) -> Option<std::path::PathBuf> {
        match self {
            Rows::Map { file, .. } => std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd())).ok(),
            Rows::Mem(_) => None,
        }
    }

    /// Drops the mapped rows from this process and from the page cache.
    pub fn evict(&self) {
        if let Rows::Map { map, file, .. } = self {
            unsafe {
                libc::madvise(map.as_ptr() as *mut libc::c_void, map.len(), libc::MADV_DONTNEED);
                libc::posix_fadvise(file.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED);
            }
        }
    }

    /// Bytes of the mapped rows resident in the page cache (0 in memory).
    pub fn resident(&self) -> usize {
        match self {
            Rows::Mem(_) => 0,
            Rows::Map { map, .. } => resident_of(map),
        }
    }

    /// Whether `rows` (rows of `n` f16 each) are most likely in the page cache: a sample of eight is.
    pub fn cached(&self, rows: &[u32], n: usize) -> bool {
        let Rows::Map { map, .. } = self else { return true };
        let (base, rb) = (map.as_ptr() as usize, 2 * n);
        let k = rows.len().min(8);
        k > 0 && (0..k).all(|j| {
            let o = rows[j * rows.len() / k] as usize * rb;
            let p0 = (base + o) & !4095;
            let mut v = [0u8; 2];
            unsafe { libc::mincore(p0 as *mut libc::c_void, base + o + rb - p0, v.as_mut_ptr()) == 0 && v[0] & 1 == 1 }
        })
    }

    /// How the rows are read, when they are on file.
    pub fn policy(&self) -> Option<&ReadPolicy> {
        match self {
            Rows::Map { policy, .. } => Some(policy),
            Rows::Mem(_) => None,
        }
    }

    /// Whether the rows are on file and read through io_uring.
    pub fn uring(&self) -> bool {
        self.policy().is_some_and(|p| p.uring.load(Ordering::Relaxed))
    }

    /// Whether the rows are on file and the planner prices their reads.
    pub fn priced(&self) -> bool {
        self.policy().is_some_and(|p| p.priced.load(Ordering::Relaxed))
    }

    /// Whether the rows are on file and their pages are requested ahead of reading.
    pub fn willneed_on(&self) -> bool {
        self.policy().is_some_and(|p| p.willneed.load(Ordering::Relaxed))
    }

    /// Starts reading the pages of `rows` (rows of `n` f16 each) without waiting, when the policy asks for it.
    #[inline]
    pub fn willneed(&self, rows: &[u32], n: usize) {
        if !self.willneed_on() {
            return;
        }
        if let Rows::Map { map, .. } = self {
            let base = map.as_ptr() as usize;
            let rb = 2 * n;
            // a madvise per row of a resident batch would cost more than the reads it starts (10k rows: 14 ms against
            // 1 ms)
            if self.cached(rows, n) {
                return;
            }
            let mut i = 0;
            // adjacent rows coalesce into one range
            while i < rows.len() {
                let a = rows[i] as usize * rb;
                let mut b = a + rb;
                let mut j = i + 1;
                while j < rows.len() && rows[j] as usize * rb <= b + 4096 && rows[j] as usize * rb >= a {
                    b = b.max(rows[j] as usize * rb + rb);
                    j += 1;
                }
                let p0 = (base + a) & !4095;
                unsafe {
                    libc::madvise(p0 as *mut libc::c_void, base + b - p0, libc::MADV_WILLNEED);
                }
                i = j;
            }
        }
    }
}

/// Bytes of a mapping resident in the page cache (mincore).
pub fn resident_of(map: &[u8]) -> usize {
    let pages = (map.len() + 4095) / 4096;
    let mut v = vec![0u8; pages];
    unsafe {
        assert_eq!(libc::mincore(map.as_ptr() as *mut libc::c_void, map.len(), v.as_mut_ptr()), 0);
    }
    v.iter().filter(|&&b| b & 1 == 1).count() * 4096
}

/// This process's cgroup directory.
pub fn cgroup_dir() -> String {
    let s = std::fs::read_to_string("/proc/self/cgroup").unwrap();
    let p = s.lines().find_map(|l| l.strip_prefix("0::")).expect("cgroup v2").trim().to_string();
    format!("/sys/fs/cgroup{p}")
}

fn read_cg(f: &str) -> String {
    std::fs::read_to_string(format!("{}/{f}", cgroup_dir())).unwrap_or_default()
}

/// memory.current, anon and file of this cgroup, in MiB, with the counts of memory.events.
pub fn cg_report() -> String {
    let mb = |x: u64| x as f64 / (1 << 20) as f64;
    let cur: u64 = read_cg("memory.current").trim().parse().unwrap_or(0);
    let st = read_cg("memory.stat");
    let get = |k: &str| st.lines().find_map(|l| l.strip_prefix(&format!("{k} "))).and_then(|v| v.trim().parse::<u64>().ok()).unwrap_or(0);
    let ev = read_cg("memory.events").replace('\n', " ");
    format!("cgroup current {:.1} MiB, anon {:.1}, file {:.1}, max {}; events {}", mb(cur), mb(get("anon")), mb(get("file")), read_cg("memory.max").trim(), ev.trim())
}

/// Bytes this process has caused to be read from storage (/proc/self/io read_bytes).
pub fn io_read_bytes() -> u64 {
    std::fs::read_to_string("/proc/self/io").unwrap_or_default().lines().find_map(|l| l.strip_prefix("read_bytes: ")).and_then(|v| v.trim().parse().ok()).unwrap_or(0)
}

/// Major page faults of this process so far.
pub fn major_faults() -> u64 {
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) };
    ru.ru_majflt as u64
}

/// Returns freed heap to the system, sets this cgroup's memory.max to `cap` bytes and reclaims its page cache.
pub fn cap_self(cap: u64) -> String {
    extern "C" {
        fn malloc_trim(pad: usize) -> i32;
    }
    unsafe {
        malloc_trim(0);
    }
    let dir = cgroup_dir();
    std::fs::write(format!("{dir}/memory.max"), cap.to_string()).unwrap_or_else(|e| panic!("cannot cap {dir}: {e}"));
    let _ = std::fs::write(format!("{dir}/memory.reclaim"), (16u64 << 30).to_string());
    cg_report()
}

/// Nanoseconds readers spent waiting for reads, reads issued and bytes read, over the process (diagnostics).
pub static WAIT_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static NREADS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static NBYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// CPU seconds of this process so far: user, system.
pub fn cpu_s() -> (f64, f64) {
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) };
    let s = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 * 1e-6;
    (s(ru.ru_utime), s(ru.ru_stime))
}

const RWF_DONTCACHE: i32 = 0x80;

const RING: u32 = 256;
/// Longest single read; a longer extent is split so that its pieces are read in parallel.
const PIECE: usize = 256 << 10;
/// Bytes of read buffers a `Reader` holds before it waits for the oldest batch, and bytes of free buffers it keeps for
/// reuse (in power-of-two sizes, so that they fit later batches). Buffers are anonymous mappings, so that a freed one
/// goes back to the system rather than to the heap: under a cap, memory the heap keeps is memory the page cache loses.
const BUDGET: usize = 8 << 20;
const KEEP: usize = 16 << 20;

/// How an extent is read.
#[derive(Clone, Copy, PartialEq)]
enum Via {
    Cached,
    Direct,
    DontCache,
}

/// One batch of rows read into a buffer: `rows` ascending, `at[k]` the byte offset of row k in `buf`.
pub struct Batch<T> {
    pub tag: T,
    pub rows: Vec<u32>,
    at: Vec<u32>,
    buf: memmap2::MmapMut,
    ext: Vec<(u64, u32, u32, u32, Via)>, // file offset, length, offset in buf, bytes done, how
    left: usize,
}

impl<T> Batch<T> {
    /// Row k's stored vector.
    #[inline]
    pub fn row(&self, k: usize, n: usize) -> &[u16] {
        unsafe { std::slice::from_raw_parts(self.buf.as_ptr().add(self.at[k] as usize) as *const u16, n) }
    }
}

thread_local! {
    // this thread's ring and read buffers, kept across queries so that buffers are not allocated afresh each time
    static THREAD_RING: std::cell::RefCell<Option<(io_uring::IoUring, Vec<memmap2::MmapMut>)>> = const { std::cell::RefCell::new(None) };
}

/// Batches of rows read with many reads in flight: `push` plans a batch's reads and queues them, `pop` returns the
/// oldest batch once its reads are done, and up to the policy's `qd` reads of the queued batches are in flight meanwhile. Rows within
/// `gap` bytes of each other are read with one read, gap included. Reads are buffered (through the page cache), so a
/// cached row costs a copy and the cache is charged to the cgroup as with the mapping.
pub struct Reader<T> {
    ring: std::mem::ManuallyDrop<io_uring::IoUring>,
    fd: i32,
    dfd: i32, // the file opened with O_DIRECT
    dmin: usize,
    dcmin: usize,
    rb: usize,
    gap: usize,
    q: std::collections::VecDeque<Batch<T>>,
    seq0: u64,          // sequence number of q[0]
    next: (u64, usize), // the next extent to submit: batch sequence number, extent
    retry: Vec<(u64, usize)>,
    inflight: usize,
    qd: usize,
    held: usize,
    free: Vec<memmap2::MmapMut>,
    pub reads: u64,
    pub bytes: u64,
}

impl<T> Reader<T> {
    /// A reader of `rows` (rows of `n` f16) merging rows `gap` bytes apart, on this thread's ring; None in memory or
    /// unless the policy reads through io_uring.
    pub fn new(rows: &Rows, n: usize, gap: usize) -> Option<Reader<T>> {
        let Rows::Map { rfd, dfd, policy, .. } = rows else { return None };
        if !policy.uring.load(Ordering::Relaxed) {
            return None;
        }
        let (ring, free) = THREAD_RING.with(|r| r.borrow_mut().take()).unwrap_or_else(|| (io_uring::IoUring::new(RING).expect("io_uring"), Vec::new()));
        Some(Reader { ring: std::mem::ManuallyDrop::new(ring), fd: rfd.as_raw_fd(), dfd: dfd.as_raw_fd(), dmin: policy.direct_min.load(Ordering::Relaxed), dcmin: policy.dontcache_min.load(Ordering::Relaxed), rb: 2 * n, gap, q: Default::default(), seq0: 0, next: (0, 0), retry: Vec::new(), inflight: 0, qd: policy.qd.load(Ordering::Relaxed).clamp(1, RING as usize / 2), held: 0, free, reads: 0, bytes: 0 })
    }

    /// Whether the caller should take a batch before pushing another: reads are waiting for a free slot, or the
    /// buffers held reach the budget.
    pub fn full(&self) -> bool {
        self.next.0 < self.seq0 + self.q.len() as u64 || self.held >= BUDGET
    }

    pub fn is_empty(&self) -> bool {
        self.q.is_empty()
    }

    /// Plans the reads of `rows` (any order; the batch holds them ascending) and starts them. An extent of at least
    /// `dmin` bytes is read with O_DIRECT, widened to whole pages; one of at least `dcmin` with RWF_DONTCACHE.
    pub fn push(&mut self, tag: T, mut rows: Vec<u32>) {
        rows.sort_unstable();
        let rb = self.rb as u64;
        // extents [start, end) of rows merged across gaps, and each row's extent
        let mut sp: Vec<(u64, u64)> = Vec::new();
        let mut at: Vec<u32> = Vec::with_capacity(rows.len());
        let mut rx: Vec<u32> = Vec::with_capacity(rows.len());
        for &r in &rows {
            let s = r as u64 * rb;
            match sp.last_mut() {
                Some(e) if s <= e.1 + self.gap as u64 && s + rb - e.0 <= PIECE as u64 => e.1 = s + rb,
                _ => sp.push((s, s + rb)),
            }
            rx.push(sp.len() as u32 - 1);
            at.push(s as u32); // file offset for now (rebased below)
        }
        let mut ext = Vec::with_capacity(sp.len());
        let mut tot = 0usize;
        for &(a, e) in &sp {
            let len = (e - a) as usize;
            let via = if len >= self.dmin { Via::Direct } else if len >= self.dcmin { Via::DontCache } else { Via::Cached };
            let direct = via == Via::Direct;
            let (a, e) = if direct { (a & !4095, (e + 4095) & !4095) } else { (a, e) };
            if direct {
                tot = (tot + 4095) & !4095;
            }
            ext.push((a, (e - a) as u32, tot as u32, 0u32, via));
            tot += (e - a) as usize;
        }
        for k in 0..rows.len() {
            let x = &ext[rx[k] as usize];
            let s = rows[k] as u64 * rb;
            at[k] = x.2 + (s - x.0) as u32;
        }
        // the smallest free buffer that holds the batch, else a new one
        let need = tot.max(64 << 10).next_power_of_two();
        let buf = match self.free.iter().enumerate().filter(|(_, b)| b.len() >= need).min_by_key(|(_, b)| b.len()) {
            Some((k, _)) => self.free.swap_remove(k),
            None => memmap2::MmapMut::map_anon(need).expect("read buffer"),
        };
        let need = buf.len();
        self.held += need;
        self.reads += ext.len() as u64;
        self.bytes += tot as u64;
        NREADS.fetch_add(ext.len() as u64, Ordering::Relaxed);
        NBYTES.fetch_add(tot as u64, Ordering::Relaxed);
        let left = ext.len();
        self.q.push_back(Batch { tag, rows, at, buf, ext, left });
        self.pump();
    }

    /// Submits queued reads while fewer than `qd` are in flight.
    fn pump(&mut self) {
        let end = self.seq0 + self.q.len() as u64;
        let mut n = 0;
        while self.inflight < self.qd {
            let (seq, i) = if let Some(x) = self.retry.pop() {
                x
            } else {
                while self.next.0 < end && self.next.1 >= self.q[(self.next.0 - self.seq0) as usize].ext.len() {
                    self.next = (self.next.0 + 1, 0);
                }
                if self.next.0 == end {
                    break;
                }
                let x = self.next;
                self.next.1 += 1;
                x
            };
            let b = &self.q[(seq - self.seq0) as usize];
            let (off, len, bo, done, via) = b.ext[i];
            let fd = if via == Via::Direct { self.dfd } else { self.fd };
            let e = io_uring::opcode::Read::new(io_uring::types::Fd(fd), unsafe { b.buf.as_ptr().add((bo + done) as usize) as *mut u8 }, len - done)
                .offset(off + done as u64)
                .rw_flags(if via == Via::DontCache { RWF_DONTCACHE } else { 0 })
                .build()
                .user_data(seq << 24 | i as u64);
            unsafe { self.ring.submission().push(&e) }.expect("the ring holds twice qd entries");
            self.inflight += 1;
            n += 1;
        }
        if n > 0 {
            self.ring.submit().expect("io_uring submit");
        }
    }

    /// Waits for at least one completion, accounts every completion available and refills the queue.
    fn reap(&mut self) {
        self.ring.submit_and_wait(1).expect("io_uring wait");
        let done: Vec<(u64, i32)> = self.ring.completion().map(|c| (c.user_data(), c.result())).collect();
        for (ud, res) in done {
            self.inflight -= 1;
            let (seq, i) = (ud >> 24, (ud & 0xFF_FFFF) as usize);
            let b = &mut self.q[(seq - self.seq0) as usize];
            assert!(res > 0, "read of the full-vector file failed: {}", if res < 0 { std::io::Error::from_raw_os_error(-res).to_string() } else { "end of file".into() });
            b.ext[i].3 += res as u32;
            if b.ext[i].3 < b.ext[i].1 {
                self.retry.push((seq, i));
            } else {
                b.left -= 1;
            }
        }
        self.pump();
    }

    /// The oldest batch, once all of its reads are done.
    pub fn pop(&mut self) -> Option<Batch<T>> {
        if self.q.front()?.left > 0 {
            let t = std::time::Instant::now();
            while self.q.front()?.left > 0 {
                self.reap();
            }
            WAIT_NS.fetch_add(t.elapsed().as_nanos() as u64, Ordering::Relaxed);
        }
        let b = self.q.pop_front().unwrap();
        self.seq0 += 1;
        if self.next.0 < self.seq0 {
            self.next = (self.seq0, 0);
        }
        self.held -= b.buf.len();
        Some(b)
    }

    /// Returns a batch's buffer for reuse; buffers beyond KEEP are freed.
    pub fn recycle(&mut self, b: Batch<T>) {
        if self.free.iter().map(|b| b.len()).sum::<usize>() + b.buf.len() <= KEEP {
            self.free.push(b.buf);
        }
    }
}

impl<T> Drop for Reader<T> {
    fn drop(&mut self) {
        while self.inflight > 0 {
            self.reap();
        }
        let ring = unsafe { std::mem::ManuallyDrop::take(&mut self.ring) };
        THREAD_RING.with(|r| *r.borrow_mut() = Some((ring, std::mem::take(&mut self.free))));
    }
}
