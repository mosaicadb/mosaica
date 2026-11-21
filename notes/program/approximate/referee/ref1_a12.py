"""Referee 1, attack on A1.2 (the leaf floor). Small numpy; one core.

Part 1. Fixed level T. Adaptive leaf-query rules on a two-level Model T, each compared with the claimed frontier
        found*(B) = M E[p; p > lambda(B)],  M P(p > lambda(B)) = B.   A rule above the frontier would refute A1.2.
Part 2. The k best leaves (remark (i)). The least expected rows of a rule that knows every parent value, for
        E[found]/k >= r, computed from the Poisson form of theory.md (3.1) tree by tree, against R(r) at the fixed
        level with M Phibar(z_T) = k.
"""
import math
import numpy as np
from scipy.stats import norm, poisson

rng = np.random.default_rng(11)


def frontier(M, f, zT, grid=20001):
    """Returns budgets B(a) and found(a) for the rule 'parents with Y > a' (v = 1)."""
    y = np.linspace(-8 * math.sqrt(f), 8 * math.sqrt(f), grid)
    dens = norm.pdf(y / math.sqrt(f)) / math.sqrt(f)
    p = norm.sf((zT - y) / math.sqrt(1 - f))
    dy = y[1] - y[0]
    B = M * np.cumsum((dens * dy)[::-1])[::-1]
    found = M * np.cumsum((dens * p * dy)[::-1])[::-1]
    return B, found


print("Part 1. Fixed T: adaptive rules against the claimed frontier (K parents x F leaves)")
K, F, f, L = 200, 500, 0.5, 10
M = K * F
zT = norm.isf(L / M)
sd = math.sqrt(1 - f)
B, FD = frontier(M, f, zT)


def best_found(budget):
    return np.interp(budget, B[::-1], FD[::-1])


trees = 12000
names = ["threshold on parent, a = 0.6 zT f",
         "parents in descending Y, stop at 9 found",
         "parents in descending Y, stop at 9 found or 15 parents",
         "descending Y; leave a parent after 150 leaves with none found",
         "descending Y; stop when next parent's p*F < 0.02 unless last parent held >= 2",
         "random parents until 5 found (a bad rule, for scale)"]
acc = {n: [[], []] for n in names}
for _ in range(trees):
    Y = rng.normal(0, math.sqrt(f), K)
    U = Y[:, None] + rng.normal(0, sd, (K, F))
    hit = U >= zT
    order = np.argsort(-Y)
    hits_o = hit[order]
    per = hits_o.sum(1)
    cum = np.cumsum(per)
    # rule 0
    sel = Y > 0.6 * zT * f
    acc[names[0]][0].append(sel.sum() * F); acc[names[0]][1].append(hit[sel].sum())
    # rule 1: stop inside a parent at the leaf where the 9th target is found
    flat = hits_o.ravel()
    c = np.cumsum(flat)
    i9 = np.searchsorted(c, 9)
    q = min(i9 + 1, M)
    acc[names[1]][0].append(q); acc[names[1]][1].append(c[q - 1])
    q2 = min(q, 15 * F)
    acc[names[2]][0].append(q2); acc[names[2]][1].append(c[q2 - 1])
    # rule 3: abandon a parent after 150 leaves with no target; budget 12 parents
    qq = 0; ff = 0
    for j in range(12):
        row = hits_o[j]
        if row[:150].any():
            qq += F; ff += row.sum()
        else:
            qq += 150
    acc[names[3]][0].append(qq); acc[names[3]][1].append(ff)
    # rule 4: adaptive stop that uses what the last parent held
    pY = norm.sf((zT - Y[order]) / sd) * F
    qq = 0; ff = 0; last = 0
    for j in range(K):
        if pY[j] < 0.02 and last < 2:
            break
        qq += F; last = per[j]; ff += last
    acc[names[4]][0].append(qq); acc[names[4]][1].append(ff)
    # rule 5
    perm = rng.permutation(K)
    cc = np.cumsum(hit[perm].sum(1))
    j = min(np.searchsorted(cc, 5), K - 1)
    acc[names[5]][0].append((j + 1) * F); acc[names[5]][1].append(cc[j])
print(f"   M {M}, f {f}, L {L}, z_T {zT:.3f}, {trees} trees; found is a mean count of targets")
for n in names:
    q = np.array(acc[n][0], float); fd = np.array(acc[n][1], float)
    bf = best_found(q.mean())
    print(f"   {n:78s} E|Q| {q.mean():9.1f}  E found {fd.mean():6.3f} +- {fd.std() / math.sqrt(trees):.3f}  frontier at that E|Q| {bf:6.3f}  excess {fd.mean() - bf:+.3f}")

print("\nPart 2. The k best: least expected rows given every parent value, against R(r) at the fixed level")
print("   K      F     f     k   r     R(r) fixed T   k-best, one price over trees   k-best, each tree to r   ratio (one price / R)   sd of tau_k* over trees (units of sqrt v)")


def R_fixed(M, f, k, r):
    zT = norm.isf(k / M)
    B, FD = frontier(M, f, zT)
    return float(np.interp(r * k, FD[::-1], B[::-1]))


def kbest(K, F, f, k, r, trees=300):
    sd = math.sqrt(1 - f)
    allp = []; pertree = []; taus = []
    for _ in range(trees):
        Y = rng.normal(0, math.sqrt(f), K)
        top = Y.max()
        s = np.linspace(top - 2 * sd, top + 9 * sd, 900 if K <= 600 else 350)
        sf = norm.sf((s[None, :] - Y[:, None]) / sd)          # K x grid
        Lam = F * sf.sum(0)
        H = poisson.cdf(k - 1, Lam)                           # P(tau <= s)
        dH = np.diff(H, prepend=0.0)
        p = (sf * dH[None, :]).sum(1)
        p *= k / (F * p.sum())                                # sum rule, removes grid error
        taus.append(float((s * dH).sum() / dH.sum()))
        allp.append(p)
        o = np.sort(p)[::-1]
        c = np.cumsum(o * F)
        j = np.searchsorted(c, r * k)
        prev = c[j - 1] if j > 0 else 0.0
        pertree.append(j * F + (r * k - prev) / o[j])
    P = np.sort(np.concatenate(allp))[::-1]
    c = np.cumsum(P * F)
    j = np.searchsorted(c, r * k * trees)
    prev = c[j - 1] if j > 0 else 0.0
    rows = (j * F + (r * k * trees - prev) / P[j]) / trees
    return rows, float(np.mean(pertree)), float(np.std(taus))


for (K, F) in ((500, 2000), (339, 2000), (28, 784), (5400, 125)):
    for f in (0.3, 0.45, 0.6):
        for k in (10,):
            for r in (0.9, 0.99):
                Rf = R_fixed(K * F, f, k, r)
                g, pt, st = kbest(K, F, f, k, r, trees=200 if K > 1000 else 300)
                print(f"   {K:5d}  {F:5d}  {f:.2f}  {k:3d}  {r:.2f}  {Rf:12.0f}   {g:12.0f}                   {pt:12.0f}             {g / Rf:5.2f}                  {st:.3f}")
