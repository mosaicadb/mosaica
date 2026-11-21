# Targeted attack on F2 (and F1 = W) where F2 exceeds both W and Theorem 1 the most: a steep head of m equal eigenvalues 1
# over a long flat tail of N eigenvalues eps. Codes: the head by ECSQ (m = 1, 2, 3; offset 0.5) or by the hexagonal lattice
# (m = 2); the tail by its whitened norm on a uniform radial grid (step D, origin offset o), which is the only kind of tail
# code that gets the tail's width below sum sqrt(gam_tail). Everything is exact (closed forms, chi2 CDF, polygon quadrature).
# The steps are optimised continuously (Nelder-Mead on E h / bound) for each spectrum, against F2 with S = the head (the
# case the claim singles out), against max over S (= the claim's full strength), and against W.
import math, warnings
import numpy as np
from scipy import optimize, stats, special
from common import Phi, E_norm, Eg, logkappa, log_mass, WFloor, standard_run_header

warnings.filterwarnings("ignore")
standard_run_header("f12_target.py")
SQ2PI = math.sqrt(2 * math.pi)


def ent(P):
    P = P[P > 0]
    return float(-(P * np.log(P)).sum())


def ecsq(w, off=0.5):
    """entropy of ECSQ with step w >= 0.002: cell masses by log-survival differences on the far side of 0 (Lb - La is at
    least ~w there, so -expm1 loses nothing); avoids log_mass's Gauss-Legendre arrays."""
    j = np.arange(-int(40 / w) - 2, int(40 / w) + 3)
    e = (j + off) * w
    a, b = e[:-1], e[1:]
    flip = (a + b) < 0
    a2, b2 = np.where(flip, -b, a), np.where(flip, -a, b)
    La, Lb = special.log_ndtr(-a2), special.log_ndtr(-b2)
    return ent(np.exp(La + np.log(-np.expm1(Lb - La))))


def tail_norm(N, D, o):
    """shells [o + jD, o + (j+1)D) of ||z_tail|| (first cell [0, o + D)); returns (E[rho_upper], H)."""
    rmax = math.sqrt(stats.chi2.isf(1e-18, N))
    # the first edge o + D is always a cell boundary, and edges continue past rmax (an earlier version dropped every
    # edge when o + D > rmax, leaving the tail with zero width)
    e = np.concatenate([[0.0], o + D + D * np.arange(0, max(1, int(math.ceil((rmax - o - D) / D)) + 2))])
    F = np.concatenate([[0.0], stats.chi2.cdf(e[1:] ** 2, N)])      # mass beyond the last edge < 1e-18
    P = np.diff(F)
    up = e[1:]
    return float((P * up).sum()), ent(P)


class F2B:
    def __init__(self, gam):
        self.g = np.sort(gam)[::-1]; n = len(self.g)
        self.W = WFloor(self.g)
        self.lgc = np.cumsum(np.log(self.g))
        self.cache = {}

    def k(self, H, k):
        if k not in self.cache:
            self.cache[k] = WFloor(self.g[k:]) if k < len(self.g) else None
        Wt = self.cache[k]; lk = logkappa(k) + 0.5 * self.lgc[k - 1] / k
        f = lambda u: math.exp(lk - (H - u) / k) + (Wt(u) if Wt is not None else 0.0)
        r = optimize.minimize_scalar(f, bounds=(0, H), method="bounded", options={"xatol": 1e-12 * max(1, H)})
        return min(r.fun, f(0), f(H))

    def best(self, H):
        n = len(self.g)
        ks = sorted(set(list(range(1, min(n, 8) + 1)) + [2 ** j for j in range(14) if 2 ** j <= n] + [n]))
        vals = [(self.k(H, k), k) for k in ks]
        return max(vals)


hexV = np.array([[math.cos(math.pi / 6 + j * math.pi / 3), math.sin(math.pi / 6 + j * math.pi / 3)] for j in range(6)]) / math.sqrt(3)
hexB = np.array([[1.0, 0.0], [0.5, math.sqrt(3) / 2]])
xg, wg = np.polynomial.legendre.leggauss(24)
def poly_P(V):
    xs = np.unique(V[:, 0]); tot = 0.0; E = np.stack([V, np.roll(V, -1, 0)], 1)
    for x0, x1 in zip(xs[:-1], xs[1:]):
        if x1 - x0 < 1e-15: continue
        x = 0.5 * (x1 - x0) * xg + 0.5 * (x1 + x0); ys = []
        for (p, q) in E:
            if min(p[0], q[0]) <= x0 + 1e-15 and max(p[0], q[0]) >= x1 - 1e-15 and abs(q[0] - p[0]) > 1e-15:
                ys.append(p[1] + (x - p[0]) * (q[1] - p[1]) / (q[0] - p[0]))
        if len(ys) < 2: continue
        ys = np.array(ys); tot += 0.5 * (x1 - x0) * (wg * stats.norm.pdf(x) * np.exp(log_mass(ys.min(0), ys.max(0)))).sum()
    return tot
_hexcache = {}
def hex_head(s):
    """hexagonal lattice of scale s on the 2 head coordinates (gam = 1): (E h, H)."""
    s = float(np.round(s, 6))
    if s not in _hexcache:
        V = hexV * s; Bm = hexB * s
        per = 6 * s / math.sqrt(3)
        J = int(10 / (0.5 * s)) + 2
        ii, jj = np.meshgrid(np.arange(-J, J + 1), np.arange(-J, J + 1))
        pts = ii.ravel()[:, None] * Bm[0] + jj.ravel()[:, None] * Bm[1]
        pts = pts[np.linalg.norm(pts, axis=1) < 9.5 + s]
        Ps = np.array([poly_P(V + p) for p in pts])
        _hexcache[s] = (per / (2 * SQ2PI), ent(Ps))
    return _hexcache[s]


print("spectrum [1]*m + [eps]*N; head code; for each target the optimised code (w head step, D tail step, o tail offset)")
print("m  N     eps    head   target     E h/target   H (nats)   E h        F2(S=head)  F2(max S, k)    W          code parameters")
overall = {}
import sys
MS = [int(a) for a in sys.argv[1:] if not a.startswith('--')] or [1, 2, 3]
ONLY = [a[7:] for a in sys.argv[1:] if a.startswith('--only=')]
HEX_S = np.geomspace(0.25, 4.0, 12)
for m in MS:
    for N in (20, 99, 400, 2000):
        for eps in (1e-2, 1e-4, 1e-6):
            gam = np.array([1.0] * m + [eps] * N)
            B = F2B(gam); n = m + N
            Etail_unit = math.sqrt(eps) * Eg(N)
            heads = ["ECSQ"] + (["hex"] if m == 2 else [])
            for head in heads:
                def code(p):
                    w = math.exp(p[0]); D = math.exp(p[1]); o = p[2]
                    if not (2e-3 < w < 20 and 0.01 < D < 50 and 0 <= o < 2 * math.sqrt(N) + 10): return None   # w >= 0.002 and D >= 0.01 keep the quadrature arrays small (under ~10 MB)
                    if head == "ECSQ":
                        Eh_h, H_h = m * w / SQ2PI, m * ecsq(w)
                    else:
                        Eh_h, H_h = hex_head(w)
                    Er, H_t = tail_norm(N, D, o)
                    return Eh_h + Er * Etail_unit, H_h + H_t
                for tname in (("F2(S=head)", "F2(max S)", "W") if head == "ECSQ" else ("F2(S=head)", "F2(max S)")):
                    def obj(p):
                        c = code(p)
                        if c is None: return 1e9
                        Eh, H = c
                        if tname == "F2(S=head)": b = B.k(H, m)
                        elif tname == "F2(max S)": b = B.best(H)[0]
                        else: b = B.W(H)
                        return Eh / b
                    best = (1e9, None)
                    if head == "hex":
                        # the hexagon is precomputed on a grid of scales; only the tail (D, o) is optimised continuously
                        for s_ in HEX_S:
                            hex_head(s_)
                            obj2 = lambda q, s_=s_: obj([math.log(s_), q[0], q[1]])
                            r = optimize.minimize(obj2, [math.log(2 / math.sqrt(N)), max(math.sqrt(N) - 1.5, 0.0)], method="Nelder-Mead",
                                                  options={"xatol": 1e-6, "fatol": 1e-10, "maxiter": 150})
                            if r.fun < best[0]: best = (r.fun, np.array([math.log(s_), r.x[0], r.x[1]]))
                    starts = [] if head == "hex" else [
                        [math.log(0.5), math.log(2 / math.sqrt(N)), math.sqrt(N) - 1.5], [math.log(0.1), math.log(0.5 / math.sqrt(N)), math.sqrt(N) - 2],
                        [math.log(2.0), math.log(1.0), math.sqrt(N) - 1], [math.log(0.02), math.log(4 / math.sqrt(N)), math.sqrt(N) - 3]]
                    for x0 in starts:
                        x0[2] = max(x0[2], 0.0)
                        r = optimize.minimize(obj, x0, method="Nelder-Mead", options={"xatol": 1e-7, "fatol": 1e-11, "maxiter": 800})
                        if r.fun < best[0]: best = (r.fun, r.x)
                    Eh, H = code(best[1])
                    f2h = B.k(H, m); f2b, kb = B.best(H); wv = B.W(H)
                    key = (tname, head)
                    if key not in overall or best[0] < overall[key][0]:
                        overall[key] = (best[0], m, N, eps, H)
                    print("%d %5d %7.0e  %-5s  %-10s  %.6f    %8.3f   %.6g  %.6g  %.6g (%d)  %.6g  w=%.4g D=%.4g o=%.4g" % (
                        m, N, eps, head, tname, best[0], H, Eh, f2h, f2b, kb, wv, math.exp(best[1][0]), math.exp(best[1][1]), best[1][2]))
print("\nSmallest E h / bound found (below 1 would refute):")
for key, v in sorted(overall.items()):
    print("   %-10s head %-5s: %.6f  (m=%d, N=%d, eps=%g, H=%.3f nats)" % (key[0], key[1], *v))
