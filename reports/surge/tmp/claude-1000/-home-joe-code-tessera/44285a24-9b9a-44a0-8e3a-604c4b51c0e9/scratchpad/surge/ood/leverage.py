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
GT = np.array([1, .3634, .1188, .03744, .01154, .00349, .00104, .0003043, .0000876])
def alloc(l, Qe, B):
    b = np.zeros(len(l), int); f = lambda bb: np.sqrt(Qe * l * GT[bb]); val = f(b)
    for _ in range(B):
        nb = np.minimum(b + 1, 8); gain = val - f(nb); gain[b >= 8] = -1; i = int(gain.argmax()); b[i] += 1; val = f(b)
    return b

# Leverage: share of ALL items undecided at a level when the certificate half-width is scaled by f (f = c_ex / c), H25 weights, modelled error.
Z = np.zeros((N, n), np.float32)
for g in range(G):
    ii = np.where(grp[a] == g)[0]; Z[ii] = (X[ii] - Mean[a[ii]]) @ V[g]
LV = [16, 32, 96]; sI = {}; Wd = {}
for L in LV:
    sI[L] = np.zeros(N, np.float32)
    for g in range(G):
        bits = alloc(lam[g], H25[g], L * 8); W = ((lam[g] * GT[bits]) / H25[g]) ** 0.25; Wd[L, g] = W.astype(np.float32)
        ii = np.where(grp[a] == g)[0]; sI[L][ii] = np.linalg.norm(Z[ii] * (np.sqrt(GT[bits]) / W).astype(np.float32), axis=1)
print("prep", round(time.time() - t0), "s", flush=True)
fs = [1.0, 0.89, 0.76, 0.5]
print("reported text (agent2, 0.01% answers): undecided after det 16 B 45%, det 32 B 29%, det 96 B 8.5%; split 32/48/96 B: 20% / 10.5% / 1.3%")
for nm in ("item", "flat0.5", "flat0.75"):
    Q = fam[nm]; S = X @ Q[:40].T; srt = np.sort(S, axis=0)[::-1]
    for ti, kk in ((0, 20), (1, 200)):
        t = srt[kk - 1]
        if ti == 0:
            dens = [float(((np.abs(S - t[None]) < w).sum(0) / kk).mean()) for w in (0.005, 0.01, 0.02, 0.04, 0.08)]
            print(f"{nm}: items within +-w of the top-20 threshold per true match, w = .005/.01/.02/.04/.08:", np.round(dens, 1), "(text reported 1.7 5.5 38 105 632; item reported 4.6 19.5 86.5 276 563)")
        for L in LV:
            und = np.zeros(len(fs))
            for j in range(40):
                wn = np.zeros(K, np.float32)
                for g in range(G):
                    cs = np.where(grp == g)[0]
                    if len(cs): wn[cs] = np.linalg.norm(((Q[j][None] - Mean[cs]) @ V[g]) * Wd[L, g][None], axis=1)
                hw = wn[a] * sI[L]; gap = np.abs(S[:, j] - t[j])
                und += [(gap < f * hw).mean() for f in fs]
            und /= 40
            print(f"  {nm:8s} top-{kk:<3d} {L:3d} B: undecided at width x1 / x0.89 / x0.76 / x0.5 = " + " / ".join(f"{100 * u:.1f}%" for u in und) + f"   exponent d ln U / d ln w (1 -> 0.76) = {np.log(und[2] / und[0]) / np.log(0.76):.2f}", flush=True)
print("done", round(time.time() - t0), "s")
