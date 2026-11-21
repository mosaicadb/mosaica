# Time to recall across builds of the library, from ab_orders.py's outputs named ab-knn-DATASET-BUILD.txt and
# ab-thr-DATASET-BUILD.txt in one directory (each build's run with its own legacy and exact arms, run back to back).
# k-NN: per dataset and filter, each build's adaptive time to each recall over its own run's spread time, then the last
# build's time over each earlier build's (absolute, across the back-to-back runs). Threshold: per dataset, filter and
# answer share, each build's adaptive time to each recall over its own run's exact query time.
# The curves are ab_curves.py's: points taken in order of rows read, interpolated linearly; "-" where the grid's
# largest point does not reach the recall.
# usage: python3 -I builds_table.py DIR BUILD1,BUILD2,... [--targets 0.8,0.9,0.95,0.98,0.99,0.995] [--datasets a,b]
import argparse
import os
import re
from collections import defaultdict

ap = argparse.ArgumentParser()
ap.add_argument("dir")
ap.add_argument("builds")
ap.add_argument("--targets", default="0.8,0.9,0.95,0.98,0.99,0.995")
ap.add_argument("--datasets", default="")
a = ap.parse_args()
builds = a.builds.split(",")
targets = [float(x) for x in a.targets.split(",")]


def parse(path):
    """(filter, arm) -> [(rows, recall, us, point)]"""
    cells = defaultdict(list)
    for line in open(path):
        m = re.match(r"^(\S+)\s+(\S+)\s+(\S+)=(\S+):\s+([\d.]+)\s+(\d+) rows\s+([\d.]+) us", line)
        if m:
            f, arm, _, pt, rec, rows, us = m.groups()
            cells[(f, arm)].append((float(rows), float(rec), float(us), pt))
    return cells


def reach(pts, target):
    pts = [p for p in pts if p[3] != "inf"]
    prev = (0.0, 0.0)
    for p in sorted(pts):
        if p[1] >= target:
            r0, x0 = prev
            return p[2] if p[1] == r0 else x0 + (p[2] - x0) * (target - r0) / (p[1] - r0)
        prev = (p[1], p[2])
    return None


def fmt(v):
    return "-" if v is None else f"{v:.2f}"


def ratio(x, y):
    return None if x is None or y is None or y == 0 else x / y


names = sorted({re.match(r"ab-(knn|thr)-(.+)-([0-9a-f]+)\.txt$", f).group(2) for f in os.listdir(a.dir)
                if re.match(r"ab-(knn|thr)-(.+)-([0-9a-f]+)\.txt$", f)})
if a.datasets:
    names = [n for n in a.datasets.split(",") if n in names]
tl = "/".join(f"{t:g}" for t in targets)
print(f"== k-NN: adaptive time to recall {tl} over spread's, per build; then {builds[-1]} over each earlier build")
for ds in names:
    runs = {b: parse(os.path.join(a.dir, f"ab-knn-{ds}-{b}.txt")) for b in builds
            if os.path.exists(os.path.join(a.dir, f"ab-knn-{ds}-{b}.txt"))}
    if not runs:
        continue
    print(f"## {ds}")
    filters = list(dict.fromkeys(f for r in runs.values() for (f, _) in r))
    for f in filters:
        cols = []
        ad = {}
        for b, r in runs.items():
            sp = [reach(r[(f, "spread")], t) for t in targets]
            ad[b] = [reach(r[(f, "adaptive")], t) for t in targets]
            cols.append(f"{b} " + "/".join(fmt(ratio(x, y)) for x, y in zip(ad[b], sp)))
        last = builds[-1]
        vs = [f"/{b} " + "/".join(fmt(ratio(x, y)) for x, y in zip(ad[last], ad[b])) for b in runs if b != last and last in ad]
        print(f"{f:<10} | " + " | ".join(cols) + " || " + " | ".join(vs))
print()
print(f"== threshold: adaptive time to recall {tl} over the exact query's, per build; then {builds[-1]} over each earlier build")
for ds in names:
    runs = {b: parse(os.path.join(a.dir, f"ab-thr-{ds}-{b}.txt")) for b in builds
            if os.path.exists(os.path.join(a.dir, f"ab-thr-{ds}-{b}.txt"))}
    if not runs:
        continue
    print(f"## {ds}")
    keys = list(dict.fromkeys((f, arm.split("@")[1]) for r in runs.values() for (f, arm) in r if arm.startswith("adaptive@")))
    for f, share in keys:
        cols = []
        ad = {}
        for b, r in runs.items():
            ex = r.get((f, f"exact@{share}"))
            ext = None if not ex else ex[0][2]
            ad[b] = [reach(r[(f, f"adaptive@{share}")], t) for t in targets]
            cols.append(f"{b} " + "/".join(fmt(ratio(x, ext)) for x in ad[b]))
        last = builds[-1]
        vs = [f"/{b} " + "/".join(fmt(ratio(x, y)) for x, y in zip(ad[last], ad[b])) for b in runs if b != last and last in ad]
        print(f"{f + ' ' + share:<16} | " + " | ".join(cols) + " || " + " | ".join(vs))
