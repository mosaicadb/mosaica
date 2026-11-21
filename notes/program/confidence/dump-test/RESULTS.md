# Dump test of the rate-margin law (DESIGN-CLAIMS C4.4): results

Tags as `notes/README.md`. Every [measured] number below is from the regenerated 50k verify dump
(`work/dump.bin`, 19,473,729 records, of which 9,736,897 index-plan records are used; 10 item and 10 text queries x 7
answer sizes, 49,500 rows, n = 768), by the script named. The dump has records only from the first randomised level
(56 B) on, and only while the 9.33 bound keeps an item. Measured bytes in SYSTEM.md are at 1M; nothing here is at 1M.

## 1. The dump [measured: run.log, prep.py, dump_stats_regen.log]

- Regenerated with the REPORT.txt command, cores 4-7, written to `work/`; 21 s. `dump_stats.py` output is identical to
  the committed `reports/bounds3/dump_stats.log` in every line but the raw record count (19,473,729 now, 19,473,773
  then; the 44 are as-planned records, whose plan depends on a timing calibration). Index-plan records: 9,736,897 in
  both, by level 3,816,651 / 3,173,871 / 1,868,363 / 878,012. No record exists for a level below 56 B; every deeper
  record follows its item's record at the previous level (`prep.log`).
- Queries 0-9 item, 10-19 text; sizes 0.01%, 0.1%, 1%, 10%, 30%, 50%, 75% (5 to 37,125 rows). Items entering 56 B per
  size, summed over the 10 queries: item 5,629 / 12,335 / 34,588 / 232,475 / 410,806 / 420,088 / 411,568; text
  105,526 / 136,985 / 211,978 / 406,451 / 479,108 / 477,251 / 471,863.
- **Build and constants.** The binary is p1c, built 2026-11-14 13:29, the commit ffb96672c of vexact-bounds ("study
  dump"), before c25333fa9 brought the per-level exact constants. Its half-width is 9.330 * sd + deterministic terms at
  every randomised level (meta.json CSG = sqrt(2 ln(2 N L / eps)), N = 1e9, L = 4: two-sided and sub-Gaussian). The
  current constants are 6.724 (56 B, one coded block) and 7.639 (several) for this 50k index, and 6.931 / 7.963 at 1M.
  The rates are designed with the fixed CSG, so the stored code is the same; the decisions are not.
- The 50k build codes block 0 only at 56 B and all four blocks at 96, 160 and 256 B (group 0 rates [2,0,0,0],
  [2,1,0.5,0.125], [3,3,1,0.25], [6,4,2,0.5]); SYSTEM.md's 1M table codes blocks 0-2 at 96 and 160 B. Stored bytes per
  item, cumulative: 66.5, 107.5, 175.2, 276.3 at the four randomised levels, so a survivor reads 29.5, 41.0, 67.7 and
  101.1 B on entering them.
- **c4_dump_test.py counts every record as an entry.** That is right only at the dump's own 9.33. At smaller constants
  an item decided earlier still has deeper records, so I added a replay (`dt.py`, `c4_dump_test_replay.py`): a record
  counts only if the item survived every earlier level at the constants given. At 9.33 the replay keeps 9,736,896 of
  9,736,897 records (one lost to float rounding) and reproduces the script's output to the printed digits. Run
  literally at 6.724 / 7.639 without the replay, the script reports T3 pooled 0.957 (`c4_dump_test_c50k_noreplay.log`);
  with it, 1.000.

## 2. The test as specified [measured: c4_dump_test.py at 9.33; c4_dump_test_replay.py at 9.33, 8 and 6.724/7.639; t2_criteria.py]

The scale η is Σ_j R_j of the item's 56 B record (R_j is filled for coded and uncoded blocks), so u and w are in units
of the blockwise Cauchy-Schwarz bound of the error the 32-byte prefix leaves, not of the zero-bit bound.

T1, geometric mean of w = h/η and sd of ln w, per class and level, records entered (constants 9.33 | 8 | 6.724/7.639):

| Cell | w̄ at 9.33 | sd ln w | w̄ at 8 | w̄ at 6.724/7.639 | sd ln w there |
|---|---|---|---|---|---|
| item 56 B | 0.714 | 0.076 | 0.687 | 0.660 | 0.088 |
| item 96 B | 0.331 | 0.081 | 0.284 | 0.272 | 0.081 |
| item 160 B | 0.140 | 0.101 | 0.120 | 0.114 | 0.101 |
| item 256 B | 0.0532 | 0.146 | 0.0457 | 0.0437 | 0.145 |
| text 56 B | 0.764 | 0.061 | 0.741 | 0.719 | 0.069 |
| text 96 B | 0.347 | 0.074 | 0.297 | 0.284 | 0.074 |
| text 160 B | 0.151 | 0.091 | 0.129 | 0.124 | 0.091 |
| text 256 B | 0.0634 | 0.116 | 0.0544 | 0.0520 | 0.115 |

Refutation criteria, the same verdict at all three sets of constants:

1. **sd of ln w at most 0.3 in every cell: met.** 0.061 to 0.146; it rises with depth (the last level's w varies
   most, cv 0.14 for item queries).
2. **Half-survival point within 10% of w̄: met** (u50 / w̄ = 0.990 to 1.005 in all 24 cells). **10-90 span at most
   twice the predicted one with T1's spread: met** (measured over predicted 0.67 to 1.13). **Item and text survival
   curves within 5 points at the same u: NOT met.** The largest difference at the same u is 28, 14, 20 and 40 points at
   the four levels (9.33; 27, 12, 19, 35 at today's constants). The cause is that w̄ differs by class at the same level:
   text's is 7%, 5%, 8% and 19% larger (9.33). With u divided by each class's own w̄ the curves agree within 6.1, 2.5,
   2.4 and 3.5 points. So query type enters other than through u: through the level's normalised width, which depends
   on how the query's weight is shared among the blocks (text puts more of η in blocks 2 and 3, coded at lower rates;
   `dump_stats_regen.log`). Item 4 of C4.1 is refuted in its stated form; the per-class form holds.
3. **T3, predicted over actual bytes of the randomised levels and full vectors: met.** Pooled 1.001 (9.33), 1.001 (8),
   1.000 (6.724/7.639); rms relative error per (query, size) 0.051 / 0.049 / 0.049, worst 0.17 to 0.18 (item queries at
   0.01%, where 5 rows are the answer). By class and size every cell is within 0.99 to 1.05 pooled. Item 50% at
   6.724/7.639: 229.5 B/item of N actual, predicted/actual 0.996, rms 0.037. T3 uses each class's own w̄, so it does
   not test criterion 2's last part.
4. T5 is in section 4 below.

What section 2 shows: given an item's normalised margin at 56 B, the four randomised levels behave as the staircase
says, for both classes, to within 5% of bytes per query. The factor of two in C4.2 is not in that part of the law. It
has to be in what C4.2 adds on top: the margins (Gaussian scores, one η per group), the widths the model gives each
level, or the deterministic levels, which this dump does not see.

T4 and T5 [measured: t5.py, `t5_c933.log`, `t5_c8.log`]. Bits added per halving of w̄, against Σ_j n_j cos²φ_j at the
level entered (measured tan φ; the bits include the stored scalars):

| Step | item, c = 9.33 | text, c = 9.33 | item, c = 8 | text, c = 8 |
|---|---|---|---|---|
| 56 -> 96 B (328 bits) | 296 / 272 = 1.09 | 288 / 272 = 1.06 | 258 / 272 = 0.95 | 249 / 272 = 0.92 |
| 96 -> 160 B (542 bits) | 434 / 425 = 1.02 | 451 / 424 = 1.06 | 434 / 425 = 1.02 | 451 / 424 = 1.06 |
| 160 -> 256 B (809 bits) | 582 / 533 = 1.09 | 647 / 529 = 1.22 | 582 / 533 = 1.09 | 648 / 529 = 1.23 |

4. **Criterion 4: met.** The only step below Σ n cos²φ is 56 -> 96 B at c = 8, by 5% (item) and 8% (text), within the
   scalars' share of that step (three or four block-rises at 1 to 1.5 B each, 7 to 15% of 41 B). That step is not a pure
   rate rise: it replaces three blocks' Cauchy-Schwarz terms by coded ones, so its ratio depends on c. T4: rms tan φ of
   block 0 at 56 B is 0.4245, 1.65 times the sphere bound at group 0's 2 bits (1.61 after the 1.0245 scale; the build's
   own table gives 1.55 before rounding up); I did not go further, because only group 0's rates are logged and the 50k
   index has three groups.

## 3. Where the bytes of large in-distribution answers go [measured: step3.py, step3_levels.py, step3_extra.py, chain_table.py]

Everything at c = 8 (the model's constant) unless stated; the same at today's 6.724 / 7.639 is in the `_c50kcur` files
and changes no conclusion. Bytes are per item of N at 50k, deterministic levels (from the run's verify2.csv less the
replayed randomised bytes) plus randomised levels and full vectors (replayed from the records).

### 3.1 Per level, item 50% (control: text 50%), summed over the 10 queries [measured: `levels_c8.log`]

| Level | Entrants (share of N) | In / out / undecided | μ/h of decided, p10/p50/p90 | μ/h of undecided | Needless entries (μ > previous h) | Early exits (decided, μ ≤ h) | Kept with μ > h |
|---|---|---|---|---|---|---|---|
| 56 B | 420,088 (0.849) | 22,222 / 30,603 / 367,263 | 1.03 / 1.25 / 1.58 | 0.06 / 0.32 / 0.77 | n/a | 2,738 | 2,996 |
| 96 B | 367,263 (0.742) | 69,843 / 73,212 / 224,208 | 1.06 / 1.50 / 2.18 | 0.09 / 0.45 / 0.88 | 2,996 (0.8%) | 8,578 | 7,740 |
| 160 B | 224,208 (0.453) | 60,482 / 57,425 / 106,301 | 1.10 / 1.62 / 2.31 | 0.10 / 0.49 / 0.91 | 7,740 (3.5%) | 4,908 | 4,745 |
| 256 B | 106,301 (0.215) | 32,589 / 31,809 / 41,903 | 1.15 / 1.78 / 2.59 | 0.10 / 0.49 / 0.92 | 4,745 (4.5%) | 2,042 | 1,959 |
| full | 41,903 (0.085) | | | | | | |

Text 50%: entrants 0.964 / 0.908 / 0.641 / 0.328 of N, full 0.143; needless entries 0.4%, 2.8%, 4.3%; the margins in
units of h are the same as for item queries to two digits from 96 B on. Item 30% and 75% have the same shape
(`levels_c8.log`).

- Decided items sit at 1.0 to 2.6 half-widths, undecided ones spread over 0 to 0.9 with median 0.3 to 0.5: within a
  shell the margins are close to uniform, as the law assumes.
- **Needless entries cost 2.3 B/item at item 50%** on entering 96, 160 and 256 B (0.25, 1.06, 0.97), and the 1,959 kept
  at 256 B with μ > h cost 6.1 B/item of full vectors. Early exits save about as much: the law decided on true margins
  with the same half-widths (X3 below) reads 238.0 B/item of randomised bytes, the index 237.9. **The estimate's error
  costs nothing net (factor 1.000).**

### 3.2 The factor, term by term [measured: `step3_c8.log`, `chain_table.log`]

A chain of substitutions from the model to the index. Each step changes one thing; the product telescopes to the whole
factor, so nothing is left over inside the chain.

- X0: c4.py row (5): Gaussian scores (sd 0.068 item, 0.040 text), η = κF_g over the 16 groups of `spec_v1.json`
  (κ = 1 item, 1.742 text), the built widths at c = 8, this build's stored bytes. Item 50%: 130.8 B (c4.py: 131.7).
- X1: the same model on this index's measured score margins (all 49,500 rows, each query's own threshold).
- X2: the index's deterministic bytes and its set of items entering 56 B; then the model's absolute widths w_l·F_g.
- X3a: the model's normalised widths w_l/w_32 times each entrant's own η = Σ_j R_j.
- X3: each entrant's measured half-width at c = 8, decided on its true margin (the law, no estimate error).
- X4: the index, replayed at c = 8.

| Cell | X0 model | X4 index | Factor | Margins X1/X0 | Det. levels X2/X1 | η X3a/X2 | Normalised widths X3/X3a | Estimate X4/X3 |
|---|---|---|---|---|---|---|---|---|
| item 10% | 67.5 | 69.6 | 1.03 | 0.69 | 1.27 | 1.27 | 0.92 | 1.00 |
| item 30% | 116.8 | 232.9 | 1.99 | 1.59 | 1.05 | 1.28 | 0.94 | 1.00 |
| **item 50%** | **130.8** | **272.0** | **2.08** | **1.68** | **1.035** | **1.28** | **0.935** | **1.000** |
| item 75% | 108.5 | 197.8 | 1.82 | 1.43 | 1.06 | 1.29 | 0.93 | 1.00 |
| text 10% | 174.1 | 179.6 | 1.03 | 0.79 | 1.05 | 1.23 | 1.01 | 1.00 |
| text 30% | 270.3 | 358.7 | 1.33 | 1.06 | 1.02 | 1.20 | 1.03 | 1.00 |
| text 50% | 297.6 | 398.2 | 1.34 | 1.10 | 1.01 | 1.17 | 1.03 | 1.00 |
| text 75% | 254.3 | 312.7 | 1.23 | 1.02 | 1.02 | 1.16 | 1.02 | 1.00 |

At today's constants item 50% is 263.7 B and the factor 2.02, with the normalised-width term 0.907 in place of 0.935
and the others unchanged.

**The terms at item 50%.**

1. **Margins, x1.68.** In-distribution scores are about twice as dense at the median threshold as a Gaussian of the
   same sd: the share of rows within 0.0025, 0.01, 0.02, 0.04 of t is 0.062, 0.240, 0.439, 0.673 measured against
   0.029, 0.117, 0.231, 0.444 for the model's Gaussian (2.1, 2.0, 1.9, 1.5 times), and 0.031 to 0.463 for a Gaussian
   with each query's own mean and sd. The model with measured margins sends 70% of items into 56 B (Gaussian: 54%) and
   reads 6.6% of full vectors (Gaussian: 3.1%). Text at 50% is 1.19 times its Gaussian and its term is 1.10. At 10% the
   in-distribution density is 0.39 times the Gaussian and the term is 0.69, which is why the model fits there at 50k.
   With the order reversed (widths first, by an effective scale κ_eff = 1.34 that makes the model reproduce the index
   on the measured margins), the margins term is 1.64 and the rest 1.27 (`step3_extra_c8.log`).
2. **Deterministic levels, x1.035.** 85% of items enter 56 B where the model, on the same margins, sends 70%; the
   deterministic bytes are 34.2 against 30.8.
3. **η, x1.28.** Each entrant's blockwise Cauchy-Schwarz scale after the 32-byte prefix is on average 0.0796 (median
   0.0769) at item 50%, against the model's equivalent w_32·F̄ = 0.0524: 1.52 times. So the absolute half-widths are
   1.41, 1.23, 1.59 and 1.41 times the model's at 56, 96, 160 and 256 B. The same term is 1.16 to 1.23 for text
   queries, where the fitted κ = 1.742 had absorbed it at 1M. η falls with the answer size for item queries (0.103 at
   0.01%, 0.094 at 1%, 0.080 at 30-75%), the opposite of C4.2's suggested cause (i).
4. **Normalised widths of the randomised levels, x0.935.** h/η measured against the model's w_l/w_32: 0.93, 0.81,
   1.03, 0.91 at the four levels (item 50%). The index's randomised levels are at or below the model's built schedule
   at the same normalised margin.
5. **Estimate error, x1.000.** Section 3.1.

Product: 1.68 x 1.035 x 1.28 x 0.935 x 1.000 = 2.08.

**Answer to the question.** The index does not read twice what it needs relative to the model's built schedule: given
each item's margin and scale, its randomised levels read what the law says (T3, X3 = X4) and its normalised widths are
0.81 to 1.03 of the model's. The model is wrong for these queries, in two places: its Gaussian margins (x1.68, specific
to in-distribution queries at large answers) and its scale η (x1.28 here, common to both classes; at 1M it was hidden
in the fitted κ for text). "Needs" here means the model's row (5), which includes the built codes; it is not the floor.

**What is left unexplained.** Nothing inside the chain, but two of its terms are measured, not traced to a cause. The
η term (x1.28) cannot be split by the dump into the query's offset length, the residual the 32-byte prefix leaves,
and this index's three-group basis, because the dump holds only the product Σ_j |y_j|·‖Δ_j‖. The deterministic term
(x1.035) is a total: the dump has no record below 56 B.

### 3.3 The width's parts and what each costs in bytes [measured: `step3_c8.log`, `levels_c8.log`]

Randomised bytes at item 50% under the law (X3, 238.0 B/item) with one term of the half-width set to zero: without the
uncoded blocks' Cauchy-Schwarz term 206.5 (-31.5, 13%); without the random term c·sd 54.2; without the other
deterministic terms 235.7 (-2.3, 1%). The uncoded term exists only at 56 B in this build (block 0 alone is coded
there), where it is 76% of the mean half-width; the model's 56 B width has it at 84%.

## 4. The half-width's parts and the scalar reads [measured: step3_levels.py, `levels_c8.log`, `levels_c50kcur.log`]

Share of each level's mean half-width over entrants (random term c·sd / uncoded blocks' Cauchy-Schwarz / other
deterministic terms). It hardly moves with the answer size:

| Class, constants | 56 B | 96 B | 160 B | 256 B |
|---|---|---|---|---|
| item, c = 8 | 0.24 / 0.76 / 0.001 | 0.997 / 0 / 0.003 | 0.995 / 0 / 0.005 | 0.985 / 0 / 0.015 |
| text, c = 8 | 0.19 / 0.81 / 0.0005 | 0.997 / 0 / 0.003 | 0.996 / 0 / 0.004 | 0.989 / 0 / 0.011 |
| item, 6.724 / 7.639 | 0.21 / 0.79 / 0.001 | 0.996 / 0 / 0.004 | 0.994 / 0 / 0.006 | 0.984 / 0 / 0.016 |
| text, 6.724 / 7.639 | 0.16 / 0.84 / 0.0005 | 0.997 / 0 / 0.003 | 0.996 / 0 / 0.004 | 0.989 / 0 / 0.011 |

At the smallest answers the other deterministic part at 256 B grows to 0.03 (item 0.01%). The per-size rows are in
the logs.

(Item, coded block) reads per query, and rises among them (a block is counted as rising when it is newly coded or its
tan φ changed from the previous level, so a rise that kept the same tan code is missed), at today's constants:

| Cell | 56 B | 96 B | 160 B | 256 B | Per item of N: pairs / rises |
|---|---|---|---|---|---|
| item 1% | 3,459 / 3,459 | 7,039 / 5,279 | 2,582 / 2,582 | 1,038 / 779 | 0.29 / 0.24 |
| item 10% | 23,248 / 23,248 | 46,140 / 34,605 | 11,692 / 11,692 | 4,193 / 3,473 | 1.72 / 1.48 |
| item 50% | 42,009 / 42,009 | 144,700 / 108,525 | 86,624 / 86,624 | 40,748 / 40,008 | 6.35 / 5.60 |
| text 1% | 21,198 / 21,198 | 52,743 / 39,557 | 15,238 / 15,238 | 4,828 / 4,130 | 1.90 / 1.62 |
| text 50% | 47,725 / 47,725 | 178,516 / 133,887 | 123,178 / 123,178 | 62,304 / 59,562 | 8.32 / 7.36 |

Every entrant at 56 B also reads four |y| scalars. At item 50% that is 5.60 block-rises per item of N (5.73 at c = 8), inside C5.1's
inferred 5.4 to 6.0 at 1M, but this is a different index (all four blocks coded from 96 B here, three at 1M).

## 5. What the dump cannot show

- Nothing below 56 B: the deterministic levels enter only as a total (verify2.csv bytes less the replayed randomised
  bytes) and as the set of items entering 56 B. The hand-over and the deterministic staircase are out of reach.
- It is a 50k index (K = 50, G = 3), and SYSTEM.md's bytes are at 1M (G = 64). The 50k corpus is drawn from the 1M
  corpus (four probe rows all found, `subset_check.log`), but the item queries are a different held-out set. The
  margins term should carry to 1M for the same kind of query; the η term may not. If both carried, the model's 131.7
  B at item 50% would become 131.7 x 1.68 x 1.24 = 274 against 276.2 measured; that is arithmetic, not a measurement.
- At item 10% the 50k index reads 69.6 B (c = 8), the model 67.5 and the 1M index 133.2. The dump does not reproduce
  the 1M shortfall at 10%; there the comparison of a 50k dump with 1M figures does matter, and I cannot say which way.
- The dump's own constant is 9.33 everywhere; other constants are replays of the same records, exact for any c <= 9.33.
  24,606 of 5.5 M law-survivals at c = 8 have no next record (the 9.33 estimate decided them), so X3 is slightly low.
- 10 queries per class. No timing; nothing here says anything about time.
