# Confidence-dial experiment on glove-100-angular: IVF + b-bit rotated residual code with a per-item error sd,
# keep items whose estimate + m*sd reaches the running k-th best exact score, re-rank in bound order.
import sys, time, numpy as np, h5py
path = sys.argv[1]; NQ = 200; K = int(sys.argv[2]); k = 10
rng = np.random.default_rng(1)
f = h5py.File(path, 'r')
X = np.asarray(f['train'], dtype=np.float32); X /= np.maximum(np.linalg.norm(X, axis=1, keepdims=True), 1e-12)
Q = np.asarray(f['test'][:NQ], dtype=np.float32); Q /= np.maximum(np.linalg.norm(Q, axis=1, keepdims=True), 1e-12)
N, n = X.shape
t0 = time.time()
S = Q @ X.T                                   # exact scores
gt = np.argpartition(-S, k, axis=1)[:, :k]
tk = np.take_along_axis(S, gt, 1).min(1)      # exact k-th score
# k-means on a sample
smp = X[rng.choice(N, min(N, 200_000), replace=False)]
C = smp[rng.choice(len(smp), K, replace=False)].copy()
for it in range(8):
    a = np.concatenate([np.argmax(smp[i:i+50000] @ C.T - 0.5 * (C * C).sum(1), axis=1) for i in range(0, len(smp), 50000)])
    for c in range(0, K):
        pass
    sums = np.zeros_like(C); np.add.at(sums, a, smp); cnt = np.bincount(a, minlength=K)
    C[cnt > 0] = sums[cnt > 0] / cnt[cnt > 0, None]
a = np.concatenate([np.argmax(X[i:i+50000] @ C.T - 0.5 * (C * C).sum(1), axis=1) for i in range(0, N, 50000)])
R = X - C[a]; rn = np.linalg.norm(R, axis=1)
P = np.linalg.qr(rng.standard_normal((n, n)))[0]; d = np.sign(np.diag(np.linalg.qr(rng.standard_normal((n, n)))[1]))
O = np.nan_to_num(R / np.maximum(rn, 1e-12)[:, None]) @ P.T                   # rotated unit residuals
PQ = Q @ P.T
qc = Q @ C.T
crank = np.argsort(np.argsort(-qc, axis=1), axis=1)    # rank of each cluster per query
print(f'N={N} n={n} K={K} prep {time.time()-t0:.0f}s  mean |r|={rn.mean():.3f}  mean kth score={tk.mean():.3f}', flush=True)

def code(O, b):
    if b == 1:
        return np.sign(O).astype(np.float32)
    L = 2 ** b - 1; best = None
    for s in np.linspace(0.6, 3.0, 9) * np.sqrt(n) * (L / 3.0) / 2:
        Cd = np.clip(2 * np.floor(O[:20000] * s / 2) + 1, -L, L)
        cs = ((Cd * O[:20000]).sum(1) / np.linalg.norm(Cd, axis=1)).mean()
        if best is None or cs > best[0]: best = (cs, s)
    s = best[1]
    return np.clip(2 * np.floor(O * s / 2) + 1, -L, L).astype(np.float32)

NS_SCAN = {b: 0.3 + 0.066 * b * 256 / 8 for b in (1, 2, 4)}
NS_RERANK = 60.0; US_FIXED = 5.0 + K * 0.025             # random 100-d exact dot; centroid ranking + tables
for b in (1, 2):
    Cd = code(O, b); cn = np.maximum(np.linalg.norm(Cd, axis=1), 1e-12)
    cos = (Cd * O).sum(1) / cn
    fac = rn / (cn * cos)
    sd = rn * np.sqrt(np.maximum(1 - cos * cos, 0)) / cos / np.sqrt(n - 1)
    bad = ~np.isfinite(sd) | ~np.isfinite(fac); sd[bad] = 0; fac[bad] = 0; cos[bad] = 1
    E = (PQ @ Cd.T) * fac[None, :] + np.take_along_axis(qc, np.broadcast_to(a, (NQ, N)), 1)
    z = (E - S)[:, sd > 0] / sd[None, sd > 0]
    print(f'b={b}: {b*n/8:.1f} B/item, mean cos {cos.mean():.4f}, mean sd {sd.mean():.4f}, err/sd: sd {z.std():.3f} share |z|>3 {(np.abs(z) > 3).mean():.2e} >5 {(np.abs(z) > 5).mean():.2e} >9.35 {(np.abs(z) > 9.35).mean():.2e}; items with sd<1e-4: {(sd < 1e-4).sum()}', flush=True)
    for nprobe in (K//64, K//16, K//4, K):
        for m in (0.0, 1.0, 2.0, 3.0, 9.35):
            scanned = rer = 0; rec = 0; rec_eps = 0
            for qi in range(NQ):
                idx = np.flatnonzero(crank[qi, a] < nprobe) if nprobe < K else np.arange(N)
                e = E[qi, idx]; ub = e + m * sd[idx]; sc = S[qi, idx]
                ns = min(100 if m == 0.0 else 30, len(idx) - 1)
                seed = np.argpartition(-e, ns)[:ns]
                tau = np.sort(sc[seed])[-k]
                if m == 0.0:
                    surv = seed; cnt = len(seed)
                else:
                    surv = np.union1d(np.flatnonzero(ub >= tau), seed)
                    tau1 = np.sort(sc[surv])[-k]
                    cnt = 30 + int((ub >= tau1).sum())        # what the walk reads once the threshold has risen
                bid = idx[surv[np.argsort(-sc[surv])[:k]]]
                scanned += len(idx); rer += cnt
                rec += len(np.intersect1d(bid, gt[qi])); rec_eps += int((S[qi, bid] >= tk[qi] - 1e-3).sum())
            us = US_FIXED + (scanned * NS_SCAN[b] + rer * NS_RERANK) / NQ / 1000
            print(f'  nprobe={nprobe:5d} m={m:5.2f} scanned={scanned/NQ:9.0f} rerank={rer/NQ:8.0f} recall={rec/NQ/k:.4f} recall_eps={rec_eps/NQ/k:.4f} model={us:7.0f} us -> {1e6/us:6.0f} QPS', flush=True)
    del E, z, Cd
