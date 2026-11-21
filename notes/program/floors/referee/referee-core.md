# Referee report: floors track, core claims (CLAIMS.md sections 0 to 4)

Referee: independent agent, 21 November 2026. Scope: the local setting, L1 to L5, Proposition A, F1, F2 and F2a.
Not read: NOTEBOOK.md, and CLAIMS.md sections 5 onward except where sections 0 to 4 cite them.
Scripts are beside this file (`core_*.py`, logs `core_*.log`), run on core 3, nice 10, one thread.

The report was written in stages, kept below in the order of the work. The verdicts are in this table; "Summary for
the PI" at the end has the unstated hypotheses, the relation to section 6 and to Theorems 1 to 3, and what was not
checked. "No error found" means every step was checked and an attack failed; it is not a certificate.

## Verdicts

| Claim | Verdict | Smallest ratio or gap found numerically |
|---|---|---|
| Setting (section 0) | no error found | reduction checked on singular, non-commuting Σ_Q, Σ_R (`core_setting.py`) |
| L1 | no error found; a known result if the citation is as remembered (not new) | not re-run in one dimension (the author's adversary did that) |
| L2 | no error found | gap 0.091 nats (adversary, 6×3 grid); 0.153 on shapes; needs ≥ 0 |
| L3 | no error found | closed form equals a numerical minimiser to 2·10⁻⁴; the minimiser never goes below it |
| L4 | no error found | every inequality holds on 400 spectra, equality only at H = 0 |
| L5 | no error found; confirmed by a second proof that does not use L2 | κ_k − k ≥ 0.6487 (k = 1) |
| Proposition A | no error found; one clause missing in (a), supplied in stage 4 | cell by cell E_g h / Φ(ln 1/P_c) ≥ 1.0011 |
| F1 | no error found; it is the section 6 conjecture, and a little more | E h / W ≥ 1.653 (plane), 1.687 (space), 1.00055 (shells, n = 10⁴) |
| F2 | no error found | inner inequality (*) ≥ 1.0006 (adversary, plane), 1.060 (space); E h / max_S V_S ≥ 1.131 (plane), 1.237 (space), 1.00045 (product code, n = 3,001) |
| F2a | no error found, ties included | brute force over u equals the formula to 5·10⁻¹⁶ |

No error, gap or counter-example was found in any claim of sections 0 to 4.

## Stage 1: setting, L1, L2, L3, F1

The first five paragraphs are the reading; the numerics follow them.

**Setting (section 0).** Checked: Δ·r = Σ_{i≤m} √γᵢ gᵢ zᵢ for almost every r and every g (the components of
Σ_Q^½ r along the null eigenvectors of B have variance 0); z ~ N(0, I_m); (r, c) independent of Δ also for a
randomised encoder, so E h = Σ_c P_c E_g h(c, ·); P_c ν_c ≤ γ_m for deterministic and randomised encoders alike;
(0.1), including that E_g s_c(g) is well defined in (−∞, ∞] (s_c dominates one linear function) and that
s_c(g) + s_c(−g) ≥ 0. Cells of probability 0 do not enter E h and need no ν_c.

**L1.** Steps 1 to 5 checked. Step 4 (pressed as asked): the identity f(T(x))·T′(x) = φ(x) is used only at points
where T is differentiable and F_μ is differentiable at T(x) with derivative f(T(x)); the exceptional set is
N₂ ∪ T⁻¹(N₁), which is Gaussian-null because μ(N₁) = 0 and T(g) has law μ. No change of variables through T is
used, only the law of T(g), so a singular part of T (gaps in the support, or f = 0 on a set of positive measure
inside the hull of the support) does no harm: it only enters step 3 with the right sign. Jensen in step 5 is
E ln T′ ≤ ln E T′, the right direction. No error found.

**L2.** Checked: joint measurability of the conditional quantile function; T^ν(g) has law ν by induction; the chain
rule is used for KL(ν ‖ γₙ) with non-negative terms, so no ∞ − ∞; E ln C(ν_{i|<i}) is quasi-integrable (its
positive part by ln C ≤ ½(m₂ − 1), its negative part by L1 and finiteness of the conditional KL); Jensen is
E ln C ≤ ln E C; E[gᵢ Tᵢ(g) | g_<i] = C(ν_{i|<i}). No error found.

**L3.** Checked, including H = 0 (λ = max aᵢ) and the Lagrangian argument. No error found. Zero eigenvalues are
outside by the reduction to rank m and contribute 0 to both sides of the section 6 formula.

**F1.** Steps 1 to 6 checked. The point of T^c(g) is a function of the same g for which the support function is
taken, so s_c(g) ≥ Σ √γᵢ gᵢ T^cᵢ(g) holds pointwise and soundness is used legitimately. Step 5 (C̄ᵢ ≤ 1) is Jensen
for the square root and Σ_c P_c E[zᵢ² | c] = 1. Unstated: I(z; c) < ∞ is needed in step 4 (if it is infinite the
claim is the trivial E h ≥ 0). No error found on reading.

**L2, numerical (`core_l2_2d.py`, log `core_l2_2d.log`).** The author tested L1 only in one dimension. For a density
constant on the rectangles of a product grid, h, C₁ and C₂ are closed forms, so
gap = ln(2πe) + ln C₁ + ln C₂ − h can be evaluated exactly, in both coordinate orders. 20,000 random densities
(with zeros, spikes, heavy weights): smallest gap 0.218. Gaussian restricted to ten non-product shapes (discs,
annulus, tilted strip, two discs, complement of a disc, L, checkerboard, tilted ellipse): smallest gap 0.1533 (a
thin tilted strip; it is the uniform law's ½(1 − ln 2) = 0.1534, as it should be). L-BFGS adversary over masses
and breakpoints on 2×2, 3×3, 4×4 and 6×3 grids: smallest gap 0.091, decreasing with the grid as the adversary
approaches a Gaussian. The correlated Gaussian (ρ = 0.8) gives gap 2·10⁻⁴, the discretisation error. No violation.

**F1, numerical attack 1: a search over planar cell shapes (`core_opt2d.py`, log `core_opt2d.log`).** A code is a
labelling of a 64×64 grid of squares on [−5, 5]² (unit squares outside, each its own cell, out to |z| = 12). Masses
and hull perimeters are exact, so E h (the exact range, E_g s_c = perimeter of the hull of Γ^½K_c over 2√(2π)) and
H(c) are exact for the code. The search moves one square at a time between neighbouring cells and keeps a move
when E h / max_S V_S(H) falls. 17 runs of 150,000 proposals, γ = (1, ε²) with ε = 1, 0.3, 0.1, 0.05, from product
grids, hexagonal lattices, strips with caps and random Voronoi partitions, 6 to 313 cells, H from 1.25 to 3.97
nats. Smallest E h / W = 1.653 (hexagons, ε = 1); smallest E h / max_S V_S = 1.131 (the same code; the hexagonal
lattice's known 1.05 at high rate plus the coarse grid). No code below either floor. The greedy search stalls
close to its start (300 to 2,700 accepted moves), so this is a test of many shapes, not a global optimum. As the
author says, two dimensions cannot press the tail: the floors are loose there by construction.

**F1, numerical check 2: the proof's inner inequalities on non-product partitions (`core_chain2d.py`, log
`core_chain2d.log`).** z replaced by its piecewise-constant version on a 100×100 grid so that all quantities are
exact; 21 partitions (grids, hexagons, Voronoi with 6 to 60 cells, strips), ε = 1, 0.3, 0.05, both coordinate
orders. Smallest values: coupling step, E_g s_c / Σ√γᵢCᵢ(ν_c) = 1.383 (needs ≥ 1); L2 per cell, gap 0.172 (needs
≥ 0); steps 3 and 4 together, Σᵢ ln(√(2πe)C̄ᵢ) − (h(z) − H(c)) = 0.208 (needs ≥ 0); step 5, √(E zᵢ²) − C̄ᵢ = 0.150.
No violation.

**Stage 1 verdicts.** L1: no error found. L2: no error found. L3: no error found. F1: no error found; it is the
section 6 conjecture exactly (Gaussian r, Gaussian Δ independent of r, deterministic sound certificate, H(c) ≤ B ln 2),
and slightly more (I(z; c) in place of H(c), randomised encoders, soundness only almost surely on the cell).

**L1 and Bolley–Gentil–Guillin (arXiv:1507.01086).** Checked: the arXiv record exists with the title "Dimensional
improvements of the logarithmic Sobolev, Talagrand and Brascamp-Lieb inequalities", authors Bolley, Gentil, Guillin.
Not checked: the text of Theorem 2.1 (the PDF was fetched but no text extractor is installed here, and the arXiv page
lists no journal reference). From memory, so [cited, unchecked]: the dimensional Talagrand inequality there is
W₂²(ν, γₙ) ≤ M₂(ν) + n − 2n·exp((M₂(ν) − n − 2 KL(ν ‖ γₙ))/(2n)). In one dimension the monotone coupling is optimal,
W₂² = 1 + m₂ − 2C(μ), and the inequality becomes KL ≥ ½m₂ − ½ − ln C(μ), which is L1's second form word for word.
If the statement is as remembered, L1 is its case n = 1 and is not new. L2 is not that theorem in n dimensions: BGG
use the Brenier map and get n·ln of the mean correlation; L2 uses the triangular map and gets Σᵢ ln Cᵢ, which is
what the water-filling needs (a separate correlation per eigen-direction). L2's proof does not depend on the
citation.

## Stage 2: L4 and L5

**L4.** All five items checked line by line.
(1) Φ and W are values of a linear objective over a set that is jointly convex in (s, H); the minimum is attained
because each term of the constraint is non-negative on (0, 1], so every sᵢ is bounded away from 0. Φ(0) = W(0) = F
because k(s) = 0 or ln(1/s) = 0 on (0, 1] only at s = 1. V_S is the infimal convolution of u ↦ W_{S^c}(u) and
v ↦ κ_k·GM·e^(−v/k) on u, v ≥ 0, both convex and non-increasing, hence convex and non-increasing.
(2) k(s) ≤ ln(1/s) on (0, 1]. (3) k(1) = k′(1) = 0, k″ = 1 + s⁻² ≥ 2, so k(s) ≥ (1 − s)²; then Cauchy–Schwarz.
(4) both bounds follow from L3's closed form (a − λ ≤ a·ln(a/λ); ((a − λ)/a)² ≤ 1 − λ/a ≤ ln(a/λ)); the exact
form needs γ₁ > γ₂ to be non-empty, and with a tie at the top it holds only at H = 0, as the stated range says.
(5) each term of the minimand is bounded below at its own best u and the other is non-negative.
Verdict: no error found. Unstated but harmless: V_S with S^c empty is κ_n·GM·e^(−H/n) (W over no indices is 0).

**L5.** The proof is correct given L2 (the uniform law on a ball has a density, finite second moments and finite
entropy; T^ν(g) lies in the ball almost surely; arithmetic–geometric mean in the right direction). Independent
confirmation that does not use L2: Wendel's inequality gives E‖g_k‖ = √2·Γ((k+1)/2)/Γ(k/2) ≥ k/√(k+1), and
Stirling's lower bound Γ(x+1) ≥ √(2πx)(x/e)^x at x = k/2 gives √(2πe)·ω_k^(−1/k) ≥ √k·(πk)^(1/2k); so
κ_k ≥ k·√(k/(k+1))·(πk)^(1/2k) ≥ k, because (πk)^(1/k) ≥ 1 + ln(πk)/k ≥ 1 + 1/k (ln π > 1). Numerical check in
`core_f2_misc.py` (stage 3). Verdict: no error found; the result is true by a second, elementary proof.

## Stage 3: F2 and F2a

### Reading (numerics follow below)

**Step 1 (order, independence).** z_T = T^c(g_T) is a function of g_T and of the cell only, so it is independent
of g_S, and it has law ν_{c,T} exactly (L2 on the T-marginal, whose density is at most (2π)^(−|T|/2)/P_c). The
cell is fixed before g is drawn and the bound is taken cell by cell, so nothing depends on the future.

**Step 2 (the fibre).** ν_c(K) = 1 disintegrates to ν_{c,S|z_T}(K^{z_T}) = 1 for ν_{c,T}-almost every z_T, and z_T
has law ν_{c,T}, so the property holds at T^c(g_T) almost surely. The inequality
s_c(g) ≥ Σ_T √γᵢ gᵢ (z_T)ᵢ + sup over the fibre of Σ_S √γᵢ gᵢ sᵢ holds pointwise in g = (g_T, g_S): the right side
is a supremum over a subset of K (the points of K whose T-part is z_T) of the same linear function, for the same
g. So the half-width is compared with the support function for the same g, as soundness requires. No point of the
fibre is selected, so no measurable-selection argument is needed. Measurability of the fibre supremum: K is closed,
K ∩ {|z| ≤ j} is compact, the supremum over the fibre of a compact set is upper semicontinuous in (z_T, g_S)
(−∞ on empty fibres), and the supremum over j is Borel. Checked.

**Step 3 (Urysohn on the fibre).** Conditional on g_T the fibre is a fixed closed set and g_S is standard Gaussian
in ℝᵏ, so step 3 of Theorem 1 applies to Γ_S^½K^{z_T} as written; the volume scaling is right; an unbounded fibre
gives +∞ (the cone argument is correct: a convex cone that is not the whole space lies in a half-space). The
expectation is well defined because the fibre supremum dominates a linear function of g_S. ln vol(K^{z_T}) ≥
h(ν_{c,S|z_T}) needs the conditional law to have a density, true for almost every z_T.

**Step 4 (Jensen in the cell).** E exp(X/k) ≥ exp(E X/k) with X = h(ν_{c,S|z_T}), z_T ~ ν_{c,T}; E X = a_c is
finite because a_c = h(ν_c) − h(ν_{c,T}) and both are finite. The expectation over g_T is the expectation over
ν_{c,T} only because the tail point has exactly the cell's tail law; this is where the order matters, and it is
used correctly.

**Steps 5 and 6 (the split of the rate).** a_c + b_c = h(ν_c); ā + b̄ = h(z | c) = (n/2) ln(2πe) − I(z; c);
b̄ ≤ Σ_T ln(√(2πe)C̄ᵢ) (L2, then Jensen for the logarithm); C̄ᵢ ≤ 1; with u = Σ_T ln(1/C̄ᵢ), ā ≥ (k/2) ln(2πe) −
(I − u); u ≤ I because ā = h(z_S | z_T, c) ≤ h(z_S). Every direction checked. The split is exact: nothing is
counted twice and nothing is dropped.

**F2a.** (Π_S γᵢ)^(1/2k)·e^(−H/k) = √θ is the water-filling equation. f is convex; f′(0⁺) = (κ_k/k)√θ − √γ_{k+1}.
With a repeated eigenvalue at the top of the tail (γ_{k+1} = … = γ_{k+j}) L3 gives W_T(u) = j·√γ_{k+1}·e^(−u/j) +
rest for small u, whose right derivative at 0 is still −√γ_{k+1}; so the claim W_T′(0⁺) = −√γ_{k+1} holds with
ties. γ_{k+1} ≤ θ by the definition k = #{γᵢ > θ} (eigenvalues equal to θ are in the tail), and κ_k ≥ k (L5), so
f′(0⁺) ≥ 0 and the minimum is at u = 0. With T empty (k = n) f is increasing. H > 0 guarantees k ≥ 1. Checked.
Remark, not an error: at a tie F2a is not the best head. For γ = (4, 1, 1) and H = ln 2 the water-filling head is
{1} and F2a gives κ₁ + 2 = 3.649, while F2 with S = {1, 2, 3} gives κ₃ = 4.092. Both are valid.

### Numerics for F2, part 1: the inner inequality in the plane (`core_f2chain.py`, log `core_f2chain.log`)

Steps 1 to 4 of F2 prove, for any law ν with a density carried by a set K (a Gaussian cell or not),

    (*)  E_g h_{Γ^½K}(g) ≥ Σ_T √γᵢ Cᵢ(ν_T) + E‖g_k‖·ω_k^(−1/k)·(Π_S γᵢ)^(1/2k)·exp((h(ν) − h(ν_T))/k).

This is the step in which the head and the tail meet, so it is the one to attack. In the plane, with one head and
one tail coordinate, both sides are exact for densities constant on a product grid.
- Ten shapes (discs, tilted and steep strips, triangle, tilted ellipse, square, L, two discs), Gaussian restricted
  to each, four spectra, both choices of the head: smallest LHS/RHS = 1.026 (a square with the small eigenvalue on
  the tail).
- An adversary: 300 random supports on grids up to 6×6 (full rectangles, random masks, staircases), L-BFGS over the
  density to minimise LHS/RHS: smallest 1.00062. The adversary finds the equality case (a rectangle, the tail mass
  pushed to its two ends, the head uniform, which I computed by hand to be an equality) and does not pass it.
- 21 partitions: per cell 1.0036; the averaged form of step 6, E h / (Σ_T √γᵢC̄ᵢ + head term at ā), 1.029; the
  entropy step b̄ ≤ ln(√(2πe)C̄_T) has slack 0.054.
No violation. A first run reported 0: that was an overflow in my own routine for masses below 1e-308, fixed
(the routine now agrees with brute-force quadrature to 2·10⁻⁶ on laws with one mass as small as 10⁻⁴⁰), and the
stage 1 logs were regenerated with the fixed routine and are unchanged.

### Numerics for F2, part 2: three dimensions (`core_f2_3d.py`, logs `core_f2_3d_part1.log`, `core_f2_3d_part2.log`)

- Single cells, the inequality (*) with a head of two coordinates and a tail of one (Urysohn in the plane on each
  fibre) and with a head of one and a tail of two (the triangular coupling in the plane), all nine assignments of
  the axes and both orders of the tail, four spectra and their permutations, eight shapes (ball, cube, tilted
  slab, tilted rod, simplex, tilted plate, off-centre ball, flat box). The Gaussian mean width of each hull is
  exact (Qhull, then E‖g₃‖·M/(4π) with M half the sum of edge length times exterior angle; checked on the cube).
  Smallest LHS/RHS = 1.060 (the flat box, tail of two small eigenvalues). No violation.
- Whole codes on the cube [−4, 4]³ in 32³ voxels: Voronoi partitions in the Euclidean and the spectrum's metric
  (8 to 50 cells), cubic and body-centred lattices, and a grid on coordinate 1 times a disc with eight outer
  sectors on coordinates 2 and 3; spectra (1,1,1), (1,1,0.1), (1,0.1,0.1), (1,0.3,0.05) in √γ; all eight head sets.
  E h and H are exact for the piecewise-constant law on the cube (the Gaussian's mass outside is 1.9·10⁻⁴, so the
  comparison with the Gaussian floor is approximate at that level). Smallest E h / max_S V_S = 1.237 (body-centred
  lattice, isotropic, S = all); smallest with a binding S that leaves a non-empty tail (S = {1, 2}, tail {3}) =
  1.546. Smallest E h / W = 1.687. No code below.

### Numerics for F2, part 3: lemmas, F2a with ties, high dimension (`core_f2_misc.py`, log `core_f2_misc.log`)

- L5: κ_k − k is smallest at k = 1 (0.6487, κ₁ = √e) over every k ≤ 5,000 and a log grid to 10⁷.
- L3: closed form against a constrained minimiser (SLSQP) on 300 programmes, n ≤ 5, with ties, H = 0, 0.3, 2, 12:
  agreement to 2.4·10⁻⁴ (the minimiser's tolerance), and the minimiser never goes below the closed form.
- L4: on 400 spectra (ties included) and 13 rates each, W ≥ Φ ≥ F − √(HΣγ) and W ≥ F − min(√γ₁H, √(HΣγ)) hold with
  equality only at H = 0; the exact form of item 4 holds on its range; midpoint convexity and monotonicity of W, Φ
  and V_S hold to rounding (smallest defect +1.4·10⁻¹⁰).
- F2a with repeated eigenvalues: for (4,1,1), (4,4,1,1,¼), (9,4,4,4,1), (1,1,1,1), (16,4,1,1,1,0.01) and 60 random
  spectra, at 7 rates including those that put θ exactly on a repeated eigenvalue, the minimum over a grid of 4,001
  values of u of the F2 minimand equals κ_k√θ + Σ_{i>k}√γᵢ to 5·10⁻¹⁶: the minimum is at u = 0 in every case.
- High dimension, exact through the chi law. One head coordinate on a uniform grid of step w times shells of
  thickness δ on an isotropic tail of n_T coordinates with √γ = ε. These are product codes, and a product code
  cannot fall below F2 if Theorem 1 holds for each factor, so this run cannot refute F2; it shows how tight the
  floor is. Smallest E h / max_S V_S: 1.043 at n_T = 30, 1.0045 at n_T = 300, 1.00045 at n_T = 3,000 (the binding
  head set there is not {1}; against V_{1} the ratio is 1.0017 and against W 1.0019). Shells alone against W:
  1.032, 1.0043, 1.00055 at n = 100, 1,000, 10,000, an excess of about 0.9 + ½ ln n in units of √γ, as a hand
  computation predicts; against Theorem 1 the excess is 0.557 at every n. So "the tail in full" is tested to within
  (ln n)/n and holds.

**What the F2 numerics do and do not cover.** The inner inequality (*) was attacked directly in two and three
dimensions with non-product cells and an adversary, and is tight (1.0006) and unbroken. Whole codes with coupled
head and tail were tested in two and three dimensions only, where the floor is loose (1.13 and above). In high
dimension only product codes were computed. No high-dimensional code with cells that couple the head to the tail
was tested by anyone.

## Stage 4: Proposition A

**Reading.** (a) KL(ν ‖ γₙ) < ∞ gives finite second moments (Donsker–Varadhan with λ|z|², λ < ½) and h(ν) > −∞, so
L2's hypotheses hold. The chain rule for KL, L1's second form on each conditional law, and E ln C ≤ ln E C give
KL ≥ Σᵢ[½E zᵢ² − ½ − ln Cᵢ]; E zᵢ² ≥ Cᵢ² gives Σᵢ k(Cᵢ). With sᵢ = min(Cᵢ, 1): k ≥ 0 everywhere with k(1) = 0, so
k(sᵢ) ≤ k(Cᵢ) also when Cᵢ > 1 (the author's "k decreases on (0, 1]" covers only Cᵢ ≤ 1; the other case needs
k ≥ 0, which is true), and Σ√γᵢCᵢ ≥ Σ√γᵢsᵢ ≥ Φ(KL). (b) the coupling step of F1, then dν_c/dγₙ ≤ 1/P_c gives
KL(ν_c ‖ γₙ) ≤ ln(1/P_c), and Φ is non-increasing. (c) Jensen for the convex Φ in the right direction
(Σ_c P_c Φ(x_c) ≥ Φ(Σ_c P_c x_c)); Σ_c P_c KL(ν_c ‖ γₙ) = I(z; c) because the cell laws mix to γₙ; Φ is continuous
at 0 (L4.3), so Jensen holds for countably many cells. Verdict: no error found. It is weaker than F1 on average
(Φ ≤ W) and is the only one of the three that holds cell by cell.

**Numerics (`core_propA.py`, log `core_propA.log`).** The cheapest cells for their probability are centred at the
origin. Centred balls, isotropic, n from 1 to 10⁴, 54 radii each: smallest E_g h / Φ(ln 1/P) = 1.0011 (n = 10⁴; the
small-radius limit is κ_n/n, so (b) is nearly tight in high dimension and never broken). 4,000 centred boxes with
random sides and spectra, n ≤ 6: smallest 1.6487 (= √e, one short interval). 75 centred ellipses in the plane:
smallest 1.461 (= κ₂/2). The author's remark that (b) has no analogue for W is confirmed by example: for the same
balls E_g h / W(ln 1/P) falls to 0.62 (n = 100), and a small disc at the origin in the plane has the limit
κ₂/(2√e) = 0.886. This is not an error in F1, which is claimed only on average.

## Summary for the PI

### Is each result the conjecture of section 6, or weaker, or stronger?

- **F1** is the "full floor" conjecture exactly: under the model of Theorems 2 and 3 (r ~ N(0, Σ_R), Δ = Σ_Q^½ g
  independent of r, a deterministic sound certificate, H(c) ≤ B ln 2), E h ≥ Σᵢ √min(γᵢ, θ) with
  Σᵢ ½ log₂⁺(γᵢ/θ) = B. It is stronger in four small ways: the rate is I(z; c), which is at most H(c); the encoder
  may be randomised; soundness is needed only for almost every point of a cell; Σ_Q and Σ_R may be singular and need
  not commute. It is the argument section 6 called missing: one coupling gives the head and the tail together.
- **F2a** is stronger than the conjecture: κ_k√θ + Σ_{i>k}√γᵢ, larger by (κ_k − k)√θ. **F2** is a family of floors,
  one per head set; F1 is its case S = ∅.
- **Proposition A** is weaker than F1 on average (Φ ≤ W; at high rate each coded coordinate counts √θ/√e) and is not
  the conjecture. Its part (b) holds cell by cell, which F1 does not (confirmed: a small disc at the origin has
  E_g h = 0.886·W(ln 1/P_c)).

### Do Theorems 1 to 3 follow?

- Corollary 1 of Theorem 1 (Gaussian residuals), for every k: yes, from F2 with S = {1, …, k} and L4.5. For a
  subspace that is not spanned by eigenvectors the Gaussian case of Theorem 1 follows too, because the determinant
  of a k-dimensional compression of the matrix is at most γ₁⋯γ_k.
- Theorem 1 itself, no: it covers any residual with a density on a subspace, and Corollary 2 covers shared
  information; F2 is proved only under the Gaussian model (F3 and F5, outside this report, are where the author
  treats those). F2 is also not independent of Theorem 1: its step 3 is Theorem 1's step 3 applied to a fibre.
- Theorem 2: yes, from F1. W ≥ W_T for every index set T, and L4.4 gives W_T(H) ≥ Σ_T √γᵢ − √(H·Σ_T γᵢ), which is
  above Theorem 2's bound.
- Theorem 3: yes, from F2. V_S(H) ≥ κ_k(Π_S γᵢ)^(1/2k)e^(−H/k) + W_T(H) is not what L4.5 states (L4.5 gives the
  maximum of the two terms, not the sum), but it holds: in the minimand the head term is at least its value at
  u = 0 and W_{S^c}(u) ≥ W_{S^c}(H) ≥ W_T(H) for every u ≤ H. With L4.4 this is above Theorem 3's bound, with no
  added bit and no 1/√e.

### Unstated hypotheses

1. h(c, ·) must be a measurable function of Δ for E h to exist. Standard, not stated.
2. I(z; c) < ∞ in the averaging steps of F1, F2 and A(c). If it is infinite every claim reduces to E h ≥ 0.
3. The certificate may depend on the item only through c and on the query only through Δ. A seed shared by the
   encoder and the certificate is not covered by the words "randomised function of r"; it is covered by the proofs
   if one conditions on the seed and uses the convexity of W, V_S and Φ, giving the floors at I(z; c | seed).
4. The model is exact: r Gaussian with mean zero, Δ Gaussian and independent of r. The author says this under
   "What is not claimed"; it is repeated here because a unit query minus a centre is not Gaussian.
5. In F2 the head set has k ≥ 1; its complement may be empty (then W over no indices is 0). In F2a, eigenvalues
   equal to θ belong to the tail.
6. In Proposition A(a), KL(ν ‖ γₙ) < ∞ is what supplies L2's hypotheses (finite second moments, h > −∞). True, not
   said.

### Points for the author (none changes a statement)

1. Proposition A(a): "k decreases on (0, 1] and k(1) = 0" does not cover a correlation Cᵢ > 1, which a cell far
   from the origin can have. Add "and k ≥ 0 on (0, ∞)". Then k(min(Cᵢ, 1)) ≤ k(Cᵢ) in both cases.
2. F1, remark on tightness: the factor "√e in one" dimension is the whole slack between E h and W for an interval
   cell at high rate. It is the product of two losses, the coupling's √2 (the supremum over the interval against
   the correlation with its uniform law) and L1's √(e/2) for a uniform law, as the author's own check
   (`check_chain1d.log`, 1.414) shows. The sentence reads as if the coupling alone cost √e.
3. L1, prior art: the sentence "the librarian did not find the inequality" is out of date if the Bolley–Gentil–
   Guillin statement is as I remember it. L1 should then carry [cited] beside its proof, and should not be called
   new. L2, the triangular version with one correlation per coordinate, is what F1 needs and is not that theorem.
4. F2a: at a repeated eigenvalue the water-filling head is not the best head (example in stage 3). The text says the
   best head is "a little larger"; a tie is the clean case of it.
5. Theorem 3 follows from F2 by the sum bound written above, which is slightly more than L4.5 states.

### What I tested that the author had not

L2 in two dimensions, exactly, with an adversary. The inner inequalities of F1 on planar partitions that are not
products. A search over planar cell shapes. The inequality (*) in which F2's head and tail meet, in the plane with
an adversary over densities and in space with exact mean widths. Whole codes in space against all eight head sets.
F2a with ties by brute force. κ_k ≥ k by a second proof. Proposition A cell by cell. Product codes up to 10⁴
dimensions.

### What I did not check

- NOTEBOOK.md and CLAIMS.md from section 5 on (F3 to F6, F2-alt, prior art, the measured spectra, D1, task B).
- The text of Bolley–Gentil–Guillin's Theorem 2.1: only the arXiv record was confirmed.
- Theorems 1 to 3 themselves; Theorem 1's step 3 (Urysohn for a bounded set) is used as refereed.
- The author's scripts were not re-run and their logs not reproduced. I read `floors_lib.py` in full and the head of
  `check_schemes.py`; my own numbers agree with the author's in size (1.653 against 1.658 in the plane).
- Any high-dimensional code whose cells couple the head to the tail. In high dimension only product codes were
  computed, and a product code cannot fall below F2 if Theorem 1 holds, so that run measures tightness and refutes
  nothing. In two and three dimensions, where coupled cells were tested, the floors are loose (1.13 and more).
- A global optimum over cell shapes: my planar search is greedy and stops near its start.
- Exactness beyond floating point: no interval arithmetic. The three-dimensional whole-code comparison uses the
  Gaussian cut to a cube (mass 1.9·10⁻⁴ outside).
- Whether the Gaussian model fits the data, and anything about randomised (δ-valid) certificates: neither is
  claimed in sections 0 to 4.

### Files

`core_l2_2d.py` (L2 in the plane), `core_chain2d.py` (F1's inner inequalities), `core_opt2d.py` (search over planar
cells), `core_f2chain.py` (F2's inner inequality in the plane), `core_f2_3d.py` (the same in space, and whole
codes), `core_f2_misc.py` (L3, L4, L5, F2a with ties, high dimension), `core_propA.py` (Proposition A cell by cell),
`core_setting.py` (the reduction of section 0); each with a log of the same name (`core_f2_3d_part1.log`,
`core_f2_3d_part2.log` for the two parts).
