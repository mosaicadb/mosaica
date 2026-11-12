# kappa_k >= k; exact cap tail vs 2(1-s^2)^(m/2); sharper c table; 0F1 mgf sub-Gaussian check.
import numpy as np, math
from scipy import special, stats, optimize
def kappa(k):
    Eg = math.sqrt(2) * math.exp(math.lgamma((k + 1) / 2) - math.lgamma(k / 2))
    logvol = k / 2 * math.log(math.pi) - math.lgamma(k / 2 + 1)
    return Eg * math.sqrt(2 * math.pi * math.e) / math.exp(logvol / k)
r = np.array([kappa(k) / k for k in range(1, 200001)])
print("kappa_k/k: min over k<=200000 = %.6f at k=%d; k=1 %.4f k=2 %.4f k=10 %.4f k=100 %.5f k=1e4 %.6f k=2e5 %.7f" % (r.min(), r.argmin() + 1, r[0], r[1], r[9], r[99], r[9999], r[-1]))
print("  asymptotic (pi k)^(1/2k)(1-1/4k) at k=100: %.5f" % ((math.pi * 100) ** (1 / 200) * (1 - 1 / 400)))
print("  monotone decreasing:", bool(np.all(np.diff(r) < 0)))
# exact tail: P(|u1|>=s) = I_{1-s^2}((m-1)/2, 1/2)
def tail(m, s): return special.betainc((m - 1) / 2, 0.5, 1 - s * s)
worst = 0; wm = None
for m in list(range(2, 400)) + [500, 767, 1000, 2000, 5000]:
    s = np.linspace(1e-4, 1 / math.sqrt(2), 4000)
    q = tail(m, s) / (2 * (1 - s * s) ** (m / 2))
    if q.max() > worst: worst, wm = q.max(), (m, s[q.argmax()])
print("max exact/[2(1-s^2)^(m/2)] over m>=2, s<=1/sqrt2: %.5f at (m,s)=%s" % (worst, wm))
for m in (2, 3, 5, 10, 30):
    s = np.linspace(0.708, 0.99999, 20000); q = tail(m, s) / (2 * (1 - s * s) ** (m / 2))
    print("  m=%d beyond 1/sqrt2: max ratio %.3f (first s with ratio>1: %s)" % (m, q.max(), s[q > 1][0] if (q > 1).any() else None))
# two-sided without the factor 2?  P(|u1|>=s) <= (1-s^2)^(m/2)?
print("sharper-c table c'/c (c' = s*sqrt(m), tail(s)=2exp(-c^2/2)):")
for m in (23, 30, 767):
    out = []
    for c in (6., 8., 10.):
        lp = math.log(2) - c * c / 2
        f = lambda s: math.log(max(tail(m, s), 1e-320)) - lp
        # use log-space via betainc is fine down to 1e-300
        s = optimize.brentq(f, 1e-6, 1 - 1e-9); out.append(s * math.sqrt(m) / c)
    print("  m=%d: %s" % (m, " ".join("%.3f" % v for v in out)))
# mgf: 0F1(;m/2;l^2/4) <= exp(l^2/(2m)) and Monte Carlo
rng = np.random.default_rng(0)
for m in (5, 30):
    g = rng.standard_normal((400000, m)); u = g[:, 0] / np.linalg.norm(g, axis=1)
    for l in (2., 6.):
        ex = special.hyp0f1(m / 2, l * l / 4)
        print("  m=%d lam=%.0f: MC mgf %.4f, 0F1 %.4f, subG bound %.4f" % (m, l, np.exp(l * u).mean(), ex, math.exp(l * l / (2 * m))))
