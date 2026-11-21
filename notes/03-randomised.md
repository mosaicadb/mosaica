# 3. The randomised levels: the error law, the guarantee, the constants

A randomised level codes the error that the deterministic prefix leaves, after a Haar rotation drawn from the build's
seed ω. Because the rotation is random, the direction of each block's coding error is uniform on a sphere, and the
failure probability of an interval built on that error is known exactly. This chapter gives that law (section 3.2),
the tails it implies (3.3), what a probability over ω does and does not promise (3.4), the guarantees for k-nearest-
neighbour (k-NN) queries (3.5) and for threshold sets (3.6), and the confidence constants that turn a guarantee into a
half-width (3.7). Sections 3.8 to 3.10 give the measured effect of the constants, the constants the index uses, and the
prior work the chapter builds on. Four lettered sections were added later: §3.2a says what a rounded copy of the
rotation on the query side costs, §3.4 ends with what the law needs of the rotation and what the seed's width means,
§3.6a corrects R9 on the union over N items, and §3.7a gives a lower bound for the confidence constant in one block.
Chapter 1 treats the deterministic certificates in front of these levels, and chapter 5 the approximate mode that
reuses their intervals.

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
| d (unsubscripted, in §3.2, §3.3, Theorem 3.11, δ_d, §3.6a and §3.7a) | the degrees of freedom of a single block, d_j with j dropped; in §3.6a and §3.7a the block lives in ℝ^(d+1) and its unit sphere is S^d; where §3.4 and §3.6 speak of a tolerance d, d is chapter 0's tolerance |
| m, s (Propositions 3.2 and 3.3) | the dimension of a sphere's space and a tail level; elsewhere m is a centre and s(x) a score, as in chapter 0; in §3.7a m is the number of grid points of Lemma 3.41 |
| ζ_d(δ), ζ, ζ₀ (§3.6a, §3.7a) | the upper δ-quantile of U in ℝ^d, Pr[U ≥ ζ_d(δ)] = δ, so ζ_d(δ)·√d is the one-block constant c with δ_d(c) = δ; ζ₀ is a smaller trial value. The confidence track writes q and q₀ |
| T_d(x) | Pr[U ≥ x] for U one coordinate of a uniform unit vector in ℝ^d, equal to 1 below −1 and 0 above 1; T_d(c/√d) = δ_d(c). The subscript d distinguishes it from T(a; x) |
| **c**, B, ν (§3.6a, §3.7a) | the stored message of one block of an item, one of M = 2^B values; ν = B/d, the rate in bits per degree of freedom (the track writes c and R) |
| 𝒞, x₀, Δ′ | a codebook; the decoded direction Rᵀ·(codeword), a unit vector; the rotated query offset R·Δ (per block, Δ′_j) |
| Ω_d(θ), C_d | the normalised measure of a cap of angle θ on S^d; C_d = Γ((d+1)/2)/(√π·Γ(d/2)) (the track writes κ_d) |
| θ, θ_ν | a cap angle; sin θ_ν = 2^(−ν), so tan θ_ν = (4^ν − 1)^(−½) (the track writes θ_R) |
| 𝓛, ŷ, κ, h₁, e_∥, e_⊥ (§3.7a) | the class of schemes with estimate Δ·ŷ(**c**, ω) and half-width h₁(**c**, ω)·‖Δ‖; κ = ‖ŷ‖; with e = ŷ − y, e_∥ = e·y and e_⊥ = ‖ŷ − (ŷ·y)y‖ (the track writes η, a and b) |
| ŝ, h (§3.7a) | a scheme's estimate of Δ·y and its half-width, any functions of (**c**, Δ, ω) in the model; Ê keeps its meaning, the unbiased estimate Δ·x₀/cos φ |
| W_ζ(φ) | ζ·tan φ/√(1 + ζ²tan²φ) for φ < π/2, and 1 for φ ≥ π/2 (the track writes H_q) |
| ξ² (§3.7a) | ζ²·tan²φ, which is Theorem 3.11's ξ² for one block of unit norm (the track writes k) |
| v (§3.6a, §3.7a) | Δ·y for ‖y‖ = ‖Δ‖ = 1, the latitude of the query relative to the item (the track writes s); it is §3.5's v in these units; in Lemma 3.41, v_i are grid latitudes, not block contributions |
| ψ_m | the half-spacing π/(2(m − 1)) of the grid of Lemma 3.41 (the track writes ε_m) |
| w*, R_cert, R_sd | the least mean half-width in 𝓛 at rate ν, and the rates a certificate and a standard deviation of a given size need (§3.7a) |
| F_i, p(ω), p̄, π₂ (§3.6a) | item i fails; p(ω) = Pr[F_i given ω]; p̄ = E p(ω); π₂ = E p(ω)² = Pr[F_1 ∩ F_2] |
| λ (§3.6a) | M·Ω_d(θ), the expected number of codewords within θ of an item (the track writes Λ) |
| I₀(θ), I(θ) | the pair terms of Lemma 3.31 from its steps (c) and (c′) (the track writes J); the bare subscripted I_{1−s²} of §3.3 is the incomplete beta function |
| χ (Lemma 3.32 only) | the cosine of the angle between two items' error components; chapter 0's hazard rate does not appear here |
| I*(θ), τ, G_i (Lemma 3.31) | either pair term I₀ or I; τ = −Δ′·c′ for a codeword c′, not chapter 0's τ_k; G_i(c′) the event that item i fails through codeword c′ |
| n, μ, T_F, v′, g′, t′, B_s, S (§3.4's last part) | the block's number of positions, with d = n − 1; a law on O(n), and in Theorem 3.43 the seed's law; the Funk transform; Rb; the codebook's error field; a query norm; the seed's bits; its number of equally likely values (S(t) of chapter 0 is not used there) |
| L(x), C′_d, c₀ (Corollary 3.44's proof only) | −ln(1 − x²); Γ(d/2)/(√π·Γ((d − 1)/2)); a constant with ζ ≥ c₀/√d; elsewhere L is the number of levels |
| ϑ, ζ(u), θ(u) (Corollary 3.45) | the share of exceptional mass given to the tail (the track writes ρ); the quantile and cap angle at level u |
| ĉ, φ̂, ξ̂, κ̂, g, t̄, ϵ_g, ê_∥, ê_⊥ (Propositions 3.36 and 3.47) | cos φ rounded down on the grid; arccos ĉ; ζ·tan φ̂; the scale from ĉ; cos φ/ĉ; √(1/ĉ² − 1); the bias moved by rounding (the track writes ζ); e_∥ and e_⊥ at φ̂ |
| 𝓔₁ (Proposition 3.48) | the one-sided event {−U ≤ ζ} (the track writes E₁, which here is block 1's error) |
| R̃_j, ρ_R,j, η_R,j, dk_j (§3.2a) | a rounded copy of the rotation R_j held for queries; a bound on ‖R̃_j − R_j‖₂; a bound on the relative error of the computed product; the relative rounding of the decoded κ_j (`dk` in the kernel) |
| D_ℓ | the number of items still undecided after level ℓ (chapter 1's D_ℓ, a level's weights, does not appear here) |
| K (Proposition 3.17 and its rule) | the factor 2/(1 − 2δ(c)); in the certificate method, K is a grid resolution; chapter 0's cluster count K does not appear here |
| M, M_j | the number of subspaces in a union (after Theorem 3.5) and the caps of Theorem 3.15; in §3.4's last part, §3.6a and §3.7a, M is the number of messages or codewords of a block (and of a density ratio in Theorem 3.26); chapter 0's metric M does not appear here |
| G | the event of Theorem 3.14, and the surge referee's gap in S(t + G) ⊆ R ⊆ S(t − G) (§3.6); chapter 0's group count G does not appear here |
| β, P | the scale bracket and the set of near items of Theorem 3.16 |
| η | a slack: the radial-error and floating-point slacks after Lemma 3.1, and the lattice step of the certificate method (chapter 1's item scale η does not appear here); η_g is the ratio of a geometric grid for a stored scalar (§3.7a), and η_R,j is defined above |
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

## 3.2a A rounded copy of the rotation on the query side

The dense Haar matrices are 9.93 MB in f64 at n = 2,048, and a query reads them once per group. This section shows
that the query may use a copy rounded to f32 or int16 while the build keeps encoding against the f64 matrix, and that
the cost is a deterministic term added to the half-width, with the constant unchanged. The source is the rotations
track, `notes/program/rotations/CLAIMS.md` section E, and its referee, `notes/program/rotations/referee/referee-1.md`
section "E1, E2"; the track's ρ and η are written ρ_R and η_R here.

**Proposition 3.25 (E1 of the rotations track; the allowance).** Let the build encode block j against R_j as now:
z = R_j·y_j, ō = 𝒞(z), and κ_j and tan φ_j computed from (ō, z). Let the query hold any matrix R̃_j and compute x̃, its
floating-point value of R̃_j·Δ_j. Let ρ_R,j ≥ ‖R̃_j − R_j‖₂ and η_R,j ≥ ‖x̃ − R̃_jΔ_j‖/‖Δ_j‖. Then

    | κ_j⟨ō, x̃⟩ − κ_j⟨ō, R_jΔ_j⟩ |  ≤  ‖y_j‖·sec φ_j·(ρ_R,j + η_R,j)·‖Δ_j‖.

So if h is a valid half-width for the exact estimate, h + Σ_j (ρ_R,j + η_R,j)·‖y_j‖·sec φ_j·‖Δ_j‖ is a valid half-width
for the computed one, with the same constant and the same δ, for any data. As a share of the random term c·sd_j of
block j the allowance is at most (ρ_R,j + η_R,j)·√d_j/(c·sin φ_j).

*Proof.* κ_j⟨ō, x̃ − R_jΔ_j⟩ is at most κ_j‖ō‖·(‖x̃ − R̃_jΔ_j‖ + ‖(R̃_j − R_j)Δ_j‖), and κ_j‖ō‖ = ‖ŷ_j‖ = ‖y_j‖/cos φ_j.
The exact estimate's error is untouched, because ō, κ_j and tan φ_j are functions of R_j·y_j alone and Lemma 3.1 is
applied to R_j. sec φ_j is computed from the stored tan φ_j and ‖y_j‖, both rounded up, and the term increases in each.
The share is ‖y_j‖ sec φ_j·ρ_R,j·‖Δ_j‖ divided by c·‖y_j‖ tan φ_j·‖Δ_j‖/√d_j. ∎

[proved, refereed] `notes/program/rotations/CLAIMS.md` E1; referee "E1, the inequality: no error found". The bound is
pointwise in ω, so it may use a ρ_R,j computed from the realised matrices. The referee attacked it with the worst query
for each item, Δ ∝ (R̃ − R)ᵀō: the change of the estimate reached 0.52 to 0.58 of the allowance and never exceeded it
(300 items, n_j = 256 and 768, depths 1, 4 and 8, f32 and int16) [measured, simulation, `ref_e.py`]. So the allowance is
within a factor 2 of attained.

**The build rule.** The build must go on encoding against its f64 matrix: it must not encode against R̃_j, or derive κ_j
or tan φ_j from a product with R̃_j. Otherwise the codeword is a function of R_j·y_j + (R̃_j − R_j)·y_j, and R̃_j − R_j is a
function of R_j not carried by R_j·y_j, so Lemma 3.1's premise fails. Proposition 3.25 puts no condition on R̃_j itself.
The referee asked whether the rule is necessary, and found it exact for int16 and int8 and overstated for f32
(referee, "Is 'the build must not encode against R̃' necessary?").

- For the exact law it is necessary. With a grid as coarse as the entries (step 1/8 at n_j = 64) and the one-bit code
  taken from R̃y, the standardised error had mean +0.046 (90 standard errors) and Pr[> 2 sd] = 0.0234 against the law's
  0.0223; with step 2⁻¹⁰ nothing showed in 4·10⁶ draws [measured, simulation, `ref_e_build.py`].
- For a guarantee at f32 it is not. The device after Lemma 3.1 (a code computed from a rounded product costs a factor
  E[2^Q] on δ, Q the coordinates within rounding of a cell boundary) applies when Q is defined from z alone with the
  worst-case radius ‖N_p‖₂·‖y‖ per coordinate, N = R̃ − R: given z the code is one of 2^Q functions of z, and the lemma
  applies to each. The referee estimates E[Q] ≈ 0.35 at n_j = 768 and depth 8 (about 0.0005 at depth 1), a factor of
  about e^0.35 on δ and 0.04 in c at the deepest code [model; the referee's estimate]. That route also needs a radial
  slack of order ρ_R·sec φ·|Ê|, a tan φ rounded up by ρ_R·sec²φ (the present margin of 10⁻⁷ on tan φ is too small at
  depth 8) and the same count for near-ties between the six scales, which was not worked out. [sketch: gap named]
- For int16, E[Q] is in the hundreds and no argument is available. The realised codes from R·y and R̃·y differed for no
  item with f32 and for 1% to 62% of items with int16 [measured, simulation].

So the rule as stated is the simple sufficient one, and it costs nothing, because the build regenerates the f64 matrix
from the seed and no query reads it.

**The referee's repairs to the use of Proposition 3.25** (referee items 1 to 10, in order of weight). Each is
[proved, unrefereed]: the referee's own argument, not re-read by anyone.

1. *The slot form needs the factor (1 + dk_j).* The kernel adds a per-block allowance for the f32 cast of the rotated
   weights, `ewj` in `cluster_q2w` (`rust/src/v2/q.rs` line 570), into the slot `ekw` (line 1512), and the level kernel
   multiplies that slot by the decoded κ (`hpar += kap·ekw`, line 1186), while the κ-rounding term is dk_j·|Ê_j| with Ê_j
   the computed block estimate (line 1185). Proposition 3.25 is stated with the true κ_j. Write
   κ_dec⟨ō, x̃⟩ − κ_j⟨ō, R_jΔ_j⟩ = (κ_dec − κ_j)⟨ō, x̃⟩ + κ_j⟨ō, x̃ − R_jΔ_j⟩: the first term is what dk_j·|Ê_j| covers, and
   the second needs the true κ_j ≤ κ_dec·(1 + dk_j). So the slot term √n_j·ρ_R,j·‖Δ_j‖ must be multiplied by (1 + dk_j),
   or replaced by Proposition 3.25's own form ‖y_j‖·sec φ_j·ρ_R,j·‖Δ_j‖ from the stored scalars, which has no decoded κ
   in it. With f32 the shortfall, about 6·10⁻⁹·tan φ·‖y‖‖Δ‖ at depth 1, lies inside the kernel's factor (1 + 10⁻⁶),
   which is worth about 2.8·10⁻⁷·tan φ·‖y‖‖Δ‖ at d = 767; with int16, about 10⁻⁵·tan φ·‖y‖‖Δ‖, it does not.
2. *The int16 figures in the slot.* The slot form √n_j·ρ_R,j·‖Δ_j‖ bounds Σ_p |ō_p|·|((R̃ − R)Δ)_p| since |ō_p| ≤ 1, and
   it exceeds Proposition 3.25's form by √n_j/‖ō‖: 2.0 at depth 1, median 2.6 (largest 3.6) at depth 4, median 3.0 to
   3.5 (largest 4.2) at depth 8 [computed, `ref_e.txt`]. The track said two to three times. So for int16 in the slot the
   allowance is about 0.05 to 0.1% / 0.4 to 0.7% / 6 to 13% of c·sd at depths 1 / 4 / 8 (n_j = 256 to 768); these figures
   supersede the track's int16 figures for any use through the slot. f32 is immaterial either way.
3. *The product's error is against x_j and the centre, not Δ_j.* The query does not form R̃Δ. It forms R̃x per group,
   casts to f32 and subtracts α times the stored rotated centre per cluster (`wmr`, line 566). So the product's error is
   η_R,j·(‖x_j‖ + |α|·‖m_j‖), not η_R,j·‖Δ_j‖, and ‖x_j‖ can exceed ‖Δ_j‖ without bound for a query near a centre. With
   f64 accumulation (η_R about 2·10⁻¹², inside the existing 4u of `ewj`) this changes nothing. For blocked f32 sums
   the bound must multiply ‖x_j‖ + |α|‖m_j‖.
4. *The kernel's f32 bound.* `cluster_q2w` uses max(n/64 + 8, 70)·u (line 543), not (n/64 + 8)·u, for a dot product of
   two f32-rounded vectors against √(Σa²·Σb²). At n = 768 that is 70u, 3.5 times the 20u behind the track's 0.02% and
   1.5% for blocked f32 sums.
5. *The centre tables.* The form with ‖Δ_j‖ holds only if the stored rotated centres and everything derived from them
   (`wmr`, `l1m`, the per-cluster centre tables) are recomputed with R̃_j; otherwise the allowance is ρ_R,j·‖x_j‖, not
   ρ_R,j·‖Δ_j‖. Rotating centres with R̃_j does not break the build rule, because a centre is not coded.
6. *The share is at the rms tan φ, not per item.* The share ρ_R√d/(c·sin φ) is unbounded as tan φ → 0, and per item the
   referee saw up to 0.0028% (f32) and 4.5% (int16) at n_j = 768, depth 8, in 300 items. What is bounded is the
   absolute allowance ρ_R·sec φ·‖y‖‖Δ‖, which for f32 is a tenth of the `ewj` term already carried.
7. *ρ_R rounded outward, and f32 without a norm.* The Schatten bound used for ρ_R is at least ‖N‖₂ in exact arithmetic
   but can sit within 10⁻¹³ of it when one singular value dominates, so it must be evaluated with a margin. For f32 no
   norm need be computed: |N_ik| ≤ 2⁻²⁴|R_ik| entrywise gives ρ_R ≤ ‖N‖_F ≤ 2⁻²⁴·‖R‖_F = 2⁻²⁴·√n_j, which is 1.65·10⁻⁶
   at n_j = 768, a share of 0.07% at depth 8.
8. *Fixed-point weights.* "η_R = 0" for an integer product covers the product alone; converting x_j to fixed point adds
   an input error of (1 + ρ_R)·κ‖ō‖·‖x_fx − x‖, which needs its own term.
9. *Widths that depend on the estimate.* Proposition 3.25 is for a plain width. For the self-normalised widths of
   Theorems 3.11 and 3.12 the allowance enters as Ē = Ê_computed + allowance, not as an added term.
10. *ρ_R is against the matrix the build encoded with, bit for bit.* It holds while every item, inserts included, was
    encoded with the same f64 R. `haar_rotation` is scalar f64 with its own logarithm, so regeneration is reproducible;
    a stored hash of R beside R̃ and ρ_R would make that checkable.

The track's remaining notes on use stand: a certified operator norm by ‖N‖₂ ≤ (tr (NᵀN)^p)^(1/2p), within a factor
n^(1/2p) of the truth (p = 64 by six squarings is within 1.05 at n = 768), where the Frobenius norm is too large by
about √n/2; and, for the product accumulated in f64 with R̃'s entries in f32, int16 or int8,
η_R ≤ γ_n·√n·(1 + ρ_R) with γ_n = n·2⁻⁵³/(1 − n·2⁻⁵³). The build's own matrix is itself a floating-point matrix, and
ρ_R adds to the slack ‖R − P*‖ ≤ ‖RᵀR − I‖_F already carried after Lemma 3.1.

**E2: the figures** [computed, `notes/program/rotations/e_rounding.py`, `out_e.txt`: one Haar matrix per size; entries
rounded to nearest; the integer formats use one scale, the largest entry; ρ_R the Schatten bound of order 128;
floating point, not interval-verified; the referee reproduced the f32 and int16 rows at n_j = 256 and 768 on their own
matrices and code, and did not check int8 or the other sizes]. The built code's rms tan φ at depths 1 to 8 is 0.754,
0.365, 0.194, 0.105, 0.056, 0.029, 0.015, 0.0076 (uniform directions, n_j = 256). Allowance in Proposition 3.25's form, as a
percentage of c·sd at c = 7.8 and the rms tan φ:

| Block n_j | f32: ρ_R; share at depths 1 / 4 / 8 | int16: ρ_R; share at depths 1 / 4 / 8 |
|---|---|---|
| 256 (track) | 5.0·10⁻⁸; at most 0.0014% | 8.9·10⁻⁵; 0.030 / 0.18 / 2.4% |
| 256 (referee) | 5.10·10⁻⁸; 0.000017 / 0.00010 / 0.0014% | 7.4·10⁻⁵; 0.025 / 0.145 / 2.0% |
| 768 (track) | about 5·10⁻⁸; at most 0.0024% | 8.3·10⁻⁵; 0.049 / 0.28 / 3.9% |
| 768 (referee) | 5.15·10⁻⁸; 0.000030 / 0.00017 / 0.0022% | 8.2·10⁻⁵; 0.049 / 0.27 / 3.6% |

The int16 ρ_R moves by ±20% with the draw's largest entry. With the Frobenius norm in place of the certified operator
norm the shares are 4 times larger at n_j = 64 and 14 times at 768. int8 (track only) costs 8 to 13% at depth 1 on the
large blocks and is unusable from depth 4. Accumulating the product naively in f32 gives a worst case γ_n·√n of 0.75 /
4.3 / 59% at n_j = 768 and depths 1 / 4 / 8, so the product must be accumulated in f64, in exact integers, or in blocked
f32 sums with the kernel's bound as corrected in item 4 and against the vector of item 3.

Read with repairs 1 and 2: f32 on the query side is free at every block size and depth, whether through Proposition
3.25's form or the slot; it halves the bytes (4.97 MB at n = 2,048). int16 quarters them (2.48 MB) and costs under 0.3%
of the random term up to depth 4 in Proposition 3.25's form, or 0.4 to 0.7% at depth 4 and 6 to 13% at depth 8 in the
slot form. The slot form is valid only when its term carries repair 1's factor (1 + dk_j). Nothing was timed.

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

### What the law needs of the rotation, and the seed's width

Every result above models the rotation as Haar. The next three results, from the rotations track
(`notes/program/rotations/CLAIMS.md` sections A and C), say exactly what Lemma 3.1 needs of the rotation's law, and what
a rotation drawn from finitely many seeds can promise. Here y is one block of n positions, z = R·y, ō a codeword that is
a function of z, κ = ‖y‖²/⟨ō, z⟩, e = κō − z the error in rotated coordinates, E = Rᵀe, A = ‖e‖ and Δ_⊥ = P_{y⊥}Δ; "the
law" is Lemma 3.1's scalar statement that, given z, Δ·E has the law of A·‖Δ_⊥‖·U with U one coordinate of a uniform unit
vector of ℝ^(n−1). In this part d = n − 1, the block's degrees of freedom, and not chapter 0's tolerance.

**Theorem 3.26 (A1 of the rotations track; the exact condition).** Let μ be a law on O(n), n ≥ 3. The following are
equivalent.

1. For every y, every Δ fixed independently of R, and every codebook (every measurable ō(z) with ⟨ō, z⟩ > 0), the law
   holds.
2. For every orthonormal pair (a, b), the pair (Ra, Rb) is uniform on the Stiefel manifold V₂(ℝⁿ).

For a bound in place of the exact law the following suffices: if for every orthonormal (a, b) the conditional law of Rb
given Ra has a density at most M against the uniform law on the unit sphere of (Ra)^⊥, then for every codebook
Pr[Δ·E ≥ x | z] ≤ M·Pr[A·‖Δ_⊥‖·U ≥ x], and with J independently rotated blocks the factor is M^J.

*Proof.* (2 ⇒ 1) Let a = y/‖y‖ and b = Δ_⊥/‖Δ_⊥‖. Uniformity on V₂ means that given Ra the vector v′ = Rb is uniform on
the unit sphere of (Ra)^⊥ = z^⊥. Now RΔ = (Δ·y/‖y‖²)·z + ‖Δ_⊥‖·v′ and e ⟂ z, so Δ·E = ⟨e, RΔ⟩ = ‖Δ_⊥‖·⟨e, v′⟩. Given
z, e is a fixed vector of z^⊥ of norm A and v′ is uniform on the unit sphere of z^⊥, a space of dimension n − 1, so
⟨e, v′⟩ has the law of A·U.
(1 ⇒ 2) Take y = a and Δ = b. For a unit vector field e(z) ∈ z^⊥ and a positive function t(z), the codebook
ō = z + t(z)·e(z) has κ = 1, error t(z)·e(z) and A = t(z). By 1, ⟨e(z), Rb⟩ has the law of U given z for every such
field. Apply this to the fields e_w(z) = P_{z⊥}w/‖P_{z⊥}w‖ for w in a countable dense set: given z, every one-dimensional
marginal of Rb in z^⊥ is that of the uniform law on the sphere of z^⊥, so by Cramér–Wold the conditional law of Rb is
that uniform law. It remains to show that Ra is uniform. Write T_F for the operator that sends a law ν on the sphere to
∫ unif(S(x^⊥)) dν(x). The conditional statement for (a, b) gives law(Rb) = T_F·law(Ra), and for (b, a) it gives
law(Ra) = T_F·law(Rb), so law(Ra) = T_F²·law(Ra). T_F commutes with rotations, so it multiplies the degree-k spherical
harmonics by a constant λ_k, the value at 0 of the normalised Gegenbauer polynomial: λ_k = 0 for odd k and |λ_k| < 1 for
even k ≥ 2 when n ≥ 3. So every harmonic component of law(Ra) of degree at least 1 vanishes, and Ra is uniform.
The bound with M: given z the density of v′ is at most M times uniform, so every conditional probability is at most M
times its uniform value; blocks are independent. ∎

[proved, refereed] `notes/program/rotations/CLAIMS.md` A1; `notes/program/rotations/referee/referee-1.md` "A1: no error
found". T_F is the track's T (the Funk transform), renamed here because T(a; x) is taken. The referee adds two unstated
points, which do not change the statement. Statement 1 holds for almost every z. And "the law" is the scalar law for one
Δ: pair-uniformity does not give the law of the whole vector u of Lemma 3.1, so a result that uses more than one direction
of E at once, such as Theorem 3.5's subspace classes, is not covered by condition 2 and would need uniformity of (Ra, R·W).

Haar measure satisfies 2, and so does any law invariant under R ↦ R·Oᵀ for every O fixing the block; asked for every
block, those O generate O(n), so that sufficient condition is Haar itself, while pair-uniformity is strictly weaker
(the track's H2, outside this chapter). A density ratio M costs c² → c² + 2·ln M at a sub-Gaussian tail, since the budget
becomes δ/M; a total-variation distance τ from pair-uniformity gives only Pr ≤ δ + τ, which needs τ ≲ δ.

**Proposition 3.27 (A2 of the rotations track; a finitely supported rotation leaves nothing random).** Let R take
finitely many values R₁, …, R_S. For every y outside the finite union of proper subspaces ker(R_s − R_t), s ≠ t, the
matrix R is a function of z = Ry. For such y and any Δ not parallel to y there is a codebook, a function of z alone, for
which Δ·E = A·‖Δ_⊥‖ with probability one: the error sits at its Cauchy–Schwarz size, √d·sd.

*Proof.* If R_s y = R_t y then y ∈ ker(R_s − R_t), a proper subspace when R_s ≠ R_t. So z determines R, and RΔ_⊥ is a
function g′(z) with g′(z) ⟂ z. Take ō(z) = z + g′(z): then κ = 1, e = g′(z), and Δ·E = ⟨g′(z), RΔ⟩ = ‖Δ_⊥‖² =
A·‖Δ_⊥‖. ∎

[proved, refereed] `notes/program/rotations/CLAIMS.md` A2; referee "A2: no error found". The proposition refutes the
exact conditional law, and "any codebook", for a codebook built with knowledge of Δ and of the family. It does not refute
a tail bound for the built codebook, which needs only the joint law of (e(z), RΔ).

The referee's point on its scope is this. Proposition 3.27 applies word for word to the index's own dense matrix, which
is a function of a 64-bit seed and so takes at most 2⁶⁴ values, fewer than the 2^(kn) of k sign rounds with truly random
signs. So for the present build too, "for every codebook" holds only in the Haar model, and Proposition 3.27 cannot be the
reason to reject products of sign diagonals and Hadamard matrices. The reasons are explicit failing inputs for the built
code under one to four such rounds (the track's section B) and Proposition 3.28 (referee, "A2"). [proved, unrefereed:
the referee's remark]

**Proposition 3.28 (C1 of the rotations track; a budget below the seed's atom forces a deterministic bound).** Let the
rotation be a function of B_s fair bits, or take S equally likely values (B_s = log₂ S). (i) For fixed (y, Δ) every event
has a probability that is a multiple of 2^(−B_s), so a half-width that is δ-valid with δ < 2^(−B_s) never fails.
(ii) Let the half-width be h = h(A, ‖Δ_⊥‖), a function of the stored error scale and of the query's norm, as c·sd is. If
it never fails for any fixed pair, then h(A, t′) ≥ A·t′ for every value A that occurs: it is at least the Cauchy–Schwarz
bound, which is c = √d in units of sd.

*Proof.* (i) is counting. (ii) Take y and a seed value ω₀, with rotation R₀ and error vector E₀ = R₀ᵀe(R₀y) of norm A₀,
orthogonal to y. The vector Δ = t′·E₀/A₀ is a fixed query, orthogonal to y, so ‖Δ_⊥‖ = t′. At ω = ω₀, an event of
positive probability, the error is A₀·t′ and the stored scale is A₀, so h(A₀, t′) ≥ A₀·t′. ∎

[proved, refereed] `notes/program/rotations/CLAIMS.md` C1; referee "C1: no error found". B_s is the track's B, renamed
because B counts a message's bits in §3.7a. For blocks with d ≤ c² the width c·sd is already at least Cauchy–Schwarz,
so nothing is lost by a deterministic bound there.

**The 64-bit seed.** The rotations track drew the following from Proposition 3.28 (C2 of the track). If every rotation
comes from one 64-bit seed, S ≤ 2⁶⁴ and every per-event probability is a multiple of 2⁻⁶⁴ = 5.42·10⁻²⁰. The one-sided
budget δ = ε/(N·L) = 2.5·10⁻¹⁹ at N = 10⁹, L = 4 and ε = 10⁻⁹ is 4.6 atoms; with N rounded up to a power of two it is 1.07
atoms at N = 2³² and first falls below one atom at N = 2³³. [computed; arithmetic checked by the referee] The track
concluded that a build for more than about 4·10⁹ rows should draw its rotations from a seed of 128 bits or more.

The referee withdrew that conclusion (referee, "C2"). The guarantee is the answer-level one,
Pr_ω[answer wrong] ≤ ε = 10⁻⁹, which is 1.8·10¹⁰ atoms at every N. The per-event δ is a step in a union bound under the
Haar model, not a promise about seeds. For a generator that behaves like 2⁶⁴ independent Haar draws, the number of seeds
failing one event is about Poisson(δ·2⁶⁴), and the fraction of seeds that fail a query is ε to within one part in 10⁵,
whether δ·2⁶⁴ is 4.6 or 0.5. So nothing changes at one atom, and "at most four seeds fail a given event" is not something
the build establishes at N = 10⁹; it is what δ-validity would mean if it were proved for the generator, and it is not. A
longer seed is harmless and makes nothing provable. [proved, unrefereed: the referee's argument, a model of the
generator as independent Haar draws] The advice to use a longer seed above 4·10⁹ rows is withdrawn.

**The seed as built.** The build's seed is a public compile-time constant: `rust/src/index.rs` line 5 reads
`pub const SEED: u64 = 20261112;`, and SYSTEM.md speaks of "a public seed" (referee, "C2"; the line was confirmed for
these notes). So as built S = 1: nothing is drawn, every event has probability 0 or 1 for given data and query, and
every guarantee of this chapter is a statement in the Haar model, for data and queries that do not depend on the
constant. This sharpens the remark on the public seed above, which treats a party that reads the seed; as built, every
party can. If a real probability over ω is wanted, the change that buys it is drawing the seed at build from the system's
entropy and storing it, at any width, since the width matters only against ε and not against δ.

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

## 3.6a The union over N items: R9 corrected

Every threshold constant above pays a union over the items that reach a randomised level, which costs about 2·ln N in c².
R9, stated in §3.7, claimed that no per-item rule can avoid it. This section gives the referee's counter-example to R9 as
worded, the form of R9 that is true, and a theorem that the union is attained for one block coded by the plain rule with
a random shared codebook. The sources are the confidence track's `notes/program/confidence/CLAIMS.md` part C2 (revised
after the referee) and its referee, `notes/program/confidence/referee/referee-lower-bounds.md` section 6.

*Setting.* Block j of every item in a group is rotated by the same Haar rotation R_j, and any codebook is shared; ω is
the whole shared seed. A **per-item rule** is any rule under which item i's decision at each level (exclude, include,
read on) is a function of item i's own stored messages, the seed ω and the query. Item i **fails** (event F_i) if some
level excludes it although it matches, or includes it although it does not.

**R9 as worded is false** [refuted; the counter-example is the referee's, section 6]. R9 claimed that for any per-item
rule an answer exact with probability 1 − ε on every corpus of N items forces the per-item failure probability below
(ε/N)(1 + o(1)). Fix a set S′ of rotated queries with Pr[Δ′ ∈ S′] = ε, and let the rule exclude every item when Δ′ ∈ S′
and otherwise read every item to the end. This is a per-item rule. The answer is wrong with probability exactly ε on every
corpus that has a match, and the mean per-item failure probability is ε, not ε/N. So the union is avoided by any rule that
makes its items fail together. Such a rule gains nothing by it, and Corollary 3.30 says so.

**Theorem 3.29 (Theorem C of the confidence track; independence given the seed).** Let the N items be drawn
independently from one law, independently of ω. For any codebooks, any shared rotations, any number of blocks and levels
and any per-item rule, conditionally on ω the events F_1, …, F_N are independent with a common probability p(ω). Hence,
averaged over the random corpus,

    Pr[some item fails] = E_ω[1 − (1 − p(ω))^N],   N·p̄ − (N(N − 1)/2)·π₂ ≤ Pr[some item fails] ≤ N·p̄,

with p̄ = E p(ω) and π₂ = E p(ω)² = Pr[F_1 ∩ F_2]. So some fixed corpus of N items has
Pr_ω[some item fails] ≥ N·p̄ − (N²/2)·π₂.

*Proof.* Given ω, F_i is a function of item i alone, and the items are i.i.d. and independent of ω. The lower bound is
Bonferroni's second inequality, or (1 − p)^N ≤ 1 − Np + N(N − 1)p²/2. An average over corpora is attained by some
corpus. ∎

[proved, unrefereed] CLAIMS.md C2.1, repaired after the referee: the first version conditioned on Δ′ = (R_jΔ_j)_j alone,
which is false for a random shared codebook (with Δ′ fixed and one random codeword shared by two items,
Pr[F_1 ∩ F_2 | Δ′] was 83 times the product, `ref_sim_c.py` part B) and for a rule that uses the rotations other than
through Δ′. The referee proposed conditioning on ω, which is the statement here.

*The latitude corpus.* Fix the query, a threshold t, and per block a norm and a block score, with the part of the score
read before the randomised levels taken equal for all items. Draw each item's block j independently and uniformly from
the latitude {y : ‖y‖ = given norm, Δ_j·y = given block score}. Then every item has score exactly t, so every item is a
match at margin zero and an upper bound that fails at any level excludes a match: F_i implies a wrong answer. And R_j maps
the latitude of Δ_j onto that of Δ′_j, so for a rule that uses the rotations only through Δ′, p depends on ω only
through (Δ′, codebooks). The corpus must be free to place blocks anywhere on the latitude with one common prefix read.
The referee's simulation, with a fixed query, a corpus on its latitude and one Haar matrix per trial, agreed with this
model in 16 one-block cells and in a two-block, two-level case (Pr[at least one failure] 0.6020 against 0.6014)
[measured, simulation, `ref_sim_c.py`].

**Corollary 3.30 (Corollary Q of the confidence track; R9 in quantile form).** Suppose the answer is wrong with
probability at most ε on every corpus of N items. Take any i.i.d. corpus whose items are at the threshold, so that F_i
implies a wrong answer. Then for every x > 0,

    Pr_ω[p(ω) ≥ x/N] ≤ ε/(1 − e^(−x)).

For example p(ω) ≤ 0.7/N outside a set of seeds of probability 2ε, and p(ω) ≤ 3/N outside one of probability 1.06ε.

*Proof.* Given ω, Pr[some item fails] = 1 − (1 − p(ω))^N ≥ 1 − e^(−N·p(ω)), which is at least 1 − e^(−x) when
p(ω) ≥ x/N. Averaged over ω and over the corpus this probability is at most ε, so Pr_ω[p(ω) ≥ x/N]·(1 − e^(−x)) ≤ ε. ∎

[proved, unrefereed] The statement is the referee's (section 6) and the proof the track's (CLAIMS.md C2.1); nobody has
re-read the pair. It holds for every per-item rule, every code, and any number of blocks and levels. It is consistent with
the counter-example, where p(ω) = 1 on a set of seeds of probability ε. In words: a rule cannot keep a per-item failure
rate above about 1/N on more than a few ε of the seeds; it can only move its failures onto few seeds, where they buy
nothing. In mean form, under the same hypothesis p̄ ≤ ε/N + (N/2)·π₂, so the union is attained up to the factor 1 − γ/2
whenever π₂ ≤ γ·p̄/N with γ < 2 (the track's Corollary C1, [proved, unrefereed]); this bounds a mean over a random
latitude item, and R9's "for every scale vector that occurs" cannot be had, because an item's scales are random through φ.

When p(ω) is nearly constant in ω the union over N is attained. The rest of this section proves that for one block, one
level, the plain rule and a random shared codebook. Both lemmas below are for the *equator corpus*: one block, one
level, ‖y‖ = ‖Δ‖ = 1 and v = Δ·y = 0, so items are uniform on the unit sphere of Δ^⊥; and the plain one-sided rule with
exact φ, half-width ζ·tan φ with ζ = ζ_d(δ), so p̄ = δ up to Pr[φ ≥ π/2] ≤ 2^(−M). In the rotated frame, with z_i = R·y_i,
x_i item i's codeword and φ_i its angle, F_i = {−Δ′·x_i > ζ·sin φ_i} (and φ_i < π/2). Ω_d and C_d are as in Lemma 3.34.

**Lemma 3.31 (Lemma C2 of the confidence track; a random codebook, the pair term).** Let the codebook be M independent
uniform points of S^d, shared by all items and independent of the rotation. For every θ ∈ (0, π/2), with
λ = M·Ω_d(θ),

    π₂ ≤ 2·e^(−λ) + λ²·δ² + M·δ·I*(θ),

where I*(θ) may be either of

    (c)  I₀(θ) = Ω_d(θ)·Ω_{d−1}(θ) ≤ Ω_d(θ)²·(1 + 2/d)/(sin θ·cos θ);
    (c′) I(θ) = ∫₀^θ C_d·sin^(d−1)φ·T_d( cos θ/√(1 − ζ²·sin²φ) ) dφ,

and (c′) is smaller than (c) by a factor of about ((1 − ζ²)/(1 − ζ²sin²θ))^((d−1)/2). So
N·π₂/δ ≤ 2N·e^(−λ)/δ + N·λ²·δ + N·M·I*(θ).

*Proof.* For a codeword c′ let G_i(c′) = {angle(c′, z_i) ≤ θ and −Δ′·c′ > ζ·sin angle(c′, z_i)}. Then
F_i ⊂ {φ_i > θ} ∪ ⋃_{c′} G_i(c′), and Pr[φ_i > θ] = (1 − Ω_d(θ))^M ≤ e^(−λ) (Proposition 3.37). The union over the two
items of {φ_i > θ} gives the first term.
(a) One codeword, one item. Given z_i and the angle φ ≤ θ of c′ from z_i, c′ = cos φ·z_i + sin φ·u′ with u′ uniform on the
unit sphere of z_i^⊥. Δ′ ⊥ z_i, so Δ′·c′ = sin φ·U with U a coordinate of a uniform unit vector in ℝ^d, and
Pr[G_i(c′) | φ] = T_d(ζ) = δ. So Pr[G_i(c′)] = Ω_d(θ)·δ.
(b) Two codewords c′ ≠ c″. Given Δ′, the pairs (z_1, c′) and (z_2, c″) are independent, so
Σ_{c′≠c″} Pr[G_1(c′) ∩ G_2(c″)] ≤ M²·Ω_d(θ)²·δ² = λ²δ².
(c) One codeword for both. Pr[G_1(c′) ∩ G_2(c′)] ≤ Pr[G_1(c′) and angle(c′, z_2) ≤ θ]. Given (c′, Δ′, z_1), z_2 is uniform
on the unit sphere of Δ′^⊥, and z_2·c′ = √(1 − τ²)·U with τ = −Δ′·c′, so
Pr[z_2·c′ ≥ cos θ | c′, Δ′, z_1] = T_d(cos θ/√(1 − τ²)) ≤ T_d(cos θ) = Ω_{d−1}(θ). With (a), the sum over c′ is at most
M·Ω_d(θ)·δ·Ω_{d−1}(θ).
(c′) The same line without discarding τ. On G_1(c′), τ > ζ·sin φ_1, and T_d(cos θ/√(1 − τ²)) decreases in τ, so
Pr[G_1(c′) and angle(c′, z_2) ≤ θ] ≤ E[1_{G_1(c′)}·T_d(cos θ/√(1 − ζ²sin²φ_1))] = δ·I(θ), using (a) for each φ_1 and the
density C_d·sin^(d−1)φ of the angle of c′ from z_1.
The comparison of caps in (c) is Lemma 3.34 in dimensions d − 1 and d, with C_{d−1}·d/(C_d·(d − 1)) ≤ 1 + 2/d:
C_{d−1}/C_d = [Γ(d/2)/Γ((d − 1)/2)]²·2/(d − 1) ≤ 1 by Wendel's inequality Γ(x + ½) ≤ √x·Γ(x) at x = (d − 1)/2, and
d/(d − 1) ≤ 1 + 2/d for d ≥ 2. ∎

[proved, unrefereed] CLAIMS.md C2.2. The referee found no error in the first version, whose step (c) was stated without
the conditioning on z_1 and whose inequality on C_d was checked only numerically for d ≥ 4. The proof by Wendel's
inequality and step (c′) are the referee's (section 6), and (c′) is itself unrefereed. The event's probability for one
uniform codeword was 0.83 to 0.96 of δ·I(θ) in four settings (d = 6, 12, 24; δ = 0.01, 0.002; 10 to 20 million draws)
and never exceeded it [measured, simulation, `c2_revision.py`].

**Lemma 3.32 (Lemma C4 of the confidence track; any code, through the angle between two errors).** For any fixed codebook
(or conditionally on a random one), d ≥ 3, and any two fixed linearly independent items y_1, y_2 orthogonal to Δ, let
e_i = x_i/cos φ_i − z_i be item i's error vector in the rotated frame and χ the cosine of the angle between the components
of e_1 and e_2 orthogonal to span(z_1, z_2), with χ = 1 if the two items decode to the same codeword. Then

    Pr[F_1 ∩ F_2 | z_1, z_2] ≤ T_{d−1}( ζ·√(2/(1 + χ)) ).

*Proof.* If O is a rotation fixing y_1 and y_2 then R·Oᵀ is Haar, gives the same (z_1, z_2), and sends Δ to R·OᵀΔ. So
given (z_1, z_2) the rotated query Δ′ is uniform on the unit sphere of span(z_1, z_2)^⊥, of dimension d − 1. Write
−Δ′·e_i = r_i·W_i with r_i = ‖(e_i)_⊥‖ ≤ ‖e_i‖ = tan φ_i and W_i the coordinate of −Δ′ along (e_i)_⊥. On F_1 ∩ F_2 both
W_i > ζ, so W_1 + W_2 > 2ζ, and W_1 + W_2 = √(2 + 2χ)·(a coordinate of a uniform unit vector of ℝ^(d−1)). ∎

[proved, unrefereed] CLAIMS.md C2.2; the referee found no error (section 6) and supplied the hypotheses d ≥ 3, linear
independence and conditioning on a random codebook, which the revision added. It is Proposition 3.24's method with two
items in place of two levels. The bound is below δ·10⁻¹¹ when χ ≤ χ₀, with χ₀ = 0.52 / 0.45 / 0.34 / 0.33 at d = 95 /
127 / 255 / 287 [computed, `c2_union.log` part 3; reproduced by the referee]. So for any code π₂ ≤ δ·10⁻¹¹ +
Pr[F_1 and χ > χ₀]: the question is how often two unrelated items' errors are aligned by more than about a third (a half on
the smallest block), given that one of them fails. Theorem 3.33 does not use this lemma; it is the route to the built
code (gap G3 under Open).

**Theorem 3.33 (Theorem D of the confidence track; the union over N is attained).** Take one block of d degrees of
freedom, one level, a random codebook of M = 2^B codewords shared by the items and independent of the shared Haar
rotation, the plain one-sided rule with exact φ at per-item level δ, and N items. Let λ = ln(200·N/δ) and
M·Ω_d(θ) = λ. If N·M·I*(θ) ≤ 0.01, with I* from step (c) or (c′) of Lemma 3.31, then there is a corpus of N items, all at
the threshold of one query, with

    Pr_ω[the answer is wrong] ≥ N·δ·(1 − 0.01 − N·λ²·δ/2).

So for this scheme a guarantee of ε on every corpus forces δ ≤ (ε/N)·(1 + 0.011) when N·δ·λ² ≤ 10⁻³: its per-item budget
is ε/N, and not ε/N′ for any N′ < 0.99·N.

*Proof.* Theorem 3.29 on the equator corpus gives a corpus with failure probability at least N·p̄ − (N²/2)·π₂, and
p̄ = δ up to 2^(−M). With this λ, 2N·e^(−λ)/δ = 0.01, so Lemma 3.31 gives N·π₂/δ ≤ 0.01 + N·λ²·δ + 0.01. The failure
probability is therefore at least N·δ − (N/2)·δ·(0.02 + N·λ²·δ). ∎

[proved, unrefereed] CLAIMS.md C2.3. The referee found no error in the version it read (section 6, "for what it
states"); the revision narrowed the title and statement to this scope and added step (c′), and nobody has re-read it. In
Corollary 3.30's terms this is a case where p(ω) is rarely large: π₂ ≤ λ²·δ² + 0.02·δ/N, far below δ/N.

The scope is exactly this: the plain rule, one block, one level, a random shared codebook. The code-size condition is
sufficient only. At N = 10⁹ and δ = 2.5·10⁻¹⁹ (ε = 10⁻⁹, L = 4 entering only through δ), with λ = 68.9, the middle term
N·λ²·δ is 1.2·10⁻⁶ and the last term N·M·I*(θ) is below 0.01 from [computed, `c2_union.log` part 2 and
`c2_revision.log` part 1; reproduced by the referee's `ref_num2b.py`]:

| step used | d = 95 | d = 127 | d = 255 | d = 287 |
|---|---|---|---|---|
| (c), the theorist's | 50 bits | 50 | 51 | 51 |
| (c′), the referee's refinement | 30 bits | 33 | 40 | 41 |

The first row is the slack of step (c), not a boundary. At d = 95 and 29 bits, where M = 5.4·10⁸ is less than N, the
last term with (c′) is 0.026, so there the union is attained within 2% with fewer codewords than items. The track's first
version had drawn two conclusions that the referee showed unsupported and that are withdrawn [refuted as stated]: that
the union needs M ≫ N·ln²(N/δ), and that it fails with fewer codewords than items. What survives of them is the case of
one codeword (the track's Claim E, [proved, unrefereed]): with M = 1 every item of the equator corpus with cos φ_i > 0
fails as soon as τ = −Δ′·c′ ≥ ζ, because sin φ_i ≤ 1, so p(Δ′) = ½ on an event of probability T_{d+1}(ζ) and
π₂ ≥ ¼·T_{d+1}(ζ), of order δ rather than δ², and the union over N is not attained. Where between one codeword and the
table's sizes the union starts to be attained is open; the referee measured the excess Pr[F_2 | F_1] − p̄ for a random
shared code at n = 13 as 0.5/M, 0.25/M, 0.14/M at M = 8, 64, 512 for δ = 0.01 and 0.35/M, 0.10/M, 0.05/M for δ = 0.002, so
the boundary depends on d and δ, not on M/N alone [measured, simulation, `ref_sim_c.py` part D].

What Theorem 3.33 does not cover, each [sketch: gap named]: the factor N·L over levels, which needs
Pr[F_{1,ℓ} ∩ F_{2,ℓ′}] for two items at two levels whose codes are nested refinements, while Lemma 3.31 is for one
codebook and Proposition 3.24 for one item; rules other than the plain one, because Lemma 3.31's terms are bounded
relative to δ while the conclusion needs them relative to the rule's own p̄ (for Proposition 3.46 on the
equator, T_d(ζ√(1 + ξ²)), about 10⁻⁶·δ at d = 95 and one bit), and the equator is the wrong corpus for that interval, whose worst latitude
is v = ξ/√(1 + ξ²); several blocks; and the built scalar code in place of a random one (G3). k-NN, where an item's
decision depends on other items' intervals, is not covered at all.

So for the index: for one block coded with a random code and decided by the plain rule, δ cannot be taken larger than
ε/N by more than 1%. For every per-item rule, Corollary 3.30 forbids a per-item failure rate above about 1/N on more
than a few ε of the seeds. Neither says that the index's constant is forced as built, because its levels have up to four
blocks, use a scalar code and intersect intervals.

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

**R9 (what no per-item rule can beat)** [refuted as worded, 21 November 2026: the confidence track's referee gave a
per-item rule whose items fail together, with mean per-item failure probability ε rather than ε/N; §3.6a gives the
counter-example, the true quantile form (Corollary 3.30) and the case where the union is attained (Theorem 3.33). The
text below is kept as first stated; it was tagged sketch: gap named, unchecked by the referee.] *Claim:* if every decision at a
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

## 3.7a A lower bound for the confidence constant in one block

Sections 3.3 to 3.7 compute the constant that a given rule needs. This section asks what any rule must pay: for one
block coded in B bits, how narrow can a δ-valid half-width be? The answer is proved for a class 𝓛 of schemes that
contains the index's plain rule on one block, and the least mean half-width in 𝓛 is, to first order,
w* = ζ/√(4^ν − 1 + ζ²) with ζ = ζ_d(δ). Nothing is proved outside 𝓛, and 𝓛 leaves out R8, the intersections the
search loops make, the radial slack and several blocks, so the result is not a statement about the index's certificate
beyond one block with the plain rule. The source is the confidence track, `notes/program/confidence/CLAIMS.md` part C1,
revised after its referee, `notes/program/confidence/referee/referee-lower-bounds.md` sections 1 to 5 and 7. Where a
statement below is tagged [proved, refereed], the referee read that statement and its proof in the track and found no
error, and the revision left both unchanged; a proof here may spell out a step the track leaves implicit.

*The model* (CLAIMS.md C1.0; referee section 1: "no error found as a formalisation of one block, one item, one query").
The build draws ω from a probability space, independently of the item and the query. The item's block is a unit vector
y ∈ S^d ⊂ ℝ^(d+1); a lower bound for unit blocks is a lower bound for any scheme that must handle all norms, and the upper
bound scales by ‖y‖ when ‖y‖ is stored outside the B bits. The stored message is **c** = enc(y, ω) ∈ {1, …, M},
M = 2^B = 2^(νd), with enc any measurable function, and nothing else about y is kept. The query offset Δ ∈ ℝ^(d+1) is
fixed independently of ω, and the quantity to bound is Δ·y. At query time the scheme outputs an estimate
ŝ = est(**c**, Δ, ω) and a half-width h = hw(**c**, Δ, ω) ≥ 0, both any measurable functions; in particular h may
depend on the message, so a stored scale such as ‖y‖ tan φ counts inside **c**. The scheme is *one-sided δ-valid* if
Pr_ω[Δ·y − ŝ > h] ≤ δ for every (y, Δ), and *two-sided δ-valid* if Pr_ω[|ŝ − Δ·y| > h] ≤ δ for every (y, Δ). Since h is
random, "the least half-width" means its mean over y and Δ uniform on S^d and over ω. A lower bound on that mean bounds
the supremum over (y, Δ) of E_ω h; it says nothing about a single y, and a scheme may be narrow on a set of y of measure
up to M·Ω_d(θ) (referee 1(c)).

### Achievability

**Lemma 3.34 (F1 of the confidence track; cap measure).** On S^d ⊂ ℝ^(d+1), for 0 ≤ θ < π/2, the normalised measure of
a cap of angle θ is Ω_d(θ) = C_d·∫₀^θ sin^(d−1)t dt with C_d = Γ((d+1)/2)/(√π·Γ(d/2)), and

    C_d·sin^dθ/d ≤ Ω_d(θ) ≤ C_d·sin^dθ/(d·cos θ).

*Proof.* For the lower bound, sin^(d−1)t ≥ sin^(d−1)t·cos t, whose integral is sin^dθ/d. For the upper bound, the
derivative of sin^dt/(d·cos t) is sin^(d−1)t + sin^(d+1)t/(d·cos²t) ≥ sin^(d−1)t. ∎

[proved, refereed] CLAIMS.md C1.1; referee section 8. Since C_d/d = (2πd)^(−½)(1 + O(1/d)),
Ω_d(θ)^(1/d) = sin θ·(1 + O(ln d/d)) for θ bounded away from π/2.

**Proposition 3.35 (A1 of the confidence track; the scheme and its law).** Let ω = (R, 𝒞): R a Haar rotation of ℝ^(d+1)
and 𝒞 a codebook of M unit vectors independent of R (𝒞 may be fixed). Store the index of the codeword nearest R·y, and
let φ be its angle to R·y and x₀ = Rᵀ·(that codeword). For φ < π/2 put Ê = Δ·x₀/cos φ. Then Ê − Δ·y = tan φ·‖Δ_⊥‖·U,
with Δ_⊥ = Δ − (Δ·y)y and U one coordinate of a uniform unit vector in ℝ^d, independent of (φ, **c**, 𝒞). Hence, for a
rule that depends on (φ, Δ) only, h = ζ_d(δ/2)·tan φ·‖Δ‖ is two-sided δ-valid and h = ζ_d(δ)·tan φ·‖Δ‖ is one-sided
δ-valid; and the scheme "if ζ·tan φ ≥ 1, output ŝ = 0 and h = ‖Δ‖" is also valid, because |Δ·y| ≤ ‖Δ‖ always.

*Proof.* Lemma 3.1 with ‖y‖ = 1: the error vector is tan φ·u, with u uniform on the unit sphere of y^⊥ given R·y, hence
given (φ, **c**). So Δ·(error) = tan φ·‖Δ_⊥‖·(u·Δ_⊥/‖Δ_⊥‖). Given φ the failure probability is
Pr[|U| > ζ_d(δ/2)·‖Δ‖/‖Δ_⊥‖] ≤ δ, since ‖Δ_⊥‖ ≤ ‖Δ‖. The switch to (0, ‖Δ‖) is a function of φ. ∎

[proved, refereed] CLAIMS.md C1.1; referee section 8. This is the plain rule of §3.1 for one block.

**Proposition 3.36 (A2 of the confidence track; the scalar inside the bit budget).** Proposition 3.35 stores the real
number φ, which the model does not allow. Store instead ĉ = cos φ rounded down on a geometric grid of ratio 1 + η_g, and
use ŝ = Δ·x₀/ĉ and t̄ = √(1/ĉ² − 1). Then g = cos φ/ĉ ∈ [1, 1 + η_g], ŝ − Δ·y = (g − 1)·Δ·y + g·tan φ·‖Δ_⊥‖·U, and
h = [η_g + (1 + η_g)·ζ·t̄]·‖Δ‖ is valid (ζ = ζ_d(δ/2) two-sided or ζ_d(δ) one-sided), with
t̄ ≤ (1 + η_g)·tan φ + √(3η_g).

*Proof.* |(g − 1)Δ·y| ≤ η_g‖Δ‖. t̄ ≥ tan φ because ĉ ≤ cos φ, so g·tan φ·‖Δ_⊥‖·|U| ≤ (1 + η_g)·t̄·‖Δ‖·|U|, and the event
{|U| > ζ} has probability δ given φ. Finally t̄² ≤ (1 + η_g)²/cos²φ − 1 = (1 + η_g)²tan²φ + 2η_g + η_g². ∎

[proved, refereed] CLAIMS.md C1.1; referee section 8. With φ ≤ θ₁ < π/2 guaranteed, or the fallback of Proposition 3.35
outside it, the grid on [cos θ₁, 1] has ln(1/cos θ₁)/ln(1 + η_g) cells, so η_g = d⁻² costs O(log d) bits.

**Proposition 3.37 (A3 of the confidence track; the angle at rate ν).** Let 𝒞 be M independent uniform points of S^d.
For every fixed y and θ, Pr[φ > θ] = (1 − Ω_d(θ))^M ≤ exp(−M·Ω_d(θ)). So with θ₁ defined by M·Ω_d(θ₁) = 2 ln d,
Pr[φ > θ₁] ≤ d⁻², and by Lemma 3.34, sin θ₁ = 2^(−ν)·(1 + O(ln d/d)) for fixed ν, so
tan θ₁ = tan θ_ν·(1 + O(ln d/d)), with tan θ_ν = (4^ν − 1)^(−½). The constant in O depends on ν and grows as ν → 0.

*Proof.* The M angles to R·y are independent, and each is at most θ with probability Ω_d(θ). ∎

[proved, refereed] CLAIMS.md C1.1; referee section 8. A fixed covering code does the same without the d⁻² exception
(Böröczky and Wintsche give covering density at most d ln d + d ln ln d + 5d) [cited, unchecked: from the librarian's
summary]. Conversely, for any M points and y uniform, Pr[φ ≤ θ] ≤ M·Ω_d(θ), so no codebook has sin φ below
2^(−ν)(1 − a/d) on more than about e^(−a)/√(2πd) of the sphere: θ_ν is the least angle for schemes of Lemma 3.1's form.
[proved, unrefereed] CLAIMS.md C1.1, "The limit on tan φ", taken from `notes/program/codes/REPORT.md`; the referee's
verdicts do not name it.

**Theorem 3.38 (Theorem A of the confidence track; achievability).** For fixed ν > 0 and δ = δ_d with
ζ_d(δ_d)·√d ≥ 1, there is a scheme in the model, of B = νd + O(log d) bits and two-sided δ-valid, whose half-width
satisfies, for every (y, Δ) with ‖Δ‖ = 1, h ≤ ζ_d(δ/2)·tan θ_ν·(1 + O(ln d/d)) with probability at least 1 − d⁻² over ω,
and h ≤ 1 always. So E_ω h ≤ ζ_d(δ/2)·tan θ_ν·(1 + O(ln d/d)) + d⁻². The one-sided statement is the same with ζ_d(δ).

*Proof.* Propositions 3.35, 3.36 with η_g = d⁻², and 3.37. The additive terms η_g and ζ·√(3η_g) are O(1/d) relative to
ζ·tan θ₁, because ζ ≥ d^(−½) and tan θ₁ is a constant. ∎

[proved, refereed] CLAIMS.md C1.1; referee section 8, which did not check the constants inside the O(ln d/d) terms.
Theorem 3.38's half-width is the target ζ·(4^ν − 1)^(−½) the program started from. It is an upper bound, and the
half-width of Proposition 3.47 below is smaller by the factor √(1 + ξ²).

### The class 𝓛 and the converse

**The class 𝓛** (CLAIMS.md C1.3). The estimate is ŝ = Δ·ŷ(**c**, ω) for a reconstruction point ŷ(**c**, ω) ∈ ℝ^(d+1) of
any length and direction, and the half-width is h = h₁(**c**, ω)·‖Δ‖ with h₁ ≥ 0. The message, the seed and the code are
unrestricted. For fixed (y, ω) write e = ŷ − y, e_∥ = e·y = ŷ·y − 1, e_⊥ = ‖ŷ − (ŷ·y)y‖, ŷ = κ·x with x a unit vector and
κ ≥ 0, and φ = angle(x, y) ∈ [0, π]; if ŷ = 0, put φ := π, so e_∥ = −1 and e_⊥ = 0. Then e_∥ = κ cos φ − 1 and
e_⊥ = κ sin φ. The convention φ := π is the referee's repair 1(b): the first version's π/2 broke Lemma 3.42 at θ = π/2
with M = 1.

**What 𝓛 contains and what it leaves out** (CLAIMS.md C1.3, from referee section 4, accepted). In 𝓛: one block with the
plain rule, whose estimate f·⟨ō, RΔ⟩ is linear in Δ (the centre correction of Theorem 3.11 (i) cancels identically) and
whose half-width c·‖y‖·tan φ·‖Δ‖/√d is a multiple of ‖Δ‖, with stored scales rounded up inside h₁(**c**, ω); and the
interval of Proposition 3.46. Not in 𝓛:

1. the radial slack η·|Ê| required after Lemma 3.1, which depends on the direction of Δ;
2. any intersection with the deterministic interval |Δ·y| ≤ ‖Δ‖‖y‖ or with an earlier level's interval, which the
   search loops make;
3. R8 (Theorem 3.12, and Theorem 3.11 (ii) for one block), which uses the same message and is never wider than
   Proposition 3.46's interval;
4. several blocks, where the half-width is √(Σ_j h₁,j²‖Δ_j‖²) and not a multiple of ‖Δ‖.

So the index's certificate is in 𝓛 only for one block with the plain rule, and Theorem 3.43 says nothing about (2) to
(4). Every statement about w* below is a statement about 𝓛 and one block. 𝓛 is a fair class there, but it excludes the
project's own refereed interval R8, so w* is "the least mean half-width in 𝓛" and never "the least half-width".

**Lemma 3.39 (B0 of the confidence track; the algebra).** (i) For e_⊥ ≥ 0, the maximum over v ∈ [−1, 1] of
−v·e_∥ + ζ·√(1 − v²)·e_⊥ is √(e_∥² + ζ²e_⊥²). (ii) The minimum over κ ≥ 0 of (κ cos φ − 1)² + ζ²κ²sin²φ is W_ζ(φ)²,
attained at κ = cos φ/(cos²φ + ζ²sin²φ) for φ < π/2 and at κ = 0 for φ ≥ π/2.

*Proof.* (i) Cauchy–Schwarz for (−e_∥, ζe_⊥)·(v, √(1 − v²)), with equality attainable because e_⊥ ≥ 0. (ii) For
cos φ ≤ 0 the first term is at least 1, which is W_ζ(φ)² there. Otherwise the quadratic in κ has its minimum where
cos φ·(1 − κ cos φ) = ζ²κ sin²φ, and its value there is ζ²sin²φ·(cos²φ + ζ²sin²φ)/(cos²φ + ζ²sin²φ)², which is
W_ζ(φ)². ∎

[proved, refereed] CLAIMS.md C1.3; referee section 4 ("both parts checked by hand, including cos φ ≤ 0"). W_ζ(φ)
increases in φ and in ζ.

**Lemma 3.40 (B1 of the confidence track; validity for one y and one family of queries).** Let the scheme be in 𝓛 and
one-sided δ-valid. Fix y and v ∈ [−1, 1]. Then

    E_ω T_d( (h₁ + v·e_∥) / (√(1 − v²)·e_⊥) ) ≤ δ,

with the argument +∞ or −∞ by the sign of h₁ + v·e_∥ when √(1 − v²)·e_⊥ = 0, and T_d(0/0) := 0.

*Proof.* Let u′ be uniform on the unit sphere of y^⊥, independent of ω, and Δ = v·y + √(1 − v²)·u′. Each such Δ is a fixed
unit query, so Pr_ω[Δ·y − ŝ > h] ≤ δ for each u′; average over u′. Now Δ·y − ŝ = −Δ·e = −v·e_∥ − √(1 − v²)·(e·u′), and
e·u′ = e_⊥·U with U a coordinate of a uniform unit vector of ℝ^d, independent of ω. U is symmetric, so given ω the
failure probability is T_d((h₁ + v·e_∥)/(√(1 − v²)·e_⊥)). ∎

[proved, refereed] CLAIMS.md C1.3; referee section 4, which confirmed that averaging over u′ is legitimate because each
Δ(u′) is a fixed query and u′ is independent of ω.

**Lemma 3.41 (B2 of the confidence track; a grid of latitudes and the steep tail).** Let v_i = sin(−π/2 + iπ/(m − 1)),
i = 0, …, m − 1, m ≥ 2, and ψ_m = π/(2(m − 1)). Fix ζ₀ ∈ (0, 1). For every y,

    Pr_ω[ h₁ < cos ψ_m·√(e_∥² + ζ₀²e_⊥²) ] ≤ m·δ/T_d(ζ₀).

*Proof.* Write (v, √(1 − v²)) = (sin ψ, cos ψ) with ψ ∈ [−π/2, π/2]. Then −v·e_∥ + ζ₀√(1 − v²)·e_⊥ =
√(e_∥² + ζ₀²e_⊥²)·cos(ψ − ψ*) for some ψ* ∈ [−π/2, π/2], the maximiser of Lemma 3.39 (i). A grid angle ψ_i lies within
ψ_m of ψ*, so −v_i·e_∥ + ζ₀√(1 − v_i²)·e_⊥ ≥ cos ψ_m·√(e_∥² + ζ₀²e_⊥²). If h₁ is below that, then
(h₁ + v_i·e_∥)/(√(1 − v_i²)·e_⊥) < ζ₀, and the i-th failure probability of Lemma 3.40 is at least T_d(ζ₀) (it is 1 when
√(1 − v_i²)·e_⊥ = 0). By Lemma 3.40 and Markov's inequality, Pr_ω[the i-th failure probability ≥ T_d(ζ₀)] ≤ δ/T_d(ζ₀).
The grid index depends on ω through ψ*, so take the union over all m indices. ∎

[proved, refereed] CLAIMS.md C1.3; referee section 4. T_d(ζ₀)/δ is the factor by which the failure probability rises
when the half-width falls from ζ_d(δ) to ζ₀ standard units; it is exponential in d·(ζ_d(δ)² − ζ₀²)/2. At δ = 2.5·10⁻¹⁹,
cutting the half-width to 0.95 of the quantile multiplies the failure probability by 334 at d = 95 and 92 at d = 255
[computed, `c1_converse.py` part 2].

**Lemma 3.42 (B3 of the confidence track; the cap bound).** For every fixed ω and θ ≤ π/2,
σ{y : φ(y, ω) ≤ θ} ≤ M·Ω_d(θ), with σ the uniform probability on S^d.

*Proof.* φ(y, ω) ≤ θ puts y in the cap of angle θ about the direction of ŷ(**c**, ω) for **c** = enc(y, ω). There are at
most M such directions, and a message with ŷ = 0 has φ = π and contributes none. ∎

[proved, unrefereed] CLAIMS.md C1.3. The referee found the first version correct after its repair 1(b), the convention
φ := π for ŷ = 0, which the revision adopted; the repaired text has not been re-read.

**Theorem 3.43 (Theorem B of the confidence track; the converse in 𝓛, every d and δ).** For every one-sided δ-valid
scheme in 𝓛 with M = 2^(νd) messages, every m ≥ 2, ζ₀ ∈ (0, 1) and θ ∈ (0, π/2],

    σ⊗μ{ (y, ω) : h₁ ≥ cos ψ_m·W_{ζ₀}(θ) } ≥ 1 − m·δ/T_d(ζ₀) − M·Ω_d(θ),

and so E_{y,ω} h₁ ≥ cos ψ_m·W_{ζ₀}(θ)·(1 − m·δ/T_d(ζ₀) − M·Ω_d(θ)). A two-sided δ-valid scheme is one-sided δ-valid, so
the same holds for it.

*Proof.* By Lemma 3.39 (ii), √(e_∥² + ζ₀²e_⊥²) ≥ W_{ζ₀}(φ), which is at least W_{ζ₀}(θ) when φ ≥ θ. Integrate Lemma 3.41
over y, and remove the set of Lemma 3.42. ∎

[proved, refereed] CLAIMS.md C1.3; referee section 4 ("no error found; hypotheses used and not stated: none beyond 𝓛
and one-sided validity"). Its proof uses Lemma 3.42 with the repaired convention, which the referee says leaves the
theorem unaffected. It bounds the mean over y uniform and ω, in 𝓛, for one block.

**Corollary 3.44 (B4 of the confidence track; first order).** Let ν > 0 be fixed, d → ∞, and δ = δ_d → 0 with
ln(1/δ_d)/ln d → ∞. Then for one-sided δ-valid schemes in 𝓛,

    E_{y,ω} h₁ ≥ W_ζ(θ_ν)·(1 − o(1)) = ζ/√(4^ν − 1 + ζ²)·(1 − o(1)),   ζ = ζ_d(δ).

*Proof.* In Theorem 3.43 take m = d, T_d(ζ₀) = d²·δ and M·Ω_d(θ) = 1/d. Then cos ψ_m → 1, the exceptional mass is 2/d,
and sin θ → 2^(−ν) by Lemma 3.34. It remains to show W_{ζ₀}(θ)/W_ζ(θ) → 1. For 0 < x < 1,
T_d(x) = C′_d·∫ₓ¹(1 − t²)^((d−3)/2)dt with C′_d = Γ(d/2)/(√π·Γ((d − 1)/2)). Multiplying the integrand by t ≤ 1, or by
t/x ≥ 1, gives (1 − x²)^((d−1)/2)/(d − 1) ≤ ∫ ≤ (1 − x²)^((d−1)/2)/(x(d − 1)). With L(x) = −ln(1 − x²) this is
ln(1/T_d(x)) = ((d − 1)/2)·L(x) + ½·ln d + O(1) − r(x), with 0 ≤ r(x) ≤ ln(1/x). At ζ = ζ_d(δ) and ζ₀ = ζ_d(d²δ),
((d − 1)/2)·(L(ζ) − L(ζ₀)) ≤ 2 ln d + ln(1/ζ), and ((d − 1)/2)·L(ζ) ≥ ln(1/δ) − ½ ln d − O(1). Once δ is below a constant,
ζ ≥ c₀/√d for a constant c₀, so ln(1/ζ) = O(ln d), and 1 − L(ζ₀)/L(ζ) = O(ln d/ln(1/δ)) → 0. Since x = √(1 − e^(−L)) has
d ln x/d ln L = L/(2(e^L − 1)) ≤ ½, ln(ζ/ζ₀) ≤ ½·ln(L(ζ)/L(ζ₀)) → 0, for every size of ζ including ζ → 1. And
∂ ln W_ζ(θ)/∂ ln ζ = cos²θ/(cos²θ + ζ²sin²θ) ≤ 1, so W_{ζ₀}(θ)/W_ζ(θ) ≥ ζ₀/ζ → 1. ∎

[proved, unrefereed] CLAIMS.md C1.3. The statement is unchanged from the version the referee read; the quantile step,
which the first version checked only numerically, now has the referee's proof (section 4), and the pair has not been
re-read. Corollary 3.44 is a limit statement. At the index's budget, δ = 2.5·10⁻¹⁹, ln(1/δ)/ln d is 9.4 at d = 95 and 7.7
at d = 255, where ζ_d(d²δ)/ζ_d(δ) ≈ √(1 − 2 ln d/ln(1/δ)) and the corollary's choices are off by 8 to 15% (the referee's
figures). Numbers at the index's sizes therefore come from Corollary 3.45.

**Corollary 3.45 (B6 of the confidence track; Theorem 3.43 integrated).** For ϑ ∈ (0, 1) and u ∈ (0, 1], let
ζ(u) = ζ_d(mδ/(ϑu)), or 0 if mδ/(ϑu) ≥ ½, and let θ(u) solve M·Ω_d(θ(u)) = (1 − ϑ)u. Then

    E_{y,ω} h₁ ≥ cos ψ_m·∫₀¹ W_{ζ(u)}(θ(u)) du.

*Proof.* By Theorem 3.43 with ζ₀ = ζ(u) and θ = θ(u), the event h₁ < cos ψ_m·W_{ζ(u)}(θ(u)) has probability at most
ϑu + (1 − ϑ)u = u (when ζ(u) = 0 the event is empty). The right side decreases as u decreases, so
u ↦ cos ψ_m·W_{ζ(u)}(θ(u)) lies below the quantile function of h₁, whose integral is E h₁. ∎

[proved, refereed] CLAIMS.md C1.3; referee section 4 ("the integrand is non-decreasing and lies below the upper quantile
function"). ϑ is the share of the exceptional mass given to the tail; the track writes ρ.

### Meeting the bound in 𝓛

**Proposition 3.46 (A4 of the confidence track; R8 with its half-width frozen at Ê = 0).** In the scheme of Proposition
3.35 (a random or covering code, a Haar rotation, the code a function of R·y alone and φ the true angle, as Lemma 3.1
requires), with φ < π/2 stored as a real number, take

    ŷ = κ*·x₀,  κ* = cos φ/(cos²φ + ζ²sin²φ) = 1/(cos φ·(1 + ξ²)),  h₁ = W_ζ(φ) = ζ·tan φ/√(1 + ξ²),  ξ² = ζ²tan²φ,

and for φ ≥ π/2 take ŷ = 0 and h₁ = 1. With ζ = ζ_d(δ) the scheme is one-sided δ-valid for every (y, Δ). Conditionally
on φ its failure probability equals δ at v = Δ·y/‖Δ‖ = +ξ/√(1 + ξ²) and is smaller elsewhere; because that v depends on
φ, for a fixed query and a code whose φ varies the unconditional failure probability is below δ at every v. With
ζ = ζ_d(δ/2) it is two-sided δ-valid.

*Proof.* As in Lemma 3.1's proof, x₀ = cos φ·y + sin φ·u″ with u″ uniform on the unit sphere of y^⊥ and independent of φ.
So e = e_∥·y + e_⊥·u″ with e_∥ = κ* cos φ − 1 = −ξ²/(1 + ξ²) and e_⊥ = κ* sin φ = tan φ/(1 + ξ²). For a unit
Δ = v·y + √(1 − v²)·u′ with u′ fixed, Δ·e = v·e_∥ + √(1 − v²)·e_⊥·U. The one-sided failure probability given φ is
T_d((h₁ + v·e_∥)/(√(1 − v²)·e_⊥)), which is at most T_d(ζ) because h₁² = e_∥² + ζ²e_⊥² = ξ²/(1 + ξ²) and so
h₁ ≥ −v·e_∥ + ζ√(1 − v²)·e_⊥ by Lemma 3.39 (i), with equality at v = ξ/√(1 + ξ²). Two-sided: the lower tail is the mirror
image, v → −v. ∎

[proved, unrefereed] CLAIMS.md C1.3, restated after the referee, whose verdict on the first wording was "no error found
in the statement for real φ" (section 2) and who supplied the conditional wording and the case φ ≥ π/2; the restatement
has not been re-read. The theorist's runs gave 0.0090 to 0.0093 against δ = 0.01 at the worst v. The referee's
simulation over seven codes (the sign code at n = 8, a 2-bit scalar grid at n = 13, a ternary code at n = 5, random
codebooks of 512, 16, 2 and 1 words) and 63 cells never put the one-sided rate above δ (largest 0.0094 at δ = 0.01 and
0.0192 at δ = 0.02), and in every cell the rate matched the proof's conditional law within two standard errors
[measured, simulation, `ref_sim_a4.py`].

*What Proposition 3.46 is and is not* (referee section 2, accepted in full).

1. It is not a new interval. For one block R8 has centre Ê/(1 + ξ²) and half-width ζ·tan φ·√(1 + ξ² − Ê²)/(1 + ξ²), with
   Ê = Δ·x₀/cos φ the unbiased estimate (Theorem 3.11 (ii) with b = 1, or Theorem 3.12). Proposition 3.46 has the same
   centre, since Δ·ŷ = Ê/(1 + ξ²), and the same half-width with Ê² dropped under the root. So its interval contains
   R8's for every outcome: it is R8 widened to its half-width at Ê = 0, which is the one change that puts it in 𝓛. §3.5
   already records that R8 at Ê = 0 is the plain width divided by √(1 + ξ²). What is new is only that this member of 𝓛
   meets Theorem 3.43.
2. It does not dominate the plain rule one-sidedly. "Narrower than the plain rule by √(1 + ξ²)" is a statement about
   half-widths, not about the bound an exclusion uses. Its upper end Ê/(1 + ξ²) + ξ/√(1 + ξ²) is above the plain upper
   end Ê + ξ exactly when Ê < −ξ·√(1 + ξ²)/(1 + √(1 + ξ²)), about −½·ζ·tan φ. In the referee's runs that held in 13 to
   22% of draws at v = 0, in 50 to 98% at v = −0.3 for codes with ξ² below 0.4 (15 to 23% for the codebooks of 16 words
   or fewer), and in over 99% at v = −0.9 [measured, simulation]. So neither end should be used alone; Proposition 3.48
   shows that their minimum costs no second event.

**Proposition 3.47 (A4′ of the confidence track; Proposition 3.46 inside the bit budget).** Proposition 3.46 stores the
real number φ, and rounding φ up is not safe by itself: the bias e_∥ moves by (cos φ/cos φ̂ − 1)/(1 + ξ̂²), which W_ζ(φ̂)
does not cover. Instead store ĉ ≤ cos φ on a geometric grid of ratio 1 + η_g, so that g = cos φ/ĉ ∈ [1, 1 + η_g]; put
φ̂ = arccos ĉ ≥ φ, ξ̂² = ζ²tan²φ̂, κ̂ = 1/(ĉ·(1 + ξ̂²)), ŷ = κ̂·x₀ and h₁ = W_ζ(φ̂) + η_g. This is one-sided δ-valid at
ζ = ζ_d(δ).

*Proof.* e_∥ = κ̂ cos φ − 1 = (κ̂ĉ − 1) + (g − 1)·κ̂ĉ with κ̂ĉ = 1/(1 + ξ̂²) ≤ 1, so e_∥ = ê_∥ + ϵ_g with
ê_∥ = −ξ̂²/(1 + ξ̂²) and 0 ≤ ϵ_g ≤ η_g. And e_⊥ = κ̂ sin φ ≤ κ̂ sin φ̂ = ê_⊥. For every v,
−v·e_∥ + ζ√(1 − v²)·e_⊥ ≤ −v·ê_∥ + ζ√(1 − v²)·ê_⊥ + η_g ≤ √(ê_∥² + ζ²ê_⊥²) + η_g = W_ζ(φ̂) + η_g by Lemma 3.39 (i). So the
argument of T_d in the proof of Proposition 3.46 is at least ζ. ∎

[proved, unrefereed] CLAIMS.md C1.3. The repair and its proof sketch are the referee's (section 1(a)); the theorist wrote
the proof out, and no one else has read it. With η_g = d⁻² the grid costs O(log d) bits, as in Proposition 3.36, and
W_ζ(φ̂) ≤ W_ζ(φ)·(1 + O(d⁻¹)) on the event of Proposition 3.37. So the bound of Theorem 3.43 is met at B = νd + O(log d)
bits by Proposition 3.47, not by Proposition 3.46 as first written.

**Proposition 3.48 (A5 of the confidence track; one event serves the plain rule, Proposition 3.46 and capped R8).** Take
units ‖y‖ = ‖Δ‖ = 1, one block and real φ < π/2, and let 𝓔₁ = {v − Ê ≤ ξ·√(1 − v²)}, which is the event {−U ≤ ζ} of
probability 1 − δ given φ. On 𝓔₁ all three of the following hold:

- plain: v ≤ Ê + ξ;
- Proposition 3.46: v ≤ Ê/(1 + ξ²) + ξ/√(1 + ξ²);
- R8 with Theorem 3.11's cap: v ≤ 1 if Ê ≥ 1 or 1 + ξ² − Ê² < 0, and otherwise
  v ≤ min(1, (Ê + ξ·√(1 + ξ² − Ê²))/(1 + ξ²)).

So the smallest of the three upper ends is a one-sided bound that costs one event, not three.

*Proof.* By Proposition 3.35, v − Ê = −tan φ·√(1 − v²)·U, so 𝓔₁ is {−U ≤ ζ}. Plain: √(1 − v²) ≤ 1. Proposition 3.46:
v·(1 + ξ²) − Ê = (v − Ê) + ξ²·v ≤ ξ·(√(1 − v²) + ξ·v) ≤ ξ·√(1 + ξ²) by Cauchy–Schwarz. R8: Theorem 3.11's repaired
statement of the upper end, with b̄ = 1 and Ē = Ê, which is proved on this same event. ∎

[proved, unrefereed] CLAIMS.md C1.3; the statement and proof are the referee's (section 2), not re-read by anyone.
Theorem 3.11's repaired statement is written on the two-sided event |U| ≤ c/√d, while the proof uses it on the
one-sided event 𝓔₁ (Editor's query 20). In
the theorist's simulation (d = 12, 512 random codewords, δ = 0.01, 400,000 draws per row) the smallest of the three ends
was exceeded at rates 0.0097 to 0.0102 at v = 0, ±0.3, 0.6, 0.9 and 0.99, and 0.0001 at v = −0.9 [measured, simulation,
`c1_revision.log` part 4]. The remark after Theorem 3.12 that R8 and the plain interval have different events, so that
their minimum costs a second event, concerns several blocks, where the shares are unknown; for one block Proposition
3.48 shows a single event serves both (see Editor's query 13).

The cap is needed. Without it R8's upper end, used one-sidedly at ζ_d(δ), fails with probability rising to 2δ for aligned
queries: 0.0168 at v = 0.9, 0.0197 at 0.99 and 0.0200 at 0.999 (n = 13, M = 512, δ = 0.01), and 0.0394 at v = 0.99
(n = 5, δ = 0.02); with the cap it is at most δ in every cell [measured, simulation, referee section 2; reproduced by the
theorist]. Its two-sided validity held in every cell.

**Lemma 3.49 (B5 of the confidence track; one side against two).** (i) A two-sided δ-valid scheme in 𝓛 satisfies Theorem
3.43 with δ, and at v = 0 the stronger E_ω T_d(h₁/e_⊥) ≤ δ/2, because there both tails are equal. (ii) Proposition 3.46
with ζ_d(δ/2) is two-sided δ-valid. So the two-sided optimum in 𝓛 lies between w* at ζ_d(δ) and w* at ζ_d(δ/2). (iii) A
one-sided scheme run on Δ and on −Δ is a two-sided scheme at 2δ, since in 𝓛 the estimate is odd and the half-width even
in Δ. So one side costs at most one halving of δ.

*Proof.* (i) At v = 0 the error is e_⊥·U, which is symmetric. (ii) Proposition 3.46. (iii) Linearity. ∎

[proved, unrefereed] CLAIMS.md C1.3; (iii) was corrected after the referee (section 4), who found "exactly one halving"
wrong for ξ² not small. The two tails of Proposition 3.46 peak at v = ±ξ/√(1 + ξ²), and at either peak the other tail is
negligible: the largest sum of the two tails over v, in units of T_d(ζ), is 1.0000 at d = 95 (ξ² = 0.19 and 1.38) and at
d = 255, ξ² = 0.09; 1.035 at d = 255, ξ² = 0.018; and 1.91 at d = 255, ξ² = 0.001 (the referee computed 1.94)
[computed, `c1_revision.log` part 3; `ref_num1.py`]. So for ξ² ≥ 0.09, Proposition 3.46 at ζ_d(δ) is already two-sided
δ-valid to four digits, and the one-sided and two-sided optima in 𝓛 coincide. The effect is never more than
ζ_d(δ/2)/ζ_d(δ) − 1 ≈ ln 2/(d·ζ²), which the track and the referee put at 0.5% at d = 255 (Editor's query 19).

**The optimum in 𝓛.** Theorem 3.43 with Corollary 3.44, and Propositions 3.47 and 3.37, all above, give: among schemes
in 𝓛, for one block, the least mean half-width at rate ν is, to first order in Corollary 3.44's regime,

    w*(ν, δ, d) = ζ/√(4^ν − 1 + ζ²) = ζ·tan θ_ν/√(1 + ζ²tan²θ_ν),   ζ = ζ_d(δ) (one-sided).

[proved, unrefereed: it combines refereed results with Proposition 3.47 and Corollary 3.44, which are not re-read]
w* is Theorem 3.38's target ζ·(4^ν − 1)^(−½) divided by √(1 + ξ²), with ξ² = ζ²tan²θ_ν; the target is the limit ξ → 0.
The plain rule is valid and wider than w* by √(1 + ξ²) at the ideal angle. The bound is forced by queries with latitude
v up to ξ/√(1 + ξ²) (0.13 at ξ² = 0.018, 0.29 at 0.09); a typical offset has |v| of order 1/√d, where R8's gain over
Proposition 3.46 is O(1/d), but the guarantee is for every query.

*Why tan φ and not sin φ* (the track's N2; [proved, unrefereed]; the exact form is the referee's, section 3). For any code
of Lemma 3.1's form, if Δ is uniform on S^d and independent of (y, ω), the shrunk estimate cos φ·(Δ·x₀) has error
Δ·(y − cos φ·x₀), and ‖y − cos φ·x₀‖ = sin φ exactly. So the error is sin φ·U′ with U′ a coordinate of a uniform unit
vector in ℝ^(d+1), independent of φ, and the half-width ζ_{d+1}(δ)·sin φ·‖Δ‖ fails with probability exactly δ on average
over Δ. A converse that uses only the requirement averaged over independent (y, Δ) therefore proves at most
ζ_{d+1}(δ)·E sin φ, about ζ·2^(−ν). That rule is not valid for fixed queries: at δ = 0.01 it failed in 1.8% of draws at
v = 0.3, 12% at 0.6, 48% at 0.9 and 78% at 0.99 [measured, simulation, `c1_revision.py` part 4; the referee found
1.3%, 9.4%, 40% and 73% at 0.3, 0.6, 0.9, 0.999]. The tan comes from queries correlated with the item, which Lemma 3.40
uses. The ratio of the two forms is cos θ_ν = √(1 − 4^(−ν)): 0.968 at ν = 2, 0.866 at ν = 1, 0.541 at ν = ¼. The track
reads this as the difference between TurboQuant's 4^(−b) and 1/(4^b − 1) [cited, unchecked: from the librarian's
summary].

### Numbers at the index's block sizes

[computed, `notes/program/confidence/c1_converse.py` part 3 and `c1_revision.py` part 1; rows (95, 1), (255, ¼) and
(95, ¼) reproduced by the referee's `ref_num1.py`.] One-sided, δ = 10⁻⁹/(10⁹·4) = 2.5·10⁻¹⁹, one block, in 𝓛. "Plain" is
ζ·tan θ_ν at the ideal angle; "mean" is the exact mean of W_ζ(φ) for Proposition 3.46 with a random code,
Pr[φ > θ] = (1 − Ω_d(θ))^M, which is the proved upper bound for real φ (Proposition 3.47's grid adds O(1/d)); "B6" is
Corollary 3.45's proved lower bound as a left sum of the non-decreasing integrand on 400 cells, maximised over
m ∈ {5, 9, 17, 33} and ϑ ∈ {0.5, 0.8, 0.95}.

| d | ν | ζ_d(δ) | ξ² = ζ²tan²θ_ν | plain | w* | mean (upper) | B6 (lower) | B6/mean | B6/w* |
|---|---|---|---|---|---|---|---|---|---|
| 95 | ¼ | 0.7565 | 1.381 | 1.1754 | 0.7616 | 0.7852 | 0.7550 | 0.962 | 0.991 |
| 95 | 1 | 0.7565 | 0.191 | 0.4367 | 0.4002 | 0.4122 | 0.3908 | 0.948 | 0.976 |
| 95 | 2 | 0.7565 | 0.038 | 0.1953 | 0.1917 | 0.1972 | 0.1863 | 0.945 | 0.972 |
| 95 | 7 | 0.7565 | 0.000 | 0.0059 | 0.0059 | 0.0061 | 0.0057 | 0.944 | 0.970 |
| 255 | ¼ | 0.5186 | 0.649 | 0.8058 | 0.6275 | 0.6405 | 0.6106 | 0.953 | 0.973 |
| 255 | 1 | 0.5186 | 0.090 | 0.2994 | 0.2868 | 0.2910 | 0.2743 | 0.943 | 0.956 |
| 255 | 2 | 0.5186 | 0.018 | 0.1339 | 0.1327 | 0.1344 | 0.1265 | 0.941 | 0.953 |
| 255 | 7 | 0.5186 | 0.000 | 0.0041 | 0.0041 | 0.0041 | 0.0039 | 0.941 | 0.952 |

ζ_d(δ)·√d is 7.37 at d = 95 and 8.28 at d = 255, the one-block constants of the index's budget. At these sizes the proved
lower bound in 𝓛 is 4 to 6% below the proved upper bound, and 1 to 5% below the first-order w*. The rest is the finite-d
cost of the union over the grid of latitudes (2 to 3%) and of the integral over the steep tail (about 1%). The plain
half-width exceeds w* by √(1 + ξ²): 54% at ¼ bit and 9% at 1 bit on the block of 96 positions, 28% and 4% on the block of
256, and under 2% from 2 bits up. These compare half-widths of one block alone at the ideal angle, inside 𝓛. They are not
savings for the index, whose search intersects intervals and uses several blocks, both outside 𝓛, and whose built code
has a tan φ 1.3 to 1.8 times the ideal (`notes/program/codes/REPORT.md`), which makes ξ² larger. Where ζ·tan θ_ν > 1
(¼ bit) the plain half-width exceeds the deterministic bound |Δ·y| ≤ ‖Δ‖‖y‖, an interval cut there is outside 𝓛, and w*
is 0.76 and 0.63 of the deterministic bound at d = 95 and 255; so the low-rate rows say least about the index.

**Simulation** [measured, simulation, `c1_revision.py` part 4: d = 12, 512 random codewords, δ = 0.01, 400,000 draws per
row, standard error 0.00016]. One-sided rates of "truth above the upper end". "Uniform" is Δ uniform and independent of
y; the other rows are fixed queries at latitude v. "sin" is the rule of the averaged requirement above.

| v | plain | sin | Proposition 3.46 | R8, no cap | R8 with cap | smallest of three | share with the 3.46 end above the plain end |
|---|---|---|---|---|---|---|---|
| uniform | 0.0074 | 0.0098 | 0.0031 | 0.0098 | 0.0097 | 0.0097 | 22% |
| 0 | 0.0102 | 0.0006 | 0.0024 | 0.0102 | 0.0102 | 0.0102 | 13% |
| 0.3 | 0.0064 | 0.0182 | 0.0075 | 0.0100 | 0.0100 | 0.0100 | 0.4% |
| 0.6 | 0.0005 | 0.1198 | 0.0090 | 0.0100 | 0.0100 | 0.0100 | 0% |
| 0.9 | 0.0000 | 0.4825 | 0.0000 | 0.0169 | 0.0099 | 0.0099 | 0% |
| 0.99 | 0.0000 | 0.7802 | 0.0000 | 0.0199 | 0.0098 | 0.0098 | 0% |
| −0.3 | 0.0068 | 0.0000 | 0.0001 | 0.0102 | 0.0102 | 0.0102 | 50% |
| −0.9 | 0.0000 | 0.0000 | 0.0000 | 0.0031 | 0.0001 | 0.0001 | 100% |

Every simulation, the referee's included, is at n ≤ 25 and δ ≥ 0.001; figures at δ = 2.5·10⁻¹⁹ are computed from the
formulae.

### The confidence tax

**Corollary 3.50 (the confidence tax of the confidence track).** In 𝓛, for one block, one-sided and to first order in
Corollary 3.44's regime, a δ-certificate of mean half-width w (in units of ‖y‖‖Δ‖) needs

    R_cert(w) = ½·log₂(1 + ζ²·(1 − w²)/w²)

bits per degree of freedom, ζ = ζ_d(δ), while making the unbiased estimate's standard deviation tan θ_ν/√d equal to w
needs R_sd(w) = ½·log₂(1 + 1/(d·w²)). The tax is

    R_cert − R_sd = ½·log₂[ (w² + ζ²(1 − w²)) / (w² + 1/d) ],

whose limit as w → 0 is log₂(ζ·√d) = log₂ c bits per degree of freedom, with c the one-block constant; the limit needs
w ≪ 1/√d.

*Proof.* Solve w = ζ/√(4^ν − 1 + ζ²) for ν, which gives 4^ν = 1 + ζ²(1 − w²)/w², and tan θ_ν/√d = w, which gives
4^ν = 1 + 1/(d·w²); subtract the logarithms. As w → 0 the ratio tends to ζ²·d. ∎

[proved, unrefereed, given Proposition 3.47 and Theorem 3.43] CLAIMS.md C1.3. The referee found the algebra correct and
the condition misstated: the first version said w ≪ ζ, and the limit needs w ≪ 1/√d (section 5). Figures [computed,
`c1_revision.log` part 2; the referee reproduced 2.16, 2.90 and 3.03 at d = 255, the three limits, and 0.92]:

| d | c = ζ√d | 1/√d | limit (bits) | w = 0.3 | w = 0.1 | w = 0.03 | w = 0.01 |
|---|---|---|---|---|---|---|---|
| 95 | 7.37 | 0.103 | 2.88 | 1.30 | 2.41 | 2.82 | 2.88 |
| 255 | 8.28 | 0.063 | 3.05 | 0.92 | 2.16 | 2.90 | 3.03 |
| 767 | 8.69 | 0.036 | 3.12 | 0.49 | 1.63 | 2.75 | 3.07 |

At the limit a block of 256 positions pays about 97 bytes for confidence. The tax says that an interval of c standard
deviations needs a standard deviation c times smaller, at 6 dB per bit, with the exact form of the saturation at coarse
widths; nothing deeper is proved by it. At the index's sizes the proved bounds bracket w* within about ±5%, which is
±0.07 bit per degree of freedom. It is not a statement about what any certificate must cost, because outside 𝓛 nothing is
proved.

### Gaps

- **G1 (the full model)** [sketch: gap named]. Outside 𝓛 the half-width may depend on the direction of Δ and the
  estimate need not be linear in Δ; R8 is such a scheme, and no lower bound of any kind, not even ζ·2^(−ν), is proved
  there. What is proved: for fixed (ω, **c**) with cell K = enc(·, ω)⁻¹(**c**), 2h is at least the width, in direction Δ,
  of the part of K on which the scheme does not fail. What is missing is a "trimmed Urysohn inequality": among sets of
  given measure, the cap minimises the mean over directions of the width that holds all but a δ-fraction of the mass.
  The referee's remark (section 7): for a scheme that starts with a Haar rotation, "for every y" is, in the rotated
  frame, an average over R·y uniform, so the validity requirement is exactly the family of Bayes problems "Δ′ uniform,
  R·y uniform on the latitude v of Δ′", one for each v. The track conjectures that the least mean half-width in the full
  model is that of R8, ζ·tan θ_ν·√(1 + ξ² − v²)/(1 + ξ²) at latitude v, which equals w* at v = 0 [conjecture]. Known
  meanwhile: R8 has a half-width below 𝓛's bound by the factor √(1 − Ê²/(1 + ξ²)), through its dependence on the estimate.
- **G2 (δ fixed as d → ∞)** [sketch: gap named]. Corollary 3.44 needs ln(1/δ) ≫ ln d. For δ fixed, Theorem 3.43 with
  m = 9 and T_d(ζ₀) = 270·δ (which needs δ < 1/540) gives a bound a constant factor below w*, and the first-order constant
  is not proved. At v = 0, E_ω T_d(h₁/e_⊥) ≤ δ and T_d is convex on [0, ∞) for d ≥ 3, so E_ω[h₁/e_⊥] ≥ ζ_d(δ) for every y
  by Jensen; what resists is a lower bound e_⊥ ≥ tan φ·(1 − o(1)) outside an event small enough not to move the quantile.
  The index is not in this regime.
- **G3 (the built code)** [sketch: gap named]. Theorem 3.33 is for a random shared codebook. For the index's scalar code
  there is no bound on Pr[χ > χ₀ | F_1] at the level 10⁻¹¹ needed by Lemma 3.32. The measured correlation of two items'
  normalised errors under a one-bit sign code (0.00 at residual cosine 0, 0.02 at 0.3) and the referee's run in which the
  sign code at n = 13 behaved like a 512-word random code (Pr[F_2 | F_1] = 0.0096 against p̄ = 0.0099) describe the bulk
  and say nothing about a tail of 10⁻¹¹.

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
  *21 November 2026:* settled as far as the sources go. R9 as worded is [refuted] (§3.6a); its true form is Corollary
  3.30; the i.i.d. latitude corpus of Theorem 3.29 is the construction, and Theorem 3.33 shows its overlap negligible for
  one block, one level, the plain rule and a random shared codebook. What remains open is listed in the next five items.
- **The factor N·L in Theorem 3.33** [sketch: gap named]. The pair overlap of two items at two different levels, whose
  codes are nested refinements.
- **Theorem 3.33 for other rules and several blocks** [sketch: gap named]. Lemma 3.31's terms relative to the rule's own
  p̄ (for Proposition 3.46 at its worst latitude, not the equator); a several-block Lemma 3.31, where partial sharing of
  codewords gives χ < 1 and Lemma 3.32 applies.
- **G3: the built code's pair tail** [sketch: gap named]. Pr[χ > χ₀ | F_1] at 10⁻¹¹ for the index's scalar code.
- **Where the union starts to be attained for small codes.** Between one codeword (Claim E, not attained) and the
  sufficient sizes of Theorem 3.33 (30 to 41 bits); the measured excess depends on d and δ, not on M/N alone.
- **k-NN under Corollary 3.30 and Theorem 3.33.** Neither covers a decision that depends on other items' intervals.
- **G1: a lower bound outside 𝓛** [sketch: gap named; conjecture]. A trimmed Urysohn inequality; the conjectured optimum
  is R8's half-width ζ·tan θ_ν·√(1 + ξ² − v²)/(1 + ξ²).
- **G2: the first-order constant for δ fixed as d → ∞** [sketch: gap named].
- **A lower bound for the index's certificate as built.** Several blocks, intersections across levels and with the
  deterministic bound, and the radial slack are all outside 𝓛; the factor L over levels is a sketch.
- **An f32 build** (§3.2a) [sketch: gap named]. Encoding against a rounded matrix costs a factor E[2^Q] on δ (about
  e^0.35 at depth 8 and n_j = 768 by the referee's estimate), plus a radial slack, a larger margin on tan φ and a count of
  near-ties between the six scales, which was not worked out. The present rule (encode against f64) avoids it.
- **The existing `ewj` term when ‖x_j‖ ≫ ‖Δ_j‖.** The rotations referee observed, without checking, that today's
  allowance for the f32 cast has the shape of §3.2a's repair 1 without the factor (1 + dk_j), and sits inside the
  kernel's 10⁻⁶ only while ‖x_j‖ is within about ten times ‖Δ_j‖. It belongs to chapter 1's deterministic terms.
- **A drawn seed.** As built the seed is a public constant (§3.4), so no probability over ω exists outside the model.
  Whether to draw it from the system's entropy at build is a design question, not a mathematical one.
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
    *21 November 2026:* the confidence track's referee found the same independently by simulation: R8's upper end used
    one-sidedly without the cap fails at up to 2δ for aligned queries, and at most δ with it (§3.7a, after Proposition
    3.48). This supports the cap as a condition; it does not re-tag bounds2's statement, which stays open for the PI.
13. Theorem 3.12's remark says R8's event differs from the plain interval's, so taking the smaller of the two costs a
    second event. Proposition 3.48 shows that for one block one event serves the plain end, Proposition 3.46's end and
    the capped R8 end. The confidence track and its referee read the remark as about several blocks, where the shares
    are unknown, and the two statements are then consistent; the remark does not say so, and it is left as written.
14. Tags in §3.7a. The confidence track tags every result [proved, unrefereed] in its revised form, because no one has
    re-read the revision. This chapter tags [proved, refereed] the results whose statement and proof the referee read
    and found no error in and the revision left unchanged, checked by comparing `CLAIMS.md` with
    `CLAIMS_before_referee.md`: Lemma 3.34, Propositions 3.35 to 3.37, Theorem 3.38, Lemmas 3.39 to 3.41, Theorem 3.43
    and Corollary 3.45. Theorem 3.43's proof uses Lemma 3.42, whose convention φ := π is the referee's repair and is
    tagged unrefereed; the referee states the theorem is unaffected. If the PI reads the track's blanket tag as binding,
    downgrade these to [proved, unrefereed].
    *Audit, 21 November 2026:* the two files were compared claim by claim. F1, A1, A2, A3, Theorem A, B0, B1, B2, Theorem
    B and B6 are identical in both, and the referee's verdict on each is "no error found" (section 8; section 4 for B0,
    B1, B2, Theorem B and B6). B3 differs (the ŷ = 0 sentence), B4 lost its parenthesis on δ and gained the referee's
    proof, and A4, B5 and the tax were rewritten, so 3.42, 3.44, 3.46, 3.49 and 3.50 rightly stay unrefereed; A4′ and A5
    are new. The proofs of Lemmas 3.39 and 3.41 and Corollary 3.45 here spell out steps the track leaves implicit (the
    attainability of the maximum, ψ* and the union over grid indices, ϑu + (1 − ϑ)u = u); those steps are the referee's
    checks or arithmetic, and the tags stand on that reading. The remark on the least angle after Proposition 3.37 is not
    named in the verdicts and is tagged unrefereed.
15. The same choice in §3.2a and §3.4: Proposition 3.25 (E1) and Theorem 3.26, Propositions 3.27 and 3.28 (A1, A2, C1 of
    the rotations track) are tagged [proved, refereed] on the rotations referee's "no error found"; the track itself
    marks every proof unrefereed and was not revised after the referee. The referee's unstated hypotheses for Theorem
    3.26 (almost every z; the scalar law only) are recorded beside it, not added to the statement.
    *Audit, 21 November 2026:* `notes/program/rotations/CLAIMS.md` was last changed in the commit before the one that
    added `referee/referee-1.md`, and is unmodified in the worktree, so the referee read the text quoted here. E1, A1,
    A2 and C1 match it after renaming, except that Proposition 3.25 had dropped "at most" before the share, now
    restored; Proposition 3.27 omits the track's parenthetical example of sign–Hadamard products, which is not a
    hypothesis. The verdicts are "no error found" for each. The referee's remarks on A2's scope and on C2, and repairs 1
    to 10 to E1's use, are the referee's own and are tagged unrefereed.
16. §3.2's bullet "Haar is a model of a pseudo-random rotation from a 64-bit seed, so probabilities near 2^−64 mean
    nothing", with the per-event budget of about 2^−62, reads the per-event δ as a promise about seeds. The rotations
    referee (§3.4, "The 64-bit seed") argues that the guarantee is against ε, which is 1.8·10¹⁰ atoms at every N, and
    that the per-event δ is a step in a union bound under the model; and the seed as built is a public constant, so S = 1.
    The bullet is refereed text and is left as written; §3.4 now states the correction.
17. The rotations track's E2 table gives int16 shares at depths 1 / 2 / 4 / 8; the referee reproduced depths 1, 4 and 8
    only, at n_j = 256 and 768, with a ρ_R that moves ±20% between draws (7.4·10⁻⁵ against the track's 8.9·10⁻⁵ at 256).
    §3.2a gives both rows. For use through the kernel's slot the referee's figures (repair 2) supersede the track's.
18. Lemma 3.49's sum of the two tails at d = 255, ξ² = 0.001: the theorist computes 1.91 (2·T_d(ζ√(1 + ξ²))/T_d(ζ) =
    1.908 at v = 0) and the referee 1.94. Both give the same conclusion; the chapter quotes the theorist's with the
    referee's beside it.
19. Lemma 3.49's last figure. The track and the referee quote ζ_d(δ/2)/ζ_d(δ) − 1 ≈ ln 2/(d·ζ²) as 0.5% at d = 255.
    With the track's own ζ = 0.54 (or the table's 0.5186) the formula gives 0.93% (1.0%), not 0.5%. A direct evaluation
    of the two quantiles from the incomplete beta function, made for this audit, gives 0.73% at d = 255 and 0.54% at
    d = 95. The conclusion (under 1%) stands either way; the figure is for the PI to settle with the track.
20. Proposition 3.48's proof applies Theorem 3.11's repaired upper end on the one-sided event 𝓔₁ = {−U ≤ ζ}, and the
    track says that end "is proved on this same event". Theorem 3.11's repaired statement, as written in §3.5, is on the
    two-sided event |U| ≤ c/√d. Query 12 records that the surge referee found the capped end right with the upper event
    alone, by a check on 20,000 cases, and that bounds2's one-sided argument looks wrong without the cap. Whether a
    one-sided proof of the capped end exists in the sources is for the PI; until then the R8 part of Proposition 3.48
    rests on that reading.
