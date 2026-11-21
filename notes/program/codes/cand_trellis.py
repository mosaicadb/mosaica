# Candidate T: the sliding-window (bitshift) trellis of Mao, Gray and Linder, as QTIP uses it: R new bits per position,
# the reproduction of a position a table over the last L bits. The table is trained by Lloyd iterations with a Viterbi
# encoder (2^(L-R) states), starting from Gaussian entries. The start state is free (L - R bits per block not counted).
# Decode cost in this index: a 2^L-entry table per position; only L = 4 is a 16-entry lookup, and its nibble has to be
# assembled from bits of neighbouring positions: one lookup per position, that is 4 / R times today's lookups per bit.
import math
import sys
import numpy as np
from clib import *


def viterbi(X, tab, R, L):
    N, T = X.shape
    S = 1 << (L - R)
    W = 1 << L
    prev = np.arange(W) >> R
    cost = np.zeros((N, S), dtype=np.float32)
    bps = np.zeros((T, N, S), dtype=np.uint8)
    tab32 = tab.astype(np.float32)
    for t in range(T):
        d = X[:, t, None].astype(np.float32) - tab32[None, :]
        tot = (cost[:, prev] + d * d).reshape(N, 1 << R, S)
        bp = tot.argmin(1)
        bps[t] = bp
        cost = np.take_along_axis(tot, bp[:, None, :], 1)[:, 0, :]
    s = cost.argmin(1)
    ar = np.arange(N)
    w = np.zeros((N, T), dtype=np.int64)
    for t in range(T - 1, -1, -1):
        wt = bps[t][ar, s].astype(np.int64) * S + s
        w[:, t] = wt
        s = wt >> R
    return w


def train(R, L, m=256, ntrain=500, iters=20, seed=0, chunk=None):
    """Lloyd iterations; the blocks are encoded in chunks so that the back-pointers (T x chunk x states bytes) stay
    under about 40 MB."""
    rng = np.random.default_rng(seed)
    S = 1 << (L - R)
    chunk = chunk or max(20, min(ntrain, 40_000_000 // (m * S)))
    X = sphere(ntrain, m, 5000 + seed)
    X = X / np.sqrt((X * X).mean(1, keepdims=True))
    tab = rng.standard_normal(1 << L) * math.sqrt(1 - 4.0 ** -R)
    D = None
    for _ in range(iters):
        cnt = np.zeros(1 << L)
        sm = np.zeros(1 << L)
        err = 0.0
        for i in range(0, ntrain, chunk):
            Xc = X[i: i + chunk]
            w = viterbi(Xc, tab, R, L)
            cnt += np.bincount(w.reshape(-1), minlength=1 << L)
            sm += np.bincount(w.reshape(-1), weights=Xc.reshape(-1), minlength=1 << L)
            err += float(((Xc - tab[w]) ** 2).sum())
        D = err / X.size          # distortion of the table the blocks were encoded with
        ok = cnt > 0
        tab[ok] = sm[ok] / cnt[ok]
    return tab, D


if __name__ == "__main__":
    m = 256
    cases = [(1, L) for L in (1, 4, 6, 8, 10)] + [(2, L) for L in (2, 4, 6, 8, 10)]
    if len(sys.argv) > 1:
        cases = [(int(a.split(",")[0]), int(a.split(",")[1])) for a in sys.argv[1:]]
    Y = sphere(120, m, 2000 + m)
    rms = np.sqrt((Y * Y).mean(1, keepdims=True))
    for R, L in cases:
        ntrain = max(300, (1 << L) * 120 // m)
        tab, D = train(R, L, m, ntrain=ntrain, iters=20 if L <= 8 else 14)
        w = viterbi(Y / rms, tab, R, L)
        t2 = tan2(tab[w], Y)
        bt2, _ = built_encode(Y, [depths(R, m)])
        t, b = math.sqrt(t2.mean()), math.sqrt(bt2.mean())
        lookups = "one 16-entry lookup per position" if L <= 4 else f"a {1 << L}-entry table per position"
        print(f"R={R} L={L:<2} states {1 << (L - R):<5} training D {D:.4f} ({-10 * math.log10(D):.2f} dB); test tan {t:.4f}; / built {t / b:.3f}; "
              f"/ bound {t / math.sqrt(bound_tan2(R)):.3f}; {lookups}", flush=True)
