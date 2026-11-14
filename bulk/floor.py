# Floors of the proofs on the measured spectra, as bytes needed to bring a sound interval down to a share R of its
# zero-read value. gamma_i per group are taken from the dual pair fitted to BASIS (variance of a dual coordinate = sqrt(gamma_i)).
#   proved   (Corollary 1):  max_k kappa_k (gamma_1..gamma_k)^(1/2k) 2^(-B/k)
#   conject. (section 6):    sum_i sqrt(min(gamma_i, theta)), theta from reverse water-filling at B bits
#   ideal randomised:        c * sqrt(sum_i min(gamma_i, theta))   (rate-distortion error sd of the estimate, times c)
#   exact coordinates:       sum_{i>k} sqrt(gamma_i) after the first k coordinates, read exactly (the unit of the coordinate tables)
import numpy as np, os, sys
from scipy.special import gammaln
from common import setup, log
BASIS = os.environ.get("BASIS"); C = float(os.environ.get("C", "9.47"))
D = setup(BASIS, nq_text_step=400, nq_item=0)
LAM, gs, G, n = D["LAM"].astype(np.float64), D["gs"], D["G"], D["n"]
wg = np.bincount(gs, minlength=G) / len(gs)
k = np.arange(1, n + 1)
logkappa = 0.5 * np.log(2) + gammaln((k + 1) / 2) - gammaln(k / 2) + 0.5 * np.log(2 * np.pi * np.e) - (0.5 * np.log(np.pi) - gammaln(k / 2 + 1) / k)
BY = np.array([0, 1, 2, 4, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256, 384]); BITS = 8 * BY
out = dict(proved=np.zeros(len(BY)), conj=np.zeros(len(BY)), rand=np.zeros(len(BY)), l1=0.0, l2=0.0, pr=0.0)
coords = np.zeros(n + 1)
for g in range(G):
    sg = np.sort(LAM[g])[::-1]; gam = sg * sg; lg = np.log(gam); cl = np.cumsum(lg) / (2 * k)        # log of (gamma_1..gamma_k)^(1/2k)
    l1 = sg.sum(); out["l1"] += wg[g] * l1; out["l2"] += wg[g] * np.sqrt(gam.sum()); out["pr"] += wg[g] * l1 * l1 / gam.sum()
    coords += wg[g] * np.concatenate([[l1], l1 - np.cumsum(sg)])
    for j, B in enumerate(BITS):
        out["proved"][j] += wg[g] * np.exp((logkappa + cl - B * np.log(2) / k).max())
        if B == 0: th = gam[0] * 2
        else:
            # reverse water-filling: sum_i 0.5 log2+(gamma_i / theta) = B
            lo, hi = np.log(gam[-1]) - 60, np.log(gam[0])
            for _ in range(80):
                mid = 0.5 * (lo + hi); r = 0.5 * np.maximum(0, lg - mid).sum() / np.log(2)
                if r > B: lo = mid
                else: hi = mid
            th = np.exp(0.5 * (lo + hi))
        out["conj"][j] += wg[g] * np.sqrt(np.minimum(gam, th)).sum(); out["rand"][j] += wg[g] * C * np.sqrt(np.minimum(gam, th).sum())
np.set_printoptions(precision=3, suppress=True, linewidth=200)
print(f"basis {BASIS}: zero-read l1 = sum sqrt(gamma) = {out['l1']:.4f}, l2 = {out['l2']:.4f}, participation ratio (l1/l2)^2 = {out['pr']:.0f}, c = {C}")
print("bytes                 ", BY)
for nm in ("proved", "conj", "rand"): print(f"{nm:8s} / zero-read l1 ", out[nm] / out["l1"])
def inv(y, x, R): return float(np.interp(-R, -y, x)) if y.min() <= R else float("nan")
print("bytes (or exact coordinates) at which the bound is R times the zero-read l1:")
for R in (0.9, 0.75, 0.6, 0.5, 0.4, 0.33, 0.25):
    print(f"  R={R:4.2f}: proved floor {inv(out['proved'] / out['l1'], BY, R):6.1f} B   conjectured floor {inv(out['conj'] / out['l1'], BY, R):6.1f} B   "
          f"ideal randomised {inv(out['rand'] / out['l1'], BY, R):6.1f} B   exact coordinates in order {inv(coords / out['l1'], np.arange(n + 1), R):6.1f}")
log("done")
