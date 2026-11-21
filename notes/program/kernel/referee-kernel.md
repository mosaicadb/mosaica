# Referee's report: the built integer decision kernel

Referee's working file, appended as findings arrive. Nothing here has entered the chapters.

Commit read: `8c0b316f9149a4ad46d234cfe57ee3c1400413e7` (worktree `vexact-theory`, 21 November 2026).
Code read: `rust/src/v2/kern.rs`, `rust/src/v2/q.rs` and what they call. Nothing was built or run except small numpy
scripts kept in this directory.

A path I could not break is recorded as "no error found", not as "correct".

## Summary

- Theorem 1.19 as built: sound, proof in section 2 (signed full B, dial on the OUT side only, t − tol on the IN
  side). Every IN is certified at ψ = 1 whatever the dial.
- 1.21′ as built: sound up to 127 deterministic units (proof in 3a); from 128 units the OUT compare is wrong, and
  the unit count is not asserted on the default (combined-tables) path (K1). Not reachable with the shipped levels.
- 1.22 as built: sound (proof in 3d), but it is a float combine, not the notes' integer combine, and e_ℓ ≤ 3Uσ is
  false for it.
- f64 thresholds: sound (proof in 3b). A NaN threshold clamps Tin to its unsafe end (K2, edge).
- The certificate (A0) that all of this implies holds only under unasserted preconditions: the key's clamp loses
  x·m̂ − 1 for a row along its centre whose f16 norm exceeds 1 (K3), and the f32 evaluation of m·r is covered only
  while |α|·‖m‖ ≤ 23 (K4).
- No input found on which an integer decision contradicts (A0).

The other files in this directory (`referee-range.md`, `range_*.py`, `nucut_attack.py`, `closed_form_check.py`) are
another referee's and are not part of this report. Mine: this file, `k1_compare.py`, `k2_thresholds.py`,
`k3_margins.py`, `k4_tables.py`, `k6_key.py`, `kemu.py` and their `.out` files.

## Log of findings

### 1. The statement the code relies on (from the code alone)

Sources: `kern.rs` lines 386–412 (comment and constants), `rtab` (462), `int_thresholds` (518, 549), `cmp2w` (594,
654), `int_level` (616, 668); `q.rs` lines 1310–1320 (thresholds), 1394–1413 (margins), 1578–1600 (use of the masks),
`check_det` (1197).

Per (query, cluster): qm, alr (α), fixed, ewdet, dwa_ℓ are f32 values widened to f64; σ is one f32 step for all
deterministic levels. Per item, from its 16-bit key in f32: the scale s > 0 and mr. Per level ℓ: B_ℓ = the sum over
the units through ℓ of the units' table minima (the whole sum, signed; no remainder is taken and no centring constant
is added to the accumulators), e_ℓ = the sum of the parts' error bounds times (1 + 10⁻⁶), D_ℓ = `Prep2::dial[ℓ]`,
D_max = the largest D_ℓ over the deterministic levels. For norm code n < 15:

    X_ℓ(n) = dwa_ℓ · pconst_ℓ · (1 + 10⁻⁶) · ntab_ℓ[n] + ewdet            (f64)
    Rin_ℓ(n)  = up((X_ℓ(n) + e_ℓ − B_ℓ)/σ),     Rout_ℓ(n) = up((B_ℓ + e_ℓ + D_ℓ·X_ℓ(n))/σ),
    up(x) = ⌈x + (|x| + 1)·10⁻¹²⌉.

A margin is kept only if RIN_MIN ≤ Rin < RSENT (ROUT_MIN ≤ Rout < RSENT); otherwise, and for n = 15, it is RSENT.
RIN_MIN = THETA_MAX − TMAX + 1 = −4,047,166; ROUT_MIN = TMIN = −6,144,000; RSENT = 8,388,352; THETA_MAX = 2,097,088;
TMAX = 6,144,255.

Per item, once, in f64:

    K_in  = (t − tol + fixed − qm − alr·mr)/s,       K_out = (t − D_max·fixed − qm − alr·mr)/s,
    Tin  = clamp(⌈y_in + slack + |y_in|·2⁻⁴⁶⌉),      Tout = clamp(⌊y_out − slack − |y_out|·2⁻⁴⁶⌋),
    y = fl(K/σ),  slack = (|t| + |fixed| + |qm| + |alr·mr|)·2⁻⁴⁶/(s·σ),  clamp to [TMIN, TMAX].

(With `out.iv` set, K_in = +∞ and Tin = TMAX: the integer levels take nothing in.)

Accumulators: Θ = 256·Hs + Ls, the exact sum of the 16-bit table entries θ_u ∈ [0, 65,535] of every unit through
level ℓ (i16 words; no wrap while Hs ≤ 8,128 and Ls ≤ 32,767).

    IN  at level ℓ  iff  Θ − Rin_ℓ(n) ≥ Tin        (non-strict)
    OUT at level ℓ  iff  Θ + Rout_ℓ(n) < Tout      (strict)

both evaluated as sat(sat(Hs − TH) ∓ RH) + ((Ls − TL ∓ RL) >> 8) with a saturating last addition, and then masked by
the live lanes. The real statement each is meant to imply (the one `check_det` tests in verify mode), with
est = qm + alr·mr + s·A, A the real table sum on the f32 weights, and hh = s·X_ℓ(n) + fixed:

    IN  ⟹ est − hh ≥ t − tol,            OUT ⟹ est + D_ℓ·hh < t.

### 2. Theorem 1.19 against the built inequalities

Write (1) for |σΘ + B_ℓ − A| ≤ e_ℓ (Lemma 1.18 with B_ℓ the whole sum of minima; the proof is unchanged, because the
proof never used |B_ℓ| < σ).

*IN.* Θ ≥ Tin + Rin gives σΘ ≥ K_in + X + e − B, so A ≥ σΘ + B − e ≥ K_in + X by (1). Multiplying by s > 0:
qm + alr·mr + s·A − (s·X + fixed) ≥ t − tol. That is the lower end of (A0) at ψ = 1.

*OUT.* Θ + Rout < Tout gives σΘ + B + e + D_ℓ·X < σ·Tout ≤ K_out, so A + D_ℓ·X < K_out by (1), and
qm + alr·mr + s·A + D_ℓ·s·X + D_max·fixed < t. Because fixed ≥ 0 and D_max ≥ D_ℓ, est + D_ℓ·(s·X + fixed) < t.

Differences from the refereed design, and whether each is covered:

- **Signed B_ℓ.** The design's margin is ⌈(D·X + e + |B|)/σ⌉ on both sides, B a remainder. The built margins carry
  −B on the IN side and +B on the OUT side, B the full sum. The design's proof uses |B| only through
  −|B| ≤ B ≤ |B|; replacing each by B itself is the same two lines with equality in place of an inequality, so the
  theorem's proof covers it. The signed form is the tight one. Verdict: implied by the theorem's own argument. The
  price is that R is no longer in [0, R_cap]: it ranges over negative values down to RIN_MIN and ROUT_MIN, which
  changes the saturation argument (section 3 below), not the implication.
- **No centring constant.** The design adds ⌊B/σ⌋ to the accumulators per level and leaves a remainder. The code
  adds nothing; the accumulators are unsigned sums. This removes the referee's third gap (the range of the centring
  constant) because the constant does not exist.
- **Dial on the OUT side only.** For ψ = 1 the built inequalities are the design's with signed B: no difference.
  For ψ ≠ 1 the design scaled both sides by D, so its IN at D < 1 was not a certificate. The built IN carries no
  dial anywhere (not in K_in, not in Rin), so **every IN is certified at ψ = 1 whatever the dial**. The built OUT
  carries D_ℓ on s·X and D_max on fixed; for D_ℓ < 1 it is a statistical exclusion and nothing more, for D_ℓ ≥ 1 it
  is implied by the certificate. This matches the f32 path (`lb = est − hh`, `ub = est + hh·dial`) and `check_det`.
  Verdict: sound by the argument above, which is the theorem's with D set to 1 on one side; the theorem as written
  (one D on both sides) does not state it and should be restated with two factors, D_in = 1 and D_out = D_ℓ.
- **D_max on the fixed term.** One Tout serves every deterministic level, so the fixed term takes the largest dial.
  It only lowers Tout, so the integer OUT asks for slightly more than the f32 path's `ub < t` at a level whose dial
  is below the largest. Sound (shown above), and conservative.
- **tol.** K_in carries t − tol. IN then certifies score ≥ t − tol, which is the guarantee's left inclusion with
  d = tol. Not in the design; sound.

### 3a. The two-word compare and the saturation repair (1.21′) — `k1_compare.py`, output `k1_compare.out`

Read from `cmp2w`: IN is sat(sat(Hs − TH) − RH) + ((Ls − TL − RL) >> 8) ≥ 0 with the last addition saturating, the
low word wrapping; OUT is the same with +RH, +RL and < 0.

Proof that it equals the full-width compare, for the ranges the code enforces. Hs ∈ [0, 8,128] and TH ∈ [−24,000,
24,000], so Hs − TH ∈ [−24,000, 32,128] is exact. RH ∈ [−15,810, 32,767] on the IN side and [−24,000, 32,767] on the
OUT side, so d = Hs − TH ∓ RH can leave i16 in either direction; the second operation saturates. g = Ls − TL ∓ RL
lies in [−510, 16,320] (IN) or [−255, 16,575] (OUT) while Ls ≤ 64·255, so g is exact in i16 and g >> 8 = ⌊g/256⌋ ∈
[−2, 64]. Lemma 1.20 gives 256d + g ≥ 0 ⟺ d + ⌊g/256⌋ ≥ 0 for the true d. If d was not saturated, the last
saturating add can clip only a sum above 32,767 (true sum positive) or below −32,768 (true sum negative), so the sign
is kept. If d was saturated high, the true d ≥ 32,768 and both the computed word (≥ 32,765) and the true value
256d + g are positive. If saturated low, the true d ≤ −32,769, the computed word is ≤ −32,704 and the true value is
≤ 256·(−32,769) + 16,575 < 0. ∎

The sentinel and the clamps. With R = RSENT: Θ − RSENT ≤ THETA_MAX − RSENT = −6,291,264 < TMIN ≤ Tin, never IN;
Θ + RSENT ≥ RSENT > TMAX ≥ Tout, never OUT. Tin lowered to TMAX: Θ − Rin ≤ THETA_MAX − RIN_MIN = TMAX − 1, never
IN. Tout raised to TMIN: Θ + Rout ≥ ROUT_MIN = TMIN, never < TMIN, never OUT (the code comment writes "> TMIN"; it
is "≥", which is enough because OUT is strict). Tin raised to TMIN and Tout lowered to TMAX only withhold.
These need Θ ∈ [0, THETA_MAX], which holds because entries are clamped at 0 and the code asserts the sum of the
units' largest entries (`id.q`) ≤ 256·IHMAX.

Numerical attack: 4,000,000 random cases per setting with half on the decision boundary and thresholds and margins
at their range ends, plus a sweep of every saturation corner. At 64 units (as built) and at 127 units: 0 mismatches
for IN and OUT, sentinel and clamps never decide. **At 128 units the OUT compare is wrong** (first case found:
Hs = 0, Ls = 32,639, Tout = −4,201,876, Rout = −4,234,514: kernel says OUT, exact compare says no), because
Ls + RL passes 32,767 and the low word's shift turns negative. At 160 units both sides are wrong.

**Finding K1 (a missing guard, not reachable with the default levels).** "At most 64 deterministic units" is
asserted only in `int_sigma`, the path without combined tables. The default plan has `gtab: true`, whose step is
computed in `CQ2::enable_int` with no such assert. The build accepts any deterministic schedule below the first
randomised level (`mod.rs` 2345–2350), so a build with `--dlv` ending at 64 bytes or more (which also needs `--rlv`
starting above it) gives 128 or more units and a low-word sum that can wrap. With the shipped schedules (at most 32
bytes, 64 units; or anything up to 63 bytes, 126 units) the compare is exact. Repair: assert units ≤ 127 in
`enable_int` (or ≤ 64 to keep the comment's ranges), or refuse the schedule at build.

Verdict on 1.21′: **sound with the proof above for at most 127 units; sound after the stated change in general.**

### 3b. Thresholds in f64 — `k2_thresholds.py`, output `k2_thresholds.out`

Direction needed: Tin up, Tout down. Direction taken: ⌈y + slack + |y|·2⁻⁴⁶⌉ and ⌊y − slack − |y|·2⁻⁴⁶⌋.

Proof that the allowance covers the worst case (u = 2⁻⁵³; s, mr, alr, qm, fixed, σ are f32 values, exact in f64).
The caller's cin = fl(fl(fl(t − tol) + fixed) − qm) has |cin_c − cin| ≤ u(|t − tol| + |t − tol + fixed| + |cin|) ≤
u(3|cin| + 2·mag), using |t − tol| ≤ |cin| + fixed + |qm|, with mag = |t| + fixed + |qm|. The same bound holds for
cout whatever D_max is, because D_max·fixed ≤ |t| + |cout| + |qm|: this settles Editor's query 7's point about a
dial above 1. Then a = fl(alr·mr), the subtraction, r = fl(1/s) and two products each add a relative u:

    |y − K/σ| ≤ 8u·|K/σ| + (4u·|a| + 2u·mag)/(sσ)          (first order).

The slack as computed is at least 127u·(mag + |a|)/(sσ) and the relative term at least 127u·|y|, and the two
additions lose at most 2u(|y| + slack). So the argument of the ceiling is at least K_in/σ and that of the floor at
most K_out/σ, with a factor of about 14 to spare. ∎

Numerical attack: 1,700,000 cases against exact rationals (random; s at its floor √(10⁻¹²/λ); |α| in the hundreds;
dials to 10⁶; tol to 10³; t placed so that K/σ is an integer to f64 accuracy and stepped ±3 ulps; cin − a at
cancellation). 0 violations. The outward rounding costs 0 steps beyond the exact ceiling or floor in the best case,
so the slack is not wasteful.

**Finding K2 (edge, low).** The clamp is min(max(v, TMIN), TMAX) with x86 semantics, which sends a NaN to TMIN on
both thresholds. TMIN is the safe end for Tout and the unsafe end for Tin. With finite inputs no NaN arises (s ≥
√(10⁻¹²/λ) > 0, every product is finite, and K_in = +∞ under `out.iv` gives TMAX). A NaN threshold t in threshold
mode gives Tin = TMIN and every item whose margin is not the sentinel is taken IN, where the f32 path (ordered
compares) decides nothing. The refiner's NaN threshold runs in Interval mode, which never uses the integer path, so
this is reachable only by a caller passing t = NaN to a threshold query. t = −∞ also gives Tin = TMIN, which is the
right answer there. Repair: write the Tin clamp as min(TMAX, max(TMIN, v)) with the operands ordered so NaN goes to
TMAX, or refuse a NaN threshold at the entry.

Verdict on the f64 allowance: **sound, proof above** (it was argued in no written source before).

### 3c. Margins in f64 (`rtab`'s `up`) — `k3_margins.py`, output `k3_margins.out`

Direction needed: both margins up. Taken: up(x) = ⌈x + (|x| + 1)·10⁻¹²⌉ on the f64 quotient.

Rin = (X + e − B)/σ has no cancellation when B < 0 (the usual case: B is a sum of minima of tables that straddle 0),
so its f64 error is about 3·2⁻⁵³ relative and `up` covers it. Rout = (B + e + D·X)/σ can cancel: |B|/σ is up to
about 2²⁰ steps, so the f64 sum can be off by 2⁻⁵³·2²¹ ≈ 2·10⁻¹⁰ steps while `up` adds only 10⁻¹² when the quotient
is near 0. So `up` alone does not prove Rout ≥ the real quotient. What closes it is the slack in e_ℓ: a shortfall is
at most 2·10⁻¹⁰ steps, and e_ℓ carries at least 4·10⁻⁶ steps beyond half a step per unit (its two factors of
1 + 10⁻⁶ on at least 8 units), so Rout·σ ≥ B + (e_ℓ without that slack) + D·X still holds. The attack agrees
(296,012 cases placed at cancellation): the computed quotient fell below an integer that the exact one exceeded 63
times, all at |R| ≥ 1,859, by at most 1.0·10⁻¹⁰ steps where `up` adds at least 1.9·10⁻⁹; near small integers it
never did (integer multiples of σ lie on the f64 grid of the summands, σ being an f32); Rout was short of the exact
quotient in 0 cases and Rin in 0. **No error found; sound, with the slack in e_ℓ as the proof.**

X itself is computed in f64 from f32 factors, three roundings; the encoder's guarantee is
fl32(fl32(pconst·s)·ntab[n]) ≥ ρ(1 + 10⁻⁶) + 10⁻¹² (`mod.rs` 1693–1706, checked by a while-loop on the stored code),
so the real pconst·s·ntab[n]·(1 + 10⁻⁶) exceeds the true weighted error norm ρ by a factor of about 1 + 1.9·10⁻⁶,
which covers the two f32 products (1.2·10⁻⁷) and the f64 roundings. Sound.

### 3d. Tables: `build_lut`, and the centre tables as built (1.22) — `kemu.py`, `k4_tables.py`, output `k4_tables.out`

Direction needed: none (entries are rounded to nearest); the allowance e_ℓ must be at least the worst error, on both
sides. What the code does, per unit u and code v:

- *Own tables* (`build_lut`, the path without combined tables). t_u(v) is an f32 FMA chain over at most 4 fields;
  lo_u is the f32 sum of the fields' minima; θ_u(v) = rne((t_u(v) − lo_u)·fl(1/σ)) clamped to [0, 65,535]; the
  part's offset is Σ lo_u rounded to f32. Error bound per part: nu·σ/2·(1 + 10⁻⁶) + (48 + 2nu)·2⁻²⁴·Σ|w|max|E| +
  10⁻³⁰. My count of the float part: FMA chain 2, lo 3.5, the subtraction 1, the product with 1/σ 2, the offset's
  cast 0.5: 9 units of 2⁻²⁴·Σ|w|max|E| against 48 allowed. The clamp never acts: σ ≥ (largest unit range)·(1 +
  1/512)/65,535 keeps every entry below 65,408, and a slightly negative t − lo rounds to 0 within the float term.
- *Combined tables* (`combine_lut`, the default, `gtab`). v_u(e) = fl32(Tq_u(e) − θ^m_u(e)·fl32(α·σ_m)) by one
  fused operation; μ_u is the exact minimum of the 16 computed values; θ_u(e) = rne((v − μ)·fl(1/σ)) saturated to
  u16; offset Σ μ_u − α·Σ lo^m_u, rounded to f32. Bound per part:
  err_q + |α|·err_m + σ·(nu/2·(1 + 10⁻⁶) + Σ_u(4·2⁻²⁴·mag_u + 3·2⁻²⁴·65,536)) + 4·2⁻²⁴·|off| + 10⁻³⁰,
  with err_q = 8·2⁻²⁴·Σ|wq|max|E| the group table's float error and err_m the stored centre table's own bound
  (its grid step σ_m/2 per unit plus float terms). Each term covers what it names: the FMA's rounding and the
  rounded factor α·σ_m need 0.5·2⁻²⁴·(|α|σ_m·θ^m_max + max|v|)/σ steps against 4·2⁻²⁴·mag_u; the subtraction and
  the product with 1/σ need 1.5·2⁻²⁴·65,536 against 3·2⁻²⁴·65,536; the offset's cast needs 0.5·2⁻²⁴·|off| against 4.
  Saturation never acts: σ ≥ (largest combined range)·(1 + 1/512)/65,533, computed by `det_spans` from the same
  f32 values `combine_lut` recomputes.
- *Linearity.* The centre's stored tables are built from `Cl2::wm` = fl32(mv32·sig), the same f32 weights the query
  path without combined tables uses and verify mode's f64 reference uses, so the real table of the combined path is
  L^q − α·L^m on the weights wq − α·wm in real arithmetic. Proposition 1.22's algebra holds as stated.
- *Every `as f32` on an error bound* rounds to nearest, so it can lose 3·10⁻⁸ relative. Each bound's leading term
  carries 1 + 10⁻⁶ and `id.e` multiplies by 1 + 10⁻⁶ again; the float terms have a factor of 2 or more in hand.

So (1) holds: |σΘ + B_ℓ − A| ≤ e_ℓ for the real table sum A on the f32 weights. ∎ (float constants by my count, not
by a machine-checked analysis).

**Proposition 1.22 as built is not the proposition in the notes.** The notes (and the design) state an integer
combine by `mulhi` with e_ℓ ≤ 3Uσ. The code combines in f32 and its bound is the sum above. It is sound, and it is
*not* bounded by 3Uσ: the centre table's own grid enters as |α|·σ_m/2 per unit, in the centre table's step, and
σ_m is unrelated to the cluster's step σ. When the query is close to α times the centre in the group's coordinates
the combined range (and σ) is small while |α|σ_m is not.

Numerical attack (two seeds, 600 trials each: five weight regimes, five level schedules to 64 units, three table
shapes; own and combined tables): inequality (1) at its worst case over all codes reached 0.976·e_ℓ and never
exceeded it (3,600 level checks per seed); no entry was clamped; the sum of the largest entries never exceeded
`qsum` nor 256·IHMAX. e_ℓ per unit in fine steps: own tables 0.51–0.63 (design: 0.5); combined tables median
0.6–0.84 and at most 1.8 in the ordinary regimes (design: 3), but median 344–367 and up to 1,750 when
wq = α·wm·(1 ± 10⁻³). That last regime is a tightness loss the design's bound hides, not an error.

Verdict on 1.22 as built: **sound, by the proof above; the statement in the notes must be replaced** (float
combine, the bound as the code computes it, no 3Uσ).

### 3e. What the certificate (A0) takes from the key and the weights — `k6_key.py`, output `k6_key.out`

These are shared with the f32 path; an integer decision is only as good as (A0).

- *Item scale s.* Not an approximation: the encoder (`item_scale`, `index.rs` 11) and the query (`q.rs` 1286–1289)
  apply the same f32 operations in the same order to the same key, the stored norm code is relative to that s, and
  verify mode asserts equality. s ≥ √(10⁻¹²/λ) > 0. No rounding question. No error found.
- *Query weights.* wq = fl32(qv·sig) with qv exact to one f64 rounding; rounded to nearest, covered by
  s·ewdet = s·12·2⁻²⁴·(Σ|wq| + |α|Σ|wm|). Needed by my count: (1 to 2)·2⁻²⁴ per weight times the largest decoded
  coordinate value, which the code comment puts below 3.5 (not asserted anywhere I found); 12 is enough while that
  value is below about 6 on the path with own tables and 24 on the combined path. No error found.
- *m·r.* mr = fl32(mn32·(key/32767) − mn32²). Allowance in `fixed`: |α|·‖m‖·(0.5/32767 + 10⁻⁷) + 4·10⁻⁶.
  (a) The f32 evaluation differs from ‖m‖·a_q − ‖m‖² by up to 2.7·10⁻⁷·‖m‖ (every key, 404 centre norms), more
  than the 10⁻⁷ in DA. The rest must come out of the absolute 4·10⁻⁶, which suffices while |α|·‖m‖ ≤ 23. Nothing
  bounds |α|: it is a least-squares coefficient in weighted coordinates. **Finding K4 (gap, small):** for
  |α|·‖m‖ > 23 the allowance is short by 1.7·10⁻⁷·|α|‖m‖ − 4·10⁻⁶. Repair: DA = 0.5/32767 + 3·10⁻⁷.
  (b) **Finding K3 (unsound outside an unstated precondition).** The build stores
  round(clamp(x·m̂, −1, 1)·32767) (`mod.rs` 1589). A stored row has f16-rounded norm, not norm 1, so x·m̂ can exceed
  1 for a row lying along its centre, and the clamp then discards the excess: the key's error is x·m̂ − 1, not at
  most half a step. The half-step allowance holds only if x·m̂ ≤ 1 + 10⁻⁷ for every row, which nothing asserts.
  Input that breaks it: a cluster whose mean is one row (a single row, or exact duplicates), centre stored int8
  with one scale as the fit does. Simulated: the key's error exceeds DA for 0% of such rows at n = 768 (largest
  1.5·10⁻⁵, just inside), 4.2% at n = 256, 16% at n = 100, 32% at n = 32, and 44% for rows with a few large
  coordinates at any n; largest errors 5·10⁻⁵ to 3·10⁻⁴ against an allowance of 1.5·10⁻⁵·|α|‖m‖ + 4·10⁻⁶. The
  estimate is then low by α·‖m‖·(x·m̂ − 1) and the certificate does not contain the score. On the integer path
  such an item is very likely not decided at all (its scale is tiny, so its norm code is the escape), which is why
  this may never have shown in the deterministic counts; the randomised levels and the f32 path read the same mr.
  Repair: drop the clamp's silent loss (assert |x·m̂| ≤ 1 + 10⁻⁷ at build and scale the key's range, or add the
  cluster's largest clamp excess to DA per cluster). The same key feeds `rmax` and the sorted range.

### 4. Ties and edge cases

- *Score exactly at the threshold.* IN is non-strict at every step (Θ − Rin ≥ Tin ⟹ lb ≥ t − tol), OUT strict at
  every step (Θ + Rout < Tout ≤ K_out/σ ⟹ ub < t). An item with score = t cannot be OUT while (A0) holds, and may
  be IN only if its lower bound equals its score. Θ + Rout = Tout is undecided. Correct strictness.
- *Zero residual.* s is floored, so K/σ is huge and both thresholds clamp: Tin to TMAX (no IN) or TMIN (a larger
  threshold than the true one), Tout to TMAX (smaller than the true one) or TMIN (no OUT). Sound. But see K3: the
  rows nearest their centre are the ones whose key is clamped.
- *Empty and single-row clusters.* `run_cluster` indexes `cands[0]`, so it must not be called with no candidates
  (callers' duty; a panic, not a wrong answer). One candidate: the block's other 31 lanes hold `u32::MAX`, are
  computed (keys read as 0, thresholds written, accumulators advanced) and masked out by `inm & act`,
  `outm & act`. Lanes past `n` rounded up to 8 keep stale thresholds and are masked the same way. No error found.
- *Norm code 15.* `rin[15] = rout[15] = None` → RSENT on both sides → never decides at that level; the item goes
  on, and a later level's code is looked up afresh. A NaN or out-of-range margin also becomes RSENT (NaN fails both
  range comparisons). Sound.
- *Widths that are not a multiple of the SIMD width.* The integer path has no ragged loop: a part's units are a
  multiple of 4 (asserted in `Part::pack`), so a level is whole 64-byte groups; a block is always 32 slots with
  padding; `int_thresholds` rounds n up to 8 (4 on AVX2) inside arrays padded to whole blocks plus one; the
  f32-to-integer hand-over at the first randomised level steps by 4 inside the same padding. `dot32` (cluster
  setup) has the scalar tail the f16 bug lacked; `qv_exact_v` asserts n % 32 == 0. The f16 pattern is not present.
- *Reused tables (latent).* `cq.luts[l]` is cached by level only. Tables built in Interval mode (per-part steps)
  would be reused on the integer path with step σ if one `CQ2` with `int` set were ever run in both modes. No
  caller does (the three `Mode2::Thr` call sites open a fresh cluster setup; the Interval call sites never enable
  the integer path). An assert that a cached deterministic table's step equals σ would make it impossible.

### 5. Numerical attack: summary

All through `run-limited.sh 2 1 2`, each under 20 s. Scripts are transcriptions from my reading of the code, not
runs of the binary.

| Script | What | Cases | Result |
|---|---|---|---|
| `k1_compare.py` | two-word saturating compare against exact integers, signed margins, clamps, sentinel | 4M per unit count + corner sweep | 0 mismatches at 64 and 127 units; wrong OUT from 128 units |
| `k2_thresholds.py` | Tin, Tout in f64 against exact rationals, ties at ±3 ulps, tiny s, large α, dial, tol | 1.7M | 0 violations |
| `k3_margins.py` | Rin, Rout in f64 against exact rationals at cancellation | 296k | 0 short |
| `k4_tables.py` | inequality (1) worst case over codes for own and combined tables; then decisions at thresholds on the boundary against the real certificate | 2 × 600 trials; 72,711 decisions near the boundary, 11,567 within a few steps | (1) never exceeded (worst 0.976 e); 0 decisions not implied |
| `k6_key.py` | m·r from the key: f32 evaluation; the clamp | all keys × 404 norms; 24,000 rows | K4 and K3 above |

I found no input on which an integer decision contradicts the real certificate (A0). The inputs that break things
are K1 (128 or more deterministic units) and K3 (the certificate itself, through the clamped key).

### 6. Verdicts

- **Theorem 1.19 as built: sound, with the proof in section 2**, given inequality (1) (section 3d), the thresholds
  (3b), the margins (3c) and the compare (3a). Restate it with B_ℓ the signed full sum, no centring constant, the
  dial on the OUT side only (D_max on the fixed term), and t − tol on the IN side. For exact queries (dial 1) the
  built decisions are the design's with the tighter signed margin. For a dial below 1 every IN is still certified at
  ψ = 1; OUT is statistical.
- **1.21′ (saturation) as built: sound for at most 127 deterministic units, with the proof in 3a; sound in general
  after one change** (K1: assert the unit count on the combined-tables path, where it is missing). Breaking input:
  128 units, Hs = 0, Ls = 32,639, Tout = −4,201,876, Rout = −4,234,514.
- **1.22 (centre tables) as built: sound, proof in 3d**, but it is a different construction from the proposition
  in the notes, and the notes' bound e_ℓ ≤ 3Uσ is false for it.
- **The f64 threshold allowance: sound, proof in 3b.** Editor's query 7's two open points are closed (the dial
  above 1 is covered for any dial; the slack is now argued in writing).
- **The certificate (A0) under all of them: holds only under preconditions the code does not assert** (K3: no row
  with x·m̂ > 1 + 10⁻⁷; K4: |α|·‖m‖ ≤ 23; decoded coordinate values below about 6). These are not integer-kernel
  matters, but an integer decision is implied by (A0) and by nothing stronger.

### 7. What I could not settle without the binary, and what to run

1. That my transcriptions match the compiled kernels (the lane orders of `order32`, `order16`, `look16`, the AVX2
   and AVX-512 `cmp2w`). I traced them by hand and found no error. The crate's own tests settle it:
   `two_word_compare_matches_i64`, `int_level_matches_scalar`, `int_thresholds_match_scalar`,
   `combine_lut_within_bound`, `part_sums2_at_the_largest_part` in `rust/src/v2/kern.rs`, once built for AVX-512
   and once for AVX2 without AVX-512.
2. K3 on a real index: build a small index at n ≤ 256 from a corpus that contains groups of exact duplicates large
   enough to be clusters of their own, use those rows as item queries, and run verify mode
   (`--v2 --index <dir> --in-memory verify`); read `cert_viol`, `viol_det`, `viol_rand` and wrong in/out. A
   non-zero `cert_viol` or `viol_rand` on those rows confirms it; zero does not clear it (the violation needs a
   threshold within about 10⁻⁴ of the row's score).
3. K4: print the largest |alr|·mn over opened clusters on the text queries. Above 23 the float share of the m·r
   allowance is short.
4. K1 cannot be shown by verify mode in practice (it needs a low-word sum above 32,512). Extend
   `two_word_compare_matches_i64` to Ls ≤ 255·units for the largest unit count a build accepts.
5. The bound "decoded coordinate value below 3.5" behind ewdet: assert it at build from the code tables.
