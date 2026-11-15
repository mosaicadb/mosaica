# Next steps

What is open on LIMINAL as of 15 November 2026, in rough order. SYSTEM.md describes the index as it stands; each item
names the branch or report that holds its evidence.

## Merges owed onto this branch

- **Benchmark branches.** research/bench-capped (LIMINAL under a 776 MB memory cap), research/disk-reads (io_uring,
  merged reads, O_DIRECT from 8 KiB under the cap), research/filter-sweep and research/plots-refresh (filtered and
  genus-filter sweeps with ACORN, plots with the new approximate mode). Each is pushed and reviewed by its own runs;
  merge them here with a build and the 50k check.
- **research/scan-planner** (in progress): the planner chooses index or scan per query and per cluster, and a session
  can compact its visible set's codes (and optionally vectors) into one block. Review, then merge.

## Exact path

- **Never slower than a plain scan under a filter.** The scan-planner work above. The filter sweep showed the index
  losing to a plain scan of the visible rows at 1% and 10% random and at 1% genus visible.
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
