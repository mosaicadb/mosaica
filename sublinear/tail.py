# Step 5. The shape of each query's score tail and the intrinsic-dimension statistics that decide whether theory's
# sublinear regimes apply: where the k-th best score sits as the answer fraction p = k/N shrinks, how many items lie
# within w score-sd below it (the shell), the Karger-Ruhl expansion constant at the k-th neighbour, the local intrinsic
# dimension (Levina-Bickel), the Chavez-Navarro intrinsic dimension, and how the k-th neighbour distance shrinks with N.
import numpy as np, json, os
from scipy.stats import norm
from common import *
Q, qtype, qkind, qi = queries()
rank = np.load(os.path.join(OUT, "rank.npy")); vis = rank < N; NV = int(vis.sum()); rv = rank[vis]
KS = [1, 3, 10, 30, 100, 300, 1000, 3000, 10000, 30000, 100000]; WS = [1, 2, 3, 4, 5, 6]
rec = {t: {"z": [], "shell": [], "c": [], "lid": [], "cn": [], "nnslope": [], "sd": [], "med": [], "max": [], "kurt": [], "top": []} for t in ("text", "item")}
for q in range(NQ):
    s = score_row(q)[vis]; o = np.argsort(-s); ss = s[o]; asc = ss[::-1]; sd = float(s.std()); med = float(np.median(s)); R = rec[qtype[q]]
    R["sd"].append(sd); R["med"].append(med); R["max"].append(float(ss[0])); R["kurt"].append(float(((s - s.mean()) ** 4).mean() / sd ** 4))
    R["z"].append([(ss[k - 1] - med) / sd for k in KS])
    R["shell"].append([[(NV - np.searchsorted(asc, ss[k - 1] - w * sd, side="left")) / NV for w in WS] for k in KS])
    # expansion constant |B(q, 2r)| / |B(q, r)| at r = distance to the k-th neighbour (chordal distance, d^2 = 2 - 2s)
    R["c"].append([(NV - np.searchsorted(asc, 4 * ss[k - 1] - 3, side="left")) / k for k in (10, 100, 1000)])
    d = np.sqrt(np.maximum(0, 2 - 2 * ss[:1000].astype(np.float64)))
    R["lid"].append([1 / np.mean(np.log(d[k - 1] / np.maximum(d[:k - 1], 1e-9))) for k in (20, 100, 1000)])
    da = np.sqrt(np.maximum(0, 2 - 2 * s.astype(np.float64))); R["cn"].append(float(da.mean() ** 2 / (2 * da.var())))
    # k-th neighbour distance on nested subsamples
    rk = rv[o[:200000]]; dk = []
    for sz in SIZES:
        top = ss[:200000][rk < sz]; dk.append(np.sqrt(max(0, 2 - 2 * float(top[9]))))
    R["nnslope"].append(float(np.polyfit(np.log(SIZES), np.log(np.maximum(dk, 1e-9)), 1)[0])); R["top"].append(dk)
out = {}
np.set_printoptions(precision=3, suppress=True, linewidth=200)
for t, R in rec.items():
    z = np.array(R["z"]); sh = np.array(R["shell"]); print(f"\n== {t} queries ({len(z)}): score sd {np.mean(R['sd']):.4f}, median {np.mean(R['med']):.3f}, max {np.mean(R['max']):.3f} "
          f"(max is {np.mean((np.array(R['max']) - np.array(R['med'])) / np.array(R['sd'])):.2f} sd above the median), kurtosis {np.mean(R['kurt']):.2f}")
    print("answer fraction p = k/N     " + "".join(f"{k/NV:10.0e}" for k in KS))
    print("threshold, sd above median  " + "".join(f"{v:10.2f}" for v in z.mean(0)))
    print("  Gaussian tail would give  " + "".join(f"{norm.isf(k/NV):10.2f}" for k in KS))
    for wi, w in enumerate(WS):
        m = sh[:, :, wi].mean(0); print(f"shell fraction, w = {w} sd      " + "".join(f"{100*v:9.3f}%" for v in m))
        g = 1 + np.diff(np.log(m)) / np.diff(np.log(NV / np.array(KS, float))); print(f"  local exponent of the shell count in N " + "".join(f"{v:8.2f}" for v in g))
    c = np.array(R["c"]); print("expansion constant at the 10th / 100th / 1000th neighbour: median", np.median(c, 0), "mean log2", np.log2(c).mean(0))
    print("local intrinsic dimension (k = 20 / 100 / 1000): median", np.median(np.array(R["lid"]), 0))
    print(f"Chavez-Navarro intrinsic dimension mu^2 / (2 sigma^2) of the distance histogram: median {np.median(R['cn']):.1f}")
    ns = np.array(R["nnslope"]); print(f"10th-neighbour distance ~ N^slope: slope median {np.median(ns):.4f}, mean {ns.mean():.4f} -> dimension {-1/np.median(ns):.1f}; mean distance by N {np.mean(R['top'], 0)}")
    out[t] = {"KS": KS, "z": z.mean(0).tolist(), "shell": sh.mean(0).tolist(), "c": np.median(c, 0).tolist(), "lid": np.median(np.array(R["lid"]), 0).tolist(),
              "cn": float(np.median(R["cn"])), "nnslope": float(np.median(ns))}
json.dump(out, open(os.path.join(OUT, "tail" + os.environ.get("SFILE", "S.f32")[1:-4] + ".json"), "w"))
