# 1. Deterministic certificates, the dual basis, the cluster-level steps, the codes

An exact query decides most items by bounds that hold for every draw of the seed ω, and only the items these bounds
leave undecided reach the randomised levels of chapter 3. This chapter sets out the deterministic part. It shows how
the score splits into known and unknown terms, how whole clusters and the two ends of each cluster are decided from a
table and a sort key, how a level's stored error norm bounds the unread part of the score, which metric makes that
bound tightest, what a grouping of clusters can and cannot buy, what the codes are and how far they sit from the floors
of chapter 2, and why the integer kernel's decisions are implied by the real-valued bounds. Results of the theory
program's new tracks are left out because none of them has been refereed.

Unless a passage says otherwise, "the index" is the native prototype described in `SYSTEM.md` (1M BioCLIP-2 image
embeddings, n = 768, f16 rows, K = 999 clusters, G = 64 groups), and "text" or "OOD" queries are embeddings of species
prompts searched against the image embeddings.

## Local symbols

The symbols of chapter 0 apply. This chapter adds the following.

| Symbol | Meaning |
|---|---|
| m̂ | m/‖m‖, the direction of the cluster's stored centre |
| â | the sort key x·m̂ of a row, stored in 16 bits; δ_â is its quantum |
| p̂, β | the anchor nearest the query, and β = q·p̂ |
| q_⊥, ρ_⊥ | q_⊥ = q − βp̂, and ρ_⊥ = ‖q_⊥‖ (the cut's remainder norm of chapter 0) |
| lo_c, hi_c | the stored extremes of p̂·x over cluster c, rounded outward |
| x₂ | a bound on ‖x‖² for every stored row (1 up to f16 rounding) |
| D, D_ℓ | the positive diagonal weights of a deterministic level ℓ |
| e, e_ℓ | the coding error a level leaves, in the coded coordinates |
| ν_ℓ | the stored, rounded-up value of ‖D_ℓ⁻¹e_ℓ‖ |
| V_q, z | a group's query-side basis and the item's coded coordinates z = V_q⁻¹r |
| η | the item scale derived from the key (the sources write s, which chapter 0 keeps for the score) |
| ŝ_ℓ(x) | the decoded partial score of x through level ℓ, a sum of table entries |
| ψ | the caller's dial on the deterministic half-width; ψ = 1 gives the guarantee |
| σ | the integer kernel's table step (unsubscripted; chapter 5's σ_c does not appear here) |
| **c** | the message an encoder stores for one item, as in chapter 2 |
| N_g, B_s | the number of items that share a summary and the summary's bits, as in chapter 2's Corollary 2.4 (the sources write M and S, which chapter 0 keeps for the metric and the answer set) |
| j (Proposition 1.15 only) | the label of the cell of a partition, as in chapter 2's Proposition 2.8 (the source writes ℓ, which this chapter keeps for a level) |
| φ_q, φ_x (Proposition 1.7 only) | the angles of q and x to a representative p, as in chapter 2's Lemma 2.7 |
| ν (Propositions 1.11, 1.12) | a stored value of ‖W⁻¹e‖ for weights W, as ν_ℓ is of ‖D_ℓ⁻¹e_ℓ‖ (the source writes s, which chapter 0 keeps for the score) |
| g(b), g_tree(b) | the distortion of a b-bit uniform or tree quantiser on a unit Gaussian (§1.3, §1.6); g in §1.8 is a word of the compare |
| B (§1.4 only) | the matrix Σ_Q^½ Σ_R Σ_Q^½, as in `groups/REPORT.md` §2.4; elsewhere B is a number of bits, as in chapter 0 |
| k | in Proposition 1.12 the free parameter of the two-norm bound, as in the sources; in Corollary 1.14 and Proposition 1.15 the dimension of a subspace, as in chapter 2; elsewhere the number of neighbours, as in chapter 0 |
| K_in, K_out, T_in, T_out, R_ℓ, B_ℓ, Θ, U, h, d, g (§1.8 only) | the integer kernel's thresholds, margins, centring remainder, accumulator, unit count, high-part width and the two words of its compare, as in `reports/surge/integer-kernel.txt`; they do not carry chapter 0's meanings there |

## 1.1 The decomposition of the score

Each row x belongs to one cluster, whose centre m is stored at 8 bits per dimension. The residual r = x − m is taken
against the stored centre, so the identity q·x = q·m + q·r holds exactly (`SYSTEM.md` §2.1).

**Lemma 1.1** (the Δ decomposition; `FINDINGS.md` §4 item 2 for α = 1, `agent2/REPORT.md` "Fixes" and
`reports/surge/integer-kernel.txt` §0 for general α). [proved, unrefereed]
For every real α, with Δ = q − αm,

    q·x = q·m + α·m·r + Δ·r.

*Proof.* Δ·r = q·r − α·m·r. Adding q·m + α·m·r gives q·m + q·r, which is q·x. ∎

The first term is one number per (query, cluster), and Lemma 1.2 shows that the second is known once the item's key is
read, so only Δ·r is uncertain. The identity holds for every α, which means α affects the width of a bound and never its
soundness. At a randomised level α must also be fixed independently of ω, because the error law of chapter 3 is stated
for a direction chosen without knowledge of the rotation.

Bounding Δ·r in place of the full coding error q·e tightened the bounds about threefold [measured] on the BioCLIP-2
sample of `FINDINGS.md` §4, which names no script for this figure.

**Lemma 1.2** (the key gives m·r and ‖r‖; `quant/REPORT.md` "The code as kept" item 3, `entropy/REPORT.md` §4,
`reports/surge/level0/REPORT.txt` §1). [proved, unrefereed] For a unit row x with key â = x·m̂,

    m·r = ‖m‖â − ‖m‖²,    ‖r‖² = 1 − 2‖m‖â + ‖m‖².

*Proof.* m·x = ‖m‖â, so m·r = m·x − ‖m‖². Expanding ‖x − m‖² with ‖x‖ = 1 gives the second identity. ∎

The two identities together give m·r = (1 − ‖m‖² − ‖r‖²)/2, the form used in `reports/surge/level0/REPORT.txt`, so a
separately stored m·r scalar or residual norm carries no information the key does not. Taking the item scale and the
residual norm from the key in this way cut per-item bytes by 1–8% [measured] in `quant/REPORT.md` (BioCLIP-2 1M with
200 item and 369 text queries, and four other datasets; `run2.py`). The key is stored in 16 bits, so the m·r it gives
is off by at most ‖m‖·δ_â, and the built certificate adds |α|·‖m‖·δ_â to its fixed term (§1.3). The identity needs
‖x‖ = 1; the index covers f16 rounding of the norm through x₂.

**Proposition 1.3** (the centre correction).

(a) (Theorem B(i) of `reports/surge/level0/REPORT.txt` §1.) [proved, refereed] For unit q and α = q·m/‖m‖², the
offset Δ = q − αm is orthogonal to m and ‖Δ‖ = √(1 − (q·m̂)²) ≤ 1. Because α depends only on q and the centre, Δ is
fixed independently of ω.

*Proof.* Δ·m = q·m − α‖m‖² = 0. Then ‖Δ‖² = ‖q‖² − 2α·q·m + α²‖m‖² = 1 − (q·m)²/‖m‖² = 1 − (q·m̂)². ∎

The referee (`reports/surge/referee.txt` §3) found (a) correct, and added that the stored scalar of the source's
estimator is a cached function of the code, the centre and the rotation, so it adds no information. Part (a) is
Theorem 3.11 (i) of chapter 3, where it is used with the randomised level's estimator.

(b) (`agent2/REPORT.md` "Fixes", `combined/REPORT.md` item 3.) [proved, unrefereed] For positive diagonal D, the α
that minimises ‖D(q − αm)‖ is α* = ⟨Dq, Dm⟩/‖Dm‖².

*Proof.* ‖D(q − αm)‖² = ‖Dq‖² − 2α⟨Dq, Dm⟩ + α²‖Dm‖² is a convex quadratic in α with its minimum at α*. ∎

A deterministic level's margin is proportional to ‖D_ℓΔ‖ (Proposition 1.8), so α* minimises that level's margin. The
index computes one α per (query, cluster) in floating point from the weighted query and centre coordinates, uses it at
every level, and evaluates each level's ‖D_ℓΔ‖ at that α with an allowance for f32 rounding (`rust/src/v2/q.rs`,
cluster setup).

Measured effects [measured]:

- In the ablation of `combined/REPORT.md` (numpy prototype `core.py`, 1M BioCLIP-2, 200 item and 369 text queries),
  adding α* to every deterministic level cut per-item bytes by 9 / 7 / 4 / 2% for item queries and 7 / 6 / 4 / 2% for
  text queries at answers of 0.01 / 0.1 / 1 / 5%.
- A native variant that shared tables across clusters by setting α = 0 read 40–46% more bytes
  (`rust/REPORT2.md`, as reported in `reports/surge/integer-kernel.txt` §3).
- With α = q·m/‖m‖², ‖Δ‖ over true neighbours was 0.80–0.81 on GloVe (n = 100, K = 4,096) and 0.93 on NYTimes
  (n = 256, K = 1,024) at k = 10 (`reports/surge/level0/REPORT.txt` §1, Theorem B(i); scripts `lvl0*.py` in that
  directory).

## 1.2 The cluster cut and the sorted range

Before any code byte is read, the query rules out or takes in whole clusters and the two ends of each remaining
cluster. Every centre direction is an anchor in the index as built (`SYSTEM.md` §2.1; the design of `DESIGN.md` §2 also
had 256 text-prompt anchors). For each anchor p̂ and cluster c the build stores lo_c ≤ p̂·x ≤ hi_c over the cluster's
rows, rounded outward, in 2 bytes each.

**Proposition 1.4** (the cluster cut; `SYSTEM.md` §2.2, `DESIGN.md` §3 steps 1–2, `agent3b/REPORT.md` §2 C2).
[proved, unrefereed] Let β̄_c = max(β·lo_c, β·hi_c) and β̲_c = min(β·lo_c, β·hi_c). Every row x of cluster c with
‖x‖ ≤ √x₂ satisfies

    β̲_c − ρ_⊥√x₂ ≤ q·x ≤ β̄_c + ρ_⊥√x₂.

*Proof.* q·x = β·p̂·x + q_⊥·x. The first term lies between β̲_c and β̄_c, and |q_⊥·x| ≤ ρ_⊥‖x‖ by Cauchy–Schwarz. ∎

A cluster whose lower bound reaches t is taken in unread, and one whose upper bound is below t is skipped unread. For a
unit query ρ_⊥ = √(1 − β²). The extremes stay valid when rows are deleted and need only widen when rows are inserted,
which is why the design keeps them for the write path (`agent3b/REPORT.md` §2; the write path is not built).

In each cluster that survives the cut, the query bounds each row from its key â alone, in two ways. Write
x = â·m̂ + x_m with x_m ⊥ m̂, so that ‖x_m‖ ≤ √(x₂ − â²).

1. *Annulus.* With q = (q·m̂)m̂ + q_m and q_m ⊥ m̂, q·x = (q·m̂)â + q_m·x_m, so
   q·x ∈ (q·m̂)â ± ‖q_m‖·√(x₂ − â²).
2. *Table plus remainder.* q·x = β·p̂·x + q_⊥·x and q_⊥·x = (q_⊥·m̂)â + (q_⊥ − (q_⊥·m̂)m̂)·x_m, so
   β̲_c + (q_⊥·m̂)â − ‖q_⊥ − (q_⊥·m̂)m̂‖·√(x₂ − â²) ≤ q·x ≤ β̄_c + (q_⊥·m̂)â + ‖q_⊥ − (q_⊥·m̂)m̂‖·√(x₂ − â²).
   The number q_⊥·m̂ = q·m̂ − β·p̂·m̂ is one per (query, cluster).

Each upper bound has the form U(â) = a₀ + a₁â + a₂√(x₂ − â²) with a₂ ≥ 0, and each lower bound the form
L(â) = b₀ + b₁â − b₂√(x₂ − â²) with b₂ ≥ 0.

**Lemma 1.5** (one contiguous undecided range; `agent3b/REPORT.md` §2 C1, `combined/REPORT.md` item 2,
`DESIGN.md` §3 step 3). [proved, unrefereed] Within a cluster whose rows are sorted by â, the rows that neither pair of
bounds decides form one contiguous range of the sort order.

*Proof.* On [−√x₂, √x₂] the function â ↦ √(x₂ − â²) is concave, so each U is concave and each L is convex. A row is not
excluded when min(U₁, U₂) ≥ t, that is when â lies in {U₁ ≥ t} ∩ {U₂ ≥ t}. A superlevel set of a concave function on an
interval is an interval, and so is an intersection of two intervals. A row is not included when max(L₁, L₂) < t, that
is when â lies in {L₁ < t} ∩ {L₂ < t}, and a strict sublevel set of a convex function is likewise an interval. The
undecided rows are those whose key lies in the intersection of these two intervals, which is an interval I, and the
rows whose key lies in an interval are contiguous when the rows are sorted by key. ∎

Lemma 4.10 of chapter 4 states the annulus case of this lemma, from the same source (`agent3b/REPORT.md` §2 C1), and
uses it for the "range" cut of its scaling measurements. The sources state this argument in one line ("the upper bound is concave in a and the lower convex", and "intersection
of two intervals"). It is not among the consolidated proofs, and no referee has checked it. The query finds the ends of
I by binary search, and because the stored key is rounded, the index widens each end by δ_â + 10⁻⁹ before searching
(`rust/src/v2/q.rs`, `sorted_range2`). Rows outside the not-excluded interval are decided out and rows inside it but
outside I are decided in; the visible rows in I are the cluster's candidates.

**Proposition 1.6** (sorting by the residual's norm; `groups/REPORT.md` §3.3, `DESIGN.md` §9 item 2).
[proved, unrefereed] Let V_q be the stored basis and z̃_x the coordinates the encoder stores for r, as in Lemma 1.9.
Suppose each row stores ν_x ≥ ‖z̃_x‖, and the cluster stores b_min ≤ m·r ≤ b_max and r_max ≥ ‖r‖ over its rows.
(For the exact dual basis, ‖z̃_x‖ = ‖r‖_{M⁻¹} and ‖V_qᵀΔ‖ = ‖Δ‖_M, by §1.4.) Then for any α fixed per (query,
cluster), with Δ = q − αm, D_α ≥ ‖V_qᵀΔ‖ and E ≥ e_inv·(‖q‖ + |α|·‖m‖)·r_max,

    q·m + min(αb_min, αb_max) − D_α·ν_x − E ≤ q·x ≤ q·m + max(αb_min, αb_max) + D_α·ν_x + E,

and both ends are monotone in ν_x, so in a cluster sorted by ν_x the rows decided in or out form a prefix.

*Proof.* By Lemma 1.1, q·x = q·m + α·m·r + Δ·r, and α·m·r lies between αb_min and αb_max. By Lemma 1.9 with
u = Δ, Δ·r = (V_qᵀΔ)·z̃_x + Δ·ε with |Δ·ε| ≤ e_inv·‖Δ‖·‖r‖ ≤ E, because ‖Δ‖ ≤ ‖q‖ + |α|·‖m‖. Cauchy–Schwarz gives
|(V_qᵀΔ)·z̃_x| ≤ D_α·ν_x. ∎

`groups/REPORT.md` §3.3 states the bound in one sentence, in exact coordinates and without E; the statement above, with
the basis-rounding term, is the one in the comment on `NuCut` in `rust/src/v2/q.rs` (Editor's query 13).

The index as measured sorts by â. The ν-sort is implemented in the code (`NuCut` in `rust/src/v2/q.rs`, which takes the
best of three values of α), but `dualproto/REPORT.md` §7 lists it as not run natively, and `SYSTEM.md` does not use it.
With it, the support table and its anchors could be dropped.

**Proposition 1.7** (one representative; `groups/REPORT.md` §2.1). [proved, unrefereed] For unit q, x and p with
φ_q = ∠(q, p) and φ_x = ∠(x, p), and n ≥ 3, the tightest interval for q·x that depends on φ_q and φ_x alone is
[cos(φ_q + φ_x), cos(φ_q − φ_x)], of width 2 sin φ_q sin φ_x.

*Proof.* Write q = cos φ_q·p + sin φ_q·u and x = cos φ_x·p + sin φ_x·v with u, v unit vectors orthogonal to p. Then
q·x = cos φ_q cos φ_x + sin φ_q sin φ_x·(u·v), and for fixed u the product u·v takes every value in [−1, 1] as v
ranges over the unit sphere of p^⊥. ∎

Lemma 2.7 of chapter 2 is the same result from the same source; its Editor's query 7 discusses the case n = 2.

The annulus bound is this proposition with p = m̂. The slack is the product of the query's distance from the
representative and the member's, and `groups/REPORT.md` §2.1 argues that neither factor can be made small: the member's
distance falls only as K^(−0.053) (§1.5 below), and the query's needs a representative near a fresh query.

Measured effects of the cut and the range [measured]:

- In the index at a 1% answer, the candidates are 22% of rows for in-distribution queries and 95% for OOD queries
  (`SYSTEM.md` §2.2).
- In the native version 2 prototype, the share of items touched after the cut and the range was 13 / 22 / 47 / 84% for
  item queries and 75 / 83 / 94 / 99.6% for text at answers of 0.01 / 0.1 / 1 / 5%, the same in every basis variant
  (`dualproto/REPORT.md` §3, 200 item and 369 text queries). See Editor's query 3.
- With exact coordinates (`groups/REPORT.md` §3.1, `scripts/measure.py`, K = 1,000), the sorted range on â alone left
  80.2 / 86.6 / 95.6 / 99.7% of rows undecided for text and 30.9 / 44.8 / 72.7 / 97.7% for items; adding the support
  table with 1,000 anchors left 74.8 / 82.7 / 94.1 / 99.6% and 13.8 / 23.2 / 47.8 / 84.2%. An oracle that knew each
  cluster's exact extremes would leave 0.97 / 3.32 / 15.1 / 44.8% and 0.66 / 1.57 / 6.54 / 28.3%.
- The table depends on the ratio of anchors to clusters. For item queries at 1M, rows touched rose from
  12.9 / 20.6 / 38.9 / 76.0% with 1,000 anchors to 27.6 / 39.9 / 59.8 / 96.3% with the range alone, and at the
  0.01% answer every centre was opened with 16 anchors or fewer (68% with 1,000) (`review/REPORT.md` §1.3,
  `e1_anchors.py`). The review conjectures that 16–64
  anchors is the regime comparable to 10⁹ rows [conjecture].
- Sorting by the norm of Proposition 1.6 left 61.1 / 70.8 / 85.7 / 97.0% of rows for text and
  13.3 / 22.1 / 47.0 / 83.8% for items under the data-only fit, with nothing read per member and no table
  (`groups/REPORT.md` §3.3, numpy, exact coordinates).

## 1.3 Weighted Cauchy–Schwarz with a stored norm

A deterministic level ℓ codes the item's coordinates z, scaled by 1/η, and leaves an error e_ℓ. The unread part of the
score is then Δ·e_ℓ in the coded coordinates, and the level bounds it from one stored scalar.

**Proposition 1.8** (weighted Cauchy–Schwarz; `FINDINGS.md` §4 item 5, `SYSTEM.md` §2.1). [proved, unrefereed] For
positive diagonal D and any vectors Δ and e, |Δ·e| ≤ ‖DΔ‖·‖D⁻¹e‖. If ν ≥ ‖D⁻¹e‖, then Δ·e ∈ [−‖DΔ‖ν, ‖DΔ‖ν].

*Proof.* Δ·e = (DΔ)·(D⁻¹e), and Cauchy–Schwarz applies. ∎

Proposition 5.16 of chapter 5 takes this bound as h and models the spread of Δ·e within it as a share ρ_{g,ℓ} of h,
which the approximate mode uses in place of the certified width.

The bound holds for any code, because it uses nothing about the quantiser except that ν is at least the true weighted
error norm. A coordinate outside the quantiser's range therefore lands in ν and widens the interval without making it
unsound (`reports/surge/stage2-design.txt` §1.7). In the index each item stores, per deterministic level, a 4-bit code
for ν on a grid of ratio 2^(1/16) relative to the item scale, rounded up, and code 15 escapes to an exact float in a side
table (`SYSTEM.md` §2.1, `quant/REPORT.md` item 5). The weights come from a profile, the mean square of V_qᵀ(q − m)
for 5,000 corpus items against their four nearest centres. `FINDINGS.md` §4 item 5 gives the per-coordinate weights as
D_i = (error variance_i / Q_i)^(1/4), with Q_i the query-offset energy in coordinate i. (Editor's note, not in the
sources: these are the weights at which the Cauchy–Schwarz step in the proof of Proposition 1.13 becomes an equality
when the metric is restricted to diagonal matrices.)

**Lemma 1.9** (a quantised basis; `review/REPORT.md` §2 item 4, `SYSTEM.md` §2.1). [proved, unrefereed] Let V be the
stored basis, z̃ any vector the encoder stores for r, ε = r − Vz̃, and e_inv ≥ ‖ε‖/‖r‖. Then for every vector u,

    u·r = (Vᵀu)·z̃ + u·ε,    |u·ε| ≤ e_inv·‖u‖·‖r‖.

*Proof.* Substitute r = Vz̃ + ε and apply Cauchy–Schwarz to u·ε. ∎

The basis is stored in int8 with one scale per vector, the build encodes z = V_q⁻¹r for that int8 V_q, and e_inv is
the measured defect of the computed inverse. The query computes V_qᵀq exactly, as int8 entries times the query split
into two 13-bit halves, summed in 32-bit integers and rounded once (`SYSTEM.md` §2.2). For the non-orthogonal dual basis
the inverse is started from a Gauss–Jordan inverse and polished by Newton steps; the largest condition number of the
int8 basis was 209–227 and the largest defect 1.0·10⁻¹² (`dualproto/REPORT.md` §4).

**1.10 The certificate as built** (`reports/surge/integer-kernel.txt` §0 (A0), `rust/src/v2/q.rs`).
[sketch: gap named] Combining Lemma 1.1, Proposition 1.8 and Lemma 1.9, the index decides item x of cluster c at
deterministic level ℓ by

    |q·x − (q·m + α·m·r + η·ŝ_ℓ(x))| ≤ ψ·(η·(w_ℓ(n_x) + e_fl) + h_fix),

where ŝ_ℓ(x) is the sum of the table entries for the 4-bit units read through level ℓ, built from the weights V_qᵀΔ;
w_ℓ(n) = ‖D_ℓΔ‖·ν_ℓ(n)·(1 + 10⁻⁶) with ν_ℓ(n) the decoded value of norm code n; e_fl bounds the effect of rounding the
weights to f32 (`ewdet` in the source and the code, "weight-rounding bound"); and
h_fix = |α|·‖m‖·δ_â + e_inv·(‖q‖ + |α|·‖m‖)·r_max + 4·10⁻⁶ covers the key's rounding and the basis defect, with r_max
the largest residual norm in the cluster. This is (A0) of the source. When the index evaluates it in f32, it also adds
a bound on the tables' quantisation (`edet`) and one on the f32 evaluation (`fl`); the integer kernel of §1.8 replaces
both by the term e_ℓ of (1). The gap: the algebra follows from the three results above, but the floating-point
allowances (e_fl, `edet`, `fl` and the constant 4·10⁻⁶) are justified in code comments and checked in verify mode, and
no written source proves them. Verify mode recomputes every decision in float64; it found
0 deterministic violations in 731M, 605M and 512M interval checks on the three basis variants (`dualproto/REPORT.md`
§2), and 0 of 10.6M integer decisions that the float64 bound did not imply (`rust/REPORT_BOUNDS.md`, correctness table)
[measured].

The certificate is loose for the typical item and tight only for the worst pairs. At 96 B the actual |Δ·e| was 29
times below the certified bound for text and 21 times below it for items (`agent2/REPORT.md`, 1M) [measured]. With
nothing read, the interval spans ±6.1 score standard deviations for text and ±4.5 for items (`groups/REPORT.md` §3.5,
`margin.py`, 200k sample) [measured]. `FINDINGS.md` §5 reports that typical errors are 5–10% of the deterministic bound
while the worst pairs reach it, and that these pairs are near-duplicates of the query whose offset points along the
item's own error [measured].

**Proposition 1.11** (one stored norm fixes the weights; `reports/surge/ood/REPORT.txt` item 6). [proved, unrefereed]
Let an item store ν = ‖W⁻¹e‖ for a positive diagonal W. The errors consistent with ν form the ellipsoid
{e : ‖W⁻¹e‖ ≤ ν}, whose support function in direction Δ is ν‖WΔ‖, so Proposition 1.8 with D = W is the exact range for
that information. If a query uses other positive diagonal weights W′ with the same stored ν, its bound
‖W′Δ‖·max_i(W_i/W′_i)·ν is at least ν‖WΔ‖.

*Proof.* Substituting e = Wf, the support function is sup over ‖f‖ ≤ ν of (WΔ)·f = ν‖WΔ‖. For W′, each coordinate
satisfies |e_i/W′_i| = |e_i/W_i|·(W_i/W′_i), so ‖W′⁻¹e‖ ≤ max_i(W_i/W′_i)·ν, and the bound of Proposition 1.8 is
‖W′Δ‖·max_i(W_i/W′_i)·ν. Coordinatewise, W′_i·max_j(W_j/W′_j)·|Δ_i| ≥ W_i·|Δ_i|, so this is at least ν‖WΔ‖. ∎

The weights are therefore a build decision. A query whose offsets differ from the profile cannot re-weight the bound
after the fact, which is why the fit of the profile and the basis (§1.4) matters for OOD queries.

**Proposition 1.12** (two stored norms; `reports/surge/ood/REPORT.txt` item 6, `quant/REPORT.md` "The code as kept"
item 4). [sketch: gap named] If an item stores a ≥ ‖e‖² and ν ≥ ‖W⁻¹e‖, then for every k ≥ 0,

    |Δ·e|² ≤ (a + k ν²) · Σ_i Δ_i² / (1 + k W_i⁻²).

The gap: both sources assert this ("valid for every k", "sound by weak duality") without writing the step. The editor's
reading of the step: every e satisfying both constraints satisfies Σ_i (1 + k W_i⁻²) e_i² ≤ a + k ν², which is an
ellipsoid whose support function in direction Δ is the square root of the right side. `quant/REPORT.md` applies the same
device to the intersection of the error ellipsoid with a ball whose radius comes from the key, with a closed form at five
values of k. The gain is small [measured]: 2–10% over the better of the plain and weighted bounds for text-like
stand-in queries under the item profile and 0–2% under the hedged profile H25 (`reports/surge/ood/twonorm.py`, 200k
BioCLIP-2, stand-ins in place of real text), and about 0.3% for the exact box ∩ ellipsoid range (`quant/REPORT.md`).
Storing two norms was worse overall once the second norm's bytes were charged (`review/REPORT.md` §2 item 1,
`e4_detprofile.py`, 524k items) [measured].

Other refinements in the sources, none of them built:

- *Offset orthogonal to the code* (`FINDINGS.md` §4 item 5, `reports/surge/ood/REPORT.txt` item 5). [sketch: gap named]
  If r = γ·code + e with e orthogonal to the code, Δ may be replaced by its component orthogonal to the code. The gap is
  the hypothesis: e ⟂ code holds for a least-squares scale γ, and the built tree code has no such scale (Editor's query
  5). Its gain is put at "a few per cent" [conjecture].
- *A stored exact scalar along a fixed direction* (`reports/surge/ood/REPORT.txt` item 2). For a fixed unit g with g·r
  stored exactly, Δ·r = (Δ·g)(g·r) + Δ′·r with Δ′ = Δ − (Δ·g)g, so the certificate may use Δ′ in place of Δ. The
  identity is elementary; the predicted saving of 10–25% of text bytes after charging one byte per item, with g learnt
  from a query log, is [conjecture].
- *Box ∩ ball bounds, one-sided sign bounds and Hölder variants* are argued to be no better than weighted
  Cauchy–Schwarz for spread errors (`FINDINGS.md` §7) [conjecture].

**Deterministic and randomised widths** (`FINDINGS.md` §5). [model] With per-coordinate contributions
a_i = √(Q_i·λ_i·g(b_i)), where λ_i is the residual variance and g(b) the distortion of a b-bit uniform quantiser on a
unit Gaussian (`FINDINGS.md` §4 item 4), the best deterministic bound is about Σ_i a_i and the randomised one about
c·√(Σ_i a_i²). The randomised gain is then about √(number of coded coordinates)/c, which is nothing for short codes.
The same per-coordinate terms a_i give the statistical width ρ_{g,ℓ} = ‖a‖₂/‖a‖₁ of chapter 5's Proposition 5.16.
`FINDINGS.md` tags this "[argued, consistent with measurement]". It is the sources' reason for keeping short prefixes
deterministic.

## 1.4 The best norm: the dual basis

For any positive definite M, |Δ·r| ≤ ‖Δ‖_M·‖r‖_{M⁻¹} with ‖v‖²_M = vᵀMv. The question is which M makes this bound
tightest on average.

**Proposition 1.13** (the best norm for a representative bound; Proposition 3 of `groups/REPORT.md` §2.4).
[proved, unrefereed] Let Σ_Q and Σ_R be the second-moment matrices of Δ and r (Editor's query 2), and γ₁ ≥ … ≥ γₙ the
eigenvalues of B = Σ_Q^½ Σ_R Σ_Q^½. For every positive definite M,

    E‖Δ‖²_M · E‖r‖²_{M⁻¹} = tr(MΣ_Q)·tr(M⁻¹Σ_R) ≥ (Σ_i √γ_i)²,

with equality at M = Σ_Q^(−½) B^½ Σ_Q^(−½) = Σ_Q⁻¹ # Σ_R, the matrix geometric mean (and at its positive multiples).

*Proof.* Put P = Σ_Q^½ M Σ_Q^½. Then tr(MΣ_Q) = tr P, and P⁻¹B = Σ_Q^(−½) M⁻¹ Σ_R Σ_Q^½, so
tr(M⁻¹Σ_R) = tr(P⁻¹B). By Cauchy–Schwarz for the Frobenius inner product,
tr B^½ = tr(P^½ · P^(−½)B^½) ≤ √(tr P)·√(tr(P⁻¹B)), with equality when P is a multiple of B^½. Finally
tr B^½ = Σ_i √γ_i. ∎

Proposition 1.13 is Theorem 2 of Bhatia, Jain and Lim (Expo. Math. 2019; arXiv:1712.01504), eq. (16), minimiser
included, as a referee confirmed from the paper's source (**[cited]**; `program/floors/referee/referee-extensions.md`,
B1).

*Coordinates.* Let B = VΓVᵀ with V orthogonal and Γ = diag(γ_i), and put V_q = Σ_Q^(−½)VΓ^¼. The pair
g″ = V_qᵀΔ = Γ^¼VᵀΣ_Q^(−½)Δ and r″ = V_q⁻¹r = Γ^(−¼)VᵀΣ_Q^½r satisfies g″·r″ = Δ·r, ‖g″‖ = ‖Δ‖_M and
‖r″‖ = ‖r‖_{M⁻¹} at the optimal M, and E g″g″ᵀ = E r″r″ᵀ = Γ^½. Both sides therefore have uncorrelated coordinates with
equal second moments √γ_i. (The source states these facts; the editor checked them by substitution:
g″·r″ = ΔᵀΣ_Q^(−½)VΓ^¼Γ^(−¼)VᵀΣ_Q^½r = Δ·r, and E r″r″ᵀ = Γ^(−¼)VᵀBVΓ^(−¼) = Γ^½.) Any certificate written for an
orthonormal basis, such as prefix codes, block norms or sorted ranges, carries over unchanged, because it uses only the
identity g″·r″ = Δ·r and Cauchy–Schwarz. The index's query side is V_qᵀq with the centre removed in the same
coordinates, and its item side is z = r″ (`SYSTEM.md` §2.1, `dualproto/REPORT.md` §4).

**Corollary 1.14** (the zero-bit ball against the floor; `groups/REPORT.md` §2.4). [proved, unrefereed] Let r be
Gaussian and Δ Gaussian and independent of r. With no bits read, every sound certificate has
E h ≥ κ_k·(γ₁⋯γ_k)^(1/2k) for each k (Corollary 1 of the consolidated proofs, which is Corollary 2.3 of chapter 2 at
B = 0, [proved, refereed]), which is at least k times the geometric mean of √γ₁, …, √γ_k because κ_k ≥ k for
k ≤ 200,000 ([computed], `proofcheck/constants.py`; the remarks on Corollary 2.3).
The M-ball certificate has mean half-width E‖Δ‖_M·E‖r‖_{M⁻¹} ≤ Σ_i √γ_i by Proposition 1.13 and Jensen's inequality
(the source states the rms form, "an rms slack of Σ √γᵢ"). So at
k = n the ball is within the ratio of the arithmetic to the geometric mean of the √γ_i of the best zero-bit certificate.

A design whose weights are diagonal in the eigenbasis of Σ_R, as the earlier PCA design was, reaches these coordinates
only when Σ_Q and Σ_R commute (`groups/REPORT.md` §2.4).

**The fit as built** (`SYSTEM.md` §2.1, `dualproto/REPORT.md` §1). Clusters are grouped by k-means on their centre
directions, and each group gets its own basis. Σ_Q is fitted once for all groups from 3,000 corpus items used as
pseudo-queries, with offsets Δ = q − (q·m̂)m̂ to 40 clusters drawn by size and a ridge of 0.1·tr(Σ_Q)/n; an optional
sample of expected queries can be mixed in. The fit affects cost only, because every bound in this chapter holds for any
basis.

Measured effects [measured]:

- *Exact coordinates* (`groups/REPORT.md` §3.3, numpy, 1M, K = 1,000, 369 text and 100 item queries, a per-member read
  in dyadic blocks). Coordinates read per item at answers of 0.01 / 0.1 / 1 / 5% fell, for text, from
  44.9 / 64.9 / 116.7 / 202.0 in the PCA basis to 37.7 / 54.9 / 99.9 / 175.9 with the geometric mean fitted on data
  only and 25.3 / 38.8 / 75.8 / 142.1 with the mixed fit; for items they fell from 4.2 / 10.5 / 37.0 / 112.3 to
  2.8 / 6.8 / 23.0 / 68.8 (data only) and 3.0 / 7.3 / 24.9 / 74.6 (mixed). The best diagonal weights in the PCA basis,
  fitted to text, reached only 43.2 / 62.7 / 113.6 / 197.9 for text.
- *Native quantised code* (`dualproto/REPORT.md` §§2–3, Rust version 2 prototype, 1M, 200 item and 369 text queries,
  `bytesq`). Against the PCA basis, the data-only fit cut bytes per item by 9 / 17 / 24 / 30% for items and
  10 / 9 / 7 / 6% for text; the fit mixing items with 2,206 held-out prompts cut them by 6 / 13 / 19 / 25% and
  34 / 32 / 28 / 25%. Every variant gave 0 wrong answers, 0 deterministic and 0 randomised violations, and exact k-NN on
  569 of 569 queries. On the index of `SYSTEM.md`, supplying the text training prompts cut OOD bytes by a further
  20–22% at answers of 1–75% (`rust/results/combined/s1/bytes_dual_vs_dualmix.log`).
- *The dead-list entry overturned* (`dualproto/REPORT.md` §6, `tools/clipcheck.py`, 12 groups, 220,263 items, 569
  queries). `FINDINGS.md` §7 had recorded simultaneous diagonalisation as hurting beyond 16 B. Re-run in the same
  quantiser, the loss came from fitting Σ_Q per group to nearest-cluster offsets; with the global fit above, the dual
  pair narrowed the mean certified half-width against PCA by 19% at 4 B and 32% at 96 B for items, and by 17% at 96 B for
  text. Clip rates were 2.0–2.6% in every basis, so clipping was not the cause. Why the per-group fit fails (it gives no
  bits to directions where real offsets still carry energy) is [conjecture].
- *Zero-bit slack.* With nothing read, the plain M-ball in the dual basis barely beats PCA (items 0.149 against 0.152,
  text 0.228 against 0.232; the mixed fit gains 9% on text), so the byte gains come from compacting energy across levels
  (`dualproto/REPORT.md` §6, `tools/diag_sq.py`). The block-by-block bound in exact coordinates of `groups/REPORT.md`
  §3.3 (`tables/bases.txt`) falls further: for text from 0.250 in the PCA basis to 0.213 with the data-only fit and
  0.190 with the mixed fit, and for items from 0.248 to 0.181 and 0.188. `dualproto/REPORT.md` quotes only the
  0.250 → 0.190 pair, which is the mixed fit. The two measures have not been reconciled (Open, Editor's query 12).
- *Grouping in the same metric.* Assigning rows to the centre nearest in the M⁻¹ norm instead of by cosine changed
  coordinates read by at most 0.7 per item (under 1% for text; 0.1 per item, up to 4%, for items) and moved only the
  group-ball bound, which the sorted range already beats (`groups/REPORT.md` §3.4, `prep_m.py`).

## 1.5 What a grouping and a group summary can buy

The cluster-level steps of §1.2 decide whole groups of items from shared data. The results of this section bound how
much any such sharing can buy, and they explain why the design decides items one by one.

**Proposition 1.15** (a grouping is worth at most log₂K bits per item; Proposition 1 of `groups/REPORT.md` §2.2).
[proved, unrefereed] Let x have a density, let V₁, …, V_K be a partition of space with fixed centres m_j, and let
r = x − m_j for x ∈ V_j, with j the cell label. Hold the law of Δ fixed, Gaussian and independent of x, and write
x′ = Σ_Q^½x. For any encoder of r with message **c** and any sound certificate, for every k-dimensional subspace S,

    E h ≥ E‖g_k‖ · ω_k^(−1/k) · exp((H(P_S x′) − H(j) − H(**c** | j))/k),    with H(j) ≤ ln K,

where H(P_S x′) is a differential entropy in nats, as in chapter 2.

*Proof.* Apply Theorem 1 of the consolidated proofs (Theorem 2.2 of chapter 2, [proved, refereed]) within each cell.
Given j, r′ is a translate of x′, so H(P_S r′ | j) = H(P_S x′ | j) = H(P_S x′) − I(P_S x′; j) ≥ H(P_S x′) − H(j). Average
over j and use the convexity of exp. ∎

Proposition 2.8 of chapter 2 is the same result from the same source, with the reading of "hold the law of Δ fixed"
set out in its Editor's query 14.

The label therefore enters the floor exactly as the bit budget B does: any K-way grouping, by any rule, lowers the
floor by at most the factor K^(−1/k). `groups/REPORT.md` reads the measured radius law, residual ∝ K^(−0.053), as this
statement with k ≈ 19. A k-means partition of N ≫ K points fits the hypothesis of fixed centres up to the dependence of
a centre on one member, which is of order one over the group size.

A summary of B_s bits shared by N_g items with independent residuals is worth at most B_s/N_g bits per item (Corollary 2
of the consolidated proofs, which is Corollary 2.4 of chapter 2, [proved, refereed]). The referee added that the independence is needed, because
near-duplicates let a shared message inform every item (`proofcheck/REFEREE.md`, "Fixes to the floor").

**Proposition 1.16** (summaries the query chooses; Proposition 2 of `groups/REPORT.md` §2.3). [proved, unrefereed]
Let the query choose a = a(Δ) among A read sets, the choice not depending on the data. Under read set a, let w_a(g) be
half the width, in direction g, of the set of residuals consistent with everything read, in the coordinates
r′ = Σ_Q^½r, with Δ = Σ_Q^½g and g standard Gaussian. Let every residual satisfy ‖r‖ ≤ ν. Then

    E w_{a(g)}(g) ≥ min_a E w_a(g) − ν·√(2·λ_max(Σ_Q)·ln A),

with expectations over the data and g.

*Proof.* Φ_a(g) = E_data w_a(g) is an average of functions that are Lipschitz with constant L = ν‖Σ_Q^½‖, because each
is half the sum of two support functions of a set inside the ball of radius L. Gaussian concentration gives
E exp(λ(EΦ_a − Φ_a)) ≤ exp(λ²L²/2), hence E max_a (EΦ_a − Φ_a(g)) ≤ L√(2 ln A). Then
E Φ_{a(g)}(g) ≥ E min_a Φ_a(g) ≥ min_a EΦ_a − L√(2 ln A). ∎

Each min_a E w_a is bounded below by the floor at the rate read under a. When offsets spread over D directions, the
correction is the Cauchy–Schwarz slack times √(2 ln A / D), which is the cosine of the angle to the nearest of A random
anchors, so an anchor table helps only when ln A is comparable to D. On this data the statement is weak: λ_max(Σ_Q) is
18% of the trace for text (`offsets.py`), so the correction exceeds the slack at A = 1,000. The proposition does not
cover choices that depend on bits already read. Proposition 2.9 of chapter 2 is the same result from the same source.

**Remark 1.17** (why cluster extremes are the wrong target; `groups/REPORT.md` §2.5). [model] A group is cut only when
its largest member clears the threshold by the slack, and for Gaussian scores the largest of about 1,000 members sits
more than three within-group standard deviations above the group mean. A member is decided when its own score clears.
Certifying a group's extremes to ±δ therefore asks for more precision than deciding its members one by one, and an
oracle's count of undecided groups is neither a floor nor a target for a scheme that reads per member.

Measured effects [measured] (`groups/REPORT.md` §§3.1–3.2, numpy, 1M, K = 1,000, PCA bases, % of items not decided at
answers of 0.01 / 0.1 / 1 / 5%):

- An idealised group summary that knows the exact extremes in a group's first 64 local coordinates leaves
  89.0 / 93.9 / 98.9 / 99.9% for text and 32.9 / 49.9 / 78.4 / 98.6% for items, while the same 64 coordinates held per
  member leave 15.9 / 24.1 / 45.1 / 72.4% and 0.51 / 2.12 / 12.1 / 44.5%. Block norms alone as a summary leave
  99.9–100% for text.
- Finer groups do not pay. Going from 1,000 to 15,880 groups (4 more bits of label) saved a text query 4.5 of 45
  coordinates per item at the 0.01% answer and cost 11.4 coordinates per item in centres; across 1,000 to 58,750 groups
  the rms residual fell from 0.3765 to 0.3034, as K^(−0.053).
- A text query's scores had standard deviation 0.040 (0.020 within groups), against a plain Cauchy–Schwarz slack of
  0.296 and a block slack of 0.250 with a threshold 0.074–0.200 above the median, and 75% of a text query's score
  variance lay between groups (84% for items).

## 1.6 The codes as built

**The deterministic levels** (`SYSTEM.md` §2.1, `quant/REPORT.md`). The first six levels end at 4, 8, 12, 16, 24 and
32 bytes per item. Each coordinate of z/η is coded additively, first as a tree field of depth 1, 2 or 4 on Gaussian
centroids and then as 1-bit residual stages. A greedy nested allocation gives each next bit to the coordinate with the
largest profile-weighted gain per bit, minimising Σ_i √(λ_i·Q_i·g_tree(b_i)), so every level refines the one before. A
level's new bits are packed into self-contained 4-bit units and stored in blocks of 32 items with transposed nibbles, so
a query scores a level by looking each unit up in a 16-entry table (the fast-scan layout). Each level also stores the
4-bit norm code of §1.3. The item scale is η = ‖r‖/√(Σλ) with ‖r‖ from Lemma 1.2 (`quant/REPORT.md` item 3); under the
dual basis the denominator is the group's mean ‖r‖², because Σλ equals it only for an orthogonal basis
(`dualproto/REPORT.md` §4 change 2).

This fixed-rate tree code replaced an escape code (a sign, a unary index on a fixed grid, then halvings), which read
13–30% fewer bytes than the first scalar code (`DESIGN.md` §5) but decoded at 15–20 ns per coded byte
(`review/REPORT.md` §1.1).
The fixed-rate code costs 1.5–2.0% more bytes on BioCLIP-2 and decodes 10–17 times faster [measured, bytes;
[model], times estimated from measured decode rates] (`quant/REPORT.md`, "Bytes against time").

What the encoder does on real residuals [measured] (`dualproto/REPORT.md` §6, `VEXACT_CLIPSTATS`, all 1M items): every
coded deterministic coordinate is a sign (±0.798, the mean of a half-Gaussian) followed by up to four 1-bit stages, and
tree depths 2 and 4 are never chosen. Under the PCA basis, 31.7 / 28.2 / 25.5 / 23.3% of coded coordinates lie beyond
the code's reach at 4 / 8 / 16 / 32 B, and that excess is 14.7 / 19.1 / 25.8 / 35.5% of the squared weighted error. The
dual variants are within 2.5 points of these figures. Coded values beyond 3 and 4 standard deviations occur at 0.301% and
0.0118% against 0.27% and 0.0063% for a Gaussian, and the kurtosis is 3.08. The excess is carried by ν, so it costs width
and not soundness.

**The randomised levels** (`SYSTEM.md` §2.1, `rust/REPORT_BOUNDS.md` "The quantiser"). Four further levels end at 56, 96,
160 and 256 bytes. They code the error that the 32-byte prefix leaves, weighted by the profile, ordered by importance and
split into blocks at 1/8, 7/24 and 5/8 of n (96, 128, 256 and 288 positions at n = 768). Each block has its own Haar
rotation drawn from ω and shared by every group. Each rotated coordinate is coded as a nested uniform binary expansion
with a scale chosen per item from six values, and a block's rate only rises from one level to the next. The error law
that this code supports (Lemma 8 of the consolidated proofs, Lemma 3.1 of chapter 3) and the constants are in chapter 3.

**The stored scalars** (`SYSTEM.md` §2.1, `rust/REPORT_BOUNDS.md` Part 1). The first randomised level stores ‖y_j‖ per
block in 8 bits, and each rise of a block stores the estimator's scale κ_j in 4 or 8 bits and tan φ_j in 4 bits. The
tables are fitted to round up with the least excess, and each 256-entry table is the product of two 16-entry tables
indexed by the two nibbles of the code, so decoding is two byte permutes and a multiply. The encoder rounds up against
the stored tables, and the build asserts for every item, level and block that the decoded ‖y_j‖ and tan φ_j are at
least the true values; values off the grids escape to exact floats, 41,393 of them at 1M. That a rounded-up scale only
widens the bound is result R1 of `reports/bounds3/REPORT.txt` (Theorem 3.18 of chapter 3, with Theorem 3.19 for its
use). One encoder bug shows why every rounding
direction is checked: a relative block-norm code that rounded values below its grid up to the grid's minimum broke the
reconstruction-scale bound in 542 cases, and a fallback on both sides removed them (`quant/REPORT.md`) [measured].

The randomised levels keep a deterministic part. A block not yet coded contributes ‖y_j‖·‖Δ_j‖ by Cauchy–Schwarz, and
the 32-byte prefix's error, table and κ rounding and floating-point evaluation add further terms (`SYSTEM.md` §2.2). On
a dump of 9.7M intervals (50k index, 10 item and 10 text queries, `study/bounds_study.py`), the mean half-width at 56 B
was 0.080, of which 0.062 was the Cauchy–Schwarz term on the three uncoded blocks; from 96 B on the random term was
the whole half-width (`rust/REPORT_BOUNDS.md` Part 2) [measured].

## 1.7 How far the codes are from optimal

**The randomised quantiser.** [computed] A search over the ratio of successive steps at every depth, on Gaussian blocks
of 128 and for the six rise schedules in use, found halving optimal in every case. At one rise of 2 bits the code's
tan²φ is 0.133, the Lloyd–Max value for a Gaussian. Nesting costs something: at 2 bits the code reaches 0.179 against
0.137 when the same block must also serve 3 and 6 bits, and unequal steps do not recover it (`rust/REPORT_BOUNDS.md`,
`study/quant_study.py`).

**The deterministic levels against the floors.** [computed] With real residuals, Gaussian offsets and every stored bit
counted, `quant/REPORT.md` (`floor2.py`) places the deterministic levels on BioCLIP-2 at 1.34–1.37 times the conjectured
floor (Conjecture 2.14) before its changes and 1.17–1.21 times after, which is 1.60–1.86 and 1.42–1.66 times the proved
floor (Corollary 2.3); chapter 2 §2.9 gives the same figures with the item-offset profile's rows beside them, and its
Editor's queries 12 and 17 say what `floor2.py` measures and which "before" it uses; on
BioCLIP-2.5 (n = 1,024) the figures are 1.30–1.31 and 1.17–1.18 against the conjectured floor and 1.58–1.78 and
1.44–1.65 against the proved one. At the early levels most of the gap was stored scalars and not the shape of the
quantiser. What remains is the per-coordinate scalar quantiser's loss against the rate–distortion bound (1.13–1.46
times) and, at the deep levels, the fixed-rate tree code against entropy coding (about 1.1–1.25 times). An E8 lattice
could recover about 7% of width at depth 3 and beyond [conjecture]; it is not built.

The referee (`proofcheck/REFEREE.md`, "Headline claim") checked an earlier version of this comparison and narrowed it.
Agent 1's escape code sits 1.36–1.42 times above the proved bound at 96–576 B, but only on synthetic Gaussian residuals
with diagonal spectra and independent Gaussian offsets, and the earlier scalar code sat 1.5–1.7 times above it at
96–192 B and 2.7–9 times beyond [computed]. Ratios that divided a certificate using three stored scalars by a floor that
did not count them were corrected from 1.40 / 1.36 / 1.36 / 1.41 / 1.55 / 2.02 to 1.61 / 1.52 / 1.49 / 1.51 / 1.65 / 2.11.
The referee also lists why no floor is proved for the real data: the residuals are non-Gaussian, the offsets were taken
diagonal in the residual basis, queries are neither Gaussian nor independent of items, the shared local bases (about 600
bits per item at 1M) are not counted, and the stored m·e scalar is information about r. See Editor's query 4.

**Structure beyond second order** (`entropy/REPORT.md`). [measured] Held-out cross-entropy under explicit models gives
lower bounds on the negentropy J that a code could exploit. On BioCLIP-2 (n = 768, 200k) J is 80 bits per item, and
modelling it would lower the floor by a factor of 0.93, or 0.89 with an ICA rotation; a usable code at matched
distortion saves 1.9–4.0% of bits, a finer Gaussian mixture about 9%, and prediction from neighbours 3.5%. On
BioCLIP-2.5 the floor factor is 0.83 (0.79) and the usable saving 4.9–10.4%. On GloVe, NYTimes, ada-002 and
text-embedding-3-large every marginal or scale model saves under 1%. The heavy tails of the image embeddings are almost
entirely per-cluster and per-item scale variation. Bytes read would fall by at most 3–8% on BioCLIP and under 1% on the
text sets. Closing the escape code's 1.36–1.42× quantiser gap (the referee's figure for agent 1's code, not the
fixed-rate code as built) is estimated to be worth 1.3–1.6 times in bytes by the same cost law [conjecture].

## 1.8 Soundness of rounding in the integer kernel

The index decides deterministic levels in integers: it decodes 32 items at a time with byte shuffles and 16-bit
additions and compares integer sums with per-item thresholds, so no floating-point bound is evaluated per item
(`SYSTEM.md` §2.2). This section shows that every integer decision is implied by the real-valued certificate of §1.3.
The randomised levels' integer form (Lemma R and Theorem R of `reports/surge/integer-kernel.txt` §2) belongs to
chapter 3; the referee found both correct with repairs. Chapter 3 does not yet state them, and
`06-open-problems.md` lists the omission.

*Tables.* A cluster's deterministic levels share one step σ > 0. For a 4-bit unit u with ideal table L_u(v) (real
arithmetic on the f32 weights), the stored entry is θ_u(v) = round((L_u(v) − b_u)/σ), an integer in [0, 2^(h+8) − 1]
split into a high part of h bits and a low byte, with b_u ≤ min_v L_u(v). Then |σθ_u(v) + b_u − L_u(v)| ≤ δ_u = σ/2 + f_u,
where f_u bounds the float error of building the table. Let B_ℓ = Σ b_u and e_ℓ = Σ δ_u over the units through level ℓ.
The integer part of B_ℓ/σ is added to the accumulators as a per-level constant, and B_ℓ denotes the remainder from here
on, so |B_ℓ| < σ.

**Lemma 1.18** (the accumulators; `reports/surge/integer-kernel.txt` §1.2, equation (1)). [proved, refereed] Let Hs be
the sum of the high parts and Ls the sum of the low bytes looked up for an item through level ℓ, and Θ = 256·Hs + Ls.
Then Θ is exactly the sum of the θ_u looked up, and

    |σΘ + B_ℓ − ŝ_ℓ| ≤ e_ℓ.                                                    (1)

*Proof.* 256·hi + lo reconstructs each θ_u exactly, so Θ = Σθ_u. Summing the per-unit bounds over the units read gives
(1). ∎

*Thresholds.* Dividing the certificate of §1.3 by η > 0, an item is in when ŝ_ℓ − ψ(w_ℓ(n) + e_fl) ≥ K_in and out when
ŝ_ℓ + ψ(w_ℓ(n) + e_fl) < K_out, with

    K_in = (t + ψ·h_fix − q·m − α·m·r)/η,    K_out = (t − ψ·h_fix − q·m − α·m·r)/η.

Per item, once, from its key: T_in = ⌈K_in/σ⌉ and T_out = ⌊K_out/σ⌋. Per level and norm code n:
R_ℓ(n) = ⌈(ψ(w_ℓ(n) + e_fl) + e_ℓ + |B_ℓ|)/σ⌉. The rule is IN iff Θ − R_ℓ(n) ≥ T_in, and OUT iff Θ + R_ℓ(n) < T_out. The
escape code 15, and any item or code whose values fall outside the ranges the kernel can represent, set a mask that
forces "undecided".

**Theorem 1.19** (integer decisions are implied by the real bound; Theorem D of `reports/surge/integer-kernel.txt`
§1.3, renamed here to avoid the Theorem D of the consolidated proofs, which is Theorem 2.11 of chapter 2).
[proved, refereed] IN implies that the lower
bound of the certificate is at least t, and OUT implies that its upper bound is below t.

*Proof.* IN: σΘ ≥ σT_in + σR_ℓ(n) ≥ K_in + ψ(w_ℓ(n) + e_fl) + e_ℓ + |B_ℓ|. By (1), ŝ_ℓ ≥ σΘ + B_ℓ − e_ℓ ≥
K_in + ψ(w_ℓ(n) + e_fl). Multiplying by η > 0 and rearranging gives
q·m + α·m·r + η·ŝ_ℓ − ψ(η(w_ℓ(n) + e_fl) + h_fix) ≥ t. OUT: σΘ + σR_ℓ(n) < σT_out ≤ K_out, and by (1)
ŝ_ℓ ≤ σΘ + B_ℓ + e_ℓ, so ŝ_ℓ + ψ(w_ℓ(n) + e_fl) < K_out; rearranging in the same way gives the upper bound below t. ∎

Every rounding widens: R_ℓ is rounded up, T_in up and T_out down, and e_ℓ and |B_ℓ| are added on both sides. The
referee confirmed the implication (`reports/surge/referee.txt` §6).

**Lemma 1.20** (the two-word compare; `reports/surge/integer-kernel.txt` §1.4). [proved, refereed] For integers d and
g, 256d + g ≥ 0 if and only if d + ⌊g/256⌋ ≥ 0.

*Proof.* Write g = 256f + r with 0 ≤ r < 256. If d + f ≤ −1 then 256(d + f) + r ≤ −1. If d + f ≥ 0 the sum is at least
0. ∎

The kernel evaluates d = Hs − TH ∓ RH and g = Ls − TL ∓ RL on the high and low words of the thresholds and margins and
tests d + (g >> 8) with an arithmetic shift, so the lemma makes the 16-bit compare equal to the full-width one.

**1.21 Saturation.** The source's range paragraph chose h = 7 and R_cap = 16,000 so that 2U(2^h − 1) + R_cap + 64 ≤ 32,767
with U = 64 units, and concluded that the saturating additions never saturate. [refuted] The referee showed that the
budget counts R_cap once: on unmasked lanes Hs − TH − RH reaches −48,256, which wraps to +17,280 under wrapping arithmetic
and gives a false IN (`reports/surge/referee.txt` §6). Two repairs work: R_cap ≤ 8,222, or every operation saturating in
the order (Hs − TH), then ∓RH, then +(g >> 8), which gave 0 sign mismatches in 4,000,000 random cases.

The built kernel takes the second repair. [proved, unrefereed] (comment at `rust/src/v2/kern.rs` lines 386–404). T_in
and T_out are clamped to [T_MIN, T_MAX], so their high words lie in [−24,000, 24,000] and Hs − TH is exact in 16 bits.
Clamping only withholds decisions: lowering T_in to T_MAX cannot make an IN because Θ − R_in ≤ Θ_MAX − R_IN_MIN < T_MAX,
and raising T_out to T_MIN cannot make an OUT because Θ + R_out ≥ R_OUT_MIN > T_MIN. A margin outside its range is
replaced by a sentinel that never decides, as is the escape code. The rest of each compare saturates in the referee's
order, with g in [−510, 16,575], and a saturated word has magnitude at least 32,768, which g >> 8 (in [−2, 64]) cannot
carry across zero.

The referee named two further gaps. Thresholds computed in f32 "with a relative 2⁻²⁰" are not a proof, because K_in is
a difference of nearly equal terms divided by η, so its error is absolute: (|t| + |ψ·h_fix| + |q·m| + |α·m·r|)·2⁻²²/(ησ)
steps. The referee's repair is f64 or that absolute slack, and it adds that rounding (L_u(v) − b_u)/σ in f32 adds up to
2⁻⁸ of a step to δ_u. The built threshold routine (`int_thresholds` in `rust/src/v2/kern.rs`) works in f64 and adds both
an absolute slack (|t| + |h_fix| + |q·m| + |α·m·r|)·2⁻⁴⁶/(ησ) and a relative 2⁻⁴⁶ before rounding outward (Editor's
query 7). The source also leaves the range of the per-level centring constant unbounded, which the clamps above make
harmless in the built kernel.

**Proposition 1.22** (centre tables by linearity; `reports/surge/integer-kernel.txt` §3). [proved, unrefereed] The
weights of cluster c are w_c = w_q − α_c·μ_c, with w_q per (query, group) and μ_c the centre in the same coordinates.
Tables are linear in the weights, so L^c_u(v) = L^q_u(v) − α_c·L^m_u(v), with L^q per (query, group, level) and L^m per
cluster and independent of the query. The real table is therefore identical and so are the estimate and the half-width;
only the rounding term changes, from 0.5 to at most 0.5 + 0.5 + 2 fine steps per unit, so e_ℓ ≤ 3Uσ. This is how the
index builds each cluster's tables from the group's float tables and the stored centre tables while keeping α
(`SYSTEM.md` §2.2).

**Full vectors** (`SYSTEM.md` §§1, 2.2). [sketch: gap named] Items still undecided after 256 bytes are scored from
their f16 vector in float32, with an exact integer product only when the float score lies within its rounding bound of
t. "Exact" means exact in f16 arithmetic on the stored vectors and the query rounded to f16, so answers can differ from
a float64 brute force only by items whose score lies within f16 rounding of t. The gap: the float32 rounding bound
(`full_err_qn` in `rust/src/v2/q.rs`) is stated in code, and no written source proves it.

Measured end to end [measured]: on a 50k index (200 item and 200 text queries, seven answer sizes, filtered visible
sets, k-NN, float64 oracle, `check.sh`), every step of `rust/REPORT_BOUNDS.md` gave 0 wrong answers, 0 deterministic and
0 randomised violations in about 44M interval checks, 0 of 10.6M integer decisions not implied, and exact k-NN on 400 of
400 queries. At 1M the index's answers were byte-identical to the plain f16 scan at every visible set (`SYSTEM.md` §3).

## 1.9 Prior work

The attributions are those of `lit/REPORT.md`, whose bibliography records how each source was checked. A source the
survey read in full is tagged [cited]; one known from an abstract, a summary or a citation only is tagged
[cited, unchecked].

| Component | Prior work | Tag |
|---|---|---|
| Progressive refinement of compressed approximations with guaranteed bounds, then full reads | VA-file [1], IQ-tree [3], BOND [4], LEMP [6], ANSMET [8], PDX [9], Panorama [10] | [cited] |
| The same | VA+-file [2], FEXIPRO [7] | [cited, unchecked] |
| A stored remainder norm with Cauchy–Schwarz | LEMP's INCR [6], Panorama [10] | [cited] |
| The same | L2AP [19] | [cited, unchecked] |
| Stored scalars counted as cost | Extended RaBitQ [13] | [cited] |
| Fitting a code to a query distribution unlike the data | QUIP [31], LeanVec [32], RoarGraph [33]; ScaNN's anisotropic loss [30] uses no query sample | [cited] |
| Clusters, residuals against the centres, 4-bit codes scored from 16-entry tables, rescoring; with no bound per item | ScaNN [30], SOAR [53], SPANN [54], SPFresh [55] | [cited, unchecked] |
| The sorted range on a key relative to a reference point | iDistance [34] | [cited] |
| Covering-radius pruning of clusters | M-tree [35], the list of clusters [36]; cosine form, Schubert [38] | [cited] |
| The same, origin | Fukunaga and Narendra [37] | [cited, unchecked] |
| Per-cluster minimum and maximum against split points | GNAT's range tables [39]; pivots, LAESA [40]; inverted-file use, Tribase [41] | [cited] |
| The same, recent inverted-file variants | TRIM [42], CLIP [43] | [cited, unchecked] |

The numbers refer to the bibliography of `lit/REPORT.md`. The survey judged the following parts new or partly new: the
weighting of a stored norm by query offsets applied to the quantisation error of every prefix of a nested code, with
outward-rounded log-grid scalars charged in the rate; a mixture of item and text queries applied to the allocation and
certificate weights of an exact method; and, as "possibly new and small", the concave-above, convex-below lemma behind
Lemma 1.5. It found no single prior scheme carrying deterministic certificates in its first levels and randomised ones
after. The Lloyd–Max quantiser of §1.7 and the matrix geometric mean of §1.4 are used in the sources without a
citation.

## Open

- Lemma 1.5 and Propositions 1.13, 1.15 and 1.16 are short proofs that no referee has checked (`groups/REPORT.md` §5
  says the same of Propositions 1–3). Chapter 4's Lemma 4.10 and chapter 2's Propositions 2.8 and 2.9 are the same
  results and share this status.
- The floating-point allowances of the certificate as built (§1.3, 1.10), the float32 rounding bound of the full-vector
  step and the two-norm bound of Proposition 1.12 need written proofs.
- The ν-sort of Proposition 1.6 has not been run natively. It would remove the support table, whose size and anchor
  ratio at 10⁹ rows are unresolved (`review/REPORT.md` §1.3), and the atomic widening of the table on insert.
- Two measures of the dual basis's zero-bit slack disagree (for text with the data-only fit, 0.232 → 0.228 for the
  plain ball in `dualproto/REPORT.md` §6 against 0.250 → 0.213 for the block bound in `groups/REPORT.md` §3.3) and have
  not been reconciled.
- One Σ_Q serves every group and one ridge value was tried (`groups/REPORT.md` §5, `dualproto/REPORT.md` caveats). The
  text evidence for the mixed fit is optimistic, because test and training prompts came from the same templates
  (`review/REPORT.md` §1.6).
- No floor is proved for the real data (§1.7). The deterministic code's remaining gap to the rate–distortion bound
  (1.13–1.46 per coordinate, 1.1–1.25 against entropy coding at deep levels) is untouched; lattice and trellis codes
  are not built, and entropy coding in randomised blocks (about 3% on text) is not table-decodable.
- Proposition 1.16 does not cover summaries chosen from bits already read, and it is numerically weak on this data.
- The sorted-range lemma does not transfer to non-unit vectors or to L2 distance, where the bound on the part of x
  orthogonal to m̂ changes (`reports/surge/stage2-design.txt` §1.8(b)); the fixed-point definition of similarity on
  stored integers asked for in `review/REPORT.md` §1.10 is covered for unit f16 rows only through x₂.
- Prior art for the matrix-geometric-mean metric has been searched by the program's librarian. Proposition 1.13 is
  Theorem 2 of Bhatia, Jain and Lim (§1.4, [cited]; `program/floors/referee/referee-extensions.md`, B1), and no
  search, quantisation or sketching paper was found that uses the metric (`program/library/theory.md`, item 2;
  [cited, unchecked]; chapter 6 §6.4).

## Editor's queries

1. `SYSTEM.md` §2.1 says the dual pair is what `groups/REPORT.md` §2.4 "derives as minimising the expected width of the
   bound". Proposition 1.13 minimises E‖Δ‖²_M·E‖r‖²_{M⁻¹}, which bounds the squared mean width from above when Δ and r
   are independent (Corollary 1.14). The two objectives differ, and the wording in `SYSTEM.md` should say which is meant.
2. Proposition 1.13 uses E‖Δ‖²_M = tr(MΣ_Q), which holds for the second-moment matrix of Δ or for a covariance of a
   mean-zero Δ. The sources call Σ_Q a covariance, and the built fit uses offsets Δ = q − (q·m̂)m̂ with no centring step
   mentioned (`dualproto/REPORT.md` §1). This chapter states the result for second-moment matrices. The fit scripts
   compute exactly that: `groups/scripts/sigmaq.py` and `dualproto/tools/dual.py` accumulate DᵀD over the offsets and
   divide by their count, with no centring. The reports' word "covariance" should read "second-moment matrix".
3. `SYSTEM.md` §2.2 gives the candidates at a 1% answer as 22% of rows for in-distribution queries and 95% for OOD
   queries. `dualproto/REPORT.md` §3 and `groups/REPORT.md` §3.1 give 47–48% for item queries and 94% for text at the same
   answer size, and `combined/REPORT.md` gives 46.9% and 90.3%. SYSTEM's 22% matches the item figure at the 0.1% answer
   in the other three sources (22.3, 22 and 23.2%), so it may be mislabelled by answer size. The builds, query sets or
   definitions (candidates of opened clusters against items touched) may also differ; the figures should be reconciled
   or their definitions stated.
4. `DESIGN.md` §6 item 3 says the per-item code "sits 1.36–1.42× above the proved floor on Gaussian data", which is the
   referee's figure for agent 1's escape code on synthetic residuals. `quant/REPORT.md` gives 1.60–1.86 times the proved
   floor for the code before its changes on real residuals and 1.42–1.66 after. These are different codes on different
   data, so the two ranges should not be compared as one quantity.
5. The refinement "e ⟂ code, so Δ can be replaced by its component orthogonal to the code" (`FINDINGS.md` §4 item 5) is
   listed among free refinements in `reports/surge/ood/REPORT.txt` item 5. It holds for a code with a least-squares
   scale, as in the construction of `FINDINGS.md` §4, and it does not hold as stated for the built tree code, whose scale
   comes from the key.
6. `SYSTEM.md` §2.2 describes the decided rows as "certain matches above it and certain non-matches below". Lemma 1.5
   proves only that the undecided rows are contiguous, and the code (`sorted_range2`) returns the out rows as the
   complement of one range and the in rows as the complement of a nested range within it. The description in `SYSTEM.md`
   reads as the usual arrangement for q·m̂ > 0, and the editor has not checked whether in rows can lie on both sides in
   other cases.
7. The referee's absolute slack for the f32 thresholds includes |α·m·r|. The editor read `int_thresholds`
   (`rust/src/v2/kern.rs`): it adds |α·m·r| per item to the scale |t| + |h_fix| + |q·m| passed from `q.rs`, and it works
   in f64 with 2⁻⁴⁶ in place of 2⁻²², so the referee's term is covered. Two small points remain unwritten. The scale
   carries |h_fix| where the referee writes |ψ·h_fix|, which matters only for a dial above 1. The f64 slack is argued
   in no written source.
8. Proposition 1.12 and the coordinate facts after Proposition 1.13 contain steps written out by the editor: the
   enclosing-ellipsoid argument for the former and the substitution checks for the latter. The sources assert both
   without these steps, and the tags were kept at the sources' level.
9. `reports/surge/integer-kernel.txt` names its main result "Theorem D", which collides with Theorem D (instance
   optimality) of the consolidated proofs; it is Theorem 1.19 here.
10. `DESIGN.md` §2 describes a 1-byte error norm per level and a level-0 norm byte; `SYSTEM.md` and `quant/REPORT.md`
    describe a 4-bit norm code per level and no level-0 norm. This chapter follows `SYSTEM.md`, which follows the code.
11. Proposition 1.3(a) is refereed for α = q·m/‖m‖² in the randomised setting of `reports/surge/level0/REPORT.txt`. The
    index computes a different α, a least-squares coefficient in the weighted coordinates, which Lemma 1.1 makes sound at
    the deterministic levels. Whether one α per (query, cluster) is used consistently at every randomised level, as
    chapter 3's union bound needs, is outside this chapter's sources and should be checked there (Lemma 3.1 and
    Theorem 3.11 (i) state the condition that α be fixed independently of ω).
12. `dualproto/REPORT.md` §6 says `groups/REPORT.md` reports a cut "0.250 to 0.190 for text". That pair compares the
    PCA basis with the mixed fit (`groups/tables/bases.txt`), while dualproto's own 0.232 → 0.228 is for the data-only
    fit; the like-for-like block figure is 0.250 → 0.213. The two measures still differ (plain ball against block-by-block
    bound, quantised against exact coordinates), but part of the apparent disagreement was a mismatched comparison.
13. Proposition 1.6 is stated here in the stored-basis quantities that `NuCut` uses (‖z̃‖, ‖V_qᵀΔ‖ and the basis term
    E), because the exact-coordinate form of `groups/REPORT.md` §3.3 omits the basis rounding that Lemma 1.9 requires of
    the built index. The proof combines Lemmas 1.1 and 1.9 and Cauchy–Schwarz; no source writes it out, and the tag is
    kept at [proved, unrefereed]. The code also takes the tighter of the bounds for three values of α, which is sound
    because each holds.
14. The built kernel (comment at `rust/src/v2/kern.rs` lines 386–404, and `q.rs` where `int_thresholds` is called)
    differs from the design of `reports/surge/integer-kernel.txt` §1.3 in two places. It uses the signed remainder,
    −B_ℓ in the IN margin and +B_ℓ in the OUT margin, which the proof of Theorem 1.19 covers. It also applies the dial
    to the OUT side only (K_in carries h_fix and R_in carries w_ℓ(n) + e_fl without ψ), so for ψ ≠ 1 the built IN test
    is that of ψ = 1. Neither change is in a written source, and neither was refereed.
