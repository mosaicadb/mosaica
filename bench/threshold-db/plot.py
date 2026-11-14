# Latency against answer size, log-log, one line per system: unfiltered and 1% visible, at 1 thread and at 4.
# usage: python3 -I plot.py RESULTS_DIR PREP_DIR
# Reads RESULTS_DIR/{ours/dbcmp_main.csv, clickhouse.csv, duckdb.csv, ann.csv}; writes RESULTS_DIR/plot_data.csv and
# RESULTS_DIR/latency_{1,4}t.png. Every system on the queries with index divisible by 10 (DuckDB: by 30); x is the
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


LABEL = {"index": "ours: index", "flat": "ours: f16 scan", "clickhouse_t32_dot": "ClickHouse Float32",
         "clickhouse_tbf_dot": "ClickHouse BFloat16", "duckdb_t32_dot": "DuckDB FLOAT[768]",
         "duckdb_t32_dot+mat": "DuckDB FLOAT[768]"}
pts = defaultdict(list)  # (label, exact, threads, vis, si) -> [(ms, recall)]
for f in ("ours/dbcmp_main.csv", "clickhouse.csv", "duckdb.csv"):
    if not os.path.exists(f"{res}/{f}"):
        continue
    for r in csv.DictReader(open(f"{res}/{f}")):
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
       ("panorama", "exact"): "faiss Panorama flat range"}
if os.path.exists(f"{res}/ann.csv"):
    for r in csv.DictReader(open(f"{res}/ann.csv")):
        exact = r["method"] in ("flat", "panorama")
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

colours = plt.get_cmap("tab20").colors
labs = sorted({k[0] for k in lines})
col = {l: colours[i % 20] for i, l in enumerate(labs)}
for th in (1, 4):
    fig, axes = plt.subplots(1, 2, figsize=(15, 6.5), sharey=True)
    for ax, vis, title in ((axes[0], "all", "unfiltered"), (axes[1], "vis_1", "1% visible set")):
        for (lab, ex, t, v), p in sorted(lines.items()):
            if t != th or v != vis:
                continue
            p.sort()
            xs, ys, rs = zip(*p)
            ours = lab.startswith("ours")
            ax.plot(xs, ys, "-" if ex else "--", marker="o" if ex else "s", ms=4, lw=2.6 if ours else 1.4,
                    color=("black" if lab == "ours: index" else "dimgrey") if ours else col[lab], label=lab, zorder=3 if ours else 2)
            if not ex:
                for x, y, r in p:
                    ax.annotate(f"{r:.3f}", (x, y), fontsize=7, color=col[lab], xytext=(3, 3), textcoords="offset points")
        ax.set_xscale("log")
        ax.set_yscale("log")
        ax.set_title(f"{title}, {th} thread{'s' if th > 1 else ''}")
        ax.set_xlabel("answer size (items, median exact count)")
        ax.grid(True, which="both", alpha=0.3)
    axes[0].set_ylabel("median ms per query (whole id set returned)")
    h, l = [], []
    for ax in axes:
        for hh, ll in zip(*ax.get_legend_handles_labels()):
            if ll not in l:
                h.append(hh)
                l.append(ll)
    fig.legend(h, l, loc="lower center", ncol=4, fontsize=8)
    fig.suptitle("Threshold filter q·x ≥ t on 1M BioCLIP-2 vectors (768 d): solid = exact, dashed = approximate "
                 "(labels: mean recall)", fontsize=10)
    fig.tight_layout(rect=(0, 0.13, 1, 0.97))
    fig.savefig(f"{res}/latency_{th}t.png", dpi=110)
print(len(rows) - 1, "points")
