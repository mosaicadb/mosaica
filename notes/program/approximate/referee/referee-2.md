# Referee report 2: section A4 of `approximate/CLAIMS.md` (the level width)

21 November 2026. Independent referee; NOTEBOOK.md not read. Verdicts are appended as each item is checked.
"No error found" means I tried to break the claim and could not; it does not mean correct. The measurements are
taken as recorded in A4.0 and in `program/STATE.md`; none was re-measured. My numerics are synthetic
(`referee/ref2_*.py`, outputs `referee/ref2_*.txt`).

## 1. A4.1 (the error over the half-width as a cosine)

**Verdict: no error found.** The identity (Δ·e_vec)/h = Δ̂·ê, the two means (Δ̂ᵀC_c Δ̂ for the mean squared ratio,
Δ̂ᵀS_c Δ̂ / tr S_c for the ratio of second moments), and the reduction to (5.6) and to (A3.1) under S = κ²·diag(a_i)
all check by direct algebra (ν_i/D_i² = a_i; on the profile ω_i = a_i and w² = ‖a‖₁).

What each width is the second moment of, stated because the text leaves the reader to sort it out:

- the derived ρ_(g,ℓ)² is a **ratio of two means**, E(Δ·e)²/E h², pooled over rows and over offsets on the profile,
  with S_c replaced by diag(a_i); it is not the mean of cos²;
- A3's ρ_ℓ(q, c)² is the same ratio at one offset, still with the diagonal S;
- chapter 5's "measured rms of (score − midpoint)/half-width" is the **mean of the ratio**, Δ̂ᵀC_c Δ̂;
- the sampled τ of A4.2 is a fourth thing: the **centred** standard deviation of e over the rows, with no h in it,
  so the width ψτ is the same for every row of a cluster and no longer uses the row's stored norm u.

Unstated hypotheses.

1. D is diagonal in coordinates in which the score is the plain pairing Σ Δ_i e_i (A3's "dual coordinates"), and h
   is the code term w·u alone with u exact. If u is stored rounded up, or h carries other terms, the ratio is below
   the cosine.
2. The "left out" expression Δᵀ Σ_(E,c) Δ − κ² Σ Δ_i² ν_i compares numerators only. It is what A3 leaves out of the
   ratio only if κ² is *defined* by tr S_c = κ²·‖a‖₁. Σ_(E,c) is not in the local symbols; it has to be the uncentred
   second-moment matrix of e_vec over the cluster for "and its mean" to make sense.
3. The mean of the ratio and the ratio of the means differ when the a_i are unequal, and A4.0's first row ("actual
   rms error over the derived ρ·h") does not say which of the two it is. Sized in A3's model at n = 384 with the
   offset on the profile (`ref2_a41.txt`): the two agree within 1% for equal or water-filled a_i and within 3% for
   log-normal a_i of standard deviation 2. So this does not touch the 1.29 and 2.04.

Weaker or stronger than shown: as shown.

## 2. A4.3 (a neighbour's error is not a typical row's)

**Item 1 (kept if and only if e ≤ y + (s − t)): no error found. The sentence after it needs a hypothesis.** "A row
exactly at the threshold is the worst case whenever y does not fall as s rises" is incomplete as worded: y is a
function of what the level knows, not of s, and the kept probability P(a ≥ t − y | s) is non-decreasing in s only if
the law of the midpoint given
the score is stochastically non-decreasing in the score. That holds under a linear regression with constant
residual spread and Cov(a, s) ≥ 0. It can fail when the scatter widens with height, which STATE.md records.

**Item 2 (the joint normal law): no error found.** E[e | s] = ρ_es·x·τ, sd = √(1 − ρ_es²)·τ,
ρ_es = √(1 − f) + ρ_ae·σ_a/σ_c, the keep probability and the width all follow from the regression; with ρ_ae = 0
they are chapter 5's Proposition 5.17. Unstated: E e = 0 over the cluster's visible rows (otherwise add the mean).

**Item 3 (refutation of the uncorrelated normal law): the refutation holds.** Φ((4 − ζ√(1 − f))/√f) is 0.997 to
0.9995 at ζ = 3 and 0.981 to 0.9986 at ζ = 4 for f from 0.67 to 0.92; measured 0.79 to 0.82. The predicted slope
√(1 − f) is 0.28 to 0.57; measured 0.92 to 1.24. "Neighbours near ζ = 3" is not a recorded figure; it is implied by
the recorded median y of about 3 and median y/x of 0.89 to 1.21 (x about 2.5 to 3.4). The refutation does not
depend on it: at ζ = 4 the prediction is still 0.98.

**Item 4 (the slope under a linear regression): no error found.** If E[e | s] is linear over the rows of a cluster,
the slope of y on x is Cov(e, s)/(τσ_c) = ρ_es ≤ 1; selection on s (being a neighbour) does not bias it; a pooled
slope through the origin is then a weighted mean of the per-(query, cluster) correlations, so also ≤ 1. The factor
formulas (1 − f = Σ w_k κ_k², ρ_es = Σ w_k κ_k/√(Σ w_k κ_k²)) are correct, by Jensen with Σ w_k = 1.

What follows from a slope above 1 is exactly this and no more: **with errors centred, and x and y in the cluster's
own exact μ_c, σ_c and τ, E[e | s] is not linear over the cluster's rows.** Unstated hypotheses that the inference
needs and A4.0 does not record:

1. y is centred. A slope through the origin of an uncentred y is ρ_es + ē/(τ·x). A mean error of 0.3τ adds 0.1 at
   x = 3 and 0.24 at x = 1.25. A4.9 recommends adding the sample's mean of e to the midpoint, so the author thinks
   the mean is not zero. Whether the session centred y is not recorded.
2. τ and σ_c are those of the population the regression is taken over. They were sampled (64 rows). Sampling noise
   in σ̂/τ̂ moves the pooled slope by about 1%, not 20%.
3. "The diagonal model gives ρ_es = √(1 − f)" also needs each coordinate's reconstruction to be uncorrelated with
   its error (a centroid condition). Independence across coordinates alone does not give it. The recorded slope of
   score on midpoint of 1.04 to 1.09 says the built midpoints do not satisfy it.

**Item 5: the arithmetic is right; one sentence is stronger than what is shown.** τ/τ₁ = 1/√(1 + 2Cov(a, e)/τ²)
gives Cov(a, e)/τ² = 0.21 to 1.42 from 0.84 and 0.51; yahoo's ρ_es = 0.56 follows from share 0.84 (read as r² of s
on a) and slope 1.09. A consistency constraint the text does not state: ρ_es ≤ 1 forces
Cov(a, e)/τ² ≤ 1/√(1 − f) − 1, which is 2.5, 1.7, 1.4, 0.86, 0.74 for the five indexes, so the figure 0.51 can only
belong to iNaturalist; "1.2 to 2.4 times √(1 − f)" pairs extremes whose index is not recorded. "Cannot come from a
linear regression" is proved. "They need a component of the score with a heavier upper tail than the rest and a
left-out share above the root mean square" is not: that is one sufficient mechanism, in the family of additive
independent factors. Any E[e | s] convex in the upper tail does it.

## 3. A4.4 (a neighbour shares a component with the query)

**Verdict: the algebra of the model is right; the model as run does not reproduce the figure the width rests on
(y ≈ m·x through the origin with m of 0.9 to 1.2), and the text does not say so. Gap.** `ref2_a44.py`,
`ref2_a44.txt` (the model in population, 24 million rows a setting, the author's parameters).

What checks.

- e = κ_q·(s − μ_c) + (e_G − κ_q·(a_G + e_G)); s_res; s_res/(√f·τ) = 0.98 to 1.28; the slope of score on midpoint
  1.06 to 1.09 over all rows and tending to 1/(1 − κ_q); "too kind by 7 to 14 points" and "3 to 7 points" at ψ0 = 4.
  All reproduce.

What does not.

1. **The slope.** High in the cluster the model's regression of y on x is a line with a negative intercept, not a
   ray: given a high score the normal part is tilted by the exponential, E[a_G + e_G | s] → Var(a_G + e_G)·λ_R (λ_R
   the rate of R), so

       E[e | s] → κ_q·(s − μ_c) + c0 ,     c0 = ((1 − κ_q)·Var(e_G) − κ_q·Var(a_G))·λ_R ,

   which is −0.65τ to −1.81τ in the five settings. "r tends to κ_q, the slope of y on x to κ_q/√(1 − f)" is the local
   slope far up. What was measured is the ratio y/x and the slope through the origin. The model's median y/x:

   | | iNaturalist, 12 | iNaturalist, 1 | yahoo | LAION, 64 | LAION, 8 |
   |---|---|---|---|---|---|
   | measured median y/x | 1.17 | 1.21 | 0.94 | 0.89 | 0.96 |
   | model, x from 3 to 4 (where the measurement puts the neighbours) | 0.74 | 0.80 | 0.67 | 0.74 | 0.78 |
   | model, x from 4 to 5 (where the author's run puts them: ζ = 4.1) | 0.85 | 0.92 | 0.73 | 0.78 | 0.83 |
   | model, x from 0.5 to 2 | 0.40 | 0.45 | 0.53 | 0.62 | 0.65 |

   The same can be read from the author's own table: model median r over √(1 − f) is 0.85, 0.91, 0.73, 0.78, 0.82.
   So the model has no slope of 1 and no slope above 1 at any height below 5 spreads, on any index. Its median y
   of 3.1 to 3.7 agrees with the measured "about 3" because its neighbours sit a spread higher than the data's
   (4.1 against about 3) and its ratio is 12 to 27% lower: two errors that cancel.
2. **iNaturalist's 1.2 is out of the model's reach at the stated tail rate.** With the largest κ_q the model allows
   (all of the error in the shared component, κ_q = √(1 − f)·2.2) the median y/x is 0.14, 0.69, 1.06, 1.32 for x in
   0.5–2, 2–3, 3–4, 4–5. Measured on iNaturalist at 12 groups: 1.06 at 0.5–2 rising to about 1.3 at 4–5. A4.4 item 3
   cites that rise as agreement. The model's rise is from 0.4 to 0.85 with the author's κ_q and from 0.14 to 1.32
   with the largest: the direction agrees and the level does not. The bottom bin is not like for like. A cluster's
   neighbours are all of its rows above t, at most ten, so a neighbour at x between 0.5 and 2 comes from a cluster
   with at most ten rows above that height: a small one, or one whose sampled σ_c is inflated. The model's clusters
   of 2,000 rows have no neighbours there. What those clusters are is not recorded.
3. **A trade-off the text does not show.** With the shared component carrying half the score variance (tail rate
   1.4, `ref2_a44_rate1.4.txt`) the author's κ_q give y/x of 1.03, 1.08, 0.83, 0.87, 0.92 at x from 3 to 4, close to
   the measurement, and Cov(a, e)/τ² of 0.4 to 1.5, the measured range. But the slope of score on midpoint over all
   rows is then 1.17 to 1.30 against a measured 1.04 to 1.09, and the scores' tail rate is not A2.4's. At rate 2.2
   the all-rows slope fits and Cov(a, e)/τ² is 0.15 to 0.60: the measured 1.4 cannot be reached at all, since the
   model's largest value is 1/(4·2.2²·(1 − f)) = 0.65 at f = 0.92. The model fits the bulk or the neighbours, not both.
4. **Figures in the output and not in the table.** The model keeps 0.855 to 0.989 with κ = 1 − f at ψ0 = 3 (measured
   0.77 to 0.90) and 0.65 to 0.90 at ψ0 = 2 (measured 0.47 to 0.73); with (ζ + ψ0·√f)·τ at ψ0 = 2 it keeps 0.99 to
   1.00 (measured 0.918 to 0.976), and it needs ψ0 of −0.3 to 0.7 for a kept share of 0.9 where the data need about
   1 to 2 (0.918 at ψ0 = 2 on iNaturalist; κ = 1 − f at ψ0 = 3, which is this width at ψ0 near 1 when ζ = 3, keeps
   0.77 to 0.90). The stated misfit ("3 to 7 points") is the smallest of these.
5. **The spread of r.** Model 10% to 90%: 0.21 to 0.30 wide. Measured: 0.33 to 0.49 wide. Model I is refused for
   being 0.77 wide against LAION's 0.47; model C is 0.27 wide against the same 0.47. The two miss by about the same
   factor in opposite directions. A κ_q that varies between (query, cluster) pairs would widen C's; the run fixes
   one κ_q a setting.

**Item 2's participation-ratio formula: correct for all-or-nothing coding only.** "√(1 − f) ≈ √(tr Λ_left²/tr Λ²)"
treats the left-out part of a partly coded coordinate as a fixed share of its amplitude, which makes a_G and e_G
perfectly correlated on that coordinate; the model takes them independent. With quantisation error uncorrelated
with the reconstruction (distortion D_i on a coordinate of variance Λ_i), the consistent statement is
κ_q ≈ tr D/tr Λ (the gain of a centroid quantiser to a small common shift is D_i/Λ_i), 1 − f ≈ tr(DΛ)/tr(Λ²), and

    slope far up ≈ (tr D/tr Λ)·√(tr(Λ²)/tr(DΛ)) .

This is what A4.6 item 5 uses ("the level's plain distortion" against "the distortion weighted by the cluster's
variances"). The two forms agree when every coordinate is wholly coded or wholly left out. They disagree for a
level that leaves the same share ρ of every coordinate: the text's form gives slope 1 ("as concentrated as the
whole"), the consistent form gives √ρ = √(1 − f), the uncorrelated normal value. The direction for iNaturalist
(left-out variance spread over many low-variance coordinates gives a slope above 1) survives. Further hypothesis:
the gain D_i/Λ_i is a small-shift statement; it fails for a near-duplicate of the query.

**"A single population with independent heavy tails does not fit": the conclusion is supported; two of its four
grounds are not.**

- "It cannot reach f above about 0.8" is an artefact of giving the estimate's and the error's exponential parts one
  scale. With separate scales any f is reachable. (It is then no better a fit: if the error's scale is the smaller,
  the far tail of the score is the estimate's and r tends to 0 there.)
- The spread of r does not separate the models (point 5 above).
- What does refute independence of midpoint and error, for any tails: Cov(a, e) would be 0, so form 1 with the exact
  variance would be right (measured 0.51 to 0.84 of the truth) and the slope of score on midpoint would be 1 at every
  height (measured 1.04 to 1.09); and the plain rule would be efficient given a common shape of e/τ (measured 7 to
  18% worse than κ = 1 − f on LAION). These are model-free and enough. What is refuted is independence of a and e,
  not "a single population": model C is one population too.
- "The mixture reading is refuted by r having one mode" is a fair reading of the record and not a proof.

Weaker or stronger: the section's summary in A4.9 ("fits in kind, not to the point") is too kind. The kind of thing
the width needs, a ray through the origin with slope near 1 and a centred scatter, is what the model lacks.

## 4. A4.5 (the width (ζ + ψ0·√f)·τ)

**Item 1 (the keep probability as a function of the slope): no error found, as algebra on its stated law.** With
e = m·x·τ + √f·τ·g a row at the threshold is kept if and only if g ≤ ψ0 − (m − 1)·ζ/√f, and a row above it gains
(x − ζ)·(σ_c − m·τ). Hypotheses the sentence "whatever f and ζ are" needs: g is centred, of spread exactly √f·τ, and
its law does not depend on height. STATE.md records that the scatter widens with height, so the last fails on the
data, and the kept share at one ψ0 falls with ζ even at m = 1.

The domain of the law, which the text does not state. A cluster's neighbours are all of its rows above t, at most
ten of them, so the law was measured on the top ten rows of a cluster and nowhere else. It cannot be extended
down: over a whole cluster it would give Var(e) = (m² + f)·τ², about 2τ², and already over the rows above half a
spread of a normal cluster 0.485·m² + 0.309·f, which is 0.74 to 0.98 of the whole τ² at the recorded m and f. So
it says nothing about a row in the bulk. That matters twice: for A4.8's "neighbours that are not extreme in their
cluster", and for any filter that moves the visible neighbours down into the bulk (item 6, point 4), where the
slope to use is the bulk's ρ_es (0.56 on yahoo) and not m.

**Item 2 (the inferred tail): the arithmetic is right; the inference is wrong in a way that hid the later finding.**
The text compares the probability of keeping a row *at the threshold* (0.970 and 0.993 for a Laplace scatter) with
the measured share of *all ten* kept. Nine of the ten lie above the threshold and have slack, by item 1's own
formula. With an exponential excess of rate 2.2 a spread (`ref2_a45.txt`):

| miss at ψ0 = 2 / 3 | yahoo | LAION, 64 | LAION, 8 | iNaturalist, 12 | iNaturalist, 1 |
|---|---|---|---|---|---|
| recorded | 0.024 / 0.008 | 0.031 / 0.015 | 0.036 / 0.019 | 0.082 / 0.025 | 0.082 / 0.028 |
| Laplace, at the threshold, m = 1 (the text's comparison) | 0.030 / 0.007 | same | same | | |
| Laplace, over the ten, m = 1 | 0.017 / 0.004 | 0.019 / 0.005 | 0.020 / 0.005 | | |
| Laplace, over the ten, the measured m | 0.016 / 0.004 | 0.012 / 0.003 | 0.018 / 0.004 | 0.034 / 0.008 | 0.049 / 0.012 |
| normal, over the ten, the measured m | 0.009 / 0.0004 | 0.005 / 0.0002 | 0.010 / 0.0005 | 0.034 / 0.003 | 0.053 / 0.005 |

Sampled statistics add a factor of about 1.2 at ψ0 = 2 and 1.3 at ψ0 = 3 (the author's `a4_width_noise.txt`). So
on A4.0's own numbers, with the measured slopes, the misses are 1.3 to 2.1 times a Laplace scatter's at ψ0 = 2 and
1.6 to 3.8 times at ψ0 = 3: the tail was already heavier than Laplace. Independently of any
model, one unit of ψ0 from 2 to 3 divides the recorded misses by 1.9 to 3.3; a Laplace tail divides them by 4.1.

What survives the later measurement (99.9th percentile of the scatter 4.6 to 7.5; widening with height):

- ψ0 is not a normal quantile. Survives.
- "ψ0 means about 1 − ½·exp(−√2·ψ0)". Does not survive as an equality; it survives as A4.8's upper bound ("at
  best"). Near ψ0 = 2 the normal, the Laplace and the data agree (0.96 to 0.98 kept where m is near 1), because the
  two laws cross there. Beyond it use the recorded decay: each unit of ψ0 divides the misses by 2 to 3.
- "A recall of 0.999 a level needs ψ0 near 4.4" (A4.8). Does not survive: 4.6 to 7.5 as recorded, and the recorded
  decay from ψ0 = 2 gives 4.9 to 7.6 for 0.999 of the ten.
- "iNaturalist's shortfall at ψ0 = 2 is its slope". Survives as a statement about the difference between indexes:
  the slope multiplies the predicted miss by 2.6 to 3.2 and the recorded ratio to the other three is 2.3 to 3.4. It
  does not survive as a level: with the slack counted, slope 1.2 and a Laplace scatter give 0.034 to 0.049, and
  0.082 was recorded.
- A scatter that widens with height is what a slope that varies between (query, cluster) pairs looks like when
  pooled: sd(y | x)² = s² + x²·Var(m). Then the coefficient of ζ that gives ψ0 one meaning is an upper quantile
  of the pairs' slopes, not their centre. Not measured; it follows from the record that y/x has a 10% to 90% range
  of 0.5 to 1.65.

**Item 3 (which rule is efficient).** The principle (threshold on the probability of beating t) and the second
case (κ = 1 − 1/b = Cov(a, e)/Cov(a, s); 0.04 to 0.08) check. Hypotheses to add: one cost a row passed on; t fixed;
and in the first case a common law of e/τ across clusters, without which "depends on t − a alone" holds within a
cluster only. **The third case is wrong as stated; correct after this repair.** In the model's tail the law of s
given a is normal about μ_c + (a − μ_c)/(1 − κ_q) − κ_q·Var(a_G)·λ_R/(1 − κ_q)², with spread s_res/(1 − κ_q). So:

    efficient rule:      y = κ_q·T − κ_q·Var(a_G)·λ_R/(1 − κ_q) + ψ·s_res ,
    stable dial (s = t):  y = κ_q·T + ((1 − κ_q)·Var(e_G) − κ_q·Var(a_G))·λ_R + ψ0·s_res .

Both have the coefficient κ_q on T, as claimed. Both carry a constant the text omits, and the two dials differ by
s_res·λ_R/(1 − κ_q), which depends on the cluster. So "the efficient rule and the rule with a stable dial are the
same rule" holds within a cluster and not at one price across clusters. Checked in population (`ref2_a44.txt`):
y = κ_q·T + ψ0·s_res without the constant keeps a row at the threshold with probability 0.93 / 0.80 / 0.76 at
ψ0 = 0 on iNaturalist 12 / yahoo / LAION 64 (and 0.99 / 0.97 / 0.96 at ψ0 = 1); with the constant, 0.46 to 0.49,
0.82 to 0.84 and 0.97 to 0.98 at ψ0 = 0, 1, 2 in every setting and at both heights. The law of s given a high
midpoint (`ref2_a45b.txt`) is reached slowly: among the top 0.1% of rows by midpoint the constant is −1.31 where
the limit is −1.43, and the spread 0.46 where the limit is 0.54. The two dials differ by 1.2, 1.4 and 2.0 spreads of
the scatter in three of the settings.

**Item 5 ("under A4.4 the kept share is P(g ≤ ψ0) on every index"): wrong in the author's own output.**
`out/a4_resemble.txt` gives the ψ0 needed for a kept share of 0.9 with κ = κ_q as −0.91, −0.70, 0.03, 0.18, 0.06 on
the five settings, and with κ = √(1 − f) as 0.18, 0.71, −0.31, −0.14, 0.10. A dial with one meaning would give one
number. The cause is the omitted constant. "κ = 1 − f keeps least where the slope over √(1 − f) is largest" cannot be
checked: A4.0 gives that row as a range without indexes.

**Efficiency statements against the record.** "Within 7% of the best": the best is κ = 1 − f, the best of the rules
tried; correct for the five unfiltered indexes (−4% to +7%). "The plain rule is not efficient" is measured on LAION
only (7 to 18% more rows than κ = 1 − f); on iNaturalist and yahoo it is recorded level with κ = 1 − f. Compounded,
the proposed width against a re-dialled plain rule is about 0 to 4% fewer rows on three indexes and about 0 to 15%
on LAION. That comparison is not itself on record. Item 4 (the two widths differ in tilt; ψ0 larger by
ζ·(1 − √(1 − f))/√f, 1.6 to 2.2 at ζ = 3) checks, and it is consistent with the record: κ = 1 − f at ψ0 = 4 keeps
0.90 to 0.96 where this width at ψ0 = 2 keeps 0.92 to 0.98.

## 5. A4.6 and A4.7

**A4.6 (measuring the slope): proposals; two statements stronger than shown.**

- Item 2, "ρ̂_es is a floor for m": not proved. It is one comparison (yahoo, 0.56 against 0.99, from two different
  dumps) and the direction model C gives. A regression concave in the upper tail would put the neighbours' slope
  below the bulk's. The sampling error (1 − ρ²)/√m is the normal-theory value: 0.12 at ρ = 0.56 and 0.09 at 0.7 for
  32 rows.
- Item 3: the arithmetic checks (b of 1.35 to 1.52 gives κ of 0.26 to 0.34). By item 3 of this report the
  correction also needs the constant, which a slope does not give.
- Item 4 (at build, from corpus rows): unmeasured. Add: exclude the row itself; fit in the units the reader will
  use (see item 7, the slope is not unit-free); by item 4 above an index-level slope is a centre, and a stable
  dial wants an upper quantile.
- Item 5: κ_q for a corpus row as its own query is ‖x − x̂‖²/‖x − m_c‖² when the error is orthogonal to the
  reconstruction. Checks. This is the variance-share reading that A4.4 item 2's formula contradicts.

**A4.7, the sample's error: correct after a small repair.** The three parts are right. The formula for ν² leaves
out two terms: the dependence of √f̂ on τ̂/σ̂, and the covariance between τ̂/σ̂ and τ̂. To first order, with
c1 = ζ − ψ0·(1 − f)/√f and c2 = ψ0·√f,

    ν² = ( c1²·(1 − ρ_es²) + c2²/2 + c1·c2·(1 − ρ_es²) + 1 ) / (m·f) ,

about 20% above the text's. Against the author's simulation at ψ0 = 2 (`ref2_a45.txt`): text 0.958, full 0.954,
simulated 0.953 at ζ = 3 and 32 rows; 0.947, 0.940, 0.941 at ζ = 5, f = 0.92. The quoted kept shares are the
simulated ones and stand. In the ψ_eff variant the term (1 − f)/f should be 1 (the mean residual); immaterial.
Hypotheses: normal rows, the neighbour not in the sample. The recorded 10% to 90% range at 32 rows (0.83 to 1.29)
is wider on the upper side than the normal 0.84 to 1.16, so ν² is understated on real rows; A4.8's last bullet
says as much. For the later rule (height from the whole cluster, τ sampled) ν² = (ζ + ψ0·√f)²/(2·m·f): the same size
at ρ_es = 0.56 and larger at 0.7 (0.931 against 0.940 kept at ζ = 5, 32 rows), because the errors of τ̂ and σ̂ no
longer partly cancel. "Beside the order and the stop", third bullet ("a visible set needs no separate treatment,
as long as the sample is of visible rows"): contradicted by the later measurement (item 7).

**A4.7, the identity at a randomised level: no error found.** With N = a_R − s, Lemma 3.1 gives E_ω[N | stored
scalars] = 0 and E_ω[N² | stored scalars] = v_R, blocks independent, so E[a_R] = s, E[(a_R − a)²] = e² + v_R and
E[(a_R − a)·a_R] = e·s + v_R, for any fixed a, e, s. The variance of a row's squared-error estimate, 4e²v_R + 2v_R²
beyond Var(e²), and the factors 1.2 and 1.6 check. Simulated with Haar rotations and one-bit codes
(`ref2_a47.txt`): all three hold within sampling error, and the sample's τ² and σ_c² come out at 1.001 and 1.000 of
the truth. Hypotheses, none stated in A4.7:

1. The estimate uses the lemma's exact scale ‖y‖²/⟨ō, Ry⟩. The index stores the estimator's scale rounded up. A
   scale high by η adds η·Δ_j·y_j to a_R: not mean zero, and correlated with e. In the simulation 3% gives τ² high by
   7%. That is the wide side.
2. The code is a function of the row's own rotated block only, and the query does not depend on the seed (the
   conditions chapter 3 attaches to the lemma).
3. v_R needs ‖P_(y_j⊥)Δ_j‖, which the reader cannot compute without the row. It has ‖Δ_j‖ (that is sd_R²) or it can
   use the level's own estimate: ‖Δ_j‖² − (Δ_j·ŷ_j)²/‖y_j‖², biased by a term of relative size tan²φ_j/d_j. In the
   simulation that estimate gives τ² at 1.003 of the truth.
4. The rotations are shared by every row, so the errors of two rows are not independent over the seed. Each
   identity is per row and survives. The "rows needed" count assumes independence. In the simulation the errors of
   two rows correlate at ±0.02 for ordinary pairs and 0.97 for a near-duplicate pair, so the count is right
   unless the sample holds near-duplicates. The uncentred second moment is exactly unbiased; the centred one is
   low by the mean pairwise covariance.

**Direction of the bias.** As the text says, and checked:

- A block uncoded: τ² and σ_c² low, the unsafe side. "Low by the share of the level's error variance in that
  block" assumes that block's part is uncorrelated with the rest of the error. In the simulation the block holds
  39% of τ² and the estimate is 30% of the truth, because the blocks' parts are correlated. μ_c is also shifted, by
  the mean of the block's part, which the text does not mention.
- The stored variance an upper bound (sd_R² for v_R, or stored scalars rounded up): too much is subtracted from
  both τ² and σ_c², by the same amount; τ/σ_c and τ both fall; the unsafe side. The recorded bias of 2 to 6%, low on
  LAION, has this sign. Subtracting nothing errs wide by v_R.

## 6. The later rule: height and f from the whole cluster, τ from the visible sample

**The PI's argument, stated precisely.** Fix a query and a cluster. Draw a row uniformly from the cluster; let s
be its score, e the level's error and V its visibility.

*Lemma.* The law of e given (s, V = 1) equals the law of e given s, for every s that a visible row can have, if and
only if P(V = 1 | s, e) does not depend on e. *Proof.* p(e | s, V = 1) = p(e | s)·P(V = 1 | s, e)/P(V = 1 | s). ∎

So the argument is correct, and it is exactly conditional independence of visibility and error given the score:
true for a filter that selects on this query's score (with or without independent randomness), for a random set,
and for a set that takes or leaves whole clusters. Checked on two synthetic row laws (`ref2_filter.txt`): bottom
quarter, top quarter and a soft filter on score leave the mean and spread of e at each score unchanged to two
decimals.

**What follows, and what does not.**

1. A width built from whole-cluster quantities keeps a visible row of score s with the same probability as
   unfiltered, at the same s and t. The kept share over the visible neighbours is not invariant, because the filter
   moves t and so the heights at which the rule is used.
2. No statistic of the visible rows is invariant. In particular τ, which the recorded rule takes from the visible
   sample: τ_vis² = E[Var(e | s) | V] + Var(E[e | s] | V). Under a linear regression τ_vis/τ = √(1 − ρ_es²·(1 − Var_V(x))):
   0.87 for a quarter by score at ρ_es = 0.56 (0.87 simulated) and 0.79 at 0.7. So the recorded rule is not the
   invariant one. Under a linear regression its width is narrower than the lemma licenses by 13 to 21% for a
   quarter cut on score, the unsafe side. Where the regression steepens into the tail (A4.3 item 5) a top quarter's
   τ_vis is larger than this formula gives.
3. The mean of e over the visible rows is ρ_es·τ·E_V[x], not zero (−0.71τ and +0.72τ for the two quarters in the
   simulation). A4.9's "the sample's mean of e added to the midpoint" counts the height twice once ζ is taken from
   the whole cluster. The quantity to add is the sample's mean of e − m·τ·(s − μ_c)/σ_c, the mean distance from the
   ray, which is zero for a filter on score.
4. The recorded confirmation for the bottom quarter is an identity, not a test. Visible neighbours of a bottom
   quarter lie below their cluster's mean, so max(T, 0) = 0 and the width is ψ0·√f·τ: the plain rule. Rows of 2,160
   against 2,159 say that and nothing about the law of e. The lemma predicts more: at s = t below the mean
   E[e | s] is negative, so the plain rule over-covers there, and a width with a negative height at the bulk's slope,
   (ρ_es·ζ + ψ0·√(1 − ρ_es²))·τ with ζ < 0, should read fewer rows at the same kept share. Not measured.

**For which filters it fails.** Every filter whose visibility, at a given score, still depends on the error, which
is the same as depending on the midpoint. With visibility decided by a row's position g = v·(x − m_c) along a
direction v and normal rows, the condition for the lemma is Cov(g, e | s) = 0, that is

    vᵀ·(Λ_left − β·Λ)·Δ = 0 ,      β = Cov(e, s)/Var(s) = ρ_es·√(1 − f) ,

Λ the cluster's covariance and Λ_left the cross-covariance of a row with its coding error (β = 1 − f when the
midpoint and its error are uncorrelated). It holds for v along the query's offset (a filter on score). For a
direction unrelated to the query the partial correlation is of order 1/√n in an isotropic cluster and larger when
the variance sits in few directions, which is a reason the recorded region sets are within noise. It fails when the filter's direction shares with the query's offset mainly
coded directions or mainly left-out ones:

- a filter on the midpoint, on the code, or on an attribute carried by the well-coded, high-variance directions
  that the query also weighs (a category the query is about). Visible rows on the selected side have lower errors
  at a given score: the width over-covers. On the complement they have higher errors: it under-covers.
- a filter along left-out directions the query weighs. The visible rows' errors are higher at a given score:
  unsafe.
- a filter on the score for a different query, unless that query's offset in the cluster is parallel to this one's.

In the simulation a quarter cut along a direction correlated 0.7 with the part of the midpoint, or of the error,
that the score does not explain moves E[e | s] by −0.73τ or +0.73τ at every score, with the scatter falling from
0.83τ to 0.66τ. The shift is constant in s, so the correction of point 3 (the visible sample's mean distance from
the ray) removes it to first order. That is the one use of the visible sample's mean error that the lemma supports.

## 7. A4.8 and the recommendation, against the record

| recommended or stated | the record | verdict |
|---|---|---|
| (1) every quantity from a sample of 64 visible rows | with heights from the visible rows: 1.4 to 2.5 times the plain rule's rows under an in-cluster bottom quarter, about twice under a top quarter on LAION | **contradicted** for filters that cut a cluster on score |
| A4.7: "a visible set needs no separate treatment, as long as the sample is of visible rows" | the same | **contradicted** |
| the replacement: ζ and f from the whole cluster | LAION unfiltered loses 2 to 3 points and 3 to 17% rows; LAION at 8 groups is 17% above the best rule | "within 7% of the best" is **contradicted** for this variant on LAION at 8 groups |
| (1) the sample's mean of e added to the midpoint | not measured | counts the height twice with whole-cluster heights (item 6) |
| (2) the slope fitted at build, 1.2·ζ for iNaturalist | not measured. With whole-cluster heights and no slope iNaturalist keeps 0.951 at ψ0 = 2 (was 0.918) | the recorded miss falls from 0.082 to 0.049 when only the source of the height changes, so at least part of the 1.2 belongs to the sample's σ_c against the library's; a slope fitted in one and applied in the other would correct twice |
| (3) the sample at a randomised level | measured: bias 2 to 6%, low on LAION; about 10 kB against 256 kB | supported; the low bias is the unsafe side and has the sign of an upper bound used for v_R |
| (4) the cap of form 3 as a warning | not measured | nothing against it |
| (5) not the plain rule; "not efficient"; ψ0 with one meaning | plain rule level with κ = 1 − f on iNaturalist and yahoo, 7 to 18% worse on LAION; under a bottom quarter the new rule *is* the plain rule | efficiency is measured on LAION only. Stability is **not shown**: across the five indexes the plain rule keeps 0.790 to 0.824 at ψ = 4 and 0.88 to 0.93 at ψ = 5; the new rule 0.918 to 0.976 at ψ0 = 2. Both move by about half a unit of their dial. Across the 18 cases the new rule keeps 0.93 to 0.99; the plain rule's range on the same cases is not on record |
| A4.8: a top-by-score set over-covers, "safe and wasteful" | about twice the rows on LAION | confirmed |
| A4.8: bottom-by-score, "nothing above was measured" | 1.4 to 2.5 times the plain rule's rows | now measured; wasteful |
| A4.8: "ψ0 means 1 − ½·exp(−√2·ψ0) at best"; 0.999 needs ψ0 near 4.4 | 99.9th percentile of the scatter 4.6 to 7.5 | **contradicted** (item 4) |
| A4.8 does not list a scatter that widens with height | recorded | missing from the list; it breaks "whatever ζ" |
| A4.8: for neighbours not extreme in their cluster the width ψ0·√f·τ "is right only if ρ_es is small" | | imprecise: under a linear regression the needed width at ζ = 0 is ψ0·√(1 − ρ_es²)·τ ≤ ψ0·√f·τ, so it is never too narrow there and exact when a and e are uncorrelated |
| A4.9: a width one scatter too wide is 1.6 to 2 times the rows, "the scatter is 0.2 to 0.3 of that spread" | √(f·(1 − f)) is 0.27, 0.35, 0.38, 0.45, 0.47 with the recorded f | understated: 1.8 to 2.8 times at a rate of 2.2 |

The two variants fail in different places, and the lemma says why. The whole-cluster height is the right frame
for a cut on score and is only as good as the library's σ_c; the sample's height is exact for σ_c and wrong under a
cut on score. The reader already holds the statistic that tells the two cases apart: a sketch's error in σ_c moves
the pooled scale ρ_q and leaves the shift δ_q near zero (llama: 0.60 to 0.63), and a cut on score moves δ_q by one
to three spreads. That is an observation, not a result.

## Not checked

- A4.2 (the forms for the spread, the selection lemma, the cap) and A4.0's figures against any dump.
- `a4_model.py` and `out/a4_model.txt`; part B of `a4_select.py`.
- The cost statements about the reader (A3.2, A4.6 item 5, A4.7's bytes beyond arithmetic), and anything in the
  Rust code.
- Whether the session's y was centred, which σ_c and τ its x and y used, and what "f from the whole cluster" is
  when τ comes from the visible sample. Three of the verdicts above turn on these.
- The slack calculation of item 4 uses A4.5's own law with an exponential excess of rate 2.2; it is a model, and
  the recorded widening of the scatter with height is not in it.
- Item 6's condition for region filters is for normal rows; the size of the effect on real region sets is not
  bounded here.

## Summary: one line per claim

- **A4.1**: no error found. Four different second moments are in play (ratio of means; the same at one offset;
  mean of the ratio; the centred τ) and the text should say which each width is.
- **A4.3 item 1**: no error found; the "worst case at the threshold" sentence needs the midpoint to be
  stochastically increasing in the score.
- **A4.3 items 2 and 3**: no error found; the refutation of the uncorrelated normal law holds on the record.
- **A4.3 item 4**: no error found. A slope above 1 shows only that E[e | s] is not linear, and only if y was
  centred, which is not recorded.
- **A4.3 item 5**: arithmetic right; "they need a heavier-tailed component" is one sufficient mechanism, not a
  necessity.
- **A4.4, the model**: gap. Its algebra is right; as run it gives y/x of 0.67 to 0.80 where 0.89 to 1.21 was
  measured, has a negative intercept the text does not mention, and cannot reach iNaturalist's 1.2 at tail rate 2.2.
- **A4.4 item 2, the participation-ratio formula**: correct after repair (all-or-nothing coding only; otherwise
  (tr D/tr Λ)·√(tr Λ²/tr(DΛ))).
- **A4.4, "independent heavy tails cannot fit"**: conclusion supported by model-free facts (Cov(a, e) > 0, the
  slope of score on midpoint, LAION's rows); two of the four stated grounds are an artefact and a wash.
- **A4.5 item 1**: no error found as algebra; the law holds only for a cluster's top ten rows, and "whatever ζ" fails
  with a scatter that widens with height.
- **A4.5 item 2**: wrong inference (threshold probability compared with a share over all ten). With the slack
  counted the record already showed a tail heavier than Laplace. "0.999 needs ψ0 near 4.4" is contradicted.
- **A4.5 item 3**: first two cases no error found; third case correct after repair (a constant is missing; the
  efficient and the stable dial agree in tilt, not across clusters).
- **A4.5 item 4**: no error found.
- **A4.5 item 5**: wrong in the author's own output (ψ0 for 0.9 kept ranges over a unit across settings).
- **A4.5, efficiency**: "within 7% of the best" correct for the five unfiltered indexes; "the plain rule is not
  efficient" is measured on LAION only.
- **A4.6**: proposals, unmeasured; "ρ̂_es is a floor" not proved; the slope must be fitted in the reader's units.
- **A4.7, the sample's error**: correct after a small repair to ν² (two terms missing, about 20%).
- **A4.7, the identity**: no error found, verified on simulated rotations; four hypotheses unstated (exact
  estimator scale, the lemma's conditions, v_R not computable by the reader as written, shared rotations).
- **A4.7, direction of the bias**: no error found; both cases err narrow; the recorded low bias on LAION has the
  sign of an upper bound used for v_R.
- **The whole-cluster height (STATE.md)**: the lemma is true and is exactly "visibility independent of the error
  given the score". It covers ζ and f, not the visible τ or the visible mean error. It fails for any filter that,
  at a given score, still selects on the coded or the left-out part of a row.
- **A4.8 and the recommendation**: recommendation (1) and A4.7's "no separate treatment" are contradicted under
  cuts on score; the whole-cluster replacement is contradicted on LAION at 8 groups; (2) is unmeasured and
  unit-dependent; (3) is supported; (5)'s "one meaning for ψ0" is not shown by the record.
