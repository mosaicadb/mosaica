# Latency against answer size within a visible set, 1 thread, one panel per visible set: random sets (1%, 10%, 50%)
# in one figure and sets of whole genera (about 1%, 10%, 50%) in another.
# usage: python3 -I plot_filtered.py RESULTS_DIR PREP_DIR
# Reads RESULTS_DIR/{filtered/dbcmp_planner_1t.csv, ann.csv, approx/liminal_approx.csv,
# filtered/liminal_approx.csv}; writes RESULTS_DIR/filtered/plot_data.csv and RESULTS_DIR/latency_{filtered,correlated}_1t.png.
# Every system on the queries with index divisible by 10 whose exact answer within the set is not empty (an empty
# answer has no top-m to ask for); x is the threshold's share of the corpus, y the median over those queries of each
# query's median time.
import csv, os, sys
from collections import defaultdict
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

res, prep = sys.argv[1:3]
N = 999950
SH = (1e-4, 1e-3, 1e-2, 1e-1, 0.3, 0.5, 0.75)
klen = [round(s * N) for s in SH]
order = np.load(f"{prep}/order.npy", mmap_mode="r")
SETS = ("vis_1", "vis_10", "vis_50", "gen_1", "gen_10", "gen_50")
masks = {}
for v in SETS:
    masks[v] = np.zeros(N, bool)
    masks[v][np.load(f"{prep}/{v}.npy")] = True
size = {}  # (vis, share, query) -> exact answer size within the set
for v in SETS:
    for qi in range(0, len(order), 10):
        o = np.asarray(order[qi, :klen[-1]])
        cs = np.cumsum(masks[v][o])
        for si, k in enumerate(klen):
            size[(v, si, qi)] = int(cs[k - 1])

pts = defaultdict(list)  # (system, vis, si) -> [(ms, recall, qi)]
LIM = {"index": "LIMINAL", "cvindex": "LIMINAL, compacted", "flat": "LIMINAL f16 scan"}
for f in ("filtered/dbcmp_planner_1t.csv",):
    for r in csv.DictReader(open(f"{res}/{f}")):
        qi = int(r["query"])
        if r["threads"] == "1" and r["vis"] in SETS and qi % 10 == 0:
            pts[(LIM[r["what"]], r["vis"], int(r["share_index"]))].append((float(r["median_ms"]), 1.0, qi))
ANN = {("flat", "compacted"): "faiss flat, compacted copy", ("hnsw_sq16", "tight"): "faiss HNSW-SQfp16",
       ("ivf_sq16", "tight"): "faiss IVF4096-SQfp16", ("liminal", "tight"): "LIMINAL approximate",
       ("acorn", "tight"): "ACORN-γ", ("acorn", "default"): "ACORN-γ, ef = m"}
for f in ("ann.csv", "approx/liminal_approx.csv", "filtered/liminal_approx.csv"):
    for r in csv.DictReader(open(f"{res}/{f}")):
        key = (r["method"], r["setting"])
        if key in ANN and r["vis"] in SETS and not (r["method"] == "flat" and r["param"] != "1"):
            pts[(ANN[key], r["vis"], int(r["share_index"]))].append((float(r["median_ms"]), float(r["recall"]), int(r["query"])))

rows = [["system", "vis", "share_index", "answer_size_median", "median_ms", "p90_ms", "mean_recall", "queries"]]
lines = defaultdict(list)
for (lab, vis, si), v in sorted(pts.items()):
    v = [x for x in v if size[(vis, si, x[2])] > 0]
    if not v:
        continue
    ms = [x[0] for x in v]
    rec = float(np.mean([x[1] for x in v]))
    m = float(np.median([size[(vis, si, x[2])] for x in v]))
    rows.append([lab, vis, si, f"{m:.0f}", f"{np.median(ms):.3f}", f"{np.quantile(ms, 0.9):.3f}", f"{rec:.4f}", len(v)])
    lines[(lab, vis)].append((100 * SH[si], float(np.median(ms)), rec))
os.makedirs(f"{res}/filtered", exist_ok=True)
with open(f"{res}/filtered/plot_data.csv", "w", newline="") as f:
    csv.writer(f).writerows(rows)

# exact methods solid, approximate (at mean recall >= 0.99) dashed, hollow where 0.99 was not reached within 5 s
SERIES = [  # (label, colour, exact)
    ("LIMINAL", "#2a78d6", True),
    ("LIMINAL, compacted", "#86b6ec", True),
    ("LIMINAL f16 scan", "#123e7a", True),
    ("LIMINAL approximate", "#e34948", False),
    ("faiss flat, compacted copy", "#1baf7a", True),
    ("faiss HNSW-SQfp16", "#eb6834", False),
    ("faiss IVF4096-SQfp16", "#eda100", False),
    ("ACORN-γ", "#6250d6", False),
]
SHARES = [0.01, 0.1, 1, 10, 30, 50, 75]
INK, MUTED, GRID = "#0b0b0b", "#52514e", "#e4e3df"
plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 10, "axes.edgecolor": MUTED, "axes.labelcolor": INK,
                     "xtick.color": MUTED, "ytick.color": MUTED, "axes.spines.top": False, "axes.spines.right": False})


def draw(panels, title, xlabel, path):
    fig, axes = plt.subplots(1, 3, figsize=(19, 8.6), sharey=True)
    for ax, (vis, ptitle) in zip(axes, panels):
        ends = []
        for key, colour, exact in SERIES:
            p = sorted(lines.get((key, vis), []))
            if not p:
                continue
            xs, ys, rs = zip(*p)
            scan = key == "LIMINAL f16 scan"
            ax.plot(xs, ys, linestyle="-" if exact else (0, (5, 3)), color=colour, lw=2.8 if key in ("LIMINAL", "LIMINAL, compacted") else 1.4 if scan else 1.8,
                    zorder=4 if key.startswith("LIMINAL") else 3)
            shown = None
            for x, y, r in zip(xs, ys, rs):
                ok = exact or r >= 0.99
                ax.plot([x], [y], "s" if scan else "o", ms=4.5 if scan else 5.5, mec=colour, mfc=colour if ok else "white", mew=1.6, zorder=5)
                # the recall of a hollow point, unless it repeats the last one shown or the line-end name gives it
                if not ok and x != xs[-1] and (shown is None or abs(r - shown) > 0.02):
                    ax.annotate(f"{r:.3f}", (x, y), xytext=(0, 7 if x == xs[0] or key == "faiss HNSW-SQfp16" else -14), textcoords="offset points", fontsize=7.5, color=colour, ha="center")
                    shown = r
            name = key if exact or rs[-1] >= 0.99 else f"{key} (recall {rs[-1]:.3f})"
            if xs[-1] < 75 and not exact:
                name += ", none further"
            ends.append([ys[-1], xs[-1], name, colour])
        ends.sort()
        ly = [np.log10(e[0]) for e in ends]
        for i in range(1, len(ly)):
            ly[i] = max(ly[i], ly[i - 1] + 0.15)
        for (y, x, name, colour), l in zip(ends, ly):
            ax.annotate(name, (x, y), xytext=(110, 10 ** l), textcoords="data", fontsize=8.5, color=INK, va="center",
                        arrowprops=dict(arrowstyle="-", color="#c9c8c2", lw=0.8, shrinkA=2, shrinkB=4))
        ax.set_xscale("log")
        ax.set_yscale("log")
        ax.set_xlim(0.007, 3500)
        ax.set_xticks(SHARES)
        ax.set_xticklabels([f"{s:g}%" if s not in (30, 50) else "" for s in SHARES])
        ax.minorticks_off()
        yt = [0.1, 1, 10, 100, 1000]
        ax.set_yticks(yt)
        ax.set_yticklabels([f"{v:g} ms" for v in yt])
        ax.grid(True, axis="y", color=GRID, lw=0.8)
        ax.set_title(ptitle, loc="left", fontsize=11, color=INK, pad=10)
        ax.set_xlabel(xlabel)
        ax.tick_params(length=0)
    axes[0].set_ylim(0.1, 30000)
    fig.suptitle(title, x=0.05, ha="left", fontsize=13, color=INK, fontweight="bold")
    fig.text(0.05, 0.885, "Median time, 1 thread, to return the whole set of matching ids (approximate methods: top-m with m the exact "
             "answer size). Solid lines are exact. Dashed lines are approximate methods\nat their fastest setting with mean "
             "recall ≥ 0.99; hollow points could not reach 0.99 within 5 s per query and show the recall reached. A line that "
             "stops early has no point further on: over 5 s per query, or not run (see the report).", fontsize=9, color=MUTED)
    fig.tight_layout(rect=(0.02, 0.02, 0.99, 0.87))
    fig.savefig(path, dpi=130, facecolor="white")
    plt.close(fig)


draw([("vis_1", "1% of items visible, at random (10k)"), ("vis_10", "10% visible, at random (100k)"),
      ("vis_50", "50% visible, at random (500k)")],
     "Threshold filter within a random visible set, 1M BioCLIP-2 vectors (768 d)",
     "answer size, as a share of the visible items", f"{res}/latency_filtered_1t.png")
draw([("gen_1", "19 whole genera visible (10k items, 1.0%)"), ("gen_10", "241 genera visible (95k items, 9.5%)"),
      ("gen_50", "964 genera visible (494k items, 49.4%)")],
     "Threshold filter within a set of whole genera, 1M BioCLIP-2 vectors (768 d)",
     "threshold, as the share of the whole corpus it selects", f"{res}/latency_correlated_1t.png")
print(len(rows) - 1, "points")
