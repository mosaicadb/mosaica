"""Small numpy replay for the approximate track (A1 profile and law; A2 stop on expected misses).

Counts only; nothing is timed. Reads a suite fit (clusters, groups) and the prep's queries and visible sets, read-only.
Every cluster's mu_c and sigma_c are the exact mean and standard deviation of ALL its rows' scores along the query
(the library has sigma from 16 directions; this is the order's own limit, as layered.md's replay A).
Clusters are read whole in the static order mu_c + 3 sigma_c; search inside a read cluster is exact.

usage: python3 replay_np.py DATASET OUT.json [sets,comma,separated]
"""
import json, sys, math, time
import numpy as np
from scipy.stats import norm

SUITE = "/home/joe/code/vexact-data/suite"
KNN = 10
PSIS = [round(2 + 0.25 * i, 2) for i in range(41)]            # 2 .. 12
MISS = [5, 4, 3, 2.5, 2, 1.5, 1.25, 1, 0.8, 0.65, 0.5, 0.4, 0.3, 0.2, 0.15, 0.1, 0.07, 0.05, 0.03, 0.02, 0.01, 0.005, 0.002, 0.001]
TARGETS = [0.9, 0.95, 0.98, 0.99]


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


def kmeans(X, k, iters=6, seed=0):
    n = X.shape[0]
    if n <= k:
        return np.arange(n) % k
    rng = np.random.default_rng(seed)
    C = X[rng.choice(n, k, replace=False)].copy()
    lab = np.zeros(n, dtype=np.int64)
    for _ in range(iters):
        d = X @ C.T * -2 + (C * C).sum(1)[None, :]
        lab = d.argmin(1)
        for j in range(k):
            m = lab == j
            if m.any():
                C[j] = X[m].mean(0)
    return lab


def main():
    ds = sys.argv[1]
    out = sys.argv[2]
    sets = sys.argv[3].split(",") if len(sys.argv) > 3 else ["none", "random-25", "region-25", "region-1", "random-1"]
    fit, prep = f"{SUITE}/fit/{ds}", f"{SUITE}/prep/{ds}"
    Xs = np.load(fit + "/Xs.npy", mmap_mode="r")
    cstart = np.load(fit + "/cstart.npy")
    grp = np.load(fit + "/grp.npy")
    perm = np.load(fit + "/perm.npy")
    Q = np.load(prep + "/queries.npy").astype(np.float64)
    Q /= np.linalg.norm(Q, axis=1, keepdims=True)
    N, n = Xs.shape
    K = len(cstart) - 1
    nq = Q.shape[0]
    Qp = np.zeros((nq, n), dtype=np.float32)
    Qp[:, : Q.shape[1]] = Q
    nc = np.diff(cstart)
    cid = np.repeat(np.arange(K), nc)
    log(ds, "N", N, "n", n, "K", K, "nq", nq)

    # scores, and sub-clusters (nested 4-way twice: a4, a16) made on the way
    S = np.empty((nq, N), dtype=np.float32)
    sub4 = np.empty(N, dtype=np.int64)
    sub16 = np.empty(N, dtype=np.int64)
    for c in range(K):
        a, b = cstart[c], cstart[c + 1]
        X = np.asarray(Xs[a:b], dtype=np.float32)
        S[:, a:b] = Qp @ X.T
        l4 = kmeans(X, 4, seed=c)
        l16 = np.zeros(b - a, dtype=np.int64)
        for j in range(4):
            m = l4 == j
            if m.any():
                l16[m] = kmeans(X[m], 4, seed=1000 + c * 4 + j)
        sub4[a:b] = c * 4 + l4
        sub16[a:b] = (c * 4 + l4) * 4 + l16
    log("scores and sub-clusters done")

    res = {"dataset": ds, "N": int(N), "K": int(K), "G": int(grp.max() + 1), "nq": int(nq), "k": KNN}

    # ---------- A1: the variance profile and the leaf law ----------
    tot_m = S.mean(1, dtype=np.float64)
    tot_v = S.var(1, dtype=np.float64)
    sums = np.add.reduceat(S.astype(np.float64), cstart[:-1], axis=1)
    mu = sums / nc
    sq = np.zeros((nq, K))
    for i in range(nq):
        sq[i] = np.add.reduceat(S[i].astype(np.float64) ** 2, cstart[:-1])
    sig = np.sqrt(np.maximum(sq / nc - mu * mu, 1e-18))

    def share(labels, nl):
        cnt = np.bincount(labels, minlength=nl).astype(np.float64)
        f = np.empty(nq)
        for i in range(nq):
            sm = np.bincount(labels, weights=S[i], minlength=nl)
            mean = np.divide(sm, cnt, out=np.zeros(nl), where=cnt > 0)
            f[i] = (cnt * (mean - tot_m[i]) ** 2).sum() / N / tot_v[i]
        return f, int((cnt > 0).sum())

    prof = {}
    for name, lab, nl in (("groups", grp[cid], int(grp.max() + 1)), ("clusters", cid, K), ("a4", sub4, 4 * K), ("a16", sub16, 16 * K)):
        f, cells = share(lab, nl)
        prof[name] = {"cells": cells, "g": math.log(cells) / math.log(N), "f_mean": float(f.mean()), "f_p10": float(np.quantile(f, 0.1)), "f_p90": float(np.quantile(f, 0.9))}
        if name == "clusters":
            f_cl = f
        log("profile", name, prof[name])
    res["profile"] = prof

    # the top-k level in units of the total spread, on the full set and on random subsamples (the REM level for comparison)
    rng = np.random.default_rng(1)
    zT = {}
    for frac in (1, 4, 16, 64):
        idx = np.arange(N) if frac == 1 else np.sort(rng.choice(N, N // frac, replace=False))
        Ss = S[:, idx] if frac > 1 else S
        tk = np.partition(Ss, Ss.shape[1] - KNN, axis=1)[:, Ss.shape[1] - KNN]
        z = (tk - tot_m) / np.sqrt(tot_v)
        zT[f"N/{frac}"] = {"rows": int(len(idx)), "zT_mean": float(z.mean()), "zT_p10": float(np.quantile(z, 0.1)), "zT_p90": float(np.quantile(z, 0.9)), "rem_level": float(norm.isf(KNN / len(idx)))}
        if frac == 1:
            tau = tk.copy()
            zT_full = z.copy()
        log("zT", f"N/{frac}", zT[f"N/{frac}"])
    res["zT"] = zT

    # the law's two parts at the cluster level, per query: threshold a = f zT - z_r sqrt(f(1-f)) on the standardised
    # centre score; (1) coverage of the true neighbours' clusters, (2) share of rows in clusters above the threshold
    A = (mu - tot_m[:, None]) / np.sqrt(tot_v)[:, None]            # standardised cluster means
    law = {}
    istop = [np.nonzero(S[i] >= tau[i])[0][:KNN + 5] for i in range(nq)]
    anc = np.concatenate([A[i, cid[istop[i]]] for i in range(nq)])
    anc_pred = np.concatenate([np.full(len(istop[i]), f_cl[i] * zT_full[i]) for i in range(nq)])
    anc_sd = np.concatenate([np.full(len(istop[i]), math.sqrt(f_cl[i] * (1 - f_cl[i]))) for i in range(nq)])
    res["ancestor"] = {"mean_measured": float(anc.mean()), "mean_bridge": float(anc_pred.mean()), "resid_sd_measured": float((anc - anc_pred).std()), "resid_sd_model": float(anc_sd.mean()),
                       "std_resid_mean": float(((anc - anc_pred) / anc_sd).mean()), "std_resid_sd": float(((anc - anc_pred) / anc_sd).std()),
                       "std_resid_q": [float(x) for x in np.quantile((anc - anc_pred) / anc_sd, [0.01, 0.05, 0.25, 0.5, 0.75, 0.95, 0.99])]}
    log("ancestor", res["ancestor"])
    for r in TARGETS:
        zr = norm.ppf(r)
        cov, shr, pred = [], [], []
        for i in range(nq):
            f = f_cl[i]
            a = f * zT_full[i] - zr * math.sqrt(f * (1 - f))
            keep = A[i] >= a
            cov.append(keep[cid[istop[i]]].mean())
            shr.append(nc[keep].sum() / N)
            pred.append(norm.sf(zT_full[i] * math.sqrt(f) - zr * math.sqrt(1 - f)))
        law[str(r)] = {"coverage_measured": float(np.mean(cov)), "share_measured": float(np.mean(shr)), "share_law": float(np.mean(pred))}
        log("law", r, law[str(r)])
    res["law"] = law
    # cluster-mean distribution: is the standardised centre score N(0, f)? and the within spread homoscedastic?
    res["centres"] = {"A_sd_over_sqrt_f": float((A.std(1) / np.sqrt(f_cl)).mean()), "A_excess_kurtosis": float((((A - A.mean(1, keepdims=True)) ** 4).mean(1) / A.var(1) ** 2 - 3).mean()),
                      "sigma_cv_within_query": float((sig.std(1) / sig.mean(1)).mean()), "corr_mu_sigma": float(np.mean([np.corrcoef(mu[i], sig[i])[0, 1] for i in range(nq)]))}
    log("centres", res["centres"])

    # ---------- A2: the stop, replayed ----------
    inv_vis = {}
    res["sets"] = {}
    for st in sets:
        if st == "none":
            vrows = np.arange(N)
        else:
            vis = np.load(f"{prep}/vis_{st}.npy")
            mask = np.zeros(N, dtype=bool)
            ent = np.zeros(N, dtype=bool)
            ent[vis] = True
            mask = ent[perm]
            vrows = np.nonzero(mask)[0]
        vstart = np.searchsorted(vrows, cstart)
        vc = np.diff(vstart)
        thr_file = np.load(f"{prep}/knn_thr.npy" if st == "none" else f"{prep}/knn_thr_{st}.npy")
        per_q = []
        chk = []
        for i in range(nq):
            sv = S[i, vrows]
            kk = min(KNN, len(sv))
            tau_v = np.partition(sv, len(sv) - kk)[len(sv) - kk]
            chk.append(abs(tau_v - thr_file[i, 0]))
            top_rows = np.nonzero(sv >= tau_v)[0]
            Nc = np.bincount(np.searchsorted(vstart, top_rows, side="right") - 1, minlength=K)
            order = np.argsort(-(mu[i] + 3 * sig[i]))
            order = order[vc[order] > 0]
            unread = vc > 0
            topk = np.empty(0, dtype=np.float32)
            n_pool = 0; zs = 0.0; zss = 0.0
            exc = np.empty(0, dtype=np.float64)
            rows = 0; found = 0
            steps = []
            for m, c in enumerate(order):
                x = sv[vstart[c]:vstart[c + 1]]
                z = (x.astype(np.float64) - mu[i, c]) / sig[i, c]
                rows += len(x); found += int(Nc[c]); unread[c] = False
                n_pool += len(z); zs += z.sum(); zss += (z * z).sum()
                exc = np.sort(np.concatenate([exc, z[z > 0.5]]))
                topk = np.concatenate([topk, x])
                if len(topk) > kk:
                    topk = np.partition(topk, len(topk) - kk)[len(topk) - kk:]
                rec = {"rows": rows, "recall": found / kk, "m": m + 1}
                if len(topk) >= kk and unread.any() and n_pool >= 2:
                    th = float(topk.min())
                    u = np.nonzero(unread)[0]
                    zeta = (th - mu[i, u]) / sig[i, u]
                    v = vc[u].astype(np.float64)
                    delta = zs / n_pool
                    rho = math.sqrt(max(zss / n_pool - delta * delta, 1e-12))
                    rec["a"] = float(((zeta - delta) / rho).min())
                    rec["zeta_min"] = float(zeta.min())
                    rec["delta"] = delta; rec["rho"] = rho
                    rec["M_G"] = float((v * norm.sf((zeta - delta) / rho)).sum())
                    rec["M_G0"] = float((v * norm.sf(zeta)).sum())                 # the unpooled Gaussian, delta 0 rho 1
                    ne = len(exc)
                    # two upper quantiles of the pooled z (exceedance shares p1, p2), a normal through them
                    p1, p2 = 0.05, 0.005
                    k1, k2 = int(round(p1 * n_pool)), int(round(p2 * n_pool))
                    if k2 >= 8 and k1 < ne:
                        q1, q2 = exc[ne - k1], exc[ne - k2]
                        rt = (q2 - q1) / (norm.isf(p2) - norm.isf(p1))
                        dt = q1 - rt * norm.isf(p1)
                        rec["M_Q2"] = float((v * norm.sf((zeta - dt) / max(rt, 1e-6))).sum())
                        rec["dt"] = float(dt); rec["rt"] = float(rt)
                        rec["a_Q2"] = float(((zeta - dt) / max(rt, 1e-6)).min())
                    else:
                        rec["M_Q2"] = rec["M_G"]; rec["a_Q2"] = rec["a"]
                    # generalised Pareto above the 5% level (moment estimators), empirical below it
                    if k1 >= 40 and k1 < ne:
                        uu = exc[ne - k1]
                        ex = exc[ne - k1:] - uu
                        m1 = ex.mean(); s2 = ex.var()
                        xi = 0.5 * (1 - m1 * m1 / max(s2, 1e-12)); beta = 0.5 * m1 * (m1 * m1 / max(s2, 1e-12) + 1)
                        y = zeta - uu
                        emp = (ne - np.searchsorted(exc, zeta, side="left")) / n_pool
                        emp = np.where(zeta < 0.5, np.maximum(norm.sf((zeta - delta) / rho), emp), emp)
                        if abs(xi) < 1e-6:
                            tail = p1 * np.exp(-np.maximum(y, 0) / beta)
                        else:
                            tail = p1 * np.maximum(1 + xi * np.maximum(y, 0) / beta, 0) ** (-1 / xi)
                        rec["M_GPD"] = float((v * np.where(y > 0, tail, emp)).sum())
                        rec["M_EXP"] = float((v * np.where(y > 0, p1 * np.exp(-np.maximum(y, 0) / m1), emp)).sum())
                        rec["xi"] = float(xi)
                    else:
                        rec["M_GPD"] = rec["M_G"]; rec["M_EXP"] = rec["M_G"]
                    xs_un = np.concatenate([sv[vstart[c2]:vstart[c2 + 1]] for c2 in u[np.argsort(zeta)[:40]]]) if len(u) else np.empty(0)
                    rec["X_true"] = int((xs_un >= th).sum())          # rows above the running k-th best in the 40 nearest unread clusters
                    rec["n_pool"] = n_pool; rec["n_z2"] = int(ne - np.searchsorted(exc, 2.0)); rec["n_z3"] = int(ne - np.searchsorted(exc, 3.0))
                    rec["miss"] = kk - found
                steps.append(rec)
                if "a" in rec and rec["a"] > 12.5 and rec["M_Q2"] < 5e-4 and rec["M_GPD"] < 5e-4 and rec["M_G"] < 5e-4 and found == kk:
                    break
            per_q.append(steps)
        log(st, "visible", len(vrows), "max |tau - prep's threshold|", float(np.max(chk)))

        def run_rule(key, grid, bigger_stops):
            curve = []
            for g in grid:
                rr, cc, rws = [], [], []
                for steps in per_q:
                    s = steps[-1]
                    for t in steps:
                        if key in t and ((t[key] > g) if bigger_stops else (t[key] < g)):
                            s = t; break
                    rr.append(s["recall"]); rws.append(s["rows"]); cc.append(s["m"])
                curve.append({"par": g, "recall": float(np.mean(rr)), "rows": float(np.mean(rws)), "clusters": float(np.mean(cc)), "below_0.9": float(np.mean(np.array(rr) < 0.9 - 1e-9)),
                              "recall_p10": float(np.quantile(rr, 0.1)), "recall_sd": float(np.std(rr))})
            return curve

        def at_recall(curve, target):
            c = sorted(curve, key=lambda e: e["rows"])
            for a, b in zip(c, c[1:]):
                if a["recall"] < target <= b["recall"]:
                    w = (target - a["recall"]) / (b["recall"] - a["recall"])
                    return a["rows"] + w * (b["rows"] - a["rows"])
            return None

        rules = {"S0 psi (body moments)": ("a", PSIS, True), "S0 psi (two upper quantiles)": ("a_Q2", PSIS, True),
                 "S1 Gaussian, prior (0,1)": ("M_G0", MISS, False), "S1 Gaussian, body moments": ("M_G", MISS, False), "S1 two upper quantiles": ("M_Q2", MISS, False),
                 "S1 generalised Pareto": ("M_GPD", MISS, False), "S1 exponential tail": ("M_EXP", MISS, False)}
        outset = {"visible": int(len(vrows)), "rules": {}}
        for name, (key, grid, big) in rules.items():
            cv = run_rule(key, grid, big)
            outset["rules"][name] = {"curve": cv, "rows_at": {str(t): at_recall(cv, t) for t in TARGETS}}
        # fixed number of clusters, and each query stopped by an oracle at its own recall target
        mx = max(len(s) for s in per_q)
        fx = []
        for m in range(1, mx + 1):
            rr = [s[min(m, len(s)) - 1]["recall"] for s in per_q]; rws = [s[min(m, len(s)) - 1]["rows"] for s in per_q]
            fx.append({"par": m, "recall": float(np.mean(rr)), "rows": float(np.mean(rws))})
        outset["rules"]["fixed nprobe"] = {"rows_at": {str(t): at_recall(fx, t) for t in TARGETS}}
        orc = {}
        for t in TARGETS:
            rws = []
            for s in per_q:
                e = next((x for x in s if x["recall"] >= t - 1e-9), s[-1]); rws.append(e["rows"])
            orc[str(t)] = float(np.mean(rws))
        outset["rules"]["oracle per-query stop (every query >= target)"] = {"rows_at": orc}
        # calibration of the predicted count at the step where each S1 rule first predicts < 1 (recall target 0.9 at k = 10)
        cal = {}
        for key in ("M_G0", "M_G", "M_Q2", "M_GPD", "M_EXP"):
            ms, xt, mi, npool, nz2, nz3, zmin = [], [], [], [], [], [], []
            for steps in per_q:
                for t in steps:
                    if key in t and 0.3 <= t[key] <= 3.0:
                        ms.append(t[key]); xt.append(t["X_true"]); mi.append(t["miss"])
                e = next((t for t in steps if key in t and t[key] < 1.0), None)
                if e is not None:
                    npool.append(e["n_pool"]); nz2.append(e["n_z2"]); nz3.append(e["n_z3"]); zmin.append(e["zeta_min"])
            cal[key] = {"steps_with_pred_0.3_to_3": len(ms), "mean_pred": float(np.mean(ms)) if ms else None, "mean_rows_above_running_kth_in_40_nearest_unread": float(np.mean(xt)) if xt else None,
                        "mean_true_misses": float(np.mean(mi)) if mi else None,
                        "at_stop_r0.9": {"queries": len(npool), "pooled_rows_p10_p50_p90": [float(x) for x in np.quantile(npool, [0.1, 0.5, 0.9])] if npool else None,
                                         "pooled_z_above_2_p10_p50_p90": [float(x) for x in np.quantile(nz2, [0.1, 0.5, 0.9])] if nz2 else None,
                                         "pooled_z_above_3_p10_p50_p90": [float(x) for x in np.quantile(nz3, [0.1, 0.5, 0.9])] if nz3 else None,
                                         "zeta_of_nearest_unread_p10_p50_p90": [float(x) for x in np.quantile(zmin, [0.1, 0.5, 0.9])] if zmin else None}}
        outset["calibration"] = cal
        res["sets"][st] = outset
        for name in outset["rules"]:
            log(st, name, {k: (None if v is None else round(v)) for k, v in outset["rules"][name]["rows_at"].items()})
        json.dump(res, open(out, "w"), indent=1)
    log("done")


if __name__ == "__main__":
    main()
