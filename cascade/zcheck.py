# Recomputes in float64, from the stored vectors, codes and scalars, every (query, item) pair whose float32 z
# (as run.py computes it, first 1000 queries) has |z| > 4.5, and counts |z| > 5 again.
# usage: zcheck.py <name> <workdir>
import sys, numpy as np, h5py
name, wd = sys.argv[1], sys.argv[2]
L = lambda key: np.load(f'{wd}/{key}.npy', mmap_mode='r')
X = np.asarray(L('X')); a = np.asarray(L('a')); counts = np.asarray(L('counts')); C = np.asarray(L('C')); Rot = np.asarray(L('Rot'))
rn = np.asarray(L('rn')); cr = np.asarray(L('cr')); cn2 = (C * C).sum(1); rsafe = np.maximum(rn, 1e-12)
Qa = np.asarray(h5py.File(f'/home/joe/code/vexact-data/{name}/{name}.hdf5', 'r')['test'])[:1000].astype(np.float32)
Q = Qa / np.maximum(np.linalg.norm(Qa, axis=1, keepdims=True), 1e-12)
X64 = X.astype(np.float64); C64 = C.astype(np.float64); R64 = X64 - C64[a]; rn64 = np.linalg.norm(R64, axis=1); cr64 = (C64[a] * R64).sum(1)
for c in ('c1', 'c2', 'c2t', 'c4n', 'c4i'):
    CD = np.asarray(L(c)); fac = np.asarray(L(c + '_fac')); sd = np.asarray(L(c + '_sd')); ecr = np.asarray(L(c + '_ec')) - cr
    ok = (rn > 1e-6) & (sd > 0)
    n32 = n64 = 0; worst = []
    for i in range(0, len(Q), 8):
        q = Q[i:i + 8]; S = q @ X.T; qc = q @ C.T
        ala = np.repeat(qc / cn2[None, :], counts, axis=1); wa = np.repeat(np.sqrt(np.maximum(1 - qc * qc / cn2[None, :], 0)), counts, axis=1)
        tt = S - np.repeat(qc, counts, axis=1) - ala * cr[None, :]
        est = ((q @ Rot.T) @ CD.T) * fac[None, :] + ala * ecr[None, :]
        dperp = np.sqrt(np.maximum(wa * wa - (tt / rsafe[None, :]) ** 2, 1e-12))
        z = (tt - est) / (sd[None, :] * dperp)
        z[:, ~ok] = 0
        qi, xi = np.nonzero(np.abs(z) > 4.5)
        n32 += int((np.abs(z) > 5).sum())
        if len(qi):
            q64 = q[qi].astype(np.float64); c64 = C64[a[xi]]
            al = (q64 * c64).sum(1) / (c64 * c64).sum(1)
            D = q64 - al[:, None] * c64; w2 = (D * D).sum(1)
            t = (D * R64[xi]).sum(1)
            e = fac[xi].astype(np.float64) * ((q64 @ Rot.T.astype(np.float64)) * CD[xi]).sum(1) + al * ecr[xi].astype(np.float64)
            dp = np.sqrt(np.maximum(w2 - t * t / (rn64[xi] ** 2), 0))
            z64 = (t - e) / (sd[xi] * dp)
            n64 += int((np.abs(z64) > 5).sum())
            for j in np.flatnonzero(np.abs(z64) > 5): worst.append((float(z[qi[j], xi[j]]), float(z64[j]), float(dp[j]), float(t[j] / (rn64[xi[j]] * np.sqrt(w2[j])))))
    worst.sort(key=lambda v: -abs(v[1]))
    print(f'{c}: |z|>5 float32 {n32}, float64 {n64}; largest float64 (z32, z64, |D_perp|, t/R): {[tuple(round(x, 4) for x in v) for v in worst[:4]]}', flush=True)
