# Next steps

What is open on LIMINAL as of 15 November 2026, in rough order. SYSTEM.md describes the index as it stands; each item
names the branch or report that holds its evidence.

## Merges owed onto this branch

- **Checks run in the cloud session on fresh data (15 November).** Not the owner's c50k, c1m or text prompts, which
  were not reachable: BioCLIP-2 image embeddings fetched from imageomics/TreeOfLife-200M-Embeddings (a seeded sample of
  whole row groups), fitted by tools/fit.py, queries held-out images. With the binary at e9b6f29:
  - 50k (50 clusters): verify with 200 item and 200 held-out queries: 0 wrong-in and wrong-out, 0 deterministic and 0
    randomised violations, 0 decisions not implied, k-NN 0 mismatches of 400; bench/oracle_thr.py: 0 wrong, k-NN 0 of
    80 differ.
  - Answers before and after: dbcmp dumps on the 50k index (40 queries, five shares, unfiltered and three visible sets)
    from 546375a (before the merges), fe1b413 (after them, before the deletions) and e9b6f29 are byte-identical for
    every variant each binary has (40 files).
  - 1M (1,000 clusters, 64 groups): dbcmp flat, index, idx, scan, cindex and cvindex, unfiltered and 1% visible, all
    500 answers identical; dbtopk exact recall 1.0 at k 10 and 100, the approximate mode reaching 0.9 and 0.99;
    dbcmp with --full-file read through io_uring and the mapping, answers byte-identical to the in-memory dumps.
  - Not run: the capped run. The session has no systemd user bus and only cgroup v1, so neither `systemd-run --user`
    nor the binary's own cap works there. It and the 50k check on the owner's c50k with text queries remain owed.

- [x] **Benchmark branches, merged.** research/plots-refresh (with research/filter-sweep) and research/disk-reads (with
  research/bench-capped). The release build passes, and `cargo test --release` gives 15 passed and 1 ignored. Still to do
  on the owner's machine before this counts as checked: the 50k check (rust/results/const/check.sh), one `dbcmp`
  threshold run and one approximate `dbtopk` run on the 1M index, and one capped `--full-file` run through
  bench/threshold-db/capped/run_ours.sh at a single answer share, whose answers must match the uncapped dumps.
- [x] **research/scan-planner, merged** (b79dbc8d5, with the review's fixes). The release build passes and
  `cargo test --release` gives 15 passed and 1 ignored; the same checks as above are still owed on the owner's machine,
  with `dbcmp --cmp-what flat,index,scan,cindex,cvindex` under a visible set as the smoke run. Its filtered rows are being
  remeasured on a quiet machine for the plots on research/plots-refresh. What the merge changed or revealed:
  - Full vectors are `Arc<disk::Rows>`, with `Rows::Mem` holding the aligned `F16Rows`, so compaction's sharing and the
    file under a cap both hold. No compacted index uses the io_uring reader: codes-and-vectors compaction copies the
    vectors into memory (which a cap cannot evict, so compacting a large set under 776 MB can be killed), and
    codes-only compaction reads its parent's rows through `vrow` and the mapping (`Index2::own_rows`).
  - `plan2` had no disk costs (only `plan2_prev` did); it now prices on-file rows the way `plan2_prev` does, estimating
    an unsampled cluster's candidate span as d of its rows, and only on an index whose own rows are on file. Still
    mispriced: `Pre::Direct` and the filtered scan read through the mapping (scattered faults) but are priced as
    sequential reads, and codes-only compaction's faults are not priced. capped/run_ours.sh's plan.txt has none of
    plan2's constants. Unmeasured: run the capped comparison with the new planner.
  - `dbcmp` interleaves its variants per query by default; capped runs pass `--cmp-order variant` (run_ours.sh) to
    keep REPORT_CAPPED's order.
  - No tests cover the planner's choices, `Index2::compact` or `F16Rows`.

## Exact path

- **Never slower than a plain scan under a filter: mostly done.** The scan-planner work above
  (bench/threshold-db/results/planner/REPORT_PLANNER.md on that branch; times from a loaded machine, interleaved). With
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
  exact k-NN). fit.py's default on that branch now grows with N: 2,000 items per cluster up to 2M, 4,000 up to 8M,
  8,000 above, with up to 64 groups.
- **Store the codes at build.** They are rebuilt at every open (2.5–3.5 minutes at 1M); at a billion items they must
  be stored.
- **A hot-row cache over O_DIRECT,** only if repeated small queries under a memory cap matter: direct reads miss the
  page cache, so a warm repeat at 1% answers costs 65 ms against 48–50 through the mapping. RWF_DONTCACHE was tried and
  lost (REPORT_DISK.md on research/disk-reads).
- **Bring DESIGN.md up to date with the code** (anchors, constants, level schedule); SYSTEM.md is current.
- [x] **The column-layout code is deleted** (rust/REPORT3.md on research/colorder keeps the record). Still to do:
  delete the side branches and worktrees.

- **`dot_f16_x4` drops dimensions past the last multiple of 16** (kernel.rs:599, :671), and no error bound covers them:
  a soundness bug in the scan and direct scoring for any n that is not a multiple of 16 (768 is). Fixed on research/scan-planner
  (b79dbc8d5) and merged; `dot_i8` still has no tail (harmless while n % 32 == 0).

## Approximate path

- [x] **The old approximate mode is retired** (`knn2_dial`, `knn2_approx`, SOAR, the old dial fit, `--approx-mode`,
  `--fit-dial`, `--soar`). `knn2_fast` is the approximate mode; `ann` runs it as `--dial np:eps`. Rows measured with the
  old mode can no longer be reproduced.
- **Approximate threshold, as IVF range search means it** (faiss `range_search`): probe the `nprobe` best clusters in
  the approximate mode's order and return every visible item in them scoring at least t, decided exactly, so the answer
  holds no false positives and misses only items in clusters not probed. There is no entry point for it yet; dbknn
  measures the approximate mode as top-m instead.

- **The three steps in reports/annideas/NEXT.md:** cheaper group setup, sub-clusters (row order ruled: rows stay sorted
  by key and the approximate mode gathers each sub-cluster's rows; reports/annideas/REPORT.md), and a fused fast-scan
  pass over levels 0–3.
- **Hand over to the exact path at large answers.** Under filters the approximate mode loses to the exact index by up to
  three times once answers pass about 10% of the visible set; the planner should switch.
- **Fit the dial above k = 1,000.** Larger answers currently reuse the k = 1,000 dial.
- **Low finding from the merge review:** ties between equal scores can differ from the exact path's order.

## Code review

review/CODE_REVIEW.md holds four independent reviews of the whole implementation (architecture, duplication,
performance, correctness) at da91da6, from reading only. Ruled: keep the AVX-512 kernels and the synthetic scale path;
retire the old approximate mode (done); approximate threshold as above; compaction codes-only by default. Still to rule:
keep or drop the norm sort. The order the reviews suggest, each step keeping dbcmp's answers bit for bit:

1. [x] A synthetic end-to-end test: rust/src/v2/e2e.rs on rust/testdata/synth4k (4,000 items, 64 dimensions, 32
   clusters, fitted by tools/fit.py `--per-cluster 125`; testdata/make_synth.py regenerates it byte for byte). Threshold,
   scan, exact k-NN and the approximate mode over every cluster, against brute force over the exact products, unfiltered
   and under four visible sets (random 10%, whole clusters, three items, empty), on the index and on both compactions,
   under the default and measured plans, each left to choose and forced. At this size the default costs only scan or
   cut, so a cost set that makes the planner mix every path (direct scoring included) is tested too. Planted bugs it
   catches: the planner skipping clusters not certainly in, visible counts one short, the exact fallback dropped. Still
   to do on the owner's machine: a 50k run comparing dbcmp answer dumps (`--dump`) before and after each step.
2. [x] Deleted: the first-generation index (index.rs, query.rs and kernel.rs keep only what the current index uses),
   the column layout, `plan2_prev`, the switches the reports rejected (`jump`, `alpha0`, `--u8`, `bail`, `rdec`) and the
   dead items; 16.4k lines to 11k. `Mode2::Sweep` and the dump stay (study tools). Runs that used the removed flags
   cannot be repeated as written: results/planner/run.sh's `index:prev` and breakdown2 `--variants`. Still owed on the
   owner's machine: the 50k check and dbcmp answer dumps compared with those from before the deletions.
3. [x] The crate is a library (lib.rs: the index) and a binary (main.rs: run2, ann, scale). `v2::open(dir,
   &BuildOptions, log)` builds a queryable index; `BuildOptions` holds what the build's globals and environment
   variables held (the guarantee's N, the plain constant, the design constants, the calibration sample) and the
   build choices setup2_at applied to the parameters (profile, cluster subset, level schedule, norm sort). The disk
   globals are the `ReadPolicy` of the vectors on file. The binary's verify, dbcmp (every variant; vectors on file
   read through io_uring, the mapping and O_DIRECT) and dbtopk run on testdata/synth4k with every answer equal. Left
   for step 5: `PROF`, `ERRS` (which `Dial::fit` sets around a parallel loop, so it is unsafe beside other queries)
   and `DUMP`; and the process-wide read counters.
4. Split `Plan` into a cost model, tuning whose defaults are the measured system (`--int --gtab --flags repack`), and
   query arguments. [x] The defaults: `Plan::default()` now has the integer levels, combined tables and repack on;
   `--int`, `--gtab` and `--flags repack` restate them, and `--flags noint,nogtab,norepack` gives the float path. A
   flagless run on the 50k index answers byte for byte as the flagged one, on the integer path. [x] `Plan::disk` is
   gone: the read policy of the vectors on file says whether the planner prices their reads (`priced`, on with
   io_uring; dbcmp's `:plan=disk|mem` set it). [x] `Plan::calibrate` times the index path on the integer levels without
   collecting intervals, as queries run it; on the shared cloud machine the fitted index-path factor moved from
   0.34-0.40 to 0.43-0.44, within its run-to-run spread, so time it on the owner's machine and refit plan files.
   Still to do, with step 6 (it rewrites the same call sites): the split into a cost model, tuning and per-query
   arguments (`tol`, the forced path).
5. Move verification, profiling and dumps out of the level loop; split q.rs; one owner of full-vector scoring.
6. One `threshold` and one `knn`, each with a path (scan, exact, approximate) and one planner.
7. The correctness findings: a visible set naming an unknown entity panics; k-NN returns certified items with +inf
   scores in entity order; k = 0 panics; a non-power-of-two `--nguar` overstates `npow`.
8. The performance findings, measured one at a time: k-NN selection rebuilds a cluster's tables per refined row;
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
