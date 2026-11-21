# Referee's attacks on F3 and F4 of CLAIMS.md.  Small numpy/scipy only.
# Run:  nice -n 10 taskset -c 2 env OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 ref_f3_f4.py
import math
import numpy as np
from scipy import stats

rng = np.random.default_rng(7)
EG = {k: math.sqrt(2) * math.exp(math.lgamma((k + 1) / 2) - math.lgamma(k / 2)) for k in (1, 2)}
OM = {1: 2.0, 2: math.pi}
KAP = {k: EG[k] * math.sqrt(2 * math.pi * math.e) * OM[k] ** (-1 / k) for k in (1, 2)}


def W(gam, H):
    """sum sqrt(min(gam_i, theta)), sum 0.5 ln+(gam_i/theta) = H (own implementation, any n)."""
    a = np.sort(np.sqrt(np.asarray(gam, float)))[::-1]
    if len(a) == 0:
        return 0.0
    if H <= 0:
        return float(a.sum())
    for k in range(1, len(a) + 1):
        lam = math.exp((np.log(a[:k]).sum() - H) / k)
        nxt = a[k] if k < len(a) else 0.0
        if lam >= nxt - 1e-15 and lam <= a[k - 1] + 1e-15:
            return float(k * lam + a[k:].sum())
    raise RuntimeError


def V(gam, S, H, grid=4001):
    gam = np.asarray(gam, float); S = list(S); T = [i for i in range(len(gam)) if i not in S]; k = len(S)
    if k == 0:
        return W(gam, H)
    gm = math.exp(np.log(gam[S]).sum() / (2 * k))
    return min(KAP[k] * gm * math.exp(-(H - u) / k) + W(gam[T], u) for u in np.linspace(0, H, grid if T else 1))


def best_floor(gam, H):
    return max(V(gam, S, H, 801) for S in ([], [0], [1], [0, 1]))


# ------------------------------------------------------------------ F3, test 1: z uniform on a rotated square
# u uniform on [-sqrt3, sqrt3]^2, z = R(phi) u: E zz^T = I, h(z) = ln 12, J = ln(2 pi e) - ln 12.
# Cells are rectangles in the u coordinates; the smallest sound certificate of a cell K has mean half-width
# E|g_2| * perimeter(Gamma^1/2 K) / (2 pi), exactly.
J = math.log(2 * math.pi * math.e) - math.log(12.0)
s3 = math.sqrt(3.0)
worst = (9e9, None); worst_noJ = (9e9, None); cnt = 0
for trial in range(4000):
    eps = float(rng.choice([1.0, 0.7, 0.4, 0.2, 0.05])); gam = np.array([1.0, eps * eps])
    phi = float(rng.choice([0.0, math.pi / 4, rng.uniform(0, math.pi / 2)]))
    R = np.array([[math.cos(phi), -math.sin(phi)], [math.sin(phi), math.cos(phi)]])
    A = np.diag(np.sqrt(gam)) @ R
    l1, l2 = np.linalg.norm(A[:, 0]), np.linalg.norm(A[:, 1])
    def cuts():
        m = int(rng.choice([1, 1, 2, 3, 5, 8, 16, 40]))
        if rng.random() < 0.5:
            c = np.linspace(-s3, s3, m + 1)
        else:
            c = np.concatenate([[-s3], np.sort(rng.uniform(-s3, s3, m - 1)), [s3]])
        return np.diff(c)
    w1, w2 = cuts(), cuts()
    P = np.outer(w1, w2) / 12.0
    H = float(-(P * np.log(P)).sum())
    per = 2 * (l1 * w1[:, None] + l2 * w2[None, :])
    Eh = float((P * EG[2] * per / (2 * math.pi)).sum())
    f = best_floor(gam, H + J); f0 = best_floor(gam, H)
    cnt += 1
    if Eh / f < worst[0]: worst = (Eh / f, (eps, phi, len(w1), len(w2), H))
    if Eh / f0 < worst_noJ[0]: worst_noJ = (Eh / f0, (eps, phi, len(w1), len(w2), H))
print("F3 test 1 (uniform on a rotated square, J = %.4f nats, %d codes): min E h / max_S V_S(H + J) = %.6f at (eps, phi, cells1, cells2, H) = %s"
      % (J, cnt, worst[0], worst[1]))
print("    the same against the Gaussian floor without J: min E h / max_S V_S(H) = %.6f at %s  (below 1 means J is needed)" % worst_noJ)

# ------------------------------------------------------------------ F3, test 2: product law, coded uniform x uncoded bimodal
# z1 uniform (coded on a grid of m cells); z2 = +-[1-e, 1+e] uniform on the two pieces, rescaled to unit second moment (uncoded).
worst2 = (9e9, None)
for e in (0.9, 0.5, 0.2, 0.05, 0.01):
    sc = 1 / math.sqrt(1 + e * e / 3)                       # E z2^2 = 1 after scaling
    h2 = math.log(4 * e * sc)                               # entropy of z2
    J2 = (0.5 * math.log(2 * math.pi * math.e) - math.log(2 * s3)) + (0.5 * math.log(2 * math.pi * math.e) - h2)
    for eps in (1.0, 0.3, 3.0, 10.0):
        gam = np.array([1.0, eps * eps])
        for m in (1, 2, 4, 16, 64):
            H = math.log(m)
            Eh = EG[1] * (s3 / m) + EG[1] * eps * (1 + e) * sc     # support function of a product is the sum
            f = best_floor(gam, H + J2)
            if Eh / f < worst2[0]: worst2 = (Eh / f, (e, eps, m, J2))
print("F3 test 2 (uniform x bimodal): min E h / max_S V_S(H + J) = %.6f at (e, eps, m, J) = %s" % worst2)
# the tight case by hand: n = 1, uniform law, uniform grid: E h = E|g| sqrt3/m and kappa_1 e^-(ln m + J) are equal
J1 = 0.5 * math.log(2 * math.pi * math.e) - math.log(2 * s3)
print("    n = 1 uniform law on a grid of 7 cells: E h = %.6f, kappa_1 e^-(H+J) = %.6f" % (EG[1] * s3 / 7, KAP[1] * math.exp(-(math.log(7) + J1))))

# ------------------------------------------------------------------ F4
# (i) F/f of the chi_n law is non-decreasing (the proof's condition), by the closed forms, on a wide grid
bad = 0
for n in (1, 2, 3, 5, 10, 76, 768):
    r = np.exp(np.linspace(math.log(1e-3), math.log(math.sqrt(n) + 12), 20000))
    lr = stats.chi.logcdf(r, n) - stats.chi.logpdf(r, n)
    ok = np.isfinite(lr)
    d = np.diff(lr[ok])
    bad += int((d < -1e-9).sum())
print("F4 (i): decreases of ln(F/f) for chi_n on a grid, n in {1,2,3,5,10,76,768}: %d" % bad)
# (ii) cell statement and its average on partitions whose exact width is known
#      n = 1: intervals; E h = E|g| * sum P_c * (half length).  n = 2: annuli (hull = disc): E h = E|g_2| * sum P_c R_c.
w4 = (9e9, None)
for trial in range(3000):
    if rng.random() < 0.5:
        step = math.exp(rng.uniform(-3, 2.2)); off = rng.uniform(0, 1)
        edges = (np.arange(-400, 401) + off) * step
        if rng.random() < 0.5:
            edges = np.sort(np.concatenate([edges[np.abs(edges) > rng.uniform(0, 3)], [rng.uniform(-1, 1)]]))
        edges = edges[np.abs(edges) < 40]
        P = np.diff(stats.norm.cdf(edges)); half = np.diff(edges) / 2
        # the two unbounded end cells have mass < 1e-300 and are dropped (their width is infinite, so this favours the code)
        keep = P > 0; P, half = P[keep], half[keep]
        H = float(-(P * np.log(P)).sum()); Eh = EG[1] * float((P * half).sum()); n = 1
    else:
        m = int(rng.integers(1, 30)); rad = np.sort(np.concatenate([np.exp(rng.uniform(-2, 1.6, m)), np.arange(6, 40.0)]))
        cdf = np.concatenate([[0.0], stats.chi.cdf(rad, 2)]); P = np.diff(cdf); keep = P > 0
        P, Rr = P[keep], rad[keep]
        H = float(-(P * np.log(P)).sum()); Eh = EG[2] * float((P * Rr).sum()); n = 2
    Pe = math.exp(-H)
    q = stats.chi.ppf(Pe, n) if Pe < 0.5 else stats.chi.isf(-math.expm1(-H), n)
    f4 = EG[n] * q
    if Eh / f4 < w4[0]: w4 = (Eh / f4, (n, H))
print("F4 (ii): 3000 interval and annulus partitions: min E h / (E|g_n| rho_n(e^-H)) = %.6f at (n, H) = %s" % w4)
