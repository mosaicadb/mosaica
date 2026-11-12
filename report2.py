import json, sys, numpy as np
R = json.load(open(sys.argv[1]))
for G, o in R.items():
    tot = o["N"] * o["NQ"]; TS = o["TS"]
    print(f"\n== G={G} violations={o['violations']}  thresholds {TS}")
    print("err median:", {k: round(v, 3) for k, v in o["err_median"].items()})
    for v, d2 in o["acc"].items():
        print(f"-- {v}: undecided/A | survivors % of N | mean half-width over matches")
        for B, a in d2.items():
            a = np.array(a); A = a[:, 0]
            print(f"   {int(B)//8:3d} B: ", " ".join(f"{x/y:7.2f}" for x, y in zip(a[:, 2], A)), " | ",
                  " ".join(f"{x/tot*100:6.2f}" for x in a[:, 1]), " | ", " ".join(f"{x/y:.3f}" for x, y in zip(a[:, 4], A)))
