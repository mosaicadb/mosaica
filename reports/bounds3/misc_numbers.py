# Small numbers quoted in the report.
import math, numpy as np
from law import c1, tail, tail1
from ctable import quantiles
LN = math.log(1e-9 / 1e9); lnd = LN - math.log(4)
ds = [95, 127, 255, 287]
print("single block d = 95 by share of eps/N: " + ", ".join(f"{p}: {c1(95, LN + math.log(p)):.3f}" for p in (1.0, 0.7, 0.25, 0.1, 1 / 30)))
print("four blocks, exact worst case by share of eps/N (argmax fixed at (d+2)/sum): " + ", ".join(
    f"{p}: {quantiles(np.array([[(d + 2) / 772 for d in ds]]), ds, [LN + math.log(p)])[0, 0]:.4f}" for p in (1.0, 0.7, 0.25, 0.1, 1 / 30)))
# L = 5 (the n = 1536 schedule in fit.py has five levels) against L = 4
for n_, dd in ((768, ds), (1536, [191, 255, 511, 575])):
    S = sum(d + 2 for d in dd); w = np.array([[(d + 2) / S for d in dd]])
    print(f"n {n_}: four blocks at L = 4: {quantiles(w, dd, [LN - math.log(4)])[0,0]:.4f}; L = 5: {quantiles(w, dd, [LN - math.log(5)])[0,0]:.4f}; L = 3: {quantiles(w, dd, [LN - math.log(3)])[0,0]:.4f}")
# overlap of two levels' failure events for one item: correlation rho between the errors
for rho in (0.45, 0.47, 0.6, 0.8, 0.9):
    f = math.sqrt(2 / (1 + rho))
    s = c1(95, lnd) / math.sqrt(95)
    one = math.log(tail1(95, min(s * f, 0.999999))) - lnd if s * f < 1 else -np.inf
    wst = [(d + 2) / 772 for d in ds]; a = [math.sqrt(x * d) for x, d in zip(wst, ds)]
    c = quantiles(np.array([wst]), ds, [lnd])[0, 0]
    four = tail(a, ds, c * f)[1] - lnd
    print(f"rho {rho}: P[both fail] / delta <= exp({one:.1f}) (one block, d = 95), exp({four:.1f}) (four blocks, worst shares)")
# block-wise bounds added (eps split over the blocks), at the worst-case shares
cj = [c1(d, lnd - math.log(4)) for d in ds]
print("adding block-wise bounds at delta/4 each, worst-case shares:", round(sum(c_ * math.sqrt(x) for c_, x in zip(cj, wst)), 3), "sd against", round(c, 3))
# two and three blocks against the sphere at D'
for sub in ([95, 127], [95, 127, 255]):
    S = sum(d + 2 for d in sub)
    print(sub, "sphere at D' =", S - 2, ":", round(c1(S - 2, lnd), 4))
# Gaussian quantile at the same delta, for reference
from scipy.special import ndtri_exp
print("Gaussian quantile at delta (one-sided, N = 1e9, L = 4):", -ndtri_exp(lnd), "; sub-Gaussian bound:", math.sqrt(-2 * lnd))
