# Candidate L: keep the built uniform grid for every rise but the block's last, and code the last plane (or the last
# two planes) of the last rise as an additive trellis stream on the grid's residual. Earlier rises are bit for bit the
# built code's. The last rise has the same bits and the same lookups; its nibbles' tables cover J p positions.
#   last = (planes, p, J): the last `planes` bits per position are replaced by a stream of one nibble per p positions
#   (4 / p = planes).
import math
import sys
import numpy as np
from clib import *
from streams import Stream, Nested
from exp_onerate import codebook

CASES = {
    "256:q-1-2 J2": (256, [0.25, 1, 2], (1, 4, 2)),
    "256:q-1-2 J3": (256, [0.25, 1, 2], (1, 4, 3)),
    "128:1-3-4 J2": (128, [1, 3, 4], (1, 4, 2)),
    "128:1-3-4 J3": (128, [1, 3, 4], (1, 4, 3)),
    "96:2-3-4-7 J2": (96, [2, 3, 4, 7], (1, 4, 2)),
    "96:2-3-4-7 J3": (96, [2, 3, 4, 7], (1, 4, 3)),
    "96:2-3-4-7 two planes J2": (96, [2, 3, 4, 7], (2, 2, 2)),
    "128:2-3 J2": (128, [2, 3], (1, 4, 2)),
    "128:2-3 J3": (128, [2, 3], (1, 4, 3)),
    "128:3-4-7 J3": (128, [3, 4, 7], (1, 4, 3)),
    "128:2-3-6 J3": (128, [2, 3, 6], (1, 4, 3)),
    "128:2-4 two planes J2": (128, [2, 4], (2, 2, 2)),
}


def grid_codes(Y, rd, lam):
    """The built code of every rise in rd at scale lam, in grid units (the block divided by lam rms)."""
    rms = np.sqrt((Y * Y).mean(1, keepdims=True))
    V = Y / (lam * rms)
    maxd = rd[-1]
    cell = built_values(V, maxd[None, :])
    return V, [built_decode(cell, maxd[None, :], d[None, :]) for d in rd]


def run(name, ntrain=None, ntest=600, iters=30):
    m, rises, (planes, p, J) = CASES[name]
    final = int(rises[-1])
    rd_built = [depths(r, m) for r in rises]
    rd_grid = rd_built[:-1] + [depths(final - planes, m)]      # the grid stops `planes` short at the last rise
    ntrain = ntrain or max(400, 60000 * p // m)
    # training residuals at the scales the built encoder picks for the grid part
    Ytr = sphere(ntrain, m, 7000 + m)
    _, pick = built_encode(Ytr, rd_grid)
    res = np.zeros_like(Ytr)
    for li, lam in enumerate(LAMS):
        s = pick == li
        if s.any():
            V, codes = grid_codes(Ytr[s], rd_grid, lam)
            res[s] = V - codes[-1]
    step = 2.0 ** (1 - (final - planes))                         # cell width of the grid in grid units
    # start from the built planes themselves (the stream then can only improve on them on the training blocks)
    if planes == 1:
        init = np.array([[(1.0 if i >> b & 1 else -1.0) for b in range(4)] for i in range(16)]) * step / 4
    else:
        u = np.array([-3.0, -1.0, 1.0, 3.0]) * step / 8
        init = np.array([[u[i & 3], u[i >> 2]] for i in range(16)])
    st = Stream(p, J, 0, seed=1, init=init, scale=step / 4)
    nest = Nested([st], 1)
    nest.train(res, iters=iters, rounds=0)
    # test: per item the scale that minimises the sum over rises of log tan^2, as rand_choose does
    Yte = sphere(ntest, m, 2000 + m)
    best = np.full(ntest, np.inf)
    out = np.zeros((ntest, len(rises)))
    for lam in LAMS:
        V, codes = grid_codes(Yte, rd_grid, lam)
        nib = st.encode([(0, 1.0, V - codes[-1])])
        last = codes[-1] + st.decode(nib, m)
        t2 = np.stack([tan2(c, Yte) for c in codes[:-1]] + [tan2(last, Yte)], axis=1)
        obj = np.log(np.maximum(t2, 1e-300)).sum(1)
        bt = obj < best
        best[bt] = obj[bt]
        out[bt] = t2[bt]
    bt2, _ = built_encode(Yte, rd_built)
    print(f"--- {name}: last {planes} plane(s) as a stream of one nibble per {p} positions, J = {J}; {ntrain} training blocks ---")
    for k, r in enumerate(rises):
        reff = bits_of(r, m) / m if r < 1 else r
        t, b, bd = math.sqrt(out[:, k].mean()), math.sqrt(bt2[:, k].mean()), math.sqrt(bound_tan2(reff))
        print(f"  rise {r}: tan {t:.4f}; built {b:.4f}; ratio {t / b:.3f}; / bound {t / bd:.3f}; built / bound {b / bd:.3f}", flush=True)


if __name__ == "__main__":
    for name in (sys.argv[1:] or list(CASES)):
        run(name)
