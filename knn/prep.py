# Ground truth, filters and the seed stage (stage 1 of exact k-NN; also the approximate mode).
# usage: python3 prep.py NAME
import numpy as np, sys, os, json
from kcommon import *
NAME = sys.argv[1]; C = Ctx(NAME, need_rand=False); W_ = workdir(NAME)
N, n, K, G, NQ, Q, Xs, cstart = C.N, C.n, C.K, C.G, C.NQ, C.Q, C.Xs, C.cstart
log(NAME, "N", N, "n", n, "K", K, "NQ", NQ, {c: int((C.qcls == c).sum()) for c in np.unique(C.qcls)})
# ---- true scores ----
if os.path.exists(W_ + "S.npy"): S = np.load(W_ + "S.npy")
else:
    S = np.empty((N, NQ), np.float32)
    for i in range(0, N, 20_000): S[i:i+20_000] = unit(Xs[i:i+20_000]) @ Q.T
    np.save(W_ + "S.npy", S)
log("scores done")
# ---- filters over rows: all, random 50/10/1/0.1 %, whole clusters 10 %, original-id range 10 % ----
r = np.random.default_rng(11); FN = ["all", "rand50", "rand10", "rand1", "rand0.1", "clusters10", "idrange10"]
M = np.ones((7, N), bool)
for i, p in enumerate((0.5, 0.1, 0.01, 0.001)): M[1 + i] = r.random(N) < p
ck = np.zeros(K, bool); ck[r.choice(K, K // 10, replace=False)] = True; M[5] = ck[C.assign]
M[6] = C.perm < N // 10
np.save(W_ + "masks.npy", M)
CFG = [(1, 0), (10, 0), (100, 0)] + [(10, f) for f in range(1, 7)]          # (k, filter)
KMAX = 100
# ---- ground truth per (query, filter): top 101 visible rows ----
GTI = np.zeros((7, NQ, KMAX + 1), np.int64); GTS = np.zeros((7, NQ, KMAX + 1), np.float32)
for f in range(7):
    rows = np.where(M[f])[0]; Sf = S[rows]
    for q in range(NQ):
        s = Sf[:, q].copy()
        if C.qrow[q] >= 0 and M[f][C.qrow[q]]: s[np.searchsorted(rows, C.qrow[q])] = -np.inf
        top = np.argpartition(-s, KMAX + 1)[:KMAX + 2]; top = top[np.argsort(-s[top], kind="stable")][:KMAX + 1]
        GTI[f, q] = rows[top]; GTS[f, q] = s[top]
    del Sf
log("ground truth done")
# ---- cluster orders ----
cntv = np.stack([np.add.reduceat(M[f].astype(np.int64), cstart[:-1]) for f in range(7)])     # visible items per cluster
cm = C.qm / C.mn[None]                                                                          # q . m^
NS = min(64, K); ORD = {}
for f in range(7):
    okc = cntv[f] > 0
    ub = np.where(okc[None], C.ubc, -np.inf); a = np.argsort(-ub, axis=1, kind="stable")[:, :NS]
    b = np.take_along_axis(a, np.argsort(-np.take_along_axis(cm, a, 1), axis=1, kind="stable"), 1)
    c_ = np.argsort(-np.where(okc[None], cm, -np.inf), axis=1, kind="stable")[:, :NS]
    ORD[f] = {"ub": a, "ub64qm": b, "qm": c_}
need = [set() for _ in range(K)]
for f in range(7):
    for o in ORD[f].values():
        for q in range(NQ):
            for c in o[q]: need[c].add(q)
# ---- pass A: estimates after the 16-byte level for (item, query) pairs of the seed clusters ----
LSS = [C.LV.index(16), C.LV.index(96) if 96 in C.LV else C.LV.index(32)]
SBS = [3 + sum(C.LV[l] - C.LV[l - 1] + 1 for l in range(1, L + 1)) for L in LSS]; SB = SBS[0]; E = {}
for g in range(G):
    gs = gsetup(C, g, Q, rand=False)
    for ci, c in enumerate(gs.ks):
        if not need[c]: continue
        ql = np.array(sorted(need[c])); lo_, hi_ = int(cstart[c]), int(cstart[c + 1])
        x = unit(Xs[lo_:hi_]); z = (x - C.Mq[c]) @ gs.V; u = z / gs.sig
        ah = C.ahat[lo_:hi_].astype(np.float32) / 32767; m_ = C.mn[c]; mr = (m_ * ah - m_ * m_)[:, None]
        T = min(len(gs.se), 8 * C.LV[LSS[-1]] + 1); se = gs.se[:T]; s2 = gs.s2[gs.s2 < T]
        Cst = np.ones((hi_ - lo_, T), np.int32); k2 = np.floor(np.abs(u) / W).astype(np.int32); Cst[:, s2] = k2[:, se[s2]] + 1
        cum = np.cumsum(Cst, 1); ests = []
        for LS in LSS:
            dep = gs.dep_at[(cum <= 8 * C.LV[LS]).sum(1)]
            code = recon_esc(u, dep) * gs.sig; al = gs.esc[LS - 1][2][ci][ql]
            ests.append((C.qm[ql, c] + code @ gs.QV[ql].T + al * (mr - (code @ gs.MV[ci])[:, None])).astype(np.float32))
        pos = np.full(NQ, -1, np.int32); pos[ql] = np.arange(len(ql)); E[c] = (pos, ests[0], ests[1])
    if g % 8 == 0: log("pass A group", g)
log("pass A done; pairs", sum(e[1].size for e in E.values()), "level bytes", SBS)
# ---- seed stage for every config, order and number of clusters ----
CG = [c for c in (1, 2, 4, 8, 16, 32, 64) if c <= NS]; ON = ["ub", "ub64qm", "qm"]
sh = (len(CFG), len(ON), len(CG), NQ)
REC = np.zeros(sh); CREC = np.zeros(sh); NM1 = np.zeros(sh); TAU = np.full(sh, -np.inf, np.float32); NV = np.zeros(sh); NGR = np.zeros(sh); NFULL = np.zeros(sh); NABOVE = np.zeros(sh)
SEEDG = np.zeros(sh + (G,), bool)
for ci_, (k, f) in enumerate(CFG):
    kp = 2 * k + 8
    for oi, on in enumerate(ON):
        for q in range(NQ):
            cl = ORD[f][on][q]; rows = []; est = []; est2 = []; off = [0]
            for c in cl:
                rr = np.arange(cstart[c], cstart[c + 1]); v = M[f][rr] & (rr != C.qrow[q])
                rows.append(rr[v]); est.append(E[c][1][v, E[c][0][q]]); est2.append(E[c][2][v, E[c][0][q]]); off.append(off[-1] + int(v.sum()))
            rows = np.concatenate(rows); est = np.concatenate(est); est2 = np.concatenate(est2); sk = GTS[f, q, k - 1]; m1 = 20 * k + 200
            for gi, c_ in enumerate(CG):
                m = off[c_]; NV[ci_, oi, gi, q] = m; gset = np.unique(C.grp[cl[:c_]]); NGR[ci_, oi, gi, q] = len(gset); SEEDG[ci_, oi, gi, q, gset] = True
                if m == 0: continue
                CREC[ci_, oi, gi, q] = np.isin(GTI[f, q, :k], rows[:m]).mean()
                t1 = np.argpartition(-est[:m], m1 - 1)[:m1] if m > m1 else np.arange(m); NM1[ci_, oi, gi, q] = len(t1)
                top = t1[np.argpartition(-est2[t1], kp - 1)[:kp]] if len(t1) > kp else t1
                ex = np.sort(S[rows[top], q])[::-1]; NFULL[ci_, oi, gi, q] = len(top)
                REC[ci_, oi, gi, q] = (ex[:k] >= sk - 1e-6).sum() / k
                if len(ex) >= k: TAU[ci_, oi, gi, q] = ex[k - 1]
            for gi in range(len(CG)):
                t = TAU[ci_, oi, gi, q]
                NABOVE[ci_, oi, gi, q] = (GTS[f, q] >= t).sum() if np.isfinite(t) else np.inf     # capped at 101
    log("seed config", (k, FN[f]))
np.savez(W_ + "seed.npz", REC=REC, TAU=TAU, NV=NV, NGR=NGR, NFULL=NFULL, NABOVE=NABOVE, CREC=CREC, NM1=NM1, SBS=np.array(SBS), SEEDG=SEEDG, GTI=GTI, GTS=GTS, CG=np.array(CG),
         CFG=np.array(CFG), cntv=cntv, SB=SB)
for cl in np.unique(C.qcls):
    m = C.qcls == cl
    for ci_, (k, f) in enumerate(CFG):
        for oi, on in enumerate(ON):
            print(f"{cl:5s} k={k:3d} {FN[f]:10s} {on:7s} recall " + " ".join(f"{REC[ci_, oi, gi, m].mean():.3f}" for gi in range(len(CG)))
                  + " | cluster recall " + " ".join(f"{CREC[ci_, oi, gi, m].mean():.3f}" for gi in range(len(CG)))
                  + " | n>=tau1 (median) " + " ".join(f"{np.median(NABOVE[ci_, oi, gi, m]):.0f}" for gi in range(len(CG)))
                  + " | tau1=s_k share " + " ".join(f"{(TAU[ci_, oi, gi, m] >= GTS[f, m, k - 1] - 1e-6).mean():.2f}" for gi in range(len(CG)))
                  + " | scanned " + " ".join(f"{NV[ci_, oi, gi, m].mean():.0f}" for gi in range(len(CG))))
