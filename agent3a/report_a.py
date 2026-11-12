# Like-for-like estimate of the owner's metric from results_a*.json: three code levels (smallest, middle, largest budget) then the full vector.
# bytes per item = (b1 + 2 s1) + (b2 - b1 + 2 s2) u1 + (b3 - b2 + 2 s3) u2 + 2n u3, u = share of the corpus undecided after a level,
# s = stored scalars of the level at 2 bytes each (det: 3 scalars). Levels are separate allocations, not nested prefixes (as in FINDINGS 6.6).
import json, sys
for f in sys.argv[1:]:
    r = json.load(open(f)); n = r["dim"]; P = r["pairs"]; acc = r["acc"]; sc = r["scalars"]
    buds = sorted({int(k[3:]) for k in acc if k.startswith("det")})[-3:]
    print(f"\n{f}: n={n}, pairs={P}, violations={sum(r['viol'].values())}, levels {[b // 8 for b in buds]} B; thresholds {r['TS']}")
    print("true matches, % of sample:      ", " ".join(f"{100 * a[0] / P:6.2f}" for a in acc[f"det{buds[0]}"]))
    schemes = ["det"] + sorted({k[:k.index(str(buds[0]))] + "|" + k[k.index("c"):] for k in acc if not k.startswith("det") and str(buds[0]) in k[3:8]})
    for s in schemes:
        if s == "det": keys = [f"det{b}" for b in buds]; scal = [3, 3, 3]
        else:
            nm, c = s.split("|"); keys = [f"{nm}{b}{c}" for b in buds]
            scal = [sc.get(f"{nm}{b}{c}", sc.get(f"{nm}{b}")) for b in buds]
        if any(k not in acc for k in keys): continue
        for hybrid in ([False] if s == "det" else [False, True]):
            for sb in (2, 1):
                out = []; sc2 = [3] + scal[1:] if hybrid else scal
                for ti in range(len(r["TS"])):
                    u = [acc[k][ti][2] / P for k in keys]; b = [x / 8 for x in buds]
                    if hybrid: u[0] = acc[f"det{buds[0]}"][ti][2] / P
                    by = (b[0] + sb * sc2[0]) + (b[1] - b[0] + sb * sc2[1]) * u[0] + (b[2] - b[1] + sb * sc2[2]) * u[1] + 2 * n * u[2]
                    out.append(100 * by / (2 * n))
                print(f"{s.replace('|', ' '):10s} {'det level 1' if hybrid else '           '} {sb}B/scalar  read %:", " ".join(f"{x:6.2f}" for x in out), "  scalars", sc2)
