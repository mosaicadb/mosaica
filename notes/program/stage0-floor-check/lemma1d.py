# Step 1: attack the one-dimensional lemma  C(mu) >= s(KL(mu || N(0,1))), at fixed KL = kappa, kappa from 0.01 to 10 nats.
# C(mu) = int_0^1 F^{-1}(u) Phi^{-1}(u) du. Integrating by parts with I(u) = phi(Phi^{-1}(u)) (I' = -Phi^{-1}, I(0) = I(1) = 0):
#     C(mu) = int_R I(F_mu(x)) dx,
# which needs only the CDF and is exact for laws with gaps (a gap of length L at level F = u contributes I(u) L).
# Every law below is a genuine probability law evaluated with its own KL, so a ratio C/s(KL) < 1 beyond the quadrature
# error would refute the lemma. A free minimisation of C/s(KL) drifts to KL -> 0 where everything tends to the trivial
# C = s = 1, so each family is minimised at fixed KL.
import math, sys, warnings
import numpy as np
from scipy import optimize, stats, special, linalg
from common import kfun, s_of_kappa, ratio_to_s, log_mass, standard_run_header

warnings.filterwarnings("ignore")
standard_run_header("lemma1d.py")
rng = np.random.default_rng(20261121)
SQ2PI = math.sqrt(2 * math.pi)
KAPPAS = [0.01, 0.03, 0.1, 0.3, 0.5, 1, 2, 3, 5, 7, 10]
GX, GW = np.polynomial.legendre.leggauss(64)


def I(u):
    u = np.clip(np.asarray(u, float), 0.0, 1.0)
    z = special.ndtri(np.clip(u, 1e-300, 1 - 1e-16))
    return np.where((u <= 0) | (u >= 1), 0.0, np.exp(-0.5 * z * z) / SQ2PI)


# ---------------- restricted Gaussians: N(0,1) on a union of intervals ----------------
def C_union(a, b, nsub=8):
    """(C, KL) for N(0,1) restricted to the union of sorted disjoint intervals [a_j, b_j]. Composite Gauss-Legendre in x
    on each interval (F from log_mass, no cancellation), gaps exact."""
    a, b = np.asarray(a, float), np.asarray(b, float)
    lm = log_mass(a, b); M = lm.max(); m = np.exp(lm - M); P = m.sum()
    cum = np.concatenate([[0], np.cumsum(m)]) / P
    tot = 0.0
    for j in range(len(a)):
        edges = np.linspace(a[j], b[j], nsub + 1)
        x = (0.5 * (edges[1:] - edges[:-1])[:, None] * (GX[None] + 1) + edges[:-1, None]).ravel()
        wts = (0.5 * (edges[1:] - edges[:-1])[:, None] * GW[None]).ravel()
        F = cum[j] + np.exp(log_mass(np.full_like(x, a[j]), x) - M) / P
        tot += float((wts * I(F)).sum())
        if j + 1 < len(a):
            tot += float(I(cum[j + 1])) * (a[j + 1] - b[j])
    return tot, -(math.log(P) + M)


def right_edge(a, target_lm):
    if float(log_mass(a, 60.0)[0]) < target_lm:
        return None
    lo, hi = a, 60.0
    for _ in range(120):
        mid = 0.5 * (lo + hi)
        if float(log_mass(a, mid)[0]) < target_lm: lo = mid
        else: hi = mid
    return hi


c0, k0 = C_union([-1e-6], [1e-6]); c1, k1 = C_union([-40.0], [40.0], nsub=64)
print("checks: narrow interval C/s = %.8f (limit sqrt(e/2) = %.8f); whole line C = %.10f, KL = %.2e" % (ratio_to_s(c0, k0), math.sqrt(math.e / 2), c1, k1))


# ---------------- mixtures on a merged grid ----------------
def C_mix(w, mu, sd):
    w = np.asarray(w, float) / np.sum(w)
    grids = [np.linspace(m_ - 14 * s_, m_ + 14 * s_, 3001) for m_, s_ in zip(mu, sd)]
    grids.append(np.linspace(min(mu - 14 * sd), max(mu + 14 * sd), 6001))
    x = np.unique(np.concatenate(grids))
    z = (x[:, None] - mu[None]) / sd[None]
    lpdf = special.logsumexp(-0.5 * z * z - np.log(sd[None] * SQ2PI) + np.log(w[None]), axis=1)
    F = (w[None] * special.ndtr(z)).sum(1)
    C = np.trapezoid(I(F), x)
    f = np.exp(lpdf)
    KL = np.trapezoid(f * (lpdf + 0.5 * x * x + 0.5 * math.log(2 * math.pi)), x)
    return float(C), float(KL)


cm, km = C_mix(np.array([1.0]), np.array([0.3]), np.array([0.5]))
print("checks: N(0.3, 0.5^2) on the mixture grid: C = %.8f (exact 0.5), KL = %.8f (exact %.8f)" % (cm, km, kfun(0.5) + 0.045))


# ---------------- the convex programme over piecewise-uniform quantile functions ----------------
# mu = mass p_j uniform on [x_{j-1}, x_j]:  KL = sum p_j [ln p_j - ln dx_j] + sum p_j (x_{j-1}^2 + x_{j-1} x_j + x_j^2)/6 + ln(2 pi)/2,
#                                            C = sum (w_j/p_j) dx_j,  w_j = int_{u_{j-1}}^{u_j} I(u) du.   (both exact)
# KL is convex in x (minus log of linear forms plus a convex quadratic); C is linear: a convex programme.
def make_grid(M, tail):
    z = np.linspace(-tail, tail, M - 1)
    u = np.concatenate([[0.0], stats.norm.cdf(z), [1.0]]); p = np.diff(u)
    zz = np.concatenate([[-np.inf], z, [np.inf]])
    w = (special.ndtr(math.sqrt(2) * zz[1:]) - special.ndtr(math.sqrt(2) * zz[:-1])) / (2 * math.sqrt(math.pi))
    return p, w, z


def KL_C(x, p, w):
    dx = np.diff(x); a, b = x[:-1], x[1:]
    return (float((p * (np.log(p) - np.log(dx))).sum() + (p * (a * a + a * b + b * b)).sum() / 6 + 0.5 * math.log(2 * math.pi)),
            float(((w / p) * dx).sum()))


def solve_lagr(p, w, lam, x0):
    """minimise C + lam KL by damped Newton with the tridiagonal Hessian; returns x and the smallest Hessian eigenvalue."""
    x = x0.copy(); M = len(p); cw = w / p
    for it in range(100):
        dx = np.diff(x); a, b = x[:-1], x[1:]
        g = np.zeros(M + 1)
        gd = cw - lam * p / dx
        g[1:] += gd; g[:-1] -= gd
        g[:-1] += lam * p * (2 * a + b) / 6; g[1:] += lam * p * (a + 2 * b) / 6
        hd = lam * p / dx ** 2
        diag = np.zeros(M + 1); diag[:-1] += hd + lam * p / 3; diag[1:] += hd + lam * p / 3
        off = -hd + lam * p / 6
        ab = np.zeros((3, M + 1)); ab[0, 1:] = off; ab[1] = diag; ab[2, :-1] = off
        step = linalg.solve_banded((1, 1), ab, -g)
        dec = -g @ step
        f0 = KL_C(x, p, w); f0 = f0[1] + lam * f0[0]; t = 1.0
        while True:
            xn = x + t * step
            if np.all(np.diff(xn) > 0):
                f1 = KL_C(xn, p, w); f1 = f1[1] + lam * f1[0]
                if f1 <= f0 - 0.25 * t * dec: break
            t *= 0.5
            if t < 1e-14: break
        x = xn
        if dec < 1e-24: break
    mineig = float(linalg.eigvals_banded(ab[:2], lower=False, select="i", select_range=(0, 0))[0])
    return x, mineig


rows = []
print("\nkappa | s(kappa) | min C/s at KL = kappa: interval | union of 2 intervals | mixture of 2 | mixture of 3 | uniform | Laplace | convex programme M=200 | M=800 | min Hessian eig")
for kap in KAPPAS:
    s = s_of_kappa(kap); lt = -kap
    def f_int(a):
        b = right_edge(a, lt)
        if b is None: return np.inf
        return ratio_to_s(*C_union([a], [b]))
    grid = np.linspace(-12, 12, 121); vals = np.array([f_int(a) for a in grid]); j = int(np.argmin(vals))
    r = optimize.minimize_scalar(f_int, bounds=(grid[max(j - 1, 0)], grid[min(j + 1, 120)]), method="bounded", options={"xatol": 1e-10})
    r_int = min(r.fun, vals.min())
    def f_u2(p):
        a1, w1, gap = p[0], math.exp(p[1]), math.exp(p[2]); b1 = a1 + w1
        l1 = float(log_mass(a1, b1)[0])
        if l1 >= lt: return 1e9
        rest = lt + math.log(-math.expm1(l1 - lt)); a2 = b1 + gap; b2 = right_edge(a2, rest)
        if b2 is None: return 1e9
        return ratio_to_s(*C_union([a1, a2], [b1, b2]))
    r_u2 = np.inf
    for t in range(40):
        x0 = [rng.normal(-1, 1.5), math.log(rng.uniform(0.02, 3)), math.log(rng.uniform(0.01, 3))]
        if f_u2(x0) > 1e8: continue
        rr = optimize.minimize(f_u2, x0, method="Nelder-Mead", options={"xatol": 1e-7, "fatol": 1e-10, "maxiter": 600})
        r_u2 = min(r_u2, rr.fun)
        if t > 12 and np.isfinite(r_u2): break
    r_mix = {}
    for nc in (2, 3):
        best = np.inf
        for t in range(12):
            x0 = np.concatenate([rng.normal(0, 1, nc), rng.normal(0, 1.2, nc), math.log(s) + rng.normal(0, 0.8, nc)])
            def obj(p):
                w_ = np.exp(p[:nc]); mu = p[nc:2 * nc]; sd = np.exp(np.clip(p[2 * nc:], -8, 2))
                C, KL = C_mix(w_, mu, sd)
                if not (KL > 0 and np.isfinite(C)): return 1e9
                return ratio_to_s(C, KL) + 30 * math.log(KL / kap) ** 2
            rr = optimize.minimize(obj, x0, method="Nelder-Mead", options={"xatol": 1e-6, "fatol": 1e-10, "maxiter": 1500})
            p = rr.x; C, KL = C_mix(np.exp(p[:nc]), p[nc:2 * nc], np.exp(np.clip(p[2 * nc:], -8, 2)))
            if abs(math.log(KL / kap)) < 0.01: best = min(best, ratio_to_s(C, KL))
        r_mix[nc] = best
    uk = lambda c: -math.log(2 * c) + c * c / 6 + 0.5 * math.log(2 * math.pi)
    r_uni = ratio_to_s(optimize.brentq(lambda c: uk(c) - kap, 1e-300, math.sqrt(3)) / math.sqrt(math.pi), kap) if uk(math.sqrt(3)) < kap else np.nan
    lk_ = lambda c: -math.log(2 * c) - 1 + c * c + 0.5 * math.log(2 * math.pi)
    if lk_(1 / math.sqrt(2)) < kap:
        cl = optimize.brentq(lambda c: lk_(c) - kap, 1e-300, 1 / math.sqrt(2))
        xs = np.linspace(-50 * cl, 50 * cl, 400001)
        r_lap = ratio_to_s(float(np.trapezoid(I(stats.laplace.cdf(xs, 0, cl)), xs)), kap)
    else:
        r_lap = np.nan
    cp = []
    for M in (200, 800):
        p, w, zq = make_grid(M, 7.5)
        x0f = lambda: s * np.concatenate([[-8.0], zq, [8.0]])
        lo, hi = math.log(1 / (1 / s - s)) - 3, math.log(1 / (1 / s - s)) + 3
        for _ in range(50):
            mid = 0.5 * (lo + hi)
            x, me = solve_lagr(p, w, math.exp(mid), x0f())
            if KL_C(x, p, w)[0] > kap: lo = mid
            else: hi = mid
        x, me = solve_lagr(p, w, math.exp(hi), x0f())
        KLv, Cv = KL_C(x, p, w)
        cp.append((ratio_to_s(Cv, KLv), KLv, me))
    rows.append((kap, min(r_int, r_u2, r_mix[2], r_mix[3], cp[0][0], cp[1][0])))
    print(f"{kap:5.2f} | {s:.6g} | {r_int:.6f} | {r_u2:.6f} | {r_mix[2]:.6f} | {r_mix[3]:.6f} | {r_uni:.6f} | {r_lap:.6f} | {cp[0][0]:.8f} | {cp[1][0]:.8f} | {min(cp[0][2], cp[1][2]):.1e}")

print("\nsmallest C/s(KL) over every family and kappa: %.8f (1 is equality, attained only by N(0, s^2); below 1 refutes)" % min(r for _, r in rows))
p, w, zq = make_grid(800, 7.5); s = s_of_kappa(1.0)
x, _ = solve_lagr(p, w, 1 / (1 / s - s), s * np.concatenate([[-8.0], zq, [8.0]]))
sel = np.abs(zq) < 4
print("convex programme at the Gaussian multiplier lam = s/(1 - s^2), kappa = 1: max |x_j - s Phi^-1(u_j)| over |z| < 4 = %.2e (s = %.4f)" % (np.abs(x[1:-1][sel] - s * zq[sel]).max(), s))
worst = min(ratio_to_s(sig, float(kfun(sig)) + m * m / 2) for sig in np.exp(np.linspace(-6, 2, 200)) for m in (0, 0.1, 0.5, 1, 3))
print("N(m, sigma^2), closed form over sigma in [e^-6, e^2], m in {0, .1, .5, 1, 3}: min C/s = %.10f (equality at m = 0, sigma <= 1)" % worst)
