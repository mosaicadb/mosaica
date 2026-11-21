# 5. Approximate k-NN: the cluster order, the stop, statistical widths, hierarchies, visible sets

An approximate query decides every row it returns exactly: a returned row is visible and carries its exact score.
Only recall and cost are therefore at stake, and this chapter is about how to spend reads on the rows most likely to be
in the answer. It gives the theory of the cluster visit order (§5.2 to §5.6), what the replays measured (§5.7), the
order and stop the library runs (§5.8), the statistical width at the levels (§5.9), the hierarchy results (§5.10),
approximate threshold queries (§5.11), behaviour under visible sets (§5.12) and what was tried and rejected (§5.13).
Chapter 4 §4.7 gives the measured scaling law of this read in N and cluster size.

No result in this chapter has been refereed. The proofs come from four notes written by single authors, and no
referee report under `proofcheck/` or `program/` covers them, so a proved result here is at most [proved, unrefereed].
The references of these notes were collected by an agent or written from memory and were not each checked against
the paper (`visit-order/README.md`; `hierarchy/theory.md`, References), so every cited result is [cited, unchecked].

Source paths below are relative to `/home/joe/code/vexact-visitorder/bench/suite/notes/visit-order/` unless they
start with `reports/`, which is relative to `/home/joe/code/vexact-theory/`, or with `region25-order/`, which is
`/home/joe/code/vexact-sc/bench/suite/notes/visit-order/results/region25-order/`.

## 5.0 Local symbols

Chapter 0's symbols hold (n the dimension, μ_c, σ_c(q), ζ_c, rad_c, p_c, g_c, ψ, δ_q, ρ_q, ρ_{g,ℓ}, w as a cost in
rows). The chapter adds these.

| Symbol | Meaning (source spelling in brackets) |
|---|---|
| 𝒯(q), N_c | the true top-k set of the visible rows, and N_c = \|𝒯(q) ∩ c\|, so Σ_c N_c = k |
| Rec(V) | recall of a visited set V: (1/k) Σ_{c∈V} N_c |
| I | what is known at query time: μ_c, σ_c(q), n_c, rad_c, σ_μ(q) |
| σ_μ(q), σ_rad | standard deviation over clusters of μ_c, and of rad_c (σ(q), σ(rad)) |
| F_c, f_c, F̄_c | distribution, density and survival function of the scores in cluster c |
| Γ̄ | a common strictly decreasing shape, F̄_c(s) = Γ̄((s − μ_c)/σ_c) (Ḡ) |
| s | a score level, as an argument; z = (s − μ_c)/σ_c a row's standardised score |
| Λ(s) | Σ_c n_c F̄_c(s), the expected number of rows scoring at least s |
| τ*, τ̂_k | the plug-in root of Λ(τ*) = k, and the model's running estimate of τ_k (τ*, τ̂) |
| o | the overhead of probing one cluster, in rows, so w_c = n_c + o (h) |
| z(k, m) | Φ⁻¹(1 − k/m), Φ the standard normal distribution and Φ̄ = 1 − Φ |
| η | the relative error of an estimate of σ_c |
| ξ₀, ξ′ | radius weights of the orders O0 and O2 (f, f′) |
| α_H | the shrink multiplier of the order H (α) |
| b, β, v | a scale b > 0, an offset β and a direction v along which the corpus does not vary (a, β, v) |
| ψ_o | the order's multiplier δ_q + 3ρ_q (lambda in `adaptive.rs`) |
| n_c^vis | the visible rows of cluster c (v, nv) |
| â, σ̂ | a unit's estimate of a score and the spread of what the estimate leaves out (a, τ in `layered.md`) |
| ϑ, ϑ_j | the share of a unit's score variance a stage, or tree level j, explains (f, f_j) |
| λ | a price, in answers (or recall) per unit of cost |
| o_g | the cost of expanding node g (in row units, `hierarchy/theory.md`), or of opening group g (in modelled microseconds, `hierarchy/policy.md`) (e_g, a_g) |
| ν_g | the reservation value of node g |
| κ | in Proposition 5.15 the ratio with qᵀWq = κ²·qᵀBq; κ₄ a fourth cumulant; κ_q a query's calibration factor (§5.10.6); in §5.10.5 the rows of a basis per n² |
| g(S), c(S) | the expected answers found by a set of actions S, and its cost; in Proposition 5.17 a bare g is a gap counted in spreads |
| B, B̄ | a budget in rows (or in clusters) and its mean over queries; in Proposition 5.15 only, B is the covariance over clusters of the centres and W a shape matrix; chapter 0's B (bits) does not appear here |
| δ (Theorem 5.13, Proposition 5.25, §5.12) | a bound on the log-ratio of an estimated value to the true one; chapter 0's failure probability δ does not appear here |
| D | the total weight of the two difference sets (proof of Theorem 5.13); D_i the weights of a deterministic level, as in chapter 1 (Proposition 5.16); D the depth of a tree (Proposition 5.18); D_c the net value of a direct read (Lemma 5.22) |
| Q_i | the query profile of coordinate i, as in chapters 1 and 2 (Proposition 5.16; the source writes π_i) |
| L, T, U, A_j, K_j, v_j, ε, ε_j, θ_j (Proposition 5.18 only) | in model T: the number of target leaves, the target threshold, a leaf's key, a level-j node's key, the nodes at level j, the cumulative variances, the miss probability per level, the level-j noise and the level-j cut; chapter 0's L, T, U_j and ε do not apply there |
| L(z) (Theorem 5.5) | E(Z − z)₊ for a standard normal Z |
| G_c (Theorem 5.12) | E[N_c given the scores outside c]; chapter 0's group count G is written G without a subscript |
| s_u(t) (Proposition 5.27) | unit u's share of the answer; an unsubscripted s is a score level, as above |
| ρ_c′ (Proposition 5.15) | a cluster's scale in assumption (A1), proportional to rad_c |

The named orders are: **O0** μ_c + ξ₀·rad_c (the library's original radius order, ξ₀ fitted on corpus rows); **O1**
the order by μ_c alone; **O2** or `spread`, μ_c + ξ′·σ_μ(q)·rad_c/σ_rad; **O3** μ_c + σ_c(q)·z(k, n_c); **H**,
μ_c + α_H·z(k, n_c)·σ̂_{c,r}(q) + 0.3·σ_μ(q)·rad_c/σ_rad with σ̂_{c,r} from r stored directions; the **ζ-order** or
threshold order, by ζ_c ascending.

## 5.1 Setting and models

A probe of cluster c scans all of c at cost w_c. A **static** policy chooses its order from I alone; an **adaptive**
policy may also use every score seen. The models, from weakest to strongest (`theory.md` §1):

- **M0.** No model: only the realised statistics are known.
- **M1.** Given I, the score multisets of different clusters are independent, and the scores in c are i.i.d. with
  law F_c.
- **M2.** The Poisson version of M1: the scores of c form a Poisson process of intensity n_c f_c(s) ds. M1 and M2
  differ by O(Σ_c n_c F̄_c(s)²) in total variation for the counts above s (Le Cam [cited, unchecked]).
- **M3.** M1 or M2 with F̄_c(s) = Γ̄((s − μ_c)/σ_c) for one shape Γ̄.

For unit vectors rad_c² = E‖x − m_c‖² = 1 − ‖m_c‖², so rad_c carries nothing beyond ‖m_c‖.

## 5.2 The optimal static order

**Lemma 5.1 (Lemma 1 of `theory.md`).** For any static policy with visited set V = V(I) and any joint law of the
scores, E[Rec(V) | I] = (1/k) Σ_{c∈V} g_c. [proved, unrefereed] (`theory.md` §2.1)

*Proof.* Rec(V) is linear in the N_c and 1[c ∈ V] is I-measurable; take conditional expectations. ∎

The competition between neighbours through τ_k changes the variance of recall and the values g_c, which depend on
every cluster through τ_k, but it does not change which static set is best.

**Theorem 5.2 (Theorem 1 of `theory.md`; the knapsack facts cited).** Maximising expected recall over static
policies with Σ_{c∈V} w_c ≤ B is the 0/1 knapsack with values g_c and weights w_c. Let π order the clusters by
decreasing g_c/w_c and let P_j be its first j clusters. Then:

1. P_j is optimal for the budget w(P_j), also against fractional solutions;
2. for w(P_j) ≤ B < w(P_{j+1}), OPT(B) ≤ g(P_j) + (B − w(P_j))·g_{π(j+1)}/w_{π(j+1)} < g(P_j) + g_{π(j+1)}, so the
   greedy prefix loses less recall than g_{π(j+1)}/k;
3. with unit costs (a budget in clusters) the order by g_c = n_c p_c is optimal for every budget at once;
4. no single order is optimal for every row budget: sizes 2 and 3 with values 0.5 and 0.6 at budgets 2 and 3.

[proved, unrefereed; Dantzig's bound cited, unchecked] (`theory.md` §2.2)

*Proof.* Lemma 5.1 gives the knapsack. The LP relaxation is solved by filling in density order, and its value bounds
OPT from above; this is Dantzig's bound, which gives (1) and (2). (3) is the case of equal weights. ∎

With row cost and o = 0 the index is p_c, which has no size bonus: a larger cluster holds more neighbours and costs
proportionally more. With a budget in clusters it is n_c p_c, and with overhead it is n_c p_c/(n_c + o). There is no
factor 1 − 1/e, because recall is modular.

**Theorem 5.3 (Theorem 2 of `theory.md`; Smith's rule).** For a static order let C_c be the rows scanned when c is
finished. Then ∫₀^∞ (1 − E Rec(B)) dB = (1/k) Σ_c g_c C_c, the weighted completion time of single-machine scheduling,
and it is minimised exactly by decreasing g_c/w_c. [cited, unchecked: Smith 1956; the source proves it by adjacent
interchange] (`theory.md` §2.3)

**Theorem 5.4 (Theorem 3 of `theory.md`; global threshold).** Let queries come from any distribution and let a static
policy choose V(q) subject to E_q[w(V(q))] ≤ B̄. For λ ≥ 0 let V_λ(q) = {c : g_c(q)/w_c > λ}. Then V_λ maximises
E_q[Rec] among all policies whose average cost is at most E_q[w(V_λ(q))]. [proved, unrefereed] (`theory.md` §2.4)

*Proof.* For every q and every V, Σ_V (g_c − λw_c) ≤ Σ_{V_λ} (g_c − λw_c). Taking expectations,
E g(V) ≤ E g(V_λ) − λ(E w(V_λ) − E w(V)) ≤ E g(V_λ). ∎

The optimal policy spends more rows on hard queries: it stops when the best remaining p_c falls below λ, which needs
p_c calibrated across queries. Quake's adaptive partition scanning and learned early termination (LAET, DARTH) are
instances [cited, unchecked]. SPANN's query-aware pruning, which reads every list whose centre lies within (1 + ε) of
the nearest centre's distance, is a static order by centre score with a relative stop that ignores the clusters'
spread (`lit/REPORT.md` [54]) [cited, unchecked].

Minimising expected rows to reach recall r is, for k > 1, a generalised min-sum set cover with only constant-factor
greedy guarantees (Feige, Lovász and Tetali 2004 and successors) [cited, unchecked]; a system cannot observe its
recall, so the source keeps the budget and area forms (`theory.md` §2.5).

**Theorem 5.5 (Theorem 4 of `theory.md`; Weitzman's reservation index).** Under M1 with o = 0, maximise
E[sum of the k best scores found] − λ·(rows scanned) over adaptive policies. Each row is a box of cost λ with value law
F_c, and the optimal policy opens rows in decreasing reservation value r_c, defined by E_{F_c}[(S − r_c)₊] = λ, and
stops when the running k-th best reaches the next r_c. All rows of a cluster share r_c, so the policy is a cluster
order. For Gaussian F_c, r_c = μ_c + ψ_c σ_c with ψ_c = L⁻¹(λ/σ_c), L(z) = E(Z − z)₊ for Z standard normal.
[cited, unchecked: Weitzman 1979 for k = 1; Kleinberg, Waggoner and Weyl 2016 and Singla 2018 for top-k selection; the
cluster corollary is immediate] (`theory.md` §2.5)

For this objective the optimal adaptive policy has a static order of the linear form μ_c + ψσ_c with no dependence on
n_c, and adaptivity enters only through the stopping time. It is the cleanest support for an O3-shaped rule, for an
objective other than recall.

## 5.3 The index under a model

**Proposition 5.6 (Proposition 5 of `theory.md`; the exact Bayes index).** Under M2, with τ_k the k-th largest of all
scores,

    p_c = ∫ f_c(s) · Pr(Poisson(Λ(s)) ≤ k − 1) ds = E[F̄_c(τ_k)],      Σ_c n_c p_c = k.            (5.1)

[proved, unrefereed; the Mecke formula cited, unchecked] (`theory.md` §3.1; see Editor's query 3 on the sum rule)

*Proof.* By the Mecke formula E[N_c] = ∫ n_c f_c(s) Pr(fewer than k points of the superposition lie above s) ds, and
the number of points above s is Poisson(Λ(s)). That event is {τ_k ≤ s}, so the integral is n_c E∫_{τ_k}^∞ f_c =
n_c E[F̄_c(τ_k)]. The sum rule is ∫₀^∞ Pr(Poisson(λ) ≤ k − 1) dλ = k. Under M1 the Poisson count becomes a sum of
binomials, and the difference is Le Cam's error. ∎

Under M1 or M2 the order by p_c = E[F̄_c(τ_k)] is therefore the optimal static order for row cost, and the order by
n_c p_c is optimal for a budget in clusters. Two approximations of (5.1) [proved, unrefereed]: the **plug-in** τ*,
because Λ(τ_k) is Gamma(k, 1) and Pr(τ_k ≤ s) is a step of relative width 1/√k around τ*, so p_c ≈ F̄_c(τ*); τ* is
found by bisection in O(K) per step, and max_c[μ_c + σ_c z(k, n_c)] is a lower bound to start from. The **smoothed**
form: if τ_k ~ N(τ*, v²) and F_c is Gaussian, then exactly

    p_c = Φ̄((τ* − μ_c) / √(σ_c² + v²)).                                                       (5.2)

Uncertainty about τ_k acts as a variance floor added to every cluster and pulls the order towards plain μ_c. In-model
v ≈ σ̄/(ζ̄√k), by the delta method on Λ(τ_k) ~ Gamma(k, 1).

**Proposition 5.7 (Proposition 6 of `theory.md`; the order does not depend on the tail shape).** Under M3 with τ_k
given, the row-cost order is by ζ_c ascending for every strictly decreasing Γ̄. The shape changes the order only through
τ* (the root of Σ n_c Γ̄((τ − μ_c)/σ_c) = k) and through the smoothing in (5.1). [proved, unrefereed] (`theory.md`
§3.2)

*Proof.* p_c = Γ̄(ζ_c) and Γ̄ is strictly decreasing. ∎

A Student-t or Pearson II correction with one common shape parameter and the true or running τ_k gives exactly the
Gaussian order. The candidate orders form a one-parameter family indexed by τ, which the source calls the ζ-fan.

**Proposition 5.8 (Proposition 7 of `theory.md`; Cantelli).** For the empirical law of the n_c scores of cluster c with
exact mean μ_c and variance σ_c², and any t > μ_c, #{i ∈ c : s_i ≥ t} ≤ n_c / (1 + ζ_c(t)²), with ζ_c(t) =
(t − μ_c)/σ_c, and a two-point law attains it. Consequently the robust index sup Pr(S ≥ τ_k) = 1/(1 + ζ²) is again
monotone in ζ_c, and Σ_{c∉V, μ_c<t̂} min(k, n_c/(1 + ζ_c(t̂)²)) bounds the neighbours still missing, provided σ_c² is
an upper bound on qᵀΣ_c q. [proved, unrefereed; Cantelli's inequality cited, unchecked] (`theory.md` §3.3)

*Proof.* Cantelli's one-sided inequality gives Pr(X − μ ≥ a) ≤ σ²/(σ² + a²) for the empirical law; a ≥ 0 gives the
count bound, and t̂ ≤ τ_k gives the bound on the missing neighbours. ∎

The bound is useless in size: n_c = 2,000 and ζ = 3 allow 200 rows against k = 10.

**Proposition 5.9 (Proposition 8 of `theory.md`; no model, no guarantee).** Suppose two unvisited clusters a and b both
satisfy n_c/(1 + ζ_c(t₀)²) ≥ k for t₀ = max_c(μ_c + σ_c). Then two score configurations with identical (μ_c, σ_c, n_c)
for every c exist, one with all k neighbours in a and one with all k in b, so every order has recall 0 on one of them
until it has scanned both. [proved, unrefereed] (`theory.md` §3.3)

*Proof.* Put k rows of the chosen cluster at μ + σ√((n − k)/k) and its other rows at μ − σ√(k/(n − k)): the mean is μ
and the variance σ², and the top value is at least t₀ exactly when n/(1 + ζ(t₀)²) ≥ k. Put every other cluster on the
two points μ ± σ, both at most t₀. The k rows of the chosen cluster are then the top k. Any order visits one of a, b
first. ∎

So optimality from first and second moments is necessarily Bayesian. Fourth moments would tighten the count to order
n·κ₄/ζ⁴, but they need tensors no query can afford.

**Proposition 5.10 (Proposition 9 of `theory.md`; linear rules as tangents of the fan).** In the (μ, σ) plane the
level sets of the ζ-order are the lines μ + ζσ = τ_k through (τ_k, 0), and those of a linear rule μ + ψσ are parallel
lines. Therefore:

1. {c : ζ_c ≤ ζ₀} = {c : μ_c + ζ₀σ_c ≥ τ_k}: at each budget the fan selects the clusters the linear rule with ψ = ζ₀,
   the marginal ζ, selects when cut at τ_k;
2. that marginal ζ increases with the budget, so a fixed multiplier reproduces the fan at one budget only, and the two
   orders agree at every budget if and only if they never disagree on a pair, which holds when all σ_c are equal;
3. with Gaussian F_c, O3's key satisfies μ_c + σ_c z(k, n_c) ≥ τ_k ⇔ n_c F̄_c(τ_k) ≥ k, so O3 selects the clusters
   expected to hold at least k rows above τ_k; a marginal cluster expected to hold m rows sits at ζ = z(m, n_c), which
   for n_c = 2,000 is 2.58, 3.29, 3.89, 4.42 at m = 10, 1, 0.1, 0.01.

[proved, unrefereed] (`theory.md` §3.4)

*Proof.* (1) ζ_c ≤ ζ₀ ⇔ τ_k − μ_c ≤ ζ₀σ_c. (2) follows from (1) and the monotonicity of the marginal ζ in the budget.
(3) z(k, n_c) ≥ ζ_c ⇔ Φ̄(ζ_c) ≥ k/n_c. ∎

O3's dependence on n_c through z(k, n_c) is a size bonus the row-cost optimum does not have: z(10, 1,000) = 2.33 and
z(10, 4,000) = 2.81 differ by half a σ_c. A linear rule for row budgets should use a constant multiplier.

## 5.4 Static against adaptive

**Example 5.11 (`theory.md` §4.1).** Take k = 1, unit costs and a budget of 2. Cluster A's best score is 2 or 0 with
probability ½ each, B's is 1, and C's is 3 with probability 0.3 and 0 otherwise. The nearest neighbour lies in C, A, B
with probabilities 0.30, 0.35, 0.35, and the best static pair {A, B} has recall 0.70. The adaptive policy scans A; if A
shows 2, B cannot win and it scans C (recall 1), and otherwise it scans B (recall 0.7), for expected recall 0.85.
[proved, unrefereed]

Seeing scores moves the posterior on τ_k, and when the σ_c differ the best remaining cluster depends on τ_k.

**Theorem 5.12 (Theorem 10 of `theory.md`; a ceiling on adaptive policies).** Under M1, let X_{−c} be the scores of
all clusters but c and G_c(X_{−c}) = E[N_c | X_{−c}]. For every adaptive policy π with budget B,

    k · E[Rec(π)] = E[Σ_{c∈V_π} G_c(X_{−c})] ≤ E[max_{w(V)≤B} Σ_{c∈V} G_c(X_{−c})],                 (5.3)

and G_c(X_{−c}) ≤ n_c F̄_c(τ_{−c}), τ_{−c} the k-th best score outside c. [proved, unrefereed] (`theory.md` §4.2)

*Proof.* The decision to probe c is taken before any score of c is seen, so 1[c ∈ V_π] is a function of X_{−c} (a
randomised policy is a mixture of deterministic ones). Hence E[N_c 1[c ∈ V_π]] = E[1[c ∈ V_π] G_c(X_{−c})]. Each
realised V_π is feasible, which gives the inequality. A row of c in the top k beats τ_k ≥ τ_{−c}. ∎

No adaptive policy beats the knapsack that is told τ_k in advance, and the static optimum is the same knapsack with
g_c = E[G_c] in its place. The size of the gap is a [sketch: gap named]: in-model τ_k fluctuates by about
σ̄/(ζ̄√k), which reorders only clusters whose 1/σ_c differ, but no bound on the reordering's cost is proved. In the
synthetic simulation (`theory/sim.py`: K = 500, n_c log-normal around 2,000, k = 10, 300 queries a setting) the
model-τ, true-τ, adaptive and exact-Bayes orders agree to 0.001 in mean recall when the model is right; with t₅ tails
the true τ_k gains about 0.01 and the adaptive rule (5.4) below recovers most of it [model]. For monotone submodular
surrogates such as the sum of the k best scores the adaptivity gap is at most 2 and tight (Bradac, Singla and Zuzic
2019) [cited, unchecked]; recall is not a function of the visited realisations alone, so that factor is not claimed
for it.

The principled adaptive rule re-ranks by ζ_c(τ̂_k), with τ̂_k the root of

    #{found scores ≥ t} + Σ_{c∉V} n_c Γ̄((t − μ_c)/σ_c) = k,                                     (5.4)

which is at least t̂ and tends to τ_k as the clusters holding the answer are scanned (`theory.md` §4.4). The replays
and the policy work found that (5.4) undershoots on real data and that re-solving it lets a wrong table steer the stop
(§5.10.6).

## 5.5 Sensitivity to the statistics

**Theorem 5.13 (Theorem 12 of `theory.md`; margin identity and multiplicative guarantee).** Fix a query and a budget,
take o = 0, let V* be the optimal density prefix and V̂ the prefix of an order by an index p̂_c, of the same total weight
(the last cluster filled fractionally), and let λ* be the marginal density of V*. Then

    Rec(V*) − Rec(V̂) = (1/k) Σ_{c ∈ V* Δ V̂} n_c |p_c − λ*|,                                     (5.5)

and if |ln p̂_c − ln p_c| ≤ δ on the symmetric difference, after any monotone recalibration of p̂, then
Rec(V̂) ≥ e^(−2δ) Rec(V*). [proved, unrefereed] (`theory.md` §8)

*Proof.* The two difference sets have equal weight D, and p_c ≥ λ* on V*∖V̂ while p_b ≤ λ* on V̂∖V*; adding and
subtracting λ*D gives (5.5). For c ∈ V*∖V̂ and b ∈ V̂∖V*, p̂_c ≤ p̂_b, so p_c ≤ e^(2δ) p_b. Hence
g(V*∖V̂) ≤ D max p_c ≤ e^(2δ) D min p_b ≤ e^(2δ) g(V̂∖V*), and g(V̂) ≥ g(V* ∩ V̂) + e^(−2δ) g(V*∖V̂) ≥ e^(−2δ) g(V*). ∎

Under a Gaussian tail ln p ≈ −ζ²/2 − ln ζ, with slope −(ζ + 1/ζ), so an error ε_μ in μ_c gives δ ≈ ζε_μ/σ_c, a relative
error η in σ_c gives δ ≈ ζ²η, a common error Δ in τ_k counts only through its spread over the margin, ½·Δ times the
range of ζ_c/σ_c there, and a wrong common shape with the right τ_k gives δ = 0. The ζ-order therefore needs η ≲ 1/(2ζ²): about 5% at ζ = 3 and 2% at
ζ = 5 [proved, unrefereed, as a consequence of Theorem 5.13] (`theory.md` §5).

**Shrinkage** [sketch: gap named]. If ln σ̂_c = ln σ_c + noise, the posterior mean of ln σ_c is w·ln σ̂_c +
(1 − w)·(pooled ln σ), with w = Var(ln σ)/Var(ln σ̂) the reliability, and a linear rule's smaller multiplier is shrinkage of σ̂_c towards a common
value. The gap: the identification of H's α_H < 1 and its rad term with this shrinkage is an interpretation, not a
derivation (`theory.md` §5, §11). It explains the measured multipliers (exact Σ_c 1.25, r = 16 about 0.75 to 1.0,
r = 4 0.5). In `theory/sim.py` with 30% noise on σ̂ the raw ζ-order reaches 0.884 mean recall, the shrunk ζ-order 0.959
and the best linear rule 0.958 at ψ = 2 [model].

## 5.6 Invariance under a query shift

**Theorem 5.14 (Theorem 11 of `theory.md`).** Let 𝒩 be the set of directions v with Σ_c v = 0 for every c and v·m_c
equal for every c. For b > 0 and v ∈ 𝒩 let q′ = b·q + v. Every score changes by one affine map s ↦ b·s + β with
β = v·m_c, so the true top k, every N_c and the clairvoyant order are unchanged, and μ_c ↦ bμ_c + β,
σ_c(q) ↦ bσ_c(q), σ_μ(q) ↦ bσ_μ(q), τ_k ↦ bτ_k + β. Consequently the ζ-order, O1, O2, O3, H and every linear rule in
σ_c(q) are invariant, while O0 for q′ equals b·(μ_c + (ξ₀/b) rad_c) + β, the order of O0 with weight ξ₀/b.
[proved, unrefereed] (`theory.md` §6.1)

*Proof.* v·x = v·m_c + v·(x − m_c), and the variance of v·x over the rows of c is vᵀΣ_c v = 0, so v·x = v·m_c = β for
every row. The rest is substitution. Renormalising q′ is a further positive scaling. ∎

A text query against image embeddings is, to first order, of this form: a component along the modality gap plus a
scaled-down component where images vary. An order should be a function of the invariants ζ_c, σ_c(q)/σ_μ(q) and n_c.

**Proposition 5.15 (`theory.md` §6.2; O2 from O3).** Let B be the covariance over clusters of the centres, so
σ_μ(q)² = qᵀBq. Assume (A1) Σ_c = ρ_c'²·W for one shape W with ρ_c' ∝ rad_c, and (A2) qᵀWq = κ²·qᵀBq for the queries
of interest. Then σ_c(q) = κρ_c'σ_μ(q), and μ_c + ψσ_c(q) is O2 with ξ′ = ψκσ_rad/√(tr W). [proved, unrefereed,
under A1 and A2] (`theory.md` §6.2)

*Proof.* Substitute (A1) and (A2) into σ_c(q)² = qᵀΣ_c q, and use ρ_c' = rad_c/√(tr W) for unit vectors. ∎

(A2) is an assumption about queries. That one ξ′ = 0.4 is within 0.006 of the per-dataset best on all five replay
datasets is evidence that κ is stable [measured]. `theory.md` §7 adds the identity 1/n′(q, c) = qᵀΣ_c q / tr Σ_c for an
effective dimension n′, whose average over directions drawn from Σ_c is the participation ratio (tr Σ_c)²/tr(Σ_c²)
[proved, unrefereed]; with ψ ≈ 3 the fitted ξ₀ = 0.5 for images corresponds to n′ of 20 to 40 and ξ₀ = 0.1 for text to
n′ ≈ n [sketch: gap named: it assumes ψ ≈ 3 for both query kinds].

## 5.7 What the replays measured

A replay ranks the clusters of a real fit under each order, for each query, and scores recall@10 against the share of
rows scanned, reading whole clusters with exact search inside, so recall is decided by the order alone. It was checked
against the index's own approximate query within 0.001. Datasets: LAION 1M with text queries (out of distribution) and
with 1,000 held-out image queries, glove-100-angular, yahoo-minilm-384, VIBE laion-clip-512 (out of distribution);
100 queries each. Scripts in `scripts/`; tables `loo_summary.txt`, `table3.txt`, `table4.txt`, `table5.txt`
(`README.md`, Findings). All [measured]:

- **The radius order failed on text.** Text queries score about 0.3 against images where images score 0.8, so the
  spread of μ_c over clusters falls from 0.075 to 0.021 while ξ₀·rad_c does not, and the radius decides the order. At
  1.6% of rows on LAION text: O0 with ξ₀ = 0.5 fitted on corpus rows 0.48 (27 of 100 queries at recall 0), ξ₀ = 0.1
  0.81, faiss at the same cluster count 0.82, faiss with 4,096 clusters 0.93.
- **Invariant orders recover it.** Recall at 1.6% on LAION text / LAION images: O0 .481 / .931; O2 with ξ′ = 0.4
  .812 / .931; H r16 .854 / .935; the threshold order with rank-16 Σ .856 / .906; the threshold order with exact Σ
  .862 / .929. ξ′ and H's constants were chosen by leave-one-dataset-out, so they are validated, not derived.
- **The threshold order needs an accurate spread.** How τ_k is set barely matters (model, running and true within
  0.005); a Student-t correction lowers recall; with rank-16 Σ the order loses on in-distribution images, as
  Theorem 5.13 predicts.
- **The exceedance rate is not a function of ζ alone.** At equal ζ, clusters in the top rad tercile hold 1.6 to 3.8
  times more rows above τ_k than the bottom tercile on four of five datasets, so M3 fails there; H's rad term corrects
  part of it. For the same reason a spread calibrated once per index (σ̃² = a·σ̂²_r + b·σ_μ(q)²·rad_c² + c₀·σ_μ(q)²,
  ranked by μ_c + 3.3σ̃) matches H r16 on average but loses up to 0.007 on LAION images. Every order built from (μ, σ)
  stays about as far from the clairvoyant order (.97 to .99 at 1.6%).
- **A shared basis is not enough.** At LAION 1M every order pays q·m_c (256k multiply-adds); H r16 adds 4.1M and H r4
  1.0M. A shared basis of 16 to 128 global directions costs about 1/60 of H r16 but reaches only O2's recall, because it
  spans 39 to 55% of the residual variance; 2 to 4 of each cluster's own directions on top recover H r4's recall.
- **A global stop beats a fixed probe count.** Thresholding a calibrated probability across queries gains +0.017 to
  +0.043 recall at 0.5% scanned and up to +0.028 at 1.6%. A rad-aware model ln p = g(ζ) + β ln(rad_c/median rad),
  fitted by a Poisson fit on plain and shifted corpus pseudo-queries (a 65-value table), gains +0.035, +0.024, +0.015 at
  0.5, 1.6, 6.3% (mean of five datasets), and stopping each query at predicted recall r gives .932 (images) and .923
  (text) at r = 0.9, with 18% of queries below r.
- **ξ′ estimated from the corpus** came out 2 to 3 times too low.

These recommendations were superseded by the library's order (§5.8), which replaced O0, H, the calibrated tables and
query fitting.

## 5.8 The order and the stop as built

The library's approximate k-NN (`rust/src/v2/adaptive.rs` at `370580d8e`; `HANDOVER.md`; `PLAN.md`, Progress) works as
follows.

1. **Spread.** σ_c(q) comes from 16 stored int8 directions of the cluster's covariance and an isotropic rest,
   16n + 136 bytes a cluster, computed by the build by subspace iteration on up to 4,096 rows.
2. **Order.** Clusters are read in decreasing μ_c + ψ_o σ_c(q), ψ_o = δ_q + 3ρ_q. δ_q and ρ_q are the mean and
   standard deviation of z = (s − μ_c)/σ_c over the rows of every cluster of the window read directly so far (prior 0
   and 1 until two rows are scored); the small clusters of the direct batch (item 5) have no σ_c and give none. The
   order does not depend on ψ.
3. **Stream and window.** A stream keyed by μ_c + ψ_o rad_c √(qᵀC̄q / tr C̄), with C̄ = Σ_c n_c Σ_c / N the pooled
   within-cluster covariance, feeds a window of 8 clusters whose σ_c is computed (a cluster whose σ_c would cost more
   than reading it directly keeps the stand-in rad_c·√(qᵀC̄q / tr C̄) in its place). The stream is re-keyed when the rows
   behind the estimate have doubled, at most about 17 times a query. This stand-in is invariant under Theorem 5.14's
   shift, where rad_c‖q‖/√n was not.
4. **Stop.** The query stops when the window's largest μ_c + (δ_q + ψρ_q)σ_c(q) is below t̂. ψ is the caller's dial and
   ψ = ∞ is the exact query.
5. **Reads.** A cluster with at most 16 visible rows joins a batched direct read with no σ. Every other read is direct
   or on the index path, priced by constants timed at open; a group's setup is bought once the savings its directly
   read clusters forwent reach its cost.

**Why μ + 3σ.** By Theorems 5.2 and 5.4 the recall gained per row read is the per-row chance Φ̄((τ_k − μ_c)/σ_c), and the
cluster size cancels; by Proposition 5.10 μ + cσ is the threshold-free tangent of that order, and rows read are flat in
c over about 2.5 to 4 (`HANDOVER.md`). Bruch, Krishnan and Nardini (NeurIPS 2025, arXiv 2405.12207) use the same order
at the same multiplier, with no filters, stop or exact fallback [cited, unchecked]. Measured [measured]:

- **The ablation** (`results/ablation/`, three datasets, five visible sets, k = 10): the key μ_c + ψσ_c with stored
  directions beat a calibrated process (exceedance tables per spread tier, a price, hand-overs) almost everywhere and
  was the first design to beat `spread` and H unfiltered; time to recall 0.8 / 0.9 / 0.95 against `spread` was
  0.59 / 0.67 / 0.59 on LAION unfiltered. The calibrated process, H, O0 and query fitting were removed.
- **The size factor costs rows.** The order μ + z(1, n_c^vis)σ, which favours clusters with many visible rows, needs
  1.12 to 1.22 times the rows of μ + 4σ on glove unfiltered and random-25, and 1.41 to 1.53 under region-25
  (`study_order_rows_to_recall`, 100 queries). Against μ + z(1, n_c^vis)σ as the base, the free stand-in rad_c·s in
  place of σ_c needs 1.06 to 1.46 times the rows on LAION, and μ alone 1.43 to 1.83 there unfiltered, so σ earns its
  cost. The gap order (μ − t̂)/σ re-keyed at the running k-th best needs 0.64 to 1.14 of the base's rows, and the static
  μ + 3σ does as well or better, never more than 1.04 of the base (`results/order-rows/`).
- **Stress** (`study_order_stress`, rows only, 100 queries; glove, yahoo, LAION, Gaussian and Student-t(3) mixtures at
  n = 100 and 512, llama-128-ip at 257k rows and a 200k sample of yandex Text2Image): μ + 3σ is within 1.10 of the best
  order in 97% of cells outside the in-cluster bottom sets, worst 1.29 (glove, k = 1, recall 0.995). The best c falls
  with k on glove (5, 4, 4, 3 at k = 1, 10, 100, 1,000) and LAION (3.5, 3, 3, 2.5), but rows are flat in c. Its
  advantage over μ alone shrinks with k (glove 1.36 to 1.10, LAION 2.15 to 1.35). Under visible sets made of each
  cluster's lowest-scoring rows it needs 1.4 to 7.1 times the best order's rows.
- **The self-correcting order** (`results/order-self-correcting/`) fixes those sets: under the in-cluster bottom sets it
  is at most 1.29 of the best (median 1.16) against μ + 3σ's 7.14 (median 1.91), with median 1.00 elsewhere and worst
  cells 1.10 (1.14 on LAION at k = 1). Under benign sets δ_q is −0.06 to 0.04 and ρ_q 1.00 (0.86 to 1.12); under bottom
  sets δ_q is −1.2 to −2.8 and ρ_q 0.31 to 0.53; on llama ρ_q learns 0.60 to 0.63, correcting the sketch's overestimated
  σ. Dropping δ_q from the stop breaks the bottom-set fix: 156 clusters against 6 on glove.
- **Pooled, not frozen** (`region25-order/README.md`; `study_region_order`, `study_region_probe`; yahoo, LAION,
  arxiv, landmark, bioclip, iNaturalist at n = 384 to 2,048, glove and SIFT as guardrails; k = 10, 100 queries). An
  earlier build froze δ_q and ρ_q after the clusters read while fewer than k items were held, one cluster at k = 10.
  Under a region set the shift is a property of the cluster: its standard deviation is 0.31 to 0.48 in σ units and 88
  to 93% of its variance lies between the clusters of one query, so the frozen stop's multiplier ranged from 4.5 to 8.3
  at ψ = 7, and the mean rows at a given mean recall rose 1.2 to 3.3 times over the same order with a plain stop on
  five of the six datasets (bioclip, where most queries form no estimate, 0.95 to 1.19). The scale is a property of the query on LAION text (82% of its
  variance between queries). Pooling over every directly read cluster takes region-25 to 0.33 to 0.92 of the frozen
  build's rows on the six datasets (iNaturalist at recall 0.9 alone at 1.06), leaves the bottom sets unchanged and costs
  region-1 up to 18% (yahoo at 0.99). It was built in `b87f1513e`.
- **The stop is a heuristic.** A cluster not yet priced may still have a key above t̂: on synth4k 11 of 878 stops. The
  exact lazy stop priced σ for every cluster on real data (the bound is about 5 times σ) and cost about 500 µs a query on
  LAION, so it was removed (`PLAN.md`, Phase 3).

**Open second step: per-cluster visible centres.** Averaging each cut cluster's visible rows once per session makes
each cluster's shift exact, so δ_q = 0 and only ρ_q is estimated. Over pooling it needs 0.76 to 1.02 of the rows under
region-25 at recall 0.9 to 0.95 and 0.63 to 1.00 under random-1, for one pass over the visible rows a session, repaid
after 70 to 400 queries under region-25 [measured] (`region25-order/README.md`). It is not built.

## 5.9 Rule 1: the statistical width of a level

**The rule** (`hierarchy/layered.md` §4.1). The index holds, for every row of a cluster, ever finer descriptions: the
centre and spread, the key, a code at each level and the stored vector. Each gives an estimate â of the row's score and
leaves a residual of spread σ̂. A unit is read further when â + ψσ̂ ≥ t̂. For a cluster â = μ_c and σ̂ = σ_c(q). For a
row at deterministic level ℓ of group g, â is the midpoint of its certified interval and σ̂ = ρ_{g,ℓ}·h, h the certified
half-width's code term; the row passes when min(certified upper bound, â + ψσ̂) ≥ t̂. A stored vector has σ̂ = 0.

Its derivation [model]: reading a unit costs c_u per row and wins rows above the final threshold; with a normal residual
the chance is Φ̄((t̂ − â)/σ̂), so greedy recall per cost is a threshold on (t̂ − â)/σ̂, the rule with ψ = Φ̄⁻¹(λc_u). The
stages' costs differ by at most 60 times (18.5 bytes to 2n), which moves ψ by about one unit, so one ψ for the stop and
every stage is within the rule's resolution; measured, one ψ is within 0 to 4% of the best pair on glove and yahoo and
14 to 17% on LAION text. The exact queries are the same recursion with each spread replaced by a bound.

**Proposition 5.16 (`hierarchy/layered.md` (4.2); the width of a level).** A deterministic level bounds the uncoded part
by |Δ·e| ≤ ‖DΔ‖·‖D⁻¹e‖ = h. The build sets D_i⁴ = v_i/Q_i, v_i the variance of coordinate i's coding error and Q_i the
query profile, so E(D_iΔ_i)² = E(e_i/D_i)² = a_i = D_i²Q_i = √(v_iQ_i). If a row's error is not aligned with the query
and the coordinates are independent,

    ρ_{g,ℓ}² = E(Δ·e)² / E h² = Σ_i a_i² / (Σ_i a_i)²,                                            (5.6)

the inverse participation ratio of the level's per-coordinate terms: 1/n when they are equal. [model] (`hierarchy/
layered.md` §4.2)

The bound h is chapter 1's Proposition 1.8, and the weights are those of chapter 1 §1.3, D_i = (error variance_i/Q_i)^(1/4).
Chapter 1's model "Deterministic and randomised widths" (§1.3) compares Σ_i a_i with √(Σ_i a_i²) for the same terms.

*Derivation.* E(Δ·e)² = Σ_i Q_i v_i = Σ_i a_i². E‖DΔ‖² = Σ_i D_i²Q_i = Σ_i a_i and E‖D⁻¹e‖² = Σ_i v_i/D_i² = Σ_i a_i,
and E h² is taken as the product of these, (Σ_i a_i)². The gap is that product step, which assumes the two norms
uncorrelated.

Everything in (5.6) is the build's own: no query, fit or machine enters. Checked on the suite's fits (16-byte level,
derived median over 64 groups against measured rms of (score − midpoint)/half-width over 2 to 5 million rows of
100 queries, `hierarchy/layered/explore.py` on `study_layered_dump`) [measured]:

| | glove (n 100) | yahoo (n 384) | LAION text (n 512) |
|---|---|---|---|
| ρ derived (min / median / max over groups) | 0.1009 / 0.1013 / 0.1029 | 0.0592 / 0.0638 / 0.0676 | 0.0512 / 0.0561 / 0.0657 |
| ρ measured | 0.0994 | 0.0613 | 0.0485 |
| 1/√n | 0.1000 | 0.0510 | 0.0442 |
| certified half-width in units of σ_c (median) | 4.3 | 6.2 | 10.0 |
| share ϑ of the cluster's score variance the level explains | 0.81 | 0.84 | 0.72 |
| error/(ρh) at 99% / 99.9% of rows (normal 2.33 / 3.09) | 2.3 / 3.1 | 2.6 / 3.9 | 2.5 / 3.6 |

The derived ρ is within 2 to 5% of the measured one in distribution and about 15% above it for LAION's text queries,
which is the safe side because a wider spread keeps more rows. The certified half-width is 4 to 10 times the spread of the scores it must separate, which is why the
certified levels drop almost nothing at a k-NN threshold. On BioCLIP-2 (1M rows, n = 768, 50 queries) the error's
standard deviation is 1/13.6 to 1/17 of the certified half-width at levels 0 to 6, and keeping 99.8% of the true top k
needs a factor 0.4 to 0.5 on the half-width (`reports/annideas/REPORT.md`) [measured].

**Proposition 5.17 (`hierarchy/layered.md` §4.3; what a stage loses).** Under the normal model, a row whose score is
exactly t̂, in a cluster whose mean lies g spreads below t̂, passes a stage that explains a share ϑ of the cluster's
variance with probability Φ((ψ − g√(1 − ϑ))/√ϑ). [model]

*Derivation.* The estimate given the score is N(μ_c + ϑgσ_c, σ_c²ϑ(1 − ϑ)) and the residual spread is σ_c√(1 − ϑ); the
row passes when â + ψσ_c√(1 − ϑ) ≥ t̂. ∎

A stage that explains little cannot drop rows without dropping answers. At g = ψ = 4 the source gives losses of 0.7%
for ϑ = 0.8, 1.4% for ϑ = 0.72 and 19% for ϑ = 0.2 (see Editor's query 6). This is why the key is not given a
statistical width: it explains 20%, 6% and 4% of a cluster's score variance (glove, yahoo, LAION text), and applying
the rule to it needs more clusters and more bytes (glove at 0.95: 5.10 MB against 4.61) [measured].

**Measured** (`hierarchy/layered.md` §5; replay B is `study_layered_dump` with `hierarchy/layered/cascade.py`, the reader study
`study_layered_kernel` with `hierarchy/layered/kernel.sh`; k = 10, 100 queries) [measured]:

- Unfiltered, MB read per query to recall 0.9 / 0.95, direct against statistical with the measured ρ (replay B's
  headline column; see Editor's query 11): glove 28.7 / 46.5
  against 2.89 / 4.61, yahoo 10.4 / 23.3 against 1.40 / 2.54, LAION text 46.7 / 78.9 against 7.39 / 10.8, a factor of 6
  to 10; stored vectors read fall from 30k to 180k to a few hundred or thousand. The reader study confirms 5 to 10 times
  from recall 0.9 to 0.995.
- 25% visible sets: 2.3 to 7.4 times in the replay, 2.2 to 7 in the reader. Random 1% sets: exactly today's reads, since
  no cluster holds enough visible rows to pay for its tables. Region 1% sets: 1.6 to 2.6 times.
- Recall at the same ψ falls: LAION text 6.5, 3.5, 0.7, 0.2 points at ψ = 4, 5, 6, 7; yahoo 1.2, 0.7, 0.2 at ψ = 5, 6, 7;
  glove 0.2 or less. The default ψ moves up by about half a unit.
- ρ = 1/√n with no build quantity is as good on glove and 10 to 45% worse on yahoo and LAION. Two deterministic levels
  are the right depth: one costs 15 to 20% more and a third changes nothing. At k = 100 the gains are 0.85 to 0.9 of
  those at k = 10.

**As built** (`370580d8e`): each group stores ρ_{g,ℓ} per deterministic level from (5.6); the approximate k-NN sets each
level's factor on its half-width to min(1, ψρ_ℓ), ρ_ℓ the median over groups, and the randomised levels take the last
deterministic level's. Exact queries and the approximate threshold query keep certified widths. Nothing is calibrated.
The build is not yet timed.

## 5.10 Hierarchies

### 5.10.1 A beam over a tree

**Proposition 5.18 (Proposition 1 of `hierarchy/theory.md`; the tree law).** In model T a tree has levels j = 1..D with
K_j nodes, and a leaf's key is U = ε₁ + … + ε_D with independent ε_j ~ N(0, v_j − v_{j−1}) shared by the leaves under
one level-j node; a level-j node's key is A_j = ε₁ + … + ε_j and ϑ_j = v_j/v_D. The targets are the leaves with
U ≥ T = z_T√v_D, z_T = Φ⁻¹(1 − L/K). Keep at level j the nodes with A_j ≥ θ_j = √v_D (z_Tϑ_j − z_ε√(ϑ_j(1 − ϑ_j))),
z_ε = Φ⁻¹(1 − ε). Then every target's level-j ancestor is kept with probability at least 1 − ε, the expected number
kept is

    B_j = K_j · Φ̄(z_T√ϑ_j − z_ε√(1 − ϑ_j)),                                                       (5.7)

and a target survives all levels with probability at least 1 − (D − 1)ε. [proved, unrefereed, in model T]
(`hierarchy/theory.md` §2.3)

*Proof.* (A_j, U) are jointly Gaussian with covariance v_j, so A_j | U = u is N(ϑ_j u, v_Dϑ_j(1 − ϑ_j)); at u = T the
ancestor passes θ_j with probability exactly 1 − ε, and with more for u > T. Unconditionally A_j ~ N(0, v_j), and θ_j/√v_j
is the argument of (5.7). The last claim is the union bound. ∎

With K_j = K^(g_j), B_j ≈ K^(g_j − ϑ_j) L^(ϑ_j) exp(z_εz_T√(ϑ_j(1 − ϑ_j)) − z_ε²(1 − ϑ_j)/2): a node's centre understates
its best children by the variance it does not explain, and the beam is polynomial in K when a level explains less than
its share of log K. On the suite's fits (`hierarchy/theory/hier.py`) the law holds from groups to clusters within about 25% and is
optimistic by 1.5 to 4 times one level below the clusters and up to 13 times two levels below on glove and LAION text,
because the best sub-centres are outliers [measured]. One middle level scores 3.6 to 10 times fewer centres than a
flat rank at up to 5% more rows [measured]. No polylogarithmic ranking exists with a guarantee: exact top-nprobe over
centres is maximum inner-product search over K points, and a sublinear index for it would refute the Orthogonal
Vectors conjecture when n grows faster than log K [cited, unchecked, worst case]. Chapter 4's Proposition 4.2 proves
the corresponding reduction from Orthogonal Vectors for threshold queries over the rows.

### 5.10.2 The key of a node, and when to expand it

**Identity 5.19 (`hierarchy/theory.md` (3.1)).** For a node g with children c of row shares n_c/n_g and centre
M_g = Σ (n_c/n_g) m_c, Σ_g = C_g + Σ_c (n_c/n_g) Σ_c with C_g = Σ_c (n_c/n_g)(m_c − M_g)(m_c − M_g)ᵀ, so
σ_g(q)² = qᵀC_g q + mean_c σ_c(q)². [proved, unrefereed: the law of total variance]

A node taken whole is therefore one cluster with (μ_g, σ_g, n_g), Theorem 5.14 holds for every term, and a cluster's own
key is already the node key for its sub-clusters.

**Theorem 5.20 (Theorem 2 of `hierarchy/theory.md`).** Assume (S1) recall is additive over leaves and (S2) expanding
g changes the value of no leaf outside its subtree and what it reveals has a law independent of what was seen elsewhere
(true for threshold queries, or for k-NN with τ fixed at τ*). For a price λ define V_c(λ) = (g_c − λw_c)₊ for a leaf and

    V_g(λ) = max(0, n_g(p̄_g − λ)·[whole], E[Σ_{c child of g} V_c(λ) | g] − λo_g)

for a node, the middle term present when the node can be scanned whole. Among all policies that expand and probe
adaptively on revealed statistics, the expected value of k·recall − λ·cost is maximised by taking at each node the
largest term, and with one global λ this rule maximises expected recall at its expected cost. [proved, unrefereed]
(`hierarchy/theory.md` §3.2)

*Proof.* By (S1) the objective is a sum over root subtrees and by (S2) each is maximised separately. By induction on
height an expanded node's children are separate subproblems worth V_c(λ); expanding is worth their expected sum less
λo_g, scanning whole n_g(p̄_g − λ), doing nothing 0. V_g is non-increasing in λ, so expanding is a threshold on λ, and the
budget form is the argument of Theorem 5.4. ∎

For a node whose children are leaves the threshold is the reservation value ν_g, the root of
E[Σ_c (g_c − νw_c)₊] = νo_g. For the area under the recall curve the tree is a branching bandit, for which the index
policy is optimal [cited, unchecked: Weiss 1988; Bertsimas and Niño-Mora 1996]; exhaustive dynamic programming on 300
random small instances agrees on every one, while the whole-node density as index is worse on 109 of them [model].

**Corollary 5.21 (never expand a sparse node).** Expanding beats scanning whole only if E Σ_c n_c(1 − p_c/λ)₊ > o_g, and
the left side is at most n_g, so a node holding no more visible rows than its expansion costs in row units is read whole
or not at all. [proved, unrefereed] (`hierarchy/theory.md` §3.2)

*Proof.* The difference of the expand and whole terms, divided by λ, is Σ_c n_c(1 − p_c/λ)₊ − o_g. ∎

The total-variance key μ_g + 3σ_g cuts the parents to expand by 15 to 25% against the centre score on glove and LAION
text, and an explicit best-child term adds nothing; no key from the parent's moments reaches the oracle, needing 25 to
35% more parents [measured].

### 5.10.3 The multiplier does not depend on the unit's size

By Proposition 5.10 the multiplier is the marginal ζ, a property of the budget. Measured best ψ in μ + ψσ with the exact
σ, by rows to recall 0.9 at k = 10: clusters 3.0 / 4.0 / 4.0 (glove / yahoo / LAION text), 16 sub-clusters a cluster
3.0 / 3.0 / 4.0, 64 a cluster 3.5 / 3.5 / 3.0, and 2.5 to 3.5 at k = 100 [model, measured] (`hierarchy/theory.md`
§3.4). H's z(k, n_c) would give 2.6 for 2,000 rows, 1.4 for 125 and 0.5 for 31, so it must not be carried below the
cluster level; its collapse to z(10, 20) ≈ 0 under sparse visible sets was the same error.

### 5.10.4 Sub-clusters

In rows [measured] (`hierarchy/theory.md` §4.1, `hierarchy/theory/hier.py`, k = 10): 16 k-means sub-clusters a cluster cut the
rows to recall 0.9 by 2.3, 3.6 and 2.1 times (glove, yahoo, LAION text) with the `spread` key and no new state, and by
2.9 to 3.8 times with the exact spread; the clairvoyant order needs 8 to 11 times fewer. The exact spread of a
sub-cluster is worth another 1.4 to 3.4 times in rows, and borrowing the cluster's spread gets part of it on text only.
Clusters and groups opened rise by up to 1.7 and 1.6 times. On BioCLIP-2 (1M, 150 queries) the same split gave 1.9 to
3.7 times fewer items (`reports/annideas/REPORT.md`).

In time (`subclusters.md`, branch `subclusters-exp`, `ab_sub.py`, 100 queries, three datasets, five visible sets)
[measured]: only rows stored contiguously by sub-cluster (layout (c)) give a win, 0.65 to 0.82 of the library's time to
recall 0.8 to 0.95 on dense sets, but they cost the exact queries 2 to 8% unfiltered and up to 17% (k-NN, random-25),
and the approximate query loses 2 to 24% under 1% sets on LAION. A per-row label (masked) pays a pass over the cluster
that costs about what it saves; contiguous key ranges are not coherent in space and gain nothing. Sub-clusters were
shelved.

For 100M rows and more, `hierarchy/layered.md` §8.6 and §8.7 derive a design in which a 2,000-row leaf is the unit of
storage and of an approximate read, a group's basis covers a fixed κn² rows, and only routing grows with N [model]; at
1M, leaves in parents of 8 and 32 read 4 to 9 times fewer rows than parents read whole [measured]. None of it is built.

### 5.10.5 The groups

As a level the groups do not prune: with ϑ ≈ 0.6 a quarter to two fifths of the 64 groups hold the best 1% of clusters,
and as probe units they need 3 to 6 times more rows than clusters [measured] (`hierarchy/theory.md` §2.4, §0). Their
role in the approximate mode is a cost: opening a group computes V_qᵀq over its n² int8 basis (chapter 1, Lemma 1.9
and §1.4), and every cluster opened
reads every coordinate of that product (the half-width terms of every level need it), so no partial product is exact
(`groups-setup.md` §2). By operation and byte counts, not timing [model], the exact savings around it are 5 to 15% of a
group's setup at n = 384 to 512, under 2% of an unfiltered approximate query; the setups themselves, 4, 15 and 28 µs a
group at n = 128, 384 and 512, are measurements the note quotes from elsewhere. An order that charges
for opening a group opens 15 to 29% fewer groups for −5 to +6% rows [measured]. Balancing a basis's setup (n² bytes)
against what its tightness saves gives G = N/(κn²) groups; re-encodes at G = 8, 16 and 64 put κ at 1.1 (yahoo) and 0.7
(LAION) by bytes, and 8 groups were never slower than 64 at 1M [measured] (`hierarchy/layered.md` §0, §8.7). The rule
G = round(N/(0.5 n²)) was committed, held back because it gives glove 237 groups (`HANDOVER.md`), and re-applied in
`037653e01` so that it can be timed against the earlier rule; it is not yet timed.

### 5.10.6 One process for the approximate modes

`hierarchy/policy.md` specified a single planned process: each unit's value from a calibrated exceedance table, one
price λ, groups opened when the index path's savings over their clusters exceed λ times the setup, and sub-clusters
expanded by rule. Its theorems, under (C1) additivity of recall, (C2) values fixed when the plan is made and
(C3) calibration E[N_u | features] = n_u^vis P_u, are these. A set of actions is closed when every action's
prerequisites (a group's setup, a cluster's expansion) are in it.

**Lemma 5.22 (Lemma 1 of `hierarchy/policy.md`).** For every λ ≥ 0 the plan that opens group g if and only if
Σ_{c∈g}(J_c − D_c)₊ > λo_g, with D_c, I_c, X_c the net values (v − λw)₊ of reading c directly, on the index path, or
expanded, and J_c = max(I_c, X_c), maximises g(S) − λc(S) over closed sets; without the direct alternative the plans are
nested in λ. [proved, unrefereed] (`hierarchy/policy.md` §2.1)

*Proof.* The objective is a sum over groups, which share no cost; within a group each cluster contributes its best use
given whether the group is open, and opening is worth Σ max(D_c, J_c) − λo_g against Σ D_c. Without D every net value is
non-increasing in λ, so an action taken at λ is taken at any smaller price with its prerequisites. ∎

**Theorem 5.23 (Theorem B of `hierarchy/policy.md`).** Without the direct alternative, let the index of an available action be
ν_j = sup{λ : the best closed set it unlocks has positive value}. The order "always the available action of largest
index" has every plan S(λ) as a prefix, stopping when the largest index is at most λ returns S(λ), and between two plans
it is within one block's value of the optimum. [proved, unrefereed; that the order also minimises the area is cited,
unchecked: Horn 1972, Sidney 1975, and checked exhaustively on 200 random trees of 14 jobs] (`hierarchy/policy.md` §2.2)

*Proof.* An available action of index above λ belongs to S(λ) by Lemma 5.22, so everything done before the largest index
first falls to λ lies in S(λ). A member of S(λ) not yet done has its first undone ancestor available with index above λ,
so nothing of S(λ) remains at that time. For B = c(S(λ)) every closed S with c(S) ≤ B has g(S) ≤ g(S_λ) − λ(B − c(S)). ∎

**Proposition 5.24 (Proposition D of `hierarchy/policy.md`).** If n_c^vis·c_item ≤ o_c (the expansion cost), expanding never beats
reading the cluster whole, and expanding exactly when n_c^vis·c_item > o_c loses at most λo_c per expanded cluster.
[proved, unrefereed]

*Proof.* With θ = λc_item, expanding and reading the sub-clusters with P_s > θ against reading all differ by
Σ_s n_s(θ − P_s)₊ − λo_c ≤ θn_c − λo_c, and the rule's only error is when this is negative, by at least −λo_c. ∎

**Proposition 5.25 (Proposition E of `hierarchy/policy.md`).** If |ln v̂_u − ln v_u| ≤ δ for every leaf, the plan Ŝ at any price
satisfies g(Ŝ) ≥ e^(−2δ)g(S) for every closed S with c(S) ≤ c(Ŝ). [proved, unrefereed]

*Proof.* ĝ(Ŝ) − λc(Ŝ) ≥ ĝ(S) − λc(S) by Lemma 5.22 for v̂, and g ≥ e^(−δ)ĝ, ĝ ≥ e^(−δ)g on every set, so
g(Ŝ) ≥ e^(−δ)ĝ(Ŝ) ≥ e^(−δ)[e^(−δ)g(S) + λ(c(Ŝ) − c(S))] ≥ e^(−2δ)g(S). ∎

A factor common to all of a query's values leaves the order unchanged and moves the plan at a price, so a price needs
calibration across queries.

**Proposition 5.26 (Proposition F of `hierarchy/policy.md`; lazy evaluation).** A heap ordered by (key descending, id ascending)
holding exact indices or upper bounds of them, refreshing a bound when popped and taking an exact entry, takes exactly the
eager order's actions with ties broken by id. [proved, unrefereed]

*Proof.* A popped exact entry is at least every key, hence every exact index; a tied action of smaller id has a key at
least that index and is popped, refreshed and taken first. ∎

Exactness is not saving: with the rank-0 bounds ‖q‖²·(least variance) ≤ σ_c² ≤ ‖q‖²·(largest variance), 499 of 500
clusters were evaluated (`PLAN.md`, Phase 2).

The replay of the process (`hierarchy/policy_replay.md`, three datasets, 100 queries) refuted its specification in
three places (`hierarchy/policy.md` §11.1) [measured]: the cheapest spread tier, σ_μ(q) alone, gave an unusable table;
re-solving τ̂_k at epochs did worse than τ* solved once; and the table was calibrated
in the pool but wrong per query by a factor κ_q whose 10 to 90% range at k = 10 was e^(−2.5) to e^(+1.6). The revision
rests on one fact: a k-NN answer has exactly k rows.

**Proposition 5.27 (Proposition G of `hierarchy/policy.md`; shares).** Under the error model E[N_u | features, q] = κ_q·n_u P_u(τ_k),
s_u(t) = n_u P_u(t)/Σ_w n_w P_w(t) at t = τ_k is u's exact expected share of the answer, free of κ_q; at t = τ*, to first
order, ln s_u(τ*) − ln s_u(τ_k) ≈ (χ_u/χ̄ − 1) ln κ_q, with χ_u = −∂ ln P_u/∂t the unit's hazard and χ̄ its value-weighted
mean. [proved, unrefereed, to first order] (`hierarchy/policy.md` §11.2)

*Proof.* The first claim is division by the sum over units. Moving t from τ* to τ_k multiplies P_u by exp(−χ_uΔ); summing,
k/κ_q = k exp(−χ̄Δ), so Δ = ln κ_q/χ̄, and the share of u changes by exp(−(χ_u − χ̄)Δ). ∎

**Theorem 5.28 (Theorem C′ of `hierarchy/policy.md`).** If the shares are calibrated, E[N_u/A | features] = s_u with A the answer's
size, the plan at price λ on the values s_u has the largest expected recall among all rules choosing a closed set from the
same features at no greater expected cost. [proved, unrefereed] (Lemma 5.22 and Theorem 5.4 with g(S) = Σ_S s_u.)

So τ* solved once, with values as shares, is the optimal static rule for mean recall per mean cost whatever the table's
level per query; re-solving (5.4) brings the per-query factor back into the stop read by read. In the replay the true τ_k
was worst and τ* once best on all three datasets, and the share-valued plans reached recall 0.9 at 0.52 to 0.92 of
`spread`'s modelled cost [measured]; a synthetic check put re-solving at 1.08 and true τ_k with raw values at 1.34 of τ*
once [model] (`hierarchy/theory/policy_check2.py`). The process was nevertheless replaced by the plain key μ_c + ψσ_c in the
ablation of §5.8.

## 5.11 Approximate threshold queries

Recall is additive over units, so Theorems 5.2 to 5.4 and 5.20 apply with g_c = n_c F̄_c(t), and three things are
simpler (`hierarchy/theory.md` §6.1) [proved, unrefereed]: t is given, so the ζ-fan is evaluated at its true apex and,
under independent units, adapting to scanned scores gains nothing (in Theorem 5.12, E[N_c | everything outside c] = g_c);
with σ_c ∝ rad_c the key (t − μ_c)/rad_c needs no constant and is invariant under Theorem 5.14's shift; and the expected
answer size Λ(t) is known before reading. The right linear multiplier, the marginal ζ, falls as the answer grows.

Measured (`hierarchy/theory.md` §6.2, `hierarchy/theory/hier.py`, share of rows to recall 0.9) [measured]: the best ξ′ falls with
the answer size on every dataset; (t − μ)/rad is within 1% of the best ξ′ per size on glove and yahoo and within 6% on
LAION text with nothing fitted; 16 sub-clusters save 1.7 to 2.5 times at answers of 0.01% but only 1.15 to 1.26 at 10%,
where the cluster order with exact spread is within 0.3% of the clairvoyant order, so large answers belong to the exact
path. With policy values as counts the plan failed on LAION at 0.01% answers (1.86 of `spread`'s cost), and dividing by
Λ(t), floored at 1, turns them into shares [measured; the fix checked in simulation only] (`hierarchy/policy.md` §11.3).

The library's approximate threshold query reads the clusters whose μ_c + ψσ_c reaches t, after the support table drops
those wholly below t (chapter 1's Proposition 1.4); it decides first from √(least variance)·‖q‖ ≤ σ_c ≤ √(largest variance)·‖q‖ and computes σ_c
only for the undecided. On a visible set where the exact query scans first it is the exact query from the start: at 1%
visible it had cost 1.04 to 1.29 times the exact query's time to recall 0.9 and 0.95. Unfiltered, its time to recall
0.9 / 0.95 / 0.99 as a share of the exact query's is 0.25 / 0.32 / 0.54 on glove at answers of 1 in 10,000, 0.38 / 0.51
/ 0.78 on LAION and 0.20 / 0.30 / 0.51 on yahoo at 1% [measured] (`PLAN.md`, Progress, "The gap curve dropped").

## 5.12 Visible sets

**Random sets** thin each cluster: under a uniformly random visible set E[N_u^vis] = n_u^vis P_u with the same table
[model] (`hierarchy/policy.md` §4.1), and a random visible set behaves as a subsample, the same share of the visible rows in the
same share of the clusters [measured] (`hierarchy/layered.md` §0). At 1% a cluster holds about 20 visible rows at 1M,
so per-cluster work dominates: rule 1 never engages there, sub-clusters cost more in centres than they save in rows, and
small clusters go straight into the direct batch.

**Correlated sets.** Under the 1% of rows nearest one corpus row (`hierarchy/theory/hier.py` §F; a 5% set was also
run), every order from whole-cluster centres reads 35 to 48% of the visible rows for recall 0.9, because a cluster's visible rows are the ones
near the topic and q·m_c misplaces them; the clairvoyant order needs 3 to 5 times fewer rows, and the planner's scan
of the visible rows is the practical answer at that size [measured] (`hierarchy/theory.md` §7.1). Clusters holding no
visible row (38 to 78% at 1%) are skipped by count.

**Region sets** (300 regions, as the suite makes them) cut every cluster a query reaches, and the visible rows of a cut
cluster sit off its centre by a shift that belongs to the cluster (§5.8). The pooled estimate absorbs the common part of
the shift; the visible-centre step would make it exact. Under the in-cluster bottom and top sets every cluster is shifted
alike, so one estimate per query suffices.

**What shares buy.** With values as shares any miscalibration common to a query's units cancels, including the table's
drift under thinning (δ of 0.4 to 1.3 in the counts on yahoo and LAION); the part that differs by unit under correlated
sets does not [proved for the common part by Proposition 5.27; measured for the drift] (`hierarchy/policy.md` §11.3).

## 5.13 What was rejected

- **O0, the radius order**, because it is not invariant under Theorem 5.14's shift and lost up to half its recall on
  text queries (§5.7).
- **H, the calibrated spread and the calibrated exceedance process** (tables, price, group term, hand-overs), because
  the plain key μ_c + ψσ_c with stored directions beat them in the ablation (§5.8).
- **The size factor z(1, n_c^vis) in the order**, which favours large clusters and costs rows (§5.8).
- **Re-solving τ̂_k by (5.4)** as the stop, which lets a wrong table steer it (§5.10.6), and the gap order at the running
  k-th best, which is no better than μ + 3σ.
- **A frozen δ_q, ρ_q** (wrong under region sets), **the stop without δ_q** (breaks the bottom sets), **a per-cluster
  shift shrunk towards the query's** (a cluster's shift is known only once it is read), **visible centres without the
  estimate** (2.5 to 9 times the rows under bottom-25), **centres from 16 or 64 sampled visible rows**, and **waiting
  for two clusters or widening ρ_q by the between-cluster spread of the shifts** (no change under the region sets, 13
  to 57% more rows under bottom-25) (region25 study); **a shift fitted to match counts** (E2 in `PLAN.md`: worst 1.40 of
  μ + 3σ at k = 1 and 1.81 under the bottom sets).
- **The exact lazy stop** (prices every cluster, about 500 µs a query on LAION) and **a scan hand-over by rows scored**
  (doubled glove random-25's reads; reverted).
- **The key at a statistical width** (Proposition 5.17), **certified bounds against a few shared directions** (the
  remainder term prunes nothing), **sign codes of centres**, **a graph over centres**, **a second assignment of rows**
  (SOAR [53] and SPANN's copies [54] in `lit/REPORT.md`; breaks the exact layout; the spilled copies of
  `reports/surge2/approx-design.txt`, a design never run), and **a 4 or 8
  byte first level** (no gain on yahoo at 1M) (`hierarchy/layered.md` §3).
- **A level of centres over the clusters at 1M, and the groups as that level** (§5.10.5); **sub-clusters** in all three
  layouts at 1M (§5.10.4); **flat finer clusters**, which cost the exact path's per-cluster work 16 times over.

## Open

1. **A stop with a guarantee.** The window stop is a heuristic (11 of 878 stops on synth4k missed a better cluster),
   and the exact lazy stop is too costly; nothing in between is known.
2. **The size of the adaptivity gap** on real data (Theorem 5.12 bounds it; only synthetic evidence measures it).
3. **Calibrated shares on out-of-distribution queries**, and whether a learned level (`hierarchy/policy.md` (11.2)) or features
   F1 to F4 of `hierarchy/policy.md` §11.5 close part of the gap to the clairvoyant order, which every (μ, σ) order leaves.
4. **Per-cluster visible centres** (§5.8): measured in rows, not built or timed.
5. **Rule 1 in time**: the build at `370580d8e` is untimed, and the planner's constants are still timed at certified
   widths.
6. **The group count** G = N/(κn²) at 1M: re-applied in `037653e01` after being held back for glove's 237 groups, and
   not yet timed.
7. **Scale**: the leaf, basis and sketch rules of `hierarchy/layered.md` §8.6 and §8.7, extrapolated from a 16-fold range
   of N; and sub-clusters at 8,000 rows a cluster.
8. **The branching-bandit index** for nodes [cited, unchecked], supported only by an exhaustive check on small instances.
9. **The tree law beyond 38,000 leaves**, and the regression slopes of σ_c(q) on σ̂_{c,r} and σ_μ(q)·rad_c that
   `theory.md` §5 predicts (experiment E5, not run).
10. [conjecture] A sub-centre stored as one more 20-byte coded row would not rank sub-centres, since the code's error
    (sd 0.011 on BioCLIP-2) is as large as the spread of q·(s − m_c) on LAION text (rms 0.013) (`hierarchy/theory.md`
    §5.2).

## Editor's queries

1. **"The theory of the order" in `PLAN.md`.** `hierarchy/layered.md` §4.1 and the brief cite a section of that name;
   no section or phrase of that name exists in `PLAN.md` at the visit-order worktree's head or in its history. The
   argument (per-row chance Φ̄((t* − μ)/σ), cluster size cancels, μ + cσ its threshold-free form) is in `HANDOVER.md`
   and follows from Theorems 5.2 and 5.4 and Proposition 5.10; §5.8 cites those.
2. **Tag conversion.** The sources use [heuristic], [estimated], [derived], [checked] and [proved in the model], which
   chapter 0 does not have. I mapped heuristic arguments to [sketch: gap named] with the gap stated, synthetic
   simulations and exhaustive checks to [model], derivations of `layered.md` to [model], estimated times to [model]
   beside [measured] counts, and "proved in the model" to [proved, unrefereed] with the model in the statement. The PI
   may prefer another mapping.
3. **The sum rule of Proposition 5.6.** Under M2 with finite total intensity N, Λ runs from 0 to N, so
   Σ_c n_c p_c = ∫₀^N Pr(Poisson(λ) ≤ k − 1) dλ = k − Σ_{j<k} Pr(Gamma(j + 1) > N), which is k only up to a term
   exponentially small in N; τ_k is also undefined when fewer than k points occur. The source states the rule as exact.
   The status is kept.
4. **Theorem 5.13 and Proposition 5.9 need o = 0 and, in 5.9, an even n_c** for the two-point laws μ ± σ; the source
   leaves both implicit.
5. **Proposition 5.16's product step** E h² ≈ (Σ_i a_i)² treats ‖DΔ‖ and ‖D⁻¹e‖ as uncorrelated; the source labels the
   result derived and checks it against measurement. Also, `layered.md` §0 writes ρ ≈ 1/√n, but its own yahoo value
   (0.0638 against 1/√384 = 0.051) is 25% above that.
6. **The numbers of Proposition 5.17.** Evaluating Φ((ψ − g√(1 − ϑ))/√ϑ) at g = ψ = 4 gives losses of 0.67%, 1.3% and
   17.3% for ϑ = 0.8, 0.72 and 0.2; the source gives 0.7%, 1.4% and 19%.
7. **What the built rule 1 reads.** `hierarchy/layered.md` §4.2 says the approximate path can skip the randomised
   levels; the commit `370580d8e` reads them at the last deterministic level's ρ and uses one median ρ per level over
   the groups where the derivation gives one per group.
8. **Two readings of τ_k that look opposed.** `README.md` finding 4 says model, running and true τ_k give orders
   within 0.005; `hierarchy/policy.md` §11.1 says true τ_k was worst and τ* once best. The first is about the order at a fixed
   probe count, the second about the stop at a price on counts, so they need not conflict; the sources do not say so.
9. **Superseded recommendations.** `hierarchy/theory.md` §8 recommends masked sub-clusters (labels) and a group-aware
   order; `subclusters.md` measured masked sub-clusters as no gain and `hierarchy/layered.md` §3 rejects them, and the
   policy replay found the group term small at the measured setup costs. This chapter reports the later finding.
10. **Citation of approx-design.txt's bound** "expected recall ≥ 1 − Lδ(m), proved, L0" refers to a report outside this
    chapter's sources (`surge/level0/REPORT.txt`) and was not checked here. Chapter 3 §3.5 transcribes that report's
    k-NN results (Theorem 3.10).
11. **Which ρ produced rule 1's headline bytes.** `hierarchy/layered.md` §5.2 labels the column behind glove 2.89 /
    4.61, yahoo 1.40 / 2.54 and LAION text 7.39 / 10.83 MB "statistical, ρ measured", while §0 quotes the same numbers
    as evidence for the ρ derived from the build. The two ρ differ by 2 to 5% in distribution and about 15% on LAION
    text, so the derived rule's bytes are not stated anywhere in replay B; the reader study (§5.4) used one ρ per level
    and does not say which. The chapter reports the column as labelled.
12. **How close one ξ′ = 0.4 is to the per-dataset best.** `theory.md` §6.2 says within 0.006 on all datasets;
    `README.md` finding 3 gives a worst case of +0.007 at 6.3% scanned. The chapter keeps `theory.md`'s figure.
13. **The group setup times** (4, 15 and 28 µs at n = 128, 384 and 512) are quoted by `groups-setup.md` from a source
    it does not name ("the ones I was given"); they are reported as measured but were not traced.
