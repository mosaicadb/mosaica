"""What every adapter provides, and the measurements they share."""

from __future__ import annotations

import functools
import os
import re
import subprocess
import time
from abc import ABC, abstractmethod
from pathlib import Path

from frame import Camera, Filter, Frame


class Adapter(ABC):
    """One system: build it from a tier, serve it, and answer frames and tables as the viewer in
    `viewer.json`.

    `frame` and `tables` return the decoded answer and the seconds from the first byte sent to the
    last byte received, over every request the system needs for it. Decoding is outside that time.
    The viewer's restriction is the system's own access control, set up by the adapter; no query an
    adapter sends names the viewer's terms. `access` says how, for the record.
    """

    name: str
    access: str

    def __init__(self, tier: Path, work: Path, nice: int = 0):
        self.tier = tier.resolve()
        #: The niceness of the build, applied the same way to every system; servers run at 0.
        self.nice = nice
        self.work = work.resolve()
        self.work.mkdir(parents=True, exist_ok=True)

    @abstractmethod
    def version(self) -> dict: ...

    @abstractmethod
    def build(self) -> dict:
        """Build from nothing. Returns `build_s`, `peak_rss_bytes`, `cgroup_peak_bytes` and
        `disk_bytes`, and anything else worth keeping."""

    @abstractmethod
    def start(self) -> dict:
        """Start the server and open the viewer's connection or session. Returns its phases in
        seconds, such as `ready_s`, for the session-start record."""

    @abstractmethod
    def stop(self) -> None: ...

    @abstractmethod
    def evict(self) -> None:
        """Remove this system's own caches and its files' pages from the page cache, with the
        server stopped. Never the whole machine's page cache."""

    @abstractmethod
    def frame(self, camera: Camera, filt: Filter | None) -> tuple[Frame, float]: ...

    @abstractmethod
    def tables(self, camera: Camera, filt: Filter | None) -> tuple[dict, float]:
        """The aggregation workload's tables in the shape `Oracle.tables` gives."""

    @abstractmethod
    def serve_memory(self) -> dict:
        """`peak_rss_bytes` and `cgroup_peak_bytes` of the running server since it started."""

    @abstractmethod
    def config(self) -> dict:
        """The settings this system ran with, as recorded in the result."""

    def remove(self) -> None:
        """Remove anything `start` left behind that `stop` does not, such as a container."""

    def restart(self, cold: bool) -> dict:
        self.stop()
        if cold:
            self.evict()
        return self.start()


def evict_files(root: Path) -> int:
    """Ask the kernel to drop the cached pages of every file under `root`. Returns the files
    touched. A page still mapped by a live process is kept, so call this with the server stopped."""
    n = 0
    for dirpath, _, files in os.walk(root):
        for name in files:
            path = os.path.join(dirpath, name)
            try:
                fd = os.open(path, os.O_RDONLY)
            except OSError:
                continue
            try:
                os.posix_fadvise(fd, 0, 0, os.POSIX_FADV_DONTNEED)
                n += 1
            finally:
                os.close(fd)
    return n


def du_bytes(path: Path) -> int:
    out = subprocess.run(["du", "-sb", str(path)], capture_output=True, text=True, check=True)
    return int(out.stdout.split()[0])


@functools.cache
def user_scopes() -> bool:
    """Whether a systemd user manager can put a process in a scope of its own."""
    return subprocess.run(["systemd-run", "--user", "--scope", "-q", "true"],
                          capture_output=True).returncode == 0


def scoped(unit: str, memory_max: str = "16G") -> list[str]:
    """The prefix that runs a command in its own scope `unit` under `memory_max`, with no swap,
    so its cgroup's `memory.peak` is its own. Empty where there is no user manager: the command
    then runs uncapped and its cgroup figures are not read."""
    if not user_scopes():
        return []
    return ["systemd-run", "--user", "--scope", "-q", "--unit", unit,
            "-p", f"MemoryMax={memory_max}", "-p", "MemorySwapMax=0"]


def scope_dir(unit: str) -> Path:
    return Path(f"/sys/fs/cgroup/user.slice/user-{os.getuid()}.slice/user@{os.getuid()}.service/app.slice/{unit}.scope")


def read_int(path: Path) -> int | None:
    try:
        return int(path.read_text().strip())
    except (OSError, ValueError):
        return None


def vm_hwm(pid: int) -> int | None:
    try:
        m = re.search(r"VmHWM:\s+(\d+) kB", Path(f"/proc/{pid}/status").read_text())
    except OSError:
        return None
    return int(m[1]) * 1024 if m else None


def wait_for(check, timeout: float, what: str) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if check():
            return
        time.sleep(0.1)
    raise TimeoutError(f"{what} did not happen within {timeout} s")
