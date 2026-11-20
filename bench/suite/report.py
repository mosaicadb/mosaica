# The report stage: a markdown summary per query type across datasets, and a plot per type, from results/<suite>/.
# usage: python3 -I report.py SUITE
# Figures: per (dataset, filter, parameter, system, setting), the median over queries of each query's median time
# (ms), and the p90 over the same per-query medians. Exact types carry the check: a cell is exact when no item lies
# outside the rounding allowance (check.py); "~" marks a cell whose differing items all lie within it; "WRONG n"
# counts the items beyond it; "?" marks a cell not checked. Approximate k-NN carries the mean recall.
import csv
import glob
import json
import os
import sys
from collections import defaultdict

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import RESULTS, share_label, summarise  # noqa: E402
import suites as S  # noqa: E402

ROLE_ORDER = {"ours": 0, "standard": 1, "frontier": 2, "database": 3, "in memory": 4}
# one colour per system in every plot (matplotlib's ten-colour cycle), so a system reads the same across panels
SYSTEM_ORDER = ["vexact", "faiss-flat", "faiss-flat-mmap", "faiss-hnsw", "faiss-ivf-disk", "faiss-panorama", "pdx-bond",
                "acorn", "diskann", "pgvector"]
# the recall targets of the scale plots' approximate k-NN: the time each system needs to reach them
RECALL_TARGETS = (0.8, 0.9, 0.95)


def colour(system):
    i = SYSTEM_ORDER.index(system) if system in SYSTEM_ORDER else len(SYSTEM_ORDER) + abs(hash(system)) % 10
    return f"C{i % 10}"


def style(setting):
    """The line style of a system's setting: solid for its default, dashed for any other (e.g. faiss's compacted)."""
    return "-" if col_setting(setting) in ("", "default") else "--"
FILTER_ORDER = {f: i for i, f in enumerate(["none"] + list(S.FILTERS))}


def read_csv(p):
    with open(p) as f:
        return list(csv.DictReader(f))


def param_label(qt, p):
    p = json.loads(p)
    if qt == "threshold":
        return share_label(p["share"])
    if qt == "exact":
        return f"k={p['k']}"
    return f"k={p['k']} " + " ".join(f"{a}={b}" for a, b in p.items() if a != "k")


def ds_key(ds):
    """Datasets in name order, the scale sets by size."""
    from datasets import DATASETS
    d = DATASETS.get(ds, {})
    return (d.get("source") == "laion-scale", d.get("N", 0) if d.get("source") == "laion-scale" else 0, ds)


def param_key(qt, p):
    p = json.loads(p)
    return (p.get("share", 0), p.get("k", 0), tuple(sorted((a, b) for a, b in p.items() if a not in ("k", "share"))))


def load(suite):
    root = os.path.join(RESULTS, suite)
    cells = defaultdict(lambda: dict(ms=[], nonmatch=0, recall=[], returned=[], stats=[]))
    checks, builds = {}, []
    for ddir in sorted(glob.glob(os.path.join(root, "*", ""))):
        ds = os.path.basename(os.path.dirname(ddir))
        for f in glob.glob(os.path.join(ddir, "*.csv")):
            b = os.path.basename(f)
            if b == "scale_diffs.csv":
                for r in read_csv(f):
                    key = (ds, r["system"], "exact", r["filter"], json.dumps({"k": int(r["k"])}, separators=(",", ":")))
                    d = checks.setdefault(("diffs",) + key, dict(items_differing=0, beyond_rounding=0))
                    d["items_differing"] += 1
                    d["beyond_rounding"] += int(r["beyond_rounding"])
            elif b == "check.csv":
                for r in read_csv(f):
                    checks[(ds, r["system"], r["type"], r["filter"], r["setting"], r["param"])] = r
            elif b == "builds.csv":
                builds += [dict(r, dataset=ds) for r in read_csv(f)]
            elif not b.endswith("_setup.csv"):
                for r in read_csv(f):
                    c = cells[(ds, r["type"], r["filter"], r["param"], r["system"], r["setting"], r["role"])]
                    c["ms"].append(float(r["median_ms"]))
                    if r["nonmatch"] != "":
                        c["nonmatch"] += int(r["nonmatch"])
                        c["recall"].append(float(r["recall"]))
                    c["returned"].append(int(r["returned"]))
                    if r.get("stats"):
                        c["stats"].append(json.loads(r["stats"]))
    return cells, checks, builds


def mark(chk, scale=False, cell=None, diffs=None):
    if scale:  # no check at scale: the exact k-NN answers against the streaming truth, the threshold answer sizes
        if cell is None or not cell["recall"]:
            return f" ({int(np.median(cell['returned']))} items)" if cell else ""
        if not cell["nonmatch"]:
            return ""
        if diffs is None:
            return f" ({cell['nonmatch']} differ, not examined)"
        if diffs["beyond_rounding"]:
            return f" WRONG {diffs['beyond_rounding']} of {cell['nonmatch']} differing"
        return " ~"

    if chk is None:
        return " ?"
    if int(chk["beyond_rounding"]):
        return f" WRONG {chk['beyond_rounding']}"
    return " ~" if int(chk["items_differing"]) else ""


# a system's unfiltered run and its main filtered form share a column; other forms (a compacted copy) get their own
MAIN_SETTINGS = ("default", "", "idselector", "selection")


def col_setting(setting):
    return "" if setting in MAIN_SETTINGS else setting


def col_name(system, setting):
    return system if col_setting(setting) == "" else f"{system} ({setting})"


def report(suite):
    cells, checks, builds = load(suite)
    root = os.path.join(RESULTS, suite)
    scale = bool(S.SUITES.get(suite, {}).get("scale"))
    out = [f"# The {suite} suite: results", "",
           "Generated by `report.py` from the CSV files beside it. Times are ms per query, one thread on one pinned "
           "core: the median over queries of each query's median of its repetitions, then `/` the p90 over the same "
           "per-query medians. Under a visible set, faiss filters through an `IDSelectorBitmap` and PDX "
           "through a selection vector; `(compacted)` is faiss over a copy of the visible rows alone. Exactness (exact k-NN, threshold) is from `check.py`: no mark means every answer was "
           "exact; `~` that some items differ but all lie within the rounding allowance; `WRONG n` counts items beyond "
           "it; `?` not checked. Filters: `none` is the whole corpus; `random-p` a random visible set of p% (k-NN and "
           "threshold shares are then of the visible set).", ""]
    for qt, title in (("exact", "Exact k-NN"), ("threshold", "Threshold queries (answer share of the corpus or of "
                                                                "the visible set)")):
        keys = [k for k in cells if k[1] == qt]
        if not keys:
            continue
        cols = sorted({(k[6], k[4], col_setting(k[5])) for k in keys}, key=lambda c: (ROLE_ORDER.get(c[0], 9), c[1], c[2]))
        rows = sorted({(k[0], k[2], k[3]) for k in keys}, key=lambda r: (ds_key(r[0]), FILTER_ORDER.get(r[1], 99),
                                                                          param_key(qt, r[2])))
        out += [f"## {title}", "",
                "| dataset | filter | param | " + " | ".join(f"{col_name(s, st)} [{role}]" for role, s, st in cols)
                + " |", "|---" * (3 + len(cols)) + "|"]
        for ds, f, p in rows:
            line = [ds, f, param_label(qt, p)]
            for role, s, st in cols:
                hit = [k for k in keys if k[:5] == (ds, qt, f, p, s) and k[6] == role and col_setting(k[5]) == st]
                if not hit:
                    line.append("")
                    continue
                c = cells[hit[0]]
                med, p90 = summarise(c["ms"])
                line.append(f"{med:.3g} / {p90:.3g}" + mark(checks.get((ds, s, qt, f, hit[0][5], p)), scale, c,
                                                                 checks.get(("diffs", ds, s, qt, f, p))))
            out.append("| " + " | ".join(line) + " |")
        out.append("")
    keys = [k for k in cells if k[1] == "approx"]
    if keys:
        out += ["## Approximate k-NN" + ("" if scale else " under a visible set"), "",
                ("At these sizes this runner runs approximate k-NN unfiltered too. " if scale else
                 "Unfiltered approximate k-NN is measured by VIBE and ANN-Benchmarks (`public.py`); this runner runs it "
                 "under the visible sets. ") + "Recall is the mean over queries of |answer and true k nearest visible| / k.",
                "", "| dataset | filter | system | setting | median ms | p90 ms | recall |", "|---|---|---|---|---|---|---|"]
        for k in sorted(keys, key=lambda k: (ds_key(k[0]), FILTER_ORDER.get(k[2], 99), ROLE_ORDER.get(k[6], 9), k[4],
                                             param_key("approx", k[3]))):
            c = cells[k]
            med, p90 = summarise(c["ms"])
            out.append(f"| {k[0]} | {k[2]} | {k[4]} [{k[6]}] | {param_label('approx', k[3])} | {med:.3g} | "
                       f"{p90:.3g} | {np.mean(c['recall']):.4f} |")
        out.append("")
    if builds:
        out += ["## Builds", "", "Several threads; recorded apart from the queries.", "",
                "| dataset | system | status | build s | threads | index GiB | peak RSS GiB | detail |",
                "|---|---|---|---|---|---|---|---|"]
        for b in sorted(builds, key=lambda b: (ds_key(b["dataset"]), b["system"])):
            out.append(f"| {b['dataset']} | {b['system']} | {b['status']} | {b.get('build_s', '')} | "
                       f"{b.get('threads', '')} | {b.get('index_gib', '')} | {b.get('peak_rss_gib', '')} | "
                       f"{b.get('detail', '')} |")
        out.append("")
    if scale:
        out[2] = ("Generated by `report.py`. The scale suite measures speed only: times as in the other suites (fewer "
                  "queries and one repetition for the scans); exact k-NN is compared with the streaming truth of "
                  "prep_scale.py, and each item that differs is examined with check.py's rounding allowance "
                  "(`scale_diffs.csv`): `~` all within it, `WRONG n` beyond it; threshold cells give the median "
                  "answer size returned; approximate k-NN the mean recall.")
    if scale:
        rows = []
        for k in sorted(cells, key=lambda k: (ds_key(k[0]), ["exact", "threshold", "approx"].index(k[1]),
                                              FILTER_ORDER.get(k[2], 99), param_key(k[1], k[3]), k[4])):
            st = cells[k]["stats"]
            if not st:
                continue
            med = lambda key: float(np.median([x.get(key, 0) for x in st]))
            extra = (f"{med('reads'):.0f} reads, {med('rounds'):.0f} rounds" if "rounds" in st[0] else
                     f"{med('reads'):.0f} reader reads" if "reads" in st[0] else "")
            rows.append(f"| {k[0]} | {k[1]} | {k[2]} | {param_label(k[1], k[3])} | {col_name(k[4], k[5])} | "
                        f"{med('proc_read_kb'):.0f} | {med('majflt'):.0f} | {extra} |")
        if rows:
            out += ["## Storage reads per query", "",
                    "Medians over the queries of each query's last repetition: KiB the system's processes read from "
                    "storage (/proc), major page faults, and the system's own counters (vexact's io_uring reader reads; "
                    "DiskANN's QueryStats reads and rounds). Zero where the data sat in the page cache.", "",
                    "| dataset | type | filter | param | system | KiB read | major faults | own counters |",
                    "|---|---|---|---|---|---|---|---|"] + rows + [""]
    m = machine_record(root)
    head = ["", f"**Machine** (`machine.json`, recorded at the last run): {m}", ""] if m else []
    if scale:
        head += regime_section(builds, root)
    out[3:3] = head
    out += caps_section(suite)
    with open(os.path.join(root, "REPORT.md"), "w") as f:
        f.write("\n".join(out))
    print(f"wrote {os.path.join(root, 'REPORT.md')}")
    if scale:
        for f in ("exact.png", "threshold.png", "approx.png"):
            if os.path.exists(os.path.join(root, f)):
                os.remove(os.path.join(root, f))
        scale_plots(cells, builds, root)
    else:
        plots(cells, root)


def machine_record(root):
    p = os.path.join(root, "machine.json")
    if not os.path.exists(p):
        return ""
    import machine
    return machine.summary(json.load(open(p)))


def regime(gib, ram):
    """fits (at most half the RAM), partly fits (up to the RAM), exceeds RAM."""
    if not ram:
        return "?"
    return "fits" if gib <= 0.5 * ram else "partly fits" if gib <= ram else "exceeds RAM"


def regime_section(builds, root):
    """At scale the machine's RAM is the cap: each size's f16 vectors and each system's footprint against it."""
    p = os.path.join(root, "machine.json")
    ram = json.load(open(p))["ram_gib"] if os.path.exists(p) else 0
    from datasets import DATASETS
    dss = sorted({b["dataset"] for b in builds}, key=ds_key)
    systems = sorted({b["system"] for b in builds})
    out = ["## Regimes", "",
           f"The machine's RAM ({ram} GiB) is the cap. Each size's f16 vectors (N x d x 2 bytes) and each system's "
           "footprint (its index on disk, or for an in-memory system what it holds in memory; and the build's peak RSS) "
           "are set against it: `fits` at most half the RAM, `partly fits` up to the RAM, `exceeds RAM` beyond. A "
           "system the OOM killer stopped is `does not fit`.", "",
           "| dataset | f16 vectors GiB | " + " | ".join(f"{s}: index GiB / build peak GiB" for s in systems) + " |",
           "|---" * (2 + len(systems)) + "|"]
    for ds in dss:
        d = DATASETS.get(ds, {})
        f16 = d.get("N", 0) * d.get("d", 0) * 2 / 2 ** 30
        line = [ds, f"{f16:.2f} ({regime(f16, ram)})"]
        for sname in systems:
            b = next((b for b in builds if b["dataset"] == ds and b["system"] == sname), None)
            if b is None:
                line.append("")
            elif b["status"] != "ok":
                line.append(b["status"] + (" (disk)" if "on disk" in b.get("detail", "") else
                                           " (OOM)" if "OOM" in b.get("detail", "") else ""))
            else:
                ix, pk = float(b["index_gib"] or 0), float(b["peak_rss_gib"] or 0)
                line.append(f"{ix:.2f} ({regime(ix, ram)}) / {pk:.2f} ({regime(pk, ram)})")
        out.append("| " + " | ".join(line) + " |")
    return out + [""]


def caps_section(suite):
    """The memory-cap axis: per dataset, warm median / cold ms per cell and cap, what did not fit, and the evidence."""
    root = os.path.join(RESULTS, suite)
    out = []
    dss = sorted(d for d in os.listdir(root) if os.path.isdir(os.path.join(root, d, "caps"))) if os.path.isdir(root) else []
    if not dss:
        return out
    caps_cfg = S.SUITES.get(suite, {}).get("caps", {})
    out += ["## Under a memory cap", "",
            "Caps are fractions of the dataset's size, N x d x 4 bytes (its f32 vectors as distributed), for the whole "
            "query process (and a server it uses), page cache included, with no swap (capmem.py). Each system builds "
            "uncapped, then each cap runs in a fresh process: the system's files are dropped from the page cache, the "
            "limit is set, and each cell runs one cold query (query 0 once, after the full vectors, or for the other "
            "systems the group's page cache, are dropped; codes, graph and PQ stay) and then the warm repetitions. "
            "Cells are `warm median / cold` ms, with the mean recall for approximate k-NN. `does not fit`: the kernel "
            "refused the limit, or the OOM killer stopped the process under it. An in-memory system named on the "
            "axis is built inside each capped process and the cap set after, so it does not fit wherever what it "
            "holds exceeds the cap. Storage reads per query and the peak memory are in the tables below.", ""]
    for ds in dss:
        cdir = os.path.join(root, ds, "caps")
        status = {(r["system"], r["cap"]): r for r in read_csv(os.path.join(cdir, "status.csv"))} \
            if os.path.exists(os.path.join(cdir, "status.csv")) else {}
        caps = sorted({c for _, c in status}, key=lambda c: (c != "uncapped", -float(c) if c != "uncapped" else 0))
        cells = defaultdict(dict)
        for f in sorted(glob.glob(os.path.join(cdir, "*.cap-*.csv"))):
            if f.endswith(".mem.csv") or f.endswith("_setup.csv"):
                continue
            for r in read_csv(f):
                k = (r["type"], r["filter"], r["param"], r["system"], r["role"])
                c = cells[k].setdefault(r["cap"], dict(ms=[], cold=None, rec=[], stats=[]))
                if r["kind"] == "cold":
                    c["cold"] = float(r["median_ms"])
                else:
                    c["ms"].append(float(r["median_ms"]))
                    if r["recall"] != "":
                        c["rec"].append(float(r["recall"]))
                    if r.get("stats"):
                        c["stats"].append(dict(json.loads(r["stats"]), ms=float(r["median_ms"])))
        out += [f"### {ds}", "", "| type | filter | param | system | " + " | ".join(
            "uncapped" if c == "uncapped" else f"{c}%" for c in caps) + " |", "|---" * (4 + len(caps)) + "|"]
        for k in sorted(cells, key=lambda k: (["exact", "threshold", "approx"].index(k[0]), FILTER_ORDER.get(k[1], 99),
                                              param_key(k[0], k[2]), ROLE_ORDER.get(k[4], 9), k[3])):
            line = [k[0], k[1], param_label(k[0], k[2]), f"{k[3]} [{k[4]}]"]
            for cap in caps:
                st = status.get((k[3], cap))
                c = cells[k].get(cap)
                if c and c["ms"]:
                    v = f"{np.median(c['ms']):.3g} / {c['cold']:.3g}" if c["cold"] is not None else f"{np.median(c['ms']):.3g}"
                    if k[0] == "approx" and c["rec"]:
                        v += f" r {np.mean(c['rec']):.3f}"
                    if st and st["status"] == "does not fit":
                        v += " †"
                    line.append(v)
                elif st and st["status"] == "does not fit":
                    line.append("does not fit")
                else:
                    line.append("")
            out.append("| " + " | ".join(line) + " |")
        if any(st["status"] == "does not fit" for st in status.values()):
            out += ["", "† measured before the OOM killer stopped that run, later in the same cap."]
        io_rows = []
        for k in sorted(cells, key=lambda k: (["exact", "threshold", "approx"].index(k[0]), FILTER_ORDER.get(k[1], 99),
                                              param_key(k[0], k[2]), ROLE_ORDER.get(k[4], 9), k[3])):
            line = [k[0], k[1], param_label(k[0], k[2]), k[3]]
            any_st = False
            for cap in caps:
                c = cells[k].get(cap)
                st = c["stats"] if c else []
                if not st:
                    line.append("")
                    continue
                any_st = True
                med = lambda key: float(np.median([x.get(key, 0) for x in st]))
                if "rounds" in st[0]:  # DiskANN: its own counters
                    v = f"{med('reads'):.0f} reads, {med('rounds'):.0f} rounds, {med('proc_read_kb'):.0f} KiB"
                else:
                    v = (f"{med('reads'):.0f} reads, " if "reads" in st[0] else "") + \
                        f"{med('majflt'):.0f} faults, {med('proc_read_kb'):.0f} KiB"
                line.append(v)
            if any_st:
                io_rows.append("| " + " | ".join(line) + " |")
        if io_rows:
            out += ["", "Storage reads per query (medians over the warm queries, from the last repetition): DiskANN's "
                    "own QueryStats (reads, rounds, bytes), vexact's reader (io_uring reads), major page faults, and the "
                    "bytes the system's processes read from storage (/proc).", "",
                    "| type | filter | param | system | " + " | ".join(
                        "uncapped" if c == "uncapped" else f"{c}%" for c in caps) + " |", "|---" * (4 + len(caps)) + "|"]
            out += io_rows
        bf = os.path.join(cdir, "builds.csv")
        if os.path.exists(bf):
            out += ["", "Builds for the cap axis (uncapped, several threads):", "",
                    "| system | status | build s | threads | index GiB | peak RSS GiB | detail |", "|---|---|---|---|---|---|---|"]
            for b in read_csv(bf):
                out.append(f"| {b['system']} | {b['status']} | {b['build_s']} | {b['threads']} | {b['index_gib']} | "
                           f"{b['peak_rss_gib']} | {b['detail']} |")
        out += ["", "Evidence (after the last cell of each run): the cgroup's usage, its anonymous and file memory, "
                "its peak since the cap was set, the limit, the times the limit was hit (v1 failcnt; v2 max events), "
                "OOM kills and the bytes the process read from storage over its cells.", "",
                "| system | cap | status | usage MiB | anon MiB | file MiB | peak MiB | limit MiB | failcnt | OOM | read MiB | detail |",
                "|---|---|---|---|---|---|---|---|---|---|---|---|"]
        for (sname, cap), st in sorted(status.items()):
            mem = os.path.join(cdir, f"{sname}.cap-{cap}.mem.csv")
            rows = read_csv(mem) if os.path.exists(mem) else []
            last = rows[-1] if rows else {}
            rd = sum(float(r["read_mib"] or 0) for r in rows)
            det = st["detail"] if st["status"] != "ok" else ""
            out.append(f"| {sname} | {cap} | {st['status']} | {last.get('usage_mib', '')} | {last.get('anon_mib', '')} | "
                       f"{last.get('file_mib', '')} | {last.get('peak_mib', '')} | {last.get('limit_mib', '')} | "
                       f"{last.get('failcnt', '')} | {last.get('oom_kill', '')} | {rd:.0f} | {det} |")
        out.append("")
    return out


def plots(cells, root):
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
    except ImportError:
        print("matplotlib is not installed: no plots")
        return
    for qt in ("exact", "threshold", "approx"):
        keys = [k for k in cells if k[1] == qt]
        if not keys:
            continue
        dss = sorted({k[0] for k in keys}, key=ds_key)
        fig, axes = plt.subplots(len(dss), 1, figsize=(10, 3.2 * len(dss)), squeeze=False)
        for ax, ds in zip(axes[:, 0], dss):
            kk = [k for k in keys if k[0] == ds]
            if qt == "approx":
                for (f, s, st) in sorted({(k[2], k[4], k[5]) for k in kk}):
                    pts = sorted((np.mean(cells[k]["recall"]), summarise(cells[k]["ms"])[0])
                                 for k in kk if (k[2], k[4], k[5]) == (f, s, st))
                    ax.plot([p[0] for p in pts], [p[1] for p in pts], marker="o", label=f"{col_name(s, st)}, {f}",
                            color=colour(s), linestyle=FILTER_LINES[FILTER_ORDER.get(f, 0) % len(FILTER_LINES)])
                ax.set_xlabel("recall")
            else:
                xs = sorted({(k[2], k[3]) for k in kk}, key=lambda x: (FILTER_ORDER.get(x[0], 99), param_key(qt, x[1])))
                series = sorted({(k[4], col_setting(k[5])) for k in kk})
                for j, (s, st) in enumerate(series):
                    pts = [(i + (j - (len(series) - 1) / 2) * 0.06, summarise(cells[k]["ms"])[0])
                           for i, x in enumerate(xs) for k in kk
                           if (k[2], k[3], k[4], col_setting(k[5])) == (x[0], x[1], s, st)]
                    ax.plot([p[0] for p in pts], [p[1] for p in pts], marker="o" if style(st) == "-" else "s",
                            linestyle="", label=col_name(s, st), color=colour(s))
                ax.set_xticks(range(len(xs)))
                ax.set_xticklabels([f"{f}\n{param_label(qt, p)}" for f, p in xs], fontsize=8)
            ax.set_yscale("log")
            ax.set_ylabel("median ms / query")
            ax.set_title(ds, fontsize=10)
            ax.grid(alpha=0.3)
            ax.legend(fontsize=7, loc="best")
        fig.tight_layout()
        fig.savefig(os.path.join(root, f"{qt}.png"), dpi=110)
        plt.close(fig)


FILTER_LINES = ["-", "--", ":", "-."]


def time_at_recall(pts, target):
    """The time a system needs for mean recall `target`, from its (recall, ms) points: on the points that are not
    dominated (more recall for more time), interpolated in log time between the last point below the target and the
    first at or above it; the first point's time when that already reaches it; None when no point does."""
    front, best = [], -1.0
    for r, ms in sorted(pts, key=lambda p: (p[1], -p[0])):
        if r > best:
            front.append((r, ms))
            best = r
    for i, (r, ms) in enumerate(front):
        if r >= target:
            if i == 0:
                return ms
            r0, m0 = front[i - 1]
            t = (target - r0) / (r - r0)
            return float(np.exp(np.log(m0) + t * (np.log(ms) - np.log(m0))))
    return None


def n_label(n):
    """The nominal size: 200,090 -> 200k, 1,000,448 -> 1M."""
    n = float(f"{n:.2g}")
    return f"{n / 1e6:g}M" if n >= 1e6 else f"{n / 1e3:g}k"


def scale_plots(cells, builds, root):
    """At scale: how each system's cost grows with N, one line per system across the sizes. Exact k-NN and threshold
    queries plot the median time per query; approximate k-NN the time to reach each recall target
    (`time_at_recall`); the build its time, index size and peak memory. Panels: rows are filters, columns the
    parameter (k, share, recall target)."""
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
    except ImportError:
        print("matplotlib is not installed: no plots")
        return
    from datasets import DATASETS
    N = lambda ds: DATASETS.get(ds, {}).get("N", 0)
    dss = sorted({k[0] for k in cells}, key=ds_key)
    ns = [N(d) for d in dss]

    def frame(ax, title, ylabel):
        ax.set_xscale("log")
        ax.set_yscale("log")
        ax.set_xticks(ns)
        ax.set_xticklabels([n_label(n) for n in ns])
        ax.minorticks_off()
        ax.set_xlabel("vectors (N)")
        ax.set_ylabel(ylabel)
        ax.set_title(title, fontsize=10)
        ax.grid(alpha=0.3)

    def save(fig, axes, name):
        handles = {}
        for ax in axes.flat:
            for h, l in zip(*ax.get_legend_handles_labels()):
                handles.setdefault(l, h)
        if handles:
            fig.legend(list(handles.values()), list(handles.keys()), loc="lower center", ncol=min(5, len(handles)),
                       fontsize=8, frameon=False)
        fig.tight_layout(rect=(0, 0.04 + 0.025 * ((len(handles) - 1) // 5), 1, 1))
        fig.savefig(os.path.join(root, name), dpi=110)
        plt.close(fig)

    for qt in ("exact", "threshold", "approx"):
        keys = [k for k in cells if k[1] == qt]
        if not keys:
            continue
        filters = sorted({k[2] for k in keys}, key=lambda f: FILTER_ORDER.get(f, 99))
        if qt == "approx":
            rows = [[(f, c) for c in RECALL_TARGETS] for f in filters]
        else:
            # only the (filter, parameter) cells that were run: the filtered rows have fewer parameters
            rows = [[(f, c) for c in sorted({k[3] for k in keys if k[2] == f}, key=lambda p: param_key(qt, p))]
                    for f in filters]
        ncol = max(len(r) for r in rows)
        fig, axes = plt.subplots(len(rows), ncol, figsize=(4.2 * ncol, 3.4 * len(rows)), squeeze=False)
        for i, row in enumerate(rows):
            for j in range(len(row), ncol):
                axes[i, j].axis("off")
            for j, (f, c) in enumerate(row):
                ax = axes[i, j]
                series = sorted({(k[4], k[5]) for k in keys if k[2] == f},
                                key=lambda x: (SYSTEM_ORDER.index(x[0]) if x[0] in SYSTEM_ORDER else 99, x[1]))
                for s, st in series:
                    xs, ys = [], []
                    for ds in dss:
                        if qt == "approx":
                            pts = [(np.mean(cells[k]["recall"]), summarise(cells[k]["ms"])[0]) for k in keys
                                   if k[0] == ds and k[2] == f and (k[4], k[5]) == (s, st) and cells[k]["recall"]]
                            y = time_at_recall(pts, c) if pts else None
                        else:
                            kk = [k for k in keys if k[0] == ds and k[2] == f and k[3] == c and (k[4], k[5]) == (s, st)]
                            y = summarise(cells[kk[0]]["ms"])[0] if kk else None
                        if y is not None:
                            xs.append(N(ds))
                            ys.append(y)
                    if xs:
                        ax.plot(xs, ys, marker="o", color=colour(s), linestyle=style(st), label=col_name(s, st))
                what = (f"recall {c:g}" if qt == "approx" else param_label(qt, c))
                frame(ax, f"{f}, {what}", "ms / query" + (" to reach the recall" if qt == "approx" else ""))
        title = {"exact": "Exact k-NN", "threshold": "Threshold queries (answer share)",
                 "approx": "Approximate k-NN: time to reach a recall (absent: not reached)"}[qt]
        fig.suptitle(title, fontsize=11)
        save(fig, axes, f"scale_{qt}.png")

    ok = [b for b in builds if b["status"] == "ok"]
    if ok:
        fig, axes = plt.subplots(1, 3, figsize=(13, 3.6), squeeze=False)
        systems = sorted({b["system"] for b in ok}, key=lambda x: SYSTEM_ORDER.index(x) if x in SYSTEM_ORDER else 99)
        for ax, (field, ylabel) in zip(axes[0], (("build_s", "build s"), ("index_gib", "index GiB"),
                                                 ("peak_rss_gib", "build peak RSS GiB"))):
            for sname in systems:
                pts = sorted((N(b["dataset"]), float(b[field])) for b in ok if b["system"] == sname and b[field])
                if pts:
                    ax.plot([p[0] for p in pts], [p[1] for p in pts], marker="o", color=colour(sname), label=sname)
            frame(ax, ylabel, ylabel)
        fig.suptitle("Builds (a system missing at a size did not fit or was not run)", fontsize=11)
        save(fig, axes, "scale_build.png")


if __name__ == "__main__":
    report(sys.argv[1])
