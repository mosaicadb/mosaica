# The headline tables of subclusters.md from results/*/ab_sub_curves.txt (`../scripts/ab_curves.py` output): per dataset,
# visible set and recall, each arm's time to that recall as a ratio of the adaptive arm's (and its time in us).
# usage: python3 -I summary.py [ARM,...] [BASE_ARM]   (BASE_ARM: adaptive by default)
import glob
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
arms_want = sys.argv[1].split(",") if len(sys.argv) > 1 else None
base_arm = sys.argv[2] if len(sys.argv) > 2 else "adaptive"
for path in sorted(glob.glob(os.path.join(HERE, "results", "*", "ab_sub_curves.txt"))):
    for block in open(path).read().split("\n")[:-1]:
        m = re.match(r"^(\S+), (\S+): us / rows to reach recall \((.*)\)$", block)
        if m:
            ds, filt, names = m.group(1), m.group(2), m.group(3).split(" / ")
            want = arms_want or names
            print(f"\n{ds}, {filt}: time to recall, us, and against {base_arm}")
            print("  recall  " + "  ".join(f"{a:>18}" for a in want))
            continue
        m = re.match(r"^\s+([\d.]+): (.*)$", block)
        if not m:
            continue
        cells = re.findall(r"(-|\d+) us(?: x[\d.]+)? / (-|\d+) rows", m.group(2))
        us = {n: (None if c[0] == "-" else float(c[0])) for n, c in zip(names, cells)}
        base = us.get(base_arm)
        out = []
        for a in want:
            v = us.get(a)
            out.append(f"{'-':>18}" if v is None else f"{v:9.0f} {'' if base is None else f'x{v / base:.2f}':>8}")
        print(f"  {m.group(1):<6}  " + "  ".join(out))
