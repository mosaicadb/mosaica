# E9: can a group's local basis be a small rotation inside a shared global subspace? If so the query
# is projected once (n x D), then each opened group costs D x D instead of n x n.
# For each group of clusters: share of the group's residual energy held by its top j local directions,
# (a) unrestricted (the group's own PCA in n dimensions), (b) PCA restricted to the top D directions
# of the pooled residuals, (c) the pooled (global) basis itself with no local rotation.
#
# usage: e9_nestedbasis.py K G
import sys, numpy as np, faiss
sys.path.insert(0, "/tmp/claude-1000/-home-joe-code-tessera/44285a24-9b9a-44a0-8e3a-604c4b51c0e9/scratchpad/clusters")
from common import *

K, Gn = int(sys.argv[1]), int(sys.argv[2])
faiss.omp_set_num_threads(4)
X = np.concatenate([np.load(f'{DATA}/bioclip2-200k/g{i}.npy').astype(np.float32) for i in range(4)])
X /= np.linalg.norm(X, axis=1, keepdims=True)
N, n = X.shape
C = kmeans(X, K, niter=20); _, I = nearest(X, C); a = I[:, 0]
Cg = kmeans(C, Gn, niter=25, mpc=100000); _, I = nearest(C, Cg); g = I[:, 0][a]
R = X - C[a]
S0 = R.T @ R / N
w0, V0 = np.linalg.eigh(S0); V0 = V0[:, ::-1]; w0 = w0[::-1]
log(f'bioclip N={N} n={n} K={K} groups={Gn}; mean |r| {np.linalg.norm(R, axis=1).mean():.3f}; pooled residual energy in top D: ' + ', '.join(f'D={D}: {w0[:D].sum() / w0.sum():.3f}' for D in (32, 64, 128, 256)))
JS = (4, 8, 16, 32, 64)
acc = {('own', None): np.zeros(len(JS)), ('global', None): np.zeros(len(JS))}
DS = (64, 128, 256)
for D in DS: acc[('nested', D)] = np.zeros(len(JS))
wt = 0
for gi in range(Gn):
    Rg = R[g == gi]
    if len(Rg) < 2000: continue
    Sg = Rg.T @ Rg / len(Rg); tr = np.trace(Sg); wgt = len(Rg); wt += wgt
    w = np.linalg.eigvalsh(Sg)[::-1]
    acc[('own', None)] += wgt * np.array([w[:j].sum() / tr for j in JS])
    dg = np.einsum('in,nm,im->i', V0.T, Sg, V0.T)          # variance along each global direction
    acc[('global', None)] += wgt * np.array([dg[:j].sum() / tr for j in JS])
    for D in DS:
        Sr = V0[:, :D].T @ Sg @ V0[:, :D]
        wr = np.linalg.eigvalsh(Sr)[::-1]
        acc[('nested', D)] += wgt * np.array([wr[:j].sum() / tr for j in JS])
log('share of a group\'s residual energy in its top j coded directions, j = ' + ', '.join(str(j) for j in JS))
for key, v in acc.items():
    label = {'own': 'own basis (n x n per group)', 'global': 'pooled basis, no local rotation', 'nested': f'rotation inside the pooled top {key[1]} (D x D per group)'}[key[0]]
    log(f'  {label:>55}: ' + ', '.join(f'{x:.3f}' for x in v / wt))
for D in DS:
    log(f'  projection cost for 64 groups at n=768: nested D={D}: {(n * D + 64 * D * D) / 1e6:.2f}M multiply-adds; own bases: {64 * n * n / 1e6:.1f}M')
log('done')
