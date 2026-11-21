"""F2: the constant for more, smaller blocks. Theorem 3.21 (R4, the closed form by convex order) evaluated for
today's four blocks at n = 768 (to reproduce chapter 3's 8.815) and n = 2048, and for the same blocks halved or cut
into sub-blocks of 64; with the exact one-coordinate sphere quantile in the predicted dimension sum(d_j + 2) - 2.
One-sided, N = 1e9 (ln(1/delta) = 42.83) and N = 2^20 (35.97).
"""
import numpy as np
import mpmath as mp
from scipy import optimize

mp.mp.dps = 40


def sphere_tail(D, t):
    """Pr[U >= t], U a coordinate of a uniform unit vector in R^D, 0 <= t < 1."""
    return mp.betainc((D - 1) / 2.0, 0.5, 0, 1 - t * t, regularized=True) / 2


def sphere_c(D, lnd):
    f = lambda c: float(mp.log(sphere_tail(D, c / np.sqrt(D)))) + lnd
    return optimize.brentq(f, 1.0, np.sqrt(D) * 0.999999)


def r4(ds, lnd):
    ds = np.array(ds, float)
    D = ds.sum()
    lognu = [float(mp.loggamma(d / 2 + 0.5) + mp.loggamma(D / 2) - mp.loggamma(d / 2) - mp.loggamma(D / 2 + 0.5)) for d in ds]
    theta = np.sqrt(max(d / (D * np.exp(2 * ln)) for d, ln in zip(ds, lognu)))
    s = theta * np.sqrt(D)  # s <= theta sqrt(D) sd, sd = 1
    logC = mp.loggamma(D / 2) - mp.log(mp.sqrt(mp.pi)) - mp.loggamma((D - 1) / 2)

    def mean_excess(tau):  # E (U - tau)_+
        return mp.exp(logC + (D - 1) / 2 * mp.log(1 - tau * tau)) / (D - 1) - tau * sphere_tail(D, tau)

    def log_bound(x):
        g = lambda tp: float(mp.log(s * mean_excess(tp / s) / (x - tp)))
        res = optimize.minimize_scalar(g, bounds=(x * 0.5, x - 1e-6), method="bounded", options={"xatol": 1e-7})
        return res.fun

    c = optimize.brentq(lambda x: log_bound(x) + lnd, 5.0, 12.0, xtol=1e-5)
    return c, theta


if __name__ == "__main__":
    layouts = {
        "n = 768, four blocks (95, 127, 255, 287)": [95, 127, 255, 287],
        "n = 2048, four blocks (255, 335, 687, 767)": [255, 335, 687, 767],
        "n = 2048, each block halved (8 sub-blocks)": [127, 127, 167, 167, 343, 343, 383, 383],
        "n = 2048, 32 sub-blocks of 64": [63] * 32,
        "n = 2048, 128 sub-blocks of 16": [15] * 128,
    }
    for label, lnd in (("N = 1e9", np.log(4e18)), ("N = 2^20", 35.97)):
        print(label)
        for name, ds in layouts.items():
            c, theta = r4(ds, lnd)
            Dp = sum(d + 2 for d in ds) - 2
            print(f"  {name}: R4 = {c:.3f} (theta = {theta:.5f}); sphere in sum(d+2)-2 = {Dp} dimensions: {sphere_c(Dp, lnd):.3f}")
