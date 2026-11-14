# Turn a fit.py index directory into the index.npz that combined/run.py reads (format of combined/build.py), with the
# data-only H25 profile in place of both the item profile (deterministic levels) and the mixture (randomised levels),
# the held-out queries, and one threshold column per (answer share, tolerance d): thr = t, tl = t - d.
# Anchors: the cluster centres only (fit.py has no text anchors), support table exact and rounded outward as in build.py.
# usage: adapt.py WORKDIR
import sys, json, numpy as np
W = sys.argv[1]; F = W + "/fit/"
m = json.load(open(F + "meta.json")); N, n, K, G = m["N"], m["n"], m["K"], m["G"]
X = np.load(F + "Xs.npy").astype(np.float32); X /= np.maximum(np.linalg.norm(X, axis=1, keepdims=True), 1e-12)
cstart = np.load(F + "cstart.npy"); grp = np.load(F + "grp.npy"); msc = np.load(F + "msc.npy")
Mq = (np.load(F + "mq8.npy").astype(np.float64) * msc[:, None]).astype(np.float32)
mn = np.linalg.norm(Mq, axis=1).astype(np.float32); Mh = Mq / mn[:, None]
assign = np.repeat(np.arange(K), np.diff(cstart)).astype(np.int32)
ahat = np.round(np.clip(np.einsum("ij,ij->i", X, Mh[assign]), -1, 1) * 32767).astype(np.int16)
prof = np.load(F + "profile.npy")
Q = np.load(W + "/queries.npy").astype(np.float32); T = np.load(W + "/thr.npy")
DS = (0.0, 0.002, 0.005, 0.01)
thr = np.concatenate([T for d in DS], 1).astype(np.float32); tl = np.concatenate([T - d for d in DS], 1).astype(np.float32)
hi = np.zeros((K, K), np.float32); lo = np.zeros((K, K), np.float32)
for c in range(K):
    px = X[cstart[c]:cstart[c + 1]] @ Mh.T; hi[:, c] = px.max(0); lo[:, c] = px.min(0)
hi = (np.ceil(hi.astype(np.float64) * 32768) / 32768).astype(np.float32); lo = (np.floor(lo.astype(np.float64) * 32768) / 32768).astype(np.float32)
np.savez(W + "/index.npz", N=N, n=n, K=K, G=G, assign=assign, cstart=cstart, grp=grp, Mq=Mq, mn=mn, msc=msc, ahat=ahat,
         lam=np.load(F + "lam.npy"), V=np.load(F + "V.npy"), Qi=prof, Qt=prof, Qmix=prof, Q=Q, qcls=np.array(["item"] * len(Q)),
         qi=np.zeros(0, int), thr=thr, tl=tl, ds=np.array(DS), sizes=np.array([15, 150, 1500, 15000, 45000]), P=Mh, nimg=K, hi=hi, lo=lo)
print("index.npz: N", N, "K", K, "G", G, "NQ", len(Q), "columns", thr.shape[1])
