"""The condensed tables of layered.md from cascade.py's results. usage: python3 summary.py results/"""
import glob
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cascade_tables import interp, mb  # noqa: E402

SHOW = (0.9, 0.95, 0.99)
SETS = ("none", "random-25", "region-25", "random-1", "region-1")


def main():
    d = sys.argv[1]
    for k in (10, 100):
        rows = []
        for ds in ("glove", "yahoo", "laion"):
            for st in SETS:
                p = f"{d}/cascade_{ds}_{st}.json"
                if not os.path.exists(p):
                    continue
                R = json.load(open(p))
                col = {c: i for i, c in enumerate(R["columns"])}
                T = R["targets"]
                runs = {n[len(f"k={k} "):]: e for n, e in R["runs"].items() if n.startswith(f"k={k} ")}
                if not runs:
                    continue
                one = [e["one_dial"] for n, e in runs.items() if n.startswith("stat-m ") and n.endswith(" L16+32")]
                oned = [e["one_dial"] for n, e in runs.items() if n.startswith("stat-d ") and n.endswith(" L16+32")]
                cells = []
                extra = ("-", "-", "-")
                for t in SHOW:
                    i = T.index(t)
                    a, c, s, sd = runs["direct"]["stop"][i], runs["cert"]["stop"][i], interp(one, t), interp(oned, t)
                    if a is None or s is None:
                        cells.append("-")
                        continue
                    b = col["bytes"]
                    cells.append(f"{mb(a[b])} / {mb(c[b]) if c else '-'} / {mb(s[b])} / {mb(sd[b]) if sd else '-'} ({a[b] / s[b]:.1f}x)")
                    if t == 0.95:
                        extra = f"{a[col['clusters']]:.0f} -> {s[col['clusters']]:.0f}", f"{a[col['full_rows']]:,.0f} -> {s[col['full_rows']]:,.0f}", f"{max(s[col[c2]] for c2 in col if c2.startswith('enter_l')):,.0f}"
                rows.append(f"| {ds} | {st} | " + " | ".join(cells) + f" | {extra[0]} | {extra[1]} | {extra[2]} |")
        if rows:
            print(f"\nk = {k}. MB read per query to a recall: direct / certified levels / statistical levels with rho measured / with rho = 1/sqrt(d) (direct over statistical); at recall 0.95: clusters read, stored vectors read, rows scanned at the first level.\n")
            print("| dataset | visible set | recall 0.9 | 0.95 | 0.99 | clusters, direct -> statistical | stored vectors | rows scanned at 16 B |")
            print("|---|---|---|---|---|---|---|---|")
            print("\n".join(rows))
    # parts
    print("\nParts of the statistical rule's bytes at recall 0.95, k = 10 (MB): centres / sigmas / codes / stored vectors / group setups / tables.\n")
    print("| dataset | visible set | parts | total |")
    print("|---|---|---|---|")
    for ds in ("glove", "yahoo", "laion"):
        for st in SETS:
            p = f"{d}/cascade_{ds}_{st}.json"
            if not os.path.exists(p):
                continue
            R = json.load(open(p))
            col = {c: i for i, c in enumerate(R["columns"])}
            runs = {n[len("k=10 "):]: e for n, e in R["runs"].items() if n.startswith("k=10 ")}
            one = [e["one_dial"] for n, e in runs.items() if n.startswith("stat-m ") and n.endswith(" L16+32")]
            s = interp(one, 0.95)
            if s is None:
                continue
            print(f"| {ds} | {st} | {mb(R['fixed_centre_bytes'])} / " + " / ".join(mb(s[col[c]]) for c in ("sigma", "code", "full", "group", "table")) + f" | {mb(s[col['bytes']])} |")


if __name__ == "__main__":
    main()
