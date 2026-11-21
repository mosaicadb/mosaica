# Referee check of F2 in three dimensions, where the head set and the tail are both non-empty.
#   part 1: the inner inequality (*) of core_f2chain.py for single cells, with a head of two coordinates and a tail
#           of one (Urysohn in the plane on each fibre) and a head of one and a tail of two (triangular coupling in
#           the plane), every assignment of the coordinates.  Mean widths are exact: for a polytope in R^3,
#           E_g h_K(g) = E|g_3| M / (4 pi), M = (1/2) sum over edges of length x exterior dihedral angle.
#   part 2: whole codes on the cube [-4, 4]^3 (Voronoi partitions in several metrics, lattices); E h and H exact for
#           the piecewise-constant law on the cube, compared with max over all 8 head sets S of V_S(H).
#           The law is the Gaussian cut to the cube and made constant on voxels (mass outside 1.9e-4), so the
#           ratio is approximate at that level.
import itertools
import math
import sys
import numpy as np
from scipy.special import ndtr
from scipy.spatial import ConvexHull
from core_l2_2d import corr1d, gap2d

EG3 = math.sqrt(8 / math.pi)


def log_kappa(k):
    leg = 0.5 * math.log(2) + math.lgamma((k + 1) / 2) - math.lgamma(k / 2)
    lvol = k / 2 * math.log(math.pi) - math.lgamma(k / 2 + 1)
    return leg + 0.5 * math.log(2 * math.pi * math.e) - lvol / k


def Wf(a, H):
    """Water-filling value for sqrt-eigenvalues a (any order) at rate H nats."""
    a = np.sort(np.asarray(a, float))[::-1]
    if a.size == 0:
        return 0.0
    if H <= 0:
        return float(a.sum())
    la = np.log(a)
    for k in range(1, a.size + 1):
        lam = math.exp((la[:k].sum() - H) / k)
        if k == a.size or lam >= a[k]:
            return k * lam + float(a[k:].sum())


def VS(a, S, H):
    """V_S(H) for sqrt-eigenvalues a and head index set S (tuple).  Convex in u: golden section plus the ends."""
    a = np.asarray(a, float)
    S = list(S)
    T = [i for i in range(a.size) if i not in S]
    if not S:
        return Wf(a, H)
    k = len(S)
    lhead = log_kappa(k) + np.log(a[S]).sum() / k
    f = lambda u: math.exp(lhead - (H - u) / k) + Wf(a[T], u)
    lo, hi = 0.0, H
    gr = (math.sqrt(5) - 1) / 2
    c, d = hi - gr * (hi - lo), lo + gr * (hi - lo)
    fc, fd = f(c), f(d)
    for _ in range(60):
        if fc < fd:
            hi, d, fd = d, c, fc
            c = hi - gr * (hi - lo); fc = f(c)
        else:
            lo, c, fc = c, d, fd
            d = lo + gr * (hi - lo); fd = f(d)
    return min(f(0.0), f(H), f(0.5 * (lo + hi)))


def gauss_width(mask, e, a):
    """E_g h_K(g) for K = hull of the voxels in mask, coordinates scaled by a."""
    idx = np.argwhere(mask)
    corners = set()
    for d in itertools.product((0, 1), repeat=3):
        for row in map(tuple, idx + np.array(d)):
            corners.add(row)
    c = np.array(sorted(corners))
    pts = np.stack([e[c[:, 0]] * a[0], e[c[:, 1]] * a[1], e[c[:, 2]] * a[2]], 1)
    hull = ConvexHull(pts)
    eq = hull.equations[:, :3]
    M = 0.0
    for i, simp in enumerate(hull.simplices):
        for j in range(3):
            nb = hull.neighbors[i, j]
            v = [simp[m] for m in range(3) if m != j]
            ln = np.linalg.norm(hull.points[v[0]] - hull.points[v[1]])
            cs = float(np.clip(eq[i] @ eq[nb], -1.0, 1.0))
            M += ln * math.acos(cs)
    M *= 0.25          # each edge seen from both of its facets; M = (1/2) sum l alpha
    return EG3 * M / (4 * math.pi)


def entropy(M, vol):
    m = M > 0
    return float(-(M[m] * np.log(M[m] / vol[m])).sum())


def star(M, e, a, tail):
    """LHS / RHS of (*) for the law M (3-d array of masses), tail = tuple of axes, head = the rest."""
    M = M / M.sum()
    head = tuple(i for i in range(3) if i not in tail)
    w = np.diff(e)
    vol = w[:, None, None] * w[None, :, None] * w[None, None, :]
    h = entropy(M, vol)
    lhs = gauss_width(M > 0, e, a)
    if len(tail) == 1:
        Mt = M.sum(axis=head)
        hT = float(-(Mt[Mt > 0] * np.log(Mt[Mt > 0] / w[Mt > 0])).sum())
        corr = a[tail[0]] * float(corr1d(e, Mt))
        # head of two: E|g_2| omega_2^(-1/2) = 1/sqrt 2
        rhs = corr + math.sqrt(a[head[0]] * a[head[1]]) / math.sqrt(2) * math.exp((h - hT) / 2)
    else:
        Mt = M.sum(axis=head[0])
        if tail[0] > tail[1]:
            Mt = Mt.T
        gap, hT, C1, C2 = gap2d(e, e, Mt)
        corr = a[tail[0]] * C1 + a[tail[1]] * C2
        rhs = corr + a[head[0]] / math.sqrt(2 * math.pi) * math.exp(h - hT)
    return lhs / rhs


def part1(N=32, L=4.0):
    e = np.linspace(-L, L, N + 1)
    x = 0.5 * (e[:-1] + e[1:])
    X, Y, Z = np.meshgrid(x, x, x, indexing="ij")
    p = np.diff(ndtr(e))
    base = p[:, None, None] * p[None, :, None] * p[None, None, :]
    shapes = [
        ("ball r=1.5", X * X + Y * Y + Z * Z < 2.25),
        ("cube side 2", (np.abs(X) < 1) & (np.abs(Y) < 1) & (np.abs(Z) < 1)),
        ("tilted slab", (np.abs(X - Y) < 0.4) & (np.abs(X) < 2) & (np.abs(Z) < 1.5)),
        ("tilted rod", ((X - Z) ** 2 + (Y - 0.5 * Z) ** 2 < 0.3) & (np.abs(Z) < 2)),
        ("simplex", (X > 0) & (Y > 0) & (Z > 0) & (X + Y + Z < 2.5)),
        ("plate", (np.abs(Z - 0.5 * X - 0.5 * Y) < 0.3) & (X * X + Y * Y < 4)),
        ("off-centre ball", (X - 1.5) ** 2 + (Y + 1) ** 2 + (Z - 0.5) ** 2 < 0.8),
        ("box 3 x 1 x 0.5", (np.abs(X) < 1.5) & (np.abs(Y) < 0.5) & (np.abs(Z) < 0.25)),
    ]
    spectra = [(1, 1, 1), (1, 1, 0.1), (1, 0.1, 0.1), (1, 0.3, 0.05)]
    tails = [(0,), (1,), (2,), (0, 1), (1, 0), (0, 2), (2, 0), (1, 2), (2, 1)]
    worst = (np.inf, None)
    for name, mask in shapes:
        best = (np.inf, None)
        for a in spectra:
            for perm in set(itertools.permutations(a)):
                for tail in tails:
                    r = star(base * mask, e, np.array(perm, float), tail)
                    if r < best[0]:
                        best = (r, (perm, tail))
        print("  shape %-16s min LHS/RHS = %.4f at spectrum %s tail axes %s" % (name, best[0], best[1][0], best[1][1]))
        sys.stdout.flush()
        if best[0] < worst[0]:
            worst = (best[0], name)
    return worst


def part2(N=32, L=4.0, seed=3):
    rng = np.random.default_rng(seed)
    e = np.linspace(-L, L, N + 1)
    x = 0.5 * (e[:-1] + e[1:])
    X, Y, Z = np.meshgrid(x, x, x, indexing="ij")
    P3 = np.stack([X, Y, Z], -1).reshape(-1, 3)
    p = np.diff(ndtr(e))
    mass = (p[:, None, None] * p[None, :, None] * p[None, None, :])
    mass = mass / mass.sum()
    worst = (np.inf, None)
    subsets = [S for k in range(4) for S in itertools.combinations(range(3), k)]
    for a in [(1, 1, 1), (1, 1, 0.1), (1, 0.1, 0.1), (1, 0.3, 0.05)]:
        a = np.array(a, float)
        for kind in ("voronoi", "voronoi-metric", "cubic", "bcc", "head grid x tail disc"):
            if kind.startswith("voronoi"):
                K = int(rng.choice([8, 20, 50]))
                sites = rng.normal(0, 1.3, (K, 3))
                met = a if kind == "voronoi-metric" else np.ones(3)
                d = (((P3[:, None, :] - sites[None]) ** 2) * met).sum(-1)
                lab = d.argmin(1)
            elif kind == "cubic":
                s = np.array([1.0, 1.0, 1.0]) * rng.choice([1.0, 1.6]) / np.sqrt(a) ** 0.5
                lab = (np.floor(P3 / s + 0.5) * np.array([1, 100, 10000])).sum(1).astype(int)
            elif kind == "bcc":
                s = rng.choice([1.2, 2.0]) / np.sqrt(a) ** 0.5
                q0 = np.round(P3 / s)
                q1 = np.round(P3 / s - 0.5) + 0.5
                d0 = (((P3 / s - q0)) ** 2).sum(1); d1 = (((P3 / s - q1)) ** 2).sum(1)
                q = np.where((d0 <= d1)[:, None], q0, q1)
                lab = (np.round(q * 2) * np.array([1, 100, 10000])).sum(1).astype(int)
            else:
                # coordinate 0 on a grid; coordinates 1, 2 in a disc of radius 2.2 or in one of 8 outer sectors
                w = rng.choice([0.5, 1.0])
                i0 = np.floor(P3[:, 0] / w + 0.5).astype(int)
                rad = np.hypot(P3[:, 1], P3[:, 2])
                sec = np.where(rad < 2.2, 0, 1 + np.floor((np.arctan2(P3[:, 2], P3[:, 1]) + math.pi) / (math.pi / 4)))
                lab = i0 * 100 + sec.astype(int)
            _, lab = np.unique(lab, return_inverse=True)
            lab = lab.reshape(N, N, N)
            Eh = H = 0.0
            for c in range(lab.max() + 1):
                m = lab == c
                P = float(mass[m].sum())
                if P <= 0:
                    continue
                H -= P * math.log(P)
                Eh += P * gauss_width(m, e, a)
            fl = [(VS(a, S, H), S) for S in subsets]
            best = max(fl)
            print("  a=%s %-22s cells=%d H=%.3f Eh=%.4f  W=%.4f  max_S V_S=%.4f at S=%s  Eh/W=%.3f  Eh/maxV=%.3f"
                  % (tuple(a), kind, lab.max() + 1, H, Eh, fl[0][0], best[0], best[1], Eh / fl[0][0], Eh / best[0]))
            sys.stdout.flush()
            if Eh / best[0] < worst[0]:
                worst = (Eh / best[0], (tuple(a), kind, best[1]))
    return worst


if __name__ == "__main__":
    # self-test of the mean width: a cube of side 2 has E_g h = E|g_1| * 3
    e = np.array([-1.0, 1.0])
    v = gauss_width(np.ones((1, 1, 1), bool), e, np.ones(3))
    assert abs(v - 3 * math.sqrt(2 / math.pi)) < 1e-9, v
    assert abs(VS([1, 1, 1], (0, 1, 2), 0.7) - math.exp(log_kappa(3) - 0.7 / 3)) < 1e-12
    assert abs(Wf([2, 1, 1], math.log(2)) - 3.0) < 1e-12
    which = sys.argv[1] if len(sys.argv) > 1 else "both"
    if which in ("1", "both"):
        w = part1()
        print("part 1 (single cells, (*) in three dimensions): smallest LHS/RHS = %.4f (%s)" % w)
    if which in ("2", "both"):
        w = part2()
        print("part 2 (codes on the cube): smallest Eh / max_S V_S = %.4f" % w[0], w[1])
