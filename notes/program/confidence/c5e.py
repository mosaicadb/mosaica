# C5: one-block first levels that cost the same scalars as the built one (one block-rise, 24 B of code, the 256-bit prefix kept).
import math, numpy as np
from scipy.optimize import brentq
from scipy.special import betainc
from design_lib import *
def c1(d):
    f = lambda s: math.log(0.5 * betainc((d - 1) / 2, 0.5, 1 - s * s) + 1e-300) + 35.97
    return brentq(f, 1e-3, 0.999) * math.sqrt(d)
a, _ = spectra()[12]; F = float(a.sum()); lam = wf(a, 256 * math.log(2))[1]; ap = RHO_DET * np.minimum(a, lam) / F; tot = float(ap.sum())
print(f"model norms (group 12, built curves): width at 32 B {tot:.4f}")
def one(m, bits=192.0, norms=ap):
    A = float(norms[:m].sum()); R = bits / m; c = c1(m - 1); k = c * c * t_built(R) / (m - 1); rest = float(norms[m:].sum())
    return rest + A * math.sqrt(k), rest + A * math.sqrt(k / (1 + k)), R, c
base = one(96)
for m in (96, 128, 160, 192, 224, 256, 320, 384):
    p, w, R, c = one(m); print(f"  one block of {m:3d} positions at {R:.2f} bit, c = {c:.3f}: plain {p:.4f} ({p / base[0] - 1:+.1%}) | shrunk centre {w:.4f} ({w / base[1] - 1:+.1%})")
# the same with block 0 made 1.5 times heavier relative to the rest, as the 50k dump's level-0 means suggest (0.039 against 0.043)
nd = ap.copy(); s0 = nd[:96].sum(); sr = nd[96:].sum(); nd[:96] *= (0.0394 / 0.0821) * tot / s0; nd[96:] *= (0.0427 / 0.0821) * tot / sr
base = one(96, norms=nd); print("norms rescaled so that block 0 holds 48% of the sum after the prefix, as in the dump (the model has 38%):")
for m in (96, 128, 160, 192, 224, 256):
    p, w, R, c = one(m, norms=nd); print(f"  one block of {m:3d} positions at {R:.2f} bit: plain {p:.4f} ({p / base[0] - 1:+.1%}) | shrunk centre {w:.4f} ({w / base[1] - 1:+.1%})")
