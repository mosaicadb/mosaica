# Data-only stand-ins for text queries on the 200k BioCLIP-2 sample: which reproduces agent2's reported text statistics?
import numpy as np, time, sys
t0 = time.time()
D = "/home/joe/code/vexact-data/bioclip2-200k/"
X = np.concatenate([np.load(D + f"g{i}.npy").astype(np.float32) for i in range(4)])
X /= np.linalg.norm(X, axis=1, keepdims=True); N, n = X.shape
rng = np.random.default_rng(1)
mu = X.mean(0); print("N", N, "n", n, "mean norm", np.linalg.norm(mu))
C = (X.T @ X).astype(np.float64) / N - np.outer(mu, mu); gl, gV = np.linalg.eigh(C); gl = gl[::-1].copy(); gV = gV[:, ::-1].astype(np.float32); gl = np.maximum(gl, 1e-12)
print("global centred energy", gl.sum(), "cum 8/32/128/256/512", [round(float(gl[:k].sum() / gl.sum()), 3) for k in (8, 32, 128, 256, 512)])
K, G = 200, 12
M = X[rng.choice(N, K, replace=False)].copy()
for it in range(8):
    a = np.concatenate([(X[i:i + 50000] @ M.T).argmax(1) for i in range(0, N, 50000)])
    for c in range(K):
        s = X[a == c]
        if len(s): M[c] = s.mean(0)
    M /= np.linalg.norm(M, axis=1, keepdims=True)  # spherical k-means for assignment
a = np.concatenate([(X[i:i + 50000] @ M.T).argmax(1) for i in range(0, N, 50000)])
Mean = np.stack([X[a == c].mean(0) if (a == c).any() else M[c] for c in range(K)])
gc = Mean[rng.choice(K, G, replace=False)].copy()
for it in range(10):
    grp = (Mean @ gc.T).argmax(1)
    for g in range(G):
        if (grp == g).any(): gc[g] = Mean[grp == g].mean(0)
grp = (Mean @ gc.T).argmax(1)
R = X - Mean[a]; mr = (R * Mean[a]).sum(1)
V = np.zeros((G, n, n), np.float32); lam = np.zeros((G, n)); Z = np.zeros_like(R)
for g in range(G):
    ii = np.where(grp[a] == g)[0]
    if len(ii) < n: V[g] = np.eye(n); lam[g] = 1e-9; continue
    Cg = (R[ii].T @ R[ii]).astype(np.float64) / len(ii); l, v = np.linalg.eigh(Cg); lam[g] = np.maximum(l[::-1], 1e-12); V[g] = v[:, ::-1]; Z[ii] = R[ii] @ V[g]
print("kmeans+bases done", round(time.time() - t0), "s; mean |r|", np.linalg.norm(R, axis=1).mean())
edges = [0, 8, 32, 64, 128, 256, 512, 768]
band = lambda v: np.array([v[..., e0:e1].sum(-1) for e0, e1 in zip(edges[:-1], edges[1:])]).T
rn = np.linalg.norm(Z, axis=1); rS8 = np.linalg.norm(Z[:, :8], axis=1); rS128 = np.linalg.norm(Z[:, 8:128], axis=1); rT = np.linalg.norm(Z[:, 128:], axis=1)
rS = np.linalg.norm(Z[:, :128], axis=1)
print("residual band shares %", np.round(100 * band(lam).sum(0) / lam.sum(), 1))
nq = 200; qi = rng.choice(N, nq, replace=False); xq = X[qi]; xc = xq - mu; cn = np.linalg.norm(xc, axis=1, keepdims=True)
unit = lambda v: v / np.linalg.norm(v, axis=1, keepdims=True)
def flat(p, v):
    return ((v @ gV) * gl[None] ** (-p / 2)).astype(np.float32) @ gV.T
fam = {"item": xq, "diff": unit(xq - X[rng.choice(N, nq)]), "iso": unit(rng.standard_normal((nq, n)).astype(np.float32))}
for b in (0.5, 1.0): fam[f"mix{b}"] = unit(xq + b * unit(rng.standard_normal((nq, n)).astype(np.float32)))
for k in (8, 32): fam[f"pcrm{k}"] = unit(mu + xc - (xc @ gV[:, :k]) @ gV[:, :k].T)
for p in (0.25, 0.5, 0.75, 1.0): fam[f"flat{p}"] = unit(mu + cn * unit(flat(p, xc)))
for p in (0.5,): fam[f"flat{p}s"] = unit(mu + 0.8 * cn * unit(flat(p, xc)))
print("TEXT (agent2, reported): |D| 0.73  bands 12.0 14.3 12.0 15.3 19.8 22.9 3.8  score sd 0.052  thr200-med 0.185  nearest-centre cos 0.75  | item: |D| 0.37 bands 26.4 31.1 15.7 13.5 9.2 4.0 0.2 sd 0.098 thr-med 0.32")
tb = np.array([12.0, 14.3, 12.0, 15.3, 19.8, 22.9, 3.8])
sz = np.bincount(a, minlength=K)
prof = {}
for nm, Q in fam.items():
    S = X @ Q.T; qm = Q @ Mean.T; top = np.argsort(-qm, axis=1)[:, :8]
    srt = np.sort(S, axis=0)[::-1]; t20 = srt[19]; t200 = srt[199]
    sd = S.std(0).mean(); gapm = (t200 - np.median(S, axis=0)).mean()
    cosn = (Q @ unit(Mean).T).max(1).mean()
    # within-cluster share of score variance
    cm = np.stack([S[a == c].mean(0) if sz[c] else np.zeros(nq) for c in range(K)]); wv = ((S - cm[a]) ** 2).mean(0); within = (wv / S.var(0)).mean(); wsd = np.sqrt(wv).mean()
    # offset profile against the 8 nearest centres, per group; plus level-0 exclusion over ALL items
    Qg = np.zeros((G, n)); cnt = np.zeros(G); dn = []; bsh = np.zeros(7)
    ex = np.zeros(4); 
    for j in range(nq):
        dl = np.zeros((K, n), np.float32)
        for g in range(G):
            cs = np.where(grp == g)[0]
            if len(cs): dl[cs] = (Q[j][None] - Mean[cs]) @ V[g]
        t8 = top[j]; e = dl[t8] ** 2; dn.append(np.sqrt(e.sum(1)).mean()); bsh += band(e).sum(0)
        np.add.at(Qg, grp[t8], e); np.add.at(cnt, grp[t8], 1)
        if j < 60:
            base = qm[j][a] + mr; dN = np.linalg.norm(dl, axis=1); d8 = np.linalg.norm(dl[:, :8], axis=1); dS = np.linalg.norm(dl[:, :128], axis=1); d128 = np.linalg.norm(dl[:, 8:128], axis=1); dT = np.linalg.norm(dl[:, 128:], axis=1)
            t = t20[j]
            ex += [(base + dN[a] * rn < t).mean(), (base + dS[a] * rS + dT[a] * rT < t).mean(), (base + d8[a] * rS8 + d128[a] * rS128 + dT[a] * rT < t).mean(), (base + dS[a] * rS < t).mean()]
    prof[nm] = Qg / np.maximum(cnt, 1)[:, None]
    bs = 100 * bsh / bsh.sum()
    print(f"{nm:8s} |D| {np.mean(dn):.3f} bands {np.round(bs, 1)} TVdist-to-text {np.abs(bs - tb).sum() / 2:5.1f}  sd {sd:.3f} thr200-med {gapm:.3f} (= {gapm / wsd:.1f} within-sd) cosnear {cosn:.3f} within-var share {within:.2f} | excluded at level 0 (top-20 thr): plain {ex[0]/60:.3f} 2band {ex[1]/60:.3f} 3band {ex[2]/60:.3f} tail-free {ex[3]/60:.3f}", flush=True)
# modelled Cauchy-Schwarz width loss of candidate profiles against each stand-in 'truth' (as dataonly/profiles.py)
def cost(T, P, e):
    w = np.sqrt(np.sqrt(e / P)); return (np.sqrt((T * w ** 2).sum(1)) * np.sqrt((e / w ** 2).sum(1)) / np.sqrt(T * e).sum(1)).mean()
Qi = prof["item"] + 1e-12; tr = Qi.sum(1, keepdims=True); iso = np.ones_like(Qi) / n * tr
blend = lambda A, B, w: ((1 - w) * np.sqrt(A) + w * np.sqrt(B)) ** 2
cands = {"Qi": Qi, "H25": blend(Qi, iso, .25), "H50": blend(Qi, iso, .5), "lam^0.5": lam ** 0.5, "lam^0.4": lam ** 0.4, "Qi^0.5": Qi ** 0.5, "0.5Qi+0.5lam^.5(amp)": blend(Qi / tr, lam ** 0.5 / (lam ** 0.5).sum(1, keepdims=True), .5), "flat0.5 standin": prof["flat0.5"] + 1e-12, "flat0.75 standin": prof["flat0.75"] + 1e-12}
# synthetic text truth: band shares as reported, within-band shape proportional to lam^0.5
Tt = np.zeros_like(lam)
for g in range(G):
    for (e0, e1), s in zip(zip(edges[:-1], edges[1:]), tb / 100):
        sh = lam[g, e0:e1] ** 0.5; Tt[g, e0:e1] = s * sh / sh.sum()
print("modelled CS width / optimum; columns: truth = item | text(band-matched synthetic) | flat0.5 | flat0.75 | mix1.0")
for nm, P in cands.items():
    print(f"  {nm:24s} {cost(Qi, P, lam):.3f}  {cost(Tt, P, lam):.3f}  {cost(prof['flat0.5'] + 1e-12, P, lam):.3f}  {cost(prof['flat0.75'] + 1e-12, P, lam):.3f}  {cost(prof['mix1.0'] + 1e-12, P, lam):.3f}")
print("band shares of candidate profiles %:")
for nm, P in cands.items(): print(f"  {nm:24s}", np.round(100 * (band(P).sum(0) / P.sum()), 1))
print("done", round(time.time() - t0), "s")
