# Library report: are the four floors results published?

Written 21 November 2026 for the PI. It covers the results the floors theorist wants to call new: L1 and L2
(`floors/CLAIMS.md` §1), F1 (§3), F2 (§4) and B2 (§10). It does not repeat `theory.md`. Notation is that of
`00-notation.md` and `floors/CLAIMS.md` §0.

How each source was read:

- **[raw]**: I read the source's own text. For arXiv papers that means the LaTeX source, downloaded and searched;
  quotes are from it.
- **[text, §x]**: I read the full text through the fetch tool, which summarises and transcribes.
- **[abstract]**: the abstract only. **[summary]**: a search-engine summary only.
- **[cited, unchecked]**: from memory, or I could not open the source.
- **[my arithmetic]**: my own reasoning from quoted statements. I ran no computation.

## Summary of verdicts

| Result | Verdict | Key source |
|---|---|---|
| L1: h(μ) ≤ ln(√(2πe)·C(μ)) | **Published.** It is the case n = 1 of the dimensional Gaussian Talagrand inequality, and the proof is the same: transport, Jensen, then Gaussian integration by parts. | Bolley, Gentil, Guillin, Ann. Probab. 46 (2018), Theorem 2.1 with μ = γ, proved through Lemma 2.6. The inequality was first proved, by semigroup methods, in Bakry, Bolley, Gentil, PTRF 154 (2012). |
| L2: the same under the Knothe–Rosenblatt coupling, coordinate by coordinate | **A known consequence.** It follows from L1 by the chain rule for entropy and Jensen, which is exactly the proof in CLAIMS. I did not find it stated in this product form. It is stronger than the published n-dimensional (Brenier) form. | The same paper, plus the chain rule |
| F1: E h ≥ Σᵢ √min(γᵢ, θ) | **Apparently new as a theorem.** The formula was published as an asymptotic identification rate, and its converse was proved only in the isotropic case and only asymptotically. I found no finite-n bound on mean width, and no proof of the anisotropic converse. | Ingber–Courtade–Weissman 2015 (isotropic, asymptotic); Wu–Wang–Flierl 2020 (the anisotropic formula, with no proved converse) |
| B2b: E a(Δ)·E b(r) ≥ (E‖Δ‖_M)² | **A known consequence.** After the change of variables u = M^½Δ, v = M^(−½)r, the two laws are equal, and the claim becomes the classical one-line proof of MM* ≥ 1 for one Gaussian measure. | Aubrun–Szarek–Ye, CPAM 2014, Appendix D (the one-line proof); Bhatia–Jain–Lim 2019, Theorem 2 (M is the metric that equalises the laws) |

---

## 1. L1

**Statement (CLAIMS §1, L1).** For μ on ℝ with a density and finite second moment, h(μ) ≤ ln(√(2πe)·C(μ)), where
C(μ) = E[g·T_μ(g)] and T_μ is the monotone map from N(0, 1) to μ. Equivalently,
KL(μ ‖ N(0,1)) ≥ ½m₂(μ) − ½ − ln C(μ).

**Verdict: published.** It is the one-dimensional case of a stated theorem, and the published proof is the
theorist's proof.

### 1.1 The source

**Bolley, Gentil, Guillin, "Dimensional improvements of the logarithmic Sobolev, Talagrand and Brascamp–Lieb
inequalities", Ann. Probab. 46(1):261–301, 2018, arXiv:1507.01086 [raw: the LaTeX source, §2].**

- **Theorem 2.1 (Dimensional Talagrand inequality).** Let μ have density e^(−V) with Hess V ≥ R·Id, R > 0. Then
  for every ν ∈ P₂(ℝⁿ),
  (R/2)·W₂²(μ, ν) ≤ ν(V) − μ(V) + n − n·exp[(1/n)(ν(V) − μ(V) − H(ν|μ))].
  The text then says: "When μ is the standard Gaussian measure γ on ℝⁿ, then R = 1 and we recover the
  dimensional Talagrand inequality (6)."
- **Lemma 2.6.** If ∇φ pushes μ₀ forward to μ₁, then ∫Δφ dμ₀ ≥ n·exp[(Ent(μ₀) − Ent(μ₁))/n], where
  Ent(μ) = ∫μ log μ = −h. The proof takes logarithms in the Monge–Ampère equation, giving
  Ent(μ₀) = Ent(μ₁) + ∫log det Hess φ dμ₀. It then applies Jensen to the eigenvalues:
  ∫log det Hess φ dμ₀ ≤ n·log((1/n)∫Δφ dμ₀).
- **Lemma 2.5** turns the Laplacian into ∫(∇φ − x)·… by integration by parts against the Gaussian, using the
  Alexandrov Laplacian as in Cordero-Erausquin (2002). This is the Stein step.
- **Remark 2.7.** In the Gaussian case, "there are no other extremals" than translations of the Gaussian, because
  Hess φ must be constant and equal to the identity.
- **Introduction, eq. (6).** The Gaussian dimensional Talagrand inequality is credited to Bakry, Bolley and Gentil,
  "Dimension dependent hypercontractivity for Gaussian kernels", PTRF 154(3):845–874, 2012, by "local
  hypercontractivity techniques". As printed in the arXiv source, the exponent contains ∫|x|²/(2n) dγ; see §1.2 for
  why this must be dν.

### 1.2 The derivation [my arithmetic]

Take μ = γ in Theorem 2.1, with R = 1 and V = |x|²/2 + const, so that ν(V) − γ(V) = (m₂ − n)/2. Write C* for the
maximal covariance sup E⟨X, G⟩ over couplings of ν and γ, so that W₂² = m₂ + n − 2C*. Then the theorem reads

  C* ≥ n·exp[(m₂ − n)/(2n) − H(ν|γ)/n],

that is,

  H(ν|γ) ≥ ½(m₂ − n) − n·ln(C*/n),  or equivalently  h(ν) ≤ n·ln(√(2πe)·C*/n).

At n = 1 the Brenier map is the monotone map, so C* = C(μ), and this is L1 in both of its forms. The steps of the
proof also match L1's. Step 4 of L1 is the Monge–Ampère identity. Step 5 is Lemma 2.6's Jensen step. Steps 2 and 3
are the Gaussian integration by parts, with the singular part discarded, which is the Alexandrov-Laplacian step.

Eq. (6) as printed has ∫|x|²/(2n) dγ inside the exponential. That integral equals ½ and cancels the −½, leaving
exp(−H/n). Theorem 2.1, which the paper says recovers (6), gives ∫|x|²/(2n) dν instead. So the printed (6) is
weaker than the theorem, and dγ is a typographical error for dν. Cite Theorem 2.1, not (6).

### 1.3 The other places the brief named

- **Hosking, "Distributions with maximum entropy subject to constraints on their L-moments or expected order
  statistics", J. Statist. Plann. Inference 137, 2007 [summary; the IBM reports RC22960 and RC24177 refused
  connection, and the journal page returned 403].** It maximises h = ∫ln Q′(u)du subject to linear constraints on
  Q. The solution has Q′(u) = 1/Σ a_r Z_r(u), with polynomial Z_r for L-moments. The logistic distribution is the
  solution for two L-moments.
  [my arithmetic] With the weight Φ⁻¹(u) in place of a polynomial, the same Lagrangian gives a density-quantile
  function proportional to ∫ᵤ¹Φ⁻¹ = φ(Φ⁻¹(u)), which is the Gaussian. So L1 is "maximum entropy subject to a fixed
  normal-scores functional". I could not check whether Hosking's theorem admits that weight. The same argument
  with the logistic in place of the Gaussian gives h ≤ 2 + ln λ₂, where λ₂ is the second L-moment; I believe that
  bound is Hosking's two-L-moment case, but this is [cited, unchecked].
- **Normality tests [summary].** Shao, Guo and Alemayehu, "Tests for normality based on entropy divergences",
  Statistics in Biopharmaceutical Research 2:408–418, 2010, give "a unified derivation of both the Shapiro–Wilk
  test and Vasicek's test based on estimated entropy divergence". That is the nearest statistical neighbour of L1.
  I could not open it, and it may contain a population form of L1.
  [my arithmetic] L1 orders the two population measures of normality:
  e^(h)/(√(2πe)·σ) ≤ C/σ = ρ_SF, where the left side is Vasicek's population ratio and ρ_SF is the population
  Shapiro–Francia (de Wet–Venter) correlation. I did not find this ordering stated.
- **Rioul, "Yet another proof of the entropy power inequality", arXiv:1606.05969 [text, Lemma 2, eqs. (5),
  (20)]**, and **Rioul, "Rényi entropy power and normal transport", arXiv:1902.06120 [text].** Both use the
  identity h(T(X*)) = h(X*) + E log T′(X*) with Jensen, and Knothe maps in n dimensions. Neither bounds h by
  log E T′, and neither uses Stein's identity.
- **Courtade–Fathi–Pananjady, Cordero-Erausquin (2002), Barthe–Kolesnikov (2008):** not opened [cited,
  unchecked]. Bolley–Gentil–Guillin place their transport proof "in the spirit of" Barthe–Kolesnikov and
  Cordero-Erausquin, so an earlier statement may exist there.

**What a careful author would cite:** Bolley–Gentil–Guillin 2018, Theorem 2.1 and Lemma 2.6, with Bakry–Bolley–Gentil
2012 for the Gaussian case. L1 should be presented as the one-dimensional case written in correlation form, with
the remark that C* = ½(m₂ + 1 − W₂²).

---

## 2. L2

**Statement (CLAIMS §1, L2).** h(ν) ≤ Σᵢ ln(√(2πe)·Cᵢ(ν)), with Cᵢ = E[gᵢ·T^νᵢ(g)] under the Knothe–Rosenblatt map.

**Verdict: a known consequence of a published result.** The derivation is the one in CLAIMS: apply the n = 1 case
of §1 to each conditional law, add the conditional entropies by the chain rule, and apply Jensen to the logarithm,
using E[gᵢT^νᵢ | g_<i] = C(ν_{i|<i}).

**How it relates to the published n-dimensional form [my arithmetic].** By the inequality of the means,
Σᵢ ln Cᵢ ≤ n·ln(ΣCᵢ/n). The Knothe coupling is one coupling among many, so ΣCᵢ ≤ C*. Hence L2 implies
h ≤ n·ln(√(2πe)·C*/n), the Bolley–Gentil–Guillin bound. L2 is therefore the stronger of the two: it keeps a
separate correlation for each coordinate. The theorist needs that per-coordinate form for the water-filling in F1.
The form is new only in the weak sense that I did not find it written down.

**Searched, not found:** an entropy bound written as a product of the diagonal terms of the Knothe map's Jacobian.
Rioul (1606.05969) uses a triangular map with positive diagonal entries but applies Jensen across two variables,
not to the logarithm of an expected Jacobian [text].

**What to cite:** Bolley–Gentil–Guillin for n = 1, with the chain rule. Rioul 2017 for the triangular change of
variables in entropy proofs.

---

## 3. F1 (and the head refinement F2)

**Statement (CLAIMS §3).** Under the Gaussian model, for any encoder and any sound certificate,
E h ≥ W(I(z; c)) = Σᵢ √min(γᵢ, θ), where Σᵢ ½ln⁺(γᵢ/θ) = I.

**Verdict: apparently new as a theorem.** The formula is not new. The theorem is: a bound on mean width, at every
n, for Σ_Q ≠ Σ_R that need not commute, with a proved converse.

### 3.1 What exists

- **Ingber, Courtade, Weissman, IEEE TIT 61(5), 2015.** Already read and recorded in `theory.md` §1.1 and CLAIMS §6.
  The converse covers isotropic Gaussians only, is asymptotic in n, and bounds the probability of the answer
  "maybe", not the mean width.
- **Steiner, Dempfle, Ingber, Weissman, "Compression for quadratic similarity queries: finite blocklength and
  practical schemes", arXiv:1404.5173, 2014 [raw: LaTeX, `lb.tex`, Theorem (theo:lb)].** This is the finite-n
  converse of the ICW line. It is a lower bound on Pr[maybe] built from shell densities of ‖X‖ and ‖Y‖ on
  [√(n(1−η)), √(n(1+η))], a cap-measure function Ω, and the constraint R ≤ (1/n)log₂((1−c)/p*). It is isotropic
  (norms and caps on the sphere), it bounds a probability rather than a width, and it gives no closed form. It is
  not F1. CLAIMS does not cite it, and it should.
- **Wu, Wang, Flierl, arXiv:2001.07941 (v1, 22 January 2020; no journal reference or comments on the arXiv page)
  [abstract page; body as in `theory.md`].** It states the reverse water-filling formula that F1 matches when
  Σ_Q = Σ_R. As CLAIMS P2 records, the paper does not prove a converse.
- **Balasubramanian, Bhandari, Kasiviswanathan, "Query-dependent reading for quantized inner-product estimation:
  a minimax separation", arXiv:2610.24890, 17 October 2026 [text, Theorems 4.3, 6.1, Proposition C.2].** This is
  the newest converse for inner products that I found.
  - It bounds the worst-case mean squared error of an estimate of xᵀy, not a sound interval.
  - Its full-read limit is 1/(4^R − 1), which is the factor in `theory.md` §3.
  - Proposition C.2 uses volumes of the cells (a "geometric rectangle bound"), not widths or support functions.
  - It has no water-filling. The visible text does not cite ICW, although the reference list was cut off.
  - It is close in spirit, and the authors should know of F1.
- **Bai, Wu, Özgür, arXiv:2008.10249 [abstract]**, and the rate–distortion–perception papers with Gaussian reverse
  water-filling: Qian and Chen, arXiv:2406.18008; Chen et al., arXiv:2503.03744 [summary]. These optimise an
  *expected* cost or correlation under an information constraint on a coupling. F1 bounds the expected
  *supremum over a cell*. In the scalar case the information-constrained optimum is a correlation σ₁σ₂√(1 − e^(−2R))
  [my arithmetic, cited, unchecked]. That differs from √min(γ, θ), so these papers do not contain F1.
- **Dumer, Pinsker, Prelov, "On coverings of ellipsoids in Euclidean spaces", IEEE TIT 50(10), 2004 [abstract via
  summary].** The ε-entropy of an ellipsoid is governed by the sub-ellipsoid of axes longer than 2ε. That is
  reverse water-filling for covering in Euclidean radius, a kin result, but it measures a different quantity.
- **Convex geometry.**
  - Sudakov and dual Sudakov (Pajor–Tomczak-Jaegermann) relate Gaussian width and covering numbers up to unspecified
    constants [summary].
  - Paouris and Pathak, arXiv:2607.29522 [abstract], bound mean width against volume radius, up to universal
    constants.
  - Bizeul, arXiv:2607.29458 [abstract and introduction], uses Urysohn's inequality.
  - I found no inequality that bounds mean width below by Gaussian measure or entropy with a sharp constant (a
    "Gaussian Urysohn"), and no average over the cells of a partition.

### 3.2 What a careful author would cite

- ICW 2015 and Steiner–Dempfle–Ingber–Weissman 2014, as the isotropic, probability-of-"maybe" predecessors.
- WWF 2020, for the anisotropic formula, with the remark that its converse is not proved.
- Balasubramanian et al. 2026, as the nearest current work on inner products, which concerns MSE.
- Dumer–Pinsker–Prelov, as reverse water-filling for covering.
- For F2's head term, Urysohn, through Theorem 1 of the consolidated proofs.

The claim should be worded as "the first proof of the converse, at finite n and for non-commuting covariances, in
the mean-width form". It should not claim the formula.

---

## 4. B2

**Statement (CLAIMS §10, B2b).** If |Δ·r| ≤ a(Δ)·b(r) for all Δ and r, then for independent Δ ~ N(0, Σ_Q) and
r ~ N(0, Σ_R), E a(Δ)·E b(r) ≥ (E‖Δ‖_M)², with M = Σ_Q⁻¹#Σ_R. Equality holds for the M-ellipsoid.

**Verdict: a known consequence of the classical MM* ≥ 1, through the geometric mean.**

### 4.1 Sources

- **Aubrun, Szarek, Ye, "Entanglement thresholds for random induced states", Comm. Pure Appl. Math. 67, 2014,
  arXiv:1106.2264 [raw: LaTeX, Appendix D, "The ℓ-position"].** Quoted: "the lower bound w_G(K)w_G(K°) ≥ γ_m² ∼ m
  (or, equivalently, w(K)w(K°) ≥ 1) follows simply from γ_m = E|G| ≤ E(‖G‖_K^½‖G‖_{K°}^½) ≤
  (E‖G‖_K)^½(E‖G‖_{K°})^½ = γ_m w(K°)^½ w(K)^½." The same appendix gives the upper bound (the MM* estimate) and
  credits it to Figiel–Tomczak-Jaegermann and Pisier ("See chapter 3 in [Pisier 1989]").
- **Bizeul, arXiv:2607.29458 [text, introduction]:** "J(K) := M(K)M*(K) satisfies J(K) ≥ 1". There it is derived
  through Urysohn's inequality and polar volume.
- **Bhatia, Jain, Lim 2019, Theorem 2 [as read in `theory.md`]:** MΣ_QM = Σ_R at M = Σ_Q⁻¹#Σ_R.

### 4.2 The derivation [my arithmetic]

1. Put A = M^½, u = AΔ and v = A⁻¹r. Because MΣ_QM = Σ_R, the matrices AΣ_QA and A⁻¹Σ_RA⁻¹ are equal. So u and v
   have the same law, N(0, S).
2. Δ·r = u·v, so the certificate becomes |u·v| ≤ ã(u)·b̃(v).
3. Set v = u, apply the Aubrun–Szarek–Ye line to G ~ N(0, S), and use E|G| = E‖Δ‖_M. This gives
   E a·E b ≥ (E‖Δ‖_M)².
4. The argument uses only |x|² ≤ ã(x)b̃(x) and Cauchy–Schwarz. So it holds for any product form, not only for norms,
   and for any law of G.
5. Equality for the M-ellipsoid is the Euclidean ball in the u-coordinates.

The second-moment version (B2a) is ℓ(K)ℓ(K°) ≥ (E|G|²) in the same coordinates, which is Bhatia–Jain–Lim (16).

**What is new:** only the observation that the geometric-mean metric is the linear map that turns two different
Gaussian measures into one, so that MM* ≥ 1 applies. That is a remark worth one paragraph, not a theorem. **Cite:**
the classical MM* ≥ 1 (Aubrun–Szarek–Ye, Appendix D, for the one-line Gaussian proof; Pisier 1989, ch. 3, for
context [cited, unchecked]) and Bhatia–Jain–Lim, Theorem 2.

---

## Bibliography

1. D. Bakry, F. Bolley, I. Gentil. Dimension dependent hypercontractivity for Gaussian kernels. PTRF 154(3):845–874, 2012 — [cited, unchecked]; attribution from Bolley–Gentil–Guillin
2. F. Bolley, I. Gentil, A. Guillin. Dimensional improvements of the logarithmic Sobolev, Talagrand and Brascamp–Lieb inequalities. Ann. Probab. 46(1):261–301, 2018. https://arxiv.org/abs/1507.01086 — [raw: introduction eq. (6), §2 Theorems 2.1, 2.3, Lemmas 2.5, 2.6, Remark 2.7]
3. J. R. M. Hosking. Distributions with maximum entropy subject to constraints on their L-moments or expected order statistics. J. Statist. Plann. Inference 137, 2007, doi:10.1016/j.jspi.2006.10.010 — [summary]
4. C. Shao, Y. Guo, D. Alemayehu (as given by the search). Tests for normality based on entropy divergences. Statistics in Biopharmaceutical Research 2:408–418, 2010 — [summary]
5. O. Rioul. Yet another proof of the entropy power inequality. arXiv:1606.05969 — [text, Lemma 2, eqs. (5), (20)]
6. O. Rioul. Rényi entropy power and normal transport. arXiv:1902.06120 — [text]
7. F. Steiner, S. Dempfle, A. Ingber, T. Weissman. Compression for quadratic similarity queries: finite blocklength and practical schemes. arXiv:1404.5173 — [raw: `lb.tex`, the lower-bound theorem]
8. H. Wu, Q. Wang, M. Flierl. Computing similarity queries for correlated Gaussian sources. arXiv:2001.07941 — abstract page (v1 22 January 2020, no journal reference); body as in `theory.md`
9. K. Balasubramanian, S. Bhandari, S. P. Kasiviswanathan. Query-dependent reading for quantized inner-product estimation: a minimax separation. arXiv:2610.24890 — [text, first 100,000 characters: Theorems 4.3, 6.1, Proposition C.2]
10. Y. Bai, X. Wu, A. Özgür. Information constrained optimal transport. arXiv:2008.10249 — [abstract]
11. I. Dumer, M. S. Pinsker, V. V. Prelov. On coverings of ellipsoids in Euclidean spaces. IEEE TIT 50(10):2348–2356, 2004 — [summary]
12. G. Paouris, R. Pathak. Optimal mean width and metric entropy estimates for convex bodies. arXiv:2607.29522 — [abstract]
13. P. Bizeul. Optimal MM* bounds for convex bodies. arXiv:2607.29458 — [abstract; introduction, text]
14. G. Aubrun, S. J. Szarek, D. Ye. Entanglement thresholds for random induced states. Comm. Pure Appl. Math. 67, 2014. arXiv:1106.2264 — [raw: Appendix D]
15. L. Qian, J. Chen, arXiv:2406.18008; Chen et al., arXiv:2503.03744 — [summary]

## Not opened

- Hosking 2007 and both IBM reports (connection refused; journal page 403). Whether his theorem allows the weight
  Φ⁻¹ is unchecked.
- Shao–Guo–Alemayehu 2010 (summary only). It may state a population form of L1 and should be read before any claim
  about normality tests.
- Bakry–Bolley–Gentil 2012, Barthe–Kolesnikov 2008, Cordero-Erausquin 2002, Courtade–Fathi–Pananjady, Vasicek 1976
  and de Wet–Venter. An earlier statement of L1 may be in Barthe–Kolesnikov.
- Klartag's lecture notes on MM* (connection reset), Rothvoss's notes (404) and Pisier 1989.
- The bodies of Bai–Wu–Özgür, Dumer–Pinsker–Prelov and the rate–distortion–perception papers.
- The last 58,000 characters of Balasubramanian et al., which include the end of its reference list.
