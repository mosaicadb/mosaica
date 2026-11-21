# C5, parts 0 and B: the constant by number of coded blocks, and the first randomised level's alternatives with the built prefix and cuts,
# valued in bytes and in time. Built curves, group 12. usage: c5a.py
import math, itertools, json, numpy as np
from scipy.optimize import minimize, brentq
from scipy.special import betainc
from scipy.stats import norm
from design_lib import *
LN_DELTA = -35.97                                              # ln(eps / (N L)) at N = 2^20, L = 4, eps = 1e-9 (SYSTEM.md section 3)
def c1(d):                                                     # exact one-block constant: P[U >= s] = (1/2) I_{1-s^2}((d-1)/2, 1/2) = delta, c = s sqrt(d)
    f = lambda s: math.log(0.5 * betainc((d - 1) / 2, 0.5, 1 - s * s) + 1e-300) - LN_DELTA
    return brentq(f, 1e-3, 0.999) * math.sqrt(d)
CM = 7.963                                                     # certified constant of the four-block layout and its subsets of two or more
print("one-block constant c1(d):", {d: round(c1(d), 3) for d in (31, 63, 95, 127, 223, 255, 287, 479, 671, 767)}, "| several blocks:", CM)
S = spectra(); a, _ = S[12]; n = len(a); F = float(a.sum())
tot = sum(v[1] for v in S.values()); FG = np.array([float(v[0].sum()) for v in S.values()]); WG = np.array([v[1] / tot for v in S.values()])
CUTS = [0, 96, 224, 480, 768]; SIZES = [96, 128, 256, 288]
lam = wf(a, 256 * math.log(2))[1]; ap = np.minimum(a, lam); A = RHO_DET * np.array([ap[CUTS[j]:CUTS[j + 1]].sum() for j in range(4)]) / F
w32 = RHO_DET * wf(a, 256 * math.log(2))[0] / F
def width(rates, merged=None):                                 # h/F at the first randomised level, built curves, constant by number of coded blocks
    J = sum(r > 0 for r in rates); cc = c1(SIZES[[r > 0 for r in rates].index(True)] - 1) if J == 1 else CM
    v = sum(A[j] ** 2 * t_built(rates[j]) / (SIZES[j] - 1) for j in range(4) if rates[j] > 0)
    return cc * math.sqrt(v) + sum(A[j] for j in range(4) if rates[j] == 0)
BITS = 192
def best_free(sub):
    idx = [j for j in range(4) if sub[j]]; w = np.array([SIZES[j] for j in idx], float); best = None
    def f(x):
        r = [0.0] * 4
        for k, j in enumerate(idx): r[j] = max(x[k], 1e-4)
        return math.log(width(r))
    for x0 in (np.full(len(idx), BITS / w.sum()), np.array([0.8 * BITS / w[0]] + [0.2 * BITS / max(w[1:].sum(), 1)] * (len(idx) - 1))):
        x0 = np.maximum(x0, 0.02); x0 *= BITS / float(w @ x0)
        r = minimize(f, x0, method="SLSQP", bounds=[(0.02, 12)] * len(idx), constraints=[{"type": "eq", "fun": lambda z: float(w @ z) - BITS, "jac": lambda z: w}], options={"maxiter": 200, "ftol": 1e-10})
        if abs(float(w @ r.x) - BITS) < 1e-3 * BITS and (best is None or r.fun < best[0]):
            rr = [0.0] * 4
            for k, j in enumerate(idx): rr[j] = float(r.x[k])
            best = (r.fun, rr)
    return math.exp(best[0]), best[1]
MENU = [0, .125, .25, .5, 1, 2, 3, 4]
def best_menu(J):
    best = None
    for rates in itertools.product(MENU, repeat=4):
        if sum(r > 0 for r in rates) != J or sum(s * r for s, r in zip(SIZES, rates)) > BITS: continue
        h = width(rates)
        if best is None or h < best[0]: best = (h, list(rates))
    return best
print(f"group 12, built curves: w32 = {w32:.4f}; block sums after the prefix / F = {np.round(A, 4).tolist()}")
alts = {}
built = [2, 0, 0, 0]; alts["built [2,0,0,0]"] = (width(built), built)
for J in (1, 2, 3, 4):
    fr = min((best_free(sub) for sub in itertools.product((0, 1), repeat=4) if sum(sub) == J), key=lambda v: v[0]); alts[f"{J} blocks, free rates"] = fr
    mn = best_menu(J)
    if mn: alts[f"{J} blocks, menu rates"] = mn
for k, (h, r) in alts.items(): print(f"  {k:26s} h/F = {h:.4f}  ({h / alts['built [2,0,0,0]'][0] - 1:+.1%})  rates {[round(x, 2) for x in r]}")
# value of each alternative: only the number entering 96 B changes (an item goes on when u < w56)
Z = np.linspace(-9, 9, 36001); PZ = norm.pdf(Z); PZ /= PZ.sum()
def share(p, ratio, w):
    zt = norm.isf(p); return sum(wg * float((PZ * (np.abs(Z - zt) / r < w)).sum()) for r, wg in zip(ratio, WG))
CELLS = {"item 1% (kappa 1)": (1e-2, 1.0 * FG / 0.068), "item 50% (kappa 1)": (.5, 1.0 * FG / 0.068), "text 1% (kappa 1.742)": (1e-2, 1.742 * FG / 0.040), "text 50% (kappa 1.742)": (.5, 1.742 * FG / 0.040)}
T0, TB, CODE96, SB = 3.2, 0.133, 36.0, 1.5                      # ns per (item, level); ns per code byte; code bytes of the 96 B level; scalar bytes per (block, rise)
out = {}
for cell, (p, ratio) in CELLS.items():
    e56 = share(p, ratio, w32); e96b = share(p, ratio, alts["built [2,0,0,0]"][0])
    print(f"--- {cell}: entering 56 B {e56:.3f} of N, entering 96 B as built {e96b:.3f}")
    for k, (h, r) in alts.items():
        J = sum(x > 0 for x in r)
        if k.startswith("built") or J == 1: continue
        e96 = share(p, ratio, h); d96 = e96b - e96; extra = e56 * (J - 1)
        dbytes = extra * SB - d96 * (CODE96 + 3 * SB)
        tflip = d96 * (T0 + TB * CODE96) / (extra - 3 * d96) if extra > 3 * d96 else float("inf")
        dt = {tau: extra * tau - d96 * (T0 + TB * CODE96 + 3 * tau) for tau in (1.8, 3.0, 10.0)}
        print(f"  {k:24s} entering 96 B {e96:.3f} ({-d96 / e96b:+.1%}) | bytes per item {dbytes:+.2f} | ns per item at 1.8 / 3 / 10 ns per block-rise: {dt[1.8]:+.2f} / {dt[3.0]:+.2f} / {dt[10.0]:+.2f} | pays in time only below {tflip:.2f} ns")
        out[f"{cell}|{k}"] = [e56, e96b, e96, dbytes, tflip]
json.dump({"alts": {k: [v[0], v[1]] for k, v in alts.items()}, "cells": out, "c1_95": c1(95)}, open("c5a.json", "w"))
