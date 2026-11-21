"""A4 (third round), synthetic only: a neighbour's error under two models, set against the visit-order session's figures.

Model C (a common factor: the neighbour resembles the query). Per row of a cluster with mean mu:
    s - mu = R + aG + eG,   a - mu = (1 - kappa) R + aG,   e = kappa R + eG,
R >= 0 exponential with scale sigma/lam (what the row shares with the query's offset; it gives the scores an
exponential upper tail of rate lam a spread), aG and eG independent normals (a typical residual's coded and uncoded
parts). kappa is the share of the offset's own weight the level leaves out.
Model I (independent heavy parts): the same with two independent exponentials, Ra in a and Re in e, of one scale.

For five settings matched to the measured (f, median r) it prints, among the true ten of 30 clusters of 2,000 rows:
the quantiles of r = e/(s - mu), e/tau, the share kept by the plain rule and by psi_eff, the rows passed at a kept
share of 0.9 for several kappa in  a + psi0 sqrt(f) tau + kappa max(t - mu, 0) >= t,  and the slope of s on a among
the rows with the highest a.

usage: run-limited.sh 6 1 2 python3 a4_resemble.py
"""
import math
import numpy as np
from scipy.stats import norm

NQ, NC, NR, K = 200, 30, 2000, 10
LAM = 2.2


def run(name, f, kap, model, rng):
    b = 1.0 / LAM                                    # sigma = 1
    if model == "C":
        vG = 1 - b * b                               # variance of the normal part of the score
        one_m_fG = ((1 - f) - kap * kap * b * b) / vG
    else:
        # independent exponentials Ra and Re of scale b: Var(s) = 2 b^2 + vG, Var(e) = b^2 + (1 - fG) vG
        b = 1.0 / LAM / math.sqrt(1.0)               # the tail rate of s is still 1/b
        vG = 1 - 2 * b * b
        one_m_fG = ((1 - f) - b * b) / vG
    if not (0 < one_m_fG < 1):
        print(f"   {name}: model {model} cannot give f = {f} (the normal part's uncoded share would be {one_m_fG:.3f})")
        return
    sa, se = math.sqrt((1 - one_m_fG) * vG), math.sqrt(one_m_fG * vG)
    tau = math.sqrt(1 - f)
    mus = -np.linspace(0.0, 4.0, NC)
    rules = {"0": 0.0, "0.1": 0.1, "0.2": 0.2, "0.3": 0.3, "0.5": 0.5, "1-f": 1 - f, "sqrt(1-f)": math.sqrt(1 - f), "kappa_q": kap}
    need_nb = {k: [] for k in rules}; need_all = {k: [] for k in rules}
    rr, et, zz, slope_top, slope_all = [], [], [], [], []
    plain4 = eff4 = eff3 = tot = 0
    for q in range(NQ):
        if model == "C":
            R = rng.exponential(b, (NC, NR)); Ra, Re = (1 - kap) * R, kap * R
        else:
            Ra, Re = rng.exponential(b, (NC, NR)), rng.exponential(b, (NC, NR))
        aG = rng.normal(0, sa, (NC, NR)); eG = rng.normal(0, se, (NC, NR))
        adev = Ra + aG - Ra.mean(); e = Re + eG - Re.mean()            # centred, as a midpoint and a cluster mean are
        s = mus[:, None] + adev + e
        a = mus[:, None] + adev
        t = np.partition(s.ravel(), -K)[-K]
        nb = s >= t
        T = np.maximum(t - mus, 0)[:, None] * np.ones((1, NR))
        dev = (s - mus[:, None])
        up = nb & (dev > 0)
        rr.extend((e[up] / dev[up]).tolist()); et.extend((e[nb] / tau).tolist()); zz.extend(dev[nb].tolist())
        plain4 += int((a[nb] + 4 * tau >= t).sum()); tot += int(nb.sum())
        eff4 += int((a[nb] + 4 * math.sqrt(f) * tau + (1 - f) * T[nb] >= t).sum())
        eff3 += int((a[nb] + 3 * math.sqrt(f) * tau + (1 - f) * T[nb] >= t).sum())
        for k_, kv in rules.items():
            need = (t - a - kv * T) / (math.sqrt(f) * tau)
            need_nb[k_].append(need[nb]); need_all[k_].append(need[need < 12].astype(np.float32))
        c0 = slice(0, 6)                                              # the six nearest clusters
        x = (a[c0] - mus[c0, None]).ravel(); y = (s[c0] - mus[c0, None]).ravel()
        hi = x > np.quantile(x, 0.995)
        slope_top.append(np.polyfit(x[hi], y[hi], 1)[0]); slope_all.append(np.polyfit(x, y, 1)[0])
    rr = np.array(rr); et = np.array(et)
    qs = np.quantile(rr, [0.1, 0.25, 0.5, 0.75, 0.9])
    line = (f"   {name:18s} model {model} f {f:.2f} kappa {kap:.2f}: neighbours at zeta median {np.median(zz):.1f}; r p10/25/50/75/90 " + "/".join(f"{x:.2f}" for x in qs) +
            f"; r/(1-f) median {qs[2] / (1 - f):.1f}; e/tau median {np.median(et):.1f}, beyond 4 tau {np.mean(et > 4):.2f}; kept: plain psi 4 {plain4 / tot:.3f}, psi_eff psi0 4 {eff4 / tot:.3f}, psi0 3 {eff3 / tot:.3f};"
            f" slope of s on a, all rows {np.mean(slope_all):.2f}, top 0.5% by a {np.mean(slope_top):.2f} (1/(1-kappa) = {1 / (1 - kap):.2f})")
    print(line)
    rows = {}
    for k_ in rules:
        nbn = np.concatenate(need_nb[k_]); p0 = np.quantile(nbn, 0.9)
        rows[k_] = (p0, sum(int((x <= p0).sum()) for x in need_all[k_]) / NQ, float(np.mean(nbn <= 2.0)))
    best = min(v[1] for v in rows.values())
    print("        rows passed a query at kept 0.9 (psi0 needed; kept at psi0 = 2):  " + ";  ".join(f"kappa {k_}: {v[1]:.0f} ({v[0]:.2f}; {v[2]:.2f})" + (" *" if v[1] == best else "") for k_, v in rows.items()), flush=True)


if __name__ == "__main__":
    rng = np.random.default_rng(21)
    print("measured by the visit-order session, for comparison: median r 0.33 / 0.47 / 0.36 / 0.46 / 0.55; e/tau median about 3, 0.24 to 0.35 beyond 4 tau;")
    print("   kept by the plain rule at psi 4: 0.818 / 0.824 / 0.803 / 0.809 / 0.790; by psi_eff at psi0 4: 0.898 / 0.918 / 0.934 / 0.960 / 0.951; at psi0 3: 0.77 to 0.90\n")
    sets = (("iNaturalist, 12 groups", 0.92, 0.36), ("iNaturalist, 1 group", 0.86, 0.50), ("yahoo", 0.82, 0.39), ("LAION", 0.71, 0.50), ("LAION, 8 groups", 0.67, 0.58))
    print("Model C, a common factor (kappa set a little above the measured median r):")
    for name, f, kap in sets:
        run(name, f, kap, "C", rng)
    print("\nModel I, independent heavy parts in the estimate and in the error:")
    for name, f, kap in sets:
        run(name, f, kap, "I", rng)
