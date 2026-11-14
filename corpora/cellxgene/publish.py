"""The files `mosaica.datasets` builds the CELLxGENE dataset from, cut from what `prepare.py --sample
0` wrote, for the Hugging Face dataset `mosaicadb/cellxgene-census-map`.

Writes into `--out`:

- `parts/part-NNN.parquet`: every cell, both species, in `PARTS` parts. Each species' cells are
  dealt to the parts by a seeded random permutation, so every part holds a uniform sample of each
  species and any number of parts is a uniform sample of the whole. A row is the cell's
  `cells.parquet` row with its position, `x` and `y` as float32, which is what the layout was
  computed in, and its access label. Within a part the rows are in `entity_id` order.
- `cell-ontology-tiers.parquet`: each cell type's four tier keys, which is what a cell's
  membership of the Cell Ontology layer depends on, so a loader makes the member file for any
  set of parts.
- `artifacts-cell-ontology.parquet`, the `vocab-*.parquet` files and `restricted-collections.json`,
  copied as they are.
- `README.md`, the dataset card, from `dataset-card.md`.

    ~/venvs/projection/bin/python -m corpora.cellxgene.publish --out /path/to/upload
    hf upload mosaicadb/cellxgene-census-map /path/to/upload --repo-type dataset
"""

from __future__ import annotations

import argparse
import shutil
import sys
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.ipc as ipc
import pyarrow.parquet as pq

from . import sources

PARTS = 128
SEED = 0

COPIED = ["artifacts-cell-ontology.parquet", "restricted-collections.json"]


def dealt(rows: int, rng: np.random.Generator) -> np.ndarray:
    """Each of `rows` cells' part: a random permutation dealt round the parts, so the parts differ
    in size by at most one cell."""
    part = np.empty(rows, dtype=np.uint8)
    part[rng.permutation(rows)] = np.arange(rows) % PARTS
    return part


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--rung", type=Path, default=sources.rung(0),
                    help="what `prepare.py --sample 0` wrote")
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    rung, out = args.rung, args.out
    (out / "parts").mkdir(parents=True, exist_ok=True)
    spool = out / "spool"
    spool.mkdir(exist_ok=True)

    cells = pq.ParquetFile(rung / "cells.parquet")
    points = {s: pq.ParquetFile(rung / f"points-{s}.parquet") for s in sources.SPECIES}
    members = pq.ParquetFile(rung / "members-cell-ontology.parquet")
    rng = np.random.default_rng(SEED)
    parts = {s: dealt(spec["rows"], rng) for s, spec in sources.SPECIES.items()}

    schema = None
    writers: list = []
    tiers: dict[str, list[str]] = {}
    group = 0
    for species, spec in sources.SPECIES.items():
        at = 0
        for g in range(points[species].metadata.num_row_groups):
            cell = cells.read_row_group(group)
            point = points[species].read_row_group(g)
            member = members.read_row_group(group)
            group += 1
            assert cell["entity_id"].equals(point["entity_id"])
            assert cell["entity_id"].equals(member["entity"])
            x = point["x"].cast(pa.float32())
            y = point["y"].cast(pa.float32())
            assert x.cast(pa.float64()).equals(point["x"]) and y.cast(pa.float64()).equals(point["y"])
            # A list cannot be a grouping key, so each chain is grouped joined into one string.
            chains = pc.binary_join(member["key"], "\x1f")
            pairs = pa.table({"c": cell["cell_type"], "k": chains}).group_by(
                ["c", "k"]).aggregate([])
            for c, k in zip(pairs["c"].to_pylist(), pairs["k"].to_pylist()):
                assert tiers.setdefault(c, k.split("\x1f")) == k.split("\x1f"), (
                    f"{c} has two tier chains")
            table = cell.add_column(1, "x", x).add_column(2, "y", y).add_column(
                3, "access", point["access"])
            if schema is None:
                schema = table.schema
                writers = [ipc.new_stream(spool / f"{i:03d}.arrow", schema,
                                          options=ipc.IpcWriteOptions(compression="zstd"))
                           for i in range(PARTS)]
            m = table.num_rows
            part = parts[species][at : at + m]
            at += m
            order = np.argsort(part, kind="stable")
            bounds = np.searchsorted(part[order], np.arange(PARTS + 1))
            sorted_table = table.take(pa.array(order))
            for i in range(PARTS):
                writers[i].write_table(sorted_table.slice(bounds[i], bounds[i + 1] - bounds[i]))
        assert at == spec["rows"], f"{species}: {at:,} rows against {spec['rows']:,}"
        print(f"  {species}: {at:,} cells dealt", flush=True)
    for w in writers:
        w.close()

    counts = []
    for i in range(PARTS):
        with ipc.open_stream(spool / f"{i:03d}.arrow") as reader:
            table = reader.read_all()
        pq.write_table(table, out / "parts" / f"part-{i:03d}.parquet", compression="zstd",
                       row_group_size=1 << 20)
        counts.append(table.num_rows)
        (spool / f"{i:03d}.arrow").unlink()
    spool.rmdir()
    print(f"  {PARTS} parts of {min(counts):,} to {max(counts):,} cells", flush=True)

    names = sorted(tiers)
    pq.write_table(pa.table({
        "cell_type": pa.array(names, pa.string()),
        "key": pa.array([tiers[n] for n in names], pa.list_(pa.string())),
    }), out / "cell-ontology-tiers.parquet")
    for name in COPIED + sorted(p.name for p in rung.glob("vocab-*.parquet")):
        shutil.copyfile(rung / name, out / name)
    shutil.copyfile(Path(__file__).with_name("dataset-card.md"), out / "README.md")
    total = sum(f.stat().st_size for f in out.rglob("*") if f.is_file())
    print(f"wrote {total / 1e9:.2f} GB to {out}")


if __name__ == "__main__":
    sys.exit(main())
