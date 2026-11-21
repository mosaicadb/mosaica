# 6. Open problems

This chapter collects what chapters 1 to 5 leave open: the conjectures and named gaps of their "Open" lists, grouped
by subject, with what each lacks and where it is stated; then every Editor's query, chapter by chapter, with the
queries that comparing the chapters raised; and then the work the theory program has under way. It adds no result.
`LEDGER.md` gives the status of every claim.

## 6.1 Conjectures and named gaps

### Floors and lower bounds

- **The full floor** (Conjecture 2.14, [conjecture]). Missing: one argument that gives the head term k·√θ and the tail
  Σ_{i>k} √γᵢ together, without Theorem 2.6's factor 1/√e, its extra bit and the concentration penalty. The best proved
  bound is 0.70 of the conjectured floor at 0 bytes. Stated in chapter 2 §2.10 and Open
  (`proofcheck/CONSOLIDATED_PROOFS.md` §6); its hypotheses are the editor's reading (chapter 2, query 8).
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
- **Floors for randomised certificates.** Theorems 2.2–2.6 cover deterministic sound certificates only, and no lower
  bound says what a confidence constant must cost. R9 is the nearest statement and is [sketch: gap named]; the gap is a
  construction of N items whose failure events overlap negligibly under the index's shared rotations. Chapter 2 Open,
  chapter 3 §3.7 and Open (`reports/bounds3/REPORT.txt` R9).
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

- **The floating-point allowances** of the certificate as built (§1.3 item 1.10: e_fl, `edet`, `fl`, 4·10⁻⁶) and the
  f64 threshold slack (chapter 1, query 7). They are argued in code comments and checked in verify mode, and no written
  source proves them. Chapter 1 §1.3, §1.8 and Open.
- **The full-vector rounding bound** (`full_err_qn`, [sketch: gap named]). Stated in code only. Chapter 1 §1.8 and Open.
- **The two-norm bound** (Proposition 1.12, [sketch: gap named]). The step the sources assert is written only in the
  editor's reading. Chapter 1 §1.3 and Open.
- **The offset orthogonal to the code** ([sketch: gap named]). The hypothesis e ⟂ code needs a least-squares scale that
  the built tree code lacks. Chapter 1 §1.3 (query 5).
- **The built kernel's departures from the refereed design** (signed B_ℓ, the dial on the OUT side only). Neither is in
  a written source or refereed. Chapter 1 query 14.
- **The randomised levels' integer form.** Lemma R and Theorem R of `reports/surge/integer-kernel.txt` §2, which the
  surge referee found correct with repairs, are assigned to chapter 3 by chapter 1 §1.8 and are not transcribed in
  chapter 3.
- **Unrefereed short proofs.** Lemma 1.5 (= Lemma 4.10), Propositions 1.13, 1.15 (= 2.8) and 1.16 (= 2.9) need a
  referee. Chapter 1 Open.
- **The ν-sort** (Proposition 1.6). Implemented as `NuCut`, not run natively; it would remove the support table, whose
  anchor ratio at 10⁹ rows is unresolved (the 16–64 anchors regime is a [conjecture], §1.2). Chapter 1 Open.
- **Two measures of the zero-bit slack** (dualproto's plain ball against groups' block bound) are not reconciled.
  Chapter 1 §1.4 and Open (query 12).
- **The fit of Σ_Q.** One Σ_Q serves every group and one ridge value was tried; the text evidence for the mixed fit is
  optimistic, and why the per-group fit fails is a [conjecture]. Chapter 1 §1.4 and Open.
- **Non-unit vectors and L2.** The sorted-range lemma does not transfer, and the fixed-point similarity on stored
  integers is covered for unit f16 rows only. Chapter 1 Open.
- **Prior art for the matrix-geometric-mean metric** has not been searched. Chapter 1 Open.

### Codes

- **The deterministic code's gap to the rate–distortion bound** (1.13–1.46 per coordinate, 1.1–1.25 against entropy
  coding at deep levels). Lattice and trellis codes are not built; an E8 lattice's 7% is a [conjecture]. Chapter 1
  §1.7 and Open, chapter 2 §2.9.
- **Entropy coding in randomised blocks** (about 3% on text) is not table-decodable. Chapter 1 Open.
- **Closing the escape code's quantiser gap** is estimated at 1.3–1.6 times in bytes, a [conjecture]. Chapter 1 §1.7.
- **A stored exact scalar along a learnt direction** (10–25% of text bytes, [conjecture]). Chapter 1 §1.3.

### Randomised levels and constants

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
  pass's argument looks wrong for Ê > b. Chapter 3 §3.5 (query 12).
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

### Approximate search

- **A stop with a guarantee.** The window stop is a heuristic (11 of 878 stops on synth4k), and the exact lazy stop
  costs too much. Chapter 5 §5.8 and Open.
- **The size of the adaptivity gap** on real data ([sketch: gap named] after Theorem 5.12). Chapter 5 §5.4 and Open.
- **Shrinkage** ([sketch: gap named]). The identification of H's α_H with shrinkage of σ̂_c is an interpretation.
  Chapter 5 §5.5.
- **ξ₀ against n′** ([sketch: gap named]). It assumes ψ ≈ 3 for both query kinds. Chapter 5 §5.6.
- **Calibrated shares on out-of-distribution queries**, and whether a learned level or features F1–F4 close part of the
  gap to the clairvoyant order. Chapter 5 Open.
- **Per-cluster visible centres**: measured in rows, not built or timed. Chapter 5 §5.8 and Open.
- **Rule 1 in time**: the build at `370580d8e` is untimed. Its width (Proposition 5.16) is a [model] whose product step
  assumes uncorrelated norms (query 5). Chapter 5 §5.9 and Open.
- **The group count** G = N/(κn²): re-applied in `037653e01`, not yet timed. Chapter 5 §5.10.5 and Open.
- **Scale**: the leaf, basis and sketch rules of `layered.md` §§8.6–8.7, and sub-clusters at 8,000 rows a cluster.
  Chapter 5 Open.
- **The branching-bandit index** for nodes ([cited, unchecked]), supported only by an exhaustive check on small
  instances. Chapter 5 §5.10.2 and Open.
- **The tree law beyond 38,000 leaves**, and the regression slopes of experiment E5 (not run). Chapter 5 Open.
- **Sub-centres as coded rows** ([conjecture]): the code's error is as large as the spread it must rank. Chapter 5 Open.

## 6.2 Editor's queries

The queries are summarised here; the full text is at the end of each chapter.

### Chapter 1

1. `SYSTEM.md` says the dual pair minimises "the expected width"; Proposition 1.13 minimises a product of second
   moments.
2. Σ_Q is a second-moment matrix in the fit scripts, and the reports' "covariance" should say so.
3. Candidates at a 1% answer: 22% in `SYSTEM.md` against 47–48% elsewhere; SYSTEM's figure may be mislabelled.
4. 1.36–1.42× (escape code, synthetic) and 1.42–1.86× (built code, real residuals) are different quantities.
5. "Offset orthogonal to the code" needs a least-squares scale the built code lacks.
6. Whether in-rows can lie on both sides of the undecided range is unchecked.
7. The f32 threshold slack is covered by `int_thresholds` in f64, but |h_fix| replaces |ψ·h_fix| and no source argues
   the f64 slack.
8. Proposition 1.12 and the checks after Proposition 1.13 contain steps the editor wrote.
9. The integer kernel's "Theorem D" collides with the consolidated proofs' Theorem D.
10. `DESIGN.md`'s 1-byte norm against the 4-bit norm code as built.
11. Whether one α per (query, cluster) is used at every randomised level.
12. The zero-bit comparison 0.250 → 0.190 mixes fits; like for like it is 0.250 → 0.213.
13. Proposition 1.6 is restated in stored-basis form, and no source writes the proof.
14. The built kernel uses the signed B_ℓ and applies ψ on the OUT side only; neither is refereed.

### Chapter 2

1. Agent 1's computed bounds use the constant 1 instead of 1/√2 and were not recomputed.
2. "0.93 or more from 96 bytes" holds for group 12 only; group 2 gives about 0.91.
3. Every floor on a "measured spectrum" takes Σ_Q diagonal in the residual basis.
4. The 1,200–1,900× of `agent3b/REPORT.md` does not reproduce (685–1,280× unrounded).
5. Proposition 2.13's equality holds only at ε = 0, its percentage needs a factor 100, and its proof steps are the
   editor's.
6. Proposition 2.10's reduction to INDEX is not specified.
7. Lemma 2.7 needs n ≥ 3 for one step, though its conclusion holds at n = 2.
8. Conjecture 2.14's hypotheses are not stated in the source.
9. The box ∩ ellipsoid gain is 3–14% of width in one source and 0.3% of bytes in another.
10. The high-rate case of Conjecture 2.15 rests on a one-dimensional numerical check.
11. The referee's "worst 1.13" does not say which code was measured.
12. `floor2.py` gives a model upper bound with the escape code, no bases counted, and identical n1024 rows.
13. Agent 3a's T5 lacked the independence of items for shared bits.
14. "Hold the offset law fixed" in Proposition 2.8 is read as one law in every cell, independent of x.
15. The [proved, refereed] tag of the random-W identity rests on a referee remark.
16. Which of three points are the referee's "two small fixes".
17. `floor2.py`'s "before" differs from the bytes-read "before" of `quant/REPORT.md`.

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
12. The one-sided upper end of Theorem B is claimed with no cap; the referee's counter-example needs the cap at b.

### Chapter 4

1. Proposition 4.2's degenerate vectors and the unspecified η′.
2. Lemma 4.4's lower bound needs intervals nested across stages.
3. Proposition 4.5's shell includes the k matches, and Lemma 4.4's does not.
4. Corollary 4.6's Gaussian case holds only to leading order, and its proof lines are the editor's.
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

### Chapter 5

1. No section "the theory of the order" exists in `PLAN.md`.
2. How the visit-order notes' tags were mapped.
3. The sum rule of Proposition 5.6 is exact only up to a term exponentially small in N.
4. Theorem 5.13 needs o = 0, and Proposition 5.9 an even n_c.
5. Proposition 5.16's product step assumes uncorrelated norms; ρ is 25% above 1/√n on yahoo.
6. Proposition 5.17's losses recompute to 0.67 / 1.3 / 17.3%, not 0.7 / 1.4 / 19%.
7. The built rule 1 reads the randomised levels and uses a median ρ per level.
8. Two findings about τ_k look opposed.
9. Superseded recommendations of `hierarchy/theory.md` §8.
10. `approx-design.txt`'s recall bound cites a report outside the chapter's sources.
11. Whether rule 1's headline bytes used the derived or the measured ρ.
12. ξ′ = 0.4 is within 0.006 of the best in one source and +0.007 in another.
13. The group setup times have no traced source.

### Queries from comparing the chapters

1. **One remark, two tags.** The claim that an unbounded index over a finite query set attains the entropy bound is
   [sketch: gap named] in chapter 2 §2.8 (2.q) and [model] in chapter 4 §4.1 (4.c), from the same source
   (`agent3b/REPORT.md` §1). The PI should choose one.
2. **The same narrowed comparison, tagged once.** The referee's narrowed reading of agent 1's headline (escape code
   1.36–1.42×) is [computed] in chapter 1 §1.7 and untagged in chapter 2 §2.9.
3. **Tags outside the scheme**, listed after the table in `LEDGER.md`, need mapping: [proved by example; measured],
   [refuted as a constant], [measured and argued], [refuted as a saving], [measured on a proxy], [measured, share
   inferred] and [model, measured]. Agent 1's synthetic Monte Carlo (2.r) is [measured] on synthetic data, and the grid
   check of Conjecture 2.18 is [computed] though it evaluates no proved inequality.
4. **A result assigned to no chapter.** Lemma R and Theorem R of `reports/surge/integer-kernel.txt` §2 (the randomised
   levels' integer form) are assigned to chapter 3 by chapter 1, and chapter 3 does not state them.
5. **Proposition 1.7 and Lemma 2.7** state the same result with and without the condition n ≥ 3; chapter 2's query 7
   says the conclusion holds at n = 2.
6. **Proposition 2.13 and Theorem 4.1** state the same bound under different names, with the same open points (chapter
   2 queries 4 and 5, chapter 4 queries 6 and 16); the recomputed ratio is 685–1,280× unrounded in chapter 2 and about
   670–1,280× rounded in chapter 4.

## 6.3 In progress

The theory program started on 21 November 2026. Its work is in the notebooks below, which are not part of the notes
until refereed; this section says what each track and check is for and where its notebook is, and nothing about results.

| Track or check | Subject | Notebook directory |
|---|---|---|
| floors | one floor giving the head and the tail of Conjecture 2.14 together; certificates as optimal transport | `program/floors/` |
| confidence | a lower bound for randomised certificates (what the confidence constant must cost), the gap in R9, the hand-over from deterministic to randomised levels, a rate–margin law for bytes read | `program/confidence/` |
| codes | closing the gap between the randomised levels' code and the sphere's rate–distortion bound with table-decodable codes | `program/codes/` |
| approximate | a random-energy model of the read, a stop on expected misses, a per-query spread at the levels | `program/approximate/` |
| stage 0: floor check | a numerical attack on the floor conjecture | `program/stage0-floor-check/` |
| stage 0: block norms | block norms on out-of-distribution queries | `program/stage0-block-norms/` |
| stage 0: library | a check of prior art and of every citation given from memory | `program/library/` |

## 6.4 Prior art found by the program's librarians (21 November 2026)

These points postdate the chapters and correct what chapters 1, 2 and 5 say about novelty. The reports, with how each
source was read, are `program/library/theory.md` and `program/library/codes-and-search.md`. Each is [cited, unchecked]
here until the track that depends on it has read the source itself.

- **Proposition 1.13 (the geometric-mean metric) is published.** Bhatia, Jain and Lim (Expo. Math. 2019), Theorem 2,
  eq. (16): F(A, B) = min over X > 0 of √(tr(AX)·tr(BX⁻¹)), with minimiser A⁻¹#B. Zadeh, Hosseini and Sra (ICML 2016)
  solve the same problem for metric learning. No search, quantisation or sketching paper was found that uses it.
- **The full-floor conjecture (Conjecture 2.14) appears to be published in an asymptotic, equal-covariance form.**
  Ingber, Courtade and Weissman (IEEE Trans. Inf. Theory 2015) and Wu, Wang and Flierl (arXiv:2001.07941) study
  compressed Gaussian data answering similarity queries with no false negatives; the latter's Theorem 1 is read as rate
  Σ log⁺(ξ/τ) for a certified level Σ min(ξ, τ). The earlier survey's "apparently new" verdict on the floors missed this
  line. A finite-n statement and unequal covariances were not found.
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
