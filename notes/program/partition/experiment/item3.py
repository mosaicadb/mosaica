# Item 3: the price of the built grouping. usage: item3.py BASELINE
# BASELINE-rand (random grouping, the baseline's cells-per-group counts), -one (one group), -cell (a basis per cell
# with covariance 0.5 Sigma_c + 0.5 pooled Sigma_g). Cells are the baseline's, so S2 to S5 are the baseline's.
import sys
from lib import *

base = sys.argv[1]
X = load_X()
SCT = scores_T(X)
cell, grp = load_part(base)
K = int(cell.max()) + 1
G = int(grp.max()) + 1
M, cnt = means(X, cell, K)
grand = np.random.default_rng(1).permutation(grp).astype(np.int32)
log("rand grouping: cells per group", np.bincount(grand).tolist(), "fit rows per group",
    [int(cnt[grand == g].sum()) for g in range(G)])
score_partition(f"{base}-rand", X, SCT, cell, grand, M=M, struct=False)
score_partition(f"{base}-one", X, SCT, cell, np.zeros(K, np.int32), M=M, struct=False)
Cc, c2 = scatters(X, cell, K, M, FIT)
gcov, Ng = group_covs(Cc, c2, grp, G)
covs = [0.5 * Cc[c] / c2[c] + 0.5 * gcov[grp[c]] for c in range(K)]
del Cc
score_partition(f"{base}-cell", X, SCT, cell, grp, M=M, bas=np.arange(K), covs=covs, struct=False,
                extra={"cell_fit_rows_min": int(c2.min()), "cell_fit_rows_median": float(np.median(c2))})
log("done")
