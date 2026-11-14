# Build the shared index: 1M unit vectors, K0 spherical k-means clusters, nested 4-way splits to three finer levels,
# G groups of clusters. Follows the construction of research/exact-vector-search agent2/prep.py.
import numpy as np, time, sys, os
import scipy.sparse as sp
W = sys.argv[1]; NF = int(sys.argv[2]) if len(sys.argv) > 2 else 20
rng = np.random.default_rng(0); t0 = time.time()
def log(*a): print(f"[{time.time()-t0:5.0f}s]", *a, flush=True)
files = list(range(20)) if NF == 20 else [0, 5, 10, 15]
X = np.concatenate([np.load(f"{W}/data/e{i:02d}.npy").astype(np.float32) for i in files])
X /= np.linalg.norm(X, axis=1, keepdims=True); N, n = X.shape
K0 = N // 1000; G = max(12, round(64 * N / 1_000_000))
log("loaded", X.shape, "K0", K0, "G", G)

def sums(S, a, K):
    return np.asarray(sp.csr_matrix((np.ones(len(a), np.float32), (a, np.arange(len(a)))), shape=(K, len(a))) @ S)
def assign_to(S, C):
    return np.concatenate([(S[i:i+50_000] @ C.T).argmax(1) for i in range(0, len(S), 50_000)]).astype(np.int32)
def kmeans(S, K, iters=12):
    C = S[rng.choice(len(S), K, replace=False)].copy()
    for it in range(iters):
        a = assign_to(S, C); M = sums(S, a, K)
        cnt = np.bincount(a, minlength=K); empty = cnt == 0
        M[empty] = S[rng.choice(len(S), int(empty.sum()))]
        C = M / np.linalg.norm(M, axis=1, keepdims=True)
    return C
Chat = kmeans(X, K0); a0 = assign_to(X, Chat); log("k-means done; sizes min/med/max", np.bincount(a0, minlength=K0).min(), int(np.median(np.bincount(a0, minlength=K0))), np.bincount(a0, minlength=K0).max())

def split4(idx):
    # 4-way spherical k-means of one group's members; returns sub-labels 0..3
    S = X[idx]
    if len(idx) < 8: return np.zeros(len(idx), np.int32)
    C = S[rng.choice(len(idx), 4, replace=False)].copy()
    for it in range(8):
        a = (S @ C.T).argmax(1)
        for j in range(4):
            m = a == j
            C[j] = S[m].mean(0) if m.any() else S[rng.integers(len(idx))]
        C /= np.linalg.norm(C, axis=1, keepdims=True)
    return (S @ C.T).argmax(1).astype(np.int32)
levels = [a0]
for L in range(3):
    prev = levels[-1]; order = np.argsort(prev, kind="stable"); bounds = np.flatnonzero(np.diff(prev[order])) + 1
    new = np.zeros(N, np.int64)
    for idx in np.split(order, bounds):
        new[idx] = prev[idx[0]] * 4 + split4(idx)
    _, new = np.unique(new, return_inverse=True); levels.append(new.astype(np.int32))
    log("level", L + 1, "groups", new.max() + 1, "median size", int(np.median(np.bincount(new))))
Mean0 = sums(X, a0, K0) / np.bincount(a0, minlength=K0)[:, None]
Mh = Mean0 / np.linalg.norm(Mean0, axis=1, keepdims=True)
grp = (Mh @ kmeans(Mh.astype(np.float32), G, 20).T).argmax(1).astype(np.int32)
np.save(f"{W}/X.npy", X)
np.savez(f"{W}/index.npz", levels=np.stack(levels), grp=grp, Chat=Chat)
qi = np.sort(np.random.default_rng(5).choice(N, 100, replace=False)); np.save(f"{W}/item_q_idx.npy", qi)
log("done")
