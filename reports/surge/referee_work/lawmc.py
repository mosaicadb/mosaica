# Lemma 8 at small dimension, where the tail near the Cauchy-Schwarz cap is testable.
# One fixed residual y, one fixed direction D, many Haar rotations. Quantiser: sign + magnitude bits, scale per item.
# Variants: (a) per-item scale from the item's own rotated vector (function of Ry only);
#           (b) scale fitted on ANOTHER rotated vector (the direction D_perp itself): codebook depends on R beyond Ry;
#           (c) stored scalars quantised (cos phi in 1 byte, rounded so tan rounds up), estimator scale derived from them.
import numpy as np, math, sys
sys.path.insert(0, __file__.rsplit('/', 1)[0])
from tailcheck import log_tail
n = int(sys.argv[1]); NB = int(sys.argv[2]); bits = int(sys.argv[3]); B = 100000
m = n - 1
rng = np.random.default_rng(11)
y = rng.standard_normal(n) * np.linspace(1.5, 0.3, n); y *= 0.7 / np.linalg.norm(y)
D = rng.standard_normal(n) + 0.8 * y / np.linalg.norm(y); D *= 0.9 / np.linalg.norm(D)
yn = np.linalg.norm(y); t = D @ y
Dp = D - (D @ y) / yn ** 2 * y; Dpn = np.linalg.norm(Dp); Dph = Dp / Dpn
Lv = 2 ** bits - 1
grid = np.linspace(0.5, 3.0, 13) * math.sqrt(n) * (Lv / 3.0) / 2 if bits > 1 else np.array([1.0])
def quant(z, s):            # odd integers in [-Lv, Lv]
    return np.clip(2 * np.floor(z * s[:, None] / 2) + 1, -Lv, Lv)
def best_scale(z):          # per-row scale maximising the cosine: a function of z only
    best = np.full(len(z), -1.0); bs = np.zeros(len(z))
    for s in grid:
        c = quant(z, np.full(len(z), s)); cs = (c * z).sum(1) / np.linalg.norm(c, axis=1)
        up = cs > best; best[up] = cs[up]; bs[up] = s
    return bs
S = np.array([0.3, 0.5, 0.7, 0.8, 0.9]) if n <= 10 else np.array([2.0, 3.0, 4.0, 4.5]) / math.sqrt(m)
exact = np.array([math.exp(log_tail(m, s)) for s in S])
cnt = {v: np.zeros((2, len(S))) for v in ('a', 'b', 'c_naive', 'c_repaired')}
cntA = np.zeros((2, len(S))); orth = 0.0; normerr = 0.0; tot = 0
for it in range(NB):
    G = rng.standard_normal((B, n, n)); Q, Rr = np.linalg.qr(G)
    Rm = Q * np.sign(np.einsum('bii->bi', Rr))[:, None, :]            # Haar on O(n)
    z = Rm @ y / yn; zD = Rm @ D; zo = Rm @ Dph
    for var in ('a', 'b'):
        s = best_scale(z) if var == 'a' else best_scale(zo)
        c = quant(z, s); cn = np.linalg.norm(c, axis=1); cos = (c * z).sum(1) / cn
        f = yn / (cn * cos); est = f * (c * zD).sum(1); A = yn * np.sqrt(1 - cos ** 2) / cos
        u1 = (est - t) / (A * Dpn)                                    # = <E, D>/(A |D_perp|)
        for j, sv in enumerate(S): cnt[var][0, j] += (u1 > sv).sum(); cnt[var][1, j] += (u1 < -sv).sum()
        if var == 'a':
            yhat = f[:, None] * np.einsum('bij,bi->bj', Rm, c); E = yhat - y
            orth = max(orth, np.abs(E @ y).max()); normerr = max(normerr, np.abs(np.linalg.norm(E, axis=1) - A).max())
            hiA = A > np.median(A)
            for j, sv in enumerate(S): cntA[0, j] += (np.abs(u1[hiA]) > sv).sum(); cntA[1, j] += (np.abs(u1[~hiA]) > sv).sum()
            # (c) quantised scalars: cos' = floor(255 cos)/255 (so tan' >= tan), |r|' = |r| rounded up to 2^-16
            cq = np.floor(cos * 255) / 255; rq = math.ceil(yn * 65536) / 65536
            fq = rq / (cn * cq); estq = fq * (c * zD).sum(1); Aq = rq * np.sqrt(1 - cq ** 2) / cq
            eta = 1 - (yn / rq) * 0 - (cq / (cq + 1 / 255)) * (1 - 2 ** -16 / rq)      # >= 1 - f/f'
            dev = np.abs(estq - t)
            for j, sv in enumerate(S):
                # naive: self-normalised event with rounded-up A', no bias term
                cnt['c_naive'][0, j] += (dev > sv * Aq * Dpn).sum()
                cnt['c_repaired'][0, j] += (dev > sv * Aq * Dpn + eta * np.abs(estq)).sum()
    tot += B
print(f'n={n} m={m} bits={bits} rotations={tot}; max |<E,y>| {orth:.2e}; max ||E|-A| {normerr:.2e}; t/(|y||D|)={t/(yn*0.9):.3f}')
print('  s            ' + ' '.join(f'{s:10.3f}' for s in S))
print('  exact 1-side ' + ' '.join(f'{e:10.3e}' for e in exact))
for var in ('a', 'b'):
    for side in (0, 1):
        f_ = cnt[var][side] / tot; zsc = (cnt[var][side] - exact * tot) / np.sqrt(exact * tot)
        print(f'  ({var}) side {"+-"[side]}  ' + ' '.join(f'{x:10.3e}' for x in f_) + '   z-scores ' + ' '.join(f'{x:+.1f}' for x in zsc))
print('  (a) two-sided, A above median ' + ' '.join(f'{x:10.3e}' for x in cntA[0] / (tot / 2)) + '\n      two-sided, A below median ' + ' '.join(f'{x:10.3e}' for x in cntA[1] / (tot / 2)))
print('  exact 2-side ' + ' '.join(f'{2 * e:10.3e}' for e in exact))
for var in ('c_naive', 'c_repaired'):
    zsc = (cnt[var][0] - 2 * exact * tot) / np.sqrt(2 * exact * tot)
    print(f'  ({var}) 2-sided ' + ' '.join(f'{x:10.3e}' for x in cnt[var][0] / tot) + '   z-scores ' + ' '.join(f'{x:+.1f}' for x in zsc))
