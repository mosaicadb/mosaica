"""C1: is the pivot interval N1(c) exact two-sided, and what are its one-sided rates? d=12, M=512, s=0.4."""
import numpy as np
from c1_converse import q
rng = np.random.default_rng(7)
d, M, delta, s = 12, 512, 0.01, 0.4
qd = q(d, delta)
code = rng.standard_normal((M, d + 1)); code /= np.linalg.norm(code, axis=1, keepdims=True)
n, batch = 2000000, 4000
up = lo = piv = empty = 0
for _ in range(n // batch):
    u = rng.standard_normal((batch, d + 1)); u /= np.linalg.norm(u, axis=1, keepdims=True)
    v = rng.standard_normal((batch, d + 1)); v -= (v * u).sum(1, keepdims=True) * u
    v /= np.linalg.norm(v, axis=1, keepdims=True)
    Dl = s * u + np.sqrt(1 - s * s) * v
    dots = u @ code.T; idx = dots.argmax(1); cosf = dots[np.arange(batch), idx]
    E0 = (Dl * code[idx]).sum(1); sinf = np.sqrt(1 - cosf ** 2); tanf = sinf / cosf
    k = qd * qd * tanf ** 2
    U = (E0 - cosf * s) / (sinf * np.sqrt(1 - s * s))
    piv += np.count_nonzero(np.abs(U) > qd)
    disc = 1 + k - (E0 / cosf) ** 2
    empty += np.count_nonzero(disc < 0)
    c = (E0 / cosf) / (1 + k); hw = qd * tanf * np.sqrt(np.maximum(disc, 0)) / (1 + k)
    up += np.count_nonzero(s - c > hw); lo += np.count_nonzero(c - s > hw)
print(f"n={n} s={s} delta={delta}: |U|>q {piv/n:.5f}  truth above {up/n:.5f}  truth below {lo/n:.5f}  "
      f"sum {(up+lo)/n:.5f}  empty set {empty/n:.5f}  se(one side) {np.sqrt(delta/n):.5f}")
