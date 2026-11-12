import numpy as np, time, json, sys
rng = np.random.default_rng(0)
t0 = time.time()
def log(*a): print(f"[{time.time()-t0:6.0f}s]", *a, flush=True)

X16 = np.concatenate([np.load(f"data/e{i:02d}.npy") for i in range(20)])
N, d = X16.shape
CH = 50_000
def chunk(i):
    a = X16[i:i+CH].astype(np.float32)
    a /= np.linalg.norm(a, axis=1, keepdims=True)
    return a
log("loaded", X16.shape)

TS = np.array([0.95, 0.9, 0.8, 0.7, 0.6, 0.5], dtype=np.float32)
DELTAS = [0.005, 0.01, 0.02, 0.05, 0.1, 0.2]
NQ = 200
qi = np.sort(rng.choice(N, NQ, replace=False))
Q = X16[qi].astype(np.float32); Q /= np.linalg.norm(Q, axis=1, keepdims=True)

# sample for k-means / PCA
si = np.sort(rng.choice(N, 200_000, replace=False))
S = X16[si].astype(np.float32); S /= np.linalg.norm(S, axis=1, keepdims=True)
gmean = S.mean(0)
log("global mean norm", float(np.linalg.norm(gmean)))

def kmeans(S, K, iters=12):
    C = S[rng.choice(len(S), K, replace=False)].copy()
    for it in range(iters):
        a = np.empty(len(S), np.int64)
        for i in range(0, len(S), 50_000):
            a[i:i+50_000] = (S[i:i+50_000] @ C.T).argmax(1)
        M = np.zeros_like(C); np.add.at(M, a, S)
        cnt = np.bincount(a, minlength=K)
        empty = cnt == 0
        M[empty] = S[rng.choice(len(S), int(empty.sum()))]
        C = M / np.linalg.norm(M, axis=1, keepdims=True)
    return C

# optimal uniform quantiser steps for unit Gaussian (Max 1960); b=1 is Lloyd-Max
STEP = {2: 0.9957, 3: 0.5860, 4: 0.3352, 5: 0.1881, 6: 0.1041, 7: 0.0569, 8: 0.0308}
def quant(Z, bits):
    """Z ~ unit-variance coords; bits: scalar or per-column int array. Returns reconstruction."""
    if np.isscalar(bits):
        bits = np.full(Z.shape[1], bits)
    out = np.zeros_like(Z)
    for b in np.unique(bits):
        cols = np.where(bits == b)[0]
        if b == 0: continue
        z = Z[:, cols]
        if b == 1:
            out[:, cols] = np.where(z >= 0, 0.7979, -0.7979)
        else:
            s = STEP[int(b)]; L = 2 ** int(b)
            k = np.clip(np.floor(z / s), -L // 2, L // 2 - 1)
            out[:, cols] = (k + 0.5) * s
    return out

def alloc(lam, B, cap=8):
    lo, hi = 1e-12, lam.max()
    for _ in range(80):
        D = np.sqrt(lo * hi)
        b = np.clip(0.5 * np.log2(np.maximum(lam, 1e-30) / D), 0, cap)
        if b.sum() > B: lo = D
        else: hi = D
    fl = np.floor(b).astype(int); rem = int(B - fl.sum())
    order = np.argsort(-(b - fl))
    fl[order[:max(rem, 0)]] += 1
    return np.minimum(fl, cap)

Rot, _ = np.linalg.qr(rng.standard_normal((d, d)).astype(np.float32))

results = {}
for K in [1, 1000, 4000]:
    log("=== K", K)
    if K == 1:
        Chat = (gmean / np.linalg.norm(gmean))[None]
    else:
        Chat = kmeans(S, K)
    # pass 1: assign, bucket means, per-item cos to centroid
    assign = np.empty(N, np.int32); costh = np.empty(N, np.float32)
    Msum = np.zeros((K, d), np.float64)
    for i in range(0, N, CH):
        x = chunk(i); s = x @ Chat.T
        a = s.argmax(1); assign[i:i+CH] = a; costh[i:i+CH] = s[np.arange(len(a)), a]
        np.add.at(Msum, a, x)
    cnt = np.bincount(assign, minlength=K)
    Mean = (Msum / np.maximum(cnt, 1)[:, None]).astype(np.float32)   # non-unit bucket mean
    th = np.arccos(np.clip(costh, -1, 1))
    order = np.argsort(assign, kind="stable")
    th_max = np.zeros(K); th_95 = np.zeros(K)
    starts = np.concatenate([[0], np.cumsum(cnt)])
    for k in range(K):
        v = th[order[starts[k]:starts[k+1]]]
        if len(v): th_max[k] = v.max(); th_95[k] = np.quantile(v, 0.95)
    log("bucket sizes: median", int(np.median(cnt)), "max", int(cnt.max()), "empty", int((cnt == 0).sum()))
    log("bucket angular radius deg: median θ95", round(float(np.degrees(np.median(th_95))), 1),
        "median θmax", round(float(np.degrees(np.median(th_max))), 1),
        "item median θ", round(float(np.degrees(np.median(th))), 1))
    # residual stats + PCA on sample
    Rs = S - Mean[assign[si]]
    rn = np.linalg.norm(Rs, axis=1)
    log("residual norm: median", round(float(np.median(rn)), 3), "p95", round(float(np.quantile(rn, .95)), 3))
    lam, V = np.linalg.eigh((Rs.T @ Rs).astype(np.float64) / len(Rs))
    lam = lam[::-1].copy(); V = V[:, ::-1].astype(np.float32).copy()
    cum = np.cumsum(lam) / lam.sum()
    log("residual PCA dims for 50/80/90/95/99% energy:", [int(np.searchsorted(cum, f) + 1) for f in (.5, .8, .9, .95, .99)])
    sig = np.sqrt(np.maximum(lam, 1e-20)).astype(np.float32)
    configs = [("rand", b, b * d) for b in (1, 2, 4, 8)] + [("pca", None, B) for B in (384, 768, 1536, 3072)]
    allocs = {B: alloc(lam, B) for (_, _, B) in configs}
    for B in (384, 768, 1536, 3072):
        log("pca alloc", B, "bits -> dims with bits:", int((allocs[B] > 0).sum()), "max", int(allocs[B].max()))
    Qrot = Q @ Rot; Qpca = Q @ V
    qc = Q @ Chat.T                      # NQ x K cos to unit centroid
    qm = Q @ Mean.T                      # NQ x K dot with bucket mean
    alpha = np.arccos(np.clip(qc, -1, 1))
    acc = {name: np.zeros((len(TS), 4)) for name in
           ["truth", "bucket95", "bucketmax", "ring"] + [f"{m}{B}" for (m, _, B) in configs]}
    slack = np.zeros((len(TS), len(DELTAS)))
    errn = {f"{m}{B}": [] for (m, _, B) in configs}
    viol = 0
    for i in range(0, N, CH):
        x = chunk(i); a = assign[i:i+CH]; n = len(x)
        true = x @ Q.T                    # n x NQ
        al = alpha[:, a].T                # n x NQ
        thx = th[i:i+CH][:, None]
        ub_ring = np.cos(np.maximum(0, al - thx)); lb_ring = np.cos(np.minimum(np.pi, al + thx))
        ub_b95 = np.cos(np.maximum(0, al - th_95[a][:, None]))
        spill = (thx > th_95[a][:, None])
        ub_bmax = np.cos(np.maximum(0, al - th_max[a][:, None]))
        lb_bmax = np.cos(np.minimum(np.pi, al + th_max[a][:, None]))
        r = x - Mean[a]
        base = qm[:, a].T
        for ti, t in enumerate(TS):
            A = true >= t
            acc["truth"][ti] += [A.sum(), 0, 0, 0]
            for di, dl in enumerate(DELTAS): slack[ti, di] += (true >= t - dl).sum()
            # bucket (θ95) survivors + spilled items passing their own ring check
            sv = np.where(spill, ub_ring >= t, ub_b95 >= t)
            acc["bucket95"][ti] += [A.sum(), sv.sum(), 0, 0]; viol += (A & ~sv).sum()
            sv = ub_bmax >= t; win = lb_bmax >= t
            acc["bucketmax"][ti] += [A.sum(), sv.sum(), (sv & ~win).sum(), win.sum()]; viol += (A & ~sv).sum() + (win & ~A).sum()
            sv = ub_ring >= t; win = lb_ring >= t
            acc["ring"][ti] += [A.sum(), sv.sum(), (sv & ~win).sum(), win.sum()]; viol += (A & ~sv).sum()
        for (m, b, B) in configs:
            name = f"{m}{B}"
            if m == "rand":
                z = r @ Rot; nr = np.linalg.norm(z, axis=1, keepdims=True) + 1e-12
                code = quant(z / nr * np.sqrt(d), b); qq = Qrot
            else:
                z = r @ V
                code = quant(z / sig, allocs[B]) * sig; qq = Qpca
            cn2 = (code * code).sum(1, keepdims=True) + 1e-20
            gam = (z * code).sum(1, keepdims=True) / cn2
            e = np.sqrt(np.maximum((z * z).sum(1, keepdims=True) - gam * gam * cn2, 0))
            errn[name].append(e[:, 0])
            sc = code @ qq.T               # n x NQ
            g2 = np.minimum(sc * sc / cn2, 1.0)
            est = base + gam * sc
            h = e * np.sqrt(1 - g2) * 1.0001 + 1e-5
            ub = est + h; lb = est - h
            for ti, t in enumerate(TS):
                A = true >= t; sv = ub >= t; win = lb >= t
                acc[name][ti] += [A.sum(), sv.sum(), (sv & ~win).sum(), win.sum()]
                viol += (A & ~sv).sum() + (win & ~(true >= t - 1e-5)).sum()
        log("chunk", i // CH, "violations so far", int(viol))
    out = {"K": K, "N": N, "NQ": NQ, "TS": TS.tolist(), "DELTAS": DELTAS, "violations": int(viol),
           "slack": slack.tolist(), "acc": {k: v.tolist() for k, v in acc.items()},
           "err_median": {k: float(np.median(np.concatenate(v))) for k, v in errn.items()},
           "err_p95": {k: float(np.quantile(np.concatenate(v), .95)) for k, v in errn.items()}}
    results[K] = out
    json.dump(results, open("results.json", "w"))
    print(json.dumps(out["err_median"]), flush=True)
log("done")
