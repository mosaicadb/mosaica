# Step 6. The hashing family at its most favourable: idealised cap filters (locality-sensitive filtering), tuned per query.
# B Gaussian directions g_i. An item is stored under g_i when g_i.x/|x| >= a; a query probes g_i when g_i.q >= b.
# For a pair with cosine c the two events are the upper orthant of a bivariate normal with correlation c, whatever the
# dimension, so the expected work follows from the exact scores alone:
#   B = ln(k/eps) / P_c*(a, b)   (every match found with probability 1 - eps/k; c* = the smallest cosine a match can have)
#   work = B P[Z >= b] bucket probes + sum over items of 1 - exp(-B P_c(a, b)) distinct candidates, each then verified
#   space = B P[Z >= a] entries per item.
# (a, b) minimise the work under the space budget, separately for each query, N and answer size (no real index can
# re-tune per query: this is a lower bound for the family). "raw" hashes x; "centred" hashes x - mu in four norm buckets.
# A first run capped a and b at 6 (lsf_grid6.log); this grid goes to 12.
# Nothing here is a bound on a score, so there is nothing to violate: the counts are expectations, not a run.
import numpy as np, json, os
from scipy.stats import norm
from common import *
Q, qtype, qkind, qi = queries()
rank = np.load(os.path.join(OUT, "rank.npy")); xmu = np.load(os.path.join(OUT, "xmu.npy")); mu = np.load(os.path.join(OUT, "mu.npy"))
xc = np.sqrt(np.maximum(1e-12, 1 - 2 * xmu + float(mu @ mu))); QMU = Q @ mu
AG = np.arange(0.5, 12.01, 0.5); BG = np.arange(0.0, 12.01, 0.5); NC = 800; CB = np.linspace(-1, 1, NC + 1); CC = np.clip((CB[:-1] + CB[1:]) / 2, -0.9995, 0.9995)
def orth(a, b, c):
    u = a + np.linspace(0, 9, 361); w = np.full(len(u), u[1] - u[0]); w[0] /= 2; w[-1] /= 2
    c = np.clip(np.atleast_1d(c), -0.9995, 0.9995)
    return (norm.sf((b - c[:, None] * u[None, :]) / np.sqrt(1 - c[:, None] ** 2)) * (norm.pdf(u) * w)[None, :]).sum(1)
T = np.empty((len(AG), len(BG), NC))
for i, a in enumerate(AG):
    for j, b in enumerate(BG): T[i, j] = orth(a, b, CC)
log("orthant table done")
PA = norm.sf(AG)[:, None]; PB = norm.sf(BG)[None, :]
EPS = 1e-9; BUD = [16, 256, 4096]; KK = [10, 1000]
edges = np.quantile(xc, [0.25, 0.5, 0.75]); bucket = np.searchsorted(edges, xc); bmax = np.array([xc[bucket == j].max() for j in range(4)])
def best(hist, cstar, Lf):
    """Minimum expected work for each space budget; hist over the correlation bins; cstar the smallest match correlation."""
    if cstar <= 0: return [float(hist.sum())] * len(BUD), [(np.nan, np.nan)] * len(BUD)
    k0 = min(NC - 1, max(0, int(np.searchsorted(CB, cstar, side="right") - 1)))        # bin holding c*: its centre may exceed c*
    lo = max(k0 - 1, 0); Pst = np.minimum(T[:, :, k0], T[:, :, lo]) if CC[k0] > cstar else T[:, :, k0]   # conservative: a lower correlation
    B = Lf / np.maximum(Pst, 1e-300)
    nz = np.flatnonzero(hist); work = B * PB + (hist[nz][None, None, :] * -np.expm1(-B[:, :, None] * T[:, :, nz])).sum(-1)
    space = B * PA; res = []; arg = []
    for m in BUD:
        w = np.where(space <= m, work, np.inf); i = np.unravel_index(np.argmin(w), w.shape)
        res.append(float(min(w[i], hist.sum()))); arg.append((float(AG[i[0]]), float(BG[i[1]])))
    return res, arg
out = []
for q in range(0, NQ, 2):
    wait_lock(); s = score_row(q); rec = {"q": q, "type": qtype[q]}
    braw = np.clip(np.searchsorted(CB, s, side="right") - 1, 0, NC - 1)
    cen = (s - QMU[q]) / xc; bcen = np.clip(np.searchsorted(CB, cen, side="right") - 1, 0, NC - 1) + NC * bucket
    for sz in SIZES:
        m = rank < sz; ss = np.sort(s[m]); hr = np.bincount(braw[m], minlength=NC).astype(float); hc = np.bincount(bcen[m], minlength=4 * NC).astype(float).reshape(4, NC)
        for k in KK:
            t = float(ss[len(ss) - k]); Lf = np.log(k / EPS)
            r, arg = best(hr, t, Lf); c = np.zeros(len(BUD))
            for j in range(4): c += np.array(best(hc[j], (t - QMU[q]) / bmax[j], Lf)[0])
            rec[f"{sz}_{k}"] = {"raw": r, "cen": c.tolist(), "arg": arg}
    out.append(rec)
    if q % 40 == 0: log("query", q)
json.dump(out, open(os.path.join(OUT, "lsf.json"), "w"))
x = np.log(SIZES)
for typ in ("text", "item"):
    R = [r for r in out if r["type"] == typ]
    for k in KK:
        print(f"\n{typ} queries ({len(R)}), k = {k}, eps = {EPS}: expected probes + candidates as a multiple of N, by N; exponent of the work in N")
        for mode in ("raw", "cen"):
            for bi, m in enumerate(BUD):
                A = np.array([[r[f"{sz}_{k}"][mode][bi] for sz in SIZES] for r in R]); mean = A.mean(0)
                ab = np.nanmean(np.array([r[f"{SIZES[-1]}_{k}"]["arg"][bi] for r in R]), 0) if mode == "raw" else (np.nan, np.nan)
                print(f"  {mode:4s} {m:5d} entries/item: " + "  ".join(f"{v/sz:8.4f}" for v, sz in zip(mean, SIZES)) + f"   exponent {np.polyfit(x, np.log(mean), 1)[0]:.3f}   queries with no gain over a scan at 1M: {100*(A[:, -1] >= 0.999*SIZES[-1]).mean():.0f}%   mean (a, b) at 1M: {ab[0]:.1f}, {ab[1]:.1f}")
