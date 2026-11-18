# Time and rows to reach a recall, per order, from ab_orders.py's output: each order's (time, recall) and (rows,
# recall) points over nprobe, interpolated linearly; "-" where the largest nprobe run does not reach it.
# usage: python3 -I ab_curves.py AB_OUTPUT.txt [--targets 0.8,0.9,0.95]
import argparse
import re
from collections import defaultdict

ap = argparse.ArgumentParser()
ap.add_argument("path")
ap.add_argument("--targets", default="0.8,0.9,0.95")
a = ap.parse_args()
targets = [float(x) for x in a.targets.split(",")]

cells = defaultdict(lambda: defaultdict(list))  # (dataset, filter) -> order -> [(nprobe, recall, rows, us)]
ds = None
for line in open(a.path):
    m = re.match(r"^(\S+): \d+ clusters", line)
    if m:
        ds = m.group(1)
        continue
    m = re.match(r"^(\S+)\s+nprobe\s+(\d+)\s+(.*)$", line)
    if not m:
        continue
    f, npb = m.group(1), int(m.group(2))
    for o, rec, rows, us in re.findall(r"(\S+): ([\d.]+)\s+(\d+) rows\s+([\d.]+) us", m.group(3)):
        cells[(ds, f)][o].append((npb, float(rec), float(rows), float(us)))


def reach(pts, target, col):
    prev = (0.0, 0.0)
    for p in sorted(pts):
        if p[1] >= target:
            r0, x0 = prev
            return p[col] if p[1] == r0 else x0 + (p[col] - x0) * (target - r0) / (p[1] - r0)
        prev = (p[1], p[col])
    return None


fmt = lambda v: "-" if v is None else f"{v:.0f}"
for (d, f), orders in cells.items():
    names = list(orders)
    print(f"{d}, {f}: us / rows to reach recall ({' / '.join(names)})")
    for t in targets:
        us = [reach(orders[o], t, 3) for o in names]
        rows = [reach(orders[o], t, 2) for o in names]
        rel = ["" if us[0] is None or u is None else f" x{u / us[0]:.2f}" for u in us]
        print(f"  {t}: " + "   ".join(f"{fmt(u)} us{r} / {fmt(w)} rows" for u, r, w in zip(us, rel, rows)))
