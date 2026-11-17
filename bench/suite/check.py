# The check stage: every exact answer (exact k-NN, threshold) dumped by the run stage against the f64 truth of prep.
# usage: python3 -I check.py SUITE DATASET [--data DIR]
# For each dumped cell, counts the items that differ from the exact answer (the f64 top k, or every item above the
# threshold, within the visible set). A differing item is "within rounding" when |s - t| is at most what rounding the
# stored vectors and the query can move its similarity (as ../threshold-db/check.py): sum_i |q_i x_i| times the unit
# roundoff of each (f16 systems: 2^-11 on both; f32 systems: 2^-24 on both, plus an f32 accumulation bound of
# n 2^-24 sum_i |q_i x_i|). s is the f64 product of the f64 query and the stored f32 vector, and t the threshold, or for
# k-NN the midpoint between the k-th and (k+1)-th similarity. Writes results/<suite>/<dataset>/check.csv.
import argparse
import csv
import glob
import json
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import Prep, data_dir, dump_dir, prep_dir, results_dir  # noqa: E402

COLS = ["system", "type", "filter", "setting", "param", "answers", "items_expected", "items_differing",
        "beyond_rounding", "max_abs_s_minus_t"]


def roundoff(kind, n):
    if kind == "f16":
        return 2.0 ** -11, 2.0 ** -11, 0.0
    return 2.0 ** -24, 2.0 ** -24, n * 2.0 ** -24


def check(suite, ds, data):
    prep = Prep(prep_dir(data, ds))
    X = prep.corpus()
    Q = prep.queries()
    rows = []
    for sdir in sorted(glob.glob(os.path.join(dump_dir(data, suite, ds, "*")))):
        system = os.path.basename(sdir)
        for meta_f in sorted(glob.glob(os.path.join(sdir, "*.json"))):
            base = os.path.basename(meta_f)[:-5]
            qt, filt, setting = base.split("__")
            if qt == "approx":
                continue
            meta = json.load(open(meta_f))
            ux, uq, uacc = roundoff(meta["rounding"], prep.d)
            order = prep.order(filt)
            if qt == "threshold":
                T = prep.thresholds(filt)
                shares = prep.filter_shares(filt)
            else:
                T = np.load(prep.f("knn_thr.npy" if filt == "none" else f"knn_thr_{filt}.npy"))
                ks = prep.meta["k"] if filt == "none" else prep.meta["filters"][filt]["k"]
            a = np.fromfile(os.path.join(sdir, base + ".bin"), "<u4")
            stats = {}
            p = 0
            while p < len(a):
                qi, pi, c = (int(x) for x in a[p:p + 3])
                got = a[p + 3:p + 3 + c].astype(np.int64)
                p += 3 + c
                par = meta["params"][pi]
                if qt == "threshold":
                    m, t = prep.answer_size(filt, par["share"]), T[qi, shares.index(par["share"])]
                else:
                    m, t = par["k"], T[qi, ks.index(par["k"])]
                exp = np.asarray(order[qi, :m], dtype=np.int64)
                diff = np.setxor1d(got, exp)
                st = stats.setdefault(pi, [0, 0, 0, 0, 0.0])
                st[0] += 1
                st[1] += len(exp)
                st[2] += len(diff)
                if len(diff):
                    x = np.asarray(X[np.sort(diff)], dtype=np.float64)
                    s = x @ Q[qi]
                    mag = np.abs(x * Q[qi]).sum(1)
                    gap = np.abs(s - t)
                    st[3] += int((gap > mag * (ux + uq + uacc) + 1e-12).sum())
                    st[4] = max(st[4], float(gap.max()))
            for pi in sorted(stats):
                st = stats[pi]
                rows.append(dict(system=system, type=qt, filter=filt, setting=setting,
                                 param=json.dumps(meta["params"][pi], separators=(",", ":")), answers=st[0],
                                 items_expected=st[1], items_differing=st[2], beyond_rounding=st[3],
                                 max_abs_s_minus_t=f"{st[4]:.3g}"))
                print(",".join(str(rows[-1][c]) for c in COLS), flush=True)
    out = os.path.join(results_dir(suite, ds), "check.csv")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with open(out, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=COLS)
        w.writeheader()
        w.writerows(rows)
    return rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("suite")
    ap.add_argument("dataset")
    ap.add_argument("--data", default=None)
    a = ap.parse_args()
    check(a.suite, a.dataset, data_dir(a.data))


if __name__ == "__main__":
    main()
