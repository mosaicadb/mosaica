# The capped plot, in the style of ../plot.py: LIMINAL's index and f16 scan under the memory cap against its
# uncapped index, with the competitors whose capped runs completed.
# usage: python3 -I plot_capped.py RESULTS_DIR
# Reads RESULTS_DIR/capped/{ours_*.csv, clickhouse.csv, faiss.csv}, RESULTS_DIR/disk/ours_*.csv (LIMINAL reading its
# full vectors through io_uring) and RESULTS_DIR/ours/dbcmp_r4_1t.csv (uncapped);
# writes RESULTS_DIR/latency_capped_1t.png. Every system on the queries with index divisible by 10; y is the median over
# them of each query's median time, x the answer share.
import csv, sys
from collections import defaultdict
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

res = sys.argv[1]
CAP_MB = 776  # 775,946,240 bytes = 740 MiB
SHARES = [0.01, 0.1, 1, 10, 30, 50, 75]
pts = defaultdict(list)  # (series, vis, share_index) -> [ms], [recall]


def add(key, vis, si, ms, rec=1.0):
    pts[(key, vis, si)].append((ms, rec))


# unfiltered from the runs with a madvise(WILLNEED) per batch; at 1% visible from the run that skips it when a sample
# of the batch is resident (ours_r4_visskip_1t), the read path now in the code
for f, key, vis in (("capped/ours_r4_index_1t.csv", "index", "all"), ("capped/ours_r384_index_1t.csv", "r384", "all"),
                    ("capped/ours_r4_flatall_1t.csv", "flat", "all"), ("capped/ours_r4_visskip_1t.csv", None, "vis_1")):
    for r in csv.DictReader(open(f"{res}/{f}")):
        if r["kind"] == "warm" and r["vis"] == vis and r["what"] in ("index", "flat") and int(r["query"]) % 10 == 0:
            add(key or r["what"], r["vis"], int(r["share_index"]), float(r["median_ms"]))
for f, key in (("disk/ours_r4_index_1t.csv", "disk"), ("disk/ours_r384_index_1t.csv", "disk384")):
    for r in csv.DictReader(open(f"{res}/{f}")):
        if r["kind"] == "warm" and r["what"] == "index" and int(r["query"]) % 10 == 0:
            add(key, r["vis"], int(r["share_index"]), float(r["median_ms"]))
for r in csv.DictReader(open(f"{res}/ours/dbcmp_r4_1t.csv")):
    if r["what"] == "index" and r["vis"] in ("all", "vis_1") and int(r["query"]) % 10 == 0:
        add("uncapped", r["vis"], int(r["share_index"]), float(r["median_ms"]))
for r in csv.DictReader(open(f"{res}/capped/clickhouse.csv")):
    if r["kind"] == "warm" and int(r["query"]) % 10 == 0:
        add("ch", r["vis"], int(r["share_index"]), float(r["median_ms"]))
for r in csv.DictReader(open(f"{res}/capped/faiss.csv")):
    # IVF at its tight nprobe (mean recall >= 0.99 uncapped), not nprobe 1
    if r["kind"] == "warm" and r["what"].startswith("faiss_sqivf") and not r["what"].endswith("_np1"):
        add("ivf", r["vis"], int(r["share_index"]), float(r["median_ms"]), float(r["recall"]))

SERIES = [  # (key, name, colour, style, width, alpha)
    ("disk", "LIMINAL", "#2a78d6", "-", 2.8, 1),
    ("disk384", "LIMINAL, five levels", "#1baf7a", "-", 2.2, 1),
    ("uncapped", "LIMINAL, uncapped", "#2a78d6", (0, (4, 3)), 1.2, 1),
    ("index", "LIMINAL, mmap (before)", "#2a78d6", (0, (1, 2)), 1.8, 0.6),
    ("r384", "LIMINAL, five levels, mmap (before)", "#1baf7a", (0, (1, 2)), 1.6, 0.6),
    ("flat", "LIMINAL f16 scan, mmap (before)", "#2a78d6", "-", 1.6, 0.35),
    ("ch", "ClickHouse bf16, capped", "#008300", "-", 1.8, 1),
    ("ivf", "faiss IVF-SQfp16 on disk, capped (partial)", "#eda100", (0, (5, 3)), 1.8, 1),
]
INK, MUTED, GRID = "#0b0b0b", "#52514e", "#e4e3df"
plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 10, "axes.edgecolor": MUTED, "axes.labelcolor": INK,
                     "xtick.color": MUTED, "ytick.color": MUTED, "axes.spines.top": False, "axes.spines.right": False})
fig, axes = plt.subplots(1, 2, figsize=(13, 8.2), sharey=True)
for ax, vis, title in ((axes[0], "all", "All 1M items visible"), (axes[1], "vis_1", "1% of items visible (10k)")):
    ends = []
    for key, name, colour, style, lw, alpha in SERIES:
        sis = sorted(si for (k, v, si) in pts if k == key and v == vis)
        if not sis:
            continue
        xs = [SHARES[si] for si in sis]
        ys = [float(np.median([m for m, _ in pts[(key, vis, si)]])) for si in sis]
        rs = [float(np.mean([r for _, r in pts[(key, vis, si)]])) for si in sis]
        ax.plot(xs, ys, linestyle=style, color=colour, lw=lw, alpha=alpha, zorder=4 if key != "ch" else 3)
        for x, y, r in zip(xs, ys, rs):
            ok = r >= 0.99
            ax.plot([x], [y], "o", ms=5.5 if lw > 1.5 else 3.5, mec=colour, mfc=colour if ok else "white", mew=1.6, alpha=alpha, zorder=5)
        ends.append([ys[-1], xs[-1], name])
    ends.sort()
    ly = [np.log10(e[0]) for e in ends]
    for i in range(1, len(ly)):
        ly[i] = max(ly[i], ly[i - 1] + 0.2)
    for (y, x, name), l in zip(ends, ly):
        ax.annotate(name, (x, y), xytext=(110, 10 ** l), textcoords="data", fontsize=9, color=INK, va="center",
                    arrowprops=dict(arrowstyle="-", color="#c9c8c2", lw=0.8, shrinkA=2, shrinkB=4) if x < 75 or abs(np.log10(y) - l) > 0.03 else None)
    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_xlim(0.007, 3000)
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
fig.suptitle(f"LIMINAL under a {CAP_MB} MB memory cap: 1M BioCLIP-2, 1 thread", x=0.06, ha="left", fontsize=13,
             color=INK, fontweight="bold")
fig.text(0.06, 0.875,
         f"The cap ({CAP_MB} MB = 740 MiB, cgroup memory.max, no swap) holds the whole process including its page cache: LIMINAL's codes and\n"
         "parameters (428 MiB for the default four levels, 577 MiB for five) stay in memory and the rest caches about a fifth of the 1.5 GB of\n"
         "f16 vectors, which are read from disk. LIMINAL reads them with io_uring (O_DIRECT for merged reads, a disk-aware planner); the dotted\n"
         "lines read them through the mapping. Median time to return the whole set of matching ids, warm within the cap. All lines but faiss are\n"
         "exact; faiss IVF is approximate at its uncapped tight nprobe (recall ≥ 0.99), its 1% points from one repetition.",
         fontsize=9, color=MUTED)
fig.tight_layout(rect=(0.03, 0.02, 0.97, 0.85))
fig.savefig(f"{res}/latency_capped_1t.png", dpi=130, facecolor="white")
print("wrote", f"{res}/latency_capped_1t.png")
