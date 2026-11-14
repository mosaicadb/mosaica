import numpy as np, math
raw = np.fromfile("dump/dump.bin", dtype=np.float32).reshape(-1, 24)
tag = raw[:, 0].view(np.uint32); row = raw[:, 1].view(np.uint32)
k = (tag & 1) == 1; r, tg, rw = raw[k], tag[k], row[k]; del raw
ds = np.array([95, 127, 255, 287.0])
lv = r[:, 2].astype(int); est = r[:, 5].astype(float); sd = r[:, 7].astype(float); tr = r[:, 4].astype(float); hn = r[:, 6].astype(float)
key = tg.astype(np.int64) << 32 | rw.astype(np.int64); h = 9.330163 * sd + hn
qi = tg >> 6; si = (tg >> 1) & 31
print("qi values", np.unique(qi).tolist(), "si values", np.unique(si).tolist())
for l in (1, 2, 3):
    m = lv == l; mb = lv == l - 1
    safe = np.isin(key[m], key[mb][np.abs(tr[mb]) < 0.2 * h[mb]]); idx = np.where(m)[0][safe]
    z2 = ((tr[idx] - est[idx]) / sd[idx]) ** 2
    a = r[idx][:, [9, 13, 17, 21]].astype(float); v = a * a / ds; w = v / v.sum(1, keepdims=True)
    # the estimate's block parts: |est_j| / (stored a_j) is at most 1/tan-like; a proxy for the parallel part
    for name, sel in (("qi < 10", qi[idx] < 10), ("qi >= 10", qi[idx] >= 10)):
        rho, *_ = np.linalg.lstsq(w[sel], z2[sel], rcond=None)
        print(f"level {l} {name}: n {sel.sum()} mean z^2 {np.mean(z2[sel]):.4f} +- {np.std(z2[sel])/math.sqrt(sel.sum()):.4f}; true/stored variance by block {np.round(rho, 3).tolist()}")
