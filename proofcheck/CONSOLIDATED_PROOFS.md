# Certified threshold search: consolidated proofs

Results that survived an independent referee check (see `REFEREE.md` in this directory). Results
found only to have a gap are in section 6 as conjectures. Numerical checks are in
`proofcheck/constants.py`, `proofcheck/toyfloor.py` and `proofcheck/misc.py`.

## 0. Setting and notation

- **Items and queries.** An item is a unit vector x = m + r in ℝⁿ, with m its cluster mean and r
  the residual. A query is a unit vector q with threshold t. The offset is Δ = q − m, so
  q·x = q·m + m·r + Δ·r, and only Δ·r is uncertain.
- **Covariances.** Σ_R is the covariance of r and Σ_Q that of Δ. Write Δ = Σ_Q^½ g and
  r′ = Σ_Q^½ r, so Δ·r = g·r′. The numbers γ₁ ≥ γ₂ ≥ … ≥ γₙ are the eigenvalues of the symmetric
  matrix Σ_Q^½ Σ_R Σ_Q^½, with orthonormal eigenvectors v₁, …, vₙ.
- **Message.** An encoder maps r to a message c in a countable set. The message is everything
  stored for the item, including stored scalars. P_c is the probability of c,
  K_c = {r : encoder(r) = c}, and K′_c = Σ_Q^½ K_c. H(c) is the entropy of the message in nats.
  If the message is B bits of fixed length, or a uniquely decodable code of expected length B
  bits, then H(c) ≤ B ln 2.
- **Certificate.** A certificate assigns to each (c, Δ) an interval of half-width h(c, Δ). It is
  *sound* (deterministic) if the interval contains Δ·r for every r ∈ K_c. E h is the mean over r
  and Δ.
- **Geometry.** For a set A, h_A(y) = sup over a ∈ A of y·a is the support function. ω_k is the
  volume of the unit ball of ℝᵏ. g_k is a standard Gaussian vector in ℝᵏ, with
  E‖g_k‖ = √2·Γ((k+1)/2)/Γ(k/2). Define κ_k = E‖g_k‖·√(2πe)·ω_k^(−1/k). For a subspace S, P_S is
  the orthogonal projection.
- **Randomised scheme.** The index is built with a seed ω. R(q, t, ω) is the returned set and
  S(q, t) = {x : q·x ≥ t} the exact one. In one rotated block of n_b dimensions, y is the item's
  block, R a Haar rotation drawn from ω, φ the angle between the code and Ry, and A = ‖y‖·tan φ
  the stored error scale. Write m_b = n_b − 1.

## 1. The volume floor

**Theorem 1.** Let r have a law such that r′ = Σ_Q^½ r has a density on a k-dimensional subspace
S after projection, with differential entropy h_S = h(P_S r′) in nats. Let Δ = Σ_Q^½ g with g
standard Gaussian and independent of r. For any encoder and any sound certificate,

E h ≥ E‖g_k‖ · ω_k^(−1/k) · exp((h_S − H(c))/k).

**Proof.**

1. *Width.* Fix c and g. Soundness gives an interval containing g·x for all x ∈ K′_c, so
   2h(c, Δ) ≥ h_{K′_c}(g) + h_{K′_c}(−g). Since g and −g have the same law,
   E_g h(c, ·) ≥ E_g h_{K′_c}(g). If K′_c is unbounded the right side is infinite and the theorem
   is trivial for that cell.
2. *Projection.* Split g = (g_S, g_⊥) along S and its orthogonal complement. These are
   independent and E[g_⊥ | g_S] = 0. The support function is convex, so by Jensen
   E[h_{K′_c}(g) | g_S] ≥ h_{K′_c}((g_S, 0)) = h_{P_S K′_c}(g_S).
3. *Urysohn.* For bounded A ⊂ ℝᵏ write g_S = ‖g_S‖·θ with θ uniform on the sphere and independent
   of ‖g_S‖. Then E h_A(g_S) = E‖g_k‖ · E_θ h_A(θ). The second factor is half the mean width of
   conv A. Urysohn's inequality bounds it below by (vol(conv A)/ω_k)^(1/k) ≥ (vol(A)/ω_k)^(1/k).
4. *Entropy.* Given c = c the vector P_S r′ lies in P_S K′_c almost surely. The uniform law
   maximises entropy on a set, so h(P_S r′ | c = c) ≤ ln vol(P_S K′_c). Averaging,
   Σ_c P_c ln vol(P_S K′_c) ≥ h(P_S r′ | c) = h_S − I(P_S r′; c) ≥ h_S − H(c).
5. *Jensen.* Σ_c P_c vol(P_S K′_c)^(1/k) ≥ exp((1/k) Σ_c P_c ln vol(P_S K′_c))
   ≥ exp((h_S − H(c))/k). Combine with steps 1–3. ∎

The proof only uses that the conditional law of r′ is carried by K′_c. Requiring soundness almost
surely instead of everywhere gives the same bound.

**Corollary 1 (Gaussian residuals).** If r ~ N(0, Σ_R) and H(c) ≤ B ln 2, then for every k with
γ_k > 0,

E h ≥ κ_k · (γ₁⋯γ_k)^(1/2k) · 2^(−B/k).

*Proof.* Take S = span(v₁, …, v_k). Then P_S r′ is Gaussian with covariance diag(γ₁, …, γ_k), so
h_S = ½ ln((2πe)ᵏ γ₁⋯γ_k). ∎

Σ_Q and Σ_R need not commute. If θ and k solve Σᵢ ½ log₂⁺(γᵢ/θ) = B with k = #{i : γᵢ > θ}, the
bound equals κ_k·√θ.

**Corollary 2 (shared information).** Let M items have independent residuals. Item j has message
c_j, and all M share a further message s of entropy at most S bits, which may depend on every
item. If the mean of H(c_j) is at most B bits, the mean over items of E h satisfies Corollary 1
with B replaced by B + S/M.

*Proof.* In step 4 the information is I(P_S r′_j; c_j, s). By independence of the items,
Σ_j I(r′_j; c_j, s) ≤ Σ_j I(r′_j; c^M, s) ≤ I(r′^M; c^M, s) ≤ Σ_j H(c_j) + H(s). Apply step 5
over items and cells together. ∎

Independence is needed. If the items are copies of one another, s informs all of them.

**Remarks.**
- κ_k ≥ k for every k ≤ 200,000, with κ_k/k = 1.649, 1.461, 1.161, 1.027 at k = 1, 2, 10, 100
  and κ_k/k ≈ (πk)^(1/2k)(1 − 1/4k) (`constants.py`). So the bound is at least k·√θ, the head
  term of Σᵢ √min(γᵢ, θ).
- The bound is attained in one dimension at high rate by an entropy-coded uniform quantiser: the
  ratio is 1.0001 at step 0.05σ. No code tested fell below it (`toyfloor.py`, `misc.py` (a)).
- Stored exact real scalars are outside the theorem, because one real can encode all of r. Stored
  scalars must be counted at their bit width.
- For non-Gaussian residuals the bound is lower by exp(−J/k), with J the negentropy in nats of
  P_S r′.
- The theorem assumes Gaussian Δ independent of r.

## 2. The concentration floor

**Theorem 2.** Let r ~ N(0, Σ_R) and Δ = Σ_Q^½ g as above. Let T be any set of eigen-indices. For
any encoder and sound certificate,

E h ≥ Σ_{i∈T} √γᵢ − (√(2H(c)) + 1/√2) · √(Σ_{i∈T} γᵢ).

**Proof.** By steps 1–2 of Theorem 1, E_g h(c, ·) ≥ E h_{K_T}(g_T), where K_T is the projection of
K′_c on span(vᵢ : i ∈ T). Work in those coordinates and drop the subscript T. Let Γ = diag(γᵢ) and
r′ = Γ^½ z with z standard Gaussian. Let A = Γ^(−½) K_T. Its standard Gaussian measure is
P ≥ P_c.

For any a ∈ A, g·Γ^½ a = (Γ^½ g)·g − (Γ^½ g)·(g − a) ≥ Σ √γᵢ gᵢ² − ‖Γ^½ g‖·‖g − a‖. Taking a to
approach the nearest point of A gives

h_{K_T}(g) ≥ Σ √γᵢ gᵢ² − ‖Γ^½ g‖ · dist(g, A).

Take expectations and apply Cauchy–Schwarz:
E h_{K_T}(g) ≥ Σ √γᵢ − √(Σ γᵢ) · √(E dist(g, A)²).

By the Gaussian isoperimetric inequality, Pr[dist(g, A) > t] ≤ Pr[Z > t − a], where Z is standard
normal and a = Φ⁻¹(1 − P). If P ≥ ½ then a ≤ 0. Otherwise P = Pr[Z > a] ≤ exp(−a²/2), so
a ≤ √(2 ln(1/P)). Hence dist(g, A) is stochastically at most √(2 ln(1/P_c)) + Z₊, and
√(E dist²) ≤ √(2 ln(1/P_c)) + 1/√2.

Average over cells. By concavity of the square root, Σ_c P_c √(2 ln(1/P_c)) ≤ √(2H(c)). ∎

Check: the per-cell inequality holds on centred and off-centre balls in 5, 50 and 500 dimensions
(`toyfloor.py` (4)).

## 3. The combined floor

**Theorem 3.** Let r ~ N(0, Σ_R), let S and T be disjoint sets of eigen-indices with |S| = k, and
let H(c) ≤ B ln 2. Then

E h ≥ (κ_k/√e) · 2^(−(B+1)/k) · (Π_{i∈S} γᵢ)^(1/2k) + Σ_{i∈T} √γᵢ
     − (√(2(B+1) ln 2) + 1/√2) · √(Σ_{i∈T} γᵢ).

**Proof.** Project on the span of S ∪ T as before and write points as (s, t). The S- and
T-coordinates of r′ are independent Gaussians with laws μ_S and μ_T. For a cell K with probability
P_c, let K_s = {t : (s, t) ∈ K} and G = {s : μ_T(K_s) ≥ P_c/2}.

*Mass of G.* P_c ≤ ∫ μ_T(K_s) dμ_S(s) ≤ μ_S(G) + P_c/2, so μ_S(G) ≥ P_c/2. The density of μ_S is
at most (2π)^(−k/2) (Π_S γᵢ)^(−½), so vol(G) ≥ (P_c/2)(2π)^(k/2)(Π_S γᵢ)^(½).

*Splitting.* For η > 0 choose s* ∈ G, a measurable function of g_S only, with
g_S·s* ≥ h_G(g_S) − η. Then h_K(g) ≥ g_S·s* + h_{K_{s*}}(g_T). The vector g_T is independent of
g_S and K_{s*} has μ_T-measure at least P_c/2. The per-cell bound in the proof of Theorem 2 gives
E[h_{K_{s*}}(g_T) | g_S] ≥ Σ_T √γᵢ − √(Σ_T γᵢ)(√(2 ln(2/P_c)) + 1/√2).

*S part.* By step 3 of Theorem 1,
E h_G(g_S) ≥ E‖g_k‖ (vol(G)/ω_k)^(1/k) ≥ E‖g_k‖ ω_k^(−1/k) √(2π) (Π_S γᵢ)^(1/2k) (P_c/2)^(1/k).

*Averaging.* Σ_c P_c (P_c/2)^(1/k) ≥ exp(−(H(c) + ln 2)/k) ≥ 2^(−(B+1)/k) by Jensen.
Σ_c P_c √(2 ln(2/P_c)) ≤ √(2(H(c) + ln 2)) ≤ √(2(B+1) ln 2). Note
E‖g_k‖ ω_k^(−1/k) √(2π) = κ_k/√e. Let η → 0. ∎

The factor 1/√e against Theorem 1 is the price of bounding volume by peak density.

## 4. Instance-optimality for a fixed stream

**Setting.** The encoder is a fixed map: means, bases, allocation and cluster assignment are fixed
parameters, not functions of the corpus. Each item is stored as a stream of nested levels with
cumulative sizes c₀ < c₁ < … < c_L, the last level determining x. F_ℓ(x) is the set of unit
vectors the encoder maps to the same first ℓ levels in the same cluster as x. Define ℓ*(x, q, t) as
the least ℓ such that q·x′ ≥ t for all x′ ∈ F_ℓ(x), or q·x′ < t for all x′ ∈ F_ℓ(x).

**Theorem 4.** Any deterministic algorithm that reads level prefixes of items and returns S(q, t)
exactly on every corpus reads at least ℓ*(x, q, t) levels of every item x. An algorithm that reads
levels in order and stops when the exact range of q·x′ over F_ℓ(x) excludes t reads exactly ℓ*.

**Proof.** Suppose the algorithm reads ℓ < ℓ* levels of x on some corpus. Some x′ ∈ F_ℓ(x) lies on
the other side of t. Replace x by x′. The encoder is a fixed map, so every byte the algorithm reads
is unchanged, and so is its output. The correct output differs in whether the item is included.
The second statement is the definition of ℓ*. ∎

**Corollary (byte-granular readers).** A deterministic reader of the same streams that may stop at
any byte needs more than c_{ℓ*−1} bytes of x. The level reader therefore costs at most
max over ℓ of c_ℓ/(c_{ℓ−1} + 1) times the minimum. For level sizes 0, 4, 8, 16, 32, 48, 96, 192,
384, 576 bytes this is below 2 from the second level on and 4 at the first level. Where the
1,536-byte full vector is read in addition to the 576-byte prefix it is 2112/577 ≈ 3.66.

The theorem compares readers of one stream, for d = 0, with the exact-range certificate. It says
nothing about other encodings or about a looser certificate such as weighted Cauchy–Schwarz.

## 5. Guarantees of the randomised scheme

**Theorem 5 (adaptive queries, d = 0).** Suppose Pr_ω[R(q, t, ω) ≠ S(q, t)] ≤ ε for every (q, t)
fixed independently of ω. Let a client issue T queries, each a function of its own coins and the
earlier answers, with the coins independent of ω. Then Pr[some answer is wrong] ≤ Tε.

**Proof.** Fix the coins. Let (q*ᵢ, t*ᵢ) be the queries the client issues when answered by the
exact oracle S. They do not depend on ω. Let Eᵢ = {R(q*ᵢ, t*ᵢ, ω) = S(q*ᵢ, t*ᵢ)}. On
E₁ ∩ … ∩ E_{i−1} the real answers so far equal the exact ones, so the real i-th query is
(q*ᵢ, t*ᵢ). A first wrong answer at step i therefore implies the complement of Eᵢ. Apply the union
bound and average over coins. ∎

The proof uses that a correct answer is unique, so it does not extend to d > 0.

**Theorem 6 (leakage).** If the client also observes side information whose whole transcript takes
at most 2^L values, then Pr[some answer is wrong] ≤ 2^L·T·ε. With ε proportional to exp(−c²/2),
replacing c² by c² + 2L ln 2 restores the bound.

**Proof.** For each transcript value z define the ideal sequence by running the client against S
with side information z. These are 2^L·T queries fixed independently of ω. If all are answered
correctly, the real run follows the ideal sequence of the real transcript and every answer is
correct. Apply the union bound. ∎

**Theorem 7 (query-time randomness does not help).** Fix the bytes read for an item, and let F be
the set of vectors that store those bytes. If min_F q·x′ < t ≤ max_F q·x′, any decoder, randomised
or not, that reads no more errs with probability at least ½ on some corpus.

**Proof.** Take x₁, x₂ ∈ F on opposite sides of t. The decoder's input is identical on the two
corpora, so it includes the item with the same probability p. Its error probabilities are 1 − p
and p. ∎

**Remark (public seed).** Theorems 5 and 6 require the client's coins, and the corpus, to be
independent of ω. A party that reads the seed is outside Theorem 5, makes Theorem 6 vacuous, and
faces a scheme that is deterministic to them, so Theorem 7 applies.

**Lemma 8 (law of the block error).** With the unbiased rescaled estimator
ŷ_est = (‖y‖²/⟨code, Ry⟩)·Rᵀcode, the error E = ŷ_est − y is orthogonal to y, has norm
A = ‖y‖ tan φ, and E = A·u with u uniform on the unit sphere of y^⊥ and independent of A. This
holds for any codebook.

**Proof.** ⟨ŷ_est, y⟩ = ‖y‖², so E ⟂ y and ‖E‖ = ‖y‖ tan φ. Let O be a rotation fixing y. R·Oᵀ is
Haar, maps y to Ry, and so gives the same code and the same A. Its estimate is O·ŷ_est. So
conditional on Ry the law of E is invariant under every rotation fixing y. ∎

**Theorem 9 (class-uniform bound).** Let m_b ≥ 3 and let W be a k-dimensional subspace of the
block, fixed independently of ω. With probability at least 1 − exp(−c²/2), simultaneously for every
Δ,

|⟨E, Δ⟩| ≤ A · [ ‖P_W Δ‖ · (√(k/m_b) + c/√(m_b − 2)) + ‖P_W^⊥ Δ‖ ].

**Proof.** |⟨u, Δ⟩| ≤ ‖P_W u‖·‖P_W Δ‖ + ‖P_W^⊥ Δ‖ since ‖u‖ = 1. The map u ↦ ‖P_W u‖ is
1-Lipschitz, and E‖P_W u‖² = (k − ‖P_W y‖²/‖y‖²)/m_b ≤ k/m_b, so E‖P_W u‖ ≤ √(k/m_b). On the unit
sphere of an m_b-dimensional space, a 1-Lipschitz F satisfies
Pr[F ≥ E F + t] ≤ exp(−(m_b − 2)t²/2) (Lévy; see Ledoux, *The Concentration of Measure
Phenomenon*, Theorem 2.3). Take t = c/√(m_b − 2). ∎

For a union of M such subspaces, replace c² by c² + 2 ln M. The bound holds for all Δ at once, so
it covers queries chosen with knowledge of ω, provided W and the items are independent of ω.
Check: the exact Beta law of ‖P_W u‖² stays below the bound in every case tried (`misc.py` (c)).

**Proposition 10 (exact tail).** Let u be uniform on the unit sphere of ℝᵐ, m ≥ 2. For
0 ≤ s ≤ 1/√2, Pr[|u₁| ≥ s] ≤ 2(1 − s²)^(m/2).

**Proof.** Let C = {u : u₁ ≥ s}. Its measure is the volume of the cone {τu : u ∈ C, 0 ≤ τ ≤ 1}
divided by ω_m. For a point τu of the cone, ‖τu − s·e₁‖² = τ² − 2τs·u₁ + s² ≤ τ² − 2τs² + s². This
is convex in τ. At τ = 1 it equals 1 − s². At τ = 0 it equals s², which is at most 1 − s² because
s ≤ 1/√2. So the cone lies in a ball of radius √(1 − s²), and Pr[u₁ ≥ s] ≤ (1 − s²)^(m/2). Double
for the two sides. ∎

The restriction is needed: for m = 2 the inequality fails from s ≈ 0.946. Since
(1 − s²)^(m/2) ≤ exp(−ms²/2), this sharpens the sub-Gaussian tail. The exact tail is the
regularised incomplete beta function I_{1−s²}((m−1)/2, ½). Solving it for the same failure
probability as 2 exp(−c²/2) shrinks c by 0.74 / 0.64 / 0.54 at m = 30 and 0.92 / 0.94 / 0.94 at
m = 767, for c = 6 / 8 / 10 (`constants.py`).

**Proposition 11 (several blocks).** For u uniform on the unit sphere of ℝᵐ,
E exp(λu₁) = ₀F₁(; m/2; λ²/4) ≤ exp(λ²/(2m)). Hence for independent blocks b with scales
a_b = A_b‖Δ_b‖ and dimensions m_b, Pr[|Σ_b a_b u_b| ≥ c·√(Σ_b a_b²/m_b)] ≤ 2 exp(−c²/2), and the
Chernoff bound with the exact function is at least as tight.

**Proof.** The series Σ_j (λ²/4)^j / (j!·(m/2)_j) is the stated function. Since
(m/2)_j ≥ (m/2)^j, each term is at most (λ²/(2m))^j/j!. Blocks use independent rotations, so by
Lemma 8 the u_b are independent given the A_b. Apply Chernoff. ∎

Check: the series matches Monte Carlo at m = 5 and 30 (`constants.py`).

**Proposition 12 (union over undecided items).** Let U be a set of (item, level) pairs that does
not depend on ω, such that every decision outside U is correct for every ω. If each randomised
interval fails with probability at most 2 exp(−c²/2), then
Pr[some decision is wrong] ≤ 2·|U|·exp(−c²/2).

**Proof.** A wrong decision occurs at a pair in U whose interval fails. U is fixed, so the union
bound applies. ∎

A deterministic certificate computed on seed-rotated codes depends on ω and cannot define U. U
depends on (q, t), so this bounds a reported failure probability.

**Proposition 13 (sub-bit sketch).** Keep the signs of the first k′ of n_b rotated coordinates.
Then cos φ = Σ_{i≤k′}|zᵢ| / (√k′·‖z‖) with z = Ry, and as k′ → ∞ with n_b ≥ k′,

tan²φ / (π n_b/(2k′) − 1) → 1 in probability.

The block error then has standard deviation close to ‖y‖‖Δ‖·√(π/(2k′) − 1/n_b).

**Proof.** z/‖z‖ has the law of G/‖G‖ for a standard Gaussian G. By the law of large numbers
(1/k′)Σ_{i≤k′}|Gᵢ| → √(2/π) and ‖G‖²/n_b → 1, so cos²φ·π n_b/(2k′) → 1. The denominator
π n_b/(2k′) − 1 is at least π/2 − 1, so the ratio converges. By Lemma 8 the error variance is
A²‖P_{y⊥}Δ‖²/(n_b − 1). ∎

E tan²φ is infinite for k′ ≤ 2. The mean exceeds the formula by 78% at k′ = 4, 11% at k′ = 16 and
2% at k′ = 64 (`misc.py` (b)).

## 6. Conjectures

- **Full floor.** E h ≥ Σᵢ √min(γᵢ, θ) with θ from Σᵢ ½ log₂⁺(γᵢ/θ) = B. Missing: a single
  argument giving the head term k√θ and the tail Σ_{i>k} √γᵢ together without the losses of
  Theorem 3. On the two measured spectra the best proved bound is 0.70 of it at 0 bytes and 0.93
  or more from 96 bytes (`agent1/theory.py`; not re-run by the referee).
- **Gaussian residuals are the worst case.** The true optimum, not only the bound of Theorem 1, is
  no larger for a non-Gaussian residual with the same covariance. Missing: an achievability
  argument at low rate. At high rate it follows from the tightness of Theorem 1.
- **Non-Gaussian queries.** Theorem 1 for Δ with covariance Σ_Q but not Gaussian. Missing:
  replacements for E[g_⊥ | g_S] = 0 and for rotation invariance in the Urysohn step. It is false
  without some condition, for example for Δ supported on finitely many directions.
- **√k is unavoidable for a class unknown at build.** Known: for a uniformly random k-dimensional W
  independent of the error, E‖P_W(x − x̂)‖² = (k/n)‖x − x̂‖² for any decoder. Missing: a lower bound
  for every W, or a minimax statement over decoders.
- **Tail without the factor 2.** Pr[|u₁| ≥ s] ≤ (1 − s²)^(m/2) for s ≤ 1/√2. It holds on a grid for
  2 ≤ m ≤ 5000, with equality at m = 2, s = 1/√2 (`constants.py`). Missing: a proof.
