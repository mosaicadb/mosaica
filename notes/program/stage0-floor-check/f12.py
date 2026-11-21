# The PI's two stronger claims, attacked with whole encoders (statements only; program/floors/ not read).
#   F1: E h >= W(H),  W(H) = sum_i sqrt(min(gam_i, theta)), sum_i (1/2) ln+(gam_i/theta) = H (nats). Averaged, not per cell.
#   F2: for every set S of k eigen-indices,
#       E h >= min_{0<=u<=H} [ kappa_k (prod_S gam)^(1/2k) e^{-(H-u)/k} + W_{S^c}(u) ].
# E h is the exact mean half-width: the mean over cells and Gaussian queries of the support function of the cell's hull,
# the smallest sound certificate. Coordinates are whitened: z ~ N(0, I) in the eigenbasis, K_c = Gam^{1/2} A_c.
# Every E h and H below is exact (closed form or 1-D quadrature) except the D4 family (Monte Carlo, with s.e.).
# Families: ECSQ on the head + tail bounded by a norm, a weighted norm, or coarse ECSQ (tail uncoded gives E h = inf);
# norm-only codes (shell index); sign + norm codes (orthant x shell, non-product cells); 2-D lattices (hexagonal and square,
# rotated and sheared so cells mix head and tail coordinates); the D4 lattice in 4-D (24-cell Voronoi cells).
import math, json, sys, warnings
import numpy as np
from scipy import stats, optimize, integrate, special
from common import Phi, E_norm, T1, conj_floor, logkappa, log_mass, WFloor, standard_run_header
from gaussq import log_cdf

warnings.filterwarnings("ignore")
standard_run_header("f12.py")
rng = np.random.default_rng(1212)
SQ2PI = math.sqrt(2 * math.pi)
FAST = "--fast" in sys.argv


class Bounds:
    """W, F2 (max over S = top-k for k in a set, plus the k at the water level) and Phi for one spectrum, with cached W's."""
    def __init__(self, gam):
        self.g = np.sort(np.asarray(gam, float))[::-1]
        n = len(self.g)
        self.W = WFloor(self.g)
        self.ks = sorted(set(range(1, min(n, 12) + 1)) | {2 ** j for j in range(13) if 2 ** j <= n} | {n})
        self.Wtail = {k: (WFloor(self.g[k:]) if k < n else None) for k in range(1, n + 1)}
        lgc = np.cumsum(np.log(self.g))
        self.head = {k: logkappa(k) + 0.5 * lgc[k - 1] / k for k in range(1, n + 1)}

    def F2_k(self, H, k):
        lk, Wt = self.head[k], self.Wtail[k]
        f = lambda u: math.exp(lk - (H - u) / k) + (Wt(u) if Wt is not None else 0.0)
        if H <= 0:
            return f(0.0)
        r = optimize.minimize_scalar(f, bounds=(0.0, H), method="bounded", options={"xatol": 1e-12 * max(H, 1)})
        return min(r.fun, f(0.0), f(H))      # f is convex in u (both terms are), so this is the minimum

    def F2(self, H):
        n = len(self.g)
        kw = int((self.g > self.theta(H)).sum())
        ks = set(self.ks) | {k for k in (kw - 1, kw, kw + 1) if 1 <= k <= n}
        best, bk = -1.0, None
        for k in sorted(ks):
            v = self.F2_k(H, k)
            if v > best: best, bk = v, k
        return best, bk

    def theta(self, H):
        W = self.W
        if H <= 0: return W.g[0]
        lt = (W.L - 2 * H) / W.ks
        nxt = np.concatenate([W.lg[1:], [-np.inf]])
        k = int(np.argmax((lt <= W.lg + 1e-12) & (lt >= nxt - 1e-12)))
        return math.exp(lt[k])


_BCACHE = {}
def bounds_for(gname, gam):
    if gname not in _BCACHE:
        _BCACHE[gname] = Bounds(gam)
    return _BCACHE[gname]


rec = {}


def report(fam, gname, n, Eh, H, gam, desc, err=0.0, show=False):
    B = bounds_for(gname, gam)
    w = B.W(H); f2, k = B.F2(H); ph = Phi(gam, H)
    rw, rf, rp = Eh / w, Eh / f2, Eh / ph
    for key, r in (((fam, "W"), rw), ((fam, "F2"), rf), ((fam, "Phi"), rp)):
        if key not in rec or r < rec[key][0]:
            rec[key] = (r, err / Eh * r if Eh > 0 else 0, gname, n, H, desc + (f", F2 best k={k}" if key[1] == "F2" else ""))
    if show:
        print("    %-14s n=%3d  H=%8.3f nats  E h=%.5g  W=%.5g  F2=%.5g (k=%d)  Phi=%.5g  | E h/W=%.4f  E h/F2=%.4f  E h/Phi=%.4f  %s" % (
            gname, n, H, Eh, w, f2, k, ph, rw, rf, rp, desc))
    return rw, rf


def show_best(fam, extra=""):
    for b in ("W", "F2", "Phi"):
        if (fam, b) in rec:
            r = rec[(fam, b)]
            print("   smallest E h/%-3s = %.5f %s (%s, n=%d, H=%.3f nats, %s)" % (b, r[0], ("+- %.5f" % r[1]) if r[1] else "", r[2], r[3], r[4], r[5]))


def spectra():
    out = {}
    for n in (2, 5, 20, 100):
        out[f"flat{n}"] = np.ones(n)
    for m, N, eps in ((1, 19, 1e-2), (1, 99, 1e-4), (2, 18, 1e-2), (4, 96, 1e-3), (8, 92, 1e-2), (1, 4, 1e-2)):
        out[f"{m}x1+{N}x{eps:g}"] = np.array([1.0] * m + [eps] * N)
    for n in (20, 100):
        i = np.arange(1, n + 1.0)
        out[f"i^-2 n{n}"] = 1 / i ** 2
        out[f"i^-1 n{n}"] = 1 / i
    spec = json.load(open("../../../agent1/spec2_n128.json"))
    g = np.sort(np.array(spec["5"]["lam"]) * np.array(spec["5"]["Qe"]))[::-1]
    out["measured n128 g5"] = g / g[0]
    return out


def ecsq_P(w, off):
    j = np.arange(-int(40 / w) - 2, int(40 / w) + 3)
    e = (j + off) * w
    lp = log_mass(e[:-1], e[1:])
    return np.exp(lp)


def ent(P):
    P = P[P > 0]
    return float(-(P * np.log(P)).sum())


def shell_P(lam_m2, edges):
    """probabilities of ||M z||^2 in [edges_j^2, edges_{j+1}^2), M^2 = diag(lam_m2); last edge = inf."""
    n = len(lam_m2)
    if np.allclose(lam_m2, lam_m2[0]):
        F = stats.chi2.cdf(np.asarray(edges[1:-1]) ** 2 / lam_m2[0], n)
    else:
        F = np.array([math.exp(log_cdf(r * r, lam_m2)[0]) for r in edges[1:-1]])
    F = np.concatenate([[0.0], F, [1.0]])
    return np.maximum(np.diff(F), 0)


def radial_grid(kind, par, lam_m2, n):
    rmax = math.sqrt(lam_m2.max() * stats.chi2.isf(1e-17, n))
    if kind == "geo":
        q, r1 = par
        J = int(math.ceil(math.log(max(rmax / r1, 1.0001)) / math.log(q))) + 1
        e = np.concatenate([[0.0], r1 * q ** np.arange(J)])
    else:
        d = par
        e = np.arange(0, rmax + d, d)
    return np.concatenate([e, [np.inf]])


def norm_code(gam, m, kind, par):
    """shells of ||M z||, M = diag(m): E h = E[rho_{j+1}] E||M^-1 Gam^1/2 g||, H = entropy of the shell index."""
    n = len(gam)
    edges = radial_grid(kind, par, m * m, n)
    P = shell_P(m * m, edges)
    hi = edges[1:].copy(); hi[-1] = edges[-2] * 2 if np.isfinite(edges[-2]) else 0   # last cell has mass < 1e-17
    return float((P * hi).sum()) * E_norm(gam / (m * m)), ent(P), P, edges


def E_norm_pos(c_full, c_pos):
    """E sqrt(sum_full X_i^2 + sum_pos (Y_i)_+^2), X_i ~ N(0, c_full_i), Y_i ~ N(0, c_pos_i), all independent.
    Uses sqrt(a) = (1/(2 sqrt pi)) int (1 - E e^{-ta}) t^{-3/2} dt; E e^{-t Y_+^2} = 1/2 + (1/2)(1 + 2 t c)^{-1/2}."""
    cf, cp = np.asarray(c_full, float), np.asarray(c_pos, float)
    sc = max(cf.max() if cf.size else 0, cp.max() if cp.size else 0)
    cf, cp = cf / sc, cp / sc
    def f(u):
        t = math.exp(u)
        lg = -0.5 * np.log1p(2 * t * cf).sum() + np.log(0.5 + 0.5 / np.sqrt(1 + 2 * t * cp)).sum()
        return -math.expm1(lg) * math.exp(-u / 2)
    v = integrate.quad(f, -60, 80, limit=600, epsabs=0, epsrel=1e-11)[0]
    return v / (2 * math.sqrt(math.pi)) * math.sqrt(sc)


print("\nChecks of the exact formulas:")
x = rng.standard_normal((100000, 5)) * np.sqrt([1, .5, .2, .1, .05])
v = np.sqrt((x[:, :2] ** 2).sum(1) + (np.maximum(x[:, 2:], 0) ** 2).sum(1))
print("  E_norm_pos vs MC: %.6f vs %.6f +- %.6f" % (E_norm_pos([1, .5], [.2, .1, .05]), v.mean(), v.std() / math.sqrt(len(v))))
Bc = Bounds(np.array([1, .3, .1]))
print("  W at H=0 = %.6f (sum sqrt gam %.6f); F2 with S = all at H=2: %.6f, Theorem 1 k=3 term %.6f" % (Bc.W(0), 1 + math.sqrt(.3) + math.sqrt(.1), Bc.F2_k(2.0, 3), math.exp(logkappa(3) + np.log([1, .3, .1]).sum() / 6 - 2.0 / 3)))

SP = spectra()
if FAST:
    SP = {k: SP[k] for k in list(SP)[:3]}

# ---------------- 0. ECSQ on every coordinate (step 3's first code), aligned with the eigenbasis or in a Haar basis ----------------
print("\n0. ECSQ on every coordinate: steps common or shaped w_i = min(w0 sqrt(gam_1/gam_i), 6); E h = sum_j w_j sqrt((R^T Gam R)_jj)/sqrt(2 pi)")
for gname, gam in SP.items():
    n = len(gam)
    for rot in ("aligned", "Haar"):
        if rot == "aligned":
            d = np.sqrt(gam)
        else:
            q_, r_ = np.linalg.qr(rng.standard_normal((n, n))); Rm = q_ * np.sign(np.diag(r_))
            d = np.sqrt(np.einsum("ij,i,ij->j", Rm, gam, Rm))
        for rule in ("common", "shaped"):
            for w0 in (4.0, 2.0, 1.0, 0.5, 0.25, 0.1, 0.03):
                for off in (0.5, 0.0):
                    w = np.full(n, w0) if rule == "common" else np.minimum(w0 * d.max() / d, 6.0)
                    H = sum(ent(ecsq_P(wi, off)) for wi in w)
                    report("0: ECSQ every coordinate", gname, n, (d * w).sum() / SQ2PI, H, gam, f"{rot}, {rule} w0={w0} off={off}",
                           show=(rot == "aligned" and rule == "common" and off == 0.5 and w0 in (2.0, 0.25) and gname in ("flat2", "flat100", "1x1+99x0.0001")))
show_best("0: ECSQ every coordinate")

# ---------------- A. ECSQ on the head, tail bounded ----------------
print("\nA. ECSQ (step w in whitened units) on the k leading coordinates; tail: (i) uncoded -> E h = inf, not evaluated;")
print("   (ii) whitened norm shells, (iii) dual-weighted norm shells M = gam^{1/4}, (iv) coarse midtread ECSQ per tail coordinate")
for gname, gam in SP.items():
    n = len(gam); a = np.sqrt(gam)
    ks = sorted(set([1, 2, 4, 8]) & set(range(1, n)))
    for k in ks:
        aT, gT = a[k:], gam[k:]
        tails = []
        for kind, par in (("geo", (2 ** (1 / 16), 0.8 * math.sqrt(n - k))), ("geo", (1.25, 0.6 * math.sqrt(n - k))), ("uni", 0.5), ("uni", 0.15)):
            Eh_t, H_t, _, _ = norm_code(gT, np.ones(n - k), kind, par)
            tails.append((f"tail whitened norm {kind}{par if kind == 'uni' else round(par[0], 4)}", Eh_t, H_t))
        m = gT ** 0.25; m = m / math.sqrt((m * m).mean())
        for kind, par in (("geo", (2 ** (1 / 16), 0.8 * math.sqrt(n - k))), ("geo", (1.25, 0.6 * math.sqrt(n - k)))):
            if FAST and kind == "geo" and par[0] < 1.1: continue
            Eh_t, H_t, _, _ = norm_code(gT, m, kind, par)
            tails.append((f"tail dual-weighted norm q={par[0]:.4f}", Eh_t, H_t))
        for Wt in (3.0, 4.0, 6.0):
            ctr = np.arange(-20, 21) * Wt
            P = np.exp(log_mass(ctr - Wt / 2, ctr + Wt / 2))          # midtread cells
            tails.append((f"tail midtread ECSQ step {Wt}", aT.sum() * Wt / SQ2PI, (n - k) * ent(P)))
        for w in (2.0, 1.0, 0.5, 0.2, 0.05):
            for off in ((0.5, 0.0) if w >= 1 else (0.5,)):
                Hh = k * ent(ecsq_P(w, off))
                Ehh = a[:k].sum() * w / SQ2PI
                for tname, Eh_t, H_t in tails:
                    report("A: ECSQ head + bounded tail", gname, n, Ehh + Eh_t, Hh + H_t, gam, f"k={k} w={w} off={off}; {tname}")
show_best("A: ECSQ head + bounded tail")

# ---------------- B. norm-only codes ----------------
print("\nB. norm only (message = shell index of ||M z||): M = I, gam^{1/4} (dual weights), gam^{-1/4}; geometric and uniform radial grids")
for gname, gam in SP.items():
    n = len(gam)
    for wname, m in (("I", np.ones(n)), ("gam^1/4", gam ** 0.25), ("gam^-1/4", gam ** -0.25)):
        m = m / math.sqrt((m * m).mean())
        for kind, par in (("geo", (2.0, 0.5 * math.sqrt(n))), ("geo", (1.25, 0.7 * math.sqrt(n))), ("geo", (2 ** (1 / 16), 0.8 * math.sqrt(n))),
                          ("uni", 2.0), ("uni", 1.0), ("uni", 0.5), ("uni", 0.2), ("uni", 0.05)):
            if FAST and kind == "uni" and par < 0.5: continue
            if wname != "I" and kind == "uni" and par < 0.2 and n > 20: continue
            Eh, H, _, _ = norm_code(gam, m, kind, par)
            report("B: norm only", gname, n, Eh, H, gam, f"M={wname}, grid {kind} {par}", show=(gname in ("flat100", "1x1+99x0.0001") and kind == "uni"))
show_best("B: norm only")

# ---------------- C. sign of the head (or all) + norm: orthant x shell cells ----------------
print("\nC. sign of the k leading coordinates + shell of ||M z|| (cells: orthant-part x shell, not products)")
print("   h = rho_{j+1} ||(M^-1 y) with head entries positive-parted||, y = Gam^1/2 g; when k = n and y is all negative the")
print("   support is -rho_j min_i |y_i|/m_i (included exactly); P = 2^-k P(shell)")
for gname, gam in SP.items():
    n = len(gam)
    for wname, m in (("I", np.ones(n)), ("gam^1/4", gam ** 0.25)):
        m = m / math.sqrt((m * m).mean())
        c = gam / (m * m)
        for k in sorted(set([1, 2, 4, n]) & set(range(1, n + 1))):
            Epos = E_norm_pos(c[k:], c[:k])
            for kind, par in (("geo", (1.25, 0.7 * math.sqrt(n))), ("geo", (2 ** (1 / 16), 0.8 * math.sqrt(n))), ("uni", 1.0), ("uni", 0.3)):
                if wname != "I" and kind == "uni" and n > 20: continue
                edges = radial_grid(kind, par, m * m, n)
                P = shell_P(m * m, edges)
                hi = edges[1:].copy(); hi[-1] = 0.0; lo = edges[:-1]
                Eh = float((P * hi).sum()) * Epos
                if k == n:
                    # all-negative correction: E[min_i |y_i|/m_i] * 2^-n, min of independent half-normals with scales sqrt(c_i)
                    emin = integrate.quad(lambda t: float(np.prod(2 * stats.norm.sf(t / np.sqrt(c)))), 0, np.inf, limit=400)[0]
                    Eh -= float((P * lo).sum()) * emin * 2.0 ** (-n)
                H = k * math.log(2) + ent(P)
                report("C: sign + norm", gname, n, Eh, H, gam, f"k={k} signs, M={wname}, grid {kind} {par}")
show_best("C: sign + norm")

# ---------------- D. 2-D lattices: hexagonal and square, scaled, rotated, sheared ----------------
print("\nD. 2-D lattice quantisers (whitened coordinates): cells = Voronoi cells of T L, L hexagonal or square, T = s * rot * diag(1, r)")
print("   E h = perimeter(Gam^1/2 T V)/(2 sqrt(2 pi)) for every cell (exact); H = -sum P ln P over all cells with P > 1e-16 (exact quadrature)")
xg, wg = np.polynomial.legendre.leggauss(24)
def poly_P(V):
    """Gaussian measure of a convex polygon (vertices ccw), by Gauss-Legendre over vertical strips."""
    xs = np.unique(V[:, 0]); tot = 0.0
    E = np.stack([V, np.roll(V, -1, 0)], 1)
    for x0, x1 in zip(xs[:-1], xs[1:]):
        if x1 - x0 < 1e-15: continue
        x = 0.5 * (x1 - x0) * xg + 0.5 * (x1 + x0)
        ys = []
        for (p, q) in E:
            if min(p[0], q[0]) <= x0 + 1e-15 and max(p[0], q[0]) >= x1 - 1e-15 and abs(q[0] - p[0]) > 1e-15:
                ys.append(p[1] + (x - p[0]) * (q[1] - p[1]) / (q[0] - p[0]))
        if len(ys) < 2: continue
        ys = np.array(ys)
        tot += 0.5 * (x1 - x0) * (wg * stats.norm.pdf(x) * np.exp(log_mass(ys.min(0), ys.max(0)))).sum()
    return tot

hexV = np.array([[math.cos(math.pi / 6 + j * math.pi / 3), math.sin(math.pi / 6 + j * math.pi / 3)] for j in range(6)]) / math.sqrt(3)
hexB = np.array([[1.0, 0.0], [0.5, math.sqrt(3) / 2]])           # basis of the hexagonal lattice with that Voronoi cell
sqV = np.array([[-.5, -.5], [.5, -.5], [.5, .5], [-.5, .5]]); sqB = np.eye(2)
print("   check: hexagon area %.6f (expected sqrt(3)/2 = %.6f), measure of a cell = %.8f via quadrature" % (
    0.5 * abs(np.cross(hexV, np.roll(hexV, -1, 0)).sum()), math.sqrt(3) / 2, poly_P(hexV * 0.3)))
spec2 = {"(1,1)": np.array([1.0, 1.0]), "(1,0.1)": np.array([1.0, 0.1]), "(1,0.01)": np.array([1.0, 0.01]), "(1,1e-4)": np.array([1.0, 1e-4])}
for gname, gam in spec2.items():
    for lname, V0, B0 in (("hex", hexV, hexB), ("square", sqV, sqB)):
        for ang in (0.0, math.pi / 12, math.pi / 4):
            for r in (1.0, 0.5 * (gam[1] / gam[0]) ** 0.25 * 2, (gam[1] / gam[0]) ** -0.25, 3.0):
                for s in (2.5, 1.5, 1.0, 0.6, 0.35):
                    if FAST and s < 0.6: continue
                    Rm = np.array([[math.cos(ang), -math.sin(ang)], [math.sin(ang), math.cos(ang)]])
                    T = s * Rm @ np.diag([1.0, r])
                    V = V0 @ T.T
                    if np.cross(V[1] - V[0], V[2] - V[1]) < 0: V = V[::-1]
                    Bm = B0 @ T.T
                    per = np.linalg.norm(np.diff(np.vstack([V, V[:1]]) * np.sqrt(gam), axis=0), axis=1).sum()
                    Eh = per / (2 * SQ2PI)
                    # enumerate lattice points within radius 9 (mass outside < 1e-17)
                    J = int(12 / (s * min(1.0, r) * 0.5)) + 2
                    ii, jj = np.meshgrid(np.arange(-J, J + 1), np.arange(-J, J + 1))
                    pts = ii.ravel()[:, None] * Bm[0] + jj.ravel()[:, None] * Bm[1]
                    pts = pts[np.linalg.norm(pts, axis=1) < 9.0 + 2 * s * max(1, r)]
                    Ps = np.array([poly_P(V + p) for p in pts])
                    H = ent(Ps)
                    report("D: 2-D lattice", gname, 2, Eh, H, gam, f"{lname}, angle {ang:.3f}, aspect {r:.3f}, scale {s}; mass covered {Ps.sum():.12f}")
show_best("D: 2-D lattice")

# ---------------- E. D4 lattice in 4-D (24-cell Voronoi cells) ----------------
print("\nE. D4 lattice quantisers in 4-D: lattice points T*Z with Z in D4 (integer, even sum); Voronoi cell = 24-cell, vertices")
print("   (+-1,0,0,0) and (+-1/2,...) up to permutation; T = s diag(a); E h by Monte Carlo (max over 24 vertices, s.e. given);")
print("   P per cell = vol(TV) * mean of phi over 4,000 uniform points in the cell (quadrature error by halving)")
import itertools
V4 = [np.array(p, float) for p in set(itertools.permutations([1, 0, 0, 0])) | set(itertools.permutations([-1, 0, 0, 0]))]
V4 += [np.array(p) * 0.5 for p in itertools.product([-1, 1], repeat=4)]
V4 = np.array(V4)
nbrs = np.array([np.array(p, float) for p in set(itertools.permutations([1, 1, 0, 0])) | set(itertools.permutations([1, -1, 0, 0])) | set(itertools.permutations([-1, -1, 0, 0]))])
U = rng.uniform(-1, 1, (36000, 4))
acc = np.array([((U[i:i + 4000, None, :] - nbrs[None]) ** 2).sum(2).min(1) >= (U[i:i + 4000] ** 2).sum(1) for i in range(0, 36000, 4000)]).ravel()
print("   uniform points in the 24-cell: %d; volume check 16 * acceptance = %.4f (should be 2)" % (acc.sum(), 16 * acc.mean()))
U = U[acc][:4000]
G4 = rng.standard_normal((40000, 4))
spec4 = {"flat4": np.ones(4), "(1,1,.1,.1)": np.array([1, 1, .1, .1]), "(1,.01,.01,.01)": np.array([1, .01, .01, .01])}
for gname, gam in spec4.items():
    for aname, a in (("isotropic", np.ones(4)), ("dual", (gam / gam.max()) ** -0.25)):
        for s in (2.0, 1.2, 0.8):
            T = s * a
            Vt = V4 * T
            y = G4 * np.sqrt(gam)
            hs = (y @ Vt.T).max(1)
            Eh, se = hs.mean(), hs.std() / math.sqrt(len(hs))
            R = 8.5
            rng_ = [int(R / (s * a[i])) + 2 for i in range(4)]
            grid = np.array(np.meshgrid(*[np.arange(-q, q + 1) for q in rng_], indexing="ij")).reshape(4, -1).T
            grid = grid[(grid.sum(1) % 2) == 0]
            pts = grid * T
            pts = pts[np.linalg.norm(pts, axis=1) < R + 2 * s * a.max()]
            vol = 2.0 * np.prod(T)
            Pa, Pb = np.empty(len(pts)), np.empty(len(pts))
            half = len(U) // 2
            for c0 in range(0, len(pts), 40):
                X = pts[c0:c0 + 40, None, :] + (U * T)[None]
                ph = np.exp(-0.5 * (X * X).sum(2)) / (2 * math.pi) ** 2
                Pa[c0:c0 + 40] = vol * ph[:, :half].mean(1); Pb[c0:c0 + 40] = vol * ph[:, half:].mean(1)
            P = 0.5 * (Pa + Pb)
            H = ent(P / P.sum()); dH = abs(ent(Pa / Pa.sum()) - ent(Pb / Pb.sum())) / 2
            rw, rf = report("E: D4 lattice", gname, 4, Eh, H, gam, f"{aname}, scale {s}; mass {P.sum():.6f}; H +- {dH:.1e}", err=se)
            print("    %-16s %-9s s=%.1f  H=%.4f +- %.1e  E h=%.5f +- %.5f  E h/W=%.4f  E h/F2=%.4f" % (gname, aname, s, H, dH, Eh, se, rw, rf))
show_best("E: D4 lattice")

print("\nSummary: smallest E h / bound per family (W = F1; F2 = max over the top-k sets tested; Phi = the stage-0 conjecture)")
for fam in ["0: ECSQ every coordinate", "A: ECSQ head + bounded tail", "B: norm only", "C: sign + norm", "D: 2-D lattice", "E: D4 lattice"]:
    for b in ("W", "F2", "Phi"):
        if (fam, b) in rec:
            r = rec[(fam, b)]
            print("   %-28s /%-2s  %.5f  +- %.1e   %s n=%d H=%.3f  %s" % (fam, b, r[0], r[1], r[2], r[3], r[4], r[5]))
