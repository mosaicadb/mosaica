# A hierarchy over or under the clusters, for the approximate modes

Does a level above the clusters (super-clusters, or the groups) or below them (sub-clusters) help approximate k-NN
and an approximate threshold query, and in what form? This note extends `../theory.md` (the cluster order) to trees,
checks the extensions on the suite's three fits, and weighs each candidate against the two things the index must
keep: checking through the per-group dual bases with rows sorted by key inside a cluster, and behaviour under
visible sets.

Status labels: **[proved]** (proof or complete sketch here), **[model]** (derived under a stated model),
**[measured]** (counted on real data by the scripts in `theory/`; rows and centres, never time), **[estimated]**
(measured counts times per-item constants taken from other reports, which were timed on BioCLIP-2 at n = 768),
**[cited]** (a known result; references at the end were written from memory and not checked), **[conjecture]**.

Data: the suite's fits (`rust/tools/fit.py` defaults) of glove-100-angular (N 1.18M, K 592), yahoo-minilm-384
(N 0.68M, K 339) and laion-clip-512 with text queries (N 1.0M, K 500; out of distribution), 64 groups each, 100
queries each. Sub-clusters are plain k-means inside each cluster: `a4`, `a16`, `a64` are 4, 16 and 64 per cluster by
recursive 4-way splits (a tree), `d16` is one 16-way k-means. Recall is against the exact answer; search inside a
probed unit is exact, so recall is decided by the order alone. A curve probes the same number of units for every
query; "rows to recall r" is the mean number of rows at which the mean recall reaches r. With 100 queries,
differences under about 5% in rows are not findings. Outputs: `theory/out_*.txt`, `theory/model_out.txt`.

---

## 0. Conclusions

1. **Ranking every cluster is not the cost to attack, except under sparse visible sets.** Under the build rule the
   bytes of all centres equal the first-pass bytes of about six clusters at every N, and in time about a quarter
   of one cluster's scan. It becomes the larger part only when a visible set makes clusters nearly empty. [estimated]
2. **No polylogarithmic ranking is available.** Exact top-nprobe of K inner products is as hard as exact search
   (conditional, worst case). Under an additive Gaussian model of a tree the number of nodes that must be kept at a
   level is `K_j · Φ̄(z_T √f_j − z_ε √(1 − f_j))`, f_j the share of the leaf-key variance that level explains: at
   best `nprobe^f · exp(O(√log K))`, polynomial in K when a level explains less variance than its share of log K.
   [proved in the model] On the fits the law holds from groups to clusters (within about 25%) and is optimistic by
   1.5 to 4 times one level below the clusters, and up to 13 times two levels below, on glove and LAION text.
   [measured] Misses add over levels, so the beam must
   widen with depth. A two-level rank (all clusters, then the sub-centres of the best P) scores 3.6 to 10 times
   fewer centres than a flat rank of every sub-centre at the same rows. [measured]
3. **The groups are not a useful level above the clusters.** As probe units they need 3 to 6 times more rows than
   clusters; as a filter on clusters, holding 99% of the best 1% of clusters needs 24 to 41% of the groups.
   [measured] What the groups are good for in the approximate mode is the cost they impose: an order that charges
   for opening a new group opens 15 to 35% fewer groups for at most 12% more rows. [measured]
4. **The key of a node is its own cluster-type key with the total variance**, by the law of total variance; the
   optimal rule for expanding is a threshold on a reservation value `ν_g` defined by
   `E Σ_children (g_c − ν w_c)₊ = ν e_g`, on the same global level λ that stops probing. [proved] An index policy
   with that value is optimal for the area under the recall curve (checked exhaustively on 300 small instances;
   the theorem is cited). Measured, the total-variance key cuts the parents that must be expanded by 15 to 25%
   against the centre score on glove and LAION text; an explicit "best child" term adds nothing over it.
5. **The multiplier on the spread does not depend on the unit's size.** The best ψ in `μ + ψσ` (exact σ) is 3 to 4
   for clusters and for sub-clusters of 125 and 30 rows alike. H's `z(k, n_c)` must not be carried down: it would
   give 1.4 and 0.5. [model, measured]
6. **Sub-clusters cut rows by 2.1 to 3.6 times (16 per cluster) with keys that need nothing but a centre and a
   radius, and would cut them 2.9 to 3.8 times with the exact spread. They do not cut clusters or groups opened,
   which rise by up to 1.7 and 1.6 times.** At 1M and n = 512 the estimated time gain is about 10% (LAION text),
   18% with a group-aware order; 30% and 45% on glove and yahoo, where group setup is cheap. The gain should grow
   with N, as clusters grow and setup is amortised. [measured counts; estimated times]
7. **Keep the cluster as the privileged unit.** Sub-clusters as data about unchanged rows (a label per row, a
   centre and a radius per sub-cluster) leave the layout, the bases, the certificates and the exact paths alone.
   Flat finer clusters buy the same rows (`reports/annideas`) and cost the exact path. [argued from measured costs]
8. **Sub-centres in the top coordinates of the group basis save memory, not time, at 1M**, and lose rows unless
   r is 64 to 128 (of 384 to 512). [measured; estimated] A different use of the groups looks better: **the
   cluster key's spread from the group's covariance** (H with 64 sets of directions in place of K) gets a third
   to two thirds of the exact spread's gain at the cluster level on yahoo and LAION text, at a state that does
   not grow with N. [measured, constants in-sample]
9. **Approximate threshold queries have the same optimal form with one simplification: τ is given.** The key
   `(t − q·m_c)/rad_c` has no constant, adapts to the answer size, and matches the best per-size f′ to within 1% on
   glove and yahoo and 6% on LAION text. The benefit of sub-clusters falls from 1.7–2.5 times at answers of 0.01%
   to 1.15–1.26 times at 10%, where the cluster order is within 5% of the clairvoyant order for its partition
   (0.3% with the exact spread). [proved form;
   measured]
10. **Under visible sets.** The row saving of sub-clusters survives random and correlated visible sets (1.5 to 3
    times), but the centres they score do not shrink with the visible set: at 1% visible they cost more than they
    save, at 5% (correlated) they break even, at 25% they win 1.5 to 2.3 times. The threshold rule of point 4 gives
    the test: do not expand a cluster holding fewer visible rows than its sub-centres cost. A correlated visible
    set of 1% makes every order built from whole-cluster centres read a third to a half of the visible rows for
    recall 0.9; that is a weakness of the cluster order itself, which a hierarchy only partly repairs. [measured]

What I would build, in order, is in section 8; what I found wrong or overstated in the earlier reports in section 9.

---

## 1. Setting

Notation of `../theory.md`: query q, clusters c with n_c rows, centre m_c, μ_c = q·m_c, σ_c(q)² = qᵀΣ_c q,
rad_c² = 1 − |m_c|², σ(q) the standard deviation of μ_c over clusters, T(q) the answer (top-k, or the rows with
q·x ≥ t), N_c = |T(q) ∩ c|, g_c = E[N_c | information], p_c = g_c/n_c, w_c the cost of probing c in row units.

A **tree** has the probe units as leaves. Expanding an internal node g costs e_g (scoring its children's centres,
in row units) and reveals the children's statistics. A cluster with sub-clusters is a node whose leaves are its
sub-clusters; a super-cluster is a node whose children are clusters. Costs per query, from the reports
(BioCLIP-2, n = 768): a row in the cascade about 22 ns, a centre (int8, d = 512) about 22 ns, cluster setup 4 to
17 µs, group setup 55 to 120 µs (an n × n product, so about (n/768)² of that at other n).

---

## 2. The cost in K

### 2.1 What ranking costs next to scanning [estimated]

Under `per_cluster(N)` a cluster holds 2,000·√(N/1M) rows, so K = 500·√(N/1M): both grow as √N. All centres are
K·d bytes; the first deterministic levels of one cluster are about 22 B × n_c. The ratio is K·d/(22·n_c) = 5.8
clusters' worth of bytes at d = 512, **at every N**. In time: 11 µs for K = 500 against 44 µs or more for one
cluster of 2,000 rows, so ranking costs at most a quarter of one probed cluster, also at every N. At 1B the rule
(`rules.py`) gives 64,000 rows a cluster and 15,625 clusters (`PLAN.md` and the brief say 22,000 of 45,000, the
unrounded 2,000·√(N/2M)): 0.35 to 0.5 ms of ranking against 1.0 to 1.4 ms per cluster, if the centres' 8 to 11 MB
stay in cache.

A visible set of share ρ multiplies the rows of a probed cluster by ρ and leaves the ranking alone, so ranking
weighs about 0.25/ρ clusters: of the order of ten to twenty-five at ρ = 1% (`PLAN.md` measured 18 of 24 µs at
nprobe 4, twelve clusters' worth). So the case for
sublinear ranking is the sparse visible set, and there it competes with the planner's scan of the visible rows.
At 1M there is little room between the two: on the three fits the groups as scan units would beat the clusters
only below about 0.2 to 0.7% visible (scaling the random-1% rows of section 7 with the share: clusters need 450
to 2,150 visible rows and K centres, groups 865 to 3,771 rows and 64 centres), where the whole visible set is
2,000 to 4,500 rows and a scan of it costs at most three times as much. At
1B and 1% visible, the same 0.35 to 0.5 ms stands against 450 to 640 visible rows a cluster, a sixth to a half of
a query at nprobe 16 to 64 (at the 50 ns a visible row implied by `PLAN.md`'s 1 µs per sparse cluster). That is the regime a second level of centres would serve.

### 2.2 Lower bounds

- **Exact top-nprobe is exact search over the centres.** The keys `spread` and the rad part of H are
  `[q, β(q)]·[m_c, rad_c/σ(rad)]`, an inner product with one more coordinate (`PLAN.md`), so ranking is maximum
  inner product search over K points in d + 1 dimensions. Proposition 1 of `sublinear/REPORT.md` applies with K in
  place of N: an index with polynomial build answering it in K^(1−δ) would refute the Orthogonal Vectors conjecture
  when d grows faster than log K. [cited; worst case over centre sets, not a statement about k-means centres] H's
  spread term is not an inner product (it is a norm of projections), so it has to be a second stage over a
  candidate set, as `PLAN.md` phase 2 already does.
- **Approximate ranking by hashing** has the exponents quoted in `sublinear/REPORT.md` 3.4: with near-linear
  space, 1 − τ² for targets at cosine τ against a background at 0. The cosine here is between q and the centred
  centres; for text queries it is small. [cited]
- Neither binds at K = 22,000. They say there is no structure to look for that makes ranking polylogarithmic
  with a guarantee; what a tree can do is the next section.

### 2.3 A beam over a tree: the law [proved in the model]

**Model T.** A tree with levels j = 1..D, K_j nodes at level j, K = K_D leaves. The key of a leaf is
U = ε_1 + … + ε_D, where ε_j is shared by the leaves under one level-j node, the ε are independent,
ε_j ~ N(0, v_j − v_{j−1}), v_0 = 0. The key of a level-j node is A_j = ε_1 + … + ε_j (the key its centre would
have). f_j = v_j/v_D is the share of the leaf-key variance explained by level j. The targets are the leaves with
U ≥ T = z_T·√v_D, z_T = Φ⁻¹(1 − L/K) for L targets.

**Proposition 1.** Keep at level j the nodes with A_j ≥ θ_j = √v_D·(z_T f_j − z_ε √(f_j(1 − f_j))),
z_ε = Φ⁻¹(1 − ε). Then (i) every target's level-j ancestor is kept with probability at least 1 − ε; (ii) the
expected number of nodes kept is

    B_j = K_j · Φ̄( z_T √f_j − z_ε √(1 − f_j) ) ;                                              (2.1)

(iii) a target survives all levels with probability at least 1 − (D − 1)ε.

*Proof.* (A_j, U) are jointly Gaussian with Cov = v_j, so A_j | U = u is N(f_j u, v_D f_j(1 − f_j)). For u = T the
ancestor passes θ_j with probability exactly 1 − ε, and for u > T with more, since the conditional mean rises with
u. A_j ~ N(0, v_j) unconditionally, and θ_j/√v_j is the argument in (2.1). (iii) is the union bound. ∎

Reading (2.1), with K_j = K^(g_j) and Φ̄(x) ≈ exp(−x²/2):

    B_j ≈ K^(g_j − f_j) · L^(f_j) · exp( z_ε z_T √(f_j(1 − f_j)) − z_ε²(1 − f_j)/2 ) .          (2.2)

- **Regression to the mean is the mechanism.** The parent of a leaf at height u sits at f_j·u on average: a node's
  centre understates its best children by the share of variance it does not explain. A level that explains little
  (f_j small) keeps almost everything, however good its key.
- **When each level explains at least its share of log K (f_j ≥ g_j)** the beam is at most about
  L^f · exp(z_ε z_T/2), with z_T ≈ √(2 ln(K/L)): more than any power of log K, less than any power of K.
  **When f_j < g_j it is polynomial, K^(g_j − f_j).** (2.2) is a guide to exponents; for values use (2.1): for
  K = 22,000, L = 64, ε = 1% and a middle level of 148 nodes with f = ½, 56 nodes are kept.
- **Depth.** Holding total loss ε over D − 1 levels needs z_{ε/(D−1)} per level, which multiplies each beam by
  about exp((z_{ε/(D−1)} − z_ε) z_T √(f(1 − f))): 1.5 to 2 for going from two levels to four at ε = 1%. So yes,
  the beam has to widen with depth, slowly; the larger effect of depth is that the middle levels sit near f = ½,
  where √(f(1 − f)) is largest.
- **Loss.** The guarantee is on the leaves' own keys (the tree returns the flat order's top L with probability
  1 − (D − 1)ε each); it is a statement in the model, per query direction. Nothing distribution-free is possible:
  one child far from its parent's centre is lost by any beam that does not expand that parent.

Check on synthetic trees (`theory/model.py`, K about 22,000, L = 64, ε = 1%): measured against (2.1), nodes kept
17 | 17 and 133 | 143 (three equal levels), 23 | 24 and 373 | 410 (shares 0.15, 0.25, 0.6), 13 | 7 and 51 | 40
(shares 0.6, 0.25, 0.15), 94 | 100 of 150 (two levels, 0.3 / 0.7). With three equal levels the tree scores
28 + 28·(17 + 133) = 4,200 centres of 21,952; with shares 0.15, 0.25, 0.6 it scores 11,100. In-model the saving
is a factor of 2 to 5, not a change of order.

### 2.4 The law on the fits [measured]

f is measured per query as the variance over leaves of the parent's centre score divided by that of the leaf's
(`theory/hier.py`, section C). "Kept" is the share of parents that must be kept, by centre score, to hold 99% of
the best leaves by centre score; measured | (2.1).

| parent → child | f (glove / yahoo / LAION text) | kept, best 1% of leaves | kept, best 0.2% |
|---|---|---|---|
| groups → clusters | 0.58 / 0.66 / 0.62 | .41 \| .39, .24 \| .29, .36 \| .34 | .20 \| .25, .14 \| .16, .25 \| .20 |
| clusters → 16 sub-clusters (a16) | 0.79 / 0.79 / 0.77 | .25 \| .16, .11 \| .16, .40 \| .17 | .22 \| .07, .06 \| .07, .33 \| .08 |
| a16 → a64 | 0.87 / 0.89 / 0.84 | .39 \| .09, .12 \| .08, .43 \| .12 | .41 \| .03, .06 \| .03, .38 \| .05 |
| groups → a16 | 0.46 / 0.52 / 0.49 | .53 \| .55, .44 \| .47, .69 \| .52 | .49 \| .41, .28 \| .32, .58 \| .37 |

- From groups to clusters the law is right to within about 25%, on all three. Below the clusters it holds on yahoo
  (within a factor of 2 at the last level) and is optimistic by 1.5 to 4 times on glove and LAION text one level
  down and by 4 to 13 times two levels down: the best sub-centres are not Gaussian draws around their cluster's
  centre (small, outlying sub-clusters), so a beam sized by the model would lose them.
- f is nearly the same on the out-of-distribution text queries as on the others (0.62 against 0.58 and 0.66 for
  groups). Text queries are not worse because a level explains less; they are worse below the cluster level
  because the tail is.
- **The groups do not prune clusters.** f ≈ 0.6 with K_j = 64: a quarter to two fifths of the groups hold the best
  1% of clusters. At 1M that saves nothing (64 + 0.4·K centres against K, of an 11 µs job). At 1B the same 64
  groups would stand over 16,000 to 22,000 clusters and explain less.

End to end, as an emulation of a larger K (the a16 leaves, about 5,400 to 9,500 per fit, as the "clusters", the
fit's clusters as the middle level; section B of `hier.py`): expanding the best P parents by a parent key and
ranking their children reaches recall 0.9 within 5% of the rows of the flat order over every leaf at P = 64–128,
16 and 16–32 on glove, yahoo and LAION text with the best parent key of section 3 (L, the leaves probed, is 222,
34 and 131), that is 1,616–2,640 of 9,472, 595 of 5,424 and 756–1,012 of 7,961 centres scored: **3.6–5.9, 9 and
8–10 times fewer**. With the plain `spread` key on the parents LAION text needs P = 128 (3 times fewer). In
`PLAN.md`'s terms (candidates scored per unit probed) that is m = 2 to 9: its m = 2, found for re-ranking the
same clusters by H, is the low end here, and glove needs 5 to 9.

**Answer to sub-question 1.** O(polylog K) ranking: no, in any model that fits these data. O(nprobe·K^o(1)) in the
Gaussian tree model, and measured 3.6 to 10 times fewer centres with one middle level at up to 5% more rows.
Since ranking is a quarter of one cluster's scan when unfiltered, the prize is confined to sparse visible sets at
large N.

---

## 3. The key of a node, and when to expand it

### 3.1 A node seen as one cluster [proved identity]

For a node g with children c (row shares n_c/n_g), centre M_g = Σ (n_c/n_g) m_c,

    Σ_g = C_g + Σ_c (n_c/n_g) Σ_c ,      C_g = Σ_c (n_c/n_g)(m_c − M_g)(m_c − M_g)ᵀ ,                (3.1)

so the variance of the scores of the node's rows is σ_g(q)² = qᵀC_g q + mean_c σ_c(q)²: between-child plus
within-child. Two consequences.

- Everything in `../theory.md` applies to a node taken whole, with (μ_g, σ_g, n_g): its row density is
  p̄_g = Ḡ((τ − μ_g)/σ_g) under the location-scale model, and the invariance of Theorem 11 holds for every term
  (μ_g shifts by β, qᵀC_g q and σ_c² scale by a²).
- **A cluster's own key is already the node key for its sub-clusters.** Σ_c as H models it (top directions of the
  cluster's covariance about its centre) contains the spread of the sub-centres. Nothing new has to be stored for
  the parent when sub-clusters are added under a cluster. For a super-cluster the state is a low-rank C_g plus
  the mean of its clusters' spreads, as `PLAN.md` proposes.

### 3.2 The optimal rule with expansion costs [proved]

Assumptions. (S1) Recall is additive over leaves (Lemma 1 of `../theory.md`; true for top-k and for threshold
answers). (S2) What expanding g reveals (its children's statistics) changes the expected value of no leaf outside
g's subtree, and the law of what it reveals does not depend on what was seen elsewhere. (S2) holds when τ is
given (threshold queries) or fixed in advance by the plug-in τ* of `../theory.md` 3.1; it ignores what scanned
scores teach about τ, which that note bounds as second order in-model.

Define, for a price λ per row,

    V_c(λ) = (g_c − λ w_c)₊                              for a leaf,
    V_g(λ) = max( 0 ,  n_g(p̄_g − λ)·[whole] ,  E[ Σ_{c child of g} V_c(λ) | g ] − λ e_g )        for a node,

where the middle term is present when the node can be scanned whole without expanding (a cluster can; then
n_g and p̄_g use its visible rows), and ν_g = sup{λ : the last term is positive}.

**Theorem 2.** Under (S1), (S2), among all policies that choose expansions and probes adaptively on the revealed
statistics, the expected value of recall·k − λ·cost is maximised by: expand g if and only if the third term is the
largest (in particular ν_g > λ), scan it whole if the second is, probe a leaf if and only if g_c/w_c > λ. Hence
(as Theorem 3 of `../theory.md`) this rule with one global λ maximises expected recall among all policies with the
same expected cost over the query distribution.

*Proof.* By (S1) the objective is a sum over the root subtrees of (answers found − λ·cost spent) in each. By (S2)
the choices in one subtree neither inform nor constrain another, so each term is maximised separately. Induct on
height: once g is expanded its children are separate subproblems worth V_c(λ) each by induction, so expanding is
worth their expected sum minus λe_g; scanning whole is worth n_g(p̄_g − λ); doing nothing is worth 0. V_g is
non-increasing in λ, so "expand" is a threshold on λ. The budget form is the Lagrangian argument of Theorem 3,
unchanged. ∎

For a node whose children are leaves, ν_g is the root of

    E[ Σ_c (g_c − ν w_c)₊ ] = ν e_g ,                                                             (3.2)

a reservation value of Weitzman's kind. It lies between the node's whole density Σg_c/(e_g + Σw_c) and its best
child's density, tends to the best child's as e_g → 0, and to (expected answers)/e_g as e_g grows. (ν > λ if and
only if some rule over the children collects more than λ per unit of cost including e_g: the Dinkelbach form.)

**The global threshold extends to nodes** in this exact sense: one λ, compared with ν_g for expanding and with p_c
for probing. It is not a threshold on the probability that the node holds an answer.

**For the area under the recall curve** (Smith's rule in `../theory.md`, Theorem 2) the tree is a scheduling
problem in which finishing a job (expanding a node) releases new jobs of random type. That is a branching bandit,
for which the policy "always work on the available item of largest index" is optimal, the index of a leaf being
g_c/w_c and that of a node the ν of (3.2) [cited: Weiss 1988; Bertsimas and Niño-Mora 1996]. Checked here by
exhaustive dynamic programming on 300 random instances (three nodes, two children each, three types): the index
policy attains the optimum on every one (largest excess 0), while the same policy with the whole-node density as
the node's index is worse on 109 of them (mean +0.8%, worst +9%); the threshold rule equals the Lagrangian
optimum to 4·10⁻¹⁵ (`theory/model_out.txt`).

**Two corollaries used below.**

- *Never expand a sparse node* [proved]. Expanding beats scanning whole only if the rows it lets one skip are
  worth more than e_g: E Σ_c n_c(1 − p_c/λ)₊ > e_g (the difference of the two terms, divided by λ). The left side
  is at most the node's rows. So **a node holding no more (visible) rows than its expansion costs in row units is
  scanned whole or not at all**, whatever the model. With 16 sub-centres at about one row each, that is a cluster
  with 16 or fewer visible rows; a random 1% visible set at 1M leaves 20.
- *Expansion is nearly free otherwise* [estimated]. Unfiltered, e_g is 16 against 2,000 rows, so ν_g is the
  density of the best sub-clusters the cluster is expected to hold, and a cluster should be expanded whenever any
  of it should be read.

### 3.3 The form of the key [model, measured]

With Gaussian children, μ_c ~ N(μ_g, s_g²) (s_g² = qᵀC_g q) and a common within-child spread σ_w, two closed
forms bracket ν_g: the whole-node ζ_g = (τ − μ_g)/√(s_g² + σ_w²), and the best-child form
(τ − μ_g − m_F s_g)/σ_w with m_F the expected maximum of F standard normals (1.77 for 16). When the ratio s_g/σ_w
is the same for every node, the two give the same order. They differ only through the variation of that ratio
across nodes.

Measured (section B; parents that must be expanded to hold 95% of the first L leaves of the flat sub-cluster order,
L the leaves to recall 0.9; a16, k = 10; all spreads exact):

| parent key | glove (L/16 = 13.9) | yahoo (2.1) | LAION text (8.2) |
|---|---|---|---|
| centre score μ_g | 73 | 11.8 | 52 |
| `spread`, f′ = 0.4 | 65 | 12.5 | 48 |
| μ_g + 3σ_g (total spread, (3.1)) | 61 | 10.9 | 40 |
| μ_g + m_F s_g (between-child only) | 74 | 10.5 | 40 |
| μ_g + m_F s_g + 2σ_w | 64 | 10.7 | 37 |
| oracle: the best child's key | 45 | 8.4 | 29 |

- The total-variance key is as good as any form with an explicit best-child term, as the common-ratio argument
  predicts. It is the cluster's own H-type key. On LAION text it lets P = 16 to 32 parents match the flat order
  where the centre score and `spread` need 64 to 128.
- No key built from the parent's moments reaches the oracle (the parent holding one outlying good sub-cluster):
  25 to 35% more parents than the oracle on every dataset. That gap is the price of the level.
- A parent stands in for its children far less efficiently than L/16: the first L leaves lie in 3.9 to 4.4 times
  L/16 parents (54, 9.3 and 34 parents for 222, 34 and 131 leaves).

### 3.4 The multiplier does not depend on the unit's size [model, measured]

By Proposition 9 of `../theory.md` the multiplier of a linear rule is the marginal ζ at the budget, ψ = Ḡ⁻¹(λ),
and λ is answers per row at the margin: a property of the budget, not of the unit. Measured best ψ in `μ + ψσ`
with the exact σ, by rows to recall 0.9 (k = 10): clusters 3.0 / 4.0 / 4.0 (glove / yahoo / LAION), a16
3.0 / 3.0 / 4.0, a64 3.5 / 3.5 / 3.0; at k = 100, 2.5 to 3.5 everywhere. z(10, n) is 2.6 for 2,000 rows, 1.4 for
125 and 0.5 for 31. So H's `α·z(k, n_c)` must not be used below the cluster level, and for row budgets it is
better replaced by a constant at the cluster level too, as `../theory.md` 3.4 argued.

### 3.5 What a sub-cluster's key can be built from [measured]

Rows to recall 0.9, share of N in %, k = 10, flat order over the sub-clusters:

| key of a sub-cluster | glove a16 / a64 | yahoo a16 / a64 | LAION text a16 / a64 |
|---|---|---|---|
| centre score | 5.18 / 4.05 | 0.83 / 0.54 | 2.83 / 1.67 |
| centre + 0.2·rad (`annideas`; not invariant) | 3.36 / 2.79 | 0.67 / 0.42 | 1.71 / 1.08 |
| `spread`: f′ = 0.4, σ(q) and σ(rad) over the sub-centres | 2.87 / 2.63 | 0.70 / 0.43 | 1.89 / 1.23 |
| `spread`, best f′ per dataset | 2.86 / 2.19 | 0.66 / 0.40 | 1.62 / 1.02 |
| μ_s + ψ·σ_C(q)·rad_s/rad_C (the cluster's exact spread, scaled) | 3.27 / 2.44 | 0.70 / 0.43 | 1.33 / 0.87 |
| μ_s + ψ·σ_s(q), exact | 2.05 / 1.15 | 0.48 / 0.22 | 0.93 / 0.36 |
| clairvoyant | 0.10 / 0.03 | 0.13 / 0.04 | 0.10 / 0.04 |

- A radius term is worth 1.2 to 1.7 times in rows over the bare centre score, as `annideas` found. The invariant
  `spread` key carries over to sub-clusters as it is, with its statistics taken over the sub-centres, but at
  f′ = 0.4 it is not better than `+0.2·rad` on these queries (15% better on glove, 4% worse on yahoo, 10% worse on
  LAION text at 16); the best f′ drifts up as units get smaller (0.2–0.5 for clusters, 0.3–0.6 at 16, 0.6–1.0 at
  64), so f′ = 0.4 is too small below the cluster level on text. The case for `spread` over a fixed radius weight
  is the invariance argument of `../theory.md` (a fixed weight is right for one scale of query only), not these
  rows.
- **The exact spread of a sub-cluster is worth another 1.4 to 3.4 times in rows**, more than at the cluster level
  (1.1 to 1.5 there), and most on text. This is the largest untapped quantity in the table.
- **Borrowing the cluster's spread gets part of it on text only**: the cluster's exact σ_C(q) scaled by
  rad_s/rad_C is 1.2 to 1.4 times better than `spread` on LAION text (1.33 against 1.62–1.89 at 16), no better
  on yahoo and worse on glove at 16. Sub-clusters of one cluster share its covariance shape only loosely. It
  costs nothing new where H's σ̂_c is already computed for the expanded clusters. Variances in the group's basis
  recover little more: section 5.3.

---

## 4. Why sub-clusters help, and what they do not touch

### 4.1 Rows [measured]

Rows to recall 0.9 (share of N, %), k = 10, by leaves per cluster, with `spread` f′ = 0.4 and with the exact
spread (best ψ):

| leaves per cluster | glove | yahoo | LAION text |
|---|---|---|---|
| 1 (clusters) | 6.51 · 5.86 | 2.49 · 1.82 | 4.06 · 2.67 |
| 4 | 4.41 · 3.29 | 1.43 · 0.93 | 2.52 · 1.66 |
| 16 | 2.87 · 2.05 | 0.70 · 0.48 | 1.89 · 0.93 |
| 64 | 2.63 · 1.15 | 0.43 · 0.22 | 1.23 · 0.36 |
| clairvoyant, 1 / 16 / 64 | 1.07 / 0.10 / 0.03 | 1.06 / 0.13 / 0.04 | 0.85 / 0.10 / 0.04 |

- 16 sub-clusters: 2.3, 3.6 and 2.1 times fewer rows at recall 0.9 with a key that needs no new state; at k = 100,
  1.8, 2.5 and 2.0 times. This reproduces `annideas` (1.9 to 3.7 times on BioCLIP-2) on three other datasets and
  with an invariant key.
- Rows fall as (leaves)^(−γ) with γ = 0.39, 0.51, 0.48 for the exact spread and 0.22, 0.42, 0.29 for `spread`:
  with the cheap key the gain from 16 to 64 is small on glove (the key, not the partition, is the limit there).
- The reason is in the last row: the answer occupies few leaves, so the row density of the best leaves rises
  almost in proportion to the leaf count (the clairvoyant order needs 8 to 11 times fewer rows at 16). The
  practical keys realise a fifth to two fifths of that.
- A single 16-way k-means (d16) and two nested 4-way splits (a16) are within 20% of each other, either way.

### 4.2 What a probe costs besides rows [measured counts, estimated times]

At recall 0.9, k = 10, flat order with `spread` (section G):

| | rows | clusters opened | groups opened |
|---|---|---|---|
| glove: clusters / a16 / a64 | 77,013 / 33,919 / 31,069 | 38 / 54 / 98 | 14.2 / 19.0 / 29.4 |
| yahoo | 16,869 / 4,751 / 2,899 | 9.0 / 9.3 / 11.8 | 5.2 / 5.3 / 6.6 |
| LAION text | 40,573 / 18,897 / 12,336 | 20 / 34 / 51 | 8.8 / 13.7 / 19.0 |

Sub-clusters take their rows from more clusters and more groups. With 22 ns a row (25 when gathered), 12 µs a
cluster and 55 µs·(n/768)² a group (1.5, 14 and 24 µs), the model time is

| | clusters | a16 | a64 |
|---|---|---|---|
| glove (n 128) | 2.17 ms | 1.53 (−30%) | 2.00 |
| yahoo (n 384) | 0.55 | 0.31 (−45%) | 0.31 |
| LAION text (n 512) | 1.34 | 1.20 (−10%) | 1.37 (+2%) |

and at BioCLIP's n = 768 with the 0.12 ms a group of `rust/REPORT_CLUSTERSIZE.md` the LAION counts would give a
loss (93 against 101 thousand row-equivalents). These are estimates from constants timed elsewhere; what they
establish is the shape: **at 1M the rows are two fifths to a half of the cost once sub-clusters have cut them,
and 64 per cluster is past the optimum.** `annideas` says the same of its own estimate (groups 1.4 ms, cluster setup 0.5 ms,
items 0.8 ms).

Two further caveats on reading rows as cost.
- The planner's rule under a visible set (`NEXT.md`: the index path pays only above about 64 + 0.186·n_c visible
  rows of a cluster) says a cluster's cost on the index path is partly per cluster row, not per selected row. The
  probed sub-clusters of a cluster are such a selection: on LAION, 3.9 sub-clusters a cluster, about 490 of 2,000
  rows, at the crossover. Below it the rows are scored from full vectors at about 73 ns (n = 512), three times
  the cascade's cost, and the row saving is spent. [estimated; needs timing]
- Per-cluster setup may itself fall with fewer selected rows (tables per unit); not known.

**Scaling [model].** Setup per cluster and per group is fixed while a cluster grows as √N, so the rows' share of
the cost grows with N, and so does what sub-clusters save: at 100M (16,000 rows a cluster) the 12 µs setup is 3%
of a cluster's scan against 27% at 1M. Sub-clusters are worth least at 1M and should be judged at 16M or more
(or at `--per-cluster 8000` on 1M, which reproduces the ratio).

### 4.3 An order that knows what a group costs [measured counts]

The cost of a set of probes is rows + (clusters opened) + (groups opened): a knapsack with shared set-up costs.
A greedy order that lowers the key of a unit in a group not yet opened by π·σ(q), and re-evaluates after each
probe (section G):

| at recall 0.9 | π = 0 | π = 0.25 | π = 0.5 |
|---|---|---|---|
| glove, clusters: rows, groups | 77,013, 14.2 | 80,238, 11.1 | 91,448, 10.5 |
| glove, a16 | 33,919, 19.0 | 34,981, 13.5 | 46,519, 12.5 |
| yahoo, a16 | 4,751, 5.3 | 4,868, 4.5 | 5,270, 4.0 |
| LAION text, clusters | 40,573, 8.8 | 38,615, 7.0 | 47,785, 7.0 |
| LAION text, a16 | 18,897, 13.7 | 20,044, 10.9 | 21,078, 8.9 |

At π = 0.25, 15 to 29% fewer groups for −5 to +6% rows; on LAION a16 at π = 0.5, 35% fewer groups for 12% more
rows, which takes the model time from 1.20 to about 1.11 ms (−18% against clusters). The principled form is
Theorem 2 with the group as a node whose expansion cost e_g is its setup: a group is opened when the expected
value of its units above λ exceeds λ·e_g. π is a stand-in for that and was not tuned beyond the grid. This is
the one use of the groups as a level that the measurements support.

---

## 5. How sub-clusters should sit under a cluster

### 5.1 As data about unchanged rows

The proposal that keeps both strengths: rows stay in cluster order sorted by key, codes stay in the group's dual
basis, centres and groups are untouched. A sub-cluster is (i) a centre, (ii) a radius, (iii) membership. For
membership I would store **a label per row, in row order** (4 bits for 16, a byte up to 256) in place of the
position lists of the `annideas` ruling (u16 positions grouped by sub-cluster, 2 B a row):

- probing sub-clusters S of a cluster is then a row mask (label ∈ S), built by one pass over the cluster's
  labels (1 KB for 2,000 rows at 4 bits) and ANDed with the session's visible bitmap; the approximate mode runs
  over that mask as it already runs over a visible set, and visible counts per sub-cluster are a popcount;
- an insert places its row in sort order and its label goes with it; position lists would be invalidated by every
  insert into the cluster (all later positions shift);
- it is a quarter to a half of the bytes.

The pass over labels is per cluster row, not per selected row; at 64,000 rows a cluster (1B) it is 32 KB against
the selected rows' first level, which is comparable, so a coarser first split would be wanted there. [argued;
nothing here is timed]

### 5.2 Sub-centres in the top coordinates of the group's basis [measured, estimated]

q·s = q·m_c + pᵀz with p = V_gᵀq and z = V_g⁻¹(s − m_c); keeping the first r coordinates (the basis is ordered by
the dual metric's eigenvalues) costs r multiply-adds a sub-centre once p is known. Share of the variance of
q·(s − m_c) left out, and rows to recall 0.9 (a16, `spread` key; exact scores in the last column):

| r | glove (n 128) | yahoo (n 384) | LAION text (n 512) |
|---|---|---|---|
| 16 | 0.33, 4.57% | 0.27, 1.11% | 0.36, 3.04% |
| 32 | 0.21, 4.19% | 0.14, 0.81% | 0.23, 2.47% |
| 64 | 0.08, 3.35% | 0.05, 0.76% | 0.14, 2.08% |
| 128 | 0, 2.87% | 0.014, 0.70% | 0.07, 2.00% |
| exact | 2.87% | 0.70% | 1.89% |

- The offsets of sub-centres are not concentrated in a few coordinates: r = 16 loses a third of the variance and
  about half of the sub-clusters' gain; r = 64 costs 9 to 17% in rows; text needs r = 128 to come within 6%.
- **Time.** Scoring 16 int8 sub-centres directly is 16 × 22 ns = 0.35 µs per expanded cluster, under 1% of the
  cluster's scan. The basis route needs p in every group that holds a candidate parent, before those groups are
  probed: the first r rows of the projection, r·n multiply-adds a group. For P = 64 candidates in about 15 groups (the candidate parents of section 3.3 lie in 6 to 22)
  at r = 64, n = 512 that is 0.49M + 0.07M against 0.52M for the direct route: no saving. It saves time only when
  candidates × fan-out greatly exceeds r × groups, that is with hundreds of sub-clusters per cluster (large N).
- **Memory.** int8 sub-centres are d bytes per 125 rows: 4 B a row at d = 512, about 10% on top of the resident
  codes (39 B). In r = 64 coordinates they are 0.5 B a row. This is the real argument, and it is modest.
- A sub-centre stored as one more coded row of its cluster (`annideas`) was not tested here. The level-3 code's
  error (sd 0.011 at 20 B on BioCLIP-2) is as large as the whole spread of q·(s − m_c) on LAION text (rms 0.013),
  so I would not expect 20 B to rank sub-centres. [conjecture]

So: store sub-centres as int8 vectors at 1M to 100M; consider basis coordinates only when their memory matters.
Either way they are data beside the index; neither touches the bases or the codes.

### 5.3 The spread of a unit from variances in the group's basis [measured]

Since the exact spread is worth so much (3.5), can it be had from the group, whose basis the query is projected
into anyway? Two models (`hier.py`, section I). (a) In the group's dual coordinates:
σ_u(q)² = Σ_{i<r} p_i² v_{u,i} + ρ_u Σ_{i≥r} p_i² v̄_{g,i}, with v_{u,i} the variance of the unit's rows in
coordinate i about its centre, v̄_g the same over the group, ρ_u one scalar per unit. (b) The group's shape alone:
σ_u(q)² = (rad_u²/tr Σ_g)·[Σ_{i<r} λ_i (u_i·q)² + λ̄(|q|² − Σ_{i<r}(u_i·q)²)], with (λ_i, u_i) the top eigenpairs
of the group's within-cluster residual covariance Σ_g; nothing per unit but its radius. Rows to recall 0.9
(% of N), key `μ + ψσ`, best ψ per cell:

| units | spread of the unit's scores | glove | yahoo | LAION text |
|---|---|---|---|---|
| clusters | `spread` f′ = 0.4 / best f′ | 6.51 / 6.47 | 2.49 / 2.34 | 4.06 / 3.80 |
| clusters | (a), r = 8 (r up to n gives the same) | 5.93 | 2.14 | 3.33 |
| clusters | (b), r = 16 / r = 64 | 6.69 / 6.52 | 2.13 / 2.10 | 3.44 / 3.01 |
| clusters | exact | 5.86 | 1.82 | 2.67 |
| d16 | `spread` f′ = 0.4 | 3.26 | 0.65 | 1.54 |
| d16 | (a), r = 8 | 3.06 | 0.70 | 1.32 |
| d16 | (b), r = 64 | 3.03 | 0.72 | 1.19 |
| d16 | exact | 1.94 | 0.46 | 0.75 |
| a64 | `spread` f′ = 0.4 | 2.63 | 0.43 | 1.23 |
| a64 | (a), r = 8 / (b), r = 64 | 2.25 / 2.50 | 0.44 / 0.43 | 1.05 / 0.91 |
| a64 | exact | 1.15 | 0.22 | 0.36 |

- **At the cluster level a spread taken from the group gets a third to nine tenths of what the exact one gets**,
  on all three with (a) (glove 5.93 against 6.47 and 5.86) and on yahoo and LAION text with (b) (LAION 3.01–3.44
  against 3.80 and 2.67; glove none). In (a) the unit's own r variances make no difference: it is the group's
  profile with one scalar per cluster. So the clusters of a group share a covariance shape well enough for the
  order. (b) is H with the group's directions in place of each cluster's: 64·r·d bytes at any N (0.5 MB at
  r = 16, d = 512, against H r16's 8 MB per million rows) and 64·r·d multiply-adds a query (0.5M, twice the
  centre ranking, against 4.1M), with no group setup. It was not among the bases the README tried (one global
  basis, or each cluster's own). The ψ here is the best of eight on the same queries, so this needs the README's
  replay with fixed constants before it is believed (section 8, X2b).
- (a) needs p in all n coordinates for every group holding a candidate, which is the group setup the approximate
  mode is trying to avoid; (b) does not.
- **Below the cluster level neither gets much**: 15 to 25% on LAION text, nothing on glove and yahoo, against the
  2 to 3.4 times of the exact spread. A sub-cluster's covariance is its own.

### 5.4 Should the privileged unit change?

No. [argued from measurements]

- Flat clusters of the sub-clusters' size read the same rows as nested ones of equal leaf count (`annideas`: flat
  4,096 against 4 per cluster) and no fewer than 16 per cluster; so the approximate mode gains nothing from
  flatness.
- The exact path prefers larger clusters (`rust/REPORT_CLUSTERSIZE.md`: 4,000 a cluster is 8 to 17% faster for
  text), because its per-cluster work (cut, setup, tables) is paid for nearly every cluster; a 16 times finer
  unit multiplies that work by 16.
- The groups and their dual bases are fitted over clusters' residuals; a finer unit shortens residuals by about
  14% (the K^(−0.053) law) and leaves the query side alone (`groups/REPORT.md`), so the certificates barely gain.
- Per-cluster visible counts are cheap because a cluster is a contiguous row range. A finer privileged unit keeps
  that but has too few visible rows per unit under any sparse set (section 7).

Nested sub-clusters let each mode have its own granularity: the exact path its 2,000 to 64,000 rows, the
approximate mode its hundred or so.

---

## 6. Approximate threshold queries

### 6.1 The objective and the optimal form [proved]

Recall = (answers found)/(answers), cost = rows read (plus set-up). Recall is additive over units, so Lemma 1 and
Theorems 1 to 3 of `../theory.md` and Theorem 2 above apply as they stand, with g_c = n_c F̄_c(t). Three things are
simpler than for k-NN.

- **t is given**, so the ζ-fan is evaluated at its true apex: ζ_c = (t − μ_c)/σ_c, no τ to estimate, no coupling
  between units. Under independent units a policy that adapts to scanned scores gains nothing: in Theorem 10 of
  `../theory.md`, E[N_c | everything outside c] = g_c. [proved]
- **The key needs no constant.** With σ_c ∝ rad_c the order by (t − μ_c)/rad_c is the fan under the shared-shape
  assumption of `../theory.md` 6.2, whatever the constant of proportionality, and it is invariant under the query
  shift (t moves with the scores).
- **The expected answer size is known before reading**: Λ(t) = Σ_c n_c Ḡ(ζ_c). A recall target is then "probe in
  order until the model's expected answers found reach r·Λ(t)", which depends on calibrated Ḡ, as for k-NN
  (README finding 8).

For a linear key the right multiplier is the marginal ζ, which falls as the answer grows (a larger answer puts the
margin at a higher density). A fixed f′ therefore cannot be right across answer sizes.

### 6.2 Measured

Share of rows (%) read to recall 0.9, by answer size (share of N):

| answer | units | `spread` 0.4 | `spread`, best f′ (its value) | (t − μ)/rad | ζ(t), exact σ | clairvoyant |
|---|---|---|---|---|---|---|
| glove 0.01% | C / a16 | 9.58 / 5.40 | 9.20 (0.3) / 5.46 (0.3) | 8.96 / 5.28 | 8.64 / 4.38 | 4.53 / 0.83 |
| glove 1% | C / a16 | 33.7 / 24.2 | 30.6 (0.15) / 23.3 (0.3) | 30.5 / 23.1 | 29.9 / 21.6 | 28.7 / 17.7 |
| glove 10% | C / a16 | 57.7 / 49.5 | 54.2 (0.15) / 47.3 (0.15) | 54.2 / 47.3 | 53.8 / 46.5 | 53.7 / 45.6 |
| yahoo 0.01% | C / a16 | 3.50 / 1.31 | 3.25 (0.15) / 1.32 (0.3) | 3.26 / 1.32 | 2.84 / 0.91 | 1.94 / 0.43 |
| yahoo 1% | C / a16 | 15.6 / 9.47 | 14.3 (0.15) / 8.92 (0.15) | 14.3 / 8.94 | 13.0 / 7.87 | 12.6 / 7.22 |
| yahoo 10% | C / a16 | 48.7 / 38.3 | 45.5 (0.15) / 35.9 (0.15) | 45.3 / 35.9 | 43.2 / 34.2 | 43.1 / 33.6 |
| LAION text 0.01% | C / a16 | 5.86 / 2.87 | 5.89 (0.5) / 2.64 (0.7) | 6.20 / 2.80 | 4.20 / 1.69 | 2.51 / 0.55 |
| LAION text 1% | C / a16 | 20.1 / 13.6 | 19.8 (0.3) / 13.7 (0.3) | 20.3 / 13.8 | 17.2 / 11.5 | 16.5 / 10.3 |
| LAION text 10% | C / a16 | 44.5 / 36.1 | 43.4 (0.15) / 35.7 (0.3) | 43.4 / 35.7 | 41.7 / 34.2 | 41.6 / 33.8 |

- The best f′ falls (never rises) with the answer size on every dataset, as predicted, and f′ = 0.4 is 1 to 7%
  worse than the best at answers of 10%.
- **(t − μ)/rad is within 1% of the best f′ per size on glove and yahoo and within 6% on LAION text**, with
  nothing to fit. For threshold queries I would use it in place of `spread`.
- **The hierarchy matters less as the answer grows.** 16 sub-clusters save 1.7, 2.5 and 2.2 times at 0.01%,
  1.3, 1.6 and 1.5 at 1%, 1.15, 1.26 and 1.21 at 10%. At 10% the cluster order with the exact spread is within
  0.3% of the clairvoyant order (the constant-free key within 1 to 5%): the partition, not the key, is the limit, and reading 42 to 54% of the rows for
  recall 0.9 of a 10% answer is no bargain against the exact path. `NEXT.md` already hands large answers to the
  exact path; these numbers put the handover at answers of a few per cent.
- Sub-clusters still cut the clusters and groups opened less than the rows, as in 4.2.

A threshold query's answer can exceed any nprobe-sized set of units, so its stopping rule has to be the global λ
(or the predicted-recall form above), not a count.

---

## 7. Visible sets, and writes

### 7.1 Measured under four visible sets

k = 10 among visible rows; units ordered by `spread` from whole-unit centres; units with no visible row skipped.
"Random" sets are uniform; "correlated" sets are the 5% and 1% of rows nearest one corpus row (a topic). Visible
rows read to recall 0.9, and (in brackets) that plus the centres scored, one row-equivalent each: K for clusters,
K + 32 per parent of a probed leaf for two-level a16 (section F).

| visible set | glove: clusters / a16 | yahoo | LAION text |
|---|---|---|---|
| random 1% | 2,150 (2,742) / 1,136 (6,111) | 450 (789) / 234 (1,608) | 988 (1,486) / 519 (3,138) |
| random 25% | 23,173 (23,765) / 12,309 (15,321) | 5,657 (5,996) / 1,883 (2,650) | 13,346 (13,845) / 6,293 (8,136) |
| correlated 5% | 11,663 (12,212) / 7,518 (12,116) | 8,228 (8,523) / 3,838 (6,881) | 8,858 (9,132) / 4,918 (7,476) |
| correlated 1% | 4,088 (4,456) / 2,547 (7,200) | 3,173 (3,350) / 1,864 (4,783) | 4,835 (4,946) / 3,039 (5,387) |

- **The row saving survives every visible set** (1.6 to 3.0 times), including the correlated ones.
- **The centres do not shrink with the visible set.** At random 1% two-level sub-clusters cost twice what
  clusters do; at correlated 5% they break even or win slightly; at random 25% they win 1.5 to 2.3 times. The
  corollary of 3.2 is the switch: expand a cluster only if its visible rows exceed its sub-centres' cost by a
  margin. It needs the per-cluster visible counts the index already has, and it is the same kind of rule as
  `H_MIN_ROWS_PER_K`. With it, sub-clusters cost nothing where they do not help. [the rule is proved; the
  crossover in rows needs timing]
- Under correlated sets the clusters holding no visible row (38 to 78% of them at 1%) are skipped by count, which
  is why a clustered set is cheaper to rank than a random one of the same size.
- **A weakness that is not the hierarchy's.** Under the correlated 1% set every order from whole-cluster centres
  reads 35 to 48% of the visible rows for recall 0.9 (6.5%, 2.5% and 4.1% of the rows unfiltered; 7 to 18% under
  random 1%). The visible rows of a cluster are then the ones near the topic, not a sample of the cluster, and
  q·m_c misplaces them. The clairvoyant order needs 3 to 5 times fewer rows at the cluster level and 16 to 36
  times fewer with a16, so the information exists at the finer level; the keys do not reach it. Sub-clusters
  repair 1.6 times of it. At that size the planner's scan of the visible rows (7,000 to 12,000 rows) costs about
  twice the approximate mode at recall 0.9, so the practical answer is the scan. [measured]
- The groups as scan units are worse than clusters at every visible set tested (865 to 3,771 against 450 to
  2,150 rows at random 1%, with 8 to 17 of the 64 groups probed).

### 7.2 Every candidate against the two things the index must keep

Approximate answers are decided exactly per returned item, so no structure here can return a wrong item; only
recall and cost are at stake. "Checking" below means the per-group dual bases, the codes written in them, the
certified intervals, rows sorted by key in a cluster, and the exact paths that share that layout.

| candidate | checking through the dual basis | random visible sets | correlated visible sets | verdict |
|---|---|---|---|---|
| Sub-clusters as labels over unchanged rows (5.1) | untouched: no row moves, no basis or code changes, exact paths do not read the labels | rows −1.9 to −3 times at 1% to 25%, but the sub-centres cost more than the rows at 1%: needs the rule "do not expand below e_g visible rows" (3.2), then never worse than clusters | rows −1.6 to −2.1 times; break-even at 5%, a loss at 1% without the rule; counts per sub-cluster by popcount of (label mask ∧ visible) | build, behind the rule; judge by time at ≥ 8,000 rows a cluster |
| Rows re-ordered by sub-cluster inside a cluster | costs the exact path one range search per sub-list and in-place block reads (`annideas` ruling: 1–5%) | as above | as above, with contiguous counts | no: labels give the same without the cost |
| Flat finer clusters (the privileged unit shrinks 16 times) | per-cluster work (cut, setup, tables) × 16 on the exact path, which prefers larger clusters; bases refitted; member side of the bounds 14% tighter | 16 times the centres to rank over nearly empty units | units small, counts cheap, ranking still 16 times | no |
| A second level of centres over the clusters (2.4) | untouched (an index over centres) | the only regime it serves: ranking shrinks 3.6 to 10 times while rows rise up to 5%; needs visible counts summed per node | nodes with no visible cluster skipped; otherwise as random | not at 1M; measure at K ≥ 5,000 |
| The groups as that level, or as probe units | untouched | 1.8 to 3.2 times more visible rows than clusters at 1% | worse than clusters at 1% and 5% | no |
| A group-aware order (4.3) | untouched; it lowers the number of group setups, the cost the dual basis imposes on the approximate mode | under a sparse set a group's setup is a larger share of the query, so the charge matters more (not measured) | not measured | try first: no stored state |
| Sub-centres in the group's top coordinates (5.2) | reads the basis, changes nothing; needs the first r rows of the projection for candidate groups before they are probed | as sub-clusters | as sub-clusters | only for memory |
| Cluster spread from the group's covariance (5.3 b) | separate small state per group; nothing shared with the codes | as H: with few visible rows per cluster the spread term should fade (3.4 says by λ, not by z(k, n)) | not measured | replay with fixed constants |
| Threshold key (t − μ)/rad and the λ stop (6) | untouched | uses visible counts in Λ(t) | suffers the misplaced centres of 7.1 like every centre-based key | adopt for threshold queries |

Writes, for the three structures that hold state:

| | sub-clusters as labels under clusters | a second level of centres over clusters | flat finer clusters |
|---|---|---|---|
| insert | nearest sub-centre of the chosen cluster (16 dots); label stored with the row; counts and centre sums updated or left to compaction | cluster choice unchanged if inserts still rank every centre; if they descend the tree, a misplaced row costs tightness, not correctness | as today |
| delete | masked, as today | as today | as today |
| compaction | refit sub-clusters when the cluster is rebuilt | rebuild the level | as today |
| state | 0.5–1 B a row + a centre (d B, or r) and a radius per sub-cluster | a centre and a low-rank C_g per node | 16 times the centres, cut tables, cluster records |

Staleness between compactions degrades sub-centres and radii as it degrades the clusters' own; a sub-cluster that
has absorbed many inserts has a centre that is off by the mean of the inserted offsets, which a running sum per
sub-cluster (count and Σx in int16, or just count and Σ of q-independent norms for the radius) corrects.

---

## 8. Recommendation, and the experiments that would decide

**Structure.** Clusters stay the privileged unit. Below them, sub-clusters of about 125 rows as labels, with int8
centres and radii, probed in the approximate modes only. No level above the clusters at 1M to 100M; the groups
enter the order as a cost, not as a level.

**Order and stopping, in one rule (Theorem 2).**
1. Rank all clusters by the invariant cluster key (`spread`, or H over the spread-ranked candidates). It is the
   node key (3.1).
2. Expand, in that order, the clusters whose visible rows exceed the expansion threshold; score their sub-centres
   with the `spread` key (σ(rad) over all sub-centres is a build constant, σ(q) over them is qᵀCq with a low-rank
   C as `PLAN.md` proposes for the centres; the replay used the statistics over every sub-centre) and a constant
   multiplier, not z(k, n); f′ refitted for sub-clusters (0.4 is too small on text). Clusters below the threshold
   are probed whole.
3. Probe sub-clusters (and whole clusters) in one merged order, charging a unit in an unopened group the group's
   setup.
4. Stop on a global λ once the rad-aware probability of README finding 8 exists for sub-clusters; until then on a
   row or unit count. Score the sub-centres of enough parents: 2 to 9 sub-centres per sub-cluster probed (2.4),
   by dataset.
5. Threshold queries: the same with key (t − μ)/rad, the λ stop, and a handover to the exact path when the
   model's Λ(t) exceeds a few per cent of the visible rows.

None of this moves a row, changes a basis or a code, or is read by an exact path; under a visible set every step
either uses the per-cluster visible counts or falls back to whole clusters.

**Experiments that discriminate.**

- **X1. Time, not rows, at two cluster sizes.** Sub-cluster probing through a row mask in `rust/`, on the fits at
  2,000 and 8,000 rows a cluster (the 16M ratio), recall against time. Prediction: about 10% on LAION at 2,000,
  much more at 8,000; and a crossover in selected rows per cluster below which the index path loses to direct
  scoring (4.2). If the gain at 8,000 is under 20%, drop sub-clusters.
- **X2. Group-aware order** on today's clusters, no sub-clusters: π from Theorem 2 with the measured group setup.
  Prediction: 15 to 30% fewer groups at equal recall; a time gain where group setup is a large share (n ≥ 512).
  It is the cheapest thing here to try.
- **X2b. The group's covariance as H's spread** (5.3 b) in the README's replay, with constants fixed by
  leave-one-dataset-out, against `spread`, H r4 and H r16 at equal multiply-adds.
- **X3. The expansion rule under visible sets.** Random 1%, 5%, 25% and a correlated 1% and 5% set: sub-clusters
  with and without the rule of 3.2. Prediction: with the rule, never slower than clusters.
- **X4. A spread for sub-clusters.** The exact σ_s is worth 1.4 to 3.4 times in rows. Test a rank-2 to rank-4
  model per sub-cluster about its own centre (2 to 4·d bytes per 125 rows is too much; in r group coordinates it
  is small), against 5.3. If nothing cheap gets half of it, stop at `spread`.
- **X5. A second level over clusters**, only at K ≥ 5,000 (`--per-cluster 200` as `PLAN.md` 2b proposes, or a
  100M fit): centres scored against rows, under random 1%, with the total-variance node key and m from 2 to 16.
  Prediction from 2.4: 3.6 to 10 times fewer centres; worth building only if ranking is over a quarter of a
  filtered query there.
- **X6. Threshold queries**: (t − μ)/rad against `spread` at answers from 0.01% to 10% in `rust/`, with the
  λ stop; and the answer size at which the exact path wins.
- **X7. Correlated visible sets** as a standing case in the suite (the suite has only random sets). A key from
  the visible rows' own mean per cluster (a running sum over the session's visible rows, computed once per
  session) would test whether the loss in 7.1 is the centre's location.

---

## 9. What I found wrong or overstated in the earlier notes

- **`reports/annideas/REPORT.md`, idea 3**: "It costs about 1 MB in total and nothing per item." The ruling that
  follows stores 2 B a row of positions; int8 sub-centres are a further 4 B a row at d = 512 (the 0.7 MB figure
  assumes a sub-centre can be a 37 B coded row, which that report marks speculative and 5.2 doubts). Position lists are
  also invalidated by an insert in sort order; labels are not.
- **The same report's "1.9–3.7× fewer items"** is right as a count and reproduces here (2.1 to 3.6), but it is not
  a time: clusters and groups opened rise by up to 1.7 and 1.6 times, and its own breakdown has setup at about
  two thirds of the query after the cut. At 1M the estimated gain is about 10% at n = 512.
- **`reports/annideas`: "the radius bonus matters as much as the split"** was said of recall 0.99. At recall 0.9
  here the bonus is worth 1.2 to 1.7 times over the bare centre score at 16 sub-clusters and the split 2.1 to
  3.6. Its key (+0.2 × radius) is not invariant under the query shift; that it does as well as `spread` on these
  three datasets is because 0.2 happens to suit them.
- **`PLAN.md`, the node key** "q·M_g + z·√(qᵀC_g q) plus the largest child spread term": the between-child term
  alone is no better than the centre score on glove (74 against 73 parents), and with the within-child spread
  added it only matches the total-variance key (64 / 10.7 / 37 against 61 / 10.9 / 40), which is simpler and is
  the cluster's own key.
- **`PLAN.md`: "m = 2 already reaches the same rows"** holds for re-ranking clusters by H. For parents standing
  in for 16 children each it is the low end of what is needed (2 on LAION text with the total-variance key, 5 to
  9 on glove, 8 with `spread` on the parents for text); a hierarchy should not assume it.
- **`PLAN.md`: "the groups are already a level"**: as a level they keep 24 to 41% of the groups for the best 1%
  of clusters and are 3 to 6 times worse than clusters as probe units. They are a level for cost accounting only.
- **`PLAN.md` / README, H's `z(k, n_c)`**: the multiplier should not depend on the unit's rows (3.4). At the
  cluster level the dependence is mild (sizes vary little); below it, it is wrong by a factor of 2 to 7. The
  switch to `spread` under sparse visible sets (`H_MIN_ROWS_PER_K`) is this error showing: z(10, 20) ≈ 0 is a
  property of the formula, not of the data.
- **`../theory.md` 3.4** says a linear rule "should use a constant multiplier for row budgets"; measured here, and
  it holds (best ψ 3 to 4 at every unit size).
- **`sublinear/REPORT.md` 5.6 and `FINDINGS.md`** ("a hierarchy reads 129% of the leaf centres") are about a
  certified cut and stand. They do not carry over to the approximate order, where one middle level scores 3.6 to
  10 times fewer centres at about the same rows; but nor does the statistical tree make ranking logarithmic (2.3).
- **`PLAN.md` and the brief: "22,000 clusters of 45,000 rows at 1B".** `rules.py` rounds to a power-of-two
  multiple of 2,000: 64,000 rows a cluster and 15,625 clusters (as `NEXT.md` has it). It changes nothing above
  by more than a factor of 1.4.
- I could not check the cited theorems (Weiss; Bertsimas and Niño-Mora; the hashing exponents) against the
  papers. The branching-bandit statement is supported here only by the exhaustive check on small instances.

## 10. Limits of this evidence

One clustering seed per fit; 100 queries per dataset (LAION's are all text); sub-clusters by 8 Lloyd iterations,
unbalanced (sizes 1 to several hundred); exact means in place of the int8 centres; every spread called "exact" is
the empirical variance of the unit's scores, which no deployable key has. The constants f′, ψ and π marked "best"
were chosen on the same queries they are reported on (grids of 6 to 11 values), so those rows are optimistic by
the usual amount; the comparisons at fixed f′ = 0.4 are not. Times are estimates from constants measured on
another dataset and dimension. Nothing was run under a visible set in `rust/`; the visible sets here are
synthetic and the correlated ones are of one kind (a ball around a row). The tree law was checked at fan-outs 4,
8 and 16 over at most 38,000 leaves, not at K = 22,000 clusters of 64,000 rows.

## References (from memory; not checked)

- G. Weiss, Branching bandit processes, Probability in the Engineering and Informational Sciences 2 (1988).
- D. Bertsimas, J. Niño-Mora, Conservation laws, extended polymatroids and multiarmed bandit problems,
  Mathematics of Operations Research 21 (1996).
- M. L. Weitzman, Optimal search for the best alternative, Econometrica 47 (1979).
- W. Dinkelbach, On nonlinear fractional programming, Management Science 13 (1967).
- W. A. Horn, Single-machine job sequencing with treelike precedence ordering and linear delay penalties, SIAM J.
  Appl. Math. 23 (1972) (the deterministic tree case of the index rule).
- The references of `../theory.md` and `sublinear/REPORT.md` for everything cited through them.
