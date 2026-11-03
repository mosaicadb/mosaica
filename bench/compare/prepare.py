"""Derive the benchmark tiers from the PaperSeek points file.

    python prepare.py [--source data/ladder/paperseek/points.parquet] [--data data/bench-compare]
                      [--tiers 1e6,1e7,full]

Every step is deterministic, so two runs write the same rows:

1. The source is the PaperSeek rung's `points.parquet` (102,117,343 works, one row each, with a
   unique `entity_id`). Only the columns the benchmark uses are read: `entity_id`, `x`, `y`,
   `licence`, `publication_year`, `type` and `is_oa`. Titles and abstracts are never read.
2. The extent is one square for every system, fixed by the full file's bounds: the side is
   `ceil(max(x_max - floor(x_min), y_max - floor(y_min)))`, and each axis runs from the floor of
   its minimum for that side. `tiers-manifest.json` records it and `frame.py` reads it from there;
   a source whose bounds give a different square is refused.
3. A row is in the tier of nominal size N where `splitmix64(entity_id ^ SAMPLE_SEED) < floor(N *
   2^64 / 102,117,343)`, and in `full` always. The tiers therefore nest. The sample is Bernoulli,
   so a tier holds N rows give or take a few thousand; the manifest records the exact count.
4. `qx` and `qy` are the 32-bit grid positions `frame.fixed32` gives, for the oracle.

Rows are written in source order; each system sorts as its own build does. The manifest beside
the tiers records the source's size, the seed, the extent, thresholds, row counts, rows per
licence and each tier's SHA-256, which must match `tiers-manifest.json`, the committed copy that
`run.py` checks a tier against.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.parquet as pq

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
SOURCE = REPO / "data/ladder/paperseek/points.parquet"
SOURCE_ROWS = 102117343
SAMPLE_SEED = 0x6D6F736169636131
COLUMNS = ["entity_id", "x", "y", "licence", "publication_year", "type", "is_oa"]

SCHEMA = pa.schema([
    ("entity_id", pa.uint64()),
    ("x", pa.float64()),
    ("y", pa.float64()),
    ("licence", pa.string()),
    ("publication_year", pa.int32()),
    ("type", pa.string()),
    ("is_oa", pa.bool_()),
    ("qx", pa.uint32()),
    ("qy", pa.uint32()),
])


def splitmix64(v: np.ndarray) -> np.ndarray:
    with np.errstate(over="ignore"):
        z = v.astype(np.uint64) + np.uint64(0x9E3779B97F4A7C15)
        z = (z ^ (z >> np.uint64(30))) * np.uint64(0xBF58476D1CE4E5B9)
        z = (z ^ (z >> np.uint64(27))) * np.uint64(0x94D049BB133111EB)
        return z ^ (z >> np.uint64(31))


def threshold(n: int) -> int | None:
    return None if n >= SOURCE_ROWS else (n << 64) // SOURCE_ROWS


def tier_name(n: int) -> str:
    return "full" if n >= SOURCE_ROWS else f"{n:.0e}".replace("+0", "")


def sha256(path: Path) -> str:
    """The file's SHA-256, read in chunks."""
    h = hashlib.sha256()
    with path.open("rb") as f:
        while chunk := f.read(1 << 24):
            h.update(chunk)
    return h.hexdigest()


def square_extent(source: pq.ParquetFile) -> list[float]:
    """`[x_min, x_max, y_min, y_max]` of the square step 2 defines."""
    lo = [math.inf, math.inf]
    hi = [-math.inf, -math.inf]
    for g in range(source.metadata.num_row_groups):
        t = source.read_row_group(g, columns=["x", "y"])
        for i, c in enumerate(("x", "y")):
            mm = pc.min_max(t[c])
            lo[i] = min(lo[i], mm["min"].as_py())
            hi[i] = max(hi[i], mm["max"].as_py())
    x0, y0 = math.floor(lo[0]), math.floor(lo[1])
    side = math.ceil(max(hi[0] - x0, hi[1] - y0))
    return [float(x0), float(x0 + side), float(y0), float(y0 + side)]


def derive(source: pq.ParquetFile, out: Path, tiers: list[int], extent: list[float]) -> dict:
    from frame import fixed32_array

    out.mkdir(parents=True, exist_ok=True)
    names = {n: tier_name(n) for n in tiers}
    paths = {n: out / f"paperseek-{names[n]}.parquet" for n in tiers}
    writers = {n: pq.ParquetWriter(paths[n], SCHEMA, compression="zstd") for n in tiers}
    rows = {n: 0 for n in tiers}
    licences: dict[int, dict[str, int]] = {n: {} for n in tiers}
    cuts = {n: threshold(n) for n in tiers}
    widest = max(tiers)
    read = 0
    for g in range(source.metadata.num_row_groups):
        t = source.read_row_group(g, columns=COLUMNS)
        read += t.num_rows
        h = splitmix64(t["entity_id"].to_numpy() ^ np.uint64(SAMPLE_SEED))
        if cuts[widest] is not None:
            keep = h < np.uint64(cuts[widest])
            t, h = t.filter(pa.array(keep)), h[keep]
        t = t.select(COLUMNS).cast(pa.schema([SCHEMA.field(c) for c in COLUMNS]))
        qx = pa.array(fixed32_array(t["x"].to_numpy(), extent[0], extent[1]))
        qy = pa.array(fixed32_array(t["y"].to_numpy(), extent[2], extent[3]))
        t = t.append_column("qx", qx).append_column("qy", qy)
        for n in tiers:
            sel = t if cuts[n] is None else t.filter(pa.array(h < np.uint64(cuts[n])))
            writers[n].write_table(sel, row_group_size=1 << 20)
            rows[n] += sel.num_rows
            vc = pc.value_counts(sel["licence"])
            for v in vc.to_pylist():
                licences[n][v["values"]] = licences[n].get(v["values"], 0) + v["counts"]
        if g % 50 == 0:
            print(f"row group {g}: {read:,} rows read", file=sys.stderr, flush=True)
    for w in writers.values():
        w.close()
    if read != SOURCE_ROWS:
        sys.exit(f"read {read} rows, expected {SOURCE_ROWS}")
    return {
        names[n]: {
            "file": paths[n].name,
            "nominal_rows": min(n, SOURCE_ROWS),
            "rows": rows[n],
            "threshold": cuts[n],
            "rows_by_licence": dict(sorted(licences[n].items(), key=lambda kv: -kv[1])),
            "sha256": sha256(paths[n]),
        }
        for n in tiers
    }


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--source", type=Path, default=SOURCE)
    ap.add_argument("--data", type=Path, default=REPO / "data/bench-compare")
    ap.add_argument("--tiers", default="1e6,1e7")
    args = ap.parse_args()
    tiers = sorted(SOURCE_ROWS if t == "full" else int(float(t)) for t in args.tiers.split(","))
    source = pq.ParquetFile(args.source)
    if source.metadata.num_rows != SOURCE_ROWS:
        sys.exit(f"{args.source} holds {source.metadata.num_rows} rows, not {SOURCE_ROWS}")
    committed_path = HERE / "tiers-manifest.json"
    committed = json.loads(committed_path.read_text()) if committed_path.exists() else {}
    extent = square_extent(source)
    if committed.get("extent") not in (None, extent):
        sys.exit(f"the source's bounds give the extent {extent}, not the committed "
                 f"{committed['extent']}: the source has changed")
    if "extent" not in committed:
        # frame.py reads the extent from the committed manifest, so it is written before the tiers.
        committed_path.write_text(json.dumps(dict(committed, extent=extent), indent=2) + "\n")
    derived = derive(source, args.data / "tiers", tiers, extent)
    manifest = {
        "source": {
            "path": str(args.source.resolve().relative_to(REPO)) if args.source.resolve().is_relative_to(REPO)
                    else str(args.source),
            "bytes": args.source.stat().st_size,
            "rows": SOURCE_ROWS,
            "columns": COLUMNS,
            "origin": "The PaperSeek rung of the Mosaica test ladder: PaperSeek works joined to "
                      "OpenAlex for licence, year, type and open access, laid out in 2D by UMAP "
                      "over Stella V5 embeddings of title and abstract",
        },
        "sample_seed": SAMPLE_SEED,
        "extent": extent,
        "extent_rule": "side = ceil(max(x_max - floor(x_min), y_max - floor(y_min))) over the full "
                       "file; each axis from the floor of its minimum",
        "tiers": derived,
    }
    path = args.data / "tiers" / "manifest.json"
    path.write_text(json.dumps(manifest, indent=2) + "\n")
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
