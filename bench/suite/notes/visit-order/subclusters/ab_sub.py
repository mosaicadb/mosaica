# Question 3: what sub-clusters buy the approximate k-NN, in time. Paired as `../scripts/ab_orders.py` pairs (one
# process; for each query the (arm, grid point) cells in an order shuffled per query and repetition; the first round
# measures recall and rows and is not timed). Arms:
#   spread      the legacy order by nprobe (`knn_nprobe`), default layout
#   adaptive    the library's approximate k-NN (`knn_approx`), default layout
#   adaptive-c  the same on layout (c) (every cluster read through its 16 runs)
#   sub-L[-own][-unsplit|-filter]  `knn_sub` on layout L: a (masked sub-clusters on the default layout), b (contiguous
#               key ranges of it) or c (contiguous k-means runs); sigma_s = sigma_c rad_s / rad_c, or with -own from the
#               sub-cluster's own variances along its cluster's directions; a split cluster's sub-clusters read
#               best-first (`SplitMode::BestFirst`), -unsplit as that unless reading them one by one would cost as much
#               as the whole cluster (`Unsplit`), -filter those whose key reaches the k-th best in one read (`Filter`)
# Per (filter, arm, grid point): mean recall@k against the prep's truth, mean rows touched, and the median over queries
# of each query's median time; for the adaptive arms their paths, clusters read, and for the sub-cluster arms the
# clusters split, sub-clusters read, and clusters scored but read whole. `../scripts/ab_curves.py` interpolates.
# usage: python3 -I ab_sub.py DATASET DEFAULT_DIR C_DIR [--arms ...] [--psi ...] [--nprobe ...] [--filters ...]
#        [--reps 3] [--core 3] [--k 10]
import argparse
import os
import random
import sys
import time
from collections import Counter

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
from common import Prep, prep_dir  # noqa: E402

import vexact_py  # noqa: E402

ARMS = "spread,adaptive,adaptive-c,sub-a,sub-b,sub-c,sub-c-own,sub-c-filter,sub-c-unsplit"
ap = argparse.ArgumentParser()
ap.add_argument("dataset")
ap.add_argument("default_dir")
ap.add_argument("c_dir")
ap.add_argument("--arms", default=ARMS)
ap.add_argument("--nprobe", default="4,8,16,32,64,128,256,512")
ap.add_argument("--psi", default="2,2.5,3,3.5,4,4.5,5,5.5,6,7,8,10")
ap.add_argument("--filters", default="none,random-1,random-25,region-1,region-25")
ap.add_argument("--reps", type=int, default=3)
ap.add_argument("--core", type=int, default=3)
ap.add_argument("--k", type=int, default=10)
ap.add_argument("--nq", type=int, default=0, help="the first this many queries (0: all)")
a = ap.parse_args()
os.sched_setaffinity(0, {a.core})
prep = Prep(prep_dir(os.environ.get("SUITE_DATA", "/home/user/suite-data"), a.dataset))
arms = a.arms.split(",")
psis = [float(x) for x in a.psi.split(",")]
opened = {}


def index(which):
    if which not in opened:
        d, sub = which
        opened[which] = vexact_py.Index(d, core=a.core, sub=sub, sub_build=16 if d == a.c_dir else 0)
    return opened[which]


where = {"spread": (a.default_dir, None), "adaptive": (a.default_dir, None), "adaptive-c": (a.c_dir, None)}
for arm in a.arms.split(","):
    if arm.startswith("sub-"):
        where[arm] = {"a": (a.default_dir, "masked"), "b": (a.default_dir, "keyranges"), "c": (a.c_dir, "runs")}[arm[4]]
ix = {arm: index(where[arm]) for arm in arms}
first = next(iter(ix.values()))
Q = prep.queries()
Q = Q[: a.nq] if a.nq else Q
qb = np.zeros(first.dim, np.float32)
print(f"{a.dataset}: {first.clusters} clusters, {len(Q)} queries, k {a.k}, {a.reps} repetitions; k-NN arms " + ", ".join(arms))
for arm in arms:
    x = ix[arm]
    if arm.startswith("sub"):
        print(f"{arm}: {x.sub_info}")
print(f"measured constants: default {index(where['adaptive']).measured_costs}")
for f in a.filters.split(","):
    vis = prep.vis(f)
    for x in opened.values():
        x.set_visible(None if vis is None else vis.astype(np.int64))
    order = prep.order(f)
    want = lambda qi: set(order[qi, : a.k].tolist())
    cells = []
    for arm in arms:
        x = ix[arm]
        if arm == "spread":
            for p in [int(v) for v in a.nprobe.split(",")]:
                cells.append((arm, f"nprobe={p}", lambda q, x=x, p=p: x.knn_nprobe(q, a.k, p),
                              lambda q, x=x, p=p: dict(touched=x.explain(q, a.k, p)[0])))
        elif arm.startswith("adaptive"):
            for v in psis:
                cells.append((arm, f"psi={v:g}", lambda q, x=x, v=v: x.knn_approx(q, a.k, v),
                              lambda q, x=x, v=v: x.explain_approx(q, a.k, v)))
        else:
            sig = "own" if "-own" in arm else "scaled"
            un = "unsplit" if "-unsplit" in arm else "filter" if "-filter" in arm else "bestfirst"
            for v in psis:
                cells.append((arm, f"psi={v:g}", lambda q, x=x, v=v, sig=sig, un=un: x.knn_sub(q, a.k, v, sig, un),
                              lambda q, x=x, v=v, sig=sig, un=un: x.explain_sub(q, a.k, v, sig, un)))
    w = max(len(c[0]) for c in cells) + 2
    rng = random.Random(f"{a.dataset}/{f}")
    t_ = np.zeros((len(cells), len(Q)))
    rec = np.zeros((len(cells), len(Q)))
    rows = np.zeros((len(cells), len(Q)))
    info = [[] for _ in cells]
    for qi, q in enumerate(Q):
        qb[:] = 0
        qb[: len(q)] = q
        ts = [[] for _ in cells]
        for rep in range(a.reps + 1):
            perm = list(range(len(cells)))
            rng.shuffle(perm)
            for j in perm:
                _, _, call, expl = cells[j]
                t0 = time.perf_counter()
                got = call(qb)
                dt = time.perf_counter() - t0
                if rep:
                    ts[j].append(dt)
                else:
                    w_ = want(qi)
                    rec[j, qi] = len(w_ & set(got.tolist())) / max(1, len(w_))
                    e = expl(qb)
                    rows[j, qi] = e["touched"]
                    info[j].append(e)
        t_[:, qi] = [np.median(x) * 1e6 for x in ts]
    for j, (arm, pl, _, _) in enumerate(cells):
        line = f"{f:<10} {arm:<{w}} {pl}: {rec[j].mean():.3f} {rows[j].mean():8.0f} rows {np.median(t_[j]):8.1f} us"
        if "visited" in info[j][0]:
            m = lambda key: np.mean([e[key] for e in info[j]])
            paths = Counter(e["path"] for e in info[j])
            line += (f"  paths {dict(paths)} visited {m('visited'):.1f} direct {m('direct'):.1f} index {m('index'):.1f} "
                     f"groups {m('groups'):.1f} spreads {m('spreads'):.0f}")
            if "split" in info[j][0]:
                line += f" split {m('split'):.1f} subreads {m('sub_reads'):.1f} unsplit {m('unsplit'):.1f}"
        print(line, flush=True)
