# Referee report: DESIGN-CLAIMS.md (C3, C4, C5)

Referee: independent agent, 2026-11-21. Object: `notes/program/confidence/DESIGN-CLAIMS.md` sections 0, C3 (a) to
(c), C4, C5. Not read: `DESIGN-NOTEBOOK.md`. Later results read: `dump-test/RESULTS.md`, `CLAIMS.md` (after its
referee), `codes/REPORT.md`. Scripts of this referee are `referee/rd_*.py`, run through `run-limited.sh 3 1 2`.

Verdicts are appended step by step below.

## 1. Section 0, the model

Verdict: **the width formula (0.1) is assembled correctly from its parts, with one omission that matters (nesting) and
one category point that must be stated.**

- Deterministic part. W_T(H) = Σ min(aᵢ, λ) with Σ ln⁺(aᵢ/λ) = H is Lemma 2.21's closed form, and Theorem 2.24 makes
  it a floor on the *mean* half-width E h of any sound certificate of T with H(c) ≤ H nats, Gaussian residuals and
  offsets [proved, refereed in chapter 2]. Using W_D for D is therefore optimistic (a floor), as section 0 says.
- Rotated block. c·‖y‖‖Δ‖·tan φ/√d is Lemma 3.1 with ‖P_{y⊥}Δ‖ ≤ ‖Δ‖ (an upper bound on the sd, as chapter 3 says).
  tan²φ ≥ 1/(4^R − 1) is the cap bound of codes/REPORT.md, [proved, unrefereed] there, with a 1 to 2% finite-n
  correction at 1 bit that the model drops. √(E‖y‖²·E‖Δ‖²) = F_T at the stated weights: checked (Cauchy–Schwarz on
  Σ s_i²σ_{r,i}² · Σ σ_{q,i}²/s_i²).
- The prefix's exchange rate e^(−b) per nat is Shannon's rate–distortion for a Gaussian coordinate, not Theorem 2.24:
  it is an achievability statement for the error's sd, and a scalar prefix code does not reach it. Section 0 says
  "at best", which is right; "the same exchange rate as in W" is right as arithmetic.
- Category. (0.1) adds a *floor* (W_D, a lower bound on every sound code) to an *idealised achievable* block width
  (sphere bound) and compares the sum with W. Both ideals are optimistic for their own family, so "ideal" comparisons
  are bound against bound and do not say which built code wins; section 0 and C3b.2 say this in effect. Theorem 2.24
  covers sound certificates only; nothing proved bounds a randomised certificate from below outside one block in the
  class 𝓛 (CLAIMS.md C1), so the block side has no floor.
- What the model leaves out, beyond the list in section 0: (i) **nesting**. "Built" uses the codes track's one-rate
  ratios (design_lib: 1.305 at 1 bit, 1.41 at 2). The index's block 0 at 56 B is a nested code that must also serve
  3, 4 and 7 bits: codes/out_built.txt measures 1.625 times the bound at its 2-bit rise (nesting 1.157), and the dump
  test measures rms tan 0.4245, 1.65 times the bound with the stored scale. The model's first-level block is thus
  about 15% too good in tan φ. (ii) The deterministic factor 1.19 is held constant at every rate (section 0 says the
  ramp is an estimate); with ρ_c rising with rate this alone makes the built-curve optimum shift bits into the prefix
  above 1.67 bits per position (the rate at which −d ln t_built/dR = 2 ln 2; every built-curve block in `c5b.log`
  sits at 1.67), so long built-curve prefixes are a consequence of an assumption, not a finding. (iii) |y| is stored
  for every block of the index, coded or not; the model's D carries one 0.5 B norm instead, so the index's per-block
  Cauchy–Schwarz terms (Σ_j |y_j|‖Δ_j‖ ≤ ‖y‖‖Δ‖) and their 4 B of scalars are not what the model's D represents.
  (iv) A per-item scale η: the dump test measures η 1.28 to 1.52 times the model's (RESULTS 3.2 item 3).

## 2. C3 (a)

Verdict: **closed forms correct; tables reproduce; two errors in the readings.**

Checked by hand: C3a.1 (c²t < d ⇔ R > ½log₂(1 + c²/d); limit c²/(2 ln 2) = 0.721c²; sign sketch
k′ > (π/2)c²n/(c² + d) → πc²/2 = 100.5 at c = 8; ratio π ln 2 = 2.18; n ≥ 38 from d > (π/2 − 1)c²). C3a.2 (a.1) and the
flat case (4^R(d − c²) > d; B* = (n/2)log₂(d/(d − c²))). (a.2) at high rate (λ = GM·2^(−R)). (a.3) (per nat −1/k_eff
against −1/(n cos²φ)). The single-block ratio λ_j/ν = √(d/(c²t)) and ½log₂(d/(c²t)) = 2.58 at d = 767, R = 1.

Three figures recomputed: B₀ = 35.7 (c = 8, n = 96: 48·log₂(1 + 64/95)); B* = 77.6 (48·log₂(95/31)); B* = 48.8 at
c = 6.93, n = 96. The margin line: 8·1.305·1.0245/1.19 = 8.99, 9.71 at 2 bits, 11.64 at 7; right side 8.44, 9.44,
9.67, 9.75; 6.931·1.41·1.0245/1.19 = 8.41. All match `c3a.log`, which I read in full against both tables.

Errors.
- E1 (C3a.2, the bold sentence on the first randomised level). "(Its own constant is 6.93 … puts it on the winning
  side by 11%)" uses the one-rate ratio 1.41 at 2 bits. The built block 0 is nested: 1.625 (codes/out_built.txt) or
  1.65 with the stored scale (dump test T4). With 1.625·1.0245: left side 6.931·1.665/1.19 = 9.70 against 9.44, on the
  *losing* side by 3%; at c = 8, 11.2 against 9.44. Repair: "at its own constant and the measured nested code the first
  level sits 3% on the losing side of this margin; with a one-rate code it would win by 11%."
- E2 (C3a.2 table, "built" columns). The built deterministic comparator is 1.19·W at every budget, which is above the
  zero-bit bound F at small budgets; so "built" B* can fall below B₀ (c = 4: 23 against 24, 21 against 26, 22 against
  27), contradicting item 3 ("B* ≥ B₀") without comment. C3 (b) uses a ramp from 1 to 1.19 instead. Repair: use the
  same ramp, or min(F, 1.19·W), in `c3a.py`, or footnote that the c = 4 built B* are artefacts.
- Minor: "AM/GM far above √95/8 = 1.22": AM/GM is 1.41 on [0, 96) and 1.99 on [0, 224) against √d/8 = 1.87 there. The
  conclusion stands (no B* within 16 bits per position), "far" does not.

## 3. C3 (b)

Verdict: **the programme, its first-order structure and the identity (b.1) are correct; the run reproduces exactly;
the labelling as an upper bound is stated once (C3b.2) and then not carried into the readings.**

- (b.1) derivation checked: dh/dR_j = −c·W_j²t_j(1 + t_j)ln 2/(S d_j); with share_j, h/ν = n_j cos²φ_j/share_j for every
  block; summing gives h/ν = Σ n_j cos²φ_j, and h/ν = dB/d log₂(1/h). Check at 56 B: 90.2 + 69.9 + 50.3 + 30.0 = 240.4;
  slope (0.1242/0.0731 over 192 bits) 250.8 bits per halving, from the log.
- Rerun: `c3b.py 12 8 ideal` through the wrapper (`referee/rd_rerun_c3b_g12_c8_ideal.log`) is identical to the
  committed log in every figure.
- Labelling. C3b.2 says every figure is an upper bound on h*(B). Every reading that follows uses the found layouts as
  if they were h*: "where the hand-over belongs" (12 to 16 B) is an upper bound on the hand-over budget; "det / best"
  and the ideal gain 1.4 / 2.1 / 2.7 are lower bounds on the true gain; C3 (c)'s ratios "schedule over optimum" are
  lower bounds on the in-model loss; C4.1's B*(u) is an upper bound on the bits an item needs. The deterministic
  column alone is exact (closed form). None of this reverses a reading, but each should carry "at least" or "at
  most". The local search also excludes, besides what C3b.2 lists, a D that is not the tail and per-block |y|.

## 4. C3 (c) and C5: the costs and what rests on them

### 4.1 Costs traced to their sources

| Figure in C5.1 | Source | Supported? | Uncertainty |
|---|---|---|---|
| Stored bytes 6.5 … 276.3; level adds 29.5, 40.4, 69.9, 99.5 | `bd_p2c.out` build line | yes, exact | none |
| 24 B code + 5.5 B scalars at 56 B (4 |y| + κ 8 bits + tan 4 bits) | SYSTEM.md 2.1, 5 | yes | none |
| 36, 68, 93 B of code at 96/160/256 B; 12.8 B of scalars over 10 rises | group 0 rates in `bd_p2c.out` | arithmetic right | large: per level 4.4/3, 1.9/3, 6.5/4 B per rise (0.6 to 1.6), so the mean over 64 groups is not group 0's |
| 1 to 1.5 B per (item, coded block, rise); 1 B of |y| per block once | SYSTEM.md 2.1; REPORT_CONST step 4 (+0.5 B per 4-bit row) | yes | the 4-or-8-bit κ choice |
| Phase times (ms) and scalar shares 4.5 / 12.3 / 8.8 / 12.4% | SYSTEM.md 4.2 = `bd_p2c.out` | yes, exact | profiled run; stamps add 12 to 16% to the wall, mostly elsewhere |
| 8.17 item-levels per item at item 50% | `bd_p2c.out` tables line (9566 × 853.7 / 999,000) | yes | none |
| 5.4 to 6.0 rises per item, item 50% | 91% and 20% entering 56 B and 256 B from SYSTEM.md 4.1 | **mixed builds**: 4.1 is the final build (constants 6.931/7.963), the breakdown is p2c (7.413/9.086), which sends more items deeper; the inferred count is low and τ high. The dump test's 5.60 (5.73 at c = 8) at 50k is a different index but agrees | ±10% |
| τ = 1.8 to 2.0 ns (1.4 to 1.5 with |y|), text 1% 1.75 to 2.8; range 1.4 to 2.8 ns | the two lines above | arithmetic right | a factor 2; biased high by the mixed builds |
| All-in 3 to 10 ns per extra (item, block) | REPORT_BOUNDS Part 2, one sentence: "8–24% slower at 1M" | the sentence exists; which cells, which build, and the 71.3 ms are not in it | a factor 3.3, confounded: the trial changed every level's rates |
| 0.133 ns per code byte; 3.2 ns per (item, level); 104 ns per full vector; planner 7.8 + 31.3·depth ns | 18.23 ms / ~137 B; phase sums / 8.17; SYSTEM 2.2; `bd_p2c.out` plan line | yes (the 3.2 ns depends on which phases are counted: 2.4 to 3.6 ns) | ±25% for the 3.2 ns |

Errors in C5.1/C5.2:
- E3 (C5.2 item 4, circular check). "The trial's 8 to 24% is what the cost model returns for τ between 3 and 10 ns. So
  the cost side of this section agrees with the trial." The 3 to 10 ns were derived in C5.1 *from* that 8 to 24% with
  the same formula. The agreement is an identity, not a check. Repair: delete the sentence; keep that the width side
  over-predicts the trial (6 to 26% against 2%).
- E4 (C5.5 "Changes C4.3"). "each level entered … the largest component in every cell": at τ = 10 ns the scalars are
  47% and 52% against 31% and 23% for levels at the two 50% cells (`c5d.log` table as quoted). True only for τ ≤ 3 ns.

### 4.2 Is "the optimum codes one block at the first randomised level" robust?

C5.3 item 1 charges s = 2.5 B per coded block (1 B |y| + 1.5 B κ and tan). I reran C5.3's programme at s = 1 and 2 B
(`rd_c5b_s.py`, `rd_c5b_s.log`, the committed code with only the charge changed):

| s (B per coded block) | none | one | two | three | four | best |
|---|---|---|---|---|---|---|
| 1.0 | 0.2106 | 0.1755 | 0.1720 | **0.1651** | 0.1661 | three, −5.9% |
| 1.5 (`c5b.log`) | 0.2089 | 0.1755 | 0.1740 | **0.1691** | 0.1720 | three, −3.6% |
| 2.0 | 0.2073 | 0.1755 | 0.1760 | **0.1728** | 0.1787 | three, −1.5% |
| 2.5 (`c5b.log`) | 0.2056 | **0.1755** | 0.1780 | 0.1769 | 0.1853 | one, by 0.8% |

- **In bytes it is not robust.** The source's own range for s is 2.0 to 2.5 B with |y| counted, and the crossover is
  2.3 B. More to the point, the index stores |y| for all four blocks whether coded or not (SYSTEM.md 2.1, section 5),
  so the marginal stored cost of coding one more block at 56 B is κ and tan φ only, 1 to 1.5 B, where three blocks win
  by 4 to 6% in the model. (The model's D, one 0.5 B norm, is not the index's four per-block |y|, which is the
  omission (iii) of section 1.) All margins here are a few percent, below the model's known errors (block norms,
  nesting, the 7.963 used for every multi-block layout as an estimate). Statement as it can stand: "in stored bytes the
  first level is at a near-tie between one and three coded blocks; which wins depends on whether |y| is charged".
- **In time "more than one block loses" is robust**: at s ≥ 10.5 (τ ≥ 1.4 ns) two blocks are 24% or more wider than
  one (0.218 against 0.1755 at s = 11), and the trial lost on time. With the model's over-predicted gain (C5.2 item 4)
  the margin is larger still.
- **"One rather than none" is not robust**: the crossover s = 12.7 (1.7 ns) is inside the decode-alone range 1.4 to
  2.8 ns, and the "none" side uses 1.19·W at 556 to 1,044 bits, beyond the 296 at which 1.19 was measured. C5.3
  item 3 says this; the bold item 1 should say "in time, at most one".

### 4.3 C3 (c)

- Table: spot-checked against `c3b`/`c3c` widths (56 B: 0.197/0.1024 = 1.92; 0.197 is reached at 32 B, 1.75; 96 B
  1.45; 160 B 1.24; 256 B 1.40). Correct as model figures; they are lower bounds on the in-model loss (section 3).
- Item 1's measured shares: `reports/bounds3/dump_stats.log`, level 0: 0.04266 of 0.05854 (73%) and 0.07449 of 0.09422
  (79%). Correct. Withdrawn for time by C5.2; see 4.2 for bytes.
- Item 4 asked for the build's rates beside the dump's shares. The dump test now gives the 50k build's group 0 rates
  ([2,1,½,⅛], [3,3,1,¼], [6,4,2,½]). The rate condition share_j ∝ n_j cos²φ_j then predicts [0.25, 0.27, 0.36, 0.13]
  at 96 B against the dump's item shares [0.336, 0.398, 0.260, 0.005]; [0.19, 0.25, 0.39, 0.17] at 160 B against
  [0.371, 0.227, 0.385, 0.017]; [0.16, 0.21, 0.40, 0.24] at 256 B against [0.147, 0.173, 0.622, 0.058]. Block 3 sits
  at the menu's lowest rate, where the condition is only an inequality, so its mismatch says nothing. Blocks 0 and 1
  carry about twice block 2's share per unit of n cos²φ at 96 B, and block 0 about twice at 160 B. Indicative (one
  group of three, a discrete menu, ideal-curve marginals) that the built rates are off at 96 and 160 B; not a
  finding. Item 4's "I do not claim the built rates are wrong" is still the right strength.

### 4.4 C5.4, the third curve, after the referee of CLAIMS.md

What the referee established (CLAIMS.md, "What A4 is and is not", accepted): A4's interval is R8's (chapter 3,
Theorem 3.11 (ii) for one block, [proved, refereed]) with its half-width frozen at Ê = 0, so it always contains R8's;
"narrower by √(1+k)" is a statement about half-widths; A4's upper end is above the plain upper end whenever
Ê < −√k·√(1+k)/(1 + √(1+k)) (13% of draws at s = 0, 50% at s = −0.3), so A4 alone is not a better one-sided bound.

Consequences for C5.4.
- One block. Every one-block figure of C5.4 survives if it is restated for R8 instead of A4: R8's half-width is
  q tan φ√(1 + k − Ê²)/(1 + k) ≤ w* at every Ê, and R8's solution set is contained in the plain rule's on the same event
  (√(b² − v²) ≤ b), so it is one-sidedly no worse than plain. So "B₀ disappears", "B* = 0 iff d > c² for a flat
  block", the ratio table at ⅛ to 2 bits, and "one wide block −22% / −26% (−17% / −19% with dump-like norms)" stand as
  statements about R8's worst-case (Ê = 0) half-width, [proved, refereed] for the interval and [model] for the
  numbers. C5.4's tag "[carried from A4, proved there and unrefereed]" must become "R8, Theorem 3.11".
  On the built layout this is worth little: chapter 3 records ξ² = 0.092 at 56 B on the dump and 0.6% of bytes.
- Several blocks. C5.4 says "not proved" and brackets with S1 (an unproved approximation) and S2 (a sketch). A proved
  several-block interval exists and was not used: chapter 3, **Theorem 3.12 (R8 for several blocks, [proved,
  refereed])**, half-width at most c·sd/√(1 + ξ²) with one pooled ξ² = c²/Θ, Θ = Σ_j d_j/tan²φ_j. Θ is dominated by
  the finest-coded block, so the shrink is much smaller than S1's per-block k_j. Recomputed in C5.4's own model
  (`rd_c5c_r8.py`, `rd_c5c_r8.log`; for one block it reproduces w*: 0.2394):

  | First level | ξ² | R8 h/F | against built | pays below (four cells) | bytes per item (item 1%, item 50%, text 1%, text 50%) | C5.4's S1 / S2 |
  |---|---|---|---|---|---|---|
  | Three blocks at 1, ½, ⅛ | 0.27 | 0.1825 | −23.8% | 0.5 to 1.5 ns | −0.3, −2.0, −3.3, −1.1 | −43.0% / −30.0%; 1.4–3.9 / 0.7–2.1 ns |
  | Four blocks at ½, ½, ⅛, ⅛ | 0.43 | 0.1981 | −17.3% | 0.2 to 0.6 ns | −0.1, −0.2, −1.3, +1.5 | −37.9% / −20.0% |
  | Two blocks at 0.93, 0.80 (free) | 0.23 | 0.1856 | −22.5% | 1.1 to 5.8 ns | −0.4, −2.6, −3.7, −2.2 | −26.5% / −22.6%; 1.5–10.3 / 1.1–5.9 ns |
  | Two blocks at 1, ½ (menu) | 0.29 | 0.2138 | −10.7% | 0.4 to 1.4 ns | −0.1, −0.8, −1.5, −0.1 | −21.6% / −15.8% |

Figures of C5 that no longer stand: the whole S1 row and every S1 figure in the prose (−43%, −37.9%, 1.4 to 3.9 ns,
"inside the range in three cells", bytes −0.7 to −6.2, "−26.5% … below 1.5 to 10.3 ns"); the S2 row, to be replaced by
the Theorem 3.12 row above; "three blocks save 0.4 to 6 B per item" (0.3 to 3.3 B); "only at the edge for three"
(three blocks at menu rates pay only below 0.5 to 1.5 ns, under the decode-alone 1.4 to 2.8 ns in every cell but text
1%, where they reach 1.5); "four blocks pay below 0.2 to 1.5 ns" (0.2 to 0.6); "two blocks at menu rates −21.6% /
−15.8%, 0.6 to 5.2 ns" (−10.7%, 0.4 to 1.4 ns). What stands: two blocks at about 1 and ¾ bit at free rates are the
one multi-block first level that could pay in time (below 1.1 to 5.8 ns under Theorem 3.12, the same as S2 by
coincidence), more at 1% answers than at 50%; and "the cheapest use is one wide block at ¾ to 1 bit", now with R8.
The C5.6 row "under w* (unrefereed) both are zero" becomes "under R8 (Theorem 3.11), as worst-case half-widths". C5.7's
"if the one-block shrunk-centre interval is implemented … it must be refereed" becomes: implement R8 (Theorems 3.11
and 3.12, already refereed), or A5's smallest of three ends for one block; never A4 alone.
All of this still carries C5.2 item 4 (the model's gain was several times the trial's) and the nesting omission.

## 5. C4, against the dump test

Checked: the statement's algebra (E B*(U) = ∫ k(u) Pr[U < u] d log₂(1/u) is Fubini on B*(u) = ∫_u^1 k d log₂(1/u′));
the frontier table against `c3b_g12_c8_ideal.log` (every u matches); "a tenth needs 56 B, a hundredth about 215 B"
(217 by log interpolation between 208 and 256 B); C4.3's arithmetic (5.5 + 3.1 + 2.2 + 4.4 + 2.4 = 17.6; the
percentages; 26 to 34% for hybrid against deterministic). The dump test's X0 reproduces c4.py row (5) at item 50%
(130.8 against 131.7 B with this build's stored bytes), which is an independent check of the evaluation.

Defects of the test as specified in C4.4 (found by the dump tester, confirmed by reading RESULTS 1 and 2):
- It told the script to use 6.724 / 7.639 on a dump recorded at 9.33 without a replay; every record then counts as an
  entry (T3 0.957 instead of 1.000). The replay was needed.
- η was taken as Σ_j R_j of the 56 B record, the blockwise Cauchy–Schwarz bound after the 32-byte prefix, not C4.1's
  zero-bit bound. So the dump tests a law in a different normalisation from C4.1's frontier table.
- T3 used each class's own w̄, so it cannot test item 4.

Statement by statement:

| C4 statement | Verdict | Evidence |
|---|---|---|
| C4.1 item 1: survival is a step at w smeared by the estimate's error; the level's w is a property of the level | **confirmed** at the four randomised levels, per class, in the 56 B normalisation | T1 sd ln w 0.061 to 0.146; T2 half-survival at 0.990 to 1.005 of w̄; 10–90 span 0.67 to 1.13 of predicted |
| C4.1 item 2: bytes = Σ_i β(u_i) | **confirmed** for randomised levels and full vectors | T3 pooled 1.000 to 1.001, rms 5%, every (class, size) 0.99 to 1.05 |
| C4.1 item 3 / (b.1) on the built steps | **consistent** (criterion 4 met) | T5 0.92 to 1.23; the one step below 1 is within the scalars' share |
| C4.1 item 4: query type enters only through u | **refuted** in the stated form; holds per class | item and text survival differ by up to 28, 14, 20, 40 points at the same u; w̄ differs by class by 5 to 19%; within 2.4 to 6.1 points with u/w̄_class |
| C4.1 item 4: answer size enters only through u | confirmed within a class | T3 by size |
| C4.1 item 4: the visible set enters only through u | **untested** | no filtered dump |
| The frontier table w(B), B*(u), "not a power law" | **untested** (a model computation; the dump sees no level below 56 B and no other layout) | the index's normalised widths are 0.81 to 1.03 of the model's *built schedule*, which is not the frontier |
| Deterministic staircase, hand-over, absolute bytes | **untested** | RESULTS 5 |
| C4.2: Gaussian margins with one η per group | **refuted** (C4.2 said so for item queries; now located) | item 50%: factor 2.08 = margins 1.68 × det. levels 1.035 × η 1.28 × normalised widths 0.935 × estimate 1.000 |
| C4.2 cause (i): η longer near the threshold at large answers | **refuted in direction** | η falls with answer size (0.103 at 0.01%, 0.080 at 30–75%); the η term is a level shift common to both classes |
| C4.2 cause (ii): support-table cut | untested | |
| C4.2 cause (iii): item scores peaked, not Gaussian | **confirmed** | density within 0.0025 to 0.04 of t is 1.5 to 2.1 times the Gaussian's at item 50% |
| C4.2 cause (iv): model's last-level widths too narrow | **confirmed in absolute terms, wrong in mechanism** | absolute half-widths 1.23 to 1.59 times the model's, through η; the normalised widths are not too narrow (0.81 to 1.03) |
| C4.2 "smearing moves row (5) by < 0.2 B" | **confirmed** | estimate term 1.000 |
| C4.2 text fit "within 8% with one κ" | stands as a fit; the dump says the fitted κ absorbed the η term | RESULTS 3.2 item 3 |
| C4.3 the split (floor, spacing, layout, code, scalars) | stands only as a split **of the model's bytes**; it is not a split of what the index reads | the model misses a factor 2 at item 50% in two inputs outside the split |
| C4.3 "17.6 B against 18.6 B at item 1% with nothing fitted; 1.0 B not explained" | **not supported** as an attribution | the dump puts the model's η 1.16 to 1.28 low for item queries (0.094 at 1%); agreement at 1% needs a compensating margins term the dump did not measure at 1% |
| C4.3 "code distance" column | understated at the first level | the model's built curve is one-rate (1.41 at 2 bits); the built block 0 is 1.625 (codes) / 1.65 (dump T4) |

Also: the 50k index reads 69.6 B at item 10% against the model's 67.5 and the 1M index's 133.2, so C4.2's "half the
measured bytes at 10%" is not reproduced at 50k and its cause at 1M is open (RESULTS 5).

### 3a. Addendum to section 3: a wider local search

`rd_c3b_check.py` (`rd_c3b_check_fixed.log`): an independent implementation of (0.1), ideal curves, group 12, c = 8,
water-filling in closed form; 240 random layouts per budget with J = 1 to 6, cuts on a grid of 16 and a head of 0 to 48
coordinates in D, three to eight SLSQP starts each. At 32 B the best found is 0.1981 against the committed 0.1972; at
56 B 0.1022 (six blocks) against 0.1024. It re-evaluates the committed points to four digits. So the committed
frontier is not beaten by more than 0.2% by a wider search at these budgets; still a local search, still an upper
bound. (A first version of my script had a bug in the closed-form water level for a vanishing prefix, which SLSQP
exploited and which gave a spurious 0.150 at 32 B; the log is the fixed run.)

## 6. Recommendations the later results contradict

1. C3 (c) item 1, "code every block at 56 B; a sub-bit code would pay most there". Contradicted on time by C5 itself
   and by the native trial; and the codes track (REPORT.md) closed the code change: nested, table-decodable codes keep
   6 to 8%, the one-rate gains do not survive nesting, and a one-rate sub-bit code applies only to a block whose last
   rise is its only rise, which no block at 56 B is. Do not enter.
2. C3a.2, "the first randomised level is on the winning side by 11% at its own constant". Contradicted by the codes
   track's nested 1.625 and the dump's 1.65 for block 0 at 2 bits (E1).
3. C4.2 cause (i). Contradicted in direction by the dump (η falls with answer size).
4. C5.4's conclusions from S1 and S2. Superseded by the referee of CLAIMS.md and by Theorem 3.12 (section 4.4).
5. C5.7 "each variant with and without the shrunk-centre interval". Use R8 (Theorems 3.11, 3.12), not A4.
6. Not contradicted: C5.7's V1 (one wider block at the same 24 B). Two later facts bear on it in opposite directions.
   Nesting favours it: in a 1-2-3-4 schedule the 1-bit first rise has no nesting loss, while the built 2-bit rise
   carries 1.157. The dump weakens the model's gain: the uncoded term is 76% of the 56 B half-width, not the model's 84%,
   and removing that term entirely would save at most 31.5 B per item (13%) of randomised bytes at item 50% (RESULTS
   3.3), a ceiling for any change of the 56 B level.
7. C4.3's "code distance" share (20 to 29% of the modelled bytes) is a distance to the sphere bound, not a recoverable
   saving: the codes track estimates 3% of bytes for the best package that keeps the decode.

## 7. What should enter the notes

As they stand (with the tag shown):
- C3a.1: the closed forms, the B₀ table, the sign-sketch limit c²π/2 and the factor π ln 2 [model].
- C3a.2: (a.1), the flat-block threshold d > c² and B*, (a.2) and its reading, item 3 for ideal curves [model]; the
  table's ideal columns [computed].
- C3a.3: (a.3) and the single-block water-level ratio √(d/(c²t)) [model].
- C3b.1: the programme, structure items 1 to 3, and the identity (b.1) [model; derivation checked].
- C3b.2 and the C3b.3 tables, labelled upper bounds on h*(B) [computed in the model; rerun identical; a wider search
  improves them by at most 0.2%].
- C3b.4 item 1 (the layout does not depend on c once D is empty) [proved in the model]; item 4 (scope of c = 4).
- C4.1 items 1 to 3 [model], with "consistent with the dump test at 50k at the four randomised levels, per class, η the
  blockwise bound after the 32-byte prefix" [measured, dump-test RESULTS 2].
- C4.4's list of what the dump cannot test.
- C5.1: stored bytes, the 56 B split, the phase table and scalar shares [measured, `bd_p2c.out`].
- C5.2: the cost formula, the table, readings 1 to 3 [model with measured costs].
- C5.3: the table, items 3 and 4 [model].
- C5.5: the bytes split with corrected stored sizes, and "by 256 B an item has paid 11 block-rises, 15 to 110 ns,
  against 34 ns to decode its 253 B of code" [estimate].

Corrected:
- Section 0: add the omissions (nesting of the first-level block; ρ_det constant at every rate, which drives the
  built-curve optimum's prefixes above 1.67 bits; per-block |y| in the index; η).
- C3a.2: the first-level margin sentence (E1); the c = 4 built B* (E2); "far above" to "above".
- C3b.3 items 1 and 2, C3b.4 item 2, C3 (c), C4.1's frontier: "at most 16 B", "at least 1.4", "at least 1.92" etc.
  C3b.3 item 4: the long built prefixes follow from holding 1.19 constant, not from data.
- C3 (c) item 1: withdrawn for time; in bytes a near-tie. Item 4: add the check with the 50k build's rates
  (indicative of a misallocation at 96 and 160 B, not a finding).
- C4.1 item 4: per query class (u/w̄_class); the visible set untested.
- C4.2: cause (i) refuted in direction; cause (iii) confirmed; cause (iv) holds through η, not the normalised widths;
  the absolute predictions enter only as the model the dump test decomposed (×1.68 margins, ×1.28 η at item 50%).
- C4.3: a split of the model's bytes, not of the index's; drop the 1% attribution; code distance understated at 56 B.
- C4.4: the replay below the dump's constant; η is Σ R_j at 56 B; T3 per class.
- C5.1: the entrant shares come from a different build than the phases (τ biased high); state the all-in figure's
  confounding.
- C5.2 item 4: delete the circular agreement sentence.
- C5.3 item 1: "in time at most one coded block (robust over 1.4 to 10 ns); one against none undecided; in stored bytes
  a near-tie, three blocks winning by 4 to 6% when |y| is not charged".
- C5.4: one-block statements restated for R8 (Theorem 3.11); several blocks with Theorem 3.12's figures (section 4.4).
- C5.5: "largest component in every cell" only for τ ≤ 3 ns.
- C5.6 and C5.7 accordingly.

Should not enter:
- C3 (c) item 1's recommendation as written; C3a.2's "winning side by 11%".
- C4.1 item 4 as stated; C4.2 cause (i); C4.3's attribution of SYSTEM.md's 18.6 and 67 B to the five components.
- C5.2 item 4's "the cost side agrees with the trial".
- C5.3 item 1's bold sentence as written.
- Every S1 and S2 figure of C5.4 and the conclusions drawn from them ("in bytes, yes", "at the edge for three",
  "four blocks below 0.2 to 1.5 ns", "two blocks at menu rates −21.6% / −15.8%").
