# Layers over and under the clusters: what scales, and what to build

2026-11-20. Research and design; nothing in the library's behaviour changes. Two test-only studies were added to
`rust/src/v2/e2e.rs` (`study_layered_dump`, `study_layered_kernel`; that file is outside the library's fingerprint).
Scripts and results are in `layered/`. Everything here is a **count** (rows, clusters, bytes read, centres scored);
nothing was timed. Status labels as in `theory.md`: **[measured]** counted on real data, **[derived]** follows from
the build's own quantities or from a stated model, **[estimated]** a count times a constant taken from elsewhere.

Data: the suite's three datasets and their real fits (glove-100-angular, 1.18M rows, K 592; yahoo-minilm-384, 0.68M,
K 339; laion-clip-512 with text queries, 1.0M, K 500, out of distribution), and one held-out L2 dataset,
sift-128-euclidean (1M rows; scores q.x − |x|²/2). Replay A uses 1,000 queries per dataset, replay B the suite's 100.

## 0. Summary

**The question was how to get below √N per query, for 100M to 1B rows and beyond. The measurements say the read is
not √N today (it is worse), that no layer of clusters makes it polylogarithmic, and that the design for scale is
four rules, each derived from N and the index and each off or free at 1M.**

1. **The law of the read [measured, four datasets, 62k to 1.2M rows].** To a fixed recall the order reads a share of
   the corpus that depends on the number of clusters K and hardly on N: rows ≈ N·φ(K). Fitted, rows ∝ S^γ·N^η with
   γ = 0.4 to 0.6 (in the cluster size) and η = 0.3 to 0.5 (in N at a fixed cluster size) at recall 0.95; at 0.99,
   γ = 0.3 to 0.5 and η = 0.5 to 0.7. Under the build rule (S ∝ √N) the rows read grow as **N^(0.6 to 0.72)** (0.71 to
   0.85 at recall 0.99), not N^0.5: the clusters read are not constant, they grow as K^(0.3 to 0.6). A random visible
   set behaves as a subsample: the same share of the visible rows, in the same share of the clusters. With leaves
   of a fixed size the rows grow as **N^η**, which is the floor for any hierarchy keyed by centres and spreads on
   these data. No N^(1/3), no polylog.
2. **Rule 1, at every N: one rule at every level, and no new structure** (sections 4, 5). A unit (a cluster, then a
   row at a level) is read further when its optimistic score, estimate + ψ·spread, reaches the running k-th best.
   For clusters this is today's key. For a row at a deterministic level the estimate is the midpoint of its
   certified interval and the spread is ρ times its certified half-width, ρ = ‖a‖₂/‖a‖₁ ≈ 1/√d **derived from the
   build's own weights** (within 2 to 5% of the measured value in distribution, 15% on the safe side for LAION's text
   queries). Exact queries are the same recursion with certified widths and are untouched: the kernel already takes
   a factor per level (`Prep2::dial`).
   **Evidence [measured, real codes; replay and the library's own reader].** Unfiltered, k = 10, bytes read per
   query to recall 0.9 / 0.95: glove 29 / 47 → 2.9 / 4.6 MB, yahoo 10 / 23 → 1.4 / 2.5, LAION text 47 / 79 → 7.4 /
   10.8 (6 to 10 times; the reader study gives 5 to 10 times up to recall 0.995). Stored vectors read fall 40 to
   450 times. 25% visible sets: 2.3 to 7.4 times. Random 1% sets: unchanged by construction. Region 1%: 1.6 to 2.6.
3. **Rule 2, from 8M rows: leaves.** A cluster of more than 2,000 rows is stored as 2,000-row runs, each with the
   centre and directions a 1M cluster has; the approximate path expands clusters by the same rule and reads leaves,
   the exact path keeps its large clusters. It is the only candidate that changes the exponent (N^(γ/2+η) → N^η).
   Model: a further 1.9 to 2.9 times at 100M and 3.3 to 5.5 at 1B; measured in that shape at 1M (leaves in parents
   of 8 and 32): 4 to 9 times fewer rows than reading parents whole. Off at 1M by the rule. It needs the exact
   queries timed on such a build before adoption; `subclusters.md`'s 5 to 17% was with 125-row runs.
4. **Rule 3, from K = 2,000 (16M rows): a sketch of the centres** (32 int8 coordinates each, then the same rule):
   0.15 to 0.26 of the centre products on yahoo, LAION and sift, 0.6 on glove; better than a tree of parents, and
   than the groups used as that tree. The centre scan is the largest term at 100M to 1B only under sparse sets.
5. **Rule 4, the number of groups** (8.4). The balance of a group's setup (n²) against what its tighter basis saves
   gives G* ∝ N/n² (rows per group ∝ n²; the clusters read cancel), with a factor the build can compute from the fit.
   Proposed: G = max(64, min(N/(3n²), K/8)): 64 at 1M (unchanged), 127 to 781 at 100M, 1,272 to 1,953 at 1B. By a proxy
   each doubling of G is worth 8% (yahoo) to 12% (LAION) of the 16 B code and nothing on glove; at 1B today's cap of 64 pools 244 clusters a basis.
   This is the least supported number here.
6. **Rule 5, visible sets at scale**: counts per group to drop empty regions for G comparisons; a row-ordered
   Roaring bitmap beyond 2^27 rows (a 1B cluster is about one container). By the subsample law the sparse regime
   recedes: 1% visible is 20 rows a cluster at 1M and 640 at 1B, where rule 1 engages.
7. **At 1B [model, section 8]**: today 0.9 to 5 GB read per query unfiltered at recall 0.95; rule 1: 56 to 362 MB;
   all rules: 10 to 109 MB. Codes dominate unfiltered and at 25%; the centre scan under 1% sets until rule 3.
8. **Rejected**: the sorted key at a statistical width (it explains 4 to 20% of a cluster's variance: costs recall,
   saves nothing); certified shared-direction ("Bloom") bounds (the remainder term prunes nothing; the support
   table is their best form); sign codes of centres; a graph over centres; a second assignment; a 4 or 8 B first
   level (no gain on yahoo at 1M).
9. **Risks.** Bytes and rows, not time: the existing level kernel costs about what a direct read of a
   100-dimensional row costs, so at 1M glove needs a leaner first-level pass and yahoo and LAION may gain up to 2
   times; at 100M and beyond direct reads are bound by memory bandwidth and the byte ratio is the time ratio. The
   exponents are fitted on a 16-fold range of N and carried 1,000-fold. Recall at a given ψ falls 0 to 6 points at
   ψ = 4 to 5 (LAION text most), so the default ψ moves up by about half a unit.

## 1. What a query reads, in bytes and operations

| read | bytes | pattern |
|---|---|---|
| a centre, int8 | n | all K, in order, every query |
| a cluster's sigma (16 int8 directions) | 16 n | each cluster read and the 8 ahead of it |
| a group's setup (the int8 basis product) | n² | once per group a query opens on the index path; the randomised levels' rotations add about 1.2 n² (f32) and are not needed below |
| a cluster's tables | about 96 per 4-bit unit of the levels read | once per cluster opened |
| a row at the 16 B level | 18.5 (code, 2 B key, 4-bit norm code) | the cluster's range, in order, blocks of 32 |
| a row at the 32 B level | 16.5 more | survivors |
| a stored vector (f16) | 2 n | one per survivor, scattered |

n is the padded dimension (128, 384, 512). In AVX-512 operations a stored vector is n/16 multiply-adds (8, 24, 32)
and a row's 16 B level is 32 table look-ups, about one 64-lane byte shuffle and one add when 32 rows share them
(AVX2 doubles both) [estimated]. The existing kernel does more per row than that (integer thresholds per row and
norm code, survivor lists, re-packing); see section 10.

The approximate k-NN today reads most clusters directly at 1M (the measured cost model prices the index path above a
direct read except on LAION). Per query, unfiltered, to recall 0.95 at k = 10 [measured, replay B]: glove 47 MB
(181k vectors in 75 clusters), yahoo 23 MB (30k in 15), LAION 64 MB (61k in 29); the centres are 0.08, 0.13 and
0.26 MB of that and the sigmas 0.17 to 0.30 MB.

## 2. How the read scales with N [measured]

Replay A (`layered/scaling.py`; tables in `layered/results/scaling_tables.md`). For each dataset: nested random
subsamples of N, N/4 and N/16 rows; on each, k-means partitions at cluster sizes S from 62 to 16,000 (K from 62 to
4,700; faiss, 10 iterations, 128 training rows a centre); for 1,000 queries the exact mean μ_c and standard deviation
σ_c of every cluster's scores; clusters read in decreasing μ_c + 3σ_c, the same number for every query; rows at which
the mean recall reaches a target. The key uses each cluster's exact spread, so this is the order's own limit, not a
property of the library's 16 directions. Search inside a read cluster is exact.

Rows read to recall 0.95 at k = 10 (clusters read in brackets):

| rows | S = 250 | S = 1,000 | S = 4,000 | S = 16,000 |
|---|---|---|---|---|
| glove, N = 1,183,514 | 51,054 (169) | 98,086 (82) | 173,545 (37) | 278,369 (16) |
| glove, N/4 = 295,878 | 27,984 (95) | 50,653 (44) | 78,118 (17) | - |
| glove, N/16 = 73,969 | 13,130 (46) | 22,598 (20) | - | - |
| yahoo, N = 677,305 | 7,543 (30) | 18,235 (18) | 41,637 (10) | 96,938 (6) |
| yahoo, N/4 = 169,326 | 5,136 (20) | 12,262 (12) | 28,033 (7) | - |
| yahoo, N/16 = 42,331 | 3,655 (15) | 8,010 (8) | - | - |
| LAION text, N = 1,000,448 | 18,052 (73) | 38,978 (37) | 78,395 (19) | 175,216 (10) |
| LAION text, N/4 = 250,112 | 11,024 (44) | 24,207 (21) | 41,109 (9) | - |
| LAION text, N/16 = 62,528 | 6,423 (24) | 13,621 (12) | - | - |
| sift (L2), N = 1,000,000 | 13,278 (49) | 23,193 (22) | 45,632 (11) | 94,975 (6) |
| sift (L2), N/4 = 250,000 | 7,552 (28) | 14,232 (14) | 25,868 (6) | - |
| sift (L2), N/16 = 62,500 | 3,884 (15) | 7,003 (7) | - | - |

(The smallest subsample was also run at S = 62: 6,826, 1,330, 2,735 and 2,262 rows for glove, yahoo, LAION, sift.)

Least squares of log rows on log S and log N over the ten partitions of each dataset, rows ∝ S^γ·N^η (the fit is
within a factor 1.04 to 1.3 of every point):

| k, recall | glove γ, η | yahoo γ, η | LAION text γ, η | sift (L2) γ, η | rows ∝ N^(γ/2+η) under S ∝ √N |
|---|---|---|---|---|---|
| 10, 0.9 | 0.47, 0.41 | 0.69, 0.20 | 0.62, 0.30 | 0.50, 0.40 | 0.55 to 0.64 |
| 10, 0.95 | 0.41, 0.52 | 0.62, 0.29 | 0.54, 0.39 | 0.45, 0.44 | 0.60 to 0.72 |
| 10, 0.99 | 0.29, 0.70 | 0.47, 0.50 | 0.40, 0.60 | 0.39, 0.52 | 0.71 to 0.85 |
| 100, 0.9 | 0.34, 0.45 | 0.55, 0.22 | 0.48, 0.29 | 0.40, 0.45 | 0.49 to 0.64 |
| 100, 0.95 | 0.28, 0.57 | 0.50, 0.30 | 0.42, 0.40 | 0.37, 0.49 | 0.55 to 0.71 |
| 100, 0.99 | 0.20, 0.73 | 0.37, 0.50 | 0.30, 0.61 | 0.31, 0.57 | 0.69 to 0.83 |

- **The share of the corpus read depends on K, not on N.** At a fixed number of clusters the rows read are a nearly
  fixed share of the rows: glove at K = 296 reads 17.7%, 17.1% and 14.7% of 74k, 296k and 1.18M rows to recall 0.95;
  sift at K = 250 reads 6.2%, 5.7% and 4.6%. Each fourfold K divides the share by 1.8 to 2.4 (φ(K) ∝ K^(−γ)). So
  γ + η is 0.9 to 1: the clusters read are m ≈ K·φ(K) ∝ K^(0.3 to 0.6), not a constant.
- **Under the build rule the read grows faster than √N**: K ∝ √N gives rows ∝ N^(γ/2+η), 0.55 to 0.72 at recall 0.9
  to 0.95 and 0.71 to 0.85 at 0.99. The premise "m clusters of √N rows" understates the read at scale: by this
  law a glove-like corpus of 1B rows in 15,625 clusters reads about 2% of its rows to recall 0.95.
- **Leaves of a fixed size would give rows ∝ N^η** (η = 0.2 to 0.5 at recall 0.9 to 0.95, 0.5 to 0.7 at 0.99), and
  the centres scored on the way grow at least as fast as the leaves read. That is the best exponent a hierarchy of
  clusters keyed by centre and spread reaches on these data; it is a property of the data (how slowly a finer
  partition concentrates the neighbours), not of the build rule. Earlier counts agree (`rust/REPORT_SCALING.md`:
  exponent 0.03 to 0.44 in N at a fixed sub-cluster size, from two points).
- **A random visible set is a subsample.** The full set's partition read through a random 1/16 of its rows needs the
  rows and clusters of a partition made of 1/16 of the rows with clusters of the same visible size (glove, 296
  clusters of 250 visible rows: 13,499 rows in 47.9 clusters against 13,130 in 46.4; sift, 250 clusters: 3,859 in
  14.7 against 3,884 in 14.9). At 1% visible the query still reads φ(K) of the visible rows, spread over the same
  share of the clusters (glove K = 296: 2,244 of 11,835 visible rows in 52 clusters). This is why per-cluster
  overhead and the centre scan dominate under sparse sets.
- The spread term is worth 1.2 to 1.9 times in rows over the centre score alone at every size and N (last column of
  the tables), as `../theory.md` and Bruch et al. found; nothing here changes the order.

What this means for the design: (i) the read, not the centre scan, is what grows; (ii) the lever with the largest
reach is the cost per row read, since every row of a read cluster is touched today; (iii) more, smaller leaves cut
the rows, but only as K^(−γ), and section 7 weighs what they cost.

## 3. The candidates

| | candidate | per query | build / state | verdict |
|---|---|---|---|---|
| over | a tree or beam over the centres (k-means levels, statistical node key) | √K node keys + the children of the P best | a level of centres and their covariance sketches | **no; the sketch instead.** At K ≤ 600 nothing to win (`replay.md`). At K ≈ 1,000 to 4,700 [measured here, parents keyed as one cluster]: sift holds the rows within 1% scoring 29% of the centres; glove needs 52% of them for +3% / +12% rows at 0.9 / 0.95. A factor of 2 to 3 on the centre products, less than the sketch on three datasets of four (section 6) |
| over | a sketch of the centres: their first r coordinates in a shared basis, the rest scored for the survivors of the same rule | K·r/n centre products + the survivors | r·n + K·r bytes | **rule 3, from K = 2,000** (8.3). Same role as the tree and simpler (no partition, visible counts apply per cluster). 0.26 of the centre products on sift at r = 16, 0.6 to 0.7 on glove [measured]; 0.15 to 0.17 on yahoo and 0.23 to 0.27 on LAION text at r = 32 to 64. Section 6 |
| over | certified bounds against a few shared directions ("Bloom" style): per cluster the range of u_j·x for shared u_j | j projections + K·j comparisons | K·j intervals | **rejected on paper.** q·x ≤ Σ_j max(a_j lo_j, a_j hi_j) + ‖q_rest‖·R: with 16 shared directions in a few hundred dimensions ‖q_rest‖ ≥ 0.85‖q‖ and the remainder alone exceeds any k-th best. The support table is this structure with one direction chosen per query among 1,024 (the nearest anchor), which is its best form, and it stays |
| over | sign or hyperplane codes of the centres | K·b bits | K·b bits | **rejected on paper.** b sign bits estimate a cosine to about 1/√b; centre scores of the clusters worth reading differ by 0.01 to 0.05, so thousands of bits a centre, more than its int8 coordinates. An int8 sketch coordinate gives an exact partial sum for the same byte |
| over | a graph over the centres | ef × degree centre products, random access | K × degree links | **rejected.** The same factor as the tree; the key's spread term is not a metric the graph can be built on; under a visible set most nodes are dead and must still be walked; of no use to the exact queries |
| over | the groups as the level | 64 + their clusters | none | **rejected as the level** (`theory.md`: 24 to 41% of the groups hold the best 1% of clusters; rechecked with G scaled in 8.3: the tree column); kept for emptiness under region sets (8.5) |
| over | a second assignment of rows (spilling) | fewer clusters | rows stored twice | **rejected**: it breaks the exact layout (a row in two clusters) |
| under | **the level codes at a statistical width** | first-level bytes for the rows of a read cluster, stored vectors for 0.2 to 3% of them | one number per group and level, derived at build | **recommended** (sections 4, 5) |
| under | a finer first level (4 or 8 B) under the same rule | 4.5 to 8.5 B a row in place of 18.5 | the fit's six-level schedule | **not now [measured, yahoo]**: 2.48 to 2.63 MB against 2.54 at recall 0.95. A 4 or 8 B level explains too little, so the query needs 2 to 3 more clusters, and at 1M the codes are a quarter of the bytes. To re-measure where codes dominate (glove; large N) |
| under | the sorted key at a statistical width | a binary search; the rows of the range | two numbers per cluster | **rejected [measured]**: the key explains 20%, 6% and 4% of a cluster's score variance (glove, yahoo, LAION text); it drops answer rows the levels would have kept and saves no first-level rows at equal recall |
| under | sub-clusters, rows contiguous (layout (c) of `subclusters.md`) | sub-centres of the clusters read + the rows of the sub-clusters kept | rows reordered; n bytes per sub-cluster | **rule 2, from 8M rows, with 2,000-row leaves** (8.2). The only candidate that changes the exponent (rows ∝ N^η). It costs the exact queries 5 to 17% as built, as built at 1M with 125-row leaves. **Section 7 makes it the rule from 8M rows on, with leaves of 2,000 rows** |
| under | sub-clusters as labels (masked) | a label pass over the cluster + the rows kept | 1 B a row + sub-centres | **rejected**: per row it costs what a 4 B first level costs, and needs k-means per cluster and a gather |
| under | key ranges as sub-clusters | | none | **rejected** (`subclusters.md`: not coherent in space) |
| non-tree | product codes, or the index's own level code, over the centres | 16 B a centre | a code per centre | the principled form of the sketch (the same rule one level up); to build only if the sketch's numbers at a real K justify it |

## 4. The recommended design: one rule at every level

### 4.1 The rule

The index already holds, for every row of a cluster, a chain of ever finer descriptions: the cluster's centre and
spread, the row's key, its code at each level, its stored vector. Each description gives an estimate a of the row's
score and leaves a residual. The approximate k-NN reads a unit further when

    a + ψ·τ ≥ t,                                                                                  (4.1)

a the unit's estimate, τ the spread of what the estimate leaves out, t the running k-th best.

- **Cluster**: a = μ_c, τ = σ_c(q) from the 16 stored directions. This is today's key and stop; nothing changes.
- **Row at a deterministic level l**: a = the midpoint of the row's certified interval, τ = ρ_{g,l}·h, h the certified
  half-width's code term and ρ_{g,l} a number per group and level derived at build (4.2). The row goes on to the next
  level, and after the last to its stored vector, when min(certified upper bound, a + ψτ) ≥ t.
- **Stored vector**: τ = 0; the score enters the k best.

Where it comes from [derived]. Reading a unit further costs c per row and wins the rows above the final threshold.
Greedy recall per unit of cost reads the units whose rows have the largest probability of being above t per unit
of cost; with a normal residual that probability is Φ̄((t − a)/τ), so the rule is a threshold on (t − a)/τ: (4.1) with
ψ = Φ̄⁻¹(λ·c) for one price λ. This is the cluster order's derivation (`PLAN.md`, "theory of the order") applied to a
row instead of a cluster; the cluster's size cancelled there, and here the unit is one row. The costs of the stages
differ by at most 60 times (18.5 B to 2n), which moves ψ by about one unit between the first level and the stored
vector at ψ = 4 to 6; one ψ for the stop and for every stage is within the rule's resolution, and measured it is
within 0 to 4% of the best pair of ψ on glove and yahoo and 14 to 17% on LAION text (section 5).

The exact queries are the same recursion with τ replaced by a bound: the support table and the cut bound a cluster,
the key and each level bound a row, a randomised level's bound is c_δ times a spread whose law is known. The
approximate mode replaces each Cauchy–Schwarz product, the worst alignment of two vectors, by ψ times its spread.

### 4.2 The spread of a level's error [derived, checked on three datasets]

A deterministic level bounds what it has not coded by weighted Cauchy–Schwarz, |Δ·e| ≤ ‖DΔ‖·‖D⁻¹e‖ = h (Δ the query's
weights in the group's basis, e the row's coding error, D the level's weights). The build sets D_c⁴ = v_c/π_c, v_c
the variance of coordinate c's error at the level and π_c the query profile (`GroupPlan2::new`). With those weights
the two factors carry the same amount per coordinate, E(D_cΔ_c)² = E(e_c/D_c)² = a_c = √(v_c π_c). If a row's error
is not aligned with the query, E(Δ·e)² = Σ_c π_c v_c = Σ a_c² while E h² ≈ (Σ a_c)², so

    ρ_{g,l}² = E(Δ·e)² / E h² = Σ_c a_c² / (Σ_c a_c)² = 1 / n_eff,     a_c = D_c²·π_c,                (4.2)

the participation ratio of the level's per-coordinate terms: 1/n when they are equal (Cauchy–Schwarz over n
coordinates overstates a random alignment by √n), more when the residual sits in fewer coordinates. Everything in
(4.2) is the build's own; no query, no fit, no machine.

| | glove (d 100) | yahoo (d 384) | LAION text (d 512) |
|---|---|---|---|
| ρ derived (4.2), 16 B level: min / median / max over the 64 groups | 0.1009 / 0.1013 / 0.1029 | 0.0592 / 0.0638 / 0.0676 | 0.0512 / 0.0561 / 0.0657 |
| ρ measured: rms of (score − midpoint) / half-width over 2 to 5 million rows of 100 queries' clusters | 0.0994 | 0.0613 | 0.0485 |
| the same at 32 B: derived median, measured | 0.1014, 0.0992 | 0.0609, 0.0582 | 0.0527, 0.0459 |
| 1/√d | 0.1000 | 0.0510 | 0.0442 |
| measured ρ·√d per (query, cluster), 5 / 50 / 95%, 16 B | 0.96 / 1.00 / 1.08 | 1.14 / 1.27 / 1.44 | 0.99 / 1.16 / 1.45 |
| the certified half-width in units of the cluster's spread σ_c, 16 B / 32 B (median) | 4.3 / 2.4 | 6.2 / 4.8 | 10.0 / 8.3 |
| share f of the cluster's score variance the level explains, 16 B / 32 B (median) | 0.81 / 0.94 | 0.84 / 0.91 | 0.72 / 0.83 |
| slope of the score on the midpoint, 16 B | 1.04 | 1.09 | 1.09 |
| error / (ρ·h), 99% / 99.9% of all rows (normal: 2.33 / 3.09), 16 B | 2.3 / 3.1 | 2.6 / 3.9 | 2.5 / 3.6 |

(`layered/explore.py` on the dump of `study_layered_dump`; the derived line is printed by `study_layered_kernel`.)

- The derived ρ is within 2 to 5% of the measured one in distribution (glove, yahoo) and 15% above it for LAION's
  text queries, which do not follow the profile the weights were built for; the error is on the safe side (a wider
  spread keeps more rows).
- The certified half-width is 4 to 10 times the spread of the scores it should separate: at a k-NN's threshold, 3 to
  5 spreads above a cluster's mean, it drops almost nothing (as `PLAN.md` counted: 98 / 94 / 81% of LAION's rows
  enter levels 2 / 3 / 4). The statistical width ψρh is 0.25 to 0.6 of a spread at ψ = 4.
- The upper tail is heavier than normal on yahoo and LAION (99.9% at 3.6 to 3.9 spreads), and the midpoint
  under-disperses by 4 to 9%; both make ψ a little less than a normal quantile. They are in the replay's counts.
- A randomised level's half-width is already c_δ times a spread with a known law (`SYSTEM.md` section 3, c = 6.9 to
  8.0); at ψ in place of c_δ its miss probability per row is exact for any data. The replay shows the two
  deterministic levels are enough, so the approximate path can skip the randomised levels and their rotations.

### 4.3 What a stage loses [derived; measured]

Under the normal model a row whose score is exactly t, in a cluster whose mean is g spreads below t, passes a stage
that explains a share f of the cluster's variance with probability Φ((ψ − g√(1 − f))/√f): its estimate regresses to
the cluster's mean by the share the stage does not explain. A stage that explains little cannot drop rows without
dropping answers, whatever its key: at g = ψ = 4 the loss is 0.7% for f = 0.8 (the 16 B level), 1.4% for f = 0.72
(LAION), and 19% for f = 0.2 (the key on glove). Measured against direct reads at the same ψ and the same clusters
(the library's reader, section 5.4): recall falls by 1.2 / 0.7 / 0.2 points at ψ = 5 / 6 / 7 on yahoo, by 0.2 or less
on glove, and by 6.5 / 3.5 / 0.7 / 0.2 points at ψ = 4 / 5 / 6 / 7 on LAION text (out of distribution, f = 0.72).

### 4.4 The key stays certified [measured]

The same rule on the key (a = (q·m̂)·key, τ from the cluster's spread less what the key explains, which is calibrated
with no constant: rms 1.00 of error over τ on glove) was tried and is rejected. The key explains 20% (glove), 6%
(yahoo) and 4% (LAION text) of a cluster's score variance; by 4.3 it loses answers at any useful ψ, the query then
needs more clusters for the same recall (glove at 0.95: 107 against 77; yahoo 26 against 17), and the bytes are more, not fewer (glove 5.10 MB against
4.61, yahoo 3.06 against 2.54). The certified range costs nothing and stays.

### 4.5 The process, in order

1. Centres, stream, sigmas, order and stop: as today.
2. The first cluster (until k rows are held): its first level over every visible row, and the 2k + 8 largest
   estimates scored from stored vectors (the exact k-NN's seed, on one cluster). Today the first cluster is read
   whole, 2,000 to 2,600 stored vectors; this is 28 to 208.
3. Every later cluster read: the certified range at t; the 16 B level over the range; rows with a + ψρh ≥ t to the
   32 B level; those passing it to their stored vectors. A cluster is read this way when the bytes it saves exceed
   its tables and, for a group new to the query, the group's setup; else directly, as today (the library's planner
   makes this choice with its measured costs, which must be timed under the rule; section 10).
4. ψ = ∞ is the exact query, as today.

State added: ρ_{g,l}, 2 × 64 numbers. Work added per query: none (the factor multiplies a half-width the kernel
computes anyway).

## 5. Evidence: the rule on the real codes [measured]

### 5.1 Method

Indexes: the suite's three fits, encoded with this tree's library and its default levels (16 and 32 B deterministic;
56 B and up randomised). Two instruments.

- **Replay B** (`study_layered_dump`, `layered/cascade.py`). The study dumps, for each of the suite's 100 queries, every
  cluster's μ and σ as the library computes them and, for the first M clusters of the library's order, every visible
  row's exact score, key and certified interval from the key and from each of the first three levels. The replay
  reads the clusters in that order with a running k-th best and applies a row rule; it counts rows entering each
  stage, clusters, groups, and turns them into bytes with the table of section 1. Rules: `direct`; `certified` (the
  levels with their certified widths); `statistical` (4.5: the seed, the certified key range, the two deterministic
  levels at ψρh, then stored vectors). A cluster goes through the levels only when that saves bytes (its range's rows
  times a stored vector less a first-level row, against its tables and, if new, its group's setup); the library
  decides the same with measured costs. Stop: the library's (the next 8 clusters' largest μ + ψσ below t), with the
  same ψ as the rows ("one dial").
- **The reader study** (`study_layered_kernel`, `layered/kernel.sh`). The library's own `KnnReader` on the index path
  with `Prep2::dial` set to ψρ per level, the clusters in the library's order and the same stop, against the same
  reads direct and with the dial at 1. No truncation at M clusters, the kernel's own integer path, its own
  first-cluster handling (the k largest keys, which leaves about 1,000 rows of the first cluster to score). Its
  counters (rows entering each level, stored vectors, clusters and groups opened) are turned into bytes the same
  way; "statistical, two levels" scores from stored vectors the rows that survive the 32 B level.

### 5.2 Replay B: bytes to a recall [measured]

k = 10. MB read per query: direct / certified levels / **statistical, ρ measured** / statistical, ρ = 1/√d (direct
over statistical). The last three columns are at recall 0.95. "-": the M clusters dumped do not hold that recall.

| dataset | visible set | recall 0.9 | 0.95 | 0.99 | clusters read, direct → statistical | stored vectors | rows scanned at 16 B |
|---|---|---|---|---|---|---|---|
| glove | none | 28.72 / 7.60 / 2.89 / 2.89 (9.9x) | 46.52 / 10.68 / 4.61 / 4.61 (10.1x) | - | 75 -> 77 | 180,753 -> 397 | 183,821 |
| glove | random-25 | 6.07 / 2.72 / 1.08 / 1.08 (5.6x) | 11.41 / 4.25 / 1.93 / 1.93 (5.9x) | - | 72 -> 75 | 43,615 -> 222 | 45,072 |
| glove | region-25 | 13.13 / 4.68 / 1.78 / 1.78 (7.4x) | 20.87 / 6.54 / 2.82 / 2.82 (7.4x) | - | 82 -> 85 | 80,517 -> 365 | 83,114 |
| glove | random-1 | 0.73 / 0.73 / 0.73 / 0.73 (1.0x) | 1.17 / 1.17 / 1.17 / 1.17 (1.0x) | - | 127 -> 127 | 3,192 -> 3,192 | 0 |
| glove | region-1 | 2.26 / 1.60 / 0.85 / 0.85 (2.6x) | 2.73 / 1.93 / 1.14 / 1.14 (2.4x) | 3.27 / 2.30 / 1.46 / 1.46 (2.2x) | 134 -> 146 | 9,220 -> 1,187 | 8,431 |
| yahoo | none | 10.39 / 5.63 / 1.40 / 2.04 (7.4x) | 23.32 / 9.74 / 2.54 / 3.04 (9.2x) | 51.19 / 16.28 / 4.92 / 6.47 (10.4x) | 15 -> 17 | 30,009 -> 328 | 35,540 |
| yahoo | random-25 | 3.21 / 2.87 / 1.14 / 1.26 (2.8x) | 5.53 / 4.41 / 1.88 / 2.71 (2.9x) | 20.32 / 10.95 / 5.16 / 5.87 (3.9x) | 13 -> 16 | 6,866 -> 178 | 8,424 |
| yahoo | region-25 | 7.74 / 5.97 / 2.03 / 2.62 (3.8x) | 13.28 / 8.91 / 3.44 / 4.38 (3.9x) | 29.41 / 15.17 / 6.92 / 8.29 (4.3x) | 25 -> 32 | 16,853 -> 991 | 21,156 |
| yahoo | random-1 | 0.59 / 0.59 / 0.59 / 0.59 (1.0x) | 0.91 / 0.91 / 0.91 / 0.91 (1.0x) | 1.92 / 1.92 / 1.92 / 1.92 (1.0x) | 33 -> 33 | 684 -> 684 | 0 |
| yahoo | region-1 | 3.34 / 3.71 / 1.39 / 1.44 (2.4x) | 3.99 / 4.37 / 1.74 / 1.83 (2.3x) | 5.12 / 5.41 / 2.48 / 2.55 (2.1x) | 33 -> 46 | 4,700 -> 981 | 4,505 |
| laion | none | 46.67 / 49.13 / 7.39 / 8.42 (6.3x) | 78.90 / 81.16 / 10.83 / 12.68 (7.3x) | 196 / 184 / 20.98 / 19.55 (9.3x) | 36 -> 63 | 76,449 -> 1,802 | 132,794 |
| laion | random-25 | 12.23 / 14.66 / 5.30 / 5.92 (2.3x) | 23.79 / 27.75 / 7.34 / 8.29 (3.2x) | - | 42 -> 58 | 22,583 -> 886 | 31,046 |
| laion | region-25 | 28.99 / 32.61 / 7.72 / 8.66 (3.8x) | 49.58 / 54.39 / 10.74 / 11.41 (4.6x) | - | 55 -> 77 | 47,665 -> 2,284 | 64,833 |
| laion | random-1 | 1.80 / 1.80 / 1.80 / 1.80 (1.0x) | 2.69 / 2.69 / 2.69 / 2.69 (1.0x) | 5.81 / 5.81 / 5.81 / 5.81 (1.0x) | 75 -> 75 | 1,711 -> 1,711 | 0 |
| laion | region-1 | 7.24 / 8.21 / 4.60 / 4.63 (1.6x) | 8.94 / 9.85 / 5.41 / 5.44 (1.7x) | 10.92 / 11.67 / 6.61 / 6.70 (1.7x) | 90 -> 136 | 7,696 -> 2,440 | 6,693 |

k = 100, unfiltered:

| dataset | visible set | recall 0.9 | 0.95 | 0.99 | clusters read | stored vectors | rows scanned at 16 B |
|---|---|---|---|---|---|---|---|
| glove | none | 28.39 / 12.40 / 3.23 / 3.23 (8.8x) | 39.41 / 16.09 / 4.55 / 4.54 (8.7x) | - | 64 -> 69 | 153,077 -> 1,622 | 165,036 |
| yahoo | none | 14.02 / 11.74 / 2.21 / 2.31 (6.3x) | 26.48 / 19.25 / 3.59 / 3.94 (7.4x) | 78.90 / 39.08 / 9.70 / 10.27 (8.1x) | 17 -> 19 | 34,107 -> 1,432 | 37,955 |
| laion | none | 57.34 / 62.58 / 8.70 / 9.29 (6.6x) | 94.18 / 102 / 13.09 / 13.90 (7.2x) | - | 44 -> 60 | 91,307 -> 4,346 | 125,809 |

Parts of the statistical rule's bytes at recall 0.95, k = 10 (MB): centres / sigmas / codes / stored vectors / group
setups / tables.

| dataset | visible set | parts | total |
|---|---|---|---|
| glove | none | 0.08 / 0.17 / 3.46 / 0.10 / 0.33 / 0.47 | 4.61 |
| glove | random-25 | 0.08 / 0.17 / 0.85 / 0.06 / 0.31 / 0.46 | 1.93 |
| glove | region-25 | 0.08 / 0.19 / 1.57 / 0.09 / 0.37 / 0.52 | 2.82 |
| glove | random-1 | 0.08 / 0.28 / 0.00 / 0.82 / 0.00 / 0.00 | 1.17 |
| glove | region-1 | 0.08 / 0.31 / 0.16 / 0.30 / 0.10 / 0.18 | 1.14 |
| yahoo | none | 0.13 / 0.16 / 0.67 / 0.25 / 1.23 / 0.11 | 2.54 |
| yahoo | random-25 | 0.13 / 0.15 / 0.16 / 0.14 / 1.20 / 0.10 | 1.88 |
| yahoo | region-25 | 0.13 / 0.25 / 0.41 / 0.76 / 1.73 / 0.16 | 3.44 |
| yahoo | random-1 | 0.13 / 0.25 / 0.00 / 0.53 / 0.00 / 0.00 | 0.91 |
| yahoo | region-1 | 0.13 / 0.33 / 0.09 / 0.75 / 0.38 / 0.06 | 1.74 |
| laion | none | 0.26 / 0.58 / 2.53 / 1.85 / 5.23 / 0.39 | 10.83 |
| laion | random-25 | 0.26 / 0.54 / 0.60 / 0.91 / 4.68 / 0.35 | 7.34 |
| laion | region-25 | 0.26 / 0.69 / 1.25 / 2.34 / 5.78 / 0.43 | 10.74 |
| laion | random-1 | 0.26 / 0.68 / 0.00 / 1.75 / 0.00 / 0.00 | 2.69 |
| laion | region-1 | 0.26 / 1.18 / 0.13 / 2.50 / 1.23 / 0.11 | 5.41 |

- **Unfiltered: 6 to 10 times fewer bytes** than direct reads, at every recall the dump reaches, on all three
  datasets; stored vectors read fall from 30k to 180k to a few hundred or thousand.
- **25% sets: 2.3 to 7.4 times** (glove 5.6 to 7.4, yahoo 2.8 to 4.3, LAION 2.3 to 4.6). Less than unfiltered because
  the fixed parts (group setups, tables, sigmas) are spread over a quarter of the rows.
- **1% random sets: exactly today's reads** (ratio 1.0): no cluster holds enough visible rows to pay for its tables
  and its group, so the rule never engages. **1% region sets: 1.6 to 2.6 times**: the visible rows sit in few
  clusters, which are then read like unfiltered ones.
- **Where the certified levels already help** (glove, yahoo: 2 to 4 times fewer bytes than direct) the statistical
  width gains another 2.3 to 4; where they do not (LAION text: 1.0) it gains the whole factor.
- ρ = 1/√d with no build quantity at all is as good on glove (where it is exact) and 10 to 45% worse on yahoo and
  LAION, where it is too narrow by 1.1 to 1.2 and unbalances the one dial. The derived ρ (4.2) is the one to use.
- **A group's setup is the largest part at n ≥ 384** (yahoo 1.2 of 2.5 MB, LAION 5.2 of 10.8): section 8.4.
- At k = 100 the gains are 0.85 to 0.9 of those at k = 10.

### 5.3 Variants [measured, replay B, one dial, MB at recall 0.9 / 0.95, unfiltered]

| | glove | yahoo | LAION text |
|---|---|---|---|
| 16 and 32 B (the proposal) | 2.89 / 4.61 | 1.40 / 2.54 | 7.39 / 10.83 |
| best pair of ψ (rows, stop) per recall | 2.88 / 4.59 | 1.36 / 2.44 | 6.33 / 9.53 |
| 16 B only, then stored vectors | 3.37 / 5.52 | 1.58 / 2.89 | 9.10 / 12.88 |
| 16, 32 and 56 B (the first randomised level, at its measured ρ) | - | 1.39 / 2.50 | 7.79 / 10.56 |
| the key's range statistical too | 3.54 / 5.10 | 2.02 / 3.06 | not better (9.05 at 0.95 with 48 clusters dumped) |
| a 4 B or 8 B first level (six-level build): 8, 16, 32 / 4, 16, 32 / all six | | 1.57 / 2.48, 1.82 / 2.58, 1.85 / 2.61 | |

Two deterministic levels are the right depth: one costs 15 to 20% more (more stored vectors), a third changes
nothing. A finer first level does not pay at 1M on yahoo (it loses recall per cluster, 4.3, and codes are a quarter
of the bytes there).

### 5.4 The library's own reader with the factor [measured]

MB read per query to a recall, interpolated along ψ (3 to 8): direct / certified, every level (rotations included) /
**statistical, two levels** (direct over statistical). Full tables, with the rows entering each level:
`layered/results/kernel_tables.md`.

| dataset | visible set | recall 0.9 | recall 0.95 | recall 0.98 | recall 0.99 | recall 0.995 |
|---|---|---|---|---|---|---|
| glove | none | 29.49 / 5.84 / 3.38 (8.7x) | 55.49 / 10.00 / 5.91 (9.4x) | 101 / 16.83 / 10.20 (9.9x) | 139 / 22.27 / 13.64 (10.2x) | 172 / 26.61 / 16.80 (10.3x) |
| glove | random-25 | 6.07 / 2.21 / 1.18 (5.1x) | 12.23 / 4.04 / 2.17 (5.6x) | 19.40 / 6.12 / 3.40 (5.7x) | 29.07 / 8.80 / 4.68 (6.2x) | 37.71 / 11.12 / 5.94 (6.3x) |
| glove | region-25 | 13.15 / 3.69 / 2.06 (6.4x) | 20.56 / 5.54 / 3.11 (6.6x) | 30.46 / 8.02 / 4.35 (7.0x) | 38.98 / 10.30 / 5.59 (7.0x) | 47.15 / 12.52 / 6.83 (6.9x) |
| laion | none | 53.50 / 14.62 / 10.16 (5.3x) | 101 / 24.15 / 15.08 (6.7x) | 182 / 39.22 / 21.81 (8.3x) | 255 / 51.77 / 28.28 (9.0x) | 301 / 59.49 / 34.73 (8.7x) |
| laion | random-25 | 14.31 / 8.98 / 6.46 (2.2x) | 33.20 / 17.65 / 9.44 (3.5x) | 54.73 / 25.95 / 15.45 (3.5x) | 78.21 / 35.12 / 20.34 (3.8x) | 156 / 56.54 / 23.93 (6.5x) |
| laion | region-25 | 32.82 / 14.90 / 9.78 (3.4x) | 62.15 / 24.97 / 14.86 (4.2x) | 108 / 40.03 / 21.28 (5.1x) | 141 / 50.18 / 23.85 (5.9x) | 174 / 60.38 / 28.67 (6.1x) |
| yahoo | none | 10.39 / 3.72 / 2.70 (3.8x) | 19.65 / 5.64 / 3.52 (5.6x) | 33.49 / 8.25 / 5.12 (6.5x) | 48.56 / 10.88 / 6.75 (7.2x) | 84.98 / 16.56 / 9.24 (9.2x) |
| yahoo | random-25 | 3.21 / 2.52 / 1.44 (2.2x) | 5.53 / 3.92 / 2.15 (2.6x) | 11.89 / 7.35 / 3.82 (3.1x) | 20.90 / 11.58 / 5.44 (3.8x) | 28.98 / 15.13 / 7.72 (3.8x) |
| yahoo | region-25 | 7.74 / 4.56 / 2.56 (3.0x) | 13.50 / 7.04 / 3.78 (3.6x) | 22.51 / 10.84 / 5.48 (4.1x) | 27.94 / 12.92 / 7.09 (3.9x) | 36.88 / 16.24 / 8.70 (4.2x) |

Recall and clusters at the same ψ, direct against statistical (unfiltered): glove 0.930 / 0.928 at ψ = 4, 0.989 /
0.989 at 5 (241 clusters), 0.998 / 0.998 at 6; yahoo 0.922 / 0.910 at 5, 0.982 / 0.975 at 6, 0.994 / 0.992 at 7;
LAION text 0.893 / 0.828 at 4, 0.983 / 0.948 at 5, 0.997 / 0.990 at 6, 0.999 / 0.997 at 7. Rows entering the 16 B /
32 B levels and stored vectors at ψ = 6, statistical: glove 846,947 / 26,965 / 3,269 (direct: 847,337 vectors);
yahoo 44,750 / 2,781 / 2,157 (44,706); LAION 435,862 / 12,869 / 5,260 (434,801).

- The reader confirms the replay where both reach: **5 to 10 times fewer bytes unfiltered from recall 0.9 to 0.995,
  2.2 to 7 times under the 25% sets**, with the kernel as it is and one ρ per level.
- It also shows what the replay could not: at recall 0.99 the library's stop reads 240 (glove), 216 (LAION) and 46
  (yahoo) clusters, 40% of the corpus on glove and LAION. That is the cost of a high ψ `PLAN.md` records; under the
  rule those marginal clusters cost 18.5 B a row, and 97 to 99.5% of their rows stop at the first level.
- The certified path already reads 4 to 6 times fewer bytes than direct reads, and the library still reads glove
  and yahoo directly at 1M because its time is no better: bytes are an upper bound on the gain (section 10).

## 6. Over the clusters

**What the centre scan weighs.** After the levels' rule the centres are 2 to 5% of an unfiltered query's bytes at
1M (0.08 to 0.26 MB of 2.5 to 11 MB at recall 0.95), and by the law of section 2 the read grows faster than the
centres (N^(γ/2+η) against N^0.5), so unfiltered the share falls with N. Under a sparse visible set the read
shrinks with the visible share and the centres do not: at 1% random sets the centres and sigmas are 31 to 42% of the bytes at 1M (section 5.2), and in the model of section 8 the centres are the largest term at 100M and 1B under 1% sets. So a level over the centres serves one
regime, K in the thousands under sparse sets, as `theory.md` concluded; the measurements below say which level.

**A sketch against a tree [measured, replay A, full sets].** The sketch scores every centre from its first r
coordinates in the centres' principal basis (int8, K·r bytes; the query's projection is r·n) and then scores in full
only the centres whose sketch key ranks high enough to hold 99% of the clusters the exact order reads to recall
0.95. The tree makes √K parents by k-means over the centres, keys a parent as one cluster (the mean and spread of
its rows, `theory.md` 3.1) and scores the children of the P best.

| | K | sketch: best r, centre products / K | tree: centre products / K for at most +5% rows at 0.9 and 0.95 |
|---|---|---|---|
| glove | 4734 | r = 32: 0.61 | - |
| glove | 1184 | r = 32: 0.67 | 1.00 |
| yahoo | 2709 | r = 32: 0.15 | 0.63 |
| yahoo | 677 | r = 32: 0.17 | 0.65 |
| LAION text | 4002 | r = 64: 0.23 | 0.55 |
| LAION text | 1000 | r = 64: 0.25 | 0.59 |
| sift (L2) | 4000 | r = 16: 0.26 | 0.29 |
| sift (L2) | 1000 | r = 16: 0.26 | 0.53 |

("-": no P up to 32 holds the rows within 5%.)

- The sketch needs a quarter to a half of the tree's products on yahoo and LAION, matches it on sift, and neither
  helps much on glove, whose 100 dimensions leave little to sketch (a third of the coordinates to keep 85% of the
  between-centre variance) and whose answers lie in many clusters.
- It is one more stage of rule (4.1): estimate = the sketch's score, spread = the standard deviation over the
  clusters of what the sketch leaves out, which for a query is √(q_⊥ᵀC_⊥q_⊥) from the centres' covariance outside the
  basis. No partition of the clusters, no node to maintain; visible counts apply per cluster as now.
- **Verdict.** Off at K ≤ 600. From K = 2,000 (16M rows) the sketch, r = 32, is the centres' first stage (rule 3,
  section 8.3). Expect 0.15 to 0.3 of the centre bytes on data like yahoo, LAION and sift and 0.6 on data like glove.

Certified shared directions were ruled out on paper (section 3). The one certified structure over the clusters
that works is the support table, because it picks its direction per query: with β = q·p̂ for the nearest of 1,024
anchors the remainder is √(1 − β²)·R, small only because β is as large as a direction can make it.

## 7. Under the clusters

**The rows of a read cluster are the cost** (section 2), and there are two ways to touch fewer bytes of them.

**(a) A finer first level, same rows.** The fit's six-level schedule (4, 8, 12, 16, 24, 32 B; `--dlv fit`) was
built for yahoo and the rule run on it (5.3). It does not pay there: 2.48 to 2.63 MB against 2.54 at recall 0.95. A
4 B level explains too little of a cluster's variance, so by 4.3 it drops answers and the query reads 2 to 3 more
clusters; and at 1M the codes are a quarter of the bytes. It remains a candidate where codes dominate (glove: 75%
of the bytes; any dataset at 100M), to be measured there; it would cost the exact queries a level's work
(`rust/REPORT_DETLEVELS.md` chose two levels for them).

**(b) Fewer rows: sub-clusters.** By the law of section 2 leaves of size s inside clusters of size S cut the rows by
(S/s)^γ, 2.1 to 3.6 times for 16 leaves of 125 rows at 1M, as `theory.md` and `replay.md` measured. What they cost
is settled by `subclusters.md`: the gain needs the leaves contiguous (layout (c)), and as built that cost the exact
queries 5 to 17%, so they were shelved. I agree at 1M, and the law says where that changes:

- At 1M the clusters are already the smallest unit the exact path tolerates; leaves of 125 rows are 4 blocks of 32,
  so half of a cluster's blocks straddle a run boundary and the sorted range needs 16 searches for 2,000 rows.
- From 8M rows the rule makes clusters of 4,000 to 64,000 rows. **Leaves of the size a cluster has at 1M (2,000
  rows, 62 blocks)** leave 3% of the blocks straddling and cost one search per 2,000 rows, which is what the exact
  path pays at 1M today. Each leaf is then a 1M-style cluster inside the large one: an int8 centre, a radius, its 16
  directions, the same state per row as now. The exact path keeps its large clusters (support table, cut, tables,
  groups); the approximate path expands a large cluster into its leaves by the same rule (4.1) and reads leaves.
- What it buys [model, section 8]: rows ÷ (S/2,000)^γ, that is 1.3 to 1.5 times at 8M, 1.9 to 2.9 in bytes at 100M
  and 3.3 to 5.5 at 1B, and it is the only candidate that changes the exponent of the read (N^η in place of N^(γ/2+η)).
- So: nothing under the clusters at 1M to 2M beyond the levels' rule; **from the first N at which S exceeds 2,000
  (8M rows), split clusters into 2,000-row runs** (rule 2, section 8.2), after timing the exact queries on such a build (`--per-cluster 8000` with
  4 runs at 1M reproduces the ratio). This is `subclusters.md`'s third condition, with the leaf size fixed by the
  exact path instead of by the approximate one.

Leaves smaller than that (250 rows) buy little more in the model: their sub-centres and sigmas cost about what the
extra rows save.

**Measured in the shape of a large index [replay A2, `layered/nested.py`, 500 queries].** Leaves of 250 rows grouped
8 or 32 to a parent (the ratio of a 16,000- or 64,000-row cluster to 2,000-row leaves). Rows to recall at k = 10:

| | recall | parents read whole: rows (parents) | leaves, flat order: rows | nested: parents expanded, centres scored, rows / flat |
|---|---|---|---|---|
| sift, 500 parents of 8 | 0.95 / 0.99 | 60,813 (27) / 120,405 (55) | 14,204 / 31,409 | 64, 1,055, 1.01 / 128, 1,585, 1.00 |
| sift, 125 parents of 32 | 0.95 / 0.99 | 102,020 (12) / 183,176 (22) | 14,204 / 31,409 | 32, 1,196, 1.01 / 64, 2,159, 1.00 |
| yahoo, 338 parents of 8 | 0.95 / 0.99 | 34,749 (17) / 120,022 (60) | 7,583 / 39,031 | 64, 859, 1.02 / 128, 1,380, 0.98 |
| yahoo, 84 parents of 32 | 0.95 / 0.99 | 73,787 (9) / 239,544 (30) | 7,583 / 39,031 | 32, 1,125, 1.04 / 64, 2,145, 0.99 |

Expanding 2 to 4 times the parents the parent order would read reaches the flat order's rows, 4 to 9 times fewer
than reading parents whole, while scoring a quarter to a half of the leaf centres. (Parents made by k-means over
leaf centres are worse clusters than the build's, so against real clusters of the parents' size the gain is the
law's (S/s)^γ, 2.3 to 4.7 here.)

## 8. Design for 100M to 1B rows

The target is 100M to 1B rows and beyond; every structure below must cost nothing, or a few per cent, at 1M to 10M.
At 1B the build rule (`fit::per_cluster_by_n`) gives S = 64,000 and K = 15,625; at 100M, S = 16,000 and K = 6,250. The
groups rule (`fit::Groups::count`, by-n) gives min(64, K, N/(20n)) = 64 from 1M on, so a group covers 8 clusters at
1M, 98 at 100M and 244 at 1B.

### 8.1 What each term becomes [model]

`layered/extrapolate.py` carries replay A's law (fitted per dataset on 62k to 1.2M rows; the rows of the nprobe curve
with exact spreads, k = 10) to larger N, with the bytes of section 1. **It is a model, not a measurement.** A random
visible share v is treated as a subsample (measured in section 2: the law at (S·v, N·v) with the same K). Inputs
besides the law: 1% of first-level rows go on to stored vectors (0.2 to 3% measured); the share of groups opened is
2.5 times the share of clusters read (reader study: 2 to 2.7); a sketch scores K·32 bytes and 3m centres in full
(depth measured 2 to 7 m); 2.5 m clusters are expanded into leaves (replay A2: 2 to 4). Each design's planner takes
the cheaper, in these bytes, of reading a unit through the levels or directly, and of expanding clusters or not.

MB read per query to recall 0.95 (recall 0.99 and 10M in `layered/results/extrapolate.md`). `today`: direct reads.
`levels`: the rule of section 4, nothing else. `all`: the rule, leaves of 2,000 rows, the centres' sketch, and
G = max(64, min(N/(3n²), K/8)) groups.

| dataset | N | K, S | visible | rows read (k), clusters | today | levels: total, largest terms | all: total, largest terms | G (all) |
|---|---|---|---|---|---|---|---|---|
| glove | 1,000,000 | 500, 2,000 | 1 | 116, 58 | 29.8 | 3.3: codes 2.1, tables 0.3 | 3.3: codes 2.1, tables 0.3 | 64 |
| glove | 1,000,000 | 500, 2,000 | 0.25 | 32, 64 | 8.4 | 1.6: codes 0.6, tables 0.4 | 1.6: codes 0.6, tables 0.4 | 64 |
| glove | 1,000,000 | 500, 2,000 | 0.01 | 2, 82 | 0.7 | 0.7: vectors 0.4, sigmas 0.2 | 0.7: vectors 0.4, sigmas 0.2 | 64 |
| glove | 100,000,000 | 6,250, 16,000 | 1 | 2,934, 183 | 752 | 64.2: codes 54.3, vectors 7.5 | 33.7: codes 23.3, tables 3.8 | 781 |
| glove | 100,000,000 | 6,250, 16,000 | 0.25 | 814, 203 | 210 | 19.7: codes 15.1, vectors 2.1 | 15.3: codes 6.5, tables 4.2 | 781 |
| glove | 100,000,000 | 6,250, 16,000 | 0.01 | 41, 259 | 12.0 | 3.9: tables 1.6, centres 0.8 | 4.6: tables 1.6, groups 1.3 | 781 |
| glove | 1,000,000,000 | 15,625, 64,000 | 1 | 17,008, 266 | 4,357 | 362: codes 315, vectors 43.5 | 109: codes 76.8, tables 12.5 | 1,953 |
| glove | 1,000,000,000 | 15,625, 64,000 | 0.25 | 4,717, 295 | 1,210 | 104: codes 87.3, vectors 12.1 | 48.6: codes 21.3, tables 13.8 | 1,953 |
| glove | 1,000,000,000 | 15,625, 64,000 | 0.01 | 240, 375 | 64.3 | 10.2: codes 4.4, tables 2.3 | 10.7: codes 4.4, tables 2.3 | 1,953 |
| yahoo | 1,000,000 | 500, 2,000 | 1 | 31, 15 | 23.8 | 1.9: groups 0.7, codes 0.6 | 1.9: groups 0.7, codes 0.6 | 64 |
| yahoo | 1,000,000 | 500, 2,000 | 0.25 | 9, 17 | 7.0 | 1.5: groups 0.8, centres 0.2 | 1.5: groups 0.8, centres 0.2 | 64 |
| yahoo | 1,000,000 | 500, 2,000 | 0.01 | 0, 24 | 0.7 | 0.7: vectors 0.4, sigmas 0.2 | 0.7: vectors 0.4, sigmas 0.2 | 64 |
| yahoo | 100,000,000 | 6,250, 16,000 | 1 | 416, 26 | 322 | 13.8: codes 7.7, vectors 3.2 | 4.7: codes 2.1, vectors 0.9 | 226 |
| yahoo | 100,000,000 | 6,250, 16,000 | 0.25 | 118, 30 | 93.6 | 6.1: centres 2.4, codes 2.2 | 2.8: sigmas 0.7, codes 0.6 | 226 |
| yahoo | 100,000,000 | 6,250, 16,000 | 0.01 | 6, 40 | 7.6 | 3.3: centres 2.4, sigmas 0.3 | 1.5: groups 0.5, sigmas 0.3 | 226 |
| yahoo | 1,000,000,000 | 15,625, 64,000 | 1 | 1,902, 30 | 1,467 | 56.4: codes 35.2, vectors 14.6 | 10.3: codes 4.1, vectors 1.7 | 1,953 |
| yahoo | 1,000,000,000 | 15,625, 64,000 | 0.25 | 542, 34 | 422 | 20.8: codes 10.0, centres 6.0 | 6.6: groups 1.6, codes 1.2 | 1,953 |
| yahoo | 1,000,000,000 | 15,625, 64,000 | 0.01 | 29, 46 | 28.8 | 7.5: centres 6.0, codes 0.5 | 4.0: groups 2.1, centres 0.6 | 1,953 |
| laion | 1,000,000 | 500, 2,000 | 1 | 56, 28 | 57.6 | 4.7: groups 2.3, codes 1.0 | 4.7: groups 2.3, codes 1.0 | 64 |
| laion | 1,000,000 | 500, 2,000 | 0.25 | 15, 31 | 16.4 | 3.8: groups 2.6, sigmas 0.3 | 3.8: groups 2.6, sigmas 0.3 | 64 |
| laion | 1,000,000 | 500, 2,000 | 0.01 | 1, 39 | 1.4 | 1.4: vectors 0.8, sigmas 0.4 | 1.4: vectors 0.8, sigmas 0.4 | 64 |
| laion | 100,000,000 | 6,250, 16,000 | 1 | 1,020, 64 | 1,048 | 33.9: codes 18.9, vectors 10.4 | 14.4: codes 6.2, vectors 3.4 | 127 |
| laion | 100,000,000 | 6,250, 16,000 | 0.25 | 283, 71 | 293 | 12.9: codes 5.2, centres 3.2 | 8.0: sigmas 2.2, codes 1.7 | 127 |
| laion | 100,000,000 | 6,250, 16,000 | 0.01 | 14, 90 | 18.7 | 5.6: centres 3.2, sigmas 0.8 | 3.3: groups 1.2, sigmas 0.8 | 127 |
| laion | 1,000,000,000 | 15,625, 64,000 | 1 | 5,253, 82 | 5,388 | 160: codes 97.2, vectors 53.8 | 38.6: codes 15.2, vectors 8.4 | 1,272 |
| laion | 1,000,000,000 | 15,625, 64,000 | 0.25 | 1,457, 91 | 1,500 | 51.5: codes 26.9, vectors 14.9 | 23.1: groups 4.9, sigmas 4.6 | 1,272 |
| laion | 1,000,000,000 | 15,625, 64,000 | 0.01 | 74, 116 | 84.9 | 12.1: centres 8.0, codes 1.4 | 10.7: groups 6.2, codes 1.4 | 1,272 |
| sift | 1,000,000 | 500, 2,000 | 1 | 34, 17 | 8.9 | 1.0: codes 0.6, tables 0.1 | 1.0: codes 0.6, tables 0.1 | 64 |
| sift | 1,000,000 | 500, 2,000 | 0.25 | 10, 20 | 2.7 | 0.6: codes 0.2, tables 0.1 | 0.6: codes 0.2, tables 0.1 | 64 |
| sift | 1,000,000 | 500, 2,000 | 0.01 | 1, 29 | 0.3 | 0.3: vectors 0.1, sigmas 0.1 | 0.3: vectors 0.1, sigmas 0.1 | 64 |
| sift | 100,000,000 | 6,250, 16,000 | 1 | 653, 41 | 168 | 14.9: codes 12.1, vectors 1.7 | 7.0: codes 4.7, tables 0.8 | 781 |
| sift | 100,000,000 | 6,250, 16,000 | 0.25 | 190, 48 | 49.7 | 5.2: codes 3.5, centres 0.8 | 3.5: codes 1.4, tables 0.9 | 781 |
| sift | 100,000,000 | 6,250, 16,000 | 0.01 | 11, 68 | 3.8 | 1.6: centres 0.8, tables 0.4 | 1.4: tables 0.4, groups 0.3 | 781 |
| sift | 1,000,000,000 | 15,625, 64,000 | 1 | 3,331, 52 | 855 | 72.6: codes 61.6, vectors 8.5 | 18.8: codes 12.8, tables 2.1 | 1,953 |
| sift | 1,000,000,000 | 15,625, 64,000 | 0.25 | 972, 61 | 251 | 23.0: codes 18.0, vectors 2.5 | 9.1: codes 3.7, tables 2.4 | 1,953 |
| sift | 1,000,000,000 | 15,625, 64,000 | 0.01 | 56, 87 | 16.5 | 3.9: centres 2.0, codes 1.0 | 2.9: codes 1.0, centres 0.5 | 1,953 |

Which term dominates:

| | unfiltered | 25% visible | 1% visible |
|---|---|---|---|
| today, 100M and 1B | stored vectors, 0.2 to 5 GB a query | stored vectors, 50 MB to 1.5 GB | stored vectors, 4 to 85 MB |
| `levels`, 100M | first-level codes (55 to 85%), then stored vectors | codes; the centre scan reaches 15 to 40% at n ≥ 384 | **the centre scan** (n ≥ 384: 55 to 70%); tables on glove |
| `levels`, 1B | codes (60 to 87%) | codes, then vectors or centres | **the centre scan** (50 to 80%), then codes |
| `all`, 100M and 1B | codes of the leaves read (40 to 70%), then vectors and tables | codes, sigmas, tables and group setups within a factor 2 of each other | group setups and sigmas (n ≥ 384); tables and codes (n = 128) |

- Unfiltered and at 25% the read is the cost at every N, and it is the term that grows (N^(γ/2+η)). The levels' rule
  divides it by 12 to 34 at 1B; leaves divide the rest by 3.3 to 5.5 (glove 362 → 109 MB, yahoo 56 → 10, LAION
  160 → 39, sift 73 → 19).
- Under 1% sets at 100M and beyond a cluster holds 160 to 640 visible rows: the levels' rule engages there (it does
  not at 1M), and what is left is the centre scan, K·n = 2 to 8 MB at 1B, which the sketch takes to 0.5 to 0.8 MB.
- Sigma pricing ((m + 8)·16n) is 1 to 6 MB at 1B, never the largest term in `levels`; with leaves it is paid per leaf
  read and becomes comparable to the codes under filters. The stream already skips a sigma that costs more than
  the read it prices; with leaves that rule should compare a leaf's sigma (16n) with the leaf's visible rows.
- **Where the extrapolation is weakest.** (i) The exponents come from a 16-fold range of N (62k to 1.2M) and are
  carried 1,000-fold; η is the sensitive one: ±0.1 in η is a factor 2 at 1B. (ii) Subsampling a 1M corpus thins it;
  a real 1B corpus is denser in the same space, which is what the law models, but its clusters may not be
  self-similar. (iii) The law is for the nprobe curve with exact spreads; the library's stop reads 1.3 to 3 times
  more clusters at recall 0.99 (5.4). (iv) The leaves' gain assumes nested leaves behave as flat ones (measured at 1M
  in replay A2 and in `replay.md`). (v) Group counts above 64 were not encoded; their effect is a proxy (8.4).
- **What to measure on a 100M+ set** (Deep1B or BigANN/SIFT1B slices at 10M and 100M; LAION-100M or MS-Turing 100M
  for a high-dimensional, partly out-of-distribution case): rows and clusters to recall 0.9 / 0.95 / 0.99 at the
  build rule's S and at S = 2,000, with the order μ + 3σ (`scaling.py` does it from a partition and brute-force
  scores of 1,000 queries; at 100M the scores can be streamed by cluster). Two numbers decide the plan: η (rows at
  S = 2,000 against N) and γ (rows at the rule's S against rows at 2,000). If η is under 0.3 the leaves matter more
  than the model says; if γ is under 0.3 (glove at recall 0.99) they matter less.

### 8.2 The level under the clusters: leaves of 2,000 rows (from 8M)

Section 7(b). Rule: **a cluster of more than 2,000 rows is stored as runs of 2,000**, each a contiguous, key-sorted
block with its own int8 centre, radius and 16 directions (the state a 1M cluster has: 16n + 136 + n bytes per
2,000 rows, 4.4 B a row at n = 512, the same per row at every N). The approximate path expands the clusters whose key
passes (4.1) and reads leaves; the exact path keeps the large cluster as its unit and meets a run boundary once
per 2,000 rows.

- At 1M to 2M: **off by the rule** (S = 2,000: a cluster is one leaf). Overhead zero.
- 8M to 32M (S = 4,000 to 8,000): 2 to 4 leaves; model gain 1.3 to 1.6 in bytes unfiltered.
- 100M: 8 leaves; `levels` → `all` 64 → 34 MB (glove), 14 → 4.7 (yahoo), 34 → 14 (LAION), 15 → 7 (sift).
- 1B: 32 leaves; 362 → 109, 56 → 10, 160 → 39, 73 → 19 MB. Rows ∝ N^η (0.3 to 0.5 at recall 0.95) in place of
  N^(γ/2+η) (0.6 to 0.72).
- Under a visible set a cluster is expanded only when its visible rows pay for its leaves' centres (the planner's
  comparison in bytes: (S/2,000)·n against the first-level rows the leaves save); under 1% sets at 100M the model
  leaves clusters whole.
- Exact queries: to be timed before adoption (the one measurement this needs; `subclusters.md` measured 5 to 17% with
  125-row runs at 1M, where half the 32-row blocks straddled a boundary; with 2,000-row runs 3% do). Certification
  is unchanged: runs are the same rows and codes in another order within the cluster, each run sorted by key.
- Visible counts per leaf are a popcount of a contiguous row range, as for clusters.

### 8.3 The level over the clusters: the centres' sketch (from K = 2,000)

Section 6. Rule: **when K ≥ 2,000 (N ≥ 16M) the stream's centre scores come from r = 32 int8 coordinates per centre
in the centres' principal basis, and a centre is scored in full when its sketch key passes (4.1)** with the spread
of what the sketch leaves out. Measured at K = 2,700 to 4,700: 0.15 (yahoo), 0.23 (LAION), 0.26 (sift) and 0.61
(glove) of the centre products; never more than the scan (0.87 at worst over every K and r tried).

- At 1M (K = 500): **off by the rule**; overhead zero. (Switched on it would save 0.1 to 0.2 MB of 2.5 to 11.)
- At 100M: the centre scan 0.8 to 3.2 MB → 0.3 to 0.7; at 1B: 2 to 8 MB → 0.6 to 1.3. It matters under sparse
  sets: yahoo at 1B and 1%: 7.5 → 4.0 MB with the other rules.
- State: K·32 bytes and a 32 × n basis (0.5 MB at 1B). Exact queries do not read it.
- With leaves the same sketch serves the leaves of an expanded cluster (32 leaves × 32 B in place of 32 × n).

**Could the groups be this level, with G scaled?** Rechecked. As a tree level they are the "tree" column of section
6 (parents by k-means over the centre directions is how groups are made): with √K parents the children of 30 to 65%
of them must be scored to hold the rows within 5% on yahoo, LAION and glove (29% on sift), against 15 to 26% of the
products for the sketch on three of the four. A level of parents loses to a sketch because a parent is keyed by one
centre and one spread, while the sketch gives every centre its own 32 coordinates. Where the groups do help as a
level is **emptiness**: a per-group visible count rejects 244 clusters at a time under region sets, for nothing
(8.5).

### 8.4 The number of groups G(N, K, n)

Today G = min(64, K, N/(20n)). What moves with G:

- **Setup per query**: n² bytes of int8 basis per group opened (and for exact queries the rotations of the query's
  weights, 0.7 to 1.2 n² more). Storage G·n² (17 MB at n = 512 and G = 64; 0.5 GB at G = 2,000).
- **Groups opened** [measured, reader study, G = 64]: the share of groups opened is 2 to 2.7 times the share of
  clusters read while that is small (LAION: 22 clusters → 7.8 groups, 92 → 23, 216 → 43; glove 57 → 15, 241 → 37;
  yahoo 8.3 → 4.5, 46 → 18). The clusters a query reads are neighbours, and so are a group's.
- **Tightness** [proxy, `layered/groups.py`]: each group's basis as the eigenbasis of its pooled residual covariance,
  bits by reverse water-filling per group, a cluster's distortion in its group's coordinates. yahoo (K 339, n 384):

| groups | clusters per group | f at 16 B | f at 32 B | bits a row for G = 64's distortion at 16 B (128 bits there) | int8 bases, MB |
|---|---|---|---|---|---|
| 1 | 339 | 0.533 | 0.735 | 186 | 0.1 |
| 2 | 170 | 0.548 | 0.745 | 178 | 0.3 |
| 4 | 85 | 0.569 | 0.758 | 167 | 0.6 |
| 8 | 42 | 0.593 | 0.774 | 154 | 1.2 |
| 16 | 21 | 0.608 | 0.783 | 146 | 2.4 |
| 32 | 11 | 0.623 | 0.793 | 137 | 4.7 |
| 64 | 5 | 0.641 | 0.804 | 128 | 9.4 |
| 128 | 3 | 0.663 | 0.817 | 117 | 18.9 |
| 250 | 1 | 0.690 | 0.835 | 103 | 36.9 |
| 339 | 1 | 0.704 | 0.844 | 96 | 50.0 |

  glove (K 592, n 128) is flat: f at 16 B 0.832 (1 group), 0.835 (8), 0.839 (64), 0.851 (592); 131 → 128 → 122 bits.

  LAION text (K 500, n 512), the same columns:

| groups | clusters per group | f at 16 B | f at 32 B | bits for G = 64's distortion at 16 B | int8 bases, MB |
|---|---|---|---|---|---|
| 1 | 500 | 0.467 | 0.639 | 226 | 0.3 |
| 2 | 250 | 0.490 | 0.659 | 209 | 0.5 |
| 4 | 125 | 0.513 | 0.679 | 191 | 1.0 |
| 8 | 62 | 0.545 | 0.705 | 168 | 2.1 |
| 16 | 31 | 0.565 | 0.720 | 154 | 4.2 |
| 32 | 16 | 0.585 | 0.736 | 142 | 8.4 |
| 64 | 8 | 0.606 | 0.753 | 128 | 16.8 |
| 128 | 4 | 0.634 | 0.774 | 112 | 33.6 |
| 250 | 2 | 0.662 | 0.795 | 96 | 65.5 |

  (LAION's run was stopped before its last row.) On yahoo each doubling of the groups is worth about 10 bits a
  row at the 16 B level's distortion (8% of the code), on LAION 12 to 23 bits (9 to 18%), from 1 group to one per
  cluster; on glove, nearly isotropic, nothing. At 1B with 64 groups a group pools
  244 clusters, the left end of the table: on yahoo-like data the 16 B level would explain what about 11 B explain
  with today's 5 to 8 clusters a group, on LAION-like data what about 10 B explain. That widens every certified interval too (the exact queries read deeper),
  and it lowers f, so the levels' rule passes more rows and loses more recall per stage (4.3). The proxy is PCA
  with isotropic queries, not the dual basis; the trend, not the level, is what it shows.
- **The support table and certification** do not depend on G: the table is anchors × clusters, every bound holds in
  any basis, the randomised constant depends on the block layout (n), and the rotations are shared by all groups.
- **AVX2 and AVX-512**: a group's setup is an int8 product of n² terms (twice, for the query's two halves); AVX-512
  VNNI does it in about half the operations of AVX2. The index must not depend on the instruction set, so G is
  set for the dearer one.

**The rule [derived].** A group pays when what its tighter basis saves on the rows read under it exceeds its setup.
Let β be the share of a row's bytes a doubling of G saves (the proxy's slope: 0.08 on yahoo, 0.12 on LAION, 0.01
on glove), b the
bytes a query reads per row of a cluster it reads, a = 2.5 the concentration above, s the setup in units of n².
Per query the rows cost m·S·b·(1 − β·log₂G) and the setups a·(m/K)·G·s·n²; the minimum over G is at

    G* = (β·b / (ln 2 · a · s)) · N / n²,          bounded by K/8 above.                              (8.1)

The clusters read, m, cancel: **rows per group proportional to n²**, a group as large as the square of the dimension.
With b = 18.5 (the levels' rule) and s = 1: G* = 0.85·N/n² on yahoo-like data, 1.3·N/n² on LAION-like, 0.1·N/n² on glove-like; for exact
queries (b of 40 to 130 B, s ≈ 2) 1 to 3 N/n². β is a property of the data the build can compute from the fit as
`groups.py` does (cluster covariances in candidate bases), so (8.1) has no fitted constant.

| | 1M | 100M | 1B |
|---|---|---|---|
| today: min(64, K, N/(20n)) | 64 | 64 | 64 |
| (8.1) at N/(3n²), n = 128 / 384 / 512, capped at K/8 | 20 / 2 / 1 | 781 / 226 / 127 | 1,953 / 1,953 / 1,272 |
| proposed: max(64, min(N/(3n²), K/8)) | 64 | 781 / 226 / 127 | 1,953 / 1,953 / 1,272 |

- (8.1) says two things. At 1B, 64 groups are 20 to 30 times too few: the bases are stretched over 244 clusters.
  At 1M and n ≥ 384, 64 groups are more than the approximate path can amortise: their setups are half of the
  levels' bytes (5.2). I do not propose lowering G at 1M: the exact queries' timings chose it
  (`rust/REPORT_SCALING.md`), bytes are not time, and the floor of 64 keeps 1M exactly as it is. **Overhead at 1M:
  zero** (the rule returns 64 up to 3M rows at n = 128, 28M at n = 384, 50M at n = 512).
- Gain at 100M and 1B: not in the approximate path's bytes (its first level is 18.5 B a row whatever the basis; in
  the model the extra setups cost 1 to 6 MB under sparse sets), but in what the levels explain: by the proxy, the
  16 B level's distortion at 1B falls to what about 23 B would give with 64 groups on yahoo-like data, f rises by
  about 0.1, and the exact queries' intervals narrow by the same amount. This is the weakest number here: a proxy at
  1M. It needs one encode of a 10M+ fit at G = 64 and at (8.1) and a count of bytes per row.
- Storage at 1B: 1,272 to 1,953 bases, 0.03 GB (n = 128), 0.29 GB (384), 0.33 GB (512), under 1% of the codes.

### 8.5 Visible sets at large K

- **Per session.** `Vis2::new` builds a dense bit per row (125 MB at 1B), the sorted visible rows (4 B each) and a
  count per cluster. At 1B a cluster is about one Roaring container (2^16 rows): a visible set kept as a Roaring
  bitmap in row order gives each cluster's count, emptiness and rank from its container's header, and a leaf's from
  a popcount of 250 words. Rule: dense bits while N ≤ 2^27 or so (16 MB), row-ordered Roaring beyond. No query cost.
- **Per query, empty regions.** Clusters with no visible row are skipped by count today, but their centres are
  still scored in the scan. With counts per group (the sum of its clusters'), a region set that leaves most groups
  empty skips their centres, sigmas and setups in 64 to 2,000 comparisons; the sketch then scores only the live
  clusters' rows (a gather when under half are live). Overhead when nothing is empty: G comparisons.
- **Per query, nearly empty clusters.** The order already discounts them (z(1, v) of the visible count). The read
  rule is the planner's: a cluster or leaf goes through the levels when its visible rows pay for its tables (and its
  group, if new), else directly; a cluster is expanded into leaves when its visible rows pay for their centres.
  By the subsample law the sparse regime recedes with N: 1% visible is 20 rows a cluster at 1M, 160 at 100M, 640
  at 1B.
- **What stays weak**: a set correlated with the score inside clusters (`theory.md` 7.1). Leaves help it a little
  (a leaf's visible rows are closer to its centre than a cluster's); nothing here repairs it.

### 8.6 The design as a sequence of rules

Each rule is derived from N, K, n and counts the index holds; none is a timed or fitted constant; each is off, or
free, at 1M.

| | rule | switches on | overhead at 1M (counted bytes and operations) | gain at 100M | gain at 1B |
|---|---|---|---|---|---|
| 1 | levels at the statistical width ψρh, ρ from (4.2); two deterministic levels, then stored vectors; the seed by the first level | always, per cluster when its visible rows pay for its tables and group | none (a factor on a half-width); 128 numbers stored | bytes ÷ 11 to 31 unfiltered, ÷ 10 to 23 at 25%, ÷ 2.3 to 3.3 at 1% | ÷ 12 to 34, ÷ 11 to 29, ÷ 3.8 to 7 |
| 2 | leaves: clusters stored as 2,000-row runs with their own centre and directions | S > 2,000 (N > 2M); per query when a cluster's visible rows pay for its leaves' centres | zero (a cluster is one leaf) | a further ÷ 1.9 to 2.9 unfiltered, ÷ 1.3 to 2.2 at 25% | ÷ 3.3 to 5.5, ÷ 2.1 to 3.2 |
| 3 | the centres' sketch (32 int8 coordinates, then rule 4.1) | K ≥ 2,000 (N ≥ 16M) | zero (off) | centre scan ÷ 2 to 6; total under 1% sets ÷ 1.7 to 2.2 on yahoo and LAION, none on glove and sift | centre scan ÷ 4 to 10; total under 1% ÷ 1.1 to 1.9 |
| 4 | G = max(64, min(N/(3n²), K/8)), the factor from (8.1) and the fit | N/(3n²) > 64 (3M to 50M rows, by n) | zero (64) | tighter levels (proxy: about 2 to 4 B a row at 16 B on yahoo-like data); up to 1 MB more setups under sparse sets | about 5 to 7 B a row; 1 to 6 MB more setups |
| 5 | visible counts per group; row-ordered Roaring beyond 2^27 rows | G comparisons a query; the bitmap by N | G comparisons | empty groups cost nothing; 12 MB less per session | 125 MB less per session |

(The gains of rules 1 to 3 are the model's ranges over the four datasets at recall 0.95; rule 4's are the proxy's.)

**What the scaling becomes.** Rows read: N^(γ/2+η), 0.6 to 0.72, with rule 1 alone (a tenth to a thirtieth of the
bytes, the same exponent); N^η, 0.3 to 0.5 at recall 0.95 and 0.5 to 0.7 at 0.99, with rule 2. Centres scored in
full: K·(0.15 to 0.6) with rule 3, about 3m + 2.5m·S/2,000·(its sketch share) ∝ N^(η) in all. There is no N^(1/3) and
no polylogarithm on these data: with ideal spreads, an order built from centres and spreads reads N^η rows at a
fixed leaf size, and η is a property of the data. Fixed overhead per query at 1B with every rule: the sketch 0.5 MB,
3m full centres, the sigmas of the clusters and leaves read (1 to 7 MB), group setups 1 to 6 MB, tables 6 kB a
leaf.

## 9. Exact queries and visible sets

**Exact queries are untouched.** Nothing stored changes: the same rows in the same order, the same codes, bases,
support table and certified constants. The rule is a factor on the upper half-width the level kernel already takes
per level (`Prep2::dial`, 1 for every exact query). ψ = ∞ still is the exact query. Lower bounds stay certified, so
a row the levels take in is still a true candidate, and every returned item is scored from its stored vector as
now. The exact k-NN could later take its first threshold from this rule (a seed at ψ ≈ 4 in place of 2,000 rows at
16 B); that is an option, not part of the proposal, and it would not change any bound.

**Visible sets.**
- The rule sees only visible rows (the reader's candidates are the visible rows of the range), and ρ and the
  estimate are per row, so no filter pattern can bias them: a row's interval does not depend on which other rows are
  visible. A cluster's spread σ_c is the whole cluster's, as today; the rows' rule does not use it.
- Sparse sets: a cluster with few visible rows is read directly, by the planner's own comparison, exactly as
  today. Replay B at random 1%: the same bytes, rows and clusters as direct reads on all three datasets (ratio 1.0).
- Dense sets (25%): 2.3 to 7.4 times fewer bytes in replay B, 2.2 to 7 in the reader study, random and region alike.
- Region sets concentrate the visible rows in a few clusters, which are then read through the levels like
  unfiltered clusters; the centre scan and the stop are unchanged. Region 1%: 1.6 to 2.6 times fewer bytes.
- By the subsample law of section 2 a random 1% set at 1B has 640 visible rows a cluster, as a 32% set has at 1M:
  the sparse regime moves to smaller shares as N grows, and the levels' rule covers more of the filtered cases.

**What does change for a caller**: recall at a given ψ falls by 0 to 5 points at ψ = 4 to 5 (LAION text most) and by
0.2 or less at ψ ≥ 7 (section 5.4), so the default ψ would move from 4.5 to about 5; the bytes at a given recall are
what sections 5.2 and 5.4 report.

## 10. What it would take to build, and the risks

In order; each step is small and can be judged alone on the suite (recall against time, paired, as `PLAN.md` does).

1. **ρ at build**: (4.2) from `GroupPlan2`'s weights and the profile, per group and deterministic level, stored with
   the group (128 numbers). Test: against the measured rms on a sample of rows (within 5% in distribution; it is
   15% high for LAION's text queries, on the safe side).
2. **The reader**: on the index path of the approximate k-NN and threshold queries, the levels' factor is
   min(1, ψ·ρ_{g,l}) (per group, where `Prep2::dial` is per level today), and rows surviving the last deterministic
   level go to their stored vectors (no randomised level, so `group_q2r(.., rot = false)`: no rotations). The review
   of 15 Nov (F2) found the integer path applies the factor to the code term only; (4.2) is about that term, so this
   is the intended form, but the fixed terms then stay certified and should be checked to be small.
3. **The seed**: the first cluster through its first level, the best 2k + 8 estimates scored (today the k largest
   keys, which leaves about 1,000 rows of the first cluster to score: 5.4).
4. **The planner's constants**: `cut_row_us` is timed at certified widths, where most rows run every level. Under
   the rule a row costs the first level only; the constants must be timed with the factor in place, or the planner
   will keep choosing direct reads.
5. **Then, only if 1 to 4 show the time**: a lean first-level pass for the approximate path (estimate and one
   comparison per row, no per-row integer thresholds, no survivor lists until a row passes), and a cheaper
   per-cluster setup for two levels.
6. For scale (section 8.6; each off at 1M by its rule): leaves of 2,000 rows (build: split clusters above 2,000
   rows, a centre and directions per run; reader: expand by rule 4.1; first, time the exact queries on such a
   build, `--per-cluster 8000` at 1M with 4 runs); the centres' sketch from K = 2,000; G by (8.1), after one
   encode at 10M+ confirms the proxy; group visible counts and the row-ordered bitmap.

**Risks.**
- **Bytes are not time.** The library reads glove and yahoo directly today because the index path's per-row and
  per-cluster work costs more than the vectors it avoids (`subclusters.md`: 16 to 22 µs a cluster and 16 to 33 ns a
  row in range, against 16 to 61 ns a row read directly; `rust/REPORT.md`: 24 ns a row through 16 B). The rule removes
  the deeper levels for 93 to 99% of the rows, not the first level's cost. Expectation [estimated from those
  constants]: at 1M a gain of up to about 2 times on LAION and yahoo unfiltered, none on glove, until step 5; from
  10M to 100M, where a direct read of a cluster is megabytes and bound by memory bandwidth, the byte ratio is the
  time ratio.
- **A group's setup** (n² bytes, 56 to 106 µs measured elsewhere) is half of the rule's bytes at 1M on yahoo and
  LAION. It is amortised at scale (at most 64 groups) and the planner already prices it; a setup restricted to the
  coordinates the two levels code would cut it, and was not studied (`groups-setup.md` covers the exact case).
- **The tail.** The error's upper tail is heavier than normal out of distribution (99.9% at 3.6 to 3.9 spreads),
  and a level explains less there (f = 0.72 at 16 B on LAION text against 0.81 to 0.84). Both show as recall lost at
  a given ψ, most at ψ ≤ 5, and are inside the measured curves; a dataset whose 16 B level explains under half the
  variance would gain little (4.3) and would show it as a low share of rows dropped, which the build can report.
- **ρ under other queries.** (4.2) assumes the query's weights follow the profile. Text queries on LAION have a ρ
  15% below it (safe: more rows kept). A query distribution much more concentrated than the profile would have a
  larger ρ than derived (unsafe: recall lower than ψ suggests); `--query-sample` exists for that case and feeds the
  profile.
- **k.** At k = 100 the gains are 0.8 to 0.9 of those at k = 10 (more rows pass each level). k = 1,000 was not run.
- **Correlated filters and the cluster order.** A visible set aligned with the score inside clusters makes σ_c
  (whole cluster) wrong for the visible rows; that is today's weakness (`theory.md` 7.1) and the rule neither helps
  nor hurts it.
- **The replays' limits**: section 11.

## 11. Deviations and limits

- 100 queries per dataset in replay B and in the reader study; differences under about 10% are not findings.
  Replay A has 1,000 queries; its partitions are faiss k-means with 10 iterations on 128 rows a centre, not the
  library's, and its exponents come from 16 times in N (62k to 1M rows), extrapolated 1,000 times in section 8.
- Replay B reads the first M clusters of the library's order only (160 for glove, 64 to 96 for yahoo and LAION,
  96 to 256 under visible sets): its curves are cut off above the recall those hold (0.98 on glove unfiltered). The
  reader study has no such limit and is the reference for recall 0.98 and above.
- Replay B scales the whole certified half-width by ψρ; the kernel's integer path scales the code term (step 2
  above). The reader study uses the kernel as it is, with one ρ per level (the measured one) for every group.
- ρ "measured" in replay B comes from 20 of the 100 queries (the clusters ranked 24th and beyond).
- The bytes of tables (96 B per 4-bit unit) and of a group's setup (n²) are structural counts, not traced reads.
- sift was run in replay A only (no index was built for it); L2 enters there as the score q·x − |x|²/2.
- k = 1,000, AVX2 against AVX-512, and writes were not studied: the rule changes no stored byte and no kernel
  instruction, only a factor, so both instruction sets and inserts behave as today; ρ is per group and level and
  moves only when the build's weights do (a rebuild).
- No time was measured anywhere.

## Files

- `layered/scaling.py`, `scaling_tables.py`, `extrapolate.py`: replay A and its model; `results/scaling_*.json`,
  `results/scaling_tables.md`, `results/extrapolate.md`. `layered/nested.py`: replay A2 (leaves inside parents),
  `results/nested_*.md`. `layered/groups.py`: the groups proxy, `results/groups_*.md`.
- `rust/src/v2/e2e.rs`: `study_layered_dump` (the dump replay B reads) and `study_layered_kernel` (the library's
  reader with the factor; also prints ρ derived). `layered/dump.sh`, `kernel.sh` run them on an index built from the
  suite's fits with this tree's library.
- `layered/dump.py`, `explore.py`, `cascade.py`, `cascade_tables.py`, `summary.py`, `kernel_tables.py`: replay B;
  `results/cascade_*.json`, `results/cascade_tables.md`, `results/summary.md`, `results/explore_*.txt`,
  `results/kernel_*.txt`, `results/kernel_tables.md`.
- Scratch (deleted): `/home/user/scratch-layered` held the indexes rebuilt from the suite's fits (`vexact --v2 --index
  DIR --rebuild`, and `--dlv fit` for yahoo), the dumps, the test binary and the two ann-benchmarks files replay A
  took its sift rows and its glove and sift queries from (`sift-128-euclidean.hdf5`, `glove-100-angular.hdf5`).
