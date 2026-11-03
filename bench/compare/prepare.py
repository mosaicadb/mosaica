"""Download the HoloViz OSM billion-point file and derive the benchmark tiers from it.

    python prepare.py [--data data/bench-compare] [--tiers 1e7,1e8]

Every step is deterministic, so two runs write the same rows:

1. The source is `osm-1billion.parq.zip` from the HoloViz datashader bucket, checked against the
   SHA-256 below. Its 119 part files are read in the order of their part number, and a row's index
   (`rid`) is its position in that concatenation, from 0 to 1,000,050,362.
2. A row is in the tier of nominal size N where `splitmix64(rid ^ SAMPLE_SEED) < floor(N * 2^64 /
   1,000,050,363)`. The tiers therefore nest: every row of the 10^7 sample is in the 10^8 tier. The
   sample is Bernoulli, so a tier holds N rows give or take a few thousand; the manifest records
   the exact count.
3. Each row gets two synthetic attributes, since the OSM file has none, from `h =
   splitmix64(rid ^ ATTR_SEED)`: `band` from `h % 1000` (A below 500, B below 800, C below 949, D
   below 999, E at 999, so 50%, 30%, 14.9%, 5% and 0.1%) and `num` as `(h >> 20) % 1000`. They
   are spatially uniform by construction.
4. `qx` and `qy` are the 32-bit grid positions `frame.fixed32` gives, for the oracle.

Rows are written in source order (the source is roughly spatially partitioned, not sorted);
each system sorts as its own build does. The manifest beside the tiers records the source
checksum, the seeds, thresholds, row counts and each tier's SHA-256, which must match
`tiers-manifest.json`, the committed copy that `run.py` checks a tier against.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import zipfile
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq

from frame import BANDS as FRAME_BANDS, EXTENT, fixed32_array

SOURCE_URL = "https://s3.amazonaws.com/datashader-data/osm-1billion.parq.zip"
SOURCE_SHA256 = "7b6e89c3b8be96813c67682d6c9e6a05519ef2e31136378123c9dfea3b93ef57"
SOURCE_BYTES = 5498778647
SOURCE_ROWS = 1000050363
SAMPLE_SEED = 0x6D6F736169636131
ATTR_SEED = 0x6174747269627331
BANDS = np.array(FRAME_BANDS)
BAND_EDGES = np.array([500, 800, 949, 999])

SCHEMA = pa.schema([
    ("rid", pa.uint64()),
    ("x", pa.float32()),
    ("y", pa.float32()),
    ("band", pa.string()),
    ("num", pa.uint16()),
    ("qx", pa.uint32()),
    ("qy", pa.uint32()),
])


def splitmix64(v: np.ndarray) -> np.ndarray:
    with np.errstate(over="ignore"):
        z = v.astype(np.uint64) + np.uint64(0x9E3779B97F4A7C15)
        z = (z ^ (z >> np.uint64(30))) * np.uint64(0xBF58476D1CE4E5B9)
        z = (z ^ (z >> np.uint64(27))) * np.uint64(0x94D049BB133111EB)
        return z ^ (z >> np.uint64(31))


def threshold(n: int) -> int:
    return (n << 64) // SOURCE_ROWS


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        while chunk := f.read(1 << 24):
            h.update(chunk)
    return h.hexdigest()


def fetch(source: Path) -> Path:
    zpath = source / "osm-1billion.parq.zip"
    if not zpath.exists() or zpath.stat().st_size != SOURCE_BYTES:
        source.mkdir(parents=True, exist_ok=True)
        subprocess.run(["curl", "-sS", "-C", "-", "-o", str(zpath), SOURCE_URL], check=True)
    digest = sha256(zpath)
    if digest != SOURCE_SHA256:
        sys.exit(f"{zpath} has SHA-256 {digest}, not {SOURCE_SHA256}; delete it and run again")
    parts = source / "osm-1billion.parq"
    if not parts.exists():
        with zipfile.ZipFile(zpath) as z:
            z.extractall(source)
    return parts


def part_files(parts: Path) -> list[Path]:
    files = sorted(parts.glob("part.*.parquet"), key=lambda p: int(re.search(r"part\.(\d+)", p.name)[1]))
    if len(files) != 119:
        sys.exit(f"{parts} holds {len(files)} part files, not 119; delete it and run again")
    return files


def derive(parts: Path, out: Path, tiers: list[int]) -> dict:
    out.mkdir(parents=True, exist_ok=True)
    writers = {n: pq.ParquetWriter(out / f"osm-{n:.0e}.parquet".replace("+0", ""), SCHEMA,
                                   compression="zstd") for n in tiers}
    rows = {n: 0 for n in tiers}
    cuts = {n: np.uint64(threshold(n)) for n in tiers}
    offset = 0
    for path in part_files(parts):
        table = pq.read_table(path, columns=["x", "y"])
        n = table.num_rows
        rid = np.arange(offset, offset + n, dtype=np.uint64)
        offset += n
        h = splitmix64(rid ^ np.uint64(SAMPLE_SEED))
        widest = max(tiers)
        keep = h < cuts[widest]
        rid, h = rid[keep], h[keep]
        x = table["x"].to_numpy()[keep]
        y = table["y"].to_numpy()[keep]
        a = splitmix64(rid ^ np.uint64(ATTR_SEED))
        band = BANDS[np.searchsorted(BAND_EDGES, a % np.uint64(1000), side="right")]
        num = ((a >> np.uint64(20)) % np.uint64(1000)).astype(np.uint16)
        qx = fixed32_array(x, EXTENT[0], EXTENT[1])
        qy = fixed32_array(y, EXTENT[2], EXTENT[3])
        for t in tiers:
            sel = h < cuts[t]
            batch = pa.table([rid[sel], x[sel], y[sel], band[sel], num[sel], qx[sel], qy[sel]],
                             schema=SCHEMA)
            writers[t].write_table(batch, row_group_size=1 << 20)
            rows[t] += batch.num_rows
        print(f"{path.name}: {offset:,} rows read", file=sys.stderr, flush=True)
    for w in writers.values():
        w.close()
    if offset != SOURCE_ROWS:
        sys.exit(f"read {offset} rows, expected {SOURCE_ROWS}")
    return {
        f"{t:.0e}".replace("+0", ""): {
            "file": f"osm-{t:.0e}.parquet".replace("+0", ""),
            "nominal_rows": t,
            "rows": rows[t],
            "threshold": int(cuts[t]),
            "sha256": sha256(out / f"osm-{t:.0e}.parquet".replace("+0", "")),
        }
        for t in tiers
    }


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--data", type=Path, default=Path(__file__).resolve().parents[2] / "data/bench-compare")
    ap.add_argument("--tiers", default="1e7,1e8")
    args = ap.parse_args()
    tiers = sorted(int(float(t)) for t in args.tiers.split(","))
    parts = fetch(args.data / "source")
    derived = derive(parts, args.data / "tiers", tiers)
    manifest = {
        "source": {
            "url": SOURCE_URL,
            "sha256": SOURCE_SHA256,
            "bytes": SOURCE_BYTES,
            "rows": SOURCE_ROWS,
            "origin": "OpenStreetMap bulk GPS points (planet.gpx, 2013), projected to Web Mercator "
                      "by HoloViz with datashader.utils.lnglat_to_meters, stored as float32",
            "licence": "not stated by HoloViz or on the OSM wiki's Planet.gpx page; confirm before "
                       "publishing",
        },
        "sample_seed": SAMPLE_SEED,
        "attr_seed": ATTR_SEED,
        "extent": EXTENT,
        "tiers": derived,
    }
    path = args.data / "tiers" / "manifest.json"
    path.write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps(manifest, indent=2))
    committed = json.loads((Path(__file__).with_name("tiers-manifest.json")).read_text())["tiers"]
    for name, tier in derived.items():
        if name in committed and committed[name]["sha256"] != tier["sha256"]:
            sys.exit(f"{tier['file']} has SHA-256 {tier['sha256']}, not the committed "
                     f"{committed[name]['sha256']}: the derivation is not reproducing")


if __name__ == "__main__":
    main()
