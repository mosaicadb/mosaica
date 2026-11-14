# Cluster size: fewer, larger clusters at 1M

Saved by the coordinating session from the agent's returned text. Every number is the agent's. The
timings, bytes and verify lines were compared with the raw CSVs and logs in `results/clustersize/`. Two
sentences were corrected:
- the noise floor, which the agent understated;
- the group-setup share, which mixed profiled and wall milliseconds.

Code: branch `research/levels-clusters`, on `research/combined` at d443f66, on top of the
deterministic-level change (`REPORT_DETLEVELS.md`). The setup is the same as there:
- a 4-core Xeon VM with AVX-512;
- one thread pinned to core 2, the process on cores 1–3, nothing else running during timing;
- fixed plan constants (`results/bounds/plan_fixed.txt`), `--int --gtab --flags repack`;
- the six-level binary;
- the same 999,000 BioCLIP-2 rows and the same queries.

Scripts, CSVs and the generated tables are in `results/clustersize/`.

## Summary

Larger clusters make text queries faster at every answer size:
- **P4** (4,000 items per cluster, 64 groups): −17% at 0.01–0.1%, −13% at 1%, −8 to −11% at 10–75%.
- **P16** (16,000 items per cluster, 62 groups): −10% at 0.01%, −12 to −18% elsewhere.

Item queries gain 8–17% at answers of 10–75%. They lose at small answers, where the cut prunes less:
- P4: −9%, −5%, +16% at 0.01 / 0.1 / 1%;
- P16: +79%, +139%, +43%.

The per-cluster phases fall roughly as K falls. Part of the saving goes back to per-item work,
because more items enter the levels.

The gain combines with the two-level deterministic schedule. P4 with 16/32 is 18–33% faster than the
baseline for text and 10–23% faster for items.

At 1M the group count matters as much as the cluster count. Group setup is about 0.12 ms per group:
7.7 of the 43 profiled ms of a text 0.01% query at 64 groups (18%). Fewer groups are faster at small
answers, but read 5–27% more bytes.

Every fit gives 0 wrong against brute force at 1M.

## Changes

- **`tools/fit.py`:** `--per-cluster` (default 1,000) and `--groups` (default 0, meaning the existing
  rule `min(64, K/8, N/(20n))`; a value above K is capped at K). The fit log now also prints group
  sizes and clusters per group. The default fit reproduces `c1m`.
- **`src/run2.rs`:** the breakdown also prints, per query, clusters on the index path, clusters sent
  to full vectors, items entering the first level, and item-levels.

**Limits checked in the Rust code.** I found no per-item 16-bit offsets, no fixed per-cluster
buffers, and no assumptions about cluster size:
- in-cluster indices are `u32`;
- the survivor state grows per cluster;
- the u16 fields index coordinates and fields, not items;
- `nukey` is a code on each cluster's own grid, rounded up.

The planner's "fewer than 64 live clusters" check applies only to the scan band, which is off. The
largest cluster in the sweep is 47,978 items. The verify runs below are the evidence that nothing
broke.

## Fits

| Fit | Items per cluster | K | G | cluster sizes | group sizes | clusters per group | basis condition (max) |
|---|---|---|---|---|---|---|---|
| base (`c1m`) | 1,000 | 999 | 64 | 84..4,635 | 1,359..49,701 | – | 21 |
| P4 | 4,000 | 250 | 64 | 579..15,490 | 624..49,491 | 1..11 | 108 |
| P16 | 16,000 | 62 | 62 (one cluster per group) | 2,094..47,978 | 2,094..47,978 | 1 | 23 |
| D4 | 4,000 | 250 | 31 (fit.py's rule) | 579..15,490 | 624..89,266 | 1..21 | 108 |
| D16 | 16,000 | 62 | 7 (fit.py's rule) | 2,094..47,978 | 42,281..356,298 | 4..23 | 13 |

- P and D of the same size have identical clusters (same seed); only the grouping differs.
- P16 cannot reach 64 groups with 62 clusters. Its smallest group (2,094 items) is above n = 768, so
  two clusters per group was not needed.
- P4 and D4 have one group of 624 items, fewer than n. Its covariance is rank-deficient and clipped,
  which explains the condition number of 108. This affects efficiency only: soundness does not
  depend on the basis.

## Method

- Each process builds one index and runs the end-to-end timing (median over 3 runs of the median
  over 20 queries, six answer sizes) and the profiled breakdown.
- In the first pass, verify (20 item and 20 text queries) runs in the same process before any
  timing.
- Order: base, P4, P16, D4, D16, then the reverse.
- The baseline was rerun inside the sweep.
- **Noise floor:** the spread between the two repeats is 0–7% for the baseline (largest: text 50%,
  6.6%). For the other fits most cells are within 6%; the largest are P4 item 1% (11.6%), D16 item
  0.01% (11.4%) and P16 item 10% (7.6%). In the stack runs, P4 with 16/32 at item 0.1% has a 14%
  spread.
- The plan constants were left at the values timed for 1,000-item clusters.

## Results at 1M (measured)

### Text, ms (change against base) [repeats]

| Fit | 0.01% | 0.1% | 1% | 10% | 50% | 75% |
|---|---|---|---|---|---|---|
| base | 31.8 [31.5/32.2] | 38.9 [38.0/39.9] | 62.6 [62.6/62.6] | 94.4 [93.2/95.7] | 122.4 [118.5/126.3] | 116.0 [114.2/117.9] |
| P4 | 26.6, −17% | 32.2, −17% | 54.5, −13% | 84.1, −11% | 112.5, −8% | 105.8, −9% |
| P16 | 28.8, −10% | 31.8, −18% | 55.0, −12% | 81.8, −13% | 106.1, −13% | 99.3, −14% |
| D4 | 22.5, −29% | 27.9, −28% | 49.8, −20% | 79.1, −16% | 107.8, −12% | 101.3, −13% |
| D16 | 20.6, −35% | 27.2, −30% | 49.1, −22% | 79.0, −16% | 106.4, −13% | 101.0, −13% |

### Item, ms (change against base)

| Fit | 0.01% | 0.1% | 1% | 10% | 50% | 75% |
|---|---|---|---|---|---|---|
| base | 2.5 [2.5/2.5] | 3.9 [3.9/3.9] | 13.8 [13.7/13.9] | 81.4 [80.2/82.6] | 113.5 [113.3/113.7] | 110.8 [107.5/114.2] |
| P4 | 2.3, −9% | 3.7, −5% | 16.0, +16% [15.1/16.9] | 74.5, −8% | 103.9, −8% | 98.8, −11% |
| P16 | 4.5, +79% | 9.4, +139% | 19.7, +43% | 71.9, −12% | 100.9, −11% | 91.7, −17% |
| D4 | 2.1, −18% | 3.7, −7% | 12.0, −13% | 67.4, −17% | 100.8, −11% | 92.5, −17% |
| D16 | 3.0, +18% | 5.6, +44% | 14.0, +2% | 70.1, −14% | 100.6, −11% | 91.5, −17% |

### Bytes per item, full-vector share and touched share

| Fit | text 0.01% | text 1% | text 50% | item 0.01% | item 1% | item 50% |
|---|---|---|---|---|---|---|
| base | 19.1 B, 0.43%, t .62 | 67.9, 0.73%, t .96 | 348.3, 11.57% | 3.3, 0.14%, t .03 | 17.5, 0.61%, t .22 | 292.5, 9.08% |
| P4 | 17.3, 0.17%, t .73 | 71.6, 0.75%, t .98 | 356.8, 11.92% | 3.5, 0.13%, t .08 | 16.3, 0.43%, t .37 | 301.9, 9.47% |
| P16 | 17.9, 0.11%, t .82 | 72.1, 0.71%, t .995 | 357.4, 11.92% | 4.7, 0.15%, t .19 | 17.0, 0.34%, t .59 | 296.9, 9.20% |
| D4 | 17.6, 0.10%, t .73 | 76.5, 0.82%, t .98 | 375.4, 12.81% | 2.7, 0.07%, t .08 | 15.4, 0.31%, t .37 | 311.5, 9.88% |
| D16 | 24.3, 0.02%, t .82 | 92.9, 0.98%, t .995 | 429.4, 15.43% | 3.6, 0.03%, t .19 | 20.3, 0.28%, t .59 | 362.3, 12.19% |

- With 64 groups (P), large answers read 1–3% more bytes.
- With fewer groups (D), the bases are poorer: up to +23% bytes and a 15.4% full-vector share for
  D16 at text 50%.
- The entry shares rise at every level with coarser clusters (text 1%, entering 56 B: base .465, P4
  .499, P16 .514, D16 .665). All entry shares are in `tables.md`.

**The planner's choices shift (measured).** At text 0.01%, the base sends 108 clusters per query
straight to full vectors; P4 sends 11 and P16 sends 2. The gap between `full_reads_per_item` and the
last entry share falls from 0.42% to 0.15% and 0.10%. At item 1% the flat clusters fall from 170 to
41 (P4) and 10 (P16). The constants were timed on 1,000-item clusters, so part of P16's item loss
may be planning, not the cut (inferred, not tested).

## Breakdown (profiled ms per query, mean of two repeats)

Per-cluster phases are cut, cluster setup, tables and level setup. Per-item phases are re-pack,
decode, bounds, compaction and level other; full vectors are listed separately. "Groups" is basis,
rotations and group post-processing.

| Cell | Fit | wall | per-cluster phases | per-item phases | full | groups | clusters (index / flat) | tables, units | items entering | item-levels |
|---|---|---|---|---|---|---|---|---|---|---|
| text 0.01% | base | 31.4 | 3.00 + 2.87 + 3.08 + 1.36 = 10.3 | 20.1 | 0.80 | 7.65 | 868 / 108 | 6,220, 122k | 619k | 1.76M |
| | P4 | 26.8 | 0.88 + 1.23 + 1.50 + 0.47 = 4.1 | 22.5 | 0.29 | 7.77 | 239 / 11 | 1,847, 42k | 729k | 2.08M |
| | P16 | 27.1 | 0.24 + 0.66 + 0.79 + 0.21 = 1.9 | 24.9 | 0.19 | 7.50 | 60 / 2 | 488, 13k | 815k | 2.25M |
| | D16 | 22.8 | 1.7 | 31.6 | 0.06 | 1.18 | 62 / 0 | 505, 13k | 816k | 3.13M |
| text 1% | base | 62.4 | 16.4 | 55.8 | 1.59 | 8.14 | 994 / 5 | 9,089, 362k | 955k | 5.16M |
| | P4 | 54.0 | 6.9 | 57.2 | 1.64 | 8.38 | 249 / 1 | 2,383, 107k | 979k | 5.42M |
| | P16 | 53.0 | 3.4 | 58.5 | 1.45 | 7.67 | 62 / 0 | 605, 29k | 994k | 5.49M |
| text 50% | base | 124.5 | 20.2 | 106.2 | 22.6 | 8.27 | 992 / 7 | 9,902, 499k | 998k | 8.69M |
| | P4 | 112.6 | 8.6 | 108.6 | 24.1 | 8.77 | 249 / 1 | 2,487, 126k | 998k | 8.75M |
| | P16 | 107.0 | 4.1 | 103.1 | 23.0 | 8.18 | 62 / 0 | 620, 31k | 999k | 8.76M |
| item 1% | base | 14.4 | 5.7 | 9.6 | 1.11 | 3.35 | 353 / 170 | 2,632, 65k | 212k | 0.83M |
| | P4 | 14.5 | 2.8 | 12.6 | 0.79 | 5.11 | 146 / 41 | 1,108, 28k | 362k | 1.09M |
| | P16 | 19.6 | 1.5 | 15.8 | 0.62 | 6.06 | 50 / 10 | 388, 9k | 588k | 1.37M |
| item 50% | base | 113.4 | 19.1 | 97.9 | 17.5 | 7.93 | 973 / 15 | 9,665, 482k | 975k | 8.22M |
| | P4 | 103.8 | 7.9 | 99.1 | 18.6 | 8.17 | 245 / 3 | 2,432, 121k | 980k | 8.28M |
| | P16 | 97.1 | 3.9 | 99.9 | 18.0 | 7.84 | 62 / 0 | 613, 31k | 984k | 8.29M |

D4 and the remaining D16 cells are in `tables.md`.

**Derived costs (from the profiled times, which run about 1.3–1.4x the unprofiled wall):**
- **Cut:** 3.0 µs per cluster at base, 3.3–4.5 µs at the coarser fits, paid for every cluster.
- **Opened cluster (cluster setup + tables + level setup):** 8–17 µs at base, 13–31 µs at P4, 25–61
  µs at P16. It grows with the units per table.
- **Tables per unit:** about 23 ns at base and about 62 ns at P16 (text 1%). Inferred cause: with
  one cluster per group, the group table of `gtab` is no longer shared across clusters.
- **Per item-level (re-pack through level other):** 10–12 ns, the same at every fit.
- **Group setup:** about 0.12 ms per group (7.65 ms for 64, 3.77 for 31, 1.18 for 7).

**Where the time goes.**
- Text 0.01%: P16 removes 8.4 ms of per-cluster work and adds 4.8 ms of per-item work (32% more items
  enter the first level, because the cut prunes less).
- Text 50%: per-cluster work falls by 16 ms; per-item work is about flat.
- Item 1%: the per-item and group costs grow faster than the per-cluster costs fall, because more
  clusters (and so more groups) are opened.

**Extrapolation (inferred).** A text query costs roughly:
- G × 0.12 ms for groups;
- K × (3 + 8 to 17) µs for clusters;
- 11 ns per item-level;
- full vectors.

All are profiled figures; scale by about 0.72 for wall time. At fixed items per cluster, both the
cluster and item terms grow linearly in N. The group term does not grow, because G is capped at 64.
At 10M with 1,000-item clusters, text 0.01% would spend about 110 ms (profiled) per-cluster and about
200 ms per-item, so coarser clusters would save a similar share of the total as here. The group term
falls from 18% of the query to about 2%, so the D fits' advantage at 1M would mostly disappear.

## Do the gains stack (NDL 2, `--dlv 16,32`; measured, two repeats)

| Variant | text 0.01% | text 1% | text 50% | item 0.01% | item 1% | item 50% |
|---|---|---|---|---|---|---|
| base, 6 levels | 31.8 | 62.6 | 122.4 | 2.5 | 13.8 | 113.5 |
| base, 16/32 | −19% | −11% | −5% | −10% | −12% | −6% |
| P4, 6 levels | −17% | −13% | −8% | −9% | +16% | −8% |
| P4, 16/32 | 21.3, −33% | 46.8, −25% | 100.8, −18% | 1.9, −23% | 12.4, −10% | 92.9, −18% |

The other cells for P4 with 16/32 are text −33 / −22 / −18% at 0.1 / 10 / 75% and item −19 / −22 /
−21%. The bytes are those of each part: for example 22.3 B at text 0.01% (base 19.1) and 354.9 B at
text 50% (base 348.3).

The gains roughly add. The cluster change removes per-cluster work; the level change removes
per-level, per-block work.

## Correctness (1M, `verify --vitem 20 --vtext 20`: brute force at 7 sizes, filtered sets, k-NN)

| Fit | Wrong in / out | det / rand violations | Decisions not implied | k-NN | Largest randomised error |
|---|---|---|---|---|---|
| base | 0 / 0 | 0 / 0 (767M checks) | 0 of 112M | 0 of 40 | 5.42 sd |
| P4 | 0 / 0 | 0 / 0 (789M) | 0 of 125M | 0 of 40 | 5.34 sd |
| P16 | 0 / 0 | 0 / 0 (793M) | 0 of 149M | 0 of 40 | 5.49 sd |
| D4 | 0 / 0 | 0 / 0 (817M) | 0 of 117M | 0 of 40 | 5.51 sd |
| D16 | 0 / 0 | 0 / 0 (915M) | 0 of 117M | 0 of 40 | 5.34 sd |
| P4, 16/32 | 0 / 0 | 0 / 0 (789M) | 0 of 125M | 0 of 40 | 5.34 sd |

The 50k check and the float64 oracle were not run on the coarse fits.

## What I would turn on

- **About 4,000 items per cluster with 64 groups, together with the 16/32 deterministic schedule.**
  It is the best overall: text −18 to −33%, item −10 to −23% at every size measured. Alone, P4 costs
  item 1% (+16%).
- **Not 16,000 per cluster.** Small item answers are 1.4–2.4x slower.
- **Not fewer groups as a default.** At 1M they help small answers through group setup, but they
  read up to 27% more bytes, and the group cost does not grow with N (inferred).
- The planner's constants should be retimed for any new cluster size before a final comparison.

## Limits and what was not done

- One VM, one 1M sample, one seed per fit, 20 queries per class, two repeats per fit.
- The plan constants were not retimed for larger clusters; the planner's per-cluster choices shift
  (above).
- Not run on the coarse fits: the 50k checks and the float64 oracle. Only the 1M verify was run.
- The stack test ran in separate processes after the sweep. Its baseline is the sweep's six-level
  base, not an in-process one.
- No intermediate sizes (2,000 or 8,000 per cluster) and no group counts between 7 and 64 except 31.
- The extrapolation to larger N is a linear model fitted to profiled phases; it was not measured.
