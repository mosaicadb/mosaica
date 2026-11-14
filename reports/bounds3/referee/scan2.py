import math, numpy as np, itertools, time
from mytail import *
ds = [95, 127, 255, 287]; J = 4; G = 36
lnds = [-math.log(8e18), -math.log(4e18), -math.log(8e15), -math.log(4e15)]
t0 = time.time(); res = []
for p in itertools.product(range(G + 1), repeat=3):
    if sum(p) > G: continue
    w = [x / G for x in p] + [1 - sum(p) / G]
    res.append(quantiles_fft(w, ds, lnds) + w)
R = np.array(res)
print(len(R), f"points, {time.time()-t0:.0f}s")
for i, nm in enumerate(["N1e9 two", "N1e9 one", "N1e6 two", "N1e6 one"]):
    o = np.argsort(-R[:, i])[:2]
    # local maxima on the grid: points not exceeded by any grid neighbour
    key = {tuple(np.round(r[4:7] * G).astype(int)): r[i] for r in R}
    loc = 0
    for kx, v in key.items():
        nb = [tuple(np.add(kx, dlt)) for dlt in itertools.product((-1, 0, 1), repeat=3) if any(dlt)]
        if all(key.get(q, -1) <= v for q in nb): loc += 1
    print(nm, "max", np.round(R[o[0], i], 5), "at", np.round(R[o[0], 4:], 3), "; grid local maxima:", loc, "; max on the faces (some share 0):", np.round(R[(R[:, 4:] == 0).any(1), i].max(), 4))
