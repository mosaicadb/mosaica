"""Referee 2, A4.7: the identity for a sample read at a randomised level, on simulated Haar rotations.
Two blocks of 48 coordinates (d_j = 47), one-bit sign codes, the estimator of Lemma 3.1 of chapter 3, one rotation a
block shared by every row (as the index does). 64 rows; s = base + Delta.y; the deterministic level's midpoint is
a = s - e with e fixed per row. Over 4,000 seeds: E[a_R] - s, E[(a_R - a)^2] - v_R against e^2, E[(a_R - a) a_R] - v_R
against e s; then the sample's estimates of tau^2 and sigma^2 with (i) v_R, (ii) sd_R^2 (||Delta_j|| for the
projected norm), (iii) v_R from the level's own estimate of Delta.y, (iv) one block left uncoded, (v) the estimator's
scale rounded up by 3%; and the correlation over seeds of two rows' errors. Synthetic."""
import numpy as np
rng = np.random.default_rng(2)
NB, NJ, M, SEEDS = 2, 48, 64, 4000
delta = rng.normal(size=(NB, NJ)); delta /= np.linalg.norm(delta)
common = rng.normal(size=(NB, NJ))
y = 0.22 * rng.normal(size=(M, 1, 1)) * (0.7 * delta / np.linalg.norm(delta, axis=1, keepdims=True) + 0.3 * common / np.linalg.norm(common, axis=1, keepdims=True)) \
    + rng.normal(size=(M, NB, NJ)) / np.sqrt(NJ)
y[1] = y[0] + 0.02 * rng.normal(size=(NB, NJ)) / np.sqrt(NJ)          # row 1 is a near-duplicate of row 0
base = rng.normal(size=M) * 0.5
e_det = rng.normal(size=M) * 0.05                                  # the part of the 16 B level's error outside y
blk = np.einsum('bj,mbj->mb', delta, y)                            # Delta_j . y_j
s = base + blk.sum(1); a = base - e_det; e = s - a
ny = np.linalg.norm(y, axis=2); nd = np.linalg.norm(delta, axis=1)
proj2 = nd[None, :] ** 2 - (blk / ny) ** 2                         # ||P_{y perp} Delta_j||^2

def haar(n):
    q, r = np.linalg.qr(rng.normal(size=(n, n))); return q * np.sign(np.diag(r))

acc = {k: np.zeros(M) for k in ("aR", "sq", "cross", "sq_sd", "sq_est", "sq_unc", "aR_unc", "sq_round", "aR_round")}
est = {k: [] for k in ("tau2", "sig2", "tau2_sd", "sig2_sd", "tau2_est", "tau2_unc", "sig2_unc", "tau2_round")}
errs = np.zeros((SEEDS, M))
for it in range(SEEDS):
    yhat = np.zeros_like(y); A2 = np.zeros((M, NB))
    for b in range(NB):
        Rm = haar(NJ); z = y[:, b, :] @ Rm.T; o = np.sign(z)
        ip = (o * z).sum(1)                                        # <o, Ry>
        yhat[:, b, :] = (ny[:, b] ** 2 / ip)[:, None] * (o @ Rm)
        cos = ip / (np.sqrt(NJ) * ny[:, b]); A2[:, b] = ny[:, b] ** 2 * (1 / cos ** 2 - 1)
    est_blk = np.einsum('bj,mbj->mb', delta, yhat)
    aR = base + est_blk.sum(1)
    vR = (A2 * proj2).sum(1) / (NJ - 1); sd2 = (A2 * nd[None, :] ** 2).sum(1) / (NJ - 1)
    vE = (A2 * np.maximum(nd[None, :] ** 2 - (est_blk / ny) ** 2, 0)).sum(1) / (NJ - 1)
    errs[it] = aR - s
    acc["aR"] += aR; acc["sq"] += (aR - a) ** 2 - vR; acc["cross"] += (aR - a) * aR - vR
    acc["sq_sd"] += (aR - a) ** 2 - sd2; acc["sq_est"] += (aR - a) ** 2 - vE
    est["tau2"].append(np.var(aR - a, ddof=1) - vR.mean()); est["sig2"].append(np.var(aR, ddof=1) - vR.mean())
    est["tau2_sd"].append(np.var(aR - a, ddof=1) - sd2.mean()); est["sig2_sd"].append(np.var(aR, ddof=1) - sd2.mean())
    est["tau2_est"].append(np.var(aR - a, ddof=1) - vE.mean())
    aU = base + est_blk[:, 0]; vU = A2[:, 0] * proj2[:, 0] / (NJ - 1)       # block 1 left uncoded
    acc["aR_unc"] += aU; acc["sq_unc"] += (aU - a) ** 2 - vU
    est["tau2_unc"].append(np.var(aU - a, ddof=1) - vU.mean()); est["sig2_unc"].append(np.var(aU, ddof=1) - vU.mean())
    aQ = base + 1.03 * est_blk.sum(1)                                       # the estimator's scale rounded up by 3%
    acc["aR_round"] += aQ; acc["sq_round"] += (aQ - a) ** 2 - vR
    est["tau2_round"].append(np.var(aQ - a, ddof=1) - vR.mean())
for k in acc: acc[k] /= SEEDS
tau2, sig2 = e.var(ddof=1), s.var(ddof=1)
vbar = errs.var(0).mean()
print(f"tau^2 {tau2:.4f}, sigma^2 {sig2:.4f}, mean error variance of the randomised level {vbar:.4f} (v_R/tau^2 = {vbar / tau2:.2f}); cos of Delta_j with y_j, rms {np.sqrt(((blk / ny / nd) ** 2).mean()):.2f}")
se = errs.std(0).mean() / np.sqrt(SEEDS)
print(f"per row, over seeds: max |E a_R - s| {np.abs(acc['aR'] - s).max():.4f} (one standard error {se:.4f});  max |E[(a_R - a)^2 - v_R] - e^2| {np.abs(acc['sq'] - e ** 2).max():.4f};  max |E[(a_R - a) a_R - v_R] - e s| {np.abs(acc['cross'] - e * s).max():.4f}")
c = np.corrcoef(errs.T); off = c[np.triu_indices(M, 1)]
cy = np.array([[(y[i] * y[j]).sum() / np.linalg.norm(y[i]) / np.linalg.norm(y[j]) for j in range(M)] for i in range(M)])[np.triu_indices(M, 1)]
print(f"correlation over seeds of two rows' errors: near-duplicate pair {c[0, 1]:+.2f}; other pairs mean {off[1:].mean():+.3f}, rms {np.sqrt((off[1:] ** 2).mean()):.3f}; its correlation with the cosine of the two rows' residuals {np.corrcoef(off, cy)[0, 1]:+.2f} (rows' cosines rms {np.sqrt((cy ** 2).mean()):.2f})")
f = lambda k, ref: f"{np.mean(est[k]) / ref:.3f}"
print("sample estimates over the truth (mean over seeds):")
print(f"  with v_R:                       tau^2 {f('tau2', tau2)}   sigma^2 {f('sig2', sig2)}   (sd over seeds of the tau^2 estimate {np.std(est['tau2']) / tau2:.3f})")
print(f"  with sd_R^2 (upper bound):      tau^2 {f('tau2_sd', tau2)}   sigma^2 {f('sig2_sd', sig2)}")
print(f"  with v_R from the level's own estimate of Delta_j.y_j:  tau^2 {f('tau2_est', tau2)}")
print(f"  one block uncoded:              tau^2 {f('tau2_unc', tau2)}   sigma^2 {f('sig2_unc', sig2)}   mean of a_R - s {np.mean(acc['aR_unc'] - s):+.4f} (share of tau^2 in that block {blk[:, 1].var(ddof=1) / tau2:.2f})")
print(f"  estimator's scale up by 3%:     tau^2 {f('tau2_round', tau2)}   max |E a_R - s| {np.abs(acc['aR_round'] - s).max():.4f}")
