# Referee report: confidence track, lower bounds (CLAIMS.md C1 and C2)

Referee: independent agent, 21 November 2026. Source refereed: `notes/program/confidence/CLAIMS.md` as of 07:30.
Not read: NOTEBOOK.md, DESIGN-CLAIMS.md, DESIGN-NOTEBOOK.md. Verdicts are appended as they are reached; a later
entry may correct an earlier one and says so. "No error found" means I tried to break it and could not.
Referee scripts are in the session scratchpad and are quoted where a number depends on them.

## 1. The model M1 to M6

**Verdict: no error found as a formalisation of one block, one item, one query. Four unstated points, none fatal.**

- M1 to M5 say what "a B-bit message per item, valid for every fixed pair with probability 1 − δ over the build's
  randomness" means: enc(y, ω) into M = 2^B values, (ŝ, h) any functions of (c, Δ, ω), validity for every (y, Δ).
  The reduction to unit blocks is sound for a lower bound (a scheme for all norms restricts to one for unit blocks).
- (a) **Real scalars are outside M2.** A1 and A4 store the real number φ. A2 puts A1 inside the budget. A4 is stated
  with "φ stored" and is never put inside the budget, and rounding φ up is *not* safe for A4 by itself: with
  ŷ = κ*(φ̂)·x₀ and φ̂ > φ the bias a = κ*(φ̂)cos φ − 1 moves by (cos φ/cos φ̂ − 1)/(1 + k̂), which a half-width
  H_q(φ̂) does not cover (at q → 0 the half-width tends to 0 and the bias does not). Repair, same as A2: with
  cos φ/ĉ = g ∈ [1, 1 + η_g], use ŷ = κ*(φ̂)x₀ and half-width H_q(φ̂) + η_g. Proof: a = (κ̂ĉ − 1) + (g − 1)κ̂ĉ with
  κ̂ĉ = 1/(1 + k̂) ≤ 1, and b = κ̂ sin φ ≤ κ̂ sin φ̂, so −s·a + q√(1−s²)·b ≤ H_q(φ̂) + η_g by B0(i). With η_g = d⁻²
  this is the O(log d) bits of A2, so "the bound of Theorem B is attained at B = Rd + O(log d) bits" stands after
  this repair, not as written.
- (b) **Lemma B3's convention φ = π/2 for ŷ = 0** breaks B3 at θ = π/2 with M = 1 (σ{φ ≤ π/2} = 1 > M·Ω_d(π/2) = ½).
  Repair: put φ = π when ŷ = 0 (a = −1, b = 0 whatever φ is). Theorem B is unaffected.
- (c) **M6.** "A lower bound on the average is a lower bound on the worst case" is true for sup over (y, Δ) of E_ω h.
  Theorem B bounds E over (y uniform, ω) of η; it gives nothing for a single y, and a scheme may be narrow on a
  set of y of measure up to M·Ω_d(θ). That is what the statement says; the summary's "least half-width" should
  keep the words "on average over y".
- (d) **G2's Jensen step needs d ≥ 3** (T_d is concave on [0, 1] for d = 2); M-model says d ≥ 2.
- Nothing in C1.1 to C1.3 uses more than the model grants: B1 averages the validity requirement over queries
  drawn independently of ω, which M3 and M5 allow; B3 uses only that there are M messages per ω. C2 is a different
  model (N items, per-item rule) and is refereed on its own terms in section 6.
- The class 𝓛 is a restriction of M4, stated as such. See section 4 for whether it is fair.
- δ is quoted as "≈ 10⁻¹³" in B4 and as 2.5·10⁻¹⁹ everywhere else; the second is the one-sided budget of the notes.

## 2. Claim A4 (centre shrunk by 1/(1 + k), half-width q·tan φ/√(1 + k))

**Verdict: no error found in the statement for real φ; correct inside the bit budget only after the repair of
section 1(a). The statement is weaker than what is true: A4 is a relaxation of R8 on the same event.**

Checked by hand: a = −k/(1+k), b = tan φ/(1+k), a² + q²b² = k/(1+k) = η², so by B0(i) the argument of T_d is at
least q for every s; equality at (s, √(1−s²)) ∝ (−a, qb), which is s = +√(k/(1+k)). At s = ±1 the error is the
deterministic ∓a·s and |a| ≤ η. Two-sided with q_d(δ/2): the lower tail is the mirror image (s → −s).

Unstated hypotheses and imprecisions.
- "The failure probability equals δ at s = √(k/(1+k))" is conditional on φ. The worst s depends on φ through k, so
  for a fixed query and a code whose φ varies the unconditional failure probability is strictly below δ at every
  s (0.0090 against δ = 0.01 at the best s in my run with 512 random codewords at d = 12).
- φ ≥ π/2 (possible with few codewords) is handled by H_q = 1, ŷ = 0; the claim's formulae assume φ < π/2.
- The code must be a function of Rot·y alone (Lemma 3.1's condition), and φ must be the true angle.

Simulation (referee script `ref_sim_a4.py`; 400,000 draws per cell; δ = 0.01 or 0.02; real Haar matrices by QR in
two of the runs, a random orthonormal 2-frame in the others, which is the same law). Codes: the sign code on the
hypercube (n = 8), a 2-bit scalar grid (n = 13), a ternary dead-zone code (n = 5), random codebooks with
M = 512 (n = 13), 16 (n = 8), 2 (n = 4) and 1 (n = 8). Queries at s = 0, ±0.3, 0.6, ±0.9, 0.99, ±0.999.
- A4 one-sided at q_d(δ): never above δ in any of the 63 cells; largest 0.0094 (δ = 0.01) and 0.0192 (δ = 0.02).
  In every cell the rate agrees with the mean over the drawn φ of T_d((η + s·a)/(√(1−s²)·b)) to within about two
  standard errors (largest gap 0.0003), so the exact conditional law of the proof is what the simulation sees.
- A4 two-sided at q_d(δ/2): never above 0.0093 (δ = 0.02) or 0.0077 (δ = 0.01).
- Aligned and anti-aligned queries (|s| ≥ 0.9): A4 fails with probability below 0.003; nothing breaks.

Relation to R8 (Theorem 3.12, one block: ξ² = k, c·sd = q·tan φ, Θ·sd² = b² = 1).
- R8's interval is centre Ê/(1+k), half-width q·tan φ·√(1 + k − Ê²)/(1+k). A4 is the same centre with Ê² dropped
  under the root. So A4's interval contains R8's for every outcome: **A4 is R8 with its half-width replaced by its
  maximum over Ê**, the only change needed to land in 𝓛. Chapter 3 already says "at Ê = 0 it is the plain width
  divided by √(1 + ξ²)". A4 is therefore not a new interval; what is new is that it meets Theorem B.
- One event really does serve all three. Let E₁ = {s − Ê ≤ √k·√(1 − s²)} (the one-sided event {U ≤ q}). On E₁:
  plain, s ≤ Ê + √k; A4, s·(1+k) − Ê ≤ √k·(√(1−s²) + √k·s) ≤ √(k(1+k)) by Cauchy–Schwarz; R8 as repaired. So
  the smallest of the three upper ends costs one event, in one block. (Chapter 3's remark that R8 and the plain
  interval have different events is about several blocks, where the shares are unknown.)
- **A4 does not dominate the plain rule as a one-sided bound.** Its upper end Ê/(1+k) + √(k/(1+k)) is above the
  plain upper end Ê + √k whenever Ê < −√k·√(1+k)/(1 + √(1+k)), about −½·q·tan φ. In the runs this was 13% to 22% of
  draws at s = 0; at s = −0.3 it was 50% to 98% for the codes with k below 0.4 and 15% to 23% for the three
  codebooks of 16 words or fewer; at s = −0.9 it was over 99% for the former. "Narrower by √(1+k)" is a statement about
  half-widths (the functional of M6), not about the bound an exclusion uses. Since the events coincide, use the
  minimum.
- **The pivot's upper end is not a one-sided δ-bound without the cap of Theorem 3.11.** Used one-sidedly at q_d(δ)
  as CLAIMS.md's Monte Carlo table does (column "pivot"), it fails with probability rising to 2δ for aligned
  queries: 0.0168 at s = 0.9, 0.0197 at 0.99, 0.0200 at 0.999 (n = 13, M = 512, δ = 0.01); 0.0394 at s = 0.99
  (n = 5, δ = 0.02). With the refereed cap (v ≤ b when Ê ≥ b) it is at most δ in every cell, within
  noise. The table stops at s = 0.8 and so shows 0.0100. N1(c) itself claims two-sided validity only, and that
  held: the rate was δ·Pr[cos φ > 0] within noise in every cell (0.0096 to 0.0104 for the codes with cos φ > 0
  always, at δ = 0.01).

## 3. Claims N1 and N2

**N1: no error found.** (a), (b) are substitutions; (c): the quadratic (1+k)s² − 2Ês + Ê² − k ≤ 0 has the stated
centre and half-width; the set is empty when Ê² > 1 + k (the event has failed), which the claim does not say.
Two-sided failure is exactly δ for every s, including |s| → 1 (simulated, section 2). One-sided use needs the cap.

**N2: correct after a small repair of its supporting statement; the conclusion stands and is stronger than stated.**
- The supporting statement is given for cap-shaped cells with a "(1 + O(1/√d))-ish" factor. Neither is needed.
  For *any* code of the Lemma 3.1 form, if Δ is uniform and independent of (y, ω) then
  Δ·y − cos φ·(Δ·x₀) = Δ·(y − cos φ·x₀), and ‖y − cos φ·x₀‖ = sin φ exactly. So the error of the shrunk estimator
  is sin φ·U′ with U′ a coordinate of a uniform unit vector in ℝ^(d+1), independent of φ, and the half-width
  q_(d+1)(δ)·sin φ·‖Δ‖ fails with probability exactly δ on average. Simulated on four codes: 0.0101, 0.0099,
  0.0100 (δ = 0.01), 0.0198 (δ = 0.02).
- Hence a converse that uses only the averaged requirement cannot prove more than q_(d+1)(δ)·E sin φ, about
  q·2^(−R). That is N2. The ratios cos θ_R = 0.968, 0.866, 0.541 are right.
- **The Monte Carlo column "naive shrunk (N1b)" uses half-width q·sin φ·cos φ, which is not the rule the averaged
  requirement allows**: it fails on average too (0.031 to 0.104 in my runs against δ = 0.01). The sentence "the one
  the averaged converse would allow" should refer to half-width q·sin φ. With that half-width the conclusion is
  the same: for fixed queries it fails 1.3% at s = 0.3, 9.4% at 0.6, 40% at 0.9, 73% at 0.999 (n = 13, M = 512),
  and 50% at s = 0.99 with M = 16.
- The comparison with TurboQuant rests on a library verdict I did not read. [not checked]

## 4. Theorem B, Lemmas B0 to B3, Corollaries B4 and B6, Lemma B5

**B0: no error found.** Both parts checked by hand, including cos φ ≤ 0.

**B1: no error found.** The average over v is legitimate because each Δ(v) is a fixed query and v is independent
of ω; e·v = b·(v·ê_⊥) has the coordinate law for every ω, hence is independent of (ω, η). The three conventions at
√(1−s²)·b = 0 are the right ones (failure is the strict event −s·a > η).

**B2: no error found.** I pressed three points. (1) The grid: ψ ↦ −a·sin ψ + q₀·b·cos ψ = √(a² + q₀²b²)·cos(ψ − ψ*)
with ψ* ∈ [−π/2, π/2] because b ≥ 0, and the grid's spacing is 2ε_m, so some grid angle is within ε_m. (2) The
grid index depends on ω through ψ*(ω); the union over all m indices covers that. (3) Markov is applied to the
conditional failure probability P_i(ω) ∈ [0, 1] with E_ω P_i ≤ δ from B1, for each fixed i; the end points
s = ±1 give P_i = 1 when the inequality is strict, which is at least T_d(q₀). The bound holds for every y.

**B3: correct after the repair of section 1(b)** (φ := π when ŷ = 0; or restrict to θ < π/2).

**Theorem B: no error found.** It is a statement about E over (y uniform, ω); see 1(c). Hypotheses used and not
stated: none beyond 𝓛 and one-sided validity for every (y, Δ) with ‖Δ‖ = 1.

**B4: correct; the step checked only numerically has a short proof, given here.** For 0 < x < 1, multiplying the
integrand of T_d(x) = κ′_d·∫ₓ¹(1 − t²)^((d−3)/2)dt by t ≤ 1, or by t/x ≥ 1, gives
(1 − x²)^((d−1)/2)/(d − 1) ≤ ∫ ≤ (1 − x²)^((d−1)/2)/(x(d − 1)). With L(x) = −ln(1 − x²) this is
ln(1/T_d(x)) = ((d−1)/2)·L(x) + ½·ln d + O(1) − r(x), 0 ≤ r(x) ≤ ln(1/x). At q = q_d(δ) and q₀ = q_d(d²δ):
((d−1)/2)·(L(q) − L(q₀)) ≤ 2 ln d + ln(1/q), and ((d−1)/2)·L(q) ≥ ln(1/δ) − ½ ln d − O(1); q ≥ c/√d once δ is below
a constant, so ln(1/q) = O(ln d). Hence 1 − L(q₀)/L(q) = O(ln d / ln(1/δ)) → 0. Since x = √(1 − e^(−L)) has
d ln x / d ln L = L·e^(−L)/(2(1 − e^(−L))) ≤ ½, ln(q/q₀) ≤ ½·ln(L(q)/L(q₀)) → 0, for every size of q including q → 1.
And ∂ ln H_q/∂ ln q ≤ 1, so H_{q₀}(θ)/H_q(θ) → 1. ∎
Numerically q_d(d²δ)/q_d(δ) ≈ √(1 − 2 ln d/ln(1/δ)): 0.89 to 0.93 at ln(1/δ)/ln d = 10, 0.966 to 0.993 at 30,
0.995 to 1.000 at 100 (d = 95 to 10⁵). At the index ln(1/δ)/ln d is 9.4 (d = 95) and 7.7 (d = 255), so B4 is a
limit statement and is off by 8 to 15% there; the numbers must come from B6, as the author does.

**B5: (i), (ii) and the sandwich: no error found. The last sentence of (iii) is wrong as a statement about the
optimum.** "One side costs exactly one halving of δ" holds only as k → 0. For k not small the two tails of A4 peak
at s = +√(k/(1+k)) and s = −√(k/(1+k)), and at either the other tail is negligible. Maximum over s of the sum of
the two tails, in units of T_d(q) (computed, `ref_num1.py`): 1.0000 at d = 95, k = 0.19 and 1.38; 1.0000 at d = 255,
k = 0.09; 1.035 at d = 255, k = 0.018; 1.94 at d = 255, k = 0.001. So A4 run two-sidedly at q_d(δ) (not q_d(δ/2)) is
already two-sided δ-valid to four digits when k ≥ 0.09, and the two-sided optimum in 𝓛 equals the one-sided one
there. The effect is at most the 0.5% the author quotes; the sentence, and the summary line "one halving of δ,
nothing else", should say "at most one halving".

**B6: no error found.** u ↦ cos ε_m·H_{x(u)}(θ(u)) is non-decreasing and lies below the upper quantile function of η.

**Two rows recomputed at δ = 2.5·10⁻¹⁹ (my code, `ref_num1.py`; cap and tail integrated in log space).**

| d | R | q_d(δ) | k | plain | w* | achievable | B6 (author) | B6 (mine, midpoint) | B6 (mine, left sum) |
|---|---|---|---|---|---|---|---|---|---|
| 95 | 1 | 0.7565 | 0.191 | 0.4367 | 0.4002 | 0.4260 | 0.3917 | 0.3917 | 0.3908 |
| 255 | ¼ | 0.5186 | 0.649 | 0.8058 | 0.6275 | 0.6565 | 0.6120 | 0.6120 | 0.6106 |

(Also d = 95, R = ¼: 0.7568 / 0.7568 / 0.7550.) All entries reproduce. Two small remarks. The author's integral is
a midpoint rule on 400 cells; the rigorous lower sum of a non-decreasing integrand is 0.2% lower. "Achievable"
is H_q at the angle with M·Ω = 2 ln d and leaves out the d⁻² exception and the scalar's quantisation (1.8% of
the value in the R = 7, d = 95 row, negligible elsewhere); the exact mean of H_q(φ) for the random code is lower,
0.4122 and 0.6405 in these two rows, so the proved sandwich is about 5% wide, not 7 to 8%.

**Is 𝓛 fair, and is the index's certificate in it?**
- One block, plain rule: yes. The estimate f·⟨ō, RΔ⟩ is linear in Δ (the centre correction of Theorem 3.11(i)
  cancels identically), and c·‖y‖·tan φ·‖Δ‖/√d is a multiple of ‖Δ‖; stored scales rounded up are inside η(c, ω).
- Not in 𝓛: (1) the radial slack η·|Ê| the referees required after Lemma 3.1, which depends on the direction of
  Δ; (2) any intersection with the deterministic interval |Δ·y| ≤ ‖Δ‖‖y‖ or with an earlier level's interval,
  which the search loops do; (3) R8; (4) several blocks, where the half-width is √(Σ_j η_j²‖Δ_j‖²), not a
  multiple of ‖Δ‖. Theorem B says nothing about (2) to (4).
- Fairness. 𝓛 excludes the project's own refereed interval R8, which uses the same message and is never wider
  than A4. So w* is "the least half-width in 𝓛", never "the least half-width". Point (2) matters exactly where w*
  is interesting: at ¼ bit w* is 0.76 (d = 95) and 0.63 (d = 255) of the deterministic bound, and an interval
  cut at ±‖Δ‖‖y‖ is outside the class. For a typical offset (|s| of order 1/√d) R8's gain over A4 is O(1/d), so
  the restriction costs nothing there; the bound is forced by queries with |s| up to √(k/(1+k)), where R8 does gain.

## 5. The confidence tax

**Verdict: algebra correct; one condition misstated.** R_cert, R_sd and the difference follow from
w = q/√(4^R − 1 + q²) and tan θ_R/√d = w. The limit log₂(q√d) needs **w ≪ 1/√d**, not w ≪ q: at d = 255, w = 0.1
(w/q = 0.19) the tax is 2.16 bits, not 3.05; it is 2.90 at w = 0.03 and 3.03 at w = 0.01. The three limits
(2.88, 3.05, 3.12), the value 0.92 at w = 0.3 and the 97 bytes are right. It inherits "in 𝓛", "one-sided", "one
block" and "first order" (B4's regime); at the index's sizes B6 and the achievable value bracket w* within
about ±5%, which is ±0.07 bit per degree of freedom. The tax is the statement that a c-sd interval needs an sd c
times smaller, at 6 dB per bit; nothing deeper is proved by it.

## 6. Theorem C, C1, C2, C3, Theorem D, C4, Claim E

**Theorem C: correct after a repair of what is conditioned on; the displayed formulae and the Bonferroni bound
stand. The independence is weaker (more trivial) than the remarks make it sound.**

- What is true, for any i.i.d. corpus, any shared seed and any per-item rule: given the whole seed ω (rotations,
  codebooks, anything else shared), F_i is a function of (item i, ω), so the F_i are i.i.d. given ω. Hence
  Pr[some item fails] = E[1 − (1 − p(ω))^N] and π₂ = E p(ω)² = Pr[F_1 ∩ F_2]. No latitude is needed for this.
  It holds for a shared rotation, a shared codebook, several blocks and several levels.
- What the latitude adds: every item has score exactly t, and p depends on ω only through (Δ′, codebooks) for
  rules that use the rotations only through Δ′.
- **The literal statement "conditionally on Δ′ the events are independent, for any codebooks and any per-item
  rule" is false in two cases the text allows.**
  (i) A codebook that is random and shared (as in Lemma C2, "it is part of ω"). Counter-example, simulated
  (`ref_sim_c.py` part B; n = 13, δ = 0.01, Δ′ held fixed, equator, plain rule, one random codeword shared by two
  items): Pr[F_1 | Δ′] = 0.00507, Pr[F_1 ∩ F_2 | Δ′] = 0.00213, which is 83 times the product; 39 times with two
  codewords, 7 times with eight. Repair: condition on (Δ′, C). π₂ = E p(Δ′, C)², which is what C2 computes.
  (ii) A rule that uses the rotations other than through Δ′. C2.0 defines a per-item rule as a function of "the
  rotations and the query"; the proof assumes "functions of its messages and of Δ′". Repair: condition on ω, or
  restrict the definition.
- Unstated: the corpus must be free to place blocks anywhere on the latitude with one common prefix read.
- Simulation (`ref_sim_c.py` parts A and C). "Real": a fixed query, a corpus drawn on its latitude, one Haar
  matrix by QR per trial applied to all N = 20 items. "Model": Δ′ uniform and items i.i.d. on its latitude. One
  block (n = 13, δ = 0.01), fixed codebooks with M = 1, 8, 64, 512 and the sign code (8,192 codewords), plain rule
  and A4, latitudes s = 0 and 0.3, 40,000 trials each: in all 16 cells p̄, π₂, Pr[K ≥ 1] and Pr[K ≥ 2] agree
  between real and model within noise (largest gap in Pr[K ≥ 1]: 0.1235 against 0.1288, 2.3 standard errors of
  the difference; rerun with 400,000 trials, that cell gave 0.1242 against 0.1240 and the next largest 0.1232
  against 0.1237), and N·p̄ − C(N,2)·π₂ ≤ Pr[K ≥ 1] ≤ N·p̄ in every cell. Two blocks (n = 6 and 8,
  separate shared rotations), two levels (level 1: sign code on block 1, block 2 by Cauchy–Schwarz; level 2: a
  2-bit grid on block 1 and the sign code on block 2), latitudes (0.10, −0.05), 200,000 trials: Pr[K ≥ 1] =
  0.6020 real, 0.6014 model; Pr[K ≥ 2] = 0.2368, 0.2371; the histograms of K agree to 0.001.
  Few against many codewords (plain, equator, real): effective count Pr[K ≥ 1]/p̄ = 4.0 (M = 1), 11.7 (8), 17.7
  (64), 18.2 (512), 18.3 (sign code); independent items give 18.2. This reproduces Claim E's table.

**Corollary C1: no error found in the two inequalities.** Two limits on what it says.
- It bounds the mean p̄ over a random latitude item. R9 claims the bound "for every scale vector that occurs";
  C1 is weaker, and R9's "every" cannot be had (the scales are random through φ).
- "p̄ = Σ_ℓ T(a_ℓ; h_ℓ)" needs every block on its equator (s_j = 0); off it the true scale is A_j‖P_{y⊥}Δ_j‖ < a_j.
- **C2.0's "claim to prove" is false for per-item rules in general, whatever the code.** Counter-example: fix a
  set S of rotated queries with Pr[Δ′ ∈ S] = ε; the rule excludes every item when Δ′ ∈ S and otherwise reads
  every item to the end. It is a per-item rule (Theorem C's remark 1 allows dependence on Δ′). The answer is
  wrong with probability exactly ε on every corpus that has a match, and p̄ = ε, not ε/N. C1 is consistent with
  it (π₂ = ε). So "no per-item rule can avoid the union over N" is not a theorem; the union is avoided by any
  rule that makes items fail together, and the question is whether doing so buys anything. What does hold for
  every per-item rule and every i.i.d. corpus is the quantile form, from 1 − (1 − p)^N ≥ 1 − e^(−x) when
  p ≥ x/N: **Pr_ω[p(ω) ≥ x/N] ≤ ε/(1 − e^(−x)) for every x > 0.** For example p(ω) ≤ 0.7/N outside a set of
  seeds of probability 2ε. A rule cannot be narrow on more than a few ε of the seeds; it can only move its
  failures onto a few seeds, where they are no use. I suggest R9 be restated in that form, with Theorem D as the
  case where p(ω) is nearly constant.

**Lemma C2: no error found; step (c) is valid and very loose; the unproved inequality has a hand proof.**
- (a), (b): checked. Pr[G_1(c)] = Ω_d(θ)·δ confirmed by simulation in five settings (`ref_num3.py`).
- (c): given (c, Δ′), z_2 is uniform on the unit sphere of Δ′^⊥ and independent of z_1, and
  z_2·c = √(1 − τ²)·U with τ = −Δ′·c, so the bound T_d(cos θ) = Ω_{d−1}(θ) is right. It throws away τ. On G_1(c),
  τ > q·sin φ_1, so the same line gives the **refinement**
    Pr[G_1(c) ∩ {angle(c, z_2) ≤ θ}] ≤ δ·∫₀^θ κ_d·sin^(d−1)φ·T_d(cos θ/√(1 − q²·sin²φ)) dφ,
  smaller than Ω_d(θ)·δ·Ω_{d−1}(θ) by about ((1 − q²)/(1 − q²sin²θ))^((d−1)/2). Simulated in five settings at
  d = 6, 12, 24: the event's probability is 72 to 80% of the refined bound in four of them (1.1 ± 0.3 times it in
  the fifth, the noisiest) and 10 to 31% of the author's.
- The inequality κ_{d−1}·d/(κ_d·(d − 1)) ≤ 1 + 2/d: κ_{d−1}/κ_d = Γ(d/2)²/(Γ((d+1)/2)·Γ((d−1)/2))
  = [Γ(d/2)/Γ((d−1)/2)]²·2/(d − 1) ≤ 1 by Wendel's inequality Γ(x + ½) ≤ √x·Γ(x) (log-convexity of Γ) at
  x = (d−1)/2; and d/(d − 1) ≤ 1 + 2/d for d ≥ 2. So it holds for every d ≥ 2, not only d ≥ 4. Checked numerically
  for d = 2 to 4,999.

**Corollary C3: numbers reproduce.** Λ = 68.85, middle term 1.19·10⁻⁶; the last term falls below 0.01 at 50 bits
(d = 95, 127) and 51 bits (d = 255, 287). "At 48 bits γ ≤ 0.05" holds at d = 95 and 127 (0.045, 0.046) and fails
narrowly at d = 255 and 287 (0.054, 0.056).

**Theorem D: no error found, for what it states** (one block, one level, a random codebook shared by the items
and independent of the rotation, the plain one-sided rule with exact φ, the equator corpus). The arithmetic
(γ₁ = 0.01, last term ≤ 0.01, N·δ·(1 − 0.01 − N·Λ²·δ/2)) is right; p̄ = δ up to Pr[φ ≥ π/2] ≤ 2^(−M).
- **The sentence "with levels … so the factor is N·L" is not proved.** It needs Pr[F_{1,ℓ} ∩ F_{2,ℓ′}] for two
  items at two different levels, whose codes in the index are nested refinements of one another; Lemma C2 is for
  one codebook. R7 as refereed is about one item, with measured correlations of the built code. Tag: sketch.
- The remark that A4 "changes only step (a)" understates the change. C2's three terms are bounded relative to δ;
  the conclusion needs them relative to A4's own p̄, which on the equator is T_d(q√(1+k)), 10⁻⁶·δ at d = 95, R = 1.
  The equator is also the wrong corpus for A4, whose worst latitude is s = √(k/(1+k)), and C2 uses Δ′ ⊥ z_i.

**Is "the union over N is attained to 1% from about 50 bits per block" what the proofs give?**
Yes as a sufficient condition, and only for: one block, one level, a random shared codebook, the plain rule,
N = 10⁹, ε = 10⁻⁹, L = 4 entering only through δ. It is not proved for any per-item rule (see C1), for several
blocks, for N·L, or for the built code. And 50 bits is the proof's slack, not a boundary: with the refinement of
step (c) and nothing else changed, the last term is below 0.01 from **30 bits at d = 95, 33 at d = 127, 40 at
d = 255, 41 at d = 287** (`ref_num2b.py`; at d = 95 and 29 bits, where M = 5.4·10⁸ < N, it is 0.026, so the union is
attained within 2% with fewer codewords than items).

**Lemma C4: no error found.** Given (z_1, z_2) the rotated query is uniform on the unit sphere of their orthogonal
complement (needs d − 1 ≥ 2 and y_1, y_2 independent), F_i ⊂ {W_i > q}, and W_1 + W_2 = √(2 + 2χ)·(a coordinate in
ℝ^(d−1)). χ₀ = 0.519, 0.450, 0.340, 0.327 and T/δ = e^(−58) at χ = 0, d = 255, reproduce. For a random codebook
the lemma is conditional on the codebook.

**Claim E: the M = 1 case is proved and simulated; the general conclusions drawn from it are not supported.**
- M = 1: correct. p(Δ′) = ½ on {τ ≥ q}, so π₂ ≥ ¼·T_{d+1}(q) (simulated 0.00213 against 0.00187).
- "R9 … needs the hypothesis M ≫ N·ln²(N/δ)" and C2.0's "false when it has fewer [codewords than items]":
  C2 gives a sufficient condition and nothing shows it necessary. By the refinement above it is not necessary,
  and at d = 95 the union is attained with M < N. The reason is geometric: an item fails through a codeword
  tilted by τ > q·sin φ off the query's equator, and such a codeword's cell holds little of the equator, less by a
  factor of order δ-to-a-power as δ falls. The simulations at d = 12, δ = 0.01 (q small) do not show this regime.
  Measured excess Pr[F_2 | F_1] − p̄ for a random shared code at n = 13 (`ref_sim_c.py` part D): 0.5/M, 0.25/M,
  0.14/M at M = 8, 64, 512 for δ = 0.01, and 0.35/M, 0.10/M, 0.05/M for δ = 0.002. The boundary depends on d and δ,
  not on M/N alone.

**C2.5 (against Molinaro, Woodruff and Yaroslavtsev).** Point 1 claims the constant "for per-item rules"; Theorem D
has it for the plain rule, one block. Point 3's boundary is the sufficient condition above. MWY not read by me.

## 7. The gaps G1, G2, G3

- **G1: correctly named as a gap; two sentences inside it claim more than is known.** "Two things are certain
  meanwhile": the pivot shows the bound of 𝓛 *can* be beaten through dependence on ŝ; that it can be beaten
  *only* so, and "by nothing when |s| ≪ 1", are the conjecture itself. The second "certain" thing is followed by
  "also not proved here". Nothing tagged proved depends on G1 provided every use of w* keeps "in 𝓛" (the C1
  summary does; the phrase "the corrected target" in N3 and the tax's "must cost" read wider than that).
  A remark that may help whoever attacks G1: for a scheme that starts with a Haar rotation, "for every y" is, in
  the rotated frame, an average over z = Rot·y uniform, so M5 is exactly the family of Bayes problems "Δ′
  uniform, z uniform on the latitude s of Δ′", one for each s. That is N3's prior, with no heuristic step.
- **G2: correctly stated.** The Jensen step needs d ≥ 3, and "m = 9, T_d(q₀) = 270·δ" needs δ < 1/540.
- **G3: correctly stated as a gap.** Dependent on it and tagged [model], as it should be: "the index is on the
  side where the union is unavoidable". That sentence also depends on the several-block version of C2 and on
  the N·L step, neither proved. In my small runs the sign code (n = 13) behaved like the 512-word random code
  (Pr[F_2 | F_1] = 0.0096 against p̄ = 0.0099), which says nothing about a tail of 10⁻¹¹.

## 8. One line per claim

| Claim | Verdict |
|---|---|
| M1 to M6 | No error found; real scalars (A4's φ) and B3's convention need the repairs of 1(a), 1(b) |
| F1, A1, A2, A3, Theorem A | No error found (checked by hand; the O(ln d/d) constants not checked) |
| A4 | No error found for real φ; inside the bit budget after repair 1(a). It is R8 widened to its value at Ê = 0; one event serves plain, A4 and R8; its upper end does not dominate the plain one |
| N1 | No error found; (c) is two-sided only, one-sided use needs Theorem 3.11's cap (2δ without it, simulated) |
| N2 | Correct, and holds for every code with half-width q_(d+1)(δ)·sin φ exactly; the table's "naive" column uses a different half-width (q·sin φ·cos φ) that is not valid on average either |
| B0, B1, B2 | No error found |
| B3 | Correct after repair 1(b) |
| Theorem B | No error found; a bound on the mean over y, in 𝓛, one block |
| B4 | Correct; the numerically checked step now has a proof (section 4); a limit statement, 8 to 15% off at the index |
| B5 | Sandwich correct; "exactly one halving" wrong for k not small (two-sided costs nothing extra at k ≥ 0.09, d = 255) |
| B6 and the table | No error found; three rows reproduced; the midpoint rule overstates the rigorous sum by 0.2% |
| Confidence tax | Algebra correct; the limit needs w ≪ 1/√d, not w ≪ q |
| Theorem C | Correct after repair: condition on (Δ′, codebooks), or on ω; rules must use rotations through Δ′ only. Given ω it holds for any i.i.d. corpus and is elementary |
| C1 | No error found; but C2.0's claim for every per-item rule is false (counter-example in section 6); the quantile form holds |
| C2 | No error found; step (c) valid and loose by ((1−q²)/(1−q²sin²θ))^((d−1)/2); the unproved inequality proved by Wendel, d ≥ 2 |
| C3 | Numbers reproduce (50 and 51 bits); "γ ≤ 0.05 at 48 bits" fails narrowly at d = 255, 287 |
| Theorem D | No error found for one block, one level, plain rule, random code; "factor N·L" is a sketch |
| C4 | No error found |
| Claim E | M = 1 proved and simulated; "needs M ≫ N·ln²(N/δ)" and "false with fewer codewords than items" not supported, and contradicted at d = 95 by the refined bound (30 bits suffice; 2% at 29 bits, M < N) |
| G1 | A gap, correctly named; its "two things are certain" are not both proved |
| G2 | Correctly stated; Jensen needs d ≥ 3 |
| G3 | Correctly stated; the [model] sentence also rests on the unproved several-block and N·L steps |

## 9. Not checked

- The comparison with TurboQuant, the reading of Molinaro–Woodruff–Yaroslavtsev, the Böröczky–Wintsche citation:
  all rest on the librarian's summaries, which I did not read.
- N3 (a sketch; I did not verify its posterior computation) and the conjecture in G1.
- The constants inside the O(ln d/d) terms of A3 and Theorem A.
- Table rows other than (95, 1), (255, ¼), (95, ¼); the steepness figures beyond the quantiles.
- R7's figures (10⁻⁶, e⁻¹⁵) and `codes/REPORT.md`'s figures, taken as quoted.
- `c2_union.py`, `c5a.py`, `c5b.py` and the c3/c4 files were not read; `c1_converse.py` was.
- Nothing was simulated at the index's sizes: every simulation is at n ≤ 25 and δ ≥ 0.001. Statements at
  δ = 2.5·10⁻¹⁹ are computed from the formulae.
- Floating-point rotations and pseudo-random Haar (chapter 3's conditions) are outside this report.
- The refined step (c) of section 6 is the referee's own and is itself unrefereed; it was checked by simulation at
  small sizes only.
