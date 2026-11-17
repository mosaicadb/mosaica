# Score-level simulation of the independent-cluster model (synthetic only).
# Clusters: n_c rows, score mean mu_c, sd sig_c, standardised shape G (gauss | t_nu | pearson-II of dim dd).
# Poisson model: number of rows of c above a cut L is Poisson(n_c Gbar((L-mu_c)/sig_c)); only the tail is sampled.
import sys, numpy as np
from scipy import stats, special, optimize
rng = np.random.default_rng(int(sys.argv[1]) if len(sys.argv) > 1 else 0)
K, k, NQ = 500, 10, 300
SH = np.array([0.005, 0.008, 0.01, 0.016, 0.02, 0.03, 0.04, 0.063, 0.08, 0.1, 0.125, 0.15, 0.2, 0.25])

def shape(name):
    if name == "gauss": return stats.norm
    if name.startswith("t"):
        nu = float(name[1:]); return stats.t(nu, scale=np.sqrt((nu - 2) / nu))
    if name.startswith("p"):  # coordinate of a uniform point on a sphere in R^dd, scaled to unit variance
        dd = float(name[1:]); a = (dd - 1) / 2; return stats.beta(a, a, loc=-np.sqrt(dd), scale=2 * np.sqrt(dd))

def run(s0, h, true_shape, sig_noise=0.0, label=""):
    G = shape(true_shape)
    class N01:  # fast normal
        sf = staticmethod(lambda x: special.ndtr(-np.asarray(x, float)))
        isf = staticmethod(lambda p: -special.ndtri(p))
        pdf = staticmethod(lambda x: np.exp(-0.5 * np.asarray(x, float) ** 2) / np.sqrt(2 * np.pi))
    res = {}; lamstore = []
    def add(name, rec): res.setdefault(name, []).append(rec)
    chk_num = 0.0; chk_den = 0.0; adapt_tau = []; zeta_margin = {s: [] for s in (0.01, 0.063, 0.25)}
    for qi in range(NQ):
        n = np.maximum(50, (2000 * rng.lognormal(0, 0.5, K))).astype(int); Ntot = n.sum()
        mu = rng.normal(0, 1, K); sig = s0 * rng.lognormal(0, h, K)
        sig_hat = sig * rng.lognormal(0, sig_noise, K) if sig_noise else sig
        lam = lambda t, s=sig, F=G: (n * F.sf((t - mu) / s)).sum()
        L = optimize.brentq(lambda t: lam(t) - 300, -50, 200)
        cnt = rng.poisson(n * G.sf((L - mu) / sig))
        cl = np.repeat(np.arange(K), cnt)
        u = rng.random(cl.size)
        sc = mu[cl] + sig[cl] * G.isf(u * G.sf((L - mu[cl]) / sig[cl]))
        o = np.argsort(-sc)[:k]; tau = sc[o[-1]]
        Nc = np.bincount(cl[o], minlength=K)
        # model quantities (Gaussian model with sig_hat)
        lam_m = lambda t: (n * N01.sf((t - mu) / sig_hat)).sum()
        tau_m = optimize.brentq(lambda t: lam_m(t) - k, -50, 200)
        zeta = lambda t, v=0.0: (t - mu) / np.sqrt(sig_hat ** 2 + v * v)
        zkn = N01.isf(k / n)
        # exact Poisson-model Bayes density p_c = int f_c(s) P(Poi(Lam(s)) <= k-1) ds (Gaussian model), on a grid
        grid = np.linspace(tau_m - 3.0 * s0, tau_m + 6 * s0, 240)
        Lg = (n[None] * N01.sf((grid[:, None] - mu[None]) / sig_hat[None])).sum(1)
        Qk = stats.poisson.cdf(k - 1, Lg)
        fc = N01.pdf((grid[:, None] - mu[None]) / sig_hat[None]) / sig_hat[None]
        p_bayes = np.trapezoid(fc * Qk[:, None], grid, axis=0)
        if qi < 200: chk_num += (n * p_bayes).sum(); chk_den += 1
        # empirical-Bayes shrinkage of log sig_hat to the pooled log-mean, weight w = var(log sig)/(var(log sig)+noise^2),
        # then the predictive exceedance probability integrates the remaining lognormal uncertainty (Gauss-Hermite)
        ls = np.log(sig_hat); vtot = ls.var(); w = max(vtot - sig_noise ** 2, 1e-9) / vtot
        ls_sh = w * ls + (1 - w) * ls.mean(); post_sd = np.sqrt(w) * sig_noise
        gh_x, gh_w = np.polynomial.hermite_e.hermegauss(9); gh_w = gh_w / gh_w.sum()
        sig_sh = np.exp(ls_sh)
        def p_pred(t): return sum(wi * N01.sf((t - mu) / (sig_sh * np.exp(post_sd * xi))) for xi, wi in zip(gh_x, gh_w))
        tau_sh = optimize.brentq(lambda t: (n * p_pred(t)).sum() - k, -50, 300)
        lamstore.append((n, Nc, p_bayes))
        orders = {
            "THR shrunk sig, plug-in": (tau_sh - mu) / sig_sh,
            "THR shrunk sig, predictive": -p_pred(tau_sh),
            "THR shrunk, pred, true tau": -p_pred(tau),
            "lin psi=3 shrunk sig": -(mu + 3 * sig_sh),
            "O1 mu": -mu,
            "lin psi=2": -(mu + 2 * sig_hat), "lin psi=3": -(mu + 3 * sig_hat), "lin psi=4": -(mu + 4 * sig_hat),
            "O3 mu+z(k,n)sig": -(mu + zkn * sig_hat),
            "O3 x1.25": -(mu + 1.25 * zkn * sig_hat), "O3 x0.75": -(mu + 0.75 * zkn * sig_hat),
            "THR model tau (p)": zeta(tau_m),
            "THR model tau (n p)": -n * N01.sf(zeta(tau_m)),
            "THR true tau": zeta(tau),
            "THR model tau, v=.5s0": zeta(tau_m, 0.5 * s0),
            "Bayes p (soft thr)": -p_bayes,
            "clairvoyant N_c/n_c": -(Nc / n) - 1e-9 * (-zeta(tau)),
        }
        B = SH * Ntot
        def rec_of(perm):
            cum = np.cumsum(n[perm]); got = np.cumsum(Nc[perm])
            j = np.searchsorted(cum, B, side="right")  # whole clusters within the budget
            return np.where(j > 0, got[np.maximum(j - 1, 0)], 0) / k, cum
        for name, key in orders.items():
            perm = np.argsort(key, kind="stable"); r, cum = rec_of(perm); add(name, r)
            if name == "THR true tau":
                for s in zeta_margin:
                    j = np.searchsorted(cum, s * Ntot); zeta_margin[s].append(key[perm[min(j, K - 1)]])
        # adaptive: tau_hat solves  #found(>=t) + sum_unvisited n_c Phibar(zeta_c(t)) = k ; re-rank after each probe
        vis = np.zeros(K, bool); perm = []; found = np.empty(0); t_hat = tau_m
        cost = 0
        while cost < B[-1] and len(perm) < K:
            z = zeta(t_hat); z[vis] = np.inf; c = int(np.argmin(z)); vis[c] = True; perm.append(c); cost += n[c]
            found = np.concatenate([found, sc[cl == c]])
            if len(perm) > 24 and len(perm) % 8: continue
            f = lambda t: (found >= t).sum() + (n[~vis] * N01.sf((t - mu[~vis]) / sig_hat[~vis])).sum() - k
            lo, hi = tau_m - 4 * s0, tau_m + 8 * s0
            for _ in range(22):
                mid = (lo + hi) / 2
                if f(mid) > 0: lo = mid
                else: hi = mid
            t_hat = hi
        rest = [c for c in np.argsort(zeta(t_hat)) if not vis[c]]
        add("THR adaptive tau", rec_of(np.array(perm + rest))[0])
        adapt_tau.append((tau_m - tau) / s0)
    # Neyman-Pearson check: one global threshold on p_c across queries vs the same budget for every query
    np_rows = []
    allp = np.concatenate([p for _, _, p in lamstore]); alln = np.concatenate([a for a, _, _ in lamstore]); allN = np.concatenate([b for _, b, _ in lamstore])
    oo = np.argsort(-allp); cn = np.cumsum(alln[oo]) / alln.sum(); cg = np.cumsum(allN[oo]) / (k * len(lamstore))
    glob = [cg[np.searchsorted(cn, s, side="right") - 1] if np.searchsorted(cn, s, side="right") > 0 else 0 for s in SH]
    res["Bayes p, global threshold*"] = [np.array(glob)]
    print(f"\n== {label or ''} s0={s0} h={h} true={true_shape} sig_noise={sig_noise}  (sum n_c p_c = {chk_num/chk_den:.3f}, want {k};"
          f" (tau_model - tau)/s0: mean {np.mean(adapt_tau):+.3f} sd {np.std(adapt_tau):.3f};"
          f" marginal zeta at 1%/6.3%/25%: " + "/".join(f"{np.median(v):.2f}" for v in zeta_margin.values()) + ")")
    print(f"{'order':28s} " + " ".join(f"{s*100:5.1f}%" for s in SH[[0, 3, 5, 7, 10, 13]]) + "   mean   se")
    for name, v in res.items():
        v = np.array(v); m = v.mean(0)
        print(f"{name:28s} " + " ".join(f"{x:6.3f}" for x in m[[0, 3, 5, 7, 10, 13]]) + f"  {m.mean():.3f}  {v.mean(1).std()/np.sqrt(len(v)):.3f}")

if __name__ == "__main__":
    for args in eval(sys.argv[2]) if len(sys.argv) > 2 else [(1.0, 0.35, "gauss")]:
        run(*args)
