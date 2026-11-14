# Group certificates: what a grouping can buy, and the metric its bound should use

Follows `DESIGN.md` and `proofcheck/CONSOLIDATED_PROOFS.md` (cited below as "the proofs"). Counts
from numpy on the 1M BioCLIP-2 sample of `fetch.py` (n = 768), the 369 real text queries of
`agent2/encode_text.py` and 100 item queries, answer sizes 0.01 / 0.1 / 1 / 5 % throughout. Every
bound was checked against the exact score on every item, about 8·10⁹ interval checks per run:
0 violations in all runs. Nothing is timed. The proofs in section 2 have not been refereed.

Status: the dual basis of section 2.4 has been checked in the native version 2 prototype
(`dualproto/REPORT.md`): bytes read on the quantised code fall 9–30 % for items (data-only fit) and
25–34 % for text (fit including text prompts), with 0 violations. The sort key of section 3.3 has not
been run natively.

## 1. Question and answers

The framing: find a grouping with a small set of representatives whose distance from the query
bounds each member's distance in both directions as tightly as possible, testing as few
representatives as possible. An oracle that knows each group's exact extremes of q·x leaves 1–45 %
of items for text where the design leaves 57–99 %, so the question was how few bits per member a
fresh query must read to certify a group's extremes.

1. **The oracle is not a reachable target.** A summary of S bits shared by M members is worth at most
   S/M bits per member (Corollary 2 of the proofs), and the grouping itself at most log₂K bits per
   member (section 2.2). Measured: an idealised summary that knows a group's exact extremes in its
   first 64 local coordinates still leaves 89–100 % of items in undecided groups for text. The same
   64 coordinates held per member leave 16–72 %.
2. **More groups do not help.** Sixteen times as many groups (4 more bits of label) saves a text
   query 4.5 of 45 coordinates per item and costs 11.4 coordinates per item in centres.
3. **The bound is taken in the wrong metric, and that is worth 30–44 %.** The tightest
   representative bound uses the norm M = Σ_Q⁻¹ # Σ_R, the matrix geometric mean of the inverse
   query-offset covariance and the residual covariance (section 2.4). The design's weighting is
   diagonal in the residual PCA basis; Σ_Q is far from diagonal there. With M fitted to held-out text
   prompts, an idealised per-member read for text falls from 45 / 65 / 117 / 202 to 25 / 39 / 76 / 142
   coordinates per item. The best diagonal weighting fitted to the same prompts reaches
   43 / 63 / 114 / 198.
4. **One sortable norm in that metric replaces the support table.** Sorting a group by ‖r‖ in M⁻¹
   instead of by x·m̂ leaves, for a mixed fit, 51 / 62 / 81 / 95 % of items for text (sorted range
   plus support table: 75 / 83 / 94 / 100) and 15 / 24 / 50 / 87 % for items (14 / 23 / 48 / 84),
   with no table.

## 2. Theory

Notation as in the proofs, section 0: x = m + r, offset Δ, Σ_Q and Σ_R the covariances of Δ and r,
γ₁ ≥ γ₂ ≥ … the eigenvalues of Σ_Q^½ Σ_R Σ_Q^½, g = Σ_Q^(−½) Δ, r′ = Σ_Q^½ r.

### 2.1 One representative

For unit q, x, p with angles φ = ∠(q, p) and θ = ∠(x, p), write q = cos φ·p + sin φ·u and
x = cos θ·p + sin θ·v with u, v unit and orthogonal to p. Then

q·x = cos φ cos θ + sin φ sin θ·(u·v),

and since u·v takes every value in [−1, 1], the tightest interval from φ and θ alone is
[cos(φ + θ), cos(φ − θ)], of width 2 sin φ sin θ. The slack is a product of the query's distance
from the representative and the member's. Neither factor can be made small: the member side falls
as K^(−0.053) (section 3.2) and the query side needs a representative near a fresh query.

### 2.2 A grouping is worth at most log₂K bits per item

**Proposition 1.** Let the item x have a density, let V₁, …, V_K be a partition of space with fixed
centres m_ℓ, and let r = x − m_ℓ for x ∈ V_ℓ. Hold the offset law fixed and write x′ = Σ_Q^½ x. For
any encoder of r with message c and any sound certificate, for every k-dimensional subspace S,

E h ≥ E‖g_k‖ · ω_k^(−1/k) · exp((h(P_S x′) − H(ℓ) − H(c | ℓ))/k),  with H(ℓ) ≤ ln K.

*Proof.* Apply Theorem 1 of the proofs within each cell. Given ℓ, r′ is a translate of x′, so
h(P_S r′ | ℓ) = h(P_S x′ | ℓ) = h(P_S x′) − I(P_S x′; ℓ) ≥ h(P_S x′) − H(ℓ). Average over ℓ and use
the convexity of exp. ∎

So the label enters the floor exactly as B does: any K-way grouping, by any rule, lowers the floor
by at most the factor K^(−1/k). The measured radius law is this statement with k ≈ 19.

A k-means partition of N ≫ K points fits the hypothesis up to the dependence of a centre on one
member, which is of order one over the group size.

### 2.3 Summaries the query chooses

Corollary 2 covers a summary read by every query. An anchor table lets the query choose one of A
rows.

**Proposition 2.** Let the query choose a = a(Δ) among A read sets, the choice not depending on the
data. Under read set a let w_a(g) be half the width, in direction g, of the set of residuals
consistent with everything read, in the coordinates r′. Let every residual satisfy ‖r‖ ≤ ν, and let
g be standard Gaussian. Then

E w_{a(g)}(g) ≥ min_a E w_a(g) − ν·√(2 λ_max(Σ_Q) ln A),

expectations over the data and g.

*Proof.* Φ_a(g) = E_data w_a(g) is an average of functions that are Lipschitz with constant
L = ν‖Σ_Q^½‖, because each is half the sum of two support functions of a set inside the ball of
radius L. Gaussian concentration gives E exp(λ(EΦ_a − Φ_a)) ≤ exp(λ²L²/2), hence
E max_a (EΦ_a − Φ_a(g)) ≤ L√(2 ln A). Then E Φ_{a(g)}(g) ≥ E min_a Φ_a(g) ≥ min_a EΦ_a − L√(2 ln A). ∎

Each min_a E w_a is bounded below by the floor at the rate read under a. With offsets spread over
D directions the correction is the Cauchy–Schwarz slack times √(2 ln A / D), the cosine of the angle
to the nearest of A random anchors, so a table helps only when ln A is comparable to D.

On this data the statement is weak: λ_max(Σ_Q) is 18 % of the trace for text (`offsets.py`), so the
correction exceeds the slack at A = 1,000. It does not cover choices that depend on bits already
read.

### 2.4 The best norm for a representative bound

For any positive definite M, |Δ·r| ≤ ‖Δ‖_M · ‖r‖_{M⁻¹}, with ‖v‖²_M = vᵀMv.

**Proposition 3.** E‖Δ‖²_M · E‖r‖²_{M⁻¹} = tr(MΣ_Q)·tr(M⁻¹Σ_R) ≥ (Σᵢ √γᵢ)², with equality at

M = Σ_Q^(−½) (Σ_Q^½ Σ_R Σ_Q^½)^½ Σ_Q^(−½) = Σ_Q⁻¹ # Σ_R.

*Proof.* Put P = Σ_Q^½ M Σ_Q^½ and B = Σ_Q^½ Σ_R Σ_Q^½. Then tr(MΣ_Q) = tr P and
tr(M⁻¹Σ_R) = tr(P⁻¹B). By Cauchy–Schwarz for the Frobenius inner product,
tr B^½ = tr(P^½ · P^(−½)B^½) ≤ √(tr P) · √(tr(P⁻¹B)), with equality when P is a multiple of B^½. ∎

In coordinates: with V the eigenvectors of B, the pair g″ = Γ^¼ Vᵀ Σ_Q^(−½) Δ and
r″ = Γ^(−¼) Vᵀ Σ_Q^½ r satisfies g″·r″ = Δ·r exactly, both have uncorrelated coordinates, and
E g″ᵢ² = E r″ᵢ² = √γᵢ. Any certificate written for an orthonormal basis (prefix codes, block norms,
sorted ranges) carries over unchanged, since the bound only uses the identity and Cauchy–Schwarz.

This is the basis in which the floor of the proofs is stated. With no bits read, Corollary 1 gives
E h ≥ κ_k (γ₁⋯γ_k)^(1/2k) ≥ k·(geometric mean of √γ₁ … √γ_k), and the M-ball gives an rms slack of
Σ √γᵢ, so the ball is within the gap between the arithmetic and geometric means of the √γᵢ of the
best any zero-bit certificate can do. The design reaches the floor's coordinates only when Σ_Q and
Σ_R commute, because its weights are diagonal in the eigenbasis of Σ_R.

### 2.5 Why the extremes were the wrong target

A group is cut only if its largest member clears the threshold by the slack, and the largest of
~1,000 members sits more than three within-group standard deviations above the group mean (for
Gaussian scores). A member is
decided when its own score clears. So certifying extremes to ±δ asks for more precision than
deciding the members one by one, and the oracle's count is neither a floor nor a target for a
scheme that reads per member.

## 3. Measurements

Setup (`scripts/`): the branch's 20 row groups, unit-normalised; spherical k-means, K = 1,000;
three nested 4-way splits; 64 groups of clusters, each with its own basis; the 369 test prompts of
`agent2/encode_text.py` and 100 corpus items. "Coordinates" are exact local coordinates in dyadic
blocks (1, 1, 2, 4, …, 256, 256); what is not read is bounded block by block by the product of the
two block norms. A per-member read takes blocks until the member is decided.

### 3.1 Group summaries against the oracle (K = 1,000, PCA basis; % of items not decided)

| | Text | Item |
|---|---|---|
| Sorted range on x·m̂ | 80.2 / 86.6 / 95.6 / 99.7 | 30.9 / 44.8 / 72.7 / 97.7 |
| Sorted range and support table (1,000 anchors) | 74.8 / 82.7 / 94.1 / 99.6 | 13.8 / 23.2 / 47.8 / 84.2 |
| Group summary, block norms only | 99.9 / 100 / 100 / 100 | 97.9 / 99.2 / 100 / 99.9 |
| Group summary, exact extremes in 16 coordinates | 98.8 / 99.7 / 100 / 100 | 88.3 / 93.2 / 98.6 / 99.7 |
| Group summary, exact extremes in 64 coordinates | 89.0 / 93.9 / 98.9 / 99.9 | 32.9 / 49.9 / 78.4 / 98.6 |
| Group summary, exact extremes in 256 coordinates | 19.7 / 30.0 / 59.9 / 91.1 | 1.31 / 3.76 / 17.1 / 62.6 |
| Per member, 16 coordinates | 45.5 / 57.3 / 77.2 / 93.4 | 5.79 / 13.7 / 39.1 / 79.4 |
| Per member, 64 coordinates | 15.9 / 24.1 / 45.1 / 72.4 | 0.51 / 2.12 / 12.1 / 44.5 |
| Per member, 256 coordinates | 0.56 / 1.54 / 5.26 / 14.6 | 0.01 / 0.08 / 0.69 / 4.30 |
| Oracle, extremes to ±0.05 | 9.38 / 18.5 / 49.7 / 89.7 | 2.13 / 4.35 / 17.2 / 66.2 |
| Oracle, extremes to ±0.02 | 2.89 / 7.49 / 27.1 / 66.1 | 1.26 / 2.59 / 9.68 / 43.6 |
| Oracle, exact extremes | 0.97 / 3.32 / 15.1 / 44.8 | 0.66 / 1.57 / 6.54 / 28.3 |

The group-summary rows are upper limits for their family: they assume the exact support function of
the group in those coordinates, which no compact summary provides.

Between-group share of a query's score variance: text 0.750, items 0.841. A text query's scores
have standard deviation 0.040 (0.020 within groups); the plain Cauchy–Schwarz slack is 0.296 and
the block slack 0.250, against a threshold 0.074–0.200 above the median.

### 3.2 Finer groups (PCA basis)

| Groups | Median size | rms residual | Centres, coordinates per item | Text, coordinates per item | Item, coordinates per item |
|---|---|---|---|---|---|
| 1,000 | 838 | 0.3765 | 0.8 | 44.9 / 64.9 / 116.7 / 202.0 | 4.2 / 10.5 / 37.0 / 112.3 |
| 3,997 | 203 | 0.3509 | 3.1 | 43.1 / 63.0 / 114.9 / 200.5 | 3.8 / 9.7 / 35.8 / 110.8 |
| 15,880 | 48 | 0.3288 | 12.2 | 40.4 / 59.9 / 111.5 / 197.2 | 3.2 / 8.8 / 33.9 / 108.2 |
| 58,750 | 10 | 0.3034 | 45.1 | 35.5 / 54.3 / 104.7 / 189.5 | 2.6 / 7.4 / 30.7 / 102.8 |

The residual falls as K^(−0.053). Every step costs more in centres than it saves per member. Even
at ten members per group, exact extremes in 64 coordinates leave 39 / 52 / 77 / 94 % of items for
text.

### 3.3 The geometric-mean metric (K = 1,000; ridge 0.1 of the mean eigenvalue of Σ_Q)

Σ_Q is fitted to offsets of pseudo-queries from cluster centres: 3,000 corpus items ("data only"),
the 2,206 held-out prompts of `agent2/encode_train.py` ("text"), or the mean of the two normalised
("mixed"). The test queries are in none of them. Coordinates read per item by a per-member read:

| Basis | Text | Item |
|---|---|---|
| PCA of the residuals | 44.9 / 64.9 / 116.7 / 202.0 | 4.2 / 10.5 / 37.0 / 112.3 |
| PCA, best diagonal weights for text | 43.2 / 62.7 / 113.6 / 197.9 | 4.3 / 10.5 / 37.0 / 112.4 |
| Geometric mean, data only | 37.7 / 54.9 / 99.9 / 175.9 | 2.8 / 6.8 / 23.0 / 68.8 |
| Geometric mean, text | 25.1 / 38.6 / 75.8 / 141.5 | 4.7 / 10.6 / 34.3 / 98.6 |
| Geometric mean, mixed | 25.3 / 38.8 / 75.8 / 142.1 | 3.0 / 7.3 / 24.9 / 74.6 |

By prompt kind, mixed fit against PCA, at the 0.01 % answer: species names 11.9 against 27.0, full
taxonomy 11.2 / 25.9, bare names 45.4 / 68.9, common names 14.6 / 29.0, higher taxa 21.0 / 39.4,
free descriptions 66.5 / 104.7, species not in the sample 43.6 / 64.2.

With nothing read per member (% of items not decided):

| | Text | Item |
|---|---|---|
| Sorted range on x·m̂ (as built) | 80.2 / 86.6 / 95.6 / 99.7 | 30.9 / 44.8 / 72.7 / 97.7 |
| Sorted range and support table | 74.8 / 82.7 / 94.1 / 99.6 | 13.8 / 23.2 / 47.8 / 84.2 |
| Sorted range on ‖r‖ in M⁻¹, data only | 61.1 / 70.8 / 85.7 / 97.0 | 13.3 / 22.1 / 47.0 / 83.8 |
| Sorted range on ‖r‖ in M⁻¹, mixed | 51.4 / 62.4 / 80.5 / 95.1 | 14.6 / 23.9 / 49.8 / 86.5 |
| Group ball in M⁻¹ alone, data only | 98.2 / 99.2 / 99.9 / 100 | 68.0 / 76.7 / 89.4 / 99.1 |

The sorted range uses one key per member, ‖r‖ in M⁻¹, and the group's largest and smallest m·r.

### 3.4 K-means in the same metric (data-only fit)

Assigning each item to the centre nearest in the M⁻¹ norm (`prep_m.py`), instead of the centre with
the largest cosine, changes nothing that a member-level bound sees:

| | Text | Item |
|---|---|---|
| Coordinates per item, cosine k-means | 37.7 / 54.9 / 99.9 / 175.9 | 2.8 / 6.8 / 23.0 / 68.8 |
| Coordinates per item, k-means in M⁻¹ | 38.0 / 55.2 / 100.3 / 176.6 | 2.9 / 6.9 / 23.2 / 69.2 |
| Sorted range on the norm, cosine k-means | 61.1 / 70.8 / 85.7 / 97.0 | 13.3 / 22.1 / 47.0 / 83.8 |
| Sorted range on the norm, k-means in M⁻¹ | 61.0 / 70.6 / 85.6 / 97.0 | 13.1 / 21.9 / 46.5 / 83.2 |
| Group ball alone, cosine k-means | 98.2 / 99.2 / 99.9 / 100 | 68.0 / 76.7 / 89.4 / 99.1 |
| Group ball alone, k-means in M⁻¹ | 94.7 / 97.3 / 99.6 / 100 | 54.9 / 65.8 / 83.7 / 98.4 |

Only the group ball moves, and the sorted range already does better than the ball. The rule that
forms the groups matters as little as Proposition 1 says.

### 3.5 Where the per-member read spends its coordinates (`margin.py`, `tables/margin.txt`)

200k-item random sample, PCA bases fitted on it, 62 text queries (every sixth) and 40 item queries.
Distances are in units of the query's own score standard deviation (sd: 0.040 for text, 0.068 for
items). "Near" is within 0.5 sd of the threshold; "bulk" is 2–8 sd from it.

| Answer size | Threshold above median | Density at t, per sd | Coordinates per item | Cost share, near | Cost share, bulk |
|---|---|---|---|---|---|
| Text 0.01 % | 4.88 sd | 0.0045 | 44.6 | 3.6 % | 86.6 % |
| Text 0.1 % | 4.16 sd | 0.0073 | 64.4 | 6.0 % | 78.5 % |
| Text 1 % | 2.98 sd | 0.024 | 114.0 | 12.8 % | 55.9 % |
| Text 5 % | 1.81 sd | 0.067 | 195.5 | 18.6 % | 23.2 % |
| Item 0.01 % | 5.77 sd | 0.0047 | 5.5 | 21.8 % | 64.8 % |
| Item 0.1 % | 4.92 sd | 0.0070 | 13.0 | 23.0 % | 66.4 % |
| Item 1 % | 3.23 sd | 0.020 | 45.6 | 20.0 % | 58.2 % |
| Item 5 % | 1.66 sd | 0.088 | 131.3 | 28.1 % | 16.9 % |

- With nothing read, the interval is ±6.1 sd for text and ±4.5 sd for items, wider than the
  threshold's distance from the median. Almost every item starts undecided and is read until its
  slack is below its own margin, at a cost that rises roughly as one over the margin.
- So cost tracks how far the bulk sits from the threshold, not the height of the score density at
  t: from the 0.01 % to the 5 % answer the density rises 15× and the cost 4.4× (text).
- The score tail is heavier than Gaussian: the 0.01 % threshold sits 4.9 sd above the median, where
  a Gaussian puts it at 3.7.
- The ratio that predicts cost is the zero-read half-width over the threshold's height above the
  median. It is above 1 for text at every answer size here.

### 3.6 Tolerance bands (`band.py`, `tables/band.txt`)

`FINDINGS.md` section 2 allows S(t) ⊆ R ⊆ S(t − d). Exclusion then needs an upper bound below t and
inclusion a lower bound at or above t − d:

| Item | Decided once the half-width is below, exact | With tolerance d |
|---|---|---|
| Non-match, u below t | u | u |
| Match, u above t | u | u + d |
| Inside [t − d, t) | its distance from t | d/2 |

The bulk sits below t, so **a tolerance below the threshold does nothing for it** (argued from the
rule; consistent with `agent2/REPORT.md`: text 3.17 % → 3.15 % read at d = 0.02). The shell argument
of `sublinear/REPORT.md` section 4 is unchanged, so neither are the exponents.

A band above the threshold (S(t + d′) ⊆ R) moves every bulk item d′ further from the line it must
clear. Interpolating the costs of section 3.5 between answer sizes, each quarter sd of d′ saves about
11–12 % of coordinates for text and 15–22 % for items. That is exactly a higher threshold with a
wider band, and it gives up the guarantee on many matches, because matches crowd just above t.
Share of the answer within b sd above the threshold (200k sample, all 469 queries):

| Answer size | Text, b = 0.25 / 0.5 / 1 | Item, b = 0.25 / 0.5 / 1 | Best match above t, text / item |
|---|---|---|---|
| 0.01 % | 69 / 89 / 99 % | 77 / 90 / 97 % | 0.68 / 0.85 sd |
| 0.1 % | 54 / 78 / 95 % | 63 / 80 / 93 % | 1.42 / 1.54 sd |
| 1 % | 40 / 63 / 85 % | 43 / 60 / 80 % | 2.66 / 2.88 sd |
| 5 % | 28 / 47 / 72 % | 27 / 42 / 63 % | 3.84 / 4.35 sd |

Recommendation: keep the guaranteed-inclusion edge at t and d = 0 as the default. A caller who wants
a cheaper query can raise t and knows what they asked for.

## 4. What follows for the design

- Stop looking for a better grouping or a richer group summary. Both are capped by sections 2.2
  and 2.3 of this report and Corollary 2 of the proofs, and the caps are small on this data.
- Replace each group's PCA basis and diagonal profile by the dual pair of section 2.4. Stored size
  and query work are the same: one n × n transform per group on each side, and centres stored in the
  query-side coordinates so that ‖Δ‖_M costs one more dot product per cluster.
- Sort each cluster by ‖r‖ in M⁻¹. This matches the support table for items and beats it for text,
  and removes the table, whose size at 10⁹ is an open problem in `review/REPORT.md` section 1.3.
- The data-only rule keeps about two fifths of the gain available to text. A fit that mixes
  pseudo-queries with a query log recovers the rest and keeps most of the item gain.

## 5. Limits of this evidence

- Coordinates are exact. A real stream quantises r″; the certificate carries over (section 2.4) but
  the byte counts were not measured here. The dead list of `FINDINGS.md` records the same transform
  ("simultaneous diagonalisation") as hurting beyond 16 bytes. I conjectured that `exp5.py`'s
  clipping quantiser caused that loss; `dualproto/REPORT.md` section 6 refutes this. Clip rates are
  equal in both bases, and the loss comes from `exp5.py`'s per-group, nearest-cluster fit of Σ_Q; the
  fit used here wins at every budget in the same quantiser.
- One dataset, one dimension, one ridge value, one Σ_Q for all groups.
- Test and training prompts come from the same templates (`review/REPORT.md` section 1.6). The
  hand-written free descriptions and the absent species gain 21–37 % under the mixed fit, the
  species-name prompts 33–56 %.
- The transforms are not orthogonal. The float32 allowance on every bound is 2·10⁻⁴; a fixed-point
  definition is needed as for the existing bases.
- Propositions 1–3 are short but unrefereed. Proposition 2 is numerically weak here.

## 6. Files

Scripts take the working directory as their first argument (the original runs used a scratchpad
path); the data files they read are listed in the README's "Data" section.

- `scripts/fetch.py` (the 1M sample and its metadata), `fetch_text_tower.py`, `encode_text.py`,
  `encode_train.py` (copies of the `agent2/` scripts, repointed).
- `scripts/prep.py`: the index (k-means, three nested 4-way splits, 64 groups).
- `scripts/measure.py`: all counts of sections 3.1–3.4. `MODE` (`pca` or a Σ_Q file), `DIAG`, `EPS`
  and `INDEX` select the basis, `NPROC` and `BQ` the parallelism.
- `scripts/sigmaq.py` (Σ_Q from items, held-out prompts, and their mix), `offsets.py` (spectrum of
  Σ_Q), `prep_m.py` (k-means in the M-norm), `report.py` and `compare.py` (tables),
  `margin.py` (section 3.5), `band.py` (section 3.6).
- `tables/`: the printed tables. `levels012.txt`, `level3.txt` (sections 3.1–3.2); `bases.txt`,
  `opt_*.txt`, `diag_text_e0.1.txt` (sections 3.3–3.4); `offsets.txt`; `margin.txt`, `band.txt`.
- `logs/`: build and measurement logs.

The raw JSON results (1–3 MB each) are not committed; `measure.py` regenerates them.
