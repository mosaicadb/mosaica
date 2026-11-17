# Calibrated-spread probe orders (theory/visit_order.md section 9), their ceilings, the collapse check and a global
# stopping rule, on one fit (merged defaults) and several query sets.
#   sigma~_c^2(q) = a sigmahat_{c,r}^2(q) + b sd(q)^2 rad_c^2 + c0 sd(q)^2, with sigmahat_{c,r} the top-r PCA +
#   isotropic-rest estimate (r 4, 16) and sd(q) the spread of q.m over clusters; a, b, c0 >= 0 fitted once per index
#   with no queries, in the log domain (relative error; variance-domain NNLS also reported), on the exact q'Sigma_c q of
#   1,000 corpus-row pseudo-queries ("plain") or those plus 1,000 shifted ones ("mixed": u = a'x + v, a' ~ U(0.3, 0.4),
#   v a random unit direction orthogonal to the corpus's top principal subspace, renormalised).
#   Orders with sigma~: zeta-order (tau_model; per row: P(score > tau); per probe: n_c P), the linear fallback
#   mu + 3.3 sigma~, and tau_running (tau_model until k rows are scanned, then the running k-th best, re-ranked every
#   probe). References: H r16, H r4, O2 f'=0.4, the rad-f grid (oracle), faiss nlist = K, the raw-r16 and exact-Sigma
#   zeta-orders. Ceilings: clairvoyant N_c / n_c, and the true-tau zeta-order with exact Sigma.
#   E2 collapse: rows above tau_true / rows, binned by zeta_c(tau_true) with exact sigma, by rad / n_c / kurtosis tercile.
#   E7: a global threshold lambda on P(score > tau) per cluster against a fixed budget, at equal mean rows scanned.
# usage: python -I orders4.py FITDIR CORPUS.npy OUTDIR DSNAME [--no-faiss] QSET=QUERIES.npy:GT.npy [...]
import sys, os, json, time
import numpy as np
from scipy.special import ndtr, log_ndtr
from scipy.stats import norm
from scipy.optimize import nnls, least_squares

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
PSI = 3.3

meta = json.load(open(os.path.join(fitdir, "meta.json")))
cstart = np.load(os.path.join(fitdir, "cstart.npy"))
perm = np.load(os.path.join(fitdir, "perm.npy"))
mq8 = np.load(os.path.join(fitdir, "mq8.npy"))
msc = np.load(os.path.join(fitdir, "msc.npy"))
Xs = np.load(os.path.join(fitdir, "Xs.npy"), mmap_mode="r")
N, n = Xs.shape
n0 = meta["n_orig"]
K = len(cstart) - 1
sizes = np.diff(cstart)
row_cl = np.repeat(np.arange(K), sizes)
ent_row = np.empty(N, np.int64)
ent_row[perm] = np.arange(N)
Mq = (mq8.astype(np.float64) * msc[:, None]).astype(np.float32).astype(np.float64)
f16 = lambda q: np.asarray(q, np.float32).astype(np.float16).astype(np.float64)

# ---- per-cluster statistics (corpus only): mean, rad, top-16 PCA, trace, excess kurtosis on 32 corpus directions
cache = os.path.join(fitdir, "stats4.npz")
if os.path.exists(cache):
    z = np.load(cache)
    Mean, rad_q, lam, U, tr, exk = (z[k] for k in ("Mean", "rad_q", "lam", "U", "tr", "exk"))
else:
    rng = np.random.default_rng(5)
    D = np.asarray(Xs[np.sort(rng.choice(N, 200, replace=False))], np.float64)
    D = D[np.linalg.norm(D, axis=1) > 0.5][:32]
    D32 = (D / np.linalg.norm(D, axis=1, keepdims=True)).astype(np.float32)
    Mean = np.zeros((K, n)); rad_q = np.zeros(K); lam = np.zeros((K, 16)); U = np.zeros((K, n, 16), np.float32)
    tr = np.zeros(K); exk = np.zeros(K)
    for c in range(K):
        x = np.asarray(Xs[cstart[c]:cstart[c + 1]], np.float32)
        x64 = x.astype(np.float64)
        m = x64.mean(0)
        Mean[c] = m
        rad_q[c] = np.sqrt(((x64 - Mq[c]) ** 2).sum(1).mean())
        xc = x64 - m
        C = xc.T @ xc / len(x)
        tr[c] = np.trace(C)
        w, v = np.linalg.eigh(C)
        lam[c] = w[::-1][:16]
        U[c] = v[:, ::-1][:, :16]
        P = (x @ D32.T).astype(np.float64)
        P -= P.mean(0)
        m2, m4 = (P ** 2).mean(0), (P ** 4).mean(0)
        exk[c] = np.median(np.where(m2 > 0, m4 / np.maximum(m2, 1e-300) ** 2 - 3, 0)) if len(x) >= 20 else 0.0
    np.savez(cache, Mean=Mean, rad_q=rad_q, lam=lam, U=U, tr=tr, exk=exk)
log(f"K {K}, N {N}, n {n} (n0 {n0}), sizes {sizes.min()}..{sizes.max()}; corpus stats in {time.time() - t0:.0f} s")
rad_n = rad_q / rad_q.std()
zq = np.where(sizes > KNN, norm.ppf(1 - KNN / np.maximum(sizes, KNN + 1)), 0.0)


def var_pca(Qr, r):
    Ur = np.ascontiguousarray(U[:, :, :r].transpose(1, 0, 2).reshape(n, K * r), dtype=np.float32)
    proj = (Qr.astype(np.float32) @ Ur).astype(np.float64).reshape(len(Qr), K, r)
    head = (proj ** 2 * lam[None, :, :r]).sum(2)
    rest = np.maximum(tr - lam[:, :r].sum(1), 0) / max(n0 - r, 1)
    return head + rest[None, :] * np.maximum((Qr ** 2).sum(1)[:, None] - (proj ** 2).sum(2), 0)


def cluster_pass(Qr, tau=None, topk=False):
    """Exact per-cluster score variance (and, if asked, each cluster's top k scores and the rows above tau)."""
    nq = len(Qr)
    Q32 = Qr.astype(np.float32)
    var = np.zeros((nq, K)); tk = np.full((nq, K, KNN), -np.inf) if topk else None
    above = np.zeros((nq, K)) if tau is not None else None
    for c in range(K):
        S = Q32 @ np.asarray(Xs[cstart[c]:cstart[c + 1]], np.float32).T
        var[:, c] = S.astype(np.float64).var(1)
        if topk:
            kk = min(KNN, S.shape[1])
            tk[:, c, :kk] = -np.sort(np.partition(-S, kk - 1, axis=1)[:, :kk], axis=1)
        if tau is not None:
            above[:, c] = (S > tau[:, None]).sum(1)
    return var, tk, above


# ---- calibration of sigma~ on pseudo-queries (no queries)
ccache = os.path.join(fitdir, "calib4.json")
if os.path.exists(ccache):
    calib = json.load(open(ccache))
else:
    rng = np.random.default_rng(17)
    X = np.asarray(Xs[np.sort(rng.choice(N, 2400, replace=False))], np.float64)
    X = X[np.linalg.norm(X, axis=1) > 0.5]
    X /= np.linalg.norm(X, axis=1, keepdims=True)
    plain, base = X[:1000], X[1000:2000]
    # the corpus's top principal subspace (uncentred second moment of a 50,000-row sample, first n0 dimensions)
    G = np.asarray(Xs[np.sort(rng.choice(N, 50_000, replace=False))], np.float64)[:, :n0]
    w, V = np.linalg.eigh(G.T @ G / len(G))
    top = V[:, ::-1][:, :min(64, n0 // 2)]
    R = rng.standard_normal((1000, n0))
    R -= (R @ top) @ top.T
    R /= np.linalg.norm(R, axis=1, keepdims=True)
    ap = rng.uniform(0.3, 0.4, 1000)
    sh = np.zeros((1000, n))
    sh[:, :n0] = ap[:, None] * base[:, :n0] + R
    sh /= np.linalg.norm(sh, axis=1, keepdims=True)
    calib = {}
    feats = {}
    for name, Pq in (("plain", plain), ("shifted", sh)):
        Pr = f16(Pq)
        ve, _, _ = cluster_pass(Pr)
        sdq2 = (Pr @ Mq.T).var(1, keepdims=True)
        feats[name] = dict(y=ve, v4=var_pca(Pr, 4), v16=var_pca(Pr, 16), f2=sdq2 * rad_q[None, :] ** 2,
                           f3=np.broadcast_to(sdq2, ve.shape))
        Ssub = Pr.astype(np.float32) @ np.asarray(Xs[:50_000], np.float32).T
        feats[name]["top1"] = float(np.where(Ssub < 0.9999, Ssub, -1).max(1).mean())
    log(f"pseudo-queries: plain 1st-neighbour sim (first 50k rows) {feats['plain']['top1']:.3f}, shifted {feats['shifted']['top1']:.3f}")
    for setname, parts in (("plain", ["plain"]), ("mixed", ["plain", "shifted"])):
        for r in (4, 16):
            y = np.concatenate([feats[p]["y"].ravel() for p in parts])
            F = np.stack([np.concatenate([feats[p][k].ravel() for p in parts]) for k in (f"v{r}", "f2", "f3")], 1)
            ok = y > 1e-10
            y, F = y[ok], F[ok]
            # variance-domain NNLS (on a 200k subsample), then the log-domain fit started from it
            sub = np.random.default_rng(1).choice(len(y), min(len(y), 200_000), replace=False)
            sc = F[sub].std(0)
            cv, _ = nnls(F[sub] / sc, y[sub])
            cv = cv / sc
            x0 = np.maximum(cv, 1e-3 * y[sub].mean() / np.maximum(F[sub].mean(0), 1e-30))
            res = least_squares(lambda th: np.log(F[sub] @ th + 1e-30) - np.log(y[sub]), x0, bounds=(0, np.inf), x_scale=1 / sc)
            cl = res.x
            pred = F @ cl
            err = np.log(pred + 1e-30) - np.log(y)
            raw = np.log(F[:, 0]) - np.log(y)
            calib[f"{setname} r{r}"] = dict(log=cl.tolist(), var=cv.tolist(), rms_log_err=float(np.sqrt((err ** 2).mean())),
                                            rms_log_err_raw=float(np.sqrt((raw ** 2).mean())), mean_log_err_raw=float(raw.mean()))
            # the error of the calibrated sigma on each pseudo-query kind (relative error in sigma = half the log error)
            for p in ("plain", "shifted"):
                yy, FF = feats[p]["y"].ravel(), np.stack([feats[p][k].ravel() for k in (f"v{r}", "f2", "f3")], 1)
                o = yy > 1e-10
                e = np.log(FF[o] @ cl + 1e-30) - np.log(yy[o])
                e0 = np.log(FF[o, 0]) - np.log(yy[o])
                calib[f"{setname} r{r}"][f"sigma_relerr_{p}"] = float(np.sqrt((e ** 2).mean()) / 2)
                calib[f"{setname} r{r}"][f"sigma_relerr_raw_{p}"] = float(np.sqrt((e0 ** 2).mean()) / 2)
    json.dump(calib, open(ccache, "w"), indent=1)
for k, v in calib.items():
    log(f"calib {k}: log fit a {v['log'][0]:.3f} b {v['log'][1]:.4f} c0 {v['log'][2]:.5f} | var NNLS a {v['var'][0]:.3f} "
        f"b {v['var'][1]:.4f} c0 {v['var'][2]:.5f} | sigma rel err plain {v['sigma_relerr_plain']:.3f} (raw {v['sigma_relerr_raw_plain']:.3f}), "
        f"shifted {v['sigma_relerr_shifted']:.3f} (raw {v['sigma_relerr_raw_shifted']:.3f})")
dsinfo = dict(K=int(K), N=int(N), n0=n0, calib=calib)


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

for qname, spec in qsets:
    qp, gp = spec.split(":")
    t1 = time.time()
    Q0 = np.load(qp).astype(np.float64)
    nq = len(Q0)
    Q = np.zeros((nq, n)); Q[:, :Q0.shape[1]] = Q0
    Qr = f16(Q)
    gt = np.load(gp)[:, :KNN].astype(np.int64)
    gtcl = row_cl[ent_row[gt]]
    gts = np.array([Qr[j].astype(np.float32) @ np.asarray(Xs[np.sort(ent_row[gt[j]])], np.float32).T for j in range(nq)])
    tau_true = gts.min(1).astype(np.float64)
    var_exact, topk, above = cluster_pass(Qr, tau=tau_true.astype(np.float32), topk=True)
    mu = Qr @ Mean.T
    qm = Qr @ Mq.T
    sd_q = qm.std(1, keepdims=True)
    v4, v16 = var_pca(Qr, 4), var_pca(Qr, 16)
    sig = {"exact": np.sqrt(np.maximum(var_exact, 1e-12)), "raw r16": np.sqrt(np.maximum(v16, 1e-12)),
           "raw r4": np.sqrt(np.maximum(v4, 1e-12))}
    for cs in ("plain", "mixed"):
        for r, vr in ((4, v4), (16, v16)):
            a, b, c0 = calib[f"{cs} r{r}"]["log"]
            sig[f"cal-{cs} r{r}"] = np.sqrt(np.maximum(a * vr + b * sd_q ** 2 * rad_q[None, :] ** 2 + c0 * sd_q ** 2, 1e-12))
    res = dict(name=qname, dataset=dsname, nq=nq, **dsinfo, orders={})

    def add(key, score=None, ordr=None):
        if ordr is None:
            ordr = np.argsort(-score, axis=1, kind="stable")
        res["orders"][key] = summ(*curve_order(ordr, gtcl))
        return ordr

    for f in FGRID:
        add(f"rad f={f}", qm + f * rad_q[None, :])
    add("O2 f'=0.4", qm + 0.4 * sd_q * rad_n[None, :])
    hkey = qm + 0.75 * sig["raw r16"] * zq[None, :] + 0.3 * sd_q * rad_n[None, :]
    add("H r16", hkey)
    add("H r4", qm + 0.5 * sig["raw r4"] * zq[None, :] + 0.3 * sd_q * rad_n[None, :])
    if faiss_cache is not None:
        Cf, fa, fs = faiss_cache
        res["orders"]["faiss nlist=K"] = summ(*curve(Q0.astype(np.float32).astype(np.float64) @ Cf.T.astype(np.float64), fa[gt], fs))
    # clairvoyant: true neighbours per row, ties (no neighbour) in H r16 order
    Nc = np.zeros((nq, K)); np.add.at(Nc, (np.repeat(np.arange(nq), KNN), gtcl.ravel()), 1)
    hr = np.argsort(np.argsort(-hkey, axis=1, kind="stable"), axis=1)
    add("clairvoyant N_c/n_c", Nc / sizes[None, :] * 1e6 - hr * 1e-6)

    def tau_model(s):
        lo, hi = np.full(nq, -1.5), np.full(nq, 1.5)
        for _ in range(50):
            mid = (lo + hi) / 2
            cnt = (sizes[None, :] * ndtr(-(mid[:, None] - mu) / s)).sum(1)
            lo, hi = np.where(cnt > KNN, mid, lo), np.where(cnt > KNN, hi, mid)
        return (lo + hi) / 2

    def running(s, tm):
        logp = lambda tau: log_ndtr(-(tau[:, None] - mu) / s)
        probed = np.zeros((nq, K), bool); ordr = np.zeros((nq, K), np.int64)
        run = np.full((nq, KNN), -np.inf); scanned = np.zeros(nq); ar = np.arange(nq)
        for step in range(K):
            tau = np.where(np.isfinite(run[:, KNN - 1]), run[:, KNN - 1], tm)
            key = np.where(probed, -np.inf, logp(tau))
            nxt = key.argmax(1)
            ordr[:, step] = nxt; probed[ar, nxt] = True; scanned += sizes[nxt]
            run = -np.sort(-np.concatenate([run, topk[ar, nxt]], axis=1), axis=1)[:, :KNN]
            if scanned.min() > 0.31 * N:
                rest = np.argsort(-np.where(probed, -np.inf, key), axis=1, kind="stable")
                for i in range(nq):
                    ordr[i, step + 1:] = rest[i, :K - step - 1]
                break
        return ordr

    stop = {}
    for sname in ["exact", "raw r16", "raw r4", "cal-plain r16", "cal-mixed r16", "cal-plain r4", "cal-mixed r4"]:
        s = sig[sname]
        tm = tau_model(s)
        lp = log_ndtr(-(tm[:, None] - mu) / s)
        o = add(f"zeta {sname} model row", lp)
        add(f"zeta {sname} model probe", lp + np.log(sizes)[None, :])
        if sname.startswith("cal") or sname == "raw r16":
            add(f"lin {sname} mu+3.3sigma", qm + PSI * s)
        if sname == "exact":
            add("zeta exact true row", log_ndtr(-(tau_true[:, None] - mu) / s))
        if sname in ("cal-mixed r16", "cal-plain r16"):
            add(f"zeta {sname} running row", ordr=running(s, tm))
        if sname.startswith("cal") or sname == "exact":
            # E7: global threshold on P(score > tau_model) (a prefix of the per-row zeta order) vs fixed budgets
            rec, items = curve_order(o, gtcl)
            ps = np.take_along_axis(lp, o, axis=1)  # descending along the order
            lams = np.unique(np.quantile(ps[np.isfinite(ps)], np.linspace(0, 1, 800)))[::-1]
            ms, mr = [], []
            for lam_ in lams:
                cnt = (ps >= lam_).sum(1)
                r_ = np.where(cnt > 0, rec[np.arange(nq), np.maximum(cnt - 1, 0)], 0.0)
                it = np.where(cnt > 0, items[np.arange(nq), np.maximum(cnt - 1, 0)], 0.0)
                ms.append(it.mean() / N); mr.append(r_.mean())
            ms, mr = np.array(ms), np.array(mr)
            oo = np.argsort(ms)
            fixed = summarise(rec, items, N, shares=SHARES)
            stop[sname] = {f"{x:g}": dict(global_lambda=float(np.interp(x, ms[oo], mr[oo])), fixed_nprobe=fixed["fixed"][f"{x:g}"],
                                          per_query_rows=fixed["budget"][f"{x:g}"]) for x in SHARES}
    res["stopping"] = stop
    res["oracle"] = max(FGRID, key=lambda f: res["orders"][f"rad f={f}"]["avg"])

    # E2 collapse: exceedance rate vs zeta(tau_true) with exact sigma, by rad / n_c / kurtosis tercile
    zt = (tau_true[:, None] - mu) / sig["exact"]
    rate = above / sizes[None, :]
    bins = np.arange(0, 8.01, 0.5)
    def binned(mask):
        out = []
        for lo in bins[:-1]:
            m = mask & (zt >= lo) & (zt < lo + 0.5)
            out.append([float(lo + 0.25), int(m.sum()), float(above[m].sum() / max(np.broadcast_to(sizes, m.shape)[m].sum(), 1))])
        return out
    coll = {"all": binned(np.ones_like(zt, bool))}
    for cov, vals in (("rad", rad_q), ("n_c", sizes.astype(float)), ("kurtosis", exk)):
        t1_, t2_ = np.percentile(vals, [33.3, 66.7])
        for nm, m in (("low", vals <= t1_), ("mid", (vals > t1_) & (vals <= t2_)), ("high", vals > t2_)):
            coll[f"{cov} {nm}"] = binned(np.broadcast_to(m[None, :], zt.shape))
    res["collapse"] = coll
    res["gaussian_tail"] = [[float(b + 0.25), float(ndtr(-(b + 0.25)))] for b in bins[:-1]]
    json.dump(res, open(os.path.join(outdir, f"{qname}.json"), "w"), indent=1)
    oa = res["orders"][f"rad f={res['oracle']}"]["avg"]
    top = sorted(res["orders"].items(), key=lambda kv: -kv[1]["avg"])[:7]
    log(f"{qname}: {nq} queries in {time.time() - t1:.0f} s; oracle {oa:.3f}; H r16 {res['orders']['H r16']['avg']:.3f}; "
        + "; ".join(f"{k} {v['avg']:.3f}" for k, v in top))
