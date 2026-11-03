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

from oracle import COUNT_ERRORS, POINT_ERRORS

GIB = 1 << 30
#: The fields a record must carry to be tabled; an older record without them is refused.
REQUIRED = ("system", "caches", "tier", "tier_sha256", "harness_commit", "cameras_sha256",
            "version", "build", "warm", "errors", "session_start", "serve_memory")


def column(r: dict) -> str:
    return r["system"] + (" (default caches)" if r["caches"] == "defaults" else "")


def latest(records: list[dict]) -> dict[tuple[str, str], dict]:
    out = {}
    for r in records:
        missing = [k for k in REQUIRED if k not in r]
        if missing:
            raise SystemExit(f"a record of {r.get('system', '?')} from {r.get('started_at', '?')} "
                             f"lacks {', '.join(missing)}: it predates this harness; pass only "
                             f"records from one run of it")
        out[(r["tier"], column(r))] = r
    return out


def check_comparable(tier: str, runs: dict[str, dict]) -> None:
    """Refuse records whose harness, cameras or tier differ, or Mosaica columns from different
    images."""
    checks = {k: lambda r, k=k: r[k] for k in ("harness_commit", "cameras_sha256", "tier_sha256")}
    checks["Mosaica image"] = lambda r: (r["version"]["image_id"], r["version"]["version"])
    for name, get in checks.items():
        seen = {get(r) for r in runs.values() if name != "Mosaica image" or r["system"].startswith("mosaica")}
        if len(seen) > 1:
            raise SystemExit(f"the {tier} records come from different {name} values {sorted(seen)}; "
                             f"rerun the older systems, or pass only the records of one run")


def ms(s: dict) -> str:
    return f"{s['p50_ms']:.1f} / {s['p99_ms']:.1f}"


def median_max(s: dict) -> str:
    return f"{s['p50_ms']:.1f} / {s['max_ms']:.1f}"


def size(b: int | None) -> str:
    return "–" if b is None else (f"{b / GIB:.2f} GiB" if b >= GIB / 4 else f"{b / (1 << 20):.0f} MiB")


def error_cell(e: dict) -> str:
    wrong = sum(e[k] for k in COUNT_ERRORS)
    pts = sum(e[k] for k in POINT_ERRORS)
    return (f"{e['frames_exact']}/{e['frames']} exact; count err {wrong:,} of {e['matched_true']:,}; "
            f"point err {pts:,}")


def agg_error_cell(e: dict) -> str:
    wrong = e["type_abs_err"] + e["year_abs_err"]
    return (f"{e['tables_exact']}/{e['tables']} exact; count err {wrong:,} of "
            f"{e['type_total_true'] + e['year_total_true']:,}")


def timed_cell(e: dict) -> str:
    return (f"{e['frames_mismatched']:,} of {e['frames']:,} frames, "
            f"{e['tables_mismatched']:,} of {e['tables']:,} tables")


def table(tier: str, runs: dict[str, dict]) -> str:
    systems = sorted(runs)
    dev = any(r.get("development_only", True) for r in runs.values())
    head = f"### {tier}" + (" (development only: not for publication)" if dev else "")
    repeats = sorted({r["repeats"] for r in runs.values()})
    n = "/".join(map(str, repeats))
    # Rows that may be blank: a run with --no-cold has no cold figures.
    optional = {"Cold battery p50 / p99 (ms)", "Serve cgroup memory after a cold restart and one frame"}
    rows = [("Version", lambda r: r["version"]["version"]),
            ("Build time", lambda r: f"{r['build']['build_s']:.1f} s"),
            ("Build peak RSS", lambda r: size(r["build"]["peak_rss_bytes"])),
            ("Build peak cgroup memory (incl. page cache)", lambda r: size(r["build"]["cgroup_peak_bytes"])),
            ("Size on disk (ClickHouse: data directory; Mosaica: bundle)", lambda r: size(r["build"]["disk_bytes"])),
            ("Serve peak RSS", lambda r: size(r["serve_memory"]["peak_rss_bytes"])),
            ("Serve peak cgroup memory (incl. page cache)", lambda r: size(r["serve_memory"]["cgroup_peak_bytes"])),
            ("Serve cgroup memory after a cold restart and one frame",
             lambda r: size(r["serve_memory_after_cold"]["cgroup_peak_bytes"]))]
    for what, key in (("Server start to ready", "server"), ("Session start to first frame", "session")):
        for mode in ("warm", "cold"):
            rows.append((f"{what}, {mode}, median / max of {n} (ms)",
                         lambda r, k=key, m=mode: median_max(r["session_start"][m][k])))
    rows.append(("Cold battery p50 / p99 (ms)", lambda r: ms(r["cold"]["battery"])))
    zooms = sorted({z for r in runs.values() for z in r["warm_battery_by_zoom"]}, key=int)
    for z in zooms:
        rows.append((f"Warm battery, zoom {z}, p50 / p99 (ms)",
                     lambda r, z=z: ms(r["warm_battery_by_zoom"][z])))
    rows += [("Warm camera script p50 / p99 (ms)", lambda r: ms(r["warm"]["script"])),
             ("Warm filter change p50 / p99 (ms)", lambda r: ms(r["warm"]["filters"])),
             ("Warm aggregation p50 / p99 (ms)", lambda r: ms(r["warm"]["aggregation"]))]
    for w in ("battery", "script", "filters"):
        rows.append((f"Error versus oracle, {w}", lambda r, w=w: error_cell(r["errors"][w])))
        rows.append((f"Points sent, {w} (from dense tiles)",
                     lambda r, w=w: f"{r['errors'][w]['points_sent']:,} ({r['errors'][w]['points_dense']:,})"))
        rows.append((f"Bytes received, {w}", lambda r, w=w: size(r["errors"][w]["bytes"])))
    rows += [("Error versus oracle, aggregation", lambda r: agg_error_cell(r["errors"]["aggregation"])),
             ("Timed responses differing from the oracle where the check was right",
              lambda r: timed_cell(r["errors"]["timed"])),
             ("Viewer's visible fraction", lambda r: f"{r['viewer']['visible_rows']:,} of "
              f"{r['viewer']['tier_rows']:,} ({r['viewer']['visible_fraction']:.1%})"),
             ("Access enforced by", lambda r: r["access"]),
             ("Harness commit", lambda r: r["harness_commit"][:12]
              + ("-dirty" if r["harness_commit"].endswith("-dirty") else "")),
             ("Load average before / after", lambda r: f"{r['load_before'][0]:.1f} / {r['load_after'][0]:.1f}")]
    out = [head, "", "| | " + " | ".join(systems) + " |", "|---|" + "---|" * len(systems)]
    for name, f in rows:
        cells = []
        for s in systems:
            try:
                cells.append(str(f(runs[s])))
            except KeyError:
                if name not in optional:
                    raise SystemExit(f"the {s} record has no figure for '{name}'; it was not "
                                     f"written by this harness, or its run failed")
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
