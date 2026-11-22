"""Approximate k-NN: one panel per filter, each system's speed-up over a flat scan, averaged over datasets.

Dot: geometric mean over the datasets where the system reaches the recall. Bar: the range over those datasets.
A hollow dot means the system reached that recall on only some datasets (the count is printed).
Usage: python3 bench/suite/plot_avg_approx.py <results/compare dir> <out png>
"""
import csv, collections, statistics as st, sys, pathlib, math
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.ticker import FixedLocator, NullLocator

root, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
dss = sorted(d.name for d in root.iterdir() if (d / "check.csv").exists() and (d / "vexact.csv").exists())
TARGETS = (0.8, 0.9, 0.95, 0.99)
FILTERS = [("none", "Unfiltered"), ("random-25", "random-25"), ("region-25", "region-25"),
           ("random-1", "random-1"), ("region-1", "region-1")]
SYSTEMS = [("vexact", "LIMINAL", "#2a78d6"), ("faiss-hnsw", "faiss HNSW", "#eb6834"),
           ("acorn", "ACORN-γ", "#1baf7a"), ("faiss-ivf-panorama", "faiss IVF-Panorama", "#eda100"),
           ("hnswlib", "hnswlib", "#e87ba4"), ("glass", "glass", "#008300"),
           ("pynndescent", "pynndescent", "#6250d6"), ("faiss-ivfpqfs", "faiss IVF-PQ fast scan", "#e34948")]
INK, MUTED, GRID = "#1f1f1e", "#6b6a64", "#e4e3de"

def read(ds, sysn):
    p = root / ds / f"{sysn}.csv"
    return list(csv.DictReader(open(p))) if p.exists() else []

def curves(ds, sysn):
    g = collections.defaultdict(lambda: ([], []))
    for r in read(ds, sysn):
        if r["type"] == "approx":
            k = (r["filter"], r["setting"], r["param"])
            g[k][0].append(float(r["median_ms"])); g[k][1].append(float(r["recall"]))
    c = collections.defaultdict(list)
    for (f, _, _), (ms, rc) in g.items():
        c[f].append((st.mean(rc), st.median(ms)))
    return c

def flat_ms(ds):
    g = collections.defaultdict(list)
    for r in read(ds, "faiss-flat"):
        if r["type"] == "exact" and "compact" not in r["setting"]:
            g[r["filter"]].append(float(r["median_ms"]))
    return {f: st.median(v) for f, v in g.items()}

def cost_at(pts, t):
    x = [ms for rc, ms in pts if rc >= t]
    return min(x) if x else None

flat = {ds: flat_ms(ds) for ds in dss}
cv = {(ds, s): curves(ds, s) for ds in dss for s, *_ in SYSTEMS}

fig, axes = plt.subplots(1, len(FILTERS), figsize=(4.0 * len(FILTERS), 5.2), sharey=True)
used = []
for ax, (filt, title) in zip(axes, FILTERS):
    present = [s for s in SYSTEMS if any(filt in cv[(ds, s[0])] for ds in dss)]
    w = 0.8 / len(present)
    for j, (sysn, name, col) in enumerate(present):
        for i, t in enumerate(TARGETS):
            ran = [ds for ds in dss if filt in cv[(ds, sysn)]]
            sp = []
            for ds in ran:
                c = cost_at(cv[(ds, sysn)][filt], t)
                if c is not None and filt in flat[ds]:
                    sp.append(flat[ds][filt] / c)
            if not sp:
                continue
            x = i - 0.4 + w * (j + 0.5)
            gm = math.exp(sum(map(math.log, sp)) / len(sp))
            full = len(sp) == len(ran)
            lw = 2.4 if sysn == "vexact" else 1.6
            ax.plot([x, x], [min(sp), max(sp)], color=col, lw=lw, alpha=0.55, solid_capstyle="round", zorder=2)
            ax.scatter(x, gm, s=60 if sysn == "vexact" else 40, zorder=3, lw=1.4,
                       facecolor=col if full else "white", edgecolor=col if not full else "white")
            if not full:
                ax.text(x, gm * 0.8, f"{len(sp)}/{len(ran)}", fontsize=6.5, color=MUTED, ha="center", va="top")
        if name not in [u[0] for u in used]:
            used.append((name, col))
    ax.axhline(1, color=MUTED, lw=0.8, ls=(0, (3, 3)), zorder=1)
    for i in range(1, len(TARGETS)):
        ax.axvline(i - 0.5, color=GRID, lw=0.8, zorder=0)
    ax.set_xticks(range(len(TARGETS)))
    ax.set_xticklabels([f"{t}" for t in TARGETS], fontsize=9, color=INK)
    ax.set_xlabel("recall@10", fontsize=8.5, color=MUTED)
    ax.set_xlim(-0.5, len(TARGETS) - 0.5)
    ax.set_title(title, fontsize=10.5, color=INK, loc="left")
    ax.set_yscale("log")
    ax.yaxis.set_major_locator(FixedLocator([0.3, 1, 3, 10, 30, 100, 300, 1000]))
    ax.yaxis.set_minor_locator(NullLocator())
    ax.set_yticklabels(["0.3×", "1×", "3×", "10×", "30×", "100×", "300×", "1000×"], fontsize=8.5, color=MUTED)
    ax.grid(axis="y", color=GRID, lw=0.8, zorder=0)
    for sp_ in ("top", "right"):
        ax.spines[sp_].set_visible(False)
    for sp_ in ("left", "bottom"):
        ax.spines[sp_].set_color(GRID)
    ax.tick_params(length=0)
axes[0].set_ylabel("speed-up over a flat scan (higher is faster)", fontsize=9, color=INK)
handles = [plt.Line2D([], [], ls="", marker="o", color=c, markersize=7, label=n) for n, c in used]
fig.legend(handles=handles, loc="upper right", ncol=len(handles), frameon=False, fontsize=8.5, bbox_to_anchor=(0.995, 1.0))
fig.suptitle("Approximate k-NN, averaged over datasets", fontsize=12, color=INK, x=0.01, ha="left", y=0.985)
fig.text(0.01, 0.905, f"Dot: geometric mean over {len(dss)} datasets of the speed-up over faiss flat (same filter); bar: the range "
         "across datasets. Hollow: reached on only some datasets (count shown); the mean covers those.  "
         "Datasets: " + ", ".join(d.split("-")[0] for d in dss) + ".", fontsize=8, color=MUTED)
fig.tight_layout(rect=(0, 0, 1, 0.9))
fig.savefig(out, dpi=150, facecolor="white")
print(out)
