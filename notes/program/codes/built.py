# The gap table: the built randomised code, reimplemented from rust/src/v2/mod.rs (rand_encode, rand_value,
# rand_choose, rate_depth) and rust/src/derive.rs (bits_of), measured on uniform directions at every block size and
# rise schedule in use, beside the one-rate values and the sphere bound.
#
# Schedules:
#   the six of rust/results/bounds/study/quant_study.py: 1-2, 2-3, 2-3-6, 1-2-3-4, 2-4, 3-4-7;
#   the four of group 0 of the 1M BioCLIP-2 build (rust/results/system/bd_p2c.out, "rates g0"):
#     block 0 (96): 2-3-4-7, block 1 (128): 1-3-4, block 2 (256): 0.25-1-2, block 3 (288): 0.25.
import math
import sys
import numpy as np
from clib import *

NV = 3000
STUDY = [[1, 2], [2, 3], [2, 3, 6], [1, 2, 3, 4], [2, 4], [3, 4, 7]]
G0 = {96: [2, 3, 4, 7], 128: [1, 3, 4], 256: [0.25, 1, 2], 288: [0.25]}


def one_rate_lm(Y, bits):
    """Lloyd-Max levels at the item's rms scale, one rate (not nested), gain free."""
    lev, edges, _ = lloyd_max(1 << bits)
    rms = np.sqrt((Y * Y).mean(1, keepdims=True))
    idx = np.searchsorted(edges[1:-1], Y / rms)
    return tan2(lev[idx], Y)


def fmt(t2):
    a, b, _ = summ(t2)
    return f"{math.sqrt(a):.4f}"


def run(m, sched, Y, tag):
    rd = [depths(r, m) for r in sched]
    t2, pick = built_encode(Y, rd)
    rows = []
    for k, r in enumerate(sched):
        nested = t2[:, k]
        alone, _ = built_encode(Y, [rd[k]])
        alone = alone[:, 0]
        if r >= 1:
            lm = one_rate_lm(Y, int(r))
            lm_s = f"{math.sqrt(lm.mean()):.4f}"
            cont_lam, cont = golden(lambda l: uniform_tan2(int(r), l), 0.5, 8.0)
        else:
            lm_s = "   -  "
        reff = bits_of(r, m) / m if r < 1 else r
        lo, rc, _ = __import__("bounds").finite_bounds(m, reff)
        bt = math.sqrt(bound_tan2(reff))
        mt2, mt, _ = summ(nested)
        rows.append((m, tag, r, reff, mt2, math.sqrt(mt2), mt, math.sqrt(alone.mean()), lm_s, bt, math.sqrt(lo[0])))
        print(f"m={m:<3} {tag:<10} rise {str(r):<5} R={reff:<6.4f} nested: tan2 {mt2:.4f} rms-tan {math.sqrt(mt2):.4f} mean-tan {mt:.4f} | "
              f"one-rate built {math.sqrt(alone.mean()):.4f} | one-rate LM {lm_s} | bound {bt:.4f} (finite {math.sqrt(lo[0]):.4f}) | "
              f"ratio nested/bound {math.sqrt(mt2) / bt:.3f}  one-rate/bound {math.sqrt(alone.mean()) / bt:.3f}  nesting {math.sqrt(mt2 / alone.mean()):.3f}", flush=True)
    lamshare = np.bincount(pick, minlength=len(LAMS)) / len(pick)
    print(f"      scale chosen (share of items per LAMS entry): {np.round(lamshare, 2)}")
    return rows


if __name__ == "__main__":
    allrows = []
    for m in BLOCKS:
        Y = sphere(NV, m, 100 + m)
        print(f"--- block size {m}: its own schedule in group 0 of the 1M build ---")
        allrows += run(m, G0[m], Y, "g0")
        print(f"--- block size {m}: the study's schedules ---")
        for s in STUDY:
            allrows += run(m, s, Y, "-".join(map(str, s)))
    np.save("out_built_rows.npy", np.array([(r[0], r[2], r[3], r[4], r[6], r[7], r[9], r[10]) for r in allrows]))
