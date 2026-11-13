# E1: additive coarse quantiser. Cell centre = C1[i] + C2[j] with one shared second codebook, so all
# k1 * k2 cell scores come from k1 + k2 dot products (q . (c1 + c2) = q . c1 + q . c2 exactly).
# Measures share of items scanned at recall targets, against flat balanced k-means with the saved
# structures, in the same harness.
#
# usage: e1_additive.py <dataset> flat K [K ...] | add k1 k2 [k1 k2 ...]
import sys, numpy as np, faiss
sys.path.insert(0, "/tmp/claude-1000/-home-joe-code-tessera/44285a24-9b9a-44a0-8e3a-604c4b51c0e9/scratchpad/clusters")
from common import *

name, mode = sys.argv[1], sys.argv[2]
args = [int(x) for x in sys.argv[3:]]
faiss.omp_set_num_threads(4)
NQ = 1000
X, Q, T, G = load(name, NQ)
N, n = X.shape
log(f'{name} N={N} n={n} queries={len(Q)}')

def report(label, C_item, a, Kc, key_parts, a2=None, fs=(0.0, 0.2, 0.35)):
    rad, cnt = cell_radius(X, C_item, a, Kc)
    empty = cnt == 0
    for f in fs:
        def keys(Qb):
            key = key_parts(Qb) + f * rad[None, :]
            key[:, empty] = -np.inf
            return key
        rec, su = curves(keys, Q, T, a, a2, Kc, cnt)
        log(f'{label} f={f}: {fmt(at_targets(rec, su, N))}   nonempty cells {int((~empty).sum())}, mean rad {np.average(rad, weights=cnt):.3f}')

if mode == 'flat':
    for K in args:
        z = np.load(f'{DATA}/{name}/probe/K{K}.npz'); C = z['Cb']; a = z['ab']
        report(f'flat balanced K={K}', C[a], a, K, lambda Qb: Qb @ C.T, fs=(0.35,))
        report(f'flat balanced+soar K={K}', C[a], a, K, lambda Qb: Qb @ C.T, a2=z['sb'], fs=(0.35,))
    sys.exit()

def train(k1, k2, L=8, rounds=4):
    C1 = kmeans(X, k1)
    _, I = nearest(X, C1); a1 = I[:, 0]
    C2 = kmeans(X - C1[a1], k2, seed=2)
    a2 = np.zeros(N, dtype=np.int64)
    for r in range(rounds + 1):
        D, I = nearest(X, C1, min(L, k1))
        G12 = C1 @ C2.T; n2 = (C2 ** 2).sum(1)
        err = 0.0
        for s in range(0, N, 20000):
            xc2 = X[s:s + 20000] @ C2.T
            cost = D[s:s + 20000, :, None] - 2 * (xc2[:, None, :] - G12[I[s:s + 20000]]) + n2[None, None, :]
            b = len(cost); flat = cost.reshape(b, -1); am = flat.argmin(1)
            a1[s:s + 20000] = I[s:s + 20000][np.arange(b), am // k2]; a2[s:s + 20000] = am % k2
            err += flat[np.arange(b), am].sum()
        log(f'  round {r}: mean squared residual {err / N:.4f}')
        if r == rounds: break
        for _ in range(2):
            R = X - C1[a1]; c = np.bincount(a2, minlength=k2)
            S = np.zeros_like(C2); np.add.at(S, a2, R); C2[c > 0] = S[c > 0] / c[c > 0, None]
            R = X - C2[a2]; c = np.bincount(a1, minlength=k1)
            S = np.zeros_like(C1); np.add.at(S, a1, R); C1[c > 0] = S[c > 0] / c[c > 0, None]
    return C1, C2, a1, a2

for k1, k2 in zip(args[0::2], args[1::2]):
    C1, C2, a1, a2 = train(k1, k2)
    cell = a1 * k2 + a2; Kc = k1 * k2
    np.savez(f'/tmp/claude-1000/-home-joe-code-tessera/44285a24-9b9a-44a0-8e3a-604c4b51c0e9/scratchpad/clusters/add_{name}_{k1}x{k2}.npz', C1=C1, C2=C2, a1=a1, a2=a2)
    def parts(Qb):
        return ((Qb @ C1.T)[:, :, None] + (Qb @ C2.T)[:, None, :]).reshape(len(Qb), -1)
    cs = np.bincount(cell, minlength=Kc); ne = cs[cs > 0]
    log(f'additive {k1}x{k2}: {Kc} cells, {len(ne)} nonempty, mean size {ne.mean():.1f}, max {ne.max()}, centre dots {k1 + k2}')
    report(f'additive {k1}x{k2}', C1[a1] + C2[a2], cell, Kc, parts)
log('done')
