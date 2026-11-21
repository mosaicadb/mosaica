# Candidate P: one nibble = one 16-word codebook over k positions of its own (a product code; rate 4/k bits per
# position). Decode is unchanged: one 16-entry lookup per nibble; the table's 16 entries are <w, c_i> over k positions.
#   codebooks: the built one (signs of 4 of the k positions), Hadamard rows, the simplex, the biorthogonal set,
#   and a 16-point Lloyd (LBG) quantiser of N(0, I_k).
# Each is measured on whole blocks (tan of the block's angle, one gain per block, as kappa gives).
import math
import sys
import numpy as np
from clib import *
from bounds import finite_bounds

NV = 2000


def hadamard(n):
    H = np.array([[1.0]])
    while H.shape[0] < n:
        H = np.block([[H, H], [H, -H]])
    return H


def lbg(k, ncw=16, ntrain=40000, iters=80, seed=0, init=None):
    """Lloyd's algorithm for N(0, I_k) with ncw points; returns the codebook with the best training distortion."""
    rng = np.random.default_rng(seed)
    X = rng.standard_normal((ntrain, k))
    C = X[rng.choice(ntrain, ncw, replace=False)].copy() if init is None else init.copy()
    for _ in range(iters):
        d = (C * C).sum(1)[None, :] - 2.0 * X @ C.T
        a = d.argmin(1)
        for i in range(ncw):
            s = a == i
            if s.any():
                C[i] = X[s].mean(0)
            else:
                C[i] = X[rng.integers(ntrain)]
    d = (C * C).sum(1)[None, :] - 2.0 * X @ C.T
    D = float(((X * X).sum(1) + d.min(1)).mean() / k)
    return C, D


def encode_product(Y, C, scales=(1.0,)):
    """Rows of Y in sub-blocks of k = C.shape[1]; nearest codeword per sub-block at the item's rms scale times each of
    `scales`; the scale with the smallest tan^2 is kept per item. Positions beyond a whole number of sub-blocks stay
    uncoded (zero)."""
    nv, m = Y.shape
    k = C.shape[1]
    nb = m // k
    rms = np.sqrt((Y * Y).mean(1, keepdims=True))
    best = np.full(nv, np.inf)
    cn = (C * C).sum(1)
    for s in scales:
        V = (Y / (s * rms))[:, : nb * k].reshape(nv * nb, k)
        a = (cn[None, :] - 2.0 * V @ C.T).argmin(1)
        code = np.zeros_like(Y)
        code[:, : nb * k] = C[a].reshape(nv, nb * k)
        best = np.minimum(best, tan2(code, Y))
    return best


def built_sub(k):
    """The built code at rate 4/k as a 16-word codebook over k positions: signs of the first 4. For k < 4 the built
    code is not a product of nibbles; its baseline is measured by built_mixed below."""
    if k < 4:
        return None
    C = np.zeros((16, k))
    for i in range(16):
        for b in range(4):
            C[i, b] = 1.0 if i >> b & 1 else -1.0
    return C


def built_mixed(Y, k):
    """The built code at 4/k bits per position for k = 2 (every position at 2 bits) and k = 3 (one position in three
    at 2 bits, the others at 1)."""
    m = Y.shape[1]
    d = np.ones(m, dtype=np.int64)
    d[:: (1 if k == 2 else 3)] = 2
    t2, _ = built_encode(Y, [d])
    return t2[:, 0]


def codebooks(k):
    out = {"built (4 signs)": built_sub(k)}
    if k >= 16 and k % 16 == 0:
        H = hadamard(16)
        out["Hadamard-16 rows"] = np.repeat(H, k // 16, axis=1)
        S = H[:, 1:]  # the simplex: 16 points in 15 dimensions
        Sk = np.zeros((16, k))
        Sk[:, :15] = S
        out["simplex (15 of k)"] = Sk
        if k == 16:
            pass
    if k >= 8 and k % 8 == 0:
        H8 = np.repeat(hadamard(8), k // 8, axis=1)
        out["biorthogonal (+-8 Hadamard)"] = np.concatenate([H8, -H8])
    return out


if __name__ == "__main__":
    ks = [int(a) for a in sys.argv[1:]] or [32, 24, 16, 12, 8, 6, 5, 4, 3, 2]
    saved = {}
    for k in ks:
        R = 4.0 / k
        cbs = codebooks(k)
        best = None
        inits = [None, None, None]
        if "simplex (15 of k)" in cbs:
            inits.append(cbs["simplex (15 of k)"] * 0.45)
        if "biorthogonal (+-8 Hadamard)" in cbs:
            inits.append(cbs["biorthogonal (+-8 Hadamard)"] * 0.6)
        for si, init in enumerate(inits):
            C, D = lbg(k, seed=10 * k + si, init=init)
            if best is None or D < best[1]:
                best = (C, D)
        cbs["Lloyd 16-point"] = best[0]
        saved[k] = best[0]
        print(f"--- k = {k} positions per nibble, R = {R:.4f} bits per position; Lloyd training D per position {best[1]:.4f} "
              f"(tan2 = D/(1-D) = {best[1] / (1 - best[1]):.4f}); asymptotic bound tan {math.sqrt(bound_tan2(R)):.4f} ---")
        for m in BLOCKS:
            if m % k:
                continue
            Y = sphere(NV, m, 200 + m)
            lo, rc, _ = finite_bounds(m, R)
            row = f"  m={m:<3} finite bound {math.sqrt(lo[0]):.4f} |"
            ref = None
            for name, C in cbs.items():
                sc = (1.0,) if name != "Lloyd 16-point" else (0.85, 1.0, 1.15)
                t2 = built_mixed(Y, k) if C is None else encode_product(Y, C, sc)
                rt = math.sqrt(t2.mean())
                if ref is None:
                    ref = rt
                row += f" {name}: {rt:.4f} ({rt / ref:.3f})"
            print(row, flush=True)
    np.savez(f"out_product_codebooks_{ks[0]}.npz", **{f"k{k}": c for k, c in saved.items()})
