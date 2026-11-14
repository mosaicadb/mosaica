"""Download the parts of CZ CELLxGENE Census 2025-11-08 this corpus maps, onto local disk.

Three steps, each skipped where its output is already present:

- `--obs`: each species' cell table (the columns in `sources.OBS_COLUMNS`) and the Census's
  dataset table, as parquet. About a minute.
- `--vectors`: each species' scVI embedding as a `rows x 50` float16 matrix in `soma_joinid`
  order, read in contiguous ranges of a million rows by a few threads at once. 15.9 GB for
  human and 4.4 GB for mouse. Resumable: a ledger beside the matrix names the ranges written, and
  a rerun reads only the rest.
- `--ontology`: the Cell Ontology's `cl-basic.json` at the pinned release.

    ~/venvs/projection/bin/python -m corpora.cellxgene.stage --obs --vectors --ontology
"""

from __future__ import annotations

import argparse
import concurrent.futures as cf
import json
import resource
import time
import urllib.request
from pathlib import Path

import numpy as np
import pyarrow.parquet as pq

from ..common.timing import Steps
from . import sources

CHUNK_ROWS = 1_000_000
READ_WORKERS = 3


def partial(path: Path) -> Path:
    """Where a file is written before it is renamed into place, so an interrupted run leaves no
    file that a rerun would take as finished."""
    return path.with_name(path.name + ".partial")


def stage_obs() -> None:
    import cellxgene_census

    with cellxgene_census.open_soma(census_version=sources.VINTAGE) as census:
        datasets = sources.staging() / "datasets.parquet"
        if not datasets.exists():
            table = census["census_info"]["datasets"].read().concat()
            pq.write_table(table, partial(datasets), compression="zstd")
            partial(datasets).rename(datasets)
            print(f"  datasets: {table.num_rows:,} rows", flush=True)
        for species, spec in sources.SPECIES.items():
            path = sources.obs_path(species)
            if path.exists():
                continue
            path.parent.mkdir(parents=True, exist_ok=True)
            obs = census["census_data"][spec["experiment"]].obs
            table = obs.read(column_names=sources.OBS_COLUMNS).concat().combine_chunks()
            table = table.sort_by("soma_joinid")
            joinid = table.column("soma_joinid").to_numpy()
            assert len(joinid) == spec["rows"] and joinid[0] == 0 and joinid[-1] == len(joinid) - 1, (
                f"{species} obs is not soma_joinid 0..{spec['rows'] - 1}"
            )
            pq.write_table(table, partial(path), compression="zstd", row_group_size=CHUNK_ROWS)
            partial(path).rename(path)
            print(f"  {species} obs: {table.num_rows:,} rows, {path.stat().st_size / 1e9:.2f} GB",
                  flush=True)


def read_range(array, lo: int, hi: int) -> tuple[np.ndarray, int]:
    """Rows `lo..hi` (exclusive) of a 50-column sparse array, densified, and how many of them
    lacked a value in some column."""
    # A SOMA slice includes its upper bound.
    coo = array.read(coords=(slice(lo, hi - 1),)).tables().concat()
    rows = coo.column("soma_dim_0").to_numpy() - lo
    cols = coo.column("soma_dim_1").to_numpy()
    out = np.zeros((hi - lo, sources.DIM), dtype=np.float32)
    out[rows, cols] = coo.column("soma_data").to_numpy()
    filled = np.zeros(hi - lo, dtype=np.int64)
    np.add.at(filled, rows, 1)
    missing = int((filled != sources.DIM).sum())
    return out.astype(np.float16), missing


def stage_vectors(species: str) -> dict:
    import tiledbsoma as soma

    n = sources.SPECIES[species]["rows"]
    path = sources.vectors_path(species)
    path.parent.mkdir(parents=True, exist_ok=True)
    ledger_path = path.with_suffix(".ledger.jsonl")
    mode = "r+" if path.exists() and path.stat().st_size == n * sources.DIM * 2 else "w+"
    if mode == "w+":
        ledger_path.unlink(missing_ok=True)
    out = np.memmap(path, dtype=np.float16, mode=mode, shape=(n, sources.DIM))
    done = set()
    if ledger_path.exists():
        done = {json.loads(line)["lo"] for line in ledger_path.read_text().splitlines() if line}
    todo = [lo for lo in range(0, n, CHUNK_ROWS) if lo not in done]
    print(f"  {species}: {len(todo)} of {-(-n // CHUNK_ROWS)} ranges to read", flush=True)

    uri = f"{sources.EMBEDDING_ROOT}/{sources.SPECIES[species]['embedding']}"
    context = soma.SOMATileDBContext(tiledb_config=sources.S3_CONFIG)
    missing_rows, t0, written = 0, time.time(), 0
    with soma.open(uri, context=context) as array, open(ledger_path, "a") as ledger:
        assert array.shape == (n, sources.DIM), f"{uri} has shape {array.shape}"

        def job(lo):
            hi = min(lo + CHUNK_ROWS, n)
            block, missing = read_range(array, lo, hi)
            return lo, hi, block, missing

        with cf.ThreadPoolExecutor(READ_WORKERS) as pool:
            for lo, hi, block, missing in pool.map(job, todo):
                out[lo:hi] = block
                missing_rows += missing
                written += hi - lo
                ledger.write(json.dumps({"lo": lo, "hi": hi, "missing": missing}) + "\n")
                ledger.flush()
                if (lo // CHUNK_ROWS) % 10 == 0:
                    out.flush()
                    wall = time.time() - t0
                    print(f"    {species} {written:,} rows in {wall / 60:.1f} min "
                          f"({written / wall:,.0f} rows/s)", flush=True)
    out.flush()
    del out
    return {"rows": n, "read": written, "rows_missing_values": missing_rows,
            "seconds": round(time.time() - t0, 1)}


def stage_ontology() -> None:
    path = sources.staging() / f"cl-basic-{sources.CL_RELEASE}.json"
    if not path.exists():
        with urllib.request.urlopen(sources.CL_URL) as got:
            partial(path).write_bytes(got.read())
        partial(path).rename(path)
    print(f"  Cell Ontology {sources.CL_RELEASE}: {path.stat().st_size / 1e6:.1f} MB", flush=True)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--obs", action="store_true", help="the cell tables and the dataset table")
    ap.add_argument("--vectors", action="store_true", help="the scVI embeddings")
    ap.add_argument("--ontology", action="store_true", help="the Cell Ontology")
    ap.add_argument("--species", nargs="+", default=list(sources.SPECIES),
                    choices=list(sources.SPECIES))
    args = ap.parse_args()
    if not (args.obs or args.vectors or args.ontology):
        ap.error("name at least one of --obs, --vectors and --ontology")

    steps = Steps()
    report: dict = {}
    print(f"staging {sources.staging()}", flush=True)
    if args.ontology:
        with steps.step("ontology"):
            stage_ontology()
    if args.obs:
        with steps.step("obs"):
            stage_obs()
    if args.vectors:
        for species in args.species:
            with steps.step(f"vectors {species}"):
                report[species] = stage_vectors(species)
    report["seconds"] = dict(steps)
    report["peak_rss_gb"] = round(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 2**20, 2)
    log = sources.staging() / "stage-runs.jsonl"
    with open(log, "a") as f:
        f.write(json.dumps(dict(report, at=time.strftime("%Y-%m-%d %H:%M"))) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
