# Accuracy of the vectorised inversion with fewer nodes, against the 801-node version and the exact beta corner values.
import numpy as np, math, time
from ctable import quantiles
from law import c1
ds = [95, 127, 255, 287]
rng = np.random.default_rng(0)
W = np.vstack([rng.dirichlet([0.6] * 4, 300), np.eye(4), [[0.5, 0.5, 0, 0], [0, 0, 0.5, 0.5], [0.01, 0.01, 0.01, 0.97]]])
lnds = [-43.526, -41.4465, -22.11]
ref = quantiles(W, ds, lnds, T=60.0, npts=2401)
for T, npts in ((40, 801), (30, 401), (30, 301), (25, 251)):
    t0 = time.time(); q = quantiles(W, ds, lnds, T=T, npts=npts)
    print(T, npts, "max abs diff", np.abs(q - ref).max(0), f"{time.time()-t0:.2f}s")
for lam in (6.0, 12.0):
    q = quantiles(W, ds, lnds, lam=lam, T=60.0, npts=2401)
    print("lam", lam, "max abs diff", np.abs(q - ref).max(0))
print("corners vs beta:", [(round(ref[300 + j, 0], 6), round(c1(ds[j], lnds[0]), 6)) for j in range(4)])
print("corner d=95 at -22.11:", ref[300, 2], c1(95, -22.11))
