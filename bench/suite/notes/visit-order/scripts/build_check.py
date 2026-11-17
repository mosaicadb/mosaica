# Builds the real index (vexact_py.Index, as bench/suite/systems/vexact.py: default pseudo-queries, so Approx::fit runs
# in Rust) on a fit and measures knn_approx recall@10 at eps 0 for the test queries at several nprobe, to identify the
# f the library chose (by matching the numpy replay's per-f recall), and the same after fit_queries on learn queries.
# usage: python -I build_check.py FITDIR PREPDIR LEARNDIR OUT.json
import sys, os, json, time
import numpy as np
import vexact_py

fitdir, prep, learndir, outp = sys.argv[1:5]
t0 = time.time()
ix = vexact_py.Index(fitdir)
print(f"built/opened in {time.time() - t0:.0f} s; clusters {ix.clusters}", flush=True)
Q = np.load(os.path.join(prep, "queries.npy")).astype(np.float32)
gt = np.load(os.path.join(prep, "order.npy"))[:, :10]
NP = (8, 16, 32, 64, 128, 256)


def recalls():
    out = {}
    for p in NP:
        r = [len(set(ix.knn_approx(np.ascontiguousarray(q), 10, p, 0.0).tolist()) & set(g.tolist())) / 10 for q, g in zip(Q, gt)]
        out[str(p)] = float(np.mean(r))
    return out


res = dict(default=recalls())
print("default (pseudo-query f):", res["default"], flush=True)
LQ = np.load(os.path.join(learndir, "learn_q.npy")).astype(np.float32)
Lgt = np.load(os.path.join(learndir, "learn_gt.npy")).astype(np.int64)
ix.fit_queries(LQ, Lgt)
res["fit_queries"] = recalls()
print("after fit_queries (1,000 learn text queries):", res["fit_queries"], flush=True)
json.dump(res, open(outp, "w"), indent=1)
