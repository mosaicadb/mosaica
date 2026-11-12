import numpy as np, time
rng = np.random.default_rng(3)
t0 = time.time()
X = np.concatenate([np.load(f"data/e{i:02d}.npy") for i in (0, 5, 10, 15)]).astype(np.float32)
X /= np.linalg.norm(X, axis=1, keepdims=True)
N, d = X.shape
g = X.mean(0)
Xc = X - g; Xc /= np.linalg.norm(Xc, axis=1, keepdims=True)     # centred sphere
NQ = 40
qi = rng.choice(N, NQ, replace=False)
true = X @ X[qi].T
for m, s in ((16384, 32), (65536, 32)):
    W = rng.standard_normal((m, d)).astype(np.float32); W /= np.linalg.norm(W, axis=1, keepdims=True)
    J = np.empty((N, s), np.int32); beta = np.empty(N, np.float32); top = np.empty(N, np.float32); align = np.empty(N, np.float32)
    CH = 4000 if m > 20000 else 10000
    for i in range(0, N, CH):
        F = Xc[i:i+CH] @ W.T
        idx = np.argpartition(F, m - s, axis=1)[:, m - s:]
        v = np.take_along_axis(F, idx, 1)
        J[i:i+CH] = idx; beta[i:i+CH] = v.min(1); top[i:i+CH] = v.max(1)
        dec = W[idx].sum(1); dec /= np.linalg.norm(dec, axis=1, keepdims=True)
        align[i:i+CH] = (dec * Xc[i:i+CH]).sum(1)
    print(f"[{time.time()-t0:.0f}s] m={m} s={s}: best feature cos median {np.median(top):.3f}, s-th {np.median(beta):.3f}; "
          f"decoded-code alignment cos median {np.median(align):.3f} (certified angle {np.degrees(np.arccos(np.median(align))):.0f} deg)", flush=True)
    for t in (0.95, 0.9, 0.8, 0.7):
        rows = []
        for j in range(NQ):
            mask = np.zeros(m, bool); mask[J[qi[j]]] = True
            ov = mask[J].sum(1)
            A = true[:, j] >= t
            if A.sum() < 5: continue
            T = ov[A].min()
            rows.append((A.sum(), np.median(ov[A]), T, np.median(ov[~A]), (ov >= max(T, 1)).mean() if T > 0 else 1.0,
                         (ov >= np.quantile(ov[A], 0.1)).sum() / A.sum(), (A & (ov >= 1)).sum() / A.sum()))
        R = np.array(rows, float)
        print(f"   t={t}: answers median {np.median(R[:,0]):.0f}; shared features of {s}: matches median {np.median(R[:,1]):.0f}, "
              f"worst match {np.median(R[:,2]):.0f} (zero in {np.mean(R[:,2]==0)*100:.0f}% of queries), background median {np.median(R[:,3]):.0f}; "
              f"candidates needed to keep every match: {np.median(R[:,4])*100:.1f}% of corpus; "
              f"recall if reading only the query's {s} lists: {np.median(R[:,6])*100:.1f}%", flush=True)
