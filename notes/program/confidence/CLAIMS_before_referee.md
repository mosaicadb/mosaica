# Confidence track: claims

Tags are those of `notes/README.md`. Symbols are those of `notes/00-notation.md`; local symbols are declared here.
Nothing in this file is refereed. Each claim is saved when it is stated; a later section may correct an earlier one,
and says so.

## C1.0 The model (one block, one item, one query)

Local symbols. d ≥ 2 is the block's degrees of freedom and the block lives in ℝ^(d+1). S^d is its unit sphere and σ
the uniform probability on S^d. B is the number of stored bits and R = B/d the rate in bits per degree of freedom.
Ω_d(θ) = σ{x : angle(x, x₀) ≤ θ} is the normalised cap measure. U is one coordinate of a uniform unit vector in ℝ^d
(so a uniform point of the unit sphere of y^⊥), and q_d(δ) is its upper δ-quantile: Pr[U ≥ q_d(δ)] = δ.

**M1 (seed).** The build draws ω from a probability space (W, μ). ω does not depend on the item or the query.

**M2 (message).** The item's block is y ∈ S^d. Only unit blocks are considered: a lower bound for them is a lower
bound for every scheme that must also handle other norms, and the upper bound scales by ‖y‖ when ‖y‖ is stored
exactly outside the B bits. The stored message is c = enc(y, ω) ∈ {1, …, M}, M = 2^B = 2^(Rd). enc is any measurable
function. Nothing else about y is kept.

**M3 (query).** The query offset is Δ ∈ ℝ^(d+1), fixed independently of ω. Δ_⊥ = Δ − (Δ·y)y. The quantity to bound
is s = Δ·y.

**M4 (certificate).** At query time the scheme outputs an estimate ŝ = est(c, Δ, ω) ∈ ℝ and a half-width
h = hw(c, Δ, ω) ≥ 0. Both are any measurable functions. In particular h may depend on the message (the index's
stored scale A = ‖y‖tan φ is a function of (y, ω) and is counted inside c here), on Δ, and on ω. The estimate need
not be linear in Δ and need not be unbiased.

**M5 (validity).** Two-sided δ-validity: for every y ∈ S^d and every Δ,
Pr_ω[ |ŝ − Δ·y| > h ] ≤ δ.
One-sided δ-validity (the harmful side only): for every (y, Δ), Pr_ω[ Δ·y − ŝ > h ] ≤ δ. Because the requirement
holds for Δ and for −Δ, a one-sided scheme is treated by the same argument; C1 says where the two differ.

**M6 (what is measured).** h is random, so "the least half-width" needs a functional. The claims below use
w̄ = E over (y uniform on S^d, Δ uniform on S^d, ω) of h, and state which results hold for other functionals
(a quantile of h; the worst (y, Δ) of E_ω h). A lower bound on the average w̄ is a lower bound on the worst case.

**Not in the model.** Several blocks, several levels, several items (C2), and a query-side seed (Theorem 7 of
`proofcheck/CONSOLIDATED_PROOFS.md` already shows it does not help).

## C1.1 Achievability

**Fact F1 (cap measure).** [proved, unrefereed] On S^d ⊂ ℝ^(d+1), for 0 ≤ θ < π/2,
Ω_d(θ) = κ_d ∫₀^θ sin^(d−1)t dt with κ_d = Γ((d+1)/2)/(√π·Γ(d/2)), and
κ_d·sin^dθ / d ≤ Ω_d(θ) ≤ κ_d·sin^dθ / (d·cos θ).
Proof. The lower bound: sin^(d−1)t ≥ sin^(d−1)t·cos t, whose integral is sin^dθ/d. The upper bound: the derivative
of sin^dt/(d cos t) is sin^(d−1)t + sin^(d+1)t/(d cos²t) ≥ sin^(d−1)t. ∎
κ_d/d = (2πd)^(−½)(1 + O(1/d)). So Ω_d(θ)^(1/d) = sin θ·(1 + O(ln d / d)) for θ bounded away from π/2.

**Claim A1 (the scheme and its exact error law).** [proved, unrefereed; rests on Lemma 8 as refereed]
ω = (Rot, C): Rot a Haar rotation of ℝ^(d+1) and C a codebook of M unit vectors, independent of Rot (C may be
fixed). Store c = the index of the codeword nearest Rot·y, and φ = its angle to Rot·y. For φ < π/2 put
ŝ = Δ·Rotᵀcode / cos φ. Then ŝ − Δ·y = tan φ·‖Δ_⊥‖·U, with U one coordinate of a uniform unit vector in ℝ^d,
independent of (φ, c, C). Hence, for any threshold rule that depends on (φ, Δ) only,
h = q_d(δ/2)·tan φ·‖Δ‖ is two-sided δ-valid and h = q_d(δ)·tan φ·‖Δ‖ is one-sided δ-valid;
and the scheme "if q·tan φ ≥ 1 output ŝ = 0, h = ‖Δ‖" is also valid (|Δ·y| ≤ ‖Δ‖ always).
Proof. Lemma 8 with ‖y‖ = 1: the error vector is E = tan φ·u, u uniform on the unit sphere of y^⊥ given Rot·y,
hence given (φ, c). Δ·E = tan φ·‖Δ_⊥‖·(u·Δ_⊥/‖Δ_⊥‖). The conditional failure probability given φ is
Pr[|U| > q_d(δ/2)·‖Δ‖/‖Δ_⊥‖] ≤ δ, since ‖Δ_⊥‖ ≤ ‖Δ‖. The switch to (0, ‖Δ‖) is a function of φ. ∎
Remark. With ‖Δ_⊥‖ in place of ‖Δ‖ the failure probability is exactly δ. The decoder does not know ‖Δ_⊥‖;
for Δ uniform, ‖Δ_⊥‖² = ‖Δ‖²(1 − Beta(½, d/2)), so the loss is a factor 1 − O(1/d) on average.

**Claim A2 (the scalar inside the bit budget).** [proved, unrefereed]
A1 stores the real number φ. Inside M2 it must be quantised. Store ĉ = cos φ rounded down on a geometric grid
of ratio 1 + η, and use ŝ = Δ·Rotᵀcode/ĉ and t̄ = √(1/ĉ² − 1). Then g = cos φ/ĉ ∈ [1, 1+η],
ŝ − Δ·y = (g − 1)·Δ·y + g·tan φ·‖Δ_⊥‖·U, and
h = [η + (1 + η)·q·t̄]·‖Δ‖ is valid (q = q_d(δ/2) or q_d(δ)), with t̄ ≤ (1 + η)·tan φ + √(3η).
Proof. |(g−1)Δ·y| ≤ η‖Δ‖. t̄ ≥ tan φ because ĉ ≤ cos φ, so g·tan φ·‖Δ_⊥‖·|U| ≤ (1+η)·t̄·‖Δ‖·|U|, and the event
{|U| > q} has probability δ given φ. t̄² ≤ (1+η)²/cos²φ − 1 = (1+η)²tan²φ + 2η + η². ∎
With φ ≤ θ₁ < π/2 guaranteed (or the fallback of A1 outside it), the grid on [cos θ₁, 1] has
ln(1/cos θ₁)/ln(1+η) cells. Taking η = d⁻² costs O(log d) bits, which is O(log d / d) in R.

**Claim A3 (the angle at rate R).** [proved, unrefereed]
Let C be M independent uniform points of S^d (part of ω). For every fixed y and θ,
Pr[φ > θ] = (1 − Ω_d(θ))^M ≤ exp(−M·Ω_d(θ)).
So with θ₁ defined by M·Ω_d(θ₁) = 2 ln d, Pr[φ > θ₁] ≤ d⁻². By F1, sin θ₁ = 2^(−R)·(1 + O(ln d/d)) when R is
fixed, so tan θ₁ = (4^R − 1)^(−½)·(1 + O(ln d / d)) (the constant in O depends on R and grows as R → 0).
Proof. The M angles to Rot·y are independent and each is ≤ θ with probability Ω_d(θ). ∎
A fixed covering code does the same without the d⁻² exception; Böröczky and Wintsche give covering density
at most d ln d + d ln ln d + 5d. [cited, unchecked: from the librarian's summary]

**Theorem A (achievability).** [proved, unrefereed]
For fixed R > 0 and δ = δ_d with q_d(δ_d)·√d ≥ 1 there is a scheme in the model M1 to M5, of B = Rd + O(log d)
bits, valid two-sided at δ, whose half-width satisfies, for every (y, Δ) with ‖Δ‖ = 1,
h ≤ q_d(δ/2)·(4^R − 1)^(−½)·(1 + O(ln d / d)) with probability at least 1 − d⁻² over ω, and h ≤ 1 always.
So E_ω h ≤ q_d(δ/2)·(4^R − 1)^(−½)·(1 + O(ln d/d)) + d⁻². One-sided: the same with q_d(δ).
Proof. A1, A2 with η = d⁻², A3. The additive terms η and q·√(3η) are O(1/d) relative to q·tan θ₁ because
q ≥ d^(−½) and tan θ₁ is a constant. ∎

**The limit on tan φ (from `notes/program/codes/REPORT.md`).** For any M points and y uniform,
Pr[φ ≤ θ] ≤ min(1, M·Ω_d(θ)). With F1: Pr[sin φ ≤ 2^(−R)·x] ≤ κ_d·x^d/(d·cos θ). So no codebook, random or
not, has sin φ below 2^(−R)(1 − a/d) on more than about e^(−a)/√(2πd) of the sphere: Theorem A's angle is the
least possible for schemes of the Lemma 8 form. Whether schemes of another form can do better is C1.2.

## C1.2 The converse: first findings (saved before the lemmas; these change the target)

Notation for this section: x₀ = Rotᵀcode (the decoded direction, a unit vector), s = Δ·y, E₀ = Δ·x₀, ‖Δ‖ = 1,
q the relevant quantile, k = q²tan²φ.

**Claim N1 (three estimators from the same message, exact laws for fixed (y, Δ)).** [proved, unrefereed]
In the scheme of A1, x₀ = cos φ·y + sin φ·v with v uniform on the unit sphere of y^⊥ and independent of φ. So
E₀ = cos φ·s + sin φ·√(1 − s²)·U, U a coordinate of a uniform unit vector in ℝ^d, independent of φ. Then:
(a) Unbiased, ŝ = E₀/cos φ: error tan φ·√(1−s²)·U. Half-width q·tan φ (uses √(1−s²) ≤ 1).
(b) Shrunk, ŝ = cos φ·E₀: error −sin²φ·s + sin φ cos φ·√(1−s²)·U. The noise is q·sin φ·cos φ, but the bias
    sin²φ·s is unknown to the decoder and is as large as sin²φ. Valid only with half-width sin²φ + q·sin φ cos φ,
    unless |s| is known to be small.
(c) Pivot (the one-block case of R8): the set {s : |E₀ − cos φ·s| ≤ q·sin φ·√(1 − s²)} fails with probability
    exactly δ (two-sided, q = q_d(δ/2)). It is the interval with centre (E₀/cos φ)/(1+k) and half-width
    q·tan φ·√(1 + k − E₀²/cos²φ)/(1 + k), which is at most q·tan φ/√(1+k). For one block it is never wider than
    (a) and needs no second event.
Proof. The law of x₀ given y is invariant under rotations fixing y (Lemma 8's argument). (a), (b): substitute.
(c): the pivot (E₀ − cos φ·s)/(sin φ√(1−s²)) = U has a law free of s; solve the quadratic. ∎

**Claim N2 (the PI's averaging cannot give tan φ).** [proved, unrefereed, as a statement about the proof method]
If the requirement is averaged over y uniform and Δ uniform and independent of y, then s = Δ·y is itself a sphere
coordinate (variance 1/(d+1)), and the shrunk estimator (b) with cap-shaped cells has average-case half-width
q·sin φ·(1 + O(1/√d))-ish, not q·tan φ: given (x₀, Δ) and y uniform in the cap of angle θ about x₀,
Δ·y = cos φ'·E₀ + sin φ'·√(1−E₀²)·U with φ' the angle of y from x₀ (φ' ≤ θ), exactly.
So any converse that averages over independent (y, Δ) proves at most q·sin θ = q·2^(−R). This is TurboQuant's
4^(−b) against 1/(4^b − 1) (library verdict 3), and it is why their bound has no −1.
The two differ by cos θ = √(1 − 4^(−R)): 3% at R = 2, 13% at R = 1, 46% at R = ¼.

**Claim N3 (which prior gives tan φ: heuristic).** [sketch: gap named]
Take y uniform, s drawn from a density π on (−1, 1), v uniform on the unit sphere of y^⊥, Δ = s·y + √(1−s²)·v.
Then Δ is uniform and independent of s, and given (Δ, s) the block y is uniform on the latitude
S_Δ(s) = {y : y·Δ = s}. The posterior density of s given (Δ, cell K) is proportional to π(s)·λ_{Δ,s}(K),
λ_{Δ,s} the uniform probability on S_Δ(s). For K the cap of angle θ about x₀,
λ_{Δ,s}(K) = Pr[U ≥ g(s)], g(s) = (cos θ − E₀·s)/(√(1−s²)·√(1−E₀²)), and
1 − g(s)² = [sin²θ·(1−E₀²) − (s − E₀cos θ)²] / [(1−s²)(1−E₀²)] + … (a Gram determinant over (1−s²)(1−E₀²)).
With π(s) ∝ (1−s²)^((d−2)/2) (independent Δ) the posterior is centred at E₀·cos θ with scale sin θ: Claim N2.
With π flat the posterior is the (d/2)-th power of G(s)/(1−s²), G(s) = sin²θ(1−E₀²) − (s − E₀cos θ)², whose
level set at 1 − q² is exactly the pivot interval (c): centre (E₀/cos θ)/(1+k), half-width q·tan θ/√(1+k) at E₀ = 0.
So the corrected target is the pivot interval's width, not q·tan θ:
  least half-width ≈ q·tan θ_R·√(1 + k − s²)/(1 + k), k = q²·tan²θ_R, tan θ_R = (4^R − 1)^(−½).
When δ is fixed and d → ∞, q → 0 like z_δ/√d, k → 0 and this is q·tan θ_R, the PI's target, for |s| ≪ 1.
When q stays of order one (the index: q ≈ 8.7/√255 ≈ 0.54, δ = e^(−Θ(d))), k = q²tan²θ is not small at low rates
(k = 0.04 at 2 bits, 0.10 at 1 bit, 0.7 at ¼ bit for d = 255) and the pivot interval is the right benchmark.
Gap: cells are not caps and the decoder is arbitrary; the lemmas below are the attempt to prove it.

## C1.3 The converse as numbered lemmas

Plan. The converse is proved for a class 𝓛 of schemes that contains the index's rule and every scheme of the
form "reconstruct a point, take its inner product with Δ, report a multiple of ‖Δ‖". The general model M4
(an estimate that is not linear in Δ, a half-width that depends on the direction of Δ) is left as a named gap (G1).

**Class 𝓛.** ŝ = Δ·ŷ(c, ω) for a reconstruction point ŷ(c, ω) ∈ ℝ^(d+1) (any length, any direction), and
h = η(c, ω)·‖Δ‖ with η ≥ 0. The message, the seed and the code are unrestricted. Local symbols, for fixed (y, ω):
e = ŷ − y; a = e·y = ŷ·y − 1 (the bias along y); b = ‖ŷ − (ŷ·y)y‖ (the error across y); ŷ = κ·x with x a unit
vector and κ ≥ 0; φ = angle(x, y) ∈ [0, π] (φ = π/2 if ŷ = 0). So a = κcos φ − 1, b = κ sin φ.
T_d(x) = Pr[U ≥ x] for U a coordinate of a uniform unit vector in ℝ^d (T_d = 1 below −1, 0 above 1), so
T_d(q_d(δ)) = δ. For q ∈ (0, 1]:
  H_q(φ) = q·sin φ / √(cos²φ + q²·sin²φ) for φ ≤ π/2, and H_q(φ) = 1 for φ ≥ π/2.
H_q(φ) = q·tan φ/√(1 + q²tan²φ). It increases in φ and in q.

**Lemma B0 (the algebra).** [proved, unrefereed]
(i) max over s ∈ [−1, 1] of [−s·a + q·√(1−s²)·b] = √(a² + q²b²).
(ii) min over κ ≥ 0 of (κcos φ − 1)² + q²κ²sin²φ = H_q(φ)², attained at κ = cos φ/(cos²φ + q²sin²φ) for
φ < π/2 and at κ = 0 for φ ≥ π/2.
Proof. (i) Cauchy–Schwarz for (−a, qb)·(s, √(1−s²)) with b ≥ 0. (ii) For cos φ ≤ 0 the first term is at least 1.
Otherwise the quadratic in κ has its minimum where cos φ(1 − κcos φ) = q²κ sin²φ, and the value there is
q²sin²φ·(cos²φ + q²sin²φ)/(cos²φ + q²sin²φ)² . ∎

**Lemma B1 (what validity says for one y and one family of queries).** [proved, unrefereed]
Let the scheme be in 𝓛 and one-sided δ-valid. Fix y and s ∈ [−1, 1]. Then
  E_ω T_d( (η + s·a) / (√(1−s²)·b) ) ≤ δ,
with the convention that the argument is +∞ or −∞ by the sign of η + s·a when √(1−s²)·b = 0 (T_d(0/0) := 0).
Proof. Let v be uniform on the unit sphere of y^⊥, independent of ω, and Δ = s·y + √(1−s²)·v. Each such Δ is a
fixed unit query, so Pr_ω[Δ·y − ŝ > h] ≤ δ for each v; average over v. Δ·y − ŝ = −Δ·e = −s·a − √(1−s²)·(e·v),
and e·v = b·U with U a coordinate of a uniform unit vector of ℝ^d, independent of ω. U is symmetric, so given ω
the failure probability is T_d((η + s·a)/(√(1−s²)·b)). ∎

**Lemma B2 (a grid of s and the steep tail).** [proved, unrefereed]
Let s_1, …, s_m be the points sin(−π/2 + iπ/(m−1)), i = 0..m−1, m ≥ 2, and ε_m = π/(2(m−1)). Fix q₀ ∈ (0, 1).
For every y,
  Pr_ω[ η < cos ε_m·√(a² + q₀²b²) ] ≤ m·δ / T_d(q₀).
Proof. Write (s, √(1−s²)) = (sin ψ, cos ψ), ψ ∈ [−π/2, π/2]. The maximiser of B0(i) has some angle ψ*, and a grid
angle ψ_i lies within ε_m of it, so −s_i·a + q₀√(1−s_i²)·b ≥ cos ε_m·√(a² + q₀²b²). If η is below that, then
(η + s_i·a)/(√(1−s_i²)·b) < q₀ and the i-th failure probability of B1 is at least T_d(q₀)
(when √(1−s_i²)·b = 0 it is 1). By B1 and Markov, Pr_ω[the i-th failure probability ≥ T_d(q₀)] ≤ δ/T_d(q₀).
Union over i. ∎
This is the PI's "steepness": T_d(q₀)/δ is the factor by which the failure probability rises when the half-width
falls from q_d(δ) to q₀ standard units, and it is exponential in d·(q_d(δ)² − q₀²)/2.

**Lemma B3 (the cap bound for reconstruction directions).** [proved, unrefereed]
For every fixed ω and θ ≤ π/2, σ{y : φ(y, ω) ≤ θ} ≤ M·Ω_d(θ).
Proof. φ(y, ω) ≤ θ puts y in the cap of angle θ about the direction of ŷ(c, ω) for c = enc(y, ω). There are at
most M such directions. ∎

**Theorem B (converse in 𝓛, every d and δ).** [proved, unrefereed]
For every one-sided δ-valid scheme in 𝓛 with M = 2^(Rd) messages, every m ≥ 2, q₀ ∈ (0,1), θ ∈ (0, π/2],
  σ⊗μ{ (y, ω) : η ≥ cos ε_m·H_{q₀}(θ) } ≥ 1 − m·δ/T_d(q₀) − M·Ω_d(θ),
and so E_{y,ω} η ≥ cos ε_m·H_{q₀}(θ)·(1 − m·δ/T_d(q₀) − M·Ω_d(θ)).
A two-sided δ-valid scheme is one-sided δ-valid, so the same holds; see B5 for what two-sidedness adds.
Proof. By B0(ii), √(a² + q₀²b²) ≥ H_{q₀}(φ), which is at least H_{q₀}(θ) when φ ≥ θ. Integrate B2 over y, and remove
the set of B3. ∎

**Corollary B4 (first order).** [proved, unrefereed]
Let R > 0 be fixed, d → ∞, and δ = δ_d → 0 with ln(1/δ_d)/ln d → ∞ (the index: δ ≈ 10⁻¹³, d ≤ 287, so
ln(1/δ) ≈ 30 against ln d ≈ 5.6). Then for one-sided δ-valid schemes in 𝓛,
  E_{y,ω} η ≥ H_{q_d(δ)}(θ_R)·(1 − o(1)),  sin θ_R = 2^(−R),  i.e.
  η ≳ q·tan θ_R/√(1 + q²·tan²θ_R) = q/√(4^R − 1 + q²),  q = q_d(δ).
Proof. Take m = d, T_d(q₀) = d²·δ, M·Ω_d(θ) = 1/d. Then cos ε_m → 1, the exceptional mass is 2/d, sin θ → 2^(−R)
by F1, and q₀/q_d(δ) → 1 because ln T_d(x) is, to first order, −(d/2)·ln(1/(1−x²)) and ln(d²) = o(ln(1/δ)). ∎
(The last step, the ratio of quantiles, is checked numerically in `c1_converse.py`; a referee should want the
two-line proof from Proposition 10 and the matching lower bound on the tail.)

**Claim A4 (the bound of Theorem B is attained in 𝓛: a shrunk centre).** [proved, unrefereed; simulated]
In the scheme of A1 (random or covering code, Haar rotation, φ stored), replace the estimator by
  ŷ = κ*·x₀, κ* = cos φ/(cos²φ + q²sin²φ) = 1/(cos φ·(1+k)),  η = H_q(φ) = q·tan φ/√(1+k),  k = q²tan²φ.
With q = q_d(δ) it is one-sided δ-valid for every (y, Δ), and the failure probability equals δ at
s = Δ·y/‖Δ‖ = +√(k/(1+k)) (the worst s: the shrunk centre underestimates a positive s) and is smaller elsewhere. With q = q_d(δ/2) it is two-sided δ-valid.
Proof. x₀ = cos φ·y + sin φ·v′, v′ uniform on the unit sphere of y^⊥ and independent of φ (N1). So
e = a·y + b·v′ with a = κ*cos φ − 1 = −k/(1+k), b = κ*sin φ = tan φ/(1+k). For a unit Δ = s·y + √(1−s²)·v
(v fixed), Δ·e = s·a + √(1−s²)·b·U. The one-sided failure probability given φ is
T_d((η + s·a)/(√(1−s²)·b)) ≤ T_d(q), because η = √(a² + q²b²) ≥ −s·a + q√(1−s²)·b (B0). Two-sided: add the two
tails. ∎
Consequences. (1) Theorem A's half-width q·tan φ is not optimal in 𝓛: it is beaten by the factor √(1+k), with the
same message, by shrinking the centre by 1/(1+k) and narrowing the interval; no extra event is spent. (2) Together
with Theorem B and A3: in 𝓛 the least half-width at rate R is, to first order,
  w*(R, δ, d) = q/√(4^R − 1 + q²),  q = q_d(δ)   (one-sided),
which is the PI's target q·(4^R − 1)^(−½) divided by √(1 + k). The target is correct in the limit k → 0.
(3) This is the isotropic form of R8's interval; R8's interval is narrower still when |ŝ| is not small
(by √(1 − ŝ²/(1+k)) in units where ‖y‖‖Δ‖ = 1), and that one is outside 𝓛.
Size of k for the index (q ≈ c/√d with c ≈ 7.4 to 8.7; ideal tan²θ_R = 1/(4^R − 1)): see `c1_converse.log`.

**Lemma B5 (one-sided against two-sided).** [proved, unrefereed]
(i) A two-sided δ-valid scheme in 𝓛 satisfies Theorem B with δ, and at s = 0 the stronger
E_ω T_d(η/b) ≤ δ/2 (both tails are equal there). (ii) A4 with q_d(δ/2) is two-sided δ-valid. So the two-sided
optimum lies between w* at q_d(δ) and w* at q_d(δ/2), and the one-sided optimum is w* at q_d(δ) exactly to first
order. (iii) A one-sided scheme run on Δ and on −Δ is a two-sided scheme at 2δ, since ŝ is odd and h even in Δ
for 𝓛. So one side costs exactly one halving of δ, as R6 used; q_d(δ/2)/q_d(δ) − 1 ≈ ln 2/(d·q²) (0.5% at
d = 255, q = 0.54).
Proof. (i) At s = 0 the error is b·U, symmetric. (ii) A4. (iii) Linearity. ∎

**Gap G1 (the general model).** [sketch: gap named]
Outside 𝓛 the decoder may use a half-width that depends on the direction of Δ and an estimate that is not linear
in Δ. What is proved for it: for fixed (ω, c) with cell K = enc(·, ω)⁻¹(c), the interval [ŝ − h, ŝ + h] contains
Δ·y for every y ∈ K on which the scheme does not fail, so 2h(c, Δ, ω) is at least the width, in direction Δ, of
the non-failing part of K. What is missing: a lower bound on the average over Δ of that trimmed width for an
arbitrary measurable K of measure 1/M, under the prior of N3 (Δ uniform, s = Δ·y with a flat density, y uniform
on its latitude). For caps the posterior computation of N3 gives the pivot interval N1(c). For general K one
needs a "trimmed Urysohn inequality": among sets of given measure, the cap minimises the mean over directions of
the width that holds all but a δ-fraction of the flat-prior mass. Urysohn's inequality is the case δ = 0 in
Euclidean space; I know no trimmed form and did not find a proof. Conjecture: the least average half-width in the
full model is that of the pivot interval, q·tan θ_R·√(1 + k − s²)/(1 + k) at Δ·y = s, which equals w* at s = 0.
Two things are certain meanwhile. The pivot interval shows the bound of 𝓛 can be beaten outside 𝓛 only through
dependence on ŝ, and by nothing when |s| ≪ 1 (a residual's offset has s of order 1/√d). And no scheme can beat
q·2^(−R)-type bounds by more than the averaged posterior allows; that weaker statement is also not proved here.

**Corollary B6 (Theorem B integrated; the form used for numbers).** [proved, unrefereed]
For ρ ∈ (0,1), u ∈ (0,1] let x(u) = q_d(mδ/(ρu)) (0 if mδ/(ρu) ≥ ½) and θ(u) solve M·Ω_d(θ(u)) = (1−ρ)u. Then
  E_{y,ω} η ≥ cos ε_m·∫₀¹ H_{x(u)}(θ(u)) du.
Proof. By Theorem B with q₀ = x(u), θ = θ(u), the event η < cos ε_m·H_{x(u)}(θ(u)) has probability at most u, and
the right side decreases as u decreases. So u ↦ cos ε_m·H_{x(u)}(θ(u)) lies below the quantile function of η. ∎

**Numbers [computed] (`c1_converse.py` part 3, `c1_converse.log`).** One-sided, δ = 10⁻⁹/(10⁹·4) = 2.5·10⁻¹⁹.
"Plain" is the index's rule q·tan θ_R at the ideal angle; w* = q/√(4^R − 1 + q²); "achievable" is A4 with a random
code at M·Ω = 2 ln d (a safe overestimate of the angle); B6 maximised over m ∈ {5, 9, 17, 33}, ρ ∈ {.5, .8, .95}.

| d | R | q_d(δ) | k = q²tan²θ_R | plain | w* | achievable | B6 bound | B6/achievable | B6/w* |
|---|---|---|---|---|---|---|---|---|---|
| 95 | ¼ | 0.7565 | 1.381 | 1.1754 | 0.7616 | 0.8187 | 0.7568 | 0.924 | 0.994 |
| 95 | 1 | 0.7565 | 0.191 | 0.4367 | 0.4002 | 0.4260 | 0.3917 | 0.920 | 0.979 |
| 95 | 2 | 0.7565 | 0.038 | 0.1953 | 0.1917 | 0.2032 | 0.1867 | 0.919 | 0.974 |
| 95 | 7 | 0.7565 | 0.000 | 0.0059 | 0.0059 | 0.0063 | 0.0057 | 0.919 | 0.972 |
| 255 | ¼ | 0.5186 | 0.649 | 0.8058 | 0.6275 | 0.6565 | 0.6120 | 0.932 | 0.975 |
| 255 | 1 | 0.5186 | 0.090 | 0.2994 | 0.2868 | 0.2952 | 0.2749 | 0.931 | 0.959 |
| 255 | 2 | 0.5186 | 0.018 | 0.1339 | 0.1327 | 0.1361 | 0.1268 | 0.932 | 0.956 |
| 255 | 7 | 0.5186 | 0.000 | 0.0041 | 0.0041 | 0.0041 | 0.0039 | 0.932 | 0.955 |

Reading. At the index's sizes the proved lower bound is within 7 to 8% of a proved upper bound, and within 1 to 5%
of the first-order formula w*. The rest is the finite-d cost of the union over the grid of s (2 to 3%), of the
integral over the steep tail (about 1%) and of the random code's margin. The plain rule q·tan θ exceeds w* by
√(1+k): 54% at ¼ bit and 9% at 1 bit on the block of 96; 28% and 4% on the block of 256; under 2% from 2 bits up.
These are statements about one block alone at the ideal angle; the built code's tan is 1.3 to 1.8 times the ideal
(`codes/REPORT.md`), which makes k larger, and with several blocks the gain belongs to the DESIGN track to price.

**Monte Carlo [simulated] (`c1_converse.py` part 1, d = 12, M = 512 random codewords, δ = 0.01, 400,000 draws per
row, standard error 0.00016).** One-sided failure rates at s = Δ·y:

| s | plain (A1) | naive shrunk (N1b) | shrunk centre (A4) | pivot (N1c) | mean width: plain | A4 | pivot |
|---|---|---|---|---|---|---|---|
| 0 | 0.0100 | 0.0100 | 0.0025 | 0.0100 | 0.581 | 0.499 | 0.485 |
| 0.2 | 0.0084 | 0.0585 | 0.0059 | 0.0100 | 0.581 | 0.499 | 0.477 |
| 0.4 | 0.0046 | 0.1721 | 0.0093 | 0.0103 | 0.581 | 0.498 | 0.453 |
| 0.6 | 0.0006 | 0.3739 | 0.0091 | 0.0102 | 0.581 | 0.499 | 0.412 |
| 0.8 | 0.0000 | 0.6778 | 0.0024 | 0.0100 | 0.582 | 0.499 | 0.347 |
| −0.4 | 0.0046 | 0.0000 | 0.0000 | 0.0103 | 0.581 | 0.499 | 0.454 |

The naive shrunk estimator (the one the averaged converse would allow) fails up to 68% of the time: N2 is not an
artefact. A4 never exceeds δ and is 14% narrower than plain at mean k = 0.35. The pivot's 0.0103 is noise: a
second run of 2,000,000 draws at s = 0.4 gives 0.00999 above, 0.00997 below, 0.01996 two-sided
(`c1_pivot_check.py`, `c1_pivot_check.log`).

**Steepness [computed] (part 2).** The factor T_d(t·q)/δ by which the failure probability rises when a half-width
is cut to t times the quantile, at δ = 2.5·10⁻¹⁹: for t = 0.98 / 0.95 / 0.90 it is 11.5 / 334 / 46,000 at d = 95 and
6.4 / 92 / 5,900 at d = 255. Conversely q_d(100δ)/q_d(δ) = 0.961 (d = 95) and 0.949 (d = 255).

**Gap G2 (δ fixed as d → ∞).** [sketch: gap named]
Corollary B4 needs ln(1/δ) ≫ ln d. For δ fixed, Theorem B still gives a bound of the right order (take m = 9,
T_d(q₀) = 30·m·δ: a constant factor below w*). The first-order constant is not proved. What is exact: at s = 0,
E_ω T_d(η/b) ≤ δ, and T_d is convex on [0, ∞), so E_ω[η/b] ≥ q_d(δ) for every y (Jensen). What resists: b = κ sin φ
must be bounded below by tan φ·(1 − o(1)), which needs κcos φ ≥ 1 − o(1); validity at s ≠ 0 gives that only
outside an event of probability of order δ, and an event of that size moves the quantile at first order when δ is
fixed. I did not find the right joint treatment of the two budgets. The index is not in this regime.

**The confidence tax.** [proved, unrefereed, given Theorems A4 and B; first order]
To reach half-width w (in units of ‖y‖‖Δ‖) a one-sided δ-certificate in 𝓛 needs, to first order,
  R_cert(w) = ½·log₂(1 + q²·(1 − w²)/w²) bits per degree of freedom, q = q_d(δ)   (from w = q/√(4^R − 1 + q²)).
To make the unbiased estimate's standard deviation tan θ_R/√d equal to w needs R_sd(w) = ½·log₂(1 + 1/(d·w²)).
The tax is R_cert − R_sd = ½·log₂[(w² + q²(1 − w²))/(w² + 1/d)]. For w ≪ q it is log₂(q·√d) = log₂ c:
2.88 bits per degree of freedom at d = 95 (c = 7.37), 3.05 at d = 255 (c = 8.28), 3.12 at d = 767 (c = 8.69).
It is smaller where the width saturates: at d = 255, w = 0.3 it is 0.92 bits.
In bytes: a block of 256 positions pays about 97 bytes of its code for confidence at fine widths, which is more
than the first randomised levels hold; there the certificate is in the saturated range and k is of order one.

**C1 summary: what is settled and what is not.**
- Message-dependent half-width (the stored scale): allowed in 𝓛 (η is any function of (c, ω)); it does not change
  the first-order answer. Theorem B bounds the mean of η, and a scheme whose η varies gains nothing beyond the
  probability budget of B2. [proved, unrefereed]
- Dependence of h on the direction of Δ, and estimates not linear in Δ: not covered (G1). The pivot interval shows
  such dependence does help when |Δ·y| is a sizeable share of ‖Δ‖‖y‖, by √(1 − ŝ²/(1+k)). [proved for that scheme]
- Biased against unbiased: the optimum in 𝓛 is neither sin (centre shrunk by cos²φ: invalid for worst-case Δ, N1b,
  simulated) nor tan (unbiased: valid but √(1+k) too wide). It is the centre shrunk by 1/(1+k) with half-width
  q·tan φ/√(1+k). An averaged requirement (Δ independent of y) would allow sin; this is the difference between
  TurboQuant's 4^(−b) and 1/(4^b − 1). [proved, unrefereed; simulated]
- One-sided against two-sided: one halving of δ, nothing else (B5). [proved, unrefereed]
- Constants: w* = q_d(δ)/√(4^R − 1 + q_d(δ)²), first order for ln(1/δ) ≫ ln d (B4); finite-d sandwich within 8%
  at the index's sizes (B6 table). [proved, unrefereed; computed]
- The PI's target q·(4^R − 1)^(−½) is the k → 0 limit of w*, and is an upper bound (Theorem A), not the optimum.
- Which queries force the bound. Theorem B uses queries with s = Δ·y/(‖Δ‖‖y‖) up to the worst value √(k/(1+k))
  (0.13 at k = 0.018, 0.29 at k = 0.09, 0.63 at k = 0.65). A guarantee asked only for queries with |s| ≤ s_max
  below that value would allow a centre shrunk further and a narrower interval; the bound with a restricted s is
  B0(i) with the maximum taken over |s| ≤ s_max, and is not worked out here. A typical offset has |s| of order
  1/√d, but the guarantee is for every query. [remark]
- Caveat on the low-rate rows. Where q·tan θ_R > 1 (¼ bit) the plain rule is wider than the deterministic bound
  |Δ·y| ≤ ‖Δ‖‖y‖, so the fair comparison there is min(plain, 1) against w*: 31% at d = 95 and 28% at d = 255.

# C2. The union over N items (closing R9)

## C2.0 The statement

Setting: the index's randomised levels, any number of blocks and levels. Block j of every item in a group is
rotated by the same Haar rotation Rot_j (shared across items; independent across blocks). A **per-item rule** is
any rule under which item i's decision at each level (exclude, include, read on) is a function of item i's own
stored messages, the rotations and the query. Item i **fails** (event F_i) if some level excludes it although it
matches or includes it although it does not. p_i = Pr_ω[F_i].
Claim to prove: if the answer is exact with probability 1 − ε on every corpus of N items, then the per-item
failure probability, averaged over a named family of items at the threshold, is at most (ε/N)·(1 + o(1));
the union bound over N is attained, so N cannot be replaced by a smaller count. Correction found on the way:
this is true when the code has many more codewords than there are items and false when it has fewer (C2.3).

## C2.1 The construction: an i.i.d. corpus on a latitude, and conditional independence

**Construction.** Fix the query, hence the offsets Δ_j per block, a threshold t, and per block a norm ρ_j and a
block score s_j with |s_j| ≤ ρ_j‖Δ_j‖ and Σ_j s_j = t − (the part of the score read before the randomised
levels, taken equal for all items). Draw each item's block j independently and uniformly from the latitude
Λ_j = {y : ‖y‖ = ρ_j, Δ_j·y = s_j}. Every item of this corpus has score exactly t: all N items are matches at
margin zero, and an interval that fails on the upper side at any level excludes a match.

**Theorem C (conditional independence; exact).** [proved, unrefereed]
Let Δ′ = (Rot_j Δ_j)_j. For any codebooks and any per-item rule, conditionally on Δ′ the events F_1, …, F_N are
independent with a common probability p(Δ′). Hence, averaged over the random corpus,
  Pr[some item fails] = E[1 − (1 − p(Δ′))^N],  and
  N·p̄ − (N(N−1)/2)·π₂ ≤ Pr[some item fails] ≤ N·p̄,  p̄ = E p(Δ′),  π₂ = E p(Δ′)² = Pr[F_1 ∩ F_2].
So some fixed corpus of N items at the threshold has Pr_ω[wrong answer] ≥ N·p̄ − (N²/2)·π₂.
Proof. Rot_j maps Λ_j onto the latitude of Δ′_j with the same (ρ_j, s_j). Given Rot_j (hence Δ′_j), the rotated
blocks z_{i,j} = Rot_j y_{i,j} are independent over i and uniform on that latitude, a law that depends on Rot_j
only through Δ′_j. Item i's messages are functions of (z_{i,j})_j, and its estimates and half-widths are
functions of its messages and of Δ′ (the estimate is Δ′_j·(decoded point); a rule may use Δ′ in any way). So F_i
is a function of ((z_{i,j})_j, Δ′), and these are i.i.d. over i given Δ′. The lower bound is Bonferroni's second
inequality, or (1−p)^N ≤ 1 − Np + N(N−1)p²/2. An average over corpora is attained by some corpus. ∎
Remarks. (1) No property of the code is used, and the rule may depend on Δ′, on the item's stored scales, and on
all levels at once. (2) The shared rotation matters only through Δ′: the theorem says the whole dependence between
items is carried by where the rotated query lands. (3) For this corpus a rule that looks at other items' messages
gains nothing about item i: given (ω, Δ′) the other items are independent of it. [remark; not used]

**Corollary C1 (R9 reduced to one number).** [proved, unrefereed]
If the answer is exact with probability 1 − ε on every corpus of N items, then p̄ ≤ ε/N + (N/2)·π₂. If
π₂ ≤ γ·p̄/N for some γ < 2 then p̄ ≤ (ε/N)/(1 − γ/2). In words: the union over N is unavoidable up to the factor
1 − γ/2 as soon as the chance that a second random threshold item fails, given that a first one does, is at most
γ/N. Independence would give Pr[F_2 | F_1] = p̄ ≈ ε/N, which is smaller than 1/N by the factor ε; so the claim
survives any positive dependence that multiplies the conditional probability by less than γ/ε (4·10⁸ for
ε = 10⁻⁹ and γ = 0.4).
With p̄ = Σ_ℓ T(a_ℓ; h_ℓ) up to the level overlaps of R7 (e⁻¹⁵ relative), this is R9's claim for the family of
items of the construction: the mean of Σ_ℓ T(a_ℓ; h_ℓ) over the scale vectors of a random latitude item is at
most (ε/N)(1 + o(1)).

## C2.2 The pairwise bound

Both lemmas are for one block, the equator corpus (s = 0, ρ = 1, ‖Δ‖ = 1: items uniform on the unit sphere of
Δ^⊥) and the plain one-sided rule of A1, h = q·tan φ with q = q_d(δ), so p̄ = δ exactly. In the rotated frame,
with x_i the codeword of item i and φ_i its angle, F_i = {−Δ′·x_i > q·sin φ_i} (and φ_i < π/2).

**Lemma C2 (random codebook: π₂ in closed form).** [proved, unrefereed]
Let the codebook be M independent uniform points of S^d, shared by all items (it is part of ω). For every
θ ∈ (0, π/2), with Λ = M·Ω_d(θ),
  π₂ ≤ 2·e^(−Λ) + Λ²·δ² + Λ·δ·Ω_{d−1}(θ),
and Ω_{d−1}(θ) ≤ Ω_d(θ)·(1 + 2/d)/(sin θ·cos θ) = Λ·(1 + 2/d)/(M·sin θ·cos θ) (for d ≥ 4). So
  N·π₂/δ ≤ 2N·e^(−Λ)/δ + N·Λ²·δ + (1 + 2/d)·N·Λ²/(M·sin θ·cos θ).
Proof. For a codeword c let G_i(c) = {angle(c, z_i) ≤ θ and −Δ′·c > q·sin angle(c, z_i)}. Then
F_i ⊂ {φ_i > θ} ∪ ⋃_c G_i(c), and Pr[φ_i > θ] = (1 − Ω_d(θ))^M ≤ e^(−Λ) (A3).
(a) One codeword, one item: given z_i and the angle φ ≤ θ of c from z_i, c = cos φ·z_i + sin φ·v with v uniform
on the unit sphere of z_i^⊥; Δ′ ⊥ z_i, so Δ′·c = sin φ·U with U a coordinate of a uniform unit vector in ℝ^d, and
Pr[G_i(c) | φ] = T_d(q) = δ. So Pr[G_i(c)] = Ω_d(θ)·δ.
(b) Two codewords c ≠ c′: given Δ′, (z_1, c) and (z_2, c′) are independent, so
Σ_{c≠c′} Pr[G_1(c) ∩ G_2(c′)] ≤ M²·Ω_d(θ)²·δ² = Λ²δ².
(c) One codeword for both: Pr[G_1(c) ∩ G_2(c)] ≤ Pr[G_1(c) and angle(c, z_2) ≤ θ]. Given (c, Δ′), z_2 is uniform
on the unit sphere of Δ′^⊥ and z_2·c = ‖c − (Δ′·c)Δ′‖·U with the norm at most 1, so
Pr[z_2·c ≥ cos θ | c, Δ′] ≤ T_d(cos θ) = Ω_{d−1}(θ). Sum over c: at most M·Ω_d(θ)·δ·Ω_{d−1}(θ).
The comparison of caps is F1 in dimensions d − 1 and d: Ω_{d−1}(θ) ≤ κ_{d−1}sin^(d−1)θ/((d−1)cos θ),
Ω_d(θ) ≥ κ_d sin^dθ/d, and κ_{d−1}·d/(κ_d·(d−1)) ≤ 1 + 2/d for d ≥ 4. ∎
[The last numerical inequality is checked in `c2_union.py`, not proved by hand.]

**Corollary C3 (for which N, M, d, δ the union is unavoidable; random code).** [proved, unrefereed; computed]
Take Λ = ln(2N/(γ₁δ)). Then γ = N·π₂/δ ≤ γ₁ + N·Λ²·δ + (1 + 2/d)·N·Λ²/(M·sin θ·cos θ), θ given by M·Ω_d(θ) = Λ.
For N = 10⁹, ε = 10⁻⁹, L = 4 (δ = 2.5·10⁻¹⁹) and γ₁ = 0.01: Λ = 68.9, the middle term is 1.2·10⁻⁶, and the last
term is below 0.01 once M·sin θ·cos θ ≥ 4.8·10¹⁴, that is from about 50 to 52 bits in the block (the table in
`c2_union.log`, part 2, gives 50 bits at d = 95 and 127 and 51 at d = 255 and 287; at 48 bits γ ≤ 0.05, at 40
bits the bound is useless). So for a block coded with at least 52 bits,
p̄ ≤ (ε/N)·(1 + 0.011): the count N is exact to one percent. The condition is M ≫ N·Λ²; it does not depend on d
or δ except through Λ = ln(N/δ) + O(1).
The smallest code listed in `codes/REPORT.md` is 64 bits (256 positions at ¼ bit), so the index is on the side
where the union is unavoidable, for a random code of the same size. [model: the built code is not random]

**Lemma C4 (any code: the pair bound through the angle between the two errors).** [proved, unrefereed]
For any codebook and any two fixed items y_1, y_2 orthogonal to Δ, let e_i = x_i/cos φ_i − z_i be item i's error
vector in the rotated frame and χ the cosine of the angle between the components of e_1 and e_2 orthogonal to
span(z_1, z_2) (χ = 1 if the two items decode to the same codeword). Then
  Pr[F_1 ∩ F_2 | z_1, z_2] ≤ T_{d−1}( q·√(2/(1 + χ)) ).
Proof. If O is a rotation fixing y_1 and y_2 then Rot·Oᵀ is Haar and gives the same (z_1, z_2), and it sends Δ to
Rot·OᵀΔ. So given (z_1, z_2) the rotated query Δ′ is uniform on the unit sphere of span(z_1, z_2)^⊥, of dimension
d − 1 (Δ ⊥ y_1, y_2, and the y_i are linearly independent). −Δ′·e_i = r_i·W_i with r_i = ‖(e_i)_⊥‖ ≤ ‖e_i‖ = tan φ_i and W_i the coordinate of −Δ′ along (e_i)_⊥.
On F_1 ∩ F_2 both W_i > q, so W_1 + W_2 > 2q, and W_1 + W_2 = √(2 + 2χ)·(a coordinate of a uniform unit vector
of ℝ^(d−1)). This is R7's method with two items in place of two levels. ∎
Size [computed]: the bound is below δ·(γ/N) with γ/N = 10⁻¹¹ when χ ≤ χ₀, χ₀ = 0.52 / 0.45 / 0.34 / 0.33 at
d = 95 / 127 / 255 / 287 (`c2_union.log`, part 3); at χ = 0 it is zero at d = 95 (q√2 > 1) and δ·e^(−58) at d = 255.
So π₂ ≤ δ·γ/N + Pr[F_1 and χ > χ₀]: for any code the whole question is how often two unrelated items' errors are
aligned by more than about a third (a half on the smallest block), on the event that one of them fails. For a random code that is the shared-codeword event of C2(c).

**Gap G3 (the built code).** [sketch: gap named]
For the index's code (scalar quantisers on the rotated coordinates, not a random codebook) I have no bound on
Pr[χ > χ₀ | F_1] at the level 10⁻¹¹. What exists: the measured correlation of two items' normalised errors under a
one-bit sign code, 0.00 at residual cosine 0 and 0.02 at 0.3 (`reports/surge/bounds2_work/pairdep.py`, n = 100,
400,000 rotations), which is a statement about the bulk and says nothing about a tail of 10⁻¹¹. A proof would
bound, for a product code, the chance that two independent uniform points have quantisation-error vectors with
cosine above χ₀; for independent coordinates' rounding errors this is a large-deviation event of probability
e^(−Θ(d)), and e^(−Θ(d)) at d = 95 to 287 against 10⁻¹¹ is the comparison that has to be made with constants.

## C2.3 The Bonferroni step and the statement that results

**Theorem D (the union over N is attained; one block, random code).** [proved, unrefereed]
One block of d degrees of freedom, a shared random codebook of M = 2^B codewords, a shared Haar rotation, the
plain one-sided rule at per-item level δ, N items. If M·sin θ·cos θ ≥ 100·(1 + 2/d)·N·Λ² with
Λ = ln(200·N/δ) and M·Ω_d(θ) = Λ, then there is a corpus of N items, all at the threshold of one query, with
  Pr_ω[the answer is wrong] ≥ N·δ·(1 − 0.01 − N·Λ²·δ/2).
So a guarantee of ε on every corpus forces δ ≤ (ε/N)·(1 + 0.011) when Nδ·Λ² ≤ 10⁻³: the per-item budget is ε/N
and not ε/N′ for any N′ < 0.99·N.
Proof. Theorem C gives a corpus with failure probability at least N·p̄ − (N²/2)·π₂, and p̄ = δ. Lemma C2 with
γ₁ = 0.01 gives N·π₂/δ ≤ 0.01 + N·Λ²·δ + 0.01. Then N·δ − (N/2)·δ·(0.02 + N·Λ²·δ). ∎
With levels: Pr[item fails at some level] ≥ (L − 10⁻⁶)·δ by R7, and Theorem C applies to F_i = ⋃_ℓ F_{i,ℓ}
unchanged, so the factor is N·L. [proved given R7 as stated in `reports/bounds3`; the multi-block version of
Lemma C2 is not written: see below]

**What is and is not covered.**
- Covered: threshold queries; any per-item rule for Theorem C; the plain rule and a random code for Theorem D.
  A rule with a different half-width (A4, the pivot) changes only step (a) of Lemma C2, where the failure
  probability given φ must be at most δ; for A4 at s = 0 it is, so Theorem D holds for it with p̄ ≤ δ replaced
  by its own p̄. [sketch: the constant in (a) not rechecked for the pivot]
- Not covered: several blocks with a random code in each. Theorem C holds as stated. In Lemma C2 the shared-codeword
  event becomes "the same codeword in every coded block" for full alignment, and partial sharing gives χ < 1, to
  which C4's bound applies with the blocks' variance shares. I expect the condition to become
  Π_j M_j ≫ N·Λ² over the coded blocks, which is weaker. [sketch: gap named]
- Not covered: the built code (G3).
- Not covered: k-NN, where an item's decision depends on other items' intervals.

## C2.4 Where the union is not attained: fewer codewords than items

**Claim E (the union over N is avoidable when M is small).** [simulated; the M = 1 case proved]
For M = 1 (one codeword c, τ = −Δ′·c) every item of the equator corpus with cos φ_i > 0 fails as soon as τ ≥ q,
because sin φ_i ≤ 1. So p(Δ′) ≥ ½ on an event of probability T_{d+1}(q), which is of the order of δ, and
π₂ ≥ ¼·T_{d+1}(q): the pair overlap is of order δ, not δ². The items' failures are then nearly one event and
Pr[some item fails] grows far more slowly than N·δ.
Monte Carlo (`c2_union.py` part 4; d = 12, δ = 0.01, N = 20, 100,000 trials, one codebook per M):

| M | p̄ | Pr[some item fails] | effective count Pr/p̄ | Pr[F_2 given F_1] |
|---|---|---|---|---|
| 1 | 0.0050 | 0.019 | 3.9 | 0.455 |
| 2 | 0.0078 | 0.038 | 4.9 | 0.276 |
| 8 | 0.0100 | 0.113 | 11.2 | 0.081 |
| 64 | 0.0099 | 0.175 | 17.6 | 0.012 |
| 512 | 0.0101 | 0.182 | 18.1 | 0.020 |

Independent items give 1 − 0.99²⁰ = 0.182 and an effective count of 18.2: reached at M = 512, missed by a factor
of 5 at M = 1. In general items that decode to the same codeword have nested failure events (for the equator
corpus F_i = {τ_c > q·sin φ_i} with the same τ_c), so a cell's union is its best-coded item's event.
Consequence for R9. R9's claim "N cannot be replaced by anything smaller" needs the hypothesis M ≫ N·ln²(N/δ)
(about 50 bits in the block at N = 10⁹). For a block coded with fewer bits than log₂N + 20 the per-item count N
is not forced by this argument, and Claim E shows it is not forced at all for M = 1. Whether a level with a very
short code can use a smaller count soundly is a design question I have not answered: the effective count for
1 ≪ M ≪ N is not derived. [conjecture: of order M·(a factor polynomial in ln N) nested families]

## C2.5 What this adds to Molinaro, Woodruff and Yaroslavtsev

MWY (SODA 2013, as read by the librarian from the abstract and introduction): in one-way communication, solving k
copies with probability 2/3 needs Ω(k·R_{1/k}(f)) bits, so "solve each copy with error O(1/k) and take a union
bound" is optimal up to a constant factor in the communication. It is a statement of order, for every protocol,
and its consequence for sketches is a dimension or bit count Ω(ε⁻²·log(n/δ)).
What is here that is not there:
1. A constant, for a named architecture. For per-item rules with a shared rotation and a random code, the per-item
   failure budget is ε/N to within one percent (Theorem D), so the confidence constant must rise by exactly
   the amount that divides δ by N: c² grows by 2·ln N to first order, with no smaller constant in front of ln N.
   MWY leave that constant open: Ω(log(n/δ)) allows log(n^(1/10)/δ).
2. Shared randomness. In MWY's model the copies are independent instances. Here all N items share one rotation
   and one codebook, which is the case in which a saving was conceivable. Theorem C shows the dependence is
   carried by the rotated query alone, and Lemma C2 bounds it.
3. A boundary. The union is attained when M ≫ N·ln²(N/δ) and is not when the code has few codewords (Claim E).
   MWY's statement has no such exception because its protocols may be re-randomised per copy.
What MWY has that is not here: all protocols, including decoders that look at several items together and codes
that are not per-item. Theorem C's remark (3) covers joint decoders only for the i.i.d. latitude corpus, where
every item is at the threshold and there is nothing to decide; a lower bound for joint decoders with items on
both sides of the threshold is not attempted.

## C2 summary

- Theorem C: exact conditional independence on a latitude corpus, any code, any per-item rule, any number of
  blocks and levels. [proved, unrefereed; simulated at d = 12]
- Corollary C1: R9 holds iff the pair overlap π₂ of two random threshold items is ≪ p̄/N. [proved, unrefereed]
- Lemma C2, Corollary C3, Theorem D: for one block and a random code, π₂ ≤ 0.02·δ/N from 52 bits up at N = 10⁹,
  ε = 10⁻⁹; so the count N is exact to 1%. [proved, unrefereed; computed]
- Lemma C4: any code, the overlap through the cosine χ of the two errors; harmless below χ₀ = 0.33 to 0.52.
  [proved, unrefereed; computed]
- Claim E: the union is not attained for a one-codeword code; R9 as stated needs M ≫ N·ln²(N/δ). [proved for
  M = 1; simulated for M ≤ 8]
- Open: the built code's tail of χ (G3); several blocks in Lemma C2; the effective count when M ≪ N; k-NN.
