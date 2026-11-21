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
| â, ã | the key x·m̂ of a row, and the value stored for it in 16 bits; rows are sorted by ã |
| DA | the bound the index assumes on \|ã − â\|: DA = 0.5/32767 + 10⁻⁷, half the key's quantum plus 10⁻⁷ (`rust/src/index.rs` line 4) |
| δ_â | the widening of the sorted range, which Lemma 1.5 needs to be at least \|ã − â\|; the index takes δ_â = DA + 10⁻⁹ (the referee of §1.2 writes δ, which chapter 0 keeps for a failure probability) |
| p̂, β | the anchor nearest the query, and β = q·p̂ as computed (Proposition 1.4 allows any number) |
| q_⊥, ρ_⊥ | q_⊥ = q − βp̂, and ρ_⊥ a number at least ‖q_⊥‖ (the cut's remainder norm of chapter 0) |
| v_m | v − (v·m̂)m̂ for any vector v, the part orthogonal to m̂ (so q_m and x_m below) |
| lo_c, hi_c | the stored extremes of p̂·x over cluster c, rounded outward |
| x₂, R | x₂ is a bound on ‖x‖² for every stored row (1 up to f16 rounding); R = √x₂ (§1.2 only) |
| a, U, L (§1.2 only) | a is a key value in [−R, R]; U(a) and L(a) are an upper and a lower bound on q·x for a row with key a, and U₁, U₂, L₁, L₂ the two pairs of Lemma 1.5a |
| ϱ, φ (Lemma 1.5b only) | the polar form of a bound's coefficients |
| ϑ, ϑ_t (§1.2 only) | the angle of q to m̂, and arccos(t/(‖q‖√x₂)); the referee writes φ and ψ |
| E_α (Proposition 1.6) | the basis-rounding term of the norm sort for a given α |
| ε_r (§1.8a only) | the absolute rounding error of a computed difference of squares before its square root |
| I_U, I_L, J_U, J_L, J⁺ (§1.2 only) | the key intervals of Lemma 1.5, the intervals the index computes in their place, and J⁺ the set of points within δ_â of an interval J |
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
| K_in, K_out, T_in, T_out, R_ℓ, B_ℓ, Θ, U, h, d, g (§1.8 only) | the integer kernel's thresholds, margins, sum of table offsets, accumulator, unit count, high-part width and the two words of its compare, as in `reports/surge/integer-kernel.txt`; they do not carry chapter 0's meanings there |
| Rin_ℓ(n), Rout_ℓ(n), X_ℓ(n), up(·) (§1.8 only) | the built kernel's IN and OUT margins for norm code n, the half-width in table units they carry, and the outward rounding of a margin, as in `program/kernel/referee-kernel.md` §1 |
| Hs, Ls, TH, TL, RH, RL (§1.8 only) | the high and low words of the accumulator, of a threshold and of a margin |
| ψ_ℓ, ψ_max (§1.8 only) | the dial of level ℓ (`Prep2::dial[ℓ]`) and its largest value over the deterministic levels; the referee writes D_ℓ and D_max, which this chapter keeps for weights |
| tol (§1.8 only) | the tolerance d of chapter 0 as the kernel receives it; d in §1.8 is a word of the compare |
| est, hh, e°_ℓ (§1.8 only) | the estimate q·m + α·m·r + η·ŝ_ℓ, the half-width η·X_ℓ(n) + h_fix, and e_ℓ less its multiplicative slack |
| n (as the argument of X_ℓ, Rin_ℓ, Rout_ℓ, ν_ℓ, w_ℓ) | a 4-bit norm code; elsewhere n is the dimension, as in chapter 0 |
| δ_u, mag_u (§1.8 only) | a bound on the error of unit u's table entries (Lemma 1.18), and the per-unit magnitude of Proposition 1.22 |
| ū, ρ_e (§1.8 only) | the f64 rounding unit 2⁻⁵³ (the report writes u, which §1.8 keeps as a unit's index), and an item's true weighted error norm at a level (the report writes ρ) |
| THETA_MAX, TMIN, TMAX, RIN_MIN, ROUT_MIN, RSENT, IHMAX (§1.8 only) | the kernel's range constants in `rust/src/v2/kern.rs` lines 406–412 |
| w_q, w_m, σ_m, n_p (§1.8 only) | a group's query weights, a cluster's centre weights, the step of the stored centre tables, and the number of units in a part |

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
is off by at most ‖m‖·DA, and the built certificate adds |α|·‖m‖·DA to its fixed term (§1.3). The identity needs
‖x‖ = 1; the index covers f16 rounding of the norm through x₂.

The referee of the cluster cut (`program/kernel/referee-range.md`, "Verdicts") read Lemmas 1.1 and 1.2 as stated here
and found no error. It adds that the first identity of Lemma 1.2 needs no unit norm, and that in general
‖r‖² = ‖x‖² − 2‖m‖â + ‖m‖². The error bound ‖m‖·DA on the m·r that the stored key gives holds only when
|x·m̂| ≤ 1 + DA, because the build clamps the key to [−1, 1] (§1.8a, defect 1). The kernel referee adds that the f32
evaluation of m·r from the key is covered by the fixed term only while |α|·‖m‖ ≤ 23 (§1.8a, defect 5).

**Proposition 1.3** (the centre correction).

(a) (Theorem B(i) of `reports/surge/level0/REPORT.txt` §1.) [proved, refereed] For unit q and α = q·m/‖m‖², the
offset Δ = q − αm is orthogonal to m and ‖Δ‖ = √(1 − (q·m̂)²) ≤ 1. Because α depends only on q and the centre, Δ is
fixed independently of ω.

*Proof.* Δ·m = q·m − α‖m‖² = 0. Then ‖Δ‖² = ‖q‖² − 2α·q·m + α²‖m‖² = 1 − (q·m)²/‖m‖² = 1 − (q·m̂)². ∎

The referee (`reports/surge/referee.txt` §3) found (a) correct, and added that the stored scalar of the source's
estimator is a cached function of the code, the centre and the rotation, so it adds no information. Part (a) is
Theorem 3.11 (i) of chapter 3, where it is used with the randomised level's estimator. The referee of the cluster cut
(`program/kernel/referee-range.md`, "Verdicts" and "The statements and proofs as they should stand") read (a) again,
found no error, and restates it with the same content and the same proof ("for unit q and α = q·m/‖m‖², Δ = q − αm is
orthogonal to m and ‖Δ‖² = 1 − (q·m̂)²"). Because the restatement adds nothing, the refereed wording above is kept. That
referee also notes that α plays no part in the cluster cut or in the sorted range on the key; it enters only `NuCut`
(Proposition 1.6) and the levels.

(b) (`agent2/REPORT.md` "Fixes", `combined/REPORT.md` item 3; statement and proof as restated in
`program/kernel/referee-range.md`, "The statements and proofs as they should stand".) [proved, unrefereed] For
positive diagonal D with Dm ≠ 0, ‖D(q − αm)‖ is least at α* = ⟨Dq, Dm⟩/‖Dm‖².

*Proof.* The square is ‖Dq‖² − 2α⟨Dq, Dm⟩ + α²‖Dm‖², a convex quadratic in α with vertex α*, because ‖Dm‖² > 0. ∎

The referee found no error in the chapter's earlier statement of (b), and its restatement adds the hypothesis Dm ≠ 0,
without which α* is undefined. The restatement is the referee's own, so it is tagged unrefereed.

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
rows, rounded outward; how they are stored is listed with the code's other departures at the end of this section. A
query uses one anchor for every cluster, the one nearest to it, chosen once per query (`anc_among`, `rust/src/v2/q.rs`
line 117).

The statements of this section are those of the referee of the cluster cut and the sorted range
(`program/kernel/referee-range.md`, "The statements and proofs as they should stand"), who read the code at commit
8c0b316f9 and restated the chapter's earlier statements so that they cover the index as built. Each restatement is the
referee's own and is therefore tagged [proved, unrefereed]. The referee read the earlier statements as well, and its
verdict on each is given beside the restatement.

*Setting.* A cluster has a unit direction m̂. Every row has ‖x‖² ≤ x₂; write R = √x₂ and â = x·m̂, so |â| ≤ R. The
stored key ã satisfies |ã − â| ≤ δ_â, and the rows are sorted by ã. For an anchor vector p̂ the cluster stores
lo_c ≤ p̂·x ≤ hi_c over its rows.

**Proposition 1.4** (the cluster cut; `SYSTEM.md` §2.2, `DESIGN.md` §3 steps 1–2, `agent3b/REPORT.md` §2 C2;
statement and proof as restated in `program/kernel/referee-range.md`). [proved, unrefereed] Let q and p̂ be any
vectors and β any number, q_⊥ = q − βp̂, ρ_⊥ ≥ ‖q_⊥‖, β̄_c = max(β·lo_c, β·hi_c) and β̲_c = min(β·lo_c, β·hi_c).
Then every row x of cluster c satisfies

    β̲_c − ρ_⊥R ≤ q·x ≤ β̄_c + ρ_⊥R.

If ‖p̂‖ = 1 and β = q·p̂, then ‖q_⊥‖² = ‖q‖² − β².

*Proof.* q·x = β(p̂·x) + q_⊥·x. Since lo_c ≤ p̂·x ≤ hi_c, the number β(p̂·x) lies between β·lo_c and β·hi_c, in either
order according to the sign of β, and hence in [β̲_c, β̄_c]. By Cauchy–Schwarz, |q_⊥·x| ≤ ‖q_⊥‖‖x‖ ≤ ρ_⊥R. For the
last claim, ‖q − βp̂‖² = ‖q‖² − 2β(q·p̂) + β²‖p̂‖² = ‖q‖² − β². ∎

The referee found no error in the chapter's earlier statement, which took ρ_⊥ = ‖q_⊥‖, and restated it for any p̂ and
any β once ρ_⊥ ≥ ‖q − βp̂‖ ("Verdicts"). The generality matters, because neither ‖p̂‖ = 1 nor β = q·p̂ holds exactly
in floating point: a loaded anchor is admitted at norm 1 ± 10⁻¹² (`check_anchors` in `rust/src/index.rs`), and β is a
computed dot product. The index computes ρ_⊥ = √max(‖q‖² − β², 0)·(1 + 10⁻¹²) + 10⁻¹² (`anc_of`, `rust/src/v2/q.rs`
line 182), and that number is not always at least ‖q_⊥‖ (§1.8a, defect 2).

A cluster whose lower bound reaches t is taken in unread, and one whose upper bound is below t is skipped unread. For a
unit query and a unit anchor, with β = q·p̂ exact, ρ_⊥ = √(1 − β²) suffices. The extremes stay valid when rows are
deleted and need only widen when rows are inserted, which is why the design keeps them for the write path
(`agent3b/REPORT.md` §2; the write path is not built).

In each cluster that survives the cut, the query bounds each row from its key alone, in two ways.

**Lemma 1.5a** (two bounds from the key; `agent3b/REPORT.md` §2 C1 for the annulus, the chapter's earlier text for
the table plus remainder; statement and proof as restated in `program/kernel/referee-range.md`). [proved, unrefereed] For every
row x of the cluster,

(i) (annulus) |q·x − (q·m̂)â| ≤ ‖q_m‖·√(R² − â²);

(ii) (table plus remainder) β̲_c + (q_⊥·m̂)â − ‖(q_⊥)_m‖·√(R² − â²) ≤ q·x ≤ β̄_c + (q_⊥·m̂)â + ‖(q_⊥)_m‖·√(R² − â²).

Here ‖v_m‖² = ‖v‖² − (v·m̂)², and q_⊥·m̂ = q·m̂ − β·p̂·m̂ is one number per (query, cluster).

*Proof.* Write x = âm̂ + x_m with x_m ⟂ m̂, so ‖x_m‖² = ‖x‖² − â² ≤ R² − â². For any vector v,
v·x = (v·m̂)â + v·x_m = (v·m̂)â + v_m·x_m, because x_m ⟂ m̂, and |v_m·x_m| ≤ ‖v_m‖·√(R² − â²). Taking v = q gives (i).
For (ii), q·x = β(p̂·x) + q_⊥·x. The first term lies in [β̲_c, β̄_c] as in Proposition 1.4, and v = q_⊥ bounds the
second. ∎

Each bound has the form U(a) = a₀ + a₁a + a₂√(R² − a²) or L(a) = b₀ + a₁a − a₂√(R² − a²) on [−R, R] with a₂ ≥ 0. A
bound stays valid when a₂ and a₀ are replaced by larger numbers and b₀ by a smaller one, which is what lets the index
round these constants outward. (The referee writes c, p, r and c′ for a₀, a₁, a₂ and b₀; this chapter keeps c for a
cluster and r for a residual.)

**Lemma 1.5b** (the shape of the bounds; statement and proof as written in `program/kernel/referee-range.md`).
[proved, unrefereed] U is concave and L convex on [−R, R], and both are continuous. For every t, {a : U(a) ≥ t} is a
closed interval and {a : L(a) < t} an interval open in [−R, R]; either may be empty. With a = R cos θ, θ ∈ [0, π],
ϱ = √(a₁² + a₂²) and φ = atan2(a₂, a₁) ∈ [0, π],

    U = a₀ + ϱR cos(θ − φ),    L = b₀ + ϱR cos(θ + φ).

*Proof.* The map a ↦ √(R² − a²) is continuous on [−R, R], with second derivative −R²(R² − a²)^(−3/2) < 0 inside, so
it is concave. Since a₂ ≥ 0 and a₁a is linear, U is concave and L convex. If U(a₁′) ≥ t and U(a₂′) ≥ t, then for
λ ∈ [0, 1], U(λa₁′ + (1 − λ)a₂′) ≥ λU(a₁′) + (1 − λ)U(a₂′) ≥ t, so the set is convex, hence an interval, and it is
closed by continuity. The same argument with L(λa₁′ + (1 − λ)a₂′) ≤ λL(a₁′) + (1 − λ)L(a₂′) < t gives the second
claim. The trigonometric forms follow from a₁ = ϱ cos φ, a₂ = ϱ sin φ and √(R² − a²) = R sin θ. ∎

Nothing here depends on the sign of a₁ (that is, of q·m̂ or q_⊥·m̂), on the sign or size of t, or on α, which does not
occur. At a = ±R the square root is 0, and both bounds equal their constant plus or minus a₁R. (In the proof, a₁′ and
a₂′ are two key values; the referee writes a₁ and a₂.)

**Lemma 1.5** (one contiguous undecided range, for the stored key; `agent3b/REPORT.md` §2 C1, `combined/REPORT.md`
item 2, `DESIGN.md` §3 step 3; statement and proof as restated in `program/kernel/referee-range.md`).
[proved, unrefereed] Let U₁, U₂ and L₁, L₂ be the bounds of Lemma 1.5a, t a threshold, I_U = {U₁ ≥ t} ∩ {U₂ ≥ t} and
I_L = {L₁ < t} ∩ {L₂ < t}. Let J_U ⊇ I_U and J_L ⊇ I_L be intervals, and J⁺ the set of points within δ_â of an
interval J. Call a row *out* if ã ∉ J_U⁺, *in* if ã ∈ J_U⁺ and ã ∉ J_L⁺, and *undecided* if ã ∈ J_U⁺ ∩ J_L⁺. Then

(a) every out row has q·x < t;

(b) every in row has q·x ≥ t;

(c) in the order of ã, the rows that are not out form one contiguous range, the undecided rows form one contiguous
range inside it, and the in rows are the at most two ranges between them;

(d) with exact keys (δ_â = 0, ã = â, J_U = I_U, J_L = I_L), the undecided rows are exactly those with
min(U₁, U₂)(â) ≥ t and max(L₁, L₂)(â) < t.

The lemma does not say at which end the out rows or the in rows lie; each kind can lie at either end, or at both.

*Proof.* I_U and I_L are intervals by Lemma 1.5b, as intersections of two intervals. (a) If ã ∉ J_U⁺ then, as
|ã − â| ≤ δ_â, â ∉ J_U ⊇ I_U, so U_i(â) < t for some i, and q·x ≤ U_i(â) < t by Lemma 1.5a. (b) If ã ∉ J_L⁺ then
â ∉ I_L; since â ∈ [−R, R], L_i(â) ≥ t for some i, and q·x ≥ L_i(â) ≥ t. (c) J_U⁺ and J_L⁺ are intervals, and so is
their intersection. Membership of a row in each depends on ã alone, so rows with equal keys fall together, and the rows
whose sort key lies in an interval are contiguous in the sort order. The in rows are those of the first range outside
the second, which is nested in it. (d) is the definition. ∎

*What the index takes.* `sorted_range2` (`rust/src/v2/q.rs` line 715) does not evaluate a bound row by row. It solves
U ≥ t and L < t in the closed forms of Lemma 1.5b (`sup_set`, `inf_set`, `isect`) and takes

    J_U = cl{U₁ ≥ t − 10⁻⁹} ∩ cl{U₂ ≥ t − 10⁻⁹},    J_L = cl{L₁ < t + 10⁻⁹} ∩ cl{L₂ < t + 10⁻⁹},    δ_â = DA + 10⁻⁹,

with DA = 0.5/32767 + 10⁻⁷. It then finds the rows whose stored key lies in J_U⁺ and in J_L⁺ with two `partition_point`
calls each. No bound is evaluated at a rounded key, so the set returned is a range of rows by construction. The lemma
then applies under two hypotheses that the code does not secure in every case: |ã − â| ≤ DA, which the clamp of the
key breaks (§1.8a, defect 1), and the outward rounding of the bounds' constants, which the square roots of cancelling
differences break (§1.8a, defect 2). The visible rows in the undecided range are the cluster's candidates.

The referee's verdict on the chapter's earlier statement, which was about rows sorted by the true key â, is that it
and its proof are correct for exact keys; that statement is part (d) here. For the index the lemma had to be restated
for the stored key with a widening, and it must not say at which end the decided rows lie.

Numerical checks [model] (`referee-range.md` F2, F7; float64 transcriptions of `anc_of`, `cut_params2`, `sup_set`,
`inf_set`, `isect`, `sorted_range2`, `support_pre`, `bound_up` and `bound_down`, the key and x₂ of the build, on small
synthetic indexes with n = 3 to 32, f16 rows, 8-bit centres and adversarial queries and thresholds, with exact
rational comparisons near t):

- `closed_form_check.py` compared `sup_set` and `inf_set` with the bound on a grid of 4,003 keys for 20,000 random
  coefficient sets, including a₂ down to 10⁻⁹ and a₁ = 0. No grid point satisfying the inequality fell outside the
  returned interval, and none failed it by more than 10⁻⁹ inside, so the code's interval is the lemma's.
- Over seeds 2, 3 and 5 (872,256 checks, 26.8M rows decided), the real-valued undecided set at the true keys was never
  non-contiguous in true-key order. Read in the stored order it was non-contiguous in 3,887 checks, through rows at
  equal stored keys with different true keys; this is why the lemma is stated for ã. All 1,729 wrong decisions were
  rows with |x·m̂| > 1 + DA (defect 1), and none occurred with the key in range.
- Along increasing key, with O for out, I for in and u for undecided, seed 2 gave the arrangements u 107,135; uO 36,872;
  uI 36,267; Ou 25,858; OuI 25,332; Iu 24,459; OuO 24,056; IuI 20,695; O 20,476; IuO 18,683; I 12,798; OI 2,215;
  IO 1,438. So out rows and in rows each occur at either end, and no arrangement had an out run and an in run at the
  same end.

*Which end.* For the annulus bound alone, with ϑ the angle of q to m̂ and ϑ_t = arccos(t/(‖q‖√x₂)), the referee states
that the high-key end is out when ϑ > ϑ_t and in when ϑ < ϑ_t, and that the low-key end is out when ϑ + ϑ_t < π and in
when ϑ + ϑ_t > π [sketch: gap named] (`referee-range.md`, "Verdicts"; the referee writes φ and ψ for ϑ and ϑ_t, and the
gap is that the report gives the rule without a proof). The description in `SYSTEM.md` §2.2 ("certain matches above it
and certain non-matches below") is therefore wrong in general (Editor's query 6).

Lemma 4.10 of chapter 4 states the annulus case of the earlier lemma, from the same source (`agent3b/REPORT.md` §2 C1),
and uses it for the "range" cut of its scaling measurements.

**Proposition 1.6** (sorting by the residual's norm; `groups/REPORT.md` §3.3, `DESIGN.md` §9 item 2, the comment on
`NuCut` in `rust/src/v2/q.rs` lines 634–635; statement and proof as restated in `program/kernel/referee-range.md`).
[proved, unrefereed] Let V_q be the stored basis, and for each row let z̃_x be the stored coordinates of r = x − m and
ε = r − V_q z̃_x, with ‖ε‖ ≤ e_inv‖r‖. Let ν_x ≥ ‖z̃_x‖, and let b_min ≤ m·r ≤ b_max and r_max ≥ ‖r‖ over the
cluster. For any α chosen from q and the cluster alone, with Δ = q − αm, D_α ≥ ‖V_qᵀΔ‖ and
E_α ≥ e_inv(‖q‖ + |α|‖m‖)r_max,

    q·m + min(αb_min, αb_max) − D_α·ν_x − E_α ≤ q·x ≤ q·m + max(αb_min, αb_max) + D_α·ν_x + E_α.

The upper end is non-decreasing and the lower end non-increasing in ν_x, and so are the least upper end and the
greatest lower end over any finite set of values of α. Hence, in a cluster sorted by ν_x, the rows whose upper end is
below t form a prefix, the rows whose lower end is at least t form a prefix, and at most one of the two prefixes is
non-empty.

*Proof.* q·x = q·m + q·r = q·m + α(m·r) + Δ·r (Lemma 1.1). The term α(m·r) lies between αb_min and αb_max. Next,
Δ·r = Δ·V_q z̃_x + Δ·ε = (V_qᵀΔ)·z̃_x + Δ·ε. By Cauchy–Schwarz, |(V_qᵀΔ)·z̃_x| ≤ ‖V_qᵀΔ‖‖z̃_x‖ ≤ D_αν_x, and
|Δ·ε| ≤ ‖Δ‖e_inv‖r‖ ≤ (‖q‖ + |α|‖m‖)e_inv r_max ≤ E_α. This gives the two inequalities. Since D_α ≥ 0, the ends are
monotone in ν_x; a minimum of non-decreasing functions is non-decreasing, and a maximum of non-increasing functions is
non-increasing. A monotone condition on the sort key holds on a prefix. If both prefixes were non-empty, the first row
would have its upper end below t and its lower end at least t, which contradicts lower end ≤ q·x ≤ upper end. ∎

For the exact dual basis, ‖z̃_x‖ = ‖r‖_{M⁻¹} and ‖V_qᵀΔ‖ = ‖Δ‖_M by §1.4; the referee notes that it did not referee
§1.4. `groups/REPORT.md` §3.3 states the bound in one sentence, in exact coordinates and without E_α. The referee found
no error in the chapter's earlier statement of this proposition or in its proof, basis term included (Editor's query
13). Its restatement adds the last claim, that at most one prefix is non-empty, and the extension to the best of
several values of α, which the code uses: `NuCut` takes the tighter of the bounds for the three values of α that its comment names,
α ∈ {qv·mv/‖mv‖², q·m/‖m‖², 0} (q.rs line 634, with qv and mv the query and centre in the group's coordinates).

The referee's model of `NuCut` (`nucut_attack.py`: int8 basis with one scale per vector, Newton-polished inverse, e_inv,
ν rounded up to 16 bits on the cluster's grid, b_min, b_max and r_max from the stored keys, three values of α, the
cut-offs and their margins; n = 4 to 32; seeds 1 to 3) made 244,800 checks with 15.0M row decisions offered and 1.87M
taken [model]. It found 748 wrong decisions and 979 exact scores outside the bounds of their norm code, all of them rows
with |x·m̂| > 1 + DA, whose m·r leaves [b_min, b_max] because b_min and b_max come from the clamped key (§1.8a,
defect 1); it found none otherwise. The hypothesis ‖ε‖ ≤ e_inv‖r‖ is not established by the code as a bound (§1.8a,
defect 7).

The index as measured sorts by the stored key ã. The ν-sort is implemented in the code (`NuCut`), but
`dualproto/REPORT.md` §7 lists it as not run natively, and `SYSTEM.md` does not use it. With it, the support table and
its anchors could be dropped.

*Departures of the code from the earlier text of this section* (`referee-range.md` F0 and F8, from the code at commit
8c0b316f9):

- *The support table.* lo_c and hi_c are stored as f32 on a grid of 2⁻¹⁵, 4 bytes a value in memory (`Index2::hi`,
  `Index2::lo`), and not in 2 bytes each as this section said before. Each is the exact f64 extreme moved outward by
  10⁻⁹ and then rounded outward to the grid (`bound_up`, `bound_down` in `rust/src/v2/mod.rs`). The f32 screen in
  `support_bounds`, which recomputes in f64 every row within twice the f32 error of the f32 extreme, is argued
  correctly for unit anchors and any summation order. The referee found no error in it. Editor's query 15 asks about
  the size on disk.
- *The key and DA.* `rows_of` (`rust/src/v2/mod.rs` line 1583) computes â = x·m̂ in f64 from the f16 row and the f64
  direction m̂ = m/‖m‖ of the stored centre, stores round(clamp(â, −1, 1)·32767) as an i16 (line 1589), and sorts the
  cluster by (stored key, row). DA = 0.5/32767 + 10⁻⁷ is half the key's quantum plus 10⁻⁷, whereas this chapter called
  δ_â the quantum. The clamp is defect 1 of §1.8a.
- *x₂.* x₂ = 1 + max over rows of (‖x‖² − 1)·(1 + 10⁻⁹) + 10⁻¹², and never below 1 + 10⁻¹², from the f16 rows in f64.
- *The threshold.* `sorted_range2` puts 10⁻⁹ on the threshold as well as widening the key interval, whereas this
  section mentioned only the widening.
- *The single anchor.* The query uses one anchor, the nearest, for every cluster, and the remark ρ_⊥ = √(1 − β²) needs
  ‖p̂‖ = 1 and β = q·p̂ exactly, which floating point does not give. This is why Proposition 1.4 is stated for any p̂ and
  β, and why the rounding of ρ_⊥ matters (defect 2).
- *e_inv.* e_inv = (the computed Frobenius defect of V·X)·(1 + 10⁻⁶) + 10⁻¹² (`rust/src/v2/mod.rs` line 747), where X
  is the computed inverse. It does not bound the rounding of z̃ = fl(X·r) or of the defect itself. The referee
  estimates that rounding, with standard bounds, at up to about 5·10⁻¹⁰ relative at n = 768 and condition 227, and
  typically far less [model]. The +10⁻⁹ in `NuCut`'s E_α (q.rs line 654) and the 4·10⁻⁶ in the levels' fixed term
  (q.rs line 574) absorb it, but no source says so. It belongs with the gap named in 1.10 (§1.8a, defect 7).
- *The centre's 8 bits.* They have no effect on soundness, because the key, q·m and p̂·m̂ all use the same stored centre
  and the same m̂ = m/‖m‖ in f64.

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
h_fix = |α|·‖m‖·DA + e_inv·(‖q‖ + |α|·‖m‖)·r_max + 4·10⁻⁶ (`rust/src/v2/q.rs` line 574) covers the key's
rounding and the basis defect, with r_max the largest residual norm in the cluster. This is (A0) of the source. When the index evaluates it in f32, it also adds
a bound on the tables' quantisation (`edet`) and one on the f32 evaluation (`fl`); the integer kernel of §1.8 replaces
both by the term e_ℓ of (1). The gap: the algebra follows from the three results above, but the floating-point
allowances (e_fl, `edet`, `fl` and the constant 4·10⁻⁶) are justified in code comments and checked in verify mode, and
no written source proves them. Verify mode recomputes every decision in float64; it found
0 deterministic violations in 731M, 605M and 512M interval checks on the three basis variants (`dualproto/REPORT.md`
§2), and 0 of 10.6M integer decisions that the float64 bound did not imply (`rust/REPORT_BOUNDS.md`, correctness table)
[measured].

The kernel referee (`program/kernel/referee-kernel.md` §3e) examined three of the inputs (A0) takes from the key and
the weights, which the f32 path and the integer kernel share, and the gap stays named because each holds only under a
condition that the code does not assert.

- *The item scale η.* The encoder (`item_scale`, `rust/src/index.rs` line 11) and the query (q.rs lines 1286–1289)
  apply the same f32 operations in the same order to the same key, the stored norm code is relative to that η, and
  verify mode asserts the equality. η ≥ √(10⁻¹²/Σλ) > 0, with Σλ the denominator of η in §1.6. No rounding question arises, and the referee found no error.
- *The query weights.* w_q = fl32(qv·sig), with qv exact to one f64 rounding, so the weights are rounded to nearest. The
  term η·e_fl = η·12·2⁻²⁴·(Σ|w_q| + |α|Σ|w_m|) covers them while the largest decoded coordinate value is below about 6
  on the path with own tables and 24 on the path with combined tables. The referee's count needs (1 to 2)·2⁻²⁴ per
  weight times that value. A code comment puts the value below 3.5, and the referee found no assertion of it. No error
  found within that condition.
- *m·r.* m·r is computed in f32 from the key, and its allowance in h_fix is |α|·‖m‖·DA + 4·10⁻⁶. The f32 evaluation
  differs from ‖m‖ã − ‖m‖² by up to 2.7·10⁻⁷·‖m‖ over every key and 404 centre norms [model], which is more than the
  10⁻⁷ in DA; the rest must come out of the absolute 4·10⁻⁶, which suffices while |α|·‖m‖ ≤ 23 (§1.8a, defect 5). The
  clamp of the key is a separate loss (§1.8a, defect 1).

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

This section states the kernel as built at commit 8c0b316f9, as the referee of the built kernel read it from the code
(`program/kernel/referee-kernel.md` §1: `rust/src/v2/kern.rs` lines 386–412, `rtab`, `int_thresholds` at lines 518
and 549, `cmp2w`, `int_level`; `rust/src/v2/q.rs` lines 1310–1320 for the thresholds, 1394–1413 for the margins,
1578–1600 for the masks, and `check_det`). The design of `reports/surge/integer-kernel.txt`, which an earlier referee
checked (`reports/surge/referee.txt` §6), differs from the build in the ways listed after Proposition 1.22. Lemmas 1.18
and 1.20 are the design's, and they hold for the build with the same proofs. Lemmas 1.18a and 1.18b, Theorem 1.19,
Proposition 1.21′ and Proposition 1.22 are stated as built. Their proofs are the kernel referee's, so they are tagged
[proved, unrefereed]. The referee records a path it could not break as "no error found", not as "correct". Its
numerical attacks run float64 and exact-rational transcriptions of the kernel's arithmetic on synthetic inputs, not the
binary, so their counts are tagged [model].

In this section q·m, α, h_fix and m·r denote the f32 values the code uses (`qm`, `alr`, `fixed`, `mr`), widened to
f64. σ is one f32 step shared by a cluster's deterministic levels, and η > 0 is the item scale, computed in f32 from
the 16-bit key.

*Tables as built* (`referee-kernel.md` §1 and §3d). For a 4-bit unit u with ideal table L_u(v) (real arithmetic on the
f32 weights), the stored entry θ_u(v) is an integer in [0, 65,535], held as a high part and a low byte, and b_u is the
unit's offset. On the path with own tables (`build_lut`), the computed table t_u(v) is an f32 FMA chain over at most
four fields, b_u is the f32 sum of the fields' minima, and θ_u(v) = rne((t_u(v) − b_u)·fl(1/σ)), clamped to
[0, 65,535]. On the default path the tables are combined from the group's and the centre's (Proposition 1.22). B_ℓ is
the signed sum of the offsets over the units through level ℓ, rounded to f32 per part. No remainder is taken, and no
per-level constant is added to the accumulators, which are unsigned sums. e_ℓ is the sum of the error bounds of the
parts through level ℓ, times (1 + 10⁻⁶). For a part of n_p units with own tables the bound is

    n_p·σ/2·(1 + 10⁻⁶) + (48 + 2n_p)·2⁻²⁴·Σ|w|·max|E| + 10⁻³⁰,

where Σ|w|·max|E| is the code's bound on the magnitude of the part's table values (the weights' magnitudes times the
largest decoded field values). The step is chosen so that the clamp never acts: σ ≥ (largest unit range)·(1 + 1/512)/
65,535 (`int_sigma`) keeps every entry below 65,408, and a slightly negative t_u(v) − b_u rounds to 0 within the float
term. The code asserts that the sum of the units' largest entries is at most 256·IHMAX, so Θ below lies in
[0, THETA_MAX].

**Lemma 1.18** (the accumulators; `reports/surge/integer-kernel.txt` §1.2, equation (1)). [proved, refereed] Let Hs be
the sum of the high parts and Ls the sum of the low bytes looked up for an item through level ℓ, and Θ = 256·Hs + Ls.
Then Θ is exactly the sum of the θ_u looked up. If, moreover, every entry looked up satisfies
|σθ_u(v) + b_u − L_u(v)| ≤ δ_u, and B_ℓ = Σ b_u and e_ℓ ≥ Σ δ_u over the units through level ℓ, then

    |σΘ + B_ℓ − ŝ_ℓ| ≤ e_ℓ.                                                    (1)

*Proof.* 256·hi + lo reconstructs each θ_u exactly, so Θ = Σθ_u. Summing the per-unit bounds over the units read gives
(1). ∎

The design took B_ℓ as a remainder with |B_ℓ| < σ and added ⌊B_ℓ/σ⌋ to the accumulators. As built, B_ℓ is the whole
signed sum. The kernel referee read the lemma in that form and found the proof unchanged, because it never uses
|B_ℓ| < σ (`referee-kernel.md` §2), so (1) holds with the built B_ℓ whenever each unit's entry is within its
share of e_ℓ of the ideal table. That the code's e_ℓ is such a bound is the referee's §3d
[proved, unrefereed]. On the path with own tables, the referee's count of the float part is 9 units of
2⁻²⁴·Σ|w|·max|E| (the FMA chain 2, the offset 3.5, the subtraction 1, the product with 1/σ 2, the offset's cast 0.5)
against the 48 allowed. The combined path is Proposition 1.22. Every `as f32` on an error bound rounds to nearest and
can lose 3·10⁻⁸ relative, which is covered because each bound's leading term carries 1 + 10⁻⁶, e_ℓ carries a further
1 + 10⁻⁶, and the float terms have a factor of 2 or more in hand. The float constants are the referee's count by hand,
not a machine-checked analysis. In the referee's model (`k4_tables.py`: two seeds of 600 trials over five weight
regimes, five level schedules up to 64 units and three table shapes, own and combined tables), the worst case of (1)
over all codes reached 0.976·e_ℓ and never exceeded it in 3,600 level checks per seed, and no entry was clamped
[model]. With own tables e_ℓ came to 0.51–0.63 steps σ per unit, against the design's 0.5.

*Margins as built* (`referee-kernel.md` §1; `rtab` in `rust/src/v2/kern.rs` line 462). For a norm code n < 15, in f64,

    X_ℓ(n) = dwa_ℓ·pconst_ℓ·(1 + 10⁻⁶)·ntab_ℓ[n] + ewdet,

which is w_ℓ(n) + e_fl of 1.10: dwa_ℓ is the f32 upper bound on ‖D_ℓΔ‖, pconst_ℓ·ntab_ℓ[n] is the decoded norm
ν_ℓ(n), and ewdet is the weight-rounding term e_fl. Then

    Rin_ℓ(n) = up((X_ℓ(n) + e_ℓ − B_ℓ)/σ),    Rout_ℓ(n) = up((B_ℓ + e_ℓ + ψ_ℓ·X_ℓ(n))/σ),    up(x) = ⌈x + (|x| + 1)·10⁻¹²⌉.

A margin is kept only if RIN_MIN ≤ Rin_ℓ(n) < RSENT (ROUT_MIN ≤ Rout_ℓ(n) < RSENT). Otherwise, and for the escape code
n = 15, it is RSENT. The constants are RIN_MIN = THETA_MAX − TMAX + 1 = −4,047,166, ROUT_MIN = TMIN = −6,144,000,
RSENT = 8,388,352, THETA_MAX = 2,097,088 and TMAX = 6,144,255.

*Thresholds as built* (`referee-kernel.md` §1; `int_thresholds`, called from q.rs lines 1310–1320). Per item, once, in
f64,

    K_in = (t − tol + h_fix − q·m − α·m·r)/η,          K_out = (t − ψ_max·h_fix − q·m − α·m·r)/η,
    T_in = clamp(⌈y_in + slack + |y_in|·2⁻⁴⁶⌉),        T_out = clamp(⌊y_out − slack − |y_out|·2⁻⁴⁶⌋),

with y = fl(K/σ), slack = (|t| + |h_fix| + |q·m| + |α·m·r|)·2⁻⁴⁶/(ησ), and the clamp to [TMIN, TMAX]. When `out.iv` is
set, K_in = +∞ and T_in = TMAX, so the integer levels take nothing in.

*The rule.* With Θ the exact sum of the θ_u of every unit through level ℓ, as in Lemma 1.18,

    IN at level ℓ  iff  Θ − Rin_ℓ(n) ≥ T_in  (non-strict),       OUT at level ℓ  iff  Θ + Rout_ℓ(n) < T_out  (strict),

both evaluated by the two-word compare of Lemma 1.20 and Proposition 1.21′ and then masked by the live lanes. The real
statement each is meant to imply, which `check_det` tests in verify mode, is, with est = q·m + α·m·r + η·ŝ_ℓ and
hh = η·X_ℓ(n) + h_fix,

    IN ⟹ est − hh ≥ t − tol,        OUT ⟹ est + ψ_ℓ·hh < t.

**Lemma 1.18a** (the thresholds in f64; `referee-kernel.md` §3b). [proved, unrefereed] Let ū = 2⁻⁵³, and let η, m·r, α,
q·m, h_fix and σ be f32 values, exact in f64. Then, to first order in ū, the argument of the ceiling in T_in is at
least K_in/σ, and the argument of the floor in T_out is at most K_out/σ, for any tol and any dial.

*Proof.* Write mag = |t| + h_fix + |q·m| and a = fl(α·m·r). The caller's c_in = fl(fl(fl(t − tol) + h_fix) − q·m)
differs from its exact value by at most ū(|t − tol| + |t − tol + h_fix| + |c_in|) ≤ ū(3|c_in| + 2·mag), using
|t − tol| ≤ |c_in| + h_fix + |q·m|. The same bound holds for c_out whatever ψ_max is, because
ψ_max·h_fix ≤ |t| + |c_out| + |q·m|. Then a, the subtraction, the reciprocal fl(1/η) and two products each add a relative ū, so

    |y − K/σ| ≤ 8ū·|K/σ| + (4ū·|a| + 2ū·mag)/(ησ)          (first order).

The slack as computed is at least 127ū·(mag + |a|)/(ησ) and the relative term at least 127ū·|y|, while the two
additions lose at most 2ū(|y| + slack). So the argument of the ceiling is at least K_in/σ and that of the floor at most
K_out/σ, with a factor of about 14 to spare. ∎

The argument is first order in u, as the report gives it. It was argued in no written source before, and it settles
Editor's query 7: the dial above 1 is covered for any dial. The referee's attack (`k2_thresholds.py`: 1,700,000 cases
against exact rationals, with η at its floor, |α| in the hundreds, dials to 10⁶, tol to 10³, t placed so that K/σ is an
integer to f64 accuracy and stepped by ±3 ulps, and c_in − a at cancellation) found 0 violations [model]. The outward
rounding cost 0 steps beyond the exact ceiling or floor in the best case, so the slack is not wasteful.

**Lemma 1.18b** (the margins in f64; `referee-kernel.md` §3c). [proved, unrefereed] Let e°_ℓ be e_ℓ less its
multiplicative slack (its two factors of 1 + 10⁻⁶). The report takes e°_ℓ still to bound the error in (1) without
writing that step (Editor's query 16), and Theorem 1.19 assumes it. Then

(i) σ·Rout_ℓ(n) ≥ B_ℓ + e°_ℓ + ψ_ℓ·X_ℓ(n);

(ii) σ·Rin_ℓ(n) ≥ X_ℓ(n) + e_ℓ − B_ℓ when B_ℓ < 0;

(iii) in real arithmetic, pconst_ℓ·η·ntab_ℓ[n]·(1 + 10⁻⁶) exceeds the item's true weighted error norm ρ_e (the
quantity that η·ν_ℓ bounds; the report writes ρ) by a factor of about 1 + 1.9·10⁻⁶, which covers the two f32 products and the f64
roundings in X_ℓ(n).

*Proof.* (ii) When B_ℓ < 0, the usual case because B_ℓ is a sum of minima of tables that straddle 0, the quotient
(X + e − B)/σ has no cancellation, so its f64 error is about 3·2⁻⁵³ relative and `up` covers it. (i) The quotient
(B + e + ψ_ℓX)/σ can cancel. |B_ℓ|/σ is up to about 2²⁰ steps, so the f64 sum can be off by 2⁻⁵³·2²¹ ≈ 2·10⁻¹⁰ steps,
while `up` adds only 10⁻¹² when the quotient is near 0. So `up` alone does not prove the claim. What closes it is the
slack in e_ℓ: a shortfall is at most 2·10⁻¹⁰ steps, and e_ℓ carries at least 4·10⁻⁶ steps beyond half a step per unit
(its two factors of 1 + 10⁻⁶ on at least 8 units), so σ·Rout ≥ B + e°_ℓ + ψ_ℓX still holds. (iii) X is computed in f64
from f32 factors, with three roundings. The encoder guarantees fl32(fl32(pconst·η)·ntab[n]) ≥ ρ_e(1 + 10⁻⁶) + 10⁻¹² for
the true weighted error norm ρ_e (`rust/src/v2/mod.rs` lines 1693–1706, checked by a loop on the stored code), so the real
pconst·η·ntab[n]·(1 + 10⁻⁶) exceeds ρ_e by a factor of about 1 + 1.9·10⁻⁶. That covers the two f32 products (1.2·10⁻⁷)
and the f64 roundings. ∎

The report argues (ii) only for B_ℓ < 0 (Editor's query 16). The referee's attack (`k3_margins.py`: 296,012 cases placed
at cancellation) found that the computed quotient fell below an integer that the exact one exceeded 63 times, all at
|R| ≥ 1,859, by at most 1.0·10⁻¹⁰ steps, where `up` adds at least 1.9·10⁻⁹ [model]. Near small integers it never did,
because integer multiples of σ lie on the f64 grid of the summands, σ being an f32. Rout was short of the exact quotient
in 0 cases and Rin in 0.

**Theorem 1.19** (integer decisions are implied by the real bound, as built; Theorem D of
`reports/surge/integer-kernel.txt` §1.3 for the design, renamed here to avoid the Theorem D of the consolidated proofs,
which is Theorem 2.11 of chapter 2; statement and proof as built from `referee-kernel.md` §2 and §6).
[proved, unrefereed] Suppose that (1) holds with the built B_ℓ and with an allowance e°_ℓ ≤ e_ℓ, that
σ·Rin_ℓ(n) ≥ X_ℓ(n) + e°_ℓ − B_ℓ and σ·Rout_ℓ(n) ≥ B_ℓ + e°_ℓ + ψ_ℓ·X_ℓ(n) (Lemma 1.18b), and that T_in ≥ K_in/σ and
T_out ≤ K_out/σ (Lemma 1.18a, for thresholds the clamp leaves unchanged; Proposition 1.21′ deals with clamped ones).
Let h_fix ≥ 0, X_ℓ(n) ≥ 0 and ψ_ℓ ≤ ψ_max. Then

(i) IN at level ℓ implies est − hh ≥ t − tol, which is the lower end of the certificate of 1.10 at ψ = 1, whatever the
dial;

(ii) OUT at level ℓ implies est + ψ_ℓ·hh < t. For ψ_ℓ ≥ 1 this implies that the upper end of the certificate at ψ = 1
is below t; for ψ_ℓ < 1 it is a statistical exclusion and nothing more.

*Proof.* (i) Θ ≥ T_in + Rin_ℓ(n) gives σΘ ≥ K_in + X + e°_ℓ − B_ℓ, so ŝ_ℓ ≥ σΘ + B_ℓ − e°_ℓ ≥ K_in + X by (1).
Multiplying by η > 0 gives q·m + α·m·r + η·ŝ_ℓ − (η·X + h_fix) ≥ t − tol, which is est − hh ≥ t − tol. (ii)
Θ + Rout_ℓ(n) < T_out gives σΘ + B_ℓ + e°_ℓ + ψ_ℓ·X < σT_out ≤ K_out, so ŝ_ℓ + ψ_ℓ·X < K_out by (1), and
q·m + α·m·r + η·ŝ_ℓ + ψ_ℓ·η·X + ψ_max·h_fix < t. Because h_fix ≥ 0 and ψ_max ≥ ψ_ℓ, est + ψ_ℓ(η·X + h_fix) < t. When
ψ_ℓ ≥ 1, hh ≥ 0 gives est + hh ≤ est + ψ_ℓ·hh < t. ∎

The design's Theorem 1.19 scaled both sides by one dial ψ, used |B_ℓ| with B_ℓ a remainder, and was found correct
(`reports/surge/referee.txt` §6). The kernel referee shows that the built form follows by the same argument: the design's
proof uses |B_ℓ| only through −|B_ℓ| ≤ B_ℓ ≤ |B_ℓ|, and putting B_ℓ itself in each place makes those two steps
equalities, while the dial is set to 1 on the IN side. For ψ = 1 the built decisions are the design's with the tighter
signed margin. Every rounding widens: Rin and Rout are rounded up, T_in up and T_out down. Ties fall on the safe side,
because IN is non-strict and OUT strict at every step. An item whose score equals t therefore cannot be OUT while (A0)
holds, and may be IN only if its lower bound equals its score; Θ + Rout = T_out leaves the item undecided
(`referee-kernel.md` §4).

**Lemma 1.20** (the two-word compare; `reports/surge/integer-kernel.txt` §1.4). [proved, refereed] For integers d and
g, 256d + g ≥ 0 if and only if d + ⌊g/256⌋ ≥ 0.

*Proof.* Write g = 256f + r with 0 ≤ r < 256. If d + f ≤ −1 then 256(d + f) + r ≤ −1. If d + f ≥ 0 the sum is at least
0. ∎

The kernel evaluates d = Hs − TH ∓ RH and g = Ls − TL ∓ RL on the high and low words of the thresholds and margins and
tests d + (g >> 8) with an arithmetic shift, so the lemma makes the 16-bit compare equal to the full-width one when no
word overflows. Proposition 1.21′ deals with overflow.

**1.21 Saturation (the design).** The source's range paragraph chose h = 7 and R_cap = 16,000 so that
2U(2^h − 1) + R_cap + 64 ≤ 32,767 with U = 64 units, and concluded that the saturating additions never saturate.
[refuted] The referee showed that the budget counts R_cap once: on unmasked lanes Hs − TH − RH reaches −48,256, which
wraps to +17,280 under wrapping arithmetic and gives a false IN (`reports/surge/referee.txt` §6). Two repairs work:
R_cap ≤ 8,222, or every operation saturating in the order (Hs − TH), then ∓RH, then +(g >> 8), which gave 0 sign
mismatches in 4,000,000 random cases. The built kernel takes the second repair, with signed margins.

**Proposition 1.21′** (saturation as built; `referee-kernel.md` §3a, from `cmp2w` and the comment at
`rust/src/v2/kern.rs` lines 398–404). [proved, unrefereed] Suppose that the deterministic levels of a cluster hold at
most 64 units, that every entry lies in [0, 65,535], that the sum of the units' largest entries is at most
256·IHMAX (so Θ ∈ [0, THETA_MAX] and Hs ∈ [0, 8,128]), that T_in and T_out are clamped to [TMIN, TMAX], and that each
margin lies in [RIN_MIN, RSENT) on the IN side or [ROUT_MIN, RSENT) on the OUT side, or equals RSENT. Then

(a) the kernel's IN test, sat(sat(Hs − TH) − RH) + ((Ls − TL − RL) >> 8) ≥ 0 with the last addition saturating and the
low word wrapping, holds exactly when Θ − Rin ≥ T_in, and its OUT test, the same with +RH, +RL and < 0, holds exactly
when Θ + Rout < T_out;

(b) a margin equal to RSENT never decides;

(c) a clamped threshold only withholds decisions.

*Proof.* (a) Hs ∈ [0, 8,128] and TH ∈ [−24,000, 24,000], so Hs − TH ∈ [−24,000, 32,128] is exact. RH lies in
[−15,810, 32,767] on the IN side and in [−24,000, 32,767] on the OUT side, so d = Hs − TH ∓ RH can leave i16 in either
direction, and the second operation saturates. Ls ≤ 64·255, so g = Ls − TL ∓ RL lies in [−510, 16,320] on the IN side
and in [−255, 16,575] on the OUT side, so g is exact in i16 and g >> 8 = ⌊g/256⌋ ∈ [−2, 64]. Lemma 1.20 gives
256d + g ≥ 0 ⟺ d + ⌊g/256⌋ ≥ 0 for the true d. If d was not saturated, the last saturating addition can clip
only a sum above 32,767, whose true value is positive, or below −32,768, whose true value is negative, so the sign is
kept. If d saturated high, the true d is at least 32,768, and both the computed word (at least 32,765)
and the true value 256d + g are positive. If d saturated low, the true d is at most −32,769, the computed word is at
most −32,704, and the true value is at most 256·(−32,769) + 16,575 < 0. (b) With R = RSENT,
Θ − RSENT ≤ THETA_MAX − RSENT = −6,291,264 < TMIN ≤ T_in, so the item is never IN, and Θ + RSENT ≥ RSENT > TMAX ≥ T_out,
so it is never OUT. (c) If T_in is lowered to TMAX, then Θ − Rin ≤ THETA_MAX − RIN_MIN = TMAX − 1, so the item is never
IN. If T_out is raised to TMIN, then Θ + Rout ≥ ROUT_MIN = TMIN, which is never below TMIN, so the item is never OUT.
Raising T_in to TMIN or lowering T_out to TMAX makes the test harder to pass. All of this needs Θ ∈ [0, THETA_MAX], which
holds because entries are clamped at 0 and the code asserts the sum of the units' largest entries. ∎

*Up to 127 units.* [sketch: gap named] The report's verdict is that the compare is exact for at most 127 units
("sound with the proof above for at most 127 units"), and its attack below found no mismatch at 127. The gap is that
the report writes the ranges of its proof for 64 units only. The editor's reading of them for U units, which is not in
the report and which no referee has read, is as follows. Ls ≤ 255U, so g lies in [−510, 255U] on the IN side and in
[−255, 255U + 255] on the OUT side; both lie in i16 for U ≤ 127, because 255·127 + 255 = 32,640, and then
g >> 8 ∈ [−2, 127]. The bounds on Hs and Θ come from the assertion on the units' largest entries and not from U. The
saturated-low step then reads −32,768 + 127 for the computed word and 256·(−32,769) + 255·127 + 255 for the true value,
both negative (Editor's query 20).

The comment in the code writes "Θ + Rout ≥ ROUT_MIN > TMIN"; it is "≥", which is enough because OUT is strict, and
this chapter's earlier text repeated the ">". The referee's attack (`k1_compare.py`: 4,000,000 random cases per setting, half on the decision boundary, with
thresholds and margins at their range ends, plus a sweep of every saturation corner) found 0 mismatches at 64 and at 127
units, and the sentinel and the clamps never decided [model]. **At 128 units the OUT compare is wrong.** The first case
found was Hs = 0, Ls = 32,639, T_out = −4,201,876, Rout = −4,234,514: the kernel says OUT and the exact compare says
not, because Ls + RL passes 32,767 and the low word's shift turns negative. At 160 units both sides are wrong
[model]. The shipped levels do not reach 128 units, but the code asserts the unit count on one path only (§1.8a,
defect 3).

The design's referee named two further gaps, both closed in the build. The design computed thresholds in f32 "with a
relative 2⁻²⁰", which is not a proof, because K_in is a difference of nearly equal terms divided by η, so its error is
absolute: (|t| + |ψ·h_fix| + |q·m| + |α·m·r|)·2⁻²²/(ησ) steps. The build computes them in f64 with an absolute and a
relative slack (Lemma 1.18a). That referee also noted that rounding (L_u(v) − b_u)/σ in f32 adds up to 2⁻⁸ of a step
to each unit's error, which the built tables' float term counts (the subtraction and the product with 1/σ, above). The
source left the range of the per-level centring constant unbounded, and the build has no such constant.

**Proposition 1.22** (centre tables as built; `reports/surge/integer-kernel.txt` §3 for the linearity,
`referee-kernel.md` §3d for the construction and its bound). [proved, unrefereed]

(i) *Linearity.* The weights of cluster c are w_c = w_q − α_c·w_m, with w_q per (query, group) and w_m = fl32(mv32·sig)
(`Cl2::wm`) the centre in the same coordinates, which are the same f32 weights that the path with own tables and verify
mode's f64 reference use. Tables are linear in the weights, so the real table of the combined path is
L^c_u(v) = L^q_u(v) − α_c·L^m_u(v), with L^q per (query, group, level) and L^m per cluster and independent of the query.
The estimate and the real half-width are therefore those of the path with own tables.

(ii) *The combine.* The combined tables (`combine_lut`, the default `gtab` path) compute, for each unit u and code e,
v_u(e) = fl32(Tq_u(e) − θ^m_u(e)·fl32(α·σ_m)) by one fused operation, with Tq_u the group's f32 table and θ^m_u the
stored centre table. μ_u is the exact minimum of the 16 computed values, θ_u(e) = rne((v_u(e) − μ_u)·fl(1/σ)) is
saturated to 16 bits, and the part's offset is Σμ_u − α·Σlo^m_u, rounded to f32, with lo^m_u the centre table's
offset. The error bound of a part, summed into e_ℓ, is

    err_q + |α|·err_m + σ·(n_p/2·(1 + 10⁻⁶) + Σ_u (4·2⁻²⁴·mag_u + 3·2⁻²⁴·65,536)) + 4·2⁻²⁴·|off| + 10⁻³⁰,

with err_q = 8·2⁻²⁴·Σ|w_q|·max|E| the group table's float error, err_m the stored centre table's own bound (its grid
step σ_m/2 per unit plus float terms), mag_u the code's bound on the unit's magnitude in steps, and off the part's
offset. With this e_ℓ, (1) holds.

(iii) The design's bound e_ℓ ≤ 3Uσ does not hold for the built combine.

*Proof.* (i) is linearity of the table in the weights, in real arithmetic on the f32 weights w_q − α·w_m. (ii) Each
term covers what it names. The FMA's rounding and the rounded factor α·σ_m need 0.5·2⁻²⁴·(|α|σ_m·θ^m_max + max|v|)/σ
steps against 4·2⁻²⁴·mag_u. The subtraction and the product with 1/σ need 1.5·2⁻²⁴·65,536 against 3·2⁻²⁴·65,536. The
offset's cast needs 0.5·2⁻²⁴·|off| against 4·2⁻²⁴·|off|. err_q and |α|·err_m carry the float error of the group's table
and the grid and float error of the centre's table, scaled by |α|. Saturation never acts, because
σ ≥ (largest combined range)·(1 + 1/512)/65,533, computed by `det_spans` from the same f32 values that `combine_lut`
recomputes. (iii) The centre table's grid enters as |α|·σ_m/2 per unit, in the centre table's step σ_m, which is
unrelated to the cluster's step σ. When the query is close to α times the centre in the group's coordinates, the
combined range, and with it σ, is small while |α|σ_m is not. ∎

The float constants are the referee's count by hand. The design's statement, an integer combine by `mulhi` with
0.5 + 0.5 + 2 fine steps per unit and so e_ℓ ≤ 3Uσ, describes a construction the build does not use, and that bound is
false for the built form. The loss is of tightness and not of soundness. In the referee's model (`k4_tables.py`, as
above), e_ℓ per unit with combined tables had a median of 0.6–0.84 fine steps and a maximum of 1.8 in the ordinary
weight regimes, against the design's 3, but a median of 344–367 and a maximum of 1,750 when w_q = α·w_m·(1 ± 10⁻³)
[model]. This is how the index builds each cluster's tables from the group's float tables and the stored centre tables
while keeping α (`SYSTEM.md` §2.2).

*Differences between the code and the design* (`referee-kernel.md` §§1–3, 6):

- *Signed B_ℓ.* The design's margin is ⌈(ψ·X + e_ℓ + |B_ℓ|)/σ⌉ on both sides, with B_ℓ a remainder. The built margins
  carry −B_ℓ on the IN side and +B_ℓ on the OUT side, with B_ℓ the whole sum. Theorem 1.19's argument covers it. The
  margins can then be negative, down to RIN_MIN and ROUT_MIN, which changes the saturation argument (Proposition 1.21′)
  but not the implication.
- *No centring constant.* The design adds ⌊B_ℓ/σ⌋ to the accumulators per level; the code adds nothing, and the
  accumulators are unsigned sums. This removes the design referee's third gap.
- *The dial on the OUT side only.* The built IN carries no dial, in K_in or in Rin, so every IN is certified at ψ = 1
  whatever the dial. The built OUT carries ψ_ℓ on η·X_ℓ(n) and ψ_max on h_fix. This matches the f32 path
  (`lb = est − hh`, `ub = est + hh·dial`) and `check_det`. The design scaled both sides by ψ, so its IN at ψ < 1 was not
  a certificate.
- *ψ_max on the fixed term.* One T_out serves every deterministic level, so the fixed term takes the largest dial. This
  only lowers T_out, so the integer OUT asks for slightly more than the f32 path's `ub < t` at a level whose dial is
  below the largest. It is conservative.
- *tol.* K_in carries t − tol, so IN certifies score ≥ t − tol, the left inclusion of the guarantee with d = tol. The
  design has no tol.
- *`out.iv`.* With it set, K_in = +∞ and T_in = TMAX, so nothing is taken in by the integer levels.
- *Thresholds* are computed in f64 with an absolute and a relative slack of 2⁻⁴⁶ (Lemma 1.18a), not in f32 with a
  relative 2⁻²⁰, and are clamped to [TMIN, TMAX].
- *Margins* are computed in f64, rounded by `up`, and replaced by the sentinel RSENT outside their ranges and for code
  15, in place of the design's R_ℓ ∈ [0, R_cap] with R_cap = 16,000.
- *Entries* are 16-bit integers in [0, 65,535], where the design had [0, 2^(h+8) − 1] with h = 7.
- *The compare* saturates in the design referee's order (Proposition 1.21′).
- *Centre tables* are combined in f32 by one fused operation, not in integers by `mulhi` (Proposition 1.22).
- *The unit count* is asserted at most 64 only on the path with own tables (`int_sigma`, `rust/src/v2/kern.rs`
  line 435), and not on the default path (§1.8a, defect 3).

*Checked, no error found* (`referee-kernel.md` §§2–6, in brief). The referee found no input on which an integer
decision contradicts the real certificate (A0). It found no error in:

- Theorem 1.19 as built, given (1), the thresholds, the margins and the compare;
- the compare for at most 127 units, the sentinel and the clamps (Proposition 1.21′, whose written proof covers 64
  units; the sketch after it covers 127);
- the f64 thresholds (Lemma 1.18a) and the f64 margins (Lemma 1.18b);
- the tables, own and combined: in its model inequality (1) was never exceeded, no entry was clamped, the sum of the
  largest entries never exceeded `qsum` or 256·IHMAX, and of 72,711 decisions placed near the boundary, 11,567 of them
  within a few steps of it, none was not implied by the real certificate [model];
- the linearity of the combined tables;
- the item scale η and the query weights, the latter within the condition stated after 1.10;
- ties: IN non-strict and OUT strict at every step;
- a zero residual: η is floored, so K/σ is huge and both thresholds clamp to values that only withhold, although the
  rows nearest their centre are the ones whose key is clamped (§1.8a, defect 1);
- empty and single-row clusters: `run_cluster` indexes `cands[0]`, so a caller must not call it with no candidates (a
  panic, not a wrong answer); with one candidate, the block's other 31 lanes are computed and masked out by
  `inm & act` and `outm & act`, and lanes past the candidate count rounded up to 8 keep stale thresholds and are masked
  the same way;
- norm code 15, and a NaN or out-of-range margin, which become RSENT on both sides and never decide at that level;
- widths that are not a multiple of the SIMD width: a part's units are a multiple of 4 (asserted in `Part::pack`), a
  block is always 32 padded slots, `int_thresholds` rounds the item count up to 8 (4 on AVX2) inside arrays padded to whole blocks
  plus one, the hand-over from f32 to integers at the first randomised level steps by 4 inside the same padding, `dot32`
  has a scalar tail, and `qv_exact_v` asserts n % 32 = 0.

What the referee could not settle without the binary is whether its transcriptions match the compiled kernels (the lane
orders of `order32`, `order16` and `look16`, and the AVX2 and AVX-512 `cmp2w`). It traced them by hand and found no
error. The crate's tests `two_word_compare_matches_i64`, `int_level_matches_scalar`, `int_thresholds_match_scalar`,
`combine_lut_within_bound` and `part_sums2_at_the_largest_part` in `rust/src/v2/kern.rs` settle it, once built for
AVX-512 and once for AVX2 without AVX-512 (`referee-kernel.md` §7 item 1).

**Full vectors** (`SYSTEM.md` §§1, 2.2). [sketch: gap named] Items still undecided after 256 bytes are scored from
their f16 vector in float32, with an exact integer product only when the float score lies within its rounding bound of
t. "Exact" means exact in f16 arithmetic on the stored vectors and the query rounded to f16, so answers can differ from
a float64 brute force only by items whose score lies within f16 rounding of t. The gap: the float32 rounding bound
(`full_err_qn` in `rust/src/v2/q.rs`) is stated in code, and no written source proves it.

Measured end to end [measured]: on a 50k index (200 item and 200 text queries, seven answer sizes, filtered visible
sets, k-NN, float64 oracle, `check.sh`), every step of `rust/REPORT_BOUNDS.md` gave 0 wrong answers, 0 deterministic and
0 randomised violations in about 44M interval checks, 0 of 10.6M integer decisions not implied, and exact k-NN on 400 of
400 queries. At 1M the index's answers were byte-identical to the plain f16 scan at every visible set (`SYSTEM.md` §3).

## 1.8a Open defects of the build

This section lists the defects that the two referees of the program's kernel track found in the code at commit
8c0b316f9 (`program/kernel/referee-range.md`, `program/kernel/referee-kernel.md`), and one that the partition
experiment found in the build's grouping (`program/partition/experiment/RESULTS.md`). The first seven are [model]:
they rest on the referees' reading of the code at commit 8c0b316f9 and, for defects 1, 2, 3 and 5, on float64 or
numpy transcriptions of it, and none was reproduced in a binary. In each transcription the referee rewrote the named
Rust functions in Python by reading the code, ran them in float64 on small synthetic indexes or on synthetic inputs to
one routine, and compared the decisions with exact scores in rational arithmetic; the binary's summation order and lane
order were not reproduced. Defects 4 and 6 come from reading the code alone, and defect 7 from an estimate with
standard rounding bounds. Defect 8 is [measured]. Verify mode cannot reach the first
two in the cut and the sorted range, because it compares answers with brute force but takes its thresholds from answer
sizes (`referee-range.md`, "Not settled without the binary or the data", item 2). The kernel referee names a verify
run on a purpose-built index that could confirm defect 1 in the certificate, though a zero there would not clear it
(entry 1 below). All eight were passed to the implementing
session on 21 November 2026. None is repaired at commit 8c0b316f9.

1. **The key clamp** (`referee-range.md` F1, F2, F6, F7; `referee-kernel.md` §3e (b), finding K3). The two referees
   found this defect independently.
   - *What the code does.* `rows_of` (`rust/src/v2/mod.rs` line 1589) stores round(clamp(x·m̂, −1, 1)·32767). A stored
     row has an f16-rounded norm, not norm 1, so x·m̂ can exceed 1 for a row lying along its centre, and the clamp then
     discards the excess. The key's error is then x·m̂ − 1, which exceeds DA as soon as x·m̂ > 1 + DA ≈ 1 + 1.536·10⁻⁵.
     Everything that assumes |ã − â| ≤ DA is then unsound for that row: `sorted_range2` and `key_interval2` (Lemma 1.5),
     the r_max, b_min and b_max of `cluster_info` and `build2` (Proposition 1.6, h_fix), the |α|·‖m‖·DA term of the
     levels' fixed half-width (1.10), and the m·r that the randomised levels and the f32 path read.
   - *Breaking input, cut and range* [model] (`range_attack.py`, `range_corner.py` part A). Over seeds 2, 3 and 5 as
     built, 872,256 (cluster, query, threshold) checks offered 81.8M row decisions and took 26.8M; 1,729 were wrong, all
     of them rows with |x·m̂| > 1 + DA. The constructed case: n = 16, two clusters with orthogonal centre directions as
     the anchors, and a cluster of six near-duplicates along its centre direction m̂, one of norm 1 + 1.197·10⁻⁴ with
     true key 1 + 1.197·10⁻⁴, stored as 32767. With q = unit(0.75·p̂ + 0.65·m̂) rounded to f16 and t equal to that row's
     exact score 0.655012466, the not-excluded key interval is [1.0000885, 1.0001616], wholly above 1 + DA + 10⁻⁹, so
     the stored key 1.0 falls outside it and the row is decided out although its score equals t.
   - *Breaking input, norm sort* [model] (`nucut_attack.py`). 748 wrong decisions and 979 exact scores outside the
     bounds of their norm code in 244,800 checks, all rows with |x·m̂| > 1 + DA.
   - *Breaking input, certificate* [model] (`k6_key.py`, 24,000 rows). For a cluster whose mean is one row (a single
     row, or exact duplicates), with the centre stored in int8 with one scale as the fit does, the key's error exceeds
     DA for 0% of rows at n = 768 (largest 1.5·10⁻⁵, just inside), 4.2% at n = 256, 16% at n = 100, 32% at n = 32, and
     44% for rows with a few large coordinates at any n. The largest errors are 5·10⁻⁵ to 3·10⁻⁴, against an allowance
     of 1.5·10⁻⁵·|α|‖m‖ + 4·10⁻⁶. The estimate is then low by α·‖m‖·(x·m̂ − 1), and the certificate does not contain
     the score. On the integer path such an item is very likely not decided at all, because its scale is tiny and its
     norm code is the escape, which may be why the deterministic counts never showed it.
   - *Exposure of the 1M index.* `rust/results/run1m.log` and `scale.log` print "x2 bound 4.882e-4", so
     √x₂ − 1 = 2.44·10⁻⁴, sixteen times DA. A row at the top of that norm range is clamped past DA when its angle to its
     centre direction is under about 21 mrad. The range referee estimates, without measuring, that the 8-bit centre of
     a cluster of duplicates sits about 8 mrad from its rows at n = 768.
   - *Count on built indexes* [measured, by the visit-order session, eight indexes, 21 November 2026]
     (`program/LOG-2026-11-21.md`, "After the first pass"). No row lies beyond 1 + DA on any of the eight indexes. The
     largest |x·m̂| is 0.996 (BioCLIP), and the largest row norm is 1 + 4.0·10⁻⁴ (SIFT). So no recorded result was
     touched by this defect, and its repair is preventive. The owner decided on 21 November 2026 to repair it now, by
     adding each cluster's largest clamp excess to its key allowance.
   - *Repairs proposed.* The range referee: assert |x·m̂| ≤ 1 + 10⁻⁷ at build, or widen the sorted range by
     DA + max(0, √x₂ − 1). In its model the widening gave 0 wrong decisions in 369,600 checks of seed 2, and rows decided
     fell from 10.73M to 10.57M, where √x₂ − 1 is near 10⁻³ [model]. The kernel referee: assert |x·m̂| ≤ 1 + 10⁻⁷ at
     build and scale the key's range, or add the cluster's largest clamp excess to DA per cluster.
   - *What would settle it on a binary.* Count at build, in `rows_of`, the rows with |x·m̂| > 1 + DA; or, on a built
     index, the rows with stored key ±32767 and their x·m̂ in f64. A unit test beside `e2e.rs` would build an index
     holding a row of norm 1.0002 along its centre and run the query of `range_corner.py` part A at the row's exact
     score, expecting the row in the answer. For the certificate, build a small index at n ≤ 256 from a corpus with
     groups of exact duplicates large enough to be clusters of their own, use those rows as item queries, and run
     verify mode (`--v2 --index <dir> --in-memory verify`), reading `cert_viol`, `viol_det`, `viol_rand` and the wrong
     in and out counts. A non-zero `cert_viol` or `viol_rand` on those rows confirms the defect; zero does not clear it,
     because the violation needs a threshold within about 10⁻⁴ of the row's score.

2. **The square root of a cancelling difference** (`referee-range.md` F3).
   - *What the code does.* ρ_⊥ = √max(‖q‖² − β², 0)·(1 + 10⁻¹²) + 10⁻¹² (`anc_of`, `rust/src/v2/q.rs` line 182), and
     likewise s1 = √max(‖q‖² − (q·m̂)², 0)·(1 + 10⁻¹²) + 10⁻¹² (line 705) and rp = √max(ρ_⊥² − rm², 0)·(1 + 10⁻¹²) +
     10⁻¹² (line 711), with rm the computed q_⊥·m̂; s1 and rp are the norms of q_m and (q_⊥)_m in Lemma 1.5a. Exactly,
     ‖q − βp̂‖² = ‖q‖² − β² + β²(‖p̂‖² − 1) + 2β(β − q·p̂), and the computed difference carries the rounding of ‖q‖², β²
     and β, an absolute error ε_r of order 10⁻¹⁶·‖q‖² (in the worst case about 2n·2⁻⁵³·‖q‖², and up to 2·10⁻¹²·β² for
     a loaded anchor of norm 1 ± 10⁻¹²). The square root turns ε_r into a deficit of ε_r/(2ρ_⊥), and of √ε_r ≈ 10⁻⁸
     when ρ_⊥ itself is of that size. The slack 10⁻¹² covers it only for ρ_⊥ ≳ 10⁻⁴, and the 10⁻⁹ on the threshold
     only for ρ_⊥ ≳ 10⁻⁷. Proposition 1.4 and Lemma 1.5a need ρ_⊥ ≥ ‖q_⊥‖ and the constants rounded outward, so they do
     not apply when the deficit is not covered.
   - *Breaking input* [model] (`range_corner.py` part B, `range_corner_c.py`, exact values in rationals). For a query
     equal to a centre's int8 vector (exact in f16), ρ_⊥ as coded was below the exact ‖q − βp̂‖ in 219 of 400 queries
     at n = 32 (largest deficit 1.83·10⁻⁸, above 10⁻⁹ in 101) and in 45 of 60 at n = 768 (largest 1.36·10⁻⁸, above
     10⁻⁹ in 13); s1 was short in 209 of 400 and 49 of 60. With that query and a top row whose p̂·x lies just under a
     point of the table's 2⁻¹⁵ grid, `support_pre` skips the cluster at t equal to the top row's exact score: in one
     printed case at n = 8, ρ_⊥ as coded is 1.0·10⁻¹² against an exact 1.726·10⁻⁸, and
     β·hi + ρ_⊥·√x₂ = 0.827366226830 < t − 10⁻⁹ = 0.827366231872 while the row scores exactly t. A matching row is
     skipped with its cluster, in 8 cases from 400 centres tried. The binary sums q·p̂ in another order, which changes
     which queries hit the defect and not whether the formula is safe. In the sorted range the same deficit in s1 and rp
     is in practice hidden by the 10⁻⁷ in DA, but nothing written says so, and the worst-case ε_r is larger than that
     margin.
   - *Repair proposed.* Add the rounding before the root: ρ_⊥ = √(max(‖q‖² − β², 0) + c·n·2⁻⁵³·‖q‖² + 2·10⁻¹²·β²)
     with c ≈ 4, and the same for s1 and rp, where for rp the term is 2|rm| times the error of rm. The referee puts the
     cost at under 10⁻⁶ of width at ρ_⊥ = 0 and nothing measurable elsewhere.
   - *What would settle it on a binary.* A unit test of `support_pre` with the query equal to a centre's int8 vector
     and a top row placed just under a grid point of the table, as in `range_corner_c.py`. Verify mode does not reach
     it.

3. **The missing unit-count guard** (`referee-kernel.md` §3a, finding K1).
   - *What the code does.* "At most 64 deterministic units" is asserted only in `int_sigma` (`rust/src/v2/kern.rs`
     line 435), the path without combined tables. The default plan has `gtab: true`, whose step is computed in
     `CQ2::enable_int` (`rust/src/v2/q.rs` lines 488–498) with no such assertion. The build accepts any deterministic
     schedule below the first randomised level (`rust/src/v2/mod.rs` lines 2345–2350), so a build with `--dlv` ending
     at 64 bytes or more, and `--rlv` starting above it, gives 128 or more units.
   - *Breaking input* [model] (`k1_compare.py`). At 128 units the OUT compare is wrong: Hs = 0, Ls = 32,639,
     T_out = −4,201,876, Rout = −4,234,514 gives OUT where the exact compare does not, because Ls + RL passes 32,767. At
     160 units both sides are wrong. With the shipped schedules (at most 32 bytes, 64 units) the compare is exact by
     Proposition 1.21′. The report puts the limit at 127 units, so that any schedule up to 63 bytes (126 units) is
     exact; the chapter carries that extension as a sketch after Proposition 1.21′.
   - *Repair proposed.* Assert at most 127 units in `enable_int` (or at most 64, to keep the comment's ranges), or
     refuse such a schedule at build.
   - *What would settle it on a binary.* Verify mode cannot show it in practice, because it needs a low-word sum above
     32,512. Extending the crate test `two_word_compare_matches_i64` to Ls ≤ 255·units for the largest unit count a
     build accepts would.

4. **The NaN threshold** (`referee-kernel.md` §3b, finding K2; edge, low).
   - *What the code does.* `int_thresholds` clamps as min(max(v, TMIN), TMAX) with x86 semantics
     (`rust/src/v2/kern.rs` line 526 on AVX2, line 556 on AVX-512), which sends a NaN to TMIN for both thresholds. TMIN
     is the safe end for T_out and the unsafe end for T_in. With finite inputs no NaN arises: η > 0, every product is
     finite, and K_in = +∞ under `out.iv` gives TMAX.
   - *Breaking input* [model] (the referee's reading of `int_thresholds` at commit 8c0b316f9; nothing was run). A
     threshold query with t = NaN gives T_in = TMIN, so every item whose margin is not the sentinel
     is taken IN, where the f32 path, with ordered compares, decides nothing. The refiner's NaN threshold runs in
     Interval mode, which never uses the integer path, so only a caller passing t = NaN to a threshold query reaches it.
     t = −∞ also gives T_in = TMIN, which is the right answer there. The finding is from reading the code; the report
     gives no count for it.
   - *Repair proposed.* Write the clamp of T_in as min(TMAX, max(TMIN, v)) with the operands ordered so that NaN goes to
     TMAX, or refuse a NaN threshold at the entry.
   - *What would settle it on a binary.* The report names no test. The editor's reading is that one threshold query
     with t = NaN on any index, through the integer path, would show it.

5. **The m·r allowance** (`referee-kernel.md` §3e (a), finding K4; a small gap).
   - *What the code does.* m·r = fl32(‖m‖₃₂·(ã) − ‖m‖₃₂²) from the key, with the f32 centre norm, and its allowance in
     h_fix is |α|·‖m‖·DA + 4·10⁻⁶ (`rust/src/v2/q.rs` line 574).
   - *Breaking input* [model] (`k6_key.py`, every key times 404 centre norms). The f32 evaluation differs from
     ‖m‖ã − ‖m‖² by up to 2.7·10⁻⁷·‖m‖, which is more than the 10⁻⁷ in DA. The rest must come out of the absolute
     4·10⁻⁶, which suffices while |α|·‖m‖ ≤ 23. Nothing bounds |α|, which is a least-squares coefficient in weighted
     coordinates, and for |α|·‖m‖ > 23 the allowance is short by 1.7·10⁻⁷·|α|‖m‖ − 4·10⁻⁶.
   - *Repair proposed.* DA = 0.5/32767 + 3·10⁻⁷.
   - *What would settle it on a binary.* Print the largest |α|·‖m‖ over opened clusters on the text queries. Above 23
     the float share of the m·r allowance is short.

6. **The table cache keyed by level only** (`referee-kernel.md` §4; latent).
   - *What the code does.* `cq.luts[l]` (`rust/src/v2/q.rs` lines 1369–1384) caches a cluster's tables by level only.
     Tables built in Interval mode, with a step per part, would be reused on the integer path with step σ if one `CQ2`
     with the integer path enabled were ever run in both modes.
   - *Breaking input* [model] (the referee's reading of the code at commit 8c0b316f9; nothing was run). None
     reachable: the three `Mode2::Thr` call sites open a fresh cluster setup, and the Interval
     call sites never enable the integer path. The finding is from reading the code.
   - *Repair proposed.* Assert that a cached deterministic table's step equals σ, which would make the reuse impossible.
   - *What would settle it on a binary.* No input reaches it; the assertion would keep it unreachable.

7. **e_inv does not bound the rounding of z̃** (`referee-range.md` F8).
   - *What the code does.* e_inv = (the computed Frobenius defect of V·X)·(1 + 10⁻⁶) + 10⁻¹² (`rust/src/v2/mod.rs`
     line 747), with X the computed inverse. It does not bound the rounding of z̃ = fl(X·r), or of the defect itself.
     Lemma 1.9 and Proposition 1.6 need e_inv ≥ ‖ε‖/‖r‖ for the stored z̃.
   - *Size* [model] (standard rounding bounds applied to the code as read, not the binary's summation order). The
     referee estimates the missing rounding at up to about 5·10⁻¹⁰ relative at
     n = 768 and condition 227, and typically far less. The +10⁻⁹ in `NuCut`'s E_α (q.rs line 654) and the 4·10⁻⁶ in
     the levels' fixed term (q.rs line 574) absorb it, but no source says so. The report gives no breaking input.
   - *Repair proposed.* The report proposes none, beyond placing the point with the gap named in 1.10. The editor's
     reading is that the absorbing argument should be written, or the rounding of z̃ added to e_inv.
   - *What would settle it on a binary.* The report names nothing. The editor's reading is that only the written
     argument settles it, and that a measurement on the binary would show the size of the rounding against the margins
     that absorb it.

8. **Groups with fewer rows than the dimension** (`program/partition/experiment/RESULTS.md`, item 1 notes, item 2 "Where
   the coding spread comes from", and "Limits of the evidence").
   - *What the code does.* `rust/tools/fit.py` sets G = min(64, K, N/(20n)), which gives 20n rows a group on average,
     and assigns clusters to groups by k-means on the unit centre directions, with one dual basis per group
     (`program/partition/NOTEBOOK.md`, "What is built, checked in the source"). `RESULTS.md` finds that this rule
     "does not hold a group above n rows" at 200,000 rows and G = 12. A group of fewer than n rows has a singular pooled
     covariance, which the fit floors (Editor's query 21).
   - *Breaking input* [measured] (200,000-row BioCLIP-2 sample, n = 768, K = 200, G = 12, `diag_groups.py`; costs under
     the floor's idealised level model, not the built code's bytes). With build seed 1 the grouping holds a group of 2
     cells and 617 fit rows, fewer than n, whose pooled covariance is singular and floored. The 208 held-out rows of that group need
     200 B at w* = 0.06, against a mean of 36 B, which by itself adds about 0.7 B to the mean. Build seed 2 made two
     groups of one cell each, with 1 and 3 fit rows. Over groups of at least 3n fit rows, the seed spread of the coding
     score falls from 1.08 B to about 0.6 B. The defect costs bytes and not soundness, because every bound of this
     chapter holds for any basis (§1.4, "The fit as built").
   - *Repair proposed.* `RESULTS.md` proposes none for the build. Its experiments B and C hold a floor of 3n = 2,304
     fit rows a group, B by refusing moves out of a smaller group and C by reverting them; the editor's reading is that
     such a floor in `fit.py` would be one repair.
   - *What would settle it on a binary.* The source names nothing. The editor's reading is that the group sizes of the
     1M index's fit show whether any group there has fewer than n rows.

A further precondition that the code does not assert, the largest decoded coordinate value behind e_fl, is recorded
after 1.10; the kernel referee did not list it as a finding.

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

- Propositions 1.13, 1.15 and 1.16 are short proofs that no referee has checked (`groups/REPORT.md` §5 says the same
  of Propositions 1–3). Chapter 2's Propositions 2.8 and 2.9 are the same results and share this status. The earlier,
  exact-key form of Lemma 1.5, which is Lemma 4.10 of chapter 4 in its annulus case, has now been checked by the
  referee of the cluster cut and found correct (§1.2); the restatements of §1.2 (Proposition 1.4, Lemmas 1.5a, 1.5b
  and 1.5, Proposition 1.6) are that referee's own and await a referee.
- The statements of §1.8 as built (Lemmas 1.18a and 1.18b, Theorem 1.19, Propositions 1.21′ and 1.22) are the kernel
  referee's own proofs, with float constants counted by hand, and await a referee. The extension of Proposition 1.21′
  from 64 to 127 units is a sketch whose ranges no source writes (Editor's query 20).
- The floating-point allowances of the f32 certificate (§1.3, 1.10: e_fl, `edet`, `fl` and the constant 4·10⁻⁶), the
  float32 rounding bound of the full-vector step and the two-norm bound of Proposition 1.12 need written proofs. The
  integer kernel's own allowances (thresholds, margins, tables) are now argued in §1.8, and parts of e_fl and h_fix
  are argued after 1.10 under conditions the code does not assert.
- The eight defects of §1.8a are open at commit 8c0b316f9.
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
   *Settled, 21 November 2026,* by `program/kernel/referee-range.md` (F2 and "Verdicts"). In rows can lie at either
   end, and so can out rows: in the referee's model the decided rows took every arrangement listed after Lemma 1.5,
   OuO, IuI and IuO among them [model]. The description in `SYSTEM.md` §2.2 is wrong in general, and Lemma 1.5 now
   says that it does not fix the ends. The referee's rule for the annulus bound alone is given after Lemma 1.5
   [sketch: gap named]. `SYSTEM.md` itself is unchanged.
7. The referee's absolute slack for the f32 thresholds includes |α·m·r|. The editor read `int_thresholds`
   (`rust/src/v2/kern.rs`): it adds |α·m·r| per item to the scale |t| + |h_fix| + |q·m| passed from `q.rs`, and it works
   in f64 with 2⁻⁴⁶ in place of 2⁻²², so the referee's term is covered. Two small points remain unwritten. The scale
   carries |h_fix| where the referee writes |ψ·h_fix|, which matters only for a dial above 1. The f64 slack is argued
   in no written source.
   *Settled, 21 November 2026,* by `program/kernel/referee-kernel.md` §3b. The f64 slack is now argued in writing
   (Lemma 1.18a, [proved, unrefereed], first order in the rounding unit; Editor's query 19). The scale's |h_fix| in
   place of |ψ·h_fix| is covered for any dial, because ψ_max·h_fix ≤ |t| + |c_out| + |q·m|.
8. Proposition 1.12 and the coordinate facts after Proposition 1.13 contain steps written out by the editor: the
   enclosing-ellipsoid argument for the former and the substitution checks for the latter. The sources assert both
   without these steps, and the tags were kept at the sources' level.
9. `reports/surge/integer-kernel.txt` names its main result "Theorem D", which collides with Theorem D (instance
   optimality) of the consolidated proofs; it is Theorem 1.19 here.
10. `DESIGN.md` §2 describes a 1-byte error norm per level and a level-0 norm byte; `SYSTEM.md` and `quant/REPORT.md`
    describe a 4-bit norm code per level and no level-0 norm. This chapter follows `SYSTEM.md`, which follows the code.
    *Note, 21 November 2026:* the kernel referee's reading of the code (`program/kernel/referee-kernel.md` §1 and §4:
    margins for norm codes n < 15, with code 15 the escape) agrees with `SYSTEM.md`. The point about `DESIGN.md`
    stands.
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
    *Settled, 21 November 2026,* by `program/kernel/referee-range.md` ("Verdicts" and "The statements and proofs as
    they should stand"). The referee found the statement and proof above correct, basis term included, and restated
    the proposition with two additions: the best of several values of α, and the claim that at most one of the two
    prefixes is non-empty. The restatement now stands in §1.2. It is the referee's own, so the tag stays
    [proved, unrefereed]. Two points remain against the code: the hypothesis e_inv ≥ ‖ε‖/‖r‖ is not established as a
    bound (§1.8a, defect 7), and every wrong decision in the referee's model of `NuCut` came from the key clamp
    (defect 1).
14. The built kernel (comment at `rust/src/v2/kern.rs` lines 386–404, and `q.rs` where `int_thresholds` is called)
    differs from the design of `reports/surge/integer-kernel.txt` §1.3 in two places. It uses the signed remainder,
    −B_ℓ in the IN margin and +B_ℓ in the OUT margin, which the proof of Theorem 1.19 covers. It also applies the dial
    to the OUT side only (K_in carries h_fix and R_in carries w_ℓ(n) + e_fl without ψ), so for ψ ≠ 1 the built IN test
    is that of ψ = 1. Neither change is in a written source, and neither was refereed.
    *Settled, 21 November 2026,* by `program/kernel/referee-kernel.md` §2 and §6. Both changes are sound by the
    theorem's own argument, and Theorem 1.19 is now stated as built: B_ℓ the signed full sum, no centring constant,
    the dial on the OUT side only (ψ_ℓ on η·X_ℓ(n) and ψ_max on h_fix), and t − tol on the IN side. Every IN is
    certified at ψ = 1 whatever the dial, and OUT at a dial below 1 is a statistical exclusion. The statement and proof
    are the kernel referee's, so they are tagged [proved, unrefereed]. The referee lists further differences, given
    after Proposition 1.22.
15. The earlier text of §1.2 said the support table stores lo_c and hi_c "in 2 bytes each". The referee of the cluster
    cut found them stored as f32 on a 2⁻¹⁵ grid, 4 bytes a value in memory (`referee-range.md` F8). Neither report says
    how they are stored on disk, and the chapter named no source for the 2-byte figure. `SYSTEM.md` §2.1 and the
    on-disk format should be checked, because the size of the table at 10⁹ rows is one of the open questions of §1.2.
16. Lemma 1.18b (ii) is argued in `referee-kernel.md` §3c only for B_ℓ < 0, which the report calls the usual case. For
    B_ℓ ≥ 0 the quotient (X + e − B)/σ can cancel as Rout's can, and the report gives no argument; its attack found Rin
    short of the exact quotient in 0 cases [model]. The editor's reading is that the slack of e_ℓ used for Rout would
    cover the same shortfall, but no source writes that step, so Theorem 1.19 takes the IN margin inequality as a
    hypothesis. The proof of (i) also needs e°_ℓ, e_ℓ less its two factors of 1 + 10⁻⁶, still to bound the error in
    (1). The report says only that σ·Rout ≥ B_ℓ + e°_ℓ + ψ_ℓX "still holds", and its §3d uses part of the same factors
    to cover the `as f32` rounding of each error bound, so the step is not written; Theorem 1.19 takes it as a
    hypothesis too.
17. The referee of the cluster cut read Lemmas 1.1 and 1.2, the earlier statement of Proposition 1.4 and the earlier,
    exact-key Lemma 1.5 (now part (d) of Lemma 1.5) and found no error in any of them ("Verdicts"). Their tags were not
    raised in this transcription, because the referee's attack was aimed at the code and its own restatements replace
    the earlier statements. Whether these statements, and Lemma 4.10 of chapter 4 (the annulus case), now enter as
    [proved, refereed] is for the PI to decide.
18. Other chapters still describe the kernel's allowances and departures as unargued: `06-open-problems.md` (the
    entries on the floating-point allowances, the f64 threshold slack, "the built kernel's departures from the refereed
    design" and the unrefereed Lemma 1.5) and chapter 4's Lemma 4.10. The kernel referee's report now argues the f64
    slack, the margins, the tables and the departures (§1.8, [proved, unrefereed]), and the range referee's report
    covers Lemma 1.5. Those entries should be revised by their chapters' editor; this transcription did not touch them.
19. Lemma 1.18a's proof is first order in the rounding unit ū, as `referee-kernel.md` §3b writes it ("(first
    order)"). The report claims a factor of about 14 to spare and writes no second-order terms. The float constants of
    the built tables (after Lemma 1.18) and of Proposition 1.22 are likewise the referee's count by hand. Proposition
    1.22 also uses a per-unit magnitude mag_u, which the report does not define beyond comparing 4·2⁻²⁴·mag_u with
    0.5·2⁻²⁴·(|α|σ_m·θ^m_max + max|v|)/σ; that it is a bound in steps is the editor's reading of that comparison.
20. Proposition 1.21′ is stated for at most 64 units, because `referee-kernel.md` §3a writes its proof with the ranges
    for 64 (g ∈ [−510, 16,320] or [−255, 16,575], g >> 8 ∈ [−2, 64], a saturated-low word at most −32,704). The same
    report's verdict, and its attack at 127 units [model], put the limit at 127 units, and the code's own comment
    (`rust/src/v2/kern.rs` lines 391 and 398–404) gives the 64-unit ranges. The ranges for U ≤ 127 after the
    proposition are the editor's and are tagged [sketch: gap named]. A referee should read them, or the report should
    write them.
21. Defect 8's account of the grouping rule is taken from the partition track's reading of `rust/tools/fit.py`
    (`program/partition/NOTEBOOK.md`) and from `RESULTS.md`. Neither gives line numbers for `fit.py` or
    `rust/tools/rules.py`, nor the size of the eigenvalue floor in the build's fit (the experiment's own model floors
    at 10⁻¹² of the largest, `EXPERIMENT-SPEC.md`), so this chapter gives neither.
