# Recall targets for the stopping rule, on one fit (merged defaults) and several query sets. For each calibrated order
# (H r16, a shared-basis H, O2 f'=0.4, the calibrated zeta-order, the rad-aware model), each cluster gets an expected
# number of true neighbours e_c = n_c p_c from a calibration fitted on corpus pseudo-queries only ("plain": 1,000 corpus
# rows; "mixed": those plus 1,000 shifted ones):
#   gap orders (H, O2): e_c = E[true neighbours | (s_max - s_c)/sd(q)], isotonic regression on the pseudo-queries
#   zeta-order: sigma~ calibrated as in orders4.py (a, b, c0 by a log-domain fit), e_c = n_c P(score > tau_model)
#   rad-model: e_c = n_c exp(g(zeta_c(tau_model)) + beta log(rad_c / med)), g and beta fitted as in orders5.py
# Targets:
#   per query: visit in the order's ranking until sum_visited e_c >= r sum_all e_c (r 0.8, 0.9, 0.95, 0.99)
#   running (model orders): after each probe solve #{found >= t} + sum_unvisited n_c P_c(t) = k (theory 4.2) and stop
#     once #{found >= t}/k >= r
#   average: a global threshold lambda on e_c / n_c chosen on the pseudo-queries (their true recall) to give mean
#     recall r, applied to the test queries; its rows against a fixed nprobe of the same order reaching the same recall
# usage: python -I targets5.py FITDIR OUTDIR DSNAME SHARED_SPREAD ALPHA W QSET=QUERIES.npy:GT.npy [...]
import sys, os, json, time
import numpy as np
from scipy.stats import norm
from scipy.special import ndtr
from scipy.optimize import minimize, least_squares, nnls

fitdir, outdir, dsname, shname = sys.argv[1:5]
sh_a, sh_w = float(sys.argv[5]), float(sys.argv[6])
qsets = [a.split("=", 1) for a in sys.argv[7:] if "=" in a]
t0 = time.time()
log = lambda *a: print(f"[{dsname} {time.time() - t0:5.0f}s]", *a, flush=True)
KNN = 10
TARGETS = (0.8, 0.9, 0.95, 0.99)

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
Rcap = lambda R: min(R, n0 - 8)

# build-time statistics (as orders5.py, without the group bases)
Mean = np.zeros((K, n)); rad_q = np.zeros(K); lam = np.zeros((K, 16)); U = np.zeros((K, n, 16)); tr = np.zeros(K)
Cres = np.zeros((n, n))
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
w, v = np.linalg.eigh(Cres / N)
Us = v[:, ::-1][:, :128]
lsh = np.zeros((K, 128))
for c in range(K):
    xc = np.asarray(Xs[cstart[c]:cstart[c + 1]], np.float64) - Mean[c]
    lsh[c] = ((xc @ Us) ** 2).mean(0)
rad_n = rad_q / rad_q.std()
zq = np.where(sizes > KNN, norm.ppf(1 - KNN / np.maximum(sizes, KNN + 1)), 0.0)
lr = np.log(rad_q / np.median(rad_q))
log(f"K {K}, N {N}, n {n}; stats in {time.time() - t0:.0f} s")


def spread(Qr, name):
    qq = (Qr ** 2).sum(1)[:, None]
    nq = len(Qr)
    proj = (Qr.astype(np.float32) @ np.ascontiguousarray(U.transpose(1, 0, 2).reshape(n, K * 16), dtype=np.float32)).astype(np.float64).reshape(nq, K, 16)
    if name == "own r16":
        rest = np.maximum(tr - lam.sum(1), 0) / max(n0 - 16, 1)
        v = (proj ** 2 * lam[None]).sum(2) + rest[None, :] * np.maximum(qq - (proj ** 2).sum(2), 0)
    elif name.startswith("sh"):
        R0 = int(name[2:].split("+")[0]); R = Rcap(R0)
        p = Qr @ Us[:, :R]
        if "+own" in name:
            T = int(name.split("+own")[1])
            M = np.einsum("knt,nr->ktr", U[:, :, :T], Us[:, :R])
            lp = np.maximum(lsh[:, :R] - (lam[:, :T, None] * M ** 2).sum(1), 0)
            rem = np.maximum(tr - lam[:, :T].sum(1) - lp.sum(1), 0) / max(n0 - R - T, 1)
            v = (proj[:, :, :T] ** 2 * lam[None, :, :T]).sum(2) + (p ** 2) @ lp.T + rem[None, :] * np.maximum(qq - (p ** 2).sum(1, keepdims=True), 0)
        else:
            rem = np.maximum(tr - lsh[:, :R].sum(1), 0) / max(n0 - R, 1)
            v = (p ** 2) @ lsh[:, :R].T + rem[None, :] * np.maximum(qq - (p ** 2).sum(1, keepdims=True), 0)
    return np.sqrt(np.maximum(v, 1e-12))


def cluster_pass(Qr, self_rows=None):
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
    return var, t.astype(np.float64), Nc, tk


# pseudo-queries, as orders5.py (same seed and construction)
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
shq = np.zeros((1000, n))
shq[:, :n0] = ap[:, None] * base[:, :n0] + Rr
shq /= np.linalg.norm(shq, axis=1, keepdims=True)
PQ = f16(np.concatenate([plain, shq]))
v1, t1_, N1, _ = cluster_pass(PQ[:1000], self_rows=prow)
v2, t2_, N2, _ = cluster_pass(PQ[1000:])
pvar, pt, pN = np.concatenate([v1, v2]), np.concatenate([t1_, t2_]), np.concatenate([N1, N2])
pmu, pqm = PQ @ Mean.T, PQ @ Mq.T
psd = pqm.std(1, keepdims=True)
psig = {"own r16": spread(PQ, "own r16"), shname: spread(PQ, shname)}
log(f"pseudo-queries done in {time.time() - t0:.0f} s")
SUB = {"plain": slice(0, 1000), "mixed": slice(0, 2000)}

# ---- calibrations
DB = np.arange(0, 40.01, 0.05)


def pav_decreasing(y, w_):
    yy, ww, cnt = [], [], []
    for a, b_ in zip(y, w_):
        yy.append(a); ww.append(b_); cnt.append(1)
        while len(yy) > 1 and yy[-2] < yy[-1]:
            a2, w2, c2 = yy.pop(), ww.pop(), cnt.pop()
            yy[-1] = (yy[-1] * ww[-1] + a2 * w2) / (ww[-1] + w2); ww[-1] += w2; cnt[-1] += c2
    return np.repeat(yy, cnt)


def gap_table(score_p, sdp, Np):
    d = ((score_p.max(1, keepdims=True) - score_p) / sdp).ravel()
    b = np.clip(np.searchsorted(DB, d) - 1, 0, len(DB) - 1)
    s_ = np.bincount(b, Np.ravel(), minlength=len(DB)); c_ = np.bincount(b, minlength=len(DB)).astype(float)
    nz = c_ > 0
    y = np.zeros(len(DB)); y[nz] = pav_decreasing(s_[nz] / c_[nz], c_[nz])
    last = 0.0
    for i in range(len(DB) - 1, -1, -1):
        if nz[i]:
            last = y[i]
        else:
            y[i] = last
    return y


def gap_e(score, sd, tab):
    d = (score.max(1, keepdims=True) - score) / sd
    return tab[np.clip(np.searchsorted(DB, d) - 1, 0, len(DB) - 1)]


def sigma_cal(sl):
    """orders4.py's log-domain fit of sigma~^2 = a v16 + b sd^2 rad^2 + c0 sd^2 on the pseudo-queries in sl."""
    y = pvar[sl].ravel()
    F = np.stack([(psig["own r16"][sl] ** 2).ravel(), (psd[sl] ** 2 * rad_q[None, :] ** 2).ravel(),
                  np.broadcast_to(psd[sl] ** 2, pvar[sl].shape).ravel()], 1)
    okk = y > 1e-10
    y, F = y[okk], F[okk]
    sub = np.random.default_rng(1).choice(len(y), min(len(y), 200_000), replace=False)
    sc = F[sub].std(0)
    cv, _ = nnls(F[sub] / sc, y[sub])
    cv = cv / sc
    x0 = np.maximum(cv, 1e-3 * y[sub].mean() / np.maximum(F[sub].mean(0), 1e-30))
    return least_squares(lambda th: np.log(F[sub] @ th + 1e-30) - np.log(y[sub]), x0, bounds=(0, np.inf), x_scale=1 / sc).x


ZB = np.arange(-2.0, 14.01, 0.25)


def fit_model(sl, s):
    zt = ((pt[sl, None] - pmu[sl]) / s[sl]).ravel()
    Nn = pN[sl].ravel(); nn = np.broadcast_to(sizes, pN[sl].shape).ravel().astype(float); ll = np.broadcast_to(lr, pN[sl].shape).ravel()
    b = np.clip(np.searchsorted(ZB, zt) - 1, 0, len(ZB) - 1)
    nb, pen = len(ZB), 2.0 * len(Nn) / 1e4

    def f(th):
        g, beta = th[:nb], th[nb]
        eta = g[b] + beta * ll
        lam_ = nn * np.exp(np.minimum(eta, 5))
        d2 = g[2:] - 2 * g[1:-1] + g[:-2]
        nll = (lam_ - Nn * eta).sum() + pen * (d2 ** 2).sum()
        r = lam_ - Nn
        gg = np.bincount(b, r, minlength=nb)
        gg[2:] += 2 * pen * d2; gg[1:-1] -= 4 * pen * d2; gg[:-2] += 2 * pen * d2
        return nll, np.r_[gg, (r * ll).sum()]

    th0 = np.r_[np.log(np.maximum(norm.sf(ZB), 1e-12)), 0.0]
    x = minimize(f, th0, jac=True, method="L-BFGS-B", options=dict(maxiter=2000)).x
    return x[:nb], float(x[nb])


def g_eval(g, zz):
    slope = (g[-1] - g[-5]) / (ZB[-1] - ZB[-5])
    return np.where(zz > ZB[-1], g[-1] + slope * (zz - ZB[-1]), np.interp(zz, ZB, g))


def tau_solve(mu, s, pfun, found=None, unv=None):
    """t with #{found >= t} + sum_unvisited n_c P_c(t) = k (found: nq x m scores; unv: nq x K mask)."""
    nq = len(mu)
    lo, hi = np.full(nq, -1.5), np.full(nq, 1.5)
    for _ in range(30):
        mid = (lo + hi) / 2
        e = sizes[None, :] * pfun((mid[:, None] - mu) / s)
        cnt = (e * unv).sum(1) if unv is not None else e.sum(1)
        if found is not None:
            cnt = cnt + (found >= mid[:, None]).sum(1)
        lo, hi = np.where(cnt > KNN, mid, lo), np.where(cnt > KNN, hi, mid)
    return (lo + hi) / 2


CAL = {}
for cs, sl in SUB.items():
    a, b, c0 = sigma_cal(sl)
    g, beta = fit_model(sl, psig["own r16"])
    CAL[cs] = dict(sig=(a, b, c0), model=(g, beta))
    log(f"calibration {cs}: sigma~ a {a:.3f} b {b:.3f} c0 {c0:.4f}; rad-model beta {beta:+.2f}")


def order_defs(Qr, sdq, qm, mu, sig16, sigsh, cs, pseudo):
    """For each order: (score for the ranking, expected true neighbours per cluster e_c, per-row tail function or None)."""
    a, b, c0 = CAL[cs]["sig"]
    g, beta = CAL[cs]["model"]
    sl = SUB[cs]
    out = {}
    H16 = lambda q_m, sd, s: q_m + 0.75 * zq[None, :] * s + 0.3 * sd * rad_n[None, :]
    HSH = lambda q_m, sd, s: q_m + sh_a * zq[None, :] * s + sh_w * sd * rad_n[None, :]
    O2 = lambda q_m, sd: q_m + 0.4 * sd * rad_n[None, :]
    for nm, sc, scp in (("H r16", H16(qm, sdq, sig16), H16(pqm[sl], psd[sl], psig["own r16"][sl])),
                        (f"H {shname}", HSH(qm, sdq, sigsh), HSH(pqm[sl], psd[sl], psig[shname][sl])),
                        ("O2 f'=0.4", O2(qm, sdq), O2(pqm[sl], psd[sl]))):
        tab = gap_table(scp, psd[sl], pN[sl])
        out[nm] = (sc, gap_e(sc, sdq, tab), None, gap_e(scp, psd[sl], tab))
    st = np.sqrt(np.maximum(a * sig16 ** 2 + b * sdq ** 2 * rad_q[None, :] ** 2 + c0 * sdq ** 2, 1e-12))
    stp = np.sqrt(np.maximum(a * psig["own r16"][sl] ** 2 + b * psd[sl] ** 2 * rad_q[None, :] ** 2 + c0 * psd[sl] ** 2, 1e-12))
    pz = lambda z: ndtr(-z)
    tm = tau_solve(mu, st, pz); tmp = tau_solve(pmu[sl], stp, pz)
    e = sizes[None, :] * pz((tm[:, None] - mu) / st); ep = sizes[None, :] * pz((tmp[:, None] - pmu[sl]) / stp)
    out["zeta cal r16"] = (np.log(np.maximum(e / sizes[None, :], 1e-300)), e, (mu, st, pz), ep)
    pm = lambda z: np.exp(np.minimum(g_eval(g, z) + beta * lr[None, :], 0))
    tm = tau_solve(mu, sig16, pm); tmp = tau_solve(pmu[sl], psig["own r16"][sl], pm)
    e = sizes[None, :] * pm((tm[:, None] - mu) / sig16); ep = sizes[None, :] * pm((tmp[:, None] - pmu[sl]) / psig["own r16"][sl])
    out["rad-model r16"] = (np.log(np.maximum(e / sizes[None, :], 1e-300)), e, (mu, sig16, pm), ep)
    return out


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
    _, _, _, tk = cluster_pass(Qr)
    mu, qm = Qr @ Mean.T, Qr @ Mq.T
    sdq = qm.std(1, keepdims=True)
    sig16, sigsh = spread(Qr, "own r16"), spread(Qr, shname)
    res = dict(name=qname, dataset=dsname, nq=nq, shared=f"{shname} a{sh_a} w{sh_w}", results={})
    ar = np.arange(nq)
    for cs in SUB:
        defs = order_defs(Qr, sdq, qm, mu, sig16, sigsh, cs, None)
        for oname, (score, e, tail, ep) in defs.items():
            ordr = np.argsort(-score, axis=1, kind="stable")
            es = np.take_along_axis(e, ordr, 1)
            ces = np.cumsum(es, 1) / es.sum(1, keepdims=True)
            rec = np.cumsum(np.take_along_axis(Nt, ordr, 1), 1) / KNN
            items = np.cumsum(sizes[ordr], 1).astype(float)
            mr_curve, ms_curve = rec.mean(0), items.mean(0) / N
            out = {}
            for r in TARGETS:
                p_ = np.minimum((ces < r - 1e-12).sum(1), K - 1)  # clusters needed - 1
                ach = rec[ar, p_]
                out[f"per-query {r}"] = dict(recall=float(ach.mean()), below=float((ach < r - 1e-9).mean()), share=float(items[ar, p_].mean() / N))
            # running (4.2), model orders only
            if tail is not None:
                mu_, s_, pf = tail
                unv = np.ones((nq, K)); found = np.full((nq, KNN), -np.inf)
                stopped = {r: np.full(nq, -1) for r in TARGETS}
                for step in range(K):
                    c_ = ordr[:, step]
                    unv[ar, c_] = 0
                    found = -np.sort(-np.concatenate([found, tk[ar, c_]], 1), 1)[:, :KNN]
                    th = tau_solve(mu_, s_, pf, found=found, unv=unv)
                    pred = (found >= th[:, None]).sum(1) / KNN
                    for r in TARGETS:
                        hit = (stopped[r] < 0) & (pred >= r - 1e-12)
                        stopped[r][hit] = step
                    if all((stopped[r] >= 0).all() for r in TARGETS) or items[:, step].min() > 0.5 * N:
                        break
                for r in TARGETS:
                    p_ = np.where(stopped[r] >= 0, stopped[r], K - 1)
                    ach = rec[ar, p_]
                    out[f"running {r}"] = dict(recall=float(ach.mean()), below=float((ach < r - 1e-9).mean()), share=float(items[ar, p_].mean() / N))
            # average target: lambda on e_c / n_c chosen on the pseudo-queries (their true recall) for mean recall r
            sl = SUB[cs]
            lpp = np.log(np.maximum(ep / sizes[None, :], 1e-300))
            lpt = np.log(np.maximum(e / sizes[None, :], 1e-300))
            lams = np.unique(np.quantile(lpp, np.linspace(0, 1, 1000)))[::-1]
            prec = np.array([((lpp >= l_) * pN[sl]).sum(1).mean() / KNN for l_ in lams])
            for r in TARGETS:
                i = int(np.argmax(prec >= r)) if (prec >= r).any() else len(lams) - 1
                sel = lpt >= lams[i]
                ach_q = (sel * Nt).sum(1) / KNN
                ach, share = float(ach_q.mean()), float((sel * sizes[None, :]).sum(1).mean() / N)
                # fixed nprobe of the same order reaching the same mean recall
                j = int(np.argmax(mr_curve >= ach - 1e-12)) if (mr_curve >= ach - 1e-12).any() else K - 1
                out[f"average {r}"] = dict(recall=ach, below=float((ach_q < r - 1e-9).mean()), share=share,
                                           fixed_share_same_recall=float(ms_curve[j]), pseudo_recall=float(prec[i]))
            res["results"][f"{oname} | {cs}"] = out
    json.dump(res, open(os.path.join(outdir, f"targets-{qname}.json"), "w"), indent=1)
    log(f"{qname}: {nq} queries in {time.time() - t1:.0f} s")
    for k, v in res["results"].items():
        log(f"  {k:28s} " + " ".join(f"{t}:{v[t]['recall']:.3f}/{v[t]['share'] * 100:.1f}%" for t in v if t.endswith("0.9")))
