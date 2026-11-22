# post.py OUT: from study_w_principled's OUT.post and OUT.rows (DIAG_POST), per index-path cluster: the error's
# calibration (actual sd of e0 over W2's tau0), its regression on the row's deviation d = s - mu (c = cov(e0, d) /
# var(d), against MMSE's (tau/sigma_act)^2 and full shrinkage's tau/sigma_act), and corr(e0, est0); then, over the rows
# near the final k-th best, the share at or above it by x0 = (t_fin - est0) / tau0 (corrected) and by x0b (as built),
# split by the cluster's zeta_fin = (t_fin - mu) / sigma.
import sys
import numpy as np

out = sys.argv[1]
P = np.genfromtxt(out + ".post", names=True, dtype=None, encoding=None)
sig_act = P["sd_d"]
c = P["cov_e0_d"] / sig_act ** 2
mmse = (P["sd_e0"] / sig_act) ** 2
full = P["sd_e0"] / sig_act
corr_e_est = P["cov_e0_est0"] / (P["sd_e0"] * np.sqrt(np.maximum(sig_act ** 2 + P["sd_e0"] ** 2 - 2 * P["cov_e0_d"], 1e-300)))
q = lambda v: "/".join(f"{np.nanquantile(v, p):.3f}" for p in (0.1, 0.5, 0.9))
hold = P["above"] > 0
print(f"{out}: {len(P)} index-path clusters ({hold.sum()} holding a row at or above the final k-th best); p10/median/p90")
for name, m in (("all", np.ones(len(P), bool)), ("holding", hold)):
    print(f"  {name}: sd(e0)/tau0 {q(P['sd_e0'][m] / P['tau0'][m])}  sd(e0)/built0 {q(P['sd_e0'][m] / P['built0'][m])}  sigma_act/sigma_model {q(sig_act[m] / P['sigma'][m])}")
    print(f"     c = cov(e0,d)/var(d) {q(c[m])}; MMSE (sd_e/sd_d)^2 {q(mmse[m])}; full shrinkage sd_e/sd_d {q(full[m])}; c/MMSE {q(c[m] / mmse[m])}; corr(e0, est0) {q(corr_e_est[m])}")
    print(f"     mean e0 / tau0 {q(P['mean_e0'][m] / P['tau0'][m])}; built0/tau0 {q(P['built0'][m] / P['tau0'][m])}; tau0/sigma {q(P['tau0'][m] / P['sigma'][m])}")
R = np.genfromtxt(out + ".rows", names=True, dtype=None, encoding=None, usecols=(3, 4, 5, 6, 7, 8))
key = {(a, b): i for i, (a, b) in enumerate(zip(P["qi"], P["ci"]))}
ix = np.array([key[(a, b)] for a, b in zip(R["qi"], R["ci"])])
zf = ((P["t_fin"] - P["mu"]) / P["sigma"])[ix]
edges = [-np.inf, 0, 1, 2, 3, 4, 5, 6, 8, 12]
for col in ("x0", "x0b"):
    print(f"  share of rows at or above the final k-th best by {col} (rows in bin): zeta_fin < 3 | 3 to 4.5 | >= 4.5")
    for lo, hi in zip(edges[:-1], edges[1:]):
        cells = []
        for zlo, zhi in ((-np.inf, 3), (3, 4.5), (4.5, np.inf)):
            m = (R[col] >= lo) & (R[col] < hi) & (zf >= zlo) & (zf < zhi)
            cells.append(f"{R['above'][m].mean() if m.sum() else float('nan'):.2e} ({m.sum()})")
        print(f"    [{lo:g}, {hi:g}): " + " | ".join(cells))
A = R["above"] == 1
print(f"  rows above: {A.sum()}; their x0 p50/p90/max {np.quantile(R['x0'][A], .5):.2f}/{np.quantile(R['x0'][A], .9):.2f}/{R['x0'][A].max():.2f}; x0b {np.quantile(R['x0b'][A], .5):.2f}/{np.quantile(R['x0b'][A], .9):.2f}/{R['x0b'][A].max():.2f}; h {np.quantile(R['h'][A], .5):.2f}")
