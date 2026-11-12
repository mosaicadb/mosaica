# Aggregate run2.py worker outputs. usage: python3 report2.py NAME TAG [TAG ...]   prints per-item bytes by class and threshold
import numpy as np, sys, json, glob, os
VEC = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/"
NAME = sys.argv[1] if not sys.argv[0].endswith("levels.py") else sys.argv[1]
IDX = VEC + (f"combined/idx_{NAME}/" if os.path.exists(VEC + f"combined/idx_{NAME}/index.npz") else f"quant/idx_{NAME}/")
I = np.load(IDX + "index.npz"); N, n, K = int(I["N"]), int(I["n"]), int(I["K"]); qcls = I["qcls"]; FULL = 2 * n; nA = len(I["P"])
TH = ["0.01%", "0.1%", "1%", "5%", "0.95", "0.9", "0.8", "0.7"]
def load(tag):
    fs = sorted(glob.glob(VEC + f"quant/out/{NAME}_{tag}_w*.npz")); D = [dict(np.load(f)) for f in fs]
    R = {k: sum(d[k] for d in D) for k in ("enter", "ib", "cb", "gt", "viol", "A")}
    IV = {}; ESC = {}
    for d in D:
        for k, v in json.loads(str(d["IV"])).items(): IV[k] = IV.get(k, 0) + v
        for k, v in json.loads(str(d["ESC"])).items(): ESC[k] = [a + b for a, b in zip(ESC.get(k, [0, 0]), v)]
    R.update(IV=IV, ESC=ESC, ALLV=list(D[0]["ALLV"]), CFG=str(D[0]["CFG"]), nw=len(D)); return R
def rows(R, cl):
    m = qcls == cl; nts = range(8) if cl == "item" else range(4); out = []
    for t in nts:
        it = R["ib"][t][m] / N; pc = (R["cb"][t][m] + K * 4) / N; idm = 4 * R["A"][t][m] / N
        fx = R["gt"][t][m] * (n * n * 4) + nA * (n + 4)
        out.append(dict(item=it.mean(), touched=R["enter"][0, t][m].mean() / N, tot1G=(it + pc + idm + fx / 1e9).mean()))
    return out
if __name__ == "__main__":
    base = None
    for tag in sys.argv[2:]:
        R = load(tag); print(f"\n{NAME} {tag} ({R['nw']} workers) {R['CFG']} levels {R['ALLV']}\n  violations {int(R['viol'].sum())} intervals {R['IV']} escapes {R['ESC']}")
        for cl in np.unique(qcls):
            rr = rows(R, cl); s = " ".join(f"{r['item']:7.3f}" for r in rr)
            if base is not None: s += "  | vs first: " + " ".join(f"{100 * (r['item'] / b['item'] - 1):+5.1f}%" for r, b in zip(rr, base[cl]))
            print(f"  {cl:4s} B/item {s}  | touched% " + " ".join(f"{100 * r['touched']:.1f}" for r in rr))
            if cl == "text" or cl == "item": print(f"  {cl:4s} total % of data at N=1e9: " + " ".join(f"{100 * r['tot1G'] / FULL:.3f}" for r in rr))
        if base is None: base = {cl: rows(R, cl) for cl in np.unique(qcls)}
