# Checks of the repairs made after referee-1. numpy, synthetic or arithmetic on figures printed in cited reports.
# usage: python3 check_after_referee.py > check_after_referee.log
import numpy as np
from math import log, sqrt
rng = np.random.default_rng(5)
def sqrtm(S):
    l, u = np.linalg.eigh(S); return (u * np.sqrt(np.maximum(l, 0))) @ u.T
def F(S, A=None):
    A = np.eye(len(S)) if A is None else A
    return np.sqrt(np.maximum(np.linalg.eigvalsh(A @ S @ A), 0)).sum()

print("== 1. P1.4: the two segments as ONE group (a = 1), Sigma_Q = I ==")
for h in (0.1, 0.35):
    WA, WB = np.diag([1 / 3, 0.0]), np.diag([1 / 12, h * h])
    print(f"h {h}: trace A {np.trace(WA):.3f} B {np.trace(WB):.3f};  zero-read F(W_g): A {F(WA):.3f}  B {F(WB):.3f};  B/A {F(WB) / F(WA):.3f}")
# fixed points: nearest centre in the group's own dual norm, M^-1 = F * W^-1/2 (ridge 1e-9 for the flat direction)
s = rng.uniform(-1, 1, 20000); top = rng.random(20000) < 0.5
for h in (0.1, 0.35):
    X = np.stack([s, np.where(top, h, -h)], 1)
    for name, a in (("A", top.astype(int)), ("B", (s > 0).astype(int))):
        C = np.stack([X[a == j].mean(0) for j in (0, 1)]); R = X - C[a]; W = R.T @ R / len(X) + 1e-9 * np.eye(2)
        l, u = np.linalg.eigh(W); Mi = (u / np.sqrt(l)) @ u.T
        d = np.stack([np.einsum("xi,ij,xj->x", X - C[j], Mi, X - C[j]) for j in (0, 1)], 1)
        print(f"   h {h} partition {name}: rows whose nearest centre in the group's own dual norm is another cell: {int((d.argmin(1) != a).sum())}")

print("\n== 2. P1.2: F(W) <= F(W0)/2 + tr(M(W0)^-1 W)/2, equality at W0 (300 random pairs, random Sigma_Q) ==")
worst = np.inf
for _ in range(300):
    n = int(rng.integers(2, 9)); g = lambda: (lambda Z: Z @ Z.T / n + 1e-3 * np.eye(n))(rng.normal(size=(n, n)) * rng.lognormal(0, 1, n))
    SQ, W0, W = g(), g(), g(); A = sqrtm(SQ); B = A @ W0 @ A; l, u = np.linalg.eigh(B); Mi = A @ ((u / np.sqrt(l)) @ u.T) @ A
    worst = min(worst, 0.5 * F(W0, A) + 0.5 * np.trace(Mi @ W) - F(W, A)); assert abs(np.trace(Mi @ W0) - F(W0, A)) < 1e-8 * F(W0, A)
print("least value of majoriser - F over 300 pairs:", f"{worst:.3e}", "(never negative)" if worst >= -1e-9 else "VIOLATION")

print("\n== 3. P1.3: four ordered quantities, as averages over a group (300 random groups) ==")
phi = lambda u: np.where(u <= 1, np.sqrt(np.maximum(u, 0)), 1 + 0.5 * np.log(np.maximum(u, 1)))
bad = 0; cellwise = 0; cells = 0
for _ in range(300):
    n = int(rng.integers(3, 10)); J = int(rng.integers(2, 6)); p = rng.dirichlet(np.ones(J)); lam = float(rng.lognormal(0, 1.5))
    S = [(lambda Z: Z @ Z.T / n + 1e-3 * np.eye(n))(rng.normal(size=(n, int(rng.integers(1, n + 1)))) * 1.0) for _ in range(J)]
    P = sum(q * s_ for q, s_ in zip(p, S)); gam, U = np.linalg.eigh(P); sg = np.minimum(1, 1 / (lam * np.sqrt(gam)))
    own = [phi(lam ** 2 * np.linalg.eigvalsh(s_)).sum() for s_ in S]
    beta = [np.maximum(np.diag(U.T @ s_ @ U), 0) for s_ in S]
    diag = [phi(lam ** 2 * b).sum() for b in beta]
    fixed = [(np.log(1 / sg) + lam * sg * np.sqrt(b)).sum() for b in beta]
    top = phi(lam ** 2 * gam).sum(); av = lambda v: float(np.dot(p, v))
    bad += not (av(own) <= av(diag) + 1e-9 and av(diag) <= av(fixed) + 1e-9 and av(fixed) <= top + 1e-9)
    cellwise += sum(d > top + 1e-9 for d in diag); cells += J
print("violations of own <= diagonal <= fixed allocation <= D(pooled), averaged over the group:", bad,
      f"; cells whose diagonal-model cost exceeds D(pooled): {cellwise} of {cells}")

print("\n== 4. P2.3: S* with per-cell work falling as S^-p (figures of rust/REPORT_CLUSTERSIZE.md's breakdown) ==")
for name, cell, row in (("text 0.01%", (10.3, 4.1, 1.9), (20.1, 22.5, 24.9)), ("text 1%", (16.4, 6.9, 3.4), (55.8, 57.2, 58.5)), ("item 1%", (5.7, 2.8, 1.5), (9.6, 12.6, 15.8))):
    p = log(cell[0] / cell[2]) / log(16); eps = (row[2] / row[0] - 1) / 4; a, b = cell[0], row[0]
    print(f"{name}: p {p:.2f}  eps {eps:.4f}  S* with 1/S {1000 * a * log(2) / (b * eps):,.0f}  S* with S^-p {1000 * (p * a * log(2) / (b * eps)) ** (1 / p):,.0f}"
          f"   measured per-cell + per-row ms at 1,000 / 4,000 / 16,000: {[round(c + r, 1) for c, r in zip(cell, row)]}")

print("\n== 5. P2.2 against the measured bytes (same report) ==")
print("text 1%: bytes per item entering the levels:", round(67.9 * 999 / 955, 1), "->", round(71.6 * 999 / 979, 1), "; per doubling", round((71.6 * 999 / 979 - 67.9 * 999 / 955) / 2, 2), "B")
print("text 50%: rise", round(356.8 - 348.3, 1), "B, of which full vectors", round((0.1192 - 0.1157) * 1536, 1), "B")

print("\n== 6. P3.4: bytes per item against the number of groups, same cells (same report) ==")
rows = {"64 -> 31 groups, 4,000 rows a cell": (1.0, [("text 0.01%", 17.3, 17.6, .0017, .0010), ("text 1%", 71.6, 76.5, .0075, .0082), ("text 50%", 356.8, 375.4, .1192, .1281), ("item 1%", 16.3, 15.4, .0043, .0031), ("item 50%", 301.9, 311.5, .0947, .0988)]),
        "62 -> 7 groups, 16,000 rows a cell": (np.log2(62 / 7), [("text 0.01%", 17.9, 24.3, .0011, .0002), ("text 1%", 72.1, 92.9, .0071, .0098), ("text 50%", 357.4, 429.4, .1192, .1543), ("item 1%", 17.0, 20.3, .0034, .0028), ("item 50%", 296.9, 362.3, .0920, .1219)])}
for k, (dbl, lst) in rows.items():
    print(k, f"({dbl:.2f} doublings)")
    for nm, b0, b1, f0, f1 in lst:
        c0, c1 = b0 - 1536 * f0, b1 - 1536 * f1
        print(f"   {nm}: all bytes {100 * (b1 / b0 - 1):+.1f}% ({100 * ((b1 / b0) ** (1 / dbl) - 1):+.1f}% per halving of G); "
              f"without full vectors {c0:.1f} -> {c1:.1f} B, {100 * (c1 / c0 - 1):+.1f}% ({100 * ((c1 / c0) ** (1 / dbl) - 1):+.1f}% per halving)")
