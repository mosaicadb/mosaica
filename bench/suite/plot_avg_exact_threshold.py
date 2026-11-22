"""Threshold queries or exact k-NN: one panel per filter, each system's speed-up over faiss flat, averaged over datasets.

Dot: geometric mean over datasets. Bar: the range across datasets.
Usage: python3 bench/suite/plot_avg_exact_threshold.py <results/compare dir> <out png> [threshold|exact]
"""
import csv, collections, statistics as st, sys, pathlib, math, json
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.ticker import FixedLocator, NullLocator

root, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
KIND = sys.argv[3] if len(sys.argv) > 3 else "threshold"
dss = sorted(d.name for d in root.iterdir() if (d / "check.csv").exists() and (d / "vexact.csv").exists())
FILTERS = [("none", "Unfiltered"), ("random-25", "random-25"), ("region-25", "region-25"),
           ("random-1", "random-1"), ("region-1", "region-1")]
# (system, compacted?, label, colour)
SYSTEMS = [("vexact", False, "LIMINAL", "#2a78d6"),
           ("faiss-flat", True, "faiss flat, pre-compacted copy", "#eb6834"),
           ("faiss-panorama", False, "faiss Panorama", "#1baf7a")]
if KIND == "exact":
    SYSTEMS.append(("pdx-bond", False, "PDX-BOND", "#eda100"))
INK, MUTED, GRID = "#1f1f1e", "#6b6a64", "#e4e3de"

def times(ds, sysn, compacted):
    """{(filter, share): median over queries of per-query median ms}"""
    p = root / ds / f"{sysn}.csv"
    g = collections.defaultdict(list)
    if p.exists():
        for r in csv.DictReader(open(p)):
            if r["type"] == KIND and ("compact" in r["setting"]) == compacted:
                prm = json.loads(r["param"])
                g[(r["filter"], prm.get("share", prm.get("k")))].append(float(r["median_ms"]))
    return {k: st.median(v) for k, v in g.items()}

flat = {ds: times(ds, "faiss-flat", False) for ds in dss}
tm = {(ds, s, c): times(ds, s, c) for ds in dss for s, c, *_ in SYSTEMS}
shares = {f: sorted({k[1] for ds in dss for k in flat[ds] if k[0] == f}) for f, _ in FILTERS}

fig, axes = plt.subplots(1, len(FILTERS), figsize=(13, 5.0), sharey=True,
                         gridspec_kw={"width_ratios": [max(1, len(shares[f])) + 0.6 for f, _ in FILTERS]})
for ax, (filt, title) in zip(axes, FILTERS):
    present = [s for s in SYSTEMS if any(k[0] == filt for ds in dss for k in tm[(ds, s[0], s[1])])]
    w = 0.7 / len(present)
    for i, sh in enumerate(shares[filt]):
        for j, (sysn, comp, name, col) in enumerate(present):
            sp = [flat[ds][(filt, sh)] / tm[(ds, sysn, comp)][(filt, sh)] for ds in dss
                  if (filt, sh) in flat[ds] and (filt, sh) in tm[(ds, sysn, comp)]]
            if not sp:
                continue
            x = i - 0.35 + w * (j + 0.5)
            gm = math.exp(sum(map(math.log, sp)) / len(sp))
            ours = sysn == "vexact"
            ax.plot([x, x], [min(sp), max(sp)], color=col, lw=2.4 if ours else 1.6, alpha=0.55,
                    solid_capstyle="round", zorder=2)
            ax.scatter(x, gm, s=60 if ours else 40, color=col, edgecolor="white", lw=1.2, zorder=3)
    ax.axhline(1, color=MUTED, lw=0.8, ls=(0, (3, 3)), zorder=1)
    for i in range(1, len(shares[filt])):
        ax.axvline(i - 0.5, color=GRID, lw=0.8, zorder=0)
    ax.set_xticks(range(len(shares[filt])))
    ax.set_xticklabels([f"{s * 100:g}%" if KIND == "threshold" else f"k = {s}" for s in shares[filt]], fontsize=9, color=INK)
    ax.set_xlabel("answer share" if KIND == "threshold" else "", fontsize=8.5, color=MUTED)
    ax.set_xlim(-0.5, len(shares[filt]) - 0.5)
    ax.set_title(title, fontsize=10.5, color=INK, loc="left")
    ax.set_yscale("log")
    ax.yaxis.set_major_locator(FixedLocator([0.1, 0.2, 0.5, 1, 2, 5, 10, 20, 50]))
    ax.yaxis.set_minor_locator(NullLocator())
    ax.set_yticklabels(["0.1×", "0.2×", "0.5×", "1×", "2×", "5×", "10×", "20×", "50×"], fontsize=8.5, color=MUTED)
    ax.grid(axis="y", color=GRID, lw=0.8, zorder=0)
    for sp_ in ("top", "right"):
        ax.spines[sp_].set_visible(False)
    for sp_ in ("left", "bottom"):
        ax.spines[sp_].set_color(GRID)
    ax.tick_params(length=0)
axes[0].set_ylabel("speed-up over faiss flat (higher is faster)", fontsize=9, color=INK)
handles = [plt.Line2D([], [], ls="", marker="o", color=c, markersize=7, label=n) for _, _, n, c in SYSTEMS]
handles.append(plt.Line2D([], [], color=MUTED, lw=0.8, ls=(0, (3, 3)), label="faiss flat"))
fig.legend(handles=handles, loc="upper right", ncol=len(handles), frameon=False, fontsize=8.5, bbox_to_anchor=(0.995, 1.0))
fig.suptitle(("Threshold queries" if KIND == "threshold" else "Exact k-NN") + ", averaged over datasets", fontsize=12, color=INK, x=0.01, ha="left", y=0.985)
fig.text(0.01, 0.895, f"Dot: geometric mean over {len(dss)} datasets of the speed-up over faiss flat (same filter" + (" and answer share" if KIND == "threshold" else "") + "); "
         "bar: the range across datasets.  Datasets: " + ", ".join(d.split("-")[0] for d in dss) + ".",
         fontsize=8, color=MUTED)
fig.tight_layout(rect=(0, 0, 1, 0.885))
fig.savefig(out, dpi=150, facecolor="white")
print(out)
