"""A cluster's spreads from its rows read at a randomised level instead of their stored vectors, from
`study_layered_dump` run at every level (DIAG_LEVELS 99).

At a randomised level where every block is coded, a row's estimate a_R is unbiased for its score s over the build's
rotation, with variance v_R (the block error law; theory program claim A4.4). So, over s rows of a cluster spread
evenly over its rows, with a_l a deterministic level's midpoint:
- the level's mean squared error, mean (s - a_l)^2, is estimated by mean (a_R - a_l)^2 - mean v_R;
- the cluster's mean score by mean a_R, and its score variance by Var(a_R) - mean v_R.
v_R is taken from the level's interval: its half-width is c sd + small rounding terms, c the certified constant
(`csgl`), so v_R = (h_R / c)^2, an upper bound (the rounding terms left in). Per dataset and deterministic level, and
sample size: the ratio of each estimate to the same quantity from the sampled rows' exact scores (median, 10th and 90th
percentiles over (query, cluster)); the level used and its bytes a row; and v_R over the exact squared error (the
extra noise each row carries).
usage: python3 -I rand_sample.py PREFIX
"""
import ast
import sys

import numpy as np

prefix = sys.argv[1]
meta = dict(line.rstrip("\n").split(" ", 1) for line in open(prefix + ".meta"))
nq, M, L, ndl, nl = (int(meta[x]) for x in ("nq", "m", "levels", "ndl", "nl"))
level_bytes = ast.literal_eval(meta["level_bytes"])
csgl = ast.literal_eval(meta["csgl"])  # groups x randomised levels
rates = ast.literal_eval(meta["rates"])  # groups x randomised levels x blocks
idx = np.fromfile(prefix + ".idx", np.uint32).reshape(nq, M, 2)
w = 5 + 2 * L
raw = np.memmap(prefix + ".rows", np.float32, mode="r").reshape(-1, w)
off = np.concatenate([[0], np.cumsum(idx[:, :, 1].astype(np.int64).ravel())])
grp = np.load(prefix + ".grp.npy")


def first_full(g):
    """The first randomised level (its index among them) by which every block of group g is coded."""
    seen = np.zeros(len(rates[g][0]), bool)
    for lr, r in enumerate(rates[g]):
        seen |= np.array(r) > 0
        if seen.all():
            return lr
    return None


full = [first_full(g) for g in range(len(rates))]
sizes = (32, 64, 128, 256, 0)  # 0: every row of the cluster
acc = {(l, s, q): [] for l in range(ndl) for s in sizes for q in ("msq", "sd", "mu", "var", "noise")}
used = []
for qi in range(nq):
    for j in range(M):
        c = idx[qi, j, 0]
        if c == 0xFFFFFFFF or idx[qi, j, 1] < 64:
            continue
        g = int(grp[c])
        lr = full[g]
        if lr is None or ndl + lr >= L:
            continue
        a, b = off[qi * M + j], off[qi * M + j + 1]
        r = np.asarray(raw[a:b], np.float64)
        ex = r[:, 1]
        lv = r[:, 5:].reshape(-1, L, 2)
        mid = (lv[:, :, 0] + lv[:, :, 1]) / 2
        hh = (lv[:, :, 1] - lv[:, :, 0]) / 2
        aR, vR = mid[:, ndl + lr], np.square(hh[:, ndl + lr] / csgl[g][lr])
        used.append(ndl + lr)
        for s in sizes:
            sel = np.arange(len(ex)) if s == 0 else np.unique(np.linspace(0, len(ex) - 1, min(s, len(ex))).astype(int))
            for l in range(ndl):
                e = ex[sel] - mid[sel, l]
                d = aR[sel] - mid[sel, l]
                msq_x, msq_r = np.mean(e * e), np.mean(d * d) - np.mean(vR[sel])
                sd_x = np.std(e)
                sd_r = np.sqrt(max(msq_r - np.mean(d) ** 2, 0.0))
                if msq_x > 0 and sd_x > 0:
                    acc[(l, s, "msq")].append(msq_r / msq_x)
                    acc[(l, s, "sd")].append(sd_r / sd_x)
                    acc[(l, s, "noise")].append(np.mean(vR[sel]) / msq_x)
                if l == 0:
                    sc = np.std(ex[sel])
                    if sc > 0:
                        acc[(l, s, "mu")].append((np.mean(aR[sel]) - np.mean(ex[sel])) / sc)
                        var_r = np.var(aR[sel]) - np.mean(vR[sel])
                        acc[(l, s, "var")].append(var_r / np.var(ex[sel]))

name = prefix.split("/")[-1]
u, n = np.unique(used, return_counts=True)
print(f"## {name}: the first randomised level coding every block: "
      + ", ".join(f"level {x} ({level_bytes[x]} B a row) for {c} (query, cluster) pairs" for x, c in zip(u, n)))
q = lambda a: " / ".join(f"{x:.2f}" for x in np.quantile(a, [0.1, 0.5, 0.9])) if len(a) else "-"
for l in range(ndl):
    print(f"  deterministic level {l} ({level_bytes[l]} B): estimate over exact, p10 / median / p90")
    for s in sizes:
        lab = "all rows" if s == 0 else f"{s} rows"
        print(f"    {lab:<9} mean sq error {q(acc[(l, s, 'msq')])} | its sd {q(acc[(l, s, 'sd')])} | v_R / mean sq error {q(acc[(l, s, 'noise')])}")
print("  the cluster's mean (difference in score sds) and score variance (ratio), from a_R:")
for s in sizes:
    lab = "all rows" if s == 0 else f"{s} rows"
    print(f"    {lab:<9} mean {q(acc[(0, s, 'mu')])} | variance {q(acc[(0, s, 'var')])}")
