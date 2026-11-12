# Combined design and ablation under one accounting

Saved by the coordinating session from the build agent's returned text; condensed. Every number is
the agent's. Scripts, logs and raw results are in this directory (`core.py`, `build.py`, `run.py`,
`report.py`, `tables.py`, `resident.py`, `tables_*.md`, `report_*.txt`, `out_*.npz`).

## Headline (1M items, n = 768, 200 item and 369 text queries, zero violations)

Per-item bytes read (and % of the dataset at 1,536 B per item):

| Answer size | 0.01% | 0.1% | 1% | 5% |
|---|---|---|---|---|
| Item queries | 2.92 B (0.190%) | 7.12 B (0.463%) | 25.2 B (1.64%) | 78.5 B (5.11%) |
| Text queries | 33.4 B (2.17%) | 46.0 B (3.00%) | 79.3 B (5.17%) | 133.1 B (8.67%) |

Item queries at absolute thresholds 0.95 / 0.9 / 0.8 / 0.7: 0.049 / 0.105 / 1.045 / 4.09%.

All costs at N = 10⁹ (per-item, per-cluster, id lookups, per-query fixed): items 0.237 / 0.515 /
1.70 / 5.19%; text 2.24 / 3.06 / 5.24 / 8.75%. At N = 1M the local bases (109–196 MB per query)
dominate and totals are 7.3–21.5%.

## The design as built

1. **Support table (C2).** Anchors: 1,000 image cluster centres + 256 k-means centres of training
   prompts. Per query, nearest anchor p̂, β = q·p̂, ‖ρ‖ = √(1 − β²). Table stores exact max/min of
   p̂·x per cluster (2 B each, rounded outward); clusters decided without reading their centre.
2. **Sorted range (C1 + C2 per item).** Items sorted by â = x·m̂ (16 bits; also gives m·r). The
   undecided items are one contiguous range (proved: intersection of two intervals). Charged:
   opened cluster centres plus 4 binary-search endpoints × ⌈log₂ size⌉ × 2 B.
3. **Deterministic levels at 0, 4, 8, 16, 32 B.** Escape code (agent 1), nested, no scale, one
   1-byte weighted error norm per level (2^(1/16) grid, rounded up). α* at every level.
4. **Randomised levels at 48, 96, 192, 384, 576 B** coding the residual of the 32-byte prefix.
   Dimensions sorted by √(λ_res·Q_mix) and split into fixed blocks (96 / 128 / 256 / 288 at
   n = 768), one Haar rotation per block per group shared by all randomised levels, rates only
   increase across levels, sub-bit rates keep the first k′ rotated signs. Rates chosen greedily to
   minimise the predicted split bound. Scalars: ‖y_b‖ at 2 B per block once, tan φ rounded up at
   1 B per block whenever its rate rises (~9 B at the first randomised level, ~4 B at later ones).
5. **Full vector** (1,536 B).

Also: 8-bit cluster centres (exact decomposition against the stored centre). Randomised levels
start after 32 B (pilot: after 48 B is ~2% better for items, 3.7% worse for text; after 16 B worse
for both).

**Constant c.** ε = 10⁻⁹, N = 10⁹, 5 randomised levels; union over all N × 5 (item, level) events
(seed-independent count) gives c = 9.354 sub-Gaussian. In the runs, an exact Chernoff bound per
undecided pair with a tabulated log ₀F₁ (upper-bounded by linear interpolation of a convex
function; equivalent c = 7.56 for one 96-dimension block). Each level's guarantee is valid with
shared rotations across levels (stabiliser argument).

## Ablation, 1M, n = 768 (per-item bytes; change from previous row)

Item queries:

| Row | 0.01% | 0.1% | 1% | 5% | 0.95 | 0.9 | 0.8 | 0.7 |
|---|---|---|---|---|---|---|---|---|
| Original construction, honest scalars | 8.69 | 14.83 | 39.59 | 119.9 | 5.21 | 8.45 | 30.60 | 95.2 |
| + escape code | 7.58 (−13%) | 12.19 (−18%) | 30.03 (−24%) | 85.2 (−29%) | 4.72 | 7.31 | 23.53 | 67.3 |
| + α* | 6.92 (−9%) | 11.31 (−7%) | 28.74 (−4%) | 83.2 (−2%) | 4.36 | 6.56 | 22.16 | 65.4 |
| + randomised levels | 6.97 (+1%) | 11.49 (+2%) | 29.53 (+3%) | 83.4 (0%) | 4.39 | 6.61 | 22.40 | 66.0 |
| + sub-bit tail blocks | 6.97 (0%) | 11.46 (0%) | 29.21 (−1%) | 80.8 (−3%) | 4.38 | 6.60 | 22.24 | 64.9 |
| + sorted range | 4.62 (−34%) | 9.58 (−16%) | 28.19 (−4%) | 80.7 (0%) | 1.47 | 4.29 | 21.38 | 64.7 |
| **+ support table = combined** | **2.92** | **7.12** | **25.20** | **78.5** | **0.76** | **1.61** | **16.05** | **62.9** |
| Items touched, % | 12.9 | 22.3 | 46.9 | 84.1 | 3.0 | 6.7 | 38.5 | 83.3 |
| Deterministic only + range + table | 2.86 | 6.97 | 24.73 | 80.95 | 0.73 | 1.58 | 15.99 | 63.3 |

(The original row treats independent allocations as nested; real nesting costs it +4 to +8%, so it
flatters the baseline.)

Text queries:

| Row | 0.01% | 0.1% | 1% | 5% |
|---|---|---|---|---|
| Original construction, honest scalars | 55.83 | 76.04 | 133.2 | 237.7 |
| + escape code | 42.14 (−25%) | 56.68 (−25%) | 96.47 (−28%) | 166.0 (−30%) |
| + α* | 39.21 (−7%) | 53.49 (−6%) | 92.86 (−4%) | 162.0 (−2%) |
| + randomised levels | 37.25 (−5%) | 50.03 (−6%) | 84.27 (−9%) | 141.0 (−13%) |
| + sub-bit tail blocks | 36.09 (−3%) | 48.03 (−4%) | 80.12 (−5%) | 133.2 (−6%) |
| + sorted range | 35.42 (−2%) | 47.57 (−1%) | 79.97 (0%) | 133.2 (0%) |
| **+ support table = combined** | **33.39** | **46.01** | **79.34** | **133.1** |
| Items touched, % | 64.1 | 74.2 | 90.3 | 99.1 |
| Deterministic only + range + table | 36.51 | 51.47 | 92.07 | 161.9 |

Interactions: the table is far weaker for text (64–99% of items touched vs 13–84%); training-prompt
anchors recover −4.4% at 0.01%. Randomised levels and skipping items are independent. The sharp
tail is worth −0.7 to −3.6%. For text at 0.1%, levels 0–32 B take 24 of 46 B per item.

## Other dimensions (item queries; per-item bytes, % of dataset)

| Set | 0.01% | 0.1% | 1% | 5% |
|---|---|---|---|---|
| n = 1,024 native BioCLIP-2.5, 200k | 2.63 (0.128%) | 5.14 (0.251%) | 13.56 (0.662%) | 31.27 (1.527%) |
| n = 128 PCA stand-in, items | 1.63 (0.638%) | 2.84 (1.11%) | 6.66 (2.60%) | 17.48 (6.83%) |
| n = 128 PCA stand-in, text | 7.57 (2.96%) | 10.85 (4.24%) | 18.74 (7.32%) | 33.58 (13.1%) |
| n = 100 native GloVe, deterministic + range + table | 23.4 (11.7%) | 31.7 (15.9%) | 46.2 (23.1%) | 72.6 (36.3%) |

n = 1,024 at 0.95 / 0.9 / 0.8 / 0.7: 0.100 / 0.111 / 0.123 / 0.270%. Randomised levels: 0 to −5%
at n = 1,024; ~0% at n = 128; +5 to +35% at n = 100 (GloVe thresholds sit at cosine 0.55 / 0.45 /
0.33 / 0.23, so that set mostly measures low-similarity geometry). Range and table do nothing on
GloVe at those thresholds.

## Fixed costs and memory

- Local bases of touched groups (float32, 2.36 MB each): 109–196 MB per query — 7–13% of the
  dataset at 1M, 0.007–0.013% at 10⁹. Largest single cost at 1M. fp16 would halve it (conjectured).
- Centres + range searches + table row: 0.62–0.86 B per item. Id lookup: 4 B per returned item.
- Stored bytes per item, cumulative: 3 (â + norm), 8, 13, 22, 39 (through 32 B), 67, 119, 219, 611
  (through 576 B); full vector 1,536 B and id 4 B on disk.
- Suggested resident: through 32 B (39 B/item) or through 16 B (22 B/item).
- Fixed at 10⁹: centres and metadata 788 MB, bases 151 MB, rotations 45 MB (or regenerate), support
  table 5.0 GB on disk (5 B/item, one row read per query).

## Contradictions with earlier reports

1. Agent 2's randomised text gain (−13 to −29%) is −8 to −18% in a real nested stream; for items
   randomised levels never pay at answers ≤ 1%.
2. Agent 3a's sub-bit tail blocks (~20% narrower bounds) are worth 0–3% for items, 3–6% for text.
3. Agent 1's sharper constant (conjectured 5–10%) is worth 0.7–3.6%.
4. No earlier figure counted the local bases.
5. Agent 3b's conjectured 0.12% / 0.29% at 0.95 / 0.9 was pessimistic: 0.049% / 0.105% per item.
   The 4K² table does not scale; with 1,256 anchors it is 5 B per item at 10⁹.
6. "Cluster pruning is weak" (FINDINGS 6.1) is superseded: 3.0% of items touched at 0.95.
7. Randomised certificates only at deep levels: confirmed.

## Caveats

Bases charged at float32 (fp16 not simulated); code lengths exact but no bit-serial encoder;
randomised levels charged full budget; item queries are corpus items; 1M sample is 20 dense slices;
text prompts written by agent 2; profiles fitted on training data only.
