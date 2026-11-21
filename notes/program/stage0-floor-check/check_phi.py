# Independent check of the closed-form KKT solver for Phi against scipy's SLSQP on small spectra.
import math, numpy as np
from scipy import optimize
from common import Phi, kfun, standard_run_header
standard_run_header("check_phi.py")
rng = np.random.default_rng(3)
worst = 0
for t in range(40):
    n = int(rng.integers(1, 25)); gam = np.sort(np.exp(rng.normal(0, 2, n)))[::-1]; H = float(np.exp(rng.uniform(-4, 3.5)))
    a = np.sqrt(gam)
    best = np.inf
    for st in range(4):
        x0 = rng.uniform(-3, 0, n)
        r = optimize.minimize(lambda x: (a * np.exp(x)).sum(), x0, method="SLSQP", bounds=[(-60, 0)] * n,
                              constraints=[{"type": "ineq", "fun": lambda x: H - kfun(np.exp(x)).sum()}], options={"ftol": 1e-14, "maxiter": 2000})
        if r.success and kfun(np.exp(r.x)).sum() <= H * (1 + 1e-9):
            best = min(best, r.fun)
    ph = Phi(gam, H)
    worst = max(worst, (ph - best) / best)
    if t < 8: print("n=%2d H=%8.4f  KKT %.10g  SLSQP %.10g" % (n, H, ph, best))
print("max relative excess of the KKT value over SLSQP's best feasible value: %.2e (positive would mean the KKT solver is not the minimum)" % worst)
