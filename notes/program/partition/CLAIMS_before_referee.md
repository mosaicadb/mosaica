# Partition track: claims

The owner's question: LIMINAL groups its rows by spherical k-means; is that the right thing to group by, and what are
the alternatives? Every claim below carries a tag of `notes/README.md` and its derivation. Nothing here is refereed.
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

### P1.2 Properties of D_λ [proved, unrefereed]

**Proposition P1.2.**
1. φ is increasing, concave and continuously differentiable on (0, ∞), with φ′(u) = 1/(2√u) for u ≤ 1 and 1/(2u) for
   u ≥ 1.
2. D_λ is concave on the positive semidefinite matrices and monotone: Σ ⪯ Σ′ implies D_λ(Σ) ≤ D_λ(Σ′).
3. *Refinement.* Let a cell with covariance Σ be split into sub-cells j with shares π_j, means m_j and covariances Σ_j.
   Then Σ = Σ_j π_j Σ_j + Σ_j π_j (m_j − m)(m_j − m)ᵀ and

       D_λ(Σ)  ≥  D_λ(Σ_j π_j Σ_j)  ≥  Σ_j π_j D_λ(Σ_j).

   The first gap is what the sub-cells' centres remove. The second is what their difference in shape removes.
4. *The three exponents.* k-means in the query metric minimises Σ_c n_c tr(Σ_Q Σ_c) = Σ_c n_c Σᵢ σᵢ(Σ_c)². As λ → 0,
   D_λ(Σ)/λ → F(Σ) = Σᵢ σᵢ(Σ). As λ → ∞ with B(Σ) non-singular, D_λ(Σ) − n(1 + ln λ) → Σᵢ ln σᵢ(Σ) = ½ ln det B(Σ). So the
   three objectives are the sums of σᵢ², σᵢ and ln σᵢ over the same singular values, and the floor at a price moves from
   the second to the third as the price rises.
5. *Gradient.* Where the γᵢ are distinct and positive, with B(Σ) = U diag(γ) Uᵀ,
   ∇D_λ(Σ) = Σ_Q^½ U diag(λ²φ′(λ²γᵢ)) Uᵀ Σ_Q^½. As λ → 0 this is (λ/2)·M(Σ)⁻¹.

**Proof.** (1) Both pieces are increasing and concave; at u = 1 the values are 1 and the derivatives ½; the
derivative is decreasing across the join. (2) Σ ↦ B(Σ) is linear and preserves the order. For a concave f and
symmetric A = tX + (1 − t)Y with orthonormal eigenvectors uᵢ, tr f(A) = Σᵢ f(t·uᵢᵀXuᵢ + (1 − t)·uᵢᵀYuᵢ)
≥ t Σᵢ f(uᵢᵀXuᵢ) + (1 − t) Σᵢ f(uᵢᵀYuᵢ), and Σᵢ f(uᵢᵀXuᵢ) ≥ tr f(X) because uᵢᵀXuᵢ is an average of the eigenvalues of X with
weights that sum to one over i as well. For monotonicity, X ⪯ Y gives γᵢ(X) ≤ γᵢ(Y) for every i (Weyl) and f is
increasing. (3) The identity is the law of total covariance. The first inequality is monotonicity, the second
concavity. (4) The limits of φ. (5) The derivative of a spectral function; at λ → 0, λ²φ′(λ²γ) = λ/(2√γ), and
Σ_Q^½ B^(−½) Σ_Q^½ = M⁻¹. ∎

**Consequence: what k-means sees and what it misses.** For k-means the cost of a cell is linear in Σ_c, so the
second gap of item 3 is identically zero: it gains only from where the centres are. The coding cost has both terms.
Item 5 says that to first order about a reference covariance the zero-read cost is k-means in the M⁻¹ norm of that
covariance. That first-order experiment has been run with one reference covariance for the whole corpus: assigning rows to the
centre nearest in the M⁻¹ norm of the pooled covariance changed the coordinates read by under 1%
(`groups/REPORT.md` 3.4) [measured there]. So what remains untested is the second gap, the difference in shape
between the things that share a basis, and that is a property of the groups (P3), because the index has a basis
per group and not per cell.

### P1.3 The partition's coding objective, and when it disagrees with k-means [model]

At a price λ the bits and width of the whole index, in nats, are at least

    J_λ(P, a) = Σ_c n_c D_λ(c | a(c)) + K·c_m + G·c_V,

where c_m is the stored size of a centre and of the rest of a cell's state, c_V = 8n² ln 2 that of a basis, and
D_λ(c | g) is the cost of cell c under group g's basis. Three models of D_λ(c | g), in decreasing cost:

- *Group-wide allocation*: every row of the group is coded as if its residual had the pooled covariance,
  D_λ(Σ̄_g).
- *The diagonal model*: the group's basis pair with the best allocation for the cell,
  Σᵢ φ(λ² βᵢ^(c|g)), where β^(c|g) is the diagonal of B(Σ_c) in the eigenbasis of B(Σ̄_g). It is the cost of the family
  the index uses (weights diagonal in the group's basis, |Δ·e| ≤ ‖DΔ‖·‖D⁻¹e‖) when each coordinate's error is scaled
  by sᵢ at rate ln(1/sᵢ): the least of √(tr D²Q)·√(tr D⁻²S²A) over diagonal D is Σᵢ sᵢ √(qᵢaᵢ), and in the group's dual
  pair qᵢaᵢ = βᵢ^(c|g).
- *Own basis*: D_λ(Σ_c).

They are ordered: Σ_c (n_c/N_g) D_λ(Σ_c) ≤ Σ_c (n_c/N_g) Σᵢ φ(λ²βᵢ^(c|g)) ≤ D_λ(Σ̄_g). The first is Schur's inequality
(a diagonal is majorised by the eigenvalues and φ is concave); the second is Jensen, because the β^(c|g) average to
γ(Σ̄_g) over the group. [proved, unrefereed, as inequalities between the three models; that the built code behaves as
the diagonal model is the model.]

**k-means' objective** is J_km(P) = Σ_c n_c tr Σ_c (the built fit; Σ_Q = I in P1.2 item 4).

**Exactly when the two disagree.** Take two candidate cells with dual spectra γ and γ′. k-means prefers the one
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

So **coding prefers A exactly when w < 2h, and k-means prefers B exactly when h < a/2**: for w/2 < h < a/2 they
disagree, whatever a is. With a = 1 and h = 0.1 the traces are 0.333 (A) and 0.093 (B), so k-means prefers B by a
factor 3.6. At w = 0.05, H*_A = ½ ln 133.3 = 2.45 nats and H*_B = 2.45 + ln 4 = 3.83 nats: A saves 2.00 bits a row.
At w = 0.3, H*_A = 0.65 nats and H*_B = ½ ln(0.0833/0.04) = 0.37 nats: B saves 0.42 bits. The crossing is at the
width where the gap between the two segments falls below the water level: a cell needs to be flat only down to
the width the query asks for. Checked numerically (`check_p1.py`, log `check_p1.log`).

Two cautions. The cells of B are far from Gaussian in the second coordinate (two atoms), so a code that spends one
more bit on the sign of that coordinate turns B into the four-cell refinement of both; the example is about the
second-moment code the index uses. And in the index a cell's own shape is not used: the group's basis is (P1.2,
consequence), so the example describes cells against groups, not rows against cells.

### P1.4 The row-to-cell step under the built architecture [model; corrected after the librarian's report]

With one basis and one bit allocation per group, a row's half-width at a level is ‖D_gΔ‖·‖D_g⁻¹e‖, with e the coding
error of its residual r = x − m_c. The code scales each row by its own residual norm (`SYSTEM.md` 2.1, the key gives
the scale), so ‖D_g⁻¹e‖ is the row's residual length in a norm fixed by the group, times a factor that does not depend
on which cell of the group the row is in. Two cases follow, and they differ.

1. *Between two cells of one group.* The row's cost changes through ‖x − m_c‖ in the group's norm, and through the
   query-side factor E‖D_g(q − αm_c)‖, which is nearly the same for neighbouring centres. So within a group the
   coding-optimal rule is nearest centre in the group's dual norm: k-means in a metric. Cells that are flats or
   low-rank Gaussians cannot lower the coding cost by moving rows within a group, because the group's basis, not
   the cell's shape, is what codes them.
2. *Between cells of two groups.* The row changes basis. Its cost in cell c is
   ω(x | c)² = tr(M_g Σ_Q)·‖x − m_c‖²_{M_g⁻¹} with g = a(c) at zero bytes read, and the two norms differ. A row can be
   cheaper in a farther cell whose group's basis fits the direction of its residual. This is the rule of Archer and
   Leen's coding-optimal partition (each vector to the region whose transform coder distorts it least) with the
   certified width in place of the squared error, and it is not k-means in any one metric. It is treated in P3.5.

**What has been measured, exactly.** `groups/REPORT.md` 3.4 (`groups/scripts/prep_m.py`, read) ran k-means in one
metric for the whole corpus, the M⁻¹ of the covariance pooled over all cells, and then grouped the centres by
direction as before. It found under 1% in coordinates read and under 1 point in the sorted range. That tests the
first-order term of P1.2 item 5 with one reference covariance. It does not test case 1 with each group's own norm,
and it does not test case 2 at all. An earlier version of this section said it did; that was wrong.

So what the coding cost can respond to is (i) the number of cells K (P2), (ii) which cells share a basis (P3.1 to
P3.4), (iii) which basis a row is coded in (P3.5), and (iv) state added per cell (P4.3).

### P1.5 The cut: an extent along the anchors [proved as an identity; the Gaussian estimate is a model]

The query's nearest anchor is p̂, β = q·p̂ and ρ = √(1 − β²). The build stores e_c⁺(a) = max_{x∈c} a·x and
e_c⁻(a) = min_{x∈c} a·x for every anchor a and cell c (`SYSTEM.md` 2.1 and 2.2). Since every row has norm at most ν
(1 up to rounding), every score in cell c lies in [β e_c⁻(p̂) − ρν, β e_c⁺(p̂) + ρν]. The rows the cut leaves are

    R_cut(q, t) = Σ_c n_c·1[ β e_c⁻(p̂) − ρν < t ≤ β e_c⁺(p̂) + ρν ].

A cell is undecided over a window of thresholds of length β·ext_c(p̂) + 2ρν, where ext_c(a) = e_c⁺(a) − e_c⁻(a) is the
width of the cell's convex hull along a. So the cut implies two things about a partition, and neither is a
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

### P1.6 The sorted range: a tail count of the quantity whose mean spherical k-means maximises [proved as an identity]

Write θ_x for the angle between a row and its cell's centre direction m̂, φ_c for the angle between q and m̂, and
τ = arccos t. From the key x·m̂ = cos θ_x alone the tightest interval for a unit row is
[cos(φ_c + θ_x), cos(φ_c − θ_x)] (`groups/REPORT.md` 2.1). A row is undecided when |φ_c − τ| < θ_x ≤ φ_c + τ, a contiguous
range of keys, and

    R_range(q, t) = Σ_c #{x ∈ c : |φ_c − τ| < θ_x ≤ φ_c + τ}.

For a cell farther from the query than the threshold angle (φ_c > τ, the common case at small answers) the rows
left are those with θ_x > φ_c − τ: the rows of the cell with the *lowest keys*. Three consequences.

1. Spherical k-means maximises Σ_x x·m̂_c(x), the sum of the keys. The sorted range counts the keys below a level set
   by the query. So this is the one use for which the built objective is the right quantity, up to the difference
   between a mean and a tail count.
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

the k-means objective for the rows mapped by Σ_qq^½. Plain k-means is the case Σ_qq ∝ I. For queries drawn like the
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
| cut | Σ_c n_c·ext_c(a) over anchors a, and the queries' distance to the nearest centre | extent (largest row) | no: a width of the hull, not a variance; the net is a property of K |
| sorted range | Σ_c #{rows with key below the query's level} | tail count of the radius | nearly: the mean of the same quantity |
| coding | Σ_c n_c Σᵢ φ(λ²·βᵢ^(c|g)) plus centres and bases | concave spectral function, between Σσᵢ and Σ ln σᵢ | row to cell within a group: yes, in the group's metric (P1.4); cell to basis and row to basis: no (P3) |
| approximate order | tr(W_P Σ_qq) | variance, in the queries' metric | yes for isotropic queries |
| layout | cells met, cells too thin to open | counts | no: no balance, no purity |

---

## P2. Scaling with the number of cells

### P2.1 The model, and what it is fitted to [model; the exponents measured elsewhere]

**Model U.** Rows are drawn independently from one law, so a cell's covariance depends on K and not on N. Refining
the partition scales every residual direction alike: γᵢ(Σ_c) ∝ K^(−e) for all i, with e = 2/D. (The alternative,
model M, a flat of dimension D whose directions shrink and a remainder that does not, is the classical
high-resolution picture; the data fit U better, below.)

What the exponent is on BioCLIP-2 at 1M [measured, `groups/REPORT.md` 3.2 and `sublinear/REPORT.md` 5.1; exponents
computed here]: the mean square residual falls as K^(−0.106) from K = 1,000 to 58,750 (rms 0.3765 to 0.3034), so
D = 19, the value three neighbour-count estimators give (16 to 22, `sublinear/REPORT.md` 5.4). From K = 31 to 1,000
(subsamples at 1,000 rows a cell, rms 0.507 to 0.377) it falls as K^(−0.17). So the exponent itself falls as K
grows, and D = 19 is the local value at the cell counts the index uses. The residual has n_eff = 72 to 76 in the
dual metric (`../floors/CLAIMS.md` section 7): D counts how the residual scales, n_eff how many directions it
occupies. A cell of 1,000 rows in 19 dimensions is 1,000^(1/19) = 1.4 neighbour spacings across, so it is not a
flat patch at this scale, which is why U and not M.

### P2.2 What a doubling of K is worth to the code [proved in model U; numbers computed from the floors' table]

**Proposition P2.2.** In model U, for every cell and every target half-width w,

    H*(w; Σ_c at 2K cells) = H*(2^(e/2)·w; Σ_c at K cells),

and the saving per doubling of K is (e/2)·λw bits a row, where λ = θ^(−½) is the price at w. λw = k_a + Σ_{σᵢ<√θ} σᵢ/√θ,
with k_a the number of directions above the level, so λw ≥ k_a.

**Proof.** Scaling every γᵢ by c scales every σᵢ by √c, and the programme of Lemma 3 at spectrum cγ and width w is
the programme at γ and width w/√c. By P1.1, ∂H*/∂w = −λ, so ∂H*/∂ ln c = ½λw. Put c = 2^(−e). ∎

So **doubling the number of cells buys exactly what a target width larger by 2^(1/D) buys: 3.7% at D = 19.**
In bytes, from the floor W on the measured spectrum of group 12 (`../floors/CLAIMS.md` section 7; λw is the slope
of nats against ln W between rows of that table) [computed]:

| depth of the read | λw | saving per doubling of K, per row read to that depth |
|---|---|---|
| 0 to 4 B | 125 | 6.6 bits, 0.8 B |
| 4 to 8 B | 150 | 8 bits, 1.0 B |
| 8 to 16 B | 187 | 10 bits, 1.2 B |
| 16 to 32 B | 246 | 13 bits, 1.6 B |
| 32 to 96 B | 365 | 19 bits, 2.4 B |

It cannot exceed (e/2)·n = 41 bits, 5.1 B, at n = 768.

**Against the measurements** [measured elsewhere; the comparison is mine].
- `rust/REPORT_CLUSTERSIZE.md`, bytes per row at 1,000 against 4,000 rows a cell (two doublings, 64 groups in both):
  text 1%, 67.9 → 71.6 B, 1.85 B per doubling against 1.6 to 2.4 predicted at that depth; text 50%, 348.3 → 356.8 B,
  4.3 B per doubling against at most 5.1. (Text 0.01% falls, 19.1 → 17.3 B, because the planner sends fewer cells to
  full vectors; that is P1.8, not the code.)
- `groups/REPORT.md` 3.2, exact coordinates read by a text query as K goes 1,000 → 3,997 → 15,880 → 58,750: 2 to 6% per
  doubling at the 0.01% answer and 0.4 to 2% at 5%. In bytes of the native read (19 and about 220 B) that is 0.4 to
  1.1 B and 0.9 to 4.4 B per doubling; the model gives about 1.2 and at most 5.1. The currencies differ (exact
  coordinates against floor bits), so this is agreement in size only.
- The saving is more than the one bit per doubling that the label can carry against the best code for the true
  law (`groups/REPORT.md` Proposition 1; F5). There is no contradiction: the index codes a cell by its second
  moments, and finer cells of a law that is far from Gaussian are closer to Gaussian. The excess is negentropy
  removed by the partition. `NOTEBOOK.md` records that I first had this wrong.

### P2.3 The cell size for the exact path [model; unit costs measured in `rust/REPORT_CLUSTERSIZE.md`]

Let a query read a share u of the rows to a depth where a doubling of K saves s_B bytes a row (P2.2), and let a
cell cost the query c_K byte-equivalents.

**By bytes.** The cost per row is c_K/S − u·s_B·log₂K + const, least at

    S* = c_K·ln 2 / (u·s_B).

With only the centre charged (c_K = n = 768 B) and u = 1: S* = 220 to 670 rows. With the support table's row
(2K = 1,000 B at K = 500) about double that. So by bytes cells should be several hundred rows for a query that reads
every row, as text queries nearly do.

**By time.** A cell costs 3 µs in the cut and 8 to 17 µs when opened; a row-level costs 10 to 12 ns for 4 to 8
bytes, about 2 ns a byte. So c_K is 5,500 to 10,000 byte-equivalents, 7 to 13 centres, and S* = 1,600 to 8,700 rows
at u = 1. The same from the profiled phases directly: with a the per-cell milliseconds and b the per-row
milliseconds at S₀ = 1,000, and ε the measured growth of the per-row work per doubling of S, the minimum of
a·S₀/S + b·(1 + ε·log₂(S/S₀)) is at S* = S₀·a·ln 2/(b·ε):

| query | a, b (ms) | ε per doubling of S | S* |
|---|---|---|---|
| text 0.01% | 10.3, 20.1 | 0.055 (20.1 → 22.5 → 24.9) | 6,500 |
| text 1% | 16.4, 55.8 | 0.012 (55.8 → 57.2 → 58.5) | 17,000 |
| item 1% | 5.7, 9.6 | 0.15 (9.6 → 12.6 → 15.8), groups rising too | under 2,700 |

[computed from that report's breakdown table.] These agree with what it timed: 4,000 to 16,000 best for text,
about 2,000 to 4,000 for selective item queries. **So the built cell size is set by the per-cell time, not by the
centre's bytes, and by the selective in-distribution query, whose per-row work grows fastest with S because the cut
and the sorted range prune less (P1.5, P1.6).** The code alone accounts for ε of 1 to 3% (s_B over the bytes read);
the rest of the item query's 15% is the cut.

**With N.** The per-cell work grows as K = N/S and the per-row work as N^η, with η the measured exponent of the read
(0.79 to 0.87 for text and 0.49 to 0.60 for items, coordinates read at 12 bases, `sublinear/REPORT.md` 5.3). The
optimum then moves as

    S* ∝ N^(1 − η):   N^(0.13 to 0.21) for text,   N^(0.40 to 0.51) for items.

The built rule is S ∝ N^(0.5): the item queries' exponent. For text the rule starts too small (2,000 against 6,500 or
more at 1M) and grows too fast, and the two meet near 1B (64,000 by the rule; 6,500 to 17,000 times
1,000^(0.13 to 0.21) = 16,000 to 72,000). [model; the exponents are measured over 1.5 decades of N.]

What would move it: a per-cell cost that does not grow with K (a level over the cells for the cut) would make
smaller cells pay; each halving of S is worth 0.8 to 2.4 B a row read.

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
are N/G = 15,600 = 0.026·n², against the κ of 0.5 to 1.1 that counts and timings support
(`hierarchy/layered.md` 8.8 to 8.10). In the terms of P1.3 the balance is the Jensen gap of the group against n²
bytes: a doubling of G was measured to narrow the 16 B level by about 2% and to save 2.2 to 3.7% of code bytes, 0.4 to
2.5 B on a row read to 19 to 68 B, while it adds n²·G/N = 37.7 B a row of setup and storage at this size. The gain
per doubling is the same size as the gain from a doubling of K (P2.2, 0.8 to 2.4 B), at 100 times the fixed cost a
row (37.7 against 0.38 B). So at 1M and n = 768 the theory sides with fewer bases and no fewer cells, which is the
direction of every measurement cited.
---

## P3. The groups: assigning cells, and rows, to a basis by coding cost

### P3.1 The cell-level scheme and what it minimises [proved, unrefereed; rests on `groups/REPORT.md` Proposition 3 and on B4b's sum rule, both proved, unrefereed]

For a metric M ≻ 0 and a cell c let ω_c(M)² = tr(MΣ_Q)·tr(M⁻¹Σ_c), the mean-square zero-read half-width of the cell's
rows under the M-norm certificate. For a grouping a and metrics M₁, …, M_G put Ψ(a, M) = Σ_c n_c ω_c(M_a(c))².
Assume Σ_Q ≻ 0 and every pooled Σ̄_g ≻ 0 (the fit's ridge gives the first).

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
1), and W_P is the average of the Σ̄_g with weights N_g/N. The upper bound on the price is B4b item 3. (5) By (1),
F(Y)² = min_M tr(MΣ_Q)·tr(M⁻¹Y), so where F² is differentiable its gradient at Y is tr(M(Y)Σ_Q)·M(Y)⁻¹ (the envelope
theorem), and the tangent plane at Y evaluated at X is tr(M(Y)Σ_Q)·tr(M(Y)⁻¹X) = ω_X(M(Y))². ∎

**What it is and is not.** It minimises the mean-square width with nothing read, with one norm per group. It
stops at a local minimum (in `check_p3.py`'s second instance, 8 groups could hold the 6 kinds of shape at price 1 and
the scheme stops at 1.13). The structure is that of Lloyd's algorithm and of Banerjee, Merugu, Dhillon and Ghosh's
Bregman clustering [cited, unchecked]; the generator −F² and the object clustered (covariances of cells) are, as far
as the librarian found, not published (`../library/partition.md` 1.5 and 6.1).

### P3.2 The same at a rate [model]

For a level read to a water level θ = λ⁻², replace −F² by −D_λ. D_λ is concave (P1.2), so the Bregman scheme with
generator −D_λ has the same two steps and the same descent, and the centroid of a group is again its pooled
covariance (the mean minimises the sum of Bregman divergences to a point, for any generator). The divergence is
d_λ(Σ_c, Σ̄_g) = Σᵢ [φ(uᵢ) + φ′(uᵢ)(λ²βᵢ^(c|g) − uᵢ)] − D_λ(Σ_c), with uᵢ = λ²γᵢ(Σ̄_g). For a direction above the level its first
part is 1 + ½ ln uᵢ + ½(βᵢ^(c|g)/γᵢ(Σ̄_g) − 1): the code length of the cell's coordinate under the group's Gaussian. So above
the level this is the classification form of a Gaussian mixture over cells, and below it the zero-read scheme of
P3.1. What it charges is the group-wide allocation of P1.3; the built code is between that and the diagonal model.
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
the few groups nearest in centre direction. A cell's own rows must be left out of the pooled covariance it is
compared with, or it is biased towards staying.

### P3.4 What it could gain, from theory and from the measured slope [estimate]

- *An upper limit.* No grouping at G groups beats a basis per cell (P3.1 item 4). Between G = 8 and G = 64 the median
  certified half-width at 16 B narrowed by 6%, about 2% per doubling of G, and the code bytes read fell by 2.2 to
  3.7% per doubling (`hierarchy/layered.md` 8.8) [measured there]. On yahoo's proxy the gain per doubling is about
  constant from one group to one per cell (same file, 8.4). At 2,000 rows a cell and 64 groups a basis per cell is
  three doublings away: at most about 6% in width and 7 to 11% in code bytes [estimate: a straight extrapolation of a
  measured slope, and the slope at G = K is fitted on the rows it codes].
- *What decides the share reached.* If a cell's shape varies smoothly with its position, grouping by centre
  direction is already near the best grouping and the scheme gains little. If shape varies with something else
  (the kind of content, the density), grouping by direction is no better than a random grouping and the scheme
  gains most. The toy of `check_p3.py` shows both: 1.023 → 1.008 when shape follows position, 1.381 → 1.128 when it
  does not. The measured slope says grouping by direction does capture something on yahoo and LAION (more groups
  help), which is evidence for the first case. I put the gain at 1 to 3% of code bytes at 1M [estimate; no
  measurement].
- *Its cost to the other uses.* The cells do not change, so the cut, the sorted range and the order are untouched.
  The groups stop being neighbourhoods. A query's cells are neighbours, and a group opened is n² bytes and 60 to 130
  µs (`hierarchy/layered.md` 8.4 and 8.10); today the share of groups opened is 2 to 2.7 times the share of cells
  read. With groups made by shape that concentration is lost in part, and at G = 64 and n = 768 one more group opened
  costs as much as the code bytes of about 10,000 to 30,000 rows read at 19 to 68 B. So the scheme should carry a
  penalty on the distance between a cell's centre and its group's, or be used only where groups are few (κ near
  1), where almost every group is opened anyway.
- *Relation to the group-count rule.* A better grouping at the same G is worth some number x of doublings of G,
  x = (gain)/(slope per doubling). In the balance (8.1) of `hierarchy/layered.md` it multiplies the rows a basis can
  serve by 2^x: at a 2% gain and a 3% slope, x = 0.7 and κ rises by a factor 1.6. That is its use at scale: the same
  tightness from fewer bases.

### P3.5 The same at the level of rows: each row to the cell where its interval is narrowest [proved for the idealised cost; its fit to the built code is a model]

This is the rule the librarian names as not found in print: Archer and Leen's coding-optimal partition, or the
entropy-constrained encoder, with the certified width as the cost.

**The cost of a row in a cell.** With nothing read, ω(x | c)² = tr(M_g Σ_Q)·‖x − m_c‖²_{M_g⁻¹}, g = a(c). At a level of B
bytes, in the group's dual coordinates z = V_g⁻¹(x − m_c), with sᵢ^g the scale factors of the group's water-filling
at that rate and W_g = Σᵢ sᵢ^g σᵢ(Σ̄_g) its floor width, the idealised cost (a coded coordinate's error is sᵢ times the
coordinate, and the level's weights are the best diagonal ones for the group) is

    ω_B(x | c)² = W_g·Σᵢ sᵢ^g zᵢ²,

a quadratic form in x − m_c with one matrix per group. The levels of the index end at the same byte for every
group, so no price for the rate is needed: the rule is the narrowest interval at the level. (With a rate that
varied by group the rule would add the group's rate to λ times the width, as the entropy-constrained encoder adds
λ times the code length.) A cell's fixed state is paid per cell, not per row, so it sets K and does not enter the
assignment.

**The scheme.** Alternate: (a) each row to the cell of least ω(x | c)²; (b) each centre to the mean of its cell;
(c) each group's metric to that of its pooled covariance; and optionally (d) the cell-level step of P3.1.

**Proposition P3.5.** The objective Σ_x ω(x | c(x))² equals Σ_g N_g F(Σ̄_g)² after step (c), the functional of P3.1, now
over all partitions of the rows into K cells and of the cells into G groups. No step raises it, and the scheme
stops after finitely many steps.

**Proof.** (a) is a minimisation row by row. (b) For a positive semidefinite form the sum over a cell of
(x − m)ᵀA(x − m) is least at the mean. (c) is P3.1 item 1 with the rows of the group as the cells. There are finitely
many assignments. The level-B form is the same with the group's water-filled pair, which attains the floor W_g
for the pooled covariance in the idealised cost. ∎

**How it differs from everything measured.** Cells of two groups may now interleave: a row near one centre can
belong to a farther one whose group's basis fits its residual. The cells are no longer the Voronoi cells of the
centres in any one metric. `groups/REPORT.md` 3.4 did not test this (P1.4).

**What it could gain** [estimate; the one published size is from another kind of data]. It contains the cell-level
scheme as the special case in which whole cells move, so it gains at least as much. Its extra gain is the
difference in shape *within* a cell, which no measurement here speaks to. Archer and Leen report 1.1 to 1.8 dB over
k-means regions at 16 and 32 regions on image blocks, each region with its own transform and allocation
(`../library/partition.md` 1.4) [cited there from the paper]; at equal rate that is an rms error 0.81 to 0.88 of
k-means'. Image blocks have a few sharply different kinds (edges by orientation, textures, flats); whether rows of
an embedding within one cell of 2,000 do is the question the experiment asks first, with one assignment step and no
refit. The toy of `check_p3_rows.py` (two kinds of residual in every blob): rms zero-read width 0.966 of the built
grouping at the cell level and 0.932 at the row level, with 25% of the rows moved and the k-means objective up by
4%; the descent was monotone in 31 steps.

**What it costs the other uses** [argued from P1.5 to P1.7]. A row moved to a farther centre has a lower key, so
it is among the rows the sorted range leaves; it widens its new cell's extent along the anchors; and the centres
explain less of the score variance. In the toy the mean square radius rose 4%. So the row-level scheme trades the
range, the cut and the order against the code, and the experiment must report all four. A price on the key,
ω(x | c)² + ν·(1 − x·m̂_c), moves along that trade; ν → ∞ is the built partition.
---

## P4. The alternatives, each against the five uses

One exchange rate is used throughout [model U, P2.2]: a cell radius larger by a factor ρ costs the code what
D·log₂ρ fewer doublings of K cost. At D = 19 a radius 10% larger is 2.6 doublings, 2 to 6 B a row read to 4 to 96 B.
Any alternative that loosens the cells pays that before it gains anything.

### P4.1 Balanced k-means [model for the distortion; the rest argued from P1]

- *Coding.* Within a group the cost is a distance to the centre (P1.4), so balance can only cost. The classical
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
keys (the sorted range), a larger residual about the one stored centre (the code, at the exchange rate above) and
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

What the family can do here: nothing by moving rows within a group (P1.4 case 1); the cell-level scheme (P3.1);
the row-level scheme (P3.5), which is this family's assignment rule with the index's cost; and state per cell. For
the last, the nearest published design is locally optimised product quantisation, a rotation per cell
(`../library/partition.md` 3.3); at n = 768 a basis per 2,000-row cell is 295 B a row, so only a few directions per
cell are affordable. The index already stores 16 directions per cell for the approximate spread (6.1 B a row,
P2.5). Coding the residual's part along them first, with the group's basis for the rest, is bounded above by the
basis-per-cell limit of P3.4 (about 6% in width) and costs a second transform in the kernel [estimate; not
specified further here].

Verdict: this is where a gain of more than a few percent on the code could be, through P3.5, and the published
size (Archer and Leen, P3.5) is from image blocks.

### P4.4 A second assignment per row [cited evidence; the judgement is argued]

SOAR places a second copy where its residual is orthogonal to the first, so that the two cells' score errors are
uncorrelated. Published: rows read to 95% recall fall by 1.14 times on GloVe-1M at 2,000 cells and by 3.4 and 4.3
times at 10⁹ points with 7.2M cells (`../library/partition.md` 3.2) [cited there, paper read]. The first figure is
the one at this index's cell size. For the exact path a row must be decided once, the build refuses two rows of
one entity, and a copy widens its second cell's extremes and range. So it could only be a list of references used
by the approximate path, at a random read per reference, against the contiguity that P1.8 values.

Verdict: the approximate order only, and about 1.1 times at this cell size by the published figure. Not
recommended.

### P4.5 Hyperplane trees [cited; argued]

Cells from recursive splits are balanced and cheap to route to. With log₂K = 9 to 14 splits in 19 intrinsic
dimensions a cell is bounded in at most 14 directions and loose in the rest, so its radius exceeds a k-means
cell's, and by the exchange rate that costs the code. Verma, Kpotufe and Dasgupta found 2-means trees to have the
lowest quantisation error among the trees they compared (`../library/partition.md` 5.4) [cited there, abstract
only]: the best tree is hierarchical k-means, which is P4.1. Ramaswamy and Rose's hyperplane bounds are a property
of the bound, not of the partition, and the support table is already of that family (same file, 5.3).

Verdict: no use on which it beats k-means; useful only as the way to build a balanced k-means.

### P4.6 Lattice and product partitions [model]

They remove the stored centres, so they allow many more cells for the same state. By P2.3 the centre's bytes are
not what sets the cell size: the per-cell time is, at 7 to 13 centres' worth, and a product partition multiplies
the number of cells that pay it. Their cells are also looser than k-means cells at equal count (a product of two
half-space quantisers; a lattice ignores the density), which the exchange rate charges. An additive form of the
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
  radius-chosen partition within a few percent of k-means on the cut and the range, and worse on the code. The
  experiment reports the ratio of the largest to the rms radius per cell, which tests the 1.32.
- *Cells aligned with a filter's attribute.* If visible sets are slices of an attribute with A values, a
  partition made inside each value meets A times fewer cells for a filtered query (P1.8). For an attribute
  independent of the vectors it spends log₂A of the label's bits on something the code cannot use, which for an
  unfiltered query is log₂A doublings of K lost: 0.8 to 2.4 B a row each (P2.2). For an attribute the vectors
  nearly determine, as a taxon is by an image embedding, k-means has the purity already. The region-filter study in
  `vexact-sc` is the place for this; I have not read it.

### P4.8 The ranking

| | alternative | use it could improve | size | what it costs |
|---|---|---|---|---|
| 1 | rows to the cell of narrowest interval (P3.5) | coding | unknown here; 12 to 19% in rms error on image blocks (Archer and Leen) | lower keys, wider extents, less variance explained; all measurable |
| 2 | cells to bases by coding cost (P3.1) | coding, or fewer bases at the same width | 1 to 3% of code bytes estimated; at most 6% in width | groups stop being neighbourhoods: more setups per query unless penalised |
| 3 | neighbour-graph partition (P4.2) | approximate order, queries like the data | up to 2.3 times at 16 to 256 cells; nothing known at thousands | every exact use; needs k-means leaves under it |
| 4 | balanced k-means (P4.1) | layout | a few percent of time | perhaps the range, slightly |
| 5 | k-means in the queries' metric (P1.7) | approximate order | unknown; a doubling of cells is worth 0.03 of f | the code and the range, at the exchange rate |
| 6 | second assignment (P4.4) | approximate order | 1.14 times at this cell size | contiguity, the exact path's one-row rule |
| 7 | radius-chosen, hyperplane trees, lattice and product | none | | |

Only rows 1 and 3 could beat k-means by more than a few percent on a use, by what is known. Row 3 is not a
partition the exact path can use. Row 1 is untested on embeddings and is the experiment's first question.

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
| Not found: a partition chosen for a certified bound's width or the rows it leaves (6.1) | P1.5 and P1.6 give the two counts as functions of the partition; P3.1 and P3.5 give schemes that descend the width. I have not searched further than the librarian. |
| For exact search the slack of a cell is its covering radius, not its variance (5.1); unbalanced beat balanced as the intrinsic dimension grows (5.2); radius-chosen against variance-chosen has not been compared in hundreds of dimensions (5.5) | P1.5: for the support table the slack is the hull's width along an anchor, which under a Gaussian model is z(n_c) standard deviations, so a variance returns. P1.6: for the key it is the tail of the radius. P4.7 predicts the largest radius at 1.32 times the typical one and a small gap between the two kinds of partition. P4.1 notes the direction of the balance result. |
| Graph partitions read up to 2.3 times fewer candidates; all gains empirical; none at thousands of cells (3.4, 3.6) | P4.2. |
| Error falls as K^(−2/n) or K^(−2/d_int); at about 1,000 cells in 768 dimensions the index is outside the high-resolution regime (2.3, 4.2) | Agreed. P2.1's exponent is a measured local slope (0.106, drifting from 0.17), not Zador's limit, and model U is fitted to it. The floor used in P1 and P2.2 is a reverse water-filling, which is the rate-distortion kind of statement. The one high-resolution formula I use (P4.1) is marked as outside its regime. |
| The √N rule has no published derivation, and no cost model gives K or the number of bases for a certifying index (4.1, 4.4) | P2.3 gives one for K: S* = c_K·ln 2/(u·s_B) and S* ∝ N^(1−η). It reproduces the timed optimum at 1M and the exponent ½ for item queries. P2.4 gives the leaf size. The number of bases is `hierarchy/layered.md` (8.1), which P2.5 and P3.4 restate in terms of the price of a grouping. |
| Gersho's conjecture is unproved above dimension 2 | P4.1 says so where it uses the point density. |

One correction to the report's premise: it takes the cell size as about 1,000 rows (`SYSTEM.md` 2.1). The code's
default is 2,000·√(N/1M) (`rust/tools/rules.py`), so the index does follow a √N rule, and the report's comparison
at 10⁸ rows (4.1) is with the earlier default.

---

## S. The short answer

1. **For three of the five uses k-means is the right kind of objective.** The sorted range counts the low tail of
   the quantity whose mean spherical k-means maximises (P1.6). The approximate order wants the within-cell variance
   small in the queries' metric (P1.7). Within a group the code's cost for a row is its distance to the centre in
   the group's norm (P1.4).
2. **For the cut the quantity is an extent, and for the layout a count; k-means optimises neither, and neither is
   worth much.** The cut's extent is z(n_c) standard deviations under a Gaussian model, so a variance returns
   (P1.5). The layout wants no thin cells (P1.8).
3. **Where k-means is the wrong objective is the basis.** The code's cost is a concave function of a covariance's
   spectrum in the dual metric (P1.1, P1.2): between F = Σσᵢ and the log-determinant, where k-means minimises Σσᵢ². A
   concave cost rewards putting unlike shapes under different bases, which a trace cannot see. The index has that
   freedom only through its groups, and it forms them by the direction of the centres.
4. **Two schemes descend the right objective** (P3): cells to bases by cost, which leaves the cells alone and by
   the measured slope is worth a few percent at most; and rows to the cell of narrowest interval, which is the
   published coding-optimal partition with a certified width as its cost, has a published gain of 12 to 19% on
   image blocks, is untested on embeddings, and pays for its gain in the range, the cut and the order.
5. **The number of cells matters more than the rule that makes them.** A doubling of K is worth a target width
   3.7% larger, 0.8 to 2.4 B a row (P2.2), and the cell size is held up by the per-cell time, not by the centres
   (P2.3). The bases are the largest fixed cost at 1M by a factor of six (P2.5).
6. Of the other alternatives only a neighbour-graph partition has published gains above a few percent, for the
   approximate order alone, and its cells are not ones the exact path can use (P4.2).

## What failed or was withdrawn

- A bound I first wrote: no partition into K cells beats k-means for the code by more than log₂K bits a row. True
  against the best code for the true law, false for the second-moment code the index uses (P2.2, last point).
- P1.4 as first written credited `groups/REPORT.md` 3.4 with testing the within-group rule. It tested one metric for
  the whole corpus. Corrected.
- The balance estimate from the point density formula stands only as an indication (P4.1).
- The gain of the cell-level scheme is an estimate from a slope measured on other datasets (yahoo, LAION), with
  another grouping variable (G); nothing here measures it on BioCLIP-2.
- No claim here is refereed. P1.1, P1.2, P2.2, P3.1 and P3.5 are short proofs resting on F1, on Proposition 3 of
  `groups/REPORT.md` and on B4b, all unrefereed.

## Files

`CLAIMS.md` (this file), `NOTEBOOK.md`, `EXPERIMENT-SPEC.md`; `check_p1.py` and `.log` (P1.1 to P1.3),
`check_p3.py` and `.log` (P3.1, toy), `check_p3_rows.py` and `.log` (P3.5, toy), `check_p4.py` and `.log` (arithmetic of
P2 and P4). Every script is numpy on synthetic data and runs in seconds; none reads a data file.
