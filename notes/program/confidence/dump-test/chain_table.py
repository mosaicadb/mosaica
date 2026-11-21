# The factor between the model (X0) and the index (X4) as a product of the substitution steps, from step3_<tag>.json.
import sys, json
for tag in sys.argv[1:]:
    J = json.load(open(f"step3_{tag}.json")); print(f"constants {J['constants']}")
    print("  cell | X0 model | X4 index | factor | margins X1/X0 | det levels X2/X1 | eta X3a/X2 | normalised widths X3/X3a | estimate X4/X3 | X3 rand without uncoded CS / c sd / other det")
    for k, c in J["cells"].items():
        x0 = c["X0 total"]; x1 = c["X1 total"]; x2 = c["measured det"] + c["X2 rand"]; x3a = c["measured det"] + c["X3a rand"]; x3 = c["measured det"] + c["X3 rand"]; x4 = c["measured det"] + c["X4 rand"]
        print(f"  {k:10s} | {x0:6.1f} | {x4:6.1f} | {x4 / x0:5.3f} | {x1 / x0:5.3f} | {x2 / x1:5.3f} | {x3a / x2:5.3f} | {x3 / x3a:5.3f} | {x4 / x3:5.3f} | "
              f"{c['X3 rand']:.1f} -> {c['X3 no uncoded CS']:.1f} / {c['X3 no random term']:.1f} / {c['X3 no other det']:.1f}")
