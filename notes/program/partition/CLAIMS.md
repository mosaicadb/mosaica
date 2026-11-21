# Partition track: claims

The owner's question: LIMINAL groups its rows by spherical k-means; is that the right thing to group by, and what are
the alternatives? Every claim below carries a tag of `notes/README.md` and its derivation. This is the version revised
after referee 1's report (`referee/referee-1.md`); the text the referee saw is `CLAIMS_before_referee.md`, and the last
section, "Changes after the referee", lists what changed in each claim. No tag has been raised.
Sources are given as file and section. The log of how each was reached is `NOTEBOOK.md`. Numbers recalled from the
literature are [cited, unchecked]; the librarian's check is `../library/partition.md` when it exists.

Written in the order P1 (the objective each use implies), P2 (scaling), P3 (the groups), P4 (alternatives), P5 (a
scoring recipe); the experiment is in `EXPERIMENT-SPEC.md`. Section S at the end is the short answer.

## 0. Setting and local symbols

Chapter 0's symbols, with these additions.

- **Partition.** P splits the N rows into K cells. Cell c has n_c rows, share p_c = n_c/N, mean m_c and covariance
  Σ_c = (1/n_c)·Σ_{x∈c} (x − m_c)(x − m_c)ᵀ. S = N/K is the mean number of rows in a cell.
- **Scatter.** T = (1/N)·Σ_x (x − x̄)(x − x̄)ᵀ is the total scatter, W_P = Σ_c p_c Σ_c the within-cell scatter and
  B_P = T − W_P = Σ_c p_c (m_c − x̄)(m_c − x̄)ᵀ the between-cell scatter.
- **Dual spectrum of a covariance.** For Σ ⪰ 0, B(Σ) = Σ_Q^½ Σ Σ_Q^½, γᵢ(Σ) its eigenvalues in decreasing order,
  σᵢ(Σ) = √γᵢ(Σ), F(Σ) = Σᵢ σᵢ(Σ) and M(Σ) = Σ_Q⁻¹ # Σ. The σᵢ(Σ_c) are the singular values of n_c^(−½)·R_c Σ_Q^½, with R_c
  the n_c × n matrix of the cell's residuals.
- **Groups.** A grouping a sends each cell to one of G groups. Group g has N_g rows and pooled covariance
  Σ̄_g = Σ_{c: a(c)=g} (n_c/N_g)·Σ_c.
- **φ.** φ(u) = √u for 0 ≤ u ≤ 1 and φ(u) = 1 + ½ ln u for u ≥ 1.
- **λ** is the price of width in nats per unit of half-width (declared here, as chapter 0 asks). θ = λ⁻² is the water
  level of the floor.
- **What is built** (checked in the source; `NOTEBOOK.md`, entry "what is built"). `rust/tools/fit.py` runs 12 Lloyd
  iterations of spherical k-means on a sample of 200,000 rows (`combined/core.py::kmeans`) and assigns every row to the
  centre of largest inner product. Sizes are not balanced: 84 to 4,635 rows at a mean of 1,000
  (`rust/REPORT_CLUSTERSIZE.md`, "Fits"). The rule is S = 2,000·√(N/1M), rounded to a power of two times 2,000
  (`rust/tools/rules.py`). The stored centre is the mean of the cell at 8 bits a dimension. Cells are grouped by
  k-means on the centres' directions, G = min(64, K, N/(20n)), and each group has one dual basis fitted to its pooled
  residual covariance. Σ_Q is one matrix for the whole index, so two groups' metrics differ only through Σ̄_g.

---

## P1. The objective each use implies

### P1.1 Coding: the cost of a cell at a price [proved, unrefereed; rests on F1, which is proved, unrefereed]

**Proposition P1.1.** Under the model, the message and the certificate of `../floors/CLAIMS.md` N.0, for a cell
whose residual is N(0, Σ), and for every λ > 0,

    H(c) + λ·E h  ≥  D_λ(Σ) := Σᵢ φ(λ² γᵢ(Σ)) = Σ_{σᵢ ≤ 1/λ} λσᵢ + Σ_{σᵢ > 1/λ} (1 + ln(λσᵢ)).

Equivalently, a certificate of mean half-width at most w, 0 < w < F(Σ), needs

    H(c) ≥ H*(w; Σ) := Σᵢ ½ ln⁺(γᵢ/θ),   θ the solution of Σᵢ √min(γᵢ, θ) = w,

and H*(w; Σ) = D_λ(Σ) − λw at λ = θ^(−½).

**Proof.** F1 gives E h ≥ W(H(c)). By Lemma 3 of the floors, W(H) is the least value of Σᵢ σᵢ sᵢ over sᵢ ∈ (0, 1] with
Σᵢ ln(1/sᵢ) ≤ H. So H(c) + λ·E h ≥ inf_s Σᵢ [ln(1/sᵢ) + λσᵢ sᵢ]. Each term is convex in sᵢ with derivative −1/sᵢ + λσᵢ, so
it is least at sᵢ = min(1, 1/(λσᵢ)), where it equals λσᵢ if λσᵢ ≤ 1 and 1 + ln(λσᵢ) otherwise. That is φ(λ²γᵢ). For the
second form, W is non-increasing and strictly decreasing where it is positive, so E h ≤ w forces W(H(c)) ≤ w and
H(c) ≥ W⁻¹(w), which Lemma 3 gives in closed form. With k coordinates above θ, the width is k√θ + Σ_{i>k} σᵢ = w, so at
λ = θ^(−½): D_λ − λw = k + Σ_{i≤k} ½ ln(γᵢ/θ) + Σ_{i>k} σᵢ/√θ − w/√θ = Σ_{i≤k} ½ ln(γᵢ/θ). ∎

Reading. A direction above the water level costs one nat plus the logarithm of its length over the level. A direction
below it costs its length times the price. This is a floor: the measured codes sit 1.17 to 1.6 above it in width
(`../floors/CLAIMS.md` section 7), and real residuals are not Gaussian.

**What the statement assumes and what it is worth** (added after the referee). It is exactly as strong as F1: W is
convex and non-increasing, so the bounds for all λ > 0 together are equivalent to E h ≥ W(H). It holds per cell under
all of F1's hypotheses: the cell's residual is exactly Gaussian with mean zero, the offset is Gaussian with one Σ_Q
and independent of the residual, and the certificate sees the row only through its message. In the index the
offset is q − αm_c and depends on the cell, so one Σ_Q for all cells is a model. H(c) is the entropy of a row's
message given its cell: the cell's label costs no bytes read, which is why sums of D_λ over cells can fall by more
than one bit a row per doubling of K (P2.2). In this section c is the message, as in F1; elsewhere in this file c
is a cell.

### P1.2 Properties of D_λ [proved, unrefereed]

**Proposition P1.2.**
1. φ is increasing, concave and continuously differentiable on (0, ∞), with φ′(u) = 1/(2√u) for u ≤ 1 and 1/(2u) for
   u ≥ 1.
2. D_λ is concave on the positive semidefinite matrices and monotone: Σ ⪯ Σ′ implies D_λ(Σ) ≤ D_λ(Σ′).
3. *Refinement.* Let a cell with covariance Σ be split into sub-cells j with shares π_j, means m_j and covariances Σ_j.
   Then Σ = Σ_j π_j Σ_j + Σ_j π_j (m_j − m)(m_j − m)ᵀ and

       D_λ(Σ)  ≥  D_λ(Σ_j π_j Σ_j)  ≥  Σ_j π_j D_λ(Σ_j).

   The first gap is what the sub-cells' centres remove. The second is what their difference in shape removes.
   These are inequalities between values of a second-moment cost. D_λ is a floor for a cell only when that cell is
   Gaussian, and a parent and its sub-cells cannot all be Gaussian unless nothing was gained. So the gain of a
   split is a gain in the model cost, not in a floor that holds for the same data: for a Gaussian parent the true
   gain is at most the entropy of the sub-cell label (`groups/REPORT.md` Proposition 1), while the model gain has no
   bound (the example of P1.3 gains without limit as w → 0).
4. *The three exponents.* k-means in the query metric minimises Σ_c n_c tr(Σ_Q Σ_c) = Σ_c n_c Σᵢ σᵢ(Σ_c)². As λ → 0,
   D_λ(Σ)/λ → F(Σ) = Σᵢ σᵢ(Σ). As λ → ∞ with B(Σ) non-singular, D_λ(Σ) − n(1 + ln λ) → Σᵢ ln σᵢ(Σ) = ½ ln det B(Σ). So the
   three objectives are the sums of σᵢ², σᵢ and ln σᵢ over the same singular values, and the floor at a price moves from
   the second to the third as the price rises.
5. *Gradient.* Where the γᵢ are positive (they need not be distinct), with B(Σ) = U diag(γ) Uᵀ,
   ∇D_λ(Σ) = Σ_Q^½ U diag(λ²φ′(λ²γᵢ)) Uᵀ Σ_Q^½. For λσ₁ ≤ 1 this is exactly (λ/2)·M(Σ)⁻¹, so ∇F(Σ) = ½M(Σ)⁻¹ and
   tr(M(Σ)⁻¹Σ) = F(Σ).

**Proof.** (1) Both pieces are increasing and concave; at u = 1 the values are 1 and the derivatives ½; the
derivative is decreasing across the join. (2) Σ ↦ B(Σ) is linear and preserves the order. For a concave f and
symmetric A = tX + (1 − t)Y with orthonormal eigenvectors uᵢ, tr f(A) = Σᵢ f(t·uᵢᵀXuᵢ + (1 − t)·uᵢᵀYuᵢ)
≥ t Σᵢ f(uᵢᵀXuᵢ) + (1 − t) Σᵢ f(uᵢᵀYuᵢ), and Σᵢ f(uᵢᵀXuᵢ) ≥ tr f(X) because uᵢᵀXuᵢ is an average of the eigenvalues of X with
weights that sum to one over i as well. For monotonicity, X ⪯ Y gives γᵢ(X) ≤ γᵢ(Y) for every i (Weyl) and f is
increasing. (3) The identity is the law of total covariance. The first inequality is monotonicity, the second
concavity. (4) The limits of φ. (5) The derivative of a spectral function; at λ → 0, λ²φ′(λ²γ) = λ/(2√γ), and
Σ_Q^½ B^(−½) Σ_Q^½ = M⁻¹. ∎

**Consequence: what k-means sees and what it misses** (reworded after the referee). For k-means the cost of a
cell is linear in Σ_c, so the second gap of item 3 is identically zero: it gains only from where the centres are.
The coding cost has both terms, wherever a covariance is formed: for a cell, for the scatter W_g pooled over a
group's cells, for a group.

*What k-means in the M⁻¹ norm is, exactly.* There are three statements, and I mean the second.
1. With one metric M₀ held fixed for all cells, the mean-square zero-read width is tr(M₀Σ_Q)·tr(M₀⁻¹W_P): the k-means
   objective in the M₀⁻¹ norm, with no approximation.
2. With one basis refitted to the partition, the cost is F(W_P). F is concave and homogeneous of degree ½, so
   F(W) ≤ ½F(W₀) + ½tr(M(W₀)⁻¹W) for every W, with equality at W = W₀. K-means in the M(W₀)⁻¹ norm therefore minimises a
   majoriser of F(W_P) that touches it at the current partition. It is a descent step for F(W_P), not only a
   first-order one, and it is a step: the metric then changes. The same holds group by group with each group's
   own metric (P1.4, P3.5). [proved, unrefereed; the inequality checked on 300 random pairs,
   `check_after_referee.py`.]
3. With a basis per cell the same form is a majoriser of Σ_c n_c F(Σ_c) that touches only if every cell has the
   reference covariance, which real cells do not. I do not use this reading.
It is a statement at zero bytes read only. At a rate the gradient is the matrix of item 5, not M⁻¹.

*What the one measurement shows.* `groups/REPORT.md` 3.4 is one such step for reading 2 with one basis for the
whole corpus, from a new random start and with ordinary (not spherical) k-means, so its difference from the base
includes the difference between two random starts. The zero-read quantity moved the right way: undecided by the
group ball alone, 98.2 → 94.7% (text) and 68.0 → 54.9% (items) at the smallest answer. The coordinates read did not
follow: they rose in every column, by under 1% for text (37.7 → 38.0) and by 0.6 to 3.6% for items (2.8 → 2.9 is one
unit in the last printed digit). [measured there.] So the measurement agrees with the theory at zero bytes, and
shows that a gain at zero bytes need not survive into the bytes read. Every scheme below whose proved descent is
in the zero-read width carries that warning.

### P1.3 The partition's coding objective, and when it disagrees with k-means [model]

At a price λ the bits and width of the whole index, in nats, are at least

    J_λ(P, a) = Σ_c n_c D_λ(c | a(c)) + K·c_m + G·c_V,

where c_m is the stored size of a centre and of the rest of a cell's state, c_V = 8n² ln 2 that of a basis, and
D_λ(c | g) is the cost of cell c under group g's basis. Four quantities for D_λ(c | g), in decreasing order when averaged over the cells of a group (not cell by cell,
except the last inequality):

- *An upper bound*: D_λ(Σ̄_g), the cost of the pooled covariance.
- *Group-wide allocation*: the group's scale factors sᵢ^g with the cell's own widths,
  Σᵢ [ln(1/sᵢ^g) + λ·sᵢ^g·√βᵢ^(c|g)], where β^(c|g) is the diagonal of B(Σ_c) in the eigenbasis of B(Σ̄_g).
- *The diagonal model*: the group's basis pair with the best allocation for the cell, Σᵢ φ(λ² βᵢ^(c|g)). It is the
  cost of the family the index uses (weights diagonal in the group's basis, |Δ·e| ≤ ‖DΔ‖·‖D⁻¹e‖) when each
  coordinate's error is scaled by sᵢ at rate ln(1/sᵢ): the least of √(tr D²Q)·√(tr D⁻²S²A) over diagonal D is
  Σᵢ sᵢ √(qᵢaᵢ), and in the group's dual pair qᵢaᵢ = βᵢ^(c|g).
- *Own basis*: D_λ(Σ_c).

Averaged with weights n_c/N_g: own basis ≤ diagonal model ≤ group-wide allocation ≤ D_λ(Σ̄_g). The first is Schur's
inequality (a diagonal is majorised by the eigenvalues and φ is concave) and holds cell by cell. The second is the
choice of the best allocation. The third is Jensen on the square root, because the β^(c|g) average to γ(Σ̄_g) over
the group. [proved, unrefereed, as inequalities between averages; 0 violations in 300 random groups, and 428 of
1,047 cells with a diagonal-model cost above D_λ(Σ̄_g), `check_after_referee.py`. That the built code behaves as one
of the middle two is the model.] The version the referee saw called D_λ(Σ̄_g) the cost of group-wide allocation; it
is an upper bound on it.

**k-means' objective.** The built fit ascends Σ_c n_c‖m_c‖, the sum of the keys (spherical k-means; P1.6 says how
far it gets). For unit rows the trace objective is J_km(P) = Σ_c n_c tr Σ_c = N − Σ_c n_c‖m_c‖². The two rank
partitions alike only when the ‖m_c‖ are nearly equal. "k-means" below means the trace, with Σ_Q = I in P1.2 item 4.

**Exactly when the two disagree** (at a fixed price λ; the example after it is at a fixed width w, and the two
can order a pair of cells differently). Take two candidate cells with dual spectra γ and γ′. k-means prefers the one
with the smaller Σᵢ γᵢ. Coding at price λ prefers the one with the smaller Σᵢ φ(λ²γᵢ). Since φ is concave they disagree
when one cell is *flat* (few directions above the level θ = λ⁻², however long) and the other is *small* (less total
variance, spread over more directions above the level). For k directions of variance a against k′ directions of
variance b, all above θ: k-means prefers the second when k′b < ka; coding prefers the first when
k·(1 + ½ ln(a/θ)) < k′·(1 + ½ ln(b/θ)). The count of directions above the level comes first and their length enters
by its logarithm.

**A two-cluster example, by hand** [model: cells coded by their second moments, as the index codes them; Σ_Q = I].
In the plane put half the rows uniformly on the segment {(s, +h) : |s| ≤ a} and half on {(s, −h) : |s| ≤ a}.

| partition into two cells | covariance of each cell | k-means cost per row (trace) |
|---|---|---|
| A, by segment | diag(a²/3, 0) | a²/3 |
| B, by the sign of s | diag(a²/12, h²) | a²/12 + h² |

k-means takes B whenever h < a/2. For coding at half-width w (both cells of a partition are alike, so the cost per
row is H*(w) of one cell):

- A has one direction: √θ = w and H*_A = ½ ln(a²/(3w²)), for w < a/√3.
- B with both directions above the level (w ≤ 2h, and h² ≤ a²/12): 2√θ = w and
  H*_B = ½ ln(a²/(3w²)) + ½ ln(4h²/w²) = H*_A + ln(2h/w).
- B with one direction above the level (w ≥ 2h): √θ + h = w and H*_B = ½ ln(a²/(12(w − h)²)) = H*_A − ln(2(w − h)/w).

So, for h² ≤ a²/12 and w below both zero-read widths (above them both costs are zero and the two tie), **coding
prefers A exactly when w < 2h, and k-means prefers B exactly when h < a/2**. For a/(2√3) < h < a/2 the conclusion
also holds, by another computation: there h is B's larger direction and F_A < F_B, so A is better even with nothing
read. So for w/2 < h < a/2 the two disagree, whatever a is. With a = 1 and h = 0.1 the traces are 0.333 (A) and 0.093 (B), so k-means prefers B by a
factor 3.6. At w = 0.05, H*_A = ½ ln 133.3 = 2.45 nats and H*_B = 2.45 + ln 4 = 3.83 nats: A saves 2.00 bits a row.
At w = 0.3, H*_A = 0.65 nats and H*_B = ½ ln(0.0833/0.04) = 0.37 nats: B saves 0.42 bits. The crossing is at the
width where the gap between the two segments falls below the water level: a cell needs to be flat only down to
the width the query asks for. Checked numerically (`check_p1.py`, log `check_p1.log`).

One caution, and one correction. The caution: the cells of B are far from Gaussian in the second coordinate (two
atoms), so a code that spends one more bit on the sign of that coordinate turns B into the four-cell refinement of
both; the example is about the second-moment code the index uses. The correction: the version the referee saw
said here that the example "describes cells against groups, not rows against cells", because the index codes
with the group's basis and not the cell's. That was wrong. The two cells of either partition have the same
covariance, so a group that holds both has that covariance as its pooled covariance and one basis fits both
exactly. Choosing A or B is a choice of which rows go to which cell inside one group with one basis, and it
changes the cost by the amounts computed. See P1.4.

### P1.4 The row-to-cell step under the built architecture [model; case 1 was wrong as the referee saw it, and is restated]

With one basis and one bit allocation per group, a row's half-width at a level is ‖D_gΔ‖·‖D_g⁻¹e‖, with e the coding
error of its residual r = x − m_c. The group's basis is fitted to W_g, the within-cell scatter pooled over the group's
cells, and W_g moves when rows move. That is the step the earlier version missed.

1. *Between two cells of one group.* For a group metric held fixed, the cheapest cell for a row is the nearest
   centre in that metric. The metric is then refitted, and the alternation descends the group's zero-read cost
   N_g·F(W_g)² (P3.5 with the rows confined to their group). That cost is a concave function of W_g, where k-means'
   is linear in it. "Nearest centre in the group's own dual norm" is the fixed-point condition of the alternation,
   and several partitions satisfy it at different costs.

   *Counter-example to the earlier wording* (the referee's; reproduced in `check_after_referee.py`). Take the two
   segments of P1.3 as one group with one basis, a = 1, Σ_Q = I. Partitions A and B are both fixed points (no row of
   A and, in a sample of 20,000, 7 to 17 rows of B beside s = 0 have a nearer centre in the group's own norm). At
   h = 0.35 the zero-read cost F(W_g) is 0.577 for A and 0.639 for B, while the trace is 0.333 for A and 0.206 for B:
   moving rows within the group, against k-means, lowers the cost with nothing read by 10%. At h = 0.1 and
   w = 0.05 the same move saves 2.00 bits a row.

   So what holds is this. **Cells that are flat along the same directions do lower a group's cost, because they
   lower the nuclear norm of its pooled scatter. What a group cannot use is a difference of shape between its
   cells.** In terms of the spectrum: k-means gains 2σᵢ·dσᵢ from shrinking direction i and so spends its cells on the
   directions of largest spread; the zero-read cost gains dσᵢ from every direction alike, and at a level a direction
   above the water gains by its ratio (P1.1). The earlier version said that the within-group rule is k-means in a
   metric and that flat cells cannot lower the cost within a group. Both statements were wrong.
2. *Between cells of two groups.* The row changes basis. Its cost in cell c is
   ω(x | c)² = tr(M_g Σ_Q)·‖x − m_c‖²_{M_g⁻¹} with g = a(c) at zero bytes read, and the two norms differ. A row can be
   cheaper in a farther cell whose group's basis fits the direction of its residual. This is the rule of Archer and
   Leen's coding-optimal partition (each vector to the region whose transform coder distorts it least) with the
   certified width in place of the squared error. It is treated in P3.5, which contains case 1.

Two hypotheses in the first paragraph, left unstated before. The code scales a row by its Euclidean residual norm
(`SYSTEM.md` 2.1), and the error of the scaled code depends on the direction of the residual in the group's
coordinates, which changes when the row changes cell; "the row's residual length in a norm fixed by the group"
holds only for a quantiser whose error norm does not depend on direction. And the norm is fixed by the group and
the level: M_g⁻¹ with nothing read, another quadratic form at each level (P3.5). A row is assigned once and read at
several levels, so its cheapest cell is not defined until the levels are weighted.

**What has been measured, exactly.** `groups/REPORT.md` 3.4 (`groups/scripts/prep_m.py`, read by me and by the
referee) is one step of case 1 with one basis for the whole corpus: the M⁻¹ of the covariance pooled over all cells,
12 passes of ordinary k-means in that norm from a new random start, then the centres grouped by direction as
before. It does not test each group's own norm, it does not iterate, and it does not test case 2. Its result is in
P1.2: the zero-read quantity improved and the coordinates read did not.

So the coding cost can respond to (i) the number of cells K (P2), (ii) which rows share a cell inside a group,
through the shape of the group's scatter (this section and P3.5), (iii) which cells share a basis (P3.1 to P3.4),
(iv) which basis a row is coded in (P3.5), and (v) state added per cell (P4.3).

### P1.5 The cut: a net, then an extent along the anchors [proved as an identity; the Gaussian estimate is a model]

The query's nearest anchor is p̂, β = q·p̂ and ρ = √(1 − β²). The build stores e_c⁺(a) = max_{x∈c} a·x and
e_c⁻(a) = min_{x∈c} a·x for every anchor a and cell c (`SYSTEM.md` 2.1 and 2.2). Since every row has norm at most ν
(1 up to rounding), every score in cell c lies in [β e_c⁻(p̂) − ρν, β e_c⁺(p̂) + ρν]. The rows the cut leaves are

    R_cut(q, t) = Σ_c n_c·1[ β e_c⁻(p̂) − ρν < t ≤ β e_c⁺(p̂) + ρν ].

A cell is undecided over a window of thresholds of length β·ext_c(p̂) + 2ρν, where ext_c(a) = e_c⁺(a) − e_c⁻(a) is the
width of the cell's convex hull along a. (For β ≥ 0; for β < 0 the two extremes swap.) Of the two terms the second is
the larger by far for any query that is not close to an anchor: 2ρν is of order 1 at ρ = 0.5, while β·ext is a few
standard deviations of a cell along one direction, and the second term does not depend on the cells' shapes. So the cut implies two things about a partition, the first much the larger, and neither is a
variance:

1. *Its centres as a net for the queries*: ρ is the sine of the angle from the query to the nearest centre. It
   depends on K and on where queries fall, not on the cells' shapes. For out-of-distribution queries no centre is
   near and the cut decides nothing (`sublinear/REPORT.md` 5.2: every centre read at every N).
2. *The extent of each cell along the other centres' directions*: Σ_c n_c·ext_c(a), averaged over the anchors queries
   choose. It is set by the most extreme row of the cell in that direction. For small answers only the upper
   extreme e_c⁺ matters.

The shape rewarded is a cell with a small width along the directions in which centres lie (the directions along
the data, where the anchors are), with no outlying row; its thickness in directions no anchor points along costs
nothing. Under a Gaussian model of the projection, ext_c(a) ≈ 2·z(n_c)·√(aᵀΣ_c a) with z(n) the expected largest of n
standard normals (3.24, 3.43, 3.62, 3.96 at n = 1,000, 2,000, 4,000, 16,000) [model]. Three differences from the
trace follow: the cut adds standard deviations and not variances; a larger cell has farther extremes at the same
covariance (about 5% per doubling, plus the growth of the covariance itself); and a tail heavier than Gaussian,
which the scores have (`sublinear/REPORT.md` 5.4), costs more than its variance says.

### P1.6 The sorted range: a tail count of the quantity whose mean spherical k-means ascends [proved as an identity]

Write θ_x for the angle between a row and its cell's centre direction m̂, φ_c for the angle between q and m̂, and
τ = arccos t. From the key x·m̂ = cos θ_x alone the tightest interval for a unit row is
[cos(φ_c + θ_x), cos(φ_c − θ_x)] (`groups/REPORT.md` 2.1). A row is undecided when |φ_c − τ| < θ_x ≤ φ_c + τ, a contiguous
range of keys, and

    R_range(q, t) = Σ_c #{x ∈ c : |φ_c − τ| < θ_x ≤ φ_c + τ}.

(The lower end is clamped at −1 when φ_c + θ_x > π.) For a cell farther from the query than the threshold angle
(φ_c > τ, the common case at small answers) the rows left are those with φ_c − τ ≤ θ_x ≤ φ_c + τ; the upper condition
can fail only for rows farther from their centre than the query is, so in practice they are the rows of the cell
with the *lowest keys*. Three consequences.

1. Spherical k-means ascends Σ_x x·m̂_c(x), the sum of the keys, and the sorted range counts the keys below a level
   set by the query. So this is the use whose quantity the built objective addresses, up to the difference between
   a mean and a tail count, **and up to how far the built fit is from a maximum, which is not known.**

   *The fit, exactly* (`rust/tools/fit.py` lines 204 to 223, `combined/core.py::kmeans`; restated after the referee).
   (i) 12 Lloyd passes of spherical k-means from random rows, on a sample of min(N, 200,000) rows, with emptied
   centres reseeded at random in each pass; not run to convergence, and Lloyd's fixed points are local. (ii) Every
   row to the sample centroid of largest inner product; centroids that receive no row are dropped. (iii) The
   stored centre is the mean of the resulting cell, rounded to 8 bits a dimension, not the centroid that made the
   assignment; the key is x·m̂ for the direction of that stored mean, in 16 bits. For the final partition the
   stored direction is the best one (the direction of the mean maximises Σ x·m̂). But the partition is not the best
   one for the stored directions: a row need not be at the stored centre of largest key. So the built partition is
   one assignment step and one centre step past a Lloyd iterate on a sample. It is neither a fixed point nor a
   maximum. On the referee's synthetic toy made the same way (60,000 unit rows, n = 48, K = 60, 200 sample rows a
   centre; `referee/ref1_p14.py`) 11.7% of rows did not have their largest key at their own stored centre, and
   continuing Lloyd on all rows raised the sum of the keys by 4.7%. The size on the real corpus is not known; it
   is the experiment's first item. (The Rust `create` path, `rust/src/partition.rs`, was not read by me or by the
   referee.)
2. Only the radial law of a cell enters: the bound takes the worst alignment of the residual with the query, so
   the cell is treated as a cap about its centre. Its shape beyond that is not seen, and thickness in every ambient
   direction counts, including directions no query points along. (Sorting by ‖r‖ in the M⁻¹ norm, as
   `groups/REPORT.md` 3.3 proposes, replaces the ambient radius by the dual one; the objective is then the tail of
   the M⁻¹ radius.)
3. The radius concentrates (the logarithm of the dual norm has standard deviation about 0.08,
   `../floors/CLAIMS.md` D1), so a cell is decided nearly as a whole, when φ_c − τ exceeds its radius. What a
   partition can add beyond a smaller radius is fewer stragglers: rows whose key is far below their cell's typical
   key.

### P1.7 The approximate order: k-means in the queries' second-moment metric [proved as an identity; its use is a model]

For a query q the share of the score variance over rows that the cells explain is f(q) = qᵀB_P q / qᵀT q, by the law
of total variance. Averaged over queries with second-moment matrix Σ_qq = E qqᵀ, the explained variance is
tr(B_P Σ_qq) and what is left is

    tr(W_P Σ_qq) = E_q Σ_c p_c σ_c(q)²,

the k-means objective for the rows mapped by Σ_qq^½. (The identity is for the average of the explained variance qᵀB_P q. The
average of the share f(q), which P5's score S4a reports, weights queries by 1/qᵀTq and is a different average; on
the referee's toy the two were 0.1523 and 0.1527.) Plain k-means is the case Σ_qq ∝ I. For queries drawn like the
data, Σ_qq = T + x̄x̄ᵀ, which weights the directions in which the data vary most: the opposite of whitening. The
order μ_c + ψσ_c(q) is governed by f (`hierarchy/theory.md` 2.3, the beam law) [model there], so for this use the
trace is the right kind of objective and the metric is the question. Two reservations: the neighbours live in the
upper tail of a cell's scores, which a variance does not describe (A1.7 items 2 and 4), and the profile already
lies on the diagonal f = g at the clusters (A1.7 item 1).

### P1.8 Layout: opened cells and their fixed costs [model, from measured unit costs]

A query pays a fixed cost per cell it meets (3 µs for the cut, 8 to 17 µs for an opened cell of 1,000 rows,
`rust/REPORT_CLUSTERSIZE.md`, "Derived costs") and a cost per visible row read. For a visible set V the layout cost
is about

    Σ_c 1[V meets c]·c_cut + Σ_{c opened} (c_open + |V ∩ c|·c_row·levels) + (rows of cells too thin to open)·c_full.

A partition lowers it in two ways. *Balance*: a cell with fewer visible rows than about c_open/c_row (several
hundred) never repays its setup and is read from full vectors at about five to ten times the cost per row, so a long
lower tail of sizes is a loss and nothing is gained by the upper tail. *Purity*: if cells are pure for the attributes
filters are made of, a visible set meets few cells and fills the ones it meets (measured for taxonomic slices,
`sublinear/REPORT.md` 5.5: 10, 44, 144 and 375 of 999 cells for slices of 977 to 250,000 rows). Purity is the mutual
information between the cell and the filter's attribute; for a filter independent of the vectors no partition has
any.

### P1.9 The five objectives side by side

| use | what the partition should make small | kind | does k-means minimise it? |
|---|---|---|---|
| cut | first the queries' distance to the nearest centre (the 2ρν term, a property of K and of the queries); then Σ_c n_c·ext_c(a) over anchors a | a net; then an extent (largest row) | no: the net does not depend on the cells' shapes, and the extent is a width of the hull |
| sorted range | Σ_c #{rows with key below the query's level} | tail count of the radius | it ascends the mean of the same quantity; the built fit stops short of a fixed point (P1.6) |
| coding | Σ_c n_c Σᵢ φ(λ²·βᵢ^(c|g)) plus centres and bases; within a group at zero read, N_g·F(W_g)² | concave spectral function, between Σσᵢ and Σ ln σᵢ | no, at any step: rows within a group (concave in the group's scatter; nearest centre in the group's norm is only a fixed-point condition, P1.4), cells to bases and rows to bases (P3) |
| approximate order | tr(W_P Σ_qq) | variance, in the queries' metric | yes for isotropic queries |
| layout | cells met, cells too thin to open | counts | no: no balance, no purity |

---

## P2. Scaling with the number of cells

### P2.1 The model, and what it is fitted to [model; the exponents measured elsewhere]

**Model U** has three assumptions, each of which the experiment's sweep tests or leaves open.
(a) Refining the partition scales every direction of the dual spectrum alike: γᵢ(Σ_c) ∝ K^(−e) for all i, with
e = 2/D. What has been measured is the Euclidean trace only; no measurement cited says how the dual spectrum's
shape changes with K. (The alternative, model M, a flat of dimension D whose directions shrink and a remainder
that does not, is the classical high-resolution picture.) My reason for U over M is an argument: a cell of 1,000
rows in 19 dimensions is 1,000^(1/19) = 1.4 neighbour spacings across, so it is not a flat patch at this scale.
(b) Rows are drawn independently from one law, so a cell's covariance depends on K and not on N.
(c) The exponent measured at K ≥ 1,000 holds at the cell counts where it is used (K = 250 to 1,000 below).
In P2.2's table a fourth is added: (d) a group's spectrum stands for a cell's.

What was measured on BioCLIP-2 [measured, `groups/REPORT.md` 3.2 and `sublinear/REPORT.md` 5.1; exponents computed
here and recomputed by the referee]. Varying K at fixed N = 1M, with the rows per cell falling from 1,000 to 17: the
mean square residual falls as K^(−0.106) from K = 1,000 to 58,750 (rms 0.3765 to 0.3034), D = 19, the value three
neighbour-count estimators give (16 to 22, `sublinear/REPORT.md` 5.4). Varying N at a fixed 1,000 rows a cell
(K = 31 to 1,000, rms 0.507 to 0.377): K^(−0.17), D = 12. **These are two different experiments.** The earlier
version read the difference as the exponent falling as K grows. It is equally a difference between shrinking
cells and growing corpora, which is assumption (b) failing, and the data cited cannot tell the two apart. The
residual has n_eff = 72 to 76 in the dual metric (`../floors/CLAIMS.md` section 7): D counts how the residual
scales, n_eff how many directions it occupies. None of this is Zador's limit (section L).

### P2.2 What a doubling of K is worth to a second-moment code [an identity for the cost H*, proved, unrefereed; its use is model U; it is not a floor; numbers computed from the floors' table]

**Proposition P2.2.** If every γᵢ of a cell's covariance is scaled by 2^(−e), then for every target half-width w

    H*(w; Σ_c scaled) = H*(2^(e/2)·w; Σ_c),

and to first order the fall is (e/2)·λw bits a row, where λ = θ^(−½) is the price at w and
λw = k_a + Σ_{σᵢ<√θ} σᵢ/√θ, with k_a the number of directions above the level, so k_a ≤ λw ≤ n. The first-order figure is
an upper bound on the exact fall (0.86 to 1.00 of it on the referee's random spectra, median 0.995), because λw
falls as w grows.

**Proof.** Scaling every γᵢ by c scales every σᵢ by √c, and the programme of Lemma 3 at spectrum cγ and width w is
the programme at γ and width w/√c. By P1.1, ∂H*/∂w = −λ, so ∂H*/∂ ln c = ½λw. Put c = 2^(−e). ∎

**What it is.** Under model U: *the second-moment cost of reaching a width falls, per doubling of K, by what a
target width 2^(1/D) larger saves, 3.7% at D = 19.* It is an identity about the function H*. H* is a floor for a
code only when the cell is Gaussian, and model U cannot hold with Gaussian cells at both K and 2K: the Gaussian
entropy of a cell would fall by its rank over D bits per doubling (40 bits at full rank), and a label of one bit
cannot remove more than one bit of entropy from a density. So this is a prediction for a code that charges second
moments, as the index's does, and the excess over one bit is negentropy that the partition removes
(`groups/REPORT.md` Proposition 1 and F5 bound the gain against the best code for the true law at one bit).
`NOTEBOOK.md` records that I first had this wrong the other way.

In bytes, from the floor W on the spectrum of group 12 of an earlier fit (`../floors/CLAIMS.md` section 7: one
group of 49,674 rows, diagonal Σ_Q in the residual basis, 48 bits of stored scalars charged; λw is the slope of
nats against ln W between rows of that table) [computed; assumption (d)]:

| depth of the read | λw | fall per doubling of K, per row read to that depth |
|---|---|---|
| 0 to 4 B | 124 | 6.6 bits, 0.8 B |
| 4 to 8 B | 150 | 8 bits, 1.0 B |
| 8 to 16 B | 187 | 10 bits, 1.2 B |
| 16 to 32 B | 246 | 13 bits, 1.6 B |
| 32 to 96 B | 365 | 19 bits, 2.4 B |

It cannot exceed (e/2)·n = n/D = 40.4 bits, 5.05 B, at n = 768.

**Against the measurements** [measured elsewhere; the comparison is mine, corrected after the referee]. The
agreement is in order of magnitude and no better.
- `rust/REPORT_CLUSTERSIZE.md`, 1,000 against 4,000 rows a cell (two doublings, 64 groups in both). Text 1%: bytes per
  item go 67.9 → 71.6, but the items entering the levels go 955k → 979k (the cut and the range prune less), so per
  item entering the bytes go 71.0 → 73.1: **1.0 B per doubling**, inside the model's range over depths (0.8 to 2.4)
  and below the 1.6 to 2.4 of that depth. The earlier version set 1.85 B against 1.6 to 2.4, which mixed in the
  pruning. Text 50%: of the 8.5 B rise, 5.4 B is the rise in the share of full vectors (11.57 → 11.92% of items at
  1,536 B), which are not code at a rate the cost describes; the comparison with the cap of 5.05 B is withdrawn.
  Text 0.01% falls, 19.1 → 17.3 B, because the planner sends fewer cells to full vectors (P1.8).
- `groups/REPORT.md` 3.2, exact coordinates read by a text query as K goes 1,000 → 3,997 → 15,880 → 58,750: 2 to 6% per
  doubling at the 0.01% answer and 0.4 to 2% at 5%. In bytes of the native read (19 and about 220 B) that is 0.4 to
  1.1 B and 0.9 to 4.4 B per doubling; the model gives about 1.2 and at most 5.05. The currencies differ (exact
  coordinates against floor bits).

### P2.3 The cell size for the exact path [model; unit costs measured in `rust/REPORT_CLUSTERSIZE.md`; the S* figures are good to a factor of three]

Let a query read a share u of the rows to a depth where a doubling of K saves s_B bytes a row (P2.2), and let a
cell cost the query c_K byte-equivalents. Assumed throughout: u = 1 in the figures; 2 ns a byte; the per-row work
linear in log₂S (it is, to the digits given).

**By bytes.** The cost per row is c_K/S − u·s_B·log₂K + const, least at

    S* = c_K·ln 2 / (u·s_B).

With only the centre charged (c_K = n = 768 B) and u = 1: S* = 220 to 670 rows. With the support table's row
(2K = 1,000 B at K = 500) 2.3 times that, 510 to 1,530. So by bytes cells should be several hundred rows for a query
that reads every row, as text queries nearly do.

**By time.** A cell costs 3 µs in the cut and 8 to 17 µs when opened (at 1,000 rows; 13 to 31 µs at 4,000 and 25 to
61 at 16,000, same report); a row-level costs 10 to 12 ns for 4 to 8 bytes, about 2 ns a byte. So at 1,000 rows c_K
is 5,500 to 10,000 byte-equivalents, 7 to 13 centres, and the formula gives S* = 1,600 to 8,700 rows at u = 1.

The same from the profiled phases (which that report says run 1.3 to 1.4 times the unprofiled wall). With a the
per-cell milliseconds and b the per-row milliseconds at S₀ = 1,000, ε the measured growth of the per-row work per
doubling of S, and the per-cell work falling as S^(−p), the minimum of a·(S/S₀)^(−p) + b·(1 + ε·log₂(S/S₀)) is at
S* = S₀·(p·a·ln 2/(b·ε))^(1/p). The earlier version took p = 1. The table it was fitted to has p = 0.5 to 0.6, because an
opened cell costs more as it grows:

| query | a, b (ms) | per-cell ms at 1,000 / 4,000 / 16,000 rows; p | ε per doubling of S | S* at p = 1 | S* at the measured p | measured per-cell + per-row ms at 1,000 / 4,000 / 16,000 |
|---|---|---|---|---|---|---|
| text 0.01% | 10.3, 20.1 | 10.3 / 4.1 / 1.9; 0.61 | 0.060 (20.1 → 22.5 → 24.9) | 5,950 | 8,300 | 30.4 / 26.6 / 26.8 |
| text 1% | 16.4, 55.8 | 16.4 / 6.9 / 3.4; 0.57 | 0.012 (55.8 → 57.2 → 58.5) | 16,800 | 53,000 | 72.2 / 64.1 / 61.9 |
| item 1% | 5.7, 9.6 | 5.7 / 2.8 / 1.5; 0.48 | 0.16 (9.6 → 12.6 → 15.8), groups rising too | 2,550 | 1,500 | 15.3 / 15.4 / 17.3 |

[computed from that report's breakdown table; `check_after_referee.py`. The earlier version had ε = 0.055 and
S* = 6,500 for text 0.01%; ε is 0.060.] The measured totals are flat near the optimum, and the two columns of S*
differ by up to a factor of three, so **the S* figures are good to a factor of three and no better.** What
survives that: text queries want larger cells than selective item queries, 4,000 to 16,000 or more against about
2,000; **the built cell size is set by the per-cell time, not by the centre's bytes, and by the selective
in-distribution query, whose per-row work grows fastest with S because the cut and the sorted range prune less
(P1.5, P1.6).** The code alone accounts for ε of 1 to 3% (s_B over the bytes of an item that enters the levels);
the rest of the item query's 16% is the cut.

**With N.** If the per-cell work grows as K = N/S and the per-row work as N^η, with η the measured exponent of the
read (0.79 to 0.87 for text and 0.49 to 0.60 for items, coordinates read at 12 bases, `sublinear/REPORT.md` 5.3,
measured at a fixed cell size), the optimum moves as

    S* ∝ N^(1 − η):   N^(0.13 to 0.21) for text,   N^(0.40 to 0.51) for items.

The built rule is S ∝ N^(0.5): the item queries' exponent. This inherits the factor of three and adds two
assumptions: that the per-cell work is proportional to K at every N (with the measured p it is not proportional
to 1/S at fixed N), and that η does not depend on S. So the exponent's agreement with the rule for item queries is
the result, and I withdraw the earlier figures for where the text optimum and the rule meet. [model; the exponents
are measured over 1.5 decades of N.]

What would move it: a per-cell cost that does not grow with K (a level over the cells for the cut) would make
smaller cells pay; each halving of S is worth 0.8 to 2.4 B a row read by the second-moment cost.

### P2.4 The leaf size for the approximate path [model]

At a fixed N the rows read to a recall fall as cells^(−γ), so with leaves of S rows a query reads R₀·(S/S₀)^γ rows at b
bytes each and prices ξ leaves for each one it reads, at c_leaf bytes of state per leaf. The bytes
b·R₀·(S/S₀)^γ + ξ·c_leaf·(R₀/S₀)·(S/S₀)^(γ−1) are least at

    S*_leaf = ((1 − γ)/γ)·ξ·c_leaf / b,

which does not depend on N. With γ = 0.43 to 0.56 (the order μ + 3σ, A1.7 item 7), c_leaf = 17n + 136 bytes (a centre
and 16 directions, `hierarchy/layered.md` 8.2), b = 18.5 B (the first read of a row there) and ξ = 1 to 3:

| n | c_leaf | S*_leaf |
|---|---|---|
| 128 | 2,312 B | 100 to 500 rows |
| 512 | 8,840 B | 380 to 1,900 |
| 768 | 13,192 B | 560 to 2,850 |

So the fixed leaf of 2,000 rows (`hierarchy/layered.md` 8.7) is what this gives at n = 512 to 768, and the reason it is
as large as that is that a row's first read is 18.5 B: with rows read as full vectors (b = 2n) the same formula
gives 5 to 40 rows. At n = 128 it gives leaves four to twenty times smaller than 2,000. The per-leaf tables (6 kB,
same source) are left out and would raise each figure by a factor of about 1.5 to 3.5. [model; nothing here is
measured by me.]

**Variance explained against K** [computed from A1.7 item 1's measured profile]. Below the clusters the unexplained
share 1 − f falls as cells^(−e_q): yahoo 0.58 → 0.52 → 0.47 → 0.42 over three 4-way splits, e_q = 0.077; LAION
0.55 → 0.49 → 0.45 → 0.41, e_q = 0.071; glove 0.64 → 0.59 → 0.55 → 0.51, e_q = 0.055. As 2/D these are D = 26, 28 and 36
in the queries' metric. A doubling of the cells raises f by about 0.03, which is the whole effect a finer k-means
has on the order.

### P2.5 The groups, and the fixed costs side by side [computed from the rules; measured rules cited]

Bytes per row of fixed state at 1M, n = 768, S = 2,000, G = 64 (the built rules):

| state | size | bytes per row |
|---|---|---|
| centres | n·K | 0.38 |
| support table | 2 bytes × K anchors × K cells | 0.5 (and 2N/S², so constant in N under S ∝ √N until the anchors are capped) |
| cell directions for the approximate spread | 16n·K | 6.1 |
| bases, G = 64 | n²·G | **37.7** |
| bases at one per κn² rows, κ = 1 | n²·N/(κn²) | 1.0 |
| deterministic codes and key | | 34 |

The bases are the largest fixed cost by a factor of six, and larger than the deterministic codes. Rows per basis
are N/G = 15,600 = 0.026·n², against the κ of 0.5 to 1.1 that counts and timings on yahoo and LAION support
(`/home/joe/code/vexact-visitorder/bench/suite/notes/visit-order/hierarchy/layered.md` 8.8 to 8.10, cited below as
`hierarchy/layered.md`; it is not in this worktree and the referee could not open it).

**The number of bases is a trade between bytes and time, not a direction** (corrected after the referee). The
earlier version said the theory "sides with fewer bases", as "the direction of every measurement cited". On this
corpus the measurement says otherwise for bytes. `rust/REPORT_CLUSTERSIZE.md` has fits with the same cells and
different groupings: from 64 groups to 31 at 4,000 rows a cell the bytes per item rose 6.8% (text 1%), 5.2% (text
50%) and 3.2% (items 50%); from 62 groups, a basis per cell, to 7 at 16,000 rows a cell they rose 36%, 29% and 20%
for text at 0.01%, 1% and 50% and 19 to 22% for items, which is 6 to 10% per halving of G. With the full vectors
taken out the code bytes alone rose 2 to 10% per halving in the first pair and 3 to 13% in the second
(`check_after_referee.py`, from that report's byte and full-vector columns) [measured there; the split is my
arithmetic]. Its summary: fewer groups are faster at small answers and read 5 to 27% more bytes. So fewer bases
cost bytes read and save setup time and storage (37.7 B a row here), and the slope on BioCLIP-2 is about twice the
2.2 to 3.7% per doubling that `hierarchy/layered.md` 8.8 measured on yahoo and LAION and that the earlier version
used. In the terms of P1.3 the bytes are the Jensen gap of the group and the time is n² per group opened.

---

## P3. The groups: assigning cells, and rows, to a basis by coding cost

### P3.1 The cell-level scheme and what it minimises [proved, unrefereed; rests on `groups/REPORT.md` Proposition 3 and on B4b's sum rule, both proved, unrefereed]

For a metric M ≻ 0 and a cell c let ω_c(M)² = tr(MΣ_Q)·tr(M⁻¹Σ_c), the mean-square zero-read half-width of the cell's
rows under the M-norm certificate. For a grouping a and metrics M₁, …, M_G put Ψ(a, M) = Σ_c n_c ω_c(M_a(c))².
Assume Σ_Q ≻ 0, and every pooled Σ̄_g ≻ 0 at every step of the alternation, not only at the start (the fit's ridge
gives the first; the second can fail when a group shrinks below n rows, and a group with a singular pooled
covariance has no metric).

**Theorem P3.1.**
1. For a fixed grouping, Ψ is least at M_g = Σ_Q⁻¹ # Σ̄_g, and its value there is Σ_g N_g F(Σ̄_g)².
2. For fixed metrics, Ψ is least when each cell goes to the group g that minimises ω_c(M_g)².
3. Alternating 1 and 2 (a cell keeps its group on a tie; an emptied group is dropped) never raises Ψ and stops
   after finitely many steps, at a grouping that neither step changes.
4. For every grouping, Σ_c n_c F(Σ_c)² ≤ Σ_g N_g F(Σ̄_g)² ≤ N·F(W_P)². The left side is the value with a basis per cell,
   the right the value with one basis. Call the ratio of Ψ to the left side the **price** of the grouping; it is
   at least 1 and at most the largest number of cells in a group.
5. Σ_g N_g F(Σ̄_g)² − Σ_c n_c F(Σ_c)² = Σ_c n_c d(Σ_c, Σ̄_a(c)), with d(X, Y) = ω_X(M(Y))² − F(X)² ≥ 0 the Bregman
   divergence of the convex function −F². So the scheme is Bregman hard clustering of the K cell covariances,
   weighted by n_c, with that generator.

**Proof.** (1) Ψ is linear in each Σ_c: Σ_{c in g} n_c ω_c(M)² = N_g·tr(MΣ_Q)·tr(M⁻¹Σ̄_g) ≥ N_g F(Σ̄_g)², with equality at the
geometric mean (Proposition 3). (2) The terms are separate. (3) Each step is a minimisation over one block with
the other fixed, so Ψ does not rise. With ties keeping the current group, step 2 changes the grouping only when Ψ falls strictly, so
no grouping recurs, and there are finitely many groupings. (4) Left: ω_c(M)² ≥ F(Σ_c)² for every M, cell by cell. Right: F² is concave (B4b item
1), and W_P is the average of the Σ̄_g with weights N_g/N. The upper bound on the price is B4b item 3, used with its two sides exchanged: B4b is stated for mixtures of query
families at a fixed Σ_R, and here the mixture is of residual covariances at a fixed Σ_Q, which is valid because F is
symmetric in its two matrices and ω(M; Σ_Q, Σ_R) = ω(M⁻¹; Σ_R, Σ_Q). (5) By (1),
F(Y)² = min_M tr(MΣ_Q)·tr(M⁻¹Y), so where F² is differentiable its gradient at Y is tr(M(Y)Σ_Q)·M(Y)⁻¹ (the envelope
theorem), and the tangent plane at Y evaluated at X is tr(M(Y)Σ_Q)·tr(M(Y)⁻¹X) = ω_X(M(Y))². ∎

**What it is and is not** (restated after the referee).
- *What it descends.* Ψ: the mean-square half-width of the M-norm certificate with nothing read, one norm per
  group, under three hypotheses left unstated before: one Σ_Q for every cell and group, unchanged by the regrouping
  (true of the fit as built); query and row paired uniformly with Δ uncorrelated with r, so that the mean square
  factors; and a certificate that knows each row's ‖r‖ in the M_g⁻¹ norm, which the index does not have with nothing
  read (the key gives the Euclidean norm). So Ψ is a proxy for the first levels, and `groups/REPORT.md` 3.4 is the
  warning about that proxy (P1.2).
- *Where it stops.* At a pair (grouping, metrics) each of which is optimal given the other. That is not a local
  minimum of Σ_g N_g F(Σ̄_g)² under the smallest change of grouping: on the referee's 40 random instances, moving one
  cell to another group and refitting both metrics lowered the value from the scheme's stopping point in 29. The
  earlier version said "local minimum". A pass of single-cell moves with both metrics refitted (Hartigan's rule
  where this is Lloyd's) improves on it, and the experiment adds one. In `check_p3.py`'s second instance 8 groups
  could hold the 6 kinds of shape at price 1 and the scheme stops at 1.13.
- *The divergence.* −F² is convex and finite on the positive semidefinite cone and differentiable on the positive
  definite cone, which is all the theorem uses. It is not strictly convex: F² is homogeneous of degree one, so
  d(tY, Y) = 0 for every t > 0, and d vanishes exactly when X is a multiple of Y. So d is a divergence between
  *shapes*, covariances up to scale, and a cell's scale enters only through its weight in the pooled covariance.
  The scheme has the structure of Lloyd's algorithm and of Banerjee, Merugu, Dhillon and Ghosh's Bregman
  clustering [cited, unchecked], but theirs takes a strictly convex generator, and their correspondence with
  exponential families and the uniqueness of the centroid do not carry over; here the centroid is unique up to
  scale, which is harmless because the metric is. The first argument of d may be singular (a cell with fewer rows
  than dimensions); the second may not.
- The function −F² and the object clustered (covariances of cells) are, as far as the librarian found, not
  published (`../library/partition.md` 1.5 and 6.1).

### P3.2 The same at a rate [model]

For a level read to a water level θ = λ⁻², replace −F² by −D_λ. D_λ is concave (P1.2), so the Bregman scheme with
generator −D_λ has the same two steps and the same descent, and the centroid of a group is again its pooled
covariance (the mean minimises the sum of Bregman divergences to a point, for any generator). The divergence is
d_λ(Σ_c, Σ̄_g) = Σᵢ [φ(uᵢ) + φ′(uᵢ)(λ²βᵢ^(c|g) − uᵢ)] − D_λ(Σ_c), with uᵢ = λ²γᵢ(Σ̄_g). For a direction above the level its first
part is 1 + ½ ln uᵢ + ½(βᵢ^(c|g)/γᵢ(Σ̄_g) − 1): the code length of the cell's coordinate under the group's Gaussian. So above
the level this is the classification form of a Gaussian mixture over cells, and below it the zero-read scheme of
P3.1. What it charges a cell is the tangent plane of D_λ at the pooled covariance, which lies above the group-wide
allocation cost of P1.3 for every cell and equals D_λ(Σ̄_g) on average (the referee's observation); the built code is
between the group-wide allocation and the diagonal model.
For the experiment both generators are specified, λ set at the 16 B level.

### P3.3 What it costs at build [estimate]

At N rows, n dimensions, K cells, G groups, per pass:

| step | work | at 1M, n = 768, K = 500, G = 64 |
|---|---|---|
| covariance of every cell | N·n² multiply-adds, K·n² numbers held | 6·10¹¹; 1.2 GB as float32 |
| ω_c(M_g)² for every pair | K·G·n² | 2·10¹⁰ |
| G metrics from the pooled covariances | G eigendecompositions, about 10·n³ each | 3·10¹¹ |

The fit already does the first once per group and the third once. So an iteration costs about one more basis
fit, and ten iterations about ten. At 1B rows the K·n² numbers do not fit (37 GB at K = 15,625); a sample of a few
hundred rows per cell gives tr(M_g⁻¹Σ_c) as a mean of ‖r‖² in the group's norm to about 1%, with candidates limited to
the few groups nearest in centre direction. A cell's own rows bias it towards staying in its group. Leaving them out of the pooled covariance it is compared
with removes the bias but is not the scheme of the theorem and loses its guarantee: on the referee's 40 instances
that variant raised the functional in 153 of 609 passes. The same is true of any cross-fitted variant. If one is
used, the descent must be checked on held-out rows and not assumed; the experiment runs the theorem's scheme and
a cross-fitted one side by side.

### P3.4 What it could gain, from theory and from the measured slopes [estimate; revised after the referee]

- *An upper limit.* No grouping at G groups beats a basis per cell (P3.1 item 4). Two measured slopes exist. On
  yahoo and LAION the median certified half-width at 16 B narrowed by 6% from 8 to 64 groups, about 2% per doubling,
  and the code bytes fell by 2.2 to 3.7% per doubling (`hierarchy/layered.md` 8.8) [measured there; not in this
  worktree]. On BioCLIP-2 at 1M, with the same cells and different groupings, bytes per item fell by 3 to 7% from 31
  groups to 64 and by 6 to 10% per doubling from 7 groups to a basis per cell (`rust/REPORT_CLUSTERSIZE.md`; P2.5)
  [measured there]. The earlier version used only the first. At 2,000 rows a cell and 64 groups a basis per cell
  is three doublings away, so a straight extrapolation gives 6% in width and 7 to 11% of bytes by the first slope
  and 9 to 30% of bytes by the second. Neither is a ceiling that can be reached: a basis fitted on 2,000 rows in
  768 dimensions is not estimable (that report's group of 624 rows has a basis of condition 108), and the report's
  basis-per-cell point is at 16,000 rows a cell.
- *What decides the share reached.* If a cell's shape varies smoothly with its position, grouping by centre
  direction is already near the best grouping and the scheme gains little. If shape varies with something else
  (the kind of content, the density), grouping by direction is no better than a random grouping and the scheme
  gains most. The toy of `check_p3.py` shows both: 1.023 → 1.008 when shape follows position, 1.381 → 1.128 when it
  does not. The measured slopes say grouping by direction does capture something (more groups help), which is
  evidence for the first case. With the steeper slope the estimate rises with the limit: a few percent of bytes,
  and I cannot say more than 1 to 6% [estimate; no measurement].
- *It descends a proxy.* Its proved descent is in the zero-read width (P3.1). The level form (P3.2) is the one
  that could predict bytes, and it is a model. The one existing measurement of a zero-read descent step found the
  bytes did not follow (P1.2). The estimate above is therefore of what a level-form scheme might reach, not of
  what P3.1's theorem delivers.
- *Its cost to the other uses.* The cells do not change, so the cut, the sorted range and the order are untouched.
  The groups stop being neighbourhoods. A query's cells are neighbours, and a group opened is n² bytes and 60 to 130
  µs (`hierarchy/layered.md` 8.4 and 8.10); today the share of groups opened is 2 to 2.7 times the share of cells
  read. With groups made by shape that concentration is lost in part, and at n = 768 one more group opened costs as
  much as the code bytes of about 9,000 to 31,000 rows read at 68 to 19 B. So the scheme should carry a penalty on
  the distance between a cell's centre and its group's.
- *Relation to the group count.* The number of bases is a trade between bytes read and setup time (P2.5). A better
  grouping at the same G moves that trade: it is worth x = (gain)/(slope per doubling) doublings of G in bytes at
  no setup, or the same bytes from 2^x times fewer bases. At a 3% gain and a 6% slope x = 0.5.

### P3.5 The same at the level of rows: each row to the cell where its interval is narrowest [the descent is proved for the idealised cost, unrefereed as a whole, with the referee's step for (c); its fit to the built code is a model]

This is the rule the librarian names as not found in print: Archer and Leen's coding-optimal partition, or the
entropy-constrained encoder, with the certified width as the cost. It contains the within-group case of P1.4.

**The cost of a row in a cell.** With nothing read, ω(x | c)² = tr(M_g Σ_Q)·‖x − m_c‖²_{M_g⁻¹}, g = a(c). At a level of B
bytes, in the group's dual coordinates z = V_g⁻¹(x − m_c), with sᵢ^g the scale factors of the group's water-filling
at that rate and W_g = Σᵢ sᵢ^g σᵢ(Σ̄_g) its floor width, the idealised cost (a coded coordinate's error is sᵢ times the
coordinate, and the level's weights are the best diagonal ones for the group) is

    ω_B(x | c)² = W_g·Σᵢ sᵢ^g zᵢ²,

a quadratic form in x − m_c with one matrix per group *and per level*.

**The price term, and the choice of level** (restated after the referee). At one level every group has spent the
same bytes (the level ends are global constants, `DLV` and `RLV` in `fit.py`), so the entropy-constrained encoder's
rate term is the same for every candidate cell and drops out, and the cell's label costs no bytes read. That is
true of one level and not of the rule as a whole. The form differs between levels and from the zero-read form (on
the referee's toy 12% of rows had a different cheapest cell at zero read than at a level), and a row is assigned
once and read through several levels. So the rule needs a level, or weights over levels by how often rows stop at
each; those weights do the work a price would, trading width early against width late. The earlier version said
no price term is needed, without this. The experiment uses the 16 B level as the rule, a zero-read rule as the
contrast, and one weighted rule (the sum over 4, 8, 16 and 32 B of ω_B², each divided by its mean over the rows of
the built partition).

**The scheme.** Alternate: (a) each row to the cell of least ω(x | c)², a row keeping its cell on a tie; (b) each
centre to the mean of its cell, a cell that has lost all its rows being dropped (and a group left with no cell
dropped with it); (c) each group's metric to that of its pooled covariance, which must stay positive definite at
every step (the fit floors the eigenvalues at 10⁻¹² of the largest; the functional is then that of the floored
covariance); and optionally (d) the cell-level step of P3.1. Steps (a) to (c) never change a cell's group; only
(d) does.

**Proposition P3.5.** With the level fixed, the objective Σ_x ω(x | c(x))² equals Σ_g N_g F(Σ̄_g)² after step (c) at zero
read, and Σ_g N_g W_g(B)² at a level in the idealised cost. No step raises it, and the scheme stops after finitely
many steps, at an assignment and metrics each optimal given the other. With (a) to (c) it searches the partitions
of the rows into cells with the cells' groups fixed; with (d) the groupings too.

**Proof.** (a) is a minimisation row by row. (b) For a positive semidefinite form the sum over a cell of
(x − m)ᵀA(x − m) is least at the mean. (c) at zero read is P3.1 item 1 with the rows of the group as the cells. (c) at
a level (the referee's argument; the earlier version asserted this step in one sentence): after (a) and (b) the
group's value is N_g·(Σᵢ sᵢqᵢ)(Σᵢ sᵢaᵢ) in the old pair, with aᵢ the new mean squares. That is at least N_g·(Σᵢ sᵢ√βᵢ)², with
β the diagonal of the new B(Σ̄_g) in the old eigenbasis (Cauchy–Schwarz; the old dual pair is an orthonormal basis
of the whitened space up to scaling of its columns, because Σ_Q is fixed). And Σᵢ sᵢ√βᵢ ≥ W_new(H), because the
water-filling value is a symmetric concave function of the spectrum, hence Schur-concave, and a diagonal is
majorised by the eigenvalues. So refitting the pair and the water-filling does not raise the value. With the tie
rule a change in (a) lowers the value strictly, so no assignment recurs, and there are finitely many. ∎
(The referee ran 36 instances of (a), (b), (c) at zero read and two rates with no rise, and 300 random basis pairs
none of which beat the dual pair with water-filling.)

**How it differs from everything measured.** Cells of two groups may now interleave: a row near one centre can
belong to a farther one whose group's basis fits its residual. The cells are no longer the Voronoi cells of the
centres in any one metric. `groups/REPORT.md` 3.4 is one step of the zero-read form with one group (P1.4).

**Two reasons its descent may not reach the bytes.** The zero-read form descends a quantity that the one
measurement shows need not carry into bytes (P1.2). The level form is idealised: the built code scales a row by
its Euclidean residual norm and then uses one table per group, so a coded coordinate's error is closer to a
multiple of the row's norm than of the coordinate. Both give a quadratic form per group and level, but not the
same form.

**What it could gain** [estimate; the one published size is from another kind of data]. Started from the
cell-level scheme's result it ends no higher than that scheme, because no step raises the value; from another
start nothing orders the two (each stops where it stops, and P3.1's toy shows the stopping point depends on the
start: 1.128 from the built grouping, 1.105 from a random one). The earlier version said it "gains at least as
much", without the condition. Its extra gain is the difference in shape *within* a cell and the within-group
flattening of P1.4, which no measurement here speaks to at a level. Archer and Leen report 1.1 to 1.8 dB over
k-means regions at 16 and 32 regions on image blocks, each region with its own transform and allocation
(`../library/partition.md` 1.4) [cited there from the paper]; at equal rate that is an rms error 0.81 to 0.88 of
k-means'. Image blocks have a few sharply different kinds (edges by orientation, textures, flats); whether rows of
an embedding within one cell of 2,000 do is what the experiment asks, with one assignment step and no refit. The
toy of `check_p3_rows.py` (two kinds of residual in every blob, zero-read form): rms zero-read width 0.966 of the
built grouping at the cell level and 0.932 at the row level, with 25% of the rows moved and the k-means objective
up by 4%; the descent was monotone in 31 steps.

**What it costs the other uses** [argued from P1.5 to P1.7]. A row moved to a farther centre has a lower key, so
it is among the rows the sorted range leaves; it widens its new cell's extent along the anchors; and the centres
explain less of the score variance. In the toy the mean square radius rose 4%. So the row-level scheme trades the
range, the cut and the order against the code, and the experiment must report all four. A price on the key,
ω(x | c)² + ν·(1 − x·m̂_c), moves along that trade; ν → ∞ is the built partition.

---

## P4. The alternatives, each against the five uses

**An exchange rate, and its limit** [model U, P2.2; limited after the referee]. A cell whose dual spectrum is
larger by a factor ρ² in every direction costs the second-moment code what D·log₂ρ fewer doublings of K cost. At
D = 19 a uniform loosening of 10% is 2.6 doublings, 2 to 6 B a row read to 4 to 96 B. This prices a *uniform*
loosening in the dual metric. It does not price a change of shape, and the Euclidean radius is the wrong measure
for one: in `check_p3_rows.log` the row-level scheme raised the mean square radius by 4% and lowered the rms width
by 7%. The earlier version charged it to every alternative that "loosens the cells". It is a reason against an
alternative only if its cells are looser in the directions the dual metric weights, which I have not shown for
any of them. Below it is used as a question to put to an alternative, not as a charge.

### P4.1 Balanced k-means [model for the distortion; the rest argued from P1]

- *Coding.* Balance is a constraint on the sizes and gives the code nothing to use; whether it costs depends on
  what it does to the groups' scatter (P1.4). The classical
  estimate, from the point density of a best quantiser, λ*(x) ∝ f(x)^(D/(D+2)) against a balanced one ∝ f(x): the
  distortion ratio is E[f^(−2/D)]/(E[f^(−2/(D+2))])^((D+2)/D), 1.006 for a Gaussian density at D = 19 (1.02 at D = 10, 1.10 at
  D = 5), and ln of the ratio is about 4·Var(ln f)/(D²(D + 2)) in general [computed here from the standard formula,
  which is cited, unchecked, and which assumes Gersho's conjecture, unproved above dimension 2]. **This is outside
  its regime**: with 500 to 1,000 cells in 19 intrinsic dimensions the partition is at about 0.5 bit a dimension,
  where the librarian's sources say rate-distortion theory and not high-resolution theory applies
  (`../library/partition.md` 2.3). I keep it only as a reason to expect the cost to be small; the experiment
  measures it.
- *Cut and sorted range.* Sign unknown. The range's count is closer to a sum of radii than of squared radii
  (P1.6), and at a given sum of squares unequal radii have the smaller sum. Chávez and Navarro found unbalanced
  partitions better as the intrinsic dimension grows, on synthetic data up to dimension 20
  (`../library/partition.md` 5.2) [cited there]. Both point the same way: balance may cost the range a little.
- *Approximate order.* The explained variance barely moves; what balance buys is a known cost per probe, which is
  SPANN's stated reason for it (same file, 3.1).
- *Layout.* The gain is here: no cells of 84 rows beside cells of 4,635, so fewer cells too thin to open (P1.8).

Verdict: a few percent of time through the layout at most, nothing for the code, a possible small loss in the
range. Not a candidate to beat k-means by more than a few percent on any use; cheap to score.

### P4.2 A balanced partition of the nearest-neighbour graph [cited evidence; the judgement is argued]

Its objective, the number of neighbour pairs cut, is the right one for the approximate order with queries drawn
like the data: it counts neighbours kept together, where the variance of P1.7 is an average over all rows.
Published: up to 2.3 times fewer candidates than k-means at 16 and 256 bins on 1M points (Neural LSH), up to 2.14
times the throughput at about 40 shards on 10⁹ points (Gottesbüren et al.) (`../library/partition.md` 3.4) [cited
there, papers read by the librarian]. Every result is empirical, none is at thousands of cells, and none is for
queries from another distribution.

Against the other uses it loses each time, and for one reason: its cells are not caps about a centre. Gottesbüren
et al. found routing by one centre per part poor (28% of neighbours in the top-ranked part on one set) and routed
with a k-means index inside each part. For this index that means a wider hull along the anchors (the cut), lower
keys (the sorted range), a residual about the one stored centre that is larger and, since a part is not a
cap, plausibly larger in every direction (the code, if the exchange rate applies; not shown) and
an order μ_c + ψσ_c that no longer describes the cell. It would have to sit over k-means leaves, as a routing level.
It also needs the neighbour graph at build.

Verdict: the only alternative with published gains of more than a few percent for the approximate order, at cell
counts 10 to 100 times smaller than this index's; not a replacement for the cells the exact path uses. The
experiment scores the built partition on its objective (neighbours kept together) so that its headroom is known.

### P4.3 Cells that are flats or low-rank Gaussians [proved or argued in P1.3, P1.4, P3; evidence cited]

This family minimises a coding length: Ma, Derksen, Hong and Wright's Σ over groups of
(m + n)/2·log₂det(I + n/(ε²m)·VVᵀ) plus membership bits, k-subspaces' distance to a flat, a mixture of probabilistic
PCA's likelihood (`../library/partition.md` 1.1, 1.2). Placed against P1:

- Ma et al.'s regularised length of a cell is Σᵢ ½ ln(1 + γᵢ/θ′) with θ′ = ε²/n: linear in γᵢ below the level and
  logarithmic above. The certified width's cost is Σᵢ φ(γᵢ/θ): a square root below the level and logarithmic above
  (P1.1). Both are concave functions of the spectrum, and both prefer flats; they differ below the level, where a
  squared error charges an uncoded direction its variance and a certified width charges its standard deviation.
  So Ma et al.'s objective runs from k-means (ε large) to the log-determinant, and this index's from the nuclear
  norm F to the log-determinant. That confirms and sharpens the librarian's summary (1.5): F is the zero-read end
  and the log-determinant the high-rate end of one family. [proved: a comparison of two scalar functions.]
- Their Theorem 3 and Example 1 (concavity of log det makes a hard split pay) are P1.2 item 3's second inequality
  for one concave function; P1.2 states it for every concave spectral cost. Not new in kind.
- Ma et al. price a basis at n more vectors' worth of rate (the factor m + n). Here a basis is n² bytes, the code
  bytes of 9,000 to 31,000 rows at 19 to 68 B a row, against n = 768. Their price would give 10 to 40 times too many
  bases for this index.

What the family can do here (corrected after the referee; the earlier version said "nothing by moving rows
within a group", which P1.4's counter-example refutes): flatten a group's pooled scatter by moving rows between
its cells (P1.4 case 1); the cell-level scheme (P3.1); the row-level scheme (P3.5), which is this family's
assignment rule with the index's cost and contains the first; and state per cell. For the last, the nearest
published design is locally optimised product quantisation, a rotation per cell (`../library/partition.md` 3.3);
at n = 768 a basis per 2,000-row cell is 295 B a row, so only a few directions per cell are affordable. The index
already stores 16 directions per cell for the approximate spread (6.1 B a row, P2.5). Coding the residual's part
along them first, with the group's basis for the rest, is bounded above by the basis-per-cell limit of P3.4 and
costs a second transform in the kernel [estimate; not specified further here].

Verdict: this is where a gain of more than a few percent on the code could be, through P3.5, and the published
size (Archer and Leen, P3.5) is from image blocks. The proved descent is in a zero-read or idealised cost (P3.5).

### P4.4 A second assignment per row [cited evidence; the judgement is argued]

SOAR places a second copy where its residual is orthogonal to the first, so that the two cells' score errors are
uncorrelated. Published: rows read to 95% recall fall by 1.14 times on GloVe-1M at 2,000 cells and by 3.4 and 4.3
times at 10⁹ points with 7.2M cells (`../library/partition.md` 3.2) [cited there, paper read]. The first figure
is the nearest published one to this index's cell size and is still a factor of four away: 2,000 cells of 500 rows,
where the index at 1M has 500 cells of 2,000. For the exact path a row must be decided once, the build refuses two rows of
one entity, and a copy widens its second cell's extremes and range. So it could only be a list of references used
by the approximate path, at a random read per reference, against the contiguity that P1.8 values.

Verdict: the approximate order only, and about 1.1 times at the nearest published cell size. Not recommended.

### P4.5 Hyperplane trees [cited; argued]

Cells from recursive splits are balanced and cheap to route to. With log₂K = 9 to 14 splits in 19 intrinsic
dimensions a cell is bounded in at most 14 directions and loose in the rest, so its radius exceeds a k-means
cell's; whether that costs the code depends on the directions in which it is loose (the exchange rate's
limit). Verma, Kpotufe and Dasgupta found 2-means trees to have the
lowest quantisation error among the trees they compared (`../library/partition.md` 5.4) [cited there, abstract
only]: the best tree is hierarchical k-means, which is P4.1. Ramaswamy and Rose's hyperplane bounds are a property
of the bound, not of the partition, and the support table is already of that family (same file, 5.3).

Verdict: no use on which it beats k-means; useful only as the way to build a balanced k-means.

### P4.6 Lattice and product partitions [model]

They remove the stored centres, so they allow many more cells for the same state. By P2.3 the centre's bytes are
not what sets the cell size: the per-cell time is, at 7 to 13 centres' worth, and a product partition multiplies
the number of cells that pay it. Their cells are also looser than k-means cells at equal count (a product of two
half-space quantisers; a lattice ignores the density), which costs the code if they are looser in the dual metric (not shown). An additive form of the
support table over the factors would make the cut cheap for product cells, but the sorted range, the key and the
contiguous layout have no such form.

Verdict: no.

### P4.7 Two alternatives the brief did not list

- *A partition chosen for the radius* (k-centre), which the librarian found never compared with a
  variance-chosen one in hundreds of dimensions (`../library/partition.md` 5.5). Prediction [model]: the logarithm
  of a row's radius has standard deviation about (2n_eff)^(−½), so the largest of n_c radii is about
  exp(z(n_c)/√(2n_eff)) times the typical one: 1.32 at n_eff = 76 and 2,000 rows. That factor is all a radius-chosen
  partition could remove, and the rows that carry it are far from every centre, so moving them gains little. The
  cut's extent along an anchor is set by alignment, z(n_c)·√(aᵀΣ_c a), which is a variance again. So I expect a
  radius-chosen partition within a few percent of k-means on the cut and the range; I make no claim for the code. The
  experiment reports the ratio of the largest to the rms radius per cell, which tests the 1.32.
- *Cells aligned with a filter's attribute.* If visible sets are slices of an attribute with A values, a
  partition made inside each value meets A times fewer cells for a filtered query (P1.8). For an attribute
  independent of the vectors it spends log₂A of the label's bits on something the code cannot use, which for an
  unfiltered query is log₂A doublings of K lost: 0.8 to 2.4 B a row each (P2.2). For an attribute the vectors
  nearly determine, as a taxon is by an image embedding, k-means has the purity already. The region-filter study in
  `vexact-sc` is the place for this; I have not read it.

### P4.8 The ranking [revised after the referee]

| | alternative | use it could improve | size | what it costs |
|---|---|---|---|---|
| 1 | rows to the cell of narrowest interval at a level (P3.5), which includes flattening a group's scatter (P1.4) | coding | unknown here; 12 to 19% in rms error on image blocks (Archer and Leen) | lower keys, wider extents, less variance explained; all measurable |
| 2 | cells to bases by coding cost (P3.1, P3.2) | coding, or the same bytes from fewer bases | a few percent of bytes, 1 to 6% estimated; the limit, a basis per cell, is 7 to 30% away by two measured slopes and cannot be reached | groups stop being neighbourhoods: more setups per query unless penalised |
| 3 | neighbour-graph partition (P4.2) | approximate order, queries like the data | up to 2.3 times at 16 to 256 cells; nothing known at thousands | every exact use; needs k-means leaves under it |
| 4 | the built rule run to a fixed point (P1.6) | sorted range, cut, coding, order | unknown; on a toy 11.7% of rows were not at their best centre and the sum of keys rose 4.7% | build time only |
| 5 | balanced k-means (P4.1) | layout | a few percent of time | perhaps the range, slightly |
| 6 | k-means in the queries' metric (P1.7) | approximate order | unknown; a doubling of cells is worth 0.03 of f | unknown for the code and the range |
| 7 | second assignment (P4.4) | approximate order | 1.14 times at the nearest published cell size | contiguity, the exact path's one-row rule |
| 8 | radius-chosen, hyperplane trees, lattice and product | none | | |

Three things to hold against this table.
- Rows 1 and 2 are ranked on schemes whose proved descent is in the zero-read width or in an idealised level
  cost. The one existing measurement of a zero-read descent step (`groups/REPORT.md` 3.4) improved the zero-read
  quantity and not the coordinates read. So their place rests on the level forms, which are models, and on a
  published result from another kind of data. The experiment's headline is therefore a level cost and bytes,
  with the zero-read width as a secondary column.
- Row 4 is new. It is not an alternative to k-means; it is k-means finished. It is first in the experiment
  because it is the cheapest thing to measure and every other comparison is against a partition that may be 5 to
  10% of rows from its own fixed point.
- Only rows 1 and 3 could beat k-means by more than a few percent on a use, by what is published. Row 3 is not a
  partition the exact path can use. Row 1 is untested on embeddings.

---

## P5. A scoring recipe: what to compute from a partition and the data, with no index build

Inputs: the rows X, an assignment of rows to cells and of cells to groups, one fixed Σ_Q (from the built partition,
so that partitions are compared in one metric), a sample of queries with their exact thresholds. Every score is
computed on rows held out from the fit of the bases (half the rows of each cell fit, half score).

| score | definition | predicts | from |
|---|---|---|---|
| S1a | Σ_g (N_g/N)·F(Σ̄_g), and the root of Σ_g (N_g/N)·F(Σ̄_g)² | the zero-read width | P3.1 |
| S1b | Σ_g (N_g/N)·H*(w₀; Σ̄_g)/(8 ln 2) bytes at fixed widths w₀, and the floor width W at 4, 8, 16, 32 B | bytes a row needs at a width (group-wide allocation) | P1.1 |
| S1c | Σ_c p_c Σᵢ φ(λ²βᵢ^(c|g)) at the λ of the 16 B level | the same under the diagonal model | P1.3 |
| S1d | the mean over held-out rows of √(W_g·Σᵢ sᵢ^g zᵢ²) at 16 B | a row's width at a level; the quantity P3.5 minimises | P3.5 |
| S1e | fixed bytes per row: n·K/N + n²·G/N | the state the partition adds | P2.5 |
| S2 | R_cut(q, t)/N from the exact extremes of every anchor over every cell | rows left by the cut | P1.5 |
| S3 | R_range(q, t)/N from the keys; and, per cell, the largest over the rms radius | rows left by the sorted range | P1.6 |
| S2+3 | rows undecided by both | candidates entering the levels | |
| S4a | the mean over queries of qᵀB_P q / qᵀT q | variance explained, the order | P1.7 |
| S4b | for sample rows, the share of their 10 nearest rows in their own cell; and rows in the cells visited in the order μ_c + 3σ_c(q) until 9 of the 10 best are covered | the approximate read | P4.2 |
| S5 | the coefficient of variation of n_c; the share of rows in cells under 500 rows; cells met by a slice of consecutive ids | layout | P1.8 |

The headline coding scores are S1d at 16 B and S1b (a level cost and bytes); S1a, the zero-read width, is
reported beside them as a secondary column, because a gain there need not carry into bytes (P1.2). S0, added after
the referee and placed first in the experiment: the share of rows whose largest key is not at their own stored
centre, and every score again after further Lloyd passes on all rows (P1.6).

A single figure for the exact path, if one is wanted [model]: bytes a row ≈ (S2+3 share)·(bytes at the row's
margin from S1b), with the margin taken from the query's score distribution; the per-cell time is K times the unit
costs of P2.3 and does not depend on which partition of K cells it is.
Arithmetic of P2 and P4: `check_p4.py`, log `check_p4.log`.

---

## L. The claims against the librarian's report (`../library/partition.md`)

| librarian's finding | where it meets these claims |
|---|---|
| Coding cost charges a cell its log-determinant where k-means charges its trace; the index's own cost, F = Σσᵢ for a group plus each cell's radius, matches neither, and no published objective charges it (1.5, 1.6) | Confirmed and sharpened in P1.1, P1.2 item 4 and P4.3. The cost of a cell at a price is Σᵢ φ(λ²γᵢ): F at zero bytes read, the log-determinant at high rate, and between them at the depths the index reads. Ma et al.'s regularised length runs from the trace to the log-determinant; this one from F to the log-determinant. The radius is P1.5 and P1.6. |
| Ma et al.: concavity makes a hard partition optimal and a split pay (1.1) | P1.2 items 2 and 3 are the same argument for any concave spectral cost. Not new in kind. |
| Archer and Leen assign each vector to the region whose coder distorts it least and gain 1.1 to 1.8 dB over k-means (1.4) | Their regions carry a transform each, so they are this index's groups, not its cells. The rule is P3.5; the gain is the only published size for it and is from image blocks. P3.1 is the same idea with whole cells as the things assigned. |
| The entropy-constrained encoder adds λ times the code length, so its cells are not Voronoi cells (1.3) | P3.5: the levels end at the same byte for every group, so the price term vanishes; with rates that varied by group it would be the group's rate. The cells of P3.5 are not Voronoi cells either, for a different reason (a metric per group). |
| Locally optimised product quantisation, a rotation per cell, is the nearest analogue of the per-group basis (3.3) | P4.3: at n = 768 a rotation per cell is 295 B a row; a few directions per cell is the affordable form. |
| Not found: a partition chosen for a certified bound's width or the rows it leaves (6.1) | P1.5 and P1.6 give the two counts as functions of the partition; P3.1 and P3.5 give schemes that descend the zero-read width, and an idealised width at a level. I have not searched further than the librarian. |
| For exact search the slack of a cell is its covering radius, not its variance (5.1); unbalanced beat balanced as the intrinsic dimension grows (5.2); radius-chosen against variance-chosen has not been compared in hundreds of dimensions (5.5) | P1.5: for the support table the slack is the hull's width along an anchor, which under a Gaussian model is z(n_c) standard deviations, so a variance returns. P1.6: for the key it is the tail of the radius. P4.7 predicts the largest radius at 1.32 times the typical one and a small gap between the two kinds of partition. P4.1 notes the direction of the balance result. |
| Graph partitions read up to 2.3 times fewer candidates; all gains empirical; none at thousands of cells (3.4, 3.6) | P4.2. |
| Error falls as K^(−2/n) or K^(−2/d_int); at about 1,000 cells in 768 dimensions the index is outside the high-resolution regime (2.3, 4.2) | Agreed. P2.1's exponent is a measured slope (0.106 with K varied at fixed N; 0.17 with N varied at a fixed cell size), not Zador's limit, and model U is fitted to it. The floor used in P1 and P2.2 is a reverse water-filling, which is the rate-distortion kind of statement. The one high-resolution formula I use (P4.1) is marked as outside its regime. |
| The √N rule has no published derivation, and no cost model gives K or the number of bases for a certifying index (4.1, 4.4) | P2.3 gives one for K: S* = c_K·ln 2/(u·s_B) and S* ∝ N^(1−η). It gives the timed optimum at 1M to a factor of three and the exponent ½ for item queries. P2.4 gives the leaf size. The number of bases is `hierarchy/layered.md` (8.1), which P2.5 and P3.4 restate in terms of the price of a grouping. |
| Gersho's conjecture is unproved above dimension 2 | P4.1 says so where it uses the point density. |

One correction to the report's premise: it takes the cell size as about 1,000 rows (`SYSTEM.md` 2.1). The code's
default is 2,000·√(N/1M) (`rust/tools/rules.py`), so the index does follow a √N rule, and the report's comparison
at 10⁸ rows (4.1) is with the earlier default.

---

## S. The short answer [rewritten after the referee: the mismatch is not only in the basis]

1. **The code's cost is concave in a covariance's spectrum, where k-means' is linear, and that mismatch is present
   at every step where a covariance is formed.** The cost is between F = Σσᵢ and the log-determinant in the dual
   metric; k-means minimises Σσᵢ² (P1.1, P1.2). It shows in three places: which rows share a cell inside a group,
   because the group's basis is fitted to the scatter pooled over its cells and cells flat along the same
   directions lower it (P1.4); which cells share a basis (P3.1); and which basis a row is coded in (P3.5). Nearest
   centre in a group's own norm is a condition that partitions of different cost all satisfy, not a minimiser.
   The earlier version said that within a group k-means is the right objective and that the mismatch is only in
   the basis. That was wrong.
2. **How much that is worth in bytes is not known, and the one measurement is a caution.** A single descent step
   for one basis (`groups/REPORT.md` 3.4) moved the zero-read quantity the right way (undecided by the ball alone
   68.0 → 54.9% for items) and the coordinates read the wrong way (by 0.6 to 3.6%). The published coding-optimal
   partition gains 12 to 19% in rms error on image blocks. Nothing has been measured at a level on embeddings. The
   schemes of P3 have proved descents in the zero-read width and in an idealised level cost only.
3. **For the sorted range the built objective addresses the right quantity, and the built fit does not finish the
   job.** The range counts the low tail of the keys; spherical k-means ascends their sum; the fit runs 12 passes on
   a sample, reassigns, and recentres, so it is not at a fixed point, by an amount not known on real data (P1.6).
4. **For the approximate order the trace is the right kind of objective**, in the queries' metric (P1.7).
5. **For the cut the first thing is the net, a property of K and of the queries; the cells' extent is a
   correction to it. For the layout the quantity is a count.** K-means optimises neither, and neither is where a
   new rule for forming cells would pay (P1.5, P1.8).
6. **The number of cells and the number of bases are trades.** A doubling of K lowers the second-moment cost by
   what a target width 3.7% larger saves (model U; 1.0 B per doubling per entering item measured at text 1%), and
   the cell size is held up by the per-cell time, with the optimum known to a factor of three (P2.2, P2.3). Fewer
   bases cost bytes read (3 to 10% per halving on BioCLIP-2) and save setup time and 37.7 B a row of storage (P2.5).
7. Of the other alternatives only a neighbour-graph partition has published gains above a few percent, for the
   approximate order alone, and its cells are not ones the exact path can use (P4.2).

## What failed or was withdrawn

Before the referee:
- A bound I first wrote: no partition into K cells beats k-means for the code by more than log₂K bits a row. True
  against the best code for the true law, false for the second-moment code the index uses (P2.2).
- P1.4 as first written credited `groups/REPORT.md` 3.4 with testing the within-group rule. It tested one metric for
  the whole corpus.
- The balance estimate from the point density formula stands only as an indication (P4.1).

After the referee:
- P1.4 case 1 ("within a group the coding-optimal rule is k-means in a metric; flat cells cannot lower the cost
  within a group"): wrong; the referee's counter-example is my own example. With it go the second caution after
  P1.3's example, P1.9's coding row, P4.3's "nothing by moving rows within a group" and S.1.
- P2.2 as a statement "proved in model U" that a doubling "buys exactly" a wider width: it is an identity for a
  second-moment cost and not a floor. Its comparison with measured bytes at text 1% (1.85 B) mixed in the pruning,
  and the one at text 50% compared full-vector bytes with a bound on code.
- P2.3's S* column (6,500 / 17,000 / under 2,700) and the figures for where the rule and the text optimum meet at
  1B: good to a factor of three; the second withdrawn.
- P2.5's "the theory sides with fewer bases, the direction of every measurement": fewer bases cost bytes on this
  corpus.
- P3.1's "local minimum"; P3.5's "no price term is needed" (true of one level); P3.5's "gains at least as much as
  the cell-level scheme" (true from that scheme's result).
- The exchange rate as a charge on every alternative (P4).
- The estimate for the cell-level scheme was 1 to 3% from one slope; with the slope measured on this corpus it is
  wider and less certain (1 to 6%).

## Changes after the referee

Referee 1's report is `referee/referee-1.md`. P1.1, P1.2 items 1 to 5, P1.3's inequalities, P1.5 to P1.7's identities,
P2.2's derivation, P2.3's formulas, P3.1 items 1 to 5 and P3.5's descent survived. I dispute none of its points. No
tag has been raised; P2.2's tag is narrowed. Checks of the repairs: `check_after_referee.py`, log beside it.

| claim | what changed |
|---|---|
| P1.1 | Added: it is exactly as strong as F1; the hypotheses per cell; one Σ_Q for all cells is a model; the label is free; the symbol c. |
| P1.2 | Item 3: the gain of a split is a gain in a model cost, not in a floor. Item 5: eigenvalues need not be distinct; exact for λσ₁ ≤ 1. The consequence says which of three readings is meant (a majoriser touching at the current partition when one basis is refitted: a descent step, at zero read only), and records that in the cited measurement the zero-read quantity improved and the coordinates read rose. |
| P1.3 | Four ordered quantities, as averages over a group; D_λ(Σ̄_g) is an upper bound on the group-wide allocation cost. The built fit ascends Σ n_c‖m_c‖, not the trace. The example: the condition w below both zero-read widths, the case h² > a²/12, fixed width against fixed price. The second caution replaced by its correction. |
| P1.4 | Case 1 restated: for a fixed metric the nearest centre; the metric is refitted; the cost N_g·F(W_g)² is concave in the group's scatter; the counter-example; what a group can and cannot use. Two more hypotheses stated (direction-dependent error; the norm depends on the level). |
| P1.5 | β ≥ 0; the net term dominates and is listed first. |
| P1.6 | The fit stated exactly; "maximises" replaced by "ascends"; the referee's toy figures; clamping and the non-strict inequality. |
| P1.7 | The identity is for the average explained variance, not the average share. |
| P1.9 | Coding, cut and range rows rewritten. |
| P2.1 | Model U's assumptions listed; the two exponents are two experiments, and the reading "the exponent falls as K grows" is withdrawn as one of two readings. |
| P2.2 | Retagged and reworded as an identity for the second-moment cost, with the reason it cannot be a floor; first-order figure an upper bound; cap 40.4 bits; the two comparisons corrected. |
| P2.3 | ε = 0.060; per-cell work as S^(−p) with the measured p; both S* columns; "good to a factor of three"; the extrapolation in N inherits it; 2.3 times, not double, with the support table. |
| P2.5 | The number of bases as a trade between bytes and time, with the cluster-size report's figures. |
| P3.1 | Positive definite at every step; B4b used with its sides exchanged; where the scheme stops; the divergence is between shapes and −F² is not strictly convex; the unstated hypotheses; the proxy. |
| P3.2 | It charges the tangent plane, above the group-wide allocation cost. |
| P3.3 | Leave-out and cross-fitted variants are not the theorem's scheme and can raise the functional. |
| P3.4 | Both measured slopes; the limit cannot be reached; the estimate widened; the proxy; the group count as a trade. |
| P3.5 | The referee's argument for step (c); tie and empty-cell rules; steps (a) to (c) keep groups fixed; the price term per level only, and the levels the experiment uses; the condition on "at least as much"; two reasons the descent may not reach bytes. |
| P4 | The exchange rate limited to a uniform loosening in the dual metric; P4.1, P4.2, P4.5, P4.6 and P4.7 reworded where they leaned on it; P4.3's list corrected; SOAR's cell size; the ranking revised, with the built rule run to a fixed point as a new row and the proxy warning under the table. |
| P5 | Headline scores are a level cost and bytes; S0 added. |
| S | Rewritten. |

Two small additions of mine to the referee's figures, neither a dispute. On a sample of 20,000 rows partition B of
the counter-example has 7 to 17 rows beside s = 0 with a nearer centre in the group's own norm, where the referee
reports none; that is the sample's noise in the centres. And in the group-count comparison the bytes with the full
vectors taken out rise as fast or faster per halving of G (2 to 13%) at answers up to 1%, and about half as fast at
50%, where half the rise is full vectors; the referee's reading stands.

Not refereed and unchanged except where noted: P1.8, P2.4, P3.2 to P3.4, P5, section L. Figures cited from
`hierarchy/layered.md` and `hierarchy/theory.md` (in the vexact-visitorder worktree) remain unchecked by a referee.

## Files

`CLAIMS.md` (this file), `CLAIMS_before_referee.md` (the text the referee saw), `NOTEBOOK.md`, `EXPERIMENT-SPEC.md` and
`EXPERIMENT-SPEC_before_referee.md`; `referee/` (the referee's report and scripts); `check_p1.py` (P1.1 to P1.3),
`check_p3.py` (P3.1, toy), `check_p3_rows.py` (P3.5, toy), `check_p4.py` (arithmetic of P2 and P4, as first written;
its S* figures are the p = 1 ones with ε = 0.055), `check_after_referee.py` (the repairs), each with its `.log`. Every
script is numpy on synthetic data or arithmetic on printed figures and runs in seconds; none reads a data file.
