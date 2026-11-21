# Replay specification: the statistical width of a level (A4)

For the PI to arrange with the session that owns the visit-order worktrees. It counts rows, bytes and recall; nothing
is timed. Symbols are chapter 0's and those declared at the head of section A4 of `CLAIMS.md`.

## 0. What it is for

Rule 1's derived ρ is reported about 2.3 times too narrow on iNaturalist (n = 2,048, 12 groups) and within 2 to 15%
on yahoo, LAION and glove. This replay does three things, in this order:

1. **finds the cause** among the six of `CLAIMS.md` A4.2 (alignment with the cluster's own directions, a mean
   error, a slope, correlated coded errors, heavy tails, the row's scale);
2. **compares the widths** of A4 per (query, cluster): the derived ρ, A3, form 1, form 2, form 3's cap and its
   sketch, the cousin of A4.6, against the actual rms;
3. **replays the reader** at ψ = 4 and 5 with each width, for recall and rows.

Datasets: iNaturalist, with yahoo and LAION as controls (the derived ρ is right there, and a width that costs rows
on them is not acceptable). k = 10.

Nothing in A4 has seen real data. What section 3 predicts comes from a constructed model (`out/a4_model.txt`).

## 1. The dump

An extension of `study_layered_dump`. For each query, the clusters in the library's own read order at ψ = ∞, the
first 12 of them in full. Split files by dataset and query set so that none exceeds 100 MB.

**Query sets** (100 each):

- Q1: the dataset's queries;
- Q2: corpus rows used as queries, each scored against its own cluster (the row itself left out) and the other
  clusters of its group;
- Q3: corpus rows scored against clusters of groups far from their own;
- Q4: random unit directions.

Q2 to Q4 exist only to separate the causes: an alignment effect is largest on Q2, near 1 on Q3 and below 1 on Q4.

**Per (query, cluster):**

| field | what |
|---|---|
| `q`, `set`, `c`, `g`, `pos` | query, query set, cluster, group, position in the read order |
| `mu`, `sigma16` | μ_c and the 16-direction σ_c(q), as the library computes them |
| `sigma_exact` | the standard deviation of the cluster's rows' scores (optional; separates σ's error from form 1's) |
| `that` | the running k-th best when the library reaches the cluster at ψ = ∞ |
| per level ℓ in {16 B, 32 B}: `w` | ‖D_ℓΔ‖, the query's factor of the half-width (`dwa`) |
| per level: `rho_g` | the group's derived ρ_(g,ℓ), and the per-level median the build uses |
| per level: `a3` | √(Σ_i Δ_i²ν_i/(w²·‖a‖₁)), A3's factor, ν_i = D_i⁴·π_i from the plan |
| per level: `cousin` | the cousin's spread √(T_U + T_C) of A4.6, and its two terms T_U and T_C |

**Per row of those clusters:**

| field | what |
|---|---|
| `s` | the exact score |
| `vis` | visible or not, if a visible set is on |
| per level: `a` | the midpoint the reader would use |
| per level: `h` | the certified half-width's code term, w·u with the stored norm as the reader reads it |
| per level: `hfull` | the whole certified half-width (with the fixed terms) |
| `aR`, `sdR` | optional: the estimate and sd at the first randomised level where every block is coded |

**Per (cluster, level), once per index, from the rows' weighted errors ẽ = D⁻¹(r − r̂) in the group's coordinates:**

| field | what |
|---|---|
| `cap` | √λ_max of the mean of ê·êᵀ over the cluster's rows, ê = ẽ/‖ẽ‖ (power iteration, 20 steps) |
| `eig16` | optional: its top 17 eigenvalues and top 16 eigenvectors, for the sketch; then also per (query, cluster) the sketch's value √(Σ_(k≤16) λ_k(v_k·Δ̂)² + λ_17·(1 − Σ_(k≤16)(v_k·Δ̂)²)) |

The study has the stored vectors and the codes, so it can form ẽ for every row; this is the only part that is not
already in `study_layered_dump`, besides the query sets, `that`, `a3` and `cousin`.

## 2. The analyses

All offline, in numpy, from the dump.

### 2.1 The cause

Per (query, cluster, level), over the cluster's rows:

- R_all = the rms of (s − a)/h; and the same with `hfull`;
- the mean and the standard deviation of s − a, and R_all with the mean removed;
- the slope b of s on a, and ϑ = the squared correlation;
- R_near = the rms of (s − a)/h over the rows with a + 4.5·R_all·h ≥ `that`;
- the 99% and 99.9% points and the kurtosis of (s − a)/(R_all·h);
- R_all within quintiles of the stored norm u = h/w.

Report medians and the 10th and 90th percentiles of each over (query, cluster), by dataset, query set and level,
and R_all over `rho_g` in the same breakdown. Read the cause off the table of `CLAIMS.md` A4.2.

### 2.2 The widths, per (query, cluster) and level

Each as a multiple of R_all, and of R_near:

| width | how it is formed from the dump |
|---|---|
| derived | `rho_g` (the group's; and the per-level median, as built) |
| A3 | `a3` |
| form 1 | √max(`sigma16`² − Var(a), 0) over the rms of h, at the 16 B level; also with `sigma_exact` |
| form 2 | in the read order of the query's clusters: ρ̂² = (m₀·ρ_g²·h̄² + Σ d²)/(m₀·h̄² + Σ h²) over the rows that would have gone on so far, d = s − a at the last deterministic level and d = a_32 − a_16 at the first; m₀ = 4 and 16; seeded by the 2k + 8 best midpoints of the first cluster; the value in force when each cluster is reached |
| form 3 | `cap` |
| the sketch | from `eig16` |
| the cousin | `cousin`, and its safe version √T_U + √T_C |

Report the median and the 10th and 90th percentiles of each ratio by dataset and query set. For form 2 report it
by the cluster's position in the read order (1, 2, 3, 4 to 12).

### 2.3 The reader, replayed

For each width, at ψ = 4 and ψ = 5, walk the query's clusters in the dumped order with the running k-th best taken
from the rows the replay itself scores in full:

    first cluster: the 2k + 8 best midpoints at the 16 B level are scored in full (the reader's seed)
    a row goes from the 16 B level to the 32 B level when min(a_16 + hfull_16, a_16 + ψ·ρ·h_16) ≥ t̂
    and from the 32 B level to its stored vector when     min(a_32 + hfull_32, a_32 + ψ·ρ·h_32) ≥ t̂
    ρ·h is replaced by the width's own spread where the width is not a multiple of h (form 1, the cousin)

Count, per query: the true neighbours found among the dumped clusters over the true neighbours that lie in them
(the level's recall, which leaves the stop out of the question); rows at 32 B; stored vectors; bytes
(18.5 per row at 16 B, 16.5 more at 32 B, 2n per vector). Add two references: certified widths (ρ = 1 in units of
`hfull`), and the oracle width R_near per (query, cluster).

Report, per dataset and width: level recall at ψ = 4 and 5, and bytes over the certified reader's. Then the same at
equal level recall, by interpolating each width's own ψ from 3 to 7.

## 3. What would refute each claim

| claim | refuted if |
|---|---|
| the cause is alignment with the cluster's own directions (A4.2) | on iNaturalist R_all over the derived ρ is the same on Q2, Q3 and Q4 (within 15%); or it does not fall from the 16 B level to the 32 B level; or removing the mean of s − a brings it within 15% of 1 |
| the cause is the slope | accepted in place of alignment if R_all is within 15% of the derived ρ while R_near is twice it and b exceeds 1.15 |
| the cause is the tail | accepted if R_all is within 15% of the derived ρ and the 99.9% point of (s − a)/(R_all·h) exceeds 5 |
| the derived ρ is right on the controls | R_all over the derived ρ lies outside 0.85 to 1.15 in the median on yahoo's or LAION's Q1 |
| A3 corrects nothing of the alignment (A4.1, A4.2) | on iNaturalist's Q1 the median of A3 over R_all is above 0.8 (the model says about 1/2.3) |
| form 1 is wide when b > 1 and ill-conditioned (A4.3) | its median over R_all is within 0.9 to 1.1 on all three datasets and under 5% of (query, cluster) pairs have σ̂² < Var(a) |
| form 2 recovers from a wrong prior (A4.4) | on iNaturalist's Q1, from the third cluster on, its width is under 0.8 of R_near in more than 10% of (query, cluster) pairs; or its level recall at ψ = 4.5 is more than half a point under the certified reader's; or on a control it reads over 10% more bytes than the derived ρ at equal level recall |
| form 3 bounds every query (A4.5) | any (query, cluster) has R_all above `cap` by more than 1% (that would be an error in the claim or in the dump, since the claim is an identity) |
| form 3 is loose where the derived ρ is right | the median of `cap` over R_all is under 1.2 on the controls' Q1 (then the cap could be the width) |
| the sketch is within 10% where the derived ρ fails | on iNaturalist's Q1 its median over R_all is above 1.2, or any pair has R_all above it by more than 1% |
| the cousin is narrow where it is needed (A4.6) | on iNaturalist's Q1 its 10th percentile over R_all is above 0.9 |
| a wide width is dear because the scores' tail is near-exponential (A4.7) | at ψ = 4.5 the rows at 32 B under a width F times R_near are within 20% of those under R_near for F = 1.6 |

## 4. How the result decides

- If the cause is not alignment, A4's forms 3, the sketch and the cousin do not apply, and form 2 still does: it
  measures whatever the cause is. A mean error or a slope is better repaired in the estimate (subtract the mean; use
  ā + b·(a − ā)) than in the width.
- If form 2 passes its row of section 3 on iNaturalist and the controls, it is the width, with `cap` as its
  ceiling.
- If form 2 fails on the first clusters only, the sketch is the prior in place of the derived ρ.
- If neither holds, the derived ρ stays where `cap` is within 1.4 times it, and the level keeps its certified width
  elsewhere.

## 5. Limits of this specification

It replays two deterministic levels and the stored vector; the randomised levels enter only through the optional
`aR` and `sdR`, for the identity of A4.4 item 4. It fixes the clusters to the library's read order, so it measures
what a level loses and costs, not the stop. It uses 100 queries a set; a ratio's median is good to a few per cent
and a level recall to about half a point.
