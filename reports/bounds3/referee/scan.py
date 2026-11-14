import math, numpy as np, itertools, time, sys
from mytail import *
from scipy.optimize import minimize
ds = [int(x) for x in sys.argv[1].split(",")]
J = len(ds)
lnds = {"N1e9 two": -math.log(2 * 1e9 * 4 / 1e-9), "N1e9 one": -math.log(1e9 * 4 / 1e-9), "N1e6 two": -math.log(2 * 1e6 * 4 / 1e-9), "N1e6 one": -math.log(1e6 * 4 / 1e-9)}
print({k: round(v, 6) for k, v in lnds.items()})
G = int(sys.argv[2]) if len(sys.argv) > 2 else 16
t0 = time.time()
pts = [p for p in itertools.product(range(G + 1), repeat=J - 1) if sum(p) <= G]
res = []
for p in pts:
    w = [x / G for x in p] + [1 - sum(p) / G]
    res.append((quantiles_fft(w, ds, list(lnds.values())), w))
print(f"grid {len(pts)} points {time.time()-t0:.0f}s")
for i, name in enumerate(lnds):
    r = sorted(res, key=lambda t: -t[0][i])[:3]
    print(name, "top grid:", [(round(q[i], 5), np.round(w, 3).tolist()) for q, w in r])
    # refine from the best grid point (softmax parametrisation), and from d_j + 2 shares
    def f(z, i=i):
        e = np.exp(z - z.max()); w = e / e.sum()
        return -quantiles_fft(w, ds, [list(lnds.values())[i]])[0]
    best = None
    for w0 in (r[0][1], [(d + 2) / (sum(ds) + 2 * J) for d in ds], [1 / J] * J):
        z0 = np.log(np.maximum(w0, 1e-3))
        o = minimize(f, z0, method="Nelder-Mead", options={"xatol": 1e-5, "fatol": 1e-9, "maxiter": 600})
        e = np.exp(o.x - o.x.max()); w = e / e.sum()
        if best is None or -o.fun > best[0]: best = (-o.fun, w)
    print("   sup c =", round(best[0], 5), "at shares", np.round(best[1], 4).tolist(), "; (d+2) shares:", np.round([(d + 2) / (sum(ds) + 2 * J) for d in ds], 4).tolist(), f"[{time.time()-t0:.0f}s]")
