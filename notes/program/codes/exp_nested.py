# Nested stream codes for the schedules in use, against the built nested code on the same test blocks.
# A configuration is a list of (p, J, rise index) streams; a rise's code is the sum of the streams of rises up to it.
# usage: python3 exp_nested.py <config name> [...]
import math
import sys
import numpy as np
from clib import *
from streams import Stream, Phased, Nested
from exp_onerate import codebook

# name: (m, rises as rates, streams [(p, J, rise index, initial scale)], emphasis per rise)
CONFIGS = {
    # the block of 256: 1/4, 1, 2 bits
    "q12_a": (256, [0.25, 1, 2], [(16, 1, 0, 1.0), (8, 1, 1, 0.9), (16, 1, 1, 0.8), (4, 2, 2, 0.6)], None),
    "q12_b": (256, [0.25, 1, 2], [(16, 1, 0, 1.0), (8, 2, 1, 0.9), (16, 1, 1, 0.8), (4, 2, 2, 0.6)], None),
    "q12_c": (256, [0.25, 1, 2], [(16, 1, 0, 1.0), (16, 1, 1, 0.9), (16, 1, 1, 0.85), (16, 1, 1, 0.8), (4, 2, 2, 0.6)], None),
    "q12_d": (256, [0.25, 1, 2], [(16, 1, 0, 1.0), (8, 1, 1, 0.9), (16, 1, 1, 0.8), (4, 3, 2, 0.6)], None),
    "q12_e": (256, [0.25, 1, 2], [(16, 1, 0, 1.0), (8, 1, 1, 0.9), (16, 1, 1, 0.8), (4, 2, 2, 0.6)], [1, 2, 4]),
    # 1 and 2 bits only (the same block without its sub-bit level)
    "12_a": (256, [1, 2], [(4, 2, 0, 1.0), (4, 2, 1, 0.6)], None),
    "12_b": (256, [1, 2], [(4, 1, 0, 1.0), (4, 2, 1, 0.6)], None),
    "12_c": (256, [1, 2], [(4, 2, 0, 1.0), (4, 3, 1, 0.6)], None),
    "12_d": (256, [1, 2], [(4, 3, 0, 1.0), (4, 3, 1, 0.6)], None),
    "12_e": (256, [1, 2], [(4, 2, 0, 1.0), (4, 2, 1, 0.6)], [1, 4]),
    # staggered: the second stream's nibbles straddle the first's
    "12_s1": (256, [1, 2], [(4, 1, 0, 1.0, 0), (4, 1, 1, 0.6, 2)], None),
    "12_s2": (256, [1, 2], [(4, 2, 0, 1.0, 0), (4, 2, 1, 0.6, 2)], None),
    "12_s3": (256, [1, 2], [(4, 1, 0, 1.0, 0), (4, 1, 1, 0.6, 2)], [1, 4]),
    "12_s4": (256, [1, 2], [(4, 1, 0, 1.0, 0), (4, 1, 1, 0.6, 2)], [1, 16]),
    "12_s5": (256, [1, 2], [(4, 2, 0, 1.0, 0), (4, 3, 1, 0.6, 2)], None),
    # phased trellis: nibbles of rise 1 and rise 2 alternate along the block, 2 positions per step
    "12_p0": (256, [1, 2], [("ph", 2, [0, 1], [1.0, 0.5], [1, 0])], [0.001, 1]),
    "12_p1": (256, [1, 2], [("ph", 2, [0, 1], [1.0, 0.5], [1, 0])], None),
    "12_p2": (256, [1, 2], [("ph", 2, [0, 1], [1.0, 0.5], [1, 0])], [1, 3]),
    "12_p3": (256, [1, 2], [("ph", 2, [0, 1], [1.0, 0.5], [1, 0])], [3, 1]),
    "12_p4": (256, [1, 2], [("ph", 2, [0, 1], [1.0, 0.5], [1, 0])], [1, 10]),
    # 256 states
    "12_q1": (256, [1, 2], [("ph", 2, [0, 1], [1.0, 0.5], [1, 0], 3)], None),
    "12_q2": (256, [1, 2], [("ph", 2, [0, 1], [1.0, 0.5], [1, 0], 3)], [1, 3]),
    "12_q3": (256, [1, 2], [("ph", 2, [0, 1], [1.0, 0.5], [1, 0], 3)], [1.5, 1]),
    # the block of 256 as built, 1/4-1-2: the 1/4-bit rise reads the 1-bit nibbles of the first quarter of the block
    "q12_p1": (256, [0.25, 1, 2], [("ph", 2, [1, 2], [1.0, 0.5], [1, 0], 2, (256, 1))], None),
    "q12_p2": (256, [0.25, 1, 2], [("ph", 2, [1, 2], [1.0, 0.5], [1, 0], 2, (256, 1))], [0.5, 1, 1.5]),
    "q12_q1": (256, [0.25, 1, 2], [("ph", 2, [1, 2], [1.0, 0.5], [1, 0], 3, (256, 1))], None),
    # the block of 256 with the sub-bit level kept beside a phased trellis: rates 1/4, 5/4, 9/4
    "q12_x": (256, [0.25, 1.25, 2.25], [(16, 1, 0, 1.0), ("ph", 2, [1, 2], [0.9, 0.45], [1, 0])], None),
    "q12_y": (256, [0.25, 1.25, 2.25], [(16, 1, 0, 1.0), ("ph", 2, [1, 2], [0.9, 0.45], [1, 0])], [1, 1, 2]),
    # the block of 128: 1, 3, 4 bits
    "134_a": (128, [1, 3, 4], [(4, 2, 0, 1.0), (2, 2, 1, 0.6), (4, 2, 2, 0.2)], None),
    "134_b": (128, [1, 3, 4], [(4, 2, 0, 1.0), (4, 2, 1, 0.6), (4, 2, 1, 0.35), (4, 2, 2, 0.2)], None),
    # the block of 96: 2, 3, 4, 7 bits
    "2347_a": (96, [2, 3, 4, 7], [(2, 2, 0, 1.0), (4, 2, 1, 0.33), (4, 2, 2, 0.2), (2, 2, 3, 0.1), (4, 2, 3, 0.03)], None),
    "234_a": (96, [2, 3, 4], [(2, 2, 0, 1.0), (4, 2, 1, 0.33), (4, 2, 2, 0.2)], None),
    "23_a": (96, [2, 3], [(2, 2, 0, 1.0), (4, 2, 1, 0.33)], None),
    "23_b": (96, [2, 3], [(2, 3, 0, 1.0), (4, 3, 1, 0.33)], None),
    "24_a": (128, [2, 4], [(2, 2, 0, 1.0), (2, 2, 1, 0.33)], None),
    # one rate, more bits per position, as references
    "4_a": (128, [4], [(1, 2, 0, 1.0)], None),
    "4_b": (128, [4], [(1, 1, 0, 1.0)], None),
    "3_a": (128, [3], [(2, 2, 0, 1.0), (4, 2, 0, 0.33)], None),
}


def rdepths(r, m):
    """Depths of the built code at block rate r; above 1 bit a fractional rate gives the first share of the positions
    one bit more (the built menu has no such rate; this is its nearest equivalent)."""
    if r < 1 or r == int(r):
        return depths(r, m)
    d = np.full(m, int(r), dtype=np.int64)
    d[: int(round((r - int(r)) * m))] += 1
    return d


def make(d, seed):
    """(p, J, rise, initial scale[, offset]) -> Stream; ("ph", p, rises by phase, scales by phase, wide flags) -> Phased.
    A wide phase starts from a Lloyd codebook over its own step and the next (2p positions); a narrow one from a Lloyd
    codebook over its own step."""
    if d[0] != "ph":
        return Stream(d[0], d[1], d[2], seed=seed, init=codebook(d[0]) * d[3], off=(d[4] if len(d) > 4 else 0))
    _, p, rho, scales, wide = d[:5]
    J = d[5] if len(d) > 5 else 2
    quarter = d[6] if len(d) > 6 else None   # (m, rise index): the phases of that rise are read at rise - 1 over the first quarter of the block
    rho_steps = None
    if quarter is not None:
        m, rq = quarter
        T = m // p
        rho_steps = [rho[t % len(rho)] - (1 if (rho[t % len(rho)] == rq and t < T // 4) else 0) for t in range(T)]
    ph = Phased(p, rho, J=J, seed=seed, rho_steps=rho_steps)
    for f in range(len(rho)):
        if wide[f]:
            C = codebook(2 * p) * scales[f]
            ph.G[0, f, :16], ph.G[1, f, :16] = C[:, :p], C[:, p:]
        else:
            ph.G[0, f, :16] = codebook(p) * scales[f]
    return ph


def run(name, ntest=500, iters=14, seed=0, scales=(1.0,), log=print):
    m, rises, sdef, emph = CONFIGS[name]
    pmax = max((d[1] * len(d[2]) if d[0] == "ph" else d[0]) for d in sdef)
    ntrain = max(300, 50000 * pmax // m)
    Xtr = sphere(ntrain, m, 3000 + seed)
    Xtr = Xtr / np.sqrt((Xtr * Xtr).mean(1, keepdims=True))
    streams = [make(d, seed + i) for i, d in enumerate(sdef)]
    nest = Nested(streams, len(rises), emph)
    nest.train(Xtr, iters=iters, rounds=2)
    Yte = sphere(ntest, m, 2000 + m)
    t2 = nest.tan2(Yte, rounds=4, scales=scales)
    bt2, _ = built_encode(Yte, [rdepths(r, m) for r in rises])
    log(f"--- {name}: m = {m}, rises {rises}, streams {sdef}, emphasis {emph}, "
        f"{ntrain} training blocks, scales {scales} ---")
    out = []
    for k, r in enumerate(rises):
        reff = bits_of(r, m) / m if r < 1 else r
        t, b, bd = math.sqrt(t2[:, k].mean()), math.sqrt(bt2[:, k].mean()), math.sqrt(bound_tan2(reff))
        log(f"  rise {r}: stream code tan {t:.4f}; built nested {b:.4f}; ratio {t / b:.3f}; stream / bound {t / bd:.3f}; built / bound {b / bd:.3f}")
        out.append((r, t, b, bd))
    return nest, out


if __name__ == "__main__":
    for name in sys.argv[1:]:
        run(name)
        sys.stdout.flush()
