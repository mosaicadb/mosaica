# Bytes per item spent at each level (entering share x level size), mean over queries of a class. usage: levels.py NAME TAG CLASS TI
import sys, numpy as np
import report2 as R2
R = R2.load(sys.argv[2]); cl = sys.argv[3]; ti = int(sys.argv[4]); m = R2.qcls == cl
e = R["enter"][:, ti][:, m].mean(1) / R2.N; lv = [int(v) for v in R["ALLV"]] + ["full"]
print(sys.argv[1], sys.argv[2], cl, R2.TH[ti], "entering %:", " ".join(f"{l}:{100 * x:.1f}" for l, x in zip(lv, e)), "| B/item total", round(R["ib"][ti][m].mean() / R2.N, 2))
