# Rotations track: notebook

Rotations theorist, 21 November 2026. The owner's question: can the certified bound of the randomised levels be
re-derived for a rotation cheaper than a dense Haar matrix? Claims are in `CLAIMS.md`; this file is the dated log.
Numerics are small numpy jobs run through `../run-limited.sh 6 1 2`. Nothing was timed.

## 11:00 Reading

- `notes/README.md`, `00-notation.md`, chapter 3 sections 3.1 to 3.3 and 3.7 to 3.9, `SYSTEM.md` section 2.
- Code facts used below (`rust/src/v2/mod.rs`, `q.rs`, `util.rs`):
  - `haar_rotation(m, SEED, j)`: Gaussian matrix, Householder QR, f64, one matrix per block, shared by every group.
  - Build: `ymat = wmat · rotsᵀ` by `dgemm` in f64, so z = R·y with R the stored row-major matrix.
  - `rand_encode(v, depth)`: cell = clamp(⌊v·2^(depth−1)⌋ + 2^(depth−1), 0, 2^depth − 1); the decoded value is the
    cell's midpoint, (cell + ½)·2^(1−depth) − 1. So v = 0 lies in the cell [0, 2^(1−depth)) and decodes to
    +2^(−depth): the tie rule sends an exact zero up. Values at or beyond ±1 clamp (overload).
  - `rand_choose`: v = y_p/(λ·rms), λ from LAMS = (2, 2.3, 2.6, 3, 3.5, 4.2), chosen per item and block to minimise
    the sum over the block's rises of ln tan²φ; first minimum wins.
  - κ = ‖y‖²/⟨ō, y⟩ and tan φ per rise; ‖y‖ once.
  - Query (`group_q2r`, `setup_groups`): rotated weights by `matvec64` or `dgemm` against the f64 `rots`, then **cast
    to f32** (`wq[n + …] = y[a] as f32`). So the query side already carries a rounding of the rotated weights at
    relative 2⁻²⁴ per coordinate.
- Sizes at n = 2048: blocks 256, 336, 688, 768, Σ n_j² = 1,241,600 entries, 9.93 MB in f64.

## 11:10 First pass of the mathematics (before any numerics)

- What the law needs, exactly: for every codebook, iff the pair (Ra, Rb) is uniform on the Stiefel manifold V₂ for
  every orthonormal (a, b) (claim A1). The PI's condition (invariance under R → R·Oᵀ) is sufficient, not necessary.
- Any closed subgroup G of O(n) with its Haar measure gives a conditional law of the error invariant under the
  stabiliser G_y (claim H1). This one lemma covers Haar on O(n), sub-blocks, the unitary and the symplectic group, and
  shows why a finite group (sign-Hadamard products) leaves nothing random.
- One round H·D: the PI's example works and is stronger than stated when one of the two coordinates has index 0.
  With y = e_a + e_b the error vector is exactly d₀e₀ − d_a d_b d_s e_s, s = a xor b.
- Two rounds: a first attempt predicted a width ratio 2^(r/2) for subgroup-indicator data; that was wrong. Redone:
  conditional on a structured first round (probability 2^(r+1−2^r)) the error is sd·g·g′, a product of two
  normalised sums, so its tail is exp(−c), not exp(−c²/2). To be measured.
- Dither: the PI's 1.3 to 1.45 looks too low; the grid must cover the largest coordinate and with a nested code the
  dither must be uniform over the coarsest cell, which doubles the range needed at depth one. To be computed.

## 11:20 Librarian's report read (`../library/rotations.md`)

Used as the source for every literature statement in `CLAIMS.md`; I searched nothing myself. Points taken: the
reduction of its section 0 (checked, and sharpened in claim A1: for *every codebook* the pair law must be exactly
uniform, and for a bound at δ a density ratio suffices); Low's 1/S floor; the two-round fact |U₀| = |U₁|; the
classification recalled for groups transitive on pairs; the dither hypotheses.
