# Question 1: what reading a sub-cluster costs, by layout, against reading its whole cluster (`v2::sub::read_costs`).
# For each query, its 16 clusters of largest centre score holding a visible row; at the running k-th best of a k-NN at
# k = 10 over them, each cluster read whole (directly, and on the index path), its best sub-cluster by sub-centre
# score read alone, and its best four read one after another (the index path's per-cluster setup kept between them):
# each timing the fastest of 3 after evicting the core's caches. Layouts (a) masked and (b) key ranges on the default
# index, (c) runs on the index built with BuildOptions::sub; the queries go to the three in an order shuffled per
# query. Per layout: mean visible rows and microseconds of each read, the ratio of each sub-cluster read to the whole
# cluster's at its cheaper path, and a least-squares fit of each kind of read's time as fixed + per-row x rows.
# usage: python3 -I read_costs.py DATASET DEFAULT_DIR C_DIR [--filters none,random-25] [--top 16] [--core 3]
import argparse
import os
import random
import sys

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
from common import Prep, prep_dir  # noqa: E402

import vexact_py  # noqa: E402

ap = argparse.ArgumentParser()
ap.add_argument("dataset")
ap.add_argument("default_dir")
ap.add_argument("c_dir")
ap.add_argument("--filters", default="none,random-1,random-25,region-1,region-25")
ap.add_argument("--top", type=int, default=16)
ap.add_argument("--core", type=int, default=3)
ap.add_argument("--nq", type=int, default=100)
a = ap.parse_args()
os.sched_setaffinity(0, {a.core})
prep = Prep(prep_dir(os.environ.get("SUITE_DATA", "/home/user/suite-data"), a.dataset))
ix = {
    "a masked": vexact_py.Index(a.default_dir, core=a.core, sub="masked"),
    "b keyranges": vexact_py.Index(a.default_dir, core=a.core, sub="keyranges"),
    "c runs": vexact_py.Index(a.c_dir, core=a.core, sub="runs", sub_build=16),
}
first = ix["a masked"]
Q = prep.queries()[: a.nq]
qb = np.zeros((len(Q), first.dim), np.float32)
qb[:, : Q.shape[1]] = Q
print(f"{a.dataset}: {first.clusters} clusters, {len(Q)} queries, top {a.top} clusters each")
for name, x in ix.items():
    lay, s, secs, lb, sb, e_us = x.sub_info
    print(f"  {name}: {s} sub-clusters a cluster, computed in {secs:.1f}s at open; layout state {lb / 1e6:.2f} MB, "
          f"approximate state {sb / 1e6:.2f} MB; expanding a cluster {e_us:.2f} us")
cols = ["ci", "rows", "whole_dir", "whole_idx", "one_rows", "one_dir", "one_idx", "four_rows", "four_dir", "four_idx"]
for f in a.filters.split(","):
    vis = prep.vis(f)
    for x in ix.values():
        x.set_visible(None if vis is None else vis.astype(np.int64))
    got = {k: [] for k in ix}
    rng = random.Random(f"{a.dataset}/{f}")
    for qi in range(len(Q)):
        names = list(ix)
        rng.shuffle(names)
        for k in names:
            got[k].extend(ix[k].sub_read_costs(qb[qi : qi + 1], a.top))
    print(f"\n{f}: mean over (query, cluster) samples with a sub-cluster read")
    print("  layout        rows  whole dir/idx us | best sub: rows  dir/idx us  x whole | best four: rows  dir/idx us  x whole"
          " | fit us = fixed + per-row x rows: whole dir, whole idx, best sub dir, best sub idx")
    for k, v in got.items():
        d = np.array([r for r in v if r[4] > 0], dtype=np.float64)
        if len(d) == 0:
            continue
        m = d.mean(axis=0)
        whole = np.minimum(d[:, 2], d[:, 3])
        one = np.minimum(d[:, 5], d[:, 6])
        four = np.minimum(d[:, 8], d[:, 9])

        def fit(x, y):
            A = np.stack([np.ones_like(x), x], 1)
            c, *_ = np.linalg.lstsq(A, y, rcond=None)
            return f"{c[0]:.2f} + {c[1] * 1e3:.1f}n"

        print(f"  {k:<12} {m[1]:6.0f}  {m[2]:7.1f} / {m[3]:7.1f} | {m[4]:5.0f}  {m[5]:6.1f} / {m[6]:6.1f}  "
              f"x{one.mean() / whole.mean():.2f} | {m[7]:5.0f}  {m[8]:6.1f} / {m[9]:6.1f}  x{four.mean() / whole.mean():.2f} | "
              f"{fit(d[:, 1], d[:, 2])}, {fit(d[:, 1], d[:, 3])}, {fit(d[:, 4], d[:, 5])}, {fit(d[:, 4], d[:, 6])}  (ns a row)",
              flush=True)
