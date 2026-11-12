"""The one route to a position at 2.33x10^8 — fit a layout on a staged sample, place the rest off
the share.

Rung 4's route with the one change this rung's disk forces. There, 209 GB of vectors were staged
whole and the placement pass read them off a local memmap; here the matrix is **233,055,986 x 768
float16 = 346 GB** against 217 GB free on this box, so nothing but the fit sample is ever local:

1. **Fit.** `stage.py --fit` writes `staging/fit.f16` — 2,500,000 rows drawn evenly from all 666
   files, L2-normalised, 3.84 GB. That block goes on the card as one CAGRA index, its own kNN graph
   goes to cuML's UMAP as a `precomputed_knn`, and that is the layout.
2. **Place.** A second pass reads each of the 666 files' `emb` column once **off the share**,
   normalises it, searches the same index, and positions every row at the
   **similarity-weighted mean of its 15 fit-set neighbours' positions**. The fit rows are then
   overwritten with their own UMAP positions, so every row goes through one code path. Nothing is
   kept but the 233,055,986 x 2 float32 layout — 1.86 GB.

**The share is the constraint, not the card.** Measured 2026-09-03: one reader sustains ~20 MB/s
off this SMB mount and two or more sustain ~30 MB/s, so the 344 GB pass is ~3.2 hours of I/O
against ~45 minutes of GPU search at rung 4's measured placement rate. `place_from_share` therefore
reads ahead on a small thread pool and hands the card whole row groups; the row-group *order* is
preserved because each batch carries its own global offset and writes its own slice.

**BioCLIP-2 publishes unnormalised embeddings** (norms 35.6 … 69.0 over the first row group,
measured), so both halves normalise explicitly. The metric is still named `cosine` so the index
does its own normalisation too and a change to the staging convention cannot silently change the
graph.

⊘ **The layout is not reproducible under a seed**, for rung 3's and rung 4's reason: CAGRA's index
build is approximate and takes none, so UMAP is handed a different graph each run.

    ~/venvs/projection/bin/python -m corpora.treeoflife.routes --rows 2000000

reports index build time, search throughput, the layout's wall and the peak device memory each
stage reached.
"""

from __future__ import annotations

import argparse
import collections
import concurrent.futures as cf
import json
import sys
import time
from pathlib import Path

import numpy as np

from ..common.fit_place import (
    DeviceWatermark,
    Placer,
    fit_layout,
    knn_graph,
    layout,
    normalise,
    step,
)

#: Row groups read ahead of the card while it searches. Three readers is what the share sustains;
#: the queue is one deeper so a reader never waits on the GPU.
READ_WORKERS = 3
READ_QUEUE = 4

#: Attempts at one row group off the share before the pass refuses. This box drops a read in a
#: multi-hour SMB pass — a 2026-09-03 placement died on `ZSTD decompression failed` from a file the
#: fit pass had read cleanly — so a read is retried and the count is reported.
RETRIES = 4


# ----------------------------------------------------------------------------- the place half


def placement_plan(files) -> tuple[list[tuple[int, int, int, int]], int]:
    """`[(file, row group, global offset, rows)]` for every source row group, and the row total.

    From the 666 footers, which are ~50 ms apiece over SMB and are the only way to know a group's
    true row count. This is the unit of work, of the ledger and of the retry below.
    """
    import pyarrow.parquet as pq

    plan: list[tuple[int, int, int, int]] = []
    at = 0
    for i, path in enumerate(files):
        meta = pq.ParquetFile(path).metadata
        for g in range(meta.num_row_groups):
            rows = meta.row_group(g).num_rows
            plan.append((i, g, at, rows))
            at += rows
    return plan, at


def read_group(files, job, retries: int = RETRIES) -> tuple[int, int, int, np.ndarray | None, int]:
    """One row group's `emb`, normalised — retried, and **`None` where the source cannot be read**.

    ⊘ **One row group of TreeOfLife-200M is corrupt at the source.** `train-00035-of-00666.parquet`
    row group 4 answers `ZSTD decompression failed: Src size is incorrect` on its `emb` column at
    every attempt, from the share and from a byte copy of the file on local disk, while the same
    group's `uuid` and rank columns read cleanly — so it is the publisher's bytes rather than this
    box's SMB (measured 2026-09-03, three reads off the share and one off a local copy).

    50,000 vectors of 233,055,986 is 0.021% of the corpus, and the campaign's rule for an input is
    to **ignore and report rather than refuse** (`CLAUDE.md`, *How strict to be*). So the read
    returns `None`, the caller places those rows at the layout's centroid, and the count, the file
    and the group travel in the manifest and in the rung's README. Refusing would cost the rung for
    a fifth of a tenth of a per cent, and a silent zero vector would put them at a position the
    index chose for the origin.

    The retry stays for the transient case: this box does drop an SMB read in a multi-hour pass,
    and the run records how many attempts it needed.
    """
    import pyarrow.parquet as pq

    i, g, offset, rows = job
    for attempt in range(retries):
        try:
            table = pq.ParquetFile(files[i]).read_row_group(g, columns=["emb"], use_threads=False)
            values = table.column("emb").combine_chunks()
            block = np.asarray(
                values.values.to_numpy(zero_copy_only=False), dtype=np.float32
            ).reshape(table.num_rows, -1)
            return i, g, offset, normalise(block), attempt
        except Exception as exc:  # noqa: BLE001 — any read fault is retried, then refused
            if attempt == retries - 1:
                print(f"    ⊘ {files[i].name} group {g} is unreadable after {retries} attempts "
                      f"({type(exc).__name__}: {exc}); its {rows:,} rows take the layout's "
                      f"centroid", flush=True)
                return i, g, offset, None, attempt
            print(f"    ⊘ {files[i].name} group {g}: {type(exc).__name__}: {exc} — "
                  f"retry {attempt + 1}/{retries - 1}", flush=True)
            time.sleep(2.0 * (attempt + 1))
    raise AssertionError("unreachable")


def share_batches(files, plan, workers: int = READ_WORKERS):
    """`(file, group, global offset, normalised float16 block, retries)` per planned row group.

    **The reads run ahead of the card on a small pool and the results are reordered.** The pool
    holds `READ_QUEUE + workers` in flight and yields them in plan order, so the consumer never
    sees a group out of place and the share is never idle while the GPU searches.
    """
    with cf.ThreadPoolExecutor(workers) as pool:
        pending: collections.deque = collections.deque()
        it = iter(plan)
        for job in it:
            pending.append(pool.submit(read_group, files, job))
            if len(pending) >= READ_QUEUE + workers:
                break
        while pending:
            got = pending.popleft().result()
            yield got
            job = next(it, None)
            if job is not None:
                pending.append(pool.submit(read_group, files, job))


def place_from_share(fit_block, fit_xy, fit_rows, n: int, t: dict, files, checkpoint: Path):
    """Every row's position, searched against one CAGRA index over the fit set — **resumable**.

    The pass is ~2.5 hours of share I/O and this box drops a read in it, so the positions go
    straight into a memmap on local disk and a ledger names the row groups already placed. A rerun
    reads the ledger, plans only what is missing, and costs the share only that. The fit rows are
    overwritten with their own UMAP positions at the end, so every row goes through one code path.
    """
    checkpoint.mkdir(parents=True, exist_ok=True)
    path = checkpoint / "place.f32"
    mode = "r+" if path.exists() and path.stat().st_size == n * 8 else "w+"
    xy = np.memmap(path, dtype=np.float32, mode=mode, shape=(n, 2))

    ledger_path = checkpoint / "place-ledger.jsonl"
    done: set[tuple[int, int]] = set()
    if ledger_path.exists():
        for line in ledger_path.read_text().splitlines():
            if line.strip():
                rec = json.loads(line)
                done.add((rec["file"], rec["group"]))

    full, total = placement_plan(files)
    assert total == n, f"the share holds {total:,} rows against the corpus's {n:,}"
    plan = [job for job in full if (job[0], job[1]) not in done]
    t["groups"], t["groups_resumed"] = len(full), len(done)
    placed_before = sum(rows for i, g, _, rows in full if (i, g) in done)
    print(f"    {len(plan):,} of {len(full):,} row groups to place "
          f"({placed_before:,} rows already on disk)", flush=True)

    placed, retries, last_report = 0, 0, 0
    unreadable: list[dict] = []
    # Where a row whose vector cannot be read goes. The middle of the layout, so the defect is one
    # visible pile at one place rather than 50,000 points smeared invisibly through real structure.
    centroid = fit_xy.mean(axis=0).astype(np.float32)
    t0 = time.time()
    with step("place", t):
        placer = Placer(fit_block, fit_xy)
        with open(ledger_path, "a") as ledger:
            for i, g, offset, block, tries in share_batches(files, plan):
                retries += tries
                if block is None:
                    rows = next(r for f, gg, _, r in full if (f, gg) == (i, g))
                    xy[offset : offset + rows] = centroid
                    unreadable.append({"file": files[i].name, "group": g, "rows": rows})
                    placed += rows
                    ledger.write(json.dumps(
                        {"file": i, "group": g, "rows": rows, "unreadable": True}) + "\n")
                    ledger.flush()
                    continue
                xy[offset : offset + len(block)] = placer.place(block)
                placed += len(block)
                ledger.write(json.dumps({"file": i, "group": g, "rows": len(block)}) + "\n")
                ledger.flush()
                del block
                if (placed // 10_000_000) != (last_report // 10_000_000):
                    last_report = placed
                    wall = time.time() - t0
                    xy.flush()
                    print(f"    placed {placed + placed_before:,}/{n:,} in {wall / 60:.1f} min "
                          f"({placed / wall:,.0f} rows/s, {placed * 1536 / wall / 1e6:.1f} MB/s "
                          f"off the share, {retries} retry(ies))", flush=True)
        placer.close()
    xy.flush()
    t["placed"], t["read_retries"] = placed + placed_before, retries
    t["unreadable_groups"] = unreadable
    t["unreadable_rows"] = sum(u["rows"] for u in unreadable)
    if unreadable:
        print(f"    ⊘ {t['unreadable_rows']:,} row(s) in {len(unreadable)} unreadable source row "
              f"group(s) carry the layout's centroid and not a position of their own: "
              + ", ".join(f"{u['file']} group {u['group']}" for u in unreadable), flush=True)
    assert placed + placed_before == n, f"placed {placed + placed_before:,} against {n:,}"

    out = np.array(xy, dtype=np.float32)
    del xy
    out[fit_rows] = fit_xy
    return out


#: Two views, and only one of them is a route. `bioclip` is the embedding layout this module
#: produces; `geo` is the GBIF join's own longitude and latitude, projected inside the build.
ROUTES = {"bioclip": fit_layout}


# ------------------------------------------------------------------ the measurement driver


def main() -> None:
    ap = argparse.ArgumentParser(description="measure the route before committing a 233M run")
    ap.add_argument("--rows", type=int, default=2_000_000,
                    help="rows of the staged fit sample to fit over; 0 takes all of it")
    ap.add_argument("--managed", action="store_true", help="RMM managed memory for the layout")
    ap.add_argument("--graph-only", action="store_true", help="stop after the kNN graph")
    ap.add_argument("--report", type=Path, default=None, help="append the timings here as JSON")
    args = ap.parse_args()

    from . import sources

    matrix, rows, meta = sources.fit_matrix()
    n = meta["rows"] if not args.rows else min(args.rows, meta["rows"])
    block = np.ascontiguousarray(matrix[:n])
    t: dict = {"rows": n, "fit_staged": meta["rows"], "dim": meta["dim"],
               "managed": args.managed, "measured_at": time.strftime("%Y-%m-%d %H:%M")}
    print(f"{n:,} rows x {meta['dim']} off the staged fit sample", flush=True)

    with DeviceWatermark() as watch:
        with step("knn", t):
            nbi, nbd = knn_graph(block, t)
    t["graph_peak_vram_gb"] = round(watch.peak_gb, 2)
    print(f"  graph peak device memory {t['graph_peak_vram_gb']:.2f} GB, "
          f"self-first {t['self_first']:.3%}", flush=True)

    if not args.graph_only:
        with DeviceWatermark() as watch:
            xy = layout(nbi, nbd, t, managed=args.managed)
        t["layout_peak_vram_gb"] = round(watch.peak_gb, 2)
        t["layout_vram_bytes_per_row"] = round(watch.peak / len(nbi), 1)
        t["bounds"] = {"x": [float(xy[:, 0].min()), float(xy[:, 0].max())],
                       "y": [float(xy[:, 1].min()), float(xy[:, 1].max())]}
        print(f"  layout peak device memory {t['layout_peak_vram_gb']:.2f} GB "
              f"({t['layout_vram_bytes_per_row']:.0f} bytes/row), "
              f"x {t['bounds']['x']}, y {t['bounds']['y']}", flush=True)

    print(json.dumps(t, indent=2))
    if args.report:
        held = json.loads(args.report.read_text()) if args.report.exists() else []
        held.append(t)
        args.report.write_text(json.dumps(held, indent=2) + "\n")


if __name__ == "__main__":
    sys.exit(main())
