# nbfit.py OUT [MODE PSI]: true neighbours in index-path clusters (OUT.nb, one rule and psi): how their level-0 error
# over tau0 (z0) scales with their height h = (s - mu) / sigma and with tau0 / sigma: least squares of z0 on h, of
# z0 on (tau0/sigma) h, and of log(z0 / h) on log(tau0 / sigma) over those with h > 1 and z0 > 0.
import sys
import numpy as np

out = sys.argv[1]
mode = sys.argv[2] if len(sys.argv) > 2 else "corr"
psi = float(sys.argv[3]) if len(sys.argv) > 3 else 9.0
N = np.genfromtxt(out + ".nb", names=True, dtype=None, encoding=None)
for st in sorted(set(N["set"])):
    m = (N["set"] == st) & (N["mode"] == mode) & (N["psi"] == psi)
    h, z, r = N["h"][m], N["z0"][m], N["tau0_sig"][m]
    ok = np.isfinite(z) & (h > 1) & (z > 0)
    a = np.sum(h * z) / np.sum(h * h)
    b = np.sum(r * h * z) / np.sum((r * h) ** 2)
    pfit = np.polyfit(np.log(r[ok]), np.log(z[ok] / h[ok]), 1)
    res_a = np.median(np.abs(z - a * h)) if len(h) else np.nan
    res_b = np.median(np.abs(z - b * r * h)) if len(h) else np.nan
    print(f"{out} {st} {mode} psi {psi:g}: {m.sum()} neighbours; median h {np.median(h):.2f} z0 {np.median(z):.2f} tau0/sigma {np.median(r):.3f};"
          f" z0 = {a:.2f} h (median |res| {res_a:.2f}); z0 = {b:.2f} (tau/sigma) h (|res| {res_b:.2f}); log(z0/h) = {pfit[0]:.2f} log(tau/sigma) + {pfit[1]:.2f}; corr(z0, h) {np.corrcoef(h, z)[0, 1]:.2f}")
