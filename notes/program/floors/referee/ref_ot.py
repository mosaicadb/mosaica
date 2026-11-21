# Referee's attacks on B2 to B5 of CLAIMS.md.  Small numpy/scipy only.
# Run:  nice -n 10 taskset -c 2 env OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 ref_ot.py
import math
import numpy as np
from scipy import optimize

rng = np.random.default_rng(11)


def psd(n, cond=3.0):
    Q, _ = np.linalg.qr(rng.standard_normal((n, n)))
    return (Q * np.exp(rng.normal(0, cond / 2, n))) @ Q.T


def fpow(S, p):
    w, V = np.linalg.eigh(S); return (V * np.maximum(w, 0) ** p) @ V.T


def fid(SQ, SR):
    h = fpow(SQ, 0.5); return float(np.sqrt(np.maximum(np.linalg.eigvalsh(h @ SR @ h), 0)).sum())


def gm_metric(SQ, SR):
    h = fpow(SQ, 0.5); hi = np.linalg.inv(h); return hi @ fpow(h @ SR @ h, 0.5) @ hi


def omega2(M, S, SR):
    return float(np.trace(M @ S) * np.trace(np.linalg.inv(M) @ SR))


def geo(A, B, t):                      # the geodesic A #_t B
    h = fpow(A, 0.5); hi = np.linalg.inv(h); return h @ fpow(hi @ B @ hi, t) @ h


# ------------------------------------------------------------ B2: the discrete example of "what is not claimed"
D = np.array([[1, 0], [-1, 0], [0, 1], [0, -1]], float)
Rr = np.array([[1, 1], [1, -1], [-1, 1], [-1, -1]], float) / math.sqrt(2)
l1 = np.abs(D).sum(1).mean() * np.abs(Rr).max(1).mean()
sound = min(np.abs(d).sum() * np.abs(r).max() - d @ r for d in D for r in Rr)
best_mean = 9e9; best_rms = 9e9
for t in range(20000):
    M = psd(2, rng.uniform(0, 4)); Mi = np.linalg.inv(M)
    a = np.sqrt(np.einsum("ij,jk,ik->i", D, M, D)); b = np.sqrt(np.einsum("ij,jk,ik->i", Rr, Mi, Rr))
    best_mean = min(best_mean, a.mean() * b.mean()); best_rms = min(best_rms, math.sqrt((a * a).mean() * (b * b).mean()))
print("B2 example: l1/l_inf mean certificate = %.4f (1/sqrt2 = %.4f), soundness margin %.1e; 20000 random ellipsoids: least mean product %.4f, least rms product %.4f"
      % (l1, 1 / math.sqrt(2), sound, best_mean, best_rms))

# ------------------------------------------------------------ B2b: product forms that are not norms, Gaussian pair
# a(D) b(r) with a = |D|_M^p * c1, b = ... is not sound in general; a sound non-norm family: a = phi-type pairs are B3.
# Here: the M-ellipsoid against per-pair optimal block weights is B4a; nothing further to attack numerically.

# ------------------------------------------------------------ B4b.3: when is the bound A attained?
e1, e2 = np.diag([1.0, 0.0]), np.diag([0.0, 1.0])
def price(S1, S2, SR): return fid(0.5 * S1 + 0.5 * S2, SR) ** 2 / (0.5 * fid(S1, SR) ** 2 + 0.5 * fid(S2, SR) ** 2)
for c in (0.0, 0.3, 0.6, 0.9):
    SR = np.array([[1.0, c], [c, 1.0]])
    print("B4b.3: two rank-one families on orthogonal supports e1, e2, Sigma_R = [[1,c],[c,1]], c = %.1f: price^2 = %.4f (bound 2); e1^T Sigma_R e2 = %.1f"
          % (c, price(e1, e2, SR), c))
# Sigma_R-orthogonal supports that are not orthogonal: X = u u^T, Y = v v^T with u^T Sigma_R v = 0
SR = np.array([[1.0, 0.6], [0.6, 1.0]]); u = np.array([1.0, 0.0]); v = np.linalg.solve(SR, np.array([0.0, 1.0]))
X, Y = np.outer(u, u), np.outer(v, v); sx, sy = fid(X, SR) ** 2, fid(Y, SR) ** 2
print("       u^T Sigma_R v = %.1e, u.v = %.3f, equalised w F^2: price^2 = %.4f" % (u @ SR @ v, u @ v, price(X / sx, Y / sy, SR)))

# ------------------------------------------------------------ B4b closing remark: is the mixture's mean width convex?
# f(M) = sum_a w_a sqrt(tr(M S_a) tr(M^-1 SR)).  Euclidean convexity fails (f is scale-invariant); geodesic convexity?
worst_g = -9e9; worst_e = -9e9; nt = 0
for t in range(3000):
    n = int(rng.integers(2, 6)); A = int(rng.integers(1, 4)); SR = psd(n); Ss = [psd(n) for _ in range(A)]; w = rng.dirichlet(np.ones(A))
    f = lambda M: sum(wi * math.sqrt(omega2(M, S, SR)) for wi, S in zip(w, Ss))
    M0, M1 = psd(n, 4.0), psd(n, 4.0); tt = rng.uniform(0.05, 0.95)
    chord = (1 - tt) * f(M0) + tt * f(M1)
    worst_g = max(worst_g, (f(geo(M0, M1, tt)) - chord) / chord)
    worst_e = max(worst_e, (f((1 - tt) * M0 + tt * M1) - chord) / chord); nt += 1
print("B4b remark: %d random pairs of metrics: max [f(M0 #_t M1) - chord]/chord = %.3e (geodesically convex if <= 0); "
      "max [f((1-t)M0 + tM1) - chord]/chord = %.3e (Euclidean convexity fails if > 0)" % (nt, worst_g, worst_e))
# and local minima coincide: descend from several starts (log-Cholesky parametrisation, scale fixed by the objective's invariance)
def from_vec(x, n):
    L = np.zeros((n, n)); L[np.tril_indices(n)] = np.clip(x, -30, 30); L[np.diag_indices(n)] = np.exp(np.clip(np.diag(L), -12, 12)); return L @ L.T
spread = 0.0
for t in range(12):
    n = int(rng.integers(2, 5)); A = 3; SR = psd(n); Ss = [psd(n) for _ in range(A)]; w = rng.dirichlet(np.ones(A))
    def f(x):
        M = from_vec(x, n); v = [omega2(M, S, SR) for S in Ss]
        return sum(wi * math.sqrt(vi) for wi, vi in zip(w, v)) if all(np.isfinite(v)) and min(v) > 0 else 1e30
    def descend(x):
        best = f(x)
        for _ in range(8):                                       # BFGS, then a simplex polish, repeated until it stops improving
            x = optimize.minimize(f, x, method="BFGS", options={"gtol": 1e-10}).x
            x = optimize.minimize(f, x, method="Nelder-Mead", options={"xatol": 1e-9, "fatol": 1e-13, "maxiter": 4000}).x
            if best - f(x) < 1e-11 * best: break
            best = f(x)
        return f(x)
    vals = [descend(rng.normal(0, 1.0, n * (n + 1) // 2)) for _ in range(6)]
    Mw = gm_metric(sum(wi * S for wi, S in zip(w, Ss)), SR)
    spread = max(spread, (max(vals) - min(vals)) / min(vals))
    if t == 0:
        print("       example: least mean width over 6 starts %.6f .. %.6f; at the mean-square metric M_w: %.6f; sum_a w_a F_a = %.6f"
              % (min(vals), max(vals), sum(wi * math.sqrt(omega2(Mw, S, SR)) for wi, S in zip(w, Ss)), sum(wi * fid(S, SR) for wi, S in zip(w, Ss))))
print("       12 instances, 6 random starts each: largest relative spread of the minima found = %.2e" % spread)

# ------------------------------------------------------------ B4b.6: second order for a, first order for b
n = 4; SR = psd(n); Sa, Sb = psd(n), psd(n); Fa, Fb = fid(Sa, SR), fid(Sb, SR)
Ma = gm_metric(Sa, SR)
print("B4b.6: eps | omega_a(M_eps)^2/F_a^2 - 1 | [omega_b(M_0)^2 - omega_b(M_eps)^2]/F_b^2")
for eps in (0.2, 0.1, 0.05, 0.025, 0.0125):
    Me = gm_metric((1 - eps) * Sa + eps * Sb, SR)
    print("       %.4f | %.3e | %.3e" % (eps, omega2(Me, Sa, SR) / Fa ** 2 - 1, (omega2(Ma, Sb, SR) - omega2(Me, Sb, SR)) / Fb ** 2))

# ------------------------------------------------------------ B4b.7: minimax, primal by direct minimisation, dual on the simplex
gap = 0.0; gap2 = -9e9; lo, hi = 9e9, 0.0
for t in range(15):
    n = int(rng.integers(2, 5)); A = int(rng.integers(2, 4)); SR = psd(n); Ss = [psd(n) for _ in range(A)]
    Xs = [S / fid(S, SR) ** 2 for S in Ss]
    def dual(y):
        w = np.exp(y - y.max()); w /= w.sum(); return -fid(sum(wi * X for wi, X in zip(w, Xs)), SR) ** 2
    res = min((optimize.minimize(dual, rng.normal(0, 1, A), method="Nelder-Mead", options={"xatol": 1e-11, "fatol": 1e-14, "maxiter": 40000}) for _ in range(3)), key=lambda r: r.fun)
    dv = -res.fun; w = np.exp(res.x - res.x.max()); w /= w.sum()
    Mstar = gm_metric(sum(wi * X for wi, X in zip(w, Xs)), SR)
    pv = max(omega2(Mstar, X, SR) for X in Xs)                  # primal value at the claimed optimal metric
    gap = max(gap, (pv - dv) / dv); lo, hi = min(lo, dv), max(hi, dv / A)
    # direct primal: minimise tau subject to omega_a(M)^2 <= tau, from the claimed optimum perturbed and from random starts
    m = n * (n + 1) // 2
    cons = [{"type": "ineq", "fun": (lambda z, X=X: z[-1] - omega2(from_vec(z[:m], n), X, SR))} for X in Xs]
    best = 9e9
    for st in range(4):
        x0 = rng.normal(0, 0.7, m); z0 = np.append(x0, max(omega2(from_vec(x0, n), X, SR) for X in Xs))
        r = optimize.minimize(lambda z: z[-1], z0, constraints=cons, method="SLSQP", options={"ftol": 1e-13, "maxiter": 2000})
        val = max(omega2(from_vec(r.x[:m], n), X, SR) for X in Xs); best = min(best, val)
    gap2 = max(gap2, (dv - best) / dv)
print("B4b.7: 15 instances: max [max_a reg^2 at the metric of the least favourable mixture - max_w F(S_w)^2] / value = %.2e; "
      "max [dual value - best primal found by direct minimisation] / value = %.2e (weak duality needs <= 0); least value %.4f (>= 1); largest value / A = %.4f (<= 1)" % (gap, gap2, lo, hi))

# ------------------------------------------------------------ B5: which factor separates rule 1's rho^2 from the true ratio
w5 = 0.0
for t in range(200):
    n = int(rng.integers(2, 8)); SQ, SE = psd(n), psd(n)
    pi_, v = np.diag(SQ), np.diag(SE); a = np.sqrt(pi_ * v); F = fid(SQ, SE); tr = float(np.trace(SQ @ SE))
    rho2 = (a * a).sum() / a.sum() ** 2; true = tr / a.sum() ** 2; neff = F * F / tr
    w5 = max(w5, abs(rho2 / true - (pi_ * v).sum() / tr), abs(true * neff - (F / a.sum()) ** 2))
    if t == 0:
        NS = 100000; Dl = rng.standard_normal((NS, n)) @ fpow(SQ, 0.5); E = rng.standard_normal((NS, n)) @ fpow(SE, 0.5)
        Dw = (v / pi_) ** 0.25; h = np.linalg.norm(Dl * Dw, axis=1) * np.linalg.norm(E / Dw, axis=1); s = (Dl * E).sum(1)
        print("B5 example (n = %d, Gaussian, independent): rule 1's rho^2 = %.4f; E(D.e)^2/E h^2 = %.4f (sample %.4f); 1/n_eff = %.4f; "
              "mean of (D.e)^2/h^2 over pairs = %.4f" % (n, rho2, true, (s * s).mean() / (h * h).mean(), 1 / neff, ((s / h) ** 2).mean()))
print("B5: 200 pairs: rho^2/true = sum pi v / tr(SQ SE) and true * n_eff = (F/sum a)^2 hold to %.1e" % w5)
