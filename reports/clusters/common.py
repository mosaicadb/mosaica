# Shared loading and evaluation for the cluster experiments. Conventions follow probe/expA.py:
# unit vectors, k = 10, ANN-Benchmarks tolerant recall (an item counts if 1 - cos <= d_k + 1e-3).
import time, numpy as np, h5py, faiss

DATA = '/home/joe/code/vexact-data'
k = 10; EPS = 1e-3
t00 = time.time()
def log(*a): print(f'[{time.time() - t00:6.0f}s]', *a, flush=True)

def load(name, nq=2000):
    f = h5py.File(f'{DATA}/{name}/{name}.hdf5', 'r')
    X = np.asarray(f['train'], dtype=np.float32); X /= np.maximum(np.linalg.norm(X, axis=1, keepdims=True), 1e-12)
    Q = np.asarray(f['test'], dtype=np.float32)
    gt_ids = np.asarray(f['neighbors']); gt_d = np.asarray(f['distances'])
    qn = np.linalg.norm(Q, axis=1); ok = np.flatnonzero(qn > 0)[:nq]
    Q = Q[ok] / qn[ok, None]; gt_ids = gt_ids[ok]; gt_d = gt_d[ok]
    thr = gt_d[:, k - 1] + EPS
    T = np.where(gt_d <= thr[:, None], gt_ids, -1)
    return X, Q, T, gt_ids[:, :k]

def kmeans(X, K, niter=15, seed=1, mpc=256):
    km = faiss.Kmeans(X.shape[1], K, niter=niter, seed=seed, verbose=False, max_points_per_centroid=mpc)
    km.train(X)
    return km.centroids.copy()

def nearest(X, C, L=1):
    idx = faiss.IndexFlatL2(C.shape[1]); idx.add(C)
    D, I = idx.search(X, L)
    return D, I

def curves(keys_fn, Q, T, a1, a2, Kc, cnt, B=8):
    """keys_fn(Qbatch) -> (b, Kc) keys (-inf for empty cells). Returns mean tolerant recall after probing
    p cells (p = 1..Kc) and mean unique items scanned after p cells."""
    rt = np.zeros(Kc + 2); su = np.zeros(Kc)
    for i in range(0, len(Q), B):
        key = keys_fn(Q[i:i + B]); b = len(key)
        order = np.argsort(-key, axis=1, kind='stable')
        rank = np.empty((b, Kc), dtype=np.int32)
        np.put_along_axis(rank, order, np.broadcast_to(np.arange(Kc, dtype=np.int32), (b, Kc)), 1)
        ids = T[i:i + B]; safe = np.maximum(ids, 0)
        r = np.take_along_axis(rank, a1[safe], 1).astype(np.int64)
        if a2 is not None:
            r = np.minimum(r, np.take_along_axis(rank, a2[safe], 1))
        r = np.where(ids >= 0, r, Kc)
        mt = np.sort(r, 1)[:, :k]
        rt += np.bincount((mt + 1).ravel(), minlength=Kc + 2)[:Kc + 2]
        if a2 is None:
            su += np.cumsum(cnt[order], 1).sum(0)
        else:
            for j in range(b):
                mr = np.minimum(rank[j][a1], rank[j][a2])
                su += np.cumsum(np.bincount(mr, minlength=Kc)[:Kc])
    nq = len(Q)
    return np.cumsum(rt)[1:Kc + 1] / nq / k, su / nq

def at_targets(rec, su, N, targets=(0.9, 0.95, 0.99)):
    out = []
    for t in targets:
        p = int(np.searchsorted(rec, t))
        out.append((None, None) if p >= len(rec) else (su[p] / N, p + 1))
    return out

def fmt(res):
    return ' | '.join('n/a' if s is None else f'{100 * s:.2f}% (p={p})' for s, p in res)

def cell_radius(X, C_of_item, a, Kc):
    cnt = np.bincount(a, minlength=Kc)
    d2 = ((X - C_of_item) ** 2).sum(1)
    return np.sqrt(np.bincount(a, weights=d2, minlength=Kc) / np.maximum(cnt, 1)), cnt
