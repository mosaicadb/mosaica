# Index B against index A from ab_indexes.py's outputs: per filter, the approximate k-NN's time to recall over A's
# (the curves as ab_curves.py interpolates them, in order of rows read), and each exact cell's median time over A's.
# usage: python3 -I indexes_table.py AB_INDEXES_OUTPUT.txt... [--targets 0.9,0.95,0.99]
import argparse
import re
from collections import defaultdict

ap = argparse.ArgumentParser()
ap.add_argument("paths", nargs="+")
ap.add_argument("--targets", default="0.9,0.95,0.99")
a = ap.parse_args()
targets = [float(x) for x in a.targets.split(",")]


def reach(pts, t):
    prev = (0.0, 0.0)
    for p in sorted(pts):
        if p[1] >= t:
            r0, x0 = prev
            return p[2] if p[1] == r0 else x0 + (p[2] - x0) * (t - r0) / (p[1] - r0)
        prev = (p[1], p[2])
    return None


for path in a.paths:
    approx, exact = defaultdict(list), {}
    head = ""
    for line in open(path):
        if not head and re.match(r"^\S+: \d+ clusters", line):
            head = line.split(",")[0]
        m = re.match(r"^(\S+)\s+([AB]):(\S+)\s+\S+:\s+([\d.]+)\s+(\d+) rows\s+([\d.]+) us", line)
        if not m:
            continue
        f, ab, arm, rec, rows, us = m.groups()
        if arm == "adaptive":
            approx[(f, ab)].append((float(rows), float(rec), float(us)))
        else:
            exact[(f, ab, arm)] = float(us)
    print(f"## {head}: B over A")
    for f in dict.fromkeys(k[0] for k in approx):
        ra = [reach(approx[(f, "A")], t) for t in targets]
        rb = [reach(approx[(f, "B")], t) for t in targets]
        ap_ = "/".join("-" if x is None or y is None else f"{y / x:.2f}" for x, y in zip(ra, rb))
        ex = " ".join(f"{arm} {exact[(f, 'B', arm)] / exact[(f, 'A', arm)]:.2f}"
                      for (ff, ab, arm) in exact if ff == f and ab == "A" and (f, "B", arm) in exact)
        print(f"{f:<10} approximate {ap_} | {ex}")
