# Partition experiment: notebook

Experimentalist's log for `../EXPERIMENT-SPEC.md` (the version after referee 1). Started 21 November 2026, 09:13.
Results with tables go to `RESULTS.md`; this file says what was run, with what, and where the spec was departed from.

## Set-up and departures from the spec, decided before any result

- Machine rules are the PI's brief of 21 November, which replace the spec's section 0 where they differ: every job
  through `../../run-limited.sh 0-3 4 12` (cores 0 to 3, 4 threads, 12 GB address space), not one thread under
  100 MB. So the rows are held in memory as float32 (200,000 × 768, 614 MB) instead of being streamed in chunks of
  10,000, and outputs go to `notes/program/partition/experiment/` (scripts, logs here; caches in `work/`), not
  `partition/work/`.
- Rows: the four f16 files, each row normalised in float64 and stored float32. Exact scores q·x for the 830 queries
  are computed once in float64 and stored float32 (`work/scoresT.npy`, 664 MB); checked against fresh float64
  products to 1e-8.
- Split: permutation seed 1, positions ≡ 0 mod 4 held out. The 100 item queries' rows are taken out of both halves
  and out of every count (they still enter the extremes E± and every cell, so the cut's bounds hold for them).
  Fit rows 149,922; held-out rows 49,978.
- Violations are counted with a slack of 1e-5 on the score (float32 storage of rows and scores).
- The stored centre is the mean of the cell's fit rows in float64, not rounded to 8 bits as `fit.py` does. A0's
  centres are recomputed the same way (its stored `Mean` is over all rows); A0's cells were fitted on all rows, so
  A0 is not held out at the cell level.
- Groups by the build's rule on the unit directions of those means (`kmeans(·, 12, rng, 20)` continuing the
  partition's generator, as `fit.py`). For the Lloyd passes of item 1 the groups are re-made with a fresh generator
  of seed 1 each time.
- Level check (`prep_cache.py`, log `work/prep_cache.log`): on fit rows of groups 0 and 5 of A0, the coordinate
  variances of z equal √γ to 6 digits and the mean of w_B² equals W_g(B)² at B = 0, 4, 16, 32; the water level uses
  exactly 8B bits.

## Log

- 09:13. Read README, notation, the spec, CLAIMS P1.6, P3.1, P3.2, P3.5, P5, stage-0 `prep.py`/`measure.py`,
  `fit.py`, `core.py::kmeans`. Wrote `lib.py` (all shared code and the scores), `prep_cache.py`.
- 09:17. `prep_cache.py`: caches (scores, each query's 10 best valid rows, the 10 nearest rows of 2,000 sample rows
  (seed 5)), the level check above. 32 s.
- 09:19. `item12.py seeds` started: A0 and A1, A2, A3 (seeds 1, 2, 3) with S0 and every score.
- 09:21. Replaced the bisection for S1b by an exact solution (on each interval between eigenvalues the width is a
  quadratic in √θ); `check_bytes.py` agrees with an 80-step bisection to 1e-12 B and the width at the solved level
  equals w* to 1e-15. A0's first score used the bisection; all others the exact form.
- 09:25. Families and orders: the sample holds 3 families and 3 orders (one or two per file), so "cells met by one
  family" is near-trivial. Added genus (45) and species (scientific name, 989) beside them; marked in the tables.
- 09:28. `item12.py seeds` done (183 s): A0, A1, A2, A3 scored. 0 violations.
- 09:30. `diag_groups.py`: A1 has a group of 617 fit rows (< n), A2 two groups of 1 and 3 fit rows. The 617-row
  group's held-out rows cost 200 B at w* = 0.06.
- 09:31. `item12.py lloyd`: fixed point (< 0.1% moved) at pass 53; scored at passes 1, 2, 5, 10, 20, 53. 0 violations.
  S0(A1) = 4.41% > 2%, so the baseline is A1-fp by the spec's rule.
- 09:33. `item3.py A1-fp` started; items 1 and 2 written to RESULTS.md while it ran.
- 09:38. `item3.py A1-fp` done (294 s). Item 3 written. `itemB.py A1-fp` started. (Times before this entry are
  from the job logs; from here they are from `date`.)
- 09:52. `itemB.py A1-fp` done (815 s): B0/B16 in-sample and cross-fitted, single moves from each in-sample result
  (about 400 eigendecompositions each, 5 min), six groupings scored. In-sample objectives never rose. Drop rule for B
  fires for every variant (gains 0.5 to 2.5% in S1b against twice the seed spread, 5.9%). Item 4 written.
  Decision for item 5: C starts from **B16-in** (the theorem's in-sample scheme, before the single moves), since
  the spec names "B16's result" and the in-sample run is the one the descent covers; B16-cf ended 0.01% away.
- 09:52. `itemC.py A1-fp B16-in first` started.
- 09:57. C first step done (117 s, after fixing an index error in the held-out width). C16 moves 9.96% of fit rows,
  so its first-step rule does not fire. `itemC.py ... run C16` started.
- 10:16. C16-in, Cmix, C0 run (15 passes each, 6 to 7 min each). C16 drop rule fires on its S1b clause (2.4% < 3%).
  Item 5 written; two statements corrected before saving (S4b for item queries under C16 and C0 is under twice
  its seed spread, so not a result; the net figure's range is 0.7 to 2.4%).
- 10:21. `itemD.py A1-fp` done (49 s). The cap is 825 fit rows; small cells stay small (10 under 500). Drop rule does
  not fire. Item 6 written.
- 10:22. Sweep started: `sweep.py seeds`, then sub K = 50, 100, 200, then full K = 50, 100, 200, 400, 800.
- 10:30. Sweep done (8 runs, 30 to 120 s each). The 150k, K = 200 run reproduces A1-fp-one exactly (same partition).
  Model U (a) drop rule fires (slope ratio 2.96); (b) and P2.2 do not. Item 7 written.
- 10:33. Summary table, drop-rule table, limits and file list written to RESULTS.md. Total violations over 23
  partitions scored for S2/S3 (15 at K = 200, 8 in the sweep): 0. `work/` is 0.7 GB. No process of mine left running.
