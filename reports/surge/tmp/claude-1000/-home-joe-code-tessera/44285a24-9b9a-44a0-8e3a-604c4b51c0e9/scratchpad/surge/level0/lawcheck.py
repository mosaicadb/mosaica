# Counts for the global-rotation level: items whose upper bound at constant m reaches the true k-th score
# (oracle threshold) and the seed threshold; 1/2/4-bit codes, nested; plain and centre-corrected estimate.
import sys, os, time, json, math, numpy as np, faiss, h5py
path, K, NQ = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
faiss.omp_set_num_threads(4)
k = 10; NSEED = 30
MG = np.array([3, 4, 5, 5.5, 6, 6.5, 7, 8, 9.35])
t0 = time.time()
def log(*a): print(f'[{time.time()-t0:5.0f}s]', *a, flush=True)
def unit(A): A = A.astype(np.float32); return A / np.maximum(np.linalg.norm(A, axis=1, keepdims=True), 1e-12)
f = h5py.File(path, 'r'); X = unit(np.asarray(f['train'])); Qa = np.asarray(f['test'])
Qa = Qa[np.linalg.norm(Qa, axis=1) > 0][:NQ]; Q = unit(Qa)
N, n = X.shape
rng = np.random.default_rng(7)
km = faiss.Kmeans(n, K, niter=10, seed=1, max_points_per_centroid=64); km.train(X)
C = km.centroids.copy(); _, a = km.index.search(X, 1); a = a[:, 0]
R = X - C[a]; rn = np.linalg.norm(R, axis=1)
G = rng.standard_normal((n, n)); Qr, Rr = np.linalg.qr(G); Rot = (Qr * np.sign(np.diag(Rr))).astype(np.float32)
O = (R / np.maximum(rn, 1e-12)[:, None]) @ Rot.T
PC = C @ Rot.T; cr = (C[a] * R).sum(1); cn2 = (C * C).sum(1)
del R
log(f'N={N} n={n} K={K} mean|r|={rn.mean():.3f} mean|c|={np.sqrt(cn2).mean():.3f}')
def code(O, b):
    if b == 1: return np.sign(O).astype(np.float32)
    Lv = 2 ** b - 1; best = None
    for s in np.linspace(0.6, 3.0, 13) * np.sqrt(n) * (Lv / 3.0) / 2:
        Cd = np.clip(2 * np.floor(O[:20000] * s / 2) + 1, -Lv, Lv)
        cs = ((Cd * O[:20000]).sum(1) / np.linalg.norm(Cd, axis=1)).mean()
        if best is None or cs > best[0]: best = (cs, s)
    return np.clip(2 * np.floor(O * best[1] / 2) + 1, -Lv, Lv).astype(np.float32)
BITS = (1, 2, 4); CD = {}; FAC = {}; SD = {}; EC = {}
for b in BITS:
    Cd = code(O, b); cnn = np.maximum(np.linalg.norm(Cd, axis=1), 1e-12); cos = (Cd * O).sum(1) / cnn
    fac = rn / (cnn * cos); sd = rn * np.sqrt(np.maximum(1 - cos * cos, 0)) / cos / np.sqrt(n - 1)
    bad = ~np.isfinite(sd) | ~np.isfinite(fac) | (rn < 1e-6); sd[bad] = 0; fac[bad] = 0
    ec = cr - fac * (Cd * PC[a]).sum(1)          # code error along the centre: c.r - estimate of c.r
    CD[b] = Cd; FAC[b] = fac.astype(np.float32); SD[b] = sd.astype(np.float32); EC[b] = ec.astype(np.float32)
    log(f'b={b}: mean cos {cos.mean():.4f} mean tan {np.mean(np.sqrt(1-cos*cos)/cos):.4f} mean sd {sd.mean():.5f} bad {bad.sum()}')
del O
PQ = Q @ Rot.T

import importlib.util
spec = importlib.util.spec_from_file_location('consts_mod', os.path.join(os.path.dirname(os.path.abspath(__file__)), 'consts_tail.py')); cm = importlib.util.module_from_spec(spec); spec.loader.exec_module(cm)
rsafe = np.maximum(rn, 1e-12).astype(np.float32); ok = (rn > 1e-6)
zs = {b: [] for b in (1, 2)}
for i in range(0, len(Q), 20):
    q = Q[i:i + 20]; S = q @ X.T; qc = q @ C.T
    al = (qc / cn2[None, :])[:, a]; wa = np.sqrt(np.maximum(1 - qc * qc / cn2[None, :], 0))[:, a]
    t = S - qc[:, a] - al * cr[None, :]                               # true (q - alpha c).r
    for b in (1, 2):
        est = (PQ[i:i + 20] @ CD[b].T) * FAC[b][None, :] + al * (EC[b] - cr)[None, :]
        dperp = np.sqrt(np.maximum(wa * wa - (t / rsafe[None, :]) ** 2, 1e-12))
        z = ((t - est) / (SD[b][None, :] * dperp))[:, ok & (SD[b] > 0)]
        zs[b].append(z.ravel().astype(np.float32))
for b in (1, 2):
    z = np.concatenate(zs[b]); print(f'b={b}: pairs {len(z):.3g}; sd of z {z.std():.4f}; max |z| {np.abs(z).max():.2f}')
    for thr in (3, 4, 5):
        print(f'   |z|>{thr}: measured {float((np.abs(z) > thr).mean()):.3e}   exact sphere law m={n-1}: {2 * cm.tail(n - 1, thr / math.sqrt(n - 1)):.3e}   Gaussian: {math.erfc(thr / math.sqrt(2)):.3e}')
