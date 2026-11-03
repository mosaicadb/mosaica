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
            "repeats", "cold_runs", "version", "build", "warm", "errors", "serve_memory")
#: Each system's image, which every record must name and every column of one table must share.
IMAGE = {"mosaica": lambda r: (r["version"]["image_id"], r["version"]["version"]),
         "clickhouse": lambda r: r["version"]["digest"]}


def image_of(r: dict):
    return IMAGE["clickhouse" if r["system"] == "clickhouse" else "mosaica"](r)


def column(r: dict) -> str:
    return r["system"] + (" (default caches)" if r["caches"] == "defaults" else "")


def latest(records: list[dict]) -> dict[tuple[str, str], dict]:
    out = {}
    for r in records:
        missing = [k for k in REQUIRED if k not in r]
        if not missing:
            try:
                image_of(r)
            except KeyError:
                missing = ["version.image_id or version.digest"]
        if missing:
            raise SystemExit(f"a record of {r.get('system', '?')} from {r.get('started_at', '?')} "
                             f"lacks {', '.join(missing)}: it predates this harness; pass only "
                             f"records from one run of it")
        out[(r["tier"], column(r))] = r
    return out


def check_comparable(tier: str, runs: dict[str, dict]) -> None:
    """Refuse records whose harness, cameras or tier differ, or Mosaica columns from different
    images."""
    checks = {k: (lambda r, k=k: r[k], lambda r: True)
              for k in ("harness_commit", "cameras_sha256", "tier_sha256")}
    checks["Mosaica image"] = (image_of, lambda r: r["system"] != "clickhouse")
    checks["ClickHouse image"] = (image_of, lambda r: r["system"] == "clickhouse")
    for name, (get, applies) in checks.items():
        seen = {get(r) for r in runs.values() if applies(r)}
        if len(seen) > 1:
            raise SystemExit(f"the {tier} records come from different {name} values {sorted(seen)}; "
                             f"rerun the older systems, or pass only the records of one run")


def ms(s: dict) -> str:
    """p50 / p99, or median / max where a figure has five samples or fewer."""
    tail = s["max_ms"] if s["n"] <= 5 else s["p99_ms"]
    return f"{s['p50_ms']:.1f} / {tail:.1f}"


def latency(name: str, get, runs: dict[str, dict]) -> tuple:
    """A latency row, labelled by its samples, which the columns must share."""
    ns = set()
    for r in runs.values():
        try:
            ns.add(get(r)["n"])
        except KeyError:
            pass
    n = "/".join(str(v) for v in sorted(ns)) or "0"
    stat = "median / max" if ns and max(ns) <= 5 else "p50 / p99"
    return (f"{name}, {stat} of {n} (ms)", lambda r: ms(get(r)))


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
    lat = lambda name, get: latency(name, get, runs)
    rows = [("Version", lambda r: r["version"]["version"]),
            ("Build time", lambda r: f"{r['build']['build_s']:.1f} s"),
            ("Build peak RSS", lambda r: size(r["build"]["peak_rss_bytes"])),
            ("Build peak cgroup memory (incl. page cache)", lambda r: size(r["build"]["cgroup_peak_bytes"])),
            ("Size on disk (ClickHouse: data directory; Mosaica: bundle)", lambda r: size(r["build"]["disk_bytes"])),
            ("Serve peak RSS", lambda r: size(r["serve_memory"]["peak_rss_bytes"])),
            ("Serve peak cgroup memory (incl. page cache)", lambda r: size(r["serve_memory"]["cgroup_peak_bytes"])),
            ("Serve cgroup memory after a cold restart and one frame",
             lambda r: size(r["serve_memory_after_cold"]["cgroup_peak_bytes"]))]
    # The rows a run with --no-cold has no figure for.
    cold_rows = {rows[-1][0]}
    for what, key in (("Server start to ready", "server"), ("Session start to first frame", "session")):
        for mode in ("warm", "cold"):
            rows.append(lat(f"{what}, {mode}", lambda r, k=key, m=mode: r["session_start"][m][k]))
            cold_rows.add(rows[-1][0])
    rows.append(lat("Cold battery", lambda r: r["cold"]["battery"]))
    cold_rows.add(rows[-1][0])
    zooms = sorted({z for r in runs.values() for z in r["warm_battery_by_zoom"]}, key=int)
    for z in zooms:
        rows.append(lat(f"Warm battery, zoom {z}", lambda r, z=z: r["warm_battery_by_zoom"][z]))
    rows += [lat("Warm camera script", lambda r: r["warm"]["script"]),
             lat("Warm filter change", lambda r: r["warm"]["filters"]),
             lat("Warm aggregation", lambda r: r["warm"]["aggregation"])]
    for w in ("battery", "script", "filters"):
        rows.append((f"Error versus oracle, {w}", lambda r, w=w: error_cell(r["errors"][w])))
        rows.append((f"Points sent, {w} (from dense tiles)",
                     lambda r, w=w: f"{r['errors'][w]['points_sent']:,} ({r['errors'][w]['points_dense']:,})"))
        rows.append((f"Bytes received, {w}", lambda r, w=w: size(r["errors"][w]["bytes"])))
    rows += [("Error versus oracle, aggregation", lambda r: agg_error_cell(r["errors"]["aggregation"])),
             ("Timed responses wrong (against the oracle, or the untimed answer where that was wrong)",
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
                if name not in cold_rows or runs[s]["cold_runs"]:
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
