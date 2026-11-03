"""Derive the GBIF benchmark tiers from the GBIF rung's points file.

    python prepare_gbif.py [--source data/ladder/gbif-terms/points.parquet]
                           [--data data/bench-compare] [--tiers 1e6,5e8] [--workers 6]

The steps are deterministic, so two runs write the same rows, and they follow `prepare.py`'s:

1. The source is the `gbif-terms` rung's `points.parquet`: 3,495,729,729 placed occurrences, one
   row each, with GBIF's unique `gbifid`. Only `lon`, `lat`, `kingdom`, `year`, `gbifid` and
   `access` are read; the species, name and country columns are not.
2. Each position is projected to Web Mercator in metres, once, here: `x = R · lon · π / 180` and
   `y = R · ln(tan(π / 4 + lat' · π / 360))` with `R = 6,378,137` and `lat'` the latitude clamped
   to ±85.0511287798066°, as the GBIF rung's `web_mercator` view clamps it. Every system and the
   oracle read these `x` and `y`, so no system projects; Mosaica declares `projection = "none"`.
   The extent is Web Mercator's world square, `±π · R` on each axis.
3. A row is in the tier of nominal size N where `splitmix64(gbifid ^ SAMPLE_SEED) < floor(N ·
   2^64 / 3,495,729,729)`, so the tiers nest. `entity_id` is the `gbifid`.
4. `qx`, `qy` and `rank` are as `prepare.py` writes them.
5. `access` is the row's list of access terms as the rung wrote it: its country (`UNRECORDED`
   where GBIF has none), `y:` and its year, and `s:` and its species key. A row is visible to a
   viewer who holds any one of them. No list is empty, which the run checks, so the view's
   default never applies.

Rows are written in source order. The manifest beside the tiers records the source, the seeds,
the extent, the projection, the thresholds, row counts, rows by kingdom and each tier's SHA-256;
`tiers-manifest-gbif.json` is its committed copy, which a tier must reproduce.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import subprocess
import sys
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

os.environ["BENCH_DATASET"] = "gbif"

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.parquet as pq

from prepare import RANK_SEED, SAMPLE_SEED, sha256, splitmix64, threshold, tier_name

HERE = Path(__file__).resolve().parent
#: The checkout that holds `data/`, which a worktree shares with the main one.
COMMON = Path(subprocess.run(["git", "-C", str(HERE), "rev-parse", "--path-format=absolute",
                              "--git-common-dir"], capture_output=True, text=True,
                             check=True).stdout.strip()).parent
SOURCE = COMMON / "data/ladder/gbif-terms/points.parquet"
SOURCE_ROWS = 3495729729
COLUMNS = ["gbifid", "lon", "lat", "kingdom", "year", "access"]
RADIUS = 6378137.0
MAX_LATITUDE = 85.0511287798066
EXTENT = [-math.pi * RADIUS, math.pi * RADIUS, -math.pi * RADIUS, math.pi * RADIUS]

SCHEMA = pa.schema([
    ("entity_id", pa.uint64()),
    ("x", pa.float64()),
    ("y", pa.float64()),
    ("kingdom", pa.string()),
    ("year", pa.uint16()),
    ("access", pa.list_(pa.string())),
    ("qx", pa.uint32()),
    ("qy", pa.uint32()),
    ("rank", pa.int64()),
])


def mercator(lon: np.ndarray, lat: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    phi = np.clip(lat, -MAX_LATITUDE, MAX_LATITUDE)
    return RADIUS * lon * (math.pi / 180), RADIUS * np.log(np.tan(math.pi / 4 + phi * (math.pi / 360)))


def part(args: tuple[str, int, list[int | None]]) -> tuple[list[pa.Table], int, int]:
    """One source row group: each tier's rows, the rows read and the rows with no access term."""
    from frame import fixed32_array

    path, g, cuts = args
    t = pq.ParquetFile(path).read_row_group(g, columns=COLUMNS)
    read = t.num_rows
    h = splitmix64(t["gbifid"].to_numpy() ^ np.uint64(SAMPLE_SEED))
    if None not in cuts:
        keep = h < np.uint64(max(cuts))
        t, h = t.filter(pa.array(keep)), h[keep]
    empty = int(pc.sum(pc.fill_null(pc.less_equal(pc.list_value_length(t["access"]), 0), True)).as_py() or 0)
    ids = t["gbifid"].to_numpy()
    x, y = mercator(t["lon"].to_numpy(), t["lat"].to_numpy())
    out = pa.table({
        "entity_id": ids, "x": x, "y": y, "kingdom": t["kingdom"], "year": t["year"],
        "access": t["access"],
        "qx": fixed32_array(x, EXTENT[0], EXTENT[1]), "qy": fixed32_array(y, EXTENT[2], EXTENT[3]),
        "rank": (splitmix64(ids ^ np.uint64(RANK_SEED)) >> np.uint64(1)).astype(np.int64),
    }).cast(SCHEMA)
    return [out if c is None else out.filter(pa.array(h < np.uint64(c))) for c in cuts], read, empty


def derive(source: Path, out: Path, tiers: list[int], workers: int) -> dict:
    out.mkdir(parents=True, exist_ok=True)
    names = {n: tier_name(n, SOURCE_ROWS) for n in tiers}
    paths = {n: out / f"gbif-{names[n]}.parquet" for n in tiers}
    writers = {n: pq.ParquetWriter(paths[n], SCHEMA, compression="zstd") for n in tiers}
    cuts = [threshold(n, SOURCE_ROWS) for n in tiers]
    rows = {n: 0 for n in tiers}
    kingdoms: dict[int, dict[str, int]] = {n: {} for n in tiers}
    read = empty = 0
    groups = pq.ParquetFile(source).metadata.num_row_groups
    with ProcessPoolExecutor(workers) as pool:
        jobs = pool.map(part, ((str(source), g, cuts) for g in range(groups)), chunksize=4)
        for g, (tables, n_read, n_empty) in enumerate(jobs):
            read += n_read
            empty += n_empty
            for n, sel in zip(tiers, tables):
                writers[n].write_table(sel, row_group_size=1 << 20)
                rows[n] += sel.num_rows
                for v in pc.value_counts(sel["kingdom"]).to_pylist():
                    kingdoms[n][v["values"]] = kingdoms[n].get(v["values"], 0) + v["counts"]
            if g % 100 == 0:
                print(f"row group {g} of {groups}: {read:,} rows read", file=sys.stderr, flush=True)
    for w in writers.values():
        w.close()
    if read != SOURCE_ROWS:
        sys.exit(f"read {read} rows, expected {SOURCE_ROWS}")
    if empty:
        sys.exit(f"{empty} rows have no access term; the view's default would apply to them, "
                 f"which this harness does not model")
    return {
        names[n]: {
            "file": paths[n].name,
            "nominal_rows": min(n, SOURCE_ROWS),
            "rows": rows[n],
            "threshold": threshold(n, SOURCE_ROWS),
            "rows_by_kingdom": dict(sorted(kingdoms[n].items(), key=lambda kv: -kv[1])),
            "sha256": sha256(paths[n]),
        }
        for n in tiers
    }


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--source", type=Path, default=SOURCE)
    ap.add_argument("--data", type=Path, default=COMMON / "data/bench-compare")
    ap.add_argument("--tiers", default="1e6,5e8")
    ap.add_argument("--workers", type=int, default=6)
    args = ap.parse_args()
    tiers = sorted(SOURCE_ROWS if t == "full" else int(float(t)) for t in args.tiers.split(","))
    rows = pq.ParquetFile(args.source).metadata.num_rows
    if rows != SOURCE_ROWS:
        sys.exit(f"{args.source} holds {rows} rows, not {SOURCE_ROWS}")
    committed_path = HERE / "tiers-manifest-gbif.json"
    committed = json.loads(committed_path.read_text()) if committed_path.exists() else {}
    if committed.get("extent") not in (None, EXTENT):
        sys.exit(f"the committed extent is {committed['extent']}, not Web Mercator's world square "
                 f"{EXTENT}")
    if "extent" not in committed:
        # frame.py reads the extent from the committed manifest, so it is written before the tiers.
        committed_path.write_text(json.dumps(dict(committed, extent=EXTENT), indent=2) + "\n")
    derived = derive(args.source, args.data / "tiers", tiers, args.workers)
    manifest = {
        "source": {
            "path": os.path.relpath(args.source.resolve(), COMMON),
            "bytes": args.source.stat().st_size,
            "rows": SOURCE_ROWS,
            "columns": COLUMNS,
            "origin": "The gbif-terms rung of the Mosaica test ladder: GBIF occurrences "
                      "(gbif/2026-06-01) with a position, each with its access terms: country, "
                      "y:<year> and s:<species key>",
        },
        "sample_seed": SAMPLE_SEED,
        "rank_seed": RANK_SEED,
        "extent": EXTENT,
        "projection": f"Web Mercator in metres: x = R lon pi/180, y = R ln(tan(pi/4 + lat pi/360)), "
                      f"R = {RADIUS}, lat clamped to +-{MAX_LATITUDE}; the extent is the world square",
        "tiers": derived,
    }
    (args.data / "tiers" / "manifest-gbif.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps(manifest, indent=2))
    for name, tier in derived.items():
        old = committed.get("tiers", {}).get(name)
        if old and old["sha256"] != tier["sha256"]:
            sys.exit(f"{tier['file']} has SHA-256 {tier['sha256']}, not the committed "
                     f"{old['sha256']}: the derivation is not reproducing")
    merged = dict(manifest, tiers=dict(committed.get("tiers", {}), **derived))
    committed_path.write_text(json.dumps(merged, indent=2) + "\n")


if __name__ == "__main__":
    main()
