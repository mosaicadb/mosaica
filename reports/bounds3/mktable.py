# Table of the exact quantile c(w) over shares (w1, w2, w3) on a cube grid, w4 = max(0, 1 - w1 - w2 - w3), normalised to
# unit variance, for several failure probabilities. For measurement on the dump.
import numpy as np, math, sys, time, os
from ctable import quantiles
ds = [95, 127, 255, 287]
K = int(sys.argv[1]) if len(sys.argv) > 1 else 40
LN = float(math.log(1e-9 / 1e9))            # eps / N
lnds = {"two": -math.log(2 * 1e9 * 4 / 1e-9), "one": LN - math.log(4), "L1": LN,
        "p90": LN + math.log(0.9), "p70": LN + math.log(0.7), "p50": LN + math.log(0.5), "p10": LN + math.log(0.1),
        "p033": LN + math.log(1 / 30), "p05": LN + math.log(0.05), "p02": LN + math.log(0.02), "p01": LN + math.log(0.01),
        "single": math.log(1e-9 / 4)}
g = np.arange(K + 1) / K
W3 = np.stack(np.meshgrid(g, g, g, indexing="ij"), -1).reshape(-1, 3)
w4 = np.maximum(0.0, 1 - W3.sum(1))
W = np.c_[W3, w4]
tot = W.sum(1); tot[tot == 0] = 1
keys = list(lnds); vals = [lnds[k] for k in keys]
out = np.zeros((len(W), len(keys)))
t0 = time.time()
for i in range(0, len(W), 4000):
    sl = slice(i, i + 4000)
    Wn = W[sl] / tot[sl, None]
    out[sl] = quantiles(Wn, ds, vals, T=25.0, npts=251)
    if i % 20000 == 0: print(i, len(W), f"{time.time()-t0:.0f}s", flush=True)
np.savez(f"ctab_{K}.npz", K=K, keys=np.array(keys), lnds=np.array(vals), tab=out.reshape(K + 1, K + 1, K + 1, len(keys)))
print("saved", f"{time.time()-t0:.0f}s; max over grid by key:", {k: round(float(out[:, i].max()), 4) for i, k in enumerate(keys)})
