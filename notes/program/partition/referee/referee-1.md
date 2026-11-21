# Referee 1: partition track claims

Independent referee report on `../CLAIMS.md`. Verdicts are appended as each is reached. "No error found" means a real attempt to break the claim failed; it does not mean correct.

Scripts and logs for every numerical statement below are in this directory (`ref1_*.py`, `ref1_*.log`): numpy and scipy on synthetic data, seconds each.

## P1.1 The Lagrangian form of the floor: no error found

Checked by hand and numerically (`ref1_p1.py`, 300 random spectra, with ties).

- Theorem 2.24 gives E h ≥ W(H(c)), and Lemma 2.21 gives W as the least value of Σ σᵢsᵢ over s ∈ (0, 1]ⁿ with Σ ln(1/sᵢ) ≤ H. Then H + λ·E h ≥ H + λW(H) ≥ inf over s of Σ [ln(1/sᵢ) + λσᵢsᵢ], and the infimum is Σ φ(λ²γᵢ) with the φ stated. φ is right on both sides of the water level: value 1 and slope ½ from each side at u = 1. Numerically the grid infimum of H + λW(H) equals D_λ to 5·10⁻⁶ and never falls below it.
- The Lagrangian family loses nothing against Theorem 2.24. W is convex and non-increasing (Lemma 2.22 item 1), so the bounds for all λ > 0 together are equivalent to E h ≥ W(H). The claim does not say this; it is true. Numerically H*(w) = max over λ of (D_λ − λw), attained at λ = θ^(−½), to 10⁻¹².
- The second form (H ≥ H*(w)) follows: W is strictly decreasing on [0, ∞) and positive for every finite H.

Unstated hypotheses.
1. Everything Theorem 2.24 assumes, per cell: the residual of the cell is exactly Gaussian with mean zero, the offset Δ is Gaussian with the same Σ_Q for every cell and independent of the residual, and the certificate sees the row only through its message. The proposition says "under the model … of N.0", which covers this, but the index's Δ = q − αm_c depends on the cell, so "one Σ_Q for all cells" is a model and not a fact about the index.
2. H(c) is the entropy of a row's message given its cell. The cell label is free here (it costs storage position, not bytes read). That is why sums of D_λ over cells can fall by more than one bit a row per doubling of K (see P2.2).
3. Zero eigenvalues are harmless (φ(0) = 0; the floor's sums run over the positive γᵢ).

The statement is exactly as strong as Theorem 2.24, no stronger and no weaker. The symbol c is used for both the cell and the message.

## P1.2 Properties of D_λ: no error found in items 1 to 5; the "consequence" is correct after a repair of wording

Items 1 to 5, checked by hand and numerically (`ref1_p1.py`: 400 random instances with singular and non-commuting matrices; finite-difference gradients).

- Item 2. D_λ(Σ) = tr φ(λ²·Σ_Q^½ Σ Σ_Q^½). It is concave and monotone as a function of Σ, in the Loewner order, on the closed positive semidefinite cone, with Σ_Q held fixed. The proof given (Peierls' inequality plus Weyl) is correct. It needs φ concave on [0, ∞) and not only on (0, ∞); it is, by continuity at 0. Nothing is claimed, or true in general, about concavity in Σ_Q or jointly.
- Item 3. Correct as two inequalities between values of D_λ. One hypothesis is hidden, and it matters for how the result is read: D_λ(Σ) is a floor for a cell only when that cell's residual is Gaussian, and a parent and its sub-cells cannot all be Gaussian unless nothing was gained. So the "gain of a split" is a gain in the second-moment model cost and not in any floor that holds for the same data. For a Gaussian parent the true gain is at most the entropy of the sub-cell label (Proposition 1.15), while the model gain is unbounded (the two-segment example gains without limit as w → 0). The claims file says this only later, in P2.2's last bullet. It should be said here.
- Item 4. Correct. Both limits are exact identities beyond a finite price: D_λ = λF once λσ₁ ≤ 1, and D_λ = n(1 + ln λ) + Σ ln σᵢ once λσₙ ≥ 1.
- Item 5. Correct. The hypothesis "γᵢ distinct" is not needed (tr f(B) is differentiable wherever f is C¹ on the spectrum; checked with repeated eigenvalues); "γᵢ positive" is needed. ∇D_λ = (λ/2)·M(Σ)⁻¹ holds exactly for λσ₁ ≤ 1, not only in the limit. So ∇F(Σ) = ½M(Σ)⁻¹ and, by Euler, tr(M(Σ)⁻¹Σ) = F(Σ).

**"k-means in the M⁻¹ norm is the linearisation of the zero-read width": correct, once it says of what.** There are three different statements and the text does not separate them.

1. *One fixed metric M₀ for all cells* (one basis). The mean-square zero-read width is tr(M₀Σ_Q)·tr(M₀⁻¹W_P): exactly the k-means objective in the M₀⁻¹ norm. No linearisation is involved.
2. *One basis refitted to the partition.* The cost is F(W_P) (or N·F(W_P)², P3.1 item 4). F is concave and homogeneous of degree ½, so F(W) ≤ ½F(W₀) + ½tr(M(W₀)⁻¹W) for every W, with equality at W = W₀. So k-means in the M(W₀)⁻¹ norm minimises a majoriser of F(W_P) that touches it at the current partition: one step of a majorise-minimise scheme, which cannot raise F(W_P). This is the reading the cited experiment (`prep_m.py`) tests, for one step. Checked: 40 alternations on a toy never raised F(W_P).
3. *A basis per cell.* Σ_c n_c F(Σ_c) ≤ (N/2)·F(Σ₀) + ½ Σ_x ‖x − m_c‖²_{M(Σ₀)⁻¹}. Again a majoriser, but it touches only if every cell's covariance equals Σ₀. It is a first-order statement about cells whose shapes are all close to one reference, which real cells are not.

So the sentence in the consequence ("to first order about a reference covariance the zero-read cost is k-means in the M⁻¹ norm of that covariance") is right for reading 2 and weak for reading 3. Two further limits: it is the linearisation of the *mean* width F at λ → 0 only; at any positive rate the gradient is the other matrix of item 5, not M⁻¹. And "linearisation" undersells it: because F is concave the linear form is an upper bound, so the step is a descent step and not only a first-order one.

The remark that k-means' cost is linear in Σ_c, so that the second gap of item 3 is identically zero for it, is correct.

One point on the cited measurement. `groups/REPORT.md` 3.4 shows the coordinates read *rose* in every column under the M⁻¹ assignment (text 37.7 → 38.0, …, items 2.8 → 2.9, …), by under 1% for text and by 0.6 to 3.6% for items (the item figure at the 0.01% answer is one unit in the last printed digit). "Under 1%" is right for text only. The zero-read quantity the linearisation is about did move the right way in that table (group ball alone: 98.2 → 94.7 for text, 68.0 → 54.9 for items at the smallest answer). So the measurement agrees with the theory at zero bytes and shows the gain does not survive into the bytes read.

## P1.3 The three cost models and the two-segment example: no error found in the inequalities; the example is correct after two small repairs; one sentence after it is wrong

Ordering (`ref1_p1.py`, 300 random groups): Σ_c (n_c/N_g) D_λ(Σ_c) ≤ Σ_c (n_c/N_g) Σᵢ φ(λ²βᵢ^(c|g)) ≤ D_λ(Σ̄_g) held every time. Both proofs are right (Schur per cell; Jensen per coordinate, since the β average to γ(Σ̄_g)). The identity qᵢaᵢ = βᵢ^(c|g) in the group's dual pair is right, and so is the least value Σ sᵢ√(qᵢaᵢ) over diagonal D.

Three remarks.
- "Three models, in decreasing cost" holds on average over a group, as the displayed inequality says, and not cell by cell: in the random test 396 of 1,240 cells had a diagonal-model cost above D_λ(Σ̄_g). The first inequality (own basis ≤ diagonal) does hold cell by cell.
- D_λ(Σ̄_g) is not the cost of "group-wide allocation" under the same width formula. With the group's allocation s^g and the cell's own widths the cost is Σᵢ [ln(1/sᵢ^g) + λsᵢ^g√βᵢ^(c|g)], which averages to at most D_λ(Σ̄_g) (Jensen on the square root) and is at least the diagonal model. So there are four ordered quantities and D_λ(Σ̄_g) is the top one, an upper bound on the fixed-allocation cost. Checked numerically. This matters in P3.2, which says the −D_λ scheme "charges the group-wide allocation": it charges the tangent plane, which is above the fixed-allocation cost for every cell.
- J_km(P) = Σ_c n_c tr Σ_c is not exactly "the built fit". The fit is spherical k-means (it maximises Σ_c ‖Σ_{x∈c} x‖ = Σ_c n_c‖m_c‖), and for unit rows Σ_c n_c tr Σ_c = Σ_c n_c(1 − ‖m_c‖²). The two differ unless the ‖m_c‖ are equal. See P1.6.

The two-segment example, recomputed by hand and by script. Every displayed number is right: covariances diag(a²/3, 0) and diag(a²/12, h²); traces 0.333 and 0.093 (ratio 3.57); at w = 0.05, 2.446 and 3.833 nats, 2.00 bits; at w = 0.3, 0.655 and 0.367 nats, 0.415 bits; crossing at w = 2h. Sampled covariances agree. Repairs:
1. "Coding prefers A exactly when w < 2h" needs w below both zero-read widths. For w ≥ max(F_A, F_B) both costs are zero and there is a tie (for example h = 0.1, w = 0.6). And the derivation's condition h² ≤ a²/12 is dropped in the bold sentence, which then claims the range w/2 < h < a/2. For a/(2√3) < h < a/2 the conclusion still holds (A is preferred at every w where either cost is positive), but by a different computation: there h is B's larger direction and F_A < F_B, so A is better even with nothing read.
2. The comparison is at a fixed width w. The paragraph before it compares at a fixed price λ. The two can order a pair of cells differently; say which is meant.

**The sentence that is wrong** is the second caution: "in the index a cell's own shape is not used: the group's basis is, so the example describes cells against groups, not rows against cells." In this example the two cells of either partition have the same covariance, so a group that holds both has that covariance as its pooled covariance, and one basis fits both cells exactly. Choosing A or B is then a choice of which rows go to which cell *inside one group with one basis*, and it changes the coding cost by the amounts computed (2 bits a row at w = 0.05). So the example is a counter-example to P1.4 case 1 as worded; see P1.4.

## P1.4 The row-to-cell step: wrong as worded in case 1 (counter-example); correct after a stated repair. The account of the earlier experiment is confirmed.

**The script.** I read `groups/scripts/prep_m.py`. It computes one covariance, pooled over all cells of the base index and all N rows; forms T = Σ_Q^½ V Γ^(−¼) from the eigen-decomposition of Σ_Q^½·cov·Σ_Q^½ (Σ_Q with a ridge of 0.1 of its mean eigenvalue); maps every row by T, so that Euclidean distance in the mapped space is the M⁻¹ norm of that one pooled covariance (TTᵀ = Σ_Q^½B^(−½)Σ_Q^½ = M⁻¹, checked); runs 12 passes of ordinary k-means there on all rows from a fresh random start; then groups the cells by spherical k-means on the directions of their ambient means. So the claim is right: one metric for the whole corpus, groups by direction as before. Two details the claim leaves out: the run starts from a new random initialisation, so its difference from the base includes the difference between two random starts; and it is ordinary k-means (nearest in distance), not spherical.

**Case 1 is wrong as worded.** It says that within a group "the coding-optimal rule is nearest centre in the group's dual norm: k-means in a metric", and that "cells that are flats or low-rank Gaussians cannot lower the coding cost by moving rows within a group". The step that does not follow: the group's norm is treated as fixed, but it is fitted to the pooled within-cell covariance W_g of the group, which moves when rows move. Within one group the zero-read cost is N_g·F(W_g)² (P3.1 item 1), a concave function of W_g, and not a k-means objective in any fixed metric. "Nearest centre in the group's own dual norm" is the fixed-point condition of the alternation (the majorise-minimise step of P1.2), and several partitions satisfy it at different costs.

Counter-example (`ref1_p14.py`): the two-segment data of P1.3 as one group with one basis, Σ_Q = I, a = 1.
- Partition A and partition B are both fixed points of "each row to the nearest centre in the group's dual norm, refit the norm": zero rows move from either, at h = 0.1 and at h = 0.35.
- At h = 0.35 the zero-read cost is F = 0.578 for A and 0.639 for B, while the trace is 0.334 for A and 0.206 for B. So moving rows within the group (B to A) lowers the coding cost with nothing read by 10%, against k-means.
- At h = 0.1 and a target width w = 0.05 the same move saves 2.00 bits a row (P1.3's own figure).

Repair. Replace case 1 by: *for a fixed group metric* the cheapest cell for a row is the nearest centre in that metric; the metric is refitted to the result, the alternation descends N_g·F(W_g)² (this is P3.5 with G = 1 or with rows confined to their group), and its fixed points are partitions that are Voronoi in their own dual norm. A partition of a group into cells that are flat *along the same directions* does lower the cost, because it lowers the pooled covariance's nuclear norm. What a group cannot use is a difference of shape *between* its cells. The same repair is needed in P1.9 (coding row: "row to cell within a group: yes, in the group's metric"), in P4.3 ("nothing by moving rows within a group"), and in S.1, which counts this as one of the three uses for which k-means is "the right kind of objective": it is a fixed-point condition in a metric the partition itself sets, and the objective is concave in the scatter, where k-means' is linear.

Two more unstated hypotheses in case 1.
- "A factor that does not depend on which cell of the group the row is in." The code scales a row by its Euclidean residual norm (`SYSTEM.md` 2.1: the key gives the residual norm), and the error of the scaled code depends on the direction of the residual in the group's coordinates, which changes when the row changes cell. The statement holds only for a quantiser whose error norm is independent of direction.
- The norm is fixed by the group *and the level*. At zero bytes it is M_g⁻¹. At a level it is another quadratic form (P3.5's Σ sᵢzᵢ² in the idealised cost). A row is assigned once and read at several levels, so "the" coding-optimal cell of a row is not defined until the levels are weighted.

Case 2 is consistent with P3.5 and I found no error in it. The paragraph "What has been measured, exactly" is right, with the addition that the experiment is one majorise-minimise step for G = 1 from a new random start.

## P1.5 The cut: no error found in the identity

The interval [βe_c⁻(p̂) − ρν, βe_c⁺(p̂) + ρν] and the count R_cut follow from q = βp̂ + ρu with u a unit vector orthogonal to p̂, and they match `SYSTEM.md` 2.2. Unstated: β ≥ 0 (for β < 0 the two extremes swap). The window length β·ext_c(p̂) + 2ρν is right.

The Gaussian figures are right: the expected largest of n standard normals is 3.241, 3.435, 3.620, 3.966 at n = 1,000, 2,000, 4,000, 16,000 (by quadrature), and the step per doubling is 5 to 6%.

One thing the text does not weigh. The window has two terms, and for any query that is not close to an anchor the second, 2ρν, is the larger by far: it is of order 1 for ρ of 0.5, while β·ext is a few standard deviations of a cell along one direction. The second term does not depend on the cells' shapes at all. So "what the partition should make small" for the cut is first the net (a property of K and of the queries), and the extent is a correction to it. The table in P1.9 lists them the other way round.

## P1.6 The sorted range: no error found in the identity; "spherical k-means maximises the sum of the keys" is not exact for what `fit.py` does

The interval [cos(φ_c + θ_x), cos(φ_c − θ_x)] and the count are right. Small points: the condition should read φ_c − τ ≤ θ_x (not strict) on the side φ_c > τ; the case φ_c + θ_x > π needs the lower end clamped at −1; and "the rows left are those with θ_x > φ_c − τ" drops the upper condition θ_x ≤ φ_c + τ, which can fail only for rows farther from their centre than the query is.

Against the code (`rust/tools/fit.py`, `combined/core.py::kmeans`), "maximises" fails in four ways.
1. `kmeans` runs 12 Lloyd passes from a random start on a sample of min(N, 200,000) rows. It is not run to convergence, and Lloyd's fixed points are local.
2. Every row is then assigned to the sample centroid of largest inner product, and the stored centre is the *mean of the resulting cell*, not the centroid that made the assignment. The key is x·m̂ with m̂ the direction of that stored mean (after 8-bit rounding). For the final partition the stored direction is the best one (the direction of the mean maximises Σ x·m̂ over m̂, up to the rounding). But the partition is not the best one for the stored directions: rows are not at the stored centre of largest key.
3. The stored centre is rounded to 8 bits a dimension, and the key to 16 bits.
4. Emptied centres are reseeded at random in each pass and dropped at the end.

So the built partition is one assignment step and one centre step away from a Lloyd iterate on a sample; it is neither a fixed point nor a maximum. On a synthetic toy made the same way (`ref1_p14.py`: 60,000 unit rows, n = 48, K = 60, 200 sample rows a centre) 11.7% of rows did not have their largest key at their own stored centre, and continuing Lloyd on all rows raised the sum of the keys by 4.7%. That is a toy; the size on the real corpus is not known to me. The exact statement is: the fit runs a fixed number of ascent steps on the sum of the keys, on a sample, and stores the directions that are optimal for the partition it ends with.

A second inexactness is in P1.3 and P1.9, which identify the built objective with Σ_c n_c tr Σ_c. Spherical k-means maximises Σ_c n_c‖m_c‖; the trace objective is N − Σ_c n_c‖m_c‖². They rank partitions alike only when the ‖m_c‖ are nearly equal.

Consequences 2 and 3 are argued, not proved; I found nothing against them. Consequence 1's "the one use for which the built objective is the right quantity" should carry the qualification above.

## P1.7 The approximate order: no error found in the identity

f(q) = qᵀB_Pq / qᵀTq, tr(W_PΣ_qq) = E_q Σ_c p_c σ_c(q)², the reading as k-means on rows mapped by Σ_qq^½, and Σ_qq = T + x̄x̄ᵀ for queries drawn like the data are all right. One distinction is blurred: minimising tr(W_PΣ_qq) maximises the average of the explained *variance* qᵀB_Pq, which is not the average of the *share* f(q) (P5's score S4a is the average share). The two objectives weight queries differently, by qᵀTq. On a toy the two numbers were 0.1523 and 0.1527, so the difference may be small in practice, but the identity is for the first.

## P2.2 What a doubling of K is worth: the identity is correct; "exactly" is exact only for the second-moment model cost; the comparison with the measurements needs two corrections

**The derivation.** No error found (`ref1_p2.py`). Scaling every γᵢ by c gives H*(w; cγ) = H*(w/√c; γ) (checked on 200 spectra to 3·10⁻¹¹), so with c = 2^(−e) a doubling of K equals a target width larger by 2^(e/2) = 2^(1/D), 1.0372 at D = 19. ∂H*/∂w = −λ is right, λw = k_a + Σ_{σᵢ<√θ} σᵢ/√θ is right, and k_a ≤ λw ≤ n. "(e/2)·λw bits" is the first-order figure and an upper bound on the exact saving: the exact saving was 0.86 to 1.00 of it in the random test (median 0.995), because λw falls as w grows. The table's five values of λw (124.4, 150.3, 187.2, 246.5, 364.6) and its savings (0.82, 0.99, 1.23, 1.62, 2.40 B) are right from the W column of the floors' section 7. The cap is n/D = 40.4 bits = 5.05 B; the text rounds it up to 41 and 5.1.

**Recomputed figures.** (1) The table, as above. (2) The exponents: 0.1060 from rms 0.3765 → 0.3034 over K = 1,000 → 58,750, D = 18.9; 0.1706 from 0.507 → 0.377 over K = 31 → 1,000, D = 11.7. Both right.

**Measured, computed, assumed.**
- Measured elsewhere, quoted correctly (I checked the figures against `groups/REPORT.md` 3.2, `sublinear/REPORT.md` line 232 and `rust/REPORT_CLUSTERSIZE.md`): the rms residuals; the bytes per item at 1,000 and 4,000 rows a cell.
- Computed: λw, from a floor evaluated on the spectrum of one *group* (group 12 of an earlier fit, 49,674 rows, with a diagonal Σ_Q in the residual basis and 48 bits of stored scalars charged), not of a cell.
- Assumed: (a) model U itself, that every direction of the dual spectrum shrinks by the same factor. What was measured is the Euclidean trace only. No measurement cited says how the dual spectrum's shape changes with K; the argument for U over M (a cell is 1.4 neighbour spacings across) is an argument. (b) That the cell's covariance depends on K and not on N. The two quoted exponents come from two different experiments: 0.106 varies K at fixed N = 1M, with the rows per cell falling from 1,000 to 17; 0.17 varies N at a fixed 1,000 rows per cell. The text reads the difference as "the exponent falls as K grows". It is equally a difference between shrinking cells and growing corpora, and the data cited cannot tell these apart. (c) That D = 19, measured for K ≥ 1,000 at 1M, holds for K = 250 to 1,000, where the comparison below is made. (d) That a group's spectrum stands for a cell's.

**What the statement is, exactly.** It is an identity about the function H*. H* is a floor for a code only when the cell is Gaussian, and model U cannot hold with Gaussian cells at both K and 2K: the Gaussian entropy of a cell would fall by n₊/D bits per doubling, with n₊ the rank of its covariance (40 bits at full rank, and more than one bit whenever the rank exceeds D), and a label of one bit cannot remove more than one bit of entropy from a density. So P2.2 is a prediction for a code that charges second moments, and is not a floor. The claims file says so in its last bullet ("negentropy removed by the partition"); the tag "[proved in model U]" and the bold "exactly" should say it too: *the second-moment cost of reaching a width falls, per doubling of K, by what a target width 2^(1/D) larger saves.*

**The comparison with the measurements.** Two corrections, from the cited report's own tables.
1. Text 1%: 67.9 → 71.6 B per item is 1.85 B per doubling, as quoted. But the same report's breakdown shows 955k → 979k items entering the levels (the cut and the sorted range prune less). Per item entering, the bytes go 71.0 → 73.1: **1.0 B per doubling**, not 1.85. That is still inside the model's range over depths (0.8 to 2.4 B) but below the "1.6 to 2.4 at that depth" it is set against.
2. Text 50%: of the 8.5 B rise, 5.4 B is the rise in the full-vector share (11.57% → 11.92% of items at 1,536 B). Those are f16 bytes, not code at a rate the floor describes, so "4.3 B per doubling against at most 5.1" compares a quantity with a bound that does not apply to it.

So the agreement is in order of magnitude, as the second bullet there says of its own comparison; the first bullet claims more than it shows.

## P2.3 The cell size: the formulas follow from the stated cost models; one figure is off; one stated assumption contradicts the table it is fitted to

**Formulas.** No error found. c_K/S − u·s_B·log₂(N/S) is least at S* = c_K·ln 2/(u·s_B). a·S₀/S + b(1 + ε·log₂(S/S₀)) is least at S* = S₀·a·ln 2/(b·ε). With per-cell work ∝ N/S and per-row work ∝ N^η the optimum moves as N^(1−η).

**Recomputed figures** (`ref1_p2.py`).
- By bytes: 222 to 665 rows (quoted 220 to 670). With the support table's 1,000 B the factor is 2.3 (511 to 1,532), not "about double".
- By time: 5,500 to 10,000 byte-equivalents, 7.2 to 13.0 centres, S* = 1,588 to 8,664. Right.
- The table. Text 1%: ε = 0.0121, S* = 16,800 (quoted 0.012 and 17,000): right. Item 1%: ε = 0.156 to 0.162, S* = 2,550 (quoted 0.15 and "under 2,700"): right. **Text 0.01%: ε is 0.060, not 0.055** (20.1 → 22.5 → 24.9 is +11.9% and +23.9% over two and four doublings), so S* = 5,950, not 6,500.
- With N: 16,000 to 72,500 at 1B against the rule's 64,000. Right.

**Measured and assumed.** Measured (in `rust/REPORT_CLUSTERSIZE.md`, profiled runs, which that report says run 1.3 to 1.4 times the unprofiled wall): the phase times a and b at three cell sizes, the unit costs. Assumed: u = 1; 2 ns a byte; per-row work linear in log₂S (it is, to the digits given); per-cell work ∝ 1/S; η constant over N and measured at a fixed cell size; the per-cell work ∝ K at every N.

**The assumption the data contradict.** The model takes the per-cell work as a·S₀/S. The table it is fitted to shows the per-cell phases falling much more slowly: text 0.01%, 10.3 → 4.1 → 1.9 ms where 1/S gives 2.6 and 0.6; text 1%, 16.4 → 6.9 → 3.4 where 1/S gives 4.1 and 1.0; item 1%, 5.7 → 2.8 → 1.5 where 1/S gives 1.4 and 0.4. That is S^(−0.5 to −0.6), and the report gives the reason (an opened cell costs 8 to 17 µs at 1,000 rows, 13 to 31 at 4,000, 25 to 61 at 16,000). With the measured exponent p the minimum of a(S/S₀)^(−p) + b(1 + ε·log₂(S/S₀)) is at S₀·(p·a·ln 2/(b·ε))^(1/p): **8,300 for text 0.01%, 53,000 for text 1%, 1,500 for item 1%**, against 6,500, 17,000 and 2,700 quoted. The conclusion in bold survives (text wants larger cells than items, and the per-cell time, not the centre's bytes, sets the size), and the measured totals are flat near the optimum (per-cell plus per-row ms at 1,000 / 4,000 / 16,000 rows: 30.4 / 26.6 / 26.8 for text 0.01%, 72.2 / 64.1 / 61.9 for text 1%, 15.3 / 15.4 / 17.3 for item 1%). But the S* column is not reliable to better than a factor of three, and the "with N" extrapolation inherits that, since it assumes the per-cell work is proportional to K = N/S.

The sentence "the code alone accounts for ε of 1 to 3%" follows from s_B over the bytes of an item that enters the levels; over the bytes per item it would be larger for the item query. I did not find it contradicted.

## P3.1 The cell-level scheme: no error found in items 1 to 5; "local minimum" and "Bregman" each need one qualification

Checked by hand and numerically (`ref1_p3.py`: 40 random instances, 3 to 8 dimensions, 8 to 39 cells with sample covariances, 2 to 5 groups, non-commuting Σ_Q).

- Items 1 and 2: correct. Ψ is linear in each Σ_c, so a group's sum is N_g·tr(MΣ_Q)·tr(M⁻¹Σ̄_g), least at the geometric mean by Proposition 1.13.
- Item 3: correct. Neither step raised Ψ in any of 45 steps. The termination argument is right: after step 1 the value depends on the grouping alone, a change in step 2 lowers it strictly, so no grouping recurs.
- Item 4: correct, no violation in 40 instances, and the price never exceeded the largest number of cells in a group. The upper bound needs F(X + Y) ≤ F(X) + F(Y) and Cauchy–Schwarz, as B4b item 3 has them. B4b is stated for mixtures of *query* families at a fixed Σ_R; here it is used with the two sides exchanged. That is valid because F is symmetric in its two matrices and ω(M; Σ_Q, Σ_R) = ω(M⁻¹; Σ_R, Σ_Q), but the claim should say so.
- Item 5: the identity holds to 2·10⁻¹³, and d(X, Y) = ω_X(M(Y))² − F(X)² is the Bregman expression F(Y)² − F(X)² + ⟨∇F²(Y), X − Y⟩ for the function −F², by Euler's identity for a function homogeneous of degree one. ∇F²(Y) = F(Y)·M(Y)⁻¹ = tr(M(Y)Σ_Q)·M(Y)⁻¹ is right.

**Does each step lower the stated functional?** Yes: Ψ(a, M), the sum over rows of the mean-square half-width of the M-norm certificate with nothing read.

**The fixed point is a local minimum of what?** It is a pair (grouping, metrics) at which each is optimal given the other. If no cell is tied, it is a local minimum of the function of the metrics alone, M ↦ min over groupings of Ψ(a, M). It is *not* a local minimum of the functional of the grouping, Σ_g N_g F(Σ̄_g)², under the smallest change of grouping: in 29 of the 40 instances, moving one cell to another group and refitting both metrics lowered the value from the scheme's fixed point. This is the usual gap between Lloyd's and Hartigan's rules. "It stops at a local minimum" (under "What it is and is not") is stronger than what is shown; "at a fixed point of the alternation" is what is shown, and a pass of single-cell moves with refitting would improve on it.

**Is −F² a valid generator?** As used, yes; in the sense of the paper cited, no.
- −F² is convex and finite on the positive semidefinite cone, and differentiable on the positive definite cone. Those two facts give everything the theorem uses: d ≥ 0, the weighted mean (the pooled covariance) minimises Σ n_c d(Σ_c, Y) over Y, and the descent.
- It is not strictly convex. F² is homogeneous of degree one, so −F² is linear along every ray, and d(tY, Y) = 0 for every t > 0 (checked). d vanishes exactly when X is a multiple of Y (for X ≻ 0). So d is a divergence between *shapes*, covariances up to scale, and a cell's scale enters only through its weight in the pooled covariance. Banerjee, Merugu, Dhillon and Ghosh take a strictly convex generator; their correspondence with exponential families, and uniqueness of the centroid, do not carry over. Here the centroid is unique only up to scale, which is harmless because the metric is.
- The first argument may be singular (a cell with fewer rows than dimensions; d stays finite, checked). The second may not: a group whose pooled covariance is singular has no metric, and every cell outside its range is at infinite cost, so the group can neither be joined nor left sensibly. The theorem assumes every Σ̄_g ≻ 0. That must hold at every step of the alternation, not only at the start, and the alternation can shrink a group (the cited cluster-size report has a group of 624 rows at n = 768).

Unstated hypotheses.
1. One Σ_Q for every cell and group, unchanged by the regrouping (true of the fit as built, where Σ_Q is fitted once).
2. Query and row are paired uniformly, and Δ is uncorrelated with r, so that the mean-square width factors as tr(MΣ_Q)·tr(M⁻¹Σ_c).
3. The M-norm certificate with nothing read needs each row's ‖r‖ in the M_g⁻¹ norm. With nothing read the index has the key, which gives the Euclidean residual norm. So Ψ is the width of a certificate the index does not evaluate at that point; it is a proxy for the first levels. `groups/REPORT.md` 3.4, which the claims cite, is a warning about this proxy: the group ball moved from 68.0 to 54.9% undecided and the coordinates read did not fall.
4. The Σ_c are sample covariances of the rows that are then coded. P3.3 notes the resulting bias and proposes leaving a cell's own rows out of the pooled covariance it is compared with. That variant is not the scheme of the theorem and loses its guarantee: in the same 40 instances it raised the functional in 153 of 609 passes. If it is used, the descent must be checked on held-out rows, not assumed.

P3.2 (not asked, read in passing): the descent with −D_λ is sound for the same two reasons. What it charges a cell is the tangent plane of D_λ at the pooled covariance, which lies above the fixed-allocation cost of P1.3 for every cell (arithmetic–geometric mean, coordinate by coordinate) and equals D_λ(Σ̄_g) on average.

## P3.5 The row-level scheme: no error found in the descent for the idealised cost; "no price term is needed" is sound for one level and not for the assignment as a whole; one inference does not follow

Checked (`ref1_p3.py`): 36 runs of steps (a), (b), (c) at zero read and at two rates, with the level form W_g·Σ sᵢ^g zᵢ². No step raised the objective.

- (a) and (b) are right. (c) at zero read is P3.1 item 1 with rows in place of cells.
- (c) in the level form needs more than the text gives. After (a) and (b) the group's value is N_g·(Σ sᵢqᵢ)(Σ sᵢaᵢ) in the old pair, with aᵢ the new mean squares; this is at least N_g·(Σ sᵢ√βᵢ)², with β the diagonal of the new B(Σ̄_g) in the old eigenbasis (the old dual pair is an orthonormal basis of the whitened space up to scaling of its columns, because Σ_Q is fixed); and Σ sᵢ√βᵢ ≥ W_new(H), because the water-filling value is a symmetric concave function of the spectrum, hence Schur-concave, and a diagonal is majorised by the eigenvalues. I supply this step; the claim's one sentence ("attains the floor W_g … in the idealised cost") asserts it. I also tried 300 random basis pairs, orthogonal and not: none beat the dual pair with water-filling (least ratio 1.000006).
- Termination: "finitely many assignments" needs the tie rule of P3.1 (a row keeps its cell on a tie) and a rule for a cell that loses all its rows; neither is stated here. It also needs every pooled covariance to stay positive definite.
- The proposition says the functional is minimised "over all partitions of the rows into K cells and of the cells into G groups". Steps (a) to (c) never change a cell's group; only the optional step (d) does.

**Is "no price term is needed" sound?** In the sense stated, yes: at one level every group has spent the same bytes (the level ends are global constants in `fit.py`, `DLV` and `RLV`), so the entropy-constrained encoder's rate term is the same for every candidate cell and drops out; and the cell label costs no bytes read. Two things the sentence hides.
1. The cost is a different quadratic form at each level, and at zero read. A row is assigned once and read through several levels. In a toy, 12% of rows had a different cheapest cell at zero read than at a level, from the same start (and none differed between two levels that both had every coordinate above the water level, where the form is fixed up to a factor common to all groups). So an assignment rule needs a choice of level or weights over levels, by how often rows stop at each. Those weights do the work a price would: they trade width early against width late. The claim that no price is needed is true of a level and false of the rule as a whole. (`EXPERIMENT-SPEC.md` may fix a level; the claim does not.)
2. The cost is the *idealised* one, in which a coded coordinate's error is sᵢ times the coordinate. The built code scales a row by its Euclidean residual norm and then uses one table per group, so a coded coordinate's error is closer to a multiple of the row's norm than of the coordinate. Both give a quadratic form per group and level, but not the same form. The claim tags this as a model; it bears on whether descent in the idealised cost is descent in bytes.

**The inference that does not follow.** "It contains the cell-level scheme as the special case in which whole cells move, so it gains at least as much." Containment holds for the *sets* searched, so it orders the global minima. It does not order where two descents stop: each stops at a fixed point, and P3.1's own toy shows the fixed point depends on the start (1.128 from the built grouping, 1.105 from a random one). The statement is true if the row-level scheme is started from the cell-level scheme's result, because no step raises the value. It should say that.

Arithmetic: 1.1 to 1.8 dB is an rms ratio of 0.881 to 0.813, as quoted.

## P4 The ranking: where a stated reason is wrong or meets a cited measurement

1. **"Nothing by moving rows within a group" (P4.3, and S.1, P1.9).** Wrong, by the counter-example under P1.4. Its consequence for the ranking is small, because the row-level scheme of P3.5 includes the within-group moves; but P4.3 and S.1 should not list the within-group step as one where k-means is already the right objective.

2. **The exchange rate at the head of P4** ("a cell radius larger by a factor ρ costs the code what D·log₂ρ fewer doublings of K cost … any alternative that loosens the cells pays that before it gains anything"). The rate comes from model U, in which every direction of the dual spectrum scales together. It prices a *uniform* loosening. Applied to an alternative that changes a cell's shape it is in the wrong metric: the code pays for the dual-norm spectrum, not for the Euclidean radius. The file's own toy is the counter-case: the row-level scheme raised the mean square radius by 4% and *lowered* the rms width by 7% (`check_p3_rows.log`), where the exchange rate charges about half a doubling of K. So the charge is a reason against P4.2, P4.5 and P4.6 only if their cells are looser in the directions the dual metric weights, which the text does not show.

3. **The size given for row 2 (and the "at most" beside it).** It rests on a slope of 2% in width and 2.2 to 3.7% in code bytes per doubling of G, cited from `hierarchy/layered.md` 8.8 for yahoo and LAION (a file I could not open; it is not in this worktree). `rust/REPORT_CLUSTERSIZE.md`, which the claims cite for other figures, has the experiment on this corpus: fits with the same cells and different groupings. From 64 groups to 31 at 4,000 rows a cell, bytes per item rose 6.8% (text 1%) and 5.2% (text 50%), and 3.2% for items at 50%. From 62 groups (a basis per cell) to 7 at 16,000 rows a cell they rose 36%, 29% and 20% for text at 0.01%, 1% and 50%, and 19 to 22% for items at 1% and 50%: 6 to 10% per doubling. These are bytes per item including full vectors, so the currency is not the same, but the slope on BioCLIP-2 at 1M is about twice the one used. That raises both the ceiling for row 2 (three doublings to a basis per cell) and its estimate. It also qualifies P2.5's "the direction of every measurement cited": the cited report's summary says fewer groups are faster at small answers *and read 5 to 27% more bytes*.

4. **The proxy both top rows descend.** Rows 1 and 2 are ranked on schemes whose proved descent is in the zero-read width (P3.1, P3.5). The measurement the file cites for the first-order case shows that quantity moving without the bytes following (`groups/REPORT.md` 3.4: the ball alone 68.0 → 54.9, coordinates read 2.8 → 2.9). The level forms (P3.2, and P3.5 at a level) are the ones that could predict bytes, and they are models. The ranking's reasons should lean on those and say so.

5. **P4.4**, "the first figure is the one at this index's cell size": SOAR's GloVe-1M figure is at 2,000 *cells*, 500 rows a cell; the index at 1M has 500 cells of 2,000 rows. It is the nearest published figure, a factor of four away in cell size.

6. **S.1 and P1.9, "nearly" for the sorted range.** The reason given is that spherical k-means maximises the sum of the keys. For the built fit that is not exact (P1.6): the partition is not at a fixed point for the stored directions. This weakens the reason and does not reverse it.

7. **P1.9 and S.2 on the cut.** The larger term of the window is 2ρν, the net, not the extent (P1.5). The verdict "neither is worth much" for the extent is, if anything, supported by this.

Reasons I checked and found no fault in: the arithmetic of P4.1 (the distortion ratio 1.006, 1.022, 1.105 at D = 19, 10, 5, and the expansion 4·Var(ln f)/(D²(D + 2))); 2.6 doublings for a 10% radius; 295 B a row for a basis per cell; n² as 8,700 to 31,000 rows of code; 1.32 for the largest radius; the e_q values 0.078, 0.071, 0.055 and the 0.03 of f per doubling; P2.5's table.

## Summary: one line per claim

| Claim | Verdict |
|---|---|
| P1.1 | No error found. Exactly as strong as Theorem 2.24 (the Lagrangian family is equivalent to it because W is convex). φ right at and around the water level. |
| P1.2 items 1 to 5 | No error found. Concave and monotone in Σ, Loewner order, closed PSD cone, Σ_Q fixed. "Distinct eigenvalues" not needed in item 5. Item 3's gain is a gain in a model cost, not in a floor. |
| P1.2 consequence | Correct after a repair of wording: k-means in the M⁻¹ norm is a majoriser of F(W_P) touching at the current partition (one basis), exact for a fixed metric, and only a loose bound for a basis per cell; zero read only. |
| P1.3 ordering | No error found, as averages over a group. Not cell by cell. D_λ(Σ̄_g) is an upper bound on the fixed-allocation cost, not that cost. |
| P1.3 example | Every number confirmed. "Exactly when w < 2h" needs w below both zero-read widths; the range "w/2 < h < a/2" is true but outside the derivation's stated case. The second caution after it is wrong. |
| P1.4 | Case 1 wrong as worded (counter-example: the P1.3 example in one group). Correct after the repair: fixed-point condition in a metric the partition sets, objective concave in the group's scatter. The account of `prep_m.py` is confirmed. |
| P1.5 | No error found in the identity (needs β ≥ 0). Gaussian figures confirmed. The 2ρν term dominates the window and is not weighed. |
| P1.6 | No error found in the identity. "Maximises the sum of the keys" is not exact for `fit.py`: 12 passes on a sample, then one assignment, then centres replaced by cell means. |
| P1.7 | No error found. The identity is for the average explained variance, not the average share f. |
| P2.2 | Derivation correct; the table and exponents recompute. It is an identity for the second-moment cost and cannot be a floor at both K and 2K. The comparison with the measurements overstates the agreement (1.0 B, not 1.85, per entering item at text 1%; the text 50% figure is mostly f16 bytes). |
| P2.3 | Formulas follow. ε for text 0.01% is 0.060, not 0.055 (S* 5,950, not 6,500). The assumed 1/S law for per-cell work is contradicted by the table it is fitted to (S^(−0.5 to −0.6)); S* becomes 8,300 / 53,000 / 1,500. Bold conclusion stands. |
| P3.1 | No error found in items 1 to 5. Fixed point is not a minimum over single-cell moves (29 of 40 toys). −F² is convex but not strictly: valid for what the theorem uses, not a generator in Banerjee et al.'s sense. P3.3's leave-out variant loses the descent. |
| P3.5 | No error found in the descent for the idealised cost (level form needs a majorisation step the text omits; supplied). "No price term" sound per level only. "Gains at least as much as the cell-level scheme" does not follow for descents. |
| P4 | Four reasons need changing: within-group moves; the exchange rate in the wrong metric; the slope per doubling of G against the cluster-size report; the zero-read proxy. Two small ones: SOAR's cell size; "nearly" for the range. |

## Not checked

- `NOTEBOOK.md` was not read, as instructed. `EXPERIMENT-SPEC.md` was not read.
- Theorem 2.24, Lemmas 2.21 and 2.22 and Proposition 1.13 were taken as given. B4b's proofs were not refereed; the three facts P3.1 uses from it were checked numerically and by a short argument of my own.
- `hierarchy/layered.md` and `hierarchy/theory.md` are not in this worktree. Every figure cited from them is unchecked: the 2% and 2.2 to 3.7% per doubling of G, 60 to 130 µs per group, κ of 0.5 to 1.1, the 18.5 B first read, c_leaf, the exponents 0.43 to 0.56, A1.7's profile.
- `../library/partition.md`: only the SOAR entry was read. No paper was opened. Archer and Leen, Ma et al., Neural LSH, Gottesbüren et al., Chávez and Navarro and Verma et al. are as the claims quote them.
- `sublinear/REPORT.md`: only the rms line. The exponents η, the dimension estimates 16 to 22 and the slice counts are unchecked.
- The floors' section 7 table was used as printed; its W column was not recomputed from the spectra.
- P1.8, P2.1 beyond its exponents, P2.4, P3.2 to P3.4, P5 and section L were read but not refereed. P2.5's table and P2.4's e_q values were recomputed; nothing else there.
- The claimant's check scripts were not audited. Their logs were read for the quoted toy figures, which match.
- `rust/src/partition.rs` and `rust/src/fit.rs` (the Rust `create` path) were not read; P1.6 is checked against `fit.py` and `combined/core.py` only.
- No real data was touched. The distance of the built partition from a fixed point (P1.6), whether the dual spectrum scales uniformly with K (model U), and whether the built code behaves as the diagonal model or as P3.5's idealised level cost are all open and need the corpus.
