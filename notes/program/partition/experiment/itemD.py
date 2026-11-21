# Item 6: balanced k-means. usage: itemD.py BASELINE
# From the baseline's centres, 12 passes on fit rows: rows in decreasing order of (best - second best inner product
# with the unit centre directions), each to its best centre holding fewer than 1.1 * Nfit / K rows; centres to the
# means. Held-out rows are then placed by the same greedy rule with cap 1.1 * Nheld / K; the 100 item-query rows go to
# their nearest centre. Groups by the build's rule (seed 1).
import sys
from lib import *

base = sys.argv[1]
X = load_X()
SCT = scores_T(X)
cell0, _ = load_part(base)
K = int(cell0.max()) + 1
M, _ = means(X, cell0, K)


def greedy(rows, M, cap):
    P = np.concatenate([X[rows[i:i + 20_000]] @ unit(M).astype(np.float32).T for i in range(0, len(rows), 20_000)])
    order_c = np.argsort(-P, axis=1)
    top2 = np.take_along_axis(P, order_c[:, :2], 1)
    margin = top2[:, 0] - top2[:, 1]
    load = np.zeros(K, np.int64)
    out = np.empty(len(rows), np.int32)
    first_choice = 0
    for i in np.argsort(-margin, kind="stable"):
        for r, c in enumerate(order_c[i]):
            if load[c] < cap:
                out[i] = c
                load[c] += 1
                first_choice += r == 0
                break
    return out, first_choice / len(rows)


fit_rows = np.flatnonzero(FIT)
held_rows = np.flatnonzero(HELD)
cell = cell0.copy()
hist = []
for p in range(1, 13):
    nf, fc = greedy(fit_rows, M, 1.1 * len(fit_rows) / K)
    moved = float((nf != cell[fit_rows]).mean())
    cell[fit_rows] = nf
    M, cnt = means(X, cell, K)
    hist.append({"pass": p, "moved_fit": moved, "share_at_first_choice": fc, "fit_min": int(cnt.min()), "fit_max": int(cnt.max())})
    log("pass", p, hist[-1])
nh, fch = greedy(held_rows, M, 1.1 * len(held_rows) / K)
cell[held_rows] = nh
rest = np.flatnonzero(~VALID)
cell[rest] = argmax_ip(X, unit(M), rest)
M, _ = means(X, cell, K)
grp = build_groups(M, 12, np.random.default_rng(1))
score_partition("D", X, SCT, cell, grp, M=M, extra={"D_hist": hist, "held_share_at_first_choice": fch})
log("done")
