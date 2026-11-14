# Two diagnostics behind the order experiment, on the same 200k sample:
#  (1) where the zero-read slack sits: share of sum_b ||Delta_b|| * ||r_b|| by fixed block, and the slack for coarser blockings;
#  (2) how concentrated one query's offset is across a group's coordinates: share of ||Delta||^2 (and of sum Delta_j^2 lambda_j)
#      held by its k largest coordinates, against the first k in basis order and against a flat Gaussian offset.
import numpy as np, os, sys
from common import setup, log, EDGES
from scipy.stats import norm
BASIS = os.environ.get("BASIS", "pca")
D = setup(BASIS, nq_text_step=int(os.environ.get("TSTEP", "9")), nq_item=int(os.environ.get("NQI", "40")))
Z, cs, grp, K, G, n = D["Z"], D["cs"], D["grp"], D["K"], D["G"], D["n"]; Mu, VQ, LAM, Q, kind, S, Rn2 = D["Mu"], D["VQ"], D["LAM"], D["Q"], D["kind"], D["S"], D["Rn2"]
NB = len(EDGES) - 1; E0 = np.array(EDGES[:-1]); Rn = np.sqrt(Rn2); Mu2 = (Mu * Mu).sum(1); LAMK = LAM[grp]
BLK = {"11 dyadic": list(range(NB + 1)), "6": [0, 2, 4, 6, 8, 10, 11], "4": [0, 3, 6, 9, 11], "2": [0, 6, 11], "1": [0, 11]}
KS = [4, 8, 16, 32, 64, 128]
cw = np.bincount(cs, minlength=K) / len(cs)                                           # weight clusters by their items
out = {kd: dict(share=np.zeros(NB), slack={k: 0.0 for k in BLK}, top=np.zeros((4, len(KS))), nq=0) for kd in ("text", "item")}
for q, kd, s in zip(Q, kind, S.T):
    sig = s.std(); qgK = np.stack([q @ VQ[g] for g in range(G)])[grp]; al = (qgK * Mu).sum(1) / Mu2
    D2 = (qgK - al[:, None] * Mu).astype(np.float64) ** 2; Db = np.add.reduceat(D2, E0, axis=1)       # K x NB
    per = np.sqrt(Db)[cs] * Rn; o = out[kd]; o["share"] += per.mean(0) / per.sum(1).mean(); o["nq"] += 1
    for nm, ed in BLK.items():
        w = sum(np.sqrt(Db[:, ed[i]:ed[i+1]].sum(1))[cs] * np.sqrt(Rn2[:, ed[i]:ed[i+1]].sum(1)) for i in range(len(ed) - 1)); o["slack"][nm] += w.mean() / sig
    tot = D2.sum(1); P = D2 * LAMK; ptot = P.sum(1)
    sd = -np.sort(-D2, axis=1); sp = -np.sort(-P, axis=1)
    for i, k in enumerate(KS):
        o["top"][0, i] += (cw * sd[:, :k].sum(1) / tot).sum(); o["top"][1, i] += (cw * D2[:, :k].sum(1) / tot).sum()
        o["top"][2, i] += (cw * sp[:, :k].sum(1) / ptot).sum(); o["top"][3, i] += (cw * P[:, :k].sum(1) / ptot).sum()
np.set_printoptions(precision=1, suppress=True, linewidth=200)
flat = [100 * (k / n + 2 * norm.isf(k / n / 2) * norm.pdf(norm.isf(k / n / 2))) for k in KS]
for kd, o in out.items():
    q_ = o["nq"]; print(f"\n== {kd} queries ({q_}), basis {BASIS}")
    print(" share of the zero-read slack by block (%), blocks", EDGES); print(" ", 100 * o["share"] / q_)
    print(" zero-read slack in sd of the query's scores, by number of block norms:", {k: round(v / q_, 2) for k, v in o["slack"].items()})
    print(" k =", KS)
    print(" share of ||Delta||^2 in its k largest coordinates (%):", 100 * o["top"][0] / q_)
    print(" share of ||Delta||^2 in the first k coordinates    (%):", 100 * o["top"][1] / q_)
    print(" share of sum Delta^2*lambda in its k largest terms (%):", 100 * o["top"][2] / q_)
    print(" share of sum Delta^2*lambda in the first k terms   (%):", 100 * o["top"][3] / q_)
    print(" flat Gaussian offset, k largest of 768             (%):", np.array(flat))
log("done")
