# Overlap of the first two randomised levels' failure events for one item (n = 768, one-sided, N = 1e9).
# Level 1 codes block 0 only; level 2 codes all four with block 0 unchanged, so error_2 = error_1 + (blocks 1..3).
# P[error_1 >= h_1 and error_2 >= h_2] = int_{h_1}^{a_0} f_0(e) P[others >= h_2 - e] de, against delta.
import numpy as np, math
from scipy.special import gammaln
from law import tail, c1
lnd = math.log(1e-9 / 1e9) - math.log(4)
ds = [95, 127, 255, 287]; s1 = c1(95, lnd) / math.sqrt(95)
xs, ws = np.polynomial.legendre.leggauss(120)
for w in ([0.22, 0.42, 0.34, 0.02], [0.34, 0.40, 0.25, 0.01], [0.55, 0.25, 0.18, 0.02], [0.126, 0.167, 0.333, 0.374]):
    a = [math.sqrt(x * d) for x, d in zip(w, ds)]          # sd of level 2 = 1
    h1 = s1 * a[0]; h2 = 8.704
    lo, hi = h1, a[0]
    e = (hi - lo) / 2 * xs + (hi + lo) / 2
    lf = gammaln(95 / 2) - 0.5 * math.log(math.pi) - gammaln(94 / 2) + 46 * np.log1p(-(e / a[0]) ** 2) - math.log(a[0])
    tot = 0.0
    for ei, wi, lfi in zip(e, ws, lf):
        y = h2 - ei
        lt = 0.0 if y <= 0 else tail(a[1:], ds[1:], y)[1]
        tot += wi * math.exp(lfi + lt - lnd)
    tot *= (hi - lo) / 2
    print(f"shares at the second level {w}: P[both fail] / delta = {tot:.2e}")
