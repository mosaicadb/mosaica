# K-means in the geometric-mean metric: items are assigned to the centre nearest in the norm the certificate pays for,
# r' M^-1 r with M the geometric mean of the inverse query-offset covariance and the pooled residual covariance.
import numpy as np, time, sys
import scipy.sparse as sp
W, SQF, OUT = sys.argv[1], sys.argv[2], sys.argv[3]; EPS = float(sys.argv[4]) if len(sys.argv) > 4 else 0.1
rng = np.random.default_rng(0); t0 = time.time()
def log(*a): print(f"[{time.time()-t0:5.0f}s]", *a, flush=True)
X = np.load(f"{W}/X.npy"); N, n = X.shape; ix = np.load(f"{W}/index.npz"); a0 = ix["levels"][0]; K = int(a0.max()) + 1; G = int(ix["grp"].max()) + 1
def sums(S, a, K): return np.asarray(sp.csr_matrix((np.ones(len(a), np.float32), (a, np.arange(len(a)))), shape=(K, len(a))) @ S)
Mean = sums(X, a0, K) / np.bincount(a0, minlength=K)[:, None]
cov = np.zeros((n, n))
for i in range(0, N, 100_000):
    R = X[i:i+100_000] - Mean[a0[i:i+100_000]]; cov += (R.T @ R).astype(np.float64)
cov /= N
SQ = np.load(SQF).astype(np.float64); SQ += EPS * np.trace(SQ) / n * np.eye(n)
lq, vq = np.linalg.eigh(SQ); A = (vq * np.sqrt(lq)) @ vq.T
l, v = np.linalg.eigh(A @ cov @ A); l = np.maximum(l, 1e-12 * l.max()); T = ((A @ v) * l ** -0.25).astype(np.float32)
Y = np.empty((N, n), np.float32)
for i in range(0, N, 100_000): Y[i:i+100_000] = X[i:i+100_000] @ T
del X; log("transformed")
C = Y[rng.choice(N, K, replace=False)].copy()
def assign(C):
    h = 0.5 * (C * C).sum(1)
    return np.concatenate([(Y[i:i+50_000] @ C.T - h).argmax(1) for i in range(0, N, 50_000)]).astype(np.int32)
for it in range(12):
    a = assign(C); cnt = np.bincount(a, minlength=K); M = sums(Y, a, K); e = cnt == 0
    C = M / np.maximum(cnt, 1)[:, None]; C[e] = Y[rng.choice(N, int(e.sum()))]
a = assign(C); _, a = np.unique(a, return_inverse=True); a = a.astype(np.int32); K = int(a.max()) + 1; cnt = np.bincount(a)
log("k-means done; sizes min/med/max", cnt.min(), int(np.median(cnt)), cnt.max(), "objective", float(np.mean([((Y[i:i+50_000] - C[a[i:i+50_000]]) ** 2).sum(1).mean() for i in range(0, N, 50_000)])))
del Y; X = np.load(f"{W}/X.npy"); Mean = sums(X, a, K) / cnt[:, None]; Mh = (Mean / np.linalg.norm(Mean, axis=1, keepdims=True)).astype(np.float32)
def skm(S, K, iters=20):
    C = S[rng.choice(len(S), K, replace=False)].copy()
    for it in range(iters):
        b = (S @ C.T).argmax(1); M = sums(S, b, K); e = np.bincount(b, minlength=K) == 0
        M[e] = S[rng.choice(len(S), int(e.sum()))]; C = M / np.linalg.norm(M, axis=1, keepdims=True)
    return C
grp = (Mh @ skm(Mh, G).T).argmax(1).astype(np.int32)
np.savez(OUT, levels=np.stack([a, a, a, a]), grp=grp, Chat=Mh); log("done")
