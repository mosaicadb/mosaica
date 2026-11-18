# Compares approximate k-NN runs of the suite's `order` suite: per dataset, filter and nprobe, each run's mean recall and
# median time (the median over queries of each query's median), and the time each run needs to reach a recall, by
# linear interpolation between nprobe settings on its (time, recall) curve.
# usage: python3 -I compare_runs.py LABEL=RESULTS_DIR[:SETTING] ... [--targets 0.8,0.9,0.95]
#   e.g. baseline=bench/suite/results/order-baseline:default spread=bench/suite/results/order:spread
import argparse
import csv
import glob
import json
import os
from collections import defaultdict

import numpy as np


def load(spec):
    label, rest = spec.split("=", 1)
    path, _, setting = rest.partition(":")
    cells = defaultdict(lambda: ([], []))
    for f in glob.glob(os.path.join(path, "*", "vexact.csv")):
        for r in csv.DictReader(open(f)):
            if r["type"] != "approx" or (setting and r["setting"] != setting):
                continue
            p = json.loads(r["param"])
            c = cells[(r["dataset"], r["filter"], p["nprobe"])]
            c[0].append(float(r["median_ms"]))
            c[1].append(float(r["recall"]))
    return label, {k: (float(np.median(t)) * 1e3, float(np.mean(rc))) for k, (t, rc) in cells.items()}


def time_at(curve, target):
    """us to reach mean recall `target` on a run's curve (sorted by nprobe), interpolated; None if never reached."""
    pts = sorted(curve)
    for (t0, r0), (t1, r1) in zip([(0.0, 0.0)] + [(t, r) for _, t, r in pts], [(t, r) for _, t, r in pts]):
        if r1 >= target:
            return t1 if r1 == r0 else t0 + (t1 - t0) * (target - r0) / (r1 - r0)
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("runs", nargs="+")
    ap.add_argument("--targets", default="0.8,0.9,0.95,0.99")
    a = ap.parse_args()
    runs = [load(s) for s in a.runs]
    keys = sorted({k for _, r in runs for k in r})
    labels = [l for l, _ in runs]
    for ds in sorted({k[0] for k in keys}):
        for f in ["none", "random-1", "random-25"]:
            ks = [k for k in keys if k[0] == ds and k[1] == f]
            if not ks:
                continue
            print(f"\n{ds}, {f}: recall / median us by nprobe")
            print(f"  {'nprobe':>6}  " + "  ".join(f"{l:>16}" for l in labels))
            for k in ks:
                row = [r.get(k) for _, r in runs]
                print(f"  {k[2]:>6}  " + "  ".join(f"{x[1]:.3f} / {x[0]:7.1f}" if x else f"{'-':>16}" for x in row))
            print("  us to reach recall " + ", ".join(
                f"{t}: " + " / ".join(
                    (lambda v: f"{v:.0f}" if v is not None else "-")(time_at([(k[2], *r[k]) for k in ks if k in r], t))
                    for _, r in runs)
                for t in [float(x) for x in a.targets.split(",")]) + f"   ({' / '.join(labels)})")


main()
