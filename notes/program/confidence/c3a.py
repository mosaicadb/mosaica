# C3 (a): break-even bits between a rotated block and a deterministic certificate of the same coordinates.
import math, sys, numpy as np
from design_lib import *
print("flat spectrum. B0 = bits at which the ideal block beats its own zero-bit Cauchy-Schwarz bound, (n/2) log2(1 + c^2/d);")
print("  Bs = sign bits for the same (pi/2) c^2 n / (c^2 + d); B* = bits at which it beats the deterministic floor at the same bits, (n/2) log2(d/(d - c^2))")
for c in (4.0, 6.93, 8.0):
    for n in (32, 64, 96, 128, 256, 288, 768, 100000):
        d = n - 1
        B0 = n / 2 * math.log2(1 + c * c / d); Bs = math.pi / 2 * c * c * n / (c * c + d)
        Bst = n / 2 * math.log2(d / (d - c * c)) if d > c * c else float("inf")
        print(f"  c={c:5.2f} n={n:6d}  B0={B0:7.1f}  signs={Bs:7.1f}{' (more than n)' if Bs > n else ''}  B*={Bst:7.1f}")
def breakeven(a, c, tfun, rdet=1.0, grid=None):
    n = len(a); d = n - 1; F = a.sum(); out = None
    for B in (grid if grid is not None else np.arange(1, 16 * n + 1)):
        det = rdet * wf(a, B * math.log(2))[0]; rnd = c * F * math.sqrt(tfun(B / n) / d)
        if rnd <= det: return int(B), det / F
    return None, None
S = spectra()
segs = [(0, 96), (96, 224), (224, 480), (480, 768), (0, 224), (0, 768)]
for g in (12, 2):
    a, items = S[g]
    lam32 = wf(a, 256 * math.log(2))[1]; ap = np.minimum(a, lam32)
    print(f"group {g}: F = {a.sum():.4f}; water level after 256 ideal prefix bits {lam32:.3e}, coordinates above it {(a > lam32).sum()}, sum left {ap.sum():.4f}")
    for nm, arr in (("raw spectrum", a), ("after a 256-bit ideal prefix", ap)):
        for lo, hi in segs:
            seg = arr[lo:hi]; n = hi - lo; keff = seg.sum() / seg.max(); row = []
            for c in (8.0, 4.0):
                for tf, rd, lab in ((t_ideal, 1.0, "ideal"), (t_built, RHO_DET, "built")):
                    B, w = breakeven(seg, c, tf, rd)
                    B0 = next((b for b in range(1, 16 * n) if c * c * tf(b / n) / (n - 1) <= 1), None)
                    row.append(f"c={c:.0f} {lab}: B0={B0} B*={B}" + (f" (width {w:.2f} of the segment's sum)" if B else ""))
            print(f"  {nm:30s} [{lo:3d},{hi:3d}) sum {seg.sum():.4f} k_eff {keff:6.1f} | " + " | ".join(row))
