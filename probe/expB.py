# Experiment B: fit the confidence dial m on corpus pseudo-queries, evaluate on real queries.
#
# usage: expB.py <dataset> <outdir> K
#   dataset: path to an ANN-Benchmarks angular HDF5, or a directory of BioCLIP g*.npy row groups.
# Index: plain k-means (faiss), one global random rotation of the residual, b-bit RaBitQ-style code,
# per-item sd = |r| tan(phi) / sqrt(n-1). Every cluster is scanned, so recall depends on m alone.
# Query: seed = 30 best estimates, exactly scored; then a walk in decreasing estimate + m*sd that
# re-ranks each item until its bound falls below the running k-th best exact score.
import sys, os, time, json, glob, numpy as np, faiss

path, outdir, K = sys.argv[1], sys.argv[2], int(sys.argv[3])
faiss.omp_set_num_threads(1)
k = 10; EPS = 1e-3; NSEED = 30; NPSEUDO = 2000
M_GRID = np.round(np.arange(0, 4.01, 0.25), 2)
TARGETS = (0.9, 0.95, 0.99, 0.999)
t00 = time.time()
def log(*a): print(f'[{time.time() - t00:7.0f}s]', *a, flush=True)
def unit(A): A = A.astype(np.float32); return A / np.maximum(np.linalg.norm(A, axis=1, keepdims=True), 1e-12)

rng = np.random.default_rng(7)
if path.endswith('.hdf5'):
    import h5py
    f = h5py.File(path, 'r'); name = os.path.basename(path).replace('.hdf5', '')
    X = unit(np.asarray(f['train'])); Q = unit(np.asarray(f['test']))
    nz = np.flatnonzero(np.linalg.norm(np.asarray(f['train']), axis=1) > 0)
    held = rng.choice(nz, NPSEUDO, replace=False)
    P = X[held]; keep = np.ones(len(X), bool); keep[held] = False; X = X[keep]
    qnote = 'ANN-Benchmarks test queries'
else:
    name = 'bioclip2-200k'
    groups = [np.load(p) for p in sorted(glob.glob(f'{path}/g*.npy'))]
    # pseudo-queries held out of row groups 0-2; "real" queries are items of row group 3, also held out
    A = unit(np.concatenate(groups[:3])); B3 = unit(groups[3])
    held = rng.choice(len(A), NPSEUDO, replace=False); held3 = rng.choice(len(B3), NPSEUDO, replace=False)
    P = A[held]; Q = B3[held3]
    ka = np.ones(len(A), bool); ka[held] = False; kb = np.ones(len(B3), bool); kb[held3] = False
    X = np.concatenate([A[ka], B3[kb]])
    qnote = 'held-out items of a fourth row group (BioCLIP-2 text queries not available)'
NQLIM = int(os.environ.get('NQLIM', 0))
if NQLIM: P = P[:NQLIM]; Q = Q[:NQLIM]
N, n = X.shape
log(f'{name}: base N={N} n={n} K={K}; pseudo-queries {len(P)}; test queries {len(Q)} ({qnote})')

km = faiss.Kmeans(n, K, niter=15, seed=1, max_points_per_centroid=256); km.train(X)
C = km.centroids.copy(); _, a = km.index.search(X, 1); a = a[:, 0]
R = X - C[a]; rn = np.linalg.norm(R, axis=1)
G = rng.standard_normal((n, n)); Qr, Rr = np.linalg.qr(G); Rot = (Qr * np.sign(np.diag(Rr))).astype(np.float32)
O = (R / np.maximum(rn, 1e-12)[:, None]) @ Rot.T
del R
log(f'index built; mean |r| {rn.mean():.3f}; items with |r| < 1e-6: {(rn < 1e-6).sum()}')

def code(O, b):
    if b == 1:
        return np.sign(O).astype(np.float32)
    Lv = 2 ** b - 1; best = None
    for s in np.linspace(0.6, 3.0, 9) * np.sqrt(n) * (Lv / 3.0) / 2:
        Cd = np.clip(2 * np.floor(O[:20000] * s / 2) + 1, -Lv, Lv)
        cs = ((Cd * O[:20000]).sum(1) / np.linalg.norm(Cd, axis=1)).mean()
        if best is None or cs > best[0]: best = (cs, s)
    return np.clip(2 * np.floor(O * best[1] / 2) + 1, -Lv, Lv).astype(np.float32)

def walk(e, sd, sc, m, tol_thr, gt):
    """One query at one m. e, sd, sc: estimate, sd and exact score over the pool (all items whose
    bound at the largest m reaches the seed threshold, plus the seeds). Returns (re-ranks, tol hits, strict hits)."""
    seed = np.argpartition(-e, NSEED - 1)[:NSEED]
    tau0 = np.partition(sc[seed], NSEED - k)[NSEED - k]
    ub = e + m * sd
    cand = np.flatnonzero(ub >= tau0)
    cand = cand[np.argsort(-ub[cand], kind='stable')]
    isseed = np.zeros(len(e), bool); isseed[seed] = True
    cand = cand[~isseed[cand]]
    def kth(j):                     # k-th best exact score among seeds and the first j walked items
        v = np.concatenate([sc[seed], sc[cand[:j]]]); return np.partition(v, len(v) - k)[len(v) - k]
    lo, hi = 0, len(cand)           # find the first j whose bound is below the k-th best of what came before
    while lo < hi:
        mid = (lo + hi) // 2
        if ub[cand[mid]] < kth(mid): hi = mid
        else: lo = mid + 1
    got = np.concatenate([seed, cand[:lo]])
    top = got[np.argsort(-sc[got])[:k]]
    return NSEED + lo, int((sc[top] >= tol_thr).sum()), len(np.intersect1d(top, gt))

def run(Qs, Cd, fac, sd, label, B=25):
    """Per query and m: re-ranks, tolerant hits, strict hits."""
    nq = len(Qs); rr = np.zeros((nq, len(M_GRID)), np.int64); th = np.zeros_like(rr); sh = np.zeros_like(rr)
    zs = []
    PQ = Qs @ Rot.T
    for i in range(0, nq, B):
        q = Qs[i:i + B]
        S = q @ X.T
        E = (PQ[i:i + B] @ Cd.T) * fac[None, :] + (q @ C.T)[:, a]
        if i < 200:
            ok = sd > 0; zs.append(((E - S)[:, ok] / sd[None, ok]).ravel()[::7])
        for j in range(len(q)):
            s = S[j]; e = E[j]
            gt = np.argpartition(-s, k)[:k]; sk = s[gt].min(); tol_thr = sk - EPS
            seed = np.argpartition(-e, NSEED - 1)[:NSEED]
            tau0 = np.partition(s[seed], NSEED - k)[NSEED - k]
            pool = np.union1d(np.flatnonzero(e + M_GRID[-1] * sd >= tau0), seed)
            gtp = np.searchsorted(pool, gt[np.isin(gt, pool)])
            for mi, m in enumerate(M_GRID):
                rr[i + j, mi], th[i + j, mi], sh[i + j, mi] = walk(e[pool], sd[pool], s[pool], m, tol_thr, gtp)
        if i % 1000 == 0: log(f'  {label}: {i + len(q)} queries')
    z = np.concatenate(zs)
    zst = dict(sd=float(z.std()), gt2=float((np.abs(z) > 2).mean()), gt3=float((np.abs(z) > 3).mean()), gt4=float((np.abs(z) > 4).mean()), n=int(len(z)))
    return rr, th, sh, zst

out = dict(name=name, N=N, n=n, K=K, queries=qnote, n_pseudo=len(P), n_test=len(Q), m_grid=M_GRID.tolist(), codes={})
for b in (1, 2):
    Cd = code(O, b); cn = np.maximum(np.linalg.norm(Cd, axis=1), 1e-12)
    cos = (Cd * O).sum(1) / cn
    fac = rn / (cn * cos)
    sd = rn * np.sqrt(np.maximum(1 - cos * cos, 0)) / cos / np.sqrt(n - 1)
    bad = ~np.isfinite(sd) | ~np.isfinite(fac) | (rn < 1e-6); sd[bad] = 0; fac[bad] = 0
    sd = sd.astype(np.float32); fac = fac.astype(np.float32)
    log(f'b={b}: {b * n / 8:.1f} B/item, mean cos {cos.mean():.4f}, mean sd {sd.mean():.4f}')
    res = {}
    for label, Qs in (('pseudo', P), ('test', Q)):
        rr, th, sh, zst = run(Qs, Cd, fac, sd, f'b={b} {label}')
        res[label] = dict(recall=(th.mean(0) / k).tolist(), recall_strict=(sh.mean(0) / k).tolist(), rerank=rr.mean(0).tolist(),
                          rerank_p99=np.percentile(rr, 99, axis=0).tolist(), zero_recall_queries=(th == 0).sum(0).tolist(), z=zst)
        log(f'b={b} {label}: z {zst}')
        for mi, m in enumerate(M_GRID):
            log(f'   m={m:4.2f} recall {th[:, mi].mean() / k:.4f} strict {sh[:, mi].mean() / k:.4f} rerank {rr[:, mi].mean():9.1f}')
    fits = []
    for T in TARGETS:
        ps = np.array(res['pseudo']['recall']); ts = np.array(res['test']['recall'])
        fi = np.flatnonzero(ps >= T); oi = np.flatnonzero(ts >= T)
        row = dict(target=T)
        if len(fi):
            mi = fi[0]; row.update(m_fit=float(M_GRID[mi]), pseudo_recall=float(ps[mi]), test_recall=float(ts[mi]),
                                   test_recall_strict=float(res['test']['recall_strict'][mi]), test_rerank=float(res['test']['rerank'][mi]),
                                   pseudo_rerank=float(res['pseudo']['rerank'][mi]))
        if len(oi):
            mo = oi[0]; row.update(m_oracle=float(M_GRID[mo]), oracle_test_recall=float(ts[mo]), oracle_test_rerank=float(res['test']['rerank'][mo]))
        fits.append(row); log(f'b={b} fit {row}')
    out['codes'][b] = dict(bytes=b * n / 8, mean_cos=float(cos.mean()), mean_sd=float(sd.mean()), runs=res, fits=fits)
    del Cd
json.dump(out, open(f'{outdir}/expB_{name}.json', 'w'))
log('done')
