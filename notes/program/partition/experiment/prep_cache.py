# Caches shared by every item: exact scores (NQ x N), each query's 10 best valid rows, 2,000 rows' 10 nearest rows.
# Also checks the level model: on fit rows the mean of w_B^2 must equal W_g(B)^2 for the group's own basis.
from lib import *
X = load_X(); log("rows loaded", X.shape)
SCT = scores_T(X); log("scores", SCT.shape)
top10(SCT); log("top10")
nn10(X); log("nn10")
taxa(); log("taxa")
cell = s0["a"].astype(np.int32); grp = s0["grp"].astype(np.int32)
M, cnt = means(X, cell, 200)
Cc, c2 = scatters(X, cell, 200, M, FIT)
covs, Ng = group_covs(Cc, c2, grp, 12)
for g in (0, 5):
    gam, T = basis(covs[g], A_SI)
    rows = np.flatnonzero(FIT & (grp[cell] == g))
    Z = (X[rows].astype(np.float64) - M[cell[rows]]) @ T
    z2 = Z * Z
    log("group", g, "rows", len(rows), "coord var / sqrt(gam) range", (z2.mean(0) / np.sqrt(gam)).min().round(6), (z2.mean(0) / np.sqrt(gam)).max().round(6))
    for B in (0, 4, 16, 32):
        s, W, th = level(gam, B)
        log(f"  B={B}: W={W:.5f} mean w^2={(W * (z2 @ s)).mean():.6f} W^2={W*W:.6f} theta={th:.3e} bits used={0.5*np.maximum(np.log(gam/th),0).sum()/np.log(2) if B else 0:.1f}")
    by = bytes_at(z2[:2000], gam)
    log("  bytes at w* .03/.06/.12 mean", by.mean(0).round(3), "F", np.sqrt(gam).sum().round(4), "gamma1", gam[0], "gamma_n", gam[-1])
log("done")
