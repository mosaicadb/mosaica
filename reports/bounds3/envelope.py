# How well a one-dimensional statistic of the shares predicts the exact constant: Q(w) = sum_j w_j^2 / (d_j + 2)
# (the fourth cumulant of the error is -6 Q). Envelope g(q) = sup { c(w) : Q(w) >= q } over the whole simplex
# (from the grid table; not a certificate), and what it would give on the dump's shares.
import numpy as np, math, sys
tabz = np.load("ctab_40.npz"); keys = list(tabz["keys"]); TAB = tabz["tab"]; K = int(tabz["K"])
D = np.array([95, 127, 255, 287], float)
g = np.arange(K + 1) / K
W3 = np.stack(np.meshgrid(g, g, g, indexing="ij"), -1)
ok = W3.sum(-1) <= 1 + 1e-9
W = np.concatenate([W3, np.maximum(0, 1 - W3.sum(-1, keepdims=True))], -1)
Q = (W ** 2 / (D + 2)).sum(-1)
c = TAB[..., keys.index("one")]
q_, c_ = Q[ok], c[ok]
o = np.argsort(-q_); qs, cs = q_[o], c_[o]
env = np.maximum.accumulate(cs)          # sup of c over Q >= q, q descending
print("Q range", qs.min(), qs.max(), " c range", cs.min(), cs.max())
for De in (100, 150, 200, 250, 300, 350, 400, 450, 500, 550, 600, 650, 700, 768):
    q = 1 / (De + 2)
    i = np.searchsorted(-qs, -q, side="right") - 1
    m = np.abs(q_ - q) < 0.03 * q
    print(f"D_eff {De}: Q {q:.5f}; envelope sup c over Q >= q: {env[max(i,0)]:.4f}; c among grid points with Q within 3%: min {c_[m].min() if m.any() else float('nan'):.4f} max {c_[m].max() if m.any() else float('nan'):.4f}")
# concavity of c^2 along grid directions (second differences), restricted to the simplex
c2 = np.where(ok, c ** 2, np.nan)
worst = 0
for dv in ((1, 0, 0), (0, 1, 0), (0, 0, 1), (1, -1, 0), (1, 0, -1), (0, 1, -1)):
    a_ = np.roll(c2, dv, (0, 1, 2)); b_ = np.roll(c2, tuple(-x for x in dv), (0, 1, 2))
    sd2 = a_ + b_ - 2 * c2
    # drop wrapped entries
    m = np.ones_like(c2, bool)
    for ax, s in enumerate(dv):
        if s:
            idx = [slice(None)] * 3; idx[ax] = [0, K]; m[tuple(idx)] = False
    v = np.where(m, sd2, np.nan)
    print("direction", dv, "largest second difference of c^2:", np.nanmax(v), " (negative = concave); most negative", np.nanmin(v))
