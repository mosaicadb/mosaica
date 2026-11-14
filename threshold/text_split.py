# Text queries for the same corpus as split.py: the BioCLIP-2 text embeddings of text_q.json in place of the held-out
# items, thresholds by split.py's rule (midpoint of the k-th and (k+1)-th exact score at shares 0.01 / 0.1 / 1 / 10 / 30 %).
# The fit (fit/) is the item run's, data-only and unchanged; link it and corpus.npy into the new work directory first.
# usage: text_split.py OUTDIR TEXT_Q_NPY
import sys, numpy as np
OUT, TQ = sys.argv[1], sys.argv[2]
Q = np.load(TQ).astype(np.float32); Q /= np.linalg.norm(Q, axis=1, keepdims=True); np.save(f"{OUT}/queries.npy", Q); NQ = len(Q)
C = np.load(f"{OUT}/corpus.npy").astype(np.float32); C /= np.linalg.norm(C, axis=1, keepdims=True)
SH = (1e-4, 1e-3, 1e-2, 0.1, 0.3); ks = [int(round(len(C) * s)) for s in SH]
thr = np.zeros((NQ, len(SH)))
for j in range(NQ):
    s = -np.sort(-(C @ Q[j]).astype(np.float64))
    thr[j] = [(s[k - 1] + s[k]) / 2 for k in ks]
np.save(f"{OUT}/thr.npy", thr); print("NQ", NQ, "ks", ks, "mean thresholds", thr.mean(0).round(4), "max score", float((C @ Q.T).max()))
