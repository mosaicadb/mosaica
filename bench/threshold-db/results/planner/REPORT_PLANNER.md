# Threshold queries under a filter: planner and per-session compaction (15 Nov)

The question was whether LIMINAL's exact threshold query can be made never slower than its own plain f16 scan under a
visible set, and faster where it can. With the visible set compacted per session (codes and f16 vectors copied into a
block of their own), the planned query beats the plain scan at every answer share of the random 1%, 10% and 50% sets,
by 2–2.5× at 1% and by up to 2.4× at 50%. It does not yet hold everywhere: inside 19 genera (1%) it is 1.2–1.6× slower
than the plain scan, and at the largest answers under 241 and 964 genera it is 5–14% slower. Without compaction the
planner fixes the random 1% case and nothing else.

All answers were byte-identical to the plain scan's in every 1M run (1,575 compared per run, every variant), and the
50k exact check is clean.

## What changed

**Step 1, the planner** (`plan2` in `rust/src/v2/q.rs`).

- The query's nearest anchor is no longer searched in `prep2`. It is found on first use, so a query that scans never
  reads the 6 MB of anchors (0.1–0.28 ms), and when it is found it is the nearest among the clusters holding visible
  rows (any anchor gives sound bounds).
- Each live cluster gets one of four paths. Skip and take-in are decided by the support table alone, as before. A
  cluster left open is either scored directly from its visible rows' vectors (new, `Pre::Direct`), when that costs no
  more than its cut plus the cheaper of full vectors and the index for the candidates the cut would leave, or it is
  cut and run on the index or on full vectors as before. A choice of path never changes an answer.
- The whole query scans at three points: before anything is read (when the scan costs no more than cutting every
  live cluster, or than the anchor search with every live cluster at the cheaper of its cut and its rows); after the
  support table (when it costs no more than the open clusters' cheapest cut-or-rows); and after the sampled cut (when
  it costs no more than the estimate, which takes the cheaper of the plan with the index path, group setups included,
  and the plan without it, and scans on a tie).
- The cost model is the existing one (`Plan`, timed by `Plan::calibrate`) with four constants added and timed the same
  way: `cscan_us` (a scan of rows in order, which is what a compacted session or an unfiltered query reads; `scan_us`
  is now the scattered scan of a visible set alone), `anc_us` (the anchor search), `dir_us` (per cluster scored
  directly, beyond its rows; it times at 0 here), and `item_us2` (index path per candidate times the answer share,
  estimated from 256 sampled rows). `item_us2` is fitted at a 30% answer as well as at the 10th best score: candidates
  go deeper as the answer grows, and the old model, fitted at tiny answers only, could not see that. Under a visible
  set a candidate's vector read is costed at no less than `scan_us`, since it is as scattered as the scan's.
- `plan2_prev` keeps the previous planner behind the plan flag `prev`, so that the two could be timed interleaved.

**Step 2, compaction** (`Index2::compact` in `rust/src/v2/mod.rs`). A session's index is compacted to its visible
rows: each cluster keeps its visible rows in their order, with keys, codes, scalars and escapes copied into blocks of
their own, and optionally the f16 vectors. Everything that does not depend on which rows a cluster holds (bases,
tables, centres, anchors, support table, centre tables) is shared through `Arc`. Every bound a cluster stores holds for
any subset of its rows, so each row is decided exactly as in the parent. A compacted index is queried with no visible
set, so its candidates are contiguous, its blocks are dense, and its scan streams memory in order.

**Step 3, from the measurements.** The f16 rows are now 64-byte aligned (`F16Rows`). The index's own row-order copy
and the compacted copy came from a `Vec<u16>`, whose rows started 16 bytes off a cache line, and the compacted scan of
19 genera took 0.86 ms against 0.63 ms aligned. `dbcmp` now interleaves its variants per (query, share) and asserts that
every variant's answer equals the first's. The cores were shared with other benchmarks, and times drifted by up to 2×
between runs: the plain scan at random 1% measured 0.95–1.55 ms across runs on core 4.

## Results

1M BioCLIP-2, `dbcmp`, one thread pinned to core 4, the index opened on cores 6–7, 15 queries × 7 answer shares × 5
repetitions, medians per share over the queries. Variants were interleaved per query and share. The plan was
calibrated at open in each run (`runs/plan.txt` is the last). Random sets: `runs/dbcmp_s6r.csv`. Genus sets:
`runs/dbcmp_s5g.csv`, from the binary before the last scan rule (scan when the scan costs no more than cutting every
live cluster), which cannot fire on these sets. All tables: `tables_final.md`.

Rows: **scan** is LIMINAL's plain f16 scan (`flat`); **before** is the previous planner on the uncompacted index (what
the 14 Nov report calls LIMINAL); **planned** is the new planner, uncompacted; **planned, codes** has the codes
compacted and reads the vectors from the parent; **planned, codes+vectors** has both compacted; **compacted scan**
scans the compacted vectors. Bold marks where the planned, compacted query beats the plain scan.

**Random 1%** (10,000 visible)

| ms | 0.01% | 0.1% | 1% | 10% | 30% | 50% | 75% |
|---|---|---|---|---|---|---|---|
| scan | 1.11 | 1.14 | 1.10 | 1.07 | 1.17 | 1.17 | 1.25 |
| before | 1.06 | 1.07 | 1.07 | 1.11 | 1.19 | 1.23 | 1.25 |
| planned | 0.73 | 0.82 | 0.72 | 0.78 | 0.88 | 0.90 | 0.93 |
| planned, codes | 1.09 | 1.02 | 1.09 | 1.16 | 1.15 | 1.22 | 1.24 |
| planned, codes+vectors | **0.44** | **0.45** | **0.49** | **0.45** | **0.48** | **0.54** | **0.62** |
| compacted scan | 0.53 | 0.61 | 0.52 | 0.55 | 0.62 | 0.66 | 0.68 |

**Random 10%** (99,995 visible)

| ms | 0.01% | 0.1% | 1% | 10% | 30% | 50% | 75% |
|---|---|---|---|---|---|---|---|
| scan | 11.5 | 11.5 | 11.7 | 12.1 | 11.9 | 12.4 | 12.5 |
| before | 16.0 | 16.8 | 18.1 | 20.4 | 21.7 | 21.9 | 21.8 |
| planned | 16.1 | 17.4 | 18.5 | 18.5 | 15.5 | 14.6 | 14.6 |
| planned, codes | 12.8 | 13.6 | 13.6 | 13.9 | 13.9 | 13.8 | 13.9 |
| planned, codes+vectors | **9.66** | **10.1** | **10.2** | **10.2** | **10.3** | **10.4** | **10.3** |
| compacted scan | 9.37 | 9.36 | 9.41 | 9.61 | 9.67 | 9.74 | 9.76 |

**Random 50%** (499,975 visible)

| ms | 0.01% | 0.1% | 1% | 10% | 30% | 50% | 75% |
|---|---|---|---|---|---|---|---|
| scan | 47.4 | 47.3 | 47.3 | 49.2 | 50.7 | 51.4 | 51.5 |
| before | 30.2 | 33.4 | 42.3 | 57.9 | 67.3 | 71.3 | 70.3 |
| planned | 29.9 | 33.3 | 42.4 | 57.8 | 67.2 | 69.4 | 59.6 |
| planned, codes | 20.4 | 22.9 | 28.1 | 39.6 | 48.7 | 59.6 | 59.8 |
| planned, codes+vectors | **19.9** | **22.2** | **27.4** | **37.5** | **44.0** | **45.9** | **46.2** |
| compacted scan | 43.6 | 43.8 | 43.7 | 44.3 | 45.0 | 45.3 | 45.5 |

**19 genera, 1%** (10,154 visible in 159 clusters)

| ms | 0.01% | 0.1% | 1% | 10% | 30% | 50% | 75% |
|---|---|---|---|---|---|---|---|
| scan | 0.51 | 0.52 | 0.55 | 0.44 | 0.60 | 0.50 | 0.66 |
| before | 1.30 | 1.43 | 1.59 | 1.79 | 1.96 | 2.11 | 2.38 |
| planned | 0.88 | 0.89 | 0.90 | 1.09 | 1.19 | 1.33 | 1.30 |
| planned, codes | 0.72 | 0.81 | 0.99 | 0.94 | 0.98 | 1.14 | 0.97 |
| planned, codes+vectors | 0.65 | 0.70 | 0.76 | 0.78 | 0.80 | 0.97 | 0.98 |
| compacted scan | 0.77 | 0.75 | 0.63 | 0.85 | 0.67 | 0.82 | 0.79 |

**241 genera, 10%** (95,180 visible in 500 clusters)

| ms | 0.01% | 0.1% | 1% | 10% | 30% | 50% | 75% |
|---|---|---|---|---|---|---|---|
| scan | 7.32 | 7.35 | 7.42 | 7.50 | 7.87 | 8.14 | 8.28 |
| before | 7.72 | 7.92 | 8.88 | 10.8 | 12.2 | 13.4 | 12.8 |
| planned | 6.79 | 6.98 | 7.82 | 9.61 | 10.6 | 10.7 | 11.2 |
| planned, codes | 5.70 | 6.27 | 6.82 | 8.45 | 9.58 | 11.3 | 11.4 |
| planned, codes+vectors | **5.04** | **5.29** | **6.18** | 7.67 | 8.31 | 9.67 | 9.65 |
| compacted scan | 8.92 | 8.93 | 8.96 | 9.10 | 9.26 | 9.25 | 9.33 |

**964 genera, 50%** (494,151 visible in 824 clusters)

| ms | 0.01% | 0.1% | 1% | 10% | 30% | 50% | 75% |
|---|---|---|---|---|---|---|---|
| scan | 36.3 | 36.4 | 36.4 | 37.8 | 39.9 | 40.9 | 41.7 |
| before | 20.1 | 23.5 | 28.7 | 38.4 | 46.8 | 47.4 | 45.2 |
| planned | 19.3 | 22.6 | 27.6 | 37.8 | 45.1 | 45.1 | 44.4 |
| planned, codes | 17.1 | 19.7 | 23.3 | 32.5 | 41.3 | 52.9 | 53.4 |
| planned, codes+vectors | **17.0** | **18.8** | **23.1** | **32.1** | 38.3 | 46.7 | 46.8 |
| compacted scan | 43.8 | 44.2 | 44.3 | 44.8 | 45.5 | 46.0 | 46.7 |

### Reading the tables

- **Compacting the vectors is what wins.** A compacted session's scan streams its rows in order, and the index's
  candidates become contiguous ranges in dense blocks. With codes and vectors compacted, the planned query beats the
  plain scan at every share of every random set: 0.44–0.62 against 1.07–1.25 ms at 1%, 9.7–10.4 against 11.5–12.5 ms
  at 10%, and 20–46 against 47–52 ms at 50%. Under genera it wins up to answers of 10% of the corpus at 10% visible
  (5.0–7.7 against 7.3–7.5 ms) and up to 30% at 50% visible (17–38 against 36–40 ms).
- **Compacting the codes alone helps the index path but not the scan.** The rows' vectors are still read from the
  parent, scattered, so wherever the planner scans it costs what the uncompacted scan costs (1.1 ms at random 1%,
  13.8 ms at 10%).
- **Where it still loses.**
  - *19 genera.* The planned query takes 0.65–0.98 ms against 0.44–0.66 ms. The planner runs the index on 345 of 525
    runs, and the model favours it because `cscan_us` (0.10 µs per row, timed on a 1.5 GB scan) overstates a 15 MB
    scan, which runs at 0.06–0.08 µs per row when warm.
  - *Large answers under genera.* At 50–75% answers the planned query is 5–14% behind the plain scan (9.7 against
    8.3 ms at 10% visible, 46.8 against 41.7 ms at 50%).
  - *The plain scan against the compacted scan.* Under genera the plain scan beats the compacted scan, 36–42 against
    44–47 ms at 50% and 7.3–8.3 against 8.9–9.3 ms at 10%. The plain scan reads the vector file in its stored order,
    where 98% of a genus set's rows are adjacent, so it streams too. Both are contiguous and both use the same kernel,
    and the 20% gap is not explained. The random sets show the opposite: the compacted scan is 12–19% faster.
  - *Uncompacted, the planner wins only at random 1%* (0.72–0.93 against 1.07–1.25 ms), by not reading the anchors
    before a scan. At random 10% it is 1.2–1.6× slower than the scan, and at random 50% from the 10% share up. The
    index path over sparse clusters (100 visible rows in 1,000) costs more than the model says; the forced index path
    took 22 ms at random 1% and 42 ms at 10% (`runs/base_r.out`).
- **Before against planned, uncompacted.** The new planner is within 4% of the previous one on the uncompacted index
  where it is slower (random 10% at the smallest shares), and faster at random 1% (by a third), at random 10% from the
  30% share (by 29–33%), at the 75% share of random 50% (by 15%), and under 19 genera (by 32–45%).

## Cost of compaction

Built once per visible set on two threads (cores 6–7), from the session's row list. Memory is what the compacted index
holds of its own: codes and scalars padded to blocks of 32 per cluster, keys, entity ids and escapes, and with vectors
1,536 B per row.

| visible set | rows | codes: build | codes: memory | codes+vectors: build | codes+vectors: memory |
|---|---|---|---|---|---|
| random 1% | 10,000 | 12 ms | 9.1 MB | 22 ms | 24.4 MB |
| random 10% | 99,995 | 38 ms | 33.1 MB | 120 ms | 186 MB |
| random 50% | 499,975 | 162 ms | 147 MB | 576 ms | 913 MB |
| 19 genera | 10,154 | 4.3 ms | 4.2 MB | 10.6 ms | 19.7 MB |
| 241 genera | 95,180 | 34 ms | 30.1 MB | 111 ms | 176 MB |
| 964 genera | 494,151 | 172 ms | 145 MB | 539 ms | 902 MB |

The session's row list itself takes 0.2–9 ms. The codes come to about 290 B per row (276 B of code and scalars, plus
padding), and at random 1% the padding nearly triples them, because each cluster holds about 10 rows in a block of 32.
Codes and vectors at 50% visible cost half of what the whole index holds (1.8 GiB).

## Defaults

- The new planner, with direct scoring (`Plan::direct`), is on by default; `prev` and `nodirect` are plan flags.
- Compaction is an option and off by default: `Index2::compact(rows, vectors)` builds the session's index, and
  `dbcmp` measures it as `cindex` and `cvindex`. With vectors it costs 1.6 KB per visible row, so a reasonable default
  would turn it on, with vectors, for sets up to about 10% of the rows (under 190 MB and 0.12 s at 1M), and with codes
  only above that when memory is short. That switch is not built.

## Correctness

- **50k exact check** (`check.sh`, after the last change): 0 wrong, 0 violations in 30.5 million interval checks, 0 not
  implied, k-NN 400/400, oracle 0 wrong. The filtered part of `verify2` now also runs every threshold check on the
  index compacted to each visible set, with and without its vectors, under all three plans, and found none wrong. The
  interval-check count fell from 42.2 to 30.5 million because the planner now sends more filtered clusters to full
  vectors, where intervals are not checked.
- **1M:** every variant's answer equalled the plain scan's on every query and share (1,575 per run) in every run in
  `runs/`.

## Files

- `run.sh` (the measurement; `PLAN=calib` times the plan at open), `check.sh` (the 50k check, writing under this
  worktree's target), `table.py` (median tables from the CSVs), `tables_final.md`.
- `runs/`: `s6r` and `s5g` are the final runs; `s1g` to `s5r` are the steps between, `lazy1` is the lazy anchor alone,
  and `base_r` is the starting point (not interleaved, with the 13 Nov plan constants in `plan_r4.txt`).
