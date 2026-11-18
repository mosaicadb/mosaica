# Time and rows to reach a recall, per arm, from ab_orders.py's output: each arm's (time, recall) and (rows, recall)
# points over its grid (nprobe, or the process's dial), taken in order of rows read and interpolated linearly; "-"
# where the grid's largest point does not reach it. Reads both the current output (a line per filter, arm and grid
# point) and the earlier one (a line per filter and nprobe holding every order).
# usage: python3 -I ab_curves.py AB_OUTPUT.txt [--targets 0.8,0.9,0.95]
import argparse
import re
from collections import defaultdict

ap = argparse.ArgumentParser()
ap.add_argument("path")
ap.add_argument("--targets", default="0.8,0.9,0.95")
a = ap.parse_args()
targets = [float(x) for x in a.targets.split(",")]

cells = defaultdict(lambda: defaultdict(list))  # (dataset, filter) -> arm -> [(rows, recall, rows, us)]
ds = None
for line in open(a.path):
    m = re.match(r"^(\S+): \d+ clusters", line)
    if m:
        ds = m.group(1)
        continue
    m = re.match(r"^(\S+)\s+(\S+)\s+\S+=\S+:\s+([\d.]+)\s+(\d+) rows\s+([\d.]+) us", line)
    if m:
        f, arm, rec, rows, us = m.groups()
        cells[(ds, f)][arm].append((float(rows), float(rec), float(rows), float(us)))
        continue
    m = re.match(r"^(\S+)\s+nprobe\s+(\d+)\s+(.*)$", line)
    if not m:
        continue
    f = m.group(1)
    for o, rec, rows, us in re.findall(r"(\S+): ([\d.]+)\s+(\d+) rows\s+([\d.]+) us", m.group(3)):
        cells[(ds, f)][o].append((float(rows), float(rec), float(rows), float(us)))


def reach(pts, target, col):
    prev = (0.0, 0.0)
    for p in sorted(pts):
        if p[1] >= target:
            r0, x0 = prev
            return p[col] if p[1] == r0 else x0 + (p[col] - x0) * (target - r0) / (p[1] - r0)
        prev = (p[1], p[col])
    return None


fmt = lambda v: "-" if v is None else f"{v:.0f}"
for (d, f), arms in cells.items():
    names = list(arms)
    print(f"{d}, {f}: us / rows to reach recall ({' / '.join(names)})")
    for t in targets:
        us = [reach(arms[o], t, 3) for o in names]
        rows = [reach(arms[o], t, 2) for o in names]
        rel = ["" if us[0] is None or u is None else f" x{u / us[0]:.2f}" for u in us]
        print(f"  {t}: " + "   ".join(f"{fmt(u)} us{r} / {fmt(w)} rows" for u, r, w in zip(us, rel, rows)))
