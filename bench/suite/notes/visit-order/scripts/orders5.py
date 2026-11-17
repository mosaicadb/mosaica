# Cheap spreads at query time (shared basis, the index's group bases), a rad-aware exceedance model, and a global
# stopping threshold on top of H-type orders, on one fit (merged defaults) and several query sets.
# Spreads sigma_c(q) (score s.d. in cluster c along q):
#   own rR    per-cluster top-R PCA + isotropic rest (H's; R 4, 16)            query: K R d MACs
#   shR       one shared basis U (top R of the pooled within-cluster covariance), per-cluster variances lambda_cj =
#             u_j' Sigma_c u_j and a remainder: sigma^2 = sum_j lambda_cj p_j^2 + rem_c (|q|^2 - |p|^2), p = U'q
#                                                                                query: R d + K R MACs
#   shR+ownT  shared R on Sigma_c minus its own top T directions, plus those T   query: R d + K R + K T d
#   group     the index's dual group bases V_g (fit.py V.npy): sigma^2 = sum_j var_cj (V_g'q)_j^2, var_cj the
#             variance of the item-side coordinates V_g^-1 (x - m_c)            query: G d^2 + K d (every group)
#   exact     q' Sigma_c q from the rows (reference)
# Orders: H form q.m + alpha z(k, n_c) sigma_c + w sd(q) rad_c / sd(rad) (alpha 0.5/0.75/1, w 0..0.4), linear
# q.m + 3.3 sigma_c, and the rad-aware exceedance model: E[rows above t] = n_c exp(g(zeta_c(t)) + beta log(rad_c / med)),
# g tabulated, fitted (Poisson, smoothed) on 1,000 plain + 1,000 shifted corpus pseudo-queries with t their own k-th
# best score; ranked per row at tau from sum_c E = k (also with beta = 0: the tabulated shape alone).
# Stopping: per-row probability of an order's cluster (E[true neighbours | gap] / n_c, the gap (s_max - s_c) / sd(q)
# calibrated by isotonic regression on the same pseudo-queries), thresholded globally; vs fixed nprobe at equal rows.
# usage: python -I orders5.py FITDIR CORPUS.npy OUTDIR DSNAME [--no-faiss] QSET=QUERIES.npy:GT.npy [...]
import sys, os, json, time
import numpy as np
from scipy.stats import norm
from scipy.optimize import minimize

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common_r import summarise, curve, FGRID  # noqa: E402

fitdir, corpus, outdir, dsname = sys.argv[1:5]
do_faiss = "--no-faiss" not in sys.argv
qsets = [a.split("=", 1) for a in sys.argv[5:] if "=" in a]
t0 = time.time()
log = lambda *a: print(f"[{dsname} {time.time() - t0:5.0f}s]", *a, flush=True)
AVG_SHARES = [0.005, 0.008, 0.01, 0.016, 0.02, 0.03, 0.04, 0.063, 0.08, 0.1, 0.125, 0.15, 0.2, 0.25]
SHARES = sorted(set(AVG_SHARES + [0.3]))
KNN = 10
ALPHAS, WS = (0.5, 0.75, 1.0), (0.0, 0.1, 0.2, 0.3, 0.4)

meta = json.load(open(os.path.join(fitdir, "meta.json")))
cstart = np.load(os.path.join(fitdir, "cstart.npy"))
perm = np.load(os.path.join(fitdir, "perm.npy"))
mq8 = np.load(os.path.join(fitdir, "mq8.npy"))
msc = np.load(os.path.join(fitdir, "msc.npy"))
grp = np.load(os.path.join(fitdir, "grp.npy"))
Xs = np.load(os.path.join(fitdir, "Xs.npy"), mmap_mode="r")
N, n = Xs.shape
n0 = meta["n_orig"]
K = len(cstart) - 1
G = int(grp.max()) + 1
sizes = np.diff(cstart)
row_cl = np.repeat(np.arange(K), sizes)
ent_row = np.empty(N, np.int64)
ent_row[perm] = np.arange(N)
Mq = (mq8.astype(np.float64) * msc[:, None]).astype(np.float32).astype(np.float64)
f16 = lambda q: np.asarray(q, np.float32).astype(np.float16).astype(np.float64)
RS = [r for r in (16, 32, 64, 128)]
Rcap = lambda R: min(R, n0 - 8)
HYB = [(32, 2), (64, 4)]

# ---- build-time statistics (corpus only)
cache = os.path.join(fitdir, "stats5.npz")
if os.path.exists(cache):
    z = np.load(cache)
    Mean, rad_q, lam, U, tr, Us, lsh, vgrp = (z[k] for k in ("Mean", "rad_q", "lam", "U", "tr", "Us", "lsh", "vgrp"))
else:
    V = np.load(os.path.join(fitdir, "V.npy")).astype(np.float64)
    Vinv = np.stack([np.linalg.inv(V[g]) for g in range(G)])
    Mean = np.zeros((K, n)); rad_q = np.zeros(K); lam = np.zeros((K, 16)); U = np.zeros((K, n, 16), np.float32)
    tr = np.zeros(K); Cres = np.zeros((n, n)); vgrp = np.zeros((K, n))
    for c in range(K):
        x = np.asarray(Xs[cstart[c]:cstart[c + 1]], np.float64)
        m = x.mean(0)
        Mean[c] = m
        rad_q[c] = np.sqrt(((x - Mq[c]) ** 2).sum(1).mean())
        xc = x - m
        C = xc.T @ xc
        Cres += C
        C /= len(x)
        tr[c] = np.trace(C)
        w, v = np.linalg.eigh(C)
        lam[c] = w[::-1][:16]
        U[c] = v[:, ::-1][:, :16]
        zc = xc @ Vinv[grp[c]].T
        vgrp[c] = (zc * zc).mean(0)
    w, v = np.linalg.eigh(Cres / N)
    Us = v[:, ::-1][:, :128]
    lsh = np.zeros((K, 128))
    for c in range(K):
        xc = np.asarray(Xs[cstart[c]:cstart[c + 1]], np.float64) - Mean[c]
        lsh[c] = ((xc @ Us) ** 2).mean(0)
    np.savez(cache, Mean=Mean, rad_q=rad_q, lam=lam, U=U, tr=tr, Us=Us, lsh=lsh, vgrp=vgrp)
V = np.load(os.path.join(fitdir, "V.npy"))
rad_n = rad_q / rad_q.std()
zq = np.where(sizes > KNN, norm.ppf(1 - KNN / np.maximum(sizes, KNN + 1)), 0.0)
lr = np.log(rad_q / np.median(rad_q))
log(f"K {K}, G {G}, N {N}, n {n} (n0 {n0}); build stats in {time.time() - t0:.0f} s; shared-basis share of residual variance "
    + ", ".join(f"R{Rcap(R)} {lsh[:, :Rcap(R)].sum() / tr.sum():.2f}" for R in RS))
# hybrid: Sigma_c minus its own top T, in the shared basis
hyb = {}
for R, T in HYB:
    R = Rcap(R)
    M = np.einsum("knt,nr->ktr", U[:, :, :T].astype(np.float64), Us[:, :R])
    lp = np.maximum(lsh[:, :R] - (lam[:, :T, None] * M ** 2).sum(1), 0)
    rem = np.maximum(tr - lam[:, :T].sum(1) - lp.sum(1), 0) / max(n0 - R - T, 1)
    hyb[(R, T)] = (lp, rem)


def spreads(Qr, var_exact=None):
    nq = len(Qr)
    qq = (Qr ** 2).sum(1)[:, None]
    out = {}
    Q32 = Qr.astype(np.float32)
    projs = {}
    for r in (4, 16):
        Ur = np.ascontiguousarray(U[:, :, :r].transpose(1, 0, 2).reshape(n, K * r), dtype=np.float32)
        proj = (Q32 @ Ur).astype(np.float64).reshape(nq, K, r)
        projs[r] = proj
        rest = np.maximum(tr - lam[:, :r].sum(1), 0) / max(n0 - r, 1)
        out[f"own r{r}"] = (proj ** 2 * lam[None, :, :r]).sum(2) + rest[None, :] * np.maximum(qq - (proj ** 2).sum(2), 0)
    P = Qr @ Us
    for R0 in RS:
        R = Rcap(R0)
        rem = np.maximum(tr - lsh[:, :R].sum(1), 0) / max(n0 - R, 1)
        p = P[:, :R]
        out[f"sh{R0}"] = (p ** 2) @ lsh[:, :R].T + rem[None, :] * np.maximum(qq - (p ** 2).sum(1, keepdims=True), 0)
    for R0, T in HYB:
        R = Rcap(R0)
        lp, rem = hyb[(R, T)]
        p = P[:, :R]
        own = (projs[16][:, :, :T] ** 2 * lam[None, :, :T]).sum(2)
        out[f"sh{R0}+own{T}"] = own + (p ** 2) @ lp.T + rem[None, :] * np.maximum(qq - (p ** 2).sum(1, keepdims=True), 0)
    vg = np.zeros((nq, K))
    for g in range(G):
        cl = np.where(grp == g)[0]
        if len(cl):
            pg = Qr @ V[g].astype(np.float64)
            vg[:, cl] = (pg ** 2) @ vgrp[cl].T
    out["group"] = vg
    if var_exact is not None:
        out["exact"] = var_exact
    return {k: np.sqrt(np.maximum(v, 1e-12)) for k, v in out.items()}


def cluster_pass(Qr, self_rows=None):
    """Exact per-cluster score variance and each cluster's top k scores (a query's own row left out)."""
    nq = len(Qr)
    Q32 = Qr.astype(np.float32)
    var = np.zeros((nq, K)); tk = np.full((nq, K, KNN), -np.inf, np.float32)
    for c in range(K):
        S = Q32 @ np.asarray(Xs[cstart[c]:cstart[c + 1]], np.float32).T
        var[:, c] = S.astype(np.float64).var(1)
        if self_rows is not None:
            hit = np.where((self_rows >= cstart[c]) & (self_rows < cstart[c + 1]))[0]
            S[hit, self_rows[hit] - cstart[c]] = -np.inf
        kk = min(KNN, S.shape[1])
        tk[:, c, :kk] = -np.sort(np.partition(-S, kk - 1, axis=1)[:, :kk], axis=1)
    t = -np.sort(-tk.reshape(nq, -1), axis=1)[:, KNN - 1]
    Nc = (tk >= t[:, None, None]).sum(2).astype(np.float64)
    return var, t.astype(np.float64), Nc


# ---- pseudo-queries (plain corpus rows and shifted ones), their truth and spreads (cached)
pc = os.path.join(fitdir, "pseudo5.npz")
if os.path.exists(pc):
    z = np.load(pc)
    PQ, pvar, pt, pN = z["PQ"], z["pvar"], z["pt"], z["pN"]
else:
    rng = np.random.default_rng(17)
    rows = np.sort(rng.choice(N, 2400, replace=False))
    X = np.asarray(Xs[rows], np.float64)
    ok = np.linalg.norm(X, axis=1) > 0.5
    rows, X = rows[ok], X[ok]
    X /= np.linalg.norm(X, axis=1, keepdims=True)
    plain, prow, base = X[:1000], rows[:1000], X[1000:2000]
    Gs = np.asarray(Xs[np.sort(rng.choice(N, 50_000, replace=False))], np.float64)[:, :n0]
    w, Vt = np.linalg.eigh(Gs.T @ Gs / len(Gs))
    top = Vt[:, ::-1][:, :min(64, n0 // 2)]
    Rr = rng.standard_normal((1000, n0))
    Rr -= (Rr @ top) @ top.T
    Rr /= np.linalg.norm(Rr, axis=1, keepdims=True)
    ap = rng.uniform(0.3, 0.4, 1000)
    sh = np.zeros((1000, n))
    sh[:, :n0] = ap[:, None] * base[:, :n0] + Rr
    sh /= np.linalg.norm(sh, axis=1, keepdims=True)
    PQ = f16(np.concatenate([plain, sh]))
    v1, t1_, N1 = cluster_pass(PQ[:1000], self_rows=prow)
    v2, t2_, N2 = cluster_pass(PQ[1000:])
    pvar, pt, pN = np.concatenate([v1, v2]), np.concatenate([t1_, t2_]), np.concatenate([N1, N2])
    np.savez(pc, PQ=PQ, pvar=pvar, pt=pt, pN=pN)
log(f"pseudo-queries: k-th best score plain {pt[:1000].mean():.3f}, shifted {pt[1000:].mean():.3f}")
psig = spreads(PQ, pvar)
pmu = PQ @ Mean.T
pqm = PQ @ Mq.T
psd = pqm.std(1, keepdims=True)
# accuracy of each spread on the pseudo-queries (relative error of sigma, rms of log ratio / 1)
acc = {}
for k, s in psig.items():
    e = np.log(s / psig["exact"])
    acc[k] = dict(plain=float(np.sqrt((e[:1000] ** 2).mean())), shifted=float(np.sqrt((e[1000:] ** 2).mean())))
log("sigma relative error (plain / shifted): " + ", ".join(f"{k} {v['plain']:.3f}/{v['shifted']:.3f}" for k, v in acc.items()))

# ---- rad-aware exceedance model per spread
ZB = np.arange(-2.0, 14.01, 0.25)


def fit_model(sname, with_beta):
    zt = ((pt[:, None] - pmu) / psig[sname]).ravel()
    Nn, nn, ll = pN.ravel(), np.broadcast_to(sizes, pN.shape).ravel().astype(float), np.broadcast_to(lr, pN.shape).ravel()
    b = np.clip(np.searchsorted(ZB, zt) - 1, 0, len(ZB) - 1)
    nb = len(ZB)
    pen = 2.0

    def f(th):
        g, beta = th[:nb], (th[nb] if with_beta else 0.0)
        eta = g[b] + beta * ll
        lam_ = nn * np.exp(np.minimum(eta, 5))
        nll = (lam_ - Nn * eta).sum()
        d2 = g[2:] - 2 * g[1:-1] + g[:-2]
        nll += pen * (d2 ** 2).sum() * len(Nn) / 1e4
        r = lam_ - Nn
        gg = np.bincount(b, r, minlength=nb)
        d = np.zeros(nb)
        d[2:] += 2 * pen * d2 * len(Nn) / 1e4; d[1:-1] -= 4 * pen * d2 * len(Nn) / 1e4; d[:-2] += 2 * pen * d2 * len(Nn) / 1e4
        gg += d
        if with_beta:
            return nll, np.r_[gg, (r * ll).sum()]
        return nll, gg

    th0 = np.r_[np.log(np.maximum(norm.sf(ZB), 1e-12)), [0.0] if with_beta else []]
    res = minimize(f, th0, jac=True, method="L-BFGS-B", options=dict(maxiter=2000))
    return res.x[:nb], (float(res.x[nb]) if with_beta else 0.0)


def g_eval(g, zz):
    slope = (g[-1] - g[-5]) / (ZB[-1] - ZB[-5])
    v = np.interp(zz, ZB, g)
    return np.where(zz > ZB[-1], g[-1] + slope * (zz - ZB[-1]), v)


MODEL_SPREADS = ["exact", "own r16", "sh32", "sh64", "sh64+own4", "group"]
models = {}
for sname in MODEL_SPREADS:
    for wb in (True, False):
        g, beta = fit_model(sname, wb)
        models[(sname, wb)] = (g, beta)
    log(f"model {sname}: beta {models[(sname, True)][1]:+.3f}; g at zeta 2/3/4/5: " +
        "/".join(f"{np.exp(g_eval(models[(sname, True)][0], np.array([z_])))[0]:.1e}" for z_ in (2, 3, 4, 5)) +
        f" (Gaussian {'/'.join(f'{norm.sf(z_):.1e}' for z_ in (2, 3, 4, 5))})")


def model_scores(mu, s, g, beta, nq):
    lo, hi = np.full(nq, -1.5), np.full(nq, 1.5)
    for _ in range(40):
        mid = (lo + hi) / 2
        cnt = (sizes[None, :] * np.exp(np.minimum(g_eval(g, (mid[:, None] - mu) / s) + beta * lr[None, :], 5))).sum(1)
        lo, hi = np.where(cnt > KNN, mid, lo), np.where(cnt > KNN, hi, mid)
    tau = (lo + hi) / 2
    return g_eval(g, (tau[:, None] - mu) / s) + beta * lr[None, :]  # log per-row probability


# ---- stopping calibration: E[true neighbours | normalised gap] by isotonic (decreasing) regression on pseudo-queries
DB = np.arange(0, 40.01, 0.05)


def pav_decreasing(y, w):
    yy, ww, cnt = [], [], []
    for a, b_ in zip(y, w):
        yy.append(a); ww.append(b_); cnt.append(1)
        while len(yy) > 1 and yy[-2] < yy[-1]:
            a2, w2, c2 = yy.pop(), ww.pop(), cnt.pop()
            yy[-1] = (yy[-1] * ww[-1] + a2 * w2) / (ww[-1] + w2); ww[-1] += w2; cnt[-1] += c2
    return np.repeat(yy, cnt)


def calibrate_gap(score_p, sdp):
    d = ((score_p.max(1, keepdims=True) - score_p) / sdp).ravel()
    b = np.clip(np.searchsorted(DB, d) - 1, 0, len(DB) - 1)
    s_ = np.bincount(b, pN.ravel(), minlength=len(DB)); c_ = np.bincount(b, minlength=len(DB)).astype(float)
    nz = c_ > 0
    y = np.zeros(len(DB)); y[nz] = pav_decreasing(s_[nz] / c_[nz], c_[nz])
    # empty bins take the next non-empty bin's value (monotone)
    last = 0.0
    for i in range(len(DB) - 1, -1, -1):
        if nz[i]:
            last = y[i]
        else:
            y[i] = last
    return y


def stop_eval(logp_rows, rec_fixed_curve, Nt, nq):
    """Global threshold on a per-row probability (log) vs the order's fixed-nprobe curve, at equal mean rows."""
    lp = logp_rows
    lams = np.unique(np.quantile(lp[np.isfinite(lp)], np.linspace(0, 1, 600)))[::-1]
    ms, mr = [], []
    for l_ in lams:
        sel = lp >= l_
        ms.append((sel * sizes[None, :]).sum(1).mean() / N)
        mr.append((sel * Nt).sum(1).mean() / KNN)
    ms, mr = np.array(ms), np.array(mr)
    o = np.argsort(ms)
    return {f"{x:g}": dict(global_=float(np.interp(x, ms[o], mr[o])), fixed=rec_fixed_curve["fixed"][f"{x:g}"]) for x in SHARES}


faiss_cache = None
if do_faiss:
    import faiss
    Xc = np.load(corpus, mmap_mode="r")
    d = Xc.shape[1]
    rng = np.random.default_rng(1)
    trn = np.ascontiguousarray(Xc[np.sort(rng.choice(len(Xc), min(len(Xc), 200_000), replace=False))], dtype=np.float32)
    qz = faiss.IndexFlatIP(d)
    ix = faiss.IndexIVFFlat(qz, d, K, faiss.METRIC_INNER_PRODUCT)
    ix.train(trn)
    Cf = qz.reconstruct_n(0, K)
    fa = np.concatenate([qz.search(np.ascontiguousarray(Xc[i:i + 100_000], dtype=np.float32), 1)[1][:, 0]
                         for i in range(0, len(Xc), 100_000)])
    faiss_cache = (Cf, fa, np.bincount(fa, minlength=K))
    del Xc


def curve_order(ordr, gtcl):
    nq = len(ordr)
    rank = np.empty_like(ordr)
    rank[np.arange(nq)[:, None], ordr] = np.arange(K)[None, :]
    items = np.cumsum(sizes[ordr], axis=1).astype(np.float64)
    gr = rank[np.arange(nq)[:, None], gtcl]
    found = np.zeros((nq, K))
    np.add.at(found, (np.repeat(np.arange(nq), gtcl.shape[1]), gr.ravel()), 1.0 / gtcl.shape[1])
    return np.cumsum(found, axis=1), items


def summ(rec, items):
    s = summarise(rec, items, N, shares=SHARES)
    s["avg"] = float(np.mean([s["fixed"][f"{x:g}"] for x in AVG_SHARES]))
    return s


info = dict(K=int(K), G=G, N=int(N), n=int(n), n0=n0, R_actual={f"sh{R}": Rcap(R) for R in RS}, sigma_err=acc,
            models={f"{k[0]} {'beta' if k[1] else 'nobeta'}": dict(beta=v[1], g=v[0].tolist()) for k, v in models.items()},
            zbins=ZB.tolist(), shared_var_share={f"sh{R}": float(lsh[:, :Rcap(R)].sum() / tr.sum()) for R in RS})

for qname, spec in qsets:
    qp, gp = spec.split(":")
    t1 = time.time()
    Q0 = np.load(qp).astype(np.float64)
    nq = len(Q0)
    Q = np.zeros((nq, n)); Q[:, :Q0.shape[1]] = Q0
    Qr = f16(Q)
    gt = np.load(gp)[:, :KNN].astype(np.int64)
    gtcl = row_cl[ent_row[gt]]
    Nt = np.zeros((nq, K)); np.add.at(Nt, (np.repeat(np.arange(nq), KNN), gtcl.ravel()), 1)
    var_exact, _, _ = cluster_pass(Qr)
    sig = spreads(Qr, var_exact)
    mu, qm = Qr @ Mean.T, Qr @ Mq.T
    sd_q = qm.std(1, keepdims=True)
    res = dict(name=qname, dataset=dsname, nq=nq, **info, orders={}, stopping={})

    def add(key, score):
        r = summ(*curve_order(np.argsort(-score, axis=1, kind="stable"), gtcl))
        res["orders"][key] = r
        return r

    for f in FGRID:
        add(f"rad f={f}", qm + f * rad_q[None, :])
    if faiss_cache is not None:
        Cf, fa, fs = faiss_cache
        res["orders"]["faiss nlist=K"] = summ(*curve(Q0.astype(np.float32).astype(np.float64) @ Cf.T.astype(np.float64), fa[gt], fs))
    # O2 and the H forms, each with a stopping rule calibrated on the pseudo-queries
    def stop_orders():
        for f in (0.3, 0.4, 0.5):
            yield f"O2 f'={f}", lambda f=f: (qm + f * sd_q * rad_n[None, :], pqm + f * psd * rad_n[None, :])
        for sname in sig:
            for a in ALPHAS:
                for w in WS:
                    yield f"H {sname} a{a} w{w}", lambda sname=sname, a=a, w=w: (
                        qm + a * zq[None, :] * sig[sname] + w * sd_q * rad_n[None, :],
                        pqm + a * zq[None, :] * psig[sname] + w * psd * rad_n[None, :])

    for sname, s in sig.items():
        add(f"lin {sname} 3.3", qm + 3.3 * s)
    for key, fn in stop_orders():
        sc, scp = fn()
        r = add(key, sc)
        tab = calibrate_gap(scp, psd)
        d = (sc.max(1, keepdims=True) - sc) / sd_q
        b = np.clip(np.searchsorted(DB, d) - 1, 0, len(DB) - 1)
        lp = np.log(np.maximum(tab[b], 1e-30) / sizes[None, :])
        res["stopping"][key] = stop_eval(lp, r, Nt, nq)
    # rad-aware exceedance model orders (and their own stopping on the model's per-row probability)
    for sname in MODEL_SPREADS:
        for wb in (True, False):
            g, beta = models[(sname, wb)]
            lp = model_scores(mu, sig[sname], g, beta, nq)
            key = f"model {sname} {'beta' if wb else 'nobeta'}"
            r = add(key, lp)
            res["stopping"][key] = stop_eval(lp, r, Nt, nq)
    res["oracle"] = max(FGRID, key=lambda f: res["orders"][f"rad f={f}"]["avg"])
    json.dump(res, open(os.path.join(outdir, f"{qname}.json"), "w"))
    oa = res["orders"][f"rad f={res['oracle']}"]["avg"]
    top = sorted(res["orders"].items(), key=lambda kv: -kv[1]["avg"])[:6]
    log(f"{qname}: {nq} queries in {time.time() - t1:.0f} s; oracle {oa:.3f}; H own r16 a0.75 w0.3 "
        f"{res['orders']['H own r16 a0.75 w0.3']['avg']:.3f}; " + "; ".join(f"{k} {v['avg']:.3f}" for k, v in top))
