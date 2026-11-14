# Cache lines, not bytes: testing a column-then-row layout for the code stream

The hypothesis: DRAM moves 64-byte lines, so byte counts understate the traffic of scattered row reads and a column
read wastes lines once survivors are sparse; so store the early levels as columns in blocks of 64 items, switch a block
to row reads (one aligned 64 B line per step) once its survivors fall below about 1 in 64, then read the remainder exactly.

**Verdict.** The premise holds; the proposal as stated does not.

- Rows do cost far more lines than bytes: a text query at a 0.01% answer needs 12.1 stream bytes per item, which is
  43 B of lines for tight rows and 34 B for 64 B-aligned rows (66 B with adjacent-line pairs), against 23 B for columns.
  Per line, scattered row reads also run at about half the bandwidth (rows 6–11 GB/s, columns 13–22 GB/s).
- With every item visible, columns waste few lines: the survivors of a cluster stay clustered in sort order, so a
  64-item block dies out nearly at once. Columns read 1.9–2.1x the needed bytes at 0.01–1% answers.
- **A switch at 1 in 64 does almost nothing:** it saves 6–10% of col64's lines and lands on the layout already built.
  The best switch is at **4–8 of 64** (1/16–1/8) by lines and time. Even there it saves only 15–20% of lines and 0–13%
  of time against the layout already built, within the run-to-run noise at 0.01–0.1%, and nothing at the 50M placement.
- **Visibility, not survivor density, is what decides the layout.** With 10% of items visible at random, columns read
  4–10x the lines of rows, and rows are 2–5x faster at 1M (1.3–3.4x at the 50M placement). A block rule that sends
  blocks with at most 16 of 64 live items to rows matches rows exactly there.
- **The current layout is not row-wise.** The native v2 index already stores each level as its own array of 32-item
  blocks of 4-bit rows (`src/v2/mod.rs` `setnib`, `src/v2/kern.rs` `gather_rows`): it is a columnar layout, and
  re-packing still reads every line of a survivor's source block. Comparator (a) below is the row layout as described
  in the brief; the layout as built is the row **col32**.
- **64 B steps beat 128 B steps** in time everywhere, although adjacent-line prefetch makes their pair traffic equal.
- **Memory is a minority of the prototype's time.** Replaying col32's reads with a nibble decode costs 4.7 ms for a
  text query at a 1% answer; the native prototype spends about 57 ms on that query at one thread
  (`rust/results/combined/breakdown/s4.csv`: tables 8, re-pack 6, decode 8, bounds and decisions 14 ms). A layout change
  moves at most a few ms of that unless the per-item compute shrinks too.

**What I would do instead:** keep columns, and choose rows **per block from the number of live items**, at the start
(visible and inside the sorted range) and after each level: rows when at most 8–16 of 64 are live. That needs the
early bytes stored twice (columns plus rows); the 64 B row stride costs 320 B per item against the 285 B stream, so
the double copy of the whole stream is 605 B per item, or 404 B with columns only to 84 B. For comparison, the
planner's scan of visible full vectors reads 154 B of lines per item of N at 10% visible, against 3–20 B for the code
read as rows.

## Method

1. **Demand, exact, from the native harness.** `vexact ... demand` (added to `rust/src/run2.rs`; a hook in
   `rust/src/v2/q.rs` writes, for every row the level walk reads, the deepest level read, and marks rows sent to
   their full vector). Build as in `rust/results/combined` stage 5: dual basis, data-only fit, `--int --gtab --flags
   repack,nusort,pscan`, `--plan index` (so every opened cluster takes the index path), 1M BioCLIP-2. 20 text and 20
   item queries, thresholds at answers of 0.01, 0.1, 1, 10 and 50%. Bytes read per item agree with the breakdown
   (text 12.2 / 20.5 / 63.3 / 190 / 345 B including full vectors, against 63.7 at 1% in s4). The cumulative stream is
   8.5 (with the 4 B sort key), 13, 17.5, 22, 30.5, 39, 43 B (deterministic), then 60.8 … 284.9 B over 13 randomised
   levels; full vectors are 1,536 B.
2. **Visibility.** "10%" keeps a random 10% of rows (one fixed mask) and leaves the threshold at the unfiltered
   answer. Each decision depends only on the item and the threshold, so the filtered demand is the unfiltered one
   restricted to visible rows.
3. **Lines, exact** (`lines.c` count mode): distinct 64 B lines, distinct 128 B pairs (adjacent-line prefetch), and
   runs of consecutive lines, per query, for each layout. Row layouts read depth-first per item; column layouts walk
   level by level and read a block's columns for a level when any of its items is still undecided. Full-vector reads
   are identical across layouts and listed apart.
4. **Time** (`lines.c` time mode): the same reads replayed in order over a buffer of the layout's size filled with
   random bytes, each 32 B vector decoded with two pshufb lookups (row reads also sum and compare per item, column
   reads per block). L3 flushed (64 MB stream) before each run; each figure is the better of a plain and a
   software-prefetched replay, median of 5 runs, then median over 10 queries. One thread on CPU 10 (a neighbour was
   running benchmarks on CPU 4 and builds on up to 6 cores). 1M: dense buffer, transparent huge pages. 50M placement:
   each cluster's regions at random 4 KB-aligned slots spread over 50x the space, 4 KB pages, so TLB misses resemble
   a 50M index.
5. **Noise.** A repeat of the 1M text runs on CPU 11 under load 7.5 moved individual cells by up to ±15%
   (`results/time_tables.txt`, `time1rep_text`); the rankings held.

**Layouts.** W = 285 B of stream per item.
- (a) *rows, tight*: stride W, an item reads bytes [0, d).
- (b) *rows, 64 B steps*: stride 320 B, aligned; an item reads ceil(d/64) lines. This assumes a code in 64 B steps
  decides with the same bytes; most touched items need only the first line, so it is not sensitive to that.
- *rows, 128 B steps*: stride 384 B, ceil(d/128) pairs.
- *col32 (as built)*: per level, blocks of 32 items, one line holds 2 bytes of 32 items.
- (c) *col64*: blocks of 64 items, one line holds one byte of 64 items.
- (d) *hybrid*: col64 until a block has at most k live items at the start of a level, or the level ends past C bytes;
  then each live item reads its own row (whole stream stored as rows too) from the line holding the level's first
  byte, in steps of S. "Fixed split" is k = 0, C = 43.

## Lines touched per item of N (B; lines x 64 / N; in brackets 128 B pairs x 128 / N)

**text queries, visible all** — line bytes per item of N: distinct 64 B lines x 64 / N (in brackets: distinct 128 B pairs x 128 / N)

| Layout | 0.01% | 0.1% | 1% | 10% | 50% |
|---|---|---|---|---|---|
| *stream bytes needed* | 12.1 | 19.4 | 52.5 | 107.7 | 161.1 |
| (a) rows, tight, growing levels | 43.4 (75.2) | 58.9 (98.9) | 107.4 (162.6) | 164.3 (218.0) | 211.1 (254.4) |
| (b) rows, 64 B steps | 34.3 (66.1) | 46.1 (86.2) | 86.6 (142.6) | 143.7 (201.7) | 195.4 (247.1) |
| rows, 128 B steps | 64.1 (64.1) | 81.5 (81.5) | 124.1 (124.1) | 179.6 (179.6) | 232.5 (232.5) |
| col32 (as built) | 22.7 (23.7) | 35.8 (37.1) | 99.5 (101.1) | 193.9 (195.0) | 265.1 (265.5) |
| (c) col64, all columnar | 25.4 (25.9) | 39.6 (40.3) | 109.7 (110.5) | 206.6 (207.1) | 273.6 (273.8) |
| (d) fixed split: col to 43 B, then rows/64 | 22.9 (26.5) | 36.3 (43.8) | 94.4 (121.9) | 172.4 (216.3) | 237.7 (287.4) |
| (d) switch at <=1 of 64 | 22.8 (23.6) | 35.7 (36.7) | 98.6 (99.9) | 194.3 (195.3) | 267.3 (267.7) |
| (d) switch at <=4 of 64 | 20.3 (21.9) | 31.9 (34.0) | 86.0 (89.0) | 176.5 (179.0) | 255.1 (256.3) |
| (d) switch at <=8 of 64 | 19.4 (22.2) | 30.4 (34.0) | 79.5 (84.8) | 164.0 (169.1) | 243.4 (246.5) |
| (d) switch at <=16 of 64 | 19.5 (24.7) | 30.3 (37.1) | 75.3 (85.8) | 151.1 (162.7) | 225.5 (235.3) |
| (d) switch at <=8, rows/128 | 21.4 (21.9) | 33.0 (33.6) | 84.0 (84.9) | 168.8 (169.6) | 246.7 (247.1) |
| (d) switch at <=8, col only to 84 B | 19.2 (22.1) | 29.9 (33.9) | 73.6 (85.5) | 141.8 (172.7) | 206.0 (256.0) |
| *full vectors (all layouts)* | 0.3 | 1.3 | 11.0 | 82.3 | 184.5 |

**text queries, visible 10%** — line bytes per item of N: distinct 64 B lines x 64 / N (in brackets: distinct 128 B pairs x 128 / N)

| Layout | 0.01% | 0.1% | 1% | 10% | 50% |
|---|---|---|---|---|---|
| *stream bytes needed* | 1.2 | 1.9 | 5.2 | 10.7 | 16.0 |
| (a) rows, tight, growing levels | 4.3 (7.5) | 5.9 (9.9) | 10.8 (16.4) | 16.9 (23.1) | 22.1 (28.3) |
| (b) rows, 64 B steps | 3.4 (6.6) | 4.6 (8.6) | 8.6 (14.3) | 14.3 (20.5) | 19.4 (25.6) |
| rows, 128 B steps | 6.4 (6.4) | 8.1 (8.1) | 12.3 (12.3) | 17.9 (17.9) | 23.1 (23.1) |
| col32 (as built) | 15.1 (16.1) | 24.2 (25.4) | 65.5 (67.2) | 135.4 (137.0) | 200.8 (202.0) |
| (c) col64, all columnar | 18.0 (18.5) | 28.6 (29.3) | 78.4 (79.2) | 161.3 (162.0) | 235.6 (236.0) |
| (d) fixed split: col to 43 B, then rows/64 | 14.5 (15.3) | 20.9 (22.0) | 39.7 (42.7) | 54.5 (59.4) | 63.1 (69.1) |
| (d) switch at <=1 of 64 | 14.6 (15.4) | 23.5 (24.5) | 63.0 (64.4) | 131.9 (133.5) | 199.3 (200.6) |
| (d) switch at <=4 of 64 | 9.3 (11.0) | 14.7 (17.0) | 37.8 (41.2) | 74.4 (78.4) | 109.7 (113.9) |
| (d) switch at <=8 of 64 | 4.5 (7.4) | 6.5 (10.2) | 14.2 (19.4) | 25.2 (31.1) | 35.1 (41.1) |
| (d) switch at <=16 of 64 | 3.4 (6.6) | 4.6 (8.6) | 8.6 (14.3) | 14.3 (20.6) | 19.4 (25.6) |
| (d) switch at <=8, rows/128 | 7.1 (7.2) | 9.6 (9.7) | 17.7 (17.9) | 29.1 (29.3) | 39.4 (39.6) |
| (d) switch at <=8, col only to 84 B | 4.5 (7.4) | 6.5 (10.1) | 14.0 (19.2) | 23.5 (29.4) | 31.3 (37.4) |
| *full vectors (all layouts)* | 0.0 | 0.1 | 1.1 | 8.3 | 18.4 |

**item queries, visible all** — line bytes per item of N: distinct 64 B lines x 64 / N (in brackets: distinct 128 B pairs x 128 / N)

| Layout | 0.01% | 0.1% | 1% | 10% | 50% |
|---|---|---|---|---|---|
| *stream bytes needed* | 1.4 | 2.4 | 8.5 | 86.6 | 144.4 |
| (a) rows, tight, growing levels | 4.6 (7.9) | 7.5 (12.5) | 25.3 (42.3) | 144.3 (200.6) | 195.3 (240.7) |
| (b) rows, 64 B steps | 3.8 (7.1) | 6.2 (11.4) | 20.9 (38.0) | 122.7 (181.7) | 178.7 (231.1) |
| rows, 128 B steps | 6.7 (6.7) | 10.8 (10.8) | 36.3 (36.3) | 159.2 (159.2) | 215.1 (215.1) |
| col32 (as built) | 2.9 (3.0) | 4.7 (4.9) | 16.2 (16.8) | 169.7 (171.0) | 249.6 (250.1) |
| (c) col64, all columnar | 3.3 (3.4) | 5.3 (5.4) | 18.0 (18.3) | 183.7 (184.3) | 258.6 (258.8) |
| (d) fixed split: col to 43 B, then rows/64 | 2.9 (3.6) | 4.7 (5.6) | 15.6 (18.6) | 144.7 (184.1) | 216.7 (264.0) |
| (d) switch at <=1 of 64 | 2.9 (3.0) | 4.7 (4.9) | 16.2 (16.7) | 168.9 (170.0) | 251.7 (252.1) |
| (d) switch at <=4 of 64 | 2.5 (2.7) | 4.2 (4.5) | 14.4 (15.3) | 148.8 (151.7) | 238.5 (239.8) |
| (d) switch at <=8 of 64 | 2.3 (2.7) | 4.0 (4.5) | 13.6 (15.1) | 136.1 (141.7) | 225.6 (229.1) |
| (d) switch at <=16 of 64 | 2.3 (2.9) | 3.9 (4.8) | 13.3 (16.1) | 124.0 (136.2) | 205.9 (216.5) |
| (d) switch at <=8, rows/128 | 2.6 (2.7) | 4.3 (4.4) | 14.7 (15.0) | 141.4 (142.2) | 229.2 (229.7) |
| (d) switch at <=8, col only to 84 B | 2.2 (2.6) | 3.7 (4.4) | 12.7 (15.1) | 117.0 (140.1) | 186.1 (230.2) |
| *full vectors (all layouts)* | 0.2 | 0.8 | 2.7 | 48.3 | 151.7 |

**item queries, visible 10%** — line bytes per item of N: distinct 64 B lines x 64 / N (in brackets: distinct 128 B pairs x 128 / N)

| Layout | 0.01% | 0.1% | 1% | 10% | 50% |
|---|---|---|---|---|---|
| *stream bytes needed* | 0.1 | 0.2 | 0.9 | 8.6 | 14.3 |
| (a) rows, tight, growing levels | 0.5 (0.8) | 0.8 (1.3) | 2.6 (4.3) | 14.6 (20.8) | 20.4 (26.4) |
| (b) rows, 64 B steps | 0.4 (0.7) | 0.6 (1.1) | 2.1 (3.8) | 12.2 (18.3) | 17.7 (23.9) |
| rows, 128 B steps | 0.7 (0.7) | 1.1 (1.1) | 3.6 (3.6) | 15.8 (15.8) | 21.4 (21.4) |
| col32 (as built) | 1.8 (2.0) | 3.1 (3.3) | 10.8 (11.4) | 111.1 (112.8) | 183.7 (184.9) |
| (c) col64, all columnar | 2.2 (2.3) | 3.8 (3.9) | 12.9 (13.2) | 134.5 (135.3) | 217.8 (218.2) |
| (d) fixed split: col to 43 B, then rows/64 | 1.5 (1.6) | 2.3 (2.4) | 7.9 (8.4) | 50.1 (54.5) | 59.7 (65.4) |
| (d) switch at <=1 of 64 | 1.8 (1.8) | 3.0 (3.2) | 10.4 (10.9) | 106.3 (107.9) | 180.7 (182.0) |
| (d) switch at <=4 of 64 | 1.1 (1.3) | 1.8 (2.1) | 6.3 (7.3) | 59.0 (63.0) | 97.2 (101.3) |
| (d) switch at <=8 of 64 | 0.5 (0.8) | 0.9 (1.3) | 2.9 (4.4) | 20.6 (26.3) | 31.3 (37.2) |
| (d) switch at <=16 of 64 | 0.4 (0.7) | 0.6 (1.1) | 2.1 (3.8) | 12.2 (18.3) | 17.7 (23.9) |
| (d) switch at <=8, rows/128 | 0.8 (0.8) | 1.3 (1.3) | 4.2 (4.3) | 24.3 (24.4) | 35.5 (35.7) |
| (d) switch at <=8, col only to 84 B | 0.5 (0.8) | 0.8 (1.3) | 2.8 (4.4) | 19.7 (25.5) | 28.6 (34.5) |
| *full vectors (all layouts)* | 0.0 | 0.1 | 0.3 | 4.8 | 15.1 |

## Time per query (ms; at 1M this is also ns per item of N)

**text queries, visible all** — ms per query at 1M (at 50M-scale placement in brackets); median of 10 queries

| Layout | 0.01% | 0.1% | 1% | 10% | 50% |
|---|---|---|---|---|---|
| (a) rows, tight, growing levels | 4.54 (5.62) | 6.96 (7.78) | 8.85 (11.83) | 11.10 (13.23) | 11.96 (13.63) |
| (b) rows, 64 B steps | 3.32 (4.98) | 4.90 (6.73) | 8.75 (11.01) | 11.75 (13.85) | 13.04 (15.05) |
| rows, 128 B steps | 4.96 (7.94) | 7.03 (9.97) | 10.27 (13.66) | 13.21 (15.62) | 15.81 (16.47) |
| col32 (as built) | 1.33 (2.16) | 2.12 (3.18) | 4.73 (7.11) | 9.58 (13.86) | 13.76 (19.13) |
| (c) col64, all columnar | 1.59 (1.86) | 2.54 (2.89) | 5.24 (7.18) | 9.80 (13.74) | 13.88 (18.92) |
| (d) fixed split: col to 43 B, then rows/64 | 1.33 (1.94) | 2.62 (3.69) | 6.39 (8.97) | 11.31 (14.51) | 14.50 (17.10) |
| (d) switch at <=4 of 64 | 1.37 (2.26) | 2.05 (3.67) | 4.20 (7.00) | 8.30 (13.06) | 11.83 (17.94) |
| (d) switch at <=8 of 64 | 1.40 (2.36) | 2.27 (3.53) | 4.44 (6.97) | 8.33 (12.67) | 11.77 (17.17) |
| (d) switch at <=16 of 64 | 1.59 (2.89) | 2.38 (4.16) | 4.64 (8.12) | 8.52 (13.57) | 12.08 (18.34) |
| (d) switch at <=8, rows/128 | 1.58 (2.58) | 2.34 (3.84) | 4.73 (7.68) | 8.62 (12.89) | 12.02 (17.73) |
| (d) switch at <=8, col only to 84 B | 1.38 (3.01) | 2.04 (3.81) | 4.45 (7.81) | 9.30 (14.57) | 13.60 (19.45) |

**text queries, visible 10%** — ms per query at 1M (at 50M-scale placement in brackets); median of 10 queries

| Layout | 0.01% | 0.1% | 1% | 10% | 50% |
|---|---|---|---|---|---|
| (a) rows, tight, growing levels | 0.59 (1.31) | 0.83 (1.44) | 1.21 (2.37) | 1.60 (3.29) | 1.98 (3.82) |
| (b) rows, 64 B steps | 0.46 (1.14) | 0.61 (1.51) | 0.90 (2.29) | 1.56 (3.63) | 2.10 (4.27) |
| rows, 128 B steps | 0.61 (1.62) | 0.74 (2.01) | 1.04 (2.85) | 1.74 (3.88) | 2.35 (4.53) |
| col32 (as built) | 0.99 (1.52) | 1.50 (2.28) | 3.19 (4.83) | 6.48 (9.54) | 9.56 (14.34) |
| (c) col64, all columnar | 1.14 (1.63) | 2.05 (2.20) | 4.13 (5.08) | 7.36 (10.60) | 11.29 (16.36) |
| (d) fixed split: col to 43 B, then rows/64 | 0.91 (1.19) | 1.22 (1.80) | 2.34 (3.47) | 3.40 (5.59) | 4.11 (6.95) |
| (d) switch at <=4 of 64 | 0.80 (1.49) | 1.23 (2.30) | 2.33 (4.31) | 4.12 (7.06) | 5.77 (9.84) |
| (d) switch at <=8 of 64 | 0.56 (1.15) | 0.78 (1.83) | 1.35 (2.65) | 2.23 (4.14) | 2.86 (4.95) |
| (d) switch at <=16 of 64 | 0.48 (1.17) | 0.55 (1.49) | 0.90 (2.21) | 1.56 (3.41) | 2.00 (4.15) |
| (d) switch at <=8, rows/128 | 0.72 (1.57) | 1.02 (2.00) | 1.68 (3.06) | 2.51 (4.42) | 3.13 (5.60) |
| (d) switch at <=8, col only to 84 B | 0.55 (1.30) | 0.73 (1.70) | 1.32 (2.79) | 2.12 (4.22) | 2.72 (5.24) |

**item queries, visible all** — ms per query at 1M (at 50M-scale placement in brackets); median of 10 queries

| Layout | 0.01% | 0.1% | 1% | 10% | 50% |
|---|---|---|---|---|---|
| (a) rows, tight, growing levels | 0.34 (0.53) | 0.53 (0.72) | 1.80 (2.54) | 10.72 (14.62) | 11.15 (15.18) |
| (b) rows, 64 B steps | 0.25 (0.47) | 0.39 (0.66) | 1.20 (2.36) | 10.73 (14.20) | 12.26 (16.09) |
| rows, 128 B steps | 0.37 (0.62) | 0.56 (0.92) | 1.96 (3.55) | 12.61 (17.64) | 13.75 (18.45) |
| col32 (as built) | 0.14 (0.21) | 0.22 (0.33) | 0.64 (0.90) | 8.53 (11.85) | 13.10 (17.58) |
| (c) col64, all columnar | 0.15 (0.26) | 0.24 (0.39) | 0.62 (1.10) | 8.23 (12.17) | 11.84 (18.16) |
| (d) fixed split: col to 43 B, then rows/64 | 0.15 (0.26) | 0.23 (0.39) | 0.67 (1.03) | 10.05 (14.32) | 13.73 (17.28) |
| (d) switch at <=4 of 64 | 0.13 (0.23) | 0.22 (0.40) | 0.64 (1.11) | 7.29 (11.90) | 11.22 (17.61) |
| (d) switch at <=8 of 64 | 0.15 (0.24) | 0.25 (0.42) | 0.64 (1.20) | 7.23 (11.80) | 11.22 (17.52) |
| (d) switch at <=16 of 64 | 0.16 (0.27) | 0.26 (0.45) | 0.72 (1.31) | 7.49 (12.48) | 11.32 (17.51) |
| (d) switch at <=8, rows/128 | 0.16 (0.28) | 0.25 (0.45) | 0.71 (1.24) | 7.56 (12.16) | 11.34 (17.31) |
| (d) switch at <=8, col only to 84 B | 0.14 (0.24) | 0.24 (0.42) | 0.66 (1.22) | 7.99 (12.80) | 12.88 (18.89) |

**item queries, visible 10%** — ms per query at 1M (at 50M-scale placement in brackets); median of 10 queries

| Layout | 0.01% | 0.1% | 1% | 10% | 50% |
|---|---|---|---|---|---|
| (a) rows, tight, growing levels | 0.05 (0.13) | 0.07 (0.17) | 0.27 (0.56) | 1.64 (3.61) | 1.94 (4.54) |
| (b) rows, 64 B steps | 0.04 (0.11) | 0.06 (0.16) | 0.20 (0.64) | 1.31 (3.67) | 1.88 (4.67) |
| rows, 128 B steps | 0.05 (0.14) | 0.07 (0.21) | 0.25 (0.84) | 1.59 (4.12) | 2.15 (5.47) |
| col32 (as built) | 0.08 (0.14) | 0.14 (0.20) | 0.40 (0.59) | 5.62 (7.80) | 9.09 (12.66) |
| (c) col64, all columnar | 0.09 (0.16) | 0.15 (0.26) | 0.45 (0.83) | 6.04 (9.08) | 9.89 (15.27) |
| (d) fixed split: col to 43 B, then rows/64 | 0.06 (0.13) | 0.10 (0.18) | 0.30 (0.55) | 3.08 (5.25) | 3.85 (6.77) |
| (d) switch at <=4 of 64 | 0.07 (0.12) | 0.11 (0.21) | 0.34 (0.64) | 3.57 (6.32) | 5.29 (9.12) |
| (d) switch at <=8 of 64 | 0.05 (0.11) | 0.08 (0.16) | 0.24 (0.56) | 1.96 (3.79) | 2.71 (4.93) |
| (d) switch at <=16 of 64 | 0.04 (0.09) | 0.06 (0.14) | 0.20 (0.53) | 1.30 (3.01) | 1.87 (3.73) |
| (d) switch at <=8, rows/128 | 0.06 (0.13) | 0.09 (0.18) | 0.30 (0.67) | 2.15 (4.07) | 2.82 (5.19) |
| (d) switch at <=8, col only to 84 B | 0.05 (0.10) | 0.07 (0.15) | 0.24 (0.55) | 1.92 (3.72) | 2.63 (4.88) |

Effective bandwidth (distinct lines x 64 over time, median over text cells): 1M col32 19, col64 18, row64 9.5,
rowtight 10, row128 12 GB/s; 50M placement col32 13, col64 14, row64 5.8, rowtight 6.6 GB/s.

## Which layout wins

| Queries, visibility | 0.01–0.1% | 1% | 10% | 50% |
|---|---|---|---|---|
| Text, all | col32 or hybrid k = 4–8 (tie, within noise) | hybrid k = 4 (−11% vs col32 at 1M, tie at 50M) | hybrid k = 4–8 (−13% at 1M, −9% at 50M) | rows tight (−13% vs col32 at 1M, −29% at 50M) |
| Text, 10% random | rows/64 (2.2–2.5x faster than col32 at 1M; 1.3–1.5x at 50M) | rows/64 (3.5x; 2.1x) | rows (4x; 2.6x) | rows (4.5x; 3.4x) |
| Item, all | col32 (rows 1.8x slower) | col32 / col64 (rows 1.9x slower) | hybrid k = 4–8 (−15%) | hybrid ≈ rows ≈ col64 |
| Item, 10% random | rows/64 or hybrid k = 16 | rows/64 | rows/64 (4.3x faster than col32) | rows/64 (4.8x) |

**Switch point.** By lines, all visible: k = 8 of 64 (text 0.01%: 19.4 B against 20.3 at k = 4, 19.5 at k = 16 and
22.8 at k = 1); by pairs and by time, k = 4–8. With 10% visible, k ≥ 16 (blocks start with about 6 live items and go
straight to rows). A single rule of k = 8 is within 5–11% of the best all-visible time at small answers and 20–50% behind
rows under the random filter; k = 16 is equal to rows under the filter and up to 20% behind col32 all-visible at
0.01%. A simple model fits this: a block's level costs Δ lines as columns and about one line per live item as rows,
at about half the bandwidth, so rows pay below about Δ/2 live items: 2–4 at the deterministic levels (Δ = 4–8.5 B)
and 3–17 at the randomised ones (Δ = 5–34 B), not 1. (The model is mine; the switch points above are measured.)

## Surprises

- The layout as built is already the columnar one; the comparison the hypothesis needs is col32 against the hybrid,
  and at full visibility that gap is small.
- Columns waste little at full visibility because survivors stay clustered in the norm-sorted order: per touched
  text item at 0.01%, col32 reads 0.71 lines against 0.38 lines of demand, rows/64 1.08 lines.
- A query-chosen column order (bulk/REPORT.md, 11–25% fewer bytes) helps columns directly but cannot help rows at
  small answers, where nearly every touched item already reads exactly one line.
- At the 50M placement, rows lose most of their lead (each scattered item costs a TLB miss with 4 KB pages); with
  huge pages for the index the 1M figures are the better guide.
- At 10% and 50% answers, full vectors (identical in every layout) are 82–185 B per item for text, as much as the
  whole code stream; layout is not the lever there.

## Limits

- The replay times memory and a nominal decode, not the bounds, tables and re-packing of the prototype.
- Visibility is random per row, the worst case for columns; a filter clustered in sort order behaves like full
  visibility. Not measured.
- One dataset (BioCLIP-2, n = 768), one machine (Ryzen 9 5900X, AVX2, WSL2), a noisy neighbour.
- The hybrid stores rows of the whole stream; with columns only to C, lines were also counted for C = 43 and 84 B.

## Files (`lines/`)

- `lines.c` — line counter and replay timer (`gcc -O3 -march=native -o lines lines.c -lm`;
  `lines DIR CLASS count|time SCALE NQ [LAYOUTS]`)
- `agg.py`, `aggt.py`, `mdtables.py` — tables from the results
- `results/count_*.csv` — per query lines, pairs, runs, bytes, full reads; `results/count_tables.txt` — all layouts
- `results/time1_*.csv`, `results/time50_*.csv`, `results/time1rep_text.csv` — timings; `results/time_tables.txt`
  (with GB/s and ns per touched item); `results/md_tables.md`
- Demand dump (not committed, 200 MB): `/home/joe/code/vexact-linelayout-data/d1m`, made by
  `RAYON_NUM_THREADS=3 taskset -c 9-11 vexact --v2 --int --gtab --flags repack,nusort,pscan --no-approx --plan index
  --index .../c1m/dual --queries .../text_q.npy --item-queries .../c1m/item_q.npy demand --nq 20 --out DIR`
