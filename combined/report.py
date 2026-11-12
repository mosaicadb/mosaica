# Aggregate worker outputs and print the ablation under one accounting. usage: python3 report.py NAME [TAG]
import numpy as np, sys, json, glob
ROOT = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/combined/"
NAME = sys.argv[1]; TAG = sys.argv[2] if len(sys.argv) > 2 else ""
I = np.load(ROOT + f"idx_{NAME}/index.npz"); N, n, K, G = int(I["N"]), int(I["n"]), int(I["K"]), int(I["G"])
qcls = I["qcls"]; nA = len(I["P"]); nimg = int(I["nimg"]); FULL = 2 * n
fs = sorted(glob.glob(ROOT + f"out_{NAME}{TAG}_w*.npz")); D = [dict(np.load(f)) for f in fs]
LV = list(D[0]["LV"]); LD = int(D[0]["LD"]); RLV = list(D[0]["RLV"])
tracks = sorted({k.split("|")[0] for k in D[0] if "|" in k}, key=lambda s: list(D[0].keys()).index(s + "|enter"))
R = {nm: {f: sum(d[f"{nm}|{f}"] for d in D) for f in ("enter", "ib", "cb", "gt", "viol", "cent", "srch")} for nm in tracks}
A = sum(d["A"] for d in D)
IV = {}; RST = {}; RMX = {}
for d in D:
    for k, v in json.loads(str(d["IV"])).items(): IV[k] = IV.get(k, 0) + v
    for k, v in json.loads(str(d["RSTAT"])).items():
        o = RST.setdefault(k, [0, 0, 0, 0, 0]); o[0] = max(o[0], v[0]); o[1:] = [a + b for a, b in zip(o[1:], v[1:])]
    for k, v in json.loads(str(d["RMAX"])).items():
        for c, x in v.items(): RMX.setdefault(k, {}); RMX[k][c] = max(RMX[k].get(c, 0), x)
TH = ["0.01%", "0.1%", "1%", "5%", "0.95", "0.9", "0.8", "0.7"]
BOUNDS = [0] + [b for b in (96, 224, 480, 992, 2016, 4064) if b <= n - 64] + [n]
ROT = sum((b1 - b0) ** 2 for b0, b1 in zip(BOUNDS[:-1], BOUNDS[1:])) * 4      # rotation matrices of one group, float32
def fixed_bytes(nm, ti):          # per query, independent of N: bases (and rotations) of the groups touched, anchors when the table is used
    b = R[nm]["gt"][ti] * (n * n * 4 + (ROT if nm.startswith("V3") and RLV else 0))
    if "C2" in nm: b = b + (nimg if nm.endswith("img") else nA) * (n + 4)
    return b
def percluster(nm, ti):           # per query, proportional to the number of clusters: centres, range searches, table row
    b = R[nm]["cb"][ti].copy()
    if "C2" in nm: b = b + K * 4
    return b
def row(nm, cl, ti):
    m = qcls == cl
    it = R[nm]["ib"][ti][m] / N; pc = percluster(nm, ti)[m] / N; fx = fixed_bytes(nm, ti)[m]; idm = 4 * A[ti][m] / N
    touched = R[nm]["enter"][0, ti][m] / N
    return dict(item=it.mean(), clus=pc.mean(), fixed=fx.mean(), idmap=idm.mean(), touched=touched.mean(),
                tot1M=(it + pc + idm + fx / N).mean(), tot1G=(it + pc + idm + fx / 1e9).mean(), viol=int(R[nm]["viol"][ti]))
if __name__ == "__main__":
    print(f"{NAME}{TAG}: N {N} n {n} K {K} G {G} full vector {FULL} B; levels {LV}; last deterministic {LD}; random {RLV}")
    print("interval violations (pairs x levels outside their own bound):", IV)
    print("randomised error / predicted sd: max, pairs, >4, >5, >6:", RST, "by class:", RMX)
    print("c sub-Gaussian", float(D[0]["CSG"]), "scalars per randomised level by group:", {k: v[:2] for k, v in json.loads(str(D[0]["SCAL"])).items()})
    for cl in np.unique(qcls):
        m = qcls == cl; nts = range(8) if cl == "item" else range(4)
        print(f"\n=== {cl} queries ({int(m.sum())}); matches share of N:", " ".join(f"{TH[t]}:{100 * A[t][m].mean() / N:.3f}%" for t in nts))
        print("  per-item bytes read (B/item) | same as % of dataset | items touched % | violations")
        for nm in tracks:
            rr = [row(nm, cl, t) for t in nts]
            print(f"  {nm:16s}", " ".join(f"{r['item']:7.3f}" for r in rr), "|", " ".join(f"{100 * r['item'] / FULL:6.3f}" for r in rr),
                  "|", " ".join(f"{100 * r['touched']:5.1f}" for r in rr), "|", sum(r["viol"] for r in rr))
        print("  per-cluster bytes per item (centres, searches, table row) | id map B/item | fixed per query MB | total % at N=1M | total % at N=1e9")
        for nm in tracks:
            rr = [row(nm, cl, t) for t in nts]
            print(f"  {nm:16s}", " ".join(f"{r['clus']:5.3f}" for r in rr), "|", " ".join(f"{r['idmap']:5.3f}" for r in rr), "|",
                  " ".join(f"{r['fixed'] / 1e6:6.1f}" for r in rr), "|", " ".join(f"{100 * r['tot1M'] / FULL:6.3f}" for r in rr), "|",
                  " ".join(f"{100 * r['tot1G'] / FULL:6.3f}" for r in rr))
        print("  share of items entering each level (%), threshold 0.1% / 1%:")
        for nm in tracks:
            e = R[nm]["enter"]; lv = (LV if e.shape[0] == len(LV) + 1 else LV[:LV.index(LD) + 1] + RLV) + ["full"]
            for t in (1, 2):
                print(f"    {nm:16s} {TH[t]:5s}", " ".join(f"{l}:{100 * e[i, t][m].mean() / N:.2f}" for i, l in enumerate(lv)))
