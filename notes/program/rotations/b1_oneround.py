"""B1: one round R = H·D against the code as built. Exact enumeration of the signs that matter.

(a) y = e_a + e_b: the error vector E = R^T(kappa*o) - y for every sign choice, depths 1 to 3.
(b) y with k non-zero coordinates: the support of E, and the failure rate of the certified width for a query on it.
"""
import itertools
import numpy as np
from lib import fwht, built_code, estimate_error

n = 256
C = 7.8
rng = np.random.default_rng(1)


def run(y, d_signs, depth):
    """One draw: D given as a full sign vector. Returns E (original coordinates), A, tan."""
    z = fwht(d_signs * y)[None, :]
    cell = built_code(z, [depth])
    kappa, e, A, tan = estimate_error(z, cell, depth, depth)
    E = d_signs * fwht(e[0])  # R^T e = D H e
    return E, A[0], tan[0]


print(f"n = {n}; (a) y = e_a + e_b")
for (a, b) in [(0, 1), (1, 2), (5, 200)]:
    s = a ^ b
    y = np.zeros(n); y[a] = 1; y[b] = 1
    for depth in (1, 2, 3):
        pats = {}
        idx = sorted({0, a, b, s})
        for signs in itertools.product((1, -1), repeat=len(idx)):
            d = rng.choice((-1.0, 1.0), n)  # the other signs are random and must not matter
            for i, sg in zip(idx, signs):
                d[i] = sg
            E, A, tan = run(y, d, depth)
            nz = np.flatnonzero(np.abs(E) > 1e-9)
            pred = np.zeros(n)
            pred[0] += d[0]; pred[s] += -d[a] * d[b] * d[s]
            scale = A / np.sqrt(2) if a != 0 and s != 0 else A / np.linalg.norm(pred)
            ok = np.allclose(E, pred * scale, atol=1e-9)
            pats[signs] = (tuple(nz), round(A, 6), round(tan, 6), ok)
        supports = {v[0] for v in pats.values()}
        allok = all(v[3] for v in pats.values())
        A0 = next(iter(pats.values()))[1]; t0 = next(iter(pats.values()))[2]
        print(f"  a,b = {a},{b}  s = {s}  depth {depth}: supports of E over all sign choices {supports}; "
              f"A = {A0}, tan = {t0}; E = (A/|.|)(d0 e0 - da db ds es) in every case: {allok}")
    # the query: Delta along the error's two coordinates
    depth = 1
    if a == 0:
        Dl = np.zeros(n); Dl[a] = 1; Dl[b] = -1; Dl /= np.sqrt(2)
    else:
        Dl = np.zeros(n); Dl[0] = 1; Dl[s] = 1; Dl /= np.sqrt(2)
    errs = []
    for t in range(4000):
        d = rng.choice((-1.0, 1.0), n)
        E, A, tan = run(y, d, depth)
        Dperp = Dl - (Dl @ y) / (y @ y) * y
        sd = A * np.linalg.norm(Dperp) / np.sqrt(n - 1)
        errs.append((E @ Dl) / sd)
    errs = np.array(errs)
    print(f"    query on the error's coordinates, 4000 draws: error/sd takes values {np.unique(np.round(errs, 3))}; "
          f"Pr[error > {C} sd] = {(errs > C).mean():.3f}, Pr[error < -{C} sd] = {(errs < -C).mean():.3f}; sqrt(n-1) = {np.sqrt(n-1):.3f}")
    Dl = np.zeros(n); Dl[s if s != b else 3] = 1
    errs = []
    for t in range(2000):
        d = rng.choice((-1.0, 1.0), n)
        E, A, tan = run(y, d, depth)
        sd = A * np.linalg.norm(Dl - (Dl @ y) / (y @ y) * y) / np.sqrt(n - 1)
        errs.append((E @ Dl) / sd)
    print(f"    query e_s alone: |error|/sd values {np.unique(np.round(np.abs(errs), 3))}")

print("(b) y with k random non-zero coordinates on random positions; depth 2")
for k in (2, 3, 4, 5, 6, 7):
    pos = rng.choice(np.arange(1, n), k, replace=False)
    y = np.zeros(n); y[pos] = rng.standard_normal(k)
    span = {0}
    for p in pos:
        span |= {q ^ int(p) for q in span}
    span = sorted(span)
    # a fixed query spread evenly over the span, made orthogonal to y
    Dl = np.zeros(n); Dl[span] = rng.choice((-1.0, 1.0), len(span))
    Dl -= (Dl @ y) / (y @ y) * y
    Dl /= np.linalg.norm(Dl)
    out_frac = []; ratios = []
    for t in range(20000 if k <= 5 else 6000):
        d = rng.choice((-1.0, 1.0), n)
        E, A, tan = run(y, d, 2)
        mask = np.ones(n, bool); mask[span] = False
        out_frac.append((E[mask] ** 2).sum() / max((E ** 2).sum(), 1e-300))
        ratios.append((E @ Dl) / (A / np.sqrt(n - 1)))
    ratios = np.array(ratios)
    print(f"  k = {k}: |xor-span| = {len(span)}; largest share of |E|^2 outside the span = {max(out_frac):.2e}; "
          f"error/sd: rms {np.sqrt((ratios**2).mean()):.2f}, max {np.abs(ratios).max():.2f}, "
          f"Pr[|error| > {C} sd] = {(np.abs(ratios) > C).mean():.4f} (law: 2.5e-19 per side at the built budget)")
