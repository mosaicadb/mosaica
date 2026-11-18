# One process for the approximate modes

A specification of a single rule for approximate k-NN and approximate threshold queries, to replace the tuned
constants of the cluster order (f′, H's α, z(k, n) and 0.3, the candidate multiple m, the sparse fallback, nprobe,
a charge for opening groups, a rule for expanding sub-clusters), with the conditions under which it is optimal.
It follows `theory.md` (this directory) and `../theory.md`.

Status labels: **[proved]** (proof here), **[checked]** (exhaustive or simulated check in `theory/policy_check.py`,
output `theory/policy_check_out.txt`), **[cited]** (not verified against the paper), **[open]** (to be settled by
the replay of section 9). Nothing here has been run on real data; the measurements quoted are from `theory.md`.

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

## References (from memory; not checked)

- W. A. Horn, Single-machine job sequencing with treelike precedence ordering and linear delay penalties, SIAM J.
  Appl. Math. 23 (1972).
- J. B. Sidney, Decomposition algorithms for single-machine sequencing with precedence relations and deferral
  costs, Operations Research 23 (1975).
- W. Dinkelbach, On nonlinear fractional programming, Management Science 13 (1967).
- G. Weiss, Branching bandit processes, Probability in the Engineering and Informational Sciences 2 (1988);
  D. Bertsimas, J. Niño-Mora, Conservation laws, extended polymatroids and multiarmed bandit problems, Mathematics
  of Operations Research 21 (1996). Not relied on.
