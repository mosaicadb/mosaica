# Corpus and queries: a random 150k of the 200k BioCLIP-2 sample as the corpus, queries drawn from the other 50k (held out).
# Thresholds at answer shares 0.01 / 0.1 / 1 / 10 / 30 % of the corpus: midpoint of the k-th and (k+1)-th exact scores.
# usage: split.py OUTDIR NQ
import sys, numpy as np
OUT, NQ = sys.argv[1], int(sys.argv[2])
X = np.concatenate([np.load(f"/home/joe/code/vexact-data/bioclip2-200k/g{i}.npy") for i in range(4)])
p = np.random.default_rng(20261114).permutation(len(X)); cor, held = p[:150_000], p[150_000:]
np.save(f"{OUT}/corpus.npy", X[np.sort(cor)])
qi = np.sort(np.random.default_rng(1).choice(held, NQ, replace=False))
Q = X[qi].astype(np.float32); Q /= np.linalg.norm(Q, axis=1, keepdims=True); np.save(f"{OUT}/queries.npy", Q)
C = X[np.sort(cor)].astype(np.float32); C /= np.linalg.norm(C, axis=1, keepdims=True)
SH = (1e-4, 1e-3, 1e-2, 0.1, 0.3); ks = [int(round(len(C) * s)) for s in SH]
thr = np.zeros((NQ, len(SH)))
for j in range(NQ):
    s = -np.sort(-(C @ Q[j]).astype(np.float64))
    thr[j] = [(s[k - 1] + s[k]) / 2 for k in ks]
np.save(f"{OUT}/thr.npy", thr); print("ks", ks, "mean thresholds", thr.mean(0).round(4), "max query-corpus cosine", None)
