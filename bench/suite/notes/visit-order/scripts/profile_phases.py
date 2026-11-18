# Where an approximate k-NN query's time goes, per phase, on an index the suite kept (`run.py ... --keep-work`):
# the order (P_PLAN: the query's preparation, q.m of every cluster and the sort), the group setup of the probed
# clusters, the scoring of stored vectors, and the rest of the probed clusters' work. Median microseconds over the
# suite's queries, after one warm-up pass, on one pinned core.
# usage: SUITE_DATA=... python3 -I profile_phases.py DATASET [--nprobe 16,64] [--filters none,random-1,random-25]
#        [--core 3] [--order ORDER]
import argparse
import os
import sys

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
from common import Prep, prep_dir, work_dir  # noqa: E402

import vexact_py  # noqa: E402

# Stats::cyc indices (rust/src/v2/q.rs)
P = dict(ANCHOR=0, CUT=1, BASIS=2, ROT=3, CLSETUP=4, TABLES=5, REPACK=6, DECODE=7, BOUNDS=8, FULL=9, INIT=10, PLAN=11,
         RUNCL=13, CLUSTER=14, QUERY=15, MERGE=16, COMPACT=17, LVSETUP=18, GPOST=19)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dataset")
    ap.add_argument("--nprobe", default="16,64")
    ap.add_argument("--filters", default="none,random-1,random-25")
    ap.add_argument("--core", type=int, default=3)
    ap.add_argument("--k", type=int, default=10)
    ap.add_argument("--order", default=None)
    a = ap.parse_args()
    data = os.environ["SUITE_DATA"]
    prep = Prep(prep_dir(data, a.dataset))
    os.sched_setaffinity(0, {a.core})
    kw = {} if a.order is None else dict(order=a.order)
    ix = vexact_py.Index(os.path.join(work_dir(data, a.dataset, "vexact"), "index"), core=a.core, **kw)
    Q = prep.queries()
    qb = np.zeros(ix.dim, np.float32)
    print(f"{a.dataset}: {ix.clusters} clusters, {prep.N} rows, k {a.k}")
    print(f"{'filter':<10} {'nprobe':>6} {'total':>8} {'order':>8} {'groups':>8} {'stored':>8} {'rest':>8}   us, median over {len(Q)} queries")
    for f in a.filters.split(","):
        vis = prep.vis(f)
        ix.set_visible(None if vis is None else vis.astype(np.int64))
        for npb in [int(x) for x in a.nprobe.split(",")]:
            rows = []
            for rep in range(2):
                for q in Q:
                    qb[:] = 0
                    qb[: len(q)] = q
                    cyc, hz = ix.profile(qb, a.k, npb)
                    if rep:
                        us = np.array(cyc, np.float64) / hz * 1e6
                        groups = us[P["BASIS"]] + us[P["ROT"]] + us[P["GPOST"]]
                        rows.append((us[P["QUERY"]], us[P["PLAN"]], groups, us[P["FULL"]]))
            r = np.median(np.array(rows), axis=0)
            print(f"{f:<10} {npb:>6} {r[0]:8.1f} {r[1]:8.1f} {r[2]:8.1f} {r[3]:8.1f} {r[0] - r[1] - r[2] - r[3]:8.1f}")


main()
