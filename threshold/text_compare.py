# Item against text rows, from the tables.md that tables.py wrote for each run (no recomputation).
# usage: text_compare.py NAME=TABLES.md ...   (the first is the reference, normally the item run's logs/tables.md)
import sys, re
CODES = (("local PCA", "Local basis, PCA (V3sub)"), ("local GM", "Local basis, geometric-mean metric (V3sub)"), ("flat", "Flat global rotation"))
SH = ["0.01%", "0.1%", "1%", "10%", "30%"]
def parse(path):
    secs, cur = {}, None
    for line in open(path):
        if line.startswith("### "): cur = line[4:].strip(); secs[cur] = {"head": None, "und": {}, "tol": {}}; continue
        if cur is None or not line.startswith("| "): continue
        c = [x.strip() for x in line.strip().strip("|").split("|")]
        if c[0] == "Share": secs[cur]["head"] = c if secs[cur]["head"] is None else secs[cur]["head"]; continue
        if c[0] in SH: (secs[cur]["tol"] if "/" in c[1] else secs[cur]["und"])[c[0]] = c
    out = {}
    for short, title in CODES:
        s = next(v for k, v in secs.items() if k.startswith(title)); h = s["head"]
        out[short] = {sh: dict(und={h[i]: float(s["und"][sh][i]) for i in range(1, len(h)) if h[i].isdigit()},
                               reads=float(s["und"][sh][h.index("exact reads/match")]), viol=int(s["und"][sh][h.index("violations")]),
                               tol=[[float(x) for x in v.split("/")] for v in s["tol"][sh][1:]]) for sh in SH}
    return out
runs = [(a.split("=", 1)[0], parse(a.split("=", 1)[1])) for a in sys.argv[1:]]
P = print
P("## Undecided per true match after each level (d = 0) and exact reads per match, item and text side by side\n")
for short, _ in CODES:
    cols = ["32", "48", "96", "192", "384"] if short != "flat" else ["32", "64", "96", "192", "384"]
    P(f"\n{short}\n\n| Share | Queries | " + " | ".join(cols) + " | exact reads |"); P("|---" * (len(cols) + 3) + "|")
    for sh in SH:
        for nm, R in runs: r = R[short][sh]; P(f"| {sh} | {nm} | " + " | ".join(f"{r['und'][c]:.3g}" for c in cols) + f" | {r['reads']:.3g} |")
P("\n## Code bytes per item / exact reads per match, d = 0 and d = 0.01\n")
P("| Share | Queries | " + " | ".join(f"{s} d=0 | {s} d=0.01" for s, _ in CODES) + " | flat / local PCA code B, d=0 | flat / local PCA reads, d=0 |"); P("|---" * 10 + "|")
for sh in SH:
    for nm, R in runs:
        cells = [f"{R[s][sh]['tol'][d][1]:.1f} / {R[s][sh]['tol'][d][0]:.3f}" for s, _ in CODES for d in (0, 3)]
        lp, fl = R["local PCA"][sh], R["flat"][sh]
        rr = fl["tol"][0][0] / lp["tol"][0][0] if lp["tol"][0][0] > 0 else float("inf")
        P(f"| {sh} | {nm} | " + " | ".join(cells) + f" | {fl['tol'][0][1] / lp['tol'][0][1]:.2f} | {rr:.2f} |")
P("\nWrong decisions (violations) summed over shares, d = 0 rows: " + ", ".join(f"{nm}: " + "/".join(str(sum(R[s][sh]['viol'] for sh in SH)) for s, _ in CODES) for nm, R in runs))
