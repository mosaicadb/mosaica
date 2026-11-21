# Rotations track: claims

Rotations theorist, 21 November 2026. The owner's question is whether the certified bound of the randomised levels can
be re-derived for a rotation cheaper than the dense Haar matrices (9.9 MB in f64 at n = 2048, about 1 ms per group
setup). Tags are those of `notes/README.md`. Every proof here is mine and unrefereed. Literature statements are taken
from the librarian's report `../library/rotations.md` and carry its reading tags; I opened no source myself, so each is
[cited, unchecked] unless the librarian read the place named. Numerics are in the `*.py` files beside this one, with
their output in `out_*.txt`; nothing was timed.

<!-- ANSWERS -->

## Notation

Chapter 0's and chapter 3's symbols hold. For one block: y is the item's block in ℝⁿ, R the rotation, z = R·y, ō the
codeword (a function of z), κ = ‖y‖²/⟨ō, z⟩, e = κō − z the error in rotated coordinates, E = Rᵀe = ŷ − y, A = ‖e‖ =
‖y‖·tan φ, Δ the query's weight in the block and Δ_⊥ = P_{y⊥}Δ. The error of the estimate is Δ·E = ⟨e, RΔ⟩. d = n − 1.
"The law" is the scalar statement the constants consume: given z, Δ·E = A·‖Δ_⊥‖·U with U one coordinate of a uniform
unit vector of ℝ^d. sd = A·‖Δ_⊥‖/√d. H is the normalised Walsh–Hadamard matrix in Sylvester order,
H_{k,i} = χ_i(k)/√n with χ_i(k) = (−1)^popcount(i and k), indices from 0; D, D₁, D₂, … are independent diagonals of
uniform signs d(i); "k rounds" is H·D_k ⋯ H·D₁. For a set T of indices, ⟨T⟩ is its span under xor. c = 7.8 stands for
today's constant where a number is needed, and δ = 2.5·10⁻¹⁹ ≈ 2⁻⁶² for the per-event budget at N = 10⁹, L = 4.

## A. What the law needs

**A1 (the exact condition).** [proved, unrefereed] Let μ be a law on O(n), n ≥ 3. The following are equivalent.

1. For every y, every Δ fixed independently of R, and every codebook (every measurable ō(z) with ⟨ō, z⟩ > 0), the law
   holds: given z, Δ·E has the law of A·‖Δ_⊥‖·U.
2. For every orthonormal pair (a, b), the pair (Ra, Rb) is uniform on the Stiefel manifold V₂(ℝⁿ).

For a bound in place of the exact law the following suffices: if for every orthonormal (a, b) the conditional law of
Rb given Ra has a density at most M against the uniform law on the unit sphere of (Ra)^⊥, then for every codebook
Pr[Δ·E ≥ x | z] ≤ M·Pr[A·‖Δ_⊥‖·U ≥ x], and with J independently rotated blocks the factor is M^J.

*Proof.* (2 ⇒ 1) Let a = y/‖y‖ and b = Δ_⊥/‖Δ_⊥‖. Uniformity on V₂ means that given Ra the vector v = Rb is uniform on
the unit sphere of (Ra)^⊥ = z^⊥. Now RΔ = (Δ·y/‖y‖²)·z + ‖Δ_⊥‖·v and e ⟂ z, so Δ·E = ⟨e, RΔ⟩ = ‖Δ_⊥‖·⟨e, v⟩. Given z,
e is a fixed vector of z^⊥ of norm A and v is uniform on the unit sphere of z^⊥, a space of dimension d, so ⟨e, v⟩ has
the law of A·U. (1 ⇒ 2) Take y = a, Δ = b. For a unit vector field e(z) ∈ z^⊥ and a positive function t(z), the
codebook ō = z + t(z)·e(z) has κ = 1, error t(z)·e(z) and A = t(z). By 1, ⟨e(z), Rb⟩ has the law of U given z for
every such field. Apply this to the fields e_w(z) = P_{z⊥}w/‖P_{z⊥}w‖ for w in a countable dense set: given z, every
one-dimensional marginal of Rb in z^⊥ is that of the uniform law on the sphere of z^⊥, so by Cramér–Wold the
conditional law of Rb is that uniform law. It remains to show Ra is uniform. Write T for the operator that sends a law
ν on the sphere to ∫ unif(S(x^⊥)) dν(x). The conditional statement for (a, b) gives law(Rb) = T·law(Ra), and for (b, a)
it gives law(Ra) = T·law(Rb), so law(Ra) = T²·law(Ra). T commutes with rotations, so it multiplies the degree-k
spherical harmonics by a constant λ_k, the value at 0 of the normalised Gegenbauer polynomial: λ_k = 0 for odd k and
|λ_k| < 1 for even k ≥ 2 when n ≥ 3. So every harmonic component of law(Ra) of degree at least 1 vanishes, and Ra is
uniform. The bound with M: given z the density of v is at most M times uniform, so every conditional probability is at
most M times its uniform value; blocks are independent. ∎

Remarks. (i) The PI's answer A asks for invariance of μ under R → R·Oᵀ for every O fixing the block. Asked for every
block, those O generate O(n), so that condition is Haar itself. It is sufficient and not necessary: the necessary and
sufficient condition is pair-uniformity, and H2 shows it is strictly weaker than Haar. (ii) The librarian's reduction
(report, section 0) is this claim; its "if and only if" is right for "every codebook", and for one fixed codebook
pair-uniformity is sufficient only. (iii) The bound with M costs c² → c² + 2·ln M at a sub-Gaussian tail, since the
budget becomes δ/M. A total-variation distance τ gives only Pr ≤ δ + τ, which needs τ ≲ δ.

**A2 (a finitely supported rotation leaves nothing random).** [proved, unrefereed] Let R take finitely many values
R₁, …, R_S (any product of sign diagonals, Hadamard matrices and permutations, with any number of rounds; S ≤ 2^(kn)
for k rounds). For every y outside the finite union of proper subspaces ker(R_s − R_t), s ≠ t, the matrix R is a
function of z = Ry. For such y and any Δ not parallel to y there is a codebook, a function of z alone, for which
Δ·E = A·‖Δ_⊥‖ with probability one: the error sits at its Cauchy–Schwarz size, √d sd.

*Proof.* If R_s y = R_t y then y ∈ ker(R_s − R_t), a proper subspace when R_s ≠ R_t. So z determines R, and RΔ_⊥ is a
function g(z) with g(z) ⟂ z. Take ō(z) = z + g(z): κ = 1, e = g(z), and Δ·E = ⟨g(z), RΔ⟩ = ‖Δ_⊥‖² = A·‖Δ_⊥‖. ∎

So with a finitely supported rotation "any codebook" is lost outright, for every number of rounds; what remains is
whether the code as built, with data chosen against it, can be made to fail. Section B answers that. The PI's
statement "given R·y the product is generically determined, so nothing is left random for the query side" is right as
far as it goes; by itself it refutes the proof and the exact law, not a tail bound for one fixed codebook, because a
bound needs only the joint law of (e(z), RΔ) and not a conditional one.

## B. Sign–Hadamard products against the code as built

The code as built (`rand_encode`, `rand_choose`): position p is coded as the cell of z_p/(λ·rms) on the grid of step
2^(1−depth) over [−1, 1), λ one of six values chosen per item to minimise ln tan²φ, decoded to the cell's midpoint. An
exact zero lies in the cell [0, 2^(1−depth)) and decodes to +2^(−depth).

**B0 (one round: the error lives on the xor-span of the item's support).** [proved, unrefereed] Let R = H·D, let T be
the support of y, and let every position of the block have the same depth. Then z depends on D only through the signs
d(i), i ∈ T; and E = D·g with g = H·e supported on ⟨T⟩, at most 2^|T| coordinates, whatever n is.

*Proof.* z_k = n^(−½)·Σ_{i∈T} d(i)·y_i·χ_i(k). The scale λ·rms and κ are functions of z, and given them e_k = f(z_k)
for one function f. So e_k depends on k only through (χ_i(k))_{i∈T}, that is, e is constant on the cosets of the
subgroup T^⊥ = {k : χ_i(k) = 1 for all i ∈ T}. The Walsh transform of such a function is supported on (T^⊥)^⊥ = ⟨T⟩.
E = Rᵀe = D·H·e. ∎

So an item with few non-zero coordinates has its whole error energy A² on at most 2^|T| fixed coordinates, where the
law spreads it over n − 1 dimensions. A query on those coordinates sees a spread larger by about √(n/2^|T|).

**B1 (the PI's counter-example, exact).** [proved, unrefereed; verified numerically, `b1_oneround.py`,
`out_b1.txt`] Let R = H·D, n ≥ 4 a power of two, y = e_a + e_b with a ≠ b, s = a xor b, every position at one depth
(any depth from 1 to 8, any of the six scales). Then for every draw

    E = (A/√2)·( d(0)·e₀ − d(a)·d(b)·d(s)·e_s ),   tan φ = 1 / (2^depth − 1)  (1, 1/3, 1/5 at depths 1, 2, 3 with λ = 2).

- With a = 0, so that s = b: E = ±(A/√2)(e₀ − e_b). For Δ = (e₀ − e_b)/√2, which is orthogonal to y, the error is
  +A·‖Δ‖ with probability ½ and −A·‖Δ‖ with probability ½: the full Cauchy–Schwarz size, √(n − 1) sd, 15.97 sd at
  n = 256, on each side with probability ½.
- With a, b, s, 0 distinct: for Δ = (e₀ + e_s)/√2 the error is +A‖Δ‖ with probability ¼, −A‖Δ‖ with probability ¼ and
  0 otherwise; for Δ = e_s alone it is ±A‖Δ‖/√2 with probability one (11.29 sd at n = 256).

*Proof.* z = d(a)·h_a + d(b)·h_b, so z_k = (d(a)χ_a(k) + d(b)χ_b(k))/√n is ±2/√n where χ_s(k) = d(a)d(b) and 0 on the
set S = {k : χ_s(k) = σ}, σ = −d(a)d(b), which is half the positions. The non-zero coordinates have one magnitude, so
at every depth and scale their decoded values are ±v₁ for one v₁ > 0 with the right signs, and the zeros decode to
+v₀, v₀ = 2^(−depth). κ makes the code exact on the non-zero half, since ⟨ō, z⟩ involves only that half: κ·v₁ = 2/√n.
So e = κ·v₀·1_S = κv₀·(1 + σχ_s)/2 = (κv₀√n/2)·(h₀ + σh_s), and E = D·H·e = (κv₀√n/2)·(d(0)e₀ + σ·d(s)e_s). ‖E‖ = A
fixes the factor at A/√2. The cases follow, the signs d(0), d(a)d(b)d(s) being independent uniform signs when 0, a,
b, s are distinct. tan φ = A/‖y‖ = v₀/v₁, and with λ = 2 the non-zero value √2/λ = 0.707 has v₁ = 1 − 2^(−depth). ∎

The PI's statement is confirmed with two corrections. The pattern is aligned with e₀ and e_s together (the e₀ part
comes from the tie rule, which sends every zero up), so Δ = e_s alone reaches 1/√2 of Cauchy–Schwarz with probability
one, and the full size with probability ½ needs a = 0 (or ¼ per side otherwise). And the example does not depend on
the depth. A symmetric tie rule would code this particular y exactly, but B0 does not depend on the tie rule:

**B2 (one round, items with k non-zero coordinates).** [computed, `b1_oneround.py`: n = 256, depth 2, random values
on k random positions, one fixed query spread over ⟨T⟩ and orthogonal to y, 6,000 to 20,000 sign draws]

| k | size of ⟨T⟩ | share of ‖E‖² outside ⟨T⟩ | rms of error/sd | Pr[\|error\| > 7.8 sd] |
|---|---|---|---|---|
| 2 | 4 | 0 | 10.3 | 1 |
| 3 | 8 | 0 | 7.9 | 1 |
| 4 | 16 | 0 | 3.8 | 0.060 |
| 5 | 32 | 0 | 2.5 | 8·10⁻⁴ |
| 6 | 64 | 0 | 1.9 | none in 6,000 |
| 7 | 128 | 0 | 1.4 | none in 6,000 |

The law's rms is 1 and its probability 5·10⁻¹⁹. One round therefore fails by many orders for any item with up to
about log₂ n non-zero coordinates in the block's basis, and its error spread is wrong (rms above 1) for every sparse
item.

<!-- END -->
