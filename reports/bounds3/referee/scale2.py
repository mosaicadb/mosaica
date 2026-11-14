import numpy as np, math, sys
raw = np.fromfile("dump/dump.bin", dtype=np.float32).reshape(-1, 24)
tag = raw[:, 0].view(np.uint32); row = raw[:, 1].view(np.uint32)
print("records", len(raw), "tag bit0 counts", np.bincount(tag & 1), "distinct tags", len(np.unique(tag)))
ds = np.array([95, 127, 255, 287.0])
for bit in (1, 0):
    k = (tag & 1) == bit
    r, tg, rw = raw[k], tag[k], row[k]
    lv = r[:, 2].astype(int); est = r[:, 5].astype(float); sd = r[:, 7].astype(float); tr = r[:, 4].astype(float); hn = r[:, 6].astype(float)
    key = tg.astype(np.int64) << 32 | rw.astype(np.int64)
    h = 9.330163 * sd + hn
    print(f"--- tag bit0 = {bit}: {len(r)} records; levels {np.bincount(lv)}")
    for l in (0, 1, 2, 3):
        m = lv == l
        if l == 0:
            safe = np.ones(m.sum(), bool); idx = np.where(m)[0]
        else:
            mb = lv == l - 1
            kb = key[mb]; ok = np.abs(tr[mb]) < 0.2 * h[mb]
            safe = np.isin(key[m], kb[ok]); idx = np.where(m)[0]
        idx = idx[safe]
        if len(idx) < 1000: continue
        err = tr[idx] - est[idx]; s = sd[idx]; z = err / s
        a = r[idx][:, [9, 13, 17, 21]].astype(float); v = a * a / ds            # stored per-block variances
        coded = (r[idx][:, [8, 12, 16, 20]] > 0)
        chk = np.abs(np.sqrt(v.sum(1)) / s - 1).max()
        # per-block ratio true / stored variance: least squares of err^2 on the stored block variances
        cols = coded.any(0)
        rho, *_ = np.linalg.lstsq(v[:, cols], err ** 2, rcond=None)
        # bootstrap se
        bs = []
        g = np.random.default_rng(0)
        for _ in range(30):
            ii = g.integers(0, len(idx), len(idx)); bs.append(np.linalg.lstsq(v[ii][:, cols], err[ii] ** 2, rcond=None)[0])
        se = np.std(bs, 0)
        # subgroup means of z^2 by query (tag) and by share-dominant block
        tq = tg[idx]; qm = [(np.mean(z[tq == t] ** 2), (tq == t).sum()) for t in np.unique(tq)]
        qworst = max(qm); dom = np.argmax(v, 1)
        bydom = [(int(j), round(float(np.mean(z[dom == j] ** 2)), 4), int((dom == j).sum())) for j in range(4) if (dom == j).sum() > 500]
        # deciles of stored sd
        qs = np.quantile(s, np.linspace(0, 1, 6)); bysd = [round(float(np.mean(z[(s >= qs[i]) & (s <= qs[i + 1])] ** 2)), 4) for i in range(5)]
        # tails against a Gaussian of the measured variance
        from scipy.stats import norm
        tails = [(x, float(np.mean(np.abs(z) > x)), 2 * norm.sf(x)) for x in (2, 3, 4)]
        print(f"level {l}: n {len(idx)} mean z^2 {np.mean(z*z):.4f} +- {np.std(z*z)/math.sqrt(len(z)):.4f}; mean z {np.mean(z):+.4f}; |hn|/sd median {np.median(hn[idx]/s):.3f}; sd consistency {chk:.1e}")
        print(f"   true/stored variance by coded block {np.where(cols)[0].tolist()}: {np.round(rho, 3).tolist()} se {np.round(se, 3).tolist()} -> stored/true scale {np.round(1/np.sqrt(np.maximum(rho,1e-9)), 3).tolist()}")
        print(f"   worst query mean z^2 {qworst[0]:.3f} (n {qworst[1]}); by dominant block {bydom}; by quintile of stored sd {bysd}; max |z| {np.abs(z).max():.2f}")
        print(f"   P[|z| > x] measured vs unit Gaussian: {[(x, f'{p:.2e}', f'{q:.2e}') for x, p, q in tails]}")
