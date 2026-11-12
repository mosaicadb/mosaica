# faiss IVF-RaBitQ: add / remove / persist round trip, mmap load
import sys, os, numpy as np, faiss, time
S = sys.argv[1]; faiss.omp_set_num_threads(1)
X = np.load(f'{S}/vec/data/e00.npy')[:20000].astype('float32'); X /= np.linalg.norm(X, axis=1, keepdims=True); d = X.shape[1]
q = faiss.IndexFlatIP(d); ix = faiss.IndexIVFRaBitQ(q, d, 64, faiss.METRIC_INNER_PRODUCT); ix.train(X[:10000]); ix.add(X[:10000])
n0 = ix.ntotal; rem = ix.remove_ids(np.arange(0, 1000, dtype='int64')); ix.add_with_ids(X[10000:12000], np.arange(50000, 52000, dtype='int64'))
print('ntotal', n0, '-> removed', rem, '-> after add', ix.ntotal)
p = f'{S}/vec/borrow/ivfrq.faiss'; t = time.perf_counter(); faiss.write_index(ix, p); print('write whole index: bytes', os.path.getsize(p), 's', round(time.perf_counter()-t, 3))
for flag, nm in ((0, 'read'), (faiss.IO_FLAG_MMAP, 'IO_FLAG_MMAP'), (getattr(faiss, 'IO_FLAG_MMAP_IFC', None), 'IO_FLAG_MMAP_IFC')):
    if flag is None: continue
    try:
        r = faiss.read_index(p, flag); r.nprobe = 64
        D, I = r.search(X[5000:5001], 3); print(nm, 'ok ntotal', r.ntotal, 'invlists', type(faiss.downcast_InvertedLists(r.invlists)).__name__, 'top', I[0], 'removed id present:', bool((I < 1000).any()))
        try: r.add_with_ids(X[12000:12010], np.arange(60000, 60010, dtype='int64')); print('   add after', nm, 'ok')
        except Exception as e: print('   add after', nm, 'FAILED:', str(e).split('\n')[0][:120])
    except Exception as e: print(nm, 'FAILED', str(e).split('\n')[0][:160])
fl = faiss.IndexFlatIPPanorama(d, 8); fl.add(X[:1000])
try: print('panorama remove_ids', fl.remove_ids(np.arange(10, dtype='int64')), fl.ntotal)
except Exception as e: print('panorama remove FAILED', str(e).split('\n')[0][:160])
os.remove(p)
