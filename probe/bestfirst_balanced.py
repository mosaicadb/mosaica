# Two-level lossless tree with balanced groups of centres: how many centres must be scored.
import sys, numpy as np, h5py, faiss
path, K = sys.argv[1], int(sys.argv[2]); PS = [int(x) for x in sys.argv[3:]]
name = path.split('/')[-1].replace('.hdf5', '')
f = h5py.File(path, 'r')
X = np.asarray(f['train'], dtype=np.float32); X /= np.maximum(np.linalg.norm(X, axis=1, keepdims=True), 1e-12)
Q = np.asarray(f['test'][:1000], dtype=np.float32); Q = Q[np.linalg.norm(Q, axis=1) > 0]; Q /= np.linalg.norm(Q, axis=1, keepdims=True)
z = np.load(f'/home/joe/code/vexact-data/{name}/probe/K{K}.npz'); C = z['Cb']; a = z['ab']
rad = np.sqrt(np.bincount(a, weights=((X - C[a]) ** 2).sum(1), minlength=K) / np.maximum(np.bincount(a, minlength=K), 1))
lk = Q @ C.T + 0.35 * rad; srt = -np.sort(-lk, 1)
for G in (int(round(K ** 0.5)), 4 * int(round(K ** 0.5))):
    cap = int(np.ceil(K / G))
    km = faiss.Kmeans(C.shape[1], G, niter=25, seed=3); km.train(C)
    D, I = km.index.search(C, G)
    pa = -np.ones(K, int); room = np.full(G, cap)
    for r in np.argsort(D[:, 0]):              # closest centres choose first
        for g in I[r]:
            if room[g] > 0: pa[r] = g; room[g] -= 1; break
    for _ in range(5):                          # re-average, re-assign under the cap
        M = np.stack([C[pa == g].mean(0) for g in range(G)])
        D, I = faiss.knn(C, M.astype(np.float32), G)
        pa[:] = -1; room[:] = cap
        for r in np.argsort(D[:, 0]):
            for g in I[r]:
                if room[g] > 0: pa[r] = g; room[g] -= 1; break
    M = np.stack([C[pa == g].mean(0) for g in range(G)])
    br = np.zeros(G); np.maximum.at(br, pa, np.linalg.norm(C - M[pa], axis=1) + 0.35 * rad)
    gb = Q @ M.T + br
    out = []
    for p in PS:
        thr = srt[:, p - 1]
        opened = gb >= thr[:, None]               # exactly the groups best-first must expand
        scored = G + (opened * cap).sum(1)
        out.append(f'p={p}: groups opened {opened.sum(1).mean():.1f} of {G}, centres scored {scored.mean():.0f} ({scored.mean() / K:.2f} of flat)')
    print(f'{name} K={K} G={G} cap {cap}; group bound radius median {np.median(br):.3f}, leaf key sd {lk.std():.3f}: ' + '; '.join(out), flush=True)
