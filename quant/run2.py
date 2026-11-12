# Lean single-track version of combined/run.py (the "V3sub+C1+C2" track = the combined design), with switches for the
# per-item code changes measured here.  usage: python3 run2.py NAME WORKER NWORKERS TAG   (variant set by env)
#   PROF  = Qi | Qmix      profile for the deterministic weights and escape allocation (combined: Qi)
#   SPH   = 0 | 1          sphere |z| <= R (free from the stored a) in the deterministic certificate (Lagrangian dual)
#   NORM  = 8 | 4          bits per deterministic error norm: 8 = absolute 2^(1/16) grid (combined); 4 = relative to a
#                          prediction from the decoded prefix, same grid, code 15 = escape (8 more bits, taken from the code)
#   L0    = 1 | 0          1: level 0 reads a + norm (combined); 0: no level-0 norm, a is read with the first code level
#   SCHED = 4,8,16,32      deterministic level ends in bytes (level 0 is extra when L0=1)
#   RSC   = orig | cheap   randomised-level scalars: orig = |y_b| 2 B + tan 1 B; cheap = |y_b| 1 B relative + tan 4 bits relative
#   ISC   = 0 | 1          per-item scale from |r| (free) divides the escape-code coordinates
import numpy as np, sys, os, math, json
sys.path.insert(0, "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/combined")
from core import *
import core
if os.environ.get("WESC"):                 # depth-2 grid width of the escape code (combined: 0.6745)
    core.W = W = float(os.environ["WESC"]); _k = np.arange(64); _m = [core._cell(k * W, (k + 1) * W) for k in _k]
    core.CENT2 = np.array([c if m > 1e-12 else (k + 0.5) * W for k, (m, c) in zip(_k, _m)], np.float32)
    core.P2 = np.array([2 * m for m, _ in _m]); _g = [1.0, 0.3634, 1 - float((core.P2 * core.CENT2.astype(np.float64) ** 2).sum())]
    for j in range(3, core.EMAXD + 1):
        wj = W * 2.0 ** (2 - j); ii = np.floor(np.abs(core._xs) / wj); _g.append(float((core._pg * (np.abs(core._xs) - (ii + 0.5) * wj) ** 2).sum()))
    core.gE[:] = np.array(_g); core.cE[1] = float((core.P2 * (_k + 1)).sum())
VEC = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/"
NAME, WID, NW, TAG = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
E = os.environ.get
DCODE = E("DCODE", "esc")          # esc: escape code (variable length); fr: fixed-rate nested tree quantiser, table-decodable
PROF, SPH, NORM, L0, RSC, ISC = E("PROF", "Qi"), int(E("SPH", 0)), int(E("NORM", 8)), int(E("L0", 1)), E("RSC", "orig"), int(E("ISC", 0))
SCHED = [int(s) for s in E("SCHED", "4,8,16,32").split(",")]
IDX = VEC + (f"combined/idx_{NAME}/" if os.path.exists(VEC + f"combined/idx_{NAME}/index.npz") else f"quant/idx_{NAME}/")
I = dict(np.load(IDX + "index.npz")); Xs = np.load(IDX + "Xs.npy", mmap_mode="r")
N, n, K, G = int(I["N"]), int(I["n"]), int(I["K"]), int(I["G"])
assign, cstart, grp, Mq, mn, ahat = I["assign"], I["cstart"], I["grp"], I["Mq"], I["mn"], I["ahat"]
lamA, VA, QiA, QmA = I["lam"], I["V"], I["Qi"], I["Qmix"]
Q, qcls, thr, P, HI, LO = I["Q"], I["qcls"], I["thr"], I["P"], I["hi"], I["lo"]
NQ, NT = len(Q), thr.shape[1]; FULL = 2 * n
DLV = ([0] if L0 else []) + SCHED; LD = SCHED[-1]
RLV = [b for b in [int(v) for v in E("RLV", "48,96,192,384,576").split(",")] if b >= LD and b <= 0.4 * FULL]; NR = len(RLV)   # b == LD: block norms only
ALLV = DLV + RLV
BOUNDS = [0] + [b for b in (96, 224, 480, 992, 2016, 4064) if b <= n - 64] + [n]; J = len(BOUNDS) - 1
EPS, NTARGET = 1e-9, 1e9
L1 = math.log(2 * NTARGET * max(NR, 1) / EPS); CSG = math.sqrt(2 * L1)
DA = 0.5 / 32767 + 1e-7
MENU = [0, 0.125, 0.25, 0.5, 1, 2, 3, 4, 5, 6, 7, 8]
def Tof(r): return np.inf if r == 0 else (np.pi / (2 * r) - 1 if r < 1 else GTREE[int(r)] / (1 - GTREE[int(r)]))
def bits_of(r, nb): return int(r * nb)
MS = np.array([BOUNDS[j + 1] - BOUNDS[j] - 1 for j in range(J)], float)
TABS = {int(m): LogMGF(int(m)) for m in set(MS.tolist())} if NR else {}
RY = 2.0 ** (1 / 4096) if RSC == "orig" else 2.0 ** (1 / 256)
KAP = [0.1, 0.3, 1.0, 3.0, 10.0]
log(NAME, TAG, "worker", WID, dict(PROF=PROF, SPH=SPH, NORM=NORM, L0=L0, SCHED=SCHED, RSC=RSC, ISC=ISC), "levels", ALLV)

Ph = P / np.linalg.norm(P, axis=1, keepdims=True); QP = Q @ Ph.T; aj = QP.argmax(1)
beta = QP[np.arange(NQ), aj].astype(np.float32); rhoq = np.sqrt(np.maximum(1 - beta ** 2, 0)).astype(np.float32)
pmA = (Ph[aj] @ (Mq / mn[:, None]).T).astype(np.float32)
qm = (Q @ Mq.T).astype(np.float32)

NL = len(ALLV)
ST = dict(enter=np.zeros((NL + 1, NT, NQ)), ib=np.zeros((NT, NQ)), cb=np.zeros((NT, NQ)), gt=np.zeros((NT, NQ)),
          viol=np.zeros(NT, np.int64))
A_ = np.zeros((NT, NQ)); IV = {"det": 0, "pre": 0, "rand": 0}; ESC = {"det": [0, 0], "ry": [0, 0], "rt": [0, 0]}; RMAX = 0.0

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
def dws(D, QV, MV):
    w2 = D.astype(np.float64) ** 2; QV = QV.astype(np.float64); MV = MV.astype(np.float64)
    Aqm = (MV * w2) @ QV.T; Bm = (MV * MV) @ w2; Cq = (QV * QV) @ w2
    al = Aqm / Bm[:, None]; dwa = np.sqrt(np.maximum(Cq[None] - Aqm ** 2 / Bm[:, None], 0))
    return al.astype(np.float32), dwa.astype(np.float32)

gstart = np.searchsorted(grp, np.arange(G + 1))
gsize = cstart[gstart[1:]] - cstart[gstart[:-1]]; load = np.zeros(NW); owner = np.zeros(G, int)
for g in np.argsort(-gsize, kind="stable"): owner[g] = int(load.argmin()); load[owner[g]] += gsize[g]
groups = [g for g in range(G) if owner[g] == WID]

def esc_code(u, cum, budget_bits, dep_at):
    dep = dep_at[(cum <= budget_bits[:, None]).sum(1)]
    return dep
OFF = np.array([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 18])     # 15 codes; the 16th is the escape
def relnorm(rho, pred, kmin):
    """4-bit code of rho relative to pred on the 2^(1/16) grid; returns (stored upper bound, escaped mask)"""
    k = np.ceil(np.log2(np.maximum(rho, 1e-30) / pred) * 16) - kmin
    esc = k > OFF[-1]
    kq = OFF[np.minimum(np.searchsorted(OFF, k), len(OFF) - 1)] + kmin
    val = pred * np.exp2(kq / 16)
    val = np.where(esc, roundup16(rho), val)
    return np.maximum(val, rho) * (1 + 1e-6), esc

for g in groups:
    ks = np.arange(gstart[g], gstart[g + 1])
    if len(ks) == 0: continue
    touch = np.zeros((NT, NQ), bool)
    lam, V = lamA[g], VA[g]; Qi, Qm = QiA[g], QmA[g]; Qd = Qi if PROF == "Qi" else Qm
    sig = np.sqrt(lam).astype(np.float32); lamsum = float(lam.sum())
    QV = (Q @ V).astype(np.float32); MV = (Mq[ks] @ V).astype(np.float32)
    se, dep_at, sdep, expc = esc_steps(lam, Qd, 2 * 8 * LD)
    s2 = np.where(sdep == 2)[0]
    DL = []                           # per det level: D, al, dwa, depth or bits (group level), normalised distortion per coordinate
    if DCODE == "fr":                 # nested greedy allocation of fixed bits per coordinate on sum sqrt(lam Q g_tree(b))
        a_ = np.sqrt(Qd * lam); sg = np.sqrt(GTREE); bcur = np.zeros(n, int); FRB = []
        gain = a_ * (sg[0] - sg[1]); used = 0
        for b in DLV:
            while used < 8 * b:
                i = int(gain.argmax())
                if gain[i] <= 0: break
                bcur[i] += 1; used += 1; gain[i] = a_[i] * (sg[bcur[i]] - sg[bcur[i] + 1]) if bcur[i] < 8 else -1
            FRB.append(bcur.copy())
    for b in DLV:
        de = dep_at[int((expc <= 8 * b).sum())] if b > 0 else np.zeros(n, int)
        D = (((lam * gE[de] + 1e-10) / (Qd + 1e-10)) ** 0.25).astype(np.float32); al, dwa = dws(D, QV, MV)
        if DCODE == "fr": de = FRB[DLV.index(b)]
        gv = (gE[de] if DCODE == "esc" else GTREE[de]).astype(np.float64)
        D = (((lam * gv + 1e-10) / (Qd + 1e-10)) ** 0.25).astype(np.float32); al, dwa = dws(D, QV, MV)
        DL.append((D, al, dwa, de, gv))
    # relative-norm offsets learned at build from this group's items (sample): kmin per level
    KMIN = [None] * len(DLV); KPOOL = [[] for _ in DLV]; YPOOL = [[] for _ in range(J)]; TPOOL = {}; YK0 = [0] * J
    # randomised design (as built)
    if NR:
        lres = lam * DL[-1][4]
        Dt = (((lres + 1e-10) / (Qm + 1e-10)) ** 0.25).astype(np.float32); alR, _ = dws(Dt, QV, MV)
        ai = np.sqrt(lres * Qm); order = np.argsort(-ai)
        idx = [order[BOUNDS[j]:BOUNDS[j + 1]] for j in range(J)]; Sj = np.array([ai[ix].sum() for ix in idx]); nb = np.array([len(ix) for ix in idx])
        grng = np.random.default_rng([20261112, g])
        Rs = []
        for j in range(J):
            q_, r_ = np.linalg.qr(grng.standard_normal((nb[j], nb[j]))); Rs.append((q_ * np.sign(np.diag(r_))).astype(np.float32))   # Haar
        QVt = (QV * Dt).astype(np.float32); MVt = (MV * Dt).astype(np.float32)
        QR = [QVt[:, idx[j]] @ Rs[j].T for j in range(J)]; MR = [MVt[:, idx[j]] @ Rs[j].T for j in range(J)]
        nD = []
        for j in range(J):
            a2 = (QVt[:, idx[j]].astype(np.float64) ** 2).sum(1); b2 = (MVt[:, idx[j]].astype(np.float64) ** 2).sum(1)
            ab = MVt[:, idx[j]].astype(np.float64) @ QVt[:, idx[j]].astype(np.float64).T
            nD.append(np.sqrt(np.maximum(a2[None] - 2 * alR * ab + alR.astype(np.float64) ** 2 * b2[:, None], 0)).astype(np.float32))
        rates = design(Sj, nb, [8 * (B - LD) for B in RLV], MENU, CSG)
        sc = []; prev = [0] * J; chg = []
        for li, r in enumerate(rates):
            ch = [j for j in range(J) if r[j] != prev[j]]; chg.append(ch)
            sc.append(((2 * J if RSC == "orig" else J) if li == 0 else 0) + (len(ch) if RSC == "orig" else 0.5 * len(ch))); prev = r
        ypredw = [np.sqrt(np.maximum((lres[ix] / Dt[ix] ** 2).sum(), 1e-30)) for ix in idx]    # predicted |y_b| at unit item scale
        TKMIN = {}
        if g == groups[0]: log("rates", rates, "scalar bytes per randomised level", sc, "blocks", nb.tolist())
    # build-time calibration of the relative scalar codes on a sample of this group's clusters; statistics of that pass are discarded
    import copy
    for PASS in ("cal", "run"):
     CAL = PASS == "cal"
     if CAL: SAVE = copy.deepcopy((ST, A_, IV, ESC)); touch0 = touch.copy()
     else:
        ST, A_, IV, ESC = SAVE; touch = touch0
        for l in range(len(DLV)):
            if KPOOL[l]: KMIN[l] = int(np.floor(np.quantile(np.concatenate(KPOOL[l]), 0.003)))
        for j in range(J):
            if YPOOL[j]: YK0[j] = int(np.floor(np.quantile(np.concatenate(YPOOL[j]), 0.002)))
        for key, v in TPOOL.items():
            kv = np.concatenate(v); lo_q, hi_q = np.quantile(kv, [0.002, 0.998]); TKMIN[key] = int(max(lo_q, hi_q - 14))
     cal_ks = set(ks[:: max(1, len(ks) // 8)].tolist())
     for ci, c in enumerate(ks):
        if CAL and c not in cal_ks: continue
        lo_, hi_ = int(cstart[c]), int(cstart[c + 1]); nc = hi_ - lo_
        if nc == 0: continue
        x = unit(Xs[lo_:hi_]); true = x @ Q.T
        for ti in range(NT): A_[ti] += (true >= thr[:, ti]).sum(0)
        z = (x - Mq[c]) @ V
        ah = ahat[lo_:hi_].astype(np.float32) / 32767; m_ = mn[c]; mr = (m_ * ah - m_ * m_)[:, None]
        R2 = (1 + m_ * m_ - 2 * m_ * (ah - DA)) * (1 + 1e-5) + 1e-4          # upper bound on |r|^2 = |z|^2 from the stored a
        s_it = np.sqrt(np.maximum(1 + m_ * m_ - 2 * m_ * ah, 1e-12) / lamsum).astype(np.float32)   # item scale, free
        u = z / sig / (s_it[:, None] if ISC else 1)
        scl = (s_it[:, None] if ISC else 1) * sig
        qmc = qm[:, c]; mvc = MV[ci]
        # ---- prefilter C1 + C2 (as built) ----
        alo = np.maximum(ah - DA, -1)[:, None]; ahi = np.minimum(ah + DA, 1)[:, None]
        cm = np.clip(qmc / m_, -1, 1); s1 = np.sqrt(np.maximum(1 - cm * cm, 0))
        aU = np.clip(cm[None], alo, ahi); aL = np.clip(-cm[None], alo, ahi)
        ub1 = cm * aU + s1 * np.sqrt(1 - aU * aU) + 3e-5; lb1 = cm * aL - s1 * np.sqrt(1 - aL * aL) - 3e-5
        hic, loc = HI[aj, c], LO[aj, c]; Bh = np.where(beta >= 0, beta * hic, beta * loc); Bl = np.where(beta >= 0, beta * loc, beta * hic)
        rm = cm - beta * pmA[:, c]; rp = np.sqrt(np.maximum(rhoq * rhoq - rm * rm, 0)); rr = np.maximum(rhoq, 1e-12)
        aU2 = np.clip((rm / rr)[None], alo, ahi); aL2 = np.clip((-rm / rr)[None], alo, ahi)
        ub2 = Bh + rm * aU2 + rp * np.sqrt(1 - aU2 * aU2) + 3e-5; lb2 = Bl + rm * aL2 - rp * np.sqrt(1 - aL2 * aL2) - 3e-5
        IV["pre"] += int(((true > ub1) | (true < lb1) | (true > ub2) | (true < lb2)).sum())
        ubc = Bh + rhoq + 3e-5; lbc = Bl - rhoq - 3e-5
        LB = np.maximum(lb1, lb2); UB = np.minimum(ub1, ub2); nlog = math.ceil(math.log2(nc + 1))
        for ti in range(NT):
            t = thr[:, ti]; al_ = ((LB < t) & (UB >= t)).any(0); nin = (LB >= t).any(0); nout = (UB < t).any(0)
            op = (ubc >= t) & (lbc < t); sr = op & (al_ | (nin & nout))
            ST["cb"][ti] += op * (n + 4) + sr * 4 * nlog * 2; touch[ti] |= al_
        li = 0
        def level(lb, ub, inc):
            global LB, UB, li
            for ti in range(NT):
                t = thr[:, ti]; cnt = ((LB < t) & (UB >= t)); ST["enter"][li, ti] += cnt.sum(0); ST["ib"][ti] += (inc * cnt).sum(0) if np.ndim(inc) else inc * cnt.sum(0)
            np.maximum(LB, lb, out=LB); np.minimum(UB, ub, out=UB); li += 1
        # ---- deterministic levels ----
        Cst = np.ones((nc, len(se)), np.int32); k2 = np.floor(np.abs(u) / W).astype(np.int32)
        Cst[:, s2] = k2[:, se[s2]] + 1; cum = np.cumsum(Cst, 1); del Cst
        escbits = np.zeros(nc); prevb = 0
        for l, b in enumerate(DLV):
            D, alA, dwaA, de_g, gv_g = DL[l]; al = alA[ci]; dwa = dwaA[ci]
            for attempt in (0, 1):
                if b == 0: dep = np.zeros_like(u, dtype=np.int64); code = np.zeros_like(z); gdep = np.ones(u.shape)
                elif DCODE == "fr":
                    dep = np.broadcast_to(de_g, u.shape); code = np.zeros_like(z)
                    for bb in np.unique(de_g[de_g > 0]):
                        cols = np.where(de_g == bb)[0]; bd, ce = TREE[int(bb)]; code[:, cols] = ce[np.searchsorted(bd, u[:, cols])]
                    code = code * scl; gdep = np.broadcast_to(gv_g, u.shape)
                else:
                    dep = dep_at[(cum <= (8 * b - escbits)[:, None]).sum(1)]; code = recon_esc(u, dep) * scl; gdep = gE[dep]
                e = z - code; rho = np.linalg.norm(e / D, axis=1)
                if NORM == 8: rhoh = roundup16(rho); esc = np.zeros(nc, bool); break
                # prediction from the decoded prefix: coded coordinates by depth, uncoded energy from R^2 - |code|^2
                iD2 = 1 / D.astype(np.float64) ** 2; s2it = (1 + m_ * m_ - 2 * m_ * ah) / lamsum
                ec = (s2it[:, None] * lam[None] * gdep) * (dep > 0)
                Uen = np.maximum((1 + m_ * m_ - 2 * m_ * ah) - (code.astype(np.float64) ** 2).sum(1) - ec.sum(1), 1e-12)
                lu = lam[None] * (dep == 0)
                pred = np.sqrt((ec * iD2).sum(1) + Uen * (lu * iD2).sum(1) / np.maximum(lu.sum(1), 1e-30))
                if CAL:
                    KPOOL[l].append(np.ceil(np.log2(rho / pred) * 16)); rhoh = roundup16(rho); esc = np.zeros(nc, bool); break
                rhoh, esc = relnorm(rho, pred, KMIN[l])
                if attempt == 0 and esc.any() and b > 0 and DCODE == "esc":
                    esc0 = esc.copy(); escbits = escbits + 8 * esc; continue
                break
            late = esc & ~esc0 if (NORM == 4 and b > 0 and attempt == 1) else np.zeros(nc, bool)
            if DCODE == "fr": late = esc.copy()                         # fixed rate: an escaped norm is one extra byte read
            escbits = escbits + 8 * late
            ESC["det"][0] += int(esc.sum()); ESC["det"][1] += nc
            if b == 0 and NORM == 4: escbits = escbits + 8 * esc           # a level-0 escape is paid from the first code level
            cq = code @ QV.T; cmv = (code @ mvc)[:, None]
            est = qmc[None] + cq + al[None] * (mr - cmv)
            h = dwa[None] * rhoh[:, None]
            up = h.copy(); dn = h.copy()
            if SPH:
                wv = (QV - al[:, None] * mvc[None]).astype(np.float64)          # (NQ, n)
                ia = 1 / D.astype(np.float64) ** 2; c64 = code.astype(np.float64); cn2 = (c64 ** 2).sum(1)
                for f in KAP:
                    kap = f * float(ia.mean()); tau = 1 / (4 * (ia + kap))
                    Aq = (wv * wv * tau).sum(1); Bi = c64 @ (wv * tau).T; Ci = (c64 * c64 * tau).sum(1)
                    Ei = 4 * kap * kap * Ci - kap * cn2 + rhoh.astype(np.float64) ** 2 + kap * R2
                    ok = Ei > 0; rt = 2 * np.sqrt(np.maximum(Aq[None] * Ei[:, None], 0))
                    upk = np.where(ok[:, None], rt - 4 * kap * Bi, np.inf); dnk = np.where(ok[:, None], rt + 4 * kap * Bi, np.inf)
                    up = np.minimum(up, upk.astype(np.float32)); dn = np.minimum(dn, dnk.astype(np.float32))
            sl = np.abs(al)[None] * m_ * DA
            lb = est - (dn + sl) * 1.0001 - 2e-5; ub = est + (up + sl) * 1.0001 + 2e-5
            IV["det"] += int(((true < lb) | (true > ub)).sum())
            nb_ = NORM / 8.0 + ((2.0 if l == 0 else 0.0))
            level(lb, ub, ((b - prevb) + nb_ + 1.0 * late)[:, None]); prevb = b
        code_d = code; cq_d = cq; cmv_d = cmv; rho_d = rhoh
        # ---- randomised levels on the residual of the deterministic prefix (as built; scalar coding switchable) ----
        if NR:
            alc = alR[ci]; base = qmc[None] + cq_d + alc[None] * (mr - cmv_d); slack = np.abs(alc)[None] * m_ * DA
            w = (z - code_d) / Dt
            Y = [w[:, idx[j]] @ Rs[j].T for j in range(J)]; yn = [np.linalg.norm(y, axis=1) for y in Y]
            if RSC == "orig": yh = [roundup_y(v) for v in yn]; yfb = [np.zeros(nc, bool) for _ in range(J)]
            else:
                yh = []; yfb = []
                for j in range(J):
                    pr = ypredw[j] * (np.sqrt(np.maximum(1 + m_ * m_ - 2 * m_ * ah, 1e-12) / lamsum))
                    k = np.ceil(np.log2(np.maximum(yn[j], 1e-30) / pr) * 256)
                    if CAL: YPOOL[j].append(k); yh.append(roundup_y(yn[j])); yfb.append(np.zeros(nc, bool)); continue
                    fb = (k - YK0[j] > 254) | (k < YK0[j])            # the reconstruction scale needs |y_b| within one grid step on both sides
                    yh.append(np.where(fb, roundup_y(yn[j]), pr * np.exp2(np.clip(k, YK0[j], YK0[j] + 254) / 256)) * (1 + 1e-7)); yfb.append(fb)
                    ESC["ry"][0] += int(fb.sum()); ESC["ry"][1] += nc
            extra = sum(2.0 * f for f in yfb) if RSC != "orig" else 0     # fallback: absolute 2 B norm read for that block
            EX = [Y[j] @ QR[j].T - alc[None] * (Y[j] @ MR[j][ci])[:, None] for j in range(J)]
            cache = {}
            for lr, r in enumerate(rates):
                est = base.copy(); hdet = np.zeros_like(true); hpar = np.zeros_like(true); Ab = []; mb = []
                textra = np.zeros(nc)
                for j in range(J):
                    if r[j] == 0: hdet += yh[j][:, None] * nD[j][ci][None]; continue
                    if (j, r[j]) not in cache:
                        y = Y[j]; nbj = nb[j]
                        if r[j] < 1:
                            kk = bits_of(r[j], nbj); cy = np.zeros_like(y); cy[:, :kk] = np.where(y[:, :kk] >= 0, 1.0, -1.0)
                        else:
                            bd, ce = TREE[int(r[j])]; cy = ce[np.searchsorted(bd, y / yh[j][:, None] * np.sqrt(nbj))]
                        dot = (cy * y).sum(1); cnr = np.linalg.norm(cy, axis=1)
                        cosf = np.clip(dot / (cnr * yn[j] + 1e-30), 1e-9, 1.0); tanf = np.sqrt(np.maximum(1 - cosf ** 2, 0)) / cosf
                        if RSC == "orig": th = roundup_t(tanf); tfb = np.zeros(nc, bool)
                        else:
                            kt = np.ceil(np.log2(np.maximum(tanf, 2.0 ** -12.8)) * 20)
                            if CAL: TPOOL.setdefault((j, r[j]), []).append(kt); TKMIN[(j, r[j])] = int(kt.max()) - 14
                            k0 = TKMIN[(j, r[j])]; tfb = (kt > k0 + 14) | (kt < k0)    # tan also needs one grid step on both sides
                            th = np.exp2(np.maximum(kt, k0) / 20); th = np.where(tfb, roundup_t(tanf), th)
                            ESC["rt"][0] += int(tfb.sum()); ESC["rt"][1] += nc
                        sh = yh[j] * np.sqrt(1 + th * th) / cnr
                        eb = sh[:, None] * (cy @ QR[j].T - alc[None] * (cy @ MR[j][ci])[:, None])
                        dmax = RY * np.sqrt(1 + th * th) / np.sqrt(1 + th * th / RHO_T ** 2) - 1
                        dmax = np.where(yfb[j], RHO_Y * np.sqrt(1 + th * th) / np.sqrt(1 + th * th / RHO_T ** 2) - 1, dmax)
                        cache[(j, r[j])] = (eb, dmax, yh[j] * th, tfb)
                    eb, dmax, ah_, tfb = cache[(j, r[j])]
                    if j in chg[lr]: textra += 1.0 * tfb                       # fallback: absolute tan byte for that block
                    est += eb; hpar += dmax[:, None] * np.abs(eb); Ab.append(ah_[:, None] * nD[j][ci][None]); mb.append(MS[j])
                if Ab:
                    Ab = np.stack(Ab, -1); mb = np.array(mb); sd = np.sqrt((Ab * Ab / mb).sum(-1)); hsg = CSG * sd; hex_ = hsg.copy()
                    mask = np.zeros(true.shape, bool)
                    for ti in range(NT): t = thr[:, ti]; mask |= (LB < t) & (UB >= t)
                    if mask.any():
                        hex_[mask] = np.minimum(chernoff_halfwidth(Ab[mask].astype(np.float64), mb, [TABS[int(m)] for m in mb], L1), hsg[mask])
                else: hex_ = np.zeros_like(true)
                h = (hex_ + hdet + hpar + slack) * 1.0001 + 2e-5
                IV["rand"] += int((np.abs(true - est) > h).sum())
                inc = RLV[lr] - (RLV[lr - 1] if lr else LD) + sc[lr] + ((extra if lr == 0 else 0) + textra)
                level(est - h, est + h, inc[:, None] if np.ndim(inc) else inc)
        for ti in range(NT):
            t = thr[:, ti]; cnt = ((LB < t) & (UB >= t)); ST["enter"][li, ti] += cnt.sum(0); ST["ib"][ti] += FULL * cnt.sum(0)
            Aa = true >= t; ST["viol"][ti] += int((Aa & (UB < t)).sum() + (~Aa & (LB >= t)).sum())
    ST["gt"] += touch
    log("group", g, "viol", int(ST["viol"].sum()), "IV", IV, "esc", ESC, "rmax", round(RMAX, 2))
os.makedirs(VEC + "quant/out", exist_ok=True)
np.savez(VEC + f"quant/out/{NAME}_{TAG}_w{WID}.npz", A=A_, ALLV=np.array(ALLV), DLV=np.array(DLV), IV=json.dumps(IV), ESC=json.dumps(ESC),
         RMAX=RMAX, CFG=json.dumps(dict(DCODE=DCODE, PROF=PROF, SPH=SPH, NORM=NORM, L0=L0, SCHED=SCHED, RSC=RSC, ISC=ISC, RLV=RLV, WESC=E("WESC", ""))), **ST)
log("done")
