# Partition track: notebook

Researcher: the partition theorist. Reports to the PI. Started 21 November 2026, 07:57.

The question: LIMINAL groups its rows by spherical k-means for filtering. Is that the right thing to group by, and what
are the alternatives? Claims go to `CLAIMS.md` as they are reached; this file is the dated log.

## 21 November 2026

Times are from the machine's clock (`date`); the first four entries were first written with guessed times and
corrected at 08:10.

- 07:57. Read `notes/README.md` (tags) and `notes/00-notation.md` (symbols). Directory created. Next: check what is
  built in the source (`rust/tools/fit.py`, `SYSTEM.md` sections 1 to 2), then the floor (`floors/CLAIMS.md`, "For the
  notes"), then the measured rules.
- about 08:01. What is built, checked in the source:
  - `rust/tools/fit.py` lines 201 to 223: `kmeans` (`combined/core.py` line 11: 12 Lloyd iterations of spherical
    k-means from random rows, on a 200,000-row sample; empty centres re-seeded), then every row to the centre of
    largest inner product. No balancing: measured sizes at 1M with 1,000 a cluster are 84 to 4,635
    (`rust/REPORT_CLUSTERSIZE.md`, "Fits"), with 4,000 a cluster 579 to 15,490. The stored centre is the mean of the
    assigned rows (not normalised), rounded to int8 with one scale per centre; the residual is against the stored centre.
  - Rows per cluster: `rules.py::per_cluster`, 2,000·√(N/1M) rounded to a power-of-two multiple of 2,000, so 2,000 up
    to 2M and 64,000 at 1B. (The brief said 1,000 to 4,000; 1,000 was the earlier default.)
  - Groups: `kmeans(Mh, G, rng, 20)` on the unit centre directions, G = min(64, K, N/(20n)). One dual basis per group.
  - Σ_Q is ONE matrix for the whole index (`sigma_q`: offsets q − (q·m̂)m̂ of 3,000 pseudo-queries from 40 clusters each,
    drawn by size, ridge 0.1). Only Σ_R is per group. So the dual metric differs between groups only through Σ_R.
  - Rows are laid out in (group, cluster) order; within a cluster by the 16-bit key x·m̂ (the Rust build sorts).
- about 08:03. Read the floor (`floors/CLAIMS.md` N.0 to N.6, F5, B4b to B4d), `groups/REPORT.md` (all),
  `rust/REPORT_CLUSTERSIZE.md` (all), `hierarchy/layered.md` section 8 (all). Numbers I will use:
  - rms residual 0.3765 / 0.3509 / 0.3288 / 0.3034 at K = 1,000 / 3,997 / 15,880 / 58,750 on BioCLIP-2 1M: the rms falls as
    K^(−0.053), so the mean square falls as K^(−0.106), which is K^(−2/D) with D ≈ 19 (`groups/REPORT.md` 3.2).
  - 16 times more cells save a text query 4.5 of 45 coordinates and cost 11.4 in centres (same table).
  - k-means in the M⁻¹ norm: coordinates read change by under 1%, the sorted range by under 1 point, the group ball by
    3 to 13 points (3.4).
  - Tightness against G: 8 times fewer bases widen the median certified half-width by 6% at 16 B (layered 8.8), so
    about 2% per doubling; code bytes 2.2% (LAION) to 3.7% (yahoo) per halving of G; the PCA proxy said 8 to 12%.
  - κ: rows per basis = κn², κ of 0.5 to 1.1 by counts and time (8.7 to 8.10); today's 0.06 to 0.07 at n = 384 to 512.
  - A group's setup: 60 to 130 µs, n² bytes. Cut 3 µs a cluster; an opened cluster 8 to 17 µs at 1,000 rows.
- 08:08. Read `approximate/CLAIMS.md` A1.7, `sublinear/REPORT.md` sections 4 to 5, `hierarchy/theory.md` sections 0 to 2,
  `bulk/REPORT.md` summary. Numbers: clusters explain f = 0.42 / 0.45 / 0.36 of the score variance (yahoo, LAION,
  glove), on the diagonal f = g down to the clusters; range and table leave 9 to 23% (items) and 69 to 83% (text) at
  k = 10 to 1,000; intrinsic dimension 16 to 22 by three estimators (items); rms residual at 1,000 rows a cluster falls
  as N^(−0.086) from 31k to 1M (so the exponent in K is 0.17 for the mean square at K = 31 to 1,000, against 0.106 at
  K = 10³ to 6·10⁴: the exponent drifts down as K grows).
- 08:09. Derivations before writing (kept here in short; full versions in CLAIMS.md):
  - Lagrangian form of the floor: H + λ·E h ≥ Σᵢ φ(λ²γᵢ), φ(u) = √u below 1 and 1 + ½ ln u above. φ is concave and
    increasing, so the cost of a cell is a concave, monotone spectral function of its covariance. k-means is the linear
    case. Splitting a cell gains a between-centres term (monotonicity) and a shape term (Jensen); k-means sees the first.
  - In singular values σᵢ = √γᵢ of the residual matrix in the query metric: k-means Σσᵢ² (Schatten 2), zero-read width
    Σσᵢ (Schatten 1), high rate Σ ln σᵢ (Schatten 0). The floor at a price interpolates 1 → 0.
  - Gradient of F at Σ is ½M⁻¹: k-means in the M⁻¹ norm is the linearisation of the zero-read width. That is the
    experiment of `groups/REPORT.md` 3.4, which found under 1%.
  - Two sticks example: coding prefers the flat cells exactly when w < 2h; k-means prefers the short cells when h < a/2.
  - Groups: Σ_c n_c ω_c(M_g)² is linear in Σ_c, so the best metric of a group is that of its pooled covariance (B4b's sum
    rule) and the alternating scheme is Bregman hard clustering of the cluster covariances with generator −F².
- 08:12. CLAIMS.md sections 0 and P1 written (P1.1 to P1.9). `check_p1.py` (log `check_p1.log`): the two-segment example
  reproduces the closed forms (crossing at w = 2h = 0.2; 2.00 bits a row for A at w = 0.05; 0.42 bits for B at
  w = 0.3); H* = D_λ − λw to 4·10⁻¹⁴ on 200 spectra; 0 violations in 300 random trials of
  D(full) ≥ D(pooled) ≥ diagonal model ≥ own basis; the gradient of F equals ½M⁻¹ by finite differences.
- 08:12. `check_p3.py` (log `check_p3.log`), synthetic, n = 16, 240 cells, 8 groups, price = objective over the
  one-basis-per-cell value: (i) shape follows position smoothly: one basis 1.044, random groups 1.043, by centre
  direction 1.023, Bregman from that 1.008 (8 steps); (ii) six kinds of shape independent of position: 1.401, 1.387,
  1.381, Bregman 1.128 (1 step; from a random start 1.105), although 8 groups could hold the 6 kinds at price 1: the
  scheme stops at a local minimum, as Lloyd's does; (iii) all cells alike: 1 everywhere. The objective never rose.
  A toy; it shows the mechanism and the local minimum, not a size for real data.
- 08:12. A wrong turn, recorded. I first argued that no partition into K cells can beat k-means for coding by more than
  log₂K bits a row. That is true against the best code for the true distribution (the partition is part of a code),
  and false in the currency the index pays, the second-moment code of a cell: there a finer partition can save
  (k_a/D) bits a row per doubling of K, more than one bit, because finer cells are closer to Gaussian. The measured
  uniform scaling of the residual (rms ∝ K^(−0.053), D ≈ 19, with about 50 to 170 coordinates above the water level)
  is of that kind. P2 is written with the second-moment currency and says so.
- 08:16. The PI sent the librarian's report (`../library/partition.md`, read in full) and asked that the claims be placed
  against it and that P3 be treated at the level of rows as well as cells. Consequences:
  - An error of mine found by reading `groups/scripts/prep_m.py`: the 3.4 experiment used ONE metric for the whole
    corpus (M⁻¹ of the covariance pooled over all cells), not each group's. P1.4 said it tested the within-group rule;
    it did not, and nothing has tested a row choosing its basis. P1.4 rewritten with the two cases apart.
  - Archer and Leen's regions are (mean, transform, allocation) triples, so in this index's terms their partition is
    rows to bases, with G of 16 to 32. Their 1.1 to 1.8 dB over k-means is a factor 0.81 to 0.88 in rms error at equal
    rate. That is the published size for the row-level scheme, on image blocks.
  - Ma et al.'s regularised length ½ log det(I + Σ/θ′) is Σ ½ ln(1 + γᵢ/θ′): linear below the level and logarithmic above.
    The floor's φ is a square root below and logarithmic above. So Ma's objective runs from k-means to the
    log-determinant and the certified width's from the nuclear norm to the log-determinant.
  - High-resolution theory does not apply at 0.5 bit per intrinsic dimension; my balance estimate from the point
    density formula is to be marked as outside its regime, and model U's exponent as a measured local slope.
  - The librarian's premise "about 1,000 rows a cluster" is `SYSTEM.md`'s; the code's default is 2,000·√(N/1M).
- 08:20. CLAIMS.md now has P2 (scaling), P3 (cell level P3.1 to P3.4, row level P3.5), P4 (alternatives and ranking), P5
  (scoring recipe), L (placement against the librarian), S (short answer), "What failed". `check_p3_rows.py`: toy
  with two kinds of residual in every blob; rms zero-read width 0.966 (cell level) and 0.932 (row level) of the
  built grouping, 25% of rows moved, k-means objective +4%, monotone in 31 steps. `check_p4.py`: the arithmetic of
  P2 and P4 (balance ratio 1.0058 at D = 19; λw = 124 to 365; S* by time 6,458 / 16,977 / 2,744; leaf sizes).
  Next: EXPERIMENT-SPEC.md.
- 08:23. EXPERIMENT-SPEC.md written: partitions A0 to A3 (built rule, three seeds for the noise), controls (random
  groups, one group, a basis per cell), B (cells to bases), C (rows to the narrowest interval), D (balanced); scores
  S1 to S5 on held-out rows; a sweep in K for P2; a prediction and a drop rule for each. It reuses
  `../stage0-block-norms/work/setup.npz` (K = 200, G = 12, queries, thresholds, Σ_Q). Nothing of it was run: no data
  file was read by this track.
- 08:23. State at hand-over. Done: P1 to P5, the placement against the librarian, the spec. Not done: no referee; the
  region-filter study in `vexact-sc` not read (P4.7); the per-cell head directions of P4.3 not specified; the row-level
  scheme's level-B cost is the floor's idealisation, not the built code; a neighbour-graph partition is scored only
  through its objective on the built cells. `git status` shows CLAIMS.md, NOTEBOOK.md and the first two check scripts
  as tracked, so someone committed them during the session; I ran no git command that changes anything.
