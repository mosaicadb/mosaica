# Referee report: floors track, core claims (CLAIMS.md sections 0 to 4)

Referee: independent agent, 21 November 2026. Scope: the local setting, L1 to L5, Proposition A, F1, F2 and F2a.
Not read: NOTEBOOK.md, and CLAIMS.md sections 5 onward except where sections 0 to 4 cite them.
Scripts are beside this file (`core_*.py`, logs `core_*.log`), run on core 3, nice 10, one thread.

This file is written in stages. A section headed "in progress" is not a verdict.

## Stage 0 (state when this file was first written)

Read so far: `notes/README.md`, `notes/00-notation.md`, `proofcheck/CONSOLIDATED_PROOFS.md` (all), CLAIMS.md lines
1 to 360. Nothing has been verified yet. Order of work: F1 with L1, L2, L3; then F2 with L4, L5; then Proposition A.

| Claim | Verdict so far |
|---|---|
| Setting (section 0) | not yet checked |
| L1 | not yet checked |
| L2 | not yet checked |
| L3 | not yet checked |
| L4 | not yet checked |
| L5 | not yet checked |
| Proposition A | not yet checked |
| F1 | not yet checked |
| F2, F2a | not yet checked |

## Stage 1 (in progress): setting, L1, L2, L3, F1 read line by line; numerics not yet run

Analytic reading only so far. Each item below is "no error found on reading"; the numerical attack is still owed.

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
