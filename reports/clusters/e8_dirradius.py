# E8: a direction-aware radius in the cluster key. The flat key is q.c + f * rms radius, which assumes
# the cluster is round. Here the spread term is z * sqrt(q' S_c q), the standard deviation of q.r over
# the cluster's members, from (a) the full covariance, (b) its top r directions plus a round
# remainder, (c) one shared covariance per group of clusters scaled by the cluster's radius.
# Same harness as E1 (balanced k-means + SOAR; share scanned at recall targets).
#
# usage: e8_dirradius.py <dataset> K
import sys, numpy as np, faiss
sys.path.insert(0, "/tmp/claude-1000/-home-joe-code-tessera/44285a24-9b9a-44a0-8e3a-604c4b51c0e9/scratchpad/clusters")
from common import *

name, K = sys.argv[1], int(sys.argv[2])
NQ = 1000
faiss.omp_set_num_threads(4)
X, Q, T, G = load(name, NQ)
N, n = X.shape
z = np.load(f'{DATA}/{name}/probe/K{K}.npz'); C = z['Cb']; a1 = z['ab']; a2 = z['sb']
rad, cnt = cell_radius(X, C[a1], a1, K)
empty = cnt == 0
order = np.argsort(a1, kind='stable'); st = np.concatenate([[0], np.cumsum(cnt)])
S = np.zeros((K, n, n), np.float32)
for c in range(K):
    if cnt[c] == 0: continue
    R = X[order[st[c]:st[c + 1]]] - C[c]
    S[c] = R.T @ R / cnt[c]
log(f'{name} K={K}: covariances done; mean items per cluster {cnt.mean():.0f}')
w, U = np.linalg.eigh(S); w = w[:, ::-1]; U = U[:, :, ::-1]            # descending
log('share of residual energy in the top r directions of a cluster: ' + ', '.join(f'r={r}: {(w[:, :r].sum(1) / np.maximum(w.sum(1), 1e-12))[~empty].mean():.3f}' for r in (1, 2, 4, 8, 16)))
QC = Q @ C.T

def run(label, spread):
    """spread(lo, hi) -> (b, K) standard deviation of q.r for queries lo..hi."""
    for zz in ZS:
        pos = [0]
        def keys(Qb):
            s = pos[0]; pos[0] += len(Qb)
            key = QC[s:s + len(Qb)] + zz * spread(s, s + len(Qb)); key[:, empty] = -np.inf
            return key
        rec, su = curves(keys, Q, T, a1, a2, K, cnt)
        log(f'{label} z={zz}: {fmt(at_targets(rec, su, N))}')

ZS = (2.5, 3.5, 4.5) if n == 100 else (3.5, 5.0, 6.5)
run('round (rms radius / sqrt n)', lambda s, e: np.broadcast_to(rad[None, :] / np.sqrt(n), (e - s, K)))
SQ = np.empty((len(Q), K), np.float32)
for s in range(0, len(Q), 8):
    Qb = Q[s:s + 8]
    SQ[s:s + 8] = np.sqrt(np.maximum(np.einsum('bcn,bn->bc', np.tensordot(Qb, S, axes=([1], [1])), Qb), 0))
log(f'full covariance: sd of q.r across clusters for one query: mean {SQ[:, ~empty].mean():.4f}, relative spread {(SQ[:, ~empty].std(1) / SQ[:, ~empty].mean(1)).mean():.3f}; round value {(rad[~empty] / np.sqrt(n)).mean():.4f}')
run('full covariance', lambda s, e: SQ[s:e])
for r in (4, 8):
    Ur = np.ascontiguousarray(U[:, :, :r].transpose(0, 2, 1)); wr = w[:, :r]
    rest = np.maximum(w.sum(1) - wr.sum(1), 0) / (n - r)
    LR = np.empty((len(Q), K), np.float32)
    for s in range(0, len(Q), 8):
        p2 = np.einsum('bn,crn->bcr', Q[s:s + 8], Ur) ** 2
        LR[s:s + 8] = np.sqrt((p2 * wr[None]).sum(2) + rest[None, :] * np.maximum(1 - p2.sum(2), 0))
    run(f'top {r} directions + round remainder ({r} more dots per centre)', lambda s, e: LR[s:e])
for Gn in (16, 64):
    Cg = kmeans(C, Gn, niter=25, mpc=100000); _, I = nearest(C, Cg); g = I[:, 0]
    Sg = np.zeros((Gn, n, n), np.float32); wg = np.zeros(Gn)
    np.add.at(Sg, g, S * cnt[:, None, None]); np.add.at(wg, g, cnt)
    Sg /= np.maximum(wg, 1)[:, None, None]
    trg = np.einsum('gii->g', Sg)
    GS = np.sqrt(np.maximum(np.einsum('bgn,bn->bg', np.tensordot(Q, Sg, axes=([1], [1])), Q), 0))   # (nq, Gn)
    scale = rad / np.sqrt(np.maximum(trg[g], 1e-12))
    run(f'one covariance per group of clusters, {Gn} groups, scaled by cluster radius', lambda s, e: GS[s:e][:, g] * scale[None, :])
log('done')
