# Referee 1, checks on P1.4 to P1.7. numpy and scipy only, synthetic data.
import numpy as np
from scipy import integrate, stats

rng = np.random.default_rng(5)


def sqrtm_psd(A):
    l, v = np.linalg.eigh(A)
    return (v * np.sqrt(np.maximum(l, 0))) @ v.T


print("== P1.4 case 1: two partitions of one group, each a fixed point of nearest centre in its own dual norm ==")
# the two-segment data of P1.3, one group, one basis, Sigma_Q = I
a, N = 1.0, 40000
for h in [0.1, 0.35]:
    s = rng.uniform(-a, a, N); sg = rng.choice([-1.0, 1.0], N)
    X = np.stack([s, sg * h], 1)
    for name, lab in [("A (by segment)", (sg > 0).astype(int)), ("B (by sign of s)", (s > 0).astype(int))]:
        for it in range(20):
            mu = np.stack([X[lab == c].mean(0) for c in range(2)])
            R = X - mu[lab]
            W = R.T @ R / N + 1e-9 * np.eye(2)           # pooled within-cell covariance of the group
            Minv = np.linalg.inv(sqrtm_psd(W))            # Sigma_Q = I: M = W^1/2
            d = np.stack([np.einsum("ij,jk,ik->i", X - mu[c], Minv, X - mu[c]) for c in range(2)], 1)
            new = d.argmin(1)
            moved = int((new != lab).sum())
            if moved == 0:
                break
            lab = new
        g = np.linalg.eigvalsh(W)
        print(f"h = {h}: start {name}: rows moved at the fixed point {moved}; spectrum of pooled covariance {np.round(g, 4)}; "
              f"F = {np.sqrt(g).sum():.4f}, trace = {g.sum():.4f}")

print("== P1.5: expected largest of n standard normals ==")
for n in [1000, 2000, 4000, 16000]:
    f = lambda x: x * n * stats.norm.pdf(x) * stats.norm.cdf(x) ** (n - 1)
    print(n, round(integrate.quad(f, -2, 12, limit=400)[0], 3))

print("== P1.6: is the built partition a maximiser of the sum of the keys? fit.py's procedure on a toy ==")
# clustered unit vectors; 12 Lloyd passes on a sample at 200 rows a centre, every row to the best sample centroid,
# stored centre = mean of the cell (8-bit), key = x . direction of the stored centre
n, N, K = 48, 60000, 60
cent = rng.normal(size=(300, n)); cent /= np.linalg.norm(cent, axis=1, keepdims=True)
X = cent[rng.integers(0, 300, N)] * rng.uniform(0.5, 1.5, (N, 1)) + rng.normal(size=(N, n)) * 0.22
X /= np.linalg.norm(X, axis=1, keepdims=True)


def kmeans(S, K, iters=12):
    C = S[rng.choice(len(S), K, replace=False)].copy()
    for _ in range(iters):
        a_ = (S @ C.T).argmax(1)
        M = np.zeros_like(C); np.add.at(M, a_, S)
        e = np.bincount(a_, minlength=K) == 0
        M[e] = S[rng.choice(len(S), int(e.sum()))]
        C = M / np.linalg.norm(M, axis=1, keepdims=True)
    return C


C = kmeans(X[rng.choice(N, 200 * K, replace=False)], K)
assign = (X @ C.T).argmax(1)
Mean = np.stack([X[assign == c].mean(0) for c in range(K)])
msc = np.abs(Mean).max(1) / 127
Mq = np.round(Mean / msc[:, None]) * msc[:, None]
Mh = Mq / np.linalg.norm(Mq, axis=1, keepdims=True)
keys = (X * Mh[assign]).sum(1)
best = (X @ Mh.T)
print("rows whose key is not the largest x.m^ over the stored centres:", float((best.argmax(1) != assign).mean()))
print("sum of keys as built", keys.sum(), "; with each row moved to its best stored centre", best.max(1).sum())
a2 = assign.copy(); obj = []
for it in range(60):
    M2 = np.stack([X[a2 == c].sum(0) if (a2 == c).any() else Mh[c] for c in range(K)])
    Ch = M2 / np.linalg.norm(M2, axis=1, keepdims=True)
    obj.append((X * Ch[a2]).sum(1).sum())
    a2 = (X @ Ch.T).argmax(1)
print("sum of keys on continuing Lloyd on all rows: start %.1f after 60 passes %.1f (gain %.3f%%); mean (1 - key) falls %.3f%%" %
      (obj[0], obj[-1], 100 * (obj[-1] / obj[0] - 1), 100 * (1 - (N - obj[-1]) / (N - obj[0]))))
# spherical objective against the trace objective on the same partitions
nm = np.linalg.norm(Mean, axis=1); cnt = np.bincount(assign, minlength=K)
print("sum_c n_c |m_c| (spherical objective) %.1f; N - sum_c n_c |m_c|^2 (= sum n_c tr Sigma_c) %.1f; |m_c| ranges %.3f to %.3f" %
      ((cnt * nm).sum(), N - (cnt * nm ** 2).sum(), nm.min(), nm.max()))

print("== P1.7: mean of f(q) against tr(B Sigma_qq)/tr(T Sigma_qq) ==")
T = np.cov(X.T, bias=True); Wp = sum((assign == c).mean() * np.cov(X[assign == c].T, bias=True) for c in range(K)); B = T - Wp
Q = X[rng.choice(N, 2000, replace=False)]
fq = np.einsum("ij,jk,ik->i", Q, B, Q) / np.einsum("ij,jk,ik->i", Q, T, Q)
Sqq = Q.T @ Q / len(Q)
print("mean over queries of f(q) %.4f; ratio of the averaged variances %.4f" % (fq.mean(), np.trace(B @ Sqq) / np.trace(T @ Sqq)))
