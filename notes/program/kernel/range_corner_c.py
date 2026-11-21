"""Corner C (finding F3): a whole cluster skipped by the support table although its top row matches.
The query is a stored centre's int8 vector (exact in f16), so it lies along that anchor to within the f32 rounding of
the centre; rho = sqrt(max(qn^2 - beta^2, 0)) (1 + 1e-12) + 1e-12 then falls below the exact |q - beta p|. The top row
is chosen so that its p.x sits just under a point of the table's 2^-15 grid (the table then adds only its 1e-9).
Model: range_model.py.
"""
import math
from fractions import Fraction
import numpy as np
import range_model as rm

rng = np.random.default_rng(11)
n = 8
found = 0
for trial in range(400):
    mi = np.round(rng.standard_normal(n) * 40).clip(-127, 127)
    if np.any(mi == 0):
        continue
    scale = np.float32(rng.uniform(0.5, 1.0) / np.linalg.norm(mi))
    m = (mi.astype(np.float32) * scale)
    m64 = m.astype(np.float64)
    mhat = m64 / math.sqrt(float(np.dot(m64, m64)))
    q = rm.f16(mi * 2.0 ** -math.ceil(math.log2(np.linalg.norm(mi))))
    qn = math.sqrt(float(np.sum(q * q)))
    beta = float(np.dot(q, mhat))
    rho_c = math.sqrt(max(qn * qn - beta * beta, 0.0)) * (1.0 + 1e-12) + 1e-12
    qperp = q - beta * mhat
    if rho_c > 2e-12 or np.linalg.norm(qperp) < 1.5e-8:
        continue
    # candidate top rows: f16 rows at key about 0.9 with a residual along q_perp, p.x just under a grid point
    u = qperp / np.linalg.norm(qperp)
    u = u - (u @ mhat) * mhat
    u /= np.linalg.norm(u)
    X = 0.9 * mhat + math.sqrt(1 - 0.81) * (0.9 * u + 0.1 * rng.standard_normal((200000, n)))
    X = rm.f16(X / np.linalg.norm(X, axis=1, keepdims=True))
    v = X @ mhat
    g = np.ceil((v + 1e-9) * 32768.0) / 32768.0
    gain = X @ qperp - beta * (g - v) - 1e-9          # what the row's score has over beta*hi + 1e-9
    j = int(np.argmax(gain))
    if gain[j] <= 2e-10:
        continue
    top = X[j]
    others = X[(v < v[j] - 1e-3)][:20]
    ix = rm.Index([(m, np.vstack([top, others]))])
    pr = rm.prep(ix, q)
    c = ix.cl[0]
    i = int(np.argmax(c["X"] @ mhat))
    t = float(rm.exact_scores(pr["q"], c["X"][i:i + 1])[0])
    how = rm.support_pre(ix, 0, pr, t)
    isin = rm.ge_exact(pr["q"], c["X"][i], t)
    exact_rho = math.sqrt(float(sum((Fraction(float(a)) - Fraction(beta) * Fraction(float(b))) ** 2 for a, b in zip(pr["q"], ix.anchors[0]))))
    found += 1
    if found <= 3:
        print(f"trial {trial}: |q| = {qn:.6f}, beta = {beta:.9f}, rho as coded = {pr['rho']:.3e}, exact |q - beta p| = {exact_rho:.3e}")
        print(f"   top row: p.x = {float(c['X'][i] @ mhat):.12f}, stored hi = {ix.hi[0, 0]:.12f} (hi - p.x = {ix.hi[0, 0] - float(c['X'][i] @ mhat):.2e}), sqrt(x2) = {math.sqrt(ix.x2):.9f}")
        print(f"   t = the top row's exact score = {t:.12f}; beta*hi + rho*sqrt(x2) = {beta * ix.hi[0, 0] + pr['rho'] * math.sqrt(ix.x2):.12f}; t - 1e-9 = {t - 1e-9:.12f}")
        print(f"   support_pre: {how}; the row's exact score >= t: {isin}  -> {'WRONG: a matching row skipped with its cluster' if how == 'skip' and isin else 'no error'}")
print("centres tried 400; cases built where the cut skips a cluster holding a match:", found)
