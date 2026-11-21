# Approximate track: notebook

Working log of the approximate-mode theorist. The claims, with proofs and tags, are in `CLAIMS.md`; the replay for
A2 is in `REPLAY-SPEC.md`. This file says how each was reached and what failed.

## How to resume

- Everything of this track is under `notes/program/approximate/`: `scripts/` (numpy only), `out/` (their outputs;
  `out/tables.md` is the one to read), and the three documents.
- Run any script as `nice -n 10 taskset -c 7 env OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 scripts/X.py …`.
  `replay_np.py DATASET OUT.json` takes 3 to 6 minutes a dataset on one core; `tables.py > out/tables.md` rebuilds
  the tables from `out/replay_{yahoo,laion,glove}.json`.
- Data read, never written: the suite's fits and prep under `/home/joe/code/vexact-data/suite/` (`fit/<ds>/Xs.npy`,
  `cstart`, `grp`, `perm`, `V`, `lam`, `profile`, `mq8`, `msc`; `prep/<ds>/queries.npy`, `vis_<set>.npy`,
  `knn_thr*.npy`). The notes read are in `/home/joe/code/vexact-visitorder` and `/home/joe/code/vexact-sc`.
- Not done, in order of value: section 8.

---

## 2026-11-21, entry 1. What I read of the project's notes

Read in full: `notes/README.md`, `notes/00-notation.md`; in `vexact-visitorder/bench/suite/notes/visit-order/`:
`theory.md`, `README.md`, `HANDOVER.md`, `hierarchy/theory.md`, `hierarchy/layered.md` sections 0 to 4 and 8.1,
`PLAN.md` from the glove regression to the self-correcting order as built, `hierarchy/theory/model.py` and
`model_out.txt`, section C of the three `out_*.txt`; in `vexact-sc`: `results/region25-order/README.md`. In the
code, read-only: the head comment and `knn` of `rust/src/v2/adaptive.rs` at 370580d8e, `GroupPlan2::new` in
`v2/mod.rs`, `cluster_q2w` and `check_det` in `v2/q.rs`, the header of `rust/tools/fit.py`.

The search for a passage called "theory of the order" in `PLAN.md` found none under that name; the derivation
`layered.md` 4.1 cites is the paragraph "Why mu + 3 sigma" of `HANDOVER.md` and the verdicts of `PLAN.md`.

Facts used later:

- The stop as built: the window's largest μ_c + (δ_q + ψρ_q)·σ_c below t̂; δ_q, ρ_q the mean and spread of
  z = (s − μ_c)/σ_c pooled over every cluster read directly, prior (0, 1) until two rows.
- Rule 1 as built takes **the median over the groups** of ρ_(g,ℓ), one number per level (`Adaptive::new`), where
  `layered.md` 4.1 states it per group. Recorded as claim A3.4.
- `hierarchy/theory.md` measures f for leaves that are cluster or sub-cluster centres. No note measures the share
  of the **rows'** score variance that the centres explain, which is the f the read needs. Measured here (entry 4).

## Entry 2. Literature: what I read of each paper

The PI's message, received while entry 4 was running, points to `notes/program/library/codes-and-search.md` Part 2
and its corrections. I read Part 2 and the corrections in full and use them. What follows is what I read myself
before that message; where the library read more, I defer to it.

| paper | what I read | how |
|---|---|---|
| Addario-Berry and Maillard, arXiv:1810.05129 | abstract, sections 1 to 5 in full: Theorem 1.1, the proof of both parts (Lemmas 2.1, 3.1 to 3.3), section 4 (leaf-only queries), the open questions with (5.1), the statement of Proposition A.1 | the ar5iv HTML stripped to text; formulas read from their TeX |
| Ho, arXiv:2308.00857 | abstract, definitions 1.2 to 1.6, the threshold β_G (1.8), Theorems 1.9, 1.10, 1.13, Corollary 1.11 with its proof; the proofs of the theorems not read | the same |
| Ho and Maillard, arXiv:2107.11465 | abstract and the introduction's statement of the two regimes; Theorem 1.9's statement; the paragraph on Karp and Pearl, Aldous and Pemantle | the same; grep of the text |
| Pemantle, Ann. Appl. Probab. 19 (2009) | nothing of the paper; its result as a search summary of the abstract and as Ho and Maillard describe it | **[cited, unchecked]** |
| Bovier and Kurkova 2004; Derrida 1985; Mallein 2015 | nothing; through Addario-Berry and Maillard | **[cited, unchecked]** |
| Quake, arXiv:2506.03437 | section 5 in full (the geometric model, equations (7) to (9), Algorithm 1, the optimisations), the related-work paragraph on SPANN, LAET and Auncel, the discussion's line on filters | arXiv HTML stripped to text |
| DARTH, arXiv:2505.19001 | a small model's answers to seven questions on the first 100,000 characters of the HTML (features, model, when invoked, training, hard queries, filters, LAET) | **summary by a model, not my reading** |
| CaST, arXiv:2611.16148 | a small model's paraphrase of the abstract | **summary by a model** |
| ConANN, PVLDB 19 | nothing: the PDF could not be read here (no PDF tool); a search summary only | **[cited, unchecked]** |
| LAET, Auncel, PAC-NN | nothing; as Quake, DARTH and the library describe them | **[cited, unchecked]** |
| "Closed Forms and Synthetic Twins", arXiv:2609.00364 | a model's summary of the abstract: recall of partition indexes predicted by simulation on a synthetic corpus built from cluster statistics | **summary by a model**; not used |

No PDF could be opened on this machine (no poppler, no Python PDF library), so only papers with an HTML version were
read.

**Check of the library's observation** (its 2.2: Model T's "f_j ≥ g_j against f_j < g_j" is the concave against
non-concave dichotomy). Half right; claim A1.5. A concave profile is above the diagonal, so f_j < g_j implies
non-concave. The converse fails. The two dichotomies are the diagonal (weak against strong correlation) and
concavity, and they speak of different levels: Model T's formula is for the level at which a count of L is
expected, and where A(t) > t that level is empty with high probability. I also checked that in the weak regime the
beam's exponent sup(t − A(t)) is the pool exponent of their conjecture (5.1) at the ground state.

## Entry 3. A1 on paper

Order of work, with the turns that failed.

1. Wrote Model T as a generalised random energy model with profile through (g_j, f_j) (A1.1).
2. First idea: the hardness theorem gives the exponent of the read. **It does not, as it stands.** Its threshold is a
   statement about a binary tree as the depth grows; a cluster has 2,000 direct children and no inner levels, so
   the linear-time greedy descent is not available inside a level. For a flat tree I worked out the beam's cost by
   a pool recursion and got η = max_j (g_j − f_(j−1)) (A1.4).
3. Found that the leaf term of that maximum is not only what a beam pays but a floor for every algorithm in the
   node-query model, by the Neyman–Pearson argument with the internal values given for nothing (A1.2). This is the
   one hardness statement that carries to a flat tree without the depth going to infinity.
4. **A contradiction that taught something.** For a profile above the diagonal and not concave, (A1.4) says
   subpolynomial and the theorem says exponential. Resolved in A1.5: (A1.4) is an expectation at a level that is
   typically empty there.
5. **Refuted guess.** I took the 13-against-7 of `model_out.txt` (shares 0.6, 0.25, 0.15) for that effect. Over 200
   trees the per-tree mean is 6.6 against the formula's 7.1 and the realised level is 0.967 of the expected one
   (the hull formula gives 0.960). The 13 was the 99th percentile of ranks pooled over 12 trees. Claim A1.8,
   [refuted]. The hull formula itself checks.
6. The closed form M·Φ̄(z_T√f − z_r√(1−f)) overstates the exact floor by 10 to 40%, and the exponent 1 − f is
   reached slowly: at M = 10⁶ the local exponent is 0.08 to 0.22 higher (A1.3). So "N^(1−f)" must not be read off
   at 1M.

## Entry 4. A1 measured

`scripts/replay_np.py` (first part), three fits, 100 queries each; the fit's clusters and groups, plus nested 4-way
k-means inside each cluster for a4, a16, a64. Tables T1 to T4, T10.

**First result, and the wrong conclusion I nearly drew.** On yahoo the 10th best score is 6.8 total standard
deviations out, against 4.2 for a Gaussian, and the law at that level predicts a share of 0.15% where 1.2% is
measured. I wrote "the mapping fails". Then the same law at the Gaussian level gave 4.5% against 2.0 to 2.7%
measured, with the right slopes in N and in the cell count. The fair statement is A1.7: the mapping holds in rank
space to a factor of two, and exactly on glove, whose scores are close to Gaussian (4.5 against 4.3).

What the numbers say, beyond `CLAIMS.md` A1.7:

- The profile A lies on the diagonal at the groups and the clusters on yahoo and LAION (0.28 at 0.31, 0.42 at 0.43;
  0.28 at 0.30, 0.45 at 0.45). The build rule's K ≈ √N sits where the centres explain half the entropy and a little
  under half the variance. Below the clusters each fourfold refinement adds only 0.04 to 0.06 of variance for 0.10
  of entropy. The last segment, from 31-row cells to rows, carries 41 to 51% of the variance in a quarter of the
  entropy. That last steep segment is what makes the profile non-concave, and it is the "information the mean and
  spread do not carry" of the README's finding 5.
- Glove's groups explain 0.20 at entropy 0.30: the groups are a weaker level there than on the other two.
- On LAION's text queries the centre score alone is a poor key (6.5% of rows at the cluster level against 2.9% for
  μ + 3σ), and a true neighbour's cluster sits 0.30 below the regression line. On yahoo μ_c and σ_c are correlated
  0.66 within a query, which is not an additive tree.
- At a fixed number of cells the share of rows read falls slowly with N (exponent −0.06 to −0.17), as
  `layered.md` section 2 found ("the share depends on K, not on N"). The Gaussian law falls faster (−0.21 to −0.29).
  This is where the model is most wrong in a way that matters for extrapolation: it would predict that a larger
  corpus at the same K is easier than it is.

**Limits.** Nested 4-way splits, not flat k-means, below the clusters. Subsamples keep the full set's partition (a
random visible set, which `layered.md` shows equals a subsample). The rank-share statistic is a quantile over
neighbours of a global rank threshold, not the nprobe curve of `layered.md`, so the exponents compare only roughly.

## Entry 5. A2: the pre-test, and why the idea does not survive it as stated

The same script replays the stop: whole clusters in the static order μ_c + 3σ_c with exact σ_c, five visible sets.
Tables T5 to T9.

**Runs.**

- v1 (yahoo only; `out/replay_v1_yahoo.*`, script `replay_np_v1.py`): the built stop, expected misses with a normal
  tail, two upper quantiles, a generalised Pareto tail and an exponential tail.
- A queued run for LAION and glove never started: its wait loop matched its own command line. Killed by pid.
- v2 (all three): added the pooled shape tables, the collapse table, the first-ten-clusters predictor and the
  rank-share law.
- LAION's first v2 run counted every row tied with the k-th best as true, and reported recalls up to 1.016 under
  region-1. Fixed (exactly k rows, ties to the lower row) and rerun; the earlier output is kept as
  `out/replay_laion_before_tie_fix.*`. Yahoo and glove ran before the fix; no recall on them exceeds 1.
- Check of the visible sets: the k-th best among visible rows agrees with the prep's `knn_thr` files to within
  0.003 to 0.017 on every set (the prep stores the midpoint to the next score, and the scores here are from f16
  vectors).

**What I expected.** That a tail fitted to the pooled scores would make the setting mean a recall and cost nothing
in rows.

**What happened.**

1. The normal tail undershoots, as the README's finding 9 said: one expected miss delivers 0.65 to 0.85 on yahoo
   and LAION. The shape is close to exponential with rate 2.0 to 2.8 per standard deviation between 3 and 6 (T5).
2. A generalised Pareto fit per query is right on average on yahoo and LAION unfiltered (it predicts 0.92 and 0.94,
   the count is 1.06 and 1.29) and still delivers 0.85 and 0.81 and reads 1.5 to 3 times the rows. The fit has about
   180 to 240 rows above z = 2 and 14 to 18 above z = 3 when it decides, and must speak of ζ near 4. The stop picks the reads where the
   fit is low. Claim A2.3 puts the per-query error at a factor of 2 to 4.
3. An exponential fit per query over-predicts 4 to 11 times unfiltered on yahoo and LAION (39 times on glove) and
   under-predicts 1.4 to 3.3 times under the 1% region set. It delivers the most stable recall of the fitted tails and reads up to 3 times the rows.
4. One shape for every query, whether from the test queries or from corpus rows at build, over-predicts at the true
   k-th best by 2.2 to 2.5 times, because queries differ in tail weight (the sum rule fails). The build's table is
   also 3.3 times too light at ζ = 4 for LAION's text queries.
5. The first ten clusters' own tail over-predicts the later clusters' neighbours 1.7 to 3.1 times: the bias from the
   best clusters being read first exists and is on the safe side.
6. So the two repairs pull against each other: a shape shared by all queries is biased by their differences, and a
   shape fitted per query is too noisy at the time it is needed. A2.2 says the noise is not an accident: nothing
   read from n rows can speak for a tail at 1/W.

**What came out that I did not expect.**

- The built stop with δ_q and ρ_q from two upper quantiles (the 5% and 0.5% levels of the pooled z) instead of the
  mean and spread reads 0.64 to 0.93 of the rows on glove under none, random-25 and region-25, and 0.80 to 1.35 on
  the other two. It is the cheapest form of "extend the pooled moments by a tail" and the only variant with an
  upside. It is in `REPLAY-SPEC.md` as R1.
- A fixed number of clusters beats the built stop on LAION unfiltered at recall 0.98 and 0.99 (0.77 and 0.81 of the
  rows) and on glove unfiltered (0.68 to 0.81). With 100 queries the high-recall cells rest on a few hard queries,
  so this needs the harness before it is believed; if it holds, the pooled (δ_q, ρ_q) costs more than it gives on
  unfiltered queries, which `results/region25-order/README.md` also half-says ("the timed 5 to 25% there is not fully
  explained in rows").
- ψ = 4.5 delivers 0.92 to 1.00 over the fifteen cells. As a dial it is steadier than any expected-miss setting.

**Not tried.** The self-consistent τ̂ arm (solve rows-read-above-t plus predicted = k); a shape learnt online from
the queries served; k = 100; the in-cluster bottom and top sets; a window; the library's σ.

## Entry 6. A3

Derived the per-query factor as a weighted mean of a_i (A3.1). Two things followed that I had not seen in the
brief: it is identically the profile's factor wherever the level codes every coordinate equally, so it can only act
through uncoded coordinates; and narrowing a width at fixed ψ is the same as lowering ψ, so its value is that one ψ
means the same in and out of distribution.

Could not use the build's own error variances: `det_alloc_tree` is in Rust and the codes are in the index. Used a
reverse water-filling stand-in, which reproduces the build's derived ρ within 1 to 6% (`out/a3_check.txt`). With it
the rule explains about 60% of LAION's 13.5-point gap at 16 B and 40% at 32 B, nothing on yahoo or glove, and not
the spread of the measured ratio across (query, cluster) pairs.

The exact check belongs to the owning session: in `study_layered_dump`, print s²/(w²·‖a‖₁) per (query, cluster,
level) beside the measured rms of (score − midpoint)/half-width that `layered/explore.py` already computes, and
correlate. Prediction: correlation near zero in distribution, and a slope near one across LAION's text queries.

## Entry 7. Queries on the source notes

Kept as queries; I have not changed any source.

1. `hierarchy/layered.md` section 0 calls N^η "the floor for any hierarchy keyed by centres and spreads". It is a
   measured property of these profiles. A1.2 is a floor in the Gaussian model only, and yahoo's read is below that
   model's.
2. `hierarchy/layered.md` 4.1 states rule 1 per group; the code takes the median over groups (A3.4).
3. `hierarchy/theory.md` 2.3, the synthetic check: "measured 13, predicted 7" is a pooled percentile over 12 trees;
   the per-tree mean agrees with the formula (A1.8). The formula's regime f_j ≥ g_j needs the caveat of A1.5.
4. `theory.md` Theorem 3 is used in the README (finding 6) to say a global threshold beats a fixed count. In the
   pre-test the built stop loses to a fixed count on two unfiltered sets at high recall (entry 5). The theorem
   assumes a calibrated p_c.
5. `theory.md` 2.4 on Quake, LAET and DARTH: corrected by the library; nothing to add.

## Entry 8. What I would do next

1. **A2 on the harness**, as `REPLAY-SPEC.md`: the dump once, R0 against R1 first. This decides whether the
   two-quantile pair is a real gain outside glove and whether the built stop really loses to a fixed count
   unfiltered.
2. **Why a fixed count wins unfiltered**, if it does: replay the built stop with the prior (0, 1) held, with ρ_q
   only, and with the moments shrunk towards the prior by the rows pooled. The region study pooled to fix a filter's
   shift; the same pooling may add noise where there is no shift.
3. **A model that fits the tail.** The standardised within-cluster scores are exponential-tailed with a rate that
   differs by dataset and by query. A branching random walk with exponential-tailed increments is the natural
   replacement for Model T; Ho and Maillard's results are stated for general increments. The question worth an
   answer: does the regression-to-the-mean law in rank space (A1.7 item 6) follow from such a model, and does it
   give the slow fall of the share with N at fixed K that the Gaussian law misses?
4. **The profile at scale.** A(t) is measurable with no truth: one pass of pseudo-query scores and the cells'
   means. Measuring it on a 10M and a 100M slice tests the assumption under A1.7 item 7 (the profile is fixed as N
   grows) and gives 1 − A(½) for the build rule and sup(t − A) without any replay.
5. **A proof of the pool exponent for a flat tree.** For D levels with flat fan-outs, is max_j (g_j − f_(j−1)) a
   lower bound for every node-query algorithm, not only the beam's cost? A1.2 gives the last term. The chain-of-
   spindles argument of Addario-Berry and Maillard should adapt, with one spindle per level.
6. **A3's exact check**, as in entry 6. Low priority: the expected gain is a consistent ψ, not rows.

## Entry 9. After referee report 1 (the same day)

The report is `referee/referee-1.md`; it covers A1.1, A1.2, A1.4, A1.5, A2.1, A2.2 and A3.1. I made every repair it
prescribes and dispute nothing. `CLAIMS.md` ends with a table of what changed; the text before the report is
`out/CLAIMS_before_referee1.md`. What the report changed in what the track can say, and what I learnt from it:

- **Two models, not one.** Model T has a step profile; the piecewise-linear A is its binary refinement's. The
  hardness theorem is about the refinement. For the index's flat tree x_* is zero and the only bounds are the leaf
  floors and the beam's W. I had said this in A1.6 and not in A1.1, where the rest cites it.
- **The k best need their own floor.** The referee's A1.2b closes the gap I had named. With a few hundred clusters
  τ_k does not concentrate, and the floor is 1.05 to 3.1 times the fixed-level one. I checked it with leaves
  sampled (1.75 against the referee's 1.73 at 28 × 784, f = 0.45).
- **My "law" in A1.7 was the closed form at a fixed level.** Replaced by the model's own value of the statistic I
  measured (`scripts/topk_model.py`). At the cluster level the closed form happened to be within 10% of it, because
  its error against the fixed-level floor and the k-best factor cancel there. The directions of A1.7's conclusions
  stand; the numbers changed.
- **Exponents do not rank designs at 1M.** A1.9's ranking is withdrawn and re-derived by counts. What survives: rows
  are the count in every design; finer leaves lower them if the read is one knapsack over the leaves; a beam with a
  cut per level gains from the first refinement only; a level above the clusters is not worth having at 1M.
- **The hull sum is no ruler at these sizes.** My 0.967 against 0.960 was an accident of L = 64. A1.7 item 8 now
  claims non-concavity in the limit and no margin.
- **A2.1 bounds a mean.** In its own model 7 to 23% of queries fall under the target. Item 4 now uses the ratio of
  the sums at the stop.
- **A2.2 does not say "nothing between".** A Cantelli stop is valid for every standardised shape. Entry 5's
  sentence "nothing read from n rows can speak for a tail at 1/W" is too strong as written: it holds for the class of
  all shapes, and what lies between that and a parametric family is open.
- **A3.1's exact statement** is the ratio of two conditional second moments given the row's scale; conditioning on
  the stored norm is the approximation.

**A slip in this round.** In the runs on the fits' trees the floor's column came out above the static optimum in 11
of 18 cells, which cannot be. A diagnostic at 19,200 draws a seed shows it is noise (floor 0.95 to 1.00 of the
static optimum). I quote the static optimum only from those runs.

**Limits kept.** Core 5, one thread, no real data. Three of the four runs peaked over the 100 MB set for this
round (154, 124 and 108 MB), each for under three minutes.

**Still open after this round:** the sequential form of A2.2; the standardised case's two gaps; whether R(r) is a
floor for the k best; the lower bound for a flat tree beyond its last term (entry 8, item 5); the assumption that a
level at entropy position g explains A(g) for non-nested partitions; siblings' independence at the 4-way splits;
the model's own value of the subsample exponents (A1.7 item 7 still uses the closed form there).

## Entry 10. A4: a level width that holds out of distribution (the same day)

The problem came from the owner through the PI: rule 1's derived ρ is about 2.3 times too narrow on iNaturalist.
The PI sent an analysis in seven parts to prove, correct or refute. Result: section A4 of `CLAIMS.md` and
`REPLAY-SPEC-RHO.md`. Synthetic numerics only, through `notes/program/run-limited.sh 6 1 2`:
`scripts/a4_model.py`, `out/a4_model.txt`. No real data was read. Nothing of A4 is refereed.

How it went, with the turns:

1. **The organising identity came first and made the rest short.** The error over the half-width is a cosine,
   (Δ·e)/h = Δ̂·ê, so the measured ratio squared is Δ̂ᵀC_cΔ̂ with C_c the second-moment matrix of the weighted errors'
   directions. The derived ρ and A3 are that quadratic form with C_c replaced by the group's diagonal. Form 3 is its
   top eigenvalue. The PI's (i) and (v) are then two lines each.
2. **The PI's hypothesis (ii) holds in a model and is a little too narrow as worded.** In a constructed group the
   ratio for a cluster's own queries rises from 1.00 at one cluster a group to 2.1 to 2.7 at 32, and A3 corrects
   none of it. But the own directions on coded coordinates count too: a coded coordinate's error keeps a share of
   the residual. So "the uncoded tail" is most of it, not all.
3. **Form 1 surprised me twice.** It is wide by 2(b − 1)·Var(a), and with the slopes `layered.md` already reports
   (1.04 to 1.09) that is 15 to 33% in the spread on the three fits, before any alignment. And it is a difference
   of two numbers, so a 12% error in the 16-direction σ is 30% in the width.
4. **Form 2's selection question has a clean answer and a caveat.** If the midpoint is a conditional mean, any
   selection made on what is known at the level, the reader's rule with an estimated width included, leaves the
   increments unbiased. If it is not (slope b ≠ 1), the rows that go on have a positive mean increment; the pooled
   mean square then includes that drift, which is the safe direction.
5. **Two corrections to the PI's (iv) on the randomised levels.** The stored scalars give sd², which bounds the
   error variance; the exact value needs the angle to the block. And the chain through a randomised level is not
   an orthogonal sum: E(a_R − a_ℓ)² = (s − a_ℓ)² + v_R, so summing mean squares through R over-counts by 2·v_R. The
   identity is still the useful thing: (a_R − a_ℓ)² − v_R is a per-row unbiased estimate of the deterministic
   level's squared error, for any data.
6. **The cousin (vi) is unsafe where it is needed**: 0.64 to 0.82 of the truth in the model from eight clusters a
   group, because the cross term it drops is positive for aligned queries.
7. **An addition of mine**: a rank-16 sketch of C_c, evaluated per query with the 17th eigenvalue for the rest. It
   is a bound, and in the model it is within 3 to 9% of the truth where the derived ρ fails. It is the cluster's σ
   one level down.
8. **A slip caught before it was written up**: I first wrote that A3 over the derived ρ is 1.00 for every kind of
   query in the model. It is 1.00 for the cluster's own queries and for queries on the profile; for isotropic
   queries A3 moves with the truth (0.62 to 0.95), as it should. Several ranges quoted from the model's table were
   also corrected against the output before saving.

Recommended: find the cause with the dump first; then form 2 as the width with the derived ρ as a small prior and
form 3's cap as ceiling; the cap at build as a warning in any case; the sketch if form 2's first rows cost recall.

Not done: the model has one covariance shape (a 1/i common spectrum, 8 own directions, 30%); the toy process for
form 2 has one level and normal errors; no check of whether `det_alloc_tree` reconstructs uncoded coordinates as
zero; nothing on the cost in rows of the drift term; iNaturalist's N, K and bits per level not looked up.

