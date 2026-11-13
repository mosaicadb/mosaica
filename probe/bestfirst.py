# Lossless best-first search over a hierarchy of cluster centres. Leaf key = q . c + f * rms radius,
# as in the flat pass. Each internal node holds the mean M of its children and a bound radius
# br = max over children of (|child - M| + br(child)), with br(leaf) = f * radius, so for a unit
# query q . M + br bounds every leaf key beneath it. Popping a max-heap of nodes keyed by these
# bounds emits leaves in exactly the flat pass's order, so the clusters probed, and therefore the
# items scanned and the recall, are identical to the flat pass; only the number of centres scored
# differs. For each target recall we take the flat pass's probe count p and count the centres
# scored until p leaves have been emitted.
#
# usage: bestfirst.py <hdf5> K f p1 [p2 ...]
import sys, time, heapq, numpy as np, h5py, faiss

path, K, F = sys.argv[1], int(sys.argv[2]), float(sys.argv[3])
PS = [int(x) for x in sys.argv[4:]]
NQ = 1000
name = path.split('/')[-1].replace('.hdf5', '')
faiss.omp_set_num_threads(1)
t00 = time.time()
def log(*a): print(f'[{time.time() - t00:7.0f}s]', *a, flush=True)

f = h5py.File(path, 'r')
X = np.asarray(f['train'], dtype=np.float32); X /= np.maximum(np.linalg.norm(X, axis=1, keepdims=True), 1e-12)
Q = np.asarray(f['test'], dtype=np.float32)
nq = np.linalg.norm(Q, axis=1); Q = Q[nq > 0][:NQ] / nq[nq > 0][:NQ, None]
n = X.shape[1]
z = np.load(f'/home/joe/code/vexact-data/{name}/probe/K{K}.npz'); C = z['Cb']; a = z['ab']
cnt = np.bincount(a, minlength=K)
rad = np.sqrt(np.bincount(a, weights=((X - C[a]) ** 2).sum(1), minlength=K) / np.maximum(cnt, 1))
del X

def build(sizes):
    """sizes: node counts per internal level, finest first. Returns per-level (vectors, bound radii,
    children lists), coarsest first, plus leaf data."""
    vecs, brs, kids = [C], [F * rad], []
    for lvl, G in enumerate(sizes):
        V = vecs[-1]
        km = faiss.Kmeans(n, G, niter=25, seed=3 + lvl, max_points_per_centroid=100_000); km.train(V)
        _, pa = km.index.search(V, 1); pa = pa[:, 0]
        used = np.unique(pa); remap = -np.ones(G, int); remap[used] = np.arange(len(used)); pa = remap[pa]; G = len(used)
        M = np.stack([V[pa == g].mean(0) for g in range(G)]).astype(np.float32)
        br = np.zeros(G); np.maximum.at(br, pa, np.linalg.norm(V - M[pa], axis=1) + brs[-1])
        vecs.append(M); brs.append(br); kids.append([np.flatnonzero(pa == g) for g in range(G)])
    return vecs, brs, kids

def run(sizes):
    vecs, brs, kids = build(sizes)
    top = len(vecs) - 1
    res = {p: [] for p in PS}; inner = {p: [] for p in PS}
    pmax = max(PS)
    for q in Q:
        # heap entries: (-key, level, index); level 0 = leaf
        s = q @ vecs[top].T + brs[top]
        heap = [(-s[i], top, i) for i in range(len(s))]; heapq.heapify(heap)
        scored = len(s); scored_inner = len(s); emitted = 0
        while emitted < pmax:
            negk, lv, i = heapq.heappop(heap)
            if lv == 0:
                emitted += 1
                if emitted in res:
                    res[emitted].append(scored); inner[emitted].append(scored_inner)
                continue
            ch = kids[lv - 1][i]
            ks = vecs[lv - 1][ch] @ q + brs[lv - 1][ch]
            scored += len(ch)
            if lv - 1 > 0: scored_inner += len(ch)
            for c, kv in zip(ch, ks): heapq.heappush(heap, (-kv, lv - 1, c))
    return {p: (np.mean(res[p]), np.mean(inner[p]), np.percentile(res[p], 90)) for p in PS}

log(f'{name} K={K} f={F} queries={len(Q)} probe counts {PS}; flat pass scores {K} centres')
shapes = {
    'two levels, sqrt K': [int(round(K ** 0.5))],
    'three levels, cube root': [int(round(K ** (2 / 3))), int(round(K ** (1 / 3)))],
}
fan = 16; s = []; g = K
while g // fan >= 2: g //= fan; s.append(g)
shapes[f'fan-out {fan}, {len(s) + 1} levels'] = s
for label, sizes in shapes.items():
    r = run(sizes)
    log(f'{label} {sizes}: ' + '; '.join(f'p={p}: centres scored mean {m:.0f} (internal {mi:.0f}), p90 {p9:.0f}, {m / K:.2f} of flat'
                                         for p, (m, mi, p9) in r.items()))
log('done')
