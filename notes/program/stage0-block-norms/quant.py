# Stage 0, block norms: the same bounds on a quantised code, so that undecided share can be read against bits.
# Data-only dual basis per group; each coordinate z_i coded by a b_i-bit Lloyd-Max quantiser for N(0, lam_i);
# bits given greedily to the coordinate with the largest profile-weighted gain, P_i lam_i (dist(b) - dist(b+1)),
# P the data-only profile, at most 8 bits per coordinate, nested over the budgets 4/8/12/16/24/32 bytes.
# The coding error e = z - zhat is on every coordinate; the level's weights are W_i = (lam_i dist(b_i) / P_i)^(1/4).
# Bounds on |u.e| (u the query offset in the query-side basis):
#   a     one norm, |W u| |W^-1 e|
#   cJ    J block norms, blocks cut over [0, n) by equal counts (cnt) or equal shares of the error variance (var)
#   d     per-coordinate absolute values
#   b2    the mixed (text) fit: mixed Sigma_Q basis, allocation and weights from the mixed profile, one norm; b2d its limit
#   "r"   item norms rounded up to the 2^(1/16) grid.
# This is a model quantiser, not the built code (tree fields and 1-bit stages, int8 basis, 16-bit tables).
# usage: quant.py WORKDIR GROUPS
import numpy as np, sys, time, os
W = sys.argv[1]; t0 = time.time()
def log(*a): print(f"[{time.time()-t0:6.0f}s]", *a, flush=True)
DD = "/home/joe/code/vexact-data/bioclip2-200k/"
F = [np.load(DD + f"g{i}.npy", mmap_mode="r") for i in range(4)]
s0 = np.load(f"{W}/setup.npz"); a = s0["a"]; Mean = s0["Mean"]; grp = s0["grp"]; Q = s0["Q"]; TH = s0["TH"]; OFF = s0["OFF"]
kind = s0["kind"]; n = Q.shape[1]; nq = len(Q); G = int(grp.max()) + 1
GROUPS = list(range(G)) if sys.argv[2] == "all" else [int(x) for x in sys.argv[2].split(",")]
BY = [4, 8, 12, 16, 24, 32]; JS = [2, 4, 8, 16]; EPS = 0.1; SLACK = 1e-9; STEP = 2 ** (1 / 16); BMAX = 8
def ridge(S): return S + EPS * np.trace(S) / n * np.eye(n)
def sqrtm(S):
    l, v = np.linalg.eigh(S); return (v * np.sqrt(l)) @ v.T, (v / np.sqrt(l)) @ v.T
AQ = {"d": sqrtm(ridge(s0["Si"])), "m": sqrtm(ridge(s0["Sm"]))}
# Lloyd-Max quantisers for the standard normal
xg = np.linspace(-9, 9, 360_001); wg = np.exp(-xg ** 2 / 2); wg /= wg.sum()
CEN = {}; DIST = np.ones(BMAX + 1)
for b in range(1, BMAX + 1):
    c = np.quantile(np.random.default_rng(b).standard_normal(200_000), (np.arange(2 ** b) + 0.5) / 2 ** b)
    for it in range(100):
        cut = (c[1:] + c[:-1]) / 2; j = np.searchsorted(cut, xg)
        c = np.bincount(j, wg * xg, 2 ** b) / np.maximum(np.bincount(j, wg, 2 ** b), 1e-300)
    cut = (c[1:] + c[:-1]) / 2; j = np.searchsorted(cut, xg); DIST[b] = float((wg * (xg - c[j]) ** 2).sum()); CEN[b] = (c, cut)
def quantise(z, b):                                    # z: items x n in units of sd; b: bits per coordinate
    zh = np.zeros_like(z)
    for bb in range(1, BMAX + 1):
        cols = np.flatnonzero(b == bb)
        if len(cols): c, cut = CEN[bb]; zh[:, cols] = c[np.searchsorted(cut, z[:, cols])]
    return zh
def alloc(lam, P, bits_list):
    b = np.zeros(n, int); out = []; gain = P * lam * (DIST[0] - DIST[1])
    for target in bits_list:
        while b.sum() < target:
            i = int(gain.argmax()); b[i] += 1
            gain[i] = P[i] * lam[i] * (DIST[b[i]] - DIST[b[i] + 1]) if b[i] < BMAX else -1
        out.append(b.copy())
    return out
def rows(idx):
    out = np.empty((len(idx), n), np.float64)
    for fi in range(4):
        m = (idx >= OFF[fi]) & (idx < OFF[fi + 1])
        if m.any(): out[m] = F[fi][idx[m] - OFF[fi]]
    out /= np.linalg.norm(out, axis=1, keepdims=True); return out
def rup(x): return STEP ** np.ceil(np.log(np.maximum(x, 1e-300)) / np.log(STEP))
NAMES = ["a", "b2"] + [f"c{c}{J}" for c in ("cnt", "var") for J in JS] + ["d", "b2d", "ar", "cvar4r", "cvar16r", "ccnt4r", "ccnt16r"]
NB = len(NAMES); NL = len(BY)
log("Lloyd-Max distortions", DIST.round(5).tolist())
for g in GROUPS:
    out = f"{W}/q{g:02d}.npz"
    if os.path.exists(out): log("group", g, "done before; skipped"); continue
    cl = np.flatnonzero(grp == g); idx_g = np.flatnonzero(np.isin(a, cl))
    SR = np.zeros((n, n))
    for c in cl:
        R = rows(idx_g[a[idx_g] == c]) - Mean[c]; SR += R.T @ R
    SR /= len(idx_g)
    B = {}
    for key, (Aq, Aqi), S in (("d", AQ["d"], s0["Si"]), ("m", AQ["m"], s0["Sm"])):
        gam, V = np.linalg.eigh(Aq @ SR @ Aq); gam = np.maximum(gam[::-1], 1e-14 * gam.max()); V = V[:, ::-1]
        VD = (Aq @ V) * gam ** -0.25; VQ = (Aqi @ V) * gam ** 0.25
        lam = np.diag(VD.T @ SR @ VD); P = np.maximum(np.diag(VQ.T @ S @ VQ), 1e-30)
        bl = alloc(lam, P, [8 * x for x in BY]); ev = [lam * DIST[b] for b in bl]
        B[key] = dict(VD=VD, VQ=VQ, sd=np.sqrt(lam), bits=bl, ev=ev, Wt=[(e / P) ** 0.25 for e in ev])
    def edges(J, how, ev):
        if how == "cnt": return np.round(np.linspace(0, n, J + 1)).astype(int)
        cs = np.concatenate([[0], np.cumsum(ev)]); e = np.concatenate([[0], np.searchsorted(cs, np.linspace(0, cs[-1], J + 1)[1:-1]), [n]])
        for i in range(1, J): e[i] = min(max(e[i], e[i - 1] + 1), n - (J - i))
        return e
    st = dict(sum_h=np.zeros((NB, NL, nq)), sum_act=np.zeros((NB, NL, nq)), und=np.zeros((NB, NL, nq, TH.shape[1]), np.int64),
              viol=np.zeros((NB, NL), np.int64), excess=np.full((NB, NL), -np.inf), npair=np.zeros(nq, np.int64))
    work = [(c, ch) for c in cl for ii_ in [idx_g[a[idx_g] == c]] for ch in np.array_split(ii_, -(-len(ii_) // 500))]
    for c, ii in work:
        X = rows(ii); m = Mean[c]; R = X - m; mr = R @ m
        s = X @ Q.T; qm = Q @ m; al = qm / (m @ m); Dq = Q - al[:, None] * m; base = qm[None] + al[None] * mr[:, None]
        st["npair"] += len(ii)
        Zs = {key: (R @ B[key]["VD"], Dq @ B[key]["VQ"]) for key in ("d", "m")}
        for il in range(NL):
            H = {}; known = {}; ref = {}
            for key in ("d", "m"):
                Z, Gq = Zs[key]; sd = B[key]["sd"]; E = Z - sd * quantise(Z / sd, B[key]["bits"][il]); Wt = B[key]["Wt"][il]
                known[key] = base + (Z - E) @ Gq.T; ref[key] = np.abs(E) @ np.abs(Gq).T
                iz = np.sqrt(((E / Wt) ** 2).sum(1)); qz = np.sqrt(((Gq * Wt) ** 2).sum(1))
                nm = "a" if key == "d" else "b2"; H[nm] = np.outer(iz, qz)
                if key == "d":
                    H["ar"] = np.outer(rup(iz), qz)
                    for how in ("cnt", "var"):
                        for J in JS:
                            e = edges(J, how, B["d"]["ev"][il])
                            ib = np.stack([np.sqrt(((E[:, e[j]:e[j + 1]] / Wt[e[j]:e[j + 1]]) ** 2).sum(1)) for j in range(J)], 1)
                            qb = np.stack([np.sqrt(((Gq[:, e[j]:e[j + 1]] * Wt[e[j]:e[j + 1]]) ** 2).sum(1)) for j in range(J)], 1)
                            H[f"c{how}{J}"] = ib @ qb.T
                            if J in (4, 16): H[f"c{how}{J}r"] = rup(ib) @ qb.T
            H["d"] = ref["d"]; H["b2d"] = ref["m"]
            for ib_, nm in enumerate(NAMES):
                key = "m" if nm.startswith("b2") else "d"; kn = known[key]; act = np.abs(s - kn); h = H[nm]
                ex = act - h; st["viol"][ib_, il] += int((ex > SLACK).sum()); st["excess"][ib_, il] = max(st["excess"][ib_, il], float(ex.max()))
                st["sum_h"][ib_, il] += h.sum(0); st["sum_act"][ib_, il] += act.sum(0)
                L = kn - h; U = kn + h
                for it in range(TH.shape[1]):
                    t = TH[:, it][None]; st["und"][ib_, il, :, it] += ((L < t) & (U >= t)).sum(0)
    st["bits_d"] = np.stack(B["d"]["bits"]); st["bits_m"] = np.stack(B["m"]["bits"])
    np.savez(out, names=np.array(NAMES), by=np.array(BY), **st)
    log(f"group {g}: {len(idx_g)} items; violations {int(st['viol'].sum())}; coordinates with bits at 32 B {int((st['bits_d'][-1] > 0).sum())}")
log("done")
