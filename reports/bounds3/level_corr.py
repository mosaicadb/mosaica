# Dependence of one item's errors across randomised levels, from the dump: z_l = (truth - est_l) / sd_l at levels 1..3
# (all four blocks coded), for items that have records at both levels.
import numpy as np, sys, math
raw = np.fromfile(sys.argv[1], dtype=np.float32).reshape(-1, 24)
tag = raw[:, 0].view(np.uint32); row = raw[:, 1].view(np.uint32)
keep = (tag & 1) == 1; raw, tag, row = raw[keep], tag[keep], row[keep]
lv = raw[:, 2].astype(np.int64); est = raw[:, 5].astype(np.float64); sd = raw[:, 7].astype(np.float64); tr = raw[:, 4].astype(np.float64)
key = tag.astype(np.int64) << 32 | row.astype(np.int64)
z = (tr - est) / sd
def at(l):
    m = lv == l
    o = np.argsort(key[m]); return key[m][o], z[m][o], sd[m][o]
K = {l: at(l) for l in (1, 2, 3)}
for l, m in ((1, 2), (2, 3), (1, 3)):
    ka, za, sa = K[l]; kb, zb, sb = K[m]
    ia = np.isin(ka, kb); ib = np.isin(kb, ka)
    x, y = za[ia], zb[ib]; r = sb[ib] / sa[ia]
    print(f"levels {l},{m}: pairs {len(x)}; corr(z_l, z_m) = {np.corrcoef(x, y)[0,1]:.3f}; mean sd_m/sd_l = {r.mean():.3f} (median {np.median(r):.3f}); "
          f"regression of z_m on z_l: slope {np.polyfit(x, y, 1)[0]:.3f}; of z_l on z_m: {np.polyfit(y, x, 1)[0]:.3f}")
# the scale: z has variance below one if the stored scales are rounded up
for l in (1, 2, 3):
    print(f"level {l}: mean z^2 = {np.mean(K[l][1]**2):.4f} -> stored sd / true sd (rms) = {1/math.sqrt(np.mean(K[l][1]**2)):.4f}")
