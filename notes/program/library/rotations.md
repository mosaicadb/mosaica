# Library: cheap structured rotations in place of Haar (stage 0)

Librarian's report for the confidence and codes tracks, written 21 November 2026. The question is whether the dense
Haar rotation of each randomised block can be replaced by a cheap structured one while a failure probability near
10⁻¹⁹ still holds for every input. No computation was run. Symbols follow `notes/00-notation.md`.

How each source was read:

- **[paper §x]**: I read the paper's own text (arXiv HTML, a PDF text extraction or a publisher page) at the named place.
- **[abstract]**: only the abstract or the listing page.
- **[summary]**: a search engine's or fetch tool's summary, one step removed from the text.
- **[cited, unchecked]**: from memory or from a secondary mention; I could not open the source.

A fetch tool returns a paraphrase produced by a small model; statements tagged [paper §x] were asked for verbatim and
are given as returned, but a theorist relying on a constant should re-open the source at the named place.

## 0. What property is needed

Lemma 3.1 (chapter 3 §3.2) uses Haar invariance in one step: for O fixing y, R·Oᵀ is again Haar. What the guarantee
consumes is the joint law of the pair (Ry, RΔ) for a fixed item block y and a fixed query weight Δ, both chosen without
knowledge of ω: the code is a function of Ry, and the estimator is linear in RΔ given the code. So a replacement
distribution μ on O(n) suffices for the per-event bound if and only if, for every fixed pair (y, Δ), the law of
(Ry, RΔ) under μ equals its law under Haar, or is close to it in a sense strong enough to move a probability of
2.5·10⁻¹⁹ by a small factor (librarian's reduction, not from a source).

Two consequences follow from the reduction, and the sections below test the literature against them.

- **Closeness must be multiplicative, not additive.** Total-variation distance τ only gives |Pr_μ(F) − Pr_Haar(F)| ≤ τ,
  which is useless unless τ ≲ 10⁻¹⁹. What would serve is a density ratio dμ_pair/dHaar_pair ≤ 1 + η on the events
  that matter (a relative-error or L^∞ bound), or τ itself below 10⁻²⁰.
- **The function is not a polynomial.** The code is a quantised function of Ry, so moment matching (designs) controls
  the event only through a polynomial approximation of an indicator.

## 1. Sign diagonals and Hadamard transforms

Write H for the normalised d×d Walsh–Hadamard matrix (d a power of two), D, D₁, D₂, … for independent diagonals of
Rademacher signs, and "k RHTs" for H·D_k⋯H·D₁. Every such product is exactly orthogonal, so ‖Ry‖ = ‖y‖ and
⟨Ry, RΔ⟩ = ⟨y, Δ⟩ hold for every draw; only the law of the code, a non-linear function of Ry, is in question.

### 1.1 What each source states

**Ailon and Chazelle, flattening lemma (FJLT, STOC 2006; SIAM J. Comput. 2009).** As restated in Fastfood (Le, Sarlós,
Smola, ICML 2013, arXiv:1408.3060, Lemma 12), for x ∈ ℝᵈ and δ > 0,
Pr[‖HDx‖_∞ ≥ ‖x‖₂·√((2/d)·log(2d/δ))] ≤ δ (the normalised form is the one used in its proof of Theorem 11). This is
Hoeffding's inequality on each coordinate plus a union over d coordinates; it holds at every δ.
[paper: Fastfood §4 Lemma 12 and the proof of Theorem 11; the original FJLT paper not opened, so the attribution and
any constant in the original are cited, unchecked.]
At δ = 10⁻¹⁹ and d = 128 the bound is ‖HDx‖_∞ ≤ 0.88·‖x‖ (my arithmetic), so flattening says almost nothing at the
failure levels of this program.

**Andoni, Indyk, Laarhoven, Razenshteyn, Schmidt, "Practical and optimal LSH for angular distance" (NeurIPS 2015,
arXiv:1509.02897).** §3 ("Pseudo-random rotations") uses x ↦ HD₃HD₂HD₁x and says, near verbatim: "While we are
currently not aware how to prove rigorously that such pseudo-random rotations perform as well as the fully random
ones, empirical evaluations show that three applications of HDᵢ are exactly equivalent to applying a true random
rotation (when d tends to infinity). We note that only two applications of HDᵢ are not sufficient."
[paper §3, read in full at that place.] So the three-round rule is empirical and asymptotic in d, and the
insufficiency of two rounds is an empirical remark for their hash, not a theorem.

**Kennedy and Ward, "Fast cross-polytope locality-sensitive hashing" (arXiv:1602.06922).** Prove that the optimal
asymptotic sensitivity of cross-polytope LSH survives when the Gaussian matrix is replaced by an FJLT followed by a
"discrete pseudo-rotation", in O(d ln d) time, with an O(ln⁹ d)-random-bit variant. [abstract only.] The guarantee is
an asymptotic collision-probability exponent, not a tail at a fixed small probability.

**Choromanski, Rowland, Weller, "The unreasonable effectiveness of structured random orthogonal embeddings" (NeurIPS
2017, arXiv:1703.00864).** [paper §3–5, theorem statements read.]
- Theorem 3.3: for the "S-Rademacher" product of k blocks SD (S = H) with m rows subsampled without replacement, the
  inner-product estimator is unbiased with MSE = (1/m)·((n−m)/(n−1))·( ((xᵀy)² + ‖x‖²‖y‖²)
  + Σ_{r=1}^{k−1} (−1)^r 2^r n^(−r) (2(xᵀy)² + ‖x‖²‖y‖²) + (−1)^k 2^k n^(−(k−1)) Σᵢ xᵢ²yᵢ² ). The last term carries the
  input's coordinate structure; it shrinks by 2/n per block, odd k beating even k. Only second moments are given.
- Theorem 4.4: for the angular kernel with any random M, MSE is written through Pr[Aⁱ] and the pairwise covariances
  δ_{i,j} of the events Aⁱ = {sgn(rⁱ·x) ≠ sgn(rⁱ·y)}; for G and G_ort, Pr[Aⁱ] = θ/π exactly. The improvement theorems
  are for G_ort (Gaussian conditioned on orthogonal rows), not for HD products.
- Proposition 5.2: the "Hadamard–Rademacher process" X_k = HD_kX_{k−1} in two dimensions explores 16 matrices, is
  ergodic for the uniform law on that set, has period 2 and is fully mixed after 3 steps. In general the products lie
  in the finite group generated by H and the sign diagonals, so the law never approaches Haar: it is supported on a
  finite set (librarian's remark, following from the definition).
- No tail bound beyond the MSE appears.

**Le, Sarlós, Smola, "Fastfood" (ICML 2013, arXiv:1408.3060).** V = (1/(σ√d))·S·H·G·Π·H·B with B signs, Π a
permutation, G a Gaussian diagonal and S a scaling diagonal. [paper §4, read at the theorems.]
- Theorem 9: each feature has the variance of the Gaussian random-features estimator, and the variance of the sum is
  at most (d/2)(1 − e^(−‖v‖²))² + d·C(‖v‖), C(α) = 6α⁴(e^(−α²) + α²/3).
- Theorem 11: for a d×d block and v = (x − x′)/σ,
  Pr[|k̂ − k| ≥ 2σ⁻¹d^(−1/2)‖x − x′‖·√(log(2/δ)·log(2d/δ))] ≤ 2δ for all δ > 0. The proof conditions on B (flattening,
  Lemma 12) and applies Gaussian concentration of Lipschitz functions (Ledoux) to the Gaussian diagonal G.
- This is the one fixed-pair tail bound at arbitrary δ for a non-linear estimator under a structured transform that I
  found; it works because a genuinely Gaussian factor G is inserted after flattening, and its price is the extra
  √log(2d/δ), about 7 at δ = 10⁻¹⁹ and d = 128 (my arithmetic).

**Ben-Basat, Kuszmaul, Mitzenmacher, Portnoy, Vargaftik, "Quantizing with randomized Hadamard transforms: efficient
heuristic now proven" (arXiv:2605.06014, May 2026).** [paper §3, §5, §6 and Appendix C, read.]
- One RHT: the example x = (e₀ + e₁)/√2 gives HDx with every coordinate in {0, ±√(2/d)} (§1).
- One RHT, tails: citing Bentkus and Dzindzalieta (Bernoulli 21(2), 2015), the tail of one coordinate of √d·HDx can be
  up to 3.1787 times the standard Gaussian tail at the same threshold, for worst-case inputs (§5). That inequality is
  multiplicative and holds at every level [the Bentkus–Dzindzalieta paper itself not opened: cited, unchecked;
  the constant is 1/(4·Pr[G ≥ √2]) in my recollection].
- Lemma 1: E ρ₃(HDx̃) ≤ 3^(3/4)/√d, with ρ₃(a) = Σ|aᵢ|³.
- Theorem 1 (two RHTs): for U = √d·(R₂x̃)₁, d_K(U, G) ≤ 0.5606·3^(3/4)/√d ≤ 1.28/√d, for every input and every
  coordinate (Berry–Esseen with constant 0.5606, conditional on the first RHT). Theorem 2 gives an O(d^(−1/2))
  Wasserstein-1 bound.
- Lemma 3 and Appendix C.2: for x̃ = (e₀ + e₁)/√2 after two RHTs, U₁ = S·U₀ exactly with S a Rademacher sign, so
  |U₀| = |U₁| on every draw; zero covariance but complete dependence, and a fixed 2-D codebook then has a limiting
  error above the Gaussian one.
- Theorem 6 (three RHTs): the D₃-conditional covariance of two coordinates has mean 0 over D₂, second moment at most
  2‖y‖²_∞ given y = R₁x̃, and RMS O(d^(−1/2)) over D₁, D₂. Theorem 7: a fixed bounded k-dimensional codebook has the
  same expected error as under a uniform rotation up to an additive term vanishing in d.
- Every bound is additive (Kolmogorov, Wasserstein, expected error). The paper notes FAISS and FALCONN use three RHTs.

**"Approximating uniform random rotations by two-block structured Hadamard rotations in high dimensions"
(Zilca and Mendelson, arXiv:2604.23418, April 2026).** [abstract and theorem statements read.]
- Theorem 4.1 (d = 2^m ≥ 8): for one coordinate scaled by √d, uniformly over inputs,
  e^(−1/24)/√(2πd) ≤ Δ_K(d) ≤ C_BE·√(3 log d/d) + 2/√d + 4/(d − 4) with C_BE = 0.56, and
  √(5/(64πe))/√d ≤ Δ_W(d) ≤ (3√(3 log d) + 6 + √2)/√d. The lower bounds are attained at u = e₁.
- Theorem 5.2: the Wasserstein-1 distance between the full output vector law and the uniform one, worst case over
  inputs, exceeds 1/3 for d ≥ 256 and 0.6 for d ≥ 32768, with lim inf ≥ √(2(1 − √(2/π))) ≈ 0.6358. The authors state
  that the marginal results "do not establish joint-coordinate approximation".

**Feng, Indyk, Kapralov, Krachun, Prokhorov (as cited by the next paper), "DASHQuant: provable quantization with a
single randomized Hadamard transform" (arXiv:2605.13810).** One HD plus a dither that is uniform in quantile space.
Theorem 1.1: unbiased, and E‖x − x̃‖² ≤ (π√3/2 + o(1))·4^(−b) uniformly in d and x as b → ∞. Theorem 1.2: for the
inner-product variant, E|⟨y, x̂ − x⟩|² ≤ 13(π√3/2 + 1 + o(1))·‖y‖²/(d·4^b). [paper §1, read.] Second moments only.

**"Pairwise-independent dithering for single-stage Hadamard quantization" (arXiv:2608.02564, August 2026).**
Theorem 1.1: one HD with a dither pair (V₀, V₁) independent of D gives E x̂ = x and
E|⟨y, x̂ − x⟩|² ≤ (3π√3/2 + o_B(1))·‖y‖²/(dB²), B = 2^b, uniformly in d, x and y. [paper, theorem read.] Second
moments only.

**Freksen and Larsen, "On using Toeplitz and circulant matrices for Johnson–Lindenstrauss transforms" (arXiv:1706.10110,
ISAAC 2017).** There is a set of N vectors for which the random Toeplitz embedding needs m = Ω(ε⁻²·lg²N) rows, so
the known O(ε⁻² lg² N) analysis is tight. [abstract.] Read as a failure probability (lg N ↔ lg(1/δ), my reading, not
checked in the paper), a structured transform can have a tail of the form exp(−c·ε·√m) where the dense one has
exp(−c·ε²·m): structure costs most exactly at very small δ.

### 1.2 Summary for item 1

- **Established.** Exact orthogonality. Flattening ‖HDx‖_∞ ≤ ‖x‖√(2 log(2d/δ)/d) at every δ. A multiplicative
  Gaussian comparison for the tail of one coordinate after one HD (factor ≤ 3.18, Bentkus–Dzindzalieta, unchecked).
  For two RHTs, each coordinate within 1.28/√d of Gaussian in Kolmogorov distance; for three, decaying pairwise
  conditional covariance and codebook universality in expectation. Fastfood's fixed-pair tail at every δ, bought with a
  Gaussian diagonal. Unbiasedness and MSE bounds for one HD with dither (DASHQuant and its follow-up).
- **Empirical only.** That three HD rounds behave as a Haar rotation (Andoni et al.; Yu, Suresh, Choromanski,
  Holtmann-Rice, Kumar, "Orthogonal random features", NeurIPS 2016, arXiv:1610.09072, whose structured variant SORF
  is HD₁HD₂HD₃ [abstract]; the 2017 paper's experiments); that two rounds are insufficient for LSH. Choromanski,
  Rowland, Chen, Weller, "Unifying orthogonal Monte Carlo methods" (ICML 2019) gave numerical support for Kac-walk
  (random Givens) matrices in this role [cited by Jain et al. 2020; not opened].
- **Not done, as far as I found.** Any bound on the joint law of (Ry, RΔ) under an HD product that is multiplicative,
  or additive at the 10⁻¹⁹ scale; any extreme-tail bound for an inner-product estimator built from a quantised HD
  image; any analysis of three RHTs beyond second moments. The proven distances are O(d^(−1/2)) additive, which at
  d ≤ 288 is about 10⁻¹, eighteen orders above the budget.
- **Known failure cases.** Sparse inputs: (e₀ + e₁)/√2 under one RHT (coordinates in {0, ±√(2/d)}) and under two
  RHTs (|U₀| = |U₁| identically); e₁ under two blocks (non-vanishing full-vector Wasserstein distance ≥ 0.6358 in the
  limit). Every HD product has a finitely supported law (at most 2^(kd) matrices), so it cannot satisfy Lemma 3.1's
  invariance exactly for any input.

## 2. Kac's walk, random Givens rotations and random reflections

Kac's walk on SO(n): Q_t = R(i_t, j_t, θ_t)·Q_{t−1}, with (i_t, j_t) a uniform pair of coordinates and θ_t uniform on
[0, 2π). One step costs O(1) to apply to a vector, so T steps cost O(T). For a fixed pair (y, Δ), the pair
(Q_T y, Q_T Δ) is the "2-column walk" on the Stiefel manifold St(n, 2) started from (y, Δ) (after orthonormalising),
which is exactly the object Lemma 3.1 needs.

### 2.1 What each source states

**Carlen, Carvalho, Loss, "Determination of the spectral gap for Kac's master equation and related stochastic
evolution" (Acta Math. 191 (2003) 1–54; arXiv:math-ph/0109003).** For Kac's walk on the sphere with uniform angle
density ρ = 1/2π, the gap of the generator N(Q − I) is Δ_N = ½·(N + 2)/(N − 1) for every N (eq. 1.12), so
liminf Δ_N = ½ (Kac's conjecture with the constant). The paper also treats the SO(N) model. A gap gives
‖G_t(f₀ − 1)‖₂ ≤ e^(−tΔ_N)‖f₀ − 1‖₂ only for an initial density in L². [paper §1, read.] Hough and Jiang (below) note
that the Kac walk on SO(N) started at a point "is not in L² after any finite number of steps", so the gap gives no
pointwise or total-variation bound from a fixed start.

**Pillai and Smith, "Kac's walk on n-sphere mixes in n log n steps" (Ann. Appl. Probab. 27 (2017) 631–650;
arXiv:1507.08554).** The total-variation mixing time of Kac's walk on S^(n−1) is between ½·n log n and 200·n log n.
[abstract.] This is for one vector, at the usual TV threshold ¼. Boosting by sub-multiplicativity,
d(kt) ≤ (2d(t))^k, gives TV ≤ 2^(−k) after k·200·n log n steps, so TV ≤ 10⁻²⁰ needs about 67 × 200·n log n ≈ 1.3·10⁴
n log n Givens steps (my arithmetic), against n² for a dense product. TV is additive in any case.

**Pillai and Smith, SO(n) (Ann. Probab. 46(4) 2018; arXiv:1605.08122).** TV mixing time on SO(n) between C₁n² and
C₂n⁴ log n "for some explicit constants", improving Jiang's O(n⁵ log² n) (Ann. Appl. Probab. 2012). [abstract; the
explicit constants not read. The 2026 paper below cites the requirement T > 10⁷·n⁴ log n from it, via search summary.]

**Pillai and Smith, "Kac's walk on rotation matrices mixes in n² log n steps" (arXiv:2604.23828, April 2026).**
Theorem 1: τ_mix = O(n² log n) in TV on SO(n); a lower bound Ω(n²) follows by counting dimensions. The proof's
second stage gives the failure of the coupling as o(1): no explicit constant. [paper §1 and §9, read.] It quotes
Oliveira's Theorem 1 exactly: for all ε > 0, t ≥ ⌈n² log(π√n/ε)⌉ and every start z,
W_{D_HS,2}(K^t(z, ·), Haar) ≤ ε (Hilbert–Schmidt distance, L² Wasserstein).

**Oliveira, "On the convergence to equilibrium of Kac's random walk on matrices" (Ann. Appl. Probab. 2009;
arXiv:0705.2253).** Wasserstein (L² transport) convergence to Haar on SO(n) in O(n² ln n) steps, at most an O(ln n)
factor from optimal; with the explicit form above. [abstract, and the theorem as quoted by Pillai–Smith 2026.] Its
Theorem 6 (as cited by Jain et al. 2020) shows that the walk cannot mix on the k-Stiefel manifold in fewer than
Ω(kd) steps, by dimension counting. [cited, unchecked.]

**Pillai, Smith, Vaikuntanathan, "On the pseudo-mixing of Kac's walk" (arXiv:2608.17374, August 2026).** [paper §1–2,
theorems read.]
- Theorem 1 (and Theorem 3): there is a universal constant C such that for 1 ≤ k < n, T ≥ 0 and all X₀, Y₀ in
  St(n, k), W_D(P^T(X₀, ·), P^T(Y₀, ·)) ≤ n^(−⌊T/(C·n(k + log n)·log n)⌋)·D(X₀, Y₀), with D the Riemannian distance
  on St(n, k). Taking Y₀ Haar-distributed, the k-column law is within that Wasserstein distance of Haar. For k = 2
  this is the pair law Lemma 3.1 needs, with geometric decay in T, at a cost of about C·n·log²n steps per factor of
  n; C is not given.
- Theorem 2: if T = ω(nk(k + log n) log n), every degree-k polynomial normalised to unit Haar variance has expectation
  under the T-step law within o(1) of its Haar expectation ("weak k-indistinguishability"). Asymptotic.
- Theorem 6.1: a JL transform from Kac's walk with the usual target dimension, r = ⌈log(N/δ)⌉, T ≥ C_JL·n(4r + log n)·(…);
  the δ-dependence enters through the polynomial degree r. [statement truncated in my extraction.]
- The paper recalls the conjecture of Vaikuntanathan and Zamir that n·polylog(n) Kac steps are computationally
  indistinguishable from Haar, and their counting argument that Haar matrices, and any law TV- or Wasserstein-close to
  Haar, cannot admit sub-quadratic matrix–vector multiplication.

**Jain, Pillai, Sah, Sawhney, Smith, "Fast and memory-optimal dimension reduction using Kac's walk" (arXiv:2003.10069,
Ann. Appl. Probab.).** Theorem 1.4: Kac's walk for O(d log d) steps, then a second phase, gives a JL map to
k ≤ Cε⁻² log n dimensions preserving all n norms within 1 ± ε "with probability at least 2/3"; the Remark says the
failure probability can be made η "with easy and standard modifications", not tracked. Theorem 1.6: the same with the
angle fixed at π/4 ("orthogonal repeated averaging", ORA), whose law has TV distance 1 from uniform after any finite
number of steps. The analysis uses the Pillai–Smith contractive coupling to show the walk produces flat
("bounded") orthogonal systems. [paper §1.3–1.4, read.]

**Systematic scan (Jones, Osipov, Rokhlin, PNAS 2011, randomised approximate nearest neighbours) and Sepehri,
"Wasserstein mixing of a systematic-scan random rotation sampler" (arXiv:2609.13424, September 2026).** Jones et al.
proposed sweeps of permutations, sequential 2-D rotations and a Fourier-type transform as a fast surrogate for a Haar
rotation in ANN search, applied in O(n(log n + m)) for m sweeps, conjectured to approach Haar after logarithmically
many sweeps. Sepehri proves this does not happen for the full matrix law in Frobenius Wasserstein distance: at fixed
normalised accuracy the mixing time is between order n/log n and order n sweeps, and the output law is singular with
respect to Haar for fewer than n/2 sweeps. [abstract and §1 read; Jones et al. cited, unchecked.]

**Random reflections and random-plane rotations (conjugation-invariant walks).**
- Porod, "The cut-off phenomenon for random reflections" (Ann. Probab. 24 (1996) 74–96): walks on O(N), U(N), Sp(N)
  whose steps are reflections have TV cut-off at ½·N log N. [search summary; paper not opened: cited, unchecked.]
- Rosenthal, "Random rotations: characters and random walks on SO(N)" (Ann. Probab. 1994): rotation by a fixed angle
  θ in a uniformly random 2-plane; lower bound at the cut-off for every θ and the upper bound for θ = π, via
  characters and an L² bound. [as described by Hough and Jiang; not opened.]
- Hough and Jiang, "Cut-off phenomenon in the uniform plane Kac walk" (Ann. Probab.; arXiv:1211.2031). [paper §1,
  read.] Theorem 1.1: on SO(2n+1), with angle law ξ (not δ₀) and σ(θ) = sin(θ/2), TV cut-off at
  t = n log n/(2ξ(σ²)). Theorem 1.2: for a fixed angle with (log n)/√n ≤ θ ≤ π, cut-off at n(log n + c)/(2σ²(θ)).
  Theorem 1.3: if ξ is supported away from 0 and π and q is its point nearest 0, the L² mixing time is
  ~ n log n/(4σ²(q) ∧ 2ξ(σ²)), with an explicit upper bound max(n log n/(2ξ(σ²)), n(log n + 2 log log n)/(4σ²(q))) + c_k·n.
  Corollary 1.2: the same holds "with the mixing time doubled and the L² norm replaced by the L^∞ norm with respect
  to Haar measure". This is a multiplicative (density-ratio) guarantee, the kind Lemma 3.1 could use. But each step
  is a rotation in a uniformly random plane, an O(n) operation, and Θ(n log n) steps are needed, so applying the
  result costs Θ(n² log n) per vector, more than the dense n².

### 2.2 Summary for item 2

- **Established.** Exact spectral gap on the sphere, ½(N + 2)/(N − 1) for N(Q − I) (L² starts only). TV mixing on the
  sphere between ½n log n and 200·n log n; on SO(n), O(n² log n) with no explicit constant. Wasserstein mixing on
  SO(n) after ⌈n² log(π√n/ε)⌉ steps, explicit. Wasserstein contraction of the k-column walk (k = 2: the pair law) at
  rate n^(−⌊T/(C n(k + log n) log n)⌋), C unstated. L^∞ (density-ratio) cut-off for conjugation-invariant
  random-plane walks, at twice the L² time, but those steps cost O(n) each.
- **Empirical only.** That Kac-type or systematic-scan rotations with n·polylog(n) operations behave as Haar for
  search and quantisation.
- **Not done, as far as I found.** A TV bound of order 10⁻¹⁹, or any density-ratio bound, for the pair law of a cheap
  (O(1)-per-step) walk at a cost below n². For the Givens walk the law on SO(n) is never in L² from a point start, so
  L^∞ bounds there are excluded; whether the induced pair law on St(n, 2) acquires a bounded density after O(n log n)
  steps is, as far as I found, not studied.
- **A route, not a result (librarian's remark).** The St(n, 2) Wasserstein bound decays geometrically, and a failure
  event that is a threshold of a Lipschitz function, apart from quantiser boundaries, transfers from Haar with an
  additive loss W/η plus the Haar mass of an η-neighbourhood of the boundaries. Making both ≤ 10⁻²⁰ needs W ≈ 10⁻⁴⁰,
  i.e. about 20 contraction epochs of C·n(2 + log n)·log n steps at n ≈ 128; whether this beats n² depends on the
  unstated C. Nobody has done this.

## 3. Butterfly networks

**Parker (1995), "Random butterfly transformations with applications in computational linear algebra" (UCLA technical
report).** Introduced random butterfly matrices to avoid pivoting in Gaussian elimination; his butterflies are not
orthogonal in general. [cited, unchecked; described by Trogdon and by Peca-Medlin and Trogdon.]

**Trogdon, "On spectral and numerical properties of random butterfly matrices" (Appl. Math. Lett. 2019;
arXiv:1710.00087).** [paper §1–2, read.]
- Definition 1.2: an orthogonal butterfly of size N = 2ⁿ is B = [[C A₁, S A₂], [−S A₁, C A₂]] with A₁, A₂ butterflies
  of size N/2 and C, S commuting symmetric matrices with C² + S² = I; it is applied in O(N log N).
- "Haar-butterfly" (simple) matrices take C_j = cos θ_j·I, S_j = sin θ_j·I with θ_j i.i.d. uniform, and A₁ = A₂ at
  each level. The set B(2ⁿ) of simple butterflies is a subgroup of SO(2ⁿ) (proved in §2; the product rule shows the
  angles add, so it is an abelian group, a torus of dimension n = log₂ N), and Proposition 2.1 says the induced law is
  Haar measure on that subgroup.
- "Non-simple" random butterflies use independent angles at every position (N/2·log₂N angles). The paper compares
  eigenvalue statistics and linear-algebra performance with Haar and SRFT matrices; it gives no result on the law of
  By for a fixed y.

**Peca-Medlin and Trogdon, "Growth factors of random butterfly matrices and the stability of avoiding pivoting"
(arXiv:2203.15921).** Growth-factor distributions for a subclass of random butterflies; numerical comparisons.
[abstract.]

**Summary for item 3.**
- **Established.** Exact orthogonality, O(N log N) application, and for the simple class the exact law (Haar on a
  log₂N-dimensional torus subgroup). Spectral and growth-factor properties.
- **Librarian's remark.** For the simple class, the orbit {By} of a fixed y is at most log₂N-dimensional, so the law
  of By is singular on the sphere and far from uniform; for the non-simple class, Be₁ has coordinates that are
  products of log₂N cosines and sines, again far from the sphere's law for a sparse input.
- **Empirical only.** Use as a fast preconditioner in randomised linear algebra.
- **Not done, as far as I found.** Any distributional guarantee for By or for pairs (By, BΔ), any tail bound, any
  comparison with Haar beyond spectra. Butterflies, as stated, are a weaker object than HD products for this question.

## 4. Orthogonal and unitary designs

A distribution ν on O(N) or U(N) is an exact k-design if E_ν[ρ^(k,k)(g)] = E_Haar[ρ^(k,k)(g)], i.e. all moments of
degree ≤ k in the entries match Haar. ε-approximate versions are additive (operator-norm distance of the moment
operators) or relative (as completely positive maps, (1 − ε)Φ_Haar ≼ Φ_ν ≼ (1 + ε)Φ_Haar).

**Low, "Large deviation bounds for k-designs" (Proc. R. Soc. A 2009; arXiv:0903.5236).** [paper §1, read.]
- Theorem 1.2: let f be a polynomial of degree K in the entries, f = Σᵢ αᵢ Mᵢ(U) with monomials Mᵢ and α(f) = Σ|αᵢ|.
  If P_Haar(|f − μ| ≥ δ) ≤ C e^(−aδ²) and ν is an ε-approximate unitary k-design, then
  P_ν(|f − μ| ≥ δ) ≤ δ^(−2m)·( C(m/a)^m + (ε/d^k)(α + |μ|)^(2m) ) for every integer m with 2mK ≤ k.
- For an exact design and the best m ≈ aδ²/e, this gives P_ν ≤ C e^(−aδ²/e): the Haar exponent divided by e.
- Theorem 1.5: a design supported on S equally likely elements has, for any non-constant f, P_ν(|f − μ| ≥ δ) ≥ 1/S for
  some δ > 0. So a tail of 10⁻¹⁹ needs at least 10¹⁹ design elements (63 random bits) in any case.
- Non-polynomial f (entropy) are handled by bounding them through a polynomial (the 2-norm).

What this means here (my arithmetic, from Theorem 1.2 with exact designs): with the Haar tail written as
e^(−c²/2) = 10⁻¹⁹ at c ≈ 9.35, the design bound reaches only e^(−c²/(2e)) ≈ 10⁻⁷ at the
same threshold; matching 10⁻¹⁹ needs the threshold raised by √e ≈ 1.65 and m ≈ 44, i.e. design degree k ≥ 88·K for a
degree-K test function. Low's moment estimate C(m/a)^m is crude; with exact Gaussian moments the loss would be only a
polynomial factor, but Low does not state that version.

**O'Donnell, Servedio, Paredes, "Explicit orthogonal and unitary designs" (FOCS 2023; arXiv:2310.13597).** [paper
§1, read.] Strongly explicit ε-approximate k-designs for O(N) and U(N), N = 2ⁿ, of cardinality poly(N^k/ε), seed
length O(nk + log(1/ε)), for every k. Theorem 1.2: the multiset P_n of one-gate circuits (CNOT on two qubits, or
Q = [[3/5, −4/5], [4/5, 3/5]] on one qubit, closed under negation and inverse) has
‖E_{P_n}ρ^(k,k) − E_Haar ρ^(k,k)‖_op ≤ 1 − 1/(n·poly(k)); a product of n·poly(k)·log(2^(nk)/ε) random gates is an
ε-approximate k-design (seed poly(n, k)·log(1/ε)), improved to O(nk + log(1/ε)) by derandomised walks. Each gate
costs O(N) on a vector, so a vector costs O(N·n·poly(k)·log(1/ε)). The paper reports an error in the earlier
orthogonal-design construction of Kothari and Meka (STOC 2015), whose pseudorandom generator for spherical caps
relied on it.

**Brandão, Harrow, Horodecki, "Local random quantum circuits are approximate polynomial-designs" (Commun. Math. Phys.
2016)** and **Schuster, Haferkamp, Huang, "Random unitaries in extremely low depth" (arXiv:2407.07754).** Unitary,
qubit-structured designs: BHH show that local random circuits of depth poly(k)·n form approximate k-designs (in
relative error in my recollection); SHH glue local designs on log n-sized patches to get approximate designs in depth
log n on any geometry. [SHH: abstract; BHH: cited, unchecked.] These concern U(2ⁿ) acting on qubits; I found no
analogue for O(N) at small N with a cost below N².

**Pillai, Smith, Vaikuntanathan (§2 above).** Kac's walk after ω(nk(k + log n) log n) steps fools degree-k
polynomials to o(1): a Kac-walk "approximate design" statement, asymptotic only.

**Summary for item 4.**
- **Established.** Large-deviation transfer from Haar to k-designs for polynomial test functions (Low), at the cost
  of a factor e in the exponent or degree k ≈ 2·(log(1/δ))·K. Explicit approximate orthogonal designs with optimal
  seed length (O'Donnell–Servedio–Paredes). A floor: any design with S equally likely elements has tails ≥ 1/S.
- **Empirical only.** Nothing specific found.
- **Not done, as far as I found.** A design result for a non-polynomial function of the rotated vector at a fixed
  small probability. A quantised code is a step function of Ry; approximating its indicator by a polynomial to
  additive accuracy 10⁻¹⁹ needs a degree that grows polynomially in 1/accuracy for sharp thresholds (a general fact
  about polynomial approximation of step functions, not checked in a source here), which rules out designs as the
  route at these levels unless the code is first smoothed (for example by dither, §6). No cheap construction with
  cost below N² for N in the hundreds is known to me to be an exact orthogonal k-design for k ≥ 4.

## 5. Exact Haar constructions and their cost

**Mezzadri, "How to generate random matrices from the classical compact groups" (Notices AMS 54 (2007);
arXiv:math-ph/0609050).** [paper §7–8, read.]
- QR of a Ginibre (i.i.d. Gaussian) matrix gives Haar only after fixing the signs of R's diagonal; a black-box QR
  routine returns the wrong law without that correction.
- Householder form: Q = H_N(v̂_N)⋯H_1(v̂_1) with v̂_m uniform on S^(m−1) (one reflection per dimension, m = N, …, 1).
  "Generating all the Householder reflections is an O(N²) process", applying one to a vector costs O(N), and
  computing Qw costs O(N²) in total; forming Q explicitly costs O(N³). The random input is N(N + 1)/2 Gaussians, against
  dim O(N) = N(N − 1)/2.
- §8 presents this as the Diaconis–Shahshahani subgroup algorithm: if Γ_m ≅ C_m × Γ_{m−1} (coset representatives
  times a subgroup), a uniform coset representative times a uniform element of the subgroup is uniform on Γ_m; the
  Householder product is the instance for the chain O(1) ⊂ O(2) ⊂ ⋯ ⊂ O(N).

**Stewart, "The efficient generation of random orthogonal matrices with an application to condition estimators"
(SIAM J. Numer. Anal. 17 (1980) 403–409)** and **Diaconis and Shahshahani, "The subgroup algorithm for generating
uniform random variables" (Probab. Eng. Inf. Sci. 1 (1987) 15–32).** Stewart's method is the Householder product
above; Diaconis–Shahshahani give the general subgroup algorithm. [both cited by Mezzadri; not opened: cited,
unchecked.]

**Cost comparison (my arithmetic).** Applying the Householder product costs about Σ_{m≤n} 4m ≈ 2n² flops per vector,
the same order as a stored dense matrix (2n² flops) but with n²/2 stored numbers and worse memory locality; neither
is cheaper. At the block sizes of chapter 3 (n_j = 96 to 288) a dense product is about 1.8·10⁴ to 1.7·10⁵ flops per
block.

**Exact uniformity on pairs with fewer parameters.**
- Truncating the subgroup algorithm after k reflections, Q_k = H_N⋯H_{N−k+1}, makes Q_k e₁, …, Q_k e_k a uniform
  k-frame (the remaining factor fixes e₁, …, e_k). This is exact for the fixed frame (e₁, …, e_k) only. For an
  arbitrary fixed pair (y, Δ) the law of (Q_k y, Q_k Δ) is not uniform, because y has components on which the dropped
  factor would have acted (librarian's remark). The truncation therefore serves per-item random frames, not one
  rotation shared by every item.
- A compact subgroup K ⊂ SO(n) acting transitively on the Stiefel manifold V₂(ℝⁿ) gives an exactly uniform pair law
  under its own Haar measure, for every input pair. Besides SO(n), the transitive actions on V₂ are, as I recall the
  classification (Montgomery–Samelson, Borel, Onishchik), G₂ on V₂(ℝ⁷) and Spin(7) on V₂(ℝ⁸) only. [cited,
  unchecked.] So no infinite family of smaller groups does it.
- I found no construction, group or not, with fewer than dim SO(n) random parameters whose pair law is exactly
  uniform for every input pair, and no lower bound saying one cannot exist. A measure μ with μ·F uniform on V₂ for
  every frame F is a one-step exact sampler from every start on V₂; dimension counting needs its support to have
  dimension at least 2n − 3, not n(n − 1)/2, so the question is open as far as I found.

**Summary for item 5.** Established: exact Haar by Householder products (O(n²) to apply, O(n²) to generate), sign
correction of QR. Not done: an exact pair-uniform rotation with fewer parameters or lower application cost, except
the exceptional groups at n = 7, 8.

## 6. Dithered quantisation

In this section Δ is the quantiser step, as in the sources; the query weight of chapter 3 is written Δ_q.

**Schuchman (1964), "Dither signals and their effect on quantization noise" (IEEE Trans. Commun. Technol. 12(4)
162–165); Sripad and Snyder (1977, IEEE Trans. ASSP 25(5) 442–448); Gray and Stockham (1993), "Dithered quantizers"
(IEEE Trans. Inf. Theory 39(3) 805–812).** As restated by a 2018 paper on estimation from dithered measurements
(arXiv:1811.06856, §II, read): for a uniform quantiser q of step Δ, a dither D independent of the input X, output
Y = q(X + D) − D and error W = Y − X, Schuchman's condition is that the dither's characteristic function vanishes at
every non-zero multiple of 2π/Δ: M_D(j·2πℓ/Δ) = 0 for ℓ ∈ ℤ∖{0}. "As long as the quantizer has a sufficient number
of levels so that it does not overload", the condition is necessary and sufficient for the inputs to be independent
of the errors, with the errors i.i.d. uniform on [−Δ/2, Δ/2] when the dithers are i.i.d. [Sripad–Snyder and
Gray–Stockham cited for this; neither original opened: cited, unchecked.] Uniform dither on [−Δ/2, Δ/2] satisfies
the condition (M_D(ju) = sin(uΔ/2)/(uΔ/2)).

Hypotheses, stated exactly as the theorem needs them: the dither is independent of the input; the quantiser is
uniform with step Δ and does not overload, i.e. its range covers every value X + D can take; for independence across
coordinates the dithers are independent across coordinates. For a fixed input x (chosen without knowledge of the
dither) and D uniform on one cell, W = q(x + D) − D − x is exactly uniform on [−Δ/2, Δ/2] for every x; no
distributional assumption on the input is needed.

**Zamir and Feder (1992), "On universal quantization by randomized uniform/lattice quantizers" (IEEE Trans. Inf.
Theory 38(2) 428–436).** For a lattice quantiser Q_Λ and dither U uniform on the fundamental (Voronoi) cell V₀, the
error Q_Λ(x + U) − U − x is uniform on −V₀ and independent of x, for every x; with a nested pair Λ_c ⊂ Λ_f and modulo
reduction ("crypto lemma") the same holds for the finite codebook. [cited, unchecked: standard, as in Zamir's book
"Lattice Coding for Signals and Networks" (2014).] Ordentlich and Polyanskiy's matrix-multiplication codes use randomly
rotated, dithered nested lattices, in expectation for every input pair (library `theory.md`).

**Nested and multi-resolution versions.** Frank-Dayan and Zamir, "Dithered lattice-based quantizers for multiple
descriptions" (IEEE Trans. Inf. Theory 2002): successive refinement built from entropy-coded dithered lattice
quantisers loses an extra half bit at any lattice dimension; "dependent dithering" is discussed. arXiv:cs/0504003
(sequential dithered quantisation) uses independent subtractive dithers per stage. [search summaries only; not
opened.] I found no statement of the following, which follows from the theorem (librarian's remark): if every level's
grid is a refinement of the same dithered uniform grid (steps Δ/2^k with the same offset D), then D modulo each
finer step is uniform on that step's cell, so each level's error is exactly uniform and independent of the input,
level by level; the levels' errors are dependent on one another, which a per-level guarantee does not need.

**Shared randomness and dither in sketching and quantisation with guarantees.**
- Ben-Basat, Mitzenmacher, Vargaftik, "How to send a real number using a single bit (and some shared randomness)"
  (ICALP 2021; arXiv:2010.02331): worst-case variance lower bounds with unrestricted shared randomness and
  near-optimal schemes with a few shared bits. [abstract.] Variance, not tails.
- Suresh, Sun, Ro, Yu, "Correlated quantization for distributed mean estimation and optimization" (ICML 2022;
  arXiv:2203.04925): correlated (shared-randomness) quantisers whose error depends on the data's mean deviation.
  [abstract.] Variance.
- Datar, Immorlica, Indyk, Mirrokni (2004, p-stable LSH) use a random offset b uniform on [0, w): ⌊(a·x + b)/w⌋; the
  collision probability is computed exactly for Gaussian a. [cited, unchecked.]
- DASHQuant (§1): one HD plus dither uniform in quantile space; unbiased, MSE bounds uniform over d and x. The
  pairwise-independent follow-up (arXiv:2608.02564): the same with two dithers. Second moments only.
- RaBitQ (Gao and Long, SIGMOD 2024) and its extension (Gao et al. 2025) are cited by DASHQuant as using dithering
  with an analysis that assumes a truly random rotation. [cited, unchecked.]

**A consequence relevant to the question (librarian's remark, not from a source).** Subtractive dither gives an exact
error law that holds for every input without any rotation: with i.i.d. uniform dithers independent of the input and
of the rotation, and a step and range chosen from the item itself so that it never overloads, the error of the
reconstructed block is i.i.d. uniform on [−Δ/2, Δ/2]ⁿ for every fixed item, whatever orthogonal map (Haar, HD, or the
identity) was applied first. The error of the inner product with a fixed query weight is (RΔ_q)·W, given R a
weighted sum of independent uniforms with known weights, whose exact tail is computable at query time and is
sub-Gaussian with variance proxy ‖Δ_q‖²·Δ²/12 whatever R is (E e^(λU) = sinh(λa)/(λa) ≤ e^(λ²a²/6)
for U uniform on [−a, a]). The rotation then matters only for rate (flattening, to keep the range small), where an
O(n log n) structured map is enough, and not for the guarantee. The step Δ must not depend on the dither. The price is
the rate of a uniform quantiser with range set by ‖x‖_∞ instead of a Lloyd–Max code on a Gaussianised coordinate;
I found no paper that states this trade or a tail bound of this kind for search.

**Summary for item 6.** Established: Schuchman's condition and its necessity and sufficiency for uniform, input-
independent error under no overload; the lattice version. Empirical only: nothing specific. Not done, as far as I
found: an extreme-tail guarantee for inner-product estimation from shared subtractive dither in vector search; the
published dithered quantisers with structured rotations (DASHQuant and its follow-up) prove second-moment bounds only.

## 7. Pseudo-random generators for the rotation

**What is rigorous.**
- **A floor from finiteness (Low 2009, Theorem 1.5, §4 above).** A distribution with S equally likely elements gives
  any non-constant test probability at least 1/S. A rotation drawn from a 64-bit seed has S ≤ 2⁶⁴, so a claimed failure
  probability below 2⁻⁶⁴ ≈ 5.4·10⁻²⁰ is either 0 or at least 2⁻⁶⁴ for that construction; it cannot be the Haar value.
  This matches chapter 3's remark that the per-event budget 2.5·10⁻¹⁹ is about 2⁻⁶²: the budget sits four times above
  the 64-bit floor (my arithmetic).
- **Unconditional derandomisation of tail statements.** Kane, Meka, Nelson, "Almost optimal explicit
  Johnson–Lindenstrauss families" (APPROX/RANDOM 2011): an explicit JL family with failure δ, target dimension
  O(ε⁻² log(1/δ)) and seed length O(log d + log(1/δ)·log(log(1/δ)/ε)). [search summary; paper not opened.] Kothari
  and Meka (STOC 2015; arXiv:1411.6299): an explicit PRG fooling spherical caps (halfspaces on the uniform sphere) to
  additive error ε with seed O(log n + log(1/ε)·log log(1/ε)) [abstract]; O'Donnell–Servedio–Paredes report an error
  in the orthogonal-design construction it uses (§4). These show that seed lengths of a few hundred bits can carry
  provable additive errors of 10⁻²⁰ for linear threshold tests; none covers a quantised code of a rotated vector
  paired with a second rotated vector.
- **Computational (conditional) statements.** If the generator is a cryptographic PRG and the failure event for a
  fixed (item, query) pair is decidable in time T from the seed's output, the failure probability differs from the
  truly random one by at most the generator's distinguishing advantage at time T. This is the standard definition
  (textbook, cited, unchecked) and is conditional on the generator's security; quantitatively it is an assumption on
  concrete security. Vaikuntanathan and Zamir ("Improving algorithmic efficiency using cryptography",
  arXiv:2502.13065, 2025) conjecture that Kac's walk with n·polylog(n) steps is computationally indistinguishable from
  Haar, and prove that any law TV- or Wasserstein-close to Haar cannot admit sub-quadratic matrix–vector products
  [via Pillai–Smith–Vaikuntanathan §1 and a search summary]. Pillai–Smith–Vaikuntanathan prove the conjecture only
  against low-degree polynomial tests (§2).

**Summary for item 7.** Established: the 1/S floor; unconditional PRGs with additive error ε at seed cost
O(log(1/ε)·log log(1/ε)) for halfspaces and JL; the computational reduction under a cryptographic assumption. Not
done, as far as I found: a rigorous statement about a pseudo-random Haar rotation (Gaussian sampling from a
counter-based generator, then QR or Householder) at failure levels near 2⁻⁶²; the floating-point Gaussian sampler
is itself discrete, which chapter 3 already treats as a model assumption.

## 8. Corrections to the brief

1. **Andoni et al. 2015 prove nothing about HD rounds.** The paper states it does not know how to prove that
   HD₃HD₂HD₁ performs as well as a random rotation; "three rounds are exactly equivalent … when d tends to infinity"
   and "two are not sufficient" are both empirical remarks. The proved fast cross-polytope result is Kennedy and Ward
   (2016), for a different construction (FJLT plus a discrete pseudo-rotation).
2. **Choromanski, Rowland and Weller (NeurIPS 2017)** prove exact MSE formulas and MSE comparisons, not tail bounds;
   their improvement theorems for the angular kernel are for Gaussian-orthogonal matrices, not for HD products. The
   Kac-walk (Givens) variant in that line is Choromanski, Rowland, Chen and Weller (ICML 2019).
3. **Fastfood is not a pure sign–Hadamard product.** It inserts a Gaussian diagonal and a permutation, and its tail
   bound comes from Gaussian concentration on that diagonal.
4. **Pillai and Smith** have three relevant papers: the sphere (TV mixing between ½n log n and 200 n log n, explicit),
   SO(n) 2018 (between C₁n² and C₂n⁴ log n), and SO(n) 2026 (O(n² log n), no explicit constant). None gives a
   probability near 10⁻¹⁹; all are TV at the ¼ threshold. The k-column (Stiefel) result, which is the one that matches
   pairs, is Pillai, Smith and Vaikuntanathan (2026), in Wasserstein distance only.
5. **Carlen, Carvalho and Loss** determine the gap exactly: ½·(N + 2)/(N − 1) for the generator N(Q − I) on the
   sphere. A gap controls only L² starts, and the Givens walk from a point is never in L² on SO(N) (Hough and Jiang).
6. **Diaconis and Shahshahani's relevant contributions** here are the subgroup algorithm (1987) and the character
   method; the random-reflections cut-off is Porod's (½N log N), Rosenthal's paper is fixed-angle rotations in a
   uniformly random plane with the upper bound only at θ = π, and the upper bound for all angles plus an L^∞ cut-off
   is Hough and Jiang.
7. **Parker's butterflies are not orthogonal in general**; the orthogonal "Haar-butterfly" class is Trogdon's, and its
   simple form is Haar measure on an abelian log₂N-dimensional torus subgroup, far from uniform on the sphere.
8. **Low's large-deviation theorem is for unitary designs**; a transfer to orthogonal designs is plausible but is not
   stated there. Its exact-design bound loses a factor e in the exponent, so it needs design degree near 2·ln(1/δ)
   times the test function's degree.
9. **The dither theorem's hypotheses**: Schuchman's condition (the dither's characteristic function vanishes at all
   non-zero multiples of 2π/Δ) is necessary and sufficient for input-independent uniform error only under no overload;
   the necessity-and-sufficiency statement is attributed to Sripad and Snyder (1977) and Gray and Stockham (1993), not
   to Schuchman alone. For a fixed input the uniform dither on one cell gives the exact law with no assumption on the
   input's distribution.
10. **"Haar is a model of a pseudo-random rotation from a 64-bit seed"** (chapter 3) can be sharpened: by Low's
    Theorem 1.5 any law with S equally likely values cannot give a non-trivial probability below 1/S, so with
    S = 2⁶⁴ the floor is 5.4·10⁻²⁰, a factor of about 4.6 under the per-event budget of 2.5·10⁻¹⁹ (my arithmetic).

## 9. Sources I could not open

The original FJLT paper (Ailon–Chazelle); Bentkus–Dzindzalieta (2015); Porod (1996, publisher page refused the
download); Rosenthal (1994); Stewart (1980); Diaconis–Shahshahani (1987); Parker (1995); Schuchman (1964); Sripad–
Snyder (1977); Gray–Stockham (1993); Zamir–Feder (1992); Frank-Dayan–Zamir (2002); Brandão–Harrow–Horodecki (2016);
Kane–Meka–Nelson (2011); Jones–Osipov–Rokhlin (2011); Vaikuntanathan–Zamir (2025, abstract via search only);
Datar et al. (2004); RaBitQ. Oliveira (2009) and Pillai–Smith (2017, 2018) were read through their abstracts and
through statements quoted in later papers. Every statement drawn from these is tagged above.
