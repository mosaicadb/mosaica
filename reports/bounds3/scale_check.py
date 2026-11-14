# How much the stored scale (sd) exceeds the true one: mean z^2 at each level over items that were certain to reach
# it whatever their earlier errors (|truth - t| below a fifth of the previous level's half-width), so that the
# selection of the dump does not bias the errors.
import numpy as np, sys, math
raw = np.fromfile(sys.argv[1], dtype=np.float32).reshape(-1, 24)
tag = raw[:, 0].view(np.uint32); row = raw[:, 1].view(np.uint32)
keep = (tag & 1) == 1; raw, tag, row = raw[keep], tag[keep], row[keep]
lv = raw[:, 2].astype(np.int64); est = raw[:, 5].astype(np.float64); sd = raw[:, 7].astype(np.float64); tr = raw[:, 4].astype(np.float64); hn = raw[:, 6].astype(np.float64)
key = tag.astype(np.int64) << 32 | row.astype(np.int64)
h = (9.330163 * sd + hn)
def at(l):
    m = lv == l; o = np.argsort(key[m]); return key[m][o], ((tr - est) / sd)[m][o], h[m][o], np.abs(tr[m][o])
for l in (1, 2, 3):
    ka, za, ha, ta = at(l); kb, zb, hb, tb = at(l - 1)
    ib = np.isin(kb, ka); ia = np.isin(ka, kb)
    safe = tb[ib] < 0.2 * hb[ib]
    z = za[ia][safe]
    print(f"level {l}: all records mean z^2 {np.mean(za**2):.4f}; unselected subset n {len(z)} mean z^2 {np.mean(z**2):.4f} +- {np.std(z**2)/math.sqrt(len(z)):.4f} -> stored sd / true sd (rms) {1/math.sqrt(np.mean(z**2)):.4f}; kurtosis {np.mean(z**4)/np.mean(z**2)**2:.3f}")
