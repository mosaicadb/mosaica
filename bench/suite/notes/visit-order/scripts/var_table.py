"""study_probe_variants' output as tables: per dataset and visible set, each variant's recall / clusters / rows per psi,
and the clusters and rows each needs to reach recall 0.9, 0.95 and 0.99 (interpolated between psi points).
python3 var_table.py OUT file [file ...]"""
import re
import sys

T = [0.9, 0.95, 0.99]
data = {}
for path in sys.argv[2:]:
    for l in open(path):
        m = re.match(r"\S*/stress-ix/(\S+) (\S+) (\S+)\s+psi (\S+): recall (\S+) clusters (\S+) \(batched (\S+)\) rows (\S+)", l)
        if m:
            data.setdefault((m.group(1), m.group(2)), {}).setdefault(m.group(3), []).append(
                (float(m.group(4)), float(m.group(5)), float(m.group(6)), float(m.group(7)), float(m.group(8))))
out = []
for (ds, f), vs in data.items():
    out.append(f"== {ds} {f}: per psi 2-7 recall/clusters(batched)/rows; then clusters and rows to recall " + "/".join(map(str, T)))
    for v, pts in vs.items():
        cells = " ".join(f"{r:.3f}/{c:.0f}({b:.0f})/{w:.0f}" for _, r, c, b, w in pts)
        to = []
        for t in T:
            hit = None
            for a, b in zip(pts, pts[1:]):
                if a[1] < t <= b[1]:
                    x = (t - a[1]) / (b[1] - a[1])
                    hit = (a[2] + x * (b[2] - a[2]), a[4] + x * (b[4] - a[4]))
                    break
            if hit is None and pts[0][1] >= t:
                hit = (pts[0][2], pts[0][4])
            to.append("-" if hit is None else f"{hit[0]:.0f}c {hit[1]:.0f}r")
        out.append(f"   {v:<5} {cells}")
        out.append(f"         to recall: " + "  ".join(to))
open(sys.argv[1], "w").write("\n".join(out) + "\n")
print("\n".join(out))
