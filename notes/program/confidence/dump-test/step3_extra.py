# Step 3, extras: (a) the order of the two largest substitutions (margins and widths) reversed, by an effective scale
# kappa_eff that makes the model reproduce the index's bytes on the measured margins; (b) eta by answer size; (c) absolute
# half-widths against the model's. usage: python3 step3_extra.py c0 c1 c2 c3 tag   (reads step3_<tag>.json)
import sys, json, math, numpy as np, dt
sys.path.insert(0, "."); import importlib
C = [float(x) for x in sys.argv[1:5]]; TAG = sys.argv[5]
sys.argv = ["step3.py"] + sys.argv[1:5] + ["_unused"]
src = open("step3.py").read().split("# measured")[0]          # the model functions of step3.py only
exec(src)
J = json.load(open(f"step3_{TAG}.json"))["cells"]
Sc = np.load("work/scores.npy"); T = np.load("work/thresholds.npy")
print(f"constants {C}")
print("(a) kappa_eff: the model's eta scale that reproduces the index's total bytes (X4) on the measured margins; then the same model on Gaussian margins")
for cls in (0, 1):
    for s in (3, 4, 5, 6):
        cell = J[f"{'text' if cls else 'item'} {dt.SIZES[s]}"]; target = cell["measured det"] + cell["X4 rand"]
        def tot(k): return np.mean([sum(model_bytes(np.abs(Sc[q] - T[q, s]), k)[:2]) for q in range(10 * cls, 10 * cls + 10)])
        lo, hi = 0.2, 10.0
        for _ in range(40):
            mid = math.sqrt(lo * hi)
            if tot(mid) < target: lo = mid
            else: hi = mid
        g = model_gauss(dt.SIZES[s], cls); x0 = g[0] + g[1]
        KAP_save = KAP[cls]; KAP[cls] = hi; ge = model_gauss(dt.SIZES[s], cls); KAP[cls] = KAP_save
        geff = ge[0] + ge[1]
        print(f"  {'text' if cls else 'item'} {dt.SIZES[s]:.0%}: index {target:.1f}, model X0 {x0:.1f}; kappa_eff {hi:.3f} (model's {KAP[cls]}); "
              f"Gaussian margins at kappa_eff {geff:.1f}: widths first x{geff / x0:.2f}, then margins x{target / geff:.2f} | margins first x{cell['X1 total'] / x0:.2f}, then the rest x{target / cell['X1 total']:.2f}")
r = dt.load(cols=("key", "lv", "tr", "es", "hn", "sd", "cod", "a", "R")); lv = r["lv"].astype(int)
R = r["R"].astype(np.float64); eta = R.sum(1); m0 = lv == 0
print("\n(b) eta = sum_j R_j at 56 B, entrants: mean (median) by answer size; model w_32 * F mean", round(WB[5] * float((FG * WG).sum()), 4))
for cls in (0, 1):
    print("  " + ("text" if cls else "item") + ": " + "  ".join(f"{dt.SIZES[s]:.2%} {eta[m0 & (r['text'] == cls) & (r['size'] == s)].mean():.4f} ({np.median(eta[m0 & (r['text'] == cls) & (r['size'] == s)]):.4f})" for s in range(7)))
h = dt.halfwidth(r, C); ent, surv = dt.replay(r, h)
Fm = float((FG * WG).sum())
print("\n(c) mean measured half-width of entrants against the model's absolute width w_l * mean F (kappa 1), and the normalised w = h / eta against w_l / w_32")
for cls in (0, 1):
    for s in (3, 5):
        out = []
        for l in range(4):
            e = ent & (lv == l) & (r["text"] == cls) & (r["size"] == s); e0 = r["i0"][e]
            hm = h[e].mean(); hmod = WB[6 + l] * Fm * (KAP[cls] if cls else 1.0)
            wn = float(np.exp(np.log(h[e] / eta[e0]).mean())); wmod = WB[6 + l] / WB[5]
            out.append(f"L{l}: h {hm:.5f} / model {hmod:.5f} = {hm / hmod:.2f}; w {wn:.4f} / {wmod:.4f} = {wn / wmod:.2f}")
        print(f"  {'text' if cls else 'item'} {dt.SIZES[s]:.0%}: " + " | ".join(out))
