# Referee: C5.3's first-level programme at the scalar charges the index's own layout implies. The index stores |y| for every
# block, coded or not, so coding one more block at 56 B adds only kappa and tan phi: s = 1 to 1.5 B; with |y| counted, 2 to 2.5 B.
import sys, json
sys.dont_write_bytecode = True; sys.path.insert(0, "..")
src = open("../c5b.py").read().split("RUNS = ")[0]
exec(src)
for s in (1.0, 2.0):
    row = {}
    for J in range(5):
        best = None
        for st in STARTS[J]:
            r = descend(st, 8 * (56 + s), 8 * s)
            if r is not None and (best is None or r[0] < best[0]): best = r
        if best is not None: row[J] = (best[0] / F, best[1], [round(v, 2) for v in best[2]])
    print(f"s = {s}: " + " | ".join(f"J{j} {row[j][0]:.4f}" for j in row) + f" | best J={min(row, key=lambda j: row[j][0])} {row[min(row, key=lambda j: row[j][0])][1:]}", flush=True)
