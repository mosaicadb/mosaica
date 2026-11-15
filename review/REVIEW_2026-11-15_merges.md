# Review of the merges and steps 1-4 (546375a..31bdf70)

Three independent reviewers read the range (merges and their interactions; what the deletions dropped; the benchmark
harness and the cost model). They read code only. Their full reports follow this summary.

## Answers

No path gives a wrong exact answer and no crash is reachable. The merges kept both sides' intent: certified
constants, one-sided budget, `select_knn`, `npow` on every result, N from the row count, `knn2_fast` and the dial,
the reader and the disk costs, scan-planner's planner, direct scoring and compaction. The AVX-512 kernels and the
synthetic scale path survive intact.

## To fix now (mine)

1. **The command line silently ignores unknown flags**, including every one removed (`--u8`, `--jump`, `--alpha0`,
   `--colstore`, `--variants`, `--soar`, `--approx`, `--approx-mode`, `--fit-dial`, ...). So `bench/run.sh --approx`
   runs and writes no approximate rows, and the documented old-mode commands (bench/topk/REPORT.md:194-201,
   bench/threshold-db/REPORT.md:588) run the new mode under the old labels. Fix: reject unknown flags with a message
   naming them.
2. **`--cmp-order variant` with `--cold`** runs every variant's cold runs before any warm pass; REPORT_CAPPED ran each
   variant's cold runs just before its own warm runs (its `--what flat,forced --cold` run). Fix: cold runs inside
   each variant's pass.
3. **Documentation**: SYSTEM.md still lists `bail` and the column store as built; bench/REPORT.md and
   bench/topk/REPORT.md describe `--soar`, `--fit-dial` and `knn2_dial`; README.md and the reports do not record that
   the measured configuration is now the default; `bench/run.sh --approx` needs `--dial` settings.
4. **`--full-file` naming the fitted file** would truncate the live mapping (SIGBUS): refuse it.

## Costs (the planner chooses; answers do not change)

5. **Calibration matches threshold queries but not exact k-NN**: since 0f93ea3 it times the integer path, but `knn2`
   runs the interval path (`want_iv: true`), so k-NN's index path is now underpriced. Either fit a second factor for
   the interval path or time both; this belongs with the `Plan` split (step 6).
6. **The index path's survivor reads are never priced** on file, while the full-vector path's are (from the disk
   branch); the index path looks better than it is at large answers under a cap.
7. Reads priced as sequential where execution faults pages (direct scoring, filtered scans, knn2's scan; codes-only
   compaction unpriced); pricing ignores the page cache; `answer_share`'s 256-row sample unpriced; `scan_band` timed
   with pre-calibration constants; `--plan-default` and the scale run price the integer path with float-path
   constants; plan files fitted before 0f93ea3 should be refitted.
8. A capped rerun differs from REPORT_CAPPED's: io_uring reads priced, the per-cluster planner with constants missing
   from ours/plan.txt falling back to defaults, compacted copies built before the cap.

## Design questions (yours)

9. **The dial on the integer path** scales only the code term at levels 0-5 (the table error and fixed term are not
   scaled); the float path scales the whole upper half-width, and `Dial::fit` assumes that. The measured approximate
   rows were all run with `--int`, so they already reflect this; the question is whether the integer path should
   apply the dial fully.
10. **A latent trap**: reader batches return file row ids as answer row ids, correct only because `own_rows()` keeps
    indexes that share a parent's vectors through `vrow` off the reader. Routing those through the reader (the
    natural fix for their unpriced reads) would return wrong rows unless batches carry answer ids.


---

# Appendix: merges


Range: `546375a..31bdf70` (first parent), checkout `/home/user/evs` at 31bdf70. Read-only. I built and ran nothing.
"Verified" below means I read the code at HEAD and, for merges, the `--cc` combined diff and the diffs against each
parent. "Inferred" means I reasoned about runtime behaviour I did not observe.

Summary: I found **no path that gives a wrong exact answer, and no reachable crash, at HEAD.** The merges kept both
sides' intent on the exact path. The findings are wrong costs, one approximate-path behaviour change caused by the new
defaults, and one latent hazard that the obvious fix for the cost problems would turn into wrong answers. NEXT.md
already records some of these. I mark those as known and list them only where I found something it does not say.

---

## Findings

### F1. knn2's index-path cost is now calibrated on a path that knn2 does not run (wrong cost; new in 0f93ea3)
- `q.rs:2012`: `Plan::calibrate` times the index path with `want_iv: false`, so it runs the integer deterministic
  levels (`run_planned` → `open_cluster(.., pl.int_det && !o.want_iv)`, `q.rs:2785`).
- `q.rs:3304`: `knn2` plans and runs with `want_iv: true`, which disables the integer path and also collects an
  interval for every taken-in row (`q.rs:2760-2772`).
- Scenario: an exact k-NN query on a calibrated plan. `plan2` compares `idx_us` (fitted on the faster integer path)
  against `cand_us` and `scan_cost`. That underestimates knn2's index-path cost by roughly the float/integer speed
  ratio plus the interval collection. Before 0f93ea3, calibration ran with `want_iv: true` and matched knn2; now it
  matches threshold queries.
- Consequence: the planner opens the index path for k-NN where the full vectors or a scan would be cheaper. Timing
  only; answers are unaffected.
- Fix: fit a second factor for the interval path (time one `want_iv: true` pass per pseudo-query) and have `plan2`
  pick the factor from `o.want_iv`. A simpler option is to scale `item_us*`/`cl_us*` by a stored float/int ratio when
  `want_iv`.

### F2. With `int_det` now the default, knn2_fast's dial only partly applies at the deterministic levels (approximate path behaviour change; c7737c9)
- Float path (`q.rs:1493-1495`): `ub = est + dial[l] * hh`, where `hh` includes `dwa*rho`, `fixed`,
  `s*(edet+ewdet)` and the f32 rounding term. The whole half-width is scaled.
- Integer path:
  - `q.rs:1201-1208`: `rout = (b + e + d*x)/sigma` with `x = dw*ntab + ewdet`. The dial scales only the code term
    and `ewdet`. The table error `e` is not scaled.
  - `q.rs:1119-1122`: the fixed term uses `dmax`, the largest dial over all deterministic levels, not `dial[l]`.
- `knn2_fast` opens clusters with `plan.int_det` (`q.rs:3260`). Before c7737c9 that was false unless `--int` was
  passed. It is now true, and so is `Dial::fit`'s recall pass (`q.rs:3068`).
- `Dial::fit` measures each level's ratio against the certificate's full half-width (`check_det`, `q.rs:1015-1022`).
  The integer path then applies the fitted factor to a smaller part of the width.
- Consequence: at dial < 1 the approximate mode prunes less at levels 0-5 than the factors intend. Recall goes up,
  speed goes down, and the per-level dial loses effect at the deterministic levels. The plots-refresh measurements
  (float path unless `--int`) do not describe the default any more. Exactness is not affected: lower bounds are
  unscaled, and at dial = 1 both paths are certified.
- Verified the formulas by reading. The size of the effect (fixed + table error relative to `dwa*rho`) is inferred.
- Fix: scale `e` by `d` in `rout`, and either keep a per-level fixed term in the integer state or document `dmax` as
  a deliberate conservative choice. Alternatively, run knn2_fast on the float path whenever a dial < 1 is given. In
  either case, refit the dials and re-measure the dbknn rows on the default.

### F3. Read costs are charged where rows are not read the priced way: knn2's unfiltered scan (wrong cost; partly new)
- Known (NEXT.md:38): `Pre::Direct` and the filtered scan read through the mapping but are priced at sequential
  bandwidth.
- Not recorded in NEXT.md: `knn2`'s `Planned::Scan` (`q.rs:3306`) goes to `scan_knn` (`q.rs:2272`). `scan_knn` reads
  every row through the mapping (`ix.full_row_f16`) and never uses the io_uring reader, even unfiltered with
  `uring` on. `plan2` still prices those rows at `cscan_us + rb/io_gbps` (`q.rs:2449`).
- The mapping is `MADV_RANDOM` by default (`--full-advice random`, `run2.rs:205`), so a cold scan is page-by-page
  synchronous faults, not a 3.3 GB/s stream (inferred).
- `Plan::scan_first` (`q.rs:2154`, used by knn2 at `q.rs:3296`) has no disk term at all.
- Consequence: on an on-file, priced index, k-NN scan decisions are made on costs several times too low when cold.
  Timing only.
- Fix: give `scan_knn` the reader path that `scan_visible` has (`q.rs:2206-2236`), or charge
  `rb/io_gbps` only where the reader is used and a per-page fault cost elsewhere. Add `disk` to `scan_first`.

### F4. Latent hazard: reader batches hand back file row ids as answer row ids (would become a wrong answer)
- `decide_batch` (`q.rs:2687-2693`) and the scan's `decide` (`q.rs:2211-2215`) push `b.rows[k]` into `acc.rows`, and
  into `acc.exact` under `want_iv`. Those ids are what was read from the file.
- This is correct today only because `Index2::own_rows()` (`mod.rs:964-969`) gives the reader nothing when `vrow` is
  non-empty, so row id == file row.
- The natural fix for the known unpriced faults of codes-only compaction is to route a `vrow` index through the
  reader. `vrow` is ascending, so the mapping would even preserve order, which makes this look safe. If someone does
  it by translating `pend` through `vrow`, the batch rows become parent rows, and the compacted index returns parent
  row ids:
  - `to_bitmap2(sub, ..)` maps them through `sub.row_ent`, which gives wrong entities or an out-of-bounds panic.
  - k-NN's `select_knn` gets wrong rows.
- Fix now, cheaply: make `Reader::push` take `(ids, file_rows)` and return `ids` in the batch, or put the answer row
  ids in the batch's `tag`. Add `debug_assert!(ix.vrow.is_empty())` where a batch's rows become answers.
- Related cosmetic issue: `Index2::full_cached` (`mod.rs:971-973`) returns `true` for a `vrow` index whose parent is
  on file, although its doc says "always, unless they are on file". It is harmless today because the only caller also
  needs a reader (`q.rs:2799`). Fix the doc or translate through `vrow`.

### F5. `priced` can disagree with how rows are read (wrong cost; low)
- `priced` is set independently of `uring`:
  - setup sets it equal to `uring` (`run2.rs:218`);
  - dbcmp's `:plan=disk` sets `priced` without `uring`, and `:plan=mem` clears it with `uring` on
    (`run2.rs:1238-1239`).
  - `mmap:plan=disk` prices io_uring-shaped reads (`io_us` per row, coalesced spans) while every read is a mapping
    fault. Intended as an experiment knob, but the doc comment on `ReadPolicy::priced` (`disk.rs:22`) does not say
    the two can diverge.
- Warm runs: the reader is skipped for rows the page cache holds (`q.rs:2799`, `disk.rs:111-121`), but `plan2`
  charges `disk_us` regardless. That biases warm runs toward the index path. Inferred and by design, but it means a
  warm `uring` run and a warm `mmap:plan=mem` run differ in plans, not only in reads.
- `:gap=BYTES` (`run2.rs:1240`) sets the gap by changing `plan.io_us`, which also changes every `disk_us` price. A
  "gap" sweep is therefore also a cost sweep. (This is harness territory; I flag it only because it changes planner
  decisions.)
- Fix: document `priced` as "planner charges", independent of `uring`. Give `io_gap` its own field instead of
  deriving it from `io_us`.

### F6. Cold planning reads 256 rows through the mapping (timing; low)
- `answer_share` (`q.rs:2410-2428`) scores 256 evenly spaced visible rows via `full_row_f16`. It is called from
  `plan2` whenever `item_us2 > 0` (`q.rs:2543`); `calibrate` usually fits it above 0. It is also called in `in_band`.
- On an on-file index after `evict` (`--cold`, each read mode), that is up to 256 synchronous faults inside planning.
  This time is neither priced nor attributed to reads (it counts as plan time). The sample positions depend only on
  the visible set, so warm runs hit cache (inferred).
- Fix: when `own_rows()` is on file and the sample is not cached, use the reader for the sample, or skip `p_ans` and
  set it to 0.

### F7. `--full-file` equal to a mapped source file truncates a live mapping (crash; low, pre-existing on research/disk-reads)
- `Rows::to_file` does `File::create(path)` (`disk.rs:63`). If `--full-file` names the fitted f16 file, which `Full2::Real.map`
  (`mod.rs:885`) and `RealSource` (`run2.rs:163`) still map, the truncation makes those mappings raise SIGBUS on
  their next access.
- Fix: refuse a path that canonicalizes to `p.xs_path`, or write to a temporary file and rename it.

### F8. O_DIRECT partial-read retry can submit an unaligned read (crash; theoretical)
- `Reader::reap` (`disk.rs:435-437`) re-queues a short read at `offset + done` with length `len - done`. For a
  `Via::Direct` extent, an unaligned `done` gives EINVAL, and the `assert!(res > 0)` panics. Short O_DIRECT reads are
  rare once the file is padded to 4096 (`disk.rs:66-68`).
- Fix: on a short Direct read, round `done` down to 4096 before retrying, or fall back to `rfd` for the remainder.

### Known and still true (not repeated as findings)
NEXT.md:33-40 and :144 cover these: codes-only compaction's faults are unpriced; `Pre::Direct` and the filtered scan
are priced as sequential; the full vectors that index-path survivors read are unpriced (`idx_us` has no disk term,
`q.rs:2544`; `plan2_prev` on research/disk-reads had the same omission); codes-and-vectors compaction copies into
anonymous memory that a cap cannot evict; `ERRS` is process-wide.

---

## Checked and found sound

### Merges
- **796a082 (plots-refresh)**: clean. Combined diff on `q.rs`, `run2.rs` and `main.rs` is empty. The only change
  against parent 1 is `Dial::fit` skipping the recall loop without targets, plus `dbknn`. `knn2_fast` and the
  per-level dial came in intact.
- **20e1355 (disk-reads + bench-capped)**: the hand resolutions in `scan_visible` and `run_planned` keep the reader
  structure and add `npow: ix.meta.npow` to every `ThrOut2` the disk side built. Against parent 2, the disk side's
  `dmax = fold(1f32, ..)` was correctly superseded by the base's `fold(0f32, ..)`, which is what both 546375a and
  ec7cf7d have. The disk side's stale `bail`/`rdec`/`alpha0`/old `Dial::fit`/old KnnOut2 lines were not
  reintroduced.
- **ce9cf94 (scan-planner)**:
  - `Full2::Real.rows: Arc<disk::Rows>` with `Rows::Mem(F16Rows)` keeps the alignment.
  - `full_row_f16` maps through `vrow`.
  - `own_rows()` guards the reader, `willneed`, `cached`, `calibrate_disk` and planner pricing consistently. I
    checked every use: `q.rs` 2088, 2206, 2242, 2280, 2332, 2441 and 2685.
  - `plan2`'s disk terms mirror `plan2_prev` from disk-reads: `row_us` adds `rb/io_gbps`; `cand_us` adds `disk_us`
    with the `d1-d0` span; `full_all` replaces `ncand_all*full_us` in the final scan check.
  - `decide_full` → `decide_full_from`: `exact_to_f64(exact_dot(q16, x))` equals `exact_value2` for `Full2::Real`
    (`q.rs:796-801`). The same f32 screen and error bound `full_err2` are used.
- No other conflicts: the `--cc` diffs of mod.rs and disk.rs show no lost hunks.

### Exact-path invariants the owner asked for
- `npow` is set on every `ThrOut2` (`q.rs` 2217, 2239, 2261, 2695, 2826) and every `KnnOut2` (`q.rs` 3278, 3293,
  3331; `ann.rs:184`).
- `select_knn` is byte-identical to 546375a. `seed2` only adds the empty-cluster filter that compaction needs.
- `tail::level_constant` is used in `build2` (`mod.rs:1520`). N comes from the row count (`mod.rs:1484-1488`).
- A compacted index keeps its parent's `meta`, so its `npow` is at least what its own rows need (conservative).
- The one-sided budget is unchanged. `knn2` is still a threshold query at the seed's score with interval refinement.

### Compaction (`mod.rs:989-1085`)
- The nibble copy matches `setnib`'s layout (BLK = 32, low/high nibble at lane 16).
- `esc` keys are global rows and are remapped by binary search.
- `ahat` is padded to match the build; `nukey` is subset in order.
- `vrow` composes when compacting a compacted index.
- `Cl2` bounds, `hi`/`lo`, `ctab`, `x2` and `repack` are valid for any subset. Empty clusters are handled by
  `plan2` (`nvis_cl == 0` → Skip), `seed2` and `knn2_fast`.
- `to_bitmap2` maps through the copied `row_ent`. `Vis2::new` on a compacted index panics by assertion, as intended.
- `to_file` needs exclusive access (`Arc::get_mut`, `run2.rs:205`). Today that holds because setup writes the file
  before any compaction (dbcmp and verify compact later), and a violation panics rather than silently diverging.
- Codes-and-vectors compaction of an on-file parent copies the rows through the mapping before the cap is set.

### Reader (`disk.rs`)
- O_DIRECT buffers are page-aligned (anonymous mmap, `tot` rounded) and the file is padded to whole pages.
- `PIECE` keeps extent lengths within u32. The `user_data` packing is sound.
- Batches are fully drained in the fold's `map` (`q.rs:2817-2824`, `2231-2236`).
- Under `want_iv`, `decide_batch` pushes exact scores, so knn2's candidates are complete.

### New defaults (c7737c9)
- The integer path is used only when `!want_iv`.
- The e2e test (`v2/e2e.rs:73-86`) checks the default (integer, gtab, repack) and the float path on every forced
  path, on full and compacted indexes and under visible sets.
- `calibrate` runs in memory before `--full-file` (`run2.rs:189` vs `205`); `calibrate_disk` runs only in dbcmp when
  priced.

### Not verified
- Every on-file and reader path: `e2e.rs` uses in-memory rows only. NEXT.md says dbcmp on synth4k compared io_uring,
  mapping and O_DIRECT answers, but I did not rerun it.
- I recommend a unit test that writes synth4k's rows to a temporary file, turns on `uring` with
  `direct_min = 0`, and repeats `threshold_answers_equal_brute_force_on_every_path` and the compacted test. That
  would cover F4's code path if it is ever changed.


---

# Appendix: deletions


Range: `546375a..31bdf70` (first-parent), repository `/home/user/evs`, branch `evs-merge`. Read-only review: nothing
was built or run. **V** means verified by reading the code at both revisions; **I** means inferred.

Baseline used for "before the deletions": `da91da6` (the last first-parent commit before 9657681, which retired the
old approximate mode). Where a script dates from before the merges, `546375a` was also checked.

## 0. The command line, then and now

`main.rs` at 31bdf70 (83 lines) parses loosely: `has(x)` and `get(x, default)` look for a token anywhere in argv, and
`run2::arg_str` / `arg_usize` do the same. **Nothing rejects an unknown flag**, so every removed flag is silently
ignored. Only plan flags (`--flags a,b`, `--b`, `--c`, dbcmp `index:FLAGS`) fail loudly: `plan_flags` panics with
"unknown plan flag" (run2.rs:136).

Flags and env vars read by the source at da91da6 and not at 31bdf70 (V, from a grep of every quoted `--flag`):

| Removed | Was used for | Status now |
|---|---|---|
| `--int`, `--gtab` | turn on the integer levels / combined tables | ignored; now the default, so harmless |
| `--u8`, `--jump`, `--alpha0`, `--colstore`, breakdown `--variants` | rejected switches, column store | ignored silently |
| `--approx GRID` (ann), `--approx-mode`, `--fit-dial`, `--soar`, `--dial-plan`, `--tight-dials`, `--skip-default` | old approximate mode | ignored silently |
| `--anchors`, `--nflat`, `--batchq`, `--dnq`; modes `bandwidth`, `batch`, `disk`, `checkderive`, and `levels/verify/bytesq/e2e/filter/--scale` without `--v2` | version 1 | without `--v2` main exits 2 with a message; the flags are ignored with `--v2` |
| `VEXACT_CAL_PER_GROUP` | encoder calibration sample | now `BuildOptions::cal_per_group`, no CLI or env: the knob is gone from the binary |
| `VEXACT_COLB/COLCK/COLEN/COLKEY/COLDBG` | column layout | gone with it |
| plan flags `col`, `colq`, `collazy`, `prev`, `bail=`, `nobail`, `rdec=`, `all` | | panic (loud) |

Modes still dispatched with `--v2`: verify, levels, setup, bytesq, e2e (with `--index-b/--b/--c` variants),
breakdown, filter, scanbench, oracle, ann-truth, ann, dbcmp, dbtopk, dbknn, `--scale`; `qvbench` without `--v2`.
All flags documented in rust/README.md still exist.

Plan files: `Plan::from_kv` reads the same 17 keys `to_kv` wrote at da91da6; every committed `*plan*.txt` uses only
those (V). `FIT_FORMAT` 2 and `INDEX_FORMAT` 3 unchanged, so existing fitted directories still open (V). CSV headers,
dump and oracle file names written by run2.rs/ann.rs are unchanged except for removed version-1/old-mode outputs
(`scale_{N}m.csv` from v1 scale, the `liminal_approx_old` rows); the build log lines moved from `[build2]`-prefixed
run2 output into `v2::open` with the same prefixes (V).

## 1. Findings, by severity

### F1. `bench/run.sh:24` silently drops the approximate rows (a run succeeds and measures less) — V

`$B ... --flat --approx "$APPROX" ann`. `--approx c:r:l,...` was ann's grid for `knn2_approx` at 546375a
(ann.rs:153 there). At 31bdf70 ann reads only `--dial np:eps` (ann.rs:152); `--methods` defaults to
`exact,flat,approx`, but with no `--dial` specs the approx method adds no configuration (ann.rs:187-193). The script
runs to completion, `score.py` scores what is there, and `ours.csv` has no approx rows. The `APPROX` default (line 13)
is now meaningless.
Also (I): the run now measures the integer path with combined tables and repack (the new `Plan::default`), not the
float path that stage 1 measured.
**Fix:** replace `--approx "$APPROX"` with `--dial` settings as in remeasure.sh (and rename the variable), or mark
run.sh as the stage-1 record that needs a pre-9657681 binary. Long term: make the binary reject unknown flags (F3).

### F2. Old-mode reproduction lines now run the new mode and append rows under the same labels — V

- `bench/topk/REPORT.md:194-195` ("exact and the old approximate mode (the committed rows)": `--fit-dial dbtopk ...
  --approx-mode old`) and `:201` (`--approx-mode old --fit-dial --no-exact --params results/liminal_topk.csv`).
- `bench/threshold-db/REPORT.md:588` ("the old mode: ... --fit-dial dbknn --approx-mode old ... [--skip-default all:2]").

All three flags are ignored now, so dbtopk and dbknn run `knn2_fast`. dbknn appends to `liminal_approx.csv` as
`liminal,tight,...` (run2.rs:1643, :1703), the same method/setting the committed old-mode rows carry, so new and old
rows become indistinguishable; the "default" setting the plots map (`threshold-db/plot.py` `("liminal","default")`)
is no longer produced. With `--params`, dbtopk evaluates the old mode's committed (nprobe, dial) settings with the new
mode and writes `liminal_approx_new` rows.
NEXT.md:85-87 says "Rows measured with the old mode can no longer be reproduced", but the reports' own reproduce
sections still present the commands as runnable.
**Severity:** a run silently measures something else. **Fix:** strike or annotate those lines ("old mode retired in
9657681; build da91da6 to rerun"), and have main reject `--approx-mode`/`--fit-dial`/`--soar` with that message.

### F3. The loose CLI turns every removal into a silent no-op — V

See section 0. `--u8`, `--jump`, `--alpha0`, `--colstore`, `--variants`, `--soar`, `--fit-dial`, `--approx`,
`--approx-mode`, `--skip-default`, `--tight-dials`, `--dial-plan`, `--anchors`, `--nflat`, `--batchq`, `--dnq` and
`VEXACT_CAL_PER_GROUP` are accepted and ignored. Anyone rerunning a recorded argv (84 recorded runs in `results/`
carry `--int`, 22 `--fit-dial`, 15 `--skip-default`, 2 `--approx-mode`) gets a run that looks fine.
**Fix:** a list of known flags in main.rs (plus `--index-b/--index-c/--b/--c`), exiting 2 on anything else, with a
specific message for the retired ones. `--int`/`--gtab` can stay accepted as no-ops, as NEXT.md:131 promises.

### F4. Flagless scripts and the scale test now measure a different configuration — I (code V)

`Plan::default()` now has `int_det`, `gtab`, `repack` on (q.rs:1878). Scripts that passed none of
`--int --gtab --flags repack` measured the float path before and the integer path now, with no change of output
labels:
- `bench/run.sh` and `bench/remeasure.sh` (the ANN-Benchmarks runs; `bench/REPORT.md` figures are float-path).
  remeasure.sh is resumable by marker files in `$DATA/<name>/results2/done/`: a partial rerun can mix a `plan.txt`
  calibrated on the float path (piece `calib`) with integer-path timings, and earlier ann/approx pieces with new ones.
- `scale::run2` (`--v2 --scale`) uses `Plan::default()` (scale.rs:186) and reads no `--flags`/`--plan`, so the
  synthetic scale test now runs the integer path, combined tables and repack on `Full2::Synth` rows, and there is no
  way to get the float path that earlier `scale2_*m.csv` files measured. The e2e test does not cover `Full2::Synth`
  (it uses synth4k, a real fitted index), so the integer path on synthetic rows is untested (I; I did not find a
  path that would panic on `Full2::Synth`: `decide_full`, `answer_share`, `exact_value2`, `exact_ge2` handle it, and
  the packed scan that calls `full_row_f16` needs `pscan`, off by default).
- `levels` (run2.rs:542) now writes `levels2_int.csv` in a flagless run instead of `levels2.csv` (no consumer found).
The owner decided this default; what is missing is the record: bench/REPORT.md and rust/README.md do not say that the
default changed or that `--flags noint,nogtab,norepack` gives the old path.
**Fix:** one line in rust/README.md (Run) and bench/REPORT.md; let scale::run2 apply `run2::plan_flags(Plan::default(),
&arg_str("--flags",""))`; write the plan flags into the scale CSV or log.

### F5. Documentation that now misdescribes the code — V

- `SYSTEM.md:349` "the `bail` option, both off by default" (bail removed in 5dfe9e7); `SYSTEM.md:358-359` "Built and
  off ... the columnar store" (col.rs deleted in a150fc5). SYSTEM.md was not touched in the range.
- `NEXT.md:75` "SYSTEM.md is current" (see above). `NEXT.md:79` cites `kernel.rs:599, :671` (now 108 and 172);
  `NEXT.md:81` says `dot_i8` has no tail: only its AVX2 copy remains and nothing calls it (F6).
- `bench/REPORT.md:148`: "with `--soar` / `--fit-dial` the second assignment and the dial fit" (both gone); the report
  never says its approximate rows (knn2_dial + SOAR) cannot be rerun, while remeasure.sh now measures knn2_fast under
  the same `approx` label and `nprobe=.. eps=..` params (soar suffix dropped).
- `bench/topk/REPORT.md:25`: "`knn2_dial`, the mode `dbknn` uses in the threshold-db work"; dbknn uses knn2_fast now.
- `rust/src/kernel.rs:1`: "the f16 and int8 dot products of full-vector scoring (AVX-512, or AVX2 without it)": the
  int8 product exists only for AVX2 and is not used by scoring.
- `rust/src/run2.rs:768` breakdown2 still emits a `column order` column (phase P_SEL removed); it is always 0.
  `variant` is always `levels` now that `--variants` is gone.
- `rust/src/v2/q.rs:1845`: "`Plan::default` holds constants for 1M BioCLIP (n = 768, AVX-512)" (already flagged
  stale by CODE_REVIEW.md:927; still there).
- rust/README.md: correct for every mode and flag it names; it does not mention `--flags` at all, nor the new default.

### F6. AVX-512 kernels: intact except `dot_i8`, whose AVX-512 twin went and AVX2 twin stayed — V

Every `#[cfg(target_feature = "avx512f")]` block in `v2/q.rs` (qv_exact, dot64v, matvec64, dot32), `v2/kern.rs`
(rep16, plan_gather, gather_rows, lane_sum, part_sums, Gather::new) and `simd.rs` is byte-identical between da91da6 and
31bdf70. In `kernel.rs`, `dot_f16` and `dot_f16_x4` keep both forms. Removed: the v1-only `run_block` and its helpers
and `basis_matvec` (version 1, by decision), and the AVX-512 `dot_i8`, while `#[cfg(not(target_feature = "avx512f"))]
dot_i8` (kernel.rs:198-215) stays. Nothing calls either now (its only caller was v1 `bandwidth`), so the crate still
builds both ways (I), but the "keep the AVX-512 twins" ruling (CODE_REVIEW.md:979 lists `dot_i8` among them) is applied
unevenly and an AVX-512 build has no `dot_i8` while an AVX2 build does. `build_lut` (kern.rs) lost its 8-bit branch;
that function is not AVX-512-gated. I could not compile the AVX-512 configuration (no build allowed).
**Fix:** delete the AVX2 `dot_i8` too (dead), or restore the AVX-512 twin; fix kernel.rs:1 and NEXT.md:81.

### F7. Synthetic scale path: survives — V

`Full2::Synth`, `q::Synth2`, `SynthSrc`, `scale::run2` and the `--v2 --scale N --nq M` dispatch are present; the
`Full2::Synth` arms in `exact_value2`, `exact_ge2`, `decide_full`, `answer_share`, `x_f64`, `full_row_f16` (panics,
as before) and the scale verification loops are kept. scale::run2 builds with `BuildOptions::default()`; at da91da6 it
also ignored `--nguar`, `--rlv`, `--csg-plain`, `--design-exact` (setup2_at was never called on that path), so no
option was lost there. Only `--u8` went (by decision). The configuration change is F4.

### F8. Functionality removed beyond the stated list — V (small)

- `VEXACT_CAL_PER_GROUP` (calibration clusters per group) can no longer be set from the binary: it became a
  `BuildOptions` field that setup2_at leaves at 8. No script used it. Fix: `--cal-per-group N` if wanted.
- `--nguar` <= 0 used to exit 2 with an explanation; now `v2::open` asserts (a panic with the message; with
  `panic = "abort"` that is an abort). Same outcome, harsher.
- Version-1 `batch` (batched queries), `bandwidth` and v1 `disk` have no v2 counterpart; they are part of the deleted
  version 1, so within the decision, but `batch` was the only batched-query measurement mode.
- No tests were removed (test names at 48720a2 are a subset of those at 31bdf70).

## 2. Scripts under results/ that can no longer be rerun as written (historical record)

- `bench/threshold-db/results/planner/run.sh` with `WHATS` containing `index:prev` (14 recorded runs): panics on the
  plan flag `prev`. Loud; acknowledged in NEXT.md:118-119.
- breakdown2 `--variants` runs: the flag is ignored, so the jump variants are silently not measured (NEXT.md:119 says
  "cannot be repeated"; it in fact runs and reports one variant).
- `bench/threshold-db/results/approx/out_b.txt`, `out_c.txt` and the bench/topk old-mode rows: argv with
  `--fit-dial --skip-default` / `--approx-mode old`; rerunning measures knn2_fast (F2).
- `bench/results` stage-1b approx rows with `soar` (remeasure.sh before 9657681): mode gone.
- `rust/results/{detlevels,clustersize,scaling}/*.sh`: use `--dlv`, plan flag `ddec=`, `e2e --knn-k`, `VEXACT_NDL`
  and branch binaries (`vexact_ndl2`, `vexact_b2`) from `research/levels-clusters`; none of these exist on this branch,
  before or after the range. The reports say so (REPORT_SCALING.md:3, REPORT_DETLEVELS.md:6). When that branch is
  rebased, its `ddec`/`--dlv` code must be re-applied over the deleted v1 code and the new BuildOptions.
- `reports/annideas/*.out`: `--dump-np`, `--stat-np`, `--dump-dir`, `--split` from a scratch binary never committed.
- `dualproto/tools/run_e2e.sh` and dualproto/REPORT.md:59-66: `--data`, `--res`, `--nflat`, `VEXACT_CLIPSTATS` belong to
  the dualproto prototype's own build; not this CLI at 546375a either (not caused by the range).
- Scripts passing `--int --gtab --flags repack` (rust/results/{bounds,const,rand,setup}, threshold-db capped/planner):
  still mean the same thing.

## 3. Pre-existing, not caused by the range

- `rust/tools/breakdown.py:7` reads a column `bounds+decisions+compaction`; breakdown2 writes `bounds+decisions` and
  `compaction` separately (run2.rs:768, the same at 546375a), so the script raises KeyError on any breakdown CSV.
  Fix: `g('level decode','bounds+decisions','compaction')`.

## 4. Checked and fine

`bench/remeasure.sh` (updated in 9657681: `--dial $G --methods approx`), `bench/threshold-db/capped/run_ours.sh`
(`--full-file`, `--cap`, `--cmp-order`, `--full-advice`, `--willneed` exist; `--int --gtab` no-ops), the reproduce
blocks of threshold-db REPORT_CAPPED.md/REPORT_DISK.md (`--what`, `--sets`, `--cold`, `--read-modes`), rust/README.md's
commands (`verify levels setup e2e filter`, `bytesq`, `dbcmp --thr`, `dbtopk --order`, `--scale`, `--clusters 40
verify`), `bench/score.py`, `oracle.py`, `oracle_thr.py`, `report.py`, `threshold-db/plot*.py`, `topk/plot.py`,
`rust/tools/cmp_e2e.py`, `fit.py` (only gained `--per-cluster`), the binary's name (`vexact`) after the lib/bin split.


---

# Appendix: harness


Lens: does `run2::dbcmp` (and dbtopk/dbknn/ann) still measure what REPORT_CAPPED, REPORT_DISK and REPORT_PLANNER say, and does `plan2` price what execution does.
Method: I read the merged `dbcmp` (rust/src/run2.rs:1100-1388) against `git show 747d371:rust/src/run2.rs` (disk-reads) and `git show b79dbc8:rust/src/run2.rs` (scan-planner), the reports, and the scripts that read the CSVs. I also read `plan2`, `run_planned`, `scan_visible`, `calibrate*` and `knn2` in rust/src/v2/q.rs and rust/src/v2/disk.rs. Nothing was built or run.
**Verified** means I read it in the code. **Inferred** means it is a consequence I did not measure.

Findings are in priority order inside each section. "Known" marks issues NEXT.md already records (NEXT.md:33-42).

---

## A. Harness (dbcmp)

### A1. `--cmp-order variant` does not restore the order REPORT_CAPPED ran in when `--cold` is given. Verified.
- **Code.** run2.rs:1302-1320 runs the cold block for **every** variant (`for wi in 0..whats.len()`) before any warm pass. The warm passes (run2.rs:1324) then run with no eviction between variants.
- **Disk branch (747d371).** For each variant it ran that variant's cold runs, which evict before each share, and then that variant's warm pass. So each variant's warm pass started from a cache holding only its own query-0 rows.
- **Now.** Variant 1's warm pass starts from the last variant's cold-run cache. Variant 2's warm pass starts from the full cache variant 1's warm pass left behind.
- **Who it affects.** Single-variant runs are unchanged. REPORT_CAPPED.md:199 (`--what flat,forced --cold`, vis_1) is affected: `forced` now starts warm on the rows `flat` just read, where it used to start almost empty. NEXT.md:41 says run_ours.sh's `--cmp-order variant` "keeps REPORT_CAPPED's order"; that is true only without `--cold`.
- **Consequence.** The `forced` warm rows of a rerun are not strictly comparable: early queries come out lower. The effect on the median is probably small, because the 1% working set fits in the cap after a few queries (inferred).
- **Fix.** When `by_variant`, run the cold block inside the pass loop, just before that variant's warm pass.

### A2. In query order, the per-pass statistics are attributed to every variant. Verified.
- The `[disk]` line (run2.rs:1361) prints the pass totals once, with the labels joined by commas.
- The per-variant summary line (run2.rs:1365) prints "read X MiB from storage in Ys" from `bio0`/`bt0` taken at the start of the pass. Every variant in an interleaved pass therefore reports the same MiB and wall time.
- The CSV rows' `disk_kb`, `majflt` and `sys_ms` are correct per variant (measured around each variant's reps).
- **Consequence.** Cosmetic, unless someone quotes MiB per variant from the log of a default-order run. No script parses these lines: I grepped bench/ and rust/results/ for `[disk]`, `timings, median` and `from storage`.
- **Also.** In query order on file, each variant runs right after the others on the same (query, share), so buffered reads made by an earlier variant serve the later ones. Only `--cmp-order variant` gives independent warm numbers under a cap. query is the default, so any capped run not launched through run_ours.sh gets the biased order.
- **Fix.** Print per-variant storage/wall only in variant order; in query order print the pass totals once. Or warn when `--full-file` is set and the order is query.

### A3. What a capped run through run_ours.sh now does differently from REPORT_CAPPED's runs. Verified unless marked.

1. **Read path.**
   - REPORT_CAPPED ran before io_uring (mmap + `--willneed`; REPORT_CAPPED.md:186).
   - `--full-read` now defaults to `uring` (run2.rs:213), `priced` is set (run2.rs:217), and `calibrate_disk` re-times io_us/io_gbps under the cap (run2.rs:1196-1199).
   - This dates from the disk-reads merge and REPORT_DISK records it. It is listed here because a rerun of REPORT_CAPPED's commands no longer reproduces REPORT_CAPPED.
2. **Planner.**
   - The merge brings the per-cluster planner: `Pre::Direct`, scan-first exits, the unfiltered per-cluster scan priced with `cscan_us`, and a disk term on every scanned or directly scored row.
   - run_ours.sh's `--plan-file` (bench/threshold-db/results/ours/plan.txt) has no `cscan_us`, `anc_us`, `dir_us` or `item_us2`. Those therefore come from `Plan::default()` (q.rs:1878: 0.10, 150, 1.0, 0.0), i.e. the BioCLIP defaults from another machine. Known in part (NEXT.md:39).
3. **Plan file timed by the old calibrate.**
   - ours/plan.txt (and planner/plan_r4.txt) were fitted with `want_iv: true` in calibration. That turns the integer levels off (q.rs:2785 gates them on `!o.want_iv`).
   - The scaled index-path constants (item_us0/1, cl_us0/1, grp_us) therefore price the float-level path. Queries run the integer path, which is now on by default (c7737c9).
   - **Consequence (inferred).** The index path is overpriced relative to full vectors and scans in every run that uses these files: the capped runs, the planner's run.sh, and remeasure.sh when its `done/calib` marker survives.
   - **Fix.** Re-time plan.txt with the new calibrate, and say in each report which calibrate fitted its plan.
4. **`--int --gtab` are now ignored.** No code reads them; they restate the defaults. Harmless for run_ours.sh and run.sh, which pass them. Any report command that **omitted** them now runs integer levels + combined tables + repack.
5. **Order.** See A1.
6. **Memory before the cap.**
   - Every session's compacted indexes are built before the cap is set (run2.rs:1155-1182). The planner branch built them per set, and without any cap.
   - Under a cap they all count as anonymous memory at once. cv at 50% is 913 MB (NEXT.md:59), more than the whole 740 MiB cap.
   - This matters only when c/cv variants are requested. Known in part (NEXT.md:35).

### A4. CSV and dump comparability. Verified.
- **Columns.** The CSV header is identical to the disk branch's 13 columns and a superset of the planner branch's 8. Every reader uses `csv.DictReader` (summarise_capped.py, plot_capped.py, plot*.py, summarise.py, results/planner/table.py), so the extra columns are fine.
- **Cold rows.** results/planner/table.py does not filter `kind`, so a planner-style run with `--cold` would mix cold rows into its medians. summarise_capped.py and plot_capped.py do filter.
- **`full_reads` for in-memory `flat`.** It is the answer count, not reads (run2.rs `"flat"` arm, `n = e.len()`). This is unchanged from both branches, but the column means different things for different variants.
- **Dump names.** Now `label.replace([':','@','='], "_")` (run2.rs:1322).
  - Disk branch: same.
  - Planner branch: raw `{what}`. So `index:noint_all.bin` became `index_noint_all.bin`.
  - bench/threshold-db/check.py parses `<what>_<vis>` and chooses the rounding tolerance by prefix (`index`/`flat` → f16). Labels `idx`, `scan`, `cindex`, `cvindex`, `cscan`, `cvscan` and `forced` fall through to the f32 tolerance (2^-24), which overstates "beyond_rounding".
  - Dump files are also written for `prep`/`anc`, whose answers are empty, so check.py would count every expected item as differing.
  - **Fix.** Skip dumps for non-answering variants, and give check.py a list of f16 variant prefixes.
- **`--what` and `--cmp-what`.** Either is accepted (`--cmp-what` wins), and `forced` is kept as an alias of `idx`, so the reports' commands still parse.

### A5. Read modes and disk constants. Verified; inherited from the disk branch.
- `calibrate_disk` runs once, before the mode loop, with the setup's qd, direct_min and dontcache_min (run2.rs:1196-1199).
- Modes that change `:qd=`, `:direct=` or `:dontcache=` keep io_us/io_gbps timed under the default policy.
- `:plan=disk` with `mmap` prices page-fault reads with constants timed through io_uring (by design, but the column label does not show it).
- **Fix.** Re-run `calibrate_disk` per mode after the policy is set.

---

## B. Cost model (`plan2`, `run_planned`, `scan_visible`, `calibrate`)

Execution read paths for rows on file, all verified in the code:

| Path | How it reads | Where |
|---|---|---|
| Open cluster, full-vector candidates | io_uring `Reader` (gap = `io_gap`), unless a sample of 8 rows is resident | q.rs:2685, 2817-2828 |
| Open cluster, index-path survivors (`pend` from `run_cluster`) | the same reader | same lines |
| `Pre::Direct` | `decide_full`: mapping faults, plus `madvise(WILLNEED)` only if `--willneed` | q.rs:2743-2755, 2277-2283 |
| `Planned::Scan` without a visible set, uring on, `pscan` off | `Reader` per 1,024-row chunk, decisions overlapped with reads | q.rs:2206-2235 |
| `Planned::Scan` under a visible set | mapping faults + `willneed` | q.rs:2236-2257 |
| Codes-only compaction (`vrow`) | parent's mapping; `own_rows()` is None, so no reader, no willneed, MADV_RANDOM 4 KiB faults | mod.rs:964-968, 974-980 |
| cv compaction | memory | — |
| k-NN scans, seed, `select_knn` re-scoring | mapping only | q.rs:3288-3293, 3306 |
| `answer_share` sample (256 rows) | mapping | q.rs:2410-2428 |

### B1. The index path's survivor reads are never priced, while the full path's are. Verified; inherited; not in NEXT.md's list.
- `cand_us` adds `disk_us(nc, span, rb)` (q.rs:2548).
- `idx_us` = `index_us_p`, which has no disk term (q.rs:1938-1944).
- Execution reads survivors through the same reader as full-path candidates (q.rs:2817-2828).
- **Consequence (inferred).**
  - With `priced` on, the per-cluster gain `full - idx` (q.rs:2581) and the group gate overstate the index path by the survivors' read cost.
  - The final full-vs-scan check (q.rs:2645) compares a disk-priced `full_all` with a scan that also pays disk, but the kept "gains" are inflated.
  - At large answers, where survivors are about d·nc, the planner keeps the index path when full vectors or a scan would cost no more. The REPORT_DISK figures at 30-75% answers are where this would show.
- **Fix.** Add `disk_us(n_surv, span, rb)` to the index path, with `n_surv ≈ p_ans·nc` (or a fitted survivor share).
- **Measurement.** dbcmp on file with `index`, `idx` and `index:...` variants, comparing `full_reads` and `disk_kb` per share against the planner's choice.

### B2. Integer levels: calibration now matches threshold queries but no longer matches k-NN. Verified.
- 0f93ea3 times the index path and the full path with `want_iv: false` (q.rs:2012), so the integer levels are on (q.rs:2785).
- `knn2` plans and runs with `want_iv: true` (q.rs:3304). That turns the integer levels off and pushes intervals and exact values.
- The factor `s` (item_us0/1, cl_us0/1, grp_us) and `full_us` now price k-NN's sweep with the faster threshold constants.
- **Consequence (inferred).** The k-NN planner underprices its index path. dbtopk's `liminal_exact` and ann's `exact` rows can shift after recalibration, independently of any code change in k-NN.
- **Fix.** Fit a second factor with `want_iv: true` (`Plan` gets `item_scale_iv`), or time both and use the one matching `o.want_iv`.

### B3. Direct scoring and filtered scans are priced as sequential reads but read by page faults. Verified; known (NEXT.md:37-39).
- `row_us` = scan cost + `rb/io_gbps` (q.rs:2449). That is about 0.47 µs per 1,536-byte row at 3.3 GB/s.
- Execution:
  - `Pre::Direct` faults through the mapping (q.rs:2743-2755).
  - The filtered `Planned::Scan` faults through the mapping (q.rs:2206-2208, `vis.is_none()` required for the reader).
  - Under MADV_RANDOM and without `--willneed`, each 4 KiB page is a synchronous fault (about 160 µs cold on the REPORT_CAPPED disk).
  - Even with willneed, scattered visible rows do not coalesce.
- `dir_us` carries no per-cluster read latency.
- **Consequence (inferred).** Under a filter the planner prefers Direct and scans that cost far more than priced when cold. It also reads them through the page cache instead of O_DIRECT, which changes what the cap holds.
- **Fix.** Route `Pre::Direct` and the filtered scan through the `Reader` when `uring()` is on, as `run_planned` does for candidates. Otherwise price them with `disk_us(nv, span_of_cluster, rb)`.

### B4. Pricing ignores what is cached. Verified mechanism; consequence inferred.
- `disk` is a property of the policy (q.rs:2441), so every row is charged a read.
- Execution skips the reader when a sample of 8 rows is resident (`full_cached`, q.rs:2820; `cached`, q.rs:2222).
- **Consequence.** In the "warm within the cap" medians that REPORT_CAPPED and REPORT_DISK report, the planner prices reads that do not happen and over-favours the index path. NEXT.md:52 ("cached scan priced at 0.10 vs 0.06-0.08") shows the same cache-blind direction in memory.
- **Fix.** Scale the disk terms by an estimate of non-residency (e.g. `mincore` on the cluster's span).
- **Measurement.** dbcmp variants `index` vs `index` under `:plan=mem` at warm shares.

### B5. The unfiltered on-file scan is priced as CPU plus reads, but runs them overlapped. Verified mechanism; inferred size.
- `scan_cost = nvis·(cscan_us + rb/io_gbps)` (q.rs:2449-2450).
- `scan_visible` keeps reads in flight while it decides earlier chunks (q.rs:2209-2234), so the real cost is about max(CPU, IO), not the sum.
- **Consequence.** The scan is overpriced by about `cscan_us` per row (0.10 against 0.47 µs IO), roughly 20%. This biases toward the index path at the largest unfiltered shares.

### B6. Codes-only compaction (`cindex`, `cscan`) reads unpriced, unadvised page faults. Verified; known in part (NEXT.md:35, 39).
- `own_rows()` is None when `vrow` is set (mod.rs:966). So: no reader, no `willneed`, no pricing. `full_cached` returns true, so `decide_full` reads through `full_row_f16` → the parent's mapping (mod.rs:974-980).
- **Consequence.** In a capped dbcmp, the `cindex` and `index` rows use different read paths (4 KiB sync faults against O_DIRECT io_uring) and are not comparable as an index-vs-compaction comparison. The planner prices `cindex` as in memory.

### B7. `answer_share` reads 256 scattered rows per query, unpriced, when `item_us2 > 0`. Verified mechanism.
- `p_ans` is computed on every Auto query when `item_us2 > 0` (q.rs:2543). It is also computed in `in_band` when a scan band is set (q.rs:2534-2540).
- It reads 256 rows through the mapping (q.rs:2421-2427).
- A calibrated plan has `item_us2 > 0`: results/planner/runs/plan.txt has 0.119. ours/plan.txt has none, so run_ours.sh is not affected today.
- **Consequence (inferred).** In a calibrated run under a cap, each query takes up to 256 cold random faults (tens of ms at about 160 µs each), outside any priced term.
- **Fix.** Sample from the codes (level-0 scores) or from rows already resident.

### B8. Calibration still times some paths differently from the queries that use them. Verified; inherited.
- **`scan_us` / `cscan_us`** are timed with `scan_knn` (the top-k heap kernel, q.rs:1986). They price threshold scans, which run `scan_visible` / `decide_full_from` (the x4 threshold kernel). That fits NEXT.md:52 (priced 0.10, actual 0.06-0.08 µs). For k-NN `scan_first` the kernel does match.
  - **Fix.** Time `scan_visible` for threshold pricing and keep `scan_knn` for `scan_first`.
- **`scan_band`** is called with `base`, the pre-calibration constants (q.rs:2061), not the plan being fitted. With `scanband=auto` the band comes from a planner running `Plan::default()` constants.
  - **Fix.** Compute the band after building the calibrated `Plan` and pass that.
- **`seed_us`** is timed unfiltered at k=10 (seed2 with `None`). It is used as `scan_first`'s fixed cost under a visible set and for any k (q.rs:3296). It has no disk term, although on file the seed's first full vectors are mapping faults.
- **`dir_us`** is fitted only under the random tenth. Unfiltered Direct uses `cscan_us` for its rows, with that `dir_us`.

### B9. Paths that still price with the old defaults. Verified.
- `--plan-default` (run2.rs:185), scale.rs:186 and e2e.rs use `Plan::default()`. Its constants (q.rs:1878) were not changed when c7737c9 turned on `int_det`, `gtab` and `repack`. They are the BioCLIP float-path values, and now price the integer path.
- Plan files fitted by the old calibrate: see A3.3.
- The k-NN planner: see B2.

### B10. Disk terms that hold. Verified.
- `disk` is off for compacted and in-memory indexes (q.rs:2441).
- `disk_us` uses the same `io_us·io_gbps` gap rule as the reader's merging (`io_gap`, q.rs:1919-1930; reader gap q.rs:2685).
- The unfiltered scan's reads go through the reader with gap 0 over contiguous chunks, which matches the sequential `rb/io_gbps` term.
- `span_d` (d·cluster rows) is a reasonable proxy for unsampled clusters. The final pass uses the exact `d1-d0` span.

---

## C. dbtopk, dbknn, ann.rs after the old approximate mode was removed (9657681)

### C1. The reproduce commands still pass removed flags, and those flags are silently ignored. Verified.
- main.rs ignores unknown arguments.
- bench/topk/REPORT.md:195 and :201 (`--fit-dial`, `--approx-mode old`) and bench/threshold-db/REPORT.md:588 (`--approx-mode old`, `--skip-default`) now run the **new** mode.
- The new mode writes under the same method names (`liminal_approx`; `liminal,tight` in liminal_approx.csv). It appends to the committed CSVs, which are opened in append mode.
- `--params results/liminal_topk.csv` holds `np:fit0.99` rows, so it panics in `dial_of` ("no dial fit0.99").
- bench/run.sh:24 passes `--approx` (knn2_approx's grid), which is now ignored: no approximate rows are produced and nothing reports it.
- **Fix.** Reject unknown flags (or at least the retired ones). Mark the old-mode reproduce lines as unreproducible (NEXT.md:86 says so; the reports do not).

### C2. ann.rs `approx` rows cannot be told apart from old ones. Verified.
- The param string is `nprobe={np} eps={e}`; for eps 0 and 0.01 it prints the same text as the old `--dial np:d` rows.
- The method name is still `approx` and ours.csv is appended.
- What changed: the algorithm (knn2_fast instead of knn2_dial) and the dial (fitted for this k instead of the setup's k=10 dial). `--dial-plan` is gone.
- remeasure.sh writes to results2 and gates the approx piece on its own marker, so a clean run is fine. A results2 directory that already holds older pieces mixes them.
- **Fix.** Name the method `approx_fast` or add the k of the fit to the param string.

### C3. dbtopk and dbknn timing is otherwise unchanged. Verified.
- Ladders, reps, recall and the CSV columns are unchanged.
- The stats method is always `liminal_approx_new`.
- `Dial::fit(..., recall_too, k, &approx)` replaces the targets slice. dbknn passes `false`, as its old `&[]` did.
- They run with `c.plan`, which now has the integer levels on by default. Older runs passed `--int` anyway.

---

## Suggested measurements that would settle the inferred items

1. **B1, B4, B5.** One capped run_ours.sh at shares 3-6 with `--what index,idx,index:nodirect --cmp-order variant` and a single read mode, then the same with `:plan=mem`. Compare `full_reads`, `disk_kb`, the scanned-run counts and the medians per share.
2. **B3.** Filtered vis_10 under the cap, `index` against `index:nodirect`, with and without `--willneed`. Use `majflt` and `sys_ms` per row.
3. **B2.** dbtopk exact k=10/100 with the plan from the new calibrate against one fitted with `want_iv: true` (`git show 0f93ea3^`), same binary otherwise.
4. **A1.** Rerun REPORT_CAPPED.md:199 once with the current code and once with the cold block moved into the pass, and compare the `forced` warm medians.
5. **B8.** scanbench or dbcmp `scan` per-row times against `scan_us`/`cscan_us` from the new calibrate.
