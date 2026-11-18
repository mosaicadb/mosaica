# One process for the approximate modes

A specification of a single rule for approximate k-NN and approximate threshold queries, to replace the tuned
constants of the cluster order (f′, H's α, z(k, n) and 0.3, the candidate multiple m, the sparse fallback, nprobe,
a charge for opening groups, a rule for expanding sub-clusters), with the conditions under which it is optimal.
It follows `theory.md` (this directory) and `../theory.md`.

Status labels: **[proved]** (proof here), **[checked]** (exhaustive or simulated check in `theory/policy_check.py`,
output `theory/policy_check_out.txt`), **[cited]** (not verified against the paper), **[open]** (to be settled by
the replay of section 9). Nothing here has been run on real data; the measurements quoted are from `theory.md`.

**Revised after the replay** (`policy_replay.md`). Sections 1 to 10 are kept as first written. Section 11 says
what the replay refuted and confirmed and replaces the parts that failed: a unit's value is its predicted **share**
of the query's answer, not a count; τ for k-NN is solved once and not updated; the tier S0 is dropped. Where
section 11 and an earlier section disagree, section 11 holds. Section 12 is the next replay.

The proposal I was given is kept in outline and corrected in five places:

1. **τ for k-NN is not the running k-th best** but the model's estimate of the final k-th best,
   the root of (found above t) + Σ_unprobed n_u P_u(t) = k. With the bare running k-th best nothing is ordered
   until k items are found, and every value is overstated until the end. [checked: 27 to 68% more cost at equal
   recall in simulation; the estimate matches the true τ]
2. **The setups are not random.** Every cluster's statistics are known before any group is opened, so a group's
   index is a deterministic maximum-density closed set, not a branching-bandit reservation value. It is computed
   exactly and its optimality is proved here directly. The reservation value is needed in one place only, the
   expansion of a cluster into sub-clusters, where I replace it by a rule with a proved bound.
3. **Each cluster has two ways of being read** in `knn2_fast` already (directly from full vectors, with no group;
   or on the index path, which needs the group). With alternatives there is no index order valid for every
   budget; there is an exact plan for a given price λ. So the process plans at λ and executes best-first.
4. **Lazy evaluation is exact but may save nothing.** `PLAN.md` measured that the valid bounds on H's spread
   prune no cluster unfiltered. The candidate multiple m does not disappear by laziness; it disappears if the
   spread comes from statistics cheap enough to compute for every cluster.
5. **One dial remains**, as it must: the price λ (or a recall target, from which λ follows).

---

## 1. The process

### 1.1 Objects

- **Units.** The fit's tree: group g → cluster c → optionally sub-cluster s (a label per row over unchanged rows,
  `theory.md` 5.1). Nothing in the layout, the bases, the codes or the exact paths is read or changed.
- **Value.** v_u(τ) = n_u^vis · P_u(τ): the expected number of answers among the unit's visible rows, with P from
  the exceedance table of section 4. τ = t for a threshold query; for k-NN the estimate τ̂ of 1.3.
- **Cost**, in the planner's microseconds (`COSTS` in `rust/src/v2/q.rs`; section 5):
  - a_g: opening group g (basis product), once;
  - a cluster read **directly**: c_D(c) = dir + full · n_c^vis, no group needed;
  - a cluster read on the **index path**: c_I(c) = b + item · n_c^vis, needs its group open;
  - a cluster **expanded**: e_c for scoring its sub-centres, then b once and item · n_s^vis (+ the label pass) per
    sub-cluster read, group needed.
- **Price.** λ, in answers per microsecond: the process stops when a further microsecond is expected to find fewer
  than λ answers.

### 1.2 The plan at price λ

For a leaf action with value v and cost w write (v − λw)₊ for its net value. Per cluster:

    D_c = (v_c − λ c_D(c))₊                                   read directly
    I_c = (v_c − λ c_I(c))₊                                   read whole on the index path
    X_c = ( Σ_s (v_s − λ w_s)₊ − λ (b + e_c) )₊               expanded (after expansion; before it, X̃_c of 1.4)
    J_c = max(I_c, X_c)                                        best use of the open group

Per group:

    V_g(λ) = Σ_{c ∈ g} D_c + ( Σ_{c ∈ g} (J_c − D_c)₊ − λ a_g )₊ .                              (1.1)

The plan: **open g if and only if Σ_c (J_c − D_c)₊ > λ a_g**; in an open group each cluster takes the larger of
J_c and D_c, in a closed one D_c; an action with net value 0 is not taken. Read: a group's basis is computed when
the index path saves, over the clusters worth reading at this price, more than the basis costs.

### 1.3 Execution

    score every centre: μ_c = q·m_c (as today); visible counts n_c^vis; features x_c (section 4)
    τ ← t                                   (threshold query)
    τ ← τ* : Σ_c n_c^vis P_c(τ*) = k        (k-NN; bisection, P monotone in τ)
    λ ← given, or λ(r) for a recall target r (1.5)
    loop:
        plan at (τ, λ) over the units not yet read            # (1.1); O(K)
        if the plan is empty: stop
        take the planned actions best-first by density, a group's setup placed just before its first use:
            expanded cluster: score sub-centres, read its sub-clusters with v_s > λ w_s in density order
            every item read is decided exactly, as today; answers go to the result
            k-NN only: at an epoch (rows read since the last one have doubled, or a group was opened)
                τ ← τ̂ : #{found ≥ τ̂} + Σ_{u unread} n_u^vis P_u(τ̂) = k ;  break to re-plan
    return the found items (k best, or all with score ≥ t)

For a threshold query the loop body runs once: the plan is the answer to "what to read", and the order within it
does not change recall or cost. For k-NN the order matters only through what the early reads teach about τ, which
is why it is best-first; decisions already executed stay (their cost is sunk and is dropped from later plans: an
open group has a_g = 0).

Within a plan, "density" of a block is (planned value)/(planned cost) of a group with its setup, or of a directly
read cluster; inside an open group, clusters by v/cost; inside an expanded cluster, sub-clusters by v_s/w_s. When
no cluster has the direct alternative this order is the index order of Theorem B and needs no plan: "always the
available action of largest index; stop when it is below λ".

### 1.4 Expanding a cluster

A cluster's sub-centres are scored only if it is expanded, so its sub-clusters' values are not known when the
plan is made. Two specifications, to be compared in the replay:

- **E1 (rule).** A cluster planned for the index path is expanded if and only if n_c^vis > e_c/item (it holds
  more visible rows than its sub-centres cost in row units), and its planning value is its whole value I_c.
- **E2 (table).** X̃_c = n_c^vis · Ψ_L(x_c; λ·item) − λ(b + e_c), with Ψ(x; θ) = E[ Σ_s (n_s/n_c)(P_s − θ)₊ | x ]
  calibrated like P (section 4.3): the expected net value per row of the sub-clusters worth reading, given the
  parent's features (since Σ_s (v_s − λw_s)₊ = Σ_s n_s(P_s − θ)₊ with θ = λ·item). Expand when X̃_c > I_c.

E1 never expands when expanding cannot pay and costs at most λ·e_c per expanded cluster when it should not have
(Proposition D). E2 is the reservation value of `theory.md` (3.2) made computable; it also lets a cluster be opened
for one good sub-cluster when its whole density is below λ, which E1 cannot.

### 1.5 The dial

- **A price λ**, global: by Theorem C the best expected recall for its expected cost over the query distribution.
  A table λ(r̄) from average recall to price is fitted at build on the pseudo-queries (one number per target).
- **A recall target r per query**: the smallest plan whose predicted recall reaches r. For a threshold query,
  predicted recall of a plan S is Σ_S v_u / Λ(t), Λ(t) = Σ_all v_u. For k-NN, read until
  #{found ≥ τ̂} ≥ r·k. λ(r) is found by bisection on the plan (plans are nested in λ, Lemma 1) and re-solved at
  epochs. A per-query target costs more on average than the global price at the same mean recall (Theorem C);
  it is a different contract.
- **A budget** (rows or microseconds): the largest plan within it; no price is needed and optimality is up to
  the last block (Theorem B).

nprobe is none of these and is not offered.

---

## 2. What is optimal, and under which conditions

Throughout: a set of actions is **closed** if every action's prerequisites are in it (a sub-cluster's cluster
setup, an index-path read's group). g(S) is the expected number of answers found by S, c(S) its cost.

**(C1) Additivity.** Recall is a sum over units of the answers they hold (true for top-k and for threshold
answers; `../theory.md`, Lemma 1). **(C2) Static values.** v_u = E[N_u | what is known when the plan is made].
**(C3) Calibration.** E[N_u | features] = n_u^vis P_u.

### 2.1 The plan [proved]

**Lemma 1.** For every λ ≥ 0, the plan of 1.2 maximises g(S) − λ c(S) over all closed sets of actions (each
cluster read at most once), with value Σ_g V_g(λ). If no cluster has the direct alternative, the plans are nested:
S(λ) ⊆ S(λ′) for λ′ < λ.

*Proof.* By (C1) the objective is a sum over groups, and the groups share no cost. Within a group, with the group
closed each cluster independently contributes D_c (its best closed-group use, or nothing); with it open each
contributes max(D_c, J_c), and J_c is the better of its two open-group uses, each a sum of independent positive
parts minus the cluster's own fixed costs. Opening is worth Σ max(D_c, J_c) − λa_g against Σ D_c: (1.1). Nesting:
without D, every net value (v − λw)₊ is non-increasing in λ, hence so are X_c, I_c, J_c and Σ_c J_c − λa_g; an
action taken at λ has positive net value at every smaller price and so have its prerequisites. ∎

With the direct alternative plans are not nested (a cluster read directly at a high price would be read on the
index path at a lower one, once its group is worth opening). That is why 1.3 plans at the price it will stop at.

**Theorem C (the price).** Let queries be drawn from any distribution and let S_λ(q) be the plan. Under (C1) to
(C3), among all rules that choose a closed set per query from the same information with expected cost at most
E_q c(S_λ(q)), the plan has the largest expected recall·(answers).

*Proof.* For every q and closed S, g(S) − λc(S) ≤ g(S_λ) − λc(S_λ) by Lemma 1. Take expectations:
E g(S) ≤ E g(S_λ) − λ(E c(S_λ) − E c(S)) ≤ E g(S_λ). ∎ (This is Theorem 3 of `../theory.md`, whose proof is the
same two lines with single clusters as the closed sets; I re-derived it and it is correct.)

### 2.2 The order [proved; the area statement cited and checked]

Without the direct alternative, define the index of an action given what is already open:
ν_j = sup{λ : V_j(λ) > 0}, V_j the value of the best closed set that j unlocks, j's own cost included. For a leaf
it is v/w; for a cluster setup, max over its sub-clusters sorted by density of Σv/(b + Σw); for a group, the
largest (value)/(a_g + setups + rows) over sets of its clusters' best prefixes. It is computed by Dinkelbach's
iteration λ ← g(S(λ))/c(S(λ)) from λ = 0, a few passes over the group.

**Theorem B.** The order "always the available action of largest index" has every plan S(λ) as a prefix, and
stopping when the largest available index is at most λ returns exactly S(λ). Hence at each such prefix it finds
the most expected answers of any closed set of no greater cost, and for a budget between two such prefixes it is
within the value of one block (the set added at one price) of the optimum.

*Proof.* An action available with index above λ is in S(λ): V_j(λ) > 0 given its prerequisites, which is
membership in the plan of Lemma 1. So everything done before the first time the largest available index is at
most λ lies in S(λ). Conversely a member of S(λ) not yet done has all its ancestors in S(λ) (closed), the first
of them not done is available, and its index exceeds λ; so at that time nothing of S(λ) is left. For a budget
B = c(S(λ)): any closed S with c(S) ≤ B has g(S) ≤ g(S_λ) − λ(B − c(S)) ≤ g(S_λ). Between two plans, the
concave envelope through the plans' (cost, value) points bounds every closed set, and the order is short of it by
at most the block's value. ∎

**Area.** The same order minimises Σ (value × completion time), the area above the recall-against-cost curve:
this is single-machine scheduling under tree precedence constraints, where the rule "available job of largest
maximum-ratio subtree" is optimal [cited: Horn 1972; Sidney 1975]. [checked: on 200 random trees of 14 jobs the
order equals the optimum of an exhaustive dynamic programme to 4·10⁻¹⁶, and S(λ) is a prefix at every price
tested.]

**How the two setups interact.** A group's index charges a_g once against everything worth reading in it, and
each cluster's b once against its rows. With many clusters to a group (350 at 1B) and large clusters, a_g and b
vanish against the rows and ν_g tends to the best cluster's density: the charge fades by itself with N. At 1M,
a_g is about 1.4 cluster reads (93 µs against 65 for 2,000 rows at d = 0.3 in `COSTS`), so a second cluster in an open group is preferred to an equal
one elsewhere by that margin: the π of `theory.md` 4.3 with its value derived. Once a group is open its clusters
compete at their own indices with every other open cluster and every closed group.

### 2.3 Sub-cluster expansion [proved bound]

**Proposition D.** (i) If n_c^vis·item ≤ e_c, expanding never beats reading the cluster whole. (ii) The rule E1
loses at most λ·e_c of objective per cluster it expands, against the best expansion decision.

*Proof.* Expanding and reading the sub-clusters with P_s > θ (θ = λ·item) against reading all: the difference in
objective is Σ_s n_s(θ − P_s)₊ − λe_c ≤ θn_c − λe_c = λ(item·n_c − e_c) (`theory.md` 3.2). (i) follows. (ii): the only error
of E1 where it expands is the case in which the left side is negative, and it is at least −λe_c. ∎

What E1 gives up is on the other side: a cluster whose whole value does not justify opening it, holding a
sub-cluster that does. That is hidden information, and the exact treatment is the reservation value of
`theory.md` (3.2); E2 is that value with the expectation tabulated. Under the assumption that expanding one
cluster does not change another's values, the plan with X̃_c is optimal for the price (`theory.md`, Theorem 2;
proved there directly). I cited Weiss (1988) and Bertsimas and Niño-Mora (1996) there for the corresponding
statement about the order (branching bandits). **I have not verified those papers and nothing in this
specification rests on them**: the setups here reveal nothing, so Lemma 1 and Theorem B cover them, and expansion
is covered by Proposition D and the Lagrangian theorem. The earlier exhaustive check (index policy optimal on 300
small stochastic instances) stands as evidence for the cited statement, no more.

### 2.4 Calibration error [proved; checked]

**Proposition E.** Let the process use values v̂ with |ln v̂_u − ln v_u| ≤ δ for every leaf. Then its plan Ŝ at any
price satisfies g(Ŝ) ≥ e^(−2δ)·g(S) for every closed S with c(S) ≤ c(Ŝ).

*Proof.* ĝ(Ŝ) − λc(Ŝ) ≥ ĝ(S) − λc(S) (Lemma 1 for v̂), g ≥ e^(−δ)ĝ and ĝ ≥ e^(−δ)g on every set. So
g(Ŝ) ≥ e^(−δ)ĝ(Ŝ) ≥ e^(−δ)[e^(−δ)g(S) + λ(c(Ŝ) − c(S))] ≥ e^(−2δ)g(S). ∎ [checked: smallest ratio 0.985 and 0.915
at δ = 0.3 and 0.7 on exhaustive small trees; the bound is far from tight when errors are independent]

This is Theorem 12 of `../theory.md` with setup costs. Two refinements matter in practice.

- **A common factor is harmless for a budget, harmful for a price.** If v̂_u = κ_q v_u for all units of a query,
  the order is unchanged, but the plan at λ is the true plan at λ/κ_q: the query is under- or over-served. The
  price needs calibration **across queries**, in particular between in-distribution and shifted queries; a budget
  or a per-query target read off found items (1.5, k-NN) does not.
- **Costs are a second calibration.** A cost wrong by a factor moves the plan like a value wrong by the inverse.
  Only ratios of costs matter for the plan; their scale only sets the unit of λ.

### 2.5 k-NN: τ moves [bounded; checked in simulation]

With τ known the problem is the static one above. It is not known; three facts.

- **No policy beats the plan that is told τ** (`../theory.md`, Theorem 10: each unit is probed before any of its
  scores is seen, so its value is at most n_u F̄_u(τ_{−u})). The plan at the true τ is the benchmark.
- **The estimate τ̂ of 1.3 is the model's expectation of that τ**: its defining equation says the expected number
  of rows above τ̂, found and not, is k. Before any read it is the plug-in τ* of `../theory.md` 3.1, whose error is
  about σ̄/(ζ̄√k) in-model. An error Δ in τ changes ln P_u by about Δ·h_u, h_u the unit's hazard rate at τ; by
  Proposition E the loss is at most a factor exp(−2Δ·max h) and, for a budget, only the spread of h_u across the
  margin counts.
- **The process is not optimal among adaptive policies** (the two-cluster example of `../theory.md` 4.1 applies);
  it is the certainty-equivalent policy: optimal for the static problem at the current estimate, re-solved at
  epochs. In simulation (400 clusters in 40 groups, k = 10, calibrated Gaussian model, 200 queries, four prices):
  with the true τ, with τ* never updated, and with τ̂ updated after every read, recall and cost agree to 0.008 and
  3%; with the bare running k-th best the cost is 27 to 68% higher at the same recall, because the first reads
  are unordered and every value is overstated. [checked] So in-model the loss from not knowing τ is second
  order, and updating τ̂ is protection against a wrong table, not a source of gain.

τ̂ can fall as well as rise (fewer answers found than expected), which is why re-planning is done at epochs and
not by patching stale keys.

### 2.6 Lazy evaluation [proved; checked]

Suppose an action's exact index needs work (H's spread for a cluster) and an upper bound is cheap.

**Proposition F.** Keep a heap ordered by (key descending, id ascending) holding each available action with either
its exact index (fresh) or an upper bound of it (stale). Pop; if stale, compute the exact index and push it back
fresh; if fresh, take the action. If every stale key is at least the exact index, the sequence of actions taken is
exactly that of the eager order with ties broken by id.

*Proof.* When a fresh entry i is popped, its key equals its exact index and is at least every other key, hence at
least every other exact index: i is an eager choice. If another action j has the same exact index and a smaller
id, its key is at least that index, so j was popped before i, refreshed to the same key, and popped again before
i. ∎ [checked: 300 trees with bounds up to 3 times the index, 0 sequences differ]

What makes a bound valid here: P is monotone in the spread, **increasing when τ > μ_u and decreasing when τ < μ_u**,
so the bound uses the largest spread on one side and the smallest on the other; a group's index is monotone in
its clusters' values, so the group's bound is its index computed from their bounds, and its exact index needs
exact values only for the clusters whose bound exceeds it. Within an epoch τ and λ are fixed, so keys do not go
stale by themselves; at an epoch all keys are rebuilt.

Exactness is not saving. Everything whose bound exceeds the stopping price is evaluated; with the rank-0 bounds
s²|q|² ≤ σ² ≤ λ₁|q|² `PLAN.md` found 499 of 500 clusters evaluated. Laziness pays only with the cascade bounds of
`PLAN.md` (item 3) or when the first-stage spread is already close.

---

## 3. Which knobs disappear

| knob today | in the process | status |
|---|---|---|
| nprobe | the price λ, a recall target or a budget (1.5) | replaced by the one dial that must exist |
| f′ = 0.4 | the fitted slope of the table in log rad (β) and its shape g | gone as a constant; fitted at build per index |
| H's α, the 0.3 | the regression of the table on the spread statistics (shrinkage is what a fitted slope does) | gone as constants; fitted |
| z(k, n_c) | nothing: k enters through τ, n through n^vis·P | gone |
| `H_MIN_ROWS_PER_K` | nothing: visible counts enter the value; the artefact z(k, n^vis) ≈ 0 is gone | gone |
| the group charge π (`theory.md` 4.3) | a_g in (1.1) | gone; derived from `COSTS` |
| direct or index path per cluster (the 129-row cut in `knn2_fast`) | D_c against J_c in (1.1), now aware of the group's setup | subsumed |
| the sub-cluster expansion rule | E1 (no constant) or E2 (a second table) | gone as a knob |
| m, the candidate multiple (`H_M`) | lazy evaluation, exact (Prop. F) | **survives unless** the bounds are tight or the spread is cheap for every cluster (4.2, tiers S0 and S1); otherwise it remains a cost-accuracy choice |
| the dial on code levels (eps) | not addressed: it trades recall inside a read unit | survives |

What appears instead, and should be counted honestly: the cost constants (they exist: `COSTS`; the plan needs
their ratios right at other n and on other machines); the calibration design (which pseudo-queries, which shifts:
4.3), which is a modelling choice with the weight f′ used to carry; the epoch schedule for τ̂ (it affects cost of
the process, not correctness; the simulation shows no measurable effect of never updating in-model).

---

## 4. The exceedance table

### 4.1 What it must satisfy

1. **Calibration in expectation** (C3): for every cell of the features, the mean over (query, unit) pairs of
   N_u/n_u equals P. Then v_u is the Bayes value for the information the features carry, and the plan is optimal
   for that information; richer features can only help (Theorem 1 of `../theory.md` applies to any calibrated
   estimate). Nothing requires the model to be "true".
2. **Invariance**: the features are unchanged under q ↦ a·q + v, v a direction in which the corpus does not vary
   (`../theory.md`, Theorem 11). ζ_u = (τ − μ_u)/s_u with any spread statistic s_u that scales with a, and
   rad_u/median rad, are invariant.
3. **Monotone in τ** (non-increasing), for the bisections and for Proposition F's bounds.
4. **Calibration across queries**, at least between plain and shifted pseudo-queries (2.4).
5. **Range**: P from about 10⁻⁷ to 0.5. Small P for unfiltered k-NN at large N; large P for threshold queries with
   large answers and for k-NN under sparse visible sets, where τ is low.
6. **Thinning**: under a visible set that is a random sample, E[N_u^vis] = n_u^vis·P_u exactly, with the same
   table; τ̂ falls by itself. Under a correlated set the visible rows are not a sample and (C3) fails: see 7.

### 4.2 Form and features

    ln P_u(τ) = g_L(ζ_u) + β_L · ln(rad_u / median rad) ,      ζ_u = (τ − μ_u)/s_u(q) ,

g_L a non-increasing piecewise-linear function on a fixed grid of ζ (65 values in README finding 8), one (g, β) per
level L (cluster, sub-cluster): "one table" is one form with a row per level, since the tail of a 125-row unit
is not that of a 2,000-row one. Three tiers for the spread s_u, by cost:

- **S0**: s_u = σ(q), the standard deviation of μ over clusters (free). The table then carries the whole
  dependence on rad; this is `spread` with its constant fitted.
- **S1**: s_u² = (rad_u²/tr Σ_g)·qᵀΣ̂_g q, Σ̂_g the group's residual covariance to rank r plus an isotropic rest
  (`theory.md` 5.3 b): 64·r·d bytes and multiply-adds, for every cluster, no laziness needed.
- **S2**: H's per-cluster rank-r spread, evaluated lazily (2.6) or over candidates.

The fit absorbs α, 0.3 and f′: with several spread statistics, s_u² = a·ŝ² + b·σ(q)²rad_u² + c₀σ(q)² fitted by
regression before the table (`../theory.md` section 9), then one g.

### 4.3 Calibration at build

    pseudo-queries: corpus rows not used as their own answers (the row itself is removed from its unit's counts),
        each used plain and shifted: q′ = normalise(a·q + v), a ∈ {1, 1/2, 1/4}, v along the corpus mean and along
        the least-variance directions of the centres (as scripts/orders5.py builds them)
    thresholds: for each pseudo-query, τ at answer fractions 10⁻⁶ … 10⁻¹ of N (several per query)
    for each (query, τ, unit): features x_u, count N_u(τ) of the unit's rows above τ (from a row sample of the unit
        when it is large), exposure n_u
    fit g_L, β_L by Poisson regression of N_u on exposure n_u·exp(g(ζ) + β ln rad), g constrained non-increasing
    fit Ψ_L (E2 only) the same way, with response Σ_s (n_s/n_c)(P_s(τ) − θ)₊ on a grid of θ
    fit λ(r̄) for average recall targets by running the process on held-out pseudo-queries

All of it is offline, on the corpus alone (the owner's rule), and is redone at compaction. Its cost is a pass of a
few thousand pseudo-queries over a row sample.

### 4.4 Checks to run offline before believing it

1. **Reliability**: bin (query, unit) pairs by predicted P; plot observed N/n against predicted, separately for
   plain and shifted pseudo-queries, for each level, for small and large units, for rad terciles, and for the
   real queries. Report the largest |ln(observed/predicted)| over bins holding 95% of the answers: this is the δ
   of Proposition E.
2. **Counts**: Σ_u n_u P_u(τ_true) against k (or the true answer size), per query: the per-query factor κ_q of
   2.4. Its spread across query types is what a global price suffers from.
3. **τ**: τ* against the true k-th best, in units of the hazard: Δ·h.
4. **Residual covariates**: after the fit, does N/(nP) still vary with n_u, with the group, with the visible share?
   Whatever does should be a feature or a level.
5. **Recall targets**: achieved against requested, by query type, with the share of queries under target.
6. **Thinning**: repeat 1 and 2 under random visible sets of 1% and 25% (should hold as they are) and under
   correlated ones (will not: quantify).

---

## 5. Cost model

From `COSTS` (µs, one thread, BioCLIP-2 at n = 768): `grp_us` 93 (a_g); index path per cluster
`cl_us0 + cl_us1·d` (b) and per candidate `item_us0 + item_us1·d` (item), d the undecided fraction (1 for the
approximate mode's worst case; the replay should take the measured mean); `dir_us` 1.08 and `full_us` 0.19 per
visible row (direct). For other n: a_g ∝ n², full ∝ n, the item terms about constant in n (they read codes). To
be timed before the Rust version, because the plan depends on them: the label pass and the per-selected-row cost
of an index-path read restricted to sub-clusters (`theory.md` 4.2; the planner's `R_KAPPA` says the index path
has a cost per cluster row, selected or not), and e_c (16 int8 dot products, about 0.35 µs at d = 512).

Only ratios enter the plan. A replay can therefore report "modelled cost" with these constants and also rows,
clusters and groups, so that conclusions can be re-priced.

---

## 6. What the process itself costs

Per query, beyond the K·d of the centre scores that every order pays:

- features and values: K table look-ups (S0), plus 64·r·d multiply-adds (S1) or H's spreads (S2);
- τ*: a monotone root over K terms, a few secant steps (O(K) each); the README's unoptimised 40 bisections took
  79 µs at K = 500, and `PLAN.md` item 5 targets under 10;
- the plan: sort each group's clusters by density once (O(K log(K/G))), then (1.1) is one pass; a recall target
  adds a bisection over λ (about 20 passes of O(K), or one pass over the sorted breakpoints);
- epochs (k-NN): one τ̂ root and one re-plan each; with doubling, about log₂(rows read / first unit) of them, 5 to
  10;
- per expanded cluster: its sub-centres (16·d) and 16 look-ups.

So O(K) arithmetic with a small constant and no dependence on the number of visible rows: visible sets enter only
through the per-cluster counts the index already has (and, for an expanded cluster, a popcount of label mask ∧
visible bitmap). At K = 500 this is of the order of the 11 µs centre ranking; at K = 16,000 to 22,000 it is the
same multiple of it, and the remedy of `theory.md` 2.4 (a middle level of centres) applies to both alike.

**State.** Per cluster: the int8 centre, rad, the row count, the group id (all exist). Per level: one table
(65 values + β; with E2 a second of 65 × 16). Per group for S1: r directions and r + 1 variances. Per sub-cluster,
if used: centre, rad, count; per row a label. The cost constants. Nothing else; in particular no f, no fitted
order constants, no per-cluster directions unless S2 is chosen.

---

## 7. The two constraints

**Dual-basis checking.** The process decides which rows are handed to the existing readers (direct scoring, or
the index path with its certified intervals and the dial) and in which order groups are set up. It reads
centres, radii, counts and its own tables; it writes nothing and moves nothing. Every returned item is decided by
the existing code. Its one new demand on the index path is a row mask for sub-clusters, which is how a visible
set is already passed.

**Visible sets.**
- Random: exact by thinning (4.1, point 6). The behaviour that needed `H_MIN_ROWS_PER_K` falls out: with few
  visible rows a cluster's value falls with n^vis, its direct read becomes cheaper than b plus a share of a_g, the
  group is not opened (1.1), and E1 forbids expansion. As the set thins the plan tends to "score the visible rows
  of the best clusters directly", and to the planner's scan when even K centre scores are not worth it (that
  last switch stays with the planner: it is a comparison of this process's total modelled cost with a scan's).
- Correlated: cost is still right (counts are exact; clusters with no visible row have value 0 and are never
  touched). Value is not: P assumes the visible rows are a sample of the unit, and `theory.md` 7.1 measured that
  every centre-based order then reads 35 to 48% of a 1% visible set. The process inherits this. What it adds is
  a symptom it can see: for k-NN, answers found against answers predicted in the units read. A target read off
  found items (1.5) keeps its meaning; a global price does not. **[open]** whether a per-query correction factor
  from that ratio (v ← κ̂·v for unread units) restores the price; to be tested in the replay, not assumed.

---

## 8. Do the exact paths fit the same frame?

Formally yes, usefully no.

- Set P_u(τ) = 0 exactly where the cluster cut certifies that the unit holds no item above τ, positive elsewhere,
  and let λ → 0: the plan becomes "read every unit not certified out, in the cheaper of its two ways, opening
  every group that pays", which is the exact threshold query's set and the planner's per-cluster choice.
  Theorem C then says nothing, since the set is forced.
- For exact k-NN the set is forced only at the end; the order decides how fast τ rises and hence how much is read.
  Best-first by density is the natural seed order (the existing seed takes the clusters with the best bound), but
  the objective is the cost to certify, not recall per cost, and the frame's theorems are not about it.
- The approximate process does not converge to the exact one as r → 1 unless the table's support is the
  certificate's; with a fitted table it reads everything with P above a tiny level, not everything uncertified.

So keep them separate: the exact paths need no table and must not depend on one. Share the unit tree, the cost
model and the per-cluster direct-or-index choice (which (1.1) states in a group-aware form the exact planner
could also use). The hand-over is one-way: when the predicted cost of the approximate plan at the requested
recall exceeds the exact path's modelled cost (large answers, `theory.md` 6.2; high targets), run the exact path.

---

## 9. The replay that would test it

In the README's replay harness (exact search inside a read unit; whole units; the three suite datasets, plus LAION
with held-out image queries as the in-distribution control), with modelled cost from section 5 and rows, clusters
and groups reported beside it.

**Arms.**

| arm | order | stop |
|---|---|---|
| T1 | `spread`, f′ = 0.4 (today's default) | nprobe |
| T2 | H r16 over m = 4 candidates (`h16x4`) | nprobe |
| T3 | T1 with the group penalty π = 0.25·σ(q) (`theory.md` 4.3) | nprobe |
| P0 | the process, spread tier S0, clusters only | price λ |
| P1 | the process, S1 (group covariance, r = 16) | price λ |
| P2 | the process, S2 (H r16 spread as a feature; lazy with the rank-0 bounds, reporting how many are evaluated) | price λ |
| P0s, P1s | P0, P1 with 16 sub-clusters per cluster, expansion E1 and, separately, E2 | price λ |
| ceilings | the plan with the true per-unit counts as values (clairvoyant), and with the true τ | price λ |

Ablations of the process, each against P1: a_g = 0 (isolates the group term); no direct alternative; τ never
updated, updated at doubling epochs, true τ; a per-query recall target and a budget in place of λ.

**Conditions.** k = 10 and 100; threshold answers of 0.01%, 0.1%, 1%, 10%; unfiltered, random visible sets of
1%, 5%, 25%, correlated sets of 1% and 5% (the rows nearest a corpus row, as `theory/hier.py` section F, and a
second kind: a contiguous range of ids).

**Calibration.** Per index, on its corpus only, as 4.3; real queries are never used. Report the checks of 4.4
before any recall number.

**Measures.** Mean recall against mean modelled cost, by sweeping λ (process) or nprobe (tuned arms); cost at
recall 0.8, 0.9, 0.95 by interpolation; the same against rows; clusters and groups opened; units evaluated
exactly against units available (laziness); epochs; for targets, achieved against requested recall and the share
of queries below.

**What would confirm it.** With no constant set by hand: P0 at least as good as T1, and P1 at least as good as
T2 and T3, in modelled cost at each recall, on every dataset and condition, within the noise (about 5% with 100
queries; use the 1,000-query sets where they exist). The a_g = 0 ablation should give back T1's group counts.
Under random 1% the process should need no fallback and match `spread`. Thresholds: P0 should match the best
per-size f′ of `theory.md` 6.2 at every answer size.

**What would refute it or send it back.** A table fitted on pseudo-queries that is miscalibrated on real text
queries by more than about e^0.5 in the counts check (then the global price fails for text while the order may
still be fine: fall back to per-query targets); P0 worse than `spread` at fixed rows (the fitted β worse than
0.4: the README's finding 10, that f′ estimated from the corpus came out 2 to 3 times too low, is a warning
that this can happen); correlated sets, where I expect the price to under-deliver and the open question of 7
to be decided.

---

## 10. Proved, checked, open

Proved here: Lemma 1, Theorems B and C, Propositions D, E, F. Checked by `policy_check.py`: Theorem B and the area
optimality of the index order (exhaustive), Proposition F, Proposition E, the k-NN statements of 2.5 (simulation,
in-model only). Cited and not verified: Horn, Sidney (area optimality; supported by the exhaustive check), Weiss,
Bertsimas and Niño-Mora (not relied on). Open: whether a table calibrated on corpus pseudo-queries is calibrated on
real out-of-distribution queries well enough for a global price; E1 against E2; the cost constants for
sub-cluster reads; the correction under correlated visible sets; the loss of the certainty-equivalent policy for
k-NN outside the model (wrong tails), which the true-τ ceiling in the replay measures.

---

## 11. Revision after the replay

The replay (`policy_replay.md`, with `replay/results/policy_tables.md` and `calib_summary.md`; three datasets, 100
real queries each, counts measured, microseconds modelled) is the evidence for this section. Its numbers are quoted
as it reports them. The synthetic check added here is `theory/policy_check2.py` (output `policy_check2_out.txt`).

### 11.1 What it refuted and what it confirmed

| in sections 1 to 10 | replay | consequence |
|---|---|---|
| Tier S0 (s = σ(q)) is a usable table (4.2) | refuted: δ 0.58 to 1.17 on real queries, plain and shifted pseudo-queries disagree, its order is worse than `spread` on two datasets | **S0 is dropped.** The cheapest tier is S1 (the group's covariance). `spread`'s f′ does not become a fitted β with σ(q) as the only scale |
| A value is a count, n·P(τ) (1.1), with one global price on counts (Theorem C) | refuted as specified: the table is right in the pool (S1, S2: δ 0.05 to 0.49) and wrong per query by a factor κ_q whose 10 to 90% range at k = 10 is e^−2.5 to e^+1.6, for in-distribution queries too | **A value is a share** (11.2, 11.3) |
| τ̂ re-solved at epochs (1.3, 2.5) is "protection against a wrong table" | refuted: true τ worst, epochs in between, τ\* once best, on all three | **τ\* once.** 2.5's last sentence is wrong: the re-solve is what lets a wrong table steer the stop (11.2) |
| The fitted table replaces f′, α, z, 0.3 in the order (section 3) | confirmed where the spread statistic is the same: O1 level with `spread`, O2 level with H r16, at a fixed probe count, no constant set by hand | stands |
| The price beats a probe count (Theorem C) | confirmed once values are shares: P1\*, P2\* at 0.52 to 0.92 of `spread`'s modelled cost at recall 0.9 on every dataset and condition run, P2\* at or ahead of H r16 nearly everywhere | stands, restated for shares (Theorem C′) |
| The group term a_g (1.2) | works and matters little at the measured group cost (1% of cost on LAION) | stands; small at these n |
| E1 for sub-clusters (1.4, Proposition D) | confirmed: expands nothing under random 1%, where the tuned two-level arms lose 20 to 120%; with one plan the sub-cluster arms are the best measured unfiltered | stands; E2 not yet run |
| Random visible sets need no fallback (section 7) | confirmed for cost (P2\* 0.77 to 0.86 of `spread` at 1%); the table's counts are off under thinning on yahoo and LAION (δ 0.4 to 1.3) | explained by shares (11.3): a factor common to a query cancels |
| Correlated sets: "a global price does not keep its meaning" (section 7) | the counts are off (δ up to 1.5); recall per cost is not worse than the tuned orders' (0.57 to 0.92) | my warning was about counts; with shares it does not arise in the same way. The weakness of centre-based orders under such sets (`theory.md` 7.1) is unchanged |
| Thresholds: one plan at τ = t on counts | P2 level or better; P1 fails on LAION at answers of 0.01% (1.86) | values must be shares there too (11.3) |
| Laziness, k = 100, E2, recall targets, the image control | not run | section 12 |

### 11.2 Why τ\* once wins, and in what sense it is optimal

**The fact that was missing.** The answer of a k-NN query has exactly k rows: Σ_u N_u = k, whatever the query.
An estimate of the counts that does not sum to k is known to be wrong before anything is read, and by how much.
Recall is Σ_read N_u / k: a sum of shares. What the plan needs from the table is each unit's share of the answer;
the level is given.

**The error model the replay found** (M-κ): E[N_u | features, q] = κ_q · n_u P_u(τ_q) for all units u of a query,
τ_q the true k-th score, κ_q a factor per query with a wide spread (ln κ_q of the order of ±2 at k = 10) and
mean about 1 in the pool. Then Σ_u n_u P_u(τ_q) = k/κ_q.

**Proposition G (shares).** Under M-κ, s_u(τ) = n_u P_u(τ)/Σ_w n_w P_w(τ) at τ = τ_q is the unit's exact expected
share of the answer, free of κ_q. At τ = τ\* (the root of Σ n P = k, where k·s_u = n_u P_u(τ\*) with no division),

    ln s_u(τ*) − ln s_u(τ_q) ≈ (h_u/h̄ − 1) · ln κ_q ,                                              (11.1)

h_u = −∂ ln P_u/∂τ the unit's hazard and h̄ its value-weighted mean. So the plan on the values n_u P_u(τ\*) has
log-error at most |ln κ_q| · max_u |h_u/h̄ − 1| over the units at the margin, and Proposition E applies with that δ;
the plan on the raw values n_u P_u(τ_q) has log-error |ln κ_q| against a price.

*Proof.* The first statement is division of M-κ by its sum over units. For the second, moving τ from τ\* to τ_q
multiplies P_u by exp(−h_u Δ) to first order, Δ = τ_q − τ\*; summing, k/κ_q = k·exp(−h̄Δ), so Δ = ln κ_q / h̄, and
the share of u changes by exp(−(h_u − h̄)Δ). ∎

So a per-query factor and an offset of τ are the same thing to first order, and solving τ from the model's own
count removes it: the model is used at its own k-th quantile, where it is self-consistent, instead of at the
truth's, where it is off by κ_q. What is left is the spread of the hazards across units, a second-order term
(with a Gaussian tail h_u ≈ ζ_u/s_u, and the units at the margin have similar ζ).

**Why the re-solved τ̂ is worse.** Its equation, (found above t) + Σ_unread n P(t) = k, trusts the table's level on
the unread units. For a query with κ_q > 1 the units read deliver more than predicted; the equation is met at a
τ̂ near the running k-th best with almost nothing predicted to remain, and the process stops while true answers
are unread. For κ_q < 1 the found count falls short, τ̂ rises, much is predicted to remain, and the process reads
on. The factor that the frozen τ\* cancels is brought back into the stopping decision, read by read. With the true
τ and raw values it is there in full. That is the order the replay measured.

[checked, `policy_check2.py`: 150 queries, a model right in the pool and off per query by ln κ of sd 1.07; modelled
cost at recall 0.9 relative to τ\* once: true τ with raw values 1.34, re-solved after every read 1.08, true τ with
values renormalised to shares 0.97, the model with the true spread 0.98. The replay's order is reproduced, and the
true τ is harmless once its values are normalised: the damage was the level, not the threshold.]

**Theorem C′ (the price on shares).** Let s_u be calibrated shares: E[N_u/A | features] = s_u, A the size of the
query's answer (k, or the number of rows at or above t). Among all rules choosing a closed set per query from the
same features with expected cost at most that of the plan S_λ on the values s_u, the plan has the largest expected
recall. *Proof.* Lemma 1 and Theorem C with g(S) = Σ_S s_u; recall is Σ_read N_u/A. ∎

That is the precise sense: **τ\* once with a global price is the optimal static rule for mean recall per mean cost
when the table's shares are calibrated, whatever its level per query.** It is not optimal when the shares
themselves are off (the shape term of (11.1), and everything a unit's features do not carry: 11.5), and it is
static: it does not use what is found (11.4).

Two corrections to the replay's reading. (i) The price is not "in effect a predicted-recall target per query":
at one price a query whose answer is concentrated stops at a high predicted recall and a diffuse one at a low
one; by Theorem C′ that unevenness is what makes the price cheaper than a per-query target at the same mean
recall. (ii) Nothing in this says the table is good at τ\*; it says its errors there are of the kind that cancel.

### 11.3 The dials, re-specified

**Value.** For both query types, the value of a unit is its predicted share of the answer:

    s_u = n_u^vis P_u(τ) / Λ(τ) ,    Λ(τ) = Σ_w n_w^vis P_w(τ)  over all units holding a visible row,

- **k-NN:** τ = τ\*, the root of Λ(τ\*) = k, solved once per query; then s_u = n_u^vis P_u(τ\*)/k. τ is not updated
  (but see 11.4).
- **Threshold:** τ = t. Λ(t) is the table's estimate of the answer size and is wrong by κ_q; dividing by it
  removes the factor exactly as τ\* does. This is what was missing where P1 failed (LAION, answers of 0.01%): the
  plan was pricing counts, and for a threshold query a count objective is also a different objective (it weighs
  a query by the size of its answer, where mean recall weighs queries equally). [checked in the same simulation:
  shares need 0.87 of the cost of counts at recall 0.9, with ln κ of sd 0.59] One guard, with no constant: divide
  by max(Λ(t), 1). An answer has at least one row or is empty, so when the table predicts less than one row the
  values fall back to counts and a query whose answer is probably empty reads little, instead of spreading a
  whole unit of recall over noise.

**Price.** λ in recall per microsecond: stop when a further microsecond is expected to add less than λ of recall.
The plan is (1.1) with v = s. One table λ(r̄) from mean recall to price, fitted at build on held-out pseudo-queries,
per k band (and per answer-size band for thresholds), because the cost of a query grows with both.

**Recall target per query.** The smallest plan with Σ_S s_u ≥ r (the breakpoints of Theorem B, or bisection on λ
when clusters have the direct alternative). It no longer reads found items (the specification in 1.5 for k-NN,
"until #{found ≥ τ̂} ≥ r·k", is withdrawn: it rests on τ̂). Achieved recall equals r on average if the shares are
calibrated; each query's own recall scatters around r by what the features do not carry.

**Budget.** Unchanged: the largest plan within it. The order is by share per cost, and a factor per query cannot
change an order, which is why O1 and O2 were right before any of this.

**Under visible sets.** Random: s is invariant to any miscalibration common to a query's units, including the
table's drift under thinning (the replay's δ of 0.4 to 1.3 with a shifted median on yahoo): shares can be right
where counts are not, and the cost curves say they are. Correlated: the same cancellation for the common part;
the part that differs by unit (which clusters' visible rows are the ones near the query) is the weakness of
`theory.md` 7.1 and stays.

**What the table must satisfy now** (replacing 4.1, points 1 and 4): calibration of shares, E[N_u/A | features] = s_u,
which the pooled δ of the replay does not measure and the per-query κ does not contradict. The check: for each
query, bin the units by predicted share and compare Σ predicted with Σ observed N_u/A; report δ_s, the largest
|ln(observed/predicted)| over bins holding 95% of the recall, per query type and per visible set. κ_q is still
reported (its spread sets the prior of 11.4 and tells how far Λ(t) can be trusted as an answer size).

**Sub-clusters.** When a cluster is expanded, its sub-clusters' values n_s P_s(τ) come from the sub-cluster row of
the table and need not sum to the cluster's n_c P_c(τ). Two specifications, both in the next replay: **N0**, use
them as they are (the finer level is taken to know better how much the cluster holds; this is what the replay
ran, and its sub-cluster arms were the best unfiltered); **N1**, rescale them to the cluster's planned value (the
query's shares keep summing to one; the finer level only redistributes). N1 is the one Theorem C′ covers; N0 can
be better if the sub-cluster table is the better estimate of level, and is then a (legitimate) update of shares.

### 11.4 Should τ, or anything, be updated from what is found?

Not by the equation of 1.3. If at all, by an update that learns the level instead of assuming it. Under M-κ with a
Gamma(a, a) prior on κ_q (mean 1; a = 1/(e^(s²) − 1) for ln κ of standard deviation s, taken from the calibration),
the counts found in the units read are Poisson with mean κ_q times the prediction, so

    κ̂(t) = ( a + #{found ≥ t} ) / ( a + Σ_read n_u P_u(t) ) ,
    τ̂ : #{found ≥ τ̂} + κ̂(τ̂) · Σ_unread n_u P_u(τ̂) = k ,        v_u = κ̂(τ̂) · n_u P_u(τ̂)  for unread units.     (11.2)

Before anything is read κ̂ = 1 and τ̂ = τ\*: it starts as the frozen plan. As reads accumulate the level is learned
from them and the values of the unread units sum to the answers actually missing, so a price on them is again a
price on counts that are right. As a → ∞ it is the update of 1.3 (which the replay refuted); a → 0 trusts only
what is found.

[checked in `policy_check2.py`: with a = 0.3, cost at recall 0.9 / 0.95 is 0.97 / 0.95 of τ\* once, equal to the
model with the true spread; with a = 3, 0.98 / 0.99; the update of 1.3 is 1.08 / 1.11.] The gain over τ\* once is a
few per cent in a model where the error is exactly a common factor. On real data the factor is not exactly
common, the units read first are the ones the table liked best (their residuals are not a fair sample if the
table's error grows towards the margin), and each epoch costs O(K). **Default: τ\* once. (11.2) is an arm of the
next replay, not part of the specification, and the plain re-solve is withdrawn.** With (11.2), re-planning at
epochs is as in 1.3 and Proposition F still needs keys rebuilt at an epoch, since values can rise.

### 11.5 The gap to the clairvoyant plan, and what could narrow it

The replay's clairvoyant plan (true counts as values) costs 0.20, 0.14 and 0.34 of `spread` at recall 0.9; the
best arms 0.44 to 0.56. With shares the per-query factor is no longer in that gap: it cancels. What is left is
the error of the shares, unit by unit. It has three parts, and only two can be reduced.

1. **Realisation.** Even with every unit's expected share known exactly, which units hold the k rows is random.
   The clairvoyant plan knows the realisation and no rule can. In the simulation, where rows are independent, this
   part is small (the clairvoyant is 0.88 of the model with true rates at recall 0.9). On real data the answer's
   rows are near one another, so they fall into few units together and the counts are over-dispersed: this part
   is larger and nothing stored per unit removes it (`../theory.md`, Proposition 8). k = 100 will show its size:
   `theory.md` measured the clairvoyant order 6 times cheaper in rows than `spread` at k = 10 and 2.2 times at
   k = 100 on glove clusters.
2. **Shape within a unit**: what the mean, one spread and the radius do not say about the unit's tail towards q.
3. **Unit effects**: units that deliver more or less than their features predict for every query (dense cores,
   outlying shells).

Candidate features, each read-only beside the index (no row, basis or code is touched) and each a statement about
shares, so robust to the per-query level:

- **F1. The mixture over sub-clusters** (part 2). P_c(τ) = Σ_s (n_s/n_c) P_s(τ) is a far better description of a
  cluster's tail than one Gaussian-like term; it is what expansion computes, and the measured gain of sub-clusters
  is this. With the exact spread per sub-cluster, `theory.md` measured rows to recall 0.9 of 2.05%, 0.48% and 0.93%
  (glove, yahoo, LAION text), against 1.07%, 1.06% and 0.85% for the clairvoyant order over whole clusters: a
  16-way mixture with good spreads is about where the cluster-level clairvoyant is. That is the realistic target:
  **1.4 to 2 times fewer rows than the sub-cluster arms' cheap spreads give (2.87%, 0.70%, 1.89% in that
  measurement), not the 3 to 7 times of the clairvoyant.** The obstacle is the
  sub-cluster spread (the replay's rank-4 own spread is miscalibrated on LAION, δ 0.54 to 0.82).
- **F2. A fitted effect per unit** (part 3): one number b_u per cluster and sub-cluster, the shrunk mean over the
  build's pseudo-queries of ln(observed/predicted) for that unit, added to ln P. One byte per unit, no query-time
  cost. The replay's residuals by rad and size terciles are within ±0.1 to ±0.27; per unit they are not known and
  are the first thing to measure.
- **F3. The extremes the exact path already stores** (part 2): the support table holds, for each anchor and
  cluster, the largest and smallest anchor·x over the cluster's rows, and a query reads one row of it. The
  cluster's realised reach towards the query's nearest anchor, (max − p̂·m_c)/s_c, is a measured tail feature in
  a direction near q. It helps where the query is near an anchor (item-like queries) and not otherwise, and it
  describes all rows, not the visible ones.
- **F4. What is found** (part 1, the only handle on it): answers are near one another, so the units near the best
  items found are likelier to hold the rest than their centre scores against q say. As a feature: the centre
  score against the best item found so far, beside the score against q. It costs K·d per use, makes the values
  move with the reads (so the plan is re-made, as with (11.2)), and is the one item here that goes beyond a static
  rule. Its ceiling can be measured before building anything: the order by centre score against the query's true
  nearest neighbour.

What I would not expect to help: more moments per unit (the fourth-moment bound of `../theory.md` 3.3 needs
tensors), or a better tail family (the per-query factor absorbed the tail's level, and Proposition 6 of that note
says a common shape does not change the order).

### 11.6 Knobs, restated

As section 3, with these changes: the epoch schedule is gone with the update; λ is in recall per microsecond;
f′ is replaced by the table on S1 or S2, not on σ(q); the prior strength a of (11.2) exists only if that arm is
adopted and is then read off the calibration, not set.

---

## 12. The next replay

Same harness, scripts and cost model as `policy_replay.md`, with the changes of section 11. Everything is
calibrated per index on its corpus; real queries are never used in a fit.

**Data.** The three suite datasets with their 100 queries **and the 1,000-query sets**; **LAION with held-out image
queries** (in distribution) beside its text queries. The price is a between-query effect and 100 queries is few
for it: report every cost ratio with a bootstrap interval over queries.

**k and answers.** k = 10 **and k = 100**; thresholds at answers of 0.01%, 0.1%, 1%, 10%. A separate λ sweep per
k and per answer size; the table λ(r̄) per band is a deliverable.

**Visible sets.** Unfiltered; random 1%, 5%, 25%; whole clusters 1% and 25%; a cap 1% and 25%; a contiguous id
range 5%.

**Arms.**

| arm | values | τ | stop |
|---|---|---|---|
| T1, T2, T4 | the tuned orders (`spread`; H r16 ×4; 16 sub-clusters two-level) | – | probe count |
| O1, O2 | P at τ\* (S1; S2), order only | τ\* once | probe count |
| **Q1, Q2** | **shares at τ\* (S1; S2); threshold: n P(t)/Λ(t)** | **τ\* once; t** | **price on recall** |
| Q1r, Q2r | the same | the same | per-query predicted-recall target r ∈ {0.8, 0.9, 0.95, 0.99} |
| Q1c | counts n P(t), thresholds only (the first replay's P1) | t | price on answers |
| Q1t | shares at the true τ (values renormalised) | true τ | price |
| Q1k | (11.2), a from the calibration's κ spread; epochs at doubling | κ-learned | price |
| Q1s, Q2s | Q1, Q2 with 16 sub-clusters, expansion **E1**, sub-values **N0** and **N1** | τ\* once | price |
| Q1e, Q2e | the same with **E2** (the Ψ table of 1.4 and 4.3, fitted on shares) | τ\* once | price |
| Q2z | Q2 with S2 evaluated lazily under the rank-0 bounds (Proposition F): units evaluated against units available | τ\* once | price |
| F2, F3 | Q1 with the per-unit effect; with the support-table reach (if the fit has the table) | τ\* once | price |
| F4 ceiling | order by centre score against the true nearest neighbour, alone and added to Q1's key | – | probe count |
| ceilings | true counts as values; true shares with the direct path fixed (`i`) | – | price |

Each Q arm also with the path fixed by the planner's rule (`i`), as before, so that order and stop are separated
from the choice of path.

**Calibration report, before any recall number.** As 4.4, plus: δ_s (11.3) per query type, level and visible set;
κ_q's distribution per k and answer size, for plain pseudo-queries, shifted ones and real ones (it sets a); the
shape term of (11.1): the spread of h_u/h̄ over the units of each plan; residuals **per unit** and per group (F2);
for shifted pseudo-queries, the over-count the first replay saw (e^1.0 to e^2.4), with the shift rebuilt along
directions in which the centres do not vary, and the table refitted without shifted queries as a control: with
shares the level they were meant to teach should no longer matter.

**Measures.** Mean recall against mean modelled cost and against rows, with intervals; cost at recall 0.8, 0.9,
0.95; clusters and groups opened; for target arms, achieved against requested recall and the share of queries
more than 0.1 below target; for Q1k, epochs and the trajectory of κ̂ against the true κ_q.

**What would confirm section 11.** Q1 ≤ P1\* and Q2 ≤ P2\* of the first replay within noise for k-NN (they are the
same plan), and for thresholds Q1 well below Q1c on LAION at 0.01% answers, with no arm above T1. Q1t within
about 5% of Q1 (the true τ harmless once normalised): if Q1t is clearly better, the shape term matters and an
update is worth having; if clearly worse, the table is better at its own quantile than at the truth's, which
(11.1) does not predict. Achieved recall of the target arms within 0.02 of requested on average, for image, text
and corpus queries alike. Under random thinning, δ_s small where δ was large.

**What would send it back.** Q1k clearly better than Q1 (then the level carries information the frozen plan
wastes, and (11.2) becomes the specification); N0 against N1 differing by more than noise in either direction
(decides whether a finer level may move mass between clusters); E2 no better than E1 (then drop the second
table); F2 or F3 closing less than a tenth of the gap to the clairvoyant plan (then stop looking for static
features and measure F4's ceiling); k = 100 showing a clairvoyant gap as large as at k = 10 (then the gap is not
mostly realisation, and features are worth more than 11.5 allows).

## References (from memory; not checked)

- W. A. Horn, Single-machine job sequencing with treelike precedence ordering and linear delay penalties, SIAM J.
  Appl. Math. 23 (1972).
- J. B. Sidney, Decomposition algorithms for single-machine sequencing with precedence relations and deferral
  costs, Operations Research 23 (1975).
- W. Dinkelbach, On nonlinear fractional programming, Management Science 13 (1967).
- G. Weiss, Branching bandit processes, Probability in the Engineering and Informational Sciences 2 (1988);
  D. Bertsimas, J. Niño-Mora, Conservation laws, extended polymatroids and multiarmed bandit problems, Mathematics
  of Operations Research 21 (1996). Not relied on.
