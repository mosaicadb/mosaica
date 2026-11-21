"""Referee 1, A1.2 remark (i): a floor for the k best that holds for every adaptive leaf-query rule.

For a rule told every parent value, E[found] = E sum_{i in Q} p'_i exactly, with
p'_i = P(U_i > tau_(-i) | parents, other leaves) = Phibar((tau_(-i) - Y_par(i))/sd), and tau_(-i) the k-th best among
the other leaves: the k-th best overall for every leaf outside the top k. Then
    E|Q| >= sup_lambda ( r k - E sum_i (p'_i - lambda)_+ ) / lambda ,
the fractional knapsack over leaves at one price over the trees and over the realisations of tau_k.

Given the parents, Lambda(tau_k) = sum_c F Phibar((tau_k - Y_c)/sd) is Gamma(k, 1) in the Poisson form of theory.md
(3.1), so tau_k is drawn by drawing Lambda and inverting; no leaf is sampled. The k leaves of the top k, whose p'
uses the (k+1)-th best, are left at the k-th: this lowers the floor by a negligible amount and keeps it a floor only
up to that amount.

Columns: R(r) at the fixed level; the floor above ("told tau_k"); the static optimum (a rule that knows the parents
and not tau_k, by the method of ref1_a12.py).
"""
import math
import numpy as np
from scipy.stats import norm, poisson

rng = np.random.default_rng(21)


def R_fixed(M, f, k, r, grid=20001):
    zT = norm.isf(k / M)
    y = np.linspace(-8 * math.sqrt(f), 8 * math.sqrt(f), grid)
    dens = norm.pdf(y / math.sqrt(f)) / math.sqrt(f)
    p = norm.sf((zT - y) / math.sqrt(1 - f))
    dy = y[1] - y[0]
    B = M * np.cumsum((dens * dy)[::-1])[::-1]
    FD = M * np.cumsum((dens * p * dy)[::-1])[::-1]
    return float(np.interp(r * k, FD[::-1], B[::-1]))


def knap(P, weight, target):
    P = np.sort(P)[::-1]
    c = np.cumsum(P * weight)
    j = int(np.searchsorted(c, target))
    if j >= len(P):
        return float("nan")
    prev = c[j - 1] if j > 0 else 0.0
    return j * weight + (target - prev) / P[j]


print("   K      F     f     r     R(r) fixed T   floor, any adaptive rule (told tau_k)   static optimum   floor/R   static/R")
k = 10
for (K, F, trees, draws) in ((500, 2000, 300, 16), (339, 2000, 400, 16), (28, 784, 1500, 20), (5400, 125, 60, 8)):
    for f in (0.3, 0.45, 0.6):
        sd = math.sqrt(1 - f)
        told = []; stat = []
        for _ in range(trees):
            Y = rng.normal(0, math.sqrt(f), K)
            top = Y.max()
            s = np.linspace(top - 2.5 * sd, top + 9 * sd, 700 if K <= 600 else 300)
            sf = norm.sf((s[None, :] - Y[:, None]) / sd)
            Lam = F * sf.sum(0)                                   # decreasing in s
            H = poisson.cdf(k - 1, Lam); dH = np.diff(H, prepend=0.0)
            p = (sf * dH[None, :]).sum(1); p *= k / (F * p.sum())
            stat.append(p)
            gam = rng.gamma(k, 1.0, draws)
            tk = np.interp(np.log(gam), np.log(Lam[::-1]), s[::-1])
            told.append(norm.sf((tk[None, :] - Y[:, None]) / sd).ravel())
        del sf
        told = np.concatenate(told); stat = np.concatenate(stat)
        for r in (0.9, 0.99):
            Rf = R_fixed(K * F, f, k, r)
            fl = knap(told, F, r * k * trees * draws) / (trees * draws)
            st = knap(stat, F, r * k * trees) / trees
            print(f"   {K:5d}  {F:5d}  {f:.2f}  {r:.2f}  {Rf:12.0f}   {fl:12.0f}                           {st:12.0f}     {fl / Rf:5.2f}     {st / Rf:5.2f}")
        print(f"          (mean of Lambda(tau_k) over the draws: {told.sum() * F / (trees * draws):.3f}, should be {k})")
