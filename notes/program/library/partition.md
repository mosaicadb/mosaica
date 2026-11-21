# Library: how to partition the vectors (stage 0, partition track)

Librarian's report for the track that asks whether k-means is the right partition for an index that certifies
inner-product thresholds, written 21 November 2026. No computation was run. Symbols follow `notes/00-notation.md`:
N rows, dimension n, K clusters, G groups. Local symbols are declared where used.

How each source was read:

- **[paper §x]**: the paper's own text (arXiv HTML, PDF text, or publisher HTML) at the named place.
- **[abstract]**: only the abstract or the listing page.
- **[summary]**: a search engine's or fetch tool's summary, one step removed.
- **[cited, unchecked]**: from memory or a secondary mention I could not open.

What the index does with a partition (from `SYSTEM.md` §2.1–2.2), so the reader can weigh each source: spherical
k-means into K ≈ N/1000 clusters; the residual r = x − m against the stored centre; a 16-bit key x·m̂ per row; a
support table of exact extremes of anchor·x over each cluster, used to accept or skip whole clusters; clusters
grouped by k-means on centre directions into G groups, each with one basis fitted to the group's residual
covariance and a query-offset covariance. The partition therefore matters in four places: the cut (the cluster's
extent along anchors and the largest residual norm), the key range (how many rows a key cannot decide), the
width of every later bound (Σ_R per group, through F = Σ √γᵢ), and setup cost per opened cluster and group.



---

## 1. Partition objectives tied to coding cost

Local symbols for this section: a cluster's sample W ∈ ℝ^(n×m) holds m vectors as columns; ε² is an allowed squared
distortion per vector; Π_j is the diagonal membership matrix of class j; λ is a Lagrange multiplier; ℓ_i a code
length in bits.

### 1.1 Ma, Derksen, Hong and Wright (2007): segmentation by coding length

Y. Ma, H. Derksen, W. Hong, J. Wright, "Segmentation of multivariate mixed data via lossy data coding and
compression", IEEE TPAMI 29(9):1546–1562, 2007. **Read: the paper, §2.1–2.3, §4.1, App. B** (PDF from the first
author's page, text extracted locally).

- **Coding rate of one Gaussian group (zero mean), eq. (5):** R(V) = ½ log₂ det(I + n/(ε² m) · V Vᵀ) bits per vector.
  Footnote 7 says the true Gaussian rate–distortion function is ½ log₂ det(n/ε² · Σ) only when ε²/n is below the
  smallest eigenvalue of Σ; the "I +" form is presented as the rate of a regularised source, valid at every ε, and
  App. A shows it is an upper bound on the bits for a finite sample.
- **Coding length of the group, eq. (6):** L(V) = (m + n)/2 · log₂ det(I + n/(ε² m) · V Vᵀ). The extra n·R(V) bits
  are explained (footnote 8) as the cost of coding the n principal axes, that is, the codebook.
- **Non-zero mean, App. B eq. (28)–(29):** the mean is coded at precision ε/√n for a further
  n/2 · log₂(1 + μᵀμ/ε²) bits, so L(W) = (m + n)/2 · log₂ det(I + n/(ε² m) W̄W̄ᵀ) + n/2 · log₂(1 + μᵀμ/ε²), with W̄
  the centred sample. (The second term is read from the extracted text; the first is the paper's stated form.)
- **Segmentation objective, eq. (7):** L^s(W₁, …, W_k) = Σᵢ [ L(Wᵢ) + |Wᵢ| · (−log₂(|Wᵢ|/m)) ], the second term
  being the bits for the membership of each sample (an entropy code; footnote 9 notes a tighter term when nearby
  samples share groups). The optimal segmentation minimises L^s over all partitions, at a user-chosen ε. A
  penalty mn·log ε can be added to choose ε (footnote 10).
- **Claimed results.** (i) A deterministic segmentation is approximately asymptotically optimal: Theorem 3 (§4)
  says the asymptotic part of the segmented rate is concave in the soft membership Π, so its minimum over the convex
  set of soft memberships is at a vertex, a hard segmentation. (ii) §4.1, Example 1: by concavity of log det,
  L(W₁) + L(W₂) ≤ L(W₁ ∪ W₂) for the m ≫ n terms, so splitting pays when the gain exceeds the one bit per vector of
  membership; the gain is largest when the two groups are orthogonal. (iii) The number of groups and their
  dimensions follow from ε alone.
- **Algorithm (§2.3, Alg. 1):** bottom-up greedy merging: start from singletons, merge the pair that most decreases
  L^s, stop when no merge decreases it. The paper says global minimisation is intractable. No approximation
  guarantee is given for the greedy merge.
- **Evidence:** simulations (phase-transition-like behaviour of the number of groups as ε or the share of outliers
  varies), segmentation of affine subspaces, natural images and microarray data. No comparison with k-means at equal
  rate on a coding task that I found in the extracted text.

**Follow-ups.**

- **Maximal coding rate reduction (MCR²):** Y. Yu, K. H. R. Chan, C. You, C. Song, Y. Ma, "Learning diverse and
  discriminative representations via the principle of maximal coding rate reduction", NeurIPS 2020,
  arXiv:2006.08558. **Read: the paper, §2.1–2.2 and App. A.** R(Z, ε) = ½ logdet(I + d/(m ε²) Z Zᵀ) (eq. 4);
  R^c(Z, ε | Π) = Σⱼ tr(Πⱼ)/(2m) · logdet(I + d/(tr(Πⱼ) ε²) Z Πⱼ Zᵀ) (eq. 5), concave in Π for fixed Z; the objective
  maximises ΔR = R − R^c over the representation Z under a norm constraint. Theorem 2.1 (informal; formal as Theorem
  A.6): at the optimum the class subspaces are orthogonal when d ≥ Σⱼ dⱼ, and each reaches its maximal rank dⱼ with
  equal leading singular values when ε is small enough. This is a criterion for *learning a map*, not for
  partitioning fixed vectors, although the paper also reports clustering by optimising Π. The objective is the
  log-det coding length of Ma et al. applied to a learnt representation.
- Wright, Tao, Zhou, Ma, "Classification via minimum incremental coding length" (NIPS 2007): assigns a test sample
  to the class whose coding length grows least when it is added [cited, unchecked].
- Later MCR²-based subspace clustering (for example NMCE, Li et al. 2022; manifold linearising and clustering, Ding
  et al. 2023) [cited, unchecked]: clustering of learnt features, not of fixed vectors.

### 1.2 k-subspaces, generalised PCA, local PCA and mixtures of probabilistic PCA

- **k-plane / k q-flats.** P. S. Bradley, O. L. Mangasarian, "k-Plane clustering", J. Global Optim. 16(1):23–32,
  2000; P. Tseng, "Nearest q-flat to m points", JOTA 105(1):249–252, 2000. **Read: abstracts and a search summary.**
  Objective: minimise Σ over points of the squared distance to the nearest of k affine q-flats; Lloyd-type
  alternation, each flat refitted by the smallest (k-plane) or largest (q-flat) eigenvectors of its points'
  scatter. Local solutions only.
- **Generalised PCA.** R. Vidal, Y. Ma, S. Sastry, IEEE TPAMI 27(12):1945–1959, 2005 [cited, unchecked: citation
  confirmed, text not opened]. Algebraic: fit a polynomial vanishing on the union of subspaces and read the normals
  from its derivatives; used to initialise k-subspaces and EM.
- **Local PCA by reconstruction error (VQPCA).** Kambhatla and Leen (Neural Comput. 1997) and Dony and Haykin
  (1995): a point goes to the region whose local PCA reconstructs it with least squared error, with a target
  dimension fixed in advance [cited, unchecked; described in the two papers below].
- **Mixtures of probabilistic PCA.** M. E. Tipping, C. M. Bishop, "Mixtures of probabilistic principal component
  analysers", Neural Comput. 11(2):443–482, 1999. **Read: the paper (preprint), §3–5.** Component i has density
  p(t | i) = (2π)^(−d/2) |Cᵢ|^(−½) exp(−E²ᵢ/2) with E²ᵢ = (t − μᵢ)ᵀ Cᵢ⁻¹ (t − μᵢ) and Cᵢ = σᵢ² I + WᵢWᵢᵀ (eqs. 25–27);
  fitted by EM, assignment is soft through responsibilities. §5.2 compares it with VQPCA on six small datasets (300
  to 500 points, d from 3 to 64): VQPCA has lower reconstruction error in general, as expected since it optimises
  that measure, except on one dataset; on the image-compression test (8×8 blocks, 4,050 vectors) the mixture's test
  reconstruction error was "on average, more than 10% lower", which the authors attribute to soft clustering
  reducing overfitting. The partition criterion here is a likelihood, which carries a ½ log|Cᵢ| term, so it is
  closer to a coding length than k-means is.

### 1.3 Entropy-constrained vector quantisation (Chou, Lookabaugh and Gray 1989)

P. A. Chou, T. Lookabaugh, R. M. Gray, "Entropy-constrained vector quantization", IEEE Trans. ASSP 37(1):31–42, 1989.
**Read: abstract and summaries; the encoder rule read in Gray and Neuhoff's survey (1998), §on Lloyd optimality
conditions** (the paper itself not opened).

- Objective: minimise the Lagrangian E[d(X, β(α(X)))] + λ E[ℓ(α(X))], expected distortion plus λ times the expected
  length of the index's lossless code.
- Necessary conditions for a locally optimal code (as the survey states them): the decoder outputs centroids; the
  lossless code is optimal for the index distribution (ℓᵢ ≈ −log pᵢ); and the encoder is the nearest-neighbour rule
  for the *modified* distortion d(x, yᵢ) + λ ℓᵢ, so that "the minimum distance encoder is suboptimal" in the
  variable-rate case. The cells are therefore no longer Voronoi cells: a frequent codeword gets a larger cell.
- Design is a Lloyd-type descent on the Lagrangian. Evidence in the paper is design experiments [cited, unchecked].
- Gray, Linder and Li, "A Lagrangian formulation of Zador's entropy-constrained quantization theorem", IEEE TIT
  48(3), 2002, give the high-resolution limit of this Lagrangian form [cited, unchecked; the PDF link was dead].

### 1.4 Rate-optimal partitions for transform coding

- **Effros and Chou, weighted universal transform coding (WUTC).** M. Effros, P. A. Chou, "Weighted universal
  transform coding: universal image compression with the Karhunen–Loève transform", ICIP 1995; extended in Effros,
  Chou, Gray, "Weighted universal image compression", IEEE TIP 8(10), 1999. **Read: abstract and catalogue
  summaries.** A two-stage code: the first stage picks, per block of data, one of a collection of (KLT, bit
  allocation) pairs, the second codes with it; the collection is designed by a Lloyd-type algorithm on the total
  rate–distortion Lagrangian, so the partition of blocks among transforms is rate–distortion optimal by
  construction. Claimed: up to 2 dB over a JPEG-style DCT coder with an optimal set of bit allocations and up to 6 dB
  over entropy-constrained weighted universal VQ, on mixed text and greyscale images. Fast variants (Effros 1997)
  limit the transform order; Feng and Effros (ICASSP 1999) use separable KLTs at 1/8 of the multiplications.
- **Archer and Leen, coding-optimal partition.** C. Archer, T. K. Leen, "From mixtures of mixtures to adaptive
  transform coding", NIPS 13, 2000 (pp. 925–931), and "A generalized Lloyd-type algorithm for adaptive transform
  coder design", IEEE TSP 52(1):255–264, 2004. **Read: the NIPS paper in full; the TSP paper by abstract.**
  - The partition rule (eq. 12): each vector goes to the region "whose transform coder compresses it with the least
    distortion", that is, by the actual quantised reconstruction error under that region's transform, bit
    allocation and scalar quantisers. The paper contrasts this explicitly with k-means (nearest mean) and with local
    PCA (dimension-reduction error at a fixed target dimension).
  - The coding-optimal transform (COT): orthogonal W with QW symmetric, where Q collects the quantisation errors
    (eq. 9). It reduces to PCA for Gaussian data.
  - The algorithm alternates: partition, transforms, bit allocation (greedy, one bit at a time to the coordinate of
    largest distortion), scalar Lloyd quantisers. It is a constrained generalised Lloyd algorithm, so it descends
    the coding distortion monotonically.
  - **Evidence (the headline for this track):** on MRI and traffic-image test sets, at equal numbers of regions (16
    and 32) and with region bits counted in the rate, the coding-optimal partition gains **1.1–1.8 dB over k-means
    partitioning**, 0.5–2.0 dB over local PCA at target dimension 8, and 2.3–3.0 dB over a global PCA, at 0.3–0.8
    bpp. The TSP abstract reports about 0.5 dB over global coders on SAR images, and that the DCT generalised better
    to new images than the COT.

### 1.5 What links these objectives [my arithmetic, from standard high-resolution facts]

At high rate a Gaussian cluster coded with its own KLT and optimal bit allocation has distortion per coordinate
proportional to 2^(−2R) · (det Σ_c)^(1/n), the geometric mean of the eigenvalues; k-means instead charges the
arithmetic mean tr Σ_c / n. So at fixed total rate the coding-cost partition minimises Σ_c n_c · log det Σ_c plus the
membership bits, which is the m ≫ n part of Ma et al.'s L^s, while k-means minimises Σ_c n_c · tr Σ_c. The two agree
only when every cluster is isotropic. (Transform coding gain as the ratio of arithmetic to geometric mean is
textbook, for example Gersho and Gray, *Vector quantization and signal compression*, 1992 [cited, unchecked].)

The index's own cost is neither. Its expected certified width per group is governed by F = Σᵢ √γᵢ over the
eigenvalues of Σ_Q^½ Σ_R Σ_Q^½ (notation 0.2), with Σ_R pooled over the group, and the cut and key are governed by
each cluster's radius. I found no published partition objective that charges Σ √γᵢ or a query-weighted spectrum of
this form.

### 1.6 Verdict for item 1

- **Established.** The log-det coding length and its segmentation objective, with concavity in the soft membership
  so hard partitions suffice asymptotically (Ma et al. 2007). The ECVQ encoder rule d(x, yᵢ) + λℓᵢ is the necessary
  condition for a Lagrangian-optimal variable-rate partition (Chou et al. 1989, as stated in Gray and Neuhoff). The
  high-resolution identity "transform coding cost is a geometric mean" (textbook).
- **Only empirical.** That a coding-optimal partition beats k-means at equal cells: Archer and Leen, 1.1–1.8 dB on two
  image sets; Effros and Chou, up to 2 dB over a DCT system. Ma et al.'s greedy merge has no guarantee. Tipping and
  Bishop's 10% is on one image.
- **Not done, as far as I found.** A partition objective for inner-product bounds rather than reconstruction error:
  none charges the query-weighted spectrum Σ√γᵢ, a cluster radius, or a key-range length. No coding-cost partition
  has been tried on embedding-scale data (millions of rows, n in the hundreds) with a comparison at equal K against
  k-means; the largest experiment I found is Archer and Leen's, a few tens of thousands of 64-dimensional blocks.

---

## 2. High-resolution theory: the best density and shape of cells

Local symbols: k is the dimension of the quantiser (n in this index), f the source density, λ(x) the point density
(cells per unit volume, normalised), M(x) the inertial profile (the normalised second moment of the cell at x),
G_k the least normalised second moment of a k-dimensional tessellating polytope, R the rate per dimension, B(x) an
input-dependent weight matrix.

**Main source: R. M. Gray, D. L. Neuhoff, "Quantization", IEEE TIT 44(6):2325–2383, 1998. Read: the paper (a
course copy of the published PDF, text extracted locally), §§ II.B–II.D, IV.A–IV.B, the high-resolution section
and its non-difference-distortion subsection.** The extraction lost most mathematical symbols, so the formulas below
are those I know to be the standard forms the survey states in words; each is marked accordingly.

### 2.1 What the survey states

- **Bennett's integral** (vector form by Gersho 1979 for congruent cells, by Na and Neuhoff 1995 for varying
  shapes): D ≈ ∫ M(x) λ(x)^(−2/k) f(x) dx · 2^(−2R) [form cited, unchecked; the survey's four validity conditions
  read in full: cells small enough that f is nearly constant on each, point density and inertial profile matched
  on a high-probability set, and adjacent cells of similar volume].
- **Gersho's conjecture (1979)**, in the survey's words: "when rate is large, most cells of a k-dimensional
  quantizer with rate and minimum or nearly minimum MSE are approximately congruent to some basic tessellating
  k-dimensional cell shape". With that, M is a constant G_k and Hölder's inequality gives the **fixed-rate optimal
  point density** λ*(x) ∝ f(x)^(k/(k+2)) and the Zador–Gersho formula D ≈ G_k · ‖f‖_(k/(k+2)) · 2^(−2R) [the
  exponent k/(k+2) is standard; cited, unchecked against the survey's eq. (29), whose symbols were lost]. The survey
  then draws the **partial distortion theorem**: cells are smaller where f is larger, and every cell contributes
  roughly the same distortion.
- **Status of the conjecture.** "Gersho's conjecture is known to be true only for k = 1 and 2" (k = 2 by Fejes
  Tóth 1959, the hexagon). For k = 3 the best *lattice* is body-centred cubic (truncated octahedron), not proved
  best among tessellations. Zador's own result does not depend on the conjecture: the constant exists and is
  independent of the source, by a construction that tiles small equal cubes and fills each with the best quantiser
  for a uniform density. Zamir and Feder showed directly that the constants tend to 1/(2πe), the normalised second
  moment of a high-dimensional ball.
- **Variable rate (entropy-coded), the case of a rate cost.** Under the same conjecture, "the best point density is
  uniform on all of ℝ^k (or at least over the support of the source density). In other words, all quantizer cells
  have the same size". The distortion becomes D ≈ G_k · 2^(−2(R − h̄)), with h̄ the dimension-normalised differential
  entropy [standard form, cited, unchecked]. Zador's thesis had the wrong variable-rate formula; his 1966 memo gave
  the correct one. Gish and Pierce (1968): in the scalar case a uniform quantiser with entropy coding is within
  1.53 dB (0.255 bit) of the Shannon bound.
- **Input-weighted quadratic distortion** d(x, y) ≈ (x − y)ᵀ B(x) (x − y) (Gardner and Rao 1995; Li, Chaddha and
  Gray 1999; Linder, Zamir and Zeger 1999, multidimensional companding). The survey states that Bennett's integral
  extends, gives the optimal point densities for both fixed and variable rate, and stresses: "the optimal point
  density for the entropy-constrained case is not in general a uniform density." The forms I recall are
  λ*(x) ∝ [f(x) · det B(x)^(1/k)]^(k/(k+2)) for fixed rate and λ*(x) ∝ det B(x)^(1/2) for variable rate, with the
  optimal cell at x the MSE-optimal shape mapped by B(x)^(−½) [cited, unchecked: symbols lost in extraction].
- **When high-resolution theory applies.** The survey: "At low rates (say 1 bit per sample or less), one has no
  choice but to look to rate distortion theory."

### 2.2 What changes when the cost is a rate (a logarithm) and not a squared error

Stated as the survey and the ECVQ literature give it, not derived here.

1. With distortion fixed and rate minimised (variable rate, ECVQ), the optimal cells are all the same size, not
   denser where the data is denser, so the fixed-rate "cells shrink where f is large" no longer holds; the gain from
   matching density moves into the entropy code of the index. The encoder rule acquires the −λ log pᵢ term
   (section 1.3).
2. The rate is a log, so the per-cell cost of a Gaussian cell is ½ log det of its covariance against the distortion,
   and the arithmetic-mean (trace) cost of k-means is replaced by the geometric mean (section 1.5).
3. With an input-dependent quadratic weight, cells take a shape set by the local weight, and even the
   entropy-coded density is non-uniform, scaling with det B(x)^(½).

### 2.3 Relevance to this index, and a caution

The index has K ≈ 1,000 cells in n = 768, which is log₂(1000)/768 ≈ 0.013 bits per dimension [my arithmetic]. That
is the extreme low-rate end, far outside the high-resolution regime, where the survey says rate–distortion theory, not
Bennett's integral, is the right guide. The cluster partition is the first stage of a multi-stage code (the levels
code the residual), so the relevant question is a successive-refinement or two-stage one, which is what Effros and
Chou's two-stage universal codes address (section 1.4). The quantity the index cares about at query time is
|q·e| for a query distribution, which is a weighted quadratic with B = Σ_Q in expectation; that is the setting of
§2.1's input-weighted results, and also of anisotropic VQ (section 3.2).

### 2.4 Verdict for item 2

- **Established.** Zador's asymptotics (fixed and variable rate) without Gersho's conjecture; the conjecture proved
  in dimensions 1 and 2 only; uniform cell size optimal for entropy-coded quantisation under the conjecture;
  constants tend to 1/(2πe); extension to input-weighted quadratic distortions with non-uniform optimal density.
- **Only empirical.** Nothing in this item; it is all asymptotic theory.
- **Not done, as far as I found.** High-resolution theory gives no guidance on choosing ~10³ cells in ~10³ dimensions,
  and I found no high-resolution or low-rate analysis of the cell shape that minimises an inner-product *bound* (a
  support function or a radius) rather than an expected squared error.

---

## 3. Partitions in inner-product and nearest-neighbour indexes

Local symbols: B is a number of bins (cells) when a paper says "bins"; r = x − x̃ is a quantisation residual, split
into r_∥ (along x) and r_⊥; T is a score threshold; λ_S is SOAR's exponent (λ in the paper).

### 3.1 What inverted-file indexes use

- **k-means as the coarse quantiser** is the standard: Jégou, Douze, Schmid, "Product quantization for nearest
  neighbor search", IEEE TPAMI 33(1), 2011 (IVFADC) [cited, unchecked: the citation and its role as the k-means
  baseline are confirmed by Dong et al. 2020 §1, which calls it the baseline that "yields very high-quality results
  for indexing"]. Spherical k-means for cosine and inner-product data is an option in the standard libraries and is
  studied against standard k-means for IVF retrieval in Bruch, Nardini, Ingber, Liberty, "Bridging dense and sparse
  maximum inner product search", arXiv:2309.09013, 2023 [abstract: the comparison exists; no numbers on the page].
- **Balanced and hierarchical variants.** SPANN (Q. Chen, B. Zhao, H. Wang, M. Li, C. Liu, Z. Li, M. Yang,
  J. Wang, NeurIPS 2021, arXiv:2111.08566) **[abstract]**: hierarchical balanced clustering "to balance the length of
  posting lists", posting lists augmented with "the points in the closure of the corresponding clusters" (boundary
  duplication), and query-aware dynamic pruning of posting lists at search time; claimed about 2× faster than
  DiskANN at 90% recall at equal memory on three billion-scale sets. The details of the closure rule (I recall a
  relative-neighbourhood-graph test on which extra clusters to copy a point into) are [cited, unchecked].
- Balance is motivated by cost control, not by a quality argument: Dong et al. (§1) note that balanced bins "allow
  tighter control of the number of candidates simply by varying the number of retrieved parts".

### 3.2 Anisotropic vector quantisation (ScaNN) and SOAR

**Guo, Sun, Lindgren, Geng, Simcha, Chern, Kumar, "Accelerating large-scale inference with anisotropic vector
quantization", ICML 2020, arXiv:1908.10396. Read: the paper, §3–4 (PDF text extracted locally).**

- **Score-aware loss** (§3): ℓ(xᵢ, x̃ᵢ, w) = E_q[ w(⟨q, xᵢ⟩) · ⟨q, xᵢ − x̃ᵢ⟩² ] for a weight w on the true score
  [definition as I read it; the extraction lost part of the display].
- **Theorem 3.2:** for q uniform on the unit sphere in ℝ^d, ℓ = h_∥(w, ‖xᵢ‖)·‖r_∥‖² + h_⊥(w, ‖xᵢ‖)·‖r_⊥‖², with
  h_∥ = (d − 1) ∫₀^π w(‖xᵢ‖ cos θ)(sin^(d−2) θ − sin^d θ) dθ and h_⊥ = ∫₀^π w(‖xᵢ‖ cos θ) sin^d θ dθ.
- **Theorem 3.3:** if w(t) = 0 for t < 0 and w is non-decreasing for t ≥ 0, then h_∥ ≥ h_⊥, with equality iff w is
  constant on [−‖xᵢ‖, ‖xᵢ‖]. So parallel error is weighted more heavily.
- **Theorem 3.4:** for w(t) = 𝟙(t ≥ T), η = h_∥/h_⊥ satisfies lim_{d→∞} η/(d − 1) = (T/‖xᵢ‖)² / (1 − (T/‖xᵢ‖)²).
  At T = 0 the two are weighted equally.
- **Definition 4.1, the anisotropic VQ problem:** choose k codewords c₁, …, c_k to minimise
  Σᵢ min_j [ h_{i,∥} ‖r_∥(xᵢ, c_j)‖² + h_{i,⊥} ‖r_⊥(xᵢ, c_j)‖² ], solved by a Lloyd-type alternation (assign by the
  anisotropic loss; update each codeword by minimising its cell's loss, a linear solve). The paper notes that with
  h_∥ = h_⊥ this is exactly k-means.
- The paper's own contribution is to quantisation (scoring), not partitioning; it says the partition step is a
  separate task. SOAR's appendix reports that the partitions in its experiments were trained with the anisotropic
  loss (below), so in ScaNN the anisotropic objective is used for the coarse partition too.
- **Relevance.** The loss is the expected squared inner-product error for a query distribution, weighted toward
  high scores. It is the nearest published objective to a threshold certificate, but it is an expectation, not a
  bound, and it assumes isotropic queries (Σ_Q ∝ I) in its closed form.

**SOAR: P. Sun, D. Simcha, D. Dopson, R. Guo, S. Kumar, "SOAR: improved indexing for approximate nearest neighbor
search", NeurIPS 2023, arXiv:2404.00774. Read: the paper (arXiv HTML), §3 and the experiments.**

- Each point gets a primary cell (residual r) and one **spilled** secondary cell chosen to minimise
  L(r′, r, Q) = E_q[ w(cos θ) · ⟨q, r′⟩² ] with w(t) = |t|^λ_S, θ the angle between q and r.
- **Theorem 3.1:** for q uniform on the sphere this reduces to ‖r′‖² + λ_S ‖proj_r r′‖²; λ_S = 0 gives the plain
  second-nearest assignment. **Lemma 3.2:** ‖proj_r r′‖ equals ‖r′‖ times the correlation of the two cells' score
  errors, so the loss pushes the secondary residual to be orthogonal to the primary and decorrelates the errors.
- **Evidence:** datapoints read for 95% recall fall by 1.14× (GloVe-1M), 3.36× (MS SPACEV) and 4.32× (MS
  Turing-ANNS) against no spilling; about 2× throughput against a non-spilled index; index size +7.7% (GloVe-1M),
  +16.8% and +17.3% (billion-scale). Partitions: 2,000 for GloVe-1M (λ_S = 1), about 7.2M at billion scale
  (λ_S = 1.5).

### 3.3 Locally optimised product quantisation

Y. Kalantidis, Y. Avrithis, "Locally optimized product quantization for approximate nearest neighbor search", CVPR
2014, pp. 2321–2328. **Read: the abstract (CVF open-access page) and a search summary.** On an existing coarse
partition (inverted file or multi-index), each cell gets its own product quantiser for the residuals, optimised
locally in both the rotation and the subspace decomposition, with a parametric (Gaussian) solution: PCA alignment
then eigenvalue allocation across subspaces, as in OPQ's parametric form. Claimed state of the art on public sets
including a billion-scale one. The partition itself is k-means; LOPQ adapts the code to the cell, not the cell to
the code. This is the closest published analogue of this index's per-group basis, with the difference that LOPQ
fits per cell and this index per group of about 16 cells.

### 3.4 Learned partitions from the neighbour graph

- **Neural LSH.** Y. Dong, P. Indyk, I. Razenshteyn, T. Wagner, "Learning space partitions for nearest neighbor
  search", ICLR 2020, arXiv:1901.08544. **Read: the paper (PDF text extracted locally), §1 and the appendix
  figures.** Build the k-NN graph of the data, partition it into B balanced parts with few cut edges (KaHIP), then
  train a classifier (a neural network, or a regression tree for the hierarchical variant) to extend the partition
  to all of ℝ^d so that queries can be located. Motivated by Andoni, Naor, Nikolov, Razenshteyn, Waingarten (STOC
  and FOCS 2018), who show that balanced sparse cuts exist in the neighbour graphs of general metrics. The appendix
  proves a spectral statement: a k-NN graph with a sparse balanced cut implies a coordinate direction along which
  the points split well. **Evidence:** GloVe and SIFT (1M), 16 and 256 bins, one and two levels; "in the high
  accuracy regime, our framework yields partitions that lead to processing up to 2.3× fewer candidates than the
  strongest baseline", with k-means the baseline named; also beats LSH, ITQ, PCA trees, RP trees and Neural
  Catalyser.
- **BLISS.** G. Gupta, T. Medini, A. Shrivastava, A. J. Smola, KDD 2022, pp. 486–495 (preprint IRLI,
  arXiv:2103.09944). **Read: search summary of the paper and slides.** Learns buckets from query–item relevance
  data and re-partitions iteratively; each item goes to the least-loaded of its top-K predicted buckets (power of K
  choices), with a load-balance guarantee "with high probability under benign assumptions". Claimed 100× less RAM
  than HNSW at similar latency, and similar to FAISS-IVF at 3–4× less memory.
- **Graph partitioning against k-means at scale.** L. Gottesbüren, L. Dhulipala, R. Jayaram, J. Łącki, "Unleashing
  graph partitioning for large-scale nearest neighbor search", arXiv:2403.01797, 2024. **Read: the paper (arXiv
  HTML), summarised by the fetch tool.** Objective: minimise the edge cut |{(u, v) ∈ E : Π(u) ≠ Π(v)}| of a 10-NN
  graph subject to every part having at most (1 + ε)|P|/s points, ε = 5%; an approximate graph suffices ("even
  low-quality graphs (graph recall < 0.3) lead to high query recall"). Overlapping variants replicate boundary points
  with overlap 1.2. **At equal shard counts** (40, or 48 with overlap at equal maximum size) on DEEP, Turing and
  Text-to-Image (about 10⁹ points each): up to 2.14× higher QPS at 90% recall@10 than the best competitor (1.19× DEEP,
  1.46× Text-to-Image, 2.14× Turing; on Turing 271.9K against 124K QPS for balanced k-means); recall in the top-ranked
  shard +25% over balanced k-means on Turing. Routing by a single centre per shard was poor ("only 28% recall in the
  top-ranked shard on the MS Turing dataset"); they route with a hierarchical k-means index over each shard's points.
  On SIFT1M and GloVe at 16 shards, BLISS was "completely outperformed". This is distributed sharding (tens of
  shards), not an IVF with thousands of cells.

### 3.5 Query-aware partitioning

- **LIRA** (arXiv:2503.23409, 2025) **[paper, via the fetch tool]**: keeps the k-means partition, learns a probing
  model from training queries' true neighbours, and copies the points the model predicts to be "long-tail" into a
  second partition. On SIFT at 64 partitions, distance computations fall by 29.8% at its recall target; at 1,024
  partitions mixed results (fewer probes, QPS from 0.93× to 1.9×). Not an equal-redundancy comparison.
- **Cluster with Auctions** (S. Bessa, P. Fernandez, G. Szilvasy, M. Douze, H. Jégou, arXiv:2607.13728, July 2026)
  **[abstract]**: jointly learns a balanced partition and a neural probing function for a query distribution,
  alternating gradient steps on the prober with an auction algorithm for the assignment; "up to 4.7× throughput over
  the state-of-the-art at equal recall" when queries and data differ in distribution. The abstract does not name
  k-means as a baseline.
- **Quake** (arXiv:2506.03437; already read in `codes-and-search.md` §2.3) splits and merges partitions under a cost
  model of sizes and access frequencies. **CrackIVF** (arXiv:2503.01823) builds partitions incrementally from the
  queries seen [search summary].

### 3.6 Verdict for item 3

- **Established (proved).** Anisotropic loss decomposition and h_∥ ≥ h_⊥ for monotone weights (Guo et al. 2020,
  Theorems 3.2–3.4); SOAR's reduction to ‖r′‖² + λ‖proj_r r′‖² and its correlation lemma (Theorem 3.1, Lemma 3.2).
  Existence of sparse balanced cuts in neighbour graphs (Andoni et al. 2018, as cited by Dong et al.).
- **Only empirical.** Every gain over k-means: Neural LSH up to 2.3× fewer candidates (16–256 bins, 1M points);
  graph partitioning up to 2.14× QPS at about 40 shards; SOAR 1.1–4.3× fewer points read; LIRA about 30% fewer
  distance computations at 64 cells. No result says k-means is worse in the regime of thousands of cells with a
  scan of every candidate.
- **Not done, as far as I found.** Graph-partition or query-aware partitions evaluated at K ≈ N/1000 for a
  *certifying* index, where the cost is the number of rows a bound cannot decide rather than recall; any partition
  objective that uses the anisotropic loss with a non-isotropic Σ_Q.

---

## 4. How many cells and how many local bases

Local symbols: K cells, N rows, n the ambient dimension, d_int an intrinsic dimension, s the training sample size.

### 4.1 The √N rule for inverted files

- **Faiss guidelines** (wiki page "Guidelines to choose an index") **[page read]**: for IVF on datasets below 1M,
  "K is 4*sqrt(N) to 16*sqrt(N)", with "between 30*K and 256*K vectors for training (the more the better)". No
  justification is given on the page.
- **RAPIDS cuVS** documentation **[search summary]**: defaults to √N lists and calls it "a good heuristic", found
  empirically.
- **The usual justification** is a cost balance, which I found stated only in blogs and in one search summary, never
  as a published result: a query pays about K centroid comparisons plus about nprobe·N/K row scans, minimised at
  K ≈ √(nprobe·N) [my arithmetic]. It ignores imbalance, recall, and any per-cell setup.
- This index's rule, clusters of about 1,000 rows (K ≈ N/1000), agrees with √N only at N = 10⁶. At 10⁸ rows √N gives
  10⁴ cells of 10⁴ rows, while this index's rule gives 10⁵ cells of 10³ rows [my arithmetic].

### 4.2 Scaling of quantisation error with K and intrinsic dimension

- **Zador's theorem** (section 2): fixed-rate distortion of the best K-cell quantiser of a density on ℝ^n falls as
  K^(−2/n).
- **On a d_int-dimensional manifold** the rate becomes K^(−2/d_int): Gruber, "Optimum quantization and its
  applications", Adv. Math. 186, 2004 [search summary: a constant times (∫ weight)^((d+2)/d) times K^(−2/d) on a
  Riemannian d-manifold; formula garbled in the snippet]; Iacobelli, "Asymptotic quantization for probability
  measures on Riemannian manifolds", ESAIM COCV 22(3), 2016, arXiv:1412.4026 **[abstract]**: K^(p/d) V_{K,p}(μ)
  converges to the cube constant times the L^(d/(d+p)) norm of the density, under a growth condition that depends on
  curvature and reduces to a moment condition in the flat case, and is sharp.
- **Training sample.** Bartlett, Linder, Lugosi, "The minimax distortion redundancy in empirical quantizer design",
  IEEE TIT 44(5):1802–1813, 1998 **[abstract and summary]**: for any design algorithm, the excess distortion over the
  optimum is Ω(s^(−½)) for some distribution on a bounded set, and the minimax rate is of order s^(−½). The
  dependence on K and n (I recall √(Kn/s) up to logs in the upper bound) is [cited, unchecked]. This bears on the
  index's 200 sample rows per centre.

### 4.3 How many local bases

- In the transform-coding papers the number of regions is chosen by experiment: Archer and Leen use 16 and 32
  regions; Effros and Chou size the collection of transforms by the rate–distortion Lagrangian, so the codebook of
  transforms is paid for in bits (section 1.4). Ma et al. (2007) choose the number of groups through ε alone, and
  the (m + n) factor in eq. (6) charges each group n·R bits for its basis, which is a principled price on a local
  basis (section 1.1). Mixtures of PPCA fix the number of components in advance.
- Estimating a basis needs enough rows: a sample covariance of sub-Gaussian vectors in ℝ^n is accurate in operator
  norm to relative error δ with about n/δ² samples (Vershynin, "How close is the sample covariance matrix to the
  actual covariance matrix?", J. Theor. Probab. 25, 2012) [cited, unchecked]. That is the standard reason for a floor
  of a constant times n rows per basis, as the index's 20n per group is.
- LOPQ fits one rotation per cell, so its number of bases equals K [abstract]. I found no paper that optimises the
  number of bases separately from the number of cells, as this index's two-level (cluster, group) structure does,
  except that the inverted multi-index and two-level IVF designs separate a coarse and a fine level for routing, not
  for bases.

### 4.4 Verdict for item 4

- **Established.** K^(−2/n) (Zador) and K^(−2/d_int) on manifolds (Gruber; Iacobelli) for the expected squared error;
  s^(−½) minimax training redundancy (Bartlett, Linder, Lugosi); covariance estimation from O(n) samples.
- **Only empirical.** The √N rule and its 4–16 multipliers; the regions in transform coders.
- **Not done, as far as I found.** No published derivation of K (or of the number of bases) from a cost model of a
  certifying index, where a larger K narrows each cell's cut and key range but adds per-cell setup and table cost. No
  result relating the optimal K to the intrinsic dimension of the data for search cost, as opposed to distortion.

---

## 5. Partitions for exact search with bounds: which cell shape prunes best

Local symbols: R_c is a cell's covering radius (largest distance from its centre to a member), p₀ a ball's centre.

### 5.1 Ball and cone bounds for inner products

P. Ram, A. G. Gray, "Maximum inner-product search using cone trees", KDD 2012, pp. 931–939 (arXiv:1202.6101,
"Maximum inner-product search using tree data-structures"). **Read: the arXiv version (ar5iv HTML), §3–4 and Table
3, through the fetch tool.**

- **Theorem 3.1:** for a ball with centre p₀ and radius R_p, max over p in the ball of ⟨q, p⟩ ≤ ⟨q, p₀⟩ + R_p‖q‖.
- **Theorem 4.2 (cone–ball, dual tree):** the inner product of any p in the ball and q in a cone of axis q₀ and
  half-aperture ω_q is at most ‖p₀‖·cos({|ϕ| − ω_q}₊) + R_p, with ϕ the angle between p₀ and q₀.
- Tree: split by two far-apart pivots (a random point's farthest point A, then A's farthest point B), assign each
  point to the nearer pivot, leaves of at most 20 points. The paper makes no claim about which split gives the
  tightest bounds; it notes balls need not nest and that its dual bound is "fairly loose".
- **Evidence:** exact MIPS speed-ups over a linear scan from 1.13× to over 10⁵× across 15 datasets, with the high
  speed-ups on low-dimensional data and about 2× on the recommender sets (Netflix 1.98×, Y!-Music 2.11×).
- The bound's slack is R_p‖q‖, so what prunes is the **covering radius**, the largest member's distance, not the
  mean squared distance that k-means minimises. This index's cut has the same structure: β times the stored extremes
  plus or minus ρ times the largest item norm (`SYSTEM.md` §2.2).

### 5.2 Compact partitions in metric spaces: the list of clusters

E. Chávez, G. Navarro, "A compact space decomposition for effective metric indexing", Pattern Recognition Letters
26(9), 2005. **Read: the paper (authors' PDF, text extracted locally), §5–8.**

- Structure: a list of (centre, covering radius, bucket); the first centre takes the points within its radius or its
  m nearest, the rest are recursed on. Two radius rules: **fixed radius** (balls get emptier down the list) and
  **fixed size** (m elements each, so the radii grow down the list).
- Centre heuristics (p1)–(p5). For fixed size, the heuristics that choose the next centre far from the previous ones
  "and therefore minimize the intersection among partitions" were best; (p5), maximising the sum of distances to
  previous centres, is used thereafter. For fixed radius the best rule changes with the radius, and "a relatively
  small change in the fixed radius yields a very large difference in performance"; fixed size is easier to tune.
- **Claim on shape and balance:** "In exact searching, balanced data structures are always best. We show how this
  ceases to be true in metric spaces as the intrinsic dimension grows"; the list is an extremely unbalanced tree. The
  argument is from the distance histogram: a query intersects the ring [s − r, s + r], and in high dimension the mass
  of the histogram concentrates, so a cut should be placed in the tails, which makes the partition unbalanced.
- **Evidence:** uniform data in the unit cube, dimensions 4 to 20, 100,000 points, queries retrieving 0.01% to 0.1%;
  the list beats bk-, gna- and sa-trees, fq-arrays and LAESA at equal memory in high dimension. Synthetic only.
- The companion survey, Chávez, Navarro, Baeza-Yates, Marroquín, "Searching in metric spaces", ACM Comput. Surv.
  33(3), 2001, defines the intrinsic dimension of a distance histogram as μ²/(2σ²) and argues that compact
  (clustering) partitions degrade more gracefully than pivot schemes as it grows [cited, unchecked].

### 5.3 Pivots and annuli

- **iDistance** (H. V. Jagadish, B. C. Ooi, K.-L. Tan, C. Yu, R. Zhang, ACM TODS 30(2):364–397, 2005)
  **[search summary]**: partition the data, give each part a reference point, key each point by its distance to
  that reference, index the keys in a B⁺-tree; a k-NN query becomes one-dimensional range searches (annuli). The
  paper says effectiveness depends on how the data are partitioned and how reference points are chosen; cluster
  centres as references are reported best (Wikipedia summary, unchecked). **This is the published ancestor of this
  index's key:** rows sorted within a cluster by a scalar against the centre, searched as one contiguous range.
- **Hyperplane bounds for Voronoi cells.** S. Ramaswamy, K. Rose, "Adaptive cluster distance bounding for
  high-dimensional indexing", IEEE TKDE 23(6), 2011 (accepted draft read). **Read: the paper (draft, text extracted
  locally), §§ I, V–VI.** For Voronoi clusters, bound the query–cluster distance by the distance to the separating
  hyperplanes between the query's cell and the cluster, instead of a bounding sphere or rectangle: "hyperspheres and
  hyperrectangles are generally not optimal bounding surfaces for clusters in high dimensional spaces". With few,
  large clusters "the sphere (MBS) bound is almost zero for the first 6–8 clusters". Clustering is plain k-means or
  GLA re-assigned to the nearest centroid; "while it is possible to also optimize the clustering itself, that is not
  our goal". This index's support table (exact extremes of anchor·x per cluster, for every anchor) is a polyhedral
  outer bound of the same family.

### 5.4 Diameter against variance in partition trees

- **Verma, Kpotufe, Dasgupta, "Which spatial partition trees are adaptive to intrinsic dimension?", UAI 2009**
  (arXiv:1205.2609). **Read: abstract and poster via a search summary.** The measure is how many levels it takes to
  halve the cells' diameters. Axis-parallel rules (k-d, dyadic) do not always adapt to intrinsic dimension, with
  matching lower bounds; RP, PCA and 2-means trees do. In their vector-quantisation experiments 2-means and PCA
  trees had the lowest quantisation error.
- Dasgupta and Freund, "Random projection trees and low dimensional manifolds", STOC 2008: the RP tree with a
  "split by distance" step shrinks the average diameter at a rate set by the intrinsic dimension [cited, unchecked].
- **Covering-radius objective.** k-centre (minimise the largest radius) has the farthest-first 2-approximation
  (Gonzalez 1985) and is hard to approximate better than 2 (Hsu and Nemhauser 1979) [cited, unchecked]. M-tree split
  policies that minimise the larger of two covering radii were found best by Ciaccia, Patella and Zezula (VLDB 1997)
  [cited, unchecked].

### 5.5 Verdict for item 5

- **Established.** The ball bound q·p₀ + R‖q‖ (Ram and Gray, Theorem 3.1) and its cone variant: the slack is set by
  the covering radius. The 2-approximation for k-centre. Adaptivity of 2-means, PCA and RP trees to intrinsic
  dimension in the diameter sense (Verma et al.).
- **Only empirical.** Fixed-size better than fixed-radius lists and unbalanced better than balanced in high
  dimension (Chávez and Navarro, synthetic cubes up to dimension 20); hyperplane bounds better than spheres
  (Ramaswamy and Rose); iDistance's dependence on reference points.
- **Not done, as far as I found.** A comparison, at equal K, of partitions chosen for covering radius (k-centre)
  against k-means (variance) for the pruning of an exact or certified index in hundreds of dimensions. No published
  analysis of how the gap between a cluster's largest and its root-mean-square residual norm behaves for k-means
  cells in high dimension, which is what separates the two objectives for a cut like this index's.

---

## 6. Partition quality and certified inner-product bounds

The question is whether anyone has tied the choice of partition to the width of a guaranteed bound on q·x, as
opposed to recall.

- **Ram and Gray (2012)** tie pruning to the covering radius through a sound bound (section 5.1), but build the tree
  by a fixed two-pivot rule and do not optimise it for the bound.
- **LEMP:** C. Teflioudi, R. Gemulla, O. Mykytiuk, "LEMP: fast retrieval of large entries in a matrix product",
  SIGMOD 2015, pp. 107–122; extended as "Exact and approximate maximum inner product search with LEMP", ACM TODS,
  2016. **Read: search summaries of the paper, slides and a citing paper.** Solves exactly the threshold problem
  (all pairs with qᵀp ≥ θ) as well as top-k. It **partitions by vector length**: buckets of similar norm, each
  reduced to a cosine search with local threshold θ′ = θ/(‖q‖‖p‖); short buckets are pruned whole. Reported up to
  24× faster than exact indexing alternatives (as cited in arXiv:1706.01449). The partition is chosen for a sound
  bound, but on the norm, which is constant for unit vectors.
- **FEXIPRO:** H. Li, T. N. Chan, M. L. Yiu, N. Mamoulis, SIGMOD 2017, pp. 835–850. **Read: search summary.** Exact
  top-k MIPS by a sequential scan with an SVD transform so that leading coordinates carry most of the score,
  Cauchy–Schwarz bounds on the tail after a few coordinates, integer approximations for fast upper bounds, and a shift
  to positive values. No partition; it is the nearest published relative of this index's prefix levels.
- **Ramaswamy and Rose (2011)** give sound, cluster-adaptive distance bounds and explicitly leave the clustering
  unoptimised (section 5.3).
- **Anisotropic VQ and SOAR** (section 3.2) choose the partition by the *expected* squared score error weighted
  toward high scores; neither gives a bound.
- **Archer and Leen; Effros and Chou** (section 1.4) choose the partition by the *coded* distortion, which is the
  right shape of idea (assign each row to the cell whose coder serves it best) with the wrong cost (squared error,
  not bound width).
- The certified-search papers of 2025–2026 listed in `theory.md` §3.4 bound per-vector errors on top of an ordinary
  k-means IVF; none that I found there or here chooses the partition for the bound.

### 6.1 Verdict for item 6

- **Established.** Sound bounds whose slack depends on a partition statistic: the covering radius (ball bound), the
  vector norm (LEMP), the separating hyperplanes (Ramaswamy and Rose).
- **Only empirical.** That these bounds prune well on some datasets; none compares partitions for bound width.
- **Not done, as far as I found.** A partition objective whose value is the width, or the expected number of
  undecided rows, of a certified inner-product bound; any theorem relating a partition's quality to a certified
  bound; an assignment rule in the style of Archer and Leen's coding-optimal partition or the ECVQ encoder ("send each
  row to the cell where its certified interval is narrowest, plus a price for the cell") for a certifying index. A
  search on these terms returned only k-centre, Elkan-style triangle-inequality k-means and bound-tightness papers
  that do not choose the partition.

---

## Bibliography and how each was read

1. Y. Ma, H. Derksen, W. Hong, J. Wright. Segmentation of multivariate mixed data via lossy data coding and
   compression. IEEE TPAMI 29(9):1546–1562, 2007. https://people.eecs.berkeley.edu/~yima/psfile/Ma-PAMI07.pdf — paper
2. Y. Yu, K. H. R. Chan, C. You, C. Song, Y. Ma. Learning diverse and discriminative representations via the
   principle of maximal coding rate reduction. NeurIPS 2020. arXiv:2006.08558 — paper
3. M. E. Tipping, C. M. Bishop. Mixtures of probabilistic principal component analysers. Neural Comput.
   11(2):443–482, 1999. https://www.cse.iitk.ac.in/users/piyush/courses/pml_winter16/mppca.pdf — paper (preprint)
4. P. S. Bradley, O. L. Mangasarian. k-Plane clustering. J. Global Optim. 16:23–32, 2000; P. Tseng. Nearest q-flat
   to m points. JOTA 105:249–252, 2000 — abstracts
5. R. Vidal, Y. Ma, S. Sastry. Generalized principal component analysis. IEEE TPAMI 27(12), 2005 — [cited, unchecked]
6. P. A. Chou, T. Lookabaugh, R. M. Gray. Entropy-constrained vector quantization. IEEE Trans. ASSP 37(1):31–42,
   1989 — abstract; conditions read in [8]
7. M. Effros, P. A. Chou. Weighted universal transform coding. ICIP 1995; M. Effros, P. A. Chou, R. M. Gray.
   Weighted universal image compression. IEEE TIP 8(10), 1999. https://authors.library.caltech.edu/7416 — abstracts
8. R. M. Gray, D. L. Neuhoff. Quantization. IEEE TIT 44(6):2325–2383, 1998.
   https://www.csd.uoc.gr/~hy438/lectures/Quantization.pdf — paper (symbols lost in extraction)
9. C. Archer, T. K. Leen. From mixtures of mixtures to adaptive transform coding. NIPS 13, 2000.
   https://proceedings.neurips.cc/paper_files/paper/2000/file/4888241374e8c62ddd9b4c3cfd091f96-Paper.pdf — paper;
   C. Archer, T. K. Leen. A generalized Lloyd-type algorithm for adaptive transform coder design. IEEE TSP
   52(1):255–264, 2004 — abstract
10. R. Guo, P. Sun, E. Lindgren, Q. Geng, D. Simcha, F. Chern, S. Kumar. Accelerating large-scale inference with
    anisotropic vector quantization. ICML 2020. arXiv:1908.10396 — paper
11. P. Sun, D. Simcha, D. Dopson, R. Guo, S. Kumar. SOAR: improved indexing for approximate nearest neighbor search.
    NeurIPS 2023. arXiv:2404.00774 — paper
12. Y. Kalantidis, Y. Avrithis. Locally optimized product quantization for approximate nearest neighbor search. CVPR
    2014 — abstract
13. Y. Dong, P. Indyk, I. Razenshteyn, T. Wagner. Learning space partitions for nearest neighbor search. ICLR 2020.
    arXiv:1901.08544 — paper
14. G. Gupta, T. Medini, A. Shrivastava, A. J. Smola. BLISS: a billion scale index using iterative re-partitioning.
    KDD 2022 — summary
15. L. Gottesbüren, L. Dhulipala, R. Jayaram, J. Łącki. Unleashing graph partitioning for large-scale nearest
    neighbor search. arXiv:2403.01797, 2024 — paper via the fetch tool
16. Q. Chen et al. SPANN. NeurIPS 2021. arXiv:2111.08566 — abstract
17. LIRA, arXiv:2503.23409, 2025 — paper via the fetch tool; S. Bessa et al., Cluster with auctions for vector
    search, arXiv:2607.13728, 2026 — abstract; CrackIVF, arXiv:2503.01823 — summary
18. S. Bruch, F. M. Nardini, A. Ingber, E. Liberty. Bridging dense and sparse maximum inner product search.
    arXiv:2309.09013 — abstract
19. Faiss wiki, Guidelines to choose an index — page; RAPIDS cuVS IVF-Flat documentation — summary
20. P. M. Gruber. Optimum quantization and its applications. Adv. Math. 186, 2004 — summary; M. Iacobelli.
    Asymptotic quantization for probability measures on Riemannian manifolds. ESAIM COCV 22(3), 2016,
    arXiv:1412.4026 — abstract
21. P. L. Bartlett, T. Linder, G. Lugosi. The minimax distortion redundancy in empirical quantizer design. IEEE TIT
    44(5):1802–1813, 1998 — abstract
22. P. Ram, A. G. Gray. Maximum inner-product search using cone trees. KDD 2012. arXiv:1202.6101 — paper via the
    fetch tool
23. E. Chávez, G. Navarro. A compact space decomposition for effective metric indexing. Pattern Recogn. Lett. 26(9),
    2005. https://users.dcc.uchile.cl/~gnavarro/ps/prl04.pdf — paper
24. H. V. Jagadish, B. C. Ooi, K.-L. Tan, C. Yu, R. Zhang. iDistance. ACM TODS 30(2):364–397, 2005 — summary
25. S. Ramaswamy, K. Rose. Adaptive cluster distance bounding for high-dimensional indexing. IEEE TKDE 23(6), 2011.
    https://scl.ece.ucsb.edu/sites/default/files/publications/b10_1_0.pdf — paper (accepted draft)
26. N. Verma, S. Kpotufe, S. Dasgupta. Which spatial partition trees are adaptive to intrinsic dimension? UAI 2009 —
    summary of abstract and poster
27. C. Teflioudi, R. Gemulla, O. Mykytiuk. LEMP. SIGMOD 2015; TODS 2016 — summary
28. H. Li, T. N. Chan, M. L. Yiu, N. Mamoulis. FEXIPRO. SIGMOD 2017 — summary

---

## Claims in the brief that were misremembered or need correcting

1. **"Archer and Leen's optimal local transform coding."** Their papers are titled "From mixtures of mixtures to
   adaptive transform coding" (NIPS 2000) and "A generalized Lloyd-type algorithm for adaptive transform coder
   design" (IEEE TSP 2004); their own terms are the *coding-optimal partition* and the *coding-optimal transform*.
   The content matches the brief; the name does not.
2. **BLISS listed under "learned partitions from the nearest-neighbour graph".** BLISS learns its buckets from
   query–item relevance data with a power-of-K-choices load balance; it does not partition a neighbour graph. Neural
   LSH and Gottesbüren et al. do.
3. **MCR² as a partition objective "that follows" Ma et al.** It uses Ma et al.'s coding rate, but it is an objective
   for learning a representation (maximise R − R^c over the features), not for partitioning fixed vectors. The
   partition objective for fixed vectors remains Ma et al.'s L^s.
4. **"Cluster size rules, for example √N".** The √N rule is a rule for the *number* of cells (Faiss: 4√N to 16√N
   below 1M), equivalently √N rows per cell; it is a heuristic with no published derivation. This index uses about
   1,000 rows per cluster, which coincides with √N only at N ≈ 10⁶.
5. **Effros and Chou's weighted universal transform coding as "clustering".** It partitions *blocks of the source*
   (image regions) among a collection of KLT–bit-allocation pairs by the rate–distortion Lagrangian; it is a
   two-stage universal code, not a clustering of individual vectors, though the design step is Lloyd-like.
6. **Gersho's conjecture** is stated in the brief without status; it is proved only in dimensions 1 and 2 (Gray and
   Neuhoff), and Zador's asymptotics do not depend on it.

No author, year or title in the brief was wrong otherwise: Ma, Derksen, Hong, Wright 2007; Tipping and Bishop
(1999); Chou, Lookabaugh, Gray (1989); Kalantidis and Avrithis (CVPR 2014); Dong, Indyk, Razenshteyn, Wagner (ICLR
2020) all check.

## Not opened

Chou, Lookabaugh and Gray 1989 (no accessible copy; conditions taken from Gray and Neuhoff); Gray, Linder and Li 2002
(dead link); Effros and Chou 1995 and Effros, Chou and Gray 1999 (abstracts only); Archer and Leen 2004 (abstract
only); Vidal, Ma and Sastry 2005; Kambhatla and Leen 1997; Kalantidis and Avrithis 2014 (abstract only); SPANN and
BLISS full texts; Iacobelli 2016 and Gruber 2004 theorems (summaries only); Bartlett, Linder and Lugosi 1998 (the K
and n dependence); Vershynin 2012; iDistance full text; LEMP and FEXIPRO full texts; Chávez et al. 2001 survey;
Dasgupta and Freund 2008; Gonzalez 1985; Ciaccia, Patella and Zezula 1997. In Gray and Neuhoff the mathematical
symbols were lost in text extraction, so its displayed formulas (Bennett's integral, the point densities, the
weighted-distortion densities) are given in their standard forms and tagged unchecked.
