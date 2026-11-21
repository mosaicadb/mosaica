# One-rate stream codes at 1/4, 1/2, 1 and 2 bits: product (J = 1), additive trellis (J = 2, 3; decode unchanged) and
# the unconstrained two-nibble window (kind free; a 256-entry table, reference only), against the built code.
import math
import sys
import numpy as np
from clib import *
from streams import Stream, Nested

from cand_product import lbg

_cb = {}


def codebook(k):
    """The best of three Lloyd 16-point codebooks of N(0, I_k) (cand_product.lbg)."""
    if k not in _cb:
        _cb[k] = min((lbg(k, seed=10 * k + i) for i in range(3)), key=lambda r: r[1])[0]
    return _cb[k]



def run(m, p, J, kind, ntrain, ntest, iters, seed=0):
    Xtr = sphere(ntrain, m, 1000 + seed)
    Xtr = Xtr / np.sqrt((Xtr * Xtr).mean(1, keepdims=True))
    Yte = sphere(ntest, m, 2000 + m)
    s = Stream(p, J, 0, kind="add", seed=seed, init=codebook(p))
    nest = Nested([s], 1)
    D = nest.train(Xtr, iters=iters, rounds=0)
    if kind == "free":
        s.set_free()
        D = nest.train(Xtr, iters=iters, rounds=0)
    t2 = nest.tan2(Yte, rounds=0)[:, 0]
    return float(D[0]), math.sqrt(t2.mean())


if __name__ == "__main__":
    m = 256
    which = sys.argv[1:] or ["2", "1", "0.5", "0.25"]
    for Rs in which:
        R = float(Rs)
        p = int(round(4 / R))
        Yte = sphere(400, m, 2000 + m)
        bt2, _ = built_encode(Yte, [depths(R, m)])
        base = math.sqrt(bt2.mean())
        bound = math.sqrt(bound_tan2(R))
        print(f"--- R = {R}, m = {m}: built {base:.4f}, asymptotic bound {bound:.4f} ---", flush=True)
        # training blocks: about 80,000 nibble steps for additive pulses, 250,000 for the free window (256 cells)
        for J, kind, steps, iters in ((1, "add", 80000, 15), (2, "add", 80000, 25), (2, "free", 250000, 25), (3, "add", 40000, 20)):
            ntrain = max(300, steps * p // m)
            D, t = run(m, p, J, kind, ntrain, 400, iters)
            print(f"  J={J} {kind:<4}: training D {D:.4f} (tan {math.sqrt(D / (1 - D)):.4f}); test tan {t:.4f}; / built {t / base:.3f}; / bound {t / bound:.3f}", flush=True)
