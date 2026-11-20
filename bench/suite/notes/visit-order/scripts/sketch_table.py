"""The sketch condition of study_order_stress: each sketch's rows to recall 0.9/0.95/0.99/0.995 over the exact
sigma's, and the relative errors (median/p90). python3 sketch_table.py OUT file [file ...]"""
import re
import sys

T = ["0.9", "0.95", "0.99", "0.995"]
NAMES = ["ours (16 dirs + rest)", "Bruch t=4", "Bruch t=8", "hybrid (16 dirs + diag)"]
out = []
for path in sys.argv[2:]:
    blocks, cur = [], None
    for l in open(path):
        m = re.match(r"== (?:\S*/stress-ix/)?(\S+) (\S+) (\S+) k=(\d+)", l)
        if m:
            cur = None
            if m.group(2).startswith("sketch"):
                cur = dict(name=f"{m.group(1)} {m.group(2)} {m.group(3)} k={m.group(4)}", errs=[], rows={})
                blocks.append(cur)
            continue
        if cur is None:
            continue
        m = re.match(r"  error (.+?)\s+median (\S+) p90 (\S+)", l)
        if m:
            cur["errs"].append((m.group(1).strip(), m.group(2), m.group(3)))
            continue
        m = re.match(r"  mu\+3 sigma (.+?)\s+(0\.9: .*)$", l)
        if m:
            cur["rows"][m.group(1).strip()] = {t: float(r) for t, r in re.findall(r"([0-9.]+): (\d+) rows", m.group(2))}
    for b in blocks:
        ex = b["rows"]["exact"]
        cells = []
        for nm in NAMES:
            r = b["rows"][nm]
            cells.append(nm.split(" (")[0] + " " + "/".join(f"{r[t] / ex[t]:.2f}" for t in T if t in r and t in ex))
        out.append(f"{b['name']:<30} rows / exact's at 0.9/0.95/0.99/0.995: " + "; ".join(cells))
        out.append("    relative error, median/p90: " + "; ".join(f"{a} {m}/{p}" for a, m, p in b["errs"]))
open(sys.argv[1], "w").write("\n".join(out) + "\n")
print("\n".join(out))
