"""Median ms per answer share and variant from dbcmp CSVs: python3 table.py runs/dbcmp_TAG.csv [...]"""
import csv, statistics, sys
from collections import defaultdict
SH = ["0.01%", "0.1%", "1%", "10%", "30%", "50%", "75%"]
d = defaultdict(list)
order, vorder = [], []
for f in sys.argv[1:]:
    for r in csv.DictReader(open(f)):
        k = (r["vis"], r["what"], int(r["share_index"]))
        d[k].append(float(r["median_ms"]))
        if r["vis"] not in order: order.append(r["vis"])
        if r["what"] not in vorder: vorder.append(r["what"])
for v in order:
    print(f"\n**{v}**\n\n| variant | " + " | ".join(SH) + " |\n|---|" + "---|" * len(SH))
    for w in vorder:
        if (v, w, 0) not in d: continue
        print(f"| {w} | " + " | ".join(f"{statistics.median(d[(v, w, s)]):.3g}" for s in range(len(SH))) + " |")
