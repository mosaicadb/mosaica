# Does the best items-per-cluster depend on N?

Code: branch `research/levels-clusters`, on `research/combined` at d443f66, on top of the deterministic-level and cluster-size changes. Earlier work it builds on:
- `REPORT_DETLEVELS.md`
- `REPORT_CLUSTERSIZE.md`, with its results in `results/clustersize/`

**Configuration:**
- The NDL 2 binary with `--dlv 16,32` and the s3 configuration (`--int --gtab --flags repack`).
- The fixed plan constants `results/bounds/plan_fixed.txt`. They were timed on 1,000-item clusters and were not retuned.
- One thread pinned to core 2, the process on cores 1–3, nothing else running while timing.
- The machine is a 4-core Xeon VM with AVX-512.
- Scripts, CSVs, fit logs, metadata, verify summaries and `tables.md` are in `results/scaling/`.

Every number is marked **measured**, or **inferred** / **modelled**.

## Summary

**Coarser clusters are faster for text queries at every N and answer size tested (measured).** The best coarse size against 1,000 per cluster:

| Sample | Answers 0.01–1% | Answers 10–75% |
|---|---|---|
| 500k row groups | −9 to −16% | −5 to −11% |
| 1M row groups | −18 to −22% (4,000) | −11 to −15% |
| R1M | −8 to −20% (2,000) | −2 to −9% |
| R3M | −13 to −18% (4,000 or 8,000) | −6 to −10% |

**Item queries are mixed (measured).**
- At large answers they gain 6–17% with coarser clusters.
- At small answers 8,000 per cluster is costly: +108% at R1M item 0.01%, +177% at 500k item 0.01%.
- 4,000 per cluster costs +44% at R1M item 0.01%, but −13% at R3M item 0.01%.

**The best size (measured):**

| Sample | Best size |
|---|---|
| 500k row groups | 2,000–8,000; the margins are within noise |
| 1M row groups | 4,000, for both classes |
| R1M | 2,000 for items; text split between 2,000 and 8,000 |
| R3M | 4,000 for exact k-NN (16–51% faster than 2,000); on threshold queries 2,000 and 4,000 tie (geometric mean −1%), with 2,000 faster on selective item queries and 8,000 at large item answers |

2,000 at R3M was timed in a later session, against 4,000 refitted (bit-identical) and timed again in that session as a control; see "R3M at 2,000". The step from 2,000 at R1M to 4,000 at R3M is measured where the sizes differ most:
- exact k-NN: 2,000 was 21–32% faster than 4,000 on item k-NN at R1M, and is 48–51% slower at R3M;
- the penalty of 4,000 on selective item queries shrinks: against 2,000 at item 0.01%, +58% at R1M and +22% at R3M.

**What changes with N at a fixed size (measured over two-point pairs):**
- For item queries at small answers, the touched share and the share of clusters opened fall with N. The log-log slope is −0.2 to −0.5.
- For text queries the change is small at 0.01% (−0.06 to −0.12) and nil at 1% and above.

So the small-answer penalty of large clusters for item queries shrinks as N grows.

**Recommendation (inferred).** Make the cluster size grow slowly with N rather than stay at 1,000. One candidate rule is items per cluster ≈ 2,000 × √(N / 1M), rounded to 2,000 / 4,000 / 8,000:
- 2,000 at 1M;
- 3,500 → 4,000 at 3M;
- 6,300 → 8,000 at 10M.

It rests on two well-measured N (R1M and R3M), with every size timed at both. If one constant has to ship now, 2,000 is the safer choice for corpora near 1M (inferred):
- it was more than 10% worse than 1,000 in only two cells: row-group 1M item 1% (+14%, slower in both repeats) and row-group 1M item k-NN k = 100 (+13%, one process);
- 4,000 was 44% worse on R1M item 0.01%.

At 3M the same constant costs 16–51% on exact k-NN against 4,000 (measured), so a constant does not serve both.

**The sub-clustered approximate mode.** With sub-clusters of about 62 items, items scanned are close to independent of the parent size (measured). So coarser parents do not harm that mode if it uses fixed-size sub-clusters.

## Why the 5M point was replaced

**Distribution (measured, `thresholds.log`).** The original 1M is one row group per shard, and the extra 4M rows came from the other six row groups of each shard. Compared at the same answer fraction:

| Pair | text 0.1% | item 0.1% |
|---|---|---|
| 500k vs 1M (row groups) | median 0.0001, largest 0.0008 | median 0.0002, largest 0.0010 |
| 1M vs 5M | median 0.046, largest 0.086 | median 0.015, largest 0.17 |

So 1M → 5M would have mixed more items with different items.

**Memory (measured, query-process anonymous memory):**

| N | Anonymous memory |
|---|---|
| 500k | 1.99 GiB |
| row-group 1M and R1M | 3.08–3.37 GiB |

That is about 0.9 + 2.3 GiB per million rows. At 5M it would be about 12.6 GiB, plus 7.7 GiB of vectors that must stay in the page cache: too much for 15 GB.

**The first 5M attempt.** A container restart killed it at about 00:50. A 5M fit and a probe job were running together; that memory caused the restart is inferred, not shown. After the restart, a watchdog killed the newest job whenever MemAvailable fell below 1.5 GB. It never fired.

**The replacement (measured):** nested uniform samples of the 5M population, stratified by shard, over all rows of each shard:
- R1M (50,000 per shard) ⊂ R3M (150,000 per shard).
- 200 new item queries drawn from R1M; the same text queries.
- 199,758 of R1M's rows lie in the original 1M row groups.

| Pair (`thresholds_r.log`) | Median difference | Largest |
|---|---|---|
| R1M vs R3M | ≤ 0.0006 | 0.0030 |
| R3M vs 5M | ≤ 0.0002 | 0.0058 |
| R1M vs the row-group 1M | 0.011–0.047 at text 0.01–1% | — |

**The anonymous memory check (measured):**

| Sample | Prediction | Measured |
|---|---|---|
| R1M | — | 3.08–3.37 GiB |
| R3M | 7.9 GiB | 7.23 GiB (1,000), 7.85 (4,000), 8.16 (8,000) |

4M was ruled out: about 10.3 + 6.1 GiB, above the 13 GB limit.

**The report therefore has three separate tests:**
1. **Density, row-group population:** 500k → 1M (2×). 500k is a stratified random half of the 1M.
2. **Density, wide population:** R1M → R3M (3×).
3. **Diversity at fixed N:** the row-group 1M against R1M.

## Changes

**`tools/fit.py`:**
- Normalised rows are read from the source on demand (`Rows`) instead of a full float32 copy, and `Xs.npy` is written through a memmap.
- `--rows FILE` fits a subset of the source rows.
- `--per-cluster` and `--groups` are kept from the cluster-size study.

The default fit reproduces the stored indices exactly (measured):
- at 1M against `c1m`: `Xs`, `perm`, `cstart`, `grp`, `mq8`, `msc`, `qi` identical, and `V`, `lam`, `r2`, `profile` bit-identical;
- at 50k against `c50k`: identical, after the `--rows` change.

Fit peak memory at 1M, including the file cache, fell to 3.76 GiB.

**`src/run2.rs`:**
- The breakdown counts line adds the groups set up and the full reads per query.
- `e2e --knn-k 10,100` times exact k-NN at several k, recorded with `answer_frac` = k.
- The query path is unchanged.

**The P4 refit reproduces the cluster-size study's P4** (measured): 1M at 4,000 gives K = 250, clusters 579..15,490, G = 64, groups 624..49,491.

## Fits (measured)

| Sample | Size | K | G | Cluster sizes | Group sizes | Clusters per group | Groups below n = 768 | Largest basis condition number |
|---|---|---|---|---|---|---|---|---|
| 500k (row groups) | 1,000 | 499 | 64 | 85..6,544 | 920..24,770 | 1..25 | 0 | 30 |
| | 2,000 | 250 | 64 | 305..9,881 | 669..24,961 | 1..15 | 1 | 126 |
| | 4,000 | 125 | 64 | 902..18,880 | 993..26,216 | 1..6 | 0 | 26 |
| | 8,000 | 62 | 62 | 2,126..21,935 | 2,126..21,935 | 1 (one cluster per group) | 0 | 20 |
| 1M (row groups) | 1,000 | 999 | 64 | 84..4,635 | 1,359..49,701 | 1..51 | 0 | 21 |
| | 2,000 | 500 | 64 | 74..12,174 | 447..85,083 | 1..35 | 2 | 278 |
| | 4,000 | 250 | 64 | 579..15,490 | 624..49,491 | 1..11 | 1 | 108 |
| | 8,000 | 125 | 64 | 1,158..32,429 | 1,924..49,797 | 1..5 | 0 | 19 |
| R1M | 1,000 | 1,000 | 64 | 87..4,679 | 417..38,554 | 1..50 | 1 | 197 |
| | 2,000 | 500 | 64 | 316..9,180 | 780..48,653 | 1..22 | 0 | 64 |
| | 4,000 | 250 | 64 | 620..19,420 | 780..57,326 | 1..11 | 0 | 59 |
| | 8,000 | 125 | 64 | 1,751..30,736 | 2,879..84,920 | 1..9 | 0 | 19 |
| R3M | 1,000 | 3,000 | 64 | **1**..5,081 | **96**..160,105 | 1..129 | 1 | **1,612** |
| | 4,000 | 750 | 64 | 310..16,875 | 3,876..152,100 | 2..29 | 0 | 14 |
| | 8,000 | 375 | 64 | 1,480..42,772 | 1,480..133,092 | 1..18 | 0 | 18 |
| | 2,000 (later session) | 1,500 | 64 | 88..10,405 | 2,266..156,735 | 1..66 | 0 | 17 |

The R3M 1,000 fit has a one-item cluster and a 96-item group, with basis condition 1,612. This affects efficiency, not soundness. It may slightly handicap 1,000 at R3M (inferred).

## Method

- **Each process** runs end-to-end (median over 3 runs of the median over 20 queries) at six answer sizes, item and text; exact k-NN at k = 10 and 100 where noted; then the profiled breakdown with the counts line.
- **Breakdown cells:** six answer sizes at 500k, 1M and R1M; 0.01 / 1 / 50% at R3M.
- **Verify** (`--vitem 20 --vtext 20`) runs first, in the first pass only.
- **500k, row-group 1M and R1M:** every fit forward, then reverse.
- **R3M:** 1,000 / 4,000 / 8,000 once each, then the second repeats of 4,000 (verify first) and 1,000 on the same kept indices. A refit with the same seed is bit-identical, so no refit was done.
- **R3M at 2,000** was added later, after a container restart: fit, then 2,000 (verify first), 4,000, 4,000, 2,000, each a separate process, with 4,000 refitted for the control (`perm`, `cstart` and `grp` identical to the first fit). It is kept out of the main tables, which compare runs of one session; `cmp_r3m_session.py` compares it.
- **The best coarse size at R3M** was taken as the lower geometric mean over the twelve threshold cells: 4,000.
- **k-NN for the row-group fits:** one separate process each.
- **Item queries:** corpus items at every N.
  - The row-group runs use 200 rows drawn from the 500k sample.
  - The R runs use 200 rows drawn from R1M.
  - So item times are comparable within a population, not across populations.
- **Text queries:** the same `text_q.npy` everywhere.

## Noise (measured)

Spread is (max − min) / mean between repeats, over the 168 cells that have repeats:
- Median spread: 4.3%.
- Largest: 34.4% (500k at 2,000, text 0.01%: 11.1 / 15.8 ms).
- 64 cells are above 6%. The largest:

| Cell | Spread |
|---|---|
| 1M row groups, 4,000, item 0.1% | 33.0% |
| R3M, 1,000, item 0.01% | 23.1% |
| R1M, 8,000, item 10% | 22.1% |
| R1M, 2,000, item 1% | 21.0% |
| 1M row groups, 4,000, text 0.01% | 19.0% |
| R1M, 1,000, text 10% | 18.1% |
| 500k, 2,000, item 1% | 18.1% |
| 1M row groups, 4,000, item 0.01% | 18.0% |
| R1M, 8,000, item 75% | 17.3% |
| R1M, 2,000, item 0.1% | 16.4% |
| R3M, 4,000, item 0.1% | 15.5% |
| 1M row groups, 1,000, item 0.1% | 15.1% |

The other 52 cells, all between 6.0% and 15.1%, are listed in `tables.md`.

Two of these are systematic, not random:
- The R1M repeats spread more than the row-group ones.
- 1M at 4,000 ran about 10% faster in its second process at nearly every cell.

**A change smaller than about 10% in a single cell is not a measured difference.**

## Results

### Text, ms: mean of repeats, change against 1,000 at the same N (measured)

| Sample | Size | 0.01% | 0.1% | 1% | 10% | 50% | 75% |
|---|---|---|---|---|---|---|---|
| 500k | 1,000 | 16.1 | 18.5 | 29.5 | 44.3 | 57.0 | 54.8 |
| | 2,000 | 13.5, −16% | 16.4, −11% | 26.7, −9% | 41.3, −7% | 54.4, −5% | 50.8, −7% |
| | 4,000 | 14.5, −10% | 16.6, −10% | 26.7, −10% | 39.7, −10% | 52.6, −8% | 50.6, −8% |
| | 8,000 | 15.9, −1% | 17.4, −6% | 26.4, −11% | 39.8, −10% | 52.5, −8% | 48.7, −11% |
| 1M row groups | 1,000 | 25.3 | 31.5 | 53.7 | 83.4 | 110.3 | 103.5 |
| | 2,000 | 22.7, −10% | 27.6, −12% | 48.0, −11% | 77.6, −7% | 104.4, −5% | 98.1, −5% |
| | 4,000 | 19.7, −22% | 25.1, −21% | 44.3, −18% | 71.2, −15% | 97.7, −11% | 91.3, −12% |
| | 8,000 | 21.9, −13% | 26.3, −17% | 46.0, −14% | 73.3, −12% | 97.6, −11% | 92.3, −11% |
| R1M | 1,000 | 34.8 | 46.6 | 62.5 | 91.6 | 118.2 | 115.2 |
| | 2,000 | 27.9, −20% | 41.2, −12% | 57.4, −8% | 89.4, −2% | 115.3, −2% | 104.9, −9% |
| | 4,000 | 29.5, −15% | 41.5, −11% | 59.2, −5% | 88.9, −3% | 118.9, +1% | 110.1, −4% |
| | 8,000 | 31.5, −9% | 41.3, −12% | 56.6, −9% | 88.4, −3% | 120.2, +2% | 107.1, −7% |
| R3M | 1,000 | 76.7 | 115.7 | 172.7 | 273.5 | 369.6 | 338.4 |
| | 4,000 | 64.9, −15% | 100.3, −13% | 150.8, −13% | 251.3, −8% | 335.4, −9% | 318.2, −6% |
| | 8,000 (one run) | 63.2, −18% | 99.6, −14% | 153.2, −11% | 254.0, −7% | 339.0, −8% | 305.1, −10% |

### Item, ms (measured)

| Sample | Size | 0.01% | 0.1% | 1% | 10% | 50% | 75% |
|---|---|---|---|---|---|---|---|
| 500k | 1,000 | 2.0 | 3.1 | 8.8 | 38.7 | 52.7 | 48.2 |
| | 2,000 | 2.1, +5% | 3.1, −1% | 7.5, −15% | 35.2, −9% | 47.3, −10% | 44.4, −8% |
| | 4,000 | 1.6, −21% | 3.2, +2% | 9.0, +2% | 34.4, −11% | 48.3, −8% | 44.5, −8% |
| | 8,000 | 5.6, +177% | 7.1, +127% | 11.3, +28% | 34.1, −12% | 46.4, −12% | 43.5, −10% |
| 1M row groups | 1,000 | 3.7 | 5.6 | 13.1 | 69.2 | 98.9 | 90.1 |
| | 2,000 | 3.3, −11% | 5.4, −4% | 15.0, +14% | 65.8, −5% | 91.6, −7% | 83.8, −7% |
| | 4,000 | 2.6, −30% | 4.9, −14% | 12.9, −2% | 60.0, −13% | 85.7, −13% | 77.7, −14% |
| | 8,000 | 4.2, +14% | 6.8, +21% | 15.9, +21% | 59.4, −14% | 85.9, −13% | 78.7, −13% |
| R1M | 1,000 | 7.6 | 17.1 | 38.9 | 79.5 | 102.3 | 101.5 |
| | 2,000 | 6.9, −9% | 15.8, −8% | 31.8, −18% | 71.3, −10% | 95.9, −6% | 88.6, −13% |
| | 4,000 | 10.9, +44% | 18.4, +8% | 35.3, −9% | 72.5, −9% | 101.5, −1% | 91.0, −10% |
| | 8,000 | 15.8, +108% | 23.4, +37% | 37.7, −3% | 79.4, −0% | 102.2, −0% | 93.1, −8% |
| R3M | 1,000 | 15.9 | 35.0 | 91.2 | 223.9 | 313.2 | 284.6 |
| | 4,000 | 13.8, −13% | 31.8, −9% | 83.0, −9% | 203.8, −9% | 286.7, −8% | 265.5, −7% |
| | 8,000 (one run) | 17.9, +13% | 33.2, −5% | 84.7, −7% | 193.3, −14% | 260.3, −17% | 243.2, −15% |

Bytes per item, the full-vector share, the touched share and the entry shares of every cell are in `tables.md`.

### Best size per N, class and answer (measured; the margin is to the next best size)

| Sample | Class | 0.01% | 0.1% | 1% | 10% | 50% | 75% |
|---|---|---|---|---|---|---|---|
| 500k | text | 2,000, +8% | 2,000, +1% | 8,000, +1% | 4,000, +0% | 8,000, +0% | 8,000, +4% |
| | item | 4,000, +27% | 2,000, +1% | 2,000, +17% | 8,000, +1% | 8,000, +2% | 8,000, +2% |
| 1M row groups | text | 4,000, +11% | 4,000, +5% | 4,000, +4% | 4,000, +3% | 8,000, +0% | 4,000, +1% |
| | item | 4,000, +27% | 4,000, +11% | 4,000, +2% | 8,000, +1% | 4,000, +0% | 4,000, +1% |
| R1M | text | 2,000, +6% | 2,000, +0% | 8,000, +2% | 8,000, +0% | 2,000, +2% | 2,000, +2% |
| | item | 2,000, +10% | 2,000, +8% | 2,000, +11% | 2,000, +2% | 2,000, +6% | 2,000, +3% |
| R3M (1,000 / 4,000 / 8,000) | text | 8,000, +3% | 8,000, +1% | 4,000, +2% | 4,000, +1% | 4,000, +1% | 8,000, +4% |
| | item | 4,000, +15% | 4,000, +4% | 4,000, +2% | 8,000, +5% | 8,000, +10% | 8,000, +9% |

For 2,000 at R3M, see "R3M at 2,000". Most text margins between coarse sizes are within noise. 1,000 is never the best, though in a few cells it is within noise of the best (500k item 0.1%: 3.1 against 3.1 ms).

### The three tests

**1. Density, row-group population, 500k → 1M (measured).** At 1,000 per cluster, time grows as N to the power:
- text 0.65 at 0.01%, 0.95 at 50%;
- item 0.86 at 0.01%, 0.91 at 50%.

The best size moves from "anything 2,000–8,000" at 500k to 4,000 at 1M.

**2. Density, wide population, R1M → R3M (measured).**
- At 1,000: text grows as N^0.72 at 0.01% and N^1.04 at 50%; item as N^0.67 at 0.01% and N^1.03 at 50%.
- At 4,000: text N^0.72 at 0.01% and N^1.01 at 50%.
- The best size moves from 2,000 to 4,000 for exact k-NN; on threshold queries 2,000 and 4,000 tie at R3M ("R3M at 2,000").
- **Measured at R3M against R1M:** the penalty of 4,000 on small item answers disappears (+44% → −13% at item 0.01%), and 8,000 improves for items (+108% → +13% at 0.01%).

**3. Diversity at fixed N, row-group 1M against R1M, at 1,000 per cluster.** Only the text queries are shared, so this compares text times.
- **Measured:** R1M text is slower: +38% at 0.01%, +48% at 0.1%, +16% at 1%, +10% at 10%, +7% at 50%, +11% at 75%.
- **Inferred:** the wider population spreads text queries over more items before the levels decide.
- The coarse-cluster gain is smaller on R1M (best −8 to −20% at small text answers) than on the row-group 1M (−18 to −22%).
- The best size moves from 4,000 to 2,000.

### Counts at fixed size across N (measured, two points per population)

Log-log slopes per doubling of N, listed as touched share / opened share of K / item-levels per item / full reads per item.

| Pair | Query | Size | 0.01% | 1% | 50% |
|---|---|---|---|---|---|
| 500k → 1M | item | 1,000 | −0.33 / −0.09 / −0.27 / −1.06 | −0.19 / −0.08 / −0.12 / −0.73 | ≈0 |
| | item | 4,000 | −0.24 / +0.02 / −0.19 / −1.27 | −0.15 / +0.01 / −0.10 / −0.62 | ≈0 |
| | item | 8,000 | −0.50 / −0.27 / −0.44 / −0.81 | −0.15 / −0.02 / −0.11 / −0.36 | ≈0 |
| | text | 1,000 | −0.12 / −0.01 / −0.13 / −0.95 | −0.02 / +0.01 / −0.03 / −0.42 | ≈0 |
| R1M → R3M | item | 1,000 | −0.24 / −0.15 / −0.21 / −0.62 | −0.05 / −0.03 / −0.06 / −0.03 | ≈0 |
| | item | 4,000 | −0.28 / −0.18 / −0.25 / −1.44 | −0.06 / −0.04 / −0.08 / −0.31 | ≈0 |
| | item | 8,000 | −0.33 / −0.20 / −0.32 / −0.95 | −0.09 / −0.05 / −0.08 / −0.07 | ≈0 |
| | text | 1,000–8,000 | −0.06 / −0.02 to +0.01 / −0.08 to −0.10 / varies | ≈0 | ≈0 |

At a fixed size, item queries at small answers touch, open and read a smaller share as N grows. The guess that "clusters are tighter in a denser corpus" holds for item queries at small answers, and barely for text. Each exponent rests on two points of one population, so these are not fitted exponents.

### Model (modelled)

**How it was fitted.** Each profiled phase was fitted against its own count by non-negative least squares, relative to the phase time, on the 500k and row-group 1M breakdowns: 96 cells. The fitted costs are profiled time on this VM:

| Phase | Cost (profiled) |
|---|---|
| Group setup | 105.8 µs per group |
| Cut | 2.10 µs per cluster |
| Opened cluster | 2.34 µs + 31.7 ns per table unit |
| Item-level | 14.7 ns |
| Full read | 168 ns |
| Other | 0.18 ms + 6.8 ns per item entering |

Wall time is the profiled prediction times the median wall/profiled ratio of the training cells: 0.88 for item, 0.81 for text.

**Prediction against measurement:**

| Sample | Profiled time: median / largest error | Wall time: median / largest error |
|---|---|---|
| 500k (in sample) | 10.8% / 27.7% | 11.8% / 180% |
| 1M row groups (in sample) | 9.2% / 20.5% | 12.8% / 150% |
| R1M (out of sample) | 13.2% / 24.1% | 11.5% / 66% |
| R3M (out of sample) | 9.1% / 21.9% | 13.9% / 73% |

The large wall errors are all item cells of 2–16 ms at 0.01–0.1%, where fixed per-query costs dominate. At R3M:
- 50% answers are predicted 14–23% low;
- 1% answers within 2–8%;
- text 0.01% 4–13% high.

The full predicted-against-measured table, for every breakdown cell, is in `tables.md`. The model is good enough to rank sizes at the middle answers (modelled), not to replace measurement at the small item answers.

**Breakdown at R3M, text 0.01%, profiled (measured).**

| Size | Cut + cluster setup + tables + level setup | Per-item phases |
|---|---|---|
| 1,000 | 26.8 ms (2,802 clusters opened) | 45.8 ms |
| 4,000 | 10.1 ms (744 opened) | 48.2 ms |
| 8,000 | 7.1 ms (373 opened) | 53.6 ms |

Group setup is 8.4–8.8 ms at every size. Larger clusters cut per-cluster work about in proportion to K, and give part of it back as more items entering (2.21M → 2.48M → 2.59M).

## Exact k-NN (measured)

Each cell is ms, then the change against 1,000 at the same N.

| Sample | Size | item k10 | item k100 | text k10 | text k100 |
|---|---|---|---|---|---|
| 500k | 1,000 | 2.51 | 5.73 | 13.07 | 15.93 |
| | 2,000 | 2.18, −13% | 5.66, −1% | 10.69, −18% | 13.86, −13% |
| | 4,000 | 2.36, −6% | 3.15, −45% | 10.99, −16% | 13.94, −13% |
| | 8,000 | 4.66, +86% | 6.89, +20% | 9.67, −26% | 11.95, −25% |
| 1M row groups | 1,000 | 3.59 | 5.89 | 20.43 | 23.75 |
| | 2,000 | 3.04, −15% | 6.68, +13% | 19.09, −7% | 22.62, −5% |
| | 4,000 | 2.76, −23% | 4.32, −27% | 17.13, −16% | 20.02, −16% |
| | 8,000 | 4.06, +13% | 6.37, +8% | 12.52, −39% | 15.18, −36% |
| R1M | 1,000 | 6.50 | 8.35 | 24.47 | 30.62 |
| | 2,000 | 6.40, −2% | 7.10, −15% | 23.66, −3% | 27.86, −9% |
| | 4,000 | 8.06, +24% | 10.45, +25% | 24.02, −2% | 28.86, −6% |
| | 8,000 | 13.80, +112% | 18.02, +116% | 26.26, +7% | 30.89, +1% |
| R3M | 1,000 | 13.36 | 17.30 | 61.45 | 69.18 |
| | 4,000 | 7.98, −40% | 11.24, −35% | 44.56, −27% | 52.87, −24% |
| | 8,000 (one run) | 9.91, −26% | 14.71, −15% | 47.34, −23% | 54.42, −21% |

The row-group k-NN figures are single processes; R1M and R3M at 1,000 and 4,000 have two repeats. k-NN follows the threshold pattern: 8,000 hurts item k-NN at R1M, and 4,000 is the best at R3M, including against 2,000 ("R3M at 2,000").

## R3M at 2,000 (measured)

This was timed in a later session, after a container restart, as four processes in the order 2,000 (verify first), 4,000, 4,000, 2,000. 4,000 was refitted with the same seed (bit-identical) and timed again as the control. That session's 4,000 was 1–13% faster than the first session's in 15 of 16 cells (about 5% at the median), so 2,000 is compared only with it. Each cell is the mean of two processes; `tables.md` has both.

| Cell | 4,000 | 2,000 | 2,000 against 4,000 |
|---|---|---|---|
| item 0.01% | 13.2 | 10.8 | **−18%** |
| item 0.1% | 29.9 | 26.1 | **−12%** |
| item 1% | 75.5 | 72.7 | −4% |
| item 10% | 198.2 | 198.6 | +0% |
| item 50% | 277.5 | 283.7 | +2% |
| item 75% | 249.6 | 249.4 | −0% |
| item k-NN k = 10 | 7.7 | 11.7 | **+51%** |
| item k-NN k = 100 | 11.6 | 17.2 | **+48%** |
| text 0.01% | 56.6 | 59.8 | +6% |
| text 0.1% | 90.7 | 91.4 | +1% |
| text 1% | 142.2 | 136.2 | −4% |
| text 10% | 237.8 | 228.9 | −4% |
| text 50% | 315.5 | 363.5 | **+15%** |
| text 75% | 301.7 | 326.5 | +8% |
| text k-NN k = 10 | 44.3 | 51.4 | **+16%** |
| text k-NN k = 100 | 48.3 | 56.1 | **+16%** |

Geometric means of 2,000 against 4,000: threshold cells −1.2% (item −5.6%, text +3.4%); exact k-NN +31.7%.

**Noise.** The control's two processes differ by up to 20% (item 75%), and by 11–17% on text 1–75%. The cells in bold have both 2,000 processes on the same side of both 4,000 processes; the others overlap.

**Counts per query (first process of each).** 2,000 opens about twice the clusters (text 0.01%: 1,464 against 744; item 0.01%: 554 against 339) and sends more of them to full vectors (item 0.01%: 126 against 63). On selective item queries the finer clusters exclude more: 617k items enter the first level against 774k. At text and larger answers the items entering differ by at most 6%, so the extra clusters are cost with no saving.

**Not profiled:** why exact k-NN is slower at 2,000. The breakdown covers threshold cells only.

## Approximate-mode probe counts (pure counting; measured)

**The scripts** were adapted from annideas' `prep_sim.py` / `sim_probe.py` / `sim_probe2.py` (`results/scaling/probe.py`).
- **Ordering:** centre score + 0.2 × RMS radius, with radius = √(1 − |mean|²) of the cluster's unit vectors (`sim_probe2.py`'s `radof`), for parents and sub-clusters alike.
- **Sub-clusters:** plain k-means inside each parent, 8 iterations, seed 1.
- **Two-level probing:** the top P parents are opened, then only their sub-centres are ranked. For each target, the P with the fewest items scanned is reported.
- **Queries:** 50 item queries and 100 text queries.
- **Truth:** the exact top 1,000 per query at that N.
- **Fits counted:** all 500k, row-group 1M and R1M fits, and R3M at 1,000, 2,000 and 4,000.

**Items scanned at mean recall 0.99, k = 10**, schemes (a) parents only / (b) sub-clusters of about 62 / (c) sub-clusters of about 250:

| Fit | image | text |
|---|---|---|
| 1M row groups, 1,000 | 12,100 / 6,643 / 8,411 | 20,001 / 6,356 / 12,763 |
| 1M row groups, 4,000 | 23,353 / 5,927 / 9,427 | 32,266 / 5,309 / 14,236 |
| 1M row groups, 8,000 | 37,797 / 5,315 / 9,511 | 45,249 / 6,504 / 9,619 |
| R1M, 1,000 | 15,957 / 3,444 / 5,020 | 13,196 / 4,784 / 8,204 |
| R1M, 2,000 | 20,338 / 2,504 / 3,063 | 13,677 / 3,916 / 7,214 |
| R1M, 4,000 | 23,014 / 2,331 / 5,091 | 19,338 / 3,253 / 5,898 |
| R1M, 8,000 | 50,983 / 2,789 / 6,425 | 35,365 / 3,332 / 7,506 |
| R3M, 1,000 | 15,000 / 3,635 / 6,524 | 18,753 / 6,745 / 10,286 |
| R3M, 2,000 | 23,558 / 4,530 / 6,807 | 31,527 / 8,804 / 18,381 |
| R3M, 4,000 | 28,429 / 4,438 / 7,357 | 36,416 / 7,225 / 18,079 |

Recall 0.8 and 0.9, k = 100 and 1,000, the 500k fits, parents and groups opened, and centres scored are in `tables.md`.

**The three questions:**
1. **Coarser parents cost the parents-only mode a lot.** At recall 0.99, items scanned rise 1.5–3.2× from 1,000 to 8,000 per cluster, at every N and k.
2. **Fixed-size sub-clusters make the parent size nearly irrelevant.** With sub-clusters of about 62, items scanned at 0.99 are flat or fall as parents grow. R1M text k = 1,000: 23,921 / 23,551 / 23,213 / 23,612. The price is centres scored: 1,000–17,000 per query at the best P, against K for parents only.
3. **The approximate mode grows sublinearly with N.** From R1M to R3M at 1,000 per cluster, with sub-clusters of about 62, items scanned at 0.99 rise 1.03–1.63× for 3× N (exponent about 0.03–0.44, two points).

At 0.99 only 2–12 groups are opened by the sub-clustered scheme, against about 63 for an exact text threshold query.

## The annideas claims

**How our numbers convert.** Our phase costs are profiled on this VM. To compare them with the 5900X:
- multiply by the wall/profiled ratio (0.81 for text);
- divide by 1.6–1.9, this VM's slowdown measured in `REPORT_DETLEVELS.md`.

| Claim | Status |
|---|---|
| "1,000-item clusters are too coarse" (approximate k-NN) | **Confirmed** (measured, in items scanned). Sub-clusters of about 62 scan 0.22–0.56× the items of 1,000-item parents at recall 0.99 (1.8–4.5× fewer), against annideas' 1.9–3.7×. Our item queries are corpus items and theirs were held out, so the absolute counts differ (img k10 at 0.99: 12,100 here against 7,095 there). |
| "Group setup is the floor whatever the cluster size" | **Confirmed in its first half, qualified in its second.** Group setup is 7–9 ms profiled per text query at every size and N (measured), so it does not depend on cluster size. It is a large share at small N (32% of text 0.01% at 500k) but only 9% at R3M. So it is not the floor at 3M and above. |
| Planner constant: 55 µs per group | **Consistent.** 105.8 µs profiled converts to 45–54 µs. |
| Planner constant: 1.6–7.8 µs per cluster | **Consistent at 1,000 per cluster.** The cut is 2.1 µs profiled per cluster (about 1.0 µs). An opened cluster at text 1% is 2.34 µs + 31.7 ns × about 364 units = 13.9 µs (6.4 µs). At 4,000 per cluster an opened cluster is about 7.5 µs, at the top of the range. |
| The combined estimate's 0.5 ms cluster setup for 120 parents | **Bracketed but not directly tested** (inferred). Our cluster-setup phase alone is 2.2 µs profiled per opened cluster (about 1.0 µs, so about 0.12 ms for 120 parents). With tables and level setup it is about 5 µs, so 0.6 ms. The exact path needs per-level tables; the approximate path's needs were not measured here. |

## Should `fit.py` use a constant size or one that depends on N?

**Measured:** 1,000 per cluster is never the best, at any N tested, for threshold queries or exact k-NN. In a few cells it is within noise of the best (500k item 0.1%; R1M item k-NN k = 10, 6.50 against 6.40 ms).

**Inferred:** a size that grows with N fits the evidence better than any constant:
- The penalty of large clusters on small item answers falls with N (R1M → R3M).
- Per-cluster work is a falling share as N grows.

The candidate rule is items per cluster ≈ 2,000 × √(N / 1M), on the 2,000 / 4,000 / 8,000 grid.

Evidence for it:
- R1M: 2,000 is best;
- R3M: 4,000 is best among 1,000 / 4,000 / 8,000, and against 2,000 it ties on threshold queries and is 16–51% faster on exact k-NN;
- the row-group 1M: 4,000 is best, and 2,000 is within 10–12%.

Evidence against it: two values of N on one population, and the population effect at fixed N (row groups 4,000, R1M 2,000) as large as the N effect. Which size is best at 3M also depends on the workload: 2,000 for selective item thresholds, 4,000 for exact k-NN.

If the approximate mode uses sub-clusters of a fixed size, the parent size can follow the exact path's needs without harming it (measured).

## Not done

- **5M:** replaced by R3M, for the distribution and memory reasons above.
- **The six-level reference run at the top N.**
- **Repeats at R3M:** 8,000 has one run only, and 1,000 and 4,000 have two.
- **Verify:**
  - every 500k, row-group 1M and R1M fit, and R3M at 2,000 and 4,000: 0 wrong in, 0 wrong out, 0 violations, 0 decisions not implied, k-NN 0 mismatches of 40 (measured);
  - R3M at 1,000 and 8,000: not run.
- **Probe counts at R3M 8,000.**
- **The planner:** not retuned for larger clusters. It sends fewer clusters straight to full vectors as clusters grow: 108 → 11 flat clusters at 1M text 0.01%, going from 1,000 to 4,000 per cluster.
- **Exponents in N:** each rests on two points of one population. None was fitted on three points.