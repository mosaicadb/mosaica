"""The cost model of layered.md section 8: replay A's law rows = A S^gamma N^eta (k = 10, recall 0.95 or 0.99) carried
to 1M, 10M, 100M and 1B rows under the build rule, in megabytes read per query, by term. A MODEL, not a measurement.

usage: python3 extrapolate.py results/scaling_*.json

Per dataset (its own n; sift as n = 128). K and S by the rule (fit::per_cluster_by_n). A random visible share v is a
subsample (measured, section 2): the law at (S v, N v), the same K. Terms:
  centres   K n; with the sketch K r + 3 m n (r = 32; the depth measured is 2 to 7 times the clusters read)
  sigmas    16 n per unit read and the 8 ahead
  groups    n^2 per group opened; groups opened = min(G, max(1, A_G (m / K) G)), A_G = 2.5 (the reader study: the
            share of groups opened is 2 to 2.7 times the share of clusters read)
  tables    6 kB per cluster or leaf read through the levels
  codes     18.5 B per row of a unit read through the levels
  vectors   2 n per row read directly, or per row surviving the levels (EPS of the rows scanned)
  leaves    n per leaf of an expanded cluster (2,000-row leaves; 2.5 m clusters expanded, as replay A2 measured)
Designs: `today` direct reads, G = 64; `levels` the statistical rule, G = 64; `all` the rule + leaves of 2,000 rows
where S > 2,000 + the sketch where K >= 2,000 + G = max(64, min(N / (3 n^2), K / 8)). Each design's planner takes the
cheaper, by these bytes, of reading its units through the levels or directly and (for `all`) of expanding clusters
into leaves or not.
"""
import json
import math
import sys

import numpy as np

EPS = 0.01
R_SK = 32
A_G = 2.5
NPAD = {"glove": 128, "yahoo": 384, "laion": 512, "sift": 128}
TERMS = ("centres", "sigmas", "groups", "tables", "codes", "vectors", "leaves")


def per_cluster(N):
    return 2000 * 2 ** max(0, math.ceil(math.log2(N / 1e6) / 2 - 0.5))


def mb(x):
    return f"{x / 1e6:,.1f}" if x < 99.5e6 else f"{x / 1e6:,.0f}"


def one(law, N, n, v, sketch, leaves, G, through):
    S = per_cluster(N)
    K = N / S
    rows = min(law(S * v, N * v), N * v)  # visible rows read, whole clusters
    m = rows / (S * v)
    t = dict.fromkeys(TERMS, 0.0)
    t["centres"] = K * R_SK + 3 * m * n if sketch else K * n
    unit_rows, units = S * v, m
    if leaves:
        rows = min(law(2000 * v, N * v), rows)
        unit_rows, units = 2000 * v, rows / (2000 * v)
        t["leaves"] = 2.5 * m * (S / 2000) * n
        t["sigmas"] = (m + 8) * 16 * n
    t["sigmas"] += (units + 8) * 16 * n
    if through:
        t["codes"] = rows * 18.5
        t["vectors"] = EPS * rows * 2 * n
        t["tables"] = units * 6e3
        t["groups"] = min(G, max(1.0, A_G * (m / K) * G)) * n * n
    else:
        t["vectors"] = rows * 2 * n
    return t, rows, m, K, S, G


def cost(law, N, n, v, design):
    """The cheapest of the choices a design's planner has (by its modelled bytes): each unit through the levels or
    directly; for `all`, clusters whole or expanded into leaves."""
    S = per_cluster(N)
    K = N / S
    if design == "today":
        return one(law, N, n, v, False, False, 64, False)
    if design == "levels":
        opts = [one(law, N, n, v, False, False, 64, th) for th in (True, False)]
    else:
        G = max(64, min(N / (3 * n * n), K / 8))
        opts = [one(law, N, n, v, K >= 2000, lv, G, th) for lv in ((False, True) if S > 2000 else (False,)) for th in (True, False)]
    return min(opts, key=lambda o: sum(o[0].values()))


def main():
    for ti, tg in ((1, 0.95), (2, 0.99)):
        print(f"\n#### Recall {tg}, k = 10: MB read per query (model)\n")
        print("| dataset | N | K, S | visible | rows read (k), clusters | today | levels: total, largest terms | all: total, largest terms | G (all) |")
        print("|---|---|---|---|---|---|---|---|---|")
        for path in sys.argv[1:]:
            R = json.load(open(path))
            ds = R["dataset"]
            n = NPAD[ds]
            xs, ys = [], []
            for name, e in R["parts"].items():
                if " | " in name or "visible" in name or e["K"] < 16:
                    continue
                r = e["rows_clusters"]["10"][ti][0]
                if r is not None:
                    xs.append([1.0, np.log(e["n"] / e["K"]), np.log(e["n"])])
                    ys.append(np.log(r))
            co, *_ = np.linalg.lstsq(np.array(xs), np.array(ys), rcond=None)
            law = lambda S, N, co=co: float(np.exp(co[0] + co[1] * np.log(S) + co[2] * np.log(N)))
            for N in (1e6, 1e7, 1e8, 1e9):
                for v in (1.0, 0.25, 0.01):
                    cells = []
                    for design in ("today", "levels", "all"):
                        t, rows, m, K, S, G = cost(law, N, n, v, design)
                        tot = sum(t.values())
                        if design == "today":
                            head = f"| {ds} | {N:,.0f} | {K:,.0f}, {S:,} | {v:g} | {rows / 1e3:,.0f}, {m:,.0f} | {mb(tot)}"
                        else:
                            top = sorted(t.items(), key=lambda x: -x[1])[:2]
                            cells.append(f"{mb(tot)}: " + ", ".join(f"{k} {mb(x)}" for k, x in top))
                    print(head + " | " + " | ".join(cells) + f" | {G:,.0f} |")


if __name__ == "__main__":
    main()
