# Aggregate run3.py (dataonly/out) or run2.py (quant/out) worker outputs under the accounting of quant/report2.py and
# quant/timing.py.  usage: python3 report3.py NAME TAG [TAG ...]   (first tag is the reference for the % columns)
# Per class and answer size (0.01 / 0.1 / 1 / 5 %): bytes per item read, items touched, total % of the f16 dataset at N = 1e9
# (report2's fixed costs, with the anchor count of that run), estimated ns per item (timing.py's model: table-decodable bytes
# at 0.58 ns/B byte table or 0.22 ns/B nibble table, f16 dot for the full vectors).
import numpy as np, sys, json, glob, os
VEC = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/"
NAME = sys.argv[1]
IDX = VEC + (f"combined/idx_{NAME}/" if os.path.exists(VEC + f"combined/idx_{NAME}/index.npz") else f"quant/idx_{NAME}/")
I = np.load(IDX + "index.npz"); N, n, K = int(I["N"]), int(I["n"]), int(I["K"]); qcls = I["qcls"]; FULL = 2 * n; nA0 = len(I["P"])
def load(tag):
    fs = sorted(glob.glob(VEC + f"dataonly/out/{NAME}_{tag}_w*.npz")) or sorted(glob.glob(VEC + f"quant/out/{NAME}_{tag}_w*.npz"))
    D = [dict(np.load(f)) for f in fs]
    R = {k: sum(d[k] for d in D) for k in ("enter", "ib", "cb", "gt", "viol", "A")}
    IV = {}
    for d in D:
        for k, v in json.loads(str(d["IV"])).items(): IV[k] = IV.get(k, 0) + v
    R.update(IV=IV, ALLV=list(D[0]["ALLV"]), CFG=json.loads(str(D[0]["CFG"])), nw=len(D), RMAX=max(float(d["RMAX"]) for d in D),
             NA=int(D[0]["NA"]) if "NA" in D[0] else nA0); return R
def rows(R, cl, nts=4):
    m = qcls == cl; out = []; cfg = R["CFG"]; DLV = ([0] if cfg["L0"] else []) + cfg["SCHED"]; full = 76 * n / 768
    for t in range(nts):
        it = R["ib"][t][m] / N; pc = (R["cb"][t][m] + K * 4) / N; idm = 4 * R["A"][t][m] / N
        fx = R["gt"][t][m] * (n * n * 4) + R["NA"] * (n + 4)
        e = R["enter"][:, t][:, m].mean(1) / N; ib = it.mean(); ns = {}
        for TB in (0.58, 0.22):
            s = 0; bdet = 0; prev = 0
            for l, b in enumerate(DLV):
                inc = (b - prev) + cfg["NORM"] / 8 + (2 if l == 0 else 0); bdet += e[l] * inc; s += e[l] * inc * TB; prev = b
            s += (ib - bdet - e[-1] * 2 * n) * TB + e[-1] * full; ns[TB] = s
        out.append(dict(item=ib, touched=e[0], tot1G=100 * (it + pc + idm + fx / 1e9).mean() / FULL, ns=ns[0.58], nsn=ns[0.22]))
    return out
def fmt(v, f="7.2f"): return " ".join(format(x, f) for x in v)
if __name__ == "__main__":
    base = None
    for tag in sys.argv[2:]:
        R = load(tag); c = R["CFG"]
        print(f"{NAME} {tag:8s} PROF={c['PROF']} RPROF={c.get('RPROF', 'Qmix')} ANCH={c.get('ANCH', 'all')} NA={R['NA']} | "
              f"violations {int(R['viol'].sum())} bad intervals {R['IV']} max rand err {R['RMAX']:.2f} sd")
        for cl in np.unique(qcls):
            rr = rows(R, cl); b = [r["item"] for r in rr]
            s = f"   {cl:4s} B/item {fmt(b)}"
            if base is not None and cl in base: s += " (" + " ".join(f"{100 * (x / y - 1):+5.1f}%" for x, y in zip(b, base[cl])) + ")"
            s += f" | touched% {fmt([100 * r['touched'] for r in rr], '5.1f')} | tot%@1e9 {fmt([r['tot1G'] for r in rr], '6.3f')}"
            s += f" | ns {fmt([r['ns'] for r in rr], '6.1f')} (nibble {fmt([r['nsn'] for r in rr], '5.1f')})"
            print(s)
        if base is None: base = {cl: [r["item"] for r in rows(R, cl)] for cl in np.unique(qcls)}
