"""What every adapter provides, and the measurements they share."""

from __future__ import annotations

import functools
import os
import re
import socket
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

    def __init__(self, tier: Path, work: Path, nice: int = 0, caches: str = "compute"):
        if caches not in ("compute", "defaults"):
            raise ValueError(f"caches is compute or defaults, not {caches}")
        #: `compute` turns off every cache kept between requests; `defaults` leaves the system's own.
        self.caches = caches
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
    def start(self) -> None:
        """Start the server and return once its data is loaded and it answers a request that is
        not the viewer's; everything the viewer's first use costs falls after `connect`."""

    @abstractmethod
    def connect(self) -> None:
        """Open the viewer's session or connections on a started server."""

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

    def restart(self, cold: bool) -> None:
        self.stop()
        if cold:
            self.evict()
        self.start()
        self.connect()


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


def fsync_tree(root: Path, skip: set[Path] = frozenset()) -> int:
    """Flush every file and directory under `root` to disc, leaving out the trees in `skip`.
    Returns the entries flushed; a file that cannot be opened is an error."""
    n = 0
    for dirpath, dirs, files in os.walk(root):
        dirs[:] = [d for d in dirs if Path(dirpath, d) not in skip]
        for path in [*(os.path.join(dirpath, f) for f in files), dirpath]:
            fd = os.open(path, os.O_RDONLY)
            try:
                os.fsync(fd)
                n += 1
            finally:
                os.close(fd)
    return n


def free_ports(n: int) -> list[int]:
    """`n` distinct ports on 127.0.0.1 that nothing listens on, chosen by the kernel, so that runs
    side by side do not collide."""
    socks = [socket.socket() for _ in range(n)]
    try:
        for s in socks:
            s.bind(("127.0.0.1", 0))
        return [s.getsockname()[1] for s in socks]
    finally:
        for s in socks:
            s.close()


def du_bytes(path: Path) -> int:
    out = subprocess.run(["du", "-sb", str(path)], capture_output=True, text=True, check=True)
    return int(out.stdout.split()[0])


#: Every server and build runs in a container under this memory, with no swap beyond it: whole
#: gibibytes, from `BENCH_MEMORY`.
MEMORY = os.environ.get("BENCH_MEMORY", "24g")
MEMORY_GIB = int(MEMORY.rstrip("g"))
#: Host binaries mounted into every container, so a server or build can run under `nice`, and a
#: build container can stay alive for `docker exec`, in an image without them. Each links only to
#: the C library.
TOOLS = {"/usr/bin/nice": "/opt/bench/nice", "/usr/bin/sleep": "/opt/bench/sleep"}


class Container:
    """One named Docker container, run as this user, capped at `MEMORY` with no swap, so its
    cgroup's `memory.peak` is its own and every file it writes belongs to this user."""

    def __init__(self, name: str):
        self.name = name

    def create(self, image: str, *, entrypoint: str, args: tuple[str, ...] = (),
               mounts: dict | None = None, ports: dict | None = None, env: dict | None = None,
               nice: int = 0, shm: str | None = None) -> None:
        """Create the container, replacing one of the same name. `mounts` maps a host path to a
        container path (suffix `:ro` for read-only), `ports` a host port on 127.0.0.1 to a
        container port, and `shm` sizes `/dev/shm` where Docker's 64 MB is too small. The entry
        point runs at niceness `nice`."""
        mounts, ports, env = mounts or {}, ports or {}, env or {}
        check_tools(image)
        self.remove()
        subprocess.run([
            "docker", "create", "--name", self.name, "--user", f"{os.getuid()}:{os.getgid()}",
            "--ulimit", "nofile=262144:262144", "--memory", MEMORY, "--memory-swap", MEMORY,
            *(a for host, inside in TOOLS.items() for a in ("-v", f"{host}:{inside}:ro")),
            *(a for host, inside in mounts.items() for a in ("-v", f"{host}:{inside}")),
            *(a for host, inside in ports.items() for a in ("-p", f"127.0.0.1:{host}:{inside}")),
            *(a for k, v in env.items() for a in ("-e", f"{k}={v}")),
            *(("--shm-size", shm) if shm else ()),
            "--entrypoint", TOOLS["/usr/bin/nice"], image, "-n", str(nice), entrypoint, *args,
        ], check=True, capture_output=True)

    def start(self) -> None:
        subprocess.run(["docker", "start", self.name], check=True, capture_output=True)

    def exec(self, args: list[str], log: Path) -> int:
        """Run `args` in the running container, its output to `log`. Returns its exit status."""
        with log.open("wb") as f:
            return subprocess.run(["docker", "exec", self.name, *args], stdout=f, stderr=f).returncode

    def stop(self) -> None:
        subprocess.run(["docker", "stop", "-t", "60", self.name], capture_output=True)

    def remove(self) -> None:
        """Remove the container and the anonymous volumes its image declares."""
        subprocess.run(["docker", "rm", "-f", "-v", self.name], capture_output=True)

    def cgroup(self) -> Path:
        cid = subprocess.run(["docker", "inspect", "-f", "{{.Id}}", self.name],
                             capture_output=True, text=True, check=True).stdout.strip()
        scope = Path(f"/sys/fs/cgroup/system.slice/docker-{cid}.scope")
        if not (scope / "memory.peak").exists():
            raise RuntimeError(f"container {self.name} has no cgroup at {scope}; run Docker with "
                               f"the systemd cgroup driver on cgroup v2, or the container is not "
                               f"running")
        return scope

    def memory(self) -> dict:
        """The running container's largest process peak RSS and its cgroup's `memory.peak`."""
        scope = self.cgroup()
        pids = [int(p) for p in (scope / "cgroup.procs").read_text().split()]
        rss = [r for r in (vm_hwm(p) for p in pids) if r is not None]
        return {"peak_rss_bytes": max(rss) if rss else None,
                "cgroup_peak_bytes": read_int(scope / "memory.peak")}


@functools.cache
def check_tools(image: str) -> None:
    """Refuse an image in which the mounted host tools do not run."""
    r = subprocess.run(["docker", "run", "--rm", *(a for h, i in TOOLS.items() for a in ("-v", f"{h}:{i}:ro")),
                        "--entrypoint", TOOLS["/usr/bin/nice"], image, "-n", "0",
                        TOOLS["/usr/bin/sleep"], "0"], capture_output=True, text=True)
    if r.returncode:
        raise RuntimeError(f"the host's nice and sleep do not run in {image} "
                           f"({r.stderr.strip()[:300]}); build the image on a C library the host's "
                           f"binaries can load, or mount static ones in TOOLS")


def docker_info() -> dict | None:
    fields = ("ServerVersion", "CgroupDriver", "CgroupVersion", "OperatingSystem", "KernelVersion")
    info = subprocess.run(["docker", "info", "--format", "|".join("{{.%s}}" % f for f in fields)],
                          capture_output=True, text=True)
    return dict(zip(fields, info.stdout.strip().split("|"))) if info.returncode == 0 else None


def image_id(image: str) -> str:
    return subprocess.run(["docker", "image", "inspect", "-f", "{{.Id}}", image],
                          capture_output=True, text=True, check=True).stdout.strip()


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
        time.sleep(0.01)
    raise TimeoutError(f"{what} did not happen within {timeout} s")
