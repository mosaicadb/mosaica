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
covariance. That first-order experiment has been run: assigning rows to the centre nearest in the M⁻¹ norm changed
the coordinates read by under 1% (`groups/REPORT.md` 3.4) [measured there]. So what remains untested is the second
gap, the difference in shape between cells that share a basis, and that is a property of the groups (P3), because
the index has a basis per group and not per cell.

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

### P1.4 The row-to-cell step under the built architecture [model; the measurement cited]

With one basis and one bit allocation per group, a row's half-width at a level is ‖D_gΔ‖·‖D_g⁻¹e‖, with e the coding
error of its residual r = x − m_c. The code scales each row by its own residual norm (`SYSTEM.md` 2.1, the key gives
the scale), so ‖D_g⁻¹e‖ is the row's residual length in a norm fixed by the group, times a factor that does not depend
on which cell of the group the row is in. Moving a row between two cells of one group therefore changes its cost
through ‖x − m_c‖ in that norm, and through the query-side factor E‖D_g(q − αm_c)‖, which is nearly the same for
neighbouring centres. **So within the architecture the coding-optimal row-to-cell rule is nearest centre in the
group's dual norm: k-means in a metric.** That is the experiment of `groups/REPORT.md` 3.4 (under 1% in coordinates
read, under 1 point in the sorted range).

Consequence: cells that are flats or low-rank Gaussians (k-subspaces, mixtures of probabilistic PCA, coding-length
segmentation) cannot lower the coding cost of the index as built by changing which cell a row is in. Their gain needs
a basis or an allocation per cell, which the index does not have. What the coding cost can still respond to is (i)
the number of cells K (P2), (ii) which cells share a basis (P3), and (iii) state added per cell (P4.3).

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
standard normals (3.24, 3.43, 3.61, 3.96 at n = 1,000, 2,000, 4,000, 16,000) [model]. Three differences from the
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
| coding | Σ_c n_c Σᵢ φ(λ²·βᵢ^(c|g)) plus centres and bases | concave spectral function, between Σσᵢ and Σ ln σᵢ | row to cell: yes, in a metric (P1.4); cell to basis: no (P3) |
| approximate order | tr(W_P Σ_qq) | variance, in the queries' metric | yes for isotropic queries |
| layout | cells met, cells too thin to open | counts | no: no balance, no purity |

