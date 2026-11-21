# Numerical checks of the Task B claims of CLAIMS.md (B1 to B5).  Small random instances; fixed samples.
# Run:  nice -n 10 taskset -c 6 env OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 ot_checks.py
import json
import math
import os
import numpy as np

rng = np.random.default_rng(20261121)


def psd(n, cond=3.0):
    A = rng.standard_normal((n, n)); Q, _ = np.linalg.qr(A)
    return (Q * np.exp(rng.normal(0, cond / 2, n))) @ Q.T


def sqrtm(S):
    w, V = np.linalg.eigh(S); return (V * np.sqrt(np.maximum(w, 0))) @ V.T


def fid(SQ, SR):                      # F = tr (SQ^1/2 SR SQ^1/2)^1/2
    h = sqrtm(SQ); w = np.linalg.eigvalsh(h @ SR @ h); return float(np.sqrt(np.maximum(w, 0)).sum())


def gmean_metric(SQ, SR):             # M = SQ^-1 # SR
    h = sqrtm(SQ); hi = np.linalg.inv(h); return hi @ sqrtm(h @ SR @ h) @ hi


# ---------------------------------------------------------------- B1
w1 = 0.0; w2 = 9e9; w3 = -9e9
for t in range(300):
    n = int(rng.integers(2, 8)); SQ, SR = psd(n), psd(n)
    M = gmean_metric(SQ, SR); F = fid(SQ, SR)
    w1 = max(w1, np.abs(M @ SQ @ M - SR).max() / np.abs(SR).max(), abs(np.trace(M @ SQ) - F) / F, abs(np.trace(np.linalg.inv(M) @ SR) - F) / F)
    Mp = psd(n); w2 = min(w2, math.sqrt(np.trace(Mp @ SQ) * np.trace(np.linalg.inv(Mp) @ SR)) / F)
    # a random coupling: cross-covariance C = SQ^1/2 K SR^1/2 with a contraction K; its correlation is tr C
    K = rng.standard_normal((n, n)); K /= np.linalg.norm(K, 2) * rng.uniform(1, 2)
    w3 = max(w3, np.trace(sqrtm(SQ) @ K @ sqrtm(SR)) / F)
print("B1: 300 non-commuting pairs: max relative error of M SQ M = SR and tr(M SQ) = tr(M^-1 SR) = F: %.1e; "
      "min over random M' of sqrt(tr(M'SQ) tr(M'^-1 SR))/F = %.4f (>= 1); max correlation of a random coupling / F = %.4f (<= 1)" % (w1, w2, w3))

# ---------------------------------------------------------------- B2: second moments and means, for random norms
def norm_pair(n):
    """A random norm |A x|_p and its dual |A^-T y|_q."""
    p = float(rng.choice([1.0, 1.5, 2.0, 3.0, np.inf])); A = psd(n, 2.0) @ np.linalg.qr(rng.standard_normal((n, n)))[0]
    q = np.inf if p == 1 else (1.0 if p == np.inf else p / (p - 1)); Ait = np.linalg.inv(A).T
    return (lambda X: np.linalg.norm(X @ A.T, ord=p, axis=1)), (lambda Y: np.linalg.norm(Y @ Ait.T, ord=q, axis=1)), p


NS = 200000
wa = 9e9; wb = 9e9; wb_ell = 9e9; wpt = 9e9; cnt = 0
for t in range(240):
    n = int(rng.choice([1, 2, 3, 5, 8])); SQ, SR = psd(n), psd(n)
    M = gmean_metric(SQ, SR); F = fid(SQ, SR)
    D = rng.standard_normal((NS, n)) @ sqrtm(SQ); R = rng.standard_normal((NS, n)) @ sqrtm(SR)
    dM = np.sqrt(np.einsum("ij,jk,ik->i", D, M, D)); rM = np.sqrt(np.einsum("ij,jk,ik->i", R, np.linalg.inv(M), R))
    base = dM.mean() * rM.mean()                               # the mean certificate of the M-ellipsoid (independent pair)
    if t % 2 == 0:
        nk, nd, p = norm_pair(n)
    else:                                                       # another ellipsoid
        Mp = psd(n); Mpi = np.linalg.inv(Mp)
        nk = lambda X: np.sqrt(np.einsum("ij,jk,ik->i", X, Mp, X)); nd = lambda Y: np.sqrt(np.einsum("ij,jk,ik->i", Y, Mpi, Y)); p = "ell"
    a, b = nk(D), nd(R)
    wpt = min(wpt, float((a * b - np.abs((D * R).sum(1))).min() / (a * b).mean()))     # the certificate is sound, pair by pair
    wa = min(wa, math.sqrt((a * a).mean() * (b * b).mean()) / F)
    r = a.mean() * b.mean() / base
    wb = min(wb, r)
    if p == "ell": wb_ell = min(wb_ell, r)
    cnt += 1
print("B2: %d random norms (|Ax|_p with dual, and ellipsoids), n in {1,2,3,5,8}, %d sample pairs each: soundness margin min %.2e (>= 0); "
      "min sqrt(E|D|_K^2 E|r|_K*^2)/F = %.4f (>= 1 up to sampling); min [E|D|_K E|r|_K*] / [E|D|_M E|r|_M^-1] = %.4f (>= 1 up to sampling; "
      "ellipsoids only: %.4f)" % (cnt, NS, wpt, wa, wb, wb_ell))
# the value of the mean certificate: F/(1+cv^2), between F (1 - 1/n_eff)^2 and F
for n in (1, 2, 8, 64):
    gam = np.exp(-0.1 * np.arange(n)); a = np.sqrt(gam); F = a.sum(); neff = F * F / gam.sum()
    x = np.sqrt((rng.standard_normal((NS, n)) ** 2) @ a)
    print("    n = %2d, n_eff = %5.1f: (E|g''|)^2 / F = %.4f; lower bound (1 - 1/n_eff)^2 = %.4f; 1/(1 + cv^2) = %.4f"
          % (n, neff, x.mean() ** 2 / F, max(0, 1 - 1 / neff) ** 2, 1 / (1 + x.var() / x.mean() ** 2)))

# ---------------------------------------------------------------- B3: scale mixtures on a measured spectrum
HERE = os.path.dirname(os.path.abspath(__file__)); ROOT = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
spec = json.load(open(os.path.join(ROOT, "agent1", "spec_v1.json")))
gam = np.sort(np.array(spec["12"]["lam"]) * np.array(spec["12"]["Qe"]))[::-1]; a = np.sqrt(gam); F = a.sum()
N3, CH = 100000, 4000
X = np.concatenate([np.sqrt((rng.standard_normal((CH, len(a))) ** 2) @ a) for _ in range(N3 // CH)])     # |Delta|_M
Y0 = np.concatenate([np.sqrt((rng.standard_normal((CH, len(a))) ** 2) @ a) for _ in range(N3 // CH)])    # |r_0|_{M^-1}
print("B3: Gaussian offsets, residual = s * Gaussian with a lognormal scale s (E s^2 = 1), spectrum of group 12 (n_eff = %.0f)." % (F * F / gam.sum()))
print("    sigma^2 (kurtosis) | lower bound on max-cov / F = E s | upper bound = comonotone norms / F | Gelbrich gap between | product-form mean / F")
for kurt in (3.0, 3.6, 4.7, 5.1, 9.2):
    s2 = 0.25 * math.log(kurt / 3); s = np.exp(math.sqrt(s2) * rng.standard_normal(N3) - s2)             # E s^2 = 1
    Es = math.exp(-s2 / 2)
    up = float((np.sort(X) * np.sort(s * Y0)).mean()) / F
    print("    %.3f (%.1f)        | %.4f                           | %.4f                             | %.4f .. %.4f     | %.4f"
          % (s2, kurt, Es, up, 1 - up, 1 - Es, X.mean() * float(s.mean()) * Y0.mean() / F))

# ---------------------------------------------------------------- B4
# (a) block identity: sum_j |D_j||e_j| = min over block-constant D of |D Delta||D^-1 e|
wB = 0.0
for t in range(200):
    J = int(rng.integers(1, 6)); sizes = rng.integers(1, 5, J); n = int(sizes.sum()); blk = np.repeat(np.arange(J), sizes)
    d, e = rng.standard_normal(n), rng.standard_normal(n)
    an = np.sqrt(np.bincount(blk, d * d)); bn = np.sqrt(np.bincount(blk, e * e)); lhs = (an * bn).sum()
    dj = np.sqrt(bn / an)                                        # the minimiser d_j^2 = |e_j|/|Delta_j|
    at_min = math.sqrt((dj ** 2 * an ** 2).sum() * (bn ** 2 / dj ** 2).sum())
    rnd = min(math.sqrt(((dd := np.exp(rng.normal(0, 1, J))) ** 2 * an ** 2).sum() * (bn ** 2 / dd ** 2).sum()) for _ in range(50))
    wB = max(wB, abs(at_min - lhs) / lhs); assert rnd >= lhs * (1 - 1e-12)
print("B4a: 200 random block structures: |value at d_j^2 = |e_j|/|Delta_j|  -  sum_j |Delta_j||e_j|| / sum <= %.1e; no random block-constant D below the sum" % wB)
# (b) concavity of F and F^2 in Sigma_Q, subadditivity of F
wc = 9e9; wc2 = 9e9; wsub = -9e9
for t in range(400):
    n = int(rng.integers(2, 7)); S1, S2, SR = psd(n), psd(n), psd(n); lam = rng.uniform(0, 1)
    f1, f2, fm, fs = fid(S1, SR), fid(S2, SR), fid(lam * S1 + (1 - lam) * S2, SR), fid(S1 + S2, SR)
    wc = min(wc, fm - (lam * f1 + (1 - lam) * f2)); wc2 = min(wc2, fm ** 2 - (lam * f1 ** 2 + (1 - lam) * f2 ** 2)); wsub = max(wsub, fs / (f1 + f2))
print("B4b: 400 random triples: min [F(mix) - mix of F] = %.2e (>= 0), min [F(mix)^2 - mix of F^2] = %.2e (>= 0), max F(S1+S2)/(F(S1)+F(S2)) = %.4f (<= 1)" % (wc, wc2, wsub))
# (c) minimax regret over a family: min_M max_a tr(M S_a) tr(M^-1 SR)/F_a^2 = max_w F(sum_a w_a S_a/F_a^2, SR)^2
wgap = 0.0; wreg = []; wrand = 9e9
for t in range(40):
    n = int(rng.integers(2, 6)); A = 3; SR = psd(n); Ss = [psd(n) for _ in range(A)]; Fa = [fid(S, SR) for S in Ss]; Xs = [S / f ** 2 for S, f in zip(Ss, Fa)]
    best = (-1, None)
    grid = [(i / 60, j / 60, 1 - (i + j) / 60) for i in range(61) for j in range(61 - i)]
    for w in grid:
        v = fid(sum(wi * X for wi, X in zip(w, Xs)), SR) ** 2
        if v > best[0]: best = (v, w)
    w = np.array(best[1])
    for it in range(300):                                        # refine on the simplex: multiplicative ascent with the supergradient tr(M X_a)
        Sw = sum(wi * X for wi, X in zip(w, Xs)) + 1e-12 * np.eye(n); M = gmean_metric(Sw, SR); M = M * np.trace(np.linalg.inv(M) @ SR)
        gr = np.array([np.trace(M @ X) for X in Xs]); w = w * np.exp(0.05 * gr / gr.max()); w /= w.sum()
    Sw = sum(wi * X for wi, X in zip(w, Xs)) + 1e-12 * np.eye(n); val = fid(Sw, SR) ** 2
    M = gmean_metric(Sw, SR); primal = max(np.trace(M @ X) for X in Xs) * np.trace(np.linalg.inv(M) @ SR)
    wgap = max(wgap, (primal - val) / val); wreg.append(math.sqrt(val))
    for _ in range(200):
        Mp = psd(n); wrand = min(wrand, max(np.trace(Mp @ X) for X in Xs) * np.trace(np.linalg.inv(Mp) @ SR) / val)
print("B4c: 40 families of 3 query covariances: max relative gap between max_a regret^2 at M* and max_w F(S_w, SR)^2 = %.1e; "
      "regret (width ratio) between %.4f and %.4f (bounds 1 and sqrt 3 = 1.732); min over random M of its worst regret^2 / the value = %.4f (>= 1)"
      % (wgap, min(wreg), max(wreg), wrand))
# (d) price of one metric for a mixture (rms criterion): F(mean)^2 / sum w_a F_a^2 in [1, A]
pr = []
for t in range(400):
    n = int(rng.integers(2, 7)); A = int(rng.integers(2, 5)); SR = psd(n); Ss = [psd(n) for _ in range(A)]; w = rng.dirichlet(np.ones(A))
    pr.append(fid(sum(wi * S for wi, S in zip(w, Ss)), SR) ** 2 / sum(wi * fid(S, SR) ** 2 for wi, S in zip(w, Ss)))
e1, e2 = np.diag([1.0, 0.0]), np.diag([0.0, 1.0])
print("B4d: 400 random mixtures: price^2 = F(mixture)^2 / sum_a w_a F_a^2 between %.4f and %.4f (bounds 1 and the number of families); "
      "two orthogonal rank-one families, equal weights: %.4f (the bound 2 is attained)" % (min(pr), max(pr), fid(0.5 * e1 + 0.5 * e2, np.eye(2)) ** 2 / (0.5 * fid(e1, np.eye(2)) ** 2 + 0.5 * fid(e2, np.eye(2)) ** 2)))

# ---------------------------------------------------------------- B5
print("B5: rule 1's rho^-2 = (sum a)^2 / sum a^2, a_c = sqrt(pi_c v_c), against n_eff(SQ, SE) = F^2 / tr(SQ SE):")
w5 = 9e9; ex = None
for t in range(300):
    n = int(rng.integers(2, 9)); SQ, SE = psd(n), psd(n)
    a = np.sqrt(np.diag(SQ) * np.diag(SE)); F = fid(SQ, SE)
    w5 = min(w5, a.sum() / F)
    if t == 0: ex = ((a.sum()) ** 2 / (a * a).sum(), F * F / np.trace(SQ @ SE), a.sum() / F, (a * a).sum() / np.trace(SQ @ SE))
print("    non-commuting pairs in a fixed orthonormal basis: min (sum a)/F = %.4f (>= 1); one example: rho^-2 = %.3f, n_eff = %.3f, (sum a)/F = %.3f, sum a^2 / tr(SQ SE) = %.3f" % ((w5,) + ex))
d1, d2 = np.exp(rng.normal(0, 1, 6)), np.exp(rng.normal(0, 1, 6)); SQ, SE = np.diag(d1), np.diag(d2); a = np.sqrt(d1 * d2)
print("    both diagonal in the basis: rho^-2 = %.6f, n_eff = %.6f" % (a.sum() ** 2 / (a * a).sum(), fid(SQ, SE) ** 2 / np.trace(SQ @ SE)))
SQ, SE = psd(6), psd(6); h = sqrtm(SQ); gam, V = np.linalg.eigh(h @ SE @ h); A = np.diag(gam ** 0.25) @ V.T @ np.linalg.inv(h)   # the dual basis: Delta'' = A Delta, e'' = A^-T e
pi = np.diag(A @ SQ @ A.T); v = np.diag(np.linalg.inv(A).T @ SE @ np.linalg.inv(A)); a = np.sqrt(pi * v)
off = np.abs(A @ SQ @ A.T - np.diag(pi)).max() + np.abs(np.linalg.inv(A).T @ SE @ np.linalg.inv(A) - np.diag(v)).max()
print("    a non-commuting pair in its dual basis: off-diagonal mass %.1e; rho^-2 = %.6f, n_eff = %.6f" % (off, a.sum() ** 2 / (a * a).sum(), fid(SQ, SE) ** 2 / np.trace(SQ @ SE)))
# M diagonal but SQ not: denominators agree, numerators do not
n = 5; SQ = psd(n); Dg = np.diag(np.exp(rng.normal(0, 0.5, n))); SE = Dg @ SQ @ Dg; a = np.sqrt(np.diag(SQ) * np.diag(SE))
print("    M diagonal in the basis, SQ not: (sum a)/F = %.6f, sum a^2 / tr(SQ SE) = %.4f (< 1), rho^-2 = %.3f against n_eff = %.3f"
      % (a.sum() / fid(SQ, SE), (a * a).sum() / np.trace(SQ @ SE), a.sum() ** 2 / (a * a).sum(), fid(SQ, SE) ** 2 / np.trace(SQ @ SE)))
