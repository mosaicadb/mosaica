# Tables and plot of the capped runs against the uncapped ones.
# usage: python3 -I summarise_capped.py CAPPED_DIR UNCAPPED_RESULTS_DIR PREP_DIR
# CAPPED_DIR holds ours_*.csv (dbcmp, with kind), faiss.csv, clickhouse.csv, diskann.csv. Writes CAPPED_DIR/summary.md,
# CAPPED_DIR/plot_data.csv and CAPPED_DIR/latency_capped_1t.png.
import csv, glob, os, sys
from collections import defaultdict
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

cap, unc, prep = sys.argv[1:4]
N = 999950
SH = (1e-4, 1e-3, 1e-2, 1e-1, 0.3, 0.5, 0.75)
SHL = ["0.01%", "0.1%", "1%", "10%", "30%", "50%", "75%"]
order_len = [round(s * N) for s in SH]
order = np.load(f"{prep}/order.npy", mmap_mode="r")
vis1 = np.zeros(N, bool)
vis1[np.load(f"{prep}/vis_1.npy")] = True
_sz = {}


def size(vis, si, qi):
    if vis == "all":
        return order_len[si]
    if (si, qi) not in _sz:
        _sz[si, qi] = int(vis1[np.asarray(order[qi, :order_len[si]])].sum())
    return _sz[si, qi]


warm = defaultdict(list)    # (label, threads, vis, si) -> [(ms, recall, size, full_reads, disk_kb)]
cold = {}                   # (label, threads, vis, si) -> ms
uncapped = defaultdict(list)
OURS = {"index": "ours: index", "flat": "ours: f16 scan", "forced": "ours: index path forced"}
for f in sorted(glob.glob(f"{cap}/ours_*.csv")):
    sched = os.path.basename(f)[5:-4].split("_")[0]  # r4, r320, r384
    for r in csv.DictReader(open(f)):
        lab = OURS[r["what"]] + ("" if sched == "r4" else f" ({sched})")
        if r["what"] == "flat" and sched != "r4":
            continue
        k = (lab, int(r["threads"]), r["vis"], int(r["share_index"]))
        if r["kind"] == "cold":
            cold[k] = float(r["median_ms"])
        else:
            warm[k].append((float(r["median_ms"]), 1.0, size(r["vis"], k[3], int(r["query"])), float(r["full_reads"]), float(r["disk_kb"])))
for name in ("faiss.csv", "clickhouse.csv", "diskann.csv"):
    p = f"{cap}/{name}"
    if not os.path.exists(p):
        continue
    for r in csv.DictReader(open(p)):
        w = r["what"]
        if name == "faiss.csv":
            lab = {"faiss_sqflat": "faiss SQ fp16 flat range (mmap)", "faiss_flat": "faiss IndexFlatIP range (mmap)"}.get(w)
            if lab is None:
                lab = ("faiss IVF4096 SQ fp16 range (on-disk lists), nprobe " if w.startswith("faiss_sqivf") else
                       "faiss IVF4096 f32 range (on-disk lists), nprobe ") + w.rsplit("_np", 1)[1]
            rec = float(r["recall"])
        elif name == "clickhouse.csv":
            lab, rec = "ClickHouse BFloat16", 1.0
        else:
            lab, rec = f"DiskANN top-m, L = {r['setting'][1:]} m", float(r["recall"])
        k = (lab, int(r["threads"]) if "threads" in r else 1, r["vis"], int(r["share_index"]))
        if r.get("kind") == "cold":
            cold[k] = float(r["median_ms"])
        else:
            warm[k].append((float(r["median_ms"]), rec, size(r["vis"], k[3], int(r["query"])), np.nan, np.nan))
# uncapped: ours from dbcmp_r4_1t, the rest from plot_data.csv
for r in csv.DictReader(open(f"{unc}/ours/dbcmp_r4_1t.csv")):
    if int(r["query"]) % 10 == 0 and r["vis"] in ("all", "vis_1"):
        uncapped[(OURS[r["what"]], int(r["threads"]), r["vis"], int(r["share_index"]))].append(float(r["median_ms"]))
for r in csv.DictReader(open(f"{unc}/plot_data.csv")):
    uncapped[(r["system"], int(r["threads"]), r["vis"], int(r["share_index"]), "pd")] = [float(r["median_ms"])]

UNC_NAME = {"ClickHouse BFloat16": "ClickHouse BFloat16", "faiss IndexFlatIP range (mmap)": "faiss IndexFlatIP range",
            "faiss SQ fp16 flat range (mmap)": "faiss IndexFlatIP range"}


def unc_of(lab, th, vis, si):
    if lab in OURS.values():
        v = uncapped.get((lab, th, vis, si))
        return float(np.median(v)) if v else None
    u = UNC_NAME.get(lab)
    if lab.startswith("faiss IVF4096"):
        u = "faiss IVF4096 range, nprobe 1" if lab.endswith("nprobe 1") else "faiss IVF4096 range, tight nprobe"
    v = uncapped.get((u, th, vis, si, "pd")) if u else None
    return v[0] if v else None


rows = [["system", "threads", "vis", "share", "answer_size", "median_ms", "p90_ms", "cold_ms", "uncapped_median_ms",
         "mean_recall", "full_reads_median", "storage_kib_per_query_median", "queries"]]
for k in sorted(warm, key=lambda k: (k[0], k[1], k[2], k[3])):
    v = warm[k]
    ms = [x[0] for x in v]
    u = unc_of(*k)
    rows.append([k[0], k[1], k[2], SHL[k[3]], f"{np.median([x[2] for x in v]):.0f}", f"{np.median(ms):.2f}",
                 f"{np.quantile(ms, 0.9):.2f}", f"{cold[k]:.1f}" if k in cold else "", f"{u:.2f}" if u else "",
                 f"{np.mean([x[1] for x in v]):.4f}", f"{np.nanmedian([x[3] for x in v]):.0f}" if not np.isnan(v[0][3]) else "",
                 f"{np.nanmedian([x[4] for x in v]):.0f}" if not np.isnan(v[0][4]) else "", len(v)])
with open(f"{cap}/plot_data.csv", "w", newline="") as f:
    csv.writer(f).writerows(rows)
md = []
for th in (1, 4):
    for vis in ("all", "vis_1"):
        sel = [r for r in rows[1:] if r[1] == th and r[2] == vis]
        if not sel:
            continue
        md.append(f"\n### {th} thread{'s' if th > 1 else ''}, {'unfiltered' if vis == 'all' else '1% visible'}: "
                  "capped median / cold / uncapped median, ms (recall when below 1)\n")
        md.append("| system | " + " | ".join(SHL) + " |")
        md.append("|---|" + "---|" * 7)
        for lab in sorted({r[0] for r in sel}, key=lambda l: (not l.startswith("ours"), l)):
            cells = []
            for s in SHL:
                r = next((r for r in sel if r[0] == lab and r[3] == s), None)
                if r is None:
                    cells.append("–")
                    continue
                c = f"{r[5]} / {r[7] or '–'} / {r[8] or '–'}"
                if float(r[9]) < 0.99995:
                    c += f" r {float(r[9]):.3f}"
                cells.append(c)
            md.append(f"| {lab} | " + " | ".join(cells) + " |")
        reads = [r for r in sel if r[10]]
        if reads:
            md.append("\nFull-vector reads per query (median over queries) / storage KiB read per query:\n")
            md.append("| system | " + " | ".join(SHL) + " |")
            md.append("|---|" + "---|" * 7)
            for lab in sorted({r[0] for r in reads}):
                cells = [next((f"{r[10]} / {r[11]}" for r in reads if r[0] == lab and r[3] == s), "–") for s in SHL]
                md.append(f"| {lab} | " + " | ".join(cells) + " |")
open(f"{cap}/summary.md", "w").write("\n".join(md).lstrip() + "\n")
print("\n".join(md))

fig, axes = plt.subplots(1, 2, figsize=(15, 6.8), sharey=True)
cols = plt.get_cmap("tab10").colors
labs = sorted({k[0] for k in warm if k[1] == 1}, key=lambda l: (not l.startswith("ours"), l))
for ax, vis, title in ((axes[0], "all", "unfiltered"), (axes[1], "vis_1", "1% visible set")):
    for i, lab in enumerate(labs):
        pts = sorted((np.median([x[2] for x in warm[k]]), np.median([x[0] for x in warm[k]]), np.mean([x[1] for x in warm[k]]), unc_of(*k))
                     for k in warm if k[0] == lab and k[1] == 1 and k[2] == vis)
        if not pts:
            continue
        xs = [max(p[0], 0.5) for p in pts]
        exact = lab.startswith(("ours", "faiss SQ", "faiss IndexFlat", "ClickHouse"))
        c = "black" if lab == "ours: index" else cols[i % 10]
        ax.plot(xs, [p[1] for p in pts], "-" if exact else "--", marker="o" if exact else "s", ms=4,
                lw=2.6 if lab.startswith("ours") else 1.4, color=c, label=lab + " (capped)")
        if not exact:
            for x, p in zip(xs, pts):
                ax.annotate(f"{p[2]:.3f}", (x, p[1]), fontsize=7, color=c, xytext=(3, 3), textcoords="offset points")
        ux = [(x, p[3]) for x, p in zip(xs, pts) if p[3]]
        if ux:
            ax.plot([u[0] for u in ux], [u[1] for u in ux], ":", marker=".", ms=3, lw=1.2, color=c, alpha=0.6)
    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_title(f"{title}, 1 thread")
    ax.set_xlabel("answer size (items, median exact count)")
    ax.grid(True, which="both", alpha=0.3)
axes[0].set_ylabel("median ms per query (whole id set returned)")
h, l = [], []
for ax in axes:
    for hh, ll in zip(*ax.get_legend_handles_labels()):
        if ll not in l:
            h.append(hh)
            l.append(ll)
fig.legend(h, l, loc="lower center", ncol=3, fontsize=8)
fig.suptitle("Threshold filter q·x ≥ t, 1M BioCLIP-2 vectors, under a 740 MiB memory cap (page cache included). "
             "Dotted, same colour: the uncapped figure.", fontsize=10)
fig.tight_layout(rect=(0, 0.16, 1, 0.96))
fig.savefig(f"{cap}/latency_capped_1t.png", dpi=110)
print(len(rows) - 1, "rows")
