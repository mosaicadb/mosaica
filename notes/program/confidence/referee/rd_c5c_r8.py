# Referee: C5.4's first-level alternatives under the proved several-block self-normalised interval (chapter 3,
# Theorem 3.12, R8; [proved, refereed]) at its widest point Ê = 0: random term c*sd/sqrt(1 + xi^2), xi^2 = c^2/Theta,
# Theta = sum_j d_j / tan^2 phi_j over the coded blocks. For one block this is exactly w* (A4) and is never narrower than
# R8 at any other Ê. Same model, prefix, cuts, built curves, cells and cost formula as c5c.py.
import sys, math, itertools, numpy as np
sys.dont_write_bytecode = True
sys.path.insert(0, "..")
src = open("../c5c.py").read().split("res = {}")[0]
exec(src)
def width_r8(rates):
    idx = [j for j in range(4) if rates[j] > 0]; unc = [j for j in range(4) if rates[j] == 0]; c = cc(rates)
    A = A0 * RHO_DET
    t = np.array([t_built(rates[j]) for j in idx]); d = np.array([SIZES[j] - 1 for j in idx]); Ac = np.array([A[j] for j in idx])
    sd = math.sqrt(float((Ac ** 2 * t / d).sum())); xi2 = c * c / float((d / t).sum())
    return sum(A[j] for j in unc) + c * sd / math.sqrt(1 + xi2), xi2
def best_free_r8(J):
    best = None
    for sub in itertools.product((0, 1), repeat=4):
        if sum(sub) != J: continue
        idx = [j for j in range(4) if sub[j]]; w = np.array([SIZES[j] for j in idx], float)
        def f(x):
            r = [0.0] * 4
            for kk, j in enumerate(idx): r[j] = max(x[kk], 1e-4)
            return math.log(width_r8(r)[0])
        for x0 in (np.full(len(idx), BITS / w.sum()), np.array([0.8 * BITS / w[0]] + [0.2 * BITS / max(w[1:].sum(), 1)] * (len(idx) - 1))):
            x0 = np.maximum(x0, 0.02); x0 *= BITS / float(w @ x0)
            r = minimize(f, x0, method="SLSQP", bounds=[(0.02, 12)] * len(idx), constraints=[{"type": "eq", "fun": lambda z: float(w @ z) - BITS, "jac": lambda z: w}], options={"maxiter": 200, "ftol": 1e-10})
            if abs(float(w @ r.x) - BITS) < 1e-3 * BITS and (best is None or r.fun < best[0]):
                rr = [0.0] * 4
                for kk, j in enumerate(idx): rr[j] = float(r.x[kk])
                best = (r.fun, rr)
    return math.exp(best[0]), best[1]
w32 = RHO_DET * wf(a, 256 * math.log(2))[0] / F
alts = {"built [2,0,0,0]": [2, 0, 0, 0], "3 blocks at 1, 1/2, 1/8": [1, .5, .125, 0], "4 blocks at 1/2, 1/2, 1/8, 1/8": [.5, .5, .125, .125],
        "2 blocks at 1, 1/2": [1, .5, 0, 0], "2 blocks at 0.96, 0.78 (S1's)": [0.96, 0.78, 0, 0]}
for J in (2, 3, 4):
    h, r = best_free_r8(J); alts[f"{J} blocks, free under R8"] = r
    mn = min(((width_r8(list(r))[0], list(r)) for r in COMBOS if sum(x > 0 for x in r) == J), key=lambda v: v[0]); alts[f"{J} blocks, best menu under R8"] = mn[1]
hb = width_r8([2, 0, 0, 0])[0]; hp = width([2, 0, 0, 0], "plain built")
print(f"width at 32 B {w32:.4f}; built level: plain {hp:.4f}, R8 at E=0 {hb:.4f}")
for name, r in alts.items():
    h, xi2 = width_r8(r); p = width(r, "plain built")
    print(f"  {name:34s} rates {[round(x, 3) for x in r]}  plain {p:.4f} ({p / hp - 1:+.1%})  R8 {h:.4f} ({h / hb - 1:+.1%})  xi^2 {xi2:.3f}")
for cell, (p, ratio) in CELLS.items():
    e56 = share(p, ratio, w32); e96b = share(p, ratio, hb); line = []
    for name, r in alts.items():
        J = sum(x > 0 for x in r)
        if J == 1: continue
        h = width_r8(r)[0]; d96 = e96b - share(p, ratio, h); extra = e56 * (J - 1)
        tflip = d96 * (T0 + TB * CODE96) / (extra - 3 * d96) if extra > 3 * d96 else float("inf")
        line.append(f"{name}: 96 B {-d96 / e96b:+.0%}, {extra * SB - d96 * (CODE96 + 3 * SB):+.2f} B, pays below {tflip:.2f} ns")
    print(f"  {cell} (enter 56 B {e56:.3f}, 96 B {e96b:.3f}):\n     " + "\n     ".join(line))
