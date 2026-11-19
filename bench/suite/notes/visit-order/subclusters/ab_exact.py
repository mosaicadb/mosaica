# Question 2: what layout (c) costs the exact queries. The default layout's index and layout (c)'s (BuildOptions::sub,
# rows ordered by (sub-cluster, key), the sorted range searched in every run) in one process; per visible set, the
# exact k-NN at k (`knn`) and the exact threshold query at each of the prep's thresholds (`threshold`, one per answer
# share), each on both layouts. For each query the (layout, query) cells run in an order shuffled per query and
# repetition; the first round checks that both layouts give the same entities and is not timed. Per cell: the median
# over queries of each query's median time, and the median over queries of the paired ratio (c) / default.
# usage: python3 -I ab_exact.py DATASET DEFAULT_DIR C_DIR [--filters ...] [--reps 5] [--core 3] [--k 10]
import argparse
import os
import random
import sys
import time

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
from common import Prep, prep_dir  # noqa: E402

import vexact_py  # noqa: E402

ap = argparse.ArgumentParser()
ap.add_argument("dataset")
ap.add_argument("default_dir")
ap.add_argument("c_dir")
ap.add_argument("--filters", default="none,random-1,random-25,region-1,region-25")
ap.add_argument("--reps", type=int, default=5)
ap.add_argument("--core", type=int, default=3)
ap.add_argument("--k", type=int, default=10)
ap.add_argument("--nq", type=int, default=0, help="the first this many queries (0: all)")
a = ap.parse_args()
os.sched_setaffinity(0, {a.core})
prep = Prep(prep_dir(os.environ.get("SUITE_DATA", "/home/user/suite-data"), a.dataset))
ix = {"default": vexact_py.Index(a.default_dir, core=a.core), "c": vexact_py.Index(a.c_dir, core=a.core, sub_build=16)}
Q = prep.queries()
Q = Q[: a.nq] if a.nq else Q
qb = np.zeros(ix["default"].dim, np.float32)
print(f"{a.dataset}: {ix['default'].clusters} clusters, {len(Q)} queries, k {a.k}, {a.reps} repetitions; exact k-NN and "
      f"threshold, default layout against layout (c)")
for f in a.filters.split(","):
    vis = prep.vis(f)
    for x in ix.values():
        x.set_visible(None if vis is None else vis.astype(np.int64))
    ths = prep.thresholds(f)
    kinds = [(f"knn@{a.k}", lambda x, q, qi: x.knn(q, a.k))]
    for j, share in enumerate(prep.filter_shares(f)):
        kinds.append((f"thr@{share:g}", lambda x, q, qi, j=j: x.threshold(q, float(ths[qi, j]))))
    cells = [(lay, kn, call) for kn, call in kinds for lay in ix]
    rng = random.Random(f"{a.dataset}/{f}")
    t_ = np.zeros((len(cells), len(Q)))
    size = np.zeros((len(cells), len(Q)))
    bad = {kn: 0 for kn, _ in kinds}
    for qi, q in enumerate(Q):
        qb[:] = 0
        qb[: len(q)] = q
        ts = [[] for _ in cells]
        for rep in range(a.reps + 1):
            perm = list(range(len(cells)))
            rng.shuffle(perm)
            got = {}
            for j in perm:
                lay, kn, call = cells[j]
                t0 = time.perf_counter()
                r = call(ix[lay], qb, qi)
                dt = time.perf_counter() - t0
                if rep:
                    ts[j].append(dt)
                else:
                    got[(lay, kn)] = r
                    size[j, qi] = int((r >= 0).sum())
            if rep == 0:
                for kn, _ in kinds:
                    if not np.array_equal(got[("default", kn)], got[("c", kn)]):
                        bad[kn] += 1
        t_[:, qi] = [np.median(x) * 1e6 for x in ts]
    for kn, _ in kinds:
        i0 = next(i for i, c in enumerate(cells) if c[:2] == ("default", kn))
        i1 = next(i for i, c in enumerate(cells) if c[:2] == ("c", kn))
        ratio = t_[i1] / t_[i0]
        print(f"{f:<10} {kn:<10} answer {size[i0].mean():9.1f}  default {np.median(t_[i0]):9.1f} us  (c) {np.median(t_[i1]):9.1f} us  "
              f"paired (c)/default median {np.median(ratio):.3f} [q25 {np.quantile(ratio, 0.25):.3f}, q75 {np.quantile(ratio, 0.75):.3f}]  "
              f"answers differ {bad[kn]}", flush=True)
