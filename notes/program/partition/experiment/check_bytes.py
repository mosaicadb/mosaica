# Check: the exact solution for S1b against the bisection, on 3,000 held-out rows of A0's group 0.
from lib import *
X = load_X(); cell = s0["a"].astype(np.int32); grp = s0["grp"]
M, _ = means(X, cell, 200); Cc, c2 = scatters(X, cell, 200, M, FIT); covs, _ = group_covs(Cc, c2, grp, 12)
gam, T = basis(covs[0], A_SI)
r = np.flatnonzero(HELD & (grp[cell] == 0))[:3000]
Z = (X[r].astype(np.float64) - M[cell[r]]) @ T; z2 = Z * Z
a = bytes_at(z2, gam, it=80); b = bytes_exact(z2, gam)
print("max abs diff bytes", np.abs(a - b).max(), "means", a.mean(0), b.mean(0))
for j, ws in enumerate(WSTARS):   # width at the solved level equals w*
    i = np.flatnonzero(b[:, j] > 0)[:5]
    for ii in i:
        th = None
        lo, hi = np.log(gam[-1]) - 60, np.log(gam[0])
        for _ in range(200):
            mid = (lo + hi) / 2
            if 0.5 * np.maximum(np.log(gam) - mid, 0).sum() / (8 * np.log(2)) > b[ii, j]: lo = mid
            else: hi = mid
        s = np.minimum(1, np.sqrt(np.exp(mid) / gam)); print(ws, np.sqrt((s * np.sqrt(gam)).sum() * (s * z2[ii]).sum()))
