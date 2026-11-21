# 4. What a query must read: shells, exponents, hardness, scaling

This chapter asks how the work of one query grows with the number of rows N, and whether any index could make it grow
more slowly. Under the owner's metric, which fixes the share of the corpus that matches, an entropy bound settles the
question at once, so section 4.1 first fixes what "sublinear" can usefully mean. Section 4.2 collects the lower bounds
and says which of them bind on the data. Section 4.3 states the cost law of the exact path: each level of a certificate
is read by the rows in a shell below the threshold, and the growth of that shell with N is set by the tail of the
query's scores. Sections 4.4 to 4.6 give the measured exponents, the visible set and the dimension. The approximate
mode obeys a different law, measured in different units on different data, so section 4.7 states it apart and then
compares the two. Section 4.8 sets cluster size against N.

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
written out; the bound above is chapter 1's annulus bound (§1.2, item 1) at x₂ = 1.

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
