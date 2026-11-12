# ns per item of faiss code scans (1 thread), with and without an IDSelector; error sd of multi-bit estimates
import sys, time, json, numpy as np, faiss, resource
S = sys.argv[1]
faiss.omp_set_num_threads(1)
rng = np.random.default_rng(7)
X = np.empty((200000, 768), dtype='float32')
for i, f in enumerate(['e00', 'e05', 'e10', 'e15']):
    X[i*50000:(i+1)*50000] = np.load(f'{S}/vec/data/{f}.npy')
X /= np.linalg.norm(X, axis=1, keepdims=True)
N, d = X.shape
Q = np.load(f'{S}/vec/agent2/text_q.npy')[::6][:40].astype('float32'); Q /= np.linalg.norm(Q, axis=1, keepdims=True)
A = rng.standard_normal((d, d)); Qm, R = np.linalg.qr(A); Qm *= np.sign(np.diag(R)); P = Qm.astype('float32')
for a in range(0, N, 20000): X[a:a+20000] = X[a:a+20000] @ P
Q = Q @ P
Sc = Q @ X.T
gt = np.argsort(-Sc, axis=1)[:, :10]
sels = {}
for fr in (0.1, 0.01):
    v = rng.random(N) < fr; bm = np.packbits(v, bitorder='little')
    sels[fr] = (v, bm, faiss.IDSelectorBitmap(N, faiss.swig_ptr(bm)))
out = {}
def bench(name, ix, mkp=None, fr=None, single=True):
    nq = len(Q); best = 1e9
    for rep in range(3):
        t0 = time.perf_counter()
        if single:
            I = np.vstack([ix.search(Q[i:i+1], 10, params=mkp() if mkp else None)[1] for i in range(nq)])
        else:
            I = ix.search(Q, 10, params=mkp() if mkp else None)[1]
        best = min(best, time.perf_counter() - t0)
    r = {'ms_per_query': round(best/nq*1e3, 3), 'ns_per_item': round(best/nq/N*1e9, 2)}
    if fr is None:
        r['recall@10'] = float(np.mean([len(set(I[i]) & set(gt[i]))/10 for i in range(nq)]))
    else:
        v = sels[fr][0]; r['ns_per_visible_item'] = round(best/nq/v.sum()*1e9, 2)
        g = np.argsort(-np.where(v, Sc, -9), axis=1)[:, :10]
        r['recall@10'] = float(np.mean([len(set(I[i]) & set(g[i]))/10 for i in range(nq)]))
    if hasattr(ix, 'code_size'): r['code_bytes'] = int(ix.code_size)
    out[name] = r; print(name, r, flush=True)
def psel(cls, fr, **kw):
    def f():
        p = cls(); p.sel = sels[fr][2]
        for k, v in kw.items(): setattr(p, k, v)
        return p
    return f
def pk(cls, **kw):
    def f():
        p = cls()
        for k, v in kw.items(): setattr(p, k, v)
        return p
    return f
RP = faiss.RaBitQSearchParameters; SP = faiss.SearchParameters; IP_ = faiss.IVFRaBitQSearchParameters

fl = faiss.IndexFlatIP(d); fl.add(X[:25000])
t0 = time.perf_counter(); [fl.search(Q[i:i+1], 10) for i in range(len(Q))]; t = time.perf_counter() - t0
out['FlatIP f32 (25k rows)'] = {'ns_per_item': round(t/len(Q)/25000*1e9, 2)}; print(out, flush=True); del fl

r1 = faiss.IndexRaBitQ(d, faiss.METRIC_INNER_PRODUCT, 1); r1.train(X); r1.add(X)
bench('RaBitQ 1-bit qb=4', r1, pk(RP, qb=4)); bench('RaBitQ 1-bit qb=0', r1, pk(RP, qb=0)); bench('RaBitQ 1-bit qb=8', r1, pk(RP, qb=8))
for fr in sels: bench(f'RaBitQ 1-bit qb=4 sel={fr}', r1, psel(RP, fr, qb=4), fr)
# error sd of the estimates relative to the stored-formula sd
c = faiss.vector_to_array(r1.center); nr = np.zeros(N, 'float32'); l1 = np.zeros(N, 'float32')
for a in range(0, N, 20000):
    Rr = X[a:a+20000] - c; nr[a:a+20000] = np.linalg.norm(Rr, axis=1); l1[a:a+20000] = np.abs(Rr).sum(1)
sd = nr * np.sqrt(np.maximum(nr**2*d/l1**2 - 1, 0)/(d-1))
def errstat(ix, name):
    zs = []; es = []
    for i in range(10):
        p = RP(); p.qb = 0; lims, D, I = ix.range_search(Q[i:i+1], -1e30, params=p)
        est = np.empty(N, 'float32'); est[I] = D; e = est - Sc[i]; es.append(e); zs.append(e/(sd*np.linalg.norm(Q[i]-c)))
    z = np.concatenate(zs); e = np.concatenate(es)
    out[name] = {'err_rms': float(np.sqrt((e**2).mean())), 'err_max_abs': float(np.abs(e).max()), 'z_std_vs_1bit_formula': float(z.std()), 'z_max_abs': float(np.abs(z).max())}
    print(name, out[name], flush=True)
errstat(r1, 'err RaBitQ 1-bit (104 B)')
try:
    fs = faiss.IndexRaBitQFastScan(r1)
except Exception as e:
    print('from-index ctor failed', str(e)[:100]); fs = faiss.IndexRaBitQFastScan(d, faiss.METRIC_INNER_PRODUCT); fs.train(X); fs.add(X)
bench('RaBitQFastScan 1-bit', fs); bench('RaBitQFastScan 1-bit batch40', fs, single=False)
for fr in sels: bench(f'RaBitQFastScan 1-bit sel={fr}', fs, psel(RP, fr), fr)
del fs, r1
for nb in (4, 9):
    r = faiss.IndexRaBitQ(d, faiss.METRIC_INNER_PRODUCT, nb); r.train(X); r.add(X)
    bench(f'RaBitQ {nb}-bit two-stage', r); errstat(r, f'err RaBitQ {nb}-bit ({r.code_size} B)'); del r
for nb in (4,):
    try:
        f2 = faiss.IndexRaBitQFastScan(d, faiss.METRIC_INNER_PRODUCT, 32, nb); f2.train(X); f2.add(X)
        bench(f'RaBitQFastScan {nb}-bit two-stage', f2); del f2
    except Exception as e: print('fs multibit', str(e)[:200])
for M in (64, 192):
    pq = faiss.IndexPQFastScan(d, M, 4, faiss.METRIC_INNER_PRODUCT); pq.train(X[:30000]); pq.add(X)
    bench(f'PQFastScan M={M} ({M//2} B)', pq); bench(f'PQFastScan M={M} batch40', pq, single=False)
    for fr in sels: bench(f'PQFastScan M={M} sel={fr}', pq, psel(SP, fr), fr)
    del pq
qz = faiss.IndexFlatIP(d); ifs = faiss.IndexIVFRaBitQFastScan(qz, d, 256, faiss.METRIC_INNER_PRODUCT); ifs.train(X); ifs.add(X)
bench('IVFRaBitQFastScan nprobe=256', ifs, pk(IP_, nprobe=256)); bench('IVFRaBitQFastScan nprobe=16', ifs, pk(IP_, nprobe=16))
for fr in sels: bench(f'IVFRaBitQFastScan nprobe=256 sel={fr}', ifs, psel(IP_, fr, nprobe=256), fr)
del ifs
qz = faiss.IndexFlatIP(d); iv = faiss.IndexIVFRaBitQ(qz, d, 256, faiss.METRIC_INNER_PRODUCT); iv.train(X); iv.add(X)
bench('IVFRaBitQ 1-bit nprobe=256', iv, pk(IP_, nprobe=256))
for fr in sels: bench(f'IVFRaBitQ 1-bit nprobe=256 sel={fr}', iv, psel(IP_, fr, nprobe=256), fr)
out['peak_rss_mb'] = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss/1024
json.dump(out, open(f'{S}/vec/borrow/speed.json', 'w'), indent=1); print('peak', out['peak_rss_mb'])
