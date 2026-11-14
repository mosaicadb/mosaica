# Basic statistics of the verify dump (records while the current 9.33 bound keeps an item): block shares of the
# variance per randomised level, and the share of the half-width that is random.
import numpy as np, math, sys
p = sys.argv[1]
raw = np.memmap(p, dtype=np.float32, mode='r').reshape(-1, 24)
n = len(raw)
tag = np.asarray(raw[:, 0]).view(np.uint32); row = np.asarray(raw[:, 1]).view(np.uint32)
keep = (tag & 1) == 1
print("records", n, "kept (index plan)", keep.sum())
raw = np.asarray(raw)[keep]; tag = tag[keep]; row = row[keep]
lv = raw[:, 2].astype(np.int64); t = raw[:, 3].astype(np.float64); tr = raw[:, 4].astype(np.float64); est = raw[:, 5].astype(np.float64)
hn = raw[:, 6].astype(np.float64); sd = raw[:, 7].astype(np.float64)
cod = raw[:, 8::4] > 0; a = raw[:, 9::4].astype(np.float64); R = raw[:, 10::4].astype(np.float64); ej = raw[:, 11::4].astype(np.float64)
D = np.array([95, 127, 255, 287], float)
qi = tag >> 6; size = (tag >> 1) & 31; text = qi >= 10
print("queries", np.unique(qi), "sizes", np.unique(size))
print("levels", np.unique(lv, return_counts=True))
v = a ** 2 / D * cod
V = v.sum(1)
print("sd check: max rel diff", np.abs(np.sqrt(V) - sd).max() / sd.mean())
for l in range(4):
    for cls, m0 in (("item", ~text), ("text", text)):
        m = (lv == l) & m0
        w = v[m] / V[m][:, None]
        print(f"level {l} {cls}: n {m.sum()}; coded sets {np.unique(cod[m] @ (1 << np.arange(4)), return_counts=True)}")
        print("   mean share", np.round(w.mean(0), 3), " median", np.round(np.median(w, 0), 3), " p10", np.round(np.percentile(w, 10, 0), 3), " p90", np.round(np.percentile(w, 90, 0), 3))
        hd = (R[m] * ~cod[m]).sum(1)
        print(f"   mean: 9.33*sd {9.33*sd[m].mean():.5f}  uncoded CS {hd.mean():.5f}  other det {(hn[m]-hd).mean():.5f}  |est-t| median {np.median(np.abs(est[m])):.5f}  |truth-t| median {np.median(np.abs(tr[m])):.5f}")
        z = (tr[m] - est[m]) / sd[m]
        print(f"   z=(truth-est)/sd: std {z.std():.3f} kurt {((z-z.mean())**4).mean()/z.var()**2:.3f} max|z| {np.abs(z).max():.2f}")
