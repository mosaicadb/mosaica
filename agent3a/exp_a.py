# E1: all-randomised geometric blocks with fractional (sub-1-bit) rates, against exp8's det and split.
# usage: python3 exp_a.py [DIM]   (DIM: optional PCA truncation of the data to DIM dimensions, renormalised)
import numpy as np, time, json, sys, itertools
rng = np.random.default_rng(0)
t0 = time.time()
def log(*a): print(f"[{time.time()-t0:6.0f}s]", *a, flush=True)
DATA = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/data/"
DIM = int(sys.argv[1]) if len(sys.argv) > 1 else 0
def unit(a):
    a = a.astype(np.float32); a /= np.linalg.norm(a, axis=1, keepdims=True); return a
X = unit(np.concatenate([np.load(DATA + f"e{i:02d}.npy") for i in (0, 5, 10, 15)]))
if DIM:
    C = (X[::10].T @ X[::10]).astype(np.float64); w, P = np.linalg.eigh(C); P = P[:, ::-1][:, :DIM].astype(np.float32)
    X = unit(X @ P)
N, d = X.shape
K, G, CH = 200, 8, 50_000
TS = [0.95, 0.9, 0.8, 0.7]; NQ = 100; CS = [8.0, 10.0]
BUD = [256, 768, 1536] if not DIM else [64, 128, 256, 512]
qi = np.sort(rng.choice(N, NQ, replace=False)); Q = X[qi]
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
def alloc_mse(gam, B):
    b = np.zeros(len(gam), int); val = gam * GT[b]
    for _ in range(B):
        nb = np.minimum(b + 1, 8); gain = val - gam * GT[nb]; gain[b >= 8] = -1
        i = int(gain.argmax()); b[i] += 1; val[i] = gam[i] * GT[b[i]]
    return b
# ---- new design: geometric blocks over dims sorted by a_i = sqrt(lam_i Q_i), rate menu with sub-bit rates
RATES = [0, 0.125, 0.25, 0.5, 1, 2, 3, 4, 5, 6, 7, 8]
def Tof(r):                       # predicted tan^2(phi) of a rotated block at rate r bits per dimension
    if r == 0: return np.inf
    if r < 1: return np.pi / (2 * r) - 1
    return GT[int(r)] / (1 - GT[int(r)])
def bits_of(r, nb): return int(r * nb)
def design(a_sorted, bounds, B, c=8.0):
    # each block: rate 0 (deterministic, uncoded), or coded and either deterministic (A*|Delta|) or randomised; chosen on predicted bound
    blocks = [(bounds[i], bounds[i + 1]) for i in range(len(bounds) - 1)]; J = len(blocks)
    Sb = np.array([a_sorted[lo:hi].sum() for lo, hi in blocks]); nbs = np.array([hi - lo for lo, hi in blocks])
    masks = np.array(list(itertools.product([0, 1], repeat=J)), bool)          # True = randomised
    def h(ri):
        T = np.array([Tof(RATES[r]) if r > 0 else 1.0 for r in ri]); coded = np.array(ri) > 0
        dj = Sb * np.sqrt(T); vj = np.where(coded, Sb ** 2 * T / (nbs - 1), 1e30)
        val = (dj[None] * ~masks).sum(1) + c * np.sqrt((vj[None] * masks).sum(1)); i = int(val.argmin())
        return val[i], masks[i]
    ri = [0] * J; used = 0
    while True:
        best = None; h0 = h(ri)[0]
        for j in range(J):
            for r2 in range(ri[j] + 1, len(RATES)):
                db = bits_of(RATES[r2], nbs[j]) - bits_of(RATES[ri[j]], nbs[j])
                if db <= 0 or used + db > B: continue
                r_try = list(ri); r_try[j] = r2; gain = (h0 - h(r_try)[0]) / db
                if best is None or gain > best[0]: best = (gain, j, r2, db)
        if best is None or best[0] <= 0: break
        _, j, r2, db = best; ri[j] = r2; used += db
    hv, fl = h(ri)
    return blocks, [RATES[r] for r in ri], used, fl
if not DIM: BOUNDS = {"geoA": [0, 24, 48, 96, 192, 384, 768], "geoB": [0, 32, 96, 224, 480, 768]}
else: BOUNDS = {"geoA": [b for b in [0, 24, 48, 96, 192, 384, 768, 1536] if b < d] + [d]}
Chat = kmeans(X, K)
assign = (X @ Chat.T).argmax(1).astype(np.int32); Msum = np.zeros((K, d)); np.add.at(Msum, assign, X)
Mean = (Msum / np.maximum(np.bincount(assign, minlength=K), 1)[:, None]).astype(np.float32)
qm = Q @ Mean.T
pq = X[rng.choice(N, 2000, replace=False)]
pk = np.argsort(-(pq @ Chat.T), axis=1)[:, :4]
Mh = Mean / np.linalg.norm(Mean, axis=1, keepdims=True); grp = (Mh @ kmeans(Mh, G, 20).T).argmax(1)
NMIN = 24
names = [f"det{B}" for B in BUD] + [f"{s}{B}c{int(c)}" for s in ["split"] + list(BOUNDS) for B in BUD for c in CS]
DESIGNED = {f"{s}{B}c{int(c)}": c for s in BOUNDS for B in BUD for c in CS}
acc = {n: np.zeros((len(TS), 5)) for n in names}; viol = {n: 0 for n in names}
sig_max = {**{f"split{B}": 0.0 for B in BUD}, **{k: 0.0 for k in DESIGNED}}; npairs = 0
scal = {}
def tally(name, est, h, true):
    h = h * 1.0005 + 5e-5; ub = est + h; lb = est - h
    for ti, t in enumerate(TS):
        A = true >= t; sv = ub >= t; win = lb >= t
        acc[name][ti] += [A.sum(), sv.sum(), (sv & ~win).sum(), win.sum(), h[A].sum()]
        viol[name] += int((A & ~sv).sum() + (win & ~(true >= t - 5e-5)).sum())
def haar(n): return np.linalg.qr(rng.standard_normal((n, n)))[0].astype(np.float32) if n > 1 else np.ones((1, 1), np.float32)
for g in range(G):
    ks = np.where(grp == g)[0]
    if len(ks) == 0: continue
    kpos = -np.ones(K, int); kpos[ks] = np.arange(len(ks))
    items = np.where(grp[assign] == g)[0]
    Rs = X[items] - Mean[assign[items]]
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
    for B in BUD:
        bits = alloc(lam, Qe, B); Dv = (((lam * GT[bits] + 1e-10) / (Qe + 1e-10)) ** 0.25).astype(np.float32)
        det[B] = (bits, Dv, np.sqrt((Dl2 * Dv ** 2).sum(2)))
    Dt = (((lam + 1e-10) / (Qe + 1e-10)) ** 0.25).astype(np.float32)
    QVt = QV * Dt; MVt = MV * Dt; Dl2t = Dl2 * Dt ** 2
    a_i = np.sqrt(lam * Qe); order = np.argsort(-a_i)
    # schemes: name -> list of blocks (rate, idx, R, dnb, QR, MR, random?)
    schemes = {}
    for B in BUD:
        bits = alloc_mse(lam * Qe, B); bl = []
        for v in np.unique(bits):
            idx = np.where(bits == v)[0]; R = haar(len(idx)) if v > 0 else None
            bl.append((float(v), idx, R, np.sqrt(Dl2t[:, :, idx].sum(2)), None if v == 0 else QVt[:, idx] @ R.T, None if v == 0 else MVt[:, idx] @ R.T, v > 0 and len(idx) >= NMIN))
        schemes[f"split{B}"] = bl
        scal[f"split{B}"] = 1 + sum(2 if b[0] > 0 else 1 for b in bl)
        for s, bounds, cd in [(s, b, c) for s, b in BOUNDS.items() for c in CS]:
            blocks, rates, used, fl = design(a_i[order], bounds, B, cd); bl = []
            for (lo, hi), r, isr in zip(blocks, rates, fl):
                idx = order[lo:hi]; R = haar(len(idx)) if r > 0 else None
                bl.append((r, idx, R, np.sqrt(Dl2t[:, :, idx].sum(2)), None if r == 0 else QVt[:, idx] @ R.T, None if r == 0 else MVt[:, idx] @ R.T, bool(isr) and r > 0))
            sn = f"{s}{B}c{int(cd)}"; schemes[sn] = bl; scal[sn] = 1 + sum(2 if b[0] > 0 else 1 for b in bl)
            if g == 0: log(sn, "bits used", used, "blocks (dims, rate, randomised)", [(hi - lo, r, int(bool(f) and r > 0)) for (lo, hi), r, f in zip(blocks, rates, fl)])
        if g == 0: log("split", B, "blocks (depth, dims)", [(b[0], len(b[1])) for b in schemes[f"split{B}"]])
    for c0 in range(0, len(items), CH):
        ii = items[c0:c0+CH]; x = X[ii]; a = assign[ii]; ap = kpos[a]
        true = x @ Q.T; base = qm[:, a].T; z = (x - Mean[a]) @ V; mva = MV[ap]
        base2 = base + (mva * z).sum(1)[:, None]
        npairs += true.size
        for B, (bits, Dv, dw) in det.items():
            code = quant(z / sig, bits) * sig; cn2 = (code * code).sum(1) + 1e-20; gm = (z * code).sum(1) / cn2
            evec = z - gm[:, None] * code
            est = base + gm[:, None] * (code @ QV.T) + (mva * evec).sum(1)[:, None]
            h = np.minimum(dn[ap] * np.linalg.norm(evec, axis=1)[:, None], dw[ap] * np.linalg.norm(evec / Dv, axis=1)[:, None])
            tally(f"det{B}", est, h, true)
        zt = z / Dt
        for sname, bl in schemes.items():
            est = base2.copy(); hdet = np.zeros_like(true); var = np.zeros_like(true); rerr = np.zeros_like(true)
            for r, idx, R, dnb, QR_, MR_, israndom in bl:
                nb = len(idx)
                if r == 0:
                    hdet += dnb[ap] * np.linalg.norm(zt[:, idx], axis=1)[:, None]; continue
                y = zt[:, idx] @ R.T; l2 = np.linalg.norm(y, axis=1) + 1e-20; mr = MR_[ap]
                if r < 1:
                    kk = bits_of(r, nb); cy = np.sign(y[:, :kk]) + (y[:, :kk] == 0)
                    num = np.abs(y[:, :kk]).sum(1); cn = np.sqrt(kk)
                    dot = cy @ QR_[:, :kk].T - (cy * mr[:, :kk]).sum(1)[:, None]
                else:
                    cy = quant(y / l2[:, None] * np.sqrt(nb), np.full(nb, int(r)))
                    num = (cy * y).sum(1); cn = np.linalg.norm(cy, axis=1)
                    dot = cy @ QR_.T - (cy * mr).sum(1)[:, None]
                a_al = np.clip(num / (cn * l2), 1e-6, 1.0); A = l2 * np.sqrt(1 - a_al ** 2) / a_al
                eb = (l2 * l2 / num)[:, None] * dot
                est += eb
                if israndom:
                    var += (A[:, None] * dnb[ap]) ** 2 / (nb - 1)
                    rerr += eb - (y @ QR_.T - (y * mr).sum(1)[:, None])
                else: hdet += A[:, None] * dnb[ap]
            sd = np.sqrt(var); ratio = np.abs(rerr) / np.maximum(sd, 1e-12)
            sig_max[sname] = max(sig_max[sname], float(ratio.max()))
            if sname in DESIGNED: tally(sname, est, hdet + DESIGNED[sname] * sd, true)
            else:
                for c in CS: tally(f"{sname}c{int(c)}", est, hdet + c * sd, true)
    log("group", g, "violations", {n: v for n, v in viol.items() if v}, "max sigma", {k: round(v, 2) for k, v in sig_max.items()})
out = {"N": N, "dim": d, "NQ": NQ, "TS": TS, "pairs": npairs, "viol": viol, "sig_max": sig_max, "scalars": scal,
       "acc": {n: v.tolist() for n, v in acc.items()}}
json.dump(out, open(f"results_a{DIM or ''}.json", "w"))
print("pairs", npairs, "total violations", sum(viol.values()), "max error in predicted sd", {k: round(v, 2) for k, v in sig_max.items()})
print("stored scalars per item (last group)", scal)
for n in names:
    a = acc[n]; A = a[:, 0]
    print(f"{n:14s} undecided/A", " ".join(f"{x/y:7.2f}" for x, y in zip(a[:, 2], A)), " | half-width", " ".join(f"{x/y:.4f}" for x, y in zip(a[:, 4], A)))
