# Synthetic checks for hierarchy/theory.md (no real data):
#   1. the beam law (2.2): nodes that must be kept per level of a tree under the additive Gaussian model;
#   2. the node index (3.2): on small random two-level instances, the policy "always work on the available item of
#      largest index" (leaf: g/w; unexpanded node: the root nu of E sum (g_c - nu w_c)+ = nu e) attains the optimum
#      of the area objective found by exhaustive dynamic programming, and the threshold rule attains the optimum of the
#      Lagrangian objective.
#   python3 model.py
import itertools, numpy as np
from functools import lru_cache
from statistics import NormalDist
ND = NormalDist()
rng = np.random.default_rng(3)

print("1. Beam law. Tree of depth D, fan-out F, leaf score = sum over levels of independent N(0, share_l); targets = "
      "the top nprobe leaves; node key = its partial sum. Nodes needed per level to hold 99% of the targets' ancestors:")
for F, shares, nprobe in ((28, (1 / 3, 1 / 3, 1 / 3), 64), (28, (0.15, 0.25, 0.6), 64), (28, (0.6, 0.25, 0.15), 64), (28, (0.05, 0.15, 0.8), 64),
                          (150, (0.3, 0.7), 64), (150, (0.1, 0.9), 64)):
    D = len(shares); Kl = F ** D
    ranks = [[] for _ in range(D - 1)]
    for trial in range(12):
        part = [rng.normal(0, np.sqrt(shares[0]), F)]
        for l in range(1, D):
            part.append(np.repeat(part[-1], F) + rng.normal(0, np.sqrt(shares[l]), F ** (l + 1)))
        top = np.argpartition(-part[-1], nprobe)[:nprobe]
        for l in range(D - 1):
            anc = top // F ** (D - 1 - l)
            rk = np.argsort(np.argsort(-part[l]))
            ranks[l].append(rk[anc])
    zT = ND.inv_cdf(1 - nprobe / Kl); ze = ND.inv_cdf(0.99)
    line = f"   F {F}, shares {tuple(round(s, 2) for s in shares)}, K {Kl}, nprobe {nprobe}:"
    for l in range(D - 1):
        f = sum(shares[:l + 1]); nodes = F ** (l + 1)
        meas = np.quantile(np.concatenate(ranks[l]), 0.99) + 1
        pred = nodes * (1 - ND.cdf(zT * np.sqrt(f) - ze * np.sqrt(1 - f)))
        line += f"  level {l + 1} ({nodes} nodes, f {f:.2f}): measured {meas:.0f}, predicted {pred:.0f}"
    print(line)

print("\n2. Node index. Three nodes, two children each; a child's (g, w) is drawn independently from a node-specific law "
      "on three types; expanding node j costs e_j and reveals its children.")


def node_index(types, probs, e, nch=2):
    """nu solving E sum_c (g_c - nu w_c)+ = nu e, for nch iid children: bisection."""
    lo, hi = 0.0, max(g / w for g, w in types) + 1e-9
    for _ in range(80):
        mid = (lo + hi) / 2
        v = nch * sum(p * max(g - mid * w, 0) for (g, w), p in zip(types, probs)) - mid * e
        lo, hi = (mid, hi) if v > 0 else (lo, mid)
    return lo


def solve(inst):
    nodes = inst  # list of (types, probs, e)
    exp_g = [2 * sum(p * g for (g, w), p in zip(t, pr)) for t, pr, e in nodes]

    @lru_cache(None)
    def opt(state):
        # state per node: None (unexpanded) or a tuple of remaining children type ids
        rem = sum(exp_g[j] if s is None else sum(nodes[j][0][c][0] for c in s) for j, s in enumerate(state))
        if rem < 1e-15 and all(s is not None for s in state):
            return 0.0
        best = np.inf
        for j, s in enumerate(state):
            t, pr, e = nodes[j]
            if s is None:
                v = e * rem
                for c1, c2 in itertools.product(range(len(t)), repeat=2):
                    ns = state[:j] + (tuple(sorted((c1, c2))),) + state[j + 1:]
                    v += pr[c1] * pr[c2] * opt(ns)
                best = min(best, v)
            else:
                for c in set(s):
                    l = list(s); l.remove(c)
                    best = min(best, t[c][1] * rem + opt(state[:j] + (tuple(l),) + state[j + 1:]))
        if best == np.inf:
            return 0.0
        return best

    nu_true = [node_index(t, pr, e) for t, pr, e in nodes]
    # the whole-node density (the node taken as one big cluster), for comparison
    nu_bar = [2 * sum(p * g for (g, w), p in zip(t, pr)) / (e + 2 * sum(p * w for (g, w), p in zip(t, pr))) for t, pr, e in nodes]

    @lru_cache(None)
    def idx(state, which=0):
        nu = nu_true if which == 0 else nu_bar
        rem = sum(exp_g[j] if s is None else sum(nodes[j][0][c][0] for c in s) for j, s in enumerate(state))
        cand = []
        for j, s in enumerate(state):
            t, pr, e = nodes[j]
            if s is None:
                cand.append((nu[j], j, None))
            else:
                for c in set(s):
                    cand.append((t[c][0] / t[c][1], j, c))
        if not cand:
            return 0.0
        _, j, c = max(cand)
        t, pr, e = nodes[j]
        if c is None:
            v = e * rem
            for c1, c2 in itertools.product(range(len(t)), repeat=2):
                v += pr[c1] * pr[c2] * idx(state[:j] + (tuple(sorted((c1, c2))),) + state[j + 1:], which)
            return v
        l = list(state[j]); l.remove(c)
        return t[c][1] * rem + idx(state[:j] + (tuple(l),) + state[j + 1:], which)

    start = (None,) * len(nodes)
    # Lagrangian objective at a few prices: optimum by dynamic programming against the threshold rule's closed form
    lag = []
    for lam in (0.05, 0.2, 0.5, 1.0):
        @lru_cache(None)
        def best(state, lam=lam):
            v = 0.0
            for j, s in enumerate(state):
                t, pr, e = nodes[j]
                if s is None:
                    a = -lam * e
                    for c1, c2 in itertools.product(range(len(t)), repeat=2):
                        a += pr[c1] * pr[c2] * best(state[:j] + (tuple(sorted((c1, c2))),) + state[j + 1:])
                    v = max(v, a)
                else:
                    for c in set(s):
                        l = list(s); l.remove(c)
                        v = max(v, t[c][0] - lam * t[c][1] + best(state[:j] + (tuple(l),) + state[j + 1:]))
            return v
        closed = sum(max(0.0, 2 * sum(p * max(g - lam * w, 0) for (g, w), p in zip(t, pr)) - lam * e) for t, pr, e in nodes)
        lag.append(abs(best(start) - closed))
    return opt(start), idx(start), max(lag), idx(start, 1)


worst = 0.0; worst_l = 0.0; gaps = []; gb = []
for trial in range(300):
    inst = []
    for j in range(3):
        types = [(float(rng.uniform(0, 1)), float(rng.uniform(0.2, 2))) for _ in range(3)]
        pr = rng.dirichlet(np.ones(3))
        inst.append((tuple(types), tuple(float(x) for x in pr), float(rng.uniform(0.05, 2))))
    o, i, l, ib = solve(tuple(inst))
    worst = max(worst, (i - o) / o); worst_l = max(worst_l, l); gaps.append((i - o) / o); gb.append((ib - o) / o)
print(f"   300 random instances: largest relative excess of the index policy over the optimum of the area objective {worst:.2e} "
      f"(mean {np.mean(gaps):.2e}); the same with the whole-node density as a node's index: largest {max(gb):.3f}, mean {np.mean(gb):.4f}, "
      f"above 1e-9 on {int((np.array(gb) > 1e-9).sum())} instances; largest gap between the Lagrangian optimum and the threshold rule's closed form {worst_l:.2e}")
