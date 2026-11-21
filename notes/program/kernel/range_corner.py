"""Corner cases for the cut and the sorted range (model: range_model.py).
A. Finding F1: a row with x.m^ > 1 + DA is decided out at a threshold equal to its own score, with |t| < |q|.
B. Finding F3: the rounding of rho = sqrt(qn^2 - beta^2) and s1 = sqrt(qn^2 - cm^2) when the query lies along an
   anchor or a centre: the code's (1 + 1e-12) + 1e-12 against the exact value (rationals).
"""
import math
from fractions import Fraction
import numpy as np
import range_model as rm

rng = np.random.default_rng(7)


def unit(v):
    return v / np.linalg.norm(v, axis=-1, keepdims=True)


def fr(v):
    return [Fraction(float(x)) for x in v]


def fdot(a, b):
    return sum(x * y for x, y in zip(a, b))


print("A. the clamp of the key")
n = 16
p = unit(rng.standard_normal(n))
w = rng.standard_normal(n)
w = unit(w - (w @ p) * p)
d = w                                             # the centre direction of cluster c, orthogonal to the anchor p
# cluster c: near-duplicates of a row along d whose f16 norm is 1 + 2.4e-4 (the 1M index logs x2 - 1 = 4.88e-4);
# the others lean slightly away from the anchor, so the long row has the cluster's largest p.x
xs = [rm.f16(1.00012 * d)]
for _ in range(5):
    e = rng.standard_normal(n)
    e -= (e @ p) * p + (e @ d) * d
    xs.append(rm.f16(unit(d - 0.01 * p + 0.02 * unit(e)) * 0.99995))
Xc = np.array(xs)
Xa = rm.f16(unit(p + 0.2 * rng.standard_normal((40, n)) / math.sqrt(n)))
ix = rm.Index([((0.9 * p).astype(np.float32), Xa), ((0.9 * d).astype(np.float32), Xc)])
c = ix.cl[1]
i = int(np.argmax(c["a"]))
q = unit(0.75 * ix.cl[0]["mhat"] + 0.65 * c["mhat"])
pr = rm.prep(ix, q)
s = rm.exact_scores(pr["q"], c["X"])
t = float(s[i])
dec, how = rm.decisions(ix, 1, pr, t)
print(f"  sqrt(x2) - 1 = {math.sqrt(ix.x2) - 1:.3e}; DA = {rm.DA:.3e}; anchor used = cluster {pr['a']} (the nearest)")
print(f"  row {i}: |x| - 1 = {np.linalg.norm(c['X'][i]) - 1:.3e}, true key = 1 + {c['a'][i] - 1:.3e}, stored key = {c['key'][i]}/32767")
print(f"  |q| = {pr['qn']:.6f}, t = the row's exact score = {t:.9f} (|t| < |q|: {abs(t) < pr['qn']})")
print(f"  path {how}; decision for the row: {int(dec[i])} (-1 out, 0 undecided, +1 in); exact s >= t: {rm.ge_exact(pr['q'], c['X'][i], t)}")
cu = rm.cut_params(ix, 1, pr)
rr = math.sqrt(ix.x2)
print("  key interval not excluded by the table bound:", rm.sup_set(cu["rm"], cu["rp"], rr, t - cu["bh"] - 1e-9), "by the annulus:", rm.sup_set(cu["cm"], cu["s1"], rr, t - 1e-9))
dec2, _ = rm.decisions(ix, 1, pr, t, rm.DA + 1e-9 + max(0.0, rr - 1.0))
print(f"  with eps = DA + 1e-9 + (sqrt(x2) - 1): decision {int(dec2[i])}")

print("B. rho and s1 for a query along a centre that is an anchor")
for n in (32, 768):
    worst = dict(rho=0.0, s1=0.0)
    under = dict(rho=0, s1=0)
    over_slack9 = dict(rho=0, s1=0)
    trials = 400 if n == 32 else 60
    for tr in range(trials):
        mi = np.round(rng.standard_normal(n) * 40).clip(-127, 127)
        scale = np.float32(rng.uniform(0.5, 1.0) / np.linalg.norm(mi))
        m = (mi.astype(np.float32) * scale).astype(np.float64)             # stored centre: int8 times a scale, in f32
        mn = math.sqrt(float(np.dot(m, m)))
        mhat = m / mn
        q = mi * 2.0 ** -math.ceil(math.log2(np.linalg.norm(mi)))            # the int8 vector, exact in f16, |q| in (0.5, 1]
        kind = tr % 3
        if kind == 1:                                                       # one zero coordinate nudged by a few f16 subnormals
            z = np.nonzero(mi == 0)[0]
            if len(z):
                q[z[0]] = rng.integers(1, 200) * 2.0 ** -24
        elif kind == 2:                                                     # one coordinate moved by one f16 step
            j = int(np.argmin(np.where(mi == 0, 1e9, np.abs(mi))))
            q[j] = float(np.nextafter(np.float16(q[j]), np.float16(2.0)))
        q = rm.f16(q)
        qn = math.sqrt(float(np.sum(q * q)))
        beta = float(np.dot(q, mhat))
        rho_c = math.sqrt(max(qn * qn - beta * beta, 0.0)) * (1.0 + 1e-12) + 1e-12
        qm = float(np.dot(q, m))
        cm = qm / mn
        s1_c = math.sqrt(max(qn * qn - cm * cm, 0.0)) * (1.0 + 1e-12) + 1e-12
        qf, pf = fr(q), fr(mhat)
        for name, coef, got in (("rho", Fraction(beta), rho_c), ("s1", Fraction(cm), s1_c)):
            diff = [a - coef * b for a, b in zip(qf, pf)]
            true = math.sqrt(float(fdot(diff, diff)))                      # |q - coef p|, exact then one rounding
            if got < true:
                under[name] += 1
                worst[name] = max(worst[name], true - got)
                over_slack9[name] += (true - got) > 1e-9
    print(f"  n = {n}, {trials} queries: rho below the exact |q - beta p| in {under['rho']} (largest deficit {worst['rho']:.2e}, above 1e-9 in {over_slack9['rho']});"
          f" s1 below the exact |q - cm m^| in {under['s1']} (largest {worst['s1']:.2e}, above 1e-9 in {over_slack9['s1']})")
