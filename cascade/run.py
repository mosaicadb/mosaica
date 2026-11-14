# The exact k-NN cascade, k = 10, every cluster scanned, on queries [lo, hi) of one dataset.
# Per query and configuration:
#   seed: the 30 best level-1 (1-bit) score estimates among visible items are scored exactly; tau = k-th best.
#   scan: clusters in decreasing q.c_j, items in id order within a cluster. A non-seed item passes level l when
#   its upper bound at every level up to l is >= tau; an item passing level 3 is scored exactly, and tau becomes
#   the k-th best exact score seen. Answer = top k of the exactly scored items.
# Upper bounds (lvl0b.py's formulas):
#   plain: q.c + f<obar,Pq> + c sd
#   thmB:  q.c + alpha rho + t_hi, t = f<obar,Pq> + alpha (e_c - rho), kappa = c sd/|r|,
#          R = |r| w; t_hi = R when t >= R; otherwise min(R, (t + kappa sqrt((1 + kappa^2) R^2 - t^2)) / (1 + kappa^2)),
#          and the item is kept (t_hi = inf) when the root is not real
# Counts are exact: the per-item bounds are computed for every item; tau's steps come from a sequential pass
# over the items whose bounds pass the seed threshold, which is a superset of the exactly scored items.
# usage: run.py <name> <workdir> <lo> <hi> <out.npz>
import sys, time, heapq, numpy as np
name, wd, lo, hi, outp = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), sys.argv[5]
k = 10; NSEED = 30; B = 8; ZQ = 1000
SLACK = 1e-5   # float32 rounding between a bound and the exact score (items equal to their centre have width 0)
t0 = time.time()
def log(*a): print(f'[{time.time()-t0:5.0f}s]', *a, flush=True)
def unit(A): A = A.astype(np.float32); return A / np.maximum(np.linalg.norm(A, axis=1, keepdims=True), 1e-12)
L = lambda key: np.load(f'{wd}/{key}.npy', mmap_mode='r')
X = L('X'); a = np.asarray(L('a')); counts = np.asarray(L('counts')); C = np.asarray(L('C')); Rot = np.asarray(L('Rot'))
rn = np.asarray(L('rn')); cr = np.asarray(L('cr')); order = np.asarray(L('order'))
N, n = X.shape; K = len(C); cn2 = (C * C).sum(1)
rsafe = np.maximum(rn, 1e-12)
CODES = ('c1', 'c2', 'c4n', 'c4i', 'c2t')
CD = {c: L(c) for c in CODES}
FAC = {c: np.asarray(L(c + '_fac')) for c in CODES}; SD = {c: np.asarray(L(c + '_sd')) for c in CODES}
ECR = {c: np.asarray(L(c + '_ec')) - cr for c in CODES}
import h5py
Q = unit(np.asarray(h5py.File(f'/home/joe/code/vexact-data/{name}/{name}.hdf5', 'r')['test'])[lo:hi])
GT = np.asarray(np.load(f'/home/joe/code/vexact-data/{name}/neighbors.npy', mmap_mode='r')[lo:hi, :k])
inv = np.empty(N, np.int64); inv[order] = np.arange(N)
CKL, CN = {100: (5.99, 7.48), 256: (6.37, 8.36)}[n]
NEST = ('c1', 'c2', 'c4n'); NESTT = ('c1', 'c2t', 'c4i'); INDEP = ('c1', 'c2', 'c4i')
CFG = [('thmB_ckL_nested', 'thmB', CKL, NEST), ('thmB_ckL_nested_tied4', 'thmB', CKL, NESTT), ('thmB_ckL_indep', 'thmB', CKL, INDEP),
       ('thmB_cN_nested', 'thmB', CN, NEST), ('thmB_9.35_nested', 'thmB', 9.35, NEST),
       ('plain_ckL_nested', 'plain', CKL, NEST), ('plain_cN_nested', 'plain', CN, NEST), ('plain_9.35_nested', 'plain', 9.35, NEST)]
FILT = [('vis10_thmB_ckL_nested', 0.10), ('vis1_thmB_ckL_nested', 0.01)]
FIELDS = ['s1', 's2', 's3', 'full', 'differ', 'nbfail_any', 'nbfail_l1', 'nbfail_l2', 'nbfail_l3', 'tau0_is_sk', 'nvis']
res = {c[0]: np.zeros((len(Q), len(FIELDS)), np.int64) for c in CFG + [(f[0],) for f in FILT]}
ZF = ['pairs', 'abs3', 'abs4', 'abs5', 'pos3', 'pos4', 'pos5']
zc = {c: np.zeros(len(ZF), np.int64) for c in CODES}; zmax = {c: 0.0 for c in CODES}
gt_overlap = 0

def cascade(Sv, e1, ubs, keyv, out):
    """Sv, e1, ubs[0..2], keyv: exact scores, level-1 estimates, upper bounds per level, scan keys over the
    visible items. Fills out[FIELDS]."""
    nv = len(Sv)
    seed = np.argpartition(-e1, NSEED - 1)[:NSEED]
    top_true = np.argpartition(-Sv, k - 1)[:k]; sk = Sv[top_true].min()
    heap = list(np.sort(Sv[seed])[-k:]); heapq.heapify(heap); tau0 = heap[0]
    M = [ubs[0], np.minimum(ubs[0], ubs[1])]; M.append(np.minimum(M[1], ubs[2]))
    isseed = np.zeros(nv, bool); isseed[seed] = True
    cand = np.flatnonzero((M[2] >= tau0) & ~isseed)
    cand = cand[np.argsort(keyv[cand], kind='stable')]
    chg_k = []; chg_t = [tau0]; scored = list(seed); tau = tau0
    m3 = M[2][cand]; sc = Sv[cand]
    for j in range(len(cand)):
        if m3[j] >= tau:
            scored.append(cand[j])
            if sc[j] > heap[0]:
                heapq.heapreplace(heap, sc[j])
                if heap[0] > tau: tau = heap[0]; chg_k.append(keyv[cand[j]]); chg_t.append(tau)
    chg_k = np.array(chg_k, np.int64); chg_t = np.array(chg_t, np.float32)
    for l in range(3):
        idx = np.flatnonzero((M[l] >= tau0) & ~isseed)
        tl = chg_t[np.searchsorted(chg_k, keyv[idx], 'left')]
        out[l] = int((M[l][idx] >= tl).sum())
    nfull = len(scored) - NSEED
    assert nfull == out[2], (nfull, out[2])
    out[3] = len(scored)
    scored = np.array(scored)
    got = np.sort(Sv[scored])[-k:]; want = np.sort(Sv[top_true])
    out[4] = int(not np.array_equal(got, want))
    f = np.stack([ubs[l][top_true] < Sv[top_true] for l in range(3)])
    out[5] = int(f.any(0).sum()); out[6:9] = f.sum(1)
    out[9] = int(tau0 == sk); out[10] = nv

for i in range(0, len(Q), B):
    q = Q[i:i + B]; nb = len(q); gi = lo + i
    S = q @ X.T
    PQ = q @ Rot.T
    qc = q @ C.T; al = qc / cn2[None, :]; w = np.sqrt(np.maximum(1 - qc * qc / cn2[None, :], 0)).astype(np.float32)
    qca = np.repeat(qc, counts, axis=1); ala = np.repeat(al, counts, axis=1); wa = np.repeat(w, counts, axis=1)
    base = qca + ala * cr[None, :]; RW = rn[None, :] * wa; RW2 = RW * RW
    T = {c: (PQ @ np.asarray(CD[c]).T) * FAC[c][None, :] for c in CODES}
    Tc = {c: T[c] + ala * ECR[c][None, :] for c in CODES}
    crank = np.argsort(np.argsort(-qc, axis=1, kind='stable'), axis=1, kind='stable')
    keys = [np.repeat(crank[j], counts).astype(np.int64) * N + np.arange(N) for j in range(nb)]
    for j in range(nb):
        gt_overlap += len(np.intersect1d(inv[GT[i + j]], np.argpartition(-S[j], k - 1)[:k]))
    if gi < ZQ:
        tt = S - base
        dperp = np.sqrt(np.maximum(wa * wa - (tt / rsafe[None, :]) ** 2, 1e-12))
        for c in CODES:
            ok = (rn > 1e-6) & (SD[c] > 0)
            z = ((tt - Tc[c]) / (SD[c][None, :] * dperp))[:, ok][:max(0, min(nb, ZQ - gi))]
            az = np.abs(z)
            zc[c] += np.array([z.size, (az > 3).sum(), (az > 4).sum(), (az > 5).sum(), (z > 3).sum(), (z > 4).sum(), (z > 5).sum()])
            zmax[c] = max(zmax[c], float(az.max()))
        del tt, dperp
    for cname, var, c, codes in CFG:
        ubs = []
        for cd in codes:
            if var == 'plain':
                ubs.append(qca + T[cd] + c * SD[cd][None, :] + SLACK)
            else:
                kap = (c * SD[cd] / rsafe)[None, :]; t = Tc[cd]; k2 = 1 + kap * kap; disc = k2 * RW2 - t * t
                thi = np.minimum((t + kap * np.sqrt(np.maximum(disc, 0))) / k2, RW)
                thi = np.where(t >= RW, RW, np.where(disc < 0, np.inf, thi))
                ubs.append(base + thi + SLACK)
        e1 = qca + T['c1'] if var == 'plain' else base + Tc['c1']
        for j in range(nb):
            cascade(S[j], e1[j], [u[j] for u in ubs], keys[j], res[cname][i + j])
        if cname == 'thmB_ckL_nested':
            for fname, frac in FILT:
                for j in range(nb):
                    rng = np.random.default_rng([int(round(1 / frac)), gi + j])
                    vis = np.flatnonzero(rng.random(N) < frac)
                    cascade(S[j][vis], e1[j][vis], [u[j][vis] for u in ubs], keys[j][vis], res[fname][i + j])
        del ubs
    if (i // B) % 25 == 0: log(f'{gi + nb} / {hi}')
np.savez(outp, fields=np.array(FIELDS), zfields=np.array(ZF), gt_overlap=gt_overlap, nq=len(Q),
         **{'r_' + kk: v for kk, v in res.items()}, **{'z_' + kk: v for kk, v in zc.items()}, **{'zmax_' + kk: v for kk, v in zmax.items()})
log('done', outp)
