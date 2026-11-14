# Builds the cascade's per-item arrays: residuals to the saved plain k-means centres (probe/K*.npz, C0/a0),
# one sign-fixed Haar rotation, RaBitQ-style odd-grid codes (code() from probe/expB.py and level0/lvl0*.py, but with the scale chosen per item from
# its own rotated vector, so the code is a function of that vector alone),
# and per item |r|, rho = c.r, and per code f, sd = |r| tan(phi)/sqrt(n-1), e_c = rho - f<obar,Pc>.
# Codes: 1 bit = sign; 2 bit = sign + 1 magnitude bit at the item's 2-bit optimal scale s2; 4 bit nested = tied scale
# s4 = 4 s2 (its top magnitude bit is the 2-bit magnitude bit); 4 bit independent = the item's own optimal 4-bit scale s4i;
# c2t = 2 bit at s4i/4, which c4i nests (the other way to tie the scales).
# Items are stored sorted by (cluster, id) so a cluster is a contiguous range.
# usage: build.py <name> <K> <workdir>
import sys, os, time, numpy as np, h5py
name, K, wd = sys.argv[1], int(sys.argv[2]), sys.argv[3]
os.makedirs(wd, exist_ok=True)
t0 = time.time()
def log(*a): print(f'[{time.time()-t0:5.0f}s]', *a, flush=True)
def unit(A): A = A.astype(np.float32); return A / np.maximum(np.linalg.norm(A, axis=1, keepdims=True), 1e-12)
f = h5py.File(f'/home/joe/code/vexact-data/{name}/{name}.hdf5', 'r')
X = unit(np.asarray(f['train'])); Q = np.asarray(f['test'], dtype=np.float32)
N, n = X.shape
z = np.load(f'/home/joe/code/vexact-data/{name}/probe/K{K}.npz')
C = np.asarray(z['C0'], np.float32); a = np.asarray(z['a0'], np.int64)
assert C.shape == (K, n) and a.shape == (N,) and a.min() >= 0 and a.max() < K and np.isfinite(C).all() and np.isfinite(X).all()
assert Q.ndim == 2 and Q.shape[1] == n and np.isfinite(Q).all()
qn = np.linalg.norm(Q, axis=1); log(f'{name}: N={N} n={n} K={K} queries={len(Q)} zero queries={(qn == 0).sum()} zero items={(np.linalg.norm(X, axis=1) == 0).sum()}')
order = np.lexsort((np.arange(N), a)); X = X[order]; a = a[order]
counts = np.bincount(a, minlength=K)
R = X - C[a]; rn = np.linalg.norm(R, axis=1)
rng = np.random.default_rng(7)
G = rng.standard_normal((n, n)); Qr, Rr = np.linalg.qr(G); Rot = (Qr * np.sign(np.diag(Rr))).astype(np.float32)
O = (R / np.maximum(rn, 1e-12)[:, None]) @ Rot.T
PC = C @ Rot.T; cr = (C[a] * R).sum(1); cn2 = (C * C).sum(1)
del R
log(f'mean|r|={rn.mean():.3f} mean|c|={np.sqrt(cn2).mean():.3f} |r|<1e-6: {(rn < 1e-6).sum()}')

GRID = np.linspace(0.5, 8.0, 61)
def best_scale(O, b):
    """Per item, the grid scale maximising the cosine between the item's own rotated vector and its code."""
    Lv = 2 ** b - 1; out = np.empty(len(O), np.float32)
    for i in range(0, len(O), 50000):
        o = O[i:i + 50000]; best = np.full(len(o), -2.0); bs = np.zeros(len(o))
        for s in GRID * np.sqrt(n) * (Lv / 3.0) / 2:
            Cd = np.clip(2 * np.floor(o * s / 2) + 1, -Lv, Lv)
            cs = (Cd * o).sum(1) / np.linalg.norm(Cd, axis=1)
            better = cs > best; best[better] = cs[better]; bs[better] = s
        out[i:i + 50000] = bs
    return out
def grid(O, b, s):
    Lv = 2 ** b - 1
    return np.clip(2 * np.floor(O * s[:, None] / 2) + 1, -Lv, Lv).astype(np.float32)
s2 = best_scale(O, 2); s4i = best_scale(O, 4)
codes = {'c1': np.sign(O).astype(np.float32), 'c2': grid(O, 2, s2), 'c4n': grid(O, 4, 4 * s2), 'c4i': grid(O, 4, s4i), 'c2t': grid(O, 2, s4i / 4)}
codes['c1'][codes['c1'] == 0] = 1
assert (np.sign(codes['c2']) == codes['c1']).all() and (np.sign(codes['c4n']) == codes['c1']).all()
assert ((np.abs(codes['c4n']) >= 9) == (np.abs(codes['c2']) == 3)).all() and ((np.abs(codes['c4i']) >= 9) == (np.abs(codes['c2t']) == 3)).all(), '4-bit code does not nest the 2-bit code'
for nm_, s_, unit_ in (('s2', s2, np.sqrt(n) / 2), ('s4i', s4i, np.sqrt(n) * 2.5)):
    g = s_ / unit_; log(f'{nm_}: grid units median {np.median(g):.3f}, at lower edge {(g <= GRID[0] + 1e-6).sum()}, at upper edge {(g >= GRID[-1] - 1e-6).sum()}')
np.save(f'{wd}/X.npy', X); np.save(f'{wd}/a.npy', a); np.save(f'{wd}/counts.npy', counts); np.save(f'{wd}/C.npy', C)
np.save(f'{wd}/Rot.npy', Rot); np.save(f'{wd}/rn.npy', rn.astype(np.float32)); np.save(f'{wd}/cr.npy', cr.astype(np.float32))
np.save(f'{wd}/order.npy', order)
for key, Cd in codes.items():
    cnn = np.maximum(np.linalg.norm(Cd, axis=1), 1e-12); cos = (Cd * O).sum(1) / cnn
    fac = rn / (cnn * cos); sd = rn * np.sqrt(np.maximum(1 - cos * cos, 0)) / cos / np.sqrt(n - 1)
    bad = ~np.isfinite(sd) | ~np.isfinite(fac) | (rn < 1e-6); sd[bad] = 0; fac[bad] = 0
    ec = cr - fac * (Cd * PC[a]).sum(1)
    log(f'{key}: mean cos {cos.mean():.4f} mean tan {np.mean((np.sqrt(1-cos*cos)/cos)[~bad]):.4f} mean sd {sd.mean():.5f} bad {bad.sum()}')
    np.save(f'{wd}/{key}.npy', Cd); np.save(f'{wd}/{key}_fac.npy', fac.astype(np.float32)); np.save(f'{wd}/{key}_sd.npy', sd.astype(np.float32)); np.save(f'{wd}/{key}_ec.npy', ec.astype(np.float32))
log('done')
