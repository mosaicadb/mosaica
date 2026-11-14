# Flat-spectrum Gaussian model of a query-chosen read order (no data): D equal coordinates, exact reads, single block.
# Fixed order: reading a share f leaves the bound at (1 - f) of the zero-read slack.
# Query-chosen order: read the coordinates with the largest g_i^2; bound = sqrt(1 - E(f)) * sqrt(1 - f), E(f) the energy
# share of the top f of chi-square(1) variables. Also the ceiling of Proposition D for any dictionary of M atoms.
import numpy as np
from scipy.stats import norm
from scipy.optimize import brentq
def E(f): z = norm.isf(f / 2); return f + 2 * z * norm.pdf(z)
print("target bound / zero-read slack R | share read: fixed, query-chosen | ratio")
for R in (0.9, 0.8, 0.7, 0.5, 0.3, 0.2, 0.1):
    fa = brentq(lambda f: (1 - E(f)) * (1 - f) - R * R, 1e-9, 1 - 1e-9); print(f"  R={R:4.2f}   fixed {1-R:5.3f}   chosen {fa:6.4f}   ratio {(1-R)/fa:4.2f}")
print("Proposition D: least k with k(1+2sqrt(L)+2L)+sqrt(pi k)+2 >= (1-R^2) D, L = ln(eM/k)")
for D, M in ((160, 768), (100, 100), (400, 5000)):
    for R in (0.8, 0.5):
        k = next(k for k in range(1, 10**5) if k * (1 + 2 * np.sqrt(np.log(np.e * M / k)) + 2 * np.log(np.e * M / k)) + np.sqrt(np.pi * k) + 2 >= (1 - R * R) * D)
        print(f"  D={D} M={M} R={R}: k >= {k}   (fixed order in the flat model: {(1-R)*D:.0f})")
