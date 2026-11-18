# Synthetic checks for hierarchy/policy.md (no real data, nothing timed):
#   1. On random small trees (groups -> clusters -> leaves, a setup cost per group and per cluster): the order "always
#      the available action of largest index" (a) minimises sum of value x completion time among all feasible orders
#      (exhaustive subset dynamic programme), (b) has every Lagrangian optimum S(lambda) as a prefix.
#   2. Lazy evaluation under valid upper bounds returns the eager sequence.
#   3. Calibration error: with values wrong by factors within exp(+-delta), the chosen set holds at least
#      exp(-2 delta) of the value of the best closed set of no greater cost (exhaustive).
#   4. k-NN, where tau moves: a score-level simulation of the policy with the true tau, the plug-in tau*, the
#      self-consistent running estimate, and the bare running k-th best.
#   python3 policy_check.py
import heapq, itertools, numpy as np
from statistics import NormalDist
ND = NormalDist()
rng = np.random.default_rng(11)


# ---------------------------------------------------------------- trees
def make_tree(ng=2, nc=2, nl=2):
    """jobs: (kind, parent, cost, value). kind 0 group setup, 1 cluster setup, 2 leaf."""
    jobs = []
    for g in range(ng):
        gi = len(jobs); jobs.append((0, -1, float(rng.uniform(0.2, 3)), 0.0))
        for c in range(nc):
            ci = len(jobs); jobs.append((1, gi, float(rng.uniform(0.1, 1.5)), 0.0))
            for l in range(nl):
                jobs.append((2, ci, float(rng.uniform(0.3, 2)), float(rng.exponential(1.0))))
    return jobs


def children(jobs):
    ch = [[] for _ in jobs]
    for j, (k, p, c, v) in enumerate(jobs):
        if p >= 0:
            ch[p].append(j)
    return ch


def V(jobs, ch, j, lam):
    """Value of the subtree of j at price lam, j's own cost included (0 if not worth entering)."""
    k, p, c, v = jobs[j]
    if k == 2:
        return max(v - lam * c, 0.0)
    return max(sum(V(jobs, ch, i, lam) for i in ch[j]) - lam * c, 0.0)


def S_of(jobs, ch, lam, vals=None):
    """The Lagrangian optimum at price lam: the jobs entered (strictly positive value), as a set."""
    out = set()
    def val(j):
        k, p, c, v = jobs[j]
        if vals is not None and k == 2:
            v = vals[j]
        if k == 2:
            return max(v - lam * c, 0.0)
        return max(sum(val(i) for i in ch[j]) - lam * c, 0.0)
    def walk(j):
        if val(j) > 0:
            out.add(j)
            for i in ch[j]:
                walk(i)
    for j, (k, p, c, v) in enumerate(jobs):
        if p < 0:
            walk(j)
    return out


def index(jobs, ch, j, vals=None):
    """nu_j = sup{lam : V_j(lam) > 0} = the largest value/cost of a closed set rooted at j (Dinkelbach)."""
    def tot(j, lam):
        k, p, c, v = jobs[j]
        if vals is not None and k == 2:
            v = vals[j]
        if k == 2:
            return (v, c) if v - lam * c > 0 else (0.0, 0.0)
        a = b = 0.0
        for i in ch[j]:
            x, y = tot(i, lam); a += x; b += y
        return (a, b + c) if a - lam * (b + c) > 0 else (0.0, 0.0)
    k, p, c, v = jobs[j]
    if k == 2:
        return (v if vals is None else vals[j]) / c
    lam = 0.0
    for _ in range(100):
        # at lam = 0 every leaf with positive value is in
        a, b = tot(j, lam)
        if b == 0:
            return lam
        new = a / b
        if new <= lam + 1e-15:
            return lam
        lam = new
    return lam


def greedy(jobs, ch):
    avail = [(-index(jobs, ch, j), j) for j, (k, p, c, v) in enumerate(jobs) if p < 0]
    heapq.heapify(avail); order = []; idx = []
    while avail:
        ni, j = heapq.heappop(avail); order.append(j); idx.append(-ni)
        for i in ch[j]:
            heapq.heappush(avail, (-index(jobs, ch, i), i))
    return order, idx


def area(jobs, order):
    t = 0.0; s = 0.0
    for j in order:
        t += jobs[j][2]; s += jobs[j][3] * t
    return s


def best_area(jobs):
    n = len(jobs); full = (1 << n) - 1
    cost = np.zeros(1 << n)
    for m in range(1, 1 << n):
        low = (m & -m).bit_length() - 1
        cost[m] = cost[m & (m - 1)] + jobs[low][2]
    INF = float("inf"); f = [INF] * (1 << n); f[0] = 0.0
    for m in range(1 << n):
        if f[m] == INF:
            continue
        for j in range(n):
            if m >> j & 1:
                continue
            p = jobs[j][1]
            if p >= 0 and not (m >> p & 1):
                continue
            m2 = m | 1 << j
            v = f[m] + jobs[j][3] * cost[m2]
            if v < f[m2]:
                f[m2] = v
    return f[full]


print("1. Trees with setup costs (2 groups x 2 clusters x 2 leaves, 14 jobs; 200 random instances)")
worst = 0.0; bad_prefix = 0; nonmono = 0
for trial in range(200):
    jobs = make_tree(); ch = children(jobs)
    order, idx = greedy(jobs, ch)
    worst = max(worst, area(jobs, order) / best_area(jobs) - 1)
    for lam in list(np.quantile(idx, [0.1, 0.3, 0.5, 0.7, 0.9])) + [x * (1 - 1e-9) for x in idx]:
        S = S_of(jobs, ch, lam)
        if set(order[:len(S)]) != S:
            bad_prefix += 1
print(f"   largest relative excess of the index order over the optimal sum of value x completion time: {worst:.2e}; "
      f"prices at which S(lambda) is not a prefix of the order: {bad_prefix}")

# ---------------------------------------------------------------- 2. lazy = eager
print("2. Lazy evaluation (300 random trees, 3 groups x 3 clusters x 3 leaves, upper bounds = exact x (1 + U(0, 2)))")
diff = 0; evals = 0; total = 0
for trial in range(300):
    jobs = make_tree(3, 3, 3); ch = children(jobs)
    eager, _ = greedy(jobs, ch)
    ub = {j: index(jobs, ch, j) * (1 + 2 * rng.uniform()) for j in range(len(jobs))}
    heap = [(-ub[j], j, False) for j, (k, p, c, v) in enumerate(jobs) if p < 0]; heapq.heapify(heap); lazy = []
    while heap:
        nk, j, fresh = heapq.heappop(heap)
        if not fresh:
            evals += 1
            heapq.heappush(heap, (-index(jobs, ch, j), j, True)); continue
        lazy.append(j)
        for i in ch[j]:
            heapq.heappush(heap, (-ub[i], i, False))
    # stop both at a price: compare the prefixes above the median index
    diff += lazy != eager; total += len(jobs)
print(f"   sequences that differ: {diff} of 300 (run to the end, so every index is evaluated: {evals} of {total}; "
      f"the saving of laziness is in what lies below the stopping price)")

# ---------------------------------------------------------------- 3. calibration error
print("3. Calibration error (2 x 2 x 2 trees, values misjudged by factors exp(U(-delta, delta)); exhaustive best closed set of no greater cost)")
for delta in (0.1, 0.3, 0.7):
    worst = 1.0; mean = []
    for trial in range(150):
        jobs = make_tree(); ch = children(jobs); n = len(jobs)
        vals = {j: jobs[j][3] * float(np.exp(rng.uniform(-delta, delta))) for j in range(n)}
        closed = []
        for m in range(1 << n):
            ok = True
            for j in range(n):
                if m >> j & 1 and jobs[j][1] >= 0 and not (m >> jobs[j][1] & 1):
                    ok = False; break
            if ok:
                closed.append((sum(jobs[j][2] for j in range(n) if m >> j & 1), sum(jobs[j][3] for j in range(n) if m >> j & 1)))
        closed = np.array(closed)
        for lam in (0.2, 0.5, 1.0):
            S = S_of(jobs, ch, lam, vals)
            if not S:
                continue
            c = sum(jobs[j][2] for j in S); g = sum(jobs[j][3] for j in S)
            best = closed[closed[:, 0] <= c + 1e-12, 1].max()
            worst = min(worst, g / best); mean.append(g / best)
    print(f"   delta {delta}: smallest ratio {worst:.3f}, mean {np.mean(mean):.3f}; bound exp(-2 delta) = {np.exp(-2 * delta):.3f}")

# ---------------------------------------------------------------- 4. k-NN with a moving tau
print("4. k-NN (k = 10): 40 groups x 10 clusters, n = 2,000, Gaussian scores N(mu_c, sigma_c^2), mu_c = group effect + N(0, 0.6^2), "
      "sigma_c = 0.3 LogNormal(0, 0.3); costs in microseconds: row 0.03, cluster 8, group 40. 200 queries. Mean recall | mean cost (ms)")
G_, CPG, NROW, KNN = 40, 10, 2000, 10
C_ROW, C_CL, C_GRP = 0.03, 8.0, 40.0
Kc = G_ * CPG; grp_of = np.repeat(np.arange(G_), CPG)
sf = np.vectorize(lambda x: 1 - ND.cdf(x))


def plan(v, opened_g, done, lam):
    """Indices of the available actions given per-cluster values v (expected answers): returns (best index, action)."""
    best = (-1.0, None)
    wc = C_CL + C_ROW * NROW
    dens = v / wc
    for g in range(G_):
        m = (grp_of == g) & ~done
        if not m.any():
            continue
        ids = np.where(m)[0]
        if opened_g[g]:
            i = ids[np.argmax(dens[ids])]
            if dens[i] > best[0]:
                best = (dens[i], i)
        else:
            o = ids[np.argsort(-dens[ids])]
            r = np.cumsum(v[o]) / (C_GRP + wc * np.arange(1, len(o) + 1))
            if r.max() > best[0]:
                best = (r.max(), o[0])            # opening the group and reading its best cluster
    return best


def run(mode, lam, mu, sg, scores_top, tau_true):
    """scores_top[c]: the cluster's 30 best scores, descending (enough for k = 10)."""
    done = np.zeros(Kc, bool); opened = np.zeros(G_, bool); cost = 0.0; found = np.array([])
    def solve_tau(found):
        lo, hi = mu.min() - 6, mu.max() + 6
        rem = ~done
        for _ in range(40):
            mid = (lo + hi) / 2
            cnt = (found >= mid).sum() + NROW * sf((mid - mu[rem]) / sg[rem]).sum()
            lo, hi = (mid, hi) if cnt > KNN else (lo, mid)
        return (lo + hi) / 2
    tau = tau_true if mode == "true" else solve_tau(found)
    steps = 0
    while not done.all():
        if mode == "running":
            tau = np.sort(found)[-KNN] if len(found) >= KNN else -np.inf
        elif mode == "estimate":
            tau = solve_tau(found)
        v = NROW * sf((tau - mu) / sg) if np.isfinite(tau) else np.full(Kc, float(NROW))
        idx, i = plan(v, opened, done, lam)
        if i is None or idx <= lam:
            break
        g = grp_of[i]
        if not opened[g]:
            opened[g] = True; cost += C_GRP
        done[i] = True; cost += C_CL + C_ROW * NROW
        found = np.concatenate([found, scores_top[i]]); steps += 1
    rec = (found >= tau_true).sum() / KNN
    return rec, cost


NQ = 200
data = []
for q in range(NQ):
    mu = np.repeat(rng.normal(0, 0.8, G_), CPG) + rng.normal(0, 0.6, Kc)
    sg = 0.3 * np.exp(rng.normal(0, 0.3, Kc))
    top = np.sort(mu[:, None] + sg[:, None] * rng.normal(size=(Kc, NROW)), axis=1)[:, -30:]
    tau_true = np.sort(top.ravel())[-KNN]
    data.append((mu, sg, top, tau_true))
for lam in (3e-3, 1e-3, 3e-4, 1e-4):
    line = f"   lambda {lam:g} answers per microsecond:"
    for mode in ("true", "plugin", "estimate", "running"):
        res = np.array([run(mode, lam, *d) for d in data])
        line += f"  {mode}: {res[:, 0].mean():.3f} | {res[:, 1].mean() / 1000:.3f}"
    print(line, flush=True)
