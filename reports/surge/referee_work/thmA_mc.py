# End-to-end Monte Carlo of Theorem A at small dimension: fixed corpus, filter and query; many rotations.
# Adaptive pieces exercised: seed step chosen from estimates, tau raised cluster by cluster, two thread-local taus,
# a tie at rank k/k+1, a filter. Checks (1) pathwise: every wrong answer has a failed bound on a true neighbour;
# (2) Pr[wrong] and Pr[some true-neighbour bound fails] against k*L*delta.  Bad variants for contrast.
import numpy as np, math, sys, time
sys.path.insert(0, __file__.rsplit('/', 1)[0])
from tailcheck import log_tail
n, N, K, k = 16, 1500, 12, 5
m = n - 1; c = float(sys.argv[1]); NS = int(sys.argv[2]); NQ = int(sys.argv[3])
rng = np.random.default_rng(5)
def unit(a): return a / np.linalg.norm(a, axis=-1, keepdims=True)
cen = unit(rng.standard_normal((K, n))); asg = rng.integers(0, K, N)
X = unit(cen[asg] * 0.8 + rng.standard_normal((N, n)) * 0.18)
vis0 = rng.random(N) < 0.6
d1 = math.exp(log_tail(m, c / math.sqrt(m))); L = 2
print(f'n={n} N={N} K={K} k={k} c={c}: one-sided tail {d1:.4e}; bound k*L*2*delta = {k*L*2*d1:.4f}; one-sided bound k*L*delta = {k*L*d1:.4f}')
grid = np.linspace(0.5, 3.0, 13) * math.sqrt(n) * (7 / 3.0) / 2
def code3(z):
    best = np.full(len(z), -1.0); out = np.zeros_like(z)
    for s in grid:
        cd = np.clip(2 * np.floor(z * s / 2) + 1, -7, 7); cs = (cd * z).sum(1) / np.linalg.norm(cd, axis=1)
        up = cs > best; best[up] = cs[up]; out[up] = cd[up]
    return out
def ends(est, R, kap):
    disc = (1 + kap * kap) * R * R - est * est; ok = disc >= 0; sq = np.sqrt(np.maximum(disc, 0))
    hi = np.where(ok, (est + kap * sq) / (1 + kap * kap), np.inf); lo = np.where(ok, (est - kap * sq) / (1 + kap * kap), -np.inf)
    return lo, hi
def topk(ids, sc):
    o = np.lexsort((ids, -sc)); return set(ids[o[:k]].tolist())
def kth(sc): return -np.inf if len(sc) < k else np.partition(sc, len(sc) - k)[len(sc) - k]
tot = {v: 0 for v in ('A_serial', 'A_threads', 'bad_lb_tau', 'bad_w_no_centre')}; totG = 0; inc = {v: 0 for v in tot}; runs = 0
t0 = time.time()
for qi in range(NQ):
    q = unit(X[rng.integers(0, N)] + rng.standard_normal(n) * 0.12)
    # tie at rank k / k+1: append a copy of the k-th visible neighbour with a higher id
    S0 = X @ q; v = np.where(vis0)[0]; kthid = v[np.argsort(-S0[v])[k - 1]]
    Xq = np.vstack([X, X[kthid]]); aq = np.append(asg, asg[kthid]); vis = np.append(vis0, True); M = N + 1
    C = np.stack([Xq[aq == j].mean(0) for j in range(K)])             # centres: fixed, independent of the rotation
    Rres = Xq - C[aq]; rn = np.linalg.norm(Rres, axis=1); rhat = Rres / rn[:, None]
    S = Xq @ q; ids = np.arange(M); visid = ids[vis]
    T = topk(visid, S[visid]); Tl = np.array(sorted(T))
    qc = C @ q; cn2 = (C * C).sum(1); al = (qc / cn2)[aq]; w = np.sqrt(1 - qc * qc / cn2)[aq]
    rho = (C[aq] * Rres).sum(1); known = qc[aq] + al * rho; Dv = q[None, :] - al[:, None] * C[aq]; t = (Dv * Rres).sum(1)
    Rw = rn * w; order = np.argsort(-qc)
    for s in range(NS):
        G = rng.standard_normal((n, n)); Qm, Rr = np.linalg.qr(G); P = Qm * np.sign(np.diag(Rr))
        z = rhat @ P.T; Pq = P @ q; PC = C @ P.T
        UB = []; LB = []; UBbad = None; est1 = None
        for lv, cd in enumerate((np.sign(z), code3(z))):
            cnn = np.linalg.norm(cd, axis=1); cos = (cd * z).sum(1) / cnn; f = rn / (cnn * cos); tanp = np.sqrt(1 - cos * cos) / cos
            eq = f * (cd @ Pq); ecen = f * (cd * PC[aq]).sum(1); est = eq - al * ecen
            lo, hi = ends(est, Rw, c * tanp / math.sqrt(m)); UB.append(known + hi); LB.append(known + lo)
            if lv == 0: est1 = known + est; UBbad = qc[aq] + eq + c * w * rn * tanp / math.sqrt(m)
        Gfail = bool((UB[0][Tl] < S[Tl]).any() or (UB[1][Tl] < S[Tl]).any()); totG += Gfail; runs += 1
        seeds = visid[np.argsort(-est1[visid])[:2 * k]]
        for var in tot:
            ub1 = UBbad if var == 'bad_w_no_centre' else UB[0]
            nth = 2 if var == 'A_threads' else 1
            scored = [list(seeds[aq[seeds] % nth == th]) for th in range(nth)]
            tau = [kth(S[np.array(sc, int)]) for sc in scored]
            if var == 'bad_lb_tau': tau[0] = max(tau[0], kth(LB[1][visid]))
            done = np.zeros(M, bool); done[seeds] = True
            for j in order:
                th = j % nth
                alive = vis & (aq == j) & ~done & ~(ub1 < tau[th]) & ~(UB[1] < tau[th])
                new = ids[alive]; scored[th].extend(new.tolist()); done[new] = True
                tau[th] = max(tau[th], kth(S[np.array(scored[th], int)]))
            allsc = np.array(sorted(set(sum(scored, []))), int)
            wrong = topk(allsc, S[allsc]) != T
            tot[var] += wrong; inc[var] += (wrong and not Gfail)
    print(f'query {qi}: {time.time()-t0:.0f}s', flush=True)
print(f'runs {runs}: Pr[some true-neighbour bound fails] = {totG/runs:.5f}')
for var in tot: print(f'  {var:16s} Pr[wrong] = {tot[var]/runs:.5f} ({tot[var]} runs); wrong while every true-neighbour bound held: {inc[var]}')
