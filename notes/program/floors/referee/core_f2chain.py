# Referee check of the inner inequalities of F2's proof in the plane, head and tail both of one coordinate.
# For any law nu with a density carried by a set K (not only a Gaussian cell), steps 1 to 4 of F2 give
#   (*)  E_g h_{Gamma^(1/2) K}(g) >= sqrt(gam_T) C(nu_T) + sqrt(gam_S) (E|g| / 2) exp(h(nu) - h(nu_T)).
# Everything is exact for a density constant on the rectangles of a product grid.
#   part 1: Gaussian restricted to non-product shapes, both choices of which coordinate is the head
#   part 2: an adversary: random supports on small grids, L-BFGS over the density to maximise RHS / LHS
#   part 3: partitions; the per-cell (*) and the averaged form of step 6
import math
import sys
import numpy as np
from scipy.special import ndtr
from scipy.optimize import minimize
from core_l2_2d import corr1d
from core_opt2d import start_labels, perimeter, MW

HALF_EG = 1.0 / math.sqrt(2 * math.pi)        # E|g| / 2 = E|g_1| omega_1^(-1)


def pieces(ex, ey, M, aT, aS):
    """Tail = axis 0 (x), head = axis 1 (y).  Returns LHS, RHS, C_T, a (conditional entropy), b (tail entropy)."""
    M = M / M.sum()
    area = np.outer(np.diff(ex), np.diff(ey))
    m = M > 0
    h = -(M[m] * np.log(M[m] / area[m])).sum()
    Mx = M.sum(1)
    mx = Mx > 0
    b = -(Mx[mx] * np.log(Mx[mx] / np.diff(ex)[mx])).sum()
    CT = float(corr1d(ex, Mx))
    a = h - b
    lhs = MW * perimeter(m, aT * ex, aS * ey)
    rhs = aT * CT + aS * HALF_EG * math.exp(a)
    return lhs, rhs, CT, a, b


def star_both(ex, ey, M, a1, a2):
    """Smallest LHS/RHS over the two assignments (coordinate 0 has sqrt-eigenvalue a1, coordinate 1 has a2)."""
    l1, r1 = pieces(ex, ey, M, a1, a2)[:2]          # tail = coord 0
    l2, r2 = pieces(ey, ex, M.T, a2, a1)[:2]        # tail = coord 1
    return min(l1 / r1, l2 / r2)


def part1():
    e = np.linspace(-5, 5, 201)
    x = 0.5 * (e[:-1] + e[1:])
    X, Y = np.meshgrid(x, x, indexing="ij")
    base = np.outer(np.diff(ndtr(e)), np.diff(ndtr(e)))
    worst = np.inf
    for name, mask in [
        ("disc r=1", X * X + Y * Y < 1),
        ("disc r=0.3 at (2,1)", (X - 2) ** 2 + (Y - 1) ** 2 < 0.09),
        ("thin tilted strip", (np.abs(X - Y) < 0.1) & (np.abs(X) < 2)),
        ("steep strip", (np.abs(Y - 3 * X) < 0.2) & (np.abs(X) < 1)),
        ("triangle", (X > 0) & (Y > 0) & (X + Y < 1.5)),
        ("tilted ellipse", (X + Y) ** 2 / 9 + (X - Y) ** 2 / 0.04 < 1),
        ("square", (np.abs(X) < 1) & (np.abs(Y) < 1)),
        ("wide rectangle", (np.abs(X) < 3) & (np.abs(Y) < 0.2)),
        ("L shape", ((X > 0) & (X < 2) & (Y > 0) & (Y < 0.4)) | ((X > 0) & (X < 0.4) & (Y > 0) & (Y < 2))),
        ("two discs", ((X - 2) ** 2 + Y * Y < 0.25) | ((X + 2) ** 2 + Y * Y < 0.25)),
    ]:
        rs = []
        for a1, a2 in [(1, 1), (1, 0.3), (1, 0.05), (0.05, 1)]:
            rs.append(star_both(e, e, base * mask, a1, a2))
        worst = min(worst, min(rs))
        print("  shape %-22s min LHS/RHS over spectra and head choice: %.4f" % (name, min(rs)))
    return worst


def part2(rng, trials):
    """Random supports, density optimised.  Returns the smallest LHS/RHS found."""
    worst = (np.inf, None)
    for t in range(trials):
        a, b = rng.integers(2, 7), rng.integers(2, 7)
        ex = np.sort(rng.normal(0, 1, a + 1))
        ey = np.sort(rng.normal(0, 1, b + 1))
        if np.diff(ex).min() < 0.02 or np.diff(ey).min() < 0.02:
            continue
        kind = t % 3
        if kind == 0:
            mask = np.ones((a, b), bool)
        elif kind == 1:
            mask = rng.random((a, b)) < 0.6
        else:                                   # a staircase: a tilted support
            mask = np.abs(np.arange(a)[:, None] * b / a - np.arange(b)[None, :]) <= 1
        if mask.sum() < 2 or mask.any(1).sum() < 1:
            continue
        aT, aS = rng.choice([1.0, 0.3, 0.05]), rng.choice([1.0, 0.3, 0.05])
        idx = np.flatnonzero(mask.ravel())

        def f(v):
            M = np.zeros(a * b)
            M[idx] = np.exp(v - v.max())
            lhs, rhs = pieces(ex, ey, M.reshape(a, b), aT, aS)[:2]
            return lhs / rhs

        best = np.inf
        for s in range(2):
            r = minimize(f, rng.normal(0, 2, idx.size), method="L-BFGS-B", options={"maxiter": 150})
            best = min(best, float(r.fun))
        if best < worst[0]:
            worst = (best, (kind, a, b, aT, aS))
    return worst


def part3(rng):
    L, NF = 5.0, 100
    e = np.linspace(-L, L, NF + 1)
    pm = np.diff(ndtr(e))
    mass = np.outer(pm, pm)
    mass /= mass.sum()
    x = 0.5 * (e[:-1] + e[1:])
    worst_cell, worst_avg, worst_b = np.inf, np.inf, np.inf
    for eps in (1.0, 0.3, 0.05):
        for kind, p1, p2 in [("hex", 0.6, 0.52), ("hex", 1.2, 1.0), ("voronoi", 25, 1.0), ("voronoi", 60, 0.2),
                             ("voronoi", 6, 1.0), ("strips", 0.4, 2.0), ("grid", 0.6, 0.6)]:
            lab = start_labels(kind, x, x, p1, p2, rng)
            _, inv = np.unique(lab, return_inverse=True)
            lab = inv.reshape(lab.shape)
            for tail_axis in (0, 1):
                aT, aS = ((1.0, eps) if tail_axis == 0 else (eps, 1.0))
                Eh = abar = bbar = Cbar = 0.0
                for c in range(lab.max() + 1):
                    M = np.where(lab == c, mass, 0.0)
                    P = M.sum()
                    if P <= 0:
                        continue
                    Mt = M if tail_axis == 0 else M.T
                    lhs, rhs, CT, a, b = pieces(e, e, Mt, aT, aS)
                    worst_cell = min(worst_cell, lhs / rhs)
                    Eh += P * lhs; abar += P * a; bbar += P * b; Cbar += P * CT
                avg = Eh / (aT * Cbar + aS * HALF_EG * math.exp(abar))
                bgap = math.log(math.sqrt(2 * math.pi * math.e) * Cbar) - bbar
                worst_avg = min(worst_avg, avg); worst_b = min(worst_b, bgap)
    return worst_cell, worst_avg, worst_b


if __name__ == "__main__":
    rng = np.random.default_rng(77)
    # exact case: rectangle, tail law pushed to the two ends -> (*) tends to equality
    ex = np.array([-1.0, -0.999, 0.999, 1.0]); ey = np.array([-0.5, 0.5])
    M = np.array([[0.5], [0.0], [0.5]])
    l, r = pieces(ex, ey, M, 1.0, 1.0)[:2]
    print("rectangle with the tail mass at its two ends: LHS/RHS = %.5f (limit 1)" % (l / r))
    w1 = part1()
    print("part 1 (shapes): smallest LHS/RHS = %.4f" % w1)
    w2 = part2(rng, int(sys.argv[1]) if len(sys.argv) > 1 else 300)
    print("part 2 (adversary over densities on random supports): smallest LHS/RHS = %.5f" % w2[0], w2[1])
    w3 = part3(rng)
    print("part 3 (partitions): per cell %.4f, averaged form of step 6 %.4f, b-bar slack %.4f (>= 0)" % w3)
