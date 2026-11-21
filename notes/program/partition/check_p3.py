# P3: Bregman grouping of cluster covariances (generator -F^2) on synthetic instances. numpy only, one thread.
# Checks: the objective never rises, the scheme stops, and how its gain over grouping by centre direction depends on
# whether a cluster's shape follows its position. usage: python3 check_p3.py > check_p3.log
import numpy as np
rng = np.random.default_rng(7)
n, K, G = 16, 240, 8

def sqrtm(S):
    l, u = np.linalg.eigh(S); return (u * np.sqrt(np.maximum(l, 0))) @ u.T
def F(S, A):                                   # tr (A S A)^1/2
    return np.sqrt(np.maximum(np.linalg.eigvalsh(A @ S @ A), 0)).sum()
def Minv(S, A):                                # M^-1 = A (A S A)^-1/2 A, and tr(M Sigma_Q) = F
    l, u = np.linalg.eigh(A @ S @ A); l = np.maximum(l, 1e-12)
    return A @ ((u / np.sqrt(l)) @ u.T) @ A, np.sqrt(l).sum()

def objective(a, Sig, w, A):                   # sum_g N_g F(pooled_g)^2
    tot = 0.0
    for g in np.unique(a):
        m = a == g; P = np.einsum("c,cij->ij", w[m], Sig[m]) / w[m].sum(); tot += w[m].sum() * F(P, A) ** 2
    return tot

def bregman(a, Sig, w, A, iters=50):
    hist = [objective(a, Sig, w, A)]
    for it in range(iters):
        cost = np.full((len(Sig), G), np.inf)
        for g in np.unique(a):
            m = a == g; P = np.einsum("c,cij->ij", w[m], Sig[m]) / w[m].sum()
            Mi, Fg = Minv(P, A); cost[:, g] = Fg * np.einsum("ij,cji->c", Mi, Sig)      # omega_c(M_g)^2
        new = np.where(cost.min(1) < cost[np.arange(len(a)), a] - 1e-12, cost.argmin(1), a)
        if (new == a).all(): break
        a = new; hist.append(objective(a, Sig, w, A))
    return a, hist

def kmeans_dir(C, G, iters=20):
    c = C[rng.choice(len(C), G, replace=False)].copy()
    for _ in range(iters):
        a = (C @ c.T).argmax(1)
        for g in range(G):
            if (a == g).any(): v = C[a == g].sum(0); c[g] = v / np.linalg.norm(v)
    return (C @ c.T).argmax(1)

def shape(seed_vec, k=4):                      # a covariance with k strong directions set by seed_vec
    r = np.random.default_rng(abs(hash(tuple(np.round(seed_vec, 6)))) % 2**32)
    Q = np.linalg.qr(r.normal(size=(n, n)))[0]; lam = np.r_[np.ones(k), 0.05 * np.ones(n - k)]
    return (Q * lam) @ Q.T

A = np.eye(n)                                  # Sigma_Q = I (the comparison is the same in any fixed query metric)
for name in ["shape follows position (smooth)", "shape independent of position (6 kinds)", "all cells alike"]:
    C = rng.normal(size=(K, n)); C[:, 3:] *= 0.15; C /= np.linalg.norm(C, axis=1, keepdims=True)   # centres near a 3-flat
    w = rng.integers(500, 4000, K).astype(float)
    if name.startswith("shape follows"):
        base = [shape(np.array([j])) for j in range(3)]
        Sig = np.array([sum(abs(C[c, j]) * base[j] for j in range(3)) + 0.02 * np.eye(n) for c in range(K)])
    elif name.startswith("shape independent"):
        kinds = [shape(np.array([10 + j])) for j in range(6)]; t = rng.integers(0, 6, K)
        Sig = np.array([kinds[t[c]] + 0.02 * np.eye(n) for c in range(K)])
    else:
        S0 = shape(np.array([99])); Sig = np.array([S0 + 0.02 * np.eye(n)] * K)
    Sig = Sig * rng.lognormal(0, 0.3, K)[:, None, None]                       # sizes differ; F^2 is homogeneous
    lower = (w * np.array([F(S, A) ** 2 for S in Sig])).sum()                 # one basis per cluster
    one = objective(np.zeros(K, int), Sig, w, A)
    a_rand = rng.integers(0, G, K); a_dir = kmeans_dir(C, G)
    a_b, hist = bregman(a_dir.copy(), Sig, w, A); a_br, hist_r = bregman(a_rand.copy(), Sig, w, A)
    mono = all(hist[i + 1] <= hist[i] * (1 + 1e-12) for i in range(len(hist) - 1))
    price = lambda v: v / lower
    print(f"{name}: price of one basis {price(one):.4f}; random groups {price(objective(a_rand, Sig, w, A)):.4f}; "
          f"by centre direction {price(hist[0]):.4f}; Bregman from that {price(hist[-1]):.4f} ({len(hist) - 1} steps, "
          f"monotone {mono}); Bregman from random {price(hist_r[-1]):.4f} ({len(hist_r) - 1} steps)")
