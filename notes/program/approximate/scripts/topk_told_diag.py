"""Diagnostic for the floor column of topk_model.py at one cluster-level cell: is told > static noise or bias?
Prints the mean of Lambda(tau_k) over draws (should be k) and the two figures for several seeds."""
import math, sys
import numpy as np
from scipy.stats import norm, poisson
sys.path.insert(0, __file__.rsplit("/", 1)[0])
from topk_model import Hist, K_NN

K, F, f, grid = 500, 2001, 0.454, 400
sd = math.sqrt(1 - f)
for seed in (1, 2, 3):
    rng = np.random.default_rng(seed)
    hs, ht = Hist(), Hist(); tot = 0.0; trees, draws = 1200, 16; clip = 0
    for _ in range(trees):
        Y = rng.normal(0, math.sqrt(f), K)
        top = Y.max()
        s = np.linspace(top - 3.5 * sd, top + 9 * sd, grid)
        sf = norm.sf((s[None, :] - Y[:, None]) / sd)
        Lam = F * sf.sum(0)
        H = poisson.cdf(K_NN - 1, Lam); dH = np.diff(H, prepend=0.0)
        p = (sf * dH[None, :]).sum(1); p *= K_NN / (F * p.sum())
        hs.add(p)
        gam = rng.gamma(K_NN, 1.0, draws)
        clip += int((gam > Lam[0]).sum() + (gam < Lam[-1]).sum())
        tk = np.interp(np.log(gam), np.log(Lam[::-1]), s[::-1])
        for t in tk:
            pp = norm.sf((t - Y) / sd); ht.add(pp); tot += F * pp.sum()
    for r in (0.9, 0.95):
        told = ht.knap(F, r * K_NN * trees * draws) / (trees * draws); stat = hs.knap(F, r * K_NN * trees) / trees
        print(f"seed {seed} r {r}: told {told:.0f}  static {stat:.0f}  told/static {told / stat:.3f}   mean Lambda(tau_k) {tot / (trees * draws):.3f}   draws off the grid {clip}", flush=True)
