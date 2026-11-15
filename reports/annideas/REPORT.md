# Making LIMINAL's approximate k-NN faster: ranked ideas with measurements

2026-11-14. Research only. Tags: **[measured]** = a count from real data (1M BioCLIP-2, `index_r4`, the 150 benchmark queries or a stated subset); **[estimated]** = measured counts times the planner's per-item constants; **[speculative]** = argued, not measured. No new idea was timed; every millisecond figure for a new idea is an estimate built from counts.

## Summary

The approximate mode is slow for four reasons, in order of size, and none of them is the cluster order.

1. **The dial does nothing on the deterministic levels.** The fitted factors are 1.0 at levels 0–5, so the first 37 bytes prune by certified bounds, which are 14–17 times wider than the real error. Two thirds of the probed items reach the first randomised level. Applying the same kind of factor at every level cuts the per-item work about 3× and the bytes read 5–7× at unchanged recall. It needs no stored data.
2. **Clusters of 1,000 items are too coarse.** Splitting each cluster into 16 sub-clusters, probed in order of centre score + 0.2 × radius, reaches the same recall with 1.9–3.7× fewer items, at 0.8 and at 0.99. It costs about 1 MB in total and nothing per item.
3. **Overheads that are not search.** The k = 1,000 selection takes 61–75 ms of a 74–99 ms query. Building the probed clusters' visible set costs about 15 ns per probed item (1–2.8 ms at 64–256 clusters). The seed takes 0.9 ms. All three can go in approximate mode.
4. **Group setup becomes the floor.** A 0.99 query opens 20–40 of the 64 groups at 55 µs each (1.1–2.2 ms), whatever the cluster size.

Combined, my estimate is 0.3–1.1 ms at recall 0.8 and 1.4–6 ms at 0.99 for k = 10 to 1,000, against 1.3–22 ms and 6.9–68 ms today. That is level with RaBitQ at 0.8 and level or ahead at 0.99.

Three ideas came out weak: top-k through an exact threshold query, ordering items inside a cluster for early stopping, and flat 4,096 clusters. Expected-miss stopping is worth 1.5× in items at 0.99 for k ≥ 100 and nothing at 0.8.

## What the current mode does per query [measured]

From the instrumented run (`stat_run.out`, 15 queries, dial fitted for 0.99):

| k | clusters | recall | ms | seed | sweep | selection | unaccounted | items touched | reach 66 B | full vectors |
|---|---|---|---|---|---|---|---|---|---|---|
| 100 | 64 | 0.965 | 6.7 | 0.90 | 3.6 | 1.2 | 1.0 | 54,282 | 34,344 | 3,548 |
| 100 | 128 | 0.991 | 9.1 | 0.84 | 5.6 | 1.1 | 1.5 | 97,093 | 61,509 | 5,605 |
| 100 | 256 | 0.997 | 14.3 | 0.99 | 9.4 | 1.2 | 2.8 | 179,660 | 113,067 | 6,980 |
| 1,000 | 128 | 0.985 | 87 | 3.3 | 12.3 | 69.9 | 1.6 | 101,066 | 76,163 | 13,881 |
| 1,000 | 256 | 0.996 | 99 | 3.4 | 17.9 | 75.0 | 2.9 | 190,081 | 137,888 | 15,939 |

- The dial changes the counts only from the 66-byte level on (34,344 → 4,239 at the next level with it, 27,893 without). Levels 0–5 show the same counts with the dial on and off.
- "Unaccounted" is the time outside seed, sweep and selection. It grows with the items probed (1.0, 1.5, 2.8 and 12 ms at 54k, 97k, 180k and 694k items), so I read it as the centre ordering plus building the restricted visible set, at about 15 ns per probed item. That attribution is inferred from the scaling, not profiled.
- The planner's constants in that run: 7.8 ns per item per deterministic level, 31 ns per randomised level, 109 ns per full vector, 55 µs per group, 1.6–7.8 µs per cluster. They reproduce the measured sweep to within 10–20%, and every estimate below uses them.

## How good the stored codes are as estimates [measured]

I dumped every probed item's interval at levels 0, 1, 2, 3, 5, 6 and 7 for 50 queries (17 image, 33 text) over the first 256 clusters, and compared the midpoint with the true score.

| Level | Bytes | sd of (true − midpoint) | mean certified half-width | ratio |
|---|---|---|---|---|
| 0 | 6.5 | 0.0171 | 0.233 | 13.6 |
| 1 | 11.0 | 0.0142 | 0.207 | 14.6 |
| 3 | 20.0 | 0.0111 | 0.173 | 15.7 |
| 5 | 37.0 | 0.0079 | 0.135 | 17.0 |
| 6 | 66.5 | 0.0057 | 0.090 | 15.9 |
| 7 | 106.9 | 0.0051 | 0.043 | 8.4 |

The codes are good estimators: 37 bytes give an error sd of 0.008. The certified bound is what is loose. The true top-k items do sit in the upper tail (keeping 99.8% of them needs a factor of 0.4–0.5 on the half-width, not the 0.2 a Gaussian would suggest), so the usable gain is 2–3× in width, not 14×.

## Ranked ideas

Gain is at recall 0.99 unless stated. Effort is for the research prototype.

| # | Idea | Gain k = 10 / 100 / 1,000 | Extra memory | Effort | Evidence |
|---|---|---|---|---|---|
| 1 | Dial on every level (factor × certified half-width at levels 0–5 too) | per-item work 2.8× / 3.1× / 3.1× less; bytes 5–7× less; about 2× on the whole query | none | small | measured counts |
| 2 | Approximate-mode overheads: heap selection, no seed, cluster list in place of a restricted visible set | −2 to −3 ms / −2 to −3 ms / −60 to −75 ms | none | small | measured times |
| 3 | 16 sub-clusters per cluster, ordered by centre score + 0.2 × radius, two-level probing | items 3.1× / 2.8× / 2.1× fewer at 0.99; 3.7× / 2.6× / 1.9× at 0.8 | ~1 MB total | medium (row order changes) | measured counts |
| 4 | Cheaper group setup in approximate mode (only the basis rows the read levels use) | −0.5 to −1.5 ms | none | medium | estimated |
| 5 | Stop on expected miss: stop when the next key < running k-th score − δ | items 1.2× / 1.6× / 1.6× fewer at 0.99; none at 0.8–0.9 | none | small | measured, cross-fitted |
| 6 | One fused fast-scan pass over levels 0–3 with a single integer threshold | per-item work a further ~2× | none | medium | speculative |
| 7 | int8 centres for the ordering, in place of float64 | −0.3 to −0.5 ms | none (they exist) | small | estimated |
| 8 | SOAR spill | not re-measured (prior work) | 4+ B/item | — | prior reports |
| – | Top-k by an exact threshold query at an estimated k-th score | none by itself | none | — | measured, negative |
| – | Order items inside a cluster by key for early stopping | none | none | — | measured, negative |
| – | Flat 4,096 clusters in place of sub-clusters | worse than idea 3 | — | — | measured |

### 1. Dial on every level [measured counts, estimated time]

Rule simulated: at each level, drop an item when midpoint + f_level × half-width is below the k-th score. The factors are fitted to lose a share eps of the true top-k per level on half the queries, and run on the other half. Items entering each level, 128 clusters probed, k = 100:

| | L0 | L1 | L2 | L3 | L5 | L6 (66 B) | L7 | full | recall | item work | bytes |
|---|---|---|---|---|---|---|---|---|---|---|---|
| certified at every level | 106,785 | 76,111 | 70,789 | 66,187 | 62,375 | 51,826 | 36,073 | 7,258 | 0.989 | 7.0 ms | 16.8 MB |
| eps 0.002 | 106,785 | 42,425 | 33,169 | 23,861 | 20,152 | 7,702 | 1,937 | 469 | 0.988 | 2.3 ms | 2.5 MB |
| eps 0.01 | 106,785 | 35,741 | 24,566 | 16,576 | 11,690 | 4,029 | 989 | 357 | 0.985 | 1.8 ms | 1.9 MB |

The same pattern holds at k = 10 (5.8 → 2.0 ms, 9.5 → 1.8 MB) and k = 1,000 (9.3 → 3.0 ms, 35 → 6.5 MB). The fitted factors run from about 0.5 at level 0 to 0.3 at level 6 for k = 10, and from 0.38 to 0.23 for k = 1,000.

- The simulation prunes against the true k-th score. With a k-th score taken from the first four clusters only, cost is unchanged for k ≤ 100 and 40–90% higher for k = 1,000. So k = 1,000 needs the running score, not a one-off seed.
- The fitted dial has factor 1.0 at levels 0–5, which I take to mean the fit records no errors for the integer levels (I did not trace this in the code). The change is to record them and scale the integer thresholds.
- Level 0 alone removes 55–70% of items. About 80–90% are gone before level 5 and 93–96% before the 66-byte level.

### 2. Overheads [measured]

- **Selection at k = 1,000** takes 61–75 ms for 10,000–14,000 candidates. In approximate mode the survivors can be scored exactly (2,200–2,900 full vectors after idea 1, about 0.3 ms) and the best k kept in a heap.
- **Restricted visible set**: about 15 ns per probed item, 1.0–2.8 ms. Passing the cluster list removes it.
- **Seed**: 0.9 ms (3.3 ms at k = 1,000). Best-first probing gives a running k-th score after the first few clusters.

### 3. Sub-clusters [measured, all 150 queries]

Mean items scanned to reach mean recall 0.8 / 0.9 / 0.99, assuming a perfect re-rank:

| Structure, order | k = 10 | k = 100 | k = 1,000 |
|---|---|---|---|
| 1,000 clusters, centre score | 9,198 / 30,405 / 195,456 | 9,198 / 28,487 / 212,709 | 15,917 / 44,455 / 245,971 |
| 1,000 clusters, + 0.2 radius (today) | 4,740 / 11,922 / 80,606 | 5,621 / 15,463 / 102,582 | 11,922 / 30,227 / 160,149 |
| flat 4,096, + 0.2 radius | 2,263 / 5,116 / 46,242 | 3,141 / 7,651 / 63,964 | 8,276 / 19,461 / 106,876 |
| 4 sub-clusters each, + 0.2 radius | 2,316 / 6,042 / 42,155 | 3,479 / 8,531 / 62,910 | 8,531 / 19,949 / 109,518 |
| 16 sub-clusters each, + 0.2 radius | 1,271 / 3,273 / 25,889 | 2,163 / 5,104 / 36,997 | 6,223 / 13,952 / 76,158 |

- The radius bonus matters as much as the split: without it the 16-way split gains nothing at 0.99 (75,524 items at k = 10). A sub-cluster's radius is √(1 − |centre|²) for unit vectors, so it needs no extra pass.
- **Two-level probing** keeps centre scoring cheap. Ranking the 1,000 parents and then the sub-centres of the best 256 (5,096 centres scored) gives the same items as ranking all 16,000. The best 128 parents reach 0.99 for k ≤ 100 but need 58,000 items at k = 100 in place of 37,000. 64 parents cannot reach 0.99.
- A sub-centre can be stored as one more coded row of its parent and scored by the same fast-scan kernel, so 16 of them cost about what 16 items cost: about 0.7 MB at 37 B each [speculative]. Storing int8 sub-centres instead is 12 MB.
- **Cost to the layout.** Rows inside a cluster are sorted by key today, and sub-clusters need rows sorted by (sub-cluster, key). The exact path's one sorted range per cluster becomes up to 16. This is the one place the idea touches the exact path; see the ruling below, which keeps the exact layout.
- Text queries set the 0.99 figure: at k = 10 they need 30,947 items against 3,680 for image queries.
- At 0.99 a query still opens 87–228 parents and 20–40 groups, the same as today, so sub-clusters cut item work and not setup.

### 4. Cheaper group setup [estimated]

Groups opened at 0.99: 20 (k = 10), 25 (k = 100), 37–40 (k = 1,000), at 55 µs each for the 768 × 768 int8 product. After ideas 1–3 this is the largest single cost. Levels 0–3 hold 160 bits, so they use at most 160 of the 768 coordinates, and the 66-byte level codes one block of 96. A product over only the rows those levels use is about a third of the work. The half-width terms for uncoded blocks would need replacing with a fitted error scale per level; I did not check that in the code. [speculative]

### 5. Expected-miss stopping [measured, cross-fitted]

Rule: stop when the next cluster's key is below the running k-th score minus δ, with δ fitted on half the queries. On 16-way sub-clusters with the radius bonus, items at 0.99:

| k | fixed probe count | stop rule (in-sample) | cross-fit halves: recall / items | per-query oracle |
|---|---|---|---|---|
| 10 | 25,889 | 21,944 | 0.996 / 29,401 and 0.987 / 20,257 | 5,886 |
| 100 | 36,997 | 23,298 | 0.993 / 25,658 and 0.990 / 20,938 | 18,207 |
| 1,000 | 76,158 | 46,672 | 0.994 / 50,095 and 0.989 / 35,369 | 43,994 |

- At k ≥ 100 the rule is close to the per-query oracle and transfers across halves. At k = 10 it is noisy with 75 queries per half, and at k = 1 it does not transfer (recall 0.97–1.0 at 10,000–114,000 items).
- At 0.8 and 0.9 it scans slightly more than a fixed count, so a fixed count is the better dial there.
- Scaling the margin by the cluster radius (the per-cluster summary variant) did no better than putting the radius bonus in the order.
- A threshold frozen after the first few probes ranges from equal to 30% worse than the running one.

### 6. Fused fast-scan pass [speculative]

Even with idea 1, levels 0–3 are four passes at 7.8 ns per item each, and they are about 80% of the remaining item work. Fast-scan kernels elsewhere run about 1 ns per item for 32 bytes. One pass over the 20-byte prefix with one integer threshold per block, written for approximate mode only, should be 2–4 ns per item. Not measured.

### Negative results [measured]

- **Top-k by an exact threshold query at an estimated k-th score.** The exact mode already works this way, and the certified path is what costs. With the true k-th score as threshold, certified pruning does 3× the item work of the dialled cascade (7.0 against 2.3 ms above), and the certified cut skips little (693,569 of 999,950 items touched with every cluster allowed). An estimated threshold helps only together with ideas 1 and 3, where it is the running k-th score.
- **Ordering inside a cluster.** Keys inside a cluster have an sd of 0.026. The certified key bound prunes 12–16% of probed items at 0.99 settings. An expected-miss version loses answers as fast as it prunes: at k = 10 a factor of 0.3 prunes 84% and loses 45% of the top 10; at k = 1,000 it prunes 52% and loses 2%. Level 0 does this job far better for 6.5 bytes.
- **Flat 4,096 clusters** scan 1.4–1.8× more than 16-way sub-clusters and would replace the cluster structure.

## Recommended first step

Ideas 1 and 2 together, on the existing 1,000 clusters: fit and apply the dial at levels 0–5, score survivors exactly into a heap, and drop the seed and the restricted visible set. Nothing stored changes, the exact path is untouched, and the existing `dbtopk` ladder tests it. Expected at recall 0.99 [estimated]: k = 10 and 100 from 6.9 and 9.2 ms to about 3.5 ms; k = 1,000 from 68 ms to 8–12 ms. Idea 3 comes next, with the row order ruled below.

## Ruling on row order (15 November 2026)

Rows stay sorted by key inside each cluster, and the exact path is unchanged. Sub-clusters belong to the approximate
mode: each cluster stores the local positions of its rows grouped by sub-cluster (u16, 2 B per item for clusters under
65,536 rows), with each sub-cluster's start, centre and radius. The approximate mode takes the probed sub-clusters of
a cluster as one sorted list of positions and gathers those rows, so setup and tables are still paid once per cluster.
Sub-clusters need not be balanced, since nothing has to align to blocks.

- **The exact path would not gain from sub-clusters.** A K-way partition lowers the half-width floor by at most
  K^(−1/k) (`groups/REPORT.md`, Proposition 1), and the measured member radius falls as K^(−0.053): 16 sub-clusters
  narrow the member side by 14% and 64 by 20%, and the query side not at all. A hierarchy over centres read 129% of
  the leaf centres for text (`sublinear/REPORT.md` 5.6). [proved and measured]
- **Ordering rows by sub-cluster would cost the exact path.** One range search per sub-list in every opened cluster,
  and the first level loses in-place block reads when the candidates are sparse: about 1–5% of query time at 1M.
  [estimated from the profiled breakdown in `rust/REPORT_CLUSTERSIZE.md`]
- **Gathering costs the approximate mode little.** It reads far fewer rows: 25,889–76,158 at recall 0.99 for k = 10 to
  1,000 in this report's 16-way simulation, and 5,000–16,000 on the scaling run's queries at 1M. At most 2–3 ns more
  per gathered row, that is 0.05–0.2 ms, a few per cent of the combined estimates below. Deeper levels are read for
  scattered survivors in either layout. [estimated]
- **With sub-clusters of about 62 items, the items scanned no longer depend on cluster size**: within ±25% between
  1,000 and 4,000 items per cluster at 1M, against 1.5–4.5 times more items without them. So the exact path can set the cluster size
  (`rust/REPORT_CLUSTERSIZE.md`). [measured counts, scaling run; report to follow]

Not yet measured: the approximate mode's time with gathered sub-clusters.

## What a combined mode could reach [estimated]

Counts are measured; times are counts × the planner constants plus setup. "Combined" is ideas 1, 2, 3 and 7; the lower end of each range adds ideas 4–6. The comparison columns are from the top-k report.

| k | recall | today (ms) | step 1 only | combined | RaBitQ | faiss IVF4096-SQfp16 |
|---|---|---|---|---|---|---|
| 10 | 0.8 | 1.33 | ~0.7 | 0.3–0.4 | 0.37 | 0.45 |
| 100 | 0.8 | 3.35 | ~0.9 | 0.4–0.5 | 0.43 | 1.29 |
| 1,000 | 0.8 | 22.4 | ~2 | 0.9–1.1 | 1.14 | 1.99 |
| 10 | 0.99 | 6.86 | ~3.5 | 1.4–2.1 | not reached (0.987 at 9.8) | 5.96 |
| 100 | 0.99 | 9.18 | ~3.5 | 1.5–2.9 | 2.88 | 12.1 |
| 1,000 | 0.99 | 68.4 | 8–12 | 3.5–6 | 4.07 | 12.3 |

The k = 100, 0.99 row in detail: 37,000 items in 120 parents and 25 groups. Centre scoring 0.1 ms, groups 1.4 ms, cluster setup 0.5 ms, item cascade 0.8 ms, 360 full vectors and the heap 0.1 ms: 2.9 ms. Expected-miss stopping takes the items to 23,000 (−0.3 ms), the partial basis product takes the groups to about 0.5 ms, and a fused pass halves the item work, which gives about 1.5 ms.

**Memory.** About 1 MB in total for sub-centres and sub-cluster boundaries. Nothing per item.

**On disk.** Bytes read per query fall from 9–35 MB to 1.4–6.5 MB at 0.99 on today's clusters, and by about a further third with sub-clusters. Code reads stay contiguous per sub-cluster and level. Full-vector reads, which are random, are about 70–100, 360–480 and 2,200–2,900 at k = 10, 100 and 1,000. At k = 1,000 they are most of the bytes (about 4 MB). Passing survivors through the 160- and 256-byte levels first should cut them, but those levels were not dumped.

## Limits of this evidence

- One dataset; 150 queries (50 image, 100 text) for the probing counts and 50 for the level counts. Probe counts were tuned on the same queries, as in the top-k report. Only the dial factors and the stop rule were cross-fitted.
- No new idea was timed. The estimates stand on the planner's constants.
- Nothing was measured under a visible-set filter. The cascade and sub-cluster probing read visible rows only, so they should carry over, but the 1% figures are untested.
- The sub-clusters are plain k-means inside each cluster (8 iterations), not balanced.
- The cascade simulation applies the sub-cluster order with all 16,000 sub-centres ranked; two-level probing was checked separately on item counts only.
- SOAR was not re-run.

## Files

All in `/tmp/claude-1000/-home-joe-code-tessera/44285a24-9b9a-44a0-8e3a-604c4b51c0e9/scratchpad/annideas/`:

- `stat_run.out`: per-phase and per-level counts of the current mode (from the previous agent).
- `sim_probe.out`, `sim_probe2.out`, `sim_touch.out`: items scanned by structure and order, two-level probing, stop rules, key pruning, parents and groups opened.
- `sim_levels.out`, `sim_cascade.out`: estimator error by level, re-rank depths, the dialled cascade.
- `prep_sim.py`, `sim_probe.py`, `sim_probe2.py`, `sim_touch.py`, `sim_levels.py`, `sim_cascade.py`: the scripts.
- `rustcopy/`: the Rust copy with `--stat-np` and `--dump-dir` added to `dbknn`. `dump/` (855 MB of dumped intervals) and `sim/` (scores and clusterings) can be deleted.