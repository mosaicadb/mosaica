# Stage 0, block norms: the shared setup on the 200k BioCLIP-2 sample.
# Spherical k-means (K = 200) and G = 12 groups of clusters, as reports/surge/ood/standin.py; 100 held-out item queries;
# query-offset covariances from training queries only (corpus pseudo-queries, the 2,206 text training prompts, and their
# normalised mix, as groups/scripts/sigmaq.py); exact thresholds for every test query at answers of 0.01/0.1/1/5 %.
# The vectors are read from the f16 files in chunks; nothing of size N x n is held in memory.
# usage: prep.py OUTDIR
import numpy as np, sys, time, json
O = sys.argv[1]; t0 = time.time()
DD = "/home/joe/code/vexact-data/bioclip2-200k/"; TD = "/home/joe/code/vexact-data/bioclip2-text/"
def log(*a): print(f"[{time.time()-t0:6.0f}s]", *a, flush=True)
F = [np.load(DD + f"g{i}.npy", mmap_mode="r") for i in range(4)]
NF = [len(f) for f in F]; N = sum(NF); n = F[0].shape[1]; OFF = np.concatenate([[0], np.cumsum(NF)])
CH = 25_000
def chunks():
    for fi, f in enumerate(F):
        for s in range(0, len(f), CH):
            x = np.asarray(f[s:s + CH], dtype=np.float32); x /= np.linalg.norm(x, axis=1, keepdims=True)
            yield OFF[fi] + s, x
def rows(idx):
    idx = np.asarray(idx); out = np.empty((len(idx), n), np.float32)
    for fi in range(4):
        m = (idx >= OFF[fi]) & (idx < OFF[fi + 1])
        if m.any(): out[m] = F[fi][np.sort(idx[m] - OFF[fi])][np.argsort(np.argsort(idx[m] - OFF[fi]))]
    out /= np.linalg.norm(out, axis=1, keepdims=True); return out
rng = np.random.default_rng(1)
K, G = 200, 12
M = rows(np.sort(rng.choice(N, K, replace=False)))
a = np.empty(N, np.int32)
for it in range(9):
    S = np.zeros((K, n)); cnt = np.zeros(K)
    for s, x in chunks():
        aa = (x @ M.T).argmax(1); a[s:s + len(x)] = aa
        order = np.argsort(aa, kind="stable"); u, st = np.unique(aa[order], return_index=True)
        S[u] += np.add.reduceat(x[order].astype(np.float64), st, axis=0); cnt[u] += np.diff(np.append(st, len(aa)))
    if it == 8: break
    emp = cnt == 0; M = np.where(emp[:, None], M, S / np.maximum(cnt, 1)[:, None]).astype(np.float32)
    M /= np.linalg.norm(M, axis=1, keepdims=True)
    log("k-means iteration", it, "empty", int(emp.sum()))
Mean = (S / cnt[:, None]).astype(np.float64)          # cluster means of the final assignment (not unit)
log("cluster sizes min/median/max", int(cnt.min()), int(np.median(cnt)), int(cnt.max()))
gc = Mean[rng.choice(K, G, replace=False)].copy()
for it in range(10):
    grp = (Mean @ gc.T).argmax(1)
    for g in range(G):
        if (grp == g).any(): gc[g] = Mean[grp == g].mean(0)
grp = (Mean @ gc.T).argmax(1).astype(np.int32)
gsz = np.bincount(grp[a], minlength=G); log("group sizes", gsz.tolist())
# held-out item queries; the pseudo-query sample for the data-only fit excludes them
qi = np.sort(rng.choice(N, 100, replace=False))
pool = np.setdiff1d(np.arange(N), qi); pi = np.sort(rng.choice(pool, 3000, replace=False))
Mh = Mean / np.linalg.norm(Mean, axis=1, keepdims=True); p = cnt / N
def cov(Q, seed):
    r = np.random.default_rng(seed); Sx = np.zeros((n, n))
    for q in Q.astype(np.float64):
        c = Mh[r.choice(K, 40, p=p)]; D = q[None] - (c @ q)[:, None] * c; Sx += D.T @ D
    return Sx / (40 * len(Q))
Si = cov(rows(pi), 3)
T = np.load(TD + "text_train.npy").astype(np.float64); T /= np.linalg.norm(T, axis=1, keepdims=True)
St = cov(T, 4); Sm = 0.5 * (Si / np.trace(Si) + St / np.trace(St))
log("Sigma_Q traces item / text", np.trace(Si).round(4), np.trace(St).round(4))
# test queries: held-out text prompts, in-sample species prompts, held-out items
Qt = np.load(TD + "text_q.npy").astype(np.float64); Qs = np.load(TD + "text_insample.npy").astype(np.float64)
Qi = rows(qi).astype(np.float64)
Q = np.concatenate([Qt, Qs, Qi]); Q /= np.linalg.norm(Q, axis=1, keepdims=True)
kind = np.array([0] * len(Qt) + [1] * len(Qs) + [2] * len(Qi), np.int8)
sub = [p_[0] for p_ in json.load(open(TD + "text_q.json"))] + ["sci-in"] * len(Qs) + ["item"] * len(Qi)
FR = [1e-4, 1e-3, 1e-2, 5e-2]; KEEP = int(max(FR) * N) + 1
top = np.full((len(Q), 0), -np.inf, np.float32)
for s, x in chunks():
    sc = (x.astype(np.float64) @ Q.T).T.astype(np.float32)                # nq x chunk
    top = np.concatenate([top, sc], 1); top = -np.partition(-top, KEEP - 1, axis=1)[:, :KEEP]
top = -np.sort(-top, axis=1)
TH = np.stack([top[:, round(f * N) - 1] for f in FR], 1)                  # the round(f N)-th best score
log("thresholds, mean by kind", [TH[kind == k].mean(0).round(3).tolist() for k in range(3)])
np.savez(f"{O}/setup.npz", a=a, Mean=Mean, cnt=cnt, grp=grp, qi=qi, pi=pi, Si=Si, St=St, Sm=Sm, Q=Q, kind=kind, TH=TH,
         FR=np.array(FR), OFF=OFF)
json.dump(sub, open(f"{O}/query_kinds.json", "w"))
log("done")
