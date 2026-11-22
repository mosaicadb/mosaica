# ratios.py: the summary's table, modelled time to recall 0.9 / 0.95 / 0.97 / 0.99 as a share of corrected's, per
# dataset and visible set, for the rules named (from fine-<ds>.txt and, where present, cross-<ds>.txt)
import os
import sys

sys.argv = ["x"]
exec(open("cost.py").read().split("if __name__")[0])
RULES = ["built", "height", "tail", "corrx1.2", "cross"]
T = (0.9, 0.95, 0.97, 0.99)
print(f"{'dataset / filter':<24}" + "".join(f"{r:<25}" for r in RULES))
for ds in ("inat", "laion", "arxiv", "yahoo"):
    rows = parse(f"fine-{ds}.txt") + (parse(f"cross-{ds}.txt") if os.path.exists(f"cross-{ds}.txt") else [])
    for st in ("none", "region-25"):
        res = {}
        for rule in ["corr"] + RULES:
            pts = [(r["psi"], r["recall"], model(ds, r, UNIT[ds])) for r in rows if r["set"] == st and r["rule"] == rule]
            res[rule] = [at_recall(pts, t) if pts else None for t in T]
        cells = []
        for rule in RULES:
            v = ["-" if x is None or c is None else f"{x[0] / c[0]:.2f}" for x, c in zip(res[rule], res["corr"])]
            cells.append(f"{'/'.join(v):<25}")
        print(f"{ds + ' ' + st:<24}" + "".join(cells))
