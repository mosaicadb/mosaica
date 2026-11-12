# Improving the per-item code

Saved by the coordinating session from the agent's returned text; condensed. Every number is the
agent's. Scripts in this directory (`run2.py`, `build.py`, `report2.py`, `ablate.py`, `timing.py`,
`levels.py`, `probe1.py`, `curves.py`, `diag_norm.py`, `floor2.py`); results in `out/`, logs in
`logs/`.

## Summary

A finer schedule for the randomised levels and cheaper stored scalars cut per-item bytes read by
about 25% on every dataset, zero violations in every run. On time, the escape code loses to a flat
scan for text; a fixed-rate, table-decodable code costs only 1–4% more bytes and decodes 10–17×
faster. **Recommendation: drop the escape code for the fixed-rate tree code.**

The baseline includes the review's mixture weights at the deterministic levels; all rotations apply
the Haar sign fix; every code is reported in bytes and estimated ns.

Per-item bytes read (answers 0.01 / 0.1 / 1 / 5%), measured, zero violations:

| Dataset | Before (combined + mixture weights) | After (fixed-rate) | Change |
|---|---|---|---|
| BioCLIP-2, 1M, item queries | 2.83 / 6.89 / 24.8 / 78.5 | 2.12 / 5.21 / 18.7 / 58.5 | −24 to −26% |
| BioCLIP-2, 1M, text queries | 30.5 / 43.2 / 77.1 / 132.0 | 22.8 / 32.2 / 57.7 / 99.3 | −25% |
| ada-002 (n = 1,536) | 73.5 / 108.5 / 169.7 / 296.7 | 56.3 / 81.9 / 134.0 / 250.8 | −23 to −15% |
| 3-large (n = 3,072) | 132.0 / 189.6 / 355.1 / 813.4 | 101.3 / 141.4 / 294.7 / 763.5 | −23 to −6% |
| GloVe (n = 100) | 23.6 / 32.7 / 51.9 / 88.7 | 18.1 / 25.2 / 37.9 / 57.0 | −23 to −36% |
| BioCLIP-2.5 (n = 1,024) | 2.63 / 5.15 / 13.6 / 31.3 | 1.89 / 3.74 / 10.0 / 23.3 | −28 to −26% |

Totals at N = 10⁹ (all costs, % of dataset), BioCLIP-2 1M: items 0.230 / 0.498 / 1.670 / 5.184% →
0.184 / 0.389 / 1.274 / 3.883%; text 2.052 / 2.878 / 5.087 / 8.674% → 1.549 / 2.163 / 3.822 /
6.543%. Absolute thresholds 0.95 / 0.9: 0.79 / 1.64 → 0.60 / 1.23 B per item.

What carried weight (marginal, in order added):
- **Finer randomised schedule:** −12 to −21% on text-like queries (most text items are decided
  between 96 and 192 B; the doubling schedule made them read the whole jump).
- **Cheaper scalars:** −3 to −8% (4-bit relative deterministic norms; relative codes for block
  norms and tan φ).
- **Free item scale and residual norm from the stored sort key** (unit-norm identity gives ‖r‖
  exactly): −1 to −8%.
- **Dropping the level-0 norm:** −1 to −5% (after the prefilter level 0 decides nothing).
- **Finer deterministic schedule:** −0 to −12%, up to +0.8% on text sets.

## The code as kept

1. Fixed-rate nested tree quantiser at 4 / 8 / 12 / 16 / 24 / 32 B; bits per coordinate from a
   greedy nested allocation on Σ√(λ·Q_mix·g_tree(b)); one table lookup per coordinate to decode.
   Sound: stored rounded-up weighted error norm + weighted Cauchy–Schwarz holds for any code.
2. No level-0 norm; the 2-byte sort key is read with the first code level.
3. Free item scale s = ‖r‖/√Σλ from ‖r‖² = 1 + ‖m‖² − 2‖m‖â; divides coordinates before
   quantising.
4. Sphere in the certificate: ellipsoid ∩ {‖z‖² ≤ R²} via a Lagrangian dual, closed form for 5
   values of κ (sound by weak duality).
5. 4-bit relative error norms (2^(1/16) grid, offsets calibrated at build on 1/8 of each group's
   clusters; escape charged one extra byte, 0–0.4% rate).
6. Randomised levels as built except: block norms 1 B relative (2^(1/256) grid), tan φ 4 bits
   relative (2^(1/20) grid), absolute fallback on either side; schedule 40, 48, 56, 64, 72, 80, 96,
   112, 128, 144, 160, 192, 224, 256, 320, 384, 480, 576 B; union bound over all 18 levels,
   c ≈ 9.47.
7. Stored per item (p768): 37 B through 32 B, ~601 B through 576 B.

A bug found and fixed: the first relative block-norm code rounded values below the grid up to its
minimum, breaking the reconstruction-scale bound (542 violations). Fallback now on both sides;
0 violations since.

## Bytes against time (estimated from the review's measured decode rates)

| Set, queries | Escape (final) | Fixed-rate, byte / nibble table | Flat scan |
|---|---|---|---|
| BioCLIP-2 1M, text | 208 / 265 / 377 / 484 ns | 13 / 19 / 33 / 55, or 5 / 7 / 12 / 21 ns | 76 ns |
| BioCLIP-2 1M, items | 25 / 55 / 161 / 371 ns | 1.2 / 3.0 / 10.7 / 33 ns | 76 ns |
| ada-002 | 454 / 497 / 538 / 580 ns | 33 / 47 / 71 / 109 ns | 152 ns |
| 3-large | 529 / 550 / 596 / 674 ns | 58 / 77 / 123 / 202 ns | 304 ns |
| GloVe | 235 / 313 / 406 / 465 ns | 10.5 / 14.5 / 21 / 28 ns | 10 ns |

Fixed rate costs +1.5–2.0% bytes on BioCLIP-2, +0.5–1.4% on ada and 3-large, +2.3–3.7% on
BioCLIP-2.5, mixed on GloVe. At n = 100 a flat scan is as fast as any coded stream.

## Distance from the floor (computed, `floor2.py`)

Deterministic levels, real residuals, Gaussian offsets, every stored bit counted: BioCLIP-2
1.34–1.37× → 1.17–1.21× above the conjectured floor (1.60–1.86× → 1.42–1.66× above the proved
floor); BioCLIP-2.5 1.30–1.31× → 1.17–1.18× (1.58–1.78× → 1.44–1.65×). At the early levels most
of the gap was stored scalars, not quantiser shape. Remaining: per-coordinate scalar loss vs the
rate–distortion bound (1.13–1.46×), and the fixed-rate tree code in deep levels (~1.1–1.25× vs
entropy coding). E8 could recover ~7% of width at depth ≥ 3 (conjectured; not built).

## Contradicts earlier reports

- Fixed-rate tree code is within 1–4% of escape once scalars are charged equally (agent 1 had
  escape ~15% ahead).
- Exact box ∩ ellipsoid range gains ~0.3%, not 3–14%.
- Level 0 is pure cost after the prefilter.
- The rotation sign fix moves no figure by more than 0.05%.
- m·r and ‖r‖ are the same scalar.

## Tried and dropped

Box ∩ ellipsoid (−0.3%); level-0 ellipsoid in the dual (<0.3%); escape grid widths other than
0.6745; other deterministic schedules; per-item relative tan φ (kept, nearly worthless). Not built:
lattice/trellis codes, per-cluster variance profile, more local bases, duplicate pointers (none in
these sets), entropy coding in randomised blocks (~3% on text, not table-decodable).

## Caveats

Earlier p768 ablation rows used unfixed rotations (≤0.05% effect). 1M: 200 item, 369 text queries;
ada / 3-large / GloVe / BioCLIP-2.5: 200 item queries on 115k / 47.6k / 200k / 200k items.
Relative-norm offsets calibrated on the corpus; mixture profile fitted on `text_train` only. Code
lengths exact but no bit-serial encoder; times are estimates, not measurements.
