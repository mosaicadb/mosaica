# Centre ranking: flat pass over K centres against two-level trees over them, on the plain k-means
# structure of expA.py (same k-means settings and seed), all test queries, ANN-Benchmarks recall.
# Top level: sqrt(K) groups from k-means over the centres. Groups ranked by q . g + f * radius(g),
# radius = largest distance from the group mean to a member centre plus that member's rms cluster
# radius, so f = 1 is a bound on q . x for every item under the group (data and queries are unit).
# Centres in the top t groups are ranked by q . centre; centres outside them are never probed.
# usage: expA_tree.py <hdf5> <outdir> K [K ...]
import sys, time, json, numpy as np, h5py, faiss

path, outdir, Ks = sys.argv[1], sys.argv[2], [int(x) for x in sys.argv[3:]]
name = path.split('/')[-1].replace('.hdf5', '')
faiss.omp_set_num_threads(1)
k = 10; EPS = 1e-3; TARGETS = (0.9, 0.95, 0.99)
t00 = time.time()
def log(*a): print(f'[{time.time() - t00:7.0f}s]', *a, flush=True)
f = h5py.File(path, 'r')
X = np.asarray(f['train'], dtype=np.float32); X /= np.maximum(np.linalg.norm(X, axis=1, keepdims=True), 1e-12)
Q = np.asarray(f['test'], dtype=np.float32); Q /= np.maximum(np.linalg.norm(Q, axis=1, keepdims=True), 1e-12)
gt_ids = np.asarray(f['neighbors']); gt_d = np.asarray(f['distances'])
N, n = X.shape; NQ = len(Q)
T = np.where(gt_d <= (gt_d[:, k - 1] + EPS)[:, None], gt_ids, -1)

out = {}
for K in Ks:
    km = faiss.Kmeans(n, K, niter=15, seed=1, verbose=False, max_points_per_centroid=256); km.train(X)
    C = km.centroids.copy(); _, a = km.index.search(X, 1); a = a[:, 0]
    cnt = np.bincount(a, minlength=K)
    crad = np.sqrt(np.bincount(a, weights=((X - C[a]) ** 2).sum(1), minlength=K) / np.maximum(cnt, 1))
    log(f'K={K} k-means done')
    # need p(R): flat clusters probed for each target, centre ordering
    S = Q @ C.T
    rank = np.argsort(np.argsort(-S, 1), 1)
    def recall_at(rank, p):
        r = np.where(T >= 0, np.take_along_axis(rank, a[np.maximum(T, 0)], 1), 10 ** 9)
        return (np.minimum((r < p).sum(1), k)).mean() / k
    flat_p = {}
    for R in TARGETS:
        lo, hi = 1, K
        while lo < hi:
            mid = (lo + hi) // 2
            if recall_at(rank, mid) >= R: hi = mid
            else: lo = mid + 1
        flat_p[R] = lo
    Gn = int(round(np.sqrt(K)))
    kmc = faiss.Kmeans(n, Gn, niter=25, seed=2, max_points_per_centroid=10_000); kmc.train(C)
    _, gc = kmc.index.search(C, 1); gc = gc[:, 0]
    GC = np.stack([C[gc == g].mean(0) if (gc == g).any() else kmc.centroids[g] for g in range(Gn)]).astype(np.float32)
    grad = np.zeros(Gn)
    np.maximum.at(grad, gc, np.linalg.norm(C - GC[gc], axis=1) + crad)
    gsz = np.bincount(gc, minlength=Gn)
    res = out[K] = dict(K=K, n=n, groups=Gn, group_size_max=int(gsz.max()), group_size_mean=float(gsz.mean()), flat_madds=K * n,
                        flat_nprobe={str(R): int(p) for R, p in flat_p.items()}, rows=[])
    GS = Q @ GC.T
    for fct in (0.0, 0.25, 0.5, 1.0):
        gorder = np.argsort(-(GS + fct * grad), 1)
        for t in (1, 2, 4, 8, 16, 32, 64):
            if t > Gn: break
            ing = np.zeros((NQ, Gn), bool); np.put_along_axis(ing, gorder[:, :t], True, 1)
            cand = ing[:, gc]
            rk = np.argsort(np.argsort(-np.where(cand, S, -np.inf), 1), 1)
            rk = np.where(cand, rk, 10 ** 9)
            row = dict(f=fct, t=t, centres_scored=float(cand.sum(1).mean()), madds=float((Gn + cand.sum(1).mean()) * n))
            for R, p in flat_p.items():
                row[f'recall_at_flat_p{R}'] = float(recall_at(rk, p))
            res['rows'].append(row)
            log(f'K={K} f={fct} t={t}: centres scored {row["centres_scored"]:.0f} of {K}, madds {row["madds"]:,.0f} vs flat {K * n:,}; '
                + ' '.join(f'R{R}@p={p}: {row[f"recall_at_flat_p{R}"]:.4f}' for R, p in flat_p.items()))
    json.dump(out, open(f'{outdir}/expA_tree_{name}.json', 'w'))
log('done')
