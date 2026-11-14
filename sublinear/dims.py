# Step 7. A proxy for other dimensions on the same corpus: project items and queries on the top d principal directions of
# the (uncentred) corpus, renormalise, and write the exact scores of the d-dimensional problem (S_d.f32). tail.py then
# gives the tail shape and intrinsic-dimension statistics at dimension d. This is a truncation of one model, not another
# embedding model.
import numpy as np, os
from common import *
Q, qtype, qkind, qi = queries()
C = np.zeros((n, n), np.float64)
for i, X in chunks(): C += (X.T @ X).astype(np.float64)
l, v = np.linalg.eigh(C / N); P = v[:, ::-1].astype(np.float32); lam = l[::-1]
log("energy in the top 32 / 96 / 256 directions", [float(lam[:d].sum() / lam.sum()) for d in (32, 96, 256)])
DS = [32, 96, 256]; QP = Q @ P[:, :max(DS)]
fs = {d: open(os.path.join(OUT, f"S_{d}.tmp"), "wb") for d in DS}
for d in DS: fs[d].truncate(NQ * N * 4)
for i, X in chunks():
    XP = X @ P[:, :max(DS)]
    for d in DS:
        S = (QP[:, :d] / np.linalg.norm(QP[:, :d], axis=1, keepdims=True)) @ (XP[:, :d] / np.linalg.norm(XP[:, :d], axis=1, keepdims=True)).T
        for q in range(NQ):
            fs[d].seek((q * N + i) * 4); S[q].astype(np.float32).tofile(fs[d])
for d in DS:
    fs[d].close(); os.rename(os.path.join(OUT, f"S_{d}.tmp"), os.path.join(OUT, f"S_{d}.f32"))
log("done")
