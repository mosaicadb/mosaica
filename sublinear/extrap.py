# Extrapolations to N = 10^9 at k = 10 and k = 1,000, from the query-mean counts at the six measured sizes. Two readings:
# the fitted exponent held constant, and the exponent of the last doubling held constant. Neither is a measurement.
import numpy as np, json, os
from common import *
Z = json.load(open(os.path.join(OUT, "zero_all.json")))["recs"]
x = np.log(SIZES)
def line(name, m):
    rho = np.polyfit(x, np.log(m), 1)[0]; last = np.log(m[-1] / m[-2]) / np.log(SIZES[-1] / SIZES[-2])
    f = m[-1] / SIZES[-1]; g = lambda r, n: f * (n / SIZES[-1]) ** (r - 1)
    print(f"  {name:34s} at 1M: {100*f:7.3f}% of N   exponent {rho:.3f} / last doubling {last:.3f}   at 1e9: {100*g(rho,1e9):7.3f}% / {100*g(last,1e9):7.3f}% of N = {g(rho,1e9)*1e9:.3g} / {g(last,1e9)*1e9:.3g}")
for typ in ("text", "item"):
    R = [r for r in Z if r["type"] == typ]
    for k in (10, 1000):
        print(f"\n{typ}, k = {k}")
        for key in ("annulus_items", "table_items", "oracle_items", "oracle02_items"):
            line(key, np.array([[r["sub"][str(s)][str(k)][key] for s in SIZES] for r in R], float).mean(0))
        for wi, w in ((0, 1), (1, 2), (2, 3), (3, 4)):
            line(f"shell, {w} sd", np.array([[r["sub"][str(s)][str(k)]["shell_sd"][wi] for s in SIZES] for r in R], float).mean(0))
        for tag, files in (("bases 12..64", [f"ladder_{j}.json" for j in range(6)]), ("12 bases", [f"ladder_{j}.json" if j < 3 else f"ladder_g_{j}.json" for j in range(6)])):
            if not all(os.path.exists(os.path.join(OUT, f)) for f in files): continue
            L = [json.load(open(os.path.join(OUT, f))) for f in files]; E = np.array(L[0]["E"]); cost = np.concatenate([E, [E[-1]]]); ki = L[0]["KK"].index(k)
            nt = sum(1 for q in L[0]["QS"] if q < 369); sl = slice(0, nt) if typ == "text" else slice(nt, None)
            H = [np.array(l["H"]["sub"])[sl, ki].mean(0) for l in L]
            line(f"ladder, undecided at 0 coords ({tag})", np.array([h[1:].sum() for h in H]))
            line(f"ladder, undecided at 32 coords ({tag})", np.array([h[7:].sum() for h in H]))
            m = np.array([(h * cost).sum() for h in H]); rho = np.polyfit(x, np.log(m), 1)[0]; last = np.log(m[-1] / m[-2]) / np.log(SIZES[-1] / SIZES[-2])
            print(f"  coordinates per item ({tag}): at 1M {m[-1]/SIZES[-1]:.1f}; exponent of the total {rho:.3f} / last doubling {last:.3f}; at 1e9: {m[-1]/SIZES[-1]*1000**(rho-1):.2f} / {m[-1]/SIZES[-1]*1000**(last-1):.2f} per item")
