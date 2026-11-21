# 4. What a query must read: shells, exponents, hardness, scaling

This chapter asks how the work of one query grows with the number of rows N, and whether any index could make it grow
more slowly. Under the owner's metric, which fixes the share of the corpus that matches, an entropy bound settles the
question at once, so section 4.1 first fixes what "sublinear" can usefully mean. Section 4.2 collects the lower bounds
and says which of them bind on the data. Section 4.3 states the cost law of the exact path: each level of a certificate
is read by the rows in a shell below the threshold, and the growth of that shell with N is set by the tail of the
query's scores. Sections 4.4 to 4.6 give the measured exponents, the visible set and the dimension. The approximate
mode obeys a different law, measured in different units on different data, so section 4.7 states it apart and then
compares the two. Section 4.8 sets cluster size against N. Section 4.9 then holds N fixed and asks how one row's bytes
are best split between deterministic and randomised levels, what an item at a given margin must read, and what the
stored scalars of a coded block cost in time.

## Local symbols

The symbols of chapter 0 hold. This chapter adds the following, and gives the source's letter where it differs.

| Symbol | Meaning |
|---|---|
| W(N; k) | the work of one query at answer size k, counted in one of three units: centres read, rows touched (any byte of the row read), or coordinates (or bytes) read |
| β | an exponent of work in N, W = O(N^β) |
| p | the answer fraction k/N; in Theorem 4.1, the probability that a row matches |
| h₂(p) | binary entropy in bits, −p log₂ p − (1 − p) log₂(1 − p), as in chapter 2 (the sources write h(p)) |
| B_s, N_g | in Claim 4.9, the bits of a shared summary and the number of items sharing it, as in chapter 2's Corollary 2.4 (the source writes B and M) |
| H(·) | in Theorem 4.1 only, an entropy in **bits**, so that it compares with bits read (chapter 0's H is in nats) |
| η, η′, c_b, λ | in Proposition 4.2 only: the query-time saving N^(1−η) and the resulting OV saving N^(2−η′), the build-time exponent N^(c_b), and the piece size N^λ (the source writes δ, δ′, c, γ) |
| s_m | in the hashing bound, the matches' cosine against a background at cosine 0 (the source writes τ) |
| μ_D, σ_D | the mean and standard deviation of a distance histogram (the source writes μ, σ) |
| D′ | the dimension of a point set in the range-searching bounds (the source writes D) |
| C₁ | an unspecified constant in the on-manifold limit after Theorem 4.7 (the source writes C) |
| e_i | in Proposition 4.3, the margin by which row i exceeds every other row of its cluster in direction r̂_i |
| s_i | the score q·x_i of row i |
| 𝒩(τ) | the number of rows with s_i ≥ τ (the source writes n(τ)) |
| [L_{i,ℓ}, U_{i,ℓ}] | the sound interval for s_i after stage ℓ of a staged read |
| b_ℓ | bytes per row of stage ℓ |
| 𝒰_ℓ | the rows not stopped after stage ℓ, with 𝒰₀ every row (the source writes A_ℓ) |
| w⁻, w⁺ | bounds on the upper slack U_{i,ℓ} − s_i of the non-matches at stage ℓ |
| s* | the essential supremum of a query's scores over the corpus distribution |
| ϑ | the rate at which a slack shrinks, w ∝ N^(−ϑ) (the source writes η) |
| ν, z | the tail index at the top of the score range, F̄(s* − z) ∝ z^ν as z → 0 (the source writes a) |
| z_N | the height of t_N above the median, in the units of w, for a Gaussian tail |
| â | the key x·m̂ of row x in its cluster, m̂ = m/‖m‖, as in chapter 1, here taken exact (the source writes a) |
| n_int | the intrinsic dimension of the data (the source writes D) |
| C_exp | the expansion constant, the factor by which a ball's count grows when its radius doubles (the source writes c) |
| C_apx | the approximation factor of an approximate near-neighbour problem (the source writes c) |
| ξ_q, ξ_u | hashing exponents: query time N^(ξ_q) and space N^(1+ξ_u) (the source writes ρ_q, ρ_u) |
| V | a viewer's visible set |
| n̄ | rows per cluster, N/K (`layered.md` writes S) |
| u, v | the exponents of the approximate law, rows read ∝ n̄^u·N^v (`layered.md` writes γ, η) |
| Φ(K) | the share of the rows the approximate order reads at K clusters (`layered.md` writes φ(K)) |
| T, n_T, d_T | in §4.9, a set of eigen-coordinates (those of chapter 0's γᵢ), its size, and d_T = n_T − 1 |
| F_T | Σ_{i∈T} √γᵢ, chapter 0's F restricted to T |
| W_T(H) | in §4.9, chapter 2's floor on the mean half-width of a sound certificate of T with H nats, Σ_{i∈T} min(√γᵢ, λ_w) with Σ_{i∈T} ln⁺(√γᵢ/λ_w) = H (Lemma 2.21, Theorem 2.24); always subscripted, unlike the work W(N; k) (the source writes W_T and λ) |
| ω_T(H) | W_T(H)/F_T |
| h*(B) | the least half-width, as a share of F, over the layouts of §4.9.1 at B bits; B*(·) is its inverse |
| n_⋆(ũ) | the dimension in play: the bits that halve the width along the frontier at normalised margin ũ (the source writes k(u)) |
| η_i | in §4.9 only, always subscripted: item i's scale. In the law as stated it is the zero-bit Cauchy–Schwarz bound in the group's metric; in the dump test it is Σ_j ‖y_j‖‖Δ_j‖ over the four blocks after the 32-byte prefix (the sources write η; Proposition 4.2's unsubscripted η is unrelated) |
| ũ_i, w̃_ℓ, w̄_ℓ | item i's normalised margin \|s_i − t\|/η_i; level ℓ's half-width over η_i; the geometric mean of w̃_ℓ over a level's entrants (the sources write u_i, w and w̄; u is this chapter's exponent) |
| c_e | the ratio of a level's half-width to the sd of its estimate's error: c at a randomised level, about √n_eff at a deterministic one |
| J | the number of coded blocks at a level |
| b_blk | the bytes charged per coded block in the layout programme (the source writes s) |
| τ_rise, τ_lvl, τ_byte | the time of one (item, coded block, rise), of entering one level per item, and of decoding one byte of code per item (the source writes τ, τ_0, τ_byte; chapter 0's τ_k is unrelated) |
| e_56, e_96 | the shares of N entering the levels at 56 B and at 96 B of code |
| D, H_D, H_j | in §4.9.1, the coordinates a layout certifies deterministically, the nats of deterministic code on D and the nats of prefix on block T_j (chapter 0's D, a level's weights, does not appear in §4.9) |
| R, R_j, t(R) | in §4.9, a rate in bits per position (of block T_j), and t(R) = 1/(4^R − 1), the cap bound on tan²φ at rate R (chapter 0's R_j, a rotation, does not appear in §4.9; the threshold t appears there only in s_i − t) |
| ρ_c(R) | in §4.9, the built block code's tan φ over the sphere bound at one rate R; the subscript is a label, not a cluster index |
| B₀, B₁ | in §4.9.2, the bits at which a rotated block of T becomes narrower than its own zero-bit Cauchy–Schwarz bound, and than the same bits spent deterministically (the source writes B₀ and B*; B*(·) here is the inverse of h*) |
| Ê, ξ², Θ | in §4.9, chapter 3's estimate of Δ·r, and Theorem 3.12's ξ² = c²/Θ with Θ = Σ_j d_j/tan²φ_j |

The sources label claims **proved**, **measured**, **cited**, **argued**, **conjectured**, **derived** and
**estimated**. Here "argued" becomes [model] when the argument is a derivation under a stated model of the data, and
[sketch: gap named] when the source names a hole. "Derived" and "estimated" in `layered.md` become [model]. None of the
propositions of this chapter appears in `proofcheck/CONSOLIDATED_PROOFS.md` or `proofcheck/REFEREE.md`, so every proof
here is [proved, unrefereed].

Two query classes recur. **Item queries** are held-out image embeddings from the corpus's own distribution (SYSTEM.md
calls them in-distribution). **Text queries** are embeddings of species prompts from the model's text tower, searched
against the image embeddings, so they are out of distribution (OOD).

## 4.1 What "sublinear" should mean

The owner set the metric for the exact path [FINDINGS.md §1]: the percentage of the dataset that must be read to
guarantee that every row meeting the threshold has been identified, with probability above 1 − ε. The working
definition takes the dataset to be the f16 vectors (2n bytes a row), counts every byte touched including index and code
bytes, and divides by N·2n. The guarantee is the one of chapter 0, Pr_ω[S(t) ⊆ R ⊆ S(t − d)] ≥ 1 − ε over the build's
seed ω alone, for every q chosen independently of ω and every t and d ≥ 0, with d = 0 the primary figure
[FINDINGS.md §1, §2]. The event is simultaneous over all rows, and "for all q, with probability 1 − ε" does not hold.

The baseline on that metric, at 1M BioCLIP-2 rows (n = 768), with the approximations listed in FINDINGS.md §6.6, is
[measured; FINDINGS.md §1]:

| Threshold t | 0.95 | 0.9 | 0.8 | 0.7 |
|---|---|---|---|---|
| True matches, share of rows | 0.22% | 0.60% | 1.95% | 9.17% |
| Dataset read, d = 0, deterministic | 0.35% | 0.50% | 1.70% | 5.81% |
| Dataset read, d = 0.01 | 0.32% | 0.46% | 1.49% | 4.91% |
| Dataset read, d = 0.05 | 0.31% | 0.44% | 1.28% | 3.90% |

Reading every true match in full would cost the first row, and the index already reads less than that at 0.9 and
below, because most matches are certified from their codes.

**Theorem 4.1 (entropy bound; agent3b/REPORT.md §1).** Let an index answer threshold queries with Pr[R ≠ S(t)] ≤ ε,
reading bits adaptively from any stored structure, and let the rows match independently, each with probability p. Then,
with entropies in bits, the expected number of bits read is at least N·h₂(p) − h₂(ε) − εN, so the share of the f16 dataset read is at least
about h₂(p)/(16n). **[proved, unrefereed]**

*Proof (the source's argument, with its steps written out).* The bits read form a transcript, and because each read
depends only on the bits read before it, the possible transcripts form a prefix-free code, so E[bits] ≥
H(transcript) ≥ H(transcript | q). The output R is a function of the transcript and q, and R = S(t) except with
probability ε. S(t) takes at most 2^N values, so Fano's inequality gives H(S(t) | transcript, q) ≤ h₂(ε) + εN. Hence
H(transcript | q) ≥ H(S(t) | q) − H(S(t) | transcript, q) ≥ H(S(t) | q) − h₂(ε) − εN, and for independent rows
H(S(t) | q) = N·h₂(p). Dividing by the 16n bits of a row gives the share. ∎

Chapter 2's Proposition 2.13 is the same bound from the same source, and its Editor's queries 4 and 5 cover the points
of this chapter's queries 6 and 16.

At a fixed answer fraction p the bound is linear in N, whatever the index and however large, so no index is sublinear
in that sense, and only the constant is in play [sublinear/REPORT.md §2]. That constant is far from the design's: at
p = 0.01% the bound is 0.0015 bits a row, while the design reads about 28 bytes a row for text queries. The source
states that the baseline is 1,200 to 1,900 times above the bound, without saying which baseline or which rows (Editor's
query 16) [measured; agent3b/REPORT.md §1].
Theorem 4.1 is also attained, in the source's argument, by an unbounded index over a finite query set, so the metric
has no useful floor unless the index's size is capped [model; agent3b/REPORT.md §1]. Chapter 2 §2.8 tags the same
remark [sketch: gap named], because no construction is written down, and `06-open-problems.md` lists the difference.

sublinear/REPORT.md §2 therefore tracks a different quantity. At a fixed answer size k, from 10 to 10,000, including
exact k-NN at k = 10, the entropy bound is about log₂ C(N, k) ≈ k·log₂(N/k) bits, which permits sublinear work. The
quantity is W(N; k) for N from 10⁶ to 10⁹, and "sublinear" means W = O(N^β) with β < 1 over that range, since an
exponent that tends to one beyond the range is still worth having inside it. The index is capped at a constant number
of bytes a row, plus shared structures of which a query reads o(1) bytes a row, because without a cap a table of answers
makes the question empty. N grows by sampling more rows from the same distribution ("densifying"), modelled by nested
random subsamples of one sample, so a query's F̄ is fixed and the threshold for k matches is t_N with F̄(t_N) = k/N.
Growth by adding unrelated content is easier, because the new rows sit far from the threshold. Two effects then share
one exponent: the answer fraction k/N falls, and the clusters (K = N/1,000 in the measurements) and the local bases
tighten. W is also studied as a function of |V| for a fixed corpus (section 4.5).

The metric also needs to count the per-query fixed reads. No row of FINDINGS.md or agent3b/REPORT.md counts the K
centres read to form q·m (K·2n bytes, 0.1% of the dataset at 1,000 rows a cluster) or the local bases, and that is as
large as what the support table saves [measured; agent3b/REPORT.md, headline]. agent3b/REPORT.md §1 proposes to state an
index-size cap, to count per-query fixed reads, and to report bytes a row beside the percentage, since the percentage
falls as 1/n by itself.

## 4.2 Lower bounds

### Unconditional bounds

The strongest cell-probe bound is Pătraşcu and Thorup's: a randomised index of S cells for exact near-neighbour search
over N points in {0,1}^n makes Ω(n / log(S·n/N)) probes, which is Ω(n / log n) with near-linear space, and the same
holds for partial match [cited; sublinear/REPORT.md §3.1, ref. 1, PDF read]. The earlier bounds were Ω(log n)
(Borodin, Ostrovsky, Rabani) and Ω(n / log N) for polynomial space (Barkol, Rabani; Jayram et al. for partial match)
[cited, unchecked; as described in ref. 1]. At n = 768 with 16-bit coordinates these bounds are of the order of a
thousand probes, so nothing unconditional separates polylogarithmic from linear work for exact search with a
near-linear index [sublinear/REPORT.md §3.1].

### Bounds conditional on SETH

Under the Strong Exponential Time Hypothesis (SETH), Orthogonal Vectors (OV) on N vectors in dimension ω(log N) needs
N^(2−o(1)) time (Williams) [cited, unchecked; ref. 5, attributed by secondary sources]. Abboud, Rubinstein and Williams
extend this to approximate bichromatic maximum inner product [cited, unchecked; ref. 6, arXiv listing]. Rubinstein
extends it to (1+ε)-approximate bichromatic closest pair in Euclidean, Manhattan, Hamming and edit distance, and states
the consequence that approximate nearest-neighbour search with polynomial preprocessing needs near-linear query time
[cited; ref. 7, abstract read]. Chen gives exact maximum inner product hardness in very low dimension [cited,
unchecked; ref. 8]. In the other direction, Alman and Williams answer N exact Hamming nearest-neighbour queries in
dimension c₀·log N in N^(2−1/Õ(c₀)) time [cited, unchecked; ref. 9, search summary], so offline batches with fast
matrix multiplication can beat N per query by a factor that vanishes as c₀ grows. At N = 10⁹ and n = 768, c₀ is at
least 25 even at one bit a coordinate [sublinear/REPORT.md §3.2].

**Proposition 4.2 (Proposition 1 of sublinear/REPORT.md §3.3).** Suppose an index over N unit vectors in dimension n
can be built in N^(c_b) time and answers "is there a row x with q·x ≥ t?" in N^(1−η) time, for n = ω(log N). Then OV
is solved in N^(2−η′) time for some η′ > 0. **[proved, unrefereed; conditional on the OV conjecture]**

*Proof.* Let A, B ⊂ {0,1}^n be the OV instance, and write |b| for the number of ones in b. Map b to the unit vector
x_b = (𝟙 − b)/√(n − |b|) and a to q_a = a/√|a|. Split B into classes by the weight n − |b|, of which there are at most
n. In a class, q_a·x_b = (|a| − a·b)/√(|a|·(n − |b|)), which equals √(|a|/(n − |b|)) when a·b = 0 and is at most
(|a| − 1)/√(|a|·(n − |b|)) otherwise. Any t strictly between the two makes "some x in the class with q_a·x ≥ t"
equivalent to "some b in the class orthogonal to a". Cut each class into pieces of N^λ vectors, build an index on each
piece, and ask every a of A against every piece. Building costs n·N^(1−λ)·N^(λc_b) and querying costs
n·N·N^(1−λ)·N^(λ(1−η)) = n·N^(2−λη), which is subquadratic for λ < 1/c_b. ∎

The hard instances of the reduction have many rows within 1/n of the threshold in cosine. The measured data lacks that
property at the top of the tail and has it in the bulk, which is why section 4.3 states cost by distance below the
threshold [sublinear/REPORT.md §3.3].

### Restricted models that match the families tried

- **Hashing (the list-of-points model).** Andoni, Laarhoven, Razenshteyn and Waingarten give space N^(1+ξ_u) and query
  time N^(ξ_q) with C_apx²·√ξ_q + (C_apx² − 1)·√ξ_u = √(2C_apx² − 1), tight in a model covering hashing schemes on
  random instances [cited; ref. 10, abstract read; the model as summarised from ref. 11, cited, unchecked]. With
  near-linear space ξ_q = (2C_apx² − 1)/C_apx⁴. For matches at cosine s_m against a background at cosine 0 this is
  ξ_q = 1 − s_m², and ξ_q → 1 as the gap closes (C_apx → 1, which is exact search with rows just below the threshold).
  For text queries, after removing the corpus mean, s_m is about 0.3 and the background has spread, so the family's
  exponent is 0.9 or more [sublinear/REPORT.md §3.4]. Section 4.4 measures the idealised family.
- **Metric trees and pivots.** Pestov proves, within a stated model, an Ω(N^(1/4)) average-case bound for metric-tree
  schemes when the dataset's size is superpolynomial and subexponential in the intrinsic dimension [cited; ref. 12,
  abstract only, partly paraphrased; see Editor's query 20]. Chávez et al. define the intrinsic dimension of a distance histogram with mean
  μ_D and standard deviation σ_D as μ_D²/(2σ_D²), and bound what a pivot scheme can discard in terms of it [cited;
  ref. 13, Definition 9 and §7.2 read]. On the 1M sample it is 279 for text queries and 92 for item queries [measured;
  sublinear/REPORT.md §3.4, `tail.py` → `tail.txt`]. Weber, Schek and Blott is the classic statement that partitioning
  degenerates to a scan in high dimension [cited; ref. 14, read in `lit/REPORT.md`].
- **Low intrinsic dimension.** Cover trees answer exactly in O(C_exp¹²·log N) [cited; ref. 15, abstract read], and
  navigating nets likewise for constant dimension [cited, unchecked; refs. 16, 17, as described in ref. 15]. At the 10th
  neighbour C_exp is 4,050 for item queries (median) and N/k, the largest value possible, for text queries, so C_exp¹²
  exceeds 10⁴³ and these guarantees are vacuous at any N of interest [measured; sublinear/REPORT.md §3.4].
- **Range searching.** A threshold query is halfspace range reporting. In dimension D′ with near-linear space the known
  query times are N^(1−1/⌊D′/2⌋) for halfspaces (Matoušek) and N^(1−1/D′) for simplices (Matoušek; Chan) [cited,
  unchecked; refs. 18, 19], and N^(1−1/D′) is a lower bound for simplex reporting with linear space in the
  pointer-machine model (Afshani and Cheng) [cited; ref. 20, abstract and §1.2 read]. The data's intrinsic dimension
  n_int is about 20 by three estimators (section 4.4), which would put those exponents at 0.90 to 0.95. The source
  calls this a transfer by analogy, because the bounds are for worst-case point sets in ℝ^(D′) and the data is a
  20-dimensional set in ℝ^768 [sublinear/REPORT.md §3.4].

The data escapes the worst case only in part [measured; sublinear/REPORT.md §3.4]. Item queries do far better than the
0.90 to 0.95 that a dimension of 20 suggests (0.49 to 0.64), because their matches sit in a few tight clusters. Text
queries lie off the data (10th-neighbour distance 0.75, falling only as N^(−0.015)), and their exponent is 0.91 to 0.93.

A lower bound on storage completes the list. It concerns what a cluster summary must hold, and says nothing about reads.

**Proposition 4.3 (summaries cannot be small; agent3b/REPORT.md §2).** Suppose row i exceeds every other row of its
cluster by e_i in some direction r̂_i. A summary that decides "the cluster's maximum is at least t" to tolerance e_i/2,
for every sub-collection of the cluster, solves the INDEX problem, so it needs at least (1 − h₂(δ)) bits for each such
row, with δ its failure probability. **[proved, unrefereed; given a measured property]**

*Proof (as the source gives it).* Querying direction r̂_i at a threshold between row i's score and the largest score of
the other rows tells whether row i is in the sub-collection, so the summary encodes one membership bit for each such
row, and the INDEX lower bound gives (1 − h₂(δ)) bits each. ∎

Chapter 2's Proposition 2.10 is the same result from the same source.

The property holds widely: 91% of rows have e_i > 0.05 and 66% have e_i > 0.1 [measured; agent3b/REPORT.md §2, 200,000
rows, n = 768].

## 4.3 The cost law of the design

The exact path reads each row's data in stages and stops a row once its interval no longer contains the threshold. The
results below say what such a read costs as N grows.

**Lemma 4.4 (shell identity; Proposition 2 of sublinear/REPORT.md §4).** Let a scheme read each row in stages, so that
after stage ℓ row i has a sound interval [L_{i,ℓ}, U_{i,ℓ}] and stops once t lies outside it. Then the bytes read are
Σ_ℓ b_ℓ·|𝒰_{ℓ−1}|. If every non-match has upper slack U_{i,ℓ} − s_i between w⁻ and w⁺, then

    𝒩(t − w⁻) − 𝒩(t) ≤ |𝒰_ℓ ∩ non-matches| ≤ 𝒩(t − w⁺) − 𝒩(t).

**[proved, unrefereed]**

*Proof.* A non-match is not stopped exactly when U_{i,ℓ} ≥ t, that is, when s_i ≥ t − (U_{i,ℓ} − s_i). ∎

Each level's work on the non-matches is therefore a **shell** count: the number of rows within that level's slack below
the threshold.

**Proposition 4.5 (exponent of a shell; Proposition 3 of sublinear/REPORT.md §4).** For rows drawn independently, the
expected count N·F̄(t_N − w) with slack w = w(N) satisfies

    d ln(N·F̄(t_N − w)) / d ln N = 1 − χ(t_N − w)/χ(t_N) + χ(t_N − w)·dw/d ln N.

**[proved, unrefereed]**

*Proof.* F̄(t_N) = k/N gives dt_N/d ln N = 1/χ(t_N). Differentiate ln N + ln F̄(t_N − w(N)). ∎

The exponent has two sources of sublinearity. The first is the tail's shape, through the ratio of hazard rates at the
shell's edge and at the threshold. The second is slack that shrinks with N: with w ∝ N^(−ϑ) the last term is
−ϑ·w·χ(t_N − w), which is ϑ times the number of e-folds of density inside the slack.

**Corollary 4.6 (three tails; sublinear/REPORT.md §4, after Proposition 3).** At a fixed slack w the shell's exponent
is 0 for an exponential tail (constant hazard), w/(w + s* − t_N) for a tail that ends at s* with F̄(s* − z) ∝ z^ν,
and w/z_N to leading order in large z_N for a Gaussian tail with the threshold z_N above the median in the units of w.
**[proved, unrefereed]**

*Proof.* Put dw/d ln N = 0 in Proposition 4.5. A constant χ gives 1 − 1 = 0. For the power tail, χ(s* − z) = ν/z, so
1 − (s* − t_N)/(s* − t_N + w) = w/(w + s* − t_N). For a Gaussian tail in units of its standard deviation, χ(z) ≈ z for large z, so
1 − (z_N − w)/z_N = w/z_N. ∎

**Theorem 4.7 (asymptotically linear; Proposition 4 of sublinear/REPORT.md §4).** If scores are bounded above by s*,
and every row's upper slack at stage ℓ is at least w > 0 whatever N is, then for every fixed k the share of rows not
stopped after stage ℓ has a limit inferior of at least F̄(s* − w) > 0. **[proved, unrefereed]**

*Proof.* 𝒰_ℓ contains every row with t_N − w ≤ s_i < t_N. As k/N → 0, t_N → s*, so that set's share tends to
F̄(s* − w). ∎

Cosines are bounded, so Theorem 4.7 applies to every certificate whose slack does not shrink to zero: **every
fixed-slack level is asymptotically linear at fixed k**. With clusters of a fixed size the cluster radius falls as
N^(−1/n_int), with n_int about 20, so the slack does shrink, but slowly. The source draws two limits from this
[model; sublinear/REPORT.md §4]. For a query on the data manifold, slack and neighbour distance shrink together, and the
count of rows touched tends to a constant of the order C₁^(n_int) for some constant C₁, which no reachable N attains.
For a query off the manifold, as text queries are, the slack shrinks as N^(−1/n_int) and the exponent tends to
1 − ν/n_int.

**Proposition 4.8 (k-NN; Proposition 5 of sublinear/REPORT.md §4).** An algorithm that returns the exact k best rows
must hold, for every other row, an upper bound below τ_k. Its reads on the non-answers are therefore at least those of
the threshold query at t = τ_k. **[proved, unrefereed; in the interval model, where what has been read about a row is a
set of scores consistent with it]**

*Proof.* If some non-answer's upper bound is at least τ_k, its score can be raised above τ_k inside the set consistent
with what was read. That changes the answer without changing anything read, so the algorithm cannot yet return. ∎

Reading order and early termination can therefore save only the overshoot of the seed threshold over τ_k, which
knn/REPORT.md measures at 5% to 22% of the bytes read by an oracle that knows τ_k [measured; 1M BioCLIP-2, n = 768,
bytes excluding bases, as cited in sublinear/REPORT.md §4]. The threshold results at k = 10 in section 4.4 are the k-NN
results up to that factor.

**Claim 4.9 (shared structure does not rescue linearity; sublinear/REPORT.md §4, "What follows").** In the family of
schemes that form sound intervals from each row's own stream plus shared summaries, total work is at least the sum over
rows of the bits a per-row code needs to bring its slack below that row's distance from the threshold, and by
Theorem 4.7 that sum is linear in N at fixed k once t_N has reached the top of the score range. **[sketch: gap named]**

*Argument.* Corollary 2 of the consolidated proofs (chapter 2's Corollary 2.4) and Proposition 1 of `groups/REPORT.md`
(chapter 2's Proposition 2.8, chapter 1's Proposition 1.15) say that a shared summary of B_s bits over N_g members is
worth B_s/N_g bits a member, and reading it costs the query B_s bits, so shared structure does not
lower the bits read per decided member. *Gap, named by the source:* the floors are stated for mean widths under a
distribution, not row by row, so the sum over rows is not yet a theorem.

## 4.4 Measured exponents and what changed them

### Setup

sublinear/REPORT.md §5.1 uses the 1M BioCLIP-2 sample (n = 768) with 369 text queries and 100 item queries. The 100
item-query rows are removed, so item queries are out of corpus and the full set has 999,900 rows. `prep_sub.py` scores
all 469 queries exactly and draws nested random subsamples of 31,250, 62,500, 125,000, 250,000, 500,000 and 999,900 rows.
Each subsample has its own spherical k-means with N/1,000 clusters, its own support table with the centres as anchors,
and its own local PCA bases. The rms residual falls with N as 0.507, 0.471, 0.441, 0.416, 0.394, 0.377 (N^(−0.086)
overall, N^(−0.066) over the last doubling). Exponents are least-squares slopes of log(mean over queries) on log N over
the six sizes, with 95% bootstrap intervals over queries, and "last doubling" is the slope from 500k to 1M. Everything
is counted and nothing is timed. Every bound was checked against the exact score wherever it was used, with 0
violations in 1.85·10⁹ interval checks in the zero-read run, 9.95·10⁹ in the two per-member runs and 7.5·10⁷
cluster-bound checks in the hierarchy.

The first pass decides rows with nothing read per row, using four cuts. **Ball** cuts whole clusters by their radius.
**Range** uses the sorted key of Lemma 4.10. **Table** adds the support table. **Oracle** counts the rows in clusters
that hold both a match and a non-match, given each cluster's exact score extremes, and "oracle ±0.02" does the same with
the extremes known to ±0.02.

**Lemma 4.10 (the undecided rows of a cluster form one range; agent3b/REPORT.md §2, "C1").** For unit q and unit x in a
cluster with centre direction m̂ and key â = x·m̂,

    q·x ∈ (q·m̂)·â ± √(1 − (q·m̂)²)·√(1 − â²).

The upper bound is concave in â and the lower bound convex, so in a cluster sorted by â the undecided rows form one
contiguous range, found by two binary searches. **[proved, unrefereed]**

*Proof.* Write x = â·m̂ + x_⊥ and q = (q·m̂)·m̂ + q_⊥ with x_⊥, q_⊥ orthogonal to m̂. Then q·x = (q·m̂)·â + q_⊥·x_⊥,
and |q_⊥·x_⊥| ≤ ‖q_⊥‖·‖x_⊥‖ = √(1 − (q·m̂)²)·√(1 − â²). The function â ↦ √(1 − â²) is concave, so the upper bound
is concave and the lower bound convex. The rows with upper bound at least t form a superlevel set of a concave function,
which is an interval, and the rows with lower bound below t form a sublevel set of a convex function, which is also an
interval. The undecided rows are their intersection. ∎

Chapter 1's Lemma 1.5 is the same result for a stored key and for the support table's bound as well, with the proof
written out; the bound above is chapter 1's annulus bound (§1.2, item 1) at x₂ = 1. Chapter 1's Lemma 1.5 is now the
current statement for the index. It is restated for the stored key with a widening δ_â, and it says that the out rows
and the in rows can each lie at either end of the range. Its statement and proof are the range referee's own
(`program/kernel/referee-range.md`), so chapter 1 tags it [proved, unrefereed]. The exact-key statement above is that
lemma's part (d) in its annulus case, which the same referee found correct; whether its tag is raised is chapter 1's
Editor's query 17.

The **support table** stores, for anchor directions a′_j (here the K centres), the maximum and minimum over cluster c of
a′_j·x. For q = a′_j + (q − a′_j) the cluster's maximum of q·x is at most the stored maximum plus a bound on the maximum
of (q − a′_j)·x of the same annulus form, so the undecided set is again one range per cluster [agent3b/REPORT.md §2]. Chapter 1's Proposition 1.4 and the
"table plus remainder" bound of §1.2 state the cut and the bound for the index as built.
A query reads one row of the table (2K bytes), storage is 4K² bytes, and maxima only grow on insert and stay valid after
delete.

### The first pass

The share of rows not decided with nothing read per row [measured; sublinear/REPORT.md §5.2, `zero.py` → `fit.txt`;
0 violations in 1,848,439,684 interval checks]:

| | 31k | 125k | 500k | 1M | Exponent [95%] | Last doubling |
|---|---|---|---|---|---|---|
| **Text, k = 10**: t_N, sd above the median | 4.65 | 5.04 | 5.34 | 5.48 | | |
| Ball | 100% | 100% | 99.4% | 98.5% | 0.996 | 0.99 |
| Range | 97.1% | 87.0% | 79.3% | 75.3% | 0.929 [0.925, 0.934] | 0.93 |
| Range and table | 95.8% | 83.9% | 74.0% | 69.1% | 0.908 [0.901, 0.914] | 0.90 |
| Centres read under the table | 100% | 100% | 100% | 100% | 1.00 | 1.00 |
| Oracle | 6.3% | 2.0% | 0.64% | 0.30% | 0.115 [0.090, 0.138] | −0.09 |
| Oracle ±0.02 | 11.0% | 4.7% | 2.0% | 1.2% | 0.337 [0.309, 0.368] | 0.24 |
| **Text, k = 1,000**: range and table | 100% | 97.0% | 87.7% | 82.7% | 0.944 [0.940, 0.949] | 0.91 |
| **Item, k = 10**: t_N, sd above the median | 4.86 | 5.20 | 5.48 | 5.59 | | |
| Ball | 100% | 96.6% | 83.2% | 73.8% | 0.914 [0.900, 0.929] | 0.83 |
| Range | 76.8% | 48.6% | 29.0% | 22.2% | 0.643 [0.613, 0.678] | 0.62 |
| Range and table | 54.5% | 26.6% | 12.7% | 9.3% | 0.488 [0.421, 0.556] | 0.56 |
| Centres read under the table | 81% | 66% | 53% | 47% | 0.848 [0.798, 0.889] | 0.84 |
| Oracle | 4.6% | 1.6% | 0.54% | 0.32% | 0.223 [0.175, 0.268] | 0.27 |
| **Item, k = 1,000**: range and table | 91.5% | 60.3% | 32.5% | 23.2% | 0.599 [0.537, 0.654] | 0.51 |

The first pass is sublinear for both query classes, and only weakly for text: a thousandfold more data costs about
600 times more first-pass work for text at k = 10. Every centre is read at every N for text queries, because the
support table's anchors are never near a text query. The oracle count is nearly flat because at k = 10 the matches sit
in 2 or 3 clusters of 1,000 at every N. The support table here has as many anchors as clusters at every N, which cannot
hold at 10⁹; with 16 to 64 anchors the item exponent would sit between the "range" and "table" rows [sublinear/REPORT.md
§5.2].

### The per-row read

The block ladder reads exact local PCA coordinates in dyadic blocks (1, 1, 2, 4, …, 256, 256), bounds the unread
remainder block by block, and charges nothing for the row's own m·r and block norms. A row reads blocks until it is
decided, and coordinates stand in for bytes. One series grows the number of local bases with N (12, 12, 12, 16, 32, 64)
and the other keeps 12 bases at every N [measured; sublinear/REPORT.md §5.3, `ladder.py` → `lfit.txt`, `lfit_g.txt`;
0 violations in 5,268,107,400 and 4,682,732,400 interval checks]:

| Coordinates read per row of N | 31k | 125k | 500k | 1M | Exponent [95%] | Last doubling |
|---|---|---|---|---|---|---|
| Text, k = 10, 12 bases | 74.4 | 66.8 | 55.1 | 47.3 | 0.873 [0.862, 0.882] | 0.78 |
| Text, k = 10, bases growing | 74.4 | 66.8 | 46.3 | 35.9 | 0.793 [0.781, 0.805] | 0.63 |
| Text, k = 1,000, 12 bases / growing | 190 | 139 | 99.9 / 87.8 | 83.5 / 67.0 | 0.766 / 0.703 | 0.74 / 0.61 |
| Item, k = 10, 12 bases | 11.7 | 7.8 | 4.2 | 2.9 | 0.597 [0.566, 0.624] | 0.46 |
| Item, k = 10, bases growing | 11.7 | 7.8 | 3.3 | 2.0 | 0.493 [0.461, 0.520] | 0.29 |
| Item, k = 1,000, 12 bases / growing | 91.9 | 44.4 | 20.8 / 17.7 | 13.6 / 10.5 | 0.453 / 0.378 | 0.39 / 0.25 |

Coordinates read at fixed k grow as N^0.79 to N^0.87 for text and N^0.49 to N^0.60 for items. Deeper levels have lower
exponents, as Proposition 4.5 predicts, because their smaller slack puts their shells higher in the tail: for text at
k = 10 with 12 bases, the share undecided after 0, 32 and 128 coordinates has exponents 0.930, 0.890 and 0.824. Deep
levels of item queries reach the constant regime, since about 200 rows need more than 128 coordinates at every N. The
bases are fitted on the rows they code, which flatters the small-N costs (a group holds about 2,600 rows in 768
dimensions at 31k) and biases the fitted exponents upward, so the last-doubling column is the safer guide
[sublinear/REPORT.md §5.3].

### The score tail

Under densification a fixed k at a larger N is a smaller p, so reading the following table left to right is reading N
upward at fixed certificates [measured; sublinear/REPORT.md §5.4, text queries at the full 1M, `tail.py` → `tail.txt`]:

| Text queries, p = | 10⁻² | 10⁻³ | 10⁻⁴ | 10⁻⁵ | 10⁻⁶ |
|---|---|---|---|---|---|
| t_N, sd above the median (a Gaussian tail gives) | 3.01 (2.33) | 4.25 (3.09) | 4.98 (3.72) | 5.48 (4.26) | 5.87 (4.75) |
| Rows within 3 sd below t_N | 48.2% | 16.0% | 8.0% | 4.5% | 2.7% |
| Rows within 6 sd below t_N | 99.9% | 94.4% | 80.0% | 65.3% | 53.0% |
| Local exponent of the 3 sd shell in N | 0.47 | 0.69 | 0.74 | 0.79 | 0.77 |
| Local exponent of the 6 sd shell in N | 0.99 | 0.93 | 0.91 | 0.92 | 0.90 |

Each standard deviation of slack removed from a certificate cuts the rows it leaves by a factor of 3 to 4 and lowers its
exponent by about 0.05. At p near 10⁻⁵ the text shells hold 65% at 6 sd, 14% at 4 sd, 4.5% at 3 sd and 1.1% at 2 sd,
with exponents at k = 10 of 0.91, 0.79, 0.74 and 0.68. The tail is heavier than Gaussian in position (5.5 sd where a
Gaussian has 4.3) but steep at its top, since the 10th best of 1M is 0.4 sd below the best. The local exponents rise as
p falls (the 1 sd shell's from 0.26 at 10⁻² to 0.83 at 10⁻⁶), which is Theorem 4.7 arriving. Item queries are already
there: their shells shrink only as N^0.92 to N^0.95 at any width, and their sublinearity in the first pass comes
entirely from slack that shrinks as clusters tighten around the query. The score sd is 0.040 (text) and 0.068 (item), and the best
score is 5.9 and 6.0 sd above the median.

The intrinsic dimension seen from item queries is 21.6, 20.2 and 15.7 by the Levina–Bickel estimator at 20, 100 and
1,000 neighbours, 20.1 from the 10th-neighbour distance (which falls as N^(−0.050)), and 19 from the cluster radius law
of `groups/REPORT.md`; seen from text queries it is 52 to 89 [measured; sublinear/REPORT.md §5.4].

### What changed the exponent, and what did not

Only one thing changed the exponent by much: knowing the query's own cluster extremes, the oracle rows above, at an
exponent of 0.12 (text) and 0.22 (item) with exact extremes and 0.34 (text) and 0.49 (item) with extremes to ±0.02. That is reachable
for a repeated query direction, as one cached row of two bytes a cluster per query (2 MB a query at 10⁹), valid for
every threshold and every viewer through the viewer's mask. It is not reachable for a fresh query, and how often query
directions repeat was not measured [measured; sublinear/REPORT.md §1, §5.2, §9].

The other levers change the exponent by a few hundredths or only the constant [measured unless stated;
sublinear/REPORT.md §5.2, §5.3, §5.6, §6]:

| Candidate | Effect at k = 10 (text / item) |
|---|---|
| Clusters kept at 1,000 rows as N grows (K ∝ N) | about −0.04 of text exponent (inferred: at the fixed 1M clustering the range leaves 75.3% at k = 10 and 80.2% at k = 100, an exponent of 0.97 in k/N alone, against 0.93 with clusters that track N) |
| More local bases as N grows (12 → 64) | −0.06 to −0.10 of exponent; −24% / −31% coordinates at 1M; 64,000 bases at 10⁹ would take 150 GB as float32 |
| Tighter first certificate | about −0.05 of exponent per sd of slack removed (from the shell table; the M-norm key itself not re-run across N) |
| Hierarchy over centres (fan-out about 4, 58,750 leaves, `tree.py`) | reads 129% / 66.5% of the leaf centres; rows under undecided leaves 82.3% / 22.5%; worse than a flat list for text |
| Idealised cap-filter hashing (`lsf.py`, an analytic expectation tuned per query) | text: 61% of N at 1M (exponent 0.89) with 256 table entries a row, 34% (0.81) with 4,096 entries (16 KB a row); item: 3.5% (0.45) and 1.6% (0.46) |
| Finer clusters, anchors for fresh text queries, group summaries, trees over rows | none; ruled out in earlier reports (finer clusters: radius ∝ K^(−0.053)) |
| Batched scans, result caches | constant only [model] |

The hierarchy fails because its cut is the ball bound and a ball's radius barely depends on the level. Hashing for text
matches the theory of section 4.2 and does no better than the design's own first pass, at far greater storage. The
item optimum of the hash family sits at caps of about 10 to 11, which means of the order of 10²⁵ filters, so it is a
bound on the family and no design. With 16 entries a row no query gains over a scan [sublinear/REPORT.md §5.6].

At fixed thresholds on 200,000-row subsets (denser than the 1M sample), the sorted range and the support table cut
the share of the dataset read and the share of rows touched as follows [measured; agent3b/REPORT.md, headline, `base.py`,
`pair.py`; 200 corpus rows as queries; 0 violations]:

| n = 768, threshold | 0.95 | 0.9 | 0.8 | 0.7 |
|---|---|---|---|---|
| Matches, share of rows | 1.07% | 3.73% | 7.90% | 15.97% |
| Dataset read: baseline replica / range / range and table (K = 200) | 0.564 / 0.347 / 0.306% | 0.870 / 0.687 / 0.560% | 1.868 / 1.782 / 1.466% | 5.336 / 5.305 / 5.118% |
| Rows touched: range / range and table | 20.1 / 12.6% | 38.1 / 17.8% | 72.6 / 39.4% | 89.0 / 76.4% |
| Entropy bound (Theorem 4.1) | 0.00044% | 0.0013% | 0.0027% | 0.0046% |

The baseline touches every row. A row at distance Δs below the threshold costs the baseline about C(t)/Δs bytes, with
C(t) ≈ 0.7, 1.5, 3 and 5.5 byte·cosine at the four thresholds, so at t = 0.95 the 0.56% of rows within 0.005 of the
threshold take 29% of the bytes [measured; agent3b/REPORT.md §1]. With a perfect cluster summary only 3.7%, 6.3%, 10.7%
and 25.0% of rows would need reading, which places the weakness of cluster pruning in the bound rather than in the
partition [measured; agent3b/REPORT.md §2].

### Extrapolation to 10⁹

Carrying two readings of each exponent (the fitted one, and the last doubling's) from 1M to 10⁹ gives, for text at
k = 10: 45% to 46% of rows undecided by the range (75% at 1M), 35% to 37% by range and table (69%), 10 to 20 coordinates a
row with 12 bases (47), and 2,000 to 7,000 rows under the oracle (3,000). For items: 1.6% to 1.9% by the range (22%),
0.3% to 0.45% by range and table (9.3%), and 0.07 to 0.18 coordinates a row (2.9). A thousand times more data then costs
roughly 200 to 400 times more per-row work for text, and the first pass still touches a third to a half of the rows. If
the tail has already ended, which the tail table suggests is close, the true figures are nearer the upper ends
[conjecture; sublinear/REPORT.md §5.7, `extrap.py` → `extrap.txt`].

### Timed cost at 1M

SYSTEM.md §4 times the native prototype on 999,000 BioCLIP-2 rows (n = 768, f16) on one pinned thread of a Ryzen 9
5900X under WSL2, with about ±5% noise from shared cores [measured; `rust/results/const/cmp.py` on
`rust/results/system/t_s4_{2,3,4}.out`]:

| Answer | Item ms | Item B/row | Item full | Text ms | Text B/row | Text full |
|---|---|---|---|---|---|---|
| 0.01% | 2.48 | 2.9 | 0.11% | 20.2 | 20.6 | 0.55% |
| 1% | 8.06 | 18.6 | 0.71% | 39.6 | 67.0 | 0.78% |
| 10% | 53.2 | 133.2 | 2.85% | 61.1 | 184.5 | 4.80% |
| 50% | 72.9 | 276.2 | 8.43% | 80.6 | 322.8 | 10.38% |

A plain f16 scan of every row takes 86 to 89 ms on the same machine, so the index is 35 times faster for item queries at
0.01% and 1.1 to 1.2 times faster at 50%. Large answers cost nearly a scan because almost every row must be read deep
enough to be decided: at a 50% item answer 91% of rows enter the first randomised level and 20% the last, and at a 1%
text answer 95% enter the first level and 46% reach 56 bytes. A query's fixed cost is 6 to 7 ms once most groups are
open, made of about 3 ms for 37.7 MB of int8 basis products, 1.8 to 1.9 ms for the cut over 6 MB of float64 centres, and
0.8 ms for the rotations [measured; SYSTEM.md §4.2, `rust/results/system/bd_p2c.out`]. These fixed reads are the ones
section 4.1 says the metric must count.

## 4.5 The visible set

Random visible subsets of the 1M clustering, with k = 10 inside the subset and cluster extremes over all rows
[measured; sublinear/REPORT.md §5.5, `zero.py`, `ladder.py`]:

| Visible rows | 977 | 15,625 | 62,500 | 250,000 | 999,900 | Exponent in \|V\| |
|---|---|---|---|---|---|---|
| Clusters holding a visible row (text centres read) | 548 | 999 | 999 | 999 | 999 | 0 above about 5,000 |
| Text: not decided by range | 94.7% | 85.4% | 81.5% | 78.1% | 75.3% | 0.97 |
| Text: coordinates per visible row | 115 | 61.8 | 50.3 | 42.1 | 35.9 | 0.836 [0.822, 0.847] |
| Text: oracle with extremes over all rows (rows) | 150 | 413 | 786 | 1,502 | 3,026 | 0.47 |
| Item: not decided by range and table | 46.7% | 20.6% | 14.9% | 11.8% | 9.3% | 0.81 |
| Item: coordinates per visible row | 36.0 | 8.6 | 4.9 | 3.2 | 2.0 | 0.593 [0.560, 0.623] |

Work per visible row rises as the set thins, because the k-th best visible score is lower. Total work is therefore
sublinear in |V| only in the same weak sense as in N (|V|^0.84 for text and |V|^0.59 for items, in coordinates; rows
touched as |V|^0.96 and |V|^0.81), on top of min(K, about |V|) centres. Visible sets contiguous in id order (taxonomic
slices) read only the centres of the clusters they meet (10, 44, 144 and 375 for 977, 15,625, 62,500 and 250,000 visible
rows), and their coordinates grow as |V|^0.67 for text at k = 10. An index built on a random visible set alone is no
better: at 31,250 visible rows its own 31 clusters leave 97.1% undecided by range and read 74.4 coordinates a row,
while the shared 999 centres leave 83.3% and read 55.6, and counting a centre as 768 coordinates the totals are 2.35M
and 2.51M. Below roughly 10⁴ to 10⁵ visible rows a scan of the visible set is the better plan [sublinear/REPORT.md §5.5].

The approximate path finds the same subsample property in its own units (section 4.7).

## 4.6 Dimension

The cost law of section 4.3 does not mention the dimension n. It acts through two numbers: each level's slack in units
of the query's score sd, and the tail's shape in the same units [sublinear/REPORT.md §7].

Slack in sd units grows with the effective dimension [model; sublinear/REPORT.md §7, where it is "argued"; see Editor's
query 17]. With nothing read, the bound on Δ·r is ‖Δ‖·‖r‖ and its typical value is that divided by √n_eff, so the
zero-read slack is about √n_eff within-cluster sd, which the source says holds whatever the model. More dimensions therefore put the
first pass further into the bulk and move its exponent toward one. A randomised level at a fixed number of bits a
dimension keeps a fixed slack in sd units, so bytes a row scale with n and the share of the vector read stays roughly
constant.

Fewer dimensions leave less room in the tail [measured on a proxy; sublinear/REPORT.md §7, `dims.py`, `tail_32.txt`,
`tail_96.txt`, `tail_256.txt`]. The proxy projects corpus and queries on the top n principal directions and renormalises,
which is a truncation of one model rather than another model:

| n | Text: t_N at p = 10⁻⁵, sd above the median | Text: within 3 sd | Text: within 6 sd | Item: local dimension (20 neighbours) | Item: C_exp |
|---|---|---|---|---|---|
| 32 | 3.76 | 25.1% | 99.6% | 15.2 | 962 |
| 96 | 4.98 | 6.3% | 85.2% | 18.6 | 1,857 |
| 256 | 5.42 | 4.6% | 68.3% | 21.3 | 3,596 |
| 768 | 5.48 | 4.5% | 65.3% | 21.6 | 4,050 |

A low dimension leaves fewer standard deviations between the bulk and the best matches, so a certificate of a given
slack in sd units decides less, which is consistent with GloVe-100 touching 96% of rows in knn/REPORT.md. The rows'
intrinsic dimension is 15 to 22 whatever the ambient dimension, so the low-dimension guarantees of section 4.2 stay
vacuous across the range. The conditional lower bound of Proposition 4.2 needs only n = ω(log N), which every dimension
from 100 to 5,000 satisfies, and the hashing exponent 1 − s_m² depends on the matches' cosine against the background and
not on n. At n = 1,024 (BioCLIP-2.5 ViT-H/14, 200,000 rows, `agent3b/data1024/`) the range and table read 0.159% to
0.419% of the dataset at thresholds 0.95 to 0.7 and touch 5.0% to 12.8% of rows [measured; agent3b/REPORT.md, headline].
No second embedding model and no native large dimension were run.

## 4.7 The approximate mode: the law of the read

The approximate k-NN path returns k rows to a target recall and certifies nothing, so neither Proposition 4.8 nor
Theorem 4.7 applies to it. `layered.md` measures how its read grows, on four datasets and in different units: rows of
the clusters read, with search inside a read cluster exact. This section keeps its law apart from the exact path's and
compares the two at the end. Chapter 5 gives the theory of the cluster order and the stop that this law measures. The
source for all of it is
`vexact-visitorder: bench/suite/notes/visit-order/hierarchy/layered.md` (§0, §1, §2, §8.1).

At 1M the approximate path reads most clusters' stored vectors directly, so the rows read are most of its cost: per
query, unfiltered, to recall 0.95 at k = 10, it reads glove 47 MB (181k vectors in 75 clusters), yahoo 23 MB (30k in 15)
and LAION 64 MB (61k in 29), of which the centres are 0.08 to 0.26 MB [measured; replay B, §1].

### The law [measured]

Replay A (`layered/scaling.py`) takes nested random subsamples of N, N/4 and N/16 rows of glove-100-angular (1.18M
rows), yahoo-minilm-384 (0.68M), laion-clip-512 with text queries (1.0M, out of distribution) and sift-128-euclidean
(1M, L2 scores q·x − ‖x‖²/2). On each it builds k-means partitions at n̄ from 62 to 16,000 (K from 62 to 4,700), scores
1,000 queries exactly, records each cluster's exact μ_c and σ_c, reads clusters in decreasing μ_c + 3σ_c, the same
number for every query, and records the rows at which mean recall reaches a target. The key uses each cluster's exact
spread, so this is the order's own limit rather than a property of the library's 16 directions [§2].

Rows read to recall 0.95 at k = 10, clusters read in brackets [measured; §2]:

| rows | n̄ = 250 | n̄ = 1,000 | n̄ = 4,000 | n̄ = 16,000 |
|---|---|---|---|---|
| glove, N = 1,183,514 | 51,054 (169) | 98,086 (82) | 173,545 (37) | 278,369 (16) |
| glove, N/16 = 73,969 | 13,130 (46) | 22,598 (20) | - | - |
| yahoo, N = 677,305 | 7,543 (30) | 18,235 (18) | 41,637 (10) | 96,938 (6) |
| yahoo, N/16 = 42,331 | 3,655 (15) | 8,010 (8) | - | - |
| LAION text, N = 1,000,448 | 18,052 (73) | 38,978 (37) | 78,395 (19) | 175,216 (10) |
| LAION text, N/16 = 62,528 | 6,423 (24) | 13,621 (12) | - | - |
| sift (L2), N = 1,000,000 | 13,278 (49) | 23,193 (22) | 45,632 (11) | 94,975 (6) |
| sift (L2), N/16 = 62,500 | 3,884 (15) | 7,003 (7) | - | - |

A least-squares fit of log rows on log n̄ and log N over the ten partitions of each dataset gives rows ∝ n̄^u·N^v,
within a factor 1.04 to 1.3 of every point [measured; §2]:

| k, recall | glove u, v | yahoo u, v | LAION text u, v | sift u, v | exponent under n̄ ∝ √N, u/2 + v |
|---|---|---|---|---|---|
| 10, 0.9 | 0.47, 0.41 | 0.69, 0.20 | 0.62, 0.30 | 0.50, 0.40 | 0.55 to 0.64 |
| 10, 0.95 | 0.41, 0.52 | 0.62, 0.29 | 0.54, 0.39 | 0.45, 0.44 | 0.60 to 0.72 |
| 10, 0.99 | 0.29, 0.70 | 0.47, 0.50 | 0.40, 0.60 | 0.39, 0.52 | 0.71 to 0.85 |
| 100, 0.95 | 0.28, 0.57 | 0.50, 0.30 | 0.42, 0.40 | 0.37, 0.49 | 0.55 to 0.71 |
| 100, 0.99 | 0.20, 0.73 | 0.37, 0.50 | 0.30, 0.61 | 0.31, 0.57 | 0.69 to 0.83 |

The share of the corpus read depends on K and hardly on N [measured; §2]. At a fixed K the rows read are a nearly fixed
share of the rows: glove at K = 296 reads 17.7%, 17.1% and 14.7% of 74k, 296k and 1.18M rows to recall 0.95, and sift at
K = 250 reads 6.2%, 5.7% and 4.6%. Each fourfold K divides the share by 1.8 to 2.4, so Φ(K) ∝ K^(−u), u + v is 0.9 to 1,
and the clusters read, K·Φ(K), grow as K^(0.3 to 0.6) rather than staying constant. Under the build rule n̄ ∝ √N the rows
read therefore grow as N^(u/2+v), which is N^0.60 to N^0.72 at recall 0.95 and N^0.71 to N^0.85 at 0.99, faster than the
√N the design had assumed. By this law a glove-like corpus of 1B rows in 15,625 clusters reads about 2% of its rows to
recall 0.95. With leaves of a fixed size the rows grow as N^v, with v = 0.2 to 0.5 at recall 0.9 to 0.95 and 0.5 to 0.7
at 0.99, and the centres scored on the way grow at least as fast as the leaves read. The spread term in the key is worth 1.2 to 1.9 times in rows over the centre score alone, at every size and N.

A random visible set behaves as a subsample [measured; §2]. The full set's partition read through a random sixteenth of
its rows needs the rows and clusters of a partition built from a sixteenth of the rows with clusters of the same visible
size (glove, 296 clusters of 250 visible rows: 13,499 rows in 47.9 clusters, against 13,130 in 46.4; sift, 250
clusters: 3,859 in 14.7, against 3,884 in 14.9). At 1% visible the query still reads Φ(K) of the visible rows, spread
over the same share of the clusters (glove at K = 296: 2,244 of 11,835 visible rows in 52 clusters), which is why
per-cluster overhead and the centre scan dominate under sparse sets.

### At 100M and 1B [model]

`layered/extrapolate.py` carries replay A's law, fitted per dataset on 62k to 1.2M rows, to larger N with the byte
cost of each structure, treating a random visible share as a subsample and taking a few ratios from other studies. The
build rule gives n̄ = 16,000 and K = 6,250 at 100M, and n̄ = 64,000 and K = 15,625 at 1B. "today" is direct reads,
"levels" the per-level rule of `layered.md` §4 alone, and "all" that rule with leaves of 2,000 rows, a sketch of the
centres, and more groups [§8.1]:

| MB per query to recall 0.95, unfiltered | today 100M | today 1B | levels 1B | all 1B |
|---|---|---|---|---|
| glove | 752 | 4,357 | 362 | 109 |
| yahoo | 322 | 1,467 | 56.4 | 10.3 |
| LAION text | 1,048 | 5,388 | 160 | 38.6 |
| sift | 168 | 855 | 72.6 | 18.8 |

Unfiltered and at 25% visible the read itself is the cost at every N, and it is the term that grows. At 1% visible
from 100M on, the centre scan (K·n, 2 to 8 MB at 1B) is the largest term of the "levels" design on yahoo, LAION and
sift, while on glove the tables (100M) or the codes (1B) are larger (Editor's query 18). The model is weakest in
v, which is fitted on a 16-fold range of N and carried 1,000-fold, so that ±0.1 in v is a factor of 2 at 1B. The law is
also for the nprobe curve with exact spreads, while the library's stop reads 1.3 to 3 times more clusters at recall
0.99 [§8.1].

### The two laws compared

The two laws count different things on different data, so their exponents can be set side by side but not subtracted.

- **Units and data.** The exact path's exponents count rows touched and coordinates read in each row's own stream, at a
  fixed k, on BioCLIP-2 (n = 768) with item and text queries. The approximate law counts every row of every cluster read,
  to a mean recall, on glove, yahoo, LAION text and sift. Both use nested random subsamples, so both model densifying
  growth.
- **Exponents.** At fixed cluster size (K ∝ N) the exact path's first pass grows as N^0.91 to N^0.93 for text and
  N^0.49 to N^0.64 for items, and its per-row read as N^0.79 to N^0.87 and N^0.49 to N^0.60. The approximate read at a
  fixed cluster size grows as N^v, with v = 0.2 to 0.5 at recall 0.9 to 0.95 and 0.5 to 0.7 at 0.99. On LAION's text
  queries, the approximate path's out-of-distribution case, v is 0.39 at recall 0.95 and 0.60 at 0.99.
- **Asymptotics.** The exact path has a theorem: every fixed-slack level is asymptotically linear (Theorem 4.7), and
  k-NN costs at least the threshold query at τ_k (Proposition 4.8). The approximate path has no such result in the
  sources. Its fitted v rises with the recall target on every dataset, but no source fits an exponent at recall 1.
- **Where the sublinearity comes from.** In both, much of it comes from a partition that refines as N grows. At the
  fixed 1M clustering the exact path's range cut has an exponent of 0.97 in k/N, against 0.93 with clusters that track N
  (section 4.4), and the approximate share Φ(K) depends on K and hardly on N.
- **Visible sets.** Both find that a random visible set behaves as a smaller corpus with the same partition: the exact
  path's work grows as |V|^0.84 (text) and |V|^0.59 (item) on top of the centres, and the approximate path reads Φ(K)
  of the visible rows in the same share of clusters.

## 4.8 Cluster size against N

Three sources set the cluster size, and each optimises a different quantity.

**The exact path's exponent** (sublinear/REPORT.md). Clusters kept at 1,000 rows as N grows (K = N/1,000) are part of
every measured exponent of section 4.4, and are worth about 0.04 of the text exponent [measured, share inferred]. Over
the 1.5 decades measured, the rms residual falls as N^(−0.086), and the cluster radius at fixed size falls as
N^(−1/n_int) with n_int about 20, which slows the approach to Theorem 4.7's linear limit. Finer clusters at fixed N
gain little, since their radius falls only as K^(−0.053) [sublinear/REPORT.md §4, §5.1, §5.6]. The report recommends
keeping clusters near 1,000 rows as N grows [§9].

**The exact path's time at 1M** (rust/REPORT_CLUSTERSIZE.md). On 999,000 BioCLIP-2 rows, on one pinned core of a 4-core
AVX-512 Xeon VM, median over 3 runs of the median over 20 queries, two repeats with 0% to 7% spread on the baseline
[measured; `results/clustersize/`, `tools/fit.py`, `src/run2.rs`]:

| Fit | rows a cluster | K | groups | text 0.01% | text 1% | text 50% | item 0.01% | item 1% | item 50% |
|---|---|---|---|---|---|---|---|---|---|
| base | 1,000 | 999 | 64 | 31.8 ms | 62.6 | 122.4 | 2.5 | 13.8 | 113.5 |
| P4 | 4,000 | 250 | 64 | −17% | −13% | −8% | −9% | +16% | −8% |
| P16 | 16,000 | 62 | 62 | −10% | −12% | −13% | +79% | +43% | −11% |

Larger clusters cut the per-cluster phases roughly in proportion to K, but the cut then prunes less and part of the
saving goes back to per-row work: at text 0.01%, P16 removes 8.4 ms of per-cluster work and adds 4.8 ms of per-row work,
with 32% more rows entering the first level, and small item answers lose for the same reason. From the profiled phases
the source models a text query as G × 0.12 ms for groups, K × (3 + 8 to 17) µs for clusters, 11 ns a row-level and the
full vectors, so at fixed rows a cluster the cluster and row terms grow linearly in N [model; REPORT_CLUSTERSIZE.md,
"Extrapolation"]. All fits gave 0 wrong rows against brute force. The report recommends about 4,000 rows a cluster with
64 groups and the 16/32 deterministic schedule (text −18% to −33%, item −10% to −23%).

At 200,000 rows the support table's rows touched rise with cluster size: at thresholds 0.95, 0.9, 0.8 and 0.7 they are
17.5%, 26.7%, 57.8% and 85.8% with 4,000 rows a cluster, 12.6%, 17.8%, 39.4% and 76.4% with 1,000, and 9.4%, 13.4%,
27.1% and 64.5% with 250 [measured; agent3b/REPORT.md §2].

**The approximate path** (`layered.md`). Rows read grow as n̄^u with u = 0.2 to 0.7 across the fits of section 4.7 (0.41 to 0.62 at k = 10,
recall 0.95), so a larger cluster costs reads,
while a smaller one costs centres. The build rule (`fit::per_cluster_by_n`) sets n̄ ∝ √N: 2,000 rows a cluster at 1M,
16,000 at 100M and 64,000 at 1B, under which the read grows as N^(u/2+v). `layered.md` proposes leaves of 2,000 rows
under clusters larger than that from 8M rows on, which is the only candidate there that changes the exponent, from
N^(u/2+v) to N^v. The approximate path would read leaves while the exact path keeps its large clusters, and the leaves
need exact queries timed on such a build before adoption [§0; the leaves' gain is model at 100M and 1B, and measured
in that shape at 1M as 4 to 9 times fewer rows than reading parents whole].

## 4.9 Spending a row's bytes: the hand-over, the rate–margin law and the price of a scalar

The earlier sections count what a query reads as N grows. This section holds N fixed and asks three questions of one
row's stream: at how many bytes a randomised level starts to beat a deterministic one, how many bytes an item at a
given margin must read, and what each coded block costs once the time of its stored scalars is counted. The sources are
under `notes/program/`: `confidence/DESIGN-CLAIMS.md` (claims C3 to C5, cited as DESIGN-CLAIMS), the dump test
`confidence/dump-test/RESULTS.md` (cited as RESULTS), the referee's report `confidence/referee/referee-design.md`
(cited as the referee), and `codes/REPORT.md` for the nesting figures. The referee checked the derivations, reran the
programme and corrected several readings; it refereed nothing as proved, so no statement here is [proved, refereed]
except where it quotes chapter 3's Theorems 3.11 and 3.12. Where the referee corrected a figure, the corrected figure is given and
sourced to the referee. DESIGN-CLAIMS tags some numbers [estimate], which is not a tag of these notes; they enter as
[model], with the model named (Editor's query 21).

The index's stream, as the sources describe it: six deterministic levels at 4, 8, 12, 16, 24 and 32 B of code, which
together are the 256-bit prefix; then four randomised levels, named by their cumulative code bytes 56, 96, 160 and
256 B (chapter 3 §3.1). The first randomised level codes block 0 (96 positions) at 2 bits and leaves the other three
blocks to their Cauchy–Schwarz bounds. An item entering it reads 29.5 B: 24 B of code and 5.5 B of scalars, which are
‖y_j‖ in 8 bits for each of the four blocks, coded or not, and κ in 8 bits and tan φ in 4 bits for the one coded block
[measured; `rust/results/system/bd_p2c.out` build line and SYSTEM.md §2.1, §5, as read in DESIGN-CLAIMS C5.1; checked
by the referee §4.1]. A coded block **rises** at a level when it is newly coded there or its rate grows, and each rise
makes the item read and decode that block's κ and tan φ again.

### 4.9.1 The model

Everything in §4.9.2, the frontier of §4.9.3, §4.9.5 and §4.9.6 that carries [model] or [computed] is derived in one model
[DESIGN-CLAIMS §0]. Write Δ·r = Σᵢ √γᵢ·gᵢzᵢ in the eigen-coordinates of chapter 0, with g and z independent standard
Gaussians. A **layout** is a set D of coordinates certified deterministically and blocks T_1, …, T_J that are rotated
and coded at R_j bits a position, with H_D nats of deterministic code on D and H_j nats of prefix on block j. Its
half-width and its bits are

    h = W_D(H_D) + c·√( Σ_j W_{T_j}(H_j)²·t(R_j)/d_j ),     bits = (H_D + Σ_j H_j)/ln 2 + Σ_j n_j R_j,

with t(R) = 1/(4^R − 1), the cap bound on tan²φ from `codes/REPORT.md` [proved, unrefereed there]. The deterministic
term is chapter 2's floor (Theorem 2.24) and the block term is Lemma 3.1 at the sphere bound, so an "ideal" comparison
sets a floor against an achievable ideal, and does not say which built code wins [referee §1]. The **built** curves
multiply tan φ by the codes track's one-rate ratio ρ_c(R) (1.305 at 1 bit, 1.41 at 2, 1.69 at 7) and by 1.0245 for the
stored scale, and multiply the deterministic floor by 1.19 [measured; `codes/out_built.txt`,
`reports/bounds3/scale_check.log`, `quant/REPORT.md`, as read in DESIGN-CLAIMS §0]. In the layout programme that factor
instead rises from 1 at no bits to 1.19, a ramp the source calls an estimate, and each coded block is charged 16 bits
of scalars [model; DESIGN-CLAIMS C3b.2]. The spectra are groups 12 and 6 of
`agent1/spec_v1.json`, a 200k PCA subset with pseudo-query offsets, not the 1M index's dual basis, and c is 8 unless
stated.

The referee adds four omissions [referee §1]. First, **nesting**: the built block 0 at 56 B is a nested code that also
serves the later rises, and it is 1.625 times the sphere bound at its 2-bit rise (`codes/REPORT.md`, nesting 1.157), or
1.65 with the stored scale on the dump, where the model uses the one-rate 1.41. The model's first-level block is
therefore about 15% too good in tan φ. Second, the factor 1.19 is held constant at every rate, and that alone drives the
built-curve optimum's long prefixes, so those prefixes are a consequence of the assumption and not a finding. Third, the
index stores ‖y_j‖ for every block, where the model's D carries one 0.5 B norm. Fourth, the scale η_i of an item is
1.28 to 1.52 times the model's on the dump (§4.9.3).

### 4.9.2 The hand-over from deterministic to randomised levels

A rotated block T of n_T positions at R bits a position is narrower than its own zero-bit Cauchy–Schwarz bound exactly
when c²·tan²φ < d_T. At the sphere bound this needs B₀ = (n_T/2)·log₂(1 + c²/d_T) bits, which tends to
c²/(2 ln 2) = 0.721·c² as d_T grows: 35.7 bits at c = 8 and n_T = 96, and 46.2 bits for a long block [model; DESIGN-CLAIMS C3a.1; recomputed by the
referee §2]. A sign per position needs c²π/2 bits in the limit, 100.5 at c = 8, which is π ln 2 = 2.18 times more.

Beating the same bits spent deterministically is harder. The block wins at B = n_T·R bits exactly when

    c²/(d_T·(4^R − 1)) < ω_T(B ln 2)²,

so a flat block wins iff d_T > c² and B > B₁ = (n_T/2)·log₂(d_T/(d_T − c²)), which is 77.6 bits at c = 8 and
n_T = 96 [model; DESIGN-CLAIMS C3a.2; recomputed by the referee §2]. At high rate the condition becomes
d_T·(GM/AM)² > c², with AM and GM
the arithmetic and geometric means of the block's amplitudes: a rotation pays the block's spread of amplitudes and a
deterministic code does not [model; same source].

The first randomised level sits just on the losing side of this margin. With built codes the block of 96 at 2 bits
wins when c·(tan φ over the sphere bound)·1.0245/1.19 < √(d_T·(1 − 4^(−2))) = 9.44. With its own constant 6.931 and the
measured nested code (1.625) the left side is 9.70, 3% on the losing side; with a one-rate
code it would win by 11% [model; the referee §2, E1, which corrects DESIGN-CLAIMS C3a.2]. The nesting of §4.9.1 is
therefore what puts the first level on the wrong side.

Over the whole frontier the hand-over follows [computed in the model; DESIGN-CLAIMS C3b.3; `c3b_g12_c8_ideal.log`,
`c3b_g12_c8_built.log`]. With ideal curves a layout found by the search first beats the deterministic code at 16 B,
and none found does at 12 B. With the built codes the first such budget is 40 B, by 3%, and none found does at 32 B, so
the hand-over lies between 32 and 40 B in the search. The search is local, so each figure is an upper bound on the
hand-over budget; a wider search by the referee, with ideal curves, improved the frontier by at most 0.2% at 32 and
56 B [referee §3, §3a]. The built schedule hands over at 32 B, where the built codes put it. Group 6 gives the same
budgets [computed; `c3_tables.log`, `c3b_g6_c8_*.log`, `c3c_g6_c8.log`]. The hand-over moves as c²: at c = 4 it is at most 4 B with ideal curves and at most 8 B with built
ones [computed in the model; DESIGN-CLAIMS C3b.4 item 2, with the referee's "at most"]. These c = 4 figures describe a
certificate of the same form at a larger failure probability; they do not describe the approximate mode as built,
whose deterministic levels are read with a statistical width rather than against W [DESIGN-CLAIMS C3b.4 item 4]. At
equal bytes the best layout found is at least 1.4, 2.1 and 2.7 times narrower than the deterministic code alone (W) at
32, 96 and 256 B with ideal curves, and at least 1.2, 1.5 and 1.9 times narrower than the built deterministic code
(1.19·W) at 56, 96 and 256 B with built ones [computed in the model; same sources].

In time the first rotated block is itself at the margin. With a block-rise charged at its decode time (§4.9.4), coding
no block at the first level is between 3% wider and 11% narrower than coding one, and the crossover, 1.7 ns a
block-rise, lies inside the decode-alone range of 1.4 to 2.8 ns [model; DESIGN-CLAIMS C5.3 item 3; the referee §4.2, who finds "one rather
than none" not robust]. The "none" side also uses 1.19·W at 556 to 1,044 bits, beyond the 296 bits at which 1.19 was
measured, so this point stays open.

Under chapter 3's Theorem 3.11 (R8), taken at its worst-case half-width (Ê = 0), a coded block is never wider than its
own Cauchy–Schwarz bound, so B₀ disappears; and a flat block with d_T > c² beats the same bits spent deterministically
from its first bit, while one with d_T ≤ c² never does [the interval is proved, refereed (chapter 3); the numbers are
model; the referee §4.4, restating DESIGN-CLAIMS C5.4 for R8]. On the built layout this is worth little, because
chapter 3 records ξ² = 0.092 at 56 B on the dump and 0.6% of bytes.

### 4.9.3 The rate–margin law for bytes read

**The law** [model; DESIGN-CLAIMS C4.1 items 1 to 3; the algebra checked by the referee §5]. Item i has margin
|s_i − t|, scale η_i and normalised margin ũ_i = |s_i − t|/η_i.

1. *Per item.* An item is decided at the first level whose half-width is below its margin, up to the error of the
   estimate: it survives a level of normalised width w̃ exactly when |ũ_i + (w̃/c_e)·Z| ≤ w̃, with Z the standardised
   error. So the bytes an item reads are a function of ũ_i alone, a staircase that steps at the level widths.
2. *Per query.* bytes = Σ_ℓ b_ℓ·#{i : ũ_i < w̃_(ℓ−1)}, which is Lemma 4.4's shell count with the margin measured in
   units of each item's own scale.
3. *Local form.* Along the frontier, dB*/d log₂(1/ũ) = n_⋆(ũ), so the bits per item are
   ∫₀¹ n_⋆(ũ)·Pr[ũ_i < ũ] d log₂(1/ũ), with i a random item: each halving of the width costs n_⋆ bits for every item still inside the shell of
   that width. At an optimum n_⋆ = Σ_j n_j cos²φ_j for a rotated layout and W_T/λ_w for a deterministic one, and this
   identity holds along the computed frontier to within 4% from 24 to 256 B [model and computed; DESIGN-CLAIMS C3b.1
   item 4 and C3b.3 item 6; derivation checked by the referee §3].
4. *Corrected.* Query type enters through ũ and through the class's own level widths w̄_ℓ, not through ũ alone. Within
   a class the answer size enters only through ũ, and the visible set is untested [measured; RESULTS §2; the referee
   §5, which replaces DESIGN-CLAIMS C4.1 item 4].

On the model's frontier (group 12, c = 8, ideal curves) an item at a tenth of its zero-bit bound needs at most about
56 B (the frontier is at 0.102 there), one at a hundredth at most about 215 B, and each further halving tends to n bits (96 B), so the law is not a power law
[computed in the model; DESIGN-CLAIMS C4.1; the referee §3 and §5 make these upper bounds and find 217 B by
interpolation]. The deterministic floor alone needs about 96 B at a tenth [computed in the model; same source].

**The dump test** [measured; RESULTS §1 and §2]. The 50k verify dump holds 9,736,897 index-plan records from 10 item
and 10 text queries at seven answer sizes (0.01% to 75%) over 49,500 BioCLIP-2 rows (n = 768); scripts
`c4_dump_test.py`, `c4_dump_test_replay.py`, `t2_criteria.py` and `t5.py`. It was recorded at the constant 9.33, and
smaller constants are replays of the same records in which a record counts only if its item survived every earlier
level; without the replay the test reads T3 as 0.957 at today's constants, with it 1.000. The scale η_i is the
blockwise Cauchy–Schwarz bound after the 32-byte prefix, so the dump tests the law in that normalisation and not in
the zero-bit one of the frontier above [referee §5]. Its findings, with the same verdicts at every constant tested:

- Each level's normalised width is a property of the level: the sd of ln w̃ is 0.061 to 0.146 in every (class, level)
  cell, against a refutation threshold of 0.3.
- Survival against ũ is the predicted step: the half-survival point is at 0.990 to 1.005 of w̄_ℓ in all 24 cells, and
  the 10–90 span is 0.67 to 1.13 of the predicted one.
- **The law predicts each query's bytes (T3).** Predicted over actual bytes of the randomised levels and full vectors
  is 1.001 pooled at 9.33 and at 8, and 1.000 at 6.724/7.639; the rms relative error per (query, size) is 0.049 to
  0.051, the worst 0.17 to 0.18 at item 0.01%, where the answer is 5 rows. T3 uses each class's own w̄_ℓ.
- Item and text survival differ by up to 28, 14, 20 and 40 points at the same ũ at the four levels (at 9.33; 27, 12,
  19 and 35 at 6.724/7.639), because text's w̄_ℓ
  is 5% to 19% larger; with ũ divided by each class's w̄_ℓ they agree within 2.4 to 6.1 points. This is item 4 above.
- The bits added per halving of w̄_ℓ are 0.92 to 1.23 of Σ_j n_j cos²φ_j at the level entered (T5, at 9.33 and 8), and the one step
  below 1 is within the scalars' share of that step.
- The estimate's error costs nothing net: decided on true margins with the same half-widths the law reads 238.0 B per
  item of N at item 50% (c = 8) and the index 237.9 [measured; RESULTS §3.1].

The dump has no record below 56 B, so the hand-over, the deterministic staircase and a query's absolute bytes are out
of its reach [RESULTS §5; DESIGN-CLAIMS C4.4].

**Where the model's inputs are wrong** [measured; RESULTS §3.2, `step3.py`, `chain_table.py`, `step3_c8.log`,
`chain_table.log`, 50k dump, c = 8]. DESIGN-CLAIMS C4.2 evaluated the law with Gaussian scores and one scale per group,
and it under-predicted in-distribution queries at large answers by a factor of 2. A chain of substitutions from that
model (130.8 B per item of N at item 50%) to the index (272.0 B) changes one input at a time, so the factors multiply
to the whole:

| Cell | Model | Index | Factor | Margins | Deterministic levels | Scale η | Normalised widths | Estimate |
|---|---|---|---|---|---|---|---|---|
| item 10% | 67.5 | 69.6 | 1.03 | 0.69 | 1.27 | 1.27 | 0.92 | 1.00 |
| item 50% | 130.8 | 272.0 | 2.08 | 1.68 | 1.035 | 1.28 | 0.935 | 1.000 |
| text 50% | 297.6 | 398.2 | 1.34 | 1.10 | 1.01 | 1.17 | 1.03 | 1.00 |

The factor is the model's error, not the index's. Given each item's margin and scale, the index's randomised levels
read what the law says (the estimate term is 1.000), and their normalised widths are 0.81 to 1.03 of the model's built
schedule. The model is wrong in two inputs. Its **Gaussian margins** are wrong by a factor 1.68 at item 50%, because
in-distribution scores are 1.5 to 2.1 times as dense within 0.0025 to 0.04 of the median threshold as a Gaussian of the
same sd; this term is specific to in-distribution queries at large answers, being 1.10 for text at 50% and 0.69 for
items at 10%. Its **scale η** is wrong by a factor 1.28 in bytes: each entrant's scale after the prefix averages 0.0796
at item 50% against the model's equivalent 0.0524, 1.52 times, which makes the absolute half-widths 1.23 to 1.59 times
the model's. This term is common to both classes (1.16 to 1.23 for text, where the scale factor 1.742 fitted in DESIGN-CLAIMS
C4.2, the source's κ, had absorbed it at 1M). For item queries η_i falls with the answer size (0.103 at 0.01%, 0.080 at 30% to 75%),
which refutes in direction the cause C4.2 had suggested [measured; RESULTS §3.2; the referee §5]. The dump holds only
the product Σ_j ‖y_j‖‖Δ_j‖, so the η term cannot be split between the offset, the residual and the 50k index's
three-group basis. If both terms carried to 1M, the model's 131.7 B at item 50% would become 131.7 × 1.68 × 1.24 =
274 B against the 276.2 B measured there; that is arithmetic on two factors, not a measurement [model; RESULTS §5;
the second factor is not the scale term's 1.28 (Editor's query 27)].

DESIGN-CLAIMS C4.3 splits the model's bytes into floor, level spacing, layout, code distance, and scalars and key. That
is a split of the model's bytes, not of what the index reads, and its "code distance" column understates the first
level, whose block is nested (§4.9.1) [the referee §5 and §7]. The code distance is a distance to the sphere bound and
not a recoverable saving: the codes track puts the best package that keeps the decode at about 3% of bytes
[estimate in `codes/REPORT.md`, entered here as model; the referee §6 item 7].

### 4.9.4 The price of a stored scalar

**In bytes** [measured; `bd_p2c.out` build line, read in DESIGN-CLAIMS C5.1; the referee §4.1]. The stored bytes per
item, cumulative by level with the key, are 6.5, 11.0, 15.5, 20.0, 28.5, 37.0, 66.5, 106.9, 176.8 and 276.3. One
(item, coded block, rise) costs 1 to 1.5 B (κ at 4 or 8 bits and tan φ at 4 bits), and each block costs 1 B of ‖y‖
once. That is about 1% of what an item reaching 256 B stores, and the scalars are 4% to 5% of what a query reads at
item 50% (about 14 of 301 B) [model on measured sizes; DESIGN-CLAIMS C5.1].

**In time** [measured; SYSTEM.md §4.2 = `bd_p2c.out`, profiled, 999,000 BioCLIP-2 rows, one thread; as in §4.4's timed
cost]:

| ms per query at 1M | Item 1% | Item 50% | Text 1% | Text 50% |
|---|---|---|---|---|
| Level decode | 1.24 | 18.23 | 7.36 | 19.55 |
| Bounds and decisions | 1.19 | 21.23 | 8.23 | 22.66 |
| of which the randomised scalars' decode | 0.58 | 10.78 | 4.20 | 11.61 |
| Re-pack and compaction | 1.66 | 9.14 | 7.97 | 8.90 |
| Full vectors | 0.86 | 10.90 | 1.11 | 13.54 |
| Profiled wall | 12.79 | 87.69 | 47.97 | 93.34 |
| Scalars' decode, share of the wall | 4.5% | 12.3% | 8.8% | 12.4% |

To turn the scalar phase into a cost per rise needs the number of rises decoded, which the breakdown does not print.
DESIGN-CLAIMS C5.1 infers 5.4 to 6.0 rises per item of N at item 50% from the shares entering each level, and so
**about 1.4 to 2.8 ns per (item, coded block, rise)** for the decode alone [model: a measured phase over inferred
counts; DESIGN-CLAIMS C5.1]. **The referee finds this figure uncertain by a factor of 2 and biased high** [referee §4.1].
The entering shares come from SYSTEM.md §4.1, which is the final build (constants 6.931/7.963), while the phases come
from the p2c build (7.413/9.086), which sends more items deeper; the inferred count is therefore low and τ_rise high. The dump test's 5.60 rises per item of N at item 50% (5.73 at c = 8) agrees with the
inferred count, on a different index [measured; RESULTS §4]. The all-in figure, 3 to 10 ns per extra (item, coded
block) at the first level, rests on one sentence of `rust/REPORT_BOUNDS.md` Part 2 ("8–24% slower at 1M") that names
no cell or build, and that trial changed every level's rates as well as the number of blocks, so it is uncertain by a
factor 3.3 and confounded [referee §4.1].

For scale, from measured phases over inferred counts at item 50%: τ_byte is about 0.133 ns per code byte, τ_lvl about
3.2 ns per (item, level) (2.4 to 3.6 ns depending on which phases are counted), and a full vector about 104 ns [model;
DESIGN-CLAIMS C5.1; the referee §4.1]. So one block-rise costs 1 to 1.5 B but as much time as decoding 11 to 21 B of
code at the decode-alone figure. By 256 B an item has paid 11 block-rises, 15 to 110 ns, against about 34 ns to decode
its 253 B of code [model; DESIGN-CLAIMS C5.5, entered by the referee §7]. The direction this points is fewer rises per
item, not more blocks per level.

The constant is a second cost of a coded block: it is 6.931 with one block coded and 7.963 with several at 1M (chapter
3 §3.9), so a level that codes a second block pays 15% on its whole random term [computed; DESIGN-CLAIMS C5.1].

**The design rule that follows.** A proposal that changes the codes or the schedule is costed twice: in bytes, and in
time per stored scalar read and decoded, with each (item, coded block, rise) and each level entered counted. The
analyses of DESIGN-CLAIMS C3 and C4 counted the first and not the second, and the conclusions that changed are listed
in §4.9.6 [a rule of method, from DESIGN-CLAIMS C5 and the measurements above; no status tag applies].

### 4.9.5 The first randomised level with the scalars charged

**The cost of one more coded block** [model with measured costs; DESIGN-CLAIMS C5.2, `c5a.py`, `c5a.log`]. A
narrower first level changes one count, the share of items entering 96 B, because an item enters a later level when its
margin is inside the previous level's width and the later widths do not move. Per item of N, with J blocks coded at
56 B in place of one,

    Δtime = e_56·(J − 1)·τ_rise − Δe_96·(τ_lvl + 36·τ_byte + 3·τ_rise),
    Δbytes = e_56·(J − 1)·1.5 − Δe_96·(36 + 4.5),

with Δe_96 the fall in the share entering 96 B. On the margins of DESIGN-CLAIMS C4.2, coding three or four blocks
would save time only below 0.3 to 1.1 ns a block-rise, under the decode-alone figure, because every entrant pays for the
extra blocks and only the fraction then decided is saved. Two blocks at free rates is the one alternative near the
margin, paying below 1.5 to 3.3 ns at 1% answers and 0.7 to 1.5 ns at 50%. In bytes two or three blocks at free rates
save 0.2 to 2.9 B per item of N.

The model over-predicts what a narrower first level gains: it gives a fall of 6% to 26% in the items entering 96 B
where the native trial of three coded blocks at 56 B measured 2% [model against measured; DESIGN-CLAIMS C5.2 item 4,
kept by the referee §4.1]. The likely cause is the model's block norms after the prefix, since the index's block 0 is
about 1.5 times heavier [model; DESIGN-CLAIMS C5.2 item 4 and C3 (c) item 4].

**The layout programme with a charge per coded block** [model; DESIGN-CLAIMS C5.3, `c5b.py`, `c5b.log`; the rows at 1 and
2 B are the referee's rerun of the same code, `referee/rd_c5b_s.py`, `rd_c5b_s.log`]. The programme of §4.9.1 at the
built level's budget (56 B of code), group 12, built curves, with b_blk charged per coded block and the exact
one-block constant for one block (6.931 at d_T = 95) and 7.963 for two or more (certified for the built layout only, an estimate for the
others). In time, b_blk = τ_rise/τ_byte turns the time objective into bytes of decode. Half-width h/F:

| b_blk | None | One block | Two | Three | Four | Best |
|---|---|---|---|---|---|---|
| 1.0 B | 0.2106 | 0.1755 | 0.1720 | 0.1651 | 0.1661 | three, −5.9% |
| 1.5 B | 0.2089 | 0.1755 | 0.1740 | 0.1691 | 0.1720 | three, −3.6% |
| 2.0 B | 0.2073 | 0.1755 | 0.1760 | 0.1728 | 0.1787 | three, −1.5% |
| 2.5 B | 0.2056 | 0.1755 | 0.1780 | 0.1769 | 0.1853 | one, by 0.8% |
| 11 (1.4 ns) | 0.180 | 0.1755 | 0.218 | 0.283 | 0.411 | one |
| 14 (1.9 ns) | 0.172 | 0.1755 | 0.235 | 0.343 | 0.645 | none |
| 23 (3 ns) | 0.151 | 0.1755 | 0.299 | 0.716 | | none |

Three readings, as the referee corrected them [referee §4.2, replacing DESIGN-CLAIMS C5.3 item 1]:

1. **In time, at most one coded block at the first randomised level.** From b_blk ≈ 10.5 (1.4 ns) on, two blocks are
   at least 24% wider than one (0.218 against 0.1755 at 11), and the native trial lost on time. This holds over the
   whole range 1.4 to 10 ns, and the model's over-predicted gain only widens the margin.
2. **In bytes, a near-tie.** One block wins above about 2.3 B per coded block and three blocks below it. The index
   stores ‖y_j‖ for every block whether coded or not, so the marginal stored cost of one more coded block is κ and tan φ
   only, 1 to 1.5 B, at which three blocks win by 3.6% to 5.9% (the referee's "4 to 6%") in the model. Every margin
   here is a few percent, below the model's known errors.
3. One block against none in time is undecided (§4.9.2).

The later levels are not affected in the same way. Charged 2.5 B per coded block in stored bytes, at the budgets of
104.9, 174.8 and 274.3 B, the programme codes three, four and two blocks (0.0726, 0.0282 and 0.0100 of F), so charging
the scalars in bytes does not reduce the number of blocks there [model; DESIGN-CLAIMS C5.3 item 4].

**Several blocks under Theorem 3.12** [model; the referee §4.4, `referee/rd_c5c_r8.py`, `rd_c5c_r8.log`]. Chapter 3's
Theorem 3.12 is a proved several-block interval (R8 for several blocks, [proved, refereed] there): at Ê = 0 its
half-width is c·sd/√(1 + ξ²) with one pooled ξ² = c²/Θ and Θ = Σ_j d_j/tan²φ_j. Θ is dominated by the finest-coded
block, so the shrink is small. The referee recomputed the first-level alternatives with it, in DESIGN-CLAIMS C5.4's
model, against the built level under Theorem 3.11 at Ê = 0 (0.2394·F):

| First level | ξ² | h/F | Against built | Saves time below (four cells) | Bytes per item of N (item 1%, item 50%, text 1%, text 50%) |
|---|---|---|---|---|---|
| Three blocks at 1, ½, ⅛ bit | 0.27 | 0.1825 | −23.8% | 0.5 to 1.5 ns | −0.3, −2.0, −3.3, −1.1 |
| Four blocks at ½, ½, ⅛, ⅛ | 0.43 | 0.1981 | −17.3% | 0.2 to 0.6 ns | −0.1, −0.2, −1.3, +1.5 |
| Two blocks at 0.93, 0.80 (free) | 0.23 | 0.1856 | −22.5% | 1.1 to 5.8 ns | −0.4, −2.6, −3.7, −2.2 |
| Two blocks at 1, ½ (menu) | 0.29 | 0.2138 | −10.7% | 0.4 to 1.4 ns | −0.1, −0.8, −1.5, −0.1 |

These figures replace DESIGN-CLAIMS C5.4's two brackets, an unproved per-block approximation and a sketch, neither of
which enters. Three blocks are about 24% narrower in the model, but they save time only below 0.5 to 1.5 ns a
block-rise, which is under the decode-alone 1.4 to 2.8 ns in every cell but text 1%. Two blocks at about 1 and ¾ bit at
free rates remain the one multi-block first level that could pay in time, more at 1% answers than at 50%. All of these
figures carry two cautions: the model's first-level block is about 15% too good in tan φ, because it uses the one-rate
code figure where the built block is nested (§4.9.1), and the model's gain at this level was several times the trial's.

### 4.9.6 What survives as a candidate, and what is withdrawn

**The candidate: one wider block at the same 24 B** [model; DESIGN-CLAIMS C5.3 item 2, `c5e.py`, `c5e.log`; C5.7 V1].
Keep the 256-bit prefix and the 24 B of code, and widen block 0: 192 positions at 1 bit give 0.194·F (−19% against
the built level's 0.2405·F under the plain rule), 224 positions at 0.86 bit 0.192 (−20%), and 256 at ¾ bit 0.192. With
the norms rescaled so that block 0 holds 48% of the sum after the prefix, as the dump's level-0 means suggest (the model
has 38%), the three give −14%, −14% and −13%. Under Theorem 3.11 at Ê = 0 the first and last give −22% and −26%, and
−17% and −19% with the dump-like norms [model; the referee §4.4]. The candidate has one block-rise, the same code bytes
and the same scalars as the built level, so it survives the scalars. Its source expects a few percent rather than 14,
because the model over-predicted the trial's gain several times. Two later facts pull in opposite directions [the
referee §6 item 6]. Nesting favours it: in a 1-2-3-4 schedule the 1-bit first rise has no nesting loss, while the
built 2-bit rise carries 1.157 (`codes/REPORT.md`). The dump caps it, as follows.

**The cap on any change to the first randomised level** [measured; RESULTS §3.3 and §4, `step3.py`, `step3_levels.py`,
50k dump, c = 8]. At 56 B the uncoded blocks' Cauchy–Schwarz term is 76% of the mean half-width over entrants for item
queries and 81% for text (79% and 84% at 6.724/7.639), where the model has 84%. Setting that term to zero cuts the
randomised bytes at item 50% from 238.0 to 206.5 B per item of N, so **any change to that level can save at most
31.5 B per item of N, 13% of the randomised bytes, at item 50%**. The test that would decide the candidate must
rest on exact entrant counts multiplied by per-unit costs from the phases, because its predicted time gain at item 50%
is about 1 ns per item (1.3% of the query) against ±5% timing noise [model; DESIGN-CLAIMS C5.7].

**Withdrawn: coding every block at the 56-byte level.** DESIGN-CLAIMS C3 (c) item 1 recommended coding all four blocks
there, since the uncoded blocks dominate its width, and a sub-bit code where it would pay most. On time it loses at
every block-rise cost the sources allow (§4.9.5), and the native trial of three coded blocks at 56 B (`--design-exact`)
cut the items entering 96 B by 2% and was 8% to 24% slower at 1M [measured; `rust/REPORT_BOUNDS.md` Part 2, one
sentence, as read in DESIGN-CLAIMS C5.1]. In bytes it is the near-tie of §4.9.5. The codes track closed the code change
as well: nested, table-decodable codes keep 6% to 8%, the one-rate gains do not survive nesting, and a one-rate sub-bit
code applies only to a block whose last rise is its only rise, which no block at 56 B is [measured; `codes/REPORT.md`;
the referee §6 item 1].

Other conclusions of DESIGN-CLAIMS C3 and C4 that the scalars change [model; DESIGN-CLAIMS C5.6, with the referee §4.1
and §7]:

- The ideal optimum codes every coordinate from the first rotated bit only if scalars are free; at 2.5 B per coded
  block it codes one block at the first level, and in time one or none.
- The scalars are 5% to 25% of the model's bytes, but 8% to 52% of the per-item time, depending on the cell and on
  τ_rise.
- More levels lower the spacing loss in bytes; in time each level entered costs about 3.2 ns per entrant and its rises,
  and it is the largest component in every cell only for τ_rise ≤ 3 ns [the referee §4.1, E4]. `rust/REPORT_RAND.md`
  measured this direction: going from 14 levels to 4 was faster at every cell for 3% to 11% more bytes [measured, as
  read in DESIGN-CLAIMS C5.5].
- For an interval narrower than the plain rule, implement R8 (chapter 3's Theorems 3.11 and 3.12); the one-block
  shrunk-centre interval (A4) alone is not a better one-sided bound [the referee §4.4 and §6 item 5].

## Open

- **Item-by-item floors.** Claim 4.9, that shared summaries cannot lift a fixed-k query off linear growth in the family
  of per-row streams plus shared structure, needs floors stated row by row rather than for mean widths under a
  distribution [sublinear/REPORT.md §4, §10].
- **Unconditional separation.** Nothing known separates polylogarithmic from linear work for exact search with a
  near-linear index [cited; sublinear/REPORT.md §3.1].
- **Exponents beyond 1M.** Every exact-path exponent comes from 1.5 decades of nested subsamples of one sample of 20
  dense taxonomic slices, so the 10⁹ figures are a conjecture and growth by unrelated content is untested
  [sublinear/REPORT.md §10]. The M-norm key was not re-run across N.
- **Cached cluster extremes.** Their value depends on how often query directions repeat, which no query log has
  measured [sublinear/REPORT.md §9].
- **The approximate law at 100M and beyond.** u and v at 100M on Deep1B or BigANN/SIFT1B slices at 10M and 100M, and on
  LAION-100M or MS-Turing 100M for a high-dimensional, partly out-of-distribution case, decide whether leaves matter
  more (v under 0.3) or less (u under 0.3) than the model says [`layered.md` §8.1].
- **A floor for centre-keyed hierarchies.** `layered.md` §0 and §2 state that N^v is the best exponent a hierarchy of
  clusters keyed by centre and spread reaches on these data, and call it a property of the data. The source files this
  under its [measured] heading; it is an inference from fits on four datasets over a 16-fold range of N, and stays
  open until the PI rules on its status (Editor's query 14).
- **The approximate exponent as recall rises.** Whether v tends to one as the recall target tends to 1, as Theorem 4.7
  forces for the exact path at fixed slack, is not addressed by any source.
- **Dimension.** No second embedding model and no native large dimension were run; the dimension evidence is a PCA
  truncation of one model [sublinear/REPORT.md §7].
- **Cluster size for the exact path at scale.** The linear-in-N time model of rust/REPORT_CLUSTERSIZE.md is fitted to
  profiled phases at 1M and was not measured at larger N; no sizes between 1,000 and 4,000 or 4,000 and 16,000 were
  timed.
- **The cost of a block-rise, measured.** τ_rise is inferred, 1.4 to 2.8 ns for the decode and 3 to 10 ns all-in. The
  decode figure is uncertain by a factor of 2, because the entrant shares and the phases come from different builds,
  and the all-in figure by a factor of 3.3, because it rests on one unattributed sentence about a trial that also
  changed every level's rates. One build that
  prints the block-rises decoded per query and the entrants per level, beside its phases, fixes it (DESIGN-CLAIMS C5.7
  item 4; the referee §4.1). Whether the first level should code one block or none in time waits on it (§4.9.2).
- **A native test of the 56-byte level.** One index, one set of queries and one binary differing only in the first
  level: V0 as built, V1 one wider block at the same 24 B, V2 two blocks, V3 the `--design-exact` layout, with exact
  entrant counts at every level, the level's half-width and its parts from a verify dump, bytes split into code,
  scalars and full vectors, and time by phase. The model is believed only if the count entering 96 B alone moves, its
  change is within a factor 1.5 of the prediction recomputed from the measured block norms, and the time difference
  matches the cost formula of §4.9.5; on the earlier trial it fails the second test (DESIGN-CLAIMS C5.7).
- **The hand-over and the deterministic staircase on data.** The dump has no record below 56 B, so the hand-over of
  §4.9.2, the law's deterministic steps and a query's absolute bytes are untested; a dump extended to the six
  deterministic levels would test them (RESULTS §5; DESIGN-CLAIMS C4.4).
- **The rate–margin law under a visible set**, and with the zero-bit normalisation of its frontier table: both untested
  (the referee §5).
- **The two wrong inputs at 1M.** Whether the margins term (×1.68) and the scale term (×1.28) carry from the 50k dump
  to the 1M index, what the scale term is made of (offset, residual or the 50k index's three-group basis), and why the
  1M index reads 133.2 B at item 10% where the 50k index reads 69.6 B and the model 67.5 (RESULTS §3.2, §5).
- **A lower bound on h*(B) over all layouts.** The frontier is a local search and an upper bound; no lower bound over
  all layouts was found, and a continuous relaxation does not give one (DESIGN-CLAIMS, "What failed"; the referee §3).
- **The built rates at 96 and 160 B.** With the 50k build's rates, blocks 0 and 1 carry about twice block 2's share of
  the variance per unit of n_j cos²φ_j at 96 B, and block 0 about twice at 160 B, which is indicative of a misallocation and is not a finding (the referee
  §4.3).

## Editor's queries

1. **Proposition 4.2, degenerate vectors.** The map needs |a| ≥ 1 and |b| ≤ n − 1. The zero vector of A and the
   all-ones vector of B are trivial cases that the source does not treat. The source also leaves η′ (its δ′)
   unspecified; the factor n is N^(o(1)) when n is polylogarithmic in N, so any η′ < λη works then. Neither point changes the result.
2. **Lemma 4.4, lower bound.** The lower bound 𝒩(t − w⁻) − 𝒩(t) ≤ |𝒰_ℓ ∩ non-matches| needs every row with small slack
   at stage ℓ to have survived the earlier stages, which holds when the intervals are nested across stages (or the
   slack is measured on their running intersection). The source's one-line proof does not state this.
3. **Proposition 4.5 and the shell.** Proposition 4.5 is the exponent of N·F̄(t_N − w), which counts the k matches with
   the shell, while Lemma 4.4's shell excludes them. The two agree when the shell is much larger than k. The
   proposition is also about an expected count, while the measurements fit counts.
4. **Corollary 4.6.** The source states the three tail cases as consequences of Proposition 3 without proof; the proof
   lines here are the editor's. The Gaussian case holds only to leading order in large z_N, because the Gaussian hazard
   rate is z only asymptotically, and the source states it as an equality. The statement here now says "to leading
   order"; the tag is the source's and the PI may prefer [sketch: gap named] for the Gaussian case.
5. **Theorem 4.7.** The proof works with the population threshold t_N rather than the empirical k-th best score, and
   with the population share rather than the sample share. A statement about the sample share needs a law of large
   numbers (or Glivenko–Cantelli) step and the assumption that F̄ has no atom at s*. The source does not say whether
   the limit inferior holds almost surely or in expectation.
6. **Theorem 4.1.** The source writes "E[bits] ≥ H(S(t)|q) − h(ε) − εN = N·h(p)", an equality that drops the h₂(ε) + εN
   terms. The statement here keeps them; they are negligible when ε is far below h₂(p). The bound also assumes rows
   match independently, so it is a bound over a distribution of corpora. agent3b/REPORT.md is marked "not
   independently verified".
7. **Proposition 4.3.** The source's proof is a one-line reduction to INDEX, and the model (what the summary answers,
   for which sub-collections and which τ) is not spelled out. The proof here is the editor's reading of that line.
8. **Lemma 4.10.** agent3b/REPORT.md gives the annulus bound and the concavity argument without a status label. The tag
   [proved, unrefereed] was assigned by the editor from the argument the source gives, and the PI may prefer to keep it
   untagged or move it to chapter 1, where Lemma 1.5 carries the same tag from other sources (`combined/REPORT.md`
   item 2, `DESIGN.md` §3).
9. **Label mapping.** "Argued" in sublinear/REPORT.md (the dimension slack, the on- and off-manifold limits, batching
   and caches) was mapped to [model]; its "What follows" paragraph was mapped to [sketch: gap named] because the source
   names the gap. "Derived" and "estimated" in `layered.md` were mapped to [model].
10. **Hashing exponents within sublinear/REPORT.md.** Section 3.4 quotes 0.81 to 0.93 for text and the summary table of
    §6 gives 0.81–0.93 (text) and 0.45–0.70 (item), with 34% to 79% and 1.6% to 26% of rows. The table of §5.6 shows
    text 0.81 to 0.89 and item 0.45 to 0.46, with 34% to 61% and 1.6% to 3.5%. The wider ranges may include
    configurations not tabulated (for example 16 entries a row, or the grid limit of 6 in `lsf_grid6.log`); this
    chapter reports the §5.6 table.
11. **Shell exponents within sublinear/REPORT.md §5.4.** The table gives a local exponent of 0.79 for the 3 sd shell at
    p = 10⁻⁵, while the text quotes exponents at k = 10 of 0.79 at 4 sd and 0.74 at 3 sd. The text's figures may be fits
    over the measured N range rather than local exponents at p = 10⁻⁵; the source does not say.
12. **Different rows and machines.** sublinear/REPORT.md uses 999,900 rows, SYSTEM.md and rust/REPORT_CLUSTERSIZE.md
    999,000, and agent3b/REPORT.md denser 200,000-row subsets. SYSTEM.md times on a Ryzen 9 5900X (AVX2) and
    rust/REPORT_CLUSTERSIZE.md on a 4-core AVX-512 Xeon VM, so their milliseconds cannot be compared.
13. **Three cluster-size rules.** sublinear/REPORT.md recommends about 1,000 rows a cluster at every N, for the exact
    path's exponent; rust/REPORT_CLUSTERSIZE.md recommends about 4,000 at 1M, for the exact path's time; `layered.md`
    uses the build rule n̄ ∝ √N (2,000 at 1M) and keeps "the exact path's large clusters" while adding 2,000-row leaves
    for the approximate path. The sources do not reconcile these for the exact path at large N.
14. **A floor stated under a measured heading.** `layered.md` §2 is headed [measured] but includes the statement that
    N^v is the best exponent any hierarchy keyed by centre and spread reaches on these data. That is an inference from
    the fits. It appears here under "Open" with the source's label stated beside it, rather than retagged as a
    conjecture, because the rules do not let the editor change a source's status; the PI should rule on it.
15. **Overshoot of the k-NN seed.** Proposition 4.8's 5% to 22% is quoted by sublinear/REPORT.md from knn/REPORT.md,
    which is not a source of this chapter; knn/REPORT.md gives it in bytes excluding bases on 1M BioCLIP-2. The same
    report says "cost is linear in N" for exact k = 10 on the strength of the share of rows touched at one size, which
    the fitted exponents of section 4.4 refine.
16. **Which baseline is 1,200 to 1,900 times above the entropy bound.** agent3b/REPORT.md §1 gives the ratio without
    naming the baseline. Read against its own 200,000-row table (baseline replica 0.564% to 5.336%, bound 0.00044% to
    0.0046%), the ratios are about 670 to 1,280. The 1,200 to 1,900 agrees roughly with the FINDINGS.md 1M baseline
    against h₂(p)/(16n) at that table's match shares, but the source does not say so. An earlier draft of this chapter
    attributed the ratio to the 200,000-row subsets; that attribution was removed.
17. **Dimension slack: "whatever the model".** sublinear/REPORT.md §7 marks the √n_eff slack "argued" and says it holds
    "whatever the model", yet gives no derivation of the 1/√n_eff factor. The tag [model] (from "argued") needs a
    stated model, which the source does not give; [sketch: gap named] may fit better. An earlier draft supplied a
    model ("a projection spread over n_eff directions") that is not in the source; it was removed.
18. **Which term leads under 1% visible sets.** `layered.md` §8.1's summary table says the centre scan is the largest
    term of "levels" at 1% visible (55 to 70% at 100M for n ≥ 384, 50 to 80% at 1B), but its own rows for glove (n = 128)
    give tables 1.6 MB against centres 0.8 MB at 100M, and codes 4.4 MB and tables 2.3 MB at 1B with centres not among
    the two largest terms. The chapter follows the rows.
19. **The text exponent of rows touched in |V|.** sublinear/REPORT.md §1 gives 0.96; its §5.5 table gives 0.97 for the
    share not decided by the range. The chapter quotes both where the source does; the source does not say whether they
    are different fits.
20. **Pestov's status.** sublinear/REPORT.md §3.4 marks ref. 12 **cited** ("abstract only"), and its reference list says
    "abstract, partly paraphrased". An earlier draft downgraded it to [cited, unchecked]; the chapter now keeps the
    source's [cited], and the PI may wish to have stage 0's library check it.
21. **Tags in §4.9.** DESIGN-CLAIMS labels the time per block-rise, the all-in cost, the per-level and per-byte times,
    the 11 block-rises by 256 B, the codes track's 3%, the "one or none" comparison in time (C5.3 item 3) and the
    deterministic factor's ramp (C3b.2) [estimate], which is not a tag of these notes. They enter as
    [model], each with what it rests on (a measured phase over inferred counts), which is not an upgrade. The design
    rule of §4.9.4 is a rule of method and carries no status tag; the PI may wish to state where such rules belong.
    The referee's report verifies derivations and reruns but referees nothing as proved, so no §4.9 statement is
    [proved, refereed] apart from chapter 3's Theorems 3.11 and 3.12, which it quotes.
22. **Two normalisations of η_i.** The law's frontier table (§4.9.3) is in units of the zero-bit Cauchy–Schwarz bound;
    the dump test measures η_i as Σ_j ‖y_j‖‖Δ_j‖ after the 32-byte prefix. The dump therefore confirms the law in the
    second normalisation, and the frontier table is untested (the referee §5). The two are not converted anywhere in
    the sources.
23. **Builds, constants and sizes in §4.9.** The model uses c = 8; the dump was recorded at 9.33 (build p1c) and
    replayed at 8 and at the 50k index's 6.724/7.639; the 1M index's constants are 6.931/7.963 and the profiled
    breakdown's build p2c used 7.413/9.086. The 50k build codes all four blocks from 96 B, the 1M build blocks 0–2 at
    96 and 160 B, so its stored bytes differ (107.5 and 175.2 against 106.9 and 176.8 B). DESIGN-CLAIMS C5.1 also
    corrected its own stored-bytes list for C3 (c) and C4 at four levels; the C4.2 figures quoted in §4.9.3 used the
    older list, and the totals move by under 2%.
24. **§4.4's timed cost and the mixed builds.** §4.4 quotes SYSTEM.md §4.1's entering shares (91% and 20% at item 50%)
    beside the fixed costs from `bd_p2c.out`. The referee (§4.1) finds that §4.1 is the final build and `bd_p2c.out`
    the p2c build, which sends more items deeper, so any per-item cost derived from the two together is biased; §4.4
    does not divide one by the other, but a reader may. The text 1% sentence of §4.4 ("95% enter the first level and 46%
    reach 56 bytes") uses "first level" for the candidate share, where §4.9 uses "first randomised level" for 56 B.
25. **"4 to 6%".** The referee states that three coded blocks win by 4 to 6% at 1 to 1.5 B per coded block; its own
    table gives 5.9% at 1 B and 3.6% at 1.5 B. §4.9.5 gives both.
26. **Where the source and the referee disagree on the first level.** DESIGN-CLAIMS C3a.2 puts the first randomised
    level 11% on the winning side of the high-rate margin at its own constant; the referee (E1) puts it 3% on the
    losing side with the nested code. §4.9.2 follows the referee, and gives the 11% only for a one-rate code.
27. **The second factor in "131.7 × 1.68 × 1.24".** RESULTS §5 carries "both terms", margins and η, to 1M as
    131.7 × 1.68 × 1.24 = 274 B, and the referee does not comment. The η term of RESULTS §3.2 is 1.28, not 1.24; the
    1.24 matches the product of the η, normalised-width and deterministic terms (1.28 × 0.935 × 1.035 = 1.239), which
    would make the sentence carry every non-margin term rather than η alone. The source does not say which it meant,
    so §4.9.3 quotes the arithmetic as written; the Open item on the two wrong inputs quotes ×1.28.
