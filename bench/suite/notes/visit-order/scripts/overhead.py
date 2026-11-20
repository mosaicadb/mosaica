"""The self-correcting order's work per query on a scratch index (SUITE_DATA): stream re-keys, the clusters left in the
stream at each re-key bounded by the live clusters, and the final delta and rho, at psi 3 and 5 for k 10, under the
visible sets given. python3 overhead.py DATASET FILTERS"""
import os
import sys

import numpy as np

sys.path.insert(0, "/home/user/mosaica/.claude/worktrees/agent-a7862df6a6c7f0b43/bench/suite")
from common import Prep, prep_dir, work_dir  # noqa: E402
import vexact_py  # noqa: E402

ds, filters = sys.argv[1], sys.argv[2].split(",")
data = os.environ["SUITE_DATA"]
prep = Prep(prep_dir(data, ds))
ix = vexact_py.Index(os.path.join(work_dir(data, ds, "vexact"), "index"), core=3)
Q = prep.queries()[:50]
for f in filters:
    vis = prep.vis(f)
    ix.set_visible(None if vis is None else vis.astype(np.int64))
    for psi in (3.0, 5.0):
        rk, vi, de, rh = [], [], [], []
        for q in Q:
            qb = np.zeros(ix.dim, np.float32)
            qb[: len(q)] = q
            d = ix.explain_approx(qb, 10, psi)
            rk.append(d["rekeys"]); vi.append(d["visited"]); de.append(d["delta"]); rh.append(d["rho"])
        print(f"{ds} {f} psi {psi}: re-keys mean {np.mean(rk):.2f} max {max(rk)}; clusters read mean {np.mean(vi):.1f}; "
              f"delta median {np.median(de):.2f}; rho median {np.median(rh):.2f} [p10 {np.percentile(rh, 10):.2f}, p90 {np.percentile(rh, 90):.2f}]; "
              f"live clusters {ix.clusters}")
