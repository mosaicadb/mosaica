import json, numpy as np
o = json.load(open("results4.json"))["64"]; TS = o["TS"]; DS = [0, 0.005, 0.01, 0.02, 0.05]
print("violations", o["violations"], "thresholds", TS)
for B, a in o["acc"]["local"].items():
    a = np.array(a); A = a[:, 0]
    print(f"{int(B)//8:4d} B  half-width {' '.join(f'{x/y:.4f}' for x, y in zip(a[:,4], A))}")
    for j, dd in enumerate(DS):
        col = a[:, 2] if j == 0 else a[:, 4 + j]
        print(f"        d={dd:<5} unsettled/A: ", " ".join(f"{x/y:7.3f}" for x, y in zip(col, A)))
