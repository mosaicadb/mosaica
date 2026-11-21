# F4 of CLAIMS.md: the isotropic low-rate floor E|g_n| * rho_n(e^-H) against Theorem 1's kappa_n e^(-H/n), and a
# numerical look at the convexity of u -> rho_n(e^-u) that its proof derives from the log-concavity of the chi law.
# Run:  nice -n 10 taskset -c 6 env OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 check_f4.py
import math
import numpy as np
from scipy import stats
import floors_lib as fl

for n in (1, 2, 8, 76, 768):
    Eg = math.sqrt(2) * math.exp(math.lgamma((n + 1) / 2) - math.lgamma(n / 2))
    u = np.linspace(1e-4, 12, 4000); rho = stats.chi.ppf(np.exp(-u), n)
    d2 = np.diff(rho, 2)
    row = "n = %3d: min second difference of rho_n(e^-u) on a grid = %.2e (convex if >= 0);" % (n, d2.min())
    row += " F4 / n and Theorem 1 / n at H = 0.01, 0.1, 0.5, 2 nats: "
    for H in (0.01, 0.1, 0.5, 2.0):
        row += "%.3f|%.3f  " % (Eg * stats.chi.ppf(math.exp(-H), n) / n, fl.kappa(n) * math.exp(-H / n) / n)
    print(row)
# per-cell statement on centred and off-centre balls, isotropic: E h = rho E|g| >= E|g| rho_n(P)
worst = 9e9
for n in (2, 5, 50):
    for rad in np.linspace(0.2, 2.0, 30) * math.sqrt(n):
        for shift in (0.0, 1.0, 3.0):
            P = stats.ncx2.cdf(rad * rad, n, shift * shift); Pc = stats.ncx2.sf(rad * rad, n, shift * shift)
            if P > 1e-300 and Pc > 1e-10:               # skip masses that round to 1, where the quantile overflows
                worst = min(worst, rad / (stats.chi.isf(Pc, n) if P > 0.5 else stats.chi.ppf(P, n)))
print("balls of radius rad centred at distance 0, 1, 3 from the origin: min rad / rho_n(P) = %.4f (>= 1 required; 1 for centred balls)" % worst)
