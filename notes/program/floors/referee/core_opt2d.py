# Referee attack on F1 and F2 in two dimensions: a local search over cell shapes.
# A code is a labelling of small squares of the z-plane (z standard Gaussian).  Masses are exact (products of
# normal cdf differences); the smallest sound certificate of a cell is the support function of its hull, whose
# Gaussian mean is perimeter(hull of Gamma^(1/2) cell) / (2 sqrt(2 pi)).  So E h and H(c) are exact for the code
# (up to mass 1e-30 beyond |z| = 12).  The search moves one square at a time between neighbouring cells and keeps
# a move when E h / floor(H) decreases, floor = max over S of V_S (F2; S = {} is F1's W).
# usage: python3 core_opt2d.py eps start param1 param2 iters seed
import math
import sys
import numpy as np
from scipy.special import ndtr

SQE = math.sqrt(math.e)
K2 = math.sqrt(math.pi * math.e)          # kappa_2
MW = 1.0 / (2 * math.sqrt(2 * math.pi))   # mean width factor: E_g h_K(g) = MW * perimeter(conv K) in the plane


def floors(H, g1, g2):
    """n = 2, sqrt-eigenvalues g1 >= g2.  Returns (W, V_{1}, V_{2}, V_{12})."""
    if H <= math.log(g1 / g2):
        W = g1 * math.exp(-H) + g2
    else:
        W = 2 * math.sqrt(g1 * g2) * math.exp(-H / 2)

    def one(head, tail):
        A = SQE * head
        u = 0.5 * (H + math.log(tail / A))
        u = min(max(u, 0.0), H)
        return A * math.exp(-(H - u)) + tail * math.exp(-u)

    return W, one(g1, g2), one(g2, g1), K2 * math.sqrt(g1 * g2) * math.exp(-H / 2)


def lower_chain_len(xs, ys):
    """Length of the lower convex chain of points sorted by x (ties allowed)."""
    hx, hy = [], []
    for x, y in zip(xs, ys):
        while len(hx) >= 2 and (hx[-1] - hx[-2]) * (y - hy[-2]) - (hy[-1] - hy[-2]) * (x - hx[-2]) <= 0:
            hx.pop(); hy.pop()
        if len(hx) == 1 and hx[0] == x:
            if y < hy[0]:
                hy[0] = y
            continue
        hx.append(x); hy.append(y)
    return sum(math.hypot(hx[i + 1] - hx[i], hy[i + 1] - hy[i]) for i in range(len(hx) - 1)), hy[0], hy[-1]


def perimeter(mask, x_edges, y_edges):
    """Perimeter of the convex hull of the union of grid rectangles in mask (axis 0 = x)."""
    rows = np.flatnonzero(mask.any(1))
    if rows.size == 0:
        return 0.0
    sub = mask[rows]
    jmin = sub.argmax(1)
    jmax = sub.shape[1] - 1 - sub[:, ::-1].argmax(1)
    xs = np.empty(2 * rows.size)
    xs[0::2] = x_edges[rows]
    xs[1::2] = x_edges[rows + 1]
    lo = np.repeat(y_edges[jmin], 2)
    hi = np.repeat(y_edges[jmax + 1], 2)
    l1, a0, a1 = lower_chain_len(xs.tolist(), lo.tolist())
    l2, b0, b1 = lower_chain_len(xs.tolist(), (-hi).tolist())
    return l1 + l2 + (-b0 - a0) + (-b1 - a1)


def start_labels(kind, x, y, p1, p2, rng):
    """x, y: square centres of the active block."""
    X, Y = np.meshgrid(x, y, indexing="ij")
    if kind == "grid":                       # product grid with steps p1, p2
        return (np.floor(X / p1 + 0.5) * 1000 + np.floor(Y / p2 + 0.5)).astype(int)
    if kind == "strips":                     # strips of width p1 in z1 over |z2| <= p2, caps beyond
        s = np.floor(X / p1 + 0.5)
        cap = np.where(Y > p2, 1, np.where(Y < -p2, 2, 0))
        return (s * 10 + cap).astype(int)
    if kind == "hex":                        # hexagonal lattice, scale p1 in z1 and p2 in z2
        a = np.array([[p1, 0.0], [0.5 * p1, p2]])
        best = None
        lab = None
        for di in range(-1, 2):
            for dj in range(-1, 2):
                j = np.round(Y / p2) + dj
                i = np.round((X - 0.5 * p1 * j) / p1) + di
                cx = i * p1 + 0.5 * p1 * j
                cy = j * p2
                d = ((X - cx) / p1) ** 2 + ((Y - cy) / (p2 * 2 / math.sqrt(3))) ** 2
                if best is None:
                    best, lab = d, (i * 1000 + j).astype(int)
                else:
                    m = d < best
                    best = np.where(m, d, best)
                    lab = np.where(m, (i * 1000 + j).astype(int), lab)
        return lab
    if kind == "voronoi":                    # p1 sites from N(0, I), metric diag(1, p2)
        k = int(p1)
        s = rng.normal(0, 1.2, (k, 2))
        d = (X[..., None] - s[:, 0]) ** 2 + p2 * (Y[..., None] - s[:, 1]) ** 2
        return d.argmin(-1)
    raise SystemExit("unknown start")


def run(eps, kind, p1, p2, iters, seed, L=5.0, nfine=64, verbose=True):
    rng = np.random.default_rng(seed)
    g1, g2 = 1.0, eps
    e = np.linspace(-L, L, nfine + 1)
    pm = np.diff(ndtr(e))
    mass = np.outer(pm, pm)
    x = 0.5 * (e[:-1] + e[1:])
    lab = start_labels(kind, x, x, p1, p2, rng)
    # relabel to 0..K-1
    _, inv = np.unique(lab, return_inverse=True)
    lab = inv.reshape(lab.shape)
    K = lab.max() + 1
    xe, ye = g1 * e, g2 * e
    # outer unit squares, each its own cell
    oe = np.arange(-12, 13).astype(float)
    om = np.diff(ndtr(oe))
    oc = 0.5 * (oe[:-1] + oe[1:])
    OX, OY = np.meshgrid(oc, oc, indexing="ij")
    out = (np.abs(OX) > L) | (np.abs(OY) > L)
    OM = np.outer(om, om)[out]
    OM = OM[OM > 0]
    out_Eh = float(OM.sum() * MW * 2 * (g1 + g2))
    out_H = float(-(OM * np.log(OM)).sum())
    P = np.bincount(lab.ravel(), weights=mass.ravel(), minlength=K)
    per = np.array([perimeter(lab == c, xe, ye) for c in range(K)])

    def totals(P, per):
        m = P > 0
        return out_Eh + MW * float((P[m] * per[m]).sum()), out_H - float((P[m] * np.log(P[m])).sum())

    def ratio(Eh, H):
        fl = floors(H, g1, g2)
        return Eh / max(fl), fl

    Eh, H = totals(P, per)
    r, fl = ratio(Eh, H)
    r0, H0, Eh0 = r, H, Eh
    acc = 0
    for it in range(iters):
        i, j = rng.integers(0, nfine, 2)
        d = rng.integers(0, 4)
        i2 = i + (1, -1, 0, 0)[d]
        j2 = j + (0, 0, 1, -1)[d]
        if not (0 <= i2 < nfine and 0 <= j2 < nfine):
            continue
        a, b = lab[i, j], lab[i2, j2]
        if a == b:
            continue
        lab[i, j] = b
        pa, pb = perimeter(lab == a, xe, ye), perimeter(lab == b, xe, ye)
        Pn = P.copy(); pn = per.copy()
        Pn[a] -= mass[i, j]; Pn[b] += mass[i, j]
        if not (lab == a).any():
            Pn[a] = 0.0
        pn[a], pn[b] = pa, pb
        Eh2, H2 = totals(Pn, pn)
        r2, fl2 = ratio(Eh2, H2)
        if r2 < r:
            P, per, Eh, H, r, fl = Pn, pn, Eh2, H2, r2, fl2
            acc += 1
        else:
            lab[i, j] = a
    # recompute from scratch as a check of the bookkeeping
    Kc = lab.max() + 1
    P = np.bincount(lab.ravel(), weights=mass.ravel(), minlength=Kc)
    per = np.array([perimeter(lab == c, xe, ye) for c in range(Kc)])
    Eh_c, H_c = totals(P, per)
    assert abs(Eh_c - Eh) < 1e-9 and abs(H_c - H) < 1e-9, (Eh_c, Eh, H_c, H)
    W, V1, V2, V12 = fl
    ncell = int((P > 1e-12).sum())
    if verbose:
        print("eps=%.3g start=%s(%g,%g) iters=%d seed=%d | start: H=%.3f Eh=%.4f ratio=%.4f | end: cells=%d H=%.3f "
              "Eh=%.4f W=%.4f V1=%.4f V2=%.4f V12=%.4f  Eh/W=%.4f  Eh/maxV=%.4f  accepted=%d"
              % (eps, kind, p1, p2, iters, seed, H0, Eh0, r0, ncell, H, Eh, W, V1, V2, V12, Eh / W, r, acc))
        sys.stdout.flush()
    return r, Eh / W


if __name__ == "__main__":
    a = sys.argv
    run(float(a[1]), a[2], float(a[3]), float(a[4]), int(a[5]), int(a[6]))
