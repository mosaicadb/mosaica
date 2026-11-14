# The largest Chernoff half-width (in units of sigma) over block shares, with the exact moment generating function of
# a coordinate of a uniform unit vector in R^d per block: max over w (simplex) of min over lambda of
# (L + sum_j psi_dj(lambda sqrt(w_j d_j))) / lambda.  L = ln(2 N L_levels / eps) as in the build.
import numpy as np, math, itertools, sys
from scipy.special import ive, gammaln
L = 43.525973215572655
c = math.sqrt(2 * L)
def psi(d, x):
    nu = d / 2 - 1; x = np.maximum(x, 1e-9)
    return gammaln(nu + 1) - nu * np.log(x / 2) + np.log(ive(nu, x)) + x
def ceff(ds, step=0.025):
    J = len(ds); g = np.arange(0, 1 + 1e-9, step)
    pts = np.array([p for p in itertools.product(g, repeat=J - 1) if sum(p) <= 1 + 1e-9])
    W = np.c_[pts, 1 - pts.sum(1)]
    best = np.full(len(W), np.inf)
    for lam in c * np.geomspace(0.7, 3, 60):
        v = L + sum(np.where(W[:, j] > 0, psi(ds[j], lam * np.sqrt(W[:, j] * ds[j])), 0) for j in range(J))
        best = np.minimum(best, v / lam)
    i = np.argmax(best)
    return best[i], W[i]
for ds in ([95], [95, 127], [95, 127, 255], [95, 127, 255, 287], [255, 287], [287]):
    v, w = ceff(ds)
    print(ds, f"c_eff {v:.3f} (c {c:.3f}, ratio {v/c:.3f}) at shares {np.round(w,3)}")
