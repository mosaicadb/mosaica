# The approximate mode's cluster visit order

Why vexact's approximate k-NN loses recall to faiss IVF on LAION text queries, what order it should visit clusters in
instead, and how far that can be justified. These are offline replays and analysis: nothing here changes `rust/`.
The order lives in `rust/src/v2/q.rs` (`Approx`, `Approx::fit`, `pseudo_queries`); the proposals below are for its
owners.

- `theory.md`: the mathematical analysis (what is optimal, under which model; proofs, bounds, references), with
  synthetic checks in `theory/`. Its references were collected by an agent and have not each been checked.
- `results/`: the replay tables. `summary_table.txt` is the first replay at 1M (both fits); `loo_summary.txt` the
  candidate defaults on five datasets with leave-one-dataset-out choices; `table3.txt` the principled orders;
  `table4.txt` the calibrated spread, the ceilings, the collapse check and the stopping rule.
- `scripts/`: the replay scripts, as run (paths under a scratch directory `$S`; they expect the suite's prep, a
  `vexact_py` install and `rust/tools/fit.py`; see the comments at the top of each).

## Method

A replay ranks the clusters of a real `fit.py` fit under each order, for each query, and scores recall@10 against
the share of rows scanned (whole clusters, in order). Search inside a probed cluster is exact (eps 0), so recall is
decided by the order alone. The replay of today's order was checked against the real index's `knn_approx` on every
dataset (within 0.001), and the faiss counting against faiss's own search. Comparing at equal rows scanned, not equal
nprobe, matters: at nprobe 16 vexact (K = 500 at 1M) scans 2.1% of rows and faiss (nlist 4096) 0.4%.

Datasets, all with the suite's merged fit defaults (clusters and groups by N): LAION 1M with text queries (out of
distribution) and with 1,000 held-out image queries (in distribution), glove-100-angular, yahoo-minilm-384, VIBE
laion-clip-512 (out of distribution); 100 queries each, plus 1,000-query sets as a check. iNaturalist did not fit on
the test machine's disk. Differences below about 0.01 are within noise at 100 queries.

## Findings

1. **Today's order is the cause.** vexact ranks clusters by `q·m_c + f·rad_c` with f fitted on corpus rows as
   pseudo-queries (f = 0.5 on LAION). Text queries score about 0.3 against images, against 0.8 for images, so the
   real differences in `q·m_c` across clusters shrink (standard deviation 0.021 against 0.075) while `f·rad_c` does
   not, and the radius term decides the order. At 1.6% scanned on LAION text: today's order .48 (27 of 100 queries
   at recall 0), the same order with f = 0.1 (what `fit_queries` on text queries picks) .81, faiss at the same
   number of clusters .82, faiss with 4,096 clusters .93. The rest of the gap to faiss-4096 is the cluster count,
   and fewer, larger clusters lower recall at a given share (the move to 2,000 items per cluster cost about 0.07
   at 1.6%).
2. **The order must not depend on a shift of the query.** The true top-k is unchanged when a query is scaled or
   moved along a direction in which the corpus does not vary, which is what separates text from image queries
   (`theory.md`, Theorem 11). Today's order is not invariant; the candidates below are.
3. **Candidate defaults** (recall at 0.5 / 1.6 / 6.3 / 25% scanned; `loo_summary.txt`, `table3.txt`):

   | order | LAION text | LAION images | glove | yahoo | VIBE laion |
   |---|---|---|---|---|---|
   | today (f fitted on corpus rows) | .278 .481 .717 .921 | .783 .931 .989 .999 | .539 .750 .898 .984 | .645 .853 .966 .996 | .437 .638 .865 .974 |
   | **f′ = 0.4** | .629 .812 .938 .993 | .788 .931 .987 .999 | .542 .746 .900 .985 | .643 .852 .966 .996 | .538 .778 .935 .993 |
   | **H r4** | .669 .827 .951 .996 | .796 .935 .990 .999 | .555 .756 .907 .985 | .658 .860 .973 .997 | .565 .787 .950 .992 |
   | **H r16** | .681 .854 .961 .996 | .795 .935 .991 .999 | .559 .762 .908 .985 | .662 .878 .974 .997 | .594 .822 .956 .993 |
   | threshold order, r16 | .683 .856 .967 .997 | .768 .906 .979 .998 | .547 .760 .909 .982 | .654 .883 .971 .996 | .606 .828 .953 .994 |
   | threshold order, exact Σ (reference) | .705 .862 .968 .998 | .786 .929 .987 .999 | .550 .753 .909 .982 | .669 .884 .973 .997 | .618 .841 .958 .994 |
   | best f per dataset ("oracle") | .634 .813 .942 .993 | .783 .931 .989 .999 | .538 .750 .897 .981 | .654 .854 .973 .996 | .534 .780 .939 .991 |
   | faiss, nlist = K | .600 .821 .933 .994 | .779 .934 .991 .999 | .537 .739 .892 .979 | .625 .860 .976 .998 | .550 .761 .938 .992 |

   - **f′**: `q·m_c + f′·σ(q)·rad_c/σ(rad)`, σ(q) the standard deviation of `q·m_c` over clusters. No extra state.
     Worst case against the oracle +0.003 mean, +0.007 at 6.3%.
   - **H r16**: `q·m_c + 0.75·sqrt(qᵀΣ_c q)·z(10, n_c) + 0.3·σ(q)·rad_c/σ(rad)`, Σ_c as its top 16 principal
     directions plus an isotropic rest, z(k, n) = Φ⁻¹(1 − k/n). Never below the oracle (worst −0.003 mean, a gain).
     About 8 MB per 1M vectors (f16) and 4M extra multiply-adds per query, about half the work of scanning 1.6%.
   - **H r4**: the same with 4 directions and 0.5 in place of 0.75; never below the oracle (worst −0.002). A
     quarter of r16's state and work.
   - The constants of f′ and H were chosen by leave-one-dataset-out; they are validated, not derived.
4. **The threshold order is the principled form but needs an accurate spread.** Ranking by
   `ζ_c = (τ − μ_c)/σ_c` (τ the query's k-th best score, μ_c = q·m_c, σ_c² = qᵀΣ_c q) is the optimal order under a
   location-scale model of the scores in a cluster, whatever the tail shape (`theory.md`, Propositions 5 and 6).
   Measured: how τ is set barely matters (model, running and true τ within about 0.005), heavy tails do not help
   (glove's scores are Gaussian; a Student-t correction lowers recall), and with exact Σ the order needs no
   constants and matches or beats the oracle everywhere. With the deployable rank-16 Σ it loses on in-distribution
   images (.906 against .935 at 1.6%): the order needs σ_c to within a few percent (Theorem 12), and H's α < 1 and
   rad term act as shrinkage of the noisy estimate.
5. **A calibrated spread does not beat H.** Fitting `σ̃² = a·σ̂²_r + b·σ(q)²·rad² + c₀·σ(q)²` once per index on
   corpus pseudo-queries (plain and shifted to mimic text) and ranking by `μ + 3.3·σ̃` matches H r16 on average and
   loses up to 0.007 on LAION images (worst case against the oracle +0.003 against H r16's −0.003); calibrated r4
   loses to H r4 by up to 0.011. The reason is a model error, not noise: at the same ζ, clusters in the top rad
   tercile hold 1.6 to 3.8 times more rows above τ than the bottom tercile on four of five datasets, so the rate is
   not a function of ζ alone. H's rad term corrects for that. Every order built from (μ, σ) stays about the same
   distance from the clairvoyant ceiling (rank by true neighbours per row: .97 to .99 at 1.6%), which is information
   the mean and spread do not carry.
6. **Stop on a global threshold, not a fixed nprobe.** Thresholding the calibrated `1 − Φ(ζ)` across queries, at the
   same mean rows scanned, gains +0.017 to +0.043 recall at 0.5% scanned and up to +0.028 at 1.6%; fixed rows per
   query is worse than either. With it, the calibrated r16 order beats H r16 at fixed nprobe on four of five
   datasets at 1.6% (LAION text .886 against .854; glove ties).
7. **f′ estimated from the corpus** (within- against between-cluster spread) came out 2 to 3 times too low and is not
   usable as derived.

## Recommendation

Retire today's order. Offer f′ = 0.4 (no extra state), H r4 and H r16 as options, with `fit_queries` kept as an
override for users with a query sample. The principled successor to H (`theory.md`, section 9) replaces its
constants with a spread calibrated once per index; measured, it matches H r16 on average but not in the worst case,
because exceedance also depends on rad (finding 5). The clearer gain is the global stopping threshold (finding 6),
which needs the order's score on a probability scale. Being measured next: a shared basis for the spread (projected
once per query instead of K·r·d work and about 8 MB read), a rad-aware exceedance model, and the stopping rule on
top of H and f′.
