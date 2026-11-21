# Referee checks that need no cell geometry.
#   (a) L5: kappa_k - k for every k <= 5000 and a log grid to 10^7
#   (b) L3: the water-filling closed form against a numerical minimiser, with ties, H = 0 and large H
#   (c) L4: Phi <= W, the bounds of items 3 and 4, convexity of W, Phi and V_S on a grid of H
#   (d) F2a: V_S(H) for the water-filling head against kappa_k sqrt(theta) + sum_{i>k} sqrt(gam_i), by brute force
#       over u, with repeated eigenvalues inside the head, at the head's edge and at the top of the tail
#   (e) high dimension: one head coordinate on a fine uniform grid times thin shells on an isotropic tail (exact
#       through the chi law).  Product codes cannot fall below F2 if Theorem 1 holds, so this shows how tight F2a
#       is, and it tests W ("the tail in full") to within about (ln n)/n.
import math
import sys
import numpy as np
from scipy.optimize import minimize
from scipy.special import gammainc, ndtr
from core_f2_3d import log_kappa, Wf, VS

rng = np.random.default_rng(11)

# (a)
worst = min((math.exp(log_kappa(k)) - k, k) for k in list(range(1, 5001)) + [int(10 ** (x / 4)) for x in range(15, 29)])
print("(a) min over k of kappa_k - k = %.4f at k = %d; kappa_1 = %.5f (sqrt e = %.5f)"
      % (worst[0], worst[1], math.exp(log_kappa(1)), math.sqrt(math.e)))

# (b)
bad = 0.0
for t in range(300):
    n = int(rng.integers(1, 6))
    a = np.exp(rng.normal(0, 1.5, n))
    if t % 3 == 0 and n > 1:
        a[1] = a[0]                       # a tie
    H = [0.0, 0.3, 2.0, 12.0][t % 4]
    cons = {"type": "ineq", "fun": lambda v: H - v.sum()}      # v_i = ln(1/s_i) >= 0
    best = np.inf
    for s in range(4):
        v0 = rng.random(n) * H / n
        r = minimize(lambda v: float((a * np.exp(-v)).sum()), v0, bounds=[(0, None)] * n, constraints=[cons],
                     method="SLSQP")
        if r.success and r.x.sum() <= H + 1e-7:
            best = min(best, r.fun)
    bad = max(bad, abs(best - Wf(a, H)) / Wf(a, H))
    bad_low = (Wf(a, H) - best) / Wf(a, H)       # the minimiser must not go below the closed form
    assert bad_low < 1e-5, (a, H, best, Wf(a, H))
print("(b) L3 closed form against SLSQP on 300 programmes (ties, H = 0, 0.3, 2, 12): max relative difference %.1e" % bad)


# (c)
def kfun(s):
    return 0.5 * (s * s - 1) - np.log(s)


def Phi(a, H):
    a = np.asarray(a, float)
    if H <= 0:
        return float(a.sum())
    lo, hi = 1e-200, 1e200
    for _ in range(300):
        mid = math.sqrt(lo * hi)
        x = a / mid
        s = 2.0 / (x + np.sqrt(x * x + 4))
        if kfun(s).sum() > H:
            lo = mid
        else:
            hi = mid
    x = a / hi
    s = 2.0 / (x + np.sqrt(x * x + 4))
    return float((a * s).sum())


w23 = w4 = wconv = np.inf
for t in range(400):
    n = int(rng.integers(1, 9))
    a = np.sort(np.exp(rng.normal(0, 1.2, n)))[::-1]
    if t % 4 == 0 and n > 2:
        a[2] = a[1]
    F, S2 = a.sum(), (a * a).sum()
    Hs = np.concatenate([[0.0], np.exp(rng.uniform(-6, 3, 12))])
    for H in Hs:
        W_, P_ = Wf(a, H), Phi(a, H)
        w23 = min(w23, (W_ - P_) / F, (P_ - (F - math.sqrt(H * S2))) / F)
        w4 = min(w4, (W_ - (F - min(a[0] * H, math.sqrt(H * S2)))) / F)
        if n > 1 and H <= math.log(a[0] / a[1]):
            assert abs(W_ - (F - a[0] * (1 - math.exp(-H)))) < 1e-10 * F
    S = tuple(range(int(rng.integers(0, n + 1))))
    for f in (lambda H: Wf(a, H), lambda H: Phi(a, H), lambda H: VS(a, S, H)):
        H1, H2 = np.exp(rng.uniform(-4, 2.5, 2))
        mid = f(0.5 * (H1 + H2))
        wconv = min(wconv, (0.5 * (f(H1) + f(H2)) - mid) / F)
        assert f(max(H1, H2)) <= f(min(H1, H2)) + 1e-9 * F
print("(c) L4 on 400 spectra: min of (W - Phi)/F and (Phi - bound 3)/F = %.2e; min (W - bound 4)/F = %.2e; "
      "convexity defect of W, Phi, V_S >= %.1e (all need >= 0 up to rounding)" % (w23, w4, wconv))

# (d)
worst_d = 0.0
cases = [np.array([4.0, 1.0, 1.0]), np.array([4.0, 4.0, 1.0, 1.0, 0.25]), np.array([9.0, 4.0, 4.0, 4.0, 1.0]),
         np.array([1.0, 1.0, 1.0, 1.0]), np.array([16.0, 4.0, 1.0, 1.0, 1.0, 0.01])]
for g in cases + [np.sort(np.exp(rng.normal(0, 2, int(rng.integers(2, 8)))))[::-1] for _ in range(60)]:
    a = np.sqrt(g)
    n = a.size
    for H in [0.05, 0.3, 0.5 * math.log(4.0), math.log(2.0), 1.0, 3.0, 8.0]:
        # water-filling: theta, k = #{gam_i > theta}
        la = np.log(a)
        for kk in range(1, n + 1):
            lam = math.exp((la[:kk].sum() - H) / kk)
            if kk == n or lam >= a[kk]:
                break
        k = int((a > lam * (1 + 1e-12)).sum())
        if k == 0:
            continue
        claim = math.exp(log_kappa(k)) * lam + a[k:].sum()
        us = np.linspace(0, H, 4001)
        lhead = log_kappa(k) + la[:k].sum() / k
        brute = min(math.exp(lhead - (H - u) / k) + Wf(a[k:], u) for u in us)
        worst_d = max(worst_d, abs(brute - claim) / claim)
        assert brute >= claim * (1 - 1e-9), (g, H, k, brute, claim)
print("(d) F2a: brute-force minimum over u against kappa_k sqrt(theta) + tail, 65 spectra x 7 rates, ties included: "
      "max relative difference %.1e (the minimum is at u = 0 in every case)" % worst_d)


# (e)
def chi_cdf(r, n):
    return gammainc(n / 2.0, r * r / 2.0)


def shells(n, delta):
    """Shells of thickness delta on |z|, z ~ N(0, I_n).  Returns (sum p_j rho_j, entropy)."""
    r0 = math.sqrt(n)
    lo = max(0.0, r0 - 12.0)
    edges = np.concatenate([[0.0], np.arange(lo + delta, r0 + 12.0, delta)]) if lo > 0 else np.arange(0, r0 + 12, delta)
    p = np.diff(chi_cdf(edges, n))
    m = p > 0
    return float((p[m] * edges[1:][m]).sum()), float(-(p[m] * np.log(p[m])).sum())


def head_grid(w):
    e = np.arange(-12, 12 + w, w)
    p = np.diff(ndtr(e))
    p = p[p > 0]
    return math.sqrt(2 / math.pi) * w / 2, float(-(p * np.log(p)).sum())


print("(e) one head coordinate (sqrt gam = 1, step w) times shells of thickness delta on an isotropic tail (sqrt gam = eps)")
worst_e = (np.inf, None)
for n in (30, 300, 3000):
    Eg = math.exp(0.5 * math.log(2) + math.lgamma((n + 1) / 2) - math.lgamma(n / 2))
    for eps in (0.01, 0.001):
        for w in (0.05, 0.3):
            for delta in (2 / math.sqrt(n), 0.3, 1.0, 3.0):
                sr, Ht = shells(n, delta)
                eh1, Hh = head_grid(w)
                Eh, H = eh1 + eps * Eg * sr, Hh + Ht
                a = np.array([1.0] + [eps] * n)
                W_ = Wf(a, H)
                V1 = VS(a, (0,), H)
                Vall = math.exp(log_kappa(n + 1) + np.log(a).sum() / (n + 1) - H / (n + 1))
                # head = the tail block only, tail = the big coordinate
                lk = log_kappa(n) + math.log(eps)
                f = lambda u: math.exp(lk - (H - u) / n) + math.exp(-u)
                Vtail = min(f(u) for u in np.linspace(0, H, 2001))
                best = max(W_, V1, Vall, Vtail)
                r = Eh / best
                if r < worst_e[0]:
                    worst_e = (r, (n, eps, w, round(delta, 3)), Eh / W_, Eh / V1)
        print("    n_T=%d done; smallest Eh/max V so far %.5f at (n_T, eps, w, delta)=%s with Eh/W=%.5f, Eh/V_{head}=%.5f"
              % (n, worst_e[0], worst_e[1], worst_e[2], worst_e[3]))
        sys.stdout.flush()
# shells alone (no head): the test of W on the tail
for n in (100, 1000, 10000):
    Eg = math.exp(0.5 * math.log(2) + math.lgamma((n + 1) / 2) - math.lgamma(n / 2))
    best = (np.inf, None)
    for delta in (1 / math.sqrt(n), 2 / math.sqrt(n), 4 / math.sqrt(n), 0.5, 2.0):
        sr, Ht = shells(n, delta)
        Eh = Eg * sr
        W_ = n * math.exp(-Ht / n)
        T1 = math.exp(log_kappa(n) - Ht / n)
        if Eh / W_ < best[0]:
            best = (Eh / W_, delta, Ht, Eh - W_, Eh - T1)
    print("    shells only, n=%d: smallest Eh/W = %.6f (delta=%.3f, H=%.3f); Eh - W = %.3f, Eh - Theorem 1 = %.3f"
          % (n, best[0], best[1], best[2], best[3], best[4]))
