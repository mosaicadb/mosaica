"""The exact frame and aggregation tables for any camera and filter, computed by DuckDB over the
tier's Parquet file for the viewer in `viewer.json`, and the comparison of a system's answer with
them.

The oracle reads only the tier file, `viewer.json` and `frame.py`'s definitions: the positions `qx`
and `qy` that `prepare.py` quantised, the attributes, and the frame's tile and grid rules. It
shares no code with any adapter's query. The viewer's restriction is a plain `licence IN (...)`
here; every system must reach the same rows through its own access control.
"""

from __future__ import annotations

import json
from pathlib import Path

import duckdb
import numpy as np

from frame import (EXTENT, K, TYPE_TOP, YEAR_BINS, Camera, Filter, Frame, fixed32, grid_depth,
                   points_by_tile, tile_of)

VIEWER = json.loads(Path(__file__).with_name("viewer.json").read_text())


def visible_sql() -> str:
    return "licence IN (" + ", ".join(f"'{t}'" for t in VIEWER["terms"]) + ")"


def where(camera: Camera, filt: Filter | None) -> str:
    """The SQL predicate for the viewer, the camera's tiles and the filter."""
    z = camera.zoom
    tx0, tx1, ty0, ty1 = camera.tile_range()
    s = 32 - z
    lo = lambda t: t << s
    hi = lambda t: ((t + 1) << s) - 1
    terms = [visible_sql(), f"qx BETWEEN {lo(tx0)} AND {hi(tx1)}", f"qy BETWEEN {lo(ty0)} AND {hi(ty1)}"]
    for leaf in (filt.leaves if filt else []):
        terms.append(leaf_sql(leaf))
    return " AND ".join(terms)


def leaf_sql(leaf: dict) -> str:
    op, col, value = leaf["op"], leaf.get("col"), leaf["value"]
    if op == "eq" and col == "type":
        return f"type = '{value}'"
    if op == "eq" and col == "is_oa":
        return f"is_oa = {'true' if value else 'false'}"
    if op == "range":
        parts = []
        if "gte" in value:
            parts.append(f"{col} >= {int(value['gte'])}")
        if "lt" in value:
            parts.append(f"{col} < {int(value['lt'])}")
        return " AND ".join(parts)
    if op == "bbox":
        x0, y0, x1, y1 = value
        return (f"qx BETWEEN {fixed32(x0)} AND {fixed32(x1)} AND "
                f"qy BETWEEN {fixed32(y0, EXTENT[2], EXTENT[3])} AND {fixed32(y1, EXTENT[2], EXTENT[3])}")
    raise ValueError(f"unknown filter leaf {leaf}")


class Oracle:
    def __init__(self, tier_path: str, threads: int = 4):
        self.con = duckdb.connect()
        self.con.execute(f"SET threads = {threads}")
        self.tier = tier_path
        self._cache: dict[str, Frame] = {}

    def count(self, camera: Camera, filt: Filter | None) -> int:
        return int(self.con.execute(
            f"SELECT count(*) FROM read_parquet('{self.tier}') WHERE {where(camera, filt)}").fetchone()[0])

    def visible(self) -> tuple[int, int]:
        """The viewer's visible rows, and all rows."""
        return tuple(int(v) for v in self.con.execute(
            f"SELECT count(*) FILTER (WHERE {visible_sql()}), count(*) FROM read_parquet('{self.tier}')"
        ).fetchone())

    def frame(self, camera: Camera, filt: Filter | None) -> Frame:
        key = repr((camera.zoom, camera.cx, camera.cy, filt.leaves if filt else None))
        if key in self._cache:
            return self._cache[key]
        z, dg = camera.zoom, grid_depth(camera.zoom)
        w = where(camera, filt)
        rows = self.con.execute(
            f"SELECT qx >> {32 - dg} AS gx, qy >> {32 - dg} AS gy, count(*) FROM read_parquet('{self.tier}') "
            f"WHERE {w} GROUP BY ALL"
        ).fetchall()
        grid = {(int(gx), int(gy)): int(n) for gx, gy, n in rows}
        tiles: dict[tuple[int, int], int] = {}
        shift = dg - z
        for (gx, gy), n in grid.items():
            t = (gx >> shift, gy >> shift)
            tiles[t] = tiles.get(t, 0) + n
        sparse = [t for t, n in tiles.items() if n <= K]
        points: dict[tuple[int, int], np.ndarray] = {}
        if sparse:
            s = 32 - z
            pick = " OR ".join(f"(qx::UBIGINT >> {s} = {tx} AND qy::UBIGINT >> {s} = {ty})"
                               for tx, ty in sparse)
            pts = self.con.execute(
                f"SELECT qx, qy FROM read_parquet('{self.tier}') WHERE {w} AND ({pick})"
            ).fetchnumpy()
            points = points_by_tile(pts["qx"], pts["qy"], z)
        frame = Frame(tiles=tiles, grid=grid, points=points)
        self._cache[key] = frame
        return frame

    def tables(self, camera: Camera, filt: Filter | None) -> dict[str, dict[str, int]]:
        """The aggregation workload's two tables over the viewer's matching items in the camera's
        tiles: `type`, each type's count and `none` for no type (every type is listed, since
        there are `TYPE_TOP` of them), and `year`, each one-year bin's count by its lower edge,
        `rest` for a year outside `YEAR_BINS` and `none` for no year. Zero counts are left out."""
        w = where(camera, filt)
        src = f"read_parquet('{self.tier}')"
        types = self.con.execute(
            f"SELECT coalesce(type, 'none'), count(*) FROM {src} WHERE {w} GROUP BY 1").fetchall()
        lo, hi = YEAR_BINS
        years = self.con.execute(
            f"SELECT CASE WHEN publication_year IS NULL THEN 'none' "
            f"WHEN publication_year < {lo} OR publication_year >= {hi} THEN 'rest' "
            f"ELSE publication_year::VARCHAR END, count(*) FROM {src} WHERE {w} GROUP BY 1").fetchall()
        assert len(types) <= TYPE_TOP + 1
        return {"type": {k: int(n) for k, n in types}, "year": {k: int(n) for k, n in years}}


def compare_tables(got: dict[str, dict[str, int]], truth: dict[str, dict[str, int]]) -> dict:
    """How far a system's tables are from the oracle's: per table, the sum of absolute count
    differences over every key either holds, and the keys whose counts differ."""
    out = {}
    for name in truth:
        g = {k: v for k, v in got.get(name, {}).items() if v}
        t = truth[name]
        keys = set(g) | set(t)
        out[f"{name}_abs_err"] = int(sum(abs(g.get(k, 0) - t.get(k, 0)) for k in keys))
        out[f"{name}_keys_wrong"] = int(sum(1 for k in keys if g.get(k, 0) != t.get(k, 0)))
        out[f"{name}_total_true"] = int(sum(t.values()))
    return out


def common_count(a: np.ndarray, b: np.ndarray) -> int:
    """How many positions two (n, 2) arrays share, counting a repeated position as often as both
    hold it."""
    if not len(a) or not len(b):
        return 0
    ua, ca = np.unique(np.ascontiguousarray(a).view(np.uint64).ravel(), return_counts=True)
    ub, cb = np.unique(np.ascontiguousarray(b).view(np.uint64).ravel(), return_counts=True)
    _, ia, ib = np.intersect1d(ua, ub, assume_unique=True, return_indices=True)
    return int(np.minimum(ca[ia], cb[ib]).sum())


def compare(got: Frame, truth: Frame, camera: Camera) -> dict:
    """How far a system's frame is from the oracle's. Every figure is zero for an exact frame,
    except `points_dense`, the positions sent from dense tiles, which the frame allows. A tile
    with points sent and no true count is a sparse tile from which nothing was wanted."""
    tile_keys = set(got.tiles) | set(truth.tiles)
    tile_err = sum(abs(got.tiles.get(t, 0) - truth.tiles.get(t, 0)) for t in tile_keys)
    cell_keys = set(got.grid) | set(truth.grid)
    diffs = [abs(got.grid.get(c, 0) - truth.grid.get(c, 0)) for c in cell_keys]
    wanted = set(camera.tiles())
    none = np.empty((0, 2), np.uint32)
    sparse_missing = sparse_extra = outside = dense = 0
    for t, pts in got.points.items():
        if len(pts):
            tx, ty = tile_of(pts[:, 0], camera.zoom), tile_of(pts[:, 1], camera.zoom)
            outside += int(np.count_nonzero((tx != t[0]) | (ty != t[1])))
        if t not in wanted:
            outside += len(pts)
    for t in set(truth.tiles) | set(got.points):
        sent = got.points.get(t, none)
        if truth.tiles.get(t, 0) <= K:
            want = truth.points.get(t, none)
            common = common_count(want, sent)
            sparse_missing += len(want) - common
            sparse_extra += len(sent) - common
        else:
            sparse_extra += max(0, len(sent) - K)
            dense += len(sent)
    return {
        "matched_true": sum(truth.tiles.values()),
        "tile_abs_err": tile_err,
        "grid_abs_err": int(sum(diffs)),
        "grid_cells_wrong": int(sum(1 for d in diffs if d)),
        "grid_cells_true": len(truth.grid),
        "sparse_points_missing": sparse_missing,
        "sparse_points_extra": sparse_extra,
        "points_outside_tile": outside,
        "points_dense": dense,
        "points_sent": int(sum(len(p) for p in got.points.values())),
    }
