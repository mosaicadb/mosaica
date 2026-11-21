# Referee 1, numerical attacks on P1.1 to P1.3. numpy and scipy only, synthetic data, seconds.
import numpy as np
from scipy.optimize import brentq, minimize

rng = np.random.default_rng(11)
phi = lambda u: np.where(u <= 1, np.sqrt(np.maximum(u, 0)), 1 + 0.5 * np.log(np.maximum(u, 1)))
dphi = lambda u: np.where(u <= 1, 0.5 / np.sqrt(u), 0.5 / u)


def W_of_H(gam, H):
    """Theorem 2.24's floor: sum sqrt(min(gam, theta)) with sum 1/2 ln+(gam/theta) = H."""
    s = np.sqrt(gam)
    if H <= 0:
        return s.sum(), gam.max()
    f = lambda lt: 0.5 * np.maximum(np.log(gam) - lt, 0).sum() - H
    lt = brentq(f, np.log(gam.max()) - 2 * H - 50, np.log(gam.max()))
    th = np.exp(lt)
    return np.sqrt(np.minimum(gam, th)).sum(), th


def Hstar(gam, w):
    s = np.sqrt(gam)
    if w >= s.sum():
        return 0.0, gam.max()
    f = lambda lt: np.sqrt(np.minimum(gam, np.exp(lt))).sum() - w
    lt = brentq(f, -200, np.log(gam.max()))
    th = np.exp(lt)
    return 0.5 * np.maximum(np.log(gam / th), 0).sum(), th


D = lambda gam, lam: phi(lam ** 2 * gam).sum()

print("== P1.1 ==")
worst_a = worst_b = worst_c = worst_d = 0.0
for trial in range(300):
    n = int(rng.integers(1, 40))
    gam = np.exp(rng.normal(0, rng.uniform(0.1, 3), n))
    if trial % 5 == 0:
        gam[: n // 2] = gam[0]  # ties
    lam = np.exp(rng.normal(0, 2))
    # (a) inf over H of H + lam W(H), by a fine grid, against D_lam
    Hs = np.concatenate([[0], np.exp(np.linspace(-8, 8, 4000))])
    vals = np.array([H + lam * W_of_H(gam, H)[0] for H in Hs])
    worst_a = max(worst_a, abs(vals.min() - D(gam, lam)) / D(gam, lam))
    assert vals.min() >= D(gam, lam) * (1 - 1e-9), "grid went below D_lam"
    # (b) direct minimisation over s in (0,1]
    obj = lambda t: (np.log1p(np.exp(-t)).sum() + lam * (np.sqrt(gam) / (1 + np.exp(-t))).sum())  # s = sigmoid(t)
    r = minimize(obj, np.zeros(n), method="L-BFGS-B")
    worst_b = max(worst_b, (r.fun - D(gam, lam)) / D(gam, lam))
    assert r.fun >= D(gam, lam) * (1 - 1e-7)
    # (c) H*(w) = D_lam - lam w at lam = theta^-1/2, and (d) H*(w) = max over lam of D_lam - lam w
    w = rng.uniform(0.02, 0.98) * np.sqrt(gam).sum()
    Hs_, th = Hstar(gam, w)
    lam0 = th ** -0.5
    worst_c = max(worst_c, abs(Hs_ - (D(gam, lam0) - lam0 * w)) / max(Hs_, 1e-9))
    lg = lam0 * np.exp(np.linspace(-3, 3, 2001))
    best = max(D(gam, l) - l * w for l in lg)
    worst_d = max(worst_d, abs(best - Hs_) / max(Hs_, 1e-9))
    assert best <= Hs_ * (1 + 1e-9) + 1e-12
print("grid inf_H [H + lam W(H)] vs D_lam, worst rel gap", worst_a)
print("direct min over s vs D_lam, worst rel excess", worst_b)
print("H* vs D_lam - lam w at lam = theta^-1/2, worst rel", worst_c)
print("H* vs max_lam (D_lam - lam w), worst rel", worst_d)
print("phi at 1 from both sides", float(phi(np.array(1 - 1e-12))), float(phi(np.array(1 + 1e-12))),
      "phi' both sides", float(dphi(np.array(1 - 1e-9))), float(dphi(np.array(1 + 1e-9))))

print("== P1.2 ==")


def sqrtm_psd(A):
    l, v = np.linalg.eigh(A)
    return (v * np.sqrt(np.maximum(l, 0))) @ v.T


def rand_psd(n, rank=None, scale=1.0):
    k = rank or n
    A = rng.normal(size=(n, k)) * np.exp(rng.normal(0, 1, k))
    return scale * A @ A.T / k


def Bmat(S, SQh):
    return SQh @ S @ SQh


def Dl(S, SQh, lam):
    g = np.maximum(np.linalg.eigvalsh(Bmat(S, SQh)), 0)
    return phi(lam ** 2 * g).sum()


def F(S, SQh):
    return np.sqrt(np.maximum(np.linalg.eigvalsh(Bmat(S, SQh)), 0)).sum()


conc = mono = refine1 = refine2 = 0.0
for trial in range(400):
    n = int(rng.integers(2, 9))
    SQh = sqrtm_psd(rand_psd(n))
    lam = np.exp(rng.normal(0, 1.5))
    X = rand_psd(n, rank=int(rng.integers(1, n + 1)))
    Y = rand_psd(n, rank=int(rng.integers(1, n + 1)), scale=np.exp(rng.normal()))
    t = rng.uniform()
    conc = min(conc, Dl(t * X + (1 - t) * Y, SQh, lam) - t * Dl(X, SQh, lam) - (1 - t) * Dl(Y, SQh, lam))
    mono = min(mono, Dl(X + Y, SQh, lam) - Dl(X, SQh, lam))
    # refinement on actual point sets
    m = int(rng.integers(2, 5))
    pts = [rng.normal(size=(int(rng.integers(3, 30)), n)) @ rng.normal(size=(n, n)) * np.exp(rng.normal()) + rng.normal(size=n) * 2 for _ in range(m)]
    allp = np.vstack(pts)
    cov = lambda P: np.cov(P.T, bias=True)
    pi = np.array([len(p) for p in pts]) / len(allp)
    Wn = sum(p_ * cov(P) for p_, P in zip(pi, pts))
    Bt = sum(p_ * np.outer(P.mean(0) - allp.mean(0), P.mean(0) - allp.mean(0)) for p_, P in zip(pi, pts))
    assert np.allclose(cov(allp), Wn + Bt)
    refine1 = min(refine1, Dl(cov(allp), SQh, lam) - Dl(Wn, SQh, lam))
    refine2 = min(refine2, Dl(Wn, SQh, lam) - sum(p_ * Dl(cov(P), SQh, lam) for p_, P in zip(pi, pts)))
print("least concavity gap", conc, "least monotone gap", mono, "least refinement gaps", refine1, refine2, "(all should be >= -1e-9)")

# limits and the gradient
gerr = merr = 0.0
for trial in range(100):
    n = int(rng.integers(2, 8))
    SQ = rand_psd(n) + 0.05 * np.eye(n)
    SQh = sqrtm_psd(SQ)
    S = rand_psd(n) + 0.05 * np.eye(n)
    g, U = np.linalg.eigh(Bmat(S, SQh))
    for lam in [1e-3 / np.sqrt(g.max()), np.exp(rng.normal()), 1e3 / np.sqrt(g.min())]:
        G = SQh @ (U * (lam ** 2 * dphi(lam ** 2 * g))) @ U.T @ SQh
        E = rng.normal(size=(n, n)); E = (E + E.T) * 1e-6
        fd = (Dl(S + E, SQh, lam) - Dl(S - E, SQh, lam)) / 2
        gerr = max(gerr, abs(fd - np.sum(G * E)) / abs(np.sum(G * E)))
    lam = 0.5 / np.sqrt(g.max())
    G = SQh @ (U * (lam ** 2 * dphi(lam ** 2 * g))) @ U.T @ SQh
    M = np.linalg.inv(SQh) @ sqrtm_psd(Bmat(S, SQh)) @ np.linalg.inv(SQh)
    merr = max(merr, np.abs(G - lam / 2 * np.linalg.inv(M)).max() / np.abs(G).max())
    assert abs(Dl(S, SQh, lam) / lam - F(S, SQh)) < 1e-9 * F(S, SQh)
    lam = 2 / np.sqrt(g.min())
    assert abs(Dl(S, SQh, lam) - n * (1 + np.log(lam)) - 0.5 * np.log(g).sum()) < 1e-8
    # Euler and the tangent: F(S') <= 1/2 F(S) + 1/2 tr(M^-1 S')
    S2 = rand_psd(n)
    assert F(S2, SQh) <= 0.5 * F(S, SQh) + 0.5 * np.trace(np.linalg.inv(M) @ S2) + 1e-9
    assert abs(np.trace(np.linalg.inv(M) @ S) - F(S, SQh)) < 1e-8 * F(S, SQh)
print("gradient: worst rel finite-difference error", gerr, "; worst rel |grad - (lam/2) M^-1| for lam*sigma_1 <= 1:", merr)
# repeated eigenvalues: is the gradient formula still right?
n = 5; SQh = np.eye(n); S = np.diag([2.0, 2.0, 2.0, 0.5, 0.5]); lam = 1.3
g, U = np.linalg.eigh(S); G = (U * (lam ** 2 * dphi(lam ** 2 * g))) @ U.T
E = rng.normal(size=(n, n)); E = (E + E.T) * 1e-6
print("repeated eigenvalues: fd", (Dl(S + E, SQh, lam) - Dl(S - E, SQh, lam)) / 2, "formula", np.sum(G * E))

# k-means in the M0^-1 norm as a majoriser: one Lloyd pass in that norm lowers the bound, does it lower F(W_P)?
print("-- MM check: k-means in the M(W)^-1 norm against F(W_P), one basis --")
n, N, K = 6, 3000, 12
A = rng.normal(size=(n, n)) * np.array([3, 2, 1, .5, .3, .1])
X = rng.normal(size=(N, n)) @ A.T + 2 * rng.normal(size=(rng.integers(1, 2), n))
SQ = rand_psd(n) + 0.1 * np.eye(n); SQh = sqrtm_psd(SQ)
a = rng.integers(0, K, N)
def within(a):
    Wm = np.zeros((n, n)); mu = np.zeros((K, n))
    for c in range(K):
        P = X[a == c]
        if len(P):
            mu[c] = P.mean(0); R = P - mu[c]; Wm += R.T @ R
    return Wm / N, mu
ups = 0
for it in range(40):
    Wm, mu = within(a)
    Minv = SQh @ np.linalg.inv(sqrtm_psd(Bmat(Wm, SQh))) @ SQh
    d = np.stack([np.einsum("ij,jk,ik->i", X - mu[c], Minv, X - mu[c]) for c in range(K)], 1)
    a2 = d.argmin(1)
    f0, f1 = F(Wm, SQh), F(within(a2)[0], SQh)
    ups += f1 > f0 + 1e-12
    if (a2 == a).all():
        break
    a = a2
print("iterations", it + 1, "times F(W_P) rose", ups, "final F", f1)

print("== P1.3 ==")
viol_1 = viol_2 = 0.0; percell = 0; tot = 0; fixed_alloc_between = 0.0
for trial in range(300):
    n = int(rng.integers(2, 8)); m = int(rng.integers(2, 7))
    SQh = sqrtm_psd(rand_psd(n) + 0.05 * np.eye(n)); lam = np.exp(rng.normal(0, 1.5))
    covs = [rand_psd(n, rank=int(rng.integers(1, n + 1)), scale=np.exp(rng.normal())) for _ in range(m)]
    p = rng.dirichlet(np.ones(m))
    pooled = sum(pi_ * C for pi_, C in zip(p, covs))
    gg, U = np.linalg.eigh(Bmat(pooled, SQh))
    betas = [np.maximum(np.diag(U.T @ Bmat(C, SQh) @ U), 0) for C in covs]
    own = sum(pi_ * Dl(C, SQh, lam) for pi_, C in zip(p, covs))
    diag = sum(pi_ * phi(lam ** 2 * b).sum() for pi_, b in zip(p, betas))
    grp = phi(lam ** 2 * np.maximum(gg, 0)).sum()
    viol_1 = min(viol_1, diag - own); viol_2 = min(viol_2, grp - diag)
    # per cell: is the group-wide figure above the cell's diagonal-model cost?
    for b in betas:
        tot += 1; percell += phi(lam ** 2 * b).sum() > grp + 1e-12
    # a fourth model: the group's allocation s^g, the cell's own widths sqrt(beta)
    sg = np.minimum(1, 1 / (lam * np.sqrt(np.maximum(gg, 1e-300))))
    fixed = sum(pi_ * (np.log(1 / sg).sum() + lam * (sg * np.sqrt(b)).sum()) for pi_, b in zip(p, betas))
    fixed_alloc_between = min(fixed_alloc_between, grp - fixed, fixed - diag)
print("averaged ordering own <= diagonal <= group-wide: least gaps", viol_1, viol_2)
print("cells whose diagonal-model cost exceeds D_lam(pooled):", percell, "of", tot)
print("fixed group allocation with the cell's widths lies between diagonal and group-wide: least gap", fixed_alloc_between)

print("-- two-segment example, a = 1 --")
for h in [0.1, 0.2, 0.35, 0.45]:
    a_ = 1.0
    gA = np.array([a_ ** 2 / 3]); gB = np.array([a_ ** 2 / 12, h ** 2])
    print(f"h = {h}: traces A {gA.sum():.4f} B {gB.sum():.4f}; F_A {np.sqrt(gA).sum():.4f} F_B {np.sqrt(gB).sum():.4f}")
    for w in [0.05, 0.15, 0.3, 0.5, 2 * h * 0.999, 2 * h * 1.001, 0.6, 0.7]:
        HA, HB = Hstar(gA, w)[0], Hstar(gB, w)[0]
        pref = "A" if HA < HB - 1e-12 else ("B" if HB < HA - 1e-12 else "tie")
        print(f"   w = {w:.4f}: H*_A {HA:.4f} H*_B {HB:.4f} nats; coding prefers {pref}; claim says {'A' if w < 2 * h else 'B'}; diff {abs(HA - HB) / np.log(2):.3f} bits")
# from sampled points, covariances as the index would see them
Npt = 200000; s = rng.uniform(-1, 1, Npt); sgn = rng.choice([-1, 1], Npt); h = 0.1
P = np.stack([s, sgn * h], 1)
cA = np.cov(P[sgn > 0].T, bias=True); cB = np.cov(P[s > 0].T, bias=True)
print("sampled covariances, A:", np.round(np.linalg.eigvalsh(cA), 5), "B:", np.round(np.linalg.eigvalsh(cB), 5), "(1/3 = 0.33333, 1/12 = 0.08333, h^2 = 0.01)")
