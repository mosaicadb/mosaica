# Recheck of the near-equality mixtures in lemma1d.py: at small kappa the best Gaussian mixtures reach C/s(KL) within a few
# 1e-6 of 1 on the trapezoid grid, on both sides. Near the optimum N(0, s^2) the ratio is 1 + O(delta^2), so the grid's error
# decides the sign. Here each candidate is re-evaluated with adaptive quadrature at relative tolerance 1e-13 and with
# Gauss-Hermite-like refinement (two grid densities), and the error is reported.
import math, warnings
import numpy as np
from scipy import optimize, integrate, special, stats
from common import kfun, s_of_kappa, log_s_of_kappa, standard_run_header
import importlib.util, sys

warnings.filterwarnings("ignore")
standard_run_header("lemma1d_mixcheck.py")
rng = np.random.default_rng(5)
SQ2PI = math.sqrt(2 * math.pi)


def I(u):
    u = np.clip(np.asarray(u, float), 0.0, 1.0)
    z = special.ndtri(np.clip(u, 1e-300, 1 - 1e-16))
    return np.where((u <= 0) | (u >= 1), 0.0, np.exp(-0.5 * z * z) / SQ2PI)


def C_mix_grid(w, mu, sd, npts):
    w = np.asarray(w, float) / np.sum(w)
    grids = [np.linspace(m_ - 14 * s_, m_ + 14 * s_, npts) for m_, s_ in zip(mu, sd)]
    grids.append(np.linspace(min(mu - 14 * sd), max(mu + 14 * sd), 2 * npts))
    x = np.unique(np.concatenate(grids))
    z = (x[:, None] - mu[None]) / sd[None]
    lpdf = special.logsumexp(-0.5 * z * z - np.log(sd[None] * SQ2PI) + np.log(w[None]), axis=1)
    F = (w[None] * special.ndtr(z)).sum(1)
    return float(np.trapezoid(I(F), x)), float(np.trapezoid(np.exp(lpdf) * (lpdf + 0.5 * x * x + 0.5 * math.log(2 * math.pi)), x))


def C_mix_quad(w, mu, sd):
    w = np.asarray(w, float) / np.sum(w)
    lo, hi = float(min(mu - 16 * sd)), float(max(mu + 16 * sd))
    pts = sorted(set(np.concatenate([mu, mu - sd, mu + sd, mu - 3 * sd, mu + 3 * sd]).tolist()))
    pts = [p for p in pts if lo < p < hi]
    F = lambda x: float((w * special.ndtr((x - mu) / sd)).sum())
    def kl(x):
        lp = float(special.logsumexp(-0.5 * ((x - mu) / sd) ** 2 - np.log(sd * SQ2PI) + np.log(w)))
        return math.exp(lp) * (lp + 0.5 * x * x + 0.5 * math.log(2 * math.pi))
    C, eC = integrate.quad(lambda x: float(I(F(x))), lo, hi, points=pts, limit=2000, epsabs=1e-15, epsrel=1e-13)
    K, eK = integrate.quad(kl, lo, hi, points=pts, limit=2000, epsabs=1e-15, epsrel=1e-13)
    return C, K, eC, eK


def ratio_err(C, K, eC, eK):
    ls = log_s_of_kappa(K); s = math.exp(ls)
    r = C / s
    dsdk = 1 / (s - 1 / s)                      # ds/dkappa = 1/k'(s)
    return r, r * math.hypot(eC / C, abs(dsdk) * eK / s)


print("kappa | candidate (weights, means, sds) | C/s on grid 3001 | grid 12001 | adaptive quad +- error")
worst = (9, None)
for kap in (0.01, 0.1, 1.0, 2.0, 5.0, 10.0):
    s = s_of_kappa(kap)
    for nc in (2, 3):
        for t in range(10):
            x0 = np.concatenate([rng.normal(0, 1, nc), rng.normal(0, 1.2, nc), math.log(s) + rng.normal(0, 0.8, nc)])
            def obj(p):
                C, KL = C_mix_grid(np.exp(p[:nc]), p[nc:2 * nc], np.exp(np.clip(p[2 * nc:], -8, 2)), 3001)
                if not (KL > 0 and np.isfinite(C)): return 1e9
                return C / math.exp(log_s_of_kappa(KL)) + 30 * math.log(KL / kap) ** 2
            rr = optimize.minimize(obj, x0, method="Nelder-Mead", options={"xatol": 1e-6, "fatol": 1e-10, "maxiter": 1500})
            p = rr.x; w = np.exp(p[:nc]); mu = p[nc:2 * nc]; sd = np.exp(np.clip(p[2 * nc:], -8, 2))
            g1 = C_mix_grid(w, mu, sd, 3001); g2 = C_mix_grid(w, mu, sd, 12001)
            if abs(math.log(g1[1] / kap)) > 0.01: continue
            r1 = g1[0] / math.exp(log_s_of_kappa(g1[1])); r2 = g2[0] / math.exp(log_s_of_kappa(g2[1]))
            C, K, eC, eK = C_mix_quad(w, mu, sd)
            rq, er = ratio_err(C, K, eC, eK)
            if r1 < 1.00001 or rq < worst[0]:
                print("%5.2f | w=%s mu=%s sd=%s | %.9f | %.9f | %.11f +- %.1e" % (kap, np.round(w / w.sum(), 4), np.round(mu, 4), np.round(sd, 4), r1, r2, rq, er))
            if rq < worst[0]: worst = (rq, er, kap)
print("\nsmallest ratio by adaptive quadrature: %.11f +- %.1e at kappa %.2f (below 1 by more than the error would refute)" % worst)
