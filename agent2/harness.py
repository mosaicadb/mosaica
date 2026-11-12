# Deterministic progressive certificate on the 200k subset, for arbitrary query sets and thresholds chosen by answer size.
# Same construction as ../exp2.py ("local" variant); what varies per config: the energy profile used for bit allocation,
# the list of weightings whose weighted error norms are stored, and whether the mean coefficient alpha is optimised.
import numpy as np, json, time, sys, os
SC = float(os.environ.get("VSCALE", 1))            # n / 768, for the reduced-dimension stand-in
LEVELS = [int(L * SC) for L in [0, 4, 8, 16, 32, 48, 96, 192, 384, 576]]; SIZES = [20, 200, 2000, 10000]; DS = [0.0, 0.005, 0.01, 0.02]
FULL = int(1536 * SC); SCAL = 4
STEP = {2: 0.9957, 3: 0.5860, 4: 0.3352, 5: 0.1881, 6: 0.1041, 7: 0.0569, 8: 0.0308}
GT = np.array([1, .3634, .1188, .03744, .01154, .00349, .00104, .0003043, .0000876])
t0 = time.time()
def log(*a): print(f"[{time.time()-t0:5.0f}s]", *a, flush=True)
def unit(a):
    a = a.astype(np.float32); a /= np.linalg.norm(a, axis=1, keepdims=True); return a
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
    b = np.zeros(len(lam), int); f = lambda bb: np.sqrt(Qe * lam * GT[bb]); val = f(b)
    for _ in range(B):
        nb = np.minimum(b + 1, 8); gain = val - f(nb); gain[b >= 8] = -1
        i = int(gain.argmax()); b[i] += 1; val = f(b)
    return b
def load():
    I = dict(np.load("index.npz"))
    I["X"] = np.load("X.npy") if os.path.exists("X.npy") else np.concatenate([unit(np.load(f"../data/e{i:02d}.npy")) for i in (0, 5, 10, 15)])
    I["Z"] = np.load("Z.npy", mmap_mode="r")
    return I
def thresholds(X, Q):
    thr = np.zeros((len(Q), len(SIZES)), np.float32)
    for j in range(0, len(Q), 64):
        s = -np.sort(-(X @ Q[j:j+64].T), axis=0)
        for i, k in enumerate(SIZES): thr[j:j+64, i] = (s[k - 1] + s[k]) / 2
    return thr
def text_profile(I, T, top=50):
    """Query-offset energy per local dimension from text queries, against each query's `top` clusters by q.m"""
    Mean, grp, V = I["Mean"], I["grp"], I["V"]; G = len(V)
    tk = np.argsort(-(T @ Mean.T), axis=1)[:, :top]; Qt = np.zeros_like(I["lam"])
    for g in range(G):
        rows, cols = np.where(grp[tk] == g)
        if len(rows) < 200: rows, cols = np.where(np.ones_like(tk, bool)); ks = np.where(grp == g)[0]; kk = ks[np.arange(len(rows)) % len(ks)]
        else: kk = tk[rows, cols]
        Qt[g] = (((T[rows] - Mean[kk]) @ V[g]) ** 2).mean(0)
    return Qt
def run(I, Q, thr, cfg, CH=4000):
    """cfg: dict(bits=G x n energy profile for allocation, weights=[G x n profiles], alpha=bool). Returns raw tallies."""
    X, Z, Mean, grp, V, lam, assign = I["X"], I["Z"], I["Mean"], I["grp"], I["V"], I["lam"], I["assign"]
    N, n = X.shape; NQ = len(Q); G = len(V); NL = len(LEVELS); NT = thr.shape[1]; ND = len(DS)
    U = np.zeros((NL, NT, ND, NQ)); H = np.zeros((NL, NT, NQ)); A_ = np.zeros((NT, NQ)); viol = np.zeros(NL, int); which = np.zeros((NL, len(cfg["weights"]) + 1))
    qm = Q @ Mean.T
    for g in range(G):
        ks = np.where(grp == g)[0]; kpos = -np.ones(len(grp), int); kpos[ks] = np.arange(len(ks))
        sig = np.sqrt(lam[g]).astype(np.float32); QV = Q @ V[g]; MV = Mean[ks] @ V[g]
        specs = []
        for B in LEVELS:
            bits = alloc(lam[g], cfg["bits"][g], B * 8); ws = []
            for W in [None] + list(cfg["weights"]):
                w2 = np.ones(n, np.float32) if W is None else np.sqrt((lam[g] * GT[bits] + 1e-10) / (W[g] + 1e-10)).astype(np.float32)   # D^2
                Aqm = (MV * w2) @ QV.T; Bm = (MV * MV) @ w2; Cq = (QV * QV) @ w2      # kg x NQ, kg, NQ
                if cfg.get("alpha"): al = Aqm / Bm[:, None]; dw = np.sqrt(np.maximum(Cq[None] - Aqm ** 2 / Bm[:, None], 0))
                else: al = np.ones_like(Aqm); dw = np.sqrt(np.maximum(Cq[None] - 2 * Aqm + Bm[:, None], 0))
                dwb = None
                if cfg.get("bands") and W is not None:     # banded form: sum over bands of |D Delta_band| |D^-1 e_band|
                    st = np.array(cfg["bands"][:-1]); Dl = QV[None] - al[:, :, None].astype(np.float32) * MV[:, None]
                    dwb = np.sqrt(np.add.reduceat(Dl * Dl * w2, st, axis=2)).astype(np.float32)
                ws.append((w2, al.astype(np.float32), dw.astype(np.float32), dwb))
            specs.append((bits, ws))
        items = np.where(grp[assign] == g)[0]
        for c0 in range(0, len(items), CH):
            ii = items[c0:c0+CH]; x = X[ii]; a = assign[ii]; ap = kpos[a]; z = np.asarray(Z[ii]); mva = MV[ap]
            true = x @ Q.T; base = qm[:, a].T
            Am = true[:, None, :] >= thr.T[None]                                  # items x NT x NQ
            A_ += Am.sum(0)
            und = np.ones((len(ii), NT, ND, NQ), bool)
            for li, (bits, ws) in enumerate(specs):
                if LEVELS[li] == 0: evec = z; gam = np.zeros(len(z), np.float32); sc = np.zeros_like(true)
                else:
                    code = quant(z / sig, bits) * sig; cn2 = (code * code).sum(1) + 1e-20; gam = (z * code).sum(1) / cn2
                    evec = z - gam[:, None] * code; sc = code @ QV.T
                me = (mva * evec).sum(1); core = base + gam[:, None] * sc
                ub = np.full_like(true, np.inf); lb = np.full_like(true, -np.inf); best = np.full_like(true, np.inf); arg = np.zeros(true.shape, np.int8)
                for wi, (w2, al, dw, dwb) in enumerate(ws):
                    est = core + al[ap] * me[:, None]
                    if dwb is None: h = dw[ap] * np.sqrt((evec * evec / w2).sum(1))[:, None]
                    else: h = np.einsum("iqb,ib->iq", dwb[ap], np.sqrt(np.add.reduceat(evec * evec / w2, np.array(cfg["bands"][:-1]), axis=1)))
                    h = h * 1.0001 + 2e-5
                    np.minimum(ub, est + h, out=ub); np.maximum(lb, est - h, out=lb)
                    m = h < best; arg[m] = wi; best[m] = h[m]
                which[li] += np.bincount(arg.ravel(), minlength=len(ws))
                hw = (ub - lb) / 2
                for ti in range(NT):
                    t = thr[None, :, ti]; A = Am[:, ti]; sv = ub >= t
                    viol[li] += int((A & ~sv).sum())
                    H[li, ti] += (hw * A).sum(0)
                    for di, dd in enumerate(DS):
                        win = lb >= t - dd
                        viol[li] += int((win & ~(true >= t - dd - 2e-5)).sum())
                        und[:, ti, di] &= sv & ~win
                        U[li, ti, di] += und[:, ti, di].sum(0)
        log(cfg.get("name", ""), "group", g, "violations", int(viol.sum()))
    return dict(U=U, H=H, A=A_, viol=viol, which=which, N=N)
def metric(r, extra=0, start=0):
    """share of the dataset read per query: N scalars, then each level's increment for items undecided at the previous level, then the full vector"""
    U = r["U"]; N = r["N"]; cost = (SCAL + extra + LEVELS[start]) * N + np.zeros_like(U[0])     # `start`: first level read by every item
    for li in range(start + 1, len(LEVELS)): cost += (LEVELS[li] - LEVELS[li - 1] + extra) * U[li - 1]
    cost += FULL * U[-1]
    return cost / (N * FULL)                                                    # NT x ND x NQ
def report(r, cls, names, extra=0, start=0):
    M = metric(r, extra, start); out = {}
    for nm in names:
        m = cls == nm if nm != "text(all)" else cls != "item"
        A = r["A"][:, m].sum(1)
        out[nm] = dict(nq=int(m.sum()), share=(r["A"][:, m].mean(1) / r["N"]).tolist(), read=M[:, :, m].mean(2).tolist(),
                       und={L: (r["U"][li, :, 0][:, m].sum(1) / A).tolist() for li, L in enumerate(LEVELS)},
                       hw={L: (r["H"][li][:, m].sum(1) / A).tolist() for li, L in enumerate(LEVELS)})
    return out
def show(rep, title):
    print("\n==", title)
    for nm, o in rep.items():
        print(f"-- {nm} (n={o['nq']}); answer sizes {SIZES}; true share % " + " ".join(f"{100*x:.3f}" for x in o["share"]))
        for di, dd in enumerate(DS): print(f"   read % d={dd:<5}: " + " ".join(f"{100*o['read'][ti][di]:7.2f}" for ti in range(len(SIZES))))
        for L in [int(L * SC) for L in (4, 16, 32, 96, 192, 384)]: print(f"   {L:3d} B undecided/match: " + " ".join(f"{x:8.2f}" for x in o["und"][L]) + "   half-width: " + " ".join(f"{x:.4f}" for x in o["hw"][L]))
