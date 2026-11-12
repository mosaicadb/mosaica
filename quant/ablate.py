# Marginal effect of each kept change (B/item at 0.01/0.1/1/5% answers), cumulative rows. usage: ablate.py NAME TAG...
import sys, numpy as np
import report2 as R2
prev = None
for tag in sys.argv[2:]:
    R = R2.load(tag); s = f"{tag:7s}"; cur = {}
    for cl in np.unique(R2.qcls):
        v = np.array([r["item"] for r in R2.rows(R, cl)][:4])
        s += f" | {cl} " + " ".join(f"{x:6.2f}" for x in v)
        if prev is not None: s += " (" + " ".join(f"{100 * (a / b - 1):+.1f}" for a, b in zip(v, prev[cl])) + ")"
        cur[cl] = v
    prev = cur
    print(s, "| viol", int(R["viol"].sum()))
