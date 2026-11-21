# Approximate track: claims

21 November 2026, revised the same day after referee report 1 (`referee/referee-1.md`). Every claim carries one tag
of `notes/README.md`. Proofs are mine. Seven claims (A1.1, A1.2, A1.4, A1.5, A2.1, A2.2, A3.1) have been through one
referee's report and are revised here; no tag is raised on that account, since the revision has not been
re-refereed. What changed is listed in "Changes after referee 1" at the end; the text before the report is
`out/CLAIMS_before_referee1.md`. Measured claims name the dataset, the size and the script; the tables are in
`out/tables.md`. How each was reached, and what failed on the way, is in `NOTEBOOK.md`.

## Local symbols

Chapter 0's symbols are used as given. These are local to this file.

- **A tree.** Levels j = 0, …, D; the root is level 0 and the leaves are level D. The tree is **regular**: every
  level-(j−1) node has the same number F_j = K_j/K_(j−1) of children, the fan-out of level j. K_j is the number of
  nodes at level j and M = K_D the number of leaves. A level is **branching** if F_j > 1. Unless a claim says
  otherwise the leaves are the rows with their exact scores (M = N) and every level is branching.
- **Entropy position and variance share.** g_j = ln K_j / ln M, non-decreasing in j and strictly increasing over the
  branching levels, with g_0 = 0 and g_D = 1. f_j is the share of the leaf values' variance explained by level j,
  with f_0 = 0 and f_D = 1. These are `hierarchy/theory.md`'s g_j and f_j; they are not chapter 0's density f or
  g_c = n_c·p_c, which this file does not use.
- **Two profiles.** A_step(t) is the step function that jumps by f_j − f_(j−1) at t = g_j: it is Model T's own
  profile. A(t), t in [0, 1], is the piecewise-linear function through the points (g_j, f_j) of the branching
  levels: it is the profile of Model T's **binary refinement** (A1.1), not of Model T. a is the derivative of A, Â
  the concave hull of the points (the same for both profiles) and â the hull's derivative. Addario-Berry and
  Maillard's theorem takes an A that is the integral of a Riemann-integrable a, which A is and A_step is not.
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

### A1.1 Model T is a generalised random energy model with a step profile [proved, unrefereed; the theorem cited]

**Statement.** Two models, which are different processes.

1. **Model T** on a regular tree, with D levels, fan-outs F_j and variance shares f_j, is Derrida's generalised random
   energy model with D levels, entropy shares g_j − g_(j−1) and variance shares f_j − f_(j−1). Placed on a binary
   tree it is the continuous model whose profile is the step function A_step.
2. **Its binary refinement**, defined when every log₂ F_j is an integer and the g_j of the branching levels increase
   strictly, is the continuous random energy model on the binary tree of depth log₂ M with the piecewise-linear
   profile A: every level j is replaced by log₂ F_j binary levels of equal variance.

The two agree in three things and differ in two.

- **Agree.** (a) The law of the values along one root-to-leaf path at the depths g_j. (b) The expected number of
  nodes of one level above a cut, so the B_j of `hierarchy/theory.md` Proposition 1. (c) The concave hull, which is
  the hull of the points (g_j, f_j) for both profiles, so the ground state x_s.
- **Differ.** (d) x_*. Theorem 1.1 below needs A to be the integral of a Riemann-integrable a. It is a theorem
  about the refinement and says nothing about Model T. For A_step the formula gives ∫√a = 0, which is right in
  substance: under a flat fan-out of M^(g_j − g_(j−1)) children with no levels inside, every gain costs a power of M.
  (e) The floor of A1.2. Its proof needs the leaves under one parent independent given the internal values with
  that parent explaining a share f_(D−1). In the refinement the internal values include the binary levels inside
  the last level, which explain nearly everything, so the floor of Model T does not transfer to it.

**Hypotheses, of the statement and not only of the mapping.** A regular tree. Integer log₂ F_j for the refinement.
Gaussian increments. An increment independent of its parent's value. **Siblings' increments independent of each
other.** Equal within-node variance at a level.

**Proof.** (1) is Derrida's definition [cited, unchecked: from memory and as Addario-Berry and Maillard describe it;
their section 1 defines the continuous model and calls it an extension of Derrida's]. In Model T two leaves whose
last common ancestor is at level j − 1 have covariance v·f_(j−1), which is v·A_step at any binary depth between
g_(j−1) and g_j. (2) is the definition of the continuous model (Addario-Berry and Maillard, section 1: a centred
Gaussian process on the binary tree with E[X_v X_w] = N·A(|v ∧ w|/N), equivalently independent increments of
variance N(A((i+1)/N) − A(i/N)) at generation i); there the same two leaves have covariance v·A at the depth of
their last common binary ancestor, anywhere in [v·f_(j−1), v·f_j). So the processes differ. (a) and (b): a node at
binary depth g_j·log₂ M of the refinement has value N(0, v·f_j), and a path's values at those depths have
independent increments of variance v·(f_j − f_(j−1)), as in Model T. (c): the hull of a step function with jumps at
the g_j and of the interpolation through the same points is the hull of the points. (d) and (e) are argued in the
statement. ∎

**The mapping of an index** (not a claim about data).

| index | model |
|---|---|
| one query q | one realisation of the energies; a different q is a different landscape on the same tree |
| a row's score s(x) | a leaf's value U |
| a group, a cluster, a sub-cluster or leaf run | a node at a branching level |
| the mean score of a node's rows (μ_c for a cluster; close to the centre score q·m_c) | the node's value Y_j |
| reading a centre score | one query of an internal node |
| a row at its key | where the row levels are modelled: the last branching level, of fan-out n_c under the cluster |
| a row at the 16 B level, at the 32 B level, its stored vector | levels of fan-out 1 below the key: g stays at 1, f rises to 1, so A_step has its last jumps at t = 1 and A is not defined through them |
| reading a stored vector | one query of a leaf |
| the k nearest neighbours | the k best leaves |
| g_j, f_j | ln(cells)/ln N, and the share of the variance of s(x) over the rows that the cells' means explain |

Everywhere but remark (iii) of A1.2 this file uses the tree whose leaves are the rows' exact scores and whose
levels are all branching.

**Where the mapping departs from the model before any data is looked at.** A node's value mapped to the mean of its
rows makes the children's increments sum to zero, weighted by size, so siblings are correlated −1/(F − 1): nothing
at F = 2,000, a third at the 4-way splits (a4, a16, a64) of A1.7. Model T's node value is a latent variable, not a
mean. A1.7 tests the Gaussian increments, the independence from the parent and the equal variance; it does not test
the independence of siblings.

**Cited.** Addario-Berry and Maillard, arXiv:1810.05129, Theorem 1.1 [cited: I read the statement, the whole proof
(sections 2 and 3), section 4 and the open questions]. For A = ∫a with a Riemann-integrable and
x_* = √(2 log 2)·∫₀¹ √a: (1) for x < x_* there is C(x) such that for every ε > 0 and all large N an algorithm stops
within CN queries at a leaf with X_v ≥ xN with probability above 1 − ε; (2) for x > x_* there is γ = γ(A, x) > 0
such that, for N large, for every algorithm that queries node values the number of queries before a leaf with
X_v ≥ xN is found dominates a Geometric(e^(−γN)) variable. The ground state is x_s = √(2 log 2)·∫₀¹ √â (Bovier and
Kurkova, quoted there as (1.1)) [cited, unchecked: through that paper]. x_* = x_s if and only if A is concave.

### A1.2 The leaf floor at a fixed level [proved, unrefereed]

**Statement.** In Model T with 0 < f < 1, f = f_(D−1), give an algorithm the value of every internal node for
nothing. Let it query leaves one at a time, adaptively, with randomness of its own that is independent of the leaf
values, and stop when it chooses; a target (a leaf with U ≥ T, T fixed) is found when its leaf is queried. Let
r < 1. If the expected number of targets found is at least r times the expected number of targets, then the
expected number of leaves queried is at least

    R(r) = M · P(Y_(D−1) > a_r),     a_r defined by  P(Y_(D−1) > a_r, U ≥ T) = r · P(U ≥ T),

where (Y_(D−1), U) are jointly normal with Var Y_(D−1) = v·f, Var U = v and covariance v·f. The rule "query the
leaves whose parent's value exceeds a_r" attains it.

**What "recall r" means here.** A ratio of expectations: E[found] ≥ r·E[targets]. With T fixed the number of targets
in a tree is random and over-dispersed (targets come in clumps under a few high parents), and the ratio weights the
trees by their target counts. It is not the mean over trees of a tree's recall.

**Proof.** Let 𝒢 be the σ-field of the internal values. Given 𝒢 the leaf values are independent, and leaf i has
U_i ~ N(Y_par(i), v(1 − f)), because U_i = Y_par(i) + ε_(D,i) and the ε_(D,i) are independent of each other and of
𝒢. Write p_i = Φ̄((T − Y_par(i))/√(v(1 − f))), which is 𝒢-measurable.

Let Q be the set of leaves queried. Fix a leaf i. Run the same rule on the input in which U_i is replaced by a
constant: the two runs agree until i is first queried, so the event {i ∈ Q} is a function of
(𝒢, (U_j)_(j≠i), the randomness). Given 𝒢, U_i is independent of that triple. Hence
P(U_i ≥ T, i ∈ Q) = E[1{i ∈ Q}·p_i], and

    E[targets found] = E[ Σ_(i ∈ Q) p_i ] .

For every λ ≥ 0 and every set Q, Σ_(i ∈ Q) p_i ≤ λ|Q| + Σ_i (p_i − λ)₊. Taking expectations,
E[targets found] ≤ λ·E|Q| + M·E(p − λ)₊, with p = Φ̄((T − Y)/√(v(1 − f))) and Y ~ N(0, v·f). Choose λ_r > 0 with
M·E[p; p > λ_r] = r·M·P(U ≥ T); it exists because p has a continuous law (0 < f < 1) and r < 1. Then

    E|Q| ≥ ( r·M·P(U ≥ T) − M·E(p − λ_r)₊ ) / λ_r = M·P(p > λ_r) = M·P(Y > a_r),

since E[p; p > λ] − E[(p − λ); p > λ] = λ·P(p > λ), and {p > λ_r} = {Y > a_r} for the a_r of the statement
(E[p; Y > a] = P(Y > a, U ≥ T)). The rule Q = {i : p_i > λ_r} has E|Q| = M·P(p > λ_r) and finds exactly the required
expected number. ∎

**Stronger than stated** (the referee's observation; I checked it against the proof). The proof uses nothing of the
tree above the leaves' parents and nothing of 𝒢 beyond each leaf's own parent. So the floor holds for any tree
shape, and for an algorithm told any further information that is independent of the ε_(D,i).

**Checks.** Mine: `scripts/grem_check.py` part A, quadrature. The referee's (`referee/ref1_a12.py`): 12,000 trees of
200 × 500 leaves, six leaf-query rules of which four adaptive; none is above the frontier, the two built to sit on
it are within 0.05 targets of it (standard error 0.10).

**Remarks.** (ii) The floor counts leaf queries only, whatever the internal queries cost. So in the model, and for
targets above a fixed level under the criterion above, it is a floor on the rows read by any hierarchy whose finest
centres explain a share f, under any order, stop or beam. (iii) With one leaf per parent the proof is unchanged, so
applied one level lower it is a floor on the stored vectors read given the 16 B or 32 B level, with f the share
those levels explain. (Remark (i) of the first version, on the k best, is replaced by A1.2b.)

### A1.2b The leaf floor for the k best [proved, unrefereed; the theorem is the referee's, the numbers computed]

**Statement.** In Model T with 0 < f < 1, under the conditions of A1.2 on the algorithm, let the targets be the k
leaves of largest value. For leaf i let τ_(−i) be the k-th largest value among the **other** leaves, siblings
included, and p′_i = Φ̄((τ_(−i) − Y_par(i))/√(v(1 − f))). If the expected number of the k best that are found is at
least r·k, then

    E|Q| ≥ sup over λ > 0 of ( r·k − E Σ_i (p′_i − λ)₊ ) / λ .                                  (A1.2b)

**Proof.** Leaf i is among the k best if and only if U_i > τ_(−i) (ties have probability zero). τ_(−i) is a function
of (U_j)_(j≠i), and {i ∈ Q} is a function of (𝒢, (U_j)_(j≠i), the randomness) as in A1.2. Given that triple
U_i ~ N(Y_par(i), v(1 − f)). So P(i among the k best, i ∈ Q) = E[1{i ∈ Q}·p′_i] and E[found] = E Σ_(i∈Q) p′_i. For
every λ > 0 and every Q, Σ_(i∈Q) p′_i ≤ λ|Q| + Σ_i (p′_i − λ)₊. Take expectations and use E[found] ≥ r·k. ∎

No concentration of τ_k is needed, and the gap named in the first version is closed. I checked the proof line by
line; it is the proof of A1.2 with p′_i for p_i, which is allowed because p′_i is a function of what {i ∈ Q} is a
function of.

**What it is and is not.**

- A leaf outside the k best has τ_(−i) = τ_k, and a leaf among them the (k+1)-th best. So the right side is the
  knapsack, at one price over trees, of a rule that is told τ_k. Σ_i E p′_i = k.
- **It is not attained**: no algorithm knows τ_k. The least expected rows of a rule that knows every internal value
  and not τ_k (a static rule at one price over trees, with p_c = E[F̄_c(τ_k)] of `theory.md` (3.1)) is attainable
  and lies 1 to 4% above the floor in the cells below. The true cost of the k best lies between the two.
- **It is not R(r).** R(r) of A1.2 at the level with M·Φ̄(z_T) = k is not proved to be a floor for the k best. In
  every cell computed it lies below (A1.2b).

**Numbers** [computed], k = 10, r = 0.9, i.i.d. parents of variance f. The referee's (`referee/ref1_a12c.py`, in the
Poisson form of `theory.md` (3.1), no leaf sampled):

| K × F | f | R(0.9), fixed level | floor (A1.2b) | static optimum | floor / R |
|---|---|---|---|---|---|
| 500 × 2,000 | 0.30 | 85,084 | 89,482 | 92,742 | 1.05 |
| 500 × 2,000 | 0.45 | 20,735 | 28,318 | 29,229 | 1.37 |
| 500 × 2,000 | 0.60 | 4,198 | 11,898 | 11,984 | 2.83 |
| 339 × 2,000 | 0.45 | 16,092 | 24,884 | 25,854 | 1.55 |
| 339 × 2,000 | 0.60 | 3,447 | 10,682 | 11,059 | 3.10 |
| 28 × 784 | 0.45 | 1,681 | 2,907 | 3,030 | 1.73 |
| 5,400 × 125 | 0.45 | 16,046 | 16,986 | 17,553 | 1.06 |

Mine, with the leaves sampled and no Poisson form (`scripts/topk_model.py check`, `out/topk_model_check.txt`; the k
leaves of the k best take the (k+1)-th best, as the theorem says): floor / R = 1.75 at 28 × 784, f = 0.45 (the
referee's 1.73); 1.26 at 28 × 784, f = 0.30 (1.21); 1.48 at 100 × 500, f = 0.45; 2.45 at 100 × 500, f = 0.60.

On trees with the three fits' measured shares, with the levels above the finest cells sampled
(`scripts/topk_model.py fits`, `out/topk_model_fits.txt`), the static optimum is 1.42, 1.50 and 1.19 times R(0.9) at
the cluster level (yahoo, LAION, glove), 1.69, 1.88 and 1.30 with 125-row leaves under the clusters and 1.38, 1.76
and 1.41 with 31-row leaves. A caution on the floor's own column in those runs: it came out above the static
optimum in 11 of 18 cells, by 0.1 to 19%, which cannot be (a floor is below what is attainable). The runs have 360
to 4,800 draws of τ_k a cell, in the Poisson form. That column is too noisy at those sample sizes, so I quote the
static optimum, which averages over τ_k analytically, and not the floor, from those runs. It is noise: at LAION's cluster-level cell with 19,200 draws a seed the floor is 0.998, 0.946 and 0.960 of the
static optimum at r = 0.9 over three seeds (`scripts/topk_told_diag.py`, `out/topk_told_diag.txt`), a spread of
about 3% between seeds, and the mean of Λ(τ_k) is 10.0 as it should be. The referee's table, with independent
parents, does not show the inversion at r = 0.9.

So with a few hundred parents the floor for the k best is 1.05 to 3.1 times R(r), and the cause is that τ_k does
not concentrate: over trees it has a standard deviation of 0.15 to 0.24 √v at K = 339 to 500 and f = 0.45 to 0.6,
against 0.03 to 0.12 at K = 5,400 (`referee/ref1_a12.txt`). The difference is the whole effect at the cluster level,
not a technicality. At r = 0.99 the static optimum is 1.0 to 2.0 times R(r); the floor is noisy there and not
quoted.

### A1.3 The fixed-level floor's closed form and its exponent [computed]

**Statement.** All of this is for targets above a fixed level, A1.2; none of it is for the k best.
R(r) = M·Φ̄(z_T·√f − z_r·√(1 − f))·c, with c between 0.60 and 0.90 for M = 10⁶ and 10⁹, L = 10 and 100, f = 0.3 to
0.7 and r = 0.9 and 0.99 (the closed form with c = 1 is `hierarchy/theory.md` (2.1) at the leaf level; it puts every
target at T, and the targets lie above T). With z_T = z_REM,
R(r) = M^(1−f)·L^f·exp(z_r·z_T·√(f(1 − f)) + O(log z_T)), so the exponent of R in M tends to 1 − f. At M = 10⁶ the
local exponent d ln R / d ln M is 0.08 to 0.22 above 1 − f: 0.60 to 0.72 for f = 0.5, 0.40 to 0.51 for f = 0.7.

**For the k best the figure is higher, by a factor that depends on the tree.** By A1.2b the floor is 1.05 to 3.1
times R(0.9) with a few hundred independent parents. On trees with the three fits' shares the attainable static
optimum is 1.19 to 1.50 times R(0.9) at the cluster level and 1.30 to 1.88 times with leaves of 125 rows under the
clusters: finer cells do not remove the factor, because the clusters above them still move τ_k
(`out/topk_model_fits.txt`). The closed form with c = 1 errs high against R (c < 1) and low against the k best, and
at the cluster level of the three fits the two nearly cancel: the model's rows when every query reads the same
number of clusters are 1.08, 1.05 and 0.91 times the closed form at r = 0.9 (A1.7 item 6). That is an accident of f
near 0.4 and a few hundred clusters, not a property of the closed form. The local exponent under the build rule
moves by at most 0.04 (A1.7 item 7).

**Source.** `scripts/grem_check.py` part A (numerical integration of the bivariate normal), `out/grem_check.txt`;
`scripts/topk_model.py`, `out/topk_model_fits.txt`.

### A1.4 The work of a beam, and where the levels should sit [model]

**Statement.** In Model T on a regular tree with f_1 > 0, with targets at the level z_REM for L expected targets
and the beam of `hierarchy/theory.md` Proposition 1 (keep a level-j node when
Y_j ≥ √v·(z_T·f_j − z_ε·√(f_j(1 − f_j))), and score the children of a node only if it and every ancestor were
kept), the expected number of nodes scored is **at most**

    W = Σ_(j=1..D) B_(j−1)·F_j ,    B_0 = 1,   B_j = K_j·Φ̄(z_T·√f_j − z_ε·√(1 − f_j)) ,

and W = M^(η + o(1)), as is the beam's own count, as M grows with D, ε, the g_j and the f_j fixed and
ln L = o(ln M), where

    η = max over j = 1, …, D of ( g_j − f_(j−1) ) .                                             (A1.4)

For a profile A that is continuous and non-decreasing, and under the assumption that a level placed at entropy
position g explains the share A(g) whatever the other levels are, the level positions that minimise η satisfy
g_1 = η and g_j = η + A(g_(j−1)), ending at g_D = 1. As D grows, after M, the minimum decreases to sup_t (t − A(t)),
which is zero if and only if A(t) ≥ t for every t.

**Derivation.**

1. *W is an upper bound.* B_j counts the level-j nodes above their own cut whatever their ancestors did; the beam
   scores the children of a node only if every ancestor was kept, so its count at level j is at most B_(j−1)·F_j.
   On trees of 28³ leaves the beam scores 3 to 7% fewer nodes than W (`referee/ref1_a14_a15.py` part A, 400 trees;
   it reaches 0.988 to 0.991 of the targets against the union bound's 0.98).
2. *The exponent.* B_j = K_j·exp(−f_j·z_T²/2 + O(z_T)) and z_T² = 2 ln(M/L) + O(log log M), so with ln L = o(ln M),
   B_j = M^(g_j − f_j + o(1)) and B_(j−1)·F_j = M^(g_j − f_(j−1) + o(1)). The beam's own count has the same exponent:
   given a node at its cut, its ancestor's conditional mean lies above the ancestor's cut by a positive multiple of
   z_ε·√v, because f_(j−2)(1 − f_(j−1)) < f_(j−1)(1 − f_(j−2)), so every ancestor is kept with probability bounded
   below by a constant (the referee's remark; with L = M^β the exponent of level j would be g_j − (1 − β)·f_(j−1)).
3. *The positions* (the referee's proof, which I checked). Let G_D(η) be the last iterate of g_1 = η,
   g_j = η + A(g_(j−1)). For any positions whose largest term is η, induction with A non-decreasing gives
   g_j ≤ the j-th iterate, so 1 = g_D ≤ G_D(η). G_D is continuous and strictly increasing with G_D(0) = 0, so η is
   at least its root, and the iterates at the root attain it.
4. *The limit in D.* For η ≤ s = sup_t (t − A(t)) the iterates stay below the point where t − A(t) ≥ η and never
   reach 1; for η > s each step gains at least η − s. So the minimum decreases to s. The order of limits is M → ∞
   first, then D → ∞. On a binary tree D = log₂ M and the limits are taken together; the corrections stay M^(o(1)),
   which this derivation does not show.

**The assumption about a level's share.** It holds by definition in the binary refinement of A1.1, where
Proposition 1 holds too. For an index it is an assumption about how nested and non-nested partitions relate (a flat
k-means at K cells and a nested split to K cells need not explain the same share), and it is not tested here.

**The exponent does not order the terms at N = 10⁶.** The first term, K_1, has no correction. Term j ≥ 2 carries
exp(z_ε·z_T·√(f_(j−1)(1 − f_(j−1)))), two orders of magnitude at N = 10⁶ and f near ½. With the profiles of A1.7,
L = 10 and a total miss of 0.1 split over the pruning levels, ln(row term)/ln N is 0.72 to 0.84 where the exponent
is 0.41 to 0.64, and the row term is the largest by count in all twelve designs of A1.9, by 20 to 100 times
(`referee/ref1_a14_a15.txt` part B). So (A1.4) is a statement about the limit. What W says at a given size is in
A1.9, by counts.

**Why [model] and not [proved].** The statement is an expectation over trees. The first moment is the typical
count where f_i < g_i at every level above (the straight path in variance then has M^(g_i − f_i) nodes at level
i). Where f_i > g_i the expectation is carried by rare trees: the level then holds no leaf with high probability
(A1.5), and the formula describes targets that do not exist. At f_i = g_i, **which is where the measured profiles
sit at the cluster level**, the expected number of level-i nodes at the straight path's height is L^(f_i) times a
negative power of z_T: it tends to zero slowly, and the first moment is carried by rare trees there too. W is an
upper bound on the cost of finding the targets, not a lower bound: the only lower bounds here are A1.2 and A1.2b,
on its last term.

**What has and has not been checked.** `scripts/grem_check.py` part C solves the recursion for the positions and
prints its root (for A(t) = t, η = 1/D; for A(t) = t², η = 0.62, 0.48, 0.42 at D = 2, 3, 4 and sup(t − A) = 0.25;
for A(t) = √t, η = 0.38, 0.20, 0.11). It builds no tree and counts no node: it is arithmetic on the formula, not a
check of it. The check of W on trees is the referee's part A (item 1). Nothing checks the exponent, and no
simulation at reachable sizes could, its correction being 0.15 to 0.3 at N = 10⁶.

**Consequences for an index, in the model's limit.**

- A level of fan-out 1 below the last branching level adds no term and leaves η unchanged, which is the case of the
  row's codes. (A fan-out-1 level placed above a branching level would lower that level's term, so this is a
  statement about this tree, not a general one.)
- A new branching level at entropy position g lowers η if and only if the maximum is attained by one term only,
  the level splits that term, and A(g) exceeds the share of the level above.
- With the clusters as the only branching level, η = max(g_1, 1 − f_1): the centres, or the rows under them.

### A1.5 Which dichotomy Model T's is [proved, unrefereed; one part computed, one part a published conjecture]

Throughout, A is the profile of the binary refinement (A1.1). The library note (`library/codes-and-search.md` 2.2)
says that Model T's regime "f_j ≥ g_j, beam subpolynomial" is A(t) ≥ t, which every concave A satisfies, and that
"f_j < g_j, beam polynomial" is a non-concave A. Checked:

1. **True in one direction.** A concave A with A(0) = 0 and A(1) = 1 lies on or above its chord, so A(t) ≥ t. Hence
   f_j < g_j at some level implies A is not concave. [proved: the chord property of a concave function]
2. **False in the other.** A(t) ≥ t does not imply concavity. Shares (0.60, 0.10, 0.30) on three levels of equal
   fan-out give A(1/3) = 0.60 and A(2/3) = 0.70, both above t, with slopes 1.8, 0.3 and 0.9, which is not concave.
   [proved by the example; the first version's (0.60, 0.07, 0.33) cleared the diagonal at 2/3 by 0.003 only]
3. **Three classes cut by two lines, and they answer two questions.**
   - Model T's "subpolynomial" side is the profiles on or above the diagonal, A(t) ≥ t for every t.
   - Bovier and Kurkova's weak-correlation regime is the **other** side: A(t) ≤ t for every t, which is when
     x_s = √(2 log 2). These are two different splits by the same line. A profile that crosses the diagonal is on
     the bad side of both, and only A(t) = t is on the good side of both. (The first version identified Model T's
     split with the weak against strong regimes; that was wrong.)
   - Addario-Berry and Maillard's split is concavity, a subset of the profiles above the diagonal.
   - For every profile above the diagonal other than the diagonal itself, ∫√â < 1, so x_s lies below the level
     z_REM and, when ln L = o(ln M), that level holds no leaf with high probability. So where A(t) ≥ t and A is not
     concave, Model T's formula (a subpolynomial count) and the theorem (exponential cost above x_*) do not
     conflict. They also answer different questions: the beam formula counts the nodes kept to reach **every**
     leaf above a level, and at a level x < 1 its last term is the M^(1 − x²) leaves there are; the theorem is
     about reaching **one**. Neither is the cost of the k best. [proved given the cited ground state]
4. **A number that was right and an inference that was not** [the inference withdrawn]. For shares (0.6, 0.25,
   0.15), fan-out 28 and the top 64 of 21,952 leaves, the realised level over z_REM is 0.967 over 200 trees, and the
   hull sum Σ_j √((f_j − f_(j−1))(g_j − g_(j−1))) is 0.960. I took that as the realised level following the hull. The
   referee's table (`referee/ref1_a14_a15.txt` part C) shows the agreement is an accident of L = 64: for the same
   tree the ratio is 0.90 at L = 1, 0.93 at L = 8, 0.97 at L = 64 and 1.00 at L = 512, and for other strong
   profiles it misses the hull sum by 4 to 6 points at L = 64. The hull sum is the limit of the maximum over
   √(2 ln M), which is the cited theorem and needs no simulation. At 21,952 leaves it does not predict the realised
   level to better than about 0.05. A1.7 item 8 is revised accordingly.
5. **At the ground state, in the weak regime, the beam exponent equals the conjectured pool exponent.** The path
   that attains the ground state has derivative b = a·√(2 log 2/â) (Addario-Berry and Maillard (1.3)), so
   b²/(2a) = (log 2)·a/â and the logarithm of the expected number of nodes at position t along it, in units of ln M,
   is t − ∫₀ᵗ a/â. When A(t) ≤ t for all t, â = 1 and this is t − A(t), whose supremum is the deep-tree limit of
   (A1.4). Their open question 2, formula (5.1), conjectures that this pool is the cost exponent above x_*. Two
   limits on what this says: it is the pool of the one path that ends at x_s, so it equals γ_*(x)/log 2 of (5.1) at
   x = x_s only (for x < x_s cheaper paths exist); and the left side is the D → ∞ limit of an expected count, in the
   joint sense of A1.4's item 4. [the identity is computed from (1.3); that it is the cost for every algorithm is
   their **[conjecture]**, not a theorem]

### A1.6 What of the hardness transfers [model; the theorem cited]

**Statement.** The theorem is about the binary refinement (A1.1), not about Model T. If the scores along a query
were a continuous random energy model with a profile A = ∫a that is not concave, then for N large every algorithm
that reads node values, adaptively and with randomness, would need a number of reads that dominates a geometric
variable of mean N^(γ/ln 2) to return even one row above the level x_*, for some γ > 0; the best rows lie at
x_s > x_*. So no hierarchy of centres, with any order, beam or stop, would read polylog(N) nodes. If the profile
were concave, a renormalised greedy descent would reach within any fixed fraction of the best level in O(log N)
reads, with probability near one.

**The query model.** One unit per node value: a centre score is an internal node, a stored vector a leaf. The
theorem counts nodes, not bytes, and gives no value of γ.

**What does not transfer, in order of weight.**

1. **Flat fan-outs.** The theorem's tree is binary: a level of 2,000 children has eleven binary levels inside it to
   descend. A cluster has none, so inside a level there is nothing to do but read the children; Model T's own
   profile is the step function, for which x_* = 0 (A1.1). For the index's tree (two to four branching levels) the
   bounds that apply are A1.2 and A1.2b from below and W of A1.4 from above. They agree in the exponent when the
   last term of (A1.4) is the largest, and at N = 10⁶ they are not close: W's row term carries
   exp(z_ε·z_T·√(f(1 − f))) and a union bound over the levels.
2. **All of the k best, not one leaf above a level.** The read is an enumeration to a recall. Its floors, A1.2 and
   A1.2b, are present whatever the shape of A: with a concave profile and D levels the rows read are still of
   order M^(1 − f_(D−1)). Concavity decides whether more levels can drive that to zero, not whether the floor exists
   at a given D.
3. **The data are not the model** (A1.7): the model's rows at the cluster level are 2.4 times the measured ones on
   yahoo with μ + 3σ and about equal on LAION and glove, so the model's floor is not a floor for the data.
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
5. **The closed form's two ingredients, tested with each query's own τ_k.** This item tests the closed form of
   A1.3 (coverage of the neighbours' clusters by a cut on the centre score, and the share of rows above that cut),
   given τ_k. It is not a test of a floor. Share of rows in clusters above the cut at nominal coverage 0.9: glove
   6.4% measured against 7.2%, with coverage 0.90; yahoo 1.2% against 0.15%, with coverage 0.82. So both
   ingredients hold on glove, and on yahoo the count fails by 8 times at the measured level.
6. **In rank space the model is right to a factor of about two** [revised after the referee: the model's figure is
   now its own value of the statistic measured, not the closed form]. The statistic is the share of rows read when
   every query reads the same number of cells in the order of their keys, at mean recall 0.9. The model's value,
   with the level at its Gaussian value, the k best random and the measured shares on a regular tree
   (`scripts/topk_model.py`, `out/topk_model_fits.txt`), against the measured share by centre score and by μ + 3σ:

   | finest cells | yahoo: model, centre, μ + 3σ | LAION | glove |
   |---|---|---|---|
   | clusters | 4.9%, 2.7%, 2.0% | 3.4%, 6.5%, 2.9% | 6.1%, 8.7%, 5.7% |
   | a16 | 2.0%, 0.95%, 0.50% | 1.5%, 2.5%, 0.98% | 2.8%, 5.4%, 2.1% |
   | a64 | 0.98%, 0.56%, 0.20% | 0.88%, 1.5%, 0.39% | 2.0%, 3.3%, 0.94% |

   At the cluster level the data are easier than the model on yahoo (2.4 times with μ + 3σ); on LAION's text
   queries and on glove the centre score alone is worse than the model (1.9 and 1.4 times) and μ + 3σ is about equal
   to it (0.88 and 0.93). Below the clusters μ + 3σ beats the model on all three, by 1.3 to 4.9 times. The closed
   form that the first version used here gave 4.5%, 3.2% and 6.8% at the cluster level, within 10% of the model's
   value, by the cancellation described in A1.3.
7. **Exponents.** In cells at fixed N (K to 64K), the share falls as cells^(−γ) with γ = 0.38, 0.36 and 0.23 (centre
   score) and 0.56, 0.48 and 0.43 (μ + 3σ) on yahoo, LAION and glove; the model gives 0.39, 0.32 and 0.28. At a fixed
   cell size over a 16-fold range of N the rows grow as N^η with η = 0.47, 0.67 and 0.70 (centre score), 0.36, 0.51
   and 0.52 (μ + 3σ); the closed form gives 0.38, 0.37 and 0.50 (the model's own value was not computed for
   subsamples). Under the build rule K ∝ √N, with f = A(½) held fixed and the k best random, the model's local
   exponent from N = 10⁶ to 1.6·10⁷ is 0.68, 0.62 and 0.70 at recall 0.9 and 0.71, 0.65 and 0.73 at 0.95
   (`out/topk_model_scaling.txt`; the fixed-level R gives 0.64, 0.62, 0.71 and 0.68, 0.65, 0.74);
   `hierarchy/layered.md` section 2 measures γ/2 + η = 0.55, 0.61 and 0.65 at 0.9 and 0.60, 0.66 and 0.73 at 0.95.
   The model is within 0.05 on LAION and glove and 0.11 to 0.13 high on yahoo. That the profile stays fixed as N
   grows is assumed, not measured.
8. **In the model's limit the measured profiles are in the hard regime; the size of the margin cannot be read at
   these sizes** [weakened after the referee]. The profiles are not concave: that rests on the slopes of item 1 (the
   last segment is the steepest on all three), so in the limit of a binary refinement with these profiles
   x_* < x_s. The first version went further and read a margin of 2.5 to 5% from Σ_j √(Δf_j·Δg_j) = 0.975, 0.975
   and 0.954. That reading is withdrawn: by A1.5 item 4 the sum does not predict a realised level to better than
   about 0.05 at trees of this size, which is the size of the margin; and the index's tree is Model T, with flat
   fan-outs, for which x_* is not the relevant quantity (A1.1).

**Verdict.** The structure maps: levels, the floors of A1.2 and A1.2b, regression to the mean, a profile that can
be measured with no truth. The numbers map on glove, and only in rank space and to a factor of two on yahoo and
LAION. What breaks it, by size: the heavy upper tail of the scores; a residual 1.2 to 1.5 times wider than the
model's; μ_c and σ_c correlated within a query on yahoo; a spread that differs by cluster and carries information
the model has no place for (μ + 3σ beats the centre score by 1.3 to 2.2 times at the cluster level); a flat tree of
two branching levels; siblings' independence, untested. The claim of `hierarchy/layered.md` that N^η is "the floor
for any hierarchy keyed by centres and spreads" is supported as a statement about these data's profile (1 − f at
the finest cells is 0.42 to 0.51), not as a theorem: A1.2 and A1.2b are floors in the model, and the data on yahoo
beat the model.

### A1.8 A hypothesis of mine about `model_out.txt`, refuted [refuted]

**Claim.** The synthetic check of `hierarchy/theory.md` with shares (0.6, 0.25, 0.15) reported 13 nodes measured
against 7 predicted at the first level because the formula uses the level z_REM while the realised level is lower.

**Refuted.** Over 200 trees (`scripts/grem_check.py` part B) the mean of the per-tree 99th percentile rank is 6.6
against the formula's 7.1; the realised level is 2.67 against 2.76. The 13 was the 99th percentile of the ranks
pooled over 12 trees, which is set by the worst trees (10 when pooled over 200). The formula is right as an
expectation at that size.

### A1.9 Which levels are worth having, by counts [model, with the measured profiles; the first version withdrawn]

**Withdrawn.** The first version ranked designs by the exponents g_j − f_(j−1) and concluded that leaves of about
125 rows bring the row term down to the centres' term on yahoo and LAION. By A1.4 the exponent does not order the
terms at N = 10⁶: by count the row term is the largest in every design, by 20 to 100 times. The table of exponents
is kept in `out/tables.md` T10 as a statement about the limit only.

**By counts**, at the sizes of the fits, k = 10, recall 0.9. Three figures of the model and the measurement:

- *static*: the least expected rows of a rule that knows every internal value and not τ_k (A1.2b; attainable; it
  pays nothing for the internal nodes);
- *beam*: W of A1.4 with L = 10 and a total miss of 0.1 split equally over the pruning levels, rows and, in
  brackets, internal nodes scored (`referee/ref1_a14_a15.txt` part B; targets above a fixed level);
- *measured*: rows when every query reads the same number of finest cells in the order μ + 3σ (table T4).

| finest cells | yahoo: static, beam, measured | LAION | glove |
|---|---|---|---|
| clusters | 29,600; 28,300 (320); 13,800 | 29,900; 28,000 (500); 29,400 | 67,700; 70,800 (620); 67,500 |
| a16 (about 125 rows) | 12,500; 18,900 (720); 3,400 | 13,900; 19,700 (900); 9,800 | 31,800; 56,600 (1,670); 25,300 |
| a64 (about 31 rows) | 6,100; 19,000 (1,780); 1,350 | 7,900; 21,600 (2,060); 3,900 | 22,100; 63,300 (4,820); 11,100 |
| groups above the clusters | as clusters; 49,400 (130); not measured | as clusters; 50,400 (160) | as clusters; 121,700 (270) |

What the counts say, in the model:

1. **Rows are the count at 1M in every design.** Internal nodes scored are 1 to 9% of the beam's total. Set against
   the static rows they are 1 to 2% at the cluster level and 22 to 29% with 31-row leaves. So at this size a level
   is worth having, by count, only if it lowers the rows.
2. **Finer leaves lower the rows, if the read is one knapsack over the finest cells.** The static figure falls 2.1
   to 2.4 times from the clusters to 125-row leaves and a further 1.4 to 2.1 times to 31-row leaves. Measured, the
   rows fall 2.7 to 4.1 times and a further 2.3 to 2.5 times: more than the model on all three.
3. **A beam with a cut at every level gains from the first refinement only.** Its rows fall 1.25 to 1.5 times with
   a16 and do not fall again with four levels, because the miss budget is split over the levels and each split
   widens every cut. The difference between the beam and the static figure, about 3 times at four levels, is the
   price of deciding level by level instead of once over the leaves (the two figures also differ in their targets:
   a fixed level for the beam, the k best for the static rule). The stop as built decides once.
4. **A level above the clusters is not worth having at 1M.** It can save at most the centres it does not score,
   320 to 620 nodes, under 3% of the total, and as one more cut in a beam it raises the rows 1.7 to 1.8 times.
5. **The upper structure starts to matter, by count, only with the finest leaves**: with 31-row leaves the
   internal nodes a beam scores (1,800 to 4,800) are of the order of the rows measured (1,350 to 11,100), and a
   flat scan of every leaf centre (21,700 to 37,800) would exceed them.

The limit of A1.4 says where this goes as N grows: no design below sup(g − f), 0.16 to 0.27 at the cells measured.
It does not say at what N the centres' term overtakes the rows.

These are counts of nodes in the model. A centre costs n bytes and a first-level row 18.5, 5 to 30 times apart,
and the measured rows are below the model's on every refinement, so a design at a given N is decided by measured
bytes, not by this table.

---

## A2. A stop on expected misses

### A2.1 What the stop guarantees when the tail is known [proved, unrefereed]

**Model and hypotheses.** Model M3 of `theory.md` with one shape G, known to the rule exactly and the same for every
query: given the clusters' statistics, the scores of the clusters read are independent of those of the clusters
unread, and every visible row of cluster c has P(s ≥ t) = G(ζ_c(t)), where μ_c and σ_c in ζ_c are the exact
location and scale of the **visible** rows' scores and v_c their exact count. A read scores every visible row of
the cluster exactly. The order depends only on the statistics, on scores already read and on randomness independent
of the unread scores. At least k rows are visible, and there are no ties at τ_k.

Three things the stop as built does not satisfy: its σ_c comes from 16 directions; under a visible set its μ_c and
σ_c are the whole cluster's, and the shift and scale it estimates (δ_q, ρ_q) are exactly what the hypothesis on
visible statistics assumes away; and the reader of rule 1, which drops rows at a level by a statistical width, makes
misses that X(t̂) does not count.

**Statement.** After each read let t̂ be the running k-th best and M = M(t̂). Then:

1. Miss ≤ X(t̂), always.
2. E[X(t̂) | everything read] = M.
3. For any stopping time σ of the reads at which M < (1 − r)·k, E[recall | everything read up to σ] > r.
4. For any stopping time σ, E[recall | everything read up to σ] ≥ 1 − M_σ/k. If the stop used an estimated tail,
   stopping when M̂ < (1 − r)·k, this is 1 − κ_σ·M̂_σ/k > 1 − κ_σ·(1 − r), with κ_σ = M_σ/M̂_σ the ratio of the two
   sums at the stop.

**Proof.** (1) A true neighbour scores at least τ_k, and τ_k ≥ t̂ because the k rows held are k rows of the visible
set. So every missed neighbour is a visible row in an unread cluster with score at least t̂. (2) Let ℱ_m be the
σ-field of the statistics, the randomness and the scores read in the first m reads. On the event that the unread
set is S, that event and t̂ are functions of the statistics, the randomness and the scores outside S, and the scores
inside S are independent of those given the statistics. So E[X(t̂) | ℱ_m] = Σ over unread c of v_c·G(ζ_c(t̂)) = M:
the sum is linear in the rows, and only each row's marginal law is used. (3) and (4): let B be an event of the
σ-field stopped at σ. For each m, B ∩ {σ = m} is ℱ_m-measurable, so by (1) and (2)
E[Miss_m; B, σ = m] ≤ E[X(t̂_m); B, σ = m] = E[M_m; B, σ = m]. Summing over m gives E[Miss_σ | ℱ_σ] ≤ M_σ, which is
(4), and (3) follows when M_σ < (1 − r)·k. An estimate Ĝ made from the scores read is ℱ_m-measurable and changes
nothing in the argument. ∎

**Weaker hypotheses suffice** (the referee's observations, which the proof bears out). The independence of the
scores inside a cluster is never used. Any stopping time will do, not only the first at which M is small.

**What it bounds, and what it does not.** The expectation is over the model's scores given the statistics, so the
statement is about the queries that share a set of statistics, and it bounds a **mean**. It does not bound the
share of queries below r. In the model itself, with the true tail known and r = 0.9, the mean recall is 0.96
(normal scores) and 0.91 (Student-t, 5 degrees), and 7% and 23% of the queries have recall under 0.9
(`referee/ref1_a2.py` part A; 300 clusters, 1,500 queries a row; the identity of item 2 holds at the stop within
1.5 standard errors). With the heavy tail the bound is nearly attained, so item 3 cannot be improved by much.

**On item 4.** The first version stated it with a κ bounding G/Ĝ at every ζ_c of an unread cluster. That is true and
nearly empty: the worst ratio is at the farthest unread cluster, which carries none of M, and for a normal Ĝ against
a heavier tail it is astronomically large (median 10¹⁹⁷ in the referee's run). The proof needs only the ratio of the
sums, which is weighted by where the mass is: in the same run κ_σ has median 27 and 90th percentile 300. κ_σ is a
random variable when Ĝ is estimated, and a stop that waits for M̂ to dip selects the reads at which Ĝ is low, so κ_σ
at the stop is larger than the ratio at a typical read (measured: A2.5, items 3 and 4).

### A2.2 No shape, no guarantee [proved, unrefereed, for a one-shot decision; the sequential form a sketch]

**Setting.** A one-shot decision with everything but the sample fixed. The clusters' statistics, the set of unread
clusters and its W rows are given. A rule sees n standardised scores drawn independently from the shape, the scores
they standardise, and anything independent of the unread rows, and decides to stop or not. ζ_0 > 0 is such that
ζ_c(t̂) ≥ ζ_0 for every unread c and almost every sample from F_0 below. Model M3 otherwise, as in A2.1.

**Statement.** Let F_0 be any shape with no mass above ζ_0 under which the rule stops with probability s. Then
there is a shape G_1, fixed before any draw, under which the n scores have a law within total variation ¼ of
their law under F_0 and

    E_(G_1)[Miss; the rule stops] ≥ ¾·s·½·min(k, W/(4n)) .

So a rule that stops surely under F_0 and keeps E[Miss] ≤ (1 − r)·k under every shape needs
n ≥ (3/32)·W/((1 − r)·k) whenever W/(4n) ≤ k: for one expected miss, about a tenth as many pooled rows as rows left
unread.

**Proof.** Under F_0 no pooled score exceeds its cluster's μ + ζ_0·σ, so every score held is at most
H = max over read c′ of (μ_c′ + ζ_0·σ_c′), a function of the statistics. Choose ζ* with μ_c + ζ*·σ_c > H for every
unread c; it depends on the statistics only. Let ε = 1/(4n) and G_1 = (1 − ε)·F_0 + ε·δ_(ζ*).

Under G_1 let E be the event that none of the n pooled draws is on the atom. P(E) = (1 − ε)ⁿ ≥ 1 − nε = ¾, which
is also the total-variation bound, and given E the sample is a sample from F_0, so P(stop | E) = s and
P(stop and E) ≥ ¾·s. Let X′ be the number of unread rows on the atom. X′ is Binomial(W, ε), and it is independent
of the sample and of whatever else the rule sees. On E every row held scores at most H, every unread row on the
atom scores above H, and every other unread row is a draw from F_0 and scores at most μ_c + ζ_0·σ_c ≤ t̂. So on E
the true neighbours in unread clusters are exactly min(k, X′): Miss = min(k, X′). Hence

    E_(G_1)[Miss; stop] ≥ P(stop and E)·E min(k, X′) ≥ ¾·s·E min(k, X′) .

X′ has mean m = W/(4n) and variance at most m. E(X′ − m)₊ = ½·E|X′ − m| ≤ ½·√m, so for 1 ≤ m ≤ k,
E min(k, X′) ≥ m − E(X′ − k)₊ ≥ m − ½√m ≥ m/2; for m < 1, E min(k, X′) ≥ P(X′ ≥ 1) ≥ 1 − e^(−m) ≥ m/2; for m > k by
monotonicity in the success probability. So E min(k, X′) ≥ ½·min(k, m). With s = 1 and m ≤ k the bound is
3W/(32n). ∎

The first version chose ζ* after looking at the scores held and called X independent of the stopping event; both
were wrong as written and both are repaired above, as the referee prescribed. The bound has ¾·s where the first
version had s − ¼; the constant 3/32 is unchanged. The referee checked E min(k, X) ≥ ½·min(k, m) exactly for k from
1 to 100 (`referee/ref1_a2.py` part B; the least ratio is 1.26).

**A sequential rule** [sketch: gap named]. In the stop of A2.1, n, the unread set, W and the ζ_c are functions of
the sample. The referee's extension: the runs under F_0 and G_1 agree until the first atom draw, which comes before
the stop with probability at most ε times the mean number of rows pooled at the stop under F_0, so the result holds
with that mean in place of n. I have not written it out, and until it is, calling the statement "the pooled-sample
form of Proposition 8" for a sequential stop is an analogy, not a theorem.

**Standardised shapes** [sketch: gaps named]. In M3 with μ_c and σ_c the mean and standard deviation of a cluster's
scores, a shape has mean 0 and variance 1. G_1 can be standardised only if ε·ζ*² < 1 − ε, that is ζ* < √(4n − 1),
by moving F_0's mean to −ε·ζ*/(1 − ε) and choosing its variance to complete the second moment. The proof needs ζ*
above ζ_max = max over unread c of (H − μ_c)/σ_c. So in the class of standardised shapes the construction is
available when ζ_max < √(4n − 1), which holds with room at the sizes of A2.3 (n from 150 to 9,100, ζ about 4), and
not otherwise. A second gap: standardising replaces F_0 by a shifted and rescaled F_0′, and s is then the stopping
probability under F_0′, or else the total variation between n-samples of F_0 and F_0′ must be added (small at n in
the thousands, not zero).

**What this does and does not rule out.**

- It rules out a rule, fed by the pooled sample, that is valid for **every shape**; and for every standardised
  shape as long as ζ_max < √(4n − 1).
- It does not rule out every distribution-free stop. In the class of standardised shapes there **is** a rule valid
  for every tail: Cantelli gives G(ζ) ≤ 1/(1 + ζ²), so stopping when Σ over unread c of v_c/(1 + ζ_c²) < (1 − r)·k
  gives item 3 of A2.1 for every standardised shape (`theory.md` Proposition 7). It is useless in practice, since
  it needs ζ_c of order √(W/((1 − r)·k)), which is beyond the range ζ_max < √(4n − 1) in which the construction
  above is available whenever n is below the theorem's bound: the two do not conflict.
- What lies between all shapes and a parametric family is **open**. The construction is one atom far above the
  sample, and any class that forbids it escapes the theorem without being a named family: a hazard rate that may
  not fall faster than a stated rate, a log-concave tail, a known upper end of the support. The first version said
  "nothing between"; that was not proved and is withdrawn.
- A guarantee on average over exchangeable queries from a calibration set with truth, as conformal risk control
  gives, is outside the setting altogether.

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

**Model.** In the group's dual coordinates, so that Δ·e = Σ_i Δ_i·e_i, a row's coding error at a level is
e_i = κ·√ν_i·ξ_i. κ is the row's own scale. Given κ the ξ_i are uncorrelated, of mean 0 and variance 1 (so κ is
independent of ξ), and ξ and κ are independent of the query. The ν_i are the same for every row of the group.

**Statement, exact part.** For a fixed offset Δ with w = ‖DΔ‖ > 0, and for every κ,

    ρ_ℓ(q, c)² := E[(Δ·e)² | κ] / E[h² | κ] = Σ_i Δ_i²·ν_i / ( w² · Σ_i a_i ) = E_ω[a] / ‖a‖₁ ,       (A3.1)

where h = w·u is the certified half-width's code term, ω_i = D_i²·Δ_i², and E_ω[a] is the mean of a_i under the
weights ω_i. Then:

1. On the profile (Δ_i² = π_i) it is rule 1's ρ_(g,ℓ) = ‖a‖₂/‖a‖₁. Averaged over offsets with E Δ_i² = π_i,
   E[ρ_ℓ(q, c)²·w²] = ρ_(g,ℓ)²·E w²: it returns chapter 5's (5.6), without (5.6)'s step of taking the mean of h² as a
   product of two means, since here w is fixed.
2. min_i a_i/‖a‖₁ ≤ ρ_ℓ(q, c)² ≤ max_i a_i/‖a‖₁ ≤ 1.
3. ρ_ℓ(q, c)²/ρ_(g,ℓ)² = E_ω[a]/E_a[a] = 1 + Cov_a(a, r)/(E_a[a]·E_a[r]), with r_i = Δ_i²/π_i and the covariance and
   means taken under the weights a_i/‖a‖₁. The factor differs from the profile's only as far as the query's energy
   relative to the profile is correlated with a_i, under those weights.
4. If a_i is the same for every coordinate, ρ_ℓ(q, c) = ρ_(g,ℓ) for every query. Under a reverse water-filling
   **weighted by π_i** (an allocation that minimises Σ ν_i·π_i, or ‖a‖₁, at a bit budget) a_i is constant on the
   coordinates the level codes and smaller on those it leaves uncoded, so the rule changes something only through
   the uncoded coordinates. Under plain reverse water-filling (ν_i constant) this is false: a_i = √(θ·π_i) varies on
   the coded coordinates. The stand-in of A3.3 is the weighted one. Whether `det_alloc_tree` is such an allocation
   is A3.3's caveat.

**Statement, approximate part.** The rule conditions on what the row stores: read a row further when the midpoint
plus min(1, ψ·ρ_ℓ(q, c))·h reaches t̂, taking ρ_ℓ(q, c)·w·u for the standard deviation of Δ·e given u. That replaces
u² by its mean given κ. It is exact given u when the ξ are Gaussian and the a_i equal, for any law of κ. Otherwise
a large u at a fixed κ means large ξ_i² on the coordinates with large a_i, and the formula is off. Sized in the
model by the referee (`referee/ref1_a3.py`, n = 384, the ratio of Σ(Δ·e)² to Σ(ρ·w·u)² within quintiles of u): within
0.97 to 1.02 when a_i is equal or water-filled with 70% coded, for any offset and law of κ; 0.79 to 1.22 from the
lowest to the highest quintile when a_i is log-normal with standard deviation 1, the rows have one scale and the
query's energy sits on the high-a coordinates, in the direction that under-states the spread of the rows with
large u. Over all rows the ratio is 1.00 ± 0.01 in every cell, as the exact part requires. So under an allocation
that makes a_i nearly constant the approximation is good to about 2% in variance.

**Further hypotheses of the rule's use.** "Standard deviation" needs E[Δ·e | u] = 0, which sign-symmetric ξ give and
uncorrelated ξ of mean 0 need not. Using ψ·ρ·h as a normal quantile needs Δ·e near normal: in the referee's runs the
99.9% point of |Δ·e|/(ρh) was 3.1 to 3.35 (normal 3.29), except where the offset's energy sits on few coordinates
(2.8 to 4.0).

**Proof.** E[(Δ·e)² | κ] = κ²·Σ_i Δ_i²·ν_i by uncorrelated coordinates given κ. u² = κ²·Σ_i (ν_i/D_i²)·ξ_i², and
ν_i/D_i² = √(ν_i·π_i) = a_i, so E[u² | κ] = κ²·‖a‖₁ and E[h² | κ] = w²·κ²·‖a‖₁. The ratio is the first form of
(A3.1), free of κ. Writing Δ_i²·ν_i = (D_i²Δ_i²)·(ν_i/D_i²) = ω_i·a_i and w² = Σ_i ω_i gives the second. (1):
ω_i = D_i²π_i = a_i, so E_ω[a] = Σ a_i²/Σ a_i; for the average, E[Σ Δ_i²ν_i] = Σ π_iν_i = ‖a‖₂² and
E w² = Σ D_i²π_i = ‖a‖₁. (2): a weighted mean lies between the least and the greatest. (3): ω_i = a_i·r_i, so
E_ω[a] = E_a[a·r]/E_a[r]. (4), first sentence, from the second form; second sentence: minimising Σ ν_iπ_i at a
budget Σ ½·ln(s_i/ν_i) with ν_i ≤ s_i gives ν_iπ_i constant where ν_i < s_i, and minimising Σ √(ν_iπ_i) gives
√(ν_iπ_i) constant there (the referee's check, which I repeated by the Lagrange condition). ∎

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

---

## Changes after referee 1

Report: `referee/referee-1.md`. The text before it: `out/CLAIMS_before_referee1.md`. No tag was raised. Every
repair the referee prescribed was made; nothing in the report is disputed. Two qualifications of mine are marked.

| claim | what changed |
|---|---|
| Local symbols | The tree is regular by definition. g is non-decreasing, strict over branching levels. Two profiles are named: A_step, Model T's own, and the piecewise-linear A, the binary refinement's. |
| A1.1 | Rewritten as two models that are different processes. Stated what they share (the path law at the depths g_j, expected single-node counts, the hull and x_s) and what they do not (x_*, and the floor of A1.2). Theorem 1.1 is said to be about the refinement only. Hypotheses added: regular tree, integer log₂ fan-outs, independent siblings (a node value taken as the mean of its rows correlates siblings −1/(F − 1); a third at the 4-way splits; untested). The table's rows for the key and the codes corrected. The theorem's "with probability above 1 − ε" and "for N large" restored. Derrida's definition marked [cited, unchecked]. |
| A1.2 | Hypotheses stated: 0 < f < 1, r < 1, the rule's randomness independent of the leaves, recall as a ratio of expectations and what that means. The step for {i ∈ Q} given by the replace-by-a-constant argument. Added that the floor holds for any tree shape. Remark (i) removed. |
| A1.2b (new) | The referee's floor for the k best, with τ_(−i) the k-th best among the other leaves, as a theorem with its proof, which I checked. No concentration argument; the gap named in the first version is closed. Not attained; R(r) is not proved a floor for the k best. The referee's table, my check with leaves sampled (floor/R 1.75 against the referee's 1.73), and the factor on trees with the fits' shares. **Qualification:** in my runs on the fits' trees the floor's column came out above the static optimum in 11 of 18 cells, which cannot be; a diagnostic at 19,200 draws a seed shows it is noise (the floor is 0.95 to 1.00 of the static optimum there), so only the static optimum is quoted from the runs on the fits' trees. |
| A1.3 (not refereed; touched) | Said to be for a fixed level only. Added the factor for the k best and that its near-cancellation with the closed form's own error at the fits' cluster level is an accident. |
| A1.4 | "At most W", with the remark that the beam's own count has the same exponent. Hypotheses of the limit stated (ln L = o(ln M), ε and D fixed, regular tree, f_1 > 0). The positions proved by the referee's induction, with its hypotheses, among them that a level at g explains A(g) whatever the others are (true in the refinement, an assumption for an index). The order of limits stated. A paragraph that the exponent does not order the terms at 10⁶. The boundary case f_i = g_i, which is the data's, added to the caveat. `grem_check.py` part C relabelled as arithmetic on the formula, not a check on trees; the referee's cascade named as the check of W. The two consequences restated with their conditions. |
| A1.5 | Item 2: a cleaner counter-example. Item 3: the wrong identification removed (Model T's good side is on or above the diagonal, the opposite side from the weak regime); the two statements said to answer different questions. Item 4: the inference withdrawn; the number kept with the referee's table showing it is an accident of L = 64. Item 5: its two limits stated. |
| A1.6 (not refereed; touched) | Says the theorem is about the refinement and that Model T has x_* = 0; item 3's figures replaced. |
| A1.7 (not refereed; touched) | Item 5 said to test the closed form's two ingredients given τ_k, not a floor. Item 6: the closed form replaced by the model's own value of the statistic measured (k best random, finite number of clusters); conclusions unchanged in direction, numbers changed. Item 7: the build-rule exponents recomputed with the k best random (0.62 to 0.70 at recall 0.9). Item 8: the margin 0.975 and 0.954 withdrawn; only non-concavity in the limit is claimed. Verdict: siblings' independence listed as untested. |
| A1.9 (not refereed; touched) | The ranking by exponents withdrawn. Re-derived by counts: the referee's W terms, the model's static optimum and the measured rows, per design. **Qualification:** the referee's W splits the miss equally over the levels, which makes deeper designs look no better than one refinement; the static figure shows that in the model finer leaves do lower the rows (2.1 to 2.4 times, then 1.4 to 2.1) when the read is one knapsack over the leaves. Both are in the table. |
| A2.1 | The model named M3 with G known and the same for every query. Hypotheses stated: exact visible statistics, exact reads, the order's randomness, k visible rows, no ties; and the three that the stop as built does not meet. The proof's last step written for events of the stopped σ-field. Said to bound a mean, with the referee's 7% and 23% of queries under the target in the model itself. Item 4 restated with κ_σ = M_σ/M̂_σ; the supremum form called true and nearly empty. Weaker hypotheses noted. |
| A2.2 | The setting stated as a one-shot decision with the unread set fixed. Proof repaired twice: ζ* chosen from the statistics, and the independence argument replaced by the event of no atom draw and the count of unread atom rows; ¾·s for s − ¼; 3/32 unchanged. The sequential form marked a sketch. The standardised case given its condition ζ_max < √(4n − 1) and its second gap. The closing rewritten: a Cantelli stop is valid for every standardised shape; "nothing between" withdrawn; what lies between all shapes and a parametric family is open. |
| A3.1 | The exact statement made the headline: ρ² = E[(Δ·e)² given κ] / E[h² given κ] for a fixed offset. The conditioning on the stored norm stated as the approximate part and sized with the referee's runs. Hypotheses added (uncorrelated given κ, independence from the query, sign symmetry, dual coordinates, near-normality for the quantile). Item 3's weighting stated. Item 4 restricted to a water-filling weighted by π_i. |

**Not changed:** A1.8, A2.3 to A2.8, A3.2 to A3.4, which the referee did not review and no finding touches.

**Numerics run for this revision** (`scripts/topk_model.py`; core 5, one thread, no real data): `check` (leaves
sampled, 98 MB), `fits`, `scaling`, and `scripts/topk_told_diag.py`. The last three peaked at 154 MB, 124 MB and
108 MB resident, for under three minutes each, over the 100 MB the PI set; nothing more is needed and none is
running.
