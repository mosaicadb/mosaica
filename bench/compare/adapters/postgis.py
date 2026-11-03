"""PostGIS: a pinned PostgreSQL and PostGIS image, one table in spatial order with one index, and
the frame in SQL, as a role under row-level security.

Three arms share this adapter and differ in the table's order, its index and one setup step:

* `postgis`: a GiST index on `geom`, the index PostGIS's documentation recommends, with the
  table in `geom`'s order (PostGIS sorts points along a Hilbert curve). Under the policy the
  viewer's queries cannot use it (see "Access" below), so each scans the table.
* `postgis+leakproof`: the same, and the superuser marks `geometry_overlaps`, the function behind
  `&&`, leakproof, so the viewer's queries can use the index.
* `postgis+integer`: no geometry; a B-tree on `(qx, qy)`, with the table in that order. The
  camera's rectangle is a range of `qx` and a range of `qy` within it, and comparisons of
  `bigint` are leakproof, so the viewer's queries use the index with no claim made.

The build streams the tier's `x`, `y`, `licence`, `publication_year`, `type`, `is_oa` and `rank` (never
the oracle's `qx` and `qy`) from pyarrow as CSV into `COPY staging FROM STDIN`. CSV rather than
binary: psycopg encodes binary `COPY` a row at a time in Python, 2.7 s against CSV's 0.8 s at 10⁶.
Then `CREATE TABLE points AS SELECT ... ORDER BY` the arm's key, in which PostgreSQL quantises
each position by the frame's rule, the arm's index and statistics on the cells at each depth, all
in the load's transaction; then `VACUUM ANALYZE`, a `CHECKPOINT` and an `fsync` of the data
directory, all in the build time. Positions are `bigint`, since PostgreSQL has no unsigned 32-bit
integer, and `geom` is the same position as a point in quantised space.

A frame is two queries sent at once on two connections, each a `COPY (...) TO STDOUT` in the
binary format: the count grid (`GROUP BY` the cell, `qx` and `qy` shifted right), and the
positions. Under the `points` frame those are each tile's K matching rows of lowest `rank`, by
`row_number() OVER (PARTITION BY` the tile `ORDER BY rank)`, which is every row of a sparse tile;
under `counts`, every position in the tiles whose count is at most K. The aggregation workload is two `GROUP BY`
queries sent at once. PostgreSQL keeps no result cache, so `--caches compute` and `defaults` run
the same server.

Access: after the build, the superuser creates the login role `viewer`, which owns nothing and is
granted `SELECT` on the table alone, enables and forces row-level security on the table, and
creates a policy for `viewer` keeping the rows whose `licence` is one of the viewer's terms. Every
frame and table is queried as `viewer`, and no query names a licence. The setup is not counted in
the build time. PostgreSQL evaluates the policy's condition before any of the query's that is not
leakproof, and does not scan an index by such a condition; PostGIS marks none of its functions
leakproof.

`SETTINGS` holds every server setting the adapter writes, and `BULK` the ones added for the build
alone; the README gives the reason for each.
"""

from __future__ import annotations

import io
import os
import secrets
import subprocess
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import numpy as np
import psycopg
import pyarrow.csv as pacsv
import pyarrow.parquet as pq

from frame import (EXTENT, K, MAX_DEPTH, YEAR_BINS, Camera, Filter, Frame, grid_depth,
                   points_by_tile)
from oracle import VIEWER

from .base import (MEMORY, MEMORY_GIB, Adapter, Container, docker_info, du_bytes, evict_files, free_ports,
                   fsync_tree, wait_for)
from .sql import filter_sql

IMAGE = "postgis/postgis:18-3.6"
IMAGE_DIGEST = "sha256:4a43e3c3e5d55b9208240fc71db543b79c638766280c625a425bd0a2390989c4"
#: The data directory inside the container, the image's default under its volume.
PGDATA = "/var/lib/postgresql/18/docker"
VIEWER_ROLE = "viewer"
CPUS = os.cpu_count() or 1
#: A quarter of the cap for shared buffers and three quarters as the planner's cache estimate, as
#: PostgreSQL's documentation suggests for a dedicated server.
SETTINGS = {
    "shared_buffers": f"{MEMORY_GIB * 256}MB",
    "effective_cache_size": f"{MEMORY_GIB * 768}MB",
    "work_mem": "64MB",
    "max_worker_processes": str(CPUS),
    "max_parallel_workers": str(CPUS),
    "max_parallel_workers_per_gather": str(max(1, CPUS // 2)),
    "jit": "off",
    "random_page_cost": "1.1",
}
BULK = {"wal_level": "minimal", "max_wal_senders": "0", "maintenance_work_mem": f"{min(2048, MEMORY_GIB * 128)}MB",
        "max_wal_size": "16GB"}
SHM = f"{min(2048, MEMORY_GIB * 128)}m"
COLUMNS = ("x", "y", "licence", "publication_year", "type", "is_oa", "rank")
#: Each arm's index and whether `&&` is marked leakproof.
ARMS = {"postgis": ("gist", False), "postgis+leakproof": ("gist", True),
        "postgis+integer": ("btree", False)}


def quantise(expr: str, lo: float, hi: float) -> str:
    """The frame's rule in double precision, as `float8`; every operand is cast, since a bare
    decimal literal is `numeric` in PostgreSQL."""
    f = lambda v: f"({v!r})::float8"
    return (f"least(greatest(floor((({expr})::float8 - {f(lo)}) / ({f(hi)} - {f(lo)}) * "
            f"(4294967296)::float8), (0)::float8), (4294967295)::float8)")


def pg_quantise(expr: str, lo: float, hi: float) -> str:
    return f"({quantise(expr, lo, hi)})::bigint"


STAGING = ("CREATE TABLE staging (x float8 NOT NULL, y float8 NOT NULL, licence text NOT NULL, "
           "publication_year integer, type text, is_oa boolean, rank bigint NOT NULL)")
QUANTISED = (f"SELECT {pg_quantise('x', EXTENT[0], EXTENT[1])} AS qx, "
             f"{pg_quantise('y', EXTENT[2], EXTENT[3])} AS qy, licence, publication_year, type, is_oa, "
             f"rank FROM staging")
ATTRIBUTES = "licence, publication_year, type, is_oa, rank"
#: Each index's table, as `CREATE TABLE points AS`, and the index on it.
TABLES = {
    "gist": (f"SELECT qx, qy, ST_MakePoint(qx, qy)::geometry(Point, 0) AS geom, {ATTRIBUTES} "
             f"FROM ({QUANTISED}) q ORDER BY geom",
             "CREATE INDEX points_geom ON points USING gist (geom)"),
    "btree": (f"SELECT qx, qy, {ATTRIBUTES} FROM ({QUANTISED}) q ORDER BY qx, qy",
              "CREATE INDEX points_qx_qy ON points (qx, qy)"),
}


def statements(index: str) -> tuple[str, ...]:
    """The build after the load, in order."""
    select, create_index = TABLES[index]
    return (f"CREATE TABLE points AS {select}", "DROP TABLE staging", create_index,
            # The cells at each depth a grid or a tile is grouped by, whose count the planner
            # cannot estimate from the columns' statistics; overestimated, it does not aggregate
            # in parallel.
            *(f"CREATE STATISTICS points_cells_{d} ON (qx >> {32 - d}), (qy >> {32 - d}) FROM points"
              for d in range(MAX_DEPTH + 1)))


def tiles_sql(camera: Camera, index: str) -> str:
    """The rectangle of the camera's tiles, exactly by the bounds on `qx` and `qy`. For GiST, also
    by `&&`, which the index answers; PostGIS compares boxes in single precision, rounded
    outwards, so `&&` alone admits points just outside the rectangle."""
    tx0, tx1, ty0, ty1 = camera.tile_range()
    s = 32 - camera.zoom
    x0, x1, y0, y1 = tx0 << s, ((tx1 + 1) << s) - 1, ty0 << s, ((ty1 + 1) << s) - 1
    exact = f"qx BETWEEN {x0} AND {x1} AND qy BETWEEN {y0} AND {y1}"
    if index == "btree":
        return exact
    return f"geom && ST_MakeEnvelope({x0}, {y0}, {x1}, {y1}, 0) AND {exact}"


def from_binary_copy(raw: bytes, ncols: int) -> np.ndarray:
    """The rows of a binary `COPY` of `ncols` non-null `bigint` columns, as an (n, ncols) array."""
    header, trailer = 19, 2
    if raw[:11] != b"PGCOPY\n\xff\r\n\0" or raw[15:19] != b"\0\0\0\0" or raw[-2:] != b"\xff\xff":
        raise ValueError("not a binary COPY with no header extension")
    dtype = np.dtype([("n", ">i2"), *((f, t) for i in range(ncols)
                                      for f, t in ((f"l{i}", ">i4"), (f"v{i}", ">i8")))])
    rows = np.frombuffer(raw[header:len(raw) - trailer], dtype)
    if len(rows) and ((rows["n"] != ncols).any() or any((rows[f"l{i}"] != 8).any() for i in range(ncols))):
        raise ValueError(f"a row is not {ncols} non-null bigint columns")
    return np.stack([rows[f"v{i}"].astype(np.int64) for i in range(ncols)], axis=1).reshape(-1, ncols)


class PostGIS(Adapter):
    def __init__(self, tier: Path, work: Path, nice: int = 0, caches: str = "compute",
                 arm: str = "postgis", dense: str = "points"):
        super().__init__(tier, work, nice, caches, dense)
        self.name = arm
        self.index, self.leakproof = ARMS[arm]
        self.access = (f"ROW LEVEL SECURITY enabled and forced on points, POLICY FOR SELECT TO "
                       f"{VIEWER_ROLE} USING (licence IN (the viewer's terms)); {VIEWER_ROLE} owns "
                       f"nothing and has SELECT on points alone; every frame and table queried as "
                       f"{VIEWER_ROLE}"
                       + ("; geometry_overlaps(geometry, geometry) marked LEAKPROOF by the superuser"
                          if self.leakproof else ""))
        self.table_made = False
        self.container = Container(f"bench-compare-pg-{os.getpid()}")
        self.port = free_ports(1)[0]
        self.root = self.work / "pg"
        self.data = self.root / Path(PGDATA).relative_to("/var/lib/postgresql")
        self.pool = ThreadPoolExecutor(2)
        self.password = secrets.token_hex(16)
        self.viewer_password = secrets.token_hex(16)
        self.admin: psycopg.Connection | None = None
        self.sessions: list[psycopg.Connection] = []

    # -- the container ---------------------------------------------------------------------------

    def _create(self, settings: dict, nice: int = 0) -> None:
        """Create the container, its server at niceness `nice`. The volume is mounted from the
        work directory, so the image leaves no anonymous volume behind."""
        self.root.mkdir(parents=True, exist_ok=True)
        self.container.create(
            f"{IMAGE.split(':')[0]}@{IMAGE_DIGEST}", entrypoint="docker-entrypoint.sh", nice=nice,
            args=("postgres", *(a for k, v in settings.items() for a in ("-c", f"{k}={v}"))),
            env={"POSTGRES_PASSWORD": self.password, "PGDATA": PGDATA}, shm=SHM,
            ports={self.port: 5432}, mounts={str(self.root): "/var/lib/postgresql"})

    def _connect(self, user: str, password: str, **kw) -> psycopg.Connection:
        return psycopg.connect(host="127.0.0.1", port=self.port, dbname="postgres", user=user,
                               password=password, autocommit=True, prepare_threshold=None, **kw)

    def start(self) -> None:
        """Start the container, ready once the superuser's query against the table answers on a
        connection of its own, or before the table exists, `SELECT 1`. The image's first start
        runs `initdb` on a server that listens on no TCP port, so the probe cannot reach it. The
        viewer's authentication and first planning fall after `connect`."""
        self.container.start()
        wait_for(self._ready, 600, f"PostgreSQL answering; see docker logs {self.container.name}")

    def _ready(self) -> bool:
        probe = "SELECT 1 FROM points LIMIT 1" if self.table_made else "SELECT 1"
        try:
            with self._connect("postgres", self.password, connect_timeout=5) as c:
                c.execute(probe).fetchall()
            return True
        except psycopg.Error:
            return False

    def connect(self) -> None:
        self.sessions = [self._connect(VIEWER_ROLE, self.viewer_password) for _ in range(2)]

    def stop(self) -> None:
        for c in [*self.sessions, *([self.admin] if self.admin else [])]:
            c.close()
        self.sessions, self.admin = [], None
        self.container.stop()

    def remove(self) -> None:
        self.container.remove()

    def evict(self) -> None:
        evict_files(self.data)

    def serve_memory(self) -> dict:
        return self.container.memory()

    # -- SQL ---------------------------------------------------------------------------------------

    def sql(self, query: str) -> list[tuple]:
        """Run `query` as the superuser, on a connection opened at its first use."""
        self.admin = self.admin or self._connect("postgres", self.password)
        cur = self.admin.execute(query)
        return cur.fetchall() if cur.description else []

    def version(self) -> dict:
        return {"version": self.sql("SHOW server_version")[0][0],
                "postgis": self.sql("SELECT postgis_full_version()")[0][0],
                "full": self.sql("SELECT version()")[0][0], "psycopg": psycopg.__version__,
                "image": IMAGE, "digest": IMAGE_DIGEST, "docker": docker_info()}

    def config(self) -> dict:
        names = [*SETTINGS, *BULK, "max_connections", "effective_io_concurrency", "io_method",
                 "io_workers", "huge_pages", "shared_memory_type", "dynamic_shared_memory_type",
                 "parallel_setup_cost", "parallel_tuple_cost", "min_parallel_table_scan_size",
                 "fsync", "synchronous_commit", "data_checksums"]
        settings = dict(self.sql("SELECT name, current_setting(name) FROM pg_settings WHERE name IN ("
                                 + ", ".join(f"'{n}'" for n in names) + ")"))
        rls = self.sql("SELECT relrowsecurity, relforcerowsecurity FROM pg_class WHERE relname = 'points'")[0]
        policy = self.sql("SELECT policyname, permissive, roles::text, cmd, qual FROM pg_policies "
                          "WHERE tablename = 'points'")
        leakproof = dict(self.sql(
            "SELECT p.oid::regprocedure::text, p.proleakproof FROM pg_operator o "
            "JOIN pg_proc p ON p.oid = o.oprcode WHERE o.oprname = '&&' "
            "AND o.oprleft = 'geometry'::regtype AND o.oprright = 'geometry'::regtype"))
        mid = Camera(8, (EXTENT[0] + EXTENT[1]) / 2, (EXTENT[2] + EXTENT[3]) / 2)
        return {"arm": self.name, "index": self.index, "build": [STAGING, *statements(self.index)],
                "settings_written": SETTINGS, "build_settings_written": BULK, "settings": settings,
                "shm_size": SHM, "caches": self.caches,
                "cache_settings": {"result cache": "none in PostgreSQL",
                                   "shared_buffers": settings.get("shared_buffers"),
                                   "client prepared statements": "off (prepare_threshold=None)"},
                "row_level_security": {"enabled": rls[0], "forced": rls[1]},
                "policy": [list(p) for p in policy], "geometry_overlaps_leakproof": leakproof,
                "plan_zoom8_grid_at_centre": self.explain(self._grid_sql(mid, None)),
                "plan_zoom8_points_at_centre": self.explain(self._points_sql(mid, None)),
                "container_memory": MEMORY, "load": "COPY staging FROM STDIN (FORMAT csv), "
                "streamed from pyarrow's CSV writer, in the build's transaction"}

    def explain(self, query: str) -> list[str]:
        """The plan of `query` as the viewer, as its first connection runs it."""
        return [r[0] for r in self.sessions[0].execute(f"EXPLAIN {query}").fetchall()]

    def build(self) -> dict:
        self.remove()
        self.table_made = False
        subprocess.run(["rm", "-rf", str(self.root)], check=True)
        self._create({**SETTINGS, **BULK}, self.nice)
        self.start()
        self.sql("SELECT 1")  # the superuser's connection, opened outside the build time
        t0 = time.monotonic()
        phases = {}
        # One transaction, so that under `wal_level = minimal` the tables it creates skip the WAL.
        with self.admin.transaction():
            self.sql(STAGING)
            with ThreadPoolExecutor(1) as loader:
                loader.submit(self._load).result()
            phases["load"] = time.monotonic() - t0
            for statement in statements(self.index):
                t = time.monotonic()
                self.sql(statement)
                phase = " ".join(statement.split()[:2]).lower()
                phases[phase] = phases.get(phase, 0) + time.monotonic() - t
        for statement in ("VACUUM ANALYZE points", "CHECKPOINT"):
            t = time.monotonic()
            self.sql(statement)
            phases[statement.split()[0].lower()] = time.monotonic() - t
        t = time.monotonic()
        fsync_tree(self.data)
        phases["fsync"] = time.monotonic() - t
        build_s = time.monotonic() - t0
        self.table_made = True
        mem = self.serve_memory()
        sizes = self.sql("SELECT pg_table_size('points'), pg_indexes_size('points'), count(*) FROM points")[0]
        templates = [f"base/{oid}" for (oid,) in self.sql("SELECT oid FROM pg_database WHERE datistemplate")]
        terms = ", ".join(f"'{t}'" for t in VIEWER["terms"])
        for statement in (
                f"CREATE ROLE {VIEWER_ROLE} LOGIN PASSWORD '{self.viewer_password}'",
                f"GRANT SELECT ON points TO {VIEWER_ROLE}",
                "ALTER TABLE points ENABLE ROW LEVEL SECURITY",
                "ALTER TABLE points FORCE ROW LEVEL SECURITY",
                f"CREATE POLICY viewer_terms ON points FOR SELECT TO {VIEWER_ROLE} "
                f"USING (licence IN ({terms}))",
                *(["ALTER FUNCTION geometry_overlaps(geometry, geometry) LEAKPROOF"] if self.leakproof else []),
                "CHECKPOINT"):
            self.sql(statement)
        self.stop()
        directory = du_bytes(self.data)
        wal = du_bytes(self.data / "pg_wal")
        template_bytes = sum(du_bytes(self.data / t) for t in templates)
        self._create(SETTINGS)
        return {"build_s": build_s, "phases_s": phases, **mem,
                "disk_bytes": directory - wal - template_bytes,
                "disk": "du of the data directory less pg_wal and the template databases",
                "directory_bytes": directory, "wal_bytes": wal, "template_bytes": template_bytes,
                "table_bytes": sizes[0], "index_bytes": sizes[1], "rows": sizes[2], "nice": self.nice,
                "boundary": "from the CREATE TABLE staging to the end of CHECKPOINT and an fsync "
                            "of every file and directory in the data directory, on a running "
                            "server; the loader, which reads the Parquet file and writes CSV, runs "
                            "in the harness's process, outside the container's cgroup"}

    def _load(self) -> None:
        """Stream the tier into `COPY` at the build's niceness: this thread's alone, so the harness
        keeps its own."""
        os.setpriority(os.PRIO_PROCESS, threading.get_native_id(), self.nice)
        options = pacsv.WriteOptions(include_header=False)
        with self.admin.cursor().copy(f"COPY staging ({', '.join(COLUMNS)}) FROM STDIN (FORMAT csv)") as cp:
            for batch in pq.ParquetFile(self.tier).iter_batches(1 << 16, columns=list(COLUMNS),
                                                                 use_threads=False):
                buf = io.BytesIO()
                pacsv.write_csv(batch, buf, options)
                cp.write(buf.getbuffer())

    # -- a frame -----------------------------------------------------------------------------------

    def _where(self, camera: Camera, filt: Filter | None) -> str:
        return tiles_sql(camera, self.index) + filter_sql(filt, pg_quantise)

    def _grid_sql(self, camera: Camera, filt: Filter | None) -> str:
        s = 32 - grid_depth(camera.zoom)
        return (f"SELECT qx >> {s}, qy >> {s}, count(*) FROM points WHERE {self._where(camera, filt)} "
                f"GROUP BY 1, 2")

    def _points_sql(self, camera: Camera, filt: Filter | None) -> str:
        where, s = self._where(camera, filt), 32 - camera.zoom
        if self.dense == "points":
            return (f"SELECT qx, qy FROM (SELECT qx, qy, row_number() OVER (PARTITION BY qx >> {s}, "
                    f"qy >> {s} ORDER BY rank) AS r FROM points WHERE {where}) ranked WHERE r <= {K}")
        return (f"SELECT qx, qy FROM points WHERE {where} AND (qx >> {s}, qy >> {s}) IN ("
                f"SELECT qx >> {s}, qy >> {s} FROM points WHERE {where} GROUP BY 1, 2 "
                f"HAVING count(*) <= {K})")

    def _copy(self, session: int, query: str) -> bytes:
        with self.sessions[session].cursor().copy(f"COPY ({query}) TO STDOUT (FORMAT binary)") as cp:
            return b"".join(cp)

    def _fetch(self, session: int, query: str) -> psycopg.Cursor:
        """The cursor holds the whole result once `execute` returns; converting it is decoding."""
        return self.sessions[session].execute(query)

    def frame(self, camera: Camera, filt: Filter | None) -> tuple[Frame, float]:
        z, dg = camera.zoom, grid_depth(camera.zoom)
        points_q = self._points_sql(camera, filt)
        t0 = time.perf_counter()
        fg = self.pool.submit(self._copy, 0, self._grid_sql(camera, filt))
        fp = self.pool.submit(self._copy, 1, points_q)
        grid_raw, points_raw = fg.result(), fp.result()
        elapsed = time.perf_counter() - t0
        grid, tiles = {}, {}
        for gx, gy, n in from_binary_copy(grid_raw, 3).tolist():
            grid[(gx, gy)] = n
            t = (gx >> (dg - z), gy >> (dg - z))
            tiles[t] = tiles.get(t, 0) + n
        p = from_binary_copy(points_raw, 2)
        points = points_by_tile(p[:, 0], p[:, 1], z) if len(p) else {}
        return Frame(tiles=tiles, grid=grid, points=points, bytes=len(grid_raw) + len(points_raw)), elapsed

    def tables(self, camera: Camera, filt: Filter | None) -> tuple[dict, float]:
        where = self._where(camera, filt)
        lo, hi = YEAR_BINS
        type_q = f"SELECT coalesce(type, 'none'), count(*) FROM points WHERE {where} GROUP BY 1"
        year_q = (f"SELECT CASE WHEN publication_year IS NULL THEN 'none' WHEN publication_year < {lo} "
                  f"OR publication_year >= {hi} THEN 'rest' ELSE publication_year::text END, count(*) "
                  f"FROM points WHERE {where} GROUP BY 1")
        t0 = time.perf_counter()
        ft = self.pool.submit(self._fetch, 0, type_q)
        fy = self.pool.submit(self._fetch, 1, year_q)
        types, years = ft.result(), fy.result()
        elapsed = time.perf_counter() - t0
        return {"type": dict(types.fetchall()), "year": dict(years.fetchall())}, elapsed
