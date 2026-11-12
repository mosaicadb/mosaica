# E1b: several anchors per query (orthogonal matching pursuit over the anchor set), sound l1 combination of table rows + annulus bound on the remainder.
import numpy as np, sys, time
sys.path.insert(0, "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/combined")
from core import kmeans, unit
ROOT = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/"
I = np.load(ROOT + "combined/idx_m768/index.npz"); Xs = np.load(ROOT + "combined/idx_m768/Xs.npy", mmap_mode="r")
N, n, K = int(I["N"]), int(I["n"]), int(I["K"]); cstart, Mq, mn, ahat = I["cstart"], I["Mq"], I["mn"], I["ahat"]
Q, qcls, thr, P, nimg = I["Q"], I["qcls"], I["thr"], I["P"], int(I["nimg"])
sel = np.r_[np.where(qcls == "item")[0][:60], np.where(qcls == "text")[0][::2][:150]]
Q, qcls, thr = Q[sel].astype(np.float64), qcls[sel], thr[sel]; NQ = len(Q); NT = 4
Mh = Mq / mn[:, None]; Ttr = unit(np.load(ROOT + "agent2/text_train.npy"))
POOLS = {"T256+I1000": np.concatenate([Mh, P[nimg:] / np.linalg.norm(P[nimg:], axis=1, keepdims=True)]), "T2206+I1000": np.concatenate([Mh, Ttr])}
JS = [1, 2, 4, 8, 16, 32]
uniq = np.concatenate([Mh, P[nimg:] / np.linalg.norm(P[nimg:], axis=1, keepdims=True), Ttr]).astype(np.float32)   # column ids: I 0..999, T256 1000..1255, T2206 1256..
pool_ids = {"T256+I1000": np.arange(0, 1256), "T2206+I1000": np.r_[np.arange(0, 1000), np.arange(1256, 1256 + len(Ttr))]}
CONF = {}
for pn, ids in pool_ids.items():
    PA = uniq[ids].astype(np.float64)
    for J in JS:
        idxs = np.zeros((NQ, J), int); coef = np.zeros((NQ, J)); rho = np.zeros(NQ)
        for qi in range(NQ):
            q = Q[qi]; S = []; r = q.copy()
            for _ in range(J):
                sc = PA @ r; sc[S] = -np.inf; S.append(int(sc.argmax()))       # positive-correlation OMP
                c, *_ = np.linalg.lstsq(PA[S].T, q, rcond=None); r = q - PA[S].T @ c
            idxs[qi] = ids[S]; coef[qi] = c; rho[qi] = np.linalg.norm(r)
        pm = np.einsum("qj,qjk->qk", coef, (uniq[idxs] @ Mh.T).astype(np.float64))      # sum_j c_j p_j . m^_c   (NQ x K)
        CONF[f"{pn} J={J}"] = (idxs, coef, rho, pm)
        print(pn, J, "rho item/text %.3f / %.3f" % (rho[qcls == "item"].mean(), rho[qcls == "text"].mean()), "sum|c| text %.2f" % np.abs(coef[qcls == "text"]).sum(1).mean(), flush=True)
qm = Q @ Mq.T; DA = 0.5 / 32767 + 1e-7
touched = {t: np.zeros((NT, NQ)) for t in CONF}; opened = {t: np.zeros((NT, NQ)) for t in CONF}; viol = 0; t0 = time.time()
for c in range(K):
    lo_, hi_ = int(cstart[c]), int(cstart[c + 1]); nc = hi_ - lo_
    if nc == 0: continue
    x = unit(Xs[lo_:hi_]); true = x @ Q.T.astype(np.float32); PX = x @ uniq.T
    HIc = np.ceil(PX.max(0).astype(np.float64) * 32768) / 32768; LOc = np.floor(PX.min(0).astype(np.float64) * 32768) / 32768
    ah = ahat[lo_:hi_].astype(np.float64) / 32767; m_ = mn[c]
    alo = np.maximum(ah - DA, -1)[:, None]; ahi = np.minimum(ah + DA, 1)[:, None]
    cm = np.clip(qm[:, c] / m_, -1, 1); s1 = np.sqrt(np.maximum(1 - cm * cm, 0))
    aU = np.clip(cm[None], alo, ahi); aL = np.clip(-cm[None], alo, ahi)
    ub1 = cm * aU + s1 * np.sqrt(1 - aU * aU) + 3e-5; lb1 = cm * aL - s1 * np.sqrt(1 - aL * aL) - 3e-5
    for k, (idxs, coef, rho, pm) in CONF.items():
        h = coef * HIc[idxs]; l = coef * LOc[idxs]; Bh = np.maximum(h, l).sum(1); Bl = np.minimum(h, l).sum(1)
        rm = cm - pm[:, c]; rp = np.sqrt(np.maximum(rho * rho - rm * rm, 0)); rr = np.maximum(np.sqrt(rm * rm + rp * rp), 1e-12)
        aU2 = np.clip((rm / rr)[None], alo, ahi); aL2 = np.clip((-rm / rr)[None], alo, ahi)
        ub2 = Bh + rm * aU2 + rp * np.sqrt(1 - aU2 * aU2) + 3e-5; lb2 = Bl + rm * aL2 - rp * np.sqrt(1 - aL2 * aL2) - 3e-5
        ub = np.minimum(ub1, ub2); lb = np.maximum(lb1, lb2); viol += int(((true > ub) | (true < lb)).sum())
        for ti in range(NT):
            t = thr[:, ti]; touched[k][ti] += ((lb < t) & (ub >= t)).sum(0); opened[k][ti] += (Bh + rr + 3e-5 >= t) & (Bl - rr - 3e-5 < t)
    if c % 200 == 0: print(c, round(time.time() - t0), "viol", viol, flush=True)
print("violations", viol)
for cl in ("item", "text"):
    m = qcls == cl
    print(f"\n== {cl} queries ({m.sum()}): items touched % | clusters opened %  at answer sizes 0.01 / 0.1 / 1 / 5 %")
    for t in CONF:
        print(f"  {t:22s}", " ".join(f"{100 * touched[t][ti][m].mean() / N:5.1f}" for ti in range(NT)), "|", " ".join(f"{100 * opened[t][ti][m].mean() / K:5.1f}" for ti in range(NT)))
