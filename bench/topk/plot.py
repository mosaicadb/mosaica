# Top-k latency against k, log-log, 1 thread, unfiltered: one panel per recall target (0.8 and 0.99), LIMINAL exact in
# both, and LIMINAL approximate before its best-first rework as a faint line. With RESULTS_DIR/liminal_topk_vis_1.csv,
# a second figure at 1% visible: LIMINAL approximate before and after, both targets.
# usage: python3 -I plot.py RESULTS_DIR
# Reads RESULTS_DIR/{liminal_topk.csv, liminal_topk_vis_1.csv, ann_*.csv}; writes RESULTS_DIR/plot_data.csv,
# ../latency_topk_1t.png and ../latency_topk_vis1_1t.png.
# y is the median over the queries of each query's median time; recall is the mean over the queries.
import csv, glob, os, sys
from collections import defaultdict
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

res = sys.argv[1]
pts = defaultdict(list)  # (method, target, k) -> [(ms, recall, param, reached)]
for r in csv.DictReader(open(f"{res}/liminal_topk.csv")):
    pts[(r["method"], r["target"], int(r["k"]))].append((float(r["median_ms"]), float(r["recall"]), r["param"], 1))
for f in sorted(glob.glob(f"{res}/ann_*.csv")):
    for r in csv.DictReader(open(f)):
        pts[(r["method"], str(float(r["target"])), int(r["k"]))].append(
            (float(r["median_ms"]), float(r["recall"]), r["param"], int(r["reached"])))

vpts = defaultdict(list)
if os.path.exists(f"{res}/liminal_topk_vis_1.csv"):
    for r in csv.DictReader(open(f"{res}/liminal_topk_vis_1.csv")):
        vpts[(r["method"], r["target"], int(r["k"]))].append((float(r["median_ms"]), float(r["recall"]), r["param"], 1))

rows = [["vis", "method", "target", "k", "param", "median_ms", "p90_ms", "mean_recall", "reached", "queries"]]
lines, vlines = defaultdict(list), defaultdict(list)
for vis, src, dst in (("all", pts, lines), ("vis_1", vpts, vlines)):
    for (m, tg, k), v in sorted(src.items()):
        ms = [x[0] for x in v]
        rec = float(np.mean([x[1] for x in v]))
        ok = all(x[3] for x in v)
        rows.append([vis, m, tg, k, v[0][2], f"{np.median(ms):.3f}", f"{np.quantile(ms, 0.9):.3f}", f"{rec:.4f}", int(ok), len(v)])
        dst[(m, tg)].append((k, float(np.median(ms)), rec, ok))
with open(f"{res}/plot_data.csv", "w", newline="") as f:
    csv.writer(f).writerows(rows)

SERIES = [  # (method, name, colour, exact)
    ("liminal_exact", "LIMINAL (exact)", "#2a78d6", True),
    ("liminal_approx", "LIMINAL approximate", "#2a78d6", False),
    ("liminal_approx_before", "LIMINAL approximate, before", "#9cc0ea", False),
    ("hnsw_sq16", "faiss HNSW-SQfp16", "#eb6834", False),
    ("ivf_sq16", "faiss IVF4096-SQfp16", "#eda100", False),
    ("rabitq", "RaBitQ", "#e87ba4", False),
]
INK, MUTED, GRID = "#0b0b0b", "#52514e", "#e4e3df"
plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 10, "axes.edgecolor": MUTED, "axes.labelcolor": INK,
                     "xtick.color": MUTED, "ytick.color": MUTED, "axes.spines.top": False, "axes.spines.right": False})
KS = [1, 10, 100, 1000]
fig, axes = plt.subplots(1, 2, figsize=(13, 7.4), sharey=True)
for ax, tg in zip(axes, ("0.8", "0.99")):
    ends = []
    for m, name, colour, exact in SERIES:
        p = sorted(lines.get((m, "exact" if exact else tg), []))
        if not p:
            continue
        xs, ys, rs, oks = zip(*p)
        ax.plot(xs, ys, linestyle="-" if exact else (0, (5, 3)), color=colour, lw=2.8 if exact else 1.8, zorder=4 if exact else 3)
        for x, y, r, ok in zip(xs, ys, rs, oks):
            ax.plot([x], [y], "o", ms=5.5, mec=colour, mfc=colour if ok else "white", mew=1.6, zorder=5)
            if not ok:
                ax.annotate(f"recall {r:.3f}", (x, y), xytext=(-20, 9), textcoords="offset points", fontsize=8, color=MUTED)
        ends.append([ys[-1], xs[-1], name])
    ends.sort()
    ly = [np.log10(e[0]) for e in ends]
    for i in range(1, len(ly)):
        ly[i] = max(ly[i], ly[i - 1] + 0.12)
    for (y, x, name), l in zip(ends, ly):
        ax.annotate(name, (x, y), xytext=(1700, 10 ** l), textcoords="data", fontsize=9, color=INK, va="center",
                    arrowprops=dict(arrowstyle="-", color="#c9c8c2", lw=0.8, shrinkA=2, shrinkB=4) if abs(np.log10(y) - l) > 0.03 else None)
    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_xlim(0.6, 30000)
    ax.set_xticks(KS)
    ax.set_xticklabels(["1", "10", "100", "1k"])
    ax.minorticks_off()
    yt = [0.1, 1, 10, 100, 1000]
    ax.set_yticks(yt)
    ax.set_yticklabels([f"{v:g} ms" for v in yt])
    ax.grid(True, axis="y", color=GRID, lw=0.8)
    ax.set_title(f"Approximate methods at mean recall@k ≥ {tg}", loc="left", fontsize=11, color=INK, pad=10)
    ax.set_xlabel("k")
    ax.tick_params(length=0)
axes[0].set_ylim(0.05, 3000)
fig.suptitle("Top-k search on 1M BioCLIP-2 vectors (768 d), 1 thread", x=0.06, ha="left", fontsize=13, color=INK, fontweight="bold")
fig.text(0.06, 0.885, "Median time per query over 15 queries (5 image, 10 text). The solid line is exact (the same in both panels). Dashed lines are\n"
         "approximate methods at their cheapest setting reaching the target; hollow points did not reach it within 5 s per query or by the end of the ladder, labelled with the recall reached.",
         fontsize=9, color=MUTED)
fig.tight_layout(rect=(0.03, 0.02, 0.97, 0.9))
fig.savefig(f"{os.path.dirname(os.path.abspath(res))}/latency_topk_1t.png", dpi=130, facecolor="white")

if vlines:
    fig, ax = plt.subplots(figsize=(8, 5.6))
    for m, name, colour in (("liminal_approx", "LIMINAL approximate", "#2a78d6"), ("liminal_approx_before", "LIMINAL approximate, before", "#9cc0ea")):
        for tg, ls in (("0.8", (0, (2, 2))), ("0.99", "-")):
            p = sorted(vlines.get((m, tg), []))
            if not p:
                continue
            xs, ys, rs, oks = zip(*p)
            ax.plot(xs, ys, linestyle=ls, color=colour, lw=2, marker="o", ms=5, label=f"{name}, recall ≥ {tg}")
    ax.legend(frameon=False, fontsize=9, loc="upper left")
    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_xlim(0.6, 2000)
    ax.set_ylim(0.2, 10)
    ax.set_xticks(KS)
    ax.set_xticklabels(["1", "10", "100", "1k"])
    ax.minorticks_off()
    yt = [0.2, 0.5, 1, 2, 5, 10]
    ax.set_yticks(yt)
    ax.set_yticklabels([f"{v:g} ms" for v in yt])
    ax.grid(True, axis="y", color=GRID, lw=0.8)
    ax.set_xlabel("k")
    ax.tick_params(length=0)
    ax.set_title("LIMINAL approximate top-k at 1% visible, 1 thread", loc="left", fontsize=12, color=INK, fontweight="bold", pad=10)
    fig.tight_layout()
    fig.savefig(f"{os.path.dirname(os.path.abspath(res))}/latency_topk_vis1_1t.png", dpi=130, facecolor="white")
print(len(rows) - 1, "points")
