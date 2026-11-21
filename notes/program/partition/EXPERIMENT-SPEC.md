# Partition track: experiment on the 200k BioCLIP-2 sample

For an experimentalist, to run when the PI says the machine is free (not before about 10:05 on 21 November). It
compares the built partition with three candidates on scores computed from a partition and the data alone, with no
index build and nothing timed. The claims it tests are in `CLAIMS.md` (P1 to P5); each prediction below was written
before any of this was run.

## 0. Rules and inputs

- The standing rules of `../STATE.md`: one thread (`OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1`), `nice -n 10`,
  `taskset` to the core the PI names, under 100 MB resident per job, no git changes, no builds. Read the vectors in
  chunks of 10,000 rows (31 MB as float32); never hold N × n in memory. Write only under
  `notes/program/partition/work/`.
- Rows: `/home/joe/code/vexact-data/bioclip2-200k/g0.npy` to `g3.npy` (4 × 50,000 × 768, float16), normalised to unit
  length as `../stage0-block-norms/prep.py` does (`chunks()` and `rows()` there can be copied).
- Shared setup, already on disk: `../stage0-block-norms/work/setup.npz` (written by that `prep.py`, 32 s). Use from
  it: `Q` (369 held-out text prompts, 361 in-sample species prompts, 100 held-out item queries; `kind` 0, 1, 2), `TH`
  (exact thresholds at answers of 0.01, 0.1, 1 and 5%), `qi` (the item queries' rows), `Si` and `Sm` (the data-only
  and the mixed fit of Σ_Q), and its partition `a`, `Mean`, `grp` (K = 200, G = 12), which is **partition A0**.
- Σ_Q is held fixed for every partition: A = (Si + 0.1·tr(Si)/n·I)^½ (the ridge of `fit.py`). Repeat the coding scores
  once with `Sm` in place of `Si`. Holding Σ_Q fixed is a choice: the build would refit it to each partition's
  centres, and the difference is expected to be small (it is an average over 40 cells per query).
- Sizes: K = 200 and G = 12, so that A0 and its thresholds can be used as they are. This is 1,000 rows a cell, the
  earlier default; the rule's 2,000 (K = 100) is covered by the sweep of section 3.
- Fit and score halves: draw one permutation of the rows (seed 1). Rows at positions ≡ 0 mod 4 are **held out**;
  the rest are **fit rows**. Centres, groups and bases are fitted on fit rows only. The item queries' own rows are
  removed from every count. The cut's table (S2) and the keys (S3) use every row, because those bounds must hold
  for every row.

## 1. The partitions

| name | cells | groups |
|---|---|---|
| A0 | `setup.npz` (spherical k-means, 9 iterations, all rows) | `setup.npz` (k-means on the cell means) |
| A1, A2, A3 | the build's rule on the fit rows: `combined/core.py::kmeans` (12 iterations, seeds 1, 2, 3), every row to the centre of largest inner product, the centre replaced by the mean of its fit rows | `kmeans` on the unit centre directions, 20 iterations |
| A1-rand | A1's cells | a random grouping with A1's numbers of cells per group |
| A1-one, A1-cell | A1's cells | one group; a basis per cell with covariance 0.5·Σ_c + 0.5·Σ̄_g (g the cell's group in A1) |
| B0, B16 | A1's cells | the cell-level scheme (P3.1) from A1's groups; generator −F² (B0) and −D_λ at the 16 B level (B16) |
| C0, C16 | the row-level scheme (P3.5) from B16's cells and groups | the same scheme |
| D | balanced k-means: A1's centres, then 12 iterations in which rows are taken in decreasing order of (best inner product − second best) and each goes to its best centre that holds fewer than 1.1·N/K rows; centres are then the means | as A1 |

A1 to A3 exist to give the spread between seeds. **A difference smaller than twice the largest gap among A1, A2
and A3 is not a result.**

**A group's basis** (every partition): Σ̄_g = the fit rows' residual covariance in the group (residual against the
cell's mean of fit rows, float64); (γ, U) = eigh(A Σ̄_g A), sorted decreasing, γ floored at 10⁻¹² γ₁;
T_g = A U diag(γ^(−¼)), so that z = (x − m_c) @ T_g are the item-side dual coordinates, ‖z‖² = ‖x − m_c‖² in the M_g⁻¹ norm,
and the query side has variances √γ. F_g = Σᵢ √γᵢ.

**A level of B bytes**: H = 8B·ln 2 nats; θ_g solves Σᵢ ½ ln⁺(γᵢ/θ) = H (bisection); sᵢ = min(1, √(θ_g/γᵢ));
W_g(B) = Σᵢ sᵢ √γᵢ. A row's model width at the level is w_B(x) = √(W_g(B)·Σᵢ sᵢ zᵢ²). B = 0 gives s = 1 and w₀ = √F_g·‖z‖.

**B0 and B16.** Split each cell's fit rows into two halves by parity of position. Each pass: pooled covariances
and T_g, W_g, s from the first halves; cost(c, g) = Σ over the cell's second-half rows of w_B(x | c, g)² with B = 0 or
16; move each cell to its cheapest group if that is cheaper than its own by more than 10⁻⁶ of it; stop when no cell
moves or after 20 passes. Record the objective at every pass (it must not rise; if it rises by more than 0.1% the
cross-fitting is the cause and the run is reported as it is).

**C0 and C16.** Each pass: for every fit row and each of its 4 candidate cells (the 4 centres of largest inner
product, fixed from A1), cost w_B(x | c)² with the bases of the current groups; the row goes to the cheapest; then
means and bases are refitted. Stop when under 0.1% of rows move or after 15 passes; save the assignment after
every pass, one job per pass (about 4·N·n² multiply-adds, a minute). Held-out rows are assigned once at the end by
the same rule. **The first number to report, before iterating: after one assignment step from B16 with no refit,
the share of fit rows that change cell, the share that change group, and the mean of w₁₆ before and after.**

## 2. The scores (definitions in `CLAIMS.md` P5)

Coding, on held-out rows, mean over rows, for Σ_Q = `Si` and again for `Sm`:

- **S1a**: w₀(x), mean and root mean square.
- **S1d**: w_B(x) at B = 4, 8, 16, 32.
- **S1b**: the bytes at which w_B(x) = w* for w* = 0.03, 0.06 and 0.12 (bisection on θ per row, 0 if w₀(x) ≤ w*), mean
  over rows. This is the headline coding score.
- **S1e**: n·K/N + n²·G/N bytes a row.
- S1c of `CLAIMS.md` P5 (the diagonal model per cell) is optional; S1d measures the same thing row by row.

Zero-read decisions, per query kind and answer size (0.01, 0.1, 1%), mean share of rows over queries:

- **S2**: with E⁺[a, c] and E⁻[a, c] the largest and smallest a·x over all rows of cell c, a the unit centre
  directions; p̂ = the anchor of largest q·a, β = q·p̂, ρ = √(1 − β²); a cell is left when
  β·E⁻[p̂, c] − ρ < t ≤ β·E⁺[p̂, c] + ρ.
- **S3**: θ_x = arccos(x·m̂_c) for every row, φ_c = arccos(q·m̂_c), τ = arccos t; a row is left when
  |φ_c − τ| < θ_x ≤ φ_c + τ. Also, per cell, the largest over the root-mean-square of ‖x − m_c‖, and the 1st percentile
  of the key over its median.
- **S2+3**: rows left by both. Check every decided row against its exact score and report the violations (0
  expected; a positive count is an error in the script).

The order, per query kind:

- **S4a**: the variance over rows of q·m_c(x) divided by the variance over rows of q·x.
- **S4b**: cells in decreasing order of μ_c + 3σ_c(q), σ_c(q)² the variance of q·x over the cell's rows; the share of
  all rows in the cells visited until 9 of the 10 best rows are covered. And, for 2,000 sample rows, the share of
  each row's 10 nearest rows that lie in its own cell (the neighbour-graph objective, P4.2).

Layout:

- **S5**: smallest, median and largest n_c; the coefficient of variation; the share of rows in cells under 500 rows.
  If `/home/joe/code/vexact-data/bioclip2-text/meta_g*.parquet` hold a taxon for each row (I did not open them), the
  mean number of cells met by the rows of one family and of one order.

## 3. The sweep for P2 (partition A1's rule only)

K = 50, 100, 200, 400, 800 on the fit rows, seed 1, **one group** (so that only K moves). For each K: the mean square
residual; the spectrum γᵢ(K) of A W_P A; S1b and S1d on held-out rows; S2+3; S4a.

## 4. Predictions, and what would make me drop each claim or candidate

| | prediction (from `CLAIMS.md`) | dropped if |
|---|---|---|
| model U (P2.1) | ln γᵢ(K) falls by the same amount for every i: the slope in ln K lies in −0.08 to −0.14 for the first 300 coordinates | the slope over i ≤ 20 is more than twice the slope over 100 < i ≤ 300 (then it is model M, and P2.2's table overstates the saving at depth) |
| P2.2 | S1b falls by 0.8 to 2.4 B per doubling of K (more at the smaller w*) | outside 0.4 to 5 B at every w* |
| P4.7 | the largest radius of a cell is 1.2 to 1.5 times its rms radius | above 2 in most cells (then stragglers are worth a rule of their own) |
| the price of the built grouping (P3.4) | on S1d at 16 B: A1-one is 5 to 10% wider than A1; A1-rand lies between them; A1-cell is 0 to 6% narrower than A1 | A1-cell is not narrower than A1 on held-out rows: then there is nothing for B to reach, and B is dropped unrun |
| B (cells to bases) | S1b 1 to 3% lower than A1; S2, S3, S4 unchanged by construction | the gain in S1b at w* = 0.06 is under 1%, or under twice the spread of A1 to A3; or it is under 3% and B's groups are not neighbourhoods (the mean angle between a cell's centre and its group's centre rises by more than half) |
| C (rows to the narrowest interval) | no size predicted. Archer and Leen's figure would be S1d 12 to 19% narrower; I expect less, because the cells' centres have already taken the positions | fewer than 5% of rows change group in the first step; or S1b at w* = 0.06 gains under 3% at convergence; or the net figure below does not fall for both item and text queries |
| D (balanced) | S1b within 1% of A1; S3 up by 0 to 3%; no cell under 500 rows | S1b rises by more than 1%, or S2+3 by more than 3% (its gain in the layout is a few percent at most, so it cannot pay for either) |
| the neighbour-graph headroom (P4.2) | not predicted | reported only: if over 80% of a row's 10 nearest rows are already in its own cell for A1, a graph partition has little to add at this K |

**The net figure for C and D** (P5): for each query kind and answer size, (S2+3 share of rows) × (S1b bytes at
w* = 0.06), against A1's. For text queries S2+3 is near 1 and the net is the coding gain; for item queries it is
where C must pay for its lower keys.

## 5. What to hand back

One table per Σ_Q with a row per partition and a column per score; the three seeds' spread beside it; the sweep's
table with the fitted slopes; B's and C's objective per pass; the first-step numbers of C; the count of
violations. Scripts and logs under `notes/program/partition/work/`. State for every figure whether rows were held
out. A negative result on B or C is a result: it says the built grouping is at its limit on this data.

## 6. Limits known in advance

- 200,000 rows and 1,000 rows a cell: bases are fitted on about 12,500 rows a group (16n), fewer than the build's
  20n, and A1-cell's bases on 750 rows with shrinkage. Held-out scoring removes the optimism and leaves the noise.
- The level model (a coded coordinate's error is sᵢ times the coordinate) is the floor's allocation, not the built
  code; `../floors/CLAIMS.md` section 7 puts the built code 1.17 to 1.6 above the floor in width. Ratios between
  partitions are what the scores are for.
- One dataset. yahoo and LAION, where the measured slopes in G come from, are not covered.
- Nothing is timed, and the per-cell and per-group setups (P2.3, P3.4) are not in any score but S1e.
