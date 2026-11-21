# Theory program: state

Kept by the PI. Written 21 November 2026 at the end of the first pass. The running log of that pass, with the
measurements the chapters cite, is `LOG-2026-11-21.md`. Statuses use the tags of `../README.md`.

## What entered the chapters

| Chapter | What | Status |
|---|---|---|
| 1, §1.2 | The cluster cut, the sorted range for the stored key (Lemmas 1.5a, 1.5b, 1.5) and the norm sort, restated by a referee | [proved, unrefereed]: the referee wrote the statements and proofs; no second reader |
| 1, §1.8 | The integer kernel as built: 1.19, 1.21′ (proof to 64 units; 127 units tested, a sketch), 1.22, the f64 thresholds and margins | [proved, unrefereed], as above |
| 1, §1.8a | Eight open defects of the build | [model] or [measured]; none reproduced in a binary |
| 2, §2.9a | The water-filling floor (Theorem 2.24, settling Conjecture 2.14), Theorem 2.25, Corollary 2.26 | [proved, refereed] |
| 3, §3.2a, §3.4 | The rounded-rotation allowance; what a seed promises; the seed as built | 3.25 to 3.28 |
| 3, §3.6a | R9 as worded refuted; the quantile form; the union over N attained for the plain rule, one block, a random code | 3.29 to 3.33 |
| 3, §3.7a | A lower bound for the confidence constant in a restricted class; the confidence tax | 3.34 to 3.50; of 3.25 to 3.50, 14 refereed and 12 unrefereed |
| 4, §4.9 | The hand-over, the rate–margin law and its dump test, the price of a stored scalar, the optimum with scalars charged | [model], [computed], [measured] |
| 5, §5.8a | The stop on expected misses dropped; the fixed-cluster-count replay | [measured] |
| 5, §5.9a | The level width as a measured rule with a stated condition | [measured]; one lemma [proved, unrefereed] |
| 5, §5.10.1a | The tree as a random energy model; two leaf floors | revised after a referee, not re-read |
| 7 | Eight things tried and ruled out | carried from the tracks |

## Owed

- A second reader for the referee-written proofs of chapter 1 and the twelve unrefereed results of chapter 3.
- The Editor's queries opened in this pass: chapter 1, 15 to 21; chapter 3, 13 to 20; chapter 4, 21 to 27; chapter 5,
  14 to 26.
- Floors: the extensions and the transport claims have a referee's report (`floors/referee/referee-extensions.md`) and
  have not entered the chapters.
- Confidence: the general decoder (G1), fixed δ (G2), the built code (G3), the factor L, the union for other rules and
  for several blocks, k-NN.
- Level width: a derivation of the slope; filters that select on the coded part of a row; the plain rule at equal
  rows.

## Needs the machine or a binary

- The build defects of chapter 1, §1.8a: a count at build of rows with |x·m̂| > 1 + DA; the two unit tests the range
  referee describes; the kernel tests on AVX-512 and on AVX2 alone. With the implementing session.
- The first randomised level: one wider block at the same 24 B, two blocks, and the `--design-exact` layout with
  phases recorded, so that the cost of a scalar read is measured and not inferred (chapter 4, §4.9).
- A fit at query weight 0.2 against the data-only fit and the fit at 0.5, on both query sets (floors B4d).
- The 24-scale encoder. Low priority: about one point of the last level's width, an encode loop four times longer,
  and the stored κ grid under a wider range of scales unchecked.

## Decisions taken by the owner (21 November 2026)

- The build defects are repaired now, with no separate step to confirm them first: the key clamp by adding each
  cluster's largest clamp excess to its key allowance, the root in the cut by putting the slack under the root, and an
  assert of at most 64 deterministic units. With the implementing session.
- The seed stays a constant for now. The guarantee is not offered against adversarial queries; it holds for queries
  chosen without regard to the seed.
- The first randomised level stays as built. The native run listed above (one wider block at the same 24 B, two
  blocks, the exact-design layout with phases recorded) is a todo, not scheduled.

- The basis is fitted to the data. A query sample is an optional input at build, mixed in with a query weight from 0
  to 0.5, default 0.2, on the trace-normalised covariances as `rust/tools/fit.py` forms them (it mixes at a fixed 0.5
  today). Any weight gives a sound index; only widths move. The weight is a dial, not a share of traffic. Owed before
  a benchmark is republished with it: one native fit at 0.2 against the data-only fit and the fit at 0.5, on both
  query sets (floors B4d has the figures by formula). With the implementing session.

## Rules for agents on this machine

Every job goes through `run-limited.sh` (cores, threads, address-space cap): one thread, nice 10, under 100 MB
resident unless the brief says otherwise, no command that walks the disk, no pass over a large data file, no builds,
no git changes. Ask the other running session which cores are free before any timing.
