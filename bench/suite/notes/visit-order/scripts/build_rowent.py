# Builds the real index on a fit (vexact_py.Index, default pseudo-queries, as the suite), keeps its row order
# (encoded/row_ent.bin -> FITDIR/row_ent_index.bin) for the pseudo-query replay, measures knn_approx recall@10 at eps 0
# for a query set at nprobe 16/64/256 (to check O0 against the replay), then removes the encoded index.
# usage: python -I build_rowent.py FITDIR QUERIES.npy GT.npy OUT.json
import sys, os, json, shutil, time
import numpy as np
import vexact_py

fitdir, qp, gp, outp = sys.argv[1:5]
t0 = time.time()
ix = vexact_py.Index(fitdir)
shutil.copy(os.path.join(fitdir, "encoded", "row_ent.bin"), os.path.join(fitdir, "row_ent_index.bin"))
Q0 = np.load(qp).astype(np.float32)
Q = np.zeros((len(Q0), ix.dim), np.float32)
Q[:, :Q0.shape[1]] = Q0
gt = np.load(gp)[:, :10]
res = {}
for p in (16, 64, 256):
    res[str(p)] = float(np.mean([len(set(ix.knn_approx(q, 10, p, 0.0).tolist()) & set(g.tolist())) / 10 for q, g in zip(Q, gt)]))
print(f"built in {time.time() - t0:.0f} s; K {ix.clusters}; knn_approx recall at nprobe 16/64/256: {res}", flush=True)
json.dump(res, open(outp, "w"))
del ix
shutil.rmtree(os.path.join(fitdir, "encoded"))
