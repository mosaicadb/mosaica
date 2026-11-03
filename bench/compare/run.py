"""Build one system from one tier, run every workload against it, check every frame against the
oracle, and append one JSON record to the results file.

    python run.py --system mosaica --tier data/bench-compare/tiers/paperseek-1e6.parquet \\
        --cameras cameras/paperseek-1e6.json --out results/dev.jsonl [--dense points|counts]

`--dense points`, the default, asks every dense tile for its sample of K positions; `--dense
counts` is the earlier frame, which asks a dense tile for none.

The run, in order, every request made as the viewer in `viewer.json`:

1. Refuse to start where the one-minute load average is above `--max-load`, and record the load
   and any `mosaica` process this run did not start.
2. Build from nothing, timed, with peak memory and size on disk.
3. Start the server, and send every frame and every aggregation once, untimed, checking each
   against the oracle. The errors recorded are from this pass.
4. Warm: every frame workload `--repeats` times over, each frame timed on the wire, then the
   aggregation workload as many times, timed apart.
5. Session start: `--repeats` times warm (the server stopped and started, its caches kept) and as
   many cold (its caches deleted and its own files' pages dropped), the time from starting the
   server to the viewer's first whole-map frame, with its phases.
6. Cold: for each battery camera, stop the server, drop its caches and its own files' pages,
   start it, and time the first frame. Mosaica's first frame includes building the session's
   visible set, which every new viewer pays.
7. Record the server's peak memory, stop it and remove what it started.

Every record carries `"development_only": true` unless `--publishable` is given, which this stage
never does.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

import duckdb
import numpy as np
import pyarrow
import requests

from dataset import DATASET
from frame import (DENSE, EXTENT, GRID_OFFSET, K, SCREEN, SUBCELL_OFFSET, TILE_PX, TYPE_TOP,
                   YEAR_BINS, Camera, Filter)
from oracle import COUNT_ERRORS, POINT_ERRORS, TABLES, VIEWER, Oracle, compare, compare_tables
from prepare import sha256

HERE = Path(__file__).resolve().parent


def hardware() -> dict:
    cpu = next((l.split(":", 1)[1].strip() for l in open("/proc/cpuinfo") if l.startswith("model name")), "")
    mem = next(int(l.split()[1]) * 1024 for l in open("/proc/meminfo") if l.startswith("MemTotal"))
    return {"cpu": cpu, "logical_cpus": os.cpu_count(), "mem_bytes": mem,
            "kernel": platform.release(), "python": platform.python_version()}


def foreign_mosaica(own: set[int]) -> list[str]:
    out = subprocess.run(["ps", "-eo", "pid=,args="], capture_output=True, text=True).stdout
    found = []
    for line in out.splitlines():
        pid, _, args = line.strip().partition(" ")
        prog = args.split(" ", 1)[0]
        if prog.endswith("/mosaica") or prog == "mosaica":
            if int(pid) not in own:
                found.append(line.strip())
    return found


def check_tier(tier: Path) -> str:
    """The tier's SHA-256, refused unless the data set's committed manifest records the same for
    its name."""
    manifest = json.loads((HERE / DATASET.manifest).read_text())
    want = next((t["sha256"] for t in manifest["tiers"].values() if t["file"] == tier.name), None)
    if want is None:
        sys.exit(f"{tier.name} is not in {DATASET.manifest}; derive it with the data set's prepare "
                 f"script, or set BENCH_DATASET to its data set")
    got = sha256(tier)
    if got != want:
        sys.exit(f"{tier} has SHA-256 {got}, not the manifest's {want}; derive it again "
                 f"with prepare.py")
    return want


def summarise(latencies: list[float]) -> dict:
    a = np.array(latencies) * 1000
    return {"n": len(a), "p50_ms": float(np.percentile(a, 50)), "p99_ms": float(np.percentile(a, 99)),
            "min_ms": float(a.min()), "max_ms": float(a.max())}


def steps_of(cameras: dict, workload: str) -> list[tuple[Camera, Filter | None]]:
    out = []
    for step in cameras[workload]:
        c = step["camera"]
        f = step["filter"]
        out.append((Camera(c["zoom"], c["cx"], c["cy"], c["label"]),
                    Filter(f["leaves"], f["label"]) if f else None))
    return out


def step_key(camera: Camera, filt: Filter | None) -> str:
    return repr((camera.zoom, camera.cx, camera.cy, filt.leaves if filt else None))


#: Each part of a frame, and the comparison figures that say it is wrong.
PARTS = {"tiles": ("tile_abs_err",), "grid": ("grid_abs_err",), "points": POINT_ERRORS}


def same_points(a: dict, b: dict) -> bool:
    """Whether two frames sent the same positions from every tile, in any order."""
    keys = {t for t, p in a.items() if len(p)} | {t for t, p in b.items() if len(p)}
    order = lambda p: np.sort(np.ascontiguousarray(p).view(np.uint64).ravel())
    none = np.empty((0, 2), np.uint32)
    return all(np.array_equal(order(a.get(t, none)), order(b.get(t, none))) for t in keys)


class TimedCheck:
    """Every timed frame and table checked as the untimed pass checks it. Where the untimed answer
    to a step was right in a part (a frame's tile counts, its grid or its points, or a table), the
    timed answer is compared with the oracle's in that part, with every point check `compare`
    and `Oracle.dense_invalid` make, and its points must also be the untimed answer's; where the untimed answer was wrong, which its errors already
    count, the timed answer is compared with the untimed one, so the figure is repeatability."""

    def __init__(self, oracle: Oracle, dense: str):
        self.oracle = oracle
        self.dense = dense
        self.untimed: dict[str, tuple] = {}
        self.counts = {"frames": 0, "frames_mismatched": 0, "tables": 0, "tables_mismatched": 0}
        self.examples: list[str] = []

    def errors(self, camera: Camera, filt: Filter | None, got) -> dict:
        truth = self.oracle.frame(camera, filt)
        cmp = compare(got, truth, camera, self.dense)
        cmp["dense_points_invalid"] = self.oracle.dense_invalid(camera, filt, got, truth)
        return cmp

    def learn(self, camera: Camera, filt: Filter | None, got, cmp: dict) -> None:
        right = {part: not any(cmp[k] for k in keys) for part, keys in PARTS.items()}
        self.untimed[step_key(camera, filt)] = (got, right)

    def learn_tables(self, camera: Camera, filt: Filter | None, got: dict) -> None:
        self.untimed[step_key(camera, filt) + "/tables"] = got

    def frame(self, camera: Camera, filt: Filter | None, got) -> None:
        before, right = self.untimed.get(step_key(camera, filt), (None, dict.fromkeys(PARTS, True)))
        cmp = self.errors(camera, filt, got)
        nonzero = lambda grid: {c: n for c, n in grid.items() if n}
        repeat = {"tiles": lambda: got.tiles == before.tiles,
                  "grid": lambda: nonzero(got.grid) == nonzero(before.grid),
                  "points": lambda: same_points(got.points, before.points)}
        # A right untimed sample must also come back the same: the oracle cannot tell one valid
        # sample of a dense tile from another.
        wrong = [part for part, keys in PARTS.items()
                 if (any(cmp[k] for k in keys)
                     or (part == "points" and before is not None and not repeat[part]())
                     if right[part] else not repeat[part]())]
        self.counts["frames"] += 1
        if wrong:
            self.counts["frames_mismatched"] += 1
            self.examples.append(f"{camera.label}/{filt.label if filt else 'none'}: {', '.join(wrong)}")

    def tables(self, camera: Camera, filt: Filter | None, got: dict) -> None:
        before = self.untimed[step_key(camera, filt) + "/tables"]
        truth = self.oracle.tables(camera, filt)
        nonzero = lambda t: {k: v for k, v in t.items() if v}
        for name in truth:
            want = truth[name] if nonzero(before.get(name, {})) == truth[name] else nonzero(before[name])
            self.counts["tables"] += 1
            if nonzero(got.get(name, {})) != want:
                self.counts["tables_mismatched"] += 1
                self.examples.append(f"{camera.label}/{filt.label}: {name} table")

    def record(self) -> dict:
        return {**self.counts, "examples": self.examples[:10],
                "method": "every timed frame and table, with dense_invalid on every timed frame"}


def session_start(adapter, repeats: int, check: TimedCheck) -> dict:
    """Warm and cold, two figures from a stopped server: `server`, from starting it until its data
    is loaded and it answers a request that is not the viewer's, and `session`, from opening the viewer's session or
    connections to the end of its first whole-map frame, which for Mosaica includes building the
    viewer's visible set on its first read."""
    whole = Camera(0, (EXTENT[0] + EXTENT[1]) / 2, (EXTENT[2] + EXTENT[3]) / 2, "whole")
    out = {}
    for mode in ("warm", "cold"):
        server, session = [], []
        for _ in range(repeats):
            adapter.stop()
            if mode == "cold":
                adapter.evict()
            t0 = time.perf_counter()
            adapter.start()
            t1 = time.perf_counter()
            adapter.connect()
            got, _ = adapter.frame(whole, None)
            server.append(t1 - t0)
            session.append(time.perf_counter() - t1)
            check.frame(whole, None, got)
        out[mode] = {"server": summarise(server), "session": summarise(session)}
    return out


def harness_commit() -> str:
    """The commit the harness ran from, with `-dirty` where its directory has changes."""
    git = lambda *a: subprocess.run(["git", "-C", str(HERE), *a], capture_output=True,
                                    text=True).stdout.strip()
    return git("rev-parse", "HEAD") + ("-dirty" if git("status", "--porcelain", ".") else "")


def make_adapter(args):
    work = args.work / f"{args.system}-{args.caches}-{args.dense}-{args.tier.stem}-{os.getpid()}"
    common = {"nice": args.nice, "caches": args.caches, "dense": args.dense}
    if args.system == "mosaica":
        from adapters.mosaica import Mosaica
        return Mosaica(args.tier, work, args.image, **common)
    if args.system == "clickhouse":
        from adapters.clickhouse import ClickHouse
        return ClickHouse(args.tier, work, **common)
    if DATASET.name != "paperseek":
        sys.exit(f"the {args.system} adapter has no schema for {DATASET.name}; run mosaica or "
                 f"clickhouse")
    if args.system in ("postgis", "postgis+leakproof", "postgis+integer"):
        from adapters.postgis import PostGIS
        return PostGIS(args.tier, work, arm=args.system, **common)
    if args.system == "opensearch":
        from adapters.opensearch import OpenSearch
        return OpenSearch(args.tier, work, **common)
    sys.exit(f"unknown system {args.system}; write mosaica, clickhouse, postgis, "
             f"postgis+leakproof, postgis+integer or opensearch")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--system", required=True)
    ap.add_argument("--tier", type=Path, required=True)
    ap.add_argument("--cameras", type=Path, required=True)
    ap.add_argument("--image", default="bench-compare/mosaica",
                    help="the Mosaica image, built from the repository's Dockerfile (README step 4)")
    ap.add_argument("--work", type=Path, help="default: work/ beside the tier's directory")
    ap.add_argument("--out", type=Path, default=HERE / "results/dev.jsonl")
    ap.add_argument("--repeats", type=int, default=5)
    ap.add_argument("--caches", choices=("compute", "defaults"), default="compute",
                    help="compute: every cache kept between requests off; defaults: the system's own")
    ap.add_argument("--dense", choices=DENSE, default="points",
                    help="points: K positions from each dense tile; counts: none required")
    ap.add_argument("--max-load", type=float, default=3.0)
    ap.add_argument("--no-cold", action="store_true")
    ap.add_argument("--nice", type=int, default=0,
                    help="niceness of every system's build (19 on a shared machine); servers run at 0")
    ap.add_argument("--publishable", action="store_true",
                    help="drop the development-only mark; only on a quiet, dedicated machine")
    args = ap.parse_args()
    args.work = args.work or args.tier.resolve().parent.parent / "work"

    load_before = os.getloadavg()
    if load_before[0] > args.max_load:
        sys.exit(f"one-minute load is {load_before[0]:.1f}, above --max-load {args.max_load}; wait "
                 f"for the machine to quieten or raise --max-load")
    tier_sha256 = check_tier(args.tier)
    foreign = foreign_mosaica(set())
    cameras = json.loads(args.cameras.read_text())
    if cameras["tier"] != args.tier.name:
        sys.exit(f"{args.cameras} was made for {cameras['tier']}, not {args.tier.name}; make the "
                 f"cameras for this tier with cameras.py")
    oracle = Oracle(str(args.tier.resolve()), copy=DATASET.oracle_copy)
    visible, rows = oracle.visible()
    adapter = make_adapter(args)
    record: dict = {
        "system": adapter.name,
        "started_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "development_only": not args.publishable,
        "dataset": DATASET.name,
        "tier": args.tier.name,
        "tier_sha256": tier_sha256,
        "caches": args.caches,
        "dense": args.dense,
        "harness_commit": harness_commit(),
        "cameras": args.cameras.name,
        "cameras_sha256": sha256(args.cameras),
        "packages": {"duckdb": duckdb.__version__, "pyarrow": pyarrow.__version__,
                     "numpy": np.__version__, "requests": requests.__version__},
        "frame": {"screen": SCREEN, "tile_px": TILE_PX, "grid_offset": GRID_OFFSET, "k": K,
                  "dense": args.dense, "subcell_offset": SUBCELL_OFFSET},
        "hardware": hardware(),
        "load_before": load_before,
        "foreign_mosaica_processes": foreign,
        "repeats": args.repeats,
        "cold_runs": not args.no_cold,
        "viewer": {**VIEWER, "visible_rows": visible, "tier_rows": rows,
                   "visible_fraction": visible / rows},
        "access": adapter.access,
        "aggregation": {"type_top": TYPE_TOP, "year_bins": YEAR_BINS},
    }
    try:
        print(f"[{adapter.name}] building", file=sys.stderr, flush=True)
        record["build"] = adapter.build()
        adapter.start()
        adapter.connect()
        record["version"] = adapter.version()
        workloads = {w: steps_of(cameras, w) for w in ("battery", "script", "filters")}

        print(f"[{adapter.name}] checking against the oracle", file=sys.stderr, flush=True)
        errors = {}
        check = TimedCheck(oracle, args.dense)
        for w, steps in workloads.items():
            per = []
            for camera, filt in steps:
                got, _ = adapter.frame(camera, filt)
                cmp = check.errors(camera, filt, got)
                check.learn(camera, filt, got, cmp)
                cmp.update(oracle.coverage(camera, filt, got, oracle.frame(camera, filt)))
                cmp["label"] = camera.label + (f"/{filt.label}" if filt else "")
                cmp["bytes"] = got.bytes
                per.append(cmp)
            errors[w] = {
                "frames": len(per),
                "frames_exact": sum(1 for c in per if not any(c[k] for k in COUNT_ERRORS + POINT_ERRORS)),
                **{k: int(sum(c[k] for c in per)) for k in (
                    "matched_true", *COUNT_ERRORS, *POINT_ERRORS, "grid_cells_wrong", "grid_cells_true",
                    "points_dense", "points_sent", "bytes", "dense_tiles_sampled", "dense_cover",
                    "dense_chi2_df")},
                **{k: float(sum(c[k] for c in per)) for k in ("dense_cover_expected", "dense_chi2")},
                "worst": sorted(per, key=lambda c: -sum(c[k] for k in COUNT_ERRORS + POINT_ERRORS))[:3],
            }
        per = []
        for camera, filt in workloads["filters"]:
            got, _ = adapter.tables(camera, filt)
            cmp = compare_tables(got, oracle.tables(camera, filt))
            check.learn_tables(camera, filt, got)
            cmp["label"] = camera.label + f"/{filt.label}"
            per.append(cmp)
        keys = [k for k in per[0] if k != "label"]
        errors["aggregation"] = {
            "tables": 2 * len(per),
            "tables_exact": sum(c[f"{t}_abs_err"] == 0 for c in per for t in TABLES),
            **{k: int(sum(c[k] for c in per)) for k in keys},
            "worst": sorted(per, key=lambda c: -sum(c[f"{t}_abs_err"] for t in TABLES))[:3],
        }

        print(f"[{adapter.name}] warm runs", file=sys.stderr, flush=True)
        warm = {w: [] for w in workloads}
        by_zoom: dict[int, list[float]] = {}
        for _ in range(args.repeats):
            for w, steps in workloads.items():
                for camera, filt in steps:
                    got, t = adapter.frame(camera, filt)
                    check.frame(camera, filt, got)
                    warm[w].append(t)
                    if w == "battery":
                        by_zoom.setdefault(camera.zoom, []).append(t)
        record["warm"] = {w: summarise(v) for w, v in warm.items()}
        record["warm_battery_by_zoom"] = {str(z): summarise(v) for z, v in sorted(by_zoom.items())}
        print(f"[{adapter.name}] aggregation runs", file=sys.stderr, flush=True)
        agg = []
        for _ in range(args.repeats):
            for camera, filt in workloads["filters"]:
                got, t = adapter.tables(camera, filt)
                check.tables(camera, filt, got)
                agg.append(t)
        record["warm"]["aggregation"] = summarise(agg)
        # Read before any restart: each restart starts the server in a new cgroup.
        record["serve_memory"] = adapter.serve_memory()
        record["config"] = adapter.config()

        if not args.no_cold:
            print(f"[{adapter.name}] session start", file=sys.stderr, flush=True)
            record["session_start"] = session_start(adapter, args.repeats, check)
            print(f"[{adapter.name}] cold runs", file=sys.stderr, flush=True)
            cold = []
            for camera, filt in workloads["battery"]:
                adapter.restart(cold=True)
                got, t = adapter.frame(camera, filt)
                check.frame(camera, filt, got)
                cold.append(t)
            # After the last cold restart and its one frame: the server's own pages, read again.
            record["serve_memory_after_cold"] = adapter.serve_memory()
            record["cold"] = {"battery": summarise(cold),
                              "method": "server restarted, own caches deleted, own files' pages "
                                        "dropped with posix_fadvise; the machine's page cache "
                                        "otherwise untouched"}
        errors["timed"] = check.record()
        record["errors"] = errors
    except BaseException:
        print(f"[{adapter.name}] failed; its work directory is kept at {adapter.work}",
              file=sys.stderr, flush=True)
        raise
    finally:
        adapter.stop()
        adapter.remove()
    record["load_after"] = os.getloadavg()
    record["finished_at"] = datetime.now(timezone.utc).isoformat(timespec="seconds")
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open("a") as f:
        f.write(json.dumps(record) + "\n")
    shutil.rmtree(adapter.work)
    print(json.dumps({k: record.get(k) for k in ("system", "build", "warm", "serve_memory", "session_start")}, indent=1))
    print(json.dumps({w: {k: v for k, v in e.items() if k != "worst"} for w, e in errors.items()}, indent=1))


if __name__ == "__main__":
    main()
