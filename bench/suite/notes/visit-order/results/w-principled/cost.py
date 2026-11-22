# cost.py DATASET OUT...: modelled time per query (us) from study_w_principled's counts, and the modelled time to
# recall 0.9 / 0.95 / 0.97 / 0.99 per (set, rule), interpolated linearly in psi between the grid's points. The model
# (unit costs per dataset, from the phase profiles; see summary.txt):
#   T = PLAN + c_vis read + sig spreads + c_open opened + c_byte sum_l enter_l bytes_l + c_full stored + c_grp groups
import re
import sys

import numpy as np

BYTES = {"inat": [16, 16, 22, 34, 65, 85, 163, 260], "laion": [16, 16, 24, 38, 60, 102],
         "yahoo": [16, 16, 24, 40, 64, 96], "arxiv": [16, 16, 24, 38, 64, 95]}
# PLAN, c_vis, sig, c_open, c_byte, c_full, c_grp
UNIT = {"inat": (30, 2.6, 2.2, 14.8, 1.19e-4, 0.119, 620),
        "laion": (16, 1.68, 0.85, 27.8, 1.55e-4, 0.061, 34.4),
        "yahoo": (117, 1.5, 0.5, 13.5, 1.55e-4, 0.034, 19),
        "arxiv": (88, 1.68, 1.11, 8.2, 1.55e-4, 0.092, 82.7)}


def parse(path):
    rows = []
    for line in open(path):
        m = re.match(r"(\S+)\s+(\S+)\s+psi=(\S+): recall (\S+) visited (\S+) direct (\S+) index \S+ opened (\S+) groups (\S+) spreads (\S+) .* stored (\S+) .* past_det (\S+) enter (\S+) \|", line)
        if m:
            s, r, psi, rec, vis, dr, op, gr, sp, sto, past, enter = m.groups()
            rows.append(dict(set=s, rule=r, psi=float(psi), recall=float(rec), read=float(vis), direct=float(dr), opened=float(op),
                             groups=float(gr), spreads=float(sp), stored=float(sto), enter=[float(x) for x in enter.split("/")]))
    return rows


def model(ds, r, unit):
    plan, c_vis, sig, c_open, c_byte, c_full, c_grp = unit
    b = BYTES[ds]
    lv = sum(e * b[l] for l, e in enumerate(r["enter"][:len(b)]))
    return plan + c_vis * r["read"] + sig * r["spreads"] + c_open * r["opened"] + c_byte * lv + c_full * r["stored"] + c_grp * r["groups"]


def at_recall(pts, target):
    pts = sorted(pts)
    for (p0, r0, t0), (p1, r1, t1) in zip(pts, pts[1:]):
        if r0 < target <= r1:
            f = (target - r0) / (r1 - r0)
            return t0 + f * (t1 - t0), p0 + f * (p1 - p0)
    return None


if __name__ == "__main__":
    ds = sys.argv[1]
    unit = UNIT[ds]
    if len(sys.argv) > 2 and sys.argv[2].startswith("unit="):
        unit = tuple(float(x) for x in sys.argv[2][5:].split(","))
        paths = sys.argv[3:]
    else:
        paths = sys.argv[2:]
    rows = [r for p in paths for r in parse(p)]
    targets = (0.9, 0.95, 0.97, 0.99)
    for st in sorted({r["set"] for r in rows}):
        print(f"{ds} {st}: modelled us to recall {' / '.join(map(str, targets))} (psi there); and the ratio to corr")
        res = {}
        for rule in dict.fromkeys(r["rule"] for r in rows if r["set"] == st):
            pts = [(r["psi"], r["recall"], model(ds, r, unit)) for r in rows if r["set"] == st and r["rule"] == rule]
            res[rule] = [at_recall(pts, t) for t in targets]
        for rule, v in res.items():
            cells = ["   -   " if x is None else f"{x[0]:6.0f} ({x[1]:.2f})" for x in v]
            rat = ["  -  " if x is None or c is None else f"{x[0] / c[0]:.3f}" for x, c in zip(v, res.get("corr", v))]
            print(f"  {rule:<14} {' '.join(cells)}   {' '.join(rat)}")
