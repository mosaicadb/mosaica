# Step 2: attack the per-cell inequality  E h_K(g) >= Phi(gam, ln(1/P)),  K = Gam^{1/2} A,  P = standard Gaussian measure of A.
# Every family below has E h_K exact (closed form or 1-D quadrature) unless marked MC, and ln P exact (closed form,
# quadrature, or Laplace inversion in gaussq.py). Ratio R = E h_K / Phi(ln 1/P); R < 1 beyond error refutes.
# Non-convex cells (shells) are included for completeness only: h_A = h_conv(A) and P(conv A) >= P(A), and Phi is
# decreasing, so replacing A by its convex hull can only lower R.
import math, json, sys, warnings
import numpy as np
from scipy import stats, optimize, special, integrate
from common import Phi, phi_alloc, E_norm, Eg, log_mass, standard_run_header
from gaussq import log_cdf

warnings.filterwarnings("ignore")
standard_run_header("percell.py")
rng = np.random.default_rng(7)
SQ2PI = math.sqrt(2 * math.pi)
DIMS = [1, 2, 5, 20, 100]
spec = json.load(open("../../../agent1/spec2_n128.json"))
gm = np.sort(np.array(spec["5"]["lam"]) * np.array(spec["5"]["Qe"]))[::-1]


def spectra(n):
    i = np.arange(1, n + 1, dtype=float)
    out = {"flat": np.ones(n), "i^-1": 1 / i, "i^-2": 1 / i ** 2, "0.8^i": 0.8 ** (i - 1), "measured(n128 g5)": gm[:n] / gm[0]}
    return out if n > 1 else {"flat": np.ones(1)}


def haar(n):
    q, r = np.linalg.qr(rng.standard_normal((n, n)))
    return q * np.sign(np.diag(r))


def log_int(a, b):
    """ln P[a <= Z <= b], elementwise, accurate for narrow and far intervals (common.log_mass)."""
    return log_mass(a, b)


best = {}   # (family) -> (R, err, n, spectrum, description)


def note(fam, R, err, n, sp, desc):
    if not np.isfinite(R):
        return
    key = fam
    if key not in best or R < best[key][0]:
        best[key] = (R, err, n, sp, desc)
    k2 = (fam, n)
    if k2 not in best or R < best[k2][0]:
        best[k2] = (R, err, n, sp, desc)


def ratio(Eh, logP, gam):
    """E h / Phi(ln 1/P). Degenerate cells (zero width, P underflowing, Phi underflowing to 0) return inf rather than a
    meaningless number; the narrow-cell limit is handled analytically in step 1 (R -> sqrt(e) per coordinate)."""
    logP = float(np.sum(logP))
    if not (np.isfinite(logP) and Eh > 0 and np.isfinite(Eh)) or -logP > 1e5:
        return np.inf
    ph = Phi(gam, -logP)
    return Eh / ph if ph > 0 else np.inf


def box_ok(lo, hi):
    return bool(np.all(np.isfinite(lo)) and np.all(np.isfinite(hi)) and np.all(hi - lo > 1e-12))


# ---------------- 1. boxes aligned with the eigenbasis (centred and off-centre) ----------------
print("\n1. aligned boxes: E h = sum sqrt(gam_i)(b_i - a_i)/sqrt(2 pi); ln P = sum ln P[a_i <= Z <= b_i]")
def box_R(lo, hi, gam):
    if not box_ok(lo, hi):
        return np.inf
    return ratio((np.sqrt(gam) * (hi - lo)).sum() / SQ2PI, float(log_int(lo, hi).sum()), gam)

for n in DIMS:
    for sp, gam in spectra(n).items():
        a = np.sqrt(gam)
        for t in range(150 if n <= 20 else 40):
            if t % 3 == 0:     # structured: widths c * gam^beta, centred
                c, beta, off = math.exp(rng.uniform(-5, 2)), rng.uniform(-0.5, 0.5), 0.0
                w = c * gam ** beta
            else:
                w = np.exp(rng.uniform(-5, 1.5, n)); off = rng.choice([0, 0.5, 2, 5]) * rng.standard_normal(n) * (t % 3 == 2)
            lo, hi = off - w / 2, off + w / 2
            note("box aligned", box_R(lo, hi, gam), 0, n, sp, f"widths {np.round(w[:4], 4)}..., centre |c|={np.linalg.norm(off):.3g}")
        # local minimisation over the structured family (c, beta, common offset along e1)
        def obj(p):
            w = math.exp(p[0]) * gam ** p[1]
            return box_R(-w / 2, w / 2, gam)
        for x0 in ([-1, 0], [0, 0.25], [-3, -0.25], [1, 0]):
            r = optimize.minimize(obj, x0, method="Nelder-Mead", options={"xatol": 1e-8, "fatol": 1e-12})
            note("box aligned", r.fun, 0, n, sp, f"centred, widths {math.exp(r.x[0]):.4g} gam^{r.x[1]:.3f}")
        if n <= 5:
            def obj2(p):
                return box_R(p[n:] - np.exp(p[:n]) / 2, p[n:] + np.exp(p[:n]) / 2, gam)
            for t in range(5):
                r = optimize.minimize(obj2, np.concatenate([rng.uniform(-3, 1, n), rng.normal(0, 0.5, n)]), method="Nelder-Mead",
                                      options={"xatol": 1e-9, "fatol": 1e-13, "maxiter": 6000})
                note("box aligned", r.fun, 0, n, sp, f"free NM: widths {np.round(np.exp(r.x[:n]), 4)}, centre {np.round(r.x[n:], 3)}")
for n in DIMS:
    print("   n = %3d: min R = %.6f  (%s; %s)" % ((n,) + (best[("box aligned", n)][0], best[("box aligned", n)][3], best[("box aligned", n)][4])))
# the n = 1 limit by hand: narrow interval at centre c gives R -> sqrt(e) e^{c^2/2}
print("   n = 1 narrow-interval limit sqrt(e) = %.6f" % math.sqrt(math.e))


# ---------------- 2. boxes in a random rotated basis ----------------
print("\n2. misaligned boxes (Haar rotation R): E h = sum_j w_j sqrt((R^T Gam R)_jj)/sqrt(2 pi); P as for the unrotated box")
for n in DIMS[1:]:
    for sp, gam in spectra(n).items():
        for t in range(30):
            Rm = haar(n) if t % 2 == 0 else np.linalg.qr(np.eye(n) + 0.2 * rng.standard_normal((n, n)))[0]   # full or slight
            dg = np.sqrt(np.einsum("ij,i,ij->j", Rm, gam, Rm))
            def obj(p):
                w = math.exp(p[0]) * dg ** (2 * p[1])
                return ratio((w * dg).sum() / SQ2PI, float(log_int(-w / 2, w / 2).sum()), gam)
            r = optimize.minimize(obj, [-1, 0], method="Nelder-Mead", options={"xatol": 1e-8, "fatol": 1e-12})
            note("box misaligned", r.fun, 0, n, sp, f"{'Haar' if t % 2 == 0 else 'slight'} rotation, widths {math.exp(r.x[0]):.4g} d^{2*r.x[1]:.3f}")
    print("   n = %3d: min R = %.6f  (%s; %s)" % (n, best[("box misaligned", n)][0], best[("box misaligned", n)][3], best[("box misaligned", n)][4]))


# ---------------- 3. balls in whitened coordinates, centred and off-centre ----------------
print("\n3. balls {||z - c|| <= rho}: E h = rho E||Gam^1/2 g||, ln P = ln ncx2.cdf(rho^2; n, ||c||^2)")
for n in DIMS:
    for sp, gam in spectra(n).items():
        En = E_norm(gam)
        for cn in [0, 0.5, 1, 2, 4]:
            def obj(lr):
                rho = math.exp(lr)
                lp = stats.chi2.logcdf(rho * rho, n) if cn == 0 else stats.ncx2.logcdf(rho * rho, n, cn * cn)
                return ratio(rho * En, float(lp), gam)
            grid = np.linspace(math.log(0.02), math.log(3 * math.sqrt(n) + 3), 80)
            vals = [obj(x) for x in grid]
            j = int(np.argmin(vals))
            r = optimize.minimize_scalar(obj, bounds=(grid[max(j - 1, 0)], grid[min(j + 1, 79)]), method="bounded", options={"xatol": 1e-10})
            note("ball", r.fun, 0, n, sp, f"rho={math.exp(r.x):.4g} (rho/sqrt n={math.exp(r.x)/math.sqrt(n):.3f}), |centre|={cn}")
for n in DIMS:
    b = best[("ball", n)]; print("   n = %3d: min R = %.6f  (%s; %s)" % (n, b[0], b[3], b[4]))
print("   isotropic centred ball, R against rho/sqrt(n) at n = 100:")
for f in [0.1, 0.3, 0.5, 0.7, 0.8, 0.9, 0.95, 1.0, 1.05, 1.2]:
    rho = f * 10
    print("     rho/sqrt n %.2f: P %.3e  R %.6f" % (f, math.exp(stats.chi2.logcdf(rho * rho, 100)), ratio(rho * Eg(100), float(stats.chi2.logcdf(rho * rho, 100)), np.ones(100))))


# ---------------- 4. ellipsoids aligned with the eigenbasis ----------------
print("\n4. aligned ellipsoids {sum z_i^2/e_i^2 <= 1}: E h = E||diag(e sqrt gam) g||, ln P by Laplace inversion (gaussq.py)")
def lowdim_logP(lam):
    """ln P[sum lam_i z_i^2 <= 1] for n <= 3 by nested 1-D quadrature (exact up to quadrature tolerance); the Laplace
    inversion of gaussq.py converges too slowly in 2 and 3 dimensions."""
    lam = np.asarray(lam, float)
    r = 1 / math.sqrt(lam[0])
    if len(lam) == 1:
        return float(log_mass(-r, r)[0])
    def f(x):
        q = 1 - lam[0] * x * x
        if q <= 0: return 0.0
        return stats.norm.pdf(x) * math.exp(lowdim_logP(lam[1:] / q))
    v = integrate.quad(f, -r, r, limit=200, epsabs=0, epsrel=1e-10)[0]
    return math.log(v) if v > 0 else -np.inf


def ell_R(e, gam, centre=None):
    if centre is None and len(e) <= 3:
        lp, rel = lowdim_logP(1 / e ** 2), 1e-10
    else:
        lp, rel = log_cdf(1.0, 1 / e ** 2, None if centre is None else centre / e)
    return ratio(E_norm(gam * e * e), lp, gam), rel

for n in DIMS[1:]:
    for sp, gam in spectra(n).items():
        a = np.sqrt(gam)
        def shape(lam):
            return 2 * lam / (a + np.sqrt(a * a + 4 * lam * lam))     # Phi's own allocation s_i(lam), normalised below
        def obj(p):
            e = math.exp(p[0]) * shape(math.exp(p[1])) ** p[2]
            e = e / e.max() * math.exp(p[0])
            return ell_R(e, gam)[0]
        for x0 in ([math.log(math.sqrt(n)) - 0.3, 0, 1], [math.log(math.sqrt(n)) - 1, -2, 1], [math.log(math.sqrt(n)), 1, 0.5], [0, -1, 1.5]):
            r = optimize.minimize(obj, x0, method="Nelder-Mead", options={"xatol": 1e-6, "fatol": 1e-10, "maxiter": 250})
            e = math.exp(r.x[0]) * shape(math.exp(r.x[1])) ** r.x[2]; e = e / e.max() * math.exp(r.x[0])
            R, rel = ell_R(e, gam)
            note("ellipsoid aligned", R, R * rel, n, sp, f"e = rho s_i(lam)^beta, rho={math.exp(r.x[0]):.4g}, lam={math.exp(r.x[1]):.3g}, beta={r.x[2]:.3f}")
        if n <= 5:
            for t in range(2):
                def obj2(p):
                    return ell_R(np.exp(p), gam)[0]
                r = optimize.minimize(obj2, rng.uniform(-1.5, 1, n) + 0.5 * math.log(n), method="Nelder-Mead", options={"xatol": 1e-7, "fatol": 1e-11, "maxiter": 1500})
                R, rel = ell_R(np.exp(r.x), gam)
                note("ellipsoid aligned", R, R * rel, n, sp, f"free NM: e = {np.round(np.exp(r.x), 4)}")
        # off-centre: the best shape shifted by one or two semi-axes along e1
        e = best[("ellipsoid aligned", n)]
        for sh in [0.5, 1.0]:
            ee = np.full(n, math.sqrt(n)) * 0.8
            c = np.zeros(n); c[0] = sh * ee[0]
            R, rel = ell_R(ee, gam, c)
            note("ellipsoid aligned", R, R * rel, n, sp, f"off-centre {sh} semi-axis")
    b = best[("ellipsoid aligned", n)]; print("   n = %3d: min R = %.6f +- %.1e  (%s; %s)" % (n, b[0], b[1], b[3], b[4]))


# ---------------- 5. ellipsoids misaligned ----------------
print("\n5. misaligned ellipsoids R diag(e) B: E h = E||diag(e) R^T Gam^1/2 g|| (eigenvalues of diag(e) R^T Gam R diag(e)); P as aligned")
for n in DIMS[1:]:
    for sp, gam in spectra(n).items():
        if sp == "flat":
            continue                   # rotation-invariant
        for t in range(4):
            Rm = haar(n) if t % 2 == 0 else np.linalg.qr(np.eye(n) + 0.3 * rng.standard_normal((n, n)))[0]
            Mt = Rm.T @ np.diag(gam) @ Rm
            dg = np.diag(Mt)
            def obj(p):
                e = math.exp(p[0]) * (dg / dg.max()) ** p[1]
                ev = np.linalg.eigvalsh(np.diag(e) @ Mt @ np.diag(e))
                lp, _ = log_cdf(1.0, 1 / e ** 2)
                return ratio(E_norm(np.maximum(ev, 0)), lp, gam)
            r = optimize.minimize(obj, [math.log(math.sqrt(n)) - 0.3, 0.2], method="Nelder-Mead", options={"xatol": 1e-6, "fatol": 1e-10, "maxiter": 150})
            note("ellipsoid misaligned", r.fun, 1e-6 * r.fun, n, sp, f"{'Haar' if t % 2 == 0 else 'slight'} rotation, e = {math.exp(r.x[0]):.3g} (d/dmax)^{r.x[1]:.3f}")
    b = best[("ellipsoid misaligned", n)]; print("   n = %3d: min R = %.6f  (%s; %s)" % (n, b[0], b[3], b[4]))


# ---------------- 6. thin shells (non-convex; hull = ball) ----------------
print("\n6. shells {rho1 <= ||z - c|| <= rho2}: E h = rho2 E||Gam^1/2 g||, P = cdf(rho2^2) - cdf(rho1^2)")
for n in DIMS:
    for sp, gam in spectra(n).items():
        En = E_norm(gam)
        for f2 in [0.3, 0.6, 0.8, 0.9, 1.0, 1.1, 1.5]:
            rho2 = f2 * math.sqrt(n)
            for thick in [0.5, 0.1, 0.01, 1e-3]:
                rho1 = max(rho2 - thick * max(1.0, rho2), 0)
                l2 = stats.chi2.logcdf(rho2 ** 2, n); l1 = stats.chi2.logcdf(rho1 ** 2, n) if rho1 > 0 else -np.inf
                lp = l2 + math.log(-math.expm1(l1 - l2)) if np.isfinite(l1) else l2
                note("thin shell", ratio(rho2 * En, lp, gam), 0, n, sp, f"rho2/sqrt n={f2}, thickness {thick}")
for n in DIMS:
    b = best[("thin shell", n)]; print("   n = %3d: min R = %.6f  (%s; %s)" % (n, b[0], b[3], b[4]))


# ---------------- 7. ball intersected with a half-space or a slab (MC over g for E h; P exact by quadrature) ----------------
print("\n7. ball cut by a half-space or slab {a <= v.z <= b}: h exact per g, E h by Monte Carlo (200,000 draws), P by quadrature")
from scipy import integrate
def logP_ball_slab(rho, a, b, n):
    a, b = max(a, -rho), min(b, rho)
    if a >= b:
        return -np.inf
    f = lambda t: stats.norm.pdf(t) * stats.chi2.cdf(rho * rho - t * t, n - 1)
    v = integrate.quad(f, a, b, limit=400, epsabs=0, epsrel=1e-11)[0]
    return math.log(v)

def h_ball_slab(Y, v, rho, a, b):
    """support function of {||x|| <= rho, a <= v.x <= b} at the rows of Y (exact)."""
    yv = Y @ v
    yn = np.linalg.norm(Y, axis=1)
    yp = np.sqrt(np.maximum(yn * yn - yv * yv, 0))
    t = rho * yv / np.maximum(yn, 1e-300)                 # v-coordinate of the ball's maximiser
    h = rho * yn
    hi_, lo_ = t > b, t < a
    h = np.where(hi_, b * yv + math.sqrt(max(rho * rho - b * b, 0)) * yp, h)
    h = np.where(lo_, a * yv + math.sqrt(max(rho * rho - a * a, 0)) * yp, h)
    return h

NMC = 50_000
for n in DIMS[1:]:
    for sp, gam in spectra(n).items():
        G = rng.standard_normal((NMC, n)) * np.sqrt(gam)                    # Y = Gam^1/2 g
        for vname in ["e1", "en", "random"]:
            v = np.zeros(n)
            if vname == "e1": v[0] = 1
            elif vname == "en": v[-1] = 1
            else: v = rng.standard_normal(n); v /= np.linalg.norm(v)
            for rho_f in [0.5, 0.8, 1.0, 1.3]:
                rho = rho_f * math.sqrt(n) + (0.5 if n <= 2 else 0)
                for (a, b) in [(-np.inf, 0.0), (-np.inf, -1.0), (-np.inf, 1.0), (-0.3, 0.3), (-0.05, 0.05), (0.5, 1.5), (2.0, 2.5), (-1, 1)]:
                    lp = logP_ball_slab(rho, a, b, n)
                    if not np.isfinite(lp):
                        continue
                    h = h_ball_slab(G, v, rho, max(a, -rho), min(b, rho))
                    Ph = Phi(gam, -lp)
                    note("ball cut by half-space/slab", h.mean() / Ph, h.std() / math.sqrt(len(h)) / Ph, n, sp,
                         f"v={vname}, rho/sqrt n={rho_f}, slab=[{a},{b}], P={math.exp(lp):.3g}")
    # re-evaluate the current best of this n with all draws for a clean standard error
    b_ = best[("ball cut by half-space/slab", n)]
    print("   n = %3d: min R = %.5f +- %.5f (MC s.e., 50,000 draws)  (%s; %s)" % (n, b_[0], b_[1], b_[3], b_[4]))


# ---------------- 8. cross-polytopes (L1 balls), aligned and rotated: MC for E h and for P ----------------
print("\n8. L1 balls {||z||_1 <= rho} (aligned and Haar-rotated): E h = rho E max_i |(R^T Gam^1/2 g)_i|, P by MC (both with s.e.)")
for n in [2, 5, 20]:
    Z = np.abs(rng.standard_normal((NMC, n))).sum(1)
    for sp, gam in spectra(n).items():
        for rot in [False, True]:
            Rm = haar(n) if rot else np.eye(n)
            Y = (rng.standard_normal((NMC, n)) * np.sqrt(gam)) @ Rm
            mx = np.abs(Y).max(1)
            for q in [0.001, 0.01, 0.05, 0.2, 0.5, 0.8]:
                rho = float(np.quantile(Z, q))
                Pm = (Z <= rho).mean()
                Eh = rho * mx.mean(); seh = rho * mx.std() / math.sqrt(NMC)
                Ph = Phi(gam, -math.log(Pm))
                # error from P: dPhi/dkappa * se(kappa) with se(kappa) = sqrt((1-P)/(P N))
                dk = math.sqrt((1 - Pm) / (Pm * NMC)); dPh = abs(Phi(gam, -math.log(Pm) + dk) - Ph)
                R = Eh / Ph
                note("L1 ball", R, R * math.hypot(seh / Eh, dPh / Ph), n, sp, f"{'rotated' if rot else 'aligned'}, P={Pm:.3g}")
    b = best[("L1 ball", n)]; print("   n = %3d: min R = %.5f +- %.5f  (%s; %s)" % (n, b[0], b[1], b[3], b[4]))


# ---------------- 9. arbitrary convex polygons in 2-D (exact E h from the perimeter, P by Gauss-Legendre quadrature) ----------------
print("\n9. convex polygons in R^2: E h_K(g) = perimeter(Gam^1/2 A)/(2 sqrt(2 pi)); P exact by quadrature across vertical strips")
from scipy.spatial import ConvexHull
xg, wg = np.polynomial.legendre.leggauss(60)
def poly_logP(V):
    xs = np.unique(V[:, 0])
    tot = 0.0
    E = np.stack([V, np.roll(V, -1, 0)], 1)
    for x0, x1 in zip(xs[:-1], xs[1:]):
        x = 0.5 * (x1 - x0) * xg + 0.5 * (x1 + x0)
        ys = []
        for (p, q) in E:
            if (p[0] - x0) * (q[0] - x1) <= 0 and p[0] != q[0] and min(p[0], q[0]) <= x0 and max(p[0], q[0]) >= x1:
                ys.append(p[1] + (x - p[0]) * (q[1] - p[1]) / (q[0] - p[0]))
        if len(ys) < 2:
            continue
        ys = np.array(ys); lo, hi = ys.min(0), ys.max(0)
        tot += 0.5 * (x1 - x0) * (wg * stats.norm.pdf(x) * np.exp(log_int(lo, hi))).sum()
    return math.log(tot) if tot > 0 else -np.inf

def poly_R(P, gam):
    try:
        hull = ConvexHull(P)
    except Exception:
        return np.inf
    V = P[hull.vertices]
    per = np.linalg.norm(np.diff(np.vstack([V, V[:1]]) * np.sqrt(gam), axis=0), axis=1).sum()
    lp = poly_logP(V)
    return ratio(per / (2 * SQ2PI), lp, gam) if np.isfinite(lp) else np.inf

# check the quadrature on a square and the perimeter formula on a fine regular polygon
sq = np.array([[-1, -1], [1, -1], [1, 1], [-1, 1]], float)
print("   check: square [-1,1]^2: ln P quadrature %.12f exact %.12f" % (poly_logP(sq), 2 * math.log(stats.norm.cdf(1) - stats.norm.cdf(-1))))
th = np.linspace(0, 2 * math.pi, 2000, endpoint=False); disc = np.c_[np.cos(th), np.sin(th)] * 1.3
print("   check: 2000-gon radius 1.3: ln P %.8f (disc %.8f), E h %.8f (disc %.8f)" % (
    poly_logP(disc), stats.chi2.logcdf(1.69, 2), np.linalg.norm(np.diff(np.vstack([disc, disc[:1]]), axis=0), axis=1).sum() / (2 * SQ2PI), 1.3 * Eg(2)))
for gname, gam in [("flat", np.ones(2)), ("1:0.1", np.array([1, 0.1])), ("1:0.01", np.array([1, 0.01])), ("1:1e-4", np.array([1, 1e-4]))]:
    for m in [3, 4, 5, 6, 8, 12]:
        for t in range(8):
            sc = math.exp(rng.uniform(-2.5, 1))
            P0 = rng.standard_normal((m, 2)) * sc + (rng.standard_normal(2) * rng.choice([0, 0.5, 2]))
            r = optimize.minimize(lambda p: poly_R(p.reshape(m, 2), gam), P0.ravel(), method="Nelder-Mead",
                                  options={"xatol": 1e-7, "fatol": 1e-10, "maxiter": 4000 if m <= 6 else 2500})
            note("polygon (2-D)", r.fun, 1e-8, 2, gname, f"{m} vertices: {np.round(r.x.reshape(m, 2), 3).tolist()}")
            note(("polygon (2-D)", gname), r.fun, 1e-8, 2, gname, f"{m} vertices")
    b = best[(("polygon (2-D)", gname))]; print("   spectrum %-7s: min R = %.6f (%s)" % (gname, b[0], b[4]))


print("\nSummary of step 2 (smallest R = E h_K / Phi(ln 1/P) per family; error is quadrature or MC s.e.)")
for fam in ["box aligned", "box misaligned", "ball", "ellipsoid aligned", "ellipsoid misaligned", "thin shell",
            "ball cut by half-space/slab", "L1 ball", "polygon (2-D)"]:
    if fam in best:
        R, err, n, sp, desc = best[fam]
        print("   %-30s R = %.6f +- %.1e   n = %3d  spectrum %-18s %s" % (fam, R, err, n, sp, desc))
print("\nBy dimension:")
for fam in ["box aligned", "box misaligned", "ball", "ellipsoid aligned", "ellipsoid misaligned", "thin shell", "ball cut by half-space/slab", "L1 ball"]:
    print("   %-30s " % fam + "  ".join("n=%d: %s" % (n, ("%.4f" % best[(fam, n)][0]) if (fam, n) in best else "  -   ") for n in DIMS))
