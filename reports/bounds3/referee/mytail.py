# Referee's own tail of X = sum_j a_j U_j (U_j a coordinate of the uniform law on the unit sphere of R^{d_j}).
# Method A (fast, double): lattice samples of the exponentially tilted densities, FFT convolution, Gregory end weights.
# Method B (mpmath, 40 digits): inversion integral on the line Re z = lam by the trapezoid rule, 0F1 by mpmath.
import numpy as np, math, mpmath as mp
from math import lgamma

def lnC(d):  # ln of the normaliser of (1 - s^2)^((d-3)/2) on [-1, 1]
    return lgamma(d / 2) - 0.5 * math.log(math.pi) - lgamma((d - 1) / 2)

def lntails_fft(a, ds, lam, eta=0.002, M=1 << 16):
    """returns (xs, ln P[X >= xs]) on lattice points xs = n eta, n = 0 .. M/2 - 8"""
    F = np.ones(M // 2 + 1, complex); lnM = 0.0
    for aj, d in zip(a, ds):
        if aj <= 0: continue
        kmax = int(math.floor(aj / eta))
        k = np.arange(-kmax, kmax + 1)
        s = k * eta / aj
        with np.errstate(divide="ignore"):
            lf = (d - 3) / 2 * np.log1p(-s * s)
        p = np.exp(lf); p /= p.sum()      # lattice samples, normalised (the sampling error is spectrally small unless a_j ~ eta)
        q = p * np.exp(lam * eta * k); m = q.sum(); lnM += math.log(m)
        arr = np.zeros(M); arr[k % M] = q / m
        F *= np.fft.rfft(arr)
    c = np.fft.irfft(F, M)[: M // 2]          # tilted pmf of the sum at n eta, n >= 0
    n = np.arange(M // 2)
    # S(n0) = sum_{n >= n0} w c(n) exp(-lam (n - n0) eta), Gregory weights at the lower end
    e = math.exp(-lam * eta)
    g = c * np.exp(-lam * eta * n)
    S = np.cumsum(g[::-1])[::-1] * np.exp(lam * eta * n)
    # Gregory correction (order 4): replace weights 1,1,1 at the first three nodes by 3/8, 7/6, 23/24
    cor = np.zeros(M // 2)
    cor[:-2] = (3 / 8 - 1) * c[:-2] + (7 / 6 - 1) * c[1:-1] * e + (23 / 24 - 1) * c[2:] * e * e
    S = S + cor
    with np.errstate(divide="ignore", invalid="ignore"):
        lt = np.log(S) + lnM - lam * n * eta
    return n * eta, lt

def quantiles_fft(w, ds, lnds, eta=0.002):
    a = [math.sqrt(wj * d) for wj, d in zip(w, ds)]
    out = []
    xs, lt = lntails_fft(a, ds, 8.3, eta)
    for lnd in lnds:
        i = int(np.argmax(lt < lnd))          # first lattice point with tail below delta
        # local cubic interpolation of x as a function of ln tail
        idx = np.arange(i - 2, i + 2)
        p = np.polyfit(lt[idx] - lnd, xs[idx], 3)
        out.append(float(p[-1]))
    return out

def lntail_mp(a, ds, x, lam=None, h=0.25, T=18, dps=40):
    mp.mp.dps = dps
    a = [mp.mpf(v) for v in a]; x = mp.mpf(x)
    sd = mp.sqrt(sum(v * v / d for v, d in zip(a, ds)))
    lam = mp.mpf(lam if lam is not None else x / sd ** 2)
    def K(z):
        return sum(mp.log(mp.hyp0f1(mp.mpf(d) / 2, (z * v) ** 2 / 4)) for v, d in zip(a, ds) if v > 0)
    K0 = K(lam)
    tot = mp.mpf(0)
    hh = mp.mpf(h) / sd
    N = int(T / h)
    for i in range(-N, N + 1):
        z = mp.mpc(lam, i * hh)
        # product of 0F1 directly (no branch trouble)
        pr = mp.mpf(1)
        for v, d in zip(a, ds):
            if v > 0: pr *= mp.hyp0f1(mp.mpf(d) / 2, (z * v) ** 2 / 4)
        tot += (pr * mp.exp(-z * x) / z)
    val = (tot * hh / (2 * mp.pi)).real
    return mp.log(val)
