# The memory cap: a cgroup holding the query process (and, for a database, its server), with a limit on memory and no
# swap, so that only the page cache gives way; and the evidence that the cap held.
#
# cgroup v1 (a writable memory controller, as in a container): the process makes a child of its own memory cgroup and
# moves itself there before it allocates anything (enter()), so everything it maps or allocates afterwards is charged
# there. cap() writes memory.limit_in_bytes and memory.memsw.limit_in_bytes (memory plus swap: no swap); the kernel
# refuses a limit below what cannot be reclaimed (EBUSY), which is reported as "does not fit". reclaim() writes
# memory.force_empty, which drops the group's page cache (anonymous memory stays: there is no swap).
# cgroup v2: the process must run in a cgroup of its own, as run.py starts it under
#   systemd-run --user --scope -p MemoryMax=infinity -p MemorySwapMax=0
# (as ../threshold-db/REPORT_CAPPED.md did); cap() writes memory.max and memory.swap.max, reclaim() memory.reclaim. A
# limit below the group's anonymous memory is refused here before it is written (v2 would OOM-kill instead).
# Pages a file left in the page cache before the process entered its group are charged elsewhere and could serve it
# unseen, so the system's files are dropped first (evict_files: fsync, then posix_fadvise DONTNEED).
import os
import time

V1_ROOT = "/sys/fs/cgroup/memory"


def _read(p):
    with open(p) as f:
        return f.read()


def _write(p, v):
    fd = os.open(p, os.O_WRONLY)
    try:
        os.write(fd, str(v).encode())
    finally:
        os.close(fd)


def evict_files(paths):
    """Drops files (or every file under directories) from the page cache, after writing back their dirty pages."""
    n = 0
    for p in paths:
        files = [p] if os.path.isfile(p) else [os.path.join(r, f) for r, _, fs in os.walk(p) for f in fs]
        for f in files:
            try:
                fd = os.open(f, os.O_RDONLY)
            except OSError:
                continue
            try:
                os.fsync(fd)
            except OSError:
                pass
            os.posix_fadvise(fd, 0, 0, os.POSIX_FADV_DONTNEED)
            os.close(fd)
            n += 1
    return n


def io_read_bytes(pids=None):
    """Bytes these processes (default: this one) have caused to be read from storage so far (/proc/PID/io)."""
    tot = 0
    for pid in pids or ["self"]:
        try:
            for l in _read(f"/proc/{pid}/io").splitlines():
                if l.startswith("read_bytes:"):
                    tot += int(l.split()[1])
        except OSError:
            pass
    return tot


class Cgroup:
    """The memory cgroup this process runs in for a capped run."""

    def __init__(self):
        self.v = None
        self.dir = None
        self.created = False
        self.limit = None

    @staticmethod
    def detect():
        lines = _read("/proc/self/cgroup").splitlines()
        for l in lines:
            _, ctl, path = l.split(":", 2)
            if "memory" in ctl.split(",") and os.path.isdir(V1_ROOT):
                return 1, V1_ROOT + path
        for l in lines:
            if l.startswith("0::"):
                d = "/sys/fs/cgroup" + l[3:].strip()
                if os.path.exists(os.path.join(d, "memory.max")):
                    return 2, d
        return None, None

    def enter(self):
        """Moves this process into a cgroup of its own (v1: a new child; v2: the scope run.py started it in)."""
        self.v, own = self.detect()
        if self.v is None:
            raise RuntimeError("no writable memory cgroup (v1 memory controller, or a v2 cgroup with memory.max)")
        if self.v == 1:
            d = os.path.join(own, f"suite-{os.getpid()}")
            os.mkdir(d)
            _write(os.path.join(d, "cgroup.procs"), os.getpid())
            self.dir, self.created = d, True
        else:
            procs = _read(os.path.join(own, "cgroup.procs")).split()
            if procs != [str(os.getpid())]:
                raise RuntimeError(f"cgroup v2: {own} holds other processes ({len(procs)}); run under "
                                   "systemd-run --user --scope -p MemoryMax=infinity -p MemorySwapMax=0 (run.py does)")
            self.dir = own
            try:
                _write(os.path.join(own, "memory.swap.max"), 0)
            except OSError:
                pass
        return self

    def add_pid(self, pid):
        """Moves another process (a database server) into this cgroup."""
        _write(os.path.join(self.dir, "cgroup.procs"), pid)

    def f(self, name):
        return os.path.join(self.dir, name)

    def stats(self):
        """usage, anon and file bytes, and the counters that show the limit at work."""
        st = dict(l.split() for l in _read(self.f("memory.stat")).splitlines())
        if self.v == 1:
            out = dict(usage=int(_read(self.f("memory.usage_in_bytes"))), anon=int(st.get("total_rss", st.get("rss", 0))),
                       file=int(st.get("total_cache", st.get("cache", 0))), failcnt=int(_read(self.f("memory.failcnt"))),
                       max_usage=int(_read(self.f("memory.max_usage_in_bytes"))),
                       limit=int(_read(self.f("memory.limit_in_bytes"))),
                       pgmajfault=int(st.get("total_pgmajfault", 0)))
            oc = dict(l.split() for l in _read(self.f("memory.oom_control")).splitlines())
            out["oom_kill"] = int(oc.get("oom_kill", 0))
            return out
        ev = dict(l.split() for l in _read(self.f("memory.events")).splitlines())
        lim = _read(self.f("memory.max")).strip()
        return dict(usage=int(_read(self.f("memory.current"))), anon=int(st["anon"]), file=int(st["file"]),
                    failcnt=int(ev.get("max", 0)), oom_kill=int(ev.get("oom_kill", 0)),
                    limit=-1 if lim == "max" else int(lim), pgmajfault=int(st.get("pgmajfault", 0)),
                    max_usage=int(_read(self.f("memory.peak"))) if os.path.exists(self.f("memory.peak")) else 0)

    def report(self):
        s = self.stats()
        mb = lambda x: f"{x / 2 ** 20:.1f}"
        return (f"cgroup v{self.v} usage {mb(s['usage'])} MiB (anon {mb(s['anon'])}, file {mb(s['file'])}), peak "
                f"{mb(s['max_usage'])}, limit {'none' if s['limit'] < 0 or s['limit'] >= 1 << 60 else mb(s['limit'])}, "
                f"{'failcnt' if self.v == 1 else 'max events'} {s['failcnt']}, oom_kill {s['oom_kill']}, "
                f"major faults {s['pgmajfault']}")

    def reclaim(self):
        """Drops the group's page cache (anonymous memory stays; there is no swap)."""
        try:
            if self.v == 1:
                _write(self.f("memory.force_empty"), 0)
            else:
                _write(self.f("memory.reclaim"), 1 << 40)
        except OSError:
            pass  # v2: EAGAIN once nothing more can be reclaimed

    def cap(self, nbytes, headroom=0):
        """Sets the limit. Returns None when it holds, else why it does not fit."""
        try:
            import ctypes
            ctypes.CDLL(None).malloc_trim(0)
        except (OSError, AttributeError):
            pass
        self.reclaim()
        anon = self.stats()["anon"]
        if anon + headroom > nbytes:
            return f"anonymous memory {anon / 2 ** 20:.0f} MiB (+{headroom >> 20} MiB headroom) exceeds the cap"
        try:
            if self.v == 1:
                # memsw (memory + swap) cannot be set below memory's limit: lower the order that keeps it valid
                cur = int(_read(self.f("memory.limit_in_bytes")))
                if nbytes < cur:
                    _write(self.f("memory.limit_in_bytes"), nbytes)
                    _write(self.f("memory.memsw.limit_in_bytes"), nbytes)
                else:
                    _write(self.f("memory.memsw.limit_in_bytes"), nbytes)
                    _write(self.f("memory.limit_in_bytes"), nbytes)
            else:
                _write(self.f("memory.swap.max"), 0)
                _write(self.f("memory.max"), nbytes)
        except OSError as e:
            return f"the kernel refused the limit ({e.strerror}): what it holds cannot be reclaimed below it"
        self.limit = nbytes
        self.reclaim()
        # the peak and the counters from here on are the capped run's (v1; v2's memory.peak resets on write in 6.12+)
        for f in (("memory.max_usage_in_bytes", "memory.failcnt") if self.v == 1 else ("memory.peak",)):
            try:
                _write(self.f(f), 0)
            except OSError:
                pass
        return None

    def leave(self):
        """Moves this process back to the parent group and removes a group it created (v1)."""
        if self.v == 1 and self.created:
            parent = os.path.dirname(self.dir)
            try:
                _write(os.path.join(parent, "cgroup.procs"), os.getpid())
                for _ in range(50):
                    try:
                        os.rmdir(self.dir)
                        break
                    except OSError:
                        time.sleep(0.1)
            except OSError:
                pass


def cleanup_stale():
    """Removes the empty groups capped runs left behind (a process the OOM killer stopped cannot leave its own)."""
    v, own = Cgroup.detect()
    if v != 1:
        return 0
    n = 0
    for d in os.listdir(own):
        p = os.path.join(own, d)
        if d.startswith("suite-") and os.path.isdir(p) and not _read(os.path.join(p, "cgroup.procs")).split():
            try:
                os.rmdir(p)
                n += 1
            except OSError:
                pass
    return n
