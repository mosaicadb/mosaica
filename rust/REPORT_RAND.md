# Cheaper randomised levels: results at 1M

Branch `research/rand-levels`, rebased onto `research/setup-cost` at 9413bc31e. Measured on the Ryzen 9 5900X (WSL2,
AVX2), one thread pinned to core 5, fixed plan constants (`results/setup/plan_fixed.txt`), s3 configuration
(`--int --gtab --flags repack`), 1M BioCLIP data-only dual index. Scripts: `results/rand/run.sh`, `check.sh`.

## Summary

At answers of 10–75%, threshold queries are 29–35% faster, and at 0.01–1% they are 6–24% faster. They read 3–11% more
bytes. Most of the gain comes from one change: four randomised levels instead of fourteen. That removes three quarters
of the per-level bound, decision, compaction and re-pack work for every item that reaches those levels. Compacting only
the state a later level reads takes another 3 ms at large answers. The index now beats a plain scan of every row
(87–89 ms) at every answer size, so the planner's new scan switch is built but stays off.

## Steps

**1. Four randomised levels: 56, 96, 160 and 256 B** (commit 9f4b2329f).
- **The schedule.** `tools/fit.py` now uses 56, 96, 160 and 256 B, then steps of 1.6x. The old schedule was 40, 48, …,
  256 B, then steps of 1.25x. `--rlv` overrides the schedule of an existing index.
- **The guarantee.** The union bound runs over N × L (item, level) events, with one event per level. With fewer levels
  its constant falls, c = 9.46 → 9.33. The rotations, the blocks and the bound per block are unchanged. Rates are still
  derived per level and only rise. So the existing proof covers the new schedule, and no new argument is needed.
- **The schedules compared.** Each set was run interleaved in one process (`e2e_rlv_14_7_5.csv`, `e2e_rlv_14_4_3.csv`):

  | Levels | item 50% | text 1% | text 50% | bytes vs 14 levels |
  |---|---|---|---|---|
  | 14 | 110 | 54 | 116 | – |
  | 7 | 88 | 44 | 97 | +1–4% |
  | 5 | 79 | 42 | 89 | +2–7% |
  | 4 | 78 | 40 | 84 | +3–11% |
  | 3 | 74 | 41 | 78 | +8–22% |

  Four levels gives most of the gain for few extra bytes. Three levels reads too much more.
- **Plan flag `rdec=…`.** It decides only at the listed randomised levels; the others only read. It also skips any
  kappa that a later rise replaces before the next level that decides. It is covered by the same union, because it
  evaluates a subset of the events. On 14 levels it gained 7–13% (`e2e_rdec_schedules.csv`), less than the coarser
  schedule, because per-level reading and re-pack remain. It is off by default.
- **Compressed pushes.** Decided rows are appended with one compressed store per 16 lanes instead of a lane loop.
- **The planner's scan switch.** The plan constants `scan_lo` and `scan_hi` set a band of answer shares. A query
  estimates its answer share from 256 evenly spaced visible rows' full vectors (about 28 µs), but only when at least a
  quarter of the visible items are candidates. If the estimate falls in the band, the query scans the visible set.
  `Plan::scan_band` calibrates the band from stored rows at 5–99% answers, and `scanband=LO:HI` sets it by hand. It is
  off by default, for the reason in step 2.

**2. Compaction keeps only the state a later level reads** (commit b2ab4b3e5).
- **What a compaction keeps.** After a randomised level:
  - the accumulators of blocks coded so far;
  - kappa and tan only for a coded block that does not rise at the next level, since a rise recomputes them before
    any use;
  - |y| from the first randomised level on;
  - after the last level, only the slots.

  On four levels every coded block rises at every level, so 8 of the 20 arrays drop out of every randomised
  compaction. Compaction falls from 7.5 to 4.5 ms at item 50%, and from 4.8 to 3.7 ms at text 1%.
- **Scalars decoded once per row.** |y|, kappa and tan are decoded with the scalar row outer and the block's two
  halves inner.
- **Plan flags now override timed constants.** They apply after the timed or filed constants, so `scanband=` takes
  effect over a calibration.
- **The index against a plain scan** (`e2e_index_vs_scan.csv`). The scan of every row takes 87–89 ms. The index takes:

  | Answer | item | text |
  |---|---|---|
  | 30% | 74.0 ms | 78.8 ms |
  | 50% | 77.6 ms | 82.9 ms |
  | 75% | 72.1 ms | 78.9 ms |

  The index wins at every size, so the band stays off.

## Correctness at 50k (200 item and 200 text queries, 7 answer sizes, filtered sets, k-NN, float64 oracle)

| Run | Wrong | det / rand violations | Decisions not implied | Oracle |
|---|---|---|---|---|
| Start (14 levels) | 0/0 | 0/0 | 0 of 10.6M | 0 wrong |
| 7, 4, 3 levels | 0/0 | 0/0 | 0 | 0 wrong |
| 14 levels, `rdec=0.3.6.9.11.13` and scan band | 0/0 | 0/0 | 0 | 0 wrong |
| Step 2, four levels (default) | 0/0 | 0/0 | 0 | 0 wrong |
| Step 2, four levels, scan band 0.2–0.9 (scan taken) | 0/0 | 0/0 | 0 | 0 wrong |
| Step 2, 14 levels, `rdec=1.3.6.9.13` | 0/0 | 0/0 | 0 | 0 wrong |

The largest randomised error was 5.47 sd (c = 9.33 on four levels). k-NN matched on 400 of 400 queries in every run.

## Before and after at 1M

**How it was measured.** Each figure is the median over 3 runs of the median over 20 queries. Each side was run twice,
as alternate processes: start, new, start, new. The table shows the mean of the two.
- **Start:** 9413bc31e, 14 levels.
- **New:** b2ab4b3e5, 4 levels.

Bytes are per item of N, and include full vectors.

| Answer | item ms start | new | Δ | item B start | new | text ms start | new | Δ | text B start | new |
|---|---|---|---|---|---|---|---|---|---|---|
| 0.01% | 2.4 | 2.3 | −6% | 2.8 | 3.0 | 21.1 | 18.9 | −11% | 20.1 | 20.9 |
| 0.1% | 3.4 | 3.1 | −10% | 5.8 | 6.0 | 27.3 | 23.4 | −14% | 25.1 | 26.8 |
| 1% | 8.7 | 7.6 | −13% | 18.7 | 19.3 | 52.7 | 39.9 | −24% | 64.6 | 71.5 |
| 10% | 79.7 | 56.2 | −30% | 136.0 | 145.6 | 89.3 | 63.0 | −29% | 192.0 | 203.2 |
| 50% | 115.4 | 77.7 | −33% | 296.6 | 307.2 | 123.1 | 83.5 | −32% | 347.0 | 359.5 |
| 75% | 111.2 | 72.2 | −35% | 254.0 | 264.9 | 120.9 | 78.8 | −35% | 300.5 | 313.7 |

The two runs of each side agree within 2%. The files are `results/rand/e2e_final_{base,new}{1,2}.csv`.

## On by default

- Four randomised levels, through `fit.py`. An existing index needs `--rlv 56,96,160,256`, or a meta.json with that
  RLV and the matching CSG.
- Lean compaction, and scalars decoded once per row.

`rdec`, the scan band and `bail` are off by default.

**Caveat for low dimensions.** The cap on the schedule leaves very small n (for example 100) with a single randomised
level at 56 B, where there used to be several. That case was not measured.

## Breakdown now (ms, one thread, 20 queries, profiled; `breakdown_final.csv` against `breakdown_start.csv`)

| Cell | wall | tables | re-pack | decode | bounds+dec (of which scalars) | full | compaction | level other |
|---|---|---|---|---|---|---|---|---|
| item 50% start | 104 | 6.8 | 8.9 | 16.8 | 44.0 | 10.4 | 8.0 | 10.1 |
| item 50% new | 78 | 5.5 | 4.7 | 17.6 | 23.1 (12.3) | 11.9 | 4.5 | 6.6 |
| text 1% start | 48 | 5.0 | 6.5 | 7.3 | 12.9 | 1.0 | 5.1 | 5.3 |
| text 1% new | 41 | 4.3 | 4.3 | 7.3 | 9.0 (4.9) | 1.2 | 3.7 | 4.2 |

The wall column is unprofiled. The profiler adds about 10 ms at large answers, mostly to the in-loop phases.

## Next bottleneck

At large answers, three costs are now similar in size:

- **Bounds and decisions:** about 23 ms. About half of it is the randomised scalars: the |y| and kappa gathers, tan,
  and the accumulator updates.
- **Decode:** about 18 ms.
- **Full vectors:** about 12 ms. About 10% of items are still undecided at 256 B.

Ideas for each, none tested here:

- **The scalars.**
  - Hold kappa·aj as one per-block product.
  - Precompute the item's constant terms (qm + α·mr + s·adet, and the fixed half-width) once at the first randomised
    level. That drops s, mr and adet from the randomised state.
  - Both change only the f32 evaluation order, which the existing 16u and 1 + 1e-6 slacks already cover.
- **Decode.** It is now the largest single phase, and it is bound by the bytes read.
- **Full vectors.** Fewer items reaching them needs tighter randomised bounds, which is a change to the code, not to
  the kernel.

At small text answers the fixed per-query costs dominate: basis, rotations, cut and cluster setup, together 6–7 ms of
19 ms.
