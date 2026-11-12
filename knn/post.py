# Stage 3 (final selection among the survivors of the sweep, from recorded interval ladders) and the cost tables.
# usage: python3 post.py NAME NWORKERS
import numpy as np, sys, json
from kcommon import *
NAME, NW = sys.argv[1], int(sys.argv[2]); C = Ctx(NAME, need_rand=False); W_ = workdir(NAME)
N, n, K, G, NQ, FULL = C.N, C.n, C.K, C.G, C.NQ, C.FULL
Sd = np.load(W_ + "seed.npz"); CFG = [tuple(x) for x in Sd["CFG"]]; CG = list(Sd["CG"]); thr = np.load(W_ + "thr.npy"); colf = np.load(W_ + "colf.npy")
GI = np.load(W_ + "seedgi.npy"); S = np.load(W_ + "S.npy", mmap_mode="r"); M = np.load(W_ + "masks.npy"); GTS = Sd["GTS"]; OI = 1
FN = ["all", "rand50", "rand10", "rand1", "rand0.1", "clusters10", "idrange10"]
R = [np.load(W_ + f"out_w{w}.npz") for w in range(NW)]
IV = {k: sum(json.loads(str(r["IV"]))[k] for r in R) for k in ("det", "pre", "rand")}
RS = [json.loads(str(r["RSTAT"])) for r in R]; RSTAT = [max(x[0] for x in RS)] + [sum(x[i] for x in RS) for i in range(1, 5)]
SC = [s for r in R for s in json.loads(str(r["SC"])) if s]; sc = np.max(np.array(SC), 0).tolist() if SC else []
print("interval violations (true outside a stated interval):", IV)
print("randomised levels: largest |error| / predicted sd %.2f over %d (item, level, query) triples; > 4 sd: %d, > 5 sd: %d, > 6 sd: %d; constant used %.2f"
      % (RSTAT[0], RSTAT[1], RSTAT[2], RSTAT[3], RSTAT[4], C.CSG))
print("clusters wholly above a threshold (never opened), per column:", sum(r["WHOLEIN"] for r in R).tolist())
LV = C.LV
POS = {"D": [0, 3] + list(3 + np.cumsum([LV[l] - LV[l - 1] + 1 for l in range(1, C.ND)])),
       "R": [0, 3] + list(3 + np.cumsum([LV[l] - LV[l - 1] + 1 for l in range(1, C.ld + 1)] + [C.RLV[i] - (C.RLV[i - 1] if i else C.LD) + sc[i] for i in range(C.NR)]))}
for t in POS: POS[t] = np.array(POS[t] + [POS[t][-1] + FULL], float)
print("cumulative bytes per item at each position:", {t: POS[t].tolist() for t in POS})
def comb(t, f, how="sum"):
    a = [r[f"{t}|{f}"] for r in R]; return np.any(a, 0) if how == "or" else np.sum(a, 0)
OUT = {}
for t in ("R", "D"):
    enter, ib, cb, gt, viol = comb(t, "enter"), comb(t, "ib"), comb(t, "cb"), comb(t, "gt", "or"), comb(t, "viol")
    print(f"\n==== track {t}: sweep decision violations per column {viol.tolist()}")
    lr = np.concatenate([r[f"{t}|lr"] for r in R]); lq = np.concatenate([r[f"{t}|lq"] for r in R])
    lLB = np.concatenate([r[f"{t}|lLB"] for r in R], 1); lUB = np.concatenate([r[f"{t}|lUB"] for r in R], 1); npos = lLB.shape[0]; P = POS[t]
    assert npos + 1 == len(P)
    byq = [np.where(lq == q)[0] for q in range(NQ)]; s3viol = 0
    for i, (k, f) in enumerate(CFG):
        for which, col in (("seed", 2 * i), ("oracle", 2 * i + 1)):
            s3 = np.zeros(NQ); s3full = np.zeros(NQ); s3n = np.zeros(NQ); rank = np.zeros(NQ); rounds = np.zeros(NQ)
            for q in range(NQ):
                ix = byq[q]; rows = lr[ix]; tv = S[rows, q] if len(ix) else np.zeros(0)
                keep = M[f][rows] & (rows != C.qrow[q]) & (tv >= thr[q, col]); ix = ix[keep]; rows = rows[keep]; tv = tv[keep]; m = len(ix)
                LBs = np.vstack([lLB[:, ix], tv[None]]); UBs = np.vstack([lUB[:, ix], tv[None]])
                p = (LBs >= thr[q, col]).argmax(0); ar = np.arange(m); s3n[q] = m
                if m < k: s3viol += 1; continue
                while True:
                    lb = LBs[p, ar]; ub = UBs[p, ar]; tau = -np.partition(-lb, k - 1)[k - 1]; out = ub < tau
                    us = np.sort(ub); cnt = m - np.searchsorted(us, lb, side="right") - (ub > lb)
                    und = ~out & ~(cnt <= k - 1) & (p < npos)
                    if not und.any(): break
                    s3[q] += (P[p[und] + 1] - P[p[und]]).sum(); s3full[q] += (p[und] + 1 == npos).sum(); p[und] += 1; rounds[q] += 1
                cert = ~out & (cnt <= k - 1); rest = np.where(~out & ~cert)[0]
                sel = np.r_[np.where(cert)[0], rest[np.argsort(-lb[rest], kind="stable")][:k - int(cert.sum())]]
                if len(sel) != k or (tv[sel] < GTS[f, q, k - 1] - 1e-6).any(): s3viol += 1
                rank[q] = (P[npos] - P[p[sel]]).sum()
            if which == "seed":
                g = GI[i]; qa = np.arange(NQ); pick = lambda A: A[i, OI, g, qa]
                seed_c = max(64, 0) * (n + 4) * np.ones(NQ) + np.maximum(np.array(CG)[g] - 64, 0) * (n + 4)
                seed_i = pick(Sd["NV"]) * Sd["SBS"][0] + pick(Sd["NM1"]) * (Sd["SBS"][1] - Sd["SBS"][0]) + pick(Sd["NFULL"]) * FULL
                sg = Sd["SEEDG"][i, OI, g, qa]; stouch = pick(Sd["NV"]); sfull = pick(Sd["NFULL"])
            else: seed_c = seed_i = stouch = sfull = np.zeros(NQ); sg = np.zeros((NQ, G), bool)
            groups = (gt[:, col, :].T | sg).sum(1)
            OUT[(t, i, which)] = dict(table=4.0 * K * np.ones(NQ), seed=seed_c + seed_i, clus=cb[col], items=ib[col], s3=s3, ids=4.0 * k * np.ones(NQ),
                                      bases=groups * n * n * 4.0, touched=enter[0, col] + stouch, full=enter[-1, col] + sfull + s3full, surv=s3n, rank=rank,
                                      rounds=rounds, groups=groups, lev=enter[:, col])
    print("stage-3 violations (selected set differs from the true top k, ties allowed):", s3viol)
np.save(W_ + "post.npy", np.array([OUT], dtype=object), allow_pickle=True)
nvis = M.sum(1)
for cl in np.unique(C.qcls):
    m = C.qcls == cl
    print(f"\n#### {NAME}: {cl} queries ({int(m.sum())}); bytes per query in MB unless marked; N = {N}, dataset = {N * FULL / 1e6:.0f} MB")
    print("| k | filter | track | which | table | seed | sweep clusters | sweep items | stage 3 | total excl. bases | B per item | % of dataset | % of visible data | bases MB (groups) | items touched | full reads | survivors of sweep | ranked extra KB |")
    print("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    for i, (k, f) in enumerate(CFG):
        for t in ("R", "D"):
            for which in ("seed", "oracle"):
                o = OUT[(t, i, which)]; mm = lambda a: float(np.mean(a[m]))
                tot = mm(o["table"] + o["seed"] + o["clus"] + o["items"] + o["s3"] + o["ids"])
                print(f"| {k} | {FN[f]} | {t} | {'procedure' if which == 'seed' else 'oracle threshold'} | {mm(o['table']) / 1e6:.3f} | {mm(o['seed']) / 1e6:.3f} | {mm(o['clus']) / 1e6:.3f} | "
                      f"{mm(o['items']) / 1e6:.3f} | {mm(o['s3']) / 1e6:.4f} | {tot / 1e6:.3f} | {tot / N:.2f} | {100 * tot / (N * FULL):.3f} | {100 * tot / (nvis[f] * FULL):.2f} | "
                      f"{mm(o['bases']) / 1e6:.0f} ({mm(o['groups']):.1f}) | {mm(o['touched']):.0f} | {mm(o['full']):.1f} | {np.median(o['surv'][m]):.0f} | {mm(o['rank']) / 1e3:.1f} |")
