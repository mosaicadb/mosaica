# One pass over the items of a share of the groups; every variant and accounting track of the ablation is evaluated on the
# same items, queries and thresholds. usage: python3 run.py NAME WORKER NWORKERS   (env LD = last deterministic level, bytes)
import numpy as np, sys, os, math, json
from core import *
ROOT = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/combined/"
NAME, WID, NW = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
LD = int(os.environ.get("LD", 32)); TAG = os.environ.get("TAG", "")
I = dict(np.load(ROOT + f"idx_{NAME}/index.npz")); Xs = np.load(ROOT + f"idx_{NAME}/Xs.npy", mmap_mode="r")
N, n, K, G = int(I["N"]), int(I["n"]), int(I["K"]), int(I["G"])
assign, cstart, grp, Mq, mn, ahat = I["assign"], I["cstart"], I["grp"], I["Mq"], I["mn"], I["ahat"]
lamA, VA, QiA, QmA = I["lam"], I["V"], I["Qi"], I["Qmix"]
Q, qcls, thr, P, nimg, HI, LO = I["Q"], I["qcls"], I["thr"], I["P"], int(I["nimg"]), I["hi"], I["lo"]
NQ, NT = len(Q), thr.shape[1]; FULL = 2 * n
LV = [b for b in (0, 4, 8, 16, 32, 48, 96, 192, 384, 576) if b <= 0.4 * FULL]
LD = max(b for b in LV if b <= LD); ld = LV.index(LD); RLV = [b for b in LV if b > LD]; NR = len(RLV)
BOUNDS = [0] + [b for b in (96, 224, 480, 992, 2016, 4064) if b <= n - 64] + [n]; J = len(BOUNDS) - 1
EPS, NTARGET = 1e-9, 1e9
L1 = math.log(2 * NTARGET * max(NR, 1) / EPS)            # one-sided exp(-L1) = delta/2, delta = eps / (N * levels)
CSG = math.sqrt(2 * L1)
DA = 0.5 / 32767 + 1e-7                                    # rounding of the stored a
RATES = {"int": [0, 1, 2, 3, 4, 5, 6, 7, 8], "sub": [0, 0.125, 0.25, 0.5, 1, 2, 3, 4, 5, 6, 7, 8]}
def Tof(r): return np.inf if r == 0 else (np.pi / (2 * r) - 1 if r < 1 else GTREE[int(r)] / (1 - GTREE[int(r)]))
def bits_of(r, nb): return int(r * nb)
MS = np.array([BOUNDS[j + 1] - BOUNDS[j] - 1 for j in range(J)], float)
TABS = {int(m): LogMGF(int(m)) for m in set(MS.tolist())} if NR else {}
log(NAME, "worker", WID, "LV", LV, "LD", LD, "random levels", RLV, "blocks", BOUNDS, "c(sub-Gaussian)", round(CSG, 3))

# ---------- query-side constants ----------
Ph = P / np.linalg.norm(P, axis=1, keepdims=True)
QP = Q @ Ph.T
anch = {"all": QP.argmax(1), "img": QP[:, :nimg].argmax(1)}
ANC = {}
for k, j in anch.items():
    beta = QP[np.arange(NQ), j]; rho = np.sqrt(np.maximum(1 - beta ** 2, 0))
    ANC[k] = (j, beta.astype(np.float32), rho.astype(np.float32), (Ph[j] @ (Mq / mn[:, None]).T).astype(np.float32))  # last: p^.m^ (NQ x K)
qm = (Q @ Mq.T).astype(np.float32)

# ---------- accounting tracks ----------
class Track:
    def __init__(s, nm, nlev):
        s.nm = nm; s.enter = np.zeros((nlev + 1, NT, NQ)); s.ib = np.zeros((NT, NQ)); s.cb = np.zeros((NT, NQ))
        s.gt = np.zeros((NT, NQ)); s.viol = np.zeros(NT, np.int64); s.li = 0; s.cent = np.zeros((NT, NQ)); s.srch = np.zeros((NT, NQ))
    def start(s, nc, pre=None, opened=None):
        s.li = 0
        if pre is None:
            s.LB = np.full((nc, NQ), -np.inf, np.float32); s.UB = np.full((nc, NQ), np.inf, np.float32)
            s.cb += n + 4; s.cent += 1; s.touch |= True; return
        s.LB, s.UB = pre[0].copy(), pre[1].copy(); nlog = math.ceil(math.log2(nc + 1))
        for ti in range(NT):
            t = thr[:, ti]; al = ((s.LB < t) & (s.UB >= t)).any(0); nin = (s.LB >= t).any(0); nout = (s.UB < t).any(0)
            op = opened[ti] if opened is not None else np.ones(NQ, bool)
            sr = op & (al | (nin & nout))
            s.cb[ti] += op * (n + 4) + sr * 4 * nlog * 2; s.cent[ti] += op; s.srch[ti] += sr; s.touch[ti] |= al
    def level(s, lb, ub, inc):
        for ti in range(NT):
            t = thr[:, ti]; c = ((s.LB < t) & (s.UB >= t)).sum(0); s.enter[s.li, ti] += c; s.ib[ti] += inc * c
        np.maximum(s.LB, lb, out=s.LB); np.minimum(s.UB, ub, out=s.UB); s.li += 1
    def alive_any(s):
        m = np.zeros(s.LB.shape, bool)
        for ti in range(NT): t = thr[:, ti]; m |= (s.LB < t) & (s.UB >= t)
        return m
    def finish(s, true):
        for ti in range(NT):
            t = thr[:, ti]; c = ((s.LB < t) & (s.UB >= t)).sum(0); s.enter[s.li, ti] += c; s.ib[ti] += FULL * c
            A = true >= t; s.viol[ti] += int((A & (s.UB < t)).sum() + (~A & (s.LB >= t)).sum())
    def gstart(s): s.touch = np.zeros((NT, NQ), bool)
    def gend(s): s.gt += s.touch

ND = len(LV)
TR = {nm: Track(nm, ND if nm in ("V0", "V1", "V2", "V2+C1+C2") else ld + 1 + NR) for nm in
      ("V0", "V1", "V2", "V3int", "V3int-sg", "V3sub", "V3sub-sg", "V3sub+C1", "V3sub+C1+C2", "V3sub+C1+C2img", "V2+C1+C2")}
if NR == 0:
    for k in ("V3int", "V3int-sg", "V3sub", "V3sub-sg", "V3sub+C1", "V3sub+C1+C2", "V3sub+C1+C2img"): del TR[k]
    TR["V2+C1"] = Track("V2+C1", ND); TR["V2+C1+C2img"] = Track("V2+C1+C2img", ND)
A_ = np.zeros((NT, NQ)); IV = {"det": 0, "pre": 0, "rand": 0}; RSTAT = {k: [0.0, 0, 0, 0, 0] for k in RATES}  # max ratio, pairs, >4, >5, >6 sd
RMAX_CLS = {k: {c: 0.0 for c in np.unique(qcls)} for k in RATES}; SCAL = {}

def dws(D, QV, MV):
    w2 = (D.astype(np.float64)) ** 2; QV = QV.astype(np.float64); MV = MV.astype(np.float64)
    Aqm = (MV * w2) @ QV.T; Bm = (MV * MV) @ w2; Cq = (QV * QV) @ w2
    dw = np.sqrt(np.maximum(Cq[None] - 2 * Aqm + Bm[:, None], 0)); al = Aqm / Bm[:, None]
    dwa = np.sqrt(np.maximum(Cq[None] - Aqm ** 2 / Bm[:, None], 0))
    return dw.astype(np.float32), al.astype(np.float32), dwa.astype(np.float32)

def design(S, nb, budgets, menu, c):
    """nested rate choice per block and level, greedy on the predicted split bound"""
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

gstart = np.searchsorted(grp, np.arange(G + 1))         # clusters of group g: gstart[g]..gstart[g+1]
gsize = cstart[gstart[1:]] - cstart[gstart[:-1]]; load = np.zeros(NW); owner = np.zeros(G, int)
for g in np.argsort(-gsize, kind="stable"): owner[g] = int(load.argmin()); load[owner[g]] += gsize[g]
groups = [g for g in range(G) if owner[g] == WID]
for g in groups:
    ks = np.arange(gstart[g], gstart[g + 1])
    if len(ks) == 0: continue
    for tr in TR.values(): tr.gstart()
    lam, V, Qi, Qm = lamA[g], VA[g], QiA[g], QmA[g]; sig = np.sqrt(lam).astype(np.float32)
    QV = (Q @ V).astype(np.float32); MV = (Mq[ks] @ V).astype(np.float32)
    D0 = (((lam + 1e-10) / (Qi + 1e-10)) ** 0.25).astype(np.float32); dw0, al0, dwa0 = dws(D0, QV, MV)
    uni = []
    for l in range(1, ND):
        bits = alloc_bound(lam, Qi, 8 * LV[l]); D = (((lam * GT[bits] + 1e-10) / (Qi + 1e-10)) ** 0.25).astype(np.float32)
        uni.append((bits, D, dws(D, QV, MV)[0]))
    se, dep_at, sdep, expc = esc_steps(lam, Qi, 1.5 * 8 * LV[-1])
    s2 = np.where(sdep == 2)[0]
    esc = []
    for l in range(1, ND):
        de = dep_at[int((expc <= 8 * LV[l]).sum())]; D = (((lam * gE[de] + 1e-10) / (Qi + 1e-10)) ** 0.25).astype(np.float32)
        esc.append((D,) + dws(D, QV, MV) + (de,))
    RC = {}
    if NR:
        de = esc[ld - 1][4] if ld > 0 else np.zeros(n, int); lres = lam * gE[de]
        Dt = (((lres + 1e-10) / (Qm + 1e-10)) ** 0.25).astype(np.float32); _, alR, _ = dws(Dt, QV, MV)
        ai = np.sqrt(lres * Qm); order = np.argsort(-ai)
        idx = [order[BOUNDS[j]:BOUNDS[j + 1]] for j in range(J)]; Sj = np.array([ai[ix].sum() for ix in idx]); nb = np.array([len(ix) for ix in idx])
        grng = np.random.default_rng([20261112, g])
        Rs = [np.linalg.qr(grng.standard_normal((nb[j], nb[j])))[0].astype(np.float32) for j in range(J)]
        QVt = (QV * Dt).astype(np.float32); MVt = (MV * Dt).astype(np.float32)
        QR = [QVt[:, idx[j]] @ Rs[j].T for j in range(J)]; MR = [MVt[:, idx[j]] @ Rs[j].T for j in range(J)]
        nD = []
        for j in range(J):
            a2 = (QVt[:, idx[j]].astype(np.float64) ** 2).sum(1); b2 = (MVt[:, idx[j]].astype(np.float64) ** 2).sum(1)
            ab = MVt[:, idx[j]].astype(np.float64) @ QVt[:, idx[j]].astype(np.float64).T
            nD.append(np.sqrt(np.maximum(a2[None] - 2 * alR * ab + alR.astype(np.float64) ** 2 * b2[:, None], 0)).astype(np.float32))
        for k, menu in RATES.items():
            rates = design(Sj, nb, [8 * (B - LD) for B in RLV], menu, CSG)
            sc = []; prev = [0] * J
            for li, r in enumerate(rates):
                ch = [j for j in range(J) if r[j] != prev[j]]; sc.append((2 * J if li == 0 else 0) + len(ch)); prev = r
            RC[k] = (rates, sc); SCAL.setdefault(k, []).append(sc)
            if g == groups[0]: log(k, "rates per level", rates, "scalar bytes per level", sc, "block dims", nb.tolist())
    for ci, c in enumerate(ks):
        lo_, hi_ = int(cstart[c]), int(cstart[c + 1]); nc = hi_ - lo_
        if nc == 0: continue
        x = unit(Xs[lo_:hi_]); true = x @ Q.T
        for ti in range(NT): A_[ti] += (true >= thr[:, ti]).sum(0)
        z = (x - Mq[c]) @ V; u = z / sig
        ah = ahat[lo_:hi_].astype(np.float32) / 32767; m_ = mn[c]; mr = (m_ * ah - m_ * m_)[:, None]
        qmc = qm[:, c]; mvc = MV[ci]
        # ---- prefilter: annulus (C1) and support table (C2) bounds as functions of the stored a ----
        alo = np.maximum(ah - DA, -1)[:, None]; ahi = np.minimum(ah + DA, 1)[:, None]
        cm = np.clip(qmc / m_, -1, 1); s1 = np.sqrt(np.maximum(1 - cm * cm, 0))
        aU = np.clip(cm[None], alo, ahi); aL = np.clip(-cm[None], alo, ahi)
        ub1 = cm * aU + s1 * np.sqrt(1 - aU * aU) + 3e-5; lb1 = cm * aL - s1 * np.sqrt(1 - aL * aL) - 3e-5
        IV["pre"] += int(((true > ub1) | (true < lb1)).sum())
        pre = {"C1": (lb1, ub1, None)}
        for k, (j, beta, rho, pm) in ANC.items():
            hic, loc = HI[j, c], LO[j, c]; Bh = np.where(beta >= 0, beta * hic, beta * loc); Bl = np.where(beta >= 0, beta * loc, beta * hic)
            rm = cm - beta * pm[:, c]; rp = np.sqrt(np.maximum(rho * rho - rm * rm, 0)); rr = np.maximum(rho, 1e-12)
            aU2 = np.clip((rm / rr)[None], alo, ahi); aL2 = np.clip((-rm / rr)[None], alo, ahi)
            ub2 = Bh + rm * aU2 + rp * np.sqrt(1 - aU2 * aU2) + 3e-5; lb2 = Bl + rm * aL2 - rp * np.sqrt(1 - aL2 * aL2) - 3e-5
            IV["pre"] += int(((true > ub2) | (true < lb2)).sum())
            ubc = Bh + rho + 3e-5; lbc = Bl - rho - 3e-5
            opened = [(ubc >= thr[:, ti]) & (lbc < thr[:, ti]) for ti in range(NT)]
            pre["C1+C2" + ("" if k == "all" else "img")] = (np.maximum(lb1, lb2), np.minimum(ub1, ub2), opened)
        PRE = {"V3sub+C1": "C1", "V3sub+C1+C2": "C1+C2", "V3sub+C1+C2img": "C1+C2img", "V2+C1+C2": "C1+C2", "V2+C1": "C1", "V2+C1+C2img": "C1+C2img"}
        for nm, tr in TR.items():
            if nm in PRE: p = pre[PRE[nm]]; tr.start(nc, (p[0], p[1]), p[2])
            else: tr.start(nc)
        # ---- level 0 ----
        ew = roundup16(np.linalg.norm(z / D0, axis=1))[:, None]
        h = (dw0[ci] * ew + m_ * DA) * 1.0001 + 2e-5; est = qmc + mr
        IV["det"] += int((np.abs(true - est) > h).sum())
        for nm in ("V0", "V1"): TR[nm].level(est - h, est + h, 3)
        h = (dwa0[ci] * ew + np.abs(al0[ci]) * m_ * DA) * 1.0001 + 2e-5; est = qmc + al0[ci] * mr
        IV["det"] += int((np.abs(true - est) > h).sum())
        for nm, tr in TR.items():
            if nm not in ("V0", "V1"): tr.level(est - h, est + h, 3)
        # ---- deterministic levels ----
        Cst = np.ones((nc, len(se)), np.int32); k2 = np.floor(np.abs(u) / W).astype(np.int32)
        Cst[:, s2] = k2[:, se[s2]] + 1; cum = np.cumsum(Cst, 1); del Cst
        for l in range(1, ND):
            inc = LV[l] - LV[l - 1]
            bits, D, dw = uni[l - 1]
            code = quant_uni(u, bits) * sig; cn2 = (code * code).sum(1) + 1e-30
            gm = np.clip(np.round((z * code).sum(1) / cn2 * 128), 0, 255) / 128
            e = z - gm[:, None] * code; ew = roundup16(np.linalg.norm(e / D, axis=1))[:, None]
            est = qmc + mr + gm[:, None] * (code @ QV.T - (code @ mvc)[:, None]); h = (dw[ci] * ew + m_ * DA) * 1.0001 + 2e-5
            IV["det"] += int((np.abs(true - est) > h).sum()); TR["V0"].level(est - h, est + h, inc + 2)
            D, dwn, al, dwa, _ = esc[l - 1]
            dep = dep_at[(cum <= 8 * LV[l]).sum(1)]
            code = recon_esc(u, dep) * sig; e = z - code; ew = roundup16(np.linalg.norm(e / D, axis=1))[:, None]
            cq = code @ QV.T; cmv = (code @ mvc)[:, None]
            est = qmc + mr + cq - cmv; h = (dwn[ci] * ew + m_ * DA) * 1.0001 + 2e-5
            IV["det"] += int((np.abs(true - est) > h).sum()); TR["V1"].level(est - h, est + h, inc + 1)
            est = qmc + cq + al[ci] * (mr - cmv); h = (dwa[ci] * ew + np.abs(al[ci]) * m_ * DA) * 1.0001 + 2e-5
            IV["det"] += int((np.abs(true - est) > h).sum())
            for nm, tr in TR.items():
                if nm in ("V2", "V2+C1+C2", "V2+C1", "V2+C1+C2img") or (l <= ld and nm not in ("V0", "V1")): tr.level(est - h, est + h, inc + 1)
            if l == ld: code_d, cq_d, cmv_d = code, cq, cmv
        if ld == 0: code_d = np.zeros_like(z); cq_d = np.zeros_like(true); cmv_d = np.zeros((nc, 1), np.float32)
        # ---- randomised levels on the residual of the deterministic prefix ----
        if NR:
            alc = alR[ci]; base = qmc + cq_d + alc * (mr - cmv_d); slack = np.abs(alc) * m_ * DA
            w = (z - code_d) / Dt
            Y = [w[:, idx[j]] @ Rs[j].T for j in range(J)]; yn = [np.linalg.norm(y, axis=1) for y in Y]; yh = [roundup_y(v) for v in yn]
            EX = [Y[j] @ QR[j].T - alc * (Y[j] @ MR[j][ci])[:, None] for j in range(J)]
            for k, (rates, sc) in RC.items():
                tnames = [nm for nm in TR if nm.startswith("V3" + k)]
                cache = {}
                for li, r in enumerate(rates):
                    est = base.copy(); hdet = np.zeros_like(true); hpar = np.zeros_like(true); Ab = []; mb = []; Pobs = np.zeros_like(true)
                    for j in range(J):
                        if r[j] == 0: hdet += yh[j][:, None] * nD[j][ci]; continue
                        if (j, r[j]) not in cache:
                            y = Y[j]; nbj = nb[j]
                            if r[j] < 1:
                                kk = bits_of(r[j], nbj); cy = np.zeros_like(y); cy[:, :kk] = np.where(y[:, :kk] >= 0, 1.0, -1.0)
                            else:
                                bd, ce = TREE[int(r[j])]; cy = ce[np.searchsorted(bd, y / yh[j][:, None] * np.sqrt(nbj))]
                            dot = (cy * y).sum(1); cnr = np.linalg.norm(cy, axis=1)
                            cosf = np.clip(dot / (cnr * yn[j] + 1e-30), 1e-9, 1.0); tanf = np.sqrt(np.maximum(1 - cosf ** 2, 0)) / cosf
                            th = roundup_t(tanf); sh = yh[j] * np.sqrt(1 + th * th) / cnr
                            eb = sh[:, None] * (cy @ QR[j].T - alc * (cy @ MR[j][ci])[:, None])
                            dmax = RHO_Y * np.sqrt(1 + th * th) / np.sqrt(1 + th * th / RHO_T ** 2) - 1
                            cache[(j, r[j])] = (eb, dmax, yh[j] * th)
                        eb, dmax, ah_ = cache[(j, r[j])]
                        est += eb; hpar += dmax[:, None] * np.abs(eb); Ab.append(ah_[:, None] * nD[j][ci]); mb.append(MS[j]); Pobs += eb - EX[j]
                    if Ab:
                        Ab = np.stack(Ab, -1); mb = np.array(mb); sd = np.sqrt((Ab * Ab / mb).sum(-1))
                        ratio = np.abs(Pobs) / np.maximum(sd, 1e-30); st = RSTAT[k]
                        st[0] = max(st[0], float(ratio.max())); st[1] += ratio.size; st[2] += int((ratio > 4).sum()); st[3] += int((ratio > 5).sum()); st[4] += int((ratio > 6).sum())
                        for cl in RMAX_CLS[k]: RMAX_CLS[k][cl] = max(RMAX_CLS[k][cl], float(ratio[:, qcls == cl].max()))
                        hsg = CSG * sd; hex_ = hsg.copy()
                        mask = np.zeros(true.shape, bool)
                        for nm in tnames:
                            if not nm.endswith("-sg"): mask |= TR[nm].alive_any()
                        if mask.any():
                            hex_[mask] = np.minimum(chernoff_halfwidth(Ab[mask].astype(np.float64), mb, [TABS[int(m)] for m in mb], L1), hsg[mask])
                    else: hsg = hex_ = np.zeros_like(true)
                    for nm in tnames:
                        hr = hsg if nm.endswith("-sg") else hex_
                        h = (hr + hdet + hpar + slack) * 1.0001 + 2e-5
                        IV["rand"] += int((np.abs(true - est) > h).sum())
                        TR[nm].level(est - h, est + h, RLV[li] - (RLV[li - 1] if li else LD) + sc[li])
        for tr in TR.values(): tr.finish(true)
    for tr in TR.values(): tr.gend()
    log("group", g, "clusters", len(ks), "violations: decisions", {nm: int(tr.viol.sum()) for nm, tr in TR.items() if tr.viol.sum()},
        "intervals", IV, "rand max sd", {k: round(v[0], 2) for k, v in RSTAT.items()})
np.savez(ROOT + f"out_{NAME}{TAG}_w{WID}.npz", A=A_, LV=np.array(LV), LD=LD, RLV=np.array(RLV), groups=np.array(groups),
         IV=json.dumps(IV), RSTAT=json.dumps(RSTAT), RMAX=json.dumps(RMAX_CLS), SCAL=json.dumps(SCAL), CSG=CSG, L1=L1,
         **{f"{nm}|{f}": getattr(tr, f) for nm, tr in TR.items() for f in ("enter", "ib", "cb", "gt", "viol", "cent", "srch")})
log("done")
