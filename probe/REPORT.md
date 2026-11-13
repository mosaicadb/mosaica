# Probe-cluster experiments (partial: run stopped before completion)

Status: INCOMPLETE. The run was cut off before it finished. Finished and logged: Experiment A on
nytimes-256 (K = 256, 1,024, 4,096, all 10,000 test queries) and Experiment A on glove-100 at
K = 1,024. Not finished: glove K = 4,096 and 16,384, the two-level tree with bound-ranked groups
(`expA_tree.py`, never run), and every full Experiment B run (`expB.py`; only a 100-query smoke test
on nytimes ran, and it is not recorded here). All background processes were killed by PID.

Measured: counts of items scanned and recall (ANN-Benchmarks `knn_threshold`: d_k + 1e-3, confirmed
against `ann_benchmarks/plotting/metrics.py`). nytimes has 9 zero-vector test queries with NaN
distances, which ANN-Benchmarks scores 0, so tolerant recall is capped at 0.9991.

Tables: `tables_partial.md` (from `report.py out`); logs: `expA_*.log`; curves: `out/expA_*.json/png`.

## Run 2: completed (2026-11-13)

All runs finished; merged tables in `tables_run2.md`, logs in `logs/run2/`. Run by the coordinating session from the scripts above, one thread per core on cores 8–11.

- **Probing (A).** Balanced k-means with a SOAR second assignment, clusters ranked by centre score + 0.35 × radius, is the best structure everywhere. Share of items scanned for recall 0.9 / 0.95 / 0.99: glove K = 16,384 0.91 / 2.2 / 7.8%; glove K = 4,096 1.4 / 3.3 / 11.3%; nytimes K = 4,096 4.8 / 12.2 / 39.2%. Balancing matters only on nytimes at large K.
- **Tree over centres.** On nytimes the bound-ranked tree needs nearly every centre to match the flat pass's recall. On glove K = 16,384 a two-level tree scores 4,712 centres (29%) for recall 0.9 at 1.34% scanned (flat: all centres, 1.08%), and 9,531 (58%) for 0.99 at 15.4% scanned (flat: 10.6%). Three levels did worse. A tree saves centre scoring by at most about 3× at 100 dimensions and nothing at 256.
- **Dial (B).** m fitted on corpus pseudo-queries reaches its target on real queries in every case but two near misses on glove (0.948 for 0.95, 0.9897 for 0.99), and is within 0.25 of the hindsight-best m. Re-ranks per query at 0.999: glove 2-bit 417, nytimes 2-bit 368, BioCLIP (held-out items) 2-bit 58. nytimes test queries need about 10× the re-ranks of pseudo-queries at small m.

## Lossless best-first tree (2026-11-13)

`bestfirst.py` expands a hierarchy over the centres best-first by true upper bounds (group score + bound radius, which is valid for unit queries), so it picks exactly the flat pass's clusters and scans the same items; only the centres scored can differ. `bestfirst_balanced.py` repeats the two-level case with groups of equal size. Log: `logs/run2/bestfirst.log`.

Result: every shape opens every group, on glove (K = 4,096 and 16,384) and nytimes (K = 4,096), at every recall target, so it scores 1.01–1.07× the flat pass's centres. The plain k-means grouping of centres was lopsided (on nytimes half the groups held one centre), but balanced groups give the same answer. The reason is scale: a group's bound radius (median 0.69–1.03) is about ten times the spread of leaf keys (sd 0.087–0.136), so no group's bound ever falls below the k-th leaf key.
