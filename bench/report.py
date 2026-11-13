# Tables and plots from the scored runs: per dataset, the best QPS of each method at recall targets (Pareto front of
# its grid), the exact rows, the filtered rows, and a recall/QPS plot.
# usage: python3 -I report.py OUT_DIR NAME:RESULTS_DIR:BASELINES_DIR ...
#   reads RESULTS_DIR/ours_scored.csv and BASELINES_DIR/baselines.csv; writes OUT_DIR/<name>.csv, <name>.png, tables.md
import csv, os, sys
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

TARGETS = [0.9, 0.95, 0.99]
COLOURS = {"ours approx": "#2a78d6", "hnswlib": "#eb6834", "faiss-hnsw": "#1baf7a", "faiss-ivfpqfs": "#eda100"}


def rows_of(rdir, bdir):
    out = []
    for r in csv.DictReader(open(os.path.join(rdir, "ours_scored.csv"))):
        out.append(dict(method="ours " + r["method"], build="-", params=r["params"], filter=r["filter"], qps=float(r["qps"]),
                        recall=float(r["recall"]), mean_ms=float(r["mean_ms"]), wrong_sets=r["wrong_sets"], wrong_scores=r["wrong_scores"]))
    for r in csv.DictReader(open(os.path.join(bdir, "baselines.csv"))):
        out.append(dict(method=r["method"], build=r["build"], params=r["params"], filter=r["filter"], qps=float(r["qps"]),
                        recall=float(r["recall"]), mean_ms=float(r["mean_ms"]), wrong_sets="", wrong_scores=""))
    return out


def front(rs):
    """Pareto front: best QPS at each recall, descending recall."""
    rs = sorted(rs, key=lambda r: (-r["recall"], -r["qps"]))
    out, best = [], 0.0
    for r in rs:
        if r["qps"] > best:
            out.append(r)
            best = r["qps"]
    return out


def at(rs, t):
    ok = [r for r in rs if r["recall"] >= t]
    return max(ok, key=lambda r: r["qps"]) if ok else None


def fmt(r):
    if r is None:
        return "–"
    cfg = " ".join(x for x in (r["build"] if r["build"] != "-" else "", r["params"] if r["params"] != "-" else "") if x)
    return f"{r['qps']:,.0f} ({r['recall']:.3f}; {cfg})" if cfg else f"{r['qps']:,.0f} ({r['recall']:.3f})"


def main():
    out = sys.argv[1]
    md = []
    for spec in sys.argv[2:]:
        name, rdir, bdir = spec.split(":")
        rows = rows_of(rdir, bdir)
        with open(os.path.join(out, f"{name}.csv"), "w", newline="") as f:
            w = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
            w.writeheader()
            w.writerows(rows)
        md.append(f"### {name}\n")
        md.append("Exact (all test queries, one thread):\n")
        md.append("| Method | Filter | QPS | Mean ms | Recall@10 | Sets differing from brute force | Score lists differing |")
        md.append("|---|---|---|---|---|---|---|")
        for r in rows:
            if r["method"] in ("ours exact", "ours flat"):
                md.append(f"| {r['method']} | {r['filter']} | {r['qps']:,.1f} | {r['mean_ms']:.2f} | {r['recall']:.4f} | {r['wrong_sets']} | {r['wrong_scores']} |")
        for ftag in ["all"] + sorted({r["filter"] for r in rows} - {"all"}, key=lambda s: -float(s.split("_")[1])):
            sub = [r for r in rows if r["filter"] == ftag and r["method"] not in ("ours exact", "ours flat")]
            if not sub:
                continue
            methods = sorted({r["method"] for r in sub})
            md.append(f"\nBest QPS at recall@10 at least the target, filter `{ftag}` (QPS (recall; configuration)):\n")
            md.append("| Method | " + " | ".join(f"≥ {t}" for t in TARGETS) + " | Highest recall |")
            md.append("|---|" + "---|" * (len(TARGETS) + 1))
            for m in methods:
                rs = [r for r in sub if r["method"] == m]
                top = max(rs, key=lambda r: (r["recall"], r["qps"]))
                md.append(f"| {m} | " + " | ".join(fmt(at(rs, t)) for t in TARGETS) + f" | {fmt(top)} |")
        md.append("")
        # plot: unfiltered fronts and the exact points
        fig, ax = plt.subplots(figsize=(7.5, 4.8), dpi=130)
        for m, col in COLOURS.items():
            fr = front([r for r in rows if r["method"] == m and r["filter"] == "all"])
            if fr:
                ax.plot([r["recall"] for r in fr], [r["qps"] for r in fr], "-o", color=col, lw=2, ms=4, label=m)
        for m, mk in (("ours exact", "*"), ("ours flat", "s")):
            ex = [r for r in rows if r["method"] == m and r["filter"] == "all"]
            if ex:
                ax.plot([ex[0]["recall"]], [ex[0]["qps"]], mk, color="#444441", ms=11 if mk == "*" else 7, label=m, ls="none")
        ax.set_yscale("log")
        ax.set_xlim(left=max(0.0, min(r["recall"] for r in rows if r["filter"] == "all") - 0.02), right=1.005)
        ax.set_xlabel("Recall@10 (ANN-Benchmarks rule)")
        ax.set_ylabel("Queries per second, one thread (log)")
        ax.set_title(f"{name}: unfiltered, k = 10")
        ax.grid(True, which="major", color="#e4e3dd", lw=0.8)
        for s in ("top", "right"):
            ax.spines[s].set_visible(False)
        ax.legend(frameon=False, fontsize=8, loc="lower left")
        fig.tight_layout()
        fig.savefig(os.path.join(out, f"{name}.png"))
    open(os.path.join(out, "tables.md"), "w").write("\n".join(md) + "\n")
    print("\n".join(md))


if __name__ == "__main__":
    main()
