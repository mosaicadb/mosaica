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
names = ['1', '2', '4', '1&2', '1&4', '2&4']
VARS = ('plain', 'plain_refined', 'centre', 'centre_refined', 'centre_refined_seed')
cnt = {v: np.zeros((len(MG), len(names))) for v in VARS}
wtop = []; B = 20; viol = {b: 0 for b in BITS}; ntop = 0
rsafe = np.maximum(rn, 1e-12).astype(np.float32)
for i in range(0, len(Q), B):
    q = Q[i:i + B]; S = q @ X.T; qc = q @ C.T
    sk = -np.partition(-S, k - 1, axis=1)[:, k - 1]
    al = (qc / cn2[None, :])[:, a]; wa = np.sqrt(np.maximum(1 - qc * qc / cn2[None, :], 0)).astype(np.float32)[:, a]
    qca = qc[:, a]
    top = np.argpartition(-S, k - 1, axis=1)[:, :k]; ntop += top.size
    wtop.append(np.take_along_axis(wa, top, 1).ravel())
    T = {}; Tc = {}
    for b in BITS:
        T[b] = (PQ[i:i + B] @ CD[b].T) * FAC[b][None, :]            # estimate of q.r
        Tc[b] = T[b] + al * (EC[b] - cr)[None, :]                      # estimate of (q - alpha c).r
    sd30 = np.argpartition(-(T[2] + qca), NSEED - 1, axis=1)[:, :NSEED]
    tau0 = -np.partition(-np.take_along_axis(S, sd30, 1), k - 1, axis=1)[:, k - 1]
    for var in VARS:
        thr = tau0 if var.endswith('_seed') else sk
        cen = var.startswith('centre'); ref = 'refined' in var
        base = qca + al * cr[None, :] if cen else qca               # known part of the score
        rw = rsafe[None, :] * wa if cen else np.broadcast_to(rsafe[None, :], S.shape)
        for mi, m in enumerate(MG):
            M = {}
            for b in BITS:
                t = Tc[b] if cen else T[b]
                if ref:
                    kap = (m * SD[b] / rsafe)[None, :]
                    hi = (t + kap * np.sqrt(np.maximum((1 + kap * kap) * rw * rw - t * t, 0))) / (1 + kap * kap)
                else:
                    hi = t + m * SD[b][None, :] * (wa if cen else 1.0)
                M[b] = base + hi >= thr[:, None]
                if var == 'centre_refined' and abs(m - 6.0) < 1e-9: viol[b] += int((~np.take_along_axis(M[b], top, 1)).sum())
            cnt[var][mi] += np.array([M[1].sum(), M[2].sum(), M[4].sum(), (M[1] & M[2]).sum(), (M[1] & M[4]).sum(), (M[2] & M[4]).sum()])
    log(f'{i + len(q)} queries')
nq = len(Q)
print('mean width factor w over true top-k items:', float(np.concatenate(wtop).mean()))
print('true top-k items pruned at m=6, centre_refined:', viol, 'of', ntop)
for key in cnt:
    print(f'\n{key}: mean items per query passing (N={N})'); print('m     ' + ' '.join(f'{x:>9}' for x in names))
    for mi, m in enumerate(MG): print(f'{m:5.2f} ' + ' '.join(f'{x:9.0f}' for x in cnt[key][mi] / nq))
json.dump(dict(N=N, n=n, K=K, nq=nq, m=MG.tolist(), names=names, cnt={k_: (v / nq).tolist() for k_, v in cnt.items()}), open(sys.argv[4], 'w'))
