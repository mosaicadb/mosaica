# Prints compact tables from the qcol2 result files: bytes per item by variant, bytes by distance band, and the code bytes
# at which the mean upper slack falls to a share R of its zero-read value.
import json, sys, numpy as np
np.set_printoptions(precision=1, suppress=True, linewidth=200)
for f in sys.argv[1:]:
    d = json.load(open(f)); print(f"\n#### {f}: basis {d['basis'].split('/')[-1]}, queries {d['nq']}, violations {sum(d['violations'].values())} in {d['checks']} interval checks, "
                                  f"largest randomised error {d['max_rand_sd']:.2f} proxy sd over {d['rand_checks']} values, scalars {d['scalars']} B")
    for kd, o in d["out"].items():
        V = o["variants"]; ref = np.array(V["fixW/det"]["cost"]); bi = np.array(o["band_items"])
        print(f"== {kd}: bytes per item at answer 0.01 / 0.1 / 1 / 5 % (change against fixW), undecided at zero read %")
        for nm, r in V.items():
            c = np.array(r["cost"]); print(f"  {nm:14s} {np.round(c, 1)}   ({np.round(100 * (c / ref - 1)).astype(int)} %)   und0 {np.round(100 * np.array(r['und0']), 1)}")
        print("   bytes per item by band |s-t|/sd: 0-0.5, 0.5-1, 1-2, 2-4, 4-8, >8   (rows: answer 0.01, 0.1, 1, 5 %)")
        for nm in ("fixW/det", "fix/det", "adapt/det", "adapt/rand", "adapts/rand1s", "adaptP/rand"):
            if nm not in V: continue
            with np.errstate(invalid="ignore", divide="ignore"): pb = np.array(V[nm]["band_cost"]) / bi
            print(f"  {nm}"); print(np.round(pb, 1))
        print("   items by band (%)"); print(np.round(100 * bi, 1))
        print("   code bytes (scalars excluded) at which the mean upper slack is R times its zero-read value; zero-read value in sd:")
        for nm, w in o["wmean"].items():
            w = np.array(w); bm = np.array(o["bmean"][nm.split("/")[0]]); r = w / w[0]
            inv = lambda R: float(np.interp(-R, -r, bm))
            print(f"  {nm:14s} zero-read {w[0]:.2f} sd;  R=0.75: {inv(0.75):5.1f} B   R=0.6: {inv(0.6):5.1f} B   R=0.5: {inv(0.5):5.1f} B   R=0.4: {inv(0.4):5.1f} B   R=0.33: {inv(0.33):5.1f} B   R=0.25: {inv(0.25):5.1f} B")
