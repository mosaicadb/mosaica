"""Each order's worst rows ratio to the best order over every block and recall of study_order_stress (the sketch
condition aside), over all, without the in-cluster bottom sets, and without those and k = 1; and the share of cells
within 1.10. python3 stress_worst.py OUT file [file ...]"""
import re
import sys

T = ["0.9", "0.95", "0.99", "0.995", "0.999"]
cells = {}
for path in sys.argv[2:]:
    cur = None
    for l in open(path):
        m = re.match(r"== \S*/stress-ix/(\S+) (\S+) (\S+) k=(\d+)", l)
        if m:
            cur = None if m.group(2) == "sketch" else dict(ds=m.group(1), cond=m.group(2), filt=m.group(3), k=int(m.group(4)), rows={})
            if cur:
                cells.setdefault("blocks", []).append(cur)
            continue
        m = re.match(r"  (\S.*?)\s+(0\.9: .*)$", l)
        if cur and m and not l.startswith("  error"):
            cur["rows"][m.group(1).strip()] = {t: float(r) for t, r in re.findall(r"([0-9.]+): (\d+) rows", m.group(2))}
blocks = cells["blocks"]
orders = list(blocks[0]["rows"])
out = [f"{'order':<22} {'worst all':>22} {'worst w/o bottom':>22} {'w/o bottom, k>1':>22}  share <= 1.10 (w/o bottom)"]
for o in orders:
    def worst(sel):
        best = (0.0, "")
        n = ok = 0
        for b in blocks:
            if not sel(b) or o not in b["rows"]:
                continue
            for t in T:
                vals = [b["rows"][x][t] for x in b["rows"] if t in b["rows"][x]]
                if t not in b["rows"][o]:
                    continue
                r = b["rows"][o][t] / min(vals)
                n += 1
                ok += r <= 1.10
                if r > best[0]:
                    best = (r, f"{b['ds']} {b['filt']} k={b['k']} {t}")
        return best, (ok / n if n else 0)
    a, _ = worst(lambda b: True)
    nb, share = worst(lambda b: not b["filt"].startswith("bottom"))
    nk, _ = worst(lambda b: not b["filt"].startswith("bottom") and b["k"] > 1)
    out.append(f"{o:<22} {a[0]:>6.2f} {a[1]:<34} | {nb[0]:.2f} {nb[1]:<34} | {nk[0]:.2f} {nk[1]:<34} | {share:.2f}")
open(sys.argv[1], "w").write("\n".join(out) + "\n")
print("\n".join(out))
