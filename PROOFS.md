# Proofs to check

Verbatim statements and proofs from two research agents, copied from their returned text. Context
and notation are in `FINDINGS.md` (same directory). Both agents worked independently and reached
similar floors by a similar route; that is not evidence either is right.

Common notation: an item is x = m + r (m its cluster mean, r the residual); a query offset is
Δ = q − m. A certificate gives, for each item and query, an interval guaranteed to contain q·x; its
half-width is h. "Deterministic" means the interval is always correct. Λ or Σ_R is the residual
covariance; Q or Σ_Q the query-offset covariance.

---

## Agent 1

### Setting

r is the residual, with a density. Δ is the query offset, independent of r. An encoder maps r to a
code word c from a countable set; K_c is the set of residuals with code word c. A sound certificate
gives, for each (c, Δ), an interval containing Δ·r for every r in K_c; h(c, Δ) is its half-width.
Nothing else is assumed of the encoder or certificate.

### Claimed corrections to the original floor (FINDINGS section 5)

- (a) If stored scalars are exact reals, one real can carry all of r (interleave digits), so K_c is
  a point and h = 0. There is no floor.
- (b) Even for well-behaved exact scalars (‖e‖, m·e) the feasible set is a piece of a sphere in a
  hyperplane, with zero volume, so a volume argument says nothing. The fix is to count the
  scalars' bits in B.
- (c) The γ formula assumes Gaussian residuals. The Gaussian has maximal entropy for a given
  covariance, so for real residuals the true floor is lower than the formula.

### Theorem A (volume)

Let Δ = Q^½ g with g standard Gaussian, r' = Q^½ r, S any k-dimensional subspace, h_S the
differential entropy of the projection of r' on S, and H(c) the entropy of the code word
(≤ B ln 2 for any B-bit code, fixed or variable length, scalars included). Then

E[h] ≥ E‖g_k‖ · vol(B_k)^(−1/k) · exp((h_S − H(c))/k).

For Gaussian r with covariance Λ and γ₁ ≥ γ₂ ≥ … the eigenvalues of QΛ, taking S as the top k
eigendirections gives E[h] ≥ κ_k · (γ₁⋯γ_k)^(1/2k) · 2^(−B/k) for every k. Here
κ_k = E‖g_k‖·√(2πe)/vol(B_k)^(1/k) ≥ k (checked numerically for k < 6000; κ_k/k → 1). At the
reverse-water-filling k this is κ_k·√θ, the head term of the FINDINGS floor.

Proof.
1. Soundness gives h(c, Δ) ≥ half the width of K'_c = Q^½K_c in direction g.
2. Write g = (g_S, g_⊥). The support function is convex and E[g_⊥ | g_S] = 0, so by Jensen
   E sup_{x∈K'} g·x ≥ E sup_x g_S·x_S. Projecting on S does not increase mean width.
3. For A ⊂ R^k the Gaussian mean half-width is E‖g_k‖ times half the mean width of conv A, which
   Urysohn bounds below by (vol A / vol B_k)^(1/k).
4. Given c, the projection of r' lies in P_S K'_c, so h(P_S r' | c) ≤ log vol(P_S K'_c).
   Averaging, Σ_c P_c log vol(P_S K'_c) ≥ h_S − I(P_S r'; c) ≥ h_S − H(c).
5. Jensen on exp(·/k). ∎

### Theorem B (concentration)

For Gaussian r', any set T of eigen-coordinates and any encoder:

E[h] ≥ Σ_{i∈T} √γᵢ − (√(2H(c)) + 1)·√(Σ_{i∈T} γᵢ).

Proof. Project on T as in step 2. Let A be the cell in coordinates whitened by Γ^(−½); its standard
Gaussian measure is at least P_c. For any a ∈ A, (Γ^½g)·a ≥ Σ√γᵢ gᵢ² − ‖Γ^½g‖·‖g − a‖, so the
support function is at least Σ√γᵢ gᵢ² − ‖Γ^½g‖·dist(g, A). Gaussian isoperimetry gives
dist(g, A) ≤ √(2 ln(1/P_c)) + Z₊ stochastically, so √(E dist²) ≤ √(2 ln(1/P_c)) + 1. Apply
Cauchy–Schwarz to the product, then Jensen over cells. ∎

### Theorem C (both)

For disjoint S (|S| = k) and T, Gaussian r':

E[h] ≥ (κ_k/√e)·2^(−(B+1)/k)·GM_S(√γ) + Σ_T √γᵢ − (√(2(B+1) ln 2) + 1)·√(Σ_T γᵢ).

Sketch. In each cell keep the points of the S-projection whose T-fibre has at least half the cell's
mass; they have S-mass ≥ P_c/2, hence volume ≥ (P_c/2)/(peak density). Take the S-maximiser there,
then apply Theorem B to its fibre, which is independent of g_S. The 1/√e is the price of the
peak-density bound.

### Claimed consequence

The full expression Σᵢ √min(γᵢ, θ) remains a conjecture at low rates. On the measured spectra, the
best proved bound is 0.70 / 0.73 / 0.79 / 0.83 / 0.93 / 0.96 / 1.00 / 1.00 of it at 0 / 4 / 16 / 32
/ 96 / 192 / 384 / 576 bytes (`agent1/theory.py`, bits include 48 bits of scalars). The baseline
code sits 2.28 / 1.92 / 1.69 / 1.59 / 1.52 / 1.66 / 2.65 / 7.4× above the best proved bound, the
escape code 2.28 / 1.94 / 1.66 / 1.53 / 1.39 / 1.40 / 1.38 / 1.36× (measured on synthetic Gaussian
data with those spectra, 1,500 residuals × 300 offsets). "From 96 B up the floor is proved to
within 7%, and weighted Cauchy–Schwarz on an ideal quantiser meets it."

### Theorem D (instance-optimality for a fixed stream)

Fix the stream; items are encoded independently given the shared means and bases. Let ℓ*(x, q, t)
be the first level at which every unit vector sharing x's prefix lies on one side of t. Any
deterministic algorithm that reads level prefixes and is correct on every corpus reads at least ℓ*
levels of every item. The progressive scheme with the exact range as certificate reads exactly ℓ*.

Proof. If fewer levels are read, some x' with the same prefix lies on the other side of t; swapping
x for x' changes nothing the algorithm sees. ∎

Against algorithms that may stop at any byte, the cost is at most max(c_ℓ/c_{ℓ−1}) times the
minimum: 2 for these level sizes, 3.7 at the final jump to the full vector.

### Sharper constant c (agent 1 §3)

- Exact tail of a coordinate of a uniform unit vector instead of 2·exp(−c²/2): per-block shrink
  factors c'/c, e.g. m = 30: 0.74 / 0.64 / 0.54 at c = 6 / 8 / 10; m = 767: 0.92 / 0.94 / 0.94.
- Several independent blocks: Chernoff bound with the exact moment generating function
  ₀F₁(; m/2; λ²/4).
- Union bound over items that can flip: Pr[fail] ≤ 2·M·L·exp(−c²/2), where M counts items left
  undecided by a certificate that does not depend on the seed. "M must be seed-independent or the
  bound is invalid."

---

## Agent 3a

Notation: a rotated block has n_b dimensions and m_b = n_b − 1; A = ‖y‖·tan φ is the stored error
scale of an item's block; the randomised estimator's error in a block is ⟨E, Δ⟩ with E = A·u.

### T1 (adaptive queries, d = 0)

Let the client's i-th query be any function of its own coins and the earlier answers. Suppose
every (q, t) fixed independently of the seed ω satisfies Pr_ω[R(q,t,ω) ≠ S(q,t)] ≤ ε. Then
Pr[some answer among T adaptive queries is wrong] ≤ T·ε.

Proof. Fix the client's coins. Define the ideal sequence (q*_i, t*_i) by running the client against
the exact oracle S; it does not depend on ω. Let E_i = {R(q*_i, t*_i, ω) = S(q*_i, t*_i)}. On
E_1 ∩ … ∩ E_{i−1} the real answers so far equal the exact ones, so the real i-th query is q*_i. A
first wrong answer at step i therefore implies ¬E_i. Union bound over i. ∎

### T2 (leakage)

If over the whole interaction the client also observes side information taking at most 2^L values,
then Pr[some wrong answer] ≤ 2^L·T·ε. Proof: one ideal sequence per value of the side information;
union bound over 2^L·T fixed queries. ∎ To keep the bound, raise c² to c² + 2·ln2·L.

### T3 (query-time randomness)

Fix the stored bytes read for an item; F is the set of vectors that would store the same bytes. If
min_F q·x′ < t ≤ max_F q·x′ there are two corpora with identical bytes and different correct
answers, so any decoder, randomised or not, errs on one with probability ≥ ½ unless it reads more.

### T4 (class-uniform)

For one rotated block the estimator's error is ⟨E, Δ⟩ with E = A·u and u uniform on the unit sphere
of ŷ^⊥ (true for any codebook: the law is invariant under rotations fixing y). Let W be a
k-dimensional subspace fixed independently of ω. Then with probability ≥ 1 − e^(−c²/2),
simultaneously for every Δ,

|⟨E, Δ⟩| ≤ A·[ ‖P_W Δ‖·(√(k/m_b) + c/√(m_b − 2)) + ‖P_W^⊥ Δ‖ ].

Proof. The sup over unit Δ ∈ W is A·‖P_W u‖, a 1-Lipschitz function of u with E‖P_W u‖² ≤ k/m_b;
concentration on S^(m_b−1) gives the deviation term. Cauchy–Schwarz on the rest. ∎ A union of M
subspaces adds 2·ln M to c². Also claimed: for a class not known at build and a linear decoder,
E_W‖P_W(x − x̂)‖² = (k/n)‖x − x̂‖², so the √k term cannot be avoided.

### T5 (deterministic floor, Gaussian model)

Let r ~ N(0, Σ_R), Δ ~ N(0, Σ_Q) independent, γ_1 ≥ … ≥ γ_n the eigenvalues of
Σ_Q^½ Σ_R Σ_Q^½. Per item the decoder holds a message of entropy ≤ B bits (code and scalars
together), plus S shared bits per group of M_g items. A deterministic certificate must contain Δ·r′
for every r′ with the same message. Then its mean half-width satisfies

E h ≥ max_k E‖g_k‖·ω_k^(−1/k)·√(2πe)·(γ_1⋯γ_k)^(1/2k)·2^(−(B + S/M_g)/k) ≈ max_k k·GM_k(√γ)·2^(−B/k).

Proof. Write Δ = Σ_Q^½ g, r′ = Σ_Q^½ r, and C′ for the hull of the feasible set of r′.
E_g h = E_g h_{C′}(g) ≥ E h_{P_U C′}(g_U) for the top-k eigenspace U (Jensen; support functions
are convex) ≥ E‖g_k‖·(vol_k(P_U C′)/ω_k)^(1/k) (Urysohn). The differential entropy of a variable
supported on a set is at most the log of its volume, and conditioning on a message of entropy H
removes at most H: E log₂ vol_k ≥ h(P_U r′) − B − S/M_g = ½ log₂((2πe)^k γ_1⋯γ_k) − B − S/M_g.
Jensen on 2^(·/k). ∎

"Variable rate, stored scalars, non-convex or lattice cells (an interval sees only the hull) and
shared group information are all inside the statement." Measured against it (`agent3a/exp_c.py`,
global residual basis): floor 0.0978 / 0.0752 / 0.0566 / 0.0240 / 0.0086 / 0.0015 and achieved
0.1373 / 0.1025 / 0.0769 / 0.0338 / 0.0133 / 0.0030 at 32 / 128 / 256 / 768 / 1536 / 3072 bits;
ratios 1.40 / 1.36 / 1.36 / 1.41 / 1.55 / 2.02. The only hole claimed is non-Gaussian structure:
negentropy of J bits per item lowers the floor by 2^(−J/k).

### Sub-bit sketch law

Keep the signs of the first k′ of n_b rotated coordinates. Claimed: tan²φ = π·n_b/(2k′) − 1, so the
block's error has standard deviation ‖y‖‖Δ‖·√(π/(2k′) − 1/n_b), independent of n_b (checked on
synthetic data in `agent3a/exp_d.py`).

### Exact tail

Claimed: a coordinate of a uniform unit vector in R^m has P(|u₁| ≥ s) ≤ 2(1 − s²)^(m/2) for
s ≤ 1/√2 (checked numerically on a grid; exact value an incomplete beta function).
