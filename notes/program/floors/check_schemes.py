# Falsification attempt for the floors F1 (water-filling) and F2 (combined) of CLAIMS.md on explicit codes whose mean
# certified half-width and message entropy are computed exactly (parts 1, 2) or by a large fixed sample (part 3).
#   E h (exact range certificate of the code)  >=  V_S(H(c)) for every head set S,   V_emptyset = W.
# The exact range is the smallest sound certificate of a code, so a ratio below 1 for any code and any S refutes F2
# (and a ratio below 1 against W refutes F1, the section 6 conjecture).
#   part 1: product codes of interval partitions, n <= 5, every subset S
#   part 2: two-dimensional partitions that are not products (Voronoi cells in random metrics, merged cells,
#           lattices, strips, rings), gamma = (1, eps^2); exact through the perimeter of the hull of each cell
#   part 3: head on grids, tail in shells of the dual norm (the design at low rate), n up to 48
# Run:  nice -n 10 taskset -c 6 env OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 check_schemes.py
import itertools
import math
import sys
import numpy as np
from scipy import stats
from scipy.spatial import ConvexHull
import floors_lib as fl

rng = np.random.default_rng(20261121)
E1 = math.sqrt(2 / math.pi)            # E|g_1|
E2 = math.sqrt(math.pi / 2)            # E|g_2|


def V_subset(gam, S, H):
    """Combined floor for an arbitrary head set S (indices into gam): min_u kappa_k GM_S e^{-(H-u)/k} + W_{S^c}(u)."""
    gam = np.asarray(gam, float)
    S = list(S)
    k = len(S)
    rest = np.delete(gam, S)
    if k == 0:
        return fl.W(gam, H)[0]
    lhead = fl.log_kappa(k) + 0.5 * np.log(gam[S]).sum() / k            # ln kappa_k + (1/2k) sum ln gam
    if len(rest) == 0:
        return math.exp(lhead - H / k)
    f = lambda u: math.exp(lhead - (H - u) / k) + fl.W(rest, u)[0]      # convex in u
    a, b = 0.0, H
    gr = (math.sqrt(5) - 1) / 2
    c, d = b - gr * (b - a), a + gr * (b - a)
    fc, fd = f(c), f(d)
    for _ in range(50):
        if fc < fd:
            b, d, fd = d, c, fc
            c = b - gr * (b - a); fc = f(c)
        else:
            a, c, fc = c, d, fd
            d = a + gr * (b - a); fd = f(d)
    return min(f(0.0), f(H), fc, fd)    # an evaluated point, so >= the true minimum: the stricter test


# Note: the floor is the exact minimum over u, which is at most the value returned, so comparing a code with the
# value returned is the stricter test (a code that passes it passes the floor).

def all_subsets(n):
    for k in range(n + 1):
        yield from itertools.combinations(range(n), k)


def best_floor(gam, H):
    best, arg = -1, None
    for S in all_subsets(len(gam)):
        v = V_subset(gam, S, H)
        if v > best:
            best, arg = v, S
    return best, arg


# ---------------------------------------------------------------- part 1: product codes
def interval_code(kind):
    """A partition of the line into bounded intervals. Returns (mean half-length weighted by mass, entropy)."""
    if kind == 0:                         # uniform grid, random step and offset
        w = math.exp(rng.uniform(-3.5, 2.3)); off = rng.uniform(0, 1)
        e = (np.arange(-int(14 / w) - 3, int(14 / w) + 4) + off) * w
    elif kind == 1:                       # random breakpoints
        m = int(rng.integers(1, 30)); e = np.sort(np.concatenate([[-13.0, 13.0], rng.normal(0, rng.uniform(0.3, 3), m)]))
        e = e[(e >= -13) & (e <= 13)]
    else:                                 # one wide central cell and geometric cells outside (a low-rate code)
        L = rng.uniform(0.5, 4.5); q = rng.uniform(1.05, 2.0); out = [L]
        while out[-1] < 13: out.append(out[-1] * q)
        e = np.array(sorted([-x for x in out] + out))
    p = np.diff(stats.norm.cdf(e)); ln = np.diff(e); ok = p > 0
    p = p / p.sum()
    return float((p * ln / 2).sum()), float(-(p[ok] * np.log(p[ok])).sum())


def part1(trials=2500):
    worst = (9e9, None); worst2 = (9e9, None); worstW = 9e9
    for t in range(trials):
        n = int(rng.integers(1, 6))
        gam = np.sort(np.exp(rng.normal(0, rng.uniform(0.2, 2.5), n)))[::-1]
        Eh, H = 0.0, 0.0
        for i in range(n):
            hl, Hi = interval_code(int(rng.integers(0, 3)))
            Eh += math.sqrt(gam[i]) * E1 * hl; H += Hi
        fb, S = best_floor(gam, H)
        r = Eh / fb; rW = Eh / fl.W(gam, H)[0]
        worstW = min(worstW, rW)
        if r < worst[0]: worst = (r, (n, S, H))
        if n >= 2 and r < worst2[0]: worst2 = (r, (n, S, round(H, 3)))
    print("part 1: %d product codes, n <= 5, all subsets S: smallest E h / max_S V_S = %.6f at (n, S, H) = %s; "
          "smallest with n >= 2: %.6f at %s; smallest E h / W = %.4f" % (trials, worst[0], worst[1], worst2[0], worst2[1], worstW))
    return worst[0]


# ---------------------------------------------------------------- part 2: planar partitions
G = 360
RMAX = 8.0
edges = np.linspace(-RMAX, RMAX, G + 1)
p1 = np.diff(stats.norm.cdf(edges)); p1 /= p1.sum()
PIX = np.outer(p1, p1)                                   # mass of pixel (i, j): z1 in cell i, z2 in cell j
ctr = (edges[1:] + edges[:-1]) / 2
Z1, Z2 = np.meshgrid(ctr, ctr, indexing="ij")
RAD = np.hypot(Z1, Z2)


def planar_Eh_H(lab, eps):
    """lab: integer labels on the pixel grid. Exact E h (gamma = (1, eps^2)) and H of that partition."""
    Eh, H = 0.0, 0.0
    for c in np.unique(lab):
        m = lab == c
        P = PIX[m].sum()
        if P <= 0:
            continue
        rows = np.where(m.any(1))[0]
        jmin = m[rows].argmax(1); jmax = G - 1 - m[rows][:, ::-1].argmax(1)
        pts = np.concatenate([np.stack([edges[rows], edges[jmin]], 1), np.stack([edges[rows + 1], edges[jmin]], 1),
                              np.stack([edges[rows], edges[jmax + 1]], 1), np.stack([edges[rows + 1], edges[jmax + 1]], 1)])
        pts = pts * np.array([1.0, eps])                 # the cell in the coordinates of r'
        per = ConvexHull(pts).area                       # in the plane .area is the perimeter
        Eh += P * E2 * per / (2 * math.pi)
        H -= P * math.log(P)
    return Eh, H


def rings(lab, R0, nring, nxt):
    """Cut the region beyond radius R0 into nring rings (the outermost takes the corners)."""
    rs = np.linspace(R0, RMAX, nring + 1)
    for a in range(nring):
        m = (RAD > rs[a]) & ((RAD <= rs[a + 1]) | (a == nring - 1))
        lab[m] = nxt + a
    return lab


def make_partition(kind, eps):
    lab = np.zeros((G, G), int)
    R0 = rng.uniform(1.5, 5.0); nring = int(rng.integers(1, 7))
    if kind == "voronoi" or kind == "merged":
        m = int(rng.integers(2, 70)); sc = rng.uniform(0.5, 2.5)
        sites = rng.normal(0, sc, (m, 2)); th = rng.uniform(0, math.pi); st = math.exp(rng.uniform(-1.5, 1.5))
        if rng.random() < 0.5: st = 1 / max(eps, 0.05) ** rng.uniform(0, 1)       # stretch the cells along the weak axis
            # (cells long in z2 are what a good code for gamma = (1, eps^2) uses)
        if rng.random() < 0.5: th = 0.0
        A = np.array([[math.cos(th), -math.sin(th)], [math.sin(th), math.cos(th)]]) @ np.diag([1.0, 1.0 / st])
        P = np.stack([Z1.ravel(), Z2.ravel()], 1) @ A.T; S = sites @ A.T
        d = ((P[:, None, :] - S[None]) ** 2).sum(2)
        lab = d.argmin(1).reshape(G, G)
        if kind == "merged":
            lab = rng.integers(0, max(2, m // 3), m)[lab]
        lab = rings(lab, R0, nring, lab.max() + 1)
    elif kind == "lattice":
        w1 = math.exp(rng.uniform(-2.2, 1.2)); w2 = w1 * math.exp(rng.uniform(-0.5, 0.5)) / max(eps, 0.03) ** rng.uniform(0, 1.2)
        shear = rng.uniform(0, 1) if rng.random() < 0.5 else 0.5                  # 0.5 with the right aspect is hexagonal-like
        j = np.floor(Z2 / w2 + rng.uniform(0, 1)).astype(int)
        i = np.floor(Z1 / w1 + shear * j + rng.uniform(0, 1)).astype(int)
        lab = (i - i.min()) * (j.max() - j.min() + 1) + (j - j.min())
        lab = rings(lab, R0, nring, lab.max() + 1)
    elif kind == "strips":                 # z1 on a grid, z2 only bounded: the tail left uncoded
        w1 = math.exp(rng.uniform(-2.5, 1.0)); L = rng.uniform(1.0, 4.5)
        i = np.floor(Z1 / w1 + rng.uniform(0, 1)).astype(int); i -= i.min()
        lab = np.where(np.abs(Z2) <= L, i, i.max() + 1 + (np.abs(Z2) > L) * (1 + np.floor((np.abs(Z2) - L) / rng.uniform(0.3, 2)).astype(int)))
        if rng.random() < 0.5: lab = rings(lab, max(R0, 3.0), nring, lab.max() + 1)
    else:                                  # "shells": the stored dual norm sqrt(z1^2 + eps z2^2) on a grid, nothing else
        rho = np.sqrt(Z1 ** 2 + eps * Z2 ** 2); q = rng.uniform(1.02, 1.8); r0 = rng.uniform(0.2, 2.5)
        lab = np.where(rho <= r0, 0, 1 + np.floor(np.log(np.maximum(rho, r0) / r0) / math.log(q)).astype(int))
    return lab


def part2(per_kind=140):
    worst_all = 9e9
    for eps in (1.0, 0.5, 0.2, 0.05):
        gam = np.array([1.0, eps * eps])
        for kind in ("voronoi", "merged", "lattice", "strips", "shells"):
            worst = (9e9, None); wW = 9e9; wT1 = 9e9
            for t in range(per_kind):
                lab = make_partition(kind, eps)
                Eh, H = planar_Eh_H(lab, eps)
                fb, S = best_floor(gam, H)
                r = Eh / fb
                wW = min(wW, Eh / fl.W(gam, H)[0]); wT1 = min(wT1, Eh / fl.T1(gam, H)[0])
                if r < worst[0]: worst = (r, (S, round(H, 3), round(Eh, 4)))
            worst_all = min(worst_all, worst[0])
            print("part 2: eps = %-4g %-8s %3d partitions: min E h / max_S V_S = %.4f at (S, H, E h) = %s; min / W = %.4f; min / Theorem 1 = %.4f"
                  % (eps, kind, per_kind, worst[0], worst[1], wW, wT1))
            sys.stdout.flush()
    return worst_all


# ---------------------------------------------------------------- part 3: grids on the head, dual-norm shells on the tail
def part3(trials=300, NS=400000):
    worst = (9e9, None); wW = 9e9
    for t in range(trials):
        n = int(rng.choice([8, 16, 32, 48])); kh = int(rng.integers(0, 4))
        gam = np.sort(np.exp(-rng.uniform(0.02, 0.35) * np.arange(n) ** rng.uniform(0.6, 1.2)) * math.exp(rng.normal(0, 0.3)))[::-1]
        sq = np.sqrt(gam)
        Eh, H = 0.0, 0.0
        for i in range(kh):                               # head coordinates on uniform grids
            w = math.exp(rng.uniform(-3, 0.5)); off = rng.uniform(0, 1)
            e = (np.arange(-int(14 / w) - 3, int(14 / w) + 4) + off) * w
            p = np.diff(stats.norm.cdf(e)); p = p[p > 0]; p /= p.sum()
            Eh += sq[i] * E1 * w / 2; H -= (p * np.log(p)).sum()
        wt = np.sqrt(sq[kh:])                             # the dual norm of the tail: rho^2 = sum sqrt(gam_i) z_i^2
        z = rng.standard_normal((NS, n - kh)); rho = np.sqrt(((z * wt) ** 2).sum(1))
        gn = float(np.sqrt(((rng.standard_normal((NS, n - kh)) * wt) ** 2).sum(1)).mean())   # E |g''_T|
        q = rng.uniform(1.01, 1.6); r0 = float(np.quantile(rho, rng.uniform(0.001, 0.9)))
        idx = np.where(rho <= r0, 0, 1 + np.floor(np.log(np.maximum(rho, r0) / r0) / math.log(q)).astype(int))
        cnt = np.bincount(idx); P = cnt / NS; top = r0 * q ** np.arange(len(cnt))
        ok = P > 0
        Eh += gn * float((P * top).sum()); H -= float((P[ok] * np.log(P[ok])).sum())
        sp = fl.Spectrum(gam)
        v, k, u = sp.V(H)
        r = Eh / v; wW = min(wW, Eh / sp.W_suffix(0, H))
        if r < worst[0]: worst = (r, (n, kh, round(H, 3), k))
    print("part 3: %d codes (grids on a head of 0 to 3 coordinates, shells of the dual norm on the tail, n = 8..48; shell masses "
          "from %d samples): min E h / max_k V_top-k = %.4f at (n, head, H, k) = %s; min E h / W = %.4f" % (trials, NS, worst[0], worst[1], wW))
    return worst[0]


if __name__ == "__main__":
    which = sys.argv[1:] or ["1", "2", "3"]
    res = []
    if "1" in which: res.append(part1())
    sys.stdout.flush()
    if "2" in which: res.append(part2())
    sys.stdout.flush()
    if "3" in which: res.append(part3())
    print("smallest ratio over everything run: %.4f; a value below 1 would refute the floor" % min(res))
