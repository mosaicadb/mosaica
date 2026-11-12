# E1c: per-query cluster extremes known only to +-delta (a cached "own row" for a repeated query direction): items left undecided after range + extremes.
import numpy as np, sys, time
sys.path.insert(0, "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/combined")
from core import unit
ROOT = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/"
I = np.load(ROOT + "combined/idx_m768/index.npz"); Xs = np.load(ROOT + "combined/idx_m768/Xs.npy", mmap_mode="r")
N, K = int(I["N"]), int(I["K"]); cstart, Mq, mn, ahat = I["cstart"], I["Mq"], I["mn"], I["ahat"]; Q, qcls, thr = I["Q"], I["qcls"], I["thr"]
sel = np.r_[np.where(qcls == "item")[0][:100], np.where(qcls == "text")[0][::2][:150]]; Q, qcls, thr = Q[sel], qcls[sel], thr[sel][:, :4]; NQ = len(Q); NT = 4
qm = Q @ Mq.T; DA = 0.5 / 32767 + 1e-7; DEL = [0, 0.005, 0.01, 0.02, 0.05]
touched = np.zeros((len(DEL), NT, NQ)); mixed = np.zeros((len(DEL), NT, NQ)); sw = np.zeros(NQ); sm = np.zeros(NQ); sm2 = np.zeros(NQ)
for c in range(K):
    lo_, hi_ = int(cstart[c]), int(cstart[c + 1]); nc = hi_ - lo_
    if nc == 0: continue
    x = unit(Xs[lo_:hi_]); true = x @ Q.T
    sw += true.var(0) * nc; mu = true.mean(0); sm += mu * nc; sm2 += mu * mu * nc
    ah = ahat[lo_:hi_].astype(np.float32) / 32767; m_ = mn[c]
    alo = np.maximum(ah - DA, -1)[:, None]; ahi = np.minimum(ah + DA, 1)[:, None]
    cm = np.clip(qm[:, c] / m_, -1, 1); s1 = np.sqrt(np.maximum(1 - cm * cm, 0))
    aU = np.clip(cm[None], alo, ahi); aL = np.clip(-cm[None], alo, ahi)
    ub1 = cm * aU + s1 * np.sqrt(1 - aU * aU) + 3e-5; lb1 = cm * aL - s1 * np.sqrt(1 - aL * aL) - 3e-5
    for di, d in enumerate(DEL):
        tmx = true.max(0) + d + 3e-5; tmn = true.min(0) - d - 3e-5; ub = np.minimum(ub1, tmx[None]); lb = np.maximum(lb1, tmn[None])
        for ti in range(NT):
            t = thr[:, ti]; touched[di, ti] += ((lb < t) & (ub >= t)).sum(0); mixed[di, ti] += (tmx >= t) & (tmn < t)
for cl in ("item", "text"):
    m = qcls == cl; wv = sw[m] / N; bv = sm2[m] / N - (sm[m] / N) ** 2
    print(f"== {cl}: score sd within clusters {np.sqrt(wv).mean():.4f}, between cluster means {np.sqrt(bv).mean():.4f}; share of score variance between clusters {(bv / (bv + wv)).mean():.2f}")
    for di, d in enumerate(DEL):
        print(f"  extremes known to +-{d}: items undecided %", " ".join(f"{100 * touched[di, ti][m].mean() / N:5.1f}" for ti in range(NT)), "| clusters undecided %", " ".join(f"{100 * mixed[di, ti][m].mean() / K:5.1f}" for ti in range(NT)))
