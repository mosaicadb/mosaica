"""Tables of scaling.py's results (markdown). usage: python3 scaling_tables.py results/scaling_*.json"""
import json
import sys

import numpy as np


def f0(x):
    return "-" if x is None else f"{x:,.0f}"


def main():
    for path in sys.argv[1:]:
        R = json.load(open(path))
        ds, d = R["dataset"], R["d"]
        P = R["parts"]
        base = {n: e for n, e in P.items() if " | " not in n and "visible" not in n}
        print(f"\n### {ds} (N {R['N']:,}, d {d}, {R['nq']} queries)\n")
        print("Rows read (clusters read) to recall@k, the order mu + 3 sigma with each cluster's exact statistics:\n")
        print("| rows N' | S | K | k=10: 0.9 | 0.95 | 0.99 | k=1: 0.95 | k=100: 0.95 | centre only, k=10: 0.95 |")
        print("|---|---|---|---|---|---|---|---|---|")
        for n, e in base.items():
            rc = e["rows_clusters"]
            c0 = P[n + " | centre only"]["rows_clusters"]["10"][1][0]
            cell = lambda x: "-" if x[0] is None else f"{x[0]:,.0f} ({x[1]:.1f})"
            print(f"| {e['n']:,} | {e['S']} | {e['K']} | {cell(rc['10'][0])} | {cell(rc['10'][1])} | {cell(rc['10'][2])} | {cell(rc['1'][1])} | {cell(rc['100'][1])} | {f0(c0)} |")
        # exponents: log rows = a + g log S + h log N'
        print("\nLeast squares of log rows on log S and log N' over these partitions (rows ~ S^gamma N'^eta):\n")
        print("| k, recall | gamma (in S) | eta (in N' at fixed S) | rows ~ N^(gamma/2 + eta) under S ~ sqrt(N) | residual (max factor) |")
        print("|---|---|---|---|---|")
        for k in ("1", "10", "100"):
            for ti, tg in enumerate(R["targets"]):
                xs, ys = [], []
                for n, e in base.items():
                    r = e["rows_clusters"][k][ti][0]
                    if r is not None and e["K"] >= 16:
                        xs.append([1.0, np.log(e["n"] / e["K"]), np.log(e["n"])])
                        ys.append(np.log(r))
                if len(xs) < 4:
                    continue
                A, y = np.array(xs), np.array(ys)
                co, *_ = np.linalg.lstsq(A, y, rcond=None)
                err = np.exp(np.abs(A @ co - y).max())
                print(f"| {k}, {tg} | {co[1]:.2f} | {co[2]:.2f} | {co[1] / 2 + co[2]:.2f} | {err:.2f} |")
        print("\nA random visible set as a subsample: the full set's partition read through a random share, against a partition made of that many rows with clusters of the same visible size:\n")
        print("| partition | visible rows | visible rows per cluster | clusters | k=10: rows (clusters) to 0.9 | 0.95 | 0.99 |")
        print("|---|---|---|---|---|---|---|")
        for n, e in P.items():
            if "visible" in n:
                rc = e["rows_clusters"]["10"]
                cell = lambda x: "-" if x[0] is None else f"{x[0]:,.0f} ({x[1]:.1f})"
                print(f"| {n} | {e['n']:,} | {e['S']:.0f} | {e['K']} | {cell(rc[0])} | {cell(rc[1])} | {cell(rc[2])} |")
        print("\nA sketch of the centres (their first r principal coordinates; the rest of the score left out), full set: the share of the between-centre variance of q.m it leaves out, and the centres that must then be scored in full to hold 99% of the clusters the exact order reads to recall 0.95 at k = 10 (m of them); work in d-dimensional centre products, K r / d + depth, against K:\n")
        print("| S | K | m | r | variance left out | depth for 99% | work / K |")
        print("|---|---|---|---|---|---|---|")
        for n, e in base.items():
            if "sketch" not in e:
                continue
            for r, s in e["sketch"].items():
                if "0.95" not in s:
                    continue
                dep = s["0.95"]["depth99"]
                print(f"| {e['S']} | {e['K']} | {s['0.95']['m']:.1f} | {r} | {s['unexplained_var']:.3f} | {dep} | {(e['K'] * int(r) / d + dep) / e['K']:.2f} |")
        print("\nTwo levels of centres (parents by k-means over the centres, keyed mu + 3 sigma of their rows; the best P expanded), full set, k = 10: centres scored, and rows to recall against the flat order's:\n")
        print("| S | K | parents | P | centres scored (of K) | rows to 0.9 / flat | 0.95 / flat | 0.99 / flat |")
        print("|---|---|---|---|---|---|---|---|")
        for n, e in P.items():
            if "two-level" not in n:
                continue
            b = P[n.split(" | ")[0]]
            rc, rb = e["rows_clusters"]["10"], b["rows_clusters"]["10"]
            cell = lambda x, y: "-" if x[0] is None or y[0] is None else f"{x[0] / y[0]:.2f}"
            print(f"| {b['S']} | {b['K']} | {b['parents']} | {n.split('P=')[1]} | {e['centres_scored']:.0f} ({e['centres_scored'] / b['K']:.2f}) | {cell(rc[0], rb[0])} | {cell(rc[1], rb[1])} | {cell(rc[2], rb[2])} |")


if __name__ == "__main__":
    main()
