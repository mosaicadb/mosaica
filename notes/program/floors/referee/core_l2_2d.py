# Referee check of L2 in two dimensions (CLAIMS.md section 1).
# For a density that is constant on the rectangles of a product grid, h, C_1 and C_2 are closed forms:
#   gap(nu) = ln(2 pi e) + ln C_1(nu) + ln C_2(nu) - h(nu)   must be >= 0   (L2), for both coordinate orders.
# Tested: random densities with zeros and spikes, Gaussians restricted to non-product shapes, and an L-BFGS
# adversary that minimises the gap over masses and breakpoints.
import math
import sys
import numpy as np
from scipy.special import ndtr, ndtri
from scipy.optimize import minimize

S2PI = math.sqrt(2 * math.pi)


def phi(x):
    return np.exp(-0.5 * x * x) / S2PI


def Gfun(x):
    # antiderivative of Phi(x) x phi(x):  -Phi phi + Phi(sqrt2 x) / (2 sqrt pi); limits 0 and 1/(2 sqrt pi)
    return -ndtr(x) * phi(x) + ndtr(math.sqrt(2) * x) / (2 * math.sqrt(math.pi))


def corr1d(edges, p):
    """C(mu) = int_0^1 Q(u) Phi^-1(u) du for the law with mass p[..., j] spread uniformly on [edges[j], edges[j+1]].
    p may have leading axes (each row a law); rows need not be normalised (they are normalised here)."""
    p = np.asarray(p, float)
    tot = p.sum(-1, keepdims=True)
    q = p / tot
    w = np.diff(edges)
    # mass below and above each piece, each summed from its own end so that both tails keep relative precision
    U1 = np.cumsum(q, -1)
    U0 = np.clip(U1 - q, 0.0, 1.0)
    A0 = np.cumsum(q[..., ::-1], -1)[..., ::-1]          # mass at or above the piece's left edge
    A1 = np.clip(A0 - q, 0.0, 1.0)                       # mass above the piece's right edge
    U1 = np.clip(U1, 0.0, 1.0)
    A0 = np.clip(A0, 0.0, 1.0)
    x0 = np.where(U0 <= 0.5, ndtri(U0), -ndtri(A0))
    x1 = np.where(U1 <= 0.5, ndtri(U1), -ndtri(A1))
    f0 = np.where(np.isfinite(x0), x0, 0.0)
    f1 = np.where(np.isfinite(x1), x1, 0.0)
    ph0 = np.where(np.isfinite(x0), phi(f0), 0.0)
    ph1 = np.where(np.isfinite(x1), phi(f1), 0.0)
    r2 = math.sqrt(2)
    # D = Phi(sqrt2 x1) - Phi(sqrt2 x0), from the nearer tail
    lo = np.where(x0 == -np.inf, 0.0, ndtr(r2 * f0))
    hi = np.where(x1 == np.inf, 1.0, ndtr(r2 * f1))
    lo_u = np.where(x0 == -np.inf, 1.0, ndtr(-r2 * f0))
    hi_u = np.where(x1 == np.inf, 0.0, ndtr(-r2 * f1))
    D = np.where(x0 >= 0, lo_u - hi_u, hi - lo)
    # int_{x0}^{x1} [y_j + w (Phi(x) - Phi(x0)) / q] x phi(x) dx
    #   = y_j (phi(x0) - phi(x1)) - w phi(x1) + (w / q) D / (2 sqrt pi)
    with np.errstate(divide="ignore", invalid="ignore"):
        term = edges[:-1] * (ph0 - ph1) - w * ph1 + np.where(q > 0, w / np.where(q > 0, q, 1.0), 0.0) * D / (
            2 * math.sqrt(math.pi))
    term = np.where(q > 0, term, 0.0)
    return term.sum(-1)


def gap2d(e1, e2, P):
    """P[i, j] = mass on [e1[i], e1[i+1]] x [e2[j], e2[j+1]].  Order: coordinate 1 first, then 2 given 1."""
    P = P / P.sum()
    area = np.outer(np.diff(e1), np.diff(e2))
    m = P > 0
    h = -(P[m] * np.log(P[m] / area[m])).sum()
    P1 = P.sum(1)
    C1 = corr1d(e1, P1)
    rows = P1 > 0
    C2 = (P1[rows] * corr1d(e2, P[rows])).sum()
    return math.log(2 * math.pi * math.e) + math.log(C1) + math.log(C2) - h, h, C1, C2


def both_orders(e1, e2, P):
    return min(gap2d(e1, e2, P)[0], gap2d(e2, e1, P.T)[0])


def selftest():
    # uniform on [0,1]: C = 1/(2 sqrt pi)
    c = corr1d(np.array([0.0, 1.0]), np.array([1.0]))
    assert abs(c - 1 / (2 * math.sqrt(math.pi))) < 1e-12, c
    # fine discretised standard Gaussian: C -> 1, product Gaussian gap -> 0
    e = np.linspace(-8, 8, 3201)
    p = np.diff(ndtr(e))
    c = corr1d(e, p)
    assert abs(c - 1) < 1e-4, c
    e = np.linspace(-7, 7, 401)
    p = np.diff(ndtr(e))
    g = gap2d(e, e, np.outer(p, p))[0]
    assert 0 <= g < 1e-2, g
    # correlated Gaussian, rho = 0.8: exact gap is 0 for the triangular map (it is linear)
    rho = 0.8
    x = 0.5 * (e[:-1] + e[1:])
    X, Y = np.meshgrid(x, x, indexing="ij")
    D = np.exp(-(X * X - 2 * rho * X * Y + Y * Y) / (2 * (1 - rho * rho)))
    g = both_orders(e, e, D)
    assert -1e-9 <= g < 2e-2, g
    print("selftest ok: uniform C, Gaussian C -> 1, product and correlated Gaussian gaps", round(g, 5))


def random_cases(rng, n_cases):
    worst = (np.inf, None)
    for t in range(n_cases):
        a, b = rng.integers(2, 9), rng.integers(2, 9)
        e1 = np.sort(rng.normal(0, rng.choice([0.3, 1, 3]), a + 1))
        e2 = np.sort(rng.normal(0, rng.choice([0.3, 1, 3]), b + 1))
        if np.diff(e1).min() < 1e-6 or np.diff(e2).min() < 1e-6:
            continue
        kind = t % 4
        P = rng.random((a, b))
        if kind == 1:
            P = P ** 6                          # spikes
        elif kind == 2:
            P = P * (rng.random((a, b)) < 0.5)  # zeros: gaps, non-convex and disconnected supports
        elif kind == 3:
            P = np.exp(rng.normal(0, 3, (a, b)))
        if P.sum() <= 0 or (P.sum(1) > 0).sum() < 1:
            continue
        g = both_orders(e1, e2, P)
        if g < worst[0]:
            worst = (g, (kind, a, b))
    return worst


def shapes(rng):
    """Gaussian restricted to non-product sets, on a fine grid (the law is the piecewise-constant one, so the
    check is exact for that law)."""
    e = np.linspace(-5, 5, 241)
    x = 0.5 * (e[:-1] + e[1:])
    X, Y = np.meshgrid(x, x, indexing="ij")
    base = np.outer(np.diff(ndtr(e)), np.diff(ndtr(e)))
    worst = (np.inf, None)
    out = []
    for name, mask in [
        ("disc r=1", X * X + Y * Y < 1),
        ("disc r=0.3 at (2,1)", (X - 2) ** 2 + (Y - 1) ** 2 < 0.09),
        ("annulus 1.5..2", (X * X + Y * Y > 2.25) & (X * X + Y * Y < 4)),
        ("thin tilted strip", np.abs(X - Y) < 0.1),
        ("triangle", (X > 0) & (Y > 0) & (X + Y < 1.5)),
        ("two discs", ((X - 2) ** 2 + Y * Y < 0.25) | ((X + 2) ** 2 + Y * Y < 0.25)),
        ("outside disc r=2", X * X + Y * Y > 4),
        ("L shape", ((X > 0) & (X < 2) & (Y > 0) & (Y < 0.4)) | ((X > 0) & (X < 0.4) & (Y > 0) & (Y < 2))),
        ("checkerboard", ((np.floor(X) + np.floor(Y)) % 2 == 0)),
        ("tilted ellipse", (X + Y) ** 2 / 9 + (X - Y) ** 2 / 0.04 < 1),
    ]:
        g = both_orders(e, e, base * mask)
        out.append((name, g))
        if g < worst[0]:
            worst = (g, name)
    return worst, out


def adversary(rng, a, b, starts):
    """Minimise the gap over masses (softmax) and breakpoints (cumulative exp), order 1 then 2."""
    def unpack(v):
        P = np.exp(v[: a * b] - v[: a * b].max()).reshape(a, b)
        e1 = np.concatenate([[0.0], np.cumsum(np.exp(np.clip(v[a * b: a * b + a], -8, 4)))]) + v[-2]
        e2 = np.concatenate([[0.0], np.cumsum(np.exp(np.clip(v[a * b + a: a * b + a + b], -8, 4)))]) + v[-1]
        return e1, e2, P

    def f(v):
        e1, e2, P = unpack(v)
        return gap2d(e1, e2, P)[0]

    best = (np.inf, None)
    for s in range(starts):
        v0 = np.concatenate([rng.normal(0, 1.5, a * b), rng.normal(-0.5, 0.5, a + b), rng.normal(-1, 1, 2)])
        r = minimize(f, v0, method="L-BFGS-B", options={"maxiter": 300})
        if r.fun < best[0]:
            best = (float(r.fun), r.x)
    return best


if __name__ == "__main__":
    rng = np.random.default_rng(20261121)
    selftest()
    w = random_cases(rng, 20000)
    print("random densities, both orders: smallest gap %.3e" % w[0], w[1])
    w, out = shapes(rng)
    for name, g in out:
        print("  shape %-22s gap %.5f" % (name, g))
    print("shapes: smallest gap %.5f (%s)" % w)
    for (a, b, starts) in [(2, 2, 12), (3, 3, 10), (4, 4, 6), (6, 3, 6)]:
        g, v = adversary(rng, a, b, starts)
        print("adversary %dx%d: smallest gap %.3e" % (a, b, g))
    sys.stdout.flush()
