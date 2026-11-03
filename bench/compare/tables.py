"""Turn run records into the design's tables, one column per system, as Markdown.

    python tables.py results/dev.jsonl [--tier paperseek-1e6.parquet]

The latest record per tier, system and cache setting is used, and a system run with its default
caches has a column of its own. A table drawn from any development-only record says so in its
heading. Records from different harness commits or camera files are refused in one table.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

GIB = 1 << 30


def column(r: dict) -> str:
    return r["system"] + (" (default caches)" if r["caches"] == "defaults" else "")


def latest(records: list[dict]) -> dict[tuple[str, str], dict]:
    out = {}
    for r in records:
        out[(r["tier"], column(r))] = r
    return out


def check_comparable(tier: str, runs: dict[str, dict]) -> None:
    for key in ("harness_commit", "cameras_sha256"):
        seen = {r[key] for r in runs.values()}
        if len(seen) > 1:
            raise SystemExit(f"the {tier} records come from different {key} values {sorted(seen)}; "
                             f"rerun the older systems, or pass only the records of one run")


def ms(s: dict | None) -> str:
    return "–" if not s else f"{s['p50_ms']:.1f} / {s['p99_ms']:.1f}"


def size(b: int | None) -> str:
    return "–" if b is None else (f"{b / GIB:.2f} GiB" if b >= GIB / 4 else f"{b / (1 << 20):.0f} MiB")


def error_cell(e: dict) -> str:
    wrong = e["tile_abs_err"] + e["grid_abs_err"]
    pts = (e["sparse_points_missing"] + e["sparse_points_extra"] + e["points_outside_tile"]
           + e["dense_points_invalid"])
    return (f"{e['frames_exact']}/{e['frames']} exact; count err {wrong:,} of {e['matched_true']:,}; "
            f"point err {pts:,}")


def agg_error_cell(e: dict) -> str:
    wrong = e["type_abs_err"] + e["year_abs_err"]
    return (f"{e['tables_exact']}/{e['tables']} exact; count err {wrong:,} of "
            f"{e['type_total_true'] + e['year_total_true']:,}")


def table(tier: str, runs: dict[str, dict]) -> str:
    systems = sorted(runs)
    dev = any(r.get("development_only", True) for r in runs.values())
    head = f"### {tier}" + (" (development only: not for publication)" if dev else "")
    rows = [("Version", lambda r: r["version"]["version"]),
            ("Build time", lambda r: f"{r['build']['build_s']:.1f} s"),
            ("Build peak RSS", lambda r: size(r["build"].get("peak_rss_bytes"))),
            ("Build peak cgroup memory (incl. page cache)", lambda r: size(r["build"].get("cgroup_peak_bytes"))),
            ("Size on disk (ClickHouse: data directory; Mosaica: bundle)", lambda r: size(r["build"].get("disk_bytes"))),
            ("Serve peak RSS", lambda r: size(r["serve_memory"].get("peak_rss_bytes"))),
            ("Serve peak cgroup memory (incl. page cache)", lambda r: size(r["serve_memory"].get("cgroup_peak_bytes"))),
            ("Server start to ready, warm, p50 / p99 (ms)", lambda r: ms(r["session_start"]["warm"]["server"])),
            ("Server start to ready, cold, p50 / p99 (ms)", lambda r: ms(r["session_start"]["cold"]["server"])),
            ("Session start to first frame, warm, p50 / p99 (ms)", lambda r: ms(r["session_start"]["warm"]["session"])),
            ("Session start to first frame, cold, p50 / p99 (ms)", lambda r: ms(r["session_start"]["cold"]["session"])),
            ("Cold battery p50 / p99 (ms)", lambda r: ms(r.get("cold", {}).get("battery")))]
    zooms = sorted({z for r in runs.values() for z in r["warm_battery_by_zoom"]}, key=int)
    for z in zooms:
        rows.append((f"Warm battery, zoom {z}, p50 / p99 (ms)",
                     lambda r, z=z: ms(r["warm_battery_by_zoom"].get(z))))
    rows += [("Warm camera script p50 / p99 (ms)", lambda r: ms(r["warm"]["script"])),
             ("Warm filter change p50 / p99 (ms)", lambda r: ms(r["warm"]["filters"])),
             ("Warm aggregation p50 / p99 (ms)", lambda r: ms(r["warm"]["aggregation"]))]
    for w in ("battery", "script", "filters"):
        rows.append((f"Error versus oracle, {w}", lambda r, w=w: error_cell(r["errors"][w])))
        rows.append((f"Points sent, {w} (from dense tiles)",
                     lambda r, w=w: f"{r['errors'][w]['points_sent']:,} ({r['errors'][w]['points_dense']:,})"))
        rows.append((f"Bytes received, {w}", lambda r, w=w: size(r["errors"][w]["bytes"])))
    rows += [("Error versus oracle, aggregation", lambda r: agg_error_cell(r["errors"]["aggregation"])),
             ("Viewer's visible fraction", lambda r: f"{r['viewer']['visible_rows']:,} of "
              f"{r['viewer']['tier_rows']:,} ({r['viewer']['visible_fraction']:.1%})"),
             ("Access enforced by", lambda r: r["access"]),
             ("Harness commit", lambda r: r["harness_commit"][:12] + ("-dirty" if r["harness_commit"].endswith("-dirty") else "")),
             ("Load average before / after", lambda r: f"{r['load_before'][0]:.1f} / {r['load_after'][0]:.1f}")]
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
        runs = {s: r for (t, s), r in by.items() if t == tier}
        check_comparable(tier, runs)
        print(table(tier, runs))
        print()


if __name__ == "__main__":
    main()
