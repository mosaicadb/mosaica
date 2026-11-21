# What the phased nested trellis changes in the stored scalars and in the tables, on the block of 256 at 1/4-1-2 bits.
#  - spread over items of tan(phi) and of kappa / |y| per rise (the grids that store them are calibrated to these);
#  - the range of a nibble's table, <w, pulse>, summed over the nibbles of a rise, for a Gaussian weight vector w: the
#    16-bit table quantisation of kern.rs is in units of this sum;
#  - multiply-adds to build a nibble's table.
import math
import sys
import numpy as np
from clib import *
from exp_nested import CONFIGS, make
from streams import Nested

if __name__ == "__main__":
    name = sys.argv[1] if len(sys.argv) > 1 else "q12_p1"
    m, rises, sdef, emph = CONFIGS[name]
    Xtr = sphere(781, m, 3000)
    Xtr = Xtr / np.sqrt((Xtr * Xtr).mean(1, keepdims=True))
    nest = Nested([make(d, i) for i, d in enumerate(sdef)], len(rises), emph)
    nest.train(Xtr, iters=14, rounds=2)
    Y = sphere(500, m, 2000 + m)
    rms = np.sqrt((Y * Y).mean(1, keepdims=True))
    nibs, codes = nest.encode(Y / rms, 4)
    yn = np.sqrt((Y * Y).sum(1))
    rd = [depths(r, m) for r in rises]
    # the built code at the scale rand_choose picks
    bt2, pick = built_encode(Y, rd)
    print(f"--- {name}: spread over 500 items ---")
    for k, r in enumerate(rises):
        c = nest.prefix(codes, k)
        t = np.sqrt(tan2(c, Y))
        kap = yn * yn / (c * Y).sum(1) / yn          # kappa / |y|
        cb = np.zeros_like(Y)
        for li, lam in enumerate(LAMS):
            s = pick == li
            if s.any():
                cell = built_values(Y[s] / (lam * rms[s]), rd[-1][None, :])
                cb[s] = built_decode(cell, rd[-1][None, :], rd[k][None, :])
        tb = np.sqrt(tan2(cb, Y))
        kb = yn * yn / (cb * Y).sum(1) / yn
        print(f"  rise {r}: tan mean {t.mean():.4f} sd of ln tan {np.log(t).std():.4f} (built {tb.mean():.4f}, {np.log(tb).std():.4f}); "
              f"sd of log2(kappa/|y|) {np.log2(kap).std():.4f} (built {np.log2(kb).std():.4f})")
    # table ranges: one stream, phases by rise
    ph = nest.streams[0]
    rng = np.random.default_rng(5)
    W = rng.standard_normal((2000, ph.J * ph.p))
    print("--- table of a nibble: range of <w, pulse> over its 16 values, mean over Gaussian w, in units of |w| per position = 1 ---")
    for f in range(ph.P):
        pulse = np.concatenate([ph.G[i, f, :16] for i in range(ph.J)], axis=1)     # 16 x (J p)
        T = W @ pulse.T
        rg = (T.max(1) - T.min(1)).mean()
        print(f"  phase {f} (rise index {ph.rho[f]}): pulse over {ph.J * ph.p} positions, rms entry {np.sqrt((pulse ** 2).mean()):.4f}, mean range {rg:.4f}")
    # the built nibble at the same rises: 4 one-bit fields of +-delta
    for d, lab in ((1, "first bit (+-1/2 of lambda rms)"), (2, "second bit (+-1/4)")):
        delta = 2.0 ** -d * 2.0   # in units of rms at lambda = 2
        T = np.abs(W[:, :4]).sum(1) * 2 * delta
        print(f"  built nibble, {lab} at lambda = 2: 4 positions, mean range {T.mean():.4f}")
    print(f"multiply-adds per nibble table: built 16 x 4; phased 16 x {ph.J * ph.p}")
