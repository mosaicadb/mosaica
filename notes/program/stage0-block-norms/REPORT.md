# Stage 0: block norms for out-of-distribution queries

Written by the PI from the experimentalist's hand-back of 21 November 2026 (the agent could not write this file itself).
`NOTEBOOK.md` has the setup decisions and the log; the full tables are `work/report_measure.txt` and
`work/report_quant.txt`; the scripts are `prep.py`, `measure.py`, `quant.py`, `report.py` and `report_quant.py`.

## Verdict

**Dropped as a remedy for out-of-distribution queries.** Every figure below is [measured] on the 200k BioCLIP-2 sample
(K = 200, G = 12, dual basis per group), with 369 held-out text prompts, 361 in-sample species prompts and 100 held-out
items, each scored against all 200,000 items.

- Storing J block norms in place of one cuts the text half-width by 10% or more only with J ≥ 8, and in the idealised
  setting only at the shallow levels.
- From J = 4 the norms cost more bytes over the levels than they save at an equal undecided share. Only J = 2 pays, for
  held-out text alone, by 0.7 to 1.8 B per item.
- The gain is not specific to text: items and in-sample prompts gain 3 to 11% as well.
- The remedy the index ships, a basis fitted on a mixed query sample, cuts the text half-width by 21 to 33%. It works
  through the basis, which no change to the norms reaches. It costs item queries 6 to 19%.

## The numbers that decide it

Half-width as a ratio to one norm with data-only weights (a), idealised setting, at k = 0 / 32 / 256 exact coordinates:

| Bound | Text held-out | Items |
|---|---|---|
| One norm, weights fitted on text, same basis | .91 / .94 / .98 | 1.16 / 1.06 / 1.03 |
| Basis and weights fitted on the mixed sample | .79 / .73 / .67 | 1.06 / 1.08 / 1.19 |
| Block norms, equal variance, J = 4 | .90 / .94 / .97 | .96 / .99 / .99 |
| Block norms, equal variance, J = 16 | .84 / .90 / .95 | .89 / .95 / .97 |
| Per-coordinate absolute values (the best diagonal weighting for each pair) | .57 / .59 / .62 | .63 to .64 |

In a model quantiser at 4 to 32 B the same ratios are .91 to .92 (J = 4), .87 to .89 (J = 16) and .71 to .76 (mixed
basis) for held-out text.

Bytes saved at an equal undecided share against the cumulative cost of the norms, held-out text, 0.01% answers, after
the levels at 4 / 8 / 12 / 16 / 24 B: J = 2 saves 2.3 to 3.2 B for 0.5 to 2.5 B; J = 4 saves 3.2 to 4.6 B for 1.5 to
7.5 B; J = 16 saves 4.8 to 6.8 B for 7.5 to 37.5 B. The mixed basis saves 11 to 17 B and stores nothing more.

Soundness: 0 violations over 830 queries × 200,000 items × every bound and level.

## What it shows

The best diagonal weighting for each (query, item) pair sits at about 0.6 of the single norm for every kind of query,
items included. So the looseness of weighted Cauchy–Schwarz is the chance alignment of the query's weights and the
item's error inside a pair, not a mismatch between the query and the profile. Blocks cut by position recover about a
fifth of it at J = 16. For queries the fit did not see, the basis is the lever.

## Limits

- Neither setting is the built code: no tree fields, int8 basis, 16-bit tables, cut, sorted range or randomised levels.
- 200k items and K = 200; one dataset; one ridge value; the cost conversion is static, not bytes read by a pipeline.
- The held-out prompts share templates with the training prompts, which favours the mixed basis.
- Correction to the brief: `text_insample.npy` is not the training set; the fits used `text_train.npy`.

## Leads left open

- J = 2 equal-variance blocks at the first one or two levels only.
- The mixed basis helps text and costs items, so a basis per query family, chosen by the query, would give each its
  best at the price of storing a second code for the early levels.
