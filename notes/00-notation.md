# 0. Setting and notation

The sources use the same letter for different things. This chapter fixes one meaning for every symbol used across
chapters, and lists the letters whose meaning depends on the chapter.

## 0.1 The problem

- **Corpus.** N unit vectors (rows) in ℝⁿ. n is the dimension; never write d for it.
- **Query.** A unit vector q, a threshold t, and a tolerance d ≥ 0 (d is never subscripted).
- **Score.** s(x) = q·x. The exact answer is S(t) = {x visible : s(x) ≥ t}. The k-nearest-neighbour answer is the k
  visible rows of largest score; τ_k is the true k-th best score and t̂ the running k-th best during a search.
- **Guarantee.** The returned set R satisfies Pr_ω[S(t) ⊆ R ⊆ S(t − d)] ≥ 1 − ε, over the build's seed ω alone.

## 0.2 Clusters, residuals, offsets

- K clusters, G groups of clusters. Cluster c has centre m_c (m when the cluster is fixed), n_c rows and residual
  covariance Σ_c. A row is x = m + r with residual r.
- The **offset** is Δ = q − αm, with α the centre correction (α = 1 in the floors chapter). Then
  s(x) = q·m + α·m·r + Δ·r, and only Δ·r is uncertain once the key is read.
- Σ_R is the covariance of r and Σ_Q that of Δ, over the group and the query distribution.
- γ₁ ≥ γ₂ ≥ … ≥ γₙ are the eigenvalues of Σ_Q^½ Σ_R Σ_Q^½.
- M = Σ_Q⁻¹ # Σ_R = Σ_Q^(−½)(Σ_Q^½ Σ_R Σ_Q^½)^½ Σ_Q^(−½) is the metric of the dual basis.
- F = Σᵢ √γᵢ. n_eff = F² / Σᵢ γᵢ.

## 0.3 Certificates

- A **certificate** gives an interval of half-width h for Δ·r from what has been read. It is **sound** if the interval
  always holds, and **δ-valid** if it fails with probability at most δ over ω on the side that can mislead.
- h_A(y) = sup over a ∈ A of y·a is the support function of a set A (always subscripted by the set).
- A deterministic level's certificate is |Δ·e| ≤ ‖DΔ‖·‖D⁻¹e‖, with e the coding error and D the level's weights.
- B is a number of bits, H(·) an entropy in nats.

## 0.4 Randomised levels

- L randomised levels, indexed ℓ. Block j has n_j positions and d_j = n_j − 1 degrees of freedom (always subscripted;
  the sources also write m_b).
- y_j is the item's block, R_j its Haar rotation, φ_j the angle between the code and R_j y_j, A_j = ‖y_j‖ tan φ_j the
  stored error scale, a_j = A_j ‖Δ_j‖.
- sd² = Σ_j a_j² / d_j. U_j is one coordinate of a uniform unit vector in ℝ^(d_j).
- T(a; x) = Pr[Σ_j a_j U_j ≥ x]. H_δ(a) is the x with T(a; x) = δ. c* is the largest H_δ(a)/sd over all block shares.
- ε is the failure probability of a whole answer and δ (bare) that of one (item, level) event: δ = ε/(N·L) one-sided.
- c, c_ℓ, c* are confidence constants in units of sd.

## 0.5 Cost laws

- F̄ and f are the survival function and density of a query's scores over the corpus; χ = f/F̄ is the hazard rate (the
  sources write h). t_N is the threshold with F̄(t_N) = k/N. A **shell** is the set of rows within a level's slack w of
  the threshold.

## 0.6 Approximate search

- μ_c = q·m_c. σ_c(q)² = qᵀΣ_c q. ζ_c = (τ_k − μ_c)/σ_c. rad_c² = 1 − ‖m_c‖².
- p_c is the probability that a random row of cluster c is a true neighbour, g_c = n_c·p_c.
- ψ is the caller's multiplier on a spread. A unit with estimate â and spread σ̂ is read further when â + ψσ̂ ≥ t̂.
- δ_q and ρ_q are the query's estimated shift and scale of the visible rows' scores in each cluster's standardised
  units (delta and rho in `adaptive.rs`).
- ρ_{g,ℓ} = ‖a‖₂/‖a‖₁ is the statistical width of group g's level ℓ as a share of its certified half-width.

## 0.7 Letters that change meaning

| Letter | Meaning, by chapter |
|---|---|
| c | a confidence constant (3); a cluster index, as a subscript (5); the stored message of an item (2, written **c** there and declared) |
| κ | κ_k the floor constant E‖g_k‖·√(2πe)·ω_k^(−1/k) (2); κ_j the estimator's stored scale (3); any other use is declared locally |
| ρ | ρ_{g,ℓ} and ρ_q as above (5); a correlation (3, declared); the cut's remainder norm is written ρ_⊥ (1) |
| δ | bare δ is a failure probability; δ_q is the query's shift (5) |
| λ | a price or Lagrange multiplier, declared where used |
| w | a slack (4); a cost in rows (5) |
| g | a standard Gaussian vector (2); g_c above (5) |
