# Test: is the exact quantile c(w) at most the single-sphere quantile at the dimension matching the fourth cumulant,
# D_eff(w) = 1 / sum_j w_j^2 / (d_j + 2) - 2 ?
import numpy as np, math, itertools
from law import cw, c1
L1 = math.log(2 * 1e9 * 4 / 1e-9)
ds = [95, 127, 255, 287]
def deff(w, ds):
    return 1 / sum(x * x / (d + 2) for x, d in zip(w, ds)) - 2
rng = np.random.default_rng(3)
pts = [[0.9, 0, 0, 0.1], [0.5, 0, 0, 0.5], [0.1, 0, 0, 0.9], [0.5, 0.5, 0, 0], [0, 0, 0.5, 0.5], [0.3, 0, 0, 0.7], [0.7, 0, 0, 0.3], [0.2, 0, 0, 0.8],
       [0.25] * 4, [0.336, 0.398, 0.26, 0.006], [0.068, 0.149, 0.64, 0.143], [0.4, 0.4, 0.1, 0.1], [0.6, 0.1, 0.1, 0.2], [0.05, 0.05, 0.45, 0.45]]
pts += [list(rng.dirichlet([0.7] * 4)) for _ in range(16)]
worst = -9
for w in pts:
    c = cw(w, ds, -L1); De = deff(w, ds); cs = c1(De, -L1)
    worst = max(worst, c - cs)
    print(np.round(w, 3), f"c(w) {c:.4f}  D_eff {De:7.1f}  c_sphere(D_eff) {cs:.4f}  diff {c - cs:+.4f}")
print("largest c(w) - c_sphere(D_eff):", worst)
