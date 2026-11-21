# 2. Floors on certified width per bit, and instance-optimality

This chapter asks how narrow a sound interval for Δ·r can be when an item is stored in B bits, and
how few bytes a reader of a fixed stream can read and still return the exact answer. Sections 2.1
to 2.5 give the lower bounds on the mean half-width of a deterministic certificate. Section 2.6
says what a grouping or a shared summary can add to the bits an item stores, and section 2.7
proves that reading levels in order until the exact range excludes t is optimal for one stream.
Section 2.8 gives the entropy bound that applies to any index, and section 2.9 sets the codes that
were built against the floors. Section 2.9a proves the full floor of Conjecture 2.14 under the
Gaussian model, with a second floor that keeps Corollary 2.3's constant on the head, and section 2.10
lists the conjectures with what each one lacks.

## 2.0 Local symbols

The symbols of chapter 0 hold, with α = 1, so that x = m + r, Δ = q − m and
s(x) = q·m + m·r + Δ·r. The following are local to this chapter.

- **Whitened coordinates.** Δ = Σ_Q^½ g, so that g = Σ_Q^(−½) Δ, and r′ = Σ_Q^½ r. Then
  Δ·r = g·r′. The eigenvalues γ₁ ≥ … ≥ γₙ of Σ_Q^½ Σ_R Σ_Q^½ (chapter 0) have orthonormal
  eigenvectors v₁, …, vₙ. For a set T of eigen-indices, Γ_T = diag(γᵢ : i ∈ T).
- **Message.** An encoder maps r to a message **c** in a countable set. The message is everything
  stored for the item, stored scalars included. A value of **c** is written c, P_c is its
  probability, K_c = {r : encoder(r) = c} is its cell and K′_c = Σ_Q^½ K_c.
- **Entropies.** H(**c**) is the entropy of the message in nats. For a vector with a density, H(·)
  is its differential entropy in nats, and H_S = H(P_S r′). h₂(p) is the binary entropy of p in
  bits. When the message is B bits of fixed length, or a uniquely decodable code of expected
  length B bits, H(**c**) ≤ B ln 2.
- **Certificate.** A certificate assigns to each (c, Δ) an interval of half-width h(c, Δ). It is
  sound when the interval contains Δ·r for every r ∈ K_c. E h is the mean of h over r and Δ.
- **Geometry.** k is the dimension of a subspace S (in this chapter k never counts neighbours),
  P_S is the orthogonal projection onto S, and ω_k = π^(k/2)/Γ(k/2 + 1) is the volume of the unit
  ball of ℝᵏ. g_k is a standard Gaussian vector in ℝᵏ, with E‖g_k‖ = √2·Γ((k + 1)/2)/Γ(k/2), and
  κ_k = E‖g_k‖·√(2πe)·ω_k^(−1/k) as in chapter 0.
- **Water level.** θ solves Σᵢ ½ log₂⁺(γᵢ/θ) = B, where log₂⁺ = max(log₂, 0).
- **Streams (section 2.7).** An item's stream has L nested levels with cumulative byte sizes
  C₀ < C₁ < … < C_L. 𝓕_ℓ(x) is the set of unit vectors that the encoder maps to the same first ℓ
  levels, in the same cluster, as x. S(q, t) = {x : q·x ≥ t}.
- **Groups (section 2.6).** A partition V₁, …, V_K of space with fixed centres m_j assigns the label
  j to x ∈ V_j. N_g items share a message s of at most B_s bits. Angles to a representative p are
  φ_q = ∠(q, p) and φ_x = ∠(x, p), as in chapter 1's Proposition 1.7.
- **The full floor (section 2.9a).** These symbols hold in section 2.9a, and sections 2.5, 2.10 and
  2.11 use W and V_S as defined here.
  - *Eigen-coordinates.* Σ_Q^½ Σ_R Σ_Q^½ = VΓVᵀ with V = (v₁, …, vₙ) orthogonal and
    Γ = diag(γ₁, …, γₙ), and n₊ is the number of positive γᵢ. For i ≤ n₊,
    zᵢ = γᵢ^(−½) vᵢᵀ Σ_Q^½ r, and z = (z₁, …, z_{n₊}).
  - *Cells.* For P_c > 0, ν_c is the law of z given **c** = c and Z_c is its closed support.
    Z′_c = {(√γᵢ zᵢ)_{i≤n₊} : z ∈ Z_c}, so that h_{Z′_c}(y) is the supremum of Σᵢ √γᵢ yᵢ zᵢ over
    z ∈ Z_c. In section 2.9a Z_c and Z′_c take the place of K_c and K′_c.
  - *Laws and entropies.* N_k is the standard Gaussian law N(0, I_k) on ℝᵏ, for any dimension k,
    and Φ and φ are the standard normal distribution function and density. KL(·‖·) is the relative
    entropy and I(z; **c**) the mutual information, both in nats. H(·) of a law is its differential
    entropy in nats, with values in [−∞, ∞).
  - *Correlations.* For a law μ on ℝ, F_μ is its distribution function, Q_μ its quantile function
    and m₂(μ) its second moment. T_μ = Q_μ∘Φ is the non-decreasing map that sends N₁ to μ, and
    C(μ) = E[g·T_μ(g)] for g ~ N₁. For a law ν on ℝᵏ with a density and an order of the coordinates,
    T^ν is the Knothe–Rosenblatt (triangular) map from N_k to ν: its i-th coordinate is T_μ(gᵢ),
    with μ the conditional law of the i-th coordinate under ν given that the earlier coordinates
    equal T^ν_<i(g_<i). Cᵢ(ν) = E[gᵢ·T^ν_i(g)]. These correlations are unrelated to the byte sizes
    C_ℓ of section 2.7.
  - *Floors, as functions of a rate H ≥ 0 in nats.* W(H) = min{Σᵢ √γᵢ sᵢ : sᵢ ∈ (0, 1],
    Σᵢ ln(1/sᵢ) ≤ H}. For a set T of indices, W_T is the same over i ∈ T only, and W_∅ = 0. For a
    set S of k ≥ 1 indices, with T = S^c its complement in {1, …, n₊},
    V_S(H) = min over 0 ≤ u ≤ H of κ_k·(Π_{i∈S} γᵢ)^(1/2k)·e^(−(H−u)/k) + W_T(u), and V_∅ = W.
    W and V_S are extended to H = ∞ by their limit, which is 0. For comparison only,
    β(s) = ½(s² − 1) − ln s and Ψ(H) = min{Σᵢ √γᵢ sᵢ : sᵢ ∈ (0, 1], Σᵢ β(sᵢ) ≤ H}. ln⁺ = max(ln, 0).
  - Symbols local to one proof of that section (f, τ, 𝒩₁, 𝒩₂, 𝒲₂, T = T_μ in Lemma 2.19, λ, aᵢ, the
    function L of Lemma 2.21's proof, ρ, the set G of Urysohn's inequality, a_c, b_c, u, j) are
    declared where they are used.
- **Symbols local to one proof or section.** In the proof of Theorem 2.6, (σ, τ) are the S- and
  T-coordinates of a point, G is a set of S-coordinates and η > 0 is a small number; chapter 1's
  table step σ, item scale η and chapter 0's group count G do not appear in this chapter. In
  section 2.9, g(b) is the distortion of a b-bit uniform quantiser on a unit Gaussian, as in
  chapter 1 §1.3; elsewhere g is the standard Gaussian vector.

## 2.1 The original claim and its corrections

The first statement of a floor read as follows.

**Claim 2.0 (FINDINGS section 5, "deterministic floor").** A B-bit code confines the residual to a
cell, the bound in direction Δ is the cell's width in that direction, and among bodies of equal
volume the ball has the least mean width (Urysohn). This gives the floor Σᵢ √min(γᵢ, θ), with γᵢ
the eigenvalues of the product of the query-offset and residual covariances. The scalar code was
said to be about 1.2–2× above it.

Status: **[refuted]** as stated, by Proposition 2.1, because B counted only the code bits while the
scheme also stores exact scalars. The source marked it "argued, not proved". The formula, with every
stored bit counted in B and the Gaussian model of Corollary 2.3, survives as Conjecture 2.14, which
Theorem 2.24 proves.
Source: `FINDINGS.md` section 5; `agent1/REPORT.md` section 2; `PROOFS.md`, Agent 1, "Claimed
corrections".

**Proposition 2.1 (claims (a) and (b) of agent 1).**
(a) If a stored scalar is an exact real number, no positive floor holds: one real can carry all of
r, so a cell can be a single point and h = 0.
(b) If the stored exact scalars are the usual ones (‖e‖ and m·e), the cell given the code is
contained in a sphere intersected with a hyperplane, which has Lebesgue measure zero in ℝⁿ. A
volume argument then gives no positive bound.
Status: **[proved, refereed]**. Source: `PROOFS.md`, Agent 1; `proofcheck/REFEREE.md`, verdicts
("Claims (a), (b): Correct").

*Proof.* (a) There is an injection of ℝⁿ into ℝ, for example by interleaving the binary digits of
the n coordinates, so storing the image of r as one real determines r. The cell of each message is
then a point, and the exact value of Δ·r is a sound interval of half-width 0. (b) The residual
differs from a point fixed by the code by the coding error e. Given ‖e‖ and m·e exactly, e lies on
a sphere of radius ‖e‖ about the origin intersected with an affine hyperplane {e : m·e = const}.
The intersection has n-dimensional volume 0, so any floor that grows with the cell's volume is
0. ∎

The repair is to count every stored scalar at its bit width inside B, which is how H(**c**) is
defined above. A real scalar stored in finitely many bits is part of the message, and Theorem 2.2
then covers it.

**Remark (claim (c) of agent 1).** The γ formula assumes Gaussian residuals. Among laws with a
given covariance the Gaussian has the largest differential entropy, so for a non-Gaussian residual
the proved bound of Theorem 2.2 is lower than the Gaussian one by the factor exp(−J/k), where J is
the negentropy of P_S r′ in nats. Status: **[proved, refereed]** as a statement about the bound
(it is Theorem 2.2 with H_S = H_S^Gauss − J). Whether the true optimum is also lower is
Conjecture 2.15. Source: `PROOFS.md`; `proofcheck/REFEREE.md` ("Claim (c): correct for the proved
bound; a gap as a claim about the true optimum"); `proofcheck/CONSOLIDATED_PROOFS.md` section 1,
remarks.

## 2.2 The volume floor

**Theorem 2.2 (Theorem 1 of the consolidated proofs; Theorem A of agent 1; T5 of agent 3a).** Let r
have a law such that, for a k-dimensional subspace S, P_S r′ has a density on S with differential
entropy H_S nats. Let Δ = Σ_Q^½ g with g standard Gaussian and independent of r. For any encoder and
any sound certificate,

E h ≥ E‖g_k‖ · ω_k^(−1/k) · exp((H_S − H(**c**))/k).

Status: **[proved, refereed]**. Source: `proofcheck/CONSOLIDATED_PROOFS.md` section 1;
`proofcheck/REFEREE.md` ("Correct with two small fixes" for Theorem A and T5 together). The
report's section "Fixes to the floor" lists three points: unique decodability of a variable-length
code, independence of the items for shared bits (Corollary 2.4), and the symmetric matrix
Σ_Q^½ Σ_R Σ_Q^½ in place of agent 1's QΛ. See Editor's query 16.

*Proof.*

1. *Width.* Fix c and g. Soundness gives an interval that contains g·y for every y ∈ K′_c, because
   Δ·r = g·r′. So 2h(c, Δ) ≥ h_{K′_c}(g) + h_{K′_c}(−g). Since g and −g have the same law,
   E_g h(c, ·) ≥ E_g h_{K′_c}(g). If K′_c is unbounded the right side is infinite and the theorem
   holds trivially for that cell.
2. *Projection.* Split g = (g_S, g_⊥) along S and its orthogonal complement. The two parts are
   independent and E[g_⊥ | g_S] = 0. The support function is convex, so by Jensen's inequality
   E[h_{K′_c}(g) | g_S] ≥ h_{K′_c}((g_S, 0)) = h_{P_S K′_c}(g_S).
3. *Urysohn.* For a bounded A ⊂ ℝᵏ write g_S = ‖g_S‖·u with u uniform on the unit sphere and
   independent of ‖g_S‖. Then E h_A(g_S) = E‖g_k‖ · E_u h_A(u). The second factor is half the mean
   width of conv A, and Urysohn's inequality bounds it below by
   (vol(conv A)/ω_k)^(1/k) ≥ (vol(A)/ω_k)^(1/k).
4. *Entropy.* Given **c** = c, the vector P_S r′ lies in P_S K′_c almost surely. The uniform law
   maximises differential entropy on a set, so H(P_S r′ | **c** = c) ≤ ln vol(P_S K′_c). Averaging
   over c,
   Σ_c P_c ln vol(P_S K′_c) ≥ H(P_S r′ | **c**) = H_S − I(P_S r′; **c**) ≥ H_S − H(**c**).
5. *Jensen.* Σ_c P_c vol(P_S K′_c)^(1/k) ≥ exp((1/k) Σ_c P_c ln vol(P_S K′_c))
   ≥ exp((H_S − H(**c**))/k). Combining with steps 1–3 gives the theorem. ∎

The proof uses only that the conditional law of r′ given **c** = c is carried by K′_c, so a
certificate required to be sound almost surely, rather than for every r ∈ K_c, satisfies the same
bound. Source: `proofcheck/CONSOLIDATED_PROOFS.md` section 1.

**Corollary 2.3 (Corollary 1 of the consolidated proofs: Gaussian residuals).** If r ~ N(0, Σ_R) and
H(**c**) ≤ B ln 2, then for every k with γ_k > 0,

E h ≥ κ_k · (γ₁⋯γ_k)^(1/2k) · 2^(−B/k).

Status: **[proved, refereed]**. Source: `proofcheck/CONSOLIDATED_PROOFS.md` section 1.

*Proof.* Take S = span(v₁, …, v_k). Then P_S r′ is Gaussian with covariance diag(γ₁, …, γ_k), so
H_S = ½ ln((2πe)ᵏ γ₁⋯γ_k). Substituting into Theorem 2.2 and using
exp(−H(**c**)/k) ≥ 2^(−B/k) gives the bound, since E‖g_k‖·ω_k^(−1/k)·√(2πe) = κ_k. ∎

Σ_Q and Σ_R need not commute, because only the eigenvalues of Σ_Q^½ Σ_R Σ_Q^½ enter. When θ is the
water level for B and k = #{i : γᵢ > θ}, the definition of θ gives (γ₁⋯γ_k)^(1/2k) = √θ·2^(B/k),
so the bound at that k equals κ_k·√θ.

**Remarks on Corollary 2.3.**

- κ_k ≥ k for every k ≤ 200,000, with κ_k/k = 1.649, 1.461, 1.161, 1.027 at k = 1, 2, 10, 100, and
  κ_k/k ≈ (πk)^(1/2k)·(1 − 1/4k). The bound is therefore at least k·√θ, the head term of
  Σᵢ √min(γᵢ, θ). Status: **[computed]** (`proofcheck/constants.py`; agent 1 checked k < 6,000 in
  `agent1/theory.py`). Source: `proofcheck/CONSOLIDATED_PROOFS.md` section 1, remarks;
  `proofcheck/REFEREE.md`, "Other checks". Lemma 2.23 proves κ_k ≥ k for every k.
- In one dimension at high rate an entropy-coded uniform quantiser attains the bound, at a ratio of
  1.0001 for step 0.05σ. No tested code fell below the bound: the referee tried 4,000 random product
  codes, optimised interval partitions and 300 Voronoi or merged non-convex partitions. Status:
  **[computed]** (`proofcheck/toyfloor.py`, `proofcheck/misc.py` (a)). Source: as above.
- On 3,000 random non-commuting pairs (Σ_Q, Σ_R) the referee found a worst ratio of 1.13 between
  achieved width and the floor stated with the eigenvalues of Σ_Q^½ Σ_R Σ_Q^½. Status:
  **[computed]**. Source: `proofcheck/REFEREE.md`, "Fixes to the floor". See Editor's query 11.
- Stored exact real scalars are outside the theorem (Proposition 2.1), so stored scalars must be
  counted at their bit width.
- The theorem assumes a Gaussian offset Δ independent of r. Conjecture 2.16 asks what survives
  without Gaussian offsets.

**Corollary 2.4 (Corollary 2 of the consolidated proofs: shared information).** Let N_g items have
independent residuals. Item i has message **c**ᵢ, and all N_g items share a further message s of
entropy at most B_s bits, which may depend on every item. If the mean over items of H(**c**ᵢ) is at most
B ln 2, then the mean over items of E h satisfies Corollary 2.3 with B replaced by B + B_s/N_g.

Status: **[proved, refereed]**. Source: `proofcheck/CONSOLIDATED_PROOFS.md` section 1;
`proofcheck/REFEREE.md`, "Fixes to the floor" (independence was missing from agent 3a's T5).

*Proof.* In step 4 of Theorem 2.2 the information about item i is I(P_S r′ᵢ; **c**ᵢ, s). Because
the items are independent,
Σᵢ I(r′ᵢ; **c**ᵢ, s) ≤ Σᵢ I(r′ᵢ; **c**₁, …, **c**_{N_g}, s) ≤ I(r′₁, …, r′_{N_g}; **c**₁, …, **c**_{N_g}, s)
≤ Σᵢ H(**c**ᵢ) + H(s).
Step 5 is then applied over items and cells together. ∎

Independence is needed, because when the items are copies of one another a single shared message
informs all of them at once. Chapter 1 (section 1.5) and chapter 4 (Claim 4.9) use this corollary.

## 2.3 The concentration floor

**Theorem 2.5 (Theorem 2 of the consolidated proofs; Theorem B of agent 1).** Let r ~ N(0, Σ_R) and
Δ = Σ_Q^½ g as above. For any set T of eigen-indices, any encoder and any sound certificate,

E h ≥ Σ_{i∈T} √γᵢ − (√(2H(**c**)) + 1/√2) · √(Σ_{i∈T} γᵢ).

Status: **[proved, refereed]**. Agent 1 stated the constant as 1, and the referee showed that 1/√2
suffices. Source: `proofcheck/CONSOLIDATED_PROOFS.md` section 2; `proofcheck/REFEREE.md`
("Theorem B: Correct (the constant 1 can be 1/√2)").

*Proof.* By steps 1 and 2 of Theorem 2.2, E_g h(c, ·) ≥ E h_{K_T}(g_T), where K_T is the projection
of K′_c onto span(vᵢ : i ∈ T). Work in those coordinates and drop the subscript T, so that
r′ = Γ^½ z with Γ = Γ_T and z standard Gaussian. Let A = Γ^(−½) K_T. Since r ∈ K_c implies z ∈ A,
the standard Gaussian measure P of A satisfies P ≥ P_c.

For any a ∈ A,

g·Γ^½ a = (Γ^½ g)·g − (Γ^½ g)·(g − a) ≥ Σᵢ √γᵢ gᵢ² − ‖Γ^½ g‖ · ‖g − a‖.

Letting a approach the nearest point of A to g gives

h_{K_T}(g) ≥ Σᵢ √γᵢ gᵢ² − ‖Γ^½ g‖ · dist(g, A).

Taking expectations and applying Cauchy–Schwarz to the product,

E h_{K_T}(g) ≥ Σᵢ √γᵢ − √(Σᵢ γᵢ) · √(E dist(g, A)²).

By the Gaussian isoperimetric inequality, Pr[dist(g, A) > τ] ≤ Pr[Z > τ − a₀], where Z is standard
normal and a₀ = Φ⁻¹(1 − P) with Φ the standard normal distribution function. If P ≥ ½ then a₀ ≤ 0.
Otherwise P = Pr[Z > a₀] ≤ exp(−a₀²/2), so a₀ ≤ √(2 ln(1/P)). In both cases dist(g, A) is
stochastically at most √(2 ln(1/P_c)) + Z₊, and Minkowski's inequality with E Z₊² = ½ gives
√(E dist²) ≤ √(2 ln(1/P_c)) + 1/√2.

Finally average over cells. Because the square root is concave,
Σ_c P_c √(2 ln(1/P_c)) ≤ √(2 Σ_c P_c ln(1/P_c)) = √(2H(**c**)). ∎

The per-cell inequality was checked on centred and off-centre balls in 5, 50 and 500 dimensions.
Status: **[computed]** (`proofcheck/toyfloor.py` (4)). Source: `proofcheck/CONSOLIDATED_PROOFS.md`
section 2.

Theorem 2.5 is positive only when Σ_T √γᵢ / √(Σ_T γᵢ) exceeds √(2H(**c**)) + 1/√2, so it gives the
tail of the conjectured floor only for codes of few bits. On the two measured spectra it is never
the best bound (section 2.5).

## 2.4 The combined floor

**Theorem 2.6 (Theorem 3 of the consolidated proofs; Theorem C of agent 1).** Let r ~ N(0, Σ_R), let
S and T be disjoint sets of eigen-indices with |S| = k, and let H(**c**) ≤ B ln 2. Then

E h ≥ (κ_k/√e) · 2^(−(B+1)/k) · (Π_{i∈S} γᵢ)^(1/2k) + Σ_{i∈T} √γᵢ
     − (√(2(B+1) ln 2) + 1/√2) · √(Σ_{i∈T} γᵢ).

Status: **[proved, refereed]**. Agent 1 gave only a sketch, and the referee found that it
completes ("Theorem C: Correct; the sketch completes"). Source:
`proofcheck/CONSOLIDATED_PROOFS.md` section 3; `PROOFS.md`, Agent 1, Theorem C.

*Proof.* Project onto the span of the eigenvectors indexed by S ∪ T as before, and write points as
(σ, τ) with σ the S-coordinates and τ the T-coordinates. The S- and T-coordinates of r′ are
independent Gaussians with laws μ_S and μ_T. For a cell K (the projection of K′_c) with probability
P_c, let K_σ = {τ : (σ, τ) ∈ K} and G = {σ : μ_T(K_σ) ≥ P_c/2}.

*Mass of G.* P_c ≤ ∫ μ_T(K_σ) dμ_S(σ) ≤ μ_S(G) + P_c/2, so μ_S(G) ≥ P_c/2. The density of μ_S is at
most (2π)^(−k/2)·(Π_S γᵢ)^(−½), so vol(G) ≥ (P_c/2)·(2π)^(k/2)·(Π_S γᵢ)^(½).

*Splitting.* For η > 0 choose σ* ∈ G, a measurable function of g_S only, with
g_S·σ* ≥ h_G(g_S) − η. Then h_K(g) ≥ g_S·σ* + h_{K_{σ*}}(g_T). The vector g_T is independent of g_S,
and K_{σ*} has μ_T-measure at least P_c/2, so the per-cell bound in the proof of Theorem 2.5 gives
E[h_{K_{σ*}}(g_T) | g_S] ≥ Σ_T √γᵢ − √(Σ_T γᵢ)·(√(2 ln(2/P_c)) + 1/√2).

*S part.* By step 3 of Theorem 2.2,
E h_G(g_S) ≥ E‖g_k‖·(vol(G)/ω_k)^(1/k) ≥ E‖g_k‖·ω_k^(−1/k)·√(2π)·(Π_S γᵢ)^(1/2k)·(P_c/2)^(1/k).

*Averaging.* By Jensen's inequality, Σ_c P_c (P_c/2)^(1/k) ≥ exp(−(H(**c**) + ln 2)/k)
≥ 2^(−(B+1)/k). By concavity of the square root,
Σ_c P_c √(2 ln(2/P_c)) ≤ √(2(H(**c**) + ln 2)) ≤ √(2(B+1) ln 2). Since
E‖g_k‖·ω_k^(−1/k)·√(2π) = κ_k/√e, letting η → 0 gives the theorem. ∎

The factor 1/√e against Corollary 2.3 is the cost of bounding a volume by a peak density, and the
extra bit in B + 1 is the cost of keeping only the half of each cell's mass that lies in G.

## 2.5 How close the proved floors come to the conjectured one

The conjectured floor Σᵢ √min(γᵢ, θ) has a head term k·√θ, which Corollary 2.3 proves up to
κ_k/k, and a tail Σ_{i>k} √γᵢ, which Theorem 2.5 proves only when the code carries few bits. The
best proved bound is the largest of Corollary 2.3, Theorem 2.5 and Theorem 2.6 over k, S and T.

On the two largest groups of the 200k BioCLIP-2 subset (n = 768; group 12 with 49,674 items and
group 2 with 34,396 items; γᵢ = λᵢQᵢ with Qᵢ the query-offset energy per residual coordinate), with
48 bits of scalars added to the code bits, the ratio of the best proved bound to the conjectured
floor for group 12 is as follows. Agent 1's report gives this row without naming a group; it is
group 12 of the log, with the log's 8-byte column left out.

| Code bytes | 0 | 4 | 16 | 32 | 96 | 192 | 384 | 576 |
|---|---|---|---|---|---|---|---|---|
| best proved / conjectured | 0.70 | 0.73 | 0.79 | 0.83 | 0.93 | 0.96 | 1.00 | 1.00 |

Status: **[computed]** (`agent1/theory.py`, `agent1/theory.log`; not re-run by the referee). The
volume bound was the best proved bound in every row, and Theorems 2.5 and 2.6 were never binding.
For group 2 the same log gives 0.72, 0.74, 0.80, 0.84, 0.91, 0.95, 1.00, 1.00 (the editor's
division of the logged columns; see Editor's query 2). Source: `agent1/REPORT.md` section 2; `PROOFS.md`, Agent 1, "Claimed consequence";
`proofcheck/CONSOLIDATED_PROOFS.md` section 6.

For power-law spectra γᵢ ∝ i^(−1.5) the gap almost closes: the best proved bound is 1.027, 1.012,
1.005, 1.004 and 1.001 of the conjectured one at n = 100, 256, 768, 1,024 and 5,000, at both
B = n and B = 4n bits. Status: **[computed]** (`agent1/theory.log`). Source: `agent1/theory.py`.
The source gives no reason why the ratio exceeds 1. The editor's reading is that the proved head
term is κ_k·√θ with κ_k > k, against k·√θ in the conjectured formula, and that on these spectra
this excess outweighs the conjectured tail Σ_{i>k} √γᵢ. A proved bound above the conjectured one
does not contradict the conjecture, because both are lower bounds.

The figures above compare Corollary 2.3 and Theorems 2.5 and 2.6 with the conjectured floor, and
section 2.9a proves that floor for Gaussian residuals (Theorem 2.24). Under the Gaussian model the new
floors contain the three earlier ones. Corollary 2.3 follows from Theorem 2.25 with item 5 of
Lemma 2.22, Theorem 2.5 from Theorem 2.24 with item 4, and Theorem 2.6 from Theorem 2.25 with item 6,
with no added bit and no factor 1/√e (**[proved, unrefereed]**, section 2.9a, "Relation to
Theorems 2.2 to 2.6"). The best proved bound for Gaussian residuals is therefore the largest V_S over
head sets S. With the water-filling head it is at least κ_k·√θ + Σ_{i>k} √γᵢ (Corollary 2.26), which
exceeds the conjectured floor by (κ_k − k)√θ, and this excess is non-negative for every k because
κ_k ≥ k (Lemma 2.23). On the measured spectra the best head set gains 1 to 2 % over the conjectured
floor (**[computed]**, section 2.9a). Theorem 2.2 itself, which covers any residual with a density on
a subspace, does not follow from the new floors.

## 2.6 What a grouping or a shared summary can be worth

Corollary 2.4 bounds a summary that every query reads: B_s bits shared by N_g independent items are
worth at most B_s/N_g bits to each. This section adds three bounds of the same kind, for a
representative, for a grouping and for a summary the query chooses, and one bound on storage.

**Lemma 2.7 (groups report, section 2.1: one representative).** For unit q, x and p, write
q = cos φ_q·p + sin φ_q·u and x = cos φ_x·p + sin φ_x·v with u and v unit vectors orthogonal to p.
Then

q·x = cos φ_q cos φ_x + sin φ_q sin φ_x·(u·v),

and since u·v can take every value in [−1, 1], the tightest interval for q·x from φ_q and φ_x alone
is [cos(φ_q + φ_x), cos(φ_q − φ_x)], of width 2 sin φ_q sin φ_x.

Status: **[proved, unrefereed]** (the groups report states that its section 2 has not been
refereed). Source: `groups/REPORT.md` section 2.1. Proposition 1.7 of chapter 1 is the same result,
stated there with the condition n ≥ 3 (see Editor's query 7).

*Proof.* The identity follows from expanding the inner product, because p ⊥ u and p ⊥ v. As u·v
ranges over [−1, 1] the expression ranges over
[cos φ_q cos φ_x − sin φ_q sin φ_x, cos φ_q cos φ_x + sin φ_q sin φ_x], whose ends are
cos(φ_q + φ_x) and cos(φ_q − φ_x), and their difference is 2 sin φ_q sin φ_x. ∎

The slack is a product of the query's distance from the representative and the member's, so both
must be small for the interval to be useful. The member factor falls only as K^(−0.053) with the
number of groups K (**[measured]**, 1M BioCLIP-2 sample, n = 768, groups report section 3.2), and the
query factor needs a representative near a fresh query.

**Proposition 2.8 (Proposition 1 of the groups report: a grouping is worth at most log₂K bits per
item).** Let x have a density, let V₁, …, V_K partition space with fixed centres m_j, and let
r = x − m_j for x ∈ V_j. Let the offset have the same law Δ = Σ_Q^½ g in every cell, independent of
x, and write x′ = Σ_Q^½ x. For any encoder of r with message **c**, any sound certificate and every
k-dimensional subspace S,

E h ≥ E‖g_k‖ · ω_k^(−1/k) · exp((H(P_S x′) − H(j) − H(**c** | j))/k),  with H(j) ≤ ln K.

Status: **[proved, unrefereed]**. Source: `groups/REPORT.md` section 2.2. Proposition 1.15 of
chapter 1 is the same result, and chapter 4's Claim 4.9 uses it.

*Proof.* Apply Theorem 2.2 within each cell. Given j, r′ is a translate of x′, so
H(P_S r′ | j) = H(P_S x′ | j) = H(P_S x′) − I(P_S x′; j) ≥ H(P_S x′) − H(j). Averaging the
per-cell bounds over j and using the convexity of exp gives the statement. ∎

The label therefore enters the floor exactly as B does, and any K-way grouping, by any rule, lowers
the floor by at most the factor K^(−1/k). The groups report reads the measured radius law
K^(−0.053) as this statement with k ≈ 19. A k-means partition of N ≫ K points fits the hypothesis
up to the dependence of each centre on any one member, which is of the order of one over the group
size. Source: `groups/REPORT.md` section 2.2.

Two measurements show the bound at work on the 1M BioCLIP-2 sample with the 369 text queries and
answer sizes 0.01 / 0.1 / 1 / 5 %. Sixteen times as many groups, which is 4 more bits of label,
saves a text query 4.5 of 45 coordinates read per item and costs 11.4 coordinates per item in
centres. An idealised summary that knows a group's exact extremes in its first 64 local coordinates
leaves 89–100 % of items in undecided groups for text, while the same 64 coordinates held per member
leave 16–72 %. Status: **[measured]**, 0 violations. Source: `groups/REPORT.md` section 1 and
tables 3.1–3.2 (`groups/scripts/`).

**Proposition 2.9 (Proposition 2 of the groups report: summaries the query chooses).** Let the query
choose a = a(Δ) among A read sets, the choice not depending on the data. Under read set a let
w_a(g) be half the width, in direction g, of the set of residuals consistent with everything read,
in the coordinates r′. Let every residual satisfy ‖r‖ ≤ ν, let λ_max(Σ_Q) be the largest eigenvalue
of Σ_Q and let g be standard Gaussian. Then

E w_{a(g)}(g) ≥ min_a E w_a(g) − ν·√(2 λ_max(Σ_Q) ln A),

with expectations over the data and g.

Status: **[proved, unrefereed]**. Source: `groups/REPORT.md` section 2.3. Proposition 1.16 of
chapter 1 is the same result.

*Proof.* Let w̄_a(g) = E_data w_a(g). Each w_a(·) is half the sum of two support functions of a set
inside the ball of radius L = ν‖Σ_Q^½‖ = ν√λ_max(Σ_Q), so it is L-Lipschitz, and so is the average
w̄_a. Gaussian concentration for Lipschitz functions gives, for every real λ,
E exp(λ(E w̄_a − w̄_a(g))) ≤ exp(λ²L²/2), hence E max_a (E w̄_a − w̄_a(g)) ≤ L√(2 ln A). Then, because
a(g) does not depend on the data,
E w̄_{a(g)}(g) ≥ E min_a w̄_a(g) ≥ min_a E w̄_a − L√(2 ln A). ∎

Each min_a E w_a is bounded below by the floor at the rate read under a. The groups report adds a
heuristic: with offsets spread over D directions the correction is the Cauchy–Schwarz slack times
√(2 ln A / D), the cosine of the angle to the nearest of A random anchors, so a table helps only
when ln A is comparable to D (**[model]**). On the 1M BioCLIP-2 sample λ_max(Σ_Q) is 18 % of the trace
of Σ_Q for text queries, so the correction exceeds the slack at A = 1,000 and the proposition says
nothing there (**[measured]**, `groups/scripts/offsets.py`). The proposition does not cover
choices that depend on bits already read. A separate measurement on the 200k subset found that 64
anchors out of 2,000 lower the share of items in undecided clusters at threshold 0.95 only from
25.8 % to 22.1 % (**[measured]**, `agent3b/pair.py`). Source: `groups/REPORT.md` section 2.3;
`agent3b/REPORT.md` section 2.

**Proposition 2.10 (agent 3b: summaries cannot be small).** Suppose item i exceeds every other item
of its cluster by eᵢ in some direction r̂ᵢ. A summary that decides "the cluster maximum is at least
τ" to tolerance eᵢ/2 for every sub-collection of the cluster solves the INDEX problem, so it needs
at least 1 − h₂(δ) bits for each such item, where δ is its error probability.

Status: **[proved, unrefereed]**. The source gives the argument in the one sentence above and calls
the result "proved given a measured property" (see Editor's query 6). Source: `agent3b/REPORT.md`
section 2. Proposition 4.3 of chapter 4 states the same result from the same source.

The property holds widely on the data: 91 % of items have eᵢ > 0.05 and 66 % have eᵢ > 0.1
(**[measured]**, 200k BioCLIP-2 subset, n = 768; `agent3b/REPORT.md` section 2, script not named).
The proposition bounds the storage of a summary, and says nothing about what a query must read.

## 2.7 Instance-optimality for a fixed stream

The floors above compare encoders. This section fixes the encoder and compares readers.

**Setting.** The encoder is a fixed map: the centres, bases, bit allocation and cluster assignment
are fixed parameters, not functions of the corpus. Each item is stored as a stream of L nested
levels with cumulative sizes C₀ < C₁ < … < C_L bytes, and the last level determines x. Define
ℓ*(x, q, t) as the least ℓ such that either q·x′ ≥ t for all x′ ∈ 𝓕_ℓ(x) or q·x′ < t for all
x′ ∈ 𝓕_ℓ(x).

**Theorem 2.11 (Theorem 4 of the consolidated proofs; Theorem D of agent 1; not the integer kernel's
Theorem D, which is chapter 1's Theorem 1.19).** Any deterministic
algorithm that reads level prefixes of items and returns S(q, t) exactly on every corpus reads at
least ℓ*(x, q, t) levels of every item x. An algorithm that reads levels in order and stops when the
exact range of q·x′ over 𝓕_ℓ(x) excludes t reads exactly ℓ* levels.

Status: **[proved, refereed]**, with the referee's two fixes built into the setting: the encoder
must be a fixed map, and ℓ* ranges over the pre-image 𝓕_ℓ(x) in x's cluster. Source:
`proofcheck/CONSOLIDATED_PROOFS.md` section 4; `proofcheck/REFEREE.md`, "Theorem D".

*Proof.* Suppose the algorithm reads ℓ < ℓ* levels of x on some corpus. By the definition of ℓ*,
some x′ ∈ 𝓕_ℓ(x) lies on the other side of t from x. Replace x by x′ in the corpus. The encoder is a
fixed map, so every byte the algorithm reads, of x′ and of every other item, is unchanged, and so is
its output. The correct output differs in whether this item is included, so the algorithm is wrong
on one of the two corpora. The second statement is the definition of ℓ*. ∎

The fixed map is necessary: if the centres and bases are recomputed from the corpus, replacing x
changes what the other items store, and the argument fails. The restriction to x's cluster is also
necessary, because an x′ that the assignment rule would send to another cluster does not give the
same bytes. Source: `proofcheck/REFEREE.md`, "Theorem D".

**Corollary 2.12 (byte-granular readers).** A deterministic reader of the same streams that may stop
at any byte needs more than C_{ℓ*−1} bytes of x. The level reader of Theorem 2.11 therefore costs
at most max_ℓ C_ℓ/(C_{ℓ−1} + 1) times the minimum. For level sizes 0, 4, 8, 16, 32, 48, 96, 192,
384, 576 bytes the factor is below 2 from the second level on and 4 at the first level (1.6 if the
4 scalar bytes are charged at level 0). Where the 1,536-byte full vector is read in addition to the
576-byte prefix the factor is 2112/577 ≈ 3.66.

Status: **[proved, refereed]**. Agent 1 stated the factor as max_ℓ C_ℓ/C_{ℓ−1} and "2 for these
level sizes", and the referee corrected it to the form above. Source:
`proofcheck/CONSOLIDATED_PROOFS.md` section 4; `proofcheck/REFEREE.md`, "Theorem D".

*Proof.* A reader that has read at most C_{ℓ*−1} bytes of x holds a function of the first ℓ* − 1
levels, so Theorem 2.11 applies to it. The level reader reads C_{ℓ*} bytes, and
C_{ℓ*}/(C_{ℓ*−1} + 1) is at most the maximum over ℓ. For the listed sizes the ratios are
4/1 = 4, 8/5, 16/9, 32/17, 48/33, 96/49, 192/97, 384/193 and 576/385, all below 2 after the
first. ∎

**Scope.** Theorem 2.11 compares readers of one stream at d = 0, against the exact-range
certificate. It says nothing about other encodings, and the upper half does not hold for a looser
certificate such as the weighted Cauchy–Schwarz bound the scheme uses. Two measurements bear on that
gap without settling it. Agent 3a found that the exact optimum over the intersection of the
quantiser box and the stored-norm ellipsoid is 3 %, 7 % and 14 % narrower in mean than weighted
Cauchy–Schwarz at 256, 768 and 1,536 bits, with 0 violations on 10,000 (query, item) pairs of the
200k subset (**[measured]**, `agent3a/exp_c.py`, E3). The later code track found that the same
range lowers bytes read by about 0.3 % (**[measured]**, `quant/REPORT.md`, "Contradicts earlier
reports"; see Editor's query 9). Neither intersection is the exact range over 𝓕_ℓ(x). Source:
`proofcheck/CONSOLIDATED_PROOFS.md` section 4; `agent1/REPORT.md` section 5; `agent3a/REPORT.md`,
outcome table; `quant/REPORT.md`.

## 2.8 The entropy bound on any index

**Proposition 2.13 (agent 3b: entropy bound).** Let any index, of any size, answer a query q by
reading bits adaptively and returning a set R with Pr[R ≠ S(q, t)] ≤ ε. Then the expected number of
bits read satisfies

E[bits read] ≥ H(S(q, t) | q) − h₂(ε) − εN,

with entropies in bits. When the N items are independent and each matches with probability p, at
ε = 0 the right side is N·h₂(p), so the share of a dataset stored at 2 bytes (16 bits) per
dimension that must be read is at least h₂(p)/(16n).

Status: **[proved, unrefereed]**. Source: `agent3b/REPORT.md` section 1 (see Editor's query 5).
Theorem 4.1 of chapter 4 states the same bound from the same source and uses it to fix what
"sublinear" can mean.

*Proof (as given in the source, with its steps written out).* For a fixed query the sequence of
bits read determines when the reader stops, so the possible transcripts form a prefix-free code and
E[bits read] ≥ H(transcript | q). The transcript and q determine R, so by Fano's inequality for a
set over N items, H(S | transcript, q) ≤ h₂(ε) + εN, and
H(transcript | q) ≥ I(transcript; S | q) = H(S | q) − H(S | transcript, q). For independent items
with match probability p, H(S | q) = N·h₂(p). ∎

On the 200k subsets with 200 item queries, the bound averaged over queries is 0.00044, 0.0013,
0.0027 and 0.0046 % of the dataset at thresholds 0.95, 0.9, 0.8 and 0.7 for n = 768, and 0.00028,
0.00066, 0.00104 and 0.00133 % for n = 1,024 (**[computed]** from measured match shares,
`agent3b/stats.py`, `agent3b/stats.log`, `agent3b/stats1024.log`). The baseline code reads 0.564,
0.870, 1.868 and 5.336 % at n = 768 (**[measured]**, `agent3b/base.py`), several hundred to over a
thousand times the bound (see Editor's query 4). The source adds that an index of unbounded size
over a finite set of queries attains the bound, so the share of the dataset read has no useful floor
unless the index size is capped (**[sketch: gap named]**: no construction is written down; chapter 4
§4.1 tags the same remark [model], and `06-open-problems.md` lists the difference).

The source draws three suggestions for the metric from this: state a cap on index size, count reads
that every query makes (centres, bases, tables), and report bytes per item beside the percentage,
because code bytes depend on the spectrum while the percentage falls as 1/n by itself. Source:
`agent3b/REPORT.md` section 1.

## 2.9 Distance of the built codes from the floors

Four sets of figures compare a code with the floors. They differ in what was measured and what was
modelled. Only the third computes a certificate's width on real residuals, and the fourth compares
bytes read on real data with a modelled floor.

**Synthetic Gaussian residuals (agent 1).** On synthetic Gaussian residuals with the spectra of
section 2.5, 1,500 residuals × 300 independent Gaussian offsets, scalars counted at 48 bits, the
mean half-width of the deterministic certificate (the smaller of plain and weighted Cauchy–Schwarz)
is the following multiple of the best proved bound, for group 12:

| Code bytes | 0 | 4 | 16 | 32 | 96 | 192 | 384 | 576 |
|---|---|---|---|---|---|---|---|---|
| baseline uniform code | 2.28 | 1.92 | 1.69 | 1.59 | 1.52 | 1.66 | 2.65 | 7.4 |
| escape code | 2.28 | 1.94 | 1.66 | 1.53 | 1.39 | 1.40 | 1.38 | 1.36 |

Status: **[measured]** on synthetic data (`agent1/theory.py`, `agent1/theory.log`); no code beat a
proved floor. Group 2 agrees to within 0.03 for the escape code. Source: `agent1/REPORT.md`
section 2. The best proved bound of this table is that of section 2.5, the largest of Corollary 2.3
and Theorems 2.5 and 2.6, and the table has not been recomputed against Theorems 2.24 and 2.25.

Agent 1 summarised this as "from 96 B up the floor is proved to within 7 %, and weighted
Cauchy–Schwarz on an ideal quantiser meets it". The referee accepted the claim only in a narrow
form: the escape code sits 1.36–1.42× above the proved bound at 96–576 bytes on synthetic Gaussian
residuals with diagonal spectra and independent Gaussian offsets, while the baseline scalar code is
at 1.5–1.7× at 96–192 bytes and 2.7–9× beyond. Source: `proofcheck/REFEREE.md`, "Headline claim".

**A model formula (agent 3a).** Agent 3a compared the expected weighted Cauchy–Schwarz width
Σᵢ √(γᵢ·g(bᵢ)) of the bit allocation, where g(b) is the distortion of a b-bit uniform quantiser on a
unit Gaussian, with Corollary 2.3 in one global residual basis of the 200k subset. Its reported
ratios divided a certificate that uses three stored scalars by a floor that did not count them, and
its quantiser's outer cells are unbounded, so without the scalars h = ∞. The referee's corrected
ratios count 48 scalar bits in the floor:

| Code bits | 32 | 128 | 256 | 768 | 1,536 | 3,072 |
|---|---|---|---|---|---|---|
| floor, 48 scalar bits counted | 0.08506 | 0.06722 | 0.05144 | 0.02237 | 0.00804 | 0.00143 |
| model width | 0.1373 | 0.1025 | 0.0769 | 0.0338 | 0.0133 | 0.0030 |
| ratio as first reported | 1.40 | 1.36 | 1.36 | 1.41 | 1.55 | 2.02 |
| ratio, scalars counted | 1.61 | 1.52 | 1.49 | 1.51 | 1.65 | 2.11 |

Status: **[model]** (`agent3a/exp_c.py`, E4; `agent3a/exp_c.log`). The referee called this the most
important error in the reports, and it concerns the reported numbers alone. Agent 1's uniform column
agrees with the corrected figure (1.52 at 768 bits). Source: `proofcheck/REFEREE.md`, "Most
important error"; `agent3a/REPORT.md`, T5.

**Real residuals, Gaussian offsets (the code track).** The code track computed the mean weighted
Cauchy–Schwarz half-width of the deterministic levels on real residuals, with Gaussian offsets of a
diagonal profile in each group's basis and every stored bit of code, sort key and norms counted.
"Before" is the code before that track's changes, with weights fitted to the item-offset profile Qi
and 8-bit norms, and "after" is the code with mixture weights, the item scale and 4-bit relative
norms (see Editor's query 17). The figures are multiples of the conjectured floor and of
Corollary 2.3, for offsets drawn from the profile named in the first column:

| Index, offset profile | Levels | / conjectured, before → after | / Corollary 2.3, before → after |
|---|---|---|---|
| BioCLIP-2 (n = 768, index p768), profile Qmix | 4–32 B | 1.34–1.37 → 1.17–1.21 | 1.60–1.86 → 1.42–1.66 |
| BioCLIP-2 (p768), profile Qi | 4–32 B | 1.34 → 1.28–1.32 | 1.59–1.83 → 1.58–1.79 |
| BioCLIP-2.5 (n = 1,024, 200k items, n1024) | 4–32 B | 1.30–1.31 → 1.17–1.18 | 1.58–1.78 → 1.44–1.65 |

`quant/REPORT.md` quotes the Qmix rows for BioCLIP-2; the Qi rows are read from the same log.
Chapter 1 §1.7 quotes the Qmix and BioCLIP-2.5 rows.

Status: **[computed]** (`quant/floor2.py`; `quant/logs/floor_p768.log`,
`quant/logs/floor_n1024.log`). At the early levels most of the gap was stored scalars, not quantiser
shape. The source attributes what remains to the per-coordinate scalar quantiser's loss against
the rate–distortion bound (1.13–1.46×) and to the fixed-rate tree code in deep levels (about
1.1–1.25× against entropy coding). It estimates that an E8 lattice could recover about 7 % of width
at depth 3 and beyond (**[conjecture]**, not built). Source: `quant/REPORT.md`, "Distance from the
floor". The residuals are real, so Corollary 2.3 is the Gaussian-model floor for these spectra and
not a proved floor for this data (see the list below and Editor's queries 3 and 12).

The conjectured floor of this table, Σᵢ √min(γᵢ, θ), is now proved for the Gaussian model
(Theorem 2.24). The table's first ratio column therefore measures the code against a floor that is
proved for Gaussian residuals with these spectra, and the "after" code is 1.17–1.32× above it. It is
not a floor for these real residuals, for the reasons listed below. In particular, for residuals that
are not Gaussian the proved bound of Theorem 2.2 falls by the factor exp(−J/k), with J their negentropy
(section 2.1), so a floor for them needs J bounded from above, and the only estimates of J on these
data are lower bounds (`entropy/REPORT.md`, Summary: "Every gap is a lower bound on J").

**Per-item floors on bytes read (agent 3b).** Agent 3b turned the conjectured floor into a share of
the dataset read, by reverse water-filling on γᵢ per group with an ideal quantiser and the prior
treated as free. At n = 768 this gives 0.077, 0.131, 0.364 and 1.378 % at thresholds 0.95, 0.9, 0.8
and 0.7, and the baseline replica is 3.9–7.4× above it; at n = 1,024 the floor is 0.046, 0.049,
0.048 and 0.083 % and the baseline is 7.6–10× above it. The support-table scheme C2 is 3.7–4.3×
above it at n = 768. Status: **[model]** (the source calls these heuristic; `agent3b/stats.py`).
Source: `agent3b/REPORT.md`, headline table and section 1.

**No floor is proved for the real data.** The referee lists five reasons, and each one is a
hypothesis of Corollary 2.3 that the data or the accounting fails:

- the residuals are not Gaussian, so the proved bound falls by exp(−J/k) (section 2.1);
- Σ_Q was taken diagonal in the residual basis instead of using the eigenvalues of
  Σ_Q^½ Σ_R Σ_Q^½;
- the queries are not Gaussian and not independent of the items;
- the shared local bases, about 600 bits per item at 1M (768² × 16 bits per group of about 15,600
  items), are not counted, although Corollary 2.4 says they must be;
- the stored m·e scalar is also information about r.

Source: `proofcheck/REFEREE.md`, "Headline claim".

## 2.9a The full floor, proved

This section proves Conjecture 2.14 under the Gaussian model (Theorem 2.24), and a second floor that
keeps the constant κ_k of Corollary 2.3 on the head while it keeps the tail in full (Theorem 2.25 and
Corollary 2.26). Both proofs couple the standard Gaussian with each cell's own law through a
triangular map. They then use one entropy inequality per coordinate (Lemma 2.20), which is a published
inequality on the line (Lemma 2.19) applied along the map. Lemmas 2.21 to 2.23 are the facts about the
floors and about κ_k that the theorems need.

The source for the whole section is `program/floors/CLAIMS.md`, "For the notes" (N.0 to N.7). The core
referee (`program/floors/referee/referee-core.md`) read the text in which these results were proved
(`program/floors/CLAIMS_before_referees.md`, sections 0 to 4) and found no error in any of them, and an
independent numerical attack (`program/stage0-floor-check/RESULTS.md`) refuted nothing. Clauses added
after the referee read the text are named under the result they belong to, and their list is the
section "Changes after the referees" of `program/floors/CLAIMS.md`.

### Setting

*The model.* Chapter 0's setting holds with α = 1. The residual is r ~ N(0, Σ_R), with mean zero. The
offset is Δ = Σ_Q^½ g, with g a standard Gaussian vector in ℝⁿ independent of r. Σ_Q and Σ_R are
positive semidefinite, and they need not commute or be invertible.

*The message.* **c** takes countably many values. It is a function of r and, if the encoder is
randomised, of randomness independent of Δ, so the pair (r, **c**) is independent of Δ.
P_c = Pr[**c** = c]. If the message is B bits of fixed length, or a uniquely decodable code of expected
length B bits, H(**c**) ≤ B ln 2.

*The certificate.* For each value c and each Δ the certificate gives an interval of half-width
h(c, Δ) ≥ 0, and three things are assumed of it.

1. It depends on the item only through c and on the query only through Δ.
2. h(c, ·) is a measurable function of Δ.
3. It is sound: for every c with P_c > 0 and every Δ, the interval contains Δ·r for almost every r
   under the law of r given **c** = c. This is weaker than the soundness of section 2.0, which asks
   it for every r ∈ K_c, so a floor proved under it holds under that definition too.

E h is the mean of h(c, Δ) over the message and the offset, and by independence
E h = Σ_c P_c E_g h(c, Σ_Q^½ g).

Hypotheses 1 and 2 were stated after the core referee read the text, at the referee's request: its
report lists them among the unstated hypotheses (`program/floors/referee/referee-core.md`, "Unstated
hypotheses", items 3 and 1; `program/floors/CLAIMS.md`, "Changes after the referees").

*Eigen-coordinates.* With z as in section 2.0, z ~ N_{n₊} and z is a function of r. Replace g by
Vᵀg, which is again standard Gaussian and independent of z and **c**. Then

Δ·r = Σ_{i≤n₊} √γᵢ gᵢ zᵢ   almost surely,

because the components of Σ_Q^½ r along the eigenvectors of eigenvalue 0 have variance 0. Every sum
or product over i in this section runs over i ≤ n₊ unless it names another range.

*Cells.* For P_c > 0, P_c ν_c ≤ N_{n₊} as measures, so ν_c has a density and
KL(ν_c ‖ N_{n₊}) ≤ ln(1/P_c). The mutual information is I(z; **c**) = Σ_c P_c KL(ν_c ‖ N_{n₊}), and
I(z; **c**) ≤ H(**c**).

*The width of a cell.* Fix c with P_c > 0 and fix Δ. A linear function of z is continuous, so its
essential supremum under ν_c is its supremum over Z_c. Soundness therefore gives
2h(c, Δ) ≥ h_{Z′_c}(g) + h_{Z′_c}(−g), and since g and −g have the same law,

E_g h(c, ·) ≥ E_g h_{Z′_c}(g).   (★)

This is step 1 of Theorem 2.2. h_{Z′_c} is a supremum of linear functions, so it is Borel and
dominates a linear function, and its mean is well defined in (−∞, ∞].

### Lemmas

**Lemma 2.19 (Lemma 1 of the floors track: entropy against the Gaussian correlation of the quantile
function).** Let μ be a law on ℝ with a density f and a finite second moment. Then
0 < C(μ) ≤ √Var(μ) and

H(μ) ≤ ln(√(2πe)·C(μ)),   equivalently   KL(μ ‖ N₁) ≥ ½ m₂(μ) − ½ − ln C(μ).

Equality holds when μ is Gaussian.

Status: **[cited]**. It is the case n = 1 of the dimensional Talagrand inequality for the Gaussian
measure: Bolley, Gentil and Guillin, "Dimensional improvements of the logarithmic Sobolev, Talagrand
and Brascamp–Lieb inequalities", Ann. Probab. 46 (2018), Theorem 2.1 at n = 1 (arXiv:1507.01086).
Bakry, Bolley and Gentil first proved it, by semigroup methods (Probab. Theory Related Fields 154,
2012). The librarian read the paper's LaTeX source (`program/library/floors-novelty.md` section 1), and
no one else in the program opened the paper's text: the core referee confirmed only the arXiv record,
and the author of the source checked only that the theorem as the librarian quotes it reduces to this
lemma. The proof is kept so that the chapter can be read without the paper, and the core referee found
no error in it. The falsifier tested a consequence of the second form, C(μ) ≥ s with s ∈ (0, 1] and
β(s) = KL(μ ‖ N₁) (it follows with m₂ ≥ C²), over seven families of laws, and refuted nothing (closest
approach 1.00000000007; **[computed]**). Source: `program/floors/CLAIMS.md`, N.1;
`program/floors/referee/referee-core.md`, stage 1; `program/stage0-floor-check/RESULTS.md`, section 1.

With the standard Gaussian, their theorem at n = 1 reads
½𝒲₂²(N₁, μ) ≤ ½(m₂ − 1) + 1 − exp(½(m₂ − 1) − KL(μ ‖ N₁)), where 𝒲₂ is the quadratic Wasserstein
distance. On the line the monotone coupling is optimal, so 𝒲₂² = m₂ + 1 − 2C(μ), and the inequality
becomes C(μ) ≥ exp(½(m₂ − 1) − KL), which is the second form. Their proof is the one below
(transport, Jensen, Gaussian integration by parts).

*Proof.* Write T = T_μ and g ~ N₁.

1. *T(g) has law μ, and E T(g)² = m₂.* The quantile transform of a uniform variable has law μ.
2. *C = ∫ φ dτ*, where τ is the Lebesgue–Stieltjes measure of the non-decreasing function T. For
   x > 0, T(x) − T(0) = τ([0, x)) and x·φ(x) ≥ 0, so by Tonelli
   E[g(T(g) − T(0)); g > 0] = ∫_{[0,∞)} τ(dy) ∫_{x>y} xφ(x) dx = ∫_{[0,∞)} φ dτ. The half-line g < 0
   is the same with both factors negative, and E[g·T(0)] = 0. C is finite by Cauchy–Schwarz,
   C = E[g(T(g) − E T(g))] ≤ √Var(μ), and positive because μ is not a point mass.
3. *C ≥ E T′(g).* T is differentiable almost everywhere and τ = T′(x)dx + τ_s with τ_s ≥ 0 singular
   (Lebesgue).
4. *H(μ) = ½ ln(2πe) + E ln T′(g).* F_μ is absolutely continuous with F_μ′ = f off a Lebesgue-null
   set 𝒩₁. Then μ(𝒩₁) = 0, so Pr[T(g) ∈ 𝒩₁] = 0. T is differentiable off a null set 𝒩₂. F_μ is
   continuous, so F_μ(T(x)) = Φ(x) for every x. For x outside 𝒩₂ ∪ T⁻¹(𝒩₁) the chain rule gives
   f(T(x))·T′(x) = φ(x). So T′(x) > 0 and ln T′(x) = ln φ(x) − ln f(T(x)) for almost every x. Take
   expectations: E ln φ(g) = −½ ln(2πe), and E[−ln f(T(g))] = −∫ f ln f = H(μ), which is well defined
   in [−∞, ∞) because f ln(1/f) ≤ φ − f + f·(½ ln 2π + y²/2).
5. *Jensen.* E ln T′(g) ≤ ln E T′(g) ≤ ln C by step 3. With step 4, H(μ) ≤ ½ ln(2πe) + ln C.

The second form is the first with KL(μ ‖ N₁) = −H(μ) + ½ ln 2π + ½ m₂. ∎

**Lemma 2.20 (Lemma 2 of the floors track: the same along a triangular coupling).** Let ν be a law on
ℝᵏ with a density, finite second moments and H(ν) > −∞. For any order of the coordinates:

1. T^ν(g) has law ν when g is standard Gaussian in ℝᵏ;
2. 0 < Cᵢ(ν) ≤ √(E_ν zᵢ²) for every i;
3. H(ν) ≤ Σᵢ ln(√(2πe)·Cᵢ(ν)),

where the sums and the index i run over the k coordinates of ν.

Status: **[proved, refereed]**. Source: `program/floors/CLAIMS.md`, N.2;
`program/floors/referee/referee-core.md`, stage 1 ("no error found"; an adversary in the plane found
no gap below 0.091 nats); `program/stage0-floor-check/RESULTS.md`, section 2 (the falsifier's per-cell
bound, which rests on this lemma, is not refuted on the families that finished).

*Proof.* *The map.* Regular conditional laws exist. The conditional distribution functions are
jointly measurable, and so are their quantile functions, because
{Q(u | z_<i) ≤ t} = {u ≤ F(t | z_<i)}. Given g_<i, Φ(gᵢ) is uniform and independent of g_<i, so
T^ν_i(g) has the conditional law of the i-th coordinate given that the earlier ones equal
T^ν_<i(g_<i). By induction T^ν(g) has law ν. No continuity of the conditional distribution functions
is used, because the map goes from the Gaussian law, which has no atoms, to ν.

*The entropy.* Write ν_{i|<i}(· | z_<i) for the conditional law of the i-th coordinate given that the
earlier ones equal z_<i. ν has a density, so for ν-almost every z_<i this law has a density, and it
has a finite second moment. The chain rule holds for KL(ν ‖ N_k), with non-negative terms:
KL(ν ‖ N_k) = Σᵢ ∫ KL(ν_{i|<i}(· | z_<i) ‖ N₁) dν. Apply the second form of Lemma 2.19 to each
conditional law and integrate:

KL(ν ‖ N_k) ≥ Σᵢ [½ E_ν zᵢ² − ½ − E ln C(ν_{i|<i}(· | T^ν_<i(g_<i)))].

The last expectation is well defined: its positive part by ln C ≤ ½(m₂ − 1), its negative part by
Lemma 2.19 and the finiteness of the conditional KL. By Jensen for the logarithm,
E ln C(ν_{i|<i}) ≤ ln E C(ν_{i|<i}) = ln Cᵢ(ν), the equality because
E[gᵢ T^ν_i(g) | g_<i] = C(ν_{i|<i}). With KL(ν ‖ N_k) = −H(ν) + (k/2) ln 2π + ½ E_ν‖z‖² this is
item 3. Item 2 is Cauchy–Schwarz and the positivity in Lemma 2.19. ∎

*Relation to the published form.* Bolley, Gentil and Guillin's theorem in k dimensions uses the Brenier
map and gives H(ν) ≤ k·ln(√(2πe)·C*/k), with C* the largest E⟨g, x⟩ over couplings of N_k and ν.
Lemma 2.20 implies it, because Σᵢ ln Cᵢ ≤ k·ln(Σᵢ Cᵢ/k) by the inequality of the means and
Σᵢ Cᵢ ≤ C* because the triangular coupling is one coupling. What Lemma 2.20 adds is one correlation
per coordinate, which the water-filling of Theorem 2.24 needs. The librarian did not find it stated in
this form. This paragraph was written after the core referee read the text, on the librarian's report,
so it is **[proved, unrefereed]** (`program/library/floors-novelty.md` section 2).

**Lemma 2.21 (Lemma 3 of the floors track: the water-filling programme).** Let aᵢ > 0 and H ≥ 0, and
let λ ∈ (0, max aᵢ] solve Σᵢ ln⁺(aᵢ/λ) = H. Then

min{Σᵢ aᵢ sᵢ : sᵢ ∈ (0, 1], Σᵢ ln(1/sᵢ) ≤ H} = Σᵢ min(aᵢ, λ),

attained at sᵢ* = min(1, λ/aᵢ). With aᵢ = √γᵢ and θ = λ², W(H) = Σᵢ √min(γᵢ, θ), where
Σᵢ ½ ln⁺(γᵢ/θ) = H.

Status: **[proved, refereed]**. Source: `program/floors/CLAIMS.md`, N.3;
`program/floors/referee/referee-core.md`, stage 1 ("no error found"; stage 3 compares the closed form
with a numerical minimiser on 300 programmes).

*Proof.* For H > 0, λ exists and is unique because λ ↦ Σᵢ ln⁺(aᵢ/λ) is continuous and strictly
decreasing on (0, max aᵢ], from ∞ to 0; for H = 0 take λ = max aᵢ. Let
L(s) = Σᵢ aᵢ sᵢ − λ Σᵢ ln sᵢ. In each coordinate L is convex on (0, 1] with derivative aᵢ − λ/sᵢ, so
it is least at sᵢ*. For a feasible s, Σᵢ ln sᵢ + H ≥ 0, so
Σᵢ aᵢ sᵢ ≥ L(s) − λH ≥ L(s*) − λH = Σᵢ aᵢ sᵢ* + λ(Σᵢ ln(1/sᵢ*) − H) = Σᵢ aᵢ sᵢ*. ∎

**Lemma 2.22 (Lemma 4 of the floors track: properties of the floors).** Ψ and β are as in section
2.0. Ψ is the floor of the floors track's Conjecture A (`program/floors/CLAIMS.md`), which these notes
do not state, and it appears here only for comparison.

1. Ψ, W and V_S are convex and non-increasing in H, the minima are attained, and Ψ(0) = W(0) = F.
2. W(H) ≥ Ψ(H).
3. Ψ(H) ≥ F − √(H·Σᵢ γᵢ).
4. W(H) ≥ F − min(√γ₁·H, √(H·Σᵢ γᵢ)). If γ₁ > γ₂ and H ≤ ½ ln(γ₁/γ₂), then
   W(H) = F − √γ₁(1 − e^(−H)).
5. V_S(H) ≥ max(W_T(H), κ_k (Π_{i∈S} γᵢ)^(1/2k) e^(−H/k)), with T = S^c.
6. V_S(H) ≥ κ_k (Π_{i∈S} γᵢ)^(1/2k) e^(−H/k) + W_T(H), with T = S^c. The same holds with T replaced
   by any subset of S^c.

Status: items 1 to 5 **[proved, refereed]**. Source: `program/floors/CLAIMS.md`, N.4;
`program/floors/referee/referee-core.md`, stage 2 ("no error found"; stage 3 checks every inequality on
400 spectra). Item 6, which the core referee
suggested, and the condition γ₁ > γ₂ in item 4 were added after the referee read the text, so they are
**[proved, unrefereed]** (`program/floors/CLAIMS.md`, "Changes after the referees").

*Proof.* (1) Ψ and W are the least values of a linear objective over a set that is jointly convex in
(s, H), because β and −ln are convex. Each term of the constraint is non-negative on (0, 1] and tends
to infinity as s → 0, so the feasible set is compact and the minimum is attained. β(s) = 0 or
ln(1/s) = 0 on (0, 1] only at s = 1, so Ψ(0) = W(0) = F. V_S is the infimal convolution on [0, ∞) of
u ↦ W_T(u) and v ↦ κ_k(Π_S γᵢ)^(1/2k)e^(−v/k), both convex and non-increasing. (2) β(s) ≤ ln(1/s) on
(0, 1], so W minimises over a smaller set. (3) β(1) = β′(1) = 0 and β″ = 1 + s⁻² ≥ 2, so
β(s) ≥ (1 − s)². A feasible s has Σᵢ(1 − sᵢ)² ≤ H, and Σᵢ √γᵢ(1 − sᵢ) ≤ √(Σᵢ γᵢ)·√H by
Cauchy–Schwarz. (4) Use Lemma 2.21. For a = √γᵢ > λ: a − λ ≤ a ln(a/λ), which sums to at most
√γ₁·H; and ((a − λ)/a)² ≤ 1 − λ/a ≤ ln(a/λ), so Σᵢ(√γᵢ − λ)⁺ ≤ √(Σᵢ γᵢ)·√H by Cauchy–Schwarz. The
exact form is Lemma 2.21 with one coded coordinate. (5) and (6) In the minimand of V_S the head term
increases with u, so it is at least its value at u = 0; and W_T(u) ≥ W_T(H) for u ≤ H, because W_T is
non-increasing. This gives (6), and (5) follows because both terms are non-negative. For a subset T′
of S^c, W_{S^c} ≥ W_{T′}, because restricting a feasible point of the larger programme to T′ leaves it
feasible and drops non-negative terms. ∎

**Lemma 2.23 (Lemma 5 of the floors track).** κ_k ≥ k for every k ≥ 1.

Status: **[proved, refereed]**. Source: `program/floors/CLAIMS.md`, N.5;
`program/floors/referee/referee-core.md`, stage 2 ("no error found; confirmed by a second proof"), and
stage 3 (κ_k − k ≥ 0.6487 over every k ≤ 5,000 and a logarithmic grid to 10⁷,
`program/floors/referee/core_f2_misc.log`).

*Proof.* Let ν be uniform on the ball of radius ρ in ℝᵏ. T^ν(g) lies in the ball almost surely, so
ρ‖g‖ ≥ g·T^ν(g) and ρ·E‖g_k‖ ≥ Σᵢ Cᵢ(ν). By Lemma 2.20, Σᵢ ln(√(2πe)·Cᵢ(ν)) ≥ H(ν) = ln(ω_k ρᵏ),
and by the inequality of the means Σᵢ Cᵢ ≥ k(Πᵢ Cᵢ)^(1/k) ≥ k·ω_k^(1/k)·ρ/√(2πe). So
E‖g_k‖·√(2πe) ≥ k·ω_k^(1/k), which is κ_k ≥ k. ∎

The core referee gives a second proof that does not use Lemma 2.20, from Wendel's inequality for
E‖g_k‖ and Stirling's lower bound for ω_k (`program/floors/referee/referee-core.md`, stage 2). The
least value of κ_k − k is √e − 1 = 0.649, at k = 1 (**[computed]** by the core referee over
k ≤ 5,000 and a logarithmic grid to 10⁷ only, `program/floors/referee/core_f2_misc.log`; no proof
covers every k; see Editor's query 18).

### The water-filling floor

**Theorem 2.24 (Theorem F1 of the floors track: the water-filling floor).** Under the model, the
message and the certificate of this section,

E h ≥ W(I(z; **c**)) ≥ W(H(**c**)) = Σᵢ √min(γᵢ, θ),   where Σᵢ ½ ln⁺(γᵢ/θ) = H(**c**).

More precisely, when I(z; **c**) < ∞ there are numbers C̄ᵢ ∈ (0, 1] with E h ≥ Σᵢ √γᵢ C̄ᵢ and
Σᵢ ln(1/C̄ᵢ) ≤ I(z; **c**). When I(z; **c**) = ∞ the statement is E h ≥ 0.

With H(**c**) ≤ B ln 2 this is Conjecture 2.14, the "full floor" of the consolidated proofs' section 6:
W is non-increasing, and at H = B ln 2 the θ above is the water level for B of section 2.0. It holds
for every n, and for Σ_Q and Σ_R that need not commute.

Status: **[proved, refereed]**. Source: `program/floors/CLAIMS.md`, N.6;
`program/floors/referee/referee-core.md`, stage 1 ("no error found; it is the section 6 conjecture,
and a little more"); `program/stage0-floor-check/RESULTS.md` ("not refuted"; closest approach 1.0026).
The case I(z; **c**) = ∞, and the factorisation of the slack in "How tight" below, were added after the
core referee read the text, at its request, so they are **[proved, unrefereed]**
(`program/floors/CLAIMS.md`, "Changes after the referees").

*Proof.* Let I(z; **c**) < ∞.

1. *Width.* By (★), E_g h(c, ·) ≥ E_g h_{Z′_c}(g) for every c with P_c > 0.
2. *Coupling.* P_c ν_c ≤ N_{n₊}, so ν_c has a density at most (2π)^(−n₊/2)/P_c, finite second
   moments, and H(ν_c) ≥ ln(P_c(2π)^(n₊/2)) > −∞. Let T^{ν_c} be its triangular map (Lemma 2.20).
   T^{ν_c}(g) has law ν_c, so it lies in Z_c almost surely. It is a function of the same g at which the
   support function is taken, so pointwise h_{Z′_c}(g) ≥ Σᵢ √γᵢ gᵢ T^{ν_c}_i(g), and
   E_g h_{Z′_c}(g) ≥ Σᵢ √γᵢ Cᵢ(ν_c). The right side is integrable:
   E|gᵢ T^{ν_c}_i(g)| ≤ √(E_{ν_c} zᵢ²) ≤ P_c^(−½).
3. *Entropy.* By Lemma 2.20, H(ν_c) ≤ Σᵢ ln(√(2πe)·Cᵢ(ν_c)).
4. *Averaging.* The cell laws mix back to the Gaussian, Σ_c P_c ν_c = N_{n₊}. So
   Σ_c P_c H(ν_c) = H(z | **c**) = H(z) − I(z; **c**) = (n₊/2) ln(2πe) − I(z; **c**). With step 3,
   Σᵢ Σ_c P_c ln Cᵢ(ν_c) ≥ −I(z; **c**). Put C̄ᵢ = Σ_c P_c Cᵢ(ν_c). By Jensen for the logarithm,
   Σᵢ ln C̄ᵢ ≥ −I(z; **c**).
5. *C̄ᵢ ≤ 1.* By Lemma 2.20 and Jensen for the square root,
   C̄ᵢ ≤ Σ_c P_c √(E_{ν_c} zᵢ²) ≤ √(Σ_c P_c E_{ν_c} zᵢ²) = √(E zᵢ²) = 1.
6. E h = Σ_c P_c E_g h(c, ·) ≥ Σᵢ √γᵢ C̄ᵢ, and C̄ is feasible for the programme of W at rate
   I(z; **c**), so E h ≥ W(I(z; **c**)). W is non-increasing and I(z; **c**) ≤ H(**c**). The closed
   form is Lemma 2.21. ∎

The proof uses that the cell laws mix back to N_{n₊} in two places: in step 4, through
H(z | **c**) = H(z) − I(z; **c**), and in step 5. A bound proved cell by cell cannot use it, and it
loses for that reason (the floor Ψ of Lemma 2.22).

**How tight.** The inequalities of steps 3 to 6 are equalities when every ν_c is a Gaussian law with
covariance diag(sᵢ²), the same sᵢ for every cell, and no cell has such a law. For an actual code the
slack between E h and W is the product of the coupling step (the supremum over a cell against the
correlation with the cell's own law) and the gap in Lemma 2.20 for the cell law, which for an interval
cell at high rate are √2 and √(e/2). Computed codes come close in high dimension at low rate: shells of
the norm reach E h / W = 1.00055 at n = 10⁴ for an isotropic spectrum, as the core referee found, and
a grid on one head coordinate with a grid on the tail's norm reaches 1.0026 at 2,001 eigenvalues, as
the falsifier found (**[computed]**; `program/floors/referee/core_f2_misc.log`,
`program/stage0-floor-check/RESULTS.md`). For an isotropic spectrum Ingber, Courtade and Weissman's
scheme reaches the level of W with probability tending to 1 as n → ∞, at every rate, but its mean
width is infinite, so that is tightness in probability and not tightness of Theorem 2.24 (**[cited]**;
`program/floors/referee/referee-extensions.md`, P1). Whether some code attains W(H)(1 + o(1)) in the
mean is open.

### The combined floor

**Theorem 2.25 (Theorem F2 of the floors track: Theorem 2.2's constant on the head, the tail in
full).** Under the model, the message and the certificate of this section, for every set S of k ≥ 1
indices in {1, …, n₊}, with T = S^c,

E h ≥ V_S(I(z; **c**)) ≥ V_S(H(**c**)),

V_S(H) = min over 0 ≤ u ≤ H of { κ_k·(Π_{i∈S} γᵢ)^(1/2k)·e^(−(H−u)/k) + W_T(u) }.

T may be empty, and then V_S(H) = κ_{n₊} (Πᵢ γᵢ)^(1/2n₊) e^(−H/n₊). When I(z; **c**) = ∞ the
statement is E h ≥ 0.

Status: **[proved, refereed]**. Source: `program/floors/CLAIMS.md`, N.7;
`program/floors/referee/referee-core.md`, stage 3 ("no error found"; the inequality in which head and
tail meet, attacked in the plane and in space, kept a smallest ratio of 1.0006);
`program/stage0-floor-check/RESULTS.md` ("not refuted"; closest approaches 1.0005 and 1.0023). The
requirement k ≥ 1, the case of an empty T and the case I(z; **c**) = ∞ were stated after the core
referee read the text, at its request, so they are **[proved, unrefereed]**
(`program/floors/CLAIMS.md`, "Changes after the referees").

**Corollary 2.26 (Corollary F2a of the floors track).** Let H = H(**c**) > 0, let θ solve
Σᵢ ½ ln⁺(γᵢ/θ) = H, and let k = #{i : γᵢ > θ}, so that eigenvalues equal to θ are in the tail. Then
k ≥ 1 and

E h ≥ κ_k·√θ + Σ_{i>k} √γᵢ.

This is the full floor with κ_k in place of k on the head. It exceeds Σᵢ √min(γᵢ, θ) by (κ_k − k)√θ.

Status: **[proved, refereed]**. Source: `program/floors/CLAIMS.md`, N.7;
`program/floors/referee/referee-core.md`, stage 3 ("no error found, ties included"; a brute force over
u agrees with the formula to within 5·10⁻¹⁶); `program/stage0-floor-check/RESULTS.md`, section 3
(Theorem 2.25 at top-k head sets that include those near the water level; nothing refuted). The rule that eigenvalues equal to θ are in the tail, and the treatment of repeated
eigenvalues and of k = n₊ in the proof, were written in after the core referee read the text, at its
request, so they are **[proved, unrefereed]**, although the referee checked the case of ties in its own
reading and by brute force (`program/floors/CLAIMS.md`, "Changes after the referees").

*Proof of Theorem 2.25.* The proof uses one fact from step 3 of Theorem 2.2 (Urysohn's inequality):
for a bounded Borel set G in ℝᵏ and g_k standard Gaussian, E h_G(g_k) ≥ E‖g_k‖·(vol(G)/ω_k)^(1/k).
Let I(z; **c**) < ∞, and order the coordinates with those of T first.

1. *Tail first.* Fix c with P_c > 0. Let ν_{c,T} be the marginal of ν_c on the T-coordinates and
   T^{ν_{c,T}} its triangular map (Lemma 2.20 in ℝ^T; the density of ν_{c,T} is at most
   (2π)^(−|T|/2)/P_c). Put z_T = T^{ν_{c,T}}(g_T). It has law ν_{c,T} exactly, and since it is a
   function of g_T it is independent of g_S.
2. *Head on the fibre.* Let Z_c^{z_T} = {s : (s, z_T) ∈ Z_c}, and let ν_{c,S|z_T} be the conditional
   law under ν_c of the S-coordinates given that the T-coordinates equal z_T. Since ν_c(Z_c) = 1, for
   ν_{c,T}-almost every z_T the law ν_{c,S|z_T} has a density and is carried by Z_c^{z_T}; and z_T has
   law ν_{c,T}, so this holds at T^{ν_{c,T}}(g_T) almost surely. For every s in the fibre,
   (s, z_T) ∈ Z_c, so pointwise in g = (g_T, g_S)

   h_{Z′_c}(g) ≥ Σ_{i∈T} √γᵢ gᵢ (z_T)ᵢ + sup over s ∈ Z_c^{z_T} of Σ_{i∈S} √γᵢ gᵢ sᵢ.

   No point of the fibre is selected. The supremum is a Borel function of (z_T, g_S), because Z_c is a
   countable union of compact sets and the supremum over the fibre of a compact set is upper
   semicontinuous.
3. *Urysohn.* Given g_T the fibre is a fixed closed set and g_S is standard Gaussian in ℝᵏ. Apply
   Urysohn's inequality to the image of the fibre under s ↦ (√γᵢ sᵢ)_{i∈S}, whose volume is
   (Π_{i∈S} γᵢ)^½ vol(Z_c^{z_T}):

   E[sup over the fibre | g_T] ≥ E‖g_k‖·ω_k^(−1/k)·(Π_{i∈S} γᵢ)^(1/2k)·vol(Z_c^{z_T})^(1/k).

   An unbounded fibre makes the left side infinite. A support function that is finite in every
   direction belongs to a bounded set; otherwise the directions in which it is finite form a convex
   cone that is not the whole space, so it is +∞ on a set of Gaussian measure at least ½. The uniform
   law maximises entropy on a set, so ln vol(Z_c^{z_T}) ≥ H(ν_{c,S|z_T}).
4. *Jensen in the cell.* z_T has law ν_{c,T}, so E exp(H(ν_{c,S|z_T})/k) ≥ exp(a_c/k), where
   a_c = ∫ H(ν_{c,S|z_T}) dν_{c,T} is the conditional entropy of z_S given z_T in the cell. With (★),
   step 2, and step 2 of Theorem 2.24 applied to the tail,

   E_g h(c, ·) ≥ Σ_{i∈T} √γᵢ Cᵢ(ν_{c,T}) + E‖g_k‖·ω_k^(−1/k)·(Π_{i∈S} γᵢ)^(1/2k)·e^(a_c/k).

5. *Entropies.* b_c = H(ν_{c,T}) ≤ Σ_{i∈T} ln(√(2πe)·Cᵢ(ν_{c,T})) by Lemma 2.20, and
   a_c + b_c = H(ν_c) by the chain rule. All three are finite, because the densities are bounded and
   the second moments finite.
6. *Averaging over cells.* Write I = I(z; **c**). Let ā and b̄ be the means of a_c and b_c over c, and
   C̄ᵢ the mean of Cᵢ(ν_{c,T}). By Jensen, Σ_c P_c e^(a_c/k) ≥ e^(ā/k) and
   b̄ ≤ Σ_{i∈T} ln(√(2πe)·C̄ᵢ). Also ā + b̄ = H(z | **c**) = (n₊/2) ln(2πe) − I, and C̄ᵢ ≤ 1 as in
   Theorem 2.24. Put u = Σ_{i∈T} ln(1/C̄ᵢ) ≥ 0. Then b̄ ≤ (|T|/2) ln(2πe) − u, so
   ā ≥ (k/2) ln(2πe) − (I − u), and, because E‖g_k‖·ω_k^(−1/k)·√(2πe) = κ_k,

   E h ≥ κ_k·(Π_{i∈S} γᵢ)^(1/2k)·e^(−(I−u)/k) + Σ_{i∈T} √γᵢ C̄ᵢ
       ≥ κ_k·(Π_{i∈S} γᵢ)^(1/2k)·e^(−(I−u)/k) + W_T(u).

   Finally u ≤ I, because ā = H(z_S | z_T, **c**) ≤ H(z_S) = (k/2) ln(2πe). So E h ≥ V_S(I), and V_S
   is non-increasing. ∎

The rate splits exactly, I(z; **c**) = I(z_T; **c**) + I(z_S; **c** | z_T), and the minimum over u lets
the code choose the split. The order of the coordinates matters for two reasons. The tail point has
exactly the cell's tail law, so entropies and second moments average over it. The head supremum is
taken over a whole fibre at a point independent of the head Gaussian, so nothing is selected.

*Proof of Corollary 2.26.* H > 0 gives θ < γ₁, so k ≥ 1. Take S = {1, …, k}. By the water-filling
equation, (Π_{i≤k} γᵢ)^(1/2k) e^(−H/k) = √θ. If k = n₊ the tail is empty and V_S(H) = κ_{n₊}√θ.
Otherwise f(u) = κ_k (Π_{i≤k} γᵢ)^(1/2k) e^(−(H−u)/k) + W_T(u) is convex on [0, H]. If the largest
eigenvalue of the tail is repeated j times, Lemma 2.21 gives
W_T(u) = j·√γ_{k+1}·e^(−u/j) + Σ_{i>k+j} √γᵢ for small u, so W_T′(0⁺) = −√γ_{k+1} with or without
ties. Hence f′(0⁺) = (κ_k/k)√θ − √γ_{k+1}. Now γ_{k+1} ≤ θ by the definition of k, and κ_k ≥ k
(Lemma 2.23), so f′(0⁺) ≥ 0 and the minimum is at u = 0: V_S(H) = κ_k√θ + Σ_{i>k} √γᵢ. ∎

### Remarks on Theorems 2.24 and 2.25

- *The water-filling head is not always the best head.* Every S gives a floor. For γ = (4, 1, 1) and
  H = ln 2 the water-filling head is {1}, and Corollary 2.26 gives κ₁ + 2 = 3.649, while S = {1, 2, 3}
  gives κ₃ = 4.091. On the measured spectra the best top-k head is a little larger than the
  water-filling one, and the gain over W is 1 to 2 % (**[computed]**; `program/floors/floors_eval.py`,
  and line 2 of `program/floors/check_after_referees.log` for the example, which is the core
  referee's).
- *A seed shared by the encoder and the certificate.* If both read a seed ω independent of (r, Δ), the
  hypotheses hold conditionally on ω. W and V_S are convex (Lemma 2.22), so averaging over ω gives
  E h ≥ V_S(I(z; **c** | ω)) for every S, and I(z; **c** | ω) ≤ H(**c** | ω) ≤ H(**c**). The remark
  concerns sound certificates only, and a δ-valid certificate is outside it. **[proved, unrefereed]**:
  the core referee named the case, and the remark was written after it read the text.
- *Relation to Theorems 2.2 to 2.6.* Corollary 2.3 is the bound V_S ≥ κ_k(Π_S γᵢ)^(1/2k)e^(−H/k) of
  Lemma 2.22, item 5. Theorem 2.5 follows from Theorem 2.24, because W ≥ W_T for every index set T and
  item 4 of Lemma 2.22 applied to W_T gives W_T(H) ≥ Σ_T √γᵢ − √(H·Σ_T γᵢ), which is above Theorem
  2.5's bound. Theorem 2.6 follows from Theorem 2.25 and item 6 of Lemma 2.22, with no added bit and no
  factor 1/√e. Theorem 2.2 itself does not follow, because it covers any residual with a density on a
  subspace and Corollary 2.4 covers shared information, while Theorems 2.24 and 2.25 are proved for
  Gaussian residuals. Theorem 2.25 is not independent of Theorem 2.2 either, because its step 3 is
  Theorem 2.2's step 3 applied to a fibre. **[proved, unrefereed]**: the core referee gave this
  argument in its summary, and it was written into the text afterwards. Line 1 of
  `program/floors/check_after_referees.log` checks the two inequalities on 400 spectra
  (**[computed]**).
- *What is not claimed.* Nothing for residuals that are not Gaussian, for offsets that are not Gaussian
  or that depend on the residual, or for real data. Stored real scalars must be counted at their bit
  width, as in Theorem 2.2.

**Prior statements of the formula.** The curve of W was published as an asymptotic identification rate
for compressed Gaussian data answering similarity queries with no false negative. Ingber, Courtade and
Weissman (IEEE Trans. Inf. Theory 61, 2015) prove it, with its converse, for isotropic laws as n → ∞,
for the probability of the answer "maybe". Steiner, Dempfle, Ingber and Weissman (arXiv:1404.5173)
give a finite-n lower bound on that probability, also isotropic. Wu, Wang and Flierl
(arXiv:2001.07941) state the anisotropic formula for Σ_Q = Σ_R and do not prove its converse.
Theorem 2.24 is, as far as the librarian's search goes, the first proof of the converse at finite n,
for covariances that need not commute, in the mean-width form, and it does not claim the formula. The
nearest current work on inner products, Balasubramanian, Bhandari and Kasiviswanathan
(arXiv:2610.24890), bounds the mean squared error of an estimate and not a sound interval. Status:
**[cited]**. The extensions referee read the arXiv sources of Ingber, Courtade and Weissman
(arXiv:1307.6609) and of Wu, Wang and Flierl, and the librarian read the source of Steiner, Dempfle,
Ingber and Weissman and the text of Balasubramanian, Bhandari and Kasiviswanathan. Source:
`program/floors/CLAIMS.md`, N.7;
`program/library/floors-novelty.md` section 3; `program/floors/referee/referee-extensions.md`, P1 and
P2. Section 2.11 places the result in the literature.

**Numerical checks** (**[computed]**). A code's exact range is its smallest sound certificate, so a
code whose exact mean width fell below a floor would refute it, and none did.

- The author (`program/floors/check_schemes.py`, `program/floors/check_chain1d.py`): 2,500 product
  codes against every head set, smallest ratio 1.000040; 2,800 planar partitions that are not
  products, 1.135; head grids with shells of the dual norm on the tail, n up to 48, 1.030 against
  max_S V_S and 1.080 against W.
- The core referee (`program/floors/referee/core_*.py`): Lemma 2.20 in the plane with an adversary,
  smallest gap 0.091 nats; the inequality in which head and tail meet (steps 1 to 4 of the proof of
  Theorem 2.25 for one cell), with an adversary over densities, smallest ratio 1.0006; whole codes in
  three dimensions against all eight head sets, 1.237; Corollary 2.26 with ties by brute force over u,
  equal to the formula to within 5·10⁻¹⁶; product codes to n = 10⁴, E h / W = 1.00055.
- The falsifier (`program/stage0-floor-check/RESULTS.md`): nothing refuted; a code within 1.0026 of W
  and within 1.0005 of max_S V_S (there S is all the indices, which gives the bound of Theorem 2.2),
  and within 1.0023 of V_S with S the head.
- Not tested by anyone: a high-dimensional code whose cells couple the head to the tail.

## 2.10 Conjectures

Each conjecture is stated in the setting of Corollary 2.3 unless it says otherwise. Source for all
five: `proofcheck/CONSOLIDATED_PROOFS.md` section 6.

**Conjecture 2.14 (full floor).** E h ≥ Σᵢ √min(γᵢ, θ), with θ the water level for B.
Settled by Theorem 2.24, **[proved, refereed]** (section 2.9a). What was missing was a single argument
that gives the head term k·√θ and the tail Σ_{i>k} √γᵢ together, without the losses of Theorem 2.6
(the factor 1/√e, the extra bit, and the concentration penalty on the tail). The proof supplies it by
coupling the Gaussian with each cell's own law through a triangular map, so that one entropy
inequality per coordinate (Lemma 2.20), averaged over cells whose laws mix back to the Gaussian, prices
the head and the tail together as one water-filling programme (Lemma 2.21).

**Conjecture 2.15 (Gaussian residuals are the worst case).** The true optimum of E h, not only the
bound of Theorem 2.2, is no larger for a non-Gaussian residual than for a Gaussian one with the same
covariance. **[conjecture]**. *Missing:* an achievability argument at low rate. The source says that
at high rate the statement follows from the tightness of Theorem 2.2 (see Editor's query 10).

**Conjecture 2.16 (non-Gaussian queries).** Theorem 2.2 holds for an offset Δ with covariance Σ_Q
that is not Gaussian. **[conjecture]**. *Missing:* replacements for E[g_⊥ | g_S] = 0 in step 2 and
for rotation invariance in the Urysohn step. The statement is false without some condition, for
example when Δ is supported on finitely many directions.

**Conjecture 2.17 (√k is unavoidable for a class unknown at build).** A certificate that must hold
uniformly over a k-dimensional class of offsets not known when the index is built pays a factor of
order √k. **[conjecture]**. *Known:* for a uniformly random k-dimensional subspace W of ℝⁿ
independent of the error, E‖P_W(x − x̂)‖² = (k/n)‖x − x̂‖² for any decoder x̂, because rotation
invariance gives E P_W = (k/n)·I (**[proved, refereed]** as a statement about random W;
`proofcheck/REFEREE.md`, "T4 … the '√k unavoidable' remark is only a heuristic"). *Missing:* a lower
bound for every W, or a minimax statement over decoders. Chapter 3's Theorem 3.5 (the class-uniform
bound, with k_W for k) gives a randomised level's half-width over such a class, which grows as
√k_W, and chapter 3 lists the same conjecture under its Open items.

**Conjecture 2.18 (tail without the factor 2).** For U a coordinate of a uniform unit vector in
ℝ^(d_j) and 0 ≤ s ≤ 1/√2, Pr[|U| ≥ s] ≤ (1 − s²)^(d_j/2). **[conjecture]**. It holds on a grid for
2 ≤ d_j ≤ 5,000, with equality at d_j = 2 and s = 1/√2 (**[computed]**,
`proofcheck/constants.py`). *Missing:* a proof. The bound with the factor 2 is proved
(Proposition 10 of the consolidated proofs, Proposition 3.2 of chapter 3, which writes m for d_j), and
chapter 3 lists the same conjecture under its Open items.

## 2.11 Relation to the literature

The literature survey found no published statement of Theorems 2.2–2.6, and lists them as
apparently new, subject to a specialist's check. Theorem 2.2 has the form of the Shannon lower bound
with Urysohn's inequality in place of the second-moment step, and the survey recommends presenting
it that way (Shannon lower bound **[cited, unchecked]**). The closest published result is
Ordentlich and Polyanskiy's rate–distortion function for the mean squared error of Gaussian inner
products, which bounds the error of an estimate where Theorem 2.2 bounds the width of a sound
interval, that is the support function of the cell (**[cited]**, arXiv:2410.13780). Alon and
Klartag (**[cited, unchecked]**; explicit formula seen only in secondary slides) and Indyk and
Wagner (**[cited]**, arXiv:1609.06295) give worst-case sketching results. The survey judges the
floor unrelated in technique to Alon and Klartag, and asks for a specialist's check of that. The survey found no prior statement of Theorem 2.5, which applies Gaussian isoperimetry to
quantiser cells. Okajima and Yoshida, and Xiao and Cao, address different questions
(**[cited, unchecked]**, abstracts only). Source: `lit/REPORT.md`, item 4 and bibliography
entries 20–24.

The formula of Conjecture 2.14 and Theorem 2.24 was stated before. Wu, Wang and Flierl
(arXiv:2001.07941) state it for Σ_Q = Σ_R, and they do not prove its converse. The converses that are
proved are isotropic: Ingber, Courtade and Weissman (IEEE Trans. Inf. Theory 61, 2015;
arXiv:1307.6609) prove one as n → ∞, and Steiner, Dempfle, Ingber and Weissman (arXiv:1404.5173) prove
one at finite blocklength, both for the probability of the answer "maybe" (**[cited]**: the extensions
referee read the first two papers' arXiv sources, the librarian the third's). As far as the program's
search goes, Theorem 2.24 is the first proof of the converse at finite n, for covariances that need
not commute, in the mean-width form. Tightness holds in probability by Ingber, Courtade and Weissman's
scheme, whose mean width is infinite, and in the mean it is shown only by computed codes (section
2.9a, "How tight"). Source: `program/library/floors-novelty.md` section 3;
`program/floors/referee/referee-extensions.md`, P1 and P2; `program/floors/CLAIMS.md`, N.7.

Theorem 2.11 is the indistinguishability argument of Seidl and Kriegel's optimal multi-step search,
applied per item to nested prefixes, and the survey recommends presenting it as a generalisation
(**[cited, unchecked]**, abstract only). Source: `lit/REPORT.md`, item 5.

The proofs use these standard results, which no file on this branch states with a source read:
Urysohn's inequality, the Gaussian isoperimetric inequality, Gaussian concentration for Lipschitz
functions, Fano's inequality and the entropy bound on the expected length of a prefix-free code
(**[cited, unchecked]**; `lit/REPORT.md` lists Urysohn's inequality as cited from memory).

## Open

- **Conjecture 2.15 (Gaussian worst case):** achievability at low rate.
- **Conjecture 2.16 (non-Gaussian queries):** the conditions under which Theorem 2.2 survives.
- **Conjecture 2.17 (√k for an unknown class):** a lower bound for a given W, or a minimax form.
- **Conjecture 2.18 (tail without the factor 2):** a proof.
- **A floor for the real data.** None is proved: the residuals and queries are not Gaussian, the
  queries depend on the items, and the shared bases and the m·e scalar are not counted
  (section 2.9). Theorem 2.24 proves the full floor for Gaussian residuals only. For residuals that
  are not Gaussian the proved bound of Theorem 2.2 falls by exp(−J/k), with J their negentropy
  (section 2.1), so a floor for them needs J bounded from above, while the only estimates of J on
  these data are lower bounds (`entropy/REPORT.md`, Summary).
- **Offsets that are not Gaussian, for the full floor.** Theorems 2.24 and 2.25 assume Δ Gaussian and
  independent of r. Conjecture 2.16 asks the same question of Theorem 2.2.
- **A code that attains the full floor in the mean.** Computed codes come within 1.0026 of W, and for
  an isotropic spectrum Ingber, Courtade and Weissman's scheme attains it in probability as n → ∞, but
  no code is proved to attain W(H)(1 + o(1)) in the mean (section 2.9a, "How tight").
- **Cells that couple the head and the tail in high dimension.** No one has tested such a code against
  Theorems 2.24 and 2.25. Coupled cells were tested only in two and three dimensions, where the floors
  are loose (1.13 and above), and in high dimension only product codes were computed
  (`program/floors/referee/referee-core.md`, stage 3).
- **Instance-optimality across encodings.** Theorem 2.11 fixes the stream. No per-query bound across
  all encodings exists, and the scheme's weighted Cauchy–Schwarz certificate is not the exact range
  the theorem's upper half needs.
- **Randomised certificates.** Theorems 2.2–2.6 cover deterministic, sound certificates only. They
  do not cover randomised certificates, offsets correlated with the residual, or non-Gaussian
  structure (`agent1/REPORT.md` section 2). Chapter 3 gives the randomised certificates' upper
  bounds, and its R9 is the nearest statement of a lower bound.
- **Index-size cap.** Proposition 2.13 has no useful content unless index size is capped, and no
  bound with a cap has been stated.
- **Proposition 2.9 for choices that depend on bits already read.**

## Editor's queries

1. **Constant in Theorems 2.5 and 2.6.** `PROOFS.md` and `agent1/theory.py` use the constant 1
   where the consolidated proofs use 1/√2. Agent 1's computed "best proved" figures (section 2.5)
   therefore use a weaker form of Theorems 2.5 and 2.6. Those figures remain valid, and since the
   volume bound was binding in every row they probably do not change, but nobody has recomputed
   them with 1/√2.
2. **"0.93 or more from 96 bytes."** The consolidated proofs state this for two spectra. In
   `agent1/theory.log` group 12 gives 0.0063/0.0068 ≈ 0.93 at 96 bytes, but group 2 gives
   0.0115/0.0126 ≈ 0.91 (0.905–0.920 allowing for the four-digit rounding of the log). The 0.93
   appears to come from group 12 alone.
3. **The "measured spectra" are diagonal.** Agent 1 (`theory.py`), agent 3a (`exp_c.py`), agent 3b
   (`stats.py`) and the code track (`floor2.py`) all take γᵢ = λᵢQᵢ, with Qᵢ the query-offset energy
   along residual eigenvector i, which equals the eigenvalues of Σ_Q^½ Σ_R Σ_Q^½ only when Σ_Q and
   Σ_R commute. The groups report says Σ_Q is far from diagonal in that basis. Every floor in sections
   2.5 and 2.9 is therefore the floor of a diagonal model.
4. **"1,200–1,900× above it."** `agent3b/REPORT.md` says the baseline is 1,200–1,900× above the
   entropy bound. Its own table gives 0.564/0.00044 ≈ 1,280, 0.870/0.0013 ≈ 670, 1.868/0.0027 ≈ 690
   and 5.336/0.0046 ≈ 1,160 at n = 768, so 670–1,280×. With the unrounded bounds of
   `agent3b/stats.log` (0.00044, 0.00127, 0.00265, 0.00456 %) the range is about 685–1,280×, and
   the n = 1,024 rows give about 475–1,250×. Neither reproduces 1,200–1,900×. The chapter does not
   quote the source's range.
5. **Proposition 2.13.** The source writes "H(S(t)|q) − h(ε) − εN = N·h(p)". The equality holds only
   at ε = 0 and for independent items with a common match probability p, and the chapter states it
   in that form. The source also writes "% read ≥ h(p)/(16n)", which is a fraction and needs a
   factor 100 to be a percentage (`stats.py` applies the factor). The proof steps in section 2.8
   are the editor's expansion of the source's one sentence. The corpus is not random, so the
   independence model is an assumption, and the computed figures use each query's match share as p.
6. **Proposition 2.10.** The source states the reduction to INDEX in one sentence. It does not say
   what a summary must answer (which sub-collections, which thresholds), or which form of the INDEX
   lower bound it uses, so a referee would need those written down.
7. **Lemma 2.7.** The step "u·v can take every value in [−1, 1]" needs n ≥ 3, because with n = 2
   the vectors u and v orthogonal to p are ±the same unit vector and u·v takes only the values ±1.
   The conclusion survives at n = 2, because both ends of the interval are still attained, so the
   interval remains the tightest one. The source does not state the condition.
8. **Hypotheses of Conjecture 2.14.** The consolidated proofs state the conjecture without its
   hypotheses. The chapter assumes those of Corollary 2.3 (Gaussian residuals, Gaussian offsets
   independent of r, every stored bit in B). Theorem 2.24 now states its setting in full (section
   2.9a, "Setting"), and that setting contains these hypotheses; the core referee reads the
   conjecture with the same ones (`program/floors/referee/referee-core.md`, stage 1 verdicts and
   "Summary for the PI"). This answers the query for the conjecture as the chapter states it.
9. **Exact box ∩ ellipsoid.** Agent 3a measured a 3–14 % narrower mean half-width and the code track
   a 0.3 % reduction in bytes read, and `quant/REPORT.md` lists the second as contradicting the
   first. The two figures measure different things (width against bytes read, one global basis
   against the built code), so they need not conflict, and neither source reconciles them.
10. **Conjecture 2.15 at high rate.** The source says the high-rate case follows from the tightness
    of Theorem 2.2. Tightness is shown only numerically and only in one dimension (1.0001 at step
    0.05σ), so the high-rate case is not proved either.
11. **The referee's 3,000 non-commuting pairs.** `proofcheck/REFEREE.md` reports "worst
    achieved/floor 1.13" without saying which code or certificate was "achieved".
12. **What `floor2.py` measures.** The "achieved" half-width is √(E‖DΔ‖²) times the stored rounded
    norm, which bounds the mean weighted Cauchy–Schwarz width from above, so it is a model figure
    on real residuals. The script ignores α and the sphere constraint of the kept certificate, and
    it encodes with the escape code (`esc_steps`), while `quant/REPORT.md` recommends keeping the
    fixed-rate tree code. The shared bases are not counted. For n1024 the item and mixed offset
    profiles print identical rows, which suggests that no mixed profile was fitted for that index.
13. **Agent 3a's T5** included shared bits per group without assuming independent items. Corollary
    2.4 adds the assumption, as the referee required.
14. **Offset law in Proposition 2.8.** The source says only "hold the offset law fixed". Theorem 2.2
    needs g independent of r, so the chapter reads the hypothesis as the same law Δ = Σ_Q^½ g in
    every cell, independent of x. Since Δ = q − m_j depends on the cell, this is a modelling
    assumption a referee should confirm.
15. **Known part of Conjecture 2.17.** The sources state the identity for a uniformly random W
    without proof (agent 3a says "for a linear decoder", the consolidated proofs "for any decoder").
    The one-line reason given, E P_W = (k/n)·I by rotation invariance, is the editor's; it needs W
    independent of x − x̂ and nothing about the decoder. The tag [proved, refereed] rests on the
    referee's remark that the statement "holds only for a uniformly random W"; no written proof was
    refereed, so a referee should confirm the tag.
16. **The referee's "two small fixes".** The verdict table gives "Correct with two small fixes" for
    Theorem A and T5 together, while the section "Fixes to the floor" lists three points (unique
    decodability, independence for shared bits, the symmetric matrix). Which two the verdict means is
    not stated; the third may belong to T5 alone.
17. **"Before" in `floor2.py` and in `quant/REPORT.md`.** The bytes-read table of `quant/REPORT.md`
    says its "before" code already includes the mixture weights. `floor2.py`'s "before" variant
    weights the code by the item-offset profile Qi and charges 8-bit norms, so the two "before"
    codes differ, and the floor ratios' before → after change is not the change measured in bytes.
18. **The least value of κ_k − k.** `program/floors/CLAIMS.md`, N.5, states after its proof of
    Lemma 2.23 that the least value of κ_k − k is √e − 1, at k = 1, without a tag or an argument; its
    section 1 (L5) gives the same sentence as a numerical finding of `referee/core_f2_misc.py`. The value
    at k = 1 is exact, because κ₁ = √e. That no larger k gives less is stated by neither proof of
    Lemma 2.23. The evidence is the core referee's computation over every k ≤ 5,000 and a logarithmic
    grid to 10⁷ (`program/floors/referee/referee-core.md`, verdicts table, "κ_k − k ≥ 0.6487 (k = 1)", and
    stage 3), so the chapter tags the sentence [computed] and names that range. Beyond it the statement
    is unproved.
