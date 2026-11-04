"""The exact frame and aggregation tables for any camera and filter, computed by DuckDB over the
tier's Parquet file for the viewer in `viewer.json`, and the comparison of a system's answer with
them.

The oracle reads only the tier file, the data set's viewer file and `frame.py`'s definitions: the
positions `qx` and `qy` that the prepare script quantised, the attributes, and the frame's tile
and grid rules. It shares no code with any adapter's query. The viewer's restriction is a plain
`licence IN (...)` here, or for a list of terms `list_has_any(access, [...])`, a row being visible
where the viewer holds any of its terms; every system must reach the same rows through its own
access control.

With `copy`, the oracle first copies the viewer's visible rows' positions and attributes into a
DuckDB table ordered by position, kept beside the tier and reused while the tier and the viewer
are unchanged, and answers from it: at 10⁹ rows a scan of the tier for every query is too slow.
"""

from __future__ import annotations

import json
import math
from pathlib import Path

import duckdb
import numpy as np
import pyarrow as pa

from dataset import DATASET
from frame import (EXTENT, K, SUBCELL_OFFSET, TYPE_TOP, YEAR_BINS, Camera, Filter, Frame, fixed32,
                   grid_depth, points_by_tile)

VIEWER = json.loads(Path(__file__).with_name(DATASET.viewer).read_text())
#: The aggregation workload's two tables: the category's counts and the year histogram.
TABLES = (DATASET.category, "year")
#: The frame figures that count against an exact frame: the count errors, then the point errors.
COUNT_ERRORS = ("tile_abs_err", "grid_abs_err")
POINT_ERRORS = ("sparse_points_missing", "sparse_points_extra", "points_outside_tile",
                "dense_points_missing", "dense_points_extra", "dense_points_invalid")


def visible_sql() -> str:
    terms = ", ".join(f"'{t}'" for t in VIEWER["terms"])
    if DATASET.access_is_list:
        return f"list_has_any({DATASET.access}, [{terms}])"
    return f"{DATASET.access} IN ({terms})"


def where(camera: Camera, filt: Filter | None, visible: str | None = None) -> str:
    """The SQL predicate for the viewer (`visible`, by default `visible_sql()`), the camera's tiles
    and the filter."""
    z = camera.zoom
    tx0, tx1, ty0, ty1 = camera.tile_range()
    s = 32 - z
    lo = lambda t: t << s
    hi = lambda t: ((t + 1) << s) - 1
    terms = [visible or visible_sql(), f"qx BETWEEN {lo(tx0)} AND {hi(tx1)}", f"qy BETWEEN {lo(ty0)} AND {hi(ty1)}"]
    for leaf in (filt.leaves if filt else []):
        terms.append(leaf_sql(leaf))
    return " AND ".join(terms)


def leaf_sql(leaf: dict) -> str:
    op, col, value = leaf["op"], leaf.get("col"), leaf["value"]
    if op == "eq" and col == DATASET.category:
        return f"{col} = '{value}'"
    if op == "in" and col == DATASET.category:
        return f"{col} IN (" + ", ".join(f"'{v}'" for v in value) + ")"
    if op == "eq" and col == DATASET.flag:
        return f"{col} = {'true' if value else 'false'}"
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


def summed(rows) -> dict:
    """Each group's count, by its key (every column but the last), adding the counts of a key
    returned more than once. DuckDB 1.3.2's parallel `GROUP BY` returned some groups twice over
    the GBIF copy, each with part of the group's count, so no caller takes one row a group."""
    out: dict = {}
    for *key, n in rows:
        k = tuple(int(v) if isinstance(v, int) else v for v in key)
        k = k[0] if len(k) == 1 else k
        out[k] = out.get(k, 0) + int(n)
    return out


def _spread16(v: str) -> str:
    """A 16-bit value's bits on the even bits of a 32-bit one, in DuckDB."""
    for shift, mask in ((8, 0x00FF00FF), (4, 0x0F0F0F0F), (2, 0x33333333), (1, 0x55555555)):
        v = f"(({v}) | (({v}) << {shift})) & {mask}"
    return v


#: The Morton code of a position's top 16 bits of each axis, x on the even bits, in DuckDB.
MORTON16 = f"({_spread16('qx::UBIGINT >> 16')}) | (({_spread16('qy::UBIGINT >> 16')}) << 1)"


class Oracle:
    def __init__(self, tier_path: str, threads: int = 4, copy: bool = False):
        self.tier = tier_path
        #: The relation every query reads, and the viewer's predicate over it.
        self.src, self.visible_where = f"read_parquet('{tier_path}')", visible_sql()
        if copy:
            self.con = duckdb.connect(self._copy(threads), read_only=True)
            self.con.execute("SET memory_limit = '8GB'")
            self.src, self.visible_where = "visible", "TRUE"
        else:
            self.con = duckdb.connect()
        self.con.execute(f"SET threads = {threads}")
        self._cache: dict[str, Frame] = {}
        self._tables: dict[str, dict] = {}
        self._subcells: dict[str, dict] = {}
        self._invalid: dict[tuple, int] = {}

    def _copy(self, threads: int) -> str:
        """The DuckDB file holding the viewer's visible rows ordered by `MORTON16`, made where the
        one beside the tier was made from another tier file or another viewer."""
        path = Path(self.tier).with_suffix(f".oracle-{DATASET.access}.duckdb")
        stat = Path(self.tier).stat()
        made_from = json.dumps({"tier": Path(self.tier).name, "bytes": stat.st_size,
                                "mtime_ns": stat.st_mtime_ns, "visible": visible_sql()})
        if path.exists():
            with duckdb.connect(str(path), read_only=True) as con:
                try:
                    if con.execute("SELECT made_from FROM meta").fetchone()[0] == made_from:
                        return str(path)
                except duckdb.Error:
                    pass
            path.unlink()
        columns = ", ".join(["qx", "qy", DATASET.category, DATASET.year] + [DATASET.flag] * bool(DATASET.flag))
        tmp = path.with_suffix(".tmp")
        tmp.unlink(missing_ok=True)
        with duckdb.connect(str(tmp)) as con:
            con.execute(f"SET threads = {threads}")
            con.execute("SET memory_limit = '8GB'")
            con.execute("SET preserve_insertion_order = false")
            con.execute(f"CREATE TABLE visible AS SELECT {columns} FROM read_parquet('{self.tier}') "
                        f"WHERE {visible_sql()} ORDER BY {MORTON16}")
            rows = con.execute(f"SELECT count(*) FROM read_parquet('{self.tier}')").fetchone()[0]
            con.execute("CREATE TABLE meta (made_from VARCHAR, tier_rows BIGINT)")
            con.execute("INSERT INTO meta VALUES (?, ?)", [made_from, rows])
        tmp.rename(path)
        return str(path)

    def where(self, camera: Camera, filt: Filter | None) -> str:
        return where(camera, filt, self.visible_where)

    def count(self, camera: Camera, filt: Filter | None) -> int:
        return int(self.con.execute(
            f"SELECT count(*) FROM {self.src} WHERE {self.where(camera, filt)}").fetchone()[0])

    def visible(self) -> tuple[int, int]:
        """The viewer's visible rows, and all rows."""
        if self.src == "visible":
            return (int(self.con.execute("SELECT count(*) FROM visible").fetchone()[0]),
                    int(self.con.execute("SELECT tier_rows FROM meta").fetchone()[0]))
        return tuple(int(v) for v in self.con.execute(
            f"SELECT count(*) FILTER (WHERE {visible_sql()}), count(*) FROM {self.src}"
        ).fetchone())

    def frame(self, camera: Camera, filt: Filter | None) -> Frame:
        key = repr((camera.zoom, camera.cx, camera.cy, filt.leaves if filt else None))
        if key in self._cache:
            return self._cache[key]
        z, dg = camera.zoom, grid_depth(camera.zoom)
        w = self.where(camera, filt)
        rows = self.con.execute(
            f"SELECT qx >> {32 - dg} AS gx, qy >> {32 - dg} AS gy, count(*) FROM {self.src} "
            f"WHERE {w} GROUP BY ALL"
        ).fetchall()
        grid = summed(rows)
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
                f"SELECT qx, qy FROM {self.src} WHERE {w} AND ({pick})"
            ).fetchnumpy()
            points = points_by_tile(pts["qx"], pts["qy"], z)
        frame = Frame(tiles=tiles, grid=grid, points=points)
        self._cache[key] = frame
        return frame

    def dense_invalid(self, camera: Camera, filt: Filter | None, got: Frame, truth: Frame) -> int:
        """The positions a system sent from dense tiles that are not the viewer's matching items
        there, counted as a multiset: a position sent more often than the matching items hold it
        counts each time over."""
        sent = [p for t, p in got.points.items() if truth.tiles.get(t, 0) > K and len(p)]
        if not sent:
            return 0
        pts = np.concatenate(sent)
        # The same positions sent again for the same step, as the timed passes send them, have
        # the same answer.
        key = (repr((camera.zoom, camera.cx, camera.cy, filt.leaves if filt else None)),
               np.sort(np.ascontiguousarray(pts).view(np.uint64).ravel()).tobytes())
        if key in self._invalid:
            return self._invalid[key]
        positions = pa.table({"qx": pts[:, 0], "qy": pts[:, 1]})
        self.con.register("sent", positions)
        try:
            held = summed(self.con.execute(
                f"SELECT qx, qy, count(*) FROM {self.src} WHERE {self.where(camera, filt)} "
                f"AND (qx, qy) IN (SELECT (qx, qy) FROM sent) GROUP BY ALL").fetchall())
        finally:
            self.con.unregister("sent")
        wanted = summed((int(a), int(b), 1) for a, b in pts.tolist())
        self._invalid[key] = sum(max(n - held.get(p, 0), 0) for p, n in wanted.items())
        return self._invalid[key]

    def coverage(self, camera: Camera, filt: Filter | None, got: Frame, truth: Frame) -> dict:
        """How the positions sent from dense tiles spread over the sub-cells `SUBCELL_OFFSET`
        levels below each tile, against a uniform random sample of as many of the tile's matching
        items, drawn without replacement. Two figures, summed over the dense tiles:

        * `dense_cover` against `dense_cover_expected`: the sub-cells the sample falls in, and the
          number a uniform sample falls in on average. A sample taken from a corner of the tile
          covers far fewer; one spread more evenly than chance, more.
        * `dense_chi2` on `dense_chi2_df` degrees of freedom: Pearson's statistic of the sample's
          count in each sub-cell against its expectation, `k * n / N` for a sub-cell holding `n` of
          the tile's `N` items, scaled by `(N - 1) / (N - k)` for drawing without replacement. A
          uniform sample gives about one a degree of freedom, a clumped one more and one spread
          evenly less; the statistic is approximate where a sub-cell expects few points."""
        z = camera.zoom
        dense = [t for t, n in truth.tiles.items() if n > K and len(got.points.get(t, ()))]
        out = {"dense_tiles_sampled": len(dense), "dense_cover": 0, "dense_cover_expected": 0.0,
               "dense_chi2": 0.0, "dense_chi2_df": 0}
        if not dense:
            return out
        d = min(z + SUBCELL_OFFSET, 32)
        key = repr((z, camera.cx, camera.cy, filt.leaves if filt else None))
        if key not in self._subcells:
            rows = self.con.execute(
                f"SELECT qx::UBIGINT >> {32 - d}, qy::UBIGINT >> {32 - d}, count(*) "
                f"FROM {self.src} WHERE {self.where(camera, filt)} GROUP BY ALL").fetchall()
            per: dict[tuple[int, int], dict[tuple[int, int], int]] = {}
            for (sx, sy), n in summed(rows).items():
                per.setdefault((sx >> (d - z), sy >> (d - z)), {})[(sx, sy)] = n
            self._subcells[key] = per
        per = self._subcells[key]
        for t in dense:
            sent = got.points[t].astype(np.uint64) >> np.uint64(32 - d)
            counts: dict[tuple[int, int], int] = {}
            for a, b in sent.tolist():
                counts[(a, b)] = counts.get((a, b), 0) + 1
            cells = per.get(t, {})
            total = truth.tiles[t]
            k = min(len(sent), total)
            out["dense_cover"] += len(counts)
            chi2 = 0.0
            for cell, n in cells.items():
                rest = total - n
                # The chance that none of k items drawn without replacement falls in this cell.
                miss = 0.0 if rest < k else math.exp(math.lgamma(rest + 1) - math.lgamma(rest - k + 1)
                                                     - math.lgamma(total + 1) + math.lgamma(total - k + 1))
                out["dense_cover_expected"] += 1 - miss
                want = k * n / total
                chi2 += (counts.get(cell, 0) - want) ** 2 / want
            out["dense_chi2"] += chi2 * (total - 1) / (total - k)
            out["dense_chi2_df"] += len(cells) - 1
        return out

    def tables(self, camera: Camera, filt: Filter | None) -> dict[str, dict[str, int]]:
        """The aggregation workload's two tables over the viewer's matching items in the camera's
        tiles, named by `TABLES`: the category's, each value's count and `none` for no value
        (every value is listed, since there are `TYPE_TOP` of them), and `year`, each one-year
        bin's count by its lower edge, `rest` for a year outside `YEAR_BINS` and `none` for no
        year. Zero counts are left out."""
        key = repr((camera.zoom, camera.cx, camera.cy, filt.leaves if filt else None))
        if key in self._tables:
            return self._tables[key]
        w = self.where(camera, filt)
        src, category, year = self.src, DATASET.category, DATASET.year
        types = self.con.execute(
            f"SELECT coalesce({category}, 'none'), count(*) FROM {src} WHERE {w} GROUP BY 1").fetchall()
        lo, hi = YEAR_BINS
        years = self.con.execute(
            f"SELECT CASE WHEN {year} IS NULL THEN 'none' "
            f"WHEN {year} < {lo} OR {year} >= {hi} THEN 'rest' "
            f"ELSE {year}::VARCHAR END, count(*) FROM {src} WHERE {w} GROUP BY 1").fetchall()
        out = {TABLES[0]: summed(types), TABLES[1]: summed(years)}
        assert len(out[TABLES[0]]) <= TYPE_TOP + 1
        self._tables[key] = out
        return out


def chi2_z(chi2: float, df: int) -> float:
    """The standard normal deviate of `chi2` on `df` degrees of freedom, by Wilson and Hilferty's
    cube-root approximation: beyond about 3 either way, the sample is unlike a uniform one."""
    if df <= 0:
        return 0.0
    v = 2 / (9 * df)
    return ((chi2 / df) ** (1 / 3) - (1 - v)) / math.sqrt(v)


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


def compare(got: Frame, truth: Frame, camera: Camera, dense: str = "points") -> dict:
    """How far a system's frame is from the oracle's. Every figure is zero for an exact frame,
    except `points_dense`, the positions sent from dense tiles. Under the `points` frame a dense
    tile must send exactly `K`, and under `counts` up to `K`; whether those are the viewer's
    matching items, each sent once, is `Oracle.dense_invalid`. A tile with points sent and no true
    count is a sparse tile from which nothing was wanted."""
    tile_keys = set(got.tiles) | set(truth.tiles)
    tile_err = sum(abs(got.tiles.get(t, 0) - truth.tiles.get(t, 0)) for t in tile_keys)
    cell_keys = set(got.grid) | set(truth.grid)
    diffs = [abs(got.grid.get(c, 0) - truth.grid.get(c, 0)) for c in cell_keys]
    wanted = set(camera.tiles())
    none = np.empty((0, 2), np.uint32)
    sparse_missing = sparse_extra = outside = dense_sent = dense_missing = dense_extra = 0
    for t, pts in got.points.items():
        if t not in wanted:
            outside += len(pts)
    for t in (set(truth.tiles) | set(got.points)) & wanted:
        sent = got.points.get(t, none)
        if truth.tiles.get(t, 0) <= K:
            want = truth.points.get(t, none)
            common = common_count(want, sent)
            sparse_missing += len(want) - common
            sparse_extra += len(sent) - common
        else:
            dense_extra += max(0, len(sent) - K)
            if dense == "points":
                dense_missing += max(0, K - len(sent))
            dense_sent += len(sent)
    return {
        "matched_true": sum(truth.tiles.values()),
        "tile_abs_err": tile_err,
        "grid_abs_err": int(sum(diffs)),
        "grid_cells_wrong": int(sum(1 for d in diffs if d)),
        "grid_cells_true": len(truth.grid),
        "sparse_points_missing": sparse_missing,
        "sparse_points_extra": sparse_extra,
        "points_outside_tile": outside,
        "dense_points_missing": dense_missing,
        "dense_points_extra": dense_extra,
        "points_dense": dense_sent,
        "points_sent": int(sum(len(p) for p in got.points.values())),
    }
