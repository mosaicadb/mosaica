# Does faiss RaBitQ range_search + IDSelector return the exact threshold set? What would certification need?
import sys, time, json, numpy as np, faiss, resource
S = sys.argv[1]; ROT = int(sys.argv[2])
faiss.omp_set_num_threads(1)
rng = np.random.default_rng(7)
X = np.empty((200000, 768), dtype='float32')
for i, f in enumerate(['e00', 'e05', 'e10', 'e15']):
    X[i*50000:(i+1)*50000] = np.load(f'{S}/vec/data/{f}.npy')
X /= np.linalg.norm(X, axis=1, keepdims=True)
N, d = X.shape
TQ = np.load(f'{S}/vec/agent2/text_q.npy')[::6][:60].astype('float32')
TQ /= np.linalg.norm(TQ, axis=1, keepdims=True)
IQ = X[rng.choice(N, 40, replace=False)].copy()
if ROT:  # Haar rotation (sign-fixed QR), applied to data and queries
    A = rng.standard_normal((d, d)); Qm, R = np.linalg.qr(A); Qm *= np.sign(np.diag(R)); P = Qm.astype('float32')
    for a in range(0, N, 20000): X[a:a+20000] = X[a:a+20000] @ P
    TQ = TQ @ P; IQ = IQ @ P
vis = rng.random(N) < 0.10
bm = np.packbits(vis, bitorder='little')
sel = faiss.IDSelectorBitmap(N, faiss.swig_ptr(bm))
visidx = np.flatnonzero(vis)
out = {'rot': ROT, 'N': N, 'visible': int(vis.sum())}

def truth(Q, frac):
    Sc = Q @ X.T
    t = np.sort(Sc[:, vis], axis=1)[:, -max(1, int(frac*vis.sum()))]
    return Sc, t

def evalrs(name, idx, mk, Q, Sc, t):
    miss = extra = invis = ntrue = nret = 0; t0 = time.perf_counter()
    for i in range(len(Q)):
        try:
            lims, D, I = idx.range_search(Q[i:i+1], float(t[i]), params=mk())
        except Exception as e:
            return {'error': str(e).split('\n')[0][:160]}
        tr = set(visidx[Sc[i, vis] >= t[i]].tolist()); got = set(I.tolist())
        ntrue += len(tr); nret += len(got); miss += len(tr - got); extra += len(got - tr)
        invis += int((~vis[I]).sum())
    return {'true': ntrue, 'returned': nret, 'missed': miss, 'extra': extra, 'invisible_returned': invis,
            'ms_per_query': round((time.perf_counter()-t0)/len(Q)*1e3, 2)}

def P1(**kw):
    def f():
        p = faiss.RaBitQSearchParameters(); p.sel = sel
        for k, v in kw.items(): setattr(p, k, v)
        return p
    return f
def PI(nprobe, **kw):
    def f():
        p = faiss.IVFRaBitQSearchParameters(); p.sel = sel; p.nprobe = nprobe
        for k, v in kw.items(): setattr(p, k, v)
        return p
    return f
def PIVF(nprobe):
    def f():
        p = faiss.SearchParametersIVF(); p.sel = sel; p.nprobe = nprobe; return p
    return f

idxs = {}
for nb in (1, 4, 9):
    ix = faiss.IndexRaBitQ(d, faiss.METRIC_INNER_PRODUCT, nb); ix.train(X); ix.add(X); idxs[f'RaBitQ{nb}'] = ix
    out[f'RaBitQ{nb}_code_size'] = ix.code_size
qz = faiss.IndexFlatIP(d)
ivf = faiss.IndexIVFRaBitQ(qz, d, 256, faiss.METRIC_INNER_PRODUCT); ivf.train(X); ivf.add(X)
out['IVFRaBitQ_code_size'] = ivf.code_size
try:
    fs = faiss.IndexRaBitQFastScan(d, faiss.METRIC_INNER_PRODUCT); fs.train(X); fs.add(X)
except Exception as e: fs = None; out['fs_build_error'] = str(e)[:200]
try:
    qz2 = faiss.IndexFlatIP(d)
    ifs = faiss.IndexIVFRaBitQFastScan(qz2, d, 256, faiss.METRIC_INNER_PRODUCT); ifs.train(X); ifs.add(X)
except Exception as e: ifs = None; out['ifs_build_error'] = str(e)[:200]

for qn, Q in (('text', TQ), ('item', IQ)):
    for frac in (0.001, 0.01):
        Sc, t = truth(Q, frac); key = f'{qn}@{frac}'; r = out[key] = {}
        for nb in (1, 4, 9):
            r[f'RaBitQ{nb} qb=4(default)'] = evalrs('', idxs[f'RaBitQ{nb}'], P1(qb=4), Q, Sc, t)
            r[f'RaBitQ{nb} qb=0(float query)'] = evalrs('', idxs[f'RaBitQ{nb}'], P1(qb=0), Q, Sc, t)
        r['IVFRaBitQ1 nprobe=all qb=0'] = evalrs('', ivf, PI(256, qb=0), Q, Sc, t)
        r['IVFRaBitQ1 nprobe=16 qb=0'] = evalrs('', ivf, PI(16, qb=0), Q, Sc, t)
        if fs is not None: r['RaBitQFastScan'] = evalrs('', fs, P1(), Q, Sc, t)
        if ifs is not None: r['IVFRaBitQFastScan nprobe=all'] = evalrs('', ifs, PI(256), Q, Sc, t)
        del Sc

# --- certification: error of the 1-bit estimate in units of the paper's sd, and candidates left to re-rank
ix = idxs['RaBitQ1']
c = faiss.vector_to_array(ix.center)
Rr = X - c  # 614 MB temp
nr = np.linalg.norm(Rr, axis=1); l1 = np.abs(Rr).sum(1); del Rr
ratio = nr**2 * d / l1**2
sd_item = nr * np.sqrt(np.maximum(ratio - 1, 0) / (d - 1))      # f_error / 1.9 for inner product
cert = out['certify_1bit'] = {}
for qn, Q in (('text', TQ[:30]), ('item', IQ[:20])):
    zs = []; cand = {m: [] for m in (1.9, 3, 4, 5, 6, 7, 9.35)}; viol = {m: 0 for m in cand}; ans = []
    Sc, t = truth(Q, 0.001)
    for i in range(len(Q)):
        p = faiss.RaBitQSearchParameters(); p.qb = 0
        lims, D, I = ix.range_search(Q[i:i+1], -1e30, params=p)
        est = np.empty(N, 'float32'); est[I] = D
        g = np.linalg.norm(Q[i] - c)
        w = sd_item * g
        z = (est - Sc[i]) / w; zs.append(z)
        for m in cand:
            lo, hi = est - m*w, est + m*w
            und = (hi >= t[i]) & (lo < t[i]) & vis          # must read the full vector
            cand[m].append(und.sum() / vis.sum())
            viol[m] += int(((Sc[i] >= t[i]) & (hi < t[i]) & vis).sum() + ((Sc[i] < t[i]) & (lo >= t[i]) & vis).sum())
        ans.append(((Sc[i] >= t[i]) & vis).sum() / vis.sum())
    z = np.concatenate(zs)
    cert[qn] = {'pairs': int(z.size), 'z_std': float(z.std()), 'z_mean': float(z.mean()), 'max_abs_z': float(np.abs(z).max()),
                'frac_abs_z_gt_1.9': float((np.abs(z) > 1.9).mean()), 'frac_abs_z_gt_3': float((np.abs(z) > 3).mean()),
                'answer_frac': float(np.mean(ans)),
                'undecided_frac_by_multiplier': {str(m): float(np.mean(v)) for m, v in cand.items()},
                'wrong_decisions_by_multiplier': {str(m): viol[m] for m in cand},
                'mean_halfwidth_at_1sd': float(np.mean(sd_item) * g)}
# does the multi-bit code expose f_error? read it out of the stored code and compare with our formula
ix4 = idxs['RaBitQ4']; codes = faiss.vector_to_array(ix4.codes).reshape(N, ix4.code_size)
fe = codes[:, 96:108].copy().view('float32')[:, 2]
out['f_error_stored_vs_formula_maxrel'] = float(np.max(np.abs(fe - 1.9*sd_item) / (1.9*sd_item)))
out['peak_rss_mb'] = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024
json.dump(out, open(f'{S}/vec/borrow/rq_exact_rot{ROT}.json', 'w'), indent=1)
print(json.dumps(out, indent=1))
