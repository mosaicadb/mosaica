# faiss Panorama (flat, inner product): is filtered range_search exact, and how much does it skip?
import sys, time, json, numpy as np, faiss, resource
S = sys.argv[1]; faiss.omp_set_num_threads(1); rng = np.random.default_rng(7)
X = np.empty((100000, 768), dtype='float32')
for i, f in enumerate(['e00', 'e10']): X[i*50000:(i+1)*50000] = np.load(f'{S}/vec/data/{f}.npy')
X /= np.linalg.norm(X, axis=1, keepdims=True); N, d = X.shape
TQ = np.load(f'{S}/vec/agent2/text_q.npy')[::6][:40].astype('float32'); TQ /= np.linalg.norm(TQ, axis=1, keepdims=True)
IQ = X[rng.choice(N, 40, replace=False)].copy()
# PCA rotation (energy compaction), full rank, orthonormal: inner products preserved
pca = faiss.PCAMatrix(d, d); pca.train(X[:50000]); 
mean = faiss.vector_to_array(pca.mean); A = faiss.vector_to_array(pca.A).reshape(d, d)
print('orthonormality err', float(np.abs(A @ A.T - np.eye(d)).max()))
for a in range(0, N, 20000): X[a:a+20000] = X[a:a+20000] @ A.T     # no centring: keeps q.x
TQ = TQ @ A.T; IQ = IQ @ A.T
vis = rng.random(N) < 0.10; bm = np.packbits(vis, bitorder='little'); sel = faiss.IDSelectorBitmap(N, faiss.swig_ptr(bm)); visidx = np.flatnonzero(vis)
stats = None
for nm in ('indexPanorama_stats', 'panorama_stats', 'indexPanoramaStats'):
    try: stats = getattr(faiss.cvar, nm); print('stats var', nm); break
    except Exception: pass
out = {}
for nl in (8, 16, 64):
    ix = faiss.IndexFlatIPPanorama(d, nl); ix.add(X)
    for qn, Q in (('text', TQ), ('item', IQ)):
        Sc = Q @ X.T
        for frac in (0.001, 0.01):
            t = np.sort(Sc[:, vis], axis=1)[:, -max(1, int(frac*vis.sum()))]
            for use_sel in (True, False):
                miss = extra = invis = ntrue = border = 0
                if stats is not None: stats.reset()
                t0 = time.perf_counter()
                res = []
                for i in range(len(Q)):
                    p = faiss.SearchParameters()
                    if use_sel: p.sel = sel
                    res.append(ix.range_search(Q[i:i+1], float(t[i]), params=p))
                el = time.perf_counter() - t0
                for i, (lims, D, I) in enumerate(res):
                    m = (Sc[i] >= t[i]) & (vis if use_sel else True); tr = set(np.flatnonzero(m).tolist()); got = set(I.tolist())
                    ntrue += len(tr); miss += len(tr - got); extra += len(got - tr); invis += int((~vis[I]).sum()) if use_sel else 0
                    border += int((np.abs(Sc[i][list(tr ^ got)] - t[i]) < 1e-5).sum()) if tr ^ got else 0
                r = {'true': ntrue, 'missed': miss, 'extra': extra, 'mismatch_within_1e-5_of_t': border, 'invisible_returned': invis,
                     'ms_per_query': round(el/len(Q)*1e3, 2), 'ns_per_item': round(el/len(Q)/N*1e9, 1)}
                if stats is not None: r['ratio_dims_scanned'] = float(stats.ratio_dims_scanned)
                out[f'L{nl} {qn}@{frac} sel={use_sel}'] = r; print(f'L{nl} {qn}@{frac} sel={use_sel}', r, flush=True)
        # k-NN exactness
        if stats is not None: stats.reset()
        t0 = time.perf_counter(); I = np.vstack([ix.search(Q[i:i+1], 10)[1] for i in range(len(Q))]); el = time.perf_counter() - t0
        gt = np.argsort(-Sc, axis=1)[:, :10]
        r = {'recall@10': float(np.mean([len(set(I[i]) & set(gt[i]))/10 for i in range(len(Q))])), 'ns_per_item': round(el/len(Q)/N*1e9, 1)}
        if stats is not None: r['ratio_dims_scanned'] = float(stats.ratio_dims_scanned)
        out[f'L{nl} {qn} knn10'] = r; print(f'L{nl} {qn} knn10', r, flush=True)
    del ix
fl = faiss.IndexFlatIP(d); fl.add(X[:20000]); t0 = time.perf_counter(); [fl.search(TQ[i:i+1], 10) for i in range(40)]
out['FlatIP ns_per_item (20k rows)'] = round((time.perf_counter()-t0)/40/20000*1e9, 1); print(out['FlatIP ns_per_item (20k rows)'])
out['peak_rss_mb'] = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss/1024; print(out['peak_rss_mb'])
json.dump(out, open(f'{S}/vec/borrow/pano.json', 'w'), indent=1)
