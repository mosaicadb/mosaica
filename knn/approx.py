# Approximate mode = stage 1 alone (probe c clusters in order "ub64qm", funnel 16-byte level -> deeper level -> full vectors).
# usage: python3 approx.py NAME
import numpy as np, sys
from kcommon import *
NAME = sys.argv[1]; C = Ctx(NAME, need_rand=False); Sd = np.load(workdir(NAME) + "seed.npz"); CG = Sd["CG"]; OI = 1; n, K, N = C.n, C.K, C.N
FN = ["all", "rand50", "rand10", "rand1", "rand0.1", "clusters10", "idrange10"]; SBS = Sd["SBS"]
print(f"{NAME}: funnel levels at {SBS.tolist()} cumulative bytes per item; candidates kept 20k+200 then 2k+8; fixed per query: table row {4*K} B + 64 centres {64*(n+4)} B")
for cl in np.unique(C.qcls):
    m = C.qcls == cl
    print(f"\n| {cl} | k | filter | " + " | ".join(f"c={c}" for c in CG) + " |"); print("|---|---|---|" + "---|" * len(CG))
    for i, (k, f) in enumerate(Sd["CFG"]):
        by = 4 * K + 64 * (n + 4) + Sd["NV"][i, OI] * SBS[0] + Sd["NM1"][i, OI] * (SBS[1] - SBS[0]) + Sd["NFULL"][i, OI] * 2 * n
        cells = [f"{Sd['REC'][i, OI, g, m].mean():.3f} / {by[g, m].mean() / 1e3:.0f} KB / {Sd['NV'][i, OI, g, m].mean():.0f} / {Sd['NGR'][i, OI, g, m].mean():.1f}" for g in range(len(CG))]
        print(f"| recall / bytes / items scanned / bases | {k} | {FN[f]} | " + " | ".join(cells) + " |")
