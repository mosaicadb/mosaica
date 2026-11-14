# Aggregates zero_all.json: mean counts per query type, answer size and N (or visible-set size), and the exponent of
# count ~ N^rho fitted by least squares on the log of the query-mean counts, with a bootstrap over queries (95% interval).
import numpy as np, json, os, sys
from common import *
D = json.load(open(os.path.join(OUT, "zero_all.json"))); R = D["recs"]
print(f"interval checks {D['checks']:,}; violations {D['viol']}")
rng = np.random.default_rng(1)
KK = [10, 100, 1000, 10000]
WS = [1, 2, 3, 4, 5, 6, 8]; WA = [0.02, 0.05, 0.1, 0.2, 0.3]
def get(r, key):
    if key.startswith("shell_sd"): return r["shell_sd"][WS.index(int(key[8:]))]
    if key.startswith("shell_abs"): return r["shell_abs"][WA.index(float(key[9:]))]
    return r[key]
def table(typ, group, names, sizes, keys, kk=KK, minN=0):
    recs = [r for r in R if r["type"] == typ]
    for k in kk:
        ns = [(nm, s) for nm, s in zip(names, sizes) if str(k) in recs[0][group][nm] and s >= minN]
        if len(ns) < 3: continue
        print(f"\n{typ} queries ({len(recs)}), k = {k}: mean count (and % of the set) by size; exponent with bootstrap 95% interval")
        print("  " + "measure".ljust(20) + "".join(f"{s:>16,}" for _, s in ns) + "   exponent")
        z = np.array([[r[group][nm][str(k)]["z"] for nm, _ in ns] for r in recs]).mean(0)
        print("  " + "t, sd above median".ljust(20) + "".join(f"{v:16.2f}" for v in z))
        for key in keys:
            A = np.array([[get(r[group][nm][str(k)], key) for nm, _ in ns] for r in recs], float)      # queries x sizes
            x = np.log([s for _, s in ns])
            def slope(M): return np.polyfit(x, np.log(np.maximum(M.mean(0), 1e-9)), 1)[0]
            bs = [slope(A[rng.integers(0, len(A), len(A))]) for _ in range(400)]
            lo, hi = np.percentile(bs, [2.5, 97.5]); m = A.mean(0)
            loc = np.diff(np.log(np.maximum(m, 1e-9))) / np.diff(x)
            print("  " + key.ljust(20) + "".join(f"{v:9.0f} {100*v/s:5.1f}%" for v, (_, s) in zip(m, ns)) + f"   {slope(A):.3f} [{lo:.3f}, {hi:.3f}]  last step {loc[-1]:.3f}")
keys = ["ball_items", "annulus_items", "table_items", "table_centres_read", "oracle_items", "oracle_clusters", "oracle02_items",
        "shell_sd1", "shell_sd2", "shell_sd3", "shell_sd4", "shell_sd6", "shell_abs0.05", "shell_abs0.1", "shell_abs0.2"]
print("\n================ N grows; each N has its own clustering, ~1,000 items per cluster ================")
for typ in ("text", "item"): table(typ, "sub", [str(s) for s in SIZES], SIZES, keys)
vk = ["annulus_items", "table_items", "table_centres_read", "clusters_nonempty", "oracle_items", "oracle_all_items", "shell_sd3"]
def get2(r, key): return r
VIS = [977, 3906, 15625, 31250, 62500, 125000, 250000, 500000]
for kind in ("rand", "range"):
    print(f"\n================ visible set shrinks ({kind}); the 1M clustering, table extremes over all items ================")
    for typ in ("text", "item"):
        recs = [r for r in R if r["type"] == typ]
        for k in KK[:3]:
            ns = [(f"{kind}{v}", v) for v in VIS if str(k) in recs[0]["vis"][f"{kind}{v}"]]
            print(f"\n{typ}, k = {k}")
            print("  " + "measure".ljust(20) + "".join(f"{s:>16,}" for _, s in ns) + "   exponent (sets of 15,625 and up)")
            for key in vk:
                A = np.array([[(r["vis"][nm]["clusters_nonempty"] if key == "clusters_nonempty" else get(r["vis"][nm][str(k)], key)) for nm, _ in ns] for r in recs], float)
                x = np.log([s for _, s in ns]); sel = np.array([s >= 15625 for _, s in ns])
                def slope(M): return np.polyfit(x[sel], np.log(np.maximum(M.mean(0)[sel], 1e-9)), 1)[0]
                bs = [slope(A[rng.integers(0, len(A), len(A))]) for _ in range(300)]; lo, hi = np.percentile(bs, [2.5, 97.5]); m = A.mean(0)
                print("  " + key.ljust(20) + "".join(f"{v:9.0f} {100*v/s:5.1f}%" for v, (_, s) in zip(m, ns)) + f"   {slope(A):.3f} [{lo:.3f}, {hi:.3f}]")
