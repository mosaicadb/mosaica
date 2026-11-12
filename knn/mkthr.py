# Thresholds for the sweep: per config (k, filter) a seed column (tau1 from stage 1) and an oracle column (midpoint of s_k, s_k+1).
# Seed rule: order "ub64qm"; the smallest number of clusters in {8,16,32,64} whose visible items number >= 20k+200.
import numpy as np, sys
from kcommon import workdir
NAME = sys.argv[1]; W_ = workdir(NAME); Sd = np.load(W_ + "seed.npz"); CFG = Sd["CFG"]; CG = list(Sd["CG"]); OI = 1
NQ = Sd["TAU"].shape[-1]; thr = np.zeros((NQ, 2 * len(CFG)), np.float32); colf = np.zeros(2 * len(CFG), np.int64); GI = np.zeros((len(CFG), NQ), np.int64)
for i, (k, f) in enumerate(CFG):
    for q in range(NQ):
        gi = CG.index(8)
        while gi < len(CG) - 1 and Sd["NV"][i, OI, gi, q] < 20 * k + 200: gi += 1
        GI[i, q] = gi; t = Sd["TAU"][i, OI, gi, q]; thr[q, 2 * i] = t if np.isfinite(t) else -2.0
    thr[:, 2 * i + 1] = (Sd["GTS"][f, :, k - 1] + Sd["GTS"][f, :, k]) / 2; colf[2 * i] = colf[2 * i + 1] = f
    print(k, f, "clusters used (mean)", np.mean([CG[g] for g in GI[i]]), "tau1 missing", int((thr[:, 2 * i] == -2).sum()),
          "mean s_k - tau1", float(np.mean(Sd["GTS"][f, :, k - 1] - np.maximum(thr[:, 2 * i], -1))))
np.save(W_ + "thr.npy", thr); np.save(W_ + "colf.npy", colf); np.save(W_ + "seedgi.npy", GI)
