# Checks of CLAIMS.md P1.1 to P1.3 on small synthetic instances. numpy only, one thread, seconds.
# usage: python3 check_p1.py > check_p1.log
import numpy as np
rng = np.random.default_rng(20261121)

def phi(u):
    u = np.asarray(u, float)
    return np.where(u <= 1, np.sqrt(np.maximum(u, 0)), 1 + 0.5 * np.log(np.maximum(u, 1)))

def D(lam, gam):                       # D_lambda from a spectrum
    return phi(lam ** 2 * np.asarray(gam)).sum()

def Hstar(w, gam):                     # nats for mean half-width w: water-filling, by bisection on theta
    gam = np.asarray(gam, float); F = np.sqrt(gam).sum()
    if w >= F: return 0.0, np.inf
    lo, hi = 0.0, gam.max()
    for _ in range(200):
        th = 0.5 * (lo + hi)
        if np.sqrt(np.minimum(gam, th)).sum() > w: hi = th
        else: lo = th
    th = 0.5 * (lo + hi)
    return 0.5 * np.log(np.maximum(gam / th, 1)).sum(), th

print("== two segments, a = 1, h = 0.1 ==")
a, h = 1.0, 0.1
# covariances from a sample, to check the table
s = rng.uniform(-a, a, 400_000); top = rng.random(400_000) < 0.5
X = np.stack([s, np.where(top, h, -h)], 1)
covA = np.cov(X[top].T, bias=True); covB = np.cov(X[s > 0].T, bias=True)
print("sampled cov A diag", np.diag(covA).round(4), "exact", [round(a * a / 3, 4), 0.0])
print("sampled cov B diag", np.diag(covB).round(4), "exact", [round(a * a / 12, 4), h * h])
gA, gB = np.array([a * a / 3, 0.0]), np.array([a * a / 12, h * h])
print("trace A, B:", gA.sum().round(4), gB.sum().round(4), "ratio", (gA.sum() / gB.sum()).round(2))
for w in [0.02, 0.05, 0.1, 0.19, 0.2, 0.21, 0.3, 0.38]:
    HA, _ = Hstar(w, gA); HB, _ = Hstar(w, gB)
    pred = np.log(2 * h / w) if w <= 2 * h else -np.log(2 * (w - h) / w)
    print(f"w {w:5.2f}  H*_A {HA:6.3f}  H*_B {HB:6.3f} nats   B - A {HB - HA:+.3f} (closed form {pred:+.3f})   "
          f"bits {(HB - HA) / np.log(2):+.2f}   coding prefers {'A' if HA < HB else 'B'}")

print("\n== duality: H*(w) = D_lambda - lambda w at lambda = theta^-1/2, and <= for other lambda ==")
worst = 0.0
for _ in range(200):
    n = rng.integers(2, 40); gam = np.sort(rng.lognormal(0, 1.5, n))[::-1]
    w = rng.uniform(0.05, 0.95) * np.sqrt(gam).sum()
    H, th = Hstar(w, gam)
    worst = max(worst, abs(H - (D(th ** -0.5, gam) - w * th ** -0.5)))
    for lam in th ** -0.5 * np.array([0.3, 0.7, 1.5, 4.0]):
        assert D(lam, gam) - lam * w <= H + 1e-7
print("largest |H* - (D - lambda w)| over 200 spectra:", f"{worst:.2e}")

print("\n== P1.2: concavity, monotonicity, refinement; P1.3: the three models are ordered ==")
def rand_cov(n, k):
    A = rng.normal(size=(n, k)) * rng.lognormal(0, 1, k); return A @ A.T / k + 1e-3 * np.eye(n)
def sqrtm(S):
    l, u = np.linalg.eigh(S); return (u * np.sqrt(np.maximum(l, 0))) @ u.T
viol = 0; gaps = []
for trial in range(300):
    n = int(rng.integers(3, 12)); SQ = rand_cov(n, n); A = sqrtm(SQ)
    J = int(rng.integers(2, 6)); pi = rng.dirichlet(np.ones(J))
    Sj = [rand_cov(n, int(rng.integers(1, n + 1))) for _ in range(J)]
    mj = rng.normal(size=(J, n)) * 0.5; m = pi @ mj
    pooled = sum(p * S for p, S in zip(pi, Sj)); full = pooled + sum(p * np.outer(v - m, v - m) for p, v in zip(pi, mj))
    lam = float(rng.lognormal(0, 1.5))
    eig = lambda S: np.maximum(np.linalg.eigvalsh(A @ S @ A), 0)
    d_full, d_pool = D(lam, eig(full)), D(lam, eig(pooled))
    d_own = sum(p * D(lam, eig(S)) for p, S in zip(pi, Sj))
    U = np.linalg.eigh(A @ pooled @ A)[1]
    d_diag = sum(p * D(lam, np.maximum(np.diag(U.T @ A @ S @ A @ U), 0)) for p, S in zip(pi, Sj))
    ok = d_full >= d_pool - 1e-9 and d_pool >= d_diag - 1e-9 and d_diag >= d_own - 1e-9
    viol += not ok; gaps.append([(d_full - d_pool) / d_full, (d_pool - d_diag) / d_full, (d_diag - d_own) / d_full])
gaps = np.array(gaps)
print("violations of D(full) >= D(pooled) >= diagonal model >= own basis in 300 trials:", viol)
print("mean relative gaps (centres, allocation, basis):", gaps.mean(0).round(3))

print("\n== P1.2 item 5: gradient of F is M^-1 / 2 (finite differences) ==")
n = 6; SQ = rand_cov(n, n); SR = rand_cov(n, n); A = sqrtm(SQ); Ai = np.linalg.inv(A)
Ffun = lambda S: np.sqrt(np.maximum(np.linalg.eigvalsh(A @ S @ A), 0)).sum()
Bh = sqrtm(A @ SR @ A); Minv = A @ np.linalg.inv(Bh) @ A
E = rng.normal(size=(n, n)); E = (E + E.T) / 2; eps = 1e-6
print("finite difference", ((Ffun(SR + eps * E) - Ffun(SR - eps * E)) / (2 * eps)).round(8), " tr(M^-1 E)/2", (np.trace(Minv @ E) / 2).round(8))
