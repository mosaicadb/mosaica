# Arithmetic behind P2 and P4: the balanced-against-optimal distortion ratio for a Gaussian density (high-resolution
# formula), the expected largest of n normals, the radius ratio, the slopes of the floor table, the leaf-size formula.
import numpy as np
from math import log, exp, sqrt
print("balanced / optimal distortion, Gaussian density, D:", {D: round((1 - 2 / D) ** (-D / 2) / (1 + 2 / D) ** ((D + 2) / 2), 4) for D in (5, 10, 19, 40)})
print("second-order form exp(2/(D(D+2))):", {D: round(exp(2 / (D * (D + 2))), 4) for D in (5, 10, 19, 40)})
rng = np.random.default_rng(0)
print("E max of n standard normals (20,000 draws by the quantile transform):",
      {n: round(float(np.mean(__import__('scipy.stats').stats.norm.ppf(rng.random(20000) ** (1.0 / n)))), 2) for n in (1000, 2000, 4000, 16000)})
print("largest / typical radius at n_eff 76, 2,000 rows:", round(exp(3.43 / sqrt(2 * 76)), 3))
# floor W (group 12, floors section 7): bytes (code bytes + 6 of scalars) and widths
B = np.array([0, 4, 8, 16, 32, 96]); Wd = np.array([.0453, .0379, .0327, .0258, .0180, .0068])
nats = (8 * B + 48) * log(2); lw = -np.diff(nats) / np.diff(np.log(Wd)); e = 0.106
print("lambda*w between rows:", lw.round(0), " bits saved per doubling of K:", (e / 2 * lw).round(1), " bytes:", (e / 2 * lw / 8).round(2))
print("radius 10% larger = doublings of K lost at D = 19:", round(19 * np.log2(1.1), 2))
for nn in (128, 512, 768):
    c = 17 * nn + 136; print("leaf size, n", nn, [round((1 - g) / g * xi * c / 18.5) for g in (0.56, 0.43) for xi in (1, 3)])
for a, b, eps in ((10.3, 20.1, 0.055), (16.4, 55.8, 0.012), (5.7, 9.6, 0.15)):
    print("S* by time:", round(1000 * a * log(2) / (b * eps)))
print("S* by bytes, centre only:", [round(768 * log(2) / s) for s in (0.8, 2.4)], " by time:", [round(c * log(2) / s) for c, s in ((5500, 2.4), (10000, 0.8))])
