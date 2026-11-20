"""Replay A2 for layered.md: leaves inside large clusters, the shape a 100M to 1B index has. Counts only.

One dataset, every row: leaves of about 250 rows (k-means), and parents made of F leaves each (k-means over the leaf
centres, F = 8 and 32: the ratio of a 16,000- or 64,000-row cluster to 2,000-row leaves). Per query, with exact
means and spreads and the key mu + 3 sigma:
  parents   whole parents read in the order of their key (a parent keyed as one cluster): today's design at scale;
  flat      the leaves in the order of their key, every leaf's centre scored;
  nested    the P parents first by key expanded (their leaves' centres scored), their leaves read in key order.
Reported to recall 0.9 / 0.95 / 0.99 at k = 10: rows read, and centres scored (parents + leaves of the expanded).

usage: python3 -I nested.py DATASET OUT.json
"""
import json
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import scaling as sc  # noqa: E402

NQ = 500
PS = (1, 2, 4, 8, 16, 32, 64, 128, 256, 512)


def main():
    import faiss
    ds, outp = sys.argv[1], sys.argv[2]
    X, V, Q, metric = sc.load(ds)
    Q = Q[:NQ]
    N = len(X)
    K = round(N / 250)
    a = sc.kmeans(V, K, metric, 12)
    p = sc.Part("leaves", np.arange(N), a, K, X)
    sc.log(ds, "leaves", K)
    cent = np.ascontiguousarray(p.cent[:, :V.shape[1]], dtype=np.float32)
    pars = {}
    for F in (8, 32):
        kp = max(2, K // F)
        km = faiss.Kmeans(V.shape[1], kp, niter=20, seed=4, spherical=(metric == "angular"), min_points_per_centroid=1, max_points_per_centroid=10 ** 6)
        km.train(cent)
        pars[F] = (kp, km.index.search(cent, 1)[1][:, 0])
    acc = {}

    def push(name, cr, ch, cs=None):
        e = acc.setdefault(name, {"cr": 0, "ch": 0, "n": 0, "cs": 0})
        e["cr"] = e["cr"] + cr.sum(0)
        e["ch"] = e["ch"] + ch.sum(0)
        e["n"] += len(cr)
        if cs is not None:
            e["cs"] += float(np.sum(cs))

    for q0 in range(0, len(Q), sc.CHUNK):
        S = Q[q0:q0 + sc.CHUNK] @ X.T
        top = np.argpartition(-S, 100, axis=1)[:, :100]
        top = np.take_along_axis(top, np.argsort(-np.take_along_axis(S, top, 1), axis=1), 1)
        mu, sd, hits, s1, s2 = p.stats(S, top)
        h = {10: hits[10]}
        live = np.broadcast_to(p.cnt > 0, mu.shape)
        _, cr, ch = sc.curve(mu + sc.PSI * sd, p.cnt, h, live=live)
        push("flat", cr, ch[10])
        for F, (kp, par) in pars.items():
            P1 = np.zeros((K, kp))
            P1[np.arange(K), par] = 1
            pn = P1.T @ p.cnt
            ps1, ps2, ph = s1 @ P1, s2 @ P1, hits[10] @ P1
            pmu = ps1 / np.maximum(pn, 1)
            psd = np.sqrt(np.maximum(ps2 / np.maximum(pn, 1) - pmu * pmu, 0))
            pkey = pmu + sc.PSI * psd
            _, pcr, pch = sc.curve(pkey, pn, {10: ph}, live=np.broadcast_to(pn > 0, pkey.shape))
            push(f"F={F} parents", pcr, pch[10])
            po = np.argsort(-np.where(pn > 0, pkey, -np.inf), axis=1, kind="stable")
            prank = np.empty_like(po)
            np.put_along_axis(prank, po, np.arange(kp)[None, :].repeat(len(po), 0), 1)
            lrank = prank[:, par]
            nch = np.bincount(par, minlength=kp)
            for P in PS:
                if P >= kp:
                    continue
                lv = (lrank < P) & (p.cnt > 0)
                _, cr2, ch2 = sc.curve(mu + sc.PSI * sd, p.cnt, h, live=lv)
                cs = kp + np.take_along_axis(np.broadcast_to(nch, po.shape), po[:, :P], 1).sum(1)
                push(f"F={F} nested P={P}", cr2, ch2[10], cs)
        sc.log("queries", q0 + sc.CHUNK)
    res = {"dataset": ds, "N": N, "leaves": K, "parents": {F: v[0] for F, v in pars.items()}, "targets": sc.TARGETS, "runs": {}}
    for name, e in acc.items():
        res["runs"][name] = {"rows_units": sc.at_targets(e["cr"] / e["n"], e["ch"] / e["n"] / 10), "centres": e["cs"] / e["n"] if e["cs"] else None}
    with open(outp, "w") as f:
        json.dump(res, f, indent=1)
    # table
    R = res["runs"]
    print(f"\n### {ds}: {N:,} rows, {K} leaves of about 250 rows\n")
    print("| leaves per parent | recall | parents only: rows (parents read) | flat leaves: rows, centres | nested, fewest centres within 5% of flat's rows: P, centres, rows / flat | nested within 25%: P, centres, rows / flat |")
    print("|---|---|---|---|---|---|")
    for F in (8, 32):
        kp = res["parents"][F]
        for ti, tg in enumerate(sc.TARGETS):
            fr = R["flat"]["rows_units"][ti][0]
            pr, pm = R[f"F={F} parents"]["rows_units"][ti]
            cells = []
            for lim in (1.05, 1.25):
                c = "-"
                for P in PS:
                    e = R.get(f"F={F} nested P={P}")
                    if e and e["rows_units"][ti][0] is not None and e["rows_units"][ti][0] <= lim * fr:
                        c = f"{P}, {e['centres']:.0f}, {e['rows_units'][ti][0] / fr:.2f}"
                        break
                cells.append(c)
            print(f"| {F} ({kp} parents) | {tg} | {pr:,.0f} ({pm:.1f}) | {fr:,.0f}, {K} | {cells[0]} | {cells[1]} |")


if __name__ == "__main__":
    main()
