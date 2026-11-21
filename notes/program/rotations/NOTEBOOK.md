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

## 11:15 One round, exact (`b1_oneround.py`)

y = e_a + e_b: E = (A/√2)(d₀e₀ − d_a d_b d_s e_s) on every draw, depths 1 to 3. With a = 0 the query (e₀ − e_b)/√2 gets
±√(n − 1) sd, each with probability ½. Sparse items with k random values: the whole of ‖E‖² sits on the xor-span
(share outside: 0 to machine precision); failure rates 1, 1, 0.06, 8·10⁻⁴ at k = 2, 3, 4, 5. Saved as B0 to B2.

## 11:25 Two rounds (`b2_rounds.py`)

- Half-structured attempt (only the item's first round conditioned): rate at 7.8 sd given the structure 5·10⁻⁶ at
  k = 2, nothing at k ≥ 3. The 2^(r/2) guess was wrong. Stopped that run early; its output is kept as `out_b2c.txt`.
- Unconditional, 2·10⁶ draws: the pair (1 on 0..7, 1 on 8..15) fails at 1.5·10⁻⁶; e₁ against e₂ and a Gaussian pair
  look like the law to 4 sd.
- Worked out why: a structured round maps signed coset indicators to signed coset indicators (B3a), and the last
  round on a coset of size t is a t-dimensional problem (B3b). The tie rule's zeros cancel on a proper coset.
  Bookkeeping of which rounds can be shared by y and Δ: after a shared structured round they must separate (they are
  orthogonal on one coset, so their characters differ off G^⊥), and coming back together costs a second structured
  event with one admissible class. Hence the alternation of p_G and p_V in B3.

## 11:40 Structured rounds measured (`b3_structured.py`)

Matches B3b: mean square (n − 1)/(t − 1) at one round. Lower bounds: 2 rounds 2^(−18) at n = 256, 2^(−30) at 512 and
1,024; 3 rounds 2^(−41) at 256 and 2^(−57) at 512; 4 rounds 2^(−58) at 256; 5 rounds not reached. Small blocks
(n ≤ 64) not reached at all: the bound there is near Cauchy–Schwarz anyway.

## 11:45 Butterfly and Kac (`d_butterfly.py`)

Butterfly on (e₀, e₁): the error is a function of z (D2), rms 3.7 to 4.1 sd at n = 256, no draw above 7.8 sd because
tan φ is large on such uneven vectors. Kac at n = 64: nothing visible at 20,000 draws; only the atom's floor (D1).
A first idea, that an untouched coordinate is itself a failure of the certificate, did not hold: the built code gives
a 1-sparse z a huge tan φ through the tie rule, so its spread is large.

## 11:50 Rounding (`e_rounding.py`), sub-blocks (`f_subblocks.py`), unitary (`h_unitary.py`)

- f32 free; int16 below 0.3% to 4 bits; int8 8 to 13% at one bit at the large blocks. Clipping int8 is worse. The
  Frobenius bound is 4 to 14 times the operator norm. f32 accumulation needs the blocked bound.
- The kernel already has the slot (`ewj`), an L1 allowance on the rotated weights' f32 cast.
- Sub-blocks: the law is right (F1), but with one κ the radial mismatch between sub-blocks is a deterministic term
  of about √k/c of the width. I had first compared it with c·A/√b and thought it small; against the right spread,
  c·A/√n, it is 12% at two sub-blocks. Measured 1.12 to 2.0. Letting the encoder equalise the gains recovers part.
- U(n/2): the complex form of the lemma checks out numerically; with a real κ the Iy term is 10% of the width.
- General principle behind all of these: H1, the stabiliser's orbits. One trade for all of them: n²/s stored numbers
  for s scalars.

## 12:05 Dither (`g_dither.py`, `g_check.py`) and the sub-block constant (`f_constant.py`)

- G1 written out for the nested code: one offset per position, uniform over the coarsest cell that position ever
  uses, gives an exactly uniform error at every depth; the range must then be 1/(1 − 2^(−d₁)) times the largest
  coordinate. Checked on a grid of 2²⁰ dither values.
- A step chosen with sight of the dither breaks the law by up to the number of candidates; exact example with two
  steps (factor 2). First draft of that example used an irrational ratio and an equidistribution argument; replaced by
  an exact one (a = s/2 = s′/4 at depth 2).
- Constants: equal weights look worst (seven weightings tried by saddlepoint); exact Irwin–Hall quantiles agree with
  the saddlepoint to three decimals. Proved: sub-Gaussian with the true variance, and that Chernoff's bound is worst
  at equal weights (concavity of ln(sinh √v/√v)). Not proved: that the exact quantile is worst at equal weights.
  A first run of the saddlepoint overflowed in sinh; fixed with the stable form.
- Widths: 2.0 to 2.65 times today's when nested from one bit. The PI's 1.3 to 1.45 is the not-nested figure at 2 to 3
  bits. Clipping with a deterministic remainder and sub-bit rates do not help; at sub-bit rates the first sub-bit
  figures looked better than today's at n = 64 until compared with Cauchy–Schwarz, which both exceed there.
- R4 reproduces chapter 3's 8.815 at n = 768; halving the blocks at n = 2048 moves R4 by 0.008.
- `haar_rotation` does multiply Q by sign(diag R), so the built matrix is Haar in exact arithmetic (Mezzadri's
  correction is in place).

## 12:20 A correction to my own B4

The first B4 table compared Pr[|error| > 7.8 sd] with the budget 2^(−60.8). That is unfair to the rotation: at 7.8 sd
the law itself allows 2^(−53) at d = 255 and 2^(−50) at d = 511, so four rounds at 256 and three at 512 were not
violations at that threshold. Re-run at each block's certified constant (8.282, 8.589, 8.748; `b4_at_budget.py`):
two rounds 2^(−19), 2^(−20.5), 2^(−32) at 256, 512, 1,024; three rounds 2^(−41) and 2^(−57); four rounds 2^(−58.4) at 256.
The conclusions stand, with factors 5 (four rounds, 256) and 14 (three rounds, 512) over the budget. Table replaced.

## Open at the end of this session

- H6: an exact pair-uniform ensemble with o(n²) parameters, or a proof there is none.
- G5 (v): the exact worst case over weights for a sum of uniforms (conjectured: equal weights).
- B: whether five rounds at n = 256, three at 1,024, or any number of rounds under an incoherence assumption satisfy a
  tail bound. No evidence either way beyond the absence of a proof.
- H4: an encoder that nulls β for the unitary form (untested).
- G10: a lattice dither with a block-norm range.
- Nothing here was timed; the split of the 1 ms between bytes read and multiplications is the owner's statement.
- Note for the PI: `git status` shows CLAIMS.md, NOTEBOOK.md, lib.py, b1_oneround.py and out_b1.txt as already
  tracked, so an early state of them was committed by someone else during the session. I ran no git command that
  changes anything. The current files are the ones to read.

## 11:20 Librarian's report read (`../library/rotations.md`) (entered here out of time order)

Used as the source for every literature statement in `CLAIMS.md`; I searched nothing myself. Points taken: the
reduction of its section 0 (checked, and sharpened in claim A1: for *every codebook* the pair law must be exactly
uniform, and for a bound at δ a density ratio suffices); Low's 1/S floor; the two-round fact |U₀| = |U₁|; the
classification recalled for groups transitive on pairs; the dither hypotheses.
