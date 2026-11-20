"""Tables of `study_layered_kernel`'s output (results/kernel_DS_SET.txt): the library's own reader, counts turned into
bytes read with the table of layered.md section 1. usage: python3 kernel_tables.py results/kernel_*.txt

Bytes: K centres (n each) and the sigmas of the clusters read and the 8 ahead (16 n each); a row entering a level
its codes since the level before, with the key (2 B) at the first, a norm code (0.5 B) at a deterministic level, the
scalars (5.5 B, then 2 B) at a randomised one; a stored vector 2 n; a cluster opened on the index path its tables
(96 B per 4-bit unit of each level, for at most as many clusters as rows entered the level: an upper bound); a group
opened its basis product n^2 and, when a randomised level is read, the rotations (4 bytes per entry of the four
blocks' squares).

Modes: `direct`, `certified` and `statistical` as the study ran them (every level; the certified and statistical
totals include the rotations). `statistical, two levels` is the proposal, computed from the statistical line: the rows
that survive the 32 B level are scored from stored vectors (they are the rows entering the next level), no randomised
level is read, so no rotation and two levels' tables.
"""
import json
import os
import re
import sys

import numpy as np

FIT = "/home/user/suite-data/work"
NAMES = {"glove": "glove-100-angular", "yahoo": "yahoo-minilm-384-normalized", "laion": "laion-clip-512-normalized"}
TARGETS = (0.9, 0.95, 0.98, 0.99, 0.995)


def mb(x):
    return f"{x / 1e6:.2f}" if x < 99.5e6 else f"{x / 1e6:.0f}"


def interp(pts, tg):
    pts = sorted(pts)
    rec = [p[0] for p in pts]
    i = int(np.searchsorted(rec, tg - 1e-12))
    if i >= len(pts):
        return None
    if i == 0:
        return pts[0][1]
    f = (tg - rec[i - 1]) / max(rec[i] - rec[i - 1], 1e-12)
    return pts[i - 1][1] + f * (pts[i][1] - pts[i - 1][1])


def main():
    summary = []
    for path in sys.argv[1:]:
        ds, st = os.path.basename(path)[len("kernel_"):-4].split("_", 1)
        meta = json.load(open(f"{FIT}/{NAMES[ds]}/vexact/index/meta.json"))
        n, K, bounds = meta["n"], meta["K"], meta["BOUNDS"]
        rot = 4 * sum((b - a) ** 2 for a, b in zip(bounds, bounds[1:]))
        lines = open(path).read().splitlines()
        lb = json.loads(re.search(r"levels end at (\[[^\]]*\])", lines[0]).group(1))
        ndl = 2
        per_row = [lb[l] - (lb[l - 1] if l else 0) + (2 if l == 0 else 0) + (0.5 if l < ndl else 5.5 if l == ndl else 2) for l in range(len(lb))]
        print(f"\n### {ds}, visible set {st} (the library's reader; n {n}, K {K}; levels end at {lb} B)\n")
        for ln in lines[1:]:
            if ln.startswith("  level"):
                print(ln.strip() + "  ")
        print("\n| psi | mode | recall | clusters read | groups opened | rows entering each level | stored vectors | MB: codes / vectors / group setups / tables | MB total |")
        print("|---|---|---|---|---|---|---|---|---|")
        curves = {}
        for ln in lines:
            m = re.match(r"psi (\S+) (\S+)\s+recall (\S+) \| (\S+) (\S+) (\S+) \| ([\d ]+) \| (\d+)", ln)
            if not m:
                continue
            psi, mode, rec = float(m.group(1)), m.group(2), float(m.group(3))
            read, opened, groups = float(m.group(4)), float(m.group(5)), float(m.group(6))
            enter = [float(x) for x in m.group(7).split()]
            full = float(m.group(8))
            fixed = K * n + (read + 8) * 16 * n
            units = [2 * (lb[l] - (lb[l - 1] if l else 0)) for l in range(len(lb))]

            def emit(mode, enter, full, rotations):
                code = sum(e * b for e, b in zip(enter, per_row))
                vec = full * 2 * n
                tab = sum(min(opened, e) * u * 96 for e, u in zip(enter, units))
                grp = groups * (n * n + (rot if rotations else 0))
                tot = fixed + code + vec + grp + tab
                curves.setdefault(mode, []).append((rec, tot))
                print(f"| {psi:g} | {mode} | {rec:.4f} | {read:.1f} | {groups:.1f} | {' / '.join(f'{e:,.0f}' for e in enter)} | {full:,.0f} | {mb(code)} / {mb(vec)} / {mb(grp)} / {mb(tab)} | {mb(tot)} |")

            emit(mode, enter, full, mode != "direct")
            if mode == "statistical":
                emit("statistical, two levels", enter[:ndl], enter[ndl], False)
        row = [f"{ds} | {st}"]
        for tg in TARGETS:
            v = {m: interp(c, tg) for m, c in curves.items()}
            two = v.get("statistical, two levels")
            if v.get("direct") is None or two is None:
                row.append("-")
            else:
                row.append(f"{mb(v['direct'])} / {mb(v['certified']) if v.get('certified') else '-'} / {mb(two)} ({v['direct'] / two:.1f}x)")
        summary.append("| " + " | ".join(row) + " |")
    print("\n### Bytes to a recall, the library's reader (interpolated along psi): direct / certified, every level / statistical, two levels (direct over statistical)\n")
    print("| dataset | visible set | " + " | ".join(f"recall {t}" for t in TARGETS) + " |")
    print("|---|---|" + "---|" * len(TARGETS))
    print("\n".join(summary))


if __name__ == "__main__":
    main()
