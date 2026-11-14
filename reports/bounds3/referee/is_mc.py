# Importance sampling check, different in kind from both the certificate and my FFT/inversion tails:
# proposal Y_j ~ N(x w_j, a_j^2 / d_j) (the Gaussian shifted to the rare event), weight = sphere density / proposal.
import numpy as np, math
from math import lgamma
from mytail import lnC, lntails_fft
rng = np.random.default_rng(11)
ds = [95, 127, 255, 287]
def run(w, x, n=4_000_000, reps=4):
    a = [math.sqrt(wj * d) for wj, d in zip(w, ds)]
    ests = []
    for _ in range(reps):
        tot = np.zeros(n); lw = np.zeros(n)
        for aj, d, wj in zip(a, ds, w):
            sg = aj / math.sqrt(d); mu = x * wj
            y = mu + sg * rng.standard_normal(n)
            s = y / aj
            with np.errstate(invalid="ignore", divide="ignore"):
                lf = np.where(np.abs(s) < 1, lnC(d) - math.log(aj) + (d - 3) / 2 * np.log1p(-np.minimum(s * s, 1 - 1e-300)), -np.inf)
            lq = -0.5 * ((y - mu) / sg) ** 2 - math.log(sg * math.sqrt(2 * math.pi))
            lw += lf - lq; tot += y
        ests.append(np.mean(np.where(tot >= x, np.exp(lw), 0.0)))
    e = np.array(ests)
    return math.log(e.mean()), e.std(ddof=1) / math.sqrt(reps) / e.mean()
w = [0.1254, 0.167, 0.333, 0.3746]
a = [math.sqrt(wj * d) for wj, d in zip(w, ds)]
xs, lt = lntails_fft(a, ds, 8.3, eta=0.0005, M=1 << 18)
for x, lnd in ((8.6951, -42.832826), (8.704, -42.832826), (8.7659, -43.525973), (8.774, -43.525973), (7.9485, -35.925071), (7.957, -35.925071), (8.0271, -36.618218), (8.036, -36.618218)):
    m, se = run(w, x)
    i = int(round(x / 0.0005))
    print(f"x {x}: IS ln tail {m:.4f} (rel se {se:.4f}); FFT {lt[i]:.4f}; ln delta {lnd:.4f}; tail <= delta: {m <= lnd}")
