# Library report: prior art for the three theory results

Stage 0, written 21 November 2026 for the PI. Scope: whether the planned results of the floors and confidence
tracks are already published, and a check of the citations the brief gave from memory. Notation is that of
`00-notation.md`; in particular n is the dimension and F = Σᵢ √γᵢ.

How each source was read:

- **[raw]**: I read the source's own text (a PDF whose text I extracted, or the HTML page) and quote it directly.
- **[text, §x]**: I read the paper's full text through the fetch tool, which summarises and transcribes. Formulas
  marked as transcribed were asked for verbatim, but the wording is one step removed.
- **[abstract]**: the abstract only.
- **[summary]**: a search-engine or secondary summary only.
- **[cited, unchecked]**: from memory, or I could not open the source.

Remarks marked **[my arithmetic]** are my own reasoning from the quoted statements, not taken from any source. I ran
no computation.

## Summary of verdicts

| Planned result | Verdict | Closest published statement |
|---|---|---|
| 1. Transport-entropy floor for deterministic certificates | **Partly published.** The technique is standard and the asymptotic answer is published. The finite-n bound with this 1-D inequality was not found. | Ingber, Courtade and Weissman (2015), Theorem 1, and Wu, Wang and Flierl, Theorem 1: the identification rate for "no false negative" queries on compressed Gaussian data. Their formula is the §6 "full floor" conjecture, asymptotically. Tensorisation: Gozlan and Léonard, Proposition 1.8. |
| 2. The geometric-mean metric as optimal transport | **Published as mathematics, apparently new as an application.** | Bhatia, Jain and Lim (2019), Theorem 2, eq. (16): F(A,B) = min over X > 0 of √(tr(AX)·tr(BX⁻¹)), attained at A⁻¹#B. This is Proposition 3 of `groups/REPORT.md` verbatim. Zadeh, Hosseini and Sra (2016) solve the same problem in metric learning. No search, quantisation or sketching paper found that uses it. |
| 3. Lower bound for randomised certificates | **Partly published.** The orders are known in both regimes, and so is the unavoidable union. The quantile constant q_d(δ)·(4^R − 1)^(−½) was not found. | Jayram and Woodruff (2013): Ω(ε⁻² log 1/δ). Molinaro, Woodruff and Yaroslavtsev (2013): the union bound over k copies is optimal. Alon and Klartag (2017): Θ(nk log(1/ε)) bits for k ≤ log n, the regime of many bits per dimension. Zandieh et al. (TurboQuant, 2025), Theorem 3: expected distortion ≥ 4^(−b) (MSE) and ≥ ‖y‖²4^(−b)/d (inner product), proved on the sphere. That is the variance analogue with a constant. |

---

## 1. The floor for deterministic certificates

**Planned statement.** E h ≥ min{Σᵢ √γᵢ sᵢ : Σᵢ [½(sᵢ² − 1) − ln sᵢ] ≤ H}. The proof uses a Knothe–Rosenblatt coupling,
the chain rule for relative entropy, and a 1-D transport-entropy inequality whose extremal law is a centred
Gaussian. The summand ½(s² − 1) − ln s is D(N(0, s²) ‖ N(0, 1)).

**Verdict: partly published.** The proof technique is the standard tensorisation of transport-entropy inequalities. The
answer in the high-dimensional limit is published for quadratic similarity queries with no false negatives, a setting
the earlier survey missed. I found no finite-n lower bound on the expected support function in this form. Nor did I
find the 1-D inequality: among laws ν with D(ν ‖ N(0, γ)) ≤ η, the least maximal covariance with N(0, 1) is
attained by N(0, γs²).

### 1.1 Closest sources

**Ingber, Courtade, Weissman, "Compression for Quadratic Similarity Queries", IEEE Trans. Inf. Theory 61(5), 2015
[text, §II–III].** The setting is a database of i.i.d. length-n sequences X and an independent query Y, with
d(x, y) = (1/n)‖x − y‖². A rate-R signature T(x), with 2^(nR) values, must answer "maybe" whenever d ≤ D:
"false negatives are not permitted". The rate is achievable if Pr[maybe] → 0 as n → ∞. Theorem 1, for
P_X = N(μ, σ_X²) and P_Y = N(μ, σ_Y²), transcribed:

R_ID(D) = 0 for D < (σ_X − σ_Y)²; log[2σ_Xσ_Y / (σ_X² + σ_Y² − D)] for (σ_X − σ_Y)² ≤ D < σ_X² + σ_Y²; ∞ beyond.

Logarithms are base 2 [raw quote via fetch: "log(·) denotes the base-2 logarithm"]. Theorem 4: Gaussian P_X and
P_Y "demand the largest identification rate for given variances". The converse is geometric. It rests on "the
isoperimetric inequality on the surface of the hypersphere", followed by a probability-averaging lemma (Lemma 4).
Achievability covers the typical shell with spherical caps (Lemma 2).

*Translation [my arithmetic].* With norms concentrated, d ≤ D is the same as x·y/n ≥ ρ, where
ρ = (σ_X² + σ_Y² − D)/2. So R_ID = log₂(σ_Xσ_Y/ρ), which means the least certifiable correlation per coordinate is
ρ = σ_Xσ_Y·2^(−R). With Δ ~ N(0, 1) and r′ ~ N(0, γ) per coordinate, this is √γ·2^(−R) = √min(γ, θ), where
½log₂(γ/θ) = R. At R = 0 it is √γ, the Cauchy–Schwarz value. A vanishing "maybe" probability means the cell's support
function stays below nρ at a typical independent query. That is a statement about a typical half-width, not about
E h.

**Wu, Wang, Flierl, "Computing Similarity Queries for Correlated Gaussian Sources", arXiv:2001.07941 [text,
§III–IV; venue not found].** This paper extends ICW to an M-dimensional Gaussian source with covariance C_M, query
and database from the same law, through the KLT. Theorem 1 gives eqs. (23)–(24), transcribed:

R_ID = (1/M) Σₘ max(0, log(ξ⁽ᵐ⁾/τ)),  D_ID = (1/M) Σₘ max(0, 2(ξ⁽ᵐ⁾ − τ)),  τ ∈ (0, ξ_max].

Here ξ⁽ᵐ⁾ are the eigenvalues of C_M. Section IV calls the allocation a reverse water-filling and extends it to
stationary sources through Szegő's theorem. The log base is not stated; the derivation suggests base 2.

*Translation [my arithmetic].* The setting has Σ_Q = Σ_R = C, so γᵢ = ξᵢ² and √γᵢ = ξᵢ. The certified correlation
is Σ min(ξᵢ, τ) at a rate of Σ ½log₂⁺(γᵢ/τ²). With θ = τ² this is **exactly the "full floor" conjecture of
`CONSOLIDATED_PROOFS.md` §6**: Σᵢ √min(γᵢ, θ), with Σᵢ ½log₂⁺(γᵢ/θ) = B. So the conjectured floor is already the
asymptotic, typical-query answer for commuting equal covariances. Three caveats limit this:

- The result is asymptotic in blocks; it is not finite n.
- It holds for Σ_Q = Σ_R only. ICW covers σ_X ≠ σ_Y only for scalars.
- The paper's converse is thin. As transcribed, Lemma 1 says the overall "maybe" probability vanishes if and only if
  that of some single component does. The water-filling is then a KKT optimisation over the component rates. I would
  not rely on this converse without reading it in full, since part of the paper went to Asilomar 2018 and may not
  have had a full journal review.

**What follows for the planned result [my arithmetic].** Take the isotropic case at high rate. The planned bound's
sᵢ solves ½(s² − 1) − ln s = R ln 2, which gives s → e^(−½)·2^(−R). Theorem 1, Corollary 1 at k = n gives
κ_n√γ·2^(−R), with κ_n/n → 1, and ICW say that this is achievable asymptotically. So at high rate the planned bound
sits a factor √e ≈ 1.65 below both Theorem 1 and the published asymptotic answer. At low rate the planned s is about
1 − √(R ln 2), while the asymptotic answer is 2^(−R) ≈ 1 − R ln 2. The loss has a clear source. The extremal
centred Gaussians cannot be the conditional laws of every cell at once, because the cell laws must average back to
N(0, Γ). So the planned floor is valid but loose in both limits. Its value would have to come from anisotropic spectra
at moderate rate. The PI may prefer to attack the conjecture by adapting the ICW/WWF converse, which uses spherical
isoperimetry on the typical shell, to finite n.

**Tensorisation: Gozlan and Léonard, "Transport Inequalities. A Survey", Markov Process. Related Fields 16, 2010
[text, §1].**

- Theorem 1.5 proves T₂ for the Gaussian in 1-D. It writes ν = T#μ with T monotone and splits H(ν|μ) into
  ∫(V(T) − V − V′(T − x))dμ + ∫(T′ − 1 − log T′)dμ, eq. (7).
- Proposition 1.8 is the tensorisation with separate costs c₁ ⊕ c₂ and separate convex α₁, α₂, giving
  α₁□α₂(𝒯_{c₁⊕c₂}(ν, μ₁⊗μ₂)) ≤ H(ν|μ₁⊗μ₂).
- Its proof disintegrates ν along the first coordinate and bounds the cost by
  𝒯_{c₁}(ν₁, μ₁) + ∫𝒯_{c₂}(ν₂^{x₁}, μ₂)dν₁, eq. (10). It applies the chain rule
  H(ν|μ₁⊗μ₂) = H(ν₁|μ₁) + ∫H(ν₂^{x₁}|μ₂)dν₁, eq. (11), "suggested by Marton and Talagrand".

The inf-convolution α₁□…□αₙ is precisely the min over allocations Σ Hᵢ ≤ H in the planned statement. So the
planned proof is Proposition 1.8 applied to a new 1-D inequality for the bilinear cost −√γᵢ·xy. One point to check:
Gozlan–Léonard take costs ≥ 0 and α convex increasing on [0, ∞). The bilinear cost is not of that form, so the PI's
own induction is needed rather than a citation. The survey does not name Knothe. Talagrand's original paper (GAFA
6, 1996) I did not open [cited, unchecked].

**Information-constrained optimal transport.** Bai, Wu and Özgür (IEEE Trans. Inf. Theory 69(4), 2023;
arXiv:2008.10249) [abstract] minimise transport cost subject to a mutual-information constraint on the coupling,
"a strengthening and generalization of Talagrand's ... inequality". Zou, Chen, Özgür and Wang (arXiv:2610.25041, 18
October 2026) [abstract] ask "How much mutual information is needed to achieve a target correlation between two
copies of a prescribed distribution?" They prove the Gaussian is the worst case. Both are the nearest neighbours in
spirit, since they treat correlation against information with Gaussian extremality. Neither is the planned
quantity: the planned bound needs the least maximal covariance between a fresh N(0, I) and a cell law under a
relative-entropy budget.

**Ordentlich and Polyanskiy, "Optimal Quantization for Matrix Multiplication", arXiv:2410.13780v3 [text, §I–III].**
The closest result on inner-product rate–distortion. It concerns the MSE of an estimate, not a sound interval. It is
covered under result 3 below.

**Not found.** I searched for the following and found none of them:

- a lower bound on Gaussian mean width, or E h_K(g), in terms of Gaussian measure with a centred Gaussian or centred
  ball as extremal (a "Gaussian Urysohn");
- "reverse water-filling for support functions";
- a Shannon lower bound in which Urysohn's inequality replaces the second-moment step;
- rate–distortion with sup-over-the-cell distortion for Gaussian inner products, beyond ICW/WWF.

The functional Urysohn literature (Klartag–Milman, and arXiv:1210.4325) bounds mean width by volume, not by Gaussian
measure [summary].

### 1.2 What is new in result 1

- **New:** a finite-n bound on E h, valid for every n and every rate, with general non-commuting Σ_Q and Σ_R.
- **Possibly new:** the 1-D inequality (centred Gaussian extremal for least maximal covariance under a KL budget).
- **Not new:** the tensorisation (Marton, Talagrand, Gozlan–Léonard Prop. 1.8). Also not new: the asymptotic value of
  the full floor in the commuting equal-covariance case, which is ICW and WWF.
- **Missed by the earlier survey:** `lit/REPORT.md` item 4 called the floor family "apparently new" and named no
  information-theory precedent. The ICW line is a precedent for Theorems 1–3 as well. Theorem 1 at k = n, isotropic,
  is the finite-n, expected-width analogue of the ICW converse.

---

## 2. The geometric-mean metric as optimal transport

**Planned statement.** The tightest Cauchy–Schwarz certificate |Δ·r| ≤ ‖Δ‖_M‖r‖_{M⁻¹} uses M = Σ_Q⁻¹ # Σ_R, with
value F = Σᵢ √γᵢ. The brief makes five claims about it:

- it is Kantorovich duality between N(0, Σ_Q) and N(0, Σ_R);
- M is the optimal transport map;
- F is the Bures fidelity;
- no norm beats this ellipsoid;
- for non-Gaussian laws, the best certificate of the form φ(Δ) + φ*(r) is the Brenier potential, and the gain is the
  Gelbrich gap.

**Verdict: published as mathematics, apparently new as an application.** Every named fact checks out, with the two
precision corrections in §2.2. The central identity is a stated theorem in Bhatia–Jain–Lim.

### 2.1 Each named fact against its source

- **Fidelity.** Bhatia, Jain, Lim, "On the Bures–Wasserstein distance between positive definite matrices", Expo.
  Math. 37(2), 2019 (arXiv:1712.01504) [text, §1–3]. Eq. (1): d(A,B) = [tr A + tr B − 2 tr(A^½BA^½)^½]^½. Eq. (6):
  F(A,B) = tr(A^½BA^½)^½, "the fidelity". So F(Σ_Q, Σ_R) = Σᵢ √γᵢ. **Correct.**
- **The certificate's optimisation.** Bhatia–Jain–Lim, Theorem 2: (15) F(A,B) = min over X > 0 of ½tr(AX + BX⁻¹),
  with minimiser X₀ = A⁻¹#B. (16) F(A,B) = min over X > 0 of √(tr(AX)·tr(BX⁻¹)). With A = Σ_Q, B = Σ_R and X = M,
  (16) is Proposition 3 of `groups/REPORT.md`, including the minimiser. **Published.**
- **The optimal map.** Bhatia–Jain–Lim §3, for zero-mean vectors with covariances A and B: the optimal transport map
  "from A to B" is T = A⁻¹#B, with TAT = B. They credit Olkin–Pukelsheim and Dowson–Landau. Knott and Smith, J.
  Optim. Theory Appl. 43, 1984, give the normal-case map [summary]. So M = Σ_Q⁻¹#Σ_R is the Brenier map from
  N(0, Σ_Q) to N(0, Σ_R), and its potential is φ(Δ) = ½ΔᵀMΔ. **Correct.**
- **Dowson and Landau**, "The Fréchet distance between multivariate normal distributions", J. Multivariate Anal.
  12(3):450–455, 1982 [summary]: d² = ‖μ_X − μ_Y‖² + tr(Σ_X + Σ_Y − 2(Σ_XΣ_Y)^½). **Olkin and Pukelsheim**, "The
  distance between two random vectors with given dispersion matrices", Linear Algebra Appl. 48:257–263, 1982
  [summary]: the minimising cross-covariance. The two were found independently, and Olkin–Pukelsheim mention
  Dowson–Landau at proof stage. **Citations correct.** Givens and Shortt, Michigan Math. J. 31, 1984, is the third
  usual source [cited, unchecked].
- **Gelbrich**, "On a formula for the L2 Wasserstein metric between measures on Euclidean and Hilbert spaces", Math.
  Nachr. 147:185–203, 1990 [summary only; full text not opened]. For any laws with given means and covariances, W₂ is
  at least the Gaussian (Bures) value, with equality for Gaussians and some elliptical families. **Citation
  correct.**
- **Brenier**, "Polar factorization and monotone rearrangement of vector-valued functions", Comm. Pure Appl. Math.
  44(4):375–417, 1991 [summary]. For quadratic cost the optimal map is the gradient of a convex function. The duality
  max E[X·Y] = min (∫φ dμ + ∫φ* dν), attained at the Brenier potential, is standard (Villani, *Topics in Optimal
  Transportation*, 2003, ch. 2) [cited, unchecked].
- **"No norm beats the ellipsoid" [my arithmetic, from the above].** Take any norm ‖·‖ with dual ‖·‖_*, and
  φ = (λ/2)‖·‖², so that φ* = (1/2λ)‖·‖_*². Fenchel–Young gives Δ·r ≤ φ(Δ) + φ*(r). Take expectations under the optimal
  coupling and optimise λ. The result is √(E‖Δ‖²·E‖r‖_*²) ≥ max-cov = F for the Gaussians. Weak duality is enough,
  and equality holds at the M-ellipsoid. **Correct, in the second-moment criterion of Proposition 3.**

### 2.2 Precision corrections

1. **The criterion.** Proposition 3 minimises the product of second moments, tr(MΣ_Q)·tr(M⁻¹Σ_R). It does not
   minimise the mean certificate E‖Δ‖_M·E‖r‖_{M⁻¹}. The mean is below F by Jensen, and its minimiser need not be the
   geometric mean. "The tightest Cauchy–Schwarz certificate ... with value Σ√γᵢ" holds for the second-moment
   criterion only.
2. **The "Gelbrich gap".** Suppose the laws are centred and non-Gaussian, with the same covariances. The best
   φ(Δ) + φ*(r) in expectation is the maximal covariance C* = ½(E‖Δ‖² + E‖r‖² − W₂²). Gelbrich gives C* ≤ F. The
   gain over the ellipsoid is F − C* = ½(W₂² − G²), with G the Gelbrich (Bures) lower bound. So the gain is half the
   gap in squared distances, not the gap in distances, and it needs a means term when the means differ.
3. **One-sided versus two-sided.** φ + φ* bounds Δ·r from one side only. The two-sided |Δ·r| needs symmetric laws
   or a second potential.
4. **Pointwise comparison.** The product form ‖Δ‖_M‖r‖_{M⁻¹} equals min over λ of ½(λ‖Δ‖²_M + ‖r‖²_{M⁻¹}/λ) pair by
   pair. It is therefore never larger than any fixed-λ quadratic φ + φ*. Comparing the Brenier certificate for
   non-Gaussian laws with the Cauchy–Schwarz ellipsoid must use the same form on both sides.

### 2.3 Prior use in search, quantisation or sketching

- **Zadeh, Hosseini, Sra, "Geometric Mean Metric Learning", ICML 2016, PMLR 48:2464–2471 [text, §2].** This paper
  solves min over A ≻ 0 of tr(AS) + tr(A⁻¹D), eq. (8). The Riccati condition is ASA = D, eq. (12), and the solution
  is A = S⁻¹#D. It is the same variational problem as Proposition 3, used to fit a Mahalanobis metric from similar
  and dissimilar pairs. It makes no inner-product bound and no transport reading.
- **QUIP**: Guo, Kumar, Choromanski, Simcha, AISTATS 2016 (arXiv:1509.01469) [abstract + summary]. It minimises the
  expected inner-product error, which weights the error by the (non-centred) query covariance, a Mahalanobis metric
  in Σ_Q. It gives concentration results under "mild assumptions". There is no geometric mean and no certificate.
- **Anisotropic vector quantisation**: Guo, Sun, Lindgren, Geng, Simcha, Chern, Kumar, arXiv:1908.10396 (ICML 2020)
  [abstract]. It penalises the residual parallel to the datapoint more than the orthogonal part. There is no query
  covariance and no geometric mean.
- **LeanVec / LeanVec-OOD**: Tepper, Bhati, Aguerrebere, Hildebrand, Willke, arXiv:2312.16335 [text, §3; venue not
  confirmed]. Eq. (7) is min over A, B ∈ St(D, d) of ‖QᵀAᵀBX − QᵀX‖²_F, and eq. (8) writes it with K_Q and K_X. The
  "Eigen-search" sets P to the leading eigenvectors of K_β = ((1−β)/m)K_Q + (β/n)K_X. The fetch found no mention of a
  geometric mean, of K_Q^½K_XK_Q^½ or of optimal transport. Note that the leading eigenvectors of K_Q^½K_XK_Q^½
  (the γᵢ basis) are not those of any convex combination of K_Q and K_X unless the two commute [my arithmetic]. GleanVec
  (arXiv:2410.22347) adds "LeanVec-Sphering" [summary].
- **Bures–Wasserstein with sketching or vector search:** none found [search summary].

**What is new in result 2:** the reading of a Cauchy–Schwarz certificate for inner-product search as the quadratic
case of Kantorovich duality, and its use to pick certificate metrics. The mathematics itself, Proposition 3
included, should be cited to Bhatia–Jain–Lim Theorem 2 and not presented as new.

---

## 3. A lower bound for randomised certificates

**Planned statement.** Store a d-dimensional block at R bits per dimension so that every fixed pair is estimated
within half-width w with failure probability δ. Then w ≳ q_d(δ)·‖y‖‖Δ‖·(4^R − 1)^(−½), where q_d(δ) is the δ-quantile
of a sphere coordinate. And for N items decided one by one, the union over N cannot be avoided.

**Verdict: partly published.**

- Published: the order in the low-precision regime (bits ∝ log(1/δ)/w²); the necessity of the union (log N);
  the order in the high-precision regime (bits per dimension ∝ log(1/w)) for deterministic all-pairs sketches.
- Published with a constant: an expected-squared-error (variance) analogue, TurboQuant Theorem 3.
- Not found: a lower bound with the exact sphere quantile q_d(δ) and the factor (4^R − 1)^(−½).

### 3.1 Exact statements

| Source | Statement | Regime | Constant? | Read |
|---|---|---|---|---|
| Alon, Klartag, "Optimal compression of approximate inner products and dimension reduction", FOCS 2017, arXiv:1610.00239 | n points of norm ≤ 1 in ℝᵏ; a sketch recovers every pairwise inner product or squared distance to additive ε; f(n,k,ε) is the least total number of bits. For 1/n^0.49 ≤ ε ≤ 0.1 (Theorem 1.1): Θ(n log n/ε²) for log n/ε² ≤ k ≤ n; Θ(nk log(2 + log n/(ε²k))) for log n ≤ k ≤ log n/ε²; Θ(nk log(1/ε)) for k ≤ log n. The lower bound is a packing argument (Lemma 5.1). The randomised construction (Theorem 4.1) uses a union bound to make all pairs good with probability 1 − 2/n³. | Deterministic, all pairs. The third range is the regime of many bits per dimension (log(1/ε) per coordinate). | No: "we make no serious attempt to optimize the absolute constants" (§2). | Abstract [raw]; theorem [text, §1.1, §4, §5] |
| Jayram, Woodruff, "Optimal bounds for Johnson–Lindenstrauss transforms **and streaming problems** with subconstant error", SODA 2011; ACM TALG 9(3), 2013 | k = O(ε⁻² log 1/δ) dimensions suffice to approximate the norm of a fixed vector within 1 ± ε with probability 1 − δ, "and this bound on k is optimal up to a constant factor". This improves Alon's Ω(ε⁻² log(1/δ)/log(1/ε)). One-way communication lower bounds give streaming space Ω(ε⁻² log n log 1/δ) for ℓ_p estimation. | Per fixed vector, failure δ; distributional JL (dimension); bits via one-way communication; low precision. | Order only. | Abstract |
| Molinaro, Woodruff, Yaroslavtsev, "Beating the direct sum theorem in communication complexity with implications for sketching", SODA 2013 | "the 1-way communication required to solve k copies of f simultaneously with probability 2/3 is Ω(k·R_{1/k}(f))", so "the trivial upper bound of solving each of the k copies of f with probability 1 − O(1/k) and taking a union bound is optimal". Corollaries: oblivious JL dimension Ω(ε⁻² log(n/δ)); encodings of n vectors in [±M]^d for ℓ₁/ℓ₂ estimation need Ω(nε⁻² log(n/δ)(log d + log M)) bits; matrix sketches with the entrywise guarantee \|(ASSᵀB)ᵢⱼ − (AB)ᵢⱼ\| ≤ ε‖Aᵢ‖‖Bⱼ‖ need dimension Ω(ε⁻² log(n/δ)). | Union over N, low precision. | Order only. | Abstract and introduction [raw] |
| Larsen, Nelson, "Optimality of the Johnson–Lindenstrauss lemma", FOCS 2017, arXiv:1609.02094 | For 1/min{n,d}^0.4999 < ε < 1 there are n points such that any embedding into ℝᵐ (not only linear) preserving squared distances within 1 ± ε needs m = Ω(ε⁻² lg n). | Dimension, not bits; deterministic, all pairs. | Order only. | Abstract |
| Indyk, Wagner, "Near-optimal (Euclidean) metric compression", SODA 2017, arXiv:1609.06295; "Optimal (Euclidean) metric compression", SIAM J. Comput. 51(3):467–491, 2022, arXiv:2110.03152 | Sketches recover every pairwise distance within 1 + ε. The SODA bound was O(ε⁻² log(1/ε)·log n + log log Φ) bits per point, tight up to log(1/ε). The journal version gives a tight O(ε⁻² log n + log log Φ). | Multiplicative, all pairs, low precision. | Order only. | Abstract; journal version via search summary |
| Indyk, Wagner, "Approximate nearest neighbors in limited space", COLT 2018 | (1 + ε)-ANN with O(ε⁻² n log n log(1/ε)) bits, for queries unseen at build; "nearly tight" lower bounds up to log(1/ε) for distances from query points to all data points. | Per query, all N items. | Order only. | Search summary |
| Ordentlich, Polyanskiy, "Optimal quantization for matrix multiplication", arXiv:2410.13780v3 (IEEE Trans. Inf. Theory per search) | For iid N(0,1) entries, E‖AᵀB − g‖²_F ≥ σ⁴·nab·Γ(R) (Theorem 2, eq. 7), and D^IP(R, N(0,1)) = Γ(R). Eq. (1), transcribed: Γ(R) = 2·2^(−2R) − 2^(−4R) for R ≥ R\*, and linear time-sharing 1 − (1 − Γ(R\*))R/R\* below it, with R\* ≈ 0.906 solving R = ½log₂(1 + 4R ln 2). For non-Gaussian P: Γ(R + D(P‖N(0,1))) ≤ D^IP(R, P) ≤ Γ(R). Proposition 1: deterministic quantisers fail on the sphere, with an inexplicit δ(R). Upper bounds use randomly rotated, dithered nested lattices, in expectation for every input pair. | Both sides quantised; MSE; iid identity covariance only (no Σ_A, Σ_B water-filling). | Yes, for MSE. | [text, §I–III] |
| Zandieh, Daliri, Hadian, Mirrokni, "TurboQuant", arXiv:2504.19874, 28 April 2025 | Theorem 3: for any randomised quantiser of bit-width b there is a unit x with D_mse ≥ 4^(−b), and a unit y with D_prod ≥ (1/d)·4^(−b) (general form ‖y‖²4^(−b)/d, §1.3). The proof uses the Shannon lower bound and Yao's minimax with inputs uniform on the sphere. It bounds "the expected distortion, rather than the worst-case error". Upper bounds: D_mse ≤ (√3π/2)·4^(−b) (Theorem 1) and D_prod ≤ (√3π²‖y‖²/d)·4^(−b) (Theorem 2). | Many bits per dimension, one-sided (only x stored), expected squared error. | Yes; the gap is ≈ 2.7. | [text, §1.3, §3.3] |
| Okajima, Yoshida, arXiv:2606.11780, 10 June 2026 | "With B bits per coordinate, perfect top-k retrieval requires Bd = Ω(k ln N)", and a precision threshold B\* = O(ln ln N) below which no dimension suffices. | Deterministic; the union over N appears as ln N. | Order only. | Abstract |
| Sphere covering and rate–distortion: Wyner, "Random packings and coverings of the unit n-sphere", Bell Syst. Tech. J., 1967; Sakrison, "A geometric treatment of the source encoding of a Gaussian random variable", IEEE Trans. Inf. Theory 14(3):481–486, 1968; Böröczky and Wintsche, "Covering the sphere by equal spherical balls", 2003; Dumer, "Covering spheres with spheres", 2007 | Wyner: upper and lower bounds on cap packing and covering numbers, with random coding for the upper bound. Sakrison: the Gaussian distortion–rate function is met by spherical block codes. Böröczky–Wintsche: caps of radius φ < π/2 cover with density ≤ n ln n + n ln ln n + 5n. Dumer: density ≤ ½ n ln n for φ < π/3. | The volume bound: covering by caps of angle φ needs at least about (1/sin φ)^(d−1) caps. | Constants up to polynomial factors in the count. | Summaries only |

**The planned rate factor [my arithmetic].** tan²φ = 1/(4^R − 1) is the same as sin φ = 2^(−R). So
(4^R − 1)^(−½) is the cap angle given by the volume (covering) bound at R bits per dimension. It is also
√(2^(−2R)/(1 − 2^(−2R))), the variance of the unbiased rescaled estimate behind the one-sided Gaussian
rate–distortion function 2^(−2R). This agrees with Lemma 8 of `CONSOLIDATED_PROOFS.md` (A = ‖y‖tan φ). TurboQuant's
Theorem 3 is the same bound in variance form, with 4^(−b) in place of 1/(4^b − 1) because a biased decoder is allowed.

### 3.2 Which regime each source covers

- **Bits growing as 1/w², with log(1/δ) or log N factors:** Jayram–Woodruff, Molinaro–Woodruff–Yaroslavtsev,
  Larsen–Nelson, Indyk–Wagner, and Alon–Klartag's first range. Orders only.
- **Bits per dimension growing as log(1/w):** Alon–Klartag's third range, Θ(nk log(1/ε)) for k ≤ log n,
  deterministic. TurboQuant (expected distortion, with a constant). Ordentlich–Polyanskiy (MSE, with a constant, both
  sides quantised). The sphere covering bounds.
- **With a failure probability δ in the high-precision regime, as a quantile with a constant:** nothing found. PQR-Search
  (below) uses exact quantiles as an upper bound only.

### 3.3 The concurrent preprint, re-checked

Loh and Lee, "Per-Query Recall Guarantees for Approximate Nearest Neighbor Search in Dynamic Vector Databases"
(PQR-Search), arXiv:2611.07579. v1 is dated 5 November 2026 and v2 11 November 2026. I read the abstract [raw] and v2
through the fetch tool on the HTML [text, introduction]. Its contributions list gives three lower or optimality
results, none on bits or constants:

- "in a probe model, a per-query guarantee must probe at least a 1 − δ^(1/(m+1)) fraction of the hidden cells in
  expectation", and "a deterministic rule all but m of them" (Theorems 7 and 8);
- under randomised SETH, "no data structure with polynomial preprocessing can give such guarantees in query time"
  for worst-case inputs with D = ω(log n) (Theorem 9, appendix);
- "the Markov step of the bound is tight; it is attained exactly only by a placement of exact duplicates with
  k = m + 1" (Proposition 1).

It does not claim the union over k or n is necessary. It does not claim its quantile is optimal. It does not cite
Alon–Klartag, Jayram–Woodruff, Larsen–Nelson or rate–distortion. These contribution statements were not in the
earlier survey's notes. The appendix proofs were not read.

### 3.4 Recent work on certified or exact search with quantised codes (2025–2026)

None of these supersedes the earlier surveys. Leads:

- **BBC**, arXiv:2604.01960 [summary]: large-k ANN that skips re-ranking when RaBitQ bounds settle membership.
  Probabilistic and approximate.
- **MRQ**, arXiv:2411.06158 (PVLDB 19) [summary]: already in `lit/REPORT.md`.
- **Xiao and Cao**, arXiv:2609.09854 (v2 11 November 2026) [abstract]: "A comparison changes only when its quantization
  error exceeds its exact margin", with "model-free certificates" from held-out blocks. This is a deterministic flip
  condition plus calibration, not a bound with a probability.
- **Ann, Lee, Oh, "Block-Sphere Vector Quantization"**, arXiv:2605.19972 [abstract]: improves on EDEN, RaBitQ and
  TurboQuant in expected distortion. No lower bound and no high-probability bound.
- **Per-query recall guarantees** [summary]: ConANN (VLDB 2025), conformal and averaged over queries; an HNSW variant
  with graph spanners, arXiv:2607.02338; DARTH+ (2026). PQR-Search classes conformal guarantees as weaker. None was
  opened.

---

## Bibliography

1. N. Alon, B. Klartag. Optimal compression of approximate inner products and dimension reduction. FOCS 2017. https://arxiv.org/abs/1610.00239 — abstract [raw]; Theorem 1.1 and §4–5 [text]
2. Y. Bai, X. Wu, A. Özgür. Information constrained optimal transport: from Talagrand, to Marton, to Cover. IEEE Trans. Inf. Theory 69(4), 2023. https://arxiv.org/abs/2008.10249 — abstract
3. R. Bhatia, T. Jain, Y. Lim. On the Bures–Wasserstein distance between positive definite matrices. Expo. Math. 37(2):165–191, 2019. https://arxiv.org/abs/1712.01504 — [text, eqs. (1), (6), (7), Theorems 1–2, §3]
4. K. Böröczky Jr., G. Wintsche. Covering the sphere by equal spherical balls. Discrete Comput. Geom. (Algorithms and Combinatorics 25), 2003. https://link.springer.com/doi/10.1007/978-3-642-55566-4_10 — summary
5. Y. Brenier. Polar factorization and monotone rearrangement of vector-valued functions. Comm. Pure Appl. Math. 44(4):375–417, 1991. https://www.ceremade.dauphine.fr/~carlier/Brenier91.pdf — summary
6. M. Burr, S. Gao, F. Knoll. Optimal bounds for Johnson–Lindenstrauss transformations. JMLR 19, 2018. https://www.jmlr.org/beta/papers/v19/18-264.html — abstract (a "precise asymptotic threshold" for the JL dimension; constants not seen)
7. D. C. Dowson, B. V. Landau. The Fréchet distance between multivariate normal distributions. J. Multivariate Anal. 12(3):450–455, 1982. https://ideas.repec.org/a/eee/jmvana/v12y1982i3p450-455.html — summary
8. I. Dumer. Covering spheres with spheres. Discrete Comput. Geom. 38, 2007 — [cited, unchecked]; density figure from a search summary
9. M. Gelbrich. On a formula for the L2 Wasserstein metric between measures on Euclidean and Hilbert spaces. Math. Nachr. 147:185–203, 1990. https://doi.org/10.1002/mana.19901470121 — summary [full text not opened]
10. N. Gozlan, C. Léonard. Transport inequalities. A survey. Markov Process. Related Fields 16:635–736, 2010. https://arxiv.org/abs/1003.3852 — [text, §1: Theorem 1.5, Propositions 1.8–1.9, eqs. (7), (9)–(11)]
11. R. Guo, S. Kumar, K. Choromanski, D. Simcha. Quantization based fast inner product search. AISTATS 2016. https://arxiv.org/abs/1509.01469 — abstract and summary
12. R. Guo, P. Sun, E. Lindgren, Q. Geng, D. Simcha, F. Chern, S. Kumar. Accelerating large-scale inference with anisotropic vector quantization. ICML 2020. https://arxiv.org/abs/1908.10396 — abstract
13. P. Indyk, T. Wagner. Near-optimal (Euclidean) metric compression. SODA 2017. https://arxiv.org/abs/1609.06295 — abstract
14. P. Indyk, T. Wagner. Optimal (Euclidean) metric compression. SIAM J. Comput. 51(3):467–491, 2022. https://arxiv.org/abs/2110.03152 — summary
15. P. Indyk, T. Wagner. Approximate nearest neighbors in limited space. COLT 2018, PMLR 75. https://proceedings.mlr.press/v75/indyk18a.html — summary
16. A. Ingber, T. Courtade, T. Weissman. Compression for quadratic similarity queries. IEEE Trans. Inf. Theory 61(5):2729–2747, 2015. https://arxiv.org/abs/1307.6609 — [text, §II–III, Theorems 1, 3, 4, Lemmas 2, 4]
17. A. Ingber, T. Weissman. The minimal compression rate for similarity identification. https://arxiv.org/abs/1312.2063 — summary
18. T. S. Jayram, D. P. Woodruff. Optimal bounds for Johnson–Lindenstrauss transforms and streaming problems with subconstant error. SODA 2011; ACM TALG 9(3), 2013, doi:10.1145/2483699.2483706. https://research.ibm.com/publications/optimal-bounds-for-johnson-lindenstrauss-transforms-and-streaming-problems-with-subconstant-error — abstract
19. M. Knott, C. S. Smith. On the optimal mapping of distributions. J. Optim. Theory Appl. 43:39–49, 1984. https://link.springer.com/article/10.1007/BF00934745 — summary
20. K. G. Larsen, J. Nelson. Optimality of the Johnson–Lindenstrauss lemma. FOCS 2017. https://arxiv.org/abs/1609.02094 — abstract
21. W.-K. Loh, S. Lee. Per-query recall guarantees for approximate nearest neighbor search in dynamic vector databases. arXiv:2611.07579 (v2, 11 November 2026). https://arxiv.org/abs/2611.07579 — abstract [raw]; introduction [text]
22. M. Molinaro, D. P. Woodruff, G. Yaroslavtsev. Beating the direct sum theorem in communication complexity with implications for sketching. SODA 2013. https://gyaroslavtsev.github.io/files/soda13-mwy.pdf — abstract and introduction [raw]
23. K. Okajima, T. Yoshida. What limits does quantization place on dense top-k retrieval? A theoretical study. arXiv:2606.11780. https://arxiv.org/abs/2606.11780 — abstract
24. I. Olkin, F. Pukelsheim. The distance between two random vectors with given dispersion matrices. Linear Algebra Appl. 48:257–263, 1982 — summary
25. O. Ordentlich, Y. Polyanskiy. Optimal quantization for matrix multiplication. arXiv:2410.13780v3. https://arxiv.org/abs/2410.13780 — [text, §I–III, eqs. (1), (2), (7), Proposition 1]
26. D. J. Sakrison. A geometric treatment of the source encoding of a Gaussian random variable. IEEE Trans. Inf. Theory 14(3):481–486, 1968 — summary
27. M. Talagrand. Transportation cost for Gaussian and other product measures. Geom. Funct. Anal. 6(3):587–600, 1996. doi:10.1007/BF02249265 — [cited, unchecked]; method as described by Gozlan–Léonard
28. M. Tepper, I. S. Bhati, C. Aguerrebere, M. Hildebrand, T. Willke. LeanVec: searching vectors faster by making them fit. arXiv:2312.16335. https://arxiv.org/abs/2312.16335 — [text, §3, eqs. (7)–(16)]; venue not confirmed
29. C. Villani. Topics in Optimal Transportation. AMS, 2003 — [cited, unchecked]
30. H. Wu, Q. Wang, M. Flierl. Computing similarity queries for correlated Gaussian sources. arXiv:2001.07941. https://arxiv.org/abs/2001.07941 — [text, §III–IV, Theorem 1, Lemma 1, eqs. (23)–(25)]; venue not found
31. A. D. Wyner. Random packings and coverings of the unit n-sphere. Bell Syst. Tech. J. 46, 1967. https://www.nokia.com/bell-labs/publications-and-media/publications/random-packings-and-coverings-of-the-unit-n-sphere/ — summary
32. P. H. Zadeh, R. Hosseini, S. Sra. Geometric mean metric learning. ICML 2016, PMLR 48:2464–2471. https://proceedings.mlr.press/v48/zadeh16.html — [text, §2, eqs. (8), (12)]
33. A. Zandieh, M. Daliri, M. Hadian, V. Mirrokni. TurboQuant: online vector quantization with near-optimal distortion rate. arXiv:2504.19874. https://arxiv.org/abs/2504.19874 — [text, §1.3, Theorems 1–3]
34. J. Zou, J. Chen, A. Özgür, J. Wang. A self-coupling Gaussian extremal inequality for information-constrained optimal transport with applications to rate-distortion-perception. arXiv:2610.25041. https://arxiv.org/abs/2610.25041 — abstract
35. H. Ann, J. Lee, M. Oh. Block-sphere vector quantization. arXiv:2605.19972. https://arxiv.org/abs/2605.19972 — abstract
36. W. Xiao, X. Cao. When does low-bit quantization preserve the decisions of vector search? arXiv:2609.09854. https://arxiv.org/abs/2609.09854 — abstract

## Claims in the brief that were misremembered or wrong

1. **Jayram and Woodruff's title** is "Optimal bounds for Johnson–Lindenstrauss transforms *and streaming problems*
   with subconstant error" (SODA 2011; ACM TALG 2013). Its JL result concerns the dimension of a distributional
   transform, Ω(ε⁻² log 1/δ). Bit bounds come only through one-way communication and streaming.
2. **Alon and Klartag** is a deterministic, all-pairs, additive-ε result valid for ε ≥ n^(−0.49), with unspecified
   constants. It is not a randomised per-pair bound. Its log(1/ε)-bits-per-coordinate range needs k ≤ log n.
3. **Larsen and Nelson** bound the target dimension, Ω(ε⁻² lg n), for arbitrary embeddings of a worst-case point
   set. They do not bound bits.
4. **Indyk and Wagner**: the SODA 2017 paper is "Near-optimal"; the tight bound is the later "Optimal (Euclidean)
   metric compression", SIAM J. Comput. 2022. Both are multiplicative 1 + ε distance results for all pairs.
5. **Ordentlich and Polyanskiy** treat iid identity-covariance Gaussians with both factors quantised and MSE
   distortion. They have no general-covariance or water-filling result and no interval. Their phase transition is at
   R\* ≈ 0.906 bits per entry.
6. **"The tightest Cauchy–Schwarz certificate ... with value Σ√γᵢ"** holds for the product of second moments,
   tr(MΣ_Q)·tr(M⁻¹Σ_R). It does not hold for the mean certificate E‖Δ‖_M·E‖r‖_{M⁻¹}, which is smaller by Jensen.
   The identity is Bhatia–Jain–Lim's Theorem 2, eq. (16), and should be cited as theirs.
7. **"The gain is the Gelbrich gap"**: the gain of the best φ + φ* over the ellipsoid is ½(W₂² − G²), half the gap
   between squared distances, for centred laws with matching covariances. It is not the gap between the distances.
   φ + φ* is also one-sided.
8. **The earlier survey's "apparently new" for the floors** (`lit/REPORT.md`, item 4) missed the identification-rate
   line, Ingber–Courtade–Weissman 2015 and Wu–Wang–Flierl. That line gives the Gaussian answer for compressed data
   with no false negatives, asymptotically. Its formula coincides with the §6 full-floor conjecture when
   Σ_Q = Σ_R [my arithmetic].
9. **Planned result 1 is looser than existing results in the isotropic case [my arithmetic, not a citation
   error].** At high rate the planned floor is a factor √e below Theorem 1 at k = n and below the ICW asymptotic
   answer. At low rate it loses √R against R. The brief does not say this.
10. Not wrong, but a caution: **Knothe–Rosenblatt** is not named in the Gozlan–Léonard account of the
    tensorisation, which disintegrates the measure and attributes the step to Marton and Talagrand. The brief's
    description of the proof is the standard Marton–Talagrand tensorisation.

No claim in the brief about Dowson–Landau, Olkin–Pukelsheim, Gelbrich, Brenier, the optimal map M = Σ_Q⁻¹#Σ_R or the
fidelity Σ√γᵢ was found wrong.

## Not opened

These were not read in full, and their entries above are summaries or abstracts:

- Talagrand 1996, Gelbrich 1990, Dowson–Landau, Olkin–Pukelsheim, Knott–Smith, Givens–Shortt, Brenier and Villani;
- the bodies of Jayram–Woodruff, Larsen–Nelson, Indyk–Wagner (both), Burr–Gao–Knoll, Bai–Wu–Özgür and Zou et al.;
- Wyner, Sakrison, Böröczky–Wintsche and Dumer;
- the PQR-Search appendix (Theorems 7–9, Proposition 1 proofs);
- ConANN, DARTH+ and the HNSW-spanner paper.

The Wu–Wang–Flierl converse (Lemma 1) was read through the summariser only, and its rigour is unverified.
