//! Full vectors read from a file under a memory cap: the row-ordered f16 copy written to a file and mapped, how the
//! mapping is advised, a self-imposed cgroup cap once the build is done, and the counters that show what the cap did.

use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};

/// Before deciding a list of rows from full vectors, ask the kernel to start reading their pages (`--willneed`).
pub static WILLNEED: AtomicBool = AtomicBool::new(false);

/// The stored f16 vectors in index row order: in memory (the build's copy) or a mapped file.
pub enum Rows {
    Mem(Vec<u16>),
    Map { map: memmap2::Mmap, file: std::fs::File },
}

impl std::ops::Deref for Rows {
    type Target = [u16];
    fn deref(&self) -> &[u16] {
        match self {
            Rows::Mem(v) => v,
            Rows::Map { map, .. } => unsafe { std::slice::from_raw_parts(map.as_ptr() as *const u16, map.len() / 2) },
        }
    }
}

impl Rows {
    /// Writes the in-memory rows to `path`, syncs, drops the copy and maps the file with `advice` (normal, random or
    /// sequential).
    pub fn to_file(&mut self, path: &str, advice: &str) {
        if let Rows::Mem(v) = self {
            let bytes = unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 2) };
            let mut f = std::fs::File::create(path).unwrap_or_else(|e| panic!("{path}: {e}"));
            for ch in bytes.chunks(64 << 20) {
                f.write_all(ch).unwrap();
            }
            f.sync_all().unwrap();
            *v = Vec::new();
        }
        let file = std::fs::File::open(path).unwrap();
        let map = unsafe { memmap2::Mmap::map(&file).unwrap() };
        let a = match advice {
            "normal" => libc::MADV_NORMAL,
            "random" => libc::MADV_RANDOM,
            "sequential" => libc::MADV_SEQUENTIAL,
            _ => panic!("--full-advice is normal, random or sequential"),
        };
        unsafe {
            assert_eq!(libc::madvise(map.as_ptr() as *mut libc::c_void, map.len(), a), 0);
        }
        *self = Rows::Map { map, file };
        self.evict();
    }

    /// Drops the mapped rows from this process and from the page cache.
    pub fn evict(&self) {
        if let Rows::Map { map, file } = self {
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

    /// Starts reading the pages of `rows` (rows of `n` f16 each) without waiting, when `--willneed` is on.
    #[inline]
    pub fn willneed(&self, rows: &[u32], n: usize) {
        if !WILLNEED.load(Ordering::Relaxed) {
            return;
        }
        if let Rows::Map { map, .. } = self {
            let base = map.as_ptr() as usize;
            let rb = 2 * n;
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
