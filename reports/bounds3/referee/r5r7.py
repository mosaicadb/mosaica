import math, numpy as np, itertools
from mytail import *
ds = [95, 127, 255, 287]; G = 24
one = -math.log(4e18)
lnds = [one, -math.log(1e18), -math.log(1e18 / 0.7), -math.log(1e18 / 0.25), -math.log(1e18 / 0.1), -math.log(3e18), -math.log(5e18)]
planes = {"96B": (8.426, 8.323, 8.860, 9.316), "160B": (8.379, 8.610, 8.660, 9.217), "256B": (8.861, 8.789, 8.449, 9.079)}
worst = {k: (-9, None) for k in planes}; sup = np.zeros(len(lnds))
for p in itertools.product(range(G + 1), repeat=3):
    if sum(p) > G: continue
    w = [x / G for x in p] + [1 - sum(p) / G]
    q = quantiles_fft(w, ds, lnds)
    sup = np.maximum(sup, q)
    for k, kap in planes.items():
        gap = q[0] - math.sqrt(sum(kj * kj * wj for kj, wj in zip(kap, w)))
        if gap > worst[k][0]: worst[k] = (gap, w)
for k in planes: print("R5 plane", k, "largest c(w) - sqrt(sum kappa_j w_j) on a 1/24 grid:", round(worst[k][0], 4), "at", np.round(worst[k][1], 3).tolist(), "(must be <= 0)")
print("R7 grid sup of c (1/24 grid, slightly below the true sup): weights all / 0.7 / 0.25 / 0.1:", np.round(sup[1:5], 3).tolist(), "(report 8.551 8.589 8.695 8.789);  L = 3, 5:", np.round(sup[5:], 3).tolist(), "(report 8.666 8.718)")
