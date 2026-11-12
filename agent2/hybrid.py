# Owner's metric for the hybrid stream: deterministic levels 0-16 B, then the split-certificate levels 32/48/96/192/384 B, then the full vector.
# Upper bound on the cost: the split code is not an extension of the deterministic one, so the whole 32 B is charged to every item
# undecided after 16 deterministic bytes, and later counts use the split certificate alone (no intersection with earlier levels).
import numpy as np, sys
from harness import LEVELS, FULL, SCAL, SC
d = sys.argv[1] if len(sys.argv) > 1 else "."
rb = np.load(f"{d}/res2_base.npz" if d == "." else f"{d}/res_base.npz"); rs = np.load(f"{d}/res3b.npz"); cls = rs["cls"]; nq = len(cls)
Ud = rb["U"][:, :, 0, :nq]; Us = rs["U_split-mix-opt"]; N = int(rb["N"]); BUD = [int(b * SC) for b in (32, 48, 96, 192, 384)]
def det_chain(sd):                       # sd: scalar bytes charged per level read
    c = (SCAL + sd) * N + np.zeros_like(Ud[0])
    for li in range(1, len(LEVELS)): c += (LEVELS[li] - LEVELS[li - 1] + sd) * Ud[li - 1]
    return c + FULL * Ud[-1]
def hybrid(sd, sb, nblk=(6, 7, 8, 8, 8)):  # sb: scalar bytes per block per split level read
    c = (SCAL + sd) * N + np.zeros_like(Ud[0])
    for li in range(1, 4): c += (LEVELS[li] - LEVELS[li - 1] + sd) * Ud[li - 1]
    c += (BUD[0] + sb * nblk[0]) * Ud[3]
    prev = np.minimum(Ud[3], Us[0])
    for li in range(1, len(BUD)): c += (BUD[li] - BUD[li - 1] + sb * nblk[li]) * prev; prev = Us[li]
    return c + FULL * prev
def split_only(sb, nblk=(6, 7, 8, 8, 8)):
    c = (SCAL + BUD[0] + sb * nblk[0]) * N + np.zeros_like(Us[0])
    for li in range(1, len(BUD)): c += (BUD[li] - BUD[li - 1] + sb * nblk[li]) * Us[li - 1]
    return c + FULL * Us[-1]
for nm in ("item", "text"):
    m = cls == nm; f = lambda c: " ".join(f"{100 * x:6.2f}" for x in (c / (N * FULL))[:, m].mean(1))
    print(f"{nm}: read % of dataset at answer sizes 0.01% / 0.1% / 1% / 5%")
    print("  deterministic chain, scalars as in FINDINGS (4 B once):      ", f(det_chain(0)))
    print("  hybrid det<=16B then split, same convention:                 ", f(hybrid(0, 0)))
    print("  best of the two per query (same convention):                 ", f(np.minimum(det_chain(0), hybrid(0, 0))))
    print("  deterministic chain, 4 B of scalars per level read:          ", f(det_chain(4)))
    print("  hybrid, 4 B per det level and 2 B per block per split level: ", f(hybrid(4, 2)))
    print("  hybrid, 4 B per det level and 1 B per block per split level: ", f(hybrid(4, 1)))
    print("  split only from 32 B, 0 B scalars:                           ", f(split_only(0)))
