# Markdown tables from policy.py's JSON.   usage: python3 -I ptables.py DIR
import sys, os, json
import numpy as np

DIR = sys.argv[1]
ARMS = ["T1", "T2", "T3", "T4", "T5", "O0", "O1", "O2", "P0", "P1", "P2", "P0i", "P1i", "P2i", "P0s", "P1s", "P2s", "P1si", "P2si",
        "P1 ag0", "P1 tau", "P2 tau", "P0*", "P1*", "P2*", "P1i*", "P2i*", "P1s*", "P2s*", "P1si*", "P2si*", "clair", "clair i"]


def at(res, t):
    rec = np.array(res["rec"])
    hit = np.where(rec >= t)[0]
    if not len(hit):
        return None
    i = int(hit[0])
    f = 0.0 if i == 0 else (t - rec[i - 1]) / max(rec[i] - rec[i - 1], 1e-12)
    return {k: (v[i] if i == 0 else v[i - 1] + f * (v[i] - v[i - 1])) for k, v in res.items() if k not in ("rec", "lam")}


for ds in ("laion", "glove", "yahoo"):
    p = os.path.join(DIR, f"policy_{ds}.json")
    if not os.path.exists(p):
        continue
    O = json.load(open(p))
    p2 = os.path.join(DIR, f"star_{ds}.json")
    if os.path.exists(p2):
        for task, R in json.load(open(p2)).items():
            O[task].update(R)
    for task, R in O.items():
        print(f"\n### {ds}, {task}\n")
        print("| arm | modelled us at recall 0.8 / 0.9 / 0.95 | vs T1 at 0.9 / 0.95 | rows at 0.9 / 0.95 | clusters at 0.9 | groups at 0.9 | centres at 0.9 | epochs | max recall |")
        print("|---|---|---|---|---|---|---|---|---|")
        t1 = {t: at(R["T1"], t) for t in (0.9, 0.95)}
        for a in ARMS:
            if a not in R:
                continue
            e = {t: at(R[a], t) for t in (0.8, 0.9, 0.95)}
            f = lambda t, k, fmt="{:,.0f}": "-" if e[t] is None else fmt.format(e[t][k])
            rel = " / ".join("-" if e[t] is None or t1[t] is None else f"{e[t]['cost'] / t1[t]['cost']:.2f}" for t in (0.9, 0.95))
            ep = f"{np.mean(R[a]['epochs']):.1f}" if "epochs" in R[a] else ""
            print(f"| {a} | {f(0.8, 'cost')} / {f(0.9, 'cost')} / {f(0.95, 'cost')} | {rel} | {f(0.9, 'rows')} / {f(0.95, 'rows')} | "
                  f"{f(0.9, 'clusters', '{:.1f}')} | {f(0.9, 'groups', '{:.1f}')} | {f(0.9, 'centres')} | {ep} | {max(R[a]['rec']):.3f} |")

# condensed: modelled cost relative to T1 at recall 0.9 / 0.95, per task
print("\n## Modelled cost relative to T1 (spread, nprobe) at recall 0.9 / 0.95; `-`: recall not reached\n")
for ds in ("laion", "glove", "yahoo"):
    p = os.path.join(DIR, f"policy_{ds}.json")
    if not os.path.exists(p):
        continue
    O = json.load(open(p))
    p2 = os.path.join(DIR, f"star_{ds}.json")
    if os.path.exists(p2):
        for task, R in json.load(open(p2)).items():
            O[task].update(R)
    tasks = list(O)
    print(f"\n**{ds}** (T1's modelled us at 0.9: " + ", ".join(f"{t} {at(O[t]['T1'], 0.9)['cost']:,.0f}" for t in tasks) + ")\n")
    print("| arm | " + " | ".join(tasks) + " |")
    print("|---|" + "---|" * len(tasks))
    for a in ARMS[1:]:
        cells = []
        for t in tasks:
            if a not in O[t]:
                cells.append("")
                continue
            c = []
            for r in (0.9, 0.95):
                e, b = at(O[t][a], r), at(O[t]["T1"], r)
                c.append("-" if e is None or b is None else f"{e['cost'] / b['cost']:.2f}")
            cells.append(" / ".join(c))
        if any(cells):
            print(f"| {a} | " + " | ".join(cells) + " |")
