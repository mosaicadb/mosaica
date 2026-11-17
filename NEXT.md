# Next steps

What is open on LIMINAL as of 17 November 2026, in rough order. SYSTEM.md describes the index as it stands; each item
names the branch or report that holds its evidence.

## Checks the consolidation could not run

rust/REPORT_CONSOLIDATION.md records what was checked; these were not.
- **A run under a memory cap** with the current planner, through bench/threshold-db/capped/run_ours.sh at one answer
  share, its answers compared with the uncapped dumps. Plan files now carry only the disk constants (`io_us`,
  `io_gbps`); without one the open measures them once per machine and device and caches them (rust/REPORT_BUILD.md).
- **Out-of-distribution queries:** the 50k check (rust/results/const/check.sh) with the BioCLIP-2 text prompts, which
  need the text encoder.

## Exact path

- **Never slower than a plain scan under a filter: mostly done**
  (bench/threshold-db/results/planner/REPORT_PLANNER.md; times from a loaded machine, interleaved). With
  the visible set compacted per session (codes and vectors), the planned query beats LIMINAL's own scan at every answer
  share under random 1%, 10% and 50% visible (0.44–0.62 against 1.07–1.25 ms; 9.7–10.4 against 11.5–12.5 ms; 20–46
  against 47–52 ms), and under 241 genera up to answers of 1% and 964 genera up to 30%. Uncompacted, the planner alone fixes only
  random 1%. Answers equal the scan's bit for bit. Still open:
  - **Small, cache-warm genus sets** (19 genera, 1%): 1.28–1.96 times slower than the plain scan (0.65–0.98 against 0.44–0.66 ms). The cost model
    prices a cached scan at 0.10 µs per row against 0.06–0.08 actual, so it picks the index on 345 of 525 runs.
  - **The plain scan beats the compacted scan by about 20% under genus sets,** unexplained: both read contiguous rows
    with the same kernel.
  - **The largest answers under 241 and 964 genera:** 241 genera is 16–19% behind the scan at the 50% and 75% shares.
  - **A session switch for compaction.** It is opt-in through `Index2::compact` with no session-level setting. Ruled:
    a session compacts its codes by default, and codes and vectors is an option the caller states. It costs 22 ms and
    24 MB at random 1%, 120 ms and 186 MB at 10%, and 576 ms and 913 MB at 50% (codes alone: 12 ms and 9 MB, 38 ms and
    33 MB, 162 ms and 147 MB).
  - **The uncompacted index path over sparse clusters** costs more than the model predicts (22 ms forced at random 1%).
- **Two deterministic levels and larger clusters.** Levels at 16 and 32 B (rust/REPORT_DETLEVELS.md) and about 4,000
  items per cluster (rust/REPORT_CLUSTERSIZE.md) are 18–33% faster for text and 10–23% for items at 1M together. The
  code is on research/levels-clusters, on research/combined at d443f66; it needs rebasing onto this branch (q.rs and
  fit.py conflict). At 500k, 1M and 3M (rust/REPORT_SCALING.md), 1,000 items per cluster was never the best; on the
  uniform samples the best was 2,000 at 1M and 4,000 at 3M (where 2,000 ties on thresholds and is 16–51% slower on
  exact k-NN). fit.py's default on that branch now grows with N: 2,000 items per cluster up to 2M, doubling for each
  4x N with no cap (64,000 at 1B), with up to 64 groups. Before relying on it above 32M: time 16,000 per cluster
  there, give the approximate mode u32 positions for clusters of 65,536 rows or more, and grow fit.py's k-means sample
  with K.
- **Store the codes at build.** They are rebuilt at every open (2.5–3.5 minutes at 1M); at a billion items they must
  be stored.
- **A hot-row cache over O_DIRECT,** only if repeated small queries under a memory cap matter: direct reads miss the
  page cache, so a warm repeat at 1% answers costs 65 ms against 48–50 through the mapping. RWF_DONTCACHE was tried and
  lost (REPORT_DISK.md on research/disk-reads).
- **Bring DESIGN.md up to date with the code** (anchors, constants, level schedule); SYSTEM.md is current.
- **Delete the side branches and worktrees** whose work is merged.
- **`dot_i8` has no tail** past a multiple of 32 dimensions (harmless while n % 32 == 0, which fit.py's padding keeps).

## Approximate path

- **Approximate threshold, as IVF range search means it** (faiss `range_search`): probe the `nprobe` best clusters in
  the approximate mode's order and return every visible item in them scoring at least t, decided exactly, so the answer
  holds no false positives and misses only items in clusters not probed. There is no entry point for it yet; dbknn
  measures the approximate mode as top-m instead.
- **The caller asks for a recall target, not for `nprobe` and a dial** (ruled). Today the approximate mode takes two
  internal knobs, the clusters probed and the dial (per-level factors on the upper half-widths, fitted for a miss rate
  eps), and only the benchmarks' ladder turns a recall target into them. Design, with the single API below:
  `knn(q, k, Path::Approx { recall })` and `threshold(q, t, Path::Approx { recall })`. At open, or at the first
  approximate query, the index fits a table from (k band, recall target) to the cheapest (nprobe, eps) on
  pseudo-queries (the ladder's search, at k = 1, 10, 100 and 1,000, interpolated); a query looks its setting up and
  reports the setting it ran. Benchmarks keep explicit nprobe and eps in the binary; the library offers only the
  target. Present it as what it is: an expected recall on corpus-like queries, not a guarantee (`tol` is the
  guarantee), and an approximate answer never holds a wrong item, because the dial cuts only upper bounds.
- **The dial on the integer levels.** At levels 0-5 the integer path scales only the code term of the upper half-width
  (the table error is unscaled and the fixed term takes the largest factor), while the float path scales all of it and
  `Dial::fit` assumes that. Every measured approximate row ran with `--int`, so the reports already reflect it. Decide
  by recall per unit of time, as an internal choice under the recall target above.

- **The three steps in reports/annideas/NEXT.md:** cheaper group setup, sub-clusters (row order ruled: rows stay sorted
  by key and the approximate mode gathers each sub-cluster's rows; reports/annideas/REPORT.md), and a fused fast-scan
  pass over levels 0–3.
- **Hand over to the exact path at large answers.** Under filters the approximate mode loses to the exact index by up to
  three times once answers pass about 10% of the visible set; the planner should switch.
- **Fit the dial above k = 1,000.** Larger answers currently reuse the k = 1,000 dial.
- **Low finding from the merge review:** ties between equal scores can differ from the exact path's order.

## Costs the planner gets wrong (the planner chooses; answers do not change)

In memory the planner no longer times anything (rust/REPORT_BUILD.md, "The planner without timing"): unfiltered
queries take the support table and the index path; under a visible set a rule in rows (64 + 0.186 x cluster rows)
picks scan, direct scoring or the index path. `Plan::calibrate`, `--calib` and `--plan-default` are gone. Still open,
from review/REVIEW_2026-11-15_merges.md, on the priced (vectors on disk) path:
- **The index path's survivor reads are never priced on file** (no disk term in `index_us`), while the full-vector
  path's are, so the index path looks cheaper than it is at large answers under a cap.
- Reads priced as sequential where execution faults pages: direct scoring, filtered scans, `knn2`'s scan; codes-only
  compaction's faults are not priced; pricing ignores what is cached.
- **A capped rerun is not REPORT_CAPPED's run:** reads through io_uring are priced, and every session's compacted
  copies are built before the cap.
- **The rows rule was measured on one machine** (1M BioCLIP-2; with AVX-512 the crossover at 1,000 rows a cluster is
  221 rows on one thread and 308 on four; the owner set 250); compacted sessions of about 10% lean to the index path
  (1.4-1.6 times a scan).

## Scaling to 1B (on hold)

The build now keeps memory flat up to 4M rows (rust/REPORT_BUILD.md: 469 MB peak at 4M with the new defaults; the
assembled phase holds what still grows). Before a billion-row build, in rough order:
1. **Map `perm` and the per-cluster records** (`Cl2`, the centre tables) at build and open instead of reading them
   onto the heap. They are what still grows with N (about 18 KB a cluster, plus 4 bytes a row for `perm`).
2. **No per-row work at open:** the escape `HashMap` built at open becomes a stored table that open maps, so opening
   stays proportional to the clusters.
3. **Bound the per-cluster build buffers:** with clusters growing with N (about 64,000 rows a cluster at 1B), the
   support table's `px` and the encoder's per-cluster rows must be processed in bounded chunks.
4. **A stored `pm` table,** mapped at open rather than computed.
5. **The partition's cell count at scale:** `create` lets `tools/partition.py` choose, and at 4M it chose one cell
   (one k-means over every row: 440 s and 1.2 GB with 4,000 centres). At 1B the cells must be set from N and memory,
   and the partition should run without loading all rows (or move to Rust/GPU faiss).
6. **Then the gates at scale:** 20M BioCLIP-2, 100M synthetic, peak memory and build time per phase, open time.

## Restructuring

From review/CODE_REVIEW.md; each step keeps dbcmp's answers bit for bit, checked by the end-to-end test and answer
dumps before and after. Still to rule: keep or drop the norm sort.

1. **Move verification, profiling and dumps out of the level loop** behind an observer type: `PROF`, `ERRS` (which
   `Dial::fit` sets around a parallel loop, so it is unsafe beside other queries) and `DUMP`, and the process-wide read
   counters. Then split q.rs, and give full-vector scoring one owner.
2. **One `threshold` and one `knn`**, each with a path (scan, exact, approximate) and one planner. With it, split `Plan`
   into a cost model (timed, saved in plan files), tuning (defaults the measured configuration) and per-query arguments
   (`tol`, the forced path), and give the approximate path its recall target.
3. **The correctness findings:** a visible set naming an unknown entity panics; k-NN returns certified items with +inf
   scores in entity order; k = 0 panics; a non-power-of-two `--nguar` overstates `npow`.
4. **The performance findings,** measured one at a time: k-NN selection rebuilds a cluster's tables per refined row;
   sparse-cluster gathers and the full vectors the index path reads are unpriced; the scan constant is timed out of
   cache; the K x K support table cannot reach a billion items.

## Benchmarks

Rule: rerun only the rows whose system, settings, data or machine changed; keep built indexes.

- **The rest of the capped comparison:** faiss flat from a memory-mapped file, DiskANN (diskannpy 0.7.0, its build needs
  four threads on a quiet machine; its static disk index has no filter), four-thread runs.
- **Panorama:** a four-thread run (the 4-thread plot shows its 1-thread figures, labelled), and its k-NN search, flat
  and IVF-Panorama, against LIMINAL's.
- **Split results by query kind:** in-distribution (held-out images) against out-of-distribution (text prompts).
- **A tolerance curve** for threshold sets: time and bytes against the caller's tolerance d.
- **A fairer cache test:** rotate between many viewers' visible sets, so small sets do not all sit in L3.
- **Scale and breadth:** 20M BioCLIP-2 with the lean set (LIMINAL, ClickHouse bfloat16, faiss exact in half precision,
  IVF), then 100M+, and two or three more datasets including a 1,536-dimension text-embedding set and one with
  in-distribution queries.
- **Filtered baselines people use:** Qdrant's filter-aware HNSW (in Docker), beside ACORN-γ (built from the reference
  repository with three fixes, acorn.patch on research/filter-sweep).
- DuckDB is dropped from comparisons; faiss runs in half precision beside single precision.
