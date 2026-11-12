# Literature survey: which parts of the design are new

Saved by the coordinating session from the survey agent's returned text; the claim-by-claim section
is condensed, the related-work draft and bibliography are kept in full. Search queries used technical
terms only.

## Summary

**Mostly prior art, component by component.** The novelty is in the combination and in three or
four specific results, not in any headline mechanism.

- **Prior art:** progressive refinement of compressed approximations with guaranteed bounds for exact
  search (VA-file, IQ-tree, BOND, LEMP, FEXIPRO, ANSMET, PDX-BOND, Panorama); nested codes read
  incrementally under a randomised bound (Extended RaBitQ); stored norms with Cauchy–Schwarz (LEMP's
  INCR, L2AP, Panorama); exact k-NN by a rising threshold over bounds (VA-file, Fagin's NRA,
  Seidl–Kriegel, LEMP's Row-Top-k); the cluster-level steps (iDistance, M-tree, list of clusters,
  GNAT); pre-filtered exact search.
- **Partly new:** one stream with deterministic then randomised certificates (no paper found; the
  ingredients exist separately); weighted Cauchy–Schwarz on a quantised nested code with scalars
  charged in a rate bound; certificate weights fitted to a query/data mixture (QUIP and LeanVec-OOD
  already fit compression to queries); instance-optimality of prefix reading (the multi-level form of
  Seidl–Kriegel, technically routine); the leakage observation (post-filtering leakage is published
  for PostgreSQL and Elasticsearch, USENIX Security 2026; nothing peer-reviewed for approximate vector
  indexes).
- **Apparently new:** the floor on certified width per bit (Theorems 1–3): a Shannon-lower-bound
  argument with Urysohn's inequality replacing the second-moment step; no statement found, unrelated
  in technique to Alon–Klartag (needs a specialist check). And the combination: exact set-valued
  threshold search inside a per-request bitmap, every decision depending only on the item's own data.

**Must read:** *Panorama: Fast-Track Nearest Neighbors* (arXiv:2510.00566): energy-compacting
orthogonal basis, levels, stored per-level tail energies, Cauchy–Schwarz bounds, exact refinement,
level-major layout, IVF integration, out-of-distribution query analysis. Then Extended RaBitQ, then
LEMP (the same problem: all items with q·x ≥ θ, exactly).

**Concurrent:** arXiv:2611.07579 (5 November 2026), per-query probabilistic guarantees from exact
error quantiles of randomly rotated 1-bit codes; its guarantee model closely matches the randomised
levels (abstract only read).

## Claim by claim (condensed)

1. **Progressive multi-level refinement with guaranteed bounds — published.** VA-file [1] (one level),
   VA+-file [2], IQ-tree [3], BOND [4], Kashyap–Karras [5], LEMP [6], FEXIPRO [7], ANSMET [8]
   (most-significant bits first, deterministic), PDX-BOND [9], Panorama [10]; randomised: ADSampling
   [11], RaBitQ [12], Extended RaBitQ [13] (most significant bits equal the 1-bit code; read the rest
   only if undecided). New relative to these: a multi-level *quantised* code (more than two levels) in
   local PCA bases with a certified interval at every prefix, for exact threshold results — a
   combination.
2. **Deterministic early, randomised deep, one stream — apparently new as a combination.** The two
   lines are separate; PDX offers both as alternative modes.
3. **Stored error norm with weighted Cauchy–Schwarz; scalars as cost — partly published.** Stored
   remainder norms: LEMP INCR, L2AP [19], Panorama. RaBitQ counts its two stored floats. New: the
   weighting by query offsets, applying it to quantisation error of a nested code, outward-rounded
   log-grid scalars, and charging scalars in the rate bound.
4. **Floor on certified width per bit — apparently new.** Alon–Klartag [20] and Indyk–Wagner [21] are
   worst-case sketching results; Ordentlich–Polyanskiy [22] gives the rate–distortion function for
   mean squared error of Gaussian inner products — closest, but bounds MSE of an estimate, not a sound
   interval (the support function of the cell). Theorem 1 has the form of the Shannon lower bound with
   Urysohn replacing the second-moment step; present it as that. Theorem 2 (Gaussian isoperimetry for
   quantiser cells) not found. Okajima–Yoshida [23] and Xiao–Cao [24] address different questions.
5. **Instance-optimality of prefix reading — partly published.** Seidl–Kriegel [25], Kriegel et al.
   [26], Fagin–Lotem–Naor [27], Li et al. [28], LeJeune et al. [29]. Theorem 4 is the same
   indistinguishability argument applied per item to nested prefixes; present it as a generalisation.
6. **Fitting to a query distribution unlike the data — partly published.** ScaNN [30] (no query
   sample), QUIP [31], LeanVec-OOD [32], RoarGraph [33], Panorama's OOD analysis. New: a mixture of
   item and text queries applied to allocation and certificate weights of an exact method.
7. **Cluster-level steps — published; attributions corrected.** The sorted range is **iDistance**
   [34], not the list of clusters. Covering-radius pruning: M-tree [35], list of clusters [36], after
   Fukunaga–Narendra [37]; cosine annulus form: Schubert [38]. Per-cluster min/max tables: **GNAT**
   [39] (correct). LAESA [40] stores per-item pivot distances — cite for pivots only. Recent IVF
   versions: Tribase [41], TRIM [42], CLIP [43]. Possibly new and small: the concave-above /
   convex-below lemma giving one contiguous undecided range.
8. **Exact filtered search; leakage — partly published.** Filtered ANN [44–50] is approximate and
   treats pre-filter-then-exact-scan as the correct slow baseline. Espiritu–Cash [52] show
   post-filtering leaks through timing and relevance scores. New: exact threshold bitmaps where masking
   commutes with search. Claim result non-interference, not timing non-interference.
9. **Exact k-NN by rising threshold — published.** VA-file, NRA, Seidl–Kriegel, Kriegel et al., LEMP
   Row-Top-k (seed then threshold), Panorama, Kashyap–Karras.

**Anything doing most of this together?** No. Nearest: Panorama (deterministic, k-NN inside
approximate indexes, no filter, no threshold queries, no quantised nested code), Extended RaBitQ
(randomised, approximate), arXiv:2611.07579 (probabilistic per-query recall, 1-bit, no filter).

## Draft related-work section

See `DESIGN.md`, section 8 (added from this draft).

## Bibliography

Verification status noted where weaker than "primary source read".

1. R. Weber, H.-J. Schek, S. Blott. A Quantitative Analysis and Performance Study for Similarity-Search Methods in High-Dimensional Spaces. VLDB 1998, 194–205. https://www.vldb.org/conf/1998/p194.pdf
2. H. Ferhatosmanoglu, E. Tuncel, D. Agrawal, A. El Abbadi. Vector Approximation Based Indexing for Non-uniform High Dimensional Data Sets. CIKM 2000. https://www.cs.ucsb.edu/research/tech-reports/2000-10 (abstract and secondary description only)
3. S. Berchtold, C. Böhm, H. V. Jagadish, H.-P. Kriegel, J. Sander. Independent Quantization: An Index Compression Technique for High-Dimensional Data Spaces. ICDE 2000. https://typeset.io/pdf/independent-quantization-an-index-compression-technique-for-4ly7pc5w63.pdf
4. A. de Vries, N. Mamoulis, N. Nes, M. Kersten. Efficient k-NN Search on Vertically Decomposed Data. SIGMOD 2002, 322–333. https://www.cs.uoi.gr/~nikos/sigmod02.pdf
5. S. Kashyap, P. Karras. Scalable kNN Search on Vertically Stored Time Series. KDD 2011, 1334–1342. doi:10.1145/2020408.2020607 (bibliographic record only)
6. C. Teflioudi, R. Gemulla, O. Mykytiuk. LEMP: Fast Retrieval of Large Entries in a Matrix Product. SIGMOD 2015. https://madoc.bib.uni-mannheim.de/40018/1/teflioudi15lemp.pdf
7. H. Li, T. N. Chan, M. L. Yiu, N. Mamoulis. FEXIPRO: Fast and Exact Inner Product Retrieval in Recommender Systems. SIGMOD 2017. https://www.comp.polyu.edu.hk/~csmlyiu/conf/SIGMOD17_fexipro.pdf (search summary only)
8. Y. Li, Y. Jin, B. Tian, H. Zhang, M. Gao. ANSMET: Approximate Nearest Neighbor Search with Near-Memory Processing and Hybrid Early Termination. ISCA 2025. https://people.iiis.tsinghua.edu.cn/~gaomy/pubs/ansmet.isca25.pdf
9. L. Kuffo, E. Krippner, P. Boncz. PDX: A Data Layout for Vector Similarity Search. Proc. ACM Manag. Data 3(3), Article 196, 2025. https://arxiv.org/abs/2503.04422
10. V. Ramani, A. Schlomer, A. Nayar, S. Ranu, J. M. Patel, P. Karras. Panorama: Fast-Track Nearest Neighbors. arXiv:2510.00566, 2025. https://arxiv.org/abs/2510.00566
11. J. Gao, C. Long. High-Dimensional Approximate Nearest Neighbor Search: with Reliable and Efficient Distance Comparison Operations. Proc. ACM Manag. Data 1(2), Article 137, 2023. https://arxiv.org/abs/2303.09855
12. J. Gao, C. Long. RaBitQ: Quantizing High-Dimensional Vectors with a Theoretical Error Bound for Approximate Nearest Neighbor Search. SIGMOD 2024. https://arxiv.org/abs/2405.12497
13. J. Gao, Y. Gou, Y. Xu, Y. Yang, C. Long, R. C.-W. Wong. Practical and Asymptotically Optimal Quantization of High-Dimensional Vectors in Euclidean Space for Approximate Nearest Neighbor Search. arXiv:2409.09913, 2024 (SIGMOD 2025 per secondary sources). https://arxiv.org/abs/2409.09913
14. M. Yang, L. Jing, W. Li, W. Wang. Quantization Meets Projection: A Happy Marriage for Approximate k-Nearest Neighbor Search. PVLDB 19. https://arxiv.org/abs/2411.06158 (abstract only)
15. H. Li, S. Deng, X. Yan, X. Zhi, J. Cheng. SAQ: Pushing the Limits of Vector Quantization through Code Adjustment and Dimension Segmentation. arXiv:2509.12086. https://arxiv.org/abs/2509.12086 (abstract only)
16. M. Yang et al. Effective and General Distance Computation for Approximate Nearest Neighbor Search. arXiv:2404.16322 (ICDE 2025 per search results). https://arxiv.org/abs/2404.16322
17. Deng et al. Efficient Data-aware Distance Comparison Operations for High-Dimensional Approximate Nearest Neighbor Search (DADE). arXiv:2411.17229. https://arxiv.org/abs/2411.17229 (venue and authors not confirmed)
18. W.-K. Loh, S. Lee. Per-Query Recall Guarantees for Approximate Nearest Neighbor Search in Dynamic Vector Databases. arXiv:2611.07579, November 2026. https://arxiv.org/abs/2611.07579 (abstract only)
19. D. C. Anastasiu, G. Karypis. L2AP: Fast Cosine Similarity Search with Prefix L-2 Norm Bounds. ICDE 2014, 784–795. doi:10.1109/ICDE.2014.6816700 (abstract only)
20. N. Alon, B. Klartag. Optimal Compression of Approximate Inner Products and Dimension Reduction. FOCS 2017. https://arxiv.org/abs/1610.00239 (explicit formula seen only in secondary slides)
21. P. Indyk, T. Wagner. Near-Optimal (Euclidean) Metric Compression. SODA 2017. https://arxiv.org/abs/1609.06295
22. O. Ordentlich, Y. Polyanskiy. Optimal Quantization for Matrix Multiplication. arXiv:2410.13780 (IEEE Trans. Inf. Theory 2026 per search results). https://arxiv.org/abs/2410.13780
23. K. Okajima, T. Yoshida. What Limits Does Quantization Place on Dense Top-k Retrieval? A Theoretical Study. arXiv:2606.11780. https://arxiv.org/abs/2606.11780 (abstract only)
24. W. Xiao, X. Cao. When Does Low-Bit Quantization Preserve the Decisions of Vector Search? arXiv:2609.09854. https://arxiv.org/abs/2609.09854 (abstract only)
25. T. Seidl, H.-P. Kriegel. Optimal Multi-Step k-Nearest Neighbor Search. SIGMOD 1998, 154–165. https://www.dbs.ifi.lmu.de/Publikationen/Papers/SIGMOD98.pdf (abstract only)
26. H.-P. Kriegel, P. Kröger, P. Kunath, M. Renz. Generalizing the Optimality of Multi-step k-Nearest Neighbor Query Processing. SSTD 2007, LNCS 4605, 75–92. https://rd.springer.com/chapter/10.1007/978-3-540-73540-3_5 (abstract only)
27. R. Fagin, A. Lotem, M. Naor. Optimal Aggregation Algorithms for Middleware. J. Comput. Syst. Sci. 66(4):614–656, 2003 (PODS 2001). https://arxiv.org/abs/cs/0204046
28. Y. Li, J. Wang, B. Pullman, N. Bandeira, Y. Papakonstantinou. Index-Based, High-Dimensional, Cosine Threshold Querying with Optimality Guarantees. ICDT 2019, LIPIcs 127, 11:1–11:20. https://arxiv.org/abs/1812.07695
29. D. LeJeune, R. G. Baraniuk, R. Heckel. Adaptive Estimation for Approximate k-Nearest-Neighbor Computations. AISTATS 2019, PMLR 89:3099–3107. https://proceedings.mlr.press/v89/lejeune19a.html
30. R. Guo et al. Accelerating Large-Scale Inference with Anisotropic Vector Quantization. ICML 2020. https://research.google/blog/announcing-scann-efficient-vector-similarity-search/ (author list not verified)
31. R. Guo, S. Kumar, K. Choromanski, D. Simcha. Quantization Based Fast Inner Product Search. AISTATS 2016. https://research.google/pubs/quantization-based-fast-inner-product-search-2
32. M. Tepper et al. LeanVec: Searching Vectors Faster by Making Them Fit. arXiv:2312.16335. https://arxiv.org/abs/2312.16335 (venue and authors not verified)
33. RoarGraph: A Projected Bipartite Graph for Efficient Cross-Modal Approximate Nearest Neighbor Search. PVLDB 17, 2024. https://arxiv.org/abs/2408.08933 (authors not verified)
34. H. V. Jagadish, B. C. Ooi, K.-L. Tan, C. Yu, R. Zhang. iDistance: An Adaptive B+-tree Based Indexing Method for Nearest Neighbor Search. ACM TODS 30(2):364–397, 2005. doi:10.1145/1071610.1071612
35. P. Ciaccia, M. Patella, P. Zezula. M-tree: An Efficient Access Method for Similarity Search in Metric Spaces. VLDB 1997, 426–435. https://vldb.org/conf/1997/P426.PDF
36. E. Chávez, G. Navarro. A Compact Space Decomposition for Effective Metric Indexing. Pattern Recognition Letters 26(9), 2005. https://users.dcc.uchile.cl/~gnavarro/ps/prl04.pdf
37. K. Fukunaga, P. M. Narendra. A Branch and Bound Algorithm for Computing k-Nearest Neighbors. IEEE Trans. Computers C-24(7):750–753, 1975. doi:10.1109/T-C.1975.224297 (citation only)
38. E. Schubert. A Triangle Inequality for Cosine Similarity. SISAP 2021, LNCS 13058, 32–44. https://arxiv.org/abs/2107.04071
39. S. Brin. Near Neighbor Search in Large Metric Spaces. VLDB 1995, 574–584. https://www.vldb.org/conf/1995/P574.PDF
40. M. L. Micó, J. Oncina, E. Vidal. A New Version of the Nearest-Neighbour Approximating and Eliminating Search Algorithm (AESA) with Linear Preprocessing Time and Memory Requirements. Pattern Recognition Letters 15:9–17, 1994. doi:10.1016/0167-8655(94)90095-7
41. Q. Xu, J. Yang, F. Zhang et al. Tribase: A Vector Data Query Engine for Reliable and Lossless Pruning Compression using Triangle Inequalities. Proc. ACM Manag. Data 3(1), Article 82, 2025. doi:10.1145/3709743
42. TRIM: Accelerating High-Dimensional Vector Similarity Search with Enhanced Triangle-Inequality-Based Pruning. arXiv:2508.17828. https://arxiv.org/abs/2508.17828 (title only)
43. CLIP: Lightweight Cosine-Law-Based Inverted-List Pruning for IVF-Based Vector Search. arXiv:2606.29968. https://arxiv.org/abs/2606.29968 (title and snippet only)
44. S. Gollapudi, N. Karia, A. Singh, H. V. Simhadri et al. Filtered-DiskANN: Graph Algorithms for Approximate Nearest Neighbor Search with Filters. WWW 2023, 3406–3416.
45. L. Patel, P. Kraft, C. Guestrin, M. Zaharia. ACORN: Performant and Predicate-Agnostic Search over Vector Embeddings and Structured Data. Proc. ACM Manag. Data 2(3), 2024. https://arxiv.org/abs/2403.04871
46. M. Wang, L. Lv, X. Xu, Y. Wang, Q. Yue, J. Ni. An Efficient and Robust Framework for Approximate Nearest Neighbor Search with Attribute Constraint (NHQ). NeurIPS 2023. https://proceedings.neurips.cc/paper_files/paper/2023/hash/32e41d6b0a51a63a9a90697da19d235d-Abstract.html
47. G. Gupta, J. Yi, B. Coleman, C. Luo, V. Lakshman, A. Shrivastava. CAPS: A Practical Partition Index for Filtered Similarity Search. arXiv:2308.15014. https://arxiv.org/abs/2308.15014
48. Curator: Efficient Indexing for Multi-Tenant Vector Databases. arXiv:2401.07119. https://arxiv.org/abs/2401.07119 (title only)
49. HoneyBee: Efficient Role-based Access Control for Vector Databases via Dynamic Partitioning. arXiv:2505.01538. https://arxiv.org/abs/2505.01538 (title and secondary description only)
50. Han, Chakraborty, Mehrotra. Don't Stir the Pot! Authorized Vector Data Retrieval via Access-Aware Indexing. arXiv:2605.01342. https://arxiv.org/abs/2605.01342 (secondary description only)
51. L. S. Yalamarthi, P. Pappachan. Policy-aware Vector Search: A Vision for Fine Grained Access Control in Vector Databases. SeQureDB 2026. https://arxiv.org/abs/2606.19803 (abstract only)
52. Z. Espiritu, D. Cash. Plaintext Recovery Against Post-Filtering Access Control. USENIX Security 2026, 3753–3772. https://arxiv.org/abs/2608.11730 (abstract only)

Also relevant (abstract only): P. Ciaccia, M. Patella, PAC Nearest Neighbor Queries, ICDE 2000;
K. Echihabi et al., ProS, VLDB Journal 2022 (https://arxiv.org/abs/2212.13310); W. Equitz, T. Cover,
Successive Refinement of Information, IEEE Trans. Inf. Theory 37(2), 1991.

Cited from memory, not checked: the Shannon lower bound; Urysohn's inequality; partial distance
search (Bei and Gray, IEEE Trans. Commun. 1985); Korn et al., VLDB 1996; Yu et al., VLDB 2001
(iDistance conference version).
