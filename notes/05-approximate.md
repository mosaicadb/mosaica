# 5. Approximate k-NN: the cluster order, the stop, statistical widths, hierarchies, visible sets

An approximate query decides every row it returns exactly: a returned row is visible and carries its exact score.
Only recall and cost are therefore at stake, and this chapter is about how to spend reads on the rows most likely to be
in the answer. It gives the theory of the cluster visit order (§5.2 to §5.6), what the replays measured (§5.7), the
order and stop the library runs (§5.8), the stop on expected misses that was dropped and the stop against a fixed
cluster count (§5.8a), the statistical width at the levels (§5.9) and the measured width that replaces it (§5.9a), the
hierarchy results (§5.10), among them the tree read as a random energy model and its leaf floors (§5.10.1a),
approximate threshold queries (§5.11), behaviour under visible sets (§5.12) and what was tried and rejected (§5.13).
Chapter 4 §4.7 gives the measured scaling law of this read in N and cluster size.

Most results in this chapter have not been refereed. The proofs of §5.1 to §5.12, outside §5.8a, §5.9a and §5.10.1a,
come from four notes written by single authors, and no referee report covers them, so a proved result there is at
most [proved, unrefereed]. Sections 5.8a, 5.9a and 5.10.1a come from the approximate track of the theory program
(`program/approximate/CLAIMS.md`), which has two referee reports: `program/approximate/referee/referee-1.md` on A1 to
A3 and `referee-2.md` on A4. The track revised A1 to A3 after the first report and nobody re-read the revision;
nothing was revised after the second, and its verdicts are applied here. Each tag in those sections therefore says
which text a referee read. The references of the four notes were collected by an agent or written from memory and
were not each checked against the paper (`visit-order/README.md`; `hierarchy/theory.md`, References), so every result
cited from them is [cited, unchecked]; §5.10.1a cites one paper whose statement was read.

Source paths below are relative to `/home/joe/code/vexact-visitorder/bench/suite/notes/visit-order/` unless they
start with `reports/` or `bench/`, which are relative to `/home/joe/code/vexact-theory/`, with `program/`, which is
relative to `/home/joe/code/vexact-theory/notes/`, or with `region25-order/`, which is
`/home/joe/code/vexact-sc/bench/suite/notes/visit-order/results/region25-order/`. The visit-order session's dumps and
logs are named by absolute path.

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
| L, T, U, A_j, K_j, v_j, ε, ε_j, θ_j (Proposition 5.18 and §5.10.1a only) | in model T: the number of target leaves, the target threshold, a leaf's key, a level-j node's key, the nodes at level j, the cumulative variances, the miss probability per level, the level-j noise and the level-j cut; chapter 0's L, T, U_j and ε do not apply there. The track writes Y_j for A_j, f_j for ϑ_j, v for v_D and M for K_D. Chapter 0's A_j, the stored error scale, is used in §5.9a |
| L(z) (Theorem 5.5) | E(Z − z)₊ for a standard normal Z |
| G_c (Theorem 5.12) | E[N_c given the scores outside c]; chapter 0's group count G is written G without a subscript |
| s_u(t) (Proposition 5.27) | unit u's share of the answer; an unsubscripted s is a score level, as above |
| ρ_c′ (Proposition 5.15) | a cluster's scale in assumption (A1), proportional to rad_c |
| ent_j (§5.10.1a) | ln K_j / ln K_D, the entropy position of tree level j (g_j) |
| Π_step, Π, Π̂ (§5.10.1a) | the step profile that jumps by ϑ_j − ϑ_{j−1} at ent_j (A_step); the piecewise-linear profile through the points (ent_j, ϑ_j) of the branching levels (A); the concave hull of those points (Â). Π′ is the derivative of Π |
| x_*, x_s (§5.10.1a) | √(2 log 2)·∫₀¹ √Π′ and √(2 log 2)·∫₀¹ √Π̂′: the algorithmic level and the ground state of a continuous random energy model, per unit of binary depth |
| N′ (§5.10.1a) | the depth of a binary tree in the cited theorem (N there) |
| 𝒢, 𝒬 (§5.10.1a) | the σ-field of the internal node values, and the set of leaves an algorithm queries (Q) |
| p_i, p′_i, τ_{−i} (§5.10.1a) | leaf i's chance of being a target given 𝒢; its chance of being among the k best given 𝒢 and the other leaves; the k-th largest value among the leaves other than i, siblings included |
| r, θ_r, B_leaf(r) (§5.10.1a) | a target recall; the parent's cut and the leaf floor at recall r (a_r and R(r)) |
| e, σ̂, ϑ (§5.9a) | a row's error at a deterministic level, e = s − â, with â the level's midpoint (a); the spread of e over a cluster's rows (τ); ϑ = 1 − σ̂²/σ_c², the share of the score variance the level explains (f) |
| e_vec, Δ̃, w, Δ̂, ẽ, ê, u, C_c, S_c, κ (Proposition 5.29) | the coding error as a vector; DΔ; w = ‖Δ̃‖, the query's factor of h (not a cost in rows there); Δ̃/w; D⁻¹e_vec; ẽ/u; u = ‖ẽ‖, the row's stored norm; the means of ê·êᵀ and of ẽ·ẽᵀ over the rows of c; in the proof, a row's error scale common to its coordinates |
| z, ρ_es, ρ_ae, σ_a (§5.9a) | z = (s − μ_c)/σ_c as above (x); the correlation of e with s over a cluster's rows; that of â with e; the spread of â |
| m (§5.9a) | the neighbours' slope of e/σ̂ on z. Chapter 0's bare m, a fixed cluster's centre, is always written m_c in this chapter |
| ψ0, Y, n_s (§5.9a) | the multiplier of the scatter (not ψ_o, the order's multiplier); a width: a row is kept when â + Y ≥ t (y); the rows of a per-cluster sample (m) |
| X_q, χ_X, a_G, e_G, κ_Δ, s_res, c0 (§5.9a) | in the track's model of a shared component: the part of a row's score it shares with the query (R), with an exponential upper tail of rate χ_X (λ_R); the coded and left-out parts of a typical residual's score; the share of the query offset's weight a level leaves out (κ_q, not §5.10.6's κ_q); the spread of the scatter about the model's line; the model's intercept far up |
| â_R, v_R (Proposition 5.32) | a row's estimate at a randomised level R at which every block is coded (a_R), and its error variance over the seed given the stored scalars |
| Vis, Σ_{c,left} (Lemma 5.31) | the event that a row is visible; the cross-covariance of a row with its coding error over cluster c (Λ_left) |
| ν² (§5.9a) | the variance, in units of the scatter, of a width's error from a sample |
| β_A, c_A, r_A, g, s_g, β_e, c₁, c₂ (§5.9a) | the share of the weighted error in r_A directions of a cluster's own, and c_A² that of the query's weighted offset (β, c², r); a neighbour's scatter about the line m·z·σ̂, in units of √ϑ·σ̂, of unit spread; the scatter's spread in units of σ̂, as it enters sd(e/σ̂ \| z) (s); Cov(e, s)/Var(s) (β); the coefficients of ν² |
| var_i, dist_i, left_i, PR (§5.9a.7) | a coordinate's variance in the group's basis, its distortion at the level, the variance the level leaves of it (Λ, D, Λ_left); the participation ratio (Σx)²/Σx² |

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

## 5.8a The stop on expected misses, and the stop against a fixed cluster count

**The stop on expected misses was dropped.** The approximate track studied a stop that predicts the true neighbours
left in the unread clusters, Σ_{c unread} n_c^vis·Ĝ(ζ_c(t̂)) for an estimated tail Ĝ of the standardised
within-cluster scores, and stops when that prediction falls under (1 − r)·k (`program/approximate/CLAIMS.md` A2). Its
guarantee is weak where it holds: with the tail known exactly the stop bounds the mean recall of the queries that
share a set of statistics, and in the model at r = 0.9 still 7% (normal scores) and 23% (Student-t, 5 degrees) of the
queries fall below r [A2.1 proved, unrefereed: revised after referee 1, which found no error in items 1 to 3; the
figures model, `program/approximate/referee/ref1_a2.py`]. Without the tail there is no guarantee to have, because a
stop fed by the pooled sample that is valid for every shape needs about a tenth as many pooled rows as rows left unread
for one expected miss [A2.2 proved, unrefereed, for a one-shot decision: revised after referee 1's two repairs; the
sequential form a sketch]. Measured, the within-cluster tail is close to exponential (−d ln P/dz of 1.97 to 2.77
between 3 and 6 spreads, against 4.7 for the normal), a tail estimated per query is off by a factor of 2 to 4 at the
decision, and the stop selects the reads at which its estimate is low; in the idealised replay no stop on expected
misses read fewer rows than the built stop on yahoo or LAION beyond noise, and no tail made the setting a recall
[measured] (A2.3 to A2.6; `program/approximate/scripts/replay_np.py`). The proofs and tables belong to a later chapter
of dropped work (Editor's query 22).

**The idealised replay against a fixed count** [measured] (`program/approximate/CLAIMS.md` A2.6, items 1 and 4;
`program/approximate/scripts/replay_np.py`, tables T7 and T8 of `program/approximate/out/tables.md`). Whole clusters were
read in the order μ_c + 3σ_c with exact σ_c, on yahoo, LAION with text queries and glove, under no filter, random-25,
region-25, region-1 and random-1, with k = 10 and 100 queries, so differences under about 10% are noise. The stop was
the built one in an idealised form: every unread cluster priced, and δ_q and ρ_q pooled over every row read. Reading
the same number of clusters for every query took 0.95 to 1.65 (yahoo), 0.77 to 1.44 (LAION) and 0.68 to 1.29 (glove)
of the stop's rows at equal mean recall, over recall 0.9 to 0.99 and the five sets. Unfiltered it took 0.77 and 0.81 of
the stop's rows on LAION at recall 0.98 and 0.99, and 0.68 to 0.81 on glove. The track read this as evidence that the
pooled (δ_q, ρ_q) is not a calibrated p_c in the sense of Theorem 5.4.

**On the library's reader** [measured, by the visit-order session; logs in
/home/joe/code/vexact-data/sc-study/out-fixed2/] (`program/LOG-2026-11-21.md`, "The fixed-cluster-count replay on the real
reader"). The session reran the comparison on the library's own stream, 100 queries, k = 10. The figures are the
fixed count's rows over the stop's at recall 0.9 / 0.95 / 0.98 / 0.99, so a figure below 1 means the fixed count reads
fewer.

- The pooled stop (`b87f1513e`), unfiltered, wins on LAION (1.07 / 1.05 / 1.11 / 1.06) and on yahoo, landmark and
  BioCLIP (1.42 to 1.87). The fixed count wins on arxiv (0.82 / 0.83 / 0.75 / 0.73), iNaturalist, glove (0.82 to
  0.89) and SIFT.
- The older stop (`f528836`) loses to the fixed count on LAION (0.60 to 0.82), which is what the idealised replay saw.
- Under filters the pooled stop wins nearly everywhere (1% sets 1.03 to 1.4; BioCLIP 1.2 to 2.4). The exceptions are
  iNaturalist under region-25, SIFT at 25% and arxiv under random-25 at high recall.

The session's reading is that the per-query stop costs 5 to 25% against a fixed count on four datasets unfiltered and
wins by more elsewhere and under filters, so there is room to improve the stop at high recall on those four and no
case for a fixed count. It follows that A2.6's statement for LAION does not hold for the pooled stop as built, while
its statement for glove does (Editor's queries 14, 15 and 23).

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
The build is not yet timed. The derived ρ turned out too narrow with few groups for the dimension, and a width of ψ
spreads loses neighbours even when the spread is right; the visit-order session's build has since taken the measured
rule of §5.9a (`program/LOG-2026-11-21.md`; no commit is recorded).

## 5.9a The level width, measured

Rule 1 sets a level's width from the build alone. The visit-order session measured it on per-row dumps and found two
faults: the derived spread is too narrow when there are few groups for the dimension, and even with the right spread
a width of ψ spreads loses about a fifth of the true neighbours, because a neighbour's error is not a typical row's.
The rule that replaced it is measured and fitted, and it holds under a condition stated below; it is not derived. This
section gives the measurements, the rule, and the results of the approximate track that bear on it
(`program/approximate/CLAIMS.md` A4, refereed in `program/approximate/referee/referee-2.md`).

Unless a bullet says otherwise the measurements are [measured, by the visit-order session], on the per-row dumps in
/home/joe/code/vexact-data/suite/rho-dump/ with the scripts `hierarchy/layered/rho_check.py`, `psi_eff.py`,
`kappa_check.py`, `pair_check.py` and `rand_sample.py` (`program/LOG-2026-11-21.md`, the three sections on the level width;
`program/approximate/CLAIMS.md` A4.0, as relayed by the PI). The five indexes are iNaturalist (n = 2,048) at 12 groups
and at 1 group, yahoo (n = 384) at 64 groups, and LAION with text queries (n = 512) at 64 and at 8 groups; the first
30 clusters a query; k = 10; the true 10th best as the threshold t; the 16-byte level unless said.

### 5.9a.1 Why the derived width is narrow with few groups

The rms of the actual error over the derived ρ·h, as a median over the clusters holding a neighbour, is 1.29 on
iNaturalist at 12 groups and 2.04 at 1 group, and 1.03, 0.94 and 1.04 on yahoo and LAION at 64 and 8 groups
[measured, by the visit-order session; `rho_check.py`]. So the derived width is 1.29 to 2.04 times too narrow on
iNaturalist and right within 6% on yahoo and LAION. The record does not say whether this is the mean of the ratio or
the ratio of the means, but in A3's model the two agree within 3% [model] (`program/approximate/referee/ref2_a41.txt`),
so the factors stand (Editor's query 19).

**Proposition 5.29 (A4.1; the error over the half-width is a cosine).** In a group's dual coordinates let Δ̃ = DΔ,
w = ‖Δ̃‖, Δ̂ = Δ̃/w, ẽ = D⁻¹e_vec, u = ‖ẽ‖, ê = ẽ/u and h = w·u. Then for every row and query (Δ·e_vec)/h = Δ̂·ê. For a
fixed query the mean over cluster c's rows of the squared ratio is Δ̂ᵀC_cΔ̂, and the ratio of the second moments,
Σ(Δ·e_vec)²/Σh², is Δ̂ᵀS_cΔ̂/tr S_c. The derived ρ_{g,ℓ}² of (5.6) is the second with S_c replaced by the diagonal
matrix of the a_i and Δ̂ by its profile value. [proved, refereed] (`program/approximate/CLAIMS.md` A4.1;
`referee-2.md` §1)

*Proof.* Δ·e_vec = (DΔ)·(D⁻¹e_vec) = w·u·(Δ̂·ê). Square and average over the rows with Δ̂ fixed; Σh² = w²·n_c·tr S_c.
Under the diagonal error model e_i = κ√v_i·ξ_i with uncorrelated ξ of unit variance, v_i as in Proposition 5.16,
S_c = κ²·diag(a_i) because v_i/D_i² = a_i, which gives (5.6) on the profile. ∎

Referee 2 adds two hypotheses the source leaves implicit: D is diagonal in coordinates in which the score is the plain
pairing Σ Δ_i e_i, and h is the code term w·u alone with u exact (if u is stored rounded up, the ratio is below the
cosine). The proposition says what the derived width leaves out: it captures the query's weights but not its alignment
with the cluster's own errors. The track names what is left out as the off-diagonal part of the cluster's error
covariance in the group's basis, the departure of its diagonal from the group's, and its mean (A4.1); referee 2 notes
that this compares numerators only, and is what A3 leaves out only if the scale is defined by tr S_c = κ²·Σ_i a_i
(`referee-2.md` §1 item 2).
In a model where a share β_A of the weighted error lies in r_A directions of the cluster's own, and a share c_A² of
the query's weighted offset lies in them, (actual over derived)² ≈ (1 − β_A) + β_A·c_A²/(ρ_{g,ℓ}²·r_A), and in a
constructed group the ratio for a cluster's own queries rises from 1.00 at one cluster a group to 2.1 to 2.7 at 32
[model] (`program/approximate/CLAIMS.md` A4.2; `out/a4_model.txt` there). The model gives the measured direction,
larger with fewer groups and larger n, but not iNaturalist's numbers, whose N, K and bits it was never given.

### 5.9a.2 A neighbour's error is not a typical row's

A row with s ≥ t is kept by a width Y if and only if e ≤ Y + (s − t), since â = s − e [proved, refereed]
(`program/approximate/CLAIMS.md` A4.3 item 1). So the share of neighbours a rule keeps is a statement about the law of
e among neighbours.

Measured with the spread σ̂ taken per cluster from a sample of rows scored exactly (the actual rms error over it has a
median of 1.01 to 1.03 on every index, and a 10% to 90% range of 0.83 to 1.29 at 32 rows and 0.88 to 1.18 at 64), the
plain rule Y = ψσ̂ keeps 0.790 to 0.824 of the true ten at ψ = 4 and 0.88 to 0.93 at ψ = 5 on all five indexes. A
neighbour's e/σ̂ follows its height z: the slope through the origin is 1.21, 1.24, 0.99, 0.92 and 0.98 (iNaturalist at
12 and 1 groups, yahoo, LAION at 64 and 8) with correlation 0.68 to 0.71, the median e/σ̂ is about 3, and 0.24 to 0.35
of the neighbours lie beyond 4σ̂ [measured, by the visit-order session; `pair_check.py`].

**Proposition 5.30 (A4.3 items 2 and 4; the linear regression).** Let the regression of e on s over a cluster's visible
rows be linear with a normal residual of constant spread (joint normality suffices), let μ_c, σ_c and σ̂ be those rows'
exact statistics, and let t be fixed. Then

    E[e | s] = ρ_es·z·σ̂,      sd[e | s] = √(1 − ρ_es²)·σ̂,      ρ_es = √(1 − ϑ) + ρ_ae·σ_a/σ_c,

a row at the threshold is kept by the plain rule with probability Φ((ψ − ρ_es·ζ_c(t))/√(1 − ρ_es²)), and the slope of
e/σ̂ on z is ρ_es, which is at most 1. It is √(1 − ϑ) exactly when the midpoint is uncorrelated with its error, and 1
exactly when e is a linear function of s. [proved, refereed] (Editor's query 26) (`program/approximate/CLAIMS.md` A4.3; `referee-2.md` §2)

*Proof.* Since s = â + e, Cov(e, s) = σ̂² + Cov(â, e), so ρ_es = Cov(e, s)/(σ̂σ_c) = σ̂/σ_c + ρ_ae·σ_a/σ_c, and
σ̂/σ_c = √(1 − ϑ). The regression of e on s has slope Cov(e, s)/σ_c² and residual spread √(1 − ρ_es²)·σ̂, which gives
the first two formulas. A row at s = t is kept when e ≤ ψσ̂, and e given s = t is normal with mean ρ_es·ζ_c(t)·σ̂ and
that spread. A correlation is at most 1 in modulus, with equality exactly when e is a linear function of s. ∎

Referee 2 adds that E[e] = 0 over the cluster's visible rows is assumed; otherwise the mean is added. With â and e
uncorrelated, the measured ϑ of 0.67 to 0.92 and neighbours at ζ = 3 to 4, the proposition predicts at least 0.98
kept at ψ = 4 and a slope of 0.28 to 0.57, against the measured 0.79 to 0.82 and 0.92 to 1.24. So the uncorrelated
normal account of the loss, which is Proposition 5.17's model, is [refuted] on these data (`referee-2.md` §2 item 3).

A slope above 1 would show only that E[e | s] is not linear, and only if e/σ̂ was centred and z and σ̂ were the
cluster's own. Neither holds for the record: e/σ̂ was not centred, and z used the mean and spread of the same 64-row
sample as σ̂ (`program/LOG-2026-11-21.md`, "answers to referee 2"). The slope through the origin of an uncentred e/σ̂ is
ρ_es + ē/(σ̂·z), so a mean error of 0.3σ̂ adds 0.1 at z = 3 [proved, unrefereed: referee 2's remark]. The source's further
claim that a slope above the bulk's needs a component with a heavier upper tail is one sufficient mechanism, not a
necessary one: any E[e | s] convex in the upper tail does it (`referee-2.md` §2 item 5).

### 5.9a.3 The rule

**The rule** [measured; a fitted rule, with the condition below]. Keep a row at a deterministic level when

    â ≥ t̂ − (m·max(ζ_c(t̂), 0) + ψ0·√ϑ)·σ̂,                                                (5.8)

with ζ_c(t̂) and ϑ from the whole cluster's μ_c and the library's σ_c(q), σ̂ the spread of e over a sample of n_s = 64
visible rows of the cluster scored exactly, and the sample's mean of e − m·σ̂·(s − μ_c)/σ_c(q) added to every midpoint.
The build uses m = 1 (`program/LOG-2026-11-21.md`, last section; Editor's queries 16 and 18). When t̂ is below the cluster's
mean the width is ψ0·√ϑ·σ̂, the plain rule at ψ = ψ0·√ϑ.

*Where it comes from.* Suppose a neighbour at height z has e = m·z·σ̂ + √ϑ·σ̂·g, with m its slope and g a scatter of
unit spread. A neighbour at the threshold is kept by the width (ζ + ψ0·√ϑ)·σ̂, which is (5.8) with the coefficient 1
on the height, if and only if g ≤ ψ0 − (m − 1)·ζ/√ϑ, and one above it gains (z − ζ)·(σ_c − m·σ̂) of slack [proved,
refereed, as algebra on this law] (`program/approximate/CLAIMS.md` A4.5 item 1; `referee-2.md` §4). So when the
slope m is 1, the value the build assumes, the dial ψ0 sets the chance at the threshold whatever ϑ and ζ are, provided
g is centred, of unit spread, and of a law that does not depend on the height. The last fails on the data (§5.9a.5).
The
law was measured on a cluster's top rows only, since a cluster's neighbours are its rows above t, at most ten, and it
does not extend to the bulk: over a whole cluster it would give Var(e) ≈ (m² + ϑ)·σ̂² [model] (`referee-2.md` §4
item 1).

*The condition: m near 1, fitted and not derived.* The track's model of a component shared with the query
(`program/approximate/CLAIMS.md` A4.4) writes s − μ_c = X_q + a_G + e_G, â − μ_c = (1 − κ_Δ)·X_q + a_G and
e = κ_Δ·X_q + e_G, with X_q ≥ 0 of exponential upper tail at rate χ_X and a_G, e_G independent normal [model]. Its
algebra checks, but as run it gives a median e/σ̂ over z of 0.74, 0.80, 0.67, 0.74 and 0.78 for neighbours at z
between 3 and 4, where 0.89 to 1.21 was measured. It matched the headline figure, a median e/σ̂ of about 3, by two
errors that cancel: its neighbours sit a spread higher than the data's (ζ = 4.1 against about 3) and its ratio is 12
to 27% lower. Far up its regression is a line with a negative intercept, E[e | s] → κ_Δ·(s − μ_c) + c0 with
c0 = ((1 − κ_Δ)·Var(e_G) − κ_Δ·Var(a_G))·χ_X, which is −0.65σ̂ to −1.81σ̂ in the five settings, and at the stated tail
rate it cannot reach iNaturalist's 1.2 [model] (`referee-2.md` §3, `program/approximate/referee/ref2_a44.txt`).
Referee 2's verdict on the model is a gap: its algebra is right, and as run it does not reproduce the figure the width
rests on. The track's summary that the model fits the measurement "in kind, not to the point" (A4.9) is, in the
referee's words, too kind, since a ray through the origin with slope near 1 and a centred scatter, which the width
needs, is what the model lacks (`referee-2.md` §3, item 1 and the closing paragraph). The slope
was also measured with the sample's spread and an uncentred error (§5.9a.2), so part of iNaturalist's 1.2 belongs to
which spread was used: when only the source of the height changed from the sample to the library's whole cluster,
iNaturalist's miss at ψ0 = 2 fell from 0.082 to 0.049 [measured, by the visit-order session; `width_checks.py
--whole`] (`program/LOG-2026-11-21.md`, "The level width under filters"). A slope fitted in one
frame and applied in the other would correct twice (`referee-2.md` §7).

**Measurements of the rule** [measured, by the visit-order session].

- *With every quantity from the sample* (the first form, Y = (ζ + ψ0·√ϑ)·σ̂ with ζ, ϑ and σ̂ all sampled): unfiltered,
  at ψ0 = 2 / 3 it keeps 0.918 / 0.975, 0.918 / 0.972, 0.976 / 0.992, 0.969 / 0.985 and 0.964 / 0.981 of the true ten
  (iNaturalist at 12 and 1 groups, yahoo, LAION at 64 and 8); at a kept share of 0.9 it sends between 4% fewer and 7%
  more rows to stored vectors than the most efficient rule tried, Y = (1 − ϑ)·max(t − μ_c, 0) + ψ0·√ϑ·σ̂ (κ = 1 − ϑ in the source) (`psi_eff.py`,
  `kappa_check.py`). Under filters it holds under random-25 and region-25, but it reads 1.4 to 2.5 times the plain
  rule's rows under an in-cluster bottom quarter and about twice under a top quarter on LAION.
- *With ζ and ϑ from the whole cluster* ((5.8) at m = 1 with the shift taken as mean(e), measured before the build
  took the corrected shift of §5.9a.4, so none of these figures is of the rule as now built; `width_checks.py
  --whole`): under a
  bottom quarter the rule becomes the plain rule (2,160 / 2,321 / 2,844 rows against the plain rule's 2,159 / 2,302 /
  2,773), under a top quarter it loses its over-coverage, and random and region sets are within noise. Unfiltered,
  iNaturalist gains (0.951 kept at ψ0 = 2, was 0.918) and LAION loses 2 to 3 points and 3 to 17% in rows, presumably
  from the error of the 16-direction σ_c. Across 18 cases it keeps 0.93 to 0.99 at ψ0 = 2, at rows within 0 to 5% of
  the best rule for each case, except LAION at 8 groups unfiltered (+17%) (Editor's queries 17 and 25).
- *The plain rule at the same dial*, Y = 2σ̂, over the same cases: unfiltered 0.36, 0.23, 0.32, 0.30, 0.22 (iNaturalist
  at 12 and 1 groups, yahoo, LAION at 64 and 8); random-25 0.62, 0.57, 0.52; region-25 0.53, 0.51, 0.43; top-25 0.66,
  0.58, 0.51; bottom-25 0.93, 0.95, 0.95 (iNaturalist, yahoo, LAION) (`program/LOG-2026-11-21.md`, "answers to referee 2").
  That is 0.22 to 0.95 against 0.93 to 0.99. The
  two rules read different numbers of rows at one dial value, so the fair comparison is at equal rows, and it is not on
  record. Compounding the recorded ratios for the unfiltered indexes, referee 2 puts the sample form of the width at
  about 0 to 4% fewer rows than a re-dialled plain rule on three indexes and 0 to 15% on LAION [computed, by referee 2
  from the record] (`referee-2.md` §4). That the plain rule is not
  efficient is measured on LAION only (7 to 18% more rows than κ = 1 − ϑ); on iNaturalist and yahoo it was level.

The record does not show that ψ0 has one meaning across indexes: the plain rule keeps 0.790 to 0.824 at ψ = 4 and the
sample form of (5.8) 0.918 to 0.976 at ψ0 = 2, and both move by about half a unit of their dial (`referee-2.md` §7).

### 5.9a.4 The whole-cluster height under a filter

**Lemma 5.31 (the whole-cluster height).** Fix a query and a cluster, draw a row uniformly from the cluster, and let s
be its score, e the level's error and Vis the event that it is visible. The law of e given (s, Vis) equals the law of
e given s, for every s that a visible row can have, if and only if P(Vis | s, e) does not depend on e. [proved,
unrefereed: the statement and proof are referee 2's, written as a precise form of the PI's argument] (`referee-2.md`
§6)

*Proof.* By Bayes, p(e | s, Vis) = p(e | s)·P(Vis | s, e)/P(Vis | s). If P(Vis | s, e) does not depend on e, it equals
P(Vis | s) and the ratio is 1. Conversely, if the two laws agree, P(Vis | s, e) = P(Vis | s) wherever p(e | s) > 0. ∎

(The "only if" holds for almost every e under the law of e given s, and "every s a visible row can have" means
P(Vis | s) > 0; the source leaves both implicit. Editor's query 21.)

So a width built from whole-cluster quantities keeps a visible row of score s with the same probability as unfiltered,
at the same s and t. The kept share over the visible neighbours is not invariant, because the filter moves t and with
it the heights at which the rule is used (`referee-2.md` §6 item 1).

*Where it holds.* For a filter that selects on this query's score, with or without independent randomness; for a
random set; and for a set that takes or leaves whole clusters [proved, unrefereed, by the lemma]. On two synthetic row
laws a bottom quarter, a top quarter and a soft filter on score left the mean and spread of e at each score unchanged
to two decimals [model] (`program/approximate/referee/ref2_filter.txt`).

*Where it fails.* For every filter whose visibility at a given score still depends on the error, which is the same as
depending on the midpoint. For visibility decided by a row's position v·(x − m_c) along a direction v, with normal
rows, the lemma's condition is Cov(v·(x − m_c), e | s) = 0, that is vᵀ·(Σ_{c,left} − β_e·Σ_c)·Δ = 0 with β_e = Cov(e, s)/Var(s) =
ρ_es·√(1 − ϑ) [model: referee 2's derivation, no proof written] (`referee-2.md` §6). It holds for v along the query's
offset, which is a filter on score. For a direction unrelated to the query the partial correlation is of order 1/√n in
an isotropic cluster, larger when the variance sits in few directions. It fails for:

- a filter on the midpoint, on the code, or on an attribute carried by the well-coded high-variance directions that the
  query also weighs: at a given score the visible rows on the selected side have smaller errors and the width
  over-covers, and on the complement it under-covers;
- a filter along left-out directions the query weighs: the visible rows' errors are larger at a given score, the
  unsafe side;
- a filter on the score for a different query, unless that query's offset in the cluster is parallel to this one's.

In the simulation a quarter cut along a direction correlated 0.7 with the part of the midpoint, or of the error, that
the score does not explain moved E[e | s] by −0.73σ̂ or +0.73σ̂ at every score, with the scatter falling from 0.83σ̂ to
0.66σ̂ [model] (`ref2_filter.txt`).

*The first-order correction.* Under a filter on score and a linear regression, the mean of e over the visible rows is
ρ_es·σ̂·E[z | Vis], not zero (−0.71σ̂ and +0.72σ̂ for the two quarters in the simulation). Adding mean(e) to the
midpoint, as the track recommended (A4.9 item 1), therefore counts the height twice once ζ is taken from the whole
cluster; that recommendation is [refuted] (`referee-2.md` §6 item 3). The quantity to add is the sample's mean of
e − m·σ̂·(s − μ_c)/σ_c, the mean distance from the ray, which is zero for a filter on score and removes a shift
constant in s, such as the one above, to first order [model: linear regression; referee 2's derivation]. The
implementing session has applied it with m = 1 and the whole-cluster μ_c and σ_c (`program/LOG-2026-11-21.md`, last section);
the measurements above were made with mean(e) (Editor's query 18).

*What the lemma does not cover.* No statistic of the visible rows is invariant, and in particular not σ̂, which (5.8)
takes from the visible sample. σ̂_vis² = E[Var(e | s) | Vis] + Var(E[e | s] | Vis), and under a linear regression
σ̂_vis/σ̂ = √(1 − ρ_es²·(1 − Var(z | Vis))): 0.87 for a quarter cut on score at ρ_es = 0.56 (0.87 simulated) and 0.79 at
ρ_es = 0.7. So under a quarter cut on score (5.8) is 13 to 21% narrower than the lemma licenses, the unsafe side; where
the regression steepens into the tail, a top quarter's σ̂_vis is larger than this formula gives [model, linear
regression] (`referee-2.md` §6 item 2). The agreement under the bottom quarter is nearly an identity and confirms
nothing about the law of e: the visible neighbours of a bottom quarter lie below their cluster's mean, so
max(ζ_c, 0) = 0, the width is ψ0·√ϑ·σ̂ and the rule is the plain rule, which 2,160 rows against 2,159 says and no more.
The lemma predicts more, unmeasured: at s = t below the mean E[e | s] is negative, so the plain rule over-covers there,
and a width with a negative height at the bulk's slope, (ρ_es·ζ + ψ0·√(1 − ρ_es²))·σ̂ with ζ < 0, should read fewer
rows at the same kept share [model] (`referee-2.md` §6 item 4).

The two variants of the height fail in different places, and the lemma says why: the whole-cluster height is the right
frame for a cut on score and is only as good as the library's σ_c, while the sample's height is exact for σ_c and wrong
under a cut on score (`referee-2.md` §7). Accordingly the track's statement that "a visible set needs no separate
treatment, as long as the sample is of visible rows" (A4.7) and its recommendation to take every quantity from the
visible sample (A4.9 item 1) are [refuted] for filters that cut a cluster on score, by the measurement above
(`referee-2.md` §5, §7).

### 5.9a.5 The scatter's tail

The recorded misses of the sample form of (5.8) at ψ0 = 2 / 3 are 0.024 / 0.008 (yahoo), 0.031 / 0.015 and
0.036 / 0.019 (LAION at 64 and 8), and 0.082 / 0.025 and 0.082 / 0.028 (iNaturalist at 12 and 1) [measured, by the
visit-order session]. The track compared them with a Laplace scatter's chance of a miss at the threshold,
½·exp(−√2·ψ0) = 0.030 and 0.0072, and concluded that ψ0 means about 1 − ½·exp(−√2·ψ0). That inference is [refuted]
(`referee-2.md` §4 item 2): it sets the chance at the threshold against a share over all ten neighbours, nine of which
lie above the threshold and have slack. With the slack counted and the measured slopes, a Laplace scatter misses 0.012
to 0.049 at ψ0 = 2 and 0.003 to 0.012 at ψ0 = 3 [model, an exponential excess of rate 2.2]
(`program/approximate/referee/ref2_a45.txt`), and sampled statistics add a factor of about 1.2 and 1.3 [computed]
(`program/approximate/out/a4_width_noise.txt`, as `referee-2.md` §4 item 2 reads it). So the recorded misses are 1.3 to 2.1
times a Laplace scatter's at ψ0 = 2 and 1.6 to 3.8 times at ψ0 = 3: the tail is heavier than Laplace [computed, by
referee 2 from the record]. Independently of any model, one unit of ψ0 from 2 to 3 divides the recorded misses by 1.9
to 3.3, where a Laplace tail divides them by 4.1.

Measured directly, the 99.9th percentile of the scatter is 4.6 to 7.5, and the scatter widens with height [measured,
by the visit-order session] (`program/LOG-2026-11-21.md`, "The level width under filters"). The widening breaks "whatever ζ" in
§5.9a.3: the kept share at one ψ0 falls with ζ even at m = 1. A scatter that widens with height is what a slope that
varies between (query, cluster) pairs looks like when pooled, sd(e/σ̂ | z)² = s_g² + z²·Var(m) with s_g the scatter's
spread, and then the coefficient of ζ that gives ψ0 one meaning is an upper quantile of the pairs' slopes, not their
centre [model; not measured] (`referee-2.md` §4 item 2).

Hence the track's "a recall of 0.999 a level needs ψ0 near 4.4" (A4.8) is [refuted] (`referee-2.md` §4 item 2, §7):
the recorded 99.9th percentile is 4.6 to 7.5, and the recorded decay from ψ0 = 2 gives 4.9 to 7.6 for 0.999 of the
ten. What survives is that ψ0 is not a normal quantile; that 1 − ½·exp(−√2·ψ0) is an upper bound on what ψ0 means; and
that near ψ0 = 2 the normal law, the Laplace law and the data agree (0.96 to 0.98 kept where m is near 1), because the
two laws cross there.

### 5.9a.6 The sample: its error, and a cheaper source

**The sample's error** [model: normal rows, to first order; the formula is referee 2's repair and has not been
re-read] (`referee-2.md` §5). With μ_c, σ_c, σ̂ and ϑ from n_s sampled rows, the width (ζ + ψ0·√ϑ)·σ̂ of the sample
form has an error whose variance, in units of the scatter, is

    ν² = ( c₁²·(1 − ρ_es²) + c₂²/2 + c₁·c₂·(1 − ρ_es²) + 1 ) / (n_s·ϑ),
    c₁ = ζ − ψ0·(1 − ϑ)/√ϑ,      c₂ = ψ0·√ϑ,

and a normal scatter is then kept with probability Φ(ψ0/√(1 + ν²)). The track's formula,
(ζ²·(1 − ρ_es²) + 1 + ψ0²·ϑ/2)/(n_s·ϑ), leaves out the dependence of √ϑ̂ on σ̂/σ_c and the covariance of σ̂/σ_c with σ̂,
and is about 20% low. Referee 2's verdict on the sample's error is "correct after a small repair": the three parts
of the error stand, and the track's formula for ν² as written is [refuted] (`referee-2.md` §5). At ψ0 = 2 the track's formula, the repaired one and a
simulation give 0.958, 0.954 and 0.953 kept at ζ = 3 and 32 rows, and 0.947, 0.940 and 0.941 at ζ = 5 and ϑ = 0.92
[model] (`ref2_a45.txt`). The hypotheses are normal rows and a neighbour that is not in the sample. For (5.8), with the
height from the whole cluster and σ̂ sampled, ν² = (ζ + ψ0·√ϑ)²/(2·n_s·ϑ). It is the same size at ρ_es = 0.56 and
larger at 0.7 (0.931 against 0.940 kept at ζ = 5 and 32 rows), because the errors of σ̂ and σ_c no longer partly
cancel. On real rows ν² is understated, since the recorded 10% to 90% range of actual over sampled spread at 32 rows
(0.83 to 1.29) is wider on the upper side than the normal 0.84 to 1.16. The track's conclusion, from its simulation,
is that 64 rows are enough and 32 marginal for far clusters [computed] (`program/approximate/CLAIMS.md` A4.7).

**Proposition 5.32 (A4.7; the sample at a randomised level).** Let R be a randomised level at which every block is
coded, â_R a row's estimate there, and v_R = Σ_j A_j²·‖P_{y_j⊥}Δ_j‖²/d_j. Then, per row and for any fixed â, e and s,

    E_ω[â_R] = s,      E_ω[(â_R − â)²] = e² + v_R,      E_ω[(â_R − â)·â_R] = e·s + v_R.

[proved, refereed, as an identity in the setting of chapter 3's Lemma 3.1; referee 2 found no error and verified it
on simulated rotations] (`program/approximate/CLAIMS.md` A4.7; `referee-2.md` §5; `ref2_a47.txt`)

*Proof.* At a level where every block is coded the estimate's error is N = â_R − s = Σ_j Δ_j·E_j, E_j the block's
coding error (chapter 3 §3.1). By Lemma 3.1, Δ_j·E_j = A_j·‖P_{y_j⊥}Δ_j‖·U_j with U_j one coordinate of a uniform unit
vector in ℝ^(d_j), and blocks with separate rotations have independent U_j given the stored scalars. So E_ω[N] = 0
and E_ω[N²] = Σ_j A_j²·‖P_{y_j⊥}Δ_j‖²·E U_j² = v_R. Since â_R − â = e + N and â_R = s + N, expanding gives
E[(e + N)²] = e² + v_R and E[(e + N)(s + N)] = e·s + v_R. ∎

So over n_s sampled rows the mean of â_R estimates μ_c, the variance of â_R less the mean of v_R estimates σ_c², the
mean of (â_R − â)² − v_R less the squared mean of â_R − â estimates σ̂², and the covariance of â_R − â with â_R less the
mean of v_R estimates Cov(e, s), hence ρ_es. No stored vector is read. A row's squared-error estimate has variance
Var(e²) + 4e²v_R + 2v_R², so for normal errors the sample must be larger by 1 + 2v_R/σ̂² + v_R²/σ̂⁴: 1.2 times if the
randomised level's spread is a third of the deterministic level's, 1.6 times if it is half [proved, refereed].

Referee 2 lists four hypotheses the source does not state (`referee-2.md` §5).

1. The estimate uses Lemma 3.1's exact scale ‖y‖²/⟨ō, Ry⟩. The index stores the scale rounded up; a scale high by η
   adds η·Δ_j·y_j to â_R, which is not mean zero and is correlated with e. In the simulation 3% gave σ̂² high by 7%, the
   wide side.
2. The code is a function of the row's own rotated block only, and the query does not depend on the seed: the
   conditions chapter 3 attaches to the lemma.
3. v_R needs ‖P_{y_j⊥}Δ_j‖, which the reader cannot compute without the row. It has ‖Δ_j‖, which gives sd_R², or it can
   use the level's own estimate ‖Δ_j‖² − (Δ_j·ŷ_j)²/‖y_j‖², biased by a term of relative size tan²φ_j/d_j; in the
   simulation that estimate gave σ̂² at 1.003 of the truth.
4. The rotations are shared by every row, so two rows' errors are not independent over the seed. Each identity is per
   row and survives, but the count of rows needed assumes independence. In the simulation two rows' errors correlate
   at ±0.02 for ordinary pairs and 0.97 for a near-duplicate pair, so the count is right unless the sample holds
   near-duplicates. The uncentred second moment is exactly unbiased, and the centred one is low by the mean pairwise
   covariance.

*The direction of the bias*, checked by referee 2 by simulation [model; `program/approximate/referee/ref2_a47.py`]. If a block is still uncoded at R, â_R − s holds
Δ_j·y_j for that block, which is not random over the seed, and σ̂² and σ_c² come out low, the unsafe side. The track's
"low by the share of the level's error variance in that block" assumes that block's part is uncorrelated with the
rest; in the simulation the block held 39% of σ̂² and the estimate came out at 30% of the truth, because the blocks'
parts are correlated, and μ_c is shifted by the mean of the block's part as well. If an upper bound is used for v_R
(sd_R², or stored scalars rounded up), too much is subtracted from σ̂² and σ_c² alike, and σ̂/σ_c and σ̂ both fall, again
the unsafe side. Subtracting nothing errs wide by v_R.

Measured on the real codes, a sample read at the first randomised level where every block is coded estimates σ̂, μ_c
and σ_c with a bias of 2 to 6%, low on LAION, and modest extra noise, at about 10 kB against 256 kB of stored vectors
on iNaturalist; it needs the group's rotations set up, so it pays only where they are cheap or already done
[measured, by the visit-order session; `rand_sample.py`] (`program/LOG-2026-11-21.md`; Editor's query 20). The low bias has
the sign of an upper bound used for v_R (`referee-2.md` §7).

### 5.9a.7 The referee's other repairs

- **A4.4 item 2, the slope far up.** The track wrote the slope far up as √(PR(left)/PR(var)), with PR(x) = (Σx)²/Σx²,
  var_i the variance of coordinate i of the group's basis and left_i what the level leaves of it, so that the slope is
  1 when what is left out is as concentrated as the whole. That holds only when every coordinate is wholly coded or
  wholly left out, and is otherwise [refuted] (`referee-2.md` §3). With quantisation error uncorrelated with the
  reconstruction, a distortion dist_i on coordinate i,
  
      κ_Δ ≈ Σ dist_i / Σ var_i,      1 − ϑ ≈ Σ dist_i·var_i / Σ var_i²,
      slope far up ≈ (Σ dist_i / Σ var_i)·√(Σ var_i² / Σ dist_i·var_i)

  [model; referee 2's repair, unrefereed]. For a level that leaves the same share p of every coordinate the track's
  form gives 1 and this one gives √p = √(1 − ϑ), the uncorrelated normal value. The direction the track drew for
  iNaturalist survives: left-out variance spread over many low-variance coordinates gives a slope above 1. The gain
  dist_i/var_i is a small-shift statement and fails for a near-duplicate of the query.
- **A4.5 item 2, the inferred tail**: §5.9a.5.
- **A4.5 item 3, the third case.** The track stated that under the model of §5.9a.3 the efficient rule (a threshold
  on the chance of beating t, at one price across rows, clusters and queries) and the rule with a stable dial are the
  same rule; that is [refuted] as stated (`referee-2.md` §4 item 3). In the model's tail the law of s given â is normal
  about μ_c + (â − μ_c)/(1 − κ_Δ) − κ_Δ·Var(a_G)·χ_X/(1 − κ_Δ)², with spread s_res/(1 − κ_Δ). So

      efficient rule:            Y = κ_Δ·(t − μ_c) − κ_Δ·Var(a_G)·χ_X/(1 − κ_Δ) + ψ·s_res,
      stable dial (row at s = t): Y = κ_Δ·(t − μ_c) + ((1 − κ_Δ)·Var(e_G) − κ_Δ·Var(a_G))·χ_X + ψ0·s_res.

  Both have the coefficient κ_Δ on the height, as the track said, and both carry a constant it omitted; the two dials
  differ by s_res·χ_X/(1 − κ_Δ), which depends on the cluster. So the two rules agree within a cluster and not at one
  price across clusters [model; referee 2's repair, unrefereed]. In population, Y = κ_Δ·(t − μ_c) + ψ0·s_res without
  the constant keeps a row at the threshold with probability 0.93, 0.80 and 0.76 at ψ0 = 0 (iNaturalist at 12 groups,
  yahoo, LAION at 64) and 0.99, 0.97 and 0.96 at ψ0 = 1; with the constant it keeps 0.46 to 0.49, 0.82 to 0.84 and
  0.97 to 0.98 at ψ0 = 0, 1 and 2 in every setting and at both heights tried [model] (`ref2_a44.txt`). The limit is
  reached slowly: among the top 0.1% of rows by midpoint the constant is −1.31 where the limit is −1.43, and the spread
  0.46 where the limit is 0.54 (`ref2_a45b.txt`).
- **A4.5 item 5, one meaning for ψ0.** The track stated that under its model, with κ = κ_Δ and ψ0 on the scatter, the
  kept share is P(g ≤ ψ0) on every index. That is [refuted] by the track's own output (`referee-2.md` §4 item 5): the
  ψ0 needed for 0.9 kept is −0.91, −0.70, 0.03, 0.18 and 0.06 on the five settings with κ = κ_Δ, and 0.18, 0.71,
  −0.31, −0.14 and 0.10 with κ = √(1 − ϑ) [model] (`program/approximate/out/a4_resemble.txt`). The cause is the
  omitted constant. The corrected statement is the last sentence of the previous item: in the model a dial with one
  meaning needs the width κ_Δ·(t − μ_c) + c0 + ψ0·s_res, with c0 the stable dial's constant above (the intercept of
  §5.9a.3), and that width keeps a row at the threshold with probability 0.46 to 0.49, 0.82 to 0.84 and 0.97 to 0.98 at ψ0 = 0, 1
  and 2 in every setting [model; referee 2's repair, unrefereed].
- **Two further statements withdrawn.** That a model of independent heavy tails for the midpoint and the error
  "cannot reach ϑ above about 0.8" is [refuted]: it is an artefact of giving the two exponential parts one scale
  (`referee-2.md` §3). The conclusion stands on model-free facts: Cov(â, e) > 0, the slope of score on midpoint is 1.04
  to 1.09 and not 1, and the plain rule is 7 to 18% worse than κ = 1 − ϑ on LAION [measured]. That "a width one scatter
  too wide is 1.6 to 2 times the rows" (A4.9) is [refuted] as understated: with the recorded ϑ the scatter √(ϑ(1 − ϑ))
  is 0.27 to 0.47 of the score's spread, and the factor is 1.8 to 2.8 at a decay rate of 2.2 [computed, by referee 2]
  (`referee-2.md` §7).

### 5.9a.8 The levels' dial against the stop's dial

Sources: `program/approximate/DIAL.md` (the derivation, unrefereed as written) and
`program/approximate/referee/referee-3.md` (its referee). Timings are by the visit-order session
(`program/LOG-2026-11-21.md`, "After the first pass", and that session's branch research/sampled-width). In this
subsection ψ is the stop's dial of section 5.8 and ψ0 the levels' dial of rule (5.8).

**The question.** The reader as first built used ψ0 = ψ, so that one knob set both rules. That is too wide. At ψ of 5
to 6 the levels' width is about 1.6 times the derived width on yahoo, and the time to a given recall is 1.25 to 1.78
times that of the derived-ρ build on LAION, yahoo and BioCLIP, and 0.57 to 0.88 on iNaturalist, where the derived
width was too narrow [measured, by the visit-order session, 100 queries a dataset, unfiltered]. The owner asked for
ψ0 as a function of ψ with no constant fitted offline.

**Why equal dials are wrong** [model] (`DIAL.md` §2.1 and §2.2; `referee-3.md` §2: the algebra is right, and the reading
holds to within a factor 1.2 to 2.5). The stop declines a cluster when the threshold sits more than δ + ψ·ρ spreads
above the cluster's mean. So ψ is, roughly, a quantile of the height of the threshold in the neighbours' home
clusters, whose median is 2.8 to 3.6 spreads on the three dumps [measured, by simulation on the dumps of yahoo, LAION
and iNaturalist, `dial_laws*`]. The width of (5.8) already pays for that height through its term in ζ. The levels'
dial therefore has to cover only the scatter g that is left, and equal dials charge the height twice.

**The criterion** [model] (`DIAL.md` §2.4; `referee-3.md` §3, derived independently, with one correction). Split one
budget of missed neighbours between the stop and the levels so that the cost of the query is least. The first-order
condition is equal risk per row declined: a row that the levels drop at their margin is as likely to be a neighbour
as a row of a cluster that the stop declines, up to the factor κ = c₁/(c₀ + c₁·φ), with c₀ the cost of a row of a
newly read cluster, c₁ the cost of a row that passes the levels and φ the share of rows the levels pass. The referee's
correction is the term c₁·φ: opening a cluster also sends rows through the levels. Equal risk per neighbour, or per
cluster, is a different criterion and is not the optimum: it would be the optimum only at κ near 80, and the reader's
runs give 2 to 7 [measured, by the visit-order session, as read by referee 3].

**The formula that was derived, and why it is not recommended.** The derivation gives

    ψ0 = max(0, ψ − ū − s̄ + (1/√2)·ln(1/(κ·φ̂))),

with ū the median over the k held rows of ((t̂ − μ_c)/σ_c − δ)/ρ in each row's home cluster, s̄ a term for the held
rows' slack above t̂, and φ̂ the share of sampled clusters' rows that the levels have passed so far [model] (`DIAL.md`
§2.5 and §3). The referee's verdict is that it should not be built (`referee-3.md`, "Verdicts in one place"), for three reasons.

- It is never better than a constant dial in the replay. At its own mean dial a constant passes fewer rows and drops
  fewer neighbours on all three datasets, and the formula's spread of 0.6 to 0.8 between clusters is what costs it
  [measured, by simulation: a causal replay on the three dumps that uses the running threshold at each read,
  `dial_replay*`, re-run in `ref3_replay.py`].
- The 1/√2 is the tail rate of a Laplace law of unit spread, used as a stand-in. It is a tuning constant in effect.
  With the measured tail rates θ_y of the scatter and θ_u of the height the rule is
  −s̄ + (θ_y/θ_u)·(ψ − ū) + θ_y·ln(θ_u/(θ_y·κ·φ̂)), which is 0.7 to 1.2 units wider and dearer, and is then fitted
  [model] (`referee-3.md` §4). The statement that the formula has nothing fitted is [refuted].
- The replay's proxy for time is wrong on LAION. It prices the sample at about 8% of the cost where the timing puts
  it near a third (about 800 µs a query), and it leaves out the first cluster, which is read before k rows are held
  and is 80% of yahoo's rows past the levels at ψ = 5 (`referee-3.md` §5 and §7).

Two further corrections by the referee. The statement that unfiltered δ and ρ sit on their priors holds for yahoo
(0.0 to 0.6 clusters read directly a query) and roughly for LAION, and is [refuted] for iNaturalist (3 to 10 a
query), which is where the formula misses. And at the measured slopes m the frame lacks a term −(m − 1)·x/√ϑ, about
0.67 units of the dial on iNaturalist.

**What stands.**
- The dial is an offset of the stop's and must rise with it, at 0.72 to 0.85 per unit of ψ in the replay [model]
  (`referee-3.md` §8). A constant is not enough: at ψ = 9 the reader's recall is capped at 0.970 for ψ0 = 2 and
  0.989 for ψ0 = 3 on LAION [measured, by the visit-order session].
- At a fixed ψ0 = 3 the time to recall 0.9 / 0.95 / 0.99 over the earlier build is 0.84 / 0.59 / 0.56 on iNaturalist,
  0.93 / 1.06 / 1.31 on yahoo and 1.44 / 1.77 / – on LAION, and the rows past the deterministic levels at ψ = 5 on
  LAION fall from 15,304 to 5,699 [measured, by the visit-order session, unfiltered]. What is left of LAION's cost is
  the sample itself.
- The rule the evidence supports for the next timing is ψ0 = max(0, ψ − c) with c ≈ 1.5. The constant c is fitted, on
  three datasets of 100 queries each (`referee-3.md`, "Verdicts in one place" and §6). The owner waits on its timing (21 November 2026).
- A way to remove the fitted constant, untested: measure θ_y and θ_u at build, from corpus items used as queries,
  for which the true neighbours are known. The derivation found estimates of them within one query too noisy, and
  the pooled sample's tail 3 to 12 times lighter than the neighbours' at a dial of 4 (`DIAL.md` §5 and §6).

Not tested under any of these: filters, and the dial of the randomised levels, which still use ψ and whose risk is a
union over four levels.

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

### 5.10.1a The tree as a random energy model, and the leaf floors

This section reads model T of Proposition 5.18 as a random energy model and gives two floors on the leaves any
algorithm must read (`program/approximate/CLAIMS.md` A1, refereed in `program/approximate/referee/referee-1.md`). The
tree is **regular**: every level-(j − 1) node has the same number K_j/K_{j−1} of children. A level is branching when
K_j > K_{j−1}. The leaves are level D, there are K_D of them, a leaf's value U has variance v_D, and ϑ_j = v_j/v_D.

**Proposition 5.33 (A1.1; two models).**

1. Model T on a regular tree is Derrida's generalised random energy model with D levels, entropy shares
   ent_j − ent_{j−1} and variance shares ϑ_j − ϑ_{j−1}. Placed on a binary tree, it is the continuous random energy
   model whose profile is the step function Π_step.
2. When every log₂(K_j/K_{j−1}) is an integer and ent_j increases strictly over the branching levels, model T's
   **binary refinement**, in which level j is replaced by log₂(K_j/K_{j−1}) binary levels of equal variance, is the
   continuous random energy model on the binary tree of depth log₂ K_D with the piecewise-linear profile Π.

The two are different processes. They agree in (a) the law of the values along one root-to-leaf path at the depths
ent_j; (b) the expected number of nodes of one level above a cut, and so the B_j of (5.7); and (c) the concave hull of
the profile, which is the hull of the points (ent_j, ϑ_j) for both, and so the ground state x_s. They differ in (d)
x_*: the theorem cited below needs a profile that is the integral of a Riemann-integrable function, which Π is and
Π_step is not, so it is a theorem about the refinement and says nothing about model T; and (e) the floor of
Theorem 5.34, whose proof needs the leaves under one parent to be independent given the internal values with that
parent explaining ϑ_{D−1}, while in the refinement the internal values include the binary levels inside the last
level. The hypotheses are a regular tree, integer log₂ fan-outs for the refinement, Gaussian increments, an increment
independent of its parent's value, siblings' increments independent of each other, and equal within-node variance at
a level. [proved, unrefereed: revised after referee 1, which found the first version correct after this repair; the
revision was not re-read; Derrida's definition cited, unchecked] (`program/approximate/CLAIMS.md` A1.1;
`referee-1.md` A1.1)

*Proof.* (1) is Derrida's definition [cited, unchecked: from memory, and as Addario-Berry and Maillard describe it].
In model T two leaves whose last common ancestor is at level j − 1 have covariance v_D·ϑ_{j−1}, which is v_D·Π_step at
every binary depth between ent_{j−1} and ent_j. (2) is the definition of the continuous model (Addario-Berry and
Maillard, section 1): a centred Gaussian process on the binary tree of depth N′ with E[X_v·X_w] = N′·Π(|v ∧ w|/N′),
equivalently independent increments of variance N′·(Π((i + 1)/N′) − Π(i/N′)) at generation i. There the same two
leaves have covariance v_D·Π at the depth of their last common binary ancestor, anywhere in [v_D·ϑ_{j−1}, v_D·ϑ_j), so
the processes differ. (a) and (b): a node at binary depth ent_j·log₂ K_D of the refinement has value N(0, v_D·ϑ_j),
and a path's values at those depths have independent increments of variance v_D·(ϑ_j − ϑ_{j−1}), as in model T; the
expected count of single nodes above a cut uses only these marginal laws. (c): the hull of a step function with jumps
at the ent_j and of the interpolation through the same points is the hull of the points. (d) and (e) are argued in the
statement. ∎

*The index as the model* (a mapping, not a claim about data). A query is one realisation of the values on a fixed
tree. A row's score is a leaf's value; a group, a cluster or a sub-cluster is a node at a branching level, with the
mean score of its rows as its value; a row's codes and stored vector are levels of fan-out 1 below the last branching
level; the k nearest neighbours are the k best leaves. One departure is visible before any data is looked at: a node's
value taken as the mean of its rows makes the children's increments sum to zero, weighted by size, so siblings are
correlated −1/(F − 1) for F equal children, nothing at 2,000 children and a third at 4-way splits. The track's tests
of the mapping (A1.7) do not test this hypothesis [proved, unrefereed: referee 1's remark] (`referee-1.md` A1.1
item 5).

**Theorem 5.34 (A1.2; the leaf floor at a fixed level).** In model T with 0 < ϑ < 1, ϑ = ϑ_{D−1}, give an algorithm
the value of every internal node for nothing. It queries leaves one at a time, adaptively, with randomness of its own
independent of the leaf values, and stops when it chooses; a target, a leaf with U ≥ T for a fixed T, is found when
its leaf is queried. Let r < 1. If the expected number of targets found is at least r times the expected number of
targets, then

    E|𝒬| ≥ B_leaf(r) = K_D·P(A_{D−1} > θ_r),      θ_r defined by P(A_{D−1} > θ_r, U ≥ T) = r·P(U ≥ T),

where (A_{D−1}, U) are jointly normal with Var A_{D−1} = v_D·ϑ, Var U = v_D and covariance v_D·ϑ. The rule "query the
leaves whose parent's value exceeds θ_r" attains it. [proved, unrefereed: referee 1 found no error in the theorem as
first written; the revision adds the hypotheses 0 < ϑ < 1, r < 1 and independent randomness, the reading of recall
below, and the step for 𝒬 that the referee supplied, and was not re-read] (`program/approximate/CLAIMS.md` A1.2;
`referee-1.md` A1.2)

"Recall r" here is a ratio of expectations, E[found] ≥ r·E[targets]. With T fixed the number of targets in a tree is
random and over-dispersed, since targets come in clumps under a few high parents, so the ratio weights the trees by
their target counts; it is not the mean over trees of a tree's recall.

*Proof.* Given 𝒢 the leaf values are independent, and leaf i has U_i ~ N(A_{par(i)}, v_D·(1 − ϑ)), because U_i is its
parent's value plus a level-D increment independent of the others and of 𝒢. Write p_i = Φ̄((T − A_{par(i)})/√(v_D(1 −
ϑ))), which is 𝒢-measurable. Fix a leaf i and run the same rule on the input with U_i replaced by a constant: the two
runs agree until i is first queried, so the event {i ∈ 𝒬} is a function of 𝒢, the other leaves' values and the
randomness, of all of which U_i is independent given 𝒢. Hence P(U_i ≥ T, i ∈ 𝒬) = E[1{i ∈ 𝒬}·p_i], and
E[targets found] = E[Σ_{i∈𝒬} p_i]. For every λ ≥ 0 and every set 𝒬, Σ_{i∈𝒬} p_i ≤ λ|𝒬| + Σ_i (p_i − λ)₊. Taking
expectations, E[targets found] ≤ λ·E|𝒬| + K_D·E(p − λ)₊, with p = Φ̄((T − A)/√(v_D(1 − ϑ))) and A ~ N(0, v_D·ϑ). Choose
λ_r > 0 with K_D·E[p; p > λ_r] = r·K_D·P(U ≥ T); it exists because p has a continuous law (0 < ϑ < 1) and r < 1. Then

    E|𝒬| ≥ ( r·K_D·P(U ≥ T) − K_D·E(p − λ_r)₊ ) / λ_r = K_D·P(p > λ_r) = K_D·P(A > θ_r),

since E[p; p > λ] − E[(p − λ); p > λ] = λ·P(p > λ), and {p > λ_r} = {A > θ_r} because p increases with A, with
E[p; A > θ] = P(A > θ, U ≥ T). The rule 𝒬 = {i : p_i > λ_r} has E|𝒬| = K_D·P(p > λ_r) and finds exactly the required
expected number. ∎

The proof uses nothing of the tree above the leaves' parents and nothing of 𝒢 beyond each leaf's own parent, so the
floor holds for any tree shape and for an algorithm told any further information independent of the level-D increments
[proved, unrefereed: referee 1's observation, checked by the track] (`referee-1.md` A1.2). Because it counts leaf
queries only, in the model it is a floor on the rows read by any hierarchy whose finest centres explain a share ϑ,
under any order, stop or beam; and with one leaf per parent the proof is unchanged, so one level lower it is a floor on
the stored vectors read given the 16-byte or 32-byte level, with ϑ the share those levels explain [proved,
unrefereed] (`program/approximate/CLAIMS.md` A1.2, remarks). Referee 1 attacked it with 12,000 trees of 200 × 500
leaves and six leaf-query rules, four of them adaptive: none is above the floor, and the two built to sit on it are
within 0.05 targets of it, with a standard error of 0.10 [model] (`program/approximate/referee/ref1_a12.txt`).

**Theorem 5.35 (A1.2b; the leaf floor for the k best).** In model T with 0 < ϑ < 1, under the conditions of
Theorem 5.34 on the algorithm, let the targets be the k leaves of largest value, and let
p′_i = Φ̄((τ_{−i} − A_{par(i)})/√(v_D(1 − ϑ))). If the expected number of the k best that are found is at least r·k,
then

    E|𝒬| ≥ sup over λ > 0 of ( r·k − E Σ_i (p′_i − λ)₊ ) / λ.                                    (5.9)

[proved, unrefereed: the theorem and its proof are referee 1's, checked line by line by the track; nobody else has
read it] (`program/approximate/CLAIMS.md` A1.2b; `referee-1.md` A1.2, remark (i))

*Proof.* Leaf i is among the k best if and only if U_i > τ_{−i}, ties having probability zero. τ_{−i} is a function of
the other leaves' values, and {i ∈ 𝒬} is a function of 𝒢, the other leaves' values and the randomness, as in
Theorem 5.34. Given that triple U_i ~ N(A_{par(i)}, v_D(1 − ϑ)), so P(i among the k best, i ∈ 𝒬) = E[1{i ∈ 𝒬}·p′_i]
and E[found] = E Σ_{i∈𝒬} p′_i. For every λ > 0 and every 𝒬, Σ_{i∈𝒬} p′_i ≤ λ|𝒬| + Σ_i (p′_i − λ)₊. Take expectations
and use E[found] ≥ r·k. ∎

A leaf outside the k best has τ_{−i} = τ_k, and a leaf among them has the (k + 1)-th best, so the right side of (5.9) is
the knapsack, at one price over trees, of a rule that is told τ_k, and Σ_i E p′_i = k. The floor is therefore not
attained, since no algorithm knows τ_k. A static rule that knows every internal value and not τ_k, with
p_c = E[F̄_c(τ_k)] as in (5.1), is attainable, and in the cells below it lies 1 to 4% above the floor, so the true cost
of the k best lies between the two. B_leaf(r) at the level with K_D·Φ̄(z_T) = k is not proved to be a floor for the k
best; in every cell computed it lies below (5.9).

| K × fan-out | ϑ | B_leaf(0.9), fixed level | floor (5.9) | static optimum | floor / B_leaf |
|---|---|---|---|---|---|
| 500 × 2,000 | 0.30 | 85,084 | 89,482 | 92,742 | 1.05 |
| 500 × 2,000 | 0.45 | 20,735 | 28,318 | 29,229 | 1.37 |
| 500 × 2,000 | 0.60 | 4,198 | 11,898 | 11,984 | 2.83 |
| 339 × 2,000 | 0.30 | 62,721 | 68,786 | 71,227 | 1.10 |
| 339 × 2,000 | 0.45 | 16,092 | 24,884 | 25,854 | 1.55 |
| 339 × 2,000 | 0.60 | 3,447 | 10,682 | 11,059 | 3.10 |
| 28 × 784 | 0.45 | 1,681 | 2,907 | 3,030 | 1.73 |
| 5,400 × 125 | 0.45 | 16,046 | 16,986 | 17,553 | 1.06 |
| 5,400 × 125 | 0.60 | 3,439 | 3,998 | 4,172 | 1.16 |

[computed] (`program/approximate/referee/ref1_a12c.py`: k = 10, r = 0.9, independent parents of variance ϑ, the
Poisson form of (5.1), no leaf sampled.) So with a few hundred parents the floor for the k best is 1.05 to 3.1 times
B_leaf(r). The cause is that τ_k does not concentrate: over trees it has a standard deviation of 0.15 to 0.24·√v_D at
K = 339 to 500 and ϑ = 0.45 to 0.6, against 0.03 to 0.12 at K = 5,400 [computed] (`ref1_a12.txt`). With the leaves
sampled and no Poisson form the track finds floor over B_leaf of 1.75 at 28 × 784 and ϑ = 0.45, against the
referee's 1.73 [computed] (`program/approximate/scripts/topk_model.py check`). On trees with the three suite fits'
measured shares, the static optimum is 1.42, 1.50 and 1.19 times B_leaf(0.9) at the cluster level (yahoo, LAION
text, glove), 1.69, 1.88 and 1.30 with leaves of about 125 rows under the clusters, and 1.38, 1.76 and 1.41 with
leaves of about 31 rows [computed] (`program/approximate/out/topk_model_fits.txt`). The floor's own column
in those runs came out above the static optimum in 11 of 18 cells, which cannot be, and a run at 19,200 draws a seed
shows it is noise, so only the static optimum is quoted from them.

**What does not transfer, and statements withdrawn.**

- *The hardness theorem.* Addario-Berry and Maillard (arXiv:1810.05129, Theorem 1.1) prove, for a continuous random
  energy model on a binary tree of depth N′ with profile Π = ∫Π′, Π′ Riemann-integrable: (1) for x < x_* there is C(x)
  such that for every ε > 0 and all large N′ an algorithm stops within C(x)·N′ queries at a leaf of value at least
  x·N′ with probability above 1 − ε; (2) for x > x_* there is γ > 0 such that for N′ large, for every algorithm that
  queries node values, the number of queries before such a leaf is found dominates a Geometric(e^(−γN′)) variable
  [cited: the track read the statement and the proof; referee 1 checked the statement through a summary of the paper's
  HTML]. The ground state is x_s, after Bovier and Kurkova as that paper quotes them [cited, unchecked], and x_* = x_s
  if and only if Π is concave. The track's first version applied the theorem to model T, so that a non-concave
  measured profile would make every hierarchy of centres pay an exponential number of reads; that is [refuted]
  (`referee-1.md` A1.1 item 2). The theorem is about the binary refinement. Model T's own profile is Π_step, for which
  the formula gives x_* = 0, which is right in substance: under a flat fan-out of K_D^(ent_j − ent_{j−1}) children with
  no levels inside, every gain costs a power of K_D. For the index's tree, with two to four branching levels, the
  bounds that apply are Theorems 5.34 and 5.35 from below and the beam's count from (5.7) from above
  (`program/approximate/CLAIMS.md` A1.6 [model]). The read is also an enumeration to a recall, not the finding of one
  leaf above a level, and its floors are present at a given D whatever the shape of the profile.
- *The measured profiles.* On yahoo, LAION text and glove the profile is not concave, the last segment, from cells of
  about 31 rows to the rows, being the steepest [measured] (`program/approximate/CLAIMS.md` A1.7 item 1;
  `program/approximate/scripts/replay_np.py`). So in the limit of a binary refinement x_* < x_s. The margin of 2.5 to
  5% that the first version read from the hull sum Σ_j √((ϑ_j − ϑ_{j−1})(ent_j − ent_{j−1})) is [refuted]
  (`referee-1.md` A1.5 item 4): at 21,952 leaves and shares (0.6, 0.25, 0.15), the realised level of the best L
  leaves over the level at which L leaves are expected moves from 0.90 at L = 1 to 1.00 at L = 512, so the hull sum
  does not predict a realised level to better than about 0.05, which is the size of the margin [model]
  (`program/approximate/referee/ref1_a14_a15.txt`, part C).
- *Which dichotomy.* The first version identified model T's split (a subpolynomial beam when Π(t) ≥ t for every t)
  with Bovier and Kurkova's weak and strong correlation regimes; that is [refuted] (`referee-1.md` A1.5 item 3). Their
  weak regime is Π(t) ≤ t for every t, the other side of the diagonal: a profile that crosses the diagonal is on the
  bad side of both splits, and only Π(t) = t is on the good side of both.
- *Leaves of 125 rows.* The first version ranked designs by the exponents ent_j − ϑ_{j−1} of the beam's terms and
  concluded that leaves of about 125 rows bring the row term down to the centres' term on yahoo and LAION; that is
  [refuted] (`referee-1.md` A1.4 item 5). At N = 10⁶ term j ≥ 2 carries a factor exp(z_ε·z_T·√(ϑ_{j−1}(1 − ϑ_{j−1}))),
  two orders of magnitude at ϑ near ½, and by count the row term is the largest in all twelve designs on the three fits,
  by 20 to 100 times [computed] (`program/approximate/referee/ref1_a14_a15.txt`, part B). The exponent is a statement
  about the limit only. In the model finer leaves still lower the rows when the read is one knapsack over the leaves:
  the static optimum, as computed, falls 2.1 to 2.4 times from the clusters to 125-row leaves and a further 1.4 to 2.1 times to
  31-row leaves, against measured falls of 2.7 to 4.1 and 2.3 to 2.5 times [model]
  (`program/approximate/CLAIMS.md` A1.9), which agrees in direction with §5.10.4.

*Prior art.* The generalised random energy model is Derrida's [cited, unchecked]. Theorem 1.1 of Addario-Berry and
Maillard is cited above, with the ground state of Bovier and Kurkova through it. Their open question 2, formula (5.1),
conjectures the cost exponent above x_*; it is their [conjecture], read by the track and through a summary by
referee 1. Pemantle (Ann. Appl. Probab. 19, 2009) shows that
reaching a fraction 1 − ε of the best level on a homogeneous tree costs N·exp(Θ(ε^(−1/2))) for Bernoulli increments of
mean below ½ [cited, unchecked: the abstract through a search summary and the description in Ho and Maillard,
arXiv:2107.11465, section 1]; with ε of order 1/log N that is exp(Θ(√log N)), the form of `hierarchy/theory.md` (2.2).

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
3.0 / 3.0 / 4.0, 64 a cluster 3.5 / 3.5 / 3.0, and 2.5 to 3.5 at k = 100 [model] (`hierarchy/theory.md`
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
  (SOAR [53] and SPANN's copies [54] in `lit/REPORT.md`; breaks the exact layout), and **a 4 or 8 byte first level**
  (no gain on yahoo at 1M) (`hierarchy/layered.md` §3). The spilled copies of `reports/surge2/approx-design.txt` were
  not run as designed there, but a SOAR second assignment (λ = 1) was run in the stage 1b benchmark, under the older
  approximate mode that probes clusters by centre score plus a radius term. It read 12 to 15% fewer items for the same
  recall and was no faster (glove 863 against 922 queries a second, nytimes 316 against 319), because the second
  members are scattered rows read out of order [measured] (`bench/REPORT.md`, "What the numbers say", item (c)); the
  option is now retired (`bench/REPORT.md`, "Deviations from the protocol").
- **A stop on expected misses** (§5.8a), and **rule 1's derived ρ as a general width**, **a width of ψ spreads with
  the right spread**, and **every quantity of the width from the visible sample** (§5.9a).
- **A level of centres over the clusters at 1M, and the groups as that level** (§5.10.5); **sub-clusters** in all three
  layouts at 1M (§5.10.4); **flat finer clusters**, which cost the exact path's per-cluster work 16 times over.

## Open

1. **A stop with a guarantee.** The window stop is a heuristic (11 of 878 stops on synth4k missed a better cluster),
   and the exact lazy stop is too costly; nothing in between is known.
   21 November 2026, in part settled: a stop fed by the pooled sample cannot be valid for every shape of the
   within-cluster tail, a Cantelli stop valid for every standardised shape is useless in size, and the stop on
   expected misses was dropped (§5.8a). A guarantee for a restricted class of tails (a hazard rate bounded below, a
   log-concave tail, a known end of the support) is still open.
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
11. **The slope m of (5.8).** It is fitted (m = 1), not derived: the track's model gives 0.67 to 0.80 where 0.89 to
    1.21 was measured, and part of iNaturalist's 1.2 belongs to the frame it was measured in (§5.9a.3). A derivation is
    open, and so is a fit at build from corpus rows and their own neighbours in the reader's units
    (`program/approximate/CLAIMS.md` A4.6 item 4); a stable dial may need an upper quantile of the (query, cluster)
    slopes, not their centre (§5.9a.5).
12. **Filters that select on the coded or the left-out part of a row** at a given score, such as an attribute carried
    by well-coded directions the query weighs. Lemma 5.31 fails for them, and the size of the effect on real attribute
    and region filters is not bounded (§5.9a.4).
13. **The plain rule's rows at equal recall** against (5.8). The record compares kept shares at one dial value, at
    which the two rules read different numbers of rows; the comparison at equal rows or equal recall is not on record
    (§5.9a.3).
14. **A discriminator from the reader's pooled statistics.** A sketch's error in σ_c moves ρ_q and leaves δ_q near zero
    (llama: ρ_q 0.60 to 0.63), while a cut on score moves δ_q by one to three spreads; whether (δ_q, ρ_q) can choose per
    query between the whole-cluster and the sample height is referee 2's observation, not a result (`referee-2.md` §7).
15. **The stop at high recall unfiltered** on arxiv, iNaturalist, glove and SIFT, where a fixed cluster count reads
    fewer rows than the pooled stop (§5.8a).
16. **The width's corrected shift and the narrower σ̂ under a cut on score**: the build's shift (the sample's mean of
    e − σ̂·(s − μ_c)/σ_c) is not yet measured, and σ̂ from a visible sample is 13 to 21% too narrow under a quarter cut on
    score in the linear model (§5.9a.4).

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
   21 November 2026, in part: `program/approximate/CLAIMS.md` A3.1 states the ratio for a fixed offset exactly, with no
   product step, and recovers (5.6) as E[ρ_ℓ(q, c)²·w²] = ρ_{g,ℓ}²·E w² [proved, unrefereed: revised after referee 1,
   which found no error in the earlier text]. For a fixed offset w is a constant, so E[h² | κ] = w²·E[u² | κ] needs no
   product of means; averaged over offsets the identity is about E[ρ_ℓ(q, c)²·w²], not about E h², so the product step
   of (5.6) as stated is unchanged. A3 is not transcribed in this chapter.
6. **The numbers of Proposition 5.17.** Evaluating Φ((ψ − g√(1 − ϑ))/√ϑ) at g = ψ = 4 gives losses of 0.67%, 1.3% and
   17.3% for ϑ = 0.8, 0.72 and 0.2; the source gives 0.7%, 1.4% and 19%.
7. **What the built rule 1 reads.** `hierarchy/layered.md` §4.2 says the approximate path can skip the randomised
   levels; the commit `370580d8e` reads them at the last deterministic level's ρ and uses one median ρ per level over
   the groups where the derivation gives one per group.
   21 November 2026, closed for the second point: the approximate track read the code at `370580d8e` and confirms that
   `Adaptive::new` takes the median over the groups of each deterministic level's ρ, so the built rule is per level, not
   per group (`program/approximate/CLAIMS.md` A3.4, read in the code). The first point is unchanged.
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
14. **The idealised replay and the real reader on LAION.** The idealised stop of A2.6 pooled δ_q and ρ_q over every
    row read, as the pooled stop `b87f1513e` does, yet on LAION it lost to a fixed count as the older stop `f528836`
    does, while `b87f1513e` wins there (§5.8a). The idealised reader also differs in reading whole clusters with exact
    spreads and pricing every unread cluster; the sources do not say which difference reverses the result.
15. **"5 to 25%".** The visit-order session reads the four datasets where the fixed count wins as costing the stop 5
    to 25%, but arxiv's ratio of 0.73 at recall 0.99 is 27% fewer rows for the fixed count, or 37% more for the stop.
    The figures are reported as given.
16. **ϑ in the whole-cluster rule.** `program/LOG-2026-11-21.md` says ζ and f come from the library's μ_c and 16-direction
    σ_c(q) while σ̂ comes from the visible sample; it does not write f's formula. §5.9a.3 reads it as
    ϑ = 1 − σ̂²/σ_c(q)². Referee 2 lists it as unchecked, with whether the session centred e/σ̂ and which spreads the
    slope used; three of its verdicts turn on these three points together. `program/LOG-2026-11-21.md`, "answers to referee 2",
    has since settled the other two (not centred; the sample's own mean and spread), not this one.
17. **"18 cases".** The record says the rule keeps 0.93 to 0.99 "across 18 cases", but its list of the plain rule's
    kept shares over "the same 18 cases" has 17 entries (5 unfiltered, and 3 each under random-25, region-25, top-25
    and bottom-25).
18. **Which shift the measurements used.** Settled by `program/LOG-2026-11-21.md`, last section: the 18-case figures for (5.8)
    were measured with the shift taken as mean(e), before the correction, and the build now adds the sample's mean of
    e − σ̂·(s − μ_c)/σ_c, which is not measured on record. §5.9a.3 says the figures predate the correction. What is
    still open: no commit is recorded for that build, and `width_checks.py`, the script of the whole-cluster variant,
    is named in the record without a path.
19. **LAION's derived ρ, two figures.** §5.9's table has the derived ρ about 15% above the measured one on LAION text
    at 64 groups (0.0561 against 0.0485), while the dumps of §5.9a.1 give actual over derived 0.94 there. The first is
    an rms over all rows of 100 queries; the second a median over the clusters holding a neighbour, in the first 30
    clusters a query. Both are reported.
20. **"A4.4's identity".** `program/LOG-2026-11-21.md` calls the sampling identity at a randomised level "A4.4's identity"; it
    is A4.7's in `program/approximate/CLAIMS.md`. The chapter cites A4.7.
21. **Lemma 5.31's converse** holds for almost every e under the law of e given s, and the lemma needs P(Vis | s) > 0
    at the scores it speaks of; referee 2 leaves both implicit. They are stated beside the proof, not added to the
    statement.
22. **The proofs of the dropped stop** (A2.1, A2.2, with the Cantelli stop and the sequential sketch) are summarised
    in §5.8a with their tags and not transcribed, as the brief for this pass asked; the chapter of dropped work is to
    carry them.
23. **Which stop is `f528836`.** The record calls it "the older stop" and does not say whether it is the build of §5.8
    that froze δ_q and ρ_q after the first clusters.
24. **Tags of Theorems 5.34 and 5.35 and Proposition 5.33.** Referee 1 found no error in the leaf floor at a fixed
    level as first written, and wrote the floor for the k best itself; the track then revised both, adding the
    hypotheses the referee named. By the rule that a tag is never upgraded, all three stay [proved, unrefereed] until a
    referee reads the revised text.
25. **"The best rule for each case"** in the 18-case measurement is not named in the record; across the five
    unfiltered indexes the best rule tried earlier was the width with κ = 1 − ϑ.
26. **Proposition 5.30's last clause.** The source (A4.3 item 4), as referee 2 read it, says the slope is 1 exactly
    when e is a linear function of s, and the proof argues from |correlation| ≤ 1. Equality ρ_es = 1 needs e to be an
    increasing linear function of s; a decreasing one gives −1. The statement is kept as refereed.
