# Referee check of Proposition A(b): cell by cell, E_g h(c, .) >= Phi(ln 1/P_c).
# The cheapest cells for their probability sit at the origin, so those are the ones to try.
#   (1) centred balls, isotropic spectrum, every radius, n = 1 .. 10^4 (exact through the chi law)
#   (2) centred boxes with random half-sides, random spectra, n <= 6 (exact)
#   (3) centred ellipses in the plane, spectrum (1, eps^2) (quadrature)
#   (4) the same cells against W(ln 1/P_c): the author says (b) has no analogue for W; the small disc shows it
import math
import numpy as np
from scipy.special import gammainc, ndtr
from core_f2_3d import log_kappa, Wf
from core_f2_misc import Phi      # noqa: runs that module's checks on import, output discarded by the caller

rng = np.random.default_rng(2)

# (1)
worst1 = (np.inf, None)
worstW = (np.inf, None)
for n in (1, 2, 3, 5, 10, 30, 100, 1000, 10000):
    Eg = math.exp(0.5 * math.log(2) + math.lgamma((n + 1) / 2) - math.lgamma(n / 2))
    a = np.ones(n)
    for rho in np.concatenate([np.linspace(0.02, 1, 25) * math.sqrt(n), math.sqrt(n) + np.linspace(-3, 4, 29)]):
        if rho <= 0:
            continue
        P = float(gammainc(n / 2.0, rho * rho / 2.0))
        if not (1e-280 < P < 1):
            continue
        H = -math.log(P)
        r = Eg * rho / Phi(a, H)
        rW = Eg * rho / (n * math.exp(-H / n))
        if r < worst1[0]:
            worst1 = (r, (n, round(float(rho), 3), P))
        if rW < worstW[0]:
            worstW = (rW, (n, round(float(rho), 3), P))
print("(1) centred balls: smallest E_g h / Phi(ln 1/P) = %.4f at (n, rho, P) = %s" % worst1)
print("(4) centred balls: smallest E_g h / W(ln 1/P) = %.4f at (n, rho, P) = %s   [below 1: W fails cell by cell]"
      % worstW)
n = 2
print("    small disc at the origin, n = 2: limit kappa_2 / (2 sqrt e) = %.4f"
      % (math.exp(log_kappa(2)) / (2 * math.sqrt(math.e))))

# (2)
worst2 = np.inf
for t in range(4000):
    n = int(rng.integers(1, 7))
    a = np.exp(rng.normal(0, 1.5, n))
    b = np.exp(rng.normal(-0.5, 1.2, n))
    P = float(np.prod(2 * ndtr(b) - 1))
    if P < 1e-250:
        continue
    Eh = math.sqrt(2 / math.pi) * float((a * b).sum())
    worst2 = min(worst2, Eh / Phi(a, -math.log(P)))
print("(2) centred boxes, 4,000 random: smallest E_g h / Phi(ln 1/P) = %.4f" % worst2)

# (3)
xs, ws = np.polynomial.legendre.leggauss(400)
th = np.linspace(0, 2 * math.pi, 4001)[:-1]
rr, wr = np.polynomial.laguerre.laggauss(60)       # for the radial part of a planar Gaussian: r^2/2 = t
worst3 = np.inf
for eps in (1.0, 0.3, 0.05):
    a = np.array([1.0, eps])
    for b1 in (0.05, 0.2, 0.6, 1.5, 3.0):
        for b2 in (0.05, 0.2, 0.6, 1.5, 3.0):
            x = b1 * xs
            P = float((b1 * ws * np.exp(-x * x / 2) / math.sqrt(2 * math.pi)
                       * (2 * ndtr(b2 * np.sqrt(np.clip(1 - xs * xs, 0, 1))) - 1)).sum())
            # E sqrt(b1^2 g1^2 + eps^2 b2^2 g2^2) = E|g_2| * mean over angle of sqrt(...)
            ang = np.sqrt((b1 * np.cos(th)) ** 2 + (eps * b2 * np.sin(th)) ** 2).mean()
            Eh = math.sqrt(math.pi / 2) * ang
            worst3 = min(worst3, Eh / Phi(a, -math.log(P)))
print("(3) centred ellipses, 75 cases: smallest E_g h / Phi(ln 1/P) = %.4f" % worst3)
