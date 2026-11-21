# Paired comparison of two indexes of one dataset under one build of the library (two fits, say at two group counts):
# per query, the cells of both run in an order shuffled per query and repetition (seeded), as ab_orders.py does.
# Cells per index: the exact k-NN (`knn`, k), the exact threshold query at each of the prep's answer shares for the set
# (`threshold`), and the approximate k-NN over psi (`knn_approx`). Output lines are ab_orders.py's, the arm prefixed by
# the index's label (A or B), so ab_curves.py interpolates the approximate arms; the exact cells are one point each.
# usage: SUITE_DATA=... python3 -I ab_indexes.py DATASET DIR_A DIR_B [--psi 2,...,7] [--filters none,...]
#        [--reps 3] [--core 11] [--k 10] [--nq 0]
import argparse
import os
import random
import sys
import time

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
from common import Prep, prep_dir  # noqa: E402

import vexact_py  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dataset")
    ap.add_argument("a")
    ap.add_argument("b")
    ap.add_argument("--psi", default="2,2.5,3,3.5,4,4.5,5,5.5,6,6.5,7")
    ap.add_argument("--filters", default="none,random-1,random-25,region-1,region-25")
    ap.add_argument("--reps", type=int, default=3)
    ap.add_argument("--core", type=int, default=11)
    ap.add_argument("--k", type=int, default=10)
    ap.add_argument("--nq", type=int, default=0, help="the first this many queries (0: all)")
    a = ap.parse_args()
    prep = Prep(prep_dir(os.environ["SUITE_DATA"], a.dataset))
    os.sched_setaffinity(0, {a.core})
    ix = {"A": vexact_py.Index(a.a, core=a.core), "B": vexact_py.Index(a.b, core=a.core)}
    psis = [float(x) for x in a.psi.split(",")]
    Q = prep.queries()
    if a.nq:
        Q = Q[: a.nq]
    qb = np.zeros(ix["A"].dim, np.float32)
    print(f"{a.dataset}: {ix['A'].clusters} clusters, {len(Q)} queries, k {a.k}, {a.reps} repetitions; A {a.a}, B {a.b}")
    for lab, x in ix.items():
        print(f"{lab}: measured constants {x.measured_costs}")
    for f in a.filters.split(","):
        vis = prep.vis(f)
        for x in ix.values():
            x.set_visible(None if vis is None else vis.astype(np.int64))
        order = prep.order(f)
        ths = prep.thresholds(f)
        knn_want = lambda qi: set(order[qi, : a.k].tolist())
        cells = []  # (arm, point, call, rows, truth)
        for lab, x in ix.items():
            cells.append((f"{lab}:exact-knn", "k=10", lambda q, qi, x=x: x.knn(q, a.k),
                          lambda q, qi, x=x: x.explain(q, a.k, None)[0], knn_want))
            for j, share in enumerate(prep.filter_shares(f)):
                size = prep.answer_size(f, share)
                cells.append((f"{lab}:exact-thr@{share:g}", "psi=inf",
                              lambda q, qi, x=x, j=j: x.threshold(q, float(ths[qi, j])),
                              lambda q, qi: 0, lambda qi, size=size: set(order[qi, :size].tolist())))
            for v in psis:
                cells.append((f"{lab}:adaptive", f"psi={v:g}", lambda q, qi, x=x, v=v: x.knn_approx(q, a.k, v),
                              lambda q, qi, x=x, v=v: x.explain_approx(q, a.k, v)["touched"], knn_want))
        w = max(len(c[0]) for c in cells) + 2
        rng = random.Random(f"{a.dataset}/{f}")
        t_ = np.zeros((len(cells), len(Q)))
        rec = np.zeros((len(cells), len(Q)))
        rows = np.zeros((len(cells), len(Q)))
        for qi, q in enumerate(Q):
            qb[:] = 0
            qb[: len(q)] = q
            ts = [[] for _ in cells]
            for rep in range(a.reps + 1):
                perm = list(range(len(cells)))
                rng.shuffle(perm)
                for j in perm:
                    _, _, call, rowf, want = cells[j]
                    t0 = time.perf_counter()
                    got = call(qb, qi)
                    dt = time.perf_counter() - t0
                    if rep:
                        ts[j].append(dt)
                    else:
                        w_ = want(qi)
                        rec[j, qi] = len(w_ & set(got.tolist())) / max(1, len(w_))
                        rows[j, qi] = rowf(qb, qi)
            t_[:, qi] = [np.median(x) * 1e6 for x in ts]
        for j, (arm, pl, _, _, _) in enumerate(cells):
            print(f"{f:<10} {arm:<{w}} {pl}: {rec[j].mean():.3f} {rows[j].mean():8.0f} rows {np.median(t_[j]):8.1f} us",
                  flush=True)


main()
