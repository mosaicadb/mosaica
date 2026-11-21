# P3.5: the row-level scheme (rows choose a cell and with it a basis) against k-means and the cell-level scheme,
# on a synthetic instance where the shape of a row's residual does not follow its position. numpy, one thread, seconds.
# Objective at zero bytes read, Sigma_Q = I: Psi = sum_g N_g F(pooled_g)^2 / N, F(S) = tr S^1/2.
# usage: python3 check_p3_rows.py > check_p3_rows.log
import numpy as np
rng = np.random.default_rng(11)
n, N, K, G = 12, 12000, 24, 4

def Fsq_Minv(S):                                # F(S) and F(S) * M^-1 = tr(S^1/2) * S^-1/2  (Sigma_Q = I)
    l, u = np.linalg.eigh(S); l = np.maximum(l, 1e-9)
    return np.sqrt(l).sum(), np.sqrt(l).sum() * (u / np.sqrt(l)) @ u.T

def stats(X, a, grp):
    cnt = np.bincount(a, minlength=K); Mean = np.zeros((K, n)); np.add.at(Mean, a, X); Mean /= np.maximum(cnt, 1)[:, None]
    R = X - Mean[a]; g = grp[a]; psi = 0.0; A = np.zeros((G, n, n))
    for j in range(G):
        m = g == j
        if m.sum() < 2: continue
        Fg, A[j] = Fsq_Minv(R[m].T @ R[m] / m.sum()); psi += m.sum() * Fg ** 2
    return Mean, A, psi / len(X), (R * R).sum(1).mean()

def kmeans(X, K, iters=25):
    C = X[rng.choice(len(X), K, replace=False)].copy()
    for _ in range(iters):
        a = ((X * X).sum(1)[:, None] - 2 * X @ C.T + (C * C).sum(1)).argmin(1)
        for c in range(K):
            if (a == c).any(): C[c] = X[a == c].mean(0)
    return a

def cell_level(X, a, grp, iters=30):            # cells move between groups by omega_c(M_g)^2 (P3.1)
    hist = []
    for _ in range(iters):
        Mean, A, psi, _ = stats(X, a, grp); hist.append(psi); R = X - Mean[a]
        cost = np.stack([np.bincount(a, weights=np.einsum("xi,ij,xj->x", R, A[j], R), minlength=K) for j in range(G)], 1)
        new = np.where(cost.min(1) < cost[np.arange(K), grp] - 1e-9, cost.argmin(1), grp)
        if (new == grp).all(): break
        grp = new
    return grp, hist

def row_level(X, a, grp, iters=60):             # rows move between cells by omega(x | c)^2 (P3.5); means and metrics refit
    hist = []
    for _ in range(iters):
        Mean, A, psi, _ = stats(X, a, grp); hist.append(psi)
        cost = np.empty((len(X), K))
        for c in range(K):
            R = X - Mean[c]; cost[:, c] = np.einsum("xi,ij,xj->x", R, A[grp[c]], R)
        cur = cost[np.arange(len(X)), a]; new = np.where(cost.min(1) < cur - 1e-9, cost.argmin(1), a)
        if (new == a).all(): break
        a = new
    return a, hist

# data: 8 blobs; in every blob half the rows spread along subspace U1 and half along U2 (two "kinds"), same centre
Q = np.linalg.qr(rng.normal(size=(n, n)))[0]; U = [Q[:, :3], Q[:, 3:6]]
cent = rng.normal(size=(8, n)) * 1.2; b = rng.integers(0, 8, N); kind = rng.integers(0, 2, N)
X = cent[b] + 0.08 * rng.normal(size=(N, n))
for k in range(2):
    m = kind == k; X[m] += rng.normal(size=(m.sum(), 3)) @ U[k].T * 0.6
a0 = kmeans(X, K); cnt = np.bincount(a0, minlength=K)
Mean = np.stack([X[a0 == c].mean(0) if cnt[c] else X[0] for c in range(K)])
grp0 = kmeans(Mean, G) % G                      # groups by centre position, as the build does by direction
_, _, psi0, km0 = stats(X, a0, grp0)
one = stats(X, a0, np.zeros(K, int))[2]
grp1, h1 = cell_level(X, a0, grp0.copy()); _, _, psi1, km1 = stats(X, a0, grp1)
a2, h2 = row_level(X, a0.copy(), grp1.copy()); _, _, psi2, km2 = stats(X, a2, grp1)
purity = lambda a: np.mean([max(np.mean(kind[a == c] == 0), np.mean(kind[a == c] == 1)) for c in range(K) if (a == c).sum() > 10])
mono = lambda h: all(h[i + 1] <= h[i] * (1 + 1e-9) for i in range(len(h) - 1))
print(f"one basis for all cells:                Psi {one:.4f}")
print(f"k-means cells, groups by centre:        Psi {psi0:.4f}  k-means objective {km0:.4f}  kind purity of cells {purity(a0):.2f}")
print(f"same cells, cells to bases by cost:     Psi {psi1:.4f}  k-means objective {km1:.4f}  ({len(h1) - 1} steps, monotone {mono(h1)})")
print(f"rows to cells by certified width:       Psi {psi2:.4f}  k-means objective {km2:.4f}  kind purity of cells {purity(a2):.2f}  "
      f"({len(h2) - 1} steps, monotone {mono(h2)}); rows moved {np.mean(a2 != a0):.2f}; cells of under 50 rows {int((np.bincount(a2, minlength=K) < 50).sum())}")
print(f"ratios of rms zero-read width to the built grouping: cell level {np.sqrt(psi1 / psi0):.3f}, row level {np.sqrt(psi2 / psi0):.3f}")
