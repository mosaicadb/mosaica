# Paired comparison of the approximate mode's arms on an index the suite kept (`run.py ... --keep-work`): cluster
# orders stopped by nprobe (`radius`, `spread`, `h16x4`, ...), the approximate process swept over its price
# (`process:SPREAD`, SPREAD one of group, own, own-all) and the simple arms swept over their knob (`simple:VARIANT`,
# VARIANT one of s1 (nprobe), s2 and s3 (the spread key's f), s4 and s4-own (psi)). One index per arm in this process;
# for each query every (arm, grid point) runs in turn, so that a drift in the machine's speed falls on all of them
# alike. Per (filter, arm, grid point): mean recall@k against the prep's truth, rows touched (the visible rows the
# query read, which does not depend on the machine), and the median over queries of each query's median time; for a
# process arm also its paths (plan / scan / exact), mean predicted recall, modelled cost, units read directly and on
# the index path, groups set up and spreads computed; for a simple arm its paths (probe / race / scan), clusters
# visited, read directly and on the index path, groups, spreads and modelled cost against a scan's. ab_curves.py
# interpolates time and rows to a recall.
# usage: SUITE_DATA=... python3 -I ab_orders.py DATASET spread h16x4 process:own simple:s1 simple:s3 simple:s4
#        [--nprobe 4,8,16,32,64] [--price 1e-1,...,1e-7] [--fprime 0.1,...,1.25] [--psi 2,...,7]
#        [--filters none,random-1,random-25] [--reps 5] [--core 3] [--k 10]
import argparse
import os
import sys
import time
from collections import Counter

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
from common import Prep, prep_dir, work_dir  # noqa: E402

import vexact_py  # noqa: E402

PRICES = "1e-1,3e-2,1e-2,3e-3,1e-3,3e-4,1e-4,3e-5,1e-5,3e-6,1e-6,1e-7"
FPRIME = "0.1,0.2,0.3,0.4,0.45,0.5,0.6,0.7,0.8,0.9,1,1.25"
PSI = "2,2.5,3,3.5,4,4.5,5,5.5,6,7"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dataset")
    ap.add_argument("arms", nargs="+")
    ap.add_argument("--nprobe", default="4,8,16,32,64", help="the orders' and simple:s1's nprobe")
    ap.add_argument("--price", default=PRICES, help="the process's price (share of the answer per modelled microsecond)")
    ap.add_argument("--fprime", default=FPRIME, help="simple:s2 and simple:s3's f")
    ap.add_argument("--psi", default=PSI, help="simple:s4 and simple:s4-own's psi")
    ap.add_argument("--filters", default="none,random-1,random-25")
    ap.add_argument("--reps", type=int, default=5)
    ap.add_argument("--core", type=int, default=3)
    ap.add_argument("--k", type=int, default=10)
    a = ap.parse_args()
    data = os.environ["SUITE_DATA"]
    prep = Prep(prep_dir(data, a.dataset))
    os.sched_setaffinity(0, {a.core})
    path = os.path.join(work_dir(data, a.dataset, "vexact"), "index")
    vals = lambda s: [float(x) for x in s.split(",")]
    arms = []  # (label, index, [(point label, call, explain)])
    for arm in a.arms:
        if arm.startswith("process:"):
            spread = arm.split(":", 1)[1]
            ix = vexact_py.Index(path, core=a.core)
            pts = []
            for v in vals(a.price):
                kw = {"price": v, "spread": spread}
                pts.append((f"price={v:g}", lambda q, ix=ix, kw=kw: ix.knn_process(q, a.k, **kw),
                            lambda q, ix=ix, kw=kw: ix.explain_process(q, a.k, **kw)))
        elif arm.startswith("simple:"):
            variant = arm.split(":", 1)[1]
            ix = vexact_py.Index(path, core=a.core)
            name, grid = {"s1": ("nprobe", a.nprobe), "s2": ("f", a.fprime), "s3": ("f", a.fprime),
                          "s4": ("psi", a.psi), "s4-own": ("psi", a.psi)}[variant]
            pts = [(f"{name}={v:g}", lambda q, ix=ix, v=v, vr=variant: ix.knn_simple(q, a.k, vr, v),
                    lambda q, ix=ix, v=v, vr=variant: ix.explain_simple(q, a.k, vr, v)) for v in vals(grid)]
        else:
            ix = vexact_py.Index(path, core=a.core, order=arm)
            pts = [(f"nprobe={p}", lambda q, ix=ix, p=p: ix.knn_approx(q, a.k, p, 0.0),
                    lambda q, ix=ix, p=p: dict(touched=ix.explain(q, a.k, p)[0])) for p in [int(x) for x in a.nprobe.split(",")]]
        arms.append((arm, ix, pts))
    Q = prep.queries()
    qb = np.zeros(arms[0][1].dim, np.float32)
    print(f"{a.dataset}: {arms[0][1].clusters} clusters, {len(Q)} queries, k {a.k}, {a.reps} repetitions; arms "
          + ", ".join(a.arms))
    for arm, ix, _ in arms:
        if arm.startswith("simple:"):
            print(f"{arm}: measured constants {ix.measured_costs}")
            break
    w = max(len(x) for x in a.arms) + 2
    for f in a.filters.split(","):
        vis = prep.vis(f)
        for _, ix, _ in arms:
            ix.set_visible(None if vis is None else vis.astype(np.int64))
        truth = prep.order(f)
        cells = [(arm, pl, call, expl) for arm, _, pts in arms for pl, call, expl in pts]
        t = np.zeros((len(cells), len(Q)))
        rec = np.zeros((len(cells), len(Q)))
        rows = np.zeros((len(cells), len(Q)))
        info = [[] for _ in cells]
        for qi, q in enumerate(Q):
            qb[:] = 0
            qb[: len(q)] = q
            want = set(truth[qi, : a.k].tolist())
            ts = [[] for _ in cells]
            for rep in range(a.reps + 1):
                for j, (_, _, call, expl) in enumerate(cells):
                    t0 = time.perf_counter()
                    got = call(qb)
                    dt = time.perf_counter() - t0
                    if rep:
                        ts[j].append(dt)
                    else:
                        rec[j, qi] = len(want & set(got.tolist())) / a.k
                        e = expl(qb)
                        rows[j, qi] = e["touched"]
                        info[j].append(e)
            t[:, qi] = [np.median(x) * 1e6 for x in ts]
        for j, (arm, pl, _, _) in enumerate(cells):
            line = f"{f:<10} {arm:<{w}} {pl}: {rec[j].mean():.3f} {rows[j].mean():8.0f} rows {np.median(t[j]):8.1f} us"
            m = lambda key: np.mean([e[key] for e in info[j]])
            if "pred" in info[j][0]:
                paths = Counter(e["path"] for e in info[j])
                line += (f"  paths {dict(paths)} pred {m('pred'):.3f} cost {m('cost'):.0f} direct {m('direct'):.1f} "
                         f"index {m('index'):.1f} groups {m('groups'):.1f} spreads {m('spreads'):.0f} refined {m('refined'):.1f}")
            elif "visited" in info[j][0]:
                paths = Counter(e["path"] for e in info[j])
                line += (f"  paths {dict(paths)} visited {m('visited'):.1f} direct {m('direct'):.1f} index {m('index'):.1f} "
                         f"groups {m('groups'):.1f} spreads {m('spreads'):.0f} cost {m('cost'):.0f} scan {m('scan_cost'):.0f}")
            print(line, flush=True)


main()
