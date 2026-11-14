# Checks dumped answers against the f64 brute force of prep.py.
# usage: python3 -I check.py PREP_DIR DUMP_DIR OUT_CSV
# For every DUMP_DIR/<what>_<vis>.bin, counts the items that differ from the exact answer (the exact top-k by f64
# similarity, within the visible set). A differing item is "within rounding" when |s - t| is at most what rounding the
# stored vectors and the query can move its similarity: sum_i |q_i x_i| times the unit roundoff of each (f16: 2^-11 on
# both; bf16 vectors: 2^-8, allowing truncation, with an f32 query; f32: 2^-24 on both), plus an f32 accumulation
# bound of n 2^-24 sum_i |q_i x_i| for the f32 and bf16 systems.
import glob, os, sys
import numpy as np

prep, dump, out = sys.argv[1:4]
Q = np.load(f"{prep}/queries.npy")
T = np.load(f"{prep}/thresholds.npy")
order = np.load(f"{prep}/order.npy", mmap_mode="r")
X = np.load(f"{prep}/corpus32.npy", mmap_mode="r")
N, n = X.shape
ks = [round(s * N) for s in (1e-4, 1e-3, 1e-2, 1e-1, 0.3, 0.5, 0.75)]
vis = {"all": None}
for v in ("vis_50", "vis_10", "vis_1"):
    m = np.zeros(N, bool)
    m[np.load(f"{prep}/{v}.npy")] = True
    vis[v] = m


def unit_roundoff(what):
    if "tbf" in what:
        return 2.0 ** -8, 2.0 ** -24, n * 2.0 ** -24
    if what.startswith(("index", "flat")):
        return 2.0 ** -11, 2.0 ** -11, 0.0
    return 2.0 ** -24, 2.0 ** -24, n * 2.0 ** -24


rows = ["what,vis,share_index,answers,items_expected,items_differing,beyond_rounding,max_abs_s_minus_t"]
for f in sorted(glob.glob(f"{dump}/*.bin")):
    base = os.path.basename(f)[:-4]
    if base.endswith("_all"):
        what, v = base[:-4], "all"
    else:
        what, v = base.rsplit("_vis_", 1)
        v = "vis_" + v
    ux, uq, uacc = unit_roundoff(what)
    a = np.fromfile(f, "<u4")
    stats = {}
    p = 0
    while p < len(a):
        qi, si, c = (int(x) for x in a[p:p + 3])
        got = a[p + 3:p + 3 + c].astype(np.int64)
        p += 3 + c
        exp = np.asarray(order[qi, :ks[si]], dtype=np.int64)
        if vis[v] is not None:
            exp = exp[vis[v][exp]]
        diff = np.setxor1d(got, exp)
        st = stats.setdefault(si, [0, 0, 0, 0, 0.0])
        st[0] += 1
        st[1] += len(exp)
        st[2] += len(diff)
        if len(diff):
            x = np.asarray(X[np.sort(diff)], dtype=np.float64)
            s = x @ Q[qi]
            mag = np.abs(x * Q[qi]).sum(1)
            gap = np.abs(s - T[qi, si])
            st[3] += int((gap > mag * (ux + uq + uacc) + 1e-12).sum())
            st[4] = max(st[4], float(gap.max()))
    for si in sorted(stats):
        st = stats[si]
        rows.append(f"{what},{v},{si},{st[0]},{st[1]},{st[2]},{st[3]},{st[4]:.3g}")
        print(rows[-1], flush=True)
with open(out, "w") as fo:
    fo.write("\n".join(rows) + "\n")
