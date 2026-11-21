# Floors on the mean certified half-width, as functions of a spectrum gam (the gamma_i) and a rate H in nats.
# Used by every script in this directory.  Pure numpy; no data is read here.
#   T1, T2, T3      Theorems 1, 2, 3 of proofcheck/CONSOLIDATED_PROOFS.md (the refereed floors)
#   Phi             Conjecture A's function: min sum sqrt(gam_i) s_i  s.t. sum k(s_i) <= H, k(s) = (s^2-1)/2 - ln s
#   W               the water-filling floor: sum sqrt(min(gam_i, theta)),  sum (1/2) ln+(gam_i/theta) = H
#   V               the combined floor: max over head sets S = top-k of
#                   min_{0<=u<=H} kappa_k GM_S(sqrt gam) exp(-(H-u)/k) + W_{S^c}(u)
import math
import numpy as np

LN2 = math.log(2.0)


def log_kappa(k):
    # kappa_k = E|g_k| sqrt(2 pi e) omega_k^(-1/k)
    leg = 0.5 * math.log(2) + math.lgamma((k + 1) / 2) - math.lgamma(k / 2)
    lvol = k / 2 * math.log(math.pi) - math.lgamma(k / 2 + 1)
    return leg + 0.5 * math.log(2 * math.pi * math.e) - lvol / k


def kappa(k):
    return math.exp(log_kappa(k))


def _desc(gam):
    g = np.sort(np.asarray(gam, float))[::-1]
    return g[g > 0]


def W(gam, H):
    """Water-filling floor.  Returns (value, theta, k) with k = #{gam_i > theta}."""
    g = _desc(gam)
    n = len(g)
    if H <= 0:
        return float(np.sqrt(g).sum()), float(g[0]) if n else 0.0, 0
    lg = np.log(g)
    cl = np.cumsum(lg)
    ks = np.arange(1, n + 1)
    # rate needed to bring the level down to gam_{k+1} (0-based g[k]) with k coded coordinates
    nxt = np.append(lg[1:], -np.inf)
    need = 0.5 * (cl - ks * nxt)          # increasing in k
    k = int(np.searchsorted(need, H, side="left")) + 1   # smallest k with need[k-1] >= H
    k = min(k, n)
    lth = (cl[k - 1] - 2 * H) / k
    th = math.exp(lth)
    val = k * math.sqrt(th) + float(np.sqrt(g[k:]).sum())
    return val, th, k


def kfun(s):
    return 0.5 * (s * s - 1) - np.log(s)


def Phi(gam, H):
    """Conjecture A's function.  Returns (value, lambda)."""
    g = _desc(gam)
    a0 = np.sqrt(g)
    if H <= 0:
        return float(a0.sum()), float("inf")

    def rate(lam):
        a = a0 / lam
        s = 2.0 / (a + np.sqrt(a * a + 4))        # root of s^2 + a s - 1 = 0, stable
        return float(kfun(s).sum()), s

    lo, hi = 1e-300, 1e300
    for _ in range(400):
        mid = math.sqrt(lo * hi)
        r, _s = rate(mid)
        if r > H:
            lo = mid
        else:
            hi = mid
    r, s = rate(hi)
    return float((a0 * s).sum()), hi


def T1(gam, H):
    """Theorem 1 / Corollary 1: max_k kappa_k (gam_1..gam_k)^(1/2k) exp(-H/k).  Returns (value, k)."""
    g = _desc(gam)
    n = len(g)
    cl = np.cumsum(0.5 * np.log(g))
    ks = np.arange(1, n + 1)
    lk = np.array([log_kappa(int(k)) for k in ks])
    v = lk + cl / ks - H / ks
    i = int(v.argmax())
    return float(math.exp(v[i])), i + 1


def T2(gam, H):
    """Theorem 2 with the best suffix T (suffixes are the optimal sets)."""
    g = _desc(gam)
    s1 = np.cumsum(np.sqrt(g)[::-1])[::-1]
    s2 = np.cumsum(g[::-1])[::-1]
    v = s1 - (math.sqrt(2 * H) + 1 / math.sqrt(2)) * np.sqrt(s2)
    return float(max(v.max(), 0.0))


def T3(gam, B):
    """Theorem 3 (B in bits), S = top-k, T = a suffix beyond k, best over k and the suffix."""
    g = _desc(gam)
    n = len(g)
    s1 = np.append(np.cumsum(np.sqrt(g)[::-1])[::-1], 0.0)
    s2 = np.append(np.cumsum(g[::-1])[::-1], 0.0)
    tail = s1 - (math.sqrt(2 * (B + 1) * LN2) + 1 / math.sqrt(2)) * np.sqrt(s2)
    best_from = np.maximum.accumulate(tail[::-1])[::-1]       # best suffix starting at index >= j
    cl = np.cumsum(0.5 * np.log(g))
    ks = np.arange(1, n + 1)
    lk = np.array([log_kappa(int(k)) for k in ks])
    head = np.exp(lk - 0.5 + cl / ks - (B + 1) * LN2 / ks)
    return float((head + np.maximum(best_from[1:], 0.0)).max())


class Spectrum:
    """Prefix sums of a sorted spectrum, so that W on a suffix {k+1..n} costs O(log n)."""

    def __init__(self, gam):
        self.g = _desc(gam)
        self.n = len(self.g)
        self.lg = np.log(self.g)
        self.cl = np.concatenate([[0.0], np.cumsum(self.lg)])           # cl[j] = sum_{i<j} ln g_i
        self.cs = np.concatenate([[0.0], np.cumsum(np.sqrt(self.g))])   # cs[j] = sum_{i<j} sqrt g_i
        self.F = float(self.cs[-1])
        self.lk = np.array([0.0] + [log_kappa(k) for k in range(1, self.n + 1)])

    def W_suffix(self, k0, u):
        """W on coordinates k0..n-1 (0-based) at rate u nats."""
        n = self.n
        if k0 >= n:
            return 0.0
        if u <= 0:
            return self.cs[n] - self.cs[k0]
        # m coded coordinates k0..k0+m-1; need(m) = 0.5*(sum ln g - m ln g_{k0+m})
        lo, hi = 1, n - k0
        def need(m):
            nxt = self.lg[k0 + m] if k0 + m < n else -np.inf
            return 0.5 * ((self.cl[k0 + m] - self.cl[k0]) - m * nxt)
        while lo < hi:
            mid = (lo + hi) // 2
            if need(mid) >= u:
                hi = mid
            else:
                lo = mid + 1
        m = lo
        lth = ((self.cl[k0 + m] - self.cl[k0]) - 2 * u) / m
        return m * math.exp(0.5 * lth) + (self.cs[n] - self.cs[k0 + m])

    def V_topk(self, k, H, iters=80):
        """min_{0<=u<=H} kappa_k GM_k exp(-(H-u)/k) + W_{k+1..n}(u).  Returns (value, u*)."""
        if k == 0:
            return self.W_suffix(0, H), H
        lhead = self.lk[k] + 0.5 * self.cl[k] / k
        f = lambda u: math.exp(lhead - (H - u) / k) + self.W_suffix(k, u)
        if k >= self.n:
            return f(0.0), 0.0
        a, b = 0.0, H
        gr = (math.sqrt(5) - 1) / 2
        c, d = b - gr * (b - a), a + gr * (b - a)
        fc, fd = f(c), f(d)
        for _ in range(iters):
            if fc < fd:
                b, d, fd = d, c, fc
                c = b - gr * (b - a)
                fc = f(c)
            else:
                a, c, fc = c, d, fd
                d = a + gr * (b - a)
                fd = f(d)
        u = (a + b) / 2
        cand = [(f(0.0), 0.0), (f(H), H), (f(u), u)]
        return min(cand)

    def V(self, H, ks=None):
        """Combined floor over head sets S = top-k.  Returns (value, k, u*)."""
        best = (-1.0, 0, 0.0)
        for k in (range(0, self.n + 1) if ks is None else ks):
            v, u = self.V_topk(k, H)
            if v > best[0]:
                best = (v, k, u)
        return best
