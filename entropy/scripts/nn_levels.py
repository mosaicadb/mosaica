# Prediction from the nearest stored neighbour: energy reduction, near-duplicate share, Gaussian rate saving at matched distortion.
# usage: nn.py <dataset>
import numpy as np, json, sys, math
sys.path.insert(0, __import__("os").path.dirname(__file__))
from common import *
name = sys.argv[1]; rng = np.random.default_rng(2)
X = load(name); N, n = X.shape; NT = min(20000, N // 5)
perm = rng.permutation(N); it = perm[:NT]; ib = perm[NT:]; NB = len(ib)      # base order is random: prefixes are random subsamples
K = max(8, round(N / 1000)); G = 16 if K >= 150 else 8
Xb = X[ib]; Xt = X[it]; del X
M, ab = kmeans(Xb, K); at = assign(Xt, M); Mg, gc = kmeans(M, G, iters=30, seed=3)
log(name, "N", N, "n", n, "base", NB, "test", NT, "K", K, "G", G)
levels = [NB // 16, NB // 4, NB]; nn = np.zeros((4, NT), np.int64); cs = np.zeros((4, NT), np.float32)
for s in range(0, NT, 400):
    S = Xt[s:s + 400] @ Xb.T
    for j, L in enumerate(levels): nn[j, s:s + 400] = S[:, :L].argmax(1); cs[j, s:s + 400] = S[np.arange(len(S)), nn[j, s:s + 400]]
    S[at[s:s + 400, None] != ab[None, :]] = -2; nn[3, s:s + 400] = S.argmax(1); cs[3, s:s + 400] = S[np.arange(len(S)), nn[3, s:s + 400]]
log("neighbours found")
Rt = Xt - M[at]; e0 = (Rt * Rt).sum(1); res = {"N": int(N), "n": int(n), "NB": int(NB), "variants": {}}
names = [f"global N={L}" for L in levels] + ["in-cluster"]
for j in range(4):
    d2 = np.maximum(2 - 2 * cs[j].astype(np.float64), 0); ok = cs[j] > -1.5
    row = dict(mean_ratio=float(d2[ok].mean() / e0[ok].mean()), median_item_ratio=float(np.median(d2[ok] / e0[ok])), share_worse=float((d2[ok] > e0[ok]).mean()),
               cos999=float((cs[j] > 0.999).mean()), cos99=float((cs[j] > 0.99).mean()), cos95=float((cs[j] > 0.95).mean()), exact=float((d2 < 1e-5).mean()), mean_cos=float(cs[j][ok].mean()))
    res["variants"][names[j]] = row; log(names[j], {k: round(v, 4) for k, v in row.items()})
log("residual energy to cluster mean", float(e0.mean()))
# coordinates in the item's group basis (lambda order), whitened by the base variances
gt = gc[at]; lam = np.zeros((G, n)); U = np.zeros((NT, n), np.float32); UY = np.zeros((4, NT, n), np.float32)
for g in range(G):
    ibg = np.where(gc[ab] == g)[0]; itg = np.where(gt == g)[0]
    if len(ibg) < 50 or len(itg) == 0: continue
    RB = Xb[ibg] - M[ab[ibg]]; l, V = np.linalg.eigh((RB.T @ RB).astype(np.float64) / len(ibg)); l = np.maximum(l[::-1], 1e-12); V = V[:, ::-1].astype(np.float32); lam[g] = l
    sl = np.sqrt(l).astype(np.float32); U[itg] = (Rt[itg] @ V) / sl
    for v, j in enumerate((0, 1, 2, 3)): UY[v, itg] = ((Xb[nn[j, itg]] - M[at[itg]]) @ V) / sl
    del RB, V
del Xb
fit = np.arange(NT) < NT // 2; ev = ~fit
def rate(var, theta): return 0.5 * np.log2(np.maximum(var / theta, 1)).sum(-1)
def theta_at(bits, var_items):
    lo, hi = 1e-30, var_items.max()
    for _ in range(100):
        th = math.sqrt(lo * hi)
        if rate(var_items, th).mean() > bits: lo = th
        else: hi = th
    return th
gvar = np.zeros((G, n)); valid = np.zeros(NT, bool)   # baseline: per (group, dim) variance on the eval half, cluster-mean code
for g in range(G):                                     # re-whiten by the eval-half variance, so that pooled profiles are exact for the baseline; drop groups too small to estimate it
    sel = ev & (gt == g)
    if sel.sum() < 100: continue
    f = np.maximum((U[sel].astype(np.float64) ** 2).mean(0), 1e-20); gvar[g] = f * lam[g]; sf = np.sqrt(f).astype(np.float32)
    U[gt == g] /= sf; UY[:, gt == g] /= sf; valid[gt == g] = True
log("items in groups large enough to evaluate:", float(valid.mean()))
fit &= valid; ev &= valid; lam = gvar
budgets = [n / 3, n, 2 * n, 4 * n]; thetas = [theta_at(b, gvar[gt[ev]]) for b in budgets]
for v, j in enumerate((0, 1, 2, 3)):
    d2 = np.maximum(2 - 2 * cs[j].astype(np.float64), 0); ptr = math.log2(levels[j]) if j < 3 else math.log2(NB / K)
    rest = d2[fit & (d2 >= 0.02)]; edges = np.r_[0, 0.002, 0.02, np.quantile(rest, np.linspace(0, 1, 7)[1:-1]), 4.01]; nb = len(edges) - 1
    b = np.clip(np.searchsorted(edges, d2, side="right") - 1, 0, nb - 1); uy = UY[v]
    out = {"pointer_bits": ptr, "edges": edges.tolist(), "bins": []}
    acc = {k: np.zeros(len(budgets)) for k in ("exact", "lossy", "binonly")}; base_rate = np.zeros(len(budgets))
    for bi in range(nb):
        f = fit & (b == bi); e = ev & (b == bi); share = float(e.sum() / ev.sum())
        if f.sum() < 30 or e.sum() < 30:
            out["bins"].append(dict(share=share, skipped=True)); base_rate += share * np.array([rate(gvar[gt[e]], th).mean() if e.any() else 0 for th in thetas]); continue
        beta = (U[f].astype(np.float64) * uy[f]).sum(0) / np.maximum((uy[f].astype(np.float64) ** 2).sum(0), 1e-12)
        a0 = (U[e].astype(np.float64) ** 2).mean(0); ad = ((U[e] - beta * uy[e]).astype(np.float64) ** 2).mean(0); a1 = ((U[e] - uy[e]).astype(np.float64) ** 2).mean(0)
        lg = lam[gt[e]]; rec = dict(share=share, mean_cos=float(1 - d2[e].mean() / 2), energy_ratio_delta=float((a1 * lg).sum() / (a0 * lg).sum()), energy_ratio_regress=float((ad * lg).sum() / (a0 * lg).sum()), mean_beta_top64=float(beta[:64].mean()), saving_exact=[], saving_lossy=[])
        for t, th in enumerate(thetas):
            r0 = rate(a0 * lg, th).mean(); rg = rate(gvar[gt[e]], th).mean()
            rx = rate(ad * lg, th).mean() + ptr; rl = rate(ad * lg + beta ** 2 * np.minimum(lg, th), th).mean() + ptr
            base_rate[t] += share * rg; acc['binonly'][t] += share * (rg - r0)
            # rate with the bin index known (log2 nb bits charged below), best of cluster-mean code and neighbour code
            rec["saving_exact"].append(float(rg - min(r0, rx))); rec["saving_lossy"].append(float(rg - min(r0, rl)))
            acc["exact"][t] += share * (rg - min(r0, rx)); acc["lossy"][t] += share * (rg - min(r0, rl))
        out["bins"].append(rec)
    out["budgets_bits"] = budgets; out["baseline_rate"] = base_rate.tolist()
    out["net_saving_exact"] = (acc["exact"] - math.log2(nb)).tolist(); out["net_saving_lossy"] = (acc["lossy"] - math.log2(nb)).tolist()
    out["bin_only_saving"] = (acc["binonly"] - math.log2(nb)).tolist(); res["rd_" + names[j]] = out
    log(f"--- {names[j]}: pointer {ptr:.1f} bits, {nb} bins ({math.log2(nb):.1f} bits to name the bin); budgets (bits) {[round(x) for x in budgets]}")
    for bi, r in enumerate(out["bins"]):
        if "skipped" in r: log(f"  bin {bi} share {r['share']:.4f} skipped"); continue
        log(f"  bin {bi} d2<{edges[bi + 1]:.3f} share {r['share']:.4f} cos {r['mean_cos']:.4f} energy delta/regress {r['energy_ratio_delta']:.3f}/{r['energy_ratio_regress']:.3f} beta {r['mean_beta_top64']:.2f} | saving exact-ref {np.round(r['saving_exact'], 1).tolist()} lossy-ref {np.round(r['saving_lossy'], 1).tolist()}")
    log("  saving from knowing the bin alone (no neighbour, bin index charged):", np.round(out["bin_only_saving"], 1).tolist())
    log("  net saving per item, exact reference:", np.round(out["net_saving_exact"], 1).tolist(), " reference at the same distortion:", np.round(out["net_saving_lossy"], 1).tolist())
json.dump(res, open(BASE + f"entropy/logs/nnlev_{name}.json", "w")); log("done")
