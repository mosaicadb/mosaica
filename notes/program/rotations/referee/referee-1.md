# Referee 1: rotations track claims

Independent referee, 21 November 2026. Verdicts are appended as each claim is checked. "No error found" means a real
attempt failed to break it, not that it is correct. Scripts and outputs are beside this file (`ref_*.py`, `ref_*.txt`).
NOTEBOOK.md was not read.

## E1, E2 (a rounded copy of the rotation on the query side)

Scripts: `ref_e.py` → `ref_e.txt`; `ref_e_build.py` → `ref_e_build.txt`. Code read: `rust/src/v2/q.rs` (`group_q2r`,
`setup_groups`, `cluster_q2w`, the level kernel near line 1184, `RandLv` near 1511), `rust/src/v2/mod.rs`
(`encode_cluster`, `cluster_info`, `rand_choose`), `rust/src/util.rs` (`haar_rotation`).

**E1, the inequality: no error found.** κ‖ō‖ = ‖ŷ‖ = ‖y‖ sec φ is exact, the split x̃ − RΔ = (x̃ − R̃Δ) + (R̃ − R)Δ is
exact, and the bound is pointwise in ω, so it may use a ρ computed from the realised matrices. The error law is
untouched and the constant is unchanged, because nothing the lemma conditions on (ō, κ, tan φ, ‖y‖) has moved. I attacked
it with the worst query for each item (Δ ∝ (R̃ − R)ᵀō): the change of the estimate reached 0.52 to 0.58 of the allowance
and never exceeded it (300 items, n = 256 and 768, depths 1, 4, 8, f32 and int16). So the allowance is a true worst case
and is within a factor 2 of attained.

**E2, the numbers: reproduced** on my own Haar matrices and my own copy of the code. ρ (singular value): f32 5.10·10⁻⁸
(n = 256) and 5.15·10⁻⁸ (768); int16 7.4·10⁻⁵ and 8.2·10⁻⁵ (the int16 figure moves ±20% with the draw's largest entry;
the claim's 8.9·10⁻⁵ at 256 is one draw). Allowance as a share of c·sd, c = 7.8, at the rms tan φ, depths 1 / 4 / 8:

| | f32 | int16 |
|---|---|---|
| n = 256 | 0.000017% / 0.00010% / 0.0014% | 0.025% / 0.145% / 2.0% |
| n = 768 | 0.000030% / 0.00017% / 0.0022% | 0.049% / 0.27% / 3.6% |

These agree with the claim's table (0.0014%, 0.0024%; 0.030 / 0.18 / 2.4 and 0.049 / 0.28 / 3.9).

Repairs and unstated hypotheses, in order of weight.

1. **The slot form is short by the factor (1 + dk) [repair needed before int16 is built; harmless for f32].** E1 is
   stated with the true κ. The kernel's slot is `hpar += kap·ekw` with `kap` the *decoded* κ, and the κ-rounding term
   is `dk·|eb|` with `eb` the *computed* block estimate. Decompose κ_dec⟨ō, x̃⟩ − κ⟨ō, RΔ⟩ = (κ_dec − κ)⟨ō, x̃⟩ +
   κ⟨ō, x̃ − RΔ⟩: the first term is what `dk·|eb|` covers, the second needs the true κ ≤ κ_dec·(1 + dk). So
   √n·ρ·‖Δ‖ in the `ewj` slot undercounts by up to dk times itself (dk is up to 0.05 times a low quantile of tan φ
   for a 4-bit κ, and 0.0014 or more for an 8-bit one). With f32 the shortfall is about 6·10⁻⁹·tan φ·‖y‖‖Δ‖ at
   depth 1, and the kernel's (1 + 10⁻⁶) on the half-width is worth about 2.8·10⁻⁷·tan φ·‖y‖‖Δ‖ (d = 767), so it is
   covered. With int16 it is about 10⁻⁵·tan φ·‖y‖‖Δ‖ and is not. Repair: multiply the slot's term by (1 + dk_j), or
   add E1's own form ‖y_j‖ sec φ_j·ρ_j·‖Δ_j‖ from the stored scalars (no decoded κ in it, so no second-order term).
   Side observation, not checked: today's `ewj` term has the same shape (dk·kap·ewj is not added), and by the same
   estimate it sits inside the 10⁻⁶ only while ‖x_j‖ is within about ten times ‖Δ_j‖. I did not trace how the
   kernel's own notes justify it.
2. **The recommendation's int16 figures are for E1's form, not for the slot form.** The slot form is √n/‖ō‖ times
   larger: 2.0 at depth 1, median 2.6 (max 3.6) at depth 4, median 3.0 to 3.5 (max 4.2) at depth 8 (`ref_e.txt`). So
   "under 0.3% to 4 bits, 2.4 to 3.9% at 8 bits" becomes, at depths 1 / 4 / 8, about 0.05 to 0.1% / 0.4 to 0.7% /
   6 to 13% in the slot (n = 256 to 768). The claim says "two to three times"; at depth 8 it is three to four. f32
   is immaterial either way.
3. **η is against the wrong vector for the code as built.** The query does not form R̃Δ. It forms R̃x per group
   (`matvec64` or `dgemm`), casts to f32, and subtracts α times the stored rotated centre `wmr` per cluster. So the
   product's error is η·(‖x_j‖ + |α|·‖m_j‖), not η·‖Δ_j‖, and ‖x_j‖ can exceed ‖Δ_j‖ without bound for a query near a
   centre. With f64 accumulation (η about 2·10⁻¹², inside the existing 4u of `ewj`) this changes nothing. For the
   "blocked f32 sums" option it is material: its bound must multiply ‖x_j‖ and |α|‖m_j‖, and the 0.02% / 1.5% shares
   quoted are for ‖x_j‖ = ‖Δ_j‖.
4. **The f32 bound quoted from the kernel is misquoted.** `cluster_q2w` uses max(n/64 + 8, 70)·u, not (n/64 + 8)·u, and
   states it for a dot product of two f32-rounded vectors against √(Σa²·Σb²). At n = 768 that is 70u, 3.5 times the
   20u used for the 0.02% / 1.5% figures.
5. **The centre.** The claim's remark is right and is a hypothesis of the ‖Δ_j‖ form: `wmr` (and anything derived from
   it: `l1m`, the per-cluster centre tables `ctab`) must be recomputed with R̃, or else the allowance is ρ_j·‖x_j‖
   (`a2[j]`), not ρ_j·`nd[j]`. Rotating centres with R̃ does not conflict with "the build must not encode against R̃":
   a centre is not coded.
6. **"At most 0.0024%" is at the rms tan φ, not per item.** The share is ρ√d/(c sin φ), unbounded as tan φ → 0. Per
   item I saw up to 0.0028% (f32) and 4.5% (int16) at n = 768, depth 8, in 300 items. The absolute allowance is what
   is bounded: ρ·sec φ·‖y‖‖Δ‖. For f32 it is a tenth of the `ewj` term already carried.
7. **ρ must be rounded outward.** The Schatten bound is ≥ ‖N‖₂ in exact arithmetic but can sit within 10⁻¹³ of it
   when one singular value dominates; evaluate it with a margin. For f32 no norm need be computed at all:
   |N_ij| ≤ 2⁻²⁴|R_ij| gives ρ ≤ ‖N‖_F ≤ 2⁻²⁴·‖R‖_F ≈ 2⁻²⁴√n (1.65·10⁻⁶ at 768, share 0.07% at depth 8).
8. **Fixed-point weights.** "η = 0" for an integer product is the product alone; converting x_j to fixed point is an
   input error of (1 + ρ)·κ‖ō‖·‖x_fx − x‖ that needs its own term.
9. **A half-width that depends on the estimate.** E1 is for a plain width. The self-normalised widths (Theorems 3.11,
   3.12; not built) take Ê as an argument, so there the allowance enters as Ē = Ê_computed + allowance, not as an
   added term.
10. **ρ is against the matrix the build encoded with, bit for bit.** It holds while every item in the index, inserts
    included, was encoded with the same f64 R. `haar_rotation` is scalar f64 with its own ln, so regeneration is
    reproducible; a stored hash of R beside R̃ and ρ would make that checkable.

**Is "the build must not encode against R̃" necessary?** Necessary for the exact law, yes, and I can show it: with a
grid as coarse as the entries (g = 1/8 at n = 64) and the one-bit code taken from R̃y, the standardised error has mean
+0.046 (90 standard errors) and Pr[> 2 sd] = 0.0234 against the law's 0.0223 (`ref_e_build.txt`). With g = 2⁻¹⁰ nothing
is visible in 4·10⁶ draws. Necessary for a guarantee at f32, no: chapter 3 already has the device (the code computed
from a rounded product costs δ·E[2^Q], Q the coordinates within rounding of a cell boundary), and the present build
already encodes against a rounded matrix, the f64 one. The argument is rigorous when Q is defined from z alone with the
worst-case radius ‖N_p‖₂·‖y‖ per coordinate: given z the code is one of 2^Q functions of z, and the lemma applies to
each. With that radius E[Q] ≈ n·√n·2^D·2.9·10⁻⁸·Σ(1/λ) is about 0.35 at n = 768, depth 8 (about 0.0005 at depth 1,
where only the boundary at 0 counts), so by my estimate an f32 build costs a factor of about e^0.35 on δ, 0.04 in c,
at the deepest code and far less elsewhere. It also needs a radial slack of order ρ·sec φ·|Ê|, a tan φ rounded up by
ρ·sec²φ (the present margin of 10⁻⁷ on tan φ is too small for that at depth 8), and the same count for near-ties
between the six scales, which I did not work out. For int16 E[Q] is in the hundreds and no argument is available. So
the rule is the simple sufficient one and costs nothing; "must not" is exact for int16 and int8 and overstated for
f32. In my runs the realised codes from R·y and R̃·y differed for no item with f32 and for 1% to 62% of items with
int16.

**Anything else required of the build?** κ, tan φ and ‖y_j‖ from z = R·y with the f64 R, as now (the code computes
κ = ‖z‖²/⟨ō, z⟩ from the rotated vector, which is what is needed); the centres as in 5; ρ_j as in 7 and 10. Nothing
else found.

## A1, A2

**A1: no error found.** Both directions checked. (2 ⇒ 1) is the lemma's computation. (1 ⇒ 2): the codebooks
z + t(z)·e_w(z) are admissible (⟨ō, z⟩ = ‖z‖² > 0), the countable dense family gives all one-dimensional marginals for
almost every z, Cramér–Wold applies, and T² ν = ν forces ν uniform because the Funk transform's multipliers are 0 on odd
degrees and of modulus below 1 on even degrees ≥ 2 when n ≥ 3 (at n = 2 they are ±1, so n ≥ 3 is needed and is
stated). The density bound and M^J are right. Unstated: statement 1 is "for almost every z"; and "the law" is the
*scalar* law for one Δ. Pair-uniformity does not give the law of the whole vector u, so results that use more than one
direction of E at once (Theorem 3.5's subspace classes, which need the law of ‖P_W u‖) are not covered by condition 2;
they would need uniformity of (Ra, R·W). The statement is exactly as strong as what is shown.

**A2: no error found, but it does not separate structured rotations from the present build.** The proof is right
(the codebook z + R(z)Δ_⊥ has κ = 1 and error ‖Δ_⊥‖² = A‖Δ_⊥‖). What it refutes: the exact conditional law, and "any
codebook", for a codebook built with knowledge of Δ and of the family. What it does not refute: a tail bound for the
built codebook (the claim says so). What the claim does not say: A2 applies word for word to today's dense matrix,
which is a function of a 64-bit seed and so takes at most 2⁶⁴ values, fewer than the 2^(kn) of k sign rounds with true
random signs. So A2 cannot be the reason to reject sign–Hadamard products; the summary's first bullet under question 1
reads as if it were. The reasons are B (explicit failing inputs for the built code) and C1.

## B0 to B5

Scripts: `ref_b.py` → `ref_b.txt`; `ref_b_stored.py` → `ref_b_stored.txt`. Written without `lib.py`.

**B0: no error found**, under its stated hypothesis of one depth over the block. The built code allows a depth per
position; with mixed depths the support statement is false as written, but the conclusion survives (see B1).

**B1: no error found; rebuilt.** E = (A/√2)(d(0)e₀ − d(a)d(b)d(s)e_s) to 5·10⁻¹⁶ on 2,000 draws for each of
(a, b) = (0, 1), (1, 2), (5, 44), n = 64 and 256, depths 1, 2, 3; tan φ = 1, 1/3, 1/5; error/sd = ±15.969 with
frequencies 0.51 / 0.49 (a = 0) and ±15.969 at 0.25 / 0.25 with 0 otherwise (distinct); query e_s alone 11.292.
With depths 1 and 2 alternating over the block (not covered by the proof) the query e₀ alone still sees 6.8 to 13.6 sd
on every draw at n = 256, so the failure does not need a uniform depth. One side remark is wrong: "a symmetric tie
rule would code this particular y exactly". The grid has no zero level at any depth, so a zero decodes to ±2^(−depth)
under any tie rule and the error keeps norm A; a symmetric rule only moves it off e₀ and e_s.

**B2: not rebuilt.** Consistent with B0 and with B4's one-round row.

**B3a, B3b: no error found.** Both identities re-derived; B3a checked on 37 characters with no mismatch; B3b's
reduction checked by running the full n = 256 two-round pipeline against the t-dimensional mixture (mean square 5.79
against 5.81; Pr[> 7.8 sd] 0.0571 against 0.0564 at t = 16 and 0.00099 against 0.00094 at t = 32). Unstated: B3b's
"except that an exact zero decodes to +v₀" is the only asymmetry at depth 1; at depth ≥ 2 a non-zero coordinate
exactly on a cell boundary (possible when √t is an integer and λ = 2) is a second one. The rates were all run at
depth 1, so nothing quoted depends on it. Also "w ∈ G" should read "W = G" (take w = 0).

**B3, B4: the bounds are valid lower bounds and the conditioning does not overstate them.** The structure events are
exact products of conditional probabilities of independent rounds, and Pr[exceed] ≥ Pr[structure]·Pr[exceed |
structure] is used in the right direction. One thing the statement leaves out: for k ≥ 2 the last coset is uniform
over the n/t cosets of G, so the conditional law is the mixture (t/n)·(zeros coded up) + (1 − t/n)·(zeros contribute
nothing), and the two differ (mean square 1.00 against 0.66 at t = 32). The theorist's script samples that mixture by
one random character per chunk of 2,000 to 4,000 draws, 20 to 100 chunks per row, which is noisy for the rare
component. I recomputed every quoted entry with the exact mixture weights (exhaustive for t ≤ 16, 6·10⁶ draws above):

| n (c) | k = 2 | k = 3 | k = 4 |
|---|---|---|---|
| 256 (8.282) | 2^−19.1 (t = 64) | 2^−41.1 (t = 16) | 2^−58.5 (t = 32) |
| 512 (8.589) | 2^−20.6 (t = 64) | 2^−57.0 (t = 32) | 2^−83.0, no violation |
| 1,024 (8.748) | 2^−31.9 (t = 64) | 2^−88.9, no violation | |

All within 0.1 of the claim. They are lower bounds on Pr[|error| > c·sd] with the exact tan φ, two-sided against
5·10⁻¹⁹, which is a fair test of a one-sided 2.5·10⁻¹⁹ (one of the two signs of Δ must fail). The built half-width
uses tan φ rounded up to a 15-value table or an escape; under a pessimistic model of that (floor at the 0.2% quantile
of typical data's tan φ, then 2^(1/20) up) the entries become 2^−21.3, 2^−46.2 (or 2^−47.5 at t = 32), 2^−58.5 at 256
and 2^−21.4, 2^−58.3 at 512, so every violation stands; the margin for three rounds at 512 falls from 14 to about 6.
The unconditional two-round run reproduces: 51, 11, 1 draws above 5, 6, 7 sd in 600,000 where the law expects 0.19,
3·10⁻⁴, 10⁻⁷. Caveat on reading the table: these are Monte Carlo estimates of the conditional rate times an exact
structure probability, not proofs; the two marginal entries (k = 4 at 256, k = 3 at 512) rest on thousands of counts.

**B5: supported as far as it goes, and it goes a little further than its own "gap" says.** The disjoint-support
statement is right. The claim "an assumption on the item alone, or on the query alone, does not restore the law" is
supported by B4's t = 16 rows at n = 256, where both vectors have every coordinate at 4/√n: two rounds 2^−30.1, three
2^−41.1 (2^−46.2 with stored-scalar rounding). So a two-sided incoherence assumption with μ ≥ 4 is refuted for k = 2, 3
at n = 256 at today's constant, not merely unproved. What is not shown: anything for μ ≤ 3 (the family needs
|V| ≥ 32 there and its probability falls below budget), anything for k ≥ 5, or a failed bound (as against a wrong
variance) for one round at t = n/2. "Wrong by the factor 2 in variance on every draw" means the scaling identity
holds on every draw; the factor 2 is in the mean square.

## C1, C2

**C1: no error found.** (i) is counting. (ii): the fixed query Δ = t·E₀/A₀ is orthogonal to y, so ‖Δ_⊥‖ = t and the
conclusion h(A₀, t) ≥ A₀·t follows. The round counts (n ≥ 62, 32, 22; five rounds at 16) are right. Hypothesis used
and stated: the half-width is a function of (A, ‖Δ_⊥‖) only.

**C2: the arithmetic is right; the recommendation does not follow; one fact about the build is missing.**
2⁻⁶⁴ = 5.42·10⁻²⁰, δ = 2.5·10⁻¹⁹ is 4.6 atoms, and with N rounded up to a power of two the budget first falls below
one atom at N = 2³³ (at 2³² it is 1.07 atoms). What does not follow: the guarantee is the answer-level one,
Pr_ω[answer wrong] ≤ ε = 10⁻⁹, which is 1.8·10¹⁰ atoms at every N. The per-event δ is a step in a union bound under
the Haar model. For a generator that behaves like 2⁶⁴ independent Haar draws, the number of seeds failing one event is
about Poisson(δ·2⁶⁴), the per-event probability is a random multiple of the atom with mean δ, and the fraction of seeds
that fail a query is ε to within 1 part in 10⁵, whether δ·2⁶⁴ is 4.6 or 0.5. Nothing changes at one atom. "At most
four seeds fail a given event" is likewise not something the present build establishes at N = 10⁹; it is what
δ-validity would mean if it were proved for the generator, and it is not. A longer seed is harmless but makes nothing
provable and is not needed for ε. Missing fact: the seed is a compile-time public constant (`rust/src/index.rs` line
5, `SEED = 20261112`; SYSTEM.md says "a public seed"), so as built S = 1, every event has probability 0 or 1 for given
data and query, and the guarantee is wholly a statement in the model, for data and queries that do not depend on the
constant. If a real probability over ω is wanted, the change that buys it is drawing the seed at build from the
system's entropy and storing it, at whatever width; the width matters only relative to ε, not to δ.

### Addendum to B4: small blocks (the claim is wrong at the block's own constant)

`ref_b_128.py` → `ref_b_128.txt`; `ref_b_64.py` → `ref_b_64.txt`.

The claims say "at blocks of 64 and below the construction does not reach the constant at all" and, under B4, that
at n = 64 the construction gives at most 4.5 sd, "below any constant in use". That is true only against the round
figure 7.8 and only for the two values of t that were run (8 and 16). At t = n/2 = 32 the largest |error| is
√(63/31)·4.98 = 7.11 sd, and the block's own certified constant at d = 63 is 6.758. Direct simulation at n = 64 with
no conditioning (2·10⁶ draws, depth 1):

- one round, y = 1_G/√32, Δ = χ₁·1_G/√32: Pr[|error| > 6.758 sd] = 2.8·10⁻⁴;
- two rounds, y = (e₀ + e₃₂)/√2, Δ = (e₁ + e₃₃)/√2: 1.2·10⁻⁴, which is the construction's 2^−13.0 exactly;
- three rounds: nothing in 2·10⁶, as expected from the bound 2^−39.0. Four rounds: bound 2^−40. Five: 2^−66, not
  reached.

At n = 128 (a block size the 768 layout has, and what 96 pads to; own constant 7.717) the same construction gives
2^−11.6, 2^−16.0, 2^−42.0 and 2^−46 for one to four rounds, all above 2^−60.8. At n = 16 it does not reach (largest
1.9 to 2.2 sd against about 3.9). So sign–Hadamard rounds fail at every power-of-two block from 64 up for one to four
rounds, by this family alone. The sentence about small blocks should be withdrawn or restricted to n ≤ 32.

## D1 to D4

**D1: no error found.** The pair-selection probability 2/n, the bound (1 − 2/n)^T ≥ exp(−2T/(n − 2)), and 21.4·n at
τ = 2.5·10⁻¹⁹ are right. Stated as a floor and not as a failed bound, which is the correct strength.

**D2: correct after a repair to the statement.** The proof gives ⟨e, RΔ⟩ = ⟨e, z_odd⟩/(s·c); the displayed formula has
‖z_odd‖·‖z_even‖ = |s|·|c| in the denominator. The sign of s·c is not a function of z: (c, p) and (−c, −p) give the
same z and are equally likely. So it is |Δ·E| that is a function of z alone, and the sign is a fair coin given z. My
run: |error| equals |formula| to 2·10⁻¹¹ and the signs agree on 50.1% of draws (`ref_fgh.txt`). The theorist's script
compares absolute values, so D4's "the error equals D2's formula on every draw" has the same slip. The conclusion is
unchanged: given z the error takes two values where the law has a continuum, for any codebook.

**D3: a sketch with its gap named; nothing asserted that I could break.** The flop comparison (a factor 1.8 to 5.2
below the dense product at n = 256 to 768 with C = 1) is right as arithmetic.

**D4: consistent with the outputs; not rebuilt beyond the sign check.** Two readings to tighten: the median of
|error|/sd at n = 256 is 3.25 at depth 1, so "about half the draws exceed 4 sd" holds at depth 2 only; and at the
built block sizes (up to 768) no draw above the certified constant was seen at n ≤ 256, so for the butterfly what is
shown there is a wrong law (rms 3.7 to 4.1), with a failed bound shown only at n = 1,024.

## F1 to F5

`ref_fgh.py` → `ref_fgh.txt`.

**F1: no error found.** Unstated: every sub-block has y_i ≠ 0 (for y_i = 0 the term is κ_i‖ō_i‖‖Δ_i‖·U in b_i
dimensions, not b_i − 1), and ⟨ō_i, z_i⟩ > 0 for F2's κ_i (true for the built code whenever z_i ≠ 0, since each
position's decoded value has its coordinate's sign).

**F2: no error found; the constants reproduce.** My implementation of Theorem 3.21 gives 8.815 and θ = 1.00231 at
n = 768, and at n = 2048, N = 10⁹: 8.944, 8.952, 8.969, 9.079 (N = 2²⁰: 8.179, 8.187, 8.203, 8.304); sphere quantiles
8.830, 8.830, 8.831, 8.834. "No loss of width" is for the half-width as a function of the per-sub-block query norms
‖Δ_i‖, which F2 needs anyway; a width that knew only ‖Δ‖ would be 3 to 6% wider with two sub-blocks.

**F3: no error found.** The rounding-up step is legitimate: ā_i = ‖Δ_i‖·√(b_i − 1)·max_j A′_j/√(b_j − 1) ≥ a_i and
Theorem 3.19 applies. The single stored scalar B + c_min·M is valid for every c ≥ c_min.

**F4: no error found** in the model (the variance π/2 − 1 for the one-bit code checks by the delta method).

**F5: reproduced for four rows.** 256 = 2 × 128: 1.13 / 1.18 / 1.20 (claim 1.12 / 1.16 / 1.20); 256 = 4 × 64:
1.26 / 1.37 / 1.43; 768 = 2 × 384: 1.12 / 1.17 / 1.21; 768 = 3 × 256: 1.19 / 1.28 / 1.34. The skewed rows and the
"equalising" encoder were not rebuilt.

## G1 to G10

**G1: no error found.** Checked: no clamping under the stated range; frac(x + Kτ) uniform for integer K ≥ 1; errors
independent across positions for an item fixed independently of τ. Hypotheses, all stated: no overload, s a function
of the item alone, Δ independent of the seed. The law is for coded positions; an uncoded position contributes Δ_p·y_p,
which G7 (b) accounts for.

**G2: no error found.** **G3: no error found**; the example's factor 2 checks (v and v + ½ modulo 1), and neither
step overloads at equality a = s/2. **G4: no error found.**

**G5: (i) to (iii) no error found; (iv) cited; (v) a conjecture.** The constant 3.178 = 1/(4·Pr[G ≥ √2]) is the
Bentkus–Dzindzalieta constant as I recall it; I did not open the paper. **G5a reproduced**: sub-Gaussian 9.256 / 8.482
/ 8.149; Gaussian 8.912 / 8.117 / 7.774; at δ/3.178 9.040 / 8.256 / 7.919; sphere 8.282 / 8.694 / 8.830; uniform sums
by my own Lugannani–Rice 8.354 / 8.778 / 8.868 / 8.896 at N = 10⁹ (and the other two columns to 0.001).

**G6: no error found** as a count; the scalar layout (two nibbles for ‖y‖, one or two for κ, one for tan φ per rise)
matches `encode_cluster`. **G7: the coefficients check by hand** (0.767, 0.789, 0.740, 0.687 from the quoted tan φ);
the crest factors and variants (a), (b) were not rebuilt. **G8: no error found.**

**G9: one word to repair.** "Uncorrelated" holds only when every coded position has the same depth:
Cov(⟨ε, Δ_⊥⟩, ⟨ε, y⟩) = Σ_p Δ_⊥,p·y_p·w_p²/12, which is zero for equal w_p and not in general. The conclusion (the
rescaled estimator does not combine with the dither) is unaffected and, if anything, stronger.

**G10: a conjecture; nothing to check.**

## H1 to H7

**H1: the lemma, no error found; one table row is wrong.** For O(b) ⊗ O(k) the stabiliser of a generic y is finite
only when b = k. For b > k it contains O(b − k) acting on col(Y)^⊥ ⊗ ℝ^k (take O₂ = 1), so the row should read as the
O(b) ⊗ 1_k row does, with b² + k² stored numbers. 256 = 16 ⊗ 16 is the finite case; 768 = 32 ⊗ 24 is not. No design
conclusion moves: neither case gives a sphere.

**H2: no error found.** Λ³ℝⁿ is irreducible under O(n), has no O(n − 2)-fixed vector (each summand carries Λ^j ℝ^(n−2)
with j = 1, 2, 3, non-trivial under O(n − 2) because it contains reflections), and Schur orthogonality kills every
other π(μ). **H3: no error found.**

**H4: no error found.** The complex identity and the real-κ decomposition both check (⟨Δ, Iy⟩ = −⟨IΔ, y⟩;
|β| ≤ tan φ). Unstated: the "allowance" option also needs a stored number per item and rise, |β| or a combination
such as F3's, because |β| ≤ tan φ alone is Cauchy–Schwarz. So the choice is "a second stored scalar" or "a second
stored scalar and a second sum", not "nothing" or "both". The 10% and 21% are from n = 64 and U(m) only; nothing was
measured for Sp.

**H5: cited; agrees with what I recall** of the classification (isotropy SU(3) of G₂ on S⁶ and G₂ of Spin(7) on S⁷ are
transitive on the tangent sphere; Spin(9), U(m), Sp(m)·Sp(1) are not). Not opened.

**H6: a conjecture.** **H7: no error found** as a count. One point in its favour that the claim does not make: a
reflector I − 2ṽṽᵀ/‖ṽ‖² built from a rounded vector is exactly orthogonal, so R̃ is an exact rotation within
ρ ≤ (n − 1)·2⁻²³ of R, and only the application's own rounding is left for η.

## What I did not check

- The librarian's report, and every [cited] statement, beyond the two I could recall (Bentkus–Dzindzalieta's constant,
  the transitive groups).
- B2's table, D4's Kac rows, F5's skewed and equalising rows, G7's crest factors and variants, G5 (v), H4's measured
  10% and 21%, E2's int8 and clipped-range rows, E2 at block sizes other than 256 and 768.
- Anything by interval arithmetic. Every number of mine is floating point; the B4 entries are Monte Carlo rates times
  exact structure probabilities.
- Whether the existing kernel's κ-rounding term covers today's `ewj` when ‖x_j‖ ≫ ‖Δ_j‖ (E, repair 1, side
  observation). That is a question about chapter 1's deterministic terms, outside these claims.
- Chapter 3's own E[2^Q] remark: I used it and re-derived why it is rigorous with a worst-case radius, but I did not
  check the chapter's figure of 1 + 10⁻⁸ for the f64 build.
- No timing, no cargo, no build. NOTEBOOK.md not read.
