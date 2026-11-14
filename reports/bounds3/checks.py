# Numerical checks of three statements in the report (not proofs):
#  (1) R1: the tail of sum_j a_j U_j at x > 0 does not fall when one scale rises;
#  (2) R4: the stop-loss transform of X = sum_j a_j U_j is at most that of s U_D, s^2 = sum_j a_j^2 / m_j^2, and the
#      resulting tail bound is above the exact tail;
#  (3) R8: on the event S - E <= c sqrt(V - sum_j t_j^2 / W_j) (one-sided), S <= the self-normalised upper end, with
#      the deterministic end when E^2 > V W.
import numpy as np, math
import mpmath as mp
from law import tail
rng = np.random.default_rng(7)
ds = [95, 127, 255, 287]
# (1)
worst = 0.0
for _ in range(300):
    w = rng.dirichlet([0.7] * 4); a = [math.sqrt(x * d) for x, d in zip(w, ds)]
    x = rng.uniform(0.5, 9.5); j = rng.integers(4); up = list(a); up[j] *= 1 + rng.uniform(0.001, 0.3)
    worst = max(worst, tail(a, ds, x)[1] - tail(up, ds, x)[1])
print(f"(1) largest ln T(a) - ln T(a with one scale raised) over 300 cases: {worst:.2e} (must not be positive beyond rounding)")
# (2)
mp.mp.dps = 30
D = sum(ds)
m = [float(mp.exp(mp.loggamma(mp.mpf(d) / 2 + 0.5) + mp.loggamma(mp.mpf(D) / 2) - mp.loggamma(mp.mpf(d) / 2) - mp.loggamma(mp.mpf(D) / 2 + 0.5))) for d in ds]
def stoploss_sphere(t):     # E (U_D - t)_+
    lC = mp.loggamma(mp.mpf(D) / 2) - mp.log(mp.pi) / 2 - mp.loggamma(mp.mpf(D - 1) / 2)
    return float(mp.exp(lC + (mp.mpf(D - 1) / 2) * mp.log(1 - t * t)) / (D - 1) - t * mp.betainc(mp.mpf(D - 1) / 2, mp.mpf(1) / 2, 0, 1 - t * t, regularized=True) / 2)
def stoploss_X(a, t, n=400):  # int_t^inf T(a; u) du by Gauss-Legendre on [t, t + 12 sd]
    xs, ws = np.polynomial.legendre.leggauss(n); sig = math.sqrt(sum(x * x / d for x, d in zip(a, ds)))
    hi = min(t + 12 * sig, sum(a)); u = (hi - t) / 2 * xs + (hi + t) / 2
    return (hi - t) / 2 * sum(wi * tail(a, ds, ui)[0] for wi, ui in zip(ws, u))
bad = 0; worst = -1e9
for w in ([0.1254, 0.167, 0.333, 0.3746], [0.25] * 4, [0.6, 0.2, 0.15, 0.05], [0.02, 0.03, 0.05, 0.9]):
    a = [math.sqrt(x * d) for x, d in zip(w, ds)]; s = math.sqrt(sum(x * x / mj ** 2 for x, mj in zip(a, m)))
    for t in (1.0, 3.0, 5.0, 7.0):
        lx = stoploss_X(a, t, n=60); ly = s * stoploss_sphere(mp.mpf(t) / s)
        worst = max(worst, math.log(lx) - math.log(ly)); bad += lx > ly * (1 + 1e-9)
print(f"(2) stop-loss of X against that of s U_D at 16 (shares, t): violations {bad}; largest ln ratio {worst:.4f} (must be <= 0)")
# (3)
bad = 0; n = 200000
for _ in range(n):
    J = rng.integers(1, 5); dj = rng.choice([95, 127, 255, 287], J, replace=False).astype(float)
    Rj = rng.uniform(0.2, 2, J); tan = 10 ** rng.uniform(-1.5, 0.5, J); aj = Rj * tan      # a_j = A_j |Delta_j| = R_j tan_j
    tj = Rj * rng.uniform(-1, 1, J) * rng.choice([1.0, 0.3, 0.05])                         # true block scores, |t_j| <= R_j
    c = rng.uniform(1, 9)
    V = (aj ** 2 / dj).sum(); Wj = dj * Rj ** 2 / aj ** 2; W = Wj.sum(); S = tj.sum()
    sig_true = math.sqrt(max(V - (tj ** 2 / Wj).sum(), 0))
    err = rng.uniform(-1, 1) * c * sig_true          # S - E, any value allowed by the (two-sided) event
    E = S - err
    k = c * c / W
    if E * E <= V * W:
        disc = c * c * V * (1 + k) - k * E * E
        up = (E + math.sqrt(disc)) / (1 + k)
    else:
        up = Rj.sum()
    bad += S > up + 1e-9
print(f"(3) upper end violated in {bad} of {n} random cases")
