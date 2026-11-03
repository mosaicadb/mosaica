"""Build one system from one tier, run every workload against it, check every frame against the
oracle, and append one JSON record to the results file.

    python run.py --system mosaica --tier data/bench-compare/tiers/paperseek-1e6.parquet \\
        --cameras cameras/paperseek-1e6.json --out results/dev.jsonl

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
import hashlib
import json
import os
import platform
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

import numpy as np

from frame import EXTENT, GRID_OFFSET, K, SCREEN, TILE_PX, TYPE_TOP, YEAR_BINS, Camera, Filter
from oracle import VIEWER, Oracle, compare, compare_tables

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
    """The tier's SHA-256, refused unless `tiers-manifest.json` records the same for its name."""
    manifest = json.loads((HERE / "tiers-manifest.json").read_text())
    want = next((t["sha256"] for t in manifest["tiers"].values() if t["file"] == tier.name), None)
    if want is None:
        sys.exit(f"{tier.name} is not in tiers-manifest.json; derive it with prepare.py")
    h = hashlib.sha256()
    with tier.open("rb") as f:
        while chunk := f.read(1 << 24):
            h.update(chunk)
    if h.hexdigest() != want:
        sys.exit(f"{tier} has SHA-256 {h.hexdigest()}, not the manifest's {want}; derive it again "
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


def session_start(adapter, repeats: int) -> dict:
    """From starting a stopped server to the viewer's first whole-map frame, warm and cold."""
    whole = Camera(0, (EXTENT[0] + EXTENT[1]) / 2, (EXTENT[2] + EXTENT[3]) / 2, "whole")
    out = {}
    for mode in ("warm", "cold"):
        totals, phases = [], {}
        for _ in range(repeats):
            adapter.stop()
            if mode == "cold":
                adapter.evict()
            t0 = time.perf_counter()
            for k, v in adapter.start().items():
                phases.setdefault(k, []).append(v)
            _, frame_s = adapter.frame(whole, None)
            totals.append(time.perf_counter() - t0)
            phases.setdefault("first_frame_s", []).append(frame_s)
        out[mode] = {"total": summarise(totals), **{k: summarise(v) for k, v in phases.items()}}
    return out


def make_adapter(args):
    work = args.work / f"{args.system}-{args.tier.stem}"
    if args.system in ("mosaica", "mosaica+aggregate"):
        from adapters.mosaica import Mosaica
        return Mosaica(args.tier, work, args.image,
                       grid="underlay" if args.system == "mosaica" else "aggregate", nice=args.nice)
    if args.system == "clickhouse":
        from adapters.clickhouse import ClickHouse
        return ClickHouse(args.tier, work, nice=args.nice)
    sys.exit(f"unknown system {args.system}; write mosaica, mosaica+aggregate or clickhouse")


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
    oracle = Oracle(str(args.tier.resolve()))
    visible, rows = oracle.visible()
    adapter = make_adapter(args)
    record: dict = {
        "system": adapter.name,
        "started_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "development_only": not args.publishable,
        "tier": args.tier.name,
        "tier_sha256": tier_sha256,
        "cameras": args.cameras.name,
        "frame": {"screen": SCREEN, "tile_px": TILE_PX, "grid_offset": GRID_OFFSET, "k": K},
        "hardware": hardware(),
        "load_before": load_before,
        "foreign_mosaica_processes": foreign,
        "repeats": args.repeats,
        "viewer": {**VIEWER, "visible_rows": visible, "tier_rows": rows,
                   "visible_fraction": visible / rows},
        "access": adapter.access,
        "aggregation": {"type_top": TYPE_TOP, "year_bins": YEAR_BINS},
    }
    try:
        print(f"[{adapter.name}] building", file=sys.stderr, flush=True)
        record["build"] = adapter.build()
        adapter.start()
        record["version"] = adapter.version()
        workloads = {w: steps_of(cameras, w) for w in ("battery", "script", "filters")}

        print(f"[{adapter.name}] checking against the oracle", file=sys.stderr, flush=True)
        errors = {}
        for w, steps in workloads.items():
            per = []
            for camera, filt in steps:
                got, _ = adapter.frame(camera, filt)
                cmp = compare(got, oracle.frame(camera, filt), camera)
                cmp["label"] = camera.label + (f"/{filt.label}" if filt else "")
                cmp["bytes"] = got.bytes
                per.append(cmp)
            errors[w] = {
                "frames": len(per),
                "frames_exact": sum(1 for c in per if not (c["tile_abs_err"] or c["grid_abs_err"]
                                                        or c["sparse_points_missing"] or c["sparse_points_extra"]
                                                        or c["points_outside_tile"])),
                **{k: int(sum(c[k] for c in per)) for k in (
                    "matched_true", "tile_abs_err", "grid_abs_err", "grid_cells_wrong", "grid_cells_true",
                    "sparse_points_missing", "sparse_points_extra", "points_outside_tile", "points_dense",
                    "points_sent", "bytes")},
                "worst": sorted(per, key=lambda c: -(c["tile_abs_err"] + c["grid_abs_err"]
                                                     + c["sparse_points_missing"]))[:3],
            }
        per = []
        for camera, filt in workloads["filters"]:
            got, _ = adapter.tables(camera, filt)
            cmp = compare_tables(got, oracle.tables(camera, filt))
            cmp["label"] = camera.label + f"/{filt.label}"
            per.append(cmp)
        keys = [k for k in per[0] if k != "label"]
        errors["aggregation"] = {
            "tables": 2 * len(per),
            "tables_exact": sum((c["type_abs_err"] == 0) + (c["year_abs_err"] == 0) for c in per),
            **{k: int(sum(c[k] for c in per)) for k in keys},
            "worst": sorted(per, key=lambda c: -(c["type_abs_err"] + c["year_abs_err"]))[:3],
        }

        print(f"[{adapter.name}] warm runs", file=sys.stderr, flush=True)
        warm = {w: [] for w in workloads}
        by_zoom: dict[int, list[float]] = {}
        for _ in range(args.repeats):
            for w, steps in workloads.items():
                for camera, filt in steps:
                    _, t = adapter.frame(camera, filt)
                    warm[w].append(t)
                    if w == "battery":
                        by_zoom.setdefault(camera.zoom, []).append(t)
        record["warm"] = {w: summarise(v) for w, v in warm.items()}
        record["warm_battery_by_zoom"] = {str(z): summarise(v) for z, v in sorted(by_zoom.items())}
        print(f"[{adapter.name}] aggregation runs", file=sys.stderr, flush=True)
        agg = []
        for _ in range(args.repeats):
            for camera, filt in workloads["filters"]:
                agg.append(adapter.tables(camera, filt)[1])
        record["warm"]["aggregation"] = summarise(agg)
        # Read before any restart: each restart starts the server in a new cgroup.
        record["serve_memory"] = adapter.serve_memory()
        record["config"] = adapter.config()

        if not args.no_cold:
            print(f"[{adapter.name}] session start", file=sys.stderr, flush=True)
            record["session_start"] = session_start(adapter, args.repeats)
            print(f"[{adapter.name}] cold runs", file=sys.stderr, flush=True)
            cold = []
            for camera, filt in workloads["battery"]:
                adapter.restart(cold=True)
                _, t = adapter.frame(camera, filt)
                cold.append(t)
            record["cold"] = {"battery": summarise(cold),
                              "method": "server restarted, own caches deleted, own files' pages "
                                        "dropped with posix_fadvise; the machine's page cache "
                                        "otherwise untouched"}
        record["errors"] = errors
    finally:
        adapter.stop()
        adapter.remove()
    record["load_after"] = os.getloadavg()
    record["finished_at"] = datetime.now(timezone.utc).isoformat(timespec="seconds")
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open("a") as f:
        f.write(json.dumps(record) + "\n")
    print(json.dumps({k: record.get(k) for k in ("system", "build", "warm", "serve_memory", "session_start")}, indent=1))
    print(json.dumps({w: {k: v for k, v in e.items() if k != "worst"} for w, e in errors.items()}, indent=1))


if __name__ == "__main__":
    main()
