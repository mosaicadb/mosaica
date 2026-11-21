# Partition experiment: shared code. Rows, split, Sigma_Q, the build's k-means, groups, bases, the level model and
# every score of EXPERIMENT-SPEC.md section 2. Nothing is timed.
import numpy as np, time, os, json
import scipy.sparse as sp

EXP = os.path.dirname(os.path.abspath(__file__))
WORK = EXP + "/work"
os.makedirs(WORK + "/parts", exist_ok=True)
os.makedirs(WORK + "/scores", exist_ok=True)
SETUP = os.path.normpath(EXP + "/../../stage0-block-norms/work/setup.npz")
DD = "/home/joe/code/vexact-data/bioclip2-200k/"
TD = "/home/joe/code/vexact-data/bioclip2-text/"
_t0 = time.time()


def log(*a):
    print(f"[{time.time() - _t0:7.0f}s]", *a, flush=True)


s0 = np.load(SETUP)
Q = s0["Q"]
KIND = s0["kind"]
TH = s0["TH"]
QI = s0["qi"]
N = len(s0["a"])
n = Q.shape[1]
NQ = len(Q)
FRS = ["0.01%", "0.1%", "1%"]           # TH columns 0, 1, 2
KINDS = ["text", "species", "item"]
perm = np.random.default_rng(1).permutation(N)
POS = np.empty(N, np.int64)
POS[perm] = np.arange(N)
VALID = np.ones(N, bool)
VALID[QI] = False                         # the item queries' own rows are removed from every count
HELD = VALID & (POS % 4 == 0)
FIT = VALID & (POS % 4 != 0)
NV = int(VALID.sum())
FLOOR = 3 * n                             # least fit rows a group may hold
LEVELS = [4, 8, 16, 32]
WSTARS = [0.03, 0.06, 0.12]


def load_X():
    X = np.empty((N, n), np.float32)
    o = 0
    for i in range(4):
        f = np.load(DD + f"g{i}.npy", mmap_mode="r")
        for s in range(0, len(f), 10_000):
            x = np.asarray(f[s:s + 10_000], np.float64)
            x /= np.linalg.norm(x, axis=1, keepdims=True)
            X[o:o + len(x)] = x
            o += len(x)
    assert o == N
    return X


def scores_T(X):
    """Exact scores q.x for every query and row (float64 products, stored float32), NQ x N."""
    p = WORK + "/scoresT.npy"
    if not os.path.exists(p):
        S = np.lib.format.open_memmap(p + ".tmp.npy", mode="w+", dtype=np.float32, shape=(NQ, N))
        for s in range(0, N, 10_000):
            S[:, s:s + 10_000] = (X[s:s + 10_000].astype(np.float64) @ Q.T).T
        S.flush()
        del S
        os.replace(p + ".tmp.npy", p)
    return np.load(p, mmap_mode="r")


def Amat(S):
    Sr = S + 0.1 * np.trace(S) / n * np.eye(n)
    l, v = np.linalg.eigh(Sr)
    return (v * np.sqrt(l)) @ v.T


A_SI = Amat(s0["Si"])
A_SM = Amat(s0["Sm"])
AS = {"Si": A_SI, "Sm": A_SM}


def kmeans(S, K, rng, iters=12):
    """combined/core.py::kmeans, copied."""
    C = S[rng.choice(len(S), K, replace=False)].copy()
    for _ in range(iters):
        a = np.concatenate([(S[i:i + 50_000] @ C.T).argmax(1) for i in range(0, len(S), 50_000)])
        M = np.zeros_like(C)
        np.add.at(M, a, S)
        cnt = np.bincount(a, minlength=K)
        empty = cnt == 0
        M[empty] = S[rng.choice(len(S), int(empty.sum()))]
        C = M / np.linalg.norm(M, axis=1, keepdims=True)
    return C


def unit(M):
    return M / np.linalg.norm(M, axis=1, keepdims=True)


def argmax_ip(X, C, idx=None):
    idx = np.arange(N) if idx is None else idx
    C32 = C.astype(np.float32)
    return np.concatenate([(X[idx[i:i + 50_000]] @ C32.T).argmax(1) for i in range(0, len(idx), 50_000)]).astype(np.int32)


def cell_sums(X, cell, K, mask):
    idx = np.flatnonzero(mask)
    S = np.zeros((K, n))
    for i in range(0, len(idx), 20_000):
        ii = idx[i:i + 20_000]
        O = sp.csr_matrix((np.ones(len(ii)), (cell[ii], np.arange(len(ii)))), shape=(K, len(ii)))
        S += O @ X[ii].astype(np.float64)
    cnt = np.bincount(cell[idx], minlength=K).astype(np.float64)
    return S, cnt


def means(X, cell, K, mask=None):
    S, cnt = cell_sums(X, cell, K, FIT if mask is None else mask)
    assert (cnt > 0).all(), "a cell has no fit rows"
    return S / cnt[:, None], cnt


def compact(cell, grp=None):
    """Renumber cells (and groups) so that none is empty over all rows."""
    keep = np.unique(cell)
    remap = -np.ones(cell.max() + 1, np.int64)
    remap[keep] = np.arange(len(keep))
    cell = remap[cell].astype(np.int32)
    if grp is not None:
        grp = np.asarray(grp)[keep]
        gk = np.unique(grp)
        gm = -np.ones(grp.max() + 1, np.int64)
        gm[gk] = np.arange(len(gk))
        grp = gm[grp].astype(np.int32)
    return cell, grp


def build_groups(M, G, rng):
    """The build's rule: k-means (20 passes) on the unit centre directions, each cell to its nearest group centre."""
    Mh = unit(M).astype(np.float32)
    C = kmeans(Mh.copy(), G, rng, 20)
    grp = (Mh @ C.T).argmax(1)
    gk = np.unique(grp)
    gm = -np.ones(G, np.int64)
    gm[gk] = np.arange(len(gk))
    return gm[grp].astype(np.int32)


def scatters(X, cell, K, M, mask):
    """Per cell, sum over its rows in mask of (x - m_c)(x - m_c)^T (float64), and counts."""
    idx = np.flatnonzero(mask)
    idx = idx[np.argsort(cell[idx], kind="stable")]
    cnt = np.bincount(cell[idx], minlength=K)
    st = np.concatenate([[0], np.cumsum(cnt)])
    C = np.zeros((K, n, n))
    for c in range(K):
        if cnt[c] == 0:
            continue
        R = X[idx[st[c]:st[c + 1]]].astype(np.float64) - M[c]
        C[c] = R.T @ R
    return C, cnt


def basis(Sig, A):
    gam, U = np.linalg.eigh(A @ Sig @ A)
    gam, U = gam[::-1], U[:, ::-1]
    gam = np.maximum(gam, 1e-12 * gam[0])
    T = (A @ U) * gam ** -0.25
    return gam, T


def level(gam, B):
    """Water-filling at B bytes: (s, W, theta). B = 0 reads nothing: s = 1."""
    sg = np.sqrt(gam)
    if B == 0:
        return np.ones_like(gam), sg.sum(), np.inf
    H = 8 * B * np.log(2)
    lg = np.log(gam)
    lo, hi = lg[-1] - 60.0, lg[0]
    for _ in range(200):
        mid = 0.5 * (lo + hi)
        if 0.5 * np.maximum(lg - mid, 0).sum() > H:
            lo = mid
        else:
            hi = mid
    th = np.exp(0.5 * (lo + hi))
    s = np.minimum(1.0, np.sqrt(th / gam))
    return s, float((s * sg).sum()), th


def bytes_at(z2, gam, wstars=WSTARS, it=60):
    """Per row, bytes at which w_B(x) = w*: bisection on the water level theta; 0 if w0 <= w*."""
    sg = np.sqrt(gam)
    lg = np.log(gam)
    m = len(z2)
    w0sq = sg.sum() * z2.sum(1)
    out = np.zeros((m, len(wstars)))
    for j, ws in enumerate(wstars):
        lo = np.full(m, lg[-1] - 60.0)
        hi = np.full(m, lg[0])
        for _ in range(it):
            mid = 0.5 * (lo + hi)
            s = np.minimum(1.0, np.sqrt(np.exp(mid)[:, None] / gam[None]))
            val = (s * sg).sum(1) * (s * z2).sum(1)
            big = val > ws * ws
            hi = np.where(big, mid, hi)
            lo = np.where(big, lo, mid)
        mid = 0.5 * (lo + hi)
        Hn = 0.5 * np.maximum(lg[None] - mid[:, None], 0).sum(1)
        out[:, j] = np.where(w0sq <= ws * ws, 0.0, Hn / (8 * np.log(2)))
    return out


def bytes_exact(z2, gam, wstars=WSTARS):
    """Per row, bytes at which w_B(x) = w*, solved exactly: on each interval of the water level between consecutive
    eigenvalues w^2 is a quadratic in sqrt(theta). 0 if w0 <= w*."""
    m, nn = z2.shape
    sg = np.sqrt(gam)
    k = np.arange(nn + 1)
    Ak = np.concatenate([np.zeros((m, 1)), np.cumsum(z2 / sg, 1)], 1)            # sum_{i<k} z_i^2 / sqrt(g_i)
    Bk = np.concatenate([np.cumsum(z2[:, ::-1], 1)[:, ::-1], np.zeros((m, 1))], 1)  # sum_{i>=k} z_i^2
    Ck = np.concatenate([np.cumsum(sg[::-1])[::-1], [0.0]])                       # sum_{i>=k} sqrt(g_i)
    Lk = np.concatenate([[0.0], np.cumsum(np.log(gam))])
    ug = np.concatenate([sg, [0.0]])                                              # sqrt(theta) at the interval's foot
    V = (ug * k + Ck) * (ug[None] * Ak + Bk)                                       # w^2 at theta = g_k; V[:, n] = 0
    out = np.zeros((m, len(wstars)))
    for j, ws in enumerate(wstars):
        w2 = ws * ws
        kk = np.argmax(V <= w2, axis=1)                                          # first k with V_k <= w*^2
        r = np.arange(m)
        A_, B_, C_ = Ak[r, kk], Bk[r, kk], Ck[kk]
        a2, a1, a0 = kk * A_, kk * B_ + A_ * C_, C_ * B_ - w2
        u = np.where(a2 > 0, (-a1 + np.sqrt(np.maximum(a1 * a1 - 4 * a2 * a0, 0))) / (2 * np.maximum(a2, 1e-300)), 0.0)
        Hn = 0.5 * (Lk[kk] - kk * np.log(np.maximum(u * u, 1e-300)))
        out[:, j] = np.where(V[:, 0] <= w2, 0.0, Hn / (8 * np.log(2)))
    return out


def coding(X, cell, M, bas, covs, A, rows=None):
    """S1 on held-out rows. bas[c] = basis of cell c; covs[b] = that basis's covariance (fit rows).
    Returns per-row arrays: w0, wB (4 levels), bytes (3 widths)."""
    rows = np.flatnonzero(HELD) if rows is None else rows
    w0 = np.zeros(len(rows))
    wB = np.zeros((len(rows), len(LEVELS)))
    by = np.zeros((len(rows), len(WSTARS)))
    rb = bas[cell[rows]]
    for b in range(len(covs)):
        sel = np.flatnonzero(rb == b)
        if len(sel) == 0:
            continue
        gam, T = basis(covs[b], A)
        lv = [level(gam, B) for B in LEVELS]
        F = np.sqrt(gam).sum()
        for i in range(0, len(sel), 5000):
            ss = sel[i:i + 5000]
            r = rows[ss]
            Z = (X[r].astype(np.float64) - M[cell[r]]) @ T
            z2 = Z * Z
            w0[ss] = np.sqrt(F * z2.sum(1))
            for j, (s, W, _) in enumerate(lv):
                wB[ss, j] = np.sqrt(W * (z2 @ s))
            by[ss] = bytes_exact(z2, gam)
    return w0, wB, by


def coding_summary(w0, wB, by):
    d = {"S1a_mean": float(w0.mean()), "S1a_rms": float(np.sqrt((w0 ** 2).mean()))}
    for j, B in enumerate(LEVELS):
        d[f"S1d_{B}"] = float(wB[:, j].mean())
    for j, w in enumerate(WSTARS):
        d[f"S1b_{w}"] = float(by[:, j].mean())
    return d


_top = None
_nn = None


def top10(SCT):
    global _top
    if _top is None:
        p = WORK + "/top10.npy"
        if not os.path.exists(p):
            T = np.zeros((NQ, 10), np.int64)
            inval = np.flatnonzero(~VALID)
            for j in range(NQ):
                s = np.array(SCT[j], np.float64)
                s[inval] = -np.inf
                T[j] = np.argpartition(-s, 10)[:10]
            np.save(p, T)
        _top = np.load(p)
    return _top


def nn10(X):
    """2,000 sample rows (seed 5) and their 10 nearest valid rows by inner product, itself excluded."""
    global _nn
    if _nn is None:
        p = WORK + "/nn10.npz"
        if not os.path.exists(p):
            smp = np.sort(np.random.default_rng(5).choice(np.flatnonzero(VALID), 2000, replace=False))
            Xs = X[smp]
            best = np.full((2000, 0), -np.inf, np.float32)
            bidx = np.zeros((2000, 0), np.int64)
            for s in range(0, N, 20_000):
                sc = Xs @ X[s:s + 20_000].T
                sc[:, ~VALID[s:s + 20_000]] = -np.inf
                ii = np.arange(s, min(s + 20_000, N))
                sc[smp[:, None] == ii[None]] = -np.inf
                best = np.concatenate([best, sc], 1)
                bidx = np.concatenate([bidx, np.broadcast_to(ii, (2000, len(ii)))], 1)
                k = np.argpartition(-best, 10, axis=1)[:, :10]
                best = np.take_along_axis(best, k, 1)
                bidx = np.take_along_axis(bidx, k, 1)
            np.savez(p, smp=smp, nn=bidx)
        d = np.load(p)
        _nn = (d["smp"], d["nn"])
    return _nn


_tax = None


def taxa():
    global _tax
    if _tax is None:
        import pyarrow.parquet as pq
        cols = {"family": [], "order": [], "genus": [], "scientific_name": []}
        for i in range(4):
            t = pq.read_table(TD + f"meta_g{i}.parquet", columns=list(cols))
            for k in cols:
                cols[k] += t.column(k).to_pylist()
        assert len(cols["family"]) == N
        _tax = {}
        for nm, v in cols.items():
            u, inv = np.unique(np.array([x if x else "" for x in v]), return_inverse=True)
            _tax[nm] = (inv, u)
    return _tax


EPS = 1e-5   # slack for float32 storage of scores and rows when counting violations


def structure(X, SCT, cell, M):
    """S2, S3, S2+3 (with violations), S4a, S4b, S5 and the radius ratios, for a partition of every row."""
    K = len(M)
    Mh = unit(M)
    kx = np.empty(N)
    for s in range(0, N, 20_000):
        kx[s:s + 20_000] = (X[s:s + 20_000].astype(np.float64) * Mh[cell[s:s + 20_000]]).sum(1)
    thx = np.arccos(np.clip(kx, -1, 1))
    # extremes of every anchor over every cell, over all rows
    order = np.argsort(cell, kind="stable")
    cntall = np.bincount(cell, minlength=K)
    st = np.concatenate([[0], np.cumsum(cntall)])[:-1]
    Emax = np.empty((K, K))
    Emin = np.empty((K, K))
    Mh32 = Mh.astype(np.float32)
    for c0 in range(0, K, 50):
        P = np.empty((N, min(50, K - c0)), np.float64)
        for s in range(0, N, 50_000):
            P[s:s + 50_000] = X[s:s + 50_000].astype(np.float64) @ Mh[c0:c0 + 50].T
        Ps = P[order]
        Emax[:, c0:c0 + 50] = np.maximum.reduceat(Ps, st, axis=0)
        Emin[:, c0:c0 + 50] = np.minimum.reduceat(Ps, st, axis=0)
    # Emax[c, a] = largest a.x over cell c
    # radius ratios and key percentiles per cell
    R2 = np.empty(N)
    for s in range(0, N, 20_000):
        D = X[s:s + 20_000].astype(np.float64) - M[cell[s:s + 20_000]]
        R2[s:s + 20_000] = (D * D).sum(1)
    rr, kp = [], []
    for c in range(K):
        ii = order[st[c]:st[c] + cntall[c]]
        r = np.sqrt(R2[ii])
        rms = np.sqrt((r * r).mean())
        rr.append(r.max() / rms if rms > 0 else np.nan)
        kp.append(np.percentile(kx[ii], 1) / np.median(kx[ii]))
    rr, kp = np.array(rr), np.array(kp)
    out = {}
    nq3 = len(FRS)
    left2 = np.zeros((NQ, nq3))
    left3 = np.zeros((NQ, nq3))
    left23 = np.zeros((NQ, nq3))
    ext_rho = np.full((NQ, nq3), np.nan)
    ext_beta = np.full((NQ, nq3), np.nan)
    viol2 = 0
    viol3 = 0
    s4a_num = np.zeros(NQ)
    s4a_den = np.zeros(NQ)
    s4b = np.zeros(NQ)
    Vf = VALID.astype(np.float64)
    tops = top10(SCT)
    fitidx = np.flatnonzero(FIT)
    Ofit = sp.csr_matrix((np.ones(len(fitidx)), (cell[fitidx], np.arange(len(fitidx)))), shape=(K, len(fitidx)))
    nfit = np.bincount(cell[fitidx], minlength=K).astype(np.float64)
    nval = np.bincount(cell[VALID], minlength=K).astype(np.float64)
    for j0 in range(0, NQ, 64):
        Sblk = np.array(SCT[j0:j0 + 64], np.float64)            # nb x N
        Sf = Sblk[:, fitidx]
        s1 = (Ofit @ Sf.T).T
        s2 = (Ofit @ (Sf * Sf).T).T
        for jj in range(len(Sblk)):
            j = j0 + jj
            q = Q[j]
            s = Sblk[jj]
            qa = Mh @ q
            p = int(qa.argmax())
            beta = qa[p]
            rho = np.sqrt(max(0.0, 1 - beta * beta))
            e1, e2 = beta * Emax[:, p], beta * Emin[:, p]
            lo_c = np.minimum(e1, e2) - rho
            hi_c = np.maximum(e1, e2) + rho
            phi = np.arccos(np.clip(qa, -1, 1))[cell]
            lo3 = np.cos(np.minimum(phi + thx, np.pi))
            hi3 = np.cos(np.abs(phi - thx))
            for it in range(nq3):
                t = TH[j, it]
                in_c = lo_c >= t
                out_c = hi_c < t
                l2c = ~in_c & ~out_c
                l2 = l2c[cell]
                in3 = lo3 >= t
                out3 = hi3 < t
                l3 = ~(in3 | out3)
                viol2 += int((in_c[cell] & (s < t - EPS)).sum() + (out_c[cell] & (s >= t + EPS)).sum())
                viol3 += int((in3 & (s < t - EPS)).sum() + (out3 & (s >= t + EPS)).sum())
                left2[j, it] = (l2 & VALID).sum() / NV
                left3[j, it] = (l3 & VALID).sum() / NV
                left23[j, it] = (l2 & l3 & VALID).sum() / NV
                if l2c.any():
                    ext_rho[j, it] = 2 * rho
                    ext_beta[j, it] = (np.abs(beta) * (Emax[l2c, p] - Emin[l2c, p])).mean()
            # S4a: variance of q.m_c(x) over the variance of q.x, valid rows
            mu = M @ q
            a_ = mu[cell]
            sv = s[VALID]
            av = a_[VALID]
            s4a_num[j] = av.var()
            s4a_den[j] = sv.var()
            # S4b
            Ef = s1[jj] / np.maximum(nfit, 1)
            Ef2 = s2[jj] / np.maximum(nfit, 1)
            var = np.maximum(Ef2 - 2 * mu * Ef + mu * mu, 0)
            pri = mu + 3 * np.sqrt(var)
            oc = np.argsort(-pri)
            rank = np.empty(K, np.int64)
            rank[oc] = np.arange(K)
            rk = np.sort(rank[cell[tops[j]]])
            r9 = rk[8]
            s4b[j] = nval[oc[:r9 + 1]].sum() / NV
    for kd in range(3):
        m = KIND == kd
        for it in range(nq3):
            out[f"S2_{KINDS[kd]}_{FRS[it]}"] = float(left2[m, it].mean())
            out[f"S3_{KINDS[kd]}_{FRS[it]}"] = float(left3[m, it].mean())
            out[f"S23_{KINDS[kd]}_{FRS[it]}"] = float(left23[m, it].mean())
            out[f"S2rho_{KINDS[kd]}_{FRS[it]}"] = float(np.nanmean(ext_rho[m, it])) if np.isfinite(ext_rho[m, it]).any() else None
            out[f"S2ext_{KINDS[kd]}_{FRS[it]}"] = float(np.nanmean(ext_beta[m, it])) if np.isfinite(ext_beta[m, it]).any() else None
        out[f"S4a_mean_{KINDS[kd]}"] = float((s4a_num[m] / s4a_den[m]).mean())
        out[f"S4a_ratio_{KINDS[kd]}"] = float(s4a_num[m].mean() / s4a_den[m].mean())
        out[f"S4b_{KINDS[kd]}"] = float(s4b[m].mean())
    out["viol_S2"] = viol2
    out["viol_S3"] = viol3
    smp, nn = nn10(X)
    out["S4b_nn_owncell"] = float((cell[nn] == cell[smp][:, None]).mean())
    out["radius_max_over_rms_median"] = float(np.nanmedian(rr))
    out["radius_max_over_rms_share_above2"] = float((rr[np.isfinite(rr)] > 2).mean())
    out["cells_with_zero_radius"] = int((~np.isfinite(rr)).sum())
    out["key_p1_over_median_median"] = float(np.median(kp))
    out["mean_key"] = float(kx[VALID].mean())
    out["msq_resid"] = float(R2[VALID].mean())
    out["msq_resid_held"] = float(R2[HELD].mean())
    # S5
    out["S5_min"] = int(nval.min())
    out["S5_median"] = float(np.median(nval))
    out["S5_max"] = int(nval.max())
    out["S5_cv"] = float(nval.std() / nval.mean())
    out["S5_share_under500"] = float(nval[nval < 500].sum() / NV)
    out["S5_cells_under500"] = int((nval < 500).sum())
    tx = taxa()
    for nm in ("family", "order", "genus", "scientific_name"):
        inv, u = tx[nm]
        iv = inv[VALID]
        cv = cell[VALID]
        pairs = np.unique(iv.astype(np.int64) * (K + 1) + cv)
        per = np.bincount(pairs // (K + 1), minlength=len(u))
        sz = np.bincount(iv, minlength=len(u))
        ok = (sz > 0) & (u != "")
        out[f"S5_cells_per_{nm}"] = float(per[ok].mean())
        out[f"S5_cells_per_{nm}_rowweighted"] = float((per[ok] * sz[ok]).sum() / sz[ok].sum())
    out["K"] = K
    return out


def group_covs(Cc, cnt, grp, G):
    covs = []
    Ng = np.zeros(G)
    for g in range(G):
        m = grp == g
        Ng[g] = cnt[m].sum()
        covs.append(Cc[m].sum(0) / Ng[g])
    return covs, Ng


def score_partition(name, X, SCT, cell, grp, M=None, bas=None, covs=None, extra=None, struct=True):
    """Every score for a partition. cell: every row's cell; grp: each cell's group. bas/covs override the bases
    (default: one per group, pooled fit-row residual covariance)."""
    p = f"{WORK}/scores/{name}.json"
    if os.path.exists(p):
        return json.load(open(p))
    K = int(cell.max()) + 1
    if M is None:
        M, _ = means(X, cell, K)
    G = int(grp.max()) + 1
    Cc, cnt = scatters(X, cell, K, M, FIT)
    if covs is None:
        covs, Ng = group_covs(Cc, cnt, grp, G)
        bas = grp
    else:
        Ng = np.array([cnt[grp == g].sum() for g in range(G)])
    del Cc
    out = {"name": name, "K": K, "G": G, "group_fit_rows_min": int(Ng.min()), "group_fit_rows_max": int(Ng.max()),
           "S1e": n * K / N + n * n * len(covs) / N}
    rowsd = {}
    for key, A in AS.items():
        w0, wB, by = coding(X, cell, M, bas, covs, A)
        for k, v in coding_summary(w0, wB, by).items():
            out[f"{key}_{k}"] = v
        rowsd[key] = (w0, wB, by)
    log(name, "coding done", {k: round(v, 4) for k, v in out.items() if k.startswith("Si_")})
    if struct:
        out.update(structure(X, SCT, cell, M))
        log(name, "structure done; violations", out["viol_S2"], out["viol_S3"])
    if extra:
        out.update(extra)
    json.dump(out, open(p, "w"), indent=1)
    np.savez(f"{WORK}/parts/{name}.npz", cell=cell.astype(np.int32), grp=np.asarray(grp, np.int32))
    return out


def save_part(name, cell, grp):
    np.savez(f"{WORK}/parts/{name}.npz", cell=np.asarray(cell, np.int32), grp=np.asarray(grp, np.int32))


def load_part(name):
    d = np.load(f"{WORK}/parts/{name}.npz")
    return d["cell"], d["grp"]


def build_A(X, seed, K=200, G=12, pool=None, ratio=200):
    """The build's steps: kmeans (12 passes) on a random K*ratio of the pool (default: fit rows); every row to the
    sample centroid of largest inner product; empty centroids dropped; groups by the build's rule on the stored
    centres (means of fit rows)."""
    rng = np.random.default_rng(seed)
    pool = np.flatnonzero(FIT) if pool is None else pool
    smp = np.sort(rng.choice(pool, min(len(pool), K * ratio), replace=False))
    C = kmeans(X[smp].copy(), K, rng)
    cell = argmax_ip(X, C)
    cell, _ = compact(cell)
    K2 = int(cell.max()) + 1
    M, _ = means(X, cell, K2)
    grp = build_groups(M, G, rng) if G > 1 else np.zeros(K2, np.int32)
    return cell, grp, M, C
