# Markdown tables from the JSON written by under.py and over.py.
# usage: python3 -I tables.py DIR      (DIR holds under_<ds>.json and over_<ds>.json)
# The cost columns are ESTIMATES from counts, not timings. Constants per dataset (us), from the real index's phase
# timings on these fits (bench/suite/notes/visit-order/results/phases-1-int8-order.txt, unfiltered, nprobe 16 and 64):
#   group   the group phase / mean groups among the first P clusters: 27 (laion, n 512), 14.5 (yahoo, n 384), 4 (glove, n 128)
#   centre  the order phase / K (int8 centres): 0.07, 0.067, 0.045
#   cluster 4 (the planner's 1.6-7.8 per cluster; not separable in the phase figures)
#   row     (the rest phase at nprobe 16 - 4 x 16) / rows scanned there: 0.06, 0.04, 0.027
# The per-row figure is for whole clusters; rows of a probed sub-cluster are the ones nearest the query and may cost
# more each, so the estimate favours sub-clusters. The fixed "stored" phase (50-190 us) is left out of every row.
import sys, os, json
import numpy as np

DIR = sys.argv[1]
DSS = [("laion", 1000448, 512), ("glove", 1183514, 100), ("yahoo", 677305, 384)]
FILT = ["random-1", "random-25", "clus-1", "clus-25", "cap-1", "cap-25"]


COST = dict(laion=(0.06, 27, 4, 0.07), yahoo=(0.04, 14.5, 4, 0.067), glove=(0.027, 4, 4, 0.045))


def cost(ds, items, groups, centres, clusters):
    r, g, c, m = COST[ds]
    return r * items + g * groups + c * clusters + m * centres


def n(x, w=0):
    return "-" if x is None else f"{x:,.0f}"


def under_tables(ds, N, d):
    p = os.path.join(DIR, f"under_{ds}.json")
    if not os.path.exists(p):
        return
    U = json.load(open(p))

    def pick_two(T, s, key="spread 0.4", t="0.95", slack=1.05, kind="two-level"):
        """The two-level run with the fewest parents whose items at recall t are within 5% of the direct run's."""
        ref = T[f"x{s} direct {key}"]["at"][t]
        for P in (8, 16, 32, 64, 128, 256):
            e = T.get(f"x{s} {kind} P{P} {key}")
            if e and e["at"][t] and ref and e["at"][t]["items"] <= slack * ref["items"]:
                return f"x{s} {kind} P{P} {key}"
        return None

    def best(T, pre, t="0.95"):
        c = [(v["at"][t]["items"], k) for k, v in T.items() if k.startswith(pre) and v["at"][t]]
        return min(c)[1] if c else None

    print(f"\n### {ds}: sub-clusters, k-NN, no filter (N {N:,})\n")
    print("| structure, key | recall at 0.2 / 0.5 / 1.6 / 6.3% | items at 0.9 | at 0.95 | at 0.99 | groups at 0.9 / 0.95 / 0.99 | clusters at 0.9 / 0.95 | centres | est. us at 0.9 / 0.95 |")
    print("|---|---|---|---|---|---|---|---|---|")
    T = U["knn"]
    rows = ["flat centre", "flat spread .4", "flat H r4", "flat H r16"]
    for s in (4, 16, 64):
        rows += [f"x{s} direct centre", f"x{s} direct nonin .2", f"x{s} direct spread 0.2", f"x{s} direct spread 0.4",
                 f"x{s} direct spread 0.6", f"x{s} direct spread 0.8",
                 f"x{s} direct Hown4 a0.5", f"x{s} direct Hown4 a0.75", f"x{s} direct Hpar a0.5", f"x{s} direct Hpar a0.75",
                 f"x{s} direct gb16 0.4", f"x{s} direct gb64 0.4", f"x{s} direct gb128 0.4"]
        for P in (16, 32, 64, 128, 256):
            rows.append(f"x{s} two-level P{P} spread 0.4")
        rows += [f"x{s} two-level P64 gb16 0.4", f"x{s} two-level P64 gb64 0.4", f"x{s} two-level P64 Hown4 a0.75"]
        rows += [f"x{s} group-limited P{P} spread 0.4" for P in (8, 16, 32, 64, 128)]
    for r in rows:
        if r not in T:
            continue
        e = T[r]
        a = e["at"]
        g = " / ".join(n(a[t]["groups"]) if a[t] else "-" for t in ("0.9", "0.95", "0.99"))
        c = next((a[t]["centres"] for t in ("0.8", "0.9", "0.95", "0.99") if a[t]), None)
        est = " / ".join(n(cost(ds, a[t]["items"], a[t]["groups"], a[t]["centres"], a[t]["parents"])) if a[t] else "-" for t in ("0.9", "0.95"))
        cl = " / ".join(n(a[t]["parents"]) if a[t] else "-" for t in ("0.9", "0.95"))
        print(f"| {r} | " + " ".join(f"{e['share'][x]:.3f}" for x in ("0.002", "0.005", "0.016", "0.063")) +
              f" | {n(a['0.9'] and a['0.9']['items'])} | {n(a['0.95'] and a['0.95']['items'])} | "
              f"{n(a['0.99'] and a['0.99']['items'])} | {g} | {cl} | {n(c)} | {est} |")

    print(f"\n### {ds}: sub-clusters under visible sets and for threshold queries: items / groups / clusters touched at recall 0.9, and items at 0.95\n")
    tasks = ["knn"] + [f"knn {f}" for f in FILT] + ["thr 0", "thr 1", "thr 2"]
    tasks = [t for t in tasks if t in U]
    print("| structure, key | " + " | ".join(tasks) + " |")
    print("|---|" + "---|" * len(tasks))
    rows = ["flat centre", "flat spread .4", "flat H r16", "flat zeta r16", "x4 direct spread 0.4", "x16 direct centre",
            "x16 direct nonin .2", "x16 direct spread 0.4", "x16 direct Hown4 a0.5", "x16 direct gb64 0.4",
            "x16 direct zeta own4", "x16 2L", "x16 GL", "x64 direct nonin .2", "x64 direct spread 0.4", "x64 2L", "x64 GL"]
    for r in rows:
        cells = []
        for t in tasks:
            T = U[t]
            rr = r
            if r[-2:] in ("2L", "GL"):
                rr = pick_two(T, int(r.split()[0][1:]), t="0.9", kind="two-level" if r.endswith("2L") else "group-limited")
            e = T.get(rr) if rr else None
            if not e or not e["at"]["0.9"]:
                cells.append("-")
                continue
            a = e["at"]
            lab = f" (P{rr.split(' P')[1].split()[0]}, {n(a['0.9']['centres'])} c)" if r[-2:] in ("2L", "GL") else ""
            cells.append(f"{n(a['0.9']['items'])} / {n(a['0.9']['groups'])} / {n(a['0.9']['parents'])}; {n(a['0.95'] and a['0.95']['items'])}{lab}")
        print(f"| {r} | " + " | ".join(cells) + " |")
    print("\nRatio of items at recall 0.9 and 0.95, flat spread .4 over the variant (above 1: the variant scans fewer), "
          "and the change in groups touched at 0.9:\n")
    print("| variant | " + " | ".join(tasks) + " |")
    print("|---|" + "---|" * len(tasks))
    for r in ["x4 direct spread 0.4", "x16 direct spread 0.4", "x16 direct nonin .2", "x16 2L", "x16 GL", "x64 direct spread 0.4", "x64 2L", "x64 GL"]:
        cells = []
        for t in tasks:
            T = U[t]
            rr = pick_two(T, int(r.split()[0][1:]), t="0.9", kind="two-level" if r.endswith("2L") else "group-limited") if r[-2:] in ("2L", "GL") else r
            e, f = (T.get(rr) if rr else None), T["flat spread .4"]
            if not e or not e["at"]["0.9"] or not f["at"]["0.9"]:
                cells.append("-")
                continue
            x9 = f["at"]["0.9"]["items"] / e["at"]["0.9"]["items"]
            x95 = f["at"]["0.95"]["items"] / e["at"]["0.95"]["items"] if e["at"]["0.95"] and f["at"]["0.95"] else float("nan")
            cells.append(f"{x9:.2f}x, {x95:.2f}x; {e['at']['0.9']['groups'] - f['at']['0.9']['groups']:+.0f} g")
        print(f"| {r} | " + " | ".join(cells) + " |")


def at(res, t, what, budget=None):
    rec, v, c = np.array(res["rec"]), np.array(res[what], np.float64), np.array(res["centres"])
    ok = np.ones(len(rec), bool) if budget is None else c <= budget
    hit = np.where((rec >= t) & ok)[0]
    if not len(hit):
        return None
    i = int(hit[0])
    if i == 0 or not ok[i - 1]:
        return float(v[i])
    f = (t - rec[i - 1]) / max(rec[i] - rec[i - 1], 1e-12)
    return float(v[i - 1] + f * (v[i] - v[i - 1]))


def over_tables(ds, N, d):
    p = os.path.join(DIR, f"over_{ds}.json")
    if not os.path.exists(p):
        return
    O = json.load(open(p))
    parts = list(O)
    print(f"\n### {ds}: flat orders on each partition, k-NN, no filter: items / centres scored / groups touched\n")
    print("| partition, key | at recall 0.8 | 0.9 | 0.95 | 0.99 | est. us at 0.9 | at 0.95 |")
    print("|---|---|---|---|---|---|---|")
    for P in parts:
        for k in ("flat centre", "flat spread", "flat H r4"):
            r = O[P]["knn"].get(k)
            if not r:
                continue
            cells, est = [], []
            for t in (0.8, 0.9, 0.95, 0.99):
                x = [at(r, t, w) for w in ("items", "centres", "groups")]
                cells.append("-" if x[0] is None else " / ".join(n(v) for v in x))
                if t in (0.9, 0.95):
                    est.append("-" if x[0] is None else n(cost(ds, x[0], x[2], x[1], at(r, t, "p"))))
            print(f"| {P} {k} | " + " | ".join(cells) + " | " + " | ".join(est) + " |")

    print(f"\n### {ds}: beams, k-NN, no filter. At the nprobe where the flat spread order reaches recall 0.9 (np): "
          f"the beam's recall, the share of true neighbours lost at each tree level, centres scored (share of the flat order's)\n")
    WID = ("m1", "m2", "m4", "m8", "w2", "w4", "w8", "w16")
    print("| partition | tree | node key | " + " | ".join(WID) + " |")
    print("|---|---|---|" + "---|" * len(WID))
    for P in parts:
        T = O[P]["knn"]
        flat = T["flat spread"]
        i = int(np.where(np.array(flat["rec"]) >= 0.9)[0][0])
        K = flat["centres"][0]
        trees = sorted({k.split(" | ")[0] for k in T if " | " in k}, key=lambda s: list(T).index(next(k for k in T if k.startswith(s + " |"))))
        for tn in trees:
            for nk in ("c", "sp 1", "st 1", "st 1 local"):
                cells = []
                for m in WID:
                    r = T.get(f"{tn} | {nk} | {m}")
                    if not r or i >= len(r["p"]):
                        cells.append("")
                        continue
                    cells.append(f"{r['rec'][i]:.3f}; " + " ".join(f"{x:.3f}" for x in r["loss"][i][:-1]) +
                                 f"; {r['centres'][i]:,.0f} ({r['centres'][i] / K:.2f})")
                if any(cells):
                    print(f"| {P} np {flat['p'][i]} rec {flat['rec'][i]:.3f} | {tn} | {nk} | " + " | ".join(cells) + " |")

    print(f"\n### {ds}: the narrowest beam (of m1, m2, m4, m8; and of w2, w4, w8, w16) that reaches recall 0.9 / 0.95 / 0.99 with at most 3% more items than "
          f"the flat spread order, and the centres it scores (share of K)\n")
    print("| partition | tree (nodes per level) | node key | 0.9 | 0.95 | 0.99 |")
    print("|---|---|---|---|---|---|")
    for P in parts:
        T = O[P]["knn"]
        flat = T["flat spread"]
        K = flat["centres"][0]
        for tn in sorted({k.split(" | ")[0] for k in T if " | " in k}):
            for nk in ("c", "sp 1", "st 1"):
                cells = []
                for t in (0.9, 0.95, 0.99):
                    fi = at(flat, t, "items")
                    cs = []
                    for ws in (("m1", "m2", "m4", "m8"), ("w2", "w4", "w8", "w16")):
                        c = "-"
                        for m in ws:
                            r = T.get(f"{tn} | {nk} | {m}")
                            x = at(r, t, "items") if r else None
                            if x is not None and fi is not None and x <= 1.03 * fi:
                                cc = at(r, t, "centres")
                                c = f"{m}: {cc:,.0f} ({cc / K:.2f})"
                                break
                        cs.append(c)
                    cells.append("; ".join(cs))
                print(f"| {P} | {tn} | {nk} | " + " | ".join(cells) + " |")

    print(f"\n### {ds}: payoff. Fewest items scanned at recall 0.9 / 0.95 for a budget of centres scored per query "
          f"(best tree, key and m within the budget; `-`: not reachable), with the groups touched there\n")
    Kr = O["real"]["knn"]["flat spread"]["centres"][0]
    budgets = [Kr, 2 * Kr, 4 * Kr, 8 * Kr, None]
    print("| partition | recall | " + " | ".join("any" if b is None else f"<= {b:,.0f}" for b in budgets) + " |")
    print("|---|---|" + "---|" * len(budgets))
    for P in parts:
        T = O[P]["knn"]
        for t in (0.9, 0.95):
            cells = []
            for b in budgets:
                best = None
                for k, r in T.items():
                    if "H r4" in k or "centre" in k or "local" in k:
                        continue
                    x = at(r, t, "items", b)
                    if x is not None and (best is None or x < best[0]):
                        best = (x, at(r, t, "groups", b), at(r, t, "centres", b), k)
                cells.append("-" if best is None else f"{best[0]:,.0f} / {best[1]:.0f} g ({best[3].replace(' | ', ', ')}; {best[2]:,.0f} c)")
            print(f"| {P} | {t} | " + " | ".join(cells) + " |")
    up = os.path.join(DIR, f"under_{ds}.json")
    if os.path.exists(up):
        U = json.load(open(up))["knn"]
        for s in (4, 16, 64):
            for t in ("0.9", "0.95"):
                cells = []
                for b in budgets:
                    c = [(v["at"][t]["items"], v["at"][t]["groups"], v["at"][t]["centres"], k) for k, v in U.items()
                         if k.startswith(f"x{s} ") and k.endswith("spread 0.4") and v["at"][t]
                         and (b is None or v["at"][t]["centres"] <= b)]
                    best = min(c) if c else None
                    cells.append("-" if best is None else f"{best[0]:,.0f} / {best[1]:.0f} g ({best[3].replace(f'x{s} ', '').replace(' spread 0.4', '')}; {best[2]:,.0f} c)")
                print(f"| real, x{s} sub-clusters (spread 0.4) | {t} | " + " | ".join(cells) + " |")

    tasks = [t for t in [f"knn {f}" for f in FILT] + ["thr 0", "thr 1", "thr 2"] if t in O[parts[0]]]
    print(f"\n### {ds}: visible sets and threshold queries on each partition: items / centres / groups at recall 0.9 "
          f"(beam rows: the groups as the tree on the real fit, `hkm 256` on the refits; node key and leaf width as named)\n")
    print("| partition, order | " + " | ".join(tasks) + " |")
    print("|---|" + "---|" * len(tasks))
    for P in parts:
        rows = [("flat spread", None), ("flat zeta r4", None)] + [(nk, m) for nk in ("c", "st 1") for m in (4, 16)]
        for nk, m in rows:
            cells = []
            for t in tasks:
                T = O[P].get(t, {})
                if m is None:
                    r = T.get(nk)
                    x = [at(r, 0.9, w) for w in ("items", "centres", "groups")] if r else [None]
                    cells.append("-" if x[0] is None else " / ".join(n(v) for v in x))
                    continue
                tn = "groups" if P == "real" else "hkm 256"
                r = T.get(f"{tn} | {nk} | w{m}")
                x = [at(r, 0.9, w) for w in ("items", "centres", "groups")] if r else [None]
                cells.append("-" if x[0] is None else " / ".join(n(v) for v in x))
            if any(c != "-" for c in cells):
                print(f"| {P} {nk}{'' if m is None else f' w{m}'} | " + " | ".join(cells) + " |")


for ds, N, d in DSS:
    under_tables(ds, N, d)
for ds, N, d in DSS:
    over_tables(ds, N, d)
