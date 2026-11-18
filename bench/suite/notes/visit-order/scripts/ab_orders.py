# Paired comparison of the approximate mode's cluster orders on an index the suite kept (`run.py ... --keep-work`):
# one index per order in this process, the orders taking turns query by query, so that a drift in the machine's speed
# falls on all of them alike. Per (filter, nprobe) and order: mean recall@k against the prep's truth, rows touched
# (the visible rows of the probed clusters, which does not depend on the machine), and the median over queries of
# each query's median time; then each order's time relative to the first, as the median over queries of the ratio.
# usage: SUITE_DATA=... python3 -I ab_orders.py DATASET radius spread [h4 ...] [--nprobe 4,16,64]
#        [--filters none,random-1,random-25] [--reps 5] [--core 3] [--k 10]
import argparse
import os
import sys
import time

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
from common import Prep, prep_dir, work_dir  # noqa: E402

import vexact_py  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dataset")
    ap.add_argument("orders", nargs="+")
    ap.add_argument("--nprobe", default="4,16,64")
    ap.add_argument("--filters", default="none,random-1,random-25")
    ap.add_argument("--reps", type=int, default=5)
    ap.add_argument("--core", type=int, default=3)
    ap.add_argument("--k", type=int, default=10)
    a = ap.parse_args()
    data = os.environ["SUITE_DATA"]
    prep = Prep(prep_dir(data, a.dataset))
    os.sched_setaffinity(0, {a.core})
    path = os.path.join(work_dir(data, a.dataset, "vexact"), "index")
    ixs = [vexact_py.Index(path, core=a.core, order=o) for o in a.orders]
    Q = prep.queries()
    qb = np.zeros(ixs[0].dim, np.float32)
    print(f"{a.dataset}: {ixs[0].clusters} clusters, {len(Q)} queries, k {a.k}, {a.reps} repetitions; "
          + ", ".join(f"{o} f={ix.order[1]:g}" for o, ix in zip(a.orders, ixs)))
    w = max(len(o) for o in a.orders) + 2
    for f in a.filters.split(","):
        vis = prep.vis(f)
        for ix in ixs:
            ix.set_visible(None if vis is None else vis.astype(np.int64))
        truth = prep.order(f)
        for npb in [int(x) for x in a.nprobe.split(",")]:
            t = np.zeros((len(ixs), len(Q)))
            rec = np.zeros((len(ixs), len(Q)))
            rows = np.zeros((len(ixs), len(Q)))
            for qi, q in enumerate(Q):
                qb[:] = 0
                qb[: len(q)] = q
                want = set(truth[qi, : a.k].tolist())
                ts = [[] for _ in ixs]
                for rep in range(a.reps + 1):
                    for j, ix in enumerate(ixs):
                        t0 = time.perf_counter()
                        got = ix.knn_approx(qb, a.k, npb, 0.0)
                        dt = time.perf_counter() - t0
                        if rep:
                            ts[j].append(dt)
                        else:
                            rec[j, qi] = len(want & set(got.tolist())) / a.k
                            rows[j, qi] = ix.explain(qb, a.k, npb)[0]
                t[:, qi] = [np.median(x) * 1e6 for x in ts]
            print(f"{f:<10} nprobe {npb:>4}  " + "  ".join(
                f"{o:>{w}}: {rec[j].mean():.3f} {rows[j].mean():8.0f} rows {np.median(t[j]):8.1f} us"
                + ("" if j == 0 else f" (x{np.median(t[j] / t[0]):.3f})")
                for j, o in enumerate(a.orders)))


main()
