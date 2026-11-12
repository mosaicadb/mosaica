# Shared loading and per-group setup for the k-NN study; the level construction is copied from combined/run.py
# (tracks V3sub+C1+C2 = "R" and V2+C1+C2 = "D").
import numpy as np, sys, os, math, json
VEC = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/"
sys.path.insert(0, VEC + "combined")
from core import *
KNN = VEC + "knn/"
def idxdir(NAME): return VEC + "combined/idx_m768/" if NAME == "m768" else KNN + f"idx_{NAME}/"
def workdir(NAME):
    d = KNN + f"work_{NAME}/"; os.makedirs(d, exist_ok=True); return d

class Ctx:
    def __init__(s, NAME, need_rand=True):
        s.NAME = NAME; I = dict(np.load(idxdir(NAME) + "index.npz")); s.I = I
        s.Xs = np.load(idxdir(NAME) + "Xs.npy", mmap_mode="r"); s.perm = np.load(idxdir(NAME) + "perm.npy")
        s.N, s.n, s.K, s.G = int(I["N"]), int(I["n"]), int(I["K"]), int(I["G"])
        s.assign, s.cstart, s.grp, s.Mq, s.mn, s.ahat = I["assign"], I["cstart"], I["grp"], I["Mq"], I["mn"], I["ahat"]
        s.P, s.nimg, s.HI, s.LO = I["P"], int(I["nimg"]), I["hi"], I["lo"]
        w = workdir(NAME)
        if os.path.exists(w + "qsel.npy"): s.qsel = np.load(w + "qsel.npy")
        else:
            qc = I["qcls"]; r = np.random.default_rng(7)
            it = np.where(qc == "item")[0]; tx = np.where(qc == "text")[0]
            s.qsel = np.sort(np.r_[r.choice(it, min(100, len(it)), replace=False), r.choice(tx, min(150, len(tx)), replace=False)]) if NAME == "m768" else np.arange(len(qc))
            np.save(w + "qsel.npy", s.qsel)
        s.Q = I["Q"][s.qsel].astype(np.float32); s.qcls = I["qcls"][s.qsel]; s.NQ = len(s.Q)
        # row of each item query in the sorted corpus, -1 for queries that are not corpus items
        s.qrow = np.full(s.NQ, -1, np.int64)
        if "qi" in I and len(I["qi"]):
            isit = np.where(s.qcls == "item")[0]; s.qrow[isit] = I["qi"][s.qsel[isit]]
        n = s.n; s.FULL = 2 * n
        s.LV = [b for b in (0, 4, 8, 16, 32, 48, 96, 192, 384, 576) if b <= 0.4 * s.FULL]
        s.LD = 32; s.ld = s.LV.index(32); s.RLV = [b for b in s.LV if b > 32]; s.NR = len(s.RLV); s.ND = len(s.LV)
        s.BOUNDS = [0] + [b for b in (96, 224, 480, 992, 2016, 4064) if b <= n - 64] + [n]; s.J = len(s.BOUNDS) - 1
        s.L1 = math.log(2 * 1e9 * max(s.NR, 1) / 1e-9); s.CSG = math.sqrt(2 * s.L1)
        s.DA = 0.5 / 32767 + 1e-7
        s.MS = np.array([s.BOUNDS[j + 1] - s.BOUNDS[j] - 1 for j in range(s.J)], float)
        s.TABS = {int(m): LogMGF(int(m)) for m in set(s.MS.tolist())} if (s.NR and need_rand) else {}
        s.MENU = [0, 0.125, 0.25, 0.5, 1, 2, 3, 4, 5, 6, 7, 8]
        # query-side constants: nearest anchor, cluster bounds from the support table
        Q = s.Q; Ph = s.P / np.linalg.norm(s.P, axis=1, keepdims=True); QP = Q @ Ph.T
        j = QP.argmax(1); beta = QP[np.arange(s.NQ), j]; rho = np.sqrt(np.maximum(1 - beta ** 2, 0))
        s.anc = (j, beta.astype(np.float32), rho.astype(np.float32), (Ph[j] @ (s.Mq / s.mn[:, None]).T).astype(np.float32))
        s.qm = (Q @ s.Mq.T).astype(np.float32)
        hic, loc = s.HI[j], s.LO[j]; b = beta[:, None]
        s.Bh = np.where(b >= 0, b * hic, b * loc); s.Bl = np.where(b >= 0, b * loc, b * hic)        # NQ x K
        s.ubc = s.Bh + rho[:, None] + 3e-5; s.lbc = s.Bl - rho[:, None] - 3e-5
        s.gstart = np.searchsorted(s.grp, np.arange(s.G + 1))

def Tof(r): return np.inf if r == 0 else (np.pi / (2 * r) - 1 if r < 1 else GTREE[int(r)] / (1 - GTREE[int(r)]))
def bits_of(r, nb): return int(r * nb)

def dws(D, QV, MV):
    w2 = (D.astype(np.float64)) ** 2; QV = QV.astype(np.float64); MV = MV.astype(np.float64)
    Aqm = (MV * w2) @ QV.T; Bm = (MV * MV) @ w2; Cq = (QV * QV) @ w2
    dw = np.sqrt(np.maximum(Cq[None] - 2 * Aqm + Bm[:, None], 0)); al = Aqm / Bm[:, None]
    dwa = np.sqrt(np.maximum(Cq[None] - Aqm ** 2 / Bm[:, None], 0))
    return dw.astype(np.float32), al.astype(np.float32), dwa.astype(np.float32)

def design(S, nb, budgets, menu, c):
    ri = [0] * len(S); out = []; used = 0
    def h(r):
        T = np.array([Tof(menu[k]) for k in r]); cod = np.array(r) > 0
        return S[~cod].sum() + c * math.sqrt(((S ** 2 * np.where(cod, T, 0)) / (nb - 1))[cod].sum()) if cod.any() else S.sum()
    for B in budgets:
        while True:
            best = None; h0 = h(ri)
            for j in range(len(S)):
                for k2 in range(ri[j] + 1, len(menu)):
                    db = bits_of(menu[k2], nb[j]) - bits_of(menu[ri[j]], nb[j])
                    if db <= 0 or used + db > B: continue
                    r2 = list(ri); r2[j] = k2; gn = (h0 - h(r2)) / db
                    if best is None or gn > best[0]: best = (gn, j, k2, db)
            if best is None or best[0] <= 0: break
            ri[best[1]] = best[2]; used += best[3]
        out.append([menu[k] for k in ri])
    return out

class GS: pass
def gsetup(C, g, Q, rand=True, maxlevel=None):
    """Everything per group: basis, weights per level, escape step order, randomised-level design."""
    s = GS(); I = C.I; n = C.n; LV = C.LV; ND = C.ND if maxlevel is None else maxlevel + 1
    s.ks = np.arange(C.gstart[g], C.gstart[g + 1]); ks = s.ks
    lam, V, Qi, Qm = I["lam"][g], I["V"][g], I["Qi"][g], I["Qmix"][g]; s.V = V; s.sig = np.sqrt(lam).astype(np.float32)
    s.QV = (Q @ V).astype(np.float32); s.MV = (C.Mq[ks] @ V).astype(np.float32)
    s.D0 = (((lam + 1e-10) / (Qi + 1e-10)) ** 0.25).astype(np.float32); s.dw0, s.al0, s.dwa0 = dws(s.D0, s.QV, s.MV)
    s.se, s.dep_at, s.sdep, s.expc = esc_steps(lam, Qi, 1.5 * 8 * LV[ND - 1])
    s.s2 = np.where(s.sdep == 2)[0]; s.esc = []
    for l in range(1, ND):
        de = s.dep_at[int((s.expc <= 8 * LV[l]).sum())]; D = (((lam * gE[de] + 1e-10) / (Qi + 1e-10)) ** 0.25).astype(np.float32)
        s.esc.append((D,) + dws(D, s.QV, s.MV) + (de,))
    s.rand = bool(rand and C.NR)
    if s.rand:
        J, BOUNDS, ld = C.J, C.BOUNDS, C.ld
        de = s.esc[ld - 1][4]; lres = lam * gE[de]
        s.Dt = (((lres + 1e-10) / (Qm + 1e-10)) ** 0.25).astype(np.float32); _, s.alR, _ = dws(s.Dt, s.QV, s.MV)
        ai = np.sqrt(lres * Qm); order = np.argsort(-ai)
        s.idx = [order[BOUNDS[j]:BOUNDS[j + 1]] for j in range(J)]; Sj = np.array([ai[ix].sum() for ix in s.idx]); s.nb = np.array([len(ix) for ix in s.idx])
        grng = np.random.default_rng([20261112, g])
        s.Rs = [np.linalg.qr(grng.standard_normal((s.nb[j], s.nb[j])))[0].astype(np.float32) for j in range(J)]
        QVt = (s.QV * s.Dt).astype(np.float32); MVt = (s.MV * s.Dt).astype(np.float32)
        s.QR = [QVt[:, s.idx[j]] @ s.Rs[j].T for j in range(J)]; s.MR = [MVt[:, s.idx[j]] @ s.Rs[j].T for j in range(J)]
        s.nD = []
        for j in range(J):
            a2 = (QVt[:, s.idx[j]].astype(np.float64) ** 2).sum(1); b2 = (MVt[:, s.idx[j]].astype(np.float64) ** 2).sum(1)
            ab = MVt[:, s.idx[j]].astype(np.float64) @ QVt[:, s.idx[j]].astype(np.float64).T
            s.nD.append(np.sqrt(np.maximum(a2[None] - 2 * s.alR * ab + s.alR.astype(np.float64) ** 2 * b2[:, None], 0)).astype(np.float32))
        s.rates = design(Sj, s.nb, [8 * (B - C.LD) for B in C.RLV], C.MENU, C.CSG)
        s.sc = []; prev = [0] * J
        for li, r in enumerate(s.rates):
            ch = [j for j in range(J) if r[j] != prev[j]]; s.sc.append((2 * J if li == 0 else 0) + len(ch)); prev = r
    return s

def prefilter(C, c, ah, qsl=slice(None)):
    """Annulus and support-table bounds per item as a function of the stored a (run.py's C1+C2, all anchors)."""
    m_ = C.mn[c]; DA = C.DA; j, beta, rho, pm = C.anc; beta, rho = beta[qsl], rho[qsl]
    alo = np.maximum(ah - DA, -1)[:, None]; ahi = np.minimum(ah + DA, 1)[:, None]
    cm = np.clip(C.qm[qsl, c] / m_, -1, 1); s1 = np.sqrt(np.maximum(1 - cm * cm, 0))
    aU = np.clip(cm[None], alo, ahi); aL = np.clip(-cm[None], alo, ahi)
    ub1 = cm * aU + s1 * np.sqrt(1 - aU * aU) + 3e-5; lb1 = cm * aL - s1 * np.sqrt(1 - aL * aL) - 3e-5
    Bh, Bl = C.Bh[qsl, c], C.Bl[qsl, c]
    rm = cm - beta * pm[qsl, c]; rp = np.sqrt(np.maximum(rho * rho - rm * rm, 0)); rr = np.maximum(rho, 1e-12)
    aU2 = np.clip((rm / rr)[None], alo, ahi); aL2 = np.clip((-rm / rr)[None], alo, ahi)
    ub2 = Bh + rm * aU2 + rp * np.sqrt(1 - aU2 * aU2) + 3e-5; lb2 = Bl + rm * aL2 - rp * np.sqrt(1 - aL2 * aL2) - 3e-5
    return np.maximum(lb1, lb2), np.minimum(ub1, ub2), (lb1, ub1, lb2, ub2)
