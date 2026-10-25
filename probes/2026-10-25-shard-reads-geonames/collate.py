"""Each figure of run.sh's results as its ratio to one shard.

    python3 collate.py <out dir> <corpus> [<shard count> ...]

The interactive bench's figures are its medians, by measurement name, and the median of its
`/v1/aggregate` requests' times. The battery's are, per
principal, condition and zoom, the median over its cells of each cell's median server time; then
the text searches. A shard count without a battery.json shows only the bench's figures, and "–"
in the battery's rows. A second table gives the build's and the server's figures as measured.
"""

import json
import re
import statistics
import sys
from pathlib import Path


def bench(path: Path) -> dict[str, float]:
    b = json.loads(path.read_text())
    out = {name: m["median"] for name, m in b["measurements"].items() if m.get("median") is not None}
    # The bench names no figure for its `/v1/aggregate` requests; their median is taken here.
    aggregate = [r["last_byte_ms"] for r in b["requests"] if r["path"] == "/v1/aggregate"]
    if aggregate:
        out["requests.aggregate.last_byte"] = statistics.median(aggregate)
    return out


def walk_conditions(prefix: str, node, out: dict) -> None:
    """Every `conditions` block under `node`, keyed by the path to it."""
    if isinstance(node, dict):
        if "conditions" in node and isinstance(node["conditions"], dict):
            for cond, c in node["conditions"].items():
                p50 = c.get("server_ms", {}).get("p50") if isinstance(c, dict) else None
                if p50 is not None:
                    out[f"{prefix}.{cond}"] = p50
        for k, v in node.items():
            if k != "conditions":
                walk_conditions(f"{prefix}.{k}" if prefix else k, v, out)


def battery(path: Path) -> dict[str, float]:
    if not path.exists():
        return {}
    b = json.loads(path.read_text())
    out: dict[str, float] = {}
    for rung in b["ladder"]:
        cells: dict[str, list[float]] = {}
        for cell in rung["cells"]:
            for cond, c in cell["conditions"].items():
                p50 = c["server_ms"]["p50"]
                if p50 is not None:
                    cells.setdefault(f"z{cell['zoom']:02d}.{cond}", []).append(p50)
        for key, values in cells.items():
            out[f"viewport.{rung['target']:.0%}.{key}"] = statistics.median(values)
        out[f"first_viewport.{rung['target']:.0%}"] = rung["first_viewport_server_ms"]
    walk_conditions("text", b["text_and_drilldown"], out)
    return out


def time_v(path: Path) -> dict[str, float]:
    """Wall seconds and peak resident GiB from `/usr/bin/time -v`."""
    if not path.exists():
        return {}
    text = path.read_text()
    out = {}
    wall = re.search(r"Elapsed \(wall clock\) time \(h:mm:ss or m:ss\): (\S+)", text)
    if wall:
        secs = 0.0
        for part in wall.group(1).split(":"):
            secs = secs * 60 + float(part)
        out["build.wall_s"] = secs
    rss = re.search(r"Maximum resident set size \(kbytes\): (\d+)", text)
    if rss:
        out["build.peak_rss_gib"] = int(rss.group(1)) / 2**20
    return out


def setting(dir_: Path) -> dict[str, float]:
    """The bundle's bytes, the build's figures, and the bench's server figures."""
    out = time_v(dir_ / "build-time.txt")
    bytes_ = dir_ / "bundle-bytes.txt"
    if bytes_.exists():
        out["bundle.gb"] = int(bytes_.read_text().split()[0]) / 1e9
    b = json.loads((dir_ / "bench.json").read_text())
    server = b["server"]
    out["server.readyz_s"] = server["open_s"]
    out["server.readyz_again_s"] = server["reopen_s"]
    out["server.anon_at_ready_gib"] = server["memory_at_ready"]["rssanon_gib"]
    out["server.anon_after_run_gib"] = server["memory_after"]["rssanon_gib"]
    out["server.vmhwm_after_run_gib"] = server["memory_after"]["vmhwm_gib"]
    out["figures.walks"] = b["status"]["run"]["fills"]
    out["figures.disk_mb"] = b["status"]["run"]["disk_bytes"] / 1e6
    return out


def main() -> None:
    root, corpus = Path(sys.argv[1]), sys.argv[2]
    counts = [int(n) for n in sys.argv[3:]] or sorted(
        int(p.name.rsplit("-", 1)[1]) for p in root.glob(f"{corpus}-*") if (p / "bench.json").exists()
    )
    figures = {
        n: bench(root / f"{corpus}-{n}/bench.json") | battery(root / f"{corpus}-{n}/battery.json")
        for n in counts
    }
    base = figures[counts[0]]
    print("| figure | " + " | ".join(f"N={n}" for n in counts) + " |")
    print("|---|" + "---:|" * len(counts))
    for name in sorted(base):
        cells = [f"{base[name]:.1f} ms"]
        for n in counts[1:]:
            v = figures[n].get(name)
            cells.append("–" if v is None or not base[name] else f"{v / base[name]:.2f}")
        print(f"| {name} | " + " | ".join(cells) + " |")

    settings = {n: setting(root / f"{corpus}-{n}") for n in counts}
    print()
    print("| figure | " + " | ".join(f"N={n}" for n in counts) + " |")
    print("|---|" + "---:|" * len(counts))
    for name in settings[counts[0]]:
        cells = [
            "–" if settings[n].get(name) is None else f"{settings[n][name]:,.2f}".rstrip("0").rstrip(".")
            for n in counts
        ]
        print(f"| {name} | " + " | ".join(cells) + " |")


if __name__ == "__main__":
    main()
