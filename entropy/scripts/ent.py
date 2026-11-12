# Negentropy of residuals against the scheme's Gaussian model, as held-out cross-entropy differences under explicit models.
# usage: ent.py <dataset> [--K k] [--G g] [--control] [--tag t] [--noq]
import numpy as np, json, sys, math, argparse
from scipy.special import ndtr, gammaln, stdtr, logsumexp
sys.path.insert(0, __import__("os").path.dirname(__file__))
from common import *
ap = argparse.ArgumentParser(); ap.add_argument("name"); ap.add_argument("--K", type=int, default=0); ap.add_argument("--G", type=int, default=0)
ap.add_argument("--control", action="store_true"); ap.add_argument("--tag", default=""); ap.add_argument("--noq", action="store_true"); ap.add_argument("--rdonly", action="store_true")
ar = ap.parse_args()
rng = np.random.default_rng(1)
X = load(ar.name); N, n = X.shape
K = ar.K or max(8, round(N / 1000)); G = ar.G or (16 if K >= 150 else 8)
KD = min(n, 1024); KS = [k for k in (16, 32, 64, 128, 256, 512, 768, 1024) if k <= KD]
if KD not in KS: KS.append(KD)
log(ar.name, "N", N, "n", n, "K", K, "G", G, "KD", KD, "control", ar.control)
perm = rng.permutation(N); nA = int(0.8 * N); nB = (N - nA) // 2
split = np.zeros(N, np.int8); split[perm[nA:nA + nB]] = 1; split[perm[nA + nB:]] = 2
M, aA = kmeans(X[split == 0], K); a = assign(X, M)
Mg, gc = kmeans(M, G, iters=30, seed=3); grp = gc[a]
log("kmeans done; cluster sizes min/med/max", np.bincount(a, minlength=K).min(), int(np.median(np.bincount(a, minlength=K))), np.bincount(a, minlength=K).max(),
    "group sizes", np.bincount(grp, minlength=G).tolist())
qs = X[rng.choice(np.where(split == 0)[0], 200, replace=False)]
hold = np.where(split > 0)[0]; pos = -np.ones(N, np.int64); pos[hold] = np.arange(len(hold))
Z = np.zeros((len(hold), KD), np.float32)            # held-out coordinates, gamma order, top KD
Qd = np.zeros((G, KD)); lamA = np.zeros((G, KD)); vc = np.ones((K, KD), np.float32); sc = np.ones(K)
varfull = np.zeros((G, 2, n)); gamfull = np.zeros((G, n)); cnt = np.zeros((G, 2)); res_energy = 0.0
for g in range(G):
    cl = np.where(gc == g)[0]
    iA = np.where((grp == g) & (split == 0))[0]; iH = np.where((grp == g) & (split > 0))[0]
    if len(iA) < 50: continue
    RA = X[iA] - M[a[iA]]; res_energy += float((RA * RA).sum())
    cov = (RA.T @ RA).astype(np.float64) / len(iA); lam, V = np.linalg.eigh(cov); lam = np.maximum(lam[::-1], 1e-12); V = V[:, ::-1].astype(np.float32)
    D = (qs[:, None, :] - M[cl][None]).reshape(-1, n) @ V; wts = np.tile(np.bincount(a[iA], minlength=K)[cl], len(qs)).astype(np.float64)
    Q = (D.astype(np.float64) ** 2 * wts[:, None]).sum(0) / wts.sum(); del D
    order = np.argsort(-Q * lam); top = order[:KD]
    ZA = RA @ V[:, top]; RH = X[iH] - M[a[iH]]; ZHf = RH @ V
    if ar.control:
        ZA = (rng.standard_normal(ZA.shape) * np.sqrt(lam[top])).astype(np.float32); ZHf = (rng.standard_normal(ZHf.shape) * np.sqrt(lam)).astype(np.float32)
    for s in (1, 2):
        sel = split[iH] == s; varfull[g, s - 1] = (ZHf[sel].astype(np.float64) ** 2).mean(0); cnt[g, s - 1] = sel.sum()
    gamfull[g] = Q * lam
    Z[pos[iH]] = ZHf[:, top]; Qd[g] = Q[top]; lamA[g] = lam[top]
    for c in cl:                                       # per-cluster variance profile and scalar scale from the training split
        sel = a[iA] == c; nc = sel.sum()
        if nc == 0: continue
        v = (ZA[sel].astype(np.float64) ** 2).mean(0) / lam[top]
        vc[c] = (nc * v + 50) / (nc + 50); sc[c] = math.sqrt((nc * v.mean() + 50) / (nc + 50))
    del RA, ZA, RH, ZHf, V, cov
log("bases done; mean residual energy per item", res_energy / nA)
del X
tag = ar.name + ("_K%d_G%d" % (K, G) if (ar.K or ar.G) else "") + ("_control" if ar.control else "") + ar.tag
np.savez(BASE + f"entropy/logs/spec_{tag}.npz", varfull=varfull, gamfull=gamfull, cnt=cnt, K=K, G=G, N=N, n=n)
if ar.rdonly: sys.exit()
hs = split[hold]; hg = grp[hold]; hc = a[hold]
LN2 = math.log(2)
bands = [(0, 16), (16, 64), (64, 256), (256, KD)]; bands = [(lo, min(hi, KD)) for lo, hi in bands if lo < KD]
def cumk(lp):                                          # per-item cumulative sums at KS (float64)
    starts = [0] + KS[:-1]; return np.cumsum(np.add.reduceat(lp.astype(np.float64), starts, axis=1), axis=1)
def group_var(Zs, fit):                                # per (group, dim) variance on the fit split
    tau = np.ones((G, KD))
    for g in range(G):
        sel = fit & (hg == g)
        if sel.sum() > 20: tau[g] = (Zs[sel].astype(np.float64) ** 2).mean(0)
    return np.maximum(tau, 1e-14)
def fit_t(u):                                          # pooled standardised values -> (nu, scale)
    best = None
    for nu in (2.5, 3, 4, 5, 6, 8, 12, 20, 40, 100):
        s2 = 1.0
        for _ in range(30): w = (nu + 1) / (nu + u * u / s2); s2 = float((w * u * u).mean())
        ll = float((gammaln((nu + 1) / 2) - gammaln(nu / 2) - 0.5 * math.log(nu * math.pi * s2) - (nu + 1) / 2 * np.log1p(u * u / (nu * s2))).mean())
        if best is None or ll > best[0]: best = (ll, nu, math.sqrt(s2))
    return best[1], best[2]
def t_logpdf(x, nu): return gammaln((nu + 1) / 2) - gammaln(nu / 2) - 0.5 * math.log(nu * math.pi) - (nu + 1) / 2 * np.log1p(x * x / nu)
HB = 0.05; HMAX = 6.0
def fit_hist(u):
    au = np.abs(u); nb = int(HMAX / HB); h = np.bincount(np.minimum((au / HB).astype(int), nb), minlength=nb + 1).astype(np.float64) + 0.5
    p = h / h.sum(); ex = au[au > HMAX]; rate = 1.0 / max(1e-3, (ex.mean() - HMAX)) if len(ex) > 5 else 2.0
    return np.log(p[:nb] / (2 * HB)), math.log(p[nb] / 2), rate
def hist_logpdf(u, m):
    lp, lt, rate = m; au = np.abs(u); nb = len(lp); b = np.minimum((au / HB).astype(int), nb - 1)
    return np.where(au < HMAX, lp[b], lt + math.log(rate) - rate * (au - HMAX))
WESC = 0.6745
def esc_logpdf(u): return -(np.floor(np.abs(u) / WESC) + 2) * LN2 - math.log(WESC)
SG = np.exp(np.linspace(math.log(0.3), math.log(3.0), 20))
def em_w(L, iters=200):
    w = np.full(L.shape[1], 1.0 / L.shape[1])
    for _ in range(iters):
        r = L + np.log(w); r -= logsumexp(r, axis=1, keepdims=True); w = np.maximum(np.exp(r).mean(0), 1e-12); w /= w.sum()
    return w
def radial_fit(t):
    lo, hi = np.quantile(t, [0.001, 0.999]); pad = 0.1 * (hi - lo) + 1e-6; lo -= pad; hi += pad; nb = 120
    h = np.bincount(np.clip(((t - lo) / (hi - lo) * nb).astype(int), 0, nb - 1), minlength=nb) + 0.5
    return lo, hi, np.log(h / h.sum() * nb / (hi - lo)), t.mean(), 3 * t.std() + 1e-6
def radial_logpdf(t, m):
    lo, hi, lp, mu, sd = m; nb = len(lp); b = np.clip(((t - lo) / (hi - lo) * nb).astype(int), 0, nb - 1)
    inside = (t >= lo) & (t < hi); lg = -0.5 * ((t - mu) / sd) ** 2 - math.log(sd * math.sqrt(2 * math.pi))
    return np.logaddexp(np.where(inside, lp[b], -np.inf) + math.log(0.98), lg + math.log(0.02))
def logS(k): return math.log(2) + k / 2 * math.log(math.pi) - gammaln(k / 2)

def models(fit, ev):
    out = {}; kk = np.array(KS)
    # base whitenings: 'g' group variances; 's' cluster scalar scale; 'c' cluster variance profile
    for base in ("g", "s", "c"):
        if base == "g": Zs = Z; jac = np.zeros((len(hold), len(KS)))
        elif base == "s": Zs = Z / sc[hc][:, None].astype(np.float32); jac = -np.log(sc[hc])[:, None] * kk[None]
        else: Zs = Z / np.sqrt(vc[hc]); jac = -0.5 * cumk(np.log(vc[hc]))
        tau = group_var(Zs, fit); U = (Zs / np.sqrt(tau[hg])).astype(np.float32); jac = jac - 0.5 * cumk(np.log(tau[hg]))
        Ue = U[ev]; Uf = U[fit]; je = jac[ev]
        ce = lambda lp: -(lp + je).mean(0) / LN2
        out[base + ":gauss"] = ce(cumk(-0.5 * Ue * Ue - 0.5 * math.log(2 * math.pi)))
        # marginals, shape pooled by band
        lt = np.zeros_like(Ue); lh = np.zeros_like(Ue); nus = []
        for lo, hi in bands:
            uf = Uf[:, lo:hi].ravel().astype(np.float64)
            if len(uf) > 2_000_000: uf = rng.choice(uf, 2_000_000, replace=False)
            nu, s = fit_t(uf); nus.append(nu); lt[:, lo:hi] = t_logpdf(Ue[:, lo:hi] / s, nu) - math.log(s)
            lh[:, lo:hi] = hist_logpdf(Ue[:, lo:hi], fit_hist(uf))
        out[base + ":t"] = ce(cumk(lt)); out[base + ":hist"] = ce(cumk(lh)); out[base + ":nu"] = nus
        if base == "g":
            out["g:escape"] = ce(cumk(esc_logpdf(Ue)))
            b = np.abs(Uf).mean(0); out["g:laplace"] = ce(cumk(-np.log(2 * b) - np.abs(Ue) / b))
            out["stats"] = dict(kurt=[float((Ue[:, lo:hi].astype(np.float64) ** 4).mean()) for lo, hi in bands],
                                beyond394=float((np.abs(Ue) > 3.94).mean()), maxabs=float(np.abs(Ue).max()))
        # elliptical: nonparametric law of the whitened radius
        r2f = np.cumsum(np.add.reduceat(Uf.astype(np.float64) ** 2, [0] + KS[:-1], axis=1), axis=1); r2e = np.cumsum(np.add.reduceat(Ue.astype(np.float64) ** 2, [0] + KS[:-1], axis=1), axis=1)
        lp = np.zeros((ev.sum(), len(KS)))
        for j, k in enumerate(KS):
            tf = 0.5 * np.log(r2f[:, j]); te = 0.5 * np.log(r2e[:, j]); lp[:, j] = radial_logpdf(te, radial_fit(tf)) - logS(k) - k * te
        out[base + ":ellip"] = ce(lp)
        out[base + ":radial_cv"] = [float(np.std(r2e[:, j]) / np.mean(r2e[:, j])) for j in range(len(KS))]
        # scale mixture of products of Student-t marginals (nu = inf is the Gaussian scale mixture)
        if base in ("g", "c"):
            for nu in (8, 1e9):
                Lf = np.zeros((fit.sum(), len(SG), len(KS))); Le = np.zeros((ev.sum(), len(SG), len(KS)))
                for j, s in enumerate(SG):
                    f = (lambda x: t_logpdf(x / s, nu) - math.log(s)) if nu < 1e8 else (lambda x: -0.5 * (x / s) ** 2 - math.log(s * math.sqrt(2 * math.pi)))
                    Lf[:, j] = cumk(f(Uf)); Le[:, j] = cumk(f(Ue))
                lp = np.zeros((ev.sum(), len(KS))); lf = []
                for j in range(len(KS)):
                    w = em_w(Lf[:, :, j]); lp[:, j] = logsumexp(Le[:, :, j] + np.log(w), axis=1); lf.append(float(logsumexp(Lf[:, :, j] + np.log(w), axis=1).mean()))
                out[base + ":sm_t%g" % nu] = ce(lp); out[base + ":sm_t%g_fitll" % nu] = lf
        log("  base", base, "done")
    return out
r1 = models(hs == 1, hs == 2); r2 = models(hs == 2, hs == 1)
res = {"KS": KS, "N": int(N), "n": int(n), "K": int(K), "G": int(G)}
for k_ in r1:
    if k_ == "stats": res[k_] = r1[k_]
    elif k_.endswith(":nu"): res[k_] = [r1[k_], r2[k_]]
    else: res[k_] = ((np.array(r1[k_]) + np.array(r2[k_])) / 2).tolist()
g0 = np.array(res["g:gauss"])
log("differential cross-entropy saving against the group Gaussian, bits per item, top-k coordinates (gamma order); k =", KS)
for k_ in sorted(res):
    if ":" in k_ and not k_.endswith(("nu", "fitll", "radial_cv")): log(f"  {k_:14s}", " ".join(f"{x:9.1f}" for x in (g0 - np.array(res[k_]))))
log("  nu by band (g):", res["g:nu"], " radial cv (g):", np.round(res["g:radial_cv"], 3).tolist(), " (c):", np.round(res["c:radial_cv"], 3).tolist()); log("  stats", res["stats"])

# ---- matched distortion: one uniform quantiser on r' = sqrt(Q) z, ideal code length of its cell indices under each model ----
if not ar.noq:
    fit = np.where(hs == 1)[0]; ev = np.where(hs == 2)[0]; fit = rng.choice(fit, min(3000, len(fit)), replace=False); ev = rng.choice(ev, min(3000, len(ev)), replace=False)
    w_g = cnt.sum(1) / cnt.sum(); gam_all = gamfull
    def theta_for(bits):
        lo, hi = 1e-30, gam_all.max()
        for _ in range(200):
            th = math.sqrt(lo * hi)
            if (w_g[:, None] * 0.5 * np.log2(np.maximum(gam_all / th, 1))).sum() > bits: lo = th
            else: hi = th
        return th
    fullmask = hs > 0; tau_g = group_var(Z, hs == 1); Zc = Z / np.sqrt(vc[hc]); tau_c = group_var(Zc, hs == 1)
    sig_g = np.sqrt(tau_g[hg]); sig_c = np.sqrt(tau_c[hg] * vc[hc]); sQ = np.sqrt(Qd[hg])
    tfit = {}
    for lo, hi in bands:
        uf = (Z[hs == 1][:, lo:hi] / sig_g[hs == 1][:, lo:hi]).ravel().astype(np.float64)
        if len(uf) > 2_000_000: uf = rng.choice(uf, 2_000_000, replace=False)
        tfit[(lo, hi)] = fit_t(uf)
    def cellp(lo, hi, sig, nu=None):                    # probability of [lo, hi) under a centred scale family, tail-safe
        flip = lo + hi > 0; a = np.where(flip, -hi, lo) / sig; b = np.where(flip, -lo, hi) / sig
        p = (ndtr(b) - ndtr(a)) if nu is None else (stdtr(nu, b) - stdtr(nu, a))
        return np.log(np.maximum(p, 1e-300))
    res["quant"] = {}
    for mult in (1 / 3, 1, 2, 4, 6):
        bits = mult * n; th = theta_for(bits); dl = math.sqrt(12 * th); kth = float((w_g[:, None] * (gam_all > th)).sum())
        row = {"target_bits": bits, "k_theta": kth}
        def cells(ix):
            y = Z[ix] * sQ[ix]; q = np.rint(y / dl); return (q - 0.5) * dl / sQ[ix], (q + 0.5) * dl / sQ[ix], q
        lof, hif, qf = cells(fit); loe, hie, qe = cells(ev)
        row["nonzero_per_item"] = float((qe != 0).sum(1).mean()); row["mse_rel"] = float((((qe * dl - Z[ev] * sQ[ev]) ** 2).sum()) / ((Z[ev] * sQ[ev]) ** 2).sum())
        row["gauss"] = float(-cellp(loe, hie, sig_g[ev]).sum(1).mean() / LN2)
        row["cluster"] = float(-cellp(loe, hie, sig_c[ev]).sum(1).mean() / LN2)
        lt = np.zeros(loe.shape)
        for (lo, hi), (nu, s) in tfit.items(): lt[:, lo:hi] = cellp(loe[:, lo:hi], hie[:, lo:hi], sig_g[ev][:, lo:hi] * s, nu)
        row["t"] = float(-lt.sum(1).mean() / LN2)
        for nm, sig in (("gauss+scale", sig_g), ("cluster+scale", sig_c)):
            Lf = np.stack([cellp(lof, hif, sig[fit] * s).sum(1) for s in SG], 1); Le = np.stack([cellp(loe, hie, sig[ev] * s).sum(1) for s in SG], 1)
            w = em_w(Lf); row[nm] = float(-logsumexp(Le + np.log(w), axis=1).mean() / LN2)
        res["quant"][f"{mult:.2f}n"] = row
        log(f"quant target {bits:7.0f} bits (={bits / 8:.0f} B) k_theta {kth:6.1f} nonzero {row['nonzero_per_item']:6.1f} | gauss {row['gauss']:8.1f} | saving: t {row['gauss'] - row['t']:6.1f} cluster {row['gauss'] - row['cluster']:6.1f} "
            f"gauss+scale {row['gauss'] - row['gauss+scale']:6.1f} cluster+scale {row['gauss'] - row['cluster+scale']:6.1f}")
json.dump(res, open(BASE + f"entropy/logs/res_{tag}.json", "w"))
log("done")
