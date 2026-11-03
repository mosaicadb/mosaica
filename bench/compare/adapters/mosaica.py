"""Mosaica: `mosaica build` over the tier, `mosaica serve`, and the frame over HTTP, as the viewer.

The view declares `licence` as its `point_visibility`, so each row's licence is its access label.
After the first start the adapter creates the viewer as a principal in the catalogue, grants it
`read` and exactly the terms of `viewer.json`, and issues it an API key; every start then logs in
with that key at `POST /v1/login`, and every request carries the session token. No request
names the viewer's terms.

Two ways to ask for a frame, run as two systems:

* `mosaica`: one `POST /v1/viewport`, as Mosaica's own client asks: the camera's tiles as Morton
  prefixes, `k = K`, the grid as `underlay_offset = grid_depth - zoom` (none at zoom 16, where the
  grid is the tiles), the filter in Mosaica's grammar, and `point_rows: []` so each point carries
  its position alone.
* `mosaica+aggregate`: the same viewport request without the underlay, and beside it, sent at the
  same time, a `POST /v1/aggregate` counting the filtered set in the cells at `grid_depth` over
  the camera's tiles. The latency covers both.

The aggregation workload is one `POST /v1/aggregate` with two groupings, the counts by `type` and
the `publication_year` histogram, over the filter and a `region` box of the camera's tiles.

`SERVE` holds every server setting the adapter writes, and the README's "Departures" lists each
against its default and why. Each is recorded with the result.
"""

from __future__ import annotations

import io
import json
import os
import secrets
import shlex
import signal
import struct
import subprocess
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import numpy as np
import pyarrow.ipc as ipc
import requests

from frame import (EXTENT, FIXED_SPAN, GRID_OFFSET, K, TYPE_TOP, TYPES, YEAR_BINS, Camera, Filter,
                   Frame, deinterleave, grid_depth, interleave, points_by_tile)
from oracle import VIEWER

from .base import Adapter, du_bytes, evict_files, read_int, scope_dir, scoped, user_scopes, vm_hwm

PORTS = (18141, 18142, 18143)
CRED_ENV = "BENCH_COMPARE_OPERATOR"
VIEW = "papers"
SERVE = {
    "k_max_marks": K,
    "max_k": 1000,
    "theta_target_marks": 1 << 40,
    "max_underlay_offset": GRID_OFFSET,
    "max_underlay_cells": 20 << (2 * GRID_OFFSET),
    "max_aggregate_cells": 20 << (2 * GRID_OFFSET),
    "aggregate_response_bytes": 256 << 20,
}
CORPUS = f"""
[sources]
points = "points.parquet"

[defaults]
source = "points"

[[view]]
name             = "{VIEW}"
projection       = "none"
extent           = {{ x = [{EXTENT[0]!r}, {EXTENT[1]!r}], y = [{EXTENT[2]!r}, {EXTENT[3]!r}] }}
point_visibility = {{ field = "licence", default = "unlicensed" }}

[[vocabulary]]
name       = "type"
width      = "u8"
value_set  = "closed"
visibility = "public"
values     = {json.dumps(list(TYPES))}

[[attribute]]
name       = "type"
type       = "category"
vocabulary = "type"
index      = true
render     = true

[[attribute]]
name   = "publication_year"
type   = "i32"
index  = true
render = true

[[attribute]]
name   = "is_oa"
type   = "bool"
index  = true
render = true
"""


def filter_leaves(filt: Filter | None) -> list:
    out = []
    for leaf in (filt.leaves if filt else []):
        op, col, value = leaf["op"], leaf.get("col"), leaf["value"]
        if op == "eq":
            out.append({col: {"eq": value}})
        elif op == "range":
            out.append({col: {"range": dict(value)}})
        elif op == "bbox":
            out.append({"region": {"bbox": list(value)}})
        else:
            raise ValueError(leaf)
    return out


def combine(leaves: list):
    if not leaves:
        return None
    return leaves[0] if len(leaves) == 1 else {"all_of": leaves}


def tiles_area(camera: Camera, inset: float) -> list[float]:
    """The box of the camera's tiles, `inset` inside each edge."""
    tx0, tx1, ty0, ty1 = camera.tile_range()
    size = (EXTENT[1] - EXTENT[0]) / (1 << camera.zoom)
    return [EXTENT[0] + tx0 * size + inset, EXTENT[2] + ty0 * size + inset,
            EXTENT[0] + (tx1 + 1) * size - inset, EXTENT[2] + (ty1 + 1) * size - inset]


def aggregate_request(camera: Camera, filt: Filter | None, depth: int) -> dict:
    """The `/v1/aggregate` body counting the filtered set in the cells at `depth` over the
    camera's tiles. The area's corners sit half a cell inside the tiles' edges, so exactly the
    tiles' cells intersect it."""
    area = tiles_area(camera, (EXTENT[1] - EXTENT[0]) / (1 << depth) / 2)
    body = {"view": VIEW, "groupings": [{"count": True, "cells": {"depth": depth, "area": area}}]}
    f = combine(filter_leaves(filt))
    if f is not None:
        body["filters"] = f
    return body


def tables_request(camera: Camera, filt: Filter | None) -> dict:
    """The aggregation workload's body. The region is the camera's tiles, half a 32-bit step inside
    each edge, so after quantisation it is closed on exactly the tiles' positions."""
    region = {"region": {"bbox": tiles_area(camera, (EXTENT[1] - EXTENT[0]) / FIXED_SPAN / 2)}}
    lo, hi = YEAR_BINS
    return {"view": VIEW, "filters": combine([region] + filter_leaves(filt)), "groupings": [
        {"count": True, "by": {"field": "type", "top": TYPE_TOP}},
        {"count": True, "by": {"field": "publication_year", "bins": hi - lo, "range": [lo, hi]}},
    ]}


def split_frames(body: bytes):
    off, frames = 0, []
    while off < len(body):
        kind = body[off]
        (n,) = struct.unpack_from("<I", body, off + 1)
        frames.append((kind, body[off + 5: off + 5 + n]))
        off += 5 + n
    if not frames or frames[-1][0] != 4:
        raise ValueError("body has no trailer: incomplete")
    return frames


class Mosaica(Adapter):
    access = ("view point_visibility = the licence field; a catalogue principal granted read and "
              "exactly the viewer's terms, logged in with its API key at /v1/login; requests "
              "carry its session token and no access condition")

    def __init__(self, tier: Path, work: Path, binary: Path, grid: str = "underlay", nice: int = 0):
        super().__init__(tier, work, nice)
        if grid not in ("underlay", "aggregate"):
            raise ValueError(f"grid is underlay or aggregate, not {grid}")
        self.grid = grid
        self.name = "mosaica" if grid == "underlay" else "mosaica+aggregate"
        self.pool = ThreadPoolExecutor(2)
        self.binary = binary.resolve()
        self.cred = secrets.token_urlsafe(24)
        self.api_key: str | None = None
        self.proc: subprocess.Popen | None = None
        self.log = None
        self.unit = None
        self.http: requests.Session | None = None
        self.agg_http: requests.Session | None = None
        self.regions: set[str] = set()

    def version(self) -> dict:
        out = subprocess.run([str(self.binary), "--version"], capture_output=True, text=True, check=True)
        # Written beside the binary when it is built (README step 4); the toolchain on the PATH now
        # may not be the one that built it.
        rustc = self.binary.with_name(self.binary.name + ".rustc")
        return {"version": out.stdout.strip(),
                "rustc": rustc.read_text().strip() if rustc.exists() else None}

    def config(self) -> dict:
        return {"serve": SERVE, "corpus": CORPUS, "point_rows": [], "grid": self.grid,
                "region_answers": sorted(self.regions),
                "routes": ["/v1/login", "/v1/viewport", "/v1/aggregate"]}

    def _env(self) -> dict:
        return dict(os.environ, **{CRED_ENV: self.cred})

    def _write_deployment(self) -> None:
        (self.work / "corpus.toml").write_text(CORPUS)
        serve = "\n".join(f"{k} = {v}" for k, v in SERVE.items())
        (self.work / "mosaica.toml").write_text(f"""
[bundle]
path  = "bundle"
cache = ".mosaica/cache"
wal   = ".mosaica/wal.log"

[build]
schema = "corpus.toml"

[disclosure]
token_max_lifetime = 86400

[catalogue]
dir = ".mosaica/catalogue"

[serve]
viewer  = "127.0.0.1:{PORTS[0]}"
session = "127.0.0.1:{PORTS[1]}"
control = "127.0.0.1:{PORTS[2]}"
operator_credential_env = "{CRED_ENV}"
{serve}
""")

    def build(self) -> dict:
        subprocess.run(["rm", "-rf", str(self.work / "bundle"), str(self.work / ".mosaica")], check=True)
        self.api_key = None
        self._write_deployment()
        unit = f"bench-compare-mosaica-build-{os.getpid()}-{int(time.time())}"
        peak = self.work / "build.cgroup-peak"
        rss = self.work / "build.time"
        q = lambda p: shlex.quote(str(p))
        script = (
            f"/usr/bin/time -v -o {q(rss)} {q(self.binary)} build --deployment {q(self.work / 'mosaica.toml')} "
            f"--file {q(f'points={self.tier}')} --no-oracle-pairs "
            f"--stage-timings-json {q(self.work / 'build-stages.json')} > {q(self.work / 'build.log')} 2>&1; "
            f"rc=$?; "
            + (f"cat /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/memory.peak > {q(peak)}; "
               if user_scopes() else f"rm -f {q(peak)}; ")
            + "exit $rc"
        )
        t0 = time.monotonic()
        r = subprocess.run([*scoped(unit), "nice", "-n", str(self.nice), "bash", "-c", script],
                           cwd=self.work, env=self._env())
        build_s = time.monotonic() - t0
        if r.returncode:
            raise RuntimeError(f"mosaica build failed; see {self.work / 'build.log'}")
        max_rss = next(int(line.split()[-1]) * 1024 for line in rss.read_text().splitlines()
                       if "Maximum resident set size" in line)
        return {
            "build_s": build_s,
            "peak_rss_bytes": max_rss,
            "cgroup_peak_bytes": read_int(peak),
            "disk_bytes": du_bytes(self.work / "bundle"),
            "nice": self.nice,
            "memory_cap": "16G, no swap, in a systemd scope" if user_scopes()
                          else "none: no systemd user manager, so no scope and no cgroup figures",
        }

    def _control(self, method: str, path: str, body: dict) -> dict:
        r = requests.request(method, f"http://127.0.0.1:{PORTS[2]}{path}", json=body,
                             headers={"Authorization": f"Bearer {self.cred}"}, timeout=60)
        if r.status_code != 200:
            raise RuntimeError(f"{method} {path} {r.status_code}: {r.text[:500]}")
        return r.json()

    def _make_viewer(self) -> None:
        """The viewer as a catalogue principal holding `read` and the viewer's terms, and its key."""
        name = VIEWER["principal"]
        self._control("POST", "/control/principals", {"name": name, "kind": "service"})
        self._control("POST", "/control/grants", {"principal": name, "permission": "read"})
        self._control("POST", "/control/grants", {"principal": name, "terms": VIEWER["terms"]})
        self.api_key = self._control("POST", f"/control/principals/{name}/keys", {})["key"]

    def start(self) -> dict:
        """Start the server and log the viewer in. Returns the seconds to the server's ready line
        and to the session token."""
        t0 = time.perf_counter()
        self.unit = f"bench-compare-mosaica-serve-{os.getpid()}-{int(time.time() * 1000)}"
        self.log = open(self.work / "serve.log", "ab")
        self.proc = subprocess.Popen(
            [*scoped(self.unit), str(self.binary), "serve", "--deployment",
             str(self.work / "mosaica.toml")],
            cwd=self.work, env=self._env(), stdout=subprocess.PIPE,
            stderr=self.log, text=True)
        line = self.proc.stdout.readline()
        if not line:
            raise RuntimeError(f"mosaica serve exited; see {self.work / 'serve.log'}")
        ready = time.perf_counter() - t0
        if self.api_key is None:
            self._make_viewer()
        t1 = time.perf_counter()
        r = requests.post(f"http://127.0.0.1:{PORTS[0]}/v1/login", json={"api_key": self.api_key},
                          timeout=60)
        r.raise_for_status()
        auth = {"Authorization": f"Bearer {r.json()['token']}"}
        self.http, self.agg_http = requests.Session(), requests.Session()
        self.http.headers.update(auth)
        self.agg_http.headers.update(auth)
        return {"ready_s": ready, "login_s": time.perf_counter() - t1}

    def stop(self) -> None:
        for session in (self.http, self.agg_http):
            if session is not None:
                session.close()
        self.http = self.agg_http = None
        if self.proc is not None:
            self.proc.send_signal(signal.SIGTERM)
            try:
                self.proc.wait(timeout=60)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait()
            self.proc.stdout.close()
            self.proc = None
        if self.log is not None:
            self.log.close()
            self.log = None

    def serve_memory(self) -> dict:
        """None for a figure that cannot be read, such as from a server that has stopped."""
        if self.proc is None:
            return {"peak_rss_bytes": None, "cgroup_peak_bytes": None}
        return {"peak_rss_bytes": vm_hwm(self.proc.pid),
                "cgroup_peak_bytes": read_int(scope_dir(self.unit) / "memory.peak")}

    def evict(self) -> None:
        subprocess.run(["rm", "-rf", str(self.work / ".mosaica/cache")], check=True)
        evict_files(self.work / "bundle")

    def frame(self, camera: Camera, filt: Filter | None) -> tuple[Frame, float]:
        z, dg = camera.zoom, grid_depth(camera.zoom)
        tiles = camera.tiles()
        body = {"view": VIEW, "zoom": z, "tiles": [interleave(tx, ty) for tx, ty in tiles],
                "k": K, "point_rows": []}
        if dg > z and self.grid == "underlay":
            body["underlay_offset"] = dg - z
        f = combine(filter_leaves(filt))
        if f is not None:
            body["filters"] = f
        t0 = time.perf_counter()
        if self.grid == "underlay":
            raw, agg = self._viewport(body), None
        else:
            fv = self.pool.submit(self._viewport, body)
            fa = self.pool.submit(self._aggregate, aggregate_request(camera, filt, dg))
            raw, agg = fv.result(), fa.result()
        elapsed = time.perf_counter() - t0
        frame = self._decode(raw, z, dg)
        if agg is not None:
            frame.grid = {cell: n for cell, n in self._decode_cells(agg).items() if n}
            frame.bytes += sum(len(b) for b in agg)
        return frame, elapsed

    def tables(self, camera: Camera, filt: Filter | None) -> tuple[dict, float]:
        t0 = time.perf_counter()
        bodies = self._aggregate(tables_request(camera, filt))
        elapsed = time.perf_counter() - t0
        return self._decode_tables(bodies), elapsed

    def _viewport(self, body: dict) -> bytes:
        r = self.http.post(f"http://127.0.0.1:{PORTS[0]}/v1/viewport", json=body, timeout=600)
        if r.status_code != 200:
            raise RuntimeError(f"viewport {r.status_code}: {r.content[:500]!r}")
        if "x-mosaica-region" in r.headers:
            self.regions.add(r.headers["x-mosaica-region"])
        return r.content

    def _aggregate(self, body: dict) -> list[bytes]:
        """Every response of one aggregate read, following the cursor to the end."""
        out = []
        while True:
            r = self.agg_http.post(f"http://127.0.0.1:{PORTS[0]}/v1/aggregate", json=body, timeout=600)
            if r.status_code != 200:
                raise RuntimeError(f"aggregate {r.status_code}: {r.content[:500]!r}")
            if "x-mosaica-region" in r.headers:
                self.regions.add(r.headers["x-mosaica-region"])
            out.append(r.content)
            trailer = json.loads(split_frames(r.content)[-1][1])
            if not trailer.get("next"):
                return out
            body = dict(body, cursor=trailer["next"])

    @staticmethod
    def _records(bodies: list[bytes]) -> dict[int, list]:
        """Each table's record batches, by its grouping index."""
        tables: dict[int, list] = {}
        current = None
        for raw in bodies:
            for kind, payload in split_frames(raw):
                if kind == 9:
                    current = json.loads(payload)["grouping"]
                    tables.setdefault(current, [])
                elif kind == 7:
                    tables[current].append(ipc.open_stream(io.BytesIO(payload)).read_all())
                elif kind not in (8, 4):
                    raise ValueError(f"unknown frame kind {kind}")
        return tables

    @classmethod
    def _decode_cells(cls, bodies: list[bytes]) -> dict:
        grid = {}
        for table in cls._records(bodies).get(0, []):
            gx, gy = deinterleave(table["cell"].to_numpy())
            for a, b, n in zip(gx.tolist(), gy.tolist(), table["count"].to_numpy().tolist()):
                grid[(a, b)] = n
        return grid

    @classmethod
    def _decode_tables(cls, bodies: list[bytes]) -> dict:
        """The two tables in the oracle's shape: listed rows by key or lower edge, then `rest`
        and `none`."""
        out = {}
        for index, name, column in ((0, "type", "key"), (1, "year", "lower")):
            rows: dict[str, int] = {}
            for table in cls._records(bodies).get(index, []):
                for r in table.select(["group", column, "count"]).to_pylist():
                    k = str(r[column]) if r["group"] == "listed" else r["group"]
                    rows[k] = rows.get(k, 0) + r["count"]
            out[name] = rows
        return out

    @staticmethod
    def _decode(raw: bytes, z: int, dg: int) -> Frame:
        tiles, grid, points = {}, {}, {}
        for kind, payload in split_frames(raw):
            if kind == 4:
                continue
            table = ipc.open_stream(io.BytesIO(payload)).read_all()
            if kind == 1:
                tx, ty = deinterleave(table["tile"].to_numpy())
                for a, b, m in zip(tx.tolist(), ty.tolist(), table["matched"].to_numpy().tolist()):
                    if m:
                        tiles[(a, b)] = m
            elif kind == 2:
                gx, gy = deinterleave(table["cell"].to_numpy())
                for a, b, n in zip(gx.tolist(), gy.tolist(), table["count"].to_numpy().tolist()):
                    grid[(a, b)] = n
            elif kind == 3:
                qx, qy = deinterleave(table["code"].to_numpy())
                for t, pts in points_by_tile(qx, qy, z).items():
                    points[t] = np.concatenate([points[t], pts]) if t in points else pts
            else:
                raise ValueError(f"unknown frame kind {kind}")
        if dg == z:
            grid = dict(tiles)
        return Frame(tiles=tiles, grid=grid, points=points, bytes=len(raw))
