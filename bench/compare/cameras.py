"""The fixed camera sets every system is run over, derived from a tier's own density.

    python cameras.py --tier data/bench-compare/tiers/osm-1e7.parquet --out cameras/osm-1e7.json

Three workloads, each a list of (camera, filter) steps:

* `battery`: at zooms 0, 4, 8, 12 and 16, one screen centred on the occupied depth-`zoom` tile at
  each density decile (the tile at quantile (d + 0.5) / 10 of the occupied tiles ordered by
  count, ties by tile coordinates), with no filter. Duplicates are dropped, so zoom 0 has one.
* `script`: the interactive bench's camera script. From the whole map, into each of the two
  densest depth-3 regions through zooms 3, 6, 9, 12 and 14, each step centred on the densest tile
  of that depth inside the previous one; a pan east by half a screen at 6, 9 and 14, and south too
  at 14; then back out through 12, 9, 6 and 3.
* `filters`: at the script's zoom-6 and zoom-12 screens of the densest region, a fixed sequence of
  filter changes: none, `band` = A, D and E (50%, 5% and 0.1%), `num` below 500, 50 and 1 (the
  same), a box over the middle quarter of the screen, `band` = D with `num` below 500, and none.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import duckdb

from frame import EXTENT, SCREEN, TILE_PX, Camera

BATTERY_ZOOMS = (0, 4, 8, 12, 16)
SCRIPT_ZOOMS = (3, 6, 9, 12, 14)
PANS = {6: ["east"], 9: ["east"], 14: ["east", "south"]}
SPAN = EXTENT[1] - EXTENT[0]


def tile_centre(tx: int, ty: int, z: int) -> tuple[float, float]:
    size = SPAN / (1 << z)
    return EXTENT[0] + (tx + 0.5) * size, EXTENT[2] + (ty + 0.5) * size


def tile_counts(con, tier: str, z: int, within: tuple[int, int, int] | None = None):
    shift = 32 - z
    where = ""
    if within is not None:
        pz, ptx, pty = within
        s = 32 - pz
        where = f"WHERE qx::UBIGINT >> {s} = {ptx} AND qy::UBIGINT >> {s} = {pty}"
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
    steps = [{"camera": cam(0, 0.0, 0.0, "whole"), "filter": None}]
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


def filter_sequence(c: dict) -> list[dict]:
    x0, y0, x1, y1 = Camera(c["zoom"], c["cx"], c["cy"]).bbox()
    qw, qh = (x1 - x0) / 4, (y1 - y0) / 4
    box = [x0 + qw, y0 + qh, x1 - qw, y1 - qh]
    leaves = [
        ("none", []),
        ("band=A", [{"col": "band", "op": "eq", "value": "A"}]),
        ("band=D", [{"col": "band", "op": "eq", "value": "D"}]),
        ("band=E", [{"col": "band", "op": "eq", "value": "E"}]),
        ("num<500", [{"col": "num", "op": "lt", "value": 500}]),
        ("num<50", [{"col": "num", "op": "lt", "value": 50}]),
        ("num<1", [{"col": "num", "op": "lt", "value": 1}]),
        ("box", [{"op": "bbox", "value": box}]),
        ("band=D&num<500", [{"col": "band", "op": "eq", "value": "D"},
                            {"col": "num", "op": "lt", "value": 500}]),
        ("none-again", []),
    ]
    return [{"camera": c, "filter": {"label": label, "leaves": lv}} for label, lv in leaves]


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--tier", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    con = duckdb.connect()
    tier = str(args.tier.resolve())
    steps, anchors = script(con, tier)
    out = {
        "tier": args.tier.name,
        "battery": battery(con, tier),
        "script": steps,
        "filters": filter_sequence(anchors[(0, 6)]) + filter_sequence(anchors[(0, 12)]),
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(out, indent=1) + "\n")
    print({k: len(v) for k, v in out.items() if isinstance(v, list)})


if __name__ == "__main__":
    main()
