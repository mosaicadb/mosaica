"""C2 after the referee: the refined step (c) of Lemma C2 (the referee's, checked here).

Refined:  Pr[G_1(c) and angle(c, z_2) <= th] <= delta * I(th),
          I(th) = int_0^th kappa_d sin^(d-1)(phi) T_d(cos th / sqrt(1 - q^2 sin^2 phi)) dphi,
against the first version  delta * Omega_d(th) * Omega_(d-1)(th).
Part 1  the last term of gamma = N pi_2/delta with each version, by d and bits; least bits for 0.01.
Part 2  Monte Carlo of the event for one uniform codeword at small d, against both bounds.
Run: nice -n 10 taskset -c 5 env OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 c2_revision.py
"""
import numpy as np
from scipy.special import gammaln, betainc
from scipy.optimize import brentq
from c1_converse import T, q, cap


def lnkappa(d):
    return gammaln((d + 1) / 2) - gammaln(d / 2) - 0.5 * np.log(np.pi)


def lncap(d, th):
    c = cap(d, th)
    return np.log(c) if c > 1e-280 else lnkappa(d) + d * np.log(np.sin(th)) - np.log(d)


def lnT(d, x):
    """log T_d(x), x in (0,1); an upper bound where betainc underflows (safe direction for an upper bound)."""
    t = float(T(d, x))
    if t > 1e-290:
        return np.log(t)
    return gammaln(d / 2) - gammaln((d - 1) / 2) - 0.5 * np.log(np.pi) + ((d - 1) / 2) * np.log1p(-x * x) - np.log(x * (d - 1))


def lnI(d, th, qd, npts=4001):
    ph = np.linspace(th * 1e-6, th, npts)
    x = np.cos(th) / np.sqrt(1 - qd * qd * np.sin(ph) ** 2)
    lg = np.array([lnkappa(d) + (d - 1) * np.log(np.sin(p)) + lnT(d, xx) for p, xx in zip(ph, x)])
    m = lg.max()
    w = np.exp(lg - m)
    # trapezoid plus one cell: the integrand increases in phi, so add the last cell again as a margin
    return m + np.log(np.sum(0.5 * (w[1:] + w[:-1]) * np.diff(ph)) + w[-1] * (ph[1] - ph[0]))


def terms(d, bits, N, delta, g1, qd):
    lnM = bits * np.log(2)
    Lam = np.log(2 * N / (g1 * delta))
    f = lambda th: lncap(d, th) + lnM - np.log(Lam)
    if f(np.pi / 2 - 1e-9) < 0:
        return None
    th = brentq(f, 1e-9, np.pi / 2 - 1e-9)
    first = np.exp(np.log(N) + np.log(Lam) + lncap(d - 1, th))          # N * Lam * Omega_(d-1)
    refined = np.exp(np.log(N) + lnM + lnI(d, th, qd))                   # N * M * I
    return th, first, refined


def part1():
    N, delta, g1 = 1e9, 2.5e-19, 0.01
    print(f"PART 1  N={N:.0e} delta={delta} gamma_1={g1}; last term of gamma with the first and the refined step (c)")
    print("  d     bits   M/N        theta(deg)  first version  refined")
    for d in [95, 127, 255, 287]:
        qd = q(d, delta)
        for bits in [24, 28, 29, 30, 32, 36, 40, 41, 44, 48, 52]:
            r = terms(d, bits, N, delta, g1, qd)
            if r is None:
                print(f"  {d:<5d} {bits:<6d} code too small"); continue
            print(f"  {d:<5d} {bits:<6d} {2.0**bits/N:<10.3g} {np.degrees(r[0]):6.2f}      {r[1]:.3e}      {r[2]:.3e}")
        lo = next(b for b in range(10, 200) if (terms(d, b, N, delta, g1, qd) or (0, 0, 9))[2] <= 0.01)
        print(f"  d={d}: least bits with the refined last term <= 0.01: {lo}")


def part2(seed=3):
    print("\nPART 2  Monte Carlo of Pr[G_1(c) and angle(c, z_2) <= th] for one uniform codeword")
    rng = np.random.default_rng(seed)
    print("  d    delta   th(deg)  draws     count   measured    refined bound  first bound   meas/refined  meas/first")
    for d, delta, thdeg, n in [(12, 0.01, 75, 20000000), (12, 0.002, 75, 20000000), (6, 0.01, 70, 10000000), (24, 0.01, 80, 20000000)]:
        qd = q(d, delta); th = np.radians(thdeg); batch = 100000; cnt = 0
        unit = lambda a: a / np.linalg.norm(a, axis=1, keepdims=True)
        for _ in range(n // batch):
            D = unit(rng.standard_normal((batch, d + 1)))
            c = unit(rng.standard_normal((batch, d + 1)))
            z1 = rng.standard_normal((batch, d + 1)); z1 = unit(z1 - (z1 * D).sum(1, keepdims=True) * D)
            c1 = (c * z1).sum(1)
            ok = c1 >= np.cos(th)
            tau = -(c * D).sum(1)
            ok &= tau > qd * np.sqrt(np.maximum(1 - c1 * c1, 0))
            if ok.any():
                i = np.nonzero(ok)[0]
                z2 = rng.standard_normal((len(i), d + 1)); z2 = unit(z2 - (z2 * D[i]).sum(1, keepdims=True) * D[i])
                cnt += np.count_nonzero((c[i] * z2).sum(1) >= np.cos(th))
        meas = cnt / n
        ref = delta * np.exp(lnI(d, th, qd)); first = delta * cap(d, th) * cap(d - 1, th)
        print(f"  {d:<4d} {delta:<7} {thdeg:<7d}  {n:<9d} {cnt:<7d} {meas:.3e}   {ref:.3e}      {first:.3e}     {meas/ref:.2f}          {meas/first:.2f}")


if __name__ == "__main__":
    part1(); part2()
