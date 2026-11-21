# The reference values the gap table is read against. All [computed] by quadrature; no simulation here.
#  1. The sphere bound sin(phi) = 2^-R, and its finite-dimension form.
#       Lower bound (any code of M = 2^(mR) points, y uniform on the sphere of R^m): for every theta,
#         P[phi <= theta] <= min(1, M Omega_m(theta)),  Omega_m the normalised measure of a cap of angle theta
#       (union bound), so E g(phi) >= E g(phi*) for nondecreasing g, phi* with distribution function min(1, M Omega_m).
#       Achievable (random code of M uniform points): P[phi > theta] = (1 - Omega_m(theta))^M.
#  2. Scalar quantisers of N(0,1) in the long-block limit, tan^2 = E c^2 E y^2 / (E c y)^2 - 1 (the gain is free):
#       Lloyd-Max (fixed rate), the built uniform grid at its best scale (fixed rate, uniform), and the uniform-threshold
#       quantiser with centroid reconstruction at entropy R (entropy-coded scalar).
import math
import numpy as np
from clib import *


def log_cap(m, ng=400001):
    """theta grid on (0, pi/2] and ln Omega_m(theta)."""
    th = np.linspace(0.0, math.pi / 2, ng)[1:]
    lf = (m - 2) * np.log(np.sin(th))
    # cumulative trapezoid in logs
    seg = np.logaddexp(lf[1:], lf[:-1]) + math.log(0.5 * (th[1] - th[0]))
    first = lf[0] + math.log(th[0] / (m - 1))  # int_0^th0 sin^(m-2) ~ th0^(m-1)/(m-1)
    lc = np.logaddexp.accumulate(np.concatenate(([first], seg)))
    ltot = math.lgamma(0.5) + math.lgamma((m - 1) / 2) - math.lgamma(m / 2)  # int_0^pi sin^(m-2)
    return th, lc - ltot


def finite_bounds(m, R):
    th, lc = log_cap(m)
    lM = m * R * math.log(2.0)
    x = lM + lc  # ln(M Omega)
    F_lo = np.minimum(1.0, np.exp(np.minimum(x, 0.0)))        # distribution function of phi* (best conceivable)
    F_rc = -np.expm1(-np.exp(np.minimum(x, 50.0)))            # random code: 1 - exp(-M Omega) (M huge)
    t = np.tan(np.minimum(th, math.pi / 2 - 1e-9))
    out = []
    for F in (F_lo, F_rc):
        dF = np.diff(np.concatenate(([0.0], F)))
        tm = np.concatenate(([0.0], t))
        mid = 0.5 * (tm[1:] + tm[:-1])
        w = dF.sum()
        out.append((float((mid * mid * dF).sum() / w), float((mid * dF).sum() / w)))
    i0 = int(np.searchsorted(x, 0.0))
    return out[0], out[1], float(np.sin(th[min(i0, len(th) - 1)]))


def ecsq(R):
    """Uniform-threshold quantiser of N(0,1), centroid reconstruction, step chosen so that the output entropy is R bits;
    the better of the grids with a cell centred at 0 and with an edge at 0. Returns (D, tan^2, step)."""
    best = None
    for off in (0.0, 0.5):
        def stats(step):
            k = int(12.0 / step) + 2
            edges = (np.arange(-k, k + 1) + off) * step
            edges = np.concatenate(([-np.inf], edges, [np.inf]))
            p, m1, m2 = cell_moments(edges)
            ok = p > 1e-300
            H = float(-(p[ok] * np.log2(p[ok])).sum())
            lev = np.where(ok, m1 / np.maximum(p, 1e-300), 0.0)
            D = float((m2 - 2 * lev * m1 + lev * lev * p).sum())
            return H, D
        lo, hi = 1e-3, 12.0
        for _ in range(80):
            mid = math.sqrt(lo * hi)
            H, D = stats(mid)
            if H > R:
                lo = mid
            else:
                hi = mid
        H, D = stats(hi)
        if best is None or D < best[0]:
            best = (D, D / (1.0 - D), hi)
    return best


if __name__ == "__main__":
    print("== 1. sphere bound, tan(phi) ==")
    print("R      asymptotic   | per block size m: lower bound rms tan / mean tan ; random code rms tan ; sin(theta0)·2^R")
    for R in (0.125, 0.25, 0.5, 1, 2, 3, 4, 5, 6, 7):
        row = f"{R:<6} {math.sqrt(bound_tan2(R)):.4f}      |"
        for m in BLOCKS:
            lo, rc, s0 = finite_bounds(m, R)
            row += f" m={m}: {math.sqrt(lo[0]):.4f}/{lo[1]:.4f} ; {math.sqrt(rc[0]):.4f} ; {s0 * 2 ** R:.4f} |"
        print(row)
    print()
    print("== 2. scalar quantisers of N(0,1), long-block limit ==")
    print("bits  bound tan2  ECSQ tan2 (D)        Lloyd-Max tan2 (D)     uniform best tan2 (lam)   | tan ratios to bound: ECSQ, LM, uniform")
    for b in range(1, 9):
        bt = bound_tan2(b)
        De, te, _ = ecsq(b)
        lev, edges, Dl = lloyd_max(1 << b)
        tl = scalar_tan2(lev, edges)
        lam, tu = golden(lambda l: uniform_tan2(b, l), 0.5, 8.0)
        print(f"{b}     {bt:.6f}    {te:.6f} ({De:.6f})  {tl:.6f} ({Dl:.6f})   {tu:.6f} ({lam:.3f})        | "
              f"{math.sqrt(te / bt):.3f} {math.sqrt(tl / bt):.3f} {math.sqrt(tu / bt):.3f}")
    print()
    print("== 3. uniform grid, tan2 at each of the six built scales (long-block limit) ==")
    for b in range(1, 9):
        print(b, " ".join(f"{uniform_tan2(b, l):.6f}" for l in LAMS))
    print()
    print("== 4. sub-bit rates: signs of a share r of the positions (Proposition 13): tan2 = pi/(2r) - 1 ==")
    for r in (0.125, 0.25, 0.5):
        t = math.pi / (2 * r) - 1
        print(f"r={r}: tan2 {t:.4f} tan {math.sqrt(t):.4f}; bound tan {math.sqrt(bound_tan2(r)):.4f}; ratio {math.sqrt(t / bound_tan2(r)):.3f}")
