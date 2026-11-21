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
