# Columnar deterministic code with a query-chosen read order: results at 1M

Branch `research/colorder`, from `research/combined` at 7e223fa2b. Measured on this machine (Ryzen 9 5900X under
WSL2, AVX2), one thread pinned, with the fixed plan constants (`results/combined/plan_fixed.txt`), on the 1M
data-only dual index. The baseline runs with `--int --gtab --flags repack`. It was re-run at the start and again
inside the same process as stage A. Every new path is a plan flag (`col`, `colq`, `collazy`) and **off by default**.

## Summary

**None of the stages pays on this code. Each costs time and bytes against the level stream.**

- **A**, columnar store in fixed order: 1–27% more bytes, 10–25% slower.
- **B**, query-chosen order: saves only 0.1–1.5% of A's bytes, and its per-cluster sort adds 3–4 ms per query.
- **C**, lazy block norms: slightly worse than B. The single stored norm almost never decides on its own.

**Why.** The study's gains came from a 4-bit cell on every coordinate, with 768 columns to choose from. Here the
deterministic code is 32 B over about 116 coordinates the profile has already picked, and it is progressive: tree
indices are re-stored at each depth. A per-coordinate column must deliver a coordinate's whole code at once.
Section 4 sets out what that costs.

**Correctness.** Every path was checked against exact scores at 50k with zero violations. Every bound is
deterministic, and the randomised levels after it are unchanged.

## 1. What changed

**Bug fix (2144f31e1, `src/v2/q.rs`).** The kappa/tan escape path read `sv.idx[o + i]` for inactive lanes, and in
the last block of a large state that read went past the end. It aborted the first 1M baseline run. It now reads only
the escaped lanes. No effect on results or speed.

**Stage A: the columnar store, fixed order** (commit 9425c14ce, flag `col`).

Files:
- `src/v2/col.rs` (new): plan, store, bound and kernel.
- `src/v2/mod.rs`: the encoder records each coordinate's final code; the build assembles the store and the norm grids.
- `src/v2/q.rs`: `run_cluster_from` resumes the level walk at the first randomised level; the dispatch; the flags.
- `src/kernel.rs`: counters.
- `src/run2.rs`: flags, bytes, per-check statistics, a breakdown column.

What is stored:
- **The code change.** The stream cannot be columnised per coordinate as it stands, because a tree field re-stores
  the full index at each depth (1 → 2 → 4 bits). The smallest change stores each coded coordinate's *final* code
  once: the tree index, then its stage bits. Its value equals the stream's value after the last deterministic level
  exactly (the build asserts this for every item), so the randomised levels follow unchanged.
- **Units.** Most coded coordinates carry 1–3 bits, so one coordinate per nibble would double the store (61 B
  against 32 B). Coordinates are packed in profile order, within a norm block, into units of whole 4-bit rows:
  65 rows, 32.7 B per item, the same as the stream. A unit is read whole. Each row is one column across the
  cluster's items, laid out 32 items per 16 bytes as in the stream.
- **Scalars.** Per item, 1 B each on a log grid per group, rounded up: the norm of u = z/(sσ) in the weights
  h = (pq·λ)^(−1/4) over each of 5 blocks of the coded coordinates, plus their single norm. The last level's stored
  error norm ρ5 is reused.

How a query reads it:
- **Order and checks.** Units are read block by block over the live items of each 32-item block of candidates, with
  integer table sums. Checks fall at 0, 8, 16, 24, 32, 48 and 64 rows, and after the last row.
- **Survivors** continue at the first randomised level, held in place while occupancy is above the repack threshold.
- **The bound** (deterministic):

  ```
  q·x − qm − α·mr − s·Σ_R w·v = Σ_{R+nonD} qv′·e + s·Σ_{D−R} w·u
  |·| ≤ |(qv′ d5)_{R+nonD}|·ρ5 + s·Σ_b |(w h)_{b−R}|·N_b
  ```

  Its slacks are the existing fixed term, the f32 weights, table rounding, α·(mv − mv32), and f32 evaluation.
- **Energy accounting** of the read part is optional (`VEXACT_COLEN=1`). It bounds the energy read from the code
  values, less what ρ5 allows. It is off by default because it changed bytes by under 0.1%.

**Stage B: query-chosen order** (`colq`). Units are sorted per (query, cluster) by Σ w_c²(1 − D_c) per row. The
per-check constants come from prefix and suffix sums over that order.
- The key |w·h|² per row (`VEXACT_COLKEY=h`) was worse.
- Finer blocks (`VEXACT_COLB=0,2,4,8,16,32,64`) were also worse.

**Stage C: lazy block norms** (`collazy`). The single norm is checked first. Block norms are read only for the items
it leaves undecided, and from then on the smaller bound is used.

**Stage D was not done.** The stored cells are Lloyd–Max tree cells with residual stages, not dithered uniform cells,
so the dither certificate of bulk/REPORT.md section 3.3 does not apply. It would need a new deterministic code and
the randomised levels re-fitted after it.

## 2. Correctness at 50k

Each stage was run on 200 item and 200 text queries, at 7 answer sizes, with interval checks on every query, 4
filtered visible sets, and k-NN under three plans. Logs are in `results/colorder/v50_*.log`.

| Path | Wrong in/out | Interval checks | Violations det/rand | Decisions checked, not implied by exact score | Table sums outside f64 bound |
|---|---|---|---|---|---|
| Baseline | 0/0 | 1,219,146,892 | 0/0 | 70,090,648, 0 | 0 |
| A `col` | 0/0 | 2,791,003,976 | 0/0 | 1,642,084,915, 0 | 0 |
| B `colq` | 0/0 | 2,749,335,675 | 0/0 | 1,600,384,365, 0 | 0 |
| C `colq,collazy` | 0/0 | 2,749,272,988 | 0/0 | 1,600,309,468, 0 | 0 |

The largest randomised error was 5.87 sd (c = 9.46). k-NN matched on 400 of 400 queries in every run.

## 3. Latency and bytes at 1M

One thread, 20 queries per class, the median over 3 runs of the median over queries. Bytes are per item of N, and
include the key, the scalars and full vectors at 1,536 B. Base and A ran in one process (`e2e_base_vs_A.csv`), B and C
in another (`e2e_B_vs_C.csv`).

| Item | base ms | A | B | C | base B/item | A | B | C |
|---|---|---|---|---|---|---|---|---|
| 0.01% | 2.6 | 3.0 | 3.3 | 3.4 | 2.8 | 3.1 | 3.1 | 3.1 |
| 0.1% | 3.9 | 4.2 | 4.5 | 4.7 | 5.8 | 6.2 | 6.2 | 6.2 |
| 1% | 9.7 | 10.7 | 12.3 | 11.9 | 18.7 | 20.4 | 20.3 | 20.5 |
| 10% | 86.0 | 100.9 | 104.9 | 107.7 | 136.0 | 140.8 | 140.8 | 141.7 |
| 30% | 114.2 | 129.7 | 134.6 | 138.3 | 256.8 | 260.6 | 260.6 | 261.6 |
| 50% | 123.8 | 139.4 | 146.2 | 148.0 | 296.6 | 300.2 | 300.2 | 301.1 |
| 75% | 125.0 | 139.8 | 147.1 | 150.3 | 254.0 | 257.7 | 257.7 | 258.6 |

| Text | base ms | A | B | C | base B/item | A | B | C |
|---|---|---|---|---|---|---|---|---|
| 0.01% | 23.8 | 29.4 | 32.9 | 33.8 | 20.1 | 25.4 | 25.1 | 25.4 |
| 0.1% | 30.6 | 37.5 | 42.0 | 43.3 | 25.1 | 31.1 | 30.8 | 31.3 |
| 1% | 57.4 | 68.7 | 76.3 | 78.4 | 64.6 | 70.5 | 70.3 | 71.1 |
| 10% | 92.6 | 107.7 | 116.8 | 119.1 | 192.0 | 196.6 | 196.5 | 197.5 |
| 30% | 117.7 | 130.9 | 143.2 | 144.4 | 309.5 | 313.2 | 313.2 | 314.2 |
| 50% | 126.8 | 143.6 | 153.3 | 155.8 | 347.0 | 350.5 | 350.5 | 351.5 |
| 75% | 132.3 | 147.0 | 154.0 | 157.1 | 300.5 | 304.0 | 304.0 | 305.0 |

k-NN is unchanged: it takes the interval path, which keeps the stream.

### Breakdown (ms, one thread, 15 queries)

Files: `breakdown_base.csv`, `breakdown_colq.csv`. Setup is the basis products, rotations and cluster setup.

| Cell | Path | Total | Tables | Re-pack | Decode | Bounds+decisions | Init | Column order | Setup | Full | Bitmap |
|---|---|---|---|---|---|---|---|---|---|---|---|
| text 0.01% | base | 23.7 | 3.1 | 2.1 | 2.4 | 2.6 | 2.0 | – | 6.8 | 0.5 | 0.0 |
| text 0.01% | B | 32.4 | 3.8 | 1.0 | 5.4 | 6.7 | 5.1 | 3.2 | 5.9 | 0.5 | 0.0 |
| text 1% | base | 54.9 | 7.5 | 5.4 | 7.6 | 13.6 | 2.9 | – | 7.3 | 1.1 | 0.1 |
| text 1% | B | 72.9 | 9.1 | 3.8 | 12.2 | 20.9 | 8.4 | 3.9 | 6.5 | 1.2 | 0.1 |
| item 50% | base | 123.2 | 10.3 | 7.0 | 18.6 | 46.2 | 3.0 | – | 7.3 | 11.8 | 7.1 |
| item 50% | B | 148.4 | 13.2 | 5.8 | 24.6 | 60.0 | 9.7 | 4.4 | 6.9 | 12.6 | 7.4 |

## 4. Why the order gains little here

Measured at 50k with the index path forced. The table compares half-widths at equal bytes: the stream's f32 path
against the columnar check, item queries at 0.01%.

| Code read | Stream | Columnar, fixed order |
|---|---|---|
| 4 B | 0.162 | 0.190 |
| 8 B | 0.142 | 0.170 |
| 16 B | 0.119 | 0.123 |
| 24 B | 0.105 | 0.120 |

- **The stream's early bytes go further.** Its greedy allocation spends the first 4 B on a sign bit for each of about
  32 coordinates, which re-stored tree indices make possible. One weighted norm per level then bounds the joint error.
- **A column must deliver a coordinate's whole code at once.** A tree's deeper bits are not additive on their own, so
  the first rows inform fewer coordinates.
- **Bounding the parts of an unread coordinate separately costs up to √2.** Splitting it into code value and error
  gives √D + √(1 − D) ≥ 1 on that coordinate. The final bound keeps unread coordinates whole for this reason.
- **The order has little room.** The profile already picked the coded coordinates. A query's energy outside them sits
  in the ρ5 term, which no order reduces, and that term is 40–60% of the half-width at every check.
- **Scalars cost more per touched item.** The columnar path pays 7.5 B per touched item (key, ρ5, 5 block norms)
  against the stream's 6.5 B at its first level.
- **The column check costs more time than the stream's integer compares.** Survivors at large answers pay the column
  checks, then the same randomised levels. B's sort adds 3–4 ms. The colscan study's 9–14 ns per item compared a
  kernel with an end-to-end figure; the stream's deterministic kernel already runs at about 8 ns per touched item at
  text 0.01%.

## 5. Next bottleneck

The baseline is unchanged, since everything above is off by default. Its main costs:

- **Bounds and decisions:** 13.6 ms at text 1%, 46 ms at item 50%, mostly at the randomised levels.
- **Decode:** 7.6 and 18.6 ms in the same cells.
- **Tables:** 7.5–10 ms.
- **Group setup** (basis products): 4.2 ms.

At 10–75% answers, 70–97% of items reach the randomised levels. There, the per-level f32 bound and decision work,
emulated as 16 lanes on AVX2, is the largest single cost.

For a query order to pay, the code has to change:

- Each coordinate needs its own additive planes (uniform or dithered cells), so that columns refine progressively.
- There must be more columns than the profile's top coordinates to choose from.

That means re-fitting the randomised levels after the new deterministic part. The study's own numbers show its
4-bit cells start 30–40% above the native escape code in absolute bytes.

## Files and knobs

- `src/v2/col.rs`: plan, store, bound, kernel.
- `results/colorder/`:
  - e2e CSVs: `e2e_base_start.csv`, `e2e_base_vs_A.csv`, `e2e_B_vs_C.csv`.
  - Breakdowns: `breakdown_base.csv`, `breakdown_colq.csv`.
  - 50k verify logs: `v50_*.log`.
  - Per-check statistics at 1M: `colchecks_1m.txt`.
- Environment knobs:
  - `VEXACT_COLB`: block edges.
  - `VEXACT_COLCK`: checks, in rows.
  - `VEXACT_COLEN`: energy accounting.
  - `VEXACT_COLKEY=h`: the alternative order key.
  - `VEXACT_COLDBG`: half-width diagnostics per check.
