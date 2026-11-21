# Paired comparison of the approximate queries on an index the suite kept (`run.py ... --keep-work`).
# k-NN arms: `adaptive` (the library's approximate k-NN, `knn_approx`, swept over psi) and `spread` (the legacy order
# by nprobe, `knn_nprobe`, kept as the comparison arm). With --threshold, threshold queries at the prep's thresholds
# (one per answer share): `adaptive@SHARE` (`threshold_approx` over psi) against `exact@SHARE` (`threshold`, recall 1).
# One index per arm in this process; for each query the (arm, grid point) cells run in an order shuffled per query
# and repetition (seeded), so that neither a drift in the machine's speed nor the caches an arm's last cell leaves fall
# on one arm's first cells. Per (filter, arm, grid point): mean recall against the prep's truth (k-NN: recall@k;
# threshold: the share of the exact answer returned), rows touched (the visible rows the query read, which does not
# depend on the machine), and the median over queries of each query's median time; for the adaptive arms also their
# paths (probe / race / scan / exact), clusters visited, read directly and on the index path, groups set up, spreads
# computed and modelled cost against a scan's. ab_curves.py interpolates time and rows to a recall.
# With a build that gives them, a k-NN adaptive line also has the stored vectors read and the rows past the
# deterministic levels; --level-psi sets the deterministic levels' psi apart from the stop's, --level-psi-offset C
# to max(0, psi - C) at each psi.
# usage: SUITE_DATA=... python3 -I ab_orders.py DATASET [spread] [adaptive] [--threshold]
#        [--nprobe 4,...,512] [--psi 2,...,7] [--filters none,random-1,random-25] [--reps 5] [--core 3] [--k 10]
#        [--index DIR] [--level-psi X | --level-psi-offset C]
import argparse
import os
import random
import sys
import time
from collections import Counter

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
from common import Prep, prep_dir, work_dir  # noqa: E402

import vexact_py  # noqa: E402

NPROBE = "4,8,16,32,64,128,256,512"
PSI = "2,2.5,3,3.5,4,4.5,5,5.5,6,6.5,7"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dataset")
    ap.add_argument("arms", nargs="*", default=["spread", "adaptive"])
    ap.add_argument("--threshold", action="store_true", help="threshold queries instead of k-NN")
    ap.add_argument("--nprobe", default=NPROBE, help="the legacy arm's nprobe")
    ap.add_argument("--psi", default=PSI, help="the adaptive arm's psi")
    ap.add_argument("--filters", default="none,random-1,random-25,region-1,region-25")
    ap.add_argument("--reps", type=int, default=5)
    ap.add_argument("--core", type=int, default=3)
    ap.add_argument("--k", type=int, default=10)
    ap.add_argument("--nq", type=int, default=0, help="the first this many queries (0: all)")
    ap.add_argument("--level-psi", type=float, default=None, help="the adaptive k-NN arm's psi at the deterministic levels' "
                    "statistical width (default: psi)")
    ap.add_argument("--level-psi-offset", type=float, default=None, help="the adaptive k-NN arm's level psi at each psi: "
                    "max(0, psi - this)")
    ap.add_argument("--index", default="", help="the index directory (default: the suite's work directory); give each "
                    "build of the library its own copy (`cp -al` of the fit), since a build re-encodes a stale index")
    a = ap.parse_args()
    data = os.environ["SUITE_DATA"]
    prep = Prep(prep_dir(data, a.dataset))
    os.sched_setaffinity(0, {a.core})
    path = a.index or os.path.join(work_dir(data, a.dataset, "vexact"), "index")
    psis = [float(x) for x in a.psi.split(",")]
    ix = {arm: vexact_py.Index(path, core=a.core) for arm in a.arms}
    first = next(iter(ix.values()))
    Q = prep.queries()
    if a.nq:
        Q = Q[: a.nq]
    qb = np.zeros(first.dim, np.float32)
    print(f"{a.dataset}: {first.clusters} clusters, {len(Q)} queries, k {a.k}, {a.reps} repetitions; "
          f"{'threshold' if a.threshold else 'k-NN'} arms " + ", ".join(a.arms))
    if "adaptive" in ix:
        print(f"adaptive: measured constants {ix['adaptive'].measured_costs}")
    for f in a.filters.split(","):
        vis = prep.vis(f)
        for x in ix.values():
            x.set_visible(None if vis is None else vis.astype(np.int64))
        order = prep.order(f)
        # cells: (arm label, point label, call(q, qi), explain(q, qi), truth(qi))
        cells = []
        if not a.threshold:
            want = lambda qi: set(order[qi, : a.k].tolist())
            if "spread" in ix:
                x = ix["spread"]
                for p in [int(v) for v in a.nprobe.split(",")]:
                    cells.append(("spread", f"nprobe={p}", lambda q, qi, x=x, p=p: x.knn_nprobe(q, a.k, p),
                                  lambda q, qi, x=x, p=p: dict(touched=x.explain(q, a.k, p)[0]), want))
            if "adaptive" in ix:
                x = ix["adaptive"]
                if a.level_psi is not None and a.level_psi_offset is not None:
                    sys.exit("give --level-psi or --level-psi-offset, not both")
                for v in psis:
                    lp = {} if a.level_psi is None else dict(level_psi=a.level_psi)
                    if a.level_psi_offset is not None:
                        lp = dict(level_psi=max(0.0, v - a.level_psi_offset))
                    cells.append(("adaptive", f"psi={v:g}", lambda q, qi, x=x, v=v, lp=lp: x.knn_approx(q, a.k, v, **lp),
                                  lambda q, qi, x=x, v=v, lp=lp: x.explain_approx(q, a.k, v, **lp), want))
        else:
            ths = prep.thresholds(f)
            for j, share in enumerate(prep.filter_shares(f)):
                size = prep.answer_size(f, share)
                want = lambda qi, size=size: set(order[qi, :size].tolist())
                t = lambda qi, j=j: float(ths[qi, j])
                x = ix.get("adaptive", first)
                cells.append((f"exact@{share:g}", "psi=inf", lambda q, qi, x=x, t=t: x.threshold(q, t(qi)),
                              lambda q, qi, x=x, t=t: x.explain_approx(q, psi=float("inf"), t=t(qi)), want))
                if "adaptive" in ix:
                    for v in psis:
                        cells.append((f"adaptive@{share:g}", f"psi={v:g}", lambda q, qi, x=x, v=v, t=t: x.threshold_approx(q, t(qi), v),
                                      lambda q, qi, x=x, v=v, t=t: x.explain_approx(q, psi=v, t=t(qi)), want))
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
                    _, _, call, expl, want = cells[j]
                    t0 = time.perf_counter()
                    got = call(qb, qi)
                    dt = time.perf_counter() - t0
                    if rep:
                        ts[j].append(dt)
                    else:
                        w_ = want(qi)
                        rec[j, qi] = len(w_ & set(got.tolist())) / max(1, len(w_))
                        e = expl(qb, qi)
                        rows[j, qi] = e["touched"]
                        info[j].append(e)
            t_[:, qi] = [np.median(x) * 1e6 for x in ts]
        for j, (arm, pl, _, _, _) in enumerate(cells):
            line = f"{f:<10} {arm:<{w}} {pl}: {rec[j].mean():.3f} {rows[j].mean():8.0f} rows {np.median(t_[j]):8.1f} us"
            if "visited" in info[j][0]:
                m = lambda key: np.mean([e[key] for e in info[j]])
                paths = Counter(e["path"] for e in info[j])
                line += (f"  paths {dict(paths)} visited {m('visited'):.1f} direct {m('direct'):.1f} index {m('index'):.1f} "
                         f"groups {m('groups'):.1f} spreads {m('spreads'):.0f} cost {m('cost'):.0f} scan {m('scan_cost'):.0f}")
                if "past_det" in info[j][0]:
                    line += f" stored {m('full_reads'):.0f} past_det {m('past_det'):.0f}"
            print(line, flush=True)


main()
