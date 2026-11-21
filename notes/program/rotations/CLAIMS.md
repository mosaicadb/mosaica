# Rotations track: claims

Rotations theorist, 21 November 2026. The owner's question is whether the certified bound of the randomised levels can
be re-derived for a rotation cheaper than the dense Haar matrices (9.9 MB in f64 at n = 2048, about 1 ms per group
setup). Tags are those of `notes/README.md`. Every proof here is mine and unrefereed. Literature statements are taken
from the librarian's report `../library/rotations.md` and carry its reading tags; I opened no source myself, so each is
[cited, unchecked] unless the librarian read the place named. Numerics are in the `*.py` files beside this one, with
their output in `out_*.txt`; nothing was timed.

## Answers to the five questions

**1. Does the law, or a usable version, hold for a structured random rotation, and with what constant?** No, for
every structured rotation asked about, and there is no constant to quote.

- The law holds for every codebook exactly when the rotation makes every orthonormal pair uniform on the Stiefel
  manifold (A1). A rotation with finitely many values, which every product of sign diagonals and Hadamard matrices
  is, leaves nothing random once the item's rotated block is known (A2).
- For the code as built, one round fails outright: an item with two equal non-zero coordinates has an error that is
  one of four fixed vectors, and a query along it sees the full Cauchy–Schwarz size, 16 sd at n = 256, with
  probability ½ per side (B1). Any item with few non-zero coordinates has its whole error on a few fixed coordinates
  (B0, B2).
- Two, three and four rounds fail too, on items and queries that are constant on a subgroup of coordinates: the
  probability that the error exceeds the block's certified half-width is at least 2⁻¹⁹ for two rounds, 2⁻⁴¹ for three
  and 2⁻⁵⁸ for four at n = 256, 2⁻²⁰ and 2⁻⁵⁷ for two and three rounds at n = 512, and 2⁻³² for two rounds at n = 1,024,
  against a budget of 2⁻⁶¹ (B3, B4). A direct run of two rounds with no conditioning shows errors above 7.8 sd at
  1.5·10⁻⁶. Five rounds at 256 and three at 1,024 are not reached by my construction, which is one family of inputs and
  proves nothing for them. At blocks of 64 and below the construction does not reach the constant at all; there the
  certified width is already close to Cauchy–Schwarz (C1).
- A random butterfly has an error that is a function of the rotated item alone on two adjacent basis vectors, for
  any codebook; its rms is 4 sd at n = 256 and it exceeds 7.8 sd on 20 to 37% of draws at n = 1,024 (D2, D4). A Kac
  walk has no theorem of the kind needed and a floor of 21·n steps from untouched coordinates (D1, D3).
- The rotations that do keep the law, with today's constant or the closed-form one 0.12 above it, are Haar measures on
  subgroups:
  smaller Haar blocks (F1, F2), and the unitary and symplectic groups (H4). Each trades stored numbers for stored
  scalars at the same rate (H1). A dither keeps an exact law with no rotation and a constant 1 to 5% larger (G1, G5).

**2. Blocks that are not powers of two.** The question only arises for Hadamard products, which fail anyway; they
would need padding (336 to 512; 688 and 768 to 1,024), so the code would spread its bits over more positions. Every
option that keeps the guarantee works at any block size: rounded copies (E1), Householder storage (H7), sub-blocks
(F1), the dither (G1). The unitary form needs an even size and the symplectic a multiple of four, which the blocks
(multiples of 16) satisfy (H4).

**3. Does the guarantee stay "for any data, over the build's rotation"?** With a structured rotation it cannot: the
counter-examples are particular data, and I found no assumption that repairs it. Incoherence of the item or of the
query separately is not enough, because the failing event concerns the pair and the signs together, and the one clean
conditional statement needs the item and the query to have disjoint supports (B5). Every option I recommend keeps
"for any data" with no assumption: E1, F1, H4 and G1 are all proved for every item and every query fixed independently
of the seed.

**4. Rotations held in f32.** The constant does not change, and the allowance is a deterministic term of at most
0.0024% of the random half-width at every block size and depth (E1, E2). The build must go on encoding against its
f64 matrix; the query uses the rounded copy and adds ρ·‖y‖·sec φ·‖Δ‖ per block, which fits the slot the kernel already
has for the f32 cast of the rotated weights. int16 costs under 0.3% up to 4 bits per position and 2.4 to 3.9% at 8
bits; int8 costs 8 to 13% at one bit on the large blocks and is unusable beyond. The product must be accumulated in
f64 or in exact integers, or in blocked f32 sums with their stated bound.

**5. What a larger constant costs.** Chapter 3 measured 10.8% more exact reads per unit of c, linearly between 7.4
and 9.3 [measured there, 50k dump]. A width factor acts as the same factor on c. So: a density ratio M would cost
ln M/c in c, 0.3 for M = 10 (about 3% more exact reads) if such a bound existed (A1, D3); a dither-aware choice among
six scales 0.23 (G3); the dither's constant 0.07 to 0.43 (G5a); sub-blocks with shared scalars at least 12% of the
width, 0.9 in c, about 10% more exact reads (F5); the dither's width, 2.0 to 2.65 times, is outside any range
measured and is better read as 1.0 to 1.4 bits per position lost (G7).

## Recommendation, ranked by what it saves against what it costs

| | Change | Saves (n = 2048: 9.93 MB read per group setup) | Costs | Rests on |
|---|---|---|---|---|
| 1 | Query-side rotations in f32, build unchanged | half the bytes (4.97 MB); the same n² multiplications | nothing measurable: allowance below 0.003% of the random term; one certified number per block | E1, E2 |
| 2 | Query-side rotations in int16, product in exact integers or f64 | three quarters of the bytes (2.48 MB) | under 0.3% of width to 4 bits per position, 2.4 to 3.9% at 8 bits; keep f32 for a block whose final depth is above about 6 | E1, E2 |
| 3 | Householder factors in place of the dense matrix, with 1 | a further half (2.5 MB in f32) | a sequential product of n − 1 reflections in place of one matrix product; law untouched; not timed | H7 |
| 4 | Sub-blocks with their own scalars (each block halved) | half the entries and half the multiplications, exactly | twice the κ, tan φ and norm scalars per item, twice the sums per item; no loss of width per bit of code | F1, F2, F5 |
| 5 | Unitary or symplectic Haar rotations | a half or three quarters of the bytes; the same multiplications | 10% of width (21% at the ninth decile), or a second scalar and a second sum per item; an encoder that nulls the extra term is untested | H4 |
| 6 | Sub-blocks with one κ per block | as 4 | 12 to 20% of width at two sub-blocks, more beyond and on uneven items | F3, F4, F5 |
| 7 | Shared dither, no rotation (with or without a flattening transform) | the whole rotation and 7 to 12 bytes of scalars per item | 2.0 to 2.65 times the width when nested from one bit; no better than Cauchy–Schwarz at sub-bit rates; constant 1 to 5% larger | G1 to G9 |
| 8 | Sign–Hadamard rounds, butterflies, Kac walks | nearly everything | the guarantee | A2, B1 to B4, D1 to D4 |

1 and 2 are the answer to the owner's question as asked: the bound is not re-derived for a cheaper rotation, it is
kept for the same rotation read at lower precision. If the setup time is the bytes read, as the owner says, 2 removes
three quarters of it. Nothing above was timed. Only 4 and 6 reduce the multiplications. The dither (7) is the one new
exact law here and the librarian found no prior statement of it; it does not suit this index, because its width is
set by each block's largest coordinate and because the first randomised level is below one bit per position.

What failed or was corrected along the way is recorded where it happened: the PI's condition in A is sufficient, not
necessary (A1); the counter-example in B needs e₀ in the query (B1); my first estimate for structured inputs under two
rounds was wrong (B4); an untouched coordinate under a Kac walk is not by itself a failed bound (D1); the shared form
of F is not cheap (F4); the PI's 1.3 to 1.45 for the dither is too low for a code nested from one bit (G7); and
whether an exact pair-uniform ensemble with o(n²) parameters exists is not settled (H6).

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

    E = (A/√2)·( d(0)·e₀ − d(a)·d(b)·d(s)·e_s ),   tan φ = v₀/v₁  (1, 1/3, 1/5 at depths 1, 2, 3),

with v₀ = 2^(−depth) the decoded value of a zero and v₁ the decoded value of the non-zero coordinates.

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
b, s are distinct. tan φ = A/‖y‖ = v₀/v₁; the non-zero value is √2/λ in grid units, 0.707 at λ = 2. ∎

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

The law's rms is 1 and its probability at 7.8 sd is 9·10⁻¹⁷ (two-sided, d = 255). One round therefore fails by many orders for any item with up to
about log₂ n non-zero coordinates in the block's basis, and its error spread is wrong (rms above 1) for every sparse
item.

### Two and more rounds

For a subgroup Γ of the index group, Γ^⊥ = {k : χ_k(g) = 1 for all g ∈ Γ}, of size n/|Γ|. A *signed coset indicator* is
a vector ±χ_u·1_W/√|W| with W = w + Γ a coset.

**B3a (a structured round keeps signed coset indicators).** [proved, unrefereed] Let a = χ_u·1_W/√|Γ|, W = w + Γ. If
the signs of D on W equal ±χ_c on W for some c, then H·D·a = ±χ_w·1_{(u xor c) + Γ^⊥}/√|Γ^⊥|. For |Γ| ≥ 2 exactly
2|Γ| of the 2^|Γ| sign patterns on W have this form, so the event has probability 2^(1 + log₂|Γ| − |Γ|).

*Proof.* (H·D·a)_k = ±(n|Γ|)^(−½)·Σ_{i∈W} χ_{u xor c xor k}(i) = ±(n|Γ|)^(−½)·χ_{u xor c xor k}(w)·|Γ|·1[u xor c xor k ∈ Γ^⊥],
and √(|Γ|/n) = 1/√|Γ^⊥|. The patterns ±χ_c restricted to W are ± the |Γ| characters of Γ times a constant; a character
and the negative of a character differ at 0, so there are 2|Γ| of them. ∎

**B3b (the last round is a t-dimensional problem measured against an n-dimensional spread).** [proved, unrefereed]
Let G be a subgroup with |G| = t ≥ 2, W = w + G, a = χ_u·1_W/√t and b = χ_{u′}·1_W/√t with χ_{u xor u′} not constant on
G (so a ⟂ b). Let z = H·D·a, code z as built at one depth, and let the error be ⟨e, H·D·b⟩. Then, for every draw of D,

    error / sd_n = √((n − 1)/(t − 1)) · X_t,

where sd_n = A/√(n − 1) is the spread the law claims and X_t is the standardised error (error over A/√(t − 1)) of
the *t-dimensional* one-round problem with item 1/√t, query χ_v/√t (v = u xor u′ restricted to G) and the same code;
its exact zeros count as coded upward when w ∈ G and contribute nothing to the error when w ∉ G. The law of X_t does
not depend on n.

*Proof.* Put ε_g = d(w + g)·χ_u(w + g), independent uniform signs, ζ = H_t·ε/√t and ξ = H_t·(ε·χ_v)/√t in ℝ^t (H_t the
t-dimensional transform on G's dual). Then z_k = χ_k(w)·√(t/n)·ζ(k̄) and (H·D·b)_k = ±χ_k(w)·√(t/n)·ξ(k̄), with k̄ the
class of k modulo G^⊥; each class has n/t members. The n-dimensional code of z at scale λ·rms_n is, coordinate by
coordinate and up to the sign χ_k(w), the t-dimensional code of ζ at scale λ·rms_t, except that an exact zero decodes
to +v₀ whatever χ_k(w) is. So ‖ō‖²_n = (n/t)‖ō_t‖², ⟨ō, z⟩_n = √(n/t)⟨ō_t, ζ⟩ (zeros contribute nothing), tan φ and
hence the chosen λ are the same in both problems, κ_n = √(t/n)·κ_t, |e_k| = √(t/n)·|e_t(k̄)| for every k, and A_n = A_t.
The error is Σ_k e_k·χ_k(w)·√(t/n)·ξ(k̄) = Σ_{k̄ : ζ ≠ 0} e_t(k̄)·ξ(k̄) + κ_t·v₀·Σ_{k̄ : ζ = 0} ξ(k̄)·(t/n)·Σ_{k ∈ k̄} χ_k(w). The inner
sum is n/t if w ∈ G and 0 otherwise, because χ_·(w) is constant on the cosets of G^⊥ exactly when w ∈ (G^⊥)^⊥ = G. So
the error equals the t-dimensional error, and dividing by A/√(n − 1) in place of A/√(t − 1) gives the factor. ∎

**B3 (fixed inputs that defeat k rounds).** [proved, unrefereed, given B3a and B3b; the conditional rates are
computed] Let G = {0, …, t − 1} and V = G^⊥, the multiples of t, |V| = n/t. A shared structured round takes two
signed indicators on one coset of G to two different cosets of V (B3a, since u xor u′ ∉ V), and costs probability
p_G = 2^(1 + log₂ t − t). A round structured on each of two cosets of V, with the second character in the one class
that brings both onto one coset of G, costs p_V = 2^(1 + log₂|V| − |V|)·2^(1 − |V|). Start on one coset of G
(y = 1_G/√t, Δ = χ₁·1_G/√t) when k is odd and on two cosets of V (y = 1_V/√|V|, Δ = 1_{V+1}/√|V|) when k is even, and
alternate so that the last round starts from one coset of G. Then for these fixed unit vectors

    Pr[ |error| > c·sd ]  ≥  p_V^⌊k/2⌋ · p_G^⌊(k−1)/2⌋ · Pr[ |X_t| > c·√((t − 1)/(n − 1)) ].

For k = 1 there is no conditioning: error/sd = √((n − 1)/(t − 1))·X_t on every draw.

The rates are in the table of B4. The mechanism is that sign–Hadamard rounds map the lattice of subgroup indicators to
itself with probability 2^(−O(t)), not 2^(−O(n)), and on that lattice a round acts as a rotation of t dimensions.

**B4 (the rates).** [computed, `b3_structured.py`, `out_b3.txt`: the built code at depth 1, c = 7.8; 200,000 draws per
row for n ≤ 256 and 40,000 above; the probability of the structure is exact and the rate given the structure is Monte
Carlo; `b2_rounds.py`, `out_b2b.txt` for the unconditional run]

One round, n = 256, y = 1_G/√t and Δ = χ₁·1_G/√t, no conditioning:

| t | 2 | 4 | 8 | 16 | 32 | 64 | 128 |
|---|---|---|---|---|---|---|---|
| mean square of error/sd (law: 1) | 255 | 42.4 | 36.4 | 16.8 | 8.25 | 4.05 | 2.01 |
| (n − 1)/(t − 1) | 255 | 85 | 36.4 | 17 | 8.23 | 4.05 | 2.01 |
| Pr[\|error\| > 7.8 sd] | 1 | 0.50 | 0.25 | 0.082 | 0.0021 | 1·10⁻⁴ | 0 in 200,000 |

The mean square is (n − 1)/(t − 1) times that of X_t, which is 1 to two digits from t = 8 up (0.5 at t = 4, where
half the draws have exact zeros).

Several rounds: the lower bound of B3 on Pr[|error| > c·sd] for the fixed pair, best t run, with c the block's own
certified constant, the one-coordinate sphere quantile at 2.5·10⁻¹⁹ per side (`b4_at_budget.py`, `out_b4.txt`). The
certificate fails where the entry is above the two-sided budget 5·10⁻¹⁹ = 2^(−60.8).

| n (its constant c) | k = 2 | k = 3 | k = 4 | k = 5 |
|---|---|---|---|---|
| 256 (8.282) | 2^(−19.2) (t = 64) | 2^(−41.2) (t = 16) | 2^(−58.4) (t = 32) | at most 2^(−74), the structure's own probability |
| 512 (8.589) | 2^(−20.5) (t = 64) | 2^(−57.0) (t = 32) | not run | |
| 1,024 (8.748) | 2^(−31.9) (t = 64) | at most 2^(−83) | | |

At the round figure 7.8 the same construction gives 2^(−18.4), 2^(−41.2), 2^(−58.1) at n = 256 and 2^(−30.4), 2^(−56.6) at
n = 512 (t = 32), where the law itself allows 2^(−53.2) and 2^(−49.9); so at 7.8 only two and three rounds at 256 and
two at 512 are violations, and the table above is the fair comparison.

- Two rounds fail by at least eight orders of magnitude at every size run. Run unconditionally (2·10⁶ draws, n = 256,
  y = 1 on coordinates 0 to 7, Δ = 1 on 8 to 15, no conditioning at all), the frequencies of |error| above 5, 6, 7 and
  7.8 sd were 1.07·10⁻⁴, 2.85·10⁻⁵, 8.5·10⁻⁶ and 1.5·10⁻⁶ (3 draws), against 3.2·10⁻⁷, 5.3·10⁻¹⁰, 1.9·10⁻¹³ and
  9.4·10⁻¹⁷ under the law; B3's bound for that pair is 2^(−21.2) = 4·10⁻⁷. The same run on y = e₁, Δ = e₂ and on a fixed
  Gaussian pair showed frequencies at 3 and 4 sd within sampling error of the law and nothing beyond 5 sd, so the
  failure belongs to structured inputs, not to typical ones.
- Three rounds fail at n = 256 (by six orders) and at n = 512 (by a factor of about 14). Four rounds fail at n = 256 by
  a factor of about 5. Five rounds at 256 and three at 1,024 are not reached by this construction, which proves
  nothing about them: it is one family of inputs, and no bound in the other direction exists (librarian's report,
  section 1: every proved closeness to Haar for these products is additive and of order n^(−½)).
- Small blocks. At n = 64 and n = 16 the same construction gives |error|/sd of at most 4.5 and 2.2 (at t = 16 and
  t = 4), below any constant in use, although the mean square is 3 to 5 at n = 64. There the certified width is
  already a large part of the Cauchy–Schwarz bound (√63 = 7.9 sd), so a wrong law has little room to show as a failed
  bound; C1 is the statement that applies to small blocks.
- Without conditioning on both sides, a structured item against a generic query fails less: with only the item's
  rounds structured (`out_b2c.txt`, n = 256, |V| = 16) the rate given the structure at 7.8 sd was 5·10⁻⁶ for k = 2 and
  nothing in 400,000 draws for k = 3 to 6. An earlier estimate of mine, that such a pair's spread is wrong by the
  factor 2^(r/2), was wrong; the error there is a product of two normalised sums with a tail like exp(−c).
- Exhaustively at n = 16, two rounds, y = e₁, Δ = e₂ (all 2¹⁶ sign vectors of the second round, `out_b2a.txt`):
  the mean square of error/sd is 0.986 and the law is a lattice law, with Pr[|error| > 2 sd] = 0.082 against 0.041
  for the sphere and nothing above 2.2 sd.

**B5 (no usable version under an assumption).** [sketch: gap named] One round has a clean conditional statement when
the supports of y and Δ are disjoint: then D on supp Δ is independent of z, the error is the Rademacher sum
Σ_{i ∈ supp Δ} d(i)·Δ_i·(H·e)_i given z, and Hoeffding gives Pr[error ≥ x | z] ≤ exp(−x²/(2‖Δ‖²·‖H·e‖²_∞)), with
‖H·e‖_∞ computable at the build and at least A/√n. Disjoint supports cannot be assumed of an item and a query in one
basis. For overlapping supports the signs that fix z also enter RΔ, and I found no decoupling. An assumption on the
item alone (incoherence, a bound on ‖y‖_∞/‖y‖) can be checked at build, and one on the query at query time, but it
does not restore the law. B3's inputs at n = 256 with t = |V| = 16 have every coordinate at most 4/√n times the norm,
close to the largest coordinate of a Gaussian block (3.0/√n on average, G7), and fail two rounds at 2^(−30) and three
at 2^(−41). Under one round the pair with t = n/2, whose coordinates are at most √2/√n, has a spread wrong by the
factor 2 in variance on every draw (B4, t = 128). The failing event is a property of the pair and the signs together.
*Gap:* a statement of the form "if the item's and the query's coordinates are each at
most μ/√n in the block's basis, then k rounds satisfy a tail bound with constant c(μ, k)" is neither proved nor
refuted here; the flattening lemma gives ‖HDx‖_∞ ≤ 0.88·‖x‖ at δ = 10⁻¹⁹ and n = 128 (librarian's arithmetic), which is
no assumption at all.

## C. The randomness count

**C1 (a budget below the seed's atom forces a deterministic bound, and a deterministic bound of this form is
Cauchy–Schwarz).** [proved, unrefereed] Let the rotation be a function of B fair bits, or take S equally likely values
(put B = log₂ S). (i) For fixed (y, Δ) every event has a probability that is a multiple of 2^(−B). So a half-width that
is δ-valid with δ < 2^(−B) never fails. (ii) Let the half-width be h = h(A, ‖Δ_⊥‖), a function of the stored error
scale and of the query's norm, as c·sd is. If it never fails for any fixed pair, then h(A, t) ≥ A·t for every value A
that occurs: it is at least the Cauchy–Schwarz bound, which is c = √d in units of sd.

*Proof.* (i) is counting. (ii) Take y and a seed value ω₀, with rotation R₀ and error vector E₀ = R₀ᵀe(R₀y) of norm A₀,
orthogonal to y. The vector Δ = t·E₀/A₀ is a fixed query. At ω = ω₀, an event of positive probability, the error is
A₀·t and the stored scale is A₀, so h(A₀, t) ≥ A₀·t. ∎

Consequences, for δ = 2⁻⁶².

- k rounds of sign–Hadamard on n positions use kn bits and take at most 2^(kn − k + 1) distinct values (D_i and D_{i+1}
  may be negated together). A width below Cauchy–Schwarz therefore needs kn − k + 1 ≥ 62: n ≥ 62 for one round, 32
  for two, 22 for three, and five rounds at n = 16. This is the PI's answer C, which is right, with the addition that
  "deterministic" means no better than Cauchy–Schwarz for a width of the present form. It is a necessary condition
  only; section B shows it is far from sufficient.
- The count applies to pairs, not only to blocks. Under one round the pair (Ry, RΔ) depends on the signs at
  supp y ∪ supp Δ only, so for any n the bound is deterministic on every pair supported on fewer than 62 coordinates.
- For blocks with d ≤ c² the present width c·sd is already at least Cauchy–Schwarz and nothing is lost by a
  deterministic bound; the exact one-block constant is always below √d because |U| ≤ 1.

**C2 (the same count for the present build).** [proved, unrefereed, as arithmetic; Low's theorem is cited from the
librarian's report, section 4, and stated there for unitary designs] The build draws every rotation from one 64-bit
seed, so S ≤ 2⁶⁴ and every per-event probability is a multiple of 2⁻⁶⁴ = 5.4·10⁻²⁰. The budget δ = ε/(N·L) =
2.5·10⁻¹⁹ at N = 10⁹ is 4.6 of these atoms: the guarantee as a statement about the seed says that at most four seeds
fail a given (item, level) event. C1 (i) is all that is needed for this; Low's Theorem 1.5 is the same counting for
designs. The Haar value of the tail transfers to the generator only as a model, as chapter 3 says. Two things follow
that the chapter does not say. At N ≥ 2³³ rows (8.6·10⁹, with L = 4 and ε = 10⁻⁹) the budget is below one atom, so
the per-event statement is then "no seed fails", which the Haar law cannot support; a build meant for more than about
4·10⁹ rows should draw its rotations from a seed of 128 bits or more. And the seed's width is a property of the
generator's state, not of how many Gaussians it emits: a dense matrix and a sign vector drawn from the same 64-bit
seed have the same atom.

## D. Kac walks and butterflies

**D1 (a random-pair walk needs at least (n/2)·ln(1/τ) steps to be within τ of uniform, in any sense that sees
atoms).** [proved, unrefereed] In Kac's walk each step rotates a uniformly chosen pair of coordinates. For a = e_i the
coordinate i is untouched by all of T steps with probability (1 − 2/n)^T ≥ exp(−2T/(n − 2)), and then Ra = e_i. So the
law of Ra has an atom of that mass, its total-variation distance from uniform is at least (1 − 2/n)^T, and it has no
bounded density for any finite T. For τ = 2.5·10⁻¹⁹ this needs T ≥ 21.4·n steps: 5,500 at n = 256 and 16,400 at
n = 768, against 65,536 and 589,824 multiplications for the dense product.

This is a floor, not a guarantee, and the atom is not by itself a failure of the built certificate (an untouched
1-sparse item is coded with a large stored tan φ, so its spread is large). What is known, from the librarian's report
(section 2): total-variation mixing on the sphere between ½·n·ln n and 200·n·ln n steps at the threshold ¼ (Pillai and
Smith) [abstract]; for the pair law, a Wasserstein contraction with an unstated constant (Pillai, Smith and
Vaikuntanathan) [paper]; a density-ratio (L^∞) cut-off only for rotations in uniformly random planes, which cost O(n)
each and Θ(n·ln n) steps, so Θ(n²·ln n) per vector (Hough and Jiang) [paper]. Every bound for an O(1)-per-step walk is
additive.

**D2 (a random butterfly fails on adjacent basis vectors, for any codebook).** [proved, unrefereed; verified
numerically, `d_butterfly.py`] Let R = B_{m−1}⋯B₀, where layer ℓ rotates each pair (i, i xor 2^ℓ) by its own
independent angle, n = 2^m. For y = e₀ and Δ = e₁ and any codebook,

    Δ·E = ( κ·⟨ō_odd, z_odd⟩ − ‖z_odd‖² ) / ( ‖z_odd‖·‖z_even‖ ),

a function of z alone, where "odd" and "even" are the positions of odd and even index.

*Proof.* B₀ sends e₀ to c·e₀ + s·e₁ and e₁ to −s·e₀ + c·e₁. Layers 1 to m − 1 never pair an even index with an odd one,
so R′ = B_{m−1}⋯B₁ maps e₀ to a unit vector p on the even positions and e₁ to a unit vector q on the odd positions.
z = c·p + s·q and RΔ = −s·p + c·q = −(s/c)·z_even + (c/s)·z_odd, with |c| = ‖z_even‖, |s| = ‖z_odd‖. Since e ⟂ z,
⟨e, z_even⟩ = −⟨e, z_odd⟩, so ⟨e, RΔ⟩ = ⟨e, z_odd⟩·(s/c + c/s) = ⟨e, z_odd⟩/(sc), and ⟨e, z_odd⟩ = κ⟨ō_odd, z_odd⟩ − s². ∎

The error is the mismatch between the code's gain on the two halves, which no single κ can remove. For the built
one-bit code with ‖p‖₁ ≈ ‖q‖₁ it is about (|c| − |s|)/(|c| + |s|) in units of ‖y‖·‖Δ‖, of order one on most draws;
the measured rates are in D4. The "simple" butterflies of Trogdon are Haar measure on a torus of dimension log₂ n
(librarian's report, section 3), so their orbit of a fixed y has dimension at most log₂ n.

**D3 (what a density-ratio bound would need).** [sketch: gap named] By A1 a bound needs, for every orthonormal (a, b),
a density of Rb given Ra at most M against the uniform law on the sphere of (Ra)^⊥; the cost is c² → c² + 2J·ln M for J
blocks. Three things stand in the way for a cheap network. (i) Dimension: the pair law lives on V₂, of dimension
2n − 3, so at least 2n − 3 angles must act on the pair; a one-pass butterfly has n − 1 angles on the path of a basis
vector and about 2n − 3 on a pair at most, and D2 shows a pair on which far fewer act. (ii) Atoms: a walk with random
pairs has atoms for sparse inputs at every T (D1), so the best form is "density at most M off an event of probability
τ", which gives Pr ≤ M·δ_Haar + τ and still needs T ≥ 21.4·n. (iii) No theorem: no L^∞ or relative-error bound is
known for the pair law of any O(1)-per-step walk or fixed network (librarian's report, section 2, "not done"). *Gap:*
(iii). Whether a deterministic schedule of Givens rotations with about C·n·(ln n + ln(1/δ)) angles has a pair law
with a bounded density from every start is open; D1's floor and the count of flops (six per Givens step, not
vectorised across a random pair) leave at most a factor of 2 to 6 below the dense product at n = 256 to 768 even if
C = 1.

**D4 (measured).** [computed, `d_butterfly.py`, `out_d.txt`, 20,000 draws each, y = e₀, Δ = e₁, the built code]

- Butterfly: the error equals D2's formula to 10⁻¹¹ on every draw. The rms of error/sd is 2.40 (depth 1) and 2.36
  (depth 2) at n = 64, and 3.68 and 4.06 at n = 256, where the law has 1; the median of |error|/sd at n = 256 is 3.3
  to 4.0, so about half the draws exceed 4 sd, an event of probability 5·10⁻⁵ under the law. No draw exceeded 7.8 sd at
  these two sizes, because the butterfly's image of a basis vector is so uneven that the stored tan φ is large. At
  n = 1,024 (4,000 draws, `out_d_1024.txt`) the rms is 5.75 and 6.96 and Pr[|error| > 7.8 sd] is 0.20 at depth 1 and
  0.37 at depth 2.
- Kac walk, n = 64, T = n, 2n, 4n, 8n steps: the share of draws with coordinate 0 untouched is 0.132, 0.017, 0.0003,
  0, matching exp(−2T/n). The mean square of error/sd is 0.98 to 1.00 and the largest |error|/sd is 2.8, 3.6, 4.6, 3.8;
  Pr[|error| > 4 sd] was 2·10⁻⁴ at T = 4n (4 draws, against 0.7 expected) and 0 elsewhere. At this sample size the
  walk shows no failure on this pair and nothing can be read about 10⁻¹⁹; the claim against the walk is D1's floor and
  the absence of a theorem, not a counter-example.

## E. A rounded copy of the rotation on the query side

**E1 (the allowance).** [proved, unrefereed] Let the build encode with R as now: z = R·y, ō = C(z), κ and tan φ from
(ō, z). Let the query hold any matrix R̃ and compute x̃, its floating-point value of R̃·Δ. Let ρ ≥ ‖R̃ − R‖₂ and
η ≥ ‖x̃ − R̃Δ‖/‖Δ‖. Then

    | κ⟨ō, x̃⟩ − κ⟨ō, RΔ⟩ |  ≤  ‖y‖·sec φ·(ρ + η)·‖Δ‖,

so if h is a valid half-width for the exact estimate, h + Σ_j (ρ_j + η_j)·‖y_j‖·sec φ_j·‖Δ_j‖ is a valid half-width
for the computed one, with the same constant, at the same δ, for any data. As a share of the random term c·sd_j of
block j the allowance is at most (ρ_j + η_j)·√d_j/(c·sin φ_j).

*Proof.* κ⟨ō, x̃ − RΔ⟩ is at most κ‖ō‖·(‖x̃ − R̃Δ‖ + ‖(R̃ − R)Δ‖), and κ‖ō‖ = ‖ŷ‖ = ‖y‖/cos φ. The exact estimate's
error is untouched, because ō, κ and tan φ are functions of R·y alone and Lemma 3.1 is applied to R. sec φ is computed
from the stored tan φ and ‖y‖, both rounded up, and the term is increasing in each. Share:
‖y‖ sec φ·ρ·‖Δ‖/(c·‖y‖ tan φ·‖Δ‖/√d). ∎

The PI's answer E is right. Notes on its use.

- *What is stored.* Query side: R̃_j with its scale, and one number ρ_j per block. Build side: R_j in f64 as today,
  regenerated from the seed; a running service that takes inserts keeps it in memory, but no query reads it.
- *What the build must not do.* Encode against R̃, or derive κ or tan φ from a product with R̃. The codeword would then
  be a function of R·y + (R̃ − R)·y, and R̃ − R is a function of R that is not carried by R·y, so the lemma's premise
  (ō a function of R·y alone) fails. E1 puts no condition on R̃ itself: any matrix within ρ of R is covered.
- *The centre.* The estimate pairs the code with R̃·x − α·(rotated centre). If the stored rotated centres stay computed
  with R, the allowance's norm is ‖x_j‖ and not ‖Δ_j‖; compute them with R̃ as well.
- *Where it goes in the code.* The kernel already carries a per-block allowance for the f32 cast of the rotated
  weights, `ewj = 4u·‖R_jΔ_j‖₁` (`cluster_q2w`), which is multiplied by κ. The same slot takes √n_j·ρ_j·‖Δ_j‖, which
  bounds Σ_p |ō_p|·|((R̃ − R)Δ)_p| since |ō_p| ≤ 1. That form is two to three times E1's (‖ō‖ is about 0.4·√n), and
  needs no new per-item arithmetic.
- *The build's own matrix.* R is itself a floating-point matrix with polar factor P*, and chapter 3 already carries
  ‖R − P*‖ ≤ ‖RᵀR − I‖_F as slack; ρ adds to it.
- *The product.* With R̃'s entries held in f32, int16 or int8 and the sum accumulated in f64,
  η ≤ γ_n·‖R̃‖_F ≤ γ_n·√n·(1 + ρ), γ_n = n·2⁻⁵³/(1 − n·2⁻⁵³) (the standard bound for a dot product of n terms). With
  integer entries and the query's weights in fixed point the product is exact in integers, as the basis product
  already is, and η = 0. Accumulating in f32 puts 2⁻²⁴ in place of 2⁻⁵³ and is not negligible (table below).
- *A certified operator norm.* ‖N‖₂ ≤ (tr (NᵀN)^p)^(1/2p) for every p, within a factor n^(1/2p) of the truth; p = 64 by
  six squarings of NᵀN at build is within 1.05 at n = 768. The Frobenius norm is the case p = 1 and is too large by
  about √n/2 for a rounding error with independent entries.

**E2 (the numbers).** [computed, `e_rounding.py`, `out_e.txt`: one Haar matrix per size; entries rounded to nearest;
the integer formats use one scale, the largest entry, which is 3.6 to 5.1 times 1/√n; ρ is the Schatten bound of order
128, within 1% of the singular-value figure in every case; floating point, not interval-verified]

The built code's rms tan φ at depths 1 to 8 is 0.754, 0.365, 0.194, 0.105, 0.056, 0.029, 0.015, 0.0076 (uniform
directions, n = 256). The allowance as a percentage of the random half-width c·sd, c = 7.8, at depths 1 / 2 / 4 / 8:

| Block | f32: ρ; share | int16: ρ; share | int8: ρ; share |
|---|---|---|---|
| 64 | 4.9·10⁻⁸; below 0.001% | 6.0·10⁻⁵; 0.010 / 0.018 / 0.059 / 0.81 | 1.56·10⁻²; 2.6 / 4.6 / 15 / 209 |
| 128 | 5.0·10⁻⁸; below 0.001% | 7.4·10⁻⁵; 0.018 / 0.031 / 0.10 / 1.4 | 1.87·10⁻²; 4.5 / 7.9 / 26 / 356 |
| 256 | 5.0·10⁻⁸; at most 0.0014% | 8.9·10⁻⁵; 0.030 / 0.054 / 0.18 / 2.4 | 2.31·10⁻²; 7.9 / 13.8 / 45 / 625 |
| 336 | 5.0·10⁻⁸; at most 0.0016% | 7.9·10⁻⁵; 0.031 / 0.054 / 0.18 / 2.5 | 2.07·10⁻²; about 8 / 14 / 47 / 640 |
| 512 | about 5·10⁻⁸; at most 0.002% | 7.8·10⁻⁵; 0.038 / 0.066 / 0.22 / 3.0 | 2.03·10⁻²; 9.8 / 17 / 56 / 775 |
| 688 | about 5·10⁻⁸; at most 0.0022% | 8.0·10⁻⁵; 0.045 / 0.079 / 0.26 / 3.6 | 2.04·10⁻²; 11.4 / 20 / 66 / 906 |
| 768 | about 5·10⁻⁸; at most 0.0024% | 8.3·10⁻⁵; 0.049 / 0.086 / 0.28 / 3.9 | 2.13·10⁻²; 12.6 / 22 / 72 / 997 |

- With the Frobenius norm in place of the certified operator norm the shares are 4 times larger at n = 64 and 14
  times at n = 768: int16 gives 0.67 / 1.2 / 3.9 / 54% at n = 768. The certified norm is worth computing.
- Clipping the int8 range to 2.5, 3 or 3.5 times 1/√n makes ρ larger, not smaller (0.10 to 0.17 at n = 256).
- The product's own error, accumulated in f64, is below 10⁻⁷% everywhere. Accumulated naively in f32 its worst-case
  bound γ_n·√n is 0.08 / 0.15 / 0.48 / 6.6% at n = 256 and 0.75 / 1.3 / 4.3 / 59% at n = 768. The kernel's blocked f32
  sums carry (n/64 + 8)·2⁻²⁴ per row in place of γ_n (the bound stated in `cluster_q2w`), which gives about 0.02% at
  depth 1 and 1.5% at depth 8 at n = 768. So: f64 or exact integer accumulation, or blocked f32 sums with that bound.
- On 200 random pairs with int8 at depth 2 the estimate moved by 2 to 5% of the allowance on average and at most 22%.
  The allowance is a worst case over queries, as a deterministic term must be.

Reading: f32 is free at every block size and depth. int16 costs under 0.3% of the random term up to 4 bits per
position and 2.4 to 3.9% at 8 bits. int8 costs 8 to 13% at one bit at the large blocks and is unusable from 4 bits.
Bytes at n = 2048 (blocks 256, 336, 688, 768; 1,241,600 entries): 9.93 MB in f64, 4.97 MB in f32, 2.48 MB in int16,
1.24 MB in int8; H7's Householder form halves each. The constant is unchanged in every case.

## F. Sub-block rotations

**F1 (the law).** [proved, unrefereed] Split a block into sub-blocks y = (y₁, …, y_k) of sizes b_i with independent
Haar rotations R_i, z_i = R_i·y_i. Let the code ō = (ō₁, …, ō_k) be any function of z = (z₁, …, z_k) jointly (one scale
for the block, as built, is allowed), and let the estimate be ŷ_i = κ_i·R_iᵀō_i for scalars κ_i that are functions
of z. Put c_i = ⟨ō_i, z_i⟩/‖y_i‖². Then, given z,

    Δ·E = Σ_i ε_i·⟨Δ_i, y_i⟩ + Σ_i A′_i·‖P_{y_i⊥}Δ_i‖·U_i,   ε_i = κ_i·c_i − 1,   A′_i = κ_i·‖ō_i‖·sin φ_i,

with the U_i independent, U_i one coordinate of a uniform unit vector of ℝ^(b_i − 1), and φ_i the angle between ō_i
and z_i.

*Proof.* H1 with G = O(b₁) × … × O(b_k): given z the rotations are independent and each is uniform on
{R : R·y_i = z_i}. R_iᵀō_i = c_i·y_i + (a vector of norm ‖ō_i‖·sin φ_i orthogonal to y_i, with uniform direction). ∎

**F2 (own scalars per sub-block: exact).** [proved, unrefereed] With κ_i = 1/c_i every ε_i = 0 and the level's error
is Σ_i a_i·U_i, a_i = ‖y_i‖·tan φ_i·‖P_{y_i⊥}Δ_i‖: today's block law with the sub-blocks in the role of blocks. The
constants of Theorems 3.18 to 3.21 apply with d_i = b_i − 1. The rotations shrink from Σ n_j² to Σ_j n_j·b entries.
The price is one κ, one tan φ per rise and one norm per *sub-block*, one sum per sub-block in the scan, and one query
norm ‖Δ_i‖ per sub-block and cluster.

The constant barely moves [computed, `f_constant.py`, `out_f_constant.txt`; one-sided; the script reproduces chapter
3's R4 value 8.815 and θ = 1.00231 at n = 768]. At n = 2048 and N = 10⁹, R4 gives 8.944 for today's four blocks, 8.952
with each block halved, 8.969 for 32 sub-blocks of 64 and 9.079 for 128 of 16 (8.179, 8.187, 8.202, 8.304 at
N = 2²⁰). The sphere quantile in Σ(d_i + 2) − 2 dimensions, which chapter 3 found to predict the exact worst case to
0.0006, is 8.830, 8.830, 8.831, 8.834. A grid certificate over the shares of eight or more blocks is not practical by
the present method, so a layout with many sub-blocks would run on R4, 0.11 to 0.14 above the exact value, as the
fallback does today.

**F3 (one κ for the block: a deterministic radial term appears).** [proved, unrefereed] With one κ = ‖y‖²/⟨ō, z⟩,
Σ_i ε_i·‖y_i‖² = 0 but the ε_i are not zero, and for a query known only through ‖Δ‖ the half-width

    h = ‖Δ‖·( B + c·max_i A′_i/√(b_i − 1) ),   B² = Σ_i ε_i²·‖y_i‖²,   A² = B² + Σ_i A′_i²

is valid, with c a constant valid for the k sub-blocks at every share. Both B and max_i A′_i/√(b_i − 1) are functions
of z, so one stored scalar per rise (their combination at the smallest c ever used) replaces tan φ.

*Proof.* |Σ ε_i⟨Δ_i, y_i⟩| ≤ Σ |ε_i|·‖y_i‖·‖Δ_i‖ ≤ B·‖Δ‖ by Cauchy–Schwarz over sub-blocks. The random part has
sd² = Σ A′_i²·‖Δ_i‖²/(b_i − 1) ≤ max_i A′_i²/(b_i − 1)·‖Δ‖², and Theorem 3.19 covers a spread rounded up. ∎

**F4 (how large the radial term is).** [model; measured values in the table below] For a code whose error is
unrelated to the item's direction inside a sub-block, ε_i ≈ tan φ·g_i/√b with g_i of unit size (for the one-bit sign
code on a uniform direction the variance of ε_i·√b is π/2 − 1 = tan²φ exactly to first order). So B ≈ A/√b, and
against the one-rotation half-width c·A·‖Δ‖/√(n − 1)

    radial term / random term ≈ √(n/b)/c = √k/c.

At c = 7.8 this is 0.18 for two sub-blocks, 0.26 for four, 0.44 for twelve and 0.73 for thirty-two. The radial terms
of different sub-blocks have unrelated signs, so their sum is typically √k smaller, but a certificate must take the
worst case, and a probabilistic bound on the sum gains nothing until k is well above c². The shared form therefore
loses more width than the PI's brief expected; it is not the cheap form of F.

**F5 (measured widths).** [computed, `f_subblocks.py`, `out_f.txt`: 400 items per row, the built code with one scale
per block, uniform directions per sub-block; "flat" has equal expected energy in every sub-block, "skew" has energy
falling by a factor 4 from the first sub-block to the last; c = 7.8] Half-width as a multiple of the one-rotation
half-width c·A·‖Δ‖/√(n − 1), at depths 1 / 2 / 4:

| Block, sub-blocks | F2, own scalars, flat | F3, one κ, flat | F3, flat, encoder equalising the gains (depths 2 / 4) | F3, one κ, skew |
|---|---|---|---|---|
| 256 = 2 × 128 | 1.00 | 1.12 / 1.16 / 1.20 | 1.06 / 1.09 | 1.73 / 1.51 / 1.22 |
| 256 = 4 × 64 | 1.00 | 1.26 / 1.37 / 1.43 | 1.19 / 1.24 | 1.66 / 1.67 / 1.48 |
| 256 = 8 × 32 | 1.00 | 1.44 / 1.62 / 1.70 | 1.42 / 1.50 | 1.71 / 1.82 / 1.77 |
| 768 = 2 × 384 | 1.00 | 1.12 / 1.18 / 1.21 | 1.05 / 1.09 | 2.30 / 1.82 / 1.33 |
| 768 = 3 × 256 | 1.00 | 1.19 / 1.27 / 1.34 | 1.08 / 1.16 | 2.16 / 1.99 / 1.54 |
| 768 = 6 × 128 | 1.00 | 1.32 / 1.48 / 1.61 | 1.19 / 1.32 | 2.06 / 2.05 / 1.78 |
| 768 = 12 × 64 | 1.00 | 1.50 / 1.74 / 1.98 | 1.39 / 1.61 | 2.09 / 2.18 / 2.10 |
| 688 = 2 × 344 | 1.00 | 1.12 / 1.17 / 1.20 | 1.05 / 1.09 | 2.23 / 1.77 / 1.31 |
| 688 = 4 × 172 | 1.00 | 1.24 / 1.35 / 1.44 | 1.12 / 1.22 | 2.04 / 1.97 / 1.63 |

- F2 keeps the width (0.99 to 1.00 flat; 0.88 to 0.99 on the skewed items, because each sub-block's own κ fits it
  better than the block's).
- F3 loses 12 to 20% with two sub-blocks, 19 to 34% with three, and more beyond; the radial term alone is 0.10 to
  0.17 with two sub-blocks, close to F4's √k/c scaled by the measured B·√b/A of 0.5 to 1.4. On items whose energy is
  uneven across sub-blocks the one scale mis-fits the weak sub-blocks and the loss is 30 to 130%. An interleaving
  permutation before the rotations would even the energy for typical data, and the bound is sound without it.
- "Equalising the gains" lets the encoder pick, per sub-block, a scale within ±15% of the block's so that its gain c_i
  is nearest the block's; the code remains a function of z, so F1 holds. It removes most of the radial term at one
  rise (0.01 to 0.07 left with two to four sub-blocks) but widens the max term slightly, and a block has several
  rises with one scale, so this is an upper bound on what it can recover. It is not possible at one bit, where the
  code is the sign and does not depend on the scale.

So the PI's answer F is right about the law and the storage and wrong about the cheap form: with shared scalars the
width grows by at least 12%, which in units of the constant is +0.9 or more (question 5). With own scalars nothing is
lost in width, but the scalars per item multiply by the number of sub-blocks per block.

## G. No rotation: a shared subtractive dither

Setting. Coordinates p = 1, …, n in any fixed basis (the group's basis, or that basis followed by any fixed orthogonal
map). The seed gives numbers τ_p, independent and uniform on [0, 1), the same for every item, independent of the items
and of the queries. Position p is coded at depth d_p^(ℓ) at level ℓ, non-decreasing in ℓ, fixed by the group's design;
d₁(p) ≥ 1 is the first non-zero depth of position p. Each item has a step s > 0, a function of the item alone. The
grid of depth d has 2^d cells of width w_d = 2s·2^(−d) on [−s, s). The dither is u_p = w_{d₁(p)}·(τ_p − ½), uniform
over one cell of the coarsest grid that position p ever uses.

- Encode: k_p is the index of the depth-D cell containing y_p + u_p (D the final depth).
- Decode at depth d: m_d is the midpoint of the depth-d cell with index ⌊k_p/2^(D−d)⌋, and ŷ_p = m_d − u_p.
- The estimate of Δ·y is s·( Σ_p Δ_p·μ_p − Σ_p Δ_p·u_p/s ), with μ_p = m_d/s ∈ [−1, 1): a sum of table lookups as
  now, times the item's step, minus a number that depends on the query and the seed only.

**G1 (the error law of the nested dithered code).** [proved, unrefereed] Suppose the item does not overload:
|y_p| ≤ s·(1 − 2^(−d₁(p))) for every p. Then for every item, at every level, the errors ε_p = ŷ_p − y_p of the coded
positions are independent, ε_p is uniform on an interval of length w_{d_p} centred at 0, and their joint law depends on the
item through s alone. Hence the level's error is

    Δ·E = s·Σ_p Δ_p·2^(−d_p)·V_p,   V_p independent and uniform on (−1, 1),   sd² = s²·Σ_p Δ_p²·4^(−d_p)/3,

for any data, in any basis, with no rotation. Errors at two levels are dependent; each level's law is exact.

*Proof.* y_p + u_p lies in [−s, s) because |u_p| ≤ w_{d₁}/2 = s·2^(−d₁) with u_p < w_{d₁}/2, so the cell index is never
clamped. Write (y_p + s + u_p)/w_d = x + K·τ_p with x fixed by the item and K = w_{d₁}/w_d = 2^(d − d₁), a positive
integer for every depth d ≥ d₁ at which p is coded. For τ uniform on [0, 1) the fractional part of x + K·τ is uniform
on [0, 1) whatever x is. The error is ε_p = m_d − (y_p + u_p) = w_d·(½ − frac), uniform on an interval of length w_d
centred at 0. ε_p is a function of the item and τ_p alone, the τ_p are independent, and s is a function of the item, so
given the item the ε_p are independent. ∎

Checked directly [computed, `g_check.py`, `out_g_check.txt`]: on 2²⁰ evenly spaced dither values the error of a
single position is uniform on its cell to 10⁻⁵ in Kolmogorov distance at every depth from d₁ to 5, for d₁ = 1, 2, 3
and values of y up to the limit s·(1 − 2^(−d₁)), with no clamped index; with overload (d₁ = 1, s = 1, depth 3) the mean
error is −0.025, −0.088, −0.188, −0.725 at y = 0.6, 0.8, 1.0, 1.6, as G2 (i) says; the example of G3 (ii) gives exactly
twice the fixed-step tail at four thresholds; and for 64 positions at mixed depths the level's error has variance
0.996 to 1.000 of s²·Σ Δ_p²·4^(−d_p)/3 and the same kurtosis (2.88 to 2.91) for a Gaussian item, an item with one
large coordinate and the zero item.

This is the dither theorem for a fixed input with a uniform dither over a whole number of cells (Schuchman's condition
holds for a dither uniform over K cells; hypotheses as in the librarian's report, section 6), together with the
remark, made independently in that report, that one offset serves every refinement of the grid. I know of no
published statement of it for a nested code; the librarian found none.

**G2 (overload, and the per-item step).** [proved, unrefereed] (i) If y_p + u_p leaves [−s, s) the index clamps and
ε_p = m − (y_p + u_p) is no longer uniform; for |y_p| ≥ s + w_{d₁}/2 every draw clamps and
ε_p = sign(y_p)·(s − w_d/2) − y_p − u_p, whose mean is minus the overshoot. The law then depends on the item, and no
constant covers it. (ii)
The step s = max_p |y_p|/(1 − 2^(−d₁(p))), rounded up on any grid, is a function of the item alone and never overloads;
a larger step is also valid, at a proportionally larger spread. (iii) Nesting has a price: a position first coded at
depth d₁ needs a range 1/(1 − 2^(−d₁)) times its largest value, 2 for d₁ = 1, 4/3 for d₁ = 2, where a code used at one
depth B only needs 1/(1 − 2^(−B)). The reason is that the dither must be uniform over the coarsest cell for the first
level's error to be uniform, and the sum y_p + u_p must stay on the grid.

**G3 (a step chosen with knowledge of the dither).** [proved, unrefereed] (i) If the encoder picks the step among K
candidates s₁, …, s_K, each a function of the item and none overloading, by any rule that may look at the dither (as
`rand_choose` picks its scale by trying six and keeping the best realised code), then for any half-width rule
h(s, Δ) that is δ′-valid for each fixed candidate, Pr[error ≥ h(s_chosen, Δ)] ≤ K·δ′. (ii) The factor cannot be
dropped: for one coordinate with value a > 0, first depth 1, read at depth 2, candidates s = 2a and s′ = 4a, and the
rule "keep the step whose error is the larger share of its cell", Pr[ε/w ≥ x] = 1 − 2x on [0, ½], exactly twice the
fixed-step value ½ − x. (iii) So a dither-aware choice among K steps costs c² → c² + 2·ln K (K = 6: c from 7.80 to
8.03, 2.9% wider), and a step chosen from the item alone costs nothing. The step must also be stored exactly as used,
on a grid; encoding with one step and storing another breaks the estimate itself.

*Proof.* (i) The failure event is contained in the union over k of {error with step s_k ≥ h(s_k, Δ)}. (ii) From G1's
proof, ε/w₂ = ½ − frac(α + 2τ) with α = (a/s + 1)·2 for the step s, and the same with α′ = (a/s′ + 1)·2 for s′. Here
α = 3 and α′ = 2.5, so with v = frac(2τ), uniform on [0, 1), the two normalised errors are ½ − v and ½ − frac(v + ½).
Their maximum is ½ − v for v < ½ and 1 − v for v ≥ ½, which is uniform on (0, ½]. Neither step overloads: a ≤ s/2. ∎

The built encoder's rule (keep the scale with the least tan φ) would tend to shrink errors, not enlarge them, but a
certificate cannot rest on that; it is a rule that looks at the dither, and (i) is what covers it.

**G4 (dither held in b bits).** [proved, unrefereed] If τ_p takes the values j/2^b equally often, then
Δ·E = (the law of G1) + r with |r| ≤ s·2^(1−b)·Σ_p |Δ_p|·2^(−d₁(p)) ≤ s·2^(−b)·‖Δ‖₁. With b = 32 this is below
√n·2^(−31+D) of the spread, 10⁻⁵ at n = 768 and D = 8; with b = 16 it is not negligible.

*Proof.* For a fixed item ε_p is uniform on 2^b/K equally spaced points of spacing h = w_d·K/2^b = w_{d₁}/2^b inside its
cell. Adding an independent uniform variable on an interval of length h makes it uniform on an interval of length
w_d whose centre is within h/2 of 0; re-centring moves it by at most h/2 more. So ε_p = ε°_p + r_p with ε°_p exactly
centred uniform, independent across p, and |r_p| ≤ h = w_{d₁}/2^b. ∎

**G5 (the constant).** [proved, unrefereed, for (i) to (iii); (iv) is cited; (v) is a conjecture with computed
support] Let X = Σ_p w_p·V_p with V_p independent uniform on (−1, 1), sd² = Σ w_p²/3.

(i) Pr[X ≥ c·sd] ≤ exp(−c²/2) for every weight vector: the sub-Gaussian value with the true variance. So
c = √(2·ln(1/δ)) is valid: 9.256 at N = 10⁹ and 8.482 at N = 2²⁰, the "sub-Gaussian" row of chapter 3's table.

(ii) Chernoff's bound with the exact moment generating function is largest, among weight vectors on m coordinates
with a given sd, at equal weights: sup_w min_λ [−λx + Σ_p g(λ²w_p²)] = min_λ [−λx + m·g(λ²·3sd²/m)], g(v) = ln(sinh √v/√v).

(iii) sup over weights of the exact δ-quantile, in units of sd, is at least the Gaussian quantile z_δ (8.912 at
N = 10⁹; equal weights on m coordinates as m → ∞), and for weights on m coordinates it is at most √(3m), since
|X| ≤ Σ|w_p| ≤ √(3m)·sd. So a query whose weight sits on fewer than c²/3 coordinates (20 at c = 7.8) can never fail.

(iv) Since V = Σ_{i≥1} 2^(−i)·r_i with independent signs r_i, X is a Rademacher sum, and the bound of Bentkus and
Dzindzalieta, Pr[X ≥ x·sd] ≤ 3.178·Pr[G ≥ x], applies [cited, unchecked: as recalled in the librarian's report,
section 1; the passage to an infinite sum is by limits]. It gives c = z_{δ/3.178}: 9.041 at N = 10⁹.

(v) [conjecture] For δ ≤ 10⁻³ the worst case over weights on m coordinates is equal weights, and its quantile
increases with m to z_δ. Computed support is in the table below.

*Proof.* (i) E exp(λV) = sinh λ/λ = Σ_i λ^(2i)/(2i + 1)! ≤ Σ_i (λ²/6)^i/i! = exp(λ²/6), because (2i + 1)! ≥ 6^i·i!
(induction: (2i + 3)(2i + 2) ≥ 6(i + 1)). Chernoff. (ii) g is concave on v > 0: with t = √v, g′(v) = (t·coth t − 1)/(2t²),
and t ↦ (t·coth t − 1)/t² is decreasing, because t·coth t − 1 = Σ_{j≥1} 2t²/(t² + π²j²) (the partial fractions of
coth), a sum whose terms divided by t² each decrease. For fixed λ, Σ_p g(λ²w_p²) with Σ w_p²
fixed is then largest at equal w_p² by Jensen, and a supremum of minima is at most the minimum of suprema; equality
holds because the equal-weight vector attains it. (iii) The central limit theorem gives convergence of the quantile at
a fixed δ; the bound is the range. ∎

**G5a (the constants).** [computed, `g_dither.py`, `out_g.txt`; one-sided, in units of sd; the sphere column
reproduces chapter 3's 7.373 and 6.931 at d = 95]

| | N = 10⁹ | N = 2²⁰ | N = 2¹⁶ |
|---|---|---|---|
| sphere, one coordinate, d = 255 / 767 / 2046 (today's law) | 8.282 / 8.694 / 8.830 | 7.639 / 7.953 / 8.055 | 7.354 / 7.630 / 7.719 |
| uniforms, equal weights on m = 64 / 256 / 768 / 2048, exact quantile | 8.354 / 8.778 / 8.868 / 8.896 | 7.702 / 8.016 / 8.084 / 8.104 | 7.412 / 7.686 / 7.744 / 7.763 |
| Gaussian quantile z_δ | 8.912 | 8.117 | 7.774 |
| Gaussian at δ/3.178 (iv) | 9.040 | 8.256 | 7.919 |
| Chernoff, exact mgf, equal weights on 256 / 768 / 2048 (ii) | 9.099 / 9.204 / 9.236 | 8.361 / 8.442 / 8.467 | 8.042 / 8.113 / 8.135 |
| sub-Gaussian (i) | 9.256 | 8.482 | 8.149 |

The exact quantiles for m ≤ 256 are from the Irwin–Hall formula in 300-digit arithmetic and agree with the
Lugannani–Rice saddlepoint value to three decimals; m = 768 and 2,048 are saddlepoint only. For (v), at N = 10⁹ on 256
coordinates: equal weights 8.778; equal on half 8.640; two levels 1 and ½ 8.730; 1 and 0.2 8.662; 32 at 1 and 224 at
0.3 8.511; half-normal weights 8.554; geometric with ratio 0.98 8.253 (saddlepoint). Equal weights were the largest.

Reading. What is proved for the dither is 9.256, or 9.040 on the cited inequality; the true worst case is at least
8.896 on 2,048 coordinates and I conjecture it equals that. Against today's law in the same total dimension (8.830)
the proved constant is 4.8% larger, the cited one 2.4%, the conjectured one 0.7%. A uniform sum is closer to Gaussian
than a sphere coordinate of the same dimension (excess kurtosis −1.2/m against −6/(d + 2)), so the dither loses the
part of today's constant that comes from the sphere's lighter tail, which matters most for one small block (8.778
against 8.282 at 256).

**G6 (stored scalars and the query's work).** [proved, unrefereed, as a count against SYSTEM.md section 2.1] Per item
and block the dithered code stores one number, the step s_j, rounded up on a grid (8 bits as ‖y_j‖ has now). It needs
no κ and no tan φ: the spread of a coded block is s_j·(Σ_p Δ_p²·4^(−d_p)/3)^½, known from the step, the depths and the
query. Today's code stores ‖y_j‖ once and κ (4 or 8 bits) and tan φ (4 bits) at each rise. A block not yet coded
still needs ‖y_j‖ for its Cauchy–Schwarz term, so the dithered code stores s_j and ‖y_j‖ per block, 8 bytes for four
blocks, against 4 bytes plus 1 to 1.5 bytes per rise now: with eleven rises over four levels (the 1M layout's coded
blocks, 1 + 3 + 3 + 4, if every coded block rises at each level), 8 bytes against 15 to 20.5 of the 224 bytes the
randomised levels add. The query needs, per cluster and level, the
weighted norm Σ_p Δ_p²·4^(−d_p) per block in place of ‖Δ_j‖, and the one number Σ_p Δ_p·u_p/s per block; it needs no
rotation at all.

**G7 (the width against today's code).** [computed, `g_dither.py`: 300 uniform directions per block size for the built
code; the dither's spread is exact from G1] Per unit ‖Δ‖, today's spread is rms·tan φ·√(n/(n − 1)) and the dither's is
w_B/√12 = (2/(1 − 2^(−d₁)))·2^(−B)·y_max/√12, so the ratio is a coefficient times the block's crest factor
χ = y_max/rms. The crest factor of a rotated (Gaussian-like) block is 2.11, 2.36, 2.61, 2.85, 3.04, 3.36 on average at
n = 16, 32, 64, 128, 256, 768 (tenth to ninetieth percentile 2.64 to 3.45 at 256).

| Bits per position B | 1 | 2 | 3 | 4 |
|---|---|---|---|---|
| built tan φ (n = 256, one rise) | 0.753 | 0.366 | 0.195 | 0.105 |
| coefficient, nested from one bit (d₁ = 1) | 0.766 | 0.788 | 0.741 | 0.687 |
| ratio of spreads at n = 64 / 256 / 768, d₁ = 1 | 1.99 / 2.33 / 2.57 | 2.08 / 2.39 / 2.65 | 2.01 / 2.25 / 2.47 | 1.95 / 2.09 / 2.26 |
| the same, first coded depth 2 | | 1.39 / 1.60 / 1.77 | 1.34 / 1.50 / 1.65 | 1.30 / 1.39 / 1.51 |
| the same, not nested (d₁ = B) | 1.99 / 2.33 / 2.57 | 1.39 / 1.60 / 1.77 | 1.15 / 1.29 / 1.41 | 1.04 / 1.11 / 1.21 |

The half-width ratio is this times the ratio of constants, 1.01 to 1.12 by G5a (conjectured worst case against the
whole index's dimension, up to the sub-Gaussian value against one block of 256). So the PI's rough 1.3 to 1.45 holds
only for a block first coded at two bits or more, or for a code used at one depth of 2 to 3 bits; a code nested from
one bit is 2.0 to 2.65 times wider at every depth, which is 1.0 to 1.4 bits per position, and it is widest for the
largest blocks. The two causes are separate: covering the largest coordinate (the crest factor, where today's code
clips at 2 to 4.2 rms and lets κ absorb the clipping), and the doubling of the range that nesting from one bit needs
(G2 iii).

Two variants do not rescue it. (a) Clipping at t·rms and bounding what is clipped by Cauchy–Schwarz with a stored
norm: the best t gives half-width ratios 1.29 / 1.72 / 2.02 at one bit and 1.95 / 2.09 / 2.26 at four bits (n = 64 /
256 / 768, c = 7.8); the clipped part's bound grows as √n against the random term and only small blocks gain. (b)
Sub-bit rates: an uncoded position's error is the coordinate itself, a deterministic term. With half the positions at
one bit and a query spread evenly, the dithered half-width is 1.47, 1.07 and 0.86 times ‖y‖·‖Δ‖ at n = 64, 256, 768,
where today's sketch gives 1.47, 0.72 and 0.41 (the first is above Cauchy–Schwarz for either code). At sub-bit rates
the dither is no better than not coding the block; the rotation is what makes a sub-bit sketch work (Proposition 3.4).

**G8 (a flattening transform).** [proved, unrefereed, for soundness; the size is G7's] Any fixed orthogonal map
applied to items and queries before the dither leaves G1 intact, since G1 holds in any basis. A cheap one (sign–Hadamard
rounds, O(n log n), nothing stored) makes typical items Gaussian-like, with the crest factors of G7, and protects
against items that are spiky in the group's basis, whose crest factor can reach √n. It cannot go below the Gaussian
crest factor of about √(2 ln n): a rotation makes coordinates Gaussian, not flat. So it recovers nothing against the
table of G7, which already assumes a Gaussian-like block; it only keeps real data from being worse. This is the route
the librarian's report proposes (section 6, last remark): the guarantee from the dither, the rotation for rate only.
It is sound and exact for any data, and it costs the widths of G7. Flat representations with crest factor near 1 need
a redundant frame (Lyubarskii and Vershynin's Kashin representations) [cited, unchecked], which at least doubles the
positions to code and so halves the bits per position.

**G9 (dither with the present rescaled code).** [proved, unrefereed, for the identity; the conclusion is a judgement]
Use the dither in place of the rotation and keep ŷ = κ·(m − u), κ = ‖y‖²/⟨m − u, y⟩. Without overload m − u = y + ε
with ε as in G1, and then Δ·E = κ·⟨ε, Δ_⊥⟩ with κ = 1/(1 + ⟨ε, y⟩/‖y‖²). The stored κ is a function of ⟨ε, y⟩, and for
uniform (unlike Gaussian) ε the sum ⟨ε, Δ_⊥⟩ is uncorrelated with ⟨ε, y⟩ but not independent of it, so given the stored
scalar the error is not a centred uniform sum and G5 does not apply as it stands. Rescaling also buys nothing here,
because the dithered estimate is already unbiased. With overload (the present code's clipping) the error depends on
the item and G1 fails outright. So the two do not combine: the present code's advantages, a clipped grid and a scale
chosen by trial, are exactly what the dither forbids (G2, G3), and they are sound today only because the rotation
makes the error's direction uniform whatever the code does.

**G10 (where a better result might be).** [conjecture] A dither over a lattice cell in 8 to 24 dimensions with the
range set by the block norm of each group of positions, in place of the largest single coordinate, would replace the
crest factor of n scalars by that of n/8 or n/24 group norms, about 1.5 where the scalar code has 3; the dither
theorem holds for lattices (Zamir and Feder) [cited, unchecked]. Whether such a code is table-decodable at these rates
is the codes track's question (`../codes/REPORT.md` closed E8 as a drop-in for other reasons). Not worked out.

## H. Is there an exact ensemble with fewer parameters?

**H1 (the law for Haar measure on any closed subgroup).** [proved, unrefereed] Let G be a closed subgroup of O(n), R
Haar on G, z = Ry, and ŷ = Rᵀ·g(z) for any measurable decoder g (the rescaled estimator is g(z) = κ(z)·ō(z)). Let
G_y = {O ∈ G : Oy = y}. Then, given z, the law of E = ŷ − y is invariant under every O ∈ G_y.

*Proof.* For O ∈ G_y, R·Oᵀ is Haar on G and (R·Oᵀ)·y = R·y = z, so the decoder's output is the same and the estimate is
O·Rᵀ·g(z) = O·ŷ; its error is O·ŷ − y = O·E. ∎

Read off the orbits of G_y on ℝⁿ: on a subspace F where G_y acts trivially the error is a deterministic function of
z; on a subspace W where G_y acts transitively on the unit sphere, the component of E is its norm times a uniform
direction, independent of everything G_y fixes; and factors of G_y acting on separate subspaces give independent
directions.

| G | stored numbers | G_y fixes | random part | exact with |
|---|---|---|---|---|
| O(n) (today) | n² | span(y) | one sphere, d = n − 1 | one κ (Lemma 3.1) |
| O(b₁) × … × O(b_k), sub-blocks | Σ b_i² | span(y₁, …, y_k) | k independent spheres, d_i = b_i − 1 | k scalars (F1) |
| U(n/2) | n²/2 | span(y, Iy) | one sphere, d = n − 2 | a complex κ (H4) |
| Sp(n/4) | n²/4 | span(y, Iy, Jy, Ky) | one sphere, d = n − 4 | a quaternion κ (H4) |
| O(b) ⊗ 1_k (one b × b rotation on k sub-blocks) | b² | col(Y) ⊗ ℝ^k, dimension k·min(b, k) | not a sphere: O(b − k) acting on k copies | a k × k matrix in place of κ |
| O(b) ⊗ O(k) (Kronecker) | b² + k² | everything (G_y is finite for generic y) | nothing | no |
| any finite group (sign–Hadamard products, permutations) | — | everything | nothing | no (A2) |

The rows show one trade: a rotation stored in n²/s numbers needs about s scalars and s sums per item and block in
place of one, or else leaves s − 1 deterministic terms. No row escapes it.

**H2 (pair-uniformity does not force Haar).** [proved, unrefereed] (i) Haar measure on SO(n), n ≥ 3, is uniform on
pairs and is not Haar measure on O(n). (ii) For n ≥ 4 and 0 < |ε| ≤ 1/C(n, 3), the law with density 1 + ε·e₃(R)
against Haar measure on O(n), e₃(R) the sum of the principal 3 × 3 minors of R, is uniform on pairs, is not Haar, and
is not Haar measure on any subgroup. (iii) In general μ is uniform on pairs if and only if ∫π(R) dμ(R) = 0 for every
non-trivial irreducible representation π of O(n) that has a non-zero vector fixed by O(n − 2).

*Proof.* (iii) Fix an orthonormal pair p₀ with stabiliser K ≅ O(n − 2); V₂ = O(n)/K. By Peter–Weyl the functions
x ↦ ⟨π(x)v, w⟩ on V₂, with π irreducible, v fixed by K and w arbitrary, span a dense subspace of C(V₂). For a pair
p = g·p₀, ∫⟨π(R·g)v, w⟩ dμ(R) = ⟨π(μ)·π(g)v, w⟩ with π(μ) = ∫π(R) dμ(R). Uniformity of R·p for every p says this is
zero for all g, w and all non-trivial π with v ≠ 0; the vectors π(g)v span the representation, so π(μ) = 0. The
converse reads the same lines backwards. (ii) e₃ is the character of Λ³ℝⁿ, which is irreducible and has no vector
fixed by O(n − 2): Λ³(ℝ² ⊕ ℝ^(n−2)) = ⊕_{i ≤ 2} Λ^i ℝ² ⊗ Λ^(3−i) ℝ^(n−2), and Λ^j ℝ^m with 1 ≤ j ≤ m is a non-trivial
irreducible representation of O(m). |e₃| ≤ C(n, 3), so the density is non-negative. By Schur orthogonality
∫π(R)·e₃(R) dR = 0 for every irreducible π not isomorphic to Λ³ℝⁿ, so π(μ) = 0 for every π in (iii); and ∫e₃ dμ = ε ≠ 0,
so μ is not Haar. (i) is the case of the character det, the density 1 + det R. ∎

So the PI's guess is right that exactness on pairs is weaker than Haar. The examples save nothing: both have full
support.

**H3 (a floor on the number of parameters).** [proved, unrefereed] If μ is uniform on pairs, its support is not
contained in a countable union of smooth images of manifolds of dimension below 2n − 3, because the map R ↦ (Ra, Rb)
would send it to a null set of V₂. So at least 2n − 3 real parameters are needed, against n(n − 1)/2 for Haar.

**H4 (the complex and quaternionic forms of Lemma 3.1).** [proved, unrefereed; the complex form checked numerically
in `h_unitary.py`] Let n = 2m, identify ℝⁿ with ℂ^m, and let I be multiplication by i. Let R be Haar on U(m), ō any
function of z = Ry, κ_ℂ = ‖y‖²/⟨ō, z⟩_ℂ a complex number (the Hermitian product), and ŷ = κ_ℂ·R*ō. Then E = ŷ − y is
orthogonal to y and to Iy, and E = A·u with u uniform on the unit sphere of span(y, Iy)^⊥, a real space of dimension
n − 2, independent of A; so Δ·E = A·‖P_{span(y,Iy)^⊥}Δ‖·U with U a coordinate of a uniform unit vector of ℝ^(n−2). The
same holds for Sp(n/4) with a quaternion κ and n − 4. With the present real κ instead, the error is a random term of
the same form, ‖P_{span(z,Iz)^⊥}e‖·‖P_{span(y,Iy)^⊥}Δ‖·U with the first norm at most A, plus the deterministic term
−β·⟨IΔ, y⟩, β = ⟨e, Iz⟩/‖y‖², |β| ≤ tan φ.

*Proof.* H1 with G = U(m): the stabiliser of y is U(m − 1) acting on the complex orthogonal complement of y, and it is
transitive on that space's unit sphere. ⟨ŷ, y⟩_ℂ = κ_ℂ·⟨ō, z⟩_ℂ = ‖y‖² makes E complex-orthogonal to y, which is real
orthogonality to y and Iy. For the real κ: R commutes with I, so ⟨Ra, I·Rb⟩ = ⟨a, Ib⟩, and
RΔ = (Δ·y/‖y‖²)z + (⟨Δ, Iy⟩/‖y‖²)·Iz + ρ·v with v uniform on the sphere of span(z, Iz)^⊥; pair with e ⟂ z. ∎

This halves (or quarters) what is stored and read; the multiplications are the same n², because a complex product is
four real ones. It needs either a second stored scalar and a second sum per item and block (the scan's work doubles),
or the deterministic term kept as an allowance |β|·‖y‖·‖Δ‖. For a code whose error is unrelated to Iz, β is about
tan φ·g/√n with g of unit size, so the allowance is about |g|/c of the random half-width [model]. Measured
[computed, `h_unitary.py`, `out_h.txt`: n = 64, 20,000 Haar unitary matrices per pair, the built code on the 64 real
coordinates, three fixed pairs including y = e₀ + e₁]: with the complex scale the standardised error has mean square
1.012 to 1.014 and kurtosis 2.91 to 2.97 against the law's 1 and 2.906; with the real scale the allowance is 10% of
c·sd on average and 21% at the ninth decile, at c = 7.8. An encoder could make β nearly zero by moving one or two
coordinates by one cell, which I have not tested.

**H5 (groups).** [cited, unchecked: the librarian's recollection of the classification of Montgomery–Samelson, Borel
and Onishchik, report section 5] The only proper connected subgroups of SO(n) transitive on V₂(ℝⁿ) are G₂ (n = 7) and
Spin(7) (n = 8). With H1 this says: among Haar measures on subgroups, exactness with one real scalar is O(n) or SO(n)
apart from those two cases, and the rows of H1's table are what is left.

**H6 (not settled).** [conjecture] No ensemble with o(n²) parameters is uniform on pairs for every pair. I have neither
a construction nor a proof. H3's floor is 2n − 3; H1's table and H5 rule out subgroups; a non-group ensemble must
annihilate every representation with at most two rows of its highest weight [cited, unchecked, for that description
of the representations in (iii)] while being supported on a thin set. Exact designs match polynomial moments only and
the code is not a polynomial (librarian's report, section 4).

**H7 (the same Haar matrix in half the memory).** [proved, unrefereed, as a count] A Haar matrix is the product of
n − 1 Householder reflections with vectors of lengths n, n − 1, …, 2 (Stewart; Mezzadri) [paper, per the librarian].
Storing the vectors takes n²/2 numbers and applying them about 2n² flops, against n² numbers and 2n² flops for the
dense matrix. The law is untouched, since it is the same matrix up to rounding (covered by E1): `haar_rotation`
already builds its matrix as this product, times the sign diagonal of R that makes it Haar. It halves the bytes read
and makes the product sequential: n − 1 dependent steps in place of one matrix product. With the factors rounded to
f32, the rounding errors of the reflections add along the product, so ρ must be measured on the product at build; it
is of order √n·2⁻²⁴ if they add as a random walk and at most n·2⁻²³, against 5·10⁻⁸ for the dense f32 matrix [model,
not computed].
