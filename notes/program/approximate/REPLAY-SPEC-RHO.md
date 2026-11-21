# Replay specification: the width of a level (A4), the checks still open

For the PI to arrange with the visit-order session, on the per-row dumps it already has
(`/home/joe/code/vexact-data/suite/rho-dump/`: iNaturalist at 12 groups and at 1 group, yahoo at 64, LAION at 64 and
at 8; first 30 clusters a query; k = 10). It counts rows and kept shares; nothing is timed. Symbols are those at the
head of section A4 of `CLAIMS.md`.

The first version of this file (`out/REPLAY-SPEC-RHO_first_version.md`) specified the dump and a comparison of the
forms for the spread. That comparison has been run and is recorded in `CLAIMS.md` A4.0, so it is not repeated here:
the per-cluster sample is the spread, and form 1, form 2 pooled per query and the cousin are withdrawn. What follows
is what the measurement left open.

## 1. What is settled, and need not be run again

| question | answer, measured |
|---|---|
| is the derived ρ too narrow on iNaturalist? | by 1.29 at 12 groups and 2.04 at 1 group; about 1 on yahoo and LAION |
| which spread works? | τ from a per-cluster sample scored exactly: actual over it 1.01 to 1.03 |
| is the right spread enough? | no: the plain rule keeps 0.79 to 0.82 at ψ = 4 on every index |
| how does a neighbour's error scale? | y = e/τ against x = (s − μ_c)/σ_c has slope 0.92 to 1.24 through the origin |
| which width? | (ζ + ψ0·√f)·τ keeps 0.92 to 0.98 at ψ0 = 2, within 7% of the best in rows |

## 2. Checks on the dumps as they are

Each needs only columns the dump has (score, midpoint and half-width per level, cluster) and the 64-row sample per
cluster. Five indexes; the 16 B and the 32 B level.

**C1. The scatter.** For true neighbours, g = (e − m·x·τ)/(√f·τ), with m the index's fitted slope. Report its spread
and its upper quantiles (90, 97.7, 99, 99.9%), against the normal's (1.28, 2.0, 2.33, 3.09) and the unit Laplace's
(1.14, 2.18, 2.77, 4.39). Report the same by height (x in 0.5 to 2, 2 to 3, 3 to 4, above 4).
*Claim tested* (A4.5 item 2): the spread is about 1 and the tail near-exponential.
*Refuted if* the spread is outside 0.8 to 1.3, or the 99% point is under 2.4 on yahoo and LAION.

**C2. The bulk's correlation.** Per cluster, ρ̂_es = corr(e, s) over the 64 sampled rows. Report its median by index,
beside √(1 − f) and the neighbours' slope m.
*Claim tested* (A4.3 item 5): √(1 − f) < ρ_es < m, so the regression of e on s steepens into the tail.
*Refuted if* ρ̂_es is within 0.1 of m (then a linear law is enough and ρ̂_es is the slope, free from the sample) or
within 0.05 of √(1 − f).

**C3. The slope of score on midpoint, by height.** Over all rows of the dumped clusters, the slope of s − μ_c on
a − μ_c in bins of the midpoint's rank within its cluster: below the 90th percentile, 90 to 99, 99 to 99.9, above.
*Claim tested* (A4.4 item 4): near 1 in the bulk (1.04 to 1.09) and rising towards 1/(1 − κ), κ = m·√(1 − f), at the top.
*Refuted if* the slope in the top bin is within 0.1 of the bulk's on every index (then a and e are not tied
through a common factor, and the efficient rule has no term in T beyond the bulk's 1 − 1/b).

**C4. The kept share by cluster height.** For y = (ζ + ψ0·√f)·τ at ψ0 = 2, the share of neighbours kept in bins of
their cluster's ζ (under 2, 2 to 3, 3 to 4, above 4).
*Claim tested* (A4.5 item 1, A4.8): flat where the slope is 1; on iNaturalist falling with ζ, to about 0.75 to 0.87
above 4.
*Refuted if* it is flat on iNaturalist, or falls by more than 5 points on yahoo.

**C5. The rules in rows, with the slope known.** Rows to stored vectors to keep 0.9 and 0.95 of the true ten, for
y = (m′·ζ + ψ0·√f)·τ with m′ = 1; m′ = the index's fitted slope; m′ = ρ̂_es per cluster; and for κ = 1 − f and the plain
rule as references. Also the kept share at ψ0 = 2 and 3 for each.
*Claim tested* (A4.5 item 5): with the fitted slope the kept share at a given ψ0 is the same on the five indexes
to within 2 points, at no more rows than m′ = 1.
*Refuted if* the fitted slope does not bring iNaturalist's kept share at ψ0 = 2 to within 2 points of yahoo's, or
costs more than 5% in rows at equal kept share.

**C6. The sample's size.** C5's first rule with 16, 32, 64 and 128 sampled rows, and with the statistics of all the
cluster's rows as the reference.
*Claim tested* (A4.7): the kept share at ψ0 = 2 loses about 2 points at 32 rows and 1 at 64 against exact
statistics at ζ = 3, more for far clusters; ν² = (ζ²(1 − ρ_es²) + 1 + ψ0²f/2)/(m·f).
*Refuted if* 32 rows lose under half a point or over 5.

**C7. Visible sets.** C5's first rule under random-25, region-25 and the in-cluster top-25 and bottom-25 sets, with
the sample drawn from the visible rows and the sample's mean of e added to the midpoint.
*Claim tested* (A4.8): unchanged under random and region sets; over-covering under top-25; unknown under
bottom-25.
*Refuted if* the kept share at ψ0 = 2 under region-25 is more than 3 points under the unfiltered one.

## 3. Checks that need more than the dump has

**D1. κ_q.** Per (query, cluster, level), κ_q = Σ_i ρ_i·u_i·w_i/Σ_i u_i·w_i, with u = Vᵀ(q − αm_c), w = V⁻¹(q − αm_c) and ρ_i the
share of coordinate i the level leaves out (1 if uncoded, ν_i/λ_i if coded). Then, per neighbour, r against κ_q,
√(1 − f) and 1 − f: the three correlations, and the partial correlation of r with κ_q at a given f.
*Claim tested* (A4.4): r follows κ_q.
*Refuted if* the partial correlation is under 0.3 on yahoo and iNaturalist. Then the tendency r ≈ m·√(1 − f) stands
as an empirical rule without this model, and A4.6 item 5 is dropped.

**D2. The slope at build.** For 300 corpus rows per index used as queries, their true ten within their own group
by brute force, and the slope of y on x fitted as for the test queries.
*Claim tested* (A4.6 item 4): it reproduces the test queries' slope to within 0.1 on the four in-distribution
indexes; on LAION it shows what images against images give, against 0.92 for text.
*Refuted if* it is off by more than 0.15 on iNaturalist, where it matters.

**D3. The randomised level as the sample.** For the sampled rows, the estimate a_R and sd_R at the first randomised
level where every block is coded. Compare μ̂, σ̂², τ̂² and ρ̂_es from (a_R, v_R) by the identities of A4.7 with those
from the exact scores, per cluster.
*Claim tested*: the ratios have median 1 within 2%, and a spread larger than the exact sample's by the factor
√(1 + 2v_R/τ² + v_R²/τ⁴).
*Refuted if* τ̂² from the identity is low by more than 5% in the median (an uncoded block, or sd_R² used where v_R
is meant), or the rows needed exceed twice the exact sample's.

**D4. The cap.** λ_max of the second-moment matrix of the weighted errors' directions, per cluster and level, from
the rows' weighted errors. Report the cap over the derived ρ by index.
*Claim tested* (A4.2, A4.8): it is far above the derived ρ on iNaturalist (1.9 times or more in the constructed
model) and within 1.4 on yahoo and LAION at 64 groups, so that it flags at build the indexes where the derived ρ
and a slope of 1 fail.
*Refuted if* it does not separate iNaturalist from the others.

**D5. Glove.** The whole of section 2 on glove, whose scores are close to normal.
*Claim tested* (A4.4): the neighbours' slope there is the bulk's ρ̂_es, well under 1.
*Refuted if* the slope is near 1 on glove too. Then the slope does not come from a heavy-tailed shared component,
and the model of A4.4 is wrong in its mechanism although the width stands.

## 4. How the results decide

- C1, C4 and C5 fix what ψ0 means and whether the fitted slope is worth carrying: if C5 passes, the width is
  (m·ζ + ψ0·√f)·τ with m per index; if not, m = 1 and iNaturalist's dial is read 0.6 lower.
- C2 and D2 decide where m comes from: the sample's correlation if C2 is refuted in the first sense, the build if D2
  passes, otherwise a constant.
- C6 and D3 decide what the sample costs: 64 rows at a randomised level if D3 passes, 64 stored vectors otherwise.
- C3, D1 and D5 decide whether A4.4's model is the reason or only a description. Nothing built depends on them
  except the per-query κ_q of A4.6.
- C7 decides whether the width can be used under a visible set as it is.
- D4 decides whether the build can warn.

## 5. Limits

Everything in A4 that is called a model was computed on synthetic rows. The dumps hold the first 30 clusters and
use the true 10th best as the threshold, so they measure what a level loses and passes, not the stop or the
running threshold. 100 queries an index put about 1.5 points of noise on a kept share near 0.9 and half a point
near 0.98.
