# E6: how many clusters an exact k-NN query could leave unopened, by kind of whole-cluster bound.
# tau = exact 10th best score. A cluster must be opened if its bound reaches tau.
#   oracle     : the true maximum of q.x over the cluster (no summary can do better)
#   ball       : q.m + |q| * max |x - m|
#   own cone   : unit vectors; with t = x.mhat in [lo, hi], q.x <= b t + sqrt(1-b^2) sqrt(1-t^2), b = q.mhat
#   near anchor: the same cone bound taken about the anchor (a cluster centre) nearest the query,
#                with the exact [lo, hi] of anchor.x over the cluster (the support table)
#   best anchor: the least such bound over every anchor (K bounds per cluster; a limit, not a design)
#   two anchors: least squares of q on (own centre, nearest anchor), extremes summed, remainder by norm
# Also the share of items whose own key (x.mhat alone) cannot exclude them.
#
# usage: e6_setbounds.py glove-100-angular K | nytimes-256-angular K | bioclip K
import sys, numpy as np, faiss
sys.path.insert(0, "/tmp/claude-1000/-home-joe-code-tessera/44285a24-9b9a-44a0-8e3a-604c4b51c0e9/scratchpad/clusters")
from common import *

name, K = sys.argv[1], int(sys.argv[2])
NQ = 300
faiss.omp_set_num_threads(4)
rng = np.random.default_rng(3)
if name == 'bioclip':
    X = np.concatenate([np.load(f'{DATA}/bioclip2-200k/g{i}.npy').astype(np.float32) for i in range(4)])
    X /= np.linalg.norm(X, axis=1, keepdims=True)
    qid = rng.choice(len(X), NQ, replace=False); Q = X[qid]
    C = kmeans(X, K, niter=20); _, I = nearest(X, C); a = I[:, 0]
else:
    X, Q, T, G = load(name, NQ); qid = None
    z = np.load(f'{DATA}/{name}/probe/K{K}.npz'); C = z['Cb']; a = z['ab']
N, n = X.shape
cnt = np.bincount(a, minlength=K); ne = cnt > 0
order = np.argsort(a, kind='stable'); st = np.concatenate([[0], np.cumsum(cnt)])[:-1]
stn = st[ne]
Ph = C / np.maximum(np.linalg.norm(C, axis=1, keepdims=True), 1e-12)
lo = np.full((K, K), 1.0, np.float32); hi = np.full((K, K), -1.0, np.float32)     # [anchor, cluster]
rmax = np.zeros(K)
Xo = X[order]; ao = a[order]
for s in range(0, K, 256):                      # anchors in slabs
    Tm = Xo @ Ph[s:s + 256].T                   # N x 256
    hi[s:s + 256][:, ne] = np.maximum.reduceat(Tm, stn, axis=0).T
    lo[s:s + 256][:, ne] = np.minimum.reduceat(Tm, stn, axis=0).T
    del Tm
rmax[ne] = np.maximum.reduceat(np.linalg.norm(Xo - C[ao], axis=1), stn)
own = (Xo * Ph[ao]).sum(1)                      # each item's key about its own centre
log(f'{name} N={N} n={n} K={K}: tables done; mean centre norm {np.linalg.norm(C, axis=1).mean():.3f}, mean max radius {rmax[ne].mean():.3f}')

def cone(b, lo_, hi_):
    r = np.sqrt(np.maximum(1 - b * b, 0))
    f = lambda t: b * t + r * np.sqrt(np.maximum(1 - t * t, 0))
    return np.where(hi_ < b, f(hi_), np.where(lo_ > b, f(lo_), 1.0))

res = {k_: [] for k_ in ('oracle', 'ball', 'own cone', 'near anchor', 'best anchor', 'two anchors', 'items by own key', 'items in opened (own cone)', 'items in opened (best anchor)')}
for qi in range(NQ):
    q = Q[qi]; S = Xo @ q
    if qid is not None: S[np.flatnonzero(order == qid[qi])] = -9
    tau = -np.partition(-S, k - 1)[k - 1]
    cmax = np.full(K, -9.0); cmax[ne] = np.maximum.reduceat(S, stn)
    b = Ph @ q                                   # query against every anchor
    ball = C @ q + rmax
    oc = cone(b, lo[np.arange(K), np.arange(K)], hi[np.arange(K), np.arange(K)])
    na = int(np.argmax(b)); nb = cone(b[na], lo[na], hi[na])
    ba = cone(b[:, None].astype(np.float32), lo, hi).min(0) if K <= 4096 else nb
    # two anchors: q = u p_c + v p_na + rest
    g = Ph @ Ph[na]; det = np.maximum(1 - g * g, 1e-9)
    u = (b - g * b[na]) / det; v = (b[na] - g * b) / det
    rest = np.sqrt(np.maximum(1 - (u * u + v * v + 2 * u * v * g), 0))
    dg = np.arange(K)
    ta = np.where(u >= 0, u * hi[dg, dg], u * lo[dg, dg]) + np.where(v >= 0, v * hi[na], v * lo[na]) + rest
    ta = np.where(det < 1e-6, oc, np.minimum(ta, 1.0))
    for nm, bd in (('oracle', cmax), ('ball', ball), ('own cone', oc), ('near anchor', nb), ('best anchor', ba), ('two anchors', np.minimum(ta, oc))):
        res[nm].append((bd[ne] >= tau).mean())
    bo = b[ao]
    res['items by own key'].append((bo * own + np.sqrt(np.maximum(1 - bo * bo, 0)) * np.sqrt(np.maximum(1 - own * own, 0)) >= tau).mean())
    res['items in opened (own cone)'].append(cnt[oc >= tau].sum() / N)
    res['items in opened (best anchor)'].append(cnt[np.minimum(ba, oc) >= tau].sum() / N)
for nm, v in res.items():
    log(f'{nm:>30}: {100 * np.mean(v):6.2f}%')
log('done')
