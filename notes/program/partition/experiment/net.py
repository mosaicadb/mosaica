# The net figure of the spec (P5): (S2+3 share) x (S1b bytes at w* = 0.06), per query kind and answer size, as a
# ratio to the baseline's. usage: net.py BASELINE NAME...
import json, sys
W = 'work/scores/'
KINDS = ["text", "species", "item"]; FRS = ["0.01%", "0.1%", "1%"]
b = json.load(open(W + sys.argv[1] + '.json'))
for key in ("Si", "Sm"):
    print(f"\nNet figure, Σ_Q = {key}: (S2+3) × (S1b at 0.06), ratio to {sys.argv[1]} (below 1 is better)\n")
    print("| partition | " + " | ".join(f"{k} {f}" for k in KINDS for f in FRS) + " |")
    print("|---|" + "---|" * 9)
    for nm in sys.argv[2:]:
        d = json.load(open(W + nm + '.json'))
        r = [d[f"S23_{k}_{f}"] * d[f"{key}_S1b_0.06"] / (b[f"S23_{k}_{f}"] * b[f"{key}_S1b_0.06"]) for k in KINDS for f in FRS]
        print(f"| {nm} | " + " | ".join(f"{x:.4f}" for x in r) + " |")
