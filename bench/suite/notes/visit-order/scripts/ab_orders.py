# Paired comparison of the approximate mode's arms on an index the suite kept (`run.py ... --keep-work`): cluster
# orders stopped by nprobe (`radius`, `spread`, `h16x4`, ...) and the approximate process stopped by its dial
# (`process:SPREAD`, SPREAD one of group, own, own-all). One index per arm in this process; for each query every
# (arm, grid point) runs in turn, so that a drift in the machine's speed falls on all of them alike. Per (filter, arm,
# grid point): mean recall@k against the prep's truth, rows touched (the visible rows the query read, which does not
# depend on the machine), and the median over queries of each query's median time; for a process arm also its paths
# (plan / scan / exact), mean predicted recall, modelled cost, units read directly and on the index path, groups set
# up and spreads computed. ab_curves.py interpolates time and rows to a recall.
# usage: SUITE_DATA=... python3 -I ab_orders.py DATASET spread h16x4 process:group process:own
#        [--nprobe 4,8,16,32,64] [--dial mean_recall=0.5,0.7,0.8,0.9,0.95,0.98] [--filters none,random-1,random-25]
#        [--reps 5] [--core 3] [--k 10]
import argparse
import os
import sys
import time
from collections import Counter

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
from common import Prep, prep_dir, work_dir  # noqa: E402

import vexact_py  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dataset")
    ap.add_argument("arms", nargs="+")
    ap.add_argument("--nprobe", default="4,8,16,32,64")
    ap.add_argument("--dial", default="mean_recall=0.5,0.7,0.8,0.85,0.9,0.95,0.98",
                    help="the process's dial and its values: price, mean_recall, recall or budget")
    ap.add_argument("--filters", default="none,random-1,random-25")
    ap.add_argument("--reps", type=int, default=5)
    ap.add_argument("--core", type=int, default=3)
    ap.add_argument("--k", type=int, default=10)
    a = ap.parse_args()
    data = os.environ["SUITE_DATA"]
    prep = Prep(prep_dir(data, a.dataset))
    os.sched_setaffinity(0, {a.core})
    path = os.path.join(work_dir(data, a.dataset, "vexact"), "index")
    dname, dvals = a.dial.split("=")
    if dname not in ("price", "mean_recall", "recall", "budget"):
        sys.exit(f"--dial names price, mean_recall, recall or budget, not {dname}")
    arms = []  # (label, index, [(point label, call, explain)])
    for arm in a.arms:
        if arm.startswith("process:"):
            spread = arm.split(":", 1)[1]
            ix = vexact_py.Index(path, core=a.core)
            pts = []
            for v in dvals.split(","):
                kw = {dname: float(v), "spread": spread}
                pts.append((f"{dname}={v}", lambda q, ix=ix, kw=kw: ix.knn_process(q, a.k, **kw),
                            lambda q, ix=ix, kw=kw: ix.explain_process(q, a.k, **kw)))
        else:
            ix = vexact_py.Index(path, core=a.core, order=arm)
            pts = [(f"nprobe={p}", lambda q, ix=ix, p=p: ix.knn_approx(q, a.k, p, 0.0),
                    lambda q, ix=ix, p=p: dict(touched=ix.explain(q, a.k, p)[0])) for p in [int(x) for x in a.nprobe.split(",")]]
        arms.append((arm, ix, pts))
    Q = prep.queries()
    qb = np.zeros(arms[0][1].dim, np.float32)
    print(f"{a.dataset}: {arms[0][1].clusters} clusters, {len(Q)} queries, k {a.k}, {a.reps} repetitions; arms "
          + ", ".join(a.arms))
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
            if "path" in info[j][0]:
                paths = Counter(e["path"] for e in info[j])
                m = lambda key: np.mean([e[key] for e in info[j]])
                line += (f"  paths {dict(paths)} pred {m('pred'):.3f} cost {m('cost'):.0f} direct {m('direct'):.1f} "
                         f"index {m('index'):.1f} groups {m('groups'):.1f} spreads {m('spreads'):.0f} refined {m('refined'):.1f}")
            print(line, flush=True)


main()
