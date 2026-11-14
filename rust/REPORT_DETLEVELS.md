# Fewer deterministic decisions, and fewer deterministic levels

Saved by the coordinating session from the agent's returned text. Every number is the agent's; the headline timings
and the check counts were compared with the raw CSVs and logs.

Code: branch `research/levels-clusters`, on `research/combined` at d443f66. Measured on a 4-core Intel Xeon with AVX-512 (VM): one thread
pinned to core 2, the process on cores 1–3, nothing else running. Fixed plan constants
(`results/bounds/plan_fixed.txt`), s3 configuration (`--int --gtab --flags repack`). The 1M BioCLIP-2 data-only dual
index (999,000 rows) was fitted here with `tools/fit.py --basis dual`. Scripts, CSVs and full tables are in
`results/detlevels/`.

## Summary

Deciding at fewer of the six deterministic levels (family B) is 2–7% faster for text queries and within noise for most
item cells. A level that does not decide still builds its table, reads its codes and re-packs its survivors.

Having fewer deterministic levels (family C) is faster:

- **Text:** 14–16% at 0.01–1%, 6–8% at 10–50%, 4–7% at 75%.
- **Item:** 4–10% at 10–75%.
- **Bytes:** 13–22% more at text 0.01–0.1%, 3% more at text 1%, about 1% fewer at large answers. The full-vector
  share does not change.

Every timed variant passed the 50k checks. The 1M checks passed for C 8/32 and B {32}.

## The reading of `enter_frac_by_level`

`run_cluster_from` adds the live slots of each block at the top of level l to `enter[l]`, and the survivors it hands
to full vectors to `enter[nl]`. `e2e2` divides each by queries × N. That gives eleven entries: entering 4, 8, 12, 16,
24, 32, 56, 96, 160 and 256 B, then full vectors from the level walk. The shares decided per level that follow from
it are right (text 1%: 15 / 8 / 8 / 8 / 14 / 14%, then 41% at 56 B).

Two qualifications:

- Only clusters on the index path count. `full_reads_per_item` is larger than the last entry (item 1%: 0.0073 against
  0.0017) because it also counts clusters the planner sends straight to full vectors.
- Together, the six levels decide 51% of their entrants at text 1% (49% of N), and 2.6% at text 50%.

## What was changed

**Plan flag `ddec=L.L…|all|none`** (`src/run2.rs`, `src/v2/q.rs`). Deterministic level l decides only when its bit is
set; the default is all.

- A level that does not decide still advances the integer accumulators.
- It computes no bound, takes no decision and compacts nothing; every slot is kept, so the all-kept fast path applies.
- Verify mode still computes and checks the bound, then does not use it, as `rdec` does.

**`NDL` from `VEXACT_NDL` at build time (default 6), plus `--dlv B,B,…`.** `--dlv` replaces the index's DLV when it
opens; a length mismatch is refused with an error naming both fixes.

- The allocation, tables, integer thresholds and codes are derived from DLV when the index opens, so no refit is
  needed.
- `NDL` is used generically in the code, so this was a modest change. It is not switchable at run time.

**Soundness.**

- A deterministic interval depends only on the accumulators through its own level: per-level B and e, and per-item
  thresholds set once per cluster with the largest dial over all levels. Whether an earlier level decided does not
  enter it. The existing `first_decide` skip relies on the same property.
- A skipped decision leaves the item live, and a later level judges it on its own bound.
- The randomised union bound covers randomised levels only, so a different deterministic schedule changes no constant.
  Every build logs the same constants: 7.413, 8.933 and 9.086.

## Method

Each figure is the median over 3 runs of the median over 20 queries. Each process rebuilds the index (about 3 min) and
times up to three variants interleaved cell by cell. Six processes were run, then run again in reverse order:

| Process | Binary | Variants a / b / c |
|---|---|---|
| base | unmodified d443f66 | baseline |
| b1 | patched | `ddec=all` / `{4,24,32}` / `{4,32}` |
| b2 | patched | `ddec=all` / `{16,32}` / `{32}` |
| c3 | NDL 3, `--dlv 4,16,32` | all / decide {4,32} / decide {32} |
| c2a | NDL 2, `--dlv 8,32` | all / decide {32} |
| c2b | NDL 2, `--dlv 16,32` | all / no deterministic decisions |

**The baseline** is the mean of six measurements: the unmodified binary twice, and `ddec=all` (identical logic) in b1
and b2 twice each. Their spread, (max − min) / mean, is the noise floor:

- text: 3–11%;
- item at 10–75%: 5–7%;
- item at 0.01–1%: 26–54%, so changes below about 20% there are noise.

**Against `REPORT_BOUNDS.md`'s 5900X figures.** This machine is 1.6–1.9x slower: item 1% 13.5 against 7.2 ms, item 50%
113 against 71, text 1% 64 against 38, text 50% 124 against 76. Bytes agree within the difference of the 1M samples
(text 1% 67.9 against 69.9 B; full share at item 50% 9.1% against 9.6%).

## Time at 1M

Each cell is the change against the baseline pool. For family B, the bracket is the change against the `ddec=all`
variant in the same process. The baseline row is ms, with its spread.

**Item queries**

| Variant | 0.01% | 0.1% | 1% | 10% | 50% | 75% |
|---|---|---|---|---|---|---|
| baseline (spread) | 2.7 (49%) | 4.1 (54%) | 13.5 (26%) | 81.4 (6%) | 113.4 (5%) | 106.5 (7%) |
| B {4,24,32} | −10% (−4) | +1% (−6) | −2% (−4) | −2% (−3) | −2% (−2) | +1% (+2) |
| B {4,32} | −13% (−8) | +4% (−3) | +7% (+4) | −3% (−4) | −1% (−1) | −2% (−1) |
| B {16,32} | −21% (−14) | +1% (+25) | −7% (−2) | −4% (−2) | −3% (−1) | −3% (−2) |
| B {32} | −20% (−13) | −15% (+6) | −10% (−4) | −5% (−2) | −2% (−1) | −4% (−3) |
| C 4/16/32 | −11% | −14% | −9% | −4% | −4% | +1% |
| C 4/16/32, decide {4,32} | −20% | −15% | −10% | −7% | −6% | −4% |
| C 4/16/32, decide {32} | −15% | −17% | −1% | −9% | −2% | −2% |
| C 8/32 | −11% | −1% | −6% | −7% | −4% | −8% |
| C 8/32, decide {32} | −17% | −8% | −5% | −10% | −1% | −7% |
| C 16/32 | −3% | −19% | −7% | −6% | −7% | −10% |
| C 16/32, no deterministic decisions | −8% | −16% | −7% | −6% | −8% | −9% |

**Text queries**

| Variant | 0.01% | 0.1% | 1% | 10% | 50% | 75% |
|---|---|---|---|---|---|---|
| baseline (spread) | 31.1 (3%) | 38.1 (6%) | 63.9 (10%) | 94.1 (7%) | 123.5 (11%) | 113.9 (5%) |
| B {4,24,32} | +2% (+1) | −4% (−5) | −3% (−3) | −1% (−1) | −2% (−2) | +1% (+1) |
| B {4,32} | +1% (−0) | −1% (−2) | −5% (−5) | −2% (−2) | −4% (−3) | +2% (+3) |
| B {16,32} | −5% (−4) | −8% (−7) | −5% (−4) | −2% (−4) | −5% (−3) | −0% (+1) |
| B {32} | −7% (−6) | −7% (−7) | −7% (−6) | −4% (−6) | −3% (−1) | −3% (−2) |
| C 4/16/32 | −10% | −12% | −7% | −3% | −8% | −3% |
| C 4/16/32, decide {4,32} | −11% | −10% | −11% | −6% | −6% | −5% |
| C 4/16/32, decide {32} | −16% | −14% | −10% | −7% | −4% | −3% |
| C 8/32 | −16% | −15% | −14% | −7% | −6% | −4% |
| C 8/32, decide {32} | −17% | −15% | −14% | −9% | −4% | −3% |
| C 16/32 | −15% | −16% | −15% | −8% | −7% | −7% |
| C 16/32, no deterministic decisions | −5% | −10% | −13% | −10% | −11% | −7% |

The repeats of the C variants agree within about 5% for text, with single cells up to 8% (`results/detlevels/tables.md`).

## Bytes, full-vector share and entry shares

Bytes per item against the baseline:

| Variant | item 0.01% | item 1% | item 50% | text 0.01% | text 1% | text 50% |
|---|---|---|---|---|---|---|
| baseline | 3.3 B | 17.5 B | 292.5 B | 19.1 B | 67.9 B | 348.3 B |
| B {4,24,32} | +4% | +5% | +0% | +15% | +3% | +0% |
| B {4,32} | +7% | +10% | +0% | +27% | +7% | +0% |
| B {16,32} | +6% | +8% | +0% | +27% | +5% | +0% |
| B {32} | +15% | +20% | +0% | +69% | +13% | +0% |
| C 4/16/32 | +1% | +2% | −0% | +6% | +1% | −0% |
| C 8/32 | +4% | +5% | −1% | +17% | +3% | −1% |
| C 16/32 | +4% | +6% | −1% | +22% | +3% | −1% |
| C 16/32, no deterministic decisions | +34% | +45% | +0% | +149% | +32% | −0% |

The full-vector share is identical for every variant: item 0.14 / 0.61 / 9.08% and text 0.43 / 0.73 / 11.57% at
0.01 / 1 / 50%.

Entry shares at text 1% (`|` marks the start of the randomised levels):

- baseline: .956 .808 .743 .684 .632 .540 | .465 .249 .069 .018 | .007
- C 16/32: .956 .632 | .465 …
- C 8/32: .956 .743 | .466 …
- B {32}: .956 ×6 | .466 …

A coarse level leaves exactly what the fine levels left at the same byte count. The bit allocation reaches the same
state at each byte count whatever the stops in between.

## Breakdown

Profiled, one thread, 20 queries, ms. Baseline against C 8/32 (16/32 was not profiled). "Wall" is unprofiled, from a
single run.

| Cell | wall | tables | re-pack | decode | bounds+dec | compaction | level other | level setup | full |
|---|---|---|---|---|---|---|---|---|---|
| text 1% base | 61.6 | 8.3 | 11.7 | 13.3 | 13.8 | 6.0 | 10.1 | 1.8 | 1.6 |
| text 1% C 8/32 | 54.9 | 8.1 | 9.8 | 9.2 | 10.1 | 3.6 | 6.0 | 1.0 | 1.6 |
| text 50% base | 129.2 | 12.3 | 18.7 | 32.3 | 35.6 | 7.1 | 18.6 | 2.3 | 23.7 |
| text 50% C 8/32 | 112.0 | 11.0 | 15.9 | 23.7 | 26.5 | 4.4 | 12.2 | 1.2 | 21.6 |
| item 50% base | 115.0 | 11.0 | 18.3 | 27.8 | 30.6 | 6.8 | 16.5 | 2.1 | 17.7 |
| item 50% C 8/32 | 107.0 | 10.9 | 17.2 | 21.9 | 25.1 | 4.4 | 11.4 | 1.2 | 17.9 |

- **Tables do not fall.** Table cost is per unit and the unit count is unchanged (362k at text 1%), though the number
  of tables halves (9,089 → 5,115).
- **What falls is per-block, per-level work:** decode (part sums and accumulator loads and stores per level), bounds,
  compaction, level setup, and the profiler's per-block stamps ("level other").
- **Why B gains less (inferred from the B timings, not profiled):** B removes only the bounds, decisions and
  compaction. The per-level decode passes and gathers remain.

## Correctness

**50k** (`check.sh`: 200 item and 200 text queries, 7 sizes, filtered sets, k-NN, float64 oracle). For the baseline,
every B variant (including `none`) and every C variant, including all of C's `ddec` sub-variants:

- 0 wrong in and 0 wrong out;
- 0 deterministic and 0 randomised violations;
- 0 decisions not implied;
- k-NN 400/400;
- oracle: 0 wrong.

Decisions checked range from 6.96M to 26.4M; verify checks the would-be decisions at levels that do not decide. The
largest randomised error is 4.79 sd.

**1M** (`verify --vitem 20 --vtext 20`, brute force at every size, filtered sets, k-NN):

- C 8/32: 0 wrong in, 0 wrong out, 0 violations in 767M interval checks, 0 of 112M decisions not implied, k-NN 0
  mismatches.
- B {32}: the same, with 0 of 434M decisions not implied.
- Largest randomised error: 5.42 sd.
- Not run at 1M: C 16/32 and C 4/16/32.

## On by default (recommended)

- **A two-level deterministic schedule, 16/32 (or 8/32), deciding at both.** This needs `NDL` to become a run-time
  value, or `DLV` in `fit.py` with a matching build, and a format bump.
- **`ddec` off by default.** On six levels it gains at most 3–7% for text and costs up to 69% more bytes at small
  answers. On two levels, dropping the remaining decision does not help at small answers.

## Limits

- One 4-core VM, 1.6–1.9x slower than the 5900X; no AVX2 run.
- Small item answers are too noisy here to resolve changes below about 20%.
- C was compared with a baseline pooled across processes, not in the same process.
- One 1M sample, one fit, 20 queries per class.
- Fixed plan constants throughout; the planner was not refitted for the new schedules.
- 1M checks were run for C 8/32 and B {32} only.
- The queries were built here and are not checked against the original encodings: 1M is 20 random row groups of the
  Hugging Face set with 1,000 held out as item queries, and text is the 369 committed prompts through the BioCLIP-2
  text tower.
- Not timed: `ddec=none` on six levels (checked at 50k only). Not profiled: 16/32.
- The scripts in `results/detlevels/` carry this machine's paths.
