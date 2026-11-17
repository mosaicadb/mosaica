# Combines orders2.py results: one global f' (O2) or multiplier (O3) by leave-one-dataset-out, per-dataset tables,
# and each candidate default's loss against the per-dataset oracle (best f of the rad grid on the test queries).
# The score of an order on a dataset is its mean recall@10 over the scanned shares 0.5% .. 25% ("avg").
# usage: python -I loo.py OUT.txt RESULT.json [RESULT.json ...]   (a name ending in "*" is shown but not in the LOO)
import sys, json
import numpy as np

outp = sys.argv[1]
R = [json.load(open(p)) for p in sys.argv[2:]]
inloo = [r for r in R if not r["name"].endswith("*")]
names = [r["name"] for r in R]
F2 = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 2.5, 3.0]
MULT = [0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 2.5, 3.0]
FG = [0.0, 0.1, 0.2, 0.35, 0.5, 0.75]
FAM = {"O2 sd": ("O2 sd f'={}", F2), "O2 mad": ("O2 mad f'={}", F2), "O3 r4": ("O3 r4 x{}", MULT),
       "O3 r16": ("O3 r16 x{}", MULT), "O3 exact": ("O3 exact x{}", MULT),
       "H r4": ("H r4 {}", [f"x{m} f'={f}" for m in (0.5, 0.75, 1.0) for f in (0.1, 0.2, 0.3)]),
       "H r16": ("H r16 {}", [f"x{m} f'={f}" for m in (0.5, 0.75, 1.0) for f in (0.1, 0.2, 0.3)])}
L = []
p = L.append
avg = lambda r, k: r["orders"][k]["avg"]
oracle = {r["name"]: max(FG, key=lambda f: avg(r, f"rad f={f}")) for r in R}
p("score: mean recall@10 over scanned shares 0.5%..25%; oracle: best rad f per dataset on its test queries")
p("datasets: " + ", ".join(f"{r['name']} (K {r['K']}, nq {r['nq']}, 1st/10th sim {r['sim_top1']:.2f}/{r['sim_top10']:.2f}, "
                          f"O0 f {r['f0']}, oracle f {oracle[r['name']]})" for r in R))
glob = {}
for fam, (fmt, grid) in FAM.items():
    M = np.array([[avg(r, fmt.format(g)) for g in grid] for r in inloo])
    gi = int(np.argmax(M.mean(0)))
    glob[fam] = grid[gi]
    p(f"\n{fam}: score by parameter (rows: datasets; cols {grid})")
    for r in R:
        p(f"  {r['name']:22s} " + " ".join(f"{avg(r, fmt.format(g)):.3f}" for g in grid) +
          f"   own best {grid[int(np.argmax([avg(r, fmt.format(g)) for g in grid]))]}")
    p(f"  global (all LOO datasets) best: {grid[gi]}")
    for j, r in enumerate(inloo):
        others = np.delete(M, j, 0).mean(0)
        g = grid[int(np.argmax(others))]
        own = M[j].max()
        p(f"  leave out {r['name']:22s}: chosen {g}, loss {own - M[j][grid.index(g)]:.4f} vs own best {grid[int(np.argmax(M[j]))]}")

cands = {"O0 (today)": lambda r: "O0", "O1 q.m": lambda r: "rad f=0.0", "q.m_hat": lambda r: "m_hat",
         f"O2 sd f'={glob['O2 sd']} (global)": lambda r: f"O2 sd f'={glob['O2 sd']}",
         "O2 sd own best": lambda r: max((f"O2 sd f'={g}" for g in F2), key=lambda k: avg(r, k)),
         f"O2 mad f'={glob['O2 mad']} (global)": lambda r: f"O2 mad f'={glob['O2 mad']}",
         "O3 r4 x1 (param-free)": lambda r: "O3 r4 x1.0", f"O3 r4 x{glob['O3 r4']} (global)": lambda r: f"O3 r4 x{glob['O3 r4']}",
         "O3 r4 own best": lambda r: max((f"O3 r4 x{g}" for g in MULT), key=lambda k: avg(r, k)),
         "O3 r16 x1 (param-free)": lambda r: "O3 r16 x1.0", f"O3 r16 x{glob['O3 r16']} (global)": lambda r: f"O3 r16 x{glob['O3 r16']}",
         "O3 r16 own best": lambda r: max((f"O3 r16 x{g}" for g in MULT), key=lambda k: avg(r, k)),
         "O3 exact x1": lambda r: "O3 exact x1.0", f"O3 exact x{glob['O3 exact']} (global)": lambda r: f"O3 exact x{glob['O3 exact']}",
         f"H r4 {glob['H r4']} (global)": lambda r: f"H r4 {glob['H r4']}",
         f"H r16 {glob['H r16']} (global)": lambda r: f"H r16 {glob['H r16']}",
         "H r16 own best": lambda r: max((f"H r16 x{m} f'={f}" for m in (0.5, 0.75, 1.0) for f in (0.1, 0.2, 0.3)), key=lambda k: avg(r, k)),
         "oracle (best rad f)": lambda r: f"rad f={oracle[r['name']]}",
         "faiss nlist=K": lambda r: next((k for k in r["orders"] if k.startswith("faiss")), None)}
SH = ["0.005", "0.016", "0.063", "0.25"]
for r in R:
    p(f"\n{r['name']}  (K {r['K']}; recall@10 at scanned share)")
    p(f"  {'order':30s} " + " ".join(f"{float(s) * 100:>6g}%" for s in SH) + "     avg   r=0@1.6%")
    for c, fn in cands.items():
        k = fn(r)
        if k is None:
            continue
        o = r["orders"][k]
        p(f"  {c:30s} " + " ".join(f"{o['fixed'][s]:7.3f}" for s in SH) + f"   {o['avg']:.3f}   {o['zero16']['n_zero']:3d}   [{k}]")
p("\nloss against the oracle (oracle - order), per dataset, in avg and at 1.6% / 6.3%; worst case last")
for c, fn in cands.items():
    if c.startswith("oracle") or c.startswith("faiss"):
        continue
    la, l16, l63 = [], [], []
    for r in R:
        o, ob = r["orders"][fn(r)], r["orders"][f"rad f={oracle[r['name']]}"]
        la.append(ob["avg"] - o["avg"]); l16.append(ob["fixed"]["0.016"] - o["fixed"]["0.016"]); l63.append(ob["fixed"]["0.063"] - o["fixed"]["0.063"])
    p(f"  {c:30s} " + "  ".join(f"{n}: {a:+.3f}/{b:+.3f}/{d:+.3f}" for n, a, b, d in zip(names, la, l16, l63)) +
      f"   | worst avg {max(la):+.3f}, worst 1.6% {max(l16):+.3f}, worst 6.3% {max(l63):+.3f}")
txt = "\n".join(L)
print(txt)
open(outp, "w").write(txt + "\n")
