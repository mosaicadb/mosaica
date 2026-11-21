# Replay specification: a stop on expected misses (A2)

For the PI to arrange with the session that owns `/home/joe/code/vexact-visitorder` and `/home/joe/code/vexact-sc`,
after its timing ends. Nothing here is timed: the replay counts rows and clusters, as `study_region_order` does.
Symbols are those of `notes/00-notation.md`; local ones are declared in `CLAIMS.md`.

## 0. What the replay is for, and what to expect

The question is whether a stop that predicts the neighbours left in the unread clusters is better than the stop as
built, in rows at equal mean recall, and whether its setting means a recall.

A pre-test in numpy (`scripts/replay_np.py`, tables in `out/tables.md`, entry 5 of `NOTEBOOK.md`) already answers
this for an idealised reader: whole clusters, the static order μ_c + 3σ_c, each cluster's exact spread, yahoo, LAION
text and glove, five visible sets, k = 10, 100 queries. Its findings are the predictions this replay should try to
refute on the real reader (the library's order, window and 16-direction spread, eight datasets):

1. No stop on expected misses reads fewer rows than the ψ stop at equal mean recall on yahoo or LAION by more than
   the noise of 100 queries. Per-query tails fitted to the pooled scores read 1.1 to 3 times more there (generalised
   Pareto 1.2 to 3.0, exponential 0.95 to 3.0); a shape table from the build reads 0.9 to 1.5 times more.
2. No tail makes the setting a recall across datasets and visible sets. A nominal 0.9 delivers 0.65 to 0.95 with a
   normal tail, 0.71 to 0.87 with a generalised Pareto tail, 0.75 to 0.99 with a per-query exponential tail, and 0.80
   to 0.98 with the build's shape table (0.80 to 0.89 on LAION's text queries). ψ = 4.5 delivers 0.92 to 1.00.
3. One variant has an upside: the stop as built with δ_q and ρ_q taken from two upper quantiles of the pooled
   standardised scores instead of their mean and spread. It reads 0.64 to 0.93 of the built stop's rows on glove
   (unfiltered, random-25, region-25), 0.80 to 1.30 on yahoo and 0.80 to 1.35 on LAION.
4. The built stop is not uniformly better than the same number of clusters for every query: the fixed count reads
   0.77 to 0.81 of its rows on LAION unfiltered at recall 0.98 and 0.99, and 0.68 to 0.81 on glove unfiltered.

If the PI wants the cheapest useful run, it is the dump of section 1 on the six datasets of 384 dimensions and more,
with rules R0 and R1 of section 2 replayed from it. Rules R2 to R5 are controls.

## 1. What the harness dumps

One pass per (dataset, visible set), in the study that already reads every cluster whole
(`study_region_order` in `rust/src/v2/e2e.rs`). Per query, one record per cluster that holds a visible row, **in the
order the library's own probe reads them** at ψ = ∞ with the pooled moments (the order does not depend on ψ), so the
replay needs no model of the stream and window. Each record:

| field | type | what |
|---|---|---|
| `c` | u32 | cluster id |
| `pos` | u32 | position in the library's read order |
| `mu` | f32 | μ_c, the centre score the library uses (int8 centres) |
| `sigma` | f32 | σ_c(q) as the library computes it (16 directions and the rest), or `rad_c·s` where the library takes no sigma; one flag bit says which |
| `sigma_exact` | f32 | the spread of all the cluster's rows' scores along q (optional; separates the estimate's error from the rule's) |
| `v` | u32 | visible rows v_c |
| `direct` | u8 | 1 if the library reads it directly at its position under `fixed_costs`, 0 on the index path (index-path reads give no moments in the library) |
| `top` | k × f32 | the k largest scores among the cluster's visible rows, descending (fewer if v_c < k) |
| `sz`, `szz` | f64, f64 | Σ z and Σ z² over the visible rows, z = (s − μ_c)/σ_c with the library's σ_c |
| `hz` | 282 × u16 | counts of z over the visible rows: bins of width 0.05 on [−2, 12), one bin below and one above |

The truth needs nothing more: the k-th best visible score is the k-th largest of all `top` lists, and N_c is the
number of a cluster's `top` entries at or above it (ties broken by row id in the harness, so that exactly k rows are
true; the pre-test's first LAION run counted ties and reported recalls of up to 1.016).

Once per dataset, with no visible set and no queries, the **shape table**: `hz` summed over every cluster for 1,000
corpus rows used as queries (each left out of its own cluster's counts). It needs no truth. It is a property of the
build.

Size: 100 queries × K ≤ 600 records × about 0.7 kB is 42 MB per (dataset, set) at most; under 20 MB if records stop
at the first 300 read positions and the rest are summarised by `mu`, `sigma`, `v`, `top`. Keep each file under
100 MB.

Sets: none, random-1, random-25, region-1, region-25, region-60, the in-cluster bottom-25 and top-25, and the
mean-query bottom quarter (the sets of `results/region25-order/README.md`). k = 10; k = 100 on three datasets.
Datasets: yahoo, LAION, arxiv, landmark, bioclip, iNaturalist; glove and SIFT as guardrails.

## 2. The rules, replayed from the dump

State after reading the first m records: t̂_m the k-th largest of the `top` entries read (−∞ until k are held);
n_m, the pooled Σz and Σz² over the records read (all of them, and separately only those with `direct` = 1);
H_m the summed `hz`; U_m the records not yet read. For c in U_m, ζ_c = (t̂_m − μ_c)/σ_c.

    δ_m = Σz / n_m,  ρ_m = sqrt(Σz²/n_m − δ_m²)                (0 and 1 until two rows are pooled)

    # tail-matched shift and scale: a normal through two upper quantiles of the pooled z
    q1, q2 = the z with pooled exceedance shares p1 = 0.05 and p2 = 0.005 (from H_m)
    ρt_m = (q2 − q1) / (Φ̄⁻¹(p2) − Φ̄⁻¹(p1)),  δt_m = q1 − ρt_m·Φ̄⁻¹(p1)       (δ_m, ρ_m while p2·n_m < 8)

**R0, the stop as built.** Stop at the first m with k rows held and

    min over c in W_m of (ζ_c − δ_m)/ρ_m  >  ψ ,

W_m the library's window (the next 8 priced records) and, as a second arm, all of U_m.

**R1, the same with the tail-matched pair.** As R0 with (δt_m, ρt_m).

**R2 to R5, stops on expected misses.** Stop at the first m with k rows held and

    M̂_m = Σ over c in U_m of v_c · Ĝ_m(ζ_c)  <  (1 − r)·k ,

with the tail Ĝ_m one of:

- R2: Φ̄((ζ − δ_m)/ρ_m), the normal tail at the pooled moments;
- R3: S((ζ − δ_m)/ρ_m), S the build's shape table as a survival function (log-linear between bins, and beyond the
  last populated bin continued at the slope of its last decade);
- R4: the pooled sample's own tail: the empirical survival from H_m up to its p1 level u, and above it
  p1·exp(−(ζ − u)/e_m), e_m the mean excess of the pooled z over u (while p1·n_m < 40, R2);
- R5: as R4 with a generalised Pareto tail above u, shape and scale from the excesses' mean and variance.

A further arm for each of R2 to R5 replaces t̂_m by the τ̂_m that solves
(rows read with score ≥ t) + Σ over U_m of v_c·Ĝ_m(ζ_c(t)) = k, and stops when k minus the rows read at or above τ̂_m
is below (1 − r)·k. It removes the gap between the rows above t̂ and the true misses (claim A2.1).

Each rule is replayed twice: with moments and counts from every record read, and from the `direct` records only.

## 3. The comparison

Per (dataset, set, k):

1. **Rows at equal mean recall.** For each rule, sweep its setting (ψ from 2 to 12 in steps of 0.25; (1 − r)·k from
   5 down to 0.001), take the mean visible rows read and the mean recall, and interpolate the rows at mean recall
   0.9, 0.95, 0.98 and 0.99. Report each rule's rows over R0's. Also clusters read.
2. **What a setting delivers.** For R2 to R5 at nominal r = 0.9, 0.95 and 0.99, and for R0 and R1 at ψ = 4, 4.5, 5
   and 6: the mean recall, the share of queries below the nominal recall, the mean rows.
3. **The prediction against the count**, at every step where M̂_m lies between 0.3 and 3: the mean of M̂_m, of the
   visible rows above t̂_m in U_m, and of the true misses. This separates a tail that is wrong on average from one
   that is right on average and noisy per query.
4. **What the query holds when it decides** (first m with M̂_m < 1): n_m, the pooled rows with z > 2 and z > 3, and
   the smallest ζ_c in U_m; 10th, 50th and 90th percentiles over queries.
5. **Two references**: the same number of clusters for every query; and each query stopped at its own first m with
   recall ≥ r (not reachable; it shows what per-query targeting could buy).

## 4. What would change the recommendation

- R1 is worth building if it reads at most 0.95 of R0's rows on at least four of the six datasets of 384 dimensions
  and more under none, random-25 and region-25, and never more than 1.10 on any set. The pre-test says 0.64 to 0.93
  on glove, 0.80 to 1.30 on yahoo and 0.80 to 1.35 on LAION, which does not pass.
- A stop on expected misses is worth offering as the caller's recall setting if some tail delivers a mean recall
  within 0.03 of nominal on every set of a dataset at no more than 1.10 of R0's rows at that recall. The pre-test
  says none does.
- If neither holds, the stop stays as built, and the useful by-product is the shape table: it turns ψ into a rate,
  S(ψ) rows above the running k-th best per row left unread at the margin, which is what the caller's knob means.

## 5. What the pre-test cannot say

It read whole clusters in a static order with exact spreads, so the window, the stream's stand-in spread, the
16-direction σ and the index path are untested. It used three datasets, one of them a guardrail, and 100 queries, so
differences under about 10% are noise. Its visible sets are the prep's five; the in-cluster sets, where δ_q carries
the answer, were not run.
