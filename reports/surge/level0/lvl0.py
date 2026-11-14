# Counts for the global-rotation level: items whose upper bound at constant m reaches the true k-th score
# (oracle threshold) and the seed threshold; 1/2/4-bit codes, nested; plain and centre-corrected estimate.
import sys, os, time, json, math, numpy as np, faiss, h5py
path, K, NQ = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
faiss.omp_set_num_threads(4)
k = 10; NSEED = 30
MG = np.array([2.5, 3, 4, 5, 5.5, 6, 6.5, 7, 7.5, 8, 8.5, 9.1, 9.35])
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
names = ['1', '2', '4', '1&2', '1&4', '2&4', '1&2&4']
cnt = {v: np.zeros((len(MG), len(names))) for v in ('plain', 'centre', 'plain_seed', 'centre_seed')}
zmax = {v: {b: 0.0 for b in BITS} for v in ('plain', 'centre')}; zsd = {v: {b: [] for b in BITS} for v in ('plain', 'centre')}
zq = {b: [] for b in BITS}; wtop = []; B = 20
for i in range(0, len(Q), B):
    q = Q[i:i + B]; S = q @ X.T; qc = q @ C.T
    sk = -np.partition(-S, k - 1, axis=1)[:, k - 1]
    al = qc / cn2[None, :]; w = np.sqrt(np.maximum(1 - qc * qc / cn2[None, :], 0)).astype(np.float32)
    top = np.argpartition(-S, k - 1, axis=1)[:, :k]
    wtop.append(np.take_along_axis(w[:, a], top, 1).ravel())
    E = {}; Ec = {}; wa = w[:, a]
    for b in BITS:
        E[b] = (PQ[i:i + B] @ CD[b].T) * FAC[b][None, :] + qc[:, a]
        Ec[b] = E[b] + al[:, a] * EC[b][None, :]
        ok = SD[b] > 0
        z = (S - E[b])[:, ok] / SD[b][None, ok]; zc = (S - Ec[b])[:, ok] / (wa[:, ok] * SD[b][None, ok])
        zmax['plain'][b] = max(zmax['plain'][b], float(np.abs(z).max())); zmax['centre'][b] = max(zmax['centre'][b], float(np.abs(zc).max()))
        zsd['plain'][b].append(float(z.std())); zsd['centre'][b].append(float(zc.std()))
        zq[b].append(np.take_along_axis((S - Ec[b]) / np.maximum(wa * SD[b][None, :], 1e-12), top, 1).ravel())
    for var, EE, ww in (('plain', E, None), ('centre', Ec, wa)):
        # seed threshold from the 30 best 2-bit estimates
        sd30 = np.argpartition(-EE[2], NSEED - 1, axis=1)[:, :NSEED]
        ss = np.take_along_axis(S, sd30, 1); tau0 = -np.partition(-ss, k - 1, axis=1)[:, k - 1]
        for thr, key in ((sk, var), (tau0, var + '_seed')):
            for mi, m in enumerate(MG):
                M = {}
                for b in BITS:
                    h = SD[b][None, :] * m if ww is None else ww * SD[b][None, :] * m
                    M[b] = EE[b] + h >= thr[:, None]
                c = [M[1].sum(), M[2].sum(), M[4].sum(), (M[1] & M[2]).sum(), (M[1] & M[4]).sum(), (M[2] & M[4]).sum(), (M[1] & M[2] & M[4]).sum()]
                cnt[key][mi] += np.array(c)
    log(f'{i + len(q)} queries')
nq = len(Q)
print('mean width factor w over true top-k items:', float(np.concatenate(wtop).mean()))
for v in ('plain', 'centre'):
    print(v, 'max|z|', zmax[v], 'z sd', {b: float(np.mean(zsd[v][b])) for b in BITS})
for b in BITS:
    zz = np.concatenate(zq[b]); print(f'top-k items, centre variant b={b}: max z (truth above estimate) {zz.max():.2f}, sd {zz.std():.3f}')
for key in cnt:
    print(f'\n{key}: mean items per query passing (N={N})'); print('m     ' + ' '.join(f'{x:>9}' for x in names))
    for mi, m in enumerate(MG): print(f'{m:5.2f} ' + ' '.join(f'{x:9.0f}' for x in cnt[key][mi] / nq))
json.dump(dict(N=N, n=n, K=K, nq=nq, m=MG.tolist(), names=names, cnt={k_: (v / nq).tolist() for k_, v in cnt.items()}), open(sys.argv[4], 'w'))
