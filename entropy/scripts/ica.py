# Dependence beyond second order and beyond a per-item scale: independent-component rotations of the whitened top-k residual
# coordinates of the largest groups, scored by held-out cross-entropy against the Gaussian model on the same whitened vector.
# usage: ica.py <dataset> [k]
import numpy as np, sys, math, json
from scipy.special import gammaln, logsumexp
sys.path.insert(0, __import__("os").path.dirname(__file__))
from common import *
name = sys.argv[1]; k = int(sys.argv[2]) if len(sys.argv) > 2 else 128; rng = np.random.default_rng(1)
X = load(name); N, n = X.shape; k = min(k, n); K = max(8, round(N / 1000)); G = 16 if K >= 150 else 8
perm = rng.permutation(N); nA = int(0.8 * N); nB = (N - nA) // 2; split = np.zeros(N, np.int8); split[perm[nA:nA + nB]] = 1; split[perm[nA + nB:]] = 2
M, aA = kmeans(X[split == 0], K); a = assign(X, M); Mg, gc = kmeans(M, G, iters=30, seed=3); grp = gc[a]
qs = X[rng.choice(np.where(split == 0)[0], 200, replace=False)]
LN2 = math.log(2); SG = np.exp(np.linspace(math.log(0.3), math.log(3.0), 20))
def t_logpdf(x, nu): return gammaln((nu + 1) / 2) - gammaln(nu / 2) - 0.5 * math.log(nu * math.pi) - (nu + 1) / 2 * np.log1p(x * x / nu)
def g_logpdf(x): return -0.5 * x * x - 0.5 * math.log(2 * math.pi)
def fit_t_cols(Y):                                     # per-column (nu, scale) by maximum likelihood on a grid of nu
    best = np.full(Y.shape[1], -np.inf); bn = np.zeros(Y.shape[1]); bs = np.ones(Y.shape[1])
    for nu in (3, 4, 6, 8, 12, 20, 40, 1e6):
        s2 = (Y * Y).mean(0)
        for _ in range(30): w = (nu + 1) / (nu + Y * Y / s2); s2 = (w * Y * Y).mean(0)
        ll = (t_logpdf(Y / np.sqrt(s2), nu) - 0.5 * np.log(s2)).mean(0); up = ll > best; best[up] = ll[up]; bn[up] = nu; bs[up] = np.sqrt(s2[up])
    return bn, bs
def em_w(L, iters=200):
    w = np.full(L.shape[1], 1.0 / L.shape[1])
    for _ in range(iters):
        r = L + np.log(w); r -= logsumexp(r, axis=1, keepdims=True); w = np.maximum(np.exp(r).mean(0), 1e-12); w /= w.sum()
    return w
def fastica(U, iters=150, seed=0):
    m, d = U.shape; W = np.linalg.qr(np.random.default_rng(seed).standard_normal((d, d)))[0]
    for _ in range(iters):
        Y = U @ W.T; g = np.tanh(Y); Wn = (g.T @ U) / m - (1 - g * g).mean(0)[:, None] * W
        s, E = np.linalg.eigh(Wn @ Wn.T); W = (E / np.sqrt(s)) @ E.T @ Wn
    return W
def score(Yf, Ye):                                     # bits per item saved against the Gaussian on the same vector
    out = {}; base = g_logpdf(Ye).sum(1).mean()
    nu, s = fit_t_cols(Yf); out["marg_t"] = (sum(t_logpdf(Ye[:, i] / s[i], nu[i]) - math.log(s[i]) for i in range(Ye.shape[1])).mean() - base) / LN2
    for nv in (8, 16, 32, 1e9):
        f = (lambda x, s_: (t_logpdf(x / s_, nv) - math.log(s_)).sum(1)) if nv < 1e8 else (lambda x, s_: (g_logpdf(x / s_) - math.log(s_)).sum(1))
        Lf = np.stack([f(Yf, s_) for s_ in SG], 1); Le = np.stack([f(Ye, s_) for s_ in SG], 1); w = em_w(Lf)
        out["sm_t%g" % nv] = (logsumexp(Le + np.log(w), axis=1).mean() - base) / LN2
    return out
tot = {}; wsum = 0
for g in np.argsort(-np.bincount(grp, minlength=G))[:4]:
    cl = np.where(gc == g)[0]; iA = np.where((grp == g) & (split == 0))[0]; iB = np.where((grp == g) & (split == 1))[0]; iC = np.where((grp == g) & (split == 2))[0]
    RA = X[iA] - M[a[iA]]; lam, V = np.linalg.eigh((RA.T @ RA).astype(np.float64) / len(iA)); lam = np.maximum(lam[::-1], 1e-12); V = V[:, ::-1].astype(np.float32)
    D = (qs[:, None, :] - M[cl][None]).reshape(-1, n) @ V; Q = (D.astype(np.float64) ** 2).mean(0); top = np.argsort(-Q * lam)[:k]; Vt = V[:, top]
    ZA = (RA @ Vt).astype(np.float64); vcl = {}
    for c in cl:
        sel = a[iA] == c; nc = sel.sum(); vcl[c] = (nc * (ZA[sel] ** 2).mean(0) / lam[top] + 50) / (nc + 50) if nc else np.ones(k)
    def coords(ix): return ((X[ix] - M[a[ix]]) @ Vt).astype(np.float64) / np.sqrt(lam[top] * np.array([vcl[c] for c in a[ix]]))
    UA = coords(iA); s, E = np.linalg.eigh(UA.T @ UA / len(UA)); Wh = (E / np.sqrt(s)) @ E.T          # exact whitening on the training split
    UA = UA @ Wh; UB = coords(iB) @ Wh; UC = coords(iC) @ Wh
    W1 = fastica(UA); VA = UA / np.linalg.norm(UA, axis=1, keepdims=True) * math.sqrt(k); W2 = fastica(VA)
    # transform-coding penalty of a scalar code in the rotated basis, in the r' = sqrt(Q) z metric at high resolution (Hadamard):
    # r' = A y with A = Gamma^(1/2) Wh^(-1) W^T (cluster profile taken as 1); penalty = sum_j log2 |A e_j| - log2 |det A| bits per item
    def penalty(W):
        A = (np.sqrt(Q[top] * lam[top])[:, None] * np.linalg.inv(Wh)) @ W.T
        return float(0.5 * np.log2((A * A).sum(0)).sum() - np.linalg.slogdet(A)[1] / math.log(2))
    log(f"group {g}: transform-coding penalty in the r' metric, bits per item: pca {penalty(np.eye(k)):.1f}, ica {penalty(W1):.1f}, ica on directions {penalty(W2):.1f}")
    tot["pen:ica"] = tot.get("pen:ica", 0) + penalty(W1) * len(iA); tot["pen:pca"] = tot.get("pen:pca", 0) + penalty(np.eye(k)) * len(iA)
    kurt = lambda Y: float(((Y ** 4).mean(0) - 3).mean())
    log(f"group {g}: {len(iA)} training items, k = {k}; mean excess kurtosis: pca {kurt(UC):.3f}, ica {kurt(UC @ W1.T):.3f} (max {((UC @ W1.T) ** 4).mean(0).max() - 3:.2f}), ica on directions {kurt(UC @ W2.T):.3f}")
    for nm, W in (("pca", np.eye(k)), ("ica", W1), ("ica_dir", W2)):
        r1 = score(UB @ W.T, UC @ W.T); r2 = score(UC @ W.T, UB @ W.T)
        r = {q: (r1[q] + r2[q]) / 2 for q in r1}; log(f"   {nm:8s}", {q: round(v, 1) for q, v in r.items()})
        for q in r: tot[nm + ":" + q] = tot.get(nm + ":" + q, 0) + r[q] * len(iA)
    wsum += len(iA)
log("weighted mean over the 4 groups, bits per item saved against the Gaussian (same whitening, top-%d):" % k)
for nm in ("pca", "ica", "ica_dir", "pen"): log(f"   {nm:8s}", {q.split(':')[1]: round(float(v) / wsum, 1) for q, v in tot.items() if q.startswith(nm + ":")})
json.dump({q: v / wsum for q, v in tot.items()}, open(BASE + f"entropy/logs/ica_{name}_{k}.json", "w"))
