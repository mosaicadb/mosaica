# Stage 2 of exact k-NN: the threshold sweep at the seed threshold tau1 (and at the oracle threshold), per column
# (k, filter, seed|oracle), with visibility masks applied before anything is read. Level construction as combined/run.py.
# Records the interval ladder of every (item, query) pair with score >= its column threshold, for stage 3.
# usage: python3 sim.py NAME WORKER NWORKERS
import numpy as np, sys, os, math, json
from kcommon import *
NAME, WID, NW = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]); C = Ctx(NAME); W_ = workdir(NAME)
N, n, K, G, NQ, Q, Xs, cstart, FULL = C.N, C.n, C.K, C.G, C.NQ, C.Q, C.Xs, C.cstart, C.FULL
S = np.load(W_ + "S.npy", mmap_mode="r"); M = np.load(W_ + "masks.npy"); thr = np.load(W_ + "thr.npy"); colf = np.load(W_ + "colf.npy")
NT = thr.shape[1]; LV, ND, ld, LD, RLV, NR, J, DA, MS, L1, CSG = C.LV, C.ND, C.ld, C.LD, C.RLV, C.NR, C.J, C.DA, C.MS, C.L1, C.CSG
NF = M.shape[0]; cntv = np.stack([np.add.reduceat(M[f].astype(np.int64), cstart[:-1]) for f in range(NF)])
thrmin = np.full((NF, NQ), np.inf, np.float32)
for ti in range(NT): thrmin[colf[ti]] = np.minimum(thrmin[colf[ti]], thr[:, ti])
log(NAME, "worker", WID, "NT", NT, "LV", LV, "random levels", RLV)

class Track:
    def __init__(s, nlev):
        s.enter = np.zeros((nlev + 1, NT, NQ)); s.ib = np.zeros((NT, NQ)); s.cb = np.zeros((NT, NQ)); s.gt = np.zeros((G, NT, NQ), bool)
        s.viol = np.zeros(NT, np.int64); s.cent = np.zeros((NT, NQ)); s.srch = np.zeros((NT, NQ)); s.lad = []
    def und(s, ti): t = thr[:, ti]; return (s.LB < t) & (s.UB >= t) & VIS[ti]
    def rec(s): s.cur[0].append(s.LB[PR, PQ].copy()); s.cur[1].append(s.UB[PR, PQ].copy())
    def start(s, nc, pre, opened, g):
        s.li = 0; s.LB, s.UB = pre[0].copy(), pre[1].copy(); nlog = math.ceil(math.log2(nc + 1)); s.cur = ([], []); s.rec()
        for ti in range(NT):
            t = thr[:, ti]; al = s.und(ti).any(0); nin = ((s.LB >= t) & VIS[ti]).any(0); nout = ((s.UB < t) & VIS[ti]).any(0)
            op = opened[ti]; sr = op & (al | (nin & nout))
            s.cb[ti] += op * (n + 4) + sr * 4 * nlog * 2; s.cent[ti] += op; s.srch[ti] += sr; s.gt[g, ti] |= al
    def level(s, lb, ub, inc):
        for ti in range(NT): c = s.und(ti).sum(0); s.enter[s.li, ti] += c; s.ib[ti] += inc * c
        np.maximum(s.LB, lb, out=s.LB); np.minimum(s.UB, ub, out=s.UB); s.li += 1; s.rec()
    def alive_any(s):
        m = np.zeros(s.LB.shape, bool)
        for ti in range(NT): m |= s.und(ti)
        return m
    def finish(s, true, lo_):
        for ti in range(NT):
            t = thr[:, ti]; c = s.und(ti).sum(0); s.enter[s.li, ti] += c; s.ib[ti] += FULL * c
            A = true >= t; s.viol[ti] += int((((A & (s.UB < t)) | (~A & (s.LB >= t))) & VIS[ti]).sum())
        if len(PR): s.lad.append((PR + lo_, PQ, np.array(s.cur[0]), np.array(s.cur[1])))

TR = {"R": Track(ld + 1 + NR), "D": Track(ND)}
IV = {"det": 0, "pre": 0, "rand": 0}; RSTAT = [0.0, 0, 0, 0, 0]; SC = []; WHOLEIN = np.zeros(NT, np.int64)
gsize = cstart[C.gstart[1:]] - cstart[C.gstart[:-1]]; load = np.zeros(NW); owner = np.zeros(G, int)
for g in np.argsort(-gsize, kind="stable"): owner[g] = int(load.argmin()); load[owner[g]] += gsize[g]
for g in [g for g in range(G) if owner[g] == WID]:
    if C.gstart[g + 1] == C.gstart[g]: continue
    gs = gsetup(C, g, Q); ks, QV, MV, sig, V = gs.ks, gs.QV, gs.MV, gs.sig, gs.V; SC.append(gs.sc if gs.rand else [])
    for ci, c in enumerate(ks):
        lo_, hi_ = int(cstart[c]), int(cstart[c + 1]); nc = hi_ - lo_
        if nc == 0: continue
        x = unit(Xs[lo_:hi_]); true = np.asarray(S[lo_:hi_])
        excl = (np.arange(lo_, hi_)[:, None] == C.qrow[None])
        VISF = M[:, lo_:hi_, None] & ~excl[None]; VIS = VISF[colf]
        pm_ = np.zeros((nc, NQ), bool)
        for f in range(NF): pm_ |= (true >= thrmin[f]) & VISF[f]
        PR, PQ = np.where(pm_)
        z = (x - C.Mq[c]) @ V; u = z / sig
        ah = C.ahat[lo_:hi_].astype(np.float32) / 32767; m_ = C.mn[c]; mr = (m_ * ah - m_ * m_)[:, None]; qmc = C.qm[:, c]; mvc = MV[ci]
        lb, ub, (lb1, ub1, lb2, ub2) = prefilter(C, c, ah)
        IV["pre"] += int(((true > ub1) | (true < lb1)).sum() + ((true > ub2) | (true < lb2)).sum())
        hasv = cntv[colf, c] > 0
        opened = [(C.ubc[:, c] >= thr[:, ti]) & (C.lbc[:, c] < thr[:, ti]) & hasv[ti] for ti in range(NT)]
        for ti in range(NT): WHOLEIN[ti] += int(((C.lbc[:, c] >= thr[:, ti]) & hasv[ti]).sum())
        for tr in TR.values(): tr.start(nc, (lb, ub), opened, g)
        # level 0
        ew = roundup16(np.linalg.norm(z / gs.D0, axis=1))[:, None]
        h = (gs.dwa0[ci] * ew + np.abs(gs.al0[ci]) * m_ * DA) * 1.0001 + 2e-5; est = qmc + gs.al0[ci] * mr
        IV["det"] += int((np.abs(true - est) > h).sum())
        for tr in TR.values(): tr.level(est - h, est + h, 3)
        # deterministic levels (escape code, alpha*)
        Cst = np.ones((nc, len(gs.se)), np.int32); k2 = np.floor(np.abs(u) / W).astype(np.int32)
        Cst[:, gs.s2] = k2[:, gs.se[gs.s2]] + 1; cum = np.cumsum(Cst, 1); del Cst
        for l in range(1, ND):
            if l > ld and NR and len(PR) == 0 and not TR["D"].alive_any().any():
                TR["D"].level(TR["D"].LB, TR["D"].UB, 0); continue
            inc = LV[l] - LV[l - 1]; D, dwn, al, dwa, _ = gs.esc[l - 1]
            dep = gs.dep_at[(cum <= 8 * LV[l]).sum(1)]
            code = recon_esc(u, dep) * sig; e = z - code; ew = roundup16(np.linalg.norm(e / D, axis=1))[:, None]
            cq = code @ QV.T; cmv = (code @ mvc)[:, None]
            est = qmc + cq + al[ci] * (mr - cmv); h = (dwa[ci] * ew + np.abs(al[ci]) * m_ * DA) * 1.0001 + 2e-5
            IV["det"] += int((np.abs(true - est) > h).sum())
            TR["D"].level(est - h, est + h, inc + 1)
            if l <= ld: TR["R"].level(est - h, est + h, inc + 1)
            if l == ld: code_d, cq_d, cmv_d = code, cq, cmv
        # randomised levels on the residual of the 32-byte prefix
        if gs.rand:
            alc = gs.alR[ci]; base = qmc + cq_d + alc * (mr - cmv_d); slack = np.abs(alc) * m_ * DA
            w = (z - code_d) / gs.Dt; idx, Rs, nb, QR, MR, nD = gs.idx, gs.Rs, gs.nb, gs.QR, gs.MR, gs.nD
            Y = [w[:, idx[j]] @ Rs[j].T for j in range(J)]; yn = [np.linalg.norm(y, axis=1) for y in Y]; yh = [roundup_y(v) for v in yn]
            EX = [Y[j] @ QR[j].T - alc * (Y[j] @ MR[j][ci])[:, None] for j in range(J)]; cache = {}
            for li, r in enumerate(gs.rates):
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
                    ratio = np.abs(Pobs) / np.maximum(sd, 1e-30)
                    RSTAT[0] = max(RSTAT[0], float(ratio.max())); RSTAT[1] += ratio.size
                    RSTAT[2] += int((ratio > 4).sum()); RSTAT[3] += int((ratio > 5).sum()); RSTAT[4] += int((ratio > 6).sum())
                    hex_ = CSG * sd; mask = TR["R"].alive_any()
                    if mask.any():
                        hex_[mask] = np.minimum(chernoff_halfwidth(Ab[mask].astype(np.float64), mb, [C.TABS[int(m)] for m in mb], L1), hex_[mask])
                else: hex_ = np.zeros_like(true)
                h = (hex_ + hdet + hpar + slack) * 1.0001 + 2e-5
                IV["rand"] += int((np.abs(true - est) > h).sum())
                TR["R"].level(est - h, est + h, RLV[li] - (RLV[li - 1] if li else LD) + gs.sc[li])
        for tr in TR.values(): tr.finish(true, lo_)
    log("group", g, "clusters", len(ks), "decision violations", {nm: int(tr.viol.sum()) for nm, tr in TR.items()}, "intervals", IV,
        "rand max sd", round(RSTAT[0], 2), "ladder pairs", sum(len(a[0]) for a in TR["R"].lad))
out = dict(IV=json.dumps(IV), RSTAT=json.dumps(RSTAT), SC=json.dumps(SC), WHOLEIN=WHOLEIN)
for nm, tr in TR.items():
    for f in ("enter", "ib", "cb", "gt", "viol", "cent", "srch"): out[f"{nm}|{f}"] = getattr(tr, f)
    if tr.lad:
        out[f"{nm}|lr"] = np.concatenate([a[0] for a in tr.lad]); out[f"{nm}|lq"] = np.concatenate([a[1] for a in tr.lad])
        out[f"{nm}|lLB"] = np.concatenate([a[2] for a in tr.lad], 1); out[f"{nm}|lUB"] = np.concatenate([a[3] for a in tr.lad], 1)
np.savez(W_ + f"out_w{WID}.npz", **out)
log("done")
