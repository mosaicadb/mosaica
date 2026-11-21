# Partition experiment: results

Experimentalist's results for `../EXPERIMENT-SPEC.md`, in the spec's order. The log of what was run is `NOTEBOOK.md`.
Every number here is **[measured]** on the 200k BioCLIP-2 sample (`vexact-data/bioclip2-200k`, 200,000 rows,
n = 768), unless marked otherwise, with these common conditions:

- Split: permutation seed 1; 149,922 fit rows, 49,978 held-out rows; the 100 item-query rows are in neither and in no
  count. Coding scores (S1) are means over the **49,978 held-out rows**, with centres and bases fitted on fit rows.
  S2, S3, S2+3, S4 and S5 are over **all 199,900 valid rows** (not held out: those bounds must hold for every row),
  for the **830 queries** of `setup.npz` (369 held-out text prompts, 361 in-sample species prompts, 100 held-out item
  queries), at answers of 0.01, 0.1 and 1%.
- Σ_Q held fixed: A = (Si + 0.1·tr(Si)/n·I)^½, and again with Sm for the coding scores. K = 200, G = 12.
- The level model is the floor's idealisation (a coded coordinate's error is sᵢ times the coordinate, water-filling
  per group), **not the built code's bytes**. Ratios between partitions are what these scores are for.
- Violations: every row decided by the cut or by the range, for every query and answer size, was checked against
  its exact score (slack 1e-5). **The count is 0 for every partition in this file.**
- The seed spread is the largest gap among A1, A2, A3 (build seeds 1, 2, 3). A difference smaller than twice it is
  not a result, and is said so.

Scripts: `lib.py` (all scores), `item12.py`, `item3.py`, `itemB.py`, `itemC.py`, `itemD.py`, `sweep.py`,
`report.py` (tables), `diag_groups.py`; logs and score files in `work/` (ignored by git).

## Item 1. How far the built partition is from a fixed point (P1.6)

`item12.py lloyd`, log `work/item12_lloyd.log`. A1 (build seed 1) continued by Lloyd on all fit rows: every row to
the stored centre (fit-row mean) of largest inner product, centres to the means, groups re-made by the build's rule
(fresh generator, seed 1) at each reported pass. It reached under 0.1% of fit rows moved at pass 53 (A1-fp); no
cell was emptied. S0 for the other partitions: A0 3.32%, A2 4.39%, A3 4.18% (all valid rows).

