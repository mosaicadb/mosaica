# Shared functions for the stage-0 check of the conjectured floor Phi.
#   k(s) = (s^2 - 1)/2 - ln s, the KL in nats of N(0, s^2) from N(0, 1);
#   s(kappa) the root in (0, 1] of k(s) = kappa;
#   Phi(gam, H) = min sum sqrt(gam_i) s_i  s.t.  s_i in (0, 1], sum k(s_i) <= H  (H in nats);
#   the proved floors of proofcheck/CONSOLIDATED_PROOFS.md (Theorems 1, 2, 3) and the conjectured floor of its section 6.
import math
import numpy as np
from scipy import integrate, special, stats


def kfun(s):
    s = np.asarray(s, float)
    return 0.5 * (s * s - 1) - np.log(s)


def log_s_of_kappa(kappa):
    """ln s(kappa), finite for every kappa (s itself underflows past kappa ~ 700)."""
    kk = float(kappa)
    if kk <= 0:
        return 0.0
    if kk < 1e-12:                      # k(s) = (1-s)^2 + O((1-s)^3) near s = 1
        return math.log1p(-math.sqrt(kk))
    t = math.log(max(1 - math.sqrt(kk), 1e-3)) if kk < 0.5 else -kk - 0.5
    for _ in range(200):
        e2 = math.exp(2 * t)
        f = 0.5 * (e2 - 1) - t - kk
        tn = t - f / (e2 - 1)
        if tn >= 0:
            tn = t / 2
        if abs(tn - t) < 1e-15 * max(1, abs(t)):
            return tn
        t = tn
    return t


def ratio_to_s(C, kappa):
    """C / s(kappa) without underflow."""
    return math.exp(math.log(C) - log_s_of_kappa(kappa)) if C > 0 else 0.0


def s_of_kappa(kappa):
    """Root in (0, 1] of k(s) = kappa, vectorised, solved by Newton in t = ln s."""
    kap = np.atleast_1d(np.asarray(kappa, float))
    out = np.ones_like(kap)
    m = kap > 0
    kk = kap[m]
    # start: small kappa -> s ~ 1 - sqrt(kappa); large kappa -> s ~ exp(-kappa - 1/2)
    t = np.where(kk < 0.5, np.log(np.maximum(1 - np.sqrt(kk), 1e-3)), -kk - 0.5)
    for _ in range(100):
        e2 = np.exp(2 * t)
        f = 0.5 * (e2 - 1) - t - kk
        fp = e2 - 1                      # d/dt, negative for t < 0
        step = f / np.where(fp == 0, -1e-300, fp)
        tn = t - step
        tn = np.where(tn >= 0, t / 2, tn)  # stay inside (-inf, 0)
        if np.all(np.abs(tn - t) < 1e-15 * np.maximum(1, np.abs(t))):
            t = tn
            break
        t = tn
    out[m] = np.exp(t)
    return out if np.ndim(kappa) else float(out[0])


def phi_alloc(gam, H):
    """Minimiser and value of Phi(gam, H). KKT: sqrt(gam_i) + lam (s_i - 1/s_i) = 0, s_i = 2 lam/(a_i + sqrt(a_i^2 + 4 lam^2))."""
    a = np.sqrt(np.maximum(np.asarray(gam, float), 0))
    if H <= 0:
        return a.sum(), np.ones_like(a)
    act = a > 0
    la = np.zeros_like(a); la[act] = np.log(a[act])
    def log_s(ll):
        """ln s_i at ln lam = ll, in log space so that huge H does not underflow:
        s = 2/(r + sqrt(r^2 + 4)) with r = a/lam."""
        lr = la - ll
        r = np.exp(np.minimum(lr, 300))
        out = np.where(lr > 300, -lr, math.log(2) - np.log(r + np.sqrt(r * r + 4)))
        return np.where(act, out, 0.0)
    def K(ll):
        t = log_s(ll)
        return float((0.5 * np.expm1(2 * t) - t).sum())
    amax = float(la[act].max())
    # bracket: at lo, k(s_1) >= -ln s_1 - 1/2 >= H + 9.5 > H; at hi, sum k ~ sum (a/2lam)^2 ~ 0
    lo, hi = amax - H - 10.0, amax + 40.0
    for _ in range(500):
        mid = 0.5 * (lo + hi)
        if K(mid) > H:
            lo = mid           # too much KL: larger lam pushes s towards 1
        else:
            hi = mid
        if hi - lo < 1e-13 * max(1.0, abs(hi)):
            break
    s = np.exp(log_s(hi))
    return float((a * s).sum()), s


def Phi(gam, H):
    return phi_alloc(gam, H)[0]


def Eg(k):
    """E||g_k|| for a standard Gaussian in R^k."""
    return math.sqrt(2) * math.exp(math.lgamma((k + 1) / 2) - math.lgamma(k / 2))


def logvolball(k):
    return k / 2 * math.log(math.pi) - math.lgamma(k / 2 + 1)


def logkappa(k):
    return math.log(Eg(k)) + 0.5 * math.log(2 * math.pi * math.e) - logvolball(k) / k


def E_norm(lams):
    """E||X|| for X ~ N(0, diag(lams)), by sqrt(a) = (1/(2 sqrt pi)) int_0^inf (1 - e^{-ta}) t^{-3/2} dt, t = e^u."""
    lam = np.asarray(lams, float)
    lam = lam[lam > 0]
    if lam.size == 0:
        return 0.0
    sc = lam.max()
    l = lam / sc
    def f(u):
        t = math.exp(u)
        lg = -0.5 * np.log1p(2 * t * l).sum()
        return -math.expm1(lg) * math.exp(-u / 2)
    val, err = integrate.quad(f, -60, 60, limit=400, epsabs=0, epsrel=1e-11, points=[-math.log(2 * l.sum()), 0.0, -math.log(2 * l.min()) if l.min() > 0 else 0.0])
    return val / (2 * math.sqrt(math.pi)) * math.sqrt(sc)


# ---------------- proved floors (consolidated proofs) and the conjectured floor, H in nats ----------------
def T1(gam, H):
    g = np.sort(np.asarray(gam, float))[::-1]
    g = g[g > 0]
    lg = np.cumsum(np.log(g))
    ks = np.arange(1, len(g) + 1)
    lk = np.array([logkappa(int(k)) for k in ks])
    return float(np.exp(lk + lg / (2 * ks) - H / ks).max())


def _window_tail(a, c, start):
    """max over contiguous windows [i, j) with i >= start of sum a - c sqrt(sum a^2), floored at 0 (empty T)."""
    n = len(a)
    s1 = np.concatenate([[0], np.cumsum(a)])
    s2 = np.concatenate([[0], np.cumsum(a * a)])
    best = 0.0
    for i in range(start, n):
        v = (s1[i + 1:] - s1[i]) - c * np.sqrt(s2[i + 1:] - s2[i])
        best = max(best, float(v.max()))
    return best


def T2(gam, H):
    a = np.sqrt(np.sort(np.asarray(gam, float))[::-1])
    return _window_tail(a, math.sqrt(2 * H) + 1 / math.sqrt(2), 0)


def T3(gam, H, kmax=None):
    """Theorem 3 with S = top k and T a contiguous window after k. B + 1 bits -> H + ln 2 nats."""
    g = np.sort(np.asarray(gam, float))[::-1]
    a = np.sqrt(g)
    n = len(g)
    H1 = H + math.log(2)
    c = math.sqrt(2 * H1) + 1 / math.sqrt(2)
    s1 = np.concatenate([[0], np.cumsum(a)])
    s2 = np.concatenate([[0], np.cumsum(g)])
    # best window starting at >= i, for every i (suffix maximum)
    win_from = np.zeros(n + 1)
    for i in range(n - 1, -1, -1):
        v = (s1[i + 1:] - s1[i]) - c * np.sqrt(s2[i + 1:] - s2[i])
        win_from[i] = max(win_from[i + 1], float(v.max()))
    lg = np.cumsum(np.log(np.maximum(g, 1e-300)))
    best = 0.0
    for k in range(1, (kmax or n) + 1):
        head = math.exp(logkappa(k) - 0.5 + lg[k - 1] / (2 * k) - H1 / k)
        best = max(best, head + win_from[k])
    return best


def conj_floor(gam, H):
    """sum sqrt(min(gam, theta)) with sum (1/2) ln+(gam/theta) = H (nats; equal to the bits form with B = H/ln 2)."""
    g = np.asarray(gam, float)
    g = g[g > 0]
    if H <= 0:
        return float(np.sqrt(g).sum())
    lo, hi = math.log(g.min()) - 50 - 2 * H, math.log(g.max())
    for _ in range(300):
        mid = 0.5 * (lo + hi)
        if 0.5 * np.log(np.maximum(g / math.exp(mid), 1)).sum() > H:
            lo = mid
        else:
            hi = mid
    return float(np.sqrt(np.minimum(g, math.exp(hi))).sum())


def hybrid_T3_phi(gam, H):
    """CONDITIONAL on the per-cell conjecture: Theorem 3's head with Phi on the remaining indices for the tail."""
    g = np.sort(np.asarray(gam, float))[::-1]
    n = len(g)
    H1 = H + math.log(2)
    lg = np.cumsum(np.log(np.maximum(g, 1e-300)))
    best = Phi(g, H)
    ks = sorted(set([1, 2, 3, 5, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256, 384, 512, n - 1]) & set(range(1, n)))
    for k in ks:
        head = math.exp(logkappa(k) - 0.5 + lg[k - 1] / (2 * k) - H1 / k)
        best = max(best, head + Phi(g[k:], H1))
    return best


def standard_run_header(name):
    print(f"# {name}: numpy {np.__version__}, scipy {__import__('scipy').__version__}")


_GLX, _GLW = np.polynomial.legendre.leggauss(30)


def log_mass(a, b):
    """ln P[a <= Z <= b] for Z ~ N(0,1), accurate to near machine precision for any interval (vectorised).
    Narrow intervals (b - a < 1): 30-point Gauss-Legendre of phi in log space (no cancellation).
    Wider intervals: difference of log-survival functions on the far side of the origin."""
    a = np.atleast_1d(np.asarray(a, float)); b = np.atleast_1d(np.asarray(b, float))
    out = np.empty_like(a)
    w = b - a
    nar = w < 1
    if nar.any():
        an, wn = a[nar], w[nar]
        x = an[:, None] + 0.5 * wn[:, None] * (_GLX[None, :] + 1)
        lf = -0.5 * x * x
        mx = lf.max(1)
        out[nar] = np.log(0.5 * wn) - 0.5 * math.log(2 * math.pi) + mx + np.log((_GLW[None, :] * np.exp(lf - mx[:, None])).sum(1))
    wd = ~nar
    if wd.any():
        aw, bw = a[wd], b[wd]
        flip = (aw + bw) < 0
        a2, b2 = np.where(flip, -bw, aw), np.where(flip, -aw, bw)
        La, Lb = special.log_ndtr(-a2), special.log_ndtr(-b2)
        out[wd] = La + np.log(-np.expm1(Lb - La))
    return out


class WFloor:
    """W(H) = sum_i sqrt(min(gam_i, theta)), sum_i (1/2) ln+(gam_i/theta) = H (nats), in O(n) per call:
    with k values above the water level, ln theta = (sum_{i<=k} ln gam_i - 2H)/k, and k is the one index where
    gam_{k+1} <= theta <= gam_k."""
    def __init__(self, gam):
        g = np.sort(np.asarray(gam, float))[::-1]
        self.g = g[g > 0]
        self.lg = np.log(self.g)
        self.L = np.cumsum(self.lg)
        self.ks = np.arange(1, len(self.g) + 1)
        self.suf = np.concatenate([np.cumsum(np.sqrt(self.g)[::-1])[::-1][1:], [0.0]])   # sum_{i>k} sqrt(g_i), k = 1..n
        self.total = float(np.sqrt(self.g).sum())

    def __call__(self, H):
        if H <= 0 or len(self.g) == 0:
            return self.total
        lt = (self.L - 2 * H) / self.ks
        nxt = np.concatenate([self.lg[1:], [-np.inf]])
        ok = (lt <= self.lg + 1e-12) & (lt >= nxt - 1e-12)
        k = int(np.argmax(ok))
        return float((k + 1) * math.exp(lt[k] / 2) + self.suf[k])
