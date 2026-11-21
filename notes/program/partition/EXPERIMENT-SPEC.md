# Partition track: experiment on the 200k BioCLIP-2 sample

For an experimentalist, to run when the PI says the machine is free. It scores the built partition and three
candidates from a partition and the data alone, with no index build and nothing timed. The claims it tests are in
`CLAIMS.md` (P1 to P5). This is the version revised after referee 1 (`referee/referee-1.md`); the earlier one is
`EXPERIMENT-SPEC_before_referee.md`, and section 7 lists what changed. Every prediction below was written before
any of this was run. Nothing here has been run.

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
  centres.
- Sizes: K = 200 and G = 12, so that A0 and its thresholds can be used as they are. This is 1,000 rows a cell, the
  earlier default; the rule's 2,000 (K = 100) is covered by the sweep of section 4.
- Fit and score halves: draw one permutation of the rows (seed 1). Rows at positions ≡ 0 mod 4 are **held out**;
  the rest are **fit rows**. Centres, groups and bases are fitted on fit rows only. The item queries' own rows are
  removed from every count. The cut's table (S2) and the keys (S3) use every row, because those bounds must hold
  for every row.

## 1. The partitions

| name | cells | groups |
|---|---|---|
| A0 | `setup.npz` (spherical k-means, 9 passes on all rows, the last assignment kept and the cell means stored) | `setup.npz` (k-means on the cell means) |
| A1, A2, A3 | the build's steps, at the build's ratio of 200 sample rows a centre: `combined/core.py::kmeans` (12 passes, seeds 1, 2, 3) on a random 40,000 of the fit rows; every row to the sample centroid of largest inner product; the stored centre is the mean of the cell's fit rows | `kmeans` on the unit centre directions, 20 passes |
| A1-fp | A1 continued to a fixed point (section 3, first item) | the same rule on the new centres |
| A1-rand | the baseline's cells | a random grouping with the baseline's numbers of cells per group |
| A1-one, A1-cell | the baseline's cells | one group; a basis per cell with covariance 0.5·Σ_c + 0.5·Σ̄_g (g the cell's group) |
| B0, B16 | the baseline's cells | the cell-level scheme (P3.1, P3.2) from the baseline's groups, with the zero-read cost (B0) and the 16 B level cost (B16) |
| C16, C0, Cmix, C16-in | the row-level scheme (P3.5) from B16's cells and groups | the same scheme |
| D | balanced k-means: the baseline's centres, then 12 passes in which rows are taken in decreasing order of (best inner product − second best) and each goes to its best centre that holds fewer than 1.1·N/K rows; centres are then the means | as A1 |

**The baseline** is A1, or A1-fp if section 3's first item says the two differ (its rule is there). B, C and D
start from the baseline, so that none of them is credited with finishing Lloyd's iteration.

A1 to A3 give the spread between seeds. **A difference smaller than twice the largest gap among A1, A2 and A3 is
not a result.**

**A group's basis** (every partition): Σ̄_g = the fit rows' residual covariance in the group (residual against the
cell's mean of fit rows, float64); (γ, U) = eigh(A Σ̄_g A), sorted decreasing, γ floored at 10⁻¹² γ₁;
T_g = A U diag(γ^(−¼)), so that z = (x − m_c) @ T_g are the item-side dual coordinates, ‖z‖² = ‖x − m_c‖² in the M_g⁻¹ norm,
and the query side has variances √γ. F_g = Σᵢ √γᵢ. **No group may fall below 3n = 2,304 fit rows** in any scheme: a
move that would do so is refused (P3.1 needs every pooled covariance positive definite at every step).

**A level of B bytes**: H = 8B·ln 2 nats; θ_g solves Σᵢ ½ ln⁺(γᵢ/θ) = H (bisection); sᵢ = min(1, √(θ_g/γᵢ));
W_g(B) = Σᵢ sᵢ √γᵢ. A row's model width at the level is w_B(x) = √(W_g(B)·Σᵢ sᵢ zᵢ²). B = 0 gives s = 1 and w₀ = √F_g·‖z‖.
This is the floor's idealisation (a coded coordinate's error is sᵢ times the coordinate), not the built code.

**B0 and B16.** Two variants, run side by side, because only the first is the scheme the theorem covers.
- *In-sample* (the theorem's): each pass, pooled covariances and T_g, W_g, s from all fit rows; cost(c, g) = the sum
  over the cell's fit rows of w_B(x | c, g)², B = 0 or 16; each cell to its cheapest group if cheaper than its own by
  more than 10⁻⁶ of it (a cell keeps its group on a tie); stop when no cell moves or after 20 passes. The objective
  on fit rows must not rise; a rise is an error in the script.
- *Cross-fitted*: split each cell's fit rows in two by parity of position; pooled covariances from the first
  halves, costs from the second. This removes a cell's bias towards its own group and has no descent guarantee.
- For both, record at every pass the objective on fit rows and the same sum on held-out rows.
- Then one pass of single-cell moves with refitting: cells in random order (seed 1); the candidate is the cell's
  second-cheapest group under the current metrics; the move is kept if Σ_g N_g W_g(B)² falls with both groups'
  bases refitted (about 400 eigendecompositions). Report how many moves were kept and the gain.
- Report the mean angle between a cell's centre direction and its group's mean centre direction, before and
  after.

**C.** Each pass: for every fit row and each candidate cell, the cost with the bases of the current groups; the
row goes to the cheapest, keeping its cell on a tie; a cell left with under 32 fit rows is dropped and its rows
reassigned; then means and bases are refitted. Stop when under 0.1% of rows move or after 15 passes; save the
assignment after every pass, one job per pass (about 4·N·n² multiply-adds). Held-out rows are assigned once at the
end by the same rule. Candidates are the 4 centres of largest inner product, fixed from the baseline.
- **C16**: cost w₁₆². This is the rule the track proposes.
- **C0**: cost w₀². The contrast: the zero-read rule, whose descent is the proved one and whose gain may not carry.
- **Cmix**: the sum over B = 4, 8, 16, 32 of w_B², each divided by its mean over the fit rows at the baseline.
- **C16-in**: C16 with a row's candidates restricted to cells of its own group. It isolates the within-group
  flattening of P1.4 from the change of basis.
- **The first numbers to report, before iterating**: after one assignment step from B16 with no refit, for each of
  the four rules, the share of fit rows that change cell, the share that change group, and the mean of w₁₆ on
  held-out rows before and after. Also the share of rows whose cheapest cell differs between C0 and C16.

## 2. The scores (definitions in `CLAIMS.md` P5)

Coding, on held-out rows, mean over rows, for Σ_Q = `Si` and again for `Sm`. **The headline is S1d at 16 B and S1b.**

- **S1d**: w_B(x) at B = 4, 8, 16, 32.
- **S1b**: the bytes at which w_B(x) = w* for w* = 0.03, 0.06 and 0.12 (bisection on θ per row, 0 if w₀(x) ≤ w*).
- **S1a** (secondary): w₀(x), mean and root mean square. A gain here that does not appear in S1d and S1b is
  reported as such; `groups/REPORT.md` 3.4 is the precedent.
- **S1e**: n·K/N + n²·G/N bytes a row.

Zero-read decisions, per query kind and answer size (0.01, 0.1, 1%), mean share of rows over queries:

- **S2**: with E⁺[a, c] and E⁻[a, c] the largest and smallest a·x over all rows of cell c, a the unit centre
  directions; p̂ = the anchor of largest q·a, β = q·p̂ (the swap for β < 0 handled), ρ = √(1 − β²); a cell is left when
  β·E⁻[p̂, c] − ρ < t ≤ β·E⁺[p̂, c] + ρ. Report also the mean of 2ρ and of β·(E⁺ − E⁻) over the cells left, so that the
  net and the extent are seen apart (P1.5).
- **S3**: θ_x = arccos(x·m̂_c) for every row, φ_c = arccos(q·m̂_c), τ = arccos t; a row is left when
  |φ_c − τ| ≤ θ_x ≤ φ_c + τ (lower bound clamped at −1 where φ_c + θ_x > π). Also, per cell, the largest over the
  root-mean-square of ‖x − m_c‖, and the 1st percentile of the key over its median.
- **S2+3**: rows left by both. Check every decided row against its exact score and report the violations (0
  expected; a positive count is an error in the script).

The order, per query kind:

- **S4a**: the mean over queries of the share (variance over rows of q·m_c(x) over the variance of q·x), and beside
  it the ratio of the two means (the quantity P1.7's identity is about).
- **S4b**: cells in decreasing order of μ_c + 3σ_c(q), σ_c(q)² the variance of q·x over the cell's rows; the share of
  all rows in the cells visited until 9 of the 10 best rows are covered. And, for 2,000 sample rows, the share of
  each row's 10 nearest rows that lie in its own cell (the neighbour-graph objective, P4.2).

Layout:

- **S5**: smallest, median and largest n_c; the coefficient of variation; the share of rows in cells under 500 rows.
  If `/home/joe/code/vexact-data/bioclip2-text/meta_g*.parquet` hold a taxon for each row (I did not open them), the
  mean number of cells met by the rows of one family and of one order.

## 3. What is run, in order

1. **How far the built partition is from a fixed point** (P1.6; the cheapest item, and it sets the baseline). For
   A1, A2, A3 and A0: the share of rows whose largest key over the stored centre directions is not at their own
   cell (**S0**). Then continue Lloyd on all fit rows from A1: every row to the stored centre of largest inner
   product, centres to the means; after 1, 2, 5, 10 and 20 passes (stop when under 0.1% of rows move) report the
   share moved, the mean key, the mean square residual, S2, S3, S2+3, S1d, S1b and S4a, with the groups re-made by
   the build's rule each time. The last is A1-fp. **Rule for the baseline**: if S0 is under 2% for A1 and S2+3 and
   S1b at A1-fp are within twice the seed spread of A1's, the baseline is A1; otherwise it is A1-fp, and the
   difference between A1 and A1-fp is itself a result (what finishing the fit is worth).
2. **The noise**: every score for A1, A2, A3.
3. **The price of the built grouping**: A1-rand, A1-one, A1-cell against the baseline.
4. **B0 and B16**, both variants, then the pass of single moves.
5. **C**: the first-step numbers for the four rules; then C16 to its stopping point; then C16-in, Cmix and C0 if
   C16's first step passes its rule below.
6. **D**.
7. **The sweep** (section 4).

## 4. The sweep for P2 (the build's rule only, one group so that only K moves)

- On the fit rows (150,000): K = 50, 100, 200, 400, 800, seed 1, each continued to its fixed point as in item 1.
- On a random 37,500 of the fit rows: K = 50, 100, 200. With the first series this gives the same K at two corpus
  sizes and the same rows per cell at two values of K, which is what separates model U's assumption (b) (a cell's
  covariance depends on K and not on N) from the exponent falling with K (P2.1).
- For each: the mean square residual; the spectrum γᵢ of A W_P A; the mean over cells of each cell's own dual
  spectrum sorted (to test assumption (d), a group's spectrum for a cell's); S1b and S1d on held-out rows; S2+3 and
  the items entering (so that bytes per entering item can be formed); S4a.

## 5. Predictions, and what would make me drop each claim or candidate

| | prediction (from `CLAIMS.md`) | dropped if |
|---|---|---|
| the fixed point (P1.6) | no size from theory. The referee's toy had S0 = 11.7% and a mean key 4.7% higher at the fixed point | reported, not dropped. If S0 is under 2% the remark in P1.6 has no practical weight on this corpus |
| model U, (a) | ln γᵢ(K) falls by the same amount for every i: the slope in ln K lies in −0.08 to −0.14 for the first 300 coordinates | the slope over i ≤ 20 is more than twice the slope over 100 < i ≤ 300 (then it is model M, and P2.2's table overstates the fall at depth) |
| model U, (b) | at equal K the spectra at 150,000 and 37,500 rows agree to within the seed spread | they differ by more than the change from one doubling of K (then the cell's covariance depends on N, and P2's "with N" paragraphs fall) |
| P2.2 | S1b falls by 0.8 to 2.4 B per doubling of K (more at the smaller w*) | outside 0.4 to 5 B at every w* |
| P4.7 | the largest radius of a cell is 1.2 to 1.5 times its rms radius | above 2 in most cells (then stragglers are worth a rule of their own) |
| the price of the built grouping (P3.4) | on S1d at 16 B: A1-one is 5 to 25% wider than the baseline (the range of the two measured slopes over log₂12 doublings); A1-rand lies between; A1-cell is narrower than the baseline | A1-cell is not narrower than the baseline on held-out rows: then there is nothing for B to reach at this size, and B is reported as run with that reading |
| B (cells to bases) | S1b 1 to 6% lower than the baseline; S2, S3, S4 unchanged by construction | the gain in S1b at w* = 0.06 is under 1%, or under twice the seed spread; or it is under 3% and the mean angle between a cell's centre and its group's rises by more than half. Also: if B0 gains in S1a and not in S1d or S1b, the zero-read scheme is dropped and only B16 kept |
| C16 (rows to the narrowest interval at 16 B) | no size predicted. Archer and Leen's figure would be S1d 12 to 19% narrower; I expect less, because the cells' centres have already taken the positions | fewer than 5% of rows change cell in the first step; or S1b at w* = 0.06 gains under 3% at its stopping point; or the net figure below does not fall for both item and text queries |
| C16-in against C16 | no size predicted | reported: it says how much of C16's gain is the within-group flattening (P1.4) and how much the change of basis |
| C0 against C16 | C0 gains more in S1a and less in S1d and S1b than C16 | if C0 does as well as C16 in S1b, the zero-read rule suffices and the proxy warning does not bite here |
| D (balanced) | S1b within 1% of the baseline; S3 up by 0 to 3%; no cell under 500 rows | S1b rises by more than 1%, or S2+3 by more than 3% (its gain in the layout is a few percent at most, so it cannot pay for either) |
| the neighbour-graph headroom (P4.2) | not predicted | reported only: if over 80% of a row's 10 nearest rows are already in its own cell for the baseline, a graph partition has little to add at this K |

**The net figure for C and D** (P5): for each query kind and answer size, (S2+3 share of rows) × (S1b bytes at
w* = 0.06), against the baseline's. For text queries S2+3 is near 1 and the net is the coding gain; for item queries
it is where C must pay for its lower keys.

## 6. What to hand back, and limits known in advance

One table per Σ_Q with a row per partition and a column per score, S1d and S1b first and S1a marked secondary; the
three seeds' spread beside it; item 1's table by pass; the sweep's tables with the fitted slopes; B's and C's
objective per pass on fit and on held-out rows; the first-step numbers of C; the count of violations. Scripts and
logs under `notes/program/partition/work/`. State for every figure whether rows were held out. A negative result on
B or C is a result: it says the built grouping is at its limit on this data, in this model of a level.

- 200,000 rows and 1,000 rows a cell: bases are fitted on about 12,500 rows a group (16n), fewer than the build's
  20n, and A1-cell's bases on 750 rows with shrinkage. Held-out scoring removes the optimism and leaves the noise.
- The level model is the floor's allocation, not the built code, which scales a row by its Euclidean residual norm
  and uses one table per group; `../floors/CLAIMS.md` section 7 puts the built code 1.17 to 1.6 above the floor in
  width. Ratios between partitions are what the scores are for. Whether a gain in S1d and S1b carries into the
  bytes of the built code is not answered here: it needs a fit and an encode of the winning partition, which is
  the next experiment if one of B or C passes.
- One dataset. yahoo and LAION, where one of the two measured slopes in G comes from, are not covered.
- Nothing is timed, and the per-cell and per-group setups (P2.3, P2.5) are not in any score but S1e.

## 7. Changes after the referee

- New first item: the distance of the built partition from a fixed point (S0 and the scores by Lloyd pass), and a
  rule that makes the fixed point the baseline if it differs.
- A1 to A3 now follow the build's steps at its ratio of 200 sample rows a centre, where the earlier version ran
  k-means on all fit rows.
- The headline is a level cost and bytes (S1d at 16 B, S1b); the zero-read width S1a is secondary.
- B: the theorem's in-sample scheme and the cross-fitted one side by side, objectives on fit and held-out rows, a
  pass of single-cell moves with refitting, and a floor of 3n rows a group.
- C: the level is fixed at 16 B for the proposed rule (C16); C0, Cmix and C16-in added; tie and small-cell rules
  stated; it starts from B16's result.
- S2 reports the net and the extent apart; S3's inequality and clamp; S4a reports both averages.
- The sweep runs at two corpus sizes and each partition is continued to its fixed point.
- Predictions: the price of the grouping and B's gain widened to the range of the two measured slopes; lines added
  for the fixed point, assumption (b), C16-in and C0.
