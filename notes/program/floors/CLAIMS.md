# Floors track: claims

Every claim of the floors track, with its tag (tags of `notes/README.md`), its proof or the precise gap, and its
numerical checks. Nothing here has been refereed: a complete proof is **[proved, unrefereed]**. The log of how each was
reached, including what failed, is `NOTEBOOK.md`. Scripts are in this directory and were run as
`nice -n 10 taskset -c 6 env OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 <script>`.

Summary table (details below).

| Claim | Statement in one line | Tag |
|---|---|---|
| L1 | h(μ) ≤ ln(√(2πe)·C(μ)) for a law on ℝ, C the Gaussian correlation of its quantile function | proved, unrefereed |
| L2 | h(ν) ≤ Σᵢ ln(√(2πe)·Cᵢ(ν)) for a law on ℝⁿ under the triangular coupling | proved, unrefereed |
| L3, L4 | the water-filling programme; convexity and bounds for Φ, W, V | proved, unrefereed |
| L5 | κ_k ≥ k for every k | proved, unrefereed |
| A | Conjecture A: E h ≥ Φ(H(c)), and cell by cell E_g h ≥ Φ(ln 1/P_c) | proved, unrefereed |
| F1 | E h ≥ W(H(c)) = Σᵢ √min(γᵢ, θ): the full-floor conjecture of section 6 | proved, unrefereed |
| F2 | E h ≥ V_S(H(c)) for every head set S; with the water-filling head, E h ≥ κ_k√θ + Σ_{i>k} √γᵢ | proved, unrefereed |
| F3 | a residual with negentropy J has the Gaussian floors at rate H + J | proved, unrefereed |
| F4 | isotropic spectrum: E h ≥ √γ·E‖g_n‖·ρ_n(e^(−H)), which is infinite at H = 0 | proved, unrefereed |
| F5 | shared information and groupings enter F1 to F3 as they enter Theorem 1 | proved, unrefereed |
| F6 | offsets that are scale mixtures of Gaussians: the floors times E[t] | proved, unrefereed |
| F2-alt | the brief's order (head point first): what it proves and what stops it | sketch: gaps named |
| P | what Ingber–Courtade–Weissman and Wu–Wang–Flierl prove | cited, sources read |
| D1 | the norm-only dual-basis certificate against the floor at its own entropy | proved, unrefereed; numbers computed |
| E | every floor on the measured spectra at 0 to 96 bytes | computed |
| B1 | Proposition 3 as Kantorovich duality | cited (Bhatia–Jain–Lim 2019), with a two-line proof |
| B2 | no norm beats the M-ellipsoid, in second moments and in the mean under independence | proved, unrefereed |
| B3 | one-scalar certificates φ + φ*; scale mixtures | proved, unrefereed (duality cited) |
| B4 | several scalars; one basis for a mixture of query families | proved, unrefereed; prediction against measurement computed |
| B5 | n_eff against rule 1's ρ² | proved, unrefereed |

---

## 0. Setting and local symbols

The setting is section 0 of `proofcheck/CONSOLIDATED_PROOFS.md`, with these additions.

- **Eigen-coordinates.** B = Σ_Q^½ Σ_R Σ_Q^½ = V Γ Vᵀ, Γ = diag(γ₁ ≥ … ≥ γ_m > 0), m the rank. For i ≤ m put
  zᵢ = γᵢ^(−½) vᵢᵀ Σ_Q^½ r, and write g for Vᵀ times the standard Gaussian of Δ = Σ_Q^½ g (again standard Gaussian).
  Then Δ·r = Σ_{i≤m} √γᵢ gᵢ zᵢ almost surely, z is a function of r, and z is independent of g. Under the Gaussian
  model r ~ N(0, Σ_R), z ~ N(0, I_m). Σ_Q and Σ_R need not commute and need not be invertible. From here on n means m.
- **Message.** c takes countably many values. It may be a randomised function of r, provided it is independent of Δ
  given r. P_c = Pr[c = c], ν_c is the law of z given c, I(z; c) the mutual information in nats. Always
  I(z; c) ≤ H(c), with equality for a deterministic encoder when Σ_Q^½ r determines the message.
- **Soundness.** For every (c, Δ) the interval of half-width h(c, Δ) contains Δ·r for ν_c-almost every z. This is
  weaker than soundness on all of K_c, so a floor proved under it holds under the consolidated proofs' definition.
- **Cell width.** s_c(y) = sup of Σᵢ √γᵢ yᵢ zᵢ over z in the closed support of ν_c. A linear function's essential
  supremum under ν_c is its supremum over the support, so soundness gives 2h(c, Δ) ≥ s_c(g) + s_c(−g), and by the
  symmetry of g

      E_g h(c, ·) ≥ E_g s_c(g).                                                             (0.1)

  This is step 1 of Theorem 1. s_c is a supremum of linear functions, hence Borel.
- **Correlation of a law on ℝ.** For a law μ on ℝ with quantile function Q_μ, T_μ = Q_μ∘Φ is the non-decreasing map
  that sends N(0, 1) to μ, and C(μ) = E[g·T_μ(g)] = ∫₀¹ Q_μ(u) Φ⁻¹(u) du. m₂(μ) is the second moment.
- **Triangular coupling.** For a law ν on ℝⁿ with a density and an order of the coordinates, T^ν is the
  Knothe–Rosenblatt map from γₙ to ν: T^νᵢ(g) = T_μ(gᵢ) with μ the conditional law of zᵢ given
  z_<i = T^ν_<i(g_<i). Cᵢ(ν) = E[gᵢ·T^νᵢ(g)].
- **The three functions of a rate H ≥ 0 (nats).** k(s) = ½(s² − 1) − ln s.
  - Φ(H) = min{Σᵢ √γᵢ sᵢ : sᵢ ∈ (0, 1], Σᵢ k(sᵢ) ≤ H}.
  - W(H) = min{Σᵢ √γᵢ sᵢ : sᵢ ∈ (0, 1], Σᵢ ln(1/sᵢ) ≤ H}. For a set T of indices W_T is the same over T only.
  - For a set S of k indices, V_S(H) = min over 0 ≤ u ≤ H of κ_k·(Π_{i∈S} γᵢ)^(1/2k)·e^(−(H−u)/k) + W_{S^c}(u).
    V_∅ = W.
- F = Σᵢ √γᵢ, n_eff = F²/Σᵢ γᵢ, as in chapter 0. Entropies h(·) are differential, in nats, with values in [−∞, ∞).

---

## 1. Lemmas

### L1. Entropy against the Gaussian correlation of the quantile function [proved, unrefereed]

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
(3.7·10⁻³) and above zero.

**Prior art.** The librarian did not find the inequality. The identity h = ∫₀¹ ln Q′(u) du is classical, and
maximum entropy under linear constraints on the quantile function is treated by Hosking (2007, L-moments)
[cited, unchecked]; the statement may be there in another form and should be looked for before it is called new.

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
read this as the chain rule for KL(ν ‖ γₙ), which always holds, together with
KL(· ‖ γ) = −h + ½ ln 2π per coordinate + half the second moment.) Apply L1 to each conditional law and integrate:

    h(ν) ≤ Σᵢ E[ln(√(2πe)·C(ν_{i|<i}(· | T^ν_<i(g_<i))))] ≤ Σᵢ ln(√(2πe)·E[C(ν_{i|<i})]) = Σᵢ ln(√(2πe)·Cᵢ(ν)).

The second step is Jensen for the logarithm, and the last is E[gᵢ T^νᵢ(g) | g_<i] = C(ν_{i|<i}). Cᵢ ≤ √(E zᵢ²) is
Cauchy–Schwarz. ∎

This is where the brief's worry about step 4 goes: the conditional entropies are random, and only their mean is
controlled, but the logarithm's concavity is all that is used. The convexity of s(·) is not needed.

### L3. The water-filling programme [proved, unrefereed]

**Statement.** Let aᵢ > 0 and H ≥ 0, and let λ ∈ (0, max aᵢ] solve Σᵢ ln⁺(aᵢ/λ) = H. Then
min{Σ aᵢ sᵢ : sᵢ ∈ (0, 1], Σ ln(1/sᵢ) ≤ H} = Σᵢ min(aᵢ, λ), attained at sᵢ* = min(1, λ/aᵢ).
With aᵢ = √γᵢ and θ = λ²: W(H) = Σᵢ √min(γᵢ, θ), where Σᵢ ½ ln⁺(γᵢ/θ) = H.

**Proof.** λ exists and is unique for H > 0 because λ ↦ Σ ln⁺(aᵢ/λ) is continuous and strictly decreasing on
(0, max aᵢ] from ∞ to 0; for H = 0 take λ = max aᵢ. Let L(s) = Σ aᵢ sᵢ − λ Σ ln sᵢ. Coordinate by coordinate L is
convex on (0, 1] with derivative aᵢ − λ/sᵢ, so it is minimised at sᵢ*. For feasible s, Σ ln sᵢ + H ≥ 0, so
Σ aᵢ sᵢ ≥ L(s) − λH ≥ L(s*) − λH = Σ aᵢ sᵢ* + λ(Σ ln(1/sᵢ*) − H) = Σ aᵢ sᵢ*. ∎

### L4. Properties of Φ, W and V_S [proved, unrefereed]

1. Φ, W and V_S are convex and non-increasing in H, and Φ(0) = W(0) = F.
2. W(H) ≥ Φ(H) for every H.
3. Φ(H) ≥ F − √(H·Σᵢ γᵢ) = F·(1 − √(H/n_eff)).
4. W(H) ≥ F − min(√γ₁·H, √(H·Σᵢ γᵢ)). For H ≤ ½ ln(γ₁/γ₂), W(H) = F − √γ₁(1 − e^(−H)) exactly.
5. V_S(H) ≥ max(W_{S^c}(H), κ_k (Π_S γᵢ)^(1/2k) e^(−H/k)). In particular sup_S V_S ≥ max(W, Theorem 1's bound).

**Proof.** (1) Φ and W are value functions of programmes with a linear objective and a convex constraint (k and
−ln are convex), so a convex combination of minimisers at H₁ and H₂ is feasible at the combination of the rates.
V_S is the infimal convolution of two convex non-increasing functions. Minima are attained: the feasible sets are
compact because k(s) and ln(1/s) tend to infinity as s → 0. (2) k(s) ≤ ln(1/s) on (0, 1], so W minimises over a
smaller set. (3) k(1) = k′(1) = 0 and k″ = 1 + s⁻² ≥ 2, so k(s) ≥ (1 − s)². A feasible s has Σ(1 − sᵢ)² ≤ H, and
Σ √γᵢ(1 − sᵢ) ≤ √(Σγᵢ)·√H by Cauchy–Schwarz. (4) With a = √γᵢ > λ: a − λ ≤ a ln(a/λ), which sums to √γ₁·H; and
((a − λ)/a)² ≤ 1 − λ/a ≤ ln(a/λ), so Σ(a − λ)⁺ ≤ √(Σγᵢ)·√H by Cauchy–Schwarz. The exact form is L3 with one coded
coordinate. (5) Bound the two terms of the minimand separately: the head term is least at u = 0 and the tail term
at u = H, and both are non-negative. ∎

### L5. κ_k ≥ k for every k [proved, unrefereed]

The consolidated proofs have this for k ≤ 200,000 by computation.

**Proof.** Let ν be uniform on the ball of radius ρ in ℝᵏ. T^ν(g) lies in the ball, so ρ‖g‖ ≥ g·T^ν(g) and
ρ·E‖g_k‖ ≥ Σᵢ Cᵢ(ν). By L2, Σᵢ ln(√(2πe)Cᵢ) ≥ h(ν) = ln(ω_k ρᵏ), and by the inequality of the means
Σ Cᵢ ≥ k(Π Cᵢ)^(1/k) ≥ k·ω_k^(1/k)·ρ/√(2πe). So E‖g_k‖·√(2πe) ≥ k·ω_k^(1/k), which is κ_k ≥ k. ∎

---

## 2. Conjecture A [proved, unrefereed]

**Proposition A.** Under the Gaussian model, for any encoder and any sound certificate:

(a) for every law ν with a density and KL(ν ‖ γₙ) < ∞, Σᵢ √γᵢ Cᵢ(ν) ≥ Φ(KL(ν ‖ γₙ));
(b) for every message value, E_g h(c, ·) ≥ Φ(KL(ν_c ‖ γₙ)) ≥ Φ(ln(1/P_c));
(c) E h ≥ Φ(I(z; c)) ≥ Φ(H(c)).

**Proof.** (a) Apply the second form of L1 to each conditional law, integrate and use Jensen as in L2:
KL(ν ‖ γₙ) ≥ Σᵢ [½ E_ν zᵢ² − ½ − ln Cᵢ(ν)] ≥ Σᵢ k(Cᵢ(ν)), using E_ν zᵢ² ≥ Cᵢ². Put sᵢ = min(Cᵢ, 1); k decreases
on (0, 1] and k(1) = 0, so Σ k(sᵢ) ≤ KL and s is feasible for Φ(KL). (b) T^{ν_c}(g) lies in the support of ν_c
almost surely, so s_c(g) ≥ Σᵢ √γᵢ gᵢ T^{ν_c}ᵢ(g); take expectations and use (0.1) and (a). ν_c ≤ γₙ/P_c gives
KL(ν_c ‖ γₙ) ≤ ln(1/P_c). (c) Average (b) over c; Φ is convex (L4), Σ_c P_c KL(ν_c ‖ γₙ) = I(z; c), and Φ is
non-increasing. ∎

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
cells.

**Checks.** `floors_eval.py` evaluates Φ beside W (section 7). Every code tested against W below also passes Φ ≤ W.

---

## 3. The water-filling floor [proved, unrefereed]

**Theorem F1.** Under the Gaussian model r ~ N(0, Σ_R), Δ = Σ_Q^½ g independent of r, for any encoder and any sound
certificate,

    E h ≥ W(I(z; c)) ≥ W(H(c)) = Σᵢ √min(γᵢ, θ),      Σᵢ ½ ln⁺(γᵢ/θ) = H(c).

More precisely there are numbers C̄ᵢ ∈ (0, 1] with E h ≥ Σᵢ √γᵢ C̄ᵢ and Σᵢ ln(1/C̄ᵢ) ≤ I(z; c).

With H(c) ≤ B ln 2 this is the "full floor" conjecture of section 6 of the consolidated proofs, for every n and for
Σ_Q and Σ_R that need not commute.

**Proof.**
1. *Width.* (0.1): E_g h(c, ·) ≥ E_g s_c(g).
2. *Coupling.* ν_c ≤ γₙ/P_c, so ν_c has a density, finite second moments and h(ν_c) ≥ ln(P_c(2π)^(n/2)) > −∞. Let
   T^c = T^{ν_c} (L2). T^c(g) has law ν_c, so it lies in the support of ν_c almost surely and
   s_c(g) ≥ Σᵢ √γᵢ gᵢ T^cᵢ(g). Hence E_g s_c(g) ≥ Σᵢ √γᵢ Cᵢ(ν_c). The right side is integrable: E|gᵢ T^cᵢ| ≤
   √(E_{ν_c} zᵢ²) ≤ P_c^(−½).
3. *Entropy.* L2: h(ν_c) ≤ Σᵢ ln(√(2πe)·Cᵢ(ν_c)).
4. *Averaging.* Σ_c P_c h(ν_c) = h(z | c) = h(z) − I(z; c) = (n/2) ln(2πe) − I(z; c). So
   Σᵢ Σ_c P_c ln Cᵢ(ν_c) ≥ −I(z; c). Put C̄ᵢ = Σ_c P_c Cᵢ(ν_c). By Jensen Σᵢ ln C̄ᵢ ≥ −I(z; c).
5. *C̄ᵢ ≤ 1.* C̄ᵢ ≤ Σ_c P_c √(E_{ν_c} zᵢ²) ≤ √(Σ_c P_c E_{ν_c} zᵢ²) = √(E zᵢ²) = 1.
6. E h = Σ_c P_c E_g h(c, ·) ≥ Σᵢ √γᵢ C̄ᵢ ≥ W(I(z; c)) by the definition of W; W is non-increasing and
   I(z; c) ≤ H(c). The closed form is L3. ∎

Where the cells' second moments enter: step 4 uses h(z | c) = h(z) − I, which is the statement that the cell laws
mix back to N(0, I), and step 5 uses it again. Per cell the same argument gives
Σᵢ ln(1/Cᵢ(ν_c)) ≤ KL(ν_c ‖ γₙ) + ½ Σᵢ (1 − E_{ν_c} zᵢ²), in which the last term has either sign.

**Remarks.**
- *Against Theorem 1.* The head constant is 1 where Theorem 1 has κ_k/k. F2 restores κ_k.
- *Against Theorems 2 and 3.* No √(2H) term, no added bit, no 1/√e, and the loss at small rate is linear in H
  (L4.4) where Theorem 2 and Proposition A lose √H.
- *Tightness.* Each inequality is an equality when every ν_c is N(m_c, diag(sᵢ²)) with the same sᵢ, the backward
  test channel of rate–distortion theory. No cell has that law exactly, and the slack that remains is the gap
  between the supremum over a cell and the correlation with its law (a factor κ_k/k for ball-shaped cells in k
  dimensions, √e in one). Asymptotic tightness in the isotropic case is the achievability half of
  Ingber–Courtade–Weissman (section 6) [cited].
- *What is not claimed.* Nothing for real data: the residuals are not Gaussian (F3 charges that as rate, and J is
  not known), the offsets are not Gaussian or independent of the residual, and shared bits must be counted (F5).

**Checks.** `check_schemes.py` (logs `check_schemes_part1.log`, `check_schemes_part2.log`,
`check_schemes_part3.log`). The exact range of a code is its smallest sound certificate, so a code whose exact mean width fell below
W would refute F1.
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
  near-equality is a partition with one cell of almost all the mass) and E h / C̄ ≥ 1.414 (the coupling step; √2 is
  the loss of a one-dimensional cell against its own law).

The parts differ in how hard they press. In one and two dimensions a cell cannot bound an uncoded coordinate
cheaply, so the ratios stay large; part 3, where the tail has many effective dimensions, is the test of "the tail
in full", and it comes within 8%.

---

## 4. The combined floor: Theorem 1's constant on the head, the tail in full [proved, unrefereed]

**Theorem F2.** Under the Gaussian model, for any encoder, any sound certificate and any set S of k eigen-indices,

    E h ≥ V_S(I(z; c)) ≥ V_S(H(c)) = min over 0 ≤ u ≤ H of { κ_k·(Π_{i∈S} γᵢ)^(1/2k)·e^(−(H−u)/k) + W_{S^c}(u) }.

S = ∅ is F1. S = all indices is Corollary 1 of Theorem 1 at k = n, and V_S always dominates Corollary 1's term for
the set S (L4.5).

**Corollary F2a.** Let H > 0, let θ solve Σᵢ ½ ln⁺(γᵢ/θ) = H and k = #{i : γᵢ > θ}. Then

    E h ≥ κ_k·√θ + Σ_{i>k} √γᵢ.

This is the section 6 conjecture with κ_k in place of k on the head: larger by (κ_k − k)√θ.

**Proof of F2.** Write T = S^c. Order the coordinates with T first.
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

**Proof of F2a.** Take S = {1, …, k}. Then (Π_S γᵢ)^(1/2k) e^(−H/k) = √θ. The function
f(u) = κ_k (Π_S γᵢ)^(1/2k) e^(−(H−u)/k) + W_T(u) is convex on [0, H]. Its right derivative at 0 is
(κ_k/k)√θ − √γ_{k+1}, because W_T′(0⁺) = −√γ_{k+1} (L3). Now γ_{k+1} ≤ θ and κ_k ≥ k (L5), so the derivative is
non-negative and the minimum is at u = 0: V_S(H) = κ_k√θ + Σ_{i>k} √γᵢ. ∎

**Why this order.** The brief proposed the head first; see F2-alt below for what that proves. Taking the tail first
works because the tail point then has exactly the cell's tail law (so entropies and second moments average), and
the head supremum is over a whole fibre at a point that is independent of the head Gaussian (so no point has to be
selected). Theorem 3 lost its constant through the mass of the fibre over a selected head point; here no fibre mass
appears.

**The best head set.** Any S is allowed. On the measured spectra the best top-k set is a little larger than the
water-filling head (49 against 44 coordinates at 48 bits) and the gain over W is 1 to 2% (section 7). The gain is
about (κ_k − k)√θ, and κ_k − k ≈ ½ ln(πk): a few coordinates' worth, whatever the dimension. A head made of a band
of nearly equal eigenvalues is also allowed; I did not search over bands.

**Checks.** `check_schemes.py`, the same codes as for F1, against max over S of V_S.
- Part 1, every subset S of n ≤ 5 coordinates: smallest ratio 1.000040 (one coordinate, step 0.05σ: Theorem 1's
  known tight case); smallest with n ≥ 2 is 1.130.
- Part 2, the four subsets of two coordinates: smallest ratio 1.135 (a sheared lattice, S = both). With ε = 0.05
  the binding set is S = {1} and the smallest ratio is 1.223 (strips), where Theorem 1 alone gives 1.346: the new
  floor is closer to the code by a tenth.
- Part 3, S = top-k for every k: smallest ratio 1.030 (n = 48, no head grid, 1.76 nats in the shells; best k = 21).
- No code fell below. `floors_eval.py` asserts the same for the two codes of `agent1/theory.log`.

---

## 5. Extensions

### F3. Residuals that are not Gaussian [proved, unrefereed]

**Statement.** Drop r ~ N(0, Σ_R). Let Σ_R be the second-moment matrix of r, and assume z (section 0) has a
density, finite entropy and E zzᵀ = I. Let J = (n/2) ln(2πe) − h(z) = KL(law of z ‖ N(0, I)) ≥ 0, the negentropy in
nats. Keep Δ Gaussian and independent of r. Then

    E h ≥ V_S(H(c) + J) for every S,     in particular E h ≥ W(H(c) + J).

**Proof.** The proofs of F1 and F2 use the Gaussian law of z in two places only: h(z) = (n/2) ln(2πe), which
becomes (n/2) ln(2πe) − J, and E zᵢ² = 1, which is kept. In step 6 of F2, ā ≥ (k/2) ln(2πe) − (I + J − u), and
u ≤ I + J because h(z_S | z_T, c) ≤ h(z_S) ≤ (k/2) ln(2πe) (maximum entropy for unit second moments). ∎

So the negentropy of the residual is charged as rate, as in the remark after Theorem 1, where the factor is
exp(−J/k). This is a lower bound only: it does not say the true optimum is smaller for a non-Gaussian residual
(the section 6 conjecture "Gaussian residuals are the worst case" stays open). It agrees in form with two published
statements: Ingber–Courtade–Weissman's Theorem 4 [cited, read] and Ordentlich–Polyanskiy's
Γ(R + D(P‖N(0,1))) ≤ D^IP(R, P) [cited, unchecked: from the librarian's transcription].

For real data J is not known. `entropy/REPORT.md` gives lower bounds on J (23 to 80 bits per item on BioCLIP-2),
which bound the true floor from above, not below. Section 7 shows what those values would do.

### F4. An isotropic spectrum at very low rate [proved, unrefereed]

**Statement.** Gaussian model, γ₁ = … = γₙ = γ, deterministic encoder. Let ρₙ(P) be the P-quantile of the χₙ law.
Then E_g h(c, ·) ≥ √γ·E‖gₙ‖·ρₙ(P_c) for every cell, and

    E h ≥ √γ·E‖gₙ‖·ρₙ(e^(−H(c))).

The right side tends to infinity as H → 0, as the truth does (a cell of mass near 1 is nearly unbounded).

**Proof.** Urysohn (step 3 of Theorem 1): E_g s_c(g) ≥ √γ·E‖gₙ‖·(vol(K_c)/ωₙ)^(1/n). The Gaussian density
decreases with the radius, so among sets of a given volume the centred ball has the largest Gaussian measure; hence
vol(K_c) ≥ vol of the centred ball of measure P_c, whose radius is ρₙ(P_c). For the average, write P = e^(−u). Then
dρₙ/du = −F(ρ)/f(ρ) with F and f the distribution function and density of χₙ, and this is non-decreasing in u
exactly when F/f is non-decreasing in ρ, that is when F is log-concave. The χₙ density is log-concave for n ≥ 1, so
F is (Prékopa). Jensen in u gives Σ_c P_c ρₙ(P_c) ≥ ρₙ(e^(−H)). ∎

It beats Theorem 1 only below a few tenths of a nat [computed, `check_f4.py`, log `check_f4.log`]: at n = 76 it is
1.186n, 1.100n and 1.014n at 0.01, 0.1 and 0.5 nats, against Theorem 1's 1.033n, 1.032n and 1.027n; at n = 768,
1.059n and 1.033n at 0.01 and 0.1 nats against 1.005n. The script also checks the convexity of u ↦ ρₙ(e^(−u)) on a
grid for n = 1 to 768 and the cell statement on centred and off-centre balls. At high rate F4 is 1/√e below
Theorem 1, for the same reason as Proposition A: the cell-by-cell bound cannot use the averaging of second moments.
The anisotropic question it points to is open: the least Gaussian mean width of Γ^½K among sets K of given
Gaussian measure.

### F5. Shared information and groupings [proved, unrefereed]

**Statement.** Let M items have independent residuals, item j a message c_j, and all share a message s. If the mean
of H(c_j) is at most B nats and H(s) ≤ S nats, the mean over items of E h is at least V_S(B + S/M + J) for every
head set, in particular W(B + S/M + J). A partition into K groups with fixed centres lowers the floor by at most
ln K nats of rate (Proposition 1 of `groups/REPORT.md`, with W or V_S in place of Theorem 1).

**Proof.** F1 to F3 hold with the message (c_j, s) and rate I(z_j; c_j, s). By independence of the items
Σ_j I(z_j; c_j, s) ≤ Σ_j H(c_j) + H(s), as in Corollary 2 of the consolidated proofs. V_S is convex and
non-increasing (L4), so the mean of V_S(I_j) is at least V_S of the mean. The grouping is the same with the label
as part of the message. ∎

### F6. Offsets that are scale mixtures of Gaussians [proved, unrefereed]

**Statement.** Let Δ = t·Σ_Q^½ g with t ≥ 0 independent of g and of r, and keep the rest of the model. Then
E h ≥ E[t]·V_S(H(c) + J) for every S.

**Proof.** Condition on t. The model holds with Σ_Q replaced by t²Σ_Q, the encoder is unchanged, every γᵢ is
multiplied by t², the water-filling is invariant to the scale, and W and V_S are multiplied by t. Average over
t. ∎

This is the only step taken here towards offsets that are not Gaussian. Offsets that depend on the residual, and
offsets carried by a few directions, are outside it (the section 6 remark that the floor is false for offsets on
finitely many directions stands).

### F2-alt. The brief's order: head point first [sketch: gaps named]

Recorded because the brief asked what it proves and what stops it. It is dominated by F2.

**The argument.** One cell A ⊂ ℝ^S × ℝ^T with ν = γₙ(· | A), P = γₙ(A), deterministic encoder. For a head point s
let p(s) = γ_T(A_s) be the mass of its fibre; the fibre's law has KL(ν_{T|s} ‖ γ_T) = ln(1/p(s)). Write κ_S for the
KL of the head marginal and κ̄_T for the mean fibre KL; κ_S + κ̄_T = ln(1/P). Fix ε ∈ (0, 1) and let
G = {s : ln(1/p(s)) ≤ κ̄_T/ε}; by Markov ν_S(G) ≥ 1 − ε. Choose s*(g_S) in G within η of the supremum of
Σ_S √γᵢ gᵢ sᵢ over G, then couple g_T to the fibre over s* by its triangular map. Then

- tail, given g_S: at least Φ_T(ln(1/p(s*))) ≥ Φ_T(κ̄_T/ε), by Proposition A(a) for the fibre law;
- head: Urysohn on G, with ln vol(G) ≥ h(ν_S(· | G)) and
  KL(ν_S(· | G) ‖ γ_S) ≤ (κ_S + ln 2)/(1 − ε), from the decomposition of ν_S over G and its complement.

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

## 6. Prior art, as read [cited; both sources read in full where quoted]

Read from the arXiv TeX sources on 21 November 2026 (details in `NOTEBOOK.md`, entry 5).

**P1. Ingber, Courtade, Weissman, "Compression for quadratic similarity queries", arXiv:1307.6609, IEEE Trans. Inf.
Theory 61(5), 2015.** Signatures of nR bits for i.i.d. Gaussian vectors, a query answered no/maybe with no false
negative for (1/n)‖x − y‖² ≤ D, rate achievable if Pr[maybe] → 0. Theorem 1: for N(μ, σ_X²) against N(μ, σ_Y²),
R_ID = log₂(2σ_Xσ_Y/(σ_X² + σ_Y² − D)) between (σ_X − σ_Y)² and σ_X² + σ_Y². The converse is proved: typical shell,
projection of each cell on the sphere, Lévy's isoperimetric inequality, a Markov-type lemma over cells; it gives
the bound with O(log n/n) terms and a shell thickness that is sent to zero. Theorem 4: the Gaussian pair needs the
largest rate for given variances, by achievability; no converse for other laws.
- In our terms [my arithmetic, as the librarian's]: the least inner-product level per coordinate that can be
  certified at a typical query is σ_Xσ_Y·2^(−R) = √min(γ, θ). So for an isotropic spectrum F1 is the finite-n,
  mean-width counterpart of their converse, and their achievability shows F1 is tight to first order as n → ∞.
- It does not give: a bound at a given n; a bound on E h; anisotropic spectra.

**P2. Wu, Wang, Flierl, "Computing similarity queries for correlated Gaussian sources", arXiv:2001.07941.**
Theorem 1 states R = (1/M)Σ max(0, log(ξ_m/τ)) at D = (1/M)Σ max(0, 2(ξ_m − τ)) for query and data of the same
covariance; with γ = ξ², θ = τ² that is the curve of W. What the paper proves is an optimal allocation of thresholds
inside one family of schemes (KLT, a system per component, answers combined by AND). It has no converse: the
passage from the family to all schemes is an assertion. Its Lemma 1 equates {Σ d_m ≤ Σ D_m} with {d_m ≤ D_m for all
m}, and the AND rule as stated can answer no for a pair within the total threshold, so the achievability is also
not established as written.
- So the anisotropic formula was stated before; a proof of its converse was not in either paper. F1 is a proof of
  the converse, in the mean-width form, at every n, for Σ_Q ≠ Σ_R.

**P3. Which route.** I kept the transport route and did not adapt the Ingber–Courtade–Weissman converse. Their
argument needs the query uniform on a sphere on which the cell is projected, so both laws must be isotropic; the
authors say the general isoperimetric inequality is unknown. At finite n its shell corrections do not vanish. The
isotropic finite-n analogue of their converse already exists (Theorem 1 at k = n, with Urysohn in place of Lévy),
and F2 contains it.

**P4. Tensorisation.** The chain-rule step is the Marton–Talagrand tensorisation (Gozlan–Léonard 2010, Proposition
1.8) [cited, unchecked: librarian's reading]; as the PI notes it cannot be cited as it stands for a bilinear cost,
and L2 proves what is needed directly.

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
values are the lower bounds of `entropy/REPORT.md`, so these factors bound the real-data floor from above only.

---

## 8. Consequence for the design at low rate

### D1. The norm-only dual-basis certificate [proved, unrefereed; numbers computed]

**Scheme.** Store ρ̂, the least point of the grid {ρ₀ qʲ : j ∈ ℤ} that is at least ρ = ‖r‖_{M⁻¹}, and certify
h = ‖Δ‖_M·ρ̂. The message is j. Write X for ‖Δ‖_M; in eigen-coordinates X² and ρ² both have the law of
Σᵢ √γᵢ gᵢ², so E X² = F, and let cv² = Var(X)/(E X)².

**Theorem D1.** Under the Gaussian model:
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
- With the shipped grid 2^(1/16) the norm carries 3 bits of entropy, and no certificate of any kind with a 3-bit
  message can be more than 3.6% narrower on the Gaussian model. At zero code bytes the dual-basis ball is the
  right certificate, to that margin.
- The ratio is smallest near q = 2^(1/16). A finer norm buys nothing (the width is already F) while the floor at its
  entropy keeps falling: past about 3 bits, a bit is better spent on a coordinate than on the norm. A floor for any
  b-bit code, as a share of F: 0.962 at 4 bits, 0.905 at 8, 0.823 at 16 (V; W is 0.928, 0.876, 0.800).
- H_q is small because the norm concentrates: ln ρ has standard deviation about (2n_eff)^(−½) = 0.081, about 1.9
  grid steps at 2^(1/16), and ½ log₂(2πe(1.9² + 1/12)) = 2.97 bits against the 2.95 measured.
- The bound of item 4 is 1.13 at 2^(1/16); the computed ratio is tighter because V is used in place of F − √γ₁H.

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
    M = Σ_Q⁻¹ # Σ_R. This is Theorem 2, equations (15) and (16), of Bhatia, Jain and Lim, Expo. Math. 37 (2019)
    [cited, unchecked by me: the librarian's reading], and it is Proposition 3 of `groups/REPORT.md`.
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
(smallest ratio 1.0014); no random coupling above F (largest 0.75).

## 10. B2. No norm beats the M-ellipsoid [proved, unrefereed]

**B2a (second moments).** For every norm ‖·‖_K with dual norm ‖·‖_K°, and any laws of Δ and r,
√(E‖Δ‖²_K·E‖r‖²_K°) ≥ C*(P_Δ, P_r). For the Gaussians the right side is F, attained by the M-ellipsoid.

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

This is the classical argument that the mean norm times the mean dual norm on the sphere is at least 1, run
through the Monge coupling. It answers the librarian's caution (`library/theory.md`, 2.2 item 1) that the minimiser
of the mean "need not be the geometric mean": it is the same metric.

**B2c (scale mixtures).** If Δ = t·Δ₀ and r = s·r₀ with Gaussian Δ₀, r₀ and scales t, s ≥ 0 independent of them,
then for every norm E‖Δ‖_K·E‖r‖_K° = E t·E s·E‖Δ₀‖_K·E‖r₀‖_K° ≥ E t·E s·(E‖Δ₀‖_M)². The M-ellipsoid of the Gaussian
parts stays the best norm.

**What is not claimed.** Laws far from elliptical. For Δ uniform on {±e₁, ±e₂} and r uniform on
{±(e₁ ± e₂)/√2}, the pair ‖·‖₁, ‖·‖_∞ certifies 1/√2 where every ellipsoid certifies 1.

**Check.** 240 random norms (‖Ax‖_p with its dual for p ∈ {1, 1.5, 2, 3, ∞}, and random ellipsoids), n ∈
{1, 2, 3, 5, 8}, 200,000 sample pairs each. Sound on every pair (smallest margin −6·10⁻¹⁵, rounding). Smallest
mean-product ratio to the M-ellipsoid 1.0000 (one dimension, where all norms are proportional); smallest
second-moment ratio to F 0.9947 (the same case, sampling error of the squares). (E‖g″‖)²/F = 0.637, 0.784, 0.939,
0.987 at n_eff = 1, 2, 7.9, 36.9, and 1/(1 + cv²) agrees to 10⁻³.

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

How large can it be: 1 − E s/√(E s²) can be made as near 1 as one likes with a heavy-tailed scale, so the gain has
no bound below F. At the measured kurtoses it is 1 to 13% of F.

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
   Σⱼ aⱼbⱼ = min over D constant on blocks of ‖DΔ‖‖D⁻¹e‖, at dⱼ² = bⱼ/aⱼ (Cauchy–Schwarz and its equality case). It is
   the support function of the product of the block balls, the exact range given the J block norms.
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
   measured for items 0.98, 0.96, 0.94, 0.89. The model has two thirds to three quarters of the gain.
5. *Against the floor* [computed]. Sixteen norms at about 3 bits of entropy each are 48 bits. On group 12 of the
   agent 1 spectra the floor at 48 bits is V = 0.63·F (section 7, first row), and sixteen block norms reach about
   0.9·F. Bits spent on norms buy a quarter of what the floor allows; coordinates are the better use, which is the
   experiment's byte count.

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
   metric is optimal for every family; the last is attained by families with orthogonal supports and equal
   w_aF_a².
4. *Two families.* Put δ = (F_w² − w₁F₁² − w₂F₂²)/(2√(w₁w₂)·F₁F₂). Then 0 ≤ δ ≤ 1 and

       w₁F₁²(reg₁² − 1) + w₂F₂²(reg₂² − 1) = 2δ·√(w₁w₂)·F₁F₂,      reg_a = reg_a(M_w).

   The total excess is fixed by δ; the weights decide how the two families share it.
5. *In terms of the spectra.* Let B_a = Σ_R^½ Σ_a Σ_R^½, with eigenvalues γ^(a). Then F_a = tr B_a^½ and
   F_w = tr(w₁B₁ + w₂B₂)^½. If B₁ and B₂ commute, F_w = Σᵢ √(w₁γᵢ^(1) + w₂γᵢ^(2)) and
   reg_a² = (Σᵢ pᵢτᵢ)(Σᵢ pᵢ/τᵢ) with pᵢ = √γᵢ^(a)/F_a and τᵢ = √(γᵢ^(w)/γᵢ^(a)), which is 1 + Var_p(ln τ) to second
   order. In general, for any metric M, reg_a(M)² = tr(PN_a)·tr(P⁻¹N_a), where N_a = Γ_a^½/F_a has trace 1 and P is M
   written in family a's dual coordinates (P = I for M = M_a); hence reg_a(M) ≤ ½(√κ + 1/√κ), κ the condition number
   of P (Kantorovich's inequality).
6. *First order against second order.* Along a smooth path of metrics from M_a the derivative of ω_a² is zero at
   M_a. So mixing a small share of another family into the fit costs family a an amount of second order in the
   share, and gains the other family an amount of first order.
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
the third. (4) The identity is (2) rearranged; 0 ≤ δ ≤ 1 is (3). (5) F(Σ) = tr(Σ_R^½ΣΣ_R^½)^½ because the two
orderings of the product have the same non-zero eigenvalues. In family a's dual coordinates Σ_a and Σ_R both
become Γ_a^½, so tr(MΣ_a) = tr(PΓ_a^½) and tr(M⁻¹Σ_R) = tr(P⁻¹Γ_a^½); in the eigenbasis of P this is a scalar
Kantorovich inequality for the probabilities (N_a)_jj. The commuting case is the same with P diagonal. (6) M_a
minimises ω_a². (7) Normalise tr(M⁻¹Σ_R) ≤ 1, a convex constraint; max_a is the maximum over the simplex of the
bilinear function Σ_a w_a tr(MΣ_a)/F_a²; Sion's minimax theorem applies because the simplex is compact
[cited, standard]; the inner minimum is F(·)² by (1). The bounds are (3) with every F(Σ_a/F_a²) = 1. ∎

The mixture's *mean* width, Σ_a w_a ω_a(M), is not convex in M (it is a sum of square roots of linear functions
on the normalised set), so the mean-square criterion and the minimax criterion are the two that can be solved
exactly.

**Check.** `ot_checks.py`: concavity of F and of F² and subadditivity on 400 random triples (margins 7·10⁻⁴,
10⁻³; largest F(X+Y)/(F(X)+F(Y)) = 0.91); minimax on 40 families of three (gap between the two sides 10⁻³, no
random metric better, regrets 1.016 to 1.213); price on 400 random mixtures between 1.0004 and 1.53, and exactly 2
for two orthogonal rank-one families.

### B4c. Does the theory predict the measured 21 to 33% against 6 to 19%? [computed against measured]

`mixture_price.py`, log `mixture_price.log`. It rebuilds the experiment's set-up: its fitted Σ_Q for items and
text (`work/setup.npz`), each group's residual covariance recomputed from the 200k vectors as its `measure.py`
does, the dual basis for a fit Σ_w = (1 − w)·Σ_item/tr + w·Σ_text/tr with the same ridge, and the profile weights
from the same fit. w = 0 is its bound "a" (data-only) and w = ½ its bound "b2" (mixed). A level at reach k bounds
the coordinates from k on. The prediction of a mean half-width is the second-moment formula
√(Σ_{i≥k} Dᵢ²Pᵢ)·√(Σ_{i≥k} λᵢ/Dᵢ²), with P the profile in that basis of a second-moment matrix of query offsets,
summed over groups by size. No score of a query against an item enters.

*With the test queries' own second moments.* Mixed basis over data-only basis, at k = 0 / 32 / 256:

| Test queries | Predicted | Measured |
|---|---|---|
| Held-out text (369) | 0.787 / 0.732 / 0.669 | 0.788 / 0.732 / 0.672 |
| In-sample species prompts (361) | 0.991 / 1.011 / 0.986 | 0.991 / 1.011 / 0.988 |
| Items (100) | 1.052 / 1.080 / 1.179 | 1.055 / 1.084 / 1.195 |

Text-fitted diagonal weights in the data-only basis (its bound b1) over data-only: text 0.907 / 0.935 / 0.970
predicted against 0.907 / 0.938 / 0.977; items 1.157 / 1.058 / 1.030 against 1.163 / 1.058 / 1.026.

*From the two fits alone*, each family evaluated on its own fit, which is what can be computed before a test query
is seen: text 0.725 / 0.676 / 0.513, items 1.054 / 1.099 / 1.257. At k = 0 the regrets against each family's own
optimum F_a are 1.001 (items) and 1.462 (text) for the data-only basis, 1.055 and 1.061 for the mixed basis, 1.229
and 1.002 for the text-only basis.

**So: yes, with one qualification.**
- Given the queries' second moments, the cut of 21 to 33% for text and the cost of 6 to 19% for items are what
  the calculus of second moments gives, to 0.3 points for text and 1.6 points at worst for items. The absolute
  widths are predicted 4 to 10% above the measured means (0.310 against 0.297 for text at k = 0), as a root mean
  square should be; the ratio between bases does not see that.
- From the fits alone the item cost is right at k = 0 (5.4% against 5.5%) and both effects are overstated with
  depth: the text gain by 6 points at k = 0 and 16 at k = 256, the item cost by 6 points at k = 256. The held-out
  prompts are not distributed as the training prompts, and the held-out items not quite as the pseudo-queries; the
  size of that overstatement measures how far.
- What this licenses: a change of basis can be costed from Σ_R, the fits and a second-moment matrix of a query
  sample, without scoring a query against the corpus. The caveat is the experiment's own: the held-out prompts
  share templates with the training prompts.

### B4d. What a basis per query family, chosen by the query, would gain [computed; model]

The trade-off along the mixing weight, predicted on the test queries as in B4c, width over the data-only basis at
k = 0 / 32 / 256:

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
  gain and cuts the item cost from 5.2 / 8.0 / 17.9% to 1.6 / 2.3 / 6.5%. It stores nothing more. It is a
  prediction from second moments on this sample and has not been measured.
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

`layered.md` reports the derived ρ within 2 to 5% of the measured one on two datasets and 15% above on LAION text.
By item 4 that gap is the product of two factors that can be computed at build from Σ_Q and Σ_E, (F/Σ a_c)² and
Σ π_c v_c / tr(Σ_QΣ_E); I did not compute them for those datasets.

---

## 14. What failed, what is refuted, what is open

**Refuted or corrected.**
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

**Checks that are weaker than they look.**
- Parts 1 and 2 of `check_schemes.py` do not press on the new part of F2: in one or two dimensions an uncoded
  coordinate costs about twice its floor to bound. Part 3 does (smallest ratio 1.030), with shell masses from a
  sample.
- Every code tested is one I could compute exactly. No optimiser searched over cell shapes in more than one
  dimension. The separate numerical attack in `stage0-floor-check/` was not read and is not relied on.
- V could not be evaluated on the index behind `quant/REPORT.md`; the transfer from the agent 1 spectra is an
  estimate.

**Open.**
1. A floor for real data. F3 needs the negentropy J of the whitened residual from above; only lower bounds exist.
2. Offsets that are not Gaussian scale mixtures, and offsets that depend on the residual.
3. A statement for a typical query, not the mean (Ingber–Courtade–Weissman's form), at finite n.
4. The floor near zero rate for an anisotropic spectrum: the least Gaussian mean width of Γ^½K among sets of given
   Gaussian measure. F4 answers it for Γ = I.
5. Achievability at finite n for an anisotropic spectrum. D1 is the only matching upper bound here (3.6% at 3
   bits); at higher rates the escape code is 1.26 to 1.59 above V.
6. Whether L1 and B2b are in the literature (Hosking 2007 for L1; the M·M* argument for B2b).
7. Section 6's other conjectures are untouched: "Gaussian residuals are the worst case" (F3 gives the bound's
   side only), √k for a class unknown at build, the tail without the factor 2.
8. B4d's cheaper move (text share 0.2 in the fit) is a prediction and has not been measured.

## 15. Files

All in `notes/program/floors/`.

| File | What it is |
|---|---|
| `NOTEBOOK.md` | the dated log |
| `CLAIMS.md` | this file |
| `floors_lib.py` | Theorems 1 to 3, Φ, W and V as functions of a spectrum and a rate |
| `check_lemma1d.py`, `.log` | L1 on 40,003 laws |
| `check_chain1d.py`, `.log` | the inner inequality of F1 on interval partitions |
| `check_schemes.py`, `check_schemes_part1.log`, `_part2.log`, `_part3.log` | F1 and F2 against explicit codes |
| `check_f4.py`, `.log` | F4 |
| `floors_eval.py`, `.log` | section 7 |
| `design_lowrate.py`, `.log` | D1 |
| `ot_checks.py`, `.log` | B1 to B5 |
| `mixture_price.py`, `.log` | B4a (model of block norms), B4c, B4d |
| `work_sigmaR.npz` | cache of the twelve group residual covariances for `mixture_price.py` (28 MB, float32) |
