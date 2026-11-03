"""ClickHouse: a pinned server image, one MergeTree table sorted by Morton code, and the frame in SQL.

The build reads the tier's `x`, `y`, `licence`, `publication_year`, `type` and `is_oa` (never the
oracle's `qx` and `qy`),
quantises the coordinates in SQL by the frame's rule, and stores `m = mortonEncode(qx, qy)` as the
sorting key, so that a tile is a contiguous range of `m` as it is in Mosaica. `OPTIMIZE TABLE ...
FINAL` merges the table to one part and is counted in the build.

A frame is two queries sent at once over HTTP: the count grid (`GROUP BY` the depth-`grid_depth`
prefix of `m` over the camera's tile ranges), and every position in the tiles whose count is at
most K (the same ranges, restricted in the same statement to the sparse tiles). The query cache
is off, as it is by default. The server's own log tables are turned off so their writes stay out
of the timings.

Access: after the build, the default user (given `access_management` for this) creates the user
`viewer`, grants it `SELECT` on the table, and creates a row policy on the table for it keeping
the rows whose `licence` is one of the viewer's terms. Every frame and table is queried as
`viewer`, and no query names a licence. The setup is not counted in the build time.

The aggregation workload is two `GROUP BY` queries sent at once, over the frame's tile ranges and
the filter: counts by `type`, and by year bin with `rest` and `none` folded in by `multiIf`.
"""

from __future__ import annotations

import io
import os
import secrets
import subprocess
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import pyarrow.ipc as ipc
import requests

from frame import (EXTENT, K, YEAR_BINS, Camera, Filter, Frame, deinterleave, grid_depth, interleave,
                   points_by_tile)
from oracle import VIEWER

from .base import MEMORY, Adapter, Container, docker_info, du_bytes, evict_files, wait_for

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
ACCESS_MANAGEMENT = """<clickhouse>
  <users><default><access_management>1</access_management></default></users>
</clickhouse>
"""
DDL = ("CREATE TABLE points (m UInt64, qx UInt32, qy UInt32, licence LowCardinality(String), "
       "publication_year Nullable(Int32), type LowCardinality(Nullable(String)), is_oa Nullable(Bool)) "
       "ENGINE = MergeTree ORDER BY m")
VIEWER_USER = "viewer"


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
        col, value = leaf.get("col"), leaf["value"]
        if leaf["op"] == "eq" and col == "type":
            terms.append(f"type = '{value}'")
        elif leaf["op"] == "in" and col == "type":
            terms.append("type IN (" + ", ".join(f"'{v}'" for v in value) + ")")
        elif leaf["op"] == "eq" and col == "is_oa":
            terms.append(f"is_oa = {'true' if value else 'false'}")
        elif leaf["op"] == "range":
            if "gte" in value:
                terms.append(f"{col} >= {int(value['gte'])}")
            if "lt" in value:
                terms.append(f"{col} < {int(value['lt'])}")
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
    access = (f"ROW POLICY on points FOR SELECT USING licence IN (the viewer's terms) TO "
              f"{VIEWER_USER}; every frame and table queried as {VIEWER_USER}")

    def __init__(self, tier: Path, work: Path, nice: int = 0):
        super().__init__(tier, work, nice)
        self.container = Container(f"bench-compare-ch-{os.getpid()}")
        self.data = self.work / "ch-data"
        self.pool = ThreadPoolExecutor(2)
        self.admin = requests.Session()
        self.viewer_password = secrets.token_hex(16)
        self.sessions = []

    # -- the container ---------------------------------------------------------------------------

    def _create(self, nice: int = 0) -> None:
        """Create the container. With `nice`, the server runs at that niceness: the build's
        container, which is removed once the table is built, the serving one being created at 0."""
        (self.work / "config.d").mkdir(exist_ok=True)
        (self.work / "config.d/no-logs.xml").write_text(NO_LOGS)
        (self.work / "config.d/access-management.xml").write_text(ACCESS_MANAGEMENT)
        # Made here, not by docker, so they belong to this user and a rebuild can delete them.
        for d in ("ch-data/user_files/tiers", "ch-logs"):
            (self.work / d).mkdir(parents=True, exist_ok=True)
        self.container.create(
            f"{IMAGE.split(':')[0]}@{IMAGE_DIGEST}", entrypoint="/entrypoint.sh", nice=nice,
            env={"CLICKHOUSE_SKIP_USER_SETUP": "1"}, ports={PORT: 8123}, mounts={
                str(self.data): "/var/lib/clickhouse",
                str(self.work / "ch-logs"): "/var/log/clickhouse-server",
                str(self.work / "config.d/no-logs.xml"): "/etc/clickhouse-server/config.d/no-logs.xml:ro",
                str(self.work / "config.d/access-management.xml"):
                    "/etc/clickhouse-server/users.d/access-management.xml:ro",
                str(self.tier.parent): "/var/lib/clickhouse/user_files/tiers:ro",
            })

    def start(self) -> dict:
        """Start the container. Returns the seconds to `/ping` answering; ClickHouse's HTTP
        interface has no session to open, so the viewer's credential travels with each query."""
        t0 = time.perf_counter()
        self.container.start()
        wait_for(self._ping, 120, "ClickHouse answering /ping")
        ready = time.perf_counter() - t0
        self.sessions = [requests.Session(), requests.Session()]
        for s in self.sessions:
            s.headers.update({"X-ClickHouse-User": VIEWER_USER, "X-ClickHouse-Key": self.viewer_password})
        return {"ready_s": ready}

    def _ping(self) -> bool:
        try:
            return requests.get(f"http://127.0.0.1:{PORT}/ping", timeout=1).ok
        except requests.RequestException:
            return False

    def stop(self) -> None:
        for s in self.sessions:
            s.close()
        self.sessions = []
        self.container.stop()

    def remove(self) -> None:
        self.container.remove()

    def evict(self) -> None:
        evict_files(self.data)

    def serve_memory(self) -> dict:
        return self.container.memory()

    # -- SQL ---------------------------------------------------------------------------------------

    def sql(self, query: str, session: int | None = None, timeout: float = 3600) -> bytes:
        """Run `query` as the default user, or as the viewer on its connection `session`."""
        http = self.admin if session is None else self.sessions[session]
        r = http.post(f"http://127.0.0.1:{PORT}/", params={"use_query_cache": 0},
                      data=query.encode(), timeout=timeout)
        if r.status_code != 200:
            raise RuntimeError(f"ClickHouse {r.status_code}: {r.text[:800]}")
        return r.content

    def version(self) -> dict:
        return {"version": self.sql("SELECT version()").decode().strip(), "image": IMAGE,
                "digest": IMAGE_DIGEST, "docker": docker_info()}

    def config(self) -> dict:
        settings = self.sql("SELECT name, value FROM system.settings WHERE name IN "
                            "('max_threads', 'use_query_cache', 'max_memory_usage') FORMAT TSV").decode()
        return {"ddl": DDL, "optimize_final": True, "system_logs": "off",
                "row_policy": self.sql("SHOW CREATE ROW POLICY viewer_terms ON default.points").decode().strip(),
                "container_memory": MEMORY, "settings": dict(l.split("\t") for l in settings.splitlines())}

    def build(self) -> dict:
        self.remove()
        subprocess.run(["rm", "-rf", str(self.data), str(self.work / "ch-logs")], check=True)
        self._create(self.nice)
        self.start()
        if self.sql("SELECT mortonEncode(toUInt32(1), toUInt32(0))").strip() != b"1":
            raise RuntimeError("mortonEncode does not put its first argument on the even bits")
        src = (f"file('tiers/{self.tier.name}', Parquet, 'x Float64, y Float64, licence String, "
               f"publication_year Nullable(Int32), type Nullable(String), is_oa Nullable(Bool)')")
        t0 = time.monotonic()
        self.sql(DDL)
        self.sql(f"INSERT INTO points SELECT mortonEncode(qx, qy) AS m, qx, qy, licence, "
                 f"publication_year, type, is_oa FROM ("
                 f"SELECT {quantise('x', EXTENT[0], EXTENT[1])} AS qx, "
                 f"{quantise('y', EXTENT[2], EXTENT[3])} AS qy, licence, publication_year, type, is_oa "
                 f"FROM {src})")
        self.sql("OPTIMIZE TABLE points FINAL")
        build_s = time.monotonic() - t0
        mem = self.serve_memory()
        disk = int(self.sql("SELECT sum(bytes_on_disk) FROM system.parts WHERE active AND table = 'points'"))
        rows = int(self.sql("SELECT count() FROM points"))
        terms = ", ".join(f"'{t}'" for t in VIEWER["terms"])
        self.sql(f"CREATE USER {VIEWER_USER} IDENTIFIED WITH sha256_password BY '{self.viewer_password}'")
        self.sql(f"GRANT SELECT ON default.points TO {VIEWER_USER}")
        self.sql(f"CREATE ROW POLICY viewer_terms ON default.points FOR SELECT "
                 f"USING licence IN ({terms}) AS permissive TO {VIEWER_USER}")
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

    def tables(self, camera: Camera, filt: Filter | None) -> tuple[dict, float]:
        where = tile_ranges(camera.tiles(), camera.zoom) + filter_sql(filt)
        lo, hi = YEAR_BINS
        type_q = (f"SELECT ifNull(type, 'none'), count() FROM points WHERE {where} "
                  f"GROUP BY type FORMAT TSV")
        year_q = (f"SELECT multiIf(publication_year IS NULL, 'none', publication_year < {lo} OR "
                  f"publication_year >= {hi}, 'rest', toString(publication_year)) AS bin, count() "
                  f"FROM points WHERE {where} GROUP BY bin FORMAT TSV")
        t0 = time.perf_counter()
        ft = self.pool.submit(self.sql, type_q, 0)
        fy = self.pool.submit(self.sql, year_q, 1)
        type_raw, year_raw = ft.result(), fy.result()
        elapsed = time.perf_counter() - t0
        parse = lambda raw: {k: int(n) for k, n in (l.split("\t") for l in raw.decode().splitlines())}
        return {"type": parse(type_raw), "year": parse(year_raw)}, elapsed

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
