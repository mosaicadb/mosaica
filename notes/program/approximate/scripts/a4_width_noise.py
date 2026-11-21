"""A4, synthetic only: the width (zeta + psi0 sqrt f) tau with mu, sigma, tau and f taken from m sampled rows.

Rows of a cluster have (s, e) jointly normal with correlation rho_es (the sample's own statistic), f = 1 - Var(e)/Var(s).
A neighbour at the threshold is given the measured law: e = slope * zeta * tau + sqrt(f) tau * g, g a unit scatter, normal
or Laplace. Printed: the probability that it is kept, with exact statistics and with sampled ones.

usage: run-limited.sh 6 1 2 python3 a4_width_noise.py
"""
import math
import numpy as np
from scipy.stats import norm

rng = np.random.default_rng(5)
lap_cdf = lambda x: np.where(x < 0, 0.5 * np.exp(math.sqrt(2) * x), 1 - 0.5 * np.exp(-math.sqrt(2) * x))
print("f     rho_es  zeta  slope  m    psi0: kept with exact statistics (normal | Laplace scatter) -> with sampled statistics (normal | Laplace)")
for f, rho in ((0.82, 0.56), (0.92, 0.7), (0.71, 0.65)):
    tau = math.sqrt(1 - f)
    for zeta in (3.0, 5.0):
        for slope in (1.0, 1.2):
            for m in (32, 64):
                # sample (s, e): s ~ N(0,1), e = rho tau s + sqrt(1 - rho^2) tau z
                S = rng.normal(size=(6000, m)); E = rho * tau * S + math.sqrt(1 - rho * rho) * tau * rng.normal(size=(6000, m))
                mu, sd, te = S.mean(1), S.std(1, ddof=1), E.std(1, ddof=1)
                fh = np.clip(1 - te ** 2 / sd ** 2, 0.0, 1.0)
                line = f"{f:.2f}  {rho:.2f}    {zeta:.0f}     {slope:.1f}    {m:3d} "
                for psi0 in (2.0, 3.0):
                    w_true = (zeta + psi0 * math.sqrt(f)) * tau
                    w_hat = (np.maximum((zeta - mu) / sd, 0) + psi0 * np.sqrt(fh)) * te
                    g_true = (w_true - slope * zeta * tau) / (math.sqrt(f) * tau)
                    g_hat = (w_hat - slope * zeta * tau) / (math.sqrt(f) * tau)
                    line += f"  {psi0:.0f}: {norm.cdf(g_true):.3f} | {float(lap_cdf(np.array(g_true))):.3f} -> {norm.cdf(g_hat).mean():.3f} | {lap_cdf(g_hat).mean():.3f}"
                print(line)
