# T4 and T5 of C4.4. T5: between successive randomised levels, bits added / log2(w_l / w_{l+1}) against sum_j n_j cos^2 phi_j
# measured at level l+1. T4: rms tan phi per block and level. usage: python3 t5.py c0 c1 c2 c3
import sys, json, math, numpy as np, dt
C = [float(x) for x in sys.argv[1:5]]
r = dt.load(); lv = r["lv"].astype(int); h = dt.halfwidth(r, C); ent, surv = dt.replay(r, h)
cod = (r["cod"][:, None] >> np.arange(4)) & 1; R = r["R"].astype(np.float64); a = r["a"].astype(np.float64)
tan2 = np.where(cod == 1, (a / np.maximum(R, 1e-30)) ** 2, np.inf); cos2 = np.where(cod == 1, 1 / (1 + tan2), 0.0)
ncos = (dt.NB * cos2).sum(1); eta = R.sum(1)[r["i0"]]; lw = np.log(h / eta)
bits = 8 * dt.BY
print(f"constants {C}; bits added on entering levels 1-3: {bits[1:].tolist()}")
for cls in (0, 1):
    m = ent & (r["text"] == cls)
    wb = [math.exp(lw[m & (lv == l)].mean()) for l in range(4)]
    nc = [ncos[m & (lv == l)].mean() for l in range(4)]
    rt = [np.sqrt(np.mean(tan2[m & (lv == l)][:, j][np.isfinite(tan2[m & (lv == l)][:, j])])) if np.isfinite(tan2[m & (lv == l)][:, j]).any() else float('nan') for l in range(4) for j in range(4)]
    print(f"{'text' if cls else 'item'}: w-bar {np.round(wb, 4).tolist()}; mean sum_j n_j cos^2 phi_j by level {np.round(nc, 1).tolist()}")
    for l in range(3):
        bph = bits[l + 1] / math.log2(wb[l] / wb[l + 1])
        print(f"   step {l}->{l + 1}: {bits[l + 1]:.0f} bits / log2({wb[l]:.4f}/{wb[l + 1]:.4f}) = {bph:.0f} bits per halving; sum n cos^2 phi at level {l + 1} = {nc[l + 1]:.0f}; ratio {bph / nc[l + 1]:.2f}")
    print("   rms tan phi by level (rows) and block (columns):")
    for l in range(4): print("     L%d " % l + "  ".join(f"{x:.4f}" for x in rt[4 * l:4 * l + 4]))
