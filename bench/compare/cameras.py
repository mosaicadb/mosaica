"""The fixed camera sets every system is run over, derived from a tier's own density.

    python cameras.py --tier data/bench-compare/tiers/paperseek-1e6.parquet --out cameras/paperseek-1e6.json

Three workloads, each a list of (camera, filter) steps, all drawn from the items the viewer in
`viewer.json` may see:

* `battery`: at zooms 0, 4, 8, 12 and 16, one screen centred on the occupied depth-`zoom` tile at
  each density decile (the tile at quantile (d + 0.5) / 10 of the occupied tiles ordered by
  count, ties by tile coordinates), with no filter. Duplicates are dropped, so zoom 0 has one.
* `script`: the interactive bench's camera script. From the whole map, into each of the two
  densest depth-3 regions through zooms 3, 6, 9, 12 and 14, each step centred on the densest tile
  of that depth inside the previous one; a pan east by half a screen at 6, 9 and 14, and south too
  at 14; then back out through 12, 9, 6 and 3.
* `filters`: at the script's zoom-6 and zoom-12 screens of the densest region, a fixed sequence of
  filter changes: none; the work type nearest 50%, 5% and 0.1% of the viewer's items; a year
  below which about 50%, 5% and 0.1% lie; `is_oa` true and false; the commonest type with
  `is_oa` true and the 50% year onwards; boxes centred on the screen holding about 50%, 5% and
  0.1% of the screen's items; and none. Each step records the share of the viewer's items in the
  screen's tiles it matches. The aggregation workload runs over the same steps.
"""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path

import numpy as np

from frame import EXTENT, SCREEN, TILE_PX, Camera, Filter, fixed32
from oracle import VIEWER, Oracle, visible_sql
from oracle import where as oracle_where

BATTERY_ZOOMS = (0, 4, 8, 12, 16)
SCRIPT_ZOOMS = (3, 6, 9, 12, 14)
PANS = {6: ["east"], 9: ["east"], 14: ["east", "south"]}
SPAN = EXTENT[1] - EXTENT[0]


def tile_centre(tx: int, ty: int, z: int) -> tuple[float, float]:
    size = SPAN / (1 << z)
    return EXTENT[0] + (tx + 0.5) * size, EXTENT[2] + (ty + 0.5) * size


def tile_counts(con, tier: str, z: int, within: tuple[int, int, int] | None = None):
    shift = 32 - z
    where = f"WHERE {visible_sql()}"
    if within is not None:
        pz, ptx, pty = within
        s = 32 - pz
        where += f" AND qx::UBIGINT >> {s} = {ptx} AND qy::UBIGINT >> {s} = {pty}"
    return con.execute(
        f"SELECT qx::UBIGINT >> {shift} AS tx, qy::UBIGINT >> {shift} AS ty, count(*) AS n FROM read_parquet('{tier}') "
        f"{where} GROUP BY ALL ORDER BY n, tx, ty"
    ).fetchall()


def cam(z: int, cx: float, cy: float, label: str) -> dict:
    return {"zoom": z, "cx": cx, "cy": cy, "label": label}


def battery(con, tier: str) -> list[dict]:
    steps, seen = [], set()
    for z in BATTERY_ZOOMS:
        rows = tile_counts(con, tier, z)
        for d in range(10):
            tx, ty, n = rows[min(len(rows) - 1, int((d + 0.5) / 10 * len(rows)))]
            if (z, tx, ty) in seen:
                continue
            seen.add((z, tx, ty))
            cx, cy = tile_centre(tx, ty, z)
            steps.append({"camera": cam(z, cx, cy, f"z{z}-d{d}"), "filter": None,
                          "tile_count": n})
    return steps


def script(con, tier: str) -> tuple[list[dict], dict[tuple[int, int], dict]]:
    regions = tile_counts(con, tier, 3)[::-1][:2]
    steps = [{"camera": cam(0, *tile_centre(0, 0, 0), "whole"), "filter": None}]
    anchors = {}
    for r, (tx, ty, _) in enumerate(regions):
        parent = (3, tx, ty)
        path = []
        for z in SCRIPT_ZOOMS:
            if z > 3:
                best = tile_counts(con, tier, z, parent)[-1]
                parent = (z, best[0], best[1])
            cx, cy = tile_centre(parent[1], parent[2], z)
            c = cam(z, cx, cy, f"r{r}-z{z}")
            steps.append({"camera": c, "filter": None})
            anchors[(r, z)] = c
            path.append(c)
            half = SCREEN[0] / 2 / TILE_PX * SPAN / (1 << z)
            for i, direction in enumerate(PANS.get(z, [])):
                cx = cx + half if direction == "east" else cx
                cy = cy - SCREEN[1] / 2 / TILE_PX * SPAN / (1 << z) if direction == "south" else cy
                steps.append({"camera": cam(z, cx, cy, f"r{r}-z{z}-pan{i}"), "filter": None})
        for c in reversed(path[:-1]):
            steps.append({"camera": dict(c, label=c["label"] + "-out"), "filter": None})
    return steps, anchors


TARGETS = (0.5, 0.05, 0.001)


def closest(fractions: dict, target: float, taken: set) -> object:
    """The key whose fraction is nearest `target` by ratio, among those not taken."""
    return min((k for k in fractions if k not in taken and fractions[k] > 0),
               key=lambda k: (abs(math.log(fractions[k] / target)), str(k)))


def attribute_filters(con, tier: str) -> list[tuple[str, list[dict]]]:
    """The filters on attributes, chosen from the viewer's visible items over the whole tier: the
    work type nearest each target fraction, the year below which each target fraction lies, both
    values of `is_oa`, and one combination."""
    src = f"read_parquet('{tier}') WHERE {visible_sql()}"
    total = con.execute(f"SELECT count(*) FROM {src}").fetchone()[0]
    types = {t: n / total for t, n in con.execute(
        f"SELECT type, count(*) FROM {src} AND type IS NOT NULL GROUP BY 1").fetchall()}
    years = con.execute(f"SELECT publication_year, count(*) FROM {src} AND publication_year IS NOT NULL "
                        f"GROUP BY 1 ORDER BY 1").fetchall()
    below, acc = {}, 0
    for y, n in years:
        below[y] = acc / total
        acc += n
    out, taken = [], set()
    for target in TARGETS:
        t = closest(types, target, taken)
        taken.add(t)
        out.append((f"type={t}", [{"col": "type", "op": "eq", "value": t}]))
    cuts = []
    for target in TARGETS:
        y = closest(below, target, set(cuts))
        cuts.append(y)
        out.append((f"year<{y}", [{"col": "publication_year", "op": "range", "value": {"lt": int(y)}}]))
    for v in (True, False):
        out.append((f"is_oa={str(v).lower()}", [{"col": "is_oa", "op": "eq", "value": v}]))
    commonest = max(types, key=types.get)
    out.append((f"type={commonest}&is_oa=true&year>={cuts[0]}", [
        {"col": "type", "op": "eq", "value": commonest},
        {"col": "is_oa", "op": "eq", "value": True},
        {"col": "publication_year", "op": "range", "value": {"gte": int(cuts[0])}}]))
    return out


def boxes(oracle: Oracle, c: dict) -> list[tuple[str, list[dict]]]:
    """Boxes centred on the screen, with its aspect, holding about each target fraction of the
    viewer's items in the screen's tiles."""
    camera = Camera(c["zoom"], c["cx"], c["cy"])
    pts = oracle.con.execute(f"SELECT qx, qy FROM read_parquet('{oracle.tier}') "
                             f"WHERE {oracle_where(camera, None)}").fetchnumpy()
    qx, qy = pts["qx"].astype(np.int64), pts["qy"].astype(np.int64)
    x0, y0, x1, y1 = camera.bbox()
    hw, hh = (x1 - x0) / 2, (y1 - y0) / 2

    def box(scale: float) -> list[float]:
        return [c["cx"] - hw * scale, c["cy"] - hh * scale, c["cx"] + hw * scale, c["cy"] + hh * scale]

    def share(b: list[float]) -> float:
        inside = ((qx >= fixed32(b[0])) & (qx <= fixed32(b[2]))
                  & (qy >= fixed32(b[1], EXTENT[2], EXTENT[3])) & (qy <= fixed32(b[3], EXTENT[2], EXTENT[3])))
        return float(np.count_nonzero(inside)) / max(1, len(qx))

    out = []
    for target in TARGETS:
        lo, hi = 0.0, 4.0
        for _ in range(60):
            mid = (lo + hi) / 2
            lo, hi = (mid, hi) if share(box(mid)) < target else (lo, mid)
        out.append((f"box{target:g}", [{"op": "bbox", "value": box(hi)}]))
    return out


def filter_sequence(oracle: Oracle, c: dict, attributes: list) -> list[dict]:
    """The filter changes at one screen, each with the share of the viewer's items in the screen's
    tiles it matches."""
    leaves = [("none", [])] + attributes + boxes(oracle, c) + [("none-again", [])]
    camera = Camera(c["zoom"], c["cx"], c["cy"])
    whole = Camera(0, *tile_centre(0, 0, 0))
    visible, everywhere = oracle.count(camera, None), oracle.count(whole, None)
    steps = []
    for label, lv in leaves:
        f = Filter(lv, label)
        step = {"camera": c, "filter": {"label": label, "leaves": lv},
                "fraction": round(oracle.count(camera, f) / max(1, visible), 6)}
        if not any(leaf["op"] == "bbox" for leaf in lv):
            step["fraction_of_tier"] = round(oracle.count(whole, f) / everywhere, 6)
        steps.append(step)
    return steps


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--tier", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    tier = str(args.tier.resolve())
    oracle = Oracle(tier)
    con = oracle.con
    steps, anchors = script(con, tier)
    attributes = attribute_filters(con, tier)
    out = {
        "tier": args.tier.name,
        "viewer": VIEWER,
        "battery": battery(con, tier),
        "script": steps,
        "filters": (filter_sequence(oracle, anchors[(0, 6)], attributes)
                    + filter_sequence(oracle, anchors[(0, 12)], attributes)),
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(out, indent=1) + "\n")
    print({k: len(v) for k, v in out.items() if isinstance(v, list)})


if __name__ == "__main__":
    main()
