# Step 1. Exact scores of all 469 queries against the 1M rows (S.f32), nested random subsamples that exclude the 100
# item-query rows, and one spherical k-means per subsample with ~1,000 items per cluster (the method of vc/scripts/prep.py).
# The 999,900-row top level reuses index.npz level 0. Per clustering: labels, means, x.m-hat, m.r, |r|, the support table
# (anchors = the cluster centres) and per-group residual covariances for the local bases.
import numpy as np, os, sys
import scipy.sparse as sp
from common import *
rng = np.random.default_rng(20261113)
Q, qtype, qkind, qi = queries()
ix = np.load(os.path.join(V, "index.npz")); lab_top = ix["levels"][0].astype(np.int32)

# nested subsamples: rank[i] < N_j  <=>  row i is in subsample j. Query rows get rank N (never in a subsample).
isq = np.zeros(N, bool); isq[qi] = True
rank = np.full(N, N, np.int64); cand = np.flatnonzero(~isq); rank[rng.permutation(cand)] = np.arange(len(cand))
np.save(os.path.join(OUT, "rank.npy"), rank)
KS = [s // 1000 for s in SIZES[:-1]]                    # 31, 62, 125, 250, 500
log("sizes", SIZES, "K", KS + [1000])

# pass 0: scores, and initial centres
init_rows = [np.sort(rng.choice(np.flatnonzero(rank < s), k, replace=False)) for s, k in zip(SIZES[:-1], KS)]
C = [rows(r).copy() for r in init_rows]
if not os.path.exists(os.path.join(OUT, "S.f32")):
    with open(os.path.join(OUT, "S.tmp"), "wb") as f:
        f.truncate(NQ * N * 4)
        for i, X in chunks():
            S = Q @ X.T
            for q in range(NQ):
                f.seek((q * N + i) * 4); S[q].tofile(f)
    os.rename(os.path.join(OUT, "S.tmp"), os.path.join(OUT, "S.f32")); log("scores written")

def sums(S, a, K):
    return np.asarray(sp.csr_matrix((np.ones(len(a), np.float32), (a, np.arange(len(a)))), shape=(K, len(a))) @ S)
for it in range(12):
    M = [np.zeros((k, n), np.float64) for k in KS]; cnt = [np.zeros(k, np.int64) for k in KS]
    for i, X in chunks():
        r = rank[i:i + len(X)]
        for j, (s, k) in enumerate(zip(SIZES[:-1], KS)):
            m = r < s; Xs = X[m]; a = (Xs @ C[j].T).argmax(1)
            M[j] += sums(Xs, a, k); cnt[j] += np.bincount(a, minlength=k)
    for j, k in enumerate(KS):
        e = cnt[j] == 0
        if e.any(): M[j][e] = rows(np.sort(rng.choice(np.flatnonzero(rank < SIZES[j]), int(e.sum()), replace=False)))
        C[j] = (M[j] / np.linalg.norm(M[j], axis=1, keepdims=True)).astype(np.float32)
    log("k-means iteration", it, "empties", [int((c == 0).sum()) for c in cnt])

# final assignment; labels as full-length arrays (-1 outside the subsample)
labs = [np.full(N, -1, np.int32) for _ in KS]
for i, X in chunks():
    r = rank[i:i + len(X)]
    for j, s in enumerate(SIZES[:-1]):
        m = r < s; labs[j][i:i + len(X)][m] = (X[m] @ C[j].T).argmax(1)
labs.append(lab_top.copy()); KSA = KS + [1000]

def kmeans_small(S, K, iters=20):
    Cc = S[rng.choice(len(S), K, replace=False)].copy()
    for _ in range(iters):
        a = (S @ Cc.T).argmax(1); Mm = sums(S, a, K); e = np.bincount(a, minlength=K) == 0
        Mm[e] = S[rng.choice(len(S), int(e.sum()))]; Cc = Mm / np.linalg.norm(Mm, axis=1, keepdims=True)
    return Cc

for j, (K, lab) in enumerate(zip(KSA, labs)):
    inlab = lab >= 0                                    # the top level keeps the query rows in its clusters
    Ms = np.zeros((K, n), np.float64); cnt = np.bincount(lab[inlab], minlength=K)
    for i, X in chunks():
        l = lab[i:i + len(X)]; m = l >= 0; Ms += sums(X[m], l[m], K)
    Mean = (Ms / cnt[:, None]).astype(np.float32); mn = np.linalg.norm(Mean, axis=1); Mh = Mean / mn[:, None]
    G = max(12, round(64 * SIZES[j] / 1_000_000)) if j < len(KS) else 64
    G = min(G, K)
    grp = ix["grp"].astype(np.int32) if j == len(KS) else (Mh @ kmeans_small(Mh, G).T).argmax(1).astype(np.int32)
    G = int(grp.max()) + 1
    a = np.zeros(N, np.float32); xm = np.zeros(N, np.float32)
    Pmax = np.full((K, K), -2, np.float32); Pmin = np.full((K, K), 2, np.float32)      # [cluster, anchor]
    Cov = np.zeros((G, n, n), np.float32); gcnt = np.zeros(G, np.int64)
    for i, X in chunks():
        l = lab[i:i + len(X)]; m = l >= 0; Xs = X[m]; ls = l[m]
        P = Xs @ Mh.T
        a[i:i + len(X)][m] = P[np.arange(len(ls)), ls]; xm[i:i + len(X)][m] = P[np.arange(len(ls)), ls] * mn[ls]
        o = np.argsort(ls, kind="stable"); st = np.concatenate([[0], np.flatnonzero(np.diff(ls[o])) + 1]); cl = ls[o][st]
        Pmax[cl] = np.maximum(Pmax[cl], np.maximum.reduceat(P[o], st, axis=0)); Pmin[cl] = np.minimum(Pmin[cl], np.minimum.reduceat(P[o], st, axis=0))
        del P
        R = Xs - Mean[ls]; g = grp[ls]
        for gg in np.unique(g):
            Rg = R[g == gg]; Cov[gg] += Rg.T @ Rg; gcnt[gg] += len(Rg)
        del R
    Vb = np.empty((G, n, n), np.float32)
    for g in range(G):
        l_, v = np.linalg.eigh(Cov[g].astype(np.float64) / max(gcnt[g], 1)); Vb[g] = v[:, ::-1]
    rr = np.sqrt(np.maximum(0, 1 - 2 * xm + (mn ** 2)[np.maximum(lab, 0)]))
    np.savez(os.path.join(OUT, f"cl{j}.npz"), lab=lab, Mean=Mean, grp=grp, a=a, xm=xm, Pmax=Pmax, Pmin=Pmin, cnt=cnt)
    np.save(os.path.join(OUT, f"V{j}.npy"), Vb)
    log(f"clustering {j}: N={SIZES[j]} K={K} G={G} sizes min/med/max {cnt.min()}/{int(np.median(cnt))}/{cnt.max()} rms residual {np.sqrt((rr[inlab]**2).mean()):.4f}")
log("done")
