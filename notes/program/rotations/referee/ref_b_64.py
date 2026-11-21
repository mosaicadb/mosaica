"""Direct check, no conditioning: n = 64, one and two rounds of H D, depth-1 built code, against the block's own
certified constant 6.758 (one sphere coordinate, d = 63, 2.5e-19 one-sided)."""
import numpy as np
rng = np.random.default_rng(64)
n = 64
k_ = np.arange(n); x = k_[:, None] & k_[None, :]
pc = np.zeros((n, n), dtype=np.int64)
while x.any():
    pc += x & 1; x >>= 1
H = (1.0 - 2.0 * (pc % 2)) / np.sqrt(n)


def run(y, q, rounds, tot=2_000_000):
    y = y / np.linalg.norm(y); q = q / np.linalg.norm(q)
    assert abs(y @ q) < 1e-12
    cnt = {6.758: 0, 7.0: 0, 7.8: 0}; mx = 0.0; ms = 0.0
    for _ in range(tot // 100000):
        Z = np.broadcast_to(y, (100000, n)); W = np.broadcast_to(q, (100000, n))
        for _r in range(rounds):
            D = rng.choice((-1.0, 1.0), (100000, n))
            Z = (Z * D) @ H; W = (W * D) @ H
        Z = np.where(np.abs(Z) < 1e-12, 0.0, Z)
        o = np.where(Z >= 0, 0.5, -0.5)
        kappa = (Z * Z).sum(1) / (o * Z).sum(1)
        e = kappa[:, None] * o - Z
        A = np.linalg.norm(e, axis=1)
        r = np.where(A > 1e-12, (e * W).sum(1) / np.maximum(A, 1e-300) * np.sqrt(n - 1), 0.0)
        ms += (r * r).sum(); mx = max(mx, np.abs(r).max())
        for c in cnt:
            cnt[c] += int((np.abs(r) > c).sum())
    return ms / tot, mx, {c: v / tot for c, v in cnt.items()}


chi1 = 1.0 - 2.0 * (k_ & 1)
y1 = (k_ < 32) * 1.0; q1 = (k_ < 32) * chi1
print("one round, y = 1_G, Delta = chi_1 1_G, G = {0..31}:", run(y1, q1, 1))
y2 = np.zeros(n); y2[[0, 32]] = 1; q2 = np.zeros(n); q2[[1, 33]] = 1
print("two rounds, y = e0 + e32, Delta = e1 + e33:", run(y2, q2, 2))
print("three rounds, y = 1_G, Delta = chi_1 1_G:", run(y1, q1, 3))
print("law at d = 63: Pr[|U sqrt(d)| > 6.758] = 5e-19 two-sided; sqrt(63) = 7.94")
