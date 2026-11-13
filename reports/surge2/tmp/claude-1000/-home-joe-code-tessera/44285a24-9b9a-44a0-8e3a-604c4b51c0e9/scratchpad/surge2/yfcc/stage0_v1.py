import sys, os, time, json
os.environ.update(OMP_NUM_THREADS='1', OPENBLAS_NUM_THREADS='1', MKL_NUM_THREADS='1')
import numpy as np, faiss
faiss.omp_set_num_threads(1)
T0 = time.time()
D_DIR, M_DIR, OUT, DEADLINE = sys.argv[1], sys.argv[2], sys.argv[3], float(sys.argv[4])
N = 1_000_000; d = 192; K = 10; NC = 1000; PS = [int(x) for x in os.environ.get('PS', '32,48,64').split(',')]; NF = len(PS)
def log(*a): print(f'[{time.time()-T0:6.1f}s]', *a, flush=True)
def isqrt(a):
    a = np.asarray(a, dtype=np.int64)
    r = np.floor(np.sqrt(a.astype(np.float64))).astype(np.int64)
    r -= (r * r > a); r += ((r + 1) * (r + 1) <= a)
    return r
def read_spmat(path, nrows=None):
    nrow, ncol, nnz = np.fromfile(path, dtype=np.int64, count=3)
    indptr = np.fromfile(path, dtype=np.int64, offset=24, count=(nrows or nrow) + 1)
    indices = np.fromfile(path, dtype=np.int32, offset=24 + 8 * (nrow + 1), count=int(indptr[-1]))
    return indptr, indices, int(ncol), int(nrow), int(nnz)

bh = np.fromfile(D_DIR + '/base.first1M.u8bin.part', dtype=np.uint32, count=2)
base = np.fromfile(D_DIR + '/base.first1M.u8bin.part', dtype=np.uint8, offset=8, count=N * d).reshape(N, d)
qh = np.fromfile(D_DIR + '/query.public.100K.u8bin', dtype=np.uint32, count=2)
Q = np.fromfile(D_DIR + '/query.public.100K.u8bin', dtype=np.uint8, offset=8).reshape(-1, d)
log('base header', bh, 'query header', qh, Q.shape)
bptr, bind, ncol, bn, bnnz = read_spmat(M_DIR + '/base.metadata.10M.spmat', N)
qptr, qind, qncol, qn_, qnnz = read_spmat(M_DIR + '/query.metadata.public.100K.spmat')
log('base meta', bn, ncol, bnnz, 'used nnz', len(bind), '| query meta', qn_, qncol, qnnz)
ncol = max(ncol, qncol)
rows = np.repeat(np.arange(N, dtype=np.int32), np.diff(bptr))
order = np.argsort(bind, kind='stable')
post_rows = rows[order]; del rows, order
tagcount = np.bincount(bind, minlength=ncol)
tstart = np.concatenate([[0], np.cumsum(tagcount)])
def posting(t): return post_rows[tstart[t]:tstart[t + 1]]
def visible(tags):
    if len(tags) == 1: return posting(tags[0])
    a, b = posting(tags[0]), posting(tags[1])
    for t in tags[2:]: raise SystemExit('more than two tags')
    if len(a) > len(b): a, b = b, a
    if len(a) == 0 or len(b) == 0: return a[:0]
    pos = np.searchsorted(b, a); pos[pos == len(b)] = len(b) - 1
    return a[b[pos] == a]
log('postings built')

# ---- variance profile, k-means
Bf = base.astype(np.float32)
mean = Bf.mean(0, dtype=np.float64)
var = (Bf.astype(np.float32) ** 2).mean(0, dtype=np.float64) - mean ** 2
perm = np.argsort(-var, kind='stable')
vs = var[perm]; tot = vs.sum()
res = {'variance': {'total': float(tot), 'max': float(vs[0]), 'median': float(np.median(vs)), 'min': float(vs[-1]),
       'share_top': {p: float(vs[:p].sum() / tot) for p in (8, 16, 32, 48, 64, 96, 128)},
       'uniform_share': {p: p / d for p in PS}}}
log('variance', json.dumps(res['variance']))
cpath = OUT + '/km_centroids.npy'
if os.path.exists(cpath):
    cent = np.load(cpath)
else:
    rng = np.random.default_rng(1)
    samp = np.sort(rng.choice(N, 256 * NC, replace=False))
    km = faiss.Kmeans(d, NC, niter=10, seed=1, verbose=False)
    km.train(Bf[samp])
    cent = km.centroids.copy(); np.save(cpath, cent)
log('kmeans trained')
mbar = np.clip(np.rint(cent), 0, 255).astype(np.float32)
ix = faiss.IndexFlatL2(d); ix.add(mbar)
assign = np.empty(N, np.int64)
for s in range(0, N, 100000):
    assign[s:s + 100000] = ix.search(Bf[s:s + 100000], 1)[1][:, 0]
csz = np.bincount(assign, minlength=NC)
log('assigned; cluster size min/median/max', csz.min(), int(np.median(csz)), csz.max(), 'empty', int((csz == 0).sum()))
Bp = np.ascontiguousarray(Bf[:, perm]); del Bf
mbp = np.ascontiguousarray(mbar[:, perm]); mb_i = mbp.astype(np.int32)
nu = np.empty(N, np.int64); xn = np.empty(N, np.int64)
b2 = {p: np.empty(N, np.int64) for p in PS}; xnh = {p: np.empty(N, np.int64) for p in PS}
resvar = np.zeros(d)
# (f) one-bit code over a global random rotation of the residual
rngP = np.random.default_rng(3)
P, _ = np.linalg.qr(rngP.standard_normal((d, d))); P = P.astype(np.float32)
S = np.empty((N, d), np.int8); sabs = np.empty(N, np.float32); spm = np.empty(N, np.float32)
Pm = mbp @ P.T
for s in range(0, N, 50000):
    e = min(N, s + 50000)
    xb = Bp[s:e]; a = assign[s:e]
    xi = xb.astype(np.int32); sq = xi * xi
    xn[s:e] = sq.sum(1)
    for p in PS: xnh[p][s:e] = sq[:, :p].sum(1)
    r = xi - mb_i[a]; sq = r * r
    nu[s:e] = sq.sum(1); resvar += sq.sum(0)
    for p in PS: b2[p][s:e] = sq[:, p:].sum(1)
    pr = r.astype(np.float32) @ P.T
    sg = np.where(pr >= 0, 1, -1).astype(np.int8)
    S[s:e] = sg; sabs[s:e] = np.abs(pr).sum(1); spm[s:e] = (sg * Pm[a]).sum(1)
R_lo = isqrt(nu); b_lo = {p: isqrt(b2[p]) for p in PS}
Rmax = np.full(NC, -1, np.int64); np.maximum.at(Rmax, assign, R_lo)
Rmin = np.full(NC, 1 << 40, np.int64); np.minimum.at(Rmin, assign, R_lo)
mn = (mb_i.astype(np.int64) ** 2).sum(1)
mnT = {p: (mb_i[:, p:].astype(np.int64) ** 2).sum(1) for p in PS}
res['residual'] = {'mean_nu': float(nu.mean()), 'mean_xn_centred': float(tot),
                   'residual_share_of_variance': float(resvar.sum() / N / tot),
                   'residual_share_in_head': {p: float(resvar[:p].sum() / resvar.sum()) for p in PS}}
log('precompute done', json.dumps(res['residual']))

# ---- choose queries
rng = np.random.default_rng(7)
nt = np.diff(qptr); first = qind[qptr[:-1]]
log('query tag counts', dict(zip(*np.unique(nt, return_counts=True))))
one = np.flatnonzero(nt == 1); V1 = tagcount[first[one]]
two = np.flatnonzero(nt == 2); two_s = rng.choice(two, min(3000, len(two)), replace=False)
V2 = np.array([len(visible(qind[qptr[i]:qptr[i + 1]])) for i in two_s])
edges = [10, 100, 1000, 10000, 50000, 1 << 40]
names = ['10-99', '100-999', '1k-10k', '10k-50k', '50k+']
sel = []
for b in range(5):
    c1 = one[(V1 >= edges[b]) & (V1 < edges[b + 1])]; c2 = two_s[(V2 >= edges[b]) & (V2 < edges[b + 1])]
    log(f'bucket {names[b]}: one-tag candidates {len(c1)} of {len(one)}, two-tag {len(c2)} of {len(two_s)} sampled')
    sel += list(rng.choice(c1, min(100, len(c1)), replace=False)) + list(rng.choice(c2, min(50, len(c2)), replace=False))
sel = np.array(sel); rng.shuffle(sel)
log('selected', len(sel))

# ---- per query
recs = []; checked = 0; mismatch = 0
MS = [2.0, 2.5, 3.0]
for n_done, qi in enumerate(sel):
    if time.time() > DEADLINE: log('deadline reached, stopping at', n_done); break
    tags = qind[qptr[qi]:qptr[qi + 1]]
    idx = visible(tags); V = len(idx)
    qp_i = Q[qi][perm].astype(np.int64); qp = qp_i.astype(np.float32)
    Qm = np.zeros((d, NF + 1), np.float32); lo = 0
    for j, hi in enumerate(PS + [d]):
        Qm[lo:hi, j] = qp[lo:hi]; lo = hi
    cd = np.rint(Bp[idx] @ Qm).astype(np.int64).cumsum(1)
    qsq = qp_i * qp_i; qn = int(qsq.sum())
    dist = qn + xn[idx] - 2 * cd[:, NF]
    if checked < 20:
        sub = idx[:3000]
        ref = ((base[sub][:, perm].astype(np.int64) - qp_i) ** 2).sum(1)
        mismatch += int((ref != dist[:len(sub)]).sum()); checked += 1
    tau = int(np.partition(dist, K - 1)[K - 1])
    within = dist <= tau
    cdots = np.rint(mbp @ Qm).astype(np.int64).cumsum(1)
    A2 = qn + mn - 2 * cdots[:, NF]
    A_lo = isqrt(A2); A_hi = A_lo + (A_lo * A_lo < A2)
    c = assign[idx]
    occ, cnt = np.unique(c, return_counts=True)
    o = np.argsort(A2[occ], kind='stable'); cs = cnt[o].cumsum()
    ncl = min(len(occ), max(2, int(np.searchsorted(cs, K)) + 1))
    m = np.isin(c, occ[o[:ncl]]); ds = dist[m]
    tau_seed = int(np.partition(ds, K - 1)[K - 1])
    gc = np.maximum(np.maximum(A_lo - (Rmax + 1), Rmin - A_hi), 0); lbc = (gc * gc)[c]
    Rl = R_lo[idx]
    g = np.maximum(np.maximum(A_lo[c] - (Rl + 1), Rl - A_hi[c]), 0); lb1 = g * g
    rec = {'q': int(qi), 'ntags': int(len(tags)), 'V': V, 'tau': tau, 'tau_seed': tau_seed, 'occ': int(len(occ)),
           'seed_clusters': ncl, 'seed_rows': int(m.sum()), 'n_within': int(within.sum())}
    lbs = {'cl': lbc, 'i1': lb1}; heads = {}
    for pi, p in enumerate(PS):
        qh_ = int(qsq[:p].sum())
        h = qh_ + xnh[p][idx] - 2 * cd[:, pi]
        a2 = (qn - qh_) + mnT[p] - 2 * (cdots[:, NF] - cdots[:, pi])
        a_lo = isqrt(a2); a_hi = a_lo + (a_lo * a_lo < a2)
        bl = b_lo[p][idx]
        gT = np.maximum(np.maximum(a_lo[c] - (bl + 1), bl - a_hi[c]), 0)
        lbs[f'i2_{p}'] = h + gT * gT; heads[p] = h
    for nm, lb in lbs.items():
        rec[f'lbviol_{nm}'] = int((lb > dist).sum())
        rec[f'viol_{nm}'] = int(((lb > tau) & within).sum())
        rec[f'violseed_{nm}'] = int(((lb > tau_seed) & within).sum())
    for tn, t in (('true', tau), ('seed', tau_seed)):
        k1 = lb1 <= t
        rec[f'cl_{tn}'] = int((lbc <= t).sum()); rec[f'i1_{tn}'] = int(k1.sum())
        for p in PS:
            k2 = lbs[f'i2_{p}'] <= t
            rec[f'i2_{p}_{tn}'] = int(k2.sum()); rec[f'both_{p}_{tn}'] = int((k1 & k2).sum())
            rec[f'head_{p}_{tn}'] = int(((heads[p] <= t) & k1).sum())
    # (f) one-bit code, dial m
    Pq = P @ qp
    sq_ = S[idx].astype(np.float32) @ Pq
    nuf = nu[idx].astype(np.float64); sa = np.maximum(sabs[idx].astype(np.float64), 1e-9)
    ip = nuf * (sq_ - spm[idx]) / sa
    c0 = np.minimum(sa / (np.sqrt(d) * np.sqrt(np.maximum(nuf, 1e-9))), 1.0)
    err = np.sqrt(nuf) * np.sqrt(A2[c].astype(np.float64)) * np.sqrt((1 - c0 * c0) / (c0 * c0 * (d - 1)))
    est = A2[c] + nuf - 2 * ip
    for mm in MS:
        lbf = est - 2 * mm * err
        for tn, t in (('true', tau), ('seed', tau_seed)):
            rec[f'f{mm}_{tn}'] = int((lbf <= t).sum())
            rec[f'flost{mm}_{tn}'] = int(((lbf > t) & within).sum())
    recs.append(rec)
log('queries done', len(recs), 'exact-distance check: queries', checked, 'mismatches', mismatch)
res['exact_check'] = {'queries': checked, 'mismatches': mismatch}
json.dump({'summary': res, 'records': recs}, open(OUT + '/stage0_records' + os.environ.get('TAG', '') + '.json', 'w'))

# ---- aggregate
def bucket(v):
    for b in range(5):
        if edges[b] <= v < edges[b + 1]: return names[b]
groups = {'ALL': recs, 'one-tag V>=10k (large one-tag)': [r for r in recs if r['ntags'] == 1 and r['V'] >= 10000]}
for ntg in (1, 2):
    for nm in names:
        groups[f'{ntg}-tag V {nm}'] = [r for r in recs if r['ntags'] == ntg and bucket(r['V']) == nm]
def pw(rs, num, den='V'):
    D = sum(r[den] for r in rs); return sum(r[num] for r in rs) / D if D else float('nan')
def mq(rs, num, den='V'):
    return float(np.mean([r[num] / r[den] for r in rs if r[den] > 0])) if rs else float('nan')
for gn, rs in groups.items():
    if not rs: print(f'\n== {gn}: 0 queries'); continue
    P_ = sum(r['V'] for r in rs)
    print(f'\n== {gn}: queries {len(rs)}, pairs {P_}, median V {int(np.median([r["V"] for r in rs]))}, '
          f'median occupied clusters {int(np.median([r["occ"] for r in rs]))}, seed clusters >2 in {sum(r["seed_clusters"]>2 for r in rs)} queries, '
          f'median tau_seed/tau {np.median([r["tau_seed"]/max(r["tau"],1) for r in rs]):.3f}')
    for tn in ('true', 'seed'):
        print(f' threshold={tn}: (pair-weighted | mean per query)')
        print(f'   cluster cut alone leaves {pw(rs,"cl_"+tn):.4f} | {mq(rs,"cl_"+tn):.4f};  f_c (I1 cut+item) {pw(rs,"i1_"+tn):.4f} | {mq(rs,"i1_"+tn):.4f}')
        for p in PS:
            print(f'   p={p}: I2 over all pairs {pw(rs,f"i2_{p}_{tn}"):.4f} | {mq(rs,f"i2_{p}_{tn}"):.4f};  '
                  f's = I2 among I1 survivors {pw(rs,f"both_{p}_{tn}","i1_"+tn):.4f} | {mq(rs,f"both_{p}_{tn}","i1_"+tn):.4f};  '
                  f'head only (no tail) among I1 survivors {pw(rs,f"head_{p}_{tn}","i1_"+tn):.4f}')
        for mm in MS:
            sv = [r[f'f{mm}_{tn}'] for r in rs]
            print(f'   1-bit dial {mm} sd: survivors/query mean {np.mean(sv):.0f} median {np.median(sv):.0f} share of pairs {pw(rs,f"f{mm}_{tn}"):.5f}; '
                  f'true top-k rows lost {sum(r[f"flost{mm}_{tn}"] for r in rs)} of {sum(r["n_within"] for r in rs)}')
    vk = [k for k in rs[0] if k.startswith(('viol', 'lbviol'))]
    print('   violations:', {k: sum(r[k] for r in rs) for k in vk})
log('end')
