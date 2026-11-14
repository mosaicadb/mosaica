# Exact law of X = sum_j a_j U_j, U_j a coordinate of a uniform unit vector in R^{d_j}, independent across blocks.
# Tools (floating point, not interval arithmetic):
#   tail1(d, s)         exact one-sided tail of one coordinate, P[U >= s] = I_{1-s^2}((d-1)/2, 1/2) / 2
#   c1(d, lnd)          s* sqrt(d) with P[U >= s*] = exp(lnd)
#   lnmgf(d, z)         ln E exp(z U) = ln 0F1(; d/2; z^2/4), real or complex z
#   tail(a, ds, x)      P[X >= x] by the inversion integral on the vertical line through the saddlepoint
#   quant(a, ds, lnd)   x with P[X >= x] = exp(lnd)
import numpy as np, math
from scipy.special import betainc, betaincinv, ive, gammaln, log_ndtr, ndtri_exp
import mpmath as mp

def tail1(d, s):
    return 0.5 * betainc((d - 1) / 2, 0.5, 1 - s * s)

def c1(d, lnd):
    """c = s* sqrt(d), P[U >= s*] = exp(lnd), U a coordinate of the uniform law on S^{d-1} (d may be non-integer)."""
    # betaincinv loses accuracy for tiny arguments in some scipy versions; use mpmath bisection on log tail
    d = mp.mpf(d); target = mp.mpf(lnd)
    def lt(s):
        return mp.log(mp.betainc((d - 1) / 2, mp.mpf(1) / 2, 0, 1 - s * s, regularized=True) / 2)
    lo, hi = mp.mpf('1e-6'), mp.mpf(1) - mp.mpf('1e-12')
    for _ in range(80):
        mid = (lo + hi) / 2
        if lt(mid) > target: lo = mid
        else: hi = mid
    return float(hi * mp.sqrt(d))

def _series(b, y, terms=80):
    """0F1(; b; y) by its power series (used where |y| / b is small, so there is no cancellation)."""
    y = np.asarray(y)
    s = np.ones_like(y); t = np.ones_like(y)
    for k in range(1, terms):
        t = t * y / ((b + k - 1) * k)
        s = s + t
    return s

def lnmgf(d, z):
    """ln E exp(z U) = ln 0F1(; d/2; z^2/4): real z >= 0 (array) or complex z (array). Series for small |z|,
    the scaled Bessel function otherwise."""
    nu = d / 2 - 1
    z = np.atleast_1d(np.asarray(z))
    cplx = np.iscomplexobj(z)
    out = np.zeros(z.shape, dtype=complex if cplx else float)
    small = np.abs(z) < max(8.0, 0.12 * nu)
    if small.any():
        out[small] = np.log(_series(nu + 1, z[small] ** 2 / 4))
    big = ~small
    if big.any():
        zb = z[big]
        if cplx:
            out[big] = gammaln(nu + 1) - nu * np.log(zb / 2) + np.log(ive(nu, zb)) + np.abs(zb.real)
        else:
            out[big] = gammaln(nu + 1) - nu * np.log(zb / 2) + np.log(ive(nu, zb)) + zb
    return out if out.size > 1 else out[0]

def dlnmgf(d, z):
    """psi'(z) = I_{nu+1}(z) / I_nu(z) = (z / d) 0F1(; d/2 + 1; z^2/4) / 0F1(; d/2; z^2/4), real z >= 0."""
    nu = d / 2 - 1
    z = np.atleast_1d(np.asarray(z, dtype=float))
    out = np.zeros(z.shape)
    small = z < max(8.0, 0.12 * nu)
    if small.any():
        y = z[small] ** 2 / 4
        out[small] = z[small] / (2 * (nu + 1)) * _series(nu + 2, y) / _series(nu + 1, y)
    big = ~small
    if big.any():
        out[big] = ive(nu + 1, z[big]) / ive(nu, z[big])
    return out if out.size > 1 else out[0]

def d2lnmgf(d, z):
    """psi''(z) = 1 - (d - 1) r / z - r^2 with r = psi'(z) (from the Bessel recurrence); 1/d at z = 0."""
    z = np.atleast_1d(np.asarray(z, dtype=float))
    r = np.atleast_1d(dlnmgf(d, z))
    out = np.where(z > 1e-6, 1 - (d - 1) * r / np.maximum(z, 1e-300) - r * r, 1.0 / d)
    return out if out.size > 1 else out[0]

def K(a, ds, lam):
    return sum(lnmgf(d, lam * aj) for aj, d in zip(a, ds) if aj > 0)
def K1(a, ds, lam):
    return sum(aj * dlnmgf(d, lam * aj) for aj, d in zip(a, ds) if aj > 0)
def K2(a, ds, lam):
    return sum(aj * aj * d2lnmgf(d, lam * aj) for aj, d in zip(a, ds) if aj > 0)

def saddle(a, ds, x):
    """lambda with K'(lambda) = x (x below the support's end sum a_j)."""
    lo, hi = 0.0, 1.0
    while K1(a, ds, hi) < x:
        hi *= 2
        if hi > 1e9: raise ValueError("x beyond the support")
    for _ in range(200):
        mid = (lo + hi) / 2
        if K1(a, ds, mid) < x: lo = mid
        else: hi = mid
    return (lo + hi) / 2

def tail(a, ds, x, lam=None, npts=4001, width=14.0):
    """P[X >= x] = (1 / 2 pi) int exp(K(lam + i t) - (lam + i t) x) / (lam + i t) dt; returns (tail, ln tail)."""
    a = [float(v) for v in a]
    if lam is None:
        lam = saddle(a, ds, x)
    v = K2(a, ds, lam)
    T = width / math.sqrt(v)
    t = np.linspace(-T, T, npts)
    z = lam + 1j * t
    Kz = sum(lnmgf(d, z * aj) for aj, d in zip(a, ds) if aj > 0)
    K0 = K(a, ds, lam)
    g = np.exp(Kz - K0 - 1j * t * x) / z
    integral = (g.sum() - 0.5 * (g[0] + g[-1])) * (t[1] - t[0])
    val = integral.real / (2 * math.pi)
    return val * math.exp(K0 - lam * x) if K0 - lam * x > -700 else 0.0, math.log(val) + K0 - lam * x

def quant(a, ds, lnd, x0=None):
    """x with ln P[X >= x] = lnd (secant on ln tail)."""
    sig = math.sqrt(sum(aj * aj / d for aj, d in zip(a, ds)))
    xa = (x0 or math.sqrt(-2 * lnd) * 0.93) * sig
    xb = xa * 1.02
    fa = tail(a, ds, xa)[1] - lnd; fb = tail(a, ds, xb)[1] - lnd
    for _ in range(60):
        if abs(fb) < 1e-10: break
        xn = xb - fb * (xb - xa) / (fb - fa)
        xa, fa = xb, fb
        xb = xn; fb = tail(a, ds, xb)[1] - lnd
    return xb

def cw(w, ds, lnd):
    """c(w): the exp(lnd) quantile of X in units of its sd, for variance shares w."""
    a = [math.sqrt(wj * d) for wj, d in zip(w, ds)]
    return quant(a, ds, lnd)

if __name__ == "__main__":
    L1 = math.log(2 * 1e9 * 4 / 1e-9)
    print("L1", L1, "c subG", math.sqrt(2 * L1))
    for d in (95, 127, 255, 287, 764, 767):
        ce = c1(d, -L1)
        a = [math.sqrt(d)]
        print(d, "exact (beta)", round(ce, 5), "inversion", round(quant(a, [d], -L1), 5))
    ds = [95, 127, 255, 287]
    for w in ([0.25] * 4, [95 / 764, 127 / 764, 255 / 764, 287 / 764], [0.336, 0.398, 0.26, 0.006], [0.068, 0.149, 0.64, 0.143]):
        print(np.round(w, 3), "c(w)", round(cw(w, ds, -L1), 5))
