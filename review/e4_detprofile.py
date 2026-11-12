# E4: deterministic levels (escape code, alpha*) with the step order and the error-norm weights fitted to item offsets (as built),
# the mixture profile, or the text profile; and with two stored norms. Deterministic-only stream to 576 B then the full vector (track "V2").
import numpy as np, sys, time, math
sys.path.insert(0, "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/combined")
from core import *
ROOT = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/combined/"
I = dict(np.load(ROOT + "idx_m768/index.npz")); Xs = np.load(ROOT + "idx_m768/Xs.npy", mmap_mode="r")
N, n, K, G = int(I["N"]), int(I["n"]), int(I["K"]), int(I["G"])
assign, cstart, grp, Mq, mn, ahat = I["assign"], I["cstart"], I["grp"], I["Mq"], I["mn"], I["ahat"]
lamA, VA, PROF = I["lam"], I["V"], {"i": I["Qi"], "m": I["Qmix"], "t": I["Qt"]}
Q, qcls, thr = I["Q"], I["qcls"], I["thr"]
sel = np.r_[np.where(qcls == "item")[0][:100], np.where(qcls == "text")[0][::2][:150]]
Q, qcls, thr = Q[sel], qcls[sel], thr[sel][:, :4]; NQ = len(Q); NT = 4; FULL = 2 * n
LV = [0, 4, 8, 16, 32, 48, 96, 192, 384, 576]; ND = len(LV); DA = 0.5 / 32767 + 1e-7
VAR = {"A order=item norm=item (as built)": ("i", ["i"]), "B order=mix norm=mix": ("m", ["m"]), "C order=text norm=text": ("t", ["t"]),
       "D order=mix norms=item+text": ("m", ["i", "t"]), "E order=item norms=item+text": ("i", ["i", "t"]), "F order=item norm=mix": ("i", ["m"])}
ib = {v: np.zeros((NT, NQ)) for v in VAR}; ib32 = {v: np.zeros((NT, NQ)) for v in VAR}; a32 = {v: np.zeros((NT, NQ)) for v in VAR}; viol = {v: 0 for v in VAR}
def dws(D, QV, MV):
    w2 = (D.astype(np.float64)) ** 2; QV = QV.astype(np.float64); MV = MV.astype(np.float64)
    Aqm = (MV * w2) @ QV.T; Bm = (MV * MV) @ w2; Cq = (QV * QV) @ w2
    return (Aqm / Bm[:, None]).astype(np.float32), np.sqrt(np.maximum(Cq[None] - Aqm ** 2 / Bm[:, None], 0)).astype(np.float32)
gstart = np.searchsorted(grp, np.arange(G + 1)); groups = [int(g) for g in sys.argv[1].split(",")]; nitems = 0; t0 = time.time()
for g in groups:
    ks = np.arange(gstart[g], gstart[g + 1])
    if len(ks) == 0: continue
    lam, V = lamA[g], VA[g]; sig = np.sqrt(lam).astype(np.float32); QV = (Q @ V).astype(np.float32); MV = (Mq[ks] @ V).astype(np.float32)
    ORD = {}
    for o in "imt":
        se, dep_at, sdep, expc = esc_steps(lam, PROF[o][g], 1.5 * 8 * LV[-1]); ORD[o] = (se, dep_at, np.where(sdep == 2)[0], expc)
    W = {}
    for v, (o, ws) in VAR.items():
        for w in ws:
            for l in range(ND):
                if (o, w, l) in W: continue
                de = np.zeros(n, int) if l == 0 else ORD[o][1][int((ORD[o][3] <= 8 * LV[l]).sum())]
                D = (((lam * gE[de] + 1e-10) / (PROF[w][g] + 1e-10)) ** 0.25).astype(np.float32); W[(o, w, l)] = (D,) + dws(D, QV, MV)
    for ci, c in enumerate(ks):
        lo_, hi_ = int(cstart[c]), int(cstart[c + 1]); nc = hi_ - lo_
        if nc == 0: continue
        nitems += nc
        x = unit(Xs[lo_:hi_]); true = x @ Q.T; z = (x - Mq[c]) @ V; u = z / sig
        ah = ahat[lo_:hi_].astype(np.float32) / 32767; m_ = mn[c]; mr = (m_ * ah - m_ * m_)[:, None]; qmc = (Q @ Mq[c])[None]; mvc = MV[ci]
        LB = {v: np.full((nc, NQ), -np.inf, np.float32) for v in VAR}; UB = {v: np.full((nc, NQ), np.inf, np.float32) for v in VAR}
        k2 = np.floor(np.abs(u) / 0.6745).astype(np.int32); CUM = {}
        for o in "imt":
            se, dep_at, s2, expc = ORD[o]; Cst = np.ones((nc, len(se)), np.int32); Cst[:, s2] = k2[:, se[s2]] + 1; CUM[o] = np.cumsum(Cst, 1)
        for l in range(ND):
            inc = 2 if l == 0 else LV[l] - LV[l - 1]; cache = {}
            for o in "imt":
                if l == 0: e = z; cq = np.zeros_like(true); cmv = np.zeros((nc, 1), np.float32)
                else:
                    dep = ORD[o][1][(CUM[o] <= 8 * LV[l]).sum(1)]; code = recon_esc(u, dep) * sig; e = z - code; cq = code @ QV.T; cmv = (code @ mvc)[:, None]
                cache[o] = (e, cq, cmv)
            for v, (o, ws) in VAR.items():
                e, cq, cmv = cache[o]
                for ti in range(NT):
                    t = thr[:, ti]; cnt = ((LB[v] < t) & (UB[v] >= t)).sum(0); ib[v][ti] += (inc + len(ws)) * cnt
                    if l <= 4: ib32[v][ti] += (inc + len(ws)) * cnt
                    if l == 5: a32[v][ti] += cnt
                for w in ws:
                    D, al, dwa = W[(o, w, l)]; ew = roundup16(np.linalg.norm(e / D, axis=1))[:, None]
                    est = qmc + cq + al[ci] * (mr - cmv); h = (dwa[ci] * ew + np.abs(al[ci]) * m_ * DA) * 1.0001 + 2e-5
                    viol[v] += int((np.abs(true - est) > h).sum()); np.maximum(LB[v], est - h, out=LB[v]); np.minimum(UB[v], est + h, out=UB[v])
        for v in VAR:
            for ti in range(NT):
                t = thr[:, ti]; ib[v][ti] += FULL * ((LB[v] < t) & (UB[v] >= t)).sum(0)
    print("group", g, "items so far", nitems, round(time.time() - t0), "s viol", viol, flush=True)
print("items", nitems, "of", N)
for cl in ("item", "text"):
    m = qcls == cl
    print(f"\n== {cl} ({m.sum()} queries), answer sizes 0.01 / 0.1 / 1 / 5 %: total B per item (det-only stream) | B per item through 32 B | % of items undecided after 32 B")
    for v in VAR:
        print(f"  {v:36s}", " ".join(f"{ib[v][ti][m].mean() / nitems:7.2f}" for ti in range(NT)), "|", " ".join(f"{ib32[v][ti][m].mean() / nitems:6.2f}" for ti in range(NT)),
              "|", " ".join(f"{100 * a32[v][ti][m].mean() / nitems:5.1f}" for ti in range(NT)))
