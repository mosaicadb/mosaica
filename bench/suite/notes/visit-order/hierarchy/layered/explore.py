"""Replay B, part 1, for layered.md: what the stored key and level codes say about a row's score, statistically.

From `study_layered_dump`'s files, over the first DIAG clusters of each query: per stage (the key alone, each level),
the share f of the cluster's score variance the stage's estimate explains, the slope of the score on the estimate,
the error over the certified half-width (how loose Cauchy-Schwarz is), and the error in units of the derived spread
tau (the key: the cluster's variance less what the key explains; a level: the half-width over sqrt(d)), for all rows and for the true top 100.

usage: python3 explore.py DATASET DUMP_PREFIX [clusters per query, default 16]
"""
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from dump import Dump  # noqa: E402


def q(a, ps):
    return " ".join(f"{x:.3f}" for x in np.quantile(a, ps))


def main():
    ds, prefix = sys.argv[1], sys.argv[2]
    mc = int(sys.argv[3]) if len(sys.argv) > 3 else 16
    D = Dump(prefix, ds)
    truth = D.truth()
    print(f"{ds}: {D.nq} queries, K {D.K}, {D.M} clusters dumped, levels end at {D.level_bytes} B, n {D.n}; first {mc} clusters of each query")
    L = D.L
    names = ["key"] + [f"{D.level_bytes[l]} B" for l in range(L)]
    acc = {s: dict(f=[], slope=[], r_all=[], r_top=[], z_all=[], z_top=[], hw=[], rr=[]) for s in names}
    sig_ratio = []
    for qi in range(D.nq):
        top = set(truth[qi, :100].tolist())
        for j in range(mc):
            ent, ex, key, klo, khi, lv = D.cluster(qi, j)
            if len(ex) < 50:
                continue
            c = D.idx[qi, j, 0]
            mu, sg = D.clu[qi, c, 0], D.clu[qi, c, 1]
            v = ex.var()
            sig_ratio.append(sg / np.sqrt(v))
            istop = np.isin(ent, list(top))
            cm = mu / D.mn[c]
            ests = [(cm * key, 0.5 * (khi - klo))] + [(0.5 * (lv[:, l, 0] + lv[:, l, 1]), 0.5 * (lv[:, l, 1] - lv[:, l, 0])) for l in range(L)]
            for s, (a, hw) in zip(names, ests):
                e = ex - a
                cov = np.cov(a, ex)
                acc[s]["f"].append(1 - e.var() / v)
                acc[s]["slope"].append(cov[0, 1] / max(cov[0, 0], 1e-30))
                r = e / np.maximum(hw, 1e-12)
                acc[s]["rr"].append(np.sqrt(np.mean(r * r) * D.d))
                # the derived spread. Key: the library's sigma^2 less the variance of the estimates, spread over the rows
                # in proportion to their certified half-widths. Level: the certified half-width over sqrt(d).
                if s == "key":
                    tau2 = max(float(sg) ** 2 - a.var(), 1e-12)
                    tau = hw * np.sqrt(tau2 / np.mean(hw * hw))
                else:
                    tau = hw / np.sqrt(D.d)
                z = (e - e.mean() * 0) / tau
                acc[s]["r_all"].append(r[::7])
                acc[s]["z_all"].append(z[::7])
                acc[s]["hw"].append(np.median(hw) / np.sqrt(v))
                if istop.any():
                    acc[s]["r_top"].append(r[istop])
                    acc[s]["z_top"].append(z[istop])
    print(f"library sigma / empirical sd of the cluster's scores: quantiles 5/50/95% {q(np.array(sig_ratio), [0.05, 0.5, 0.95])}")
    ps = [0.01, 0.5, 0.99, 0.999]
    for s in names:
        a = acc[s]
        ra, rt, za, zt = (np.concatenate(a[k]) for k in ("r_all", "r_top", "z_all", "z_top"))
        print(f"{s:>6}: f (variance explained) 10/50/90% {q(np.array(a['f']), [0.1, 0.5, 0.9])} | slope 10/50/90% {q(np.array(a['slope']), [0.1, 0.5, 0.9])} | certified half-width / cluster sd, median {np.median(a['hw']):.2f}")
        print(f"        per (query, cluster): rms(error / half-width) x sqrt(d), quantiles 5/25/50/75/95% {q(np.array(a['rr']), [0.05, 0.25, 0.5, 0.75, 0.95])}")
        print(f"        error / certified half-width: all rows rms {np.sqrt((ra ** 2).mean()):.3f} q1/50/99/99.9% {q(ra, ps)} | top-100 rows mean {rt.mean():.3f} rms {np.sqrt((rt ** 2).mean()):.3f} q {q(rt, ps)}")
        print(f"        error / derived tau:          all rows rms {np.sqrt((za ** 2).mean()):.3f} q1/50/99/99.9% {q(za, ps)} | top-100 rows mean {zt.mean():.3f} rms {np.sqrt((zt ** 2).mean()):.3f} q {q(zt, ps)}")
        for psi in (1, 1.5, 2, 2.5, 3):
            print(f"          share with error > {psi} tau: all {np.mean(za > psi):.4f} (normal {0.5 * (1 - __import__('math').erf(psi / 2 ** 0.5)):.4f}), top-100 {np.mean(zt > psi):.4f}")


if __name__ == "__main__":
    main()
