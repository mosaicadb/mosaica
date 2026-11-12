# Exact threshold vector search at billion scale: findings so far

Read this whole file before doing anything. It is the only context you have. Everything here was
measured or argued in one working session; statuses are marked **[measured]**, **[argued]**,
**[from memory]** or **[untested]**.

## 1. The problem

A corpus V of N unit vectors in R^n. N is up to ~1 billion per machine, more by sharding.

**The dimension n is a parameter, not a constant.** The method must generalise across common
embedding sizes, roughly n = 100 to 5,000. Every measurement so far is at n = 768, so how each
result scales with n is open. (The letter d is reserved for the tolerance in section 2.)

A query q (unit vector) and a threshold t. Return R ⊆ V, the items with q·v ≥ t.

Context that constrains the design (the host system is a map database called Mosaica):

- The result must be a plain set (a bitmap over item ids). It is ANDed with a per-viewer bitmap of
  visible items and with other filters. The owner has ruled out three-valued or "maybe" results.
- One machine, a memory cap, NVMe disk. Items are inserted and deleted while serving.
- A threshold test is per item, so shards need no merging.
- Thresholds are user-chosen; result sets can be anywhere from tens of items to ~10% of the corpus.

### The metric to optimise (set by the owner)

> What percentage of the dataset must be read to guarantee that all vectors meeting the threshold
> have been identified, with probability > 1 − ε?

Working definition used for the baseline below (challenge it if you think it is the wrong
formalisation):

- "Dataset" = the full-precision vectors at 2 bytes per dimension (1,536 bytes at n = 768).
- "Read" = every byte touched per query, **including index/code bytes**, divided by N × 2n.
- "Identified" = the returned set R satisfies the guarantee in section 2. Report d = 0 (exact) as the
  primary figure and d > 0 as secondary.
- Extra storage (codes, bases) is allowed but report it; memory that must stay resident matters.

### Current baseline on this metric **[measured, with approximations — see 6.6]**

| Threshold | 0.95 | 0.9 | 0.8 | 0.7 |
|---|---|---|---|---|
| True matches, share of the sample | 0.22% | 0.60% | 1.95% | 9.17% |
| Dataset read, exact (d = 0), deterministic | 0.35% | 0.50% | 1.70% | 5.81% |
| Dataset read, d = 0.01 | 0.32% | 0.46% | 1.49% | 4.91% |
| Dataset read, d = 0.05 | 0.31% | 0.44% | 1.28% | 3.90% |

Reading only the true matches in full would cost the first row. The scheme already beats that at
0.9 and below because most matches are certified from their codes.

## 2. The guarantee

S(τ) = { v ∈ V : q·v ≥ τ }. The index is built with a secret random seed ω. R(q, ω) is returned.

**For every q chosen independently of ω, and every t, d ≥ 0:**
Pr over ω [ S(t) ⊆ R(q, ω) ⊆ S(t − d) ] ≥ 1 − ε.

- d = 0 is exact search with probability 1 − ε. ε = 0 (no randomness) is the deterministic version.
- The event is simultaneous over all items. "For all q with probability 1 − ε" is false.
- Backstop: the deterministic bound always holds, so with certainty
  S(t + Δ) ⊆ R ⊆ S(t − d − Δ), Δ = largest gap between deterministic and randomised half-widths
  among the decisions made.
- Not covered: queries built adaptively from earlier results; floating-point (the intended
  implementation defines similarity on fixed-point integers).

Alternative formulations discussed: (a) define the filter on the stored code ("backward error":
R = { v : q·v̂ ≥ t } exactly) and report the forward tolerance; (b) report d instead of promising
it; (c) recall 1 with a certified precision p.

## 3. Data and how to run things

Directory: `/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/`

- `data/e00.npy … e19.npy`: 20 × 50,000 × 768 float16. BioCLIP-2 image embeddings from
  `imageomics/TreeOfLife-200M-Embeddings` (Hugging Face, folder `bioclip-2_float16`). Each file is
  one contiguous row group of a corpus sorted by taxonomy, from 20 random files of 666. Not
  normalised; normalise to unit length.
- Because the sample is 20 dense slices, match counts as a share of the sample are inflated
  relative to the full 233M corpus. Ratios to the answer size transfer; shares of N do so only
  roughly.
- **Every measurement so far uses corpus items as queries.** Text queries are untested.
- Geometry: the mean vector has norm 0.76, so two random items have cosine ≈ 0.58. Thresholds
  only mean something above that.
- A second dimension is available: the same Hugging Face dataset has a 1,024-dimension set
  (folder `bioclip-2.5-vith14_float16`), and other public embedding sets may be reachable. If you
  fetch anything, keep it to at most 200k vectors and put it in your own subdirectory.
- Scripts (numpy only): `exp2.py` is the canonical harness (clusters, local bases, codes, bounds,
  counting, violation checks). `exp8.py` adds the split randomised bound. `exp10.py` measures
  plain thresholding of the estimate. `two.py`, `wide.py`, `mih.py` are small self-contained
  tests. `results*.json` hold raw counts; `report2.py`, `report4.py` print them.

**Machine rules (shared by several agents): 4 cores, 15 GB RAM.**
- Use at most the 200k subset (`e00, e05, e10, e15`) and at most ~2.5 GB of RAM.
- Set `OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1` on every run.
- Work in your own subdirectory of the directory above. Do not modify existing files. Do not
  touch `/home/user/mosaica`.
- Every experiment must count bound violations (a true match ruled out, or a non-match ruled in)
  and report the count. A bound that is "usually right" is not a bound.

## 4. The construction so far

Notation: item x = m + r, where m is the (non-unit) mean of x's cluster and r the residual.
Δ = q − m.

1. **Clusters.** k-means, ~1,000 items per cluster (K = 1000 on the 1M sample). Used as the anchor
   for coding and for layout, **not** for pruning (see dead list).
2. **The Δ decomposition.** q·x = q·m + m·r + Δ·r. The first term is one number per (query,
   cluster). The second is a stored exact scalar per item. Only Δ·r is uncertain, and both factors
   are short. This alone tightened bounds ~3× over bounding q·e directly.
3. **Local bases.** Clusters are grouped (G = 64 groups); each group has its own PCA basis of the
   residuals. Global → 64 local: ~3× tighter at 96 bytes. 64 → 256: only 12–16% more.
4. **Bound-aware bit allocation.** With λᵢ = residual variance and Qᵢ = query-offset energy in
   dimension i, bits are allocated greedily to minimise Σᵢ √(Qᵢ λᵢ g(bᵢ)), g(b) = distortion of a
   b-bit uniform quantiser on a unit Gaussian. Per-dimension uniform scalar quantisers, 0–8 bits.
5. **Deterministic certificate.** r = γ·code + e. The stored scalars are m·e and the error norms.
   |Δ·e| ≤ min( ‖Δ‖·‖e‖ , ‖DΔ‖·‖D⁻¹e‖ ) with diagonal D = (error variance / Qᵢ)^¼.
   (A free refinement not yet applied in the new variants: e ⟂ code, so Δ can be replaced by its
   component orthogonal to the code.)
6. **Progressive stream.** Each item's code is ordered most-important-first; a query reads a short
   prefix of everything and goes deeper only for undecided items. Every prefix has its own
   certificate. Final level is the full vector.
7. **Split (randomised) certificate.** Rescale coordinates by D₀ = (λᵢ/Qᵢ)^¼ so both sides have
   variance √(Qᵢλᵢ). Group dimensions by bit depth. Within each group apply a secret Haar
   rotation, then quantise. Use the unbiased rescaled estimator (scale = ‖y‖² / ⟨code, y⟩).
   The residual error is ‖y‖·tanφ·‖Δ̃_b‖·u with u a coordinate of a uniform unit vector in
   n_b − 1 dimensions, so it is sub-Gaussian with variance proxy A_b²‖Δ̃_b‖²/(n_b − 1).
   Groups are independent, so h_rand = c·√(Σ_b A_b²‖Δ̃_b‖²/(n_b − 1)), failure ≤ 2·exp(−c²/2)
   per decision. Groups with < 24 dimensions and uncoded dimensions stay deterministic.

## 5. Theory so far

- **Deterministic = ℓ1, randomised = c·ℓ2 [argued, consistent with measurement].** With per-
  dimension contributions aᵢ = √(Qᵢ·λᵢ·g(bᵢ)), the best deterministic bound is ≈ Σ aᵢ and the
  randomised one is c·√(Σ aᵢ²). Gain ≈ √(coded dimensions)/c: nothing for short codes.
- **Deterministic floor [argued, not proved].** A B-bit code confines the residual to a cell; the
  bound in direction Δ is the cell's width in that direction; among bodies of equal volume the
  ball has the least mean width (Urysohn). This gives a floor Σᵢ √min(γᵢ, θ) with γᵢ the
  eigenvalues of (query-offset covariance × residual covariance) and θ set by
  Σᵢ ½·log₂⁺(γᵢ/θ) = B. The scalar code is ~1.2–2× above it (quantiser inefficiency).
- **Scaling law [argued + measured].** If γᵢ ~ i^−α the bound falls as bytes^−(α/2 − 1). Measured:
  bytes^−0.7 up to 96 bytes, bytes^−1.3 beyond. So deciding an item at margin u costs ~u^−1 bytes.
- **Order-optimality of the randomised gain [from memory — verify].** Alon & Klartag (FOCS 2017,
  "Optimal compression of approximate inner products and dimension reduction") give
  Θ(k·log(2 + log N/(ε²k))) bits per point for N points in dimension k when k ≤ log N/ε². That
  implies a randomisation gain of at most about √(k/log N), the same form as √(coded dims)/c.
- **Structured error.** Typical actual error is 5–10% of the deterministic bound, but the worst
  pairs reach it: near-duplicates of the query, where Δ points along the item's own error.

## 6. Measured results (1M vectors unless stated; zero violations in every run)

Thresholds are 0.95 / 0.9 / 0.8 / 0.7 throughout. "Undecided" is per true match.

### 6.1 Cluster pruning is weak
Share of items left after ruling out whole clusters by a spherical-cap bound:
K=1000: 30% / 66% / 92% / 99%. K=4000: 18% / 51% / 88% / 98%. Radius shrinks ~8% per 4× clusters.

### 6.2 Deterministic code, undecided per true match
| Code | 0.95 | 0.9 | 0.8 | 0.7 |
|---|---|---|---|---|
| first design, 96 B (no Δ trick, global PCA) | 5.0 | 3.1 | 5.6 | 4.7 |
| 4 B | 9.1 | 18.2 | 28.6 | 9.7 |
| 16 B | 2.0 | 2.1 | 10.6 | 7.5 |
| 32 B | 1.2 | 0.83 | 4.3 | 4.9 |
| 96 B | 0.49 | 0.29 | 1.1 | 1.4 |
| 192 B | 0.20 | 0.12 | 0.41 | 0.49 |
| 384 B | 0.07 | 0.05 | 0.12 | 0.15 |

Share of the sample surviving a 4-byte prefix: 2.0% / 11.3% / 56.6% / 91%.
Mean guaranteed half-width over true matches at 96 B: 0.0042 / 0.0064 / 0.0153 / 0.0281.

### 6.3 Split certificate, half-width at t = 0.9 (100M pairs, max error 5.7 predicted σ)
| Code | Deterministic | c = 8 | c = 10 |
|---|---|---|---|
| 32 B | 0.0158 | 0.0146 | 0.0155 |
| 96 B | 0.0064 | 0.0043 | 0.0047 |
| 192 B | 0.0027 | 0.0013 | 0.0015 |
| 384 B | 0.0010 | 0.0004 | 0.0005 |
About 64% of the c = 10 bound at 96 B is still the deterministic part. Byte counts exclude ~2
stored scalars per group per item (up to ~16 B).

### 6.4 Tolerance d, unsettled per true match after a 192-byte deterministic code
exact: 0.20 / 0.12 / 0.41 / 0.49. d = 0.01: 0.003 / 0.03 / 0.23 / 0.37.
d = 0.02: 0.000 / 0.007 / 0.09 / 0.24.

### 6.5 Plain thresholding of the estimate from one code (no refinement)
Disagreements with the exact answer, both directions, as a share of the answer:
32 B: 3.5% / 1.9% / 4.5% / 5.1%. 96 B: 1.1% / 0.6% / 1.7% / 1.9%. 192 B: 0.5% / 0.25% / 0.7% / 0.8%.
At 0.7 and 32 B the disagreeing items sit a median 0.0025 from the threshold, 99% within 0.017,
worst 0.097. The tail comes from outlier items whose coordinates overflow the quantiser range
(largest error stays ~0.055 even at 384 B). An unbiased-rescaled estimate made this worse
(10.4% at 32 B).

### 6.6 Approximations to know about
- Code sizes were measured as independent allocations at each budget, not as prefixes of one
  nested stream. The baseline in section 1 treats them as nested levels
  (0, 4, 8, 16, 32, 48, 96, 192, 384, 576 bytes, then the full vector) and charges 4 bytes of
  scalars per item.
- Local bases cost memory not counted anywhere: 64 bases of 768² values for a 1M sample.
- No timings were measured, only counts.

## 7. Dead list (ideas tested or argued to fail, with the reason)

One reason covers most of them: a guarantee about a *set* of items is loose, because ~1,000
points spread over dozens of effective dimensions have no compact tight outer description; a
guarantee about *one* item is cheap. You are welcome to attack any of these.

- **HNSW / graph indexes.** Approximate; recall of a visible item depends on other items; breaks
  under sparse filters.
- **Multi-index hashing on binary codes** (pigeonhole over chunks). Even with an oracle radius:
  candidates are 6.5% of the corpus at 0.98, 14% at 0.95, 71% at 0.9, 99% at 0.8. **[measured]**
- **Wide sparse features from random projections** (top 32 of 65,536). Best random feature has
  cosine 0.15 with an item (0.93 for the nearest of 1,000 centroids). At 0.8 some true match
  shares zero features with the query in every query tested. **[measured]**
- **Sparse codes over a learned dictionary + inverted lists (WAND/MaxScore).** The stopping rule
  for unseen items is an ℓ1 sum of many small worst cases and the query is dense in the wide
  space. **[argued]**
- **Precomputed exact answers around anchor queries, corrected for the real query.** Correction
  band ≈ the query's distance to its anchor (~20°). **[argued]**
- **Finer or nested clusters, trees, micro-groups.** Radius ∝ K^−0.06. **[measured]**
- **Per-cluster shape descriptions** (ellipsoids, boxes, scatter matrices, moment tensors).
  Cost more than the items' own codes or are looser than the ball. **[argued]**
- **Two independent codes, intersected.** Both intervals centre on the truth; 5% gain for double
  the bytes. Combined into one estimate: error 0.38 → 0.30. A second code on the residual of the
  first: 0.23, same as one code at double precision. **[measured, 200k]**
- **Simultaneous diagonalisation of query-offset and residual covariances** (the theoretically
  optimal linear basis). Helps the first 4–8 bytes, hurts beyond 16. **[measured]**
- **Box ∩ ball bounds, one-sided sign bounds, Hölder variants.** No better than weighted
  Cauchy–Schwarz for spread errors. **[argued]**
- **Sketches of the errors (count-sketch, compressed sensing).** A dense noise floor of typical
  errors swamps the heavy hitters. **[argued]**
- **Discrepancy-minimising rounding.** Strong only for a fixed list of test queries; for a
  low-dimensional query subspace, spending the bits directly wins. **[argued]**
- **Dropping the low-energy tail.** Past local dimension 256 the residual holds 2–3% of the
  energy and is isotropic between unrelated items, but near-duplicates share it. **[measured]**
- **Lower confidence (smaller c).** c 10 → 6 tightens the 96-byte bound by ~15%. **[measured]**

## 8. Known weaknesses and open questions

- Queries that are not item-like (text against image embeddings) — the largest untested risk.
- Dimension: everything is measured at n = 768; the target range is 100 to 5,000.
- Outlier items that overflow the quantiser range.
- The floor is not proved; no instance-optimality statement.
- Whether any first pass can avoid touching every visible item.
- A guarantee uniform over a declared class of queries (removing the non-adaptivity caveat).
- Borderline membership under any tolerance depends on parameters learned from the whole corpus.

## 9. What to hand back

Write `REPORT.md` in your subdirectory, and return a summary of at most ~400 words. In the report:

- State each claim with a status: **proved** (give the proof), **measured** (give the numbers,
  the script, the sample size and the violation count), or **conjectured**.
- Report results on the owner's metric (section 1) wherever you can, against the baseline row.
- State how each result depends on the dimension n across 100 to 5,000, and test at a second
  dimension where you can.
- Say plainly what did not work and why. A solid impossibility argument is a result.
- Do not claim more than you checked.

## 10. Decisions taken by the owner

- **The seed is public.** It is a random seed stored in the clear in the bundle manifest, not a
  secret. The randomised guarantee requires queries and the corpus to be independent of it.
- **Crafting against the seed harms only the crafter.** Each item's decision depends only on its
  own code, so an item crafted against the seed affects only itself, by at most the deterministic
  bound; its writer could equally omit it or write a different vector. A query crafted against the
  seed can only make that query's own result wrong. No assumption about writers is needed.
- **A filter keeps its meaning.** Results are plain sets; no three-valued or "maybe" results.
- **The method must generalise across datasets and dimensions** (roughly 100 to 5,000), with no
  dataset-specific modelling.
- **Text queries are the expected common case.** Keep the randomised deep levels in the stream:
  they cost item queries ~1–3% and save text queries 8–18% (combined build, 1M).
