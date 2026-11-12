import numpy as np, time, json, sys
rng = np.random.default_rng(0)
t0 = time.time()
def log(*a): print(f"[{time.time()-t0:6.0f}s]", *a, flush=True)
SMALL = len(sys.argv) > 1
X16 = np.concatenate([np.load(f"data/e{i:02d}.npy") for i in ((0, 7) if SMALL else range(20))])
N, d = X16.shape
K = 100 if SMALL else 1000
GS = (1, 4) if SMALL else (1, 64)
CH = 50_000
def unit(a):
    a = a.astype(np.float32); a /= np.linalg.norm(a, axis=1, keepdims=True); return a
TS = [0.95, 0.9, 0.8, 0.7]; NQ = 100
BUDGETS = [0, 32, 64, 128, 256, 384, 768]
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
def alloc(gam, B):
    b = np.zeros(d, int); f = lambda bb: np.sqrt(gam * GT[bb]); val = f(b)
    for _ in range(B):
        nb = np.minimum(b + 1, 8); gain = val - f(nb); gain[b >= 8] = -1
        i = int(gain.argmax()); b[i] += 1; val = f(b)
    return b
def floor(gam, B):
    """Smallest achievable sum_i sqrt(gamma_i * eps_i) with sum_i 0.5*log2(1/eps_i) = B, eps_i <= 1."""
    g = np.maximum(np.sort(gam)[::-1], 1e-30); lo, hi = 1e-30, g[0]
    for _ in range(200):
        th = np.sqrt(lo * hi)
        if 0.5 * np.log2(np.maximum(g / th, 1)).sum() > B: lo = th
        else: hi = th
    return float(np.sqrt(np.minimum(g, th)).sum())

Chat = kmeans(S, K)
assign = np.empty(N, np.int32); Msum = np.zeros((K, d))
for i in range(0, N, CH):
    x = unit(X16[i:i+CH]); a = (x @ Chat.T).argmax(1); assign[i:i+CH] = a; np.add.at(Msum, a, x)
Mean = (Msum / np.maximum(np.bincount(assign, minlength=K), 1)[:, None]).astype(np.float32)
sa = assign[si]; qm = Q @ Mean.T
npq = 4000 if SMALL else 20000
pq = S[rng.choice(len(S), npq, replace=False)]
pk = np.argsort(-(pq @ Chat.T), axis=1)[:, :8]
def cov_delta(mask):
    rows, cols = np.where(mask); C = np.zeros((d, d))
    for i in range(0, len(rows), 40_000):
        D = (pq[rows[i:i+40_000]] - Mean[pk[rows[i:i+40_000], cols[i:i+40_000]]]).astype(np.float64); C += D.T @ D
    return C / max(len(rows), 1), len(rows)
SD_all, _ = cov_delta(np.ones_like(pk, bool))

results = {}
for G in GS:
    if G == 1: grp = np.zeros(K, int)
    else:
        Mh = Mean / np.linalg.norm(Mean, axis=1, keepdims=True); grp = (Mh @ kmeans(Mh, G, 20).T).argmax(1)
    acc = {B: np.zeros((len(TS), 5)) for B in BUDGETS}
    hist = {B: np.zeros(40) for B in BUDGETS}; rmax = {B: 0.0 for B in BUDGETS}
    viol = 0; floors = []
    for g in range(G):
        ks = np.where(grp == g)[0]
        if len(ks) == 0: continue
        kpos = -np.ones(K, int); kpos[ks] = np.arange(len(ks))
        m = grp[sa] == g; Rs = (S[m] - Mean[sa[m]]).astype(np.float64)
        lam, U = np.linalg.eigh(Rs.T @ Rs / max(len(Rs), 1)); lam = np.maximum(lam[::-1], 1e-6); U = U[:, ::-1]
        SDg, npairs = cov_delta(grp[pk] == g)
        SDg = SD_all if npairs < 200 else 0.8 * SDg + 0.2 * SD_all
        Lh = np.sqrt(lam)
        Mx = (U * Lh).T @ SDg @ (U * Lh)
        gam, P = np.linalg.eigh(Mx); gam = np.maximum(gam[::-1], 1e-30); P = P[:, ::-1]
        Vp = ((U / Lh) @ P).astype(np.float32)        # r' = r @ Vp   (unit variance, query energy diagonal)
        Vd = ((U * Lh) @ P).astype(np.float32)        # dual: <a, r> = <a @ Vd, r'>
        gam_pca = np.diag(U.T @ SDg @ U) * lam
        floors.append([[floor(gam_pca, B), floor(gam, B)] for B in BUDGETS])
        Qe = gam.copy()
        QV = Q @ Vd; MV = Mean[ks] @ Vd
        Dl = QV[None] - MV[:, None]
        dn = np.sqrt(np.maximum(1 - 2 * qm[:, ks].T + (Mean[ks] ** 2).sum(1)[:, None], 0))
        specs = []
        for B in BUDGETS:
            bits = alloc(gam, B); Dv = ((GT[bits] + 1e-10) / (Qe + 1e-10)) ** 0.25; Dv = Dv.astype(np.float32)
            specs.append((B, bits, Dv, np.sqrt(((Dl * Dv) ** 2).sum(2))))
        if g == 0:
            for B, bits, _, _ in specs: log("G", G, B, "bits -> dims coded", int((bits > 0).sum()), "max bits", int(bits.max()))
        items = np.where(grp[assign] == g)[0]
        for c in range(0, len(items), CH):
            ii = items[c:c+CH]; x = unit(X16[ii]); a = assign[ii]; ap = kpos[a]
            true = x @ Q.T; base = qm[:, a].T; z = (x - Mean[a]) @ Vp; mva = MV[ap]
            for B, bits, Dv, dw in specs:
                if B == 0:
                    evec = z; gm = np.zeros(len(z), np.float32); sc = np.zeros_like(true)
                else:
                    code = quant(z, bits); cn2 = (code * code).sum(1) + 1e-20; gm = (z * code).sum(1) / cn2
                    evec = z - gm[:, None] * code; sc = code @ QV.T
                en = np.linalg.norm(evec @ Vd.T, axis=1); ew = np.linalg.norm(evec / Dv, axis=1); me = (mva * evec).sum(1)
                est = base + gm[:, None] * sc + me[:, None]
                h = np.minimum(dn[ap] * en[:, None], dw[ap] * ew[:, None]) * 1.0005 + 5e-5
                ub = est + h; lb = est - h
                ratio = np.abs(est - true) / h
                hist[B] += np.histogram(ratio, bins=40, range=(0, 1))[0]; rmax[B] = max(rmax[B], float(ratio.max()))
                for ti, t in enumerate(TS):
                    A = true >= t; sv = ub >= t; win = lb >= t
                    acc[B][ti] += [A.sum(), sv.sum(), (sv & ~win).sum(), win.sum(), h[A].sum()]
                    viol += int((A & ~sv).sum() + (win & ~(true >= t - 5e-5)).sum())
        if g % 8 == 0: log("G", G, "group", g, "violations", viol)
    fl = np.mean(np.array(floors), axis=0)
    results[G] = {"N": N, "NQ": NQ, "TS": TS, "violations": viol, "budgets": BUDGETS,
                  "acc": {"gen": {str(B): a.tolist() for B, a in acc.items()}},
                  "floor_pca_vs_gen": fl.tolist(),
                  "ratio_hist": {str(B): v.tolist() for B, v in hist.items()}, "ratio_max": rmax, "err_median": {}}
    json.dump(results, open("results5_small.json" if SMALL else "results5.json", "w"))
    log("G", G, "done, violations", viol, "| floor (pca-diagonal vs simultaneous) by budget:", [[round(v, 4) for v in r] for r in fl])
    log("   max |actual error| / guaranteed bound by budget:", {B: round(v, 3) for B, v in rmax.items()})
