# Next steps

What is open on LIMINAL as of 15 November 2026, in rough order. SYSTEM.md describes the index as it stands; each item
names the branch or report that holds its evidence.

## Merges owed onto this branch

- [x] **Benchmark branches, merged.** research/plots-refresh (with research/filter-sweep) and research/disk-reads (with
  research/bench-capped). The release build passes, and `cargo test --release` gives 15 passed and 1 ignored. Still to do
  on the owner's machine before this counts as checked: the 50k check (rust/results/const/check.sh), one `dbcmp`
  threshold run and one approximate `dbtopk` run on the 1M index, and one capped `--full-file` run through
  bench/threshold-db/capped/run_ours.sh at a single answer share, whose answers must match the uncapped dumps.
- **research/scan-planner** (844dfb52a, under independent review): the planner chooses a whole-query scan, direct
  scoring of a cluster's visible rows, or the index per cluster, and a session can compact its visible set's codes and
  f16 vectors into one block. Merge once the review passes. Its filtered rows are being remeasured on a quiet machine
  for the plots on research/plots-refresh.

## Exact path

- **Never slower than a plain scan under a filter: mostly done.** The scan-planner work above
  (bench/threshold-db/results/planner/REPORT_PLANNER.md on that branch; times from a loaded machine, interleaved). With
  the visible set compacted per session (codes and vectors), the planned query beats LIMINAL's own scan at every answer
  share under random 1%, 10% and 50% visible (0.44–0.62 against 1.07–1.25 ms; 9.7–10.4 against 11.5–12.5 ms; 20–46
  against 47–52 ms), and under 241 and 964 genera up to answers of 10% and 30%. Uncompacted, the planner alone fixes only
  random 1%. Answers equal the scan's bit for bit. Still open:
  - **Small, cache-warm genus sets** (19 genera, 1%): 0.65–0.98 ms against 0.44–0.66 for the plain scan. The cost model
    prices a cached scan at 0.10 µs per row against 0.06–0.08 actual, so it picks the index on 345 of 525 runs.
  - **The plain scan beats the compacted scan by about 20% under genus sets,** unexplained: both read contiguous rows
    with the same kernel.
  - **The largest answers under 241 and 964 genera,** 5–14% behind the scan.
  - **A session switch for compaction.** It is opt-in through `Index2::compact` with no session-level setting. It costs
    22 ms and 24 MB at random 1%, 120 ms and 186 MB at 10%, and 576 ms and 913 MB at 50% (codes alone: 12 ms and 9 MB,
    38 ms and 33 MB, 162 ms and 147 MB). The suggestion is codes and vectors up to about 10% visible, codes alone above.
  - **The uncompacted index path over sparse clusters** costs more than the model predicts (22 ms forced at random 1%).
- **Store the codes at build.** They are rebuilt at every open (2.5–3.5 minutes at 1M); at a billion items they must
  be stored.
- **A hot-row cache over O_DIRECT,** only if repeated small queries under a memory cap matter: direct reads miss the
  page cache, so a warm repeat at 1% answers costs 65 ms against 48–50 through the mapping. RWF_DONTCACHE was tried and
  lost (REPORT_DISK.md on research/disk-reads).
- **Bring DESIGN.md up to date with the code** (anchors, constants, level schedule); SYSTEM.md is current.
- **Delete the column-layout code** (rust/src/v2/col.rs and its hooks; off by default and slower, rust/REPORT3.md on
  research/colorder keeps the record), then delete the side branches and worktrees.

## Approximate path

- **The three steps in reports/annideas/NEXT.md:** cheaper group setup, sub-clusters (needs a ruling on row order inside
  clusters, which touches the exact path's layout), and a fused fast-scan pass over levels 0–3.
- **Hand over to the exact path at large answers.** Under filters the approximate mode loses to the exact index by up to
  three times once answers pass about 10% of the visible set; the planner should switch.
- **Fit the dial above k = 1,000.** Larger answers currently reuse the k = 1,000 dial.
- **Low findings from the merge review:** ties between equal scores can differ from the exact path's order, and
  `dot_f16_x4` ignores dimensions past the last multiple of 16 (harmless at 768; nothing checks it).

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
