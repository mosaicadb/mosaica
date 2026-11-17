# The disk check before a capped run: what a storage read costs here, and whether the system's index files are really
# read from storage or from a cache below the guest.
# usage: python3 -I diskcheck.py [INDEX_FILE ...]      (prints the check; run.py runs it before each system's caps)
#
# A virtual machine's disk is often cached by its host (and WSL2's virtual disk by Windows), so O_DIRECT reads of a
# file written or read recently never touch the device: on the cloud VM these runs used, 4 KiB reads of a freshly
# written file took 33 us at the median, and of a large file nobody had read 650 us. Dropping a file from the guest's
# page cache does not drop it from the host's. So the check times, with O_DIRECT (past the guest's cache):
#   (a) random 4 KiB reads at queue depth 1, and a QD1 sequential read in 1 MiB requests, of a large file this run has
#       not read and that was not written recently: by default, among up to 64 existing files of 64 MB or more outside
#       the data, the repository, the temporary directories and what any process maps, the one whose short probe is
#       slowest (SUITE_COLD_FILE names one instead). Caveat: the host may still hold parts of it (another session may
#       have read it), and each check reads a few MB of it at random offsets, which the host may then keep; so the
#       figure is a lower bound on the device's cost;
#   (b) the same reads of the system's index file (its largest data file), after dropping it from the guest's cache.
# If (b) is more than 3 times faster than (a), the run is marked "host-cached: timings optimistic": the system's reads
# came from a cache under the guest, and a cold device would be slower.
import mmap
import os
import random
import sys
import time

SKIP = ("/proc", "/sys", "/dev", "/run", "/tmp", "/var/tmp", "/home", "/snap")


def _reads(path, n, seed, seq=True):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECT)
    try:
        size = os.fstat(fd).st_size
        pages = size // 4096 - 1
        rng = random.Random(seed)
        offs = rng.sample(range(pages), min(n, pages))
        buf = mmap.mmap(-1, 4096)
        ts = []
        for o in offs:
            t = time.perf_counter()
            os.preadv(fd, [buf], o * 4096)
            ts.append(time.perf_counter() - t)
        ts.sort()
        if not seq:
            return dict(p50_us=ts[len(ts) // 2] * 1e6)
        # QD1 sequential: 64 MiB (or the file) in 1 MiB requests from a random aligned offset
        big = mmap.mmap(-1, 1 << 20)
        span = min(64 << 20, (size // (1 << 20) - 1) << 20)
        start = rng.randrange(0, max(1, size // (1 << 20) - span // (1 << 20))) << 20 if span > 0 else 0
        t = time.perf_counter()
        got = 0
        for o in range(start, start + span, 1 << 20):
            got += os.preadv(fd, [big], o)
        dt = time.perf_counter() - t
        return dict(size_mb=round(size / 1e6), p50_us=round(ts[len(ts) // 2] * 1e6, 1),
                    p99_us=round(ts[int(len(ts) * 0.99)] * 1e6, 1), seq_mbs=round(got / 1e6 / dt) if dt > 0 else 0)
    finally:
        os.close(fd)


def _mapped():
    """Files any process maps now (a running program's own binary is in every cache)."""
    out = set()
    for pid in os.listdir("/proc"):
        if pid.isdigit():
            try:
                for l in open(f"/proc/{pid}/maps"):
                    f = l.split()
                    if len(f) >= 6 and f[5].startswith("/"):
                        out.add(f[5])
            except OSError:
                pass
    return out


def cold_file(exclude=(), probes=64):
    """A large file this session has likely not read: among the files of 64 MB or more outside `exclude`, the
    temporary directories and what any process maps now, the one whose short probe of random O_DIRECT reads is the
    slowest (access times are no guide: images often set them all alike). SUITE_COLD_FILE names one instead."""
    if os.environ.get("SUITE_COLD_FILE"):
        return os.environ["SUITE_COLD_FILE"]
    mapped = _mapped()
    cands = []
    for root in ("/usr", "/opt", "/var", "/root", "/srv", "/lib"):
        for r, dirs, files in os.walk(root):
            if r.startswith(SKIP) or any(r.startswith(e) for e in exclude):
                dirs[:] = []
                continue
            for f in files:
                p = os.path.join(r, f)
                try:
                    if os.path.isfile(p) and not os.path.islink(p) and os.path.getsize(p) >= 64 << 20 \
                            and os.path.realpath(p) not in mapped:
                        cands.append(p)
                except OSError:
                    pass
    random.Random(time.time()).shuffle(cands)
    best = None
    for p in cands[:probes]:
        try:
            t = _reads(p, 40, int(time.time() * 1e6), seq=False)["p50_us"]
        except OSError:
            continue
        if best is None or t > best[0]:
            best = (t, p)
    return best[1] if best else None


def check(index_files, exclude=(), n=500):
    """The check: the never-read file's figures, each index file's, and whether the latter looks host-cached."""
    seed = int(time.time() * 1e6)
    out = {}
    cf = cold_file(exclude)
    if cf:
        a = _reads(cf, n, seed)
        out.update({f"cold_{k}": v for k, v in a.items()}, cold_file=cf)
    files = [p for p in index_files if p and os.path.isfile(p)]
    if files:
        p = max(files, key=os.path.getsize)
        fd = os.open(p, os.O_RDONLY)
        os.posix_fadvise(fd, 0, 0, os.POSIX_FADV_DONTNEED)
        os.close(fd)
        b = _reads(p, n, seed + 1)
        out.update({f"index_{k}": v for k, v in b.items()}, index_file=p)
        if cf:
            out["ratio"] = round(a["p50_us"] / max(b["p50_us"], 1e-3), 1)
            out["host_cached"] = bool(out["ratio"] > 3)
    return out


def largest_files(paths):
    """The data files under the given paths (files or directories)."""
    out = []
    for p in paths:
        if os.path.isfile(p):
            out.append(p)
        elif os.path.isdir(p):
            out += [os.path.join(r, f) for r, _, fs in os.walk(p) for f in fs]
    return out


if __name__ == "__main__":
    print(check(sys.argv[1:]))
