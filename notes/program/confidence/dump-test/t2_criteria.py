# Refutation criteria 1 and 2 of DESIGN-CLAIMS C4.4 from the JSON that c4_dump_test.py (or its replay form) writes.
# usage: python3 t2_criteria.py out.json c0 c1 c2 c3
import sys, json, math, numpy as np
r = json.load(open(sys.argv[1])); C = [float(x) for x in sys.argv[2:6]]
UB = np.exp(np.linspace(math.log(1e-3), math.log(3.0), 61)); uc = np.concatenate([[UB[0]], np.sqrt(UB[1:] * UB[:-1]), [UB[-1]]])
L = r["levels"]
print("criterion 1 (sd of ln w <= 0.3) and criterion 2 (half-survival within 10% of w-bar; 10-90 span <= 2x predicted with T1's spread)")
for k, v in L.items():
    l = int(k.split()[-1]); gm = v["w geometric mean"]; s = v["sd of ln w"]; u50 = v["u at 50% survival"]; u90, u10 = v["u at 90% / 10% survival"]
    pred = math.log((1 + 1.2816 / C[l]) / (1 - 1.2816 / C[l])); predT1 = math.sqrt(pred ** 2 + (2 * 1.2816 * s) ** 2); meas = math.log(u10 / u90)
    print(f"  {k:14s} sd ln w {s:.3f} [{'ok' if s <= 0.3 else 'REFUTES'}] | u50 / w-bar {u50 / gm:.3f} [{'ok' if abs(u50 / gm - 1) <= 0.1 else 'REFUTES'}] | ln 10-90 span measured {meas:.3f}, predicted {pred:.3f}, with T1 spread {predT1:.3f}, ratio {meas / predT1:.2f} [{'ok' if meas <= 2 * predT1 else 'REFUTES'}]")
print("criterion 2, last part: survival of item and text entrants at the same u (bins with >= 200 entrants in both classes)")
for l in range(4):
    a = L[f"item level {l}"]; b = L[f"text level {l}"]
    Sa, Sb = np.array(a["survival by u"]), np.array(b["survival by u"]); ea, eb = np.array(a["entrants by u"]), np.array(b["entrants by u"])
    m = (ea >= 200) & (eb >= 200); d = (Sa - Sb)[m]; i = np.argmax(np.abs(d))
    # the same comparison after rescaling u by each class's own w-bar (u / w-bar), to see whether the difference is only the level's mean width
    xa = uc / a["w geometric mean"]; xb = uc / b["w geometric mean"]; grid = np.exp(np.linspace(math.log(0.5), math.log(1.6), 40))
    ma = ea >= 200; mb = eb >= 200
    ia = np.interp(np.log(grid), np.log(xa[ma]), Sa[ma]); ib = np.interp(np.log(grid), np.log(xb[mb]), Sb[mb])
    print(f"  level {l}: largest |S_item - S_text| at the same u = {abs(d[i]) * 100:.1f} points (u = {uc[m][i]:.4f}) [{'ok' if abs(d[i]) <= 0.05 else 'REFUTES'}]; "
          f"w-bar item {a['w geometric mean']:.4f}, text {b['w geometric mean']:.4f}; at the same u / w-bar the largest difference is {np.abs(ia - ib).max() * 100:.1f} points")
