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


### C1. Calibration checks (policy.md 4.4)

delta: the largest |ln(observed/predicted)| over bins of predicted P holding 95% of the answers. kappa: per query, the sum
of n P(tau_true) over the units, divided by the answer count.

| dataset | level, tier | beta | delta: plain / shifted / real | ln(obs/pred): plain / shifted / real | ln kappa at k = 10 (answer fraction 1e-5), median [10%, 90%]: plain; shifted; real | ln kappa at 1%: real |
|---|---|---|---|---|---|---|
| laion | cluster, S0 | +1.783 | 0.84 / 0.65 / 1.17 | -0.20 / +0.28 / +0.34 | +2.57 [-0.47, +5.24]; +1.64 [-2.75, +4.22]; -1.52 [-4.20, +1.03] | -0.80 [-1.56, -0.35] |
| laion | cluster, S1 | +0.040 | 0.21 / 0.15 / 0.33 | +0.06 / -0.05 / -0.16 | -0.50 [-3.05, +1.15]; +1.04 [-0.22, +1.98]; -0.35 [-2.50, +1.63] | +0.03 [-0.42, +0.77] |
| laion | cluster, S2 | -0.062 | 0.19 / 0.14 / 0.30 | +0.05 / -0.05 / -0.15 | +0.01 [-1.76, +1.28]; +1.12 [+0.09, +1.91]; +0.27 [-1.59, +1.73] | +0.06 [-0.23, +0.64] |
| laion | sub, S0 | +0.817 | 0.54 / 0.61 / 1.19 | -0.18 / +0.23 / +0.29 | +2.38 [-0.55, +5.04]; +1.60 [-2.62, +4.09]; -1.41 [-4.32, +1.10] | -0.59 [-1.25, -0.24] |
| laion | sub, S1 | +0.004 | 0.10 / 0.08 / 0.29 | +0.02 / -0.02 / -0.12 | -0.01 [-1.81, +1.30]; +0.76 [-0.48, +1.55]; -0.19 [-2.47, +1.58] | +0.02 [-0.28, +0.62] |
| laion | sub, S2 | -0.018 | 0.82 / 0.41 / 0.54 | +0.13 / -0.11 / -0.20 | -0.25 [-1.05, +0.48]; +1.92 [+0.94, +2.70]; +0.86 [-0.48, +2.37] | +0.16 [-0.16, +0.82] |
| glove | cluster, S0 | +1.825 | 0.72 / 1.37 / 0.70 | -0.42 / +0.84 / -0.41 | +4.91 [+1.99, +5.70]; +0.30 [-0.93, +1.38]; +4.83 [+2.04, +6.08] | +1.08 [+0.69, +1.50] |
| glove | cluster, S1 | +0.052 | 0.04 / 0.06 / 0.05 | -0.01 / +0.01 / -0.01 | +0.23 [-3.42, +1.48]; +0.21 [-0.23, +0.66]; +0.32 [-3.22, +1.57] | +0.05 [-0.07, +0.17] |
| glove | cluster, S2 | +0.067 | 0.06 / 0.07 / 0.05 | -0.01 / +0.01 / -0.01 | +0.47 [-2.10, +1.60]; +0.27 [-0.16, +0.73]; +0.52 [-2.34, +1.61] | +0.06 [-0.03, +0.15] |
| glove | sub, S0 | +1.528 | 0.74 / 1.25 / 0.71 | -0.41 / +0.80 / -0.40 | +4.73 [+1.83, +5.62]; -0.02 [-1.38, +1.16]; +4.66 [+1.83, +5.93] | +0.99 [+0.65, +1.41] |
| glove | sub, S1 | +0.034 | 0.08 / 0.11 / 0.08 | -0.03 / +0.04 / -0.03 | +0.54 [-1.50, +1.72]; +0.11 [-0.38, +0.56]; +0.55 [-1.73, +1.75] | +0.10 [-0.00, +0.25] |
| glove | sub, S2 | +0.010 | 0.01 / 0.03 / 0.01 | +0.00 / +0.00 / +0.00 | +0.37 [-0.85, +1.14]; +0.35 [-0.08, +0.76]; +0.33 [-1.02, +1.09] | +0.03 [-0.03, +0.09] |
| yahoo | cluster, S0 | +1.656 | 1.03 / 0.56 / 0.58 | -0.25 / +0.35 / -0.26 | -0.33 [-3.74, +2.54]; -0.47 [-3.31, +1.30]; +0.49 [-2.79, +2.44] | +0.26 [+0.00, +0.66] |
| yahoo | cluster, S1 | +0.006 | 0.49 / 0.37 / 0.49 | +0.15 / -0.12 / +0.15 | -0.75 [-2.46, +0.70]; +2.37 [+1.29, +3.02]; -0.54 [-2.07, +0.56] | -0.29 [-0.55, -0.18] |
| yahoo | cluster, S2 | +0.042 | 0.47 / 0.31 / 0.47 | +0.12 / -0.10 / +0.13 | -0.06 [-1.72, +0.85]; +2.14 [+1.14, +2.72]; -0.06 [-1.28, +0.65] | -0.21 [-0.38, -0.13] |
| yahoo | sub, S0 | +1.164 | 1.07 / 0.60 / 1.10 | -0.24 / +0.33 / -0.24 | +0.02 [-2.63, +2.20]; -0.77 [-3.03, +1.04]; +0.52 [-1.33, +2.33] | +0.21 [+0.02, +0.55] |
| yahoo | sub, S1 | -0.019 | 0.49 / 0.27 / 0.50 | +0.10 / -0.09 / +0.11 | -0.58 [-2.61, +0.76]; +1.90 [+0.58, +2.58]; -0.63 [-2.10, +0.57] | -0.18 [-0.36, -0.09] |
| yahoo | sub, S2 | +0.033 | 0.73 / 0.45 / 0.74 | +0.18 / -0.15 / +0.18 | -0.48 [-1.42, +0.17]; +2.41 [+1.38, +3.12]; -0.57 [-1.33, -0.06] | -0.35 [-0.53, -0.21] |

Real queries under visible sets: delta; ln kappa at k = 10, median [10%, 90%]

| dataset | level, tier | random-1 | random-25 | clus-1 | clus-25 | cap-1 | cap-25 |
|---|---|---|---|---|---|---|---|
| laion | cluster, S0 | 1.52; -1.68 [-2.72, -0.42] | 1.47; -1.87 [-3.64, +0.33] | 1.72; -1.49 [-3.02, -0.15] | 1.47; -1.30 [-3.83, +0.33] | 1.72; -0.92 [-2.26, +0.54] | 1.52; -1.14 [-3.11, +0.86] |
| laion | cluster, S1 | 0.47; -0.25 [-1.27, +0.98] | 0.41; -0.46 [-2.63, +1.56] | 0.89; -0.10 [-2.23, +1.63] | 0.41; -0.35 [-2.45, +1.61] | 0.97; +0.60 [-1.03, +1.91] | 0.35; -0.15 [-2.21, +2.11] |
| laion | cluster, S2 | 0.40; -0.07 [-0.65, +0.91] | 0.38; +0.20 [-1.69, +1.36] | 0.63; -0.28 [-1.63, +1.41] | 0.37; -0.04 [-1.65, +1.62] | 1.00; +0.85 [-0.41, +1.86] | 0.35; +0.26 [-1.23, +1.94] |
| laion | sub, S0 | 1.48; -1.04 [-2.38, -0.21] | 1.41; -1.42 [-3.44, +0.47] | 1.64; -1.49 [-2.85, -0.15] | 1.38; -1.28 [-3.39, +0.44] | 1.24; -0.70 [-1.90, +0.44] | 1.34; -0.93 [-3.10, +0.88] |
| laion | sub, S1 | 0.36; -0.03 [-0.91, +0.91] | 0.36; -0.10 [-2.15, +1.38] | 0.42; -0.28 [-2.09, +1.43] | 0.35; -0.37 [-2.34, +1.36] | 0.98; +0.46 [-1.02, +1.64] | 0.32; -0.14 [-1.65, +1.83] |
| laion | sub, S2 | 0.73; +0.25 [-0.43, +1.25] | 0.72; +0.61 [-0.64, +2.07] | 0.66; +0.52 [-0.81, +1.95] | 0.71; +0.69 [-0.56, +2.31] | 1.04; +1.00 [-0.18, +2.05] | 0.62; +0.87 [-0.26, +2.54] |
| glove | cluster, S0 | 2.44; +2.15 [+0.99, +3.01] | 2.00; +4.10 [+1.81, +5.21] | 2.35; +2.55 [+1.79, +3.49] | 2.04; +4.27 [+1.88, +5.36] | 2.41; +2.52 [+1.80, +3.16] | 2.25; +4.16 [+2.63, +5.09] |
| glove | cluster, S1 | 0.13; +0.20 [-0.58, +0.69] | 0.15; +0.36 [-1.88, +1.31] | 0.09; +0.20 [-0.96, +0.62] | 0.18; +0.43 [-2.84, +1.44] | 0.21; +0.38 [-0.68, +1.08] | 0.10; +0.48 [-1.09, +1.36] |
| glove | cluster, S2 | 0.16; +0.22 [-0.43, +0.63] | 0.15; +0.40 [-0.85, +1.26] | 0.18; +0.28 [-0.56, +0.70] | 0.17; +0.46 [-1.64, +1.34] | 0.23; +0.35 [-0.50, +1.02] | 0.07; +0.59 [-0.56, +1.38] |
| glove | sub, S0 | 2.02; +2.09 [+0.95, +2.91] | 1.93; +3.97 [+1.66, +5.15] | 2.53; +2.47 [+1.75, +3.32] | 1.96; +4.16 [+1.73, +5.28] | 2.60; +2.51 [+1.82, +3.06] | 1.89; +4.09 [+2.47, +5.09] |
| glove | sub, S1 | 0.30; +0.27 [-0.39, +0.79] | 0.26; +0.52 [-0.83, +1.48] | 0.23; +0.27 [-0.54, +0.75] | 0.26; +0.59 [-1.80, +1.57] | 0.22; +0.43 [-0.20, +1.15] | 0.15; +0.55 [-0.53, +1.44] |
| glove | sub, S2 | 0.13; +0.16 [-0.41, +0.55] | 0.11; +0.25 [-0.48, +0.98] | 0.15; +0.17 [-0.30, +0.59] | 0.12; +0.36 [-1.08, +0.89] | 0.14; +0.25 [-0.49, +0.92] | 0.05; +0.41 [-0.40, +1.12] |
| yahoo | cluster, S0 | 1.18; +0.08 [-0.72, +1.16] | 1.28; +0.38 [-2.52, +1.77] | 1.62; +1.62 [-0.26, +2.64] | 1.29; +0.50 [-2.22, +2.28] | 2.03; +2.35 [+0.69, +3.18] | 1.78; +0.81 [-2.13, +2.45] |
| yahoo | cluster, S1 | 1.25; -0.62 [-1.26, -0.04] | 1.29; -0.89 [-2.35, +0.15] | 1.52; -1.30 [-2.19, -0.36] | 1.27; -0.66 [-1.91, +0.18] | 0.79; +0.08 [-0.94, +1.31] | 0.87; -0.35 [-2.19, +0.82] |
| yahoo | cluster, S2 | 1.02; -0.45 [-0.85, +0.10] | 1.04; -0.40 [-1.21, +0.23] | 1.56; -1.29 [-2.09, -0.41] | 1.04; -0.47 [-1.54, +0.17] | 1.06; +0.12 [-0.83, +1.38] | 0.76; +0.09 [-1.42, +1.06] |
| yahoo | sub, S0 | 1.36; +0.12 [-0.40, +1.04] | 1.19; +0.49 [-1.26, +1.67] | 1.74; +1.37 [-0.26, +2.26] | 1.33; +0.47 [-1.45, +2.05] | 2.13; +2.01 [+0.44, +2.86] | 1.77; +0.69 [-0.90, +2.20] |
| yahoo | sub, S1 | 1.09; -0.34 [-0.80, +0.11] | 1.07; -0.55 [-1.48, +0.39] | 1.46; -1.44 [-2.40, -0.37] | 1.04; -0.70 [-1.76, +0.31] | 0.51; +0.02 [-0.93, +0.90] | 0.85; -0.10 [-1.47, +0.81] |
| yahoo | sub, S2 | 1.36; -0.58 [-1.07, -0.11] | 1.33; -0.78 [-1.21, -0.25] | 1.37; -1.17 [-1.86, -0.61] | 1.30; -0.80 [-1.38, -0.33] | 0.92; -0.35 [-1.29, +0.59] | 1.06; -0.43 [-1.39, +0.45] |


### P1. Modelled cost relative to T1 (spread, nprobe) at recall 0.9 / 0.95; `-`: recall not reached


**laion** (T1's modelled us at 0.9: knn 2,402, knn random-1 112, knn random-25 961, knn clus-1 388, knn clus-25 1,842, knn cap-1 386, knn cap-25 1,893, thr 0 3,855, thr 1 12,773, thr 2 27,811)

| arm | knn | knn random-1 | knn random-25 | knn clus-1 | knn clus-25 | knn cap-1 | knn cap-25 | thr 0 | thr 1 | thr 2 |
|---|---|---|---|---|---|---|---|---|---|---|
| T2 | 0.83 / 0.72 | 1.00 / 1.00 | 0.92 / 0.86 | 1.00 / 0.92 | 0.77 / 0.71 | 1.00 / 1.00 | 1.17 / 1.16 |  |  |  |
| T3 | 1.05 / 1.02 | 1.00 / 0.99 | 1.02 / 0.91 | 1.00 / 1.00 | 1.01 / 0.96 | 1.04 / 1.01 | 1.01 / 0.97 | 1.02 / 1.00 | 1.00 / 1.00 | 1.00 / 1.00 |
| T4 | 0.65 / 0.66 | 1.20 / - | 0.97 / 0.86 | 0.70 / 0.75 | 0.73 / 0.67 | 0.85 / - | 0.70 / 0.93 | 0.59 / 0.73 | - / - | - / - |
| T5 | 0.68 / 0.66 | 1.72 / - | 1.03 / 0.85 | 0.70 / 0.75 | 0.71 / 0.65 | 0.98 / - | 0.70 / 1.06 | 0.61 / 0.81 | - / - | - / - |
| O0 | 1.30 / 1.25 | 1.04 / 1.04 | 1.16 / 1.13 | 1.08 / 1.03 | 1.24 / 1.11 | 1.08 / 1.06 | 1.18 / 1.10 | 1.12 / 1.11 | 1.01 / 1.01 | 0.97 / 0.98 |
| O1 | 1.01 / 0.86 | 0.98 / 0.90 | 1.02 / 0.77 | 0.98 / 0.95 | 0.87 / 0.81 | 1.04 / 1.05 | 0.89 / 0.92 | 0.92 / 0.88 | 0.96 / 0.95 | 0.96 / 0.96 |
| O2 | 0.87 / 0.78 | 0.93 / 0.83 | 0.96 / 0.83 | 0.92 / 0.89 | 0.73 / 0.68 | 0.96 / 0.98 | 0.87 / 0.89 | 0.76 / 0.77 | 0.89 / 0.90 | 0.94 / 0.95 |
| P0 | 2.48 / 2.46 | 1.12 / 1.18 | 1.91 / 1.52 | 0.79 / 0.75 | 1.80 / 1.54 | 1.07 / 0.98 | 1.82 / 1.61 | 2.25 / 2.11 | 1.13 / 1.12 | 1.01 / 1.00 |
| P1 | 1.66 / 1.51 | 1.05 / 1.14 | 1.46 / 1.11 | 0.81 / 0.77 | 1.15 / 1.13 | 1.01 / 1.00 | 1.28 / 1.37 | 1.86 / 1.52 | 1.09 / 1.05 | 1.01 / 0.99 |
| P2 | 1.04 / 1.10 | 0.89 / 0.84 | 0.76 / 0.84 | 0.74 / 0.70 | 0.84 / 0.74 | 0.90 / 0.91 | 0.82 / 0.99 | 1.06 / 1.01 | 0.93 / 0.92 | 0.98 / 0.96 |
| P0i | 2.49 / 2.58 | 1.12 / 1.18 | 2.17 / 1.61 | 0.81 / 0.78 | 1.83 / 1.54 | 1.12 / 1.05 | 1.97 / 1.87 | 2.25 / 2.13 | 1.13 / 1.12 | 1.01 / 1.01 |
| P1i | 1.62 / 1.52 | 1.05 / 1.14 | 1.66 / 1.43 | 0.83 / 0.79 | 1.19 / 1.15 | 1.05 / 1.05 | 1.34 / 1.44 | 1.89 / 1.53 | 1.09 / 1.05 | 1.01 / 0.99 |
| P2i | 1.10 / 1.11 | 0.89 / 0.84 | 0.88 / 0.98 | 0.76 / 0.72 | 0.85 / 0.76 | 0.95 / 0.98 | 0.93 / 1.08 | 1.06 / 1.01 | 0.94 / 0.92 | 0.99 / 0.97 |
| P0s | 1.85 / 1.44 | 1.12 / 1.18 | 1.91 / 1.63 | 0.74 / 0.71 | 1.56 / 1.23 | 1.07 / 0.98 | 1.65 / 1.58 | 1.84 / 1.63 | 0.98 / 0.98 | 0.94 / 0.96 |
| P1s | 1.14 / 0.95 | 1.05 / 1.14 | 1.42 / 1.08 | 0.70 / 0.69 | 0.96 / 0.89 | 1.01 / 1.00 | 1.14 / 1.19 | 1.41 / 1.20 | 0.94 / 0.89 | 0.93 / 0.93 |
| P2s | 1.02 / 1.03 | 0.89 / 0.84 | 0.76 / 0.84 | 0.70 / 0.68 | 0.82 / 0.72 | 0.90 / 0.91 | 0.80 / 0.97 | 1.04 / 0.97 | 0.87 / 0.88 | 0.93 / 0.93 |
| P1si | 1.05 / 0.89 | 1.05 / 1.14 | 1.38 / 1.17 | 0.70 / 0.67 | 0.91 / 0.82 | 0.96 / 0.98 | 1.13 / 1.18 | 1.39 / 1.19 | 0.93 / 0.89 | 0.92 / 0.92 |
| P2si | 1.03 / 1.04 | 0.89 / 0.84 | 0.87 / 0.98 | 0.71 / 0.69 | 0.80 / 0.73 | 0.88 / 0.90 | 0.89 / 1.04 | 1.03 / 0.97 | 0.87 / 0.87 | 0.93 / 0.93 |
| P1 ag0 | 1.67 / 1.56 |  |  |  |  |  |  |  |  |  |
| P1 tau | 2.87 / 2.06 |  |  |  |  |  |  |  |  |  |
| P2 tau | 1.85 / 1.40 |  |  |  |  |  |  |  |  |  |
| P0* | 1.26 / 0.98 | 0.98 / 1.00 | 0.89 / 0.86 | 0.82 / 0.79 | 1.10 / 0.98 | 0.97 / 0.96 | 1.01 / 1.06 |  |  |  |
| P1* | 0.79 / 0.74 | 0.88 / 0.85 | 0.71 / 0.63 | 0.77 / 0.78 | 0.78 / 0.64 | 0.86 / 0.91 | 0.75 / 0.78 |  |  |  |
| P2* | 0.77 / 0.67 | 0.82 / 0.73 | 0.60 / 0.57 | 0.68 / 0.65 | 0.64 / 0.57 | 0.92 / 0.95 | 0.62 / 0.71 |  |  |  |
| P1i* | 0.79 / 0.74 | 0.88 / 0.85 | 0.84 / 0.68 | 0.80 / 0.80 | 0.79 / 0.65 | 0.90 / 0.95 | 0.79 / 0.83 |  |  |  |
| P2i* | 0.75 / 0.67 | 0.82 / 0.73 | 0.69 / 0.68 | 0.70 / 0.67 | 0.65 / 0.58 | 0.96 / 1.01 | 0.66 / 0.74 |  |  |  |
| P1s* | 0.60 / 0.51 | 0.88 / 0.85 | 0.69 / 0.61 | 0.66 / 0.68 | 0.63 / 0.51 | 0.86 / 0.91 | 0.62 / 0.64 |  |  |  |
| P2s* | 0.69 / 0.61 | 0.82 / 0.73 | 0.60 / 0.58 | 0.64 / 0.62 | 0.61 / 0.55 | 0.92 / 0.95 | 0.58 / 0.68 |  |  |  |
| P1si* | 0.56 / 0.47 | 0.88 / 0.85 | 0.77 / 0.63 | 0.67 / 0.68 | 0.56 / 0.46 | 0.84 / 0.92 | 0.60 / 0.65 |  |  |  |
| P2si* | 0.66 / 0.60 | 0.82 / 0.73 | 0.65 / 0.67 | 0.64 / 0.63 | 0.60 / 0.54 | 0.87 / 0.97 | 0.61 / 0.69 |  |  |  |
| clair | 0.20 / 0.12 |  |  |  |  |  |  |  |  |  |
| clair i | 0.21 / 0.12 |  |  |  |  |  |  |  |  |  |

**glove** (T1's modelled us at 0.9: knn 2,276, knn random-1 108, knn random-25 977, knn clus-1 152, knn clus-25 1,077, knn cap-1 137, knn cap-25 1,163, thr 0 3,418, thr 1 11,787, thr 2 20,157)

| arm | knn | knn random-1 | knn random-25 | knn clus-1 | knn clus-25 | knn cap-1 | knn cap-25 | thr 0 | thr 1 | thr 2 |
|---|---|---|---|---|---|---|---|---|---|---|
| T2 | 0.86 / 0.89 | 1.00 / 1.00 | 1.00 / 0.92 | 0.98 / 0.96 | 0.91 / 0.93 | 1.00 / 1.00 | 1.12 / 1.08 |  |  |  |
| T3 | 1.04 / 1.08 | 0.98 / 1.03 | 1.05 / 1.03 | 1.01 / 0.99 | 0.98 / 0.92 | 1.06 / 0.99 | 0.97 / 1.01 | 1.07 / 0.99 | 1.01 / 1.00 | 1.00 / 1.00 |
| T4 | 0.60 / - | - / - | 0.91 / - | 0.81 / 0.85 | 0.63 / 0.62 | - / - | 0.83 / - | 0.79 / - | - / - | - / - |
| T5 | 0.60 / 0.89 | - / - | 0.93 / - | 0.81 / 0.85 | 0.65 / 0.68 | - / - | 0.82 / - | 0.81 / - | - / - | - / - |
| O0 | 1.14 / 1.10 | 0.94 / 0.95 | 1.08 / 1.08 | 0.96 / 0.96 | 1.09 / 1.02 | 1.12 / 1.03 | 1.08 / 1.03 | 1.01 / 0.97 | 0.91 / 0.94 | 0.93 / 0.95 |
| O1 | 0.98 / 0.96 | 0.90 / 0.94 | 0.94 / 0.96 | 0.98 / 1.01 | 1.00 / 1.01 | 1.00 / 1.05 | 0.89 / 0.98 | 0.91 / 0.90 | 0.90 / 0.93 | 0.93 / 0.95 |
| O2 | 0.86 / 0.91 | 0.88 / 0.89 | 0.89 / 0.89 | 0.98 / 0.96 | 0.86 / 0.92 | 0.95 / 1.00 | 0.97 / 0.92 | 0.89 / 0.90 | 0.89 / 0.91 | 0.93 / 0.95 |
| P0 | 4.81 / 4.63 | 2.25 / 2.07 | 4.91 / 3.31 | 1.91 / 1.58 | 4.54 / 2.83 | 2.25 / 1.76 | 4.22 / 2.62 | 2.21 / 1.93 | 1.10 / 1.06 | 1.07 / 1.08 |
| P1 | 1.40 / 1.45 | 0.77 / 0.73 | 0.68 / 0.69 | 0.81 / 0.81 | 0.82 / 0.69 | 0.94 / 0.85 | 0.71 / 0.68 | 0.97 / 0.87 | 0.85 / 0.86 | 0.94 / 0.95 |
| P2 | 1.00 / 1.00 | 0.73 / 0.71 | 0.60 / 0.55 | 0.82 / 0.76 | 0.68 / 0.61 | 0.84 / 0.79 | 0.67 / 0.62 | 0.79 / 0.70 | 0.83 / 0.84 | 0.94 / 0.95 |
| P0i | 5.06 / 4.70 | 2.25 / 2.07 | 3.02 / 1.93 | 2.03 / 1.67 | 4.58 / 2.85 | 2.25 / 1.76 | 3.31 / 2.00 | 2.22 / 1.93 | 1.11 / 1.07 | 1.08 / 1.09 |
| P1i | 1.42 / 1.44 | 0.77 / 0.73 | 0.84 / 0.79 | 0.87 / 0.86 | 0.81 / 0.70 | 0.94 / 0.85 | 0.76 / 0.79 | 0.97 / 0.89 | 0.85 / 0.86 | 0.94 / 0.95 |
| P2i | 1.02 / 1.03 | 0.73 / 0.71 | 0.71 / 0.61 | 0.86 / 0.79 | 0.68 / 0.60 | 0.84 / 0.79 | 0.73 / 0.69 | 0.80 / 0.71 | 0.84 / 0.84 | 0.94 / 0.95 |
| P0s | 5.64 / 4.75 | 2.25 / 2.07 | 4.91 / 3.31 | 1.90 / 1.57 | 3.84 / 2.39 | 2.25 / 1.76 | 4.16 / 2.59 | 1.94 / 1.72 | 1.03 / 0.97 | 1.05 / 1.04 |
| P1s | 1.06 / 0.91 | 0.77 / 0.73 | 0.68 / 0.69 | 0.81 / 0.81 | 0.69 / 0.60 | 0.94 / 0.85 | 0.70 / 0.68 | 0.81 / 0.73 | 0.76 / 0.78 | 0.88 / 0.91 |
| P2s | 0.70 / 0.67 | 0.73 / 0.71 | 0.60 / 0.55 | 0.80 / 0.77 | 0.53 / 0.52 | 0.84 / 0.79 | 0.68 / 0.61 | 0.59 / 0.55 | 0.73 / 0.75 | 0.87 / 0.91 |
| P1si | 0.98 / 0.76 | 0.77 / 0.73 | 0.75 / 0.71 | 0.79 / 0.78 | 0.63 / 0.55 | 0.94 / 0.85 | 0.69 / 0.70 | 0.78 / 0.72 | 0.75 / 0.77 | 0.87 / 0.91 |
| P2si | 0.60 / 0.54 | 0.73 / 0.71 | 0.60 / 0.57 | 0.72 / 0.72 | 0.48 / 0.48 | 0.84 / 0.79 | 0.61 / 0.56 | 0.55 / 0.53 | 0.71 / 0.74 | 0.86 / 0.90 |
| P1 ag0 | 1.42 / 1.46 |  |  |  |  |  |  |  |  |  |
| P1 tau | 2.54 / 2.39 |  |  |  |  |  |  |  |  |  |
| P2 tau | 1.74 / 1.57 |  |  |  |  |  |  |  |  |  |
| P0* | 1.99 / 1.55 | 0.96 / 1.10 | 1.08 / 0.97 | 0.84 / 0.96 | 1.21 / 0.96 | 1.25 / 1.14 | 1.23 / 0.91 |  |  |  |
| P1* | 0.72 / 0.61 | 0.81 / 0.76 | 0.58 / 0.54 | 0.84 / 0.82 | 0.69 / 0.66 | 0.91 / 0.81 | 0.65 / 0.60 |  |  |  |
| P2* | 0.66 / 0.61 | 0.77 / 0.73 | 0.53 / 0.50 | 0.80 / 0.79 | 0.63 / 0.63 | 0.82 / 0.82 | 0.65 / 0.65 |  |  |  |
| P1i* | 0.72 / 0.61 | 0.81 / 0.76 | 0.68 / 0.63 | 0.89 / 0.87 | 0.71 / 0.67 | 0.91 / 0.81 | 0.72 / 0.66 |  |  |  |
| P2i* | 0.67 / 0.61 | 0.77 / 0.73 | 0.63 / 0.57 | 0.85 / 0.84 | 0.64 / 0.63 | 0.82 / 0.82 | 0.72 / 0.69 |  |  |  |
| P1s* | 0.55 / 0.48 | 0.81 / 0.76 | 0.58 / 0.54 | 0.82 / 0.79 | 0.57 / 0.55 | 0.91 / 0.81 | 0.64 / 0.59 |  |  |  |
| P2s* | 0.52 / 0.62 | 0.77 / 0.73 | 0.53 / 0.50 | 0.79 / 0.79 | 0.49 / 0.49 | 0.82 / 0.82 | 0.64 / 0.64 |  |  |  |
| P1si* | 0.51 / 0.45 | 0.81 / 0.76 | 0.59 / 0.55 | 0.78 / 0.76 | 0.52 / 0.49 | 0.91 / 0.81 | 0.64 / 0.57 |  |  |  |
| P2si* | 0.44 / 0.48 | 0.77 / 0.73 | 0.51 / 0.52 | 0.70 / 0.72 | 0.41 / 0.41 | 0.82 / 0.82 | 0.57 / 0.69 |  |  |  |
| clair | 0.14 / 0.08 |  |  |  |  |  |  |  |  |  |
| clair i | 0.14 / 0.08 |  |  |  |  |  |  |  |  |  |

**yahoo** (T1's modelled us at 0.9: knn 800, knn random-1 48, knn random-25 382, knn clus-1 294, knn clus-25 611, knn cap-1 237, knn cap-25 389, thr 0 1,108, thr 1 4,834, thr 2 14,686)

| arm | knn | knn random-1 | knn random-25 | knn clus-1 | knn clus-25 | knn cap-1 | knn cap-25 | thr 0 | thr 1 | thr 2 |
|---|---|---|---|---|---|---|---|---|---|---|
| T2 | 0.75 / 0.78 | 1.00 / 1.00 | 0.79 / 0.80 | 0.92 / - | 0.83 / 0.80 | 1.00 / 1.03 | 0.97 / 0.95 |  |  |  |
| T3 | 0.97 / 1.06 | 1.00 / 1.02 | 0.96 / 0.95 | 1.00 / - | 0.98 / 0.98 | 1.01 / 1.01 | 1.00 / 1.05 | 1.00 / 0.97 | 0.99 / 0.99 | 1.00 / 1.00 |
| T4 | 0.46 / 0.58 | 1.79 / 1.42 | 0.81 / 0.82 | 0.68 / - | 0.64 / 0.60 | 0.86 / 0.91 | 0.78 / 0.77 | 0.51 / 0.56 | 0.77 / - | - / - |
| T5 | 0.49 / 0.60 | 2.22 / 1.87 | 0.87 / 0.83 | 0.68 / - | 0.60 / 0.59 | 1.01 / - | 0.85 / 0.83 | 0.53 / 0.58 | 0.93 / - | - / - |
| O0 | 0.94 / 0.97 | 0.99 / 0.92 | 0.86 / 0.96 | 0.92 / - | 0.93 / 0.89 | 1.02 / 1.01 | 0.98 / 0.93 | 0.95 / 0.91 | 0.92 / 0.92 | 0.93 / 0.94 |
| O1 | 0.92 / 1.03 | 0.95 / 0.94 | 0.85 / 0.94 | 0.83 / - | 1.02 / 0.94 | 1.01 / 1.01 | 0.86 / 1.01 | 0.93 / 0.99 | 0.90 / 0.89 | 0.91 / 0.92 |
| O2 | 0.83 / 0.93 | 0.94 / 0.87 | 0.87 / 0.83 | 0.74 / - | 0.92 / 0.86 | 0.99 / 0.98 | 0.87 / 0.88 | 0.89 / 0.87 | 0.84 / 0.82 | 0.89 / 0.90 |
| P0 | 1.24 / 1.76 | 0.96 / 0.97 | 0.82 / 1.64 | 0.83 / - | 1.00 / 1.47 | 1.01 / 0.95 | 1.05 / 1.89 | 2.35 / 2.32 | 1.01 / 0.98 | 1.01 / 1.05 |
| P1 | 0.93 / 1.04 | 0.91 / 0.80 | 0.62 / 0.62 | 0.63 / - | 0.72 / 0.70 | 0.80 / 0.85 | 0.79 / 0.73 | 0.98 / 0.90 | 0.81 / 0.81 | 0.90 / 0.91 |
| P2 | 0.79 / 0.96 | 0.86 / 0.76 | 0.54 / 0.51 | 0.58 / - | 0.61 / 0.58 | 0.77 / 0.82 | 0.68 / 0.69 | 0.77 / 0.76 | 0.76 / 0.72 | 0.88 / 0.89 |
| P0i | 1.28 / 1.86 | 0.96 / 0.97 | 1.18 / 1.77 | 0.92 / - | 1.13 / 1.55 | 1.03 / 0.96 | 1.27 / 2.12 | 2.54 / 2.50 | 1.04 / 1.00 | 1.03 / 1.05 |
| P1i | 0.99 / 1.16 | 0.91 / 0.80 | 0.88 / 0.93 | 0.68 / - | 0.78 / 0.75 | 0.83 / 0.88 | 0.96 / 0.89 | 1.03 / 0.93 | 0.83 / 0.82 | 0.90 / 0.92 |
| P2i | 0.82 / 0.97 | 0.86 / 0.76 | 0.67 / 0.65 | 0.62 / - | 0.68 / 0.62 | 0.80 / 0.84 | 0.76 / 0.75 | 0.78 / 0.77 | 0.78 / 0.74 | 0.89 / 0.90 |
| P0s | 1.06 / 1.54 | 0.96 / 0.97 | 0.82 / 1.64 | 0.83 / - | 0.93 / 1.38 | 1.01 / 0.95 | 1.03 / 1.85 | 1.90 / 1.92 | 0.90 / 0.88 | 0.92 / 1.01 |
| P1s | 0.74 / 0.83 | 0.91 / 0.80 | 0.62 / 0.62 | 0.63 / - | 0.68 / 0.64 | 0.80 / 0.85 | 0.76 / 0.71 | 0.79 / 0.74 | 0.74 / 0.72 | 0.84 / 0.84 |
| P2s | 0.68 / 0.84 | 0.86 / 0.76 | 0.54 / 0.51 | 0.58 / - | 0.62 / 0.59 | 0.77 / 0.82 | 0.64 / 0.68 | 0.66 / 0.63 | 0.68 / 0.65 | 0.82 / 0.82 |
| P1si | 0.69 / 0.73 | 0.91 / 0.80 | 0.79 / 0.83 | 0.62 / - | 0.63 / 0.60 | 0.75 / 0.82 | 0.84 / 0.78 | 0.76 / 0.70 | 0.72 / 0.70 | 0.83 / 0.83 |
| P2si | 0.48 / 0.61 | 0.86 / 0.76 | 0.58 / 0.62 | 0.55 / - | 0.47 / 0.48 | 0.70 / 0.76 | 0.66 / 0.70 | 0.49 / 0.48 | 0.62 / 0.60 | 0.80 / 0.81 |
| P1 ag0 | 0.95 / 1.08 |  |  |  |  |  |  |  |  |  |
| P1 tau | 1.42 / 1.40 |  |  |  |  |  |  |  |  |  |
| P2 tau | 0.93 / 1.01 |  |  |  |  |  |  |  |  |  |
| P0* | 0.84 / 0.81 | 0.92 / 0.89 | 0.60 / 0.62 | 0.75 / - | 0.71 / 0.67 | 0.87 / 0.86 | 0.65 / 0.66 |  |  |  |
| P1* | 0.87 / 0.86 | 0.88 / 0.80 | 0.61 / 0.63 | 0.67 / - | 0.72 / 0.69 | 0.84 / 0.88 | 0.70 / 0.68 |  |  |  |
| P2* | 0.70 / 0.80 | 0.86 / 0.80 | 0.52 / 0.51 | 0.57 / - | 0.64 / 0.57 | 0.81 / 0.83 | 0.63 / 0.60 |  |  |  |
| P1i* | 0.90 / 0.92 | 0.88 / 0.80 | 0.82 / 0.81 | 0.71 / - | 0.76 / 0.72 | 0.86 / 0.89 | 0.77 / 0.75 |  |  |  |
| P2i* | 0.73 / 0.84 | 0.86 / 0.80 | 0.70 / 0.69 | 0.64 / - | 0.67 / 0.60 | 0.83 / 0.85 | 0.72 / 0.66 |  |  |  |
| P1s* | 0.71 / 0.69 | 0.88 / 0.80 | 0.61 / 0.63 | 0.67 / - | 0.67 / 0.65 | 0.84 / 0.88 | 0.66 / 0.66 |  |  |  |
| P2s* | 0.65 / 0.72 | 0.86 / 0.80 | 0.52 / 0.51 | 0.57 / - | 0.62 / 0.56 | 0.81 / 0.83 | 0.59 / 0.58 |  |  |  |
| P1si* | 0.64 / 0.66 | 0.88 / 0.80 | 0.72 / 0.74 | 0.62 / - | 0.62 / 0.55 | 0.75 / 0.81 | 0.67 / 0.65 |  |  |  |
| P2si* | 0.44 / 0.52 | 0.86 / 0.80 | 0.57 / 0.65 | 0.57 / - | 0.53 / 0.50 | 0.73 / 0.75 | 0.62 / 0.56 |  |  |  |
| clair | 0.34 / 0.23 |  |  |  |  |  |  |  |  |  |
| clair i | 0.36 / 0.25 |  |  |  |  |  |  |  |  |  |


### P2. Unfiltered k-NN, every arm (modelled us, rows, clusters, groups, centres)

**laion**

| arm | modelled us at recall 0.8 / 0.9 / 0.95 | vs T1 at 0.9 / 0.95 | rows at 0.9 / 0.95 | clusters at 0.9 | groups at 0.9 | centres at 0.9 | epochs | max recall |
|---|---|---|---|---|---|---|---|---|
| T1 | 1,236 / 2,402 / 4,988 | 1.00 / 1.00 | 36,599 / 77,588 | 17.1 | 7.9 | 500 |  | 1.000 |
| T2 | 998 / 2,002 / 3,596 | 0.83 / 0.72 | 30,367 / 55,599 | 13.8 | 6.7 | 500 |  | 1.000 |
| T3 | 1,249 / 2,528 / 5,087 | 1.05 / 1.02 | 39,260 / 80,416 | 18.3 | 6.7 | 500 |  | 1.000 |
| T4 | 618 / 1,549 / 3,274 | 0.65 / 0.66 | 17,497 / 43,093 | 30.4 | 12.0 | 1,524 |  | 0.974 |
| T5 | 714 / 1,637 / 3,269 | 0.68 / 0.66 | 17,960 / 43,727 | 31.4 | 10.6 | 2,883 |  | 0.974 |
| O0 | 1,349 / 3,125 / 6,210 | 1.30 / 1.25 | 48,318 / 97,715 | 21.7 | 9.2 | 500 |  | 1.000 |
| O1 | 1,088 / 2,433 / 4,311 | 1.01 / 0.86 | 37,029 / 66,662 | 18.8 | 7.8 | 500 |  | 1.000 |
| O2 | 936 / 2,085 / 3,879 | 0.87 / 0.78 | 31,271 / 59,316 | 15.4 | 7.6 | 500 |  | 1.000 |
| P0 | 2,508 / 5,949 / 12,292 | 2.48 / 2.46 | 95,734 / 199,629 | 44.3 | 8.5 | 500 | 2.8 | 0.990 |
| P1 | 1,589 / 3,985 / 7,556 | 1.66 / 1.51 | 63,129 / 120,988 | 33.1 | 6.2 | 500 | 3.2 | 0.994 |
| P2 | 947 / 2,491 / 5,480 | 1.04 / 1.10 | 38,575 / 86,600 | 19.7 | 4.4 | 500 | 2.7 | 0.998 |
| P0i | 2,420 / 5,983 / 12,888 | 2.49 / 2.58 | 96,124 / 208,830 | 44.3 | 10.8 | 500 | 2.8 | 0.991 |
| P1i | 1,596 / 3,887 / 7,596 | 1.62 / 1.52 | 61,370 / 121,139 | 32.0 | 8.4 | 500 | 3.2 | 0.994 |
| P2i | 969 / 2,639 / 5,540 | 1.10 / 1.11 | 40,685 / 87,065 | 20.7 | 7.5 | 500 | 2.7 | 0.998 |
| P0s | 1,948 / 4,451 / 7,193 | 1.85 / 1.44 | 65,517 / 100,920 | 56.6 | 10.4 | 1,348 | 2.5 | 0.979 |
| P1s | 1,108 / 2,734 / 4,742 | 1.14 / 0.95 | 39,970 / 66,778 | 33.4 | 6.1 | 971 | 2.8 | 0.981 |
| P2s | 815 / 2,439 / 5,151 | 1.02 / 1.03 | 36,756 / 79,262 | 21.1 | 4.6 | 774 | 3.2 | 0.996 |
| P1si | 1,026 / 2,520 / 4,415 | 1.05 / 0.89 | 36,082 / 59,963 | 32.2 | 8.4 | 1,015 | 2.7 | 0.982 |
| P2si | 809 / 2,472 / 5,198 | 1.03 / 1.04 | 36,791 / 79,150 | 21.4 | 7.6 | 842 | 3.2 | 0.996 |
| P1 ag0 | 1,593 / 4,016 / 7,772 | 1.67 / 1.56 | 62,948 / 123,342 | 33.1 | 9.7 | 500 | 3.2 | 0.995 |
| P1 tau | 3,887 / 6,889 / 10,269 | 2.87 / 2.06 | 110,219 / 165,427 | 58.5 | 9.8 | 500 | 0.6 | 0.991 |
| P2 tau | 1,765 / 4,434 / 6,979 | 1.85 / 1.40 | 69,984 / 110,851 | 36.6 | 7.3 | 500 | 0.6 | 0.996 |
| P0* | 1,416 / 3,035 / 4,877 | 1.26 / 0.98 | 47,726 / 77,485 | 21.4 | 5.7 | 500 | 0.5 | 0.998 |
| P1* | 1,071 / 1,905 / 3,675 | 0.79 / 0.74 | 29,402 / 57,876 | 14.7 | 3.6 | 500 | 0.6 | 0.998 |
| P2* | 802 / 1,857 / 3,347 | 0.77 / 0.67 | 28,423 / 52,172 | 14.1 | 3.8 | 500 | 0.6 | 0.996 |
| P1i* | 1,080 / 1,888 / 3,687 | 0.79 / 0.74 | 28,902 / 57,730 | 14.4 | 5.5 | 500 | 0.5 | 0.998 |
| P2i* | 816 / 1,811 / 3,343 | 0.75 / 0.67 | 27,403 / 51,633 | 13.5 | 5.9 | 500 | 0.6 | 0.996 |
| P1s* | 781 / 1,440 / 2,541 | 0.60 / 0.51 | 20,162 / 35,750 | 17.5 | 4.2 | 741 | 0.6 | 0.990 |
| P2s* | 653 / 1,655 / 3,040 | 0.69 / 0.61 | 24,379 / 45,861 | 14.3 | 3.8 | 686 | 0.6 | 0.996 |
| P1si* | 726 / 1,341 / 2,366 | 0.56 / 0.47 | 18,094 / 32,099 | 17.3 | 6.2 | 778 | 0.6 | 0.990 |
| P2si* | 631 / 1,577 / 2,987 | 0.66 / 0.60 | 22,702 / 44,304 | 13.7 | 6.0 | 720 | 0.6 | 0.996 |
| clair | 384 / 490 / 582 | 0.20 / 0.12 | 6,833 / 8,251 | 3.3 | 1.0 | 500 | 0.4 | 1.000 |
| clair i | 391 / 504 / 594 | 0.21 / 0.12 | 6,906 / 8,288 | 3.3 | 2.3 | 500 | 0.3 | 1.000 |

**glove**

| arm | modelled us at recall 0.8 / 0.9 / 0.95 | vs T1 at 0.9 / 0.95 | rows at 0.9 / 0.95 | clusters at 0.9 | groups at 0.9 | centres at 0.9 | epochs | max recall |
|---|---|---|---|---|---|---|---|---|
| T1 | 833 / 2,276 / 4,385 | 1.00 / 1.00 | 75,830 / 147,193 | 36.5 | 13.8 | 592 |  | 1.000 |
| T2 | 799 / 1,949 / 3,911 | 0.86 / 0.89 | 65,454 / 132,368 | 27.8 | 10.9 | 592 |  | 1.000 |
| T3 | 958 / 2,366 / 4,746 | 1.04 / 1.08 | 79,404 / 160,198 | 38.5 | 10.5 | 592 |  | 1.000 |
| T4 | 516 / 1,364 / - | 0.60 / - | 37,950 / - | 49.3 | 17.4 | 1,616 |  | 0.937 |
| T5 | 583 / 1,359 / 3,919 | 0.60 / 0.89 | 35,700 / 120,833 | 49.7 | 12.1 | 3,275 |  | 0.954 |
| O0 | 1,078 / 2,605 / 4,828 | 1.14 / 1.10 | 88,078 / 164,232 | 36.2 | 14.0 | 592 |  | 1.000 |
| O1 | 871 / 2,229 / 4,196 | 0.98 / 0.96 | 74,763 / 141,572 | 33.5 | 12.5 | 592 |  | 1.000 |
| O2 | 798 / 1,958 / 3,987 | 0.86 / 0.91 | 65,390 / 134,242 | 29.5 | 12.0 | 592 |  | 1.000 |
| P0 | 3,051 / 10,951 / 20,304 | 4.81 / 4.63 | 374,934 / 695,783 | 182.1 | 19.1 | 592 | 2.2 | 0.998 |
| P1 | 1,209 / 3,194 / 6,351 | 1.40 / 1.45 | 108,524 / 216,901 | 49.6 | 8.5 | 592 | 3.5 | 0.975 |
| P2 | 1,003 / 2,284 / 4,381 | 1.00 / 1.00 | 77,297 / 149,232 | 34.6 | 6.5 | 592 | 3.6 | 0.986 |
| P0i | 3,083 / 11,505 / 20,624 | 5.06 / 4.70 | 392,870 / 704,974 | 190.9 | 26.7 | 592 | 2.2 | 0.998 |
| P1i | 1,214 / 3,232 / 6,323 | 1.42 / 1.44 | 109,318 / 215,024 | 49.8 | 13.5 | 592 | 3.4 | 0.975 |
| P2i | 1,008 / 2,312 / 4,505 | 1.02 / 1.03 | 77,885 / 152,732 | 34.8 | 10.8 | 592 | 3.5 | 0.986 |
| P0s | 2,319 / 12,826 / 20,840 | 5.64 / 4.75 | 429,150 / 699,150 | 224.3 | 22.1 | 3,122 | 2.4 | 0.998 |
| P1s | 948 / 2,422 / 3,994 | 1.06 / 0.91 | 77,659 / 122,799 | 51.5 | 8.7 | 1,186 | 3.3 | 0.975 |
| P2s | 705 / 1,594 / 2,956 | 0.70 / 0.67 | 49,491 / 88,790 | 39.0 | 7.2 | 1,050 | 3.4 | 0.982 |
| P1si | 885 / 2,219 / 3,322 | 0.98 / 0.76 | 68,786 / 96,237 | 51.4 | 13.8 | 1,415 | 3.2 | 0.977 |
| P2si | 597 / 1,361 / 2,370 | 0.60 / 0.54 | 40,074 / 65,138 | 37.9 | 11.5 | 1,199 | 3.2 | 0.979 |
| P1 ag0 | 1,207 / 3,224 / 6,397 | 1.42 / 1.46 | 109,337 / 218,119 | 50.0 | 12.8 | 592 | 3.5 | 0.975 |
| P1 tau | 2,257 / 5,770 / 10,481 | 2.54 / 2.39 | 196,998 / 358,705 | 92.9 | 13.7 | 592 | 0.6 | 0.965 |
| P2 tau | 1,434 / 3,956 / 6,869 | 1.74 / 1.57 | 134,742 / 234,691 | 62.3 | 10.2 | 592 | 0.7 | 0.982 |
| P0* | 2,516 / 4,538 / 6,804 | 1.99 / 1.55 | 155,155 / 233,110 | 68.1 | 12.0 | 592 | 0.4 | 0.996 |
| P1* | 796 / 1,629 / 2,682 | 0.72 / 0.61 | 54,790 / 90,850 | 24.4 | 5.5 | 592 | 0.7 | 0.994 |
| P2* | 749 / 1,509 / 2,661 | 0.66 / 0.61 | 50,627 / 90,112 | 22.5 | 5.1 | 592 | 0.7 | 0.993 |
| P1i* | 800 / 1,633 / 2,691 | 0.72 / 0.61 | 54,621 / 90,609 | 24.2 | 8.8 | 592 | 0.7 | 0.994 |
| P2i* | 752 / 1,529 / 2,675 | 0.67 / 0.61 | 51,001 / 90,030 | 22.6 | 8.8 | 592 | 0.7 | 0.993 |
| P1s* | 621 / 1,248 / 2,119 | 0.55 / 0.48 | 39,378 / 64,448 | 25.9 | 5.8 | 896 | 0.7 | 0.988 |
| P2s* | 553 / 1,181 / 2,703 | 0.52 / 0.62 | 35,223 / 78,099 | 34.1 | 6.8 | 989 | 0.7 | 0.985 |
| P1si* | 577 / 1,165 / 1,993 | 0.51 / 0.45 | 35,350 / 53,772 | 26.9 | 9.5 | 1,023 | 0.7 | 0.987 |
| P2si* | 481 / 1,002 / 2,093 | 0.44 / 0.48 | 27,027 / 52,425 | 36.3 | 12.1 | 1,172 | 0.7 | 0.982 |
| clair | 241 / 314 / 359 | 0.14 / 0.08 | 9,726 / 11,258 | 4.4 | 1.3 | 592 | 0.4 | 0.997 |
| clair i | 245 / 319 / 364 | 0.14 / 0.08 | 9,741 / 11,265 | 4.4 | 3.0 | 592 | 0.4 | 0.997 |

**yahoo**

| arm | modelled us at recall 0.8 / 0.9 / 0.95 | vs T1 at 0.9 / 0.95 | rows at 0.9 / 0.95 | clusters at 0.9 | groups at 0.9 | centres at 0.9 | epochs | max recall |
|---|---|---|---|---|---|---|---|---|
| T1 | 359 / 800 / 1,367 | 1.00 / 1.00 | 16,878 / 29,352 | 8.2 | 4.8 | 339 |  | 1.000 |
| T2 | 336 / 604 / 1,065 | 0.75 / 0.78 | 12,618 / 22,724 | 6.0 | 3.6 | 339 |  | 1.000 |
| T3 | 363 / 775 / 1,448 | 0.97 / 1.06 | 16,517 / 31,405 | 8.0 | 4.1 | 339 |  | 1.000 |
| T4 | 221 / 365 / 788 | 0.46 / 0.58 | 4,132 / 11,632 | 8.9 | 5.0 | 1,363 |  | 0.993 |
| T5 | 248 / 392 / 818 | 0.49 / 0.60 | 4,136 / 11,888 | 8.9 | 5.0 | 1,771 |  | 0.986 |
| O0 | 370 / 749 / 1,328 | 0.94 / 0.97 | 15,816 / 28,584 | 7.6 | 4.4 | 339 |  | 1.000 |
| O1 | 406 / 735 / 1,402 | 0.92 / 1.03 | 15,715 / 30,704 | 7.7 | 3.6 | 339 |  | 1.000 |
| O2 | 338 / 666 / 1,269 | 0.83 / 0.93 | 13,886 / 27,138 | 6.9 | 4.2 | 339 |  | 1.000 |
| P0 | 405 / 993 / 2,404 | 1.24 / 1.76 | 21,925 / 54,014 | 10.6 | 1.3 | 339 | 3.2 | 0.996 |
| P1 | 374 / 748 / 1,425 | 0.93 / 1.04 | 16,405 / 31,944 | 8.0 | 1.4 | 339 | 2.7 | 0.996 |
| P2 | 299 / 630 / 1,317 | 0.79 / 0.96 | 13,655 / 29,211 | 6.7 | 1.0 | 339 | 2.2 | 0.996 |
| P0i | 406 / 1,026 / 2,542 | 1.28 / 1.86 | 22,279 / 56,724 | 10.7 | 4.8 | 339 | 3.1 | 0.996 |
| P1i | 385 / 790 / 1,592 | 0.99 / 1.16 | 17,125 / 35,356 | 8.3 | 3.4 | 339 | 2.6 | 0.996 |
| P2i | 316 / 658 / 1,323 | 0.82 / 0.97 | 13,868 / 28,599 | 6.7 | 3.7 | 339 | 2.2 | 0.996 |
| P0s | 352 / 849 / 2,101 | 1.06 / 1.54 | 18,045 / 45,475 | 10.6 | 1.3 | 419 | 3.4 | 0.995 |
| P1s | 309 / 592 / 1,132 | 0.74 / 0.83 | 12,233 / 23,874 | 8.0 | 1.4 | 423 | 2.8 | 0.994 |
| P2s | 257 / 544 / 1,148 | 0.68 / 0.84 | 10,982 / 23,674 | 8.1 | 1.2 | 401 | 2.9 | 0.992 |
| P1si | 274 / 548 / 1,001 | 0.69 / 0.73 | 10,544 / 19,882 | 8.6 | 3.5 | 477 | 2.6 | 0.993 |
| P2si | 176 / 382 / 829 | 0.48 / 0.61 | 5,923 / 13,536 | 8.9 | 4.7 | 482 | 2.6 | 0.991 |
| P1 ag0 | 396 / 761 / 1,479 | 0.95 / 1.08 | 16,377 / 32,601 | 8.0 | 3.6 | 339 | 2.7 | 0.996 |
| P1 tau | 573 / 1,134 / 1,909 | 1.42 / 1.40 | 25,282 / 43,063 | 12.4 | 2.0 | 339 | 0.6 | 0.996 |
| P2 tau | 357 / 740 / 1,383 | 0.93 / 1.01 | 16,172 / 30,706 | 7.9 | 1.2 | 339 | 0.6 | 0.996 |
| P0* | 343 / 674 / 1,110 | 0.84 / 0.81 | 14,686 / 24,591 | 7.0 | 1.1 | 339 | 0.5 | 0.997 |
| P1* | 377 / 699 / 1,176 | 0.87 / 0.86 | 15,304 / 26,221 | 7.5 | 1.3 | 339 | 0.5 | 0.996 |
| P2* | 287 / 561 / 1,093 | 0.70 / 0.80 | 12,098 / 24,125 | 6.0 | 0.8 | 339 | 0.6 | 0.996 |
| P1i* | 397 / 718 / 1,259 | 0.90 / 0.92 | 15,449 / 27,731 | 7.5 | 3.2 | 339 | 0.6 | 0.998 |
| P2i* | 298 / 583 / 1,146 | 0.73 / 0.84 | 12,205 / 24,621 | 6.0 | 3.3 | 339 | 0.6 | 0.996 |
| P1s* | 306 / 566 / 948 | 0.71 / 0.69 | 11,647 / 19,885 | 7.8 | 1.3 | 418 | 0.7 | 0.997 |
| P2s* | 260 / 519 / 982 | 0.65 / 0.72 | 10,439 / 20,141 | 7.9 | 1.1 | 397 | 0.7 | 0.994 |
| P1si* | 267 / 509 / 901 | 0.64 / 0.66 | 9,645 / 17,711 | 8.2 | 3.5 | 471 | 0.7 | 0.996 |
| P2si* | 175 / 353 / 716 | 0.44 / 0.52 | 5,302 / 11,671 | 8.5 | 4.5 | 475 | 0.7 | 0.993 |
| clair | 190 / 270 / 318 | 0.34 / 0.23 | 5,512 / 6,594 | 2.7 | 0.2 | 339 | 0.4 | 1.000 |
| clair i | 197 / 287 / 335 | 0.36 / 0.25 | 5,591 / 6,637 | 2.7 | 2.0 | 339 | 0.4 | 1.000 |
