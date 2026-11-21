# Stage 0, block norms: half-widths, undecided shares and violations of the tail bounds, idealised setting.
# A level reads the first k coordinates of the group's dual basis exactly; the unread term u.e (u the query offset's
# tail, e the item's tail) is bounded by
#   a   one weighted norm, weights D from the data-only profile           |D u| |D^-1 e|
#   b1  one weighted norm, same basis, D from the text training prompts' profile
#   b2  the fit with text (mixed Sigma_Q basis, mixed profile): its own first k coordinates, one norm
#   c   J block norms on the data-only basis, D inside each block (J = 1 is a): sum_j |D u_j| |D^-1 e_j|,
#       blocks cut over [k, n) by equal counts (cnt) or equal shares of the group's item variance (var)
#   d   per-coordinate absolute values, sum_i |u_i||e_i|: the best diagonal weighting for each pair
#   b2c the mixed basis with block norms (J = 4, 16, equal variance), b2d its per-coordinate limit
# and each stored-norm bound again with every item norm rounded up to a grid of steps 2^(1/16) ("r" suffix).
# Every bound is checked against the exact score of every (query, item) pair.
# usage: measure.py WORKDIR GROUPS(comma list or 'all')
import numpy as np, sys, time, os
W = sys.argv[1]; t0 = time.time()
def log(*a): print(f"[{time.time()-t0:6.0f}s]", *a, flush=True)
DD = "/home/joe/code/vexact-data/bioclip2-200k/"
F = [np.load(DD + f"g{i}.npy", mmap_mode="r") for i in range(4)]
s0 = np.load(f"{W}/setup.npz"); a = s0["a"]; Mean = s0["Mean"]; grp = s0["grp"]; Q = s0["Q"]; TH = s0["TH"]; OFF = s0["OFF"]
kind = s0["kind"]; N = len(a); n = Q.shape[1]; nq = len(Q); K = len(Mean); G = int(grp.max()) + 1
GROUPS = list(range(G)) if sys.argv[2] == "all" else [int(x) for x in sys.argv[2].split(",")]
KS = [0, 8, 16, 32, 64, 128, 256]; JS = [2, 4, 8, 16]; EPS = 0.1; SLACK = 1e-9; STEP = 2 ** (1 / 16)
def ridge(S): return S + EPS * np.trace(S) / n * np.eye(n)
def sqrtm(S):
    l, v = np.linalg.eigh(S); return (v * np.sqrt(l)) @ v.T, (v / np.sqrt(l)) @ v.T
AQ = {"d": sqrtm(ridge(s0["Si"])), "m": sqrtm(ridge(s0["Sm"]))}
def rows(idx):
    out = np.empty((len(idx), n), np.float64)
    for fi in range(4):
        m = (idx >= OFF[fi]) & (idx < OFF[fi + 1])
        if m.any(): out[m] = F[fi][idx[m] - OFF[fi]]
    out /= np.linalg.norm(out, axis=1, keepdims=True); return out
def rup(x): return STEP ** np.ceil(np.log(np.maximum(x, 1e-300)) / np.log(STEP))   # round up to the 2^(1/16) grid
NAMES = ["a", "b1", "b2"] + [f"c{c}{J}" for c in ("cnt", "var") for J in JS] + ["d", "b2c4", "b2c16", "b2d"]
NAMES += [nm + "r" for nm in ("a", "b1", "b2", "ccnt4", "ccnt16", "cvar4", "cvar16")]
NB = len(NAMES); NK = len(KS); LB = np.linspace(-1, 13, 281)          # log2 bins for per-pair ratios
for g in GROUPS:
    out = f"{W}/g{g:02d}.npz"
    if os.path.exists(out): log("group", g, "done before; skipped"); continue
    cl = np.flatnonzero(grp == g); idx_g = np.flatnonzero(np.isin(a, cl))
    # residual covariance of the group, cluster by cluster
    SR = np.zeros((n, n));
    for c in cl:
        ii = idx_g[a[idx_g] == c]; R = rows(ii) - Mean[c]; SR += R.T @ R
    SR /= len(idx_g)
    B = {}
    for key, (Aq, Aqi) in AQ.items():
        gam, V = np.linalg.eigh(Aq @ SR @ Aq); gam = np.maximum(gam[::-1], 1e-14 * gam.max()); V = V[:, ::-1]
        VD = (Aq @ V) * gam ** -0.25; VQ = (Aqi @ V) * gam ** 0.25
        lam = np.diag(VD.T @ SR @ VD)                                  # item-side variance per coordinate (= sqrt(gam))
        B[key] = dict(VD=VD, VQ=VQ, lam=lam, gam=gam)
    pr = lambda key, S: np.maximum(np.diag(B[key]["VQ"].T @ S @ B[key]["VQ"]), 1e-30)
    Dw = {"a": (B["d"]["lam"] / pr("d", s0["Si"])) ** 0.25, "b1": (B["d"]["lam"] / pr("d", s0["St"])) ** 0.25,
          "b2": (B["m"]["lam"] / pr("m", s0["Sm"])) ** 0.25}
    # block edges per level k, over the unread positions [k, n)
    def edges(k, J, how, lam):
        if how == "cnt": e = k + np.round(np.linspace(0, n - k, J + 1)).astype(int)
        else:
            cs = np.concatenate([[0], np.cumsum(lam[k:])]); e = k + np.searchsorted(cs, np.linspace(0, cs[-1], J + 1)[1:-1])
            e = np.concatenate([[k], e, [n]])
            for i in range(1, J): e[i] = min(max(e[i], e[i - 1] + 1), n - (J - i))
        return e
    EDG = {(k, how, J): edges(k, J, how, B["d"]["lam"]) for k in KS for how in ("cnt", "var") for J in JS}
    EDM = {(k, J): edges(k, J, "var", B["m"]["lam"]) for k in KS for J in (4, 16)}
    st = dict(sum_h=np.zeros((NB, NK, nq)), sum_rb=np.zeros((NB, NK, nq)), sum_ra=np.zeros((NB, NK, nq)), sum_act=np.zeros((NB, NK, nq)),
              und=np.zeros((NB, NK, nq, TH.shape[1]), np.int64), viol=np.zeros((NB, NK), np.int64), excess=np.full((NB, NK), -np.inf),
              hist_act=np.zeros((NB, NK, 3, len(LB) + 1), np.int64), hist_best=np.zeros((NB, NK, 3, len(LB) + 1), np.int64),
              hist_a=np.zeros((NB, NK, 3, len(LB) + 1), np.int64), npair=np.zeros(nq, np.int64), maxid=0.0)
    work = [(c, ch) for c in cl for ii_ in [idx_g[a[idx_g] == c]] for ch in np.array_split(ii_, -(-len(ii_) // 500))]
    for c, ii in work:
        X = rows(ii); m = Mean[c]; R = X - m; mr = R @ m
        s = X @ Q.T; qm = Q @ m; al = qm / (m @ m); Dq = Q - al[:, None] * m      # offset, orthogonal to the centre
        st["npair"] += len(ii)
        per = {}
        for key in ("d", "m"):
            Z = R @ B[key]["VD"]; Gq = Dq @ B[key]["VQ"]
            # products by stretch of positions between consecutive k, so each level's tail is a suffix sum
            bnd = KS + [n]
            P = np.stack([Z[:, bnd[i]:bnd[i + 1]] @ Gq[:, bnd[i]:bnd[i + 1]].T for i in range(NK)])          # NK x items x nq
            Pa = np.stack([np.abs(Z[:, bnd[i]:bnd[i + 1]]) @ np.abs(Gq[:, bnd[i]:bnd[i + 1]]).T for i in range(NK)])
            tail = np.cumsum(P[::-1], 0)[::-1]; atail = np.cumsum(Pa[::-1], 0)[::-1]
            base = qm[None] + al[None] * mr[:, None]
            st["maxid"] = max(st["maxid"], float(np.abs(base + tail[0] - s).max()))
            per[key] = (Z, Gq, tail, atail, base)
        for ik, k in enumerate(KS):
            H = {}
            for key, wkey in (("d", "a"), ("d", "b1"), ("m", "b2")):
                Z, Gq, *_ = per[key]; D = Dw[wkey]
                iz = np.sqrt(((Z[:, k:] / D[k:]) ** 2).sum(1)); qz = np.sqrt(((Gq[:, k:] * D[k:]) ** 2).sum(1))
                H[wkey] = np.outer(iz, qz); H[wkey + "r"] = np.outer(rup(iz), qz)
            hs = slice(None, None, 4)                                       # per-pair histograms on every fourth item
            def blocks(key, D, e):
                Z, Gq, *_ = per[key]
                ib = np.stack([np.sqrt(((Z[:, e[j]:e[j + 1]] / D[e[j]:e[j + 1]]) ** 2).sum(1)) for j in range(len(e) - 1)], 1)
                qb = np.stack([np.sqrt(((Gq[:, e[j]:e[j + 1]] * D[e[j]:e[j + 1]]) ** 2).sum(1)) for j in range(len(e) - 1)], 1)
                return ib @ qb.T, rup(ib) @ qb.T
            for how in ("cnt", "var"):
                for J in JS:
                    H[f"c{how}{J}"], hr = blocks("d", Dw["a"], EDG[k, how, J])
                    if J in (4, 16): H[f"c{how}{J}r"] = hr
            for J in (4, 16): H[f"b2c{J}"], H[f"b2c{J}r"] = blocks("m", Dw["b2"], EDM[k, J])
            H["d"] = per["d"][3][ik]; H["b2d"] = per["m"][3][ik]
            ref = {"d": per["d"][3][ik], "m": per["m"][3][ik]}
            for ib, nm in enumerate(NAMES):
                key = "m" if nm.startswith("b2") else "d"
                Z, Gq, tail, atail, base = per[key]
                known = base + (tail[0] - tail[ik]); act = np.abs(s - known); h = H[nm]
                ex = act - h; st["viol"][ib, ik] += int((ex > SLACK).sum()); st["excess"][ib, ik] = max(st["excess"][ib, ik], float(ex.max()))
                st["sum_h"][ib, ik] += h.sum(0); st["sum_act"][ib, ik] += act.sum(0)
                rb = h / np.maximum(ref[key], 1e-300); st["sum_rb"][ib, ik] += rb.sum(0)
                ra = h / np.maximum(H["b2" if key == "m" else "a"], 1e-300); st["sum_ra"][ib, ik] += ra.sum(0)
                for kd in range(3):
                    qs = kind == kd
                    st["hist_act"][ib, ik, kd] += np.bincount(np.searchsorted(LB, np.log2(h[hs][:, qs] / np.maximum(act[hs][:, qs], 1e-300)).ravel()), minlength=len(LB) + 1)
                    st["hist_best"][ib, ik, kd] += np.bincount(np.searchsorted(LB, np.log2(rb[hs][:, qs]).ravel()), minlength=len(LB) + 1)
                    st["hist_a"][ib, ik, kd] += np.bincount(np.searchsorted(LB, np.log2(np.maximum(ra[hs][:, qs], 2.0 ** -1.5)).ravel()), minlength=len(LB) + 1)
                L = known - h; U = known + h
                for it in range(TH.shape[1]):
                    t = TH[:, it][None]; st["und"][ib, ik, :, it] += ((L < t) & (U >= t)).sum(0)
        del per
    st["gam_d"] = B["d"]["gam"]; st["gam_m"] = B["m"]["gam"]; st["Dw"] = np.stack([Dw["a"], Dw["b1"], Dw["b2"]])
    st["edges_var"] = np.array([np.pad(EDG[k, "var", 16], (0, 0)) for k in KS])
    np.savez(out, names=np.array(NAMES), ks=np.array(KS), **st)
    log(f"group {g}: {len(cl)} clusters, {len(idx_g)} items; identity error {st['maxid']:.2e}; violations {int(st['viol'].sum())}; "
        f"text mean h at k=32: a {st['sum_h'][0, 3, kind == 0].sum() / st['npair'][0] / (kind == 0).sum():.4f} "
        f"cvar4 {st['sum_h'][NAMES.index('cvar4'), 3, kind == 0].sum() / st['npair'][0] / (kind == 0).sum():.4f} "
        f"d {st['sum_h'][NAMES.index('d'), 3, kind == 0].sum() / st['npair'][0] / (kind == 0).sum():.4f}")
log("done")
