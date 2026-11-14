# An independent float64 oracle for threshold queries and k-NN on the Rust index, sharing no code with it: brute force
# in numpy over the stored f16 vectors of the fit (Xs.npy, perm.npy), with the queries exactly as the index used them.
# Checks every result written by `vexact --v2 ... oracle` (RESULTS/oracle): a threshold result is wrong where an entity
# is in it and scores below t, or scores at least t and is missing; entities whose float64 score lies within 1e-12 of
# t are counted apart (the index decides those exactly; float64 may round either way). A k-NN result is wrong when its
# list of exact scores differs from the oracle's top 10 by more than 1e-9.
# usage: python3 oracle_thr.py INDEX_DIR ORACLE_DIR
import os, sys
import numpy as np

idx, odir = sys.argv[1], sys.argv[2]
X = np.load(os.path.join(idx, "Xs.npy"), mmap_mode="r")
perm = np.load(os.path.join(idx, "perm.npy"))
Q = np.load(os.path.join(odir, "queries.npy")).astype(np.float64)
thr = np.load(os.path.join(odir, "thr.npy"))
ids = np.load(os.path.join(odir, "res_ids.npy"))
off = np.load(os.path.join(odir, "res_off.npy"))
knn = np.load(os.path.join(odir, "knn.npy"))
vis = np.zeros(perm.max() + 1, bool)
vis[np.load(os.path.join(odir, "vis.npy"))] = True
nq, ns = thr.shape
nf = 2  # the last two thresholds of each query are over the visible set
S = np.empty((len(perm), nq))
for a in range(0, len(perm), 100_000):
    S[a:a + 100_000] = np.asarray(X[a:a + 100_000], np.float64) @ Q.T
score = np.empty_like(S)
score[perm] = S  # by entity
seen = vis[np.arange(len(score))]
wrong = border = checked = 0
for q in range(nq):
    for s in range(ns):
        i = q * ns + s
        t = thr[q, s]
        got = np.zeros(len(score), bool)
        got[ids[off[i]:off[i + 1]]] = True
        want = score[:, q] >= t
        if s >= ns - nf:
            want &= seen
        diff = got != want
        near = np.abs(score[:, q] - t) <= 1e-12
        wrong += int((diff & ~near).sum())
        border += int((diff & near).sum())
        checked += 1
kw = 0
for q in range(nq):
    for f in range(2):
        sc = score[:, q] if f == 0 else np.where(seen, score[:, q], -np.inf)
        top = np.sort(-np.partition(-sc, 9)[:10])[::-1]
        got = np.sort(score[knn[q, f], q])[::-1]
        kw += int(np.abs(got - top).max() > 1e-9)
print(f"oracle: {checked} threshold results ({nq} queries x {ns - nf} answer shares unfiltered + {nf} over {seen.sum()} visible), "
      f"{wrong} entities wrong, {border} within 1e-12 of t; k-NN {2 * nq} results, {kw} differ from the oracle")
