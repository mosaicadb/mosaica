# Aggregates ladder_*.json: items left undecided after each prefix of exact local coordinates, and coordinates read in
# total, at fixed answer sizes, against N (own clustering per N) and against the visible-set size (1M clustering).
# Exponents: least squares on the log of the query means; bootstrap over queries for the 95% interval.
import numpy as np, json, os
from common import *
rng = np.random.default_rng(2)
GF = os.environ.get("GFIX") == "1"      # 12 local bases at every N (ladder_g_*.json for the three largest N)
L = [json.load(open(os.path.join(OUT, f"ladder_g_{j}.json" if GF and j >= 3 else f"ladder_{j}.json"))) for j in range(len(SIZES))]
E = np.array(L[0]["E"]); NB = len(E) - 1; KK = L[0]["KK"]; QS = L[0]["QS"]; ntext = sum(1 for q in QS if q < 369)
print(f"interval checks {sum(l['checks'] for l in L):,}; violations {sum(l['viol'] for l in L)}")
cost = np.concatenate([E, [E[-1]]]).astype(float)                    # coordinates read by an item decided at level lev
def series(Hs, sizes, label):
    # Hs: list over sizes of arrays (nq, nk, NB+2)
    for typ, sl in (("text", slice(0, ntext)), ("item", slice(ntext, None))):
        for ki, k in enumerate(KK):
            ok = [i for i, s in enumerate(sizes) if 3 * k <= s]
            if len(ok) < 3: continue
            x = np.log([sizes[i] for i in ok])
            print(f"\n{label}: {typ} queries, k = {k}")
            print("  " + "measure".ljust(26) + "".join(f"{sizes[i]:>16,}" for i in ok) + "   exponent")
            rows = [("coordinates read, total", lambda H: (H * cost).sum(-1))]
            for lev in (1, 3, 5, 6, 7, 8, 9, 10):
                rows.append((f"undecided after {E[lev-1]} coords", (lambda lev: lambda H: H[..., lev:].sum(-1))(lev)))
            rows.append(("unresolved at full read", lambda H: H[..., NB + 1]))
            for name, f in rows:
                A = np.stack([f(np.array(Hs[i])[sl, ki]) for i in ok], 1).astype(float)          # queries x sizes
                m = A.mean(0)
                if name.startswith("unres"): print("  " + name.ljust(26) + "".join(f"{v:16.2f}" for v in m)); continue
                def slope(M): return np.polyfit(x, np.log(np.maximum(M.mean(0), 1e-9)), 1)[0]
                bs = [slope(A[rng.integers(0, len(A), len(A))]) for _ in range(400)]; lo, hi = np.percentile(bs, [2.5, 97.5])
                loc = np.diff(np.log(np.maximum(m, 1e-9))) / np.diff(x)
                if name.startswith("coord"):
                    print("  " + name.ljust(26) + "".join(f"{v/1e6:8.2f}M {v/sizes[i]:6.1f}" for v, i in zip(m, ok)) + f"   {slope(A):.3f} [{lo:.3f}, {hi:.3f}]  last step {loc[-1]:.3f}   (M coordinates; per item)")
                else:
                    print("  " + name.ljust(26) + "".join(f"{v:9.0f} {100*v/sizes[i]:5.1f}%" for v, i in zip(m, ok)) + f"   {slope(A):.3f} [{lo:.3f}, {hi:.3f}]  last step {loc[-1]:.3f}")
series([l["H"]["sub"] for l in L], SIZES, "N grows (own clustering per N" + (", 12 local bases at every N)" if GF else ", local bases 12/12/12/16/32/64)"))
if GF: raise SystemExit
VIS = [977, 3906, 15625, 31250, 62500, 125000, 250000, 500000]
for kind in ("rand", "range"):
    series([L[-1]["H"][f"{kind}{v}"] for v in VIS] + [L[-1]["H"]["sub"]], VIS + [SIZES[-1]], f"visible set shrinks ({kind}), 1M clustering")
