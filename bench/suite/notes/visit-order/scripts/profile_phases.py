# Where an approximate k-NN query's time goes, per phase, on an index the suite kept (`run.py ... --keep-work`).
# A cluster order stopped by nprobe: the order (P_PLAN: the query's preparation, the centre scores, the spread of the H
# order's candidates and the sort), the group setup of the probed clusters, the scoring of stored vectors, and the rest
# of the probed clusters' work. The approximate process (`--process SPREAD`, swept over `--price`): the centre scores
# and the reads' setup (P_PLAN), the spreads (P_SPREAD), tau and the plan (P_SOLVE), then the same three phases of the
# reads, and its counts: spreads' directions projected, units read directly and on the index path, groups set up.
# A simple arm (`--simple VARIANT`, swept over `--knob`): the centre scores (P_PLAN), the spreads (P_SPREAD), the order
# (P_SOLVE: keys, heap and stop), the path choice, the anchor search and the race's accounting (P_CHOICE), then the
# reads' three phases, and its counts: directions projected, clusters visited, read directly and on the index path,
# groups set up.
# Median microseconds (and counts) over the suite's queries, after one warm-up pass, on one pinned core.
# usage: SUITE_DATA=... python3 -I profile_phases.py DATASET [--nprobe 16,64] [--filters none,random-1,random-25]
#        [--core 3] [--order ORDER | --process group|own|own-all [--price 1e-3,1e-4,1e-5]
#        | --simple s1|s2|s3|s4|s4-own --knob 0.5,1,2]
import argparse
import os
import sys

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
from common import Prep, prep_dir, work_dir  # noqa: E402

import vexact_py  # noqa: E402

# Stats::cyc indices (rust/src/v2/q.rs)
P = dict(ANCHOR=0, CUT=1, BASIS=2, ROT=3, CLSETUP=4, TABLES=5, REPACK=6, DECODE=7, BOUNDS=8, FULL=9, INIT=10, PLAN=11,
         SPREAD=12, RUNCL=13, CLUSTER=14, QUERY=15, MERGE=16, COMPACT=17, LVSETUP=18, GPOST=19, SOLVE=25, CHOICE=30)
HCAND = 24  # a count: the clusters whose spread the H order computed
C = dict(SPREADS=26, DIRECT=27, INDEX=28, GROUPS=29, VISITED=31)  # the process's and the simple arms' counts


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dataset")
    ap.add_argument("--nprobe", default="16,64")
    ap.add_argument("--filters", default="none,random-1,random-25")
    ap.add_argument("--core", type=int, default=3)
    ap.add_argument("--k", type=int, default=10)
    ap.add_argument("--order", default=None)
    ap.add_argument("--process", default=None, help="the approximate process with this spread (group, own, own-all)")
    ap.add_argument("--price", default="1e-3,1e-4,1e-5")
    ap.add_argument("--simple", default=None, help="a simple arm (s1, s2, s3, s4, s4-own)")
    ap.add_argument("--knob", default="0.5,1,2", help="the simple arm's knob (nprobe for s1, f for s2 and s3, psi for s4)")
    a = ap.parse_args()
    data = os.environ["SUITE_DATA"]
    prep = Prep(prep_dir(data, a.dataset))
    os.sched_setaffinity(0, {a.core})
    kw = {} if a.order is None else dict(order=a.order)
    ix = vexact_py.Index(os.path.join(work_dir(data, a.dataset, "vexact"), "index"), core=a.core, **kw)
    Q = prep.queries()
    qb = np.zeros(ix.dim, np.float32)
    print(f"{a.dataset}: {ix.clusters} clusters, {prep.N} rows, k {a.k}")
    mode = "process" if a.process else "simple" if a.simple else "order"
    if mode == "order":
        points = [(str(p), lambda q, p=p: ix.profile(q, a.k, p)) for p in [int(x) for x in a.nprobe.split(",")]]
        print(f"{'filter':<10} {'nprobe':>6} {'total':>8} {'order':>8} {'groups':>8} {'stored':>8} {'rest':>8} {'H cand':>7}   us, median over {len(Q)} queries")
    elif mode == "process":
        points = [(f"price={v}", lambda q, v=float(v): ix.profile_process(q, a.k, spread=a.process, price=v)) for v in a.price.split(",")]
        print(f"process, spread {a.process}")
        print(f"{'filter':<10} {'point':>16} {'total':>8} {'centres':>8} {'spreads':>8} {'solve':>8} {'groups':>8} {'stored':>8} {'rest':>8} "
              f"{'dirs':>6} {'direct':>6} {'index':>6} {'grps':>5}   us and counts, median over {len(Q)} queries")
    else:
        points = [(f"knob={v}", lambda q, v=float(v): ix.profile_simple(q, a.k, a.simple, v)) for v in a.knob.split(",")]
        print(f"simple {a.simple}; measured constants {ix.measured_costs}")
        print(f"{'filter':<10} {'point':>12} {'total':>8} {'centres':>8} {'spreads':>8} {'order':>8} {'choice':>8} {'groups':>8} {'stored':>8} "
              f"{'rest':>8} {'dirs':>6} {'visit':>6} {'direct':>6} {'index':>6} {'grps':>5}   us and counts, median over {len(Q)} queries")
    for f in a.filters.split(","):
        vis = prep.vis(f)
        ix.set_visible(None if vis is None else vis.astype(np.int64))
        for label, prof in points:
            rows = []
            for rep in range(2):
                for q in Q:
                    qb[:] = 0
                    qb[: len(q)] = q
                    cyc, hz = prof(qb)
                    if rep:
                        us = np.array(cyc, np.float64) / hz * 1e6
                        groups = us[P["BASIS"]] + us[P["ROT"]] + us[P["GPOST"]]
                        if mode == "order":
                            rows.append((us[P["QUERY"]], us[P["PLAN"]], groups, us[P["FULL"]], cyc[HCAND]))
                        elif mode == "simple":
                            rows.append((us[P["QUERY"]], us[P["PLAN"]], us[P["SPREAD"]], us[P["SOLVE"]], us[P["CHOICE"]], groups, us[P["FULL"]],
                                         cyc[C["SPREADS"]], cyc[C["VISITED"]], cyc[C["DIRECT"]], cyc[C["INDEX"]], cyc[C["GROUPS"]]))
                        else:
                            rows.append((us[P["QUERY"]], us[P["PLAN"]], us[P["SPREAD"]], us[P["SOLVE"]], groups, us[P["FULL"]],
                                         cyc[C["SPREADS"]], cyc[C["DIRECT"]], cyc[C["INDEX"]], cyc[C["GROUPS"]]))
            r = np.median(np.array(rows), axis=0)
            if mode == "order":
                print(f"{f:<10} {label:>6} {r[0]:8.1f} {r[1]:8.1f} {r[2]:8.1f} {r[3]:8.1f} {r[0] - r[1] - r[2] - r[3]:8.1f} {r[4]:7.0f}")
            elif mode == "simple":
                rest = r[0] - r[1:7].sum()
                print(f"{f:<10} {label:>12} {r[0]:8.1f} {r[1]:8.1f} {r[2]:8.1f} {r[3]:8.1f} {r[4]:8.1f} {r[5]:8.1f} {r[6]:8.1f} {rest:8.1f} "
                      f"{r[7]:6.0f} {r[8]:6.1f} {r[9]:6.1f} {r[10]:6.1f} {r[11]:5.1f}")
            else:
                rest = r[0] - r[1:6].sum()
                print(f"{f:<10} {label:>16} {r[0]:8.1f} {r[1]:8.1f} {r[2]:8.1f} {r[3]:8.1f} {r[4]:8.1f} {r[5]:8.1f} {rest:8.1f} "
                      f"{r[6]:6.0f} {r[7]:6.1f} {r[8]:6.1f} {r[9]:5.1f}")


main()
