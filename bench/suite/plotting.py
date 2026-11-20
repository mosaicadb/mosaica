# The report's figures, in the style of the earlier threshold-db and topk plots: one colour per system, exact methods
# solid and approximate methods dashed (at their cheapest setting reaching a recall target; hollow points where no
# setting reached it, labelled with the recall reached), names at the line ends instead of a legend, a title and a
# subtitle that say what is measured.
import numpy as np

INK, MUTED, GRID, LEADER = "#0b0b0b", "#52514e", "#e4e3df", "#c9c8c2"
COLOURS = {"vexact": "#2a78d6", "faiss-flat": "#1baf7a", "faiss-flat-mmap": "#1baf7a", "faiss-panorama": "#6250d6",
           "pdx-bond": "#8c6d31", "faiss-hnsw": "#eb6834", "faiss-ivf-disk": "#eda100", "acorn": "#c2185b",
           "diskann": "#7a7a7a", "pgvector": "#008300"}
NAMES = {"vexact": "LIMINAL", "faiss-flat": "faiss flat", "faiss-flat-mmap": "faiss flat (mapped)",
         "faiss-panorama": "faiss Panorama", "pdx-bond": "PDX-BOND", "faiss-hnsw": "faiss HNSW",
         "faiss-ivf-disk": "faiss IVF (on disk)", "acorn": "ACORN-γ", "diskann": "DiskANN", "pgvector": "pgvector"}


def setup(plt):
    plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 10, "axes.edgecolor": MUTED,
                         "axes.labelcolor": INK, "xtick.color": MUTED, "ytick.color": MUTED,
                         "axes.spines.top": False, "axes.spines.right": False})


def colour(system):
    return COLOURS.get(system, "#999999")


def name(system, setting="", approx=False):
    n = NAMES.get(system, system)
    if setting and setting not in ("default",):
        n += f", {setting.replace('compacted', 'compacted copy')}"
    if approx and system == "vexact":
        n += " approximate"
    return n


def cheapest(points, target):
    """(ms, recall, reached) of the cheapest setting with mean recall >= target, else of the most accurate one."""
    ok = [p for p in points if p[1] >= target]
    if ok:
        ms, r = min(ok)
        return ms, r, True
    ms, r = max(points, key=lambda p: (p[1], -p[0]))
    return ms, r, False


def draw(ax, series, xlabel, title, xticks=None, xticklabels=None):
    """series: [dict(name, colour, exact, pts=[(x, ms, recall or None, reached)])]. Lines, markers, recall labels on
    hollow points, and names at the line ends, nudged apart in log space."""
    ends = []
    for s in series:
        pts = sorted(s["pts"])
        if not pts:
            continue
        xs, ys = [p[0] for p in pts], [p[1] for p in pts]
        main = s["name"] == "LIMINAL"
        ax.plot(xs, ys, linestyle="-" if s["exact"] else (0, (5, 3)), color=s["colour"],
                lw=2.8 if main else 1.8, zorder=4 if s["name"].startswith("LIMINAL") else 3)
        for x, y, r, ok in pts:
            ax.plot([x], [y], "o", ms=5.5, mec=s["colour"], mfc=s["colour"] if ok else "white", mew=1.6, zorder=5)
            if not ok and (x, y) != (xs[-1], ys[-1]):
                ax.annotate(f"recall {r:.2f}", (x, y), xytext=(6, -12), textcoords="offset points", fontsize=8,
                            color=MUTED)
        r_end, ok_end = pts[-1][2], pts[-1][3]
        ends.append([ys[-1], xs[-1], s["name"] + ("" if ok_end else f" (recall {r_end:.2f})")])
    ends.sort()
    ly = [np.log10(e[0]) for e in ends]
    allys = [p[1] for s in series for p in s["pts"] if p[1] > 0]
    span = np.log10(max(allys)) - np.log10(min(allys)) + 0.5 if allys else 1.0
    gap = max(0.05, 0.055 * span)  # the names' spacing: a fixed share of the panel's height
    for i in range(1, len(ly)):
        ly[i] = max(ly[i], ly[i - 1] + gap)
    for (y, x, label), l in zip(ends, ly):
        ax.annotate(label, (x, y), xytext=(1.03, 10 ** l), textcoords=("axes fraction", "data"), fontsize=8.5,
                    color=INK, va="center", annotation_clip=False,
                    arrowprops=dict(arrowstyle="-", color=LEADER, lw=0.8, shrinkA=2, shrinkB=4))
    ax.set_xscale("log")
    ax.set_yscale("log")
    if xticks is not None:
        ax.set_xticks(xticks)
        ax.set_xticklabels(xticklabels)
    ax.minorticks_off()
    ys = [p[1] for s in series for p in s["pts"] if p[1] > 0]
    if ys:
        # decade ticks over the data, with half a decade's margin (matplotlib's own choice can leave none)
        lo, hi = np.log10(min(ys)), np.log10(max(ys))
        ax.set_ylim(10 ** (lo - 0.25), 10 ** (hi + 0.25))
        dec = list(range(int(np.floor(lo - 0.25)) + 1, int(np.floor(hi + 0.25)) + 1))
        if len(dec) < 2:
            dec = sorted({int(np.floor(lo)), int(np.ceil(hi))})
            ax.set_ylim(10 ** min(lo - 0.25, dec[0]), 10 ** max(hi + 0.25, dec[-1]))
        ax.set_yticks([10.0 ** d for d in dec])
    ax.yaxis.set_major_formatter(_ms_formatter())
    ax.grid(True, axis="y", color=GRID, lw=0.8)
    ax.set_title(title, loc="left", fontsize=10.5, color=INK, pad=10)
    ax.set_xlabel(xlabel)
    ax.tick_params(length=0)


def _ms_formatter():
    from matplotlib.ticker import FuncFormatter
    return FuncFormatter(lambda v, _: f"{v:g} ms")


LABEL_IN = 1.9  # inches to the right of each panel for the names at the line ends
HEAD_IN = 1.35  # inches above the panels for the title and subtitle


def figure(plt, nrows, ncols, panel_w=3.6, panel_h=3.3):
    """A grid of panels with room to the right of each for its line-end names, and a header for title and subtitle."""
    w = ncols * (panel_w + LABEL_IN) + 0.6
    h = nrows * (panel_h + 0.75) + HEAD_IN
    fig, axes = plt.subplots(nrows, ncols, figsize=(w, h), squeeze=False)
    fig.subplots_adjust(left=0.6 / w, right=1 - LABEL_IN / w, top=1 - HEAD_IN / h, bottom=0.6 / h,
                        wspace=LABEL_IN / panel_w + 0.08, hspace=0.75 / panel_h + 0.15)
    return fig, axes


def finish(fig, plt, path, title, subtitle):
    h = fig.get_size_inches()[1]
    fig.text(0.6 / fig.get_size_inches()[0], 1 - 0.25 / h, title, ha="left", va="top", fontsize=12.5, color=INK,
             fontweight="bold")
    fig.text(0.6 / fig.get_size_inches()[0], 1 - 0.6 / h, subtitle, ha="left", va="top", fontsize=8.8, color=MUTED)
    fig.savefig(path, dpi=130, facecolor="white")
    plt.close(fig)
