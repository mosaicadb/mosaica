# diag_w.py INDEX PREP PSIS RULE: unfiltered k-NN on one pinned core (11) under the width rule RULE (ratio or w2), per
# psi: the means over the queries (100 x 3) of the stored vectors read, the rows past the deterministic levels and the
# clusters read on the index path, and of each phase's microseconds (Stats::cyc), with the median of the whole query
# and the cluster setup (CLSETUP: the cluster's tables and its width, the W2 forms' work included) per index-path
# cluster.
import sys
import numpy as np
import vexact_py
idx, prep, psis, rule = sys.argv[1], sys.argv[2], [float(x) for x in sys.argv[3].split(",")], sys.argv[4]
x = vexact_py.Index(idx, core=11)
x.set_width_rule(rule)
Q = np.load(f"{prep}/queries.npy").astype(np.float32)
q0 = np.zeros(x.dim, np.float32)
P = dict(PLAN=11, SPREAD=12, ORDER=25, CHOICE=30, ROT=3, CLSETUP=4, INIT=10, LVSETUP=18, TABLES=5, DECODE=7, BOUNDS=8, FULL=9, QUERY=15)
for psi in psis:
    fr, pd, nix, cyc = [], [], [], []
    for rep in range(3):
        for q in Q:
            qq = q0.copy(); qq[: len(q)] = q
            e = x.explain_approx(qq, 10, psi)
            fr.append(e["full_reads"]); pd.append(e["past_det"]); nix.append(e["clusters_open"])
            c, cps = x.profile_approx(qq, 10, psi)
            cyc.append(np.array(c, float) / cps * 1e6)
    c = np.array(cyc)
    print(f"{rule} psi {psi}: stored {np.mean(fr):.0f} past_det {np.mean(pd):.0f} clusters_open {np.mean(nix):.1f} | means (us): "
          + " ".join(f"{k} {c[:, v].mean():.1f}" for k, v in P.items()) + f" | QUERY median {np.median(c[:, 15]):.0f}"
          + f" | CLSETUP per open cluster {c[:, 4].sum() / max(np.sum(nix), 1):.2f} us", flush=True)
