# Confidence track, design consequences: claims C3 and C4

Tags are those of `notes/README.md`, symbols those of `notes/00-notation.md`. Nothing here is refereed. Scripts are in
this directory and were run as `nice -n 10 taskset -c 4 env OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 <script>`.
The log of how each claim was reached is `DESIGN-NOTEBOOK.md`.

## 0. The model, and local symbols

Everything below is derived in one model, and every number carries the tag [model] unless it says otherwise.

- **Coordinates.** The eigen-coordinates of `floors/CLAIMS.md` section 0: Δ·r = Σᵢ aᵢ gᵢ zᵢ with aᵢ = √γᵢ the
  **amplitude** of coordinate i, g and z independent standard Gaussians. T is a set of coordinates, n_T their number,
  d_T = n_T − 1, F_T = Σ_{i∈T} aᵢ.
- **Deterministic certificate of T** with H nats: mean half-width at least W_T(H) = Σ_{i∈T} min(aᵢ, λ), with the
  water level λ fixed by Σ_{i∈T} ln⁺(aᵢ/λ) = H (F1 of the floors track, [proved, unrefereed] there, with a referee).
  k_eff(λ) = W_T/λ is the number of coordinates the water has reached, counting a coordinate below the water by its
  share aᵢ/λ. ω_T(H) = W_T(H)/F_T.
- **Rotated block on T** at R bits per position (n_T·R bits). The block is weighted before the rotation, each
  coordinate by (sd of the item side / sd of the query side)^(−½), which minimises E‖y‖²·E‖Δ‖² and makes it F_T²
  (Cauchy–Schwarz, equality at these weights). I write ‖y‖‖Δ‖ = F_T, neglecting the concentration of the two norms
  (cv² about 1/n_eff). By Lemma 3.1 the half-width is c·F_T·tan φ/√d_T, and tan²φ ≥ t(R) = 1/(4^R − 1) (the cap
  bound of `codes/REPORT.md`). At R = 0 the block is certified by Cauchy–Schwarz at F_T = W_T(0).
- **A prefix.** Bits spent on single coordinates before the rotation shrink the amplitudes: bᵢ nats on coordinate i
  leave aᵢ·e^(−bᵢ) at best (Shannon's bound for a Gaussian coordinate, the same exchange rate as in W). The best
  prefix of H nats on T leaves the amplitudes min(aᵢ, λ), whose sum is W_T(H).
- **A layout** is a set D certified deterministically and blocks T_1, …, T_J, with prefix nats on any of them. Its
  half-width is

      h = W_D(H_D) + c·√( Σ_j W_j(H_j)²·t(R_j)/d_j ),    bits = (H_D + Σ_j H_j)/ln 2 + Σ_j n_j R_j.           (0.1)

  A block left at R_j = 0 belongs to D. c is taken as given (8 for exact answers, 4 for the approximate multiplier);
  its dependence on the layout (6.93 for one block of 96, 7.96 for four blocks, at N = 2²⁰) is ignored.
- **"Built" curves.** The built block code is ρ_c(R) times the sphere bound in tan φ, one rate: 1.50 at ¼ bit, 1.305
  at 1, 1.41 at 2, 1.51 at 3, 1.60 at 4, 1.67 at 6, 1.69 at 7 [measured, `codes/out_built.txt`], times 1.0245 for the
  stored scale [measured, `reports/bounds3/scale_check.log`]. The built deterministic code is 1.19 times W on real
  residuals with every stored bit counted [measured, `quant/REPORT.md`, 1.17 to 1.21]. These are in `design_lib.py`.
- **What the model leaves out.** Real residuals and offsets are not Gaussian; the spectra are the PCA-basis spectra of
  `agent1/spec_v1.json` (16 groups of a 200k subset, offsets of pseudo-queries against their four nearest centres),
  not the 1M index's dual basis; W is a floor, reached within 1.03 to 1.6 by the codes tested in the floors track.

---

## C3 (a). When bits on a rotated block beat the same bits spent deterministically

### C3a.1 Against the block's own Cauchy–Schwarz bound [model; one line from Lemma 3.1 and the cap bound]

A block of n positions coded at R bits per position has a randomised half-width below its zero-bit bound F_T exactly
when c²·tan²φ < d. With the sphere bound this is

    R > ½·log₂(1 + c²/d),     that is  B₀ = (n/2)·log₂(1 + c²/d) bits,     B₀ ↑ c²/(2 ln 2) = 0.721·c² as d → ∞.

With the sign sketch of Proposition 3.4 (tan²φ = πn/(2k′) − 1) it is k′ > (π/2)·c²·n/(c² + d), which tends to c²π/2.

**The PI's estimate is the sign sketch's limit for a long block, against the zero-bit bound.** It is right for that
code and that comparison: 100.5 bits at c = 8. It is not the price of the randomised bound. The sphere bound needs
2.18 times fewer bits (π·ln 2), and a block of the built sizes needs fewer again:

| c | n | B₀ ideal | sign bits | B₀ built code |
|---|---|---|---|---|
| 8 | 96 | 35.7 | 60.7 | 61 |
| 8 | 128 | 37.7 | 67.4 | 68 |
| 8 | 256 | 41.4 | 80.7 | 84 |
| 8 | 288 | 41.8 | 82.5 | 87 |
| 8 | 768 | 44.4 | 92.9 | 100 |
| 8 | ∞ | 46.2 | 100.5 | |
| 4 | 96 to 768 | 10.8 to 11.4 | 21.7 to 24.7 | 24 to 27 |

[computed, `c3a.py`, `c3a.log`.] A block of fewer than (π/2 − 1)·c² + 1 positions (38 at c = 8) cannot be brought
below its Cauchy–Schwarz bound by one sign per position. The built code at sub-bit rates is 1.50 times the bound, so
its B₀ is 1.7 to 2.3 times the ideal one.

### C3a.2 Against the deterministic floor at the same bits [model]

The block wins at B = n·R bits exactly when

    c²/(d·(4^R − 1)) < ω_T(B ln 2)².                                                                    (a.1)

Three consequences.

1. **Flat block** (all aᵢ equal, ω = 2^(−R)): the block wins iff 4^R·(d − c²) > d, that is

       d > c²   and   B > B* = (n/2)·log₂(d/(d − c²)),     B* ↓ c²/(2 ln 2) as d → ∞.

   A flat block of at most c² + 1 positions (65 at c = 8, 17 at c = 4) never wins. B* at c = 8: 77.6 (n = 96), 64.7
   (128), 53.4 (256), 52.4 (288), 48.3 (768). At c = 4: 12.8 to 11.7. At c = 6.93 (one block of 96): 48.8.
2. **High rate, any spectrum.** Once the water is below the whole block, W_T = n·GM(a)·2^(−R) and the block's width is
   c·n·AM(a)·2^(−R)/√d, with AM and GM the arithmetic and geometric means of the block's amplitudes. So the block
   wins at high rate iff

       d·(GM/AM)² > c².                                                                                 (a.2)

   A rotation pays the ratio AM/GM of its block's amplitudes; a deterministic code does not. On the raw measured
   spectra the top 96 coordinates have AM/GM far above √(95)/8 = 1.22, so at c = 8 **a rotated block on the raw top
   of the spectrum never beats the same bits spent deterministically** (`c3a.log`: B* does not exist for [0, 96),
   [0, 224) or [0, 768) on groups 12 and 2, ideal curves). At c = 4 it wins from 19 to 20 bits.
3. **Low rate.** Since ω ≤ 1, B* ≥ B₀. A deterministic code of H nats can lower the block's sum by at most a₁·H
   (L4.4), so B* is close to B₀ when the block is flat and the budget small.

After an ideal prefix of 256 bits over all coordinates (water level at coordinate 144 of 768 on group 12, 142 on group
2) the blocks are flat at the top, and (a.1) gives, for group 12 (group 2 within 3 bits except where shown):

| Block | k_eff | c = 8 ideal B₀ / B* | c = 8 built B₀ / B* | c = 4 ideal B₀ / B* | c = 4 built B₀ / B* |
|---|---|---|---|---|---|
| [0, 96) | 96 | 36 / 78 | 61 / never | 11 / 13 | 24 / 23 |
| [96, 224) | 104 | 38 / 72 | 68 / 114 | 11 / 13 | 24 / 23 |
| [224, 480) | 109 | 42 / 68 | 84 / 129 (138 on group 2) | 12 / 13 | 26 / 23 |
| [480, 768) | 63 | 42 / 229 | 87 / never | 12 / 15 | 26 / 29 |
| [0, 224) | 200 | 41 / 57 | 81 / 101 | 12 / 13 | 26 / 21 |
| [0, 768) | 256 | 45 / 60 | 100 / 128 | 12 / 13 | 27 / 22 |

[computed, `c3a.py`.] "Built" compares the built block code with 1.19·W. "Never" means no budget up to 16 bits per
position. Reading:

- At c = 8 a block of 96 that the prefix has made flat needs 78 ideal bits (0.8 bit per position) before it is worth
  more than the same bits on its coordinates, and with the built code it never is: the block needs
  c·ρ_c·1.0245/ρ_det < √(d·(1 − 4^(−R))), and the left side is 9.0, 9.7, 10.4 and 11.6 at 1, 2, 3 and 7 bits against
  8.4, 9.4, 9.7 and 9.75 on the right. **The first randomised level (block 0 alone, 96 positions, 2 bits) sits at
  this margin: 9.7 against 9.4.** (Its own constant is 6.93, not 8, which moves the left side to 8.4 and puts it on
  the winning side by 11%.)
- Merging helps: the same 224 or 768 coordinates as one block break even at 57 to 60 ideal bits, fewer than any of
  their parts, because d enters (a.1) and (a.2) directly.
- At c = 4 every block of the built sizes wins from 13 to 15 ideal bits and 21 to 29 built bits: the hand-over comes
  about 4 times earlier in bits, as c² says.

### C3a.3 Bits inside a block: prefix against rotation [model; first-order conditions of (0.1)]

Inside a coded block the two kinds of bit are not rivals in the sense of C3a.2, because prefix bits no longer buy a
deterministic width: they shrink W_j, which the block then multiplies by c·√(t/d). Per nat, a prefix bit lowers the
block's width by the fraction 1/k_eff (dW/dH = −λ) and a rotation bit by 1/(n·cos²φ) (d ln √t/dR = −ln 2/(1 − 4^(−R))).
So at the best split of a block's bits

    cos²φ_j = 1 − 4^(−R_j) = k_eff,j / n_j.                                                              (a.3)

The prefix should reach exactly the share cos²φ of the block's coordinates. A prefix that has reached every
coordinate of a block (k_eff = n) is always worth less at the margin than a rotation bit. At 1 bit per position the
prefix should reach 3/4 of the block, at 2 bits 15/16, at ¼ bit 29%.

Across the parts of a layout, with ν the width bought per nat: the deterministic part has water level λ_D = ν, and a
coded block has λ_j = ν·S·d_j/(c·W_j·t_j) with S the sd. For a single block λ_j/ν = √(d/(c²·t)): the block's prefix
stops higher than a deterministic certificate's would by exactly the factor by which the block beats its own
Cauchy–Schwarz bound. A coordinate above the water therefore costs fewer prefix nats inside a coded block than in D,
by ½ ln(d/(c²t)) nats, and pays R_j rotation bits for its position: it belongs in the block when
½·log₂(d/(c²·t)) > R_j (2.6 bits against 1 at d = 767, c = 8, R = 1).

---

## C3 (b). The best hybrid as an optimisation

### C3b.1 The programme [model]

For a margin μ, B(μ) is the least number of bits over all layouts of (0.1):

    B(μ) = min { (H_D + Σ_j H_j)/ln 2 + Σ_j n_j R_j  :  W_D(H_D) + c·√(Σ_j W_j(H_j)²·t(R_j)/d_j) ≤ μ },

the minimum taken over the partition of the coordinates into D and blocks T_1, …, T_J, the prefix nats H ≥ 0 and the
rates R_j > 0. Equivalently h*(B) is the least width at B bits and B(μ) its inverse. The programme is not convex: t(R)
is infinite at R = 0, so whether a set of coordinates is coded at all is a discrete choice.

Structure of a solution (from the first-order conditions, C3a.3):

1. Inside every part the prefix is a water-filling, so a solution is fixed by the partition, one water level per
   part and one rate per block.
2. cos²φ_j = k_eff,j/n_j in every block whose prefix is not empty; k_eff,j ≥ n_j·cos²φ_j where it is empty.
3. Splitting a block at one rate never helps: (A₁ + A₂)²/(n₁ + n₂) ≤ A₁²/n₁ + A₂²/n₂. Blocks are worth having only
   for their different rates, and each costs a degree of freedom and its scalars.
4. **Bits per halving.** Let ν be the width bought per nat at a solution with D empty. Then h/ν = Σ_j n_j cos²φ_j:
   halving the width costs Σ_j n_j·cos²φ_j bits. (The rate condition of block j is
   ν = c·S·share_j·(1 + t_j)/n_j with share_j = W_j²t_j/(d_j S²) its share of the variance, so
   h/ν = n_j·cos²φ_j/share_j for every block; multiply by share_j and sum.)
   In the deterministic regime the same quantity is k_eff = W/λ. So along the whole frontier

       dB/d log₂(1/μ) = k(μ),   k = W/λ (deterministic), Σ_j n_j cos²φ_j (rotated), k → n at high rate.      (b.1)

   Check on the computed frontier (group 12, c = 8, ideal, 56 B): the solution has blocks of 96, 96, 128 and 448
   positions at 2.02, 0.94, 0.36 and 0.05 bits, so Σ n cos²φ = 90 + 70 + 50 + 30 = 240; the frontier's slope between
   48 and 72 B is 192 bits for a factor 1.70 in width, 251 bits per halving. [computed]

### C3b.2 How it was solved [computed]

`c3b.py`. Coordinates in decreasing order of γ; blocks are contiguous, [n_{j−1}, n_j), and the tail [n_J, n) is D;
J = 0 to 4; cuts on the grid 16, 32, 48, 64, 96, 128, 160, 192, 224, 256, 320, 384, 480, 576, 672, 768. For each
budget and J: coordinate descent on the cuts from two starting layouts (one is the built cuts), and inside it SLSQP on
the prefix bits of each part and the rate of each block with the budget as an equality, from two or three starting
points. This is a local search: every figure is an upper bound on h*(B), and a better layout may exist (a head in D,
blocks that are not contiguous, more than four blocks). "Ideal" uses W and the sphere bound and charges no scalars.
"Built" multiplies tan φ by ρ_c(R)·1.0245, multiplies a prefix's or D's sum by a factor rising from 1 at no bits to
1.19 (as 1 + 0.19·(1 − e^(−bits/16)); the ramp is an estimate), and charges 16 bits of scalars per coded block and 4
for D. Spectra: groups 12 (49,674 items, n_eff 76) and 6 (18,017 items, n_eff 89) of `agent1/spec_v1.json`.

### C3b.3 Results, group 12, c = 8 [computed in the model; `c3b_g12_c8_ideal.log`, `c3b_g12_c8_built.log`, `c3_tables.log`]

h/F at a budget of code bytes (the key is not counted). "Det" is the deterministic code alone (W, or 1.19·W with its
norm code), "one block" the best single rotated block with a prefix and a deterministic tail, "best" the best layout
found.

| Bytes | Ideal: det | one block | best | det / best | Built: det | one block | best | det / best |
|---|---|---|---|---|---|---|---|---|
| 4 | 0.695 | 0.817 | 0.695 | 1.00 | 0.830 | 1.439 | 0.830 | 1.00 |
| 8 | 0.566 | 0.626 | 0.566 | 1.00 | 0.686 | 0.796 | 0.686 | 1.00 |
| 12 | 0.482 | 0.521 | 0.482 | 1.00 | 0.584 | 0.665 | 0.584 | 1.00 |
| 16 | 0.421 | 0.431 | 0.386 | 1.09 | 0.509 | 0.568 | 0.509 | 1.00 |
| 24 | 0.337 | 0.306 | 0.266 | 1.27 | 0.406 | 0.438 | 0.406 | 1.00 |
| 32 | 0.279 | 0.230 | 0.197 | 1.42 | 0.336 | 0.346 | 0.336 | 1.00 |
| 40 | 0.237 | 0.180 | 0.154 | 1.54 | 0.285 | 0.278 | 0.276 | 1.03 |
| 56 | 0.179 | 0.120 | 0.102 | 1.74 | 0.214 | 0.190 | 0.182 | 1.18 |
| 96 | 0.101 | 0.055 | 0.048 | 2.11 | 0.120 | 0.089 | 0.082 | 1.47 |
| 160 | 0.048 | 0.022 | 0.019 | 2.48 | 0.057 | 0.036 | 0.033 | 1.73 |
| 256 | 0.018 | 0.0076 | 0.0067 | 2.74 | 0.022 | 0.0122 | 0.0116 | 1.88 |
| 384 | 0.0059 | 0.0023 | 0.0020 | 2.99 | 0.0070 | 0.0036 | 0.0035 | 2.01 |

The layouts found, ideal curves: four blocks at every budget from 16 B, the prefix on the first block only.

| Bytes | Blocks (positions) | Prefix bits | Rates (bits per position) | Σ n cos²φ | Frontier's slope, bits per halving |
|---|---|---|---|---|---|
| 16 | 32, 32, 96, 416 (tail of 192 in D) | 12 | 1.53, 0.77, 0.28, 0.04 | 101 | 112 |
| 32 | 64, 64, 128, 512 | 42 | 1.72, 0.79, 0.28, 0.03 | 165 | 163 |
| 56 | 96, 96, 128, 448 | 91 + 3 | 2.02, 0.94, 0.36, 0.05 | 241 | 251 |
| 96 | 160, 96, 128, 384 | 200 + 5 | 2.18, 1.18, 0.55, 0.08 | 338 | 341 |
| 160 | 192, 128, 160, 288 | 300 + 30 + 3 | 2.98, 1.73, 0.80, 0.09 | 446 | 451 |
| 256 | 256, 128, 96, 288 | 479 + 44 + 10 + 12 | 3.76, 2.47, 1.58, 0.26 | 550 | 565 |

Built curves: two or three blocks, rates held near 1.7, 1 and 0.1 at every budget from 40 to 160 B, and every further
bit put in the prefix of the first block (87 bits at 40 B, 108 at 56 B, 265 at 96 B, 580 at 160 B, 1,115 at 256 B).

Reading.

1. **Where the hand-over belongs.** With ideal curves a hybrid first beats the deterministic code between 12 and
   16 B. With the built codes' measured distances it is between 32 and 40 B. The built schedule hands over at 32 B.
2. **How much the randomised levels are worth.** At equal bytes the ideal hybrid is 1.4 times narrower than W at
   32 B, 2.1 at 96 B and 2.7 at 256 B. The limit is √d/c (3.46 for one block of 768 at c = 8): at high rate a flat
   block of d degrees of freedom is c·2^(−R)·F/√d against F·2^(−R). With the built codes the gain is 1.2 at 56 B, 1.5
   at 96 B and 1.9 at 256 B, about two thirds of the ideal one, because the block code is 1.3 to 1.7 times its bound
   where the deterministic code is 1.19 times its own.
3. **The ideal optimum is not the built shape.** It codes every coordinate from the first randomised bit, at rates
   that fall by a factor of 2 to 3 from block to block, with a tail of 300 to 500 positions at 0.03 to 0.3 bit. Its
   prefix is short: 42 bits at 32 B and 200 at 96 B, where the index has 256 bits before any rotation.
4. **With the built codes the optimum moves the other way**: longer prefixes and low, constant rates. The reason is
   in the measured curves: the built block code loses ground with rate (1.31 times the bound at 1 bit, 1.69 at 7)
   and the model's deterministic factor stays at 1.19. The 1.19 is measured at 52 to 296 stored bits only, so the
   prefixes of 580 and 1,115 bits are an extrapolation [estimate].
5. **One block against four.** One block of all the coded coordinates is 13 to 17% wider than four with ideal curves
   and 4 to 9% wider with built curves. The blocks are worth that much.
6. The identity (b.1) holds along the computed frontier to within 4% from 24 to 256 B (last two columns).

### C3b.4 Results at c = 4, and what c changes [proved in the model for the first item; the rest computed]

1. **Once D is empty the best layout does not depend on c.** Then h = c·S, and the layout that minimises S at a
   given number of bits is the same for every c, so h*_c(B) = c·S*(B). c enters only through the hand-over and
   through how much of the tail stays in D. Computed: from 24 B (ideal) and from 56 B (built) the c = 4 and c = 8
   solutions have the same cuts, prefixes and rates, and half the width (`c3b_g12_c4_*.log`).
2. **The hand-over moves as c².** Group 12, first budget at which a hybrid beats the deterministic code alone:

   | | c = 8 | c = 4 |
   |---|---|---|
   | Ideal curves | 16 B (det still best at 12 B) | 4 B (det still best at 2 B) |
   | Built curves | 40 B (det still best at 32 B) | 8 B (by 2%; by 13% at 12 B) |

   This is C3a's B₀ ≈ 0.72·c² bits seen through the spectrum: 46 against 11.5 bits for a long flat block.
   Group 6 gives the same four entries: a tie at 12 B (0.7%) and 11% at 16 B for c = 8 ideal; 40 B for c = 8 built;
   a tie at 2 B and 24% at 4 B for c = 4 ideal; 5% at 8 B and 16% at 12 B for c = 4 built (`c3_tables.log`).
3. At c = 4 the hybrid over the deterministic code alone, h ratio at equal bytes: ideal 1.29 at 4 B, 1.67 at 8 B, 2.18
   at 16 B, 2.83 at 32 B, 5.5 at 256 B; built 1.02 at 8 B, 1.35 at 16 B, 1.82 at 32 B, 2.35 at 56 B.
4. **Scope of "c = 4".** These figures are for a certificate that keeps the form of (0.1) with a smaller constant,
   that is a guarantee at a larger failure probability. They are not the approximate mode as built: there the
   deterministic levels are read with a statistical width (ρ_{g,ℓ} of chapter 0, an l2 quantity), not with W, so
   the l1 floor is not what a rotated block competes with at ψ ≈ 4. I did not model that comparison.

---

## C3 (c). The built schedule against the optimum

The built schedule in the model (`c3c.py`, logs `c3c_g12_c8.log`, `c3c_g12_c4.log`): deterministic levels at 32, 64,
96, 128, 192 and 256 code bits; then the 256-bit prefix, blocks of 96, 128, 256 and 288 positions in the sorted order
and the rates of group 0 of the 1M build, [2, 0, 0, 0], [3, 1, ¼, 0], [4, 3, 1, 0], [7, 4, 2, ¼] at 56, 96, 160 and
256 B [measured rates, `codes/NOTEBOOK.md` entry 1]; an uncoded block adds its Cauchy–Schwarz sum. One constant c at
every level. "Ideal" rows compare at the nominal bytes; "built" rows charge the stored bytes with norm codes and
scalars (4.5, 9, 13.5, 18, 27, 36, 65.5, 107.5, 174.6, 275.3 B) and compare with the built-curve optimum, which pays
its own scalars.

Group 12 [computed in the model]. Each cell: the schedule's width over the optimum's at the same bytes, and in
brackets the schedule's bytes over the bytes at which the optimum reaches the same width.

| Level | c = 8 ideal | c = 8 built | c = 4 ideal | c = 4 built |
|---|---|---|---|---|
| 4 B | 1.00 (1.00) | 1.03 (1.11) | 1.29 (1.56) | 1.03 (1.11) |
| 8 B | 1.00 (1.00) | 1.03 (1.07) | 1.67 (2.18) | 1.09 (1.13) |
| 12 B | 1.00 (1.00) | 1.04 (1.08) | 1.96 (2.55) | 1.27 (1.34) |
| 16 B | 1.09 (1.12) | 1.05 (1.09) | 2.18 (2.77) | 1.49 (1.48) |
| 24 B | 1.27 (1.29) | 1.07 (1.10) | 2.53 (2.98) | 1.80 (1.79) |
| 32 B | 1.42 (1.41) | 1.10 (1.11) | 2.83 (3.13) | 2.06 (1.98) |
| 56 B | 1.92 (1.74) | 1.68 (1.48) | 3.62 (3.35) | 3.10 (2.47) |
| 96 B | 1.45 (1.28) | 1.71 (1.40) | 1.53 (1.33) | 1.79 (1.44) |
| 160 B | 1.25 (1.12) | 1.40 (1.18) | 1.46 (1.22) | 1.58 (1.26) |
| 256 B | 1.41 (1.15) | 1.81 (1.26) | 1.41 (1.15) | 1.81 (1.26) |

Width of the schedule in the model, h/F at c = 8: ideal 0.695, 0.566, 0.482, 0.421, 0.337, 0.279, 0.197, 0.069,
0.024, 0.0094; built 0.827, 0.673, 0.574, 0.501, 0.401, 0.332, 0.246, 0.116, 0.039, 0.0169.

**Where the built schedule is far from the optimum.**

1. **The 56 B level, by 1.7 to 1.9 in width and 1.5 to 1.7 in bytes at c = 8** (3.1 to 3.6 and 2.5 to 3.4 at c = 4).
   It codes block 0 at 2 bits and leaves 672 positions to Cauchy–Schwarz. In the model the uncoded blocks are 0.174·F
   of its 0.197·F (ideal) and 0.207 of 0.246 (built). This does not rest on the model: on the 50k dump the uncoded
   term is 0.0427 of a mean half-width of 0.0585 for item queries and 0.0745 of 0.0942 for text queries, 73% and 79%
   [measured, `reports/bounds3/dump_stats.log`, c = 9.33]. The optimum at the same bytes codes everything: with the
   built cuts and prefix and free rates, 0.73, 0.60, 0.15 and 0.02 bits give 0.113·F against 0.197 (ideal). With the
   built code the same move gives 0.195 against 0.246, a fifth, because the built code at sub-bit rates is 1.5 times
   its bound and a tail block of 672 positions then needs about 100 bits (c = 7.96) before it beats its own
   Cauchy–Schwarz term (C3a.1). **So the 56 B level is the place where the codes track's 16-word codebook below one
   bit, or any code near the bound at ⅛ to ½ bit, would pay most.**
2. **The deterministic levels from 16 to 32 B, under ideal codes only**: 1.09 to 1.42 wider at c = 8 than a hybrid
   that starts rotating at 16 B. Under the built codes they are within 3 to 10% of the optimum, and that margin is
   their six norm codes against one. The built hand-over at 32 B is where the built codes put it (C3b.3).
3. **At c = 4, every deterministic level from 12 B on**: 1.27 to 2.06 wider than the built-curve optimum, which hands
   over at 8 to 12 B. See C3b.4 item 4 for what this does and does not say about the approximate mode.
4. **The levels at 96 to 256 B: 1.25 to 1.45 (ideal) and 1.4 to 1.8 (built) in width, 1.12 to 1.40 in bytes.** In
   the model nearly all of the ideal figure is the rates, not the cuts or the prefix: the built cuts and the 256-bit
   prefix with free rates are within 10%, 2%, 5% and 7% of the optimum at 56, 96, 160 and 256 B. But the model's
   block sums after the prefix (0.105, 0.114, 0.057, 0.004 of F) are not the index's: the dump's level-0 means put
   block 0 at about 0.039 against 0.043 for the other three, 1.5 times heavier than the model's. The built rates were
   fitted to the real norms. **I therefore do not claim the built rates are wrong**; this row is an upper bound on
   their loss. A test that needs no spectrum: at the best rates for fixed blocks, block j's share of the variance is
   n_j·cos²φ_j/Σ n cos²φ over the coded blocks (the rate condition in C3b.1; with the built code's curve replace
   cos²φ by −d ln t_built/dR ÷ (2 ln 2·(1 + t))·cos²φ, a correction of a few percent). With group 0's rates that is
   [0.36, 0.36, 0.28, 0] at 96 B, [0.23, 0.30, 0.46, 0] at 160 B and [0.18, 0.23, 0.44, 0.15] at 256 B. The 50k
   dump's mean shares for item queries are [0.336, 0.398, 0.260, 0.005], [0.371, 0.227, 0.385, 0.017] and
   [0.147, 0.173, 0.622, 0.058] [measured, `dump_stats.log`]. The two are close at 96 B and differ at 160 B (block 0
   heavier than the condition allows) and 256 B (block 2 heavier). **This comparison is indicative only**: the 50k
   index codes all four blocks from 96 B on, so its rates are not group 0's, and I could not read them. The check
   needs each level's rates from the build log beside the dump's shares; it is item T4 of C4.4. The built-curve
   figure in the table also contains the optimum's longer prefix, which rests on an extrapolation (C3b.3 item 4).

**Does the optimum's shape depend on c?** No beyond the hand-over (C3b.4 item 1: the same code for every c once
everything is in a block). Yes at the hand-over: a certificate at c = 4 wants rotation from 4 to 12 B and one at
c = 8 from 16 to 40 B. One code can serve both only by starting its blocks early and letting the exact mode ignore
them until they pay, which costs the exact mode the deterministic bits those blocks displaced.

Group 6 (F = 0.279, n_eff 89) gives the same table to within 0.03 in every cell that was computed (c = 8: ideal
1.12, 1.30, 1.44 at 16, 24, 32 B and 1.94, 1.45, 1.24, 1.42 at 56 to 256 B; built 1.70, 1.71, 1.40, 1.82), and the
same hand-overs, 12 to 16 B ideal and 40 B built [computed, `c3c_g6_c8.log`, `c3b_g6_c8_*.log`]. At c = 4 its
built-curve column reads 1.04, 1.12, 1.32, 1.55, 1.85, 2.12 at 4 to 32 B and 3.13, 1.78, 1.56, 1.82 at 56 to 256 B
(`c3c_g6_c4.log`), against group 12's 1.03, 1.09, 1.27, 1.49, 1.80, 2.06 and 3.10, 1.79, 1.58, 1.81.

---

## C4. A rate–margin law

### C4.1 Statement [model; item 2 is the shell identity, proved in `sublinear/REPORT.md` section 4]

Local symbols. For item i and a query: μ_i = |s(x_i) − t| is its **margin**; η_i is its zero-bit bound, the
Cauchy–Schwarz product of the offset and the residual in the group's metric, which the query knows from the key
(E η = F in the model); u_i = μ_i/η_i is its **normalised margin**. w(B) = h*(B)/F is the group's normalised frontier
(C3b) and B*(u) its inverse. c_e is the ratio of a level's half-width to the sd of its estimate's error (c at a
randomised level, about √n_eff ≈ 8.7 at a deterministic one).

1. **Per item.** An item is decided at the first level whose half-width is below its margin, up to the error of the
   estimate: it survives a level of normalised width w exactly when |u_i + (w/c_e)·Z| ≤ w, Z the standardised error.
   So the bits an item must have read are a function of u_i alone, B*(u_i) for the best layout at every budget, and
   for a schedule with cumulative bytes β_1 < … < β_L and widths w_1 > … > w_L the staircase β(u) = β_ℓ for
   w_ℓ ≤ u < w_(ℓ−1), with the full vector below w_L and nothing at u ≥ 1.
2. **Per query.** bytes = Σ_i β(u_i) = Σ_ℓ b_ℓ·#{i : u_i < w_(ℓ−1)}, with b_ℓ the bytes level ℓ adds.
3. **Local form.** By (b.1), dB*/d log₂(1/u) = k(u), the dimension in play: W/λ while the code is deterministic,
   Σ_j n_j cos²φ_j once it is rotated, n at high rate. Hence

       bits per item = ∫₀¹ k(u)·Pr[U < u] d log₂(1/u):

   each halving of the width costs k(u) bits for every item still inside the shell of that width.
4. **Prediction.** A query's bytes are fixed by the distribution of its normalised margins and by the group's
   frontier. The query type, the answer size and the visible set enter only through that distribution. Two items
   with the same u read the same bytes, whatever their scores.

The frontier on the measured spectra (group 12, c = 8, ideal; group 6 is 6 to 9% higher in u) [computed]. k is the
frontier's slope up to 12 B, where the code is deterministic, and Σ n cos²φ from 16 B:

| Bytes | 4 | 8 | 12 | 16 | 24 | 32 | 56 | 96 | 160 | 256 | 384 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| u = w(B) | 0.695 | 0.566 | 0.482 | 0.386 | 0.266 | 0.197 | 0.102 | 0.048 | 0.019 | 0.0067 | 0.0020 |
| k, bits per halving | 96 | 121 | 116 | 101 | 133 | 165 | 241 | 338 | 446 | 550 | 629 |

So the law is not a power law. An item at a tenth of its zero-bit bound needs 56 B, at a hundredth about 215 B, and
each further halving tends to 96 B (n bits). The deterministic floor alone (W) needs 96 B at a tenth.

### C4.2 Evaluation for Gaussian scores [model; `c4.py`, logs `c4_g12_c8.log`, `c4_g6_c8.log`]

Assumptions. Scores over the corpus N(·, σ_s²) with σ_s = 0.068 for item queries and 0.040 for text queries
[measured, `sublinear/REPORT.md` 5.4]; the threshold at the answer share. η_i = κ·F_g for every item of group g, the
16 groups of `agent1/spec_v1.json` weighted by their items (mean F = 0.158); every group uses group 12's normalised
curves. Undecided after 256 B reads 1,536 B more. Rows:

- (1) **floor**: the frontier of C3b with ideal curves at c = 8, read continuously, no scalars;
- (2) the same frontier readable only at 4, 8, 12, 16, 24, 32, 56, 96, 160 and 256 B (**level spacing**);
- (3) the built schedule with ideal curves (**layout**: hand-over, prefix, cuts, rates);
- (4) the built schedule with the built codes' measured distances (**code**);
- (5) with the stored norm codes and scalars and 2 B of key per candidate (**scalars**).

Bytes per item of N. κ = 1 uses the spectra as they are and fits nothing.

| Query, κ | Answer | (1) | (2) | (3) | (4) | (5) | Measured | (5) / measured |
|---|---|---|---|---|---|---|---|---|
| Item, κ = 1 | 0.01% | 0.6 | 1.0 | 1.1 | 1.5 | 2.0 | 2.9 | 0.69 |
| | 0.1% | 1.6 | 2.6 | 3.1 | 4.3 | 5.3 | 5.8 | 0.91 |
| | 1% | 5.5 | 8.6 | 10.8 | 15.2 | 17.6 | 18.6 | 0.95 |
| | 10% | 20.1 | 33.3 | 44.1 | 62.0 | 68.1 | 133.2 | 0.51 |
| | 50% | 37.7 | 64.9 | 86.4 | 122.1 | 131.7 | 276.2 | 0.48 |
| Text, κ = 1 | 0.01% | 3.1 | 4.5 | 5.0 | 7.1 | 9.0 | 20.6 | 0.44 |
| | 1% | 12.2 | 17.7 | 23.2 | 32.2 | 37.0 | 67.0 | 0.55 |
| | 50% | 55.6 | 99.4 | 131.7 | 187.4 | 200.0 | 322.8 | 0.62 |
| Text, κ = 1.742 | 0.01% | 10.0 | 12.7 | 16.2 | 22.5 | 27.2 | 20.6 | 1.32 |
| | 0.1% | 14.1 | 18.3 | 25.2 | 34.3 | 40.4 | 26.1 | 1.55 |
| | 1% | 23.4 | 33.2 | 47.1 | 63.7 | 72.2 | 67.0 | 1.08 |
| | 10% | 50.2 | 86.1 | 116.0 | 163.0 | 175.4 | 184.5 | 0.95 |
| | 50% | 79.5 | 150.9 | 196.5 | 283.8 | 299.0 | 322.8 | 0.93 |

κ = 1.742 for text is fitted to one number, the measured share of candidates at a 1% answer (95%, SYSTEM.md 2.2).
The same fit for item queries (22% candidates) gives κ = 0.615 and row (5) = 0.4, 1.7, 8.5, 42, 87, between 0.14 and
0.46 of the measured bytes. With the estimate's error smeared (c_e = 8) row (5) moves by less than 0.2 B. With group
6's curves in place of group 12's row (5) is 7 to 13% higher (item, κ = 1: 2.2, 6.0, 19.4, 73.6, 141.2).

Text queries with the measured shells of `sublinear/REPORT.md` 5.4 in place of the Gaussian tail (`c4b.py`,
`c4b_g12_c8.log`), row (5) at κ = 1.742: 54.0, 24.9 and 16.5 B at 1%, 0.1% and 0.01%, which is 0.81, 0.95 and 0.80
of the measured 67.0, 26.1 and 20.6.

Share of rows whose full vector is read, row (5): item, κ = 1: 0.00, 0.03, 0.21, 1.38, 3.13% against 0.11, 0.24,
0.71, 2.85, 8.43% measured; text, κ = 1.742: 0.01, 0.08, 0.63, 4.09, 9.25% against 0.55, 0.40, 0.78, 4.80, 10.38%.

**What holds and what failed.**

- Text queries: one fitted scale reproduces the bytes within 8% from 1% to 50% answers and, with the measured
  shells, within 20% down to 0.01%. The Gaussian tail overshoots the two smallest answers by 32% and 55%, as it
  should: the measured threshold sits at 4.98 sd where a Gaussian has 3.72.
- Item queries with nothing fitted: within 9% at 0.1% and 1%, 31% low at 0.01%, and **half the measured bytes at 10%
  and 50%**. No single κ repairs this: the candidate share asks for 0.6 and the bytes at 50% for about 2.
  **In this form (Gaussian scores, one scale per group) the law is refuted for in-distribution queries.**
- The full-vector share is under-predicted everywhere for item queries, by 2 to 3.5 at 1% to 50%.
- Likely causes, none tested here. (i) η is not one number per group. The spectra's offsets are those of
  pseudo-queries against their four nearest centres; at a 50% answer the items near the threshold sit in average
  clusters, whose offsets are longer. (ii) For item queries the support-table cut is stronger than the zero-bit
  bound, so the candidate share understates η. (iii) Item scores are a mixture, more peaked at the centre and
  heavier in the tail than a Gaussian of the same sd. (iv) The model's last-level widths are narrower than the 50k
  dump's means for survivors: 0.0183, 0.0061 and 0.0027 at 96, 160 and 256 B (0.116, 0.039 and 0.0169 times the mean
  F) against 0.0232, 0.0098 and 0.0037 after rescaling the dump from c = 9.33 to 8, ratios 1.27, 1.61 and 1.40
  [estimate: survivors are selected for wide intervals and the 50k index has longer residuals].
- So the absolute level of the prediction is not established. What C4.4 tests is item 4 of the statement, which needs
  no score model and no κ, because the dump holds each item's own scale.

### C4.3 How much of the cost is which loss [model]

The split of row (5) is stable across query type and answer size, which the absolute level is not:

| Cell | Floor | Level spacing | Layout | Code distance | Scalars and key |
|---|---|---|---|---|---|
| Item, κ = 1, 0.01% (2.0 B) | 30% | 20% | 5% | 20% | 25% |
| Item, κ = 1, 1% (17.6 B) | 31% | 18% | 12% | 25% | 14% |
| Item, κ = 1, 50% (131.7 B) | 29% | 21% | 16% | 27% | 7% |
| Text, κ = 1.742, 1% (72.2 B) | 32% | 14% | 19% | 23% | 12% |
| Text, κ = 1.742, 50% (299.0 B) | 27% | 24% | 15% | 29% | 5% |

Against SYSTEM.md's three figures:

- **18.6 B at a 1% in-distribution answer.** The model gives 17.6 B with nothing fitted: floor 5.5 B, level spacing
  3.1, layout 2.2, code distance 4.4, scalars and key 2.4; 1.0 B is not explained.
- **67 B at a 1% out-of-distribution answer.** With the fitted scale, 72.2 B: floor 23.4, level spacing 9.8, layout
  13.9, code distance 16.6, scalars and key 8.5. With the measured shells the total is 54.0 B.
- **276 B at a 50% in-distribution answer.** The model gives 131.7 B and explains less than half, so I do not split
  the measured figure. The split of the modelled part is in the table.

Three qualifications. "Floor" is the best layout of the family (0.1) with ideal curves at c = 8, not a lower bound
over all indexes; the deterministic floor W alone would read 7.4 B (item, 1%), 57.3 B (item, 50%) and 34.2 B (text,
1%), so an ideal hybrid reads 26 to 34% fewer bytes than an ideal deterministic code. "Layout" is the least
certain column: with the built cuts and prefix and free rates it falls from 2.2 to 1.0 B in the 1% item cell, and
C3 (c) item 4 says the rest may be the model's block norms, not the index's rates. "Level spacing" assumes ten levels
at the built sizes; it is the price of reading in steps whose widths fall by 1.2 to 2.9 from one to the next, and
more levels would lower it at the cost of their scalars.

### C4.4 The test on the 50k verify dump [specified; the script is written and has run only on synthetic records]

`c4_dump_test.py dump.bin c0 c1 c2 c3 b0 b1 b2 b3 [out.json]`, with c_ℓ the build's constant at randomised level ℓ
(6.724 and 7.639 for the final 50k build; take them from the run's own output) and b_ℓ the bytes a survivor reads on
entering level ℓ (29.5 for the first, then the build log's stored sizes of the other three). One pass in chunks of
400,000 records (38 MB) after a first pass that keeps 12 bytes per level-0 record; nothing is timed.

**Read from each record** (24 float32; layout as `reports/bounds3/dump_stats.py`): the tag (index-plan bit, answer
size, query), the row, the level ℓ, truth − t, estimate − t, the deterministic part of the half-width, sd, and per
block the coded flag, a_j and R_j. Records without the index-plan bit are dropped.

**Derived per record.**
- h = c_ℓ·sd + deterministic part; survived = |estimate − t| ≤ h; μ = |truth − t|.
- The item's scale η: Σ_j R_j of its level-0 record if R_j is filled for coded blocks (the script checks), otherwise
  its level-0 half-width. Records of deeper levels are joined to it on (tag, row).
- u = μ/η, w = h/η, tan φ_j = a_j/R_j for coded blocks.

**Outputs and what the law predicts.**
- T1. Per (query class, level): the geometric mean and the spread of w. The law needs w to be a property of the
  level: sd of ln w at most about 0.2.
- T2. Per (query class, level): survival against u in 60 logarithmic bins. Predicted: a step at u = w̄_ℓ smeared by
  the estimate's error, half the entrants surviving at u = w̄_ℓ and the 90% and 10% points at w̄_ℓ·(1 ∓ 1.28/c_ℓ),
  widened by the spread found in T1. The curve must be the same for item and text queries once u is the argument.
- T3. Per (query, answer size): the bytes of the randomised levels and full vectors, predicted from the histogram of
  u at level 0 and the four w̄_ℓ alone, against the bytes the records imply.
- T4 (by hand from the output). The rms tan φ_j per block and level against the sphere bound at that level's rate
  from the build log: it should reproduce the codes track's 1.31 to 1.77, times 1.0245. And the mean variance
  shares against n_j·cos²φ_j/Σ n cos²φ (C3 (c) item 4).
- T5 (by hand). Between successive levels, (bits added)/log₂(w̄_ℓ/w̄_(ℓ+1)) against Σ_j n_j cos²φ_j with the
  measured tan φ: the identity (b.1) holds at an optimum, so the ratio measures how far each step is from one. In
  the model the built steps cost 1.12 to 1.28 times the optimum's bytes at 96 to 256 B.

**What would refute the law.**
1. sd of ln w above 0.3 inside a (class, level) cell. Then one normalised margin does not fix the level, and the law
   must be restated with the vector of block shares as its argument.
2. In T2, the half-survival point further than 10% from w̄_ℓ, or a 10-to-90 span more than twice the predicted
   one after allowing for T1's spread, or survival curves for item and text queries that differ by more than 5
   points at the same u. The last is the sharpest: it would mean query type enters other than through u.
3. In T3, predicted over actual bytes outside 0.90 to 1.10 pooled, or an rms relative error per (query, size)
   above 20%.
4. In T5, a step whose bits per halving are below Σ n cos²φ by more than the scalars' share: the identity is a
   bound at fixed curves, so that would mean the model's curves are wrong, not the law.

**What the dump cannot test.** It holds no record for the deterministic levels or for items decided before 56 B, so
the hand-over (C3b.3 item 1), the deterministic part of the staircase and the absolute bytes of a query are out of
its reach. Those need the per-level entrant counts the verify run prints (or a dump extended to the six
deterministic levels with the norm code's bound in the R_j slot).

---

## C5. The cost of stored scalars

Added at the PI's request after the owner pointed out that every coded block makes the query read and decode that
block's scalars per item, and that a native trial of three coded blocks at 56 B lost on time. C3 and C4 above are
left as written. Each place where a conclusion of theirs changes is marked **Changes** below and listed in C5.6.

### C5.1 What a coded block costs [measured where a source is named; the per-rise time is an estimate]

**Bytes.** `rust/results/system/bd_p2c.out`, build line: stored bytes per item, cumulative by level, key included:
6.5, 11.0, 15.5, 20.0, 28.5, 37.0, 66.5, 106.9, 176.8, 276.3 [measured].

- The first randomised level is 29.5 B: 24 B of code and 5.5 B of scalars (SYSTEM.md section 5). The 5.5 B are |y_j|
  in 8 bits for each of the four blocks, coded or not, and κ in 8 bits and tan φ in 4 bits for the one coded block
  (SYSTEM.md 2.1).
- The levels at 96, 160 and 256 B store 40.4, 69.9 and 99.5 B. Group 0's rates give 36, 68 and 93 B of code, so
  about 12.8 B of scalars over the 10 rises of those levels: 1.3 B per rise [estimate: only group 0's rates are
  logged and the bytes are a mean over groups].
- **Per (item, coded block, rise): 1 to 1.5 B (κ at 4 or 8 bits, tan φ at 4 bits). Per block, once: 1 B of |y|.**
  Sure to within the 4-or-8-bit choice. `rust/REPORT_CONST.md` step 4 agrees: one more 4-bit scalar row is 0.5 B
  per block per rise.
- Correction to C3 (c) and C4: my stored-bytes list (4.5, 9, 13.5, 18, 27, 36, 65.5, 107.5, 174.6, 275.3 without
  the key) was wrong at four levels. The log gives 4.5, 9, 13.5, 18, 26.5, 35, 64.5, 104.9, 174.8, 274.3.

**Time.** SYSTEM.md section 4.2 (`bd_p2c.out`, profiled, ms per query at 1M, one thread) [measured]:

| | Item 1% | Item 50% | Text 1% | Text 50% |
|---|---|---|---|---|
| Level decode | 1.24 | 18.23 | 7.36 | 19.55 |
| Bounds and decisions | 1.19 | 21.23 | 8.23 | 22.66 |
| of which the randomised scalars' decode | 0.58 | 10.78 | 4.20 | 11.61 |
| Re-pack and compaction | 1.66 | 9.14 | 7.97 | 8.90 |
| Full vectors | 0.86 | 10.90 | 1.11 | 13.54 |
| Profiled wall | 12.79 | 87.69 | 47.97 | 93.34 |
| Scalars' decode, share of the profiled wall | 4.5% | 12.3% | 8.8% | 12.4% |

`rust/REPORT_BOUNDS.md` Part 1: before that work the scalar decode was 12.2 to 13.1 ms of 23 ms, mostly AVX2 gathers
from 256-entry tables; with two permutes and a multiply per 8-bit scalar it fell to 9.9 ms at item 50% and 3.8 ms at
text 1%. Its closing section: "the scalars are now mostly the tan permute and the per-half bookkeeping of the rises".
`rust/REPORT_RAND.md` step 2: kappa and tan are recomputed at every rise, and on four levels every coded block rises
at every level. `rust/REPORT_SETUP.md` has no per-item constant for the scalars.

To turn the phase into a cost per rise I need the number of rises decoded, and the breakdown file does not print
entrants per level. At item 50%: 91% of items enter 56 B and 20% enter 256 B (SYSTEM.md 4.1); the table counts give
8.17 item-levels per item over the ten levels; with the bytes, and since no more can enter 160 B than 96 B, these
leave the share entering 96 B between 0.62 and 0.91. So the rises decoded are 5.4 to 6.0 per item of N, and

- **scalar decode: 1.8 to 2.0 ns per (item, coded block, rise) at item 50%** [estimate from a measured phase and
  inferred counts]. If the four |y| of the first level are counted as scalars of equal cost, 1.4 to 1.5 ns per rise
  and 0.7 ns per |y|. Text 1%, with rougher counts: 1.75 to 2.8 ns. I use **1.4 to 2.8 ns**.
- **All-in, from the trial**: `--design-exact` "codes three blocks at 56 B instead of one, and items entering 96 B
  fall 2%. But it is 8–24% slower at 1M, because the first level decodes more scalars" (`rust/REPORT_BOUNDS.md`,
  Part 2). At item 50% (71.3 ms; 0.91 of items enter 56 B; two extra block-rises each) 8% is 3.1 ns and 24% is
  9.4 ns per extra (item, block); at text 1% (37.7 ms, 0.46 entering) 3.3 and 9.8 ns. **3 to 10 ns per extra
  (item, coded block) at the first level** [estimate: the report does not say which cell is which, and the trial
  changed the rates of every level, not only the number of blocks]. This is above the decode alone because a coded
  block also adds accumulator updates in the bound, a state array to compact and table units to build.
- For scale [measured phases over inferred counts, item 50%]: level decode is 0.13 ns per code byte per item (18.23 ms
  over about 137 B of code per item); everything else per (item, level) is about 3.2 ns; a full vector is 104 ns
  (SYSTEM.md 2.2; the timed plan constant is 110 ns); the planner's own cost per candidate is 7.8 + 31.3·depth ns
  (`rust/src/v2/q.rs`, `bd_p2c.out`).

**So one block-rise costs 1 to 1.5 B, about 1% of what an item reaching 256 B stores, but as much time as decoding
11 to 21 B of code (decode alone) or 23 to 75 B (all-in).** In bytes scalars are 4 to 5% of what a query reads at
item 50% (about 14 of 301 B); in time their decode alone is 9 to 12% at the three larger cells. C3 and C4 counted
the first and not the second.

A second omission surfaced here: the constant. It is 6.931 with one block coded and 7.963 with several (SYSTEM.md
section 3), and C3 held c fixed. A level that codes a second block pays 15% on its whole random term.

### C5.2 The first randomised level, alternatives valued in bytes and in time [model with measured costs]

`c5a.py`, `c5a.log`. Built prefix (256 bits) and built cuts, 192 code bits, built curves, the constant by number of
coded blocks (c1(d) exact for one block: 6.931 at d = 95; 7.963 otherwise). "Menu" restricts rates to 0, ⅛, ¼, ½, 1,
2, 3, 4, as the index does. Width at 32 B in the model: 0.332·F.

| First level | Rates | h/F | Against built |
|---|---|---|---|
| Built, one block | 2, 0, 0, 0 | 0.2405 | |
| Two blocks, free rates | 0.94, 0.79, 0, 0 | 0.198 | −17.6% |
| Two blocks, menu | 1, ½, 0, 0 | 0.233 | −3.1% |
| Three blocks, free | 0.75, 0.62, 0.16, 0 | 0.194 | −19.3% |
| Three blocks, menu | 1, ½, ⅛, 0 | 0.205 | −14.9% |
| Four blocks, free | 0.73, 0.60, 0.15, 0.02 | 0.195 | −18.9% |
| Four blocks, menu | ½, ½, ⅛, ⅛ | 0.237 | −1.4% |

A narrower first level changes one count only: the items entering 96 B. An item enters a later level when its
margin is inside the previous level's width, and the later widths do not move. So per item of N

    Δtime = e_56·(J − 1)·τ − Δe_96·(τ_0 + τ_byte·36 + 3τ),      Δbytes = e_56·(J − 1)·1.5 − Δe_96·(36 + 4.5),

with τ the cost of a block-rise, τ_0 = 3.2 ns, τ_byte = 0.133 ns, e_56 the share entering 56 B and Δe_96 the fall in
the share entering 96 B. The margins are C4.2's (Gaussian; item κ = 1, text κ = 1.742).

| Cell (e_56, e_96 as built) | Alternative | Entering 96 B | Bytes per item | ns per item at τ = 1.8 / 3 / 10 | Pays in time below |
|---|---|---|---|---|---|
| Item 1% (0.068, 0.040) | 2 blocks, free | −24% | −0.29 | −0.01 / +0.04 / +0.31 | 2.0 ns |
| | 3 blocks, menu | −21% | −0.14 | +0.13 / +0.27 / +1.04 | 0.6 ns |
| Item 50% (0.536, 0.413) | 2 blocks, free | −16% | −1.82 | +0.10 / +0.51 / +2.90 | 1.5 ns |
| | 3 blocks, free | −17% | −1.27 | +0.98 / +2.01 / +8.02 | 0.7 ns |
| | 3 blocks, menu | −13% | −0.60 | +1.20 / +2.29 / +8.66 | 0.5 ns |
| Text 1% (0.480, 0.289) | 2 blocks, free | −31% | −2.85 | −0.32 / −0.06 / +1.45 | 3.3 ns |
| | 3 blocks, menu | −26% | −1.61 | +0.72 / +1.60 / +6.74 | 0.8 ns |
| Text 50% (0.910, 0.828) | 2 blocks, free | −8% | −1.26 | +0.77 / +1.63 / +6.64 | 0.7 ns |
| | 3 blocks, menu | −6% | +0.58 | +2.56 / +4.56 / +16.18 | 0.3 ns |

Reading.

1. **In time, coding three or four blocks at the first level does not pay at any cost the sources allow.** It would
   pay only below 0.3 to 1.1 ns per block-rise; the decode alone is 1.4 to 2.8 ns and the trial's all-in figure 3 to
   10 ns. The reason is in the formula: every item entering the level pays for the extra blocks, and only the
   fraction that is then decided is saved, at about 8 ns plus its own three rises each.
2. Two blocks at free rates is the one alternative near the margin: it pays below 1.5 to 3.3 ns at 1% answers and
   below 0.7 to 1.5 ns at 50%. With menu rates it gains 3% of width and pays nowhere.
3. **In bytes, the alternatives gain a little: two or three blocks at free rates save 0.2 to 2.9 B per item; at menu
   rates the change is between −1.6 and +3.9 B.** That is the case for a disk-resident store only.
4. **The model overstates the gain itself.** The trial measured 2% fewer items entering 96 B with three blocks; the
   model gives 13 to 21% for item queries and 6 to 26% for text queries. With the trial's 2% the
   formula gives +3.3, +5.5 and +18 ns per item at item 50% for τ = 1.8, 3 and 10, that is +5%, +8% and +25% of
   71.3 ms: the trial's 8 to 24% is what the cost model returns for τ between 3 and 10 ns. So the cost side of this
   section agrees with the trial, and the width side of my model does not. The likely reason is the one in C3 (c)
   item 4: the model's block norms after the prefix are not the index's, whose block 0 is about 1.5 times heavier,
   so moving bits from block 0 to the others gains less there.

**Changes C3 (c) item 1.** "The 56 B level is the far point" stays true of the width at equal bytes. The
recommendation that went with it, to code every block at that level, is **withdrawn for the in-memory index**: on
time it loses at every scalar cost consistent with the sources, and the native trial already showed it. It stands
only as a statement about bytes read, and even there the measured gain (2% of the entrants to 96 B, against 3 B more
per entrant to 56 B) is a loss of about 2 B per entrant on the trial's figures.

### C5.3 The layout programme with the scalars charged [model; `c5b.py`, `c5b.log`]

The programme of C3b.1 with two changes. A coded block costs s on top of its code, and D costs 0.5 B for its norm
code: code + s·J + 0.5·[D not empty] ≤ B. The constant follows the number of coded blocks: c1(d), the exact one-block
quantile (6.43, 6.93, 7.20, 7.64, 7.95 at d = 63, 95, 127, 255, 767), for J = 1 and 7.963 for J ≥ 2 (certified for the
built layout; used for every other layout as an estimate). In bytes s = 2.5 B (1 B of |y|, 1.5 B of κ and tan φ). The
time objective is τ_0 + τ_byte·code + τ·J per item entering the level, so with τ_byte = 0.133 ns the same programme
with s = τ/τ_byte is the time objective in bytes of decode: s = 11 to 21 for the decode alone (1.4 to 2.8 ns) and 23
to 75 for the trial's all-in 3 to 10 ns. Group 12, built curves.

The first randomised level: the budget is the built level's own cost, 56 B of code and s for its one block.

| s | 0 | 1.5 | 2.5 | 5 | 8 | 11 | 14 | 23 | 45 |
|---|---|---|---|---|---|---|---|---|---|
| No block (deterministic) | 0.214 | 0.209 | 0.206 | 0.198 | 0.189 | 0.180 | 0.172 | 0.151 | 0.113 |
| One block | 0.1755 | 0.1755 | 0.1755 | 0.1755 | 0.1755 | 0.1755 | 0.1755 | 0.1755 | 0.1755 |
| Two blocks | 0.168 | 0.174 | 0.178 | 0.189 | 0.203 | 0.218 | 0.235 | 0.299 | 0.632 |
| Three blocks | 0.158 | 0.169 | 0.177 | 0.201 | 0.236 | 0.283 | 0.343 | 0.716 | |
| Four blocks | 0.155 | 0.172 | 0.185 | 0.226 | 0.295 | 0.411 | 0.645 | | |
| Best | four | three | one | one | one | one | none | none | none |

1. **With the scalars charged at what the sources say, the optimum codes one block at the first randomised level, as
   the index does.** Every block is best only below about 0.7 B per coded block; three blocks up to about 2.3 B; one
   block from there to about 12.7 B-equivalents (1.7 ns); above that, no block. At 2.5 B the three options are
   within 1.5% of each other. Half of this is the constant: with c fixed, as in C3, three blocks still won at 2 B.
2. The one block the programme chooses is not the built one. It is 256 positions at 1 bit after a prefix of 188 bits,
   0.1755·F, where the built level (96 positions at 2 bits after 256 bits) is 0.2405·F in the same model.
   `c5e.py` keeps the built 256-bit prefix and the 24 B of code and only widens the block: 192 positions at 1 bit give
   0.194 (−19%), 224 at 0.86 bit 0.192 (−20%), 256 at ¾ bit 0.192. With the norms rescaled so that block 0 holds 48%
   of the sum after the prefix, as the dump's level-0 means suggest (the model has 38%), the same three give −14%,
   −14% and −13%. **This is the candidate that survives the scalars: it has one block-rise, the same code bytes and
   the same scalars as the built level.** See the caution in C5.2 item 4: the model overstated the trial's gain by
   several times, so I expect a few percent, not 14.
3. **In time the first rotated block is itself at the margin.** At the decode-alone cost (s = 11 to 21) coding no
   block is between 3% wider and 11% narrower than coding one; at the all-in cost the programme prefers to spend the level's time on more
   deterministic bytes (0.151 against 0.1755 at s = 23). I do not put this forward as a design [estimate]: it uses
   1.19·W at 556 to 1,044 bits, beyond the 296 at which it was measured, and assumes a byte of deterministic code
   costs the mean decode cost and no further level. What it does establish is the scale: a block-rise costs 1.4 to
   10 ns against 3.2 ns for decoding the whole 24 B of the level's code.
4. Later levels in stored bytes (2.5 B per coded block; budgets 104.9, 174.8, 274.3 B): three, four and two blocks,
   0.0726, 0.0282 and 0.0100 of F. Charging scalars in bytes does not reduce the number of blocks there.

### C5.4 A third curve: the shrunk-centre interval w* [carried from `CLAIMS.md` claim A4, proved there and unrefereed; I did not check its proof]

For one block with A = ‖y‖‖Δ‖ and k = c²·tan²φ/d, the plain rule's half-width is A·√k and the shrunk-centre
interval's is w* = A·√(k/(1 + k)), with the same stored message and no second event. It changes nothing from 2 bits
up and about 30% at a quarter of a bit.

**What it does to the hand-over (C3 (a)).** w* < A at every rate, so a coded block is never wider than its own
Cauchy–Schwarz bound: **B₀ of C3a.1 is a property of the plain rule and disappears under w*.** For a flat block at
the sphere bound, w*/(F·2^(−R)) = q·2^R/√(4^R − 1 + q²) with q² = c²/d, which is below 1 at every R > 0 exactly when
d > c². So under w* a flat block of more than c² degrees of freedom beats the same bits spent deterministically
from the first bit (B* = 0 in C3a.2 item 1), and one of fewer never does. The ratio at ⅛, ¼, ½, 1 and 2 bits: 0.93,
0.88, 0.82, 0.76, 0.72 at d = 95; 0.81, 0.71, 0.61, 0.53, 0.49 at d = 255; 0.60, 0.49, 0.39, 0.33, 0.30 at d = 767
[computed, `c5c.log`, with c1(d)]. The condition d·(GM/AM)² > c² at high rate is unchanged. **Changes C3a.1 and
C3a.2 item 1**: the threshold in bits belongs to the plain rule; what remains under w* is the condition on d, the
flatness of the block, and the scalars.

**Several blocks: not proved.** Two readings, to bracket it.
- S1, per term, as the PI asked: each coded block contributes its own w*_j² and the terms add, h² = Σ_j A_j²·k_j/(1 +
  k_j). [approximation, unproved.] It is optimistic at low rate: as R_j → 0 it puts a block's whole Cauchy–Schwarz
  term in quadrature, which no interval can do, because nothing random is left in that block.
- S2, a construction: shrink block j's centre by m_j, add the biases linearly and the random parts in quadrature
  under the several-block constant, take the worst case over each block's s_j ∈ [0, 1] and the best m. For one block
  it is w*. [sketch: gap named. It rests on A4's decomposition block by block and on the several-block constant
  holding for every vector of scales (R1); I did not simulate it.] A joint treatment of the blocks' biases could lie
  between S2 and S1.

**The first randomised level under the three curves** (`c5c.py`, `c5c.log`; built prefix and cuts, 192 code bits,
constant by number of blocks; "pays below" is the cost of a block-rise under which the alternative saves time, over
the four cells of C5.2; bytes are per item of N at item 1%, item 50%, text 1%, text 50%):

| Curve | Built level h/F | Three blocks at 1, ½, ⅛ | pays below | bytes | Four blocks at ½, ½, ⅛, ⅛ | pays below |
|---|---|---|---|---|---|---|
| Plain rule, built code | 0.2405 | −14.9% | 0.3 to 0.8 ns | −0.1, −0.6, −1.6, +0.6 | −1.4% | 0.04 ns |
| Plain rule, ideal code | 0.1936 | −38.8% | 1.4 to 2.4 ns | −0.4, −3.7, −3.6, −5.3 | −30.7% | 0.5 to 0.9 ns |
| w* at the built angle, S1 | 0.2394 | −43.0% | 1.4 to 3.9 ns | −0.7, −5.1, −6.2, −5.8 | −37.9% | 0.6 to 1.5 ns |
| w* at the built angle, S2 | 0.2394 | −30.0% | 0.7 to 2.1 ns | −0.4, −3.0, −4.3, −2.4 | −20.0% | 0.2 to 0.7 ns |

Two blocks under w*: at 0.96 and 0.78 bit, −26.5% (S1) and −22.6% (S2), paying below 1.5 to 10.3 ns and 1.1 to
5.9 ns; at menu rates (½ and 1 bit) −21.6% and −15.8%, paying below 1.0 to 5.2 ns and 0.6 to 2.6 ns. One wide block
(C5.3 item 2) under w*: 192 positions at 1 bit −22%, 256 at ¾ bit −26%; −17% and −19% with the dump-like norms.

**Does a first level that codes every block at a fraction of a bit become worth its scalars under w*?**

- **In bytes, yes, on the model's widths**: three blocks save 0.4 to 6 B per item where the plain rule with the built
  code saved at most 1.6 B, because w* removes the penalty of the built code below one bit (at ¼ bit and
  the built code's angle the plain rule is wider than Cauchy–Schwarz, and w* is 14% below it at d = 95 and 25% at
  d = 255).
- **In time, no for every block, and only at the edge for three.** Four blocks pay below 0.2 to 1.5 ns, under the
  decode-alone cost in every cell. Three blocks pay below 0.7 to 2.1 ns (S2) or 1.4 to 3.9 ns (S1). On S2
  that is the lower end of the decode-alone range (1.4 to 2.8 ns) at 1% answers (1.4 and 2.1 ns) and under it at 50%
  (1.2 and 0.7 ns); on S1 it is inside the range in three cells. Both are below the trial's all-in 3 to 10 ns
  everywhere except text 1% under the optimistic reading.
- Two blocks at about 1 and ¾ bit is the first multi-block layout that could pay in time under w* (below 1.1 to
  5.9 ns on the conservative reading), at 1% answers more than at 50%.
- The cheapest use of w* is the one that adds no scalar: one wide block at ¾ to 1 bit.
- All of it carries C5.2 item 4: the same model gave 6 to 26% where the trial measured 2%.

### C5.5 C3 (c)'s table and C4.3's split with the scalars counted [model; `c5d.py`, `c5d.log`]

**C3 (c), randomised levels, c = 8 built column.** Schedule over the optimum at equal cost, with the stored bytes of
the build log, 2.5 B per coded block for the optimum, and the constant by number of coded blocks. The schedule's
widths in the model become 0.2405, 0.1155, 0.0384, 0.0169 (the first was 0.2456 with c = 8).

| Level | As C3 (c) | Stored bytes, source scalars | Time-equivalent, 14 B per block-rise (1.9 ns) | 45 B (6 ns) |
|---|---|---|---|---|
| 56 B | 1.68 | 1.56 | 1.40 | 2.13 |
| 96 B | 1.71 | 1.59 | 2.42 | 7.3 |
| 160 B | 1.40 | 1.36 | 2.88 | not credible |
| 256 B | 1.81 | 1.68 | 5.15 | not credible |

- In bytes the table moves by 3 to 8%: the scalars were already charged there, and the correction is the stored
  sizes and the constant.
- The time-equivalent columns charge the schedule the block-rises an item has paid by each level (1, 4, 7, 11) and
  compare with a layout read in one piece, which pays one or none. They are not a comparison of schedules: nesting
  is what lets an item stop early, and the one-piece optimum would make every item pay everything. They also use
  1.19·W far beyond its measured range (prefixes of 588 to 4,455 bits). **What they show is where the time goes: by
  256 B an item has paid 11 block-rises, 15 to 110 ns, against 34 ns to decode its 253 B of code.** The direction is
  fewer rises per item, not more blocks per level; the codes report's rule that a block whose last rise is its only
  rise can use a one-rate code points the same way.

**C4.3's split in bytes**, same five cells, corrected stored sizes and constant:

| Cell | Total (was) | Floor | Level spacing | Layout | Code distance | Scalars and key |
|---|---|---|---|---|---|---|
| Item 0.01% | 2.0 (2.0) | 28% | 23% | 4% | 20% | 25% |
| Item 1% | 17.4 (17.6) | 32% | 18% | 13% | 24% | 14% |
| Item 50% | 130.3 (131.7) | 29% | 21% | 16% | 27% | 8% |
| Text 1% | 70.7 (72.2) | 33% | 14% | 19% | 22% | 12% |
| Text 50% | 296.7 (299.0) | 27% | 24% | 15% | 29% | 5% |

Norm codes and scalars alone are 5 to 9% of the bytes (1.4, 8.0, 6.5, 14.0 B in the last four cells). The bytes split
stands.

**The same cells in time.** Per item of N, the work the breakdown calls decode, bounds, re-pack and full vectors,
as entrants × (3.2 ns per level + 0.133 ns per code byte + τ per block-rise + 0.7 ns per |y|) + 110 ns per full
vector, at τ = 1.8 / 3 / 10 ns:

| Cell | ns per item | Levels | Decode | Scalars | Full vectors | Measured: scalar decode, share of the profiled wall |
|---|---|---|---|---|---|---|
| Item 1% | 7.8 / 8.1 / 9.9 | 69 / 66 / 54% | 20 / 19 / 16% | 8 / 12 / 27% | 3% | 4.5% |
| Item 50% | 38 / 42 / 60 | 49 / 45 / 31% | 25 / 23 / 16% | 17 / 23 / 47% | 9 / 8 / 6% | 12.3% |
| Text 1% | 30 / 32 / 43 | 60 / 57 / 42% | 23 / 22 / 16% | 14 / 19 / 41% | 2% | 8.8% |
| Text 50% | 69 / 76 / 117 | 39 / 35 / 23% | 27 / 25 / 16% | 19 / 27 / 52% | 15 / 13 / 9% | 12.4% |

At text 50% the model's 13 ns of scalars at τ = 1.8 stands beside 11.6 ms measured for a million items, and its 69 ns
beside about 81 ms of measured per-item phases; the other cells inherit C4.2's errors in the entrant counts.

**Changes C4.3.** "Scalars and key 5 to 25%" is a statement about bytes. In time the scalars are 14 to 19% of the
per-item work at large answers at the decode-alone cost, and a quarter to a half at the all-in cost. And "level
spacing" changes sign: in bytes more levels read less; in time each level entered costs about 3.2 ns and its
rises, the largest component in every cell. `rust/REPORT_RAND.md` measured exactly this: 14 levels down to 4 was
faster at every cell for 3 to 11% more bytes. C4.3's remark that more levels would lower the spacing loss "at the
cost of their scalars" understated it.

### C5.6 What changes in C3 and C4, in one place

| Where | As written | Now |
|---|---|---|
| C3a.1, C3a.2 item 1 | a block needs B₀ bits to beat its Cauchy–Schwarz bound, B* to beat the same bits spent deterministically | true of the plain rule; under w* (unrefereed) both are zero for a flat block with d > c² |
| C3 model, section 0 | c fixed | 6.931 with one block coded, 7.963 with several: a second block costs 15% of the random term |
| C3b.3 item 3 | the optimum codes every coordinate from the first rotated bit | only if scalars are free; at 2.5 B per coded block the optimum codes one block at the first level, in time one or none |
| C3 (c) item 1 | the 56 B level should code everything; a sub-bit code would pay most there | withdrawn for the in-memory index; in bytes only, and the trial's own figures show a loss there too. The surviving candidate is one wider block at the same cost |
| C3 (c) table | 1.68, 1.71, 1.40, 1.81 at 56 to 256 B | 1.56, 1.59, 1.36, 1.68 in stored bytes; in time the gap is the rises of a nested schedule |
| C4.2, C4.3 stored bytes | 27, 36, 65.5, 107.5 B at four levels | 26.5, 35, 64.5, 104.9 B; totals move by under 2% |
| C4.3 | scalars 5 to 25% of the cost | of the bytes; 8 to 52% of the per-item time depending on the cell and the cost of a block-rise |
| C4.3 | more levels lower the spacing loss | in bytes; in time each level costs 3.2 ns per entrant and its rises |

Unchanged: C3a.2 items 2 and 3 and C3a.3, the hand-over at 32 to 40 B with built curves at c = 8 (C5.3 item 3 moves
it later, if anywhere), the identity (b.1), the independence of the layout from c once D is empty, C4.1 and C4.4.

### C5.7 What a native test of the 56-byte level must measure

The earlier trial lost on time and reported one count. For the model to be believed, or dropped, a test needs the
following, on one index, the same queries and one binary that differs only in the first level's layout.

**Variants.** V0 as built (block 0 at 2 bits). V1 one wider block at the same 24 B (first cut at 192 positions,
1 bit): the cost-neutral candidate. V2 two blocks (about 1 and ½ to ¾ bit). V3 the `--design-exact` layout, to
reproduce the earlier loss with the phases recorded. If the one-block shrunk-centre interval is implemented, each
with and without it; it must be refereed before it decides anything.

**Per variant and cell** (item and text, answers 0.01%, 1%, 10%, 50%):
1. **Items entering each of the ten levels, and full vectors.** These are exact. The model says only the count
   entering 96 B moves; the counts entering 160 B and 256 B and the full vectors must stay within the effect of the
   changed later rates. This is the test of C5.2's formula.
2. **The level's half-width and its parts**, from a verify dump: the random term, the Cauchy–Schwarz term of the
   uncoded blocks, and each block's own Cauchy–Schwarz term (R_j) for the items entering 56 B. The last gives the
   index's block norms, which is what the model lacks; the predicted width change must be recomputed from them
   before it is compared.
3. **Bytes per item, split into code, scalars and full vectors.**
4. **Time by phase**, profiled and unprofiled: level decode; bounds and decisions with the scalar decode apart;
   re-pack and compaction; tables; full vectors. **And the number of block-rises decoded per query**, which the
   breakdown does not print today. Then the cost of a block-rise is measured, not inferred: the difference in
   scalars, bounds, compaction and tables between V3 and V0 over the difference in block-rises. The model needs it
   between 1.4 and 10 ns; the test fixes it.
5. The time of the 96 B level per entrant, from the same phases, to value an item that no longer enters it.

**What would make the model believed.** (a) Only the 96 B count moves. (b) Its change is within a factor 1.5 of the
prediction recomputed from the measured block norms; the earlier trial's 2% against my 6 to 26% fails this, so the
model is not believed today. (c) The measured time difference equals e_56·ΔJ·τ − Δe_96·T_96 with the measured τ and
T_96, to within the run-to-run noise.

**What the test cannot do on a shared machine.** For V1 the predicted time gain is Δe_96·T_96, about 1 ns per item at
item 50% in the model, 1.3% of the query, against ±5% noise. An end-to-end timing will not resolve it. The verdict on
V1 has to come from the counts, which are exact, multiplied by per-unit costs taken from the phases; V3 against V0
is large enough (8 to 24%) to time directly.

---

## What failed or is not established

- C3 (a): nothing failed. The PI's c²π/2 is confirmed as the sign sketch's limit against the zero-bit bound; it is
  not the hand-over criterion, which is (a.1).
- C3 (b): the search is local (contiguous blocks, J ≤ 4, a grid of cuts, a tail in D). No lower bound on h*(B) over
  all layouts was found, and a continuous relaxation does not give one: a block of unequal amplitudes can be better
  than per-coordinate rates (C3b.1 item 3).
- C3 (b), built curves: the deterministic factor 1.19 is used beyond the 296 bits at which it was measured, and its
  ramp from 1 is an estimate. The long prefixes of the built-curve optimum depend on both.
- C3 (c): the model's block norms after the prefix do not match the dump's, so the loss assigned to the built rates
  at 96 to 256 B is an upper bound, not a finding.
- C3 at c = 4 is not the approximate mode (C3b.4 item 4).
- C4: the Gaussian, one-scale form under-predicts in-distribution queries at 10% and 50% by a factor of 2 and cannot
  be calibrated on the candidate share. Text queries fit only with one fitted scale.
- c is taken as fixed. Its dependence on the block layout (6.93 for one block of 96, 7.96 for four, at N = 2²⁰) was
  not optimised over; it favours the first level by 13%.
- C5: the cost of a block-rise is inferred, 1.4 to 2.8 ns for the decode and 3 to 10 ns all-in; the entrant counts it
  rests on are not printed by the breakdown. The several-block form of w* is unproved (S1 an approximation, S2 a
  sketch). The model's fall in entrants for the trial's layout is 6 to 26% where 2% was measured.
- C5.3 item 3 and the time-equivalent columns of C5.5 extrapolate 1.19·W to thousands of prefix bits.
- Only groups 12 and 6 of one set of spectra (a 200k subset, PCA basis) were solved. `spec2_v2.json` has the same
  values for the groups I read; `spec2_n128.json` (n = 128) was not used.

## Files

All in `notes/program/confidence/`.

| File | What |
|---|---|
| `DESIGN-CLAIMS.md`, `DESIGN-NOTEBOOK.md` | this file and the dated log |
| `design_lib.py` | spectra, water-filling, the ideal and built curves |
| `c3a.py`, `c3a.log` | C3 (a): break-even bits |
| `c3b.py`, `c3b_chain.sh`, `c3b_g{12,6}_c{8,4}_{ideal,built}.json` and `.log` | C3 (b): the frontier and its layouts |
| `c3_tables.py`, `c3_tables.log` | compact tables of the frontiers, with Σ n cos²φ against the slope |
| `c3c.py`, `c3c_g12_c8`, `c3c_g12_c4`, `c3c_g6_c8`, `c3c_g6_c4` `.json` and `.log` | C3 (c): the built schedule in the model |
| `c4.py`, `c4_g12_c8`, `c4_g6_c8` `.json` and `.log` | C4: Gaussian scores |
| `c4b.py`, `c4b_g12_c8.log` | C4: text queries with the measured shells |
| `c4_dump_test.py` | C4.4: the test, not run on the dump |
| `c5a.py`, `c5a.log`, `c5a.json` | C5.2: the constant by block count; first-level alternatives in bytes and time |
| `c5b.py`, `c5b.log`, `c5b.json` | C5.3: the layout programme with a charge per coded block |
| `c5c.py`, `c5c.log`, `c5c.json` | C5.4: the three curves, with the shrunk-centre interval |
| `c5d.py`, `c5d.log` | C5.5: C3 (c)'s table and C4.3's split redone, in bytes and in time |
| `c5e.py`, `c5e.log` | C5.3 item 2: one wider block at the built level's cost |
