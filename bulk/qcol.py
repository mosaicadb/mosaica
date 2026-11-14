# Bytes for a columnar bulk pass: fixed-width cells per coordinate, read in a fixed or a query-chosen column order, with
#   (a) a deterministic box bound, (b) on the same stored bits a randomised bound from a public subtractive dither,
#   (c) optional sign-only columns ahead of the cell columns, giving one-sided bounds, and
#   (d) fixW, the design's shape (fixed order, one stored weighted error norm per level) on the same cells.
#
# Code (per group basis coordinate i, sd sigma_i, step s_i = STEP * sigma_i, NC cells, T_i = (NC/2 - 1/2) s_i):
#   |z| >  T : escape symbol with the sign; the reader knows z = sign * (T + xi), xi >= 0.
#   |z| <= T : k = floor((z + u_i) / s_i), u_i uniform on (-s_i/2, s_i/2) drawn from the public seed, one per coordinate
#              and group; reconstruction zh = (k + 1/2) s_i - u_i, so zh - z is uniform on [-s_i/2, s_i/2] whatever z is,
#              independent across coordinates.
# A reader at ladder point j has read the cells of the first F_j coordinates of its order and the signs of the next
# G_j - F_j (G_j = F_j without sign columns; with them, G_j counts the coordinates whose gain is at least gain(F_j) / RHO).
# Bounds on Delta.r (Delta = the query's offset in the same basis), per fixed block b, summed over blocks:
#   estimate     sum over cells read of Delta_i zh_i (an escape contributes Delta_i sign T)
#   rest, upper  sqrt(Delta energy of: unread, escaped, and sign-read coordinates whose sign agrees with Delta_i)
#                * sqrt(norm_b^2 - sum over cells read of lb_i^2),  lb_i = max(0, |zh_i| - s_i/2), or T_i for an escape
#   rest, lower  the same with the sign-read coordinates whose sign disagrees. Always valid.
#   read part    deterministic: sum over cells read, not escaped, of |Delta_i| s_i / 2.
#                randomised:    c * sqrt(sum over cells read of Delta_i^2 s_i^2 / 12): a sum of independent uniforms is
#                sub-Gaussian with that variance proxy, so each side fails with probability at most exp(-c^2/2). One number
#                per (query, cluster, level): no stored scalar, no per-item work. The interval uses the smaller read part.
# c: two-sided c_in = sqrt(2 ln(2 N L / eps)), N = 1e9, eps = 1e-9, L levels. One-sided variant (rand1s): an exclusion can
# only be wrong for a true match, so its c is sqrt(2 ln(2 f N L / eps)) at answer share f; inclusions keep c_in.
# detE adds one stored error norm per block (1 B each, over the whole block's cells): the read part of a block is then the
# smaller of the box sum and sqrt(Delta energy read) * that norm. Last ladder point: a second plane of BITS per coordinate
# (cells 16 times finer, escapes stored exactly, 4 B charged); then the full vector.
# The first HEAD coordinates of the fixed order are stored 16 times finer in a cell of 2 * BITS (same escape threshold).
# Bits: BITS (or 2 * BITS) per cell read, 1 per sign read (as for a sign-and-magnitude cell; the dithered cell simulated here would need
# its magnitude re-indexed for that, see the report). Scalars: 2 B + 1 B per block norm; fixW: 3 B + 1 B per level read.
# Every interval and every decision is checked against the exact score; the largest randomised error is reported in units
# of its proxy sd.
import numpy as np, sys, os, json
from common import setup, log, wait_lock, read_rows, V
BASIS = os.environ.get("BASIS", "pca"); OUT = sys.argv[1]
TSTEP = int(os.environ.get("TSTEP", "9")); NQI = int(os.environ.get("NQI", "30")); RHO = float(os.environ.get("RHO", "3"))
EDG = [int(x) for x in os.environ.get("BLOCKS", "0,2,8,32,128,768").split(",")]
CODES = [(int(a), int(b), float(c)) for a, b, c in (x.split(":") for x in os.environ.get("CODES", "4:14:0.6").split(","))]  # bits, cells, step
ORDERS = os.environ.get("ORDERS", "fix,adapt,adapts,adaptP").split(","); PC = int(os.environ.get("PC", "192")); HEAD = int(os.environ.get("HEAD", "64"))
# orders: fix (one per group), adapt (per query and cluster), a trailing "s" adds sign columns; adaptP lets the query
# order only the first PC coordinates of the fixed order (the part stored as columns) and keeps the fixed order after them
D = setup(BASIS, nq_text_step=TSTEP, nq_item=NQI)
Z, cs, gs, grp, K, G, n = D["Z"], D["cs"], D["gs"], D["grp"], D["K"], D["G"], D["n"]
Mean, Mu, VQ, LAM, b, Q, kind, S, st, en, cl, NS = D["Mean"], D["Mu"], D["VQ"], D["LAM"], D["b"], D["Q"], D["kind"], D["S"], D["st"], D["en"], D["cl"], D["ns"]
NB = len(EDG) - 1; E0 = np.array(EDG[:-1]); BID = np.searchsorted(np.array(EDG[1:]), np.arange(n), side="right")
SLACK = 2e-4
def round_up(x): return np.where(x > 0, 2.0 ** (np.ceil(16 * np.log2(np.maximum(x, 1e-30))) / 16), 0).astype(np.float32)
Rn = round_up(np.sqrt(np.stack([(Z[:, EDG[i]:EDG[i+1]] ** 2).sum(1) for i in range(NB)], 1))); Rn2 = Rn * Rn   # 1 B per block norm
SIG = np.sqrt(LAM).astype(np.float64); gb = np.searchsorted(gs, np.arange(G + 1)); Mu2 = (Mu * Mu).sum(1)
LADC = np.array([0, 4, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256, 384, 512, 768]); NL = len(LADC) - 1
NBIG = 1e9; EPS = 1e-9
def cval(f): return float(np.sqrt(2 * np.log(2 * f * NBIG * NL / EPS)))
C_IN = cval(1.0)

def offsets(q):
    qgK = np.stack([q @ VQ[g] for g in range(G)])[grp]; al = (qgK * Mu).sum(1) / Mu2
    return al, qgK - al[:, None] * Mu
# fixed order: one per group, by the mean of Delta_i^2 over pseudo-queries (corpus items outside the test set) times lambda_i
rng = np.random.default_rng(5); held = set(np.load(f"{V}/item_q_idx.npy").tolist())
pi = [int(i) for i in rng.choice(1_000_000, 210, replace=False) if int(i) not in held][:200]
P = np.zeros((G, n)); cntg = np.bincount(grp, minlength=G)
for i in pi:
    q = read_rows(i, i + 1)[0]; np.add.at(P, grp, offsets(q)[1].astype(np.float64) ** 2)
P /= cntg[:, None] * len(pi); P = np.maximum(P, 1e-3 * P.mean(1, keepdims=True))
GAINF = P * LAM; FIXO = np.argsort(-GAINF, axis=1, kind="stable")
log("profile and fixed orders done")

def sign_frontier(gsorted):
    """G_j for each ladder point: the number of coordinates whose gain is at least gain(F_j) / RHO; G_0 = 0."""
    gf = gsorted[np.minimum(LADC, n - 1)] / RHO
    Gv = np.maximum(LADC, np.searchsorted(-gsorted, -gf, side="right")); Gv[0] = 0; Gv[-1] = n
    return np.maximum.accumulate(Gv)

# encode
drng = np.random.default_rng(20261114)
ENC = []
for bits, nc, stepm in CODES:
    T = ((nc / 2 - 0.5) * stepm * SIG).astype(np.float32); rk = np.empty((G, n), np.int64); np.put_along_axis(rk, FIXO, np.broadcast_to(np.arange(n), (G, n)), 1)
    head = rk < HEAD; s = (stepm * SIG / np.where(head, 16, 1)).astype(np.float32); bitsv = np.where(head, 2 * bits, bits)
    u = ((drng.random((G, n)) - 0.5) * s).astype(np.float32)
    sym = np.empty((NS, n), np.int8); nesc = 0
    for g in range(G):
        a, e = gb[g], gb[g+1]; Zg = Z[a:e]; esc = np.abs(Zg) > T[g]
        k = np.floor((Zg + u[g]) / s[g]); assert np.abs(k[~esc]).max() <= 16 * (nc // 2) and 16 * (nc // 2) < 127
        sym[a:e] = np.where(esc, 127 * np.sign(Zg), k).astype(np.int8); nesc += int(esc.sum())
    ENC.append(dict(bits=bits, s=s, u=u, T=T, sym=sym, bitsv=bitsv, cumb=np.concatenate([np.zeros((G, 1)), np.cumsum(np.take_along_axis(bitsv, FIXO, 1), 1)], 1), u2=((drng.random((G, n)) - 0.5) * s / 16).astype(np.float32))); log(f"code {bits} bits, {nc} cells, step {stepm} sd: escapes {100 * nesc / sym.size:.3f}% of coordinates")

# fixW, the design's shape on the same stored cells: the fixed order, levels at LADC[LW] columns, one stored weighted error
# norm per level (1 B, rounded up), bound ||D Delta|| * ||D^-1 e|| with D^2 = sqrt(mean squared error / profile) per coordinate.
LW = np.array([i for i, v in enumerate(LADC) if v in (0, 8, 16, 32, 64, 96, 192, 384, 768)])
for E in ENC:
    WD = np.empty((G, n, len(LW)), np.float32); NR = np.empty((NS, len(LW)), np.float32); ERN = np.empty((NS, NB), np.float32)
    for g in range(G):
        a, e = gb[g], gb[g+1]; Zg = Z[a:e].astype(np.float64); k = E["sym"][a:e].astype(np.float32); esc = np.abs(k) > 100
        zh = np.where(esc, np.sign(k) * E["T"][g], (k + 0.5) * E["s"][g] - E["u"][g]); er2 = (Zg - zh) ** 2; z2 = Zg * Zg
        lam = LAM[g].astype(np.float64); g1 = er2.mean(0)
        ERN[a:e] = round_up(np.sqrt(np.add.reduceat(er2 * ~esc, E0, axis=1)))
        for li, l in enumerate(LW):
            rd = np.zeros(n, bool); rd[FIXO[g][:LADC[l]]] = True
            Ee = np.maximum(np.where(rd, g1, lam), 1e-12 * lam.max()); d2 = np.sqrt(Ee / P[g]); WD[g, :, li] = d2
            NR[a:e, li] = round_up(np.sqrt((er2[:, rd] / d2[rd]).sum(1) + (z2[:, ~rd] / d2[~rd]).sum(1)))
        del Zg, er2, z2, zh
    E["WD"] = WD; E["NR"] = NR; E["ERN"] = ERN
log("fixW weights and stored norms done")

def cluster_parts(Zc, symc, d, order, Gv, s, u, T, rn2c, u2, ernc):
    """Per item and ladder point: estimate, deterministic read part, upper and lower rest; per cluster: proxy sd; true read error."""
    m = len(Zc); FLp = np.unique(np.concatenate([LADC, Gv])); NLF = len(FLp) - 1
    iF = np.searchsorted(FLp, LADC); iG = np.searchsorted(FLp, Gv)
    cell = np.searchsorted(FLp[1:], np.arange(n), side="right") * NB + BID[order]
    p2 = np.argsort(cell, kind="stable"); co = order[p2]; c2 = cell[p2]
    stt = np.concatenate([[0], np.flatnonzero(np.diff(c2)) + 1]); cells = c2[stt]
    Zo = Zc[:, co]; k = symc[:, co].astype(np.float32); esc = np.abs(k) > 100; ne = ~esc
    so = s[co]; To = T[co]; dco = d[co].astype(np.float32); d2f = dco * dco
    zh = np.where(esc, np.sign(k) * To, (k + 0.5) * so - u[co]); lb = np.where(esc, To, np.maximum(0, np.abs(zh) - 0.5 * so))
    def red(M):
        o = np.zeros((m, NLF * NB), np.float32); o[:, cells] = np.add.reduceat(M, stt, axis=1); return o.reshape(m, NLF, NB)
    def cum(M):
        o = np.zeros((m, NLF + 1) + M.shape[2:], np.float32); o[:, 1:] = M.cumsum(1); return o
    est = cum(red(zh * dco).sum(2))[:, iF]; qdb = cum(red(ne * (np.abs(dco) * 0.5 * so)))[:, iF]; qd = qdb.sum(2); eq = cum(red(ne * (zh - Zo) * dco).sum(2))[:, iF]
    low = cum(red(lb * lb))[:, iF]; ds = cum(red(esc * d2f))[:, iF]
    d2 = dco.astype(np.float64) ** 2
    dq = np.zeros(NLF * NB); dq[cells] = np.add.reduceat(d2, stt); dqc = np.concatenate([np.zeros((1, NB)), dq.reshape(NLF, NB).cumsum(0)])
    Dun = np.maximum(0, dqc[-1][None] - dqc[iF])                                       # unread by cells, per block
    vq = np.zeros(NLF * NB); vq[cells] = np.add.reduceat(d2 * so.astype(np.float64) ** 2 / 12, stt)
    sv = np.sqrt(np.concatenate([[0], vq.reshape(NLF, NB).sum(1).cumsum()]))[iF]
    item = np.sqrt(np.maximum(0, rn2c[:, None, :] - low)); base = Dun[None].astype(np.float32) + ds
    if (Gv > LADC).any():
        ag = cum(red((np.sign(Zo) == np.sign(dco)) * d2f)); A = ag[:, iG] - ag[:, iF]         # agreeing energy among sign-read
        Dsg = (dqc[iG] - dqc[iF])[None].astype(np.float32)
        wu = (np.sqrt(np.maximum(0, base - (Dsg - A))) * item).sum(2); wl = (np.sqrt(np.maximum(0, base - A)) * item).sum(2)
    else: wu = wl = (np.sqrt(np.maximum(0, base)) * item).sum(2)
    qe = np.minimum(qdb, np.sqrt(np.maximum(0, dqc[iF][None].astype(np.float32) - ds)) * ernc[:, None, :]).sum(2)
    # second plane, every coordinate
    s2 = so / 16; u2o = u2[co]; zh2 = np.where(esc, Zo, (np.floor((Zo + u2o) / s2) + 0.5) * s2 - u2o)
    q2 = (ne * (np.abs(dco) * 0.5 * s2)).sum(1); col = lambda x, y: np.concatenate([x, y[:, None]], 1); z1 = np.zeros(m, np.float32)
    return (col(est, (zh2 * dco).sum(1)), col(qd, q2), col(qe, q2), col(wu, z1), col(wl, z1),
            np.append(sv, np.sqrt((d2 * s2.astype(np.float64) ** 2 / 12).sum())), col(eq, (ne * (zh2 - Zo) * dco).sum(1)))

FR = [1e-4, 1e-3, 1e-2, 5e-2]; BINS = [0, 0.5, 1, 2, 4, 8, np.inf]; NBIN = len(BINS) - 1; FULL = 2 * n
C_OUT = [cval(f) for f in FR]
COMBOS = [(ci, o) for ci in range(len(CODES)) for o in ORDERS]
VARS = ["det", "detE", "rand", "rand1s"]; LADX = np.append(LADC, n)
names = [f"b{CODES[ci][0]}/{o}/{v}" for ci, o in COMBOS for v in VARS] + ([f"b{c[0]}/fixW/det" for c in CODES] if "fix" in ORDERS else [])
SCAL = 2 + NB
acc = {kd: {nm: dict(cost=np.zeros(len(FR)), cols=np.zeros(len(FR)), band=np.zeros((len(FR), NBIN)), und0=np.zeros(len(FR)), full=np.zeros(len(FR))) for nm in names} for kd in ("text", "item")}
bandn = {kd: np.zeros((len(FR), NBIN)) for kd in ("text", "item")}; nqk = {"text": 0, "item": 0}
viol = {nm: 0 for nm in names}; checks = 0; maxsd = 0.0; nrand = 0; hist_sd = np.zeros(12)
wmean = {kd: {f"b{CODES[ci][0]}/{o}/{v}": np.zeros(NL + 2) for ci, o in COMBOS for v in ("det", "rand")} for kd in ("text", "item")}   # mean upper slack by level, in sd
bmean = {kd: {(ci, o): np.zeros(NL + 2) for ci, o in COMBOS} for kd in ("text", "item")}                                               # mean code bytes by level
A5 = lambda: np.empty((NS, NL + 2), np.float32)
EST = {c: A5() for c in COMBOS}; QD = {c: A5() for c in COMBOS}; WU = {c: A5() for c in COMBOS}; WL = {c: A5() for c in COMBOS}
SVI = {c: A5() for c in COMBOS}; EQ = {c: A5() for c in COMBOS}; CB = {c: A5() for c in COMBOS}; QE = {c: A5() for c in COMBOS}
ar = np.arange(NS)
for qn, (q, kd) in enumerate(zip(Q, kind)):
    if qn % 2 == 0: wait_lock()
    s_ = S[:, qn]; sig = float(s_.std()); med = float(np.median(s_)); qm = Mean @ q
    al, Dl = offsets(q); GA = Dl.astype(np.float64) ** 2 * LAM[grp]
    OA = np.argsort(-GA, axis=1, kind="stable")
    for k in range(len(cl)):
        c = cl[k]; a, e = st[k], en[k]; g = grp[c]; base = qm[c] + al[c] * b[a:e]
        for ci, o in COMBOS:
            E = ENC[ci]; order = FIXO[g] if o.startswith("fix") else OA[c]
            if o == "adaptP": fo = FIXO[g]; order = np.concatenate([fo[:PC][np.argsort(-GA[c][fo[:PC]], kind="stable")], fo[PC:]])
            if o.endswith("s"): Gv = sign_frontier((GAINF[g] if o.startswith("fix") else GA[c])[order])
            else: Gv = LADC
            est, qd, qe, wu, wl, sv, eq = cluster_parts(Z[a:e], E["sym"][a:e], Dl[c], order, Gv, E["s"][g], E["u"][g], E["T"][g], Rn2[a:e], E["u2"][g], E["ERN"][a:e])
            EST[ci, o][a:e] = base[:, None] + est; QD[ci, o][a:e] = qd; QE[ci, o][a:e] = qe; WU[ci, o][a:e] = wu; WL[ci, o][a:e] = wl; SVI[ci, o][a:e] = sv[None]; EQ[ci, o][a:e] = eq
            CB[ci, o][a:e] = np.append((np.concatenate([[0], np.cumsum(E["bitsv"][g][order])])[LADC] + (Gv - LADC)) / 8.0, (E["bitsv"][g].sum() + E["bits"] * n) / 8.0 + 4)[None]
    ths = [np.partition(s_, NS - round(f * NS))[NS - round(f * NS)] for f in FR]; sc_ = s_[:, None]; u_ = [np.abs(s_ - t) / sig for t in ths]
    for ci, o in COMBOS:
        bits = CODES[ci][0]; e_ = EST[ci, o]; qd = QD[ci, o]; wu = WU[ci, o] + SLACK; wl = WL[ci, o] + SLACK; sv = SVI[ci, o]; cb = CB[ci, o]
        with np.errstate(invalid="ignore", divide="ignore"): r = np.abs(EQ[ci, o][:, 1:]) / sv[:, 1:]
        r = r[np.isfinite(r)]; maxsd = max(maxsd, float(r.max())); nrand += r.size; hist_sd += np.histogram(r, np.arange(13))[0]
        qr = np.minimum(qd, C_IN * sv)
        wmean[kd][f"b{bits}/{o}/det"] += (qd + wu).mean(0) / sig; wmean[kd][f"b{bits}/{o}/rand"] += (qr + wu).mean(0) / sig; bmean[kd][ci, o] += cb.mean(0)
        qe = QE[ci, o]
        UL = {"det": (np.minimum.accumulate(e_ + qd + wu, 1), np.maximum.accumulate(e_ - qd - wl, 1)),
              "detE": (np.minimum.accumulate(e_ + qe + wu, 1), np.maximum.accumulate(e_ - qe - wl, 1)),
              "rand": (np.minimum.accumulate(e_ + qr + wu, 1), np.maximum.accumulate(e_ - qr - wl, 1))}
        for v in VARS:
            nm = f"b{bits}/{o}/{v}"
            for fi, t in enumerate(ths):
                if v == "rand1s": U = np.minimum.accumulate(e_ + np.minimum(qd, C_OUT[fi] * sv) + wu, 1); L = UL["rand"][1]
                else: U, L = UL[v]
                if fi == 0 or v == "rand1s": viol[nm] += int(((sc_ > U) | (sc_ < L)).sum()); checks += U.size
                dec = (L >= t) | (U < t); viol[nm] += int((((L >= t) & (sc_ < t)) | ((U < t) & (sc_ >= t))).sum())
                none = ~dec.any(1); j = dec.argmax(1); sca = SCAL + (NB if v == "detE" else 0); cost = sca + cb[ar, j].astype(float); cost[none] = sca + cb[none, -1] + FULL
                A_ = acc[kd][nm]; A_["cost"][fi] += cost.mean(); A_["cols"][fi] += LADX[j].mean(); A_["band"][fi] += np.histogram(u_[fi], BINS, weights=cost)[0] / NS
                A_["und0"][fi] += 1 - dec[:, 0].mean(); A_["full"][fi] += none.mean()
    if "fix" in ORDERS:
        D2 = Dl.astype(np.float64) ** 2
        for ci, E in enumerate(ENC):
            bits = CODES[ci][0]; nm = f"b{bits}/fixW/det"
            scw = np.concatenate([3 + E["cumb"][:, LADC[LW]] / 8.0 + np.arange(len(LW))[None], 3 + (E["cumb"][:, -1:] + bits * n) / 8.0 + 4 + len(LW)], 1)[gs]   # per item
            hD = np.sqrt(np.einsum("kj,kjl->kl", D2, E["WD"][grp].astype(np.float64))).astype(np.float32)
            ew = np.concatenate([EST[ci, "fix"][:, LW], EST[ci, "fix"][:, -1:]], 1); W = np.concatenate([hD[cs] * E["NR"], QD[ci, "fix"][:, -1:]], 1) + SLACK
            U = np.minimum.accumulate(ew + W, 1); L = np.maximum.accumulate(ew - W, 1)
            viol[nm] += int(((sc_ > U) | (sc_ < L)).sum()); checks += U.size
            for fi, t in enumerate(ths):
                dec = (L >= t) | (U < t); viol[nm] += int((((L >= t) & (sc_ < t)) | ((U < t) & (sc_ >= t))).sum())
                none = ~dec.any(1); j = dec.argmax(1); cost = scw[ar, j].astype(float); cost[none] = scw[none, -1] + FULL
                A_ = acc[kd][nm]; A_["cost"][fi] += cost.mean(); A_["cols"][fi] += np.append(LADC[LW], n)[j].mean(); A_["band"][fi] += np.histogram(u_[fi], BINS, weights=cost)[0] / NS
                A_["und0"][fi] += 1 - dec[:, 0].mean(); A_["full"][fi] += none.mean()
    for fi, t in enumerate(ths): bandn[kd][fi] += np.histogram(u_[fi], BINS)[0] / NS
    nqk[kd] += 1
    if qn % 5 == 0: log(f"query {qn+1}/{len(Q)} ({kd}); violations {sum(viol.values())}; largest randomised error {maxsd:.2f} proxy sd")

np.set_printoptions(precision=1, suppress=True, linewidth=220)
res = dict(basis=BASIS, blocks=EDG, codes=CODES, rho=RHO, nq=nqk, violations=viol, checks=checks, c_in=C_IN, c_out=C_OUT, max_rand_sd=maxsd, rand_checks=nrand,
           rand_sd_hist=hist_sd.tolist(), scalars=SCAL, ladder=LADC.tolist(), out={})
print(f"c two-sided {C_IN:.2f}; one-sided exclusion c at the four answer sizes {np.round(C_OUT, 2)}; scalar bytes per item {SCAL} (2 + {NB} block norms); fixW 3 + 1 per level")
for kd in ("text", "item"):
    if not nqk[kd]: continue
    q_ = nqk[kd]; res["out"][kd] = dict(band_items=(bandn[kd] / q_).tolist(), variants={}, wmean={}, bmean={})
    print(f"\n== {kd} queries ({q_}), basis {BASIS}, blocks {EDG}: bytes per item, scalars included")
    print("   items by band (%), cols = |s-t|/sd in", BINS); print(100 * bandn[kd] / q_)
    for nm in names:
        A_ = acc[kd][nm]; c = A_["cost"] / q_; bc = A_["band"] / q_
        print(f" {nm:16s}: bytes/item {np.round(c, 1)}  cell columns/item {np.round(A_['cols'] / q_, 1)}  undecided at zero read {np.round(100 * A_['und0'] / q_, 1)}%  full vector {np.round(100 * A_['full'] / q_, 3)}%")
        with np.errstate(invalid="ignore", divide="ignore"): pb = bc / (bandn[kd] / q_)
        print("      bytes per item in band:"); print(np.round(pb, 1)); print("      share of cost by band (%):"); print(np.round(100 * bc / c[:, None], 1))
        res["out"][kd]["variants"][nm] = dict(cost=c.tolist(), cols=(A_["cols"] / q_).tolist(), band_cost=bc.tolist(), und0=(A_["und0"] / q_).tolist(), full=(A_["full"] / q_).tolist())
    print("   mean upper slack by ladder point, in score sd, and mean code bytes at that point (cell columns", LADX.tolist(), "; the last point is the second plane)")
    for (ci, o) in COMBOS:
        bits = CODES[ci][0]; print(f"   b{bits}/{o:7s} bytes", np.round(bmean[kd][ci, o] / q_, 1)); res["out"][kd]["bmean"][f"b{bits}/{o}"] = (bmean[kd][ci, o] / q_).tolist()
        for v in ("det", "rand"):
            nm = f"b{bits}/{o}/{v}"; print(f"   {nm:16s}", np.round(wmean[kd][nm] / q_, 2)); res["out"][kd]["wmean"][nm] = (wmean[kd][nm] / q_).tolist()
print("\nviolations", viol, "interval checks", checks)
print(f"randomised read-part error: largest {maxsd:.2f} proxy sd over {nrand} (item, level) values; count by integer sd: {hist_sd.astype(int).tolist()}")
json.dump(res, open(OUT, "w")); log("done")
