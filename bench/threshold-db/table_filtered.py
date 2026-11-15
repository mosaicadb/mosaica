# Markdown tables of the filtered sweep from RESULTS_DIR/filtered/plot_data.csv (written by plot_filtered.py): one row
# per system and visible set, the median ms per query at each answer share, with the mean recall where it is below 0.99.
# usage: python3 -I table_filtered.py RESULTS_DIR
import csv, sys
from collections import defaultdict

res = sys.argv[1]
SYS = ["LIMINAL", "LIMINAL, compacted", "LIMINAL f16 scan", "faiss flat, compacted copy", "LIMINAL approximate", "faiss IVF4096-SQfp16",
       "faiss HNSW-SQfp16", "ACORN-γ", "ACORN-γ, ef = m"]
cell, size = {}, defaultdict(dict)
for r in csv.DictReader(open(f"{res}/filtered/plot_data.csv")):
    ms, rec = float(r["median_ms"]), float(r["mean_recall"])
    t = f"{ms:.2f}" if ms < 10 else f"{ms:.1f}" if ms < 100 else f"{ms:.0f}"
    cell[(r["system"], r["vis"], int(r["share_index"]))] = t + (f" r .{round(rec * 1000):03d}" if rec < 0.99 else "")
    if r["system"] == "LIMINAL":
        size[r["vis"]][int(r["share_index"])] = int(r["answer_size_median"])
for vis in ("vis_1", "vis_10", "vis_50", "gen_1", "gen_10", "gen_50"):
    print(f"\n**{vis}**, median answer size " + " / ".join(f"{size[vis].get(si, 0):,}" for si in range(7)) + "\n")
    print("| system | 0.01% | 0.1% | 1% | 10% | 30% | 50% | 75% |\n|---|---|---|---|---|---|---|---|")
    for s in SYS:
        cells = [cell.get((s, vis, si), "–") for si in range(7)]
        if any(c != "–" for c in cells):
            print(f"| {s} | " + " | ".join(cells) + " |")
