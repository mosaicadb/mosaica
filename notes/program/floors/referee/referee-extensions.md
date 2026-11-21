# Referee report: floors track, extensions and task B

Scope: `CLAIMS.md` sections 5 (F3 to F6), 6 (prior art), 8 (D1), 10 to 13 (B2 to B5), and the attribution in
section 9 (B1). Sections 1 to 4 (L1 to L5, A, F1, F2) are another referee's and are taken as given. `NOTEBOOK.md`
was not read. Scripts are beside this file.

Status of this file: **complete** (21 November 2026). Verdicts use the brief's four words. "No error found" means I
could not break the claim after a real attempt; it does not mean correct.

## Section 6 and B1: the published sources

All three were read from the arXiv TeX source (each has one arXiv version only, v1). The journal texts were not
opened, so theorem and equation numbers are those of the arXiv versions.

### P1. Ingber, Courtade, Weissman, arXiv:1307.6609

| Statement in CLAIMS.md | Finding |
|---|---|
| Signatures of nR bits for i.i.d. Gaussian vectors, no/maybe with no false negative for (1/n)‖x − y‖² ≤ D, rate achievable if Pr[maybe] → 0 | Right. |
| Theorem 1: R_ID = log₂(2σ_Xσ_Y/(σ_X² + σ_Y² − D)) between (σ_X − σ_Y)² and σ_X² + σ_Y² | Right (0 below the range, ∞ at and above it; both laws have the same mean μ). |
| The converse is proved: typical shell, projection of each cell on the sphere, Lévy's isoperimetric inequality, a Markov-type lemma over cells, O(log n/n) terms, shell thickness sent to zero | Right. The converse assumes Pr[maybe] ≤ ¼, restricts to the shell, projects T⁻¹(i) on the sphere, uses the isoperimetric inequality on the sphere (cited from Ledoux), then Lemma 4 ("Markov") for unequal cells, and lets η → 0. It is asymptotic and it uses the spherical symmetry of both densities. "Genuine but asymptotic and isotropic" is a fair description. |
| Theorem 4: the Gaussian pair needs the largest rate for given variances, by achievability; no converse for other laws | Right. Theorem 4 needs identical means and finite variances. The paper says: "For general source distributions P_X, P_Y, we lack a matching lower bound." |
| "the authors say the general isoperimetric inequality is unknown" (P3) | Right: "In general, precise isoperimetric inequalities are unknown and therefore establishing a general converse appears to be extremely difficult." |
| "the least inner-product level per coordinate that can be certified at a typical query is σ_Xσ_Y·2^(−R) = √min(γ, θ)" | Right as arithmetic. ‖x − y‖²/n ≤ D is x·y/n ≥ τ with τ = (σ_X² + σ_Y² − D)/2 on the typical shells, and R_ID = log₂(σ_Xσ_Y/τ). With γ = σ_X²σ_Y² per coordinate and R = ½log₂(γ/θ), √θ = σ_Xσ_Y·2^(−R). |
| "their achievability shows F1 is tight to first order as n → ∞" | **Overstated.** Their scheme answers maybe for every item outside the typical shell, so as a certificate its half-width is infinite with positive probability and its mean width is infinite at every n. What their achievability shows is tightness in probability (the width is at most n(√θ + o(1)) with probability tending to 1). Tightness of F1, a statement about E h, needs a scheme that also bounds the atypical items, for example the norm stored on a grid at o(n) nats. That is plausible and not in the paper. |
| "It does not give: a bound at a given n; a bound on E h; anisotropic spectra" | Right. |

### P2. Wu, Wang, Flierl, arXiv:2001.07941

| Statement in CLAIMS.md | Finding |
|---|---|
| Theorem 1 states R = (1/M)Σ max(0, log(ξ_m/τ)) at D = (1/M)Σ max(0, 2(ξ_m − τ)), query and data of the same covariance | Right (the first theorem of the paper; τ ∈ (0, ξ_max]). |
| With γ = ξ², θ = τ² that is the curve of W | Right: the rate is Σ ln⁺(ξ/τ) = Σ ½ln⁺(γ/θ), and the threshold on the inner product is Σξ_m − ½·M·D = Σ min(ξ_m, τ) = W. |
| What is proved is an optimal allocation inside one family (KLT, a system per component, AND); no converse | Right. The step from the family to all schemes is the sentence "the optimal condition also achieves equality in (16), hence the component-based model preserves the original Pr{maybe}". Nothing is proved about a scheme outside the family. |
| "Its Lemma 1 equates {Σ d_m ≤ Σ D_m} with {d_m ≤ D_m for all m}" | Right, with one precision: the equation is in the **proof** of Lemma 1, not its statement (the display after the paper's inequality labelled `eq:Dlarger`: Pr{(1/M)Σ_m d(T_m⁻¹(T_m(X^(m))), Y^(m)) ≤ (1/M)Σ_m D^(m)} = Pr{d(·)^(1) ≤ D^(1), …, d(·)^(M) ≤ D^(M)}). The second event is a strict subset of the first in general, so the equality is false and, as an inequality, it goes the wrong way for an upper bound on Pr{maybe}. Lemma 1's statement (the whole system's Pr{maybe} vanishes if and only if some component's does) is not what the theorem's proof then needs either. |
| "the AND rule as stated can answer no for a pair within the total threshold, so the achievability is also not established as written" | Right. A pair with d_1 a little above D^(1) and d_2 well below D^(2) is within the total threshold, component 1 may answer no, and AND then answers no: a false negative. The rule that is admissible is "no only if the sum of the components' lower bounds on the distance exceeds the total", which is not what the paper describes. |
| "the anisotropic formula was stated before; a proof of its converse was not in either paper" | Right for these two papers. I did not search for a later corrected version or for other papers. |

### B1. Bhatia, Jain, Lim, arXiv:1712.01504 (Expo. Math. 37, 2019)

Theorem 2 of the arXiv version reads: for positive definite A and B, (i) F(A, B) = min over X > 0 of
½ tr(AX + BX⁻¹), equation (15); (ii) F(A, B) = min over X > 0 of √(tr(AX)·tr(BX⁻¹)), equation (16); (iii) a
maximum form, equation (17). F(A, B) = tr(A^½BA^½)^½, and the proof gives the minimiser X₀ = A⁻¹ # B.
With A = Σ_Q, B = Σ_R, X = M this is B1(a) exactly. **The attribution is right**, including the equation numbers
(the TeX labels are eq15 and eq16; I did not render the paper to confirm the printed numbers, and did not open the
journal version). B1's tag "cited, unchecked by me" can become "cited, source read" on the referee's reading.

Two further findings on section 6's wording, outside the two tables:

- F3's remark that it "agrees in form with" Ingber–Courtade–Weissman's Theorem 4 is **overstated**. Theorem 4 says
  only that a non-Gaussian pair needs no more rate than the Gaussian pair of the same variances; it has no
  negentropy term. It agrees with F3 in direction. The form "rate plus relative entropy" is the
  Ordentlich–Polyanskiy statement, which I did not check.
- P3's "Theorem 1 at k = n is the isotropic finite-n analogue of their converse" is fair as an analogy: theirs
  bounds a probability at a threshold, Theorem 1 a mean width.

---

## Summary table

| Claim | Verdict | Rests on sections 1 to 4? |
|---|---|---|
| F3 | no error found; one line of the inherited proof must be replaced (below) | yes: falls with F1, F2, L1 to L4 |
| F4 | no error found; hypothesis "deterministic encoder" is not needed | no (Theorem 1 step 3 only) |
| F5 | correct after a stated repair (two hypotheses to state; the grouping sentence to make precise) | yes: F1 to F3, L4.1 |
| F6 | no error found | yes: F1 to F3 |
| D1 items 1, 2 | no error found | no |
| D1 items 3, 4 and the V, W columns | no error found | yes: F2, L4.4 |
| B2a, B2b, B2c | no error found | no |
| B3a, B3c, B3d, B3e, B3f | no error found; one sentence true but not shown | no |
| B4a items 1, 2, 3 | no error found | no |
| B4a item 4 | wrong in one sentence ("two thirds to three quarters") | no |
| B4a item 5 | no error found | yes: the 0.63·F is V |
| B4b items 1, 2, 4, 5, 7 | no error found | no |
| B4b item 3 | correct after a stated repair (the attainment condition) | no |
| B4b item 6 | gap (the first-order half is not proved); repaired below | no |
| B4b closing remark | wrong in its conclusion: the mean width is geodesically convex | no |
| B5 items 1 to 5 | no error found | no |
| B5 closing sentence | wrong (one factor, not two; and the measured ρ is another quantity) | no |
| B4c, B4d | the script computes what the text says; fits-alone figures use no test query | no |

---

## F3. Residuals that are not Gaussian

**Verdict: no error found**, given F1 and F2.

What I checked. E zzᵀ = I is automatic once Σ_R is the second-moment matrix of r, so it is not an extra
hypothesis. J = (n/2)ln(2πe) − h(z) is KL(law of z ‖ N(0, I)) because E‖z‖² = n. I went through F1 and F2 line by
line for uses of the Gaussian law of z. There are three, not two:

1. h(z) = (n/2)ln(2πe) in step 4 of F1 and step 6 of F2. Replaced by −J as the proof says.
2. E zᵢ² = 1 in step 5 of F1. Kept.
3. **ν_c ≤ γₙ/P_c**, used in step 2 of F1 (density, finite second moments, h(ν_c) > −∞) and in step 5 of F2 ("the
   densities are bounded by (2π)^(−n/2)/P_c", to make a_c and b_c finite). This is false for a non-Gaussian z and
   F3's proof does not mention it. **Repair:** ν_c ≤ μ/P_c with μ the law of z, so ν_c has a density and
   E_{ν_c}‖z‖² ≤ n/P_c. Each h(ν_c) is bounded above through its second moment, and Σ_c P_c·h(ν_c)⁺ < ∞ by Jensen.
   If H(c) = ∞ the claim is empty. Otherwise Σ_c P_c h(ν_c) = h(z) − I(z; c) > −∞, so every h(ν_c) is finite. In F2,
   a_c and b_c are each bounded above (second moments) and sum to h(ν_c), so both are finite. Nothing else changes.

The step u ≤ I + J uses the maximum-entropy bound for a given second-moment matrix (not covariance), which is right.

Hypotheses used and not stated: H(c) < ∞ for the entropies to be finite (harmless); the message independent of Δ
given r (section 0).

Statement against proof: the proof gives E h ≥ V_S(I(z; c) + J), which is **stronger** than the stated
V_S(H(c) + J).

Wording to correct in the paragraph after the proof and in section 7: the lower bounds on J from
`entropy/REPORT.md` do not "bound the true floor from above". They bound from above **what F3 can prove**:
W(H + J_true) ≤ W(H + J_lower). The least possible width for the real data may lie above or below W(H + J_lower).

Numerical attack (`ref_f3_f4.py`, log `ref_f3_f4.log`).
- z uniform on a square rotated by an angle φ (a non-product law in eigen-coordinates for φ ≠ 0, J = 0.353 nats),
  γ = (1, ε²), 4,000 codes with rectangular cells, mean width exact through the perimeter of Γ^½·cell, entropy
  exact, against max over the four head sets of V_S(H + J): smallest ratio 1.1284 (= 2/√π, a square cell against
  the disc). No violation.
- The same codes against the Gaussian floor without J: smallest ratio 0.854. So the correction J is needed; the
  Gaussian floor at rate H is not a floor for this law.
- A coded uniform coordinate times an uncoded two-piece coordinate (J up to 5 nats): smallest ratio 1.159.
- By hand: n = 1, uniform law, uniform grid of m cells. E h = E|g|·√3/m and κ₁e^(−(ln m + J)) are equal. F3 with
  S = {1} is attained, as Theorem 1 is.

## F4. An isotropic spectrum at very low rate

**Verdict: no error found.** It does not use sections 1 to 4.

Checked: Urysohn applied to the closed support K_c; γₙ(K_c) ≥ P_c; among sets of a given volume the centred ball
has the largest Gaussian measure (the density is radially decreasing), so the ball with the volume of K_c has
measure at least P_c and radius at least ρₙ(P_c). dρ/du = −F/f is right, and it is non-decreasing in u exactly when
F is log-concave. The χₙ density is log-concave on its support for n ≥ 1, and Prékopa gives the rest. Jensen with
u_c = ln(1/P_c) and mean H.

Hypothesis stated and not needed: **"deterministic encoder"**. For a randomised encoder ν_c ≤ γₙ/P_c and
ν_c(K_c) = 1 still give γₙ(K_c) ≥ P_c, and the rest is unchanged. The statement is weaker than the proof on this
point.

The high-rate remark (1/√e below Theorem 1) is right: ρₙ(P) ≈ √(2π)·ωₙ^(−1/n)·P^(1/n) for small P.

Numbers: the log reproduces exactly on rerun (`rerun_check_f4.log`). "Only below a few tenths of a nat" holds at
n = 8, 76, 768; at n = 1 and 2 the crossing is below 0.1 nat (2.057 against 1.632 at 0.01 nat, 1.331 against 1.492
at 0.1 nat for n = 1).

Numerical attack: ln(F/f) for χₙ has no decrease on a grid of 20,000 points for n in {1, 2, 3, 5, 10, 76, 768};
3,000 partitions of the line into intervals and of the plane into annuli, exact widths, smallest ratio of E h to
the bound 1.135. No violation.

## F5. Shared information and groupings

**Verdict: correct after a stated repair** (nothing in the mathematics fails; the statement omits what it needs).

The chain Σ_j I(z_j; c_j, s) ≤ I(z^M; c^M, s) ≤ Σ_j H(c_j) + H(s) is right for independent items, and the mean of
V_S(I_j + J) is at least V_S of the mean because V_S is convex and non-increasing (L4.1).

Hypotheses used and not stated:
1. **The items have the same spectrum and the same J** (identically distributed, or at least the same second-moment
   matrix and negentropy). Otherwise there is no single V_S and no single J to put the mean rate into.
2. **Δ is independent of every item's residual**, not only of item j's. The message (c_j, s) is a randomised
   function of r_j whose randomness is the other items; section 0 allows that only if it is independent of Δ
   given r_j.

Stronger than stated, by the same proof: the certificate for item j may read every item's message, since
I(z_j; c^M, s) is what the chain bounds.

The grouping sentence needs its reference floor named. What the proof gives is this. Hold the law of Δ fixed and
independent of the item x. A grouping with fixed centres, an encoder of r = x − m_ℓ and a certificate for Δ·r are
together a certificate of the same width for Δ·x with message (ℓ, c). So
E h ≥ V_S^x(H(c | ℓ) + H(ℓ) + J_x), with the spectrum and the negentropy **of x about the fixed origin**, and
H(ℓ) ≤ ln K. Read as "the floor with the within-group Σ_R, at ln K more nats" the sentence would be a different
statement, which is not what is proved. Proposition 1's own caveat (fixed centres, offset law held fixed) carries
over.

## F6. Offsets that are scale mixtures of Gaussians

**Verdict: no error found**, given F1 to F3.

Conditional on t the model holds with t²Σ_Q; z, the message law and J do not change with t; W and V_S are
homogeneous of degree one in √γ. The certificate may depend on t through Δ, which the conditioning allows.

Not stated, and worth one line: Σ_Q here is the covariance of the Gaussian part. The covariance of Δ is E[t²]·Σ_Q,
so against the floor computed from the offsets' own covariance the factor is E t/√(E t²) ≤ 1.

## D1. The norm-only dual-basis certificate

**Verdict: no error found.** Items 1 and 2 stand alone; items 3 and 4 and the columns W and V rest on F2 and L4.4.

Checked: X² and ρ² have the law of Σᵢ √γᵢgᵢ² (this needs Σ_Q and Σ_R positive definite so that M and M⁻¹ exist; not
stated). ρ̂ ≤ qρ. E h = E X·E ρ̂ needs Δ independent of r (in the model). The inequality
√x ≥ 1 + (x − 1)/2 − (x − 1)²/2 holds for x ≥ 0 with the stated remainder, and Var(X²) = 2Σγᵢ, so
E X ≥ √F(1 − 1/n_eff). The script computes E X from the Laplace transform, rounds a sample of 200,000 norms up on
the grid, takes the plug-in entropy and evaluates W and V at it; that is what the theorem says. The log reproduces
exactly on rerun (`rerun_design_lowrate.log`), and every figure in the table and in the reading matches it.

Three small points on the reading:
- "a 3-bit message" should be "a message of entropy 2.95 bits". The floor is in entropy; the grid is unbounded,
  so the scheme has no fixed-length 3-bit form.
- "past about 3 bits, a bit is better spent on a coordinate than on the norm" is **stronger than what is shown**.
  The floor says what any code could gain at most; nothing here shows that a coordinate code gains it.
- The plug-in entropy from 200,000 samples on about 20 occupied cells is biased low by about 10⁻⁴ bits; no effect.

## B1 (attribution only)

See the first section: Theorem 2, equations (15) and (16), of the arXiv version is the statement cited.

## B2. No norm beats the M-ellipsoid

**B2a: no error found.** Cauchy–Schwarz under any coupling, and the left side depends on the marginals only.
Unstated: finite second moments. For the Gaussians C* = F by B1(b), (c).

**B2b: no error found.** √(⟨Δ, r⟩⁺) ≤ √a·√b pointwise because ab ≥ 0 and ab ≥ ⟨Δ, r⟩; Cauchy–Schwarz under π; the
Monge coupling r = MΔ gives E‖Δ‖_M; the M-ellipsoid attains it because ‖MΔ‖_{M⁻¹} = ‖Δ‖_M. The statement is about
E a·E b, which is the mean certificate when Δ and r are independent (stated in the heading). The conclusion "the
M-ellipsoid minimises the mean certificate over all product forms" follows.

**B2c: no error found**, for norms (homogeneity is used). It does not extend to product forms that are not
homogeneous, and the text does not claim it.

**The counter-example under "What is not claimed"** is right: the pair ‖·‖₁, ‖·‖_∞ is sound on the eight points
with mean and root-mean-square product 1/√2; Σ_Q = Σ_R = I/2 gives F = 1; 20,000 random ellipsoids give a least
mean product of 1.0000 and a least root-mean-square product of 1.0000 (`ref_ot.py`).

The author's check reproduces exactly (`rerun_ot_checks.log`).

## B3. One-scalar certificates

**B3a: no error found.** **B3b** is cited; I did not open Villani (Theorem 2.12 is Brenier's theorem from memory).

**B3c: no error found.** W₂² − G² = 2(F − C*) by subtraction.

**B3d: no error found.** The conjugate of ½(f + g) is at most ½(f* + g*), and the conjugate of φ(−·) is φ*(−·).
Both laws must be symmetric, as stated.

**B3e: no error found.** The lower coupling is a coupling of the two laws (t independent of Δ₀, s independent of
MΔ₀); the upper bound is ⟨Δ, r⟩ ≤ XY, Hardy–Littlewood, then Cauchy–Schwarz with E X² = E Y² = F. The lognormal
arithmetic is right: E s = e^(−σ²/2), kurtosis 3e^(4σ²), σ² = 0.046, 0.112, 0.133, 0.280, and every entry of the
table matches the log.

One sentence is true but not shown: **"the gain has no bound below F"**. The text supports it with the upper bound
F(1 − E s), which bounds the gain from above. What is needed is that the lower bound F − C₁(X, Y) tends to F.
**Repair:** by Hölder C₁(X, Y) ≤ ‖X‖₄·‖Y‖_{4/3}, ‖Y‖_{4/3} = ‖s‖_{4/3}·‖Y₀‖_{4/3}, and for the lognormal scale
‖s‖_{4/3} = e^(−σ²/3) → 0 while ‖X‖₄ is fixed.

**B3f: no error found.** E t·E s ≤ C₁(t, s) is Chebyshev's inequality for comonotone variables. The comparison is
between means with Δ independent of r.

## B4a. Several stored scalars

**Items 1, 2, 3: no error found.**
- Item 1. The balls contain the origin in their interiors, so the infimal convolution needs no closure. "The least
  such bound" is the least of the split form: the exact range given the J norms (equalities) can be smaller, and
  the text does not claim otherwise. Rockafellar's Corollary 16.4.1 is the right statement from memory; I did not
  open the book.
- Item 2. Right when every a_j and b_j is positive; with a zero the minimum is an infimum.
- Item 3. Right. It needs only that each coordinate is Gaussian and that Δ is independent of e; the coordinates
  need not be uncorrelated for the per-coordinate figure (2/π)·Σ a_c.

**Item 4: wrong in one sentence.** "The model has two thirds to three quarters of the gain." From the measured
rows for items at k = 0 (`report_measure.txt`: 0.1951, 0.1912, 0.1857, 0.1765 over 0.1982) and the model's 0.994,
0.981, 0.958, 0.916:

| J | measured gain | model gain | model / measured |
|---|---|---|---|
| 2 | 1.6% | 0.6% | 0.38 |
| 4 | 3.5% | 1.9% | 0.54 |
| 8 | 6.3% | 4.2% | 0.67 |
| 16 | 10.9% | 8.4% | 0.77 |

**Repair:** "two fifths to three quarters, rising with J". The other figures of item 4 match the files (0.63 for
items; 0.62 to 0.64 for the in-sample species prompts; 0.57 and 0.907
for held-out text; model 0.641).

**Item 5: no error found** as arithmetic (0.0462/0.0728 = 0.63; (1 − 0.9)/(1 − 0.63) = 0.27). "Three bits each"
for a block norm is an estimate, and a block's norm concentrates less than the whole norm, so the 48 bits is a low
count.

## B4b. One basis for a mixture of query families

**Items 1, 2, 4, 5, 7: no error found.**
- Item 1. F² and F are minima of linear functions of Σ (B1(a) and scale invariance). The subadditivity proof by
  coupling is right; B1(b) is used for a pair whose first covariance is only positive semidefinite, which holds by
  taking the infimum over M.
- Item 5. Verified by whitening with Σ_R: ω_a(M_w)² = tr(B_w^(−½)B_a)·tr(B_w^½). In the commuting case the
  eigenvalues must be paired by common eigenvector, not by rank; the text does not say so.
- Item 7. The feasible set is convex (M ↦ tr(M⁻¹Σ_R) is convex), the function is linear in each argument and the
  simplex is compact, so Sion applies. The minimum over M is attained and equals the metric of the least favourable
  mixture when every Σ_a is positive definite (coercive objective, unique minimiser of the inner problem); that
  hypothesis is the section's standing one. Numerically (`ref_ot.py`, 15 instances): the worst regret at the metric
  of the least favourable mixture equals the dual value to 8·10⁻⁹, and a direct minimisation of the primal through
  its epigraph never goes below the dual value (largest excess 3·10⁻¹⁵).

**Item 3: correct after a stated repair.** The three inequalities are right. The attainment claim is not:
"families with orthogonal supports" attain F_w² = A·Σ w_aF_a² only when the supports are orthogonal **in the
Σ_R inner product**, Σ_aΣ_RΣ_b = 0 for a ≠ b. The author's check uses Σ_R = I, where the two notions agree.
Counter-example: Σ₁ = e₁e₁ᵀ, Σ₂ = e₂e₂ᵀ, equal weights, Σ_R = [[1, c], [c, 1]]. The price is 2.000, 1.954, 1.800,
1.436 at c = 0, 0.3, 0.6, 0.9. With u = e₁ and v = Σ_R⁻¹e₂ (u·v = −0.94, uᵀΣ_Rv = 0) it is 2.000 (`ref_ot.py`).

**Item 6: gap.** The proof line is "(6) M_a minimises ω_a²", which gives the second-order cost to family a and
nothing about family b. **Repair.** Let M(ε) minimise (1 − ε)ω_a² + ε·ω_b² on the slice tr(M⁻¹Σ_R) = 1.
Differentiating the first-order condition at ε = 0 gives M′(0) = −H_a⁻¹∇ω_b²(M_a), with H_a the Hessian of ω_a² on
the slice, so d/dε ω_b(M(ε))² at 0 is −∇ω_b²ᵀH_a⁻¹∇ω_b² ≤ 0. H_a is positive definite on the slice when Σ_a and
Σ_R are (in family a's dual coordinates the second variation is tr(H²N) − tr(HN)², zero only in the scale
direction). So the gain is of first order **unless M_a is already stationary for family b**, that is unless
M_a ∝ M_b. Numerically, halving ε from 0.2 to 0.0125 divides a's cost by 2.8, 3.1, 3.4, 3.6 (towards 4) and b's
gain by 1.5, 1.6, 1.7, 1.8 (towards 2).

**The closing remark is wrong in its conclusion.** "The mixture's mean width Σ_a w_aω_a(M) is not convex in M …
so the mean-square criterion and the minimax criterion are the two that can be solved exactly." The function is
indeed not convex along straight lines (it is constant on rays, and a convex function with that property is
constant). But it is **convex along the geodesics M₀ #_t M₁** of the positive definite cone, so every local minimum
is global and it can be minimised by descent like the other two.

*Proof.* For A ⪰ 0 the block matrix [[M₀, X], [X, M₁]] with X = M₀ # M₁ is positive semidefinite, so
tr(AX)² ≤ tr(AM₀)·tr(AM₁) (the argument of Bhatia–Jain–Lim's Theorem 2(iii)). With continuity, t ↦ tr(A(M₀ #_t M₁))
is log-convex. (M₀ #_t M₁)⁻¹ = M₀⁻¹ #_t M₁⁻¹, so t ↦ tr((M₀ #_t M₁)⁻¹Σ_R) is log-convex too. A product, a square
root and a positive combination of log-convex functions are log-convex. ∎

In the commuting case this is the convexity of log-sum-exp in the logarithms of the diagonal weights. Numerically:
3,000 random pairs of metrics, the function at the geodesic point never above the chord (largest
excess −1.3·10⁻⁴ of the chord), and above the chord on straight lines by up to 128%; on 12 instances, six random
starts reach the same minimum to 10⁻¹⁶. In the first instance the least mean width is 5.60112, the mean-square
metric M_w gives 5.60227, and Σ w_aF_a = 5.54827. So M_w is close to the minimiser of the mean and is not it.

## B4c and B4d. The computation against the measurement

I read `mixture_price.py` and its log; I did **not** rerun it (it loads the 28 MB cache and the experiment's
set-up file, which is over the memory limit of this session).

- The prediction is √(Σ_{i≥k} Dᵢ²Pᵢ)·√(Σ_{i≥k} λᵢ/Dᵢ²) per group, weighted by group size, with D from the fit's own
  profile and λ from the group's residual covariance: what B4c says, and the second-moment form of B2a and B5.1.
- In the table "with the test queries' own second moments", P is the profile of the second-moment matrix of the
  test queries' offsets (each query projected off each cluster centre, weighted by cluster size). Test queries
  enter through that matrix only; no score of a query against an item enters. This is what the text says.
- In the lines "from the two fits alone", P is the profile of the fitted matrices `Si` and `St`. No test query
  enters. `prep.py` states that these are fitted on training queries only and that the pseudo-query sample excludes
  the held-out items; I read that statement and the lines that build `Si` and `St`, and did not re-derive the
  split.
- The measured figures hard-coded in the script match `report_measure.txt` for all three kinds of query.
- Every figure quoted in B4c and B4d matches the log: the three tables, 0.3 and 1.6 points, 4 to 10%, 6 and 16
  points, 5 / 7 / 15% and 2.5 / 3.7 / 2.7%, 78 / 79 / 86%, 1.117 and δ from 0.110 to 0.123.
- The regrets against the test queries' own optimum are still printed by the script (1.36 to 1.67); the text
  rightly does not use them.

Two points of reading, not errors:
- The first table is a check that second moments reproduce the measured ratios when the test queries' second
  moments are given. It is not a prediction made before seeing them, and for the 100 item queries that matrix has
  rank at most 100 in 768 dimensions. The text says "given the queries' second moments"; the heading "predicted"
  should not be read as more.
- The fits-alone profile uses the fitted matrices as they are, and the test profile uses offsets projected off the
  centres. If the fits were built from offsets as `prep.py`'s header says, the two agree; I did not check that.

"The choice needs no classifier" is sound: the choice depends on the query and on build constants, and each
certificate is sound for every pair.

## B5. n_eff and rule 1's ρ²

**Items 1 to 5: no error found.**
- Item 1: a diagonal D in coordinates A is the metric AᵀD²A, so B1(a) applies; equality only at multiples of M.
  E h² = E‖DΔ‖²·E‖D⁻¹e‖² needs independence, which is assumed.
- Item 3: the "exactly when" holds. Term-by-term equality gives both sums equal, hence M diagonal (item 1), hence
  Σ_E = PΣ_QP in the level's coordinates and tr(Σ_QΣ_E) = Σ (Σ_Q)²_{cc′}d_c²d_{c′}² ≥ Σ a_c², with equality only
  for diagonal Σ_Q. I checked the algebra of the change of coordinates.
- Item 4: the two identities hold to 4·10⁻¹⁶ on 200 random pairs.

**The closing sentence is wrong.** "By item 4 that gap [derived ρ against measured ρ in `layered.md`] is the
product of two factors … (F/Σ a_c)² and Σ π_cv_c/tr(Σ_QΣ_E)."
1. By item 4 itself, rule 1's ρ² over the true ratio of second moments is the **single** factor
   Σ π_cv_c/tr(Σ_QΣ_E). The factor (F/Σ a_c)² relates the true ratio to 1/n_eff. It is not part of the gap between
   the derived and the true ρ.
2. The measured ρ of `notes/05-approximate.md` §5.9 is the root mean square over pairs of (score − midpoint)/h, a
   mean of ratios. It is not √(E(Δ·e)²/E h²), and second moments do not determine it. Example (`ref_ot.py`, n = 3,
   Gaussian, independent): rule 1's ρ² = 0.390, the ratio of second moments 0.397, 1/n_eff = 0.414, and the mean
   over pairs of (Δ·e)²/h² = 0.357.

**Repair:** "rule 1's ρ² is the true ratio of second moments times Σ π_cv_c/tr(Σ_QΣ_E), which can be computed at
build; the measured ρ is a mean of per-pair ratios and differs from both by the correlation between h and the
error, which second moments do not give."

---

## What I ran, and two breaches of the brief

Scripts in this directory: `ref_f3_f4.py` (log `ref_f3_f4.log`), `ref_ot.py` (log `ref_ot.log`). Reruns of the
author's `check_f4.py`, `design_lowrate.py` and `ot_checks.py`, each identical to the author's log
(`rerun_*.log`). All on core 2, nice 10, one thread.

- **Memory.** Three runs went over 100 MB resident: the first run of `ref_ot.py` (125 MB for 28 s; the sample was
  then cut and the final run is 91 MB), and the reruns of the author's `design_lowrate.py` (111 MB, 12 s) and
  `ot_checks.py` (180 MB, 24 s).
- **A search of the whole filesystem.** Looking for `layered.md` I started `find /`, unpinned and at normal
  priority. It ran for about two minutes before I stopped it. Timings taken on this machine in that window (some
  two minutes between about 03:30 and 03:40 local on 21 November) may be disturbed.

## What I did not check

- Sections 1 to 4 (L1 to L5, A, F1, F2), by the brief. F3, F5, F6, D1 items 3 and 4 and the 0.63·F of B4a fall
  with them. F4, B2, B3, B4b and B5 do not depend on them.
- The journal versions of the three papers (arXiv v1 of each was read); Villani, Rockafellar, Gozlan–Léonard,
  Ordentlich–Polyanskiy, Hosking; whether a corrected version of Wu–Wang–Flierl or a later converse exists.
- `mixture_price.py` was read and compared with its log, not rerun. The measured figures of the block-norm
  experiment and of `layered.md` were taken from their files, not re-measured. `layered.md` itself was not found
  in the worktree; I used its transcription in `notes/05-approximate.md` §5.9.
- Section 7's tables, F2-alt, and P4.
- B3b (cited). The lognormal model of B3e's table is the author's choice and I did not test it against data.
