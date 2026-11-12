# Operating points (expected bits, sqrt normalised MSE) of per-coordinate codes on N(0,1), and the allocation of the
# escape code at 4..32 B on the p768 spectra: where do the deterministic bits go, and what does the CS bound lose there.
import numpy as np, sys
sys.path.insert(0, "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/combined")
from core import *
print("escape: depth, cumulative expected bits, sqrt g, RD sqrt g at that rate")
cb = np.cumsum(np.r_[0, cE[:8]])
for j in range(8): print(j, round(cb[j], 3), round(np.sqrt(gE[j]), 4), round(2 ** -cb[j], 4))
# ideal ECSQ (uniform midtread / midrise, step d, ideal entropy) on N(0,1)
xs = np.linspace(-9, 9, 360001); p = np.exp(-xs * xs / 2); p /= p.sum()
def ecsq(d, off):
    idx = np.floor(xs / d + off); u, inv = np.unique(idx, return_inverse=True)
    P = np.bincount(inv, p); M = np.bincount(inv, p * xs) / np.maximum(P, 1e-300)
    H = -(P[P > 0] * np.log2(P[P > 0])).sum(); g = (p * (xs - M[inv]) ** 2).sum(); return H, g
print("ideal ECSQ: rate, sqrt g, RD")
pts = []
for d in np.exp(np.linspace(np.log(0.15), np.log(4), 40)):
    for off in (0.0, 0.5):
        H, g = ecsq(d, off); pts.append((H, np.sqrt(g)))
pts.sort()
for R in (0.5, 1, 1.5, 2, 2.5, 3, 4):
    best = min(s for h, s in pts if h <= R + 1e-9)
    # escape hull at R
    eh = np.interp(R, cb[:8], np.sqrt(gE[:8]))
    print(R, "ECSQ", round(best, 4), "escape hull", round(eh, 4), "RD", round(2 ** -R, 4), "escape/ECSQ", round(eh / best, 3), "escape/RD", round(eh / 2 ** -R, 3))
I = dict(np.load("/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/combined/idx_p768/index.npz"))
for g in (0, 5):
    lam = I["lam"][g]
    for nm in ("Qi", "Qt", "Qmix"):
        Qp = I[nm][g]; se, dep_at, sdep, expc = esc_steps(lam, I["Qi"][g], 1.5 * 8 * 32)
        gam = lam * Qp
        for B in (4, 8, 16, 32):
            de = dep_at[int((expc <= 8 * B).sum())]
            hcs = np.sqrt(gam * gE[de]).sum()
            # conjectured floor sum sqrt(min(gam, theta)) with sum 0.5 log2+(gam/theta) = 8B bits (+8 bits of norm charged)
            lo, hi = 1e-20, gam.max()
            for _ in range(200):
                th = np.sqrt(lo * hi); R = 0.5 * np.log2(np.maximum(gam / th, 1)).sum()
                if R > 8 * B + 8: lo = th
                else: hi = th
            fl = np.sqrt(np.minimum(gam, th)).sum()
            print(f"group {g} profile {nm:4s} {B:2d} B: coords at depth 1/2/3+: {(de == 1).sum()}/{(de == 2).sum()}/{(de >= 3).sum()};"
                  f" CS sum sqrt(gam g) {hcs:.4f} floor {fl:.4f} ratio {hcs / fl:.3f}; level-0 CS {np.sqrt(gam).sum():.4f}")
