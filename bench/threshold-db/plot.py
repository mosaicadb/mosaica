# Latency against answer size, log-log, one line per system: unfiltered and 1% visible, at 1 thread and at 4.
# usage: python3 -I plot.py RESULTS_DIR PREP_DIR
# Reads RESULTS_DIR/{ours/dbcmp_r4_1t.csv, ours/dbcmp_r4_4t.csv, clickhouse.csv, ann.csv, approx/liminal_approx.csv} and, for
# comparison, the index rows of ours/dbcmp_main.csv (the index as measured on 13 Nov); writes RESULTS_DIR/plot_data.csv and
# RESULTS_DIR/latency_{1,4}t.png. Every system on the queries with index divisible by 10; x is the
# median exact answer size over those queries, y the median over them of each query's median time.
import csv, os, sys
from collections import defaultdict
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

res, prep = sys.argv[1:3]
N = 999950
SH = (1e-4, 1e-3, 1e-2, 1e-1, 0.3, 0.5, 0.75)
order_len = [round(s * N) for s in SH]
order = np.load(f"{prep}/order.npy", mmap_mode="r")
vis1 = np.zeros(N, bool)
vis1[np.load(f"{prep}/vis_1.npy")] = True
size_cache = {}


def exact_size(vis, si, qi):
    if vis == "all":
        return order_len[si]
    if (si, qi) not in size_cache:
        size_cache[(si, qi)] = int(vis1[np.asarray(order[qi, :order_len[si]])].sum())
    return size_cache[(si, qi)]


LABEL = {"index": "LIMINAL", "flat": "LIMINAL f16 scan", "clickhouse_t32_dot": "ClickHouse Float32",
         "clickhouse_tbf_dot": "ClickHouse BFloat16", "duckdb_t32_dot": "DuckDB FLOAT[768]",
         "duckdb_t32_dot+mat": "DuckDB FLOAT[768]", "old_index": "LIMINAL (13 Nov)"}
pts = defaultdict(list)  # (label, exact, threads, vis, si) -> [(ms, recall)]
for f in ("ours/dbcmp_main.csv", "ours/dbcmp_r4_1t.csv", "ours/dbcmp_r4_4t.csv", "clickhouse.csv"):
    if not os.path.exists(f"{res}/{f}"):
        continue
    for r in csv.DictReader(open(f"{res}/{f}")):
        if f == "ours/dbcmp_main.csv":
            if r["what"] != "index":
                continue
            r["what"] = "old_index"
        qi = int(r["query"])
        if qi % 10 or r["vis"] not in ("all", "vis_1"):
            continue
        pts[(LABEL[r["what"]], True, int(r["threads"]), r["vis"], int(r["share_index"]))].append(
            (float(r["median_ms"]), 1.0, exact_size(r["vis"], int(r["share_index"]), qi)))
ANN = {("hnswlib", "default"): "hnswlib top-m, ef = m", ("hnswlib", "tight"): "hnswlib top-m, tight ef",
       ("faiss_hnsw", "default"): "faiss HNSW top-m, ef = m", ("faiss_hnsw", "tight"): "faiss HNSW top-m, tight ef",
       ("ivf", "default"): "faiss IVF4096 range, nprobe 1", ("ivf", "tight"): "faiss IVF4096 range, tight nprobe",
       ("rabitq", "all_clusters"): "RaBitQ IVF 7-bit top-m, all clusters",
       ("flat", "exact"): "faiss IndexFlatIP range", ("flat", "idselector"): "faiss IndexFlatIP range",
       ("flat", "compacted"): "faiss IndexFlatIP range, compacted visible copy",
       ("panorama", "exact"): "faiss Panorama flat range",
       ("sq16", "exact"): "faiss SQfp16 flat range", ("sq16", "idselector"): "faiss SQfp16 flat range",
       ("sq16", "compacted"): "faiss SQfp16 flat range, compacted visible copy",
       ("hnsw_sq16", "default"): "faiss HNSW-SQfp16 top-m, ef = m", ("hnsw_sq16", "tight"): "faiss HNSW-SQfp16 top-m, tight ef",
       ("ivf_sq16", "default"): "faiss IVF4096-SQfp16 range, nprobe 1",
       ("ivf_sq16", "tight"): "faiss IVF4096-SQfp16 range, tight nprobe",
       ("liminal", "default"): "LIMINAL approximate top-m, default", ("liminal", "tight"): "LIMINAL approximate top-m, tight"}
for f in ("ann.csv", "approx/liminal_approx.csv"):
    if not os.path.exists(f"{res}/{f}"):
        continue
    for r in csv.DictReader(open(f"{res}/{f}")):
        exact = r["method"] in ("flat", "panorama", "sq16")
        th = int(r["param"]) if exact else 1
        pts[(ANN[(r["method"], r["setting"])], exact, th, r["vis"], int(r["share_index"]))].append(
            (float(r["median_ms"]), float(r["recall"]), int(r["exact"])))

rows = [["system", "exact", "threads", "vis", "share_index", "answer_size_median", "median_ms", "p90_ms", "mean_recall", "queries"]]
lines = defaultdict(list)
for (lab, ex, th, vis, si), v in sorted(pts.items()):
    ms = [x[0] for x in v]
    x = float(np.median([x[2] for x in v]))
    rec = float(np.mean([x[1] for x in v]))
    rows.append([lab, int(ex), th, vis, si, f"{x:.0f}", f"{np.median(ms):.3f}", f"{np.quantile(ms, 0.9):.3f}", f"{rec:.4f}", len(v)])
    lines[(lab, ex, th, vis)].append((max(x, 0.5), float(np.median(ms)), rec))
with open(f"{res}/plot_data.csv", "w", newline="") as f:
    csv.writer(f).writerows(rows)


# The figures: exact methods solid, approximate methods (at mean recall >= 0.99) dashed, hollow markers where a
# method could not reach 0.99. Colours follow the system, in the validated categorical order; names sit at line ends.
SHARES = [0.01, 0.1, 1, 10, 30, 50, 75]
NVIS = {"all": 999_950, "vis_1": 9_999.5}
SERIES = [  # (label in plot_data, name on the chart, colour, exact, panels)
    ("LIMINAL", "LIMINAL", "#2a78d6", True, ("all", "vis_1")),
    ("LIMINAL f16 scan", "LIMINAL f16 scan", "#2a78d6", True, ("all", "vis_1")),
    ("LIMINAL approximate top-m, tight", "LIMINAL approximate", "#e34948", False, ("all", "vis_1")),
    ("faiss Panorama flat range", "faiss Panorama", "#6250d6", True, ("all", "vis_1")),
    ("faiss IndexFlatIP range", "faiss flat", "#1baf7a", True, ("all",)),
    ("faiss IndexFlatIP range, compacted visible copy", "faiss flat, compacted copy", "#1baf7a", True, ("vis_1",)),
    ("ClickHouse BFloat16", "ClickHouse bf16", "#008300", True, ("all", "vis_1")),
    ("hnswlib top-m, tight ef", "hnswlib", "#eb6834", False, ("all",)),
    ("faiss HNSW top-m, tight ef", "faiss HNSW", "#eb6834", False, ("vis_1",)),
    ("faiss IVF4096 range, tight nprobe", "faiss IVF", "#eda100", False, ("all", "vis_1")),
    ("RaBitQ IVF 7-bit top-m, all clusters", "RaBitQ", "#e87ba4", False, ("all",)),
]
INK, MUTED, GRID = "#0b0b0b", "#52514e", "#e4e3df"
plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 10, "axes.edgecolor": MUTED, "axes.labelcolor": INK,
                     "xtick.color": MUTED, "ytick.color": MUTED, "axes.spines.top": False, "axes.spines.right": False})

def share(x, vis):
    s = 100 * x / NVIS[vis]
    return min(SHARES, key=lambda v: abs(np.log(v) - np.log(s)))

def draw(th, path):
    fig, axes = plt.subplots(1, 2, figsize=(13, 8.2), sharey=True)
    for ax, vis, title in ((axes[0], "all", "All 1M items visible"), (axes[1], "vis_1", "1% of items visible (10k)")):
        ends = []
        for key, name, colour, exact, panels in SERIES:
            pts = sorted(lines.get((key, 1 if exact else 0, th, vis), []))
            if not pts and key == "faiss Panorama flat range":
                # faiss runs one range-search query on one thread, so its 1-thread figures stand in, labelled as such
                pts = sorted(lines.get((key, 1, 1, vis), []))
                name = f"{name} (1-thread run)" if pts else name
            if vis not in panels or not pts:
                continue
            xs = [share(x, vis) for x, _, _ in pts]
            ys = [y for _, y, _ in pts]
            rs = [r for _, _, r in pts]
            scan = key == "LIMINAL f16 scan"
            lw = 2.8 if key == "LIMINAL" else 1.8
            ax.plot(xs, ys, linestyle="-" if exact else (0, (5, 3)), color=colour, lw=lw, alpha=0.55 if scan else 1, zorder=4 if key.startswith("LIMINAL") else 3)
            for x, y, r in zip(xs, ys, rs):
                ok = exact or r >= 0.99
                ax.plot([x], [y], "o", ms=5.5, mec=colour, mfc=colour if ok else "white", mew=1.6, alpha=0.55 if scan else 1, zorder=5)
                if not ok and x != xs[-1] and x != 50:
                    # moved where the default spot falls on another series' point or line
                    off = {("RaBitQ", 0.01): (14, -46), ("hnswlib", 0.01): (7, -20), ("faiss HNSW", 10): (8, 4)}.get((name, x), (7, -12))
                    who = f"{name}: " if (name, x) in (("RaBitQ", 0.01), ("hnswlib", 0.01)) else ""
                    ax.annotate(f"{who}recall {r:.3f}", (x, y), xytext=off, textcoords="offset points", fontsize=8, color=MUTED,
                                arrowprops=dict(arrowstyle="-", color="#c9c8c2", lw=0.8, shrinkA=0, shrinkB=4) if off[1] < -20 else None)
            ends.append([ys[-1], xs[-1], name if exact or rs[-1] >= 0.99 else f"{name} (recall {rs[-1]:.2f})"])
        # names at the line ends, nudged apart in log space
        ends.sort()
        ly = [np.log10(e[0]) for e in ends]
        for i in range(1, len(ly)):
            ly[i] = max(ly[i], ly[i - 1] + 0.1)
        for (y, x, name), l in zip(ends, ly):
            ax.annotate(name, (x, y), xytext=(110, 10 ** l), textcoords="data", fontsize=9, color=INK, va="center",
                        arrowprops=dict(arrowstyle="-", color="#c9c8c2", lw=0.8, shrinkA=2, shrinkB=4) if x < 75 or abs(np.log10(y) - l) > 0.03 else None)
        ax.set_xscale("log")
        ax.set_yscale("log")
        ax.set_xlim(0.007, 1500)
        ax.set_xticks(SHARES)
        ax.set_xticklabels([f"{s:g}%" if s not in (30, 50) else "" for s in SHARES])
        ax.minorticks_off()
        yt = [0.1, 1, 10, 100, 1000]
        ax.set_yticks(yt)
        ax.set_yticklabels([f"{v:g} ms" for v in yt])
        ax.grid(True, axis="y", color=GRID, lw=0.8)
        ax.set_title(title, loc="left", fontsize=11, color=INK, pad=10)
        ax.set_xlabel("answer size, as a share of the visible items")
        ax.tick_params(length=0)
    axes[0].set_ylim(0.2, 8000)
    fig.suptitle(f"Exact threshold filter q·x ≥ t on 1M BioCLIP-2 vectors (768 d), {th} thread{'s' if th > 1 else ''}",
                 x=0.06, ha="left", fontsize=13, color=INK, fontweight="bold")
    approx = any(k[1] == 0 and k[2] == th for k in lines)
    fig.text(0.06, 0.885, "Median time to return the whole set of matching ids. Solid lines are exact." + (" Dashed lines are approximate methods at\n"
             "their fastest setting with mean recall ≥ 0.99; hollow points could not reach 0.99 within 5 s and show the recall reached." if approx else ""),
             fontsize=9, color=MUTED)
    fig.tight_layout(rect=(0.03, 0.02, 0.97, 0.87))
    fig.savefig(path, dpi=130, facecolor="white")
    plt.close(fig)

draw(1, f"{res}/latency_1t.png")
draw(4, f"{res}/latency_4t.png")
print(len(rows) - 1, "points")
