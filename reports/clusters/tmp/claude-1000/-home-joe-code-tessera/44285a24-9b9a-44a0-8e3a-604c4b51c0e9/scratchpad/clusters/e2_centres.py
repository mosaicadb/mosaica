# E2: cheaper centre scoring over a flat k-means structure (balanced + SOAR, key = q.c + 0.35 rad).
# Each method picks R candidate centres at some cost, scores those exactly, and probes them in exact
# key order. Reported: cost of the centre stage, and share of items scanned at recall targets.
#   A  prefix : first d coordinates in the centres' own principal basis, tail allowance m sd
#   A0 lossless: the same prefix with a Cauchy-Schwarz tail bound; counts centres not excluded
#   B  pq     : 4-bit product-quantised centres (M units), as a fast-scan pass would score them
#   C  hnsw   : a graph over the centres (inner product on [c, f rad])
#   D  tree2  : groups of centres with a SOAR second group per centre, beam over groups
#
# usage: e2_centres.py <dataset> K methods(comma list)
import sys, numpy as np, faiss
sys.path.insert(0, "/tmp/claude-1000/-home-joe-code-tessera/44285a24-9b9a-44a0-8e3a-604c4b51c0e9/scratchpad/clusters")
from common import *

name, K, methods = sys.argv[1], int(sys.argv[2]), sys.argv[3].split(',')
F = 0.35; NQ = 1000
faiss.omp_set_num_threads(4)
X, Q, T, G = load(name, NQ)
N, n = X.shape
z = np.load(f'{DATA}/{name}/probe/K{K}.npz'); C = z['Cb']; a1 = z['ab']; a2 = z['sb']
rad, cnt = cell_radius(X, C[a1], a1, K)
empty = cnt == 0
EK = Q @ C.T + F * rad[None, :]; EK[:, empty] = -np.inf          # exact keys, all queries
del X
log(f'{name} K={K} N={N} n={n} queries={len(Q)}; centre norm mean {np.linalg.norm(C, axis=1).mean():.3f}; key sd per query {EK[:, ~empty].std(1).mean():.3f}')

def evaluate(label, SEL, cost):
    """SEL: (nq, K) bool of centres scored exactly. Probing follows exact key order among them."""
    R = int(SEL.sum(1).max())
    qi = {id(Q): 0}
    pos = [0]
    def keys(Qb):
        s = pos[0]; pos[0] += len(Qb)
        return np.where(SEL[s:s + len(Qb)], EK[s:s + len(Qb)], -np.inf)
    rec, su = curves(keys, Q, T, a1, a2, K, cnt)
    rec = rec[:R]; su = su[:R]
    log(f'{label}: {fmt(at_targets(rec, su, N))}   cost {cost}')

def topR(key, R):
    SEL = np.zeros(key.shape, bool)
    idx = np.argpartition(-key, R - 1, axis=1)[:, :R]
    np.put_along_axis(SEL, idx, True, 1)
    return SEL

if 'flat' in methods:
    evaluate('flat (all centres exact)', np.ones((len(Q), K), bool), f'{K} centres x {n} dims = {K * n / 1e6:.2f}M mults')

if 'prefix' in methods or 'lossless' in methods:
    w, V = np.linalg.eigh(C.T @ C); V = V[:, ::-1]; w = w[::-1]
    Cr = C @ V; Qr = Q @ V
    log('centre energy in the first d principal coordinates: ' + ', '.join(f'd={d}: {w[:d].sum() / w.sum():.3f}' for d in (4, 8, 16, 32, 48, 64)))
    log('query energy in the same coordinates: ' + ', '.join(f'd={d}: {(Qr[:, :d] ** 2).sum(1).mean():.3f}' for d in (4, 8, 16, 32, 48, 64)))

if 'lossless' in methods:
    # centres whose Cauchy-Schwarz upper bound after d coordinates still reaches the p-th best exact key
    for p in (64, 256, 1024):
        thr = -np.sort(-EK, axis=1)[:, p - 1]
        out = []
        for d in (8, 16, 32, 48, 64, 80):
            ub = Qr[:, :d] @ Cr[:, :d].T + F * rad[None, :] + np.linalg.norm(Qr[:, d:], axis=1)[:, None] * np.linalg.norm(Cr[:, d:], axis=1)[None, :]
            surv = (ub >= thr[:, None]).sum(1).mean()
            out.append(f'd={d}: {surv:.0f} left ({(K * d + surv * (n - d)) / (K * n):.2f} of flat)')
        log(f'lossless prefix, top p={p}: ' + '; '.join(out))

if 'prefix' in methods:
    for d in (8, 16, 32):
        tq = np.linalg.norm(Qr[:, d:], axis=1); tc = np.linalg.norm(Cr[:, d:], axis=1)
        for m in (0.0, 2.0):
            key = Qr[:, :d] @ Cr[:, :d].T + F * rad[None, :] + m * tq[:, None] * tc[None, :] / np.sqrt(n - d)
            key[:, empty] = -np.inf
            for R in (512, 1024, 2048, 4096):
                if R >= K: continue
                evaluate(f'prefix d={d} m={m} R={R}', topR(key, R), f'{K}x{d} + {R}x{n - d} = {(K * d + R * (n - d)) / 1e6:.2f}M mults ({(K * d + R * (n - d)) / (K * n):.3f} of flat)')

if 'pq' in methods:
    for M in ([10, 25, 50] if n == 100 else [16, 32, 64]):
        pq = faiss.ProductQuantizer(n, M, 4); pq.train(C)
        Ch = pq.decode(pq.compute_codes(C))
        key = Q @ Ch.T + F * rad[None, :]; key[:, empty] = -np.inf
        err = (Q @ Ch.T - Q @ C.T).std()
        for R in (512, 1024, 2048, 4096):
            if R >= K: continue
            evaluate(f'pq 4-bit M={M} ({M / 2:.1f} B/centre, key error sd {err:.3f}) R={R}', topR(key, R), f'{K} codes x {M / 2:.1f} B + {R} exact')

if 'hnsw' in methods:
    A = np.hstack([C, F * rad[:, None]]).astype(np.float32); A[empty] = 0
    Qa = np.hstack([Q, np.ones((len(Q), 1), np.float32)])
    for Mh in (16, 32):
        ix = faiss.IndexHNSWFlat(n + 1, Mh, faiss.METRIC_INNER_PRODUCT); ix.hnsw.efConstruction = 200
        ix.add(A)
        faiss.omp_set_num_threads(1)
        for ef in (128, 256, 512, 1024, 2048):
            ix.hnsw.efSearch = ef
            faiss.cvar.hnsw_stats.reset()
            _, I = ix.search(Qa, ef)
            nd = faiss.cvar.hnsw_stats.ndis / len(Q)
            SEL = np.zeros((len(Q), K), bool); np.put_along_axis(SEL, np.maximum(I, 0), True, 1)
            got = np.mean([len(np.intersect1d(I[i], np.argpartition(-EK[i], 99)[:100])) for i in range(len(Q))])
            evaluate(f'hnsw M={Mh} ef={ef} (finds {got:.1f} of the true top 100 centres)', SEL, f'{nd:.0f} centre distances ({nd / K:.3f} of flat)')
        faiss.omp_set_num_threads(4)

if 'tree2' in methods:
    for Gn in (128,):
        Cg = kmeans(C, Gn, niter=25, mpc=100000)
        D, I = nearest(C, Cg, 8)
        g1 = I[:, 0]
        r = C - Cg[g1]; rr = np.maximum((r * r).sum(1), 1e-12)
        diff = C[:, None, :] - Cg[I]                              # K x 8 x n
        cst = (diff * diff).sum(2) + np.einsum('kln,kn->kl', diff, r) ** 2 / rr[:, None]
        cst[:, 0] = np.inf
        g2 = I[np.arange(K), cst.argmin(1)]
        grad = np.sqrt(np.bincount(g1, weights=rr, minlength=Gn) / np.maximum(np.bincount(g1, minlength=Gn), 1))
        gk = Q @ Cg.T
        for two in (False, True):
            for fg in (0.0, 0.5):
                order = np.argsort(-(gk + fg * grad[None, :]), axis=1)
                grank = np.empty_like(order); np.put_along_axis(grank, order, np.broadcast_to(np.arange(Gn), order.shape), 1)
                cr = grank[:, g1]
                if two: cr = np.minimum(cr, grank[:, g2])
                for beam in (Gn // 16, Gn // 8, Gn // 4, Gn // 2):
                    SEL = cr < beam
                    evaluate(f'tree2 G={Gn} {"two groups per centre" if two else "one group per centre"} fg={fg} beam={beam}', SEL, f'{Gn} + {SEL.sum(1).mean():.0f} centres ({(Gn + SEL.sum(1).mean()) / K:.3f} of flat)')
log('done')
