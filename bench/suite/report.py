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
# the systems' order in the plots (their colours are plotting.COLOURS)
SYSTEM_ORDER = ["vexact", "faiss-flat", "faiss-flat-mmap", "faiss-hnsw", "faiss-ivf-disk", "faiss-panorama", "pdx-bond",
                "acorn", "diskann", "pgvector"]
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
        scale_plots(cells, builds, root)
    else:
        plots(cells, root)
    recall_qps(cells, root)


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


APPROX_TARGET = 0.95  # the quick and full suites' plots: approximate methods at their cheapest setting reaching it
SCALE_TARGETS = (0.9, 0.95)  # the scale plots': one column each


def _vis_share(f):
    """The visible share (%) of a filter: 100 for none, the random set's share; None for other kinds."""
    if f == "none":
        return 100.0
    d = S.FILTERS.get(f, {})
    return 100.0 * d["share"] if d.get("kind") == "random" else None


def _series(cells, qt, select, xof, target=None):
    """Lines from the cells of one query type: `select(key)` picks the cells, `xof(key)` gives a cell's x. Exact
    types: one point per cell. Approximate k-NN: per x, the cheapest setting reaching `target` (else the most
    accurate one, hollow)."""
    import plotting as P
    groups = defaultdict(lambda: defaultdict(list))
    for k in cells:
        if k[1] != qt or not select(k):
            continue
        x = xof(k)
        if x is None:
            continue
        ms = summarise(cells[k]["ms"])[0]
        rec = float(np.mean(cells[k]["recall"])) if cells[k]["recall"] else 1.0
        # by the setting as shown: a system's main settings (none, idselector, selection) are one line
        groups[(k[4], col_setting(k[5]))][x].append((ms, rec))
    out = []
    for (system, setting), byx in sorted(groups.items(), key=lambda g: (SYSTEM_ORDER.index(g[0][0])
                                                                       if g[0][0] in SYSTEM_ORDER else 99, g[0][1])):
        if qt == "approx":
            pts = [(x,) + P.cheapest(v, target) for x, v in byx.items()]
            pts = [(x, ms, r, ok) for x, ms, r, ok in pts]
        else:
            pts = [(x, v[0][0], None, True) for x, v in byx.items()]
        out.append(dict(name=P.name(system, setting, approx=qt == "approx"), colour=P.colour(system),
                        exact=qt != "approx", pts=pts))
    return out


def _protocol(suite):
    d = S.SUITES.get(suite, {})
    return f"median over {d.get('nq', '?')} queries of each query's median of {d.get('reps', '?')} repetitions"


def plots(cells, root):
    """One figure per dataset: k-NN at k = 10 against the visible share, threshold queries at an answer of 1% of the
    visible items against the visible share, and threshold queries against the answer size with every item visible."""
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
    except ImportError:
        print("matplotlib is not installed: no plots")
        return
    import plotting as P
    from datasets import DATASETS
    P.setup(plt)
    suite = os.path.basename(root)
    for f in ("exact.png", "threshold.png", "approx.png"):
        if os.path.exists(os.path.join(root, f)):
            os.remove(os.path.join(root, f))
    k10 = json.dumps({"k": 10}, separators=(",", ":"))
    for ds in sorted({k[0] for k in cells}, key=ds_key):
        vis_ticks = sorted({v for k in cells if k[0] == ds for v in [_vis_share(k[2])] if v is not None})
        knn = (_series(cells, "exact", lambda k: k[0] == ds and k[3] == k10, lambda k: _vis_share(k[2]))
               + _series(cells, "approx", lambda k: k[0] == ds and json.loads(k[3]).get("k") == 10,
                         lambda k: _vis_share(k[2]), APPROX_TARGET))
        thr_vis = _series(cells, "threshold", lambda k: k[0] == ds and json.loads(k[3])["share"] == 0.01,
                          lambda k: _vis_share(k[2]))
        thr_all = _series(cells, "threshold", lambda k: k[0] == ds and k[2] == "none",
                          lambda k: 100.0 * json.loads(k[3])["share"])
        panels = [(s, xl, t, ticks) for s, xl, t, ticks in (
            (knn, "items visible (%, at random)", "k-NN, k = 10", vis_ticks),
            (thr_vis, "items visible (%, at random)", "Threshold, answer 1% of the visible items", vis_ticks),
            (thr_all, "answer size, as a share of the items", "Threshold, all items visible", None))
            if any(len(x["pts"]) > 0 for x in s)]
        if not panels:
            continue
        fig, axes = P.figure(plt, 1, len(panels))
        for ax, (series, xlabel, title, ticks) in zip(axes[0], panels):
            if ticks is None:
                ticks = sorted({p[0] for x in series for p in x["pts"]})
            P.draw(ax, series, xlabel, title, ticks, [f"{t:g}%" for t in ticks])
        d = DATASETS.get(ds, {})
        size = f" ({d['N'] / 1e6:.2g}M vectors, {d['d']} d)" if d.get("N") and d.get("d") else ""
        P.finish(fig, plt, os.path.join(root, f"{ds}.png"), f"{ds}{size}, 1 thread",
                 f"Median time per query ({_protocol(suite)}). Solid lines are exact. Dashed lines are approximate "
                 f"methods at their cheapest setting\nwith mean recall ≥ {APPROX_TARGET:g}; hollow points did not reach "
                 "it in the settings run and show the recall reached. A system missing at a point was not run there.")


def recall_qps(cells, root):
    """The ANN-Benchmarks view: per (dataset, filter), recall against queries per second at k = 10, every system
    overlaid. Approximate systems are a line through their Pareto frontier over their settings; exact systems one
    point each at their recall (1, or just under where answers differ within the rounding allowance). QPS is one
    thread's 1 / mean query time (the mean over queries of each query's median of its repetitions)."""
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
    except ImportError:
        return
    import plotting as P
    from datasets import DATASETS
    P.setup(plt)
    pts = defaultdict(list)  # (ds, filter, system, setting, approx) -> [(recall, qps)]
    for k, c in cells.items():
        if k[1] not in ("exact", "approx") or json.loads(k[3]).get("k") != 10 or not c["ms"]:
            continue
        rec = float(np.mean(c["recall"])) if c["recall"] else 1.0
        pts[(k[0], k[2], k[4], col_setting(k[5]), k[1] == "approx")].append((rec, 1000.0 / float(np.mean(c["ms"]))))
    if not any(key[4] for key in pts):
        return
    dss = sorted({key[0] for key in pts}, key=ds_key)
    filters = sorted({key[1] for key in pts}, key=lambda f: FILTER_ORDER.get(f, 99))
    fig, axes = plt.subplots(len(dss), len(filters), figsize=(4.3 * len(filters), 3.5 * len(dss) + 1.6),
                             squeeze=False)
    handles = {}
    for i, ds in enumerate(dss):
        for j, f in enumerate(filters):
            ax = axes[i, j]
            keys = sorted((key for key in pts if key[0] == ds and key[1] == f),
                          key=lambda key: (not key[4], SYSTEM_ORDER.index(key[2]) if key[2] in SYSTEM_ORDER else 99,
                                           key[3]))
            if not keys:
                ax.axis("off")
                continue
            for key in keys:
                _, _, system, setting, approx = key
                label = P.name(system, setting, approx=approx)
                v = sorted(pts[key], key=lambda p: (-p[0], -p[1]))
                # the Pareto frontier: from the most accurate setting down, keep a point only if it is faster
                front, best = [], -1.0
                for r, q in v:
                    if q > best:
                        front.append((r, q))
                        best = q
                front.sort()
                h, = ax.plot([p[0] for p in front], [p[1] for p in front], linestyle="--" if approx else "none",
                             marker="o" if approx else ("s" if setting else "D"), ms=5 if approx else 7,
                             color=P.colour(system), mfc=P.colour(system) if approx else "white", mew=1.6, lw=1.8)
                handles.setdefault(label, h)
            ax.set_yscale("log")
            ax.yaxis.set_major_locator(matplotlib.ticker.LogLocator(subs=(1.0, 2.0, 5.0)))
            ax.yaxis.set_major_formatter(matplotlib.ticker.FuncFormatter(lambda v, _: f"{v:g}"))
            ax.yaxis.set_minor_formatter(matplotlib.ticker.NullFormatter())
            rmin = min(p[0] for key in keys for p in pts[key])
            ax.set_xlim(max(0.0, rmin - 0.03), 1.008)
            ax.grid(True, color=P.GRID, lw=0.8)
            ax.tick_params(length=0)
            vis = "all items visible" if f == "none" else (f"{_vis_share(f):g}% visible at random" if _vis_share(f)
                                                            else f)
            d = DATASETS.get(ds, {})
            size = f" ({d['N'] / 1e6:.2g}M)" if d.get("N") else ""
            ax.set_title(f"{ds}{size}\n{vis}", loc="left", fontsize=9.5, color=P.INK)
            ax.set_xlabel("recall@10")
            ax.set_ylabel("queries per second")
    fig.legend(list(handles.values()), list(handles.keys()), loc="lower center", ncol=min(6, len(handles)),
               fontsize=8.5, frameon=False)
    nleg = (len(handles) - 1) // 6 + 1
    h = fig.get_size_inches()[1]
    fig.subplots_adjust(top=1 - 1.45 / h, bottom=(0.55 + 0.25 * nleg) / h + 0.6 / h, hspace=0.7, wspace=0.3,
                        left=0.07, right=0.98)
    fig.text(0.07, 1 - 0.25 / h, "Recall against queries per second, k-NN at k = 10, 1 thread", ha="left", va="top",
             fontsize=12.5, color=P.INK, fontweight="bold")
    import textwrap
    sub = ("Up and to the right is better. Approximate methods (dashed) through the Pareto frontier of their settings; "
           "exact methods (open diamonds; squares for a setting such as faiss's compacted copy) at their recall, for "
           "reference. QPS is 1 / mean query time on one thread. A panel without a dashed line had no approximate run "
           "under that filter.")
    fig.text(0.07, 1 - 0.6 / h, "\n".join(textwrap.wrap(sub, int(fig.get_size_inches()[0] * 14))), ha="left",
             va="top", fontsize=8.8, color=P.MUTED)
    fig.savefig(os.path.join(root, "recall_qps.png"), dpi=130, facecolor="white")
    plt.close(fig)


def n_label(n):
    """The nominal size: 200,090 -> 200k, 1,000,448 -> 1M."""
    n = float(f"{n:.2g}")
    return f"{n / 1e6:g}M" if n >= 1e6 else f"{n / 1e3:g}k"


def scale_plots(cells, builds, root):
    """At scale: how each system's time grows with N. One row per filter; columns: k-NN at k = 10 with approximate
    methods at their cheapest setting reaching each recall target, and threshold queries. A second figure for the
    builds: time, index size and peak memory."""
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
    except ImportError:
        print("matplotlib is not installed: no plots")
        return
    import plotting as P
    from datasets import DATASETS
    P.setup(plt)
    suite = os.path.basename(root)
    for f in ("exact.png", "threshold.png", "approx.png", "scale_exact.png", "scale_threshold.png",
              "scale_approx.png"):
        if os.path.exists(os.path.join(root, f)):
            os.remove(os.path.join(root, f))
    N = lambda ds: DATASETS.get(ds, {}).get("N", 0)
    ns = sorted({N(k[0]) for k in cells})
    ticks = (ns, [n_label(n) for n in ns])
    k10 = json.dumps({"k": 10}, separators=(",", ":"))
    filters = sorted({k[2] for k in cells}, key=lambda f: FILTER_ORDER.get(f, 99))
    rows = []
    for f in filters:
        row = []
        for t in SCALE_TARGETS:
            series = (_series(cells, "exact", lambda k: k[2] == f and k[3] == k10, lambda k: N(k[0]))
                      + _series(cells, "approx", lambda k: k[2] == f and json.loads(k[3]).get("k") == 10,
                                lambda k: N(k[0]), t))
            row.append((series, f"k-NN, k = 10, approximate at recall ≥ {t:g}"))
        shares = sorted({json.loads(k[3])["share"] for k in cells if k[1] == "threshold" and k[2] == f})
        if shares:
            sh = shares[len(shares) // 2]
            row.append((_series(cells, "threshold", lambda k: k[2] == f and json.loads(k[3])["share"] == sh,
                                lambda k: N(k[0])), f"Threshold, answer {share_label(sh)}"))
        rows.append((f, [r for r in row if any(x["pts"] for x in r[0])]))
    rows = [r for r in rows if r[1]]
    if rows:
        ncol = max(len(r[1]) for r in rows)
        fig, axes = P.figure(plt, len(rows), ncol)
        for i, (f, row) in enumerate(rows):
            for j in range(len(row), ncol):
                axes[i, j].axis("off")
            vis = "all items visible" if f == "none" else f"{_vis_share(f):g}% visible at random" if _vis_share(f) else f
            for j, (series, title) in enumerate(row):
                P.draw(axes[i, j], series, "vectors (N)", f"{title}; {vis}", *ticks)
        P.finish(fig, plt, os.path.join(root, "scale.png"), "Time per query as the collection grows, 1 thread",
                 f"Median time per query ({_protocol(suite)}). Solid lines are exact. Dashed lines are approximate "
                 "methods at their cheapest setting reaching the recall\ntarget; hollow points did not reach it in the "
                 "settings run and show the recall reached. A system missing at a size did not fit or was not run "
                 "(see the report's regimes).")
    ok = [b for b in builds if b["status"] == "ok"]
    if ok:
        fig, axes = P.figure(plt, 1, 3)
        systems = sorted({b["system"] for b in ok}, key=lambda x: SYSTEM_ORDER.index(x) if x in SYSTEM_ORDER else 99)
        for ax, (field, title) in zip(axes[0], (("build_s", "Build time (s)"), ("index_gib", "Index size (GiB)"),
                                                 ("peak_rss_gib", "Build peak memory (GiB)"))):
            series = [dict(name=P.name(sname), colour=P.colour(sname), exact=True,
                           pts=[(N(b["dataset"]), float(b[field]), None, True) for b in ok
                                if b["system"] == sname and b[field]]) for sname in systems]
            P.draw(ax, series, "vectors (N)", title, *ticks)
            ax.yaxis.set_major_formatter(matplotlib.ticker.FuncFormatter(lambda v, _: f"{v:g}"))
        P.finish(fig, plt, os.path.join(root, "scale_build.png"), "Builds as the collection grows",
                 "Build time with the suite's build cores, the index's size on disk (or in memory for an in-memory "
                 "system) and the build's peak resident memory.\nA system missing at a size did not fit or was not run.")


if __name__ == "__main__":
    report(sys.argv[1])
