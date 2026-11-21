# Referee report 1 on the approximate track's claims

21 November 2026. Referee: an independent agent, reporting to the PI. Subject: `../CLAIMS.md`, claims A1.1, A1.2,
A1.4, A1.5, A2.1, A2.2 and A3.1, as written there. `NOTEBOOK.md` was not read.

**State of this file: complete for the seven claims.** "No error found" means I could not break the claim after
the attempts described; it does not mean the claim is correct.

## Verdicts

| Claim | Verdict |
|---|---|
| A1.1 the tree model as a generalised random energy model | Correct after a repair: Model T and the model the cited theorem is about are two different processes, and the claim's A belongs to the second. |
| A1.2 the leaf floor | No error found in the theorem at a fixed level. Remark (i), the k best: the floor is not attained, a complete floor can be stated without the named gap, and it is 1.05 to 3.1 times R(r) at the cluster level. |
| A1.4 the beam exponent | Correct after repairs: W is an upper bound, not the expected count; hypotheses of the limit are missing; the exponent does not order the terms at N = 10⁶. |
| A1.5 the two dichotomies | Items 1, 2, 5: no error found. Item 3: one identification is wrong (the weak regime is the other side of the diagonal). Item 4: the number is right and the inference from it is not. |
| A2.1 the stop under a known tail | Items 1 to 3: no error found. Item 4: true and nearly empty as stated; the proof gives the useful form. |
| A2.2 no shape, no guarantee | Correct after two repairs to the proof; constant right. The closing paragraph claims more than is proved. |
| A3.1 the per-query spread | No error found. The headline is an approximation where an exact statement is available. |

## What was read

- `notes/README.md`, `notes/00-notation.md`, `../CLAIMS.md` (whole).
- `hierarchy/theory.md` sections 0 to 7.1 (Model T and Proposition 1 are section 2.3), `visit-order/theory.md` (whole:
  M0 to M3, Theorems 1 to 3 and 10, Proposition 8).
- `notes/05-approximate.md` 5.0, 5.1, 5.8, 5.9 and 5.10.1.
- `library/codes-and-search.md` Part 2 (the librarian's reading of Addario-Berry and Maillard, Theorem 1.1).
- `../scripts/grem_check.py` and `../out/grem_check.txt`, the one check script the seven claims cite.
- arXiv:1810.05129, through a model's summary of the ar5iv HTML: the definition of the model, Theorem 1.1, (1.1) to
  (1.4), the class of algorithms, section 5 and (5.1). I could not read the PDF itself on this machine.

Scripts of mine, beside this file, each run on core 2, one thread, under 100 MB: `ref1_a12.py`, `ref1_a12c.py`,
`ref1_a14_a15.py`, `ref1_a2.py`, `ref1_a3.py`, with their outputs as `.txt`.

## Claims

### A1.1 Model T is a generalised random energy model

**Verdict: correct after a stated repair.** The first sentence is true once the tree is required to be regular. The
second sentence defines a second model, which is not Model T, and the file's "Local symbols" then call that second
model's profile "Addario-Berry and Maillard's A" as if it were Model T's. The theorem cited is a theorem about the
second model only.

1. **The first sentence needs a regular tree, and says so only afterwards.** Derrida's model has every level-(j−1)
   node with the same number of children. Model T in `hierarchy/theory.md` 2.3 says only "K_j nodes at level j". The
   regularity appears in CLAIMS.md under "What the mapping assumes of the data". It is a hypothesis of the
   statement, not only of the mapping. With it the sentence is the definition of Derrida's model. [no error]
2. **The second sentence is a definition of a different model, and the two are not the same process.** In Model T
   two leaves under one level-(j−1) node and different level-j nodes have covariance v·f_(j−1). In the binary
   refinement with the piecewise-linear A their covariance is v·A(depth of their last common binary ancestor),
   anywhere in [v·f_(j−1), v·f_j). Model T, placed on a binary tree, is the continuous model whose A is the **step
   function** with a jump f_j − f_(j−1) at g_j, not the linear interpolation. What the two share: the law of the
   values along one root-to-leaf path at the depths g_j; every expectation of a count of single nodes above a cut
   (so Proposition 1's B_j; not A1.2's floor, whose proof needs the leaves under a parent independent given the
   internal values); and the concave hull, since the hull of the step function
   and of the interpolation are both the hull of the points (g_j, f_j), so the ground state x_s is the same. What
   they do not share: x_*. Theorem 1.1 assumes A(t) = ∫₀ᵗ a with a Riemann-integrable. The interpolation satisfies
   that and the step function does not. For Model T itself the formula gives ∫√a = 0, which is right in substance:
   under a flat fan-out of M^(g_j − g_(j−1)) children every gain costs a power of M.
   **Repair:** state that Model T is the generalised model (step profile), that the piecewise-linear A defines its
   binary refinement, that the two agree in the three things above, and that Theorem 1.1 is a statement about the
   refinement. A1.6 item 1 says the last of these in words; A1.1, which the rest cites, should carry it.
3. **The refinement needs log₂ F_j to be an integer** at every level, and it needs g strictly increasing. Neither
   is stated.
4. **The table's row for the key and the codes contradicts the local symbols.** "0 = g_0 < g_1 < … < g_D = 1" is
   strict, and A is "the piecewise-linear function through the points (g_j, f_j)". A level of fan-out 1 has
   g_j = g_(j−1), so A is not a function there: it has a vertical step at t = 1 from the first row-level share to 1.
   The row "a row at its key, at the 16 B level, at the 32 B level: nodes of fan-out 1 under the row's cluster" is
   right if fan-out means a node's number of children (key, 16 B and 32 B nodes have one child each). By the
   file's own definition, F_j = K_j/K_(j−1), the key level has fan-out n_c and the three levels after it have
   fan-out 1. The table should say which.
5. **A hypothesis the list of assumptions leaves out: siblings' increments independent of each other.** The list
   has "an increment independent of its parent's value". A node value mapped to "the mean score of a node's rows"
   makes the children's increments sum to zero (weighted by size), so siblings are correlated −1/(F − 1): nothing at
   F = 2,000, a third at the 4-way splits (a4, a16, a64) that A1.7 measures. A1.7 does not test it.
6. **The citation.** I checked Theorem 1.1, (1.1), (1.3), (1.4), the class of algorithms and (5.1) against the
   paper's HTML (ar5iv) through a model's faithful summary of it, not by reading the PDF myself (no PDF reader on
   this machine). They agree with CLAIMS.md and with the librarian's reading. Two things CLAIMS.md drops from the
   theorem: part 1 holds "with probability > 1 − ε for N large", not surely, and part 2 holds "for N large". The
   paper's section 1 does not define the generalised model; it calls the continuous one "an extension" of it and
   says nothing about step profiles, finitely many levels or non-binary trees. So the proof's "(Addario-Berry and
   Maillard, section 1 …)" supports the second sentence only. The first rests on Derrida's definition, which is
   [cited, unchecked] here.

**Statement against proof.** The proof is "these are the definitions". That is all the statement needs once it is
read as two models. The table is declared not to be a claim, which is right.

### A1.2 The leaf floor

**Verdict: no error found in the theorem (fixed level T). Remark (i), the k best: R(r) is not attained, and is
unproved as a floor (numerically it is one, with room); a complete floor is below. Remarks (ii) and (iii) follow.**

**The proof, step by step.** (a) Given 𝒢 the leaf values are independent N(Y_par(i), v(1 − f)): true, since ε_D is
per leaf in Model T. (b) {i ∈ Q} is a function of (𝒢, U_(−i), the randomness): true; the clean argument is to run
the same rule on the input with U_i replaced by a constant, which agrees with the real run until i is first
queried. (c) E[found] = E Σ_(i∈Q) p_i. (d) Σ_Q p_i ≤ λ|Q| + Σ_i (p_i − λ)₊ pathwise. (e) The algebra to
M·P(p > λ_r). (f) Attainment by the threshold rule. All hold.

**Hypotheses used and not stated.**

- 0 < f < 1 (the law of p must be continuous; at f = 0 the quantity a_r does not exist) and r < 1 (at r = 1,
  λ_r = 0 and the division fails; the limit gives M).
- The rule's randomness is independent of the leaf values.
- The recall is a **ratio of expectations**: E[found] ≥ r·E[targets]. The statement says this exactly, so it is not
  hidden, but every later use reads it as "recall r". With T fixed the number of targets in a tree is random and
  over-dispersed (targets come in clumps under a few high parents), and the ratio of expectations weights the trees
  by their target counts. It is not the mean over trees of a tree's recall.

**Stronger than stated.** The proof never uses the regular tree, the upper levels, or anything of 𝒢 beyond each
leaf's own parent. So the floor holds for any tree shape, and for an algorithm told any extra information that is
independent of ε_D.

**Numerical attack** (`ref1_a12.py`, `ref1_a12.txt`). Part 1: K = 200 parents of 500 leaves, f = 0.5, L = 10, 12,000
trees, six leaf-query rules, four of them adaptive (stop at a count found, leave a parent early, a stop that looks
at the last parent's count). None is above the frontier: the two rules built to sit on it are at −0.05 and −0.04
targets (standard error 0.10), the others 1.8 to 3.8 targets below. The quadrature reproduces the author's
R(0.9) = 85,084 at M = 10⁶, L = 10, f = 0.3.

**Remark (i), the k best.** Three things.

1. The object is misnamed. For a leaf-query rule the right conditional probability is
   p′_i = Φ̄((τ_(−i) − Y_par(i))/√(v(1 − f))), with τ_(−i) the k-th best among the **other leaves**, siblings
   included. "Outside the leaf's node", read as outside its parent, is Theorem 10's unit (a cluster read whole) and
   gives a weaker inequality.
2. No concentration is needed for a floor, and one can be stated completely. Steps (b) to (d) go through with
   p′_i in place of p_i, since p′_i is a function of (𝒢, U_(−i)):

       E|Q| ≥ sup over λ > 0 of ( r·k − E Σ_i (p′_i − λ)₊ ) / λ .

   That is a theorem for the k best, for every adaptive rule. A leaf outside the top k has τ_(−i) = τ_k, so the
   right side is the knapsack of a rule that is told τ_k. It is not R(r), and it is not attained, because τ_k is
   not known to the algorithm.
3. "The statement then holds" is false for the attainment half, at the sizes the track uses. I computed three
   numbers for k = 10 and r = 0.9 (`ref1_a12c.py`; the Poisson form of `theory.md` (3.1), no leaf sampled): R(r)
   at the level with M·Φ̄(z_T) = k; the floor of item 2; and the least expected rows of a static rule that knows
   every parent value and not τ_k, which is attainable.

   | K × F | f | R(0.9), fixed level | floor for every adaptive rule | static optimum | floor / R |
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

   So the true cost of the k best lies between the fourth and fifth columns, 1 to 4% apart, and it is 1.05 to 3.1
   times R(r) with a few hundred parents. The cause is that τ_k does not concentrate when a level of a few hundred
   nodes explains a large share: the plug-in τ_k varies over trees with a standard deviation of 0.15 to 0.24 √v at
   K = 339 to 500 and f = 0.45 to 0.6, and 0.03 to 0.12 at K = 5,400 (`ref1_a12.txt`). "The concentration of τ_k
   is not carried through" is therefore the whole of the difference at the cluster level, not a technicality. At
   r = 0.99 the static optimum is 1.0 to 2.0 times R(r) (`ref1_a12.txt`); the floor of item 2 is too noisy there
   at my sample sizes to quote (it is evaluated within 1% of the total mass), and the cell 5,400 × 125 at f = 0.3
   is not quoted for the same reason.
4. In every cell R(r) was below the floor of item 2, so R(r) did not fail as a floor for the k best. That it is
   one in general is not proved, here or in CLAIMS.md.

**Consequence for the other claims.** A1.3's closed form and local exponents, and A1.7 items 5 and 6 ("the law"),
are R(r) at a fixed level. For k = 10 with clusters at f about 0.45 the model's own figure is 1.4 to 1.6 times
higher. That moves A1.7's reading "the data are easier than the model" further in the same direction.

**Remarks (ii) and (iii).** (ii) holds in the model with the criterion above. (iii) holds: with one leaf per
parent the proof is unchanged.

### A1.4 The work of a beam, and where the levels should sit

**Verdict: correct after stated repairs, as a derivation in its model. The exponent η and the rule for the level
positions are right. Three statements around them need changing: the equality for W, the missing hypotheses of the
limit, and the use of η at N = 10⁶.**

1. **W is an upper bound on the nodes scored, not their expected number.** B_j of Proposition 1 counts the level-j
   nodes above their own cut, whatever their ancestors did. A beam scores the children of a node only if every
   ancestor was kept, so its expected count at level j is F_j·E#{level-(j−1) nodes above their cut with every
   ancestor above its cut} ≤ B_(j−1)·F_j. Measured on trees of 28³ leaves (`ref1_a14_a15.py` part A, 400 trees):
   the cascade scores 4,333, 11,785, 8,656 and 1,246 nodes where W is 4,508, 12,187, 9,006 and 1,333, 3 to 7%
   fewer. The exponent is unaffected: given a node at its cut, its ancestor's conditional mean lies above the
   ancestor's cut by z_ε·√v times a positive constant (because f_(j−2)(1 − f_(j−1)) < f_(j−1)(1 − f_(j−2))), so the
   ancestors are kept with probability bounded below by 2^(−D). **Repair:** "at most W", with that remark for the
   exponent. The same simulation reaches 0.988 to 0.991 of the targets against the union bound's 0.98.
2. **Hypotheses of W = M^(η + o(1)) that are not stated.** ln L = o(ln M) (with L = M^β the exponent of level j is
   g_j − (1 − β)·f_(j−1)); ε and D fixed; a regular tree (B_(j−1)·F_j needs every kept node to have F_j children);
   f_1 > 0. All are implicit in "with the g_j and f_j fixed", and none is hard.
3. **The level positions.** The one-line argument ("lowering any g_j below that raises the term of level j + 1")
   shows the equalised positions cannot be improved one coordinate at a time. A complete proof is short and I
   checked it: let G_D(η) be the last iterate of g_1 = η, g_j = η + A(g_(j−1)). For any positions with largest term
   η, induction with A non-decreasing gives g_j ≤ the j-th iterate, so 1 = g_D ≤ G_D(η); G_D is continuous and
   strictly increasing with G_D(0) = 0, so η ≥ its root. Hypotheses used: A non-decreasing and continuous, and that
   a level placed at entropy position g explains A(g) whatever the other levels are. The last holds by definition
   in the binary refinement of A1.1 (and Proposition 1 holds there too, since it uses only the law of (Y_j, U)); for
   an index it is an assumption about how nested and non-nested partitions relate, and it is not stated.
4. **The limit in D.** For η ≤ s = sup_t (t − A(t)) the iterates stay below the point where t − A(t) ≥ η and never
   reach 1; for η > s each step gains at least η − s. So the minimum decreases to s. Correct, with the order of
   limits M → ∞ first, then D → ∞. For a binary tree D = log₂ M and the two limits are taken together; the
   corrections (z at ε/D, a sum of D terms) stay M^(o(1)), but that is not in the derivation, and A1.5 item 5 uses
   the limit in that joint sense.
5. **The o(1) is not small at 10⁶ and differs by level.** The first term, K_1, has none. Term j ≥ 2 carries
   exp(z_ε·z_T·√(f_(j−1)(1 − f_(j−1)))), which at N = 10⁶ and f near ½ is two orders of magnitude. With the profiles
   of A1.7, L = 10 and a total miss of 0.1 split over the pruning levels (part B), ln(term)/ln N is 0.72 to 0.84
   for the row term against exponents 0.41 to 0.64, and equal to the exponent for the centres. The row term is the
   largest by count in all twelve designs of A1.9 on the three fits, by 20 to 100 times; by exponent the largest is
   the centres' term in three of them (yahoo with four levels, LAION with a16, LAION with four levels). So the model
   itself, evaluated and not read through its exponent, does not say that "leaves of about 125 rows bring the row
   term to the centres' term on yahoo and LAION" at 1M (A1.9). The statement of A1.4 is asymptotic and true; the
   bold terms of A1.9 are not what W gives at that size.
6. **The consequences.** "A level of fan-out 1 adds no term" is true of its own term and of η when every such level
   lies below the last branching level, as here; a fan-out-1 level does lower the next level's term
   (g_(j+1) − f_j in place of g_(j+1) − f_(j−1)), so "only branching levels move η" is true of this tree, not in
   general. "A new branching level helps if and only if it splits the term that attains the maximum" needs the
   maximum to be attained once and A(g) > f_(j−1).
7. **"Why [model]".** The caveat is right and I checked its condition: the first moment is the typical count when
   f_i < g_i at every level above (the straight path in variance has M^(g_i − f_i) nodes at level i). At f_i = g_i,
   which is where the measured profiles sit at the cluster level, the expected number of level-i nodes at the
   straight path's height is L^(f_i) times a negative power of z_T, so it tends to zero slowly and the first moment
   is carried by rare trees there too. That boundary case is the data's case and deserves a sentence.
8. **The "check on synthetic trees" is not a check.** `grem_check.py` part C solves the recursion and prints its
   root. No tree is built and no W is counted. My part A is such a check for W at D = 3; nothing here checks the
   exponent, which no simulation at these sizes could (item 5).

### A1.5 Which dichotomy Model T's is

**Verdict by item: 1 and 2 no error found; 3 correct after a repair to one identification; 4 the computation is
right and the inference is wrong; 5 the identity holds at the ground-state level, no error found.**

1. A concave A with A(0) = 0, A(1) = 1 lies on or above the chord. No error found. A here is the interpolation;
   the remark of A1.1 item 2 applies.
2. The counter-example is arithmetically right: A(1/3) = 0.60 ≥ 1/3, A(2/3) = 0.67 ≥ 0.6667, slopes 1.8, 0.21,
   0.99. Its level sum is 0.932 and its hull sum 0.964, so x_* < x_s < 1 for it. The margin at the second point is
   0.003, so I would use (0.60, 0.10, 0.30) and not depend on the rounding of 2/3.
3. **One identification is wrong.** "Model T's splits profiles by the diagonal (the weak- and strong-correlation
   regimes of Bovier and Kurkova)". Bovier and Kurkova's weak regime is A(t) ≤ t for every t (as the paper states
   after (1.2): x_s = √(2 log 2) if and only if A(t) ≤ t). Model T's good regime is A(t) ≥ t for every t. These are
   two different splits by the same line: a profile that crosses the diagonal is on the bad side of both, and only
   A(t) = t is on the good side of both. **Repair:** say that Model T's "subpolynomial" side is the profiles on or
   above the diagonal, which is the opposite side from the weak regime, and that for every such profile except the
   diagonal itself the level z_REM is empty. The resolution that follows (no conflict, because above the diagonal
   x_s < 1 and the level z_REM holds no leaf with high probability) is correct: ∫√â ≤ 1 with equality only for
   â = 1. It needs ln L = o(ln M), unstated. "The published one is the sharper" compares answers to two
   questions: the beam formula counts the nodes kept to reach every leaf above a level (at a level x < 1 its last
   term is the M^(1 − x²) leaves there are), and Theorem 1.1 is about reaching one. The item would be clearer if
   it said so; as it stands a reader takes both for the cost of the k best.
4. **The agreement of 0.967 with 0.960 is an accident of L = 64.** The hull sum is the limit of the maximum over
   √(2 ln M). At 21,952 leaves the realised level over z_REM(L) moves with L (part C, 200 trees a row):

   | shares | hull sum | L = 1 | L = 8 | L = 64 | L = 512 |
   |---|---|---|---|---|---|
   | 0.6, 0.25, 0.15 | 0.959 | 0.901 | 0.933 | 0.966 | 0.999 |
   | 0.8, 0.15, 0.05 | 0.869 | 0.803 | 0.849 | 0.911 | 0.978 |
   | 0.9, 0.07, 0.03 | 0.800 | 0.724 | 0.781 | 0.862 | 0.976 |
   | 0.60, 0.07, 0.33 | 0.964 | 0.929 | 0.930 | 0.953 | 0.980 |
   | 0.15, 0.25, 0.6 | 1.000 | 1.016 | 0.997 | 0.995 | 0.995 |

   I reproduce the claim's 0.967 (0.966 here). The maximum sits 6 to 8 points below the hull sum, the top 512 well
   above it, and at L = 64 the other strong profiles miss by 4 to 6 points. So the number is right and "the
   realised top level follows the hull" is not shown by it. The asymptotic statement is the cited theorem and
   needs no simulation; at these sizes the hull sum does not predict the realised level to better than about 0.05,
   which is the size of the effect A1.7 item 8 reads from it (0.975 and 0.954).
5. With b = a·√(2 log 2/â), b²/(2a) = (log 2)·a/â, so −E(b, t) = (log 2)·(t − ∫₀ᵗ a/â), and in units of ln M the
   count exponent is t − ∫₀ᵗ a/â. With â = 1 it is t − A(t). No error found. Two limits on what it says: it is the
   pool of the one path that ends at x_s, so it equals γ_*(x)/log 2 of (5.1) at x = x_s only (for x < x_s cheaper
   paths exist); and the left side is the D → ∞ limit of an expected count, item 4 of A1.4 above. The tags
   (computed; their conjecture) are right. I read (1.3) and (5.1) through a summary of the paper's HTML.

### A2.1 What the stop guarantees when the tail is known

**Verdict: items 1 to 3, no error found. Item 4 is correct and nearly empty as stated; the proof gives a better
statement, below.**

**The proof, step by step.** (1) τ_k ≥ t̂, so a missed neighbour is an unread visible row at or above t̂: holds.
(2) On the event that the unread set is S, that event and t̂ are functions of the statistics and the scores outside
S, and the scores inside S are independent of those given the statistics: holds. (3) The written line proves
E[Miss | σ = m] < (1 − r)·k. The statement conditions on everything read up to the stop, which is finer. One more
line closes it: for an event B of the stopped σ-field, B ∩ {σ = m} is ℱ_m-measurable, so the same inequality holds
on it. (4) follows from (2).

**Hypotheses used and not stated.**

- The model is M3 (one shape G for every cluster), not M1 as the statement names it; M1 has a law F_c per cluster.
- G is known to the rule exactly, and is the same for every query.
- μ_c, σ_c and v_c in ζ_c are the exact location, scale and count of the **visible** rows' scores. The statement
  implies it through "P(s ≥ t) = G(ζ_c(t))", but the stop as built has σ_c from 16 directions and, under a visible
  set, the whole cluster's μ_c and σ_c; the shift and scale it estimates (δ_q, ρ_q) are exactly what this
  hypothesis assumes away.
- A read scores every visible row of the cluster exactly. The reader of chapter 5.9, which drops rows at a level by
  a statistical width, is outside the statement: a row it dropped is a miss that X(t̂) does not count.
- The order's own randomness is independent of the unread scores; at least k visible rows; no ties at τ_k.

**Weaker hypotheses suffice.** Independence of the scores inside a cluster is never used: (2) is linear in the
rows. Only the marginal law of a row and independence between the read and the unread clusters, given the
statistics, are needed. Any stopping time at which M < (1 − r)·k will do, not only the first.

**What "for every query" means.** The expectation is over the model's scores given the statistics. It is a
statement about the queries that share a set of statistics, and it bounds a mean. It does not bound the share of
queries below r. In the simulation below, with the true tail known, the mean recall is 0.96 (normal) and 0.91
(Student-t, 5 degrees) at r = 0.9, and 7% and 23% of the queries have recall under 0.9.

**Numerical check in the model** (`ref1_a2.py` part A; 300 clusters of 100 to 400 rows, k = 10, 1,500 queries a
row, a static order μ + 3σ and an adaptive order re-keyed at t̂):

| scores | the stop's tail | order | E X(t̂) at the stop | E M at the stop | E Miss | mean recall |
|---|---|---|---|---|---|---|
| normal | normal | static | 0.439 ± 0.018 | 0.412 | 0.412 | 0.959 |
| normal | normal | adaptive | 0.423 ± 0.018 | 0.417 | 0.391 | 0.961 |
| t(5) | t(5) | static | 0.943 ± 0.025 | 0.978 | 0.909 | 0.909 |
| t(5) | t(5) | adaptive | 0.995 ± 0.026 | 0.981 | 0.937 | 0.906 |
| t(5) | normal | static | 7.95 ± 0.12 | 7.90 (true tail), 0.36 (used) | 4.87 | 0.513 |

The identity of item 2 holds at the stop to within 1.5 standard errors in each row, and to within 2 in each half
of the queries split at the median of M, under a static and an adaptive order. With a heavy tail the bound is nearly attained (0.906 against 0.9), so item 3 cannot be improved
by much.

**Item 4.** "G ≤ κ·Ĝ at every ζ_c of an unread cluster" is a bound on the worst ratio, and the worst ratio is at
the farthest unread cluster, which carries none of M. In the last row of the table that κ has median 10¹⁹⁷, and
the statement says nothing. The proof needs only M ≤ κ·M̂, so it gives, with no further work,

    E[recall | everything read up to the stop] ≥ 1 − M_σ/k ,   and M_σ = κ_σ·M̂_σ with κ_σ = M_σ/M̂_σ,

a ratio of the two sums, weighted by where the mass is. In the same run that κ_σ has median 27 and 90th
percentile 300. **Repair:** state item 4 with κ = M/M̂ at the stop (or the supremum of G/Ĝ over the ζ_c that carry
all but a stated share of M̂). The remark that κ at the stop exceeds κ on average is then a statement about this
ratio, and is the right warning.

### A2.2 No shape, no guarantee

**Verdict: correct after stated repairs to the proof; the constant is right. The closing paragraph claims more
than the theorem gives.**

1. **G_1 is chosen after looking at the sample.** "ζ* … large enough that μ_c + ζ*·σ_c exceeds every score held"
   makes the shape depend on the draws from it. **Repair:** F_0 has no mass above ζ_0, so when no pooled draw is on
   the atom every held score is at most max over read c′ of (μ_c′ + ζ_0·σ_c′), a function of the statistics.
   Choose ζ* from that bound.
2. **"X is independent of the event that the rule stops" is false as written.** X counts rows above t̂, and t̂ is a
   function of the sample; when the sample holds atom draws t̂ moves. **Repair:** let E be the event that none of
   the n draws is on the atom, and X′ the number of unread rows on the atom, which is Binomial(W, ε) and
   independent of the sample and of whatever else the rule sees. On E every atom row outranks every row held and
   no other unread row passes t̂, so Miss = min(k, X′). P_(G_1)(stop and E) = (1 − ε)ⁿ·s ≥ ¾·s. Hence
   E_(G_1)[Miss; stop] ≥ ¾·s·E min(k, X′) ≥ ¾·s·½·min(k, W/(4n)), which is the claim with ¾·s for s − ¼ (and
   ¾·s ≥ s − ¼). The corollary's 3/32 is unchanged.
3. **The setting is a one-shot decision with everything but the sample fixed.** The statement treats n, the
   unread set, W and the ζ_c as given. In the stop of A2.1 all four are functions of the sample: ζ_c is ζ_c(t̂),
   and n and W are what they are when the rule stops. For the theorem as stated, add: the unread set is fixed,
   and ζ_c(t̂) ≥ ζ_0 for almost every sample from F_0 (ζ_1 is then not needed). For a sequential rule the same
   coupling gives the result with E_(F_0)[n at the stop] in place of n (the two runs agree until the first atom
   draw, which comes before the stop with probability at most ε times that mean). That is not in the proof, and
   the sentence "this is the pooled-sample form" applies the one-shot result to a sequential stop.
4. **The inequality E min(k, X) ≥ ½·min(k, m).** The three cases are right: E(X − k)₊ ≤ E(X − m)₊ = ½·E|X − m| ≤
   ½√m for m ≤ k; 1 − e^(−m) ≥ m/2 for m ≤ 1; monotone in the success probabilities for m > k. Checked exactly for
   k from 1 to 100, W from k to 10⁵ and m from 0.001 to 3k (`ref1_a2.py` part B): the least value of the ratio is
   1.26, at k = 1, m = 1. The total-variation bound 1 − (1 − 1/(4n))ⁿ ≤ ¼ is right.
5. **"Every shape" is a larger class than the model's.** In M3 with μ_c and σ_c the mean and standard deviation
   of a cluster's scores, a shape has mean 0 and variance 1. In that class there **is** a rule valid for every
   tail: Cantelli gives G(ζ) ≤ 1/(1 + ζ²), so stopping when Σ over unread c of v_c/(1 + ζ_c²) < (1 − r)·k
   guarantees item 3 of A2.1 for every standardised shape (`theory.md` Proposition 7, item 2; useless in
   practice, valid). The theorem and this rule do not conflict, and the reason is the remark's unworked range:
   G_1 can be standardised only if ε·ζ*² < 1 − ε, that is ζ* < √(4n − 1) (the remark's ε·ζ*² < 1 is this to first
   order), while item 1 needs ζ* above ζ_max = max over unread c of (best score held − μ_c)/σ_c. So in the
   standardised class this construction proves the theorem when ζ_max < √(4n − 1), and is not available
   otherwise. At the sizes of A2.3 (n from 150 to 9,100, ζ about 4) the condition holds with room, so the
   practical reading stands.
6. **A second gap in the remark, not named there.** Standardising G_1 replaces F_0 by a shifted and rescaled
   F_0′ in the mixture. The rule's stopping probability s is given under F_0, not F_0′. Either define s under
   F_0′, or add the total variation between n-samples of F_0 and F_0′ (for a smooth F_0 about ζ*/(8√n) from the
   shift ζ*/(4n) and about ζ*²/(11√n) from the change of scale: small at n in the thousands, not zero).
7. **"Nothing between" is not proved.** The construction is one atom far above the sample. Any class that forbids
   it escapes the theorem without being "a named tail family": a hazard rate that may not fall faster than a
   stated rate, a log-concave tail, a known upper end of the support. The paragraph should say that the theorem
   rules out the class of all (standardised) shapes, and that what lies between that and a parametric family is
   open.

### A3.1 The per-query spread

**Verdict: no error found. Items 1 to 4 are exact identities and hold. The headline ("the standard deviation of
Δ·e is approximately ρ·h") is an approximation whose exact form is available and should be the statement.**

1. **What is exact.** Given κ, E[(Δ·e)² | κ] = κ²·Σ Δ_i²ν_i and E[h² | κ] = w²·κ²·‖a‖₁, so

       ρ_ℓ(q, c)² = E[(Δ·e)² | κ] / E[h² | κ]     for a fixed Δ and every κ.

   This is the same kind of statement as chapter 5's (5.6), without (5.6)'s gap: there the mean of h² was taken as
   a product of two means, and here w is fixed. Averaged over offsets with E Δ_i² = π_i it returns (5.6):
   E[ρ_ℓ(q, c)²·w²] = ρ_(g,ℓ)²·E w².
2. **What is approximate.** The statement conditions on the stored u, the proof conditions on κ and then replaces
   u² by its mean given κ. Given u, the ξ are no longer free: a large u at a fixed κ means large ξ_i² on the
   coordinates with large a_i. For Gaussian ξ and equal a_i the formula is exact given u, for any law of κ (the
   direction of ξ is then uniform given its length). Otherwise it is not. Sized in the model (`ref1_a3.py`,
   n = 384, 100,000 rows a cell; the ratio of Σ(Δ·e)² to Σ(ρwu)² within quintiles of u):

   | a_i | κ | offset | lowest to highest quintile of u |
   |---|---|---|---|
   | equal, or water-filled (70% coded) | fixed or log-normal | any of four | 0.97 to 1.02 in every cell |
   | log-normal, sd 1 | fixed | one draw from the profile | 0.96 to 1.04 (normal, uniform ξ); 0.91 to 1.06 (skewed ξ) |
   | log-normal, sd 1 | fixed | energy on the high-a coordinates | 0.79, 0.87, 0.97, 1.06, 1.22 |
   | log-normal, sd 1 | fixed | energy on the low-a coordinates | 1.08, 1.05, 1.01, 0.98, 0.93 |
   | log-normal, sd 1 | log-normal 0.4 | energy on the high-a coordinates | 0.95 to 1.02 |

   Over all rows the ratio is 1.00 ± 0.01 in every cell, as item 1 says it must be. So under an allocation that
   makes a_i nearly constant the approximation is good to about 2% in variance; with strongly unequal a_i and rows
   of one scale it is off by up to 20% in variance (10% in the spread), in the direction that under-states the
   spread of the rows with large u when the query's energy sits on the high-a coordinates.
3. **Hypotheses used and not stated.** ξ uncorrelated **given κ** (κ independent of ξ); ξ and κ independent of
   the query; "standard deviation" needs E[Δ·e | u] = 0, which uncorrelated ξ of mean 0 do not give (sign-symmetric
   ξ do; in the simulation a skewed ξ gave a mean of Δ·e/(ρh) within 0.01 of zero); Δ and e in dual coordinates,
   so that Δ·e = Σ Δ_i·e_i; ν_i the same for every row of the group up to the row's κ. The use of ψ·ρ·h as a
   normal quantile also needs Δ·e near normal: the 99.9% point of |Δ·e|/(ρh) was 3.1 to 3.35 (normal 3.29) except
   where the offset's energy sits on few coordinates, where it was 2.8 (uniform ξ) and 3.9 to 4.0 (skewed ξ, 8
   coordinates).
4. **Items 1 to 4 of the statement.** (1) ω_i = a_i on the profile: holds. (2) A weighted mean: holds, given
   w > 0. (3) ρ_ℓ(q, c)²/ρ_(g,ℓ)² = 1 + Cov_a(a, r)/(E_a[a]·E_a[r]) with r_i = Δ_i²/π_i and the covariance taken
   under the weights a_i/‖a‖₁: holds, and the weighting should be said. (4) First sentence: holds. Second
   sentence: I checked it for the two allocations it could mean. Minimising ‖a‖₁ (the certified width) at a bit
   budget Σ ½·ln(s_i/ν_i) gives √(ν_iπ_i) constant on the coded coordinates and √(s_iπ_i) below that constant on
   the uncoded ones; minimising Σ ν_iπ_i (the mean squared error) gives ν_iπ_i constant, the same conclusion. Plain
   reverse water-filling (ν_i constant) does not: there a_i = √(θπ_i) varies with π_i on the coded coordinates.
   So the sentence is true of a water-filling weighted by π_i and false of the unweighted one; "reverse
   water-filling" should carry the weight. Whether `det_alloc_tree` is such an allocation is A3.3's caveat.

**Statement against proof.** The statement is weaker than the proof where it says "approximately" for something
the proof has exactly (item 1), and stronger where it conditions on u (item 2).

## What I did not check

- **The other claims.** A1.3, A1.6 to A1.9, A2.3 to A2.8 and A3.2 to A3.4 were not refereed. Where a finding above
  touches one (A1.3 and A1.7 through remark (i) of A1.2; A1.9 through item 5 of A1.4; A1.7 item 8 through item 4 of
  A1.5), I say what it implies and no more. No measured table, replay script or number from real data was checked.
- **The cited paper's proof.** I checked the statement of Theorem 1.1 and the formulas (1.1) to (1.4) and (5.1),
  through a summary of the HTML, not the PDF and not verbatim. I did not read sections 2 and 3. Bovier and
  Kurkova's theorem is taken as the paper quotes it, and Derrida's definition of the generalised model from
  memory.
- **The k best for A1.2.** My floor for every adaptive rule uses the Poisson form of the top-k law (Le Cam's error
  is not bounded here) and leaves the k leaves of the top k at τ_k in place of the (k+1)-th best. Both are small,
  and neither is proved small. k = 10 only.
- **The exponent of A1.4.** No simulation at reachable sizes can test an exponent whose correction is 0.15 to 0.3
  at N = 10⁶. I checked W against a cascade at D = 3 and 21,952 leaves, and the algebra.
- **A2.2 for a sequential rule.** I state how the coupling extends and did not write it out.
- **A3.1 against data or the code.** The check is in the claim's own model. Whether the build's coding error is
  uncorrelated across the group's coordinates, and whether `det_alloc_tree` makes a_i constant, were not looked at.
- **Sizes.** Every simulation is small (at most 10⁶ leaves by quadrature, 21,952 sampled; 1,500 to 12,000
  replicates) and each ran once with a fixed seed.
