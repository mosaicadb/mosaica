# 3. The randomised levels: the error law, the guarantee, the constants

A randomised level codes the error that the deterministic prefix leaves, after a Haar rotation drawn from the build's
seed ω. Because the rotation is random, the direction of each block's coding error is uniform on a sphere, and the
failure probability of an interval built on that error is known exactly. This chapter gives that law (section 3.2),
the tails it implies (3.3), what a probability over ω does and does not promise (3.4), the guarantees for k-nearest-
neighbour (k-NN) queries (3.5) and for threshold sets (3.6), and the confidence constants that turn a guarantee into a
half-width (3.7). Sections 3.8 to 3.10 give the measured effect of the constants, the constants the index uses, and the
prior work the chapter builds on. Chapter 1 treats the deterministic certificates in front of these levels, and chapter
5 the approximate mode that reuses their intervals.

## Local symbols

Chapter 0's symbols hold. This chapter adds the following.

| Symbol | Meaning |
|---|---|
| ō_j | the codeword stored for block j, in rotated coordinates; any function of R_j y_j |
| ŷ_j, E_j | the block's estimate (‖y_j‖²/⟨ō_j, R_j y_j⟩)·R_jᵀō_j and its error E_j = ŷ_j − y_j |
| u_j | the unit vector with E_j = A_j u_j |
| b_j | ‖y_j‖·‖Δ_j‖, the Cauchy–Schwarz bound on block j's contribution |
| v_j, v, Ê | the true contribution Δ_j·y_j, the total v = Σ_j v_j, and its estimate |
| w_j | a share a_j²/(d_j·sd²), so Σ_j w_j = 1; c(w) = H_δ(a)/sd at shares w |
| δ_d(c) | Pr[U ≥ c/√d] for U one coordinate of a uniform unit vector in ℝ^d: the one-sided tail at c sd; δ(c) when d is fixed |
| c(E) | the constant with E·δ(c) = ε, a union over E one-sided events |
| Θ, ξ | Θ = Σ_j d_j/tan²φ_j; ξ = c/√Θ, which is c·tan φ/√d for one block |
| 𝒯_k, s_k | the true top k of the visible set (ties broken by id) and the true k-th score (τ_k of chapter 0) |
| 𝒰 | a set of (item, level) pairs |
| U₀, N₀ | the visible items that reach a randomised level, and their number |
| c_in, c_ex | the constants of lower bounds (certify in) and of upper bounds (exclude) |
| sd_ℓ(x) | item x's error sd at level ℓ |
| n_q, B_z | the number of queries a client issues; the bits of side information it observes |
| k_W, k′ | the dimension of a subspace W; the number of sign bits of the sub-bit sketch |
| ρ | the correlation of one item's errors at two levels |
| Λ_d | the log moment generating function of U in ℝ^d |
| J | the number of blocks coded at a level |
| ω_m | the volume of the unit ball of ℝᵐ, always subscripted; bare ω is the seed |
| p_x | an item's distance from t in units of its scale (T4's b and T5's p in the second pass) |
| d (unsubscripted, in §3.2, §3.3, Theorem 3.11 and δ_d) | the degrees of freedom of a single block, d_j with j dropped; where §3.4 and §3.6 speak of a tolerance d, d is chapter 0's tolerance |
| m, s (Propositions 3.2 and 3.3) | the dimension of a sphere's space and a tail level; elsewhere m is a centre and s(x) a score, as in chapter 0 |
| D_ℓ | the number of items still undecided after level ℓ (chapter 1's D_ℓ, a level's weights, does not appear here) |
| K (Proposition 3.17 and its rule) | the factor 2/(1 − 2δ(c)); in the certificate method, K is a grid resolution; chapter 0's cluster count K does not appear here |
| M, M_j | the number of subspaces in a union (after Theorem 3.5) and the caps of Theorem 3.15; chapter 0's metric M does not appear here |
| G | the event of Theorem 3.14, and the surge referee's gap in S(t + G) ⊆ R ⊆ S(t − G) (§3.6); chapter 0's group count G does not appear here |
| β, P | the scale bracket and the set of near items of Theorem 3.16 |
| η | a slack: the radial-error and floating-point slacks after Lemma 3.1, and the lattice step of the certificate method (chapter 1's item scale η does not appear here) |
| β_j, ν_j, θ, ϱ_j, C (Theorem 3.21 only) | block norms, the mean Beta factors, the constant with s ≤ θ·√(d_Σ)·sd, the norm shares in the proof, and the normalising constant of U_D |
| F (Theorem 3.22 only) | the function v ↦ H_δ(√(v_1 d_1), …)²; F with a subscript is an event, and chapter 0's F = Σᵢ √γᵢ does not appear here |

The sources write m_b (consolidated proofs) or m (level-0 report) for d_j; the level-0 report's single block spans all
n dimensions, so its m is n − 1, and its w_j is ‖Δ‖ = ‖q − αm‖. The second pass writes delta(c) for δ(c). "Two-sided"
means the budget δ = ε/(2NL) per side, which the code used before the third pass; "one-sided" means δ = ε/(NL) on the
harmful side only.

## 3.1 The randomised levels as built

The index has L = 4 randomised levels ending at 56, 96, 160 and 256 bytes per item (SYSTEM.md section 2.1). They code
the profile-weighted error the 32-byte deterministic prefix leaves, split into four blocks at 1/8, 7/24 and 5/8 of n:
96, 128, 256 and 288 coordinates at n = 768, so d_j = 95, 127, 255, 287. Each block has its own Haar rotation drawn
from ω and shared by every group. Each rotated coordinate is coded as a nested binary expansion whose rate only rises
from one level to the next. The first randomised level stores ‖y_j‖ per block in 8 bits, and each rise of a block
stores the estimator's scale κ_j and tan φ_j, both rounded up against fitted tables. A survivor's half-width at level ℓ
is

    h = (c_ℓ·sd + deterministic terms)·(1 + 10⁻⁶),   sd² = Σ_j (‖y_j‖·tan φ_j·‖Δ_j‖)²/d_j,

where the deterministic terms cover the prefix's error, table and κ rounding, floating-point evaluation and, for a block
not yet coded, ‖y_j‖·‖Δ_j‖ by Cauchy–Schwarz (chapter 1 §1.6 describes the codes and these terms). An item is taken in when its lower bound reaches t and left out when its
upper bound is below t. Items undecided after 256 bytes are scored from their f16 vector (SYSTEM.md section 2.2).

## 3.2 The error law

**Lemma 3.1 (Lemma 8 of the consolidated proofs).** Let y be an item's block, R a Haar rotation, ō any codeword that is
a function of Ry, φ the angle between ō and Ry, and ŷ = (‖y‖²/⟨ō, Ry⟩)·Rᵀō. The error E = ŷ − y is orthogonal to y, has
norm A = ‖y‖·tan φ, and E = A·u with u uniform on the unit sphere of y^⊥ and independent of A. This holds for any
codebook.

*Proof.* ⟨ŷ, y⟩ = ‖y‖², so E ⟂ y and ‖E‖ = ‖y‖·tan φ. Let O be a rotation fixing y. Then R·Oᵀ is Haar and maps y to Ry,
so it gives the same code and the same A, and its estimate is O·ŷ. So, conditional on Ry, the law of E is invariant
under every rotation fixing y. ∎

[proved, refereed] `proofcheck/CONSOLIDATED_PROOFS.md` section 5; `reports/surge/referee.txt` section 2.

For a query weight Δ_j fixed independently of ω, Lemma 3.1 gives Δ_j·E_j = A_j·‖P_{y⊥}Δ_j‖·U_j, with U_j one coordinate
of a uniform unit vector in ℝ^(d_j). Blocks with separate rotations have independent U_j given the A_j. Since
‖P_{y⊥}Δ_j‖ ≤ ‖Δ_j‖ and E U_j² = 1/d_j, sd² bounds the variance of the level's error, with equality when every Δ_j is
orthogonal to y_j.

The referees attach conditions to the lemma's use, because its proof needs the code to be a function of the item's own
rotated vector and of nothing else that depends on ω (`reports/surge/referee.txt` section 2,
`reports/bounds3/referee/REFEREE.txt` item 3).

- A grid step fitted on 20,000 rotated items, as `lawcheck.py` and `lvl0c.py` did, makes the codebook depend on ω
  through other items. The referee could not make this show at n = 8, but it is unproved. The repair is a per-item
  scale, or a union over the 13 grid values (c = 6.25 / 6.69 / 6.90 at n = 100 / 256 / 768).
- An estimator scale derived from rounded scalars adds a radial error (f′/f − 1)·Ê, which a width rounded up does not
  cover when the residual is nearly parallel to the offset and Ê is negative. At n = 8, |v|/b = 0.995 and a one-byte
  cos φ, upper-bound failures ran at 1.1 to 4 times the exact law (`lawmc3.py`). The repair adds η·|Ê| with
  η ≥ |1 − f/f′|, or brackets 1/f.
- With scalars calibrated on other rotated items, U_j is not uniform conditional on the scalars. The argument that holds
  is pointwise: the true a is a function of the item's own rotated vector, and h ≥ H_δ(a) holds pointwise when h is
  non-decreasing in ā and ā ≥ a.
- The rotation is computed in floating point. The polar factor P* of the computed P satisfies ‖P − P*‖₂ ≤ ‖PᵀP − I‖_F,
  and the error of a computed matrix–vector product paired with a code is up to n^1.5·2^−53·‖ō‖·‖q‖, so the width
  carries η + n^1.5·2^−52 as slack. Because the code is computed from the rounded product, not from P*r, the failure
  probability is at most δ·E[2^Q], with Q the number of coordinates within rounding of a quantiser boundary or of a tie
  between two grid steps: a factor of about 1 + 10⁻⁸.
- Haar is a model of a pseudo-random rotation from a 64-bit seed, so probabilities near 2^−64 mean nothing. The
  one-sided budget at N = 10⁹ and L = 4 is 2.5·10⁻¹⁹ per event, about 2^−62 (`reports/bounds3/REPORT.txt` section 8).

The law has been checked on data [measured]. Over 5.8·10⁷ nytimes (d = 255) and 4.7·10⁷ glove (d = 99) query-item pairs
with 1- and 2-bit codes, z = (v − Ê)/(sd·‖P_{y⊥}Δ‖) has standard deviation 0.9999–1.0000, and its tail follows the
sphere, not the Gaussian (`reports/surge/level0/lawcheck.py`):

| | Pr[\|z\| > 3]: measured / law / Gaussian | Pr[\|z\| > 4]: measured / law / Gaussian |
|---|---|---|
| nytimes | 2.53·10⁻³ / 2.54·10⁻³ / 2.70·10⁻³ | 5.01·10⁻⁵ / 5.06·10⁻⁵ / 6.33·10⁻⁵ |
| glove | 2.30·10⁻³ / 2.30·10⁻³ / 2.70·10⁻³ | 3.3–3.6·10⁻⁵ / 3.38·10⁻⁵ / 6.33·10⁻⁵ |

At n = 8, with 3 and 4 million rotations and 1- and 3-bit codes, the one-sided frequencies at s = 0.3 to 0.9 lay within
2.8 standard errors of the law. On the 50k BioCLIP-2 index the standardised error has kurtosis 2.988, the value the
block law predicts, 3 − 6·Σ_j w_j²/(d_j + 2) (`reports/bounds3/scale_check.py`).

## 3.3 Tails

The exact one-sided tail of one coordinate is Pr[U ≥ s] = ½·I_{1−s²}((d − 1)/2, ½), the regularised incomplete beta
function, for U in ℝ^d. The level-0 report states it and the referee confirmed it to six digits between tails of 10⁻³
and 10⁻¹⁹, and that d = n − 1 is the right dimension for one block over all n coordinates (using n would move c by 0.02
at n = 100). [proved, refereed] `reports/surge/level0/REPORT.txt` section 1; `reports/surge/referee.txt` section 2.

**Proposition 3.2 (Proposition 10 of the consolidated proofs).** Let u be uniform on the unit sphere of ℝᵐ, m ≥ 2. For
0 ≤ s ≤ 1/√2, Pr[|u₁| ≥ s] ≤ 2(1 − s²)^(m/2).

*Proof.* Let C = {u : u₁ ≥ s}. Its measure is the volume of the cone {τu : u ∈ C, 0 ≤ τ ≤ 1} divided by ω_m. For a point
τu of the cone, ‖τu − s·e₁‖² = τ² − 2τs·u₁ + s² ≤ τ² − 2τs² + s². This is convex in τ. At τ = 1 it equals 1 − s², and at
τ = 0 it equals s², which is at most 1 − s² because s ≤ 1/√2. So the cone lies in a ball of radius √(1 − s²), and
Pr[u₁ ≥ s] ≤ (1 − s²)^(m/2). Double for the two sides. ∎

[proved, refereed] `proofcheck/CONSOLIDATED_PROOFS.md` section 5; `proofcheck/REFEREE.md` ("Exact tail").
Conjecture 2.18 of chapter 2 is this bound without the factor 2 (see Open).

The restriction on s is needed, because for m = 2 the inequality fails from s ≈ 0.946. Since (1 − s²)^(m/2) ≤
exp(−ms²/2), the proposition sharpens the sub-Gaussian tail. Solving the exact tail instead of 2·exp(−c²/2) at the same
failure probability multiplies c by 0.741 / 0.638 / 0.537 at m = 30 and by 0.915 / 0.936 / 0.940 at m = 767, for
c = 6 / 8 / 10 [computed, `proofcheck/constants.py`, reproduced by the referee]. The closed bound is still worth less
than the numerical tail: for k = 10 neighbours and three levels it gives 6.25 / 6.72 / 6.93 / 6.99 at n = 100 / 256 /
768 / 1,536 against 5.99 / 6.37 / 6.54 / 6.59 [computed, `reports/surge/referee.txt` section 2].

**Proposition 3.3 (Proposition 11 of the consolidated proofs).** For u uniform on the unit sphere of ℝᵐ,
E exp(λu₁) = ₀F₁(; m/2; λ²/4) ≤ exp(λ²/(2m)). Hence, for independent blocks with scales a_j = A_j‖Δ_j‖ and dimensions
d_j, Pr[|Σ_j a_j U_j| ≥ c·√(Σ_j a_j²/d_j)] ≤ 2·exp(−c²/2), and the Chernoff bound with the exact function is at least
as tight.

*Proof.* The series Σ_i (λ²/4)^i/(i!·(m/2)_i) is the stated function. Since (m/2)_i ≥ (m/2)^i, each term is at most
(λ²/(2m))^i/i!. Blocks use independent rotations, so by Lemma 3.1 the U_j are independent given the A_j. Apply
Chernoff. ∎

[proved, refereed] `proofcheck/CONSOLIDATED_PROOFS.md` section 5; see Editor's query 1. The series matches Monte Carlo
at m = 5 and 30 (`proofcheck/constants.py`).

**Proposition 3.4 (Proposition 13 of the consolidated proofs; the sub-bit sketch).** Keep the signs of the first k′ of
n_j rotated coordinates. Then cos φ = Σ_{i≤k′}|z_i|/(√k′·‖z‖) with z = R_j y_j, and as k′ → ∞ with n_j ≥ k′,
tan²φ/(π n_j/(2k′) − 1) → 1 in probability. The block error then has standard deviation close to
‖y_j‖·‖Δ_j‖·√(π/(2k′) − 1/n_j).

*Proof.* z/‖z‖ has the law of G/‖G‖ for a standard Gaussian G. By the law of large numbers (1/k′)Σ_{i≤k′}|G_i| → √(2/π)
and ‖G‖²/n_j → 1, so cos²φ·π n_j/(2k′) → 1. The denominator π n_j/(2k′) − 1 is at least π/2 − 1, so the ratio
converges. By Lemma 3.1 the error variance is A²·‖P_{y⊥}Δ_j‖²/(n_j − 1). ∎

[proved, refereed] `proofcheck/CONSOLIDATED_PROOFS.md` section 5; `proofcheck/REFEREE.md` ("Sub-bit law: correct
asymptotically in k′").

E tan²φ is infinite for k′ ≤ 2, and the mean exceeds the formula by 78% at k′ = 4, 11% at k′ = 16 and 2% at k′ = 64
[computed, `proofcheck/misc.py` (b)]. The sketch beats a deterministic bound on the same block once k′ exceeds c²π/2,
about 100 bits at c = 8 and 157 at c = 10, and on 2 million synthetic pairs it matched the prediction to three digits
[measured, `agent3a/exp_d.py`].

**Theorem 3.5 (Theorem 9 of the consolidated proofs; class-uniform bound).** Let d_j ≥ 3 and let W be a subspace of the
block of dimension k_W, fixed independently of ω. With probability at least 1 − exp(−c²/2), simultaneously for every Δ,

    |⟨E, Δ⟩| ≤ A·[ ‖P_W Δ‖·(√(k_W/d_j) + c/√(d_j − 2)) + ‖P_W^⊥ Δ‖ ].

*Proof.* |⟨u, Δ⟩| ≤ ‖P_W u‖·‖P_W Δ‖ + ‖P_W^⊥ Δ‖ since ‖u‖ = 1. The map u ↦ ‖P_W u‖ is 1-Lipschitz, and
E‖P_W u‖² = (k_W − ‖P_W y‖²/‖y‖²)/d_j ≤ k_W/d_j, so E‖P_W u‖ ≤ √(k_W/d_j). On the unit sphere of a d_j-dimensional space,
a 1-Lipschitz F satisfies Pr[F ≥ E F + t] ≤ exp(−(d_j − 2)t²/2) (Lévy; Ledoux, *The Concentration of Measure
Phenomenon*, Theorem 2.3). Take t = c/√(d_j − 2). ∎

[proved, refereed] `proofcheck/CONSOLIDATED_PROOFS.md` section 5; `proofcheck/REFEREE.md` ("T4").
Conjecture 2.17 of chapter 2 asks whether the factor √k_W is unavoidable for every such class.

For a union of M such subspaces, replace c² by c² + 2 ln M. The bound holds for all Δ at once, so it covers queries
chosen with knowledge of ω, provided W and the items are independent of ω. The exact Beta law of ‖P_W u‖² stays below
the bound in every case tried (`proofcheck/misc.py` (c)). On the 200k BioCLIP-2 subset, real query offsets are
full-dimensional in the whitened blocks (participation ratio 103 of 128 and 160 of 256), so a subspace class gives a
width of 0.88–0.99 of plain Cauchy–Schwarz against 0.47–0.71 for the per-query bound [measured, `agent3a/REPORT.md`].
The half-width, as a multiple of A‖Δ‖, is c/√d_j for one query, √(c² + 2 ln M)/√d_j for a finite class of M queries,
(√k_W + c)/√d_j for a subspace class and √(c² + 1.39·B_z)/√d_j after B_z leaked bits.

## 3.4 What a probability over the seed promises

Every guarantee in this chapter is a probability over ω for a query fixed before ω is drawn. The next three results say
how far that extends to a client that adapts, to a client that sees more than answers, and to a decoder with its own
randomness.

**Theorem 3.6 (Theorem 5 of the consolidated proofs; adaptive queries, d = 0).** Suppose Pr_ω[R(q, t, ω) ≠ S(q, t)] ≤ ε
for every (q, t) fixed independently of ω. Let a client issue n_q queries, each a function of its own coins and the
earlier answers, with the coins independent of ω. Then Pr[some answer is wrong] ≤ n_q·ε.

*Proof.* Fix the coins. Let (q*_i, t*_i) be the queries the client issues when answered by the exact oracle S. They do
not depend on ω. Let F_i = {R(q*_i, t*_i, ω) = S(q*_i, t*_i)}. On F_1 ∩ … ∩ F_{i−1} the real answers so far equal the
exact ones, so the real i-th query is (q*_i, t*_i). A first wrong answer at step i therefore implies the complement of
F_i. Apply the union bound and average over the coins. ∎

[proved, refereed] `proofcheck/CONSOLIDATED_PROOFS.md` section 5; `proofcheck/REFEREE.md` ("T1").

The proof uses that a correct answer is unique, so it does not extend to d > 0. With d > 0 and randomised certificates,
whether an item in the band [t − d, t) is returned depends on its estimate and so on ω, and a binary search on t reads
the estimate. The fix is a seed-independent answer rule, either d = 0 or band items decided from a public deterministic
code, whose cost was not measured. [proved, unrefereed] `agent3a/REPORT.md` ("d > 0 is not adaptively safe", tagged
there "measured + proved" and not independently verified); `proofcheck/REFEREE.md` confirms only that Theorem 5 fails
as written for d > 0.

**Theorem 3.7 (Theorem 6 of the consolidated proofs; leakage).** If the client also observes side information whose
whole transcript takes at most 2^(B_z) values, then Pr[some answer is wrong] ≤ 2^(B_z)·n_q·ε. With ε proportional to
exp(−c²/2), replacing c² by c² + 2·B_z·ln 2 restores the bound.

*Proof.* For each transcript value z define the ideal sequence by running the client against S with side information
z. These are 2^(B_z)·n_q queries fixed independently of ω. If all are answered correctly, the real run follows the
ideal sequence of the real transcript and every answer is correct. Apply the union bound. ∎

[proved, refereed] `proofcheck/CONSOLIDATED_PROOFS.md` section 5; `proofcheck/REFEREE.md` ("T2: the side information
must be the whole transcript").

The bound is close to tight [measured, `agent3a/exp_b.py`]. Predicting one item's error from B_z leaked sign bits of
its error vector reaches 3.0 / 5.6 / 8.4 / 10.3 / 11.0 sd at B_z = 16 / 64 / 256 / 1,024 / 4,096 for n_j = 128
(deterministic ceiling 11.3), and 3.1 / 6.0 / 10.0 / 13.6 / 15.2 for n_j = 256 (ceiling 16.0). A constant of 8 therefore
falls to about 130 leaked bits, and 10 to 250–900. Latency and bytes read depend on ω, so a client that adapts on them
needs Theorem 3.7 (`reports/surge/referee.txt` section 1).

**Theorem 3.8 (Theorem 7 of the consolidated proofs; query-time randomness does not help).** Fix the bytes read for an
item, and let F be the set of vectors that store those bytes. If min_F q·x′ < t ≤ max_F q·x′, any decoder, randomised
or not, that reads no more errs with probability at least ½ on some corpus.

*Proof.* Take x₁, x₂ ∈ F on opposite sides of t. The decoder's input is identical on the two corpora, so it includes the
item with the same probability p. Its error probabilities are 1 − p and p. ∎

The argument is the indistinguishability argument of chapter 2's Theorem 2.11, applied to a randomised decoder.

[proved, refereed] `proofcheck/CONSOLIDATED_PROOFS.md` section 5; `proofcheck/REFEREE.md` ("T3").

**Remark (public seed).** Theorems 3.6 and 3.7 need the client's coins, and the corpus, to be independent of ω. A party
that reads the seed is outside Theorem 3.6, makes Theorem 3.7 vacuous, and faces a scheme that is deterministic to it,
so Theorem 3.8 applies and no guarantee below the deterministic certificate can hold against it. With live ingest,
someone who reads the seed can insert items built to fail for a fixed query, which none of these results covers. Centres
refitted from a corpus that contains such items make other items' residuals depend on ω, so a new rotation must be
drawn whenever centres are refitted. Theorem 3.5 is the only result that holds for seed-dependent queries, provided W
and the items are independent of ω. [proved, refereed] `proofcheck/CONSOLIDATED_PROOFS.md` section 5;
`proofcheck/REFEREE.md` ("T1–T4 and the public seed"); `reports/surge/referee.txt` section 1.

**Proposition 3.9 (Proposition 12 of the consolidated proofs; union over undecided items).** Let 𝒰 be a set of (item,
level) pairs that does not depend on ω, such that every decision outside 𝒰 is correct for every ω. If each randomised
interval fails with probability at most 2·exp(−c²/2), then Pr[some decision is wrong] ≤ 2·|𝒰|·exp(−c²/2).

*Proof.* A wrong decision occurs at a pair in 𝒰 whose interval fails. 𝒰 is fixed, so the union bound applies. ∎

[proved, refereed] `proofcheck/CONSOLIDATED_PROOFS.md` section 5; `proofcheck/REFEREE.md` ("Other checks"). A
deterministic certificate computed on seed-rotated codes depends on ω and cannot define 𝒰. Because 𝒰 depends on
(q, t), the proposition bounds a failure probability reported for the query, not one promised before it.

## 3.5 Exact k-NN: a union over k items

**Theorem 3.10 (Theorem A of the level-0 report, as repaired by the referee; a k-NN needs a union over k items).**

*Setting.* The corpus, the centres and the assignment, the visible set V after every filter and the query q are fixed
independently of the rotation, which is Haar. s(x) is the one score function used for full reads, and 𝒯_k is the
first k entities of V in the order (s descending, id ascending). c is fixed before the query from (n, k, L, ε) only.
For each item and level, the upper bound U_ℓ(x) is computed from q, the item's level-ℓ code and scalars, per-cluster
constants and c, and from nothing else.

*Algorithm class.* (a) Every threshold t̂ used in a drop test is at most the k-th largest s among distinct visible
entities scored exactly so far (−∞ with fewer than k). (b) An item is discarded only by the strict test U_ℓ(x) < t̂, or
by a bound that holds for every rotation. (c) Every visible item not discarded is scored exactly. (d) The result is the
first k scored entities in the same order.

*Claim.* Pr_ω[result ≠ 𝒯_k] ≤ Σ_{x* ∈ 𝒯_k} Σ_ℓ Pr[U_ℓ(x*) < s(x*)] ≤ k·L·δ.

*Proof.* t̂ ≤ s_k always, by (a), because t̂ is the k-th best of a subset. If no upper bound of a member of 𝒯_k fails,
then U_ℓ(x*) ≥ s(x*) ≥ s_k ≥ t̂ for every x* ∈ 𝒯_k, so by (b) no member is discarded, by (c) all are scored, and by (d)
the result is 𝒯_k. Apply the union bound over the fixed set 𝒯_k × levels, which is Proposition 3.9 with 𝒰 = 𝒯_k ×
levels. ∎

[proved, refereed] `reports/surge/level0/REPORT.txt` section 1; repaired statement in `reports/surge/referee.txt`
section 1.

For each fixed pair (x*, ℓ) the failure event is a function of ω alone, defined whether or not the algorithm evaluates
that bound, and the implication "answer wrong ⇒ some such event" holds one rotation at a time. Which bounds are
evaluated, in what order and against which t̂ therefore cannot matter, and t̂ may depend on ω. The referee lists six
conditions, each of which can break the theorem when left out:

- (a) t̂ counts distinct entities. If one entity can be scored through two rows (spilled copies, two versions of an
  upserted item, a deleted or suppressed row), the k-th best over rows can exceed s_k.
- (b) The drop test is strict, with one order; "≤ t̂" can drop a tied neighbour.
- (c) The bound bounds the score that full reads compute, bit for bit.
- (d) One event per bound direction. A level that uses the uncorrected estimator in the kernel and the centre-corrected
  one in the re-check has two events, so L = 4 and c = 6.02 / 6.41 / 6.58 in place of 5.99 / 6.37 / 6.54.
- (e) No randomised lower bound may raise t̂ or admit an item, and no randomised bound may skip a cluster.
- (f) The filter is independent of the rotation. A filter built from an exact-mode result is fine with its ε added; one
  built from an approximate-mode result voids the bound.

A Monte Carlo on the theorem's algorithm class confirms it [measured, `reports/surge/referee_work/thmA_mc.py`: n = 16,
1,501 items, 12 clusters, k = 5, two levels, 60% visible, a tie at rank k, 8 queries × 3,000 rotations, c = 2.2]. Some
true neighbour's bound failed in 0.1042 of runs against k·L·δ = 0.1085, the answer was wrong in 0.0102 (serial) and
0.0064 (two thread-local thresholds), and no answer was wrong while every true neighbour's bound held, in 24,000 runs of
each. A level-1 width of c·‖Δ‖·sd applied to the estimator without the centre term e_m, in contrast, is unsound,
because that estimator's error is A·‖P_{r⊥}q‖·U with ‖P_{r⊥}q‖ up to 1, and ‖Δ‖ bounds it only for the centre-corrected
estimator. In the same Monte Carlo it gave 810 wrong answers with every correct bound holding. On glove, where
‖Δ‖ = 0.80, its effective constant is 4.80 in place of 6, a per-event tail of 1.9·10⁻⁷ against a budget of 1.7·10⁻¹¹
(11,500 times); on nytimes, at ‖Δ‖ = 0.93, it is 27 times (`reports/surge/referee.txt` section 2).

Because the theorem uses only the upper bounds of 𝒯_k, one side of each interval suffices, and the union is k·L
one-sided events. The guarantee it gives, which the referee wrote for the owner to accept, is this: with probability at
least 1 − ε over ω the result is exactly the k best visible items, and every returned score is exact in every case.
What a user could see with probability ε is one or more true neighbours missing, each replaced by the next best item;
never a wrong score and never an invisible item. A missing item's score is below the deterministic upper end computed at
its drop. The miss repeats on every run of that query, and for every viewer whose top k contains the item, until the
item is re-encoded. Theorem 3.6 applies because 𝒯_k is unique.

Five further facts about the k-NN union come from the second pass (`reports/surge/bounds2.txt` section 2).

- *The union over k is tight* [proved by example; measured]. k matches tied at the k-th score with unrelated residuals
  fail nearly independently, and a toy with the oracle constant c(|S|) fails at 0.093–0.101 at ε = 0.1
  (`simcap.py`, n = 16, 40,000 seeds).
- *Levels* [proved, unrefereed]. An item is dropped once, so its drop events at different levels are disjoint, and the
  only slack is the split of ε across levels, which is sound for any fixed weights.
- *Gaps between neighbours* [refuted as a constant]. The i-th neighbour is lost only if its bound fails by the gap
  s_i − s_k as well, but the gaps are unknown until the answer is known. A rank-dependent rule is unsound, a posterior
  check on the returned gaps is unsound because losing a tied neighbour makes the gaps look safe, and lowering t̂ to
  manufacture a gap is the same as raising c. The most the gaps could give is c(kL) − c(L): 0.26 / 0.31 / 0.34 at
  k = 10 and 0.50 / 0.61 / 0.67 at k = 100, for n = 100 / 256 / 768.
- *Independence across items under one rotation* [refuted]. Two items' errors are images of fixed vectors under one
  Haar isometry, so their Gram matrix is fixed by the codes. The measured correlation of normalised errors is
  0.00 / 0.02 / 0.09 / 0.38 / 0.77 / 0.92 at residual cosine 0 / 0.3 / 0.6 / 0.9 / 0.99 / 0.999 (`pairdep.py`, n = 100,
  1 bit, 400,000 rotations). Independence would not help in any case, because for independent events
  Pr[union] ≥ sum − sum²/2, so the union bound loses only a factor 1 − ε/2.
- *One-sided tail* [proved, unrefereed; see Editor's query 12]. Theorem 3.11's upper end needs only the event
  v − Ê ≤ ξ√(b² − v²): the left side minus the right is decreasing and then increasing in v, so the event implies that
  v is at most the larger root. The surge referee found, a day earlier, that with the upper event alone the formula's
  end is wrong when Ê > b and right only with the cap at b (section 3.5, below Theorem 3.11), so the one-sided constants
  are sound only with that cap.

Together the one-sided tail and an uneven split of ε over levels are worth about 1% of bytes and 5–12% of first-level
survivors on glove, because the 1-bit pass over every item is 89% of the bytes [model, from the level-0 counts].

**Constants for k-NN** [computed; `reports/surge/level0/consts.py`, reproduced in `reports/surge/referee.txt`
section 2; the rows "all N = 10⁹ × 3, one-sided", "k = 10 × 3, one-sided" at n = 1,536 and "k = 10 × 4" from the
referee's recomputation; the row "level 1 holds 0.9ε" from `reports/surge/bounds2/consts2.py`]. ε = 10⁻⁹ unless
stated; one rotation over all n dimensions.

| Events | sub-Gaussian | n = 100 | n = 256 | n = 768 | n = 1,536 |
|---|---|---|---|---|---|
| all N × 5 levels, two-sided (before) | 9.35 | 7.48 | 8.36 | 8.79 | 8.90 |
| all N = 10⁹ × 3, one-sided | 9.22 | 7.41 | 8.26 | 8.66 | 8.77 |
| k = 10 × 3, two-sided | 7.05 | 5.99 | 6.37 | 6.54 | 6.59 |
| k = 10 × 3, one-sided | 6.95 | 5.92 | 6.28 | 6.44 | 6.49 |
| k = 10 × 3, one-sided, level 1 holds 0.9ε | 6.80 | 5.81 | 6.15 | 6.30 | |
| k = 10 × 4, two-sided (condition d) | 7.09 | 6.02 | 6.41 | 6.58 | 6.63 |
| k = 100 × 3, two-sided | 7.36 | 6.23 | 6.66 | 6.86 | 6.91 |
| k = 1,000 × 3, two-sided | 7.67 | 6.44 | 6.94 | 7.16 | 7.22 |
| k = 10 × 3, two-sided, ε = 10⁻⁶ | 5.98 | 5.16 | 5.38 | 5.47 | 5.50 |
| deterministic cap √(n − 1) | | 9.95 | 15.97 | 27.69 | 39.18 |

At n = 100 the sub-Gaussian 9.35 is 94% of the deterministic cap, which is why it pruned nothing. A guarantee over
100,000 queries at 10⁻⁹ in total needs c = 7.00 / 7.68 / 7.99 at n = 100 / 256 / 768.

### The centre-corrected, self-normalised interval

**Theorem 3.11 (Theorem B of the level-0 report).** Consider one rotated block over the residual, y = r, with centre m.

(i) *Centre correction.* Take Δ = q − αm with α = q·m/‖m‖², fixed per (query, cluster) and independent of ω. Then
q·x = q·m + α·(m·r) + Δ·r with m·r known, and ‖Δ‖ = √(1 − (q·m̂)²) ≤ 1 for unit q (chapter 1's Lemma 1.1 and
Proposition 1.3 (a)). The estimate of Δ·r is
f⟨ō, Rq⟩ + α(e_m − m·r), where f = ‖r‖/⟨ō, R r̂⟩ and e_m = m·r − f⟨ō, Rm⟩ is one stored scalar per item.

(ii) *Self-normalised width.* With v = Δ·r the unknown and b = ‖r‖‖Δ‖, ‖P_{r⊥}Δ‖² = ‖Δ‖² − v²/‖r‖², so the event
|U| ≤ c/√d is exactly the event |v − Ê| ≤ ξ·√(b² − v²), with ξ = c·tan φ/√d. Solving the quadratic gives the interval
ends (Ê ± ξ√((1 + ξ²)b² − Ê²))/(1 + ξ²).

(iii) The same formula with ξ = tan φ is a deterministic interval, which holds for every rotation and contains the
randomised one whenever c ≤ √d.

*Repaired statement of (ii), upper end.* On the event |U| ≤ c/√d of the item's level-ℓ code, with ξ ≥ c·tan φ/√d,
b̄ ≥ ‖r‖‖Δ‖ and Ē ≥ Ê: v ≤ b̄ if Ē ≥ b̄; otherwise v ≤ min(b̄, (Ē + ξ√((1 + ξ²)b̄² − Ē²))/(1 + ξ²)) when the root is
real, and the item is kept when it is not.

*Proof of (ii).* By Lemma 3.1 the error of Ê is A·‖P_{r⊥}Δ‖·U, and A·‖P_{r⊥}Δ‖ = ‖r‖tan φ·√(‖Δ‖² − v²/‖r‖²) =
tan φ·√(b² − v²). So |U| ≤ c/√d is the stated event. Squaring gives (1 + ξ²)v² − 2Êv + Ê² − ξ²b² ≤ 0, whose roots are
the stated ends. ∎

[proved, refereed] (i), (ii) as repaired, (iii): `reports/surge/level0/REPORT.txt` section 1; `reports/surge/referee.txt`
section 3, which checked the two-sided solution set against a grid in 20,000 random cases with no mismatch.

The referee's three repairs are these. A negative discriminant, Ê² > (1 + ξ²)b², means the event has failed, and the
formula gives no number, so the item is kept or the deterministic interval is used. The upper end increases with Ê
only while Ê ≤ b, so rounding Ê up narrows the bound beyond b, and the end is set to b there. With only the upper event,
the formula's end is wrong when Ê > b (1,931 of 20,000 cases, by up to 0.40·b), and the cap at b makes it right in all
cases; the one-sided k-NN constants need that cap.

The level-0 report also claimed a backstop: a missed neighbour satisfies q·x* < t̂ + (√d − c)·‖Δ‖·sd(x*). That holds
for the plain width [proved, refereed]. For the self-normalised width it is [refuted]: the gap between the deterministic
and randomised ends exceeds (√d − c)·sd in 32% of random cases with Ê < 0 and 0.8% with Ê > b, by up to 8.9 times
(b = 1, ξ = 0.3, tan φ = 0.5, Ê = −1.044: ends −0.675 and −0.956, gap 0.280 against 0.200), and never for 0 ≤ Ê ≤ b. What
holds with certainty is that a missed neighbour's score is at most its deterministic upper end; the closed-form gap holds
when 0 ≤ Ê ≤ b (`reports/surge/referee.txt` section 3).

On glove (N = 1,183,514, k = 10, all clusters scanned) the interval of Theorem 3.11 cuts survivors by 1.2 to 2.2 times
at the same constant [measured, `reports/surge/level0/lvl0c.py`]. With the plain width the 2-bit level leaves 105k
survivors at c = 9.35; with Theorem 3.11 at c = 6 it leaves 5.8k, and the 1- and 4-bit levels leave 131k and 56.
Around c = 6 to 9 each unit of c roughly doubles the 2-bit survivors. On nytimes (N = 290,000) the 2-bit level leaves
23.8k at 9.35 plain and 3.57k at 6.5 with Theorem 3.11.

**Theorem 3.12 (R8 of the third constants pass; Theorem B for several blocks).** Let Θ_j = d_j b_j²/a_j², Θ = Σ_j Θ_j =
Σ_j d_j/tan²φ_j, which does not depend on the query, and ξ² = c²/Θ with c ≥ c*. On the event {v − Ê ≤ c × true sd},
v ≤ (Ê + √(c²·sd²·(1 + ξ²) − ξ²Ê²))/(1 + ξ²) if Ê² ≤ sd²·Θ, and v ≤ Σ_j b_j otherwise. The lower end is the mirror
image.

*Proof.* The true variance is sd² − Σ_j v_j²/Θ_j, and Σ_j v_j²/Θ_j ≥ v²/Θ by Cauchy–Schwarz. If v > Ê, squaring gives a
quadratic in v whose larger root is the bound. If v ≤ Ê, the bound is at least Ê exactly when Ê² ≤ sd²·Θ; otherwise use
|v| ≤ Σ_j b_j ≤ √(sd²·Θ). ∎

[proved, refereed] `reports/bounds3/REPORT.txt` section 2, R8; `reports/bounds3/referee/REFEREE.txt` item 10 ("no errors
found"). Checked on 200,000 random cases (`checks.py`).

The theorem needs c*, not the item's own constant, because the true shares are unknown, and its event differs from the
plain interval's, so taking the smaller of the two costs a second event. At Ê = 0 it is the plain width divided by
√(1 + ξ²), and on the 50k BioCLIP-2 dump ξ² is 0.092 / 0.079 / 0.012 / 0.0008 at the four levels, because the finely
coded blocks dominate Θ [measured]. It is not built: it saves 0.6% of bytes for a square root and a division per item
per level.

## 3.6 Threshold sets

A threshold query cannot use Theorem 3.10, because an error at any item can change the answer. This section is the
second pass's analysis (`reports/surge/bounds2.txt` section 1). V is the viewer's visible set, S = {x ∈ V : q·x ≥ t},
and U₀ ⊆ V is the set of visible items that reach a randomised level, which must not depend on ω. At level ℓ an
undecided item is excluded if Ê + c_ex·sd < t, certified in if Ê − c_in·sd ≥ t, and otherwise passed on; after level L
it is read exactly. This is the **mixed rule**. Verifying every admitted item instead costs at least one exact read per
match, measured at 0.89–1.17 reads per match against 0.08–0.37 for the mixed rule (`thr768.py`), which rules it out for
large answers.

**Theorem 3.13 (T1 of the second pass; one harmful side).** A match is decided wrongly only through its upper bound, and
a non-match only through its lower bound. Hence

    Pr[R ≠ S] ≤ L·( |S ∩ U₀|·δ(c_ex) + |U₀ \ S|·δ(c_in) ).

*Proof.* Union bound over the (item, level) pairs of U₀, one event each; U₀ is fixed (Proposition 3.9). ∎

[proved, refereed] `reports/surge/bounds2.txt` T1; checked as built in `reports/bounds3/referee/REFEREE.txt` item 8 and
`reports/surge/referee.txt` section 4. See Editor's query 4.

Three consequences follow. The all-items constant is c(N₀L) taken one-sided, which is 8.61 / 8.72 / 8.83 at n = 512 /
768 / 1,536 with five levels instead of 8.68 / 8.79 / 8.90. Any pair (c_ex, c_in) with the displayed sum at most ε is
sound. And because N₀ is known before any randomised decision and is the same for every seed, Proposition 3.9 already
lets the constant be chosen per query from N₀. That lowers c by about ln(N/N₀)/c: 0.26 at N₀ = N/10 and 0.53 at N/100.
On the 50k BioCLIP-2 dump text queries have N₀ at 64–99% of N, so they gain nothing, and item queries at tight
thresholds would gain 3–6% (`reports/bounds3/REPORT.txt` section 6).

**Theorem 3.14 (T2 of the second pass; self-bounding exclusion).** Let c_x(·) be the constant with count × L × δ = ε_ex.
Let the algorithm, at each exclusion, use any constant at least c_x(𝒩), where 𝒩 is the number of items of U₀ not yet
excluded. Then Pr[some match is excluded] ≤ ε_ex, for any order of work and any number of re-tests.

*Proof.* Let c₀ = c_x(|S ∩ U₀|), a number that does not depend on the seed, and G the event that every match's upper
bound at constant c₀ holds at every level; Pr[not G] ≤ ε_ex. Take the first wrong exclusion in execution order. Before
it no match was excluded, so 𝒩 ≥ |S ∩ U₀| and the constant used was at least c₀. An upper bound that fails at a larger
constant fails at c₀, so G fails. The bad event therefore lies inside not-G. ∎

[proved, unrefereed] `reports/surge/bounds2.txt` T2. The surge referee proved the same induction independently for the
level-0 report's self-bounding sketch, and found it needs no ε/J split, only a width monotone in c
(`reports/surge/referee.txt` section 4).

The data-dependent constant needs no union over its possible values, because the only event used is the fixed event G;
the argument is the first-failure argument of Theorem 3.6. Its weakness is that 𝒩 counts everything not yet excluded,
which on weak first levels is far above |S|.

**Theorem 3.15 (T3 of the second pass; cap grid for exclusion).** Take caps M_j = M₀·2^j and constants c_j with
L·δ(c_j) = ε₀/M_j. Run the query at c_j, giving R_j, and accept if |R_j ∩ U₀| ≤ M_j, otherwise go to the next cap. Assume
no non-match is admitted, which holds with certainty when admitted items are verified and on Theorem 3.13's certify-in
event under the mixed rule. Then Pr[the accepted result differs from S] ≤ 2.1·ε₀ when caps are tried in increasing order
from M₀ = 64, and ≤ 3.1·ε₀ for any order or starting cap.

*Proof.* Fix j and let F_j be the number of matches whose upper bound fails at c_j; E F_j ≤ |S|·ε₀/M_j and
|R_j| ≥ |S| − F_j. If |S| ≤ M_j, Pr[R_j ≠ S] ≤ |S|·ε₀/M_j. If |S| > M_j, acceptance needs F_j ≥ |S| − M_j ≥ 1, and Markov
gives ε₀(|S|/M_j)/(|S| − M_j), which is at most (1 + 1/M_j)·ε₀ at the cap just below |S| and at most 2ε₀/M_j at lower
caps. In increasing order the search stops at the first cap ≥ |S| at the latest, so the sum over caps is
ε₀(1 + 1 + 1/M₀ + 4/M₀). In any order the caps above |S| add a geometric series, for a total of ε₀(2 + 1 + 5/M₀). ∎

[proved, unrefereed] `reports/surge/bounds2.txt` T3.

Markov is used because near-duplicate matches fail together: under one rotation the correlation of two items' errors
is 0.38 / 0.77 / 0.92 at residual cosine 0.9 / 0.99 / 0.999 (`pairdep.py`). The result is c_ex = c(about 4.2·|S ∩ U₀|·L)
with the whole of ε, or 8.4·|S ∩ U₀|·L with half, and no dependence on N. In a toy (`simcap.py`, n = 16, ε = 0.1, 40,000 seeds) the failure rate was at most 0.046 for
independent matches sitting at t and 0.003 for duplicates; without the factor 2.1 it sat at ε (0.098–0.102 at |S| = M₀
and M₀ + 1), so the factor cannot be dropped [measured].

**Theorem 3.16 (T4 of the second pass; band rule, both sides).** Fix nominal scales s̄_ℓ(x) that do not depend on the
seed and do not increase in ℓ, and a bracket β ≥ 1. An item whose realised scale exceeds β·s̄ uses the all-items
constant c_far at that level; otherwise its scale is raised to at least s̄. Let L·N₀·δ(c_far) = ε₀,
δ(c_acc) = ε₀/(2·𝒥·L(L + 1)) with 𝒥 the number of caps, and c_min = (β·c_far + c_acc)/(1 + β). At level ℓ use
c_ℓ = max(c_min, c(M)) with L·δ(c(M)) = ε₀/M for a cap M, on both sides, and accept the level if the number D_ℓ of
items still undecided after it is at most M. Then Pr[some wrong decision] ≤ 6ε₀.

*Proof.* For item x, level ℓ and constant c, let p_x = |q·x − t|/s̄_ℓ(x), which is seed-free.
(i) Far or badly coded items: if p_x > β(c_far − c), a wrong decision at c needs a harmful error beyond
c + p_x/β > c_far. These events do not depend on c, and their union over items and levels has probability at most ε₀.
(ii) Near items, P = {p_x ≤ β(c_far − c)}, form a seed-free set of size n_P that does not increase in c. Each is wrong
with probability at most δ(c). A near item is decided at or before level ℓ only if some level's error exceeds
c_min − β(c_far − c) ≥ c_acc in absolute value, since scales at earlier levels are at least s̄_ℓ. So the number Y of near
items not left undecided has E Y ≤ 2ℓ·δ(c_acc)·n_P.
(iii) If n_P ≤ 2M the near items contribute at most (n_P/M)·ε₀/L; summed over the caps where this holds (an upper set of
a doubling grid, with n_P non-increasing) that is at most 4ε₀/L per level. If n_P > 2M, acceptance (D_ℓ ≤ M) forces
Y ≥ n_P/2, with probability at most 4ℓ·δ(c_acc) by Markov; summed over caps and levels this is at most ε₀.
Every event is defined per (item, level, cap) and does not depend on which items reached the level, so the rule for
choosing caps is free. The total is 6ε₀. Only per-item laws are used, each conditional on that item's own rotated
vector, never a joint law. ∎

[proved, unrefereed] for the plain width; [sketch: gap named] for its use. `reports/surge/bounds2.txt` T4, which names it
as the result to referee. See Editor's query 5.

The result is c_ℓ = max(c_min, c(12·L·D_ℓ)), with a floor of 8.12 / 8.21 / 8.30 at n = 512 / 768 / 1,536, five levels and
β = 1.4. Two gaps are named. β needs a seed-free bracket on tan φ per level; at n = 768 the measured relative spread of
tan φ is 2.6% / 3.1% / 6.5% / 11% at 1 / 2 / 4 / 6 bits, so β = 1.4 is safe for 1–4 bits and needs care at 6. And the
proof is written for the plain width; Theorem 3.11's width needs the same steps in its own units. The floor is an
artefact of using Markov with no joint law. The third pass notes that T3 and T4 carry over to block rotations, because
their proofs use only each item's own law, which is still symmetric and log-concave, and that their constants are the
second pass's to within 0.01, because the four-block worst case equals the single-sphere tail to 0.001
(`reports/bounds3/REPORT.txt` section 6).

**Proposition 3.17 (T5 of the second pass).** For a fixed constant c and one level,
E[number of wrong decisions] ≤ K·δ(c)·E[number left undecided], with K = 2/(1 − 2δ(c)).

*Proof.* The sphere coordinate has a symmetric log-concave density, so δ(a + b) ≤ 2δ(a)δ(b). An item at distance p sd
from t is wrong with probability at most δ(c + p). If p ≤ c it stays undecided with probability at least ½ − δ(c). If
p > c it stays undecided with probability δ(p − c) − δ(p + c), and δ(c + p) ≤ 2δ(c)δ(p − c). ∎

[proved, unrefereed] `reports/surge/bounds2.txt` T5.

The natural rule built on it, δ(c) = ε/(K·L·D_ℓ), a union over the undecided with no floor, is a [conjecture]: sound up
to a small factor. The missing step is a lower-tail bound for D_ℓ under one shared rotation that beats Markov; a
second-moment bound would do if the near-threshold items contain no large group of near-duplicates. In a toy
(`simband.py`, n = 64, N = 2,000, ε = 0.1, 4,000 seeds) the rule ran at c = 3.02 against an all-items 3.71 and failed at
0.087 with 100 independent items at the threshold, 0.040 with half of them duplicates and 0.090 with every item at the
threshold, never above ε [measured]. With near-independent items a level at constant c fails with probability about
(undecided after the level)/c² × δ(c), so at the last level with 10⁷ exact reads and a fifth of ε, at n = 768, no
analysis of this decision rule can go below c ≈ 7.7–7.8 [model]. T4 proves 8.5 there and the all-items constant is
8.72, so the whole prize is about 10% of exact reads.

Three further statements of the second pass are [proved, unrefereed].

- *Tolerance d.* Certify in when the lower bound reaches t − d and exclude when the upper bound is below t. The unions
  are unchanged, because they run over matches above t and non-matches below t − d. An item is undecided at level ℓ
  only if its estimate lies in a window of width (c_in + c_ex)·sd_ℓ − d, so a level with (c_in + c_ex)·sd_ℓ ≤ d leaves
  nothing undecided, and d is worth d/(2·sd_ℓ) units of c at level ℓ. The answer is then no longer unique, so
  Theorem 3.6 and a filter built from another vector result lose their proofs.
- *Expected errors.* "Expected number of wrongly decided items ≤ ε" has the same constant as "probability of any error
  ≤ ε", because the union bound is the expectation. A per-item guarantee at ε_item = 10⁻⁹ has constant c(L) =
  6.14 / 6.18 / 6.22 at n = 512 / 768 / 1,536, independent of N, and by Proposition 3.17 the expected number of wrong
  items in a query is at most 2ε_item × (undecided after the first level). For a 10⁸ answer the probability that some bit
  is wrong is then bounded only by 0.4.
- *Filters.* Every count above runs over visible items, so the all-items constant becomes c(N_V·L), 7.92 at n = 768 for
  10⁶ visible items. With the all-items constant each decision depends on one item, so a result masked per viewer
  equals the result computed over the visible set, for every seed. With count-based constants (T1(c), T3, T4) the
  constant depends on other items, so on a failing seed a visible bit could depend on invisible data, and a cached
  result masked per viewer should be made only with the all-items constant.

**Constants for thresholds** [computed, `reports/surge/bounds2/consts3.py`; N = 10⁹, ε = 10⁻⁹, five levels, one
rotation over all n dimensions].

| Union | sub-Gaussian | n = 512 | n = 768 | n = 1,536 |
|---|---|---|---|---|
| N × 5, two-sided (before) | 9.35 | 8.68 | 8.79 | 8.90 |
| T1: all N, harmful side only | 9.28 | 8.61 | 8.72 | 8.83 |
| T3 exclude side, \|S\| = 10³ / 10⁵ / 10⁷ (ε/2) | 7.92 / 8.48 / 9.01 | 7.34 / 7.87 / 8.36 | 7.41 / 7.95 / 8.46 | 7.47 / 8.03 / 8.56 |
| T1(c): exposed set N₀ = 10⁶ / 10⁷ / 10⁸ | 8.50 / 8.77 / 9.03 | 7.89 / 8.14 / 8.38 | 7.97 / 8.23 / 8.48 | 8.06 / 8.32 / 8.58 |
| T4: undecided = 10⁶ / 10⁷ / 10⁸ | 8.79 / 9.05 / 9.30 | 8.16 / 8.40 / 8.63 | 8.25 / 8.50 / 8.74 | 8.34 / 8.60 / 8.85 |
| T4 floor (β = 1.4) | 8.75 | 8.12 | 8.21 | 8.30 |
| single item × L (not attainable) | 6.68 | 6.14 | 6.18 | 6.22 |

**Measured effect on thresholds** [measured, `thr768.py`: BioCLIP-2 200k, flat global-rotation codes of 1, 2, 4 and
6 bits (96 / 192 / 384 / 576 B cumulative), 40 corpus items as queries, constants as if N = 10⁹ at the same answer
share, Theorem 3.11's interval; no match lost and no non-match admitted in any row]. The band row uses Theorem 3.11's
interval and no bracket, so it shows the size of T4's effect, not a run of the proved rule. Exact reads per true match
after the 6-bit level:

| Answer share | 0.01% | 0.1% | 1% | 10% | 30% |
|---|---|---|---|---|---|
| 8.79 both sides (before) | 0.376 | 0.229 | 0.159 | 0.104 | 0.080 |
| T1, 8.69 | 0.374 | 0.227 | 0.157 | 0.103 | 0.079 |
| T4 band rule | 0.359 | 0.215 | 0.150 | 0.100 | 0.078 |
| single-item constant 6.15 (not sound) | 0.281 | 0.160 | 0.112 | 0.073 | 0.056 |
| T1 with d = 0.002 / 0.005 / 0.01 | 0.099 / 0.004 / 0 | 0.076 / 0.008 / 0 | 0.065 / 0.010 / 0 | 0.052 / 0.012 / 0 | 0.044 / 0.010 / 0 |

Exact reads are (score density at t) × (c_in + c_ex) × (last-level sd) within a factor of two on this sample, so they are
linear in the two constants, and against answer size they follow the hazard rate: about |S|^0.8 [measured and argued].
The depth of the last code level (4 to 6 bits per dimension: 3 to 3.4 times fewer reads) and the tolerance d move exact
reads far more than any sound constant.

The surge referee adds two points about threshold answers that certify items in from codes
(`reports/surge/referee.txt` section 4). The answer is no longer made of exactly scored items, so the deterministic
statement weakens to S(t + G) ⊆ R ⊆ S(t − G), with G the largest gap between the deterministic and randomised ends
among the decisions made, about 1.6 to 2.7 times the randomised width at n ≥ 512. And the referee found no proof of a
certify-in constant below the all-items one: an observed undecided count depends on ω and the undecided set is a subset
of the near-threshold set, the wrong direction for a union bound. Theorem 3.16, written a day later, is the first such
rule and is unrefereed; the referee's own route is under Open.

## 3.7 The constants for block rotations

With several blocks the error at a level is Σ_j a_j U_j with unknown scales, and the constant must hold for every
scale vector that occurs. This section is the third pass (`reports/bounds3/REPORT.txt`), refereed in
`reports/bounds3/referee/REFEREE.txt`, which found nothing unsound and reproduced the four main constants. The setting
throughout is ε = 10⁻⁹, L = 4 and the BioCLIP-2 layout at n = 768 (d_j = 95, 127, 255, 287).

The code it started from (commit c25333fa9, `rust/REPORT_BOUNDS.md` Part 2) took the exact tail where one block is coded,
c = s*·√d with Pr[U ≥ s*] = exp(−c_old²/2), which gives 7.413 at n_j = 96. Where several blocks are coded it took
Chernoff's bound with each coordinate's exact log moment generating function Λ_d: Pr[X ≥ h] ≤
exp(−λh + Σ_j Λ_{d_j}(λa_j)). The constant must bound max_w min_λ, which is at most min_λ max_w; for a fixed λ the sum is
separable, so each share is rounded up to a multiple of 1/1,000 (Λ is increasing) and the maximum found by dynamic
programming, with Λ tabulated by Simpson's rule and read by linear interpolation, an upper bound because Λ is convex. The
result was 9.086 for four blocks, against 9.073 for max_w min_λ on a grid [computed]. This is Proposition 3.3's Chernoff
bound with the exact function, so it needed no new argument; the implementation was not refereed. Before that commit the
sub-Gaussian 9.33 was used everywhere (9.46 with the earlier fourteen levels, `rust/REPORT_RAND.md`).

**Theorem 3.18 (R1; scales may be rounded up).** Let U_1, …, U_J be independent, each with a density symmetric about 0
and non-increasing on [0, ∞). For x > 0, T(a; x) is non-decreasing in every a_j ≥ 0.

*Proof.* Fix j and let Z = Σ_{i≠j} a_i U_i. If Z = 0 then T = Pr[U_j ≥ x/a_j]. Otherwise Z has a density f_Z that is
symmetric and unimodal, since a convolution of symmetric unimodal densities is. By the symmetry of U_j,
T = E[(G(x − a_j|U_j|) + G(x + a_j|U_j|))/2] with G(y) = Pr[Z ≥ y]. For fixed u ≥ 0 the derivative in a_j of
G(x − a_j u) + G(x + a_j u) is u(f_Z(x − a_j u) − f_Z(x + a_j u)) ≥ 0, because |x − a_j u| ≤ x + a_j u. ∎

[proved, refereed] R1; referee item 5 (Birnbaum's peakedness theorem; 360 corner-against-interior cases). It applies
because the coordinate's density is proportional to (1 − s²)^((d−3)/2), unimodal for d ≥ 3, and every block has d ≥ 15.

**Theorem 3.19 (R2; the exact half-width per item).** For an item, a level and its coded blocks, let ā_j ≥
A_j‖P_{y⊥}Δ_j‖ and h ≥ 0 be functions of the item's rotated vectors, its stored scalars and the query, with
T(ā; h) ≤ δ. Then Pr[error ≥ h] ≤ δ and Pr[error ≤ −h] ≤ δ. In particular h = H_δ(ā) is valid, and no function of ā
alone can be smaller.

*Proof.* Given the rotated vectors the error is Σ_j a_j U_j with a_j = A_j‖P_{y⊥}Δ_j‖ ≤ ā_j, so
Pr[error ≥ h | rotated vectors] = T(a; h) ≤ T(ā; h) ≤ δ by Theorem 3.18. Integrate. ∎

[proved, refereed] R2, with the referee's wording fix (item 3): when stored scalars are calibrated on other rotated
items the conditional step does not go through literally, and the argument that holds is pointwise, as stated after
Lemma 3.1. c*·sd, R5 and R8 with barred quantities all satisfy it. Two consequences follow: scalars rounded up on any grid
are covered, and the minimum of several upper bounds of H_δ is still one event per (item, level), since each is at least
H_δ.

**Theorem 3.20 (R3; the worst case over block shares).** h = c*·sd is valid for every item, with c* = sup_w c(w).

*Proof.* H_δ(a) ≤ c*·sd for every a, so Theorem 3.19 applies. ∎

[proved, refereed] for the reduction; the values are [computed] by a certificate the referee checked (item 6).

**The certificate method** (`cert2.py`, about 60 lines; the method is in `rust/src/certify.rs` and
`rust/tools/certify.py`). It shows T(a(w); X) ≤ δ for every share vector w by three steps, each an inequality in the safe
direction.

1. By Theorem 3.18 shares may be rounded up. At grid step 1/K every share vector lies in a cell whose corner g has
   Σ_j g_j = 1 + (J − 1)/K, so sup_w T(a(w); X) is at most the maximum over corners. Cells whose corner fails are split
   in eight and re-tested at K → 2K, from 64 up to 4,096, and the certificate holds when no cell fails.
2. a_j U_j ≤ Y_j = η⌈a_j U_j/η⌉, so the tail is at most that of the lattice sum, with η = 0.001 sd.
3. The lattice tail is an exact finite sum, evaluated through the discrete Fourier transform of the tilted
   probabilities. The frequencies left out are bounded by the block with the smallest remainder, since every other
   factor has modulus at most 1. Margins are 10⁻⁶ relative and 10⁻¹¹ absolute, and the probability mass function uses
   eight-point Gauss–Legendre per cell.

The slack of the certificate is 0.008–0.009 (grid 1.5c/K, lattice 3η). The referee found the cell cover, refinement,
lattice rounding direction and Fourier truncation correct, and floating-point error about 10⁻¹² relative against the
margins; in 360 cases the certificate lay above the full-frequency lattice, which lay above the exact tail at the corner,
which lay above the exact tail at w. At the certified X the true ln tail is 0.07–0.09 below ln δ. Independent tails by FFT convolution, by inversion with arbitrary precision (agreeing
to 10⁻⁹) and by importance sampling (within 0.002 in ln) gave exact worst cases of 8.7659 / 8.6951 / 8.0271 / 7.9485,
each 0.008–0.009 below the certified 8.774 / 8.704 / 8.036 / 7.957. X = 8.695 fails as it must. The supremum over shares
has one local maximum, at (0.1254, 0.167, 0.333, 0.3746), close to w_j = (d_j + 2)/Σ_i(d_i + 2), the shares that bring the
fourth cumulant closest to zero; that location is a predictor, not a theorem, and the referee's check of the supremum
is a scan, not a second certificate. The certified numbers are floating-point computations, not interval-verified. A certificate covers every subset of its blocks (shares on a face of the simplex)
and every budget at least as loose as its own, and certificates are never interpolated in ln δ, because the quantile is
concave in ln δ and a chord underestimates it.

Rotating four blocks separately costs almost nothing against one rotation of all dimensions: the exact worst case is
8.7659 against 8.7653 for one coordinate of a sphere in 767 dimensions [computed]. c* lies below the exact tail of one
coordinate in d′ = Σ_j(d_j + 2) − 2 dimensions by 0.00002 to 0.0006 in all twelve configurations computed.

**Theorem 3.21 (R4; a closed form by convex order against one sphere).** Let d_Σ = Σ_j d_j,
ν_j = E√Beta(d_j/2, (d_Σ − d_j)/2) = Γ(d_j/2 + ½)Γ(d_Σ/2)/(Γ(d_j/2)Γ(d_Σ/2 + ½)), and s² = Σ_j a_j²/ν_j². Then for every x,

    T(a; x) ≤ inf over t′ < x of E(s·U_D − t′)₊/(x − t′),

with U_D one coordinate of a uniform unit vector in ℝ^(d_Σ) and E(U_D − t′)₊ = C(1 − t′²)^((d_Σ−1)/2)/(d_Σ − 1) −
t′·Pr[U_D ≥ t′], C = Γ(d_Σ/2)/(√π·Γ((d_Σ − 1)/2)). Since s ≤ θ·√(d_Σ)·sd with θ² = max_j d_j/(d_Σ ν_j²) (1.0023 at
n = 768), this gives one constant.

*Proof.* Let g be standard Gaussian in ℝ^(d_Σ) with blocks g_j. The ω_j = g_j/‖g_j‖ are independent, uniform on their
spheres, and independent of the norms ‖g_j‖. u = g/‖g‖ is uniform on the unit sphere of ℝ^(d_Σ), and ϱ_j = ‖g_j‖/‖g‖ is a
function of the norms, so ϱ is independent of the ω_j, and ϱ_j² is Beta(d_j/2, (d_Σ − d_j)/2). For b with block norms
β_j, ⟨b, u⟩ = Σ_j β_j ϱ_j U_j with U_j coordinates of the ω_j, and ⟨b, u⟩ has the law of ‖b‖·U_D. For convex f, Jensen
given the U_j gives E f(Σ_j β_j ϱ_j U_j) ≥ E f(Σ_j β_j ν_j U_j). With β_j = a_j/ν_j, E f(Σ_j a_j U_j) ≤ E f(s·U_D). Apply
it to f(y) = (y − t′)₊ and Markov's inequality 1{X ≥ x} ≤ (X − t′)₊/(x − t′). ∎

[proved, refereed] R4; referee item 7 ("sound, every step"; θ = 1.002307 recomputed; `tail.rs`'s `convex_order`
implements the same inequality and bisects on the safe side). In this proof ω_j and ϱ_j are local to the proof and are
not the seed.

R4 gives 8.885 two-sided and 8.815 one-sided at n = 768, N = 10⁹, keeping 63% of the gap between 9.086 and 8.766 with
one-dimensional numerics only, and it is the fallback for layouts without a certificate. The plain statement "four blocks
are no worse than one sphere in d_Σ dimensions" is [refuted] by 0.0015.

**Theorem 3.22 (R5; one constant per block).** If ĉ ∈ ℝ₊^J satisfies H_δ(a)² ≤ Σ_j ĉ_j² a_j²/d_j for every a, then
h = √(Σ_j ĉ_j² ā_j²/d_j) is a valid half-width, and so is the smaller of it and c*·sd.

*Proof.* The hypothesis makes h ≥ H_δ(ā) pointwise, so Theorem 3.19 applies; the minimum of two such bounds is one
event (Theorem 3.19, second consequence). ∎

[proved, refereed] for the statement (referee item 9); the proof line is the report's "valid half-width (R2)" written
out. F(v) = H_δ(√(v_1 d_1), …)² is homogeneous of degree one, so its tangent plane at shares w⁰ is ∇F(w⁰)·v, and
ĉ_j² = ∂_jF(w⁰) touches F at w⁰. F is concave as far as measured (400 random segments of the simplex, all second
differences negative; 27,000 random points below three tangent planes), which would make every tangent plane valid, but
that is a [conjecture]. Three planes are certified by the grid method on the ellipsoid (`plane_cert.py`; the referee
confirmed them on a 1/24 grid with margin 0.010–0.011 but did not referee the script line by line). They are for
n = 768, N = 10⁹, one-sided, anchored at the 50k dump's mean shares; the anchor decides only how tight a plane is,
never whether it is valid.

| Level | anchor shares | ĉ_j, blocks 0–3 | at the anchor |
|---|---|---|---|
| 96 B | 0.27 0.41 0.31 0.01 | 8.426 8.323 8.860 9.316 | 8.533 sd |
| 160 B | 0.27 0.25 0.45 0.04 | 8.379 8.610 8.660 9.217 | 8.592 sd |
| 256 B | 0.10 0.16 0.63 0.11 | 8.861 8.789 8.449 9.079 | 8.617 sd |

A pointwise rule from the fourth cumulant alone, c(w) ≤ the sphere tail in 1/Σ_j(w_j²/(d_j + 2)) − 2 dimensions, is
[refuted] by up to 0.13.

**Theorem 3.23 (R6; the harmful side only).**

(i) *Thresholds.* A match is lost only if one of its upper bounds fails and a non-match admitted only if one of its
lower bounds fails, and each decision uses that level's interval alone. So δ = ε/(NL) per event, and every constant is
taken at twice the tail: 7.413 → 7.373 and 8.774 → 8.704.

(ii) *k-NN as coded in the loop `knn2d`* is not sound under one-sided validity. The loop intersects an item's intervals
across levels and counts the candidate itself: t̂ is the k-th largest lower bound over all candidates, and a candidate is
certified when at most k upper bounds reach its lower bound. If the harmless side of an item fails, its intersected
interval can invert (lower > upper), and then a true neighbour can exclude itself and a non-neighbour certify itself.
With that loop the one-sided constants give k-NN a bound of 2ε.

(iii) *k-NN with two counts changed.* Drop a candidate when at least k other candidates have a lower bound above its
upper bound; certify it when at most k − 1 other candidates have an upper bound at or above its lower bound; store an
exact score without clamping it to earlier bounds. Assume every upper bound of the true top k holds and every lower bound
of every other item holds (N·L one-sided events). Then the result is exact.

*Proof of (iii).* A top-k item x is never dropped: a candidate z with lower bound above upper(x) ≥ s(x) cannot be outside
the top k, because then its lower bound would be at most s(z) ≤ s(x); and there are at most k − 1 others inside. An item y
outside is never certified: the k top-k items are all candidates and each has upper ≥ its score ≥ s(y) ≥ lower(y), and an
item tied with the k-th score is covered because the count uses ≥. The sweep at the seed threshold τ₁ drops only on
upper < τ₁ ≤ s_k. So the survivors scored exactly contain the top k, and certified items are in it. ∎

[proved, refereed] (i) and (iii); (ii) [refuted]. `reports/bounds3/REPORT.txt` R6; referee items 1 and 8.

The third pass first "proved" (ii) sound, and only a simulation caught the error: `knn_onesided.py`, transcribing the
loop, gave 0 wrong in 4,000 with intervals valid on both sides and 554 wrong in 4,000 with intervals valid on the harmful
side only; the changed counts gave 0 in 4,000. The referee's adversarial run (`knn_adv.py`, 2 seeds × 20,000 trials,
integer arithmetic so that ties are exact, a harmless side wrong by up to 30 half-widths, a harmful side tight 15% of the
time) gave 2,214 and 2,201 wrong sets for the old loop and 0 for the changed rule. Clamping exact scores to earlier
bounds left the sets right but carried a wrong score in 604 and 578 of them, so "no clamping" is needed for the returned
scores, not for the set. The referee's conditions are these:

- Distinct entities are required, deterministically. With k = 2, two rows of one entity at 0.9 and the true second-best
  x at 0.8, both rows have lower > upper(x), so x is dropped.
- The bound must bound the exact-read score bit for bit; a float version lost true neighbours whenever upper = score was
  off by one rounding.
- The candidates must contain the top k, so τ₁ must come from exact scores of distinct visible entities.

**Proposition 3.24 (R7; the union over levels).** For one item let F_ℓ = {error_ℓ > h_ℓ}. Then
Pr[F_ℓ ∩ F_ℓ′] ≤ Pr[Z ≥ 2] with Z = error_ℓ/h_ℓ + error_ℓ′/h_ℓ′, which is again a sum of independent scaled sphere
coordinates, one per block, of variance 1/c_ℓ² + 1/c_ℓ′² + 2ρ/(c_ℓ c_ℓ′).

*Proof.* On both events Z ≥ 2, and error_ℓ and error_ℓ′ are linear in the same uniform vectors. ∎

[proved, refereed] R7; referee item 10. For one block coded at both levels, Z ≥ 2 has exactly the probability of
U ≥ s*·√(2/(1 + ρ)).

One item's errors at successive levels correlate 0.454 (96 B, 160 B), 0.470 (160 B, 256 B) and 0.224 (96 B, 256 B), with
sd ratios 0.43, 0.40 and 0.17 [measured, `level_corr.py`, 0.9–1.9 million pairs]. At ρ = 0.47 this gives
Pr[both fail]/δ ≤ e^−31 for one block and e^−15.6 for four blocks at the worst shares; e^−10.7 at ρ = 0.6 and e^−4.7 at
0.8 [computed]. The first level (block 0 alone) against the second (four blocks, block 0 unchanged) overlaps by 7·10⁻¹⁰
to 7·10⁻⁷ of δ for block 0 holding 22–55% of the second level's variance, by direct integration (`overlap01.py`). So Pr[some level fails] ≥ (L − 10⁻⁶)·δ when each level runs at δ: the factor L is real, and
no maximal inequality removes it for intervals of this form. Pairs overlap by more than 10% only above ρ = 0.9, which
needs more than about twenty levels over this range of sds. In a Brownian picture even continuous monitoring counts as
about c²·ln(sd_first/sd_last), some 140 tests [sketch: gap named]. Conditioning on an earlier level does not help either,
because the items at level ℓ + 1's boundary are those whose level-ℓ estimate was ρc sds from t [refuted as a saving].

Any fixed weights π_ℓ with Σπ_ℓ = 1 are sound for splitting ε over levels. The four-block constants by weight are 8.551
(all of ε), 8.589 (0.7), 8.695 (0.25), 8.789 (0.1), 8.899 (1/30), and the one-block constants 7.291, 7.312, 7.373,
7.426, 7.487 [computed]. The best weights are proportional to (items undecided after the level) × (cost of what they
read next). The constant for L = 3 / 4 / 5 is 8.666 / 8.695 / 8.718.

**R9 (what no per-item rule can beat)** [sketch: gap named; unchecked by the referee]. *Claim:* if every decision at a
randomised level is a function of that item alone, and the answer must be exact with probability 1 − ε on every corpus
of N items, then Σ_ℓ T(a_ℓ; h_ℓ) ≤ (ε/N)(1 + o(1)) for every scale vector that occurs. The item's own exact quantile with
ε split over levels cannot be undercut, and N cannot be replaced by anything smaller. *Sketch:* take N items at the
threshold, half just above and half just below. An item at margin zero is wrongly decided at level ℓ with probability
T(a_ℓ; h_ℓ) on its harmful side, the levels' events are disjoint up to e^−15 (Proposition 3.24), and if the items'
events are nearly independent the probability of some error is the sum. *Gap:* N items whose failure events overlap
negligibly under the index's shared rotations; within a group the sketch relies on the measured pair dependence (0.00 /
0.02 / 0.09 at residual cosine 0 / 0.3 / 0.6) rather than a construction. If R9 holds, a union only over the levels an
item reaches cannot lower the default constant, because an item at the threshold reaches every level.

**R10 (refuted on the way).** Adding block-wise bounds with ε split over blocks gives 15.9 sd at the worst shares against
8.70, never below H_δ. Bernstein-type bounds from |U| ≤ 1 are dominated by Chernoff with the exact moment generating
function, which is dominated by the exact tail. Jensen against the large sphere or a Gaussian with constant 1 does not
apply, because the tail is convex along the total variance and concave across shares, which is why R4 carries a factor.

**Constants** [computed, `reports/bounds3/consts.py`; certified rows by `cert2.py`; ε = 10⁻⁹, L = 4; half-width in sd].

| | n = 512, N = 10⁶ / 10⁹ | n = 768, N = 10⁶ / 10⁹ | n = 1,536, N = 10⁶ / 10⁹ |
|---|---|---|---|
| block dimensions d_j | 63 79 175 191 | 95 127 255 287 | 191 255 511 575 |
| sub-Gaussian, two-sided | 8.558 / 9.330 | 8.558 / 9.330 | 8.558 / 9.330 |
| one block, exact tail, two-sided | 6.469 / 6.786 | 6.976 / 7.413 | 7.549 / 8.142 |
| four blocks, Chernoff, two-sided | 8.275 / 8.962 | 8.372 / 9.086 | 8.470 / 9.214 |
| R4, two-sided | 8.078 / 8.781 | 8.156 / 8.885 | 8.234 / 8.990 |
| R3 exact worst case, two-sided | 7.945 / 8.658 | 8.027 / 8.766 | 8.110 / 8.876 |
| certified | 7.954 / 8.666 | 8.036 / 8.774 | 8.119 / 8.885 |
| one block, one-sided | 6.432 / 6.758 | 6.927 / 7.373 | 7.485 / 8.087 |
| Chernoff, one-sided | 8.201 / 8.897 | 8.295 / 9.018 | 8.391 / 9.142 |
| R4, one-sided | 8.003 / 8.714 | 8.078 / 8.815 | 8.154 / 8.918 |
| R3 exact worst case, one-sided | 7.869 / 8.590 | 7.949 / 8.695 | 8.029 / 8.803 |
| certified, one-sided | 7.878 / 8.598 | 7.957 / 8.704 | 8.038 / 8.811 |
| R3 exact, one-sided, two / three blocks | 7.29/7.73 / 7.84/8.40 | 7.57/7.85 / 8.20/8.57 | 7.83/7.98 / 8.54/8.74 |

At n = 768, N = 10⁹, one-sided, two and three blocks (95, 127) and (95, 127, 255) give 8.198 and 8.568 exact, 8.207 and
8.577 certified. Each item's own exact constant averages 8.48 / 8.52 / 8.56 by level. For reference, the Gaussian
quantile is 8.912 and the single-item constant 6.15, which is not attainable.

## 3.8 Measured effect of the constants

**On the 50k dump** [measured, `reports/bounds3/final_sim.py`: the regenerated 50k BioCLIP-2 dump, 10 item and 10 text
queries × 7 answer sizes, 9.7 million records; a record exists while the 9.33 bound keeps an item, and every row is
at most as wide, so every row is replayed exactly]. Change against the committed constants (7.413 / 9.086); bytes are
from the first randomised level on, full vector 1,536 B.

| Row | Constants | item: exact reads / bytes | text: exact reads / bytes |
|---|---|---|---|
| A | 9.330 everywhere | +2.6% / +2.4% | +2.6% / +2.2% |
| S | harmful side only, committed method (7.373 / 9.018) | −0.8% / −0.6% | −0.7% / −0.6% |
| C | exact worst case, two-sided (7.413 / 8.774) | −3.4% / −2.4% | −3.4% / −2.5% |
| C1 | R4, one-sided (7.373 / 8.815) | −3.0% / −2.1% | −2.9% / −2.2% |
| D | exact worst case, one-sided (7.373 / 8.704) | −4.1% / −3.0% | −4.1% / −3.1% |
| P | D + per-block constants (R5) | −5.3% / −3.9% | −5.0% / −3.9% |
| E | D + each item's own exact constant | −5.9% / −4.4% | −5.5% / −4.2% |
| L | D + ε split 0.1 / 0.1 / 0.1 / 0.7 | −5.3% / −3.4% | −5.3% / −3.5% |
| EL | E + that split (best sound combination) | −7.1% / −4.8% | −6.6% / −4.7% |
| T | D + Theorem 3.12's interval | −4.2% / −3.6% | −4.2% / −3.6% |
| X | single-item constant 5.67 / 6.15 (not sound) | −31.8% / −23.5% | −31.9% / −24.4% |
| N | D at N = 10⁶ (6.927 / 7.957) | −12.3% / −8.9% | −12.3% / −9.3% |
| R | D with the stored scale's rounding removed (not available to the code) | −6.4% / −4.7% | −6.4% / −4.8% |

No row made a wrong decision. The per-item constants of rows E and EL come from an interpolated table, with an error of
up to 0.01 near the faces of the simplex. Exact reads move 10.8% per unit of c in rows A, C and D alike, linearly, so the constant
is a weak lever and is close to used up. Row D gives −2.5% to −6.0% of exact reads by answer size.

**The stored scale.** The stored scale was 2.45% too large in rms (2.3–2.6% by level): the mean of (truth − estimate)²/sd²
is 0.953 ± 0.001 over 0.9 million items [measured, `scale_check.py`]. The referee reproduced it (ratios 1.0245 / 1.0264 /
1.0228) and located the excess in blocks 0 and 1, about 7% and 3% high, with no measurable margin in blocks 2 and 3. Too
large is safe by Theorem 3.18, but costs 0.21 in c and 2.4% of exact reads (row R). The source is the 15-value tan φ table:
at the encoder, tan² was 6.5–8.8% high in block 0, 2.8–6.2% in block 1 and 1.9–5.2% in blocks 2 and 3, against 0.4–0.5%
for ‖y‖². Refitting the 15 values as the round-up quantiser with the least mean square ratio, by dynamic programming
over 1,024 candidates, cut these to 4.2–5.0%, 2.0–4.1% and 1.4–3.6% at the same bits, and the build now asserts that
every decoded ‖y‖ and tan φ is at least the true value (`rust/REPORT_CONST.md` step 4). The rest of the 2.45% comes from
‖y‖ (0.2% of sd) and from bounding ‖P_{y⊥}Δ_j‖ by ‖Δ_j‖ (about 1/(2n_j), 0.5% for block 0), which no stored code can fix.

**In the native index** [measured, `rust/results/const/check.sh`, `run.sh`, `cmp.py`; 50k and 1M BioCLIP-2 dual indexes,
`rust/REPORT_CONST.md`]. Items entering each level against the start (50k verify counts):

| Step | 160 B | 256 B | full vector |
|---|---|---|---|
| 1 certified constant | −2.9% | −3.2% | −3.3% |
| 2 one-sided | −3.5% | −4.0% | −4.1% |
| 3 the index's own N (2¹⁶) | −13.6% | −15.2% | −15.6% |
| 4 refitted tan table | −14.1% | −15.9% | −16.2% |

Steps 1 and 2 match rows C and D above. At 1M, start to final, bytes fell 6.5–8.5% and full-vector reads 12–13% at
answers of 10–75%, and bytes fell 0–4% at 0.01–1%. Time moved −1% to −3% for item queries and 0 to +1% for text, within
the ±5% noise of runs on a shared machine, so the time gain is not yet measured. Every step gave no wrong item, no
violation in 42.2–44.1 million interval checks and exact k-NN on 400 of 400 queries, and the largest randomised error
stayed at 5.47 sd, 2.17 sd inside the final 50k constant of 7.64.

The earlier step from 9.33 to the exact-law constants (7.413 / 9.086) cut items entering 96 / 160 / 256 B by 2.7 /
2.2 / 2.5% and full vectors by 2.6% at 50k, and saved 1–2% of time at 1M (`rust/REPORT_BOUNDS.md` Part 2). On the same
dump, against c = 9.33 and for item queries, the one-sided union alone (c = 9.256) saved under 1% at every level, and the
several-block self-normalised interval of Theorem 3.12, at the one-sided constant, saved 1.5 / 4.4 / 1.5% of entrants
and 0.9% of full reads.

## 3.9 What the index uses

The owner chose, from the third pass's options, the harmful side only for both threshold and k-NN queries, with the k-NN
loop changed as in Theorem 3.23 (iii), and N taken from the index's own size (`rust/REPORT_CONST.md`, opening; option a
of `reports/bounds3/REPORT.txt` section 5, item 2, and section 7). As built (SYSTEM.md sections 1–3):

- δ = ε/(N·L) on one side of each (item, level) event, with ε = 10⁻⁹, L = 4 and N the row count rounded up to a power of
  two, over all rows and never a viewer's count. At 1M, N = 2²⁰ and ln(1/δ) = ln(2²⁰·4/10⁻⁹) = 35.97.
- For one coded block the constant is the exact quantile of one sphere coordinate; for several, the smallest of the
  certificate in `meta.json`, R4 and Chernoff with the exact moment generating function. The fit takes the certificate
  from a shipped table or certifies the layout within `--cert-time` (600 s by default).
- The k-NN selection drops and certifies by counts that exclude the candidate itself, stores exact scores unclamped, and
  the build refuses two rows of one entity. Its seed threshold τ₁ is the k-th best of 2k + 8 exact scores.
- The bit rates are designed with a fixed constant (`CSG`, two-sided, N = 10⁹), so the stored code does not depend on N.

At 1M the build log lists three sets of coded blocks (SYSTEM.md section 3):

| Level | Blocks coded (d_j) | c_ℓ at 1M (N = 2²⁰) | Source |
|---|---|---|---|
| 56 B | block 0 (95) | 6.931 | exact tail of one sphere coordinate |
| 96 B and 160 B | blocks 0–2 (95, 127, 255) | 7.963 | the four-block certificate, which covers its subsets |
| 256 B | all four (95, 127, 255, 287) | 7.963 | certificate |

The sub-Gaussian constant at the same budget would be 8.482. On the 50k index (N = 2¹⁶) the constants are 6.724 for
block 0 and 7.639, the four-block certificate, where all four blocks are coded; the three-block set occurs at 1M only
(`rust/REPORT_CONST.md`, "Constants used per level"). A certificate for the three blocks alone would be lower (8.568
exact, one-sided, at N = 10⁹), but it is not built, because the design certifies one layout per index. The rule for N is sound under the referee's condition
(`reports/bounds3/referee/REFEREE.txt` item 2) [proved, refereed]: a result is exact with probability at least 1 − ε over
the rotation if every randomised decision in it uses a constant valid at δ = ε/(N_c·L), where N_c is at least the number
of live rows whose bounds can affect the result at the query's snapshot, and N_c, the item set and the filter do not
depend on the rotation. The row count satisfies that; inserts crafted from the seed or from observed failures do not.
Inserts and deletes do not break it, because the guarantee is per query at its snapshot, not uniform over the index's
life. Mixing constants inside one result breaks it (2²⁰ rows at 2²⁰ plus 2²⁰ more at 2²¹ gives 1.5ε), so a cache must
drop its results when the power rises; a fall is safe, and churn at a fixed power is fine. A cached all-items result
masked per viewer is sound if, when served, every decision about a still-live row was made at a constant for an N_c at
least the live rows it covers then, and it was computed over all rows. A one-sided result is valid only for its own
(query, threshold), and reusing its intervals for another threshold or for k-NN needs the two-sided budget. Each power
needs its own valid constant, a certificate at a ln δ at most the one needed or R4, never an interpolation. Each doubling
of N adds about ln 2/c ≈ 0.08 to c.

## 3.10 Prior work

The union over the k true neighbours and the exact law of the error coordinate are published, and the chapter builds on
them by name (`reports/surge2/literature.txt`).

- ADSampling (Gao and Long, SIGMOD 2023, arXiv:2303.09855), Section 4, Corollary 4.2: for methods that generate
  candidates in one batch, correct distance comparisons for the K nearest objects rather than all N suffice, with
  log(DK/δ′) in place of log(DN/δ′) in its cost bound; objects that pass get exact distances; the constant c₀ is
  unspecified and ε₀ = 2.1 is set empirically. [cited, unchecked] (read through a summariser).
- Loh and Lee, "Per-Query Recall Guarantees for Approximate Nearest Neighbor Search in Dynamic Vector Databases"
  (PQR-Search, arXiv:2611.07579, 5 November 2026, v2), a concurrent preprint. Its Theorem 2 bounds Pr[result not exact] by
  a sum over unvisited true neighbours and stages, at most k·β with β = δ/k, and says a sum over all n vectors would need
  δ/n, as range queries do. Its Eq. 5 is the exact Beta law, G_D(−s) = ½·Pr[Beta(½, (D − 2)/2) > s²/(D − 1)], with
  quantiles in place of a constant, and its Lemma 1 the error identity. It also has a slack factor below 1 that is the
  decision form of the self-normalised width, a threshold that shrinks during the search (Theorem 5, the argument of
  t̂ ≤ s_k), a bitmap-filter corollary (Corollary 2) and an exact mode. It credits the k-neighbour union to ADSampling.
  [cited] (read from a crude text extraction of the PDF; tables not read).
- RaBitQ (arXiv:2405.12497), Appendix B.3, states the exact density of the error coordinate, proportional to
  (1 − x²)^((D−3)/2), and its independence of the code, but its Theorem 3.2 uses 2·exp(−c₀ε₀²) with c₀ unspecified, and it
  sets ε₀ = 1.9 empirically. [cited, unchecked] (summariser).

The literature check did not find the centre correction with a stored per-item scalar (Theorem 3.11 (i)), or the
closed-form two-sided interval end and its use as a deterministic interval (Theorem 3.11 (ii)–(iii)). For the
self-normalised width it found the decision form only, in PQR-Search's slack factor. That search covered the RaBitQ
line, ADSampling, DDC, MRQ, SAQ and PQR-Search, in about eight minutes of reading, and is not a proof of novelty. The
threshold results came a day later: the second pass reports that the one-sided count of Theorem 3.13, the count-based
unions of Theorems 3.14–3.16 and the inequality of Proposition 3.17 are not in the literature notes, which never searched
for them (`reports/surge/bounds2.txt`, "Related work").

## Open

- **Tail without the factor 2** [conjecture]. Pr[|u₁| ≥ s] ≤ (1 − s²)^(m/2) for s ≤ 1/√2; it holds on a grid for
  2 ≤ m ≤ 5,000, with equality at m = 2, s = 1/√2 (`proofcheck/constants.py`). It is Conjecture 2.18 of chapter 2.
- **√k_W is unavoidable for a class unknown at build** [conjecture]. Known only for a uniformly random W. It is
  Conjecture 2.17 of chapter 2.
- **Concavity of F** (Theorem 3.22) [conjecture; measured]. A proof would make every tangent plane a valid set of
  per-block constants without a certificate.
- **R9** [sketch: gap named]. A construction of N items whose failure events overlap negligibly under shared rotations.
- **T5's rule** [conjecture]. A lower-tail bound for the undecided count D_ℓ under one shared rotation that beats Markov.
- **T4's gaps**. A seed-free bracket on tan φ at 6 bits, and the proof for Theorem 3.11's width.
- **A tighter certify-in constant**. The referee's route (`reports/surge/referee.txt` section 4): with V_a the fixed set
  of items within a sd of t, on the event that every item of V_a has error below (c_N − a) sd, all of V_a is undecided at
  c_N, so c(observed) ≥ c(|V_a|). Not proved for ties at the boundary of V_a, several levels, the self-normalised width or
  blocks above 1,024 dimensions, and not tested.
- **A global grid fitted on rotated items** (Lemma 3.1's conditions). Unproved; per-item scales avoid it.
- **Interval-verified certificates**. The certified constants are floating-point computations with margins; R4 is the
  fallback that needs none.
- **The Brownian count** of Proposition 3.24 [sketch].
- **The time gain** of the one-sided budget and the index's own N at 1M needs a quiet machine.

## Editor's queries

1. `proofcheck/REFEREE.md`'s verdict table does not name Lemma 8, Proposition 11 or Proposition 12. Their [proved,
   refereed] tags rest on the header of `CONSOLIDATED_PROOFS.md` ("results that survived an independent referee
   check"), on `reports/surge/referee.txt` section 2 for Lemma 8, and on REFEREE.md's "Other checks" paragraph for
   Proposition 12's remark. Nothing records a referee reading Proposition 11's proof.
2. The closed form Pr[U ≥ s] = ½·I_{1−s²}((d − 1)/2, ½) is used without proof in every source. It is the standard Beta
   law of u₁², and the referee checked it numerically; whether it counts as proved here is the PI's call.
3. Proposition 10's remark says solving the exact tail "shrinks c by 0.74 / 0.64 / 0.54". The referee's reproduction
   (0.741 / 0.638 / 0.537) and the size of the numbers show these are factors, and section 3.3 says "multiplies c by".
4. Theorem 3.13 (T1) is tagged [proved, refereed] although `bounds2.txt` says every proof in it is unchecked. The tag
   rests on two referee reports that checked the same argument as applied: `reports/bounds3/referee/REFEREE.txt` item 8
   (thresholds, harmful side only, sound as coded) and `reports/surge/referee.txt` section 4 ("each item has one harmful
   side"). If the PI wants T1's own text refereed, downgrade to [proved, unrefereed].
5. Theorem 3.16 (T4), step (ii), writes the threshold a near item's error must pass as c_min − β(c_far − c). The direct
   computation gives c − β(c_far − c), which is at least c_min − β(c_far − c) because c ≥ c_min, and both are at least
   c_acc. The statement stands either way; the step is transcribed as written.
6. `bounds2.txt` section 3 argues that block rotations give up part of the exact-tail gain, and its constants table says
   block layouts lie between one rotation and the sub-Gaussian column. `bounds3/REPORT.txt` item 2 refutes this for the
   BioCLIP-2 layout (8.7659 against 8.7653). The chapter follows the third pass.
7. The level-0 report tags its backstop "[proved]" for the self-normalised width; the referee refuted it for that width
   with a counter-example and kept it for the plain width. The chapter follows the referee.
8. `rust/REPORT_BOUNDS.md` says the one-sided union "would double k-NN's failure bound". That is true of the `knn2d` loop
   (Theorem 3.23 (ii)), but Theorem 3.10's class uses only upper bounds and is one-sided as it stands. The two statements
   concern different selection rules.
9. The native certifier finds 8.775 two-sided where the report certifies 8.774; `REPORT_CONST.md` attributes the 0.001 to
   its starting point. Both are above the referee's exact 8.7659.
10. `agent3a/REPORT.md` says Theorem 5 "also covers items the client inserts adaptively". The consolidated statement
    covers only queries, and REFEREE.md requires the corpus to be independent of ω. For a client that sees only answers
    with d = 0, the ideal-sequence argument looks as if it would extend to inserts, but no source proves it, so the
    chapter leaves inserts out of Theorem 3.6.
11. SYSTEM.md section 3 lists blocks 0–2 as coded at 96 and 160 B and all four at 256 B at 1M. `REPORT_CONST.md` heads
    its three-block column "96 / 160 / 256 B, blocks 0–2 (1M only)", which includes 256 B. The 50k dump codes all four
    blocks from 96 B, which agrees with "1M only". Which levels code three blocks at 1M (and whether it varies by group)
    is for the PI to settle from the build log; section 3.9 follows SYSTEM.md, and an earlier draft's 50k column for the
    three-block row, which no source gives, has been removed.
12. `bounds2.txt` section 2 tags the one-sided upper end of Theorem B [proved] with no cap: "the event implies t ≤ the
    larger root". The surge referee (`referee.txt` section 3, dated the day before) found that with the upper event alone
    the end is too low when Ê > b in 1,931 of 20,000 cases, by up to 0.40·b, and right in all cases only with the cap at
    b. The second pass's argument looks wrong for Ê > b, where v − Ê − ξ√(b² − v²) is negative at v = b, so the event does
    not confine v below the root. The chapter keeps bounds2's tag and states the cap as a condition.
