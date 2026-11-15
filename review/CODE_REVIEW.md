# LIMINAL review: architecture, duplication, performance, correctness

Four independent reviewers read the whole implementation at da91da6 (research/exact-vector-search with plots-refresh,
disk-reads, bench-capped and scan-planner merged), each with one lens. They read only: nothing was built or run, so
every cost below is an estimate to measure. The four full reports follow this synthesis.

## Where they agree

- **About half the crate can go without touching an answer of the measured configuration.** The first-generation
  index (`index.rs`, `query.rs`, most of `kernel.rs`, `synth.rs`, the non-`--v2` half of `main.rs`: about 3.5k lines)
  exits without AVX-512, which the measuring machine lacks; the current index borrows 10 to 15 items from it. The
  column layout and its hooks (about 1.2k), `plan2_prev` (about 0.16k), the superseded approximate modes
  (`knn2_dial`, `knn2_approx`, SOAR, the old dial fit) and the switches the reports rejected (`jump`, `alpha0`,
  `--u8`, `bail`, `rdec`, `Mode2::Sweep`) account for most of the rest. Estimates: 16.4k lines to 9k to 11k.
- **The measured system is not the default.** Every timing script passes `--int --gtab --flags repack`;
  `Plan::default()` has all three off.
- **The 2x3 matrix (threshold, k-NN; scan, exact, approximate) is right; the code does not show it.** There are five
  places that plan, six public k-NN functions, three threshold ones, and no approximate threshold entry point. The
  proposal is `threshold(q, t, tol, Path)` and `knn(q, k, Path)` with `Path::{Scan, Exact, Approx}` and one planner.
- **`Plan` does four jobs** (35 fields): cost model, a forced mode, the caller's tolerance, and about 15 experiment
  switches. Split it into a cost model, a tuning struct whose defaults are the measured system, and query arguments.
- **Full-vector scoring is written six or seven times**, the support-table bound five times, and the cut helpers are
  copied from v1. One owner of vector scoring (`VecStore` with `Mem`, `File`, `Remap`) would replace `Full2`,
  `disk::Rows`, `vrow` and the reader dispatch.
- **No library boundary.** Only a benchmark function (`run2::setup2_at`) produces a queryable index, and settings
  arrive through 13 globals and 8 environment variables (the disk flag lives in both `plan.disk` and `disk::URING`).
- **No tests cover the query paths.** Every query-level check needs the 1M or 50k data. A synthetic end-to-end test
  (about 2k rows: threshold and k-NN x scan and exact x visible set and compaction, against brute force) should come
  before any refactor.

## Correctness

No way was found for the exact path to return a wrong answer under its stated conditions (n a multiple of 32 up to
2048, existing entities, k >= 1). The three merges kept the certified constants, the one-sided budget, `select_knn`,
`npow` on every result, N from the row count, and every benchmark mode and flag. Found:

1. A visible set naming an entity the index lacks panics the query (`Vis2::new` should skip it).
2. k-NN returns certified items with +inf scores in entity order: the set is right, the order and scores are not, and
   `oracle_dump` writes that order.
3. k = 0 panics; a non-power-of-two `--nguar` records an `npow` claiming up to twice the coverage; with `--alpha0`
   (off by default) refinement uses another alpha than the sweep, so epsilon can double; `dot_i8` has no tail.
4. From the merge, fixed in da91da6: the planner priced disk reads on compacted indexes whose vectors are in memory,
   and NEXT.md wrongly said codes-and-vectors compaction used the reader.
5. From the merge, still open (timing only): `Pre::Direct` and the filtered scan read on-file vectors through the
   mapping but are priced as sequential reads; codes-only compaction's faults are unpriced; capped/run_ours.sh's
   plan.txt lacks plan2's constants; codes-and-vectors compaction under a 776 MB cap can be killed for a large set.

## Performance (largest expected effects)

1. **Exact k-NN selection** refines one row at a time through `row_interval2`, rebuilding the cluster's state and
   tables each time (estimated 10 to 50 us a call against 0.1 to 0.2 us for an exact score). Batch per cluster, or
   score exactly.
2. **Sparse clusters on the uncompacted index** pay gathers from up to 32 blocks per level that calibration never
   prices: the likely cause of the 22 ms forced-index miss at random 1%.
3. **Cost-model gaps:** no term for full vectors the index path still reads (fits the 16 to 19% loss under 241
   genera); the scan constant timed over 1.5 GB while a 15 MB session stays in cache (the 0.10 against 0.06 to 0.08
   us per row miss); `answer_share` run twice per plan and unpriced; calibration times the float path under `--int`
   (`want_iv: true`).
4. **Plain scan 20% faster than the compacted scan** compares two code paths over different memory (the npy mapping
   against a late heap copy); page size and layout are the likely cause, not the kernel.
5. **At a billion items:** the K x K support table (8 TB at 1M clusters), about 65k groups, centres stored three times
   in f64, an N/8 bitset per session, and result bitsets sized by entity count.

## Decisions that are yours

- Keep the norm sort, the AVX-512 kernels and the synthetic scale path, or drop them (no native measurement found).
- Retire the old approximate modes: their benchmark rows could then no longer be reproduced.
- What approximate threshold means: top-m `knn2_fast`, or the levels walk with a dial.
- Whether a compacted session's vectors may exceed the cap, or compaction under a cap is codes-only.

## Suggested order (each step builds, and dbcmp answers stay bit-identical)

1. Safety net: the synthetic end-to-end test, and a 50k run comparing dbcmp answer dumps before and after each step.
2. Deletions: the column layout, `plan2_prev`, rejected switches, superseded approximate modes (after the decision).
3. Retire v1, moving the live items.
4. Split library and binary; options instead of globals; `Engine::open`.
5. Split `Plan`, with the measured system as the default.
6. Move verification, profiling and dumps out of the level loop behind an observer type.
7. Split `q.rs`; then `Visible`, `Model`/`Segment`/`VecStore` and one scoring path; then the public API.
8. Then the performance items, measured one at a time.


---

# Appendix: architecture review


Reviewer scope: the structure of `rust/src/` as a whole (layering, entry points, state, representation), not
performance, correctness or line-level duplication, which other reviewers cover. Line numbers are against the
read-only checkout at `/home/user/evs`. "Verified" means I read the code at the cited lines; "inferred" means I
reasoned from what I read but did not trace every path or run anything (nothing was built or run).

## 0. Summary

The 2×3 query matrix the owner describes is a sound shape, but the code does not have that shape. What it has:

- **Two generations of index in one binary.** v1 (`index.rs`, `query.rs`, `kernel.rs`, `synth.rs`, most of
  `main.rs`, half of `scale.rs`, about 3.5k lines) only runs on AVX-512 (`main.rs:829-832`). The measuring machine
  has no AVX-512, so for the owner it is dead code that v2 still imports about a dozen items from.
- **No library.** The crate is a single binary. Opening an index (fit parameters, build, re-pack calibration, support
  table, cost-model calibration, approximate-mode fit, moving vectors to a file) happens in the benchmark module
  `run2::setup2_at` (`run2.rs:164-319`). The library is configured through 13 process-global statics and 8
  environment variables, which the CLI sets.
- **One struct for four jobs.** `Plan` (35 fields, `q.rs:1968-2005`) holds the timed cost model, a query parameter
  (`tol`), a forced-mode override, disk constants, and about 15 experiment switches. Its defaults are not the
  measured system: SYSTEM.md's numbers come from `--int --gtab --flags repack` (`rust/results/const/check.sh`), and
  `Plan::default` has all three off (`q.rs:2008`).
- **Fan-out of entry points.** Five places make planning decisions (`plan2`, `plan2_prev`, `Plan::scan_first`, the
  inline rule in `knn2_fast`, the scan band). Six public functions answer k-NN (`knn2`, `knn2d`, `knn2_dial`,
  `knn2_fast`, `knn2_approx`, `scan_knn`). Threshold has three (`threshold2`, `scan_visible`, `run_planned`), and the
  last is called from outside with hand-built plans.
- **Experiments and instrumentation inside the hot kernel.** `run_cluster_from` is 780 lines (`q.rs:1104-1883`). It
  takes `verify`, `mode` (Thr, Interval or Sweep), `first_decide` and `resume` (the latter for the rejected columnar
  store), and reads `PROF`, `ERRS` and `DUMP` globals. `Stats` has 30 fields, half of them diagnostics.
- **Overloaded vector representation.** `Full2::{Real{map, off, rows: Arc<disk::Rows>}, Synth}`, `Index2::vrow`,
  `own_rows()`, `compacted()` and an empty `ent_row` together express "where a row's vector is" and "is this a session
  subset". A compacted index is a full `Index2` with 23 call sites that special-case it.

The largest simplifications, in order: delete v1 (−3.5k); delete the columnar store and its hooks (−1.2k); create a
library boundary with `Engine::open` and options structs in place of globals (moves about 0.4k and removes all 13
statics); split `Plan` and reduce the k-NN and planner entry points to one per cell of the matrix (−0.5k); pass
instrumentation in as a generic observer (removes about 70 runtime branches from the kernel and 4 globals); split
`Index2` into a shared `Model` and a per-row-set `Segment` with one `VecStore` (removes `vrow`, `Full2`, the dead
`map`/`off` and most compaction special cases). Section 3 gives a target layout and an order of steps that keeps
the build green after each one.

---

## 1. Map of the actual structure

### 1.1 Modules (verified by reading outlines and imports)

| File | Lines | What it holds | Generation / role |
|---|---|---|---|
| `main.rs` | 866 | CLI dispatch; the v1 experiment modes (`verify`, `levels`, `e2e`, `batch`, `disk`, `filtered`, lines 22-740); v1 `setup`; `params_with_profile`, which is public | v1 bench, plus v2 dispatch (742-828) |
| `index.rs` | 1306 | `Params`/`load_params`/`apply_profile`, `RealSource`, v1 `GroupPlan`/encoder/`build`/`build_table`, constants `DA`, `SEED`, `item_scale` | v1, with about 210 lines v2 uses |
| `query.rs` | 1043 | v1 query (prep, cut, threshold, batch, knn, `Vis`), plus `Cut`, `next_up_if_below`, and the brute-force `Flat`, `topk_f16`, `thresholds_for` | v1, with about 200 lines v2 or the bench use |
| `kernel.rs` | 794 | `Stats` (shared by v2), the v1 AVX-512 block kernel `run_block`, `dot_f16`/`dot_f16_x4`/`dot_i8` (AVX-512 and AVX2), `basis_matvec` | mixed |
| `synth.rs` | 151 | v1 synthetic source | v1 |
| `scale.rs` | 469 | `run` (v1 scale test), `run2` (v2 scale test) | bench |
| `derive.rs` | 170 | rate design and bit allocation from the profile | shared, used by v2 build |
| `util.rs` | 322 | npy, f16, exact integer dot, Haar rotation, timers, `pin` | shared |
| `simd.rs` | 235 | AVX-512 names emulated on AVX2 | v2 |
| `ann.rs` | 233 | ANN-benchmark modes (`truth`, `run`) | bench |
| `run2.rs` | 1843 | **index open** (`setup2_at`), CLI parsing (`arg_*`, `plan_flags`), every v2 experiment mode (`verify2`, `levels2`, `e2e2`, `breakdown2`, `filter2`, `oracle_dump`, `scanbench`, `dbcmp`, `dbtopk`, `dbknn`) | bench, plus library setup |
| `v2/mod.rs` | 1872 | codes (det/rand), field packing, `GroupPlan2`, `Meta2`, `Cl2`, `F16Rows`, `Full2`, `Index2` (+`compact`), `Source2`/`Real2`, encoder, `build2`, `build_table2`; 4 globals | index + build |
| `v2/q.rs` | 3931 | profiling constants and globals (1-60); `Prep2`/`GQ2`; SIMD kernels (170-320, 497-522); group and cluster setup; cut and sorted range, `NuCut`; `Vis2`; exact full-vector decisions; **the level loop** (874-1883); `calibrate_repack`; `Plan` + calibration (1954-2289); scan (2291-2509); planners `plan2`/`plan2_prev` (2511-2942); `threshold2`/`run_planned` (2944-3140); exact k-NN, `Approx`, `Dial`, approximate k-NN (3158-3784); `Synth2` (3835-3931) | everything at query time |
| `v2/kern.rs` | 1202 (≈300 tests) | table building (`build_lut`, `group_tab`/`centre_tab`/`combine_lut`), gather and re-pack, integer level kernel | kernels |
| `v2/col.rs` | 1168 | columnar store (off by default, rejected in REPORT3) | experiment |
| `v2/disk.rs` | 446 | `Rows` {Mem, Map}, io_uring `Reader`, **cgroup/proc probes**, 8 config/counter atomics | storage + bench probes |
| `v2/tail.rs` | 284 | certified constants | clean, self-contained |
| `v2/setup.rs` | 106 | `qv_exact_r` kernel variants (one is used by `q::qv_exact` on AVX2) + `qvbench` | kernel + bench |

There is no `lib.rs`. Every module is `pub` inside one binary, and `run2.rs` glob-imports `crate::v2::*` and
`crate::v2::q::*`, so nothing marks which parts are the index's API.

### 1.2 How an index comes to exist (verified, `run2.rs:164-319`)

`main` → `run2::setup2("run2")` → `setup2_at(tag, dir, flags)`:

1. CLI flags write globals: `NGUAR`, `CSG_PLAIN`, `DESIGN_C` (`v2/mod.rs:24-29`), `BUILD_COL` (`mod.rs:915`). `--rlv`
   rewrites `p.meta`.
2. `load_params` (v1 `index.rs`). The vector file is opened **three times** (`xs`, `src`, `f2`; `run2.rs:212-214`).
   One mapping goes into `Full2::Real.map` and stays alive for the life of the index.
3. `build2(p, prof, src, full, nanch, split, nusort, log)` (`mod.rs:1516`). Then the caller patches the result:
   `ix.repack = calibrate_repack(&ix)` (`run2.rs:223`), and `build_table2(&mut ix, k)` unless `nusort`
   (`run2.rs:225`). Anchors, `hi`, `lo` and `repack` are invalid until the caller does this.
4. `Plan` is assembled from flags, `plan_flags`, `--plan-file` or `Plan::calibrate`, and `plan_flags` again
   (`run2.rs:240-261`).
5. `pseudo_queries`, `Approx::fit`, optionally `Approx::second_assignment` and `Dial::fit` (`run2.rs:265-283`).
6. `--full-file` moves the vectors to a file and sets the `WILLNEED`, `URING`, `DIRECT_MIN` and `DONTCACHE_MIN`
   globals (`run2.rs:285-305`). `plan.disk` is then derived from `URING` (`run2.rs:310`).
7. Everything goes into `Ctx2 { p, ix, xs, ent_xs, items, texts, plan, approx, dial, pseudo }`. The benchmark context
   is the only place that holds a complete, queryable index.

### 1.3 The 2×3 matrix as implemented (verified)

All queries start from `prep2(ix, q)` (`q.rs:141`). It returns `Prep2`, which holds the rounded query, a lazy anchor,
lazy per-group setups, **and `dial`**, the approximate mode's per-level factors (`q.rs:86`). An optional `&Vis2`
carries the viewer's set, and `None` means everything.

**Threshold, exact index path.**
`threshold2(ix, pr, t, &Opts2{vis, verify, want_iv, plan})` (`q.rs:2944`)
→ `plan2` (`q.rs:2572`) or `plan2_prev` when `plan.prev` (`q.rs:2788`)
→ `Planned::Scan` → `scan_visible`, or `Planned::Run{pre, use_idx, first_decide, cut_cyc}`
→ `run_planned` (`q.rs:2968`): `setup_groups`, then a parallel fold over clusters on `Pre::{Skip, In, Direct, Open}`
→ for `Open`+`use_idx`: `open_cluster` (`q.rs:2952`; copies 6 `Plan` switches into `CQ2`, `enable_int`)
→ `run_cluster` (`q.rs:1096`) or `col::run_col` when `plan.col`
→ remaining rows to `decide_full` or the io_uring `Reader` (dispatch at `q.rs:3098-3110`)
→ `ThrOut2.rows` → `to_bitmap2` (`q.rs:3142`), which the caller has to call.

**Threshold, scan.** `scan_visible(ix, pr, t, vis, want_exact, packed)` (`q.rs:2331`) →
`decide_full` / `decide_full_packed` / `decide_full_from` through the `Reader`, with a dispatch that duplicates the
one in `run_planned` (`q.rs:2336-2390`). The bench's "flat" variant is a fourth implementation: v1
`query::Flat::threshold` over the source file in numpy order (`run2.rs:1391-1397`).

**Threshold, approximate.** There is no threshold entry point. `dbknn` answers "approximate threshold" as approximate
top-m, with m set to the exact answer's size (`run2.rs:1668-1683`). `Prep2.dial` would make `threshold2`
approximate if set, but no caller sets it for a threshold query (inferred from grep: `pr.dial =` appears only at
`q.rs:3582` and `q.rs:3705`, both k-NN).

**k-NN, exact index path.** `knn2` → `knn2d(..., dial=None)` (`q.rs:3687`)
→ `plan.scan_first` (the old planner's rule, `q.rs:2284`) may return `scan_knn` directly
→ `seed2` (`q.rs:3461`; `probe` runs `run_cluster` in `Mode2::Interval(3)`)
→ `plan2` at τ₁ with `want_iv: true` → `scan_visible` or `run_planned` (the integer path is disabled by `want_iv`)
→ `select_knn` (`q.rs:3748`), with `row_interval2` refinement, which rebuilds `State` and `CQ2` per row and level
(`q.rs:3171-3189`).

**k-NN, scan.** `scan_knn` (`q.rs:2402`) → v1 `query::topk_f16`.

**k-NN, approximate.** Three implementations:
`knn2_fast` (best-first over `Approx` order, `Dial`, its own per-cluster planning rule at `q.rs:3611` and `q.rs:3660`;
the default, "new");
`knn2_dial` (`Vis2::restrict` to the probed clusters, then `knn2d` with a dial; "old", `--approx-mode old`);
`knn2_approx` (IVF + re-rank at a fixed level; only `ann.rs` calls it).

**Where the planner sits.** It is spread out. `plan2`/`plan2_prev` decide per threshold query. `Plan::scan_first` is
a pre-check that `knn2d` and `plan2_prev` call. `knn2_fast` compares `full_us` with `index_us` inline.
`Plan::calibrate`/`calibrate_disk`/`scan_band` time the constants at open. `calibrate_repack` times the kernel's
re-pack thresholds and stores them in `Index2.repack`. The planner's output (`Planned::Run{pre, use_idx,
first_decide, cut_cyc}`) is a public, loosely typed tuple. `Plan::calibrate` builds `Pre` vectors by hand and calls
`run_planned` directly (`q.rs:2127-2130`).

### 1.4 Benchmark and experiment concerns inside the index (verified)

| Kind | Where |
|---|---|
| Global atomics set from the CLI | `v2/mod.rs:24,26,29,915` (`CSG_PLAIN`, `NGUAR`, `DESIGN_C`, `BUILD_COL`); `v2/disk.rs:10,196,211,214,218` (`WILLNEED`, `URING`, `DIRECT_MIN`, `DONTCACHE_MIN`, `QD`); `v2/q.rs:49,52,54,55` (`PROF`, `ERRS`, `DUMP`, `DUMP_TAG`) |
| Global counters | `disk.rs:198-200` (`WAIT_NS`, `NREADS`, `NBYTES`) |
| Environment variables read by library code | `VEXACT_CAL_PER_GROUP` (`mod.rs:1578`), `VEXACT_COLB/COLCK/COLEN/COLKEY/COLDBG` (`col.rs:85,356,875,881,886`), `NB` (`kern.rs:1041`, test) |
| A global toggled by library code | `Dial::fit` stores `ERRS = true`, runs a parallel loop, then stores `false` (`q.rs:3393-3403`). Any concurrent query also records errors meanwhile. |
| Alternatives kept "for measuring against" | `plan2_prev` (`q.rs:2787`), `knn2_dial`, `knn2_approx`, `Plan.{alpha0, gtab, int_det, repack, bail, rdec, jump_level, pscan, direct, prev, col, colq, collazy}`, `Index2.split` (`--u8`), norm sort (`nusort`/`nukey`/`NuCut`, 46 references), columnar store (`col.rs` + about 50 hooks), `Approx::second_assignment` (SOAR) |
| Benchmark modes inside the kernel | `Mode2::Sweep` (only `levels2` uses it, `run2.rs:572`); `verify` (19 branches in `run_cluster_from`); the dump writer (`q.rs:1683-1700`) |
| Profiling | 25 phase constants (`q.rs:15-48`); 53 `tick()` calls in `q.rs`; `Stats.cyc[32]`, `lcyc`; `KnnOut2.t_seed/t_sweep/t_sel` from `Timer` inside library code |
| Benchmark-only OS probes in a library module | `disk::{cg_report, cap_self, cgroup_dir, io_read_bytes, major_faults, cpu_s, resident_of}` (`disk.rs:139-208`) |
| Kernel benchmark in the library | `v2/setup.rs::qvbench`, plus 12 `qv_exact_r` instantiations |
| Synthetic data inside the index | `Full2::Synth`, `Synth2`/`SynthSrc` in `q.rs:3835-3931`, branches in `exact_value2`, `exact_ge2`, `run_cluster_from`, `answer_share`, `knn2d` |
| CLI parsing reaching into library behaviour | `run2::pool` reads `--pin` on every call (`run2.rs:21-24`); `data_dir()` exits the process (`run2.rs:52-61`); `main::params_with_profile` reads `--profile` |

---

## 2. Findings, ranked by how much simpler the code gets

Each finding gives the evidence, what is structurally wrong, a proposal, and a rough size.

### F1. v1 is still in the crate, unreachable on the measuring machine, and v2 leans on it (−≈3.5k lines)

**Evidence (verified).** `main.rs:829-832` exits unless the CPU has AVX-512. The owner's machine (5900X, SYSTEM.md
intro) does not. v1's block kernel is `#[target_feature(enable = "avx512f,...")]` with no AVX2 fallback
(`kernel.rs:298`). v2 imports from v1:
`index::{Params, load_params, item_scale, DA, SEED, RealSource}` (`v2/mod.rs:32`, `mod.rs:1149`);
`query::{Cut, next_up_if_below, topk_f16, Flat, thresholds_for}` (`q.rs:7,1194,1196,2403`; `run2.rs:5`);
`kernel::{Stats, dot_f16, dot_f16_x4}` (`q.rs:6,859,2137,2432,...`).

**What is wrong.** Every reader has to work out which half of `index.rs`, `query.rs` and `kernel.rs` is live. The
shared types live in the dead generation's files, and v2's names carry a `2` suffix (`Index2`, `Prep2`, `GQ2`,
`CQ2`, `Vis2`, `ThrOut2`, `KnnOut2`, `Meta2`, `Cl2`, `Full2`, `Source2`, `Real2`, `Items2`, `GroupPlan2`,
`ClusterCode2`, `Synth2`, `threshold2`, `knn2*`, `plan2`, `prep2`, ...) only to avoid clashing with v1.

**Proposal.** Move the live pieces first:
`Params`/`load_params`/`RealSource`/`item_scale`/`DA`/`SEED` → `params.rs`;
`Cut` and `next_up_if_below` → the cut module;
`topk_f16` → the scan module;
`Stats` and the `dot_*` kernels → `kern`;
`Flat` and `thresholds_for` → a bench `oracle.rs` (they are a test oracle, not index code).
Then delete `index.rs` (the rest), `query.rs` (the rest), `kernel.rs`'s `Mode`/`BlockOut`/`run_block`/`basis_matvec`,
`synth.rs`, `scale::run`, and `main.rs:22-740` plus the v1 dispatch. Drop the `2` suffixes afterwards (mechanical).

**Size.** `main.rs` ≈ 750, `index.rs` ≈ 1,090, `query.rs` ≈ 840, `kernel.rs` ≈ 490, `synth.rs` 151, `scale::run`
≈ 210: about 3,500 lines removed. About 450 lines are moved.

### F2. No library boundary: opening an index is a benchmark function, configured by globals (moves ≈0.4k, removes 13 statics)

**Evidence (verified).** §1.2. `setup2_at` (`run2.rs:164-319`) is the only code that produces a complete index.
`build2` returns an `Index2` that is incomplete until the caller sets `repack` and calls `build_table2`
(`mod.rs:1783` sets `repack: vec![0.6; ...]` and `anchors: Arc::new(Vec::new())`). Build-time choices come from
globals: `NGUAR` (`mod.rs:1522`), `CSG_PLAIN` (`mod.rs:1554`), `DESIGN_C` (read in `GroupPlan2::new_inner`,
`mod.rs:553`), `BUILD_COL` (`mod.rs:623`). Disk read policy also comes from globals: `Reader::new` reads `URING`,
`DIRECT_MIN`, `DONTCACHE_MIN` and `QD` (`disk.rs:288-292`), and `Rows::willneed` reads `WILLNEED` (`disk.rs:107`).

**What is wrong.**
- Nothing outside `run2` can open the index, so any second consumer (a server, a test, the Python side) has to copy
  `setup2_at`.
- The globals make two indexes in one process share configuration. That is exactly what `e2e2`'s
  `--index-b`/`--index-c` and `dbcmp`'s read modes do: `dbcmp` flips `disk::URING`/`QD`/`DIRECT_MIN` between
  variants (`run2.rs:1323-1337`).
- `plan.disk` and `disk::URING` are two sources of truth for one fact. `setup2_at:310` derives one from the other,
  and `dbcmp:1330-1339` must set both by hand.

**Proposal.**
1. Make the crate `lib` + `bin`. Library: params, build, index, query, storage, kernels. Binary: `main`, `run2`,
   `ann`, `scale`, the oracle, the OS probes.
2. `Engine::open(dir, &OpenOptions) -> Engine`. `OpenOptions { nguar, csg_plain, design_exact, rlv, profile,
   clusters, tables16, cost: CostSource::{Calibrate{nq, cpu}, File, Default}, approx: Option<ApproxFit>, vectors:
   VectorPlacement::{Memory, File{path, advice, read: ReadPolicy}} }`. It runs build, re-pack calibration, support
   table, cost calibration and the approximate fit, and returns a complete object. `build2` becomes private.
3. `ReadPolicy { uring, qd, direct_min, dontcache_min, willneed }` lives inside `Rows::Map`. Whether the planner
   counts disk cost is computed from the store, not stored in `Plan`.
4. The I/O counters move into the `Reader` (it already has `reads`/`bytes`, `disk.rs:279-280`) and are summed into
   the query's stats.
5. `cg_report`, `cap_self`, `io_read_bytes`, `major_faults`, `cpu_s` and `resident_of` move to the bench.

**Size.** About 150 lines move from `run2.rs` to the library and about 70 from `disk.rs` to the bench. The 13
statics, 6 environment reads and `params_with_profile` go away.

### F3. `Plan` is four structs in one; its defaults are not the measured system (−≈0.3k, and a clearer API)

**Evidence (verified).** `q.rs:1968-2005`, 35 fields:
- **Cost model** (timed, written to `plan.txt`): `full_us, item_us0/1/2, cl_us0/1, grp_us, scan_us, cscan_us,
  anc_us, dir_us, cut_us, seed_us, scan_lo, scan_hi, io_us, io_gbps`.
- **Forced mode**: `mode: PlanMode::{Auto, Index, Flat}`.
- **Query semantics**: `tol`, the caller's tolerance d (SYSTEM.md §1), copied into every `CQ2` (`q.rs:2960`).
- **Experiment switches**: `direct, prev, alpha0, jump_level, jump_d, int_det, gtab, repack, repack_mul, bail,
  rdec, scan_cal, pscan, col, colq, collazy, disk`.

`Plan::default` (`q.rs:2008`) sets `int_det: false, gtab: false, repack: false`. The configuration behind
SYSTEM.md's figures is `--int --gtab --flags repack` (`rust/results/const/check.sh`; 16 scripts pass `--int` and
`--gtab`). `plan2` takes `fixed_us`, and every caller passes `0.0` (`q.rs:2144,2945,3711`). `plan2_prev` is 155
lines "kept for measuring against" (`q.rs:2787`).

**What is wrong.** A caller cannot tell which fields it may set. The library's default answers come from a
configuration nobody measures. A semantic parameter (`tol`) travels in the cost model, so the planner, the kernel
and `CQ2` all carry it. Six switches are copied field by field into `CQ2` (`open_cluster`, `q.rs:2952-2965`), which
gives two places to keep in sync.

**Proposal.**
- `CostModel` (the timed constants with `to_kv`/`from_kv`/`calibrate`), owned by the `Engine`.
- `tol` becomes an argument of the threshold query.
- `PlanMode::Index`/`Flat` stays as a test and benchmark override in a `Debug`/`Force` option, not in the cost model.
- The switches go into a small `Tuning` struct whose `Default` is the measured system. Delete the losing switches
  outright: `prev` and `plan2_prev`; `col`, `colq` and `collazy` (F5); `fixed_us`.
- Ask the owner about `alpha0`, `jump_level`/`jump_d`, `bail`, `rdec`, `pscan` and `split`/`--u8`. Each is off by
  default and has a report that settled it, or should have one. Whatever survives is a `Tuning` field with a
  measured default.
- `scan_cal` belongs in `OpenOptions` (it controls calibration, not queries).

**Size.** `plan2_prev` 155 lines plus `prev`/`fixed_us` plumbing. `plan_flags` (55 lines) shrinks to the remaining
switches. `Plan` goes from 35 fields to about 19 cost fields plus a handful of tuning fields.

### F4. Too many entry points and planning sites for a 2×3 matrix (−≈0.4k, one public API)

**Evidence (verified).** §1.3.
- Planning decisions are made in `plan2` (`q.rs:2572`), `plan2_prev` (`q.rs:2788`), `Plan::scan_first` (`q.rs:2284`;
  `knn2d:3699` uses it, so k-NN mixes the old rule and the new planner), `knn2_fast`'s inline rules
  (`q.rs:3611,3660`), and the scan band (`q.rs:2670-2675`).
- k-NN entries: `knn2`, `knn2d`, `knn2_dial`, `knn2_fast`, `knn2_approx`, `scan_knn`. Threshold entries:
  `threshold2`, `scan_visible`, `run_planned`; the last is public and driven with hand-built `Pre`s from
  `Plan::calibrate` (`q.rs:2127-2130`).
- `Opts2.want_iv` doubles as "want exact scores" (`run_planned` passes it as `want_exact` to `decide_full`,
  `q.rs:3048,3107`). It also silently turns off the integer path and the columnar path (`q.rs:3080-3081`).

**What is wrong.** The matrix the owner has in mind is not visible in the API. Each cell has several
implementations, or none (approximate threshold). The planner's output is a loose tuple that callers construct
themselves.

**Proposal.** Two public functions on a session:

```rust
fn threshold(&self, q: &[f32], t: f64, tol: f64, path: Path) -> Answer;   // Answer carries the bitmap + npow + stats
fn knn(&self, q: &[f32], k: usize, path: Path) -> Vec<(f64, u32)>;
enum Path { Exact /* planner may scan */, Scan, Approx(ApproxParams { nprobe, dial }) }
```

One `plan(&CostModel, &Segment, &Query, t, &Visible) -> Plan` (rename the current `Planned`) serves both kinds;
k-NN calls it at τ₁ instead of `scan_first` + `plan2`. `knn2_fast`'s per-cluster rule should call the same per-cluster
cost function (`index_us` against `full_us`) that `plan` uses, not a copy of the arithmetic. Make `run_planned`
private, and give calibration a `plan_forced(mode)` helper.

Choose one approximate k-NN. NEXT.md and `dbtopk` treat `knn2_fast` as current. Move `knn2_dial` and `knn2_approx`
to the bench, or delete them. `Dial::fit`'s "old" branch (`q.rs:3369-3371,3426`) goes with them. Approximate
threshold should be stated as a decision: either "top-m via `knn` Approx" (what the benchmarks do today), or a
threshold with `dial` (which `Prep2.dial` already supports). In the second case the dial moves out of `Prep2` into
`ApproxParams`.

**Size.** About 155 (`plan2_prev`) + 35 (`knn2_dial`, `knn2_approx`) + 20 (`knn2`/`knn2d` merged) + the `Dial` old
branch, and a much smaller public surface.

### F5. The columnar store and its hooks (−≈1.2k)

**Evidence (verified).** `v2/col.rs` is 1,168 lines. It is off by default, and REPORT3 rejected it (SYSTEM.md §7;
NEXT.md "Delete the column-layout code"). Its hooks in the live code:
- `run_cluster_from`'s whole `resume` parameter (`q.rs:1100-1104`, `1115-1121`, and several `resume.is_none()`
  tests);
- `State.colq`, `State.cslots` (`q.rs:905-906`);
- `GroupPlan2.col` (`mod.rs:404`), `Index2.col` (`mod.rs:904`), `ClusterCode2.ccode`/`cnrm` (`mod.rs:1192-1193`);
- the build branches (`mod.rs:623, 1240-1320, 1665-1733`);
- `BUILD_COL`; `Plan.col/colq/collazy`; `Stats.col_*` (5 fields and their merging, `kernel.rs:37-41, 81-97`);
- `main.rs:788-791`; `run2.rs:218-220`.

About 50 hook lines in total.

**Proposal.** Delete it, as NEXT.md already plans. Do it before any refactor of `run_cluster_from`, because it
removes that function's `resume` mode.

**Size.** About 1,168 + 50 lines, and one mode of the kernel.

**Related: norm sort.** `nusort`/`nukey`/`NuCut`/`nu_range`/`numax`/`bmin`/`bmax`/`Items2.nu`/`sort_by_nu` have 46
references across `q.rs`, `mod.rs`, `run2.rs` and `ann.rs`. The feature is "built and off" (SYSTEM.md §7). It adds a
second cut to `plan2`, `plan2_prev`, `seed2` and `run_planned`'s interval output. If no report shows it winning,
delete it with the columnar store (about 150 lines). Otherwise give it a `Cut` trait with one implementation per
sort order, so the four call sites stop branching on `ix.nukey.is_empty()`.

### F6. Verification, profiling and study dumps live inside the kernel (−≈70 runtime branches, −4 globals)

**Evidence (verified).**
- `run_cluster_from` (`q.rs:1104-1883`, 780 lines) takes `mode: Mode2::{Thr, Interval(l), Sweep(l)}`, `verify`,
  `first_decide` and `resume`. It contains 19 `verify` branches, the f64 table recomputation (`q.rs:1310-1340`), the
  `DUMP` writer under a mutex (`q.rs:1683-1700`), and `ERRS` collection (`q.rs:1075`).
- 53 `tick()` calls in `q.rs`, each a global atomic load (`q.rs:57-59`).
- `Stats` (`kernel.rs:14-106`) has 30 fields: counters the planner and bytes accounting need (`enter`, `touched`,
  `full_reads`), plus verification (`viol_*`, `checked`, `max_sd`, `imp_*`, `cert_viol`), profiling (`cyc[32]`,
  `lcyc`), the columnar diagnostics, and `errs`.
- Every query allocates `Stats::new(nl)` (5 `Vec`s) per parallel part and merges them field by field.

**What is wrong.** Instrumentation decides the kernel's signature and its branch structure. Turning profiling on or
off, or collecting errors, is process-global (and `Dial::fit` relies on that, `q.rs:3393`). Verify mode is
load-bearing beyond tests: `Dial::fit` runs threshold queries with `verify: true` to collect `errs`
(`q.rs:3400-3401`), so instrumentation cannot simply be deleted.

**Proposal.** Pass an observer as a generic parameter:

```rust
trait Observer {
    const VERIFY: bool = false;
    fn phase(&mut self, p: Phase, cyc: u64) {}
    fn interval(&mut self, lvl: usize, row: u32, est: f64, half: f64, truth: f64) {}
    fn decision_check(&mut self, ...) {}
}
```

`NoObs` (a zero-sized type) is the default, and the bench supplies `Verify`, `Profile`, `ErrCollector` and `Dump`.
Monomorphisation removes the branches and the `tick()` loads from the production instantiation. `Dial::fit` passes an
`ErrCollector`, not a global. `Stats` shrinks to the counters queries use. Verification and profiling data live in
the observer.

`Mode2` is two different things. `Interval(l)` (k-NN probe and refinement) is a real output mode and stays. `Sweep`
is a timing harness: move it into the bench, implemented with `Interval` plus a no-op sink, or drop it.

**Size.** About 70 branch sites in the kernel and the level loop, 4 statics, 25 phase constants moved to the
observer module, and about 15 fields out of `Stats` (more once the columnar store is gone).

### F7. Vectors and compaction: `Full2`, `disk::Rows`, `vrow` and a compacted `Index2` (−≈0.2k, and many special cases)

**Evidence (verified).**
- `Full2::Real { map, off, rows: Arc<disk::Rows> }` and `Full2::Synth(Synth2)` (`mod.rs:871-874`).
- `map` and `off` are never read after `build2` copies the rows (`mod.rs:1764-1768`). `compact` only clones them
  along (`mod.rs:1043,1046`). They keep a second mapping of the whole source file alive, which matches SYSTEM.md §5,
  "held ... through more than one mapping".
- `disk::Rows::{Mem(F16Rows), Map{map, file, rfd, dfd}}`. `Index2::vrow` (`mod.rs:905`) remaps rows of a compacted
  index that shares its parent's vectors. Then `own_rows()` returns `None` (`mod.rs:956-961`), which turns off the
  reader, `willneed` and `cached` for that index (NEXT.md notes the consequence).
- `compact` (`mod.rs:981-1079`) builds a complete new `Index2` that copies 14 `Arc`s.
- A compacted index has an empty `ent_row`, so `row_of_ent` asserts (`mod.rs:1095-1098`), `Vis2::new` cannot be
  built on it, `Plan::calibrate` asserts (`q.rs:2083`), and SOAR asserts (`q.rs:3509`). `col` is set to `None`.
- 23 call sites check `compacted()`, `vrow`, `own_rows()` or `full_cached`. 7 sites branch on `Full2::Synth`.
- Callers must remember to query the compacted index with `vis: None` and `to_bitmap2(sub, ...)` (`run2.rs:1367-1376`).

**What is wrong.** Three independent facts are encoded in one `Index2`:
1. what is shared and query-independent (groups, tables, centres, rotations, anchors, support table, centre tables,
   re-pack thresholds, meta);
2. which rows exist and their codes (cluster ranges, keys, codes, scalars, escapes, ids);
3. where each row's vector is and how to read it.

`Cl2` mixes (1) and (2) as well: `r0`/`r1`/`nblk` next to `mhat`, `mv32`, `wmr`, `bm`, `b2` and so on, which is why
`compact` clones every `Cl2`.

**Proposal.**
- `Model` (an `Arc`, immutable): what `compact` shares today, plus the per-cluster query-independent part of `Cl2`.
- `Segment`: per-cluster row ranges, `ahat`, `lcode`/`lscal`/offsets, `esc`, `row_ent`, an optional `ent_row`, and a
  `VecStore`. `Segment::compact(rows, copy_vectors)` replaces `Index2::compact`. A session is `(Arc<Model>,
  Arc<Segment>)`.
- `VecStore` as an enum: `{ Mem(F16Rows), File { map, fds, policy: ReadPolicy }, Remap { base: Arc<VecStore>, rows:
  Vec<u32> } }`. It provides `row(r)`, `cached(rows)`, `reader()` and one `score(q, rows, sink)` (F8). `Remap`
  forwards `reader()` and `cached()` through the remap, which removes the "compacted index can't use the reader"
  limitation instead of special-casing it.
- Drop `map`/`off`.
- Move `Synth` to the bench as a `VecStore`-like source, or drop the synthetic scale test. Either way the `Synth`
  branches leave `exact_value2`, `exact_ge2`, `run_cluster_from` and `answer_share`.

This split is also the natural shape for the write path DESIGN.md §4 plans (segments over one model).

**Size.** About 100 lines net removed. The main gain is that about 30 special-case sites become method calls on
`Segment`/`VecStore`.

### F8. Full-vector scoring is implemented 5–6 times, and the reader dispatch twice (−≈0.2k)

**Evidence (verified).**
- Decide rows against t: `decide_full` (`q.rs:2408`), `decide_full_from` (`q.rs:2427`), `decide_full_packed`
  (`q.rs:2458`), the v1 `Flat::threshold` (`query.rs:916`).
- Top-k from rows: `TopK::score` (`q.rs:3549`), `topk_f16` (`query.rs:975`), `Flat::knn`.
- "Cached? decide from mapping : push to reader, drain when full" appears in `scan_visible` (`q.rs:2340-2366`) and in
  `run_planned` (`q.rs:2980-2988`, `3098-3123`), with `willneed` read-ahead in two more places.

**What is wrong.** Every change to how a vector is read or scored (the AVX2 tail bug in NEXT.md, packed scan, O_DIRECT,
`willneed`) has to be repeated in each copy. The duplication reviewer will list the copies. Architecturally, the
missing piece is one owner, the `VecStore`.

**Proposal.** `VecStore::for_rows(rows, |batch_of_rows_and_vectors|)` hides mapping, reader, packing and read-ahead.
`decide(t, err)` and `topk(k, err)` are the two consumers. The oracle (`Flat`) moves to the bench and reuses them over
a source-order store.

**Size.** About 200 lines.

### F9. `q.rs` is one 3.9k-line file; the level loop is one 780-line function (moves, no removal)

**Evidence (verified).** §1.1 lists twelve concerns in `q.rs`. SIMD kernels sit in the query file (`qv_exact`,
`dot64v`, `matvec64`, `dot32`, each in AVX-512 and AVX2 versions, `q.rs:170-320`, `497-522`), apart from `kern.rs`
and `simd.rs`. `calibrate_repack` (`q.rs:1883`) is a kernel micro-benchmark that runs at open.

**Proposal.** Split by concern (pure moves): `prep.rs` (query, anchor, group setup), `cut.rs`, `visible.rs`,
`levels.rs` (the loop, `State`, `CQ2`), `full.rs`/`scan.rs`, `plan.rs` (cost model, calibrate, plan), `threshold.rs`,
`knn.rs` (seed, select, refine), `approx.rs` (`Approx`, `Dial`, `knn_fast`). The SIMD routines go to `kern/dot.rs`.

After F5 and F6, split `run_cluster_from` along its existing phases:
- `init_candidates` (`q.rs:1113-1186`);
- `level_setup` (tables, integer thresholds);
- `decode_block_det` and `decode_block_rand`;
- `compact_survivors`;
- the re-pack decision.

Hot-loop inlining can be kept with `#[inline(always)]` on the helpers. The other reviewers should confirm with a
benchmark.

### F10. `Option<&Vis2>` with `None` = everything is branched on at 41 sites (−≈0.1k)

**Evidence (verified).** 41 occurrences of `vis.map_or`/`match vis`/`match o.vis` in `q.rs`, for example
`q.rs:2577, 2633-2636, 2642-2645, 2995-3006, 3040-3043, 3610, 3638-3641`. `Vis2::restrict` exists only for
`knn2_dial`.

**Proposal.** A `Visible` type with an `All(nrows, cl)` variant and the operations the code needs:
`count(ci)`, `has(r)`, `rows_in(r0..r1)`, `nrows()`, `live_clusters()`. Construct it once per session. The 41 branches
become method calls.

### F11. Three table strategies and two integer widths for one table (decision needed; −≈0.2k if resolved)

**Evidence (verified).** In `run_cluster_from`, a level's table comes from three places (`q.rs:1251-1272`):
- the group-shared table when `cq.shared` (`alpha0`, `GQ2.glut`);
- `group_tab` + `combine_lut` when `cq.gtab`;
- `build_lut` per cluster otherwise.

Independently, `Index2.split` chooses 16- or 8-bit tables (`--u8`, 20 references). The integer deterministic path
(`enable_int`, `IntDet`, `int_level`) runs alongside the float path. The float path is needed: `Interval` mode for
k-NN uses it, because `want_iv` disables `int` (`q.rs:1180`, `3081`). `probe` and `row_interval2` call
`cluster_q2`, which does not set `gtab` (`q.rs:3182, 3214`). So k-NN refinement uses `build_lut` while threshold
queries in the measured configuration use `combine_lut`.

**Proposal.** Keep the integer and float decision paths (both are needed). Pick one table builder: `gtab` is the
measured one. Route `probe` and `row_interval2` through `open_cluster` so they use it too. Then `alpha0`/`shared`/
`glut`, the per-cluster `w` vector (`cluster_q2w`'s `want_w`) and `int_sigma`'s non-`gtab` branch go away (inferred:
these become unused once `gtab` is the only builder; verify by deleting and compiling). Decide `split` the same way.

### F12. The bench (`run2.rs`, `main.rs`) is hard to extend (lower priority: bench code)

**Evidence (verified).**
- CLI access is spread through every function (`arg_str`/`arg_usize` re-collect `std::env::args()` on every call,
  `run2.rs:39-49`).
- `dbcmp` is 265 lines with string-dispatched variants (`"index" | "idx" | "forced" | "scan" | "cindex" | ...`,
  `run2.rs:1353-1399`) and per-mode global flips.
- `Ctx2` duplicates data the index already has (`xs`, `ent_xs`, `p`).

**Proposal.** Once the library has the API of F4, a variant is `(Session, Path, Tuning)`. Parse the CLI once into
typed structs (a `clap`-style struct, or a hand-written one in one place). Make each mode its own file under
`bench/`. Keep this for last: it only becomes simple once F2–F4 are done.

### Large things tripped over (for the other reviewers)

1. **The cost model may be timed on a different kernel than the one it plans for (inferred).**
   `Plan::calibrate` times the index path with `want_iv: true` (`q.rs:2142`). In `run_planned` that disables
   `int_det` and `col` (`q.rs:3080-3081`). Under `--int` the fitted index-path factor therefore comes from the float
   path, while threshold queries run the integer path. This could explain part of the planner mis-pricing that
   NEXT.md describes. Confirm by checking whether `want_iv` is needed there; nothing reads `iv` in `calibrate`.
2. **`Dial::fit` toggles a process-global `ERRS` around a parallel loop (`q.rs:3393-3403`).** It is unsafe with
   concurrent queries.
3. **Memory.** `Full2::Real.map` keeps the whole source file mapped for the life of the index, and `setup2_at` opens
   the vector file three times (`run2.rs:212-214`). This is a likely contributor to the 6.2 GiB RSS in SYSTEM.md §5.
4. **`row_interval2` (`q.rs:3171-3189`)** builds a fresh `State`, `CQ2` and tables for each refinement of each
   candidate in `select_knn` (performance).
5. **No end-to-end tests.** There are 16 unit tests (`kern` 8, `tail` 5, `kernel`/`mod`/`q` 1 each). Every query-level
   check is a CLI mode against real data (`verify`, `oracle`, `dbcmp` answer equality). NEXT.md notes that nothing
   tests the planner, `compact` or `F16Rows`. Any of the refactors below needs a small synthetic end-to-end test first
   (threshold and k-NN, Exact and Scan, with and without `Visible`, against the brute force). The current safety net
   needs the 1M data set.

---

## 3. Proposed target architecture (one page)

```
liminal/            (library crate)
  params.rs         Params, load_params, RealSource (from v1 index.rs); derive.rs; tail.rs
  code/             det.rs (DetTab, alloc), rand.rs, layout.rs (Field, Part, ExpTab, LevelPlan), group.rs (GroupPlan)
  build.rs          encode + build → (Model, Segment); support table; repack calibration     [private]
  store/
    model.rs        Model  (Arc, immutable: meta, groups, tables, centres, rotations, anchors, hi/lo, ctab, repack)
    segment.rs      Segment (cluster row ranges, keys, codes, scalars, escapes, ids, VecStore); Segment::compact
    vectors.rs      VecStore {Mem, File{ReadPolicy}, Remap}; for_rows / decide / topk
    reader.rs       io_uring Reader (unchanged, policy passed in)
  kern/             simd.rs, tables.rs (one table builder), int.rs, dot.rs (dot_f16*, qv_exact, dot64v, matvec)
  query/
    prep.rs         Query (rounded q, anchor, lazy group setup)
    visible.rs      Visible {All, Set}
    cut.rs          support table + sorted range (+ NuCut only if kept)
    levels.rs       run_cluster<O: Observer>(…, Output::{Decide, Interval(l)})
    plan.rs         CostModel (+calibrate), Tuning, plan() -> Plan  — the single planner
    threshold.rs    exact threshold over a Plan
    knn.rs          seed, select_knn, refine
    approx.rs       ApproxModel {order f, radii, dial fits}, knn_fast
    observe.rs      Observer trait, NoObs, Phase
  engine.rs         Engine::open(dir, OpenOptions); Engine::session(&RoaringBitmap, Compaction) -> Session
                    Session::threshold(q, t, tol, Path) / Session::knn(q, k, Path)
vexact/             (binary crate): main.rs (CLI → typed options), bench/{dbcmp, dbtopk, dbknn, e2e, verify,
                    breakdown, levels, filter, scanbench, ann, scale}.rs, oracle.rs (Flat, thresholds_for),
                    os.rs (cgroup, /proc probes), observers (Verify, Profile, Dump, ErrCollector), synth.rs
```

**Core types.** `Model`, `Segment`, `VecStore`, `Visible`, `Query`, `CostModel`, `Tuning`, `ApproxModel`, the
`Observer` trait, and `Path`. A session is `(Arc<Model>, Arc<Segment>, Visible)`. Compaction just produces a
different `Segment` with `Visible::All`.

**The 2×3 matrix on these types.**

| | `Path::Scan` | `Path::Exact` | `Path::Approx(p)` |
|---|---|---|---|
| threshold(t, tol) | `VecStore::decide` over `Visible` | `plan()` → per cluster {skip, in, direct, full, levels} → `VecStore::decide` (the planner may pick Scan) | the owner's decision: top-m `knn_fast`, or `levels` with `p.dial` |
| knn(k) | `VecStore::topk` over `Visible` | seed → `plan()` at τ₁ → levels in `Interval` → `select_knn` | `knn_fast(ApproxModel order, p.nprobe, p.dial)` |

One planner, one cost model, one levels kernel with two output modes, one vector-scoring owner.

**Order of steps.** Each step builds, and `dbcmp` answers stay bit-identical.

1. **Safety net.** Add a synthetic end-to-end test (about 2k rows, n = 64): threshold and k-NN × Scan and Exact × with
   and without a visible set and compaction, against brute force. Also add a 50k smoke run that compares `dbcmp`
   answer dumps before and after each step.
2. **Delete dead weight.** Delete the columnar store and its hooks (F5). Delete `plan2_prev`, `prev` and `fixed_us`
   (F3). Delete `knn2_approx` and `knn2_dial` from the library, or move them to the bench (F4). Pure deletions,
   checked by the compiler.
3. **Retire v1** (F1). Move the 10 live items, delete the rest, and run `cargo test` plus the smoke run.
4. **Split lib and bin** (F2). Move `run2`, `ann`, `scale`, `main`, `Flat` and the OS probes to the binary. The
   compiler then lists every library item the bench uses, which becomes the API to stabilise.
5. **Replace globals with options** (F2). `OpenOptions` replaces `NGUAR`, `CSG_PLAIN`, `DESIGN_C` and `BUILD_COL`
   (already gone). `ReadPolicy` on `Rows::Map` replaces the disk atomics and `plan.disk`. `Engine::open` takes over
   `setup2_at`'s library half and returns a complete object.
6. **Split `Plan`** (F3). `CostModel` + `Tuning` (defaults = int, gtab, repack) + `tol` as a query argument. Decide
   the remaining switches with the owner.
7. **Observer** (F6). Make the kernel generic and move `PROF`/`ERRS`/`DUMP`/verify into bench observers. Check that
   the `NoObs` instantiation is at least as fast (breakdown run).
8. **Split `q.rs` into modules** (F9). Pure moves. Then cut `run_cluster_from` into phase functions.
9. **`Visible`** (F10). Then **`Model`/`Segment`/`VecStore`** (F7) and the single scoring path (F8). `compact` becomes
   `Segment::compact`, `vrow` becomes `VecStore::Remap`, and `Full2` disappears.
10. **Public API** (F4). `Session::threshold`/`knn` with `Path`, and one planner for both kinds. Resolve approximate
    threshold. Then the bench (F12).

**Rough net effect.** About 16.4k lines today. Removed: about 3.5k (v1) + 1.2k (columnar store) + about 0.6k
(alternatives, `plan2_prev`, k-NN variants, norm sort if dropped, `Synth` moved out). That leaves about 11k, of which
about 4k is bench and about 7k library. The library would have no globals, no environment reads, one entry point per
matrix cell, and a kernel signature that carries only what the query needs.


---

# Appendix: duplication review


Reviewer lens: code that does the same job twice, code nothing (or only an experiment) reaches, and options that were
measured, rejected and kept. Checkout `/home/user/evs` at `ce9cf94` (branch `evs-merge`), read only; nothing was built
or run. Line numbers are of that checkout.

Legend: **[V]** verified by reading or grep (call sites listed); **[I]** inferred (not checked by building or running).
Where a release-build warning is cited, it is the owner's list; I confirmed each by grep, not by a build.

---

## 0. Headline

1. **A whole second index ("version 1") is still in the crate: about 3,600 of the ~16,400 lines.** `index.rs`,
   `query.rs`, most of `kernel.rs`, `synth.rs`, `scale::run` and the v1 half of `main.rs` implement the pre-fast-scan
   index. SYSTEM.md describes only v2. `main.rs:798` refuses to run v1 without AVX-512, and SYSTEM.md says the
   measuring machine (Ryzen 5900X) has no AVX-512, so **v1 cannot even run on the machine every figure comes from**.
   v2 borrows a handful of items from those files (listed in 2.1); everything else is unreachable from `--v2`.
2. **The threshold hot path, `run_cluster_from` (q.rs:1104–1876, 770 lines), carries six experiment switches** that
   reports rejected or that only a benchmark sets (single pass/`jump`, shared tables/`alpha0`, `bail`, `rdec`,
   `Mode2::Sweep`, the columnar `resume`), plus a study dump. Each is a branch inside the per-block loop.
3. **Two threshold planners** (`plan2`, `plan2_prev`, 155 lines, the latter "kept for measuring against") and **five
   k-NN entry points** of which two approximate modes (`knn2_dial`, `knn2_approx`) are superseded by `knn2_fast`
   (bench/topk/REPORT.md) and reachable only from benchmark modes.
4. **The measured configuration is not the default.** Every timing script passes `--int --gtab --flags repack`
   (109/100/11+ occurrences under rust/results and bench); `Plan::default()` has all three off. The defaults therefore
   select the slower alternatives, which keeps them alive.
5. Smaller but pervasive: the AVX-512 twins of every kernel (~300 lines, never compiled on the measuring machine), four
   copies of the support-table bound, three full-vector decide loops, two support-table builders in v2, and
   `dbtopk`/`dbknn` in `run2.rs` that repeat each other's ladder and dial machinery.

Removing (1)–(3) and the rejected flags takes the crate from ~16.4k to roughly 10–10.5k lines without touching any
answer of the default or measured configuration.

---

## 1. Inventory of near-duplicates

Each entry: locations, what differs, whether the difference is meaningful, what one version would look like.

### 1.1 Threshold planners: `plan2` vs `plan2_prev` [V]

- `v2/q.rs:2572–2785` `plan2` (213 lines) and `v2/q.rs:2788–2942` `plan2_prev` (155 lines).
- Selection: `threshold2` (q.rs:2945) picks `plan2_prev` iff `Plan::prev`, which only `plan_flags("prev")`
  (run2.rs:129) sets. `knn2d` always uses `plan2` (q.rs:3711), so `prev` affects threshold queries only.
- Shared, copied nearly verbatim: the norm-sort branch (q.rs:2623–2638 vs 2818–2834), the support-table test
  (2609–2618 vs 2835–2843), cut + sorted range + `ncand` (2639–2646 vs 2844–2851), `in_band` (2670–2675 vs
  2860–2865), `first_decide` (2750 vs 2907), and the whole per-cluster index/full choice with group-gain pruning
  (2753–2783 vs 2910–2940).
- Differs: `plan2` has per-cluster direct scoring (`Pre::Direct`), the anchor/cut floors before reading anchors, the
  answer-share term `item_us2`, and the scattered-read price for candidates under a visible set; `plan2_prev` uses
  `Plan::scan_first` and a sampled extrapolation.
- Meaningful? No for the product: REPORT_PLANNER.md (bench/threshold-db/results/planner, line 37) says it is kept so
  the two "could be timed interleaved"; the comparison is done (`tables_final.md` has the `index:prev` rows).
- One version: delete `plan2_prev` and `Plan::prev`. Then `threshold2` is `match plan2(..)`.
- Note: `Plan::scan_first` (q.rs:2284) is still used by `knn2d` (q.rs:3699) as its pre-seed scan check. That is the
  *old* planner's scan rule living on inside k-NN; see 1.2.

### 1.2 k-NN entry points [V]

| Function | Lines | Kind | Callers |
|---|---|---|---|
| `knn2` | q.rs:3496–3498 | wrapper: `knn2d(.., None)` | ann.rs:170, run2.rs:470, 520 (verify2), 777 (e2e2), 1056 (filter2), 1119 (oracle_dump), 1563 (dbtopk), scale.rs:450 |
| `knn2d` | q.rs:3687–3739 | exact: scan precheck (`scan_first`), seed, `plan2`, `run_planned` with intervals, `select_knn`; optional dial | `knn2`, `knn2_dial`, `Dial::fit` (non-fast branch) |
| `knn2_dial` | q.rs:3504–3512 | approximate "old": restrict the visible set to `nprobe` clusters (+SOAR), then `knn2d` with a dial | ann.rs:211, run2 dbtopk/dbknn `--approx-mode old` |
| `knn2_fast` | q.rs:3572–3684 | approximate "new" (best-first heap), the current approximate mode | run2 dbtopk/dbknn default, `Dial::fit(fast)` |
| `knn2_approx` | q.rs:3441–3457 | IVF + re-rank at a fixed level | ann.rs:187 only (`--approx` in bench/run.sh) |

- Also a sixth exact path: the `flat` closure in ann.rs:172–180 rebuilds `knn2d`'s `scan` closure (q.rs:3691–3697)
  by hand (query_f16 + scan_knn + Stats + KnnOut2).
- Meaningful? `knn2d` (exact) and `knn2_fast` (approximate) are the two the docs describe. bench/topk/REPORT.md:21–25
  and threshold-db/REPORT.md:103–114 name `knn2_fast` as the approximate mode and `knn2_dial` as the "old mode";
  `knn2_approx` appears in no current report.
- Duplication inside the survivors: `knn2_fast` re-implements the cut (q.rs:3628), the visible-row slicing
  (`vis_rows`, 3603–3607), cluster opening (3664) and full-vector scoring (`TopK::score`, 3549–3561, the 4-wide loop
  again) instead of sharing them with `run_planned`.
- One version: `knn(ix, q, k, vis, plan, approx: Option<ApproxCfg{ap, nprobe, dial}>)`, with `knn2d` as the exact
  branch and `knn2_fast` as the approximate one; delete `knn2`, `knn2_dial`, `knn2_approx`, the non-fast branch of
  `Dial::fit` (q.rs:3391, 3407, 3426: the "1,000th best score, factor 1 on deterministic levels" fit exists only for
  `knn2_dial`), `Approx::second_assignment` (SOAR, q.rs:3258–3299, used only via `knn2_dial(.., soar=true)`) and
  `Approx::second`. Replace `knn2d`'s `scan_first` precheck with the same scan rule `plan2` uses (or call `plan2` with
  `fixed_us = seed_us` before seeding).

### 1.3 Full-vector decisions: `decide_full` / `decide_full_from` / `decide_full_packed` and four more [V]

The same "f32 dot of four rows, decide with `err`, exact integer product inside the band" loop appears in:

| Location | What differs |
|---|---|
| `decide_full_from` q.rs:2427–2454 | the core; rows given by a closure (mapping or io_uring batch) |
| `decide_full` q.rs:2408–2424 | `willneed`, then `decide_full_from`; a scalar fallback through `exact_ge2` for `Full2::Synth` |
| `decide_full_packed` q.rs:2458–2509 | copies 32 rows into a buffer (with prefetch) and repeats the decide loop verbatim (2483–2507); `exact_value2` instead of `exact_to_f64(exact_dot)` |
| `exact_ge2` q.rs:855–872 | one row, `dot_f16`, same decide |
| `Flat::threshold` query.rs:917–956 | the truth oracle, over the numpy-order mapping and entity ids; same decide (closure at 918) |
| `answer_share` q.rs:2546–2564 | f32 only, one row at a time |
| `TopK::score` q.rs:3549–3561, `topk_f16` query.rs:975–1026, `Flat::scores` query.rs:899–915 | the 4-wide scoring loop with a scalar tail |

- Meaningful differences: the row source (mapping, batch, packed buffer, numpy file) and the sink (rows, entities,
  heap). The arithmetic and the error bound are the same in all.
- `decide_full_packed` is reachable only with `Plan::pscan` (off by default; SYSTEM.md 7 "built and off by default";
  set by `pscan` flag and `scanbench`). It also returns `exact_value2` where the others recompute `exact_dot`.
- One version: a single `fn score4<'a>(q32, n_rows, get: impl Fn(usize)->&'a [u16], f: impl FnMut(usize, f32))` and
  a single `fn decide(s, t, e, q16, row) -> bool`. `decide_full_from`, `Flat::threshold`, `TopK::score`,
  `topk_f16` and `Flat::scores` become 5–10-line callers. If `pscan` is retired (see 3), `decide_full_packed` goes.

### 1.4 Scans [V]

- `scan_visible` (q.rs:2331–2399) has two whole branches: io_uring reader (2340–2366) and in-memory (2368–2389),
  each building its own `ThrOut2`, plus the `packed` switch. Meaningful (disk mode is a feature), but the merge and
  per-chunk accumulator are written twice; a `Reader` that degrades to the mapping would make one loop.
- `scan_knn` (q.rs:2402) delegates to `query::topk_f16` — a v1-file function kept alive by v2. `Flat::knn` (query.rs:958)
  is the same call over the numpy file (truth). Fine to keep both callers; move `topk_f16` into v2.
- Three hand-rolled "visible rows of cluster ci" slices: `run_planned` Pre::Direct (q.rs:3042), `Vis2::restrict`
  (784–786), `knn2_fast::vis_rows` (3603–3607). `cands_of` (2527) does it by testing every row with `has()`. One
  `Vis2::rows_in(r0, r1) -> &[u32]` serves all four.
- "Visible count of cluster ci" closure `vis.map_or(r1 - r0, |v| v.cl_cnt[ci])` is re-declared in `plan2` (2577),
  `plan2_prev` (2853, 2899), `knn2_fast` (3610, 3656), `Plan::calibrate` (2154), `Approx::order*`, `seed2` (3464).

### 1.5 Cut, range and support-table bounds [V]

- The support-table bound "(hi, lo) of anchor a for cluster ci, (bh, bl) by the sign of beta" is written four times:
  `plan2::support` (q.rs:2609–2611), `plan2_prev::pre_of` (2835–2837), `run_planned` Pre::In intervals (3027–3029),
  `seed2` (3466–3470, upper side only), and a fifth time inside `cut_params2` (702–704). One
  `fn support(ix, pr, ci) -> (lo, hi)` (already including `± rho·rr`) replaces them.
- `sup_set` / `inf_set` / `isect`: q.rs:576–627 is a copy of query.rs:211–264 (diff shows only renamed locals and
  one comment). `Cut` itself is still imported from `query.rs` (q.rs:7). Goes away with v1 (2.1).
- `roundup_abs`: index.rs:667 and v2/mod.rs:1213, identical bodies. Goes away with v1.
- Two cut families coexist: the key cut (`cut_params2` / `sorted_range2` / `key_interval2`) and the norm-sort cut
  (`NuCut` / `nu_range`, q.rs:633–694), selected at every call site by `ix.nukey.is_empty()` — 7 sites (2593, 2623,
  2799, 2818, 3016/3056, 3465, 3628). The norm sort is built and off (SYSTEM.md 7; no native measurement of it found in
  rust/REPORT*.md or bench/). If kept, it should be one `trait`-like `enum ClusterCut { Key(Cut), Nu(NuCut) }` with
  `range(t)` and `interval(row)`, not seven `if`s.

### 1.6 Query and group setup wrappers [V]

Thin wrapper chains, each with one or two real callers:

| Chain | Locations | Real variation |
|---|---|---|
| `prep2` → `prep2_qm` | q.rs:141, 147 | `qm` given only by `knn2_fast` (3580) |
| `Prep2::anc` → `anc_among`, plus the anchor pick inside `prep2_qm` (155–164) | q.rs:99–126 | three ways to choose the anchor |
| `group_q2` → `group_q2r(rot)` | q.rs:323, 328 | `rot=false` only from `setup_groups` |
| `cluster_q2` → `cluster_q2a(alpha0)` → `cluster_q2w(alpha0, want_w)` | q.rs:524–572 | `alpha0`, `want_w` only via `open_cluster` (2954) |
| `run_cluster` → `run_cluster_from(resume)` | q.rs:1096, 1104 | `resume` only from col.rs:1163 |
| `full_err2` → `full_err_qn` | q.rs:840, 845 | none |
| `GroupPlan2::new_light` → `new_inner` | mod.rs:1821, 512 | none (`new_light` is the only caller) |
| `Approx::order` / `order_qm` | q.rs:3301, 3308 | `order` computes q·m; `order_qm` takes it (identical bodies otherwise) |
| "stored row → unit query" | `Plan::calibrate` 2102–2104, `scan_band` 2253–2255, `pseudo_queries` 3239–3244 | none |

- `row_interval2` (3182) and `probe` (3214) open clusters with `cluster_q2` (no repack, no gtab, no tolerance), while
  threshold queries use `open_cluster` (2952). Two cluster-opening paths with different table builders: probably
  unintended. One version: everything through `open_cluster`.
- Two rotation paths: per-group `matvec64` inside `group_q2r(rot=true)` (q.rs:375; reached through `get_gq2` from
  `row_interval2`, `probe`, `knn2_fast`), and the batched `dgemm` in `setup_groups` (413; reached from the planners).
  Same mathematics, different summation order, so the f32 weights can differ in the last bit between k-NN refinement
  and threshold queries [I: sound either way since rounding is bounded, but it is two implementations of one step].

### 1.7 Dot-product and kernel twins [V]

| Kernel | AVX-512 | AVX2 | Other copies |
|---|---|---|---|
| f16×f32 dot | kernel.rs:569 `dot_f16` | kernel.rs:648 | — |
| f16×f32 dot ×4 | kernel.rs:597 | kernel.rs:677 | (the tail bug of NEXT.md was in both) |
| int8×int16 dot | kernel.rs:623 `dot_i8` | kernel.rs:706 | used only by v1 `main.rs` |
| f64 dot | q.rs:201 `dot64v` | q.rs:262 | `util::dot64` (util.rs:258, scalar 8-way, 17 v2 call sites); `second_assignment`'s local `dot` (q.rs:3262) |
| f64 mat-vec | q.rs:221 `matvec64` | q.rs:283 | `dgemm` in `setup_groups` |
| f32 dot → f64 | q.rs:497 `dot32` | q.rs:293 | `util::dot32_64` (v1 only) |
| exact basis product | q.rs:170 `qv_exact` (VNNI) | q.rs:249 → `setup::qv_exact_r::<4,1>` | `setup::qv_exact_r` is generic over R and PFD only so `qvbench`/`setup_cost1` can time variants |
| gather/plan/part sums | kern.rs:105–246 | kern.rs:251–353 | — |
| combine tables | kern.rs:808 `combine_lut` | — | `combine_lut_unitwise` kern.rs:906, `#[cfg(test)]` reference: fine |
| int8 basis × f32 | kernel.rs:727 `basis_matvec` | — | v1 only |

- `target-cpu=native` (`.cargo/config.toml`) means the measuring machine compiles only the AVX2 column; **the
  AVX-512 column (~300 lines in q.rs, kern.rs, kernel.rs) is never compiled or tested where results are produced**
  [V for the cfgs; I that nobody builds on an AVX-512 host — the old Plan::default comment, "constants for 1M BioCLIP
  (n = 768, AVX-512)", q.rs:1965, suggests they once did].
- `util::dot64` vs `dot64v`: both compute q·m / anchor products in the query; `dot64v` for anchors (118, 163),
  `dot64` for q·m (13 sites). Different summation order, same role. One f64 dot suffices.
- One version: a single `kernels` module with one implementation per operation (AVX2+FMA+F16C, which `simd.rs`
  already lifts to 16 lanes), and AVX-512 only behind a measured win. `qv_exact_r` fixed at the chosen `<4,1>`.

### 1.8 Support-table builders [V]

- `v2::build_table2` (mod.rs:1827–1872), the inline copy in `scale::run2` (scale.rs:296–335, synthetic items via
  `p.x = p.m + (V_qᵀp)·z`), v1 `index::build_table` (index.rs:1228) and `scale::synth_table` (scale.rs:18). Same
  dgemm-max-min-round-outward loop and the same transpose into `hi/lo[a*k + c]`.
- One version: `build_table2(ix, nanch, project: impl Fn(cluster) -> (px, pm))`.
- `build_table2`'s `nanch` is always `p.meta.k` from `setup2_at` (run2.rs:226); only the synthetic path uses fewer.
  The `ix.nanch == ix.k()` tests in `anc_among` and `prep2_qm` exist for that case.

### 1.9 Benchmark modes in `run2.rs` that repeat setup [V]

- `dbtopk` (run2.rs:1490–1666) and `dbknn` (1683–1843): both read `--order`, take `c.texts` every `--step`-th query,
  parse `--approx-mode new|old` identically (1500–1504 / 1694–1698), parse `--dial-eps`, build the held-out set and the
  `--dial-from` choice identically (1570–1571 / 1701–1702), fit `Dial::fit(.., Some(&c.approx))` and log it the same
  way, build the same doubling `ladder` (1594–1595 / 1775–1776), and dispatch `knn2_fast` vs `knn2_dial` the same way
  (1581–1585 / 1742–1746). Each opens CSVs with "append, header if new" by its own closure (1515–1523, 1761–1766;
  ann.rs:158–162 is a third). They differ in what k means (fixed ks vs answer-share sizes), the stop rule of the ladder
  and the output columns. One `approx_ladder(c, vis, k, dials, stop_rule)` + `fit_dials(c, k)` + `append_csv` would
  carry both.
- `--vis` file loading: dbcmp (1245–1250), dbtopk (1527–1530), dbknn (1708–1713), three variants of "npy of sorted
  int32 entity ids → RoaringBitmap → Vis2".
- Thresholds per query from the flat scores (`prep2` → `fl.scores` → `thresholds_for(.., full_err2, sizes)`):
  9 sites (run2.rs:375, 448, 567, 765, 885, 1011, 1105, 1154, plus main.rs v1). Truth via `fl.threshold`: verify2,
  breakdown2, filter2. A `ctx.thresholds(q, sizes)` and `ctx.truth(q, t, vis)` would shorten every mode.
- `scanbench` (1139–1197: scan vs packed scan vs planned, under random visible sets) is a subset of `dbcmp`'s variants
  (`scan`, `index:pscan`, `index`). `filter2` (993–1082) overlaps `dbcmp` + `e2e2` (planner/index/flat modes under
  generated filters; its k-NN cell). Meaningful difference: `filter2` generates filters (`make_filter`) instead of
  reading them, and checks answers. Candidate to fold into `dbcmp` with a `--make-vis random:0.01` option [I].
- `--threads` parsed four times (753, 879, 1144, 1224). `pool`, the `out!` macro, `LOG`, `arg_str` exist once in
  `main.rs` (v1) and again in `run2.rs`. `main::subset` (main.rs:40) is repeated inline in `setup2_at` (204–210).
- `setup2_at` opens the vector file three times (`xs`, `src`, `f2`; run2.rs:212–214) — three mappings, which SYSTEM.md
  5 lists among the reasons for 6.2 GiB resident.

### 1.10 Run-cluster internal repetition [V]

- Compaction of the verify-only `truth` array is written twice (q.rs:1796–1804 and 1834–1842) next to `compact_det`,
  which already compacts the verify-only `a64` — move `truth` into `compact_det`'s verify branch.
- The aligned and compacted walks share the decode/bounds body but have separate key loading, compaction and
  re-pack tails (1764–1858). Meaningful (two layouts), but the "compact one 16-lane group" body appears three times
  (1789–1805, 1824–1843, and inside `compact_*`).

---

## 2. Dead or near-dead code

### 2.1 Version 1 (the whole first index) [V reachability; I for exact line counts]

Reachable only when `--v2` is absent (main.rs:793 onward), which exits on any CPU without AVX-512 (main.rs:794–797);
the measuring machine is AVX2-only (SYSTEM.md, intro; README.md:9–12 "Version 1 still needs AVX-512 at run time").

v1-only (≈3,600 lines):

| File | v1-only part | ≈ lines |
|---|---|---|
| `index.rs` (1,306) | `Tree`, `LvOps`, `MOp`, `Op`, `GroupPlan`, `Sparse`, `Cl`, `Full`, `Index`, `Items`, `Calib`, `ClusterCode`, `encode_cluster`, `cluster_info`, `Source`, `build`, `build_table`, `ry/rho_y/rho_t`, `apply_profile` | 1,120 |
| `query.rs` (1,043) | `GQ`, `CQ`, `Prep`, `prep`, `group_q`, `cluster_q`, `cut_params`, `sorted_range`, `key_interval`, `Vis`, `thr_cluster`, `threshold*`, `threshold_batch`, `knn`, `KnnOut`, the copied `sup_set/inf_set/isect` | 860 |
| `kernel.rs` (794) | `Mode`, `BlockOut`, `ops_part`, `refine`, `lookup_idx`, `tab_at`, `nibbles`, `expand`, `run_block`, `dot_i8`×2, `basis_matvec` | 520 |
| `synth.rs` | all | 151 |
| `scale.rs` | `run`, `synth_table` (lines 10–220) | 210 |
| `main.rs` (866) | `Ctx`, `setup`, `verify`, `levels`, `bandwidth`, `bytes_read`, `bytesq`, `e2e`, `batch`, `filtered`, `disk`, the v1 dispatch, `checkderive` | 720 |
| `derive.rs` | `derive`, `det_alloc`, `tof_tree` (only via `apply_profile`) | 70 |

What v2 takes from those files and must keep (move into v2 or a small `common` module) [V by grep of
`crate::index::`, `crate::query::`, `crate::kernel::` in v2/, run2.rs, ann.rs]: `Params`, `Meta`, `load_params`,
`item_scale`, `DA`, `SEED`, `RealSource` (`open`, `row`, `map`, `off`), `query::Cut`, `query::next_up_if_below`,
`query::Flat`, `query::topk_f16`, `query::thresholds_for`, `kernel::Stats`, `kernel::dot_f16`, `kernel::dot_f16_x4`.
Also `derive::{load_profile, design, design_by, bits_of}`.

Release-build warnings that belong to v1 and vanish with it: fields `mvt` (index.rs:559, `Cl`), `qvt` (query.rs:17,
`GQ`), `rmax` (index.rs:237, v1 `GroupPlan`), `v32` (index.rs:580, `Index`) [V: written, never read].

### 2.2 Dead items in v2 [V]

| Item | Location | Status |
|---|---|---|
| `calib_quantile` | mod.rs:1510 | no caller |
| `split_t` | kern.rs:491 | no caller (the test at kern.rs:988 has its own closure) |
| `Meta2::csg` | mod.rs:800 | written (1526), never read; the per-level constants live in `GroupPlan2::csgl` |
| `Cl2::smax` | mod.rs:834 | written (1494, 1507), never read (v1's `Cl::smax` is the one read, query.rs:192) |
| `GroupPlan2::states` | mod.rs:373 | stored (628), never read; `col_plan` takes the local `states` (623) |
| `build2(.., nanch, ..)` | mod.rs:1516 | parameter unused (the index is built with `nanch: 0`, 1797; `build_table2` sets it) |
| `ThrOut2::npow`, `KnnOut2::npow` | q.rs:2308, 3167 | written everywhere, never read. **Keep**: SYSTEM.md 1 documents it as the contract a cache must check ("every result carries the power"). Either read it in one test or document it as intentionally write-only. |
| `Stats::clusters_novis` | kernel.rs:21 | only v1 writes it (query.rs:476) |
| `Stats::col_*` (5 fields + 20 lines of `add`) | kernel.rs:36–40, 83–100 | col.rs only |
| `Plan::default` comment "AVX-512" | q.rs:1965 | stale |

### 2.3 Plan flags and modes nothing sets, or only a benchmark sets [V for setters; reports cited]

| Flag / mode | Set by | Reached in | Evidence |
|---|---|---|---|
| `Plan::prev` | `plan_flags("prev")` | `threshold2` → `plan2_prev` | "kept for measuring against" (q.rs:1982, 2787) |
| `Plan::jump_level`, `jump_d`, `Planned::Run.first_decide`, `run_cluster(first_decide)` | `--jump`, `breakdown2 --variants` | q.rs:1265, 1862, 2750, 2907 | REPORT2.md:68–73 "No single-pass variant beats the level walk". **Stale:** `breakdown2`'s variants pass jump 12 and 14 (run2.rs:872), but nl = 10 now, so `l == first_decide` never holds and those variants silently run the normal walk. |
| `Plan::alpha0`, `CQ2::shared`, `GQ2::glut` | `--alpha0`, breakdown2 variants | q.rs:1239–1244, 2953–2954 | REPORT2.md:67, 72 "no net gain" (40–46% more bytes) |
| `Plan::bail`, `CQ2::bail` | `bail=` flag | q.rs:1217 | REPORT_SETUP.md:55–58 and REPORT_RAND.md:112: off by default; REPORT_BOUNDS.md:224–231: the one unexplained crash ran with bail |
| `Plan::rdec`, `CQ2::rdec`, `kap_unused` logic | `rdec=` flag | q.rs:1267–1283, 2958 | REPORT_RAND.md:34–37, 112: off by default, slower than a coarser schedule |
| `Plan::pscan`, `decide_full_packed` | `pscan` flag, `scanbench` | q.rs:2383, 2458 | SYSTEM.md 7 "built and off by default" |
| `Plan::col`, `colq`, `collazy`, `BUILD_COL`, `run_cluster_from(resume)`, `State::{colq,cslots}`, `Stats::col_*`, `VEXACT_COL*` env | `col*` flags, `--colstore` | col.rs (1,168) + hooks | REPORT3.md: rejected; NEXT.md plans deletion |
| `nusort` build option, `NuCut`, `nu_range`, `nukey`, `numax`, `Items2::sort_by_nu` | `--nusort`, flag `nusort` | 7 branch sites in q.rs, mod.rs 1646–1657, 1700–1736 | SYSTEM.md 7 "built and off"; no native measurement found |
| `--u8` (8-bit tables, `Index2::split = false`) | `--u8` | kern.rs:35–95, q.rs:1461 | REPORT2.md:11 "8-bit tables cost too many bytes" |
| `Mode2::Sweep` | `levels2 --sweep` only (run2.rs:572) | q.rs:1198–1202, 1439–1444, 1736–1741 | benchmark of the decision kernel |
| `DUMP`, `DUMP_TAG` (`VEXACT_DUMP`) | verify2 only (run2.rs:442) | q.rs:1682–1715 | "Study only" (q.rs:53) |
| `scan_cal` / `scan_band` | `scanband=auto` | q.rs:2191, 2246–2280 | scan band is off by default (`scan_lo = 2.0`); REPORT_RAND.md 112 |
| `Plan::direct = false` | `nodirect` | plan2 | REPORT_PLANNER.md:182: direct is on by default; `nodirect` is for comparison |
| `PlanMode::Index` / `Flat` | `--plan`, calibrate, filter2, Dial::fit | everywhere | meaningful: forced modes are used by `calibrate` and `Dial::fit`; keep |
| `CSG_PLAIN`, `NGUAR`, `DESIGN_C`, `--rlv` | CLI only | mod.rs:23–29, run2.rs:174–203 | study switches for the constants work (REPORT_CONST.md); keep only if still wanted |
| `Full2::Synth`, `Synth2`, `SynthSrc`, `scale::run2` | `--v2 --scale` | match arms in `exact_value2`, `exact_ge2`, `decide_full`, `answer_share`, `run_cluster_from` verify, `knn2d`/`knn2_fast` `stored` checks | the synthetic 10M run "stopped during its build" (SYSTEM.md 7); NEXT.md plans *real* 20M/100M data. Experiment-only arms inside the core. |

### 2.4 Alternatives kept only "for measuring against" [V]

- `plan2_prev` (above).
- `knn2_dial` + SOAR + non-fast `Dial::fit`, `knn2_approx` (above; benches `bench/run.sh --approx`,
  `bench/remeasure.sh --soar --dial` still call them).
- `setup::qv_exact_r<R, PFD>` + `qvbench` + `setup_cost1`'s variant loop: the shipped kernel is `<4,1>`.
- `combine_lut_unitwise` is `#[cfg(test)]` — a legitimate test oracle, not accretion.
- `Flat` (query.rs) is the truth oracle over the numpy file; legitimate, but its decide loop is a third copy (1.3)
  and it shares `dot_f16`/`exact_dot` with the code it checks, so it is not an independent kernel check [I].

---

## 3. Ranked removals and consolidations

Ranked by lines removed per unit of risk. "Answers" means the threshold sets and k-NN results; "default" means the
configuration `Plan::default()` + `setup2_at` defaults; "measured" means `--int --gtab --flags repack` as in
rust/results/const/run.sh.

| # | Change | ≈ lines | Answers can change? | Timings can change? | How to check |
|---|---|---|---|---|---|
| 1 | **Delete version 1**: move the 15 shared items of 2.1 into v2/common, delete the rest of `index.rs`, `query.rs`, `kernel.rs` v1 kernels, `synth.rs`, `scale::run`, v1 `main.rs`, v1 `derive`, README v1 sections | −3,600 | No (v2 never calls the deleted code) | No | `cargo build --release` with zero warnings from these files; `cargo test --release` (15 passed, 1 ignored); rust/results/const/check.sh at 50k. Owner to confirm `checkderive` (numpy-vs-Rust profile derivation) is no longer needed. |
| 2 | **Delete the columnar store** (col.rs and its hooks: `resume` and its 10 branches in `run_cluster_from`, `State::colq/cslots`, `Plan::col/colq/collazy`, `BUILD_COL`, mod.rs col encode/build paths, `Stats::col_*`, run2 col stats, main.rs check) | −1,168 −~180 | No (off by default, never chosen by the planner) | No for the default; the measured path loses only dead branches | Same as 1, plus one `dbcmp --cmp-what flat,index,scan,cindex,cvindex` smoke run whose answers must match. Already planned in NEXT.md. |
| 3 | **Delete `plan2_prev`, `Plan::prev`**; replace `knn2d`'s `scan_first` precheck with `plan2`'s rule (then delete `scan_first`) | −160 (−170) | No (the planner chooses paths, not answers; SYSTEM.md 2.2 "which path decides a row never changes the answer") | `prev` runs disappear; filtered k-NN with tiny visible sets may scan at a slightly different point | filter2's `planner_knn10` and dbcmp under 1% visible: k-NN timings before/after; answers equal. |
| 4 | **Remove rejected switches from the hot loop**: `jump_level/jump_d/first_decide` (+ fix or drop breakdown2 `--variants`), `alpha0` + `glut` + `CQ2::shared`, `bail`, `rdec` + `kap_unused`, `--u8` 8-bit tables, `DUMP` | −250 to −300 | No (all off by default and in the measured config) | Possibly slightly faster (fewer branches/args in `run_cluster_from`); `rdec` and `bail` removal deletes per-level checks | e2e at the const config, interleaved, before/after; verify2 at 50k (0 violations); oracle_dump against bench/oracle_thr.py. Confirm with the owner that REPORT_BOUNDS's bail/fifth-level crash investigation is closed. |
| 5 | **Retire superseded approximate modes**: `knn2_dial`, `knn2_approx`, `second_assignment`/`Approx::second`, non-fast `Dial::fit` branch, `Approx::order` (keep `order_qm`), `--approx-mode old` branches in dbtopk/dbknn, ann.rs approx/dial configs, `--fit-dial`/`--soar` in `setup2_at` | −300 to −350 | Exact answers no; the "old" approximate rows can no longer be reproduced | No for exact paths | bench/topk "old mode" rows and threshold-db 14 Nov rows become historical; keep the commit hash in the reports. bench/run.sh `--approx` and remeasure.sh `--soar --dial` must be edited. |
| 6 | **Make the measured configuration the default** (`int_det`, `gtab`, `repack` on) and route every cluster opening through `open_cluster` (incl. `row_interval2`, `probe`, `levels2`); then delete `cluster_q2`/`q2a`, `want_w`, and the non-gtab `build_lut(cq.w)` path if intervals can use combined tables | −40 to −80 | No (bounds are sound under either table builder) | Yes: anyone relying on the old defaults (ann.rs, bench scripts without the flags, Dial::fit) gets the measured path; k-NN interval refinement gets gtab tables | Re-run one e2e without flags and confirm it equals the flagged run; knn exactness in verify2 (400/400). |
| 7 | **Norm sort** (`nusort`, `NuCut`, `nu_range`, `nukey`, `numax`, `sort_by_nu`, 7 call-site branches) | −220 | No (off) | No | Owner decision: no native measurement found; either measure once or delete. |
| 8 | **One full-vector decide/score helper** (1.3) used by `decide_full_from`, `Flat::threshold`, `TopK::score`, `topk_f16`, `Flat::scores`, `exact_ge2`; drop `decide_full_packed` with `pscan` | −120 (−55 for pscan) | No if the f32 products and the band are unchanged (they are the same kernel) | Inner-loop change: needs an interleaved scan timing | scanbench/dbcmp `scan` and `flat` variants; answers equal by assertion already in scanbench. |
| 9 | **Kernel consolidation**: one f64 dot (`dot64v` or `util::dot64`), one rotation path (dgemm or matvec), `qv_exact_r` fixed at `<4,1>` and `qvbench` dropped, `util::dot32_64` (v1) gone, `second_assignment`'s local dot gone (with 5) | −80 | Not the sets (all bounds carry rounding slack), but individual bits of q·m and rotated weights change, so cut ranges, candidate counts and intervals can move by an item | Small | verify2 at 50k and 1M (0 wrong, 0 violations), oracle_dump; compare touched/bytes per query before/after. |
| 10 | **Drop the AVX-512 twins** or gate them behind a measured win (q.rs 4 kernels, kern.rs Gather/plan_gather/gather_rows/part_sums, kernel.rs dot_f16/x4/dot_i8) | −300 | No | On an AVX-512 host, slower (simd.rs emulation) [I] | Owner decision: is any AVX-512 host in scope? If yes, add a CI build with `-C target-feature=+avx512f...` so the twin is at least compiled and tested. |
| 11 | **Shared helpers in q.rs**: `support(ix, pr, ci)` (5 copies), `Vis2::rows_in` (4), `nvis(ix, vis, ci)` (8), `row_as_query` (3), wrapper chains of 1.6 (`prep2_qm`, `group_q2`, `full_err_qn`, `new_light`, `knn2`), `truth` compaction into `compact_det` | −120 | No | No | Build + tests; one e2e. |
| 12 | **One support-table builder** (1.8) parameterised by the cluster projection | −50 | No | Build time only | Compare `ix.hi/lo` arrays bitwise before/after on the 50k index. |
| 13 | **Synthetic scale path** (`Full2::Synth`, `Synth2`, `SynthSrc`, `scale::run2`, the Synth arms in the core) | −400 | No | No | Owner decision: superseded by the planned real 20M/100M runs? |
| 14 | **run2.rs benchmark plumbing** (1.9): shared `approx_ladder`/`fit_dials`/`append_csv`/`load_vis_sets`/`thresholds`/`truth`; fold `scanbench` (and possibly `filter2`) into `dbcmp`; one `pool`/`out!`/arg parser; one `RealSource` mapping in `setup2_at` | −300 to −400 | No | Resident memory may fall (fewer mappings) [I] | Keep CSV columns identical (plotting scripts read them); diff one CSV per mode before/after. |
| 15 | **Dead items of 2.2** (`calib_quantile`, `split_t`, `Meta2::csg`, `Cl2::smax`, `GroupPlan2::states`, `build2`'s `nanch`, `Stats::clusters_novis`), stale `Plan::default` comment | −25 | No | No | Build with no `dead_code` warnings. Keep `npow` (documented contract). |

Rough total for 1–5 and 7, 11, 15: about −6,200 lines; for everything: about −7,500 of ~16,400.

### Order of work suggested

1, 2 and 15 first (no behaviour change, largest cut, unblock warnings). Then 3, 4, 11 inside q.rs (the planner and
`run_cluster_from` become readable). Then 6 (defaults) with one interleaved e2e. Then owner decisions 5, 7, 10, 13.
Benchmark plumbing (14) last, since it changes no index code.

---

## 4. What I did not verify

- Exact line counts are from `wc -l` on files and line ranges, not from a deletion diff; ±10%.
- I did not build, so "warns about" is the owner's list confirmed by grep, not observed.
- Whether any AVX-512 host still builds this crate (10) is inferred from comments and the README.
- Whether the norm sort (7) and the synthetic scale path (13) have value: no measurement found in rust/REPORT*.md,
  bench/ or SYSTEM.md beyond "built and off" / "stopped during its build".
- Whether removing hot-loop branches (4) changes timing measurably: expected small or favourable, unmeasured.


---

# Appendix: performance review


Scope: `rust/src/v2/{q,mod,kern,disk}.rs`, `kernel.rs`, `simd.rs`, `util.rs`, `query.rs` (the `flat` baseline) and the
`dbcmp`/`dbtopk` drivers in `run2.rs`, against SYSTEM.md, NEXT.md and REPORT_PLANNER.md. Nothing was built or run.
Each finding has a **[V]** tag for what the code shows and an **[I]** tag for what I infer, such as magnitudes,
causes or hardware behaviour, which needs a measurement. Line numbers are from the checkout at review time.

Ranked by expected effect on the owner's goals: interactive latency, never slower than a plain scan under a filter,
and billions of items on one machine.

---

## 1. k-NN selection refines one row at a time and restarts the index path from level 0 each time

**[V]** `knn2d` (q.rs:3733) passes `row_interval2` as `select_knn`'s `refine`. For a single row, `row_interval2`
(q.rs:3171–3189) does all of the following:

- finds the cluster by binary search;
- computes `qm = dot64(q64, centres_f64(ci))` (6 KB of f64);
- builds a fresh `CQ2` with `cluster_q2`, which allocates `w` (2n f32) and runs 4 + NDL `dot32`s over n;
- allocates a fresh `State::default()` (about 25 Vecs, all grown by `reset`) plus three result Vecs;
- runs `run_cluster` in `Interval(level)` mode, which builds the LUT for every level 0..=level (`cq.luts` starts
  empty) and decodes every level up to `level` for that one item.

`select_knn` (q.rs:3748–3776) then calls it again for the same row at level + 1. A row refined from level l to level L
therefore rebuilds tables and decodes about (L − l)·L/2 level-passes. Different rows of one cluster share nothing.

**[I]** One call costs table builds of up to ~500 units plus setup and allocation, roughly 10–50 µs. An exact score
costs about 0.1–0.2 µs for the f32 dot (SYSTEM: a full vector is about 104 ns), plus the i128 product when needed
(finding 9). For any candidate that needs refining, going straight to the exact score is 2–3 orders of magnitude
cheaper per step and removes the rounds.

**Change.** Use `refine = |r, _| exact(r)`, in effect `next = nl` for every uncertified candidate. If interval
refinement must stay, batch it: group the uncertified candidates by cluster and run `run_cluster` once per cluster
per round on all of them, keeping each cluster's `CQ2` and LUTs across rounds (a `HashMap<ci, CQ2>` in the selection).
Also replace `seen: HashSet<u32>` (SipHash, q.rs:3719) with sort + dedup.

**Measure.** `dbtopk` exact path (`knn2`, run2.rs:1563): the `t_sel` phase (ann.rs:94 accumulates it) at k = 10, 100
and 1000, before and after; also `st.full_reads`.

## 2. Fixed per-query cost: the cut reads up to three f64 copies of the centres, and group rotations are re-read

**[V] Anchor, centres and centre directions are read separately.**

- `anc_among` (q.rs:105–126) reads `anchors` (f64, n per live cluster, 6 MB at 1M).
- The cut then reads `centres64` for `qm` (q.rs:2639) and `cl[ci].mhat` for `pm = anchors[a]·m̂_c` (q.rs:705), each
  6 KB per cut cluster.
- The build makes every anchor a centre direction (`build_table2`, mod.rs:1832–1834), so the same direction is held
  three times in f64: `centres64`, `cl.mhat` and `anchors`.
- `prep2_qm` (q.rs:147–166) already shows how to get the anchor as argmax qm/|m| from the qm values, and `knn2_fast`
  uses it, but the threshold path does not.

**[I] Fixes.**

- Fuse into one pass over `centres64` for live clusters that computes `q·m_c`. Take the anchor as argmax `qm/|m|`
  and do one f64 dot for β.
- Then make one pass over `centres64` for the open clusters, computing `m̂_a·m_c` and taking `pm = (m̂_a·m_c)/|m_c|`.
  This needs a rounding term in `rp`. It is the one place that needs a short soundness check.
- That reads about 6–12 MB instead of 12–18 MB, and drops `anchors` and `cl.mhat` (12 MB at 1M).
- SYSTEM 4.2 puts the cut at 1.8–1.9 ms and the anchor at 0.1–0.28 ms (REPORT_PLANNER).
- f32 copies with an added error term would halve the bytes again.

**[V] Group setup is not batched on the k-NN paths.** `get_gq2` → `group_q2` → `group_q2r(rot = true)` (q.rs:436–444,
323–384) multiplies by every rotation (`matvec64`, q.rs:375): J matrices of 96², 128², 256² and 288² f64, about 1.4 MB
per group. `setup_groups` (q.rs:388–434) reads each rotation once for all groups (dgemm), but only `run_planned` and the
norm-sort cut call it. The callers that set up groups one at a time are:

- `seed2` → `probe` (q.rs:3212), up to 64 clusters across many groups;
- `knn2_fast` (q.rs:3662), per probed cluster;
- `row_interval2` (q.rs:3180).

**[I]** At 20–40 groups that re-reads 30–55 MB of rotations per k-NN query, a few ms. **Change.** In `seed2`, call
`setup_groups` on the groups of the 64 chosen clusters before `probe`. In `knn2_fast`, call it on the groups of the
first `nprobe` clusters, or in batches of 8.

**[V] The seed's cluster work is redone.** `probe` opens up to 64 clusters (`cluster_q2` plus LUTs for levels 0–3 and
their decodes). `run_planned` then reopens the same clusters from scratch. **[I]** Caching `CQ2`/LUTs per cluster in
`Prep2` (like `gq`) would remove this. The modes differ (`cluster_q2` against `open_cluster` with `gtab`/`int`), so it
only pays if the seed uses the same mode.

**Measure.** `dbcmp --cmp-what prep,anc,index` gives the fixed costs. `breakdown2` (run2.rs:855) gives P_ANCHOR, P_CUT,
P_BASIS and P_ROT. For k-NN, use `dbtopk` `t_seed`.

## 3. Sparse survivors are gathered from up to 32 source blocks per level, and nothing prices it

**[V]** When a cluster's candidates fill less than `occ(0)` of their span (q.rs:1114–1119; 0.6, or the timed
`repack`), `run_cluster` takes the gather path. For every level and every dest block it runs `plan_gather` and then
`gather_rows` on the codes and the scalars (q.rs:1406–1419).

- The AVX2 `gather_rows` (kern.rs:322–340) does, for each pair of rows, one 32-byte load, two `pshufb` and two
  `blendv` per distinct source block. `nsrc` is up to 32.
- Under a 1% visible set on the uncompacted index, each source block (32 items) holds about one candidate. Filling one
  dest block then reads 32 source blocks' rows, that is `units × 16` bytes per survivor instead of `units/2`, at every
  level.
- `calibrate_repack` (q.rs:1883–1952) times gathers from exactly two source blocks (`lanes = 64d + 2i`).
- `Plan::calibrate` fits its index-path scale `s` only on unfiltered queries (`vis.is_none() && t == t10`,
  q.rs:2178).
- Neither calibration sees the 32-source case.

**[I]** This is my first hypothesis for NEXT's "the uncompacted index path over sparse clusters costs more than the
model predicts" (22 ms forced at random 1%, 42 ms at 10%). The cost per survivor per level grows about as
32/(survivors per source block), and the model's per-candidate term is density-free. A second, smaller part: the
per-cluster fixed costs (`cluster_q2w`, one table build per level reached, `State::reset`) are spread over about 10
candidates instead of about 1,000. The model has a per-cluster term, but its scale is fitted on dense clusters.

**Change.**

- (a) Give the planner a density term. Per open cluster, ρ = ncand / (d1 − d0), already known in `plan2`. Price the
  index path per candidate as `item_us(d,p) · (1 + γ·max(0, 1/(32ρ) − 1))` when `ρ < occ(0)`. Fit γ in `calibrate`
  from the `tenth` set, which is already timed but used only for `item_us2`.
- (b) Prefer compaction, which exists (`Index2::compact`) and removes the gather entirely.
- (c) For very sparse clusters, consider running the index path on the deterministic levels only, then full vectors.

**Measure.** `dbcmp --cmp-what forced,cindex:...` under random 1% and 10%. In `breakdown2`, P_REPACK is the gather
time, separate from P_DECODE. Compare it with `C_TSURV`/`C_TABLES` against the unfiltered run.

## 4. What the cost model does not price, or prices from a different operation than the one that runs

**(a) The index path leaves out its own survivors' full vectors.** [V] `index_us`/`index_us_p` (q.rs:2068–2074) is a
per-candidate and per-cluster cost with no full-vector term. `cand_us` adds `disk_us` (q.rs:2682), but `idx_us` does
not (q.rs:2678). At a 50% in-distribution answer, 8.4% of rows go to full vectors after the index path (SYSTEM 4.1),
against 0.1–0.7% at small answers. [I] The single scale `s` is fitted at the 10th best score, where that share is
near zero, and `item_us2·p` is linear in p while the share peaks near 50% and falls at 75%. This fits NEXT's "241
genera 16–19% behind the scan at 50–75%". Under a cap, each such survivor is a scattered read that costs nothing in
the model. **Change.** Add `ncand · φ(d, p) · (full_us_eff + io)`, with φ measured during `calibrate` from
`st.enter[nl] / st.touched` at the shares it already times (10th best and 30%), plus 50% and 75%.

**(b) A filtered scan on file is priced as a sequential read.** [V] `row_us` adds `rb / io_gbps` for every scanned
row (q.rs:2583). Yet `scan_visible` only uses the io_uring reader when `vis.is_none()` (q.rs:2337). A filtered scan
faults scattered rows through the mapping (or `madvise(WILLNEED)`). [I] Under a cap, a random 1% scan is priced at
about 0.47 µs per row, while it costs about one page read per row (`io_us` is 5.7 µs at QD 64, more through faults).
The planner then prefers a scan that is about 10× dearer than modelled. **Change.** Use
`disk_us(nvis, span_of_visible_rows, rb)` for filtered scans, the same rule as candidates.

**(c) A codes-only compacted index on file is priced as io_uring but read through faults.** [V] `own_rows()` is None
when `vrow` is set (mod.rs:956–961). `run_planned`'s reader is therefore None (q.rs:2980) and `full_cached` returns
true (mod.rs:963–965). Every full vector is decided through the mapping, as synchronous faults, while `plan2` still
adds `io_us`-based `disk_us`. **Change.** Price `vrow` indexes on file with a fault cost, or give them a reader over
the parent's rows mapped through `vrow`.

**(d) `cscan_us`/`scan_us` are timed on k-NN scans of a cold working set.** [V] `calibrate` times `scan_knn` (the
top-k kernel `topk_f16`, query.rs:975), not `scan_visible`, in two runs (q.rs:2113–2123):

- unfiltered, which streams 1.5 GB from DRAM;
- over a fixed random eighth of rows (190 MB).

[I] This explains the miss NEXT names (a cached scan priced at 0.10 µs/row against 0.06–0.08). A 19-genus compacted
session is about 15 MB and stays in the 5900X's 32 MB L3 across `dbcmp`'s five back-to-back repetitions, so it runs
compute-bound, not bandwidth-bound. Note that the index path's codes (4.2 MB) are also warm there. Lowering only the
scan constant would bias the choice the other way.

**Change.** Use two constants, `cscan_hot` (one scan of a ~4 MB compacted slice, repeated) and `cscan_cold`. Choose
between them by the session's bytes against the LLC size (`/sys/devices/system/cpu/cpu0/cache/index3/size`), or keep
a per-session moving average of the measured µs/row, since every scan already measures itself. Apply the same
hot/cold split to the index path's constants. Time `scan_visible` itself.

**(e) The planner's own work is not counted, and is partly done twice.** [V] `answer_share` reads 256 scattered full
vectors. It runs once for `p_ans` whenever `auto && item_us2 > 0` (q.rs:2677), even when the plan is not sampled, and
again inside `in_band` (q.rs:2670–2675, 2747). [I] That is about 25–150 µs per query, cache-dependent, against queries
of 0.44–0.98 ms in the small sessions where the planner loses. The anchor search, sample cuts and `answer_share` are
sunk before the third scan check and appear in no comparison.

**Change.**

- Compute `p` once, lazily (OnceCell), and only when `item_us2 > 0` and the decision can change.
- Use the sampled clusters' candidates as the sample: rows already being touched, instead of 256 more scattered rows.
- Add an `anc_us + sampled cuts` term to the early scan checks (the second check already has `anc_us`).

**Measure for (a)–(e).** `dbcmp --cmp-what index,flat,scan,cvindex,cvscan` under 19/241/964 genera and random 1/10/50%,
logging `r.scanned` against the faster variant per (query, share) (dbcmp counts `nscan`). Capped: `run_ours.sh` with
the new planner (NEXT: "Unmeasured: run the capped comparison with the new planner").

## 5. Why the plain scan beats the compacted scan by about 20% under genus sets

**[V]** The two rows in REPORT_PLANNER are not the same code. `dbcmp`'s `flat` (run2.rs:1391–1396) is
`Flat::threshold` (query.rs:917–956):

- it reads the **npy file mapping** in file order;
- it maps rows to entity ids inline;
- it sorts the ids.

`cvscan` (run2.rs:1367–1372) is `prep2(sub)` plus `scan_visible(sub, …, None)` over the compacted anonymous
`F16Rows` copy. It returns rows, which `to_bitmap2` then maps. Both use `dot_f16_x4`, but the memory, the per-row code
and the per-chunk overheads all differ. Per chunk of 1,024 rows, `scan_visible` makes a `ThrOut2` with `Stats::new`
(five Vec allocations) and a 4,096-capacity row Vec (q.rs:2369–2370). Per row, `decide_full_from` calls `get` twice
(q.rs:2432, 2436), and each `full_row_f16` goes through the `Full2` match, a `vrow.is_empty()` test, the `Rows`
match and a bounds check. It also bumps `acc.st.full_reads`.

**[I]** At 494k rows (760 MB in 36 against 44 ms, 21 against 17 GB/s), the scan is DRAM-bound. Per-row instruction
overhead (about 10 ns of ~75) and per-chunk allocations (about 1 µs per 75 µs chunk) explain at most a few percent. The
likeliest remaining cause is the memory backing:

- page size (THP) and TLB reach of the late heap allocation against the page-cache mapping;
- physical contiguity, which affects prefetcher streams and DRAM page hits.

On WSL2 the page size and the availability of huge pages after a 6 GB process are both uncertain. On random sets the
file order scatters the visible rows, so the compacted copy wins there. That matches.

**Tests.**

1. Read `/proc/<pid>/smaps` for the `F16Rows` region (AnonHugePages) and the npy map (FilePmdMapped), and run
   `perf stat -e dTLB-load-misses,LLC-load-misses` on `flat` against `cvscan`.
2. Add `madvise(MADV_HUGEPAGE)` on `F16Rows` (and on the parent's copy) at allocation.
3. Separate code from memory. Run `Flat::threshold`'s loop over a `Flat` view of the compacted `F16Rows`, and run
   `scan_visible` with a `Rows::Map` over a file written from the compacted copy.
4. Compare `scan` (scan_visible on the parent, `vis` set) with `flat` on the same genus set.

**Cheap code fixes either way:**

- hoist `full_row_f16` dispatch out of the loop: resolve to a base pointer and stride once per chunk when `vrow` is
  empty and the rows are `Mem`;
- call `get` once per row;
- keep per-thread scan accumulators (`fold` as in `run_planned`) instead of one `ThrOut2` per chunk.

## 6. Small allocations and copies in per-cluster and per-chunk loops

All **[V]**, with effect **[I]** at a few percent, mostly visible in compacted small sessions of 0.4–1 ms:

- `run_planned`, per open cluster:
  - `cands_of` materialises a `Vec<u32>` (q.rs:2527–2533, 3073). For `vis = None`, the case of every compacted
    session, it is just the range `d0−r0..d1−r0`.
  - `pend = Vec::new()` (q.rs:3075).
  - `ivv = Vec::new()` (q.rs:3083).
  - `Pre::Direct` copies its visible rows with `.to_vec()` (q.rs:3040–3043) or collects the range, when
    `decide_full` takes a slice.

  **Change.** Keep `cands`/`pend` buffers in the fold state (next to `State`), pass ranges, and drop the `to_vec`.
  At about 1,000 clusters that is ~2–3k allocations per query.
- `build_lut` allocates `tabs: Vec<__m512>` per (cluster, level) (kern.rs:45) and grows it unit by unit.
  `combine_lut` heap-allocates `lv`/`hv` when a part has more than 136 units (kern.rs:818–827), which the 288-coordinate
  block can reach at higher rates. **Change.** Keep the scratch in `LevelLut` or `State`.
- `State::reset` zero-fills `a64` (f64, verify only) for every cluster (q.rs:953) and grows `truth`/`a64` even when
  not verifying (q.rs:941–944). **Change.** Gate both on `verify`.
- `scan_visible`, per chunk: `Stats::new` with five Vecs, plus a 16 KB row Vec, plus the merge (q.rs:2368–2396).
  **Change.** Use `fold`.
- `Reader::push` allocates `sp`, `at`, `rx` and `ext` per batch, and `reap` allocates `done` per call (disk.rs:311–323,
  396). This is minor next to I/O.

## 7. The AVX2 emulation of compress goes through the stack in the compaction loop

**[V]** On the measured machine (Zen 3, no AVX-512), `_mm512_maskz_compress_epi32`/`_ps` (simd.rs:223–234) do two
`vpermd`, store both halves to a stack buffer (the second at a variable offset), then reload 64 bytes across both
stores. `cs()` (q.rs:974–977) and `push_rows` (q.rs:987–992) call it for every kept 16-lane group of every live array.
In the randomised levels that is up to 3 + 4J = 19 arrays per 16 lanes (`compact_rand`, q.rs:1035–1052).

**[I]** A load spanning two earlier stores cannot be forwarded on Zen 3. That stall (about 10–15 cycles) comes on top
of the shuffles. This is a plausible large share of SYSTEM's 9 ms of "re-pack and compaction" at 50% answers.
**Change.** Store the two compressed halves directly at `w` and `w + popcnt(lo)`, as `cs` already permits writes past
the kept count (w ≤ o). `push_rows` can do the same into the reserved Vec tail. Alternatively, compact all arrays with
one shared index vector per 8-lane half instead of reloading the LUT per array.

**Measure.** `breakdown2`, P_COMPACT cycles at the 50% answer.

## 8. The full-vector reads for scattered rows

**[V]** `decide_full_from` (q.rs:2427–2454) issues no software prefetch. `decide_full_packed` prefetches the next 32
rows but also copies every row into a buffer (q.rs:2458–2509), and it is off by default (`pscan`). **[I]** For
scattered rows, such as a filtered scan on the uncompacted index or the candidates of sparse clusters, prefetching row
k + 4..k + 8 without the copy should give most of the gain without the 1.5 KB store + load per row. **Measure.**
`dbcmp --cmp-what scan,flat` under random 1% and 10% on the uncompacted index.

## 9. Exact products: scalar i128, the query converted on every call, sometimes computed twice

**[V]** `exact_dot` (util.rs:108–110) runs `f16_fixed` (branch plus assert) on both operands for every dimension, then
an i128 multiply-add: 768 scalar i128 operations per call. The query's fixed values are recomputed on every call, even
though `prep2` computes them once (`fx`, q.rs:151) and keeps only `qhi`/`qlo`. In `decide_full_from`, a row that falls
back and is also wanted exact calls `exact_dot` twice (q.rs:2443 and 2448). `want_exact` is true on every k-NN pass
(`want_iv: true`, q.rs:3710), so every accepted full-vector row pays one.

**[I]** About 2–4 µs per call, against about 0.1 µs for the f32 dot. **Changes.**

- Keep the query's fixed values in `Prep2`.
- Compute once and reuse.
- Add a cheap certified tier: f16 × f16 products are exact in f64, so an f64 FMA sum with error ≤ n·2⁻⁵³·Σ|qᵢxᵢ|
  decides or ranks unless the bound straddles. Use i128 only when it does. Alternatively, accumulate per
  exponent-sum bucket in i64 and combine at the end, which is exact and vectorisable.

**Measure.** `dbtopk` exact `t_sweep`/`t_sel`; `st.exact_fallbacks`.

## 10. Disk path: `mincore` on every cluster, done twice

**[V]** Under a cap, `run_planned` calls `ix.full_cached(&pend)` per open cluster (q.rs:3099). That is up to eight
`mincore` system calls (disk.rs:92–102). Then `decide_full` → `willneed` repeats the same eight-call check when
`--willneed` is on (disk.rs:115). [I] At about 1,000 open clusters that is 8–16k system calls per query, consistent
with REPORT_CAPPED's residual 1.7 against 1.1 ms warm. **Change.** Check residency once per query on a sample of
the candidate rows. Alternatively, keep a per-cluster residency bitmap refreshed by `mincore` over the cluster's whole
row range (one call per cluster, or one per query over the touched range), and pass the result into `decide_full`.
**Measure.** `dbcmp` capped, `sys_ms` column.

## 11. Memory: what scales with N, the visible set and the clusters

| Structure | Scales with | At 1M | At 1B (clusters of 1,000; K = 1M) | Code |
|---|---|---|---|---|
| Support table `hi`/`lo` | K × nanch × 8 B, with nanch = K | 8 MB | **8 TB**; its build is K·N·n flops | mod.rs:1827–1871 |
| `centres64` + `cl.mhat` + `anchors` (same direction, f64, three times) | 24·n B per cluster | 18 MB | 18 GB | mod.rs:884–886, 825, 1831 |
| Group int8 bases | G·n²; G ≈ N/(20n) under SYSTEM 2.1's rule | 38 MB | 38 GB, and ~3 s of basis products per query | q.rs:328–333 |
| Per-query `GQ2` per opened group | ~50 KB × G_open | 3 MB | 3 GB | q.rs:61–74 |
| Centre tables `ctab` | ~units·34 B per cluster per level | ~16 MB | ~16 GB | mod.rs:902 |
| Escapes `HashMap<(u8,u8,u32),f32>` (SipHash) | ~4% of items × ~30 B | ~1.5 MB | ~1.5 GB, plus hashing on lookups | mod.rs:897 |
| Session `Vis2.bits` | N/8, whatever the visible size | 125 KB | **125 MB per session** | q.rs:764–774 |
| Session `Vis2.rows` | 4 B per visible row, built by sort | 2 MB at 50% | 2 GB at 50%, O(V log V) | q.rs:771–772 |
| Per-query result bitset in `to_bitmap2` | nent/8 once rows·64 ≥ nent | 125 KB | 125 MB zeroed and scanned per large answer | q.rs:3144–3151 |
| Per-query planner vectors (`pre: Vec<Option<Pre>>`, `live`, `use_idx`, `grp_gain`) | ~60 B × K, several O(K) passes | 60 KB | 60 MB and ~10 ms of passes | q.rs:2578–2785 |
| Vectors | 1,536 B per row | 1.43 GiB | 1.4 TB, on disk | — |

[V] for the structures and formulas; [I] for the 1B figures, which assume the build rules stay as they are.

Recommendations:

- Cap nanch: anchors = group centres, or a fixed 256–1,024.
- Cap G so that items per group grow with N.
- Keep one copy of the centres, f32 where the bounds allow.
- Add a group-level support bound so that the per-query cut is not O(K·n).
- Build `Vis2.rows` by scanning the bitset (already sorted, O(N/64 + V)) instead of `row_of_ent` twice plus a sort.
- Use roaring or per-cluster sparse visibility for small sessions.
- Build the result bitmap per container.
- Store escapes per cluster and level as sorted `(row, value)` arrays.

The process's 6.2 GiB against 1.8 GiB (SYSTEM 5) includes the npy mapping, which `Full2::Real.map` keeps but no query
reads except to carry it into compacted indexes. **[I]** `madvise(MADV_DONTNEED)` or an unmap after the row-order copy
is built would return about 1.4 GiB of RSS.

## 12. Rayon granularity and sharing

**[V]** No false sharing found. Per-thread accumulators come from `fold` (run_planned) or per-chunk values (scans).
Shared atomics on hot paths are read-only (`PROF` in `tick()`) or once per batch (disk counters). `run_planned`'s fold
creates `State::default()` and a `Reader` per split. With `Rows::Mem` the reader returns at once. With a reader, each
leaf drains its queue at the leaf's end (q.rs:3117–3123), which breaks read pipelining at leaf boundaries [I], a small
effect at 4 threads. `plan2`'s `pre.into_par_iter()` over 999 tiny closures (q.rs:2730) is fine at 1M. At K = 1M it
should use `with_min_len(256)`.

---

## Hypotheses for NEXT's three known misses (summary)

1. **Cached scan priced at 0.10 against 0.06–0.08 µs/row.** The constants come from `scan_knn` over 1.5 GB (cold,
   DRAM-bound) and a 190 MB scattered eighth. The 19-genus session (~15 MB) is L3-resident across `dbcmp`'s repeated
   runs. See 4(d). Also, the planner's unpriced overhead (anchor among 159 clusters, 256-row `answer_share`, sample
   cuts, about 0.1–0.2 ms, 4(e)) is about the gap between planned (0.65–0.98 ms) and scan (0.44–0.66 ms).
2. **The uncompacted index path over sparse clusters.** The gather from up to 32 source blocks per dest block at every
   level is unpriced and uncalibrated (finding 3). Per-cluster setup and tables are spread over ~10 candidates, with a
   scale fitted on dense, unfiltered clusters.
3. **Plain scan about 20% faster than the compacted scan under genera.** Different code and different memory (file
   mapping against a late heap copy). The likeliest cause is page size or TLB behaviour and physical contiguity, not
   the kernel. See finding 5 for the four tests that separate them.


---

# Appendix: correctness review


Checkout: `/home/user/evs`, branch `evs-merge` at ce9cf94. I only read the code. I did not build or run anything, and I wrote nothing to the checkout. Line numbers refer to ce9cf94. "Verified" means I traced it in the code. "Inferred" means it follows from the code but I did not measure or run it.

## Summary

I found **no path that returns a wrong exact answer** under the conditions the system documents: n a multiple of 32 and at most 2048, a visible set made of the index's own entities, and k ≥ 1. Every threshold path decides each row with a sound bound or an exact integer product, and a row is always decided against the viewer's visible rows. The paths are the scan, Direct, the index levels, the support table, the key and norm ranges, and the reader. The planner's choice changes cost only.

All three merges kept both sides' intent for the exact guarantees:
- `tail.rs` and the certified constants are unchanged since 20e1355^1.
- The one-sided budget `l1 = ln(N·L/ε)` is still used.
- `select_knn` is unchanged.
- `npow` is set on every `ThrOut2` and `KnnOut2`.
- N comes from the row count, and a compacted index inherits its parent's.

The problems found fall into four kinds:
- crashes on edge-case inputs;
- **interactions between compaction, vectors on file and the planner's costs that neither branch had**, which give wrong costs or timing and, under a cap, risk running out of memory;
- one statement in NEXT.md that is false;
- small issues with how k-NN results are reported and how the guarantee is recorded.

## Findings

### F1. A visible set holding an entity the index lacks crashes the query (crash; verified)
`q.rs:764-773` (`Vis2::new`) calls `ix.row_of_ent(e)` for every entity in the viewer's bitmap. `mod.rs:1095-1098` indexes `ent_row[e]`.
- `ent_row` has length `maxent + 1` and holds `u32::MAX` for ids with no row (`mod.rs:1756`). An id below `nent` that has no row gives r = 4294967295, and `bits[r / 64]` panics out of bounds.
- An id at or above `nent` panics in `ent_row[e]`.

Scenario: a viewer's ACL bitmap names entities that have no vector, or ids issued after the build. The query panics.

Fix: in `Vis2::new`, skip any e where `e >= ent_row.len()` or `ent_row[e] == u32::MAX`. Alternatively, intersect the bitmap with the index's entity set first. Either way, decide whether to report how many ids were dropped.

Related, also verified: the assert in `row_of_ent` (`mod.rs:1096`) exempts `nrows == 0`. On an index compacted to an empty visible set, the call then indexes an empty `ent_row` and panics without the intended message.

### F2. Compacted indexes never use the io_uring reader, and NEXT.md says otherwise (cost/timing, out-of-memory risk under a cap; verified, size inferred)
How each compacted form reads its vectors:
- `Index2::compact(rows, true)` copies the vectors into `disk::Rows::Mem(F16Rows)` (`mod.rs:1040-1043`). `Reader::new` returns `None` for `Mem` (`disk.rs:286-287`). So a codes-and-vectors (cv) index reads from anonymous memory, never through the reader.
- `compact(rows, false)` shares the parent's rows through `vrow`, so `own_rows()` is `None` (`mod.rs:956-961`). It reads through page faults on the mapping, without the `willneed` or `cached` hints.

NEXT.md:18-20 ("under a cap only codes-and-vectors compaction gets the reader") is therefore **false**: neither form uses the reader.

Consequences in `dbcmp`, which runs compaction before `--cap` (`run2.rs:1274-1293`):
1. **Memory under a cap (inferred).** cv vectors are anonymous memory that cannot be evicted and, with `MemorySwapMax=0` in `run_ours.sh`, cannot be swapped either. Examples:
   - The "all" set is compacted whenever `cindex`, `cvindex`, `cscan` or `cvscan` is asked for without `--skip-all`. Its cv copy is 1.43 GiB.
   - A 50% visible set's cv copy is about 0.7 GiB.

   Under the 776 MB cap either one either evicts the main index's page cache entirely or gets the process OOM-killed. A cv timing under a cap measures an index that holds its vectors in RAM, not one that reads them from disk.
2. **Wrong costs.** In a `uring` read mode, `plan.disk = true` (`run2.rs:1331-1333`) also applies to `cindex` and `cvindex` (`run2.rs:1374`). plan2 then adds sequential-read cost to every scanned or directly scored row (`q.rs:2583`) and `disk_us` to candidates (`q.rs:2682`), even for in-memory cv rows. The c form's reads through `vrow` are scattered page faults, yet they are priced at the in-order scan rate (`cscan_us`, because `vis` is `None`: `q.rs:2063-2065`, `2583`, `2680`).

Fix:
- Price from what the index actually reads: set `disk` only when `ix.own_rows()` is `Rows::Map` and URING is on. Price c-compacted rows as scattered (`scan_us`, or `io_us` per row on file).
- Either give the c form a reader over the parent's rows (push the rows mapped through `vrow` and map them back), or write the cv copy to its own file under a cap.
- Refuse or warn when a cv copy exceeds the cap. Correct NEXT.md.

### F3. On-file vectors scored directly, or by a filtered scan, bypass the reader while the planner prices them as sequential reads (timing; code path verified, size inferred)
This interaction is new in ce9cf94: `Pre::Direct` came from scan-planner, and the reader came from disk-reads.
- `run_planned` sends only the `Pre::Open` candidates through `rd` (`q.rs:3098-3110`). `Pre::Direct` calls `decide_full` directly (`q.rs:3038-3050`), so it reads through mapping faults, with readahead only under `--willneed`.
- `scan_visible` uses the reader only when `vis.is_none()` (`q.rs:2336-2339`).

Yet plan2 prices each direct or filtered-scan row at `scan_us + rb/io_gbps` (`q.rs:2583`), that is, 1,536 B at about 3.3 GB/s ≈ 0.47 µs. A cold scattered row actually costs a synchronous random read, which is far more than 5.7 µs at QD 64. Under a filter on file, plan2 will choose Direct or Scan more often than it should, and the "planned never slower than scan" property is not established on file.

Fix: send the Direct `pend` through `rd` the same way as Open's `pend`. Price scattered rows as `min(nv · io_us, io_us + span·rb/io_gbps)` with `span` taken from the cluster's rows, as `disk_us` already does.

### F4. Planner sampling reads 256 scattered full vectors per query; capped runs use a plan file without plan2's constants (timing/cost; verified)
- `answer_share` (`q.rs:2546-2564`) reads 256 scattered stored vectors through `full_row_f16`. plan2 calls it for `p_ans` on every auto query that gets past the early scan checks whenever `item_us2 > 0` (`q.rs:2677`). It may call it a second time from `in_band` (`q.rs:2670-2675`). On file, under a cap and cold, that is up to 256 random page faults per query, and the cost model does not count them (inferred: tens of ms).
- `run_ours.sh` passes `--plan-file bench/threshold-db/results/ours/plan.txt`. That file predates plan2: it has no `cscan_us`, `anc_us`, `dir_us` or `item_us2`. `Plan::from_kv` silently keeps the BioCLIP defaults for them (`q.rs:2008`, `2018-2045`), for example `anc_us = 150` against the 13–275 µs measured in `REPORT_PLANNER`'s runs.

  The capped comparison that NEXT.md lists as owed would therefore run plan2 with uncalibrated scan rules. With `item_us2 = 0`, `answer_share` is skipped there, so the second point hides the first.

Fix: compute `answer_share` at most once per query. On file, either sample from the codes or charge the sample's reads to the cost. Regenerate the capped plan file with the current `calibrate`, or have `from_kv` warn about missing keys.

### F5. Exact k-NN reports +inf scores for certified items and orders them by entity id (wrong scores and order, not a wrong set; verified)
`select_knn` returns `f64::INFINITY` for every candidate it certifies without an exact score (`q.rs:3780`). `knn2d` sorts by score, then by entity, and truncates (`q.rs:3733-3736`). The returned `ents` therefore carry +inf scores and are ordered by entity id among the certified ones.

Example: when k is at least the visible count, or the seed returns -inf because fewer than k visible items are in the probed clusters, every candidate is returned with +inf.

The set is correct. `verify2` and `dbtopk` compare sets or recompute exact values, so they do not see this. But `KnnOut2` reads as "(score, entity), best first", and `oracle_dump` writes the ids in returned order (`run2.rs:1119-1121`). Any ordered comparison or score consumer gets wrong data.

Fix: compute exact scores for the certified items (at most k full vectors) before returning, or document the +inf convention and sort certified items by lower bound.

### F6. k = 0 panics (crash; verified)
- `seed2` returns `ex[kk - 1]` (`q.rs:3490`): with `kk = 0` this underflows and the index panics.
- `topk_f16` indexes `v[k - 1]` (`query.rs:985`).
- `knn2_fast` asserts `kk > 0`; `knn2` and `knn2d` do not.

Fix: return empty for k = 0.

### F7. `--nguar` that is not a power of two: the recorded `npow` claims more coverage than the constants give (guarantee bookkeeping; verified)
`mod.rs:1522-1526` and `1556`: the constants use `l1 = ln(nguar·L/ε)` with N = nguar exactly, but `npow = ceil(log2 nguar)`.

Example: with `--nguar 1e9`, results carry 2^30 ≈ 1.07e9 while the constants cover only 1e9. A cache that invalidates only when `npow` rises would keep results while the index grows past 1e9 rows. The default (the row count rounded up to a power of two) is consistent.

Fix: round nguar up to 2^npow before computing `l1`, or store nguar itself in `Meta2`.

### F8. With `--alpha0`, exact k-NN spends the budget twice per (item, level) (soundness margin; inferred, off by default)
`knn2d`'s sweep opens clusters with `open_cluster`, which uses α = 0 under `alpha0` (`q.rs:2953-2954`). `select_knn`'s refinement calls `row_interval2` → `cluster_q2`, which always uses α = `alr` (`q.rs:3182`, `524-525`). The randomised error A·u·PΔ depends on Δ = qv − α·mv, so the same (item, level) yields two different failure events. The union bound over N·L events then covers only half of them, which can double ε.

With the default (`alpha0` off) both use `alr` and produce identical events, so the default is sound.

Fix: refine with the same α the sweep used (pass `plan.alpha0` to `row_interval2`).

### F9. Latent: some integer kernels drop the tail past a multiple of 16 or 32 (no effect today; verified)
`kernel.rs:625-636` and `708-722` (`dot_i8`, both versions) have no tail loop and no error term for it. Nothing breaks while n % 32 == 0, which `setup.rs:17` asserts on the index path. But a scan-only query never reaches that assert.

The f16 kernels and `full_err_qn` (`q.rs:845-847`) now handle and bound the n % 16 tail. I re-derived the accumulation depth: n/16 + 4 for the AVX-512 x4 kernel, n/32 + 5 for the AVX2 kernel, both no more than n/16 + 6.

Fix: assert n % 32 == 0 at build or open, so that every path relies on it.

### F10. Minor cost issues (cost only; verified)
- `Plan::calibrate` divides by `scan_n`, `cscan_n`, `cut_n` and `nq` without guards (`q.rs:2195-2200`). On a tiny index with an empty sampled eighth these give NaN costs. Every NaN comparison is false, so the plan goes to Run, and answers stay exact.
- `knn2d`'s `scan_first` and `knn2_fast`'s choice between full vectors and the index ignore `plan.disk`.

## The exact path: checked and holding (verified unless noted)
- **Constants and budget:** `tail.rs` is unchanged by the merges (`git diff 20e1355^1 ce9cf94 -- rust/src/v2/tail.rs` is empty). `level_constant` takes the smallest of the certificate (subset-covering, budget-checked), convex order and Chernoff (min over λ of max over w, which is conservative), with a 1e-6 margin and capped at the sub-Gaussian value. Single-block `tail_s` is an upper Riemann tail. The randomised half-width is `(c·sd + det)·(1 + 1e-6)`, and the dial scales only the upper side (`q.rs:1603`, `1652`).
- **`npow` and N:** set on every `ThrOut2` and `KnnOut2` construction (`q.rs:2347`, `2369`, `2391`, `2990`, `3126`, `3456`, `3683`, `3696`, `3738`). A compacted index clones its parent's `Meta2`. `Plan::calibrate` refuses a compacted index.
- **One-sided budget in k-NN:** in `knn2d`, each (item, level) relies on the upper bound if the item is in the true top k and on the lower bound otherwise, in both the sweep and the selection. R6 applies, with the caveat in F8.
- **`select_knn`:** dropping uses a strict >, certification uses ≥, and both counts exclude the candidate itself. Ties are broken by entity id, as in `topk_f16`, so the scan and the index return the same tied set. When k exceeds the visible count, every visible item is returned (with +inf, see F5).
- **Floating point:**
  - Full-vector decisions: f32 score with the bound `(n/16 + 6 + n%16)·2⁻²⁴·|q|·max|x|·1.01`, where `max|x|` comes from the f16 vectors (`mod.rs:1158-1173`), then an exact i128 product near t.
  - `exact_ge` uses `ceil(t·2⁴⁸)`, consistent across paths.
  - The f32 bounds compare against `next_up_if_below(t32, t)`.
  - The 1e-9 slacks are absolute. That is fine for the normalised queries `run2` passes; a caller with |q| ≫ 1e6 would need relative slacks (inferred, API only).
- **Support table (plan2):** Skip when `bh + ρ·rr < t − 1e-9`, In when `bl − ρ·rr ≥ t + 1e-9`. This is sound for any anchor. The parent's extremes bound any subset, so the table stays valid on a compacted index. Pre::In pushes only visible rows.
- **Visibility:** every path decides from inside the visible set: the scan, Direct (`v.rows` slice), cuts (`count_bits` / `cands_of`), Pre::In pushes, `seed2` / `probe`, `answer_share`, `knn2_fast` and `Vis2::restrict`. Nothing aggregates over all rows before filtering.
- **Empty clusters and sets:** `nvis_cl == 0` gives Skip. `run_cluster` is reached only with non-empty candidates. An empty visible set scans 0 rows, or in Index mode returns empty.
- **t = −inf** (the seed short of k): the sorted range takes everything in. The norm range yields NaN for `cin` but falls back to "all undecided", and the float path then takes everything in.
- **Compaction:**
  - The nibble copy matches the read layout (`nib`, 16-byte rows per 32-item block) for both codes and scalars.
  - `esc` is re-keyed by `binary_search`.
  - `ahat` is padded for the 2-byte gather. `nukey` subsequences stay sorted.
  - `Cl2` is cloned, so α, `rmax`, `numax` and the other cluster fields match the parent.
  - `vrow` composes across nested compaction.
  - `verify2` checks both compacted forms against an exact brute force.
- **Reader** (`disk.rs`):
  - A batch is sorted, and `at[k]` matches `rows[k]`.
  - O_DIRECT extents are page-widened inside a file padded to whole pages.
  - Short reads are retried; `Drop` waits for reads in flight.
  - `cached` asks `mincore` about at most 2 pages, which holds for n ≤ 2048.
  - Decisions from batches are exact and carry exact values for k-NN (`want_iv`).

## Merge-by-merge
- **796a082 (plots-refresh).** The first parent had no source changes since the merge base, so nothing could be lost. `knn2_fast`, the dial at every level (deterministic float, integer `dmax`, randomised) and `dbknn` are present. The only change is that `Dial::fit` skips recall when no targets are given; its callers that pass targets (`run2.rs:275`, `1572`) are unaffected, and `run2.rs:1783` passes `&[]` and does not read the recall.
- **20e1355 (disk-reads).** The first parent's constants work (`tail.rs`, `index.rs`, `run2.rs`) survives intact; the merge diff against ^1 contains only disk additions. The exact side's `decide_full`, which handled its tail with `exact_ge2`, was replaced by `decide_full_from`, which uses `dot_f16_x4` throughout. At this commit `dot_f16_x4` still dropped the n % 16 tail, which was harmless only because n % 32 == 0; b79dbc8 fixed it, and the fix is present at ce9cf94. `npow` is kept everywhere.
- **ce9cf94 (scan-planner).** plan2, `Pre::Direct`, compaction and `F16Rows` are kept, and the disk costs were added to plan2 (both sides' intent). Every flag and mode in `run2.rs` and `main.rs` survives: I compared flag and mode lists across both parents and the merge. What neither side had, and what the merge left unhandled or mispriced: F2 (compaction with on-file vectors), F3 (Direct and the filtered scan bypassing the reader) and F4 (plan2's sampling on file, and the stale plan file).

## Suggested tests (no tests cover these today, as NEXT.md also notes)
- A visible set containing a missing entity id.
- k = 0, k = 1, and k greater than the visible count, against `scan_knn`, comparing ids in order.
- `compact` to an empty set and to a single-row cluster.
- A plan2 property test: the answer equals the scan's under forced `Pre::Direct`, Open, In and Skip choices.
- A capped smoke run with `cvindex` and a 50% visible set, watching `memory.events`.
