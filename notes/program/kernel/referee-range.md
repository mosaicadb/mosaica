# Referee's working file: the cluster cut and the sorted range

Referee for the PI. Commit read: `8c0b316f9149a4ad46d234cfe57ee3c1400413e7` (worktree `vexact-theory`, branch
`research/theory-program`). Sources read: `notes/README.md`, `notes/00-notation.md`, `notes/01-certificates.md` §1.1–1.3
and Editor's queries 6, 13, 14; `SYSTEM.md` §2.1–2.2; `rust/src/v2/q.rs` (`anc_among`, `anc_of`, `cut_params2`,
`sup_set`, `inf_set`, `isect`, `sorted_range2`, `key_interval2`, `support_pre`, `cut_pre`, `run_planned`, `NuCut`,
`nu_range`); `rust/src/v2/mod.rs` (`rows_of`, `cluster_info`, `support_bounds`, `bound_up`, `bound_down`, `quant_basis`,
`sort_by_nu`, the `x2` line of `build2`); `rust/src/index.rs` (`DA`, `load_anchors`, `check_anchors`).

Findings are appended as they are made; the verdicts and the proofs as they should stand are at the end.

## Log

### F0. What the code does (it is not what the notes say it does)

- `sorted_range2` does not search on a bound evaluated per row. It solves U(a) ≥ t and L(a) < t in closed form
  (`sup_set`, `inf_set`: with a = √x₂·cos θ the bound p·a ± r·√(x₂ − a²) is ‖(p, r)‖·√x₂·cos(θ ∓ φ), φ = atan2(r, p)),
  intersects the two key intervals of each kind, widens each by eps = DA + 10⁻⁹, and finds the rows whose **stored**
  key lies in each widened interval with two `partition_point` calls. No bound is evaluated at a rounded key, so the
  set it returns is a range of rows by construction. What has to be checked is (i) that the closed-form interval
  contains the true one, (ii) that |stored key − true key| ≤ DA, (iii) the rounding of the per-cluster constants.
- The support table is stored as f32 on a grid of 2⁻¹⁵ (`bound_up`, `bound_down`: the exact f64 extreme ± 10⁻⁹, then
  rounded outward to the grid), 4 bytes a value in memory. The notes say "2 bytes each".
- The centre correction α does not enter the cut or the sorted range on â at all. It enters `NuCut` (Proposition 1.6)
  and the levels.
- The key: `rows_of` computes a = x·m̂ in f64 from the f16 row and the f64 direction m̂ = m/‖m‖ of the stored centre,
  stores `round(clamp(a, −1, 1)·32767)` as i16, and sorts the cluster by (stored key, row). DA = 0.5/32767 + 10⁻⁷.
- x₂ = 1 + max over rows of (‖x‖² − 1)·(1 + 10⁻⁹) + 10⁻¹² (never below 1 + 10⁻¹²), from the f16 rows in f64.

### F1. The key is clamped to [−1, 1]; the bound lives on [−√x₂, √x₂] (code, unsound in a corner)

`rows_of`: `(a.clamp(-1.0, 1.0) * 32767.0).round() as i16`. For a row with x·m̂ > 1 the stored key is 1 and the error
is a − 1, which exceeds DA as soon as a > 1 + DA ≈ 1 + 1.536·10⁻⁵. Such a row needs ‖x‖ > 1 + 1.536·10⁻⁵ (f16 rounding
of a unit row gives ‖x‖ − 1 of order 5·10⁻⁶ rms, so the largest of 10⁶ rows is expected near 2.5·10⁻⁵) and an angle
to its centre direction below arccos((1 + DA)/‖x‖), a few milliradians: a singleton cluster or a cluster of
duplicates. `sorted_range2`, `key_interval2`, the `rmax` and `bmin`/`bmax` of `cluster_info`/`build2`, and the
|α|·‖m‖·DA term of the levels' fixed half-width all assume |stored − true| ≤ DA. Counter-example built in numpy
below (F6). Repair: either refuse/assert |a| ≤ 1 + 10⁻⁷ at build, or widen by eps = DA + max(0, √x₂ − 1).
To settle on a real index: count rows with |x·m̂| > 1 + 1.5·10⁻⁵ at build (one line in `rows_of`), or, from a built
index, the rows with `ahat` = ±32767 and their exact x·m̂.

### F2. Attack 1, first runs (`range_model.py`, `range_attack.py`)

Model: float64 re-implementation of `anc_of`, `cut_params2`, `sup_set`, `inf_set`, `isect`, `sorted_range2`,
`support_pre`, `bound_up/down`, the key and x₂ of the build. Small clusters (n = 3 to 32, 24 to 200 rows, 2 to 8
clusters; f16 rows; centres at 8 bits a dimension; anchors = centres or random unit rows; the nearest anchor and a
forced other one). Rows: duplicates, a row at the centre, an antipodal row, pairs at equal keys. Queries: random, the
centre direction, its negative, a stored row, 10⁻⁷ to 10⁻¹ from the centre, in the plane of two centres, the stored
centre unnormalised, orthogonal to the centre, near the negative of a row. Thresholds: rows' exact scores, one ulp
either side, 10⁻¹² to 10⁻⁸ either side, the cluster's extremes, 0, ±‖q‖. Scores are correctly rounded (`fsum` of exact
products) and compared exactly (rationals) within 10⁻¹² of t.

As built, seeds 2 and 3, 150 indexes each: 723,360 (cluster, query, threshold) checks, 66.4M row decisions offered,
21.9M rows decided. **1,714 rows decided wrongly, every one with |x·m̂| > 1 + DA** (finding F1; none with the key in
range). In this random search every wrong decision also had |t| > ‖q‖; F6 builds one with |t| < ‖q‖.
Lemma 1.5 on the real-valued bounds at the true keys: 0 non-contiguous undecided sets. The same real-valued undecided
set read in the **stored** order was non-contiguous in 3,593 checks (rows at equal stored keys with different true
keys): the lemma as written is about the true key, and the index sorts by the stored one (see the restatement).

Arrangements of decided rows along increasing key (O out, I in, u undecided), seed 2: u 107,135; uO 36,872; uI 36,267;
Ou 25,858; OuI 25,332; Iu 24,459; OuO 24,056; IuI 20,695; O 20,476; IuO 18,683; I 12,798; OI 2,215; IO 1,438. So the
"usual arrangement" of SYSTEM.md §2.2 (non-matches below, matches above: OuI) is one of four: in rows and out rows
can each sit at either end (OuO, IuI, IuO all occur). No arrangement with both an out run and an in run at the same
end was seen.

Repair check (same seed 2, eps = DA + 10⁻⁹ + (√x₂ − 1)): 369,600 checks, **0 wrong**; rows decided fall from 10.73M
to 10.57M in this low-dimensional model, where √x₂ − 1 is near 10⁻³.

The real 1M index is exposed in principle: `rust/results/run1m.log` and `scale.log` print "x2 bound 4.882e-4" (that is
2⁻¹¹), so √x₂ − 1 = 2.44·10⁻⁴, sixteen times DA. A row at the top of that norm range is clamped past DA when its angle
to its centre direction is under about 21 mrad (1.2°). The 8-bit centre of a cluster of duplicates sits about 8 mrad
from its rows at n = 768 (my estimate, not measured), so duplicates with a norm near the top would qualify. Whether any
row does is a count over the keys; I did not read the index.

### F3. The square roots of cancelling differences are not rounded up (code, unsound in a corner)

`anc_of`: rho = √max(qn² − β², 0)·(1 + 10⁻¹²) + 10⁻¹². `cut_params2`: s1 = √max(qn² − cm², 0)·(…) + 10⁻¹² and
rp = √max(rho² − rm², 0)·(…) + 10⁻¹². Exactly, ‖q − βp̂‖² = qn² − β² + β²(‖p̂‖² − 1) + 2β(β − q·p̂), and the computed
difference carries the rounding of qn², β² and β, an absolute error η of order 10⁻¹⁶·qn² (worst case about
2n·2⁻⁵³·qn², and up to 2·10⁻¹²·β² for a loaded anchor, which `check_anchors` admits at norm 1 ± 10⁻¹²). The square
root turns η into a deficit of η/(2ρ), and of √η ≈ 10⁻⁸ when ρ itself is of that size. The slack 10⁻¹² covers it only
for ρ ≳ 10⁻⁴; the 10⁻⁹ on the threshold covers it for ρ ≳ 10⁻⁷.

`range_corner.py` part B (exact values in rationals): for a query equal to a centre's int8 vector (exact in f16; the
stored centre is that vector times a scale, rounded to f32, `index::centres_of`), rho as coded was below the exact
‖q − βp̂‖ in 219 of 400 queries at n = 32 (largest deficit 1.83·10⁻⁸, above 10⁻⁹ in 101) and in 45 of 60 at n = 768
(largest 1.36·10⁻⁸, above 10⁻⁹ in 13); s1 the same (209 of 400, 49 of 60).

`range_corner_c.py`: with that query and a top row whose p̂·x lies just under a point of the table's 2⁻¹⁵ grid,
`support_pre` returns Skip for the cluster at t = the top row's exact score. Three printed cases (n = 8), e.g.
rho as coded 1.0·10⁻¹², exact 1.726·10⁻⁸; β·hi + rho·√x₂ = 0.827366226830 < t − 10⁻⁹ = 0.827366231872 while the row
scores exactly t. **A matching row is skipped with its cluster.** 8 such cases from 400 centres tried. This is the
model's arithmetic; the binary sums q·p̂ in another order, which changes which queries hit it and not whether the
formula is safe.

In the sorted range the same deficit in s1 and rp is in practice hidden by the 10⁻⁷ in DA (which shifts the key by
10⁻⁷ and so the bound by about |p|·10⁻⁷ when r is tiny), but nothing written says so, and the worst-case η is larger
than that margin.

Repair: add the rounding before the root, rho = √(max(qn² − β², 0) + c·n·2⁻⁵³·qn² + 2·10⁻¹²·β²) with c ≈ 4, and the
same for s1 and rp (for rp the term is 2|rm|·(error of rm)). It costs under 10⁻⁶ of width at ρ = 0 and nothing
measurable elsewhere.

### F6. The clamp at an ordinary threshold (`range_corner.py` part A)

n = 16, two clusters, anchors = the two centre directions, chosen orthogonal. Cluster c is six near-duplicates along
its centre direction m̂; one has norm 1 + 1.197·10⁻⁴ (inside the 1M index's √x₂ − 1 = 2.44·10⁻⁴) and true key
1 + 1.197·10⁻⁴, stored 32767; it has the cluster's largest p̂·x. Query q = unit(0.75·p̂ + 0.65·m̂) rounded to f16
(‖q‖ = 0.99996; its nearest anchor is p̂, the other cluster's centre), t = that row's exact score 0.655012466.
The table bound's not-excluded key interval is [1.0000885, 1.0001616], wholly above 1 + DA + 10⁻⁹, so the stored key
1.0 falls outside the widened interval and **the row is decided out although its score equals t**. With
eps = DA + 10⁻⁹ + (√x₂ − 1) it is left undecided.

What it takes: a row with x·m̂ > 1 + DA, and either the query within a few mrad of ±m̂ with |t| near ‖q‖·√x₂ (the
cases of F2), or q − βp̂ within about √((√x₂ − 1 − DA)/2) ≈ 10 mrad of ±m̂ with that row at the table's extreme (this
case), at any threshold.

### F7. Further runs

- `range_attack.py` seed 5 with one more query kind (a centre's int8 vector, exact in f16): 148,896 checks, 15 wrong,
  all with |x·m̂| > 1 + DA. Totals as built over seeds 2, 3, 5: 872,256 checks, 81.8M row decisions offered, 26.8M rows
  decided, 1,729 wrong, all clamped rows; real-valued undecided set non-contiguous in stored order in 3,887 checks,
  never in true-key order.
- `closed_form_check.py`: `sup_set` and `inf_set` against the bound on a grid of 4,003 keys, 20,000 random (p, r, R, t)
  including r down to 10⁻⁹ and p = 0: 0 grid points satisfying the inequality outside the returned interval, and 0
  failing it more than 10⁻⁹ inside. The closed forms are exact, so the code's interval is the lemma's.
- `nucut_attack.py` (Proposition 1.6 as built: int8 basis with a scale per vector, Newton-polished inverse, e_inv, ν
  rounded up to 16 bits on the cluster's grid, b_min/b_max and r_max from the stored keys, three values of α, the
  cutoffs and their margins; n = 4 to 32), seeds 1 to 3: 244,800 checks, 15.0M row decisions offered, 1.87M decided;
  748 wrong decisions and 979 exact scores outside the bounds of their norm code, **all rows with |x·m̂| > 1 + DA**
  (m·r leaves [b_min, b_max], finding F1), none otherwise.

### F8. Other departures of the notes from the code

- The support table is f32 on a 2⁻¹⁵ grid, 4 bytes a value in memory (`Index2::hi`, `lo`), not "2 bytes each".
- DA is half the key's quantum plus 10⁻⁷; the notes call δ_â the quantum.
- `sorted_range2` puts 10⁻⁹ on the threshold as well as widening the key interval; the notes mention the widening only.
- e_inv = (computed Frobenius defect of V·X)(1 + 10⁻⁶) + 10⁻¹². It does not bound the rounding of z̃ = fl(X·r) or of
  the defect itself (my estimate with standard bounds: up to about 5·10⁻¹⁰ relative at n = 768 and condition 227,
  typically far less). The +10⁻⁹ in `NuCut`'s E and the 4·10⁻⁶ of the levels' fixed term absorb it; no source says so.
  It belongs with the gap named in 1.10.
- `support_bounds`: the f32 screen (rows within twice the f32 error of the f32 extreme are recomputed in f64) is argued
  correctly for unit anchors and any summation order; `bound_up`/`bound_down` add 10⁻⁹ before rounding to the grid.
- The centre's 8 bits have no effect on soundness: the key, q·m and p̂·m̂ all use the same stored centre and the same
  m̂ = m/‖m‖ in f64.

## Verdicts

| Result | The mathematics | The code |
|---|---|---|
| Lemma 1.1, Lemma 1.2 | No error found. Lemma 1.2's first identity needs no unit norm; the second is ‖r‖² = ‖x‖² − 2‖m‖â + ‖m‖² in general. | The error ‖m‖·δ of the stored key holds only for \|x·m̂\| ≤ 1 + δ (F1). |
| Proposition 1.3 (centre correction) | No error found. It plays no part in the cut or the sorted range on â. | Not examined beyond `NuCut`. |
| Proposition 1.4 (cluster cut) | No error found; holds for any p̂ and any β once ρ_⊥ ≥ ‖q − βp̂‖. The remark ρ_⊥ = √(1 − β²) needs ‖p̂‖ = 1 and β = q·p̂ exactly. | Unsound in a corner (F3): ρ is not rounded up across the cancellation in qn² − β². Model counter-example. |
| Lemma 1.5 (sorted range) | Correct for exact keys, and the transcriber's proof is correct. Correct after a stated repair for the index: it must be stated for the stored key with a widening, and must not say at which end the decided rows lie. | Unsound in a corner (F1): the key is clamped to [−1, 1] and the widening assumes an error of at most DA. Model counter-examples. Otherwise sound: closed forms exact, both ends inclusive on the safe side, no off-by-one. |
| Proposition 1.6 (norm sort) | No error found in the restatement or its proof, basis term included. | Sound in the model except through F1 (b_min, b_max, r_max come from the clamped key). The hypothesis e_inv ≥ ‖ε‖/‖r‖ is not established as a bound (F8). |

Editor's query 6: SYSTEM.md §2.2 is wrong in general; see F2 and the remark after Lemma 1.5 below. For the annulus
bound alone, with φ the angle of q to m̂ and ψ = arccos(t/(‖q‖√x₂)): the high-key end is out when φ > ψ and in when
φ < ψ; the low-key end is out when φ + ψ < π and in when φ + ψ > π. Editor's query 13: the restatement and its proof
are right.

## The statements and proofs as they should stand

Setting for 1.4 and 1.5. A cluster has a unit direction m̂. Every row has ‖x‖² ≤ x₂; write R = √x₂ and â = x·m̂, so
|â| ≤ R. The stored key ã satisfies |ã − â| ≤ δ, and the rows are sorted by ã. For an anchor vector p̂ the cluster stores
lo_c ≤ p̂·x ≤ hi_c over its rows.

**Proposition 1.4.** Let q, p̂ be any vectors and β any number, q_⊥ = q − βp̂, ρ_⊥ ≥ ‖q_⊥‖,
β̄_c = max(β·lo_c, β·hi_c), β̲_c = min(β·lo_c, β·hi_c). Then every row of the cluster satisfies
β̲_c − ρ_⊥R ≤ q·x ≤ β̄_c + ρ_⊥R. If ‖p̂‖ = 1 and β = q·p̂ then ‖q_⊥‖² = ‖q‖² − β².

*Proof.* q·x = β(p̂·x) + q_⊥·x. Since lo_c ≤ p̂·x ≤ hi_c, the number β(p̂·x) lies between β·lo_c and β·hi_c, in either
order according to the sign of β, hence in [β̲_c, β̄_c]. By Cauchy–Schwarz |q_⊥·x| ≤ ‖q_⊥‖‖x‖ ≤ ρ_⊥R. For the last
claim, ‖q − βp̂‖² = ‖q‖² − 2β(q·p̂) + β²‖p̂‖² = ‖q‖² − β². ∎

**Lemma 1.5a (two bounds from the key).** For a vector v write v_m = v − (v·m̂)m̂. For every row,
(i) |q·x − (q·m̂)â| ≤ ‖q_m‖·√(R² − â²);
(ii) β̲_c + (q_⊥·m̂)â − ‖(q_⊥)_m‖·√(R² − â²) ≤ q·x ≤ β̄_c + (q_⊥·m̂)â + ‖(q_⊥)_m‖·√(R² − â²).
Here ‖v_m‖² = ‖v‖² − (v·m̂)², and q_⊥·m̂ = q·m̂ − β·p̂·m̂.

*Proof.* Write x = âm̂ + x_m with x_m ⟂ m̂, so ‖x_m‖² = ‖x‖² − â² ≤ R² − â². For any v,
v·x = (v·m̂)â + v·x_m = (v·m̂)â + v_m·x_m, because x_m ⟂ m̂, and |v_m·x_m| ≤ ‖v_m‖√(R² − â²). Taking v = q gives (i).
For (ii), q·x = β(p̂·x) + q_⊥·x; the first term lies in [β̲_c, β̄_c] as in Proposition 1.4, and v = q_⊥ bounds the
second. ∎

Each bound has the form U(a) = c + p·a + r√(R² − a²) or L(a) = c′ + p·a − r√(R² − a²) on [−R, R] with r ≥ 0, and stays
valid when r and c are replaced by larger numbers and c′ by a smaller one.

**Lemma 1.5b (shape).** U is concave and L convex on [−R, R], both continuous. For every t, {a : U(a) ≥ t} is a closed
interval and {a : L(a) < t} an interval open in [−R, R]; either may be empty. With a = R cos θ, θ ∈ [0, π],
ϱ = √(p² + r²) and φ = atan2(r, p) ∈ [0, π]: U = c + ϱR cos(θ − φ) and L = c′ + ϱR cos(θ + φ).

*Proof.* a ↦ √(R² − a²) is continuous on [−R, R] with second derivative −R²(R² − a²)^(−3/2) < 0 inside, so it is
concave; r ≥ 0 and p·a is linear, so U is concave and L convex. If U(a₁) ≥ t and U(a₂) ≥ t then for λ ∈ [0, 1],
U(λa₁ + (1 − λ)a₂) ≥ λU(a₁) + (1 − λ)U(a₂) ≥ t, so the set is convex, hence an interval, closed by continuity. The
same argument with L(λa₁ + (1 − λ)a₂) ≤ λL(a₁) + (1 − λ)L(a₂) < t gives the second. The trigonometric forms follow from
p = ϱ cos φ, r = ϱ sin φ and √(R² − a²) = R sin θ. ∎

Nothing here depends on the sign of p (that is, of q·m̂ or q_⊥·m̂), on the sign or size of t, or on α, which does not
occur. At a = ±R the square root is 0 and both bounds equal c ± pR.

**Lemma 1.5 (one contiguous undecided range, for the stored key).** Let U₁, U₂ and L₁, L₂ be the bounds of Lemma 1.5a,
t a threshold, I_U = {U₁ ≥ t} ∩ {U₂ ≥ t} and I_L = {L₁ < t} ∩ {L₂ < t}. Let J_U ⊇ I_U and J_L ⊇ I_L be intervals, and
J^δ the set of points within δ of J. Call a row *out* if ã ∉ J_U^δ, *in* if ã ∈ J_U^δ and ã ∉ J_L^δ, and *undecided*
if ã ∈ J_U^δ ∩ J_L^δ. Then
(a) every out row has q·x < t;
(b) every in row has q·x ≥ t;
(c) in the order of ã the rows that are not out form one contiguous range, the undecided rows form one contiguous
range inside it, and the in rows are the at most two ranges between them;
(d) with exact keys (δ = 0, ã = â, J = I) the undecided rows are exactly those with min(U₁, U₂)(â) ≥ t and
max(L₁, L₂)(â) < t.
The lemma does not say at which end the out rows or the in rows lie; each kind can lie at either end, or at both.

*Proof.* I_U and I_L are intervals by Lemma 1.5b, as intersections of two intervals. (a) If ã ∉ J_U^δ then, as
|ã − â| ≤ δ, â ∉ J_U ⊇ I_U, so U_i(â) < t for some i, and q·x ≤ U_i(â) < t by Lemma 1.5a. (b) If ã ∉ J_L^δ then
â ∉ I_L; since â ∈ [−R, R], L_i(â) ≥ t for some i, and q·x ≥ L_i(â) ≥ t. (c) J_U^δ and J_L^δ are intervals, and so is
their intersection. Membership of a row in each depends on ã alone, so rows with equal keys fall together, and the
rows whose sort key lies in an interval are contiguous in the sort order. The in rows are those of the first range
outside the second, which is nested in it. (d) is the definition. ∎

The index takes J_U = cl{U₁ ≥ t − 10⁻⁹} ∩ cl{U₂ ≥ t − 10⁻⁹} and J_L = cl{L₁ < t + 10⁻⁹} ∩ cl{L₂ < t + 10⁻⁹} from the
closed forms of Lemma 1.5b, and δ = DA + 10⁻⁹. The hypothesis |ã − â| ≤ δ is F1; the upward rounding of the bounds'
constants is F3.

**Proposition 1.3.** (a) For unit q and α = q·m/‖m‖², Δ = q − αm is orthogonal to m and ‖Δ‖² = 1 − (q·m̂)².
(b) For positive diagonal D with Dm ≠ 0, ‖D(q − αm)‖ is least at α* = ⟨Dq, Dm⟩/‖Dm‖².
*Proof.* (a) Δ·m = q·m − α‖m‖² = 0, and ‖Δ‖² = 1 − 2α(q·m) + α²‖m‖² = 1 − (q·m)²/‖m‖². (b) The square is
‖Dq‖² − 2α⟨Dq, Dm⟩ + α²‖Dm‖², a convex quadratic in α with vertex α*. ∎

**Proposition 1.6.** Let V be the stored basis, and for each row let z̃_x be the stored coordinates of r = x − m,
ε = r − Vz̃_x, with ‖ε‖ ≤ e_inv‖r‖. Let ν_x ≥ ‖z̃_x‖, b_min ≤ m·r ≤ b_max and r_max ≥ ‖r‖ over the cluster. For any α
chosen from q and the cluster alone, Δ = q − αm, D_α ≥ ‖VᵀΔ‖ and E_α ≥ e_inv(‖q‖ + |α|‖m‖)r_max,
q·m + min(αb_min, αb_max) − D_αν_x − E_α ≤ q·x ≤ q·m + max(αb_min, αb_max) + D_αν_x + E_α.
The upper end is non-decreasing and the lower end non-increasing in ν_x, and so are the least upper end and the
greatest lower end over any finite set of α. Hence in a cluster sorted by ν_x the rows with upper end below t form a
prefix, the rows with lower end at least t form a prefix, and at most one of the two prefixes is non-empty.

*Proof.* q·x = q·m + q·r = q·m + α(m·r) + Δ·r. The term α(m·r) lies between αb_min and αb_max. Next
Δ·r = Δ·Vz̃_x + Δ·ε = (VᵀΔ)·z̃_x + Δ·ε. By Cauchy–Schwarz |(VᵀΔ)·z̃_x| ≤ ‖VᵀΔ‖‖z̃_x‖ ≤ D_αν_x, and
|Δ·ε| ≤ ‖Δ‖e_inv‖r‖ ≤ (‖q‖ + |α|‖m‖)e_inv r_max ≤ E_α. This gives the two inequalities. D_α ≥ 0, so the ends are
monotone in ν_x; a minimum of non-decreasing functions is non-decreasing and a maximum of non-increasing ones
non-increasing. A monotone condition on the sort key holds on a prefix. If both prefixes were non-empty the first row
would have upper end below t and lower end at least t, contradicting lower ≤ q·x ≤ upper. ∎

The parenthesis of the notes' statement (‖z̃‖ = ‖r‖ in the M⁻¹ norm for the exact dual basis) rests on §1.4, which I
did not referee.

## Not settled without the binary or the data

1. Whether any row of a real index has |x·m̂| > 1 + DA. Count, at build in `rows_of`, the rows with
   `a.abs() > 1.0 + DA`; or on a built index, the rows with `ahat` = ±32767 and their x·m̂ in f64. On the 1M
   BioCLIP-2 index x₂ allows it (√x₂ − 1 = 2.44·10⁻⁴).
2. That the binary reproduces F1 and F3. Verify mode (`vexact --v2 --index DIR verify`) compares answers with brute
   force but takes its thresholds from answer sizes, so it does not reach either corner. Two unit tests beside
   `e2e.rs` would: (i) an index holding a row of norm 1.0002 along its centre, the query of `range_corner.py` part A,
   t = the row's exact score, expecting the row in the answer; (ii) `support_pre` with the query equal to a centre's
   int8 vector and a top row placed just under a table grid point, as `range_corner_c.py`.
3. The worst-case rounding of β, q·m and the matrix products: I used standard bounds, not the binary's summation
   order.
