# Objective per pass for the C runs (fit and held-out rows), from work/C/<rule>_hist.json.
import json, sys
print("| rule | pass | fit rows moved | refused by the floor | cells alive | fit: mean w₁₆² | fit: the rule's cost | held-out: mean w₁₆² | held-out: the rule's cost | fit rows off their starting cell | ... off their starting group |")
print("|---|---|---|---|---|---|---|---|---|---|---|")
for r in sys.argv[1:]:
    for h in json.load(open(f"work/C/{r}_hist.json")):
        if h["pass"] == 0:
            print(f"| {r} | 0 (B16-in) | | | 200 | {h['fit_w16sq']:.6f} | {h['fit_rule']:.6f} | {h['held_w16sq_baseline_cells']:.6f} | | | |")
            continue
        print(f"| {r} | {h['pass']} | {h['moved_fit']:.4f} | {h['refused_floor']} | {h['cells_alive']} | {h['fit_w16sq']:.6f} | {h['fit_rule']:.6f} | {h['held_w16sq']:.6f} | {h['held_rule']:.6f} | {h['fit_change_cell_from_start']:.4f} | {h['fit_change_group_from_start']:.4f} |")
