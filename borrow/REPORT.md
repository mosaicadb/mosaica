# Borrow or build

Saved by the coordinating session from the evaluation agent's returned text; condensed. Every number
is the agent's. Scripts and results in this directory (`rq_exact.py`, `speed.py`, `pano.py`,
`updates.py`, `*.json`, `*.log`). Searches used technical terms only.

## Summary

**Nothing meets the requirements, alone or as a drop-in substrate.** No maintained candidate returns
a certified exact threshold set under a bitmap applied first while keeping only compact codes in
memory; each has at most two of those three properties.

- **Closest on exactness:** faiss's upstreamed Panorama. Filtered range search and k-NN are exact up
  to float rounding at the boundary, filter applied before vectors are read (measured). Full float32
  vectors in RAM; little or no time saved on text queries.
- **Closest on compact codes:** faiss RaBitQ. `range_search` thresholds an estimate and misses 3–25%
  of true matches even with every list probed (measured). The IVF fast-scan variant's `range_search`
  returned almost nothing (measured; looks like a bug).
- **A certified result is achievable from faiss's 1-bit codes plus an external bound and re-rank**
  (measured, zero wrong decisions at multipliers ≥ 3), but needs full-vector reads for 7–12% of
  visible items on text queries — the design's nested levels are the part worth building.

**Substrates.** For the Rust database: none (Lance is the only Rust-native candidate; its RaBitQ is
1-bit with no error factor, exact only by brute force, and it brings its own format, versioning and
compaction). For a standalone library: faiss is the only plausible upstream home, but its persistence
(whole-index files, read-only mmap) and estimate-only API do not fit; a standalone crate is cleaner.

**Borrow** (all MIT or Apache-2.0): the fast-scan kernel and 32-item block layout; the RaBitQ
encoder and factor formulas; Panorama's level-major layout and batch pruning loop; PDX's layouts;
LEMP's algorithms (from the paper); faiss in Python as test oracle and benchmark baseline.

**Recommendation: build, borrowing components by reimplementation in Rust.** The missing piece (sound
per-item intervals, a three-way decision, nested levels) is the core of the design, and every
candidate's storage and update path would duplicate the database's own log and compaction. faiss's
fixed-rate fast-scan runs at 0.04–0.09 ns per code byte (measured), below the review's 0.2–0.6.
Report the faiss range-search bug upstream.

## Requirement by candidate

Columns: 1 exact or certifiable; 2 bitmap before data; 3 codes in RAM, vectors on disk; 4 live
updates surviving restart; 5 any model; 6 integration. M measured, S source/docs, C conjectured.

| Candidate | 1 | 2 | 3 | 4 | 5 | 6 |
|---|---|---|---|---|---|---|
| faiss RaBitQ (1.16.0, MIT) | Not as shipped: range search misses 3–25% (M); 1-bit codes + external bound + re-rank certify (M) | Partly: non-fast-scan tests selector first (S, M); fast-scan scans block first (S, M) | Partly: 104 B codes (M); vectors yours; mmap load (M) | Partly: add/remove; whole-index rewrite; mmap-loaded rejects add (M) | Met | Not for Rust: C++ + BLAS; faiss-rs pinned to 1.7.2; no RaBitQ in C API (S, M) |
| faiss Panorama | Met up to float rounding (M) | Met (M) | Not met: float32 in RAM | Partly | Met | As faiss |
| RaBitQ-Library (0.8.4, Apache-2.0) | Not met: probabilistic, top-k only (S) | Not met: no filter (S) | Partly | Partly | Met | Partly: frozen C++17 core API with `Factors{add, rescale, error}` |
| PDX (MIT) | Partly: PDX-BOND exact on float32 (S) | Partly (S) | Not met: in memory | Partly | Met | Partly: C++ |
| LEMP (Apache-2.0) | Met: exact above-threshold and top-k (S) | Not met (C) | Not met (C) | Not met: static (C) | Partly (C) | Not met: research tool |
| FEXIPRO | Met: exact top-k (S) | Not met | Not met | Not met | Partly | Not met: no licence file |
| Lance / LanceDB (Apache-2.0, Rust) | Partly: exact only by flat scan (S) | Met in flat sub-index (S) | Partly: IVF_RQ 1-bit, no error factor (S) | Met in its own model (S) | Partly: d divisible by 8 | Partly: own format and versioning; prefilter trait internal |
| VectorChord (AGPLv3 / ELv2) | Not met (S) | Partly (S) | Met | Met via Postgres | Met | Not met: Postgres; licence |
| pgvector | Partly: exact only without index (S) | Partly | Not met | Met | Partly | Not met: Postgres |
| Qdrant | Partly (C) | Partly (C) | Partly (C) | Met (C) | Met | Not met: server |
| Milvus / Knowhere | Partly: FLAT exact (S) | Partly (C) | Partly (C) | Met (C) | Met | Not met |
| DiskANN3 (MIT, Rust) | Not met: graph | Partly (S) | Met (S) | Met (S) | Met | Partly: best architectural fit, wrong algorithm class |
| USearch | Partly: brute force only (S) | Partly | Partly | Partly | Met | Partly |
| Tribase | Lossless pruning (S) | Not met (C) | Not met | Not met | Met | No code or licence found |

Also found: `sochdb-vector` (Rust, "certified" mode with `max_error` defaulting to 0.0 pending
calibration; AGPL-3.0; one author) — not usable.

## Hands-on results (measured; 200k BioCLIP, n = 768; 60 text, 40 item queries; 10% random visible
set via `IDSelectorBitmap`; one thread; machine shared, timings pessimistic)

**faiss RaBitQ `range_search` is not exact** (text, 0.1% answer, 1,140 true matches; no invisible
item ever returned):

| Index | Missed | Extra |
|---|---|---|
| IndexRaBitQ 1-bit (104 B) | 279 (24%) | 2,889 |
| IndexRaBitQ 4-bit (404 B) | 94 (8%) | 109 |
| IndexRaBitQ 9-bit (884 B) | 34 (3%) | 4 |
| IndexIVFRaBitQ 1-bit, all lists | 263 (23%) | 1,196 |
| IndexIVFRaBitQFastScan, all lists | 1,138 (returned 19) | 17 |
| IndexRaBitQFastScan | range search not implemented | — |

**Certifying from faiss's codes:** 1-bit estimate error / paper sd has sd 0.97 (text), 0.95 (items),
largest 5.05 / 4.94 over 6M / 4M pairs. Classify by est ± m·w, re-rank the undecided (0.1% answer):

| m | 1.9 | 3 | 4 | 6 | 7 | 9.35 |
|---|---|---|---|---|---|---|
| Text undecided, % of visible | 2.0 | 3.1 | 4.2 | 6.9 | 8.4 | 12.0 |
| Text wrong decisions | 11 | 0 | 0 | 0 | 0 | 0 |
| Items undecided, % | 0.7 | 1.5 | 2.4 | 4.6 | 5.8 | 8.7 |
| Items wrong decisions | 0 | 0 | 0 | 0 | 0 | 0 |

With 104 B resident this reads 106–184 B per item of f16 on text (vs the design's 3.6 B per item of
disk with levels through 96 B resident). What faiss does not expose for certification: a bound for
1-bit codes (`f_error` stored only when `nb_bits > 1`), any bound on multi-bit estimates, a
configurable confidence constant (1.9 is baked in), a three-way result, a seeded rotation with a
proved tail, sound arithmetic (float32, no outward rounding, 4-bit query quantisation by default,
uint8 fast-scan tables).

**Scan speed** (one thread, 200k, k = 10):

| Scan | ns per item | ns per code byte |
|---|---|---|
| PQ fast-scan 32 B | 2.12 (1.29 batched ×40) | 0.066 (0.040) |
| PQ fast-scan 96 B | 4.39 (3.44 batched) | 0.046 (0.036) |
| RaBitQ fast-scan 1-bit (104 B) | 8.84 | 0.092 |
| RaBitQ 4-bit two-stage (404 B) | 19.5 | — |
| RaBitQ 1-bit, 4-bit query | 23.2 | 0.24 |
| RaBitQ 1-bit, float query | 50.5 | 0.53 |
| RaBitQ 9-bit two-stage (884 B) | 82 | — |
| Flat float32 (3,072 B) | 121–167 | 0.04–0.05 |

Selector: non-fast-scan RaBitQ costs ~2.8 ns per item for the bitmap test and reads codes only for
visible items; fast-scan scans every block then filters (no speed-up at 10% visible).

**faiss Panorama** (100k): mismatches only within 10⁻⁵ of the threshold (float summation order); k-NN
recall@10 1.0. Text 0.1%: 88 ns per item unfiltered, 11 ns at 10% visible; dimensions scanned 20–32%
(text), 5–17% (items); text 68–154 ns per item vs flat 121–167; reads 600–980 B per item of float32.

**Updates:** IndexIVFRaBitQ add/remove and save/load work; `IO_FLAG_MMAP` lists reject later `add`;
`IO_FLAG_MMAP_IFC` aborts on `add`. Persistence is a whole-file rewrite.

## Borrowable components

| Component | From | Licence | Adapting it |
|---|---|---|---|
| 4-bit fast-scan kernel, 32-item blocks, nibble tables (AVX2 / AVX-512 / NEON) | faiss `impl/fast_scan/`; RaBitQ-Library `fastscan`; Lance `simd::dist_table` (Rust) | MIT / Apache-2.0 / Apache-2.0 | Reimplement in Rust; add outward-rounded table quantisation and a per-block error term; test against faiss. |
| RaBitQ / Extended RaBitQ encoder and factor formulas | RaBitQ-Library core API; faiss `RaBitQuantizer.cpp`, `RaBitQUtils.cpp` | Apache-2.0 / MIT | Port the formulas for the randomised levels. |
| Level-major layout with batch pruning and compacted active list | faiss `impl/Panorama.h` | MIT | Model for the deterministic levels' scan loop; add quantised levels and sound bounds. |
| Dimension-major layouts, 8-bit dot kernels | PDX | MIT | Layout ideas for int8 centres and bases. |
| Length-sorted buckets, incremental pruning, seed-then-threshold top-k | LEMP | Paper; Apache-2.0 code | Algorithms only. |
| Fast k-means | PDX SuperKMeans; faiss `Clustering` | MIT | Build-time tool. |
| Test oracle and baselines | faiss Python | MIT | Conformance and benchmarks only. |

Not borrowable: FEXIPRO (no licence), VectorChord and sochdb-vector (AGPL / ELv2), Tribase (no code).

## Risks by route

- **Build, borrowing by reimplementation (recommended):** most engineering (kernels per instruction
  set, proofs to maintain); mitigated by small well-specified layouts and faiss as a differential
  oracle; low licence risk.
- **Link faiss or RaBitQ-Library:** breaks the single Rust binary (C++, BLAS, OpenMP); own the
  bindings; duplicated persistence; selector over faiss ids rather than Roaring over entity ids;
  fast-scan reads invisible items' codes; certification on changing internals and a broken
  range-search path.
- **Build on Lance:** second storage format, manifest and compaction; large dependency tree; RaBitQ
  without a bound; exact layer still from scratch on an internal API.
- **Contribute upstream to faiss:** small items plausible (bug report, configurable epsilon, 1-bit
  bounds, three-way range handler); the full design unlikely to be accepted and still no Rust path.
- **Standalone library:** a Rust crate with a C API, filter as a bitmap over its own row ids; keep
  scope to codes, kernels and certified scans, leave storage to the host.
