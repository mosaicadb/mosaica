# Dump test of the rate-margin law: notebook

Experimentalist's log for DESIGN-CLAIMS C4.4 on the regenerated 50k verify dump. Every analysis job ran as
`notes/program/run-limited.sh 4-7 4 12 python3 <script>` from this directory unless noted.

## 2026-11-21

09:13. Regenerated the dump, from `work/` (so `--results study_res` lands there), cores 4-7, 21 s wall:

    VEXACT_DUMP=.../dump-test/work/dump.bin taskset -c 4-7 /home/joe/code/vexact-bounds-target/bin/p1c --v2 --int
      --no-approx --index /home/joe/code/vexact-rand-data/c50k_r4 --queries /home/joe/code/vexact-data/bioclip2-text/text_q.npy
      --item-queries /home/joe/code/vexact-combined-data/c50k/item_q.npy --results study_res --gtab --flags repack
      --vcheck 10 --vfilt 1 verify oracle --vitem 10 --vtext 10

Output in `work/run.log`, `work/study_res/` (verify2.csv has bytes per item per (query, size, plan)). `work/` was not
ignored by git (only `work_*/` is); I put a `.gitignore` containing `*` inside it.

09:14. `reports/bounds3/dump_stats.py` on the new dump -> `dump_stats_regen.log`. Identical to the committed
`dump_stats.log` in every line except the total record count (19,473,729 now, 19,473,773 then; the 44 are as-planned
records, bit 0 clear, whose plan depends on timing calibration). Index-plan records: 9,736,897 in both.

Which build. p1c's mtime is 2026-11-14 13:29, the time of commit ffb96672c in /home/joe/code/vexact-bounds ("study
dump"); the per-(group, level) constants arrived in c25333fa9 at 13:38. At ffb96672c the randomised half-width is
`csg * sd + hn` with csg = meta.json CSG = 9.330163 at every randomised level (sqrt(2 * 43.526), 43.526 =
ln(2 N L / eps) at N = 1e9, L = 4, eps = 1e-9: two-sided, sub-Gaussian). The run prints "c = 9.33". The 50k index at
today's constants would use 6.724 (one block, level 56 B) and 7.639 (several), SYSTEM.md section 3; the 1M index uses
6.931 / 7.963. Rates are designed with the fixed CSG, so the stored code does not depend on the constant; records exist
while the 9.33 bound keeps an item, so any constant <= 9.33 can be replayed exactly.

09:16. `c4_dump_test.py` (copied here unchanged) with c = 9.33016 at all four levels, b = 29.5, 41.0, 67.7, 101.1
(stored increments of this run's build line: 66.5, 107.5, 175.2, 276.3). -> `c4_dump_test_c933.{log,json}`.
`t2_criteria.py` evaluates criteria 1 and 2 from the json.

09:17. `prep.py` -> `work/rec_*.npy` (index-plan records as columns sorted by item and level), `prep.log`.
`dt.py` holds the loading, the half-width (with terms selectable) and the replay. `replay_check.py`: at 9.33 the replay
keeps every record but one, so the replay is exact.

09:20. `c4_dump_test_replay.py` at 9.33, 8 and 6.724/7.639 -> `replay_c933`, `replay_c8`, `replay_c50kcur`
(`.log`, `.json`), criteria in `crit_*.log`. The unchanged script at 6.724/7.639 (no replay) ->
`c4_dump_test_c50k_noreplay.log`, kept to show the miscount.

09:21. `scores.py`: exact float64 scores of the 20 queries against all 49,500 normalised rows (`work/scores.npy`) and the
run's thresholds (`work/thresholds.npy`). Count of scores >= t matches the answer sizes within 18 rows of up to 37,125
(f16 vs f64 near t). Score sd: item 0.065 mean (model 0.068), text 0.048 (model 0.040). -> `scores.log`.

09:22. `step3.py 8 8 8 8 c8`: the substitution chain X0 (model of c4.py row 5) -> X1 (measured margins) -> X2 (the
index's deterministic levels) -> X3a (each item's own eta) -> X3 (measured half-widths, true margins) -> X4 (the
index). -> `step3_c8.log`, `step3_c8.json`. The model's stored bytes replaced by this build's (66.5 ... 276.3 with the
key); X0 at item 50% is 130.8 against c4.py's 131.7. The replay of full reads at 9.33 matches the run's verify2.csv
within 1 per (query, size). 24,606 of 5.5 M law-survivals have no next record (the 9.33 estimate had decided them);
X3 treats them as decided, so X3 is low by at most 24,606 deeper entries over all 140 cells.

09:25. `step3_levels.py 8 8 8 8 c8` and `6.724 7.639 7.639 7.639 c50kcur` -> `levels_*.log/json`: per level entrants,
decisions, μ/h quantiles, needless entries, early exits, width shares, coded-block pairs and rises, and the share of
rows within x of t against Gaussians.

09:27. `step3_extra.py` (κ_eff and the reversed order; η by size; absolute and normalised widths against the model) ->
`step3_extra_c8.log`, `step3_extra_c50kcur.log`. `chain_table.py c8 c50kcur` -> `chain_table.log`.

09:28. `subset_check.py`: the 50k corpus rows are in the 1M corpus; the item query files differ.

09:29. `t5.py` at 9.33 and 8 -> `t5_c933.log`, `t5_c8.log`.

Dump left at `work/dump.bin` (1,869,477,984 bytes); `work/` holds 2.5 GB in all with the column copies and scores.
