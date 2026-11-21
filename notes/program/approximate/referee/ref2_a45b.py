"""Referee 2, A4.5 item 3, third case: the law of the score given a high midpoint in Model C.
Claim checked: s - mu given a is normal about (a - mu)/(1 - k) - k Var(aG) lam/(1 - k)^2 with spread s_res/(1 - k).
Centred as the author's script centres (R - b). Synthetic."""
import math
import numpy as np
rng = np.random.default_rng(9)
for f, kap in ((0.92, 0.36), (0.82, 0.39), (0.71, 0.50)):
    b = 1 / 2.2; vG = 1 - b * b; omf = ((1 - f) - kap * kap * b * b) / vG
    va, ve = (1 - omf) * vG, omf * vG; sres = math.sqrt((1 - kap) ** 2 * ve + kap ** 2 * va)
    W = []
    for _ in range(10):
        R = rng.exponential(b, 2_000_000)
        aG = rng.normal(0, math.sqrt(va), R.size); eG = rng.normal(0, math.sqrt(ve), R.size)
        a = (1 - kap) * R + aG; s = R + aG + eG
        hi = a > np.quantile(a, 0.999)
        W.append(s[hi] - a[hi] / (1 - kap))
    W = np.concatenate(W)
    print(f"f {f} kappa {kap}: mean of s - a/(1-k) among the top 0.1% by midpoint {W.mean():+.3f} (derived {-kap * va / b / (1 - kap) ** 2:+.3f}), spread {W.std():.3f} (derived {sres / (1 - kap):.3f}); "
          f"constant of the efficient rule {-kap * va / b / (1 - kap) / sres:+.2f} s_res, of the stable dial {((1 - kap) * ve - kap * va) / b / sres:+.2f} s_res, difference {sres / b / (1 - kap):.2f}")
