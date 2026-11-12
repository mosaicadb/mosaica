# Markdown tables for the final report. usage: python3 tables.py NAME [TAG]
import sys, numpy as np
import report as R0
from report import row, R, A, qcls, N, n, K, FULL, TH, tracks, nA, nimg, ROT
ORDER = [("V0", "original construction (uniform clipped, independent allocations), honest scalars"),
         ("V1", "+ escape code (nested)"), ("V2", "+ alpha*"), ("V3int", "+ randomised deep levels, split-bound allocation, integer rates"),
         ("V3sub", "+ sub-bit tail blocks"), ("V3sub+C1", "+ sorted range (C1)"), ("V3sub+C1+C2", "+ support table (C2) = combined")]
EXTRA = [("V3int-sg", "V3int with the sub-Gaussian tail instead of the exact sphere tail"),
         ("V3sub-sg", "V3sub with the sub-Gaussian tail"), ("V3sub+C1+C2img", "combined, table anchors from image centres only"),
         ("V2+C1+C2", "deterministic only (V2) + C1 + C2"), ("V2+C1", "V2 + C1"), ("V2+C1+C2img", "V2 + C1 + C2, image anchors")]
for cl in np.unique(qcls):
    m = qcls == cl; nts = list(range(8)) if cl == "item" else list(range(4))
    print(f"\n#### {R0.NAME}: {cl} queries ({int(m.sum())}), per-item reads only")
    print("| row | " + " | ".join(f"{TH[t]}" for t in nts) + " |"); print("|---|" + "---|" * len(nts))
    print("| matches, share of sample | " + " | ".join(f"{100 * A[t][m].mean() / N:.3f}%" for t in nts) + " |")
    prev = None
    for nm, lab in ORDER + EXTRA:
        if nm not in tracks: continue
        rr = [row(nm, cl, t) for t in nts]
        cells = []
        for i, r in enumerate(rr):
            s = f"{r['item']:.2f} B ({100 * r['item'] / FULL:.3f}%)"
            if prev is not None and (nm, lab) in ORDER: s += f" {100 * (r['item'] / prev[i] - 1):+.0f}%"
            cells.append(s)
        if (nm, lab) in ORDER: prev = [r["item"] for r in rr]
        print(f"| {lab} | " + " | ".join(cells) + " |")
    print(f"\n#### {R0.NAME}: {cl} queries, items touched (% of sample) and all costs combined")
    print("| row | touched | per-cluster B/item | id map B/item | fixed MB/query | total % at N = 1M | total % at N = 10^9 |"); print("|---|---|---|---|---|---|---|")
    for nm, lab in ORDER + EXTRA:
        if nm not in tracks: continue
        rr = [row(nm, cl, t) for t in nts]; f = lambda k, fmt: " / ".join(fmt.format(r[k]) for r in rr)
        print(f"| {nm} | {' / '.join(f'{100 * r['touched']:.1f}' for r in rr)} | {f('clus', '{:.2f}')} | {f('idmap', '{:.3f}')} | "
              f"{' / '.join(f'{r['fixed'] / 1e6:.0f}' for r in rr)} | {' / '.join(f'{100 * r['tot1M'] / FULL:.3f}' for r in rr)} | "
              f"{' / '.join(f'{100 * r['tot1G'] / FULL:.3f}' for r in rr)} |")
