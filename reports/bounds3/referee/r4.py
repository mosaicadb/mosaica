import math, mpmath as mp, numpy as np
from mytail import *
mp.mp.dps = 40
ds = [95, 127, 255, 287]
lnds = {"N1e9 two": -math.log(8e18), "N1e9 one": -math.log(4e18), "N1e6 two": -math.log(8e15), "N1e6 one": -math.log(4e15)}
# corner check of the certificate
g = [0.12549, 0.16748, 0.3335, 0.375]; g = [round(x * 2048) / 2048 for x in g]
a = [math.sqrt(x * d) for x, d in zip(g, ds)]
print("corner", g, sum(g), "exact ln T(8.704) by mp:", mp.nstr(lntail_mp(a, ds, 8.704), 10), "(certificate's lattice bound -42.83289)")
# single block and single sphere quantiles (bisection on the beta tail)
def c1(d, lnd):
    d = mp.mpf(d)
    f = lambda s: mp.log(mp.betainc((d - 1) / 2, mp.mpf(1) / 2, 0, 1 - s * s, regularized=True) / 2) - lnd
    return mp.findroot(f, (mp.mpf('0.2'), mp.mpf('0.9')), solver='anderson') * mp.sqrt(d)
for name, lnd in lnds.items():
    print(name, "one block d=95:", mp.nstr(c1(95, lnd), 6), " sphere d=767:", mp.nstr(c1(767, lnd), 7), " sphere d=770:", mp.nstr(c1(770, lnd), 7), " sphere d=764:", mp.nstr(c1(764, lnd), 7))
# R4
D = sum(ds)
m = [mp.exp(mp.loggamma(mp.mpf(d) / 2 + mp.mpf(1) / 2) + mp.loggamma(mp.mpf(D) / 2) - mp.loggamma(mp.mpf(d) / 2) - mp.loggamma(mp.mpf(D) / 2 + mp.mpf(1) / 2)) for d in ds]
# check m_j by quadrature of sqrt(Beta)
d0 = ds[0]
mq = mp.quad(lambda x: mp.sqrt(x) * x ** (mp.mpf(d0) / 2 - 1) * (1 - x) ** (mp.mpf(D - d0) / 2 - 1), mp.linspace(0, 1, 41)) / mp.beta(mp.mpf(d0) / 2, mp.mpf(D - d0) / 2)
print("m_0 formula", mp.nstr(m[0], 12), "quadrature", mp.nstr(mq, 12))
theta2 = max(mp.mpf(d) / (D * mj ** 2) for d, mj in zip(ds, m)); theta = mp.sqrt(theta2)
print("theta", mp.nstr(theta, 8), "theta^2", mp.nstr(theta2, 8))
lC = mp.loggamma(mp.mpf(D) / 2) - mp.log(mp.pi) / 2 - mp.loggamma(mp.mpf(D - 1) / 2)
def sl(t):  # E (U_D - t)_+
    return mp.exp(lC + mp.mpf(D - 1) / 2 * mp.log(1 - t * t)) / (D - 1) - t * mp.betainc(mp.mpf(D - 1) / 2, mp.mpf(1) / 2, 0, 1 - t * t, regularized=True) / 2
# check sl by quadrature
t0 = mp.mpf('0.2')
print("stop-loss formula", mp.nstr(sl(t0), 12), "quadrature", mp.nstr(mp.quad(lambda u: (u - t0) * mp.exp(lC + mp.mpf(D - 3) / 2 * mp.log(1 - u * u)), mp.linspace(t0, 1, 30)), 12))
S = theta * mp.sqrt(D)     # s / sd at its largest
def bound(c):   # ln inf_t E (S U - t)_+ / (c - t)
    f = lambda t: mp.log(S * sl(t / S)) - mp.log(c - t)
    # minimise over t by golden section on [c - 1, c - 1e-3]
    lo, hi = c - mp.mpf(1), c - mp.mpf('0.001'); gr = (mp.sqrt(5) - 1) / 2
    for _ in range(80):
        x1 = hi - gr * (hi - lo); x2 = lo + gr * (hi - lo)
        if f(x1) < f(x2): hi = x2
        else: lo = x1
    return f((lo + hi) / 2)
for name, lnd in lnds.items():
    c = mp.findroot(lambda c: bound(c) - lnd, (mp.mpf(8), mp.mpf(9)), solver='secant', tol=1e-12)
    print(name, "R4 constant", mp.nstr(c, 6))
# direct numerical check of the convex-order inequality at the worst shares and at a face: E (X - t)_+ <= E (s U_D - t)_+
for w in ([0.1254, 0.167, 0.333, 0.3746], [0.97, 0.01, 0.01, 0.01], [0.0, 0.0, 0.0, 1.0], [0.5, 0.5, 0, 0]):
    a = [math.sqrt(x * d) for x, d in zip(w, ds)]
    s = mp.sqrt(sum(mp.mpf(v) ** 2 / mj ** 2 for v, mj in zip(a, m)))
    xs, lt = lntails_fft(a, ds, 8.3, eta=0.002)
    for t in (2.0, 5.0, 8.0):
        i = int(round(t / 0.002)); T = np.exp(lt[i:i + 4000])       # tail on [t, t + 8]
        lhs = (T.sum() - 0.5 * T[0]) * 0.002
        rhs = s * sl(mp.mpf(t) / s)
        print(np.round(w, 3).tolist(), "t", t, "ln E(X-t)+ - ln E(sU_D-t)+ =", float(mp.log(lhs) - mp.log(rhs)))
