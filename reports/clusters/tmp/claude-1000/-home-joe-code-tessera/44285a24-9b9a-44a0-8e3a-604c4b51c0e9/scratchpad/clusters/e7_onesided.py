# E7: what an exact k-NN query must read when the first levels are a global rotated b-bit code of the
# residual, and what the confidence constant costs. Exclusion uses only the upper bound
# est + c sd < tau with tau taken from exact scores, so a wrong answer needs a true top-k item's upper
# bound to fail: a union over k items, not N. c = 7.02 is that one-sided union for k = 10, 5 levels,
# eps = 1e-9; 9.35 is the two-sided union over 1e9 items x 5 levels.
# Counts per query: items whose 1-bit bound survives, whose 2-bit bound survives, whose 4-bit survives.
#
# usage: e7_onesided.py <dataset> K
import sys, numpy as np
sys.path.insert(0, "/tmp/claude-1000/-home-joe-code-tessera/44285a24-9b9a-44a0-8e3a-604c4b51c0e9/scratchpad/clusters")
from common import *

name, K = sys.argv[1], int(sys.argv[2])
NQ = 200
X, Q, T, G = load(name, NQ)
N, n = X.shape
z = np.load(f'{DATA}/{name}/probe/K{K}.npz'); C = z['Cb']; a = z['ab']
rng = np.random.default_rng(1)
S = Q @ X.T
tk = -np.partition(-S, k - 1, axis=1)[:, k - 1]
gt = np.argpartition(-S, k - 1, axis=1)[:, :k]
R = X - C[a]; rn = np.linalg.norm(R, axis=1)
P = np.linalg.qr(rng.standard_normal((n, n)))[0]
O = np.nan_to_num(R / np.maximum(rn, 1e-12)[:, None]) @ P.T
PQ = Q @ P.T; qc = (Q @ C.T)[:, a]
log(f'{name} N={N} n={n} K={K} queries={NQ}; mean |r| {rn.mean():.3f}; mean k-th score {tk.mean():.3f}; score sd {S.std():.3f}')

def code(O, b):
    if b == 1: return np.sign(O).astype(np.float32)
    L = 2 ** b - 1; best = None
    for s in np.linspace(0.6, 3.0, 9) * np.sqrt(n) * (L / 3.0) / 2:
        Cd = np.clip(2 * np.floor(O[:20000] * s / 2) + 1, -L, L)
        cs = ((Cd * O[:20000]).sum(1) / np.linalg.norm(Cd, axis=1)).mean()
        if best is None or cs > best[0]: best = (cs, s)
    return np.clip(2 * np.floor(O * best[1] / 2) + 1, -L, L).astype(np.float32)

MS = (3.0, 5.0, 7.02, 8.6, 9.35)
UB = {}
for b in (1, 2, 4):
    Cd = code(O, b); cn = np.maximum(np.linalg.norm(Cd, axis=1), 1e-12)
    cos = (Cd * O).sum(1) / cn
    fac = rn / (cn * cos); sd = rn * np.sqrt(np.maximum(1 - cos * cos, 0)) / cos / np.sqrt(n - 1)
    bad = ~np.isfinite(sd) | ~np.isfinite(fac); sd[bad] = 0; fac[bad] = 0
    E = (PQ @ Cd.T) * fac[None, :] + qc
    zz = (E - S) / np.maximum(sd, 1e-12)[None, :]
    ztop = np.take_along_axis(zz, gt, 1)
    log(f'b={b}: {b * n / 8:.1f} B/item, mean sd {sd.mean():.4f}; error/sd over all pairs: sd {zz.std():.3f}, most negative {zz.min():.2f}; over the true top-{k} only: most negative {ztop.min():.2f}')
    UB[b] = (E, sd)
    del zz
for m in MS:
    alive = np.ones((NQ, N), bool); row = []
    for b in (1, 2, 4):
        E, sd = UB[b]
        alive &= (E + m * sd[None, :]) >= tk[:, None]
        row.append(alive.sum(1).mean())
    miss = sum(int((~alive[i, gt[i]]).sum()) for i in range(NQ))
    # modelled time: 1-bit pass over all, 2-bit and 4-bit reads for survivors, full vector for the rest
    b1 = n / 8
    ns = N * (0.3 + 0.066 * b1) + row[0] * (1.0 + 0.066 * b1) + row[1] * (1.0 + 0.066 * 2 * b1) + row[2] * (40 if n <= 128 else 80)
    by = N * b1 + row[0] * b1 + row[1] * 2 * b1 + row[2] * 2 * n
    log(f'c={m:5.2f}: survive 1 bit {row[0]:9.0f} ({100 * row[0] / N:5.1f}%), 2 bits {row[1]:8.0f} ({100 * row[1] / N:5.2f}%), 4 bits {row[2]:7.0f} ({100 * row[2] / N:5.3f}%); true top-{k} items lost {miss}; '
        f'bytes read {by / 1e6:.1f} MB ({100 * by / (N * 2 * n):.1f}% of f16 data); modelled {ns / 1e6:.2f} ms')
log('done')
