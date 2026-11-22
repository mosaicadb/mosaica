# fitcost.py: the second cost model, unit costs fitted to timed medians. For each dataset, the counts of the as-built
# rule under the timed run's own constants (this study's *-c6f.txt, the reader as 6f86a1c timed it) against that run's
# median query times on core 11 (results/w-rule/ab-knn-*-6f86a1c-w2.txt), psi 2 to 9: non-negative least squares of
# a0 and c_open in T = a0 + c_vis read + sig spreads + c_open opened + c_byte sum_l enter_l bytes_l + c_full stored +
# c_grp groups, the other units as cost.py has them (a free fit of all of them is degenerate: the counts move together).
# Prints the fitted units and the fit's ratios.
import re
import sys

import numpy as np
from scipy.optimize import nnls

sys.argv = ["x"]
exec(open("cost.py").read().split("if __name__")[0])
R = "/home/joe/code/vexact-visitorder/bench/suite/notes/visit-order/results/w-rule/"
SRC = {"inat": ("inat-c6f.txt", "inaturalist-resnet-2048-cosine"), "laion": ("laion-c6f-all.txt", "laion-clip-512-normalized"),
       "yahoo": ("yahoo-c6f.txt", "yahoo-minilm-384-normalized"), "arxiv": ("arxiv-c6f.txt", "arxiv-nomic-768-normalized")}
FIT = {}
for ds, (f, name) in SRC.items():
    t = {}
    for line in open(f"{R}ab-knn-{name}-6f86a1c-w2.txt"):
        m = re.match(r"none\s+adaptive\s+psi=(\S+): \S+\s+\d+ rows\s+(\S+) us", line)
        if m:
            t[float(m.group(1))] = float(m.group(2))
    try:
        rows = [r for r in parse(f) if r["set"] == "none" and r["rule"] == "built" and r["psi"] in t]
    except FileNotFoundError:
        continue
    plan, c_vis, sig, _, c_byte, c_full, c_grp = UNIT[ds]
    b = BYTES[ds]
    fixed = lambda r: c_vis * r["read"] + sig * r["spreads"] + c_byte * sum(e * b[l] for l, e in enumerate(r["enter"][:len(b)])) + c_full * r["stored"] + c_grp * r["groups"]
    A = np.array([[1, r["opened"]] for r in rows])
    y = np.array([t[r["psi"]] - fixed(r) for r in rows])
    x, _ = nnls(A, y)
    FIT[ds] = (x[0], c_vis, sig, x[1], c_byte, c_full, c_grp)
    print(f"{ds}: a0 {x[0]:.1f} c_open {x[1]:.2f}; fit / timed "
          + " ".join(f"{r['psi']:g}:{(A[i] @ x + fixed(r)) / t[r['psi']]:.2f}" for i, r in enumerate(rows)))

if __name__ == "__main__":
    print("units:", {k: tuple(round(x, 6) for x in v) for k, v in FIT.items()})
