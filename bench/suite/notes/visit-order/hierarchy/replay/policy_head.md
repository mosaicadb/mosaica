# The one-process policy against the tuned orders: replay

2026-11-18. A replay of the process specified in `policy.md` (sections 1, 4 and 9; written in another worktree)
on the three suite datasets and their real fits, with the fit, groups, bases and row order unchanged and 16
sub-cluster labels per cluster. Scripts: `replay/calib.py` (the exceedance table and its checks), `replay/policy.py`
(the arms), `replay/ptables.py`, `replay/ctables.py`; full output in `replay/results/`. Counts are **measured** in
the replay; microseconds are **modelled** from counts. 100 real queries per dataset; differences under about 10% are
noise. Nothing was timed.

## Verdict per arm

| arm | verdict |
|---|---|
| table, tier S0 (spread = sd(q)) | **Refuted.** Miscalibrated beyond e^0.5 on real queries on every dataset (δ 1.17 LAION, 0.70 glove, 0.58 yahoo), plain and shifted pseudo-queries disagree (glove: ln(obs/pred) −0.42 and +0.84), and its order is worse than `spread` at fixed rows on LAION and glove (1.32 and 1.16 times the rows at recall 0.9). |
| table, tier S1 (group covariance) | **Calibrated in the pool, not per query.** δ 0.33 (LAION text), 0.05 (glove), 0.49 (yahoo) on real queries. The per-query count factor at k = 10 has a 10 to 90% range of e^−2.5 to e^+1.6 (LAION), e^−3.2 to e^+1.6 (glove), e^−1.8 to e^+0.6 (yahoo), in-distribution queries and plain pseudo-queries included. At 1% answers it is tight. |
| P0 (process, S0) | **Fails**: 2.5, 4.8 and 1.2 times T1's modelled cost at recall 0.9 unfiltered (LAION, glove, yahoo). |
| P1 (process, S1) as specified | **Fails unfiltered**: 1.66, 1.40, 0.93 times T1 at 0.9 (T2: 0.83, 0.86, 0.75). Under visible sets: 0.62 to 0.94 of T1 on glove and yahoo; on LAION 0.81 to 1.46, worse than T1 on five of the six sets. |
| P2 (process, H r16 spread) as specified | **Level with T1 unfiltered, behind T2**: 1.04, 1.00, 0.79. Under visible sets 0.54 to 0.90 of T1. |
| the order alone (O1, O2: by P at τ*, fixed probe count) | **Holds**: O1 is level with `spread` (1.01, 0.98, 0.92 at 0.9; 0.86, 0.96, 1.03 at 0.95); O2 is level with H r16 (0.87, 0.86, 0.83 against 0.83, 0.86, 0.75). No constant was set by hand. |
| the τ estimate | **The specified update hurts.** True τ is worst (P1: 2.87, 2.54, 1.42), τ̂ re-solved at epochs in between, **τ\* solved once and never updated is best**: P1\* 0.79, 0.72, 0.87 and P2\* 0.77, 0.66, 0.70 of T1 at 0.9, P2\* ahead of T2 on all three, P1\* on LAION and glove. |
| P1\*, P2\* (one plan at τ\*, price) | **Confirmed in the sense of section 9**: at least as good as T1 on every dataset and condition run (0.52 to 0.92 of T1's modelled cost at 0.9 for P2\*; unfiltered, random 1% and 25%, whole clusters 1% and 25%, cap 1% and 25%), and as good as or better than T2 except yahoo unfiltered at 0.95 (0.80 against 0.78). |
| sub-clusters, E1, as specified (P1s, P2s) | **Loses to the tuned two-level arm**: P1s 1.14, 1.06, 0.74 and P2s 1.02, 0.70, 0.68 against T4's 0.65, 0.60, 0.46 (with the planner's path, P2si: 1.03, 0.60, 0.48). |
| sub-clusters, E1, one plan (P1si\*, P2si\*) | **Best arm measured unfiltered**: 0.56 / 0.47 (LAION, P1si\*, at 0.9 / 0.95), 0.44 / 0.48 (glove, P2si\*), 0.44 / 0.52 (yahoo, P2si\*) of T1, against T4's 0.65 / 0.66, 0.60 / not reached, 0.46 / 0.58. Under random 1% E1 expands nothing and the arm keeps the process's gain (0.77 to 0.88), where the tuned sub-cluster arms cost 1.2 to 2.2 times T1 or do not reach recall 0.9. |
| group term (a_g in the plan) | **Works, matters little at the measured group cost**: without it P1 opens 9.7 groups against 6.2 on LAION at 0.9 for 1% more cost. T3 (the fixed penalty) is 0.97 to 1.05 of T1. |
| thresholds (τ = t, one plan) | P2: 1.06, 0.79, 0.77 of T1 at 0.01% answers; 0.93, 0.83, 0.76 at 1%; 0.98, 0.94, 0.88 at 10%. P1 fails on LAION at 0.01% (1.86): with τ given there is nothing to normalise the per-query factor. P0 fails at 0.01% everywhere (2.2 to 2.4). With sub-clusters (P2si): 1.03, 0.55, 0.49; 0.87, 0.71, 0.62; 0.93, 0.86, 0.80. |
| correlated visible sets | **No collapse in cost at recall**: P2\* 0.57 to 0.92 of T1 at 0.9 on whole-cluster and cap sets. The table is miscalibrated there on LAION and yahoo (δ up to 1.0 and 1.6; 0.1 to 0.2 on glove; median count factor e^+0.6 to e^+0.9 on LAION cap 1%, e^−1.3 on yahoo whole clusters 1%), so a price read as a recall target would be off; the cost curve is not. |
| clairvoyant plan (true counts) | 0.20, 0.14, 0.34 of T1 at 0.9: what the mean, spread and radius do not carry. |

The spec's three refutation conditions, tested: (i) miscalibration on real text queries beyond e^0.5: **yes for S0;
no in the pool for S1 and S2 (0.33, 0.30), yes per query at k = 10 for every tier and every query type**; (ii) P0
worse than `spread` at fixed rows: **yes** on LAION and glove; (iii) correlated sets under-delivering: the table is
miscalibrated there, **the recall per cost is not worse than the tuned orders'**.

## What was run

- **Calibration** (`policy.md` 4.3) per index on its corpus only: 300 corpus rows as plain pseudo-queries (own row
  removed) and 300 shifted as `scripts/orders5.py` builds them; thresholds at answer fractions 10⁻⁶ to 10⁻¹; g a
  non-increasing function on 65 values of ζ (weighted isotonic fit of the Poisson rates, the tail continued at its
  last slope so that far units stay ordered), β by Newton steps, per level (cluster, sub-cluster) and tier. Fitted
  on half of the pseudo-queries, checked on the other half and on the real queries, which the fit never sees.
  Tiers: S0; S1 (the group's pooled residual covariance, rank 16 and an isotropic rest, scaled by rad²/tr); S2
  (the unit's own covariance: rank 16 for a cluster, rank 4 for a sub-cluster; evaluated for every unit, not
  lazily, so its laziness is not measured).
- **Process** (1.1 to 1.4): values n_vis·P(τ); per cluster the direct read, the index read or the expansion; a
  group opened when the index path saves more than λ·a_g over its clusters; best-first by density with a group's
  setup before its first use; epochs when the rows read have doubled, at which τ̂ is re-solved from the items found
  (counted exactly from the true scores) plus the predicted remainder; stop when the plan at λ is empty; λ swept
  over 19 values from 3 to 3·10⁻⁶ answers per µs. E1 for sub-clusters. E2 was not implemented.
- **Arms.** T1 `spread` 0.4; T2 H r16 over 4·nprobe candidates; T3 `spread` with the group penalty 0.25·σ(q);
  T4 16 sub-clusters two-level (first 64 clusters); T5 group-limited (groups of the first 32 clusters); all
  stopped by a probe count, each probed cluster on the path the planner's rule gives it (direct at or below
  64 + 0.186·S visible rows). O0, O1, O2: the process's order alone (clusters by P at τ\*) under a probe count.
  P0, P1, P2 and their sub-cluster versions (`s`); `i`: each cluster's path fixed by the planner's rule, as the
  tuned arms, which isolates order and stopping from the choice of path; `*`: τ\* solved once, one plan; `ag0`;
  `tau` (true τ); `clair` (true counts as values).
- **Conditions.** k = 10: unfiltered; random 1% and 25%; whole clusters of the real fit 1% and 25%; a cap (the
  items nearest one corpus item) 1% and 25%. Thresholds with answers of 0.01%, 1% and 10%.
- **Cost model** (µs), per dataset, from the real index's phase timings on these fits
  (`../results/phases-1-int8-order.txt`), not the planner's `COSTS`:

  | | group | cluster on the index path (b) | row, index path | cluster read directly | row, direct | centre |
  |---|---|---|---|---|---|---|
  | LAION (n 512) | 27 | 4 | 0.057 | 0.2 | 0.07 | 0.07 |
  | yahoo (n 384) | 14.5 | 4 | 0.04 | 0.2 | 0.045 | 0.067 |
  | glove (n 128) | 4 | 4 | 0.027 | 0.2 | 0.03 | 0.045 |

  Group: the group phase over the mean groups among the first P clusters. Row on the index path: the rest of the
  query at nprobe 16 over the rows scanned, after 4 µs per cluster. **The per-cluster and per-row parts could not
  be separated from the phase figures; b = 4 is the planner's range (1.6 to 7.8), assumed.** Row direct: the
  stored-vector phase per row (170 to 200 µs for the first cluster unfiltered; 23 µs for about 320 visible rows
  under random 1%), with 0.2 µs per cluster assumed. By these figures a direct read costs only 10 to 25% more per
  row than the index path, so the plan often reads a cluster directly and opens few groups (LAION, P1\* at 0.9:
  14.7 clusters, 3.6 groups). If the index path is cheaper per row than this at the code levels the approximate
  mode reads, the arms without `i` are too optimistic; the `i` arms do not depend on it. A sub-cluster read is
  priced at the index-path row cost for its visible rows, b once per expanded cluster and 16 centre scores; the
  label pass and any per-cluster-row cost of a masked read are **not priced**.

## Findings

1. **Calibration** (table C1). S1 and S2 are calibrated in the pool on the pseudo-queries at the cluster level (δ 0.04 to
   0.21 on LAION and glove; 0.31 to 0.49 on yahoo) and transfer to real queries with δ 0.33 (LAION text), 0.05 (glove), 0.49
   (yahoo). S0 does not transfer between plain and shifted pseudo-queries, let alone to real ones. For sub-clusters the own rank-4 spread (S2) is poorly
   calibrated on LAION (δ 0.82 plain, 0.54 real). Residuals by rad and size terciles are within ±0.1 for S1 on
   LAION and glove pseudo-queries and reach −0.27 on LAION text. Shifted
   pseudo-queries are over-counted at k = 10 by e^1.0 on LAION and e^2.4 on yahoo: possibly because the shift used for
   calibration is not a pure shift on these corpora (the random direction is orthogonal to 64 directions only).
2. **The per-query factor is the problem, for every query type.** Σ n·P(τ_true)/k at k = 10 spreads over a factor
   of more than 100 (10% to 90%) even for held-out corpus rows on glove with S1. It is a property of the extreme tail
   (10 answers in a million rows from two or three features per cluster), not of out-of-distribution queries. At
   1% answers the same factor is within e^±0.2 on glove and e^−0.4 to e^+0.8 on LAION.
3. **Why τ\* frozen wins.** With τ\* from Σ n·P(τ\*) = k the values of a query sum to k whatever its factor: the
   price then acts on predicted shares of the answer, and the per-query miscalibration cancels. Re-solving τ̂ from
   found items moves τ towards the true k-th score, where the values carry the per-query factor again, and a
   global price over- and under-reads by it. That is my reading of the ordering true τ worst, epochs in between,
   τ\* best, which holds on all three datasets; it was not tested further. In `policy.md`'s terms: the
   certainty-equivalent update is right in the model and harmful outside it, and the price is in effect a
   predicted-recall target per query.
4. **Order and stopping separately.** At a fixed probe count the fitted order with S1 equals `spread`, and with
   the H r16 spread equals H r16, so the fitted g and β do replace f′, α, z and 0.3 where the spread statistic
   is the same. Unfiltered at 0.9, P1\* is 13 to 28% below T1 and P2\* 7 to 23% below T2: that is the price, effort varying
   by query. With the path fixed as for the tuned arms (P1i\*, P2i\*) the gain is the same unfiltered and 9 to 21
   points smaller under random 25%, where the direct alternative is doing part of the work.
5. **Sub-clusters.** E1 as a rule is sound under filters (it expands nothing under random 1%, where T4 and T5
   lose 20 to 120%). Unfiltered, with one plan, the process beats the tuned two-level arm on LAION and
   glove and equals it on yahoo (LAION at 0.9: P1si\* about 1,350 µs against T4 1,549). With the specified τ̂ update it does not find the
   gain on LAION.
6. **Visible sets.** Random sets: the process needs no fallback; P2\* is 0.77 to 0.86 of T1 at random 1% and 0.52 to
   0.60 at random 25%. The calibration under thinning holds on glove (δ 0.13 to 0.15), is looser on LAION (0.41
   to 0.47) and poor on yahoo (1.25 to 1.29, median factor e^−0.6 to e^−0.9). Correlated sets: δ 0.4 to 1.0 on
   LAION, up to 1.5 on yahoo; cost at recall still 0.57 to 0.92 of T1.
7. **Thresholds.** One plan at τ = t. The H r16 spread is needed on LAION at small answers; at 10% answers every
   arm is within 14% of T1 and the query is a scan.

## Not run

k = 100; expansion E2; visible sets of 5% and a contiguous range of ids; threshold answers of 0.1%; the recall
target and budget dials (so check 5 of 4.4 is missing); τ in hazard units (the count factor is reported instead);
residuals by group; lazy evaluation of S2 and its count of evaluated units; the LAION image-query control and the
1,000-query sets; the planner's `COSTS` as an alternative price list.

## What I would do next

1. Re-specify the k-NN dial as it behaves: τ\* once, then a price on shares (or a predicted-recall target), and
   test whether a τ update that keeps the normalisation (re-solving so that found plus predicted equals k but
   holding each unit's share) recovers what epochs were meant to give.
2. Time a masked index-path read and a direct read of whole clusters at these n, to settle b, the two row costs
   and the label pass; the choice of path and E1's threshold both hang on them.
3. Repeat on the 1,000-query sets and the image-query control: the price's gain is a between-query effect and 100
   queries is few for it.
4. A feature for the extreme tail (for instance the unit's best few scores against its own centre direction, or
   n) to narrow the per-query factor; the clairvoyant plan is 3 to 7 times cheaper than any arm.

## Tables

