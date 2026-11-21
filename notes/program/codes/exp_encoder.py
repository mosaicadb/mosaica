# Encoder-only changes to the built code (no change to the stored format's meaning, the tables or the decode):
#  (a) a finer set of scales than the six of LAMS (the scale is not stored; kappa absorbs it);
#  (b) weights on the rises in the encoder's objective, sum over rises of w_k log tan^2_k, which moves the nesting loss
#      between a block's rises.
# Also the cost of removing the nesting loss outright: the one-rate figures of the gap table are what a code
# re-quantised at each rise given the earlier bits could reach, and its decode needs every bit of the position in the
# nibble, old and new.
import math
import numpy as np
from clib import *

NV = 2000


def encode_w(Y, rd, lams, w):
    nv = len(Y)
    maxd = rd[-1]
    rms = np.sqrt((Y * Y).mean(1, keepdims=True))
    best = np.full(nv, np.inf)
    out = np.zeros((nv, len(rd)))
    for lam in lams:
        cell = built_values(Y / (lam * rms), maxd[None, :])
        t2 = np.stack([tan2(built_decode(cell, maxd[None, :], d[None, :]), Y) for d in rd], axis=1)
        obj = (np.log(np.maximum(t2, 1e-300)) * w[None, :]).sum(1)
        bt = obj < best
        best[bt] = obj[bt]
        out[bt] = t2[bt]
    return np.sqrt(out.mean(0))


if __name__ == "__main__":
    fine = list(np.round(np.exp(np.linspace(math.log(1.6), math.log(5.0), 24)), 4))
    for m, sched in ((96, [2, 3, 4, 7]), (128, [1, 3, 4]), (256, [0.25, 1, 2]), (128, [2, 3, 6]), (128, [3, 4, 7])):
        Y = sphere(NV, m, 100 + m)
        rd = [depths(r, m) for r in sched]
        one = np.ones(len(sched))
        base = encode_w(Y, rd, LAMS, one)
        print(f"--- m = {m}, schedule {sched} ---")
        print(f"  built (six scales, equal weights):  tan {np.round(base, 4)}")
        f = encode_w(Y, rd, fine, one)
        print(f"  24 scales from 1.6 to 5.0:          tan {np.round(f, 4)}  ratio {np.round(f / base, 3)}")
        for k in range(len(sched)):
            for wt in (3.0, 10.0):
                w = one.copy()
                w[k] = wt
                r = encode_w(Y, rd, fine, w)
                print(f"  weight {wt:>4} on rise {sched[k]:<4} (24 scales): tan {np.round(r, 4)}  ratio {np.round(r / base, 3)}")
        alone = np.array([encode_w(Y, [d], fine, np.ones(1))[0] for d in rd])
        print(f"  each rise alone (no nesting, 24 scales): tan {np.round(alone, 4)}  ratio {np.round(alone / base, 3)}", flush=True)
