# log P[ sum_i lam_i (z_i + d_i)^2 <= x ] for standard Gaussian z, by inverting the Laplace transform along the vertical line
# through the saddle point (relative accuracy in the far lower tail, where Imhof's formula loses everything).
#   F(x) = (1/pi) int_0^inf Re[ L(c+iy) e^{(c+iy)x} / (c+iy) ] dy,  L(t) = prod (1 + 2 lam t)^{-1/2} exp(-lam d^2 t/(1 + 2 lam t)).
import math
import numpy as np
from scipy import integrate, optimize


def _logL(t, lam, d2):
    q = 1 + 2 * lam * t
    return (-0.5 * np.log(q) - lam * d2 * t / q).sum()


def log_cdf(x, lam, d=None):
    lam = np.asarray(lam, float)
    d2 = np.zeros_like(lam) if d is None else np.asarray(d, float) ** 2
    def psi(c):
        return float(np.real(_logL(c, lam, d2))) + c * x - math.log(c)
    # saddle on c > 0 (psi -> inf at both ends)
    lo, hi = -40.0, 40.0
    res = optimize.minimize_scalar(lambda u: psi(math.exp(u)), bounds=(lo, hi), method="bounded", options={"xatol": 1e-12})
    c = math.exp(res.x)
    base = psi(c)
    # curvature for the y scale
    h = 1e-4 * c
    d2psi = (psi(c + h) - 2 * psi(c) + psi(c - h)) / (h * h)
    sy = 1 / math.sqrt(max(d2psi, 1e-300))
    def f(y):
        t = c + 1j * y
        v = _logL(t, lam, d2) + t * x - np.log(t) - base
        return float(np.real(np.exp(v)))
    tot, err = 0.0, 0.0
    edges = [0.0] + [sy * k for k in (0.5, 1, 2, 4, 8, 16, 32, 64, 128, 256, 1024, 4096, 16384, 65536, 1e6)]
    for a, b in zip(edges[:-1], edges[1:]):
        v, e = integrate.quad(f, a, b, limit=400, epsabs=1e-16, epsrel=1e-11)
        tot += v; err += e
    v, e = integrate.quad(f, edges[-1], np.inf, limit=400, epsabs=1e-16, epsrel=1e-10)
    tot += v; err += e
    if tot <= 0:
        return -np.inf, np.inf
    return base + math.log(tot / math.pi), err / tot


if __name__ == "__main__":
    from scipy import stats
    for n, x in [(2, 0.01), (5, 0.5), (20, 3), (20, 20), (100, 10), (100, 60), (100, 98), (100, 150), (3, 1e-6)]:
        lc, rel = log_cdf(x, np.ones(n))
        print(n, x, lc, stats.chi2.logcdf(x, n), rel)
    for n, x, nc in [(5, 2.0, 4.0), (20, 5.0, 9.0), (100, 50, 25.0)]:
        dd = np.zeros(n); dd[0] = math.sqrt(nc)
        print("ncx2", n, x, nc, log_cdf(x, np.ones(n), dd)[0], stats.ncx2.logcdf(x, n, nc))
    rng = np.random.default_rng(1)
    lam = rng.exponential(size=6); z = rng.standard_normal((2_000_000, 6)); q = (z * z * lam).sum(1)
    for x in [0.5, 2.0, 6.0]:
        p = (q <= x).mean(); print("MC", x, math.log(p), "+-", math.sqrt(p * (1 - p) / len(q)) / p, log_cdf(x, lam)[0])
