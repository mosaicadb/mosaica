//! The process's own resources: what the C allocator holds, what the kernel says is resident,
//! what the machine has available, and how many files the process may hold open.
//!
//! Shared by the build and the serve path, which are one database
//! ([decision 0091](../../../docs/decisions/0091-build-is-ingest-into-an-empty-database.md)) and
//! therefore one implementation of anything they both do. The build trims at every
//! stage boundary; the server caps the arena count at startup and trims on a growth cadence. Both
//! want the same three primitives, and a second copy of [`resident_bytes`]' parser would be a
//! second answer to "how much of this process is anonymous memory". Both size a term index's
//! merge from [`available_memory`] and [`descriptor_limit`], and the binary raises the descriptor
//! limit for both at startup ([`raise_descriptor_limit`]).
//!
//! # Why an allocator call is a Mosaica concern at all
//!
//! glibc's `malloc` returns a free chunk to its arena, not to the kernel. The main arena releases
//! only from the top of the heap, so a pass that allocates a level of Roaring bitmaps and frees
//! them leaves those pages resident behind whatever was allocated above them; a non-main arena
//! keeps its 64 MiB heaps mapped. Two measurements bound the effect. A whole-corpus build held
//! 34 GB that way
//! (`docs/evidence/memos/2026-09-12-gbif-whole-corpus-build-observations.md` §5). A serving node
//! at rung 6 reached 14.96 GiB of `RssAnon` after a six-principal battery, of which 14.15 GiB was
//! inside the allocator — 373 non-main arena heaps and a 5.28 GiB main arena — against 127 MB the
//! server's own cache accounting could name. On a node whose bundle is larger than memory that
//! retention is not idle: it is the page cache the next request reads from.
//!
//! # The platform gate
//!
//! `malloc_trim` and `mallopt` are glibc extensions. Both are compiled only for a glibc target and
//! are no-ops elsewhere, so a musl or macOS build keeps the same call sites and gets an allocator
//! that already returns pages on its own terms. The descriptor limit is POSIX and is read and
//! raised on every Unix. [`resident_bytes`] and [`available_memory`] read `/proc` and answer zeros
//! or `None` where they cannot, which is also what a non-Linux target gets.

/// Return the allocator's free pages to the kernel.
///
/// `malloc_trim` walks **every** arena's free lists and gives back what is whole pages, so its
/// cost rises with the arena count rather than with the memory it returns. Call it at a boundary —
/// a build stage's end, a measured growth step on the serve path — never per unit of work.
///
/// A no-op off glibc.
pub fn trim_heap() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: asks the allocator to return free pages to the kernel; no pointer is involved.
    unsafe {
        libc::malloc_trim(0)
    };
}

/// Cap the number of arenas glibc will create, returning whether the allocator accepted it.
///
/// An arena is a private heap with its own lock and its own free lists. glibc creates one per
/// contending thread up to `8 × cores` on a 64-bit box and never destroys one, so a process with a
/// wide thread pool accumulates arenas, each retaining its own high-water mark of free chunks. The
/// cap trades allocator contention for that retention: with `n` arenas and more than `n`
/// simultaneously allocating threads, threads share arenas and serialise on their locks.
///
/// Set it before the threads start. It bounds arena *creation* from the moment it is accepted;
/// arenas already created stay.
///
/// A no-op off glibc, where it returns `false`.
pub fn set_arena_max(arenas: usize) -> bool {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    fn apply(arenas: usize) -> bool {
        let value = arenas.min(libc::c_int::MAX as usize) as libc::c_int;
        // SAFETY: `mallopt` takes two integers and returns one; no pointer is involved.
        unsafe { libc::mallopt(libc::M_ARENA_MAX, value) == 1 }
    }
    // Two definitions of one function rather than two branches in one body: the glibc arm names
    // `M_ARENA_MAX`, which does not exist elsewhere, so the arms cannot share a body.
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    fn apply(_arenas: usize) -> bool {
        false
    }
    apply(arenas)
}

/// The process's resident set in bytes, as `/proc/self/status` reports it.
///
/// The split is what separates one cause from another. The bundle's mapped pages are `file` and
/// are reclaimable under pressure; a cache's bitmaps, the allocator's retained chunks and a
/// build's spool buffers are `anon` and are not. A total that climbs because the kernel is holding
/// more of the bundle is not the same event as one that climbs because the process is holding more
/// heap, and only the pair says which happened.
///
/// Zeros where the file cannot be read or a field is absent.
///
/// **`VmHWM` is not here**, so the two peak-RSS readers in `mosaica-bench` and the build's
/// `observer::peak_rss_kib` still parse the file themselves. Adding it would let those consolidate
/// onto this type; they are left alone because a high-water mark is a different question from a
/// current reading — it is reset by `clear_refs` and is meaningless to a cadence — and folding it
/// in would put a field on this struct that every caller here ignores.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Resident {
    /// `VmRSS` — every resident page, anonymous and file-backed together.
    pub total: u64,
    /// `RssAnon` — heap, stacks and anonymous maps.
    pub anon: u64,
    /// `RssFile` — the resident part of every mapped file, the bundle above all.
    pub file: u64,
}

/// Read [`Resident`] from `/proc/self/status`.
///
/// One `read_to_string` of a pseudo-file the kernel formats on demand, then three scans of it.
/// Measured at 20–40 µs on the box the serve cadence was sized against, which is why the cadence
/// that calls it has a time gate in front of it rather than being taken per request.
pub fn resident_bytes() -> Resident {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return Resident::default();
    };
    let field = |name: &str| -> u64 {
        status
            .lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|kib| kib.parse::<u64>().ok())
            .map(|kib| kib * 1024)
            .unwrap_or(0)
    };
    Resident {
        total: field("VmRSS:"),
        anon: field("RssAnon:"),
        file: field("RssFile:"),
    }
}

/// Memory this process could take without reclaiming anything it needs, in bytes: the smaller of
/// `MemAvailable` and this process's cgroup v2 `memory.max`. `None` where `/proc/meminfo` cannot
/// be read.
///
/// **Both sources, because either can be the real bound.** `MemAvailable` is the kernel's estimate
/// of what an allocation could get on the host; `memory.max` is the ceiling this process is killed
/// at, and it charges page cache against itself. A process inside a 4 GiB container on a 48 GiB
/// machine that read only the first would size itself for the machine. A `memory.max` of `max`
/// does not parse, which leaves `MemAvailable` as the answer.
pub fn available_memory() -> Option<u64> {
    let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
    let available = meminfo
        .lines()
        .find(|line| line.starts_with("MemAvailable:"))
        .and_then(|line| line.split_whitespace().nth(1)?.parse::<u64>().ok())
        .map(|kib| kib * 1024)?;
    let limit = std::fs::read_to_string("/proc/self/cgroup")
        .ok()
        .and_then(|cgroup| {
            let path = cgroup
                .lines()
                .next()?
                .split(':')
                .nth(2)?
                .trim_start_matches('/');
            std::fs::read_to_string(format!("/sys/fs/cgroup/{path}/memory.max")).ok()
        })
        .and_then(|max| max.trim().parse::<u64>().ok());
    Some(limit.map_or(available, |limit| limit.min(available)))
}

/// The files this process may hold open at once: its soft `RLIMIT_NOFILE`. 1,024, the soft limit
/// a Linux process ordinarily starts with, where the limit cannot be read or is not a Unix one.
pub fn descriptor_limit() -> u64 {
    #[cfg(unix)]
    {
        let mut limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: `getrlimit` writes one `rlimit` through a pointer to a live local.
        if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } == 0 {
            // `rlim_t` is not `u64` on every Unix.
            #[allow(clippy::unnecessary_cast)]
            return limit.rlim_cur as u64;
        }
    }
    1024
}

/// Raise this process's soft `RLIMIT_NOFILE` to its hard limit, and return the soft limit it then
/// has. The binary calls it once at startup, for `build` and `serve` alike, since a merge's fan-in
/// is bounded by [`descriptor_limit`] as well as by memory. A limit that cannot be raised is left
/// as it is.
pub fn raise_descriptor_limit() -> u64 {
    #[cfg(unix)]
    {
        let mut limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: `getrlimit` and `setrlimit` read and write one `rlimit` through a pointer to a
        // live local.
        unsafe {
            if libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) == 0
                && limit.rlim_cur < limit.rlim_max
            {
                limit.rlim_cur = limit.rlim_max;
                libc::setrlimit(libc::RLIMIT_NOFILE, &limit);
            }
        }
    }
    descriptor_limit()
}

/// A byte count to three significant figures in binary units, such as `18.2 GiB` or `512 B`, as
/// the progress records' readers and the build's reports of the term images and unique indexes
/// print it.
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1023.95 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    let decimals = match (unit, value) {
        (0, _) => 0,
        (_, v) if v >= 99.95 => 0,
        (_, v) if v >= 9.995 => 1,
        _ => 2,
    };
    format!("{value:.decimals$} {}", UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_byte_count_reads_to_three_figures_in_binary_units() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(1023), "1023 B");
        assert_eq!(human_bytes(1024), "1.00 KiB");
        assert_eq!(human_bytes(19_565_460), "18.7 MiB");
        assert_eq!(human_bytes(19_542_626_304), "18.2 GiB");
        assert_eq!(human_bytes(200 << 30), "200 GiB");
    }

    /// The three figures are read from the running process, so the only property a unit test can
    /// hold is the one a mis-parse breaks: this process has anonymous memory, and the total is at
    /// least the sum of its two parts (they are the whole of it on Linux, but a kernel that added
    /// a third class must not make this test fail).
    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "reads /proc/self/status")]
    fn the_resident_figures_are_read_and_are_consistent() {
        let r = resident_bytes();
        assert!(r.anon > 0, "a running test process has anonymous memory");
        assert!(
            r.total >= r.anon + r.file,
            "VmRSS {} is under RssAnon {} + RssFile {}",
            r.total,
            r.anon,
            r.file
        );
    }

    /// Both allocator calls are safe to make from a test process, and the trim's effect is not
    /// assertable — it returns what happens to be free. What this pins is that the call is made
    /// and the process survives it.
    #[test]
    fn the_allocator_calls_are_callable() {
        assert!(
            set_arena_max(4) || !cfg!(all(target_os = "linux", target_env = "gnu")),
            "glibc accepts M_ARENA_MAX"
        );
        trim_heap();
    }
}
