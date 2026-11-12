import numpy as np, time, json, sys
rng = np.random.default_rng(0)
t0 = time.time()
def log(*a): print(f"[{time.time()-t0:6.0f}s]", *a, flush=True)
SMALL = len(sys.argv) > 1
X16 = np.concatenate([np.load(f"data/e{i:02d}.npy") for i in ((0, 7) if SMALL else range(20))])
N, d = X16.shape
K = 100 if SMALL else 1000
G = 4 if SMALL else 64
CH = 50_000
def unit(a):
    a = a.astype(np.float32); a /= np.linalg.norm(a, axis=1, keepdims=True); return a
TS = [0.95, 0.9, 0.8, 0.7]; NQ = 100; CS = [6.0, 8.0, 10.0]
# split configs: head dims, head bits, rotated 1-bit blocks, (rest: no bits, deterministic)
SPLITS = {256: (16, 64, [(16, 208)]), 512: (32, 128, [(32, 160), (160, 416)]), 960: (64, 256, [(64, 192), (192, 448), (448, 768)])}
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
def alloc(lam, Qe, B):
    n = len(lam); b = np.zeros(n, int); f = lambda bb: np.sqrt(Qe * lam * GT[bb]); val = f(b)
    for _ in range(B):
        nb = np.minimum(b + 1, 8); gain = val - f(nb); gain[b >= 8] = -1
        i = int(gain.argmax()); b[i] += 1; val = f(b)
    return b
Chat = kmeans(S, K)
assign = np.empty(N, np.int32); Msum = np.zeros((K, d))
for i in range(0, N, CH):
    x = unit(X16[i:i+CH]); a = (x @ Chat.T).argmax(1); assign[i:i+CH] = a; np.add.at(Msum, a, x)
Mean = (Msum / np.maximum(np.bincount(assign, minlength=K), 1)[:, None]).astype(np.float32)
sa = assign[si]; qm = Q @ Mean.T
pq = S[rng.choice(len(S), 5000 if not SMALL else 1000, replace=False)]
pk = np.argsort(-(pq @ Chat.T), axis=1)[:, :4]
Mh = Mean / np.linalg.norm(Mean, axis=1, keepdims=True); grp = (Mh @ kmeans(Mh, G, 20).T).argmax(1)
names = [f"det{B}" for B in SPLITS] + [f"split{B}c{int(c)}" for B in SPLITS for c in CS]
acc = {n: np.zeros((len(TS), 5)) for n in names}; viol = {n: 0 for n in names}
sig_hist = {B: np.zeros(48) for B in SPLITS}; sig_max = {B: 0.0 for B in SPLITS}; npairs = 0
def tally(name, est, h, true):
    h = h * 1.0005 + 5e-5; ub = est + h; lb = est - h
    for ti, t in enumerate(TS):
        A = true >= t; sv = ub >= t; win = lb >= t
        acc[name][ti] += [A.sum(), sv.sum(), (sv & ~win).sum(), win.sum(), h[A].sum()]
        viol[name] += int((A & ~sv).sum() + (win & ~(true >= t - 5e-5)).sum())
for g in range(G):
    ks = np.where(grp == g)[0]
    if len(ks) == 0: continue
    kpos = -np.ones(K, int); kpos[ks] = np.arange(len(ks))
    m = grp[sa] == g; Rs = S[m] - Mean[sa[m]]
    lam, V = np.linalg.eigh((Rs.T @ Rs).astype(np.float64) / max(len(Rs), 1))
    lam = np.maximum(lam[::-1], 1e-12).copy(); V = V[:, ::-1].astype(np.float32).copy(); sig = np.sqrt(lam).astype(np.float32)
    pm = grp[pk] == g
    if pm.sum() > 50:
        rows, cols = np.where(pm); Qe = (((pq[rows] - Mean[pk[rows, cols]]) @ V) ** 2).mean(0).astype(np.float64)
    else: Qe = lam.copy()
    QV = Q @ V; MV = Mean[ks] @ V
    Dl = QV[None] - MV[:, None]; Dl2 = Dl ** 2
    dn = np.sqrt(Dl2.sum(2))
    det = {}
    for B in SPLITS:
        bits = alloc(lam, Qe, B); Dv = (((lam * GT[bits] + 1e-10) / (Qe + 1e-10)) ** 0.25).astype(np.float32)
        det[B] = (bits, Dv, np.sqrt((Dl2 * Dv ** 2).sum(2)))
    spl = {}
    for B, (nH, BH, blocks) in SPLITS.items():
        bits = alloc(lam[:nH], Qe[:nH], BH); Dv = (((lam[:nH] * GT[bits] + 1e-10) / (Qe[:nH] + 1e-10)) ** 0.25).astype(np.float32)
        rots = [np.linalg.qr(rng.standard_normal((e - s, e - s)))[0].astype(np.float32) for s, e in blocks]
        end = blocks[-1][1]
        spl[B] = (nH, bits, Dv, np.sqrt(Dl2[:, :, :nH].sum(2)), np.sqrt((Dl2[:, :, :nH] * Dv ** 2).sum(2)), blocks, rots,
                  [np.sqrt(Dl2[:, :, s:e].sum(2)) for s, e in blocks], np.sqrt(Dl2[:, :, end:].sum(2)))
    items = np.where(grp[assign] == g)[0]
    for c0 in range(0, len(items), CH):
        ii = items[c0:c0+CH]; x = unit(X16[ii]); a = assign[ii]; ap = kpos[a]
        true = x @ Q.T; base = qm[:, a].T; z = (x - Mean[a]) @ V; mva = MV[ap]
        base2 = base + (mva * z).sum(1)[:, None]              # <q,m> + <m,r>, both exact
        npairs += true.size
        for B, (bits, Dv, dw) in det.items():
            code = quant(z / sig, bits) * sig; cn2 = (code * code).sum(1) + 1e-20; gm = (z * code).sum(1) / cn2
            evec = z - gm[:, None] * code
            est = base + gm[:, None] * (code @ QV.T) + (mva * evec).sum(1)[:, None]
            h = np.minimum(dn[ap] * np.linalg.norm(evec, axis=1)[:, None], dw[ap] * np.linalg.norm(evec / Dv, axis=1)[:, None])
            tally(f"det{B}", est, h, true)
        for B, (nH, bits, Dv, dnH, dwH, blocks, rots, dnB, dn0) in spl.items():
            zH = z[:, :nH]; code = quant(zH / sig[:nH], bits) * sig[:nH]
            cn2 = (code * code).sum(1) + 1e-20; gm = (zH * code).sum(1) / cn2; eH = zH - gm[:, None] * code
            est = base2 + gm[:, None] * (code @ QV[:, :nH].T - (code * mva[:, :nH]).sum(1)[:, None])
            hH = np.minimum(dnH[ap] * np.linalg.norm(eH, axis=1)[:, None], dwH[ap] * np.linalg.norm(eH / Dv, axis=1)[:, None])
            var = np.zeros_like(true); tail_err = np.zeros_like(true)
            for (s, e), R, dnb in zip(blocks, rots, dnB):
                nb = e - s; y = z[:, s:e] @ R.T; sg = np.sign(y); l1 = np.abs(y).sum(1); l2 = np.linalg.norm(y, axis=1)
                scale = l2 * l2 / l1                                # unbiased rescaling: |y| / (a * sqrt(n))
                a_al = l1 / (np.sqrt(nb) * l2); tanphi = np.sqrt(np.maximum(1 - a_al ** 2, 0)) / a_al
                eb = scale[:, None] * (sg @ (QV[:, s:e] @ R.T).T - (sg * (mva[:, s:e] @ R.T)).sum(1)[:, None])
                tb = z[:, s:e] @ QV[:, s:e].T - (z[:, s:e] * mva[:, s:e]).sum(1)[:, None]
                est = est + eb; tail_err += eb - tb
                var += ((l2 * tanphi)[:, None] * dnb[ap]) ** 2 / (nb - 1)
            end = blocks[-1][1]
            h0 = dn0[ap] * np.linalg.norm(z[:, end:], axis=1)[:, None]
            sd = np.sqrt(var); ratio = np.abs(tail_err) / np.maximum(sd, 1e-12)
            sig_hist[B] += np.histogram(ratio, bins=48, range=(0, 12))[0]; sig_max[B] = max(sig_max[B], float(ratio.max()))
            for c in CS: tally(f"split{B}c{int(c)}", est, hH + c * sd + h0, true)
    if g % 8 == 0: log("group", g, "split violations", {n: v for n, v in viol.items() if v}, "max sigma", {B: round(v, 2) for B, v in sig_max.items()})
out = {"N": N, "NQ": NQ, "TS": TS, "pairs": npairs, "viol": viol, "sig_max": sig_max,
       "sig_hist": {str(B): v.tolist() for B, v in sig_hist.items()}, "acc": {n: v.tolist() for n, v in acc.items()}}
json.dump(out, open("results7_small.json" if SMALL else "results7.json", "w"))
print("pairs", npairs, "violations", viol, "max tail error in predicted standard deviations", sig_max)
tot = N * NQ
for n in names:
    a = acc[n]; A = a[:, 0]
    print(f"{n:14s} undecided/A", " ".join(f"{x/y:7.2f}" for x, y in zip(a[:, 2], A)), " | half-width", " ".join(f"{x/y:.4f}" for x, y in zip(a[:, 4], A)))
