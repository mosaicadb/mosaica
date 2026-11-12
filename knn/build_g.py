# Build the index for one dataset: clusters with 8-bit centres, groups with local PCA bases, query-offset profiles,
# items sorted by (group, cluster, stored a = x.m^), test queries and per-query thresholds, anchors and the support table.
# usage: python3 build.py NAME
import numpy as np, sys, os, json
sys.path.insert(0, "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/combined")
from core import *
ROOT = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/"
CFG = {
    "gfull": dict(files=[ROOT + "knn/gdata/train.npy"], K=1000, G=64, nqi=0, text=False),
    "m768":  dict(files=[ROOT + f"data/e{i:02d}.npy" for i in range(20)], K=1000, G=64, nqi=200, text=True),
    "p768":  dict(files=[ROOT + f"data/e{i:02d}.npy" for i in (0, 5, 10, 15)], K=200, G=16, nqi=60, text=True, ntext=90),
    "n1024": dict(files=[ROOT + f"agent3b/data1024/f{i:02d}.npy" for i in range(4)], K=200, G=16, nqi=200, text=False),
    "p128":  dict(files=[ROOT + f"data/e{i:02d}.npy" for i in (0, 5, 10, 15)], K=200, G=16, nqi=200, text=True, pca=128),
    "g100":  dict(files=[ROOT + f"combined/data100/g{i:02d}.npy" for i in range(4)], K=200, G=16, nqi=200, text=False),
}
NAME = sys.argv[1]; C = CFG[NAME]; OUT = ROOT + f"knn/idx_{NAME}/"; os.makedirs(OUT, exist_ok=True)
rng = np.random.default_rng(20261112)
X16 = np.concatenate([np.load(f) for f in C["files"]]); N, n = X16.shape
X = np.empty((N, n), np.float32)
for i in range(0, N, 100_000): X[i:i+100_000] = unit(X16[i:i+100_000])
del X16
PCA = None
if C.get("pca"):
    s_ = X[rng.choice(N, 50_000, replace=False)]; w_, P_ = np.linalg.eigh((s_.T @ s_).astype(np.float64))
    PCA = P_[:, ::-1][:, :C["pca"]].astype(np.float32); X = unit(X @ PCA); n = X.shape[1]
def proj(T): return unit(T @ PCA) if PCA is not None else T
log(NAME, "N", N, "n", n)
K, G = C["K"], C["G"]
sub = X[rng.choice(N, min(N, 200_000), replace=False)]
Chat = kmeans(sub, K, rng); del sub
assign = np.concatenate([(X[i:i+50_000] @ Chat.T).argmax(1) for i in range(0, N, 50_000)])
keep = np.unique(assign); remap = -np.ones(K, int); remap[keep] = np.arange(len(keep)); assign = remap[assign]; K = len(keep)
cnt = np.bincount(assign, minlength=K)
Msum = np.zeros((K, n)); np.add.at(Msum, assign, X); Mean = Msum / cnt[:, None]
msc = np.abs(Mean).max(1) / 127; Mq = (np.round(Mean / msc[:, None]) * msc[:, None]).astype(np.float32)   # 8-bit centres + one scale
mn = np.linalg.norm(Mq, axis=1).astype(np.float32); Mh = Mq / mn[:, None]
grp = (Mh @ kmeans(Mh.copy(), G, rng, 20).T).argmax(1)
a = np.einsum("ij,ij->i", X, Mh[assign]); ahat = np.round(np.clip(a, -1, 1) * 32767).astype(np.int16)
order = np.lexsort((ahat, assign, grp[assign]))
X = X[order]; assign = assign[order]; ahat = ahat[order]
# re-number clusters so they are contiguous in the sorted order
newid = -np.ones(K, int); seen = assign[np.r_[0, np.where(np.diff(assign))[0] + 1]]; newid[seen] = np.arange(K)
assign = newid[assign]; Mq = Mq[seen]; mn = mn[seen]; Mh = Mh[seen]; grp = grp[seen]; cnt = cnt[seen]; msc = msc[seen]
cstart = np.r_[0, np.cumsum(cnt)]
assert np.all(np.diff(assign) >= 0) and np.all(np.diff(grp) >= 0)
np.save(OUT + "Xs.npy", X.astype(np.float16)); np.save(OUT + "perm.npy", order)
# local bases
lam = np.zeros((G, n)); V = np.zeros((G, n, n), np.float32)
gstart = np.searchsorted(grp[assign], np.arange(G + 1))
for g in range(G):
    Cv = np.zeros((n, n))
    for i in range(gstart[g], gstart[g + 1], 50_000):
        j = min(i + 50_000, gstart[g + 1]); R = X[i:j] - Mq[assign[i:j]]; Cv += (R.T @ R).astype(np.float64)
    l, v = np.linalg.eigh(Cv / max(gstart[g + 1] - gstart[g], 1))
    lam[g] = np.maximum(l[::-1], 1e-12); V[g] = v[:, ::-1]
log("bases done; group sizes", np.diff(gstart).tolist())
# test queries
qi = np.zeros(0, np.int64); Q = [unit(np.load(ROOT + "knn/gdata/test.npy")[:200])]; qcls = ["test"] * 200
if C["text"]:
    T = proj(unit(np.load(ROOT + "agent2/text_q.npy")))
    if "ntext" in C: T = T[np.sort(rng.choice(len(T), C["ntext"], replace=False))]
    Q.append(T); qcls += ["text"] * len(T)
Q = np.concatenate(Q).astype(np.float32); qcls = np.array(qcls); NQ = len(Q)
# query-offset profiles, fitted on corpus items that are not test queries and on the text training prompts
pool = np.setdiff1d(np.arange(N), qi); pq = X[rng.choice(pool, 5000, replace=False)]
pk = np.argsort(-(pq @ Mq.T), axis=1)[:, :4]
Qi = np.zeros((G, n)); Qt = np.zeros((G, n))
for g in range(G):
    rows, cols = np.where(grp[pk] == g)
    Qi[g] = (((pq[rows] - Mq[pk[rows, cols]]) @ V[g]) ** 2).mean(0) if len(rows) > 50 else lam[g]
if C["text"]:
    Ttr = proj(unit(np.load(ROOT + "agent2/text_train.npy"))); tk = np.argsort(-(Ttr @ Mq.T), axis=1)[:, :50]
    for g in range(G):
        rows, cols = np.where(grp[tk] == g)
        if len(rows) < 200:
            ks = np.where(grp == g)[0]; rows = np.repeat(np.arange(len(Ttr)), 2); kk = ks[np.arange(len(rows)) % len(ks)]
        else: kk = tk[rows, cols]
        Qt[g] = (((Ttr[rows] - Mq[kk]) @ V[g]) ** 2).mean(0)
    Qmix = ((np.sqrt(Qi) + np.sqrt(Qt)) / 2) ** 2
else: Qt = Qi.copy(); Qmix = Qi.copy()
thr = np.zeros((NQ, 8), np.float32); SIZES = [0]
# anchors and support table: exact max/min over each cluster of p.x, rounded outward to 2^-15
anchors = [Mh]; nimg = K
if C["text"]: anchors.append(kmeans(Ttr.copy(), 256, rng, 20))
P = np.concatenate(anchors).astype(np.float32); A = len(P)
hi = np.full((A, K), -np.inf, np.float32); lo = np.full((A, K), np.inf, np.float32)
for c0 in range(0, K, 50):
    c1 = min(c0 + 50, K); PX = X[cstart[c0]:cstart[c1]] @ P.T; st = cstart[c0:c1] - cstart[c0]
    hi[:, c0:c1] = np.maximum.reduceat(PX, st, axis=0).T; lo[:, c0:c1] = np.minimum.reduceat(PX, st, axis=0).T
hi = np.ceil(hi.astype(np.float64) * 32768) / 32768; lo = np.floor(lo.astype(np.float64) * 32768) / 32768
np.savez(OUT + "index.npz", N=N, n=n, K=K, G=G, assign=assign.astype(np.int32), cstart=cstart, grp=grp, Mq=Mq, mn=mn, msc=msc,
         ahat=ahat, lam=lam, V=V, Qi=Qi, Qt=Qt, Qmix=Qmix, Q=Q, qcls=qcls, qi=qi, thr=thr, sizes=np.array(SIZES), P=P, nimg=nimg,
         hi=hi.astype(np.float32), lo=lo.astype(np.float32))
log("saved; K", K, "G", G, "NQ", NQ, "mean thresholds by class:",
    {c: np.round(thr[qcls == c].mean(0), 3).tolist() for c in np.unique(qcls)})
