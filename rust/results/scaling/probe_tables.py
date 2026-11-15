# Tables from sc/probe/probe_<fit>.json: items scanned at mean recall 0.8 / 0.9 / 0.99, schemes (a) parents, (b) subs
# of about 62, (c) subs of about 250; at 0.99 parents and groups opened.
import json, glob, os, sys
D = sys.argv[1]
order = ["k500", "m1", "r1m", "r3m"]
fs = sorted(glob.glob(f"{D}/sc/probe/probe_*.json"), key=lambda p: (order.index(os.path.basename(p)[6:].split("_")[0]), int(p.split("_")[-1][:-5])))
R = [json.load(open(p)) for p in fs]
for kind in ["img", "txt"]:
    for k in [10, 100, 1000]:
        print(f"\n### {kind}, k = {k}: items scanned at mean recall 0.8 / 0.9 / 0.99 (a parents | b subs ~62 | c subs ~250)\n")
        print("| fit | K | (a) | (b) | (c) | (b) / (a) at 0.99 |\n|---|---|---|---|---|---|")
        for r in R:
            g = lambda s, t: r[s].get(f"{kind} k{k} r{t}", {}).get("items")
            f = lambda s: "/".join("–" if g(s, t) is None else f"{g(s, t):.0f}" for t in (0.8, 0.9, 0.99))
            ra = g("b", 0.99) / g("a", 0.99) if g("b", 0.99) and g("a", 0.99) else None
            print(f"| {r['name']} | {r['K']} | {f('a')} | {f('b')} | {f('c')} | {'' if ra is None else f'{ra:.2f}'} |")
print("\n### At mean recall 0.99: parents opened / groups opened (a | b | c), and centres scored for (b) and (c)\n")
print("| fit | img k10 | img k100 | txt k10 | txt k100 | txt k1000 |\n|---|---|---|---|---|---|")
for r in R:
    cells = []
    for kk in ["img k10", "img k100", "txt k10", "txt k100", "txt k1000"]:
        x = [r[s].get(f"{kk} r0.99") for s in "abc"]
        cells.append(" | ".join([]) + "; ".join("–" if v is None else f"{v.get('parents', 0):.0f}/{v.get('groups', 0):.0f}" + (f" ({v['centres_scored']:.0f} c)" if 'centres_scored' in v else "") for v in x))
    print(f"| {r['name']} | " + " | ".join(cells) + " |")
