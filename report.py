import json, numpy as np
R = json.load(open("results.json"))
for K, o in R.items():
    N, NQ, TS = o["N"], o["NQ"], o["TS"]
    tot = N * NQ
    print(f"\n===== K={K}  violations={o['violations']}")
    print("err median:", {k: round(v, 3) for k, v in o["err_median"].items()})
    acc = o["acc"]
    A = np.array(acc["truth"])[:, 0]
    print("t:            ", "  ".join(f"{t:>9}" for t in TS))
    print("answer frac % ", "  ".join(f"{a/tot*100:9.4f}" for a in A))
    sl = np.array(o["slack"])
    for di, dl in enumerate(o["DELTAS"]):
        print(f"count(t-{dl})/A ", "  ".join(f"{sl[ti,di]/A[ti]:9.2f}" for ti in range(len(TS))))
    for name, v in acc.items():
        if name == "truth": continue
        v = np.array(v)
        print(f"{name:10s} survivors % of N:", "  ".join(f"{x/tot*100:8.3f}" for x in v[:, 1]))
        print(f"{'':10s} survivors / A   :", "  ".join(f"{x/a:8.2f}" for x, a in zip(v[:, 1], A)))
        if v[:, 2].sum() > 0:
            print(f"{'':10s} undecided / A   :", "  ".join(f"{x/a:8.2f}" for x, a in zip(v[:, 2], A)))
            print(f"{'':10s} certain-in / A  :", "  ".join(f"{x/a:8.2f}" for x, a in zip(v[:, 3], A)))
