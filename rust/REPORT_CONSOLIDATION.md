# Consolidation: three merges, a smaller crate, and checks on fresh data (15 November 2026)

The research branches owed to `research/exact-vector-search` were merged, the code was cut down and restructured
following two rounds of independent review, and every change was checked for answers. The range is 546375a (before
the merges) to 2b64af2. The reviews are in `review/CODE_REVIEW.md` (the whole implementation, at da91da6) and
`review/REVIEW_2026-11-15_merges.md` (this range). What remains is in NEXT.md.

## Merges

- **research/plots-refresh** (with research/filter-sweep), 796a082: merged cleanly.
- **research/disk-reads** (with research/bench-capped), 20e1355: `scan_visible` and `run_planned` take the reader's
  structure with `npow` set on every `ThrOut2`; `run2.rs` keeps both sides' flags.
- **research/scan-planner** at b79dbc8 (with its review's fixes), ce9cf94. Full vectors are `Arc<disk::Rows>` with
  `Rows::Mem` holding the aligned `F16Rows`, so compaction shares rows and `--full-file` still maps them. `Plan` keeps
  both cost sets. `dbcmp` is one function with every variant of both branches, their cross-check of answers, the read
  modes, `--cap`, `--cold`, `--sets` and the disk columns; `--cmp-order variant` keeps the capped runs' order, and
  each pass's cold runs come just before its warm runs (840ee6e).

What the merges made, beyond either branch: `Index2::own_rows` keeps the reader, `willneed` and the cache test off an
index that shares a parent's vectors through `vrow` (its row ids are not the file's), and reader batches assert it
(26cf89c); `plan2` prices reads on file as `plan2_prev` did, only where the index's own rows are on file and their
read policy prices them.

## Restructuring

- **An end-to-end test** (0a2230a): `rust/src/v2/e2e.rs` on `rust/testdata/synth4k` (4,000 items, 64 dimensions, 32
  clusters, 3 groups; `testdata/make_synth.py` regenerates it byte for byte through `tools/fit.py --per-cluster 125`).
  It checks threshold queries, scans, exact k-NN and the approximate mode over every cluster against brute force over
  the exact products, unfiltered and under four visible sets, on the index and on both compactions, under the default
  plan, the float path, each forced to one path, and costs that make the planner mix every path. Planted bugs it
  catches: the planner skipping clusters not certainly in, visible counts one short, the exact fallback replaced by
  the f32 product. Two subtler ones change no answer at this size and pass.
- **Deletions** (a150fc5 to dd98399): the first-generation index, the column layout, `plan2_prev`, the switches the
  reports rejected (`jump`, `alpha0`, `bail`, `rdec`, `--u8`), the old approximate mode (`knn2_dial`, `knn2_approx`,
  SOAR; 9657681) and dead items. The crate went from 16.4k to 11k lines.
- **Library and binary** (4a7f3f4): `lib.rs` is the index, `main.rs` the experiments. `v2::open(dir, &BuildOptions,
  log)` builds a queryable index; `BuildOptions` replaces the build's globals and environment variable (b0842d3). The
  disk globals are the `ReadPolicy` of the vectors on file (26e8608), whose `priced` flag replaces `Plan::disk`
  (a291319).
- **Defaults** (c7737c9): the measured configuration (integer levels, combined tables, repack) is `Plan::default()`.
- **Calibration** (0f93ea3) times the index path on the integer levels without collecting intervals, as threshold
  queries run it.
- **The command line** refuses arguments it does not know (c8f8aba), and `--full-file` refuses the fitted file
  (7c87040).

Rulings by the owner along the way: retire the old approximate mode; keep the AVX-512 kernels and the synthetic scale
path; approximate threshold means IVF range search; sessions compact codes by default, codes and vectors on request;
the measured configuration is the default; the approximate path takes a recall target (NEXT.md has the design).

## Checks

`cargo test --release`: 19 passed, 1 ignored (the speed test) at every commit from 0a2230a on.

On data fetched in the cloud session (the owner's machine was not reachable): BioCLIP-2 float16 image embeddings from
imageomics/TreeOfLife-200M-Embeddings, whole row groups chosen with a fixed seed, fitted by `tools/fit.py`, with
held-out images as queries. `results/consolidation/` holds the fetch script and the commands.

| Check | Index | Result |
|---|---|---|
| verify (200 item and 200 held-out queries) | 50k, 50 clusters | 0 wrong-in and wrong-out; 0 deterministic and 0 randomised violations; 0 decisions not implied; k-NN 0 mismatches of 400 |
| `bench/oracle_thr.py` (float64) | 50k | 0 entities wrong; k-NN 0 of 80 differ |
| dbcmp dumps at 546375a, fe1b413 and e9b6f29 (40 queries, 5 shares, unfiltered and 3 visible sets) | 50k | byte-identical for every variant each binary has (40 files) |
| flagless run against the flagged one (after c7737c9) | 50k | byte-identical (32 files); 15M interval checks against 76M on the float path, so the integer path ran |
| dbcmp flat, index, idx, scan, cindex, cvindex (unfiltered and 1%) | 1M, 1,000 clusters | 500 answers identical |
| dbtopk | 1M | exact recall 1.0 at k 10 and 100; the approximate mode reaches 0.9 and 0.99 |
| dbcmp with `--full-file`, io_uring and the mapping | 1M | byte-identical to the in-memory dumps |

Not checked: a run under a memory cap (the session had no systemd user bus and only cgroup v1, so neither
`systemd-run --user` nor the binary's own cap works there); out-of-distribution queries (the text prompts need the
BioCLIP-2 text encoder); timings, which on the shared cloud machine are not comparable with the reports'.
