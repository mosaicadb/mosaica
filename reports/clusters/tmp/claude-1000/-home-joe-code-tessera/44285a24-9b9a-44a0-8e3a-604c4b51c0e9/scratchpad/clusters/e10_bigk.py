# E10: does a much larger K keep paying once centres are cheap to score? Plain k-means with
# K = k1 * k2 real centres (k-means inside each of k1 top clusters, then one Lloyd pass over all
# centres), SOAR second assignment, key = q.c + f rad. Same harness and queries as E1.
#
# usage: e10_bigk.py <dataset> k1 k2
import sys, numpy as np, faiss
sys.path.insert(0, "/tmp/claude-1000/-home-joe-code-tessera/44285a24-9b9a-44a0-8e3a-604c4b51c0e9/scratchpad/clusters")
from common import *

name, k1, k2 = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
faiss.omp_set_num_threads(4)
X, Q, T, G = load(name, 1000)
N, n = X.shape
C1 = kmeans(X, k1); _, I = nearest(X, C1); t = I[:, 0]
parts = []
for i in range(k1):
    Xi = X[t == i]
    kk = min(k2, max(1, len(Xi) // 4))
    km = faiss.Kmeans(n, kk, niter=10, seed=3, verbose=False, min_points_per_centroid=1); km.train(Xi)
    parts.append(km.centroids.copy())
C = np.concatenate(parts); K = len(C)
log(f'{name}: {K} centres from {k1} x {k2}')
_, I = nearest(X, C); a = I[:, 0]
S = np.zeros_like(C); np.add.at(S, a, X); c = np.bincount(a, minlength=K); C[c > 0] = S[c > 0] / c[c > 0, None]
log('one Lloyd pass done')
D, I = nearest(X, C, 8); a1 = I[:, 0]
a2 = np.empty(N, dtype=np.int64)
for i in range(0, N, 20000):
    x = X[i:i + 20000]; r = x - C[a1[i:i + 20000]]; rr = (r * r).sum(1)
    diff = x[:, None, :] - C[I[i:i + 20000]]
    cost = (diff * diff).sum(2) + np.einsum('bln,bn->bl', diff, r) ** 2 / np.maximum(rr, 1e-12)[:, None]
    cost[:, 0] = np.inf
    a2[i:i + 20000] = I[i:i + 20000][np.arange(len(x)), cost.argmin(1)]
rad, cnt = cell_radius(X, C[a1], a1, K)
empty = cnt == 0
log(f'assigned; nonempty {int((~empty).sum())}, mean size {cnt[~empty].mean():.1f}, max {cnt.max()}, mean rad {np.average(rad, weights=cnt):.3f}')
np.savez(f'/tmp/claude-1000/-home-joe-code-tessera/44285a24-9b9a-44a0-8e3a-604c4b51c0e9/scratchpad/clusters/bigk_{name}_{K}.npz', C=C, a1=a1, a2=a2)
for f in (0.2, 0.35):
    def keys(Qb):
        key = Qb @ C.T + f * rad[None, :]; key[:, empty] = -np.inf
        return key
    rec, su = curves(keys, Q, T, a1, None, K, cnt)
    log(f'K={K} single assignment f={f}: {fmt(at_targets(rec, su, N))}')
    rec, su = curves(keys, Q, T, a1, a2, K, cnt)
    log(f'K={K} + SOAR f={f}: {fmt(at_targets(rec, su, N))}')
log('done')
