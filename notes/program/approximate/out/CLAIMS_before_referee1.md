# Approximate track: claims

21 November 2026. Every claim carries one tag of `notes/README.md`. Proofs are mine and unrefereed. Measured claims
name the dataset, the size and the script; the tables are in `out/tables.md`. How each was reached, and what failed
on the way, is in `NOTEBOOK.md`.

## Local symbols

Chapter 0's symbols are used as given. These are local to this file.

- **A tree.** Levels j = 0, …, D; the root is level 0 and the leaves are level D. K_j is the number of nodes at
  level j and M = K_D the number of leaves. F_j = K_j/K_(j−1) is the fan-out at level j. The leaves are rows
  (M = N) unless a claim says they are clusters (M = K).
- **Entropy position and variance share.** g_j = ln K_j / ln M, so 0 = g_0 < g_1 < … < g_D = 1. f_j is the share of
  the leaf values' variance explained by level j, with f_0 = 0 and f_D = 1. These are `hierarchy/theory.md`'s g_j and
  f_j; they are not chapter 0's density f or g_c = n_c·p_c, which this file does not use.
- **Profile.** A(t), t in [0, 1], is the piecewise-linear function through the points (g_j, f_j). a is its
  derivative, Â its concave hull and â the hull's derivative. This is Addario-Berry and Maillard's A.
- **Model T** (`hierarchy/theory.md` 2.3). The value of a leaf is U = ε_1 + … + ε_D. ε_j is shared by the leaves
  under one level-j node, the ε are independent, and ε_j ~ N(0, v·(f_j − f_(j−1))). The value of a level-j node is
  Y_j = ε_1 + … + ε_j. v is the leaves' variance.
- **Targets.** T is a level on the leaf values, z_T = T/√v, and the targets are the leaves with U ≥ T. L is the
  expected number of targets, z_r = Φ⁻¹(r), and z_REM = Φ̄⁻¹(L/M) is the level at which a leaf count of L is
  expected. x = z_T/z_REM.
- **For A2.** V is the set of clusters read and v_c the visible rows of cluster c. X(t) is the number of visible
  rows in unread clusters with score at least t, and Miss the number of true neighbours in unread clusters. G is the
  survival function of the standardised within-cluster scores z = (s − μ_c)/σ_c (the Ḡ of `theory.md`'s model M3).
  M(t) = Σ over unread c of v_c·G(ζ_c(t)), with ζ_c(t) = (t − μ_c)/σ_c, and M̂ is the same with an estimated tail Ĝ.
- **For A3.** i indexes the n coordinates of a group's basis. ν_i is the build's variance of the coding error in
  coordinate i at a level (the brief writes v_c), π_i the query profile, D_i⁴ = ν_i/π_i the level's weights,
  a_i = √(ν_i·π_i) = D_i²·π_i. w = ‖DΔ‖ is the query's factor of the certified half-width and u = ‖D⁻¹e‖ the row's.

---

## A1. The read as a random energy model

### A1.1 Model T is Derrida's generalised random energy model [proved, unrefereed; the theorem cited]

**Statement.** Model T with D levels, K_j nodes at level j and variance shares f_j is a generalised random energy
model with D levels, entropy shares g_j − g_(j−1) and variance shares f_j − f_(j−1). The continuous random energy
model on the binary tree of depth log₂ M with profile A is the model in which every level is refined into binary
levels of equal variance per level. An index maps to it as follows.

| index | model |
|---|---|
| one query q | one realisation of the energies; a different q is a different landscape on the same tree |
| a row's score s(x) | a leaf's value U |
| a group, a cluster, a sub-cluster or leaf run | a node at a branching level |
| the mean score of a node's rows (μ_c for a cluster; close to the centre score q·m_c) | the node's value Y_j |
| reading a centre score | one query of an internal node |
| a row at its key, at the 16 B level, at the 32 B level | nodes of fan-out 1 under the row's cluster: they add variance and no entropy |
| reading a stored vector | one query of a leaf |
| the k nearest neighbours | the leaves above τ_k |
| g_j, f_j | ln(cells)/ln N, and the share of the variance of s(x) over the rows that the cells' means explain |

**Proof.** The first two sentences restate the definitions (Addario-Berry and Maillard, section 1: a centred Gaussian
process on the tree with E[X_v X_w] = N·A(|v ∧ w|/N), equivalently independent increments of variance
N(A((i+1)/N) − A(i/N)) at generation i). The table is the mapping, not a claim about data. ∎

**What the mapping assumes of the data, each of which A1.7 tests:** Gaussian increments; an increment independent
of its parent's value; equal within-node variance at a level; a tree that is regular.

**Cited.** Addario-Berry and Maillard, arXiv:1810.05129, Theorem 1.1 [cited: I read the statement, the whole proof
(sections 2 and 3), section 4 and the open questions]. With x_* = √(2 log 2)·∫₀¹ √a, a linear-time algorithm finds a
leaf with X_v ≥ xN for x < x_*, and for x > x_* every algorithm that queries node values needs a number of queries
that dominates a Geometric(e^(−γN)) variable, γ = γ(A, x) > 0. The ground state is x_s = √(2 log 2)·∫₀¹ √â
(Bovier and Kurkova, quoted there as (1.1)) [cited, unchecked: through that paper]. x_* = x_s if and only if A is
concave.

### A1.2 The leaf floor [proved, unrefereed]

**Statement.** In Model T, give an algorithm the value of every internal node for nothing. Let it query leaves one
at a time, adaptively and with its own randomness, and stop when it chooses; a target is found when its leaf is
queried. If the expected number of targets found is at least r times the expected number of targets, then the
expected number of leaves queried is at least

    R(r) = M · P(Y_(D−1) > a_r),     a_r defined by  P(Y_(D−1) > a_r, U ≥ T) = r · P(U ≥ T),

where (Y_(D−1), U) are jointly normal with Var Y_(D−1) = v·f, Var U = v, covariance v·f, f = f_(D−1). The rule
"query the leaves whose parent's value exceeds a_r" attains it.

**Proof.** Let 𝒢 be the σ-field of the internal values. Given 𝒢 the leaf values are independent, and leaf i has
U_i ~ N(Y_par(i), v(1 − f)), because U_i = Y_par(i) + ε_(D,i) and the ε_(D,i) are independent of each other and of
𝒢. Write p_i = Φ̄((T − Y_par(i))/√(v(1 − f))), which is 𝒢-measurable.

Let Q be the set of leaves queried. Fix a leaf i. Up to and including the step at which i is first queried, the
algorithm has seen 𝒢, values of other leaves and its own randomness, so the event {i ∈ Q} is a function of
(𝒢, (U_j)_(j≠i), the randomness); the stopping rule does not change this, since stopping before that step is decided
from the same information. Given 𝒢, U_i is independent of that triple. Hence
P(U_i ≥ T, i ∈ Q) = E[1{i ∈ Q}·p_i], and

    E[targets found] = E[ Σ_(i ∈ Q) p_i ] .

For every λ ≥ 0 and every set Q, Σ_(i ∈ Q) p_i ≤ λ|Q| + Σ_i (p_i − λ)₊. Taking expectations,
E[targets found] ≤ λ·E|Q| + M·E(p − λ)₊, with p = Φ̄((T − Y)/√(v(1 − f))) and Y ~ N(0, v·f). Choose λ_r with
M·E[p; p > λ_r] = r·M·P(U ≥ T); it exists because p has a continuous law. Then

    E|Q| ≥ ( r·M·P(U ≥ T) − M·E(p − λ_r)₊ ) / λ_r = M·P(p > λ_r) = M·P(Y > a_r),

since E[p; p > λ] − E[(p − λ); p > λ] = λ·P(p > λ), and {p > λ_r} = {Y > a_r} for the a_r of the statement
(E[p; Y > a] = P(Y > a, U ≥ T)). The rule Q = {i : p_i > λ_r} has E|Q| = M·P(p > λ_r) and finds exactly the required
expected number. ∎

**Remarks.** (i) The targets here are the leaves above a fixed level. For the k best leaves the level τ_k is
random; the statement then holds with the gap of `theory.md` Theorem 10 (τ against the k-th best outside the leaf's
node) [sketch: gap named, the concentration of τ_k is not carried through]. (ii) The floor counts leaf queries only,
whatever the internal queries cost. So it is a floor on the rows read by any hierarchy whose finest centres explain
a share f, under any order, stop or beam, in the model. (iii) Applied one level lower it is a floor on stored
vectors read given the 16 B or 32 B level, with f the share those levels explain.

### A1.3 The floor's closed form and its exponent [computed]

**Statement.** R(r) = M·Φ̄(z_T·√f − z_r·√(1 − f))·c, with c between 0.60 and 0.90 for M = 10⁶ and 10⁹, L = 10
and 100, f = 0.3 to 0.7 and r = 0.9 and 0.99 (the closed form with c = 1 is `hierarchy/theory.md` (2.1) at the leaf
level; it puts every target at T, and the targets lie above T). With z_T = z_REM,
R(r) = M^(1−f)·L^f·exp(z_r·z_T·√(f(1 − f)) + O(log z_T)), so the exponent of R in M tends to 1 − f. At M = 10⁶ the
local exponent d ln R / d ln M is 0.08 to 0.22 above 1 − f: 0.60 to 0.72 for f = 0.5, 0.40 to 0.51 for f = 0.7.

**Source.** `scripts/grem_check.py` part A (numerical integration of the bivariate normal), `out/grem_check.txt`.

### A1.4 The work of a beam, and where the levels should sit [model]

**Statement.** In Model T with targets at x = 1 and the beam of `hierarchy/theory.md` Proposition 1 (keep a level-j
node when Y_j ≥ √v·(z_T·f_j − z_ε·√(f_j(1 − f_j)))), the expected number of nodes scored is

    W = Σ_(j=1..D) B_(j−1)·F_j ,    B_0 = 1,   B_j = K_j·Φ̄(z_T·√f_j − z_ε·√(1 − f_j)) ,

and W = M^(η + o(1)) as M grows with the g_j and f_j fixed, where

    η = max over j = 1, …, D of ( g_j − f_(j−1) ) .                                             (A1.4)

For a given profile A the level positions that minimise η satisfy g_1 = η and g_j = η + A(g_(j−1)), ending at
g_D = 1. As D grows the minimum tends to sup_t (t − A(t)), which is zero if and only if A(t) ≥ t for every t.

**Derivation.** Proposition 1 of `hierarchy/theory.md` gives B_j exactly as the expected number of level-j nodes
above the level's threshold, and each kept node has its F_j children scored. B_j = K_j·exp(−f_j·z_T²/2 + O(z_T)),
and z_T² = 2 ln(M/L) + O(log log M), so B_j = M^(g_j − f_j + o(1)) and B_(j−1)·F_j = M^(g_j − f_(j−1) + o(1)). The
positions follow by making every term equal: lowering any g_j below that raises the term of level j + 1.

**Why [model] and not [proved].** The statement is an expectation over trees. Where f_j > g_j the expectation is
carried by rare trees: the level T then holds no leaf with high probability (A1.5), and the formula describes
targets that do not exist. So (A1.4) describes what a typical tree costs only when A(t) ≤ t at every level. It is an
upper bound on the cost of finding the targets, not a lower bound: the only lower bound here is A1.2, which is its
last term, 1 − f_(D−1).

**Check on synthetic trees.** `scripts/grem_check.py` part C: for A(t) = t, η = 1/D (0.5, 0.33, 0.25 for D = 2, 3,
4); for A(t) = t², η = 0.62, 0.48, 0.42 and sup(t − A) = 0.25; for A(t) = √t, η = 0.38, 0.20, 0.11.

**Consequences for an index, in the model.**

- A level of fan-out 1 has g_j = g_(j−1), so it adds no term: the row's key and codes change the cost of a leaf
  probe, not the exponent. Only branching levels move η.
- A new branching level at entropy position g helps if and only if it splits the term that attains the maximum.
- With the clusters as the only branching level, η = max(g_1, 1 − f_1): the centres, or the rows under them.

### A1.5 Which dichotomy Model T's is [proved, unrefereed; one part computed, one part a published conjecture]

The library note (`library/codes-and-search.md` 2.2) says that Model T's regime "f_j ≥ g_j, beam subpolynomial" is
A(t) ≥ t, which every concave A satisfies, and that "f_j < g_j, beam polynomial" is a non-concave A. Checked:

1. **True in one direction.** A concave A with A(0) = 0 and A(1) = 1 lies on or above its chord, so A(t) ≥ t. Hence
   f_j < g_j at some level implies A is not concave. [proved: the chord property of a concave function]
2. **False in the other.** A(t) ≥ t does not imply concavity. Shares (0.60, 0.07, 0.33) on three levels of equal
   fan-out give A(1/3) = 0.60 and A(2/3) = 0.67, both at or above t, with slopes 1.8, 0.21 and 0.99, which is not
   concave. [proved by the example]
3. **So the two dichotomies differ, and the published one is the sharper.** Model T's splits profiles by the
   diagonal (the weak- and strong-correlation regimes of Bovier and Kurkova); Addario-Berry and Maillard's splits
   them by concavity. Where A(t) ≥ t and A is not concave, Model T's beam formula gives a subpolynomial count while
   the theorem gives exponential cost in the depth above x_*. They do not conflict, because they speak of different
   levels: the beam formula is for the level z_REM, and when A(t) > t somewhere the ground state x_s lies below
   that level, so with high probability no leaf is there. [proved given the cited ground state]
4. **The realised top level follows the hull.** For shares (0.6, 0.25, 0.15), fan-out 28 and the top 64 of 21,952
   leaves, the realised level over the level z_REM is 0.967 over 200 trees; the hull formula
   Σ_j √((f_j − f_(j−1))(g_j − g_(j−1))) gives 0.960. [computed: `scripts/grem_check.py` part B]
5. **In the weak regime the beam exponent equals the conjectured pool exponent.** The path that attains the ground
   state has derivative a·√(2 log 2/â) (Addario-Berry and Maillard (1.3)), and along it the logarithm of the number
   of nodes at position t, in units of ln M, is t − ∫₀ᵗ a/â. When A(t) ≤ t for all t, â = 1 and this is t − A(t),
   whose supremum is the deep-tree limit of (A1.4). Their open question 2, formula (5.1), conjectures that this pool
   is the cost exponent above x_*. [the identity is computed from (1.3); that it is the cost for every algorithm is
   their **[conjecture]**, not a theorem]

### A1.6 What of the hardness transfers [model; the theorem cited]

**Statement.** If the scores along a query were a continuous random energy model with a profile that is not concave,
every algorithm that reads node values, adaptively and with randomness, would need N^(γ/ln 2) reads to return even
one row above the level x_*, for some γ > 0; the k best rows lie at x_s > x_*. So no hierarchy of centres, with any
order, beam or stop, would read polylog(N) nodes. If the profile were concave, a renormalised greedy descent would
reach within any fixed fraction of the best row in O(log N) reads.

**The query model.** One unit per node value: a centre score is an internal node, a stored vector a leaf. The
theorem counts nodes, not bytes, and gives no value of γ.

**What does not transfer, in order of weight.**

1. **Flat fan-outs.** The theorem is about a binary tree, in which a level of 2,000 children has eleven binary
   levels inside it to descend. A cluster has no such inner levels, so inside a level there is nothing to do but
   read the children. For the index's tree (two to four branching levels) the bounds that apply are A1.2 from below
   and (A1.4) from above; they agree in the exponent when the last term of (A1.4) is the largest. The theorem is
   the statement for D → ∞.
2. **All of the k best, not one leaf above a level.** The read is an enumeration to a recall. Its floor, A1.2, is
   present whatever the shape of A: with a concave profile and D levels the rows read are still M^(1 − f_(D−1)).
   Concavity decides whether more levels can drive that to zero, not whether the floor exists at a given D.
3. **The data are not the model** (A1.7): the Gaussian law over-states the read by 1.1 to 2.5 times at the cluster
   level, so the model's lower bound is not a lower bound for the data.
4. **Exact against near.** Even for A(t) = t, where x_* = x_s, the theorem reaches a fraction 1 − ε of the best
   level, not the best row. Pemantle (Ann. Appl. Probab. 19, 2009) shows that reaching 1 − ε on a homogeneous tree
   costs N·exp(Θ(ε^(−1/2))) for Bernoulli increments of mean below ½ [cited, unchecked: the abstract through a search
   summary and the description in Ho and Maillard, arXiv:2107.11465, section 1]. With ε of order 1/log N that is
   exp(Θ(√log N)), the form of `hierarchy/theory.md` (2.2).

### A1.7 The mapping on three fits [measured]

Data: the suite's fits of yahoo-minilm-384 (N 677,305, K 339), laion-clip-512 with text queries (N 1,000,448,
K 500) and glove-100-angular (N 1,183,514, K 592), 64 groups each, the prep's 100 queries, k = 10; sub-clusters by
nested 4-way k-means inside each cluster (a4, a16, a64). Scores are products of the f16 stored vectors with the
queries. Script `scripts/replay_np.py`; tables T1 to T4 and T10 of `out/tables.md`.

1. **The profile lies on the diagonal down to the clusters and below it under them, and it is not concave.**
   (g, f) at groups, clusters, a4, a16, a64: yahoo (0.31, 0.28), (0.43, 0.42), (0.54, 0.48), (0.64, 0.53),
   (0.74, 0.58); LAION (0.30, 0.28), (0.45, 0.45), (0.55, 0.51), (0.65, 0.55), (0.75, 0.59); glove (0.30, 0.20),
   (0.46, 0.36), (0.56, 0.41), (0.66, 0.45), (0.75, 0.49). The slopes of A between successive points, ending at the
   rows, are 0.90, 1.16, 0.54, 0.48, 0.47, 1.65 (yahoo), 0.92, 1.18, 0.52, 0.44, 0.44, 1.62 (LAION) and 0.67, 0.99,
   0.49, 0.39, 0.43, 2.08 (glove): the last segment, from cells of about 31 rows to the rows, is the steepest.
   sup(g − f) over the measured points is 0.17, 0.16 and 0.27. Per query f varies: the 10th to 90th percentiles at
   the cluster level are 0.34 to 0.50, 0.38 to 0.55 and 0.29 to 0.45.
2. **The scores' upper tail is heavier than Gaussian, most on yahoo.** The 10th best score sits at 6.80, 5.34 and
   4.51 total standard deviations (yahoo, LAION, glove) against z_REM = 4.18, 4.26 and 4.30. It rises by 0.52, 0.35
   and 0.28 per e-fold of N on nested random subsamples, against 0.25 for a Gaussian.
3. **Regression to the mean holds in the mean.** The standardised centre score of a true neighbour's cluster
   averages 2.89, 2.11 and 1.68; the model's f·z_T is 2.86, 2.41 and 1.61. On LAION's text queries it falls 0.30
   short.
4. **The residual is wider than the model's and the increments are not independent of the parent.** The residual's
   standard deviation is 0.72, 0.72 and 0.57 against √(f(1 − f)) = 0.49, 0.49, 0.48. The centre scores have excess
   kurtosis 4.3, 0.9 and 1.6. Within a query the correlation of μ_c with σ_c is 0.66, 0.09 and −0.20.
5. **The Gaussian law is right on glove, and wrong by an order of magnitude on yahoo at the measured level.** Share
   of rows in clusters above the law's threshold at nominal coverage 0.9: glove 6.4% measured against 7.2% by the
   law, with coverage 0.90; yahoo 1.2% against 0.15%, with coverage 0.82.
6. **In rank space the law holds to a factor of about two.** Take the level at its Gaussian value z_REM instead of
   the measured one, and compare with the share of rows in cells keyed at or above a true neighbour's cell (0.9
   quantile over neighbours). At the cluster level: yahoo 2.7% by centre score and 2.0% by μ + 3σ against 4.5% by
   the law; LAION 6.5% and 2.9% against 3.2%; glove 8.7% and 5.7% against 6.8%. So the data are easier than the
   model on yahoo, and on LAION's text queries the centre score alone is worse than the model while μ + 3σ matches
   it.
7. **Exponents.** In cells at fixed N (K to 64K), the share falls as cells^(−γ) with γ = 0.38, 0.36 and 0.23 (centre
   score) and 0.56, 0.48 and 0.43 (μ + 3σ) on yahoo, LAION and glove; the law with the measured f gives 0.34, 0.33
   and 0.28. At a fixed cell size over a 16-fold range of N the rows grow as N^η with η = 0.47, 0.67 and 0.70
   (centre score), 0.36, 0.51 and 0.52 (μ + 3σ); the law gives 0.38, 0.37 and 0.50. Under the build rule K ∝ √N the model's local exponent at N = 10⁶, with f = A(½) held
   fixed, is 0.64, 0.62 and 0.71 at recall 0.9 and 0.68, 0.65 and 0.74 at 0.95 (`scripts/local_exponent.py`);
   `hierarchy/layered.md` section 2 measures γ/2 + η = 0.55, 0.61 and 0.65 at 0.9 and 0.60, 0.66 and 0.73 at 0.95.
   The model is within 0.10, on the high side at recall 0.9. That the profile stays fixed as N grows is assumed,
   not measured.
8. **In the model, the measured profiles are in the hard regime, by a small margin.** With the level at z_REM,
   Σ_j √(Δf_j·Δg_j) over the measured points is 0.975 (yahoo, LAION) and 0.954 (glove) of the top level, and the
   hull gives 1. This sum is an upper bound on x_*/x_s, because a finer profile can only lower it. So in the model
   a greedy descent reaches rows scoring within about 2.5 to 5% of the k-th best level, not the k best.

**Verdict.** The structure maps: levels, the floor of A1.2, regression to the mean, a profile that can be measured
with no truth. The numbers map on glove, and only in rank space and to a factor of two on yahoo and LAION. What
breaks it, by size: the heavy upper tail of the scores; a residual 1.2 to 1.5 times wider than the model's; μ_c and
σ_c correlated within a query on yahoo; a flat tree of two branching levels. The claim of `hierarchy/layered.md`
that N^η is "the floor for any hierarchy keyed by centres and spreads" is supported as a statement about these
data's profile (1 − f at the finest cells is 0.42 to 0.51), not as a theorem: A1.2 is a floor in the model, and the
data on yahoo beat the model.

### A1.8 A hypothesis of mine about `model_out.txt`, refuted [refuted]

**Claim.** The synthetic check of `hierarchy/theory.md` with shares (0.6, 0.25, 0.15) reported 13 nodes measured
against 7 predicted at the first level because the formula uses the level z_REM while the realised level is lower.

**Refuted.** Over 200 trees (`scripts/grem_check.py` part B) the mean of the per-tree 99th percentile rank is 6.6
against the formula's 7.1; the realised level is 2.67 against 2.76. The 13 was the 99th percentile of the ranks
pooled over 12 trees, which is set by the worst trees (10 when pooled over 200). The formula is right as an
expectation at that size.

### A1.9 Which levels are worth having [model, with the measured profiles]

By (A1.4) with the profiles of A1.7 (table T10), the terms g_j − f_(j−1) are:

| design | yahoo | LAION | glove |
|---|---|---|---|
| clusters, rows | 0.43, **0.58** | 0.45, **0.55** | 0.46, **0.64** |
| groups, clusters, rows | 0.31, 0.16, **0.58** | 0.30, 0.17, **0.55** | 0.30, 0.26, **0.64** |
| clusters, a16, rows | 0.43, 0.22, **0.47** | **0.45**, 0.20, **0.45** | 0.46, 0.30, **0.55** |
| clusters, a4, a16, a64, rows | **0.43**, 0.11, 0.16, 0.22, 0.42 | **0.45**, 0.10, 0.14, 0.20, 0.41 | 0.46, 0.20, 0.25, 0.31, **0.51** |

- The rows under the clusters are the binding term at 1M on all three. A level above the clusters (the groups)
  leaves it unchanged. This is `hierarchy/theory.md`'s conclusion 3 and `layered.md`'s rule 2, from the profile
  alone.
- Leaves of about 125 rows bring the row term to the centres' term on yahoo and LAION. Past that the scan of all
  K centres binds, and only a level above the clusters lowers it: with the groups above and a64 below, the terms
  are 0.31, 0.16, then as in the last row.
- No design reaches below sup(g − f), 0.16 to 0.27 at the cells measured: in the model no hierarchy on these
  profiles is polylogarithmic.

These are exponents of node counts. At 1M the costs per node (n bytes a centre, 18.5 a first-level row) differ by 5
to 30 times, which is what decides a design at a given N.

---

## A2. A stop on expected misses

### A2.1 What the stop guarantees when the tail is known [proved, unrefereed]

**Statement.** Assume model M1 of `theory.md` (given the clusters' statistics, the scores of different clusters are
independent and a cluster's visible scores are independent with P(s ≥ t) = G(ζ_c(t))). Read clusters in any order
that depends only on the statistics and on scores already read. After each read let t̂ be the running k-th best and
M = M(t̂). Then:

1. Miss ≤ X(t̂), always.
2. E[X(t̂) | everything read] = M.
3. For the stop "the first read after which M < (1 − r)·k", E[recall | everything read up to the stop] > r, for
   every query.
4. If the stop uses M̂ with a tail Ĝ, and G ≤ κ·Ĝ at every ζ_c of an unread cluster, then
   E[recall | everything read up to the stop] > 1 − κ·(1 − r).

**Proof.** (1) A true neighbour scores at least τ_k, and τ_k ≥ t̂ because the k rows held are k rows of the visible
set. So every missed neighbour is a visible row in an unread cluster with score at least t̂. (2) Let ℱ be the
σ-field of the statistics and the scores read. Which clusters are unread and the value of t̂ are ℱ-measurable.
Each choice of the next cluster is made before any of its scores is seen, so given ℱ the unread clusters' scores
have their unconditional law, and E[X(t̂) | ℱ] = Σ over unread c of v_c·G(ζ_c(t̂)). (3) Let σ be the stopping step.
For each m, {σ = m} is ℱ_m-measurable, so E[Miss_m; σ = m] ≤ E[X(t̂_m); σ = m] = E[M_m; σ = m] < (1 − r)·k·P(σ = m).
(4) The same with E[X(t̂) | ℱ] ≤ κ·M̂; the estimate Ĝ may depend on the scores read, since it is then
ℱ-measurable. ∎

**Remarks.** The guarantee is conditional on what the query has seen, which a threshold on the best remaining
cluster does not give. It is a guarantee in the model. In (4), κ is a random variable when Ĝ is estimated, and a
stop that waits for M̂ to dip selects the reads at which Ĝ is low, so κ at the stop is larger than κ on average
(measured: A2.5, item 4).

### A2.2 No shape, no guarantee [proved, unrefereed]

**Statement.** Let a rule decide to stop from n standardised scores drawn independently from the shape G, and from
anything independent of the unread rows. Let W rows remain unread, in clusters whose ζ_c all lie in [ζ_0, ζ_1]. Let
F_0 be any shape with no mass above ζ_0 under which the rule stops with probability s. Then there is a shape G_1
under which the n scores have a law within total variation ¼ of their law under F_0, the rule stops with
probability at least s − ¼, and

    E_(G_1)[Miss; the rule stops] ≥ (s − ¼)·½·min(k, W/(4n)) .

So a rule that stops surely under F_0 and keeps E[Miss] ≤ (1 − r)·k under every shape needs
n ≥ (3/32)·W/((1 − r)·k) whenever W/(4n) ≤ k: for one expected miss, about a tenth as many pooled rows as rows left
unread.

**Proof.** Let ε = 1/(4n) and G_1 = (1 − ε)·F_0 + ε·δ_(ζ*), with ζ* above ζ_1 and large enough that
μ_c + ζ*·σ_c exceeds every score held, for every unread c. A sample of n from G_1 equals a sample from F_0 unless
some draw falls on the atom, which has probability at most nε = ¼; so the two sample laws are within ¼ in total
variation and the rule stops under G_1 with probability at least s − ¼. Under G_1 each unread row exceeds t̂ with
probability at least ε, independently of the sample and of the other rows (M1), so X is a sum of W independent
indicators with mean m ≥ W·ε, and every such row outranks every row held, so Miss = min(k, X). For such a sum
E(X − m)₊ = ½·E|X − m| ≤ ½·√m, hence E min(k, X) ≥ ½·min(k, m): for 1 ≤ m ≤ k, E min(k, X) ≥ m − E(X − k)₊ ≥
m − ½√m ≥ m/2; for m < 1, E min(k, X) ≥ P(X ≥ 1) ≥ 1 − e^(−m) ≥ m/2; for m > k by monotonicity in m. X is
independent of the event that the rule stops. ∎

**Remark.** F_0 and G_1 can both be given mean 0 and variance 1 when ε·ζ*² < 1, that is ζ* < 2√n, by moving F_0's
mean to −ε·ζ*/(1 − ε) and choosing its variance to complete the second moment [sketch: gap named, the admissible
range of ζ* against the scores held is not worked out]. This is the pooled-sample form of `theory.md` Proposition
8: the sample resolves the tail to about 1/n and the decision needs it at about (1 − r)·k/W.

**What this leaves possible.** A guarantee under a named tail family (A2.1 with a κ for the family), or a guarantee
on average over exchangeable queries from a calibration set with truth, as conformal risk control gives. Nothing
between: no rule that looks only at the rows it has read is valid for every tail.

### A2.3 The estimator's error at the time of decision [computed, on measured counts]

**Statement.** For a tail fitted as a location and scale of a fixed shape with local decay rate λ(ζ) = −d ln G/dζ,
pinned at a level u of the pooled scores, a relative error e in the scale gives an error of about
λ(ζ)·(ζ − u)·e in ln M̂. With the scale taken from the two upper quantiles at exceedance shares 5% and 0.5% of n
pooled rows, e ≈ 5.0/√n for a normal shape (the standard deviations of the two sample quantiles are 2.12/√n and
4.88/√n, their covariance is 3.18/n, and their difference is divided by 0.931). At n = 4,000 and ζ = 4, with
λ = 4.25 and u = 1.645, that is an error of 0.80 in ln M̂, a factor 2.2; at n = 1,300 a factor 4.0; at n = 20,000 a
factor 1.4.

**Measured counts at the decision** (table T9; the first read at which a stop on expected misses predicts under one
miss; medians over queries, normal tail): pooled rows 4,300 (yahoo) and 9,100 (LAION) unfiltered, 1,400 and 2,700
under random-25, 2,300 and 4,400 under region-25, 150 and 370 under random-1. Pooled rows with z > 3: 14 and 16
unfiltered, 4 to 8 under the 25% sets, 0 to 2 under the 1% sets. The nearest unread cluster is at ζ = 3.7 to 3.8
unfiltered. So the decision reads the tail one to three standard deviations beyond where the query holds more than
a handful of rows.

### A2.4 The shape of the standardised within-cluster scores [measured]

Pooled over 100 queries and every row of every cluster, z = (s − μ_c)/σ_c with exact μ_c and σ_c (table T5):

| x | normal | yahoo | LAION, text queries | LAION, corpus rows as queries | glove |
|---|---|---|---|---|---|
| 3 | 1.3e-3 | 4.5e-3 | 2.5e-3 | 1.1e-3 | 9.2e-4 |
| 4 | 3.2e-5 | 6.2e-4 | 2.2e-4 | 6.6e-5 | 2.5e-5 |
| 5 | 2.9e-7 | 8.7e-5 | 2.0e-5 | 5.6e-6 | 1.4e-6 |
| 6 | 9.9e-10 | 1.2e-5 | 2.1e-6 | 7.4e-7 | 2.3e-7 |

- The tail is close to exponential between 3 and 6: −d ln P/dx is 1.97 (yahoo), 2.36 (LAION) and 2.77 (glove),
  against 4.7 for the normal over the same range. At ζ = 4 the normal understates the rate by 20 times on yahoo and
  7 times on LAION; on glove it is right to within 25% up to 4.5.
- A shape taken at build from corpus rows used as queries, which needs no truth, equals the test queries' on yahoo
  and glove to within 12% up to x = 5. On LAION it is 3.3 times too light at x = 4, because text queries are out of
  distribution.

### A2.5 The premise fails in three ways [measured]

Same data; tables T6, T8, T9.

1. **One shape for all queries over-predicts at the true k-th best.** Summed over 100 queries, Σ_c n_c·G(ζ_c(τ_k))
   with the pooled shape of the test queries is 2,155 (yahoo), 2,463 (LAION) and 2,430 (glove) against the 1,000
   neighbours there are. The sum rule Σ n_c·p_c = k of `theory.md` Proposition 5 fails by 2.2 to 2.5 times, because
   the queries' tails differ and a query with a heavy tail has its τ_k far out. The normal tail gives 465, 1,247 and
   3,268.
2. **The clusters read first over-predict the rest.** The empirical tail of the first ten clusters in the order,
   applied to the clusters after them, predicts 149, 218 and 658 neighbours where there are 59, 127 and 215: 2.5,
   1.7 and 3.1 times too many. The first clusters hold the query's neighbourhood, so their upper tail towards q is
   heavier than a far cluster's. This is the bias from the clusters read being the best ones, and it is on the safe
   side.
3. **A per-query fit is right on average and wrong per query.** At reads where a generalised Pareto tail predicts
   between 0.3 and 3 rows above t̂ (mean 0.92 and 0.94), the 40 nearest unread clusters hold 1.06 and 1.29 on
   average unfiltered (yahoo, LAION). Yet stopping at a prediction of one delivers recall 0.85 and 0.81, and reads
   1.5 to 3.0 times the built stop's rows at equal mean recall.
4. **The stop selects its own low estimates.** That is item 3's mechanism: by A2.3 the prediction is off by a factor
   of 2 to 4 per query, and the rule stops at the first read where it is low.

### A2.6 The stops replayed [measured]

Whole clusters in the order μ_c + 3σ_c with exact σ_c; yahoo, LAION text and glove; none, random-25, region-25,
region-1, random-1; k = 10; 100 queries, so differences under about 10% are noise. Tables T7 and T8;
`scripts/replay_np.py`. "The built stop" here is its idealised form: every unread cluster priced, δ_q and ρ_q
pooled over every row read.

1. **Rows at equal mean recall, as a multiple of the built stop's**, over recall 0.9 to 0.99 and the five sets:

   | stop | yahoo | LAION | glove |
   |---|---|---|---|
   | expected misses, normal tail at (δ_q, ρ_q) | 0.98 to 1.09 | 0.87 to 1.13 | 0.97 to 1.77 |
   | expected misses, generalised Pareto fitted per query | 1.20 to 2.10 | 1.18 to 2.97 | 0.77 to 1.23 |
   | expected misses, exponential tail fitted per query | 0.95 to 2.12 | 1.08 to 3.01 | 0.88 to 2.24 |
   | expected misses, the build's shape table at (δ_q, ρ_q) | 1.03 to 1.50 | 0.93 to 1.31 | 0.97 to 2.23 |
   | the built stop with (δ_q, ρ_q) from two upper quantiles | 0.80 to 1.30 | 0.80 to 1.35 | 0.64 to 1.05 |
   | the same number of clusters for every query | 0.95 to 1.65 | 0.77 to 1.44 | 0.68 to 1.29 |

   No stop on expected misses is better than the built stop on yahoo or LAION beyond noise.
2. **What a setting delivers**, mean recall over the five sets:

   | setting | yahoo | LAION | glove |
   |---|---|---|---|
   | ψ = 4.5 | 0.92 to 0.99 | 0.94 to 1.00 | 0.98 to 1.00 |
   | one expected miss (nominal 0.9), normal tail | 0.65 to 0.81 | 0.74 to 0.85 | 0.92 to 0.95 |
   | the same, generalised Pareto per query | 0.71 to 0.85 | 0.76 to 0.86 | 0.76 to 0.87 |
   | the same, exponential per query | 0.75 to 0.97 | 0.87 to 0.96 | 0.95 to 0.99 |
   | the same, shape of the test queries | 0.90 to 0.97 | 0.93 to 0.97 | 0.89 to 0.95 |
   | the same, the build's shape table | 0.90 to 0.98 | 0.80 to 0.89 | 0.88 to 0.95 |

   No tail makes the setting a recall. With the test queries' own shape the nominal recall is a floor on yahoo and
   LAION; it over-delivers, at 1.1 to 4.3 times the rows the built stop reads for 0.9 (3.3 and 4.3 unfiltered).
3. **Under the region sets** every tail fitted to the pooled rows predicts too few: at a prediction of one, the
   nearest unread clusters hold 3.3 (yahoo) and 1.7 (LAION) rows above t̂ with the generalised Pareto tail under
   region-25, and 4.7 and 2.2 under region-1.
4. **The built stop is not always better than a fixed count**: the fixed count reads 0.77 and 0.81 of its rows on
   LAION unfiltered at recall 0.98 and 0.99, and 0.68 to 0.81 on glove unfiltered. `theory.md` Theorem 3 makes a
   global threshold optimal for a calibrated p_c; this is evidence that the pooled (δ_q, ρ_q) is not that.

**Limits.** An idealised reader (no window, no stream, exact spreads, whole clusters), three datasets, 100 queries,
the prep's five sets. The first yahoo and glove runs counted ties at the k-th best as true; no recall exceeded 1 on
them, and LAION, where it did, was rerun with exactly k.

### A2.7 How the stop relates to the published ones [cited]

What I read of each is in `NOTEBOOK.md` entry 2; the library's reading is `library/codes-and-search.md` 2.3.

| rule | what it predicts | from what | calibration set with truth | under a visible set |
|---|---|---|---|---|
| Quake's adaptive partition scanning (arXiv:2506.03437, section 5; read) | the share of the k neighbours in each partition, as the share of the ball B(q, ρ) beyond the bisector with the nearest centroid, normalised to sum to 1 over a candidate set; stops when the scanned shares reach the target | geometry and the running k-th distance | none | "future work": scale the shares by the rows that pass |
| LAET (SIGMOD 2020) [cited, unchecked: as DARTH and Quake describe it] | the search length a query needs | a learned model on query and early-search features, with a multiplier tuned per target | yes | not treated |
| DARTH (arXiv:2505.19001; a model's summary of the HTML read) | the recall so far | gradient-boosted trees on search-state features, among them statistics of the neighbours' distances | 10,000 training queries; about 15% of queries miss the target | not treated |
| Auncel (NSDI 2023) [cited, unchecked: search summary] | an error bound from local geometry | geometry, one calibrated parameter | a parameter search | not treated |
| PAC-NN (ICDE 2000) [cited, unchecked: the library's summary] | whether the current neighbour is within a factor of the true one | the query's distance distribution | none | not treated |
| ConANN (PVLDB 19) [cited, unchecked: search summary] | how many clusters to probe so that the expected false-negative rate is at most α | conformal risk control on a per-query score | yes, exchangeable with the queries | not treated |
| CaST (arXiv:2611.16148; abstract read through a model's summary) | when a graph search's answer is exact with probability 1 − ε | a monotone rule on declared observations, calibrated by fixed-sequence binomial tests, with a dual bound on the best rule of the class | yes | a per-category cap only |
| the stop of A2 | the expected number of neighbours in the unread clusters, Σ v_c·Ĝ(ζ_c) | each cluster's centre score, spread and visible count, and a tail | none | by construction: v_c and the visible rows read |

- **Same form as Quake's**: both stop when the predicted mass left falls under 1 − r. The difference is the model of
  the mass (a tail of scores scaled by each cluster's own spread and visible count, not a volume share) and that
  the prediction is a count, not a share forced to sum to one.
- **Against the calibrated rules, CaST above all.** CaST and ConANN take a statistic and a set of queries with
  truth and return a threshold with a guarantee on average over exchangeable queries. The stop of A2 has no such
  guarantee and, measured, is not calibrated (A2.6). What it has that they lack is that it needs no truth and is
  computed from the visible rows, so it follows a session's visible set, where a calibration made on another visible
  set is void: the truth itself changes with the set. The two compose: M̂, or the built stop's statistic, is a
  monotone score that CaST or ConANN can threshold wherever a calibration set is valid, and CaST's bound on the best
  rule of a class is relative to the observations declared, which these statistics extend.
- **Against the claim in `theory.md` 2.4** (corrected by the library): the published rules stop on a per-query
  target. The built stop is the global threshold of Theorem 3. A2.6 item 1 is the measured price of a per-query
  target with a noisy prediction.

### A2.8 What is worth testing on the harness [conjecture]

On the real reader the ranking of A2.6 holds: no stop on expected misses saves rows; the built stop with (δ_q, ρ_q)
from two upper quantiles helps on low-dimensional data and is neutral elsewhere. `REPLAY-SPEC.md` is the test.

---

## A3. A per-query spread at the levels

### A3.1 The rule [proved, unrefereed, under a stated model of the coding error]

**Model.** A row's coding error at a level is e_i = κ·√ν_i·ξ_i, with the ξ_i uncorrelated, of mean 0 and variance
1, and κ the row's own scale.

**Statement.** Given the query's offset Δ in the group's coordinates and the row's stored u = ‖D⁻¹e‖, the standard
deviation of Δ·e is approximately ρ_ℓ(q, c)·h, where h = w·u is the certified half-width's code term and

    ρ_ℓ(q, c)² = Σ_i Δ_i²·ν_i / ( w² · Σ_i a_i ) = E_ω[a] / ‖a‖₁ ,    ω_i = D_i²·Δ_i² ,              (A3.1)

E_ω[a] the mean of a_i under the weights ω_i. The rule: read a row further when the midpoint plus
min(1, ψ·ρ_ℓ(q, c))·h reaches t̂. Then:

1. On the profile (Δ_i² = π_i) it is rule 1's ρ_(g,ℓ) = ‖a‖₂/‖a‖₁.
2. min_i a_i/‖a‖₁ ≤ ρ_ℓ(q, c)² ≤ max_i a_i/‖a‖₁ ≤ 1.
3. ρ_ℓ(q, c)²/ρ_(g,ℓ)² = E_ω[a]/E_a[a]: the factor differs from the profile's only as far as the query's energy
   relative to the profile, Δ_i²/π_i, is correlated with a_i across coordinates.
4. If a_i is the same for every coordinate, ρ_ℓ(q, c) = ρ_(g,ℓ) for every query. Under a reverse water-filling
   allocation a_i is constant on the coordinates the level codes and smaller on those it leaves uncoded, so the
   rule changes something only through the uncoded coordinates.

**Proof.** E[(Δ·e)² | κ] = κ²·Σ_i Δ_i²·ν_i by uncorrelated coordinates. u² = κ²·Σ_i (ν_i/D_i²)·ξ_i², whose mean
given κ is κ²·Σ_i a_i, since ν_i/D_i² = √(ν_i·π_i); replacing u² by that mean is the approximation, good when many
coordinates carry the sum. So κ² ≈ u²/‖a‖₁ and E[(Δ·e)² | u] ≈ u²·Σ_i Δ_i²ν_i/‖a‖₁ = (ρ_ℓ(q, c)·w·u)². Writing
Δ_i²·ν_i = (D_i²Δ_i²)·(ν_i/D_i²) = ω_i·a_i and w² = Σ_i ω_i gives the second form. (1): ω_i = D_i²π_i = a_i, so
E_ω[a] = Σ a_i²/Σ a_i. (2): a weighted mean lies between the least and the greatest. (3) and (4) follow from the
second form. ∎

### A3.2 Its cost [derived from the code as read]

Σ_i Δ_i²·ν_i with Δ = q̃ − α·m̃ is q̃ᵀNq̃ − 2α·m̃ᵀNq̃ + α²·m̃ᵀNm̃, N the diagonal matrix of the ν_i. That is the form in
which `cluster_q2w` (`rust/src/v2/q.rs`) already computes w² with D² in place of N (`gq.cq`, `gq.qvd2`, `c.bm`). So
the rule adds, per level: one n-vector and one n-term sum per (query, group) at setup; one n-term dot product per
(query, cluster); one stored number per cluster. At n = 512 and two levels that is about 1,000 multiply-adds per
cluster opened on the index path, against a first-level pass of about 2,000 rows. A per-(query, group) version
drops the dot product by using the group's mean centre for m̃.

### A3.3 What it changes [computed, with a stand-in for the build's allocation]

`scripts/a3_check.py`, `out/a3_check.txt`. The ν_i here are a weighted reverse water-filling at the level's bits,
not `det_alloc_tree`'s; they give ρ_(g,ℓ) = 0.0526, 0.0606 and 0.1000 at 16 B against the build's 0.0561, 0.0638
and 0.1013 (`layered.md` 4.2). Δ = Vᵀ(q − α·m_c), over each query's ten best clusters by centre score.

| ρ_ℓ(q, c)/ρ_(g,ℓ): 5th, 50th, 95th percentile; rms | 16 B | 32 B | measured over derived (`layered.md` 4.2), 16 B / 32 B |
|---|---|---|---|
| LAION, text queries | 0.82, 0.92, 0.99; 0.915 | 0.89, 0.95, 1.00; 0.948 | 0.865 / 0.871 |
| LAION, corpus rows as queries | 0.90, 1.00, 1.06; 0.996 | 0.94, 1.00, 1.04; 0.999 | |
| yahoo, test queries | 0.96, 1.00, 1.02; 0.993 | 0.97, 1.00, 1.01; 0.995 | 0.961 / 0.956 |
| glove, test queries | 1.00, 1.00, 1.00; 1.000 | 1.00, 1.00, 1.00; 1.000 | 0.981 / 0.978 |

- On LAION's text queries the rule removes 8.5 of the 13.5 points by which the derived width exceeds the measured
  one at 16 B, and 5 of 13 at 32 B. In distribution it changes the width by under 1%.
- It does not explain the spread of the measured ρ·√d across (query, cluster) pairs in distribution (5th to 95th
  percentile 1.14 to 1.44 on yahoo): the rule's own spread there is 0.96 to 1.02.
- A narrower width at the same ψ is the same as a smaller ψ. So on a workload of one kind of query the rule buys
  nothing a retuned ψ does not; what it buys is that one ψ means the same for queries in and out of distribution.

### A3.4 The built rule uses one number per level [read in the code]

At commit 370580d8e, `Adaptive::new` takes, for each deterministic level, the median over the groups of
`GroupPlan2::rho` and `dial` returns min(1, ψ·ρ_ℓ) per level. So rule 1 as built is per level, not per group as
`layered.md` 4.1 states it. The groups' values span 0.0512 to 0.0657 on LAION at 16 B (`layered.md` 4.2), 9% below
to 17% above the median.
