# Third code: the same stream (lb_run.py) in the geometric-mean "dual pair" metric of groups/scripts/measure.py
# (research/exact-vector-search 2c528d7b0) instead of each group's PCA basis. Data-only fit: Sigma_Q from corpus items as
# pseudo-queries against their 4 nearest centres (fit.py's convention), one global matrix, ridge 0.1 of its mean eigenvalue.
# Per group: eigen-decompose Aq Sigma_R Aq (Aq = Sigma_Q^1/2); item side VD = Aq v l^-1/4, query side VQ = Aq^-1 v l^1/4,
# so (q - m) VQ . r VD = (q - m).r. lam = variance of the item coordinates, profile = mean square of the pseudo-query ones.
# usage: adapt_gm.py WORKDIR  (reads index.npz from adapt.py and fit/, writes index_gm.npz)
import sys, json, numpy as np
W = sys.argv[1]; I = dict(np.load(W + "/index.npz")); F = W + "/fit/"
X = np.load(F + "Xs.npy").astype(np.float32); X /= np.maximum(np.linalg.norm(X, axis=1, keepdims=True), 1e-12)
N, n, K, G = int(I["N"]), int(I["n"]), int(I["K"]), int(I["G"]); Mq, assign, grp, cstart = I["Mq"], I["assign"], I["grp"], I["cstart"]
rng = np.random.default_rng(20261114)
pq = X[rng.choice(N, 5000, replace=False)]; pk = np.argsort(-(pq @ Mq.T), axis=1)[:, :4]
D = (pq[:, None, :] - Mq[pk]).reshape(-1, n).astype(np.float64); SQ = D.T @ D / len(D)
SQ += 0.1 * np.trace(SQ) / n * np.eye(n)
lq, vq = np.linalg.eigh(SQ); Aq = (vq * np.sqrt(lq)) @ vq.T; Aqi = (vq / np.sqrt(lq)) @ vq.T
V = np.zeros((G, n, n), np.float32); VQ = np.zeros((G, n, n), np.float32); lam = np.zeros((G, n)); prof = np.zeros((G, n))
gi = grp[assign]
for g in range(G):
    R = (X[gi == g] - Mq[assign[gi == g]]).astype(np.float64); cov = R.T @ R / max(len(R), 1)
    l, v = np.linalg.eigh(Aq @ cov @ Aq); l = np.maximum(l[::-1], 1e-12 * l.max()); v = v[:, ::-1]
    V[g] = (Aq @ v) * l ** -0.25; VQ[g] = (Aqi @ v) * l ** 0.25
    lam[g] = np.maximum(((R @ V[g]) ** 2).mean(0), 1e-12)
    rows, cols = np.where(grp[pk] == g)
    prof[g] = (((pq[rows] - Mq[pk[rows, cols]]) @ VQ[g]) ** 2).mean(0) if len(rows) > 50 else lam[g]
    print("group", g, "items", len(R), "sum sqrt(gamma)", round(float(np.sqrt(l).sum()), 4), flush=True)
I.update(V=V, VQ=VQ, lam=lam, Qi=prof, Qt=prof, Qmix=prof)
np.savez(W + "/index_gm.npz", **I); print("index_gm.npz written")
