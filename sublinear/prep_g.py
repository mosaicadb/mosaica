# Variant for the ladder: the same clusterings with the number of local bases held at 12 for every N, to separate the
# effect of N from the effect of more (and more local) bases. Clusterings 0-2 already have 12 groups.
import numpy as np, os
import scipy.sparse as sp
from common import *
rng = np.random.default_rng(7)
def sums(S, a, K):
    return np.asarray(sp.csr_matrix((np.ones(len(a), np.float32), (a, np.arange(len(a)))), shape=(K, len(a))) @ S)
def kmeans_small(S, K, iters=20):
    Cc = S[rng.choice(len(S), K, replace=False)].copy()
    for _ in range(iters):
        a = (S @ Cc.T).argmax(1); Mm = sums(S, a, K); e = np.bincount(a, minlength=K) == 0
        Mm[e] = S[rng.choice(len(S), int(e.sum()))]; Cc = Mm / np.linalg.norm(Mm, axis=1, keepdims=True)
    return Cc
G = 12; JS = [3, 4, 5]; D = {j: np.load(os.path.join(OUT, f"cl{j}.npz")) for j in JS}
lab = {j: D[j]["lab"] for j in JS}; Mean = {j: D[j]["Mean"] for j in JS}
grp = {}
for j in JS:
    Mh = Mean[j] / np.linalg.norm(Mean[j], axis=1, keepdims=True); grp[j] = (Mh @ kmeans_small(Mh, G).T).argmax(1).astype(np.int32)
Cov = {j: np.zeros((G, n, n), np.float32) for j in JS}; cnt = {j: np.zeros(G, np.int64) for j in JS}
for i, X in chunks():
    for j in JS:
        l = lab[j][i:i + len(X)]; m = l >= 0; R = X[m] - Mean[j][l[m]]; g = grp[j][l[m]]
        for gg in np.unique(g):
            Rg = R[g == gg]; Cov[j][gg] += Rg.T @ Rg; cnt[j][gg] += len(Rg)
for j in JS:
    Vb = np.empty((G, n, n), np.float32)
    for g in range(G):
        l_, v = np.linalg.eigh(Cov[j][g].astype(np.float64) / max(cnt[j][g], 1)); Vb[g] = v[:, ::-1]
    np.save(os.path.join(OUT, f"Vg{j}.npy"), Vb); np.save(os.path.join(OUT, f"grpg{j}.npy"), grp[j]); log("bases for clustering", j, "group sizes", cnt[j].tolist())
