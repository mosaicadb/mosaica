# Two stored norms (plain and weighted) give a free one-parameter family of certificate weights per (query, item):
#   |D.e|^2 <= min_k (|e|^2 + k |W^-1 e|^2) * sum_i D_i^2 / (1 + k W_i^-2).   How much does it buy for text-like stand-ins?
import numpy as np, time
t0 = time.time()
Dd = "/home/joe/code/vexact-data/bioclip2-200k/"
X = np.concatenate([np.load(Dd + f"g{i}.npy").astype(np.float32) for i in range(4)]); X /= np.linalg.norm(X, axis=1, keepdims=True); N, n = X.shape
rng = np.random.default_rng(1); mu = X.mean(0)
C = (X.T @ X).astype(np.float64) / N - np.outer(mu, mu); gl, gV = np.linalg.eigh(C); gl = np.maximum(gl[::-1], 1e-12); gV = gV[:, ::-1].astype(np.float32)
K, G = 200, 12
M = X[rng.choice(N, K, replace=False)].copy()
for it in range(8):
    a = np.concatenate([(X[i:i + 50000] @ M.T).argmax(1) for i in range(0, N, 50000)])
    for c in range(K):
        s = X[a == c]
        if len(s): M[c] = s.mean(0)
    M /= np.linalg.norm(M, axis=1, keepdims=True)
a = np.concatenate([(X[i:i + 50000] @ M.T).argmax(1) for i in range(0, N, 50000)])
Mean = np.stack([X[a == c].mean(0) if (a == c).any() else M[c] for c in range(K)])
gc = Mean[rng.choice(K, G, replace=False)].copy()
for it in range(10):
    grp = (Mean @ gc.T).argmax(1)
    for g in range(G):
        if (grp == g).any(): gc[g] = Mean[grp == g].mean(0)
grp = (Mean @ gc.T).argmax(1)
V = np.zeros((G, n, n), np.float32); lam = np.zeros((G, n))
members = [np.where(a == c)[0] for c in range(K)]
for g in range(G):
    ii = np.where(grp[a] == g)[0]; R = X[ii] - Mean[a[ii]]
    Cg = (R.T @ R).astype(np.float64) / len(ii); l, v = np.linalg.eigh(Cg); lam[g] = np.maximum(l[::-1], 1e-12); V[g] = v[:, ::-1]
print("setup", round(time.time() - t0), "s", flush=True)
unit = lambda v: v / np.linalg.norm(v, axis=1, keepdims=True)
nq = 120; qi = rng.choice(N, nq, replace=False); xq = X[qi]; xc = xq - mu; cn = np.linalg.norm(xc, axis=1, keepdims=True)
flat = lambda p, v: ((v @ gV) * gl[None] ** (-p / 2)).astype(np.float32) @ gV.T
fam = {"item": xq, "flat0.5": unit(mu + cn * unit(flat(.5, xc))), "flat0.75": unit(mu + cn * unit(flat(.75, xc))), "mix1.0": unit(xq + unit(rng.standard_normal((nq, n)).astype(np.float32)))}
# item profile from the item family (8 nearest centres)
Qi = np.zeros((G, n)); cnt = np.zeros(G)
for j in range(nq):
    for c in np.argsort(-(xq[j] @ Mean.T))[:8]:
        Qi[grp[c]] += ((xq[j] - Mean[c]) @ V[grp[c]]) ** 2; cnt[grp[c]] += 1
Qi = Qi / np.maximum(cnt, 1)[:, None] + 1e-12; iso = np.ones_like(Qi) * Qi.sum(1, keepdims=True) / n
H25 = (0.75 * np.sqrt(Qi) + 0.25 * np.sqrt(iso)) ** 2
GT = np.array([1, .3634, .1188, .03744, .01154, .00349, .00104, .0003043, .0000876])
def alloc(l, Qe, B):
    b = np.zeros(len(l), int); f = lambda bb: np.sqrt(Qe * l * GT[bb]); val = f(b)
    for _ in range(B):
        nb = np.minimum(b + 1, 8); gain = val - f(nb); gain[b >= 8] = -1; i = int(gain.argmax()); b[i] += 1; val = f(b)
    return b
LV = [0, 16, 32, 96]; kap = np.concatenate([[0.0], 4.0 ** np.arange(-5, 6)]); kap4 = np.array([0.0, 0.25, 1.0, 4.0])
for pn, P in (("Qi (D0)", Qi), ("H25", H25)):
    sc = {}; 
    for L in LV:
        for g in range(G):
            bits = alloc(lam[g], P[g], L * 8); ev = lam[g] * GT[bits]; W = (ev / P[g]) ** 0.25; W = W / np.sqrt(np.mean(W ** -2.0)) ** -1  # normalise so mean W^-2 = 1
            sc[L, g] = (np.sqrt(GT[bits]).astype(np.float32), W.astype(np.float32))
    print(f"\nprofile {pn}: mean half-width of the bound on D.e over (query, item in its 8 nearest clusters); error modelled as residual coordinate x sqrt(g(bits))")
    print("  family    bytes  plain   weighted  min(pl,w)  2-norm family (12 k / 4 k)   per-pair l1   actual   | gain of family over min, over weighted")
    for nm, Q in fam.items():
        for L in LV:
            acc = np.zeros(7); m = 0
            for j in range(0, nq, 2):
                for c in np.argsort(-(Q[j] @ Mean.T))[:8]:
                    g = grp[c]; ii = members[c]
                    if len(ii) < 30: continue
                    ii = rng.choice(ii, 30, replace=False); sg, W = sc[L, g]
                    d = (Q[j] - Mean[c]) @ V[g]; e = ((X[ii] - Mean[c]) @ V[g]) * sg
                    e2 = (e * e).sum(1); s2 = ((e / W) ** 2).sum(1); pl = np.sqrt((d * d).sum() * e2); w = np.sqrt(((d * W) ** 2).sum() * s2)
                    F = ((d * d)[None] / (1 + kap[:, None] * (W ** -2.0)[None])).sum(1)
                    fam_b = np.sqrt(np.minimum(((e2[:, None] + kap[None] * s2[:, None]) * F[None]).min(1), w * w))
                    F4 = ((d * d)[None] / (1 + kap4[:, None] * (W ** -2.0)[None])).sum(1)
                    fam4 = np.sqrt(np.minimum(((e2[:, None] + kap4[None] * s2[:, None]) * F4[None]).min(1), w * w))
                    acc += [pl.sum(), w.sum(), np.minimum(pl, w).sum(), fam_b.sum(), fam4.sum(), (np.abs(d)[None] * np.abs(e)).sum(), np.abs(e @ d).sum()]; m += len(ii)
            acc /= m
            print(f"  {nm:9s} {L:4d}  {acc[0]:.4f}  {acc[1]:.4f}   {acc[2]:.4f}    {acc[3]:.4f} / {acc[4]:.4f}          {acc[5]:.4f}      {acc[6]:.5f} | {100 * (1 - acc[3] / acc[2]):.1f}%  {100 * (1 - acc[3] / acc[1]):.1f}%", flush=True)
print("done", round(time.time() - t0), "s")
