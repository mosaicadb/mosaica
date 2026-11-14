# Median and 90th percentile over queries of the per-query median latency, by system, threads, visible set and share.
# usage: python3 -I summarise.py OUT.md CSV...
import csv, sys
from collections import defaultdict
import numpy as np

SHARES = ["0.01%", "0.1%", "1%", "10%", "30%", "50%", "75%"]
VIS = ["all", "vis_50", "vis_10", "vis_1"]
d = defaultdict(list)
for f in sys.argv[2:]:
    for r in csv.DictReader(open(f)):
        d[(r["what"], int(r["threads"]), r["vis"], int(r["share_index"]))].append(float(r["median_ms"]))
whats = sorted({k[0] for k in d}, key=lambda w: (not w.startswith(("index", "flat")), w))
lines = []
for th in (1, 4):
    for vis in VIS:
        lines.append(f"\n### {th} thread{'s' if th > 1 else ''}, visible set {vis}: median / p90 ms over queries\n")
        lines.append("| system | " + " | ".join(SHARES) + " |")
        lines.append("|---|" + "---|" * len(SHARES))
        for w in whats:
            cells = []
            for si in range(len(SHARES)):
                v = d.get((w, th, vis, si))
                cells.append(f"{np.median(v):.1f} / {np.quantile(v, 0.9):.1f}" if v else "–")
            if any(c != "–" for c in cells):
                lines.append(f"| {w} | " + " | ".join(cells) + " |")
open(sys.argv[1], "w").write("\n".join(lines).lstrip() + "\n")
print("\n".join(lines))
