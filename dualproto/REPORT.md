# The dual-basis metric in the native version 2 prototype

Saved by the coordinating session from the agent's returned text (the agent could not write files
itself); paths adjusted to this branch. Every number is the agent's.

The question: does the dual pair of `groups/REPORT.md` section 2.4 (the matrix geometric mean of the
inverse query-offset covariance and the residual covariance) reduce bytes read and wall time on the
real quantised code of `rust/` (`--v2`), and why did `FINDINGS.md` section 7 record the same basis
("simultaneous diagonalisation") as hurting beyond 16 bytes?

Setup: the Rust `--v2` prototype, the 1M BioCLIP-2 sample, n = 768, K = 1,000, G = 64, the
prototype's own 200 item and 369 text queries, answer sizes 0.01 / 0.1 / 1 / 5 %. The prototype
changes are in `changes.diff` (against `6a1e09a`; not applied to `rust/` on this branch, because it
also repoints hard-coded paths to the machine it ran on). New tools are in `tools/`, results in
`results/<variant>/`, logs in `logs/`.

## Summary

- **The dual basis helps on the real quantised code; no run produced a violation.** Bytes read per
  item on the index path:
  - (b) Σ_Q fitted to corpus items only: items −9 / −17 / −24 / −30 %, text −10 / −9 / −7 / −6 %.
  - (c) Σ_Q mixing items and held-out text prompts: items −6 / −13 / −19 / −25 %, text
    −34 / −32 / −28 / −25 %.
  - Every variant: 0 wrong answers, 0 deterministic and 0 randomised violations, exact k-NN on
    569 / 569 queries. The baseline reproduces the published index exactly.
- **Wall time follows the bytes only where they move a lot.** (c) on text is faster in every cell of
  both timing rounds (12–26 % at one thread; k-NN 20–25 %). (b) on items is faster only at the 1 %
  and 5 % answers (18–24 %). Small answers are dominated by fixed per-query costs; differences there
  are within the ±15 % noise between rounds.
- **The dead-list loss came from how Σ_Q was fitted, not from clipping.** `exp4` against `exp5`,
  redone with `exp5`'s own quantiser on 12 groups (220,263 items, 569 queries), reproduces the
  dead-list entry. Clip rates are the same in both bases (about 2 %), and removing the clip helps both
  equally; `exp5`'s basis still loses at 48 and 96 bytes. Fitting Σ_Q as in (b) (one global Σ_Q,
  offsets to clusters drawn by size, the centre projected out, a ridge) gives a 32 % smaller bound for
  items at 96 bytes in that same quantiser, with or without clipping.
- **The sort key and the support table were not changed**, so items touched are identical in every
  variant; all of the gain is in the per-item levels. The ν-sort of `groups/REPORT.md` was not run
  natively.

## 1. Variants and commands

| Variant | Σ_Q fit | Profile |
|---|---|---|
| (a) | version 2 as published: PCA bases | data-only `H25` |
| (b) | 3,000 non-test corpus items × 40 clusters drawn by size, Δ = q − (q·m̂)m̂, one Σ_Q for all groups, ridge 0.1·tr/n (as in `groups/scripts/sigmaq.py`) | `prof_qi`: 5,000 non-test items against their 4 nearest clusters, raw q − m, projected with the int8 query-side basis, mean squares per group (as `combined/build.py` computes Qi) |
| (b) h25 | as (b) | the H25 hedge of `prof_qi` |
| (c) | ½(Σ_item/tr + Σ_text/tr), Σ_text from the 2,206 prompts of `agent2/encode_train.py` | `prof_qi`, so (b) and (c) differ only in Σ_Q |
| (c) qmix | as (c) | a Qmix-style profile in the new coordinates |

With `B=<prototype target>/release/vexact` and `P` the working directory:

```bash
cd combined && python build.py m768
cd rust/tools && python export.py m768 $P/rustdata/m768
python profile.py m768 h0.25 $P/rustdata/m768/prof_h0.25.npy
python dual.py item $P/rustdata/dual_item
python dual.py mix $P/rustdata/dual_mix

VEXACT_CLIPSTATS=1 $B --v2 verify bytesq --nq 369 --data $P/rustdata/m768 --res $P/results/a
VEXACT_CLIPSTATS=1 $B --v2 verify bytesq --nq 369 --data $P/rustdata/dual_item --profile $P/rustdata/dual_item/prof_qi.npy --res $P/results/b
VEXACT_CLIPSTATS=1 $B --v2 verify bytesq --nq 369 --data $P/rustdata/dual_mix --profile $P/rustdata/dual_mix/prof_qi.npy --res $P/results/c
$B --v2 bytesq --nq 369 --data $P/rustdata/dual_item --profile $P/rustdata/dual_item/prof_h0.25.npy --res $P/results/b_h25
$B --v2 bytesq --nq 369 --data $P/rustdata/dual_mix --profile $P/rustdata/dual_mix/prof_qmix.npy --res $P/results/c_qmix

# timed, one at a time, holding a lock against other jobs and logging uptime before and after
tools/run_e2e.sh TAG DATA PROFILE   # VEXACT_CLIPSTATS=1 $B --v2 e2e --nq 100 --nflat 10 --data DATA --profile PROFILE --res $P/results/TAG
```

## 2. Correctness (measured)

`verify`: 569 queries × 4 answer sizes; intervals checked at every level on 25 item and 25 text
queries.

| Variant | Wrong in / out | Interval checks | Deterministic violations | Randomised violations | Largest randomised error | k-NN exact |
|---|---|---|---|---|---|---|
| (a) | 0 / 0 | 731,378,512 | 0 | 0 | 5.08 sd | 569 / 569 |
| (b) | 0 / 0 | 605,382,696 | 0 | 0 | 5.42 sd | 569 / 569 |
| (c) | 0 / 0 | 512,503,417 | 0 | 0 | 5.41 sd | 569 / 569 |

c = 9.46 throughout; no bound or slack was changed. Fewer interval checks under (b) and (c) means
items stop at earlier levels. `verify` was not run for (b) h25 and (c) qmix.

## 3. Bytes per item on the index path (`bytesq`, 200 item and 369 text queries; measured)

| Variant | Item | Text |
|---|---|---|
| (a) | 2.36 / 6.15 / 23.36 / 80.63 | 27.92 / 40.44 / 79.60 / 155.62 |
| (b) | 2.15 / 5.13 / 17.81 / 56.60 (−9 / −17 / −24 / −30 %) | 25.22 / 36.66 / 73.81 / 146.28 (−10 / −9 / −7 / −6 %) |
| (b) h25 | 2.21 / 5.28 / 18.55 / 59.07 | 23.41 / 34.49 / 70.81 / 141.10 (−16 / −15 / −11 / −9 %) |
| (c) | 2.22 / 5.34 / 18.87 / 60.52 (−6 / −13 / −19 / −25 %) | 18.30 / 27.32 / 57.38 / 116.80 (−34 / −32 / −28 / −25 %) |
| (c) qmix | 2.22 / 5.31 / 18.76 / 60.21 | 17.82 / 26.69 / 56.36 / 114.85 (−36 / −34 / −29 / −26 %) |

Items touched are identical in every variant (item 13 / 22 / 47 / 84 %, text 75 / 83 / 94 / 99.6 %):
the cut and the sorted range are unchanged. These use all queries; `rust/REPORT2.md` used the first
100 of each, so the two are not directly comparable.

End to end, including full vectors read for small clusters (first 100 queries of each kind):

| Variant | Item | Text |
|---|---|---|
| (a) | 10.77 / 16.27 / 30.57 / 80.99 | 31.47 / 40.55 / 74.71 / 143.53 |
| (b) | 10.68 / 15.60 / 26.87 / 60.41 | 28.04 / 35.53 / 66.26 / 128.92 |
| (c) | 10.75 / 15.78 / 27.66 / 63.52 | 23.72 / 29.14 / 54.06 / 106.96 |

## 4. Orthogonality assumptions in the code, and what was changed

**Already correct for a non-orthogonal pair** (the `--v2` query path was written as a pair): the query
side computes qv = V_qᵀq exactly from the int8 basis; the item side codes z = V_q⁻¹r; centres are
projected on the query side (mv = V_qᵀm) and α·m is removed there; m·r comes from â; the weighted
error norms are taken in z = r″; randomised blocks rotate e/dt on the item side and qv·dt on the query
side with the same orthogonal rotation; block norms |y| are item-side; b2, bm, btot and l1m come from
mv; rmax, x2, the sorted range and the support table use Euclidean |r| and |x|, which is right. The
exported V is VQ (query side), so z = VQ8⁻¹r ≈ VDᵀr = r″.

Changes (all in `changes.diff`):
1. `rust/src/v2/mod.rs`, `quant_basis`: Newton–Schulz started from Vᵀ, which converges only for a
   near-orthogonal basis. It now starts from a Gauss–Jordan inverse (`gauss_jordan_inverse`) when
   ‖I − V_qV_qᵀ‖_F > 0.5 and polishes with the same Newton steps. e_inv is still the measured defect,
   so the "true inverse" argument is unchanged; PCA bases take the old path. Largest condition number
   of VQ8: 209 (b), 227 (c); largest defect 1.0e-12.
2. `rust/src/index.rs` (`Params`, `load_params`) and `rust/src/v2/mod.rs` (`GroupPlan2::new_inner`):
   `item_scale` divided |r|² by Σλ, which equals the mean |r|² only in an orthogonal basis. It is now
   the per-group mean |r|², from an optional `r2.npy` (falling back to Σλ); the query side in
   `v2/q.rs` uses the same field.
3. Data: `lam` must be the variance of the coded coordinates. `tools/dual.py` replicates the Rust
   int8 quantisation of VQ (per-column scale max/127, round, clamp), takes its exact inverse, and
   exports lam as the mean of z² per group. Its total agrees with Σ√γ to a factor of 1.0005, but
   single coordinates differ by up to 2.1×. Profiles are projected with the same quantised VQ8.
4. Instrumentation: `VEXACT_CLIPSTATS=1` counts overflows in `encode_cluster`; `rand_choose` returns
   the scale it chose. Nothing stored changes.
5. Paths: `rust/src/run2.rs` gains `--data DIR` and `--res DIR`; hard-coded paths repointed in
   `main.rs`, `scale.rs`, `.cargo/config.toml`, `combined/build.py`, `rust/tools/export.py` and
   `rust/tools/profile.py` (`tools/fixpaths.py`).
6. New tools: `dual.py`, `diag_sq.py`, `clipcheck.py`, `run_e2e.sh`.

Not changed (from reading only; not used by `--v2`): the version 1 tree index (`index.rs`,
`query.rs`) uses z = Vᵀr with an orthogonality-defect allowance, so it stays sound but becomes loose
for a non-orthogonal basis; the synthetic items of the scale test (`synth.rs`, `scale.rs`, `Synth2` in
`v2/q.rs`) build x = m + V·z, the wrong side of the pair.

## 5. Wall time (`e2e`; measured)

Median ms over the first 100 queries of each kind, best of 2 runs per query. Columns: the four answer
sizes, k-NN at k = 10, then flat f16 and flat int8 scans (10 queries at 0.1 %).

| Run | Kind | Threads | 0.01 % | 0.1 % | 1 % | 5 % | k-NN | Flat f16 | Flat int8 |
|---|---|---|---|---|---|---|---|---|---|
| R1 (a) | Item | 1 | 4.5 | 9.1 | 27.1 | 72.9 | 4.8 | 161 | 76 |
| R1 (b) | Item | 1 | 4.6 | 7.4 | 22.2 | 55.1 | 5.1 | 154 | 72 |
| R1 (c) | Item | 1 | 4.4 | 7.6 | 23.0 | 63.0 | 4.9 | 155 | 73 |
| R1 (a) | Item | 4 | 2.7 | 3.6 | 10.3 | 21.6 | 3.1 | 42 | 20 |
| R1 (b) | Item | 4 | 2.8 | 3.6 | 7.6 | 17.5 | 3.0 | 40 | 19 |
| R1 (c) | Item | 4 | 2.5 | 4.0 | 8.3 | 19.2 | 3.1 | 40 | 20 |
| R1 (a) | Text | 1 | 36.9 | 45.2 | 70.2 | 98.6 | 37.3 | 161 | 74 |
| R1 (b) | Text | 1 | 33.2 | 42.7 | 60.6 | 101.0 | 32.7 | 152 | 71 |
| R1 (c) | Text | 1 | 30.8 | 38.6 | 52.2 | 86.6 | 27.9 | 150 | 71 |
| R1 (a) | Text | 4 | 12.2 | 14.6 | 20.9 | 32.4 | 11.9 | 43 | 19 |
| R1 (b) | Text | 4 | 10.0 | 11.0 | 18.7 | 30.6 | 10.8 | 40 | 20 |
| R1 (c) | Text | 4 | 8.5 | 11.0 | 16.9 | 28.6 | 10.4 | 41 | 20 |
| R2 (a) | Item | 1 | 7.0 | 10.1 | 31.6 | 87.1 | 8.2 | 202 | 79 |
| R2 (b) | Item | 1 | 8.7 | 14.1 | 35.5 | 71.8 | 8.4 | 199 | 80 |
| R2 (c) | Item | 1 | 6.8 | 10.8 | 28.1 | 69.8 | 7.6 | 193 | 78 |
| R2 (a) | Item | 4 | 3.9 | 6.1 | 14.0 | 29.5 | 4.2 | 52 | 21 |
| R2 (b) | Item | 4 | 4.0 | 5.3 | 12.4 | 24.9 | 5.1 | 56 | 22 |
| R2 (c) | Item | 4 | 3.5 | 5.0 | 11.1 | 22.3 | 4.1 | 49 | 20 |
| R2 (a) | Text | 1 | 44.9 | 55.6 | 81.2 | 118.7 | 42.3 | 192 | 76 |
| R2 (b) | Text | 1 | 40.5 | 51.9 | 78.2 | 116.7 | 40.4 | 193 | 77 |
| R2 (c) | Text | 1 | 35.9 | 42.9 | 69.1 | 104.8 | 34.0 | 188 | 77 |
| R2 (a) | Text | 4 | 15.4 | 17.5 | 24.7 | 37.1 | 16.3 | 50 | 21 |
| R2 (b) | Text | 4 | 14.7 | 18.4 | 24.1 | 36.9 | 15.7 | 49 | 21 |
| R2 (c) | Text | 4 | 12.1 | 14.0 | 21.6 | 33.8 | 14.2 | 51 | 22 |

Run conditions: round 1 at load average 1.4–3.3 before each run (about 2.2 after); round 2 after a
container restart, on a machine about 25 % slower (flat f16 at one thread 188–202 ms against
150–161 ms), load 1.4–1.6 before. Compare variants only within a round. Round 1's first (b) run was
disturbed by another job and re-run (`results/b_disturbed`); round 2's first attempt was cut off by
the restart (`logs/a2_e2e_cutoff.log`).

Reading: (c) beats (a) on text in every cell of both rounds; (b) on items wins only at large answers;
at small item answers fixed costs dominate (about 10.7 of the 10.8 bytes per item at 0.01 % are
full-vector reads). Treat differences under about 10 % as unresolved: single cells vary by ±15 %
between rounds.

## 6. Overflow, clipping, and the dead-list entry

**The Rust encoder** (`VEXACT_CLIPSTATS`, all 1M items; measured). In every variant every coded
deterministic coordinate is a sign (±0.798) followed by up to four 1-bit residual stages (tree depths
2 and 4 are never chosen), so the code's reach is 0.798 + Σδ.

| | (a) | (b) | (c) |
|---|---|---|---|
| Beyond reach, % of coded coordinates, at 4 / 8 / 16 / 32 B | 31.7 / 28.2 / 25.5 / 23.3 | 33.6 / 29.4 / 26.4 / 22.9 | 32.7 / 29.4 / 26.1 / 22.8 |
| Excess share of the squared weighted error, at 4 / 8 / 16 / 32 B | 14.7 / 19.1 / 25.8 / 35.5 | 14.6 / 20.0 / 27.7 / 38.0 | 14.2 / 19.7 / 26.9 / 37.0 |
| Coded \|u\| > 3 / > 4, % (Gaussian 0.27 / 0.0063) | 0.301 / 0.0118 | 0.284 / 0.0102 | 0.281 / 0.0106 |
| Kurtosis | 3.08 | 3.07 | 3.07 |
| Randomised blocks 0–3 beyond scale, % | 0.67 / 1.50 / 4.39 / 4.49 | 0.58 / 1.46 / 4.39 / 4.47 | 0.66 / 1.51 / 4.39 / 4.47 |
| Escapes: norms, \|y\|, κ, tan | 742, 5,641, 14,562, 38,194 | 1,375, 5,844, 14,441, 36,884 | 1,252, 5,675, 14,522, 38,463 |

**The dead-list experiment redone** (`tools/clipcheck.py`; 12 groups, 220,263 items, all 569
queries, the uniform quantiser of `exp4.py` / `exp5.py`; mean certified half-width h at 4 / 16 / 32 /
48 / 96 bytes; 0 violations). Clip rates: PCA 2.6 % at 4 B, 2.1 % at 32 B, 2.0 % at 96 B; every
simultaneous-diagonalisation fit 2.0–2.4 %.

| Basis or Σ_Q fit | Clip | Item h | Text h |
|---|---|---|---|
| PCA | yes | .2127 / .1501 / .1119 / .0889 / .0512 | .2429 / .1778 / .1372 / .1123 / .0699 |
| PCA | no | .2084 / .1438 / .1048 / .0815 / .0443 | .2379 / .1703 / .1285 / .1028 / .0604 |
| `exp5` fit (per group, 8 nearest clusters, raw offsets, 0.8 / 0.2 blend) | yes | .1922 / .1398 / .1113 / .0939 / .0661 | .2267 / .1749 / .1475 / .1305 / .1037 |
| `exp5` fit | no | .1887 / .1325 / .1011 / .0827 / .0543 | .2225 / .1657 / .1339 / .1148 / .0851 |
| `exp5` fit + ridge 0.1 | yes | .1920 / .1397 / .1105 / .0925 / .0648 | .2171 / .1667 / .1387 / .1211 / .0937 |
| `exp5` fit, projected offsets + ridge | yes | .1970 / .1427 / .1121 / .0947 / .0662 | .2222 / .1686 / .1383 / .1212 / .0929 |
| Global, 8 nearest, projected, ridge | yes | .1997 / .1380 / .1009 / .0800 / .0501 | .2250 / .1649 / .1265 / .1043 / .0715 |
| (b)'s pair | yes | .1728 / .1140 / .0809 / .0622 / .0346 | .2132 / .1539 / .1169 / .0946 / .0580 |
| (b)'s pair | no | .1704 / .1096 / .0760 / .0573 / .0303 | .2103 / .1479 / .1099 / .0871 / .0508 |

Verdict:
- **Clipping is not the cause** (measured): clip rates differ by at most 0.25 points between bases;
  removing the clip helps both about equally; `exp5`'s basis still loses to PCA at 48 and 96 bytes
  without clipping (item h 0.0827 against 0.0815, 0.0543 against 0.0443).
- **The fitted Σ_Q is the cause** (measured): with (b)'s Σ_Q the same quantiser wins at every budget,
  −19 % at 4 bytes and −32 % at 96 bytes for items, −17 % for text at 96 bytes.
- **Why** (conjectured): a narrow, per-group, nearest-cluster law gives no bits to directions where
  real offsets still carry energy.

**Zero-bit slack** (`tools/diag_sq.py`; measured): with nothing read, the dual basis barely beats PCA
(items 0.149 against 0.152, text 0.228 against 0.232); the mixed fit gains 9 % on text. So the byte
gains come from energy compaction across levels, not from the zero-bit ball. (This is the plain
Cauchy–Schwarz ball; `groups/REPORT.md` section 3.3 reports a larger cut, 0.250 to 0.190 for text,
for the block-by-block bound in exact coordinates. The two measures differ and the gap has not been
reconciled.)

## 7. Not run

- The ν-sort of `groups/REPORT.md` (sort each cluster by ‖r‖ in M⁻¹ and drop the support table). Now
  warranted; the inputs are `rustdata/dual_*` (not committed).
- `verify` for (b) h25 and (c) qmix (bytes only).
- Filtered timings and the 10M scale test. The scale test's synthetic generator also has the
  wrong-side assumption of section 4.

Caveats: one global Σ_Q for all groups, at one ridge value.
