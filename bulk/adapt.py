# Query-chosen read order against the fixed order, exact coordinates, per-member read (the model of vc/scripts/margin.py).
# For each (query, cluster) the coordinates of the group's basis are read in an order chosen from the query's offset
# Delta = q - alpha*m in that basis; the unread part is bounded block by block (the same fixed dyadic blocks whose norms are
# stored per item) by sqrt(unread Delta energy in block) * sqrt(block norm^2 - energy already read in block).
# Orders: fix (basis order), prod (Delta_i^2 * lambda_i), greedy (largest one-step fall of sum_b sqrt(D_b Lambda_b),
# re-ranked at every ladder point), dq (Delta_i^2 only).
# Bounds: "both" as above; "dq" keeps the whole block norm on the item side (half-width is a per-cluster vector times
# the item's block norms, nothing accumulated per item beyond the estimate).
# Every interval is checked against the exact score; violations are counted.
import numpy as np, sys, os, json
from common import setup, log, wait_lock, EDGES
BASIS = os.environ.get("BASIS", "pca"); OUT = sys.argv[1]
TSTEP = int(os.environ.get("TSTEP", "3")); NQI = int(os.environ.get("NQI", "100"))
ORDERS = os.environ.get("ORDERS", "fix,prod,greedy").split(",")
ALPHAS = os.environ.get("ALPHAS", "star,zero").split(",")
D = setup(BASIS, nq_text_step=TSTEP, nq_item=NQI)
Z, cs, gs, grp, K, G, n = D["Z"], D["cs"], D["gs"], D["grp"], D["K"], D["G"], D["n"]
Mean, Mu, VQ, LAM, b, Q, kind, S, st, en, cl, Rn2, NS = D["Mean"], D["Mu"], D["VQ"], D["LAM"], D["b"], D["Q"], D["kind"], D["S"], D["st"], D["en"], D["cl"], D["Rn2"], D["ns"]
NB = len(EDGES) - 1
LAD = np.array([0, 1, 2, 3, 4, 6, 8, 11, 16, 23, 32, 45, 64, 91, 128, 181, 256, 362, 512, 768]); NL = len(LAD) - 1
DY = np.array([i for i, v in enumerate(LAD) if v in EDGES])                           # dyadic sub-ladder
SID = np.searchsorted(LAD[1:], np.arange(n), side="right")                            # ladder segment of a position in the order
BID = np.searchsorted(np.array(EDGES[1:]), np.arange(n), side="right")                # fixed block of a coordinate
SLACK = 2e-4
Rn = np.sqrt(Rn2)
LAMK = LAM[grp]                                                                        # K x n
Mu2 = (Mu * Mu).sum(1)
FR = [1e-4, 1e-3, 1e-2, 5e-2]; BINS = [0, 0.5, 1, 2, 4, 8, np.inf]; NBIN = len(BINS) - 1
E0 = np.array(EDGES[:-1])

def orders_for(D2):
    out = {}
    if "fix" in ORDERS: out["fix"] = None
    if "prod" in ORDERS: out["prod"] = np.argsort(-(D2 * LAMK), axis=1, kind="stable").astype(np.int16)
    if "dq" in ORDERS: out["dq"] = np.argsort(-D2, axis=1, kind="stable").astype(np.int16)
    if "greedy" in ORDERS:
        unread = np.ones((K, n), bool); O = np.empty((K, n), np.int16); rows = np.arange(K)[:, None]
        for j in range(NL):
            take = LAD[j+1] - LAD[j]
            Db = np.add.reduceat(D2 * unread, E0, axis=1); Lb = np.add.reduceat(LAMK * unread, E0, axis=1)
            dec = np.sqrt(Db * Lb)[:, BID] - np.sqrt(np.maximum(0, (Db[:, BID] - D2) * (Lb[:, BID] - LAMK)))
            dec[~unread] = -1.0
            if j == NL - 1: idx = np.argsort(-dec, axis=1, kind="stable")[:, :take]
            else:
                idx = np.argpartition(-dec, take - 1, axis=1)[:, :take]
                idx = np.take_along_axis(idx, np.argsort(-np.take_along_axis(dec, idx, 1), axis=1, kind="stable"), 1)
            O[:, LAD[j]:LAD[j+1]] = idx; unread[rows, idx] = False
        out["greedy"] = O
    return out

IDENT = np.arange(n)
def cluster_bounds(Zc, d, order, base, rn2c, rnc):
    m = len(Zc)
    if order is None: order = IDENT
    cell = SID * NB + BID[order]
    p2 = np.argsort(cell, kind="stable"); co = order[p2]; c2 = cell[p2]
    stt = np.concatenate([[0], np.flatnonzero(np.diff(c2)) + 1]); cells = c2[stt]
    Zs = Zc[:, co]; dco = d[co]
    sq = np.zeros((m, NL * NB), np.float32); ct = np.zeros((m, NL * NB), np.float32); dq = np.zeros(NL * NB)
    sq[:, cells] = np.add.reduceat(Zs * Zs, stt, axis=1)
    ct[:, cells] = np.add.reduceat(Zs * dco.astype(np.float32), stt, axis=1)
    dq[cells] = np.add.reduceat(dco.astype(np.float64) ** 2, stt)
    sqc = np.zeros((m, NL + 1, NB), np.float32); sqc[:, 1:] = sq.reshape(m, NL, NB).cumsum(1)
    ctc = np.zeros((m, NL + 1), np.float32); ctc[:, 1:] = ct.reshape(m, NL, NB).sum(2).cumsum(1)
    dqc = dq.reshape(NL, NB).cumsum(0); Dun = np.zeros((NL + 1, NB)); Dun[0] = dqc[-1]; Dun[1:] = np.maximum(0, dqc[-1][None] - dqc)
    sD = np.sqrt(Dun).astype(np.float32)
    left = np.sqrt(np.maximum(0, rn2c[:, None, :] - sqc))
    w = (left * sD[None]).sum(2) + SLACK
    w0 = rnc @ sD.T + SLACK
    est = base[:, None] + ctc
    return est, w, w0

names = [f"{o}/{bd}" for o in ORDERS for bd in ("both", "dq")]
acc = {kd: {nm: {lad: dict(cost=np.zeros(len(FR)), band_cost=np.zeros((len(FR), NBIN))) for lad in ("fine", "dyad")} for nm in names} for kd in ("text", "item")}
bandn = {kd: np.zeros((len(FR), NBIN)) for kd in ("text", "item")}; nqk = {"text": 0, "item": 0}; viol = {nm: 0 for nm in names}; checks = 0
zmean = {kd: np.zeros(len(FR)) for kd in ("text", "item")}; und0 = {kd: {nm: np.zeros(len(FR)) for nm in names} for kd in ("text", "item")}
perq = []
for qn, (q, kd) in enumerate(zip(Q, kind)):
    if qn % 5 == 0: wait_lock()
    s = S[:, qn]; sig = float(s.std()); med = float(np.median(s))
    qgK = np.stack([q @ VQ[g] for g in range(G)])[grp]; qm = Mean @ q
    U = {nm: np.full((NS, NL + 1), np.inf, np.float32) for nm in names}; L = {nm: np.full((NS, NL + 1), -np.inf, np.float32) for nm in names}
    for an in ALPHAS:
        al = (qgK * Mu).sum(1) / Mu2 if an == "star" else np.zeros(K)
        Dl = qgK - al[:, None] * Mu; D2 = (Dl * Dl).astype(np.float32)
        od = orders_for(D2)
        for k in range(len(cl)):
            c = cl[k]; a, e = st[k], en[k]; base = qm[c] + al[c] * b[a:e]
            for o in ORDERS:
                est, w, w0 = cluster_bounds(Z[a:e], Dl[c], None if od[o] is None else od[o][c].astype(np.int64), base, Rn2[a:e], Rn[a:e])
                for bd, ww in (("both", w), ("dq", w0)):
                    nm = f"{o}/{bd}"
                    U[nm][a:e] = np.minimum(U[nm][a:e], est + ww); L[nm][a:e] = np.maximum(L[nm][a:e], est - ww)
    ths = [np.partition(s, NS - round(f * NS))[NS - round(f * NS)] for f in FR]
    rec = dict(kind=kd, sig=sig, z=[float((t - med) / sig) for t in ths], cost={})
    for nm in names:
        viol[nm] += int(((s[:, None] > U[nm]) | (s[:, None] < L[nm])).sum()); checks += U[nm].size
        for fi, t in enumerate(ths):
            dec = (L[nm] >= t) | (U[nm] < t); u = np.abs(s - t) / sig
            # wrong decisions (a match ruled out or a non-match ruled in) are violations too
            wrong = ((L[nm] >= t) & (s[:, None] < t)) | ((U[nm] < t) & (s[:, None] >= t)); viol[nm] += int(wrong.sum())
            for lad, sub in (("fine", np.arange(NL + 1)), ("dyad", DY)):
                j = dec[:, sub].argmax(1); cost = LAD[sub][j].astype(float)
                dec[:, -1] = True                                         # all coordinates read: decided exactly
                A = acc[kd][nm][lad]; A["cost"][fi] += cost.mean(); A["band_cost"][fi] += np.histogram(u, BINS, weights=cost)[0] / NS
                if lad == "dyad": rec["cost"].setdefault(nm, []).append(float(cost.mean()))
            und0[kd][nm][fi] += 1 - dec[:, 0].mean()
    for fi, t in enumerate(ths):
        bandn[kd][fi] += np.histogram(np.abs(s - t) / sig, BINS)[0] / NS; zmean[kd][fi] += (t - med) / sig
    nqk[kd] += 1; perq.append(rec)
    if qn % 10 == 0: log(f"query {qn+1}/{len(Q)} ({kd}); violations {sum(viol.values())}")

res = dict(basis=BASIS, ns=NS, nq=nqk, ladder=LAD.tolist(), dyadic=LAD[DY].tolist(), fractions=FR, bins=[str(x) for x in BINS], violations=viol, checks=checks, per_query=perq, out={})
np.set_printoptions(precision=1, suppress=True, linewidth=200)
for kd in ("text", "item"):
    if not nqk[kd]: continue
    q_ = nqk[kd]; res["out"][kd] = dict(z=(zmean[kd] / q_).tolist(), band_items=(bandn[kd] / q_).tolist(), variants={})
    print(f"\n== {kd} queries ({q_}), basis {BASIS}; threshold above median (sd): {np.round(zmean[kd]/q_, 2)}")
    print("   items by band (%), rows = answer sizes, cols = |s-t|/sd in", BINS); print(100 * bandn[kd] / q_)
    for nm in names:
        r = {}
        for lad in ("dyad", "fine"):
            A = acc[kd][nm][lad]; c = A["cost"] / q_; bc = A["band_cost"] / q_
            r[lad] = dict(cost=c.tolist(), band_cost=bc.tolist())
            print(f" {nm:12s} {lad}: coords/item {np.round(c, 1)}   undecided at zero read {np.round(100 * und0[kd][nm] / q_, 1)}%")
            if lad == "dyad":
                print("      coords per item in band (0.01% / 0.1% / 1% / 5% rows):")
                with np.errstate(invalid="ignore", divide="ignore"): print(np.round(bc / (bandn[kd] / q_), 1))
                print("      share of cost by band (%):"); print(np.round(100 * bc / c[:, None], 1))
        r["und0"] = (und0[kd][nm] / q_).tolist(); res["out"][kd]["variants"][nm] = r
print("\nviolations", viol, "interval checks", checks)
json.dump(res, open(OUT, "w"))
log("done")
