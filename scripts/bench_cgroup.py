#!/usr/bin/env python3
"""Run a process under a memory cap and read what the cap saw.

A benchmark that asks how Mosaica behaves as memory shrinks needs a cap the kernel enforces, page
cache included, and the cap's own figures: what it charged, how much of that was page cache, how
many major faults, whether it killed anything. This module gives both on whichever cgroup
hierarchy the host has:

  * cgroup v2 with the memory controller: a child group with `memory.max`, swap off.
  * cgroup v1: a group under the memory hierarchy with `memory.limit_in_bytes`, swappiness 0.
  * Neither writable but a systemd bus: a `systemd-run --scope` (v2 only, as `deployment.py`).

Creating a group needs root or a delegated subtree. A cap of `None` still runs the process in its
own group, so the same figures are read for the unbounded cell.

    with Capped(["mosaica", "serve", ...], cap_bytes=1 << 30, name="cell-3") as capped:
        ...
        capped.stats()  # {"usage": ..., "file": ..., "anon": ..., "majfault": ..., "oom_kills": ...}

    python3 scripts/bench_cgroup.py --cap 1G -- mosaica build ...   # one-shot, prints the stats
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
import uuid
from pathlib import Path
from typing import Iterable, Sequence

ROOT = Path("/sys/fs/cgroup")
PARENT = "mosaica-bench"


def parse_bytes(text: str) -> int | None:
    """`512M`, `2G`, `1073741824` or `unbounded` as bytes; `None` for unbounded."""
    text = text.strip()
    if text.lower() in ("", "none", "unbounded", "max"):
        return None
    units = {"k": 1 << 10, "m": 1 << 20, "g": 1 << 30, "t": 1 << 40}
    if text[-1].lower() in units:
        return int(float(text[:-1]) * units[text[-1].lower()])
    return int(text)


def human(n: int | None) -> str:
    if n is None:
        return "unbounded"
    for unit, size in (("G", 1 << 30), ("M", 1 << 20), ("K", 1 << 10)):
        if n >= size and n % size == 0:
            return f"{n // size}{unit}"
    return str(n)


def hierarchy() -> str:
    """`v2`, `v1`, or `systemd` for the mechanism this host offers."""
    controllers = ROOT / "cgroup.controllers"
    if controllers.exists() and "memory" in controllers.read_text().split():
        return "v2"
    if (ROOT / "memory" / "memory.limit_in_bytes").exists():
        return "v1"
    if shutil.which("systemd-run"):
        return "systemd"
    raise RuntimeError("no cgroup memory controller is mounted and systemd-run is absent")


class Group:
    """One memory cgroup: its limit, its figures and its eviction."""

    def __init__(self, path: Path, kind: str):
        self.path, self.kind = path, kind

    @classmethod
    def create(cls, name: str, cap_bytes: int | None) -> "Group":
        kind = hierarchy()
        if kind == "systemd":
            raise RuntimeError("Group.create needs a writable cgroup; use Capped, which falls back")
        base = (ROOT if kind == "v2" else ROOT / "memory") / PARENT
        base.mkdir(exist_ok=True)
        if kind == "v2":
            # A v2 child can only use controllers its parent passes down.
            try:
                (base / "cgroup.subtree_control").write_text("+memory")
            except OSError:
                pass
        path = base / name
        path.mkdir()
        group = cls(path, kind)
        group.set_cap(cap_bytes)
        if kind == "v2":
            group._write("memory.swap.max", "0")
        else:
            group._write("memory.swappiness", "0")
        return group

    def _write(self, file: str, value: str) -> bool:
        try:
            (self.path / file).write_text(value)
            return True
        except OSError:
            return False

    def _read(self, file: str) -> str | None:
        try:
            return (self.path / file).read_text()
        except OSError:
            return None

    def set_cap(self, cap_bytes: int | None) -> None:
        if self.kind == "v2":
            self._write("memory.max", "max" if cap_bytes is None else str(cap_bytes))
        else:
            self._write("memory.limit_in_bytes", "-1" if cap_bytes is None else str(cap_bytes))

    def cap(self) -> int | None:
        raw = (self._read("memory.max") if self.kind == "v2" else self._read("memory.limit_in_bytes"))
        if raw is None or raw.strip() == "max" or int(raw) >= 1 << 62:
            return None
        return int(raw)

    def add(self, pid: int) -> None:
        (self.path / "cgroup.procs").write_text(str(pid))

    def stats(self) -> dict:
        """What the group holds now and has suffered so far."""
        stat = {}
        for line in (self._read("memory.stat") or "").splitlines():
            key, _, value = line.partition(" ")
            if value.strip().isdigit():
                stat[key] = int(value)
        if self.kind == "v2":
            events = {}
            for line in (self._read("memory.events") or "").splitlines():
                key, _, value = line.partition(" ")
                events[key] = int(value)
            usage = int((self._read("memory.current") or "0").strip())
            peak = self._read("memory.peak")
            return {
                "usage": usage,
                "peak": int(peak) if peak else None,
                "anon": stat.get("anon", 0),
                "file": stat.get("file", 0),
                "file_mapped": stat.get("file_mapped", 0),
                "majfault": stat.get("pgmajfault", 0),
                "oom_kills": events.get("oom_kill", 0),
                "limit_hits": events.get("max", 0),
            }
        oom = {}
        for line in (self._read("memory.oom_control") or "").splitlines():
            key, _, value = line.partition(" ")
            oom[key] = int(value)
        return {
            "usage": int((self._read("memory.usage_in_bytes") or "0").strip()),
            "peak": int((self._read("memory.max_usage_in_bytes") or "0").strip()),
            "anon": stat.get("total_rss", stat.get("rss", 0)),
            "file": stat.get("total_cache", stat.get("cache", 0)),
            "file_mapped": stat.get("total_mapped_file", stat.get("mapped_file", 0)),
            "majfault": stat.get("total_pgmajfault", stat.get("pgmajfault", 0)),
            "oom_kills": oom.get("oom_kill", 0),
            "limit_hits": int((self._read("memory.failcnt") or "0").strip()),
        }

    def reset_peak(self) -> None:
        if self.kind == "v1":
            self._write("memory.max_usage_in_bytes", "0")

    def evict(self, files: Iterable[Path] = ()) -> None:
        """Push the group's page cache out: `DONTNEED` over `files`, then ask the kernel to
        reclaim what the group charges. Neither is guaranteed; a cold cell should check that its
        major faults rose."""
        drop_page_cache(files)
        file_bytes = self.stats()["file"]
        if self.kind == "v2":
            self._write("memory.reclaim", str(max(file_bytes, 1 << 20)))
        elif file_bytes:
            # v1 has no reclaim knob: lower the limit to what the group holds besides its page
            # cache, which reclaims the cache first, then put the limit back.
            cap = self.cap()
            floor = max(self.stats()["usage"] - file_bytes, 16 << 20)
            for target in (floor, floor * 2):
                if self._write("memory.limit_in_bytes", str(target)):
                    break
            self.set_cap(cap)

    def remove(self) -> None:
        for _ in range(50):
            try:
                self.path.rmdir()
                return
            except OSError:
                time.sleep(0.1)


def drop_page_cache(files: Iterable[Path]) -> None:
    """Drop `files` from the page cache, whichever group it is charged to, so that the next group
    to read them is the one charged."""
    for path in files:
        try:
            fd = os.open(path, os.O_RDONLY)
        except OSError:
            continue
        try:
            os.posix_fadvise(fd, 0, 0, os.POSIX_FADV_DONTNEED)
        except OSError:
            pass
        finally:
            os.close(fd)


def files_under(roots: Sequence[Path]) -> list[Path]:
    out = []
    for root in roots:
        root = Path(root)
        if root.is_file():
            out.append(root)
        elif root.is_dir():
            out.extend(p for p in root.rglob("*") if p.is_file())
    return out


class Capped:
    """`argv` started inside its own memory group of `cap_bytes`. The child joins the group before
    it executes, so nothing it allocates escapes the cap."""

    def __init__(
        self,
        argv: Sequence[str],
        cap_bytes: int | None,
        name: str | None = None,
        cpus: str | None = None,
        **popen,
    ):
        self.argv, self.cap_bytes, self.cpus, self.popen = list(argv), cap_bytes, cpus, popen
        self.name = name or f"cell-{uuid.uuid4().hex[:8]}"
        self.group: Group | None = None
        self.proc: subprocess.Popen | None = None

    def start(self) -> "Capped":
        argv = self.argv
        if self.cpus:
            argv = ["taskset", "-c", self.cpus, *argv]
        if hierarchy() == "systemd":
            props = ["-p", "MemorySwapMax=0"]
            if self.cap_bytes is not None:
                props += ["-p", f"MemoryMax={self.cap_bytes}"]
            self.proc = subprocess.Popen(
                ["systemd-run", "--user", "--scope", "--quiet", *props, *argv], **self.popen
            )
            return self
        self.group = Group.create(self.name, self.cap_bytes)
        procs = str(self.group.path / "cgroup.procs")

        def join() -> None:
            with open(procs, "w") as f:
                f.write(str(os.getpid()))

        self.proc = subprocess.Popen(argv, preexec_fn=join, **self.popen)
        return self

    def stats(self) -> dict:
        return self.group.stats() if self.group else {}

    def oom_killed(self) -> bool:
        return bool(self.group and self.group.stats()["oom_kills"])

    def stop(self, timeout: float = 120.0) -> int | None:
        if self.proc is not None and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait()
        return self.proc.returncode if self.proc else None

    def close(self) -> None:
        self.stop()
        if self.group is not None:
            self.group.remove()
            self.group = None

    def __enter__(self) -> "Capped":
        return self.start()

    def __exit__(self, *exc) -> None:
        self.close()


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--cap", default="unbounded", help="512M, 2G, bytes, or unbounded")
    ap.add_argument("--cpus", help="a taskset CPU list, such as 1-3")
    ap.add_argument("--stats-out", type=Path, help="write the group's final figures here as JSON")
    ap.add_argument("argv", nargs=argparse.REMAINDER)
    args = ap.parse_args()
    argv = args.argv[1:] if args.argv[:1] == ["--"] else args.argv
    if not argv:
        ap.error("give the command to run after --")
    capped = Capped(argv, parse_bytes(args.cap), cpus=args.cpus).start()
    try:
        code = capped.proc.wait()
        stats = {"cap": capped.cap_bytes, "exit": code, **capped.stats()}
    finally:
        capped.close()
    text = json.dumps(stats)
    if args.stats_out:
        args.stats_out.write_text(text + "\n")
    print(text, file=sys.stderr)
    return code if code >= 0 else 128 - code


if __name__ == "__main__":
    sys.exit(main())
