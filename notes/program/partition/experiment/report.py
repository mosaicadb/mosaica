# Tables for RESULTS.md from work/scores/*.json. usage: report.py NAME [NAME ...]  (prints Markdown)
# The seed spread is the largest gap among A1, A2, A3 for each column, printed as a last row when all three exist.
import sys, json, os
W = os.path.dirname(os.path.abspath(__file__)) + "/work/scores/"
names = sys.argv[1:]
D = {nm: json.load(open(W + nm + ".json")) for nm in names}
SEEDS = [json.load(open(W + f"A{s}.json")) for s in (1, 2, 3) if os.path.exists(W + f"A{s}.json")]
KINDS = ["text", "species", "item"]
FRS = ["0.01%", "0.1%", "1%"]


def table(title, cols, heads, fmt, rel=None):
    print(f"\n{title}\n")
    print("| partition | " + " | ".join(heads) + " |")
    print("|---|" + "---|" * len(cols))
    for nm in names:
        d = D[nm]
        cells = []
        for c in cols:
            v = d.get(c)
            cells.append("n/a" if v is None else fmt(v))
        print(f"| {nm} | " + " | ".join(cells) + " |")
    if len(SEEDS) == 3:
        sp = []
        for c in cols:
            vs = [s.get(c) for s in SEEDS]
            sp.append("n/a" if any(v is None for v in vs) else fmt(max(vs) - min(vs)))
        print("| seed spread (A1-A3, max−min) | " + " | ".join(sp) + " |")
    if rel and rel in D:
        print(f"| ratio to {rel}: |" + "|" * len(cols))
        for nm in names:
            if nm == rel:
                continue
            cells = []
            for c in cols:
                v, b = D[nm].get(c), D[rel].get(c)
                cells.append("n/a" if (v is None or not b) else f"{v / b:.4f}")
            print(f"| {nm} / {rel} | " + " | ".join(cells) + " |")


rel = os.environ.get("REL")
for key in ("Si", "Sm"):
    cols = [f"{key}_S1d_16", f"{key}_S1b_0.06", f"{key}_S1d_4", f"{key}_S1d_8", f"{key}_S1d_32",
            f"{key}_S1b_0.03", f"{key}_S1b_0.12", f"{key}_S1a_mean", f"{key}_S1a_rms"]
    heads = ["S1d 16B", "S1b w*=.06 (B)", "S1d 4B", "S1d 8B", "S1d 32B", "S1b .03", "S1b .12", "S1a mean (sec.)", "S1a rms (sec.)"]
    table(f"Coding, Σ_Q = {key}, held-out rows (mean over 49,978 rows)", cols, heads, lambda v: f"{v:.5f}" if v < 1 else f"{v:.3f}", rel)
if os.environ.get("CODING_ONLY"):
    sys.exit()
cols = [f"S23_{k}_{f}" for k in KINDS for f in FRS]
table("S2+3 (share of valid rows left by both cut and range), mean over queries", cols, [f"{k} {f}" for k in KINDS for f in FRS],
      lambda v: f"{v:.4f}", rel)
cols = [f"S2_{k}_{f}" for k in KINDS for f in FRS]
table("S2 (cut alone)", cols, [f"{k} {f}" for k in KINDS for f in FRS], lambda v: f"{v:.4f}", rel)
cols = [f"S3_{k}_{f}" for k in KINDS for f in FRS]
table("S3 (sorted range alone)", cols, [f"{k} {f}" for k in KINDS for f in FRS], lambda v: f"{v:.4f}", rel)
cols = [f"S2rho_{k}_1%" for k in KINDS] + [f"S2ext_{k}_1%" for k in KINDS]
table("S2 at 1%: mean 2ρ (net) and mean β(E⁺−E⁻) (extent) over cells left", cols, [f"2ρ {k}" for k in KINDS] + [f"extent {k}" for k in KINDS],
      lambda v: f"{v:.4f}")
cols = [f"S4a_mean_{k}" for k in KINDS] + [f"S4a_ratio_{k}" for k in KINDS] + [f"S4b_{k}" for k in KINDS] + ["S4b_nn_owncell"]
table("Order: S4a (mean share; ratio of means), S4b (share of rows visited to cover 9 of the 10 best), own-cell share of 10-NN",
      cols, [f"S4a {k}" for k in KINDS] + [f"S4a ratio {k}" for k in KINDS] + [f"S4b {k}" for k in KINDS] + ["10-NN own cell"],
      lambda v: f"{v:.4f}", rel)
cols = ["mean_key", "msq_resid", "S5_min", "S5_median", "S5_max", "S5_cv", "S5_share_under500", "S5_cells_per_family",
        "S5_cells_per_genus", "S5_cells_per_scientific_name", "S5_cells_per_scientific_name_rowweighted", "radius_max_over_rms_median", "radius_max_over_rms_share_above2", "key_p1_over_median_median",
        "S1e", "viol_S2", "viol_S3"]
table("Layout and shape (all valid rows)", cols, ["mean key", "msq resid", "n_c min", "median", "max", "CV", "rows in cells <500",
      "cells/family", "cells/genus", "cells/species", "cells/species (row-wtd)", "max/rms radius (median cell)", "cells with max/rms>2", "key p1/median (median cell)",
      "S1e B/row", "viol S2", "viol S3"], lambda v: f"{v:.4f}" if isinstance(v, float) else str(v))
