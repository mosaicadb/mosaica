"""Lemma C2 step (c): the author's bound against the referee's refinement, at the index's sizes."""
import numpy as np
from scipy.special import betainc, betaincinv, gammaln
from scipy.optimize import brentq
from scipy.integrate import quad
def lnT(d, x):
    """ln Pr[U>=x], U coordinate of uniform unit vector in R^d, 0<=x<1, in log space"""
    v = 0.5 * betainc((d - 1) / 2, 0.5, 1 - x * x)
    if v > 1e-280: return np.log(v)
    lk = gammaln(d / 2) - gammaln((d - 1) / 2) - 0.5 * np.log(np.pi)
    f = lambda t: np.exp((d - 3) / 2 * (np.log1p(-t * t) - np.log1p(-x * x)))
    val, _ = quad(f, x, 1, epsabs=0, epsrel=1e-9)
    return lk + (d - 3) / 2 * np.log1p(-x * x) + np.log(val)
def qd(d, delta): return float(np.sqrt(1 - betaincinv((d - 1) / 2, 0.5, 2 * delta)))
def lncap(d, th): return lnT(d + 1, np.cos(th))      # Omega_d(theta) = T_(d+1)(cos theta)
delta, N = 2.5e-19, 1e9
Lam = np.log(2 * N / (0.01 * delta))
for d in (95, 127, 255, 287):
    q = qd(d, delta)
    lk = gammaln((d + 1) / 2) - gammaln(d / 2) - 0.5 * np.log(np.pi)
    print(f"d={d} q={q:.4f} Lambda={Lam:.2f}:  bits | sin(theta) | author's third term | refined third term | ratio")
    for B in range(26, 44):
        lnM = B * np.log(2)
        f = lambda th: lncap(d, th) + lnM - np.log(Lam)
        if f(np.pi / 2 - 1e-6) < 0: print(f"   {B}: cap of measure Lambda/M does not exist"); continue
        th = brentq(f, 1e-6, np.pi / 2 - 1e-6, xtol=1e-13)
        auth = np.log(N) + np.log(Lam) + lncap(d - 1, th)                      # N Lam Omega_(d-1)(theta)
        g = lambda ph: lnT(d, np.cos(th) / np.sqrt(1 - q * q * np.sin(ph) ** 2)) + (d - 1) * np.log(np.sin(ph))
        phs = np.linspace(th * 1e-3, th, 8001); gs = np.array([g(p) for p in phs]); gm = gs.max()
        integ = np.trapezoid(np.exp(gs - gm), phs)
        ref = np.log(N) + lnM + lk + gm + np.log(integ)                        # N M int kappa sin^(d-1) T_d(...)
        print(f"   {B:3d} | {np.sin(th):.4f} | {np.exp(auth):.3e} | {np.exp(ref):.3e} | {np.exp(ref-auth):.2e}")
