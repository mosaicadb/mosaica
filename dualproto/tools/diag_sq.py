# Which offset law to fit Sigma_Q on: zero-bit diagonal Cauchy-Schwarz slack E sum_c sqrt(lam_c q_c) per item, averaged
# over groups by size, for several fits, evaluated on several offset populations (global covariances).
# Offsets "proj" are q - (q.m^)m^; "raw" are q - m; "near" uses each query's nearest clusters, "rand" clusters drawn by size.
import numpy as np, time
P = "/tmp/claude-0/-home-user-mosaica/8c244050-1af9-5b77-a4e8-97638e77c857/scratchpad/vc/proto/"
VC = "/tmp/claude-0/-home-user-mosaica/8c244050-1af9-5b77-a4e8-97638e77c857/scratchpad/vc/"
t0 = time.time()
def log(*a): print(f"[{time.time()-t0:6.0f}s]", *a, flush=True)
def unit(a):
    a = np.asarray(a, dtype=np.float32); return a / np.linalg.norm(a, axis=1, keepdims=True)
I = np.load(P + "idx_m768/index.npz"); Xs = np.load(P + "idx_m768/Xs.npy", mmap_mode="r")
N, n, K, G = int(I["N"]), int(I["n"]), int(I["K"]), int(I["G"])
Mq = I["Mq"].astype(np.float32); mn = I["mn"]; Mh = (Mq / mn[:, None]).astype(np.float32)
assign, cstart, grp, qi, lamP, VP = I["assign"], I["cstart"], I["grp"], I["qi"], I["lam"], I["V"]
pc = np.diff(cstart) / N; w = np.bincount(grp[assign], minlength=G) / N
rng = np.random.default_rng(7); pool = np.setdiff1d(np.arange(N), qi)
def cov(Q, how, kn):
    S = np.zeros((n, n)); c = 0
    for i in range(0, len(Q), 200):
        q = Q[i:i+200]
        ks = rng.choice(K, (len(q), 40), p=pc) if how.startswith("rand") else np.argsort(-(q @ Mq.T), axis=1)[:, :kn]
        C = Mh[ks]; M = Mq[ks]
        D = q[:, None, :] - (np.einsum("bkn,bn->bk", C, q)[:, :, None] * C if how.endswith("proj") else M)
        D = D.reshape(-1, n).astype(np.float64); S += D.T @ D; c += len(D)
    return S / c
IT = unit(Xs[np.sort(rng.choice(pool, 3000, replace=False))]); IT2 = unit(Xs[np.sort(rng.choice(pool, 3000, replace=False))])
TT = unit(np.load(VC + "text_train.npy")); TQ = unit(np.load(VC + "text_q.npy"))
fits = {"item_rand_proj": cov(IT, "rand_proj", 0), "item_near4_proj": cov(IT, "near_proj", 4), "item_near4_raw": cov(IT, "near_raw", 4)}
St = cov(TT, "rand_proj", 0)
fits["mix_rand_proj"] = 0.5 * (fits["item_rand_proj"] / np.trace(fits["item_rand_proj"]) + St / np.trace(St))
pops = {"item near4 proj (held-out)": cov(IT2, "near_proj", 4), "item rand proj (held-out)": cov(IT2, "rand_proj", 0),
        "text test near50 proj": cov(TQ, "near_proj", 50), "text test rand proj": cov(TQ, "rand_proj", 0)}
log("covariances done")
def slack_pca(C): return sum(w[g] * np.sqrt(lamP[g] * np.einsum("ic,ij,jc->c", VP[g], C, VP[g])).sum() for g in range(G))
res = {"PCA": {k: slack_pca(C) for k, C in pops.items()}}
for name, SQ in fits.items():
    SQ = SQ + 0.1 * np.trace(SQ) / n * np.eye(n); lq, vq = np.linalg.eigh(SQ); A = (vq * np.sqrt(lq)) @ vq.T; Ai = (vq / np.sqrt(lq)) @ vq.T
    acc = {k: 0.0 for k in pops}
    for g in range(G):
        SR = (VP[g].astype(np.float64) * lamP[g]) @ VP[g].T
        gam, W = np.linalg.eigh(A @ SR @ A); gam = np.maximum(gam, 1e-12 * gam.max()); VQ = (Ai @ W) * gam ** 0.25
        for k, C in pops.items(): acc[k] += w[g] * np.sqrt(np.sqrt(gam) * np.einsum("ic,ij,jc->c", VQ, C, VQ)).sum()
    res[name] = acc; log(name, {k: round(v, 4) for k, v in acc.items()})
print("\nslack (smaller is better), rows = basis fit, columns = offset population")
print(f"{'':18s}" + "".join(f"{k:>28s}" for k in pops))
for r, d in res.items(): print(f"{r:18s}" + "".join(f"{d[k]:28.4f}" for k in pops))
