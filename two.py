import numpy as np
rng = np.random.default_rng(5)
X = np.concatenate([np.load(f"data/e{i:02d}.npy") for i in (0, 5, 10, 15)]).astype(np.float32)
X /= np.linalg.norm(X, axis=1, keepdims=True)
N, d = X.shape; NQ = 50
Q = X[rng.choice(N, NQ, replace=False)]
m = X.mean(0); R = X - m; Dl = Q - m; dn = np.linalg.norm(Dl, axis=1)
true = X @ Q.T; base = (Q @ m)[None, :] + (R @ m)[:, None]          # exact scalars
rot = [np.linalg.qr(rng.standard_normal((d, d)).astype(np.float32))[0] for _ in range(2)]
def q1(Z):                      # 1 bit per dim on unit-variance coords
    return np.where(Z >= 0, 0.7979, -0.7979).astype(np.float32)
def q2(Z):
    return ((np.clip(np.floor(Z / 0.9957), -2, 1) + 0.5) * 0.9957).astype(np.float32)
def encode(V, Rm, qf):          # returns reconstruction in original coords with optimal scale
    Z = V @ Rm; s = np.linalg.norm(Z, axis=1, keepdims=True) / np.sqrt(d) + 1e-12
    C = qf(Z / s); g = (Z * C).sum(1, keepdims=True) / (C * C).sum(1, keepdims=True)
    return (g * C) @ Rm.T
def stats(name, Rhat):
    e = np.linalg.norm(R - Rhat, axis=1); est = base + Rhat @ Dl.T
    h = e[:, None] * dn[None, :] * 1.0001 + 2e-5
    return name, est + h, est - h, float(np.median(e))
A1 = encode(R, rot[0], q1); A2 = encode(R, rot[1], q1)
one = stats("one code, 96 B", A1); two = stats("second independent code, 96 B", A2)
# joint: least-squares combination of the two reconstructions, exact error norm
a11 = (A1 * A1).sum(1); a22 = (A2 * A2).sum(1); a12 = (A1 * A2).sum(1); b1 = (R * A1).sum(1); b2 = (R * A2).sum(1)
det = a11 * a22 - a12 * a12; w1 = (b1 * a22 - b2 * a12) / det; w2 = (b2 * a11 - b1 * a12) / det
joint = stats("two independent codes combined, 192 B", w1[:, None] * A1 + w2[:, None] * A2)
resid = stats("second code on what the first missed, 192 B", A1 + encode(R - A1, rot[1], q1))
dbl = stats("one code at 2 bits per dimension, 192 B", encode(R, rot[0], q2))
print(f"{'':48s} error   undecided as multiple of answer at t=0.95, 0.9, 0.8")
def row(name, ub, lb, e):
    out = []
    for t in (0.95, 0.9, 0.8):
        A = true >= t; sv = ub >= t; win = lb >= t
        assert not (A & ~sv).any() and not (win & ~(true >= t - 1e-4)).any()
        out.append((sv & ~win).sum() / A.sum())
    print(f"{name:48s} {e:.3f}  ", "  ".join(f"{v:7.2f}" for v in out))
row(*one)
row("two independent codes, intervals intersected", np.minimum(one[1], two[1]), np.maximum(one[2], two[2]), min(one[3], two[3]))
row(*joint); row(*resid); row(*dbl)
