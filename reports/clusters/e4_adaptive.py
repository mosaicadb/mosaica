# E4: how many clusters to open, per query. Fixed probe count against a stopping rule that uses the
# running k-th best score: stop at the first cluster whose key (q.c + 0.35 rad) falls below
# (k-th best exact score so far) - delta. delta is fitted on corpus items used as queries (self
# excluded) and applied unchanged to the test queries, unfiltered and under random visible sets.
# Counts: tolerant recall@10 within the visible set, visible items scanned, non-empty clusters opened.
#
# usage: e4_adaptive.py <dataset> K
import sys, numpy as np, faiss
sys.path.insert(0, "/tmp/claude-1000/-home-joe-code-tessera/44285a24-9b9a-44a0-8e3a-604c4b51c0e9/scratchpad/clusters")
from common import *

name, K = sys.argv[1], int(sys.argv[2])
F = 0.35; NQ = 1000; NPS = 500; TOP = 200
X, Q, T, G = load(name, NQ)
N, n = X.shape
z = np.load(f'{DATA}/{name}/probe/K{K}.npz'); C = z['Cb']; a1 = z['ab']; a2 = z['sb']
rad, cnt = cell_radius(X, C[a1], a1, K)
rng = np.random.default_rng(11)
nz = np.flatnonzero(np.linalg.norm(X, axis=1) > 0.5)
ps_ids = rng.choice(nz, NPS, replace=False)
VIS = {'none': None, '10%': np.flatnonzero(rng.random(N) < 0.10), '1%': np.flatnonzero(rng.random(N) < 0.01)}
DELTAS = np.round(np.arange(-0.10, 0.6001, 0.01), 3)
log(f'{name} K={K} N={N} n={n}')

def run(Qs, self_ids, scen):
    """Per scenario: fixed-p sums (recall by p, items by p, clusters by p) and per-(query, delta) outcomes."""
    out = {s: dict(rt=np.zeros(K + 2), su=np.zeros(K), oc=np.zeros(K), rec=np.zeros((len(Qs), len(DELTAS))),
                   items=np.zeros((len(Qs), len(DELTAS))), cl=np.zeros((len(Qs), len(DELTAS))), stop=np.zeros((len(Qs), len(DELTAS)))) for s in scen}
    for qi, q in enumerate(Qs):
        S = X @ q
        if self_ids is not None: S[self_ids[qi]] = -9
        key = q @ C.T + F * rad; key[cnt == 0] = -np.inf
        order = np.argsort(-key, kind='stable'); rank = np.empty(K, np.int64); rank[order] = np.arange(K)
        ks = key[order]
        mr = np.minimum(rank[a1], rank[a2])
        for s in scen:
            idx = VIS[s]
            sv = S if idx is None else S[idx]; rv = mr if idx is None else mr[idx]
            top = np.argpartition(-sv, TOP)[:TOP]; top = top[np.argsort(-sv[top])]
            tol = top[sv[top] >= sv[top[k - 1]] - EPS]
            o = out[s]
            bc = np.bincount(rv, minlength=K)
            cum = np.cumsum(bc); occ = np.cumsum(bc > 0)
            tr = np.sort(rv[tol])[:k]
            o['rt'] += np.bincount(tr + 1, minlength=K + 2)[:K + 2]; o['su'] += cum; o['oc'] += occ
            # running k-th best over the top items in probing order
            oo = np.argsort(rv[top], kind='stable'); rs = rv[top][oo]; ss = sv[top][oo]
            best = []; tau = np.empty(TOP)
            for j in range(TOP):
                best.append(ss[j]); best.sort(reverse=True); del best[k:]
                tau[j] = best[-1] if len(best) == k else -np.inf
            j_of_p = np.searchsorted(rs, np.arange(K), 'left') - 1          # last top item with rank < p
            tau_p = np.where(j_of_p >= 0, tau[np.maximum(j_of_p, 0)], -np.inf)
            cond = ks[None, :] < tau_p[None, :] - DELTAS[:, None]
            cond[:, 0] = False
            stop = np.where(cond.any(1), cond.argmax(1), K)                 # clusters probed
            o['stop'][qi] = stop
            o['rec'][qi] = np.searchsorted(tr, stop, 'left') / k            # tolerant items with rank < stop
            o['items'][qi] = cum[np.maximum(stop - 1, 0)]; o['cl'][qi] = occ[np.maximum(stop - 1, 0)]
    return out

ps = run(X[ps_ids], ps_ids, list(VIS))
log('pseudo-queries done')
te = run(Q, None, list(VIS))
log('test queries done')
nvis = {s: (N if VIS[s] is None else len(VIS[s])) for s in VIS}
for t in (0.9, 0.95, 0.99):
    for s in VIS:
        o = te[s]; rec = np.cumsum(o['rt'])[1:K + 1] / len(Q) / k
        ph = int(np.searchsorted(rec, t)) + 1; pi = min(ph, K) - 1
        pr = ps[s]['rec'].mean(0); ok = np.flatnonzero(pr >= t); di = ok[0]
        pr0 = ps['none']['rec'].mean(0); d0 = np.flatnonzero(pr0 >= t)[0]
        rr = o['rec'].mean(0); hi = np.flatnonzero(rr >= t)[0]
        f = lambda j: f'delta {DELTAS[j]:+.2f} recall {rr[j]:.4f} items {o["items"][:, j].mean():.0f} ({100 * o["items"][:, j].mean() / nvis[s]:.2f}%) clusters {o["cl"][:, j].mean():.0f}'
        log(f'target {t} visible {s:>4}: fixed p (hindsight) p={ph} recall {rec[pi]:.4f} items {o["su"][pi] / len(Q):.0f} ({100 * o["su"][pi] / len(Q) / nvis[s]:.2f}%) clusters {o["oc"][pi] / len(Q):.0f}'
            f' || rule, delta fitted unfiltered: {f(d0)} || rule, delta fitted at this visibility on corpus items: {f(di)} (probes median {np.median(o["stop"][:, di]):.0f}, p90 {np.percentile(o["stop"][:, di], 90):.0f}) || rule, hindsight: {f(hi)}')
log('done')
