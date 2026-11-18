# Where an approximate query's time goes, per phase, on an index the suite kept (`run.py ... --keep-work`).
# The legacy arm (`spread` by nprobe, `knn_nprobe`): the order (P_PLAN: the query's preparation, the centre scores and
# the sort), the group setup of the probed clusters, the scoring of stored vectors, and the rest of the probed
# clusters' work. The approximate queries (`--adaptive`, swept over `--psi`; k-NN, or with `--threshold` the threshold
# queries at the prep's thresholds): the centre scores (P_PLAN), the spreads (P_SPREAD), the order (P_ORDER: keys,
# heap and stop), the path choice, the anchor search and the race's accounting (P_CHOICE; for a threshold query its
# support verdicts and pricing), then the reads' three phases, and the counts: directions projected, clusters visited,
# read directly and on the index path, groups set up. Median microseconds (and counts) over the suite's queries, after
# one warm-up pass, on one pinned core.
# usage: SUITE_DATA=... python3 -I profile_phases.py DATASET [--nprobe 16,64] [--filters none,random-1,random-25]
#        [--core 3] [--adaptive [--psi 3.5,4.5,5.5] [--threshold]]
import argparse
import os
import sys

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
from common import Prep, prep_dir, work_dir  # noqa: E402

import vexact_py  # noqa: E402

# Stats::cyc indices (rust/src/v2/q.rs)
P = dict(ANCHOR=0, CUT=1, BASIS=2, ROT=3, CLSETUP=4, TABLES=5, REPACK=6, DECODE=7, BOUNDS=8, FULL=9, INIT=10, PLAN=11,
         SPREAD=12, RUNCL=13, CLUSTER=14, QUERY=15, MERGE=16, COMPACT=17, LVSETUP=18, GPOST=19, ORDER=25, CHOICE=30)
C = dict(SPREADS=26, DIRECT=27, INDEX=28, GROUPS=29, VISITED=31)  # the approximate queries' counts


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dataset")
    ap.add_argument("--nprobe", default="16,64")
    ap.add_argument("--filters", default="none,random-1,random-25")
    ap.add_argument("--core", type=int, default=3)
    ap.add_argument("--k", type=int, default=10)
    ap.add_argument("--adaptive", action="store_true", help="the approximate queries (knn_approx, threshold_approx)")
    ap.add_argument("--psi", default="3.5,4.5,5.5")
    ap.add_argument("--threshold", action="store_true", help="with --adaptive: threshold queries at the prep's thresholds")
    a = ap.parse_args()
    data = os.environ["SUITE_DATA"]
    prep = Prep(prep_dir(data, a.dataset))
    os.sched_setaffinity(0, {a.core})
    ix = vexact_py.Index(os.path.join(work_dir(data, a.dataset, "vexact"), "index"), core=a.core)
    Q = prep.queries()
    qb = np.zeros(ix.dim, np.float32)
    print(f"{a.dataset}: {ix.clusters} clusters, {prep.N} rows, k {a.k}")
    if not a.adaptive:
        print(f"{'filter':<10} {'nprobe':>6} {'total':>8} {'order':>8} {'groups':>8} {'stored':>8} {'rest':>8}   us, median over {len(Q)} queries")
    else:
        print(f"approximate {'threshold' if a.threshold else 'k-NN'}; measured constants {ix.measured_costs}")
        print(f"{'filter':<10} {'point':>16} {'total':>8} {'centres':>8} {'spreads':>8} {'order':>8} {'choice':>8} {'groups':>8} {'stored':>8} "
              f"{'rest':>8} {'dirs':>6} {'visit':>6} {'direct':>6} {'index':>6} {'grps':>5}   us and counts, median over {len(Q)} queries")
    for f in a.filters.split(","):
        vis = prep.vis(f)
        ix.set_visible(None if vis is None else vis.astype(np.int64))
        if not a.adaptive:
            points = [(str(p), lambda q, qi, p=p: ix.profile(q, a.k, p)) for p in [int(x) for x in a.nprobe.split(",")]]
        elif not a.threshold:
            points = [(f"psi={v}", lambda q, qi, v=float(v): ix.profile_approx(q, a.k, v)) for v in a.psi.split(",")]
        else:
            ths = prep.thresholds(f)
            points = [(f"{share:g} psi={v}", lambda q, qi, v=float(v), j=j: ix.profile_approx(q, psi=v, t=float(ths[qi, j])))
                      for j, share in enumerate(prep.filter_shares(f)) for v in a.psi.split(",")]
        for label, prof in points:
            rows = []
            for rep in range(2):
                for qi, q in enumerate(Q):
                    qb[:] = 0
                    qb[: len(q)] = q
                    cyc, hz = prof(qb, qi)
                    if rep:
                        us = np.array(cyc, np.float64) / hz * 1e6
                        groups = us[P["BASIS"]] + us[P["ROT"]] + us[P["GPOST"]]
                        if not a.adaptive:
                            rows.append((us[P["QUERY"]], us[P["PLAN"]], groups, us[P["FULL"]]))
                        else:
                            rows.append((us[P["QUERY"]], us[P["PLAN"]], us[P["SPREAD"]], us[P["ORDER"]], us[P["CHOICE"]], groups, us[P["FULL"]],
                                         cyc[C["SPREADS"]], cyc[C["VISITED"]], cyc[C["DIRECT"]], cyc[C["INDEX"]], cyc[C["GROUPS"]]))
            r = np.median(np.array(rows), axis=0)
            if not a.adaptive:
                print(f"{f:<10} {label:>6} {r[0]:8.1f} {r[1]:8.1f} {r[2]:8.1f} {r[3]:8.1f} {r[0] - r[1] - r[2] - r[3]:8.1f}")
            else:
                rest = r[0] - r[1:7].sum()
                print(f"{f:<10} {label:>16} {r[0]:8.1f} {r[1]:8.1f} {r[2]:8.1f} {r[3]:8.1f} {r[4]:8.1f} {r[5]:8.1f} {r[6]:8.1f} {rest:8.1f} "
                      f"{r[7]:6.0f} {r[8]:6.1f} {r[9]:6.1f} {r[10]:6.1f} {r[11]:5.1f}")


main()
