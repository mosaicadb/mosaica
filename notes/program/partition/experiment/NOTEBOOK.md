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
