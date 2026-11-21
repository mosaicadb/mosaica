# 6. Open problems

This chapter collects what chapters 1 to 5 and 7 leave open: the conjectures and named gaps of their "Open" lists,
grouped by subject, with what each lacks and where it is stated; then every Editor's query, chapter by chapter, with the
queries that comparing the chapters raised; and then the work that is in progress or owed. It adds no result.
`LEDGER.md` gives the status of every claim. An entry that a chapter has closed stays here, marked with the date and the
place that closes it, so that the list keeps its numbering of subjects.

## 6.1 Conjectures and named gaps

### Floors and lower bounds

- **The full floor** (Conjecture 2.14): settled. Theorem 2.24 proves it for Gaussian residuals and Gaussian offsets
  independent of them, [proved, refereed], and Theorem 2.25 adds Corollary 2.3's constant κ_k on the head. What it
  leaves open is in chapter 2's Open list: a floor for the real data, offsets that are not Gaussian, a code that attains
  the floor in the mean, and codes whose cells couple the head and the tail in high dimension, which nobody has tested.
  Chapter 2 §2.9a and §2.10 (`program/floors/CLAIMS.md`, "For the notes"; `program/floors/referee/referee-core.md`).
- **The floors track's extensions and the transport claims.** They have a referee's report
  (`program/floors/referee/referee-extensions.md`) and have not entered the chapters (`program/STATE.md`, "Owed").
- **The least value of κ_k − k.** That it is √e − 1, at k = 1, is [computed] over every k ≤ 5,000 and a logarithmic
  grid to 10⁷, and is not proved. Lemma 2.23 proves only κ_k ≥ k. Chapter 2 §2.9a (query 18).
- **Gaussian residuals as the worst case** (Conjecture 2.15, [conjecture]). Missing: an achievability argument at low
  rate; the high-rate case rests on a one-dimensional numerical check (chapter 2, query 10). Chapter 2 §2.10.
- **Non-Gaussian offsets** (Conjecture 2.16, [conjecture]). Missing: replacements for E[g_⊥ | g_S] = 0 and for rotation
  invariance in the Urysohn step, and the conditions under which Theorem 2.2 survives. Chapter 2 §2.10.
- **√k for a class unknown at build** (Conjecture 2.17, [conjecture]; chapter 3 Open lists it as "√k_W is
  unavoidable"). Missing: a lower bound for every subspace W, or a minimax form over decoders; only a uniformly random
  W is covered. Chapter 2 §2.10, chapter 3 Open.
- **The tail without the factor 2** (Conjecture 2.18, [conjecture]; chapter 3 Open). Missing: a proof of
  Pr[|U| ≥ s] ≤ (1 − s²)^(d_j/2) for s ≤ 1/√2, which holds on a grid for 2 ≤ d_j ≤ 5,000. Chapter 2 §2.10, chapter 3
  Open, against Proposition 3.2.
- **A floor for the real data.** None is proved: the residuals and queries are not Gaussian, queries depend on items,
  the measured spectra are diagonal models, and the shared bases and the m·e scalar are not counted. Chapter 1 §1.7 and
  Open, chapter 2 §2.9 and Open (`proofcheck/REFEREE.md`, "Headline claim").
- **Floors for randomised certificates.** Changed on 21 November 2026. Chapter 3 §3.7a now proves a lower bound for
  the confidence constant in a class 𝓛 of one-block schemes whose estimate is linear in Δ and whose half-width is a
  multiple of ‖Δ‖ (Theorem 3.43, [proved, refereed]; its first-order form, Corollary 3.44, [proved, unrefereed]). The
  bound is met in 𝓛 (Proposition 3.46), and nothing is proved outside 𝓛. R9, the earlier nearest statement, is
  corrected in chapter 3 §3.6a (see "Randomised levels and constants" below). What remains is G1 to G3 and the items
  after them in that list. Chapter 2's Open list still describes R9 as the nearest statement (comparison query 7).
- **Instance-optimality across encodings.** Theorem 2.11 fixes the stream, and the scheme's weighted Cauchy–Schwarz
  certificate is not the exact range its upper half needs. Missing: a per-query bound across encodings. Chapter 2 §2.7
  and Open.
- **Item-by-item floors** (Claim 4.9, [sketch: gap named]). Missing: floors stated row by row rather than for mean
  widths under a distribution, so that shared summaries provably cannot lift a fixed-k query off linear growth.
  Chapter 4 §4.3 and Open (`sublinear/REPORT.md` §4, §10).
- **An index-size cap.** The entropy bound (Proposition 2.13, Theorem 4.1) has no useful content unless index size is
  capped, and no bound with a cap is stated; the claim that an unbounded index attains it has no construction.
  Chapter 2 §2.8 and Open, chapter 4 §4.1.
- **Summaries chosen from bits already read.** Propositions 1.16 and 2.9 cover only choices that do not depend on the
  data, and the bound is numerically weak on this data. Chapter 1 Open, chapter 2 Open.
- **Unconditional separation.** Nothing known separates polylogarithmic from linear work for exact search with a
  near-linear index. Chapter 4 §4.2 and Open.

### Deterministic certificates and the built index

- **The defects of the build.** Chapter 1 §1.8a lists eight defects found by the referees of the kernel and of the
  sorted range and by the partition experiment, with what would settle each on a binary. They are open at commit
  8c0b316f9 and are not repeated here. Chapter 1 §1.8a and Open.
- **The floating-point allowances** of the f32 certificate (§1.3 item 1.10: e_fl, `edet`, `fl`, 4·10⁻⁶) still need
  written proofs. The f64 threshold slack, which this entry also listed, is argued since 21 November 2026 in Lemma
  1.18a [proved, unrefereed; first order in the rounding unit, query 19], and chapter 1's query 7 is settled. Parts of
  e_fl and h_fix are argued after 1.10 under conditions the code does not assert. Chapter 1 §1.3, §1.8 and Open.
- **The full-vector rounding bound** (`full_err_qn`, [sketch: gap named]). Stated in code only. Chapter 1 §1.8 and Open.
- **The two-norm bound** (Proposition 1.12, [sketch: gap named]). The step the sources assert is written only in the
  editor's reading. Chapter 1 §1.3 and Open.
- **The offset orthogonal to the code** ([sketch: gap named]). The hypothesis e ⟂ code needs a least-squares scale that
  the built tree code lacks. Chapter 1 §1.3 (query 5).
- **The built kernel's departures from the refereed design** (signed B_ℓ, the dial on the OUT side only): settled on 21
  November 2026 by `program/kernel/referee-kernel.md` §2 and §6. Both are sound by the theorem's own argument, and
  Theorem 1.19 is now stated as built, [proved, unrefereed], because the statement and proof are the referee's.
  Chapter 1 §1.8 (query 14).
- **The kernel as built awaits a second reader.** Lemmas 1.18a and 1.18b, Theorem 1.19 and Propositions 1.21′ and 1.22
  are the kernel referee's own proofs, with float constants counted by hand, and are [proved, unrefereed]. Theorem 1.19
  takes two steps as hypotheses: the IN margin of Lemma 1.18b (ii) for B_ℓ ≥ 0, and the slack e°_ℓ (query 16). The
  extension of Proposition 1.21′ from 64 to 127 units is [sketch: gap named] (query 20). Chapter 1 §1.8 and Open.
- **The randomised levels' integer form.** Lemma R and Theorem R of `reports/surge/integer-kernel.txt` §2, which the
  surge referee found correct with repairs, are assigned to chapter 3 by chapter 1 §1.8 and are still not transcribed
  in chapter 3.
- **Unrefereed short proofs.** Changed on 21 November 2026 for Lemma 1.5. The referee of the cluster cut found the
  earlier, exact-key Lemma 1.5 (= Lemma 4.10 in its annulus case) correct; it is now part (d) of a restated Lemma 1.5
  for the stored key, whose statement and proof are that referee's own and so [proved, unrefereed]. Proposition 1.4,
  Lemmas 1.5a and 1.5b and Proposition 1.6 are likewise the referee's restatements. Whether the statements the
  referee read now enter as [proved, refereed] is the PI's decision (chapter 1, query 17). Propositions 1.13, 1.15
  (= 2.8) and 1.16 (= 2.9) still need a referee. Chapter 1 §1.2 and Open.
- **The ν-sort** (Proposition 1.6). Restated by the range referee, [proved, unrefereed] (query 13, settled 21 November
  2026); still not run natively. It would remove the support table, whose anchor ratio at 10⁹ rows is unresolved (the
  16–64 anchors regime is a [conjecture], §1.2), and the support table's size on disk is unknown (query 15). Chapter 1
  Open.
- **Two measures of the zero-bit slack** (dualproto's plain ball against groups' block bound) are not reconciled.
  Chapter 1 §1.4 and Open (query 12).
- **The fit of Σ_Q.** One Σ_Q serves every group and one ridge value was tried; the text evidence for the mixed fit is
  optimistic, and why the per-group fit fails is a [conjecture]. Chapter 1 §1.4 and Open.
- **Non-unit vectors and L2.** The sorted-range lemma does not transfer, and the fixed-point similarity on stored
  integers is covered for unit f16 rows only. Chapter 1 Open.
- **Prior art for the matrix-geometric-mean metric** has been searched: Proposition 1.13 is Bhatia, Jain and Lim's
  Theorem 2 ([cited], source read by the extensions referee), and no search, quantisation or sketching paper was found
  that uses the metric ([cited, unchecked]). Chapter 1 §1.4 and Open; §6.4 below.

### Codes

- **The deterministic code's gap to the rate–distortion bound** (1.13–1.46 per coordinate, 1.1–1.25 against entropy
  coding at deep levels). Lattice and trellis codes are not built for the deterministic levels; an E8 lattice's 7% is a
  [conjecture]. Chapter 1 §1.7 and Open, chapter 2 §2.9.
- **Better codes for the randomised levels**: closed as a drop-in on 21 November 2026 (chapter 7 §7.2). With the
  schedule, the nesting and the decode kept, the best package narrows the last level by 8.2%, below the 10% bar; nested
  codes keep 6 to 8% of their one-rate gain, and E8 nested keeps nothing [measured]. What would reopen it is listed
  there: a schedule whose variance-carrying blocks rise once, a lower bound on the nesting loss, and the runs not done.
- **Entropy coding in randomised blocks** (about 3% on text) is not table-decodable. Chapter 1 Open.
- **Closing the escape code's quantiser gap** is estimated at 1.3–1.6 times in bytes, a [conjecture]. Chapter 1 §1.7.
- **A stored exact scalar along a learnt direction** (10–25% of text bytes, [conjecture]). Chapter 1 §1.3.

### Randomised levels and constants

- **R9** (the gap was a construction of N items whose failure events overlap negligibly under shared rotations):
  settled on 21 November 2026 as far as the sources go, by chapter 3 §3.6a. R9 as worded is [refuted]; its true form is
  Corollary 3.30 [proved, unrefereed]; Theorem 3.29's latitude corpus is the construction, and Theorem 3.33 [proved,
  unrefereed] shows its overlap negligible for one block, one level, the plain rule and a random shared codebook. The
  next five entries are what remains.
- **The factor N·L in Theorem 3.33** [sketch: gap named]. The pair overlap of two items at two different levels, whose
  codes are nested refinements. Chapter 3 Open.
- **Theorem 3.33 for other rules and several blocks** [sketch: gap named]. Lemma 3.31's terms relative to the rule's own
  p̄, and a several-block Lemma 3.31. Chapter 3 Open.
- **G3: the built code's pair tail** [sketch: gap named]. Pr[χ > χ₀ | F_1] at 10⁻¹¹ for the index's scalar code; the
  measurements describe the bulk only. Chapter 3 §3.7a and Open.
- **Where the union starts to be attained for small codes**, between one codeword and the 30 to 41 bits of Theorem
  3.33's sufficient sizes. Chapter 3 Open.
- **k-NN under Corollary 3.30 and Theorem 3.33.** Neither covers a decision that depends on other items' intervals.
  Chapter 3 Open.
- **G1: a lower bound outside 𝓛** [sketch: gap named; conjecture]. Missing: a trimmed Urysohn inequality. The
  conjectured optimum is R8's half-width ζ·tan θ_ν·√(1 + ξ² − v²)/(1 + ξ²). Chapter 3 §3.7a and Open.
- **G2: the first-order constant for δ fixed as d → ∞** [sketch: gap named]. Chapter 3 §3.7a and Open.
- **A lower bound for the index's certificate as built.** Several blocks, intersections across levels and with the
  deterministic bound, and the radial slack are outside 𝓛; the factor L over levels is a sketch. Chapter 3 Open.
- **An f32 build** (§3.2a) [sketch: gap named]. Encoding against a rounded matrix costs a factor E[2^Q] on δ plus
  several slacks that were not worked out; the present rule, encoding against f64, avoids it. Chapter 3 Open.
- **The `ewj` term when ‖x_j‖ ≫ ‖Δ_j‖.** The rotations referee observed, without checking, that today's allowance for
  the f32 cast sits inside the kernel's 10⁻⁶ only while ‖x_j‖ is within about ten times ‖Δ_j‖. Chapter 3 Open assigns
  it to chapter 1's deterministic terms, where it is not yet listed.
- **A drawn seed.** As built the seed is a public constant (§3.4), so no probability over ω exists outside the model.
  Whether to draw it at build is a design question, and it is the owner's (§6.3). Chapter 3 §3.4 and Open.
- **Concavity of F** (Theorem 3.22, [conjecture]; measured on random segments). A proof would make every tangent plane a
  valid set of per-block constants without a certificate. Chapter 3 §3.7 and Open.
- **T5's rule** δ(c) = ε/(K·L·D_ℓ) ([conjecture]). Missing: a lower-tail bound for the undecided count D_ℓ under one
  shared rotation that beats Markov. Chapter 3 §3.6 and Open.
- **T4's gaps** (Theorem 3.16). A seed-free bracket on tan φ at 6 bits, and the proof for Theorem 3.11's width.
  Chapter 3 §3.6 and Open.
- **A tighter certify-in constant.** The surge referee's route is unproved for ties at the boundary of V_a, several
  levels, the self-normalised width and blocks above 1,024 dimensions, and untested. Chapter 3 Open
  (`reports/surge/referee.txt` §4).
- **A global grid fitted on rotated items.** Unproved that it keeps Lemma 3.1's law; per-item scales avoid it.
  Chapter 3 §3.2 and Open.
- **Interval-verified certificates.** The certified constants are floating-point computations with margins. Chapter 3
  §3.7 and Open.
- **The Brownian count** of Proposition 3.24 ([sketch: gap named]). Chapter 3 §3.7 and Open.
- **The one-sided upper end of Theorem 3.11** ([proved, unrefereed]) is sound only with the cap at b, and the second
  pass's argument looks wrong for Ê > b. On 21 November 2026 the confidence track's referee found the same by
  simulation, which supports the cap as a condition without re-tagging the statement. Proposition 3.48's R8 part rests
  on a one-sided proof of the capped end that the sources may not contain. Chapter 3 §3.5 and §3.7a (queries 12 and 20).
- **Tolerance d > 0 under adaptive clients.** A seed-independent answer rule is needed, and its cost was not measured;
  adaptive inserts are not covered by Theorem 3.6. Chapter 3 §3.4 (query 10).
- **The time gain** of the one-sided budget and of the index's own N at 1M needs a quiet machine. Chapter 3 §3.8 and
  Open.

### Cost laws

- **Exponents beyond 1M.** Every exact-path exponent comes from 1.5 decades of nested subsamples of one sample; the 10⁹
  extrapolation is a [conjecture], and growth by unrelated content is untested. Chapter 4 §4.4 and Open.
- **Cached cluster extremes.** Their value depends on how often query directions repeat, which no log has measured.
  Chapter 4 §4.4 and Open.
- **The approximate law at 100M and beyond.** u and v on 10M–100M slices decide whether leaves matter. Chapter 4 §4.7 and
  Open (`layered.md` §8.1).
- **A floor for centre-keyed hierarchies.** `layered.md` states N^v as the best exponent such a hierarchy reaches, under
  a [measured] heading; it is an inference from fits. Chapter 4 Open (query 14).
- **The approximate exponent as recall rises.** Whether v tends to one as recall tends to 1 is not addressed. Chapter 4
  Open.
- **Dimension.** One embedding model and a PCA truncation only; the √n_eff slack has no stated model (query 17).
  Chapter 4 §4.6 and Open.
- **Cluster size for the exact path at scale.** The linear-in-N time model is fitted at 1M, and three sources recommend
  three cluster sizes (query 13). Chapter 4 §4.8 and Open.
- **The cost of a block-rise, measured.** τ_rise is inferred, 1.4 to 2.8 ns for the decode and 3 to 10 ns all-in,
  uncertain by factors of 2 and 3.3. Whether the first level should code one block or none in time waits on it.
  Chapter 4 §4.9.2 and Open.
- **A native test of the 56-byte level.** One index and one binary differing only in the first randomised level (as
  built, one wider block at the same 24 B, two blocks, the `--design-exact` layout), with exact entrant counts and time
  by phase. The cap on any change there is 31.5 B per item of N at item 50% [measured]. Chapter 4 §4.9.6 and Open.
- **The hand-over and the deterministic staircase on data.** The dump has no record below 56 B, so the hand-over, the
  law's deterministic steps and a query's absolute bytes are untested. Chapter 4 §4.9.2 and Open.
- **The rate–margin law under a visible set**, and with the zero-bit normalisation of its frontier table: both
  untested (query 22). Chapter 4 §4.9.3 and Open.
- **The two wrong inputs at 1M.** Whether the margins term (×1.68) and the scale term (×1.28) carry from the 50k dump to
  the 1M index, and why the 1M index reads 133.2 B at item 10% where the 50k index reads 69.6 B. Chapter 4 §4.9.3 and
  Open.
- **A lower bound on h*(B) over all layouts.** The frontier is a local search and an upper bound, and a continuous
  relaxation does not give a lower one. Chapter 4 §4.9 and Open.
- **The built rates at 96 and 160 B** are indicative of a misallocation and are not a finding. Chapter 4 Open.

### Approximate search

- **A stop with a guarantee.** The window stop is a heuristic (11 of 878 stops on synth4k), and the exact lazy stop
  costs too much. In part settled on 21 November 2026 (chapter 5 §5.8a; chapter 7 §7.3): a stop fed by the pooled sample
  cannot be valid for every shape of the within-cluster tail, a Cantelli stop valid for every standardised shape is
  useless in size, and the stop on expected misses was dropped. A guarantee for a restricted class of tails is still
  open. Chapter 5 §5.8 and Open, item 1.
- **The stop at high recall unfiltered.** On arxiv, iNaturalist, glove and SIFT a fixed cluster count reads fewer rows
  than the pooled stop. Chapter 5 §5.8a and Open, item 15.
- **The size of the adaptivity gap** on real data ([sketch: gap named] after Theorem 5.12). Chapter 5 §5.4 and Open.
- **Shrinkage** ([sketch: gap named]). The identification of H's α_H with shrinkage of σ̂_c is an interpretation.
  Chapter 5 §5.5.
- **ξ₀ against n′** ([sketch: gap named]). It assumes ψ ≈ 3 for both query kinds. Chapter 5 §5.6.
- **Calibrated shares on out-of-distribution queries**, and whether a learned level or features F1–F4 close part of the
  gap to the clairvoyant order. Chapter 5 Open.
- **Per-cluster visible centres**: measured in rows, not built or timed. Chapter 5 §5.8 and Open.
- **Rule 1 in time**: the build at `370580d8e` is untimed. Its width (Proposition 5.16) is a [model] whose product step
  assumes uncorrelated norms (query 5, in part answered on 21 November 2026 by an exact statement for a fixed offset,
  which leaves the product step of (5.6) unchanged). Chapter 5 §5.9a now replaces the derived width with a measured
  rule. Chapter 5 §5.9 and Open.
- **The slope of the level width** (the slope m of (5.8)). It is fitted (m = 1), not derived: the track's model gives
  0.67 to 0.80 where 0.89 to 1.21 was measured. A derivation and a fit at build are open. Chapter 5 §5.9a.3 and Open,
  item 11.
- **Filters that select on the coded or the left-out part of a row.** Lemma 5.31 fails for them, and the size of the
  effect on real attribute and region filters is not bounded. Chapter 5 §5.9a.4 and Open, item 12.
- **The plain rule's rows at equal recall** against (5.8) are not on record. Chapter 5 §5.9a.3 and Open, item 13.
- **A discriminator from the reader's pooled statistics**: whether (δ_q, ρ_q) can choose per query between the
  whole-cluster and the sample height. It is referee 2's observation, not a result. Chapter 5 Open, item 14.
- **The width's corrected shift and the narrower σ̂ under a cut on score.** The build's shift is not yet measured, and
  σ̂ from a visible sample is 13 to 21% too narrow under a quarter cut on score in the linear model. Chapter 5 §5.9a.4
  and Open, item 16.
- **The group count** G = N/(κn²): re-applied in `037653e01`, not yet timed. Chapter 5 §5.10.5 and Open.
- **Scale**: the leaf, basis and sketch rules of `layered.md` §§8.6–8.7, and sub-clusters at 8,000 rows a cluster.
  Chapter 5 Open.
- **The branching-bandit index** for nodes ([cited, unchecked]), supported only by an exhaustive check on small
  instances. Chapter 5 §5.10.2 and Open.
- **The tree law beyond 38,000 leaves**, and the regression slopes of experiment E5 (not run). Chapter 5 Open.
- **The leaf floors of the tree model** (Proposition 5.33, Theorems 5.34 and 5.35) were revised after referee 1 and are
  [proved, unrefereed] until a referee reads the revision. Chapter 5 §5.10.1a (query 24).
- **Sub-centres as coded rows** ([conjecture]): the code's error is as large as the spread it must rank. Chapter 5 Open.

### Ruled out

Chapter 7 records the proposals the program tested and did not adopt, each with what would reopen it: block norms for
out-of-distribution queries (§7.1), better codes for the randomised levels (§7.2, above), the stop on expected misses
(§7.3), alternatives to k-means for the partition (§7.4), structured rotations (§7.5), a shared dither (§7.6), the
confidence track's withdrawn statements (§7.7), and the stage 0 attack on the floor (§7.8). Its conjectures stay there:
a lattice dither in 8 to 24 dimensions (G10, §7.6), the non-existence of a pair-uniform ensemble with o(n²)
parameters (H6, §7.5), and that the harness replay will not overturn A2.6's ranking (A2.8, §7.3).

## 6.2 Editor's queries

The queries are summarised here; the full text is at the end of each chapter. A query that its chapter marks settled or
answered says so, with the date.

### Chapter 1

1. `SYSTEM.md` says the dual pair minimises "the expected width"; Proposition 1.13 minimises a product of second
   moments.
2. Σ_Q is a second-moment matrix in the fit scripts, and the reports' "covariance" should say so.
3. Candidates at a 1% answer: 22% in `SYSTEM.md` against 47–48% elsewhere; SYSTEM's figure may be mislabelled.
4. 1.36–1.42× (escape code, synthetic) and 1.42–1.86× (built code, real residuals) are different quantities.
5. "Offset orthogonal to the code" needs a least-squares scale the built code lacks.
6. Whether in-rows can lie on both sides of the undecided range. Settled on 21 November 2026 by the range referee:
   in rows and out rows can each lie at either end [model], and Lemma 1.5 now says it does not fix the ends.
7. The f64 threshold slack and |h_fix| in place of |ψ·h_fix|. Settled on 21 November 2026 by the kernel referee: the
   slack is argued in Lemma 1.18a, and the scale covers any dial.
8. Proposition 1.12 and the checks after Proposition 1.13 contain steps the editor wrote.
9. The integer kernel's "Theorem D" collides with the consolidated proofs' Theorem D.
10. `DESIGN.md`'s 1-byte norm against the 4-bit norm code as built. The kernel referee's reading agrees with
    `SYSTEM.md` (note of 21 November 2026); the point about `DESIGN.md` stands.
11. Whether one α per (query, cluster) is used at every randomised level.
12. The zero-bit comparison 0.250 → 0.190 mixes fits; like for like it is 0.250 → 0.213.
13. Proposition 1.6 in stored-basis form. Settled on 21 November 2026 by the range referee, who found it correct and
    restated it; the tag stays [proved, unrefereed]. Defects 1 and 7 of §1.8a remain against the code.
14. The built kernel's signed B_ℓ and one-sided dial. Settled on 21 November 2026 by the kernel referee: both are sound,
    and Theorem 1.19 is stated as built, [proved, unrefereed].
15. The support table's lo_c and hi_c are f32 in memory, not 2 bytes each; their size on disk is unknown.
16. Lemma 1.18b (ii) is argued only for B_ℓ < 0, and the slack e°_ℓ is not written; Theorem 1.19 takes both as
    hypotheses.
17. Whether the statements the range referee read and found correct (Lemmas 1.1, 1.2, the earlier Proposition 1.4 and
    Lemma 1.5, and Lemma 4.10) now enter as [proved, refereed]; for the PI.
18. Other chapters still described the kernel's allowances and departures as unargued. This chapter's §6.1 is revised
    on 21 November 2026, and chapter 4's Lemma 4.10 now points to Lemma 1.5.
19. Lemma 1.18a's proof is first order in ū, and the float constants and mag_u are the referee's count by hand.
20. Proposition 1.21′ is proved for 64 units; the ranges for up to 127 are the editor's, [sketch: gap named].
21. Defect 8's account of the grouping rule gives no line numbers in `fit.py` and no size for the eigenvalue floor.

### Chapter 2

1. Agent 1's computed bounds use the constant 1 instead of 1/√2 and were not recomputed.
2. "0.93 or more from 96 bytes" holds for group 12 only; group 2 gives about 0.91.
3. Every floor on a "measured spectrum" takes Σ_Q diagonal in the residual basis.
4. The 1,200–1,900× of `agent3b/REPORT.md` does not reproduce (685–1,280× unrounded).
5. Proposition 2.13's equality holds only at ε = 0, its percentage needs a factor 100, and its proof steps are the
   editor's.
6. Proposition 2.10's reduction to INDEX is not specified.
7. Lemma 2.7 needs n ≥ 3 for one step, though its conclusion holds at n = 2.
8. Conjecture 2.14's hypotheses, which the source does not state. Answered for the conjecture as the chapter states it:
   Theorem 2.24 states its setting in full (§2.9a), that setting contains the hypotheses the chapter assumed (those of
   Corollary 2.3), and the core referee reads the conjecture with the same ones.
9. The box ∩ ellipsoid gain is 3–14% of width in one source and 0.3% of bytes in another.
10. The high-rate case of Conjecture 2.15 rests on a one-dimensional numerical check.
11. The referee's "worst 1.13" does not say which code was measured.
12. `floor2.py` gives a model upper bound with the escape code, no bases counted, and identical n1024 rows.
13. Agent 3a's T5 lacked the independence of items for shared bits.
14. "Hold the offset law fixed" in Proposition 2.8 is read as one law in every cell, independent of x.
15. The [proved, refereed] tag of the random-W identity rests on a referee remark.
16. Which of three points are the referee's "two small fixes".
17. `floor2.py`'s "before" differs from the bytes-read "before" of `quant/REPORT.md`.
18. That κ_k − k is least at k = 1 (√e − 1) is stated without a tag or proof. It is [computed] over every k ≤ 5,000
    and a grid to 10⁷, and not proved; Lemma 2.23's tag covers κ_k ≥ k only.

### Chapter 3

1. `REFEREE.md` does not name Lemma 8, Proposition 11 or Proposition 12; nothing records a referee reading
   Proposition 11's proof.
2. The exact Beta tail is used without proof anywhere.
3. "Shrinks c by" in the source means "multiplies c by".
4. T1's refereed tag rests on referee reports of its application, not of `bounds2.txt`'s text.
5. T4 step (ii) writes c_min where c is direct; the result is unaffected.
6. The second pass's claim that block rotations lose part of the exact-tail gain is refuted by the third.
7. The level-0 report tags its self-normalised backstop proved; the referee refuted it.
8. "Would double k-NN's failure bound" holds for `knn2d`, not for Theorem 3.10's class.
9. The native certifier gives 8.775 where the report certifies 8.774.
10. Agent 3a's claim that Theorem 5 covers adaptive inserts is unproved.
11. Which levels code three blocks at 1M needs the build log.
12. The one-sided upper end of Theorem B is claimed with no cap; the referee's counter-example needs the cap at b. On
    21 November 2026 the confidence track's referee found the same by simulation; bounds2's tag stays open for the PI.
13. Theorem 3.12's remark on a second event and Proposition 3.48's one event for one block are consistent only if the
    remark is about several blocks, which it does not say.
14. The tags of §3.7a: results the referee read unchanged are tagged [proved, refereed] against the track's blanket
    [proved, unrefereed]; audited claim by claim on 21 November 2026.
15. The same choice for §3.2a and §3.4 (Propositions 3.25, 3.27, 3.28 and Theorem 3.26); audited on 21 November 2026.
16. §3.2's refereed bullet reads the per-event δ as a promise about seeds; §3.4 now states the correction.
17. The rotations track's int16 shares: the referee reproduced three of four depths, and its figures supersede the
    track's for the kernel's slot.
18. Lemma 3.49's sum of the two tails at d = 255: 1.91 (theorist) against 1.94 (referee).
19. Lemma 3.49's last figure: 0.5% as quoted, 0.93% by the track's formula, 0.73% by direct evaluation.
20. Proposition 3.48 uses Theorem 3.11's repaired upper end on a one-sided event, while §3.5 states it on the two-sided
    event; whether a one-sided proof of the capped end exists is for the PI.

### Chapter 4

1. Proposition 4.2's degenerate vectors and the unspecified η′.
2. Lemma 4.4's lower bound needs intervals nested across stages.
3. Proposition 4.5's shell includes the k matches, and Lemma 4.4's does not.
4. Corollary 4.6's Gaussian case holds only to leading order, which the statement now says, and its proof lines are
   the editor's; the PI may prefer [sketch: gap named] for that case.
5. Theorem 4.7 is about the population share, not the sample share.
6. The source's equality in Theorem 4.1 drops the h₂(ε) + εN terms.
7. Proposition 4.3's model is not spelled out.
8. Lemma 4.10's tag was assigned by the editor.
9. How "argued", "derived" and "estimated" were mapped to tags.
10. Hashing exponent ranges disagree within `sublinear/REPORT.md`.
11. The 3 sd shell exponent disagrees within §5.4 of the same report.
12. Different row counts and machines across the sources.
13. Three cluster-size rules are not reconciled.
14. A floor stated under a [measured] heading in `layered.md`.
15. Where the overshoot figure of Proposition 4.8 comes from.
16. Which baseline is 1,200–1,900× above the entropy bound.
17. The √n_eff slack is "argued" with no model; [sketch: gap named] may fit better.
18. `layered.md` §8.1's summary of the leading term conflicts with its glove rows.
19. The text exponent of rows touched in |V| is 0.96 in one place and 0.97 in another.
20. Pestov's status, [cited] from an abstract.
21. DESIGN-CLAIMS's [estimate] labels in §4.9 enter as [model]; the design rule of §4.9.4 carries no tag.
22. Two normalisations of η_i: the dump confirms the law in one, and the frontier table is untested in the other.
23. Builds, constants and stored sizes differ across the sources of §4.9.
24. §4.4's entering shares and fixed costs come from different builds, so a per-item cost from the two is biased.
25. "4 to 6%" against the referee's own table, 5.9% at 1 B and 3.6% at 1.5 B.
26. The source puts the first randomised level 11% on the winning side and the referee 3% on the losing side; §4.9.2
    follows the referee.
27. The second factor in "131.7 × 1.68 × 1.24" is 1.28 for η alone; the source does not say which it meant.

### Chapter 5

1. No section "the theory of the order" exists in `PLAN.md`.
2. How the visit-order notes' tags were mapped.
3. The sum rule of Proposition 5.6 is exact only up to a term exponentially small in N.
4. Theorem 5.13 needs o = 0, and Proposition 5.9 an even n_c.
5. Proposition 5.16's product step assumes uncorrelated norms; ρ is 25% above 1/√n on yahoo. In part answered on 21
   November 2026: A3.1 states the ratio exactly for a fixed offset, and the product step of (5.6) is unchanged.
6. Proposition 5.17's losses recompute to 0.67 / 1.3 / 17.3%, not 0.7 / 1.4 / 19%.
7. The built rule 1 reads the randomised levels and uses a median ρ per level. Closed on 21 November 2026 for the
   median per level, which the track read in the code; the point about reading the randomised levels is unchanged.
8. Two findings about τ_k look opposed.
9. Superseded recommendations of `hierarchy/theory.md` §8.
10. `approx-design.txt`'s recall bound cites a report outside the chapter's sources.
11. Whether rule 1's headline bytes used the derived or the measured ρ.
12. ξ′ = 0.4 is within 0.006 of the best in one source and +0.007 in another.
13. The group setup times have no traced source.
14. The idealised replay and the real reader disagree on LAION, and the sources do not say which difference reverses
    the result.
15. "5 to 25%" against arxiv's 27% fewer rows for the fixed count.
16. ϑ in the whole-cluster rule is read as 1 − σ̂²/σ_c(q)²; the formula is not written in the record.
17. "18 cases" against a list of 17.
18. Which shift the measurements used: settled by the log (mean(e), before the correction); no commit and no path for
    `width_checks.py` are recorded.
19. LAION's derived ρ: 15% above the measured in §5.9, 0.94 of it in §5.9a.1; different statistics, both reported.
20. "A4.4's identity" in the log is A4.7's in the claims.
21. Lemma 5.31's converse holds almost everywhere and needs P(Vis | s) > 0; stated beside the proof.
22. The proofs of the dropped stop are summarised, not transcribed; chapter 7 §7.3 carries the summary.
23. Which build is the stop `f528836`.
24. Proposition 5.33 and Theorems 5.34 and 5.35 stay [proved, unrefereed] until a referee reads the revision.
25. "The best rule for each case" in the 18-case measurement is not named.
26. Proposition 5.30's last clause needs an increasing linear function for the slope to be 1.

### Chapter 7

1. Section 1.8a had no heading when 7.4 cited it. Chapter 1 now has §1.8a, and its defect 8 is the one 7.4 names.
2. The brief's "recomputed rates in E7": the rotations claims have no E7; 7.5 gives the referee's B4 rates.
3. [estimate] is outside the tag scheme and is carried as given.
4. "Free" for the 24 encoder scales is withdrawn on the PI's note; the measurement stands.
5. `confidence/DESIGN-CLAIMS.md` C5.3 item 1 still reads "the optimum codes one block"; 7.7 gives the referee's
   restatement.
6. 7.6's "1.3 to 1.8" is G7's table; the claims' prose gives 1.3 to 1.45.

### Queries from comparing the chapters

1. **One remark, two tags.** The claim that an unbounded index over a finite query set attains the entropy bound is
   [sketch: gap named] in chapter 2 §2.8 (2.q) and [model] in chapter 4 §4.1 (4.c), from the same source
   (`agent3b/REPORT.md` §1). The PI should choose one.
2. **The same narrowed comparison, tagged once.** The referee's narrowed reading of agent 1's headline (escape code
   1.36–1.42×) is [computed] in chapter 1 §1.7 and untagged in chapter 2 §2.9.
3. **Tags outside the scheme**, listed after the table in `LEDGER.md`, need mapping: [proved by example; measured],
   [refuted as a constant], [measured and argued], [refuted as a saving], [measured on a proxy], [measured, share
   inferred] and [model, measured]. Agent 1's synthetic Monte Carlo (2.r) is [measured] on synthetic data, and the grid
   check of Conjecture 2.18 is [computed] though it evaluates no proved inequality. [estimate], carried in chapters 4
   and 7 from the program's files, is a further one (chapter 4, query 21; chapter 7, query 3).
4. **A result assigned to no chapter.** Lemma R and Theorem R of `reports/surge/integer-kernel.txt` §2 (the randomised
   levels' integer form) are assigned to chapter 3 by chapter 1, and chapter 3 does not state them.
5. **Proposition 1.7 and Lemma 2.7** state the same result with and without the condition n ≥ 3; chapter 2's query 7
   says the conclusion holds at n = 2.
6. **Proposition 2.13 and Theorem 4.1** state the same bound under different names, with the same open points (chapter
   2 queries 4 and 5, chapter 4 queries 6 and 16); the recomputed ratio is 685–1,280× unrounded in chapter 2 and about
   670–1,280× rounded in chapter 4.
7. **R9 in chapter 2.** Chapter 2's Open list ("Randomised certificates") still calls chapter 3's R9 the nearest
   statement of a lower bound. Chapter 3 §3.6a now marks R9 as worded [refuted], and §3.7a proves a lower bound in the
   class 𝓛. Chapter 2's entry should be revised by its editor.
8. **Lemma 2.23 in the ledger.** `LEDGER.md` gives one tag, [proved, refereed], to a row that holds both κ_k ≥ k and the
   least value √e − 1 at k = 1; the second is [computed] only (chapter 2, query 18).
9. **The `ewj` term.** Chapter 3's Open list assigns the f32 cast's allowance when ‖x_j‖ ≫ ‖Δ_j‖ to chapter 1's
   deterministic terms, and chapter 1 does not list it.

## 6.3 In progress and owed

This section is taken from `program/STATE.md` as of 21 November 2026, the end of the first pass. Nothing in it is a
result.

**Owed to the notes.**

| Item | Where it stands |
|---|---|
| A second reader for the referee-written proofs of chapter 1 (§1.2 and §1.8) | [proved, unrefereed]; no second reader |
| A second reader for the twelve unrefereed results of chapter 3 among 3.25 to 3.50 | [proved, unrefereed] |
| A referee for the revised tree model and leaf floors of chapter 5 §5.10.1a | revised after referee 1, not re-read |
| The Editor's queries opened in the first pass | chapter 1, 15 to 21; chapter 3, 13 to 20; chapter 4, 21 to 27; chapter 5, 14 to 26 |
| Floors: the extensions and the transport claims | a referee's report (`floors/referee/referee-extensions.md`); not in the chapters |
| Confidence: G1, G2, G3, the factor L, the union for other rules and for several blocks, k-NN | open (§6.1) |
| Level width: a derivation of the slope; filters that select on the coded part of a row; the plain rule at equal rows | open (§6.1) |

**Needs the machine or a binary.**

| Item | Purpose |
|---|---|
| The build defects of chapter 1 §1.8a: a count at build of rows with \|x·m̂\| > 1 + DA, the range referee's two unit tests, and the kernel tests on AVX-512 and on AVX2 alone | to settle the defects on a binary, with the implementing session |
| The first randomised level: one wider block at the same 24 B, two blocks, and the `--design-exact` layout with phases recorded | to measure the cost of a scalar read rather than infer it (chapter 4 §4.9) |
| A fit at text share 0.2 against the shipped mixed fit (floors B4d) | the text share of the single fit |
| The 24-scale encoder | low priority: about one point of the last level's width, at an encode loop four times longer (chapter 7 §7.2) |

**Decisions that are the owner's.** How to proceed on the build defects, the key clamp and the root in the cut first;
whether the seed is drawn at build and stored, where as built it is a public constant; a basis per query family, or a
lower text share in the single fit; and whether to redesign the first randomised level.

## 6.4 Prior art found by the program's librarians (21 November 2026)

These points postdate the chapters and correct what chapters 1, 2 and 5 say about novelty. The reports, with how each
source was read, are `program/library/theory.md` and `program/library/codes-and-search.md`. Each is [cited, unchecked]
here until the track that depends on it has read the source itself.

- **Proposition 1.13 (the geometric-mean metric) is published.** Bhatia, Jain and Lim (Expo. Math. 2019), Theorem 2,
  eq. (16): F(A, B) = min over X > 0 of √(tr(AX)·tr(BX⁻¹)), with minimiser A⁻¹#B. This attribution is [cited], with
  the source read: the extensions referee confirmed it, equation numbers and minimiser included, from the arXiv TeX
  source (`program/floors/referee/referee-extensions.md`, B1; chapter 1 §1.4). Zadeh, Hosseini and Sra (ICML 2016)
  solve the same problem for metric learning. No search, quantisation or sketching paper was found that uses it.
- **The full-floor formula (Conjecture 2.14, Theorem 2.24) was stated before, and its converse at finite n is new as
  far as the program's search goes.** Wu, Wang and Flierl (arXiv:2001.07941) state the formula for Σ_Q = Σ_R without a
  proved converse. The converses that are proved are isotropic: asymptotic in Ingber, Courtade and Weissman (IEEE Trans.
  Inf. Theory 61, 2015), and at finite blocklength for the probability of the answer "maybe" in Steiner, Dempfle,
  Ingber and Weissman (arXiv:1404.5173). Theorem 2.24 is the first proof of the converse at finite n, for covariances
  that need not commute, in the mean-width form, as far as the program's search goes. Tightness holds in probability by
  Ingber, Courtade and Weissman's scheme and, in the mean, only by computed codes. This point is [cited], with the
  sources read: the extensions referee read the arXiv sources of Wu, Wang and Flierl and of Ingber, Courtade and
  Weissman, and the librarian read that of Steiner, Dempfle, Ingber and Weissman
  (`program/floors/referee/referee-extensions.md` P1–P2, `program/library/floors-novelty.md` §3). Chapter 2 §2.9a and
  §2.11.
- **Codes beyond the scalar gap are published as a mechanism.** BlockQuant (arXiv:2605.19972), QuIP#, NestQuant and QTIP
  code rotated vectors with block-spherical, lattice and trellis codes. Nested codes read prefix by prefix, the loss
  from nesting them, and a per-item angle bound inside a certificate were not found.
- **The tree model of chapter 5 (Model T) is a generalised random energy model.** Addario-Berry and Maillard
  (arXiv:1810.05129), Theorem 1.1, give an algorithmic threshold for its continuous version. No paper was found that
  models cluster search this way.
- **Per-query stops on an estimated recall are published** (PAC-NN, LAET, Auncel, Quake, DARTH, ConANN, CaST). Chapter 5
  follows `theory.md` in calling Quake, LAET and DARTH instances of the global threshold on p_c; they stop on a
  per-query recall target instead.
- **Smaller corrections.** Bruch, Krishnan and Nardini's multiplier 3 is a Cantelli bound at the 0.9 quantile, not a
  Gaussian tail. For the matroid form of Weitzman's rule, Singla (2018) has the theorem and Kleinberg, Waggoner and Weyl
  a footnote. Only E8's normalised second moment is exact; the Barnes–Wall and Leech values are numerical.
