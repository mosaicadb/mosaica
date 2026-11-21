# Library: codes and approximate search (stage 0)

Librarian's report for the codes and approximate tracks, written 21 November 2026. It checks figures and theorems
that the notes quote from memory, and asks whether two planned results are already published: table-decodable codes
that beat the scalar quantiser on the randomised levels, and a random-energy-model reading of the cluster read with a
stop on a tail estimate. No computation was run; numbers marked "my arithmetic" are one-line divisions or logarithms
of figures given in the sources.

How each source was read:

- **[paper §x]**: I read the paper's own text (arXiv HTML or a PDF text extraction) at the named place.
- **[abstract]**: only the abstract or the arXiv listing.
- **[summary]**: a search engine's or a fetch tool's summary of the source, one step removed.
- **[cited, unchecked]**: from memory or from a secondary mention I could not open.

Symbols follow `notes/00-notation.md`. Local symbols: R is a rate in bits per dimension, b (or B) the bits per
coordinate in a paper's own notation, D(R) the Gaussian distortion–rate function σ²·2^(−2R), G a lattice's
normalised second moment.

---

## Part 1. Codes

### 1.1 Checked figures

A unit-variance source throughout; MSE is per coordinate unless stated. For a centroid-condition quantiser of a
unit-variance Gaussian, E[x·x̂] = E[x̂²] = 1 − D, so the angle between a long rotated block and its code obeys
tan²φ = D/(1 − D) in the limit of long blocks. BlockQuant [C14] uses this identity for its EDEN and RaBitQ
inner-product constants, which is where the tan² column comes from.

| # | Quantity | Value | Source | How read |
|---|---|---|---|---|
| C1 | Sphere, rate R per dimension, best angle as n → ∞ | sin φ → 2^(−R); in MSE terms D → 4^(−R) | BlockQuant [C14] Thm 11 (c_d → 1); cap counting [C17] | paper §5 and App. I; survey §1 |
| C2 | Sphere, finite dimension d, any fixed bd-bit map | E‖x − Q(x)‖² ≥ c_d·4^(−b·d/(d−1)), c_d = (d−1)/(2πe)·(π^(d/2)/Γ(d/2))^(2/(d−1))·exp((ψ(½) − ψ(d/2))/(d−1)) ≈ 0.936, 0.991, 0.999 at d = 100, 1000, 10000 | BlockQuant [C14] Thm 11 | paper §5 |
| C3 | Same bound as stated by TurboQuant | D ≥ 4^(−b), from the Shannon lower bound with h(x) = log₂(area of S^(d−1)) | TurboQuant [C13] Lemma 3, Thm 3 | paper §2.1, §3 |
| C4 | Sphere packing on S^(n−1), angle φ (cap count) | M(n, φ) ≥ (1 + o(1))·√(2πn)·cos φ / sin^(n−1) φ (Chabauty, Shannon 1959, Wyner 1965) | Böröczky survey [C17] | paper §1 |
| C5 | Lloyd–Max MSE, Gaussian, 1–4 bits | 0.3634, 0.1175, 0.03454, 0.009497 (SNR 4.40, 9.30, 14.62, 20.22 dB, my arithmetic) | Max 1960 [C1], table of minimum MSE | [summary] of OCR snippets of the scanned table; the four values are confirmed by TurboQuant [C13] (0.36, 0.117, 0.03, 0.009 for the Beta marginal) and BlockQuant [C14] (0.363, 0.117, 0.0345, 0.0095) [paper] |
| C6 | Lloyd–Max MSE, Gaussian, 5 and 6 bits | 0.002499 (5 bits); 6 bits not found | Max 1960 [C1] | [cited, unchecked] |
| C7 | Optimum uniform (fixed-rate) quantiser, Gaussian, 1–6 bits | 0.3634, 0.1188, 0.03744, 0.01154, ≈0.00349, ? | Max 1960 [C1] | [cited, unchecked]: not found in any source I could open |
| C8 | tan²φ of rotated Lloyd–Max (EDEN), 1–4 bits | 0.571, 0.133, 0.0358, 0.0096 (all / (d−1) as inner-product error); large b: 2.721·4^(−b) | BlockQuant [C14] Cor. 5 | paper §3.2 |
| C9 | tan²φ of RaBitQ / Extended RaBitQ (uniform grid, per-vector rescale), 1–4 bits | 0.571, 0.135, 0.0389, 0.0117 | BlockQuant [C14] Cor. 6 | paper §3.2 |
| C10 | Panter–Dite constant (fixed-rate, high rate, Gaussian) | D ≈ (√3·π/2)·2^(−2R) = 2.721·2^(−2R) (4.35 dB above D(R)) | Panter & Dite 1951 [C2], via Girod lecture table [C4] and TurboQuant [C13] eq. after Thm 1 | lecture slide (text extracted); paper |
| C11 | Entropy-coded uniform scalar quantiser, high rate | factor πe/6 = 1.423 over D(R): 1.53 dB, 0.254 bit | Gish & Pierce 1968 [C3], via Girod [C4] | lecture slide; Gish–Pierce itself [cited, unchecked] |
| C12 | 1-bit RaBitQ code's angle | E⟨ō, o⟩ = √(D/π)·2Γ(D/2)/((D−1)Γ((D−1)/2)), 0.798 to 0.800 for D in [10², 10⁶]; so cos φ ≈ √(2/π) and tan²φ ≈ π/2 − 1 = 0.571 | RaBitQ [C10] §3.2.1 footnote and App.; BlockQuant [C14] | paper |
| C13 | RaBitQ error bound | ⟨ō,q⟩/⟨ō,o⟩ unbiased; error ≤ √((1−⟨ō,o⟩²)/⟨ō,o⟩²)·ε₀/√(D−1) with probability ≥ 1 − 2exp(−c₀ε₀²), c₀ an unspecified constant | RaBitQ [C10] Thm 3.2, restated as Lemma 2.1 of [C11] | paper |
| C14 | "Asymptotically optimal" in the RaBitQ line | B = Θ(log((1/D)·(1/ε²)·log(1/δ))) bits per coordinate, matching Alon–Klartag's Θ(D·log(...)) total when (1/ε²)log(1/δ) > D; constants unspecified | Extended RaBitQ [C11] Thm 3.2, restating Alon–Klartag [C12] Thm 4.1 | paper §3.1, §3.4 |
| C15 | Extended RaBitQ error per bit | ε < 2^(−B)·c_ε/√D with c_ε = 5.75 at the 99.9% quantile: **empirical**, fitted to measured errors, not proved | Extended RaBitQ [C11] Remark ("Empirical Formula"), §5.2.6 | paper |
| C16 | TurboQuant's factor | MSE ≤ (√3π/2)·4^(−b) ≈ 2.7·4^(−b) against its bound 4^(−b); small-b values 0.36, 0.117, 0.03, 0.009, i.e. ratios 1.44, 1.87, 1.92, 2.30 at b = 1–4 (my arithmetic). Inner product: ≤ √3π²·‖y‖²/d·4^(−b) against ‖y‖²/d·4^(−b) | TurboQuant [C13] Thms 1–3 | paper |
| C17 | TCQ, Gaussian, 256 states (shift register 9), fixed rate | SNR 5.08, 10.59, 16.25, 22.05 dB at R = 1, 2, 3, 4 (MSE 0.3105, 0.0873, 0.0237, 0.0062); with optimised reproduction levels 5.56, 11.04, 16.64 dB; D(R) 6.02, 12.04, 18.06, 24.08 | Marcellin & Fischer 1990 [C5] as tabulated by Mao, Gray & Linder [C6] Table I | paper (text extraction of Table I) |
| C18 | TCQ by number of states (4, 8, …, 256) | not obtained | Marcellin & Fischer 1990 [C5] | [abstract]: 0.5, 1, 2 bits beat all earlier results; per-state table [cited, unchecked] |
| C19 | Random-permutation trellis coder (stationary sliding-block decoder) | R = 1: 5.24 dB (L = 8) to 5.95 dB (L = 28); R = 2, 3, 4 at L = 24: 11.90, 17.90, 23.92 dB | Mao, Gray & Linder [C6] Table I | paper |
| C20 | 2-bit Gaussian, fixed rate, by code | Lloyd–Max 0.118; QuIP# E8P (8-D) 0.089; bitshift-trellis TCQ (L = 16, 256-D) 0.068–0.071; D(R) 0.063. As tan²φ: 0.134, 0.098, 0.073–0.076, 0.067 (my arithmetic) | QTIP [C8] Table 1 | paper §3 |
| C21 | Block-spherical codes on rotated vectors | tan² at 1–4 bits: p = 2 blocks 0.571, 0.120, 0.0306, 0.0078; p = 3 blocks 0.553, 0.113, 0.0279, 0.0071; large b: 2.015·4^(−b) (p = 2), 1.770·4^(−b) (p = 3) | BlockQuant [C14] Cor. 10 | paper §4.2 |
| C22 | E8 normalised second moment | G = 0.071682099 (exact; 929/12960, [cited, unchecked] for the fraction); gain over ℤⁿ (G = 1/12): 0.654 dB (my arithmetic) | Agrell & Allen [C15] Table I | paper |
| C23 | Barnes–Wall Λ₁₆ | G = 0.06830 (numerical estimate, Conway & Sloane 1984); 0.864 dB | Agrell & Allen [C15] Table I | paper |
| C24 | Leech Λ₂₄ | G = 0.06577 (numerical estimate); 1.028 dB; best product lattice Λ₁₆ ⊗ E₈ 0.06941; conjectured bounds 0.065608 to 0.067209 | Agrell & Allen [C15] Table I | paper |
| C25 | Limit as n → ∞ | G → 1/(2πe) = 0.05855, 1.53 dB: the same gap as C11 | standard; [C4] | lecture slide |
| C26 | E8 from the [8,4,4] extended Hamming code | Construction A: {x ∈ ℤ⁸ : x mod 2 ∈ H₈} is √2·E₈; equivalently E₈ = D₈ ∪ (D₈ + ½·1) | Conway & Sloane, SPLAG ch. 4–5 [C16] | [cited, unchecked] |
| C27 | Fast E8 decoding | closest point in E8, Dₙ, Eₙ and duals in time proportional to n | Conway & Sloane 1982 [C7] | [summary] of abstract |
| C28 | Successive refinement | successively refinable iff the rate–distortion solutions form a Markov chain; Gaussian with MSE is (block coding); Laplacian with absolute error not always | Equitz & Cover 1991 [C18] | [summary] of abstract |
| C29 | Nested scalar and nested trellis codes | Brunk–Farvardin fixed-rate successively refinable scalar quantisers; embedded scalar quantisers (Sullivan 2004); successively refinable TCQ (Jafarkhani & Tarokh 1999); optimal multi-resolution scalar quantisers can need non-contiguous cells | [C19]–[C21] | [summary]; no loss figures obtained |

Notes on the table.

- **C2 against C3.** BlockQuant points out that the uniform measure on S^(d−1) has no finite differential entropy in
  ℝ^d, so TurboQuant's Lemma 3 applies the Shannon lower bound to the wrong object. Applied to the first d − 1
  coordinates, the bound gains the factor c_d and the exponent bd/(d−1). For the angle the consequence is
  E sin²φ ≥ c_d·4^(−R·d/(d−1)) **only for codes whose scale is inside the bit budget**. A code with a free real scale
  per vector (the randomised levels store one) is a set of 2^(Rd) lines, not points. For those the cap count C4
  gives the bound instead: Pr[φ ≤ θ] ≤ 2·2^(Rd)·σ_d(θ), where σ_d is the normalised cap area, about
  sin^(d−1)θ/(√(2πd)·cos θ). The median angle then satisfies sin φ ≳ 2^(−R·d/(d−1)) up to a factor
  poly(d)^(1/(d−1)). This last step is my derivation from C4 [proved, unrefereed]; the PI should check it before use.
- **C5 and C8 agree.** 0.1175/(1 − 0.1175) = 0.1331 and 0.3634/0.6366 = 0.571, which are BlockQuant's EDEN values.
  So `rust/REPORT_BOUNDS.md`'s "tan² 0.133 is the Lloyd–Max value" is right. Its one-rate figure 0.137 at 2 bits
  sits next to RaBitQ's uniform-grid 0.135 (C9), not next to Lloyd–Max.
- **C17 is fixed rate.** The gap of 256-state TCQ to D(R) grows from 0.9 dB at 1 bit to 2.0 dB at 4 bits. Fixed-rate
  coding of a Gaussian pays a shaping loss as well as the granular loss, so the 0.21 dB figure often quoted for TCQ is
  for the **uniform** source [abstract of C5], not the Gaussian.
- **C20 and the 7% estimate.** `quant/REPORT.md` says E8 "could recover ~7% of width at depth ≥ 3". That is E8's
  granular gain (C22) as a width ratio, √(0.0717/0.0833) = 0.927. At 2 bits and fixed rate the measured codes do
  better: E8P has 0.089 against Lloyd–Max's 0.118, which is 13% of width, and a 256-dimensional trellis has 0.069,
  23% of width (my arithmetic on C20). These figures are for a Gaussian source at one rate, not for nested codes.

### 1.2 Decoders that stay table lookups

- **Lattices.** For E8 the closest-point search is a few comparisons per coordinate (C27). An inner product with a
  code word is a sum over an 8-dimensional block. With asymmetric distance computation that is one lookup per block
  in a query table of 2^(8R) entries: 256 entries at R = 1, 65,536 at R = 2. QuIP#'s E8P [C9] keeps 2-bit E8 decoding
  cheap by storing 2^8 absolute values and using the other 8 bits of each 16-bit word for signs and a shift, so the
  table is 1 KiB. That design decodes the vector; it does not build a per-query table.
- **Trellises.** A generic trellis needs a walk over the first kt bits to know the state at step t, which prevents
  parallel decoding [C8 §3]. The **bitshift trellis** (Mao & Gray's random-permutation trellis coder [C6]) makes each
  reproduction a fixed function of a sliding window of L bits: a stationary sliding-block decoder. QTIP [C8] uses it
  with L = 16 and either a lookup table, a computed pseudo-Gaussian (two to three instructions per weight), or a
  hybrid 2-D table. A coordinate then decodes as T[window], so q·x̂ = Σᵢ qᵢ·T[wᵢ]. That is a decode followed by a
  multiply-add, not a sum of query-dependent table lookups, unless the windows are short enough to tabulate per query.
- **Encoding cost.** Trellis encoding is a Viterbi pass, O(2^L·T) per sequence [C8]; QTIP's tail-biting variant
  rotates the sequence by T/2 and patches the overlap. Lattice encoding is the closest-point algorithm (C27).

### 1.3 Prior art: codes beyond the scalar gap with fast decoding

| Work | What it does | Relevance |
|---|---|---|
| Product quantisation with lookup-table distance [C22], fast-scan SIMD [C23] | learned vector codebooks per sub-block; q·x̂ is a sum of per-block table lookups | the generic answer to "beyond scalar with table decoding"; no guarantee, no rotation theory |
| QuIP# E8P [C9] | random (Hadamard) rotation, then a 2-bit E8-based codebook with a 256-entry table | rotation + lattice + table decoding; LLM weights, not search |
| QTIP [C8] | rotation, then bitshift trellis code, L = 16, 256-dimensional, 0.069 MSE at 2 bits | rotation + trellis + sliding-window decoding; LLM weights |
| NestQuant [C24] (ICML 2025) | nested Gosset (E8) lattice quantisation of weights and activations for matrix products, from Ordentlich–Polyanskiy's optimality theory | nested lattice codes for inner products; LLMs |
| BlockQuant [C14] (arXiv, May 2026) | rotation, then quantisation of 2- and 3-dimensional blocks on the sphere; proves tan² below EDEN, RaBitQ and TurboQuant at every rate; tested on embedding retrieval and KV caches | **closes part of the scalar gap of the RaBitQ family**, in the same expected-distortion terms as C8–C9, with codebook lookups per block |
| EDEN [C25] | rotation + Lloyd–Max per coordinate + rescaling (distributed mean estimation, 2021–22) | the scalar code of the randomised levels, published before TurboQuant; BlockQuant compares against it |
| RSLM [C26] (arXiv, 2026) | rotated, scaled Lloyd–Max codes for ANN residuals with SIMD codebooks of ≤ 16 entries | scalar; confirms that the scalar Lloyd–Max design is already in use for search |
| PQR-Search (arXiv:2611.07579) | 1-bit and 2-bit rotated codes with exact Beta quantiles | already in `reports/surge2/literature.txt`; scalar |

Search terms used: lattice, trellis, E8, nested lattice, block-spherical, together with nearest-neighbour, vector
search, inner product, lookup table, SIMD. I found no trellis-coded quantiser used for nearest-neighbour search, and no
lattice code with a per-item certified angle.

**Verdict on planned result 1 (table-decodable codes beyond the scalar gap).** Not new as a mechanism. Rotation
followed by a lattice (QuIP#, NestQuant), a trellis with a sliding-window decoder (QTIP) or a block-spherical code
(BlockQuant) is published, and BlockQuant does it for retrieval embeddings against RaBitQ and TurboQuant. What I did
not find: (i) a **nested** (successively refinable) lattice or trellis code read prefix by prefix; (ii) a **sound
per-item angle bound** (stored tan φ) for such codes, used in a certificate; (iii) decoding as a sum of per-query
table lookups for a lattice or trellis code; (iv) any loss figure for nesting a trellis or lattice code. The result
can be claimed as new in those respects. It should be positioned against BlockQuant and QTIP by name, and the
expected gain quoted from C20–C21 rather than from the 7% estimate.

---

## Part 2. Search as a random energy model

### 2.1 Addario-Berry and Maillard: exact statement

Source: L. Addario-Berry, P. Maillard, "The algorithmic hardness threshold for continuous random energy models",
arXiv:1810.05129 (v3, July 2019); Mathematical Statistics and Learning 2 (2020) 77–101 [summary for the venue].
Read: the paper itself, §1, §1.1, §4, §5.

**Model.** 𝕋_N is the rooted binary tree of depth N. A is the distribution function of a probability measure on
[0, 1]. X = (X_v), v ∈ 𝕋_N, is the centred Gaussian process with E[X_v X_w] = N·A(|v ∧ w|/N), where v ∧ w is the
most recent common ancestor. Equivalently, X is a time-inhomogeneous binary branching random walk whose
generation-i increments are independent N(0, N(A((i+1)/N) − A(i/N))). The CREM is (X_v) on the leaves V_N.

**Algorithms.** A randomised search algorithm is a sequence of vertices v(1), v(2), … of 𝕋_N (internal nodes
allowed) that is previsible for the filtration F_n = σ(v(1..n); X_v(1..n); U_1..U_(n+1)), with U_i i.i.d. uniform
and independent of X. So the class is every adaptive query algorithm that reads values of nodes one at a time; cost
is the number of queries.

**Theorem 1.1.** Suppose A(t) = ∫₀ᵗ a(s) ds with a Riemann-integrable. Let

    x_* = √(2 log 2) · ∫₀¹ √a(t) dt.

1. For every x < x_* there is a linear-time algorithm that finds v ∈ V_N with X_v ≥ xN with high probability.
   Precisely: there is C(x) such that for every ε > 0 and all large N there are an algorithm and a stopping time τ
   with P(τ ≤ CN, v(τ) ∈ V_N) = 1 and P(X_v(τ) ≥ xN) > 1 − ε. The algorithm is a renormalisation (blocks of
   generations) plus greedy search.
2. For every x > x_* there is γ = γ(A, x) > 0 such that, for N large and any algorithm, the number of queries before
   finding v ∈ V_N with X_v ≥ xN stochastically dominates a Geometric(exp(−γN)) variable.

**Context in the paper (§1.1, §1.2).**

- Ground state (Bovier–Kurkova 2004, Thm 3.1, quoted as (1.1)): (1/N)·max_v X_v → x_s = √(2 log 2)·∫₀¹ √â(s) ds a.s.,
  with â the derivative of the concave hull Â of A. x_s ≤ √(2 log 2), with equality iff A(t) ≤ t for all t (the
  weak-correlation regime, which contains the plain branching random walk A(t) = t).
- x_* ≤ x_s, with equality **iff A is concave**: concavity is necessary and sufficient for an efficient algorithm to
  reach (1 − o(1)) of the ground state.
- Variational form (from Mallein): with E(b, t) = −(log 2)t + ∫₀ᵗ b(s)²/(2a(s)) ds, the trajectories z with
  E(z′, t) ≤ 0 for all t are the "energetically admissible" ones; exponentially many particles follow z when
  E(z′, t) < 0 throughout. x_s = sup z(1) over them; x_* is the end of the natural-speed path z_*(t) = √(2 log 2)·∫₀ᵗ √a,
  which has E = 0 throughout.
- Static thresholds are unrelated to x_*: x_c = β_c = √(2 log 2)/sup √â; both x_* < x_c and x_* > x_c occur.
- §4: if only leaf values can be queried, averaging leaf descendants of a node estimates its value and gives an
  algorithm in time N^(2+o(1)) for x < x_*.
- §5, (5.1), conjectured: above x_*, the cost exponent is
  γ_*(x) = inf{γ > 0 : sup{∫₀¹ b : b admissible, E(b, t) ≥ −γ for all t} > x}, the size of the "pool" of particles
  that must be searched.

**Number of leaves above a level.** The paper gives no theorem for the count. Its heuristic, citing Mallein §1.2, is
that the number of particles that follow a path z to time 1 is exp(N·(−E(z′, 1)) + o(N)) when E(z′, t) < 0 for all
t ≤ 1, and zero with high probability when E(z′, t) > 0 at some t. So the count of leaves above xN is governed by
the best admissible path ending at x, with the constraint at every intermediate level, not by the first moment of
the leaf count alone [paper §1.1; the formula for the count is my reading of it, unrefereed].

### 2.2 Related results

| Result | Statement as read | Source | How read |
|---|---|---|---|
| Derrida's GREM | energies are sums of independent Gaussians attached to the levels of a tree with level-dependent variance; the CREM is its continuum limit | Derrida 1985 [S2]; Bovier–Kurkova 2004 [S3] | through [S1] §1; originals [cited, unchecked] |
| GREM/CREM ground state | √(2 log 2)·∫√â, the concave hull deciding which levels count ("effective levels") | Bovier–Kurkova [S3] Thm 3.1 via [S1] (1.1) | paper (secondary) |
| BRW in a time-inhomogeneous environment | maximal displacement: first order from an optimisation problem, second order n^(1/3) | Mallein 2015 [S4] | [summary] |
| Two-variance BRW | variance profile changes the speed and the log correction (phase transition) | Fang & Zeitouni 2012 [S5] | [summary] |
| Sampling the Gibbs measure, BRW | renormalised greedy sampling in linear time for β < β_c; hardness above | Ho & Maillard, EJP 27 (2022) [S6] | [summary] |
| Sampling the Gibbs measure, CREM | concave A: recursive sampling in O(N^(1+ε)) for all β; non-concave A: a finite threshold β_G with hardness above (KL divergence o(N) as the criterion) | Ho, arXiv:2308.00857 [S7] | [summary] |
| Sampling the CREM at high temperature | two polynomial-time samplers for β < min(β_c, β_G) | Lee & Wu, arXiv:2407.00868 [S8] | [summary] |
| Optimisation of mean-field spin glasses | AMP reaches ALG (an extended Parisi formula); no algorithm with overlap concentration (Lipschitz in the disorder: gradient descent, AMP, bounded-time Langevin) exceeds ALG; generalises Gamarnik–Sudan's overlap gap property to ultrametric forbidden structures | Huang & Sellke, arXiv:2110.07847 [S9] | [abstract] |
| Overlap gap property | topological barrier to optimisation over random structures | Gamarnik, PNAS 2021 [S10]; Montanari 2019 and El Alaoui–Montanari–Sellke 2021 for AMP | [cited, unchecked] |

An observation for the approximate track, mine and unrefereed. The tree model of `hierarchy/theory.md` §2.3 (Model
T: leaf key = sum of independent level Gaussians with variance shares f_j) is a GREM. With K_j = K^(g_j) nodes at
level j, g_j plays the role of t (the share of log K) and f_j the role of A(t). The note's regime "f_j ≥ g_j, beam
subpolynomial" is A(t) ≥ t, which every concave A satisfies, and "f_j < g_j, beam polynomial" is a non-concave A.
Theorem 1.1 is the sharp version of that dichotomy for adaptive algorithms: with concave A a greedy beam reaches the
top; otherwise reaching above x_* costs exp(γN). The note cites neither Derrida nor Addario-Berry–Maillard.

### 2.3 Visit order and stopping: what the cited works do

**Bruch, Krishnan and Nardini, "Optimistic Query Routing in Clustering-based Approximate Maximum Inner Product
Search", arXiv:2405.12207 (v4, 17 Oct 2025).** Read: the paper, §2–§3. Problem 1 seeks the smallest θ_i ≥ ⟨q, μ_i⟩
with Pr_{s∈S_i}[s ≤ θ_i] ≥ (1 + δ)/2. Lemma 1 (one-sided Chebyshev) gives θ_i = ⟨q, μ_i⟩ + √((1+δ)/(1−δ)·qᵀΣ_i q);
shards are probed in decreasing θ_i, a fixed number ℓ of them. Default δ = 0.8, so the multiplier is
√(1.8/0.2) = 3. The covariance is sketched as Σ ≈ D + D^½·Q_t Λ_t Q_tᵀ·D^½ (the "masked sketch"), with D the diagonal
and Q_t, Λ_t the top t eigenpairs of D^(−½)(Σ − D)D^(−½); the router stores t + 2 vectors per shard. §3.2: "the gains
from Optimist widen as k increases", measured against Mean, NormalizedMean, ScaNN and SubPartition, with top-k recall
against points probed. No filters, no stopping rule, no exact fallback. The arXiv page names no venue; the HTML
carries the NeurIPS 2025 checklist.

How the visit-order notes describe it (`HANDOVER.md`, `PLAN.md`, `hierarchy/layered.md`):

- "the same order at the same multiplier": correct (δ = 0.8 gives 3). The derivation differs: Bruch's 3σ is a
  Cantelli bound at the 0.9 quantile of a single row's score, not a Gaussian tail.
- "with no filters, stopping rule or exact fallback": correct.
- "diagonal plus t eigenpairs": correct in substance; the eigenpairs are of the diagonally normalised off-diagonal
  part.
- "Bruch et al. report it widening" (the advantage of μ + 3σ over μ alone, with k): **partly wrong**. Bruch report
  Optimist's gain widening with k against all baselines, chiefly NormalizedMean, not against the plain mean alone.

**Quake (Mohoney et al., arXiv:2506.03437).** Read: the paper, §2.3, §5. Adaptive Partition Scanning estimates
p_i, the probability that partition i holds one of the k neighbours, as the share of the ball B(q, ρ) (ρ the running
k-th distance) cut off by the half-space beyond the bisector with the nearest centroid; cap volumes come from a
regularised incomplete beta function (tabulated at 1,024 points). It scans partitions in decreasing p_i and **stops
when the cumulative estimated recall reaches the per-query target**, recomputing when ρ shrinks by more than a
threshold. It compares against SPANN's distance-ratio rule, LAET and Auncel.

**LAET** (Li, Zhang, Andersen, He, SIGMOD 2020) predicts a per-query search length with a trained model for a
target recall [summary]. **DARTH** (Chatzakis et al., arXiv:2505.19001; SIGMOD 2026 per the paper) predicts the
recall of the running search from search-state features and stops at a declared recall target, on HNSW and IVF
[paper, abstract and §3]. **Auncel** (Zhang et al., NSDI 2023) builds a per-query error–latency profile from local
geometry and stops when an error bound is guaranteed [summary]. **ConANN** (Horchidan et al.) chooses IVF nprobe per
query by conformal risk control on the expected false-negative rate [as described by CaST; unchecked]. **CaST** ("Calibrated Early
Termination with Bounded Optimality Gaps for Constrained Vector Search", arXiv:2611.16148, November 2026) calibrates
monotone stopping rules for graph search to an exactness probability 1 − ε with confidence 1 − δ, and bounds the gap
to the best rule of the class by Lagrangian duality [abstract and §II].

**Tail-estimation stops.** Ciaccia and Patella's PAC-NN (ICDE 2000) derives the stopping condition from the query's
distance distribution, with error and probability parameters set at query time [summary]. Bruch et al. use a Cantelli
tail bound for the order only. I found no stop that fits an extreme-value (generalised Pareto) tail.

### 2.4 Spot-check of five references in `visit-order/theory.md`

| Reference | Cited for | What it proves | Verdict |
|---|---|---|---|
| Weitzman, Econometrica 47 (1979) 641–654 | Pandora's box, k = 1: open in decreasing reservation value ρ, E[(S − ρ)₊] = cost; stop when the best found exceeds the next ρ | KWW restate this as their Theorem 1 (first-best: inspect in decreasing strike price σ_i, stop when the largest value exceeds every remaining σ) with σ defined by E[(v − σ)⁺] = c [paper, KWW §3] | correct; original [cited, bibliographic only] |
| Kleinberg, Waggoner, Weyl, "Descending price optimally coordinates search" (EC 2016; arXiv:1603.07682) | matroid (top-k) extension | main results are for one item; a footnote to §6.2 says the analysis applies "with only minor modifications in the proofs" when the feasible sets of winners form a matroid. Lemma 1 (the covered-call bound E[A_i v_i − I_i c_i] ≤ E[A_i κ_i]) is the tool | correct but thin: the matroid case is a footnote remark, not a theorem [paper] |
| Singla, "The price of information in combinatorial optimization", SODA 2018 (arXiv:1711.00405) | matroid (top-k) selection | gives the optimal adaptive algorithm for the max-weight matroid rank problem (Pandora's box and max spanning tree as cases), credits KWW for the same result independently; the proof bounds the optimum through KWW's Lemma 1 | correct [paper §1] |
| Bradac, Singla, Zuzic, arXiv:1902.01461 (APPROX/RANDOM 2019) | adaptivity gap ≤ 2, tight, for monotone submodular objectives under prefix-closed constraints, improving 3 of Gupta–Nagarajan–Singla 2017 | abstract: at most 2 with a matching lower bound, for multi-value probing; GNS Thm 1.1 gives 3 | correct [abstract; GNS via summary]; venue [cited, unchecked] |
| Feige, Lovász, Tetali, Algorithmica 40 (2004) 219–234 | greedy is a 4-approximation for min-sum set cover | greedy within 4, and 4 − ε is NP-hard | correct [summary] |

`theory.md` Theorem 4 combines these into: under M1 with h = 0, the policy that opens rows in decreasing ρ_c and
stops when the running k-th best is at least the next ρ_c is optimal for E[sum of the k best found] − λ·rows. That is
Singla's matroid result applied to the uniform matroid of rank k, so the attribution holds. The theorem needs the
rows' scores independent across rows, which M1 grants.

### 2.5 Verdict on planned result 2 (the read as a random energy model, with a stop on a tail estimate)

- **The model.** I found no paper that models inverted-file, tree or cluster-based nearest-neighbour search as a
  random energy model or a branching random walk (searches on random energy model, GREM, branching random walk with
  nearest-neighbour, inverted file, vector search, beam search). The mathematics is fully published:
  Addario-Berry–Maillard is a theorem about greedy and beam search on a Gaussian tree, with a sharp threshold, an
  exact class of algorithms, and an exponential lower bound. The application to search indexes appears new; the
  theorems it would rest on are not. The hierarchy note's Model T already is a GREM and should cite it.
- **The stop.** Not new in kind. Per-query stopping on an estimated recall or error is published in at least five
  forms (PAC-NN 2000, LAET 2020, Auncel 2023, Quake 2025, DARTH 2026), with calibrated risk control in ConANN and
  CaST, and CaST (November 2026) already bounds a stopping rule's distance from the best rule of its class. What I
  did not find: a stop on the **expected number of missed neighbours** computed from a fitted tail of the unread
  clusters' scores, and a global threshold on per-cluster p_c across queries (the Neyman–Pearson form of
  `theory.md` Theorem 3).
- **Positioning.** Claim the random-energy reading and the expected-misses stop; cite Addario-Berry–Maillard for
  the threshold and Quake, Auncel, DARTH, ConANN and CaST for adaptive stopping.

---

## Bibliography

Codes.

- [C1] J. Max, "Quantizing for minimum distortion", IRE Trans. Inf. Theory 6(1) (1960) 7–12.
  https://web.stanford.edu/class/ee398a/handouts/papers/Max%20-%20Quantizing%20for%20Min%20Distortion.pdf (scanned; not text-readable)
- [C2] P. Panter, W. Dite, "Quantization distortion in pulse-count modulation with nonuniform spacing of levels",
  Proc. IRE 39(1) (1951) 44–48. [cited through C4 and C13]
- [C3] H. Gish, J. N. Pierce, "Asymptotically efficient quantizing", IEEE Trans. Inf. Theory 14(5) (1968). [cited through C4]
- [C4] B. Girod, EE398A lecture notes, "Quantization", slides "High-rate results" and "Comparison of high-rate
  performance". https://web.stanford.edu/class/ee398a/handouts/lectures/05-Quantization.pdf
- [C5] M. W. Marcellin, T. R. Fischer, "Trellis coded quantization of memoryless and Gauss-Markov sources", IEEE
  Trans. Commun. 38(1) (1990) 82–93, doi:10.1109/26.46532. [abstract]
- [C6] M. Z. Mao, R. M. Gray, T. Linder, "Rate-constrained simulation and source coding i.i.d. sources", IEEE Trans.
  Inf. Theory (2011). https://ee.stanford.edu/~gray/trellis.pdf
- [C7] J. H. Conway, N. J. A. Sloane, "Fast quantizing and decoding algorithms for lattice quantizers and codes",
  IEEE Trans. Inf. Theory 28(2) (1982) 227–232, doi:10.1109/TIT.1982.1056484. [summary]
- [C8] A. Tseng, Q. Sun, D. Hou, C. De Sa, "QTIP: Quantization with trellises and incoherence processing",
  arXiv:2406.11235 (NeurIPS 2024 [cited, unchecked]). https://arxiv.org/abs/2406.11235
- [C9] A. Tseng, J. Chee, Q. Sun, V. Kuleshov, C. De Sa, "QuIP#: Even better LLM quantization with Hadamard
  incoherence and lattice codebooks", ICML 2024, arXiv:2402.04396. https://arxiv.org/abs/2402.04396
- [C10] J. Gao, C. Long, "RaBitQ", SIGMOD 2024, arXiv:2405.12497. https://arxiv.org/abs/2405.12497
- [C11] J. Gao, Y. Gou, Y. Xu, Y. Yang, C. Long, R. C.-W. Wong, "Practical and asymptotically optimal quantization of
  high-dimensional vectors in Euclidean space for approximate nearest neighbor search", arXiv:2409.09913.
  https://arxiv.org/abs/2409.09913
- [C12] N. Alon, B. Klartag, "Optimal compression of approximate inner products and dimension reduction", FOCS 2017,
  arXiv:1610.00239. [through C11]
- [C13] A. Zandieh, M. Daliri, M. Hadian, V. Mirrokni, "TurboQuant: Online vector quantization with near-optimal
  distortion rate", arXiv:2504.19874. https://arxiv.org/abs/2504.19874
- [C14] "Block-Sphere Vector Quantization" (BlockQuant), arXiv:2605.19972, 19 May 2026 (Seoul National University).
  https://arxiv.org/abs/2605.19972
- [C15] E. Agrell, B. Allen, "On the best lattice quantizers", IEEE Trans. Inf. Theory (2023), arXiv:2202.09605.
  https://arxiv.org/abs/2202.09605
- [C16] J. H. Conway, N. J. A. Sloane, Sphere Packings, Lattices and Groups, 3rd ed., Springer 1999. [cited, unchecked]
- [C17] K. Böröczky Jr. et al., survey "Packing and covering in higher dimensions", arXiv:2202.11358.
  https://arxiv.org/abs/2202.11358 ; A. D. Wyner, "Random packings and coverings of the unit n-sphere", Bell Syst.
  Tech. J. 46 (1967) [abstract page only].
- [C18] W. H. R. Equitz, T. M. Cover, "Successive refinement of information", IEEE Trans. Inf. Theory 37(2) (1991)
  269–275. https://isl.stanford.edu/~cover/papers/transIT/0269equi.pdf [summary]
- [C19] H. Brunk, N. Farvardin, fixed-rate successively refinable scalar quantisers, DCC 1996. [cited, unchecked]
- [C20] G. J. Sullivan, "On embedded scalar quantization", ICASSP 2004.
  https://www.microsoft.com/en-us/research/?p=799264 [summary]
- [C21] H. Jafarkhani, V. Tarokh, "Design of successively refinable trellis-coded quantizers", IEEE Trans. Inf.
  Theory 45(5) (1999). https://engineering.uci.edu/files/Jafarkhani-Design-of-Successively-Refinable-July-1999.pdf [title only]
- [C22] H. Jégou, M. Douze, C. Schmid, "Product quantization for nearest neighbor search", IEEE TPAMI 33(1) (2011). [cited, unchecked]
- [C23] F. André, A.-M. Kermarrec, N. Le Scouarnec, "Quicker ADC", arXiv:1812.09162. [title only]
- [C24] S. Savkin, E. Porat, O. Ordentlich, Y. Polyanskiy, "NestQuant: Nested lattice quantization for matrix
  products and LLMs", ICML 2025, arXiv:2502.09720. https://arxiv.org/abs/2502.09720 [abstract]
- [C25] S. Vargaftik et al., "EDEN: Communication-efficient and robust distributed mean estimation", ICML 2022.
  [cited through C14]
- [C26] R. Lenhardt et al., "RSLM: Training-free vector quantization for approximate nearest neighbor search",
  arXiv:2608.30384. https://arxiv.org/abs/2608.30384 [abstract]

Search, random energy models, stopping.

- [S1] L. Addario-Berry, P. Maillard, "The algorithmic hardness threshold for continuous random energy models",
  arXiv:1810.05129; Math. Stat. Learn. 2 (2020) 77–101. https://arxiv.org/abs/1810.05129
- [S2] B. Derrida, "A generalization of the random energy model which includes correlations between energies",
  J. Physique Lett. 46 (1985) L401–L407. [cited, unchecked]
- [S3] A. Bovier, I. Kurkova, "Derrida's generalised random energy models 1–2", Ann. Inst. H. Poincaré Probab.
  Statist. 40 (2004). [through S1]
- [S4] B. Mallein, "Maximal displacement of a branching random walk in time-inhomogeneous environment", Stoch. Proc.
  Appl. 125 (2015) 3958–4019, arXiv:1307.4496. https://arxiv.org/abs/1307.4496 [summary]
- [S5] M. Fang, O. Zeitouni, "Branching random walks in time inhomogeneous environments", EJP (2012).
  https://www.wisdom.weizmann.ac.il/~zeitouni/pdf/TwoVariances_EJP.pdf [summary]
- [S6] F. Ho, P. Maillard, "Efficient approximation of branching random walk Gibbs measures", EJP 27 (2022),
  arXiv:2107.11465. https://arxiv.org/abs/2107.11465 [summary]
- [S7] F. Ho, "Sampling from the Gibbs measure of the continuous random energy model and the hardness threshold",
  arXiv:2308.00857. https://arxiv.org/abs/2308.00857 [summary]
- [S8] H. Lee, Q. Wu, arXiv:2407.00868 (CREM sampling at high temperature). https://arxiv.org/abs/2407.00868 [summary]
- [S9] B. Huang, M. Sellke, "Tight Lipschitz hardness for optimizing mean field spin glasses", arXiv:2110.07847.
  https://arxiv.org/abs/2110.07847 [abstract]
- [S10] D. Gamarnik, "The overlap gap property: a topological barrier to optimizing over random structures", PNAS
  118(41) (2021). [cited, unchecked]
- [S11] S. Bruch, A. Krishnan, F. M. Nardini, "Optimistic query routing in clustering-based approximate maximum inner
  product search", arXiv:2405.12207. https://arxiv.org/abs/2405.12207
- [S12] J. Mohoney et al., "Quake: Adaptive indexing for vector search", arXiv:2506.03437.
  https://arxiv.org/abs/2506.03437 (venue OSDI 2025 [cited, unchecked])
- [S13] C. Li, M. Zhang, D. G. Andersen, Y. He, "Improving approximate nearest neighbor search through learned
  adaptive early termination", SIGMOD 2020. [summary]
- [S14] M. Chatzakis et al., "DARTH: Declarative recall through early termination for approximate nearest neighbor
  search", arXiv:2505.19001, SIGMOD 2026. https://arxiv.org/abs/2505.19001
- [S15] Z. Zhang et al., "Fast, approximate vector queries on very large unstructured datasets" (Auncel), NSDI 2023.
  https://www.usenix.org/conference/nsdi23/presentation/zhang-zili [summary]
- [S16] "Calibrated early termination with bounded optimality gaps for constrained vector search" (CaST),
  arXiv:2611.16148. https://arxiv.org/abs/2611.16148 [abstract, §II]; ConANN (Horchidan, Zeiher, Boström, Carbone)
  cited there [cited, unchecked].
- [S17] P. Ciaccia, M. Patella, "PAC nearest neighbor queries: approximate and controlled search in high-dimensional
  and metric spaces", ICDE 2000, 244–255. [summary]
- [S18] M. L. Weitzman, "Optimal search for the best alternative", Econometrica 47 (1979) 641–654. [bibliographic, via S19]
- [S19] R. Kleinberg, B. Waggoner, E. G. Weyl, "Descending price optimally coordinates search", arXiv:1603.07682.
  https://arxiv.org/abs/1603.07682
- [S20] S. Singla, "The price of information in combinatorial optimization", SODA 2018, arXiv:1711.00405.
  https://arxiv.org/abs/1711.00405
- [S21] D. Bradac, S. Singla, G. Zuzic, "(Near) optimal adaptivity gaps for stochastic multi-value probing",
  arXiv:1902.01461. https://arxiv.org/abs/1902.01461 ; A. Gupta, V. Nagarajan, S. Singla, SODA 2017, arXiv:1608.00673.
- [S22] U. Feige, L. Lovász, P. Tetali, "Approximating min sum set cover", Algorithmica 40 (2004) 219–234. [summary]

---

## Corrections: claims that were misremembered, wrong or overstated

1. **TurboQuant's lower bound** (relevant to the brief's "factor it proves"). The factor √3π/2 ≈ 2.7 is real, but it
   is the Panter–Dite constant measured against a bound, 4^(−b), whose proof applies the Shannon lower bound to the
   sphere as if it had a density in ℝ^d. The corrected bound is c_d·4^(−bd/(d−1)) (BlockQuant Thm 11). At b = 1–4
   the actual ratio is 1.4–2.3, not 2.7. The factor is in MSE, so √2.72 = 1.65 in width.
2. **"Asymptotically optimal" (RaBitQ line)** means the bit count matches Alon–Klartag up to an unspecified constant
   factor, not that the distortion constant is optimal. Extended RaBitQ's 2^(−B)·5.75/√D is an empirical 99.9%
   quantile, not a theorem. RaBitQ's own bound carries an unspecified c₀.
3. **The scalar gap of the RaBitQ family is already partly closed in print.** BlockQuant (arXiv, May 2026) proves
   tan² of 0.120 (2-D blocks) and 0.113 (3-D blocks) at 2 bits against 0.133 (Lloyd–Max) and 0.135 (RaBitQ).
   `reports/surge2/literature.txt` and `lit/REPORT.md` do not list it.
4. **Trellis decoding.** Marcellin–Fischer TCQ does not have a sliding-window decoder; a generic trellis decodes by a
   state walk. The sliding-window (bitshift) decoder is Mao–Gray–Linder's random-permutation trellis coder, used by
   QTIP. The "within 0.21 dB of D(R)" figure for 256-state TCQ is for the uniform source; for the Gaussian at fixed
   rate the gap is 0.9 to 2.0 dB from 1 to 4 bits.
5. **`quant/REPORT.md`, "E8 could recover ~7% of width".** 7% is E8's high-rate granular gain over the cubic lattice.
   Published 2-bit Gaussian figures show more: E8P 13% of width and a 256-dimensional trellis 23% over Lloyd–Max
   (QTIP Table 1). The estimate understates what a structured code can give at fixed rate, before nesting losses.
6. **Lattice constants.** Only E8's normalised second moment is exact (0.071682099). The Barnes–Wall (0.06830) and
   Leech (0.06577) values are numerical estimates; quote them as such.
7. **`visit-order/theory.md` §2.4**, "Quake's APS and learned early termination (LAET, DARTH) are instances of this
   idea" (a global threshold on p_c across queries, Theorem 3). Wrong. All three stop on a **per-query** recall
   target: APS when cumulative estimated recall reaches the target, LAET by predicting a per-query search length,
   DARTH by predicting recall. Only APS orders partitions by an estimated probability. The reference list's own
   description of Quake ("stopping on estimated recall") is accurate.
8. **`visit-order/PLAN.md`, "Bruch et al. report it widening".** Bruch report Optimist's gain widening with k against
   all baselines (mainly NormalizedMean), not the gain of μ + 3σ over the plain mean. The other descriptions of Bruch
   in `HANDOVER.md` and `PLAN.md` are right; the 3 comes from Cantelli with δ = 0.8, not from a Gaussian tail. The
   NeurIPS 2025 venue is not on the arXiv page (the paper carries the NeurIPS 2025 checklist).
9. **`visit-order/theory.md` Theorem 4, Kleinberg–Waggoner–Weyl for matroids.** KWW state the matroid case only in a
   footnote ("with only minor modifications in the proofs"). Singla 2018 is the source with the matroid theorem;
   cite Singla first.
10. **`visit-order/hierarchy/theory.md` §2.3** (missing prior art, not an error). Model T is Derrida's GREM, and its
    f_j ≥ g_j dichotomy is the concave/non-concave dichotomy of Addario-Berry–Maillard, who prove the sharp
    algorithmic threshold x_* = √(2 log 2)·∫√a for every adaptive query algorithm.
11. **`lit/REPORT.md`, Alon–Klartag "explicit formula seen only in secondary slides".** The formula,
    Θ(D·log((1/D)(1/ε²)log(1/δ))) bits when (1/ε²)log(1/δ) > D, is restated as Alon–Klartag Theorem 4.1 in Extended
    RaBitQ §3.1; the original was still not opened.
12. Checked and correct: tan² = 0.133 as the Lloyd–Max value (`rust/REPORT_BOUNDS.md`); sin φ = 2^(−R) as the
    asymptotic sphere bound; Panter–Dite √3π/2; πe/6; Equitz–Cover for Gaussian sources; Weitzman, Singla,
    Bradac–Singla–Zuzic and Feige–Lovász–Tetali as cited in `theory.md`; Bruch's multiplier of 3.

## Not opened

Max 1960 (scanned, LZW-compressed; values from OCR snippets and two papers that reproduce them); the per-state table
of Marcellin–Fischer 1990; any table of optimum uniform quantisers or of Lloyd–Max at 6 bits; Gish–Pierce 1968;
Conway–Sloane's SPLAG and the 1982 paper (abstract only); Derrida 1985; Bovier–Kurkova 2004; Mallein 2015 beyond its
abstract; the Gaussian-sphere successive-refinement question (no source found on whether the uniform source on the
sphere is successively refinable at finite d); loss figures for nested lattice or nested trellis codes; Alon–Klartag
itself; LAET, Auncel, ConANN, PAC-NN full texts; the venues of KWW (EC 2016), Bradac–Singla–Zuzic (APPROX/RANDOM
2019), Quake (OSDI 2025) and QTIP (NeurIPS 2024).
