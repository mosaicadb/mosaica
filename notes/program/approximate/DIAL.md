# The levels' dial as a function of the stop's dial

21 November 2026. One theorist's derivation; nothing here is refereed. Tags follow `notes/README.md`. Complete as
of the last save: §1 the rules as built, §2 the derivation, §3 the formula, §4 the check, §5 what would falsify it and
what is open, §6 the lines of attack and which failed.

## 0. Summary

- The two dials are quantiles of different laws. The stop's ψ is compared with the height of the threshold in a
  neighbour's home cluster, which sits about 3 spreads up before any margin. The level's ψ_ℓ is compared with the
  scatter left after the width has already paid for that height through ζ. Equal dials therefore make the level far
  the safer of the two, which is the measured over-width.
- Equal risk per neighbour, the criterion as first worded, is the wrong target: it makes the levels drop as many
  neighbours as the stop misses, costs up to twice the as-built reader's work on iNaturalist in replay, and is
  rejected. Equal risk per **row declined** is the same condition as the cost optimum when a row passed by the levels
  costs what a row of a newly opened cluster costs, and it is what the formula implements.
- The formula (§3): ψ_ℓ = max(0, ψ − ū − s̄ + (1/√2)·ln(1/(κ·φ̂))), with ū the median height of the threshold in the
  home clusters of the k rows held, s̄ their mean slack, φ̂ the share of rows the levels have passed so far (floored at
  the share sampled), and κ a cost ratio that is 1 until it is timed. Nothing in it is fitted.
- In a causal replay on three dumps it gives ψ_ℓ of 2.6 to 3.9 at ψ = 4 and 5 and 4.0 to 4.8 at ψ = 6, passes 31 to
  64% fewer rows than ψ_ℓ = ψ, and keeps 0.988 to 0.998 of the neighbours the stop finds. Its closing assumptions fit
  yahoo and do not fit iNaturalist (§4). The replay counts rows; nothing here was timed.

## 0a. Files

`dial_laws.py` (the two laws and the tilt, true threshold), outputs `dial_laws_{yahoo,laion,inat}.txt`;
`dial_replay.py` (the causal replay of the stop and the sampled width under each candidate rule), outputs
`dial_replay_{yahoo,laion,inat}.txt`. Dumps: `/home/joe/code/vexact-data/suite/rho-dump/{yahoo,laion,inat}` (64, 64
and 12 groups; 100 queries; the first 30 clusters a query; k = 10).

## 1. The two rules as built

Code: `/home/joe/code/vexact-sampled`, branch `research/sampled-width`, head `0a4a00979`. Line numbers are of that
head.

### 1.1 The stop (`rust/src/v2/adaptive.rs`, `Adaptive::knn_levels`, lines 732 to 921)

- **μ_c** is `qm[ci]`, the query's score with the int8 centre (`self.centres.centre_scores`, line 768).
- **σ_c(q)** is `Adaptive::sigma` (lines 650 to 660): √(Σ_j λ_j (q·v_j)² + rest·(‖q‖² − Σ_j (q·v_j)²)) over the
  r = 16 stored int8 directions v_j with variances λ_j, `rest` the variance per remaining dimension. A cluster whose
  σ would cost more than reading it directly takes the stand-in rad_c·s, s = √(qᵀC̄q / tr C̄) (line 834 to 845).
- **The window** is the next `LOOKAHEAD` = 8 clusters of the stream (line 87, lines 821 to 847), the stream keyed by
  μ_c + λ·rad_c·s with λ = δ + 3ρ (line 783).
- **The pooled moments** (lines 776 to 808). Three running sums over rows: `zn` (count), `zs` (Σ z), `zss` (Σ z²),
  with z = (s − μ_c)/σ_c, s the row's exact score, and (μ_c, σ_c) the values the window holds for that row's cluster.
  Then, once `zn ≥ 2`,

      δ = zs / zn,        ρ = √(max(zss / zn − δ², 0)),

  and before that δ = 0, ρ = 1 (line 777). So δ and ρ are the mean and the (population) standard deviation of the
  standardised height z over **rows**, pooled with equal weight per row over the clusters that feed them.
- **Which rows feed them** (lines 901 to 906 and `rust/src/v2/q.rs` lines 3839 to 3879). Only a cluster of the window
  that is read **directly** calls `visit_z`, which registers (first row, end row, μ_c, σ_c) in `zcl`; `flush` then adds
  that cluster's visible rows' moments as they are scored. A cluster read on the index path calls `visit_sampled` and
  adds nothing: neither its surviving rows nor its 64 sampled rows enter the sums. A cluster of at most r = 16 visible
  rows goes into the direct batch through `rd.visit` (line 828) and adds nothing either. A cluster with σ = 0 adds
  nothing (`q.rs` line 3875).
- **Consequence the notes do not state.** Unfiltered, where nearly every read is on the index path, the sums are
  nearly empty: on yahoo 0.1 to 0.6 clusters a query are read directly at ψ 4 to 6
  (`results/stop-and-width/timing/ab-knn-yahoo-minilm-384-normalized-b87f151.txt`), so for most unfiltered queries
  the stop runs on the prior, δ = 0 and ρ = 1, or on one cluster's rows.
- **The stop test** (line 786, used at lines 817 and 848): stop when

      t̂ > max over the window of μ_c + (δ + ψ·ρ)·σ_c,

  t̂ = `rd.kth()`, the running k-th best exact score (−∞ until k are held). Equivalently every window cluster has
  u_c(t̂) > ψ, where

      u_c(t) = (ζ_c(t) − δ)/ρ,      ζ_c(t) = (t − μ_c)/σ_c                                            (1.1)

  is the threshold's height in cluster c in the pooled standardised units. The test is on the window alone: a cluster
  further down the stream is not examined (the code's own comment, lines 28 to 30).
- **The bypass** (lines 826 to 832): a cluster of at most 16 visible rows takes no σ and no window slot; it is read
  into the direct batch unless μ_c + (δ + ψρ)·rad_c·s < t̂.
- ψ = ∞ is the exact query (line 751). The order does not depend on ψ (ORDER_C = 3, line 82).

**The PI's four statements about the stop, checked against this code.** (1) The stop is a test on the window of 8
priced clusters, and a cluster further down the stream can hold a neighbour: holds. (2) ψ is a quantile per cluster
and not per row: holds in this sense: the test compares ψ with u_c, one number a cluster, and the risk it takes is
the chance that a declined cluster holds a row above t̂; §2.2 gives the law. (3) Clusters of at most 16 visible rows
bypass the window: holds, with the proviso that such a cluster is still dropped when its key with the stand-in spread
is below t̂. (4) δ and ρ are pooled over every directly read cluster, and index-path reads give no moments: holds for
the window's clusters; the batched small clusters are read directly and give none.

### 1.2 A level (`rust/src/v2/q.rs`, `KnnReader::read` lines 3887 to 3973 and `KnnReader::sample` lines 3984 to 4064)

- A cluster read on the index path after k items are held, with at least 2·64 visible rows (line 3956), is sampled:
  `WIDTH_SAMPLE` = 64 of its visible rows (line 3789), evenly spaced in row order over **all** its visible rows
  (line 3988; rows are stored in key order, so the sample is stratified by key), scored exactly. They join the k best.
- t̂ is the running k-th best **after** the sample (line 4016). ζ = max((t̂ − μ_c)/σ_c, 0) with the stop's own μ_c
  and σ_c for this cluster (line 4029; passed from `adaptive.rs` line 905).
- Per deterministic level ℓ (lines 4032 to 4053), with a_ℓ the midpoint of the row's float interval and e = s − a_ℓ
  over the 64 rows: τ_ℓ = population sd of e; f_ℓ = clamp(1 − τ_ℓ²/σ_c², 0, 1); m_ℓ = mean(e) − τ_ℓ·(mean(s) − μ_c)/σ_c;

      W_ℓ = m_ℓ + (ζ + ψ_ℓ·√f_ℓ)·τ_ℓ.

  A row goes on from level ℓ when a_ℓ + min(certified upper half-width, W_ℓ) ≥ t̂.
- ψ_ℓ is `level_psi.unwrap_or(psi)` (`adaptive.rs` line 796): the stop's ψ unless the study knob is set.
- The **randomised** levels do not use W. They are read at min(1, ψ/c) of their certified half-width with the
  **stop's** ψ, c the level's certified constant (median over groups; `adaptive.rs` lines 634 to 647, 759). The study
  knob `level_psi` does not reach them.

## 2. Derivation

### 2.1 One frame (line of attack A)

Fix a query, a true neighbour with score s ≥ τ_k in cluster c, and write x = (s − μ_c)/σ_c for its height. Since the
k best held are real rows, t̂ ≤ τ_k ≤ s at every moment.

Every stage j has an estimate â_j of the row's score, an error e_j = s − â_j and a width Y_j, and passes the row when
â_j + Y_j ≥ t̂. So

    the row is dropped at stage j   ⇔   e_j > Y_j + (s − t̂).                                         (2.1)

[proved, unrefereed; it is §5.9a.2's identity with the running threshold, and holds for any estimate.]

- **Stage 0, the stop.** â_0 = μ_c + δσ_c for every row of c, e_0 = σ_c·(x − δ), Y_0 = ψ·ρσ_c. (2.1) reads
  (x − δ)/ρ > ψ + (s − t̂)/(ρσ_c), which is u_c(t̂) > ψ: the cluster is not read. The dial ψ is compared with the
  height **of the threshold in the cluster**, in units in which a typical row has mean 0 and spread 1.
- **Stage ℓ, a level.** With the measured tendency e_ℓ = m_ℓ + τ·(x + √f·g) and Y_ℓ = m_ℓ + (ζ + ψ_ℓ√f)·τ, and
  τ = σ_c·√(1 − f) (the definition of f as built, when it is not clamped), (2.1) reads, for ζ > 0,

      g > ψ_ℓ + (x − ζ)·β,      β = (1 − √(1 − f))/√(f·(1 − f)).                                      (2.2)

  The dial ψ_ℓ is compared with the scatter g **after** the height has been taken out by the term ζ·τ.

So equal dials do not give equal risks. The stop's ψ has to cover the height itself: a neighbour sits about 3 spreads
up its cluster, so a stop at ψ = 5 leaves about 2 spreads of margin. The level's width already pays for the height
through ζ, and ψ_ℓ = 5 puts 5 spreads of scatter on top of it. With ζ ≈ 3 and f = 0.84 the built width at ψ_ℓ = ψ = 5
is (3 + 5·0.92)/5 = 1.5 times the plain width 5τ. The derived width is close to the plain one on yahoo (§5.9a.1), and
the timing session measured the built width at about 1.6 times the derived one there.

### 2.2 What each dial is a quantile of (measured; `dial_laws.py`, outputs `dial_laws_{yahoo,laion,inat}.txt`)

All [measured] on the dumps: first 30 clusters a query, 100 queries, k = 10, the true 10th best as t, δ = 0 and
ρ = 1 as the unfiltered reader mostly has them, the library's μ_c and σ_c, the reader's own 64-row statistics.

- **The stop.** A neighbour is missed by the stop when the threshold's height in its home cluster, u_home, is above
  ψ. So ψ is a quantile of the law of u_home over the true neighbours. Measured quantiles 10 / 50 / 90%: yahoo
  2.92 / 3.63 / 4.94, LAION 2.41 / 3.05 / 4.27, iNaturalist 2.15 / 2.77 / 4.02. Share above 4 / 5 / 6: yahoo
  0.30 / 0.097 / 0.024 (the reader's stop with certified levels misses 0.219 / 0.086 / 0.022 at those ψ); LAION
  0.139 / 0.019 / 0 of those in the dump (reader 0.182 / 0.048 / 0.018; the dump's 30 clusters truncate LAION past 5).
  The upper half is close to exponential, with mean excess θ_u of 0.7 to 0.8.
- **A level.** Let y* be the least ψ_ℓ at which a neighbour passes both deterministic levels (slack included, since
  the true t is used). ψ_ℓ is a quantile of y*. Its law is close to Laplace with its own centre and spread out to the
  99.5th percentile: mean / sd −1.04 / 1.40 (yahoo), −0.36 / 1.37 (LAION), −0.26 / 1.31 (iNaturalist). P(y* > 0, 1, 2,
  3, 4): yahoo 0.217, 0.067, 0.022, 0.0052, 0.0021; LAION 0.356, 0.142, 0.049, 0.018, 0.0086; iNaturalist 0.405,
  0.144, 0.050, 0.015, 0.0062. One unit divides the miss by 2.7 to 4.2 (yahoo), 2.1 to 2.9 (LAION), 2.4 to 3.4 (iNat).
- **Two levels.** The miss at either level over the larger of the two single-level misses is 1.1 to 1.5 at dials 1
  to 3: the errors of the two nested levels are strongly dependent, and y* as defined already carries both.
- **The bulk is not the neighbours.** The sampled rows' scatter about the ray, in the same units, has sd 1.12, 0.98,
  1.12 and P(g > 2 / 3 / 4) of 0.039 / 0.0053 / 0.0005 (yahoo), 0.027 / 0.0043 / 0.0007 (LAION), 0.041 / 0.0092 /
  0.0023 (iNat). Against y* it is 0.6 to 1.8 times at 2, 1 to 4 at 3 and 3 to 12 at 4: its tail is lighter, as
  referee 2 warned.
- **The row law does not give the stop's risk.** The rows' heights have an exponential tail (rate 1.8 to 1.9 on
  yahoo, 2.4 to 2.6 on LAION, 1.8 falling to 1.1 on iNat), but the neighbours a cluster holds at threshold height u
  fall at rates 1.71, 1.37 and 1.50, and n_c·P(z > u) is wrong by 0.5 to 2.5 times (yahoo), 0.1 to 6 (LAION) and 8
  to 40 (iNat). So an extreme-value link from the pooled rows to the stop's risk fails on two of three datasets.
- **The tilt of the rule does not matter much.** Rows past both levels at equal kept share, uniform ψ_ℓ against a
  dial proportional to the cluster's margin ψ − u_c and against the plain rule: within 3% on yahoo; uniform better by
  5 to 12% on iNat; equal on LAION at 0.95 and 0.98 kept and 25% worse at 0.99.

### 2.3 The causal replay, and the slack (line of attack D)

`dial_replay.py` reads the dumped clusters in the dump's order with a running threshold, the reader's 8-cluster window
and the reader's sampling, and takes the randomised levels as exact. With certified levels it reproduces the reader's
stop: recall at ψ = 4 / 5 / 6 of 0.791 / 0.923 / 0.973 on yahoo against the reader's 0.781 / 0.914 / 0.978
(`results/stop-and-width/timing/ab-knn-*-b87f151.txt`), 0.879 / 0.956 on LAION against 0.818 / 0.952 (ψ = 6 is cut off
by the dump's 30 clusters), and 0.959 / 0.992 / 0.997 on iNaturalist against 0.973 / 0.993 / 0.999. At ψ_ℓ = ψ = 5 it
passes 12,591 rows a query past both levels on LAION, where the reader counted 15,304 less 1,657 sampled
(`results/sampled-width/float-vs-int-laion.txt`). [measured]

The slack matters more in the reader than the replays at the true threshold show. A neighbour's slack is
(s − t̂)/σ_c, and t̂ is the running threshold when its cluster is read, which is below τ_k for the early clusters where
most neighbours live; the first cluster is read before k are held and is not sampled at all. Per neighbour that the
stop finds, the levels drop, at ψ_ℓ = 2 / 3 and stop ψ = 5: 0.4% / 0.1% (yahoo), 2.1% / 0.7% (LAION), 1.2% / 0.2%
(iNaturalist), against 2.2% / 0.5%, 4.9% / 1.8% and 5.0% / 1.5% at the true threshold. So the recorded kept shares of
0.97 to 0.99 at ψ_ℓ = 2 to 3 are 0.979 to 0.999 in the causal replay. [measured; 100 queries, so a figure of 0.1% is
one event]

The mean slack of the k held rows, in dial units, s̄ = β·mean_i (s_i − t̂)/σ_home(i) with β of (2.2), is 0.56 to 0.60
on yahoo, 0.33 to 0.38 on LAION and 0.54 to 0.56 on iNaturalist. [measured]

### 2.4 Three readings of "equal risk"

Write M_stop(ψ) for the share of the true k the stop misses, M_lev(ψ_ℓ) for the share the levels drop, and
r = M_lev/M_stop. For a uniform ψ_ℓ define, at the operating point,

- N′, the rows of the clusters that one more unit of ψ would read, and π_0, the share of those rows that are true
  neighbours;
- R′, the rows that one more unit of ψ_ℓ would pass, and π_ℓ, the share of those rows that are true neighbours;
- θ_u = M_stop/(−dM_stop/dψ) and θ_y = M_lev/(−dM_lev/dψ_ℓ), the inverse hazards of the two laws of §2.2 (the mean
  excess, where a tail is exponential).

**Proposition D1 (the risk ratio).** r = (θ_y/θ_u)·(π_ℓ/π_0)·(R′/N′). [proved, unrefereed]

*Proof.* The neighbours gained by one more unit of ψ are k·(−dM_stop/dψ) = π_0·N′, and by one more unit of ψ_ℓ are
k·(−dM_lev/dψ_ℓ) = π_ℓ·R′, both by the definitions of π_0 and π_ℓ. Substitute the definitions of θ_u and θ_y and
divide. ∎

It is an identity: no law of the data enters. It separates the three readings.

1. **Equal risk per neighbour**: r = 1. Then π_ℓ/π_0 = (θ_u/θ_y)·(N′/R′), which is of order N′/R′, some 13 to 60 in
   the replay at the dials of §4. The rows the levels drop at their margin would be that many times likelier to be neighbours than the
   rows of the clusters the stop declines. The reader would be refusing cheap, likely rows while buying expensive,
   unlikely ones.
2. **Equal risk per decision** (a level's chance of dropping a neighbour set to the chance ν(ψ) = π_0·n_c that a
   declined cluster holds one): the levels then drop V·ν neighbours a query over V clusters read, which is of the
   order of everything the stop misses or more. On yahoo ν(5) is 0.10 to 0.19 and V is 8, so 0.8 to 1.5 neighbours a
   query against the stop's 0.77. It is reading 1 made worse.
3. **Equal risk per row declined**: π_ℓ = π_0. A row the levels drop at their margin is as likely a neighbour as a
   row of a cluster the stop declines at its margin. Then r = (θ_y/θ_u)·(R′/N′).

**Proposition D2 (the cost condition; line of attack C).** Suppose the query's time T is differentiable in the two
dials with ∂T/∂ψ = c_0·N′ and ∂T/∂ψ_ℓ = c_1·R′, c_0 the cost of a row of a newly read cluster (its share of the
cluster's fixed costs, the sorted range, the deterministic levels and the sample included) and c_1 the cost of a row
the deterministic levels pass (its randomised levels and, where it survives, its stored vector). Then T is stationary
at a fixed M_stop + M_lev exactly when

    π_ℓ / c_1 = π_0 / c_0,      that is      r_C = (θ_y/θ_u)·(c_1/c_0)·(R′/N′).                        (2.3)

[proved, unrefereed, under the stated form of T; sketch as a statement about the reader: the gap is that ψ_ℓ also
moves t̂ and so the stop, that ψ also moves R, and that one ψ_ℓ is used for every cluster]

*Proof.* Stationarity under the constraint is (∂T/∂ψ)/(−∂M_stop/∂ψ) = (∂T/∂ψ_ℓ)/(−∂M_lev/∂ψ_ℓ). The left side is
c_0·N′·k/(π_0·N′) and the right side c_1·R′·k/(π_ℓ·R′). ∎

So the owner's criterion, read per row declined, is the cost optimum when κ = c_1/c_0 = 1, and the cost optimum in
general is reading 3 with the price of a row put in. In words: the stop declines to spend one cluster's read to
recover ν(ψ) neighbours, so the levels should decline rows only while the neighbours those rows hold, per cluster's
read of time they would cost, stay under ν(ψ).

*Readings 1 and 2 are far from the cost optimum in the measured regime.* In the causal replay the lowest-cost rules
have r between 0.01 and 0.25 (§4), not 1; the rule nearest reading 1, ψ_ℓ = ψ − ū − s̄, lies between the rows "med-1"
and "med+0" of `dial_replay_*.txt`, whose cost proxy against the as-built reader is 0.77 to 1.06 on yahoo, 0.68 to
1.15 on LAION and 1.15 to 2.06 on iNaturalist. [measured, a proxy in rows]

**The tilt.** A dial per cluster that grows with the cluster's margin ψ − u_c is what readings 1 and 2 suggest
cluster by cluster. It does not pay: §2.2's last bullet. The reason is that the term ζ·τ of the built width already
follows the prior: in a far cluster few rows have estimates near the threshold, so both the neighbours and the rows
at a given dial thin out together, and the posterior at a given dial varies little with the cluster. [measured;
the explanation is a sketch] One uniform ψ_ℓ a query is therefore kept.

### 2.5 Closing the formula

D1 and D2 leave the level of ψ_ℓ to the two laws. Three assumptions close it; each is a modelling assumption and each
is checked in §4.

- **H1 (exponential tails).** Above their medians both laws are exponential: M_stop = ½·exp(−(ψ − ū)/θ_u) with ū the
  median of u_home, and, per neighbour found, M_lev = ½·exp(−(ψ_ℓ − ȳ)/θ_y) with ȳ the median of the needed dial.
  [model; §2.2: both fit within 20% out to the 99.5th percentile at the true threshold]
- **H2 (one scale, and the slack as a shift).** θ_u = θ_y = θ, and ȳ = −s̄: the scatter g is centred with a Laplace
  law of unit spread, so θ = 1/√2, and the slack moves its centre by its mean. [model. Measured θ_u is 0.6 to 0.87
  on six datasets from the reader's stop-only recall; θ_y at the true threshold is 0.8 to 1.0; ȳ is −1.09, −0.49,
  −0.29 against −s̄ of about −0.6, −0.35, −0.55. Replacing the slack's law by its mean understates the miss by a
  factor 1.2 to 1.3 by Jensen, 0.15 of a dial unit.]
- **H3 (equal growth).** R′/N′ = R/N = φ, the share of the rows read so far that the levels have passed: rows passed
  and rows read grow at the same rate in their dials. [model. Measured d ln R/dψ_ℓ is 0.5 to 0.9 and d ln N/dψ 0.5 to
  1.1.]

Under H1 and H2, r = exp(−(ψ_ℓ + s̄)/θ + (ψ − ū)/θ), so ψ_ℓ = ψ − ū − s̄ + θ·ln(1/r). Under H3 and (2.3),
r = κ·φ. Hence

    ψ_ℓ = ψ − ū − s̄ + θ·ln(1/(κ·φ)),      θ = 1/√2.                                                (2.4)

[model: D1 and D2 with H1 to H3]

Each term has a meaning. ψ − ū is the stop's margin over the neighbours' own clusters, the part of the stop's dial
that is not spent on height. s̄ is the slack the neighbours already have. θ·ln(1/(κφ)) is the allowance for the
levels' misses being cheap to avoid: the fewer rows the levels pass, the wider they may be.

**Several levels (the L′ of line B).** y* is the larger of the two levels' needed dials, so (2.4) is for the two
deterministic levels together and carries no further allowance. Measured, the union's miss is 1.1 to 1.5 times the
larger single level's (§2.2), so L′ would be 1.1 to 1.5 if the levels were dialled one by one: 0.1 to 0.3 of a dial
unit.

## 3. The formula

    ψ_ℓ = max(0, ψ − ū − s̄ + (1/√2)·ln(1/(κ·φ̂))),      computed once per cluster read on the index path, after its
                                                          sample is scored and t̂ is updated.

| Input | What it is | Where it comes from | Cost |
|---|---|---|---|
| ψ | the stop's dial | the caller | none |
| t̂ | the running k-th best after the cluster's sample | `self.top.kth()` (`q.rs` line 4016) | none |
| δ, ρ | the stop's pooled moments | `adaptive.rs` lines 805 to 807; pass them to the reader with (μ_c, σ_c) | none |
| u_i | for each of the k held rows, the threshold's height in its home cluster, ((t̂ − μ_home)/σ_home − δ)/ρ | store (μ_c, σ_c) of the cluster a row came from beside its heap entry; a row from a cluster with no σ uses the stream's stand-in rad_c·s, as the stop does | k subtractions and divisions |
| ū | the median of the u_i | the heap | a selection among k |
| s̄ | the mean slack, β·mean_i (s_i − t̂)/σ_home(i), β = (1 − √(1 − f))/√(f(1 − f)) with f the first deterministic level's f_ℓ of the cluster in hand; s̄ = 0 if f is 0 or 1 | the heap and the sample | k operations |
| φ̂ | max(rows the deterministic levels have passed so far in this query, rows sampled so far) / rows of the clusters sampled so far, the cluster in hand included in the last two | three counters in `KnnReader` | none |
| κ | cost of a row past the deterministic levels over cost of a row of a newly read cluster | 1 until timed; see §5 | none |

- The floor on φ̂ at the sampled share keeps the first sampled cluster from a dial set by a count of zero, and says
  that the levels need not pass fewer rows than the sample already reads. With 64 rows of 2,000 it caps the last term
  at (1/√2)·ln(31) = 2.4.
- **Before the statistics exist.** The reader samples a cluster only once k rows are held (`q.rs` line 3912), and
  from that moment ū, s̄ and φ̂ are all defined, so there is no separate start-up rule. Early in a query t̂ is low, the
  u_i are small and ψ_ℓ comes out wide, which is the safe side. If fewer than three held rows have a home cluster
  with a spread, use ψ_ℓ = ψ, the reader as built.
- The certified width still caps W_ℓ, as built. No other cap is needed: the formula can exceed ψ only in the first
  sampled clusters of a query, and by under one unit.
- **Under a visible set.** ū uses the whole cluster's μ_c and σ_c and the stop's pooled δ and ρ, which is what Lemma
  5.31 licenses; φ̂ counts visible rows; nothing new is taken from the visible sample. τ_ℓ under a cut on score stays
  13 to 21% narrow (§5.9a.4), and ψ_ℓ multiplies it, so that defect is unchanged, neither repaired nor worsened in
  kind.
- It does not depend on the units of σ_c or τ: every term is a ratio.
- Per query the whole computation is about 3k arithmetic operations and one logarithm per cluster read on the index
  path: under 0.1 µs a cluster against 60 to 120 µs for the cluster's read.

**For the pooled moments** (an observation, not part of the formula). Unfiltered, δ and ρ sit on their priors because
index-path reads feed them nothing (§1.1). The 64 sampled rows of every index-path cluster are exact scores spread
evenly over the cluster's visible rows, so their z would feed the sums at no extra read.

## 4. The check

Causal replay (`dial_replay.py`), δ = 0, ρ = 1, k = 10, 100 queries. "As built" is ψ_ℓ = ψ; "formula" is §3 with
κ = 1 (rule `fbf1` in the outputs). Columns: recall; rows past both levels a query, samples excluded; true neighbours
the levels drop a query; mean ψ_ℓ (sd over clusters).

| | stop ψ | certified: recall | as built: recall / rows / dropped | formula: recall / rows / dropped / ψ_ℓ |
|---|---|---|---|---|
| yahoo | 4 | 0.791 | 0.791 / 325 / 0.00 | 0.789 / 179 / 0.02 / 2.59 (0.60) |
| | 5 | 0.923 | 0.923 / 1,331 / 0.00 | 0.921 / 538 / 0.02 / 3.21 (0.66) |
| | 6 | 0.973 | 0.973 / 3,385 / 0.00 | 0.971 / 1,202 / 0.02 / 4.04 (0.76) |
| LAION | 4 | 0.879 | 0.875 / 4,566 / 0.04 | 0.869 / 1,855 / 0.10 / 2.67 (0.60) |
| | 5 | 0.956 | 0.955 / 12,591 / 0.01 | 0.945 / 4,501 / 0.11 / 3.41 (0.71) |
| | 6 (cut off by the dump) | 0.961 | 0.960 / 20,610 / 0.01 | 0.958 / 7,603 / 0.03 / 4.11 (0.74) |
| iNaturalist | 4 | 0.959 | 0.957 / 486 / 0.02 | 0.952 / 335 / 0.07 / 3.11 (0.67) |
| | 5 | 0.992 | 0.990 / 944 / 0.02 | 0.988 / 583 / 0.04 / 3.89 (0.78) |
| | 6 | 0.997 | 0.996 / 1,501 / 0.01 | 0.993 / 900 / 0.04 / 4.80 (0.83) |

- **Against the measured kept shares.** The formula keeps 0.997 to 0.998 (yahoo), 0.988 to 0.997 (LAION) and 0.993 to
  0.996 (iNaturalist) of the neighbours the stop finds. The constant dials 2 and 3 keep, at stop ψ = 5, 0.996 and
  0.999, 0.979 and 0.993, 0.988 and 0.998. So the formula sits at or a little above a constant 3 at ψ = 4 and 5 and
  near a constant 4 at ψ = 6. It is above the "2 to 3" of the brief at ψ = 6, because its slope in ψ is 1.
- **Rows.** It passes 45 to 64% fewer rows than as built on yahoo, 59 to 64% on LAION and 31 to 40% on iNaturalist.
- **A proxy for time at equal recall** (rows read + c_1·rows past the levels + 3·rows sampled, interpolated in recall
  along ψ, as a ratio to as built; c_1 = 3 / 6; recall 0.90 and 0.95): formula 0.90 to 0.93 / 0.82 to 0.86 on yahoo,
  0.86 to 0.87 / 0.76 on LAION, 1.00 to 1.03 / 0.99 to 1.01 on iNaturalist. With κ = 3 (`fbf3`): 0.89 to 0.92 /
  0.79 to 0.83, 0.85 to 0.88 / 0.72 to 0.75, 1.07 to 1.13 / 1.03 to 1.09. A constant ψ_ℓ = 3: 0.87 to 0.91 /
  0.77 to 0.83, 0.80 to 0.84 / 0.66 to 0.74, 0.97 to 0.98 / 0.95 to 0.96. ψ − ū + 1: 0.87 to 0.92 / 0.77 to 0.83,
  0.83 / 0.71, 1.04 to 1.09 / 1.01 to 1.05. The optimum is flat: every rule between a constant 2 and ψ − ū + 1 is
  within about 10% of the best on yahoo and LAION.
- **Where it does not fit.** (2.4) aims at r = κφ. The replay's r at the formula's dial, over φ̂: yahoo 0.026 against
  0.031 at ψ = 5, which fits; LAION 0.25 against 0.077, three times over; iNaturalist 0.17 against 0.025 at ψ = 4 and
  0.5 against 0.03 at ψ = 5, seven to seventeen times over. On iNaturalist the stop's tail is steeper (θ_u about 0.6)
  and the scatter wider than H2 allows (its slope is 1.2 and it widens with height), so the formula is 1.4 to 2 dial
  units narrower there than its own criterion asks, and on that dataset it only ties the reader as built. That is
  H2 failing, on the unsafe side. On iNaturalist the proxy's best rule is as built or a constant 3 or wider.
- **It does not reach the old build.** On yahoo the as-built reader took 1.41 to 1.76 times the derived-width build's
  time. The proxy gives the formula 0.82 to 0.93 of as built, so it recovers about a third to a half of that gap if
  the proxy's row prices are right; the 64 sampled rows a cluster remain. On LAION the derived-width build lost 0.4
  points of recall at ψ = 5 at 1,395 µs, where the sampled width at ψ_ℓ = 5 lost 0.6 at 2,190 µs: part of that gap is
  the rule's shape (one τ a cluster against a width per row), which no dial recovers.
- Samples are small: 100 queries, so a "dropped" figure of 0.02 is two neighbours. The proxy is in rows, not time.

## 5. What would falsify it, and what is not settled

**Would falsify it.**

1. Timed in the reader, with ψ_ℓ from §3: time to recall 0.9 to 0.99 on yahoo and LAION not at least 10% under the
   reader at ψ_ℓ = ψ. The session's counts of rows reaching the randomised levels at ψ_ℓ = 2 and 3 (stop ψ = 5) can be
   set beside the replay's 208 and 421 (yahoo) and 1,298 and 3,310 (LAION) first.
2. The ratio of neighbours dropped by the levels to neighbours missed by the stop, counted in the reader at the
   formula's dial, far from κφ̂. The replay already shows a factor 3 on LAION and 7 to 17 on iNaturalist, so (2.4) as
   closed is falsified as a statement of level on iNaturalist; what stands there is D1 and D2.
3. A best ψ_ℓ that does not rise about one for one with ψ. H2 predicts a slope θ_y/θ_u of 1; the measured scales
   give 1.0 to 1.6.
4. A dataset where the cost optimum has the levels dropping as many neighbours as the stop misses. That would
   restore reading 1.

**Not settled.**

- **κ.** The reader's cost model has no price for a row past the deterministic levels: `cut_row_us` prices a row the
  sorted range leaves, through everything, at certified widths. κ needs one more timed constant. In the replay κ = 3
  moves yahoo and LAION by between 1% worse and 5% better and costs iNaturalist 4 to 10%, so the data do not confirm
  a κ above 1.
- **The two scales.** θ_u and θ_y could not be measured per query: a rule that estimated them from the k held rows
  gave dials with a spread of 7 to 15 and was worse than as built (`eq` in the outputs). They could not be taken from
  the pooled sample either: the bulk's tail is 3 to 12 times lighter than the neighbours' at a dial of 4. So H2 is
  a structural stand-in, and iNaturalist shows its error.
- **Noise in the dial.** A dial that varies between clusters loses more than its mean suggests, because the miss is
  convex in the dial. The first form tried for φ̂, with no floor, had a spread of 1.0 to 1.9 and cost up to 7% more
  than §3's form on yahoo, the same within 6% on LAION and 12 to 30% more on iNaturalist (`fb4`, `fb16`). §3's form has a spread of 0.6 to 0.8, mostly from ū. Whether a median over more
  than k rows would steady it was not tried.
- **Filters.** Nothing here was run under a visible set. The filtered dumps exist
  (`rho-dump/*-random-25`, `*-region-25`); the replay would need δ and ρ pooled as the reader pools them.
- **The randomised levels.** They are read at ψ/c of their half-width with the stop's ψ (§1.2), ψ spreads of an
  error whose law is known. By the same criterion their dial is the normal quantile of the level's risk, not ψ:
  with H1 and H2, ψ_r = Φ̄⁻¹(½·exp(−√2·(ψ_ℓ + s̄))), about 2.2 at ψ_ℓ = 2 and 2.7 at ψ_ℓ = 3, where the reader uses 5
  to 6. [model; not tested, and the replay takes those levels as exact] Since over half of the as-built reader's
  extra time is randomised-level work, this may be worth as much as the deterministic dial.
- **The replay's limits.** 30 clusters a query (LAION is cut off from ψ = 5.5), 100 queries, the dump's order in
  place of the reader's window choice, the randomised levels exact, δ = 0 and ρ = 1, rows in place of time.
- D1 and D2 have had no second reader.

## 6. The lines of attack, and which failed

- **A (one frame).** Holds: (2.1), (2.2), and §2.2's two laws. The stop's ψ is a quantile of the height of the
  threshold in the neighbours' home clusters, in the pooled units of the rows' heights. It is not a quantile of the
  rows' law, nor of the cluster maxima's.
- **B (equal risk, ψ_ℓ = S_g⁻¹(p(ψ)/L′)).** Fails as worded, per neighbour and per cluster (§2.4, readings 1 and 2).
  It holds per row declined (reading 3), where it is the cost condition at equal row prices. L′ is 1.1 to 1.5 and
  is absorbed by defining the needed dial over both levels.
- **B's sources for the tails.** The pooled residuals of the sampled rows fail: lighter than the neighbours' tail by
  1 to 4 times at a dial of 3 and 3 to 12 at 4. The extreme-value link from the rows' tail to the stop's risk per
  cluster fails on LAION and iNaturalist: neighbours a cluster fall at rates 1.4 to 1.7 a spread where the rows'
  tail falls at 1.1 to 2.6, and n_c·P(z > u) is off by up to 6 and 40 times. Per-query tails from the held rows fail
  on noise.
- **C (the cost optimum).** Derived (D2). It needs the price of a row past the levels, which the reader does not
  time. It is far from reading 1 of B: r of 0.01 to 0.25 against 1.
- **D (the slack).** In: the term s̄, 0.33 to 0.60 of a dial unit; and §2.3's larger effect of the running threshold,
  which makes the levels' misses in the reader 2 to 5 times fewer than at the true threshold.
- **E (sanity).** ψ_ℓ grows with ψ, slope 1. At ψ = 4 and 5 it lands at 2.6 to 3.9, at ψ = 6 at 4.0 to 4.8, above
  the brief's 2 to 3, for the reason in §4. Unit-free. Under a filter it uses only whole-cluster heights and the
  stop's pooled moments; not tested.
