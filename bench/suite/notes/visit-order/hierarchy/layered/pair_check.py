"""Per true neighbour, its error in units of the level's spread against its height in units of the cluster's, from
`study_layered_dump`: x = (s - mu_c) / sigma_c and y = e / tau at each deterministic level, e = s - a_l, with mu_c,
sigma_c and tau from s rows sampled evenly over the cluster (as kappa_check.py). Printed: the ratio y / x (quantiles),
the slope of y on x through the origin, and the correlation; and the ratio by bins of x.
usage: python3 -I pair_check.py PREFIX PREP_DIR [s, default 64]
"""
import sys

import numpy as np

prefix, prep = sys.argv[1], sys.argv[2]
S = int(sys.argv[3]) if len(sys.argv) > 3 else 64
meta = dict(line.rstrip("\n").split(" ", 1) for line in open(prefix + ".meta"))
nq, M, L, ndl = (int(meta[x]) for x in ("nq", "m", "levels", "ndl"))
idx = np.fromfile(prefix + ".idx", np.uint32).reshape(nq, M, 2)
w = 5 + 2 * L
raw = np.memmap(prefix + ".rows", np.float32, mode="r").reshape(-1, w)
ent = np.memmap(prefix + ".rows", np.uint32, mode="r").reshape(-1, w)[:, 0]
off = np.concatenate([[0], np.cumsum(idx[:, :, 1].astype(np.int64).ravel())])
order = np.load(f"{prep}/order.npy", mmap_mode="r")
xs = {l: [] for l in range(min(ndl, L))}
ys = {l: [] for l in range(min(ndl, L))}
for qi in range(nq):
    top = set(order[qi, :10].tolist())
    for j in range(M):
        if idx[qi, j, 0] == 0xFFFFFFFF or idx[qi, j, 1] < 50:
            continue
        a, b = off[qi * M + j], off[qi * M + j + 1]
        hit = np.isin(np.asarray(ent[a:b]), list(top))
        if not hit.any():
            continue
        r = np.asarray(raw[a:b], np.float64)
        ex = r[:, 1]
        lv = r[:, 5:].reshape(-1, L, 2)
        mid = (lv[:, :, 0] + lv[:, :, 1]) / 2
        sel = np.unique(np.linspace(0, len(ex) - 1, min(S, len(ex))).astype(int))
        mu, sg = ex[sel].mean(), ex[sel].std()
        for l in xs:
            tau = np.std(ex[sel] - mid[sel, l])
            if sg <= 0 or tau <= 0:
                continue
            xs[l].extend(((ex[hit] - mu) / sg).tolist())
            ys[l].extend(((ex[hit] - mid[hit, l]) / tau).tolist())
name = prefix.split("/")[-1]
print(f"## {name} (s {S})")
for l in xs:
    x, y = np.array(xs[l]), np.array(ys[l])
    keep = x > 0.5
    rt = y[keep] / x[keep]
    slope = float(np.sum(x * y) / np.sum(x * x))
    print(f"  level {l}: {len(x)} neighbours; y/x p10/25/50/75/90 {' / '.join(f'{v:.2f}' for v in np.quantile(rt, [0.1, 0.25, 0.5, 0.75, 0.9]))};"
          f" slope {slope:.2f}; correlation {np.corrcoef(x, y)[0, 1]:.2f}")
    bins = [(0.5, 2), (2, 3), (3, 4), (4, 5), (5, 7), (7, 99)]
    print("    median y/x by x: " + "  ".join(f"{lo}-{hi}: {np.median(y[(x >= lo) & (x < hi)] / x[(x >= lo) & (x < hi)]):.2f} (n {np.sum((x >= lo) & (x < hi))})"
                                       for lo, hi in bins if np.sum((x >= lo) & (x < hi)) >= 10))
