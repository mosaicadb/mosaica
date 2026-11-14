# Bytes, not coordinates: a quantised columnar code read in a query-chosen order, against the same code read in a fixed order.
#
# Code (shared by every variant): each coordinate of the group basis is stored in two 4-bit planes.
#   plane 1: sign and floor(|z|/s) clipped to 7, s = C1 * sd of the coordinate in its group; index 7 is unbounded above.
#   plane 2: if plane 1 is bounded, 16 sub-cells; if not, floor(|z|/s - 7) clipped to 15 (index 15 unbounded above).
# A unit is (coordinate, plane); a reader reads a prefix of an order of the 2n units, plane 1 of a coordinate before plane 2.
#
# Variants:
#   fixW  the design's shape: one order per group (greedy on a fitted profile), levels at 4,8,16,32,48,96,... code bytes,
#         one stored weighted error norm per level (1 B, rounded up), bound ||D Delta|| * ||D^-1 e||. Scalars: 3 B + 1 B per level.
#   fixB  one order per group from the same profile, bound = box bound on the cells of the read units plus, per fixed block,
#         sqrt(unread Delta energy) * sqrt(block norm^2 - lower bound on the energy read). Scalars: 2 B + one byte per block norm.
#   adapt as fixB, but the order is chosen per (query, cluster) from the query's own offset.
# Both orders for the box bound are built by the same rule (largest one-step fall of the expected bound, re-ranked at each
# ladder point); fixB feeds it the profile, adapt the query. Every interval and decision is checked against the exact score.
import numpy as np, sys, os, json
from common import setup, log, wait_lock, read_rows, V
BASIS = os.environ.get("BASIS", "pca"); OUT = sys.argv[1]
TSTEP = int(os.environ.get("TSTEP", "6")); NQI = int(os.environ.get("NQI", "40")); C1 = float(os.environ.get("C1", "0.6"))
EDG = [int(x) for x in os.environ.get("BLOCKS", "0,1,2,4,8,16,32,64,128,256,512,768").split(",")]
D = setup(BASIS, nq_text_step=TSTEP, nq_item=NQI)
Z, cs, gs, grp, K, G, n = D["Z"], D["cs"], D["gs"], D["grp"], D["K"], D["G"], D["n"]
Mean, Mu, VQ, LAM, b, Q, kind, S, st, en, cl, NS = D["Mean"], D["Mu"], D["VQ"], D["LAM"], D["b"], D["Q"], D["kind"], D["S"], D["st"], D["en"], D["cl"], D["ns"]
NB = len(EDG) - 1; E0 = np.array(EDG[:-1]); BID = np.searchsorted(np.array(EDG[1:]), np.arange(n), side="right")
SLACK = 2e-4; UP = 2.0 ** (1 / 16)
def round_up(x): return np.where(x > 0, 2.0 ** (np.ceil(16 * np.log2(np.maximum(x, 1e-30))) / 16), 0).astype(np.float32)
SIG = np.sqrt(LAM); STEP = (C1 * SIG).astype(np.float32)                              # G x n
gb = np.searchsorted(gs, np.arange(G + 1))
Rn = round_up(np.sqrt(np.stack([(Z[:, EDG[i]:EDG[i+1]] ** 2).sum(1) for i in range(NB)], 1))); Rn2 = Rn * Rn   # stored block norms

# ---- profiles: mean squared offset per group coordinate, from pseudo-queries (corpus items) and from held-out prompts
Mu2 = (Mu * Mu).sum(1)
def offsets(q):
    qgK = np.stack([q @ VQ[g] for g in range(G)])[grp]; al = (qgK * Mu).sum(1) / Mu2
    return qgK, al, qgK - al[:, None] * Mu
def profile(Qs):
    P = np.zeros((G, n)); cnt = np.bincount(grp, minlength=G)
    for q in Qs:
        Dl = offsets(q)[2]; np.add.at(P, grp, Dl.astype(np.float64) ** 2)
    P /= (cnt[:, None] * len(Qs)); return np.maximum(P, 1e-3 * P.mean(1, keepdims=True))
rng = np.random.default_rng(5); held = set(np.load(f"{V}/item_q_idx.npy").tolist())
pi = [int(i) for i in rng.choice(1_000_000, 260, replace=False) if int(i) not in held][:250]
PROF = {"item": profile(np.stack([read_rows(i, i + 1)[0] for i in pi]))}
Tt = np.load(f"{V}/text_train.npy").astype(np.float32); Tt /= np.linalg.norm(Tt, axis=1, keepdims=True)
PROF["text"] = profile(Tt[rng.choice(len(Tt), 250, replace=False)])
log("profiles done")

# ---- encode: two int8 planes per coordinate; measured distortions and energy lower bounds per group coordinate
C1S = np.empty((NS, n), np.int8); C2 = np.empty((NS, n), np.int8)
G1 = np.empty((G, n)); G2 = np.empty((G, n)); K1 = np.empty((G, n)); K2 = np.empty((G, n))
def decode(c1s, c2, s):
    """Per-unit increments (m x 2n): reconstruction, squared lower bound on |z|, cell half-width, unbounded flag."""
    k1 = np.abs(c1s).astype(np.float32) - 1; sg = np.sign(c1s).astype(np.float32); un = k1 < 7; k2 = c2.astype(np.float32)
    c1 = sg * s * (k1 + 0.5 * un); lb1 = k1 * s; e1 = 0.5 * s * un
    res = un | (k2 < 15)
    c2v = np.where(un, sg * s * (k1 + (k2 + 0.5) / 16), sg * s * (7 + k2 + 0.5 * (k2 < 15)))
    lb2 = np.where(un, s * (k1 + k2 / 16), s * (7 + k2)); e2 = np.where(un, s / 32, 0.5 * s * (k2 < 15))
    Ac = np.concatenate([c1, c2v - c1], 1); Al = np.concatenate([lb1 * lb1, lb2 * lb2 - lb1 * lb1], 1)
    Aq = np.concatenate([e1, e2 - e1], 1); As = np.concatenate([(~un).astype(np.float32), -((~un) & res).astype(np.float32)], 1)
    return Ac, Al, Aq, As, c1, c2v
ERR = {}
for g in range(G):
    s_, e_ = gb[g], gb[g+1]; Zg = Z[s_:e_]; s = STEP[g][None]; az = np.abs(Zg) / s
    k1 = np.minimum(7, np.floor(az)); sg = np.where(Zg < 0, -1, 1); un = k1 < 7
    k2 = np.where(un, np.minimum(15, np.floor((az - k1) * 16)), np.minimum(15, np.floor(az - 7)))
    C1S[s_:e_] = (sg * (k1 + 1)).astype(np.int8); C2[s_:e_] = k2.astype(np.int8)
    Ac, Al, Aq, As, c1, c2v = decode(C1S[s_:e_], C2[s_:e_], s)
    lam = LAM[g].astype(np.float64)
    G1[g] = ((Zg - c1).astype(np.float64) ** 2).mean(0) / lam; G2[g] = ((Zg - c2v).astype(np.float64) ** 2).mean(0) / lam
    K1[g] = Al[:, :n].astype(np.float64).mean(0) / lam; K2[g] = K1[g] + Al[:, n:].astype(np.float64).mean(0) / lam
    ERR[g] = None
log(f"encoded; mean distortion plane1 {G1.mean():.4f} plane2 {G2.mean():.5f}; energy lower bound share {K1.mean():.3f} {K2.mean():.3f}; unbounded plane-1 cells {np.mean(np.abs(C1S) == 8):.5f}")

# ---- orders
LADB = np.array([0, 2, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256, 384, 512, 768]); LADN = 2 * LADB; NL = len(LADN) - 1   # box variants
LADW = np.array([0, 4, 8, 16, 32, 48, 96, 192, 384, 576, 768]); NLW = len(LADW) - 1                                               # fixW levels
SIDN = np.searchsorted(LADN[1:], np.arange(2 * n), side="right"); SIDW = np.searchsorted(2 * LADW[1:], np.arange(2 * n), side="right")
def order_units(D2, lam, step, k1, k2, cand=None, tail=None):
    """Rows are clusters (or groups). Returns the unit order (rows x 2n): unit j is plane 1 of coordinate j, n + j is plane 2.
    Units are taken by the largest one-step fall of the expected bound, re-ranked at every ladder point; plane 2 of a
    coordinate becomes eligible once its plane 1 is taken. With cand (rows x 2n bool, the same count in every row) only
    those units are ranked; the others follow in the order given by tail."""
    R = len(D2); ad = np.sqrt(D2); rows = np.arange(R)[:, None]
    r1 = np.zeros((R, n), bool); r2 = np.zeros((R, n), bool); O = np.empty((R, 2 * n), np.int32)
    gain = np.empty((R, 2 * n)); L0 = np.add.reduceat(lam, E0, axis=1)
    for j in range(NL):
        need = LADN[j+1] - LADN[j]; at = LADN[j]
        while need > 0:
            Db = np.add.reduceat(D2 * ~r1, E0, axis=1)
            Lb = np.maximum(0, L0 - np.add.reduceat(lam * (k1 * (r1 & ~r2) + k2 * r2), E0, axis=1))
            DbB = Db[:, BID]; LbB = Lb[:, BID]
            g1 = np.sqrt(DbB * LbB) - np.sqrt(np.maximum(0, (DbB - D2) * (LbB - k1 * lam))) - 0.5 * ad * step
            g2 = np.sqrt(DbB) * (np.sqrt(LbB) - np.sqrt(np.maximum(0, LbB - (k2 - k1) * lam))) + (15 / 32) * ad * step
            g1[r1] = -np.inf; g2[~r1 | r2] = -np.inf
            if cand is not None: g1[~cand[:, :n]] = -np.inf; g2[~cand[:, n:]] = -np.inf
            gain[:, :n] = g1; gain[:, n:] = g2
            t = int(min(need, np.isfinite(gain).sum(1).min()))
            if t == 0:
                O[:, at:] = tail[~np.take_along_axis(cand, tail, 1)].reshape(R, -1); return O
            idx = np.argpartition(-gain, t - 1, axis=1)[:, :t]
            idx = np.take_along_axis(idx, np.argsort(-np.take_along_axis(gain, idx, 1), axis=1, kind="stable"), 1)
            O[:, at:at+t] = idx; mk = np.zeros((R, 2 * n), bool); mk[rows, idx] = True; r1 |= mk[:, :n]; r2 |= mk[:, n:]
            at += t; need -= t
    return O
def fix_precedence(O):
    """Make every plane-2 unit follow its plane-1 unit (swap when the order has them reversed)."""
    R, L = O.shape; pos = np.empty((R, L), np.int64); np.put_along_axis(pos, O.astype(np.int64), np.broadcast_to(np.arange(L), (R, L)), 1)
    bad = pos[:, n:] < pos[:, :n]; p1 = np.where(bad, pos[:, n:], pos[:, :n]); p2 = np.where(bad, pos[:, :n], pos[:, n:])
    O2 = np.empty_like(O); np.put_along_axis(O2, p1, np.broadcast_to(np.arange(n), (R, n)).astype(O.dtype), 1)
    np.put_along_axis(O2, p2, np.broadcast_to(np.arange(n, 2 * n), (R, n)).astype(O.dtype), 1); return O2

FIXB = {p: order_units(PROF[p], LAM.astype(np.float64), STEP.astype(np.float64), K1, K2) for p in PROF}
# fixW: greedy on sqrt(Q lambda) times the fall in root distortion; weights D per level; stored norms per item and level
FIXW = {}; WD = {}; NRM = {}
for p in PROF:
    ben = np.concatenate([np.sqrt(PROF[p] * LAM) * (1 - np.sqrt(G1)), np.sqrt(PROF[p] * LAM) * (np.sqrt(G1) - np.sqrt(G2))], 1)
    O = fix_precedence(np.argsort(-ben, axis=1, kind="stable").astype(np.int32)); FIXW[p] = O
    WD[p] = np.empty((G, n, NLW + 1), np.float32); NRM[p] = np.empty((NS, NLW + 1), np.float32)
    for g in range(G):
        s_, e_ = gb[g], gb[g+1]; Zg = Z[s_:e_]
        Ac, Al, Aq, As, c1, c2v = decode(C1S[s_:e_], C2[s_:e_], STEP[g][None])
        e2s = [Zg.astype(np.float64) ** 2, (Zg - c1).astype(np.float64) ** 2, (Zg - c2v).astype(np.float64) ** 2]; lam = LAM[g].astype(np.float64)
        for l in range(NLW + 1):
            rd = np.zeros(2 * n, bool); rd[O[g, :2 * LADW[l]]] = True; depth = rd[:n].astype(int) + rd[n:]
            Ee = lam * np.choose(depth, [np.ones(n), G1[g], G2[g]]); Ee = np.maximum(Ee, 1e-12 * lam.max())
            d2 = np.sqrt(Ee / PROF[p][g]); WD[p][g, :, l] = d2                         # D^2 per coordinate
            nrm2 = sum(e2s[k][:, depth == k] @ (1 / d2[depth == k]) for k in range(3))
            NRM[p][s_:e_, l] = round_up(np.sqrt(nrm2))
CAND = {}
for P in (64, 192):                                                                   # candidates: the first P units of the fixed order
    m_ = np.zeros((G, 2 * n), bool); np.put_along_axis(m_, FIXB["item"][:, :P].astype(np.int64), True, 1); CAND[P] = m_
log("orders and stored norms done")
del Z, D["Z"]

# ---- query loop
FR = [1e-4, 1e-3, 1e-2, 5e-2]; BINS = [0, 0.5, 1, 2, 4, 8, np.inf]; NBIN = len(BINS) - 1
VARS = ["fixW-item", "fixW-text", "fixB-item", "fixB-text", "adapt", "adapt-P64", "adapt-P192"]
VARS = [v for v in VARS if v in os.environ.get("VARS", ",".join(VARS)).split(",")]
SC_W = 3 + LADW + np.arange(NLW + 1); SC_B = 2 + NB + LADB                             # bytes read when decided at each ladder point
FULL = 2 * n
LAMK = LAM[grp].astype(np.float64); STEPK = STEP[grp].astype(np.float64); K1K = K1[grp]; K2K = K2[grp]

def box_bounds(A, uo, dl, base, rn2c):
    Ac, Al, Aq, As = A; m = len(Ac); du = np.concatenate([dl, dl]).astype(np.float32)
    cell = SIDN * NB + BID[uo % n]; p2 = np.argsort(cell, kind="stable"); u2 = uo[p2]; c2 = cell[p2]
    stt = np.concatenate([[0], np.flatnonzero(np.diff(c2)) + 1]); cells = c2[stt]; d2u = du[u2]
    def red(M, wt):
        o = np.zeros((m, NL * NB), np.float32); o[:, cells] = np.add.reduceat(M[:, u2] * wt, stt, axis=1); return o.reshape(m, NL, NB)
    est = np.zeros((m, NL + 1), np.float32); est[:, 1:] = red(Ac, d2u).sum(2).cumsum(1)
    qt = np.zeros((m, NL + 1), np.float32); qt[:, 1:] = red(Aq, np.abs(d2u)).sum(2).cumsum(1)
    low = np.zeros((m, NL + 1, NB), np.float32); low[:, 1:] = red(Al, np.float32(1)).cumsum(1)
    ds = np.zeros((m, NL + 1, NB), np.float32); ds[:, 1:] = red(As, d2u * d2u).cumsum(1)
    dq = np.zeros(NL * NB); dq[cells] = np.add.reduceat((d2u.astype(np.float64) ** 2) * (u2 < n), stt); dqc = dq.reshape(NL, NB).cumsum(0)
    Dtot = np.add.reduceat(dl.astype(np.float64) ** 2, E0); Dun = np.zeros((NL + 1, NB)); Dun[0] = Dtot; Dun[1:] = np.maximum(0, Dtot[None] - dqc)
    w = qt + (np.sqrt(np.maximum(0, Dun[None].astype(np.float32) + ds)) * np.sqrt(np.maximum(0, rn2c[:, None, :] - low))).sum(2) + SLACK
    e = base[:, None] + est
    return np.minimum.accumulate(e + w, 1), np.maximum.accumulate(e - w, 1)

def w_bounds(A, uo, dl, base, hD, nrm):
    Ac = A[0]; m = len(Ac); du = np.concatenate([dl, dl]).astype(np.float32)
    stt = np.searchsorted(SIDW, np.arange(NLW))
    est = np.zeros((m, NLW + 1), np.float32); est[:, 1:] = np.add.reduceat(Ac[:, uo] * du[uo], stt, axis=1).cumsum(1)
    w = hD[None] * nrm + SLACK; e = base[:, None] + est
    return np.minimum.accumulate(e + w, 1), np.maximum.accumulate(e - w, 1)

CB = float(os.environ.get("CB", "0.3")); CK = float(os.environ.get("CK", "10"))      # ns per byte decoded, ns per (item, level) check
def plan_stats(j, sc, jpred=None):
    """Level 0 is always read and checked. Then each cluster jumps to one level l >= 1, checks, and walks on from there.
    j: first ladder index at which each item is decided (len(sc) if never). Returns {name: (bytes, checks)} per item for
    the plain walk (l = 1), the jump that minimises the modelled time knowing j, and the jump chosen from jpred."""
    nl = len(sc) - 1; sce = np.concatenate([sc, [sc[-1] + FULL]]).astype(float); out = {}
    ls = np.arange(1, nl + 1)
    mx = np.maximum(j[:, None], ls[None]); by = np.where(j[:, None] == 0, sce[0], sce[mx]); ck = np.where(j[:, None] == 0, 1, 2 + np.maximum(0, j[:, None] - ls[None]))
    tm = CB * by + CK * ck; out["walk"] = (by[:, 0].mean(), ck[:, 0].mean())
    best = np.add.reduceat(tm, st, axis=0).argmin(1); pick = np.repeat(best, en - st); r = np.arange(len(j))
    out["jump-oracle"] = (by[r, pick].mean(), ck[r, pick].mean())
    if jpred is not None:
        mxp = np.maximum(jpred[:, None], ls[None]); byp = np.where(jpred[:, None] == 0, sce[0], sce[mxp]); ckp = np.where(jpred[:, None] == 0, 1, 2 + np.maximum(0, jpred[:, None] - ls[None]))
        bp = np.add.reduceat(CB * byp + CK * ckp, st, axis=0).argmin(1); pick = np.repeat(bp, en - st)
        out["jump-predicted"] = (by[r, pick].mean(), ck[r, pick].mean())
    return out
PLAN = {kd: {} for kd in ("text", "item")}
RAT = {p: np.stack([np.median(NRM[p][gb[g]:gb[g+1]] / np.maximum(NRM[p][gb[g]:gb[g+1], :1], 1e-9), axis=0) for g in range(G)]) for p in PROF}
acc = {kd: {v: dict(cost=np.zeros(len(FR)), band=np.zeros((len(FR), NBIN)), und0=np.zeros(len(FR)), full=np.zeros(len(FR))) for v in VARS} for kd in ("text", "item")}
bandn = {kd: np.zeros((len(FR), NBIN)) for kd in ("text", "item")}; nqk = {"text": 0, "item": 0}; viol = {v: 0 for v in VARS}; checks = 0
blk_hist = np.zeros((2, NB)); nh = 0
for qn, (q, kd) in enumerate(zip(Q, kind)):
    if qn % 3 == 0: wait_lock()
    s = S[:, qn]; sig = float(s.std()); med = float(np.median(s)); qm = Mean @ q
    qgK, al, Dl = offsets(q); D2 = Dl.astype(np.float64) ** 2
    U = {}; L = {}
    for v in VARS:
        nl = NLW if v.startswith("fixW") else NL; U[v] = np.empty((NS, nl + 1), np.float32); L[v] = np.empty((NS, nl + 1), np.float32)
    OA = {}
    for v in VARS:
        if v == "adapt": OA[v] = order_units(D2, LAMK, STEPK, K1K, K2K)
        elif v.startswith("adapt-P"): OA[v] = order_units(D2, LAMK, STEPK, K1K, K2K, CAND[int(v[7:])][grp], FIXB["item"][grp])
    if "adapt" in VARS:
        first = OA["adapt"][:, :64]; np.add.at(blk_hist, ((first >= n).astype(int).ravel(), BID[first % n].ravel()), 1); nh += first.size
    hD = {p: np.sqrt(np.einsum("kj,kjl->kl", D2, WD[p][grp].astype(np.float64))).astype(np.float32) for p in PROF if f"fixW-{p}" in VARS}
    for k in range(len(cl)):
        c = cl[k]; a, e = st[k], en[k]; g = grp[c]; base = qm[c] + al[c] * b[a:e]
        A = decode(C1S[a:e], C2[a:e], STEP[g][None])[:4]
        for v in VARS:
            if v.startswith("adapt"): U[v][a:e], L[v][a:e] = box_bounds(A, OA[v][c], Dl[c], base, Rn2[a:e])
            elif v.startswith("fixB"): U[v][a:e], L[v][a:e] = box_bounds(A, FIXB[v[5:]][g], Dl[c], base, Rn2[a:e])
            else: p = v[5:]; U[v][a:e], L[v][a:e] = w_bounds(A, FIXW[p][g], Dl[c], base, hD[p][c], NRM[p][a:e])
    ths = [np.partition(s, NS - round(f * NS))[NS - round(f * NS)] for f in FR]
    for v in VARS:
        viol[v] += int(((s[:, None] > U[v]) | (s[:, None] < L[v])).sum()); checks += U[v].size
        sc = SC_W if v.startswith("fixW") else SC_B
        for fi, t in enumerate(ths):
            dec = (L[v] >= t) | (U[v] < t); u = np.abs(s - t) / sig
            viol[v] += int((((L[v] >= t) & (s[:, None] < t)) | ((U[v] < t) & (s[:, None] >= t))).sum())
            none = ~dec.any(1); j = dec.argmax(1); cost = sc[j].astype(float); cost[none] = sc[-1] + FULL
            A_ = acc[kd][v]; A_["cost"][fi] += cost.mean(); A_["band"][fi] += np.histogram(u, BINS, weights=cost)[0] / NS
            A_["und0"][fi] += 1 - dec[:, 0].mean(); A_["full"][fi] += none.mean()
            if v in ("fixW-item", "adapt", "adapt-P192"):
                jj = np.where(none, len(sc), j); jp = None
                if v == "fixW-item":                                                  # predicted level from level-0 data only
                    base0 = 0.5 * (U[v][:, 0] + L[v][:, 0]); hh = hD["item"][cs] * (NRM["item"][:, :1] * RAT["item"][gs])
                    okp = hh < 0.8 * np.abs(t - base0)[:, None]; jp = np.where(okp.any(1), okp.argmax(1), len(sc))
                for nm, (by_, ck_) in plan_stats(jj, sc, jp).items():
                    P_ = PLAN[kd].setdefault(v, {}).setdefault(nm, np.zeros((len(FR), 2))); P_[fi] += (by_, ck_)
    for fi, t in enumerate(ths): bandn[kd][fi] += np.histogram(np.abs(s - t) / sig, BINS)[0] / NS
    nqk[kd] += 1
    if qn % 5 == 0: log(f"query {qn+1}/{len(Q)} ({kd}); violations {sum(viol.values())}")

np.set_printoptions(precision=1, suppress=True, linewidth=200)
res = dict(basis=BASIS, c1=C1, blocks=EDG, nq=nqk, violations=viol, checks=checks, out={},
           adapt_first64_units_by_block=(blk_hist / max(nh, 1)).tolist())
for kd in ("text", "item"):
    if not nqk[kd]: continue
    q_ = nqk[kd]; res["out"][kd] = dict(band_items=(bandn[kd] / q_).tolist(), variants={})
    print(f"\n== {kd} queries ({q_}), basis {BASIS}, blocks {NB}, bytes per item (scalars included)")
    print("   items by band (%), cols = |s-t|/sd in", BINS); print(100 * bandn[kd] / q_)
    for v in VARS:
        A_ = acc[kd][v]; c = A_["cost"] / q_; bc = A_["band"] / q_
        print(f" {v:10s}: bytes/item {np.round(c, 1)}  undecided at zero read {np.round(100 * A_['und0'] / q_, 1)}%  full vector read {np.round(100 * A_['full'] / q_, 3)}%")
        with np.errstate(invalid="ignore", divide="ignore"): pb = bc / (bandn[kd] / q_)
        print("      bytes per item in band:"); print(np.round(pb, 1)); print("      share of cost by band (%):"); print(np.round(100 * bc / c[:, None], 1))
        res["out"][kd]["variants"][v] = dict(cost=c.tolist(), band_cost=bc.tolist(), und0=(A_["und0"] / q_).tolist(), full=(A_["full"] / q_).tolist())
    for v in PLAN[kd]:
        print(f" schedule for {v} (bytes per item, checks per item, modelled ns per item at {CB} ns/B and {CK} ns/check):")
        for nm, P_ in PLAN[kd][v].items():
            P_ = P_ / q_; print(f"   {nm:15s} bytes {np.round(P_[:, 0], 1)} checks {np.round(P_[:, 1], 2)} ns {np.round(CB * P_[:, 0] + CK * P_[:, 1], 1)}")
            res["out"][kd].setdefault("plan", {}).setdefault(v, {})[nm] = P_.tolist()
print("\nadaptive order, first 64 units: share by block, plane 1 then plane 2"); print(np.round(100 * blk_hist / max(nh, 1), 1))
print("violations", viol, "interval checks", checks)
json.dump(res, open(OUT, "w")); log("done")
