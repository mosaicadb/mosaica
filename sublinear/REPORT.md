# Can a query's work grow sublinearly in N?

Saved by the coordinating session from the agent's returned text (the agent could not write files
itself); paths adjusted to this branch. Every number is the agent's.

Follows `DESIGN.md`, `FINDINGS.md`, `agent3b/`, `review/`, `knn/` and `groups/REPORT.md`. Data: the
1M BioCLIP-2 sample (n = 768), 369 text queries and 100 item queries. The 100 item-query rows are
removed from the corpus here, so item queries are out-of-corpus (999,900 rows). Everything is
counted, nothing is timed. Scripts, logs and tables are in this directory. Each claim is marked
**proved**, **measured**, **cited**, **argued** or **conjectured**.

Every bound was checked against the exact score wherever it was used: 1.85·10⁹ interval checks in
the zero-read experiment, 9.95·10⁹ in the two per-member runs, 7.5·10⁷ cluster-bound checks in the
hierarchy. **Violations: 0 in every run.**

## 1. Answers

1. **Worst case: no, unless the Strong Exponential Time Hypothesis is false** (cited, with the
   reduction in 3.3). No index with polynomial build time answers exact threshold or exact
   nearest-neighbour queries in N^(1−δ) time once the dimension grows faster than log N. The
   unconditional (cell-probe) bounds are far weaker, about n / log n probes, and say nothing about
   linear work. So linear work is not proved necessary, but no method can promise less.
2. **On this data the design is already sublinear at a fixed answer size, with an exponent close
   to one for text** (measured, section 5). With clusters kept at about 1,000 items, k = 10, N from
   31k to 1M:

   | Work at k = 10 | Text: at 1M | Text: exponent | Item: at 1M | Item: exponent |
   |---|---|---|---|---|
   | Items not decided by the sorted range | 75.3% of N | 0.93 | 22.2% of N | 0.64 |
   | Items not decided by range and support table | 69.1% | 0.91 | 9.3% | 0.49 |
   | Centres read (table cut) | 100% | 1.00 | 47% | 0.85 |
   | Coordinates read per member, 12 local bases at every N | 47.3 per item | 0.87 | 2.9 per item | 0.60 |
   | Coordinates read per member, local bases growing with N | 35.9 per item | 0.79 | 2.0 per item | 0.49 |
   | Items in clusters holding both matches and non-matches (oracle) | 0.30% | 0.12 | 0.32% | 0.22 |

   Bootstrap 95% intervals are within ±0.01 for text and ±0.07 for items (5.2, 5.3).
3. **Why, and what the limit is** (proved, section 4). A level whose certificate has slack w is
   read by exactly the items whose score is within w below the threshold. The exponent of that
   count is 1 − h(t − w)/h(t) − (a term for slack that shrinks with N), with h the hazard rate of
   the query's score distribution. Scores are bounded, so for any slack that does not shrink the
   exponent tends to one: **every fixed-slack level is asymptotically linear at fixed k**. The
   sublinearity seen between 10⁶ and 10⁹ comes from the threshold moving up the tail and from
   clusters and bases tightening as N grows. It is real, slow, and for text it leaves the first
   pass over about a third to a half of the items at 10⁹ (5.7).
4. **Only one thing changed the exponent by much: knowing the query's own cluster extremes.** That
   is the oracle row above. It is reachable for a repeated query direction (a cache row per query,
   valid for every threshold and every viewer), not for a fresh one. Everything else tested changes
   the constant or makes things worse: a hierarchy over centres reads 29% more centres for text;
   idealised hashing with 16 KB of tables per item still leaves 34% of items as candidates for text
   at exponent 0.81; finer clusters, anchors and group summaries were already ruled out.
5. **Visible set** (measured, 5.5). For a random visible set V inside the 1M clustering,
   coordinates read grow as |V|^0.84 (text) and |V|^0.59 (items), items touched as |V|^0.96 and
   |V|^0.81, and the centres read stay at min(K, about |V|). Work is close to linear in |V| with a
   floor of K centres.

## 2. What "sublinear" should mean here

- **Fixed answer fraction p.** The entropy bound (agent3b) is N·h(p) bits, linear in N. Any index,
  of any size, reads linearly many bits. Only the constant is in play: h(p) is 0.0015 bits per item
  at p = 0.01%, against about 28 bytes per item the design reads for text. That gap is the price of
  a size-capped index and belongs to the per-item work.
- **Fixed answer size k** (10 to 10,000, and exact k-NN with k = 10). The entropy bound is about
  k·log₂(N/k) bits, so information theory allows sublinear work. The quantity to track is W(N; k)
  for N from 10⁶ to 10⁹, in three units: centres read, items touched (any byte of an item read),
  and bytes or coordinates read. "Sublinear" means W = O(N^ρ) with ρ < 1 over that range; an
  exponent that tends to one beyond it is still worth having inside it.
- **Index-size cap.** Without one the question is empty (a table of answers). The cap used is the
  design's: a constant number of bytes per item, plus shared structures of which a query reads
  o(1) bytes per item.
- **Visible set.** W as a function of |V| for a fixed corpus, for V random and for V contiguous in
  id order.
- **Model of growth.** N grows by sampling more items from the same distribution ("densifying"):
  nested random subsamples of the 1M sample. Then a query's score distribution F is fixed and the
  threshold for k matches is the (1 − k/N) quantile. Growth by adding unrelated content is easier:
  the new items are far from the threshold.
- Two effects are in the measured exponents and are separated where possible: the answer fraction
  k/N falls, and the clusters (K = N/1000) and local bases get tighter.

## 3. Lower bounds: what is known, and whether it binds

### 3.1 Unconditional (cell probe)

- Pătraşcu and Thorup [1] (**cited**; theorem list read in the paper): a randomised index of S
  cells for exact near-neighbour search over N points in {0,1}^d makes Ω(d / log(S·d/N)) probes;
  the same for partial match. With near-linear space this is Ω(d / log d). Earlier: Ω(log d)
  (Borodin, Ostrovsky, Rabani [2]), Ω(d / log N) for polynomial space (Barkol, Rabani [3]; Jayram
  et al. [4] for partial match). The authors state these were then the highest bounds known for any
  static data-structure problem.
- **These do not bind.** At n = 768 with 16-bit coordinates the bound is of the order of a
  thousand probes. Nothing unconditional separates polylogarithmic from linear work for exact
  search with a near-linear index.

### 3.2 Conditional on SETH

- Under the Strong Exponential Time Hypothesis, Orthogonal Vectors on N vectors in dimension
  ω(log N) needs N^(2−o(1)) time (Williams [5]). Abboud, Rubinstein and Williams [6] extend this to
  approximate bichromatic maximum inner product; Rubinstein [7] to (1+ε)-approximate bichromatic
  closest pair in Euclidean, Manhattan, Hamming and edit distance, and states the consequence:
  near-linear query time for approximate nearest-neighbour search with polynomial preprocessing
  (**cited**; abstract read). Chen [8] gives exact maximum inner product hardness in very low
  dimension.
- In the other direction, Alman and Williams [9] answer N exact Hamming nearest-neighbour queries
  in dimension c·log N in N^(2−1/Õ(c)) time. Offline and with fast matrix multiplication, a batch
  can beat N per query by a factor that vanishes as c grows (at N = 10⁹ and n = 768, c is at least
  25 even at one bit per coordinate).

### 3.3 The reduction for this problem (proved, given the Orthogonal Vectors conjecture)

**Proposition 1.** Suppose an index over N unit vectors in dimension d can be built in N^c time and
answers "is there x with q·x ≥ t?" in N^(1−δ) time, for d = ω(log N). Then Orthogonal Vectors is
solved in N^(2−δ′) time.

*Proof.* Given A, B ⊂ {0,1}^d, map b to x_b = (1 − b)/√(d − |b|) and a to q_a = a/√|a|. Split B
by w = d − |b| (at most d classes). In a class, q_a·x_b = (|a| − a·b)/√(|a|·w), which is √(|a|/w)
when a·b = 0 and at most (|a| − 1)/√(|a|·w) otherwise; any t between the two makes "some x with
q_a·x ≥ t" the same as "some b in the class orthogonal to a". Cut each class into pieces of N^γ
vectors, build an index on each, and ask every a of every piece: d·N^(1−γ)·N^(γc) to build and
d·N·N^(1−γ)·N^(γ(1−δ)) = d·N^(2−γδ) to query, subquadratic for γ < 1/c. ∎

The hard instances have many items within 1/d of the threshold in cosine. That is the property the
measured data lacks at the top of the tail and has in the bulk, and it is why section 4 states cost
by distance from the threshold.

### 3.4 Restricted models that match the families on the table

- **Hashing (list-of-points).** Andoni, Laarhoven, Razenshteyn and Waingarten [10]: space
  N^(1+ρ_u) and query time N^(ρ_q) with c²√ρ_q + (c² − 1)√ρ_u = √(2c² − 1), tight in a model that
  covers hashing-based schemes, on random instances (**cited**; abstract read, model as summarised
  from [11]). With near-linear space ρ_q = (2c² − 1)/c⁴. For matches at cosine τ against a
  background at cosine 0 this is ρ_q = 1 − τ². As the gap closes (c → 1, exact search with items
  just below the threshold) ρ_q → 1. For text here, after removing the corpus mean, τ is about 0.3
  and the background has spread, so the family's exponent is 0.9 or more. Section 5.6 measures the
  idealised family at 0.81–0.93 for text.
- **Metric trees and pivots.** Pestov [12]: within a stated model, an Ω(N^(1/4)) average-case bound
  for metric-tree schemes when the dataset size is superpolynomial and subexponential in the
  intrinsic dimension (**cited**; abstract only). Chávez et al. [13] define intrinsic dimension
  ρ = μ²/(2σ²) of the distance histogram and bound what a pivot scheme can discard in terms of it
  (**cited**; Definition 9 and section 7.2 read). Here ρ = 279 for text queries and 92 for item
  queries (**measured**, `tail.txt`). Weber, Schek and Blott [14] is the classic statement that
  partitioning degenerates to a scan in high dimension.
- **Low intrinsic dimension.** Exact search in logarithmic time needs a small expansion constant c
  (the factor by which a ball grows when its radius doubles): cover trees answer in O(c¹² log N)
  (Beygelzimer, Kakade, Langford [15], **cited**; abstract read), navigating nets likewise for
  constant dimension (Krauthgamer, Lee [16]; Karger, Ruhl [17]). **Measured** here at the 10th
  neighbour: c = 4,050 for item queries (median) and c = N/k, the largest value possible, for text.
  c¹² exceeds 10⁴³. These guarantees are vacuous on this data at any N of interest.
- **Range searching.** A threshold query is halfspace range reporting. In dimension D with
  near-linear space the known query times are N^(1−1/⌊D/2⌋) for halfspaces (Matoušek [18]) and
  N^(1−1/D) for simplices (Matoušek; Chan [19]), and N^(1−1/D) is a lower bound for simplex
  reporting with linear space in the pointer-machine model (Afshani and Cheng [20], **cited**;
  abstract and section 1.2 read). The data's intrinsic dimension is about 20 by three estimators
  (**measured**, 5.4), which would put those exponents at 0.90–0.95. This is a transfer by analogy:
  the bounds are for worst-case point sets in R^D, not for a 20-dimensional set in R^768.

**Does the data's structure escape the worst case?** Partly (**measured**). Item queries do far
better than the 0.90–0.95 a dimension of 20 would suggest (0.49–0.64), because their matches sit in
a few tight clusters. Text queries do not: they are off the data (10th-neighbour distance 0.75,
falling as N^(−0.015)), every ball around them that holds 10 items doubles to hold everything, and
their exponent is 0.91–0.93.

## 4. The cost law of the design

Notation: s_i = q·x_i; n(τ) = #{i : s_i ≥ τ}; F̄ the survival function of a query's scores over the
corpus distribution, f its density, h = f/F̄ its hazard rate; t_N the threshold with F̄(t_N) = k/N.

**Proposition 2 (shell identity, proved).** A scheme reads an item's data in stages. After stage ℓ
item i has a sound interval [L_iℓ, U_iℓ] and stops once the threshold is outside it. Let b_ℓ be the
bytes of stage ℓ and A_ℓ the items not stopped after stage ℓ. Then bytes read =
Σ_ℓ b_ℓ·|A_(ℓ−1)|, and if every non-match has upper slack U_iℓ − s_i between w⁻ and w⁺, then
n(t − w⁻) − n(t) ≤ |A_ℓ ∩ non-matches| ≤ n(t − w⁺) − n(t).

*Proof.* A non-match is not stopped exactly when U_iℓ ≥ t, that is s_i ≥ t − (U_iℓ − s_i). ∎

So each level's work is a shell count: the number of items within that level's slack below the
threshold.

**Proposition 3 (exponent of a shell, proved).** For items drawn independently, the expected shell
count N·F̄(t_N − w), with slack w = w(N), has

d ln(shell) / d ln N = 1 − h(t_N − w)/h(t_N) + h(t_N − w)·dw/d ln N.

*Proof.* F̄(t_N) = k/N gives dt_N/d ln N = 1/h(t_N). Differentiate ln N + ln F̄(t_N − w(N)). ∎

There are two sources of sublinearity.
- **The tail's shape:** the ratio of hazard rates at the shell's edge and at the threshold. An
  exponential tail (constant hazard) gives exponent 0. A Gaussian tail gives w/z, with z the
  threshold's height in the same units as w. A tail that ends at a largest score s* with
  F̄(s* − δ) ∝ δ^a gives w/(w + δ) with δ = s* − t_N.
- **Slack that shrinks with N:** with w ∝ N^(−η) the last term is −η·w·h(t_N − w), that is η times
  the number of e-folds of density inside the slack.

**Proposition 4 (asymptotically linear, proved).** If scores are bounded above by s*, and every
item's upper slack at stage ℓ is at least w > 0 whatever N is, then for every fixed k the share of
items not stopped after stage ℓ has a limit inferior of at least F̄(s* − w) > 0.

*Proof.* A_ℓ contains every item with t_N − w ≤ s_i < t_N. As k/N → 0, t_N → s*, so that set's
share tends to F̄(s* − w). ∎

Cosines are bounded, so this applies to every certificate whose slack does not shrink to zero. With
clusters of fixed size the cluster radius falls as N^(−1/D) with D about 20 (5.4), so the slack
shrinks, but slowly.
- For a query on the data manifold, slack and neighbour distance shrink together and the touched
  count tends to a constant of the order C^D, which is never reached.
- For a query off the manifold (text), the slack shrinks as N^(−1/D) and the exponent tends to
  1 − a/D, with a the tail index at the top of that query's scores.

**Proposition 5 (k-NN, proved in the interval model).** An algorithm that returns the exact k best
items must hold, for every other item, an upper bound below the k-th best score τ_k. Otherwise that
item's score can be raised above τ_k inside the set consistent with what was read, changing the
answer without changing anything read. So its reads on non-answers are at least those of the
threshold query at τ_k. Reading order and early termination can save only the overshoot of the seed
threshold, measured at 5–22% in `knn/REPORT.md`. The threshold results at k = 10 below are
therefore the k-NN results up to that factor.

**What follows (argued).** In the family "sound intervals from each item's own stream plus shared
summaries", the proofs' Corollary 2 and `groups/REPORT.md` Proposition 1 say a shared summary of S
bits over M members is worth S/M bits per member, and reading it costs the query S bits. So shared
structure does not lower the bits read per decided member. Total work is at least the sum over
items of the bits a per-item code needs to bring its slack below that item's distance from the
threshold. By Proposition 4 that sum is linear in N at fixed k once the threshold has reached the
top of the score range. I have not turned this into a theorem: the floors are stated for mean
widths under a distribution, not item by item.

## 5. Measurements

### 5.1 Setup

- `prep_sub.py`: exact scores of all 469 queries against the 1M rows; nested random subsamples of
  31,250, 62,500, 125,000, 250,000, 500,000 and 999,900 rows. Each has its own spherical k-means
  with N/1000 clusters (the method of `groups/scripts/prep.py`; the largest reuses its level 0),
  its own support table with the cluster centres as anchors, and its own local PCA bases.
- rms residual by N: 0.507, 0.471, 0.441, 0.416, 0.394, 0.377 (N^(−0.086) overall, N^(−0.066)
  over the last doubling).
- Answer sizes k = 10, 100, 1,000, 10,000; the threshold is the k-th best exact score in the
  subsample.
- Exponents are least-squares slopes of log(mean over queries) on log N over the six sizes;
  intervals are 95% bootstrap over queries. "Last doubling" is the slope from 500k to 1M.
- Sample sizes: zero-read counts 369 text and 100 item queries; per-member read 123 text (every
  third) and 100 item; hashing 185 text and 50 item.
- All counts are of nested subsamples of one corpus sample over 1.5 decades of N. Section 5.7
  extrapolates three more decades and is not a measurement.

### 5.2 What is decided with nothing read per item (`zero.py`, `fit.txt`)

Share of items not decided, by N, and the exponent of the count. "Ball" cuts whole clusters by
their radius; "range" is the sorted annulus range on x·m̂; "table" adds the support table; "oracle"
counts items in clusters that hold both a match and a non-match, given each cluster's exact score
extremes; "oracle ±0.02" the same with the extremes known to ±0.02.

| | 31k | 125k | 500k | 1M | Exponent [95%] | Last doubling |
|---|---|---|---|---|---|---|
| **Text, k = 10**: threshold, sd above the median | 4.65 | 5.04 | 5.34 | 5.48 | | |
| Ball | 100% | 100% | 99.4% | 98.5% | 0.996 | 0.99 |
| Range | 97.1% | 87.0% | 79.3% | 75.3% | 0.929 [0.925, 0.934] | 0.93 |
| Range and table | 95.8% | 83.9% | 74.0% | 69.1% | 0.908 [0.901, 0.914] | 0.90 |
| Centres read under the table | 100% | 100% | 100% | 100% | 1.00 | 1.00 |
| Oracle | 6.3% | 2.0% | 0.64% | 0.30% | 0.115 [0.090, 0.138] | −0.09 |
| Oracle ±0.02 | 11.0% | 4.7% | 2.0% | 1.2% | 0.337 [0.309, 0.368] | 0.24 |
| **Text, k = 1,000** | | | | | | |
| Range | 100% | 97.6% | 90.5% | 86.6% | 0.957 [0.954, 0.960] | 0.94 |
| Range and table | 100% | 97.0% | 87.7% | 82.7% | 0.944 [0.940, 0.949] | 0.91 |
| Oracle | 54.2% | 19.6% | 5.7% | 3.3% | 0.170 [0.140, 0.199] | 0.22 |
| **Item, k = 10**: threshold, sd above the median | 4.86 | 5.20 | 5.48 | 5.59 | | |
| Ball | 100% | 96.6% | 83.2% | 73.8% | 0.914 [0.900, 0.929] | 0.83 |
| Range | 76.8% | 48.6% | 29.0% | 22.2% | 0.643 [0.613, 0.678] | 0.62 |
| Range and table | 54.5% | 26.6% | 12.7% | 9.3% | 0.488 [0.421, 0.556] | 0.56 |
| Centres read under the table | 81% | 66% | 53% | 47% | 0.848 [0.798, 0.889] | 0.84 |
| Oracle | 4.6% | 1.6% | 0.54% | 0.32% | 0.223 [0.175, 0.268] | 0.27 |
| **Item, k = 1,000** | | | | | | |
| Range | 98.5% | 80.7% | 55.8% | 44.8% | 0.770 [0.739, 0.800] | 0.68 |
| Range and table | 91.5% | 60.3% | 32.5% | 23.2% | 0.599 [0.537, 0.654] | 0.51 |

Violations: 0 of 1,848,439,684 interval checks.

- The first pass is sublinear for both query types, weakly for text. For text, k = 10, a
  thousandfold more data costs about 600 times more first-pass work.
- For text every centre is read at every N. The support table's anchor is never near a text query.
- The oracle count is nearly flat: at k = 10 the matches sit in 2–3 clusters of 1,000 at every N.
- The support table here has as many anchors as clusters at every N, which the review says cannot
  hold at 10⁹. With 16–64 anchors the item exponent would sit between the "range" and "table" rows.
- Keeping clusters at 1,000 items is worth about 0.04 of the text exponent (inferred): at the fixed
  1M clustering the range leaves 75.3% at k = 10 and 80.2% at k = 100, an exponent of 0.97 in k/N
  alone, against 0.93 with clusters that track N.

### 5.3 The per-member read (`ladder.py`, `lfit.txt`, `lfit_g.txt`)

The block ladder of `groups/scripts/margin.py`: exact local PCA coordinates in dyadic blocks (1, 1,
2, 4, …, 256, 256), the unread remainder bounded block by block, the item's own m·r and block norms
known at no charge. An item reads blocks until decided. Coordinates stand in for bytes. Two series:
the number of local bases growing with N as `prep.py` scales it (12, 12, 12, 16, 32, 64), and 12
bases at every N.

| | 31k | 125k | 500k | 1M | Exponent [95%] | Last doubling |
|---|---|---|---|---|---|---|
| **Text, k = 10, 12 bases**: coordinates per item | 74.4 | 66.8 | 55.1 | 47.3 | 0.873 [0.862, 0.882] | 0.78 |
| Undecided with 0 coordinates read | 86.0% | 79.6% | 71.5% | 67.4% | 0.930 [0.923, 0.936] | 0.91 |
| Undecided after 32 | 48.9% | 45.1% | 37.9% | 33.0% | 0.890 [0.874, 0.903] | 0.80 |
| Undecided after 128 | 10.6% | 9.1% | 7.1% | 5.6% | 0.824 [0.795, 0.849] | 0.66 |
| **Text, k = 10, bases growing**: coordinates per item | 74.4 | 66.8 | 46.3 | 35.9 | 0.793 [0.781, 0.805] | 0.63 |
| Undecided with 0 coordinates read | 86.0% | 79.6% | 68.8% | 61.8% | 0.906 [0.898, 0.914] | 0.85 |
| Undecided after 32 | 48.9% | 45.1% | 32.0% | 24.8% | 0.809 [0.787, 0.829] | 0.63 |
| Undecided after 128 | 10.6% | 9.1% | 5.4% | 3.8% | 0.708 [0.669, 0.741] | 0.47 |
| **Text, k = 1,000**: coordinates per item, 12 bases / growing | 190 | 139 | 99.9 / 87.8 | 83.5 / 67.0 | 0.766 / 0.703 | 0.74 / 0.61 |
| **Item, k = 10, 12 bases**: coordinates per item | 11.7 | 7.8 | 4.2 | 2.9 | 0.597 [0.566, 0.624] | 0.46 |
| Undecided with 0 coordinates read | 47.6% | 34.1% | 19.9% | 14.8% | 0.659 [0.622, 0.694] | 0.57 |
| Undecided after 32 | 7.0% | 4.4% | 2.2% | 1.4% | 0.557 [0.503, 0.594] | 0.35 |
| Undecided after 128 (items, not %) | 102 | 165 | 218 | 205 | 0.216 [0.126, 0.291] | −0.09 |
| **Item, k = 10, bases growing**: coordinates per item | 11.7 | 7.8 | 3.3 | 2.0 | 0.493 [0.461, 0.520] | 0.29 |
| **Item, k = 1,000**: coordinates per item, 12 bases / growing | 91.9 | 44.4 | 20.8 / 17.7 | 13.6 / 10.5 | 0.453 / 0.378 | 0.39 / 0.25 |

Violations: 0 of 5,268,107,400 (bases growing) and 0 of 4,682,732,400 (12 bases, the three
largest N) interval checks.

- **Coordinates read at fixed k grow as N^0.79–0.87 for text and N^0.49–0.60 for items.** Deeper
  levels have lower exponents, as Proposition 3 says: their slack is smaller, so their shells sit
  higher in the tail.
- Deep levels of item queries reach the constant regime: about 200 items need more than 128
  coordinates at every N.
- More local bases as N grows is worth 0.06–0.10 of exponent in this range. It cannot continue
  linearly to 10⁹ (64,000 bases, 150 GB as float32, and a projection of the query into each), so
  the 12-base series is the safer guide.
- Caveat: bases are fitted on the items they code. At 31k with 12 bases a group holds about 2,600
  items in 768 dimensions, so the small-N costs are flattered and the fitted exponents are biased
  upward (pessimistic). The last-doubling column, where groups hold 16,000–80,000 items, is less
  affected.
- At 1M the cost is spread over levels: the blocks between 16 and 256 coordinates carry about three
  quarters of the text coordinates. As N grows the deeper levels shrink faster and the first levels
  take a larger share.

### 5.4 The score tail and the intrinsic dimension (`tail.py`, `tail.txt`)

Threshold position and shell size at the full 1M, against the answer fraction p = k/N. Under
densification a fixed k at larger N is a smaller p, so reading a row left to right is reading N
upward at fixed certificates.

| Text queries, p = | 10⁻² | 10⁻³ | 10⁻⁴ | 10⁻⁵ | 10⁻⁶ |
|---|---|---|---|---|---|
| Threshold, sd above the median (a Gaussian tail gives) | 3.01 (2.33) | 4.25 (3.09) | 4.98 (3.72) | 5.48 (4.26) | 5.87 (4.75) |
| Items within 1 sd below the threshold | 5.3% | 1.2% | 0.41% | 0.18% | 0.12% |
| Within 2 sd | 18.5% | 5.2% | 2.3% | 1.1% | 0.68% |
| Within 3 sd | 48.2% | 16.0% | 8.0% | 4.5% | 2.7% |
| Within 4 sd | 84.0% | 40.3% | 22.5% | 14.2% | 9.4% |
| Within 6 sd | 99.9% | 94.4% | 80.0% | 65.3% | 53.0% |
| Local exponent of the 3 sd shell count in N | 0.47 | 0.69 | 0.74 | 0.79 | 0.77 |
| Local exponent of the 6 sd shell count in N | 0.99 | 0.93 | 0.91 | 0.92 | 0.90 |

- **Each standard deviation of slack removed from a certificate cuts the items it leaves by a factor
  of 3 to 4 and lowers its exponent by about 0.05.** Text, p near 10⁻⁵: 65% at 6 sd, 14% at 4 sd,
  4.5% at 3 sd, 1.1% at 2 sd; exponents at k = 10 of 0.91, 0.79, 0.74, 0.68. This is the lever on
  the first pass.
- The tail is heavier than Gaussian in position (5.5 sd where a Gaussian has 4.3) but its top is
  steep: the 10th best of 1M is 0.4 sd below the best. The local exponents rise as p falls (1 sd
  shell: 0.26 at 10⁻², 0.83 at 10⁻⁶), which is Proposition 4 arriving.
- Item queries are already there: their shells shrink only as N^0.92–0.95 at any width, and their
  sublinearity in 5.2 comes entirely from slack that shrinks as clusters tighten around the query.
- Score sd: 0.040 (text), 0.068 (item). Best score: 5.9 and 6.0 sd above the median.
- Intrinsic dimension, item queries: 21.6 / 20.2 / 15.7 by the Levina–Bickel estimator at
  k = 20 / 100 / 1,000; 20.1 from the 10th-neighbour distance, which falls as N^(−0.050); 19 from
  the cluster radius law of `groups/REPORT.md`. Text queries: 89 / 69 / 52, and 69 from the
  neighbour distance (N^(−0.015)).
- Expansion constant at the 10th / 100th / 1,000th neighbour: 4,050 / 2,958 / 998 (item, median);
  N/k (text).

### 5.5 The visible set (`zero.py`, `ladder.py`; the 1M clustering, table extremes over all items)

Random visible subsets, k = 10 inside the subset:

| Visible items | 977 | 15,625 | 62,500 | 250,000 | 999,900 | Exponent in the visible size |
|---|---|---|---|---|---|---|
| Clusters holding a visible item (centres read, text) | 548 | 999 | 999 | 999 | 999 | 0 above about 5,000 |
| Text: not decided by range | 94.7% | 85.4% | 81.5% | 78.1% | 75.3% | 0.97 |
| Text: coordinates per visible item | 115 | 61.8 | 50.3 | 42.1 | 35.9 | 0.836 [0.822, 0.847] |
| Text: oracle with extremes over all items (items) | 150 | 413 | 786 | 1,502 | 3,026 | 0.47 |
| Item: not decided by range and table | 46.7% | 20.6% | 14.9% | 11.8% | 9.3% | 0.81 |
| Item: coordinates per visible item | 36.0 | 8.6 | 4.9 | 3.2 | 2.0 | 0.593 [0.560, 0.623] |

- Work per visible item rises as the set thins, because the k-th best visible score is lower.
  Total work is sublinear in |V| only in the same weak sense as in N (0.84 for text, 0.59 for items,
  in coordinates), on top of K centres once |V| exceeds a few K.
- Visible sets that are contiguous in id order (taxonomic slices): the centres read fall to the
  clusters the slice meets (10, 44, 144, 375 for 977, 15,625, 62,500, 250,000 visible items), and
  coordinates read grow as |V|^0.67 (text, k = 10).
- An index built on the visible set alone is not better for a random set. At 31,250 visible, its
  own clustering (31 clusters) leaves 97.1% undecided by range and reads 74.4 coordinates per item;
  the shared 1M clustering (999 centres) leaves 83.3% and reads 55.6. Counting a centre as 768
  coordinates, the totals are 2.35M and 2.51M.
- Cluster extremes computed over all items remain useful to a sparse viewer: 150–3,000 items left
  for text at k = 10.
- Below roughly 10⁴–10⁵ visible items the review's advice stands: scan the visible set.

### 5.6 Candidates that did not change the exponent

**Hierarchy over centres** (`tree.py`; the four nested levels of `groups/scripts/prep.py`, fan-out
about 4, 58,750 leaves; a node's children are read only if its ball bound leaves it undecided;
violations 0 of 74,690,126):

| k = 10 | Centres read, share of the 58,750 leaves | Nodes undecided by level | Items under undecided leaves |
|---|---|---|---|
| Text | 129% | 99.3 / 97.5 / 92.0 / 75.5% | 82.3% |
| Item | 66.5% | 80.3 / 62.5 / 40.3 / 19.1% | 22.5% |

For text a hierarchy reads more centres than a flat list. For items it saves a third at 17 items
per leaf and less at 1,000. The cut is the ball bound, and the radius barely depends on the level.

**Hashing, idealised** (`lsf.py`).
- Cap filters: B Gaussian directions; an item is stored under those with projection at least a, a
  query probes those with projection at least b. For a pair with cosine c the two events are an
  orthant of a bivariate normal with correlation c in any dimension, so the expected work follows
  from the exact scores.
- B is set so that each match is found with probability 1 − 10⁻⁹/k; (a, b) are chosen per query to
  minimise probes plus distinct candidates under a space budget.
- Each item's inclusion depends only on its own vector, the query and the seed, so the family meets
  the requirements. Candidates would still need certifying.
- This is an analytic expectation, optimistic for the family (per-query tuning, free decoding), not
  a run, so there are no bounds to violate.

| k = 10: probes + candidates, share of N | 31k | 1M | Exponent |
|---|---|---|---|
| Text, 256 table entries per item | 90% | 61% | 0.89 |
| Text, 4,096 entries per item | 66% | 34% | 0.81 |
| Text, corpus mean removed, 4,096 entries | 88% | 49% | 0.83 |
| Item, 256 entries | 24% | 3.5% | 0.45 |
| Item, 4,096 entries | 10% | 1.6% | 0.46 |

With 16 entries per item no query gains over a scan. At 4 bytes an entry, 4,096 entries is 16 KB
per item. The item optimum sits at a ≈ b ≈ 10–11, which means of the order of 10²⁵ filters: a bound
on the family, not a design. For text the family matches the theory of 3.4 and does no better than
the design's own first pass, at far greater storage.

**Already ruled out, and consistent with what is measured here**: finer clusters
(radius ∝ K^(−0.053)), anchors for fresh text queries, group summaries (`groups/REPORT.md`), trees
over items.

**Amortisation that changes a constant only (argued).** A batched scan shares memory traffic among
B concurrent queries; the arithmetic per query and item stays. A result cache removes repeat
executions and leaves a miss unchanged.

### 5.7 Extrapolation to 10⁹ (conjectured; `extrap.py`, `extrap.txt`)

Two readings: the fitted exponent held, and the last doubling's exponent held. Text, k = 10:

| | At 1M | At 10⁹ |
|---|---|---|
| Not decided by range | 75% | 45–46% |
| Not decided by range and table | 69% | 35–37% |
| Undecided with 0 coordinates read (12 bases) | 67% | 37–41% |
| Undecided after 32 coordinates (12 bases) | 33% | 8–15% |
| Coordinates per item (12 bases) | 47 | 10–20 |
| Coordinates per item (bases growing with N; not affordable at 10⁹) | 36 | 3–9 |
| Oracle (items) | 3,000 | 2,000–7,000 |

Item, k = 10: range 22% → 1.6–1.9%; range and table 9.3% → 0.3–0.45%; coordinates per item
2.9 → 0.07–0.18.

So for text a thousand times more data costs roughly 200–400 times more per-member work, and the
first pass still touches a third to a half of the items. The threshold at k/N = 10⁻⁸ is beyond what
1M rows can show; if the tail has already ended (5.4 suggests it is close), the true figures are
nearer the upper ends.

## 6. Candidates, with exponent and constant

| Candidate | Status | Exponent in N at k = 10 (text / item) | Constant at 1M | Changes |
|---|---|---|---|---|
| Design as it stands, first pass (range, table) | measured | 0.91–0.93 / 0.49–0.64 | 69–75% / 9–22% of items | — |
| Design, per-member read | measured | 0.79–0.87 / 0.49–0.60 | 36–47 / 2–3 coordinates per item | — |
| Clusters at 1,000 items as N grows (K ∝ N) | measured, share inferred | part of the above; radius ∝ N^(−0.07 to −0.09) | | exponent, by about 0.04 for text |
| More local bases as N grows | measured | −0.06 to −0.10 | −24% (text), −31% (item) for 12 → 64 bases at 1M | exponent, while memory lasts |
| Tighter first certificate (the M-norm key of `groups/REPORT.md`, finer first levels) | measured as shells; the key itself not re-run here | about −0.05 per sd of slack removed | 3–4× fewer items per sd | both |
| Cached exact cluster extremes for a repeated query | measured (oracle) | 0.12 / 0.22 | 0.3% of items | exponent, on hits only |
| Cached extremes to ±0.02 | measured | 0.34 / 0.49 | 1.2% / 0.9% | exponent, on hits only |
| Result cache | argued | 0 on a hit | — | hits only |
| Hierarchy over centres | measured | none | 129% / 67% of leaf centres | worse for text |
| Hashing (cap filters), idealised | measured as an expectation | 0.81–0.93 / 0.45–0.70 | 34–79% / 1.6–26% of items, at 1–16 KB per item | not worth its storage |
| Finer clusters, anchors, group summaries | earlier reports | none | — | constants, negative net |
| Batched scans | argued | none | memory traffic ÷ batch | constant |
| k-NN reading order | proved bound, measured gap | none | 5–22% above the oracle threshold | constant |
| Per-viewer index | measured | none in the visible size | fewer centres, more items | constant |

## 7. Dimension (100 to 5,000)

- **The law of section 4 does not mention dimension.** It acts through two numbers: each level's
  slack in units of the query's score sd, and the tail shape in the same units.
- **Slack in sd units grows with the effective dimension** (argued). With nothing read, the bound on
  Δ·r is ‖Δ‖·‖r‖ and its typical value is that divided by √(n_eff), so the zero-read slack is about
  √(n_eff) within-cluster sd whatever the model. More dimensions put the first pass further into
  the bulk and move its exponent toward one. A randomised level at a fixed number of bits per
  dimension keeps a fixed slack in sd units, so per-item bytes scale with n and the share of the
  vector read stays roughly constant.
- **Tail room shrinks with fewer dimensions** (measured on a proxy: `dims.py`, `tail_32.txt`,
  `tail_96.txt`, `tail_256.txt`). The corpus and queries are projected on the top d principal
  directions and renormalised. This is a truncation of one model, not another model.

  | d | Text: threshold at p = 10⁻⁵, sd above median | Text: within 3 sd of it | Text: within 6 sd | Item: local dimension (k = 20) | Item: expansion constant |
  |---|---|---|---|---|---|
  | 32 | 3.76 | 25.1% | 99.6% | 15.2 | 962 |
  | 96 | 4.98 | 6.3% | 85.2% | 18.6 | 1,857 |
  | 256 | 5.42 | 4.6% | 68.3% | 21.3 | 3,596 |
  | 768 | 5.48 | 4.5% | 65.3% | 21.6 | 4,050 |

  Low dimension leaves fewer standard deviations between the bulk and the best matches, so a
  certificate of a given slack in sd units decides less. This is consistent with GloVe-100 touching
  96% of items in `knn/REPORT.md`. The items' intrinsic dimension is 15–22 whatever the ambient
  dimension, so the low-dimension guarantees of 3.4 stay vacuous across the range.
- **The conditional lower bound** needs only dimension ω(log N), which every dimension in the range
  satisfies. The hashing exponent 1 − τ² depends on the matches' cosine against the background, not
  on n.
- Not done: a second embedding model, or a native large dimension. `agent3b/` has n = 1,024
  item-query figures at fixed fractions (range and table: 5–13% of items).

## 8. Timing channels

Results are unaffected by any of this; work is not.

- **Cluster extremes for a query, computed over all items**, make a viewer's work depend on how
  hidden items score for that query. The support table has the same property already (review,
  1.8). Extremes kept per grant avoid it and cost one row per grant per cached query.
- **A cache hit** tells a viewer that the query was run before. Keying by grant set removes this and
  most of the sharing.
- **Batched scans** make latency depend on what else is running.
- **Clusters and local bases that track N** add nothing new: they are functions of the whole corpus
  already.
- Hashing tables would leak nothing through results, but bucket sizes depend on hidden items.

## 9. Recommendation, ranked

1. **State the envelope instead of chasing an exponent for fresh queries.** No method gives a
   sublinear guarantee, and the measured exponents are 0.8–0.93 for text. A fresh text query at 10⁹
   still reads a third to a half of the items at level 0 and of the order of 10–20 coordinates per
   item. Size the system for that, or for a visible set of 10⁷–10⁸.
2. **Cache each executed query's cluster extremes with its result.** This is the only measured route
   to an exponent far below one (0.12–0.34). It serves every threshold and every viewer through
   their own mask, and a row is two bytes per cluster (2 MB per cached query at 10⁹). For hot
   queries compute exact extremes in the background with one flat scan; for the rest keep what the
   first execution proved, which helps only thresholds at or above the first one. Record the timing
   channel, or keep rows per grant. Its value depends on how often query directions repeat, which
   nothing here measures.
3. **Spend effort on the first certificate.** It is the level every item meets and the one whose
   share grows with N. Each sd of slack removed leaves 3–4 times fewer items and lowers that level's
   exponent by about 0.05. The M-norm sort key of `groups/REPORT.md` and a first level finer than
   three bytes are the candidates; both belong with the per-item work.
4. **Keep clusters near 1,000 items as N grows, and let the number of local bases grow as far as
   memory allows.** Both are part of the measured exponents; holding K or the bases fixed gives up
   0.04–0.10 of exponent.
5. **Plan by visible set**: scan it below about 10⁴–10⁵ items. Above that the shared clustering is
   as good as a per-viewer one for random sets and cheaper in centres for clustered ones.
6. **Do not build**: a hierarchy over centres for skipping (keep it for insert assignment), hashing
   tables, finer clusters, more anchors.

## 10. What did not work, and limits of this report

- No idea tested made a fresh text query's work grow slower than about N^0.8 in coordinates or
  N^0.9 in items touched.
- The asymptotic statement (Proposition 4) is a proof about fixed slack. The claim that shared
  summaries cannot rescue it rests on the earlier reports' propositions and is argued, not proved
  here.
- Exponents come from 1.5 decades of nested subsamples of one sample. The 10⁹ figures are
  extrapolations.
- The sample is 20 dense taxonomic slices, so "densifying" is the growth model tested. A corpus
  that grows by adding unrelated content was not tested.
- Per-member costs are idealised exact coordinates in bases fitted on the coded items; they are not
  bytes.
- The support table has anchors equal in number to clusters at every N.
- The M-norm key was not re-run across N; its exponent is inferred from the shell table.
- The hashing figures are expectations under per-query tuning. A grid limit of 12 on the two caps
  binds for item queries; a first run with a limit of 6 is in `lsf_grid6.log`.
- Thresholds come from scores computed in one pass and the per-member run recomputes scores per
  chunk; the two differ by float32 rounding, of the order of 10⁻⁷.
- No query log, so nothing on how often a cache would hit.
- The dimension evidence is a PCA truncation of one model; no second model was run.

## 11. Files

- `common.py` (chunked reads, timing lock), `prep_sub.py` (scores `S.f32`, subsamples `rank.npy`,
  clusterings `cl*.npz`, bases `V*.npy`; none of these are committed, 2.3 GB in all)
- `zero.py` → `fit.txt` via `fit.py` (`zero_all.json`, 17 MB, not committed)
- `ladder.py` → `ladder_*.json` (`ladder_5.json` not committed); `prep_g.py` and
  `GFIX=1 ladder.py` → `ladder_g_*.json`; `lfit.py` → `lfit.txt`, `lfit_g.txt`
- `tail.py` → `tail.txt`, `tail_32.txt`, `tail_96.txt`, `tail_256.txt`; `dims.py` (its score files
  are regenerated in 20 s)
- `tree.py` → `tree.log`, `tree.json`
- `lsf.py` → `lsf.log`, `lsf_grid6.log`, `lsf.json`
- `extrap.py` → `extrap.txt`

Logs sit beside each. One thread throughout, under 1 GB resident.

## 12. References, and how each was checked

1. M. Pătraşcu, M. Thorup. Higher Lower Bounds for Near-Neighbor and Further Rich Problems. FOCS
   2006; version of December 2008. https://people.csail.mit.edu/mip/papers/higher/higher.pdf — PDF
   text read: abstract, introduction, and the list of results (Ω(d / lg(Sd/n)) for randomised
   partial match and exact near neighbour).
2. A. Borodin, R. Ostrovsky, Y. Rabani. Lower Bounds for High Dimensional Nearest Neighbor Search
   and Related Problems. STOC 1999 — as described in [1].
3. O. Barkol, Y. Rabani. Tighter Lower Bounds for Nearest Neighbor Search and Related Problems in
   the Cell Probe Model. STOC 2000 — as described in [1].
4. T. S. Jayram, S. Khot, R. Kumar, Y. Rabani. Cell-Probe Lower Bounds for the Partial Match
   Problem. STOC 2003 — as described in [1].
5. R. Williams. A New Algorithm for Optimal 2-Constraint Satisfaction and Its Implications.
   Theoretical Computer Science 348(2–3):357–365, 2005 — bibliographic record and secondary
   descriptions; the Orthogonal Vectors consequence of SETH is attributed to it by secondary
   sources, not read in the paper.
6. A. Abboud, A. Rubinstein, R. Williams. Distributed PCP Theorems for Hardness of Approximation in
   P. FOCS 2017. https://arxiv.org/abs/1706.06407 — arXiv listing and search summary.
7. A. Rubinstein. Hardness of Approximate Nearest Neighbor Search. STOC 2018, 1260–1268.
   https://arxiv.org/abs/1803.00904 — abstract read.
8. L. Chen. On the Hardness of Approximate and Exact (Bichromatic) Maximum Inner Product.
   https://arxiv.org/abs/1802.02325 — abstract as paraphrased by the fetch tool; venue (CCC 2018)
   from memory.
9. J. Alman, R. Williams. Probabilistic Polynomials and Hamming Nearest Neighbors. FOCS 2015,
   136–150. https://arxiv.org/abs/1507.05106 — search summary of Theorem 1.3.
10. A. Andoni, T. Laarhoven, I. Razenshteyn, E. Waingarten. Optimal Hashing-based Time–Space
    Trade-offs for Approximate Near Neighbors. SODA 2017. https://arxiv.org/abs/1608.03580 —
    abstract read (the trade-off formula, tightness in a restricted model, cell-probe bounds for one
    and two probes).
11. A. Andoni, T. Laarhoven, I. Razenshteyn, E. Waingarten. Lower Bounds on Time–Space Trade-offs
    for Approximate Near Neighbors. https://arxiv.org/abs/1605.02701 — search summary (the
    list-of-points model, random instances).
12. V. Pestov. Lower Bounds on Performance of Metric Tree Indexing Schemes for Exact Similarity
    Search in High Dimensions. Algorithmica 66:310–328, 2013. https://arxiv.org/abs/0812.0146 —
    abstract, partly paraphrased. Also V. Pestov, Indexability, Concentration, and VC Theory, J.
    Discrete Algorithms 13:2–18, 2012, https://arxiv.org/abs/1008.5105 — abstract read.
13. E. Chávez, G. Navarro, R. Baeza-Yates, J. L. Marroquín. Searching in Metric Spaces. ACM
    Computing Surveys 33(3):273–321, 2001.
    https://users.dcc.uchile.cl/~gnavarro/ps/acmcs01.2.pdf — PDF text read (Definition 9 and
    section 7.2).
14. R. Weber, H.-J. Schek, S. Blott. VLDB 1998 — entry 1 of `lit/REPORT.md`, read there.
15. A. Beygelzimer, S. Kakade, J. Langford. Cover Trees for Nearest Neighbor. ICML 2006.
    https://hunch.net/~jl/projects/cover_tree/paper/paper.pdf — PDF text read (abstract,
    introduction).
16. R. Krauthgamer, J. R. Lee. Navigating Nets: Simple Algorithms for Proximity Search. SODA 2004
    — as described in [15]; the paper itself could not be fetched.
17. D. Karger, M. Ruhl. Finding Nearest Neighbors in Growth-Restricted Metrics. STOC 2002 — as
    described in [15].
18. J. Matoušek. Reporting Points in Halfspaces. Computational Geometry 2(3):169–186, 1992 —
    bibliographic record; the bound as used by a later paper (arXiv:1312.6305), not read in the
    original.
19. T. M. Chan. Optimal Partition Trees. Discrete & Computational Geometry 47:661–690, 2012 —
    first-page summary, including the attribution of O(N) space and O(N^(1−1/d)) time to Matoušek
    (1993).
20. P. Afshani, P. Cheng. An Optimal Lower Bound for Simplex Range Reporting.
    https://arxiv.org/abs/2210.14736 — abstract and section 1.2 via ar5iv
    (Q = Ω((N²/S)^((d−1)/d) + k); improves the halfspace reporting bound in dimension 9 and up; the
    halfspace formula is not stated there). P. Afshani, SoCG 2012, 339–346, as cited there.

Mentioned for context and not checked: Valiant (FOCS 2012) and Karppa, Kaski, Kohonen (SODA 2016)
on subquadratic detection of outlier correlations; Becker, Ducas, Gama, Laarhoven (SODA 2016) on
cap filters; Andoni and Razenshteyn (STOC 2015) on data-dependent hashing.
