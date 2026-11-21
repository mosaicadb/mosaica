# Falsification attempt for the one-dimensional lemma (CLAIMS.md, L1):
#   for a law mu on R with a density and a finite second moment,
#       h(mu) <= ln( sqrt(2 pi e) * C(mu) ),   C(mu) = int_0^1 Q_mu(u) Phi^{-1}(u) du,
#   equivalently  KL(mu || N(0,1)) >= m2/2 - 1/2 - ln C(mu)   (and hence >= k(C(mu)), Conjecture A's step 3).
# Densities are piecewise constant on a grid, for which h, C, m2 and KL are computed in closed form (no quadrature
# in u): on a cell [y0, y1] with density f and F(y0) = u0, Q(u) = y0 + (u - u0)/f, and
#   int Phi^{-1}(u) du = -phi(x),   int u Phi^{-1}(u) du = -Phi(x) phi(x) + Phi(sqrt2 x)/(2 sqrt pi),   x = Phi^{-1}(u).
# Run:  nice -n 10 taskset -c 6 env OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 check_lemma1d.py
import math
import numpy as np
from scipy import stats, optimize

rng = np.random.default_rng(20261121)
SQ2PIE = math.sqrt(2 * math.pi * math.e)


_GLX, _GLW = np.polynomial.legendre.leggauss(24)


def _lower_half(y0, f, p):
    """Cells lying below the median, left to right: left edges y0, densities f, masses p (all > 0).
    Returns sum over cells of int_{u0}^{u1} Q(u) Phi^{-1}(u) du, computed in x = Phi^{-1}(u) without cancellation."""
    u1 = np.cumsum(p)
    u0 = u1 - p
    u0[0] = 0.0
    x1 = stats.norm.ppf(np.minimum(u1, 0.5))
    x0 = np.maximum(stats.norm.ppf(u0), -40.0)
    half = (x1 - x0) / 2
    X = x0[:, None] + half[:, None] * (_GLX[None, :] + 1)            # nodes in x
    PH = stats.norm.pdf(X)
    # Phi(x) - Phi(x0): Simpson on thin cells, the difference of cdfs otherwise
    thin = (x1 - x0) < 0.05
    d_simp = (X - x0[:, None]) / 6 * (stats.norm.pdf(x0)[:, None] + 4 * stats.norm.pdf((X + x0[:, None]) / 2) + PH)
    d_cdf = stats.norm.cdf(X) - u0[:, None]
    D = np.where(thin[:, None], d_simp, d_cdf)
    I0 = (X * PH * _GLW[None, :]).sum(1) * half                       # = phi(x0) - phi(x1)
    K = (D * X * PH * _GLW[None, :]).sum(1) * half
    wide = (x1 - x0) > 3.0                                            # exact antiderivatives; no cancellation there
    if wide.any():
        ph = lambda x: np.where(x > -39.9, stats.norm.pdf(x), 0.0)
        G = lambda x, u: -u * ph(x) + np.where(x > -39.9, stats.norm.cdf(math.sqrt(2) * x), 0.0) / (2 * math.sqrt(math.pi))
        I0e = ph(x0) - ph(x1)
        Ke = G(x1, np.minimum(u1, 0.5)) - G(x0, u0) - u0 * I0e
        I0 = np.where(wide, I0e, I0)
        K = np.where(wide, Ke, K)
    return float((y0 * I0 + K / f).sum())


def stats_pc(edges, p):
    """edges: increasing cell edges (len J+1); p: cell masses (sum 1, zeros allowed). Returns h, C, m2, KL."""
    edges = np.asarray(edges, float)
    p = np.asarray(p, float)
    w = np.diff(edges)
    ok = p > 0
    f = np.where(ok, p / w, 0.0)
    h = -(p[ok] * np.log(f[ok])).sum()
    m2 = (p * (edges[1:] ** 3 - edges[:-1] ** 3) / (3 * w)).sum()
    # split at the median; the upper half is handled by the reflection y -> -y, under which C is invariant
    lo, fl, pl = edges[:-1][ok], f[ok], p[ok]
    hi = edges[1:][ok]
    cum = np.cumsum(pl)
    j = int(np.searchsorted(cum, 0.5))                 # the cell holding the median
    j = min(j, len(pl) - 1)
    below = 0.5 - (cum[j] - pl[j])                     # mass of cell j below the median
    below = min(max(below, 0.0), pl[j])
    ymed = lo[j] + below / fl[j]
    L_y0 = np.append(lo[:j], lo[j]); L_f = np.append(fl[:j], fl[j]); L_p = np.append(pl[:j], below)
    U_y0 = np.append(-hi[:j:-1], -hi[j]); U_f = np.append(fl[:j:-1], fl[j]); U_p = np.append(pl[:j:-1], pl[j] - below)
    C = 0.0
    for y0_, f_, p_ in ((L_y0, L_f, L_p), (U_y0, U_f, U_p)):
        keep = p_ > 0
        if keep.any():
            C += _lower_half(y0_[keep], f_[keep], p_[keep].copy())
    KL = -h + 0.5 * math.log(2 * math.pi) + 0.5 * m2
    return h, C, m2, KL


def gap(edges, p):
    h, C, m2, KL = stats_pc(edges, p)
    return math.log(SQ2PIE * C) - h, h, C, m2, KL


worst = (9e9, None)
count = 0


def record(name, edges, p):
    global worst, count
    g, h, C, m2, KL = gap(edges, p)
    k = 0.5 * (C * C - 1) - math.log(C)
    assert KL - k >= g - 1e-9          # the second form is implied by the first (m2 >= C^2)
    count += 1
    if g < worst[0]:
        worst = (g, name)
    return g


print("gap = ln(sqrt(2 pi e) C) - h must be >= 0; equality only for a Gaussian.")
# (a) discretised Gaussians N(m, s^2): the gap must be small and positive and fall with the grid
for J in (50, 400, 3200):
    for s, m in ((1.0, 0.0), (0.2, 3.0), (5.0, -1.0)):
        e = m + s * np.linspace(-8, 8, J + 1)
        p = np.diff(stats.norm.cdf((e - m) / s)); p /= p.sum()
        print("  (a) Gaussian N(%g,%g^2) on %4d cells: gap %.3e" % (m, s, J, record("gauss", e, p)))
# (b) uniform on an interval: gap = ln(sqrt(e/2)) = 0.1534...
print("  (b) uniform: gap %.6f (exact ln sqrt(e/2) = %.6f)" % (record("unif", [0.3, 1.7], [1.0]), 0.5 * math.log(math.e / 2)))
# (c) cells of a quantiser: N(0,1) restricted to [a, b]
for a, b in ((-1, 1), (0, 0.1), (2, 2.5), (-0.5, 4), (3, 9), (-6, 6)):
    e = np.linspace(a, b, 4001); p = np.diff(stats.norm.cdf(e)); p /= p.sum()
    print("  (c) N(0,1) on [%g, %g]: gap %.5f" % (a, b, record("trunc", e, p)))
# (d) random piecewise-constant densities, with gaps (zero cells), spikes, heavy spread
for trial in range(40000):
    J = int(rng.integers(1, 60))
    e = np.sort(rng.normal(0, math.exp(rng.normal(0, 1.5)), J + 1) + rng.normal(0, 3))
    if np.min(np.diff(e)) <= 1e-12:
        continue
    conc = math.exp(rng.uniform(-3, 2))
    p = rng.gamma(conc, 1.0, J) + 1e-300
    if J > 2 and rng.random() < 0.5:
        p[rng.random(J) < 0.4] = 0.0
    if p.sum() <= 0:
        continue
    p /= p.sum()
    record("random", e, p)
print("  (d) random piecewise-constant densities (gaps, spikes): %d laws so far, smallest gap %.3e (%s)" % (count, worst[0], worst[1]))
# (e) scale mixtures and bimodal laws on a fine grid
for name, pdf in (("two Gaussians +-3", lambda y: 0.5 * stats.norm.pdf(y - 3) + 0.5 * stats.norm.pdf(y + 3)),
                  ("scale mixture 0.1/3", lambda y: 0.7 * stats.norm.pdf(y / 0.1) / 0.1 + 0.3 * stats.norm.pdf(y / 3) / 3),
                  ("Laplace", lambda y: 0.5 * np.exp(-np.abs(y))),
                  ("Student t3", lambda y: stats.t.pdf(y, 3))):
    e = np.linspace(-60, 60, 240001); mid = (e[1:] + e[:-1]) / 2; p = pdf(mid) * np.diff(e); p /= p.sum()
    print("  (e) %-20s gap %.5f" % (name, record(name, e, p)))
# (f) adversary: minimise the gap over piecewise-constant densities on a fixed grid (softmax masses)
def advers(J, L):
    e = np.linspace(-L, L, J + 1)
    obj = lambda th: gap(e, np.exp(th - th.max()) / np.exp(th - th.max()).sum())[0]
    best = 9e9
    for t in range(6):
        th0 = rng.normal(0, 2.0, J)
        r = optimize.minimize(obj, th0, method="L-BFGS-B", options={"maxiter": 400})
        best = min(best, r.fun)
        p = np.exp(r.x - r.x.max()); p /= p.sum()
        record("adversary", e, p)
    # the discretised Gaussian on the same grid, for comparison
    pg = np.diff(stats.norm.cdf(e)); pg /= pg.sum()
    return best, gap(e, pg)[0]
for J, L in ((8, 4), (20, 5), (40, 6)):
    b, gg = advers(J, L)
    print("  (f) adversary on %2d cells of [-%g, %g]: smallest gap found %.3e; discretised N(0,1) on that grid %.3e" % (J, L, L, b, gg))
print("laws tested: %d; smallest gap overall: %.3e (%s); violations (gap < -1e-9): %s" % (count, worst[0], worst[1], worst[0] < -1e-9))
