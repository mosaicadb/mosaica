# Checks for the repairs made after the referees' reports (CLAIMS.md, "Changes after the referees").
# Small instances only; numpy and scipy.special; peak resident memory is printed at the end.
# Run:  nice -n 10 taskset -c 5 env OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 check_after_referees.py
import math
import resource
import numpy as np
from scipy.special import ndtr, ndtri
import floors_lib as fl

rng = np.random.default_rng(20261121)
E1 = math.sqrt(2 / math.pi)


def psd(n, spread=1.5):
    Q, _ = np.linalg.qr(rng.standard_normal((n, n)))
    return (Q * np.exp(rng.normal(0, spread / 2, n))) @ Q.T


def sqrtm(S):
    w, V = np.linalg.eigh(S); return (V * np.sqrt(np.maximum(w, 0))) @ V.T


def powm(S, t):
    w, V = np.linalg.eigh(S); return (V * np.maximum(w, 1e-300) ** t) @ V.T


def fid(SQ, SR):
    h = sqrtm(SQ); return float(np.sqrt(np.maximum(np.linalg.eigvalsh(h @ SR @ h), 0)).sum())


def gmetric(SQ, SR):                      # SQ^-1 # SR
    h = sqrtm(SQ); hi = np.linalg.inv(h); return hi @ sqrtm(h @ SR @ h) @ hi


def geo(M0, M1, t):                       # the geodesic point M0 #_t M1
    h = sqrtm(M0); hi = np.linalg.inv(h); return h @ powm(hi @ M1 @ hi, t) @ h


def width(M, S, SR): return math.sqrt(np.trace(M @ S) * np.trace(np.linalg.inv(M) @ SR))


# ---- 1. L4 item 6 (the sum bound) and "Theorems 2 and 3 follow from F1 and F2"
w_sum = 9e9; w_t2 = 9e9; w_t3 = 9e9
for t in range(400):
    n = int(rng.integers(2, 40)); gam = np.sort(np.exp(rng.normal(0, rng.uniform(0.2, 2.5), n)))[::-1]
    if t % 5 == 0: gam[1:1 + n // 3] = gam[1]                      # ties
    gam = np.sort(gam)[::-1]; sp = fl.Spectrum(gam); B = float(rng.uniform(0, 3 * n)); H = B * fl.LN2
    best = 0.0
    for k in range(0, n + 1):
        v, u = sp.V_topk(k, H)
        head = math.exp(sp.lk[k] + 0.5 * sp.cl[k] / k - H / k) if k else 0.0
        s = head + sp.W_suffix(k, H)
        w_sum = min(w_sum, v - s); best = max(best, s)
    w_t2 = min(w_t2, sp.W_suffix(0, H) - fl.T2(gam, H)); w_t3 = min(w_t3, best - fl.T3(gam, B))
print("1. 400 spectra (ties included), every top-k head: min [V_S(H) - (head(H) + W_T(H))] = %.2e (>= 0, L4.6); "
      "min [W - Theorem 2] = %.3e (>= 0); min [max_k (head + W_T) - Theorem 3] = %.3e (>= 0)" % (w_sum, w_t2, w_t3))

# ---- 2. F2a at a tie: gamma = (4, 1, 1), H = ln 2
gam = np.array([4.0, 1.0, 1.0]); H = math.log(2); sp = fl.Spectrum(gam); w, th, k = fl.W(gam, H)
print("2. gamma = (4,1,1), H = ln 2: theta = %.4f, water-filling head k = %d, W = %.4f, F2a = kappa_1 sqrt(theta) + 2 = %.4f, "
      "F2 with S = all = kappa_3 GM e^(-H/3) = %.4f; V over top-k: %s" % (th, k, w, fl.kappa(1) * math.sqrt(th) + 2, sp.V_topk(3, H)[0],
                                                                           ", ".join("k=%d: %.4f" % (j, sp.V_topk(j, H)[0]) for j in range(4))))

# ---- 3. Proposition A(a): a cell law whose correlation exceeds 1 (N(0,1) restricted to |z| >= a)
x = np.linspace(1e-9, 9, 200001); dx = x[1] - x[0]
for a in (1.0, 2.0, 3.0):
    u = 2 * ndtr(x) - 1                                            # the law of |g|
    y = ndtri(np.minimum(ndtr(a) + u * (1 - ndtr(a)), 1 - 1e-16))  # |T(g)|, the monotone map on each side
    C = float((x * y * 2 * np.exp(-x * x / 2) / math.sqrt(2 * math.pi)).sum() * dx)
    P = 2 * (1 - ndtr(a)); kk = lambda s: 0.5 * (s * s - 1) - math.log(s)
    print("3. cell |z| >= %.0f: P = %.4f, C = %.4f > 1, k(C) = %.4f, k(min(C,1)) = %.4f <= k(C) <= KL = ln(1/P) = %.4f"
          % (a, P, C, kk(C), kk(min(C, 1.0)), math.log(1 / P)))

# ---- 4. F3 by hand: one coordinate, uniform law of unit variance, uniform grid of m cells
J = 0.5 * math.log(2 * math.pi * math.e) - math.log(2 * math.sqrt(3))
for m in (1, 4, 64):
    print("4. uniform law, %2d cells: E h = E|g| sqrt3/m = %.6f; kappa_1 e^-(ln m + J) = %.6f (J = %.4f nats); Gaussian floor without J = %.6f"
          % (m, E1 * math.sqrt(3) / m, fl.kappa(1) * math.exp(-(math.log(m) + J)), J, fl.kappa(1) / m))

# ---- 5. B3e: the gain of the best phi + phi* is not bounded away from F (lognormal scale, Hoelder)
n = 16; a = np.sqrt(np.exp(-0.2 * np.arange(n))); F = a.sum(); NS = 40000
X = np.sqrt((rng.standard_normal((NS, n)) ** 2) @ a); Y0 = np.sqrt((rng.standard_normal((NS, n)) ** 2) @ a); xi = rng.standard_normal(NS)
for s2 in (0.28, 1.0, 4.0, 9.0):
    s = np.exp(math.sqrt(s2) * xi - s2)                            # E s^2 = 1
    C1 = float((np.sort(X) * np.sort(s * Y0)).mean())
    hold = (X ** 4).mean() ** 0.25 * math.exp(-s2 / 3) * (Y0 ** (4 / 3)).mean() ** 0.75
    print("5. sigma^2 = %.2f: comonotone bound C_1(X,Y)/F = %.4f (sample of %d); Hoelder bound |X|_4 e^(-sigma^2/3) |Y_0|_(4/3) / F = %.4f; "
          "so the gain is at least %.4f F" % (s2, C1 / F, NS, hold / F, 1 - min(C1, hold) / F))

# ---- 6. B4b item 3: the price A is attained by supports orthogonal in the Sigma_R inner product, not the Euclidean one
#         (and equal w_a F_a^2: v is scaled so that F(vv') = F(uu'))
e1, e2 = np.array([1.0, 0.0]), np.array([0.0, 1.0])
def price(S1, S2, SR): return fid(0.5 * S1 + 0.5 * S2, SR) ** 2 / (0.5 * fid(S1, SR) ** 2 + 0.5 * fid(S2, SR) ** 2)
row = []
for c in (0.0, 0.3, 0.6, 0.9):
    SR = np.array([[1.0, c], [c, 1.0]]); v = np.linalg.solve(SR, e2); v = v / math.sqrt(v @ SR @ v)   # u' SR v = 0 and F_1 = F_2
    row.append("c=%.1f: Euclidean-orthogonal %.3f, Sigma_R-orthogonal %.3f (u.v = %.2f)" % (c, price(np.outer(e1, e1), np.outer(e2, e2), SR),
                                                                                         price(np.outer(e1, e1), np.outer(v, v), SR), e1 @ v / np.linalg.norm(v)))
print("6. price of one basis, two rank-one families, Sigma_R = [[1,c],[c,1]]: " + "; ".join(row))

# ---- 7. B4b item 6: second order for the diluted family, first order for the added one
Sa, Sb, SR = psd(4), psd(4), psd(4); Ma = gmetric(Sa, SR); Fa = fid(Sa, SR); wb0 = width(Ma, Sb, SR)
prev = None; rows = []
for eps in (0.2, 0.1, 0.05, 0.025, 0.0125):
    M = gmetric((1 - eps) * Sa + eps * Sb, SR)                     # minimiser of (1-eps) w_a^2 + eps w_b^2 (the sum rule)
    cost = width(M, Sa, SR) ** 2 / Fa ** 2 - 1; gain = 1 - width(M, Sb, SR) ** 2 / wb0 ** 2
    if prev: rows.append("eps %.4f: cost ratio %.2f, gain ratio %.2f" % (eps, prev[0] / cost, prev[1] / gain))
    prev = (cost, gain)
print("7. halving the share eps of family b in the fit (ratios tend to 4 and 2): " + "; ".join(rows))

# ---- 8. B4b closing remark: the mixture's mean width is convex along geodesics, not along straight lines
wg = -9e9; wl = -9e9
for t in range(600):
    n = int(rng.integers(2, 5)); A = int(rng.integers(2, 4)); SR = psd(n); Ss = [psd(n) for _ in range(A)]; w = rng.dirichlet(np.ones(A))
    f = lambda M: sum(wi * width(M, S, SR) for wi, S in zip(w, Ss))
    M0, M1 = psd(n, 3.0), psd(n, 3.0); chord = 0.5 * (f(M0) + f(M1))
    wg = max(wg, f(geo(M0, M1, 0.5)) / chord - 1); wl = max(wl, f(0.5 * (M0 + M1)) / chord - 1)
print("8. 600 random pairs of metrics: mean width at the geodesic midpoint / chord - 1 at most %.2e (<= 0: convex along geodesics); "
      "at the straight midpoint / chord - 1 up to %.3f (> 0: not convex along lines)" % (wg, wl))
# a diagonal instance: the minimiser of the mean is close to the mean-square metric M_w and is not it
q = np.array([[1.0, 0.05], [0.1, 1.0]]); lam = np.array([1.0, 0.6]); w = np.array([0.5, 0.5])
fmean = lambda d: sum(w[i] * math.sqrt((q[i, 0] * math.exp(d) + q[i, 1]) * (lam[0] * math.exp(-d) + lam[1])) for i in range(2))
lo, hi = -6.0, 6.0
for _ in range(200):                                                # the function of d is convex (item 8): ternary search
    m1, m2 = lo + (hi - lo) / 3, hi - (hi - lo) / 3
    if fmean(m1) < fmean(m2): hi = m2
    else: lo = m1
qw = w @ q; dw = 0.5 * math.log(lam[0] * qw[1] / (lam[1] * qw[0]))  # M_w = diag(sqrt(lam/q_w)), d = ln(m_1/m_2)
Fs = [math.sqrt(q[i, 0] * lam[0]) + math.sqrt(q[i, 1] * lam[1]) for i in range(2)]
print("   diagonal instance: least mean width %.6f at d = %.4f; at the mean-square metric M_w (d = %.4f) %.6f; sum_a w_a F_a = %.6f"
      % (fmean((lo + hi) / 2), (lo + hi) / 2, dw, fmean(dw), w @ np.array(Fs)))

# ---- 9. B5 closing sentence: rule 1's rho^2, the ratio of second moments, 1/n_eff, and the mean of per-pair ratios
n = 3; SQ, SE = psd(n), psd(n); NS = 200000
D = rng.standard_normal((NS, n)) @ sqrtm(SQ); E = rng.standard_normal((NS, n)) @ sqrtm(SE)
pi, v = np.diag(SQ), np.diag(SE); a = np.sqrt(pi * v); Dw = (v / pi) ** 0.25
h = np.sqrt(((D * Dw) ** 2).sum(1)) * np.sqrt(((E / Dw) ** 2).sum(1)); de = (D * E).sum(1)
print("9. n = 3, Gaussian, independent: rule 1's rho^2 = %.3f; E(D.e)^2 / E h^2 = %.3f (exact %.3f); 1/n_eff = %.3f; mean over pairs of (D.e)^2/h^2 = %.3f"
      % ((a * a).sum() / a.sum() ** 2, (de ** 2).mean() / (h ** 2).mean(), np.trace(SQ @ SE) / a.sum() ** 2,
         np.trace(SQ @ SE) / fid(SQ, SE) ** 2, ((de / h) ** 2).mean()))

# ---- 10. F6: against the floor computed from the offsets' own covariance the factor is E t / sqrt(E t^2)
for s2 in (0.05, 0.28, 1.0):
    print("10. lognormal offset scale t, sigma^2 = %.2f: E t / sqrt(E t^2) = %.4f" % (s2, math.exp(-s2 / 2)))

print("peak resident memory of this run: %.0f MB" % (resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024))
