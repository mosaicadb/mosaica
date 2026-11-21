"""After referee 1: Model T's own figures for the k best, at the sizes of the three fits. No real data.

Per design (the internal levels of a regular tree with the measured variance shares; leaves under the finest cells):
  R        the leaf floor of A1.2 at the fixed level with M Phibar(z_T) = k (quadrature);
  told     the floor of A1.2b for every adaptive rule: a knapsack at one price over trees and over tau_k;
  static   the least expected rows of a rule that knows every internal value and not tau_k (one price over trees);
  count    the rows when every tree reads the same number of finest cells, in the order of their values:
           the model's value of the statistic measured in out/tables.md T4.
The k best are taken in the Poisson form of theory.md (3.1): given the internal values, Lambda(tau_k) is Gamma(k, 1).
No leaf is sampled. Histograms of p (2,400 log-spaced bins) stand for the pooled sort, so memory stays small.

usage: python3 topk_model.py [fits|scaling|check]
"""
import math, sys
import numpy as np
from scipy.stats import norm, poisson

K_NN = 10
EDGES = np.concatenate([[0.0], np.logspace(-14, 0, 2400)])


def R_fixed(M, f, k, r, grid=20001):
    zT = norm.isf(k / M)
    y = np.linspace(-8 * math.sqrt(f), 8 * math.sqrt(f), grid)
    dens = norm.pdf(y / math.sqrt(f)) / math.sqrt(f)
    p = norm.sf((zT - y) / math.sqrt(1 - f))
    dy = y[1] - y[0]
    B = M * np.cumsum((dens * dy)[::-1])[::-1]
    FD = M * np.cumsum((dens * p * dy)[::-1])[::-1]
    return float(np.interp(r * k, FD[::-1], B[::-1]))


class Hist:
    def __init__(self):
        self.n = np.zeros(len(EDGES)); self.s = np.zeros(len(EDGES))

    def add(self, p):
        b = np.searchsorted(EDGES, p, side="right") - 1
        self.n += np.bincount(b, minlength=len(EDGES)); self.s += np.bincount(b, weights=p, minlength=len(EDGES))

    def knap(self, weight, target):
        """Rows of the cheapest set of cells whose p sum to target / weight, taking bins from the top."""
        cn = np.cumsum(self.n[::-1]); cs = np.cumsum(self.s[::-1]) * weight
        j = int(np.searchsorted(cs, target))
        if j >= len(cs):
            return float("nan")
        prev_s = cs[j - 1] if j else 0.0; prev_n = cn[j - 1] if j else 0.0
        mean_p = self.s[::-1][j] / max(self.n[::-1][j], 1)
        return (prev_n + (target - prev_s) / (mean_p * weight)) * weight


def run(name, levels, F, trees, draws, rng, rs=(0.9, 0.95), grid=250, keep=6000):
    """levels: [(fan-out, cumulative variance share)] of the internal levels; F rows per finest cell."""
    f = levels[-1][1]; sd = math.sqrt(1 - f)
    Kf = int(np.prod([l[0] for l in levels])); M = Kf * F
    hs, ht = Hist(), Hist()
    cum = np.zeros(min(Kf, keep)); taus = []
    for _ in range(trees):
        Y = np.zeros(1); fp = 0.0
        for fan, fj in levels:
            Y = np.repeat(Y, fan) + rng.normal(0, math.sqrt(fj - fp), len(Y) * fan); fp = fj
        top = Y.max()
        s = np.linspace(top - 2.5 * sd, top + 9 * sd, grid)
        Lam = np.zeros(grid)
        for a in range(0, Kf, 4000):
            Lam += F * norm.sf((s[None, :] - Y[a:a + 4000, None]) / sd).sum(0)
        H = poisson.cdf(K_NN - 1, Lam); dH = np.diff(H, prepend=0.0)
        p = np.empty(Kf)
        for a in range(0, Kf, 4000):
            p[a:a + 4000] = (norm.sf((s[None, :] - Y[a:a + 4000, None]) / sd) * dH[None, :]).sum(1)
        p *= K_NN / (F * p.sum())
        hs.add(p)
        ps = np.sort(p)[::-1][:len(cum)]
        cum += np.cumsum(ps) * F
        gam = rng.gamma(K_NN, 1.0, draws)
        tk = np.interp(np.log(gam), np.log(Lam[::-1]), s[::-1])
        taus.extend(tk)
        for t in tk:
            ht.add(norm.sf((t - Y) / sd))
    cum /= trees
    out = []
    for r in rs:
        Rf = R_fixed(M, f, K_NN, r)
        told = ht.knap(F, r * K_NN * trees * draws) / (trees * draws)
        stat = hs.knap(F, r * K_NN * trees) / trees
        m = np.searchsorted(cum, r * K_NN)
        cnt = float("nan") if m >= len(cum) else (m + (r * K_NN - (cum[m - 1] if m else 0.0)) / (cum[m] - (cum[m - 1] if m else 0.0))) * F
        out.append((r, Rf, told, stat, cnt))
        print(f"   {name:34s} cells {Kf:6d} x {F:5d}  f {f:.3f}  r {r:.2f}:  R {Rf:9.0f}  told {told:9.0f}  static {stat:9.0f}  same count {cnt:9.0f}   "
              f"told/R {told / Rf:4.2f}  static/R {stat / Rf:4.2f}  count/R {cnt / Rf:4.2f}   share of rows: static {100 * stat / M:.2f}%  same count {100 * cnt / M:.2f}%", flush=True)
    print(f"      sd of tau_k over trees and draws, in units of sqrt v: {np.std(taus):.3f}", flush=True)
    return out


FITS = {  # N, groups, clusters, and the measured cumulative shares at groups, clusters, a4, a16, a64 (out/tables.md T1)
    "yahoo": (677305, 339, (0.423, 0.479, 0.528, 0.577)),
    "laion": (1000448, 500, (0.454, 0.506, 0.549, 0.593)),
    "glove": (1183514, 592, (0.358, 0.407, 0.445, 0.487)),
}

if __name__ == "__main__":
    what = sys.argv[1] if len(sys.argv) > 1 else "fits"
    rng = np.random.default_rng(31)
    if what == "fits":
        print("Model T at the sizes of the fits, k = 10 (rows; the finest cells' values are the keys):")
        for ds, (N, K, fs) in FITS.items():
            run(f"{ds} clusters", [(K, fs[0])], round(N / K), 400, 12, rng)
            run(f"{ds} clusters, a4, a16", [(K, fs[0]), (4, fs[1]), (4, fs[2])], round(N / K / 16), 120, 8, rng)
            run(f"{ds} clusters, a4, a16, a64", [(K, fs[0]), (4, fs[1]), (4, fs[2]), (4, fs[3])], round(N / K / 64), 60, 6, rng)
    elif what == "scaling":
        print("The build rule K ~ sqrt N with f = A(1/2) held fixed: local exponent of the static optimum and of R, N to 16 N")
        for ds, f in (("yahoo", 0.455), ("laion", 0.480), ("glove", 0.380)):
            a = run(f"{ds}-like N 1e6", [(500, f)], 2000, 400, 12, rng)
            b = run(f"{ds}-like N 1.6e7", [(2000, f)], 8000, 200, 12, rng)
            for (r, Ra, ta, sa, ca), (_, Rb, tb, sb, cb) in zip(a, b):
                print(f"      {ds} r {r}: d ln rows / d ln N:  R {math.log(Rb / Ra) / math.log(16):.3f}   told {math.log(tb / ta) / math.log(16):.3f}   static {math.log(sb / sa) / math.log(16):.3f}   same count {math.log(cb / ca) / math.log(16):.3f}", flush=True)
    elif what == "check":
        # leaves sampled, no Poisson form: the floor of A1.2b on small trees, against R at the fixed level
        print("Leaves sampled (no Poisson form): the A1.2b floor for the k best against R(r)")
        for (K, F, f, trees) in ((28, 784, 0.45, 4000), (28, 784, 0.30, 4000), (100, 500, 0.45, 2000), (100, 500, 0.60, 2000)):
            sd = math.sqrt(1 - f); h = Hist(); tot = 0.0
            for _ in range(trees):
                Y = rng.normal(0, math.sqrt(f), K)
                U = Y[:, None] + rng.normal(0, sd, (K, F))
                flat = np.sort(U.ravel())
                tk, tk1 = flat[-K_NN], flat[-K_NN - 1]
                # p' of a leaf outside the top k uses tau_k; of a leaf in it, the (k+1)-th best
                intop = (U >= tk).sum(1)
                p_out = norm.sf((tk - Y) / sd); p_in = norm.sf((tk1 - Y) / sd)
                for c in np.nonzero(intop)[0]:
                    h.add(np.full(intop[c], p_in[c]))
                hw = np.repeat(p_out, F - intop)
                h.add(hw)
                tot += p_out @ (F - intop) + p_in @ intop
            for r in (0.9,):
                Rf = R_fixed(K * F, f, K_NN, r)
                fl = h.knap(1, r * K_NN * trees) / trees
                print(f"   {K} x {F}, f {f}: R {Rf:.0f}   floor {fl:.0f}   floor/R {fl / Rf:.2f}   (mean of the sum of p' over a tree's leaves {tot / trees:.3f}, should be {K_NN})", flush=True)
