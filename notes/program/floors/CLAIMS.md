# Floors track: claims

Every claim of the floors track, with its tag (tags of `notes/README.md`), its proof or the precise gap, and its
numerical checks. This is the version revised after the two referees' reports and the librarian's
(`referee/referee-core.md`, `referee/referee-extensions.md`, `../library/floors-novelty.md`) and the independent
numerical attack (`../stage0-floor-check/RESULTS.md`). The text the referees saw is `CLAIMS_before_referees.md`. No
tag has been raised here; that is the PI's to do. Two tags have changed because a result turned out to be published
(L1, B2b). The last section, "Changes after the referees", lists what changed in each claim. The log of how each
result was reached, including what failed, is `NOTEBOOK.md`. Scripts are in this directory.

The file has three parts: "For the notes" (what is to enter `notes/02-floors.md`), the claims as refereed with the
repairs made (sections 0 to 15), and "Changes after the referees".

---

## For the notes

This section is written to be transcribed into `notes/02-floors.md` as it stands. It states Lemmas 1 to 5, Theorem
F1, Theorem F2 and Corollary F2a in chapter 0's notation, with every hypothesis and the full proofs. Both referees'
repairs are in it. Tags are as they stand in this file; the PI raises them. Lemma 1 is a published result and is
given as a cited lemma with its short proof. The sections after this one hold the same results as the referees saw
them, with the repairs marked, and the claims that are not for the chapter.

### N.0 Setting and local symbols

**The model.** Chapter 0's setting with α = 1. The residual is r ~ N(0, Σ_R), with mean zero. The offset is
Δ = Σ_Q^½ g, with g a standard Gaussian vector in ℝⁿ independent of r. Σ_Q and Σ_R are positive semidefinite; they
need not commute and need not be invertible.

**The message.** **c** (written c below) takes countably many values. It is a function of r and, if the encoder is
randomised, of randomness independent of Δ; that is, the pair (r, c) is independent of Δ. P_c = Pr[c = c], and
H(c) is the entropy of the message in nats. If the message is B bits of fixed length, or a uniquely decodable
code of expected length B bits, H(c) ≤ B ln 2.

**The certificate.** For each value c and each Δ the certificate gives an interval of half-width h(c, Δ) ≥ 0. Three
things are assumed of it.
1. It depends on the item only through c and on the query only through Δ.
2. h(c, ·) is a measurable function of Δ.
3. It is sound: for every c with P_c > 0 and every Δ, the interval contains Δ·r for almost every r, under the law
   of r given c. This is weaker than soundness for every r that the encoder maps to c, so a floor proved under it
   holds under that definition too.

E h is the mean of h(c, Δ) over the message and the offset. By independence E h = Σ_c P_c E_g h(c, Σ_Q^½ g).

**Local symbols.** These are added to chapter 0's.
- *Eigen-coordinates.* B = Σ_Q^½ Σ_R Σ_Q^½ = V Γ Vᵀ with V orthogonal and Γ = diag(γ₁ ≥ … ≥ γ_m > 0, 0, …, 0); m is
  the rank of B. For i ≤ m, zᵢ = γᵢ^(−½) vᵢᵀ Σ_Q^½ r. Then z = (z₁, …, z_m) ~ N(0, I_m), z is a function of r, and
  with g replaced by Vᵀg (again standard Gaussian, and independent of z and c)

      Δ·r = Σ_{i≤m} √γᵢ gᵢ zᵢ   almost surely,

  because the components of Σ_Q^½ r along the eigenvectors of eigenvalue 0 have variance 0. Every sum over i below
  runs over i ≤ m. N_d denotes the standard Gaussian law N(0, I_d) on ℝ^d.
- *Cells.* ν_c is the law of z given c = c, for P_c > 0. P_c ν_c ≤ N_m as measures, so ν_c has a density and
  KL(ν_c ‖ N_m) ≤ ln(1/P_c). K_c is the closed support of ν_c and K′_c = Γ^½ K_c, so that h_{K′_c}(y) is the supremum
  of Σᵢ √γᵢ yᵢ zᵢ over z in K_c. I(z; c) = Σ_c P_c KL(ν_c ‖ N_m) is the mutual information in nats, and
  I(z; c) ≤ H(c).
- *Entropy.* For a law or a random vector, h(·) is the differential entropy in nats, with values in [−∞, ∞). It is
  told apart from the half-width h(c, Δ) and the support function h_A(y) by its argument.
- *Correlation of a law on ℝ.* For a law μ on ℝ with quantile function Q_μ, T_μ = Q_μ∘Φ is the non-decreasing map
  that sends N(0, 1) to μ (Φ is the standard normal distribution function, φ its density). C(μ) = E[g·T_μ(g)] for
  g ~ N(0, 1), and m₂(μ) is the second moment.
- *Triangular coupling.* For a law ν on ℝ^d with a density and an order of the coordinates, T^ν is the
  Knothe–Rosenblatt map from the standard Gaussian law to ν: its i-th coordinate is T_μ(gᵢ), with μ the conditional
  law of the i-th coordinate under ν given that the earlier coordinates equal T^ν_<i(g_<i). Cᵢ(ν) = E[gᵢ·T^νᵢ(g)].
- *The floors, as functions of a rate H ≥ 0 in nats.*
  - W(H) = min{Σᵢ √γᵢ sᵢ : sᵢ ∈ (0, 1], Σᵢ ln(1/sᵢ) ≤ H}. For a set T of indices, W_T is the same over i ∈ T only, and
    W_∅ = 0.
  - For a set S of k ≥ 1 indices, with T = S^c its complement in {1, …, m},
    V_S(H) = min over 0 ≤ u ≤ H of κ_k·(Π_{i∈S} γᵢ)^(1/2k)·e^(−(H−u)/k) + W_T(u). Put V_∅ = W.
  - Both are extended to H = ∞ by their limit, which is 0.

**The width of a cell.** Fix c with P_c > 0 and Δ. A linear function of z is continuous, so its essential supremum
under ν_c is its supremum over K_c. Soundness therefore gives 2h(c, Δ) ≥ h_{K′_c}(g) + h_{K′_c}(−g), and g and −g
have the same law, so

    E_g h(c, ·) ≥ E_g h_{K′_c}(g).                                                         (N.1)

This is step 1 of Theorem 1. h_{K′_c} is a supremum of linear functions, so it is Borel and dominates a linear
function; its mean is well defined in (−∞, ∞].

### N.1 Lemma 1: entropy against the Gaussian correlation of the quantile function [cited]

**Lemma 1.** Let μ be a law on ℝ with a density f and a finite second moment. Then 0 < C(μ) ≤ √Var(μ) and

    h(μ) ≤ ln(√(2πe)·C(μ)),     equivalently     KL(μ ‖ N(0, 1)) ≥ ½ m₂(μ) − ½ − ln C(μ).

Equality holds when μ is Gaussian.

**Source.** It is the case n = 1 of the dimensional Talagrand inequality for the Gaussian measure: Bolley, Gentil
and Guillin, *Dimensional improvements of the logarithmic Sobolev, Talagrand and Brascamp–Lieb inequalities*, Ann.
Probab. 46 (2018), Theorem 2.1 (arXiv:1507.01086); first proved, by semigroup methods, by Bakry, Bolley and Gentil
(Probab. Theory Related Fields 154, 2012). Source read by the librarian (`program/library/floors-novelty.md`,
section 1), not by the author of this section. Their theorem with the standard Gaussian reads
½W₂²(N₁, μ) ≤ ½(m₂ − 1) + 1 − exp(½(m₂ − 1) − KL(μ ‖ N₁)). On the line the monotone coupling is optimal, so
W₂² = m₂ + 1 − 2C(μ), and the inequality becomes C(μ) ≥ exp(½(m₂ − 1) − KL), which is the second form. Their proof
is the one below (transport, Jensen, Gaussian integration by parts).

**Proof.** Write T = T_μ, F_μ for the distribution function of μ, and g ~ N(0, 1).
1. *T(g) has law μ, and E T(g)² = m₂.* The quantile transform of a uniform variable has law μ.
2. *C = ∫ φ dτ*, where τ is the Lebesgue–Stieltjes measure of the non-decreasing function T. For x > 0,
   T(x) − T(0) = τ([0, x)) and x·φ(x) ≥ 0, so by Tonelli
   E[g(T(g) − T(0)); g > 0] = ∫_{[0,∞)} τ(dy) ∫_{x>y} xφ(x) dx = ∫_{[0,∞)} φ dτ. The half-line g < 0 is the same with
   both factors negative, and E[g·T(0)] = 0. C is finite by Cauchy–Schwarz, C = E[g(T(g) − E T(g))] ≤ √Var(μ), and
   positive because μ is not a point mass.
3. *C ≥ E T′(g).* T is differentiable almost everywhere and τ = T′(x)dx + τ_s with τ_s ≥ 0 singular (Lebesgue).
4. *h(μ) = ½ ln(2πe) + E ln T′(g).* F_μ is absolutely continuous with F_μ′ = f off a Lebesgue-null set N₁. Then
   μ(N₁) = 0, so Pr[T(g) ∈ N₁] = 0. T is differentiable off a null set N₂. F_μ is continuous, so F_μ(T(x)) = Φ(x)
   for every x. For x outside N₂ ∪ T⁻¹(N₁) the chain rule gives f(T(x))·T′(x) = φ(x). So T′(x) > 0 and
   ln T′(x) = ln φ(x) − ln f(T(x)) for almost every x. Take expectations: E ln φ(g) = −½ ln(2πe), and
   E[−ln f(T(g))] = −∫ f ln f = h(μ), which is well defined in [−∞, ∞) because
   f ln(1/f) ≤ φ − f + f·(½ ln 2π + y²/2).
5. *Jensen.* E ln T′(g) ≤ ln E T′(g) ≤ ln C by step 3. With step 4, h(μ) ≤ ½ ln(2πe) + ln C.

The second form is the first with KL(μ ‖ N(0, 1)) = −h(μ) + ½ ln 2π + ½ m₂. ∎

### N.2 Lemma 2: the same along a triangular coupling [proved, unrefereed]

**Lemma 2.** Let ν be a law on ℝ^d with a density, finite second moments and h(ν) > −∞. For any order of the
coordinates:
1. T^ν(g) has law ν when g is standard Gaussian in ℝ^d;
2. 0 < Cᵢ(ν) ≤ √(E_ν zᵢ²) for every i;
3. h(ν) ≤ Σᵢ ln(√(2πe)·Cᵢ(ν)).

**Proof.** *The map.* Regular conditional laws exist. The conditional distribution functions are jointly
measurable, and so are their quantile functions, because {Q(u | z_<i) ≤ t} = {u ≤ F(t | z_<i)}. Given g_<i, Φ(gᵢ) is
uniform and independent of g_<i, so T^νᵢ(g) has the conditional law of the i-th coordinate given that the earlier
ones equal T^ν_<i(g_<i). By induction T^ν(g) has law ν. No continuity of the conditional distribution functions is
used, because the map goes from the Gaussian law, which has no atoms, to ν.

*The entropy.* ν has a density, so for ν-almost every z_<i the conditional law μ of the i-th coordinate has a
density, and it has a finite second moment. The chain rule holds for KL(ν ‖ N_d), with non-negative terms:
KL(ν ‖ N_d) = Σᵢ ∫ KL(ν_{i|<i}(· | z_<i) ‖ N₁) dν. Apply the second form of Lemma 1 to each conditional law and
integrate:

    KL(ν ‖ N_d) ≥ Σᵢ [½ E_ν zᵢ² − ½ − E ln C(ν_{i|<i}(· | T^ν_<i(g_<i)))].

The last expectation is well defined: its positive part by ln C ≤ ½(m₂ − 1), its negative part by Lemma 1 and the
finiteness of the conditional KL. By Jensen for the logarithm,
E ln C(ν_{i|<i}) ≤ ln E C(ν_{i|<i}) = ln Cᵢ(ν), the equality because E[gᵢ T^νᵢ(g) | g_<i] = C(ν_{i|<i}). With
KL(ν ‖ N_d) = −h(ν) + (d/2) ln 2π + ½ E_ν‖z‖² this is item 3. Item 2 is Cauchy–Schwarz and the positivity in
Lemma 1. ∎

**Relation to the published form.** Bolley, Gentil and Guillin's theorem in d dimensions uses the Brenier map and
gives h(ν) ≤ d·ln(√(2πe)·C*/d), with C* the largest E⟨g, x⟩ over couplings of N_d and ν. Lemma 2 implies it: by
the inequality of the means Σᵢ ln Cᵢ ≤ d·ln(Σᵢ Cᵢ/d), and Σᵢ Cᵢ ≤ C* because the triangular coupling is one
coupling. What Lemma 2 adds is one correlation per coordinate, which the water-filling of Theorem F1 needs. The
librarian did not find it stated in this form.

### N.3 Lemma 3: the water-filling programme [proved, unrefereed]

**Lemma 3.** Let aᵢ > 0 and H ≥ 0, and let λ ∈ (0, max aᵢ] solve Σᵢ ln⁺(aᵢ/λ) = H. Then

    min{Σᵢ aᵢ sᵢ : sᵢ ∈ (0, 1], Σᵢ ln(1/sᵢ) ≤ H} = Σᵢ min(aᵢ, λ),

attained at sᵢ* = min(1, λ/aᵢ). With aᵢ = √γᵢ and θ = λ²: W(H) = Σᵢ √min(γᵢ, θ), where Σᵢ ½ ln⁺(γᵢ/θ) = H.

**Proof.** For H > 0, λ exists and is unique because λ ↦ Σᵢ ln⁺(aᵢ/λ) is continuous and strictly decreasing on
(0, max aᵢ], from ∞ to 0; for H = 0 take λ = max aᵢ. Let L(s) = Σᵢ aᵢ sᵢ − λ Σᵢ ln sᵢ. In each coordinate L is convex
on (0, 1] with derivative aᵢ − λ/sᵢ, so it is least at sᵢ*. For a feasible s, Σᵢ ln sᵢ + H ≥ 0, so
Σᵢ aᵢ sᵢ ≥ L(s) − λH ≥ L(s*) − λH = Σᵢ aᵢ sᵢ* + λ(Σᵢ ln(1/sᵢ*) − H) = Σᵢ aᵢ sᵢ*. ∎

### N.4 Lemma 4: properties of the floors [proved, unrefereed]

Let Φ(H) = min{Σᵢ √γᵢ sᵢ : sᵢ ∈ (0, 1], Σᵢ k(sᵢ) ≤ H} with k(s) = ½(s² − 1) − ln s. (Φ is the floor of Conjecture A;
it is used here only for comparison.)

**Lemma 4.**
1. Φ, W and V_S are convex and non-increasing in H, the minima are attained, and Φ(0) = W(0) = F.
2. W(H) ≥ Φ(H).
3. Φ(H) ≥ F − √(H·Σᵢ γᵢ).
4. W(H) ≥ F − min(√γ₁·H, √(H·Σᵢ γᵢ)). If γ₁ > γ₂ and H ≤ ½ ln(γ₁/γ₂), then W(H) = F − √γ₁(1 − e^(−H)).
5. V_S(H) ≥ max(W_T(H), κ_k (Π_{i∈S} γᵢ)^(1/2k) e^(−H/k)), with T = S^c.
6. V_S(H) ≥ κ_k (Π_{i∈S} γᵢ)^(1/2k) e^(−H/k) + W_T(H), with T = S^c. The same holds with T replaced by any subset
   of S^c.

**Proof.** (1) Φ and W are the least values of a linear objective over a set that is jointly convex in (s, H),
because k and −ln are convex. Each term of the constraint is non-negative on (0, 1] and tends to infinity as
s → 0, so the feasible set is compact and the minimum is attained. k(s) = 0 or ln(1/s) = 0 on (0, 1] only at
s = 1, so Φ(0) = W(0) = F. V_S is the infimal convolution on [0, ∞) of u ↦ W_T(u) and v ↦ κ_k(Π_S γᵢ)^(1/2k)e^(−v/k),
both convex and non-increasing. (2) k(s) ≤ ln(1/s) on (0, 1], so W minimises over a smaller set. (3) k(1) = k′(1) = 0
and k″ = 1 + s⁻² ≥ 2, so k(s) ≥ (1 − s)². A feasible s has Σᵢ(1 − sᵢ)² ≤ H, and
Σᵢ √γᵢ(1 − sᵢ) ≤ √(Σᵢ γᵢ)·√H by Cauchy–Schwarz. (4) Use Lemma 3. For a = √γᵢ > λ: a − λ ≤ a ln(a/λ), which sums to
at most √γ₁·H; and ((a − λ)/a)² ≤ 1 − λ/a ≤ ln(a/λ), so Σᵢ(√γᵢ − λ)⁺ ≤ √(Σᵢ γᵢ)·√H by Cauchy–Schwarz. The exact form
is Lemma 3 with one coded coordinate. (5) and (6) In the minimand of V_S the head term increases with u, so it is
at least its value at u = 0; and W_T(u) ≥ W_T(H) for u ≤ H, because W_T is non-increasing. This gives (6), and (5)
follows because both terms are non-negative. For a subset T′ of S^c, W_{S^c} ≥ W_{T′}: restricting a feasible point
of the larger programme to T′ leaves it feasible and drops non-negative terms. ∎

### N.5 Lemma 5: κ_k ≥ k [proved, unrefereed]

**Lemma 5.** κ_k ≥ k for every k ≥ 1.

**Proof.** Let ν be uniform on the ball of radius ρ in ℝᵏ. T^ν(g) lies in the ball almost surely, so
ρ‖g‖ ≥ g·T^ν(g) and ρ·E‖g_k‖ ≥ Σᵢ Cᵢ(ν). By Lemma 2, Σᵢ ln(√(2πe)·Cᵢ(ν)) ≥ h(ν) = ln(ω_k ρᵏ), and by the inequality
of the means Σᵢ Cᵢ ≥ k(Πᵢ Cᵢ)^(1/k) ≥ k·ω_k^(1/k)·ρ/√(2πe). So E‖g_k‖·√(2πe) ≥ k·ω_k^(1/k), which is κ_k ≥ k. ∎

The core referee gives a second proof that does not use Lemma 2 (Wendel's inequality for E‖g_k‖ and Stirling's
lower bound for ω_k; `program/floors/referee/referee-core.md`, stage 2). The least value of κ_k − k is √e − 1 = 0.649, at
k = 1.

### N.6 Theorem F1: the water-filling floor [proved, unrefereed]

**Theorem F1.** Under the model, the message and the certificate of N.0,

    E h ≥ W(I(z; c)) ≥ W(H(c)) = Σᵢ √min(γᵢ, θ),      Σᵢ ½ ln⁺(γᵢ/θ) = H(c).

More precisely, when I(z; c) < ∞ there are numbers C̄ᵢ ∈ (0, 1] with E h ≥ Σᵢ √γᵢ C̄ᵢ and Σᵢ ln(1/C̄ᵢ) ≤ I(z; c).
When I(z; c) = ∞ the statement is E h ≥ 0.

With H(c) ≤ B ln 2 this is the "full floor" conjecture of the consolidated proofs, section 6. It holds for every
n, and for Σ_Q and Σ_R that need not commute.

**Proof.** Let I(z; c) < ∞.
1. *Width.* (N.1): E_g h(c, ·) ≥ E_g h_{K′_c}(g) for every c with P_c > 0.
2. *Coupling.* P_c ν_c ≤ N_m, so ν_c has a density at most (2π)^(−m/2)/P_c, finite second moments, and
   h(ν_c) ≥ ln(P_c(2π)^(m/2)) > −∞. Let T^c = T^{ν_c} (Lemma 2). T^c(g) has law ν_c, so it lies in K_c almost surely.
   It is a function of the same g at which the support function is taken, so pointwise
   h_{K′_c}(g) ≥ Σᵢ √γᵢ gᵢ T^cᵢ(g), and E_g h_{K′_c}(g) ≥ Σᵢ √γᵢ Cᵢ(ν_c). The right side is integrable:
   E|gᵢ T^cᵢ(g)| ≤ √(E_{ν_c} zᵢ²) ≤ P_c^(−½).
3. *Entropy.* Lemma 2: h(ν_c) ≤ Σᵢ ln(√(2πe)·Cᵢ(ν_c)).
4. *Averaging.* The cell laws mix back to the Gaussian, Σ_c P_c ν_c = N_m. So
   Σ_c P_c h(ν_c) = h(z | c) = h(z) − I(z; c) = (m/2) ln(2πe) − I(z; c). With step 3,
   Σᵢ Σ_c P_c ln Cᵢ(ν_c) ≥ −I(z; c). Put C̄ᵢ = Σ_c P_c Cᵢ(ν_c). By Jensen for the logarithm,
   Σᵢ ln C̄ᵢ ≥ −I(z; c).
5. *C̄ᵢ ≤ 1.* By Lemma 2 and Jensen for the square root,
   C̄ᵢ ≤ Σ_c P_c √(E_{ν_c} zᵢ²) ≤ √(Σ_c P_c E_{ν_c} zᵢ²) = √(E zᵢ²) = 1.
6. E h = Σ_c P_c E_g h(c, ·) ≥ Σᵢ √γᵢ C̄ᵢ, and C̄ is feasible for the programme of W at rate I(z; c), so
   E h ≥ W(I(z; c)). W is non-increasing and I(z; c) ≤ H(c). The closed form is Lemma 3. ∎

Where the proof uses that the cell laws mix back to N(0, I_m): in step 4, through h(z | c) = h(z) − I(z; c), and
in step 5. A bound proved cell by cell cannot use it, and loses for that reason (Conjecture A's Φ).

**How tight.** The inequalities of steps 3 to 6 are equalities when every ν_c is N(m_c, diag(sᵢ²)) with the same
sᵢ, which no cell is. For a real code the slack between E h and W is the product of the coupling step (the
supremum over a cell against the correlation with the cell's own law) and Lemma 2's gap for the cell law: for an
interval cell at high rate, √2 times √(e/2). Computed codes come close in high dimension at low rate: shells of
the norm reach E h / W = 1.00055 at n = 10⁴ for an isotropic spectrum (the core referee), and a grid on one head
coordinate with a grid on the tail's norm reaches 1.0026 at 2,001 eigenvalues (the falsifier) [computed]. For an
isotropic spectrum Ingber, Courtade and Weissman's scheme reaches the level of W with probability tending to 1 as
n → ∞, at every rate; its mean width is infinite, so that is tightness in probability and not of F1. A proof that
some code attains W(H)(1 + o(1)) in the mean is open.

### N.7 Theorem F2: Theorem 1's constant on the head, the tail in full [proved, unrefereed]

**Theorem F2.** Under the model, the message and the certificate of N.0, for every set S of k ≥ 1 indices in
{1, …, m}, with T = S^c,

    E h ≥ V_S(I(z; c)) ≥ V_S(H(c)),
    V_S(H) = min over 0 ≤ u ≤ H of { κ_k·(Π_{i∈S} γᵢ)^(1/2k)·e^(−(H−u)/k) + W_T(u) }.

T may be empty, and then V_S(H) = κ_m (Πᵢ γᵢ)^(1/2m) e^(−H/m). When I(z; c) = ∞ the statement is E h ≥ 0.

**Corollary F2a.** Let H = H(c) > 0, let θ solve Σᵢ ½ ln⁺(γᵢ/θ) = H, and let k = #{i : γᵢ > θ}, so that eigenvalues
equal to θ are in the tail. Then k ≥ 1 and

    E h ≥ κ_k·√θ + Σ_{i>k} √γᵢ.

This is the full floor with κ_k in place of k on the head. It exceeds Σᵢ √min(γᵢ, θ) by (κ_k − k)√θ.

**Proof of F2.** The proof uses one fact from Theorem 1, its step 3 (Urysohn's inequality): for a bounded Borel set
G in ℝᵏ and g_k standard Gaussian, E h_G(g_k) ≥ E‖g_k‖·(vol(G)/ω_k)^(1/k). Let I(z; c) < ∞, and order the
coordinates with those of T first.
1. *Tail first.* Fix c with P_c > 0. Let ν_{c,T} be the marginal of ν_c on the T-coordinates and T^c its triangular
   map (Lemma 2 in ℝ^T; the density of ν_{c,T} is at most (2π)^(−|T|/2)/P_c). Put z_T = T^c(g_T). It has law ν_{c,T}
   exactly. It is a function of g_T, so it is independent of g_S.
2. *Head on the fibre.* Let K_c^{z_T} = {s : (s, z_T) ∈ K_c}. Since ν_c(K_c) = 1, for ν_{c,T}-almost every z_T the
   conditional law ν_{c,S|z_T} has a density and is carried by K_c^{z_T}; and z_T has law ν_{c,T}, so this holds at
   T^c(g_T) almost surely. For every s in the fibre (s, z_T) ∈ K_c, so pointwise in g = (g_T, g_S)

       h_{K′_c}(g) ≥ Σ_{i∈T} √γᵢ gᵢ (z_T)ᵢ + sup over s ∈ K_c^{z_T} of Σ_{i∈S} √γᵢ gᵢ sᵢ.

   No point of the fibre is selected. The supremum is a Borel function of (z_T, g_S): K_c is a countable union of
   compact sets, and the supremum over the fibre of a compact set is upper semicontinuous.
3. *Urysohn.* Given g_T the fibre is a fixed closed set and g_S is standard Gaussian in ℝᵏ. Apply Urysohn's
   inequality to the image of the fibre under s ↦ (√γᵢ sᵢ)_{i∈S}, whose volume is (Π_{i∈S} γᵢ)^½ vol(K_c^{z_T}):

       E[sup over the fibre | g_T] ≥ E‖g_k‖·ω_k^(−1/k)·(Π_{i∈S} γᵢ)^(1/2k)·vol(K_c^{z_T})^(1/k).

   An unbounded fibre makes the left side infinite: a support function that is finite in every direction belongs
   to a bounded set, and otherwise the directions in which it is finite form a convex cone that is not the whole
   space, so it is +∞ on a set of Gaussian measure at least ½. The uniform law maximises entropy on a set, so
   ln vol(K_c^{z_T}) ≥ h(ν_{c,S|z_T}).
4. *Jensen in the cell.* z_T has law ν_{c,T}, so E exp(h(ν_{c,S|z_T})/k) ≥ exp(a_c/k), where
   a_c = ∫ h(ν_{c,S|z_T}) dν_{c,T} is the conditional entropy of z_S given z_T in the cell. With (N.1), step 2, and
   the coupling step of F1 on the tail,

       E_g h(c, ·) ≥ Σ_{i∈T} √γᵢ Cᵢ(ν_{c,T}) + E‖g_k‖·ω_k^(−1/k)·(Π_{i∈S} γᵢ)^(1/2k)·e^(a_c/k).

5. *Entropies.* b_c := h(ν_{c,T}) ≤ Σ_{i∈T} ln(√(2πe)·Cᵢ(ν_{c,T})) by Lemma 2, and a_c + b_c = h(ν_c) by the chain rule.
   All three are finite: the densities are bounded and the second moments finite.
6. *Averaging over cells.* Let ā and b̄ be the means of a_c and b_c over c, and C̄ᵢ the mean of Cᵢ(ν_{c,T}). By Jensen
   Σ_c P_c e^(a_c/k) ≥ e^(ā/k) and b̄ ≤ Σ_{i∈T} ln(√(2πe)·C̄ᵢ). Also ā + b̄ = h(z | c) = (m/2) ln(2πe) − I(z; c), and
   C̄ᵢ ≤ 1 as in F1. Put u = Σ_{i∈T} ln(1/C̄ᵢ) ≥ 0. Then b̄ ≤ (|T|/2) ln(2πe) − u, so
   ā ≥ (k/2) ln(2πe) − (I(z; c) − u), and, because E‖g_k‖·ω_k^(−1/k)·√(2πe) = κ_k,

       E h ≥ κ_k·(Π_{i∈S} γᵢ)^(1/2k)·e^(−(I(z;c)−u)/k) + Σ_{i∈T} √γᵢ C̄ᵢ ≥ κ_k·(Π_{i∈S} γᵢ)^(1/2k)·e^(−(I(z;c)−u)/k) + W_T(u).

   Finally u ≤ I(z; c), because ā = h(z_S | z_T, c) ≤ h(z_S) = (k/2) ln(2πe). So E h ≥ V_S(I(z; c)), and V_S is
   non-increasing. ∎

The rate splits exactly, I(z; c) = I(z_T; c) + I(z_S; c | z_T), and the minimum over u lets the code choose the
split. The order matters: the tail point has exactly the cell's tail law, so entropies and second moments average
over it, and the head supremum is over a whole fibre at a point independent of the head Gaussian, so nothing is
selected.

**Proof of F2a.** H > 0 gives θ < γ₁, so k ≥ 1. Take S = {1, …, k}. By the water-filling equation
(Π_{i≤k} γᵢ)^(1/2k) e^(−H/k) = √θ. If k = m the tail is empty and V_S(H) = κ_m√θ. Otherwise
f(u) = κ_k (Π_{i≤k} γᵢ)^(1/2k) e^(−(H−u)/k) + W_T(u) is convex on [0, H]. If the largest eigenvalue of the tail is
repeated j times, Lemma 3 gives W_T(u) = j·√γ_{k+1}·e^(−u/j) + Σ_{i>k+j} √γᵢ for small u, so W_T′(0⁺) = −√γ_{k+1}
with or without ties. Hence f′(0⁺) = (κ_k/k)√θ − √γ_{k+1}. Now γ_{k+1} ≤ θ by the definition of k, and κ_k ≥ k
(Lemma 5), so f′(0⁺) ≥ 0 and the minimum is at u = 0: V_S(H) = κ_k√θ + Σ_{i>k} √γᵢ. ∎

**Remarks.**
- *The water-filling head is not always the best head.* Every S gives a floor. For γ = (4, 1, 1) and H = ln 2 the
  water-filling head is {1} and F2a gives κ₁ + 2 = 3.649, while S = {1, 2, 3} gives κ₃ = 4.091. On the measured
  spectra the best top-k head is a little larger than the water-filling one and the gain over W is 1 to 2%
  [computed, `program/floors/floors_eval.py`].
- *A seed shared by the encoder and the certificate.* If both read a seed ω independent of (r, Δ), the hypotheses
  hold conditionally on ω. W and V_S are convex (Lemma 4), so averaging over ω gives E h ≥ V_S(I(z; c | ω)) for
  every S, and I(z; c | ω) ≤ H(c | ω) ≤ H(c). This is about sound certificates only; a δ-valid certificate is
  outside it.
- *What F1 and F2 contain.* Corollary 1 of Theorem 1 (Gaussian residuals) is V_S ≥ κ_k(Π_S γᵢ)^(1/2k)e^(−H/k)
  (Lemma 4.5). Theorem 2 follows from F1: W ≥ W_T for every index set T, and Lemma 4.4 applied to W_T gives
  W_T(H) ≥ Σ_T √γᵢ − √(H·Σ_T γᵢ), which is above Theorem 2's bound. Theorem 3 follows from F2 and Lemma 4.6, with no
  added bit and no 1/√e. Theorem 1 itself does not follow: it covers any residual with a density on a subspace,
  and Corollary 2 covers shared information, while F1 and F2 are proved for Gaussian residuals. F2 is not
  independent of Theorem 1 either, because step 3 is Theorem 1's step 3 applied to a fibre.
- *What is not claimed.* Nothing for residuals that are not Gaussian, for offsets that are not Gaussian or that
  depend on the residual, or for real data. Stored real scalars must be counted at their bit width, as in
  Theorem 1.

**Prior statements of the formula.** The curve of W was published as an asymptotic identification rate for
compressed Gaussian data answering similarity queries with no false negative. Ingber, Courtade and Weissman (IEEE
Trans. Inf. Theory 61, 2015) prove it, with its converse, for isotropic laws as n → ∞, for the probability of the
answer "maybe"; Steiner, Dempfle, Ingber and Weissman (arXiv:1404.5173) give a finite-n lower bound on that
probability, also isotropic. Wu, Wang and Flierl (arXiv:2001.07941) state the anisotropic formula for
Σ_Q = Σ_R and do not prove its converse. Theorem F1 is, as far as the librarian's search goes, the first proof of
the converse at finite n, for covariances that need not commute, in the mean-width form. It does not claim the
formula. The nearest current work on inner products, Balasubramanian, Bhandari and Kasiviswanathan
(arXiv:2610.24890), bounds the mean squared error of an estimate and not a sound interval. Sources as read:
`program/floors/CLAIMS.md` section 6 and `program/library/floors-novelty.md`.

**Numerical checks** [computed]. A code's exact range is its smallest sound certificate, so a code whose exact
mean width fell below a floor would refute it. None did.
- The author (`program/floors/check_schemes.py`, `check_chain1d.py`): 2,500 product codes against every head set, smallest
  ratio 1.000040; 2,800 planar partitions that are not products, 1.135; head grids with shells of the dual norm
  on the tail, n up to 48, 1.030 against max_S V_S and 1.080 against W.
- The core referee (`program/floors/referee/core_*.py`): Lemma 2 in the plane with an adversary, smallest gap 0.091 nats;
  the inequality in which head and tail meet (steps 1 to 4 of F2 for one cell), with an adversary over densities,
  smallest ratio 1.0006; whole codes in three dimensions against all eight head sets, 1.237; F2a with ties by
  brute force over u, equal to 5·10⁻¹⁶; product codes to n = 10⁴, E h / W = 1.00055.
- The falsifier (`program/stage0-floor-check/RESULTS.md`): nothing refuted; a real code within 1.0026 of W and
  within 1.0005 of max_S V_S (there S is all indices, which is Theorem 1), and within 1.0023 of V_S with S the
  head.
- Not tested by anyone: a high-dimensional code whose cells couple the head to the tail.

---

## Summary of the claims

"Referee" gives the verdict of the report that covered the claim: core (`referee/referee-core.md`, sections 0 to 4)
or extensions (`referee/referee-extensions.md`, the rest). "No error found" is the referees' phrase for a claim
they read in full and failed to break. The tag column is the author's and is not raised by those verdicts.

| Claim | Statement in one line | Tag | Referee |
|---|---|---|---|
| L1 | h(μ) ≤ ln(√(2πe)·C(μ)) for a law on ℝ, C the Gaussian correlation of its quantile function | cited (Bolley–Gentil–Guillin 2018, Theorem 2.1 at n = 1); proof kept | core: no error found; not new |
| L2 | h(ν) ≤ Σᵢ ln(√(2πe)·Cᵢ(ν)) for a law on ℝⁿ under the triangular coupling | proved, unrefereed | core: no error found |
| L3, L4 | the water-filling programme; convexity and bounds for Φ, W, V; the sum bound L4.6 (added) | proved, unrefereed | core: no error found (L4.6 is the referee's) |
| L5 | κ_k ≥ k for every k | proved, unrefereed | core: no error found; second proof |
| A | Conjecture A: E h ≥ Φ(H(c)), and cell by cell E_g h ≥ Φ(ln 1/P_c) | proved, unrefereed | core: no error found; one clause added |
| F1 | E h ≥ W(H(c)) = Σᵢ √min(γᵢ, θ): the full-floor conjecture of section 6 | proved, unrefereed | core: no error found |
| F2, F2a | E h ≥ V_S(H(c)) for every head set S; with the water-filling head, E h ≥ κ_k√θ + Σ_{i>k} √γᵢ | proved, unrefereed | core: no error found, ties included |
| F3 | a residual with negentropy J has the Gaussian floors at rate I(z; c) + J | proved, unrefereed | extensions: no error found; one proof line replaced |
| F4 | isotropic spectrum: E h ≥ √γ·E‖g_n‖·ρ_n(e^(−H)), which is infinite at H = 0 | proved, unrefereed | extensions: no error found |
| F5 | shared information and groupings enter F1 to F3 as they enter Theorem 1 | proved, unrefereed | extensions: correct after a stated repair (made) |
| F6 | offsets that are scale mixtures of Gaussians: the floors times E[t] | proved, unrefereed | extensions: no error found |
| F2-alt | the brief's order (head point first): what it proves and what stops it | sketch: gaps named | not refereed |
| P | what Ingber–Courtade–Weissman and Wu–Wang–Flierl prove | cited, sources read | extensions: readings right; two wordings overstated (corrected) |
| D1 | the norm-only dual-basis certificate against the floor at its own entropy | proved, unrefereed; numbers computed | extensions: no error found; reading reworded |
| E | every floor on the measured spectra at 0 to 96 bytes | computed | not refereed; the falsifier's table agrees |
| B1 | Proposition 3 as Kantorovich duality | cited (Bhatia–Jain–Lim 2019, Theorem 2), with a two-line proof | extensions: attribution right, source read |
| B2a | no norm beats the M-ellipsoid in second moments | proved, unrefereed | extensions: no error found |
| B2b | nor in the mean under independence | cited (the classical M·M* ≥ 1 argument); the change of variables that makes it apply is ours, proved, unrefereed | extensions: no error found |
| B3 | one-scalar certificates φ + φ*; scale mixtures | proved, unrefereed (duality cited) | extensions: no error found; one argument added |
| B4a | several stored scalars: identities, the 2/π, the model of block norms | proved, unrefereed; model against measurement computed | extensions: one sentence wrong (corrected) |
| B4b | one basis for a mixture of query families | proved, unrefereed | extensions: item 3 repaired, item 6 had a gap (closed), closing remark wrong (replaced) |
| B4c, B4d | second moments against the measured effect of the mixed basis; a basis per family | computed against measured; model | extensions: figures match the log; table renamed |
| B5 | n_eff against rule 1's ρ² | proved, unrefereed | extensions: items no error found; closing sentence wrong (replaced) |

---

## 0. Setting and local symbols

The setting is section 0 of `proofcheck/CONSOLIDATED_PROOFS.md`, with these additions. The same setting, in
chapter 0's notation, is N.0 above.

- **Eigen-coordinates.** B = Σ_Q^½ Σ_R Σ_Q^½ = V Γ Vᵀ, Γ = diag(γ₁ ≥ … ≥ γ_m > 0), m the rank. For i ≤ m put
  zᵢ = γᵢ^(−½) vᵢᵀ Σ_Q^½ r, and write g for Vᵀ times the standard Gaussian of Δ = Σ_Q^½ g (again standard Gaussian).
  Then Δ·r = Σ_{i≤m} √γᵢ gᵢ zᵢ almost surely, z is a function of r, and z is independent of g. Under the Gaussian
  model r ~ N(0, Σ_R), z ~ N(0, I_m). Σ_Q and Σ_R need not commute and need not be invertible. From here on n means
  m, and N_n is the standard Gaussian law on ℝⁿ (the text the referees saw wrote γₙ for it, which clashes with the
  eigenvalues).
- **Message.** c takes countably many values. It may be a randomised function of r, provided the pair (r, c) is
  independent of Δ. P_c = Pr[c = c], ν_c is the law of z given c, I(z; c) the mutual information in nats. Always
  I(z; c) ≤ H(c), with equality for a deterministic encoder when Σ_Q^½ r determines the message.
- **Certificate.** h(c, Δ) ≥ 0 is the half-width. It depends on the item only through c and on the query only
  through Δ, and h(c, ·) is measurable in Δ, so that E h exists. A seed shared by the encoder and the certificate
  is not covered by "a randomised function of r"; it is handled by conditioning on the seed (remark after F2).
- **Soundness.** For every (c, Δ) the interval of half-width h(c, Δ) contains Δ·r for ν_c-almost every z. This is
  weaker than soundness on all of K_c, so a floor proved under it holds under the consolidated proofs' definition.
- **Finite information.** The averaging steps of A(c), F1 and F2 assume I(z; c) < ∞. If it is infinite each claim
  reads E h ≥ 0, with Φ, W and V_S extended to H = ∞ by their limit 0.
- **Cell width.** s_c(y) = sup of Σᵢ √γᵢ yᵢ zᵢ over z in the closed support of ν_c. A linear function's essential
  supremum under ν_c is its supremum over the support, so soundness gives 2h(c, Δ) ≥ s_c(g) + s_c(−g), and by the
  symmetry of g

      E_g h(c, ·) ≥ E_g s_c(g).                                                             (0.1)

  This is step 1 of Theorem 1. s_c is a supremum of linear functions, hence Borel, and it dominates one linear
  function, so its mean is well defined in (−∞, ∞].
- **Correlation of a law on ℝ.** For a law μ on ℝ with quantile function Q_μ, T_μ = Q_μ∘Φ is the non-decreasing map
  that sends N(0, 1) to μ, and C(μ) = E[g·T_μ(g)] = ∫₀¹ Q_μ(u) Φ⁻¹(u) du. m₂(μ) is the second moment.
- **Triangular coupling.** For a law ν on ℝⁿ with a density and an order of the coordinates, T^ν is the
  Knothe–Rosenblatt map from N_n to ν: T^νᵢ(g) = T_μ(gᵢ) with μ the conditional law of zᵢ given
  z_<i = T^ν_<i(g_<i). Cᵢ(ν) = E[gᵢ·T^νᵢ(g)].
- **The three functions of a rate H ≥ 0 (nats).** k(s) = ½(s² − 1) − ln s.
  - Φ(H) = min{Σᵢ √γᵢ sᵢ : sᵢ ∈ (0, 1], Σᵢ k(sᵢ) ≤ H}.
  - W(H) = min{Σᵢ √γᵢ sᵢ : sᵢ ∈ (0, 1], Σᵢ ln(1/sᵢ) ≤ H}. For a set T of indices W_T is the same over T only, and
    W_∅ = 0.
  - For a set S of k ≥ 1 indices, V_S(H) = min over 0 ≤ u ≤ H of κ_k·(Π_{i∈S} γᵢ)^(1/2k)·e^(−(H−u)/k) + W_{S^c}(u).
    S^c may be empty, and then V_S(H) = κ_n(Πᵢ γᵢ)^(1/2n)e^(−H/n). V_∅ = W.
- F = Σᵢ √γᵢ, n_eff = F²/Σᵢ γᵢ, as in chapter 0. Entropies h(·) are differential, in nats, with values in [−∞, ∞).

---

## 1. Lemmas

### L1. Entropy against the Gaussian correlation of the quantile function [cited; proof kept]

**Statement.** Let μ be a law on ℝ with a density f and a finite second moment. Then 0 < C(μ) ≤ √Var(μ) and

    h(μ) ≤ ln(√(2πe)·C(μ)),

with equality when μ is Gaussian. Equivalently KL(μ ‖ N(0,1)) ≥ ½ m₂(μ) − ½ − ln C(μ), and, since m₂ ≥ C²,
KL(μ ‖ N(0,1)) ≥ k(C(μ)).

In words: the entropy power e^(2h)/(2πe) is at most C(μ)², which is at most the variance. It sharpens the
maximum-entropy property of the Gaussian.

**Proof.** Write T = T_μ, F_μ for the distribution function, g ~ N(0, 1), φ for the standard normal density.

1. *T(g) has law μ and E T(g)² = m₂.* The quantile transform of a uniform variable has law μ for any μ.
2. *C = ∫ φ dT.* T is non-decreasing; let τ be its Lebesgue–Stieltjes measure. For x > 0,
   T(x) − T(0) = τ([0, x)), and x·φ(x) ≥ 0, so by Tonelli
   E[g (T(g) − T(0)); g > 0] = ∫_{[0,∞)} τ(dy) ∫_{x>y} xφ(x) dx = ∫_{[0,∞)} φ dτ. The half-line g < 0 is the same with
   both factors negative. E[g·T(0)] = 0. So C = ∫ φ dτ. It is finite by Cauchy–Schwarz (C = E[g(T − E T)] ≤ √Var(μ))
   and positive because μ is not a point mass.
3. *C ≥ E T′(g).* By Lebesgue's theorem T is differentiable almost everywhere and τ = T′(x)dx + τ_s with τ_s ≥ 0
   singular. Hence C ≥ ∫ φ T′ dx.
4. *h(μ) = ½ ln(2πe) + E ln T′(g).* F_μ is absolutely continuous with F_μ′ = f off a Lebesgue-null set N₁. Then
   μ(N₁) = 0, so Pr[T(g) ∈ N₁] = 0. T is differentiable off a null set N₂. F_μ is continuous, so
   F_μ(T(x)) = Φ(x) for every x. For x outside N₂ ∪ T⁻¹(N₁) the chain rule applies (T differentiable at x, F_μ
   differentiable at T(x)) and gives f(T(x))·T′(x) = φ(x). So T′(x) > 0, f(T(x)) > 0 and
   ln T′(x) = ln φ(x) − ln f(T(x)) for almost every x. Take expectations: E ln φ(g) = −½ ln(2πe) and
   E[−ln f(T(g))] = −∫ f ln f = h(μ). The integral is well defined in [−∞, ∞): from ln t ≤ t − 1,
   f ln(1/f) ≤ φ − f + f·(½ ln 2π + y²/2), which is integrable.
5. *Jensen.* E ln T′(g) ≤ ln E T′(g) ≤ ln C by step 3. With step 4, h(μ) ≤ ½ ln(2πe) + ln C.

For the second form, KL(μ ‖ N(0,1)) = −h(μ) + ½ ln 2π + ½ m₂. Equality needs T′ constant and τ_s = 0, that is T
affine. ∎

**Checks** (`check_lemma1d.py`, log `check_lemma1d.log`). Piecewise-constant densities, for which h, C and m₂ are
closed forms. 40,003 laws: discretised Gaussians (gap 4·10⁻³, 7·10⁻⁵, 10⁻⁶ on 50, 400, 3,200 cells), the uniform law
(gap ln√(e/2) = 0.1534, as computed by hand), cells of a quantiser, 39,981 random densities with gaps and spikes,
mixtures, Laplace, Student t₃, and an L-BFGS adversary minimising the gap on 8, 20 and 40 cells. Smallest gap
3.8·10⁻⁷, no violation. The adversary's optimum is 1.6·10⁻³ on 40 cells, below the plain discretised Gaussian
(3.7·10⁻³) and above zero. Independently, the falsifier minimised C/s(KL) over seven families at fixed KL
(`../stage0-floor-check/RESULTS.md`, section 1): the closest approach is 1.00000000007, for mixtures that are
almost N(0, s²), and its readings below 1 on a grid were grid error (adaptive quadrature puts them above 1).

**Prior art: L1 is published.** It is the case n = 1 of the dimensional Talagrand inequality for the Gaussian
measure: Bolley, Gentil and Guillin, Ann. Probab. 46 (2018), Theorem 2.1 (arXiv:1507.01086), proved through their
Lemma 2.6 by the same three steps (transport, Jensen, Gaussian integration by parts); first proved by semigroup
methods by Bakry, Bolley and Gentil (2012). The librarian read the LaTeX source
(`../library/floors-novelty.md`, section 1); I did not. I checked the reduction from the statement as quoted there:
with the standard Gaussian their inequality is ½W₂² ≤ ½(m₂ − 1) + 1 − exp(½(m₂ − 1) − KL), and on the line
W₂² = m₂ + 1 − 2C(μ), which gives C ≥ exp(½(m₂ − 1) − KL), L1's second form. The tag is therefore [cited], and the
proof above is kept because the notes give proofs in full. The sentence the referees saw ("the librarian did not
find the inequality") was out of date. Hosking (2007) and the normality-test literature were not opened by anyone
and may hold other forms of it.

### L2. The same in n dimensions [proved, unrefereed]

**Statement.** Let ν be a law on ℝⁿ with a density, finite second moments and h(ν) > −∞. For any order of the
coordinates,

    T^ν(g) has law ν;   Cᵢ(ν) ∈ (0, √(E_ν zᵢ²)];   h(ν) ≤ Σᵢ ln(√(2πe)·Cᵢ(ν)).

**Proof.** *The map.* Regular conditional laws exist, the conditional distribution functions are jointly measurable,
and so are their quantile functions ({Q(u | z_<i) ≤ t} = {u ≤ F(t | z_<i)}). Given g_<i, Φ(gᵢ) is uniform and
independent of g_<i, so T^νᵢ(g) has the conditional law of zᵢ given z_<i = T^ν_<i(g_<i). By induction T^ν(g) has
law ν. No continuity of the conditional distribution functions is used: the map goes from the Gaussian, which has
no atoms, to ν.

*The entropy.* ν has a density, so for ν-almost every z_<i the conditional law μ of zᵢ has a density, and it has a
finite second moment. By the chain rule h(ν) = Σᵢ ∫ h(ν_{i|<i}(· | z_<i)) dν. (To avoid differences of infinities,
read this as the chain rule for KL(ν ‖ N_n), which always holds and has non-negative terms, together with
KL(· ‖ N) = −h + ½ ln 2π per coordinate + half the second moment.) Apply L1 to each conditional law and integrate:

    h(ν) ≤ Σᵢ E[ln(√(2πe)·C(ν_{i|<i}(· | T^ν_<i(g_<i))))] ≤ Σᵢ ln(√(2πe)·E[C(ν_{i|<i})]) = Σᵢ ln(√(2πe)·Cᵢ(ν)).

The second step is Jensen for the logarithm, and the last is E[gᵢ T^νᵢ(g) | g_<i] = C(ν_{i|<i}). Cᵢ ≤ √(E zᵢ²) is
Cauchy–Schwarz. ∎

This is where the brief's worry about step 4 goes: the conditional entropies are random, and only their mean is
controlled, but the logarithm's concavity is all that is used. The convexity of s(·) is not needed.

**What L2 adds to the published result.** Bolley, Gentil and Guillin's theorem in n dimensions uses the Brenier map
and gives h(ν) ≤ n·ln(√(2πe)·C*/n), with C* the largest E⟨g, x⟩ over couplings of N_n and ν. L2 implies it: by the
inequality of the means Σᵢ ln Cᵢ ≤ n·ln(Σᵢ Cᵢ/n), and Σᵢ Cᵢ ≤ C* because the triangular coupling is one coupling. L2
keeps one correlation per coordinate, which the water-filling of F1 needs and the published form does not give.
The librarian calls it a known consequence (the n = 1 case, the chain rule and Jensen, which is the proof above)
and did not find it stated; the tag stays [proved, unrefereed].

**Check by the core referee** (`referee/core_l2_2d.py`): L2 in the plane, exactly, for densities constant on a
product grid, both coordinate orders. 20,000 random densities, smallest gap 0.218 nats; the Gaussian restricted to
ten shapes that are not products, 0.1533; an L-BFGS adversary on grids up to 6×3, 0.091, falling as it approaches a
Gaussian. No violation. I had tested L1 in one dimension only.

### L3. The water-filling programme [proved, unrefereed]

**Statement.** Let aᵢ > 0 and H ≥ 0, and let λ ∈ (0, max aᵢ] solve Σᵢ ln⁺(aᵢ/λ) = H. Then
min{Σ aᵢ sᵢ : sᵢ ∈ (0, 1], Σ ln(1/sᵢ) ≤ H} = Σᵢ min(aᵢ, λ), attained at sᵢ* = min(1, λ/aᵢ).
With aᵢ = √γᵢ and θ = λ²: W(H) = Σᵢ √min(γᵢ, θ), where Σᵢ ½ ln⁺(γᵢ/θ) = H.

**Proof.** λ exists and is unique for H > 0 because λ ↦ Σ ln⁺(aᵢ/λ) is continuous and strictly decreasing on
(0, max aᵢ] from ∞ to 0; for H = 0 take λ = max aᵢ. Let L(s) = Σ aᵢ sᵢ − λ Σ ln sᵢ. Coordinate by coordinate L is
convex on (0, 1] with derivative aᵢ − λ/sᵢ, so it is minimised at sᵢ*. For feasible s, Σ ln sᵢ + H ≥ 0, so
Σ aᵢ sᵢ ≥ L(s) − λH ≥ L(s*) − λH = Σ aᵢ sᵢ* + λ(Σ ln(1/sᵢ*) − H) = Σ aᵢ sᵢ*. ∎

### L4. Properties of Φ, W and V_S [proved, unrefereed]

1. Φ, W and V_S are convex and non-increasing in H, the minima are attained, and Φ(0) = W(0) = F.
2. W(H) ≥ Φ(H) for every H.
3. Φ(H) ≥ F − √(H·Σᵢ γᵢ) = F·(1 − √(H/n_eff)).
4. W(H) ≥ F − min(√γ₁·H, √(H·Σᵢ γᵢ)). If γ₁ > γ₂ and H ≤ ½ ln(γ₁/γ₂), W(H) = F − √γ₁(1 − e^(−H)) exactly.
5. V_S(H) ≥ max(W_{S^c}(H), κ_k (Π_S γᵢ)^(1/2k) e^(−H/k)). In particular sup_S V_S ≥ max(W, Theorem 1's bound).
6. (Added after the core referee's report.) V_S(H) ≥ κ_k (Π_S γᵢ)^(1/2k) e^(−H/k) + W_T(H) for T = S^c and for every
   subset T of S^c. This is the sum of the two terms where item 5 has their maximum, and it is what makes
   Theorem 3 a consequence of F2 (section 4).

**Proof.** (1) Φ and W are value functions of programmes with a linear objective and a convex constraint (k and
−ln are convex), so a convex combination of minimisers at H₁ and H₂ is feasible at the combination of the rates.
V_S is the infimal convolution of two convex non-increasing functions. Minima are attained: the feasible sets are
compact because k(s) and ln(1/s) tend to infinity as s → 0. (2) k(s) ≤ ln(1/s) on (0, 1], so W minimises over a
smaller set. (3) k(1) = k′(1) = 0 and k″ = 1 + s⁻² ≥ 2, so k(s) ≥ (1 − s)². A feasible s has Σ(1 − sᵢ)² ≤ H, and
Σ √γᵢ(1 − sᵢ) ≤ √(Σγᵢ)·√H by Cauchy–Schwarz. (4) With a = √γᵢ > λ: a − λ ≤ a ln(a/λ), which sums to √γ₁·H; and
((a − λ)/a)² ≤ 1 − λ/a ≤ ln(a/λ), so Σ(a − λ)⁺ ≤ √(Σγᵢ)·√H by Cauchy–Schwarz. The exact form is L3 with one coded
coordinate. (5) and (6) In the minimand the head term increases with u, so it is at least its value at u = 0, and
W_{S^c}(u) ≥ W_{S^c}(H) for u ≤ H because W_{S^c} is non-increasing. That is (6) for T = S^c, and (5) follows because
both terms are non-negative. For a subset T of S^c, W_{S^c} ≥ W_T: restricting a feasible point of the larger
programme to T leaves it feasible and drops non-negative terms. ∎

**Checks.** The core referee (`referee/core_f2_misc.py`): every inequality of items 1 to 5 on 400 spectra, ties
included, at 13 rates, with equality only at H = 0; L3's closed form against a constrained minimiser on 300
programmes (agreement to 2·10⁻⁴, the minimiser never below). Item 6 (`check_after_referees.py`, line 1): on 400
spectra and every top-k head, V_S(H) − (head + W_T(H)) ≥ 0, with equality only when the head or the tail is empty.

### L5. κ_k ≥ k for every k [proved, unrefereed]

The consolidated proofs have this for k ≤ 200,000 by computation.

**Proof.** Let ν be uniform on the ball of radius ρ in ℝᵏ. T^ν(g) lies in the ball, so ρ‖g‖ ≥ g·T^ν(g) and
ρ·E‖g_k‖ ≥ Σᵢ Cᵢ(ν). By L2, Σᵢ ln(√(2πe)Cᵢ) ≥ h(ν) = ln(ω_k ρᵏ), and by the inequality of the means
Σ Cᵢ ≥ k(Π Cᵢ)^(1/k) ≥ k·ω_k^(1/k)·ρ/√(2πe). So E‖g_k‖·√(2πe) ≥ k·ω_k^(1/k), which is κ_k ≥ k. ∎

The core referee confirms it by a second proof that does not use L2: Wendel's inequality E‖g_k‖ ≥ k/√(k+1) and
Stirling's lower bound for ω_k give κ_k ≥ k·√(k/(k+1))·(πk)^(1/2k) ≥ k. Numerically κ_k − k is least at k = 1, where
it is √e − 1 = 0.649 (`referee/core_f2_misc.py`).

---

## 2. Conjecture A [proved, unrefereed]

**Proposition A.** Under the Gaussian model, for any encoder and any sound certificate (hypotheses of section 0):

(a) for every law ν on ℝⁿ with KL(ν ‖ N_n) < ∞, Σᵢ √γᵢ Cᵢ(ν) ≥ Φ(KL(ν ‖ N_n));
(b) for every message value with P_c > 0, E_g h(c, ·) ≥ Φ(KL(ν_c ‖ N_n)) ≥ Φ(ln(1/P_c));
(c) E h ≥ Φ(I(z; c)) ≥ Φ(H(c)).

**Proof.** (a) KL(ν ‖ N_n) < ∞ gives ν a density, finite second moments (Donsker–Varadhan with λ‖z‖², λ < ½) and
h(ν) > −∞, which are L2's hypotheses. Apply the second form of L1 to each conditional law, integrate and use
Jensen as in L2: KL(ν ‖ N_n) ≥ Σᵢ [½ E_ν zᵢ² − ½ − ln Cᵢ(ν)] ≥ Σᵢ k(Cᵢ(ν)), using E_ν zᵢ² ≥ Cᵢ². Put
sᵢ = min(Cᵢ, 1). **k ≥ 0 on (0, ∞), with its minimum 0 at s = 1**, so k(sᵢ) ≤ k(Cᵢ) in both cases: if Cᵢ ≤ 1 they are
equal, and if Cᵢ > 1 then k(sᵢ) = 0. Hence Σᵢ k(sᵢ) ≤ KL and s is feasible for Φ(KL), and
Σᵢ √γᵢ Cᵢ ≥ Σᵢ √γᵢ sᵢ ≥ Φ(KL). (b) T^{ν_c}(g) lies in the support of ν_c almost surely, so
s_c(g) ≥ Σᵢ √γᵢ gᵢ T^{ν_c}ᵢ(g); take expectations and use (0.1) and (a). P_c ν_c ≤ N_n gives
KL(ν_c ‖ N_n) ≤ ln(1/P_c), and Φ is non-increasing. (c) Let I(z; c) < ∞. Average (b) over c; Φ is convex and
continuous at 0 (L4), so Jensen holds for countably many cells; Σ_c P_c KL(ν_c ‖ N_n) = I(z; c) because the cell
laws mix to N_n; and Φ is non-increasing. ∎

The clause in bold was missing from the text the referees saw, which said only "k decreases on (0, 1] and
k(1) = 0". That does not cover a correlation above 1, which a cell far from the origin has: the cell
{|z| ≥ 2} in one dimension has C = 2.096 (`check_after_referees.py`, line 3). The statement is unchanged.

**What the brief expected, confirmed or corrected.**
- Φ(0) = F. Confirmed.
- Small H: Φ(H) ≥ F − √(H Σγᵢ), with equality to first order. Confirmed, and better than Theorem 2's
  (√(2H) + 1/√2)·√(Σγᵢ).
- High rate: the multiplier λ of the programme gives sᵢ = (√(aᵢ² + 4) − aᵢ)/2 with aᵢ = √γᵢ/λ. A strongly coded
  coordinate contributes √γᵢ sᵢ → λ and uses ½ ln(γᵢ/(eλ²)) nats, so in water-filling terms each contributes
  √θ/√e. Confirmed. The tail is not quite "in full": an uncoded coordinate contributes √γᵢ − γᵢ/(2λ) to first
  order.
- Step 3's lemma: the Gaussian is extremal, by L1 and m₂ ≥ C². Step 2: atoms are not an issue. Step 4: valid, and
  simpler than proposed. The coordinate order is free.

**What is lost.** The step E_ν zᵢ² ≥ Cᵢ² discards the second moments of the cells. They are not free: the cell laws
average to N(0, I), so Σ_c P_c E[zᵢ² | c] = 1. The librarian reached the same diagnosis independently
(`library/theory.md`, section 1.1). Keeping the second moments gives F1. Proposition A survives as the only bound
here that holds cell by cell with the tail in full: (b) has no analogue for W, whose proof needs the average over
cells. The core referee confirms that by example: for centred balls E_g h / W(ln 1/P_c) falls to 0.62 at n = 100,
and a small disc at the origin in the plane has the limit κ₂/(2√e) = 0.886. Neither is an error in F1, which is
claimed on average only.

**Checks.**
- `floors_eval.py` evaluates Φ beside W (section 7). Every code tested against W below also passes Φ ≤ W.
- The core referee, cell by cell (`referee/core_propA.py`): centred balls, n from 1 to 10⁴, smallest
  E_g h / Φ(ln 1/P) = 1.0011 (the small-radius limit is κ_n/n, so (b) is nearly tight in high dimension); 4,000
  centred boxes, 1.6487 = √e; 75 centred ellipses, 1.461 = κ₂/2.
- The falsifier (`../stage0-floor-check/RESULTS.md`, sections 1 to 3): not refuted. Per cell, boxes reach √e and
  balls κ_n/n (1.0266 at n = 100); averaged over a code the closest is 1.168 (a norm-only code at n = 100). Its
  runs on ellipsoids, shells, cut balls and polygons did not finish.

---

## 3. The water-filling floor [proved, unrefereed]

**Theorem F1.** Under the Gaussian model r ~ N(0, Σ_R), Δ = Σ_Q^½ g independent of r, for any encoder and any sound
certificate (hypotheses of section 0),

    E h ≥ W(I(z; c)) ≥ W(H(c)) = Σᵢ √min(γᵢ, θ),      Σᵢ ½ ln⁺(γᵢ/θ) = H(c).

More precisely, when I(z; c) < ∞ there are numbers C̄ᵢ ∈ (0, 1] with E h ≥ Σᵢ √γᵢ C̄ᵢ and Σᵢ ln(1/C̄ᵢ) ≤ I(z; c). When
I(z; c) = ∞ the statement is E h ≥ 0.

With H(c) ≤ B ln 2 this is the "full floor" conjecture of section 6 of the consolidated proofs, for every n and for
Σ_Q and Σ_R that need not commute. The core referee's reading: it is that conjecture exactly, and a little more
(I(z; c) in place of H(c), randomised encoders, soundness almost surely on the cell, singular Σ_Q and Σ_R).

**What is claimed as new, and what is not.** The formula is not new: it was published as an asymptotic
identification rate (section 6). F1 is, as far as the librarian's search goes
(`../library/floors-novelty.md`, section 3), the first proof of the converse at finite n, for covariances that need
not commute, in the mean-width form.

**Proof.** Let I(z; c) < ∞.
1. *Width.* (0.1): E_g h(c, ·) ≥ E_g s_c(g), for every c with P_c > 0.
2. *Coupling.* P_c ν_c ≤ N_n, so ν_c has a density, finite second moments and h(ν_c) ≥ ln(P_c(2π)^(n/2)) > −∞. Let
   T^c = T^{ν_c} (L2). T^c(g) has law ν_c, so it lies in the support of ν_c almost surely. It is a function of the
   same g at which s_c is taken, so pointwise s_c(g) ≥ Σᵢ √γᵢ gᵢ T^cᵢ(g). Hence E_g s_c(g) ≥ Σᵢ √γᵢ Cᵢ(ν_c). The right
   side is integrable: E|gᵢ T^cᵢ| ≤ √(E_{ν_c} zᵢ²) ≤ P_c^(−½).
3. *Entropy.* L2: h(ν_c) ≤ Σᵢ ln(√(2πe)·Cᵢ(ν_c)).
4. *Averaging.* Σ_c P_c h(ν_c) = h(z | c) = h(z) − I(z; c) = (n/2) ln(2πe) − I(z; c). So
   Σᵢ Σ_c P_c ln Cᵢ(ν_c) ≥ −I(z; c). Put C̄ᵢ = Σ_c P_c Cᵢ(ν_c). By Jensen Σᵢ ln C̄ᵢ ≥ −I(z; c).
5. *C̄ᵢ ≤ 1.* C̄ᵢ ≤ Σ_c P_c √(E_{ν_c} zᵢ²) ≤ √(Σ_c P_c E_{ν_c} zᵢ²) = √(E zᵢ²) = 1.
6. E h = Σ_c P_c E_g h(c, ·) ≥ Σᵢ √γᵢ C̄ᵢ ≥ W(I(z; c)) by the definition of W; W is non-increasing and
   I(z; c) ≤ H(c). The closed form is L3. ∎

Where the cells' second moments enter: step 4 uses h(z | c) = h(z) − I, which is the statement that the cell laws
mix back to N(0, I), and step 5 uses it again. Per cell the same argument gives
Σᵢ ln(1/Cᵢ(ν_c)) ≤ KL(ν_c ‖ N_n) + ½ Σᵢ (1 − E_{ν_c} zᵢ²), in which the last term has either sign.

**Remarks.**
- *Against Theorem 1.* The head constant is 1 where Theorem 1 has κ_k/k. F2 restores κ_k.
- *Theorem 2 follows from F1.* W ≥ W_T for every index set T, and L4.4 applied to W_T gives
  W_T(H) ≥ Σ_T √γᵢ − √(H·Σ_T γᵢ), which is above Theorem 2's bound: no √2, no additive constant, and a loss linear in
  H at small rate (L4.4) where Theorem 2 and Proposition A lose √H. (Checked on 400 spectra: W − Theorem 2 ≥ 0.22
  in the units of each spectrum, `check_after_referees.py`, line 1.)
- *Where the slack is* (reworded after the core referee's point 2). The inequalities of steps 3 to 6 are
  equalities when every ν_c is N(m_c, diag(sᵢ²)) with the same sᵢ, the backward test channel of rate–distortion
  theory. No cell has that law. For a real code the slack between E h and W is the product of two losses: the
  coupling step (the supremum over a cell against the correlation with the cell's own law) and L2's gap for the
  cell law. In one dimension, for an interval cell at high rate, the whole slack is √e, and it is √2 from the
  coupling (`check_chain1d.log`: 1.414) times √(e/2) from L1 for a uniform law; the text the referees saw read as
  if the coupling alone cost √e. For ball-shaped cells in k dimensions the whole slack is κ_k/k.
- *Tightness* (reworded after the extensions referee's finding). In probability: Ingber–Courtade–Weissman's
  scheme certifies n(√θ + o(1)) with probability tending to 1, for an isotropic spectrum. That is not tightness of
  F1, which is about the mean: their scheme answers "maybe" for every item outside the typical shell, so as a
  certificate its mean width is infinite at every n. In the mean, tightness is shown here only by computed codes
  [computed, by others]: shells of the norm alone reach E h / W = 1.032, 1.0043, 1.00055 at n = 100, 1,000, 10,000
  (isotropic; core referee, `referee/core_f2_misc.log`), and a grid on one head coordinate with a grid on the
  tail's norm reaches 1.0026 at 2,001 eigenvalues (the falsifier, `../stage0-floor-check/RESULTS.md`, section 3).
  A proof that some code attains W(H)(1 + o(1)) in the mean at every rate is not given.
- *What is not claimed.* Nothing for real data: the residuals are not Gaussian (F3 charges that as rate, and J is
  not known), the offsets are not Gaussian or independent of the residual, and shared bits must be counted (F5).

**Checks.** The exact range of a code is its smallest sound certificate, so a code whose exact mean width fell
below W would refute F1.

By the author (`check_schemes.py`, logs `check_schemes_part1.log`, `check_schemes_part2.log`,
`check_schemes_part3.log`; `check_chain1d.py`):
- Part 1: 2,500 product codes of interval partitions (uniform grids, random breakpoints, one wide cell with
  geometric cells outside), n ≤ 5, random spectra. Smallest E h / W = 1.649 (it is √e: one coordinate at high rate).
- Part 2: 2,800 planar partitions that are not products (Voronoi cells in random metrics, merged cells, sheared
  lattices, strips, rings of the dual norm), γ = (1, ε²) for ε = 1, 0.5, 0.2, 0.05; mean widths exact through the
  perimeter of each cell's hull, entropies exact. Smallest E h / W = 1.658.
- Part 3: 300 codes with grids on 0 to 3 head coordinates and shells of the dual norm on the tail, n = 8 to 48
  (shell masses from 400,000 samples). Smallest E h / W = 1.080.
- `agent1/theory.log`: the escape code measured there on Gaussian data sits 1.28 to 1.63 above W (section 7).
- The inner inequality itself, in one dimension (`check_chain1d.py`, log `check_chain1d.log`): for 160 interval
  partitions, with each cell's law discretised on 600 sub-cells, C̄·e^(H) ≥ 1.0001 (step 4 requires ≥ 1; the
  near-equality is a partition with one cell of almost all the mass) and E h / C̄ ≥ 1.414 (the coupling step).

By the core referee (`referee/core_opt2d.py`, `core_chain2d.py`, `core_f2_3d.py`, `core_f2_misc.py`):
- a greedy search over planar cell shapes, 17 runs of 150,000 moves: smallest E h / W = 1.653;
- the proof's inner inequalities on 21 planar partitions that are not products, both coordinate orders: coupling
  step ≥ 1.383, L2 per cell gap ≥ 0.172, steps 3 and 4 together ≥ 0.208 nats, step 5 slack ≥ 0.150;
- whole codes in three dimensions: smallest E h / W = 1.687;
- shells in high dimension: 1.00055 at n = 10⁴ (above).

By the falsifier (`../stage0-floor-check/RESULTS.md`): six families of whole codes, nothing below W; closest
1.0026. Three rows of its logs below 1 were a bug in its own script (a tail grid with no cells), excluded there.

The tests differ in how hard they press. In one, two and three dimensions a cell cannot bound an uncoded
coordinate cheaply, so the ratios stay at 1.6 and above. The tail in full is tested where the tail has many
dimensions: my part 3 (within 8%), the referee's shells and the falsifier's targeted family (within 0.3%).

---

## 4. The combined floor: Theorem 1's constant on the head, the tail in full [proved, unrefereed]

**Theorem F2.** Under the Gaussian model, for any encoder and any sound certificate (hypotheses of section 0), and
any set S of k ≥ 1 eigen-indices,

    E h ≥ V_S(I(z; c)) ≥ V_S(H(c)) = min over 0 ≤ u ≤ H of { κ_k·(Π_{i∈S} γᵢ)^(1/2k)·e^(−(H−u)/k) + W_{S^c}(u) }.

The complement of S may be empty; then W over no indices is 0 and V_S(H) = κ_n(Πᵢ γᵢ)^(1/2n)e^(−H/n), which is
Corollary 1 of Theorem 1 at k = n. With the convention V_∅ = W the case k = 0 is F1. V_S always dominates Corollary
1's term for the set S (L4.5). When I(z; c) = ∞ the statement is E h ≥ 0.

**Corollary F2a.** Let H > 0, let θ solve Σᵢ ½ ln⁺(γᵢ/θ) = H and k = #{i : γᵢ > θ}, so that eigenvalues equal to θ
belong to the tail. Then k ≥ 1 and

    E h ≥ κ_k·√θ + Σ_{i>k} √γᵢ.

This is the section 6 conjecture with κ_k in place of k on the head: larger by (κ_k − k)√θ.

**Proof of F2.** Let I(z; c) < ∞. Write T = S^c. Order the coordinates with T first. The proof uses one step of
Theorem 1, its step 3 (Urysohn's inequality for a bounded set), and is otherwise independent of it.
1. *Tail first.* Let ν_{c,T} be the T-marginal of ν_c and T^c its triangular map (L2, in ℝ^T). Put z_T = T^c(g_T).
   It has law ν_{c,T} exactly, and it is a function of g_T, so it is independent of g_S.
2. *Head on the fibre.* Let K be the closed support of ν_c and K^{z_T} = {s : (s, z_T) ∈ K}. For ν_{c,T}-almost
   every z_T the conditional law ν_{c,S|z_T} has a density and is carried by K^{z_T}. For every s in the fibre
   (s, z_T) ∈ K, so

       s_c(g) ≥ Σ_{i∈T} √γᵢ gᵢ (z_T)ᵢ + sup over s ∈ K^{z_T} of Σ_{i∈S} √γᵢ gᵢ sᵢ.

   The supremum is a Borel function of (z_T, g_S) (K is a countable union of compact sets, and the supremum over
   the fibre of a compact set is upper semicontinuous).
3. *Urysohn.* Given g_T, g_S is still standard Gaussian. By step 3 of Theorem 1 [proved, refereed] applied to the
   set Γ_S^½ K^{z_T}, whose volume is (Π_S γᵢ)^½ vol(K^{z_T}),

       E[sup over the fibre | g_T] ≥ E‖g_k‖·ω_k^(−1/k)·(Π_S γᵢ)^(1/2k)·vol(K^{z_T})^(1/k).

   An unbounded fibre makes the left side infinite (a support function that is finite in every direction belongs
   to a bounded set; otherwise the directions where it is finite form a convex cone that is not the whole space,
   so it is +∞ on a set of Gaussian measure at least ½). The uniform law maximises entropy on a set, so
   ln vol(K^{z_T}) ≥ h(ν_{c,S|z_T}).
4. *Jensen in the cell.* z_T has law ν_{c,T}, so
   E exp(h(ν_{c,S|z_T})/k) ≥ exp(a_c/k) with a_c = ∫ h(ν_{c,S|z_T}) dν_{c,T}, the conditional entropy of z_S given
   z_T in the cell. With (0.1) and step 2 of F1 on the tail,

       E_g h(c, ·) ≥ Σ_{i∈T} √γᵢ Cᵢ(ν_{c,T}) + E‖g_k‖·ω_k^(−1/k)·(Π_S γᵢ)^(1/2k)·e^(a_c/k).

5. *Entropies.* b_c := h(ν_{c,T}) ≤ Σ_{i∈T} ln(√(2πe)·Cᵢ(ν_{c,T})) by L2, and a_c + b_c = h(ν_c) by the chain rule
   (all three are finite: the densities are bounded by (2π)^(−n/2)/P_c and the second moments are finite).
6. *Averaging over cells.* Let ā, b̄ be the means of a_c, b_c and C̄ᵢ the means of Cᵢ(ν_{c,T}). By Jensen
   Σ_c P_c e^(a_c/k) ≥ e^(ā/k) and b̄ ≤ Σ_T ln(√(2πe)·C̄ᵢ). Also ā + b̄ = h(z | c) = (n/2) ln(2πe) − I(z; c), and
   C̄ᵢ ≤ 1 as in F1. Put u = Σ_T ln(1/C̄ᵢ) ≥ 0. Then b̄ ≤ (|T|/2) ln(2πe) − u, so ā ≥ (k/2) ln(2πe) − (I − u), and

       E h ≥ κ_k·(Π_S γᵢ)^(1/2k)·e^(−(I−u)/k) + Σ_T √γᵢ C̄ᵢ ≥ κ_k·(Π_S γᵢ)^(1/2k)·e^(−(I−u)/k) + W_T(u),

   using E‖g_k‖ ω_k^(−1/k) √(2πe) = κ_k and the definition of W_T. Finally u ≤ I, because
   ā = h(z_S | z_T, c) ≤ h(z_S) = (k/2) ln(2πe). So E h ≥ V_S(I(z; c)), and V_S is non-increasing. ∎

The rate splits exactly, I(z; c) = I(z_T; c) + I(z_S; c | z_T), and the adversary of the bound chooses the split.
In step 2 the half-width is compared with the support function at the same g, as soundness requires, and no point
of the fibre is selected, so no measurable selection is needed (the core referee's reading of the step).

**Proof of F2a.** H > 0 gives θ < γ₁, so k ≥ 1. Take S = {1, …, k}. Then (Π_S γᵢ)^(1/2k) e^(−H/k) = √θ. If k = n the
tail is empty and V_S(H) = κ_n√θ. Otherwise the function f(u) = κ_k (Π_S γᵢ)^(1/2k) e^(−(H−u)/k) + W_T(u) is convex
on [0, H]. If the largest eigenvalue of the tail is repeated j times, L3 gives
W_T(u) = j·√γ_{k+1}·e^(−u/j) + Σ_{i>k+j} √γᵢ for small u, so W_T′(0⁺) = −√γ_{k+1} with or without ties, and
f′(0⁺) = (κ_k/k)√θ − √γ_{k+1}. Now γ_{k+1} ≤ θ by the definition of k, and κ_k ≥ k (L5), so the derivative is
non-negative and the minimum is at u = 0: V_S(H) = κ_k√θ + Σ_{i>k} √γᵢ. ∎

**A shared seed.** If the encoder and the certificate both read a seed ω independent of (r, Δ), the hypotheses of
section 0 hold conditionally on ω. W, V_S and Φ are convex (L4), so averaging over ω gives
E h ≥ V_S(I(z; c | ω)) for every S, and I(z; c | ω) ≤ H(c | ω) ≤ H(c). This covers sound certificates only; a
certificate that may fail with probability δ over the seed is outside every claim here.

**Theorems 1 to 3 against F1 and F2** (after the core referee's summary).
- Corollary 1 of Theorem 1 (Gaussian residuals), for every k: it is L4.5 with S = {1, …, k}. For a subspace not
  spanned by eigenvectors the Gaussian case of Theorem 1 follows as well, because the determinant of a
  k-dimensional compression of the matrix is at most γ₁⋯γ_k.
- Theorem 1 itself does not follow. It covers any residual with a density on a subspace, and its Corollary 2 covers
  shared information; F2 is proved for Gaussian residuals only (F3 and F5 extend it). And F2 is not independent of
  Theorem 1: its step 3 is Theorem 1's step 3 applied to a fibre.
- Theorem 2 follows from F1 (section 3).
- Theorem 3 follows from F2 by L4.6: V_S(H) ≥ κ_k(Π_S γᵢ)^(1/2k)e^(−H/k) + W_T(H) for disjoint S and T, and
  W_T(H) ≥ Σ_T √γᵢ − √(H·Σ_T γᵢ) by L4.4. That is above Theorem 3's bound, with no added bit and no 1/√e. L4.5 alone,
  which the referees saw, gives only the larger of the two terms and would not have been enough.
  (Checked on 400 spectra: the best sum bound exceeds Theorem 3 by at least 0.19 in the units of each spectrum,
  `check_after_referees.py`, line 1.)

**Why this order.** The brief proposed the head first; see F2-alt below for what that proves. Taking the tail first
works because the tail point then has exactly the cell's tail law (so entropies and second moments average), and
the head supremum is over a whole fibre at a point that is independent of the head Gaussian (so no point has to be
selected). Theorem 3 lost its constant through the mass of the fibre over a selected head point; here no fibre mass
appears.

**The best head set.** Any S is allowed, and the water-filling head of F2a is valid but need not be the best. A
tie is the clean case (the core referee's example): for γ = (4, 1, 1) and H = ln 2, θ = 1, the water-filling head
is {1} and F2a gives κ₁ + 2 = 3.649, while S = {1, 2} gives 3.922 and S = {1, 2, 3} gives κ₃ = 4.091
(`check_after_referees.py`, line 2). On the measured spectra the best top-k set is a little larger than the
water-filling head (49 against 44 coordinates at 48 bits) and the gain over W is 1 to 2% (section 7). The gain is
about (κ_k − k)√θ, and κ_k − k ≈ ½ ln(πk): a few coordinates' worth, whatever the dimension. A head made of a band
of nearly equal eigenvalues is also allowed; I did not search over bands, and nobody tested head sets that are not
top-k sets in more than five dimensions.

**Checks.** `check_schemes.py`, the same codes as for F1, against max over S of V_S.
- Part 1, every subset S of n ≤ 5 coordinates: smallest ratio 1.000040 (one coordinate, step 0.05σ: Theorem 1's
  known tight case); smallest with n ≥ 2 is 1.130.
- Part 2, the four subsets of two coordinates: smallest ratio 1.135 (a sheared lattice, S = both). With ε = 0.05
  the binding set is S = {1} and the smallest ratio is 1.223 (strips), where Theorem 1 alone gives 1.346: the new
  floor is closer to the code by a tenth.
- Part 3, S = top-k for every k: smallest ratio 1.030 (n = 48, no head grid, 1.76 nats in the shells; best k = 21).
- No code fell below. `floors_eval.py` asserts the same for the two codes of `agent1/theory.log`.

By the core referee (`referee/core_f2chain.py`, `core_f2_3d.py`, `core_f2_misc.py`):
- The inequality in which the head and the tail meet, steps 1 to 4 for one law ν carried by a set K,
  E_g h_{Γ^½K}(g) ≥ Σ_T √γᵢ Cᵢ(ν_T) + E‖g_k‖ω_k^(−1/k)(Π_S γᵢ)^(1/2k) exp((h(ν) − h(ν_T))/k). In the plane, exactly: ten
  shapes, smallest ratio 1.026; an L-BFGS adversary over densities on 300 supports, 1.00062, at a case the referee
  computed by hand to be an equality (a rectangle with the tail mass at its two ends and the head uniform). In
  three dimensions, with exact mean widths, eight shapes and every assignment of the axes: 1.060.
- Whole codes in three dimensions against all eight head sets: smallest ratio 1.237; with a head that leaves a
  tail, 1.546.
- F2a with ties: for (4,1,1), (4,4,1,1,¼), (9,4,4,4,1), (1,1,1,1), (16,4,1,1,1,0.01) and 60 random spectra, at
  rates that put θ on a repeated eigenvalue, the minimum over 4,001 values of u equals the formula to 5·10⁻¹⁶.
- Product codes in high dimension (one head coordinate on a grid, shells on an isotropic tail): E h / max_S V_S =
  1.043, 1.0045, 1.00045 at 30, 300, 3,000 tail coordinates. A product code cannot fall below F2 if Theorem 1
  holds for each factor, so this measures tightness and refutes nothing.

By the falsifier (`../stage0-floor-check/RESULTS.md`, section 3): nothing below. The closest code is within 1.0005
of max_S V_S, and there the maximising S is all indices, which is Theorem 1. Against S = the head, which is F2's
own content, the closest is 1.0023 (one head coordinate, 2,000 tail eigenvalues), falling roughly as 1/N.

What nobody tested: a high-dimensional code whose cells couple the head to the tail. Coupled cells were tested in
two and three dimensions only, where the floors are loose (1.13 and above); in high dimension only product codes
were computed.

---

## 5. Extensions

### F3. Residuals that are not Gaussian [proved, unrefereed]

**Statement.** Drop r ~ N(0, Σ_R). Let Σ_R be the second-moment matrix of r (so that E zzᵀ = I holds by the
construction of z), and assume z has a density and finite entropy. Let
J = (n/2) ln(2πe) − h(z) = KL(law of z ‖ N(0, I)) ≥ 0, the negentropy in nats. Keep Δ Gaussian and independent of
(r, c), and the certificate as in section 0. Then

    E h ≥ V_S(I(z; c) + J) ≥ V_S(H(c) + J) for every S,     in particular E h ≥ W(I(z; c) + J).

The text the referees saw stated only the weaker form with H(c); its proof gave the form with I(z; c), as the
extensions referee notes.

**Proof.** Let H(c) < ∞; otherwise there is nothing to prove. The proofs of F1 and F2 use the Gaussian law of z in
three places (the text the referees saw named two; the third is the referee's).
1. h(z) = (n/2) ln(2πe), in step 4 of F1 and step 6 of F2. It becomes (n/2) ln(2πe) − J.
2. E zᵢ² = 1, in step 5 of F1. It is kept.
3. ν_c ≤ N_n/P_c, in step 2 of F1 (density, second moments, h(ν_c) > −∞) and in step 5 of F2 (a_c and b_c finite).
   This is false for a z that is not Gaussian. It is replaced as follows. With μ the law of z, P_c ν_c ≤ μ, so ν_c
   has a density and E_{ν_c}‖z‖² ≤ n/P_c. Each h(ν_c) is bounded above through its second moment, and
   Σ_c P_c h(ν_c)⁺ < ∞ by Jensen. Also Σ_c P_c h(ν_c) = h(z) − I(z; c) > −∞, so every h(ν_c) is finite. In F2, a_c and
   b_c are each bounded above through second moments and sum to h(ν_c), so both are finite.
With these, step 6 of F2 reads ā ≥ (k/2) ln(2πe) − (I + J − u), and u ≤ I + J because
h(z_S | z_T, c) ≤ h(z_S) ≤ (k/2) ln(2πe), the maximum entropy for a given second-moment matrix. ∎

So the negentropy of the residual is charged as rate, as in the remark after Theorem 1, where the factor is
exp(−J/k). This is a lower bound only: it does not say the true optimum is smaller for a non-Gaussian residual
(the section 6 conjecture "Gaussian residuals are the worst case" stays open).

Against the literature (reworded after the extensions referee's finding). Ingber–Courtade–Weissman's Theorem 4
says that a non-Gaussian pair needs no more rate than the Gaussian pair of the same variances; it has no
negentropy term, so it agrees with F3 in direction only. The form "rate plus relative entropy" is that of
Ordentlich–Polyanskiy's Γ(R + D(P‖N(0,1))) ≤ D^IP(R, P), which nobody here has checked [cited, unchecked: from the
librarian's transcription].

For real data J is not known. `entropy/REPORT.md` gives lower bounds on J (23 to 80 bits per item on BioCLIP-2).
They bound from above what F3 can prove, because W(H + J_true) ≤ W(H + J_lower). They do not bound the least
possible width for the real data, which may lie above or below W(H + J_lower). (The text the referees saw said
they "bound the true floor from above", which was wrong.) Section 7 shows what those values would do.

**Checks.**
- By hand, and `check_after_referees.py`, line 4: one coordinate, a uniform law of unit variance (J = 0.1765 nats)
  on a uniform grid of m cells has E h = E|g|·√3/m = 1.381977/m, and κ₁e^(−(ln m + J)) is the same number. So F3 with
  S = {1} is attained, and the Gaussian floor without J (1.648721/m) is not a floor for this law.
- The extensions referee (`referee/ref_f3_f4.py`): z uniform on a rotated square (J = 0.353 nats), γ = (1, ε²),
  4,000 codes with rectangular cells, exact widths and entropies, against the best of the four head sets at rate
  H + J: smallest ratio 1.128; the same codes against the Gaussian floor without J fall to 0.854, so the
  correction is needed. A coded uniform coordinate times an uncoded two-piece coordinate, J up to 5 nats: 1.159.

### F4. An isotropic spectrum at very low rate [proved, unrefereed]

**Statement.** Gaussian model, γ₁ = … = γₙ = γ, any encoder of section 0 (the text the referees saw required a
deterministic encoder, which the proof does not use). Let ρₙ(P) be the P-quantile of the χₙ law. Then
E_g h(c, ·) ≥ √γ·E‖gₙ‖·ρₙ(P_c) for every message value with P_c > 0, and

    E h ≥ √γ·E‖gₙ‖·ρₙ(e^(−H(c))).

The right side tends to infinity as H → 0, as the truth does (a cell of mass near 1 is nearly unbounded).

**Proof.** Let K_c be the closed support of ν_c. P_c ν_c ≤ N_n and ν_c(K_c) = 1 give N_n(K_c) ≥ P_c, for a
randomised encoder as for a deterministic one. Urysohn (step 3 of Theorem 1):
E_g s_c(g) ≥ √γ·E‖gₙ‖·(vol(K_c)/ωₙ)^(1/n). The Gaussian density decreases with the radius, so among sets of a given
volume the centred ball has the largest Gaussian measure; hence the centred ball with the volume of K_c has
measure at least P_c, and its radius is at least ρₙ(P_c). For the average, write P = e^(−u). Then
dρₙ/du = −F(ρ)/f(ρ) with F and f the distribution function and density of χₙ, and this is non-decreasing in u
exactly when F/f is non-decreasing in ρ, that is when F is log-concave. The χₙ density is log-concave for n ≥ 1, so
F is (Prékopa). Jensen in u gives Σ_c P_c ρₙ(P_c) ≥ ρₙ(e^(−H)). ∎

It does not use sections 1 to 4. It beats Theorem 1 only at very low rate [computed, `check_f4.py`, log
`check_f4.log`]: at n = 76 it is 1.186n, 1.100n and 1.014n at 0.01, 0.1 and 0.5 nats, against Theorem 1's 1.033n,
1.032n and 1.027n; at n = 768, 1.059n and 1.033n at 0.01 and 0.1 nats against 1.005n. So the crossing is at a few
tenths of a nat for n = 8, 76 and 768. At n = 1 and 2 it is below 0.1 nat (n = 1: 2.057 against 1.632 at 0.01 nat,
1.331 against 1.492 at 0.1 nat), as the extensions referee points out. The script also checks the convexity of
u ↦ ρₙ(e^(−u)) on a grid for n = 1 to 768 and the cell statement on centred and off-centre balls; the referee
adds 3,000 partitions of the line into intervals and of the plane into annuli (smallest ratio 1.135) and a check
that ln(F/f) never decreases (`referee/ref_f3_f4.py`). At high rate F4 is 1/√e below Theorem 1, for the same reason
as Proposition A: the cell-by-cell bound cannot use the averaging of second moments. The anisotropic question it
points to is open: the least Gaussian mean width of Γ^½K among sets K of given Gaussian measure.

### F5. Shared information and groupings [proved, unrefereed]

Restated after the extensions referee's report, which found the mathematics right and the statement short of what
it needs.

**Statement (shared information).** Let M items have independent residuals with the same spectrum γ and the same
negentropy J (for instance identically distributed). Let item j have a message c_j, and let all share a message s
that may depend on every item. Let Δ be independent of all M residuals and of every message. If the mean of
H(c_j) is at most B nats and H(s) ≤ S nats, the mean over items of E h is at least V_S(B + S/M + J) for every head
set, in particular W(B + S/M + J). The certificate for item j may read every item's message and s.

**Proof.** For item j the message is c^M = (c₁, …, c_M) together with s. It is a randomised function of r_j whose
randomness is the other items, and it is independent of Δ, so section 0 allows it. F1 to F3 give
E h_j ≥ V_S(I(z_j; c^M, s) + J). By independence of the items
Σ_j I(z_j; c^M, s) ≤ I(z^M; c^M, s) ≤ Σ_j H(c_j) + H(s), as in Corollary 2 of the consolidated proofs. V_S is convex
and non-increasing (L4.1), so the mean over j of V_S(I_j + J) is at least V_S of the mean rate plus J. ∎

Both hypotheses in the first sentence are needed: without a common spectrum and a common J there is no single V_S
and no single J to put the mean rate into. Independence is needed as in Corollary 2.

**Statement (a grouping).** Hold the law of Δ fixed and independent of the item x. Let V₁, …, V_K be a partition of
space with fixed centres m_ℓ, ℓ the label of x, r = x − m_ℓ, an encoder of r with message c, and a sound
certificate for Δ·r. Together they are a sound certificate of the same width for Δ·x with message (ℓ, c). So

    E h ≥ V_S^x(H(c | ℓ) + H(ℓ) + J_x),     H(ℓ) ≤ ln K,

where V_S^x is the floor for the spectrum of x about the fixed origin (the eigenvalues of Σ_Q^½ E[xxᵀ] Σ_Q^½) and
J_x its negentropy. This is Proposition 1 of `groups/REPORT.md` with V_S in place of Theorem 1, and its caveats
carry over (fixed centres, the offset law held fixed).

What this does not say: that the floor computed with the within-group Σ_R is lowered by at most ln K nats. The
sentence the referees saw ("a partition into K groups lowers the floor by at most ln K nats of rate") could be
read that way, and that reading is a different statement, which is not proved here.

### F6. Offsets that are scale mixtures of Gaussians [proved, unrefereed]

**Statement.** Let Δ = t·Σ_Q^½ g with t ≥ 0 independent of g and of (r, c), and keep the rest of the model. Then
E h ≥ E[t]·V_S(I(z; c) + J) for every S.

**Proof.** Condition on t. The model holds with Σ_Q replaced by t²Σ_Q; z, the law of the message and J do not
change with t; every γᵢ is multiplied by t²; the water-filling is invariant to the scale; and W and V_S are
multiplied by t. The certificate may depend on t through Δ, which the conditioning allows. Average over t. ∎

Here Σ_Q is the covariance of the Gaussian part. The covariance of Δ is E[t²]·Σ_Q, so against the floor computed
from the offsets' own covariance the factor is E t/√(E t²) ≤ 1 (0.975, 0.869, 0.607 for a lognormal scale with
σ² = 0.05, 0.28, 1; `check_after_referees.py`, line 10). The extensions referee asked for this line.

This is the only step taken here towards offsets that are not Gaussian. Offsets that depend on the residual, and
offsets carried by a few directions, are outside it (the section 6 remark that the floor is false for offsets on
finitely many directions stands).

### F2-alt. The brief's order: head point first [sketch: gaps named]

Recorded because the brief asked what it proves and what stops it. It is dominated by F2.

**The argument.** One cell A ⊂ ℝ^S × ℝ^T with ν = N_n(· | A), P = N_n(A), deterministic encoder. For a head point s
let p(s) = N_T(A_s) be the Gaussian mass of its fibre; the fibre's law has KL(ν_{T|s} ‖ N_T) = ln(1/p(s)). Write κ_S for the
KL of the head marginal and κ̄_T for the mean fibre KL; κ_S + κ̄_T = ln(1/P). Fix ε ∈ (0, 1) and let
G = {s : ln(1/p(s)) ≤ κ̄_T/ε}; by Markov ν_S(G) ≥ 1 − ε. Choose s*(g_S) in G within η of the supremum of
Σ_S √γᵢ gᵢ sᵢ over G, then couple g_T to the fibre over s* by its triangular map. Then

- tail, given g_S: at least Φ_T(ln(1/p(s*))) ≥ Φ_T(κ̄_T/ε), by Proposition A(a) for the fibre law;
- head: Urysohn on G, with ln vol(G) ≥ h(ν_S(· | G)) and
  KL(ν_S(· | G) ‖ N_S) ≤ (κ_S + ln 2)/(1 − ε), from the decomposition of ν_S over G and its complement.

Averaging over cells by Jensen gives, for every ε,

    E h ≥ min over H_S + H_T ≤ H of { (κ_k/√e)·(Π_S γᵢ)^(1/2k)·exp(−(H_S + ln 2)/((1 − ε)k)) + Φ_T(H_T/ε) }.

**What it proves.** Theorem 3 with the rate split between head and tail instead of charged twice, and Φ_T in place
of the concentration term.

**What stops it from being F2.** The head point is chosen by a support function, so it is an extreme point of G and
its law is not the head marginal of the cell. Three losses follow, and none can be averaged away:
1. every fibre over G must be good, which Markov buys at a factor 1/ε in tail rate;
2. the fibre's second moments at the chosen point are uncontrolled, so the tail has only Φ (loss √H_T), not W
   (loss H_T);
3. conditioning the head law on G costs 1/(1 − ε) in head rate and, because the second moment of the head law on G
   is not the average any more, the 1/√e of Theorem 3 stays.

**Gaps.** I did not write out the measurable choice of s* or the second-moment estimate on G that would recover
part of loss 3. Neither is worth doing now that F2 holds.

---

## 6. Prior art, as read [cited; sources read where quoted]

P1 and P2 were read by me from the arXiv TeX sources on 21 November 2026 (`NOTEBOOK.md`, entry 5), and again by
the extensions referee, who confirms every statement of fact below and found two wordings overstated; both are
corrected here. P5 to P7 were read by the librarian (`../library/floors-novelty.md`), not by me.

**P1. Ingber, Courtade, Weissman, "Compression for quadratic similarity queries", arXiv:1307.6609, IEEE Trans. Inf.
Theory 61(5), 2015.** Signatures of nR bits for i.i.d. Gaussian vectors, a query answered no/maybe with no false
negative for (1/n)‖x − y‖² ≤ D, rate achievable if Pr[maybe] → 0. Theorem 1: for N(μ, σ_X²) against N(μ, σ_Y²),
R_ID = log₂(2σ_Xσ_Y/(σ_X² + σ_Y² − D)) between (σ_X − σ_Y)² and σ_X² + σ_Y². The converse is proved: typical shell,
projection of each cell on the sphere, Lévy's isoperimetric inequality, a Markov-type lemma over cells; it gives
the bound with O(log n/n) terms and a shell thickness that is sent to zero. Theorem 4: the Gaussian pair needs the
largest rate for given variances (identical means, finite variances), by achievability; no converse for other
laws.
- In our terms [my arithmetic, as the librarian's and the referee's]: the least inner-product level per coordinate
  that can be certified at a typical query is σ_Xσ_Y·2^(−R) = √min(γ, θ). So for an isotropic spectrum F1 is the
  finite-n, mean-width counterpart of their converse.
- Tightness (corrected). Their achievability shows that F1's level is reached **in probability**: the certified
  width is at most n(√θ + o(1)) with probability tending to 1. It does not show that F1 is tight, because F1 is
  about the mean and their scheme answers "maybe" for every item outside the typical shell, so its mean width is
  infinite at every n. A scheme that also bounds the atypical items (the norm stored on a grid at o(n) nats) would
  close this; it is plausible and it is not in the paper. Section 3 lists the computed codes that come within
  0.06% to 0.3% of W in the mean.
- It does not give: a bound at a given n; a bound on E h; anisotropic spectra.
- "Theorem 1 at k = n is the isotropic finite-n analogue of their converse" (P3) is an analogy: theirs bounds a
  probability at a threshold, Theorem 1 a mean width.

**P2. Wu, Wang, Flierl, "Computing similarity queries for correlated Gaussian sources", arXiv:2001.07941** (one
arXiv version, no journal reference). Theorem 1 states R = (1/M)Σ max(0, log(ξ_m/τ)) at
D = (1/M)Σ max(0, 2(ξ_m − τ)) for query and data of the same covariance; with γ = ξ², θ = τ² that is the curve of W.
What the paper proves is an optimal allocation of thresholds inside one family of schemes (KLT, a system per
component, answers combined by AND). It has no converse: the passage from the family to all schemes is an
assertion. The proof of its Lemma 1 (not the lemma's statement, a precision the referee adds) equates
{Σ d_m ≤ Σ D_m} with {d_m ≤ D_m for all m}; the second event is a strict subset, so the equality is false and, as an
inequality, goes the wrong way for an upper bound on Pr[maybe]. The AND rule as stated can answer no for a pair
within the total threshold, so the achievability is also not established as written.
- So the anisotropic formula was stated before; a proof of its converse was not in either paper. Neither I nor the
  referee searched for a corrected version.

**P3. Which route.** I kept the transport route and did not adapt the Ingber–Courtade–Weissman converse. Their
argument needs the query uniform on a sphere on which the cell is projected, so both laws must be isotropic; the
authors say the general isoperimetric inequality is unknown. At finite n its shell corrections do not vanish. The
isotropic finite-n counterpart of their converse, in the mean-width form, is Theorem 1 at k = n (Urysohn in place
of Lévy), and F2 contains it.

**P4. Tensorisation.** The chain-rule step is the Marton–Talagrand tensorisation (Gozlan–Léonard 2010, Proposition
1.8) [cited, unchecked: librarian's reading]; as the PI notes it cannot be cited as it stands for a bilinear cost,
and L2 proves what is needed directly.

**P5. Bolley, Gentil, Guillin, Ann. Probab. 46 (2018), arXiv:1507.01086** [cited: read by the librarian]. Theorem
2.1 is the dimensional Talagrand inequality; with the standard Gaussian and n = 1 it is L1 (section 1). The
Gaussian case is due to Bakry, Bolley and Gentil (2012) [cited, unchecked].

**P6. Steiner, Dempfle, Ingber, Weissman, "Compression for quadratic similarity queries: finite blocklength and
practical schemes", arXiv:1404.5173** [cited: read by the librarian]. The finite-n converse of the
Ingber–Courtade–Weissman line: a lower bound on Pr[maybe] built from the shell densities of the two norms and a
cap-measure function. It is isotropic, it bounds a probability and not a width, and it has no closed form. It is
the nearest finite-n predecessor of F1 and was not cited in the text the referees saw.

**P7. Balasubramanian, Bhandari, Kasiviswanathan, "Query-dependent reading for quantized inner-product estimation:
a minimax separation", arXiv:2610.24890** [cited: read by the librarian, first 100,000 characters]. The newest
converse for inner products found. It bounds the worst-case mean squared error of an estimate, not a sound
interval; its full-read limit is 1/(4^R − 1); it uses volumes of cells and has no water-filling. Close in spirit,
a different quantity.

**P8. The claim.** F1 is worded in section 3 as the first proof of the converse at finite n, for covariances that
need not commute, in the mean-width form, as far as the librarian's search goes. The formula is not claimed. Also
relevant and not opened by anyone here: Dumer, Pinsker and Prelov (2004), reverse water-filling for coverings of
ellipsoids [cited, unchecked].

---

## 7. Evaluation on the measured spectra [computed]

`floors_eval.py`, log `floors_eval.log`. Spectra: `agent1/spec_v1.json`, the two largest groups, γ = λ·Q_e as
`agent1/theory.py` (diagonal Σ_Q in the residual basis, the referee's caveat). Bits charged: code bits + 48 (three
stored scalars), as `theory.py`. Measured widths: `agent1/theory.log`, Gaussian data with these spectra [measured
there, not re-run].

Group 12 (49,674 items, F = 0.0728, n_eff = 76):

| Code bytes | Theorem 1 | Theorem 2 | Theorem 3 | Φ (A) | W (F1) | V (F2), head k | Uniform code | Escape code | Escape / V | Escape / best old |
|---|---|---|---|---|---|---|---|---|---|---|
| 0 | 0.0318 | 0.0092 | 0.0237 | 0.0361 | 0.0453 | 0.0462, 49 | 0.0724 | 0.0724 | 1.57 | 2.28 |
| 4 | 0.0278 | 0.0037 | 0.0186 | 0.0295 | 0.0379 | 0.0386, 67 | 0.0534 | 0.0539 | 1.40 | 1.94 |
| 8 | 0.0247 | 0.0014 | 0.0156 | 0.0250 | 0.0327 | 0.0333, 82 | 0.0446 | 0.0445 | 1.34 | 1.80 |
| 16 | 0.0204 | 0 | 0.0123 | 0.0193 | 0.0258 | 0.0261, 115 | 0.0344 | 0.0339 | 1.30 | 1.66 |
| 32 | 0.0150 | 0 | 0.0091 | 0.0130 | 0.0180 | 0.0181, 171 | 0.0238 | 0.0229 | 1.26 | 1.53 |
| 96 | 0.0063 | 0 | 0.0038 | 0.0046 | 0.0068 | 0.0068, 339 | 0.0095 | 0.0087 | 1.27 | 1.39 |

Group 2 (34,396 items, F = 0.1523, n_eff = 72):

| Code bytes | Theorem 1 | Theorem 2 | Theorem 3 | Φ (A) | W (F1) | V (F2), head k | Uniform code | Escape code | Escape / V | Escape / best old |
|---|---|---|---|---|---|---|---|---|---|---|
| 0 | 0.0671 | 0.0166 | 0.0483 | 0.0739 | 0.0934 | 0.0955, 49 | 0.1520 | 0.1520 | 1.59 | 2.27 |
| 4 | 0.0577 | 0.0057 | 0.0381 | 0.0599 | 0.0775 | 0.0790, 62 | 0.1115 | 0.1128 | 1.43 | 1.95 |
| 8 | 0.0509 | 0.0015 | 0.0317 | 0.0505 | 0.0666 | 0.0676, 81 | 0.0928 | 0.0924 | 1.37 | 1.82 |
| 16 | 0.0412 | 0 | 0.0249 | 0.0385 | 0.0518 | 0.0525, 114 | 0.0707 | 0.0694 | 1.32 | 1.69 |
| 32 | 0.0295 | 0 | 0.0178 | 0.0254 | 0.0352 | 0.0356, 166 | 0.0479 | 0.0459 | 1.29 | 1.56 |
| 96 | 0.0115 | 0 | 0.0070 | 0.0085 | 0.0126 | 0.0127, 322 | 0.0180 | 0.0164 | 1.29 | 1.43 |

Reading.
- The W column equals `theory.log`'s "conjectured" column to four digits. The best proved floor moves from 0.70 of it
  at 0 bytes (and 0.93 at 96) to 1.00 to 1.02 of it. In every row the minimum over the split in V is at u = 0.
- Theorems 2 and 3 are never the best. Proposition A's Φ beats Theorem 1 up to 4 bytes and loses to it from 16.
- Theorems 2 and 3 are evaluated with the consolidated constants (1/√2), so their columns are slightly above
  `theory.log`'s, which used 1.
- On Gaussian data the escape code is 1.26 to 1.59 above the best proved floor, where it was 1.39 to 2.28.

**Beside `quant/REPORT.md`** ("Distance from the floor", `quant/floor2.py`, logs `quant/logs/floor_p768.log`,
`floor_n1024.log`). That table divides measured Cauchy–Schwarz widths on real residuals by W ("conjectured") and by
Theorem 1 ("proved") at every stored bit. With F1, its first ratio is a ratio to a floor proved for the Gaussian
model: BioCLIP-2 after the rework 1.17 to 1.21, BioCLIP-2.5 1.17 to 1.18 (before: 1.34 to 1.37 and 1.30 to 1.31),
in place of 1.42 to 1.66 against Theorem 1. I could not evaluate V on that index's spectra (the index is not on
this machine). On the agent 1 spectra at the same stored bits (52 to 296 bits) V/W is 1.011 to 1.021, so the
ratios to V would be about 1.15 to 1.20 [estimate by transfer between spectra, not computed on the index].
Two caveats are unchanged: the widths are for real residuals and the floor is for Gaussian ones.

**Non-Gaussian correction** (F3). W(H + J)/W(H) on group 12, at 0 / 4 / 8 / 16 / 32 / 96 bytes:
J = 23 bits: 0.88 / 0.90 / 0.91 / 0.93 / 0.95 / 0.97; J = 80 bits: 0.68 / 0.72 / 0.75 / 0.79 / 0.83 / 0.88. The J
values are the lower bounds of `entropy/REPORT.md`. So these factors bound from above what F3 can prove for the
real data, W(H + J_true) ≤ W(H + J_lower). They say nothing about the least possible width for the real data,
which may lie above or below (wording corrected after the extensions referee's report).

**An independent evaluation.** The falsifier computed the same floors on the same two groups without reading this
directory (`../stage0-floor-check/RESULTS.md`, section 4). Its Theorem 1, 2, 3, Φ and W columns equal the ones
above to four digits. Its F2 column equals V to within one unit in the fourth digit; its best head differs by a
few coordinates (48 against 49 at 0 bytes, 64 against 67 at 4) because it searched a subset of k.

---

## 8. Consequence for the design at low rate

### D1. The norm-only dual-basis certificate [proved, unrefereed; numbers computed]

**Scheme.** Store ρ̂, the least point of the grid {ρ₀ qʲ : j ∈ ℤ} that is at least ρ = ‖r‖_{M⁻¹}, and certify
h = ‖Δ‖_M·ρ̂. The message is j. Here Σ_Q and Σ_R are positive definite, so that M and M⁻¹ exist (a hypothesis the
text the referees saw left unstated). Write X for ‖Δ‖_M; in eigen-coordinates X² and ρ² both have the law of
Σᵢ √γᵢ gᵢ², so E X² = F, and let cv² = Var(X)/(E X)².

**Theorem D1.** Under the Gaussian model, with Δ independent of r:
1. the certificate is sound, and E h = E X·E ρ̂ ≤ q·(E X)² = q·F/(1 + cv²) ≤ q·F;
2. (E X)² ≥ F·(1 − 1/n_eff)², so the mean width is F up to the grid step and 2/n_eff;
3. every sound certificate whose message has entropy at most H_q = H(j) has mean width at least
   sup_S V_S(H_q) ≥ W(H_q) ≥ F − √γ₁·H_q;
4. hence the scheme's mean width is at most q/((1 + cv²)(1 − √γ₁ H_q/F)) times the least possible at its own
   entropy, whenever √γ₁ H_q < F.

**Proof.** (1) Cauchy–Schwarz in the M-norm and ρ̂ ≥ ρ; Δ and r are independent; ρ̂ ≤ qρ; E ρ = E X. (2) For x ≥ 0,
√x ≥ 1 + (x − 1)/2 − (x − 1)²/2 (the difference is (√x − 1)²·√x(√x + 2)/2). With x = X²/F and
Var(X²) = 2Σγᵢ: E X ≥ √F(1 − Σγᵢ/F²). (3) F2 and L4. (4) Divide. ∎

**Numbers** (`design_lowrate.py`, log `design_lowrate.log`; group 12, n_eff = 76, cv² = 0.0065; group 2 is within
0.4% of these). E X exactly from the Laplace transform; H_q from 200,000 samples.

| Grid ratio q | H_q (bits) | E h / F | W(H_q)/F | V(H_q)/F | E h / W | E h / V |
|---|---|---|---|---|---|---|
| 2^(1/4) | 1.20 | 1.085 | 0.974 | 1.015 | 1.114 | 1.069 |
| 2^(1/8) | 2.00 | 1.038 | 0.960 | 0.998 | 1.082 | 1.040 |
| 2^(1/16) | 2.95 | 1.016 | 0.944 | 0.980 | 1.076 | 1.036 |
| 2^(1/32) | 3.94 | 1.004 | 0.929 | 0.963 | 1.082 | 1.043 |
| 2^(1/64) | 4.94 | 0.999 | 0.914 | 0.947 | 1.093 | 1.055 |
| 2^(1/256) | 6.93 | 0.995 | 0.889 | 0.919 | 1.120 | 1.083 |

Reading.
- With the shipped grid 2^(1/16) the norm carries 2.95 bits of entropy, and no certificate of any kind whose
  message has entropy at most 2.95 bits can be more than 3.6% narrower on the Gaussian model. (The floor is in
  entropy. The grid is unbounded, so the scheme has no fixed-length 3-bit form; "a 3-bit message", as the referees
  saw it, was loose.) At zero code bytes the dual-basis ball is the right certificate, to that margin.
- The ratio is smallest near q = 2^(1/16). A finer norm buys nothing (the width is already F) while the floor at its
  entropy keeps falling, so past about 3 bits the norm has nothing more to give and the floor leaves room for
  something else to. That is all the theorem shows. It does not show that a bit spent on a coordinate gains that
  room: a floor says what any code could gain at most, and no coordinate code is analysed here. (The text the
  referees saw said "a bit is better spent on a coordinate than on the norm", which is stronger than what is
  proved; the measured codes of section 7, 1.26 to 1.59 above the floor, are the evidence for coordinates, and
  they are evidence, not a theorem.) A floor for any b-bit code, as a share of F: 0.962 at 4 bits, 0.905 at 8,
  0.823 at 16 (V; W is 0.928, 0.876, 0.800).
- H_q is small because the norm concentrates: ln ρ has standard deviation about (2n_eff)^(−½) = 0.081, about 1.9
  grid steps at 2^(1/16), and ½ log₂(2πe(1.9² + 1/12)) = 2.97 bits against the 2.95 measured.
- The bound of item 4 is 1.13 at 2^(1/16); the computed ratio is tighter because V is used in place of F − √γ₁H.
- H_q is a plug-in entropy from 200,000 samples on about 20 occupied cells; its bias is about 10⁻⁴ bits (the
  referee's estimate) and changes no figure. The referee reran the script and got the same log.

---

## Task B: certificates as optimal transport

Local symbols. For a metric M ≻ 0 and a query family with second-moment matrix Σ,
ω(M; Σ) = √(tr(MΣ)·tr(M⁻¹Σ_R)) is the root-mean-square width of the certificate ‖Δ‖_M‖r‖_{M⁻¹}; it does not change
when M is scaled. F(Σ) = F(Σ, Σ_R) = tr(Σ^½ Σ_R Σ^½)^½. C*(P, P′) is the largest E⟨Δ, r⟩ over couplings of two laws
(the maximal correlation); for centred laws C* = ½(E‖Δ‖² + E‖r‖² − W₂²). Checks: `ot_checks.py`, log
`ot_checks.log`.

## 9. B1. Proposition 3 is Kantorovich duality [cited]

**Statement.** For positive definite Σ_Q and Σ_R:

(a) min over M ≻ 0 of ½[tr(MΣ_Q) + tr(M⁻¹Σ_R)] = min over M ≻ 0 of √(tr(MΣ_Q)·tr(M⁻¹Σ_R)) = F, at
    M = Σ_Q⁻¹ # Σ_R. This is Theorem 2, equations (15) and (16), of Bhatia, Jain and Lim, Expo. Math. 37 (2019),
    arXiv:1712.01504, and it is Proposition 3 of `groups/REPORT.md`. I have not read the source. The librarian
    read it, and the extensions referee read the arXiv TeX and confirms the attribution, the equation labels and
    the minimiser X₀ = A⁻¹ # B (the journal version was not opened). The tag is left as it was for the PI.
(b) Any pair (Δ, r) on one probability space with second-moment matrices Σ_Q and Σ_R has E⟨Δ, r⟩ ≤ F.
(c) If Δ ~ N(0, Σ_Q) then r = MΔ ~ N(0, Σ_R) and E⟨Δ, MΔ⟩ = F.

So F is the maximal correlation of the two Gaussians (the Bures fidelity), M is the Monge map, φ(Δ) = ½ΔᵀMΔ is the
Brenier potential (its gradient is the map) with conjugate φ*(r) = ½rᵀM⁻¹r, and Proposition 3 is the dual problem
restricted to quadratic potentials, where the optimum lies for Gaussians. B1 is a citation, not a result.

**Proof of (b) and (c)**, for completeness. Fenchel–Young: ⟨Δ, r⟩ ≤ ½ΔᵀMΔ + ½rᵀM⁻¹r for every M ≻ 0. Take
expectations and minimise over M, using (a). For (c), MΣ_QM = Σ_R by substitution, and
tr(MΣ_Q) = tr(Σ_Q^½ Σ_R Σ_Q^½)^½. ∎

**Corrections to the brief, accepted.** F is the value for the product of second moments; the mean certificate is
smaller (B2b gives its value and its minimiser). The gain of the best φ + φ* is F − C* = ½(W₂² − G²). φ + φ* is
one-sided (B3d).

**Check.** 300 non-commuting pairs, n ≤ 7: MΣ_QM = Σ_R and both traces equal F to 9·10⁻¹⁴; no random metric below F
(smallest ratio 1.0014); no random coupling above F (largest 0.75). The extensions referee reran `ot_checks.py`
and got the same log.

## 10. B2. No norm beats the M-ellipsoid [B2a, B2c: proved, unrefereed; B2b: cited, with one remark of ours]

**B2a (second moments).** For every norm ‖·‖_K with dual norm ‖·‖_K°, and any laws of Δ and r,
with finite second moments, √(E‖Δ‖²_K·E‖r‖²_K°) ≥ C*(P_Δ, P_r). For the Gaussians the right side is F (B1(b), (c)),
attained by the M-ellipsoid.

*Proof.* Under any coupling ⟨Δ, r⟩ ≤ ‖Δ‖_K‖r‖_K°; take expectations and use Cauchy–Schwarz. The left side depends
on the marginals only. ∎ (The librarian gives the same argument through Fenchel–Young.)

**B2b (the mean, with Δ and r independent).** Let a, b ≥ 0 be functions with ⟨Δ, r⟩ ≤ a(Δ)·b(r) for all Δ and r;
a norm and its dual are one case. Then for every coupling π of the two laws

    E a(Δ)·E b(r) ≥ (E_π √(⟨Δ, r⟩⁺))².

For Δ ~ N(0, Σ_Q) and r ~ N(0, Σ_R):

    E a(Δ)·E b(r) ≥ (E‖Δ‖_M)² = E‖Δ‖_M·E‖r‖_{M⁻¹} = F/(1 + cv²),      F·(1 − 1/n_eff)² ≤ (E‖Δ‖_M)² ≤ F,

with cv the coefficient of variation of ‖Δ‖_M. **So the M-ellipsoid minimises the mean certificate too**, over all
norms and over all certificates of product form.

*Proof.* √(⟨Δ, r⟩⁺) ≤ √a(Δ)·√b(r) pointwise. Under π, Cauchy–Schwarz gives
E_π √(⟨Δ, r⟩⁺) ≤ √(E a·E b), and the right side depends on the marginals only. For the Gaussians take r = MΔ:
⟨Δ, MΔ⟩ = ‖Δ‖²_M. Also ‖MΔ‖_{M⁻¹} = ‖Δ‖_M, so ‖r‖_{M⁻¹} has the law of ‖Δ‖_M and the product of the two means is
(E‖Δ‖_M)². E‖Δ‖²_M = F gives F/(1 + cv²); the lower bound is item 2 of D1. ∎

**What is ours and what is cited** (after the librarian's report, `../library/floors-novelty.md`, section 4). B2b is
the classical one-line proof that the mean norm times the mean dual norm is at least the squared mean Euclidean
norm, M·M* ≥ 1: for one Gaussian vector G, E|G| ≤ E(‖G‖_K^½‖G‖_K°^½) ≤ (E‖G‖_K)^½(E‖G‖_K°)^½. The librarian read it in
Aubrun, Szarek and Ye, Comm. Pure Appl. Math. 67 (2014), Appendix D (arXiv:1106.2264) [cited: read by the
librarian]; the context is Pisier (1989), chapter 3 [cited, unchecked]. It applies here after the change of
variables u = M^½Δ, v = M^(−½)r: because MΣ_QM = Σ_R, u and v have the same law, Δ·r = u·v, and E|u| = E‖Δ‖_M. The
Monge coupling r = MΔ in the proof above is v = u in these coordinates. What is ours is only that remark: the
geometric-mean metric is the linear map that turns the two Gaussian laws into one, so that M·M* ≥ 1 applies to a
query and an item with different covariances. It is a remark, not a theorem. The tag of B2b is changed accordingly
from [proved, unrefereed] to [cited] plus the remark.

It still answers the librarian's earlier caution (`library/theory.md`, 2.2 item 1) that the minimiser of the mean
"need not be the geometric mean": it is the same metric.

**B2c (scale mixtures).** If Δ = t·Δ₀ and r = s·r₀ with Gaussian Δ₀, r₀ and scales t, s ≥ 0 independent of them,
then for every norm E‖Δ‖_K·E‖r‖_K° = E t·E s·E‖Δ₀‖_K·E‖r₀‖_K° ≥ E t·E s·(E‖Δ₀‖_M)². The M-ellipsoid of the Gaussian
parts stays the best norm.

**What is not claimed.** Laws far from elliptical. For Δ uniform on {±e₁, ±e₂} and r uniform on
{±(e₁ ± e₂)/√2}, the pair ‖·‖₁, ‖·‖_∞ certifies 1/√2 where every ellipsoid certifies 1 (confirmed by the extensions
referee on 20,000 random ellipsoids). B2c is for norms: it uses homogeneity and does not extend to product forms
that are not homogeneous.

**Check.** 240 random norms (‖Ax‖_p with its dual for p ∈ {1, 1.5, 2, 3, ∞}, and random ellipsoids), n ∈
{1, 2, 3, 5, 8}, 200,000 sample pairs each. Sound on every pair (smallest margin −6·10⁻¹⁵, rounding). Smallest
mean-product ratio to the M-ellipsoid 1.0000 (one dimension, where all norms are proportional); smallest
second-moment ratio to F 0.9947 (the same case, sampling error of the squares). (E‖g″‖)²/F = 0.637, 0.784, 0.939,
0.987 at n_eff = 1, 2, 7.9, 36.9, and 1/(1 + cv²) agrees to 10⁻³. The extensions referee found no error in B2a, B2b
or B2c.

## 11. B3. One-scalar certificates Δ·r ≤ φ(Δ) + φ*(r) [proved, unrefereed; strong duality cited]

**B3a (weak duality).** For every function φ with convex conjugate φ*, E φ(Δ) + E φ*(r) ≥ C*(P_Δ, P_r).
*Proof.* Fenchel–Young under any coupling. ∎

**B3b (the optimum) [cited, unchecked].** The infimum over φ equals C*, and it is attained by the Brenier potential
between the true laws when P_Δ has a density and the second moments are finite (Brenier 1991; Villani, *Topics in
Optimal Transportation*, Theorem 2.12).

**B3c (the gain).** For laws with second-moment matrices Σ_Q and Σ_R, C* ≤ F (B1b). The gain of the best φ + φ*
over the quadratic one is F − C* = ½(W₂² − G²) ≥ 0 for centred laws, G² = tr Σ_Q + tr Σ_R − 2F the Gelbrich bound.

**B3d (two sides).** φ + φ* bounds Δ·r from above only. If both laws are symmetric, φ may be taken even without
loss, and then it bounds |Δ·r|. *Proof.* Let ψ(x) = ½(φ(x) + φ(−x)). E ψ(Δ) = E φ(Δ) by symmetry. The conjugate of
an average is at most the average of the conjugates, so ψ*(y) ≤ ½(φ*(y) + φ*(−y)) and E ψ*(r) ≤ E φ*(r). With ψ
even, −⟨Δ, r⟩ = ⟨−Δ, r⟩ ≤ ψ(Δ) + ψ*(r). ∎

**B3e (scale mixtures).** Let Δ = t·Δ₀, r = s·r₀ as in B2c, with E t² = E s² = 1, and X = ‖Δ‖_M, Y = ‖r‖_{M⁻¹}.
Write C₁(U, V) = ∫₀¹ Q_U Q_V du for the comonotone expectation of two positive variables. Then

    F·C₁(t, s) ≤ C* ≤ C₁(X, Y) ≤ F,      so      F − C₁(X, Y) ≤ F − C* ≤ F·(1 − C₁(t, s)).

*Proof.* Lower bound: couple r₀ = MΔ₀ and (t, s) comonotone, independent of Δ₀; this is a coupling of the two laws
with E⟨Δ, r⟩ = E[ts]·F. Upper bound: ⟨Δ, r⟩ ≤ XY, and E[XY] is largest under the comonotone coupling
(Hardy–Littlewood) [cited, standard]; then Cauchy–Schwarz. ∎

With Gaussian queries (t ≡ 1) the gain is at most F·(1 − E s). For a lognormal scale with E s² = 1, E s = e^(−σ²/2)
and the kurtosis of a coordinate of r is 3e^(4σ²). At the kurtoses of `entropy/REPORT.md` section 1, on the
spectrum of group 12 [model: lognormal scale]:

| Kurtosis | σ² | Gain of the best φ + φ*, share of F | Mean of the product form with the stored norm, share of F |
|---|---|---|---|
| 3.6 (BioCLIP-2, low) | 0.046 | 1.2 to 2.3% | 0.970 |
| 4.7 (BioCLIP-2, high) | 0.112 | 3.3 to 5.5% | 0.940 |
| 5.1 (BioCLIP-2.5, low) | 0.133 | 4.4 to 6.4% | 0.928 |
| 9.2 (BioCLIP-2.5, high) | 0.280 | 10.1 to 13.1% | 0.861 |

How large can it be: the gain has no bound below F. The text the referees saw supported this with
1 − E s/√(E s²) → 1, which is the upper bound on the gain and proves nothing about its size; the extensions
referee supplied the missing argument. What is needed is that the lower bound F − C₁(X, Y) tends to F. By Hölder,
C₁(X, Y) ≤ ‖X‖₄·‖Y‖_{4/3}, and ‖Y‖_{4/3} = ‖s‖_{4/3}·‖Y₀‖_{4/3} with Y₀ = ‖r₀‖_{M⁻¹}, because s is independent of r₀. For
the lognormal scale ‖s‖_{4/3} = e^(−σ²/3) → 0 as σ² → ∞, while ‖X‖₄ and ‖Y₀‖_{4/3} do not depend on σ. So C* ≤ C₁(X, Y) → 0
and the gain tends to F. Numerically, on a 16-coordinate spectrum (`check_after_referees.py`, line 5): the gain
is at least 0.07·F, 0.29·F, 0.81·F and 0.98·F at σ² = 0.28, 1, 4 and 9. At the measured kurtoses it is 1 to 13% of F.

**B3f (the stored norm already has it).** For the same scale mixtures the product form satisfies

    E‖Δ‖_M·E‖r‖_{M⁻¹} = E t·E s·(E‖Δ₀‖_M)² ≤ E t·E s·F ≤ C₁(t, s)·F ≤ C* ≤ E φ(Δ) + E φ*(r) for every φ.

(The middle step is Chebyshev's inequality for comonotone variables.) So with one stored norm the design's
certificate is already narrower on average than the best certificate of the form φ + φ*, Brenier potential
included; the last column of the table is below 1 minus the gain in every row. A non-quadratic potential is not
worth building for scale mixtures.

## 12. B4. Several scalars, and one basis for a mixture of query families

The PI redirected this section after the block-norm experiment (`notes/program/stage0-block-norms/REPORT.md`):
the identities are stated and the measurement cited, and the weight is on one basis for several query families.

### B4a. Several stored scalars [identities proved; the measurement cited]

1. *Intersection.* If J norms ρⱼ = ‖r‖_{Kⱼ°} are stored, then for every split Δ = Σⱼ Δⱼ,
   ⟨Δ, r⟩ ≤ Σⱼ ρⱼ‖Δⱼ‖_{Kⱼ}; the least such bound is the support function of ∩ⱼ ρⱼKⱼ°, the infimal convolution of the J
   support functions (Rockafellar, *Convex Analysis*, Corollary 16.4.1) [cited, unchecked: from memory]. In general
   it is a convex programme per (query, item).
2. *Block norms.* For blocks j with aⱼ = ‖Δⱼ‖ and bⱼ = ‖eⱼ‖,
   Σⱼ aⱼbⱼ = min over D constant on blocks of ‖DΔ‖‖D⁻¹e‖, at dⱼ² = bⱼ/aⱼ (Cauchy–Schwarz and its equality case; when
   some aⱼ or bⱼ is zero the minimum is an infimum). It is the support function of the product of the block balls.
   In item 1 "the least such bound" is the least of the split form; the exact range given the J norms as
   equalities can be smaller.
3. *The Gaussian model of what they buy.* In the dual basis with Gaussian coordinates and Δ independent of e, a
   block has E‖Δⱼ‖ = E‖eⱼ‖ = μⱼ with μⱼ² = Fⱼ/(1 + cvⱼ²), Fⱼ = Σ_{i∈j} √γᵢ. So the mean block bound is Σⱼ μⱼ². With
   one coordinate per block it is (2/π)·F, because (E|g|)² = 2/π: **the best diagonal weighting for each pair is
   (2/π)(1 + cv²) of the single norm**, whatever the basis and the query family, as long as the profile fits.
4. *Against the measurement* [measured there; model computed in `mixture_price.py`, log `mixture_price.log`]. The
   experiment's per-pair limit is 0.63 of one norm for items, for in-sample species prompts, and for held-out text
   in the mixed basis; the model gives 0.641 on its spectra (n_eff 67 to 85). For held-out text in the data-only
   basis it is 0.57 = 0.63 × 0.907, and 0.907 is what refitting the diagonal weights on text gives in that basis
   (its bound b1). So the PI's reading holds with a constant: 2/π is chance alignment inside a pair, and what is
   left is the profile. For J = 2, 4, 8, 16 equal-share blocks the model gives 0.994, 0.981, 0.958, 0.916 of one norm;
   measured for items 0.984, 0.965, 0.937, 0.891 (0.1951, 0.1912, 0.1857, 0.1765 over 0.1982 in the experiment's
   `report_measure.txt`). So the model has 0.38, 0.54, 0.67 and 0.77 of the measured gain: two fifths to three
   quarters, rising with J. The text the referees saw said "two thirds to three quarters", which was wrong for
   J = 2 and 4; the extensions referee's table is the one above.
5. *Against the floor* [computed]. Sixteen norms at about 3 bits of entropy each are 48 bits. On group 12 of the
   agent 1 spectra the floor at 48 bits is V = 0.63·F (section 7, first row), and sixteen block norms reach about
   0.9·F. Bits spent on norms buy a quarter of what the floor allows. "Three bits each" is an estimate, and a low
   one: a block's norm concentrates less than the whole norm, so its entropy on the same grid is higher. That the
   bits are better spent on coordinates is the experiment's byte count, not something a floor shows (see D1).

Block norms are not pursued further.

### B4b. One basis for a mixture of query families [proved, unrefereed]

Families a = 1, …, A with second-moment matrices Σ_a and weights w_a summing to 1. F_a = F(Σ_a), M_a = Σ_a⁻¹ # Σ_R,
ω_a(M) = ω(M; Σ_a), and the regret of a metric for a family is reg_a(M) = ω_a(M)/F_a ≥ 1. Σ_w = Σ_a w_aΣ_a,
F_w = F(Σ_w), M_w = Σ_w⁻¹ # Σ_R.

**Statement.**
1. *Concavity.* F(·) and F(·)² are concave on positive semidefinite matrices, F(cΣ) = √c·F(Σ), and
   F(X + Y) ≤ F(X) + F(Y).
2. *Sum rule.* M_w minimises the mixture's mean-square width Σ_a w_a ω_a(M)², and Σ_a w_a ω_a(M_w)² = F_w².
3. *The price of one basis.* Σ_a w_aF_a² ≤ F_w² ≤ (Σ_a √w_a·F_a)² ≤ A·Σ_a w_aF_a². The first is an equality when one
   metric is optimal for every family. The value A is attained when the families are orthogonal in the Σ_R inner
   product, Σ_aΣ_RΣ_b = 0 for a ≠ b, and the w_aF_a² are equal. (Corrected: the text the referees saw said
   "orthogonal supports", which is the same thing only when Σ_R is a multiple of the identity.)
4. *Two families.* Put δ = (F_w² − w₁F₁² − w₂F₂²)/(2√(w₁w₂)·F₁F₂). Then 0 ≤ δ ≤ 1 and

       w₁F₁²(reg₁² − 1) + w₂F₂²(reg₂² − 1) = 2δ·√(w₁w₂)·F₁F₂,      reg_a = reg_a(M_w).

   The total excess is fixed by δ; the weights decide how the two families share it.
5. *In terms of the spectra.* Let B_a = Σ_R^½ Σ_a Σ_R^½, with eigenvalues γ^(a). Then F_a = tr B_a^½ and
   F_w = tr(w₁B₁ + w₂B₂)^½. If B₁ and B₂ commute, F_w = Σᵢ √(w₁γᵢ^(1) + w₂γᵢ^(2)), the eigenvalues paired by common
   eigenvector and not by rank, and
   reg_a² = (Σᵢ pᵢτᵢ)(Σᵢ pᵢ/τᵢ) with pᵢ = √γᵢ^(a)/F_a and τᵢ = √(γᵢ^(w)/γᵢ^(a)), which is 1 + Var_p(ln τ) to second
   order. In general, for any metric M, reg_a(M)² = tr(PN_a)·tr(P⁻¹N_a), where N_a = Γ_a^½/F_a has trace 1 and P is M
   written in family a's dual coordinates (P = I for M = M_a); hence reg_a(M) ≤ ½(√κ + 1/√κ), κ the condition number
   of P (Kantorovich's inequality).
6. *First order against second order.* Let Σ_a, Σ_b and Σ_R be positive definite and let M(ε) = Σ_ε⁻¹ # Σ_R be the metric
   of the fit Σ_ε = (1 − ε)Σ_a + εΣ_b. Then ω_a(M(ε))² − F_a² = ½ε²·q + o(ε²) and
   ω_b(M_a)² − ω_b(M(ε))² = ε·q + o(ε), with one number q ≥ 0, and q > 0 unless M_a is a multiple of M_b. So mixing a
   small share of family b into the fit costs family a an amount of second order in the share and gains family b
   an amount of first order, unless the two families already share their metric.
7. *Minimax regret.* min over M of max_a reg_a(M)² = max over w in the simplex of F(Σ_a w_aΣ_a/F_a²)². The right side
   is a concave maximisation; the left is the convex programme "minimise t subject to tr(MΣ_a) ≤ t·F_a² for all a
   and tr(M⁻¹Σ_R) ≤ 1". The optimal metric is the geometric-mean metric of the least favourable mixture, and the
   value lies in [1, A].

**Proof.** (1) By B1(a) and scale invariance, F(Σ)² = min{tr(MΣ) : M ≻ 0, tr(M⁻¹Σ_R) = 1} and
F(Σ) = min_M ½[tr(MΣ) + tr(M⁻¹Σ_R)]: minima of linear functions of Σ, so both are concave. Homogeneity is clear. For
subadditivity let Δ₁ ~ N(0, X) and Δ₂ ~ N(0, Y) be independent, Δ = Δ₁ + Δ₂ and r = MΔ with M the optimal map for
X + Y. Then (Δ₁, r) and (Δ₂, r) are pairs with second moments (X, Σ_R) and (Y, Σ_R), so by B1(b)
F(X + Y) = E⟨Δ, r⟩ ≤ F(X) + F(Y). (2) Σ_a w_aω_a(M)² = tr(MΣ_w)·tr(M⁻¹Σ_R), minimised at M_w with value F_w².
(3) Concavity and homogeneity of F² give the first; subadditivity and homogeneity of F the second; Cauchy–Schwarz
the third. For the attainment: if Σ_aΣ_RΣ_b = 0 then B_aB_b = 0, the B_a have orthogonal ranges,
(Σ_a w_aB_a)^½ = Σ_a √w_a·B_a^½ and F_w = Σ_a √w_a·F_a; the Cauchy–Schwarz step is an equality when the √w_a·F_a are
equal. (4) The identity is (2) rearranged; 0 ≤ δ ≤ 1 is (3). (5) F(Σ) = tr(Σ_R^½ΣΣ_R^½)^½ because the two
orderings of the product have the same non-zero eigenvalues. In family a's dual coordinates Σ_a and Σ_R both
become Γ_a^½, so tr(MΣ_a) = tr(PΓ_a^½) and tr(M⁻¹Σ_R) = tr(P⁻¹Γ_a^½); in the eigenbasis of P this is a scalar
Kantorovich inequality for the probabilities (N_a)_jj. The commuting case is the same with P diagonal. (6) The
proof the referees saw was the one line "M_a minimises ω_a²", which gives the second-order cost and nothing about
family b; the extensions referee supplied the rest, and this is its argument. Work on the slice
tr(M⁻¹Σ_R) = 1, on which ω_a² and ω_b² are smooth. By the sum rule (2), M(ε) minimises (1 − ε)ω_a² + εω_b² on the
slice. Let G_b be the gradient of ω_b² at M_a along the slice and H_a the Hessian of ω_a² there. H_a is positive
definite on the slice: in family a's dual coordinates, with P = e^(sH), ω_a²/F_a² = tr(e^(sH)N_a)·tr(e^(−sH)N_a) =
1 + s²[tr(H²N_a) − tr(HN_a)²] + O(s³), a variance under the positive definite N_a, which is zero only for H a
multiple of the identity, the scale direction that the slice removes. Differentiating the first-order condition
(1 − ε)∇ω_a² + ε∇ω_b² = 0 at ε = 0, where ∇ω_a² = 0, gives M′(0) = −H_a⁻¹G_b. Hence
d/dε ω_b(M(ε))² at 0 is −G_bᵀH_a⁻¹G_b = −q, and ω_a(M(ε))² − F_a² = ½ε²·G_bᵀH_a⁻¹G_b + o(ε²). q = 0 only if G_b = 0, that
is if M_a is stationary for ω_b² on the slice. ω_b does not change when M is scaled, so M_a is then stationary on
the whole cone; ω_b² is convex along geodesics (the mean criterion, below), so a stationary point is a minimiser;
and the minimiser of ω_b is M_b up to scale (the equality case of B1(a)). So q = 0 only if M_a is a multiple of
M_b. (7) Normalise tr(M⁻¹Σ_R) ≤ 1, a convex constraint; max_a is the maximum over the simplex of the
bilinear function Σ_a w_a tr(MΣ_a)/F_a²; Sion's minimax theorem applies because the simplex is compact
[cited, standard]; the inner minimum is F(·)² by (1). The bounds are (3) with every F(Σ_a/F_a²) = 1. ∎

**The mean criterion (replaced: the remark the referees saw was wrong in its conclusion).** I wrote that the
mixture's mean width Σ_a w_a ω_a(M) is not convex in M, "so the mean-square criterion and the minimax criterion are
the two that can be solved exactly". The first half is true along straight lines and the conclusion is false. The
extensions referee's correction, which I have checked:

*Every ω_a, and so the mean width, the mean-square width and the worst regret, is convex along the geodesics
t ↦ M₀ #_t M₁ of the positive definite cone. Hence every local minimum of each criterion is global, and each can be
minimised by descent.*

*Proof* (the referee's). For X = M₀ # M₁ the block matrix [[M₀, X], [X, M₁]] is positive semidefinite, so for a
vector u, uᵀXu ≤ √(uᵀM₀u·uᵀM₁u), and for A = Σ_j a_j u_ju_jᵀ ⪰ 0, tr(AX) ≤ √(tr(AM₀)·tr(AM₁)) by Cauchy–Schwarz. The
midpoint of two points of a geodesic is on it, so with continuity t ↦ tr(A(M₀ #_t M₁)) is log-convex. Also
(M₀ #_t M₁)⁻¹ = M₀⁻¹ #_t M₁⁻¹, so t ↦ tr((M₀ #_t M₁)⁻¹Σ_R) is log-convex. A product, a square root, a positive
combination and a maximum of log-convex functions are log-convex, hence convex. ∎

In the commuting case this is the convexity of log-sum-exp in the logarithms of the diagonal weights. So the mean
criterion is no harder than the other two. Its minimiser is close to M_w and is not M_w.

**Check.** `ot_checks.py`: concavity of F and of F² and subadditivity on 400 random triples (margins 7·10⁻⁴,
10⁻³; largest F(X+Y)/(F(X)+F(Y)) = 0.91); minimax on 40 families of three (gap between the two sides 10⁻³, no
random metric better, regrets 1.016 to 1.213); price on 400 random mixtures between 1.0004 and 1.53, and exactly 2
for two orthogonal rank-one families.

**Checks of the repairs** (`check_after_referees.py`, lines 6 to 8).
- Item 3: two rank-one families e₁e₁ᵀ and e₂e₂ᵀ with Σ_R = [[1, c], [c, 1]] have price 2.000, 1.954, 1.800, 1.436 at
  c = 0, 0.3, 0.6, 0.9; with the second family replaced by vvᵀ, v ∝ Σ_R⁻¹e₂ scaled to the same F, the price is 2.000
  at every c, although u·v is then −0.29, −0.51, −0.67. (The referee's figures; reproduced.)
- Item 6: halving ε from 0.2 to 0.0125 divides family a's cost by 2.59, 2.97, 3.33, 3.61 (towards 4) and family
  b's gain by 1.41, 1.58, 1.73, 1.84 (towards 2).
- The mean criterion: on 600 random pairs of metrics the mean width at the geodesic midpoint is never above the
  chord (largest excess −3·10⁻⁴ of the chord), and at the straight midpoint it is above the chord by up to 54%. In
  a diagonal two-family instance the least mean width is 1.301717, the mean-square metric M_w gives 1.301927, and
  Σ_a w_aF_a = 1.132015.
- The referee's own numerics (`referee/ref_ot.py`): 3,000 pairs, six random starts reaching one minimum on 12
  instances, and the minimax saddle to 8·10⁻⁹.

### B4c. Does the theory predict the measured 21 to 33% against 6 to 19%? [computed against measured]

Reordered and renamed after the extensions referee's report: the prediction is the one made from the fits alone
and it leads; the table that uses the test queries' second moments is a consistency check, not a prediction.

`mixture_price.py`, log `mixture_price.log` (not rerun for this revision; the referee compared the script with its
log and with the experiment's files and found every figure below in them). It rebuilds the experiment's set-up:
its fitted Σ_Q for items and text (`work/setup.npz`), each group's residual covariance recomputed from the 200k
vectors as its `measure.py` does, the dual basis for a fit Σ_w = (1 − w)·Σ_item/tr + w·Σ_text/tr with the same
ridge, and the profile weights from the same fit. w = 0 is its bound "a" (data-only) and w = ½ its bound "b2"
(mixed). A level at reach k bounds the coordinates from k on. The second-moment formula for a mean half-width is
√(Σ_{i≥k} Dᵢ²Pᵢ)·√(Σ_{i≥k} λᵢ/Dᵢ²), with P the profile in that basis of a second-moment matrix of query offsets,
summed over groups by size. No score of a query against an item enters.

**The prediction: from the two fits alone.** Each family is evaluated on its own fitted matrix, which is what can
be computed before a test query is seen. Mixed basis over data-only basis, at k = 0 / 32 / 256:

| Family | Predicted from the fits | Measured on held-out queries |
|---|---|---|
| Text | 0.725 / 0.676 / 0.513 | 0.788 / 0.732 / 0.672 |
| Items | 1.054 / 1.099 / 1.257 | 1.055 / 1.084 / 1.195 |

So the prediction has the direction and the size at the first level right for items (a cost of 5.4% against 5.5%)
and overstates both effects with depth: the text gain is predicted at 27 / 32 / 49% and measured at 21 / 27 / 33%;
the item cost is predicted at 5 / 10 / 26% and measured at 6 / 8 / 19%. At k = 0 the regrets against each family's
own optimum F_a, from the fits, are 1.001 (items) and 1.462 (text) for the data-only basis, 1.055 and 1.061 for
the mixed basis, 1.229 and 1.002 for the text-only basis.

**A consistency check: given the test queries' second moments.** Here P is the profile of the second-moment matrix
of the test queries' own offsets (each query projected off each cluster centre of the group, weighted by cluster
size). This is not a prediction: the test queries enter through that matrix, and for the 100 item queries it has
rank at most 100 in 768 dimensions. It checks that second moments carry the whole effect.

| Test queries | Second-moment formula with the test queries' matrix | Measured |
|---|---|---|
| Held-out text (369) | 0.787 / 0.732 / 0.669 | 0.788 / 0.732 / 0.672 |
| In-sample species prompts (361) | 0.991 / 1.011 / 0.986 | 0.991 / 1.011 / 0.988 |
| Items (100) | 1.052 / 1.080 / 1.179 | 1.055 / 1.084 / 1.195 |

Text-fitted diagonal weights in the data-only basis (its bound b1) over data-only: text 0.907 / 0.935 / 0.970 by
the formula against 0.907 / 0.938 / 0.977 measured; items 1.157 / 1.058 / 1.030 against 1.163 / 1.058 / 1.026.

**So.**
- Does theory predict the 21 to 33% and the 6 to 19%? From the fits alone, in direction and roughly in size: it
  overstates the text gain by 6 to 16 points and the item cost by up to 6 points. The gap is between the fitted
  families and the held-out queries (the held-out prompts are not distributed as the training prompts; the fits
  draw clusters over the whole corpus, the measurement is per group), not in the calculus: with the held-out
  queries' own second-moment matrix the formula reproduces the measured ratios to 0.3 points for text and 1.6
  points at worst for items.
- The absolute widths by the formula are 4 to 10% above the measured means (0.310 against 0.297 for text at
  k = 0), as a root mean square should be; the ratio between bases does not see that.
- What this licenses: a change of basis can be costed from Σ_R, the fits and a second-moment matrix of a query
  sample, without scoring a query against the corpus. How well the sample stands for the queries to come is then
  the whole question, and the first table shows the size of that error here.
- The caveat is the experiment's own: the held-out prompts share templates with the training prompts.
- One point the referee left unchecked, which I can settle from the experiment's `prep.py`: the fits are built
  from offsets projected off the unit centre, q − (m̂·q)m̂, and the test profile uses the same projection, so the
  two profiles are of the same kind. They differ in which clusters enter: the fits draw 40 clusters per query by
  size over the whole corpus, and the test matrices use the clusters of each group.

### B4d. What a basis per query family, chosen by the query, would gain [computed; model]

The trade-off along the mixing weight, by the second-moment formula with the test queries' matrices (the
consistency check of B4c, so these are not predictions made before seeing the queries), width over the data-only
basis at k = 0 / 32 / 256:

| Text share w of the fit | Held-out text | In-sample species | Items |
|---|---|---|---|
| 0 (data only) | 1 / 1 / 1 | 1 / 1 / 1 | 1 / 1 / 1 |
| 0.05 | 0.900 / 0.871 / 0.798 | 0.990 / 0.990 / 0.975 | 1.003 / 1.003 / 1.013 |
| 0.10 | 0.868 / 0.830 / 0.756 | 0.986 / 0.988 / 0.970 | 1.007 / 1.009 / 1.029 |
| 0.20 | 0.834 / 0.788 / 0.715 | 0.983 / 0.990 / 0.969 | 1.016 / 1.023 / 1.065 |
| 0.35 | 0.805 / 0.753 / 0.685 | 0.985 / 0.999 / 0.975 | 1.032 / 1.049 / 1.119 |
| 0.50 (as shipped) | 0.787 / 0.732 / 0.669 | 0.991 / 1.011 / 0.986 | 1.052 / 1.080 / 1.179 |
| 0.70 | 0.772 / 0.714 / 0.655 | 1.006 / 1.035 / 1.008 | 1.089 / 1.134 / 1.279 |
| 1 (text only) | 0.767 / 0.705 / 0.651 | 1.064 / 1.122 / 1.104 | 1.229 / 1.402 / 1.719 |

Reading.
- *The shape is item 6 of B4b.* The first 5% of text in the fit gives text 10 to 20% and costs items 0.3 to 1.3%.
  Between w = ½ and w = 1 text gains 2 to 3 points more and items lose 17 to 54.
- *A basis per family.* Items on the data-only basis and held-out text on the text-only basis, against the shipped
  mix: items 5 / 7 / 15% narrower, held-out text 2.5 / 3.7 / 2.7% narrower. Against the best single compromise
  (the weight that equalises the two losses, w ≈ 0.4 at k = 0 and w ≈ 0.3 at k = 256) each family gains about 4%
  at k = 0 and 8% at k = 256. That is the whole prize. Theory sizes it from the two fits: the mean-square price of
  one basis for their equal mixture is F_w²/(½F₁² + ½F₂²) = 1.117, δ = 0.12 on a scale from 0 to 1 and nearly the
  same in all twelve groups (0.110 to 0.123). That is about 5.7% in width, shared as the regrets 1.055 and 1.061
  above.
- *"Text" is not one family.* The in-sample species prompts are best at w ≈ 0.2 and lose 6 to 12% on the text-only
  basis. A basis per family needs the families to be found, not named.
- *The choice needs no classifier.* With bases b = 1, 2 stored, a query computes ‖Δ‖_{M_b} for each and the build
  knows c_b = E‖r‖_{M_b⁻¹} for the group. Choosing the b with the smaller c_b·‖Δ‖_{M_b} is sound, because both
  certificates are, and gives each query the smaller of its two expected widths.
- *The cheaper move.* Keeping one basis and lowering the text share from ½ to 0.2 keeps 78 / 79 / 86% of the text
  gain and cuts the item cost from 5.2 / 8.0 / 17.9% to 1.6 / 2.3 / 6.5%, by the formula with the test queries'
  matrices. From the fits alone, which is the prediction proper, it keeps 78 / 80 / 84% of the text gain and cuts
  the item cost from 5.4 / 9.9 / 25.7% to 1.7 / 3.4 / 10.3% (`mixture_price.log`, the block "from the two fits
  alone"). It stores nothing more. It has not been measured.
- *Which single basis.* The weight could also be chosen by minimising the mixture's mean width directly, which is
  a convex problem along geodesics (B4b, the mean criterion). I did not compute that minimiser on this set-up.
- *Cost of two bases.* A second code for every level that uses the basis, and a second n × n transform pair per
  group in memory, as the experiment's report says.

Limits: the 200k sample, K = 200, one ridge value, an idealised level (exact leading coordinates), test queries
whose templates overlap the training prompts, and a root-mean-square proxy validated on the ratios of B4c only.

## 13. B5. n_eff and rule 1's ρ² [proved, unrefereed]

Rule 1 of `layered.md` 4.2: a level with coding error e, weights D diagonal in the level's coordinates, π_c = E Δ_c²,
v_c = E e_c², a_c = √(π_c v_c), and ρ² = Σ a_c²/(Σ a_c)². Let Σ_E be the covariance of e and write
n_eff(Σ_Q, Σ_E) = F(Σ_Q, Σ_E)²/tr(Σ_QΣ_E) for chapter 0's n_eff with Σ_E in place of Σ_R.

**Statement.** Let the level's coordinates be Δ_c = (AΔ)_c and e_c = (A⁻ᵀe)_c for an invertible A (orthonormal in
`layered.md`; the dual basis is another case), so Δ·e = Σ_c Δ_c e_c. With Δ and e independent and centred:

1. E h² = (Σ_c a_c)² exactly at the weights D_c⁴ = v_c/π_c, and Σ_c a_c ≥ F(Σ_Q, Σ_E), with equality if and only if
   M = Σ_Q⁻¹ # Σ_E is diagonal in these coordinates.
2. E(Δ·e)² = tr(Σ_QΣ_E) = Σᵢ γᵢ(Σ_Q, Σ_E) always. It equals Σ_c a_c² = Σ_c π_c v_c if and only if the off-diagonal
   terms Σ_{c≠c′} (Σ_Q)_{cc′}(Σ_E)_{c′c} sum to zero (covariances taken in these coordinates).
3. **The two are the same quantity, a_c² = γ_c term by term, exactly when both covariances are diagonal in the
   level's coordinates.** For orthonormal coordinates that is: Σ_Q and Σ_E commute and the basis diagonalises both.
   In the dual basis of (Σ_Q, Σ_E) it always holds, with π_c = v_c = √γ_c. (Otherwise the two numbers ρ⁻² and n_eff
   can still coincide by accident, since each is a ratio.)
4. In general the true ratio of the statistical to the certified second moment for the diagonal certificate is
   tr(Σ_QΣ_E)/(Σ a_c)² = (1/n_eff)·(F/Σ a_c)² ≤ 1/n_eff, and rule 1's derived value is that times
   Σ π_c v_c / tr(Σ_QΣ_E).
5. It is chapter 0's n_eff only at a level that has coded nothing (e = r). At any other level it is the same
   functional of the level's error covariance.

**Proof.** (1) E h² = E‖DΔ‖²·E‖D⁻¹e‖² = (Σ D_c²π_c)(Σ v_c/D_c²), which is (Σ a_c)² at the stated weights and is the
minimum over diagonal D; B1(a) gives ≥ F² over all metrics, with equality only at multiples of M. (2) Independence
and zero means. (3) If both are diagonal, γ_c = π_c v_c. Conversely, if M is diagonal with entries d_c², then
Σ_E = MΣ_QM and tr(Σ_QΣ_E) = Σ_{c,c′} (Σ_Q)²_{cc′} d_c² d_{c′}² ≥ Σ_c π_c² d_c⁴ = Σ_c a_c², with equality only for
diagonal Σ_Q, and then Σ_E is diagonal as well. (4) and (5) by substitution. ∎

**Check.** 300 non-commuting pairs in a fixed orthonormal basis: Σ a_c/F ≥ 1.0001 always; one example has
ρ⁻² = 6.24 against n_eff = 3.74. Both diagonal: equal to 6 digits. A non-commuting pair in its dual basis: equal to
6 digits. M diagonal but Σ_Q not: Σ a_c = F to 6 digits and Σ a_c²/tr(Σ_QΣ_E) = 0.63, so ρ⁻² = 3.99 against
n_eff = 2.53.

**The gap between the derived and the measured ρ** (replaced: the sentence the referees saw was wrong).
`layered.md` reports the derived ρ within 2 to 5% of the measured one on two datasets and 15% above on LAION text.
I wrote that by item 4 this gap "is the product of two factors", (F/Σ a_c)² and Σ π_c v_c / tr(Σ_QΣ_E). Both halves
of that were wrong, as the extensions referee shows.
1. By item 4, rule 1's ρ² over the true ratio of second moments is the single factor Σ π_c v_c / tr(Σ_QΣ_E), which
   can be computed at build. The factor (F/Σ a_c)² relates the true ratio to 1/n_eff and is no part of the gap.
2. The measured ρ of `layered.md` is the root mean square over pairs of (score − midpoint)/half-width, a mean of
   per-pair ratios. It is not √(E(Δ·e)²/E h²), and second moments do not determine it: it differs from both by the
   dependence between h and the error inside a pair.

A check with four different numbers (`check_after_referees.py`, line 9; n = 3, Gaussian, independent, 200,000
pairs): rule 1's ρ² = 0.334, the ratio of second moments 0.254, 1/n_eff = 0.348, and the mean over pairs of
(Δ·e)²/h² = 0.299. The referee's own example gives 0.390, 0.397, 0.414 and 0.357.

---

## 14. What failed, what is refuted, what is open

**Refuted or corrected before the referees.**
- *Conjecture A as the target.* It is true (Proposition A) and it is the wrong target: √e·κ_n/n below Theorem 1 at
  high rate and a loss √H where the truth loses H at low rate, because its proof discards the cells' second
  moments. Counter-check, isotropic, one coordinate: Φ(H) = 0.70 at H = 0.1 nats where W(H) = 0.905.
- *"The tail in full, nothing subtracted"* for Φ at high rate: an uncoded coordinate contributes √γᵢ − γᵢ/(2λ),
  not √γᵢ. It is exact for W and V.
- *The head point first* (the brief's route to the head constant): it proves the lossy statement of F2-alt and
  cannot do better, because the chosen head point is an extreme point and nothing averages. Reversed order: F2.
- *Wu–Wang–Flierl's Theorem 1 as a proved converse.* It is not one (section 6, P2).
- *"The minimiser of the mean certificate may differ from the geometric mean"* (the librarian's caution): it does
  not (B2b).
- *V(0) = F* (my own expectation): false, V(0) > F on the measured spectra (0.0760 against 0.0728), and the truth
  at zero rate is infinite.
- *Regret against the test queries' own optimum* in the first run of `mixture_price.py`: an overfit to 100 item
  queries, not an achievable gain. Dropped.

**Wrong in the text the referees saw, and corrected** (details in "Changes after the referees").
- L1 called possibly new: it is published (Bolley–Gentil–Guillin 2018).
- "Their achievability shows F1 is tight": it shows tightness in probability, not in the mean.
- "F3 agrees in form with Ingber–Courtade–Weissman's Theorem 4": in direction only.
- "The lower bounds on J bound the true floor from above": they bound what F3 can prove.
- "Past about 3 bits a bit is better spent on a coordinate": a floor does not show that.
- B4a: "the model has two thirds to three quarters of the gain": two fifths to three quarters.
- B4b item 3: "orthogonal supports" attain the price A: orthogonal in the Σ_R inner product.
- B4b item 6: the first-order half was asserted, not proved.
- B4b closing remark: "the mean criterion cannot be solved exactly": it is convex along geodesics.
- B5 closing sentence: one factor, not two, and the measured ρ is another quantity.
- B4c: the table with the test queries' second moments was headed "predicted".

**Checks that are weaker than they look.**
- In one, two and three dimensions no test presses on the new part of F2 (the tail in full): an uncoded coordinate
  costs about twice its floor to bound there. The tests that press are in high dimension and are all product codes
  or norm shells: my part 3 (1.030, shell masses from a sample), the core referee's (1.00045) and the falsifier's
  (1.0023 against the head set, 1.0005 against the best set, which is Theorem 1). A product code cannot fall below
  F2 if Theorem 1 holds for each factor, so those runs measure tightness.
- Nobody tested a high-dimensional code whose cells couple the head to the tail. The core referee's attack on the
  inequality in which the head and the tail meet (an adversary over densities, 1.0006) is the nearest thing.
- No global optimum over cell shapes was computed by anyone; the referee's planar search is greedy.
- V could not be evaluated on the index behind `quant/REPORT.md`; the transfer from the agent 1 spectra is an
  estimate.
- `mixture_price.py` was not rerun by the referee or for this revision; its log was compared with the script and
  with the experiment's files.

**Open.**
1. A floor for real data. F3 needs the negentropy J of the whitened residual from above; only lower bounds exist.
2. Offsets that are not Gaussian scale mixtures, and offsets that depend on the residual.
3. A statement for a typical query, not the mean (Ingber–Courtade–Weissman's form), at finite n.
4. The floor near zero rate for an anisotropic spectrum: the least Gaussian mean width of Γ^½K among sets of given
   Gaussian measure. F4 answers it for Γ = I.
5. Achievability in the mean. No proof here that some code attains W(H)(1 + o(1)), or V, in the mean, at any rate
   above zero, isotropic or not. The evidence is computed codes within 0.06% to 0.3% at low rate in high dimension,
   D1 (3.6% at 2.95 bits on the measured spectra), and at higher rates the escape code 1.26 to 1.59 above V.
6. Earlier statements of L1: Hosking (2007), Shao–Guo–Alemayehu (2010), Barthe–Kolesnikov (2008) and
   Bakry–Bolley–Gentil (2012) were not opened by anyone here. The journal versions of the three papers read from
   arXiv, Villani, Rockafellar, Pisier and Ordentlich–Polyanskiy are unchecked.
7. Section 6's other conjectures are untouched: "Gaussian residuals are the worst case" (F3 gives the bound's
   side only), √k for a class unknown at build, the tail without the factor 2.
8. B4d's cheaper move (text share 0.2 in the fit) has not been measured, and the minimiser of the mean criterion
   has not been computed on the experiment's set-up.
9. Head sets that are not top-k sets (a band of nearly equal eigenvalues) were not searched on the measured
   spectra.

## 15. Files

All in `notes/program/floors/`.

| File | What it is |
|---|---|
| `NOTEBOOK.md` | the dated log |
| `CLAIMS.md` | this file, revised after the referees |
| `CLAIMS_before_referees.md` | the text the referees saw |
| `referee/` | the two referee reports and their scripts (not mine) |
| `floors_lib.py` | Theorems 1 to 3, Φ, W and V as functions of a spectrum and a rate |
| `check_lemma1d.py`, `.log` | L1 on 40,003 laws |
| `check_chain1d.py`, `.log` | the inner inequality of F1 on interval partitions |
| `check_schemes.py`, `check_schemes_part1.log`, `_part2.log`, `_part3.log` | F1 and F2 against explicit codes |
| `check_f4.py`, `.log` | F4 |
| `floors_eval.py`, `.log` | section 7 |
| `design_lowrate.py`, `.log` | D1 |
| `ot_checks.py`, `.log` | B1 to B5 |
| `mixture_price.py`, `.log` | B4a (model of block norms), B4c, B4d; not rerun for the revision |
| `check_after_referees.py`, `.log` | the checks of the repairs (core 5, 2 seconds, 89 MB resident) |
| `work_sigmaR.npz` | cache of the twelve group residual covariances for `mixture_price.py` (28 MB, float32) |

---

## Changes after the referees

One round, 21 November 2026. Sources: the core referee (`referee/referee-core.md`), the extensions referee
(`referee/referee-extensions.md`), the librarian (`../library/floors-novelty.md`) and the falsifier
(`../stage0-floor-check/RESULTS.md`), all read in full. The text they saw is `CLAIMS_before_referees.md`. No tag was
raised. Two tags were changed because a result is published (L1, B2b). Checks of the repairs are in
`check_after_referees.py` (log `check_after_referees.log`; line numbers below are the numbered lines of that log).

**Nothing of substance is disputed.** Every point of both referees and the librarian is accepted and repaired.
Three clarifications are recorded under the claims they belong to (B4b item 3, B4c, L1).

### New

- **"For the notes"**, at the top: the setting with every hypothesis, Lemma 1 as a cited lemma with its proof,
  Lemmas 2 to 5, F1, F2 and F2a with full proofs, the relation to Theorems 1 to 3, the prior statements of the
  formula, and the checks. It writes the Gaussian law as N_d, because γ with a subscript is an eigenvalue in
  chapter 0, and the support function as h_{K′_c}, as chapter 0 does.

### Core referee (sections 0 to 4): no error found in any claim; all points accepted

| Claim | Point | What was done | Check |
|---|---|---|---|
| Setting | h must be measurable in Δ | stated in section 0 and N.0 | none needed |
| Setting | I(z; c) < ∞ in the averaging steps | stated in section 0; A(c), F1 and F2 now say "let I(z; c) < ∞" and give the case I = ∞ as E h ≥ 0 | none needed |
| Setting | the certificate depends on the item only through c; a shared seed | stated; the seed is handled by conditioning and convexity, in a remark after F2 and in N.7 | none needed |
| L1 | not new if the citation is right | tag changed to [cited; proof kept]; the prior-art paragraph replaced (see the librarian) | the reduction from the quoted theorem, by hand |
| L4 | the exact form in item 4 needs γ₁ > γ₂ | stated | referee's run |
| L4 | Theorem 3 needs the sum bound, not L4.5's maximum | item 6 added and proved | line 1: V_S − (head + W_T) ≥ 0 on 400 spectra |
| A(a) | "k decreases on (0, 1]" does not cover Cᵢ > 1 | "k ≥ 0 on (0, ∞), with minimum 0 at 1" added; KL < ∞ now said to supply L2's hypotheses | line 3: the cell {|z| ≥ 2} has C = 2.096 |
| F1 | tightness remark read as if the coupling alone cost √e | reworded: √e = √2 (coupling) × √(e/2) (L1 for a uniform law) | `check_chain1d.log`, 1.414 |
| F2 | k ≥ 1; the complement may be empty | stated in section 0, F2 and N.7 | none needed |
| F2a | eigenvalues equal to θ go to the tail; ties | stated; the proof now derives W_T′(0⁺) with a repeated eigenvalue and treats k = n | referee's brute force, 5·10⁻¹⁶ |
| F2a | at a tie it is valid but not the best head | said, with the referee's example | line 2: 3.649 against 3.922 and 4.091 |
| F1, F2 | do Theorems 1 to 3 follow | new paragraph in section 4 and in N.7: Corollary 1 and Theorems 2 and 3 follow; Theorem 1 in general does not; F2 uses Theorem 1's step 3 | line 1: W ≥ Theorem 2 and the sum bound ≥ Theorem 3 on 400 spectra |

### Extensions referee: no fatal error; every numbered repair made

| Claim | Point | What was done | Check |
|---|---|---|---|
| F3 | a third use of the Gaussian law (ν_c ≤ N/P_c) | proof rewritten with the referee's repair | none needed |
| F3 | the proof gives I(z; c) + J, stronger than stated | statement strengthened to V_S(I(z; c) + J) | line 4: equality for a uniform law on a uniform grid |
| F3 | "agrees in form with" Theorem 4 of Ingber–Courtade–Weissman | "in direction only" | none needed |
| F3, section 7 | lower bounds on J "bound the true floor from above" | corrected: they bound what F3 can prove | none needed |
| F4 | "deterministic encoder" not needed | dropped; the proof's first line now covers a randomised encoder | none needed |
| F4 | "a few tenths of a nat" is false at n = 1, 2 | said | `check_f4.log` |
| F5 | two hypotheses unstated; the grouping sentence | restated: common spectrum and J, Δ independent of every item; the certificate may read every message; the grouping statement written out for the spectrum of x about the origin, and the loose reading disowned | none needed |
| F6 | which covariance | I(z; c) in the statement; the line on E t/√(E t²) added | line 10 |
| D1 | Σ_Q, Σ_R positive definite | stated | none needed |
| D1 | "a 3-bit message" | "a message of entropy at most 2.95 bits" | none needed |
| D1 | "a bit is better spent on a coordinate" | withdrawn as a consequence of the theorem; kept as what the measured codes suggest | none needed |
| B1 | attribution | the referee's reading of the source recorded; tag unchanged | none needed |
| B2a | finite second moments | stated | none needed |
| B3e | "no bound below F" not shown | the Hölder argument added | line 5: gain ≥ 0.98·F at σ² = 9 |
| B4a.2 | infimum when a norm vanishes; "least such bound" | said | none needed |
| B4a.4 | "two thirds to three quarters" | corrected to 0.38, 0.54, 0.67, 0.77 | arithmetic from the two files |
| B4a.5 | "three bits each" | marked as a low estimate | none needed |
| B4b.3 | attainment condition | "orthogonal in the Σ_R inner product, and equal w_aF_a²", with the proof | line 6 |
| B4b.5 | pairing of eigenvalues | "by common eigenvector" | none needed |
| B4b.6 | the first-order half not proved | restated with one number q for both orders, and proved | line 7: ratios tend to 4 and 2 |
| B4b | closing remark wrong | replaced by geodesic convexity, with the referee's proof | line 8 |
| B4c | the first table is not a prediction | reordered: the prediction from the fits alone leads; the other table is headed as a consistency check; the verdict reworded | none (no rerun) |
| B5 | closing sentence wrong | replaced | line 9: four different numbers |
| P1 | "tight" | in probability, not in the mean (sections 3 and 6) | the referees' computed codes |
| P2 | the false equation is in the proof of Lemma 1 | said | none needed |

**Clarification on B4b item 3.** The referee's counter-example reports the price 2.000 for u = e₁ and v = Σ_R⁻¹e₂.
That holds after v is scaled so that the two families have the same F; unscaled I get 1.999, 1.976 and 1.733 at
c = 0.3, 0.6, 0.9, because w_aF_a² are then unequal and the last inequality of item 3 is strict. The repaired
statement keeps both conditions. This does not affect the referee's point.

**Clarification on B4c.** The referee left unchecked whether the fitted matrices and the test matrices are built
from the same kind of offset. From the experiment's `prep.py` they are (both project the query off the centre's
direction). They differ in the clusters used (the fits draw clusters over the whole corpus, the test matrices use
each group's), which I now say.

### Librarian: accepted

| Claim | Point | What was done |
|---|---|---|
| L1 | published: Bolley–Gentil–Guillin 2018, Theorem 2.1 at n = 1, same proof | tag [cited; proof kept]; citation in section 1, P5 and N.1 |
| L2 | a known consequence, stronger than the published n-dimensional form | "What L2 adds" in section 1 and N.2: one correlation per coordinate; it implies the published form; tag unchanged |
| F1 | the formula is not new; the theorem is | worded as the first proof of the converse at finite n, for covariances that need not commute, in the mean-width form, as far as the search goes; the formula is not claimed |
| F1 | cite Steiner–Dempfle–Ingber–Weissman and Balasubramanian–Bhandari–Kasiviswanathan | P6 and P7 in section 6; both in N.7's paragraph on prior statements |
| B2b | the classical M·M* ≥ 1 after a change of variables | tag changed to [cited] plus the remark that is ours; Aubrun–Szarek–Ye cited |

**Clarification on L1.** The tag [cited] rests on the librarian's reading of the source. I did not open the paper.
I checked only that the theorem as the librarian quotes it reduces to L1 at n = 1.

### Falsifier (`../stage0-floor-check/RESULTS.md`): nothing refuted

Its results are now cited in the checks of L1, A, F1 and F2 and in section 7. Its closest approaches: 1.0026 to W
and 1.0005 to the best V_S (which there is Theorem 1), 1.0023 to V_S with S the head. Its unfinished runs are
listed where they bear (Proposition A's cell families; F2 at m = 2 and 3 head coordinates in high dimension).

### Not changed

L3, L5 (a second proof noted), F2-alt (notation only), B3a to B3d and B3f, B4b items 1, 2, 4, 7, B5 items 1 to 5,
the tables of sections 7 and 8, and every script and log that the referees saw.
