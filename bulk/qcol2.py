# Version 2 of qcol.py: the same columnar cells and bounds, with the finer cells as a second plane that is read as its own
# columns, in place of a head stored at double width. A unit is (coordinate, plane); every unit costs BITS = 4 bits.
#
# Code (per group basis coordinate i, sd sigma_i, step s_i = 0.6 sigma_i, 14 cells, T_i = 6.5 s_i):
#   plane 1: |z| > T: escape symbol with the sign (the reader knows z = sign * (T + xi), xi >= 0); otherwise
#            k = floor((z + u_i) / s_i), u_i uniform on (-s_i/2, s_i/2) from the public seed, one per coordinate and group.
#   plane 2 (stored for the first H2 coordinates of the group's fixed order): the 16 sub-cells of the plane-1 cell, same
#            dither, so the reconstruction error is uniform on [-s_i/32, s_i/32]; an escape is stored exactly.
# A reader at ladder point j has read the first F_j units of its unit order, and with sign columns also the signs of the
# plane-1 units among the next G_j - F_j (G_j counts the units whose key is at least key(F_j) / RHO).
# Unit keys: plane 1 of i: g_i; plane 2 of i: KAP * g_i (so it follows its plane 1), g_i = Delta_i^2 lambda_i for the
# query-chosen orders (adapt), the profile's mean of it for the fixed order (fix). adaptP: the query orders only the units
# of the first PC coordinates of the fixed order (the part stored as columns); the rest keep the fixed order after them.
# Bounds on Delta.r, per fixed block and summed, as in qcol.py: estimate from the cells read; rest = sqrt(Delta energy of
# coordinates with no cell read, or escaped at plane 1 only, less the sign-read ones on the wrong side) * sqrt(block norm^2
# - energy lower bound of the cells read); read part = sum |Delta_i| (cell half-width) (det) or the smaller of that and
# c * sqrt(sum Delta_i^2 width_i^2 / 12) (rand; rand1s uses the one-sided c for exclusions).
# fixW: the design's shape on the same cells (fixed unit order, levels, one stored weighted error norm per level).
# After every unit: both planes of every coordinate (4 B charged for exact escapes), then the full vector.
# Scalars: 2 B + 1 B per block norm; fixW 3 B + 1 B per level read.
# Every interval and decision is checked against the exact score.
# Stage statistic (rand variants): bytes per item when a cluster's first columnar stage reads, for every item it holds,
# the units up to the median (or the 80th percentile) of its items' needs, and items needing more carry on one by one.
import numpy as np, sys, os, json
from common import setup, log, wait_lock, read_rows, V
BASIS = os.environ.get("BASIS", "pca"); OUT = sys.argv[1]
TSTEP = int(os.environ.get("TSTEP", "16")); NQI = int(os.environ.get("NQI", "14")); RHO = float(os.environ.get("RHO", "3"))
EDG = [int(x) for x in os.environ.get("BLOCKS", "0,2,8,32,128,512,768").split(",")]
ORDERS = os.environ.get("ORDERS", "fix,adapt,adapts,adaptP").split(","); PC = int(os.environ.get("PC", "192")); H2 = int(os.environ.get("H2", "128"))
BITS, NC, STEPM, KAP = 4, 14, 0.6, float(os.environ.get("KAP", "0.16"))
D = setup(BASIS, nq_text_step=TSTEP, nq_item=NQI)
Z, cs, gs, grp, K, G, n = D["Z"], D["cs"], D["gs"], D["grp"], D["K"], D["G"], D["n"]
Mean, Mu, VQ, LAM, b, Q, kind, S, st, en, cl, NS = D["Mean"], D["Mu"], D["VQ"], D["LAM"], D["b"], D["Q"], D["kind"], D["S"], D["st"], D["en"], D["cl"], D["ns"]
NB = len(EDG) - 1; E0 = np.array(EDG[:-1]); BID = np.searchsorted(np.array(EDG[1:]), np.arange(n), side="right")
SLACK = 2e-4; U = n + H2
def round_up(x): return np.where(x > 0, 2.0 ** (np.ceil(16 * np.log2(np.maximum(x, 1e-30))) / 16), 0).astype(np.float32)
Rn = round_up(np.sqrt(np.stack([(Z[:, EDG[i]:EDG[i+1]] ** 2).sum(1) for i in range(NB)], 1))); Rn2 = Rn * Rn   # 1 B per block norm
SIG = np.sqrt(LAM).astype(np.float64); gb = np.searchsorted(gs, np.arange(G + 1)); Mu2 = (Mu * Mu).sum(1)
LADC = np.array([0, 4, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256, 384, 512, 768, U]); NL = len(LADC) - 1   # units read
NBIG = 1e9; EPS = 1e-9
def cval(f): return float(np.sqrt(2 * np.log(2 * f * NBIG * (NL + 1) / EPS)))
C_IN = cval(1.0)

def offsets(q):
    qgK = np.stack([q @ VQ[g] for g in range(G)])[grp]; al = (qgK * Mu).sum(1) / Mu2
    return al, qgK - al[:, None] * Mu
rng = np.random.default_rng(5); held = set(np.load(f"{V}/item_q_idx.npy").tolist())
pi = [int(i) for i in rng.choice(1_000_000, 210, replace=False) if int(i) not in held][:200]
P = np.zeros((G, n)); cntg = np.bincount(grp, minlength=G)
for i in pi:
    q = read_rows(i, i + 1)[0]; np.add.at(P, grp, offsets(q)[1].astype(np.float64) ** 2)
P /= cntg[:, None] * len(pi); P = np.maximum(P, 1e-3 * P.mean(1, keepdims=True))
GAINF = P * LAM; FIXO = np.argsort(-GAINF, axis=1, kind="stable")
P2 = FIXO[:, :H2]                                                                       # coordinates with a second plane, per group
BIDU = np.concatenate([np.broadcast_to(BID, (G, n)), BID[P2]], 1)                      # block of each unit
def ukeys(gain, g): return np.concatenate([gain, KAP * gain[P2[g]]])
KF = np.stack([ukeys(GAINF[g], g) for g in range(G)]); FIXU = np.argsort(-KF, axis=1, kind="stable")
rk = np.empty((G, n), np.int64); np.put_along_axis(rk, FIXO, np.broadcast_to(np.arange(n), (G, n)), 1)
CANDU = np.concatenate([rk < PC, np.ones((G, H2), bool)], 1)                           # units stored as columns
IS1 = np.arange(U) < n
log("profile and fixed orders done")

def sign_frontier(ks, is1o):
    """G_j per ladder point (units whose key is at least key(F_j) / RHO; G_0 = 0) and the number of signs read at j."""
    gf = ks[np.minimum(LADC, U - 1)] / RHO
    Gv = np.maximum(LADC, np.searchsorted(-ks, -gf, side="right")); Gv[0] = 0; Gv[-1] = U; Gv = np.maximum.accumulate(Gv)
    c1 = np.concatenate([[0], np.cumsum(is1o)]); return Gv, c1[Gv] - c1[LADC]

drng = np.random.default_rng(20261114)
SS = (STEPM * SIG).astype(np.float32); UU = ((drng.random((G, n)) - 0.5) * SS).astype(np.float32); TT = ((NC / 2 - 0.5) * SS).astype(np.float32)
SYM = np.empty((NS, n), np.int8); nesc = 0
def planes(Zg, symg, g):
    k = symg.astype(np.float32); esc = np.abs(k) > 100
    zh1 = np.where(esc, np.sign(k) * TT[g], (k + 0.5) * SS[g] - UU[g]); s2 = SS[g] / 16
    zh2 = np.where(esc, Zg, (np.floor((Zg + UU[g]) / s2) + 0.5) * s2 - UU[g])
    return esc, zh1, zh2
for g in range(G):
    a, e = gb[g], gb[g+1]; Zg = Z[a:e]; esc = np.abs(Zg) > TT[g]
    k = np.floor((Zg + UU[g]) / SS[g]); assert k[~esc].min() >= -(NC // 2) and k[~esc].max() <= NC // 2 - 1
    SYM[a:e] = np.where(esc, 127 * np.sign(Zg), k).astype(np.int8); nesc += int(esc.sum())
log(f"encoded: escapes {100 * nesc / SYM.size:.3f}% of coordinates")

# fixW on the same cells: levels at LADC[LW] units of the fixed unit order, one stored weighted error norm per level
LW = np.array([i for i, v in enumerate(LADC) if v in (0, 8, 16, 32, 64, 96, 192, 384, 768, U)])
WD = np.empty((G, n, len(LW)), np.float32); NR = np.empty((NS, len(LW)), np.float32)
for g in range(G):
    a, e = gb[g], gb[g+1]; Zg = Z[a:e]; esc, zh1, zh2 = planes(Zg, SYM[a:e], g); Zd = Zg.astype(np.float64)
    e2s = [Zd * Zd, (Zd - zh1) ** 2, (Zd - zh2) ** 2]; lam = LAM[g].astype(np.float64); gm = [lam, e2s[1].mean(0), e2s[2].mean(0)]
    for li, l in enumerate(LW):
        rd = np.zeros(U, bool); rd[FIXU[g][:LADC[l]]] = True; depth = rd[:n].astype(int); depth[P2[g]] += rd[n:]
        Ee = np.maximum(np.choose(depth, gm), 1e-12 * lam.max()); d2 = np.sqrt(Ee / P[g]); WD[g, :, li] = d2
        NR[a:e, li] = round_up(np.sqrt(sum((e2s[k][:, depth == k] / d2[depth == k]).sum(1) for k in range(3))))
    del Zd, e2s, zh1, zh2
log("fixW weights and stored norms done")

def cluster_parts(Zc, symc, d, order, Gv, g, rn2c):
    """Per item and ladder point: estimate, deterministic read part, upper and lower rest, true read error; per cluster: proxy sd."""
    m = len(Zc); s = SS[g]; p2 = P2[g]; T = TT[g]; esc, zh1, zh2a = planes(Zc, symc, g); ne = ~esc
    lb1 = np.where(esc, T, np.maximum(0, np.abs(zh1) - 0.5 * s)); lb2 = np.where(esc, np.abs(Zc), np.maximum(0, np.abs(zh2a) - s / 32))[:, p2]
    df = d.astype(np.float32); ad = np.abs(df); d2f = df * df; e2 = esc[:, p2]; n2 = ~e2; zh2 = zh2a[:, p2]; dp = df[p2]; sp = s[p2]
    cat = lambda x, y: np.concatenate([x, y], 1)
    FLp = np.unique(np.concatenate([LADC, Gv])); NLF = len(FLp) - 1; iF = np.searchsorted(FLp, LADC); iG = np.searchsorted(FLp, Gv)
    cell = np.searchsorted(FLp[1:], np.arange(U), side="right") * NB + BIDU[g][order]
    ps = np.argsort(cell, kind="stable"); co = order[ps]; c2 = cell[ps]
    stt = np.concatenate([[0], np.flatnonzero(np.diff(c2)) + 1]); cells = c2[stt]
    def red(M):
        o = np.zeros((m, NLF * NB), np.float32); o[:, cells] = np.add.reduceat(M[:, co], stt, axis=1); return o.reshape(m, NLF, NB)
    def cum(M):
        o = np.zeros((m, NLF + 1) + M.shape[2:], np.float32); o[:, 1:] = M.cumsum(1); return o
    est = cum(red(cat(zh1 * df, (zh2 - zh1[:, p2]) * dp)).sum(2))[:, iF]
    qd = cum(red(cat(ne * (ad * 0.5 * s), n2 * (ad[p2] * (sp / 32 - 0.5 * sp)))).sum(2))[:, iF]
    eq = cum(red(cat(ne * (zh1 - Zc) * df, n2 * (zh2 - zh1[:, p2]) * dp)).sum(2))[:, iF]
    low = cum(red(cat(lb1 * lb1, lb2 * lb2 - lb1[:, p2] ** 2)))[:, iF]; ds = cum(red(cat(esc * d2f, -(e2 * d2f[p2]))))[:, iF]
    d2 = d.astype(np.float64) ** 2; s64 = s.astype(np.float64)
    def cred(v):
        o = np.zeros(NLF * NB); o[cells] = np.add.reduceat(v[co], stt); return np.concatenate([np.zeros((1, NB)), o.reshape(NLF, NB).cumsum(0)])
    dqc = cred(np.concatenate([d2, np.zeros(H2)]))
    Dun = np.maximum(0, dqc[-1][None] - dqc[iF])
    sv = np.sqrt(np.maximum(0, cred(np.concatenate([d2 * s64 ** 2 / 12, d2[p2] * (s64[p2] ** 2 / 256 - s64[p2] ** 2) / 12])).sum(1)))[iF]
    item = np.sqrt(np.maximum(0, rn2c[:, None, :] - low)); base = Dun[None].astype(np.float32) + ds
    if (Gv > LADC).any():
        ag = cum(red(cat((np.sign(Zc) == np.sign(df)) * d2f, np.zeros((m, H2), np.float32)))); A = ag[:, iG] - ag[:, iF]
        Dsg = (dqc[iG] - dqc[iF])[None].astype(np.float32)
        wu = (np.sqrt(np.maximum(0, base - (Dsg - A))) * item).sum(2); wl = (np.sqrt(np.maximum(0, base - A)) * item).sum(2)
    else: wu = wl = (np.sqrt(np.maximum(0, base)) * item).sum(2)
    # both planes of every coordinate
    q2 = (ne * (ad * (s / 32))).sum(1); col = lambda x, y: np.concatenate([x, y[:, None]], 1); z1 = np.zeros(m, np.float32)
    return (col(est, (zh2a * df).sum(1)), col(qd, q2), col(wu, z1), col(wl, z1), np.append(sv, np.sqrt((d2 * s64 ** 2 / (256 * 12)).sum())),
            col(eq, (ne * (zh2a - Zc) * df).sum(1)))

FR = [1e-4, 1e-3, 1e-2, 5e-2]; BINS = [0, 0.5, 1, 2, 4, 8, np.inf]; NBIN = len(BINS) - 1; FULL = 2 * n
C_OUT = [cval(f) for f in FR]; VARS = ["det", "rand", "rand1s"]; LADX = np.append(LADC, 2 * n)
names = [f"{o}/{v}" for o in ORDERS for v in VARS] + (["fixW/det"] if "fix" in ORDERS else [])
SCAL = 2 + NB
acc = {kd: {nm: dict(cost=np.zeros(len(FR)), cols=np.zeros(len(FR)), band=np.zeros((len(FR), NBIN)), und0=np.zeros(len(FR)), full=np.zeros(len(FR))) for nm in names} for kd in ("text", "item")}
bandn = {kd: np.zeros((len(FR), NBIN)) for kd in ("text", "item")}; nqk = {"text": 0, "item": 0}
viol = {nm: 0 for nm in names}; checks = 0; maxsd = 0.0; nrand = 0; hist_sd = np.zeros(12)
wmean = {kd: {f"{o}/{v}": np.zeros(NL + 2) for o in ORDERS for v in ("det", "rand")} for kd in ("text", "item")}
bmean = {kd: {o: np.zeros(NL + 2) for o in ORDERS} for kd in ("text", "item")}
A5 = lambda: np.empty((NS, NL + 2), np.float32)
stage = {kd: {f"{o}/rand": np.zeros((2, len(FR))) for o in ORDERS} for kd in ("text", "item")}; cid = np.repeat(np.arange(len(cl)), en - st)
EST = {o: A5() for o in ORDERS}; QD = {o: A5() for o in ORDERS}; WU = {o: A5() for o in ORDERS}; WL = {o: A5() for o in ORDERS}
SVI = {o: A5() for o in ORDERS}; EQ = {o: A5() for o in ORDERS}; CB = {o: A5() for o in ORDERS}
ar = np.arange(NS); scw = np.append(3 + LADC[LW] * BITS / 8.0 + np.arange(len(LW)), 3 + 2 * n * BITS / 8.0 + 4 + len(LW)); NOSIGN = np.zeros(NL + 1)
for qn, (q, kd) in enumerate(zip(Q, kind)):
    if qn % 2 == 0: wait_lock()
    s_ = S[:, qn]; sig = float(s_.std()); qm = Mean @ q
    al, Dl = offsets(q); GA = Dl.astype(np.float64) ** 2 * LAM[grp]
    for k in range(len(cl)):
        c = cl[k]; a, e = st[k], en[k]; g = grp[c]; base = qm[c] + al[c] * b[a:e]; KA = ukeys(GA[c], g); oa = None
        for o in ORDERS:
            if o.startswith("fix"): order = FIXU[g]; ks = KF[g][order]
            elif o == "adaptP":
                fu = FIXU[g]; cm = CANDU[g][fu]; cand = fu[cm]; order = np.concatenate([cand[np.argsort(-KA[cand], kind="stable")], fu[~cm]])
            else:
                if oa is None: oa = np.argsort(-KA, kind="stable")
                order = oa; ks = KA[order]
            Gv, nsg = sign_frontier(ks, IS1[order]) if o.endswith("s") else (LADC, NOSIGN)
            est, qd, wu, wl, sv, eq = cluster_parts(Z[a:e], SYM[a:e], Dl[c], order, Gv, g, Rn2[a:e])
            EST[o][a:e] = base[:, None] + est; QD[o][a:e] = qd; WU[o][a:e] = wu; WL[o][a:e] = wl; SVI[o][a:e] = sv[None]; EQ[o][a:e] = eq
            CB[o][a:e] = np.append((BITS * LADC + nsg) / 8.0, 2 * n * BITS / 8.0 + 4)[None]
    ths = [np.partition(s_, NS - round(f * NS))[NS - round(f * NS)] for f in FR]; sc_ = s_[:, None]; u_ = [np.abs(s_ - t) / sig for t in ths]
    for o in ORDERS:
        e_ = EST[o]; qd = QD[o]; wu = WU[o] + SLACK; wl = WL[o] + SLACK; sv = SVI[o]; cb = CB[o]
        with np.errstate(invalid="ignore", divide="ignore"): r = np.abs(EQ[o][:, 1:]) / sv[:, 1:]
        r = r[np.isfinite(r)]; maxsd = max(maxsd, float(r.max())); nrand += r.size; hist_sd += np.histogram(r, np.arange(13))[0]
        qr = np.minimum(qd, C_IN * sv)
        wmean[kd][f"{o}/det"] += (qd + wu).mean(0) / sig; wmean[kd][f"{o}/rand"] += (qr + wu).mean(0) / sig; bmean[kd][o] += cb.mean(0)
        UL = {"det": (np.minimum.accumulate(e_ + qd + wu, 1), np.maximum.accumulate(e_ - qd - wl, 1)),
              "rand": (np.minimum.accumulate(e_ + qr + wu, 1), np.maximum.accumulate(e_ - qr - wl, 1))}
        for v in VARS:
            nm = f"{o}/{v}"
            for fi, t in enumerate(ths):
                if v == "rand1s": Ub = np.minimum.accumulate(e_ + np.minimum(qd, C_OUT[fi] * sv) + wu, 1); Lb = UL["rand"][1]
                else: Ub, Lb = UL[v]
                if fi == 0 or v == "rand1s": viol[nm] += int(((sc_ > Ub) | (sc_ < Lb)).sum()); checks += Ub.size
                dec = (Lb >= t) | (Ub < t); viol[nm] += int((((Lb >= t) & (sc_ < t)) | ((Ub < t) & (sc_ >= t))).sum())
                none = ~dec.any(1); j = dec.argmax(1); cost = SCAL + cb[ar, j].astype(float); cost[none] = SCAL + cb[none, -1] + FULL
                A_ = acc[kd][nm]; A_["cost"][fi] += cost.mean(); A_["cols"][fi] += LADX[j].mean(); A_["band"][fi] += np.histogram(u_[fi], BINS, weights=cost)[0] / NS
                A_["und0"][fi] += 1 - dec[:, 0].mean(); A_["full"][fi] += none.mean()
                if v == "rand":
                    try:
                        jj = np.where(none, NL + 1, j); js = jj[np.lexsort((jj, cid))]
                        for qi, qq in enumerate((0.5, 0.8)):
                            k1 = js[st + np.minimum(en - st - 1, np.floor(qq * (en - st)).astype(np.int64))]
                            stage[kd][nm][qi, fi] += (SCAL + np.maximum(cost - SCAL, cb[ar, np.repeat(k1, en - st)])).mean()
                    except Exception as ex: log("stage statistic failed:", repr(ex))
    if "fix" in ORDERS:
        nm = "fixW/det"; hD = np.sqrt(np.einsum("kj,kjl->kl", Dl.astype(np.float64) ** 2, WD[grp].astype(np.float64))).astype(np.float32)
        ew = np.concatenate([EST["fix"][:, LW], EST["fix"][:, -1:]], 1); W = np.concatenate([hD[cs] * NR, QD["fix"][:, -1:]], 1) + SLACK
        Ub = np.minimum.accumulate(ew + W, 1); Lb = np.maximum.accumulate(ew - W, 1)
        viol[nm] += int(((sc_ > Ub) | (sc_ < Lb)).sum()); checks += Ub.size
        for fi, t in enumerate(ths):
            dec = (Lb >= t) | (Ub < t); viol[nm] += int((((Lb >= t) & (sc_ < t)) | ((Ub < t) & (sc_ >= t))).sum())
            none = ~dec.any(1); j = dec.argmax(1); cost = scw[j].astype(float); cost[none] = scw[-1] + FULL
            A_ = acc[kd][nm]; A_["cost"][fi] += cost.mean(); A_["cols"][fi] += np.append(LADC[LW], 2 * n)[j].mean(); A_["band"][fi] += np.histogram(u_[fi], BINS, weights=cost)[0] / NS
            A_["und0"][fi] += 1 - dec[:, 0].mean(); A_["full"][fi] += none.mean()
    for fi, t in enumerate(ths): bandn[kd][fi] += np.histogram(u_[fi], BINS)[0] / NS
    nqk[kd] += 1
    if qn % 5 == 0: log(f"query {qn+1}/{len(Q)} ({kd}); violations {sum(viol.values())}; largest randomised error {maxsd:.2f} proxy sd")

np.set_printoptions(precision=1, suppress=True, linewidth=220)
res = dict(basis=BASIS, blocks=EDG, rho=RHO, kap=KAP, h2=H2, pc=PC, nq=nqk, violations=viol, checks=checks, c_in=C_IN, c_out=C_OUT, max_rand_sd=maxsd, rand_checks=nrand,
           rand_sd_hist=hist_sd.tolist(), scalars=SCAL, ladder=LADX.tolist(), out={})
print(f"c two-sided {C_IN:.2f}; one-sided exclusion c at the four answer sizes {np.round(C_OUT, 2)}; scalar bytes per item {SCAL} (2 + {NB} block norms); fixW 3 + 1 per level")
for kd in ("text", "item"):
    if not nqk[kd]: continue
    q_ = nqk[kd]; res["out"][kd] = dict(band_items=(bandn[kd] / q_).tolist(), variants={}, wmean={}, bmean={})
    print(f"\n== {kd} queries ({q_}), basis {BASIS}, blocks {EDG}: bytes per item, scalars included")
    print("   items by band (%), cols = |s-t|/sd in", BINS); print(100 * bandn[kd] / q_)
    for nm in names:
        A_ = acc[kd][nm]; c = A_["cost"] / q_; bc = A_["band"] / q_
        print(f" {nm:14s}: bytes/item {np.round(c, 1)}  units/item {np.round(A_['cols'] / q_, 1)}  undecided at zero read {np.round(100 * A_['und0'] / q_, 1)}%  full vector {np.round(100 * A_['full'] / q_, 3)}%")
        with np.errstate(invalid="ignore", divide="ignore"): pb = bc / (bandn[kd] / q_)
        print("      bytes per item in band:"); print(np.round(pb, 1)); print("      share of cost by band (%):"); print(np.round(100 * bc / c[:, None], 1))
        res["out"][kd]["variants"][nm] = dict(cost=c.tolist(), cols=(A_["cols"] / q_).tolist(), band_cost=bc.tolist(), und0=(A_["und0"] / q_).tolist(), full=(A_["full"] / q_).tolist())
    print("   mean upper slack by ladder point, in score sd, and mean code bytes at that point (units", LADX.tolist(), "; the last point is both planes of every coordinate)")
    for o in ORDERS:
        print(f"   {o:7s} bytes  ", np.round(bmean[kd][o] / q_, 1)); res["out"][kd]["bmean"][o] = (bmean[kd][o] / q_).tolist()
        for v in ("det", "rand"):
            nm = f"{o}/{v}"; print(f"   {nm:14s}", np.round(wmean[kd][nm] / q_, 2)); res["out"][kd]["wmean"][nm] = (wmean[kd][nm] / q_).tolist()
for kd in ("text", "item"):
    if not nqk[kd]: continue
    print(f"\n{kd}: bytes per item when each cluster's first stage reads to the median need of its items / to the 80th percentile")
    for nm, a in stage[kd].items():
        print(f"   {nm:14s} {np.round(a[0] / nqk[kd], 1)} / {np.round(a[1] / nqk[kd], 1)}"); res["out"][kd].setdefault("stage", {})[nm] = (a / nqk[kd]).tolist()
print("\nviolations", viol, "interval checks", checks)
print(f"randomised read-part error: largest {maxsd:.2f} proxy sd over {nrand} (item, level) values; count by integer sd: {hist_sd.astype(int).tolist()}")
json.dump(res, open(OUT, "w")); log("done")
