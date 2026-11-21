# Confidence track: claims

Tags are those of `notes/README.md`. Symbols are those of `notes/00-notation.md`; local symbols are declared here.
Nothing in this file is refereed. Each claim is saved when it is stated; a later section may correct an earlier one,
and says so.

This is the version revised after the referee's report (`referee/referee-lower-bounds.md`). The text the referee
saw is `CLAIMS_before_referee.md`. The last section lists every change. No tag was raised.

## What the track establishes

The question was what the confidence constant must cost. These are the answers, with their limits.

Proved (all [proved, unrefereed] in the revised form; the referee found no displayed inequality wrong):
1. In the class 𝓛, for one block. 𝓛 is: the estimate is the inner product of Δ with a reconstructed point, and the
   half-width is a multiple of ‖Δ‖ that may depend on the message and the seed. In 𝓛 the mean half-width over a
   uniform block is at least cos ε_m·H_{q₀}(θ)·(1 − mδ/T_d(q₀) − M·Ω_d(θ)) for every d and δ (Theorem B). To first
   order, when ln(1/δ) ≫ ln d, that is w* = q/√(4^R − 1 + q²) with q = q_d(δ) the sphere quantile (B4).
2. That bound is met in 𝓛, inside the bit budget, by R8's interval with its half-width frozen at Ê = 0 (A4, A4′).
   At the index's block sizes and δ = 2.5·10⁻¹⁹ the proved lower and upper bounds are 4 to 6% apart (B6 table).
3. The plain rule q·tan φ is valid and is wider than w* by √(1+k), k = q²tan²φ: under 2% from 2 bits per dimension
   up at the ideal angle, 4 to 9% at 1 bit, 28 to 54% at ¼ bit.
4. A converse that averages over queries independent of the item can prove only q·sin θ (N2). The tan comes from
   queries correlated with the item.
5. In one block the plain end, the A4 end and the capped R8 end hold on one event (A5, the referee's).
6. For any per-item rule, any code and any i.i.d. corpus of threshold items, the per-seed failure probability
   p(ω) satisfies Pr_ω[p(ω) ≥ x/N] ≤ ε/(1 − e^(−x)) (Corollary Q, the referee's form of R9).
7. For one block, one level, the plain rule and a random shared codebook, the union over N is attained to 1%
   under a sufficient condition on the code size (Theorem D): 50 to 51 bits per block by the first argument,
   30 to 41 bits by the referee's refinement, at N = 10⁹ and ε = 10⁻⁹.

Refuted or withdrawn:
- "The least half-width is q·(4^R − 1)^(−½)" (the brief's target): it is an upper bound and the k → 0 limit of w*.
- "For any per-item rule the per-item budget is ε/N" (R9 as first stated): false; a rule can put its failures on
  a set of seeds of probability ε. Corollary Q is what holds.
- "The union needs M ≫ N·ln²(N/δ)" and "it fails with fewer codewords than items": withdrawn; the first is only
  sufficient, and the second is contradicted at d = 95.

Sketches and gaps (not proved):
- G1: the full model, where the half-width may depend on the direction of Δ or the estimate is not linear in Δ.
  R8 is such a scheme. No lower bound of any kind is proved outside 𝓛.
- G2: δ fixed as d → ∞ (first-order constant).
- G3: the built scalar code in place of a random codebook.
- The factor L over levels in Theorem D; Theorem D for rules other than the plain one; several blocks.

What this says about the index's constant c ≈ 8 in c·sd, with δ = ε/(N·L):
- It says: for one block coded with a random code and decided by the plain rule, neither part of the constant
  can be removed. The quantile q_d(δ) is forced for half-widths of the plain form up to the factor √(1+k), which
  R8 already has; and δ cannot be taken larger than ε/N by more than 1%.
- It says, for every per-item rule: a rule cannot keep a per-item failure rate above about 1/N on more than a few
  ε of the seeds.
- It does not say that c ≈ 8 is forced for the index as built. The index's levels have up to four blocks, use a
  scalar code, intersect intervals across levels and with the deterministic bound, and need the radial slack;
  all four are outside what is proved. The factor L is a sketch. k-NN is not covered.
- It does not close the question. It closes it for one block in 𝓛 with a random code, and names what is missing
  for the rest. The earlier finding that about 2% of width is left within per-item rules is neither contradicted
  nor proved by this track.

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
(a quantile of h; the worst (y, Δ) of E_ω h). A lower bound on the average w̄ is a lower bound on the supremum over
(y, Δ) of E_ω h. It is not a bound for a single y: Theorem B bounds the mean over y uniform, and a scheme may be
narrow on a set of y of measure up to M·Ω_d(θ). Every later use of "least half-width" means this mean. [referee 1(c)]

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
    q·tan φ·√(1 + k − E₀²/cos²φ)/(1 + k), which is at most q·tan φ/√(1+k). For one block its half-width is never
    larger than (a)'s and it needs no second event (A5). The set is empty when E₀²/cos²φ > 1 + k; then the event has
    failed. The claim is two-sided only: used one-sidedly at q_d(δ), its upper end fails with probability rising to
    2δ for aligned queries (0.0169 at s = 0.9 and 0.0199 at s = 0.99 against δ = 0.01, `c1_revision.log` part 4)
    unless the cap of Theorem 3.11 is applied (end set to 1 when Ê ≥ 1 or the root is not real), with which it is
    at most δ in every row. [referee section 2; reproduced]
Proof. The law of x₀ given y is invariant under rotations fixing y (Lemma 8's argument). (a), (b): substitute.
(c): the pivot (E₀ − cos φ·s)/(sin φ√(1−s²)) = U has a law free of s; solve the quadratic. ∎

**Claim N2 (the PI's averaging cannot give tan φ).** [proved, unrefereed, as a statement about the proof method;
the exact form below is the referee's, section 3]
For any code of the Lemma 3.1 form, if Δ is uniform on S^d and independent of (y, ω), then the shrunk estimate
ŝ = cos φ·E₀ has error Δ·(y − cos φ·x₀), and ‖y − cos φ·x₀‖ = sin φ exactly. So the error is sin φ·U′ with U′ a
coordinate of a uniform unit vector in ℝ^(d+1), independent of φ, and the half-width q_(d+1)(δ)·sin φ·‖Δ‖ fails with
probability exactly δ on average over Δ. No cap-shaped cells and no correction factor are needed (the first
version asked for both).
So any converse that uses only the requirement averaged over independent (y, Δ) proves at most
q_(d+1)(δ)·E sin φ, about q·2^(−R). This is TurboQuant's 4^(−b) against 1/(4^b − 1) (library verdict 3; the
comparison rests on the librarian's summary, which the referee did not check), and it is why their bound has
no −1. The two differ by cos θ_R = √(1 − 4^(−R)): 0.968 at R = 2, 0.866 at R = 1, 0.541 at R = ¼.
That rule is not valid for fixed queries: see the Monte Carlo table in C1.3.

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
So the conjectured least half-width in the full model M4 (G1; nothing is proved outside 𝓛) is the pivot interval's:
  q·tan θ_R·√(1 + k − s²)/(1 + k), k = q²·tan²θ_R, tan θ_R = (4^R − 1)^(−½),
of which the proved statement is its value at s = 0 for the class 𝓛 (Theorem B).
When δ is fixed and d → ∞, q → 0 like z_δ/√d, k → 0 and this is q·tan θ_R, the PI's target, for |s| ≪ 1.
When q stays of order one (the index: q ≈ 8.7/√255 ≈ 0.54, δ = e^(−Θ(d))), k = q²tan²θ is not small at low rates
(k = 0.04 at 2 bits, 0.10 at 1 bit, 0.7 at ¼ bit for d = 255) and the pivot interval is the right benchmark.
Gap: cells are not caps and the decoder is arbitrary. The lemmas below prove the s = 0 value for 𝓛 only.
The referee did not check this posterior computation.

## C1.3 The converse as numbered lemmas

Plan. The converse is proved for a class 𝓛 of schemes that contains the index's rule and every scheme of the
form "reconstruct a point, take its inner product with Δ, report a multiple of ‖Δ‖". The general model M4
(an estimate that is not linear in Δ, a half-width that depends on the direction of Δ) is left as a named gap (G1).

**Class 𝓛.** ŝ = Δ·ŷ(c, ω) for a reconstruction point ŷ(c, ω) ∈ ℝ^(d+1) (any length, any direction), and
h = η(c, ω)·‖Δ‖ with η ≥ 0. The message, the seed and the code are unrestricted. Local symbols, for fixed (y, ω):
e = ŷ − y; a = e·y = ŷ·y − 1 (the bias along y); b = ‖ŷ − (ŷ·y)y‖ (the error across y); ŷ = κ·x with x a unit
vector and κ ≥ 0; φ = angle(x, y) ∈ [0, π] (φ := π if ŷ = 0, where a = −1 and b = 0 whatever φ is; the first version
said π/2, which broke B3 at θ = π/2 with M = 1 [referee 1(b)]). So a = κcos φ − 1, b = κ sin φ.

**What 𝓛 contains and what it leaves out.** [referee section 4; accepted]
In 𝓛: one block with the plain rule (the estimate f·⟨ō, RΔ⟩ is linear in Δ, the half-width c·‖y‖·tan φ·‖Δ‖/√d is
a multiple of ‖Δ‖, and scales rounded up sit inside η(c, ω)); the shrunk-centre interval A4.
Not in 𝓛: (1) the radial slack η·|Ê| required after Lemma 3.1 of chapter 3, which depends on the direction of Δ;
(2) any intersection with the deterministic interval |Δ·y| ≤ ‖Δ‖‖y‖ or with an earlier level's interval, which the
search loops do; (3) R8 (Theorem 3.12), the project's own refereed interval, which uses the same message and is
never wider than A4; (4) several blocks, where the half-width is √(Σ_j η_j²‖Δ_j‖²) and not a multiple of ‖Δ‖.
So the index's own certificate is in 𝓛 only for one block with the plain rule, and Theorem B says nothing about
(2) to (4). Every statement about w* below is a statement about 𝓛, one block.
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
most M such directions, and a message with ŷ = 0 has φ = π and contributes none. ∎

**Theorem B (converse in 𝓛, every d and δ).** [proved, unrefereed]
For every one-sided δ-valid scheme in 𝓛 with M = 2^(Rd) messages, every m ≥ 2, q₀ ∈ (0,1), θ ∈ (0, π/2],
  σ⊗μ{ (y, ω) : η ≥ cos ε_m·H_{q₀}(θ) } ≥ 1 − m·δ/T_d(q₀) − M·Ω_d(θ),
and so E_{y,ω} η ≥ cos ε_m·H_{q₀}(θ)·(1 − m·δ/T_d(q₀) − M·Ω_d(θ)).
A two-sided δ-valid scheme is one-sided δ-valid, so the same holds; see B5 for what two-sidedness adds.
Proof. By B0(ii), √(a² + q₀²b²) ≥ H_{q₀}(φ), which is at least H_{q₀}(θ) when φ ≥ θ. Integrate B2 over y, and remove
the set of B3. ∎

**Corollary B4 (first order).** [proved, unrefereed; the proof of the quantile step is the referee's, section 4]
Let R > 0 be fixed, d → ∞, and δ = δ_d → 0 with ln(1/δ_d)/ln d → ∞. Then for one-sided δ-valid schemes in 𝓛,
  E_{y,ω} η ≥ H_{q_d(δ)}(θ_R)·(1 − o(1)),  sin θ_R = 2^(−R),  i.e.
  η ≳ q·tan θ_R/√(1 + q²·tan²θ_R) = q/√(4^R − 1 + q²),  q = q_d(δ).
Proof. Take m = d, T_d(q₀) = d²·δ, M·Ω_d(θ) = 1/d. Then cos ε_m → 1, the exceptional mass is 2/d, and sin θ → 2^(−R)
by F1. It remains to show H_{q₀}(θ)/H_q(θ) → 1.
For 0 < x < 1, T_d(x) = κ′_d·∫ₓ¹(1 − t²)^((d−3)/2)dt with κ′_d = Γ(d/2)/(√π·Γ((d−1)/2)). Multiplying the integrand
by t ≤ 1, or by t/x ≥ 1, gives (1 − x²)^((d−1)/2)/(d − 1) ≤ ∫ ≤ (1 − x²)^((d−1)/2)/(x(d − 1)). With L(x) = −ln(1 − x²),
ln(1/T_d(x)) = ((d−1)/2)·L(x) + ½·ln d + O(1) − r(x), 0 ≤ r(x) ≤ ln(1/x). At q = q_d(δ) and q₀ = q_d(d²δ):
((d−1)/2)·(L(q) − L(q₀)) ≤ 2 ln d + ln(1/q), and ((d−1)/2)·L(q) ≥ ln(1/δ) − ½ ln d − O(1). Once δ is below a
constant q ≥ c/√d, so ln(1/q) = O(ln d), and 1 − L(q₀)/L(q) = O(ln d/ln(1/δ)) → 0. Since x = √(1 − e^(−L)) has
d ln x/d ln L = L/(2(e^L − 1)) ≤ ½, ln(q/q₀) ≤ ½·ln(L(q)/L(q₀)) → 0, for every size of q including q → 1. And
∂ ln H_q/∂ ln q = cos²θ/(cos²θ + q²sin²θ) ≤ 1, so H_{q₀}(θ)/H_q(θ) ≥ q₀/q → 1. ∎
B4 is a limit statement. At the index δ = 2.5·10⁻¹⁹ (ln(1/δ) = 42.8) and ln(1/δ)/ln d is 9.4 at d = 95 and 7.7 at
d = 255; there q_d(d²δ)/q_d(δ) ≈ √(1 − 2 ln d/ln(1/δ)) and the choice of B4's proof is off by 8 to 15%
(the referee's figures). Numbers at the index's sizes come from B6, not from B4. (The first version quoted
δ ≈ 10⁻¹³ here; that was a slip.)

**Claim A4 (Theorem B is met inside 𝓛 by R8's interval with its half-width frozen at Ê = 0).**
[proved, unrefereed; simulated by the author and by the referee]
In the scheme of A1 (random or covering code, Haar rotation; the code a function of Rot·y alone and φ the true
angle, Lemma 3.1's conditions), with φ < π/2 stored as a real number, take
  ŷ = κ*·x₀, κ* = cos φ/(cos²φ + q²sin²φ) = 1/(cos φ·(1+k)),  η = H_q(φ) = q·tan φ/√(1+k),  k = q²tan²φ.
For φ ≥ π/2 take ŷ = 0, η = 1. With q = q_d(δ) it is one-sided δ-valid for every (y, Δ). Conditionally on φ the
failure probability equals δ at s = Δ·y/‖Δ‖ = +√(k/(1+k)) and is smaller elsewhere; because that s depends on φ,
for a fixed query and a code whose φ varies the unconditional failure probability is below δ at every s
(0.0090 to 0.0093 against 0.01 in the runs). With q = q_d(δ/2) it is two-sided δ-valid.
Proof. x₀ = cos φ·y + sin φ·v′, v′ uniform on the unit sphere of y^⊥ and independent of φ (N1). So
e = a·y + b·v′ with a = κ*cos φ − 1 = −k/(1+k), b = κ*sin φ = tan φ/(1+k). For a unit Δ = s·y + √(1−s²)·v
(v fixed), Δ·e = s·a + √(1−s²)·b·U. The one-sided failure probability given φ is
T_d((η + s·a)/(√(1−s²)·b)) ≤ T_d(q), because η = √(a² + q²b²) ≥ −s·a + q√(1−s²)·b (B0). Two-sided: add the two
tails. ∎

**What A4 is and is not.** [referee section 2; accepted in full]
(1) A4 is not a new interval. For one block R8 (chapter 3's Theorem 3.12 with ξ² = k, or Theorem 3.11(ii)) has
centre Ê/(1+k) and half-width q·tan φ·√(1 + k − Ê²)/(1+k), Ê = E₀/cos φ. A4 has the same centre and the half-width
with Ê² dropped under the root. So A4's interval contains R8's for every outcome: it is R8 widened to its
half-width at Ê = 0, which is the one change that puts it in 𝓛. Chapter 3 already records "at Ê = 0 it is the
plain width divided by √(1 + ξ²)". What is new here is only that this member of 𝓛 meets Theorem B.
(2) "Narrower than the plain rule by √(1+k)" is a statement about half-widths, the functional of M6. It is not a
statement about the bound an exclusion uses. A4's upper end Ê/(1+k) + √(k/(1+k)) is above the plain upper end
Ê + √k exactly when Ê < −√k·√(1+k)/(1 + √(1+k)), about −½·q·tan φ. In `c1_revision.log` part 4 that happens in
13% of draws at s = 0, 50% at s = −0.3 and all draws at s = −0.9 (the referee: 13 to 99% over seven codes).
So A4 does not dominate the plain rule as a one-sided bound, and neither should be used alone.

**Claim A5 (one block: one event serves the plain rule, A4 and R8 as repaired).**
[proved, unrefereed; the statement and proof are the referee's, section 2; simulated]
Units ‖y‖ = ‖Δ‖ = 1, one block, real φ < π/2. Let E₁ = {s − Ê ≤ √k·√(1 − s²)}, which is the event {−U ≤ q} of
probability 1 − δ given φ. On E₁ all three hold:
 plain: s ≤ Ê + √k;
 A4: s ≤ Ê/(1+k) + √(k/(1+k));
 R8 with the cap of Theorem 3.11: s ≤ 1 if Ê ≥ 1 or if 1 + k − Ê² < 0, and otherwise
 s ≤ min(1, (Ê + √k·√(1 + k − Ê²))/(1 + k)).
So the smallest of the three upper ends is a one-sided bound that costs one event, not three.
Proof. Plain: √(1−s²) ≤ 1. A4: s·(1+k) − Ê = (s − Ê) + k·s ≤ √k·(√(1−s²) + √k·s) ≤ √k·√(1+k) by Cauchy–Schwarz.
R8: Theorem 3.11's repaired statement of the upper end, which is proved on this same event. ∎
(Chapter 3's remark that R8 and the plain interval have different events is about several blocks, where the
shares are unknown; it does not apply to one block.)
Simulated (`c1_revision.log` part 4, d = 12, 512 codewords, δ = 0.01, 400,000 draws per row): the smallest of the
three ends is exceeded with rate 0.0097 to 0.0102 at s = 0, ±0.3, 0.6, 0.9, 0.99 and 0.0001 at s = −0.9.

**Claim A4′ (A4 inside the bit budget).** [proved, unrefereed; the repair is the referee's 1(a), checked by hand]
A4 as stated stores the real number φ and is outside M2; and rounding φ up is not safe by itself, because the bias
a moves by (cos φ/cos φ̂ − 1)/(1 + k̂), which H_q(φ̂) does not cover. Repair, as in A2: store ĉ ≤ cos φ on a
geometric grid of ratio 1 + η_g, so g = cos φ/ĉ ∈ [1, 1 + η_g]; put φ̂ = arccos ĉ ≥ φ, k̂ = q²tan²φ̂,
κ̂ = 1/(ĉ·(1 + k̂)), ŷ = κ̂·x₀ and η = H_q(φ̂) + η_g. This is one-sided δ-valid at q = q_d(δ).
Proof. a = κ̂cos φ − 1 = (κ̂ĉ − 1) + (g − 1)·κ̂ĉ with κ̂ĉ = 1/(1 + k̂) ≤ 1, so a = â + ζ with â = −k̂/(1+k̂) and
0 ≤ ζ ≤ η_g. b = κ̂ sin φ ≤ κ̂ sin φ̂ = b̂. For every s, −s·a + q√(1−s²)·b ≤ −s·â + q√(1−s²)·b̂ + η_g ≤
√(â² + q²b̂²) + η_g = H_q(φ̂) + η_g by B0(i). So the argument of T_d in the proof of A4 is at least q. ∎
With η_g = d⁻² the grid costs O(log d) bits, as in A2, and H_q(φ̂) ≤ H_q(φ)·(1 + O(d⁻¹)) on the event of A3. So
"Theorem B is met at B = Rd + O(log d) bits" stands for A4′, not for A4 as first written.

Consequence for 𝓛. Together with Theorem B and A3: among schemes in 𝓛, for one block, the least mean half-width
at rate R is, to first order (B4's regime),
  w*(R, δ, d) = q/√(4^R − 1 + q²),  q = q_d(δ)   (one-sided),
which is the PI's target q·(4^R − 1)^(−½) divided by √(1 + k). The target is the k → 0 limit. w* is the least
half-width in 𝓛, not the least half-width: R8, with the same message, is never wider and is outside 𝓛.
Size of k for the index (q ≈ c/√d with c ≈ 7.4 to 8.7; ideal tan²θ_R = 1/(4^R − 1)): see `c1_converse.log`.

**Lemma B5 (one-sided against two-sided).** [proved, unrefereed; (iii) corrected after the referee]
(i) A two-sided δ-valid scheme in 𝓛 satisfies Theorem B with δ, and at s = 0 the stronger
E_ω T_d(η/b) ≤ δ/2 (both tails are equal there). (ii) A4 with q_d(δ/2) is two-sided δ-valid. So the two-sided
optimum in 𝓛 lies between w* at q_d(δ) and w* at q_d(δ/2). (iii) A one-sided scheme run on Δ and on −Δ is a
two-sided scheme at 2δ, since ŝ is odd and h even in Δ for 𝓛. So one side costs at most one halving of δ.
It is exactly one halving only as k → 0. For k not small the two tails of A4 peak at s = +√(k/(1+k)) and
s = −√(k/(1+k)), and at either the other tail is negligible: the largest sum of the two tails over s, in units of
T_d(q), is 1.0000 at d = 95 (k = 0.19 and 1.38) and at d = 255, k = 0.09; 1.035 at d = 255, k = 0.018; 1.91 at
d = 255, k = 0.001 [computed, `c1_revision.log` part 3; the referee's `ref_num1.py` gives 1.94 for the last]. So for
k ≥ 0.09 A4 at q_d(δ) is already two-sided δ-valid to four digits and the two optima in 𝓛 coincide. The effect is
never more than q_d(δ/2)/q_d(δ) − 1 ≈ ln 2/(d·q²) (0.5% at d = 255, q = 0.54).
Proof. (i) At s = 0 the error is b·U, symmetric. (ii) A4. (iii) Linearity; the figures are computed. ∎

**Gap G1 (the general model).** [sketch: gap named]
Outside 𝓛 the decoder may use a half-width that depends on the direction of Δ and an estimate that is not linear
in Δ. What is proved for it: for fixed (ω, c) with cell K = enc(·, ω)⁻¹(c), the interval [ŝ − h, ŝ + h] contains
Δ·y for every y ∈ K on which the scheme does not fail, so 2h(c, Δ, ω) is at least the width, in direction Δ, of
the non-failing part of K. What is missing: a lower bound on the average over Δ of that trimmed width for an
arbitrary measurable K of measure 1/M. For caps the posterior computation of N3 gives the pivot interval N1(c).
For general K one needs a "trimmed Urysohn inequality": among sets of given measure, the cap minimises the mean
over directions of the width that holds all but a δ-fraction of the mass. Urysohn's inequality is the case δ = 0
in Euclidean space; I know no trimmed form and did not find a proof.
The right prior, without a heuristic step (the referee's remark, section 7): for a scheme that starts with a Haar
rotation, "for every y" is, in the rotated frame, an average over z = Rot·y uniform. So M5 is exactly the family
of Bayes problems "Δ′ uniform, z uniform on the latitude s of Δ′", one for each s ∈ [−1, 1]. That is N3's prior.
Conjecture: the least average half-width in the full model is that of the pivot interval,
q·tan θ_R·√(1 + k − s²)/(1 + k) at Δ·y = s, which equals w* at s = 0. [conjecture]
What is known meanwhile, and no more: one scheme outside 𝓛, the pivot interval (R8 for one block), has a
half-width below 𝓛's bound, through its dependence on ŝ, by the factor √(1 − ŝ²/(1+k)). Whether that is the only
way to beat 𝓛, and whether nothing is gained when |s| ≪ 1, are the conjecture itself; the first version called
both "certain", which they are not. No bound of any kind is proved here for the full model, including the weaker
q·2^(−R) form. Nothing tagged proved depends on G1 as long as w* is read as "in 𝓛".

**Corollary B6 (Theorem B integrated; the form used for numbers).** [proved, unrefereed]
For ρ ∈ (0,1), u ∈ (0,1] let x(u) = q_d(mδ/(ρu)) (0 if mδ/(ρu) ≥ ½) and θ(u) solve M·Ω_d(θ(u)) = (1−ρ)u. Then
  E_{y,ω} η ≥ cos ε_m·∫₀¹ H_{x(u)}(θ(u)) du.
Proof. By Theorem B with q₀ = x(u), θ = θ(u), the event η < cos ε_m·H_{x(u)}(θ(u)) has probability at most u, and
the right side decreases as u decreases. So u ↦ cos ε_m·H_{x(u)}(θ(u)) lies below the quantile function of η. ∎

**Numbers [computed] (`c1_converse.py` part 3; `c1_revision.py` part 1; the logs beside them).** One-sided,
δ = 10⁻⁹/(10⁹·4) = 2.5·10⁻¹⁹, one block, in 𝓛. "Plain" is q·tan θ_R at the ideal angle; w* = q/√(4^R − 1 + q²);
"mean" is the exact mean of H_q(φ) for A4 with a random code, Pr[φ > θ] = (1 − Ω_d(θ))^M, which is the proved
upper bound (real φ; A4′'s grid adds O(1/d)); "B6" is the proved lower bound as a rigorous left sum of the
non-decreasing integrand on 400 cells, maximised over m ∈ {5, 9, 17, 33}, ρ ∈ {.5, .8, .95}.

| d | R | q_d(δ) | k = q²tan²θ_R | plain | w* | mean (upper) | B6 (lower) | B6/mean | B6/w* |
|---|---|---|---|---|---|---|---|---|---|
| 95 | ¼ | 0.7565 | 1.381 | 1.1754 | 0.7616 | 0.7852 | 0.7550 | 0.962 | 0.991 |
| 95 | 1 | 0.7565 | 0.191 | 0.4367 | 0.4002 | 0.4122 | 0.3908 | 0.948 | 0.976 |
| 95 | 2 | 0.7565 | 0.038 | 0.1953 | 0.1917 | 0.1972 | 0.1863 | 0.945 | 0.972 |
| 95 | 7 | 0.7565 | 0.000 | 0.0059 | 0.0059 | 0.0061 | 0.0057 | 0.944 | 0.970 |
| 255 | ¼ | 0.5186 | 0.649 | 0.8058 | 0.6275 | 0.6405 | 0.6106 | 0.953 | 0.973 |
| 255 | 1 | 0.5186 | 0.090 | 0.2994 | 0.2868 | 0.2910 | 0.2743 | 0.943 | 0.956 |
| 255 | 2 | 0.5186 | 0.018 | 0.1339 | 0.1327 | 0.1344 | 0.1265 | 0.941 | 0.953 |
| 255 | 7 | 0.5186 | 0.000 | 0.0041 | 0.0041 | 0.0041 | 0.0039 | 0.941 | 0.952 |

Changes from the first version of this table [referee section 4; both reproduced]. The first "achievable" column
was H_q at the angle with M·Ω = 2 ln d, a safe overestimate (0.4260 and 0.6565 in the rows where the exact mean is
0.4122 and 0.6405). The first B6 column was a midpoint rule, 0.2% above the rigorous left sum.
Reading. At the index's block sizes the proved lower bound in 𝓛 is 4 to 6% below the proved upper bound, and 1 to
5% below the first-order formula w*. The rest is the finite-d cost of the union over the grid of s (2 to 3%) and
of the integral over the steep tail (about 1%). The plain half-width q·tan θ exceeds w* by √(1+k): 54% at ¼ bit
and 9% at 1 bit on the block of 96; 28% and 4% on the block of 256; under 2% from 2 bits up. These compare
half-widths, for one block alone at the ideal angle, inside 𝓛. They are not savings for the index: its search
intersects intervals and uses several blocks, both outside 𝓛, and by A5 the plain end and the A4 end should be
used together. The built code's tan is 1.3 to 1.8 times the ideal (`codes/REPORT.md`), which makes k larger.

**Monte Carlo [simulated] (`c1_revision.py` part 4; d = 12, M = 512 random codewords, δ = 0.01, 400,000 draws per
row, standard error 0.00016).** One-sided rates of "truth above the upper end". "Uniform" is Δ uniform and
independent of y (the averaged requirement); the other rows are fixed queries at s = Δ·y. "Naive" is centre
cos φ·E₀ with half-width q_(d+1)(δ)·sin φ, the rule the averaged requirement allows (N2). The first version's
"naive" column used half-width q·sin φ·cos φ, which is not that rule and fails on average too; it is replaced.

| s | plain (A1) | naive (N2) | A4 | pivot, no cap | pivot with cap | smallest of three (A5) | A4 end above plain end |
|---|---|---|---|---|---|---|---|
| uniform | 0.0074 | 0.0098 | 0.0031 | 0.0098 | 0.0097 | 0.0097 | 22% |
| 0 | 0.0102 | 0.0006 | 0.0024 | 0.0102 | 0.0102 | 0.0102 | 13% |
| 0.3 | 0.0064 | 0.0182 | 0.0075 | 0.0100 | 0.0100 | 0.0100 | 0.4% |
| 0.6 | 0.0005 | 0.1198 | 0.0090 | 0.0100 | 0.0100 | 0.0100 | 0% |
| 0.9 | 0.0000 | 0.4825 | 0.0000 | 0.0169 | 0.0099 | 0.0099 | 0% |
| 0.99 | 0.0000 | 0.7802 | 0.0000 | 0.0199 | 0.0098 | 0.0098 | 0% |
| −0.3 | 0.0068 | 0.0000 | 0.0001 | 0.0102 | 0.0102 | 0.0102 | 50% |
| −0.9 | 0.0000 | 0.0000 | 0.0000 | 0.0031 | 0.0001 | 0.0001 | 100% |

The naive rule meets the averaged requirement (0.0098) and fails fixed queries: 1.8% at s = 0.3, 12% at 0.6, 48%
at 0.9, 78% at 0.99. So N2 is not an artefact. A4 never exceeds δ. The pivot's upper end without the cap reaches
2δ for aligned queries and is at most δ with it. The smallest of the three ends never exceeds δ beyond noise.
Mean half-widths from the first run (`c1_converse.log` part 1): plain 0.581, A4 0.499, pivot 0.485 at s = 0 falling
to 0.347 at s = 0.8, at mean k = 0.35. The referee's `ref_sim_a4.py` covers seven codes and 63 cells with the same
conclusions.

**Steepness [computed] (part 2).** The factor T_d(t·q)/δ by which the failure probability rises when a half-width
is cut to t times the quantile, at δ = 2.5·10⁻¹⁹: for t = 0.98 / 0.95 / 0.90 it is 11.5 / 334 / 46,000 at d = 95 and
6.4 / 92 / 5,900 at d = 255. Conversely q_d(100δ)/q_d(δ) = 0.961 (d = 95) and 0.949 (d = 255).

**Gap G2 (δ fixed as d → ∞).** [sketch: gap named]
Corollary B4 needs ln(1/δ) ≫ ln d. For δ fixed, Theorem B still gives a bound of the right order (take m = 9,
T_d(q₀) = 270·δ, which needs δ < 1/540: a constant factor below w*). The first-order constant is not proved. What
is exact: at s = 0, E_ω T_d(η/b) ≤ δ, and T_d is convex on [0, ∞) for d ≥ 3 (it is concave on [0, 1] for d = 2), so
E_ω[η/b] ≥ q_d(δ) for every y (Jensen). What resists: b = κ sin φ must be bounded below by tan φ·(1 − o(1)), which
needs κcos φ ≥ 1 − o(1); validity at s ≠ 0 gives that only outside an event of probability of order δ, and an event
of that size moves the quantile at first order when δ is fixed. I did not find the right joint treatment of the
two budgets. The index is not in this regime.

**The confidence tax.** [proved, unrefereed, given A4′ and Theorem B; in 𝓛, one block, one-sided, first order
in B4's regime]
To reach mean half-width w (in units of ‖y‖‖Δ‖) a one-sided δ-certificate in 𝓛 needs, to first order,
  R_cert(w) = ½·log₂(1 + q²·(1 − w²)/w²) bits per degree of freedom, q = q_d(δ)   (from w = q/√(4^R − 1 + q²)).
To make the unbiased estimate's standard deviation tan θ_R/√d equal to w needs R_sd(w) = ½·log₂(1 + 1/(d·w²)).
The tax is R_cert − R_sd = ½·log₂[(w² + q²(1 − w²))/(w² + 1/d)].
Its limit log₂(q·√d) = log₂ c needs w ≪ 1/√d (the first version said w ≪ q, which is wrong [referee section 5]):

| d | c = q√d | 1/√d | limit | w = 0.3 | w = 0.1 | w = 0.03 | w = 0.01 |
|---|---|---|---|---|---|---|---|
| 95 | 7.37 | 0.103 | 2.88 | 1.30 | 2.41 | 2.82 | 2.88 |
| 255 | 8.28 | 0.063 | 3.05 | 0.92 | 2.16 | 2.90 | 3.03 |
| 767 | 8.69 | 0.036 | 3.12 | 0.49 | 1.63 | 2.75 | 3.07 |

[computed, `c1_revision.log` part 2.] At the limit a block of 256 positions pays about 97 bytes for confidence.
What the tax says and does not say. It is the statement that an interval of c standard deviations needs a
standard deviation c times smaller, at 6 dB per bit, with the exact form of the saturation at coarse widths.
It is a first-order statement for 𝓛; at the index's sizes the proved bounds bracket w* within about ±5%, which is
±0.07 bit per degree of freedom. It is not a statement about what any certificate must cost: outside 𝓛 (R8,
intersections with other intervals, several blocks) nothing is proved (G1).

**C1 summary: what is settled and what is not.**
- Scope of every line: one block, one item, one query; the class 𝓛; the mean over y uniform and ω.
- Message-dependent half-width (the stored scale): allowed in 𝓛 (η is any function of (c, ω)); it does not change
  the first-order answer. [proved, unrefereed]
- Dependence of h on the direction of Δ, and estimates not linear in Δ: not covered (G1). R8 is such a scheme, uses
  the same message, and is never wider than A4. [proved for that scheme, chapter 3]
- Biased against unbiased: the least mean half-width in 𝓛 is attained neither by the centre shrunk by cos²φ
  (valid on average over independent queries, invalid for fixed ones: N2, simulated) nor by the unbiased centre
  (valid, half-width √(1+k) larger). It is attained by the centre shrunk by 1/(1+k) with half-width
  q·tan φ/√(1+k), which is R8 with its half-width frozen at Ê = 0 (A4, A4′). An averaged requirement would allow
  sin; this is the difference between TurboQuant's 4^(−b) and 1/(4^b − 1). [proved, unrefereed; simulated]
- One-sided against two-sided: at most one halving of δ, and none when k ≥ 0.09 at d = 255 (B5).
  [proved, unrefereed; computed]
- Constants: w* = q_d(δ)/√(4^R − 1 + q_d(δ)²) is the least mean half-width in 𝓛 to first order when
  ln(1/δ) ≫ ln d (B4); at the index's block sizes the proved lower and upper bounds are 4 to 6% apart (B6 table).
  [proved, unrefereed; computed]
- The PI's target q·(4^R − 1)^(−½) is the k → 0 limit of w*. It is an upper bound (Theorem A) and is not the
  least half-width in 𝓛.
- One block, one event: the plain end, the A4 end and the capped R8 end hold together on one event (A5), so their
  minimum is a valid one-sided bound. A4's end alone does not dominate the plain end.
- Which queries force the bound. Theorem B uses queries with s = Δ·y/(‖Δ‖‖y‖) up to the worst value √(k/(1+k))
  (0.13 at k = 0.018, 0.29 at k = 0.09, 0.63 at k = 0.65). A guarantee asked only for queries with |s| ≤ s_max
  below that value would allow a centre shrunk further and a narrower interval; the bound with a restricted s is
  B0(i) with the maximum taken over |s| ≤ s_max, and is not worked out here. A typical offset has |s| of order
  1/√d, where R8's gain over A4 is O(1/d), but the guarantee is for every query. [remark]
- Low rates. Where q·tan θ_R > 1 (¼ bit) the plain half-width exceeds the deterministic bound |Δ·y| ≤ ‖Δ‖‖y‖. An
  interval cut at ±‖Δ‖‖y‖ is outside 𝓛, and this is exactly where w* is furthest from the plain rule (w* is 0.76
  and 0.63 of the deterministic bound at d = 95 and 255). So the low-rate rows say least about the index.
- Open: G1 (the full model), G2 (fixed δ), several blocks.

# C2. The union over N items (R9)

## C2.0 The statement, corrected after the referee

Setting: the index's randomised levels, any number of blocks and levels. Block j of every item in a group is
rotated by the same Haar rotation Rot_j, and any codebook is shared; ω is the whole shared seed. A **per-item
rule** is any rule under which item i's decision at each level (exclude, include, read on) is a function of item
i's own stored messages, the seed ω and the query. Item i **fails** (event F_i) if some level excludes it although
it matches or includes it although it does not.

**R9 as first stated here is false.** [refuted; the counter-example is the referee's, section 6]
The first version claimed: for any per-item rule, an answer exact with probability 1 − ε on every corpus of N
items forces the mean per-item failure probability p̄ below (ε/N)(1 + o(1)). Counter-example: fix a set S of
rotated queries with Pr[Δ′ ∈ S] = ε; the rule excludes every item when Δ′ ∈ S and otherwise reads every item to the
end. It is a per-item rule. The answer is wrong with probability exactly ε on every corpus that has a match, and
p̄ = ε, not ε/N. So "no per-item rule can avoid the union over N" is not a theorem: the union is avoided by any
rule that makes items fail together. Such a rule gains nothing by it, and the statement that survives says so:

**R9 restated (Corollary Q below).** For every per-item rule and every i.i.d. corpus of threshold items, the
per-seed failure probability p(ω) satisfies Pr_ω[p(ω) ≥ x/N] ≤ ε/(1 − e^(−x)) for every x > 0. A rule cannot have a
per-item failure rate above about 1/N on more than a few ε of the seeds; it can only move its failures onto few
seeds. When p(ω) is nearly constant in ω, as for the plain rule with a random code (Theorem D), this is
p̄ ≤ (ε/N)(1 + o(1)) and the union over N is attained.

## C2.1 The construction and conditional independence

**Theorem C (independence given the seed; exact).** [proved, unrefereed; statement repaired after the referee;
simulated by the author and by the referee]
Let the N items be drawn independently from any one law, independently of ω. For any codebooks, any shared
rotations, any number of blocks and levels and any per-item rule, conditionally on ω the events F_1, …, F_N are
independent with a common probability p(ω). Hence, averaged over the random corpus,
  Pr[some item fails] = E_ω[1 − (1 − p(ω))^N],  and
  N·p̄ − (N(N−1)/2)·π₂ ≤ Pr[some item fails] ≤ N·p̄,  p̄ = E p(ω),  π₂ = E p(ω)² = Pr[F_1 ∩ F_2].
So some fixed corpus of N items has Pr_ω[some item fails] ≥ N·p̄ − (N²/2)·π₂.
Proof. Given ω, F_i is a function of item i alone, and the items are i.i.d. and independent of ω. The lower bound is
Bonferroni's second inequality, or (1−p)^N ≤ 1 − Np + N(N−1)p²/2. An average over corpora is attained by some
corpus. ∎
This is elementary, and no latitude is needed for it. The first version conditioned on Δ′ = (Rot_j Δ_j)_j alone
and said "for any codebooks and any per-item rule". That is false in two cases the text allowed [referee section
6]: a codebook that is random and shared (with Δ′ held fixed and one random codeword shared by two items,
Pr[F_1 ∩ F_2 | Δ′] is 83 times the product, `ref_sim_c.py` part B), and a rule that uses the rotations other than
through Δ′. The repair is to condition on ω, or on (Δ′, codebooks) for rules that use the rotations only through
Δ′. π₂ = E p(Δ′, C)² is what Lemma C2 computes, so nothing downstream changes.

**Construction (what the latitude adds).** Fix the query, hence the offsets Δ_j per block, a threshold t, and per
block a norm ρ_j and a block score s_j with |s_j| ≤ ρ_j‖Δ_j‖ and Σ_j s_j = t − (the part of the score read before
the randomised levels, taken equal for all items; the corpus must be free to place blocks anywhere on the
latitude with one common prefix read). Draw each item's block j independently and uniformly from the latitude
Λ_j = {y : ‖y‖ = ρ_j, Δ_j·y = s_j}. Then (1) every item has score exactly t: all N items are matches at margin
zero, and an interval that fails on the upper side at any level excludes a match, so F_i implies a wrong answer;
(2) Rot_j maps Λ_j onto the latitude of Δ′_j, so for a rule that uses the rotations only through Δ′, p depends on
ω only through (Δ′, codebooks).
Simulated: the author's `c2_union.py` part 4 (one block, 1 to 512 codewords); the referee's `ref_sim_c.py`, where
a fixed query, a corpus on its latitude and one Haar matrix per trial agree with the model in 16 one-block cells
and in a two-block, two-level case (Pr[at least one failure] 0.6020 real against 0.6014 model).

**Corollary Q (R9 in quantile form).** [proved, unrefereed; the statement is the referee's, the proof is two lines]
Suppose the answer is wrong with probability at most ε on every corpus of N items. Take any i.i.d. corpus whose
items are at the threshold (so that F_i implies a wrong answer). Then for every x > 0,
  Pr_ω[p(ω) ≥ x/N] ≤ ε/(1 − e^(−x)).
For example p(ω) ≤ 0.7/N outside a set of seeds of probability 2ε, and p(ω) ≤ 3/N outside one of probability 1.06ε.
Proof. Given ω, Pr[some item fails] = 1 − (1 − p(ω))^N ≥ 1 − e^(−N·p(ω)), which is at least 1 − e^(−x) when
p(ω) ≥ x/N. Average over ω and over the corpus; the average is at most ε. ∎
It holds for every per-item rule, every code, any number of blocks and levels. It is consistent with the
counter-example above, where p(ω) = 1 on a set of probability ε.

**Corollary C1 (the mean form).** [proved, unrefereed]
Under the same hypothesis, p̄ ≤ ε/N + (N/2)·π₂. If π₂ ≤ γ·p̄/N for some γ < 2 then p̄ ≤ (ε/N)/(1 − γ/2). In words:
the union over N is attained up to the factor 1 − γ/2 as soon as the chance that a second random threshold item
fails, given that a first one does, is at most γ/N. Independence would give Pr[F_2 | F_1] = p̄ ≈ ε/N, smaller than
1/N by the factor ε.
Two limits on what this says [referee section 6]. It bounds the mean p̄ over a random latitude item; R9's "for
every scale vector that occurs" cannot be had, because an item's scales are random through φ. And
p̄ = Σ_ℓ T(a_ℓ; h_ℓ) (up to the level overlaps of R7) needs every block on its equator, s_j = 0; off it the true
scale is A_j‖P_{y⊥}Δ_j‖ < a_j and p̄ is smaller.

## C2.2 The pairwise bound

Both lemmas are for one block, one level, the equator corpus (s = 0, ρ = 1, ‖Δ‖ = 1: items uniform on the unit
sphere of Δ^⊥) and the plain one-sided rule of A1 with exact φ, h = q·tan φ with q = q_d(δ), so p̄ = δ up to
Pr[φ ≥ π/2] ≤ 2^(−M). In the rotated frame, with x_i the codeword of item i and φ_i its angle,
F_i = {−Δ′·x_i > q·sin φ_i} (and φ_i < π/2).

**Lemma C2 (random codebook: π₂ in closed form).** [proved, unrefereed; step (c′) is the referee's refinement,
itself unrefereed, checked here by computation and by Monte Carlo]
Let the codebook be M independent uniform points of S^d, shared by all items and independent of the rotation.
For every θ ∈ (0, π/2), with Λ = M·Ω_d(θ),
  π₂ ≤ 2·e^(−Λ) + Λ²·δ² + M·δ·J(θ),
where either of the following may be used for J:
  (c)  J(θ) = Ω_d(θ)·Ω_{d−1}(θ) ≤ Ω_d(θ)²·(1 + 2/d)/(sin θ·cos θ);
  (c′) J(θ) = I(θ) = ∫₀^θ κ_d·sin^(d−1)φ·T_d( cos θ/√(1 − q²·sin²φ) ) dφ, which is smaller than (c) by about
       ((1 − q²)/(1 − q²sin²θ))^((d−1)/2).
So N·π₂/δ ≤ 2N·e^(−Λ)/δ + N·Λ²·δ + N·M·J(θ).
Proof. For a codeword c let G_i(c) = {angle(c, z_i) ≤ θ and −Δ′·c > q·sin angle(c, z_i)}. Then
F_i ⊂ {φ_i > θ} ∪ ⋃_c G_i(c), and Pr[φ_i > θ] = (1 − Ω_d(θ))^M ≤ e^(−Λ) (A3).
(a) One codeword, one item: given z_i and the angle φ ≤ θ of c from z_i, c = cos φ·z_i + sin φ·v with v uniform
on the unit sphere of z_i^⊥; Δ′ ⊥ z_i, so Δ′·c = sin φ·U with U a coordinate of a uniform unit vector in ℝ^d, and
Pr[G_i(c) | φ] = T_d(q) = δ. So Pr[G_i(c)] = Ω_d(θ)·δ.
(b) Two codewords c ≠ c′: given Δ′, (z_1, c) and (z_2, c′) are independent, so
Σ_{c≠c′} Pr[G_1(c) ∩ G_2(c′)] ≤ M²·Ω_d(θ)²·δ² = Λ²δ².
(c) One codeword for both: Pr[G_1(c) ∩ G_2(c)] ≤ Pr[G_1(c) and angle(c, z_2) ≤ θ]. Given (c, Δ′, z_1), z_2 is
uniform on the unit sphere of Δ′^⊥ and z_2·c = √(1 − τ²)·U with τ = −Δ′·c, so
Pr[z_2·c ≥ cos θ | c, Δ′, z_1] = T_d(cos θ/√(1 − τ²)) ≤ T_d(cos θ) = Ω_{d−1}(θ). With (a), the sum over c is at most
M·Ω_d(θ)·δ·Ω_{d−1}(θ).
(c′) The same line without throwing τ away: on G_1(c), τ > q·sin φ_1, and T_d(cos θ/√(1 − τ²)) decreases in τ, so
Pr[G_1(c) and angle(c, z_2) ≤ θ] ≤ E[1_{G_1(c)}·T_d(cos θ/√(1 − q²sin²φ_1))] = δ·I(θ), using (a) for each φ_1 and the
density κ_d sin^(d−1)φ of the angle of c from z_1.
The comparison of caps in (c) is F1 in dimensions d − 1 and d, with κ_{d−1}·d/(κ_d·(d−1)) ≤ 1 + 2/d:
κ_{d−1}/κ_d = [Γ(d/2)/Γ((d−1)/2)]²·2/(d − 1) ≤ 1 by Wendel's inequality Γ(x + ½) ≤ √x·Γ(x) at x = (d−1)/2, and
d/(d − 1) ≤ 1 + 2/d for d ≥ 2 (the referee's proof; the first version had only a numerical check and d ≥ 4). ∎
Check of (c′) [computed; simulated, `c2_revision.py`]: the event's probability for one uniform codeword is 0.83 to
0.96 of δ·I(θ) in four settings (d = 6, 12, 24; δ = 0.01, 0.002; 10 to 20 million draws; 789 to 6,730 hits) and
0.50 to 0.84 of the bound (c); it never exceeds (c′).

**Corollary C3 (a sufficient condition for γ = N·π₂/δ to be small; random code).** [proved, unrefereed; computed]
Take Λ = ln(2N/(γ₁δ)) and θ with M·Ω_d(θ) = Λ. Then γ ≤ γ₁ + N·Λ²·δ + N·M·J(θ).
For N = 10⁹, ε = 10⁻⁹, L = 4 entering only through δ = 2.5·10⁻¹⁹, and γ₁ = 0.01: Λ = 68.9 and the middle term is
1.2·10⁻⁶. The last term is below 0.01 from

| step used | d = 95 | d = 127 | d = 255 | d = 287 |
|---|---|---|---|---|
| (c), first version | 50 bits | 50 | 51 | 51 |
| (c′), refined | 30 bits | 33 | 40 | 41 |

(`c2_union.log` part 2 and `c2_revision.log` part 1; the referee's `ref_num2b.py` gives the same four refined
figures.) With (c′) at d = 95 and 29 bits, where M = 5.4·10⁸ is less than N, the last term is 0.026.
These are sufficient conditions, and the first row is the slack of step (c), not a boundary. With (c) the
condition reads M ≫ N·Λ²; with (c′) it depends on d and δ as well and is weaker. The first version's "at 48 bits
γ ≤ 0.05" holds at d = 95 and 127 and fails narrowly at d = 255 and 287 (0.054, 0.056) under (c).
The smallest code listed in `codes/REPORT.md` is 64 bits (256 positions at ¼ bit). [model: the built code is not
random, and the index has several blocks and levels, to which this corollary does not apply; see G3 and C2.3]

**Lemma C4 (any code: the pair bound through the angle between the two errors).** [proved, unrefereed]
For any fixed codebook (or conditionally on a random one), d ≥ 3, and any two fixed linearly independent items
y_1, y_2 orthogonal to Δ, let e_i = x_i/cos φ_i − z_i be item i's error vector in the rotated frame and χ the cosine
of the angle between the components of e_1 and e_2 orthogonal to span(z_1, z_2) (χ = 1 if the two items decode to
the same codeword). Then
  Pr[F_1 ∩ F_2 | z_1, z_2] ≤ T_{d−1}( q·√(2/(1 + χ)) ).
Proof. If O is a rotation fixing y_1 and y_2 then Rot·Oᵀ is Haar and gives the same (z_1, z_2), and it sends Δ to
Rot·OᵀΔ. So given (z_1, z_2) the rotated query Δ′ is uniform on the unit sphere of span(z_1, z_2)^⊥, of dimension
d − 1. −Δ′·e_i = r_i·W_i with r_i = ‖(e_i)_⊥‖ ≤ ‖e_i‖ = tan φ_i and W_i the coordinate of −Δ′ along (e_i)_⊥.
On F_1 ∩ F_2 both W_i > q, so W_1 + W_2 > 2q, and W_1 + W_2 = √(2 + 2χ)·(a coordinate of a uniform unit vector
of ℝ^(d−1)). This is R7's method with two items in place of two levels. ∎
Size [computed]: the bound is below δ·10⁻¹¹ when χ ≤ χ₀, χ₀ = 0.52 / 0.45 / 0.34 / 0.33 at d = 95 / 127 / 255 / 287
(`c2_union.log`, part 3); at χ = 0 it is zero at d = 95 (q√2 > 1) and δ·e^(−58) at d = 255.
So π₂ ≤ δ·10⁻¹¹ + Pr[F_1 and χ > χ₀]: for any code the question is how often two unrelated items' errors are
aligned by more than about a third (a half on the smallest block), on the event that one of them fails.

**Gap G3 (the built code).** [sketch: gap named]
For the index's code (scalar quantisers on the rotated coordinates, not a random codebook) I have no bound on
Pr[χ > χ₀ | F_1] at the level 10⁻¹¹. What exists: the measured correlation of two items' normalised errors under a
one-bit sign code, 0.00 at residual cosine 0 and 0.02 at 0.3 (`reports/surge/bounds2_work/pairdep.py`, n = 100,
400,000 rotations), and the referee's small run in which the sign code at n = 13 behaved like a 512-word random
code (Pr[F_2 | F_1] = 0.0096 against p̄ = 0.0099). Both are statements about the bulk and say nothing about a tail
of 10⁻¹¹. A proof would bound, for a product code, the chance that two independent uniform points have
quantisation-error vectors with cosine above χ₀; for independent coordinates' rounding errors this is a
large-deviation event of probability e^(−Θ(d)), and e^(−Θ(d)) at d = 95 to 287 against 10⁻¹¹ is the comparison that
has to be made with constants.

## C2.3 The Bonferroni step and the statement that results

**Theorem D (the union over N is attained: one block, one level, the plain rule, a random shared code).**
[proved, unrefereed]
One block of d degrees of freedom, one level, a random codebook of M = 2^B codewords shared by the items and
independent of the shared Haar rotation, the plain one-sided rule with exact φ at per-item level δ, N items. Let
Λ = ln(200·N/δ) and M·Ω_d(θ) = Λ. If N·M·J(θ) ≤ 0.01, with J from step (c) or (c′) of Lemma C2, then there is a
corpus of N items, all at the threshold of one query, with
  Pr_ω[the answer is wrong] ≥ N·δ·(1 − 0.01 − N·Λ²·δ/2).
So for this scheme a guarantee of ε on every corpus forces δ ≤ (ε/N)·(1 + 0.011) when Nδ·Λ² ≤ 10⁻³: its per-item
budget is ε/N, and not ε/N′ for any N′ < 0.99·N.
Proof. Theorem C on the equator corpus gives a corpus with failure probability at least N·p̄ − (N²/2)·π₂, and
p̄ = δ up to 2^(−M). Lemma C2 with γ₁ = 0.01 gives N·π₂/δ ≤ 0.01 + N·Λ²·δ + 0.01. Then N·δ − (N/2)·δ·(0.02 + N·Λ²·δ). ∎
In Corollary Q's terms this is a case where p(ω) is rarely large: π₂ = E p² ≤ Λ²·δ² + 0.02·δ/N, which is far below
δ/N, and Pr_ω[p(ω) ≥ x/N] ≤ N²·π₂/x² by Markov's inequality on p².
At N = 10⁹, δ = 2.5·10⁻¹⁹ the hypothesis holds from 50 or 51 bits with (c) and from 30 to 41 bits with (c′)
(Corollary C3's table).

**What is and is not covered.**
- Proved: Theorem C and Corollary Q for any per-item rule, any code, any blocks and levels. Theorem D for one
  block, one level, the plain rule and a random shared code only.
- Several levels: "the factor is N·L". [sketch: gap named] The first version asserted it from R7. It needs
  Pr[F_{1,ℓ} ∩ F_{2,ℓ′}] for two items at two different levels, whose codes in the index are nested refinements of
  one another; Lemma C2 is for one codebook, and R7 as refereed is about one item, with measured correlations of
  the built code. Not proved.
- Other half-widths (A4, the pivot). [sketch: gap named] The first version said this "changes only step (a)".
  It changes more [referee section 6]: Lemma C2's three terms are bounded relative to δ, and the conclusion needs
  them relative to the rule's own p̄, which for A4 on the equator is T_d(q·√(1+k)), 10⁻⁶·δ at d = 95, R = 1. The
  equator is also the wrong corpus for A4, whose worst latitude is s = √(k/(1+k)), and Lemma C2 uses Δ′ ⊥ z_i.
  Not proved for any rule but the plain one.
- Several blocks with a random code in each. [sketch: gap named] Theorem C holds as stated. In Lemma C2 full
  alignment needs the same codeword in every coded block, and partial sharing gives χ < 1, to which C4's bound
  applies with the blocks' variance shares. I expect a weaker sufficient condition than one block's. Not written.
- The built code: G3.
- k-NN, where an item's decision depends on other items' intervals: not covered.

## C2.4 Few codewords

**Claim E (one codeword: the items' failures are nearly one event).** [proved for M = 1; simulated for M ≤ 512
at d = 12]
For M = 1 (one codeword c, τ = −Δ′·c) every item of the equator corpus with cos φ_i > 0 fails as soon as τ ≥ q,
because sin φ_i ≤ 1. So p(Δ′) = ½ on an event of probability T_{d+1}(q), which is of the order of δ, and
π₂ ≥ ¼·T_{d+1}(q): the pair overlap is of order δ, not δ². (The referee simulated π₂ = 0.00213 against the bound
0.00187 at n = 13, δ = 0.01.) Pr[some item fails] then grows far more slowly than N·δ.
Monte Carlo (`c2_union.py` part 4; d = 12, δ = 0.01, N = 20, 100,000 trials, one codebook per M; reproduced by the
referee with a real Haar matrix per trial: 4.0, 11.7, 17.7, 18.2 at M = 1, 8, 64, 512):

| M | p̄ | Pr[some item fails] | effective count Pr/p̄ | Pr[F_2 given F_1] |
|---|---|---|---|---|
| 1 | 0.0050 | 0.019 | 3.9 | 0.455 |
| 2 | 0.0078 | 0.038 | 4.9 | 0.276 |
| 8 | 0.0100 | 0.113 | 11.2 | 0.081 |
| 64 | 0.0099 | 0.175 | 17.6 | 0.012 |
| 512 | 0.0101 | 0.182 | 18.1 | 0.020 |

Independent items give 1 − 0.99²⁰ = 0.182 and an effective count of 18.2.

**Withdrawn after the referee.** [refuted as stated]
The first version drew two general conclusions from Claim E: that R9 "needs the hypothesis M ≫ N·ln²(N/δ)", and
(in C2.0) that the union is "false when [the code] has fewer [codewords than items]". Neither is supported.
M ≫ N·Λ² is the sufficient condition of step (c) and nothing shows it necessary. By step (c′) it is not necessary:
at d = 95 the last term of Corollary C3 is 0.026 at 29 bits, where M = 5.4·10⁸ < N = 10⁹, so there the union is
attained within 2% with fewer codewords than items [computed, `c2_revision.log` part 1, row d = 95, 29 bits]. The
reason is geometric (the referee's): an item fails through a codeword tilted by τ > q·sin φ off the query's
equator, and such a codeword's cell holds little of the equator, less by a factor that is a power of δ as δ
falls. The Monte Carlo above is at d = 12 and δ = 0.01, where q is small, and does not show that regime. The
referee measured the excess Pr[F_2 | F_1] − p̄ for a random shared code at n = 13 as 0.5/M, 0.25/M, 0.14/M at
M = 8, 64, 512 for δ = 0.01 and 0.35/M, 0.10/M, 0.05/M for δ = 0.002: the boundary depends on d and δ, not on M/N
alone. My conjecture that the effective count for 1 ≪ M ≪ N is "of order M" is withdrawn with them.
What remains of Claim E: for a single codeword the union over N is not attained. Where between one codeword and
the thresholds of Corollary C3 it starts to be attained is open.

## C2.5 What this adds to Molinaro, Woodruff and Yaroslavtsev

MWY (SODA 2013, as read by the librarian from the abstract and introduction; neither the referee nor I read the
paper): in one-way communication, solving k copies with probability 2/3 needs Ω(k·R_{1/k}(f)) bits, so "solve each
copy with error O(1/k) and take a union bound" is optimal up to a constant factor in the communication. It is a
statement of order, for every protocol, and its consequence for sketches is a dimension or bit count
Ω(ε⁻²·log(n/δ)).
What is here that is not there:
1. A constant, for one named scheme. For the plain rule on one block with a shared rotation and a random shared
   code, the per-item failure budget is ε/N to within one percent (Theorem D), so there the confidence constant
   must rise by the amount that divides δ by N: c² grows by 2·ln N to first order, with no smaller constant in
   front of ln N. MWY leave that constant open: Ω(log(n/δ)) allows log(n^(1/10)/δ). This is not proved for
   per-item rules in general, for several blocks or levels, or for the built code.
2. Shared randomness, for every per-item rule. In MWY's model the copies are independent instances. Here all N
   items share one seed. Corollary Q says that sharing cannot be used to hold a per-item failure rate above
   about 1/N on more than a few ε of the seeds, whatever the rule and the code.
3. A sufficient condition under which the union is attained (Corollary C3), and one case where it is not (a
   single codeword). The boundary between them is not located.
What MWY has that is not here: all protocols, including decoders that look at several items together and codes
that are not per-item.

## C2 summary

- Theorem C: given the seed, the failures of i.i.d. items are independent; any code, any per-item rule, any number
  of blocks and levels. Elementary. [proved, unrefereed; simulated]
- R9 as first stated ("for any per-item rule, p̄ ≤ ε/N") is false: a rule can put all its failures on a set of
  seeds of probability ε. [refuted; the referee's counter-example]
- Corollary Q: for every per-item rule and i.i.d. threshold corpus, Pr_ω[p(ω) ≥ x/N] ≤ ε/(1 − e^(−x)). This is R9
  in the form that is true. [proved, unrefereed]
- Corollary C1: p̄ ≤ ε/N + (N/2)·π₂, a bound on the mean over a random latitude item. [proved, unrefereed]
- Lemma C2, Corollary C3, Theorem D: for one block, one level, the plain rule and a random shared code, the union
  over N is attained to 1% under a sufficient condition met from 50 to 51 bits per block with step (c), and from
  30 to 41 bits with the referee's step (c′), at N = 10⁹, ε = 10⁻⁹. [proved, unrefereed; computed; (c′) itself
  unrefereed, checked by Monte Carlo at d ≤ 24]
- Lemma C4: any code, the overlap through the cosine χ of the two errors; harmless below χ₀ = 0.33 to 0.52.
  [proved, unrefereed; computed]
- Claim E: with one codeword the union is not attained. [proved for M = 1; simulated at d = 12]
- Withdrawn: "R9 needs M ≫ N·ln²(N/δ)" and "false with fewer codewords than items" as necessary conditions.
- Sketches, not proved: the factor N·L over levels; rules other than the plain one in Theorem D; several blocks in
  Lemma C2. Open: the built code's tail of χ (G3); where the union starts to be attained for small codes; k-NN.

# Changes after the referee

Source: `referee/referee-lower-bounds.md` (21 November 2026) and its scripts. Every point was accepted; the two
places where my figure differs from the referee's are marked. New files: `c1_revision.py`, `c1_revision.log`,
`c2_revision.py`, `c2_revision.log`. No tag was raised; several were lowered or withdrawn (listed below).

C1
- M6: "a lower bound on the worst case" now reads as the supremum over (y, Δ) of E_ω h, with the warning that
  Theorem B bounds the mean over y and nothing for a single y. [referee 1(c)]
- Class 𝓛: the convention for ŷ = 0 is φ := π, not π/2 (B3 failed at θ = π/2, M = 1). A new paragraph lists what 𝓛
  contains (one block, plain rule; A4) and what it leaves out (radial slack, intersections, R8, several blocks).
  [referee 1(b), section 4]
- A4: retitled and restated. It is R8 with Ê² dropped under the root, not a new interval; what is new is that it
  lies in 𝓛 and meets Theorem B. "Equals δ at the worst s" is now "conditionally on φ". φ ≥ π/2 is handled. The
  claim that it is narrower than the plain rule is now a statement about half-widths only, with the figures for
  how often its upper end is above the plain one (13% at s = 0, 50% at s = −0.3, 100% at s = −0.9 in my run).
  [referee section 2]
- A5 (new): one event serves the plain rule, A4 and capped R8 in one block. Statement and proof are the referee's;
  simulated here (rate of the smallest end 0.0097 to 0.0102 at δ = 0.01).
- A4′ (new): A4 inside the bit budget by the referee's repair 1(a), ĉ with cos φ/ĉ ∈ [1, 1 + η_g] and half-width
  H_q(φ̂) + η_g; the proof is written out and was checked by hand. "Met at Rd + O(log d) bits" now refers to A4′.
- N1(c): now says the set is empty when Ê² > 1 + k, and that one-sided use needs the cap of Theorem 3.11
  (0.0169 and 0.0199 at s = 0.9 and 0.99 without it, at most δ with it; reproduced).
- N2: restated in the referee's exact form for every code, with half-width q_(d+1)(δ)·sin φ; "cap-shaped cells"
  and the "-ish" factor are gone.
- N3: "the corrected target" is now "the conjectured least half-width in the full model", with the proved part
  named as its s = 0 value in 𝓛.
- B4: the quantile step has the referee's proof in place of a numerical check; the slip δ ≈ 10⁻¹³ is corrected to
  2.5·10⁻¹⁹; B4 is marked as a limit statement that is 8 to 15% off at the index, with B6 the source of numbers.
- B5: "exactly one halving" is now "at most one halving", with the computed sums of the two tails. My figure for
  d = 255, k = 0.001 is 1.91 (the sum at s = 0 is 2·T_d(q√(1+k))/T_d(q), which is 1.908); the referee's is 1.94.
  The conclusion is the same.
- B6 table: the "achievable" column is replaced by the exact mean for the random code (0.4122 and 0.6405 where
  the referee computed them; reproduced) and the B6 column by a rigorous left sum (0.2% lower). The sandwich is
  4 to 6% wide, not 7 to 8%. The reading now says the comparison is of half-widths inside 𝓛 and is not a saving
  for the index.
- Monte Carlo table: redone. The "naive" column now uses the half-width the averaged requirement allows; a row
  for the averaged requirement, rows at s = 0.9 and 0.99, the pivot with and without the cap, the smallest of
  three ends, and the share of draws where A4's end is above the plain end are added.
- G1: the two "certain" things are removed. It now says what is known (one scheme outside 𝓛 has a smaller
  half-width) and that nothing is proved for the full model; the referee's remark that M5 is the family of Bayes
  problems indexed by the latitude s is added.
- G2: d ≥ 3 for the Jensen step; δ < 1/540 for the constant-factor bound.
- Tax: the limit needs w ≪ 1/√d, not w ≪ q; a table at finite w is added (2.16 bits at d = 255, w = 0.1);
  the scope (𝓛, one block, one-sided, first order) and what it does not say are stated.
- C1 summary: every use of w* now reads "least mean half-width in 𝓛"; the low-rate caveat now says those rows say
  least about the index.

C2
- C2.0: R9 "for any per-item rule" is marked [refuted] with the referee's counter-example and restated in the
  quantile form. A per-item rule is now defined as a function of the item's messages, the whole seed and the query.
- Theorem C: now conditions on the whole seed and holds for any i.i.d. corpus; the first version's conditioning on
  Δ′ alone is recorded as false for a shared random codebook and for rules that use the rotations otherwise.
  What the latitude adds is stated separately.
- Corollary Q (new): Pr_ω[p(ω) ≥ x/N] ≤ ε/(1 − e^(−x)); the statement is the referee's.
- Corollary C1: now says it bounds a mean over a random latitude item, and that p̄ = Σ_ℓ T(a_ℓ; h_ℓ) needs every
  block on its equator.
- Lemma C2: step (c′), the referee's refinement, is added and was checked here (thresholds 30, 33, 40, 41 bits
  reproduced; Monte Carlo at d = 6, 12, 24 gives 0.83 to 0.96 of the refined bound). The inequality on κ now has
  the referee's proof by Wendel's inequality, for d ≥ 2.
- Corollary C3: now "a sufficient condition", with both rows of thresholds; "at 48 bits γ ≤ 0.05" is limited to
  d = 95 and 127.
- Lemma C4: "any fixed codebook or conditionally on a random one", d ≥ 3, items linearly independent.
- Theorem D: the title and statement carry the scope (one block, one level, plain rule with exact φ, random shared
  code independent of the rotation). "The factor is N·L" is now [sketch: gap named]. The remark on A4 and the
  pivot is now [sketch: gap named] with the referee's two objections.
- Claim E: kept for M = 1. "R9 needs M ≫ N·ln²(N/δ)", "false with fewer codewords than items" and the conjecture
  on the effective count are withdrawn.
- C2.5: point 1 is limited to the plain rule, one block, a random code; point 2 now rests on Corollary Q; point 3
  is a sufficient condition and one counter-case, with the boundary not located.
- G3: the referee's small run with the sign code is added; the [model] sentence in C3 now names its dependence on
  the several-block and N·L steps.

Disputed: nothing of substance. One figure differs (B5, above).

Not checked by anyone: N3's posterior computation; the constants inside the O(ln d/d) terms of A3 and Theorem A;
the readings of TurboQuant, of Molinaro, Woodruff and Yaroslavtsev and of Böröczky and Wintsche, which rest on the
librarian's summaries; anything at the index's sizes by simulation (every simulation is at n ≤ 25 and δ ≥ 0.001;
figures at δ = 2.5·10⁻¹⁹ are computed from the formulae). The referee's step (c′) has been checked by me and by
the referee's own simulation, and by no third party.
