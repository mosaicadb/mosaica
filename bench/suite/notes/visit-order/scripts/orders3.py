# Principled probe orders against the tuned ones, on one fit (merged defaults) and several query sets.
# Scores in cluster c are modelled as N(mu_c, sigma_c^2): mu_c = q.mean_c, sigma_c^2 = q' Sigma_c q, Sigma_c from the
# top r principal directions (r 4, 16) plus an isotropic rest, or exact (E[(q.x)^2] - mu^2 from the rows). Orders:
#   tuned: O2 q.m + f' sd(q) rad / sd(rad) (grid, 0.4, and f' estimated from the corpus), H r4 (x0.5, f' 0.3),
#          H r16 (x0.75, f' 0.3), O3 (q.m + sigma z(k, n_c)), the rad-f grid (oracle = best per dataset), faiss nlist = K
#   threshold: p_c = P(score > tau) (per row) or n_c p_c (per probe), tau = model (sum n_c p_c = k, bisection),
#          true (the query's k-th best score), running (adaptive: tau = k-th best score among rows scanned so far;
#          re-ranked after every probe or every 8 probes; the first probe by the O3 key)
#   tail: Student-t per cluster, nu = 4 + 6 / excess kurtosis (clipped to [3, 100]) of the rows' scores on 32 corpus-row
#          directions, scale sigma sqrt((nu - 2) / nu): O3-t (its quantile for z) and threshold-t
# usage: python -I orders3.py FITDIR CORPUS.npy OUTDIR DSNAME [--no-faiss] QSET=QUERIES.npy:GT.npy [...]
import sys, os, json, time
import numpy as np
from scipy.special import ndtr, log_ndtr
from scipy.stats import norm, t as student

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common_r import summarise, FGRID  # noqa: E402

fitdir, corpus, outdir, dsname = sys.argv[1:5]
do_faiss = "--no-faiss" not in sys.argv
qsets = [a.split("=", 1) for a in sys.argv[5:] if "=" in a]
t0 = time.time()
log = lambda *a: print(f"[{dsname} {time.time() - t0:5.0f}s]", *a, flush=True)
AVG_SHARES = [0.005, 0.008, 0.01, 0.016, 0.02, 0.03, 0.04, 0.063, 0.08, 0.1, 0.125, 0.15, 0.2, 0.25]
SHARES = sorted(set(AVG_SHARES + [0.3]))
F2 = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.75, 1.0]
KNN = 10
NDIR, NKURT = 1000, 32

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

# ---- per-cluster statistics from the corpus only (no queries)
cache = os.path.join(fitdir, "stats3.npz")
if os.path.exists(cache):
    z = np.load(cache)
    Mean, rad_q, lam, U, tr, kurt, W, Bu = (z[k] for k in ("Mean", "rad_q", "lam", "U", "tr", "kurt", "W", "Bu"))
else:
    rng = np.random.default_rng(5)
    cand = rng.choice(N, 3 * NDIR, replace=False)
    D = np.asarray(Xs[np.sort(cand)], np.float64)
    D = D[np.linalg.norm(D, axis=1) > 0.5][:NDIR]
    D /= np.linalg.norm(D, axis=1, keepdims=True)
    D32 = D.astype(np.float32)
    Mean = np.zeros((K, n)); rad_q = np.zeros(K); lam = np.zeros((K, 16)); U = np.zeros((K, n, 16), np.float32)
    tr = np.zeros(K); kurt = np.full((K, NKURT), np.nan); W = np.zeros(len(D))
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
        var = (P * P).mean(0)
        W += len(x) * var
        if len(x) >= 20:
            m2, m4 = var[:NKURT], (P[:, :NKURT] ** 4).mean(0)
            kurt[c] = np.where(m2 > 0, m4 / np.maximum(m2, 1e-300) ** 2 - 3, np.nan)
    W /= N
    Bu = (D @ Mq.T).std(1)
    np.savez(cache, Mean=Mean, rad_q=rad_q, lam=lam, U=U, tr=tr, kurt=kurt, W=W, Bu=Bu)
log(f"K {K}, N {N}, n {n} (n0 {n0}), sizes {sizes.min()}..{sizes.max()}; corpus stats in {time.time() - t0:.0f} s")

# Student-t per cluster: nu from the median excess kurtosis over the directions
exk = np.nanmedian(kurt, axis=1)
exk = np.where(np.isnan(exk), 0.0, exk)
nu = np.clip(np.where(exk > 0, 4 + 6 / np.maximum(exk, 1e-9), 100.0), 3, 100)
tscale = np.sqrt((nu - 2) / nu)
zq = np.where(sizes > KNN, norm.ppf(1 - KNN / np.maximum(sizes, KNN + 1)), 0.0)
tq = np.where(sizes > KNN, student.ppf(1 - KNN / np.maximum(sizes, KNN + 1), nu) * tscale, 0.0)

# f' from the corpus: O2's bonus f' sd(q) rad_c / sd(rad) set equal to the Gaussian max bonus z sigma_c(u), with the
# within-cluster spread along u proportional to rad_c: sigma_c(u) = sqrt(W_u) rad_c / rad_rms (W_u pooled over rows,
# rad_rms^2 = row-weighted mean rad^2), and sd(q) taken as the spread of u.m over clusters (B_u). So
# f'_u = z sqrt(W_u) sd(rad) / (B_u rad_rms), z = z(k, median n_c); f' = median over the corpus-row directions u.
rad_rms = np.sqrt((sizes * rad_q ** 2).sum() / N)
zmed = norm.ppf(1 - KNN / np.median(sizes))
fu = zmed * np.sqrt(W) * rad_q.std() / (Bu * rad_rms)
f_est = float(np.median(fu))
rad_n = rad_q / rad_q.std()
log(f"nu: median {np.median(nu):.1f}, 10/90% {np.percentile(nu, 10):.1f}/{np.percentile(nu, 90):.1f}, at 3: {(nu <= 3).mean():.2f}, "
    f"at 100: {(nu >= 100).mean():.2f}; excess kurtosis median {np.median(exk):.2f}; f' estimated {f_est:.3f} "
    f"(10/90% over directions {np.percentile(fu, 10):.3f}/{np.percentile(fu, 90):.3f}; z {zmed:.2f})")
dsinfo = dict(K=int(K), N=int(N), n0=n0, f_est=f_est, f_est_p10=float(np.percentile(fu, 10)), f_est_p90=float(np.percentile(fu, 90)),
              zmed=float(zmed), nu=dict(median=float(np.median(nu)), p10=float(np.percentile(nu, 10)), p90=float(np.percentile(nu, 90)),
              at3=float((nu <= 3).mean()), at100=float((nu >= 100).mean())), exk_median=float(np.median(exk)),
              exk_p90=float(np.percentile(exk, 90)))


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
    X = np.load(corpus, mmap_mode="r")
    d = X.shape[1]
    rng = np.random.default_rng(1)
    trn = np.ascontiguousarray(X[np.sort(rng.choice(len(X), min(len(X), 200_000), replace=False))], dtype=np.float32)
    qz = faiss.IndexFlatIP(d)
    ix = faiss.IndexIVFFlat(qz, d, K, faiss.METRIC_INNER_PRODUCT)
    ix.train(trn)
    Cf = qz.reconstruct_n(0, K)
    fa = np.concatenate([qz.search(np.ascontiguousarray(X[i:i + 100_000], dtype=np.float32), 1)[1][:, 0]
                         for i in range(0, len(X), 100_000)])
    faiss_cache = (Cf, fa, np.bincount(fa, minlength=K))
    del X

for qname, spec in qsets:
    qp, gp = spec.split(":")
    t1 = time.time()
    Q0 = np.load(qp).astype(np.float64)
    nq = len(Q0)
    Q = np.zeros((nq, n)); Q[:, :Q0.shape[1]] = Q0
    Qr = f16(Q)
    Qr32 = Qr.astype(np.float32)
    gt = np.load(gp)[:, :KNN].astype(np.int64)
    gtcl = row_cl[ent_row[gt]]
    # per cluster: exact score variance and the top k scores (for the running threshold and tau_true)
    var_exact = np.zeros((nq, K)); topk = np.full((nq, K, KNN), -np.inf)
    for c in range(K):
        S = Qr32 @ np.asarray(Xs[cstart[c]:cstart[c + 1]], np.float32).T
        var_exact[:, c] = S.astype(np.float64).var(1)
        kk = min(KNN, S.shape[1])
        topk[:, c, :kk] = -np.sort(np.partition(-S, kk - 1, axis=1)[:, :kk], axis=1)  # descending
    tau_true = -np.sort(-topk.reshape(nq, -1), axis=1)[:, KNN - 1]
    gts = np.array([Qr32[j] @ np.asarray(Xs[np.sort(ent_row[gt[j]])], np.float32).T for j in range(nq)])
    log(f"{qname}: tau_true vs the truth's 10th score: mean |diff| {np.abs(tau_true - gts.min(1)).mean():.2e}")
    mu = Qr @ Mean.T
    qm = Qr @ Mq.T

    def var_pca(r):
        Ur = np.ascontiguousarray(U[:, :, :r].transpose(1, 0, 2).reshape(n, K * r), dtype=np.float32)
        proj = (Qr32 @ Ur).astype(np.float64).reshape(nq, K, r)
        head = (proj ** 2 * lam[None, :, :r]).sum(2)
        rest = np.maximum(tr - lam[:, :r].sum(1), 0) / max(n0 - r, 1)
        return head + rest[None, :] * np.maximum((Qr ** 2).sum(1)[:, None] - (proj ** 2).sum(2), 0)

    sig = {k: np.sqrt(np.maximum(v, 1e-12)) for k, v in (("r4", var_pca(4)), ("r16", var_pca(16)), ("exact", var_exact))}
    res = dict(name=qname, dataset=dsname, nq=nq, **dsinfo, orders={})

    def add(key, score=None, ordr=None):
        if ordr is None:
            ordr = np.argsort(-score, axis=1, kind="stable")
        res["orders"][key] = summ(*curve_order(ordr, gtcl))

    # tuned and reference orders
    for f in FGRID:
        add(f"rad f={f}", qm + f * rad_q[None, :])
    sd_q = qm.std(1, keepdims=True)
    for f in F2:
        add(f"O2 f'={f}", qm + f * sd_q * rad_n[None, :])
    add("O2 f'=est", qm + f_est * sd_q * rad_n[None, :])
    add("H r4", qm + 0.5 * sig["r4"] * zq[None, :] + 0.3 * sd_q * rad_n[None, :])
    add("H r16", qm + 0.75 * sig["r16"] * zq[None, :] + 0.3 * sd_q * rad_n[None, :])
    for v in ("r4", "r16", "exact"):
        add(f"O3 {v}", qm + sig[v] * zq[None, :])
        add(f"O3-t {v}", qm + sig[v] * tq[None, :])
    if faiss_cache is not None:
        Cf, fa, fs = faiss_cache
        sc = Q0.astype(np.float32).astype(np.float64) @ Cf.T.astype(np.float64)
        from common_r import curve
        res["orders"]["faiss nlist=K"] = summ(*curve(sc, fa[gt], fs))

    # threshold orders: log P(score > tau) per row, + log n_c per probe
    def logp(tau, v, tail):
        zz = (tau[:, None] - mu) / sig[v]
        if tail == "t":
            return student.logsf(zz / tscale[None, :], nu[None, :])
        return log_ndtr(-zz)

    def tau_model(v, tail):
        lo, hi = np.full(nq, -1.5), np.full(nq, 1.5)
        for _ in range(50):
            mid = (lo + hi) / 2
            zz = (mid[:, None] - mu) / sig[v]
            p = student.sf(zz / tscale[None, :], nu[None, :]) if tail == "t" else ndtr(-zz)
            cnt = (sizes[None, :] * p).sum(1)
            lo, hi = np.where(cnt > KNN, mid, lo), np.where(cnt > KNN, hi, mid)
        return (lo + hi) / 2

    for v in ("r4", "r16", "exact"):
        for tail in (("g", "t") if v != "r4" else ("g",)):
            tm = tau_model(v, tail)
            res.setdefault("tau_model_minus_true", {})[f"{v} {tail}"] = float(np.mean(tm - tau_true))
            for tn, tau in (("model", tm), ("true", tau_true)):
                lp = logp(tau, v, tail)
                add(f"thr-{tail} {v} {tn} row", lp)
                add(f"thr-{tail} {v} {tn} probe", lp + np.log(sizes)[None, :])

    # running threshold (adaptive)
    def running(v, tail, per, every, start="o3", tm=None):
        # start "o3": before k rows are scanned, the O3 key; "model": tau_model until then; "max": max(running, model)
        bonus = (tq if tail == "t" else zq)[None, :] * sig[v]
        key0 = mu + bonus if start == "o3" else logp(tm, v, tail) + (np.log(sizes)[None, :] if per else 0)
        probed = np.zeros((nq, K), bool)
        ordr = np.zeros((nq, K), np.int64)
        run = np.full((nq, KNN), -np.inf)
        scanned = np.zeros(nq)
        ranking = None
        ar = np.arange(nq)
        for step in range(K):
            if step == 0 or step % every == 0 or ranking is None:
                tau = run[:, KNN - 1]
                if start == "max":
                    tau = np.maximum(tau, tm)
                ok = np.isfinite(tau)
                key = np.where(ok[:, None], logp(np.where(ok, tau, 0.0), v, tail) + (np.log(sizes)[None, :] if per else 0), key0)
                key = np.where(probed, -np.inf, key)
                ranking = np.argsort(-key, axis=1, kind="stable")
                ptr = np.zeros(nq, np.int64)
            # next unprobed cluster in the current ranking
            nxt = ranking[ar, ptr]
            while True:
                bad = probed[ar, nxt]
                if not bad.any():
                    break
                ptr[bad] += 1
                nxt = ranking[ar, np.minimum(ptr, K - 1)]
            ordr[:, step] = nxt
            probed[ar, nxt] = True
            ptr += 1
            scanned += sizes[nxt]
            run = -np.sort(-np.concatenate([run, topk[ar, nxt]], axis=1), axis=1)[:, :KNN]
            if scanned.min() > 0.31 * N:  # beyond the reported range: the rest in the last ranking's order
                for i in range(nq):
                    rest = [c for c in ranking[i] if not probed[i, c]]
                    ordr[i, step + 1:] = rest
                break
        return ordr

    for v in ("r4", "r16", "exact"):
        for per, pn in ((False, "row"), (True, "probe")):
            for every in (1, 8):
                add(f"thr-g {v} running/{every} {pn}", ordr=running(v, "g", per, every))
    for v in ("r16", "exact"):
        for per, pn in ((False, "row"), (True, "probe")):
            add(f"thr-t {v} running/1 {pn}", ordr=running(v, "t", per, 1))
    for v in ("r4", "r16"):
        tm = tau_model(v, "g")
        for every in (1, 8):
            add(f"thr-g {v} model->running/{every} row", ordr=running(v, "g", False, every, "model", tm))
            add(f"thr-g {v} max(model,running)/{every} row", ordr=running(v, "g", False, every, "max", tm))
    res["oracle"] = max(FGRID, key=lambda f: res["orders"][f"rad f={f}"]["avg"])
    res["own_best_f2"] = max(F2, key=lambda f: res["orders"][f"O2 f'={f}"]["avg"])
    json.dump(res, open(os.path.join(outdir, f"{qname}.json"), "w"), indent=1)
    top = sorted(res["orders"].items(), key=lambda kv: -kv[1]["avg"])[:6]
    oa = res["orders"][f"rad f={res['oracle']}"]["avg"]
    log(f"{qname}: {nq} queries in {time.time() - t1:.0f} s; oracle f {res['oracle']} ({oa:.3f}); own f' {res['own_best_f2']}; "
        + "; ".join(f"{k} {v['avg']:.3f}" for k, v in top))
