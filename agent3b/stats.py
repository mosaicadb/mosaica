# Level-0 / group-level measurements and per-item floors on the 200k subset.
# usage: python3 stats.py [datadir] [files...]   (defaults: ../data e00 e05 e10 e15)
import numpy as np, time, json, sys
rng = np.random.default_rng(1)
t0 = time.time()
def log(*a): print(f"[{time.time()-t0:6.0f}s]", *a, flush=True)
DD = sys.argv[1] if len(sys.argv) > 1 else "../data"
FILES = sys.argv[2:] if len(sys.argv) > 2 else ["e00", "e05", "e10", "e15"]
OUT = "stats_" + DD.strip("./").replace("/", "_") + ".json"
def unit(a):
    a = a.astype(np.float32); a /= np.linalg.norm(a, axis=1, keepdims=True); return a
X = unit(np.concatenate([np.load(f"{DD}/{f}.npy") for f in FILES]))
N, n = X.shape
K = N // 1000; NQ = 200; G = max(1, N // 15000)
TS = [0.95, 0.9, 0.8, 0.7]
qi = np.sort(rng.choice(N, NQ, replace=False)); Q = X[qi]
true = X @ Q.T                                    # N x NQ
log("N", N, "n", n, "K", K, "G", G, "mean vector norm", float(np.linalg.norm(X.mean(0))))
res = {"N": N, "n": n, "K": K, "G": G, "NQ": NQ, "TS": TS}

# ---------- A. margins, entropy bound ----------
def h2(p):
    p = np.clip(p, 1e-300, 1 - 1e-16); return -(p * np.log2(p) + (1 - p) * np.log2(1 - p))
A = {}
for t in TS:
    pq_ = (true >= t).mean(0)
    u = np.abs(true - t)
    A[str(t)] = {"match_share": float(pq_.mean()), "match_share_median_q": float(np.median(pq_)),
                 "entropy_bits_per_item": float(h2(pq_).mean()),
                 "entropy_pct_of_dataset": float(h2(pq_).mean() / (16 * n) * 100),
                 "within": {str(w): float((u < w).mean()) for w in (0.002, 0.005, 0.01, 0.02, 0.05, 0.1)}}
res["A"] = A
edges = np.linspace(-1, 1, 2001)
hist = np.histogram(true, bins=edges)[0]
np.savez("hist_" + DD.strip("./").replace("/", "_") + ".npz", edges=edges, hist=hist, N=N, NQ=NQ)
log("A done", {t: round(v["match_share"] * 100, 3) for t, v in A.items()})

# ---------- clusters ----------
def kmeans(S, K, iters=12):
    C = S[rng.choice(len(S), K, replace=False)].copy()
    for it in range(iters):
        a = np.concatenate([(S[i:i+50_000] @ C.T).argmax(1) for i in range(0, len(S), 50_000)])
        M = np.zeros_like(C); np.add.at(M, a, S)
        cnt = np.bincount(a, minlength=len(C)); empty = cnt == 0
        M[empty] = S[rng.choice(len(S), int(empty.sum()))]
        C = M / np.linalg.norm(M, axis=1, keepdims=True)
    return C
Chat = kmeans(X, K)
assign = np.concatenate([(X[i:i+50_000] @ Chat.T).argmax(1) for i in range(0, N, 50_000)])
cnt = np.bincount(assign, minlength=K)
Mean = np.zeros((K, n)); np.add.at(Mean, assign, X); Mean = (Mean / np.maximum(cnt, 1)[:, None]).astype(np.float32)
Mh = Mean / np.maximum(np.linalg.norm(Mean, axis=1, keepdims=True), 1e-9)
grp = (Mh @ kmeans(Mh, G, 20).T).argmax(1) if G > 1 else np.zeros(K, int)
log("clusters done; sizes min/med/max", int(cnt.min()), int(np.median(cnt)), int(cnt.max()))

# group bases (as the baseline: PCA of pooled residuals), query-energy profile from pseudo-queries
pq = X[rng.choice(N, 4000, replace=False)]
pk = np.argsort(-(pq @ Chat.T), axis=1)[:, :4]
LAM, VV, QBAR = {}, {}, {}
for g in range(G):
    m = grp[assign] == g
    Rs = X[m] - Mean[assign[m]]
    lam, V = np.linalg.eigh((Rs.T @ Rs).astype(np.float64) / max(len(Rs), 1))
    LAM[g] = np.maximum(lam[::-1], 1e-12).copy(); VV[g] = V[:, ::-1].astype(np.float32).copy()
    rows, cols = np.where(grp[pk] == g)
    QBAR[g] = (((pq[rows] - Mean[pk[rows, cols]]) @ VV[g]) ** 2).mean(0).astype(np.float64) + 1e-12 if len(rows) > 50 else LAM[g].copy()
log("bases done")
pr = lambda lam: float(lam.sum() ** 2 / (lam ** 2).sum())
res["eff_dim_residual_participation"] = float(np.median([pr(LAM[g]) for g in range(G)]))
res["eff_dim_gamma_participation"] = float(np.median([pr(np.sqrt(LAM[g] * QBAR[g])) for g in range(G)]))

# ---------- floors: reverse water-filling on gamma_i = Qbar_i * lam_i ----------
BG = np.array([0] + [int(round(2 ** (j / 2))) for j in range(4, 28)])       # bits: 0,4,6,8,11,16,...,8192,11585
BG = BG[BG < 16 * n]
def waterfill(gam, B):
    if B == 0: return gam.copy()
    lo, hi = 1e-30, gam.max()
    for _ in range(200):
        th = np.sqrt(lo * hi)
        if 0.5 * np.log2(np.maximum(gam / th, 1)).sum() > B: lo = th
        else: hi = th
    return np.minimum(gam, hi)
E2 = {}                                             # per group: len(BG) x n error variance per dimension
for g in range(G):
    gam = LAM[g] * QBAR[g]
    E2[g] = np.stack([waterfill(gam, B) / QBAR[g] for B in BG]).astype(np.float64)
ZS = [3.0, 6.0, 8.4]
VAR = ["det"] + [f"z{z}" for z in ZS]
floor_bits = {v: np.zeros(len(TS)) for v in VAR}        # Gaussian-oracle: level 0 free when the prior decides
floor_bits1 = {v: np.zeros(len(TS)) for v in VAR}       # per-item model: >= 1 bit unless annulus decides
touched = {v: np.zeros(len(TS)) for v in VAR}

# ---------- B. level-0 cuts ----------
KS = [0, 1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 128]
KMAX = max(KS)
cut = np.zeros((len(KS), len(TS), 3))               # survivors (UB>=t), certified in (LB>=t), violations
capc = np.zeros((len(TS), 3)); plain = np.zeros((len(TS), 3))
oracle = {"cluster": np.zeros((len(TS), 3)), "micro32": np.zeros((len(TS), 3)), "micro8": np.zeros((len(TS), 3))}
expo = []; nmatch = np.zeros(len(TS))
def sub_groups(R, size):
    k = max(1, len(R) // size)
    if k == 1: return np.zeros(len(R), int)
    C = R[rng.choice(len(R), k, replace=False)].copy()
    for _ in range(8):
        d2 = (R * R).sum(1)[:, None] - 2 * R @ C.T + (C * C).sum(1)[None]
        a = d2.argmin(1)
        for j in range(k):
            if (a == j).any(): C[j] = R[a == j].mean(0)
    return a
for c in range(K):
    idx = np.where(assign == c)[0]; g_ = len(idx)
    if g_ == 0: continue
    Xc = X[idx]; m = Mean[c]; mh = Mh[c]; T = true[idx]         # g x NQ
    R = Xc - m; g = int(grp[c])
    # cluster cap
    a = np.clip(Xc @ mh, -1, 1); amax = np.arccos(a.min()); beta = np.arccos(np.clip(Q @ mh, -1, 1))
    ubc = np.cos(np.maximum(beta - amax, 0)) + 2e-5; lbc = np.cos(np.minimum(beta + amax, np.pi)) - 2e-5
    # plain level 0 of the baseline: q.m + s +- |Delta||r|
    s = R @ m; rn = np.linalg.norm(R, axis=1); Dl = Q - m; dn = np.linalg.norm(Dl, axis=1)
    est0 = (Q @ m)[None] + s[:, None]; h0 = np.outer(rn, dn) * 1.0001 + 2e-5
    # exact coordinates: basis [mh, group PCA directions] orthonormalised
    Bc = np.linalg.qr(np.concatenate([mh[:, None], VV[g][:, :KMAX]], axis=1).astype(np.float64))[0].astype(np.float32)
    Y = Xc @ Bc; QB = Q @ Bc
    xt = np.sqrt(np.maximum(1 - np.cumsum(Y ** 2, 1), 0)); qt = np.sqrt(np.maximum(1 - np.cumsum(QB ** 2, 1), 0))
    ub0 = None
    for ti, t in enumerate(TS):
        M = T >= t; nmatch[ti] += M.sum()
        capc[ti] += [g_ * (ubc >= t).sum(), g_ * (lbc >= t).sum(), (M & (ubc < t)[None]).sum() + (~M & (lbc >= t)[None]).sum()]
        sv = est0 + h0 >= t; win = est0 - h0 >= t
        plain[ti] += [sv.sum(), win.sum(), (M & ~sv).sum() + (win & ~M).sum()]
        nm = M.sum(0)
        oracle["cluster"][ti] += [g_ * ((nm > 0) & (nm < g_)).sum(), g_ * (nm == g_).sum(), g_ * (nm == 0).sum()]
    for ki, k in enumerate(KS):
        est = Y[:, :k+1] @ QB[:, :k+1].T; h = np.outer(xt[:, k], qt[:, k]) * 1.0001 + 2e-5
        if k == 0: ub0, lb0 = est + h, est - h
        for ti, t in enumerate(TS):
            M = T >= t; sv = est + h >= t; win = est - h >= t
            cut[ki, ti] += [sv.sum(), win.sum(), (M & ~sv).sum() + (win & ~M).sum()]
    for name, size in (("micro32", 32), ("micro8", 8)):
        sg = sub_groups(R, size)
        for j in np.unique(sg):
            mm = sg == j; gj = int(mm.sum())
            for ti, t in enumerate(TS):
                nm = (T[mm] >= t).sum(0)
                oracle[name][ti] += [gj * ((nm > 0) & (nm < gj)).sum(), gj * (nm == gj).sum(), gj * (nm == 0).sum()]
    # exposure of each item in its own direction, within its cluster
    Gm = R @ R.T; dg = np.diag(Gm).copy(); np.fill_diagonal(Gm, -np.inf)
    expo.append((dg - Gm.max(1)) / np.sqrt(np.maximum(dg, 1e-12)))
    # floors
    Qp = ((Dl @ VV[g]).astype(np.float64)) ** 2                  # NQ x n
    sig = np.sqrt(Qp @ E2[g].T)                                  # NQ x len(BG), decreasing in B
    hdet = np.sqrt(Qp) @ np.sqrt(E2[g]).T
    for ti, t in enumerate(TS):
        U = np.abs(T - t)
        undec0 = (ub0 >= t) & (lb0 < t)                          # annulus does not decide
        for v, curve in [("det", hdet)] + [(f"z{z}", z * sig) for z in ZS]:
            lvl = (curve[None, :, :] > U[:, :, None]).sum(2)     # number of grid levels that are insufficient
            bits = np.where(lvl == 0, 0, np.where(lvl >= len(BG), 16 * n, BG[np.maximum(lvl - 1, 0)]))
            floor_bits[v][ti] += bits.sum()
            b1 = np.where(undec0, np.maximum(bits, 1), 0)
            floor_bits1[v][ti] += b1.sum(); touched[v][ti] += (b1 > 0).sum()
    if c % 50 == 0: log("cluster", c)
tot = N * NQ
res["matches"] = (nmatch / tot).tolist()
res["cap"] = (capc / tot).tolist(); res["plain0"] = (plain / tot).tolist()
res["cut"] = {str(k): (cut[ki] / tot).tolist() for ki, k in enumerate(KS)}
res["cut_violations"] = float(cut[:, :, 2].sum() + capc[:, 2].sum() + plain[:, 2].sum())
res["oracle"] = {k: (v / tot).tolist() for k, v in oracle.items()}
expo = np.concatenate(expo)
res["exposure_quantiles"] = {str(p): float(np.quantile(expo, p)) for p in (0.01, 0.05, 0.25, 0.5, 0.75)}
res["exposure_share_above"] = {str(w): float((expo > w).mean()) for w in (0.02, 0.05, 0.1, 0.2)}
res["floor_pct"] = {v: (floor_bits[v] / tot / (16 * n) * 100).tolist() for v in VAR}
res["floor1_pct"] = {v: (floor_bits1[v] / tot / (16 * n) * 100).tolist() for v in VAR}
res["floor_bytes_per_item"] = {v: (floor_bits[v] / tot / 8).tolist() for v in VAR}
res["floor1_touched"] = {v: (touched[v] / tot).tolist() for v in VAR}
json.dump(res, open(OUT, "w"), indent=1)
log("violations", res["cut_violations"])
pc = lambda a: " ".join(f"{100*x:7.3f}" for x in a)
print("matches %       ", pc(res["matches"]))
print("entropy bound % ", " ".join(f"{A[str(t)]['entropy_pct_of_dataset']:.5f}" for t in TS))
print("cluster cap  surv%", pc(capc[:, 0] / tot), "| in%", pc(capc[:, 1] / tot))
print("plain lvl0   surv%", pc(plain[:, 0] / tot), "| in%", pc(plain[:, 1] / tot))
for ki, k in enumerate(KS):
    print(f"exact k={k:3d}  surv%", pc(cut[ki, :, 0] / tot), "| in%", pc(cut[ki, :, 1] / tot))
for k, v in oracle.items(): print(f"oracle {k:8s} mixed%", pc(v[:, 0] / tot), "| all-match%", pc(v[:, 1] / tot))
print("exposure", res["exposure_quantiles"], res["exposure_share_above"])
for v in VAR: print(f"floor {v:5s} % of dataset", " ".join(f"{x:.4f}" for x in res["floor_pct"][v]), "| per-item-model", " ".join(f"{x:.4f}" for x in res["floor1_pct"][v]), "| touched", pc(res["floor1_touched"][v]))
print("eff dims", res["eff_dim_residual_participation"], res["eff_dim_gamma_participation"])
