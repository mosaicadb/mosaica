# Published single-rate figures reproduced, as a check of the simulation (long-block tan^2 = D / (1 - D)).
#   BlockQuant (arXiv:2605.19972) Corollary 10: Lloyd codebooks of 2^(b p) points for blocks of p coordinates of a
#   rotated vector, rescaled per vector: tan^2 0.571, 0.120, 0.0306 (p = 2, b = 1..3) and 0.553, 0.113 (p = 3, b = 1, 2).
#   Lloyd-Max and the uniform grid are in bounds.py; the trellis and E8 in cand_trellis.py and cand_e8.py.
import math
import numpy as np
from cand_product import lbg

if __name__ == "__main__":
    pub = {(2, 1): 0.571, (2, 2): 0.120, (2, 3): 0.0306, (3, 1): 0.553, (3, 2): 0.113}
    for (p, b), ref in pub.items():
        ncw = 1 << (b * p)
        best = min((lbg(p, ncw=ncw, ntrain=60000, iters=120, seed=31 * p + 7 * b + i) for i in range(3)), key=lambda r: r[1])
        D = best[1]
        print(f"p = {p}, b = {b} ({ncw} points): Lloyd training D {D:.4f}, tan^2 = D/(1-D) = {D / (1 - D):.4f}; published {ref}")
