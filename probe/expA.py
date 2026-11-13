# Experiment A: coarse structure for probing. Recall@10 against the share of items scanned when the
# scan of each probed cluster is exact, so only the choice of clusters is tested.
#
# usage: expA.py <hdf5> <outdir> K [K ...]
# For each K: plain k-means (faiss) and balanced k-means (cap 2x mean), each with and without a
# SOAR second assignment, under several cluster orderings; queries are all test queries plus
# 2,000 corpus items as pseudo-queries (own GT, self excluded) used only to choose the bound factor.
import sys, time, json, numpy as np, h5py, faiss

path, outdir, Ks = sys.argv[1], sys.argv[2], [int(x) for x in sys.argv[3:]]
name = path.split('/')[-1].replace('.hdf5', '')
faiss.omp_set_num_threads(1)
k = 10; EPS = 1e-3; L = 16; SOAR_LAMBDA = 1.0; NPSEUDO = 2000
F_GRID = (0.1, 0.2, 0.35, 0.5, 0.75)
TARGETS = (0.9, 0.95, 0.99)
t00 = time.time()
def log(*a): print(f'[{time.time() - t00:7.0f}s]', *a, flush=True)

f = h5py.File(path, 'r')
X = np.asarray(f['train'], dtype=np.float32); X /= np.maximum(np.linalg.norm(X, axis=1, keepdims=True), 1e-12)
Q = np.asarray(f['test'], dtype=np.float32); Q /= np.maximum(np.linalg.norm(Q, axis=1, keepdims=True), 1e-12)
gt_ids = np.asarray(f['neighbors']); gt_d = np.asarray(f['distances'])
N, n = X.shape; NQ = len(Q)

# Test-query sets. ANN-Benchmarks: a returned item counts if 1 - cos <= d_k + 1e-3 (d_k from the file).
thr = gt_d[:, k - 1] + EPS
tol_mask = gt_d <= thr[:, None]
T_test = np.where(tol_mask, gt_ids, -1)
log(f'{name}: N={N} n={n} NQ={NQ}; tolerance set size mean {tol_mask.sum(1).mean():.2f}, '
    f'queries whose tolerance set fills all 100 listed neighbours: {(tol_mask.sum(1) == 100).sum()}')
G_test = gt_ids[:, :k]

# Pseudo-queries: corpus items, exact ground truth over the rest of the corpus.
rng = np.random.default_rng(5)
nz = np.flatnonzero(np.linalg.norm(X, axis=1) > 0.5)
pq_ids = rng.choice(nz, NPSEUDO, replace=False)
P = X[pq_ids]
T_ps = np.full((NPSEUDO, 100), -1); G_ps = np.zeros((NPSEUDO, k), dtype=np.int64)
for i in range(0, NPSEUDO, 100):
    S = P[i:i + 100] @ X.T
    S[np.arange(len(S)), pq_ids[i:i + 100]] = -9
    top = np.argpartition(-S, 101, axis=1)[:, :101]
    ts = np.take_along_axis(S, top, 1); o = np.argsort(-ts, 1); top = np.take_along_axis(top, o, 1); ts = np.take_along_axis(ts, o, 1)
    G_ps[i:i + 100] = top[:, :k]
    ok = ts[:, :100] >= ts[:, k - 1:k] - EPS
    T_ps[i:i + 100] = np.where(ok, top[:, :100], -1)
log('pseudo-query ground truth done')

def kmeans(K):
    km = faiss.Kmeans(n, K, niter=15, seed=1, verbose=False, max_points_per_centroid=256)
    km.train(X)
    return km.centroids.copy()

def topL(C):
    idx = faiss.IndexFlatL2(n); idx.add(C)
    D, I = idx.search(X, L)
    return D, I

def capped_assign(D, I, C, cap, Y=None):
    Y = X if Y is None else Y; K = len(C); M, Lc = I.shape
    ptr = np.zeros(M, dtype=np.int64); a = np.full(M, -1, dtype=np.int64); load = np.zeros(K, dtype=np.int64)
    active = np.arange(M)
    while active.size:
        c = I[active, ptr[active]]; d = D[active, ptr[active]]
        o = np.lexsort((d, c)); cs = c[o]
        rank = np.arange(len(cs)) - np.searchsorted(cs, cs, 'left')
        acc = rank < (cap - load[cs])
        a[active[o[acc]]] = cs[acc]; load += np.bincount(cs[acc], minlength=K)
        rej = active[o[~acc]]; ptr[rej] += 1
        done = rej[ptr[rej] >= Lc]
        for i in done:                      # no listed candidate has room: nearest centre with room
            free = np.flatnonzero(load < cap)
            j = free[np.argmin(((C[free] - Y[i]) ** 2).sum(1))]; a[i] = j; load[j] += 1
        active = rej[ptr[rej] < Lc]
    return a

def balanced(C0, rounds=5):
    K = len(C0); cap = int(np.ceil(2 * N / K)); C = C0.copy()
    for r in range(rounds):
        D, I = topL(C)
        a = capped_assign(D, I, C, cap)
        sums = np.zeros_like(C); np.add.at(sums, a, X); cnt = np.bincount(a, minlength=K)
        C[cnt > 0] = sums[cnt > 0] / cnt[cnt > 0, None]
    D, I = topL(C)
    a = capped_assign(D, I, C, cap)
    return C, a, D, I

def soar(C, a, I):
    a2 = np.empty(N, dtype=np.int64)
    for i in range(0, N, 20000):
        x = X[i:i + 20000]; r = x - C[a[i:i + 20000]]; rr = (r * r).sum(1)
        cand = I[i:i + 20000]
        Cc = C[cand]                                         # B x L x n
        diff = x[:, None, :] - Cc
        cost = (diff * diff).sum(2) + SOAR_LAMBDA * np.einsum('bln,bn->bl', diff, r) ** 2 / np.maximum(rr, 1e-12)[:, None]
        cost[cand == a[i:i + 20000, None]] = np.inf
        a2[i:i + 20000] = cand[np.arange(len(cand)), np.argmin(cost, 1)]
    return a2

def sizes_stats(a, K):
    s = np.bincount(a, minlength=K); ss = np.sort(s)[::-1]
    return dict(max_over_mean=float(s.max() / s.mean()), cv=float(s.std() / s.mean()), empty=int((s == 0).sum()),
                share_in_top10pct=float(ss[:K // 10].sum() / N))

def curves(keyfn, Qs, T, G, a1, a2, K, B=16):
    """Sum over queries, at every number of probed clusters p = 1..K, of: tolerant recall, strict recall,
    unique items scanned, codes scanned (an item with two assignments counted twice)."""
    rt = np.zeros(K + 1); rs = np.zeros(K + 1); su = np.zeros(K); sc = np.zeros(K)
    cnt1 = np.bincount(a1, minlength=K); ctot = cnt1 + (np.bincount(a2, minlength=K) if a2 is not None else 0)
    dt = np.int16 if K < 32000 else np.int32
    for i in range(0, len(Qs), B):
        key = keyfn(Qs[i:i + B]); reach = None
        if isinstance(key, tuple): key, reach = key
        b = len(key)
        order = np.argsort(-key, axis=1, kind='stable')
        rank = np.empty((b, K), dtype=dt); np.put_along_axis(rank, order, np.arange(K, dtype=dt)[None, :].repeat(b, 0), 1)
        if reach is not None: rank[~reach] = K            # unreachable clusters are never probed
        live = np.ones((b, K), bool) if reach is None else np.take_along_axis(reach, order, 1)
        def minrank(ids):
            r = np.take_along_axis(rank, a1[np.maximum(ids, 0)].reshape(b, -1), 1).astype(np.int64)
            if a2 is not None:
                r = np.minimum(r, np.take_along_axis(rank, a2[np.maximum(ids, 0)].reshape(b, -1), 1))
            return np.where(ids >= 0, r, K)
        mt = np.sort(minrank(T[i:i + B]), 1)[:, :k]
        rt += np.bincount((mt + 1).ravel(), minlength=K + 2)[:K + 1]   # probing p clusters covers rank < p
        ms = minrank(G[i:i + B])
        rs += np.bincount((ms + 1).ravel(), minlength=K + 2)[:K + 1]
        sc += np.cumsum(np.take_along_axis(np.broadcast_to(ctot, (b, K)), order, 1) * live, 1).sum(0)
        if a2 is None:
            su += np.cumsum(np.take_along_axis(np.broadcast_to(cnt1, (b, K)), order, 1) * live, 1).sum(0)
        else:
            for j in range(b):
                mr = np.minimum(rank[j][a1], rank[j][a2])
                su += np.cumsum(np.bincount(mr, minlength=K + 1)[:K])
    nq = len(Qs)
    return dict(rt=np.cumsum(rt)[1:] / nq / k, rs=np.cumsum(rs)[1:] / nq / k, su=su / nq, sc=sc / nq)

def ops(c):
    out = {}
    for R in TARGETS:
        for kind in ('rt', 'rs'):
            hit = np.flatnonzero(c[kind] >= R)
            if len(hit):
                p = int(hit[0]); out[f'{kind}{R}'] = dict(nprobe=p + 1, scanned=float(c['su'][p]), share=float(c['su'][p] / N), codes=float(c['sc'][p]))
            else:
                out[f'{kind}{R}'] = None
    return out

def thin(c, K):
    ps = np.unique(np.round(np.geomspace(1, K, 120)).astype(int)) - 1
    return {key: [round(float(v), 6) for v in c[key][ps]] for key in c} | dict(nprobe=(ps + 1).tolist())

results = {}
for K in Ks:
    log(f'K={K}: k-means'); C0 = kmeans(K)
    D0, I0 = topL(C0); a0 = I0[:, 0].copy()
    log('balanced k-means'); Cb, ab, Db, Ib = balanced(C0)
    log('SOAR'); s0 = soar(C0, a0, I0); sb = soar(Cb, ab, Ib)
    log('structures built')
    res = results[K] = dict(sizes=dict(plain=sizes_stats(a0, K), balanced=sizes_stats(ab, K),
                                       soar_second_same_as_second_nearest_plain=float((s0 == I0[:, 1]).mean())), runs={})
    for base, C, a, a2, I in (('plain', C0, a0, s0, I0), ('balanced', Cb, ab, sb, Ib)):
        for soar_on in (False, True):
            A2 = a2 if soar_on else None
            members = np.concatenate([a, A2]) if soar_on else a
            mem_ids = np.concatenate([np.arange(N), np.arange(N)]) if soar_on else np.arange(N)
            res2 = ((X[mem_ids] - C[members]) ** 2).sum(1)
            cnt = np.bincount(members, minlength=K)
            rad = np.sqrt(np.bincount(members, weights=res2, minlength=K) / np.maximum(cnt, 1))
            cn2 = (C * C).sum(1)
            orderings = {'centre': lambda q: q @ C.T, 'l2': lambda q: q @ C.T - 0.5 * cn2}
            for fct in F_GRID:
                orderings[f'bound{fct}'] = (lambda fct: lambda q: q @ C.T + fct * rad)(fct)
            tag = f'{base}{"+soar" if soar_on else ""}'
            # pseudo-queries choose the bound factor; SOAR runs reuse the factor chosen without SOAR
            if not soar_on:
                best = None
                for oname, fn in orderings.items():
                    cp = curves(fn, P, T_ps, G_ps, a, None, K)
                    o = ops(cp)['rt0.95']; sh = o['share'] if o else 9
                    res['runs'].setdefault(f'{tag}/{oname}', {})['pseudo'] = ops(cp)
                    if oname.startswith('bound') and (best is None or sh < best[0]): best = (sh, oname)
                chosen = best[1]; res[f'chosen_bound_{base}'] = chosen
                run_names = list(orderings)
            else:
                run_names = ['centre', 'l2', chosen]
            for oname in run_names:
                ct = curves(orderings[oname], Q, T_test, G_test, a, A2, K)
                r = res['runs'].setdefault(f'{tag}/{oname}', {})
                r['test'] = ops(ct); r['curve'] = thin(ct, K)
                if soar_on:
                    cp = curves(orderings[oname], P, T_ps, G_ps, a, A2, K); r['pseudo'] = ops(cp)
                o = r['test']
                log(f'K={K} {tag:14s} {oname:10s} ' + '  '.join(
                    f'R{R}: ' + (f"p={o[f'rt{R}']['nprobe']} share={o[f'rt{R}']['share']:.4f}" if o[f'rt{R}'] else 'n/a') for R in TARGETS))
    # Centre-ranking cost: flat pass over K centres against a two-level tree over the plain k-means
    # centres: sqrt(K) groups by k-means on the centres, group size capped at 2x the mean, groups
    # ranked by q . group mean, centres of the top t groups ranked by q . centre, others never probed.
    Gn = int(round(np.sqrt(K)))
    kmc = faiss.Kmeans(n, Gn, niter=20, seed=2, max_points_per_centroid=10_000); kmc.train(C0)
    GC = kmc.centroids.copy()
    for r in range(4):
        Dg, Ig = kmc.index.search(C0, min(Gn, 16)) if r == 0 else gidx.search(C0, min(Gn, 16))
        gc = capped_assign(Dg, Ig, GC, int(np.ceil(2 * K / Gn)), Y=C0)
        GC = np.stack([C0[gc == g].mean(0) if (gc == g).any() else GC[g] for g in range(Gn)]).astype(np.float32)
        gidx = faiss.IndexFlatL2(n); gidx.add(GC)
    gsz = np.bincount(gc, minlength=Gn)
    tree = res['tree'] = dict(groups=Gn, flat_madds=K * n, group_size_max=int(gsz.max()), rows=[])
    for t in (1, 2, 4, 8, 16, 32, 64):
        if t > Gn: break
        def fn(q, t=t):
            gs = q @ GC.T; topg = np.argpartition(-gs, t - 1, axis=1)[:, :t]
            ing = np.zeros((len(q), Gn), bool); np.put_along_axis(ing, topg, True, 1)
            cand = ing[:, gc]
            fn.cand += cand.sum()
            return np.where(cand, q @ C0.T, -np.inf), cand
        fn.cand = 0
        ct = curves(fn, Q, T_test, G_test, a0, None, K)
        mean_cand = fn.cand / NQ
        o = ops(ct)
        tree['rows'].append(dict(t=t, max_recall=float(ct['rt'][-1]), max_recall_strict=float(ct['rs'][-1]), scanned_at_max=float(ct['su'][-1]), mean_centres_scored=float(mean_cand), madds=float((Gn + mean_cand) * n), test=o))
        log(f'tree t={t}: centres scored {mean_cand:.0f} of {K}; max recall {ct["rt"][-1]:.4f}; ' + '  '.join(
            f'R{R}: ' + (f"p={o[f'rt{R}']['nprobe']}" if o[f'rt{R}'] else 'n/a') for R in TARGETS))
    json.dump(results, open(f'{outdir}/expA_{name}.json', 'w'))
log('done')
