import numpy as np, time, json, sys
rng = np.random.default_rng(0)
t0 = time.time()
def log(*a): print(f"[{time.time()-t0:6.0f}s]", *a, flush=True)
SMALL = len(sys.argv) > 1
X16 = np.concatenate([np.load(f"data/e{i:02d}.npy") for i in ((0, 7) if SMALL else range(20))])
N, d = X16.shape
K = 100 if SMALL else 1000
GS = (64,)
CH = 50_000
def unit(a):
    a = a.astype(np.float32); a /= np.linalg.norm(a, axis=1, keepdims=True); return a
TS = [0.95, 0.9, 0.8, 0.7]; NQ = 100
BUDGETS = [1536, 3072, 4608]
qi = np.sort(rng.choice(N, NQ, replace=False)); Q = unit(X16[qi])
si = np.sort(rng.choice(N, min(N, 200_000), replace=False)); S = unit(X16[si])

def kmeans(S, K, iters=12):
    C = S[rng.choice(len(S), K, replace=False)].copy()
    for it in range(iters):
        a = np.concatenate([(S[i:i+50_000] @ C.T).argmax(1) for i in range(0, len(S), 50_000)])
        M = np.zeros_like(C); np.add.at(M, a, S)
        cnt = np.bincount(a, minlength=K); empty = cnt == 0
        M[empty] = S[rng.choice(len(S), int(empty.sum()))]
        C = M / np.linalg.norm(M, axis=1, keepdims=True)
    return C

STEP = {2: 0.9957, 3: 0.5860, 4: 0.3352, 5: 0.1881, 6: 0.1041, 7: 0.0569, 8: 0.0308}
GT = np.array([1, .3634, .1188, .03744, .01154, .00349, .00104, .0003043, .0000876])
def quant(Z, bits):
    out = np.zeros_like(Z)
    for b in np.unique(bits):
        if b == 0: continue
        cols = np.where(bits == b)[0]; z = Z[:, cols]
        if b == 1: out[:, cols] = np.where(z >= 0, 0.7979, -0.7979)
        else:
            s = STEP[int(b)]; L = 2 ** int(b)
            out[:, cols] = (np.clip(np.floor(z / s), -L // 2, L // 2 - 1) + 0.5) * s
    return out
def alloc(lam, Qe, B, mode):
    b = np.zeros(d, int)
    f = (lambda bb: lam * GT[bb]) if mode == "mse" else (lambda bb: np.sqrt(Qe * lam * GT[bb]))
    val = f(b)
    for _ in range(B):
        nb = np.minimum(b + 1, 8); gain = val - f(nb); gain[b >= 8] = -1
        i = int(gain.argmax()); b[i] += 1; val = f(b)
    return b

Chat = kmeans(S, K)
assign = np.empty(N, np.int32); Msum = np.zeros((K, d)); Bsum = np.zeros((N // 1024 + 1, d))
for i in range(0, N, CH):
    x = unit(X16[i:i+CH]); a = (x @ Chat.T).argmax(1); assign[i:i+CH] = a
    np.add.at(Msum, a, x); np.add.at(Bsum, np.arange(i, i + len(x)) // 1024, x)
cnt = np.bincount(assign, minlength=K)
Mean = (Msum / np.maximum(cnt, 1)[:, None]).astype(np.float32)
Bmean = (Bsum / np.maximum(np.bincount(np.arange(N) // 1024, minlength=len(Bsum)), 1)[:, None]).astype(np.float32)
sa = assign[si]
log("K", K, "residual norm vs k-means mean: median", round(float(np.median(np.linalg.norm(S - Mean[sa], axis=1))), 3),
    "| vs mean of 1024 consecutive rows:", round(float(np.median(np.linalg.norm(S - Bmean[si // 1024], axis=1))), 3))
qm = Q @ Mean.T                                   # NQ x K
# pseudo-queries for the query-energy profile: sample points against their 4 nearest clusters
pq = S[rng.choice(len(S), 5000 if not SMALL else 1000, replace=False)]
pk = np.argsort(-(pq @ Chat.T), axis=1)[:, :4]

results = {}
for G in GS:
    if G == 1: grp = np.zeros(K, int)
    else:
        Mh = Mean / np.linalg.norm(Mean, axis=1, keepdims=True)
        grp = (Mh @ kmeans(Mh, G, 20).T).argmax(1)
    encs = [("mse", ("old", "delta", "delta+w")), ("bnd", ("delta+w+alloc",))] if G == 1 else [("bnd", ("local",))]
    acc = {v: {B: np.zeros((len(TS), 5)) for B in BUDGETS} for _, vs in encs for v in vs}
    errs = {(e, B): [] for e, _ in encs for B in BUDGETS}
    viol = 0
    for g in range(G):
        ks = np.where(grp == g)[0]
        if len(ks) == 0: continue
        kpos = -np.ones(K, int); kpos[ks] = np.arange(len(ks))
        m = grp[sa] == g
        Rs = S[m] - Mean[sa[m]]
        lam, V = np.linalg.eigh((Rs.T @ Rs).astype(np.float64) / max(len(Rs), 1))
        lam = np.maximum(lam[::-1], 1e-12).copy(); V = V[:, ::-1].astype(np.float32).copy(); sig = np.sqrt(lam).astype(np.float32)
        pm = grp[pk] == g
        if pm.sum() > 50:
            rows, cols = np.where(pm)
            Qe = (((pq[rows] - Mean[pk[rows, cols]]) @ V) ** 2).mean(0).astype(np.float64)
        else: Qe = lam.copy()
        QV = Q @ V; MV = Mean[ks] @ V
        Dl = QV[None] - MV[:, None]                 # kg x NQ x d
        dn = np.sqrt((Dl ** 2).sum(2))
        specs = []
        for e, vs in encs:
            for B in BUDGETS:
                bits = alloc(lam, Qe, B, e)
                Dv = ((lam * GT[bits] + 1e-10) / (Qe + 1e-10)) ** 0.25
                Dv = Dv.astype(np.float32)
                specs.append((e, vs, B, bits, Dv, np.sqrt(((Dl * Dv) ** 2).sum(2))))
        if g == 0:
            for e, vs, B, bits, _, _ in specs: log("G", G, e, B, "bits -> dims coded", int((bits > 0).sum()), "max bits", int(bits.max()))
        items = np.where(grp[assign] == g)[0]
        for c in range(0, len(items), CH):
            ii = items[c:c+CH]; x = unit(X16[ii]); a = assign[ii]; ap = kpos[a]
            true = x @ Q.T; base = qm[:, a].T
            z = (x - Mean[a]) @ V; mva = MV[ap]
            for e, vs, B, bits, Dv, dw in specs:
                if B == 0:
                    evec = z; gam = np.zeros(len(z), np.float32); sc = np.zeros_like(true); cn2 = np.ones(len(z), np.float32)
                else:
                    code = quant(z / sig, bits) * sig
                    cn2 = (code * code).sum(1) + 1e-20; gam = (z * code).sum(1) / cn2
                    evec = z - gam[:, None] * code; sc = code @ QV.T
                en = np.linalg.norm(evec, axis=1); ew = np.linalg.norm(evec / Dv, axis=1); me = (mva * evec).sum(1)
                errs[(e, B)].append(en)
                est_old = base + gam[:, None] * sc
                h_old = en[:, None] * np.sqrt(np.maximum(1 - sc * sc / cn2[:, None], 0))
                est_new = est_old + me[:, None]
                hB = dn[ap] * en[:, None]; hC = np.minimum(hB, dw[ap] * ew[:, None])
                for v in vs:
                    est, h = {"old": (est_old, h_old), "delta": (est_new, hB)}.get(v, (est_new, hC))
                    h = h * 1.0001 + 2e-5; ub = est + h; lb = est - h
                    for ti, t in enumerate(TS):
                        A = true >= t; sv = ub >= t; win = lb >= t
                        acc[v][B][ti] += [A.sum(), sv.sum(), (sv & ~win).sum(), win.sum(), h[A].sum()]
                        viol += int((A & ~sv).sum() + (win & ~(true >= t - 2e-5)).sum())
        if g % 8 == 0: log("G", G, "group", g, "violations", viol)
    results[G] = {"N": N, "NQ": NQ, "TS": TS, "violations": viol,
                  "acc": {v: {str(B): a.tolist() for B, a in d2.items()} for v, d2 in acc.items()},
                  "err_median": {f"{e}{B}": float(np.median(np.concatenate(v))) for (e, B), v in errs.items()}}
    json.dump(results, open("results2_small.json" if SMALL else "results3.json", "w"))
    log("G", G, "done, violations", viol)
