# How many rows' sort key the build clamps beyond its allowance (the theory program's referee findings A and K3):
# the build stores key = round(clamp(x . m^, -1, 1) * 32767) with x the row's f16 vector and m^ its cluster's unit
# centre direction (the stored int8 centre times its scale, normalised), and every reader assumes the key is within
# DA = 0.5 / 32767 + 1e-7 of x . m^. A row with |x . m^| > 1 + DA breaks that. Per fit: the rows' largest norm and
# largest |x . m^|, and the count and share of rows beyond 1 + DA (and beyond 1).
# usage: python3 -I key_clamp_count.py FIT_DIR...
import json
import sys

import numpy as np

DA = 0.5 / 32767 + 1e-7
for d in sys.argv[1:]:
    meta = json.load(open(f"{d}/meta.json"))
    n, k = meta["n"], meta["K"]
    xs = np.load(f"{d}/Xs.npy", mmap_mode="r")
    cst = np.load(f"{d}/cstart.npy")
    cen = np.load(f"{d}/mq8.npy").astype(np.float64).reshape(k, n) * np.load(f"{d}/msc.npy").astype(np.float64).reshape(k, 1)
    mhat = cen / np.linalg.norm(cen, axis=1, keepdims=True)
    over, over1, amax, nmax, rows = 0, 0, 0.0, 0.0, 0
    for c in range(k):
        x = np.asarray(xs[cst[c]:cst[c + 1]], np.float64)
        if len(x) == 0:
            continue
        a = x @ mhat[c]
        amax = max(amax, float(np.abs(a).max()))
        nmax = max(nmax, float(np.sqrt((x * x).sum(1)).max()))
        over += int((np.abs(a) > 1 + DA).sum())
        over1 += int((np.abs(a) > 1).sum())
        rows += len(x)
    print(f"{d.rstrip('/').split('/')[-1]}: {rows} rows, n {n}; largest norm {nmax:.9f} (1 + {nmax - 1:.2e}); "
          f"largest |x.m^| {amax:.9f}; rows beyond 1: {over1}; beyond 1 + DA: {over} ({over / rows:.2e})", flush=True)
