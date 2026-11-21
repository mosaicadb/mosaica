# Per-group held-out widths for a partition: whether a small group's singular basis inflates the means.
import sys
from lib import *
X = load_X()
for nm in sys.argv[1:]:
    cell, grp = load_part(nm); K = int(cell.max()) + 1; G = int(grp.max()) + 1
    M, _ = means(X, cell, K); Cc, c2 = scatters(X, cell, K, M, FIT); covs, Ng = group_covs(Cc, c2, grp, G)
    w0, wB, by = coding(X, cell, M, grp, covs, A_SI)
    gh = grp[cell[np.flatnonzero(HELD)]]
    print(nm)
    for g in range(G):
        m = gh == g
        print(f"  group {g}: fit rows {int(Ng[g])}, cells {(grp==g).sum()}, held rows {m.sum()}, mean S1d16 {wB[m,2].mean() if m.any() else float('nan'):.4f}, S1b.06 {by[m,1].mean() if m.any() else float('nan'):.2f}, max S1d16 {wB[m,2].max() if m.any() else float('nan'):.3f}")
    big = Ng >= FLOOR
    m = big[gh]
    print(f"  groups >= 3n only: held rows {m.sum()}, S1d16 {wB[m,2].mean():.5f}, S1b.06 {by[m,1].mean():.3f}; all: {wB[:,2].mean():.5f} {by[:,1].mean():.3f}")
