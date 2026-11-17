# Is there an optimal cluster visit order for IVF-style approximate k-NN?

Status labels used throughout: **[proved]** (proof or complete sketch given here), **[cited]** (standard result,
reference given), **[heuristic]** (argument or numerical evidence only). All numerical checks are on synthetic
score-level data (`sim.py` in this directory); nothing here was run on the real replay data.

---

## 0. Headline

1. **There is a provably optimal static order, but only relative to a probability model.** For every query, let
   `p_c = E[N_c | information] / n_c` be the probability that a random row of cluster c is a true top-k member.
   Visiting clusters in decreasing `p_c` (decreasing `n_c p_c / (n_c + h)` with a per-cluster overhead of h rows) is
   optimal among static orders for expected recall under a row budget (up to one cluster of knapsack rounding), is
   exactly optimal for the area under the recall-versus-rows curve (Smith's rule), and thresholding `p_c` at one
   global level across queries is exactly optimal for an average budget (Neyman-Pearson). The competition between
   neighbours through the k-th best score τ does **not** break this: recall is additive over clusters, so only the
   marginals `E[N_c]` matter. [proved]
2. **Without a model there is no optimal or near-optimal order.** Given only (μ_c, σ_c, n_c), an adversary can put
   all k neighbours in essentially any cluster; every order has worst-case recall 0 at any budget short of that
   cluster. Mean and variance alone certify nothing at the relevant scale (Cantelli allows n_c/(1+ζ²), about 200
   rows at ζ = 3, against k = 10). [proved]
3. **Under the independent-cluster model the optimal index is exactly `p_c = E[F̄_c(τ)]`**, the expected share of
   the cluster's rows that beat τ (the threshold order, ranked by p, not by n·p). If the within-cluster score
   distributions form a location-scale family with a common shape, the order is by
   `ζ_c = (τ − μ_c)/σ_c` ascending **whatever the shape is** (Gaussian, Student-t, Pearson-II/Beta on the sphere),
   and the distribution-free Cantelli bound gives the same order. The tail model enters only through τ. [proved]
4. **O3 and every linear rule μ_c + ψ σ_c are tangent approximations of that order.** The set selected by the
   ζ-order at one budget equals the set selected by the linear rule with ψ equal to the marginal ζ at that budget;
   the right ψ grows with the budget, so no fixed multiplier is optimal across budgets unless the σ_c are equal.
   O3's z(k, n_c) is the right multiplier only for clusters expected to hold k rows above τ. Linear rules have their
   own optimality theorem, for a different objective (sum of found scores minus a price per row; Weitzman's
   reservation index). [proved / cited]
5. **Adaptive policies can be strictly better, but in-model by very little.** No adaptive policy can beat the
   knapsack that is told τ in advance; in-model τ is known to about σ/(ζ√k), so the gain is second order. Adaptivity
   pays as protection against a wrong tail model, because the running k-th best estimates τ without one.
   [proved bound; heuristic magnitude; confirmed in simulation]
6. **The practical obstacle is the accuracy of σ_c(q), not the tail model.** Recall under an index with
   log-density error δ is at least e^(−2δ) times optimal, and for a Gaussian tail δ ≈ ζ² × (relative error of σ).
   At ζ = 3 to 5 the ζ-order needs σ_c to within a few percent. With noisy σ̂ the optimal remedy is shrinkage of σ̂
   towards a pooled estimate, which is what the multipliers α < 1 and the extra rad term in H do. [proved bound;
   heuristic identification with H; confirmed in simulation]
7. **O0 is not justifiable; O2, O3 and H are.** The true top-k is invariant under q ↦ a·q + v (v a direction in
   which the corpus does not vary: the modality-gap direction); O2, O3, H and the ζ-order are invariant too, O0 is
   not, and its effective f is off by the factor a. [proved]

Recommendation in one line: rank by `ζ_c = (τ̂ − μ_c)/σ̃_c` with a **regression-calibrated, shrunk** σ̃_c built from
the same two ingredients H already uses, τ̂ from the self-consistency equation (updated from the running k-th best
once a few clusters are scanned), and stop on a global threshold of `p_c`; until the calibration exists, H r16 is
the best-justified of the measured orders and O2 with f′ = 0.4 the best-justified cheap one. Details in section 9.

---

## 1. Setting, notation and models

Query q (unit). Clusters c = 1..K with n_c rows, N = Σ n_c. Scores S_{c,i} = q·x_{c,i}. T(q) is the true top-k set,
τ its smallest score, N_c = |T(q) ∩ c|, so Σ_c N_c = k. A probe of c scans all of c at cost w_c = n_c + h (h is
the per-cluster overhead in row units). For a visited set V,

    Rec(V) = (1/k) Σ_{c ∈ V} N_c .                                                   (1.1)

Query-time information I: μ_c = q·m_c, σ_c(q)² = qᵀΣ_c q (or an approximation), n_c, rad_c, and σ(q), the standard
deviation over clusters of μ_c. Write g_c = E[N_c | I] and `p_c = g_c / n_c`. For unit vectors
rad_c² = E|x − m_c|² = 1 − |m_c|² exactly, so rad_c carries no information beyond |m_c|.

A **static** policy chooses the order (or the set) as a function of I. An **adaptive** policy chooses the next
cluster as a function of I and all scores seen so far.

Models, in increasing strength:

- **M0 (no model).** Only the realised statistics are known.
- **M1 (independent clusters).** Given I, the score multisets of different clusters are independent and the scores
  in c are i.i.d. with distribution F_c (density f_c, survival F̄_c).
- **M2 (Poisson version of M1).** The scores of c are a Poisson process with intensity n_c f_c(s) ds. M1 and M2
  differ by O(Σ_c n_c F̄_c(s)²) in total variation for the counts above s (Le Cam), negligible where F̄_c ~ 10⁻³.
- **M3 (location-scale).** M1 or M2 with F̄_c(s) = Ḡ((s − μ_c)/σ_c) for one strictly decreasing Ḡ (the shape).
  Gaussian: Ḡ = 1 − Φ.

Λ(s) = Σ_c n_c F̄_c(s) is the expected number of rows scoring above s.

---

## 2. Objectives, and the optimal static order

### 2.1 Coupling through τ does not matter for static policies

**Lemma 1 [proved].** For any static policy with visited set V = V(I), under any joint distribution of the scores,
E[Rec(V) | I] = (1/k) Σ_{c ∈ V} g_c.

*Proof.* (1.1) is linear in the N_c and 1[c ∈ V] is I-measurable; take conditional expectations. ∎

The dependence between the N_c (they sum to k, neighbours compete) affects the variance of the recall and the
value of g_c (which depends on all clusters' statistics through τ), but not which static set is best.

### 2.2 Expected recall under a row budget

**Theorem 1 [proved; knapsack facts cited].** Maximising expected recall over static policies with Σ_{c∈V} w_c ≤ B
is the deterministic 0/1 knapsack with values g_c and weights w_c. Let π order clusters by decreasing density
d_c = g_c / w_c and P_j be its first j clusters. Then

1. P_j is optimal for budget B = w(P_j), also against fractional solutions;
2. for w(P_j) ≤ B < w(P_{j+1}), OPT(B) ≤ g(P_j) + (B − w(P_j))·d_{π(j+1)} < g(P_j) + g_{π(j+1)}: the recall loss
   of the greedy prefix is less than g_{next}/k, the expected share of neighbours in one marginal cluster;
3. with unit costs (an nprobe budget) the order by g_c = n_c p_c is exactly optimal for every budget at once;
4. in general no single order is optimal for all row budgets (knapsack solutions are not nested: sizes 2 and 3
   with g = 0.5 and 0.6, budgets 2 and 3), so "optimal order" can only mean (1) and (2).

*Proof.* Lemma 1 gives the knapsack. (1) and (2) are Dantzig's bound: the LP relaxation is solved by filling in
density order, and its value upper-bounds OPT. (3) is the case of equal weights. ∎

Consequences. With row cost and h = 0 the index is `p_c`, which has **no size bonus**: a larger cluster holds more
neighbours but costs proportionally more. With an nprobe budget it is n_c p_c. With overhead it is
n_c p_c/(n_c + h). There is no (1 − 1/e) here: that factor belongs to submodular objectives (2.5).

### 2.3 Area under the curve: Smith's rule

**Theorem 2 [cited: Smith 1956; proved by adjacent interchange].** For a static order π let C_c(π) be the rows
scanned when c is finished. Then ∫₀^∞ (1 − E Rec_π(B)) dB = (1/k) Σ_c g_c C_c(π), the weighted completion time of
single-machine scheduling with weights g_c and processing times w_c. It is minimised exactly by decreasing
g_c/w_c.

So for "mean number of rows scanned until a uniformly chosen true neighbour is found", the density order is
exactly optimal among static orders. The benchmark score (mean recall over a grid of shares) is a weighted area;
by Theorem 1(2) the density order is within g_next/k of optimal at every budget, hence for every weighting.

### 2.4 Budgets averaged over queries: a global threshold

**Theorem 3 [proved; Neyman-Pearson form].** Let queries be drawn from any distribution and let a static policy
choose V(q) subject to E_q[w(V(q))] ≤ B̄. For λ ≥ 0 let V_λ(q) = {c : g_c(q)/w_c > λ}. Then V_λ maximises
E_q[Rec] among all policies whose average cost is at most E_q[w(V_λ(q))].

*Proof.* For every q and every set V, Σ_V (g_c − λ w_c) ≤ Σ_{V_λ} (g_c − λ w_c). Take expectations:
E g(V) ≤ E g(V_λ) − λ(E w(V_λ) − E w(V)) ≤ E g(V_λ). ∎

The metric here (recall against the share of rows scanned, averaged over queries) is of this type. The optimal
policy spends more rows on hard queries and fewer on easy ones: the stopping rule is "stop when the best remaining
`p_c` is below λ", not a fixed nprobe or a fixed row count. This needs `p_c` to be calibrated across queries, which
is more than an order. In simulation (section 10) the global threshold lifts recall at a 0.5% share from 0.56 to
0.65 and from 0.78 to 0.89 in two regimes, even with exchangeable queries. Quake's APS and learned early
termination (LAET, DARTH) are instances of this idea [cited].

### 2.5 Other objectives, and where index rules stop being exact

- **Minimum expected rows to reach recall r.** For k = 1 (or "until a given neighbour is found") this is the
  classical optimal-search problem with perfect detection: boxes with location probabilities g_c and costs w_c are
  opened in decreasing g_c/w_c [cited: Blackwell; Black 1965; Stone 1975]. For k > 1 and a recall target the cost is
  the first prefix that covers r·k neighbours; this depends on the joint law of the N_c, not only the marginals, and
  is a generalised min-sum set cover. No index rule is optimal in general; greedy ratio rules are constant-factor
  approximations (4 for min-sum set cover, Feige-Lovász-Tetali 2004; constants for the generalised problem,
  Azar-Gamzu-Yin 2009, Bansal-Gupta-Krishnaswamy 2010) [cited]. This is the only one of the framings in which the
  coupling through τ matters to a static order. Since the system cannot observe that it has reached recall r, this
  objective is an evaluation metric, not something a policy can stop on; I recommend the budget and area forms.
- **(1 − 1/e).** Appears when the objective is submodular and not modular: the probability of finding at least one
  neighbour, the sum of the k best scores found, or adaptive submodular objectives (Nemhauser-Wolsey-Fisher 1978;
  Sviridenko 2004 for knapsack constraints; Golovin-Krause 2011 for adaptive greedy) [cited]. Recall against the
  true top-k is modular, so greedy does better than that (Theorem 1).
- **Gittins-type indices.** Needed when a box can be searched repeatedly or partially with learning (imperfect
  detection, or stopping part-way through a cluster). With whole-cluster probes each cluster is opened once and the
  Gittins index reduces to the ratio g_c/w_c. A genuine Gittins/Weitzman index appears for the score-sum objective:

**Theorem 4 [cited: Weitzman 1979 for k = 1; Kleinberg-Waggoner-Weyl 2016 and Singla 2018 for matroid (top-k)
selection; the corollary for clusters is immediate].** Under M1 with h = 0, consider maximising
E[sum of the k best scores found] − λ·(rows scanned) over **adaptive** policies. Each row is a Pandora box with
cost λ and value distribution F_c. The optimal policy opens rows in decreasing reservation value ρ_c, defined by
E_{F_c}[(S − ρ_c)₊] = λ, and stops when the running k-th best score is at least the next ρ_c. All rows of a cluster
share ρ_c, so the policy is a cluster order. For Gaussian F_c, ρ_c = μ_c + ψ_c σ_c with ψ_c = L⁻¹(λ/σ_c),
L(z) = φ(z) − z(1 − Φ(z)).

So for the score-sum objective the optimal adaptive policy has a **static order of the linear form μ_c + ψσ_c,
with no dependence on n_c**, and adaptivity only in the stopping time; ψ grows as λ falls (larger budgets). This is
the cleanest theorem that supports an O3-shaped rule. It is for a different objective from recall.

---

## 3. The index under a model, and what the summary statistics can and cannot give

### 3.1 The exact Bayes index under independent clusters

**Proposition 5 [proved].** Under M2, with τ the k-th largest of all scores,

    p_c = ∫ f_c(s) · P(Poisson(Λ(s)) ≤ k − 1) ds = E[ F̄_c(τ) ] ,      Σ_c n_c p_c = k .          (3.1)

*Proof.* By the Mecke formula, E[N_c] = ∫ n_c f_c(s) P(fewer than k points of the superposition lie above s) ds, and
the number of points above s is Poisson(Λ(s)). That event is {τ ≤ s}, so the integral is n_c E∫_τ^∞ f_c = n_c
E[F̄_c(τ)]. The sum rule is ∫₀^∞ P(Poisson(λ) ≤ k − 1) dλ = k. Under M1 replace the Poisson count by the sum of
binomials; the difference is the Le Cam error. ∎

Hence, **under M1/M2 the threshold order ranked by p_c = E[F̄_c(τ)] is the optimal static order for row cost**; the
variant ranked by n_c p_c is the optimal order for an nprobe budget. This answers the question "order statistic or
exceedance probability": the exceedance probability is the right object.

Two approximations of (3.1):

- **Plug-in τ*.** P(τ ≤ s) is a step of relative width 1/√k around the solution τ* of Λ(τ*) = k (because Λ(τ) is
  Gamma(k, 1)); so p_c ≈ F̄_c(τ*). τ* is not a free parameter; it is found by bisection in O(K) per step, and
  max_c [μ_c + σ_c z(k, n_c)] (the best O3 score) is a lower bound for it, a free starting point.
- **Smoothed τ.** If τ ~ N(τ*, v²) and F_c is Gaussian, then exactly

      p_c = 1 − Φ( (τ* − μ_c) / sqrt(σ_c² + v²) ) .                                              (3.2)

  Uncertainty about τ (including a common error in the μ_c or in the tail model) acts as a variance floor v² added
  to every cluster and pulls the order towards plain μ_c. In-model v ≈ σ̄/(ζ̄√k) is small (delta method on
  Λ(τ) ~ Gamma(k,1) with the Gaussian hazard ζ/σ).

### 3.2 Location-scale families: the order does not depend on the tail shape

**Proposition 6 [proved].** Under M3 with τ given, the row-cost order is by ζ_c(τ) = (τ − μ_c)/σ_c ascending, for
every strictly decreasing Ḡ. The shape Ḡ changes the order only through τ* (the solution of Σ n_c Ḡ(ζ_c(τ)) = k)
and through the smoothing in (3.1).

*Proof.* p_c = Ḡ(ζ_c) and Ḡ is strictly decreasing. ∎

Consequences:

- A Student-t (or Pearson-II) correction **with one common ν and the true or running τ produces exactly the same
  order as the Gaussian**. It can only matter through the model τ (heavier tails give a larger τ*, hence larger ζ
  and more weight on σ), through a cluster-specific ν_c, or through calibrated probabilities for a stopping rule.
  This is a check for the experiment in flight: with true τ, the t-order must coincide with the Gaussian one.
- The family of candidate orders is one-dimensional, indexed by τ: "the ζ-fan".

### 3.3 Distribution-free: Cantelli gives the same order, and no guarantee

**Proposition 7 [proved; Cantelli's inequality cited].** For the empirical distribution of the n_c scores of
cluster c, with exact mean μ_c and exact variance σ_c², and any t > μ_c,

    #{ i ∈ c : S_{c,i} ≥ t } ≤ n_c / (1 + ζ_c(t)²) ,                                              (3.3)

and the bound is attained (two-point distribution). This is deterministic. Consequences:

1. The minimax-optimistic index over all distributions with the given mean and variance, sup P(S ≥ τ) = 1/(1+ζ²),
   is again monotone in ζ_c: the robust order is the ζ-fan as well.
2. (3.3) holds with the running k-th best t_V in place of τ (t_V ≤ τ), so Σ_{c ∉ V, μ_c < t_V} min(k, n_c/(1+ζ_c(t_V)²))
   is a valid upper bound on the neighbours still missing, computable at query time, provided σ_c² is an upper bound
   on qᵀΣ_c q (with a rank-r approximation use λ_{r+1}|q_⊥|² for the remainder, not the average remaining
   eigenvalue). **Numerically it is useless**: n_c = 2,000 and ζ = 3 give 200 ≫ k.

**Proposition 8 (no model, no guarantee) [proved].** Suppose two unvisited clusters a, b both satisfy
n_c/(1 + ζ_c(t₀)²) ≥ k for t₀ = max_c(μ_c + σ_c). Then there are two score configurations with identical
(μ_c, σ_c, n_c) for all c, one with all k neighbours in a and one with all k in b (put k rows of the chosen cluster
at μ + σ·sqrt((n − k)/k) ≥ t₀, the rest at μ − σ·sqrt(k/(n − k)); every other cluster on the two points μ ± σ ≤ t₀).
Any order visits one of a, b before the other and has recall 0 on one configuration until the second is scanned.
So no order has a non-trivial worst-case guarantee from first and second moments; optimality is necessarily
Bayesian. Higher moments tighten (3.3) (to order n·κ₄/ζ⁴ with the fourth moment) but need tensors that are not
available per query.

### 3.4 O3 and linear rules versus the ζ-fan

**Proposition 9 [proved].** In the (μ, σ) plane the level sets of the ζ-order are the lines μ + ζσ = τ through the
point (τ, 0) (a fan); those of a linear rule μ + ψσ are parallel lines. Therefore:

1. the set {c : ζ_c(τ) ≤ ζ₀} equals the set {c : μ_c + ζ₀σ_c ≥ τ}: at each budget the fan selects the same clusters
   as the linear rule with multiplier ψ = ζ₀, the marginal ζ at that budget, cut at the level τ;
2. ψ(B) = ζ_margin(B) is increasing in the budget, so a fixed multiplier reproduces the fan at one budget only;
   the two orders coincide for all budgets if and only if they never disagree on a pair, which holds when all σ_c
   are equal (and then both equal the order by μ_c);
3. O3's statistic u_c = μ_c + σ_c z(k, n_c) satisfies u_c ≥ τ ⇔ n_c F̄_c(τ) ≥ k: O3 at level τ selects the clusters
   expected to hold at least k rows above τ. A marginal cluster expected to hold m rows above τ sits at
   ζ = z(m, n_c). For n_c = 2,000: z = 2.58, 3.29, 3.89, 4.42 for m = 10, 1, 0.1, 0.01, that is 1.0, 1.28, 1.51,
   1.71 times O3's multiplier.

Reading of the measurements [heuristic]: with exact Σ_c the leave-one-out multiplier for O3 is 1.25, the value this
predicts for clusters holding about one expected neighbour; with r = 16 and r = 4 it falls to 1.0 and below, which
section 5 attributes to noise in σ̂. O3's dependence on n_c through z(k, n_c) is a size bonus that the row-cost
optimum does not have (Theorem 1); z(k, 1000) = 2.33 and z(k, 4000) = 2.81 differ by half a σ_c, which is not
negligible. A linear rule should use a constant multiplier for row budgets.

---

## 4. Static versus adaptive

### 4.1 Adaptivity can strictly help

**Example [proved].** k = 1, unit costs, budget 2. Best scores: cluster A is 2 or 0 with probability 1/2 each, B is
1, C is 3 with probability 0.3 and 0 otherwise. The nearest neighbour is in C, A, B with probabilities 0.30, 0.35,
0.35; the best static pair {A, B} has recall 0.70. Adaptive: scan A; if it shows 2, B cannot win, scan C (recall 1);
otherwise scan B (recall 0.7). Expected recall 0.85.

The mechanism is the one in the question: seeing scores moves the posterior on τ, and the best remaining cluster
depends on τ when the σ_c differ (a high τ favours high-variance clusters).

### 4.2 An upper bound on any adaptive policy

**Theorem 10 [proved].** Under M1, for a cluster c let X_{−c} be the scores of all other clusters and
G_c(X_{−c}) = E[N_c | X_{−c}]. For every adaptive policy π with budget B,

    k · E[Rec(π)] = E[ Σ_{c ∈ V_π} G_c(X_{−c}) ] ≤ E[ max_{w(V) ≤ B} Σ_{c ∈ V} G_c(X_{−c}) ] ,      (4.1)

and G_c(X_{−c}) ≤ n_c F̄_c(τ_{−c}) with τ_{−c} the k-th best score outside c.

*Proof.* The decision to probe c is taken before any score of c is seen, so 1[c ∈ V_π] is a function of X_{−c}
(a randomised policy is a mixture of deterministic ones). Hence E[N_c 1[c ∈ V_π]] = E[1[c ∈ V_π] G_c(X_{−c})]. For
each realisation V_π is feasible, which gives the inequality. A row of c in the top-k beats τ ≥ τ_{−c}. ∎

So **no adaptive policy beats the knapsack that is told the others' top-k (in effect τ) in advance**, and the
static optimum is the same knapsack with E[G_c] = g_c in place of G_c. The adaptivity gap is at most the value of
knowing τ:

    E[ max_V Σ_V G_c(X_{−c}) ] − max_V Σ_V E[ G_c(X_{−c}) ] .

Under M3 the "true-τ" order with exact parameters is therefore an in-model upper bound for every adaptive scheme
(up to the τ versus τ_{−c} distinction and knapsack rounding). The replay with τ taken from the truth measures
this ceiling; it is a ceiling only if the model for F_c is right.

### 4.3 How large is the gap

- **In-model [heuristic, confirmed numerically].** τ fluctuates by about σ̄/(ζ̄√k) around τ*; a shift Δ of τ changes
  ζ_c by Δ/σ_c, which reorders only clusters whose 1/σ_c differ. In the simulation the model-τ, true-τ, adaptive-τ
  and exact-Bayes orders agree to 0.001 in mean recall when the model is right.
- **Misspecified tails [heuristic, confirmed numerically].** If the common shape is wrong, τ* is biased (by several
  σ in the t₅ simulation) and the fan is evaluated at the wrong apex. The true-τ order then gains about 0.01 in
  mean recall over model-τ, and the adaptive rule recovers most of that. That is the size of the prize; it is real
  but small next to the effect of σ accuracy (section 5).
- **Worst case [cited, for a surrogate].** For monotone submodular objectives of independently realised elements
  under prefix-closed constraints (budgets included) the adaptivity gap is at most 2 and this is tight
  (Bradac-Singla-Zuzic 2019, improving 3 of Gupta-Nagarajan-Singla 2017; e/(e−1) for matroid constraints,
  Asadpour-Nazerzadeh-Saberi). This covers "sum of the k best scores found". Recall against the true top-k is not a
  function of the visited realisations alone, so I do not claim the factor 2 for recall; Theorem 10 is the bound
  that applies to it.
- Under the score-sum objective of Theorem 4 the optimal adaptive policy has a static order: adaptivity is worth
  nothing for the order and everything for the stopping time.

### 4.4 The principled adaptive rule

After visiting V, the model's estimate of τ solves

    #{found scores ≥ t} + Σ_{c ∉ V} n_c Ḡ(ζ_c(t)) = k ,                                           (4.2)

which is at least the running k-th best and tends to the true τ as the clusters holding the top-k are scanned.
Re-rank the remaining clusters by ζ_c(τ̂) (only needed while τ̂ still moves: the first few probes). This costs
O(K) per update. The first clusters are the same under any sensible order, so a two-phase rule (a few probes by the
static order, then one re-rank) captures the benefit.

---

## 5. Uncertain statistics: why shrinkage, and what α < 1 and the extra rad term are

The Bayes index integrates over what is not known: p_c = E[Ḡ((τ − μ_c)/σ_c) | I]. Two effects.

**Sensitivity [proved, with Theorem 12].** A relative error η in σ_c moves ζ_c by ζ_c·η, and for a Gaussian tail
d ln p/dζ ≈ −(ζ + 1/ζ). The log-density error is δ ≈ ζ²η. By Theorem 12 recall is at least e^(−2δ) of optimal, so
the ζ-order needs η ≲ 1/(2ζ²): 5% at ζ = 3, 2% at ζ = 5. A rank-4 or rank-16 approximation with an isotropic
remainder is unlikely to be that accurate for every (q, c).

**Shrinkage [heuristic derivation, confirmed numerically].** If log σ̂_c = log σ_c + noise, the posterior mean of
log σ_c is w·log σ̂_c + (1 − w)·(pooled log σ) with w = Var(log σ)/Var(log σ̂), the reliability. Because the
remaining posterior uncertainty is common, the order is that of the plug-in ζ with the shrunk σ. In the simulation
with 30% noise on σ̂: raw ζ-order 0.884, shrunk ζ-order 0.959, best linear rule 0.958 (ψ = 2, where ψ = 3 to 4 was
best without noise), O1 0.935.

For a linear rule, μ + ψσ̂ = μ + ψσ̄ + ψ(σ̂ − σ̄): **a smaller multiplier is shrinkage of σ̂ towards a common value**.
The optimal multiplier is about w·ζ_margin. This explains the measured pattern α(exact) = 1.25 > α(r16) ≈ 0.75 to
1.0 > α(r4) = 0.5: the multiplier tracks the reliability of the Σ approximation. H's second term,
0.3·σ(q)·rad_c/σ(rad), is the pooled estimate that σ̂ is shrunk towards (section 6.2 derives its form). H is thus
an empirical-Bayes combination of a specific, noisy estimate of the within-cluster spread and a pooled one, done
in the standard-deviation domain with weights chosen by leave-one-dataset-out.

**Prediction that can be checked offline, without queries labelled by truth:** regress the exact σ_c(q) on σ̂_{c,r}(q)
and on σ(q)·rad_c (within query, across clusters, on pseudo-queries). The slopes should be in the ratio of H's
coefficients, and the slope on σ̂_{c,r} should rise from r = 4 to r = 16 roughly as 0.5 : 0.75.

**Student-t as a predictive distribution [cited mechanism].** A Gaussian with inverse-gamma uncertainty on σ² has a
Student-t predictive; ν = 4 + 2/CV² where CV is the coefficient of variation of the true variance around its
estimate. This is a derivation of ν from the measured error of σ̂², not a fit. By Proposition 6 it changes the
order only through τ* unless ν differs by cluster.

---

## 6. Query shift: what O0 gets wrong, and where O2 and H come from

### 6.1 An invariance every order must respect

**Theorem 11 [proved].** Let 𝒩 be the set of directions v with Σ_c v = 0 for all c and v·m_c equal for all c (the
corpus does not vary along v). For a > 0 and v ∈ 𝒩 let q′ = a·q + v (renormalising q′ is a further positive
scaling). Then every score changes by the same affine map s ↦ a·s + β, β = v·m_c: the true top-k, every N_c and
the clairvoyant order are unchanged. Under this map μ_c ↦ aμ_c + β, σ_c(q) ↦ aσ_c(q), σ(q) ↦ aσ(q), τ ↦ aτ + β.
Consequently

- the ζ-order, O1, O3, every linear rule in σ_c(q), O2 and H are invariant;
- O0 = μ_c + f·rad_c is not: for q′ it equals a·(μ_c + (f/a)·rad_c) + β, the order of O0 with f/a.

*Proof.* v·x = v·m_c + v·(x − m_c), and the variance of v·x over the rows of c is vᵀΣ_c v = 0, so v·x = v·m_c = β
for every row. The rest is substitution. ∎

A text query against image embeddings is, to first order, of this form: a component along the modality-gap
direction (constant over the corpus) plus a scaled-down component in the directions where images vary. Theorem 11
says the right f for O0 must scale with a, so an f fitted on corpus rows as pseudo-queries (a = 1) is wrong for
text by construction. Any index should be a function of the invariants ζ_c, σ_c(q)/σ(q), n_c. This is the
principled ground for dividing by σ(q), and a necessary condition that O0 fails.

Testable: the oracle f should be proportional to the query's spread, f_oracle(text)/f_oracle(image) ≈
σ_text(q)/σ_image(q) on average (0.1/0.5 predicts a ratio near 0.2).

### 6.2 O2 as O3 under a shared covariance shape

Let B be the covariance over clusters of the centres m_c, so σ(q)² = qᵀBq exactly. Assume
(A1) Σ_c = ρ_c²·W for one shape W with ρ_c ∝ rad_c, and (A2) qᵀWq = κ²·qᵀBq for the queries of interest: within
clusters the data vary along the same directions as between clusters. Then σ_c(q) = κ·ρ_c·σ(q) and

    μ_c + ψσ_c(q) = μ_c + (ψκ)·σ(q)·ρ_c :   this is O2, with f′ = ψ·κ·σ(rad)/sqrt(tr W) .      [proved under A1, A2]

Notes. Dividing rad_c by σ(rad) and using rad_c or rad_c − mean(rad) give the same order (they differ by a
per-query constant). f′ can be computed from the corpus: f′ ≈ ψ·σ(rad)·median over (pseudo-query, cluster) of
σ_c(q)/(σ(q)·rad_c), with ψ ≈ 3 from section 3.4. (A2) is an assumption about the queries, not the corpus; if text
queries had a different κ, O2 would need a different f′ for them. That a single f′ = 0.4 is within 0.006 of the
oracle on all datasets is evidence that κ is stable; it is not proved.

The isotropic observation in the brief (O3 with Σ_c = rad_c²/d·I gives f = z/√d ≈ 0.11, matching the text
oracle) is the other limit: a query with no particular alignment sees σ_c/rad_c = 1/√d, while an in-distribution
query sees 1/√d_eff with d_eff of a few tens (section 7). O3 computes the ratio per query; O2 proxies it by σ(q).

### 6.3 H

H = μ_c + α·z·σ̂_{c,r}(q) + 0.3·σ(q)·rad_c/σ(rad) is the sum of the O3 term with a reliability-shrunk multiplier
and the O2 term as the pooled component (section 5). Both terms are invariant in the sense of Theorem 11. What is
ad hoc: the combination is in the standard-deviation domain (variances add, standard deviations do not), z(k, n_c)
carries a size bonus that the row-cost optimum does not have, and the multiplier is fixed across budgets
(Proposition 9).

---

## 7. Bounded cosines, concentration on the sphere, effective dimension

- **Uniform on the sphere [cited].** For x uniform on S^{d−1}, t = q·x has density ∝ (1 − t²)^((d−3)/2) on [−1, 1]
  ((1+t)/2 is Beta((d−1)/2, (d−1)/2)), variance 1/d, and P(t ≥ ε) ≤ exp(−dε²/2). Standardised, this is the
  Pearson type II law with log-density −(z²/2)(1 − 3/D) − z⁴/(4D) + O(z⁶/D²) in dimension D: **lighter** than
  Gaussian, by a factor about exp(−z⁴/4D) in the density.
- **Within a cluster.** Write x = (x·m̂)m̂ + sqrt(1 − (x·m̂)²)·w with w a unit vector orthogonal to m̂. If w is uniform
  (an isotropic cluster), the score is μ-like plus |q_⊥|·rad_c·T/√(d−1)-like with T Pearson II in d − 1 dimensions.
  For d = 512 and z ≤ 4 the correction z⁴/4D is 0.13: the Gaussian is adequate. If the cluster effectively lives
  in D of a few tens of dimensions (in-subspace queries), z⁴/4D is of order 1 at z = 3 to 4 and the Gaussian
  overestimates the exceedance probability by a factor of e or so.
- **Does the bound change the right order?** By Proposition 6, not if D is common to all clusters. It matters when
  the effective dimension or the tail weight differs by cluster, and for calibrated probabilities (stopping).
- **A derived shape parameter [proved identity, heuristic use].** Pearson II in dimension D has excess kurtosis
  −6/(D + 2); Student-t has 6/(ν − 4). They are one family indexed by excess kurtosis γ (formally ν = 2 − D). So a
  shape parameter can be derived from the within-cluster kurtosis of the scores: D_c = −6/γ_c − 2 if γ_c < 0,
  ν_c = 4 + 6/γ_c if γ_c > 0. The per-query kurtosis needs a fourth-order tensor; a per-cluster average over
  pseudo-query directions is one scalar per cluster. Mixture structure inside clusters (sub-clusters, a few
  near-duplicates of the query) produces positive kurtosis and skew, which is the likely situation on glove; the
  sphere alone produces negative kurtosis. The sign is an empirical question per dataset.
- **Effective dimension [proved identity].** Define 1/d_eff(q, c) = qᵀΣ_c q / tr Σ_c = (σ_c(q)/rad_c)² (for the
  component of q that is not along m_c). For a direction drawn isotropically d_eff = d. For a direction drawn from
  the cluster's own covariance, E[δᵀΣ_c δ]/|δ|² = tr(Σ_c²)/tr(Σ_c), so d_eff = PR(Σ_c) = (tr Σ_c)²/tr(Σ_c²), the
  participation ratio. With f = ψ/√d_eff and ψ ≈ 3, the measured f = 0.5 for image queries corresponds to d_eff of
  roughly 20 to 40, and f = 0.1 for text queries to d_eff ≈ d. The participation ratio is computable per cluster
  from the eigenvalues already stored; O3 does not need it because it evaluates qᵀΣ_c q directly.

---

## 8. Bounds for a fixed order

**Theorem 12 (margin identity and multiplicative guarantee) [proved].** Fix a query and a budget, let p_c be the
true densities, V* the optimal density prefix and V̂ the prefix of an order by some index p̂_c, both of the same
total weight (fill the last cluster fractionally, or accept the one-cluster rounding of Theorem 1). Let λ* be the
marginal density of V*. Then

    Rec(V*) − Rec(V̂) = (1/k) Σ_{c ∈ V* Δ V̂} n_c · |p_c − λ*| ,                                  (8.1)

and if |ln p̂_c − ln p_c| ≤ δ on the symmetric difference (after any monotone recalibration of p̂), then

    Rec(V̂) ≥ e^(−2δ) · Rec(V*) .                                                                 (8.2)

*Proof.* The two difference sets have equal weight D, so adding and subtracting λ*·D gives (8.1). For c ∈ V*∖V̂
and b ∈ V̂∖V*, p̂_c ≤ p̂_b, so p_c ≤ e^(2δ) p_b; hence g(V*∖V̂) ≤ D·max p_c ≤ e^(2δ)·D·min p_b ≤ e^(2δ) g(V̂∖V*), and
g(V̂) ≥ g(V* ∩ V̂) + e^(−2δ) g(V*∖V̂) ≥ e^(−2δ) g(V*). ∎

Translations under a Gaussian tail (ln p ≈ −ζ²/2 − ln ζ, slope −(ζ + 1/ζ)):

| error | effect on ζ_c | log-density error δ |
|---|---|---|
| ε in μ_c | ε/σ_c | ≈ ζ·ε/σ_c |
| relative error η in σ_c | ζ·η | ≈ ζ²·η |
| common error Δ in τ | Δ/σ_c | only the spread counts: ½·Δ·range over the margin of ζ_c/σ_c |
| wrong common shape, right τ | none | 0 (Proposition 6) |

So: errors in τ and in the tail shape are forgiven when the σ_c are similar; errors in σ_c are amplified by ζ²;
the gap between a cluster's mean score and the neighbours' scores, measured in units of σ_c, is exactly ζ, the
quantity that sets both the difficulty and the sensitivity.

**Distance from the oracle.** The Bayes order is optimal for its information; the clairvoyant order (by N_c/n_c)
is not attainable from summary statistics, and Proposition 8 shows the gap cannot be bounded without a model. In
the simulations the gap is 0.004 to 0.03 in mean recall when the model is right and 0.15 to 0.35 when the tails
are t₅, where one or two outlying rows decide membership and no summary statistic can see them. The "oracle" in
`loo_summary.txt` (best f per dataset) is not an upper bound; the clairvoyant order and the true-τ order are, and
both can be computed in the replay.

---

## 9. Bottom line

**Is there a theoretically optimal order?** Yes, conditionally: decreasing p_c = E[F̄_c(τ)] (Theorems 1 to 3,
Proposition 5), which under a common-shape location-scale model is the order by ζ_c = (τ − μ_c)/σ_c
(Proposition 6), with τ from Λ(τ) = k or from the running k-th best. It is practical: O(K·r·d) for the variances
(already paid by O3 and H), O(K) per bisection step for τ, O(K) per adaptive update. Without a model there is none
(Proposition 8).

**Which current proposal is best justified?**

1. The **threshold order ranked by p_c** (not n_c·p_c, for row budgets) is the optimal form. Its weakness is
   sensitivity to σ̂ (δ ≈ ζ²η); used with a raw rank-r σ̂ it can lose to a linear rule, as in the simulation. It is
   the right target once σ is calibrated.
2. **H r16** is the best justified of the measured orders: invariant under the query shift, and a shrinkage
   combination of specific and pooled spread. Its constants are compromises over budgets and stand in for a
   calibration that can be done directly.
3. **O2 with f′ = 0.4** is the best justified cheap order: invariant, no extra storage, derived from O3 under a
   shared covariance shape (6.2). A sound default where the PCA storage or the O(K·r·d) cost is unwelcome.
4. **O3 with multiplier 1** answers "what is this cluster's own k-th best score", which is the right question only
   for the first cluster; the multiplier should be about z(1, n) rather than z(k, n), constant in n_c for row
   budgets, and shrunk for the noise in σ̂.
5. **O0** violates the invariance of Theorem 11 and should be retired.

**The modification I would make.**

    σ̃_c(q)² = a·σ̂²_{c,r}(q) + b·σ(q)²·rad_c² + c₀·σ(q)²          (variance domain; a, b, c₀ by offline regression of
                                                               the exact qᵀΣ_c q on these terms over pseudo-queries,
                                                               including pseudo-queries shifted as in Theorem 11)
    τ̂ : Σ_c n_c·(1 − Φ((τ̂ − μ_c)/σ̃_c)) = k                    (bisection from max_c[μ_c + z(k,n_c)σ̃_c]; after the
                                                               first few probes replace by (4.2))
    index: ζ_c = (τ̂ − μ_c)/σ̃_c ascending;  for an nprobe budget rank by n_c(1 − Φ(ζ_c));  with overhead h by
           n_c(1 − Φ(ζ_c))/(n_c + h)
    stop:  when the best remaining 1 − Φ(ζ_c) falls below a global λ (Theorem 3), instead of a fixed budget.

The regression does the shrinkage (a least-squares slope is attenuated by exactly the reliability), replaces α and
0.3 by quantities measured on the corpus alone, and removes the dependence of the constants on the budget, since
the fan adapts ψ to the budget by itself. c₀ is the variance floor of (3.2). If the collapse test E2 below fails
(the exceedance rate is not a function of ζ alone), replace 1 − Φ by the empirical Ḡ from that test, or add the
covariates that break the collapse (the cluster's kurtosis, rad_c, n_c) to a calibrated model of the exceedance
rate: by Theorem 1 any calibrated estimate of p_c is optimal for its information.

A fallback that needs no bisection: the linear rule μ_c + ψ·σ̃_c with ψ = z(1, n̄) ≈ 3.3 constant, which is the
fan's tangent at clusters holding one expected neighbour and Weitzman-optimal for the score-sum objective.

**Experiments that discriminate.**

- **E1. Ceilings.** Clairvoyant order by N_c/n_c, and the true-τ ζ-order with exact Σ_c. They bound what any static
  and (in-model) any adaptive order can reach, and replace the best-f "oracle".
- **E2. Collapse and calibration.** Over queries and clusters, bin by ζ_c(τ_true) with exact σ_c and plot
  (rows above τ)/(rows) per bin, separately for small/large rad_c, small/large n_c, and image/text queries. If the
  curves coincide, M3 holds, the ζ-order is optimal and the curve is Ḡ (which reads off Gaussian, t or Pearson II,
  and gives the ζ at which neighbours actually live). If they do not coincide, especially on glove, no
  order built from (μ, σ) alone is optimal and the covariates that separate the curves are the ones to add.
- **E3. Fan versus linear.** Best linear multiplier per scanned share: Proposition 9 predicts that it increases with
  the share and that the true-τ fan with exact Σ is at least as good as every linear rule at every share. A
  violation on glove falsifies M3 there.
- **E4. Shape invariance.** With true τ, the Student-t order with a common ν must equal the Gaussian order
  (Proposition 6). Test t only with model τ, or with cluster-specific ν_c from the kurtosis.
- **E5. Reliability regression** (section 5): exact σ_c(q) on σ̂_{c,r}(q) and σ(q)·rad_c. Predicts H's coefficient
  ratios and the r-dependence of α, and yields a, b, c₀.
- **E6. Size bonus.** Rank by p_c against n_c·p_c, and O3 with z(k, n̄) against z(k, n_c), under row and under
  nprobe budgets. Theorem 1 predicts p_c wins for rows and n_c·p_c for nprobe.
- **E7. Global threshold.** Stop at a global λ on p_c against the same average share with a fixed per-query budget
  (Theorem 3). Expect the largest gain at small shares.
- **E8. Invariance.** Shift image queries synthetically, q′ = normalise(a·q + v) with v the mean text-minus-image
  direction and a from 1 down to 0.2. O0 should degrade as it does on text, with oracle f ∝ a; O2, O3 and H should
  not move.
- **E9. Adaptive τ.** Two-phase re-rank by (4.2) against model τ and true τ. Theorem 10 says the true-τ result is
  the most this can give.

---

## 10. Numerical sanity checks (synthetic)

`sim.py` simulates the Poisson model at score level: K = 500, n_c log-normal around 2,000, μ_c ~ N(0, 1),
σ_c = s₀·LogNormal(0, h), k = 10, 300 queries per setting, recall with whole clusters within a per-query row
budget, mean over the 14 shares from 0.5% to 25%. The policy always assumes a Gaussian with the stated (μ, σ̂).
Standard errors are 0.001 to 0.008. Outputs: `sim_out.txt`, `sim_out2.txt`.

| setting | O1 | O3 ×1 | best linear ψ | ζ model τ | ζ true τ | ζ adaptive | shrunk-σ ζ | clairvoyant |
|---|---|---|---|---|---|---|---|---|
| s₀ 1.0, h 0.35, Gaussian | 0.280 | 0.906 | 0.927 (ψ 4) | 0.928 | 0.928 | 0.928 | n/a | 0.958 |
| s₀ 0.3, h 0.25, Gaussian | 0.947 | 0.981 | 0.984 (ψ 3-4) | 0.986 | 0.986 | 0.986 | n/a | 0.990 |
| s₀ 0.3, h 0.25, Pearson II (D = 20) | 0.959 | 0.991 | 0.992 (ψ 3) | 0.992 | 0.992 | 0.992 | n/a | 0.994 |
| s₀ 0.3, h 0.25, t₅ | 0.396 | 0.468 | 0.497 (ψ 4) | 0.525 | 0.537 | 0.534 | n/a | 0.875 |
| s₀ 0.3, h 0.6, t₅ | 0.160 | 0.692 | 0.752 (ψ 4) | 0.768 | 0.777 | 0.775 | n/a | 0.916 |
| s₀ 0.3, h 0.25, Gaussian, σ̂ noise 15% | 0.943 | 0.974 | 0.976 (ψ 2) | 0.969 | 0.971 | 0.972 | 0.977 | 0.990 |
| s₀ 0.3, h 0.25, Gaussian, σ̂ noise 30% | 0.935 | 0.942 | 0.958 (ψ 2) | 0.884 | 0.917 | 0.924 | 0.959 | 0.990 |

What they support: (i) Σ n_c p_c = k to four digits and the exact Bayes index (3.1) indistinguishable from the
plug-in threshold order; (ii) the ζ-order is at least as good as every linear rule when σ is exact, including with
the wrong tail family; (iii) the best linear multiplier exceeds z(k, n) when σ is exact and falls as σ̂ gets
noisier; (iv) in-model, adaptivity is worth nothing; with wrong tails true τ is worth about 0.01 and the adaptive
rule recovers most of it; (v) with noisy σ̂ the raw ζ-order is the worst of the informed orders and the shrunk one
the best; (vi) ranking by n·p is slightly worse than by p for row budgets (0.924 against 0.928; 0.985 against
0.986); (vii) a global threshold on p beats fixed per-query budgets at small shares. The regimes are illustrative;
the ratio of between-cluster to within-cluster spread in the real data was not measured here.

---

## 11. What is proved and what is not

Proved here: Lemma 1, Theorems 1, 3, 10, 11, 12, Propositions 5 to 9, the example of 4.1, the derivation of O2
under (A1), (A2), the identities of section 7.
Standard, cited: Smith's rule (Theorem 2), optimal search for one object, Weitzman/Pandora and its matroid
extension (Theorem 4), Cantelli, submodular and adaptivity-gap factors, concentration on the sphere.
Heuristic: the size of the adaptive gain; the reading of α < 1 and of H's rad term as reliability shrinkage; the
reading of the exact-Σ multiplier 1.25 as z(1, n)/z(k, n); the claim that glove fails the common-shape assumption;
stability of κ across query types; everything quantitative in section 10 as a statement about real data.
Not verified against the real replay: all of the predictions in E1 to E9.

## References

- W. E. Smith, Various optimizers for single-stage production, Naval Research Logistics Quarterly 3 (1956).
- G. B. Dantzig, Discrete-variable extremum problems, Operations Research 5 (1957) (density bound for knapsack).
- W. L. Black, Discrete sequential search, Information and Control 8 (1965); L. D. Stone, Theory of Optimal Search
  (1975) (ratio rule p/c for one hidden object).
- M. L. Weitzman, Optimal search for the best alternative, Econometrica 47 (1979).
- R. Kleinberg, B. Waggoner, E. G. Weyl, Descending price optimally coordinates search, EC 2016; S. Singla, The
  price of information in combinatorial optimization, SODA 2018.
- J. C. Gittins, Bandit processes and dynamic allocation indices, JRSS B 41 (1979).
- G. L. Nemhauser, L. A. Wolsey, M. L. Fisher, An analysis of approximations for maximizing submodular set
  functions I, Math. Programming 14 (1978); M. Sviridenko, A note on maximizing a submodular set function subject
  to a knapsack constraint, Oper. Res. Lett. 32 (2004).
- D. Golovin, A. Krause, Adaptive submodularity, JAIR 42 (2011).
- A. Asadpour, H. Nazerzadeh, A. Saberi, Stochastic submodular maximization, WINE 2008 / Management Science 2016.
- A. Gupta, V. Nagarajan, S. Singla, Adaptivity gaps for stochastic probing: submodular and XOS functions, SODA 2017.
- D. Bradac, S. Singla, G. Zuzic, (Near) optimal adaptivity gaps for stochastic multi-value probing,
  APPROX/RANDOM 2019, arXiv:1902.01461.
- U. Feige, L. Lovász, P. Tetali, Approximating min sum set cover, Algorithmica 40 (2004); Y. Azar, I. Gamzu,
  X. Yin, Multiple intents re-ranking, STOC 2009; N. Bansal, A. Gupta, R. Krishnaswamy, A constant factor
  approximation algorithm for generalized min-sum set cover, SODA 2010.
- F. P. Cantelli (one-sided Chebyshev inequality); L. Le Cam, An approximation theorem for the Poisson binomial
  distribution, Pacific J. Math. 10 (1960); the Mecke formula, e.g. Last and Penrose, Lectures on the Poisson
  Process (2017).
- M. Ledoux, The Concentration of Measure Phenomenon (2001) (projections of the uniform measure on the sphere).
- J. Mohoney et al., Quake: adaptive indexing for vector search, OSDI 2025, arXiv:2506.03437 (adaptive partition
  scanning: partitions scanned in decreasing estimated probability, stopping on estimated recall).
- C. Li et al., Improving approximate nearest neighbor search through learned adaptive early termination,
  SIGMOD 2020; DARTH: declarative recall through early termination, arXiv:2505.19001.
