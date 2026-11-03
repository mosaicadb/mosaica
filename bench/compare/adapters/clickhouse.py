"""ClickHouse: a pinned server image, one MergeTree table sorted by Morton code, and the frame in SQL.

The build reads the tier's `x`, `y`, `band` and `num` (never the oracle's `qx` and `qy`),
quantises the coordinates in SQL by the frame's rule, and stores `m = mortonEncode(qx, qy)` as the
sorting key, so that a tile is a contiguous range of `m` as it is in Mosaica. `OPTIMIZE TABLE ...
FINAL` merges the table to one part and is counted in the build.

A frame is two queries sent at once over HTTP: the count grid (`GROUP BY` the depth-`grid_depth`
prefix of `m` over the camera's tile ranges), and every position in the tiles whose count is at
most K (the same ranges, restricted in the same statement to the sparse tiles). The query cache
is off, as it is by default. The server's own log tables are turned off so their writes stay out
of the timings.
"""

from __future__ import annotations

import io
import os
import subprocess
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import pyarrow.ipc as ipc
import requests

from frame import EXTENT, K, Camera, Filter, Frame, deinterleave, grid_depth, interleave, points_by_tile

from .base import Adapter, du_bytes, evict_files, read_int, vm_hwm, wait_for

IMAGE = "clickhouse/clickhouse-server:26.8.34.13"
IMAGE_DIGEST = "sha256:8b28eb09479c36c4533eec7a6268917d04ba3a6fd90e3f4205f70e761b6d7779"
PORT = 18123
NO_LOGS = """<clickhouse>
  <query_log remove="1"/><query_thread_log remove="1"/><query_views_log remove="1"/>
  <trace_log remove="1"/><metric_log remove="1"/><asynchronous_metric_log remove="1"/>
  <part_log remove="1"/><text_log remove="1"/><processors_profile_log remove="1"/>
  <latency_log remove="1"/><error_log remove="1"/><query_metric_log remove="1"/>
  <opentelemetry_span_log remove="1"/><session_log remove="1"/>
</clickhouse>
"""
DDL = ("CREATE TABLE points (m UInt64, qx UInt32, qy UInt32, band LowCardinality(String), num UInt16) "
       "ENGINE = MergeTree ORDER BY m")


def quantise(expr: str, lo: float, hi: float) -> str:
    return (f"toUInt32(least(greatest(floor(({expr} - {lo!r}) / ({hi!r} - {lo!r}) * 4294967296.), 0.), "
            f"4294967295.))")


def tile_ranges(tiles: list[tuple[int, int]], z: int) -> str:
    if z == 0:
        return "1"
    shift = 64 - 2 * z
    out = []
    for tx, ty in tiles:
        p = interleave(tx, ty)
        out.append(f"m BETWEEN {p << shift} AND {((p + 1) << shift) - 1}")
    return "(" + " OR ".join(out) + ")"


def filter_sql(filt: Filter | None) -> str:
    terms = []
    for leaf in (filt.leaves if filt else []):
        if leaf["op"] == "eq":
            terms.append(f"{leaf['col']} = '{leaf['value']}'")
        elif leaf["op"] == "lt":
            terms.append(f"{leaf['col']} < {int(leaf['value'])}")
        elif leaf["op"] == "bbox":
            x0, y0, x1, y1 = (float(v) for v in leaf["value"])
            terms.append(f"qx BETWEEN {quantise(repr(x0), EXTENT[0], EXTENT[1])} AND "
                         f"{quantise(repr(x1), EXTENT[0], EXTENT[1])}")
            terms.append(f"qy BETWEEN {quantise(repr(y0), EXTENT[2], EXTENT[3])} AND "
                         f"{quantise(repr(y1), EXTENT[2], EXTENT[3])}")
        else:
            raise ValueError(leaf)
    return "".join(f" AND {t}" for t in terms)


class ClickHouse(Adapter):
    name = "clickhouse"

    def __init__(self, tier: Path, work: Path, nice: int = 0):
        super().__init__(tier, work, nice)
        self.container = f"bench-compare-ch-{os.getpid()}"
        self.data = self.work / "ch-data"
        self.pool = ThreadPoolExecutor(2)
        self.sessions = [requests.Session(), requests.Session()]

    # -- the container ---------------------------------------------------------------------------

    def _create(self, nice: int = 0) -> None:
        """Create the container. With `nice`, the server runs at that niceness: the build's
        container, which is removed once the table is built, the serving one being created at 0."""
        subprocess.run(["docker", "rm", "-f", self.container], capture_output=True)
        (self.work / "config.d").mkdir(exist_ok=True)
        (self.work / "config.d/no-logs.xml").write_text(NO_LOGS)
        # Made here, not by docker, so they belong to this user and a rebuild can delete them.
        for d in ("ch-data/user_files/tiers", "ch-logs"):
            (self.work / d).mkdir(parents=True, exist_ok=True)
        subprocess.run([
            "docker", "create", "--name", self.container, "--user", f"{os.getuid()}:{os.getgid()}",
            "--ulimit", "nofile=262144:262144", "--memory", "16g", "--memory-swap", "16g",
            "-p", f"127.0.0.1:{PORT}:8123", "-e", "CLICKHOUSE_SKIP_USER_SETUP=1",
            "-v", f"{self.data}:/var/lib/clickhouse",
            "-v", f"{self.work / 'ch-logs'}:/var/log/clickhouse-server",
            "-v", f"{self.work / 'config.d/no-logs.xml'}:/etc/clickhouse-server/config.d/no-logs.xml:ro",
            "-v", f"{self.tier.parent}:/var/lib/clickhouse/user_files/tiers:ro",
            *(["--entrypoint", "nice"] if nice else []),
            f"{IMAGE.split(':')[0]}@{IMAGE_DIGEST}",
            *(["-n", str(nice), "/entrypoint.sh"] if nice else []),
        ], check=True, capture_output=True)

    def _scope(self) -> Path:
        cid = subprocess.run(["docker", "inspect", "-f", "{{.Id}}", self.container],
                             capture_output=True, text=True, check=True).stdout.strip()
        return Path(f"/sys/fs/cgroup/system.slice/docker-{cid}.scope")

    def start(self) -> None:
        subprocess.run(["docker", "start", self.container], check=True, capture_output=True)
        wait_for(self._ping, 120, "ClickHouse answering /ping")

    def _ping(self) -> bool:
        try:
            return requests.get(f"http://127.0.0.1:{PORT}/ping", timeout=1).ok
        except requests.RequestException:
            return False

    def stop(self) -> None:
        subprocess.run(["docker", "stop", "-t", "60", self.container], capture_output=True)

    def remove(self) -> None:
        subprocess.run(["docker", "rm", "-f", self.container], capture_output=True)

    def evict(self) -> None:
        evict_files(self.data)

    def serve_memory(self) -> dict:
        """None for a figure that cannot be read, such as from a container that has gone."""
        try:
            scope = self._scope()
            pids = [int(p) for p in (scope / "cgroup.procs").read_text().split()]
        except (OSError, subprocess.CalledProcessError):
            return {"peak_rss_bytes": None, "cgroup_peak_bytes": None}
        rss = [r for r in (vm_hwm(p) for p in pids) if r is not None]
        return {"peak_rss_bytes": max(rss) if rss else None,
                "cgroup_peak_bytes": read_int(scope / "memory.peak")}

    # -- SQL ---------------------------------------------------------------------------------------

    def sql(self, query: str, session: int = 0, timeout: float = 3600) -> bytes:
        r = self.sessions[session].post(f"http://127.0.0.1:{PORT}/", params={"use_query_cache": 0},
                                        data=query.encode(), timeout=timeout)
        if r.status_code != 200:
            raise RuntimeError(f"ClickHouse {r.status_code}: {r.text[:800]}")
        return r.content

    def version(self) -> dict:
        fields = ("ServerVersion", "CgroupDriver", "CgroupVersion", "OperatingSystem", "KernelVersion")
        info = subprocess.run(["docker", "info", "--format", "|".join("{{.%s}}" % f for f in fields)],
                              capture_output=True, text=True)
        return {"version": self.sql("SELECT version()").decode().strip(), "image": IMAGE,
                "digest": IMAGE_DIGEST,
                "docker": dict(zip(fields, info.stdout.strip().split("|"))) if info.returncode == 0 else None}

    def config(self) -> dict:
        settings = self.sql("SELECT name, value FROM system.settings WHERE name IN "
                            "('max_threads', 'use_query_cache', 'max_memory_usage') FORMAT TSV").decode()
        return {"ddl": DDL, "optimize_final": True, "system_logs": "off",
                "container_memory": "16g", "settings": dict(l.split("\t") for l in settings.splitlines())}

    def build(self) -> dict:
        self.remove()
        subprocess.run(["rm", "-rf", str(self.data), str(self.work / "ch-logs")], check=True)
        self._create(self.nice)
        self.start()
        if self.sql("SELECT mortonEncode(toUInt32(1), toUInt32(0))").strip() != b"1":
            raise RuntimeError("mortonEncode does not put its first argument on the even bits")
        src = (f"file('tiers/{self.tier.name}', Parquet, "
               f"'x Float32, y Float32, band String, num UInt16')")
        t0 = time.monotonic()
        self.sql(DDL)
        self.sql(f"INSERT INTO points SELECT mortonEncode(qx, qy) AS m, qx, qy, band, num FROM ("
                 f"SELECT {quantise('toFloat64(x)', EXTENT[0], EXTENT[1])} AS qx, "
                 f"{quantise('toFloat64(y)', EXTENT[2], EXTENT[3])} AS qy, band, num FROM {src})")
        self.sql("OPTIMIZE TABLE points FINAL")
        build_s = time.monotonic() - t0
        mem = self.serve_memory()
        disk = int(self.sql("SELECT sum(bytes_on_disk) FROM system.parts WHERE active AND table = 'points'"))
        rows = int(self.sql("SELECT count() FROM points"))
        self.stop()
        self._create()
        return {"build_s": build_s, **mem, "disk_bytes": disk, "rows": rows,
                "data_dir_bytes": du_bytes(self.data), "nice": self.nice}

    # -- a frame -----------------------------------------------------------------------------------

    def frame(self, camera: Camera, filt: Filter | None) -> tuple[Frame, float]:
        z, dg = camera.zoom, grid_depth(camera.zoom)
        tiles = camera.tiles()
        where = tile_ranges(tiles, z) + filter_sql(filt)
        grid_q = (f"SELECT bitShiftRight(m, {64 - 2 * dg}) AS cell, count() AS n FROM points "
                  f"WHERE {where} GROUP BY cell FORMAT ArrowStream")
        tile_expr = f"bitShiftRight(m, {64 - 2 * z})" if z else "0"
        points_q = (f"SELECT qx, qy FROM points WHERE {where} AND {tile_expr} IN ("
                    f"SELECT {tile_expr} AS t FROM points WHERE {where} GROUP BY t HAVING count() <= {K}) "
                    f"FORMAT ArrowStream")
        t0 = time.perf_counter()
        fg = self.pool.submit(self.sql, grid_q, 0)
        fp = self.pool.submit(self.sql, points_q, 1)
        grid_raw, points_raw = fg.result(), fp.result()
        elapsed = time.perf_counter() - t0
        return self._decode(grid_raw, points_raw, z, dg), elapsed

    @staticmethod
    def _decode(grid_raw: bytes, points_raw: bytes, z: int, dg: int) -> Frame:
        grid, tiles, points = {}, {}, {}
        g = ipc.open_stream(io.BytesIO(grid_raw)).read_all()
        if g.num_rows:
            gx, gy = deinterleave(g["cell"].to_numpy())
            shift = dg - z
            for a, b, n in zip(gx.tolist(), gy.tolist(), g["n"].to_numpy().tolist()):
                grid[(a, b)] = n
                t = (a >> shift, b >> shift)
                tiles[t] = tiles.get(t, 0) + n
        p = ipc.open_stream(io.BytesIO(points_raw)).read_all()
        if p.num_rows:
            points = points_by_tile(p["qx"].to_numpy(), p["qy"].to_numpy(), z)
        return Frame(tiles=tiles, grid=grid, points=points, bytes=len(grid_raw) + len(points_raw))
