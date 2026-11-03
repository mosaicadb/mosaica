"""Turn run records into the design's tables, one column per system, as Markdown.

    python tables.py results/dev.jsonl [--tier osm-1e7.parquet]

The latest record per system and tier is used. A table drawn from any development-only record says
so in its heading.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

GIB = 1 << 30


def latest(records: list[dict]) -> dict[tuple[str, str], dict]:
    out = {}
    for r in records:
        out[(r["tier"], r["system"])] = r
    return out


def ms(s: dict | None) -> str:
    return "–" if not s else f"{s['p50_ms']:.1f} / {s['p99_ms']:.1f}"


def size(b: int | None) -> str:
    return "–" if b is None else (f"{b / GIB:.2f} GiB" if b >= GIB / 4 else f"{b / (1 << 20):.0f} MiB")


def error_cell(e: dict) -> str:
    wrong = e["tile_abs_err"] + e["grid_abs_err"]
    pts = e["sparse_points_missing"] + e["sparse_points_extra"] + e["points_outside_tile"]
    return (f"{e['frames_exact']}/{e['frames']} exact; count err {wrong:,} of {e['matched_true']:,}; "
            f"point err {pts:,}")


def table(tier: str, runs: dict[str, dict]) -> str:
    systems = sorted(runs)
    dev = any(r.get("development_only", True) for r in runs.values())
    head = f"### {tier}" + (" (development only: not for publication)" if dev else "")
    rows = [("Version", lambda r: r["version"]["version"]),
            ("Build time", lambda r: f"{r['build']['build_s']:.1f} s"),
            ("Build peak RSS", lambda r: size(r["build"].get("peak_rss_bytes"))),
            ("Build peak cgroup memory (incl. page cache)", lambda r: size(r["build"].get("cgroup_peak_bytes"))),
            ("Size on disk", lambda r: size(r["build"].get("disk_bytes"))),
            ("Serve peak RSS", lambda r: size(r["serve_memory"].get("peak_rss_bytes"))),
            ("Serve peak cgroup memory (incl. page cache)", lambda r: size(r["serve_memory"].get("cgroup_peak_bytes"))),
            ("Cold battery p50 / p99 (ms)", lambda r: ms(r.get("cold", {}).get("battery")))]
    zooms = sorted({z for r in runs.values() for z in r["warm_battery_by_zoom"]}, key=int)
    for z in zooms:
        rows.append((f"Warm battery, zoom {z}, p50 / p99 (ms)",
                     lambda r, z=z: ms(r["warm_battery_by_zoom"].get(z))))
    rows += [("Warm camera script p50 / p99 (ms)", lambda r: ms(r["warm"]["script"])),
             ("Warm filter change p50 / p99 (ms)", lambda r: ms(r["warm"]["filters"]))]
    for w in ("battery", "script", "filters"):
        rows.append((f"Error versus oracle, {w}", lambda r, w=w: error_cell(r["errors"][w])))
        rows.append((f"Points sent, {w} (from dense tiles)",
                     lambda r, w=w: f"{r['errors'][w]['points_sent']:,} ({r['errors'][w]['points_dense']:,})"))
        rows.append((f"Bytes received, {w}", lambda r, w=w: size(r["errors"][w]["bytes"])))
    rows.append(("Load average before / after", lambda r: f"{r['load_before'][0]:.1f} / {r['load_after'][0]:.1f}"))
    out = [head, "", "| | " + " | ".join(systems) + " |", "|---|" + "---|" * len(systems)]
    for name, f in rows:
        cells = []
        for s in systems:
            try:
                cells.append(str(f(runs[s])))
            except (KeyError, TypeError):
                cells.append("–")
        out.append(f"| {name} | " + " | ".join(cells) + " |")
    return "\n".join(out)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("results", type=Path, nargs="+")
    ap.add_argument("--tier")
    args = ap.parse_args()
    records = [json.loads(l) for p in args.results for l in p.read_text().splitlines() if l.strip()]
    by = latest(records)
    for tier in sorted({t for t, _ in by}):
        if args.tier and tier != args.tier:
            continue
        print(table(tier, {s: r for (t, s), r in by.items() if t == tier}))
        print()


if __name__ == "__main__":
    main()
