# Floors track: working notebook

Researcher: the floors theorist. Reports to the PI. Everything here is unrefereed. Claims and proofs are in
`CLAIMS.md`; this file is the log, including what failed. Notation is that of `notes/00-notation.md`; local symbols
are declared in `CLAIMS.md` section 0. All scripts are in this directory and were run as
`nice -n 10 taskset -c 6 env OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 <script>`.

To resume: read `CLAIMS.md` sections 0 to 4 (the floors F1 and F2) and 14 (what failed, what is open), then the
last dated entry here.

---

## 2026-11-21, entry 1: reading, and the first attempt to break Conjecture A

Read `notes/README.md`, `00-notation.md`, `proofcheck/CONSOLIDATED_PROOFS.md`, `proofcheck/REFEREE.md`,
`groups/REPORT.md` 2.4, `quant/REPORT.md`, `agent1/theory.py` and `theory.log`, `quant/floor2.py` and its logs,
`entropy/REPORT.md`, `layered.md` 4.1 to 4.3. I did not read `notes/program/stage0-floor-check/`.

Setting used throughout (consolidated proofs, section 0). In eigen-coordinates Δ·r = Σᵢ √γᵢ gᵢ zᵢ, with g and z
independent standard Gaussian vectors, z a function of r. A cell is a set A of z-values, and soundness gives
E_g h(c, ·) ≥ E_g sup over z in A of Σᵢ √γᵢ gᵢ zᵢ.

Attempts to break Conjecture A before proving it:

- n = 1, cell [−a, a]: the left side is a√(2/π) and the right side s(ln 1/P). At a = 1: 0.798 against 0.46. For
  a → 0 the right side is P/√e ≈ a√(2/π)/√e. No violation.
- Centred balls in ℝⁿ, γ = 1: at P = ½ the left side is about n − ¼ and the right side n − √(n ln 2). No violation.
- Uniform quantiser of step w in one dimension, high rate: E h = w/√(2π) = √e·e^(−H); Conjecture A gives
  e^(−H)/√e. A factor e below the truth in one dimension, and √e·κ_n/n below Theorem 1 in n isotropic dimensions.

So Conjecture A looked true and loose. Where the looseness comes from is the useful part: see entry 2.

## 2026-11-21, entry 2: the route of the brief, simplified; and what it throws away

Steps 1 and 2 of the brief are fine as written. Two remarks.

- Atoms are not an obstacle. The map goes from the Gaussian g, which has no atoms, to the cell law ν, by
  conditional quantile functions; a quantile transform of a uniform variable has the target law whatever the
  target. Continuity of the conditional distribution functions would be needed only for the inverse direction,
  which the proof never uses.
- The coordinate order is free. The bound holds for every order; nothing claims the per-coordinate correlations
  are the same in every order.

Step 3 has a three-line proof that needs no convex programme. Write the monotone map as T, so T(g) has law μ.
Then h(μ) = h(g) + E ln T′(g) ≤ ½ ln(2πe) + ln E T′(g), and E T′(g) ≤ E[g·T(g)] = C(μ) (Stein; the jump part of
T only helps). So

    h(μ) ≤ ln(√(2πe)·C(μ)),       equivalently   KL(μ ‖ N(0,1)) ≥ ½ m₂(μ) − ½ − ln C(μ).          (L1)

Conjecture A's step 3 follows from (L1) by one more inequality, m₂ ≥ C² (Cauchy–Schwarz). The proof of (L1) for a
general law with a density (T may jump, T′ exists only almost everywhere) is in `CLAIMS.md`, L1.

Step 4's worry (random conditional KLs, budget only on average) does not arise. Apply (L1) to each conditional
law, integrate, and use Jensen for the logarithm: E ln Cᵢ(g_<i) ≤ ln E Cᵢ(g_<i). The convexity of s(·) is not
needed. (It is true, s″ > 0 on (0, 1), so the brief's version of step 4 is also valid.)

**What Conjecture A throws away.** The step m₂ ≥ C² is the whole loss. For a small cell far from the origin m₂ is
about the squared distance of the cell from the origin, and C² is about the squared width of the cell. But the
cells' second moments are not free: the cell laws average back to N(0, I), so Σ_c P_c E[zᵢ² | c] = 1 for every i.
Keeping m₂ and averaging (L1) over the cells before discarding anything gives

    Σᵢ ln(1/C̄ᵢ) ≤ I(z; c) ≤ H(c),     C̄ᵢ = Σ_c P_c Cᵢ(c) ∈ (0, 1],     E h ≥ Σᵢ √γᵢ C̄ᵢ.

Minimising Σ √γᵢ sᵢ under Σ ln(1/sᵢ) ≤ H, sᵢ ≤ 1, is reverse water-filling: the minimum is Σᵢ √min(γᵢ, θ) with
Σ ½ ln⁺(γᵢ/θ) = H. **That is the full-floor conjecture of section 6 of the consolidated proofs**, for every n,
with Σ_Q and Σ_R general. Written out as Theorem F1 in `CLAIMS.md`. I did not expect this; the checks below were
written to break it.

## 2026-11-21, entry 3: the head constant

The brief's idea: choose the head point first (support function over a set G_τ of head points with good fibres),
then couple on the tail fibre. What stops it: the head point chosen by a support function is an extreme point of
G_τ, so its law is not the head marginal of the cell law, and nothing averages. Every fibre of G_τ has to be good,
which Markov's inequality buys at a price 1/ε in tail rate, and the entropy of the head law restricted to G_τ
costs a factor 1/(1 − ε) in head rate. Details and the resulting lossy statement in `CLAIMS.md`, F2-alt.

**Reversing the order removes the obstacle.** Couple the tail first: z_T = T(g_T) has exactly the tail marginal of
the cell law. The head Gaussian g_S is independent of g_T, hence of z_T. Take the supremum over the whole head
fibre above z_T: its mean over g_S is a Gaussian mean width, Urysohn bounds it by the fibre's volume, and the
volume by the conditional entropy h(z_S | z_T, c). No selection, no Markov. The chain rule splits the rate
exactly, I(z; c) = I(z_T; c) + I(z_S; c | z_T), and the result is Theorem F2:

    E h ≥ min over H_S + H_T ≤ H of { κ_k (Π_{i∈S} γᵢ)^(1/2k) e^(−H_S/k) + Σ_{i∉S} √min(γᵢ, θ_T(H_T)) }.

With S the water-filling head the minimum is at H_T = 0 and the bound is κ_k √θ + Σ_{i>k} √γᵢ: the section 6
conjecture with Theorem 1's constant on the head. Theorem 1 is the case S = everything, Theorem F1 the case
S = ∅.

## 2026-11-21, entry 4: the one-dimensional lemma, numerically

`check_lemma1d.py`, log `check_lemma1d.log`. Piecewise-constant densities, for which h, C, m₂ are closed forms.

- First version of the script was wrong twice: a sign error in ∫Φ⁻¹(u)du (gave C = −1 for a Gaussian), and
  catastrophic cancellation in far-tail cells. Fixed by integrating in x = Φ⁻¹(u) with Gauss–Legendre nodes,
  splitting at the median and reflecting the upper half. Validated against the uniform law (C = w/(2√π)),
  Gaussians (C = s to 7 digits), and a 4-million-point quadrature on a random law (7 digits).
- Result: 40,003 laws (discretised Gaussians, cells of a quantiser, random densities with gaps and spikes,
  mixtures, Laplace, Student t₃, and an L-BFGS adversary on 8, 20 and 40 cells). Smallest gap
  ln(√(2πe)C) − h = 3.8·10⁻⁷, for N(0,1) truncated to [−6, 6]. No violation. The adversary converges to the
  discretised Gaussian from above (1.6·10⁻³ on 40 cells against 3.7·10⁻³ for the plain discretisation).

## 2026-11-21, entry 5: message from the PI (prior art); the two papers read

The PI relayed the librarian's report (`notes/program/library/theory.md`). I read its verdicts 1 and 2 and the
corrections, then read both papers from their arXiv TeX sources (not through a summariser).

**Ingber, Courtade, Weissman, arXiv:1307.6609 (IEEE Trans. Inf. Theory 61(5), 2015).** [cited, source read]
- Setting: X, Y independent, i.i.d. coordinates, length n; signature T(x) of nR bits; g answers no/maybe and must
  answer maybe whenever (1/n)‖x − y‖² ≤ D. A rate is achievable if Pr[maybe] → 0 as n → ∞.
- Theorem 1 (`thm:RIDdiffVar`): for P_X = N(μ, σ_X²), P_Y = N(μ, σ_Y²), R_ID = log₂(2σ_Xσ_Y/(σ_X² + σ_Y² − D)) on
  (σ_X − σ_Y)² ≤ D < σ_X² + σ_Y², 0 below, ∞ above. So unequal variances are covered, but both laws are
  isotropic.
- The converse is a real proof. It restricts to the typical shell, projects each cell on the sphere, applies
  Lévy's isoperimetric inequality to the expansion of the projected cell, and averages over cells with a
  Markov-type lemma (their Lemma 4, `lem:Markov`). It concludes R ≥ log₂(2σ_Xσ_Y/(σ_X² + σ_Y² − D″)) + O(log n/n)
  from Pr[maybe] ≤ ¼, with D″ → D as the shell thickness η → 0.
- Theorem 4 (`thm:GaussianExtreme`): for given variances the Gaussian pair needs the largest rate. The proof is by
  achievability for general sources (Theorem 3); there is no converse for non-Gaussian sources, and the authors
  say why: "precise isoperimetric inequalities are unknown and therefore establishing a general converse appears to
  be extremely difficult".
- What it is not: a bound at a given n, a bound on a mean, or a statement for anisotropic covariances.

**Wu, Wang, Flierl, arXiv:2001.07941.** [cited, source read]
- Theorem 1 (`theorem:2` in the source): R = (1/M) Σ max(0, log(ξ_m/τ)), D = (1/M) Σ max(0, 2(ξ_m − τ)), query and
  data with the same covariance. The librarian's translation is right: with γ = ξ², θ = τ² this is the curve
  Σ √min(γᵢ, θ) against Σ ½ log₂⁺(γᵢ/θ).
- What the paper proves is less than the theorem says. The derivation fixes a "component-based model" (KLT, then
  an Ingber–Courtade–Weissman system per component, answers combined by AND) and optimises the allocation of
  thresholds over components by Lagrange multipliers. That is an optimisation inside one family of schemes.
- There is no converse. The step that would be one reads: under the optimal allocation "the component-based model
  preserves the same characteristics of the original system", so the two rates are "identical". No argument
  bounds an arbitrary signature.
- Lemma 1 is not sound as written: its chain uses "∝" between probabilities, and it equates the event
  {Σ_m d_m ≤ Σ_m D_m} with the event {d_m ≤ D_m for every m}, which is a strict subset.
- The achievability is also in doubt as stated: a pair with (1/M) Σ d_m ≤ D may have one component with d_m > D_m,
  and the AND rule then answers no, a false negative. A decoder that minimises the total distance over the product
  cell would be admissible, but the paper does not analyse it.
- So: the formula is stated there; the asymptotic converse for anisotropic spectra is, as far as these two papers
  go, unproved.

**The librarian's arithmetic on Conjecture A, checked.** Both statements are right. Isotropic, rate R nats per
coordinate: Conjecture A gives n·s with ½(s² − 1) − ln s = R, so s → e^(−R−½) at high rate (√e·κ_n/n below
Theorem 1) and s ≈ 1 − √R at low rate, against e^(−R) ≈ 1 − R. The diagnosis is also right, and it is exactly
entry 2 above: the cell laws average back to N(0, I), the proof of Conjecture A does not use it, and using it
(through the second moments) gives Theorem F1. Conjecture A survives as a per-cell lemma (Proposition A in
`CLAIMS.md`): it is the only bound here that holds cell by cell with the tail in full.

**Which route.** Not the adaptation of the Ingber–Courtade–Weissman converse. Reasons:
1. The transport route already gives the finite-n statement for general, non-commuting Σ_Q and Σ_R, in a page
   (F1), and the head constant on top (F2).
2. Their converse needs both laws rotation-invariant: the cell is projected on a sphere on which the query is
   uniform. With an anisotropic spectrum the typical sets are two different ellipsoidal shells and the needed
   isoperimetric inequality is not known (their own remark above). A product of spheres per eigen-block would
   need isoperimetry on products, which is not sharp.
3. At finite n the shell has a thickness, and the corrections (their η, D′, D″) do not vanish.
4. The isotropic finite-n analogue of their converse is already Theorem 1 at k = n (Urysohn in place of Lévy), as
   the librarian notes. F2 contains it.

What their paper adds to ours: asymptotic achievability in the isotropic case, so F1 is asymptotically tight
there; and a statement about a typical query rather than a mean, which F1 does not give.

Task B corrections from the PI, accepted: B1 is a citation (Bhatia, Jain, Lim 2019, Theorem 2); Σ√γᵢ is the value
for the product of second moments; the gain of the best φ + φ* over the ellipsoid is F − C* = ½(W₂² − G²);
φ + φ* is one-sided. One point goes further than the librarian's caution: the minimiser of the *mean* certificate
E‖Δ‖_K·E‖r‖_K° over all norms is still the M-ellipsoid (claim B2b in `CLAIMS.md`); the proof is the classical
M·M* ≥ 1 argument run through the Monge coupling.

## 2026-11-21, entry 6: trying to break F1 and F2 on explicit codes

`check_schemes.py`. The exact range of a code is its smallest sound certificate, so one code with exact mean width
below V_S(H) for some S would refute F2, and below W would refute F1.

- Part 1, product codes of interval partitions, n ≤ 5, every subset S (2,500 codes): smallest ratio to max_S V_S is
  1.000040, one coordinate at step 0.05σ, which is Theorem 1's known tight case. Smallest with n ≥ 2: 1.130.
  First version of V_subset took a 2,001-point grid over the split for each of 32 subsets and would have run for
  an hour; replaced by a golden-section search (the function is convex) that returns an evaluated point, which is
  at least the true minimum, so the test is the stricter one.
- Part 2, 2,800 planar partitions that are not products, γ = (1, ε²), exact through hull perimeters on a 360 × 360
  pixel grid (the pixelised partition is itself a partition, so nothing is approximated except the truncation of
  the plane at radius 8). Smallest ratio 1.135. At ε = 0.05 the binding head set is S = {1} and strips reach 1.223
  where Theorem 1 alone gives 1.346.
- Part 3, grids on a small head and shells of the dual norm on the tail, n up to 48 (300 codes): smallest ratio
  1.030 against max_k V_top-k and 1.080 against W.
- What these tests are worth. Parts 1 and 2 cannot press on the new part of the claim (the tail in full), because
  in one or two dimensions an uncoded coordinate costs about twice its floor to bound. Part 3 is the test that
  matters and it comes within 3%. Its shell masses are from 400,000 samples, so its figures carry about 0.2% of
  sampling error; nothing is within that of 1.
- I did not read `stage0-floor-check/`.

A by-product: L5, κ_k ≥ k for every k, from L2 applied to the uniform law on a ball. The consolidated proofs had it
by computation up to k = 200,000.

## 2026-11-21, entry 7: the floors on the measured spectra; the design at low rate

`floors_eval.py`: W reproduces `theory.log`'s "conjectured" column to four digits; V is 1 to 2% above it with a
head a little larger than the water-filling one and the whole rate on the head. Tables in `CLAIMS.md` section 7.
The index of `quant/floor2.py` is not on this machine (its paths point into another session's scratch directory),
so V could not be evaluated on that index; the transfer of V/W from the agent 1 spectra is marked as an estimate.

`design_lowrate.py`: the norm-only certificate on the 2^(1/16) grid stores 2.95 bits of entropy and sits 3.6%
above V at that entropy. Finer grids are worse by this measure. Theorem D1 in `CLAIMS.md`.

Something I had wrong at first: I expected V(0) = F. It is larger (0.0760 against 0.0728 on group 12), because
with no bits a head of 49 nearly equal coordinates already has κ_k·(geometric mean) above its arithmetic sum. The
true value at zero rate is infinite, and F4 shows the divergence for an isotropic spectrum; for a general spectrum
the divergence is not captured by anything here.

## 2026-11-21, entry 8: Task B

`ot_checks.py`, log `ot_checks.log`. B1 is a citation (Bhatia–Jain–Lim). Results beyond the brief:

- B2b. The M-ellipsoid also minimises the *mean* certificate E‖Δ‖_K·E‖r‖_K° over all norms, indeed over all
  product-form certificates. Proof: under the Monge coupling √⟨Δ, MΔ⟩ = ‖Δ‖_M ≤ √(‖Δ‖_K‖MΔ‖_K°), then
  Cauchy–Schwarz. This is the classical M·M* ≥ 1 argument. It settles the librarian's caution that the minimiser of
  the mean "need not be the geometric mean": it is. Checked on 240 random norms; smallest ratio 1.0000 (one
  dimension, where every norm is the same).
- B3. For scale mixtures the best φ + φ* gains F·(1 − E s) at most over the quadratic one (2 to 13% at the measured
  kurtoses), but the product form with the stored norm is already below the best φ + φ*. Nothing to build.
- B5. Rule 1's ρ⁻² is n_eff exactly when the level's coordinates decorrelate both the offsets and the errors.

## 2026-11-21, entry 9: second message from the PI (block norms measured); B4 redirected

Read `notes/program/stage0-block-norms/REPORT.md` and its notebook and tables. B4 now carries the identity and the
measurement in a few lines, and the weight is on one basis for a mixture of query families.

- The 0.6 has a constant. With Gaussian coordinates the best diagonal weighting per pair, Σ|uᵢ||eᵢ|, has mean
  (2/π)·F, because E|uᵢ||eᵢ| = (2/π)·(rms)(rms). Against one norm that is (2/π)(1 + cv²) = 0.641 on the
  experiment's spectra; measured 0.63 for items and for text in the mixed basis. For text in the data-only basis
  the measured 0.57 is 0.63 × 0.907, and 0.907 is exactly what refitting the diagonal weights on text gives there
  (bound b1). So the split is: 2/π is chance alignment inside a pair, the rest is the profile.
- Block norms under the same model: J = 2 / 4 / 8 / 16 equal-share blocks give 0.994 / 0.981 / 0.958 / 0.916 of
  one norm; measured for items 0.98 / 0.96 / 0.94 / 0.89. The model has about two thirds to three quarters of the
  gain.
- `mixture_price.py` rebuilds the experiment's bases from its saved fits and each group's residual covariance
  (one pass over the 200k vectors in chunks of 4,000 rows, cached in `work_sigmaR.npz`, 28 MB) and predicts the
  mean half-width of each bound from second moments alone, √(Σ D²P_test)·√(Σ λ/D²). Against the measurement at
  k = 0 / 32 / 256: mixed basis over data-only, held-out text 0.787 / 0.732 / 0.669 predicted, 0.788 / 0.732 /
  0.672 measured; items 1.052 / 1.080 / 1.179 predicted, 1.055 / 1.084 / 1.195 measured; in-sample species
  0.991 / 1.011 / 0.986 against 0.991 / 1.011 / 0.988. So the 21 to 33% and the 6 to 19% are what the calculus of
  second moments gives, to a few tenths of a point. The absolute widths are predicted 4 to 10% high, as an rms
  should be against a mean.
- The first run reported regrets "against the family's own optimum" computed from the test queries' own second
  moments. Those are not achievable gains: 100 item queries span about 100 directions, so their optimum is an
  overfit. I kept the ratios between bases and dropped that column from the claims.
- The trade-off along the mixing weight is lopsided, as theory says it must be (first order for the family added,
  second order for the family diluted). A text share of 0.2 keeps 78% of the text gain at k = 0 for 31% of the
  item cost.

## 2026-11-21, entry 10: the fit-only prediction; closing the session

- `mixture_price.py` rerun with each family evaluated on its own fit (no test query). Mixed over data-only: text
  0.725 / 0.676 / 0.513, items 1.054 / 1.099 / 1.257, against measured 0.788 / 0.732 / 0.672 and 1.055 / 1.084 /
  1.195. Right for items at k = 0, overstated with depth for both. So the calculus is exact given the queries'
  second moments (entry 9) and the fits are an imperfect stand-in for the held-out queries. In-family regrets at
  k = 0: data-only 1.001 (items), 1.462 (text); mixed 1.055, 1.061; text-only 1.229, 1.002. δ = 0.12.
- `check_chain1d.py`: the inner inequality of F1 tested directly on 160 interval partitions: C̄·e^H ≥ 1.0001,
  E h / C̄ ≥ 1.414.
- `check_f4.py`: the isotropic low-rate floor. First version reported a ratio of 0 for large balls: the cell mass
  rounded to 1 and the quantile overflowed. Fixed by using the survival function; the ratio is 1.0000 for centred
  balls, as it should be.
- F6 added: offsets that are scale mixtures of Gaussians multiply every floor by E[t]. One line, by conditioning.
- CLAIMS.md sections 12 (B4, rewritten after the PI's second message), 14 (failures and open points), 15 (files).

State at the end of the session.
- Proved, unrefereed: L1, L2, L3, L4, L5, A, F1, F2, F2a, F3, F4, F5, F6, D1, B2, B3 (weak duality and the
  scale-mixture bounds), B4b, B5. Cited: B1, the strong duality in B3, Sion, Hardy–Littlewood, Rockafellar.
  Sketch with gaps named: F2-alt.
- For the referee, the places I would attack first: L1 step 4 (the chain rule almost everywhere and the claim that
  T′ > 0 almost everywhere); L2 (measurability of the triangular map, the chain rule when some conditional
  entropies are −∞); F1 step 4 (h(z | c) = h(z) − I(z; c) for a randomised encoder); F2 steps 2 and 3 (the fibre
  of the closed support carries the conditional law; Borel measurability of the fibre supremum; Urysohn for a
  closed set that is not convex); F2a (W_T′(0⁺) when the largest tail eigenvalue is repeated).
- What I would do next, in order: (1) have F1 and F2 refereed; (2) ask the experimentalist for one more run of
  the block-norm set-up with the fit at text share 0.2, to test B4d's prediction; (3) a typical-query version of
  F1 by concentration of the support function on the cells of a code with bounded cells; (4) the anisotropic
  low-rate problem behind F4; (5) search the literature for L1 and B2b before calling either new.
