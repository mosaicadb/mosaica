"""Referee 2, A4.5 item 2: what the measured kept shares say about the scatter once a neighbour's slack is counted.
A4.5's own law: a neighbour at height x has e = m x tau + sqrt(f) tau g. It is kept by (zeta + psi0 sqrt f) tau iff
g <= psi0 - (m - 1) zeta / sqrt f + (x - zeta)(1/sqrt(1 - f) - m)/sqrt f.  Of the ten, one is at the threshold and nine
lie above it by an exponential excess of rate 2.2 a spread (A2.4). Printed: the miss at the threshold and over the
ten, for a normal and a Laplace scatter of unit spread, against the recorded misses. Then the sampling-noise formula
of A4.7 against the author's simulation, with and without the two terms it leaves out. Synthetic."""
import math
import numpy as np
from scipy.stats import norm
rng = np.random.default_rng(4)
lap_sf = lambda v: np.where(v < 0, 1 - 0.5 * np.exp(math.sqrt(2) * v), 0.5 * np.exp(-math.sqrt(2) * v))
meas = {"yahoo": (0.82, 0.99, 0.024, 0.008), "LAION, 64": (0.71, 0.92, 0.031, 0.015), "LAION, 8": (0.67, 0.98, 0.036, 0.019),
        "iNaturalist, 12": (0.92, 1.21, 0.082, 0.025), "iNaturalist, 1": (0.86, 1.24, 0.082, 0.028)}
exc = np.concatenate([[0.0], rng.exponential(1 / 2.2, 9 * 200000)]); wts = np.concatenate([[0.1], np.full(9 * 200000, 0.9 / (9 * 200000))])
print("index: recorded miss at psi0 2 / 3 (ratio) | slope m used | at the threshold, normal; Laplace | over the ten, normal; Laplace")
for name, (f, m, m2, m3) in meas.items():
    for mm in sorted({1.0, m}):
        line = f"  {name:16s} {m2:.3f} / {m3:.3f} ({m2 / m3:.1f}) | m {mm:.2f} |"
        for zeta in (3.0,):
            cells = []
            for sf in (norm.sf, lap_sf):
                at = [float(sf(np.array(p - (mm - 1) * zeta / math.sqrt(f)))) for p in (2.0, 3.0)]
                allten = [float((wts * sf(p - (mm - 1) * zeta / math.sqrt(f) + exc * (1 / math.sqrt(1 - f) - mm) / math.sqrt(f))).sum()) for p in (2.0, 3.0)]
                cells.append((at, allten))
            line += f" {cells[0][0][0]:.4f} / {cells[0][0][1]:.4f}; {cells[1][0][0]:.4f} / {cells[1][0][1]:.4f} | {cells[0][1][0]:.4f} / {cells[0][1][1]:.4f}; {cells[1][1][0]:.4f} / {cells[1][1][1]:.4f}"
        print(line)
print("\nA4.7's nu^2 = (zeta^2 (1 - rho^2) + 1 + psi0^2 f/2)/(m f) against the full first-order variance, and the author's simulated kept share (a4_width_noise.txt, normal scatter, psi0 2):")
sim = {(0.82, 0.56, 3, 32): 0.953, (0.82, 0.56, 3, 64): 0.966, (0.82, 0.56, 5, 32): 0.930, (0.82, 0.56, 5, 64): 0.953, (0.92, 0.70, 3, 32): 0.957, (0.92, 0.70, 5, 32): 0.941, (0.71, 0.65, 5, 32): 0.935}
for (f, rho, zeta, m), got in sim.items():
    p0 = 2.0
    nu_a = (zeta ** 2 * (1 - rho ** 2) + 1 + p0 ** 2 * f / 2) / (m * f)
    c1 = zeta - p0 * (1 - f) / math.sqrt(f); c2 = p0 * math.sqrt(f)
    nu_b = (c1 ** 2 * (1 - rho ** 2) + c2 ** 2 / 2 + c1 * c2 * (1 - rho ** 2) + 1) / (m * f)
    print(f"  f {f} rho {rho} zeta {zeta} m {m}: A4.7 {norm.cdf(p0 / math.sqrt(1 + nu_a)):.3f}, full {norm.cdf(p0 / math.sqrt(1 + nu_b)):.3f}, simulated {got:.3f};  height from the whole cluster, tau sampled: nu^2 = (zeta + psi0 sqrt f)^2/(2 m f) gives {norm.cdf(p0 / math.sqrt(1 + (zeta + p0 * math.sqrt(f)) ** 2 / (2 * m * f))):.3f}")
