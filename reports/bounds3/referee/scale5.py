import numpy as np, math
raw = np.fromfile("dump/dump.bin", dtype=np.float32).reshape(-1, 24)
tag = raw[:, 0].view(np.uint32); row = raw[:, 1].view(np.uint32)
k = (tag & 1) == 1; r, tg, rw = raw[k], tag[k], row[k]; del raw
ds = np.array([95, 127, 255, 287.0])
lv = r[:, 2].astype(int); est = r[:, 5].astype(float); sd = r[:, 7].astype(float); tr = r[:, 4].astype(float); hn = r[:, 6].astype(float)
key = tg.astype(np.int64) << 32 | rw.astype(np.int64); h = 9.330163 * sd + hn
for l in (1, 2, 3):
    m = lv == l; mb = lv == l - 1
    safe = np.isin(key[m], key[mb][np.abs(tr[mb]) < 0.2 * h[mb]]); idx = np.where(m)[0][safe]
    z2 = ((tr[idx] - est[idx]) / sd[idx]) ** 2
    a = r[idx][:, [9, 13, 17, 21]].astype(float); v = a * a / ds; w = v / v.sum(1, keepdims=True)
    q = np.quantile(w[:, 3], np.linspace(0, 1, 9))
    out = [f"{q[i]:.4f}-{q[i+1]:.4f}: {np.mean(z2[(w[:,3]>=q[i])&(w[:,3]<=q[i+1])]):.3f}" for i in range(8)]
    print(f"level {l}: mean z^2 by octile of block 3's share (se about {np.std(z2)/math.sqrt(len(z2)/8):.3f}): " + "  ".join(out))
    # the same items seen through distinct rows only (one record per row and level, the first), to see how many independent items there are
    print(f"   distinct rows {len(np.unique(rw[idx]))} of {len(idx)} records; distinct (query, row) {len(np.unique(key[idx] >> 0))}")
