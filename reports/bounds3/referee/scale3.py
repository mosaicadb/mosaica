import numpy as np, math
raw = np.fromfile("dump/dump.bin", dtype=np.float32).reshape(-1, 24)
tag = raw[:, 0].view(np.uint32); row = raw[:, 1].view(np.uint32)
k = (tag & 1) == 1; r, tg, rw = raw[k], tag[k], row[k]; del raw
ds = np.array([95, 127, 255, 287.0])
lv = r[:, 2].astype(int); est = r[:, 5].astype(float); sd = r[:, 7].astype(float); tr = r[:, 4].astype(float); hn = r[:, 6].astype(float)
key = tg.astype(np.int64) << 32 | rw.astype(np.int64); h = 9.330163 * sd + hn
item_q = ((tg >> 1) & 31)   # si
for l in (1, 2, 3):
    m = lv == l; mb = lv == l - 1
    safe = np.isin(key[m], key[mb][np.abs(tr[mb]) < 0.2 * h[mb]]); idx = np.where(m)[0][safe]
    z2 = ((tr[idx] - est[idx]) / sd[idx]) ** 2
    a = r[idx][:, [9, 13, 17, 21]].astype(float); v = a * a / ds; w = v / v.sum(1, keepdims=True)
    rho, *_ = np.linalg.lstsq(w, z2, rcond=None)
    g = np.random.default_rng(1); bs = []
    # bootstrap by query (errors of one query's items share the query's rotated direction)
    tq = tg[idx]; uq = np.unique(tq); groups = [np.where(tq == t)[0] for t in uq]
    for _ in range(200):
        ii = np.concatenate([groups[j] for j in g.integers(0, len(uq), len(uq))]); bs.append(np.linalg.lstsq(w[ii], z2[ii], rcond=None)[0])
    print(f"level {l}: n {len(idx)}, queries {len(uq)}; z^2 on shares, true/stored variance by block {np.round(rho, 3).tolist()} se (bootstrap over queries) {np.round(np.std(bs, 0), 3).tolist()}")
    mz = [np.mean(z2[grp]) for grp in groups if len(grp) > 2000]
    print(f"   per-query mean z^2 (queries with > 2000 items): n {len(mz)} min {min(mz):.3f} median {np.median(mz):.3f} max {max(mz):.3f}; over-all mean with query-bootstrap se {np.mean(z2):.4f} +- {np.std([np.mean(z2[np.concatenate([groups[j] for j in g.integers(0, len(uq), len(uq))])]) for _ in range(200)]):.4f}")
    for j in range(4):
        edges = [0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0001]
        out = []
        for e0, e1 in zip(edges[:-1], edges[1:]):
            s = (w[:, j] >= e0) & (w[:, j] < e1)
            if s.sum() > 3000: out.append(f"{e0:.1f}-{min(e1,1):.1f}: {np.mean(z2[s]):.3f}+-{np.std(z2[s])/math.sqrt(s.sum()):.3f}")
        print(f"   mean z^2 by share of block {j}: " + "  ".join(out))
