# Compact tables from the c3b_*.json files that exist.
import json, glob, math, numpy as np
for fn in sorted(glob.glob("c3b_g*_c*_*.json")):
    d = json.load(open(fn)); rows = d["rows"]
    print(f"== group {d['group']} c = {d['c']:g} {d['curve']} (F = {d['F']:.4f}) ==")
    print("bytes | det alone | one block | best | det/best | J, cuts | prefix bits | rates | sum n cos^2 | slope bits/halving")
    hb = [r["byJ"][str(r["best"])]["h_over_F"] for r in rows]; by = [r["bytes"] for r in rows]
    for i, r in enumerate(rows):
        b = r["byJ"][str(r["best"])]; J = len(b["cuts"]) - 1; x = b["x"]; cuts = b["cuts"]
        ncos = sum((cuts[j + 1] - cuts[j]) * (1 - 4.0 ** (-x[1 + J + j])) for j in range(J))
        sl = 8 * (by[min(i + 1, len(by) - 1)] - by[max(i - 1, 0)]) / math.log2(hb[max(i - 1, 0)] / hb[min(i + 1, len(by) - 1)])
        j1 = r["byJ"].get("1", {}).get("h_over_F", float("nan"))
        print(f"{r['bytes']:4d} | {r['byJ']['0']['h_over_F']:.4f} | {j1:.4f} | {b['h_over_F']:.4f} | {r['byJ']['0']['h_over_F'] / b['h_over_F']:.2f} | {J} {cuts} | {[round(v) for v in x[:1 + J]]} | {[round(v, 2) for v in x[1 + J:]]} | {ncos:.0f} | {sl:.0f}")
    cross = next((r["bytes"] for r in rows if r["best"] != 0), None)
    print(f"first budget at which a hybrid beats the deterministic code alone: {cross} B")
