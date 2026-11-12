# Agent 2 — queries that are not item-like

Saved by the coordinating session from the agent's returned text (the agent could not write files
itself). Condensed; every number is the agent's. **Not independently verified.**

Setup: 200k subset (`e00, e05, e10, e15`), n = 768, K = 200 clusters, G = 12 local bases
(`prep.py`, construction of `exp2.py`). Zero violations in every run.

## Queries

**Real text embeddings were used.** `fetch_text_tower.py` range-reads BioCLIP-2's text tower
(495 MB); `encode_text.py` runs it on CPU via `open_clip`. Check: assigning 90,217 labelled images
to the nearest of 120 species prompts gives 91.6% top-1 (`geom.py`). Test set: 369 prompts
(`text_q.json`: 120 "a photo of <Genus species>", 60 full taxonomic strings, 30 bare names, 29
common names, 60 higher taxa, 50 free descriptions, 20 species absent from the sample). A
disjoint 2,206-prompt set is used only for fitting. Item baseline: 100 corpus items. Stand-in
extreme: 30 random unit vectors.

Thresholds are per query, at answer sizes of 0.01% / 0.1% / 1% / 5% of the sample (20 / 200 /
2,000 / 10,000 items). Mean thresholds: items 0.928 / 0.903 / 0.863 / 0.797; text 0.703 / 0.681 /
0.647 / 0.605 (free prompts 0.52 / 0.50 / 0.48 / 0.45).

Geometry: BioCLIP-2's modality gap is small (text and image means have cosine 0.92; gap vector
norm 0.31, 17% of text offset energy). One text query's scores have sd 0.052 (items 0.098); the
0.1% threshold sits 0.185 above the median (items 0.32). A model with a larger gap would do worse.

## Owner's metric (dataset read, d = 0, FINDINGS-style accounting)

| | 0.01% | 0.1% | 1% | 5% |
|---|---|---|---|---|
| Item queries, existing scheme | 0.48% | 0.75% | 1.54% | 3.79% |
| Text queries, existing scheme | 3.17% | 4.32% | 7.06% | 12.37% |
| — species and taxonomic names (180) | 1.9% | 2.7% | 4.8% | 9.6% |
| — higher taxa (60) | 2.8% | 3.7% | 6.1% | 10.5% |
| — free descriptions (50) | 7.2% | 9.5% | 14.2% | 21.5% |
| — species not in the sample (20) | 4.2% | 6.2% | 10.9% | 18.2% |
| Text queries, after fix | 2.54% | 3.24% | 4.91% | 8.10% |
| Item queries, after fix | 0.49% | 0.75% | 1.42% | 3.11% |
| Random directions (stand-in extreme), existing | 17.8% | 21.0% | 27.4% | 37.7% |

A tolerance buys almost nothing for text at small answers (3.17% → 3.15% at d = 0.02).

With honest scalar accounting (4 B per deterministic level read, per-block scalars per split
level): items existing 0.89 / 1.25 / 2.21 / 4.78; items fix (2 B per block) 0.93 / 1.31 / 2.25 /
4.55; text existing 4.26 / 5.53 / 8.47 / 14.05; text fix (2 B per block) 4.02 / 4.98 / 7.10 /
10.97; text fix (1 B per block) 3.69 / 4.54 / 6.47 / 10.03.

## Damage by code size

Undecided per true match (0.01% / 0.1% / 1% / 5%) and mean half-width over matches (at 0.1%):

| Code | Items, undecided | Items, hw | Text, undecided | Text, hw |
|---|---|---|---|---|
| 32 B | 86 / 31 / 8.7 / 5.2 | 0.043 | 2894 / 364 / 49 / 13.7 | 0.155 |
| 96 B | 9.3 / 5.4 / 2.4 / 1.8 | 0.0185 | 845 / 132 / 21.9 / 7.2 | 0.081 |
| 192 B | 2.3 / 1.5 / 0.84 / 0.63 | 0.0075 | 160 / 35 / 8.5 / 3.6 | 0.039 |
| 384 B | 0.67 / 0.42 / 0.24 / 0.18 | 0.0022 | 11.6 / 4.3 / 1.9 / 1.1 | 0.012 |

## Why

At 96 B (item-fitted bits and weights), items vs text: ‖Δ‖ 0.37 vs 0.73; ‖e‖ 0.083 vs 0.127;
plain bound 0.0337 vs 0.0933; weighted 0.0173 vs 0.0811; per-pair ℓ1 0.0109 vs 0.0441; actual
|Δ·e| 0.0008 vs 0.0027; Δ energy in uncoded dims 5% vs 26%.

Energy share by local-dimension band (0–8 / 8–32 / 32–64 / 64–128 / 128–256 / 256–512 /
512–768): item offsets 26 / 31 / 16 / 14 / 9 / 4 / 0.2%; residuals 23 / 30 / 17 / 15 / 10 / 5 /
0.2%; text offsets 12 / 14 / 12 / 15 / 20 / 23 / 4%.

Bit allocation is not the cause (3–5%). Score density is not the cause. The damage is the width
of the bound: at 0.08 it reaches into the bulk of a score distribution whose sd is 0.052. The
actual error is 29× below the certified one for text (21× for items).

## Fixes

**Deterministic repairs (sound, small).** For any α, q·x = q·m + γ q·c + α m·e + (q − αm)·e; the
optimal α* = ⟨Dq, Dm⟩/‖Dm‖² is free and worth keeping (random directions −30%). A text-fitted
second weighting, banded error norms and mixture-fitted bits gain 10–23% uncharged and cost more
than they save once their scalars are charged.

**The split certificate covers text queries with the same failure probability** (the argument
uses nothing about q). Measured tail at c = 8: largest error 5.59–5.86 predicted sd, 0 violations.

**As built in `exp8.py` it loses at 96 B and below** (text 0.118 vs 0.081). Fix: allocate for the
split bound itself (`alloc_split` in `run3b.py`). Predicted bound is
c·√(Σ_coded γᵢ tan²φ(bᵢ)) + Σ_uncoded √γᵢ; a dimension coded alone costs ~6√γᵢ but a wide 1-bit
block costs c·0.756·√(Σγᵢ), so give the m dimensions of largest γ one bit each, spread the rest
by MSE, choose m to minimise the bound, with γᵢ = λᵢQᵢ from the mixture profile
Q = ((√Q_item + √Q_text)/2)². At 96 B this codes 456 dimensions, 329 at one bit.

Half-width over matches at 0.1%, det → split-opt: items 32 B 0.0433 → 0.0396; 96 B 0.0185 →
0.0111; 192 B 0.0075 → 0.0034; 384 B 0.0022 → 0.0009. Text 32 B 0.155 → 0.129; 96 B 0.081 →
0.041; 192 B 0.039 → 0.0136; 384 B 0.012 → 0.0028. Text undecided at 0.01% / 0.1%: 96 B
845 / 132 → 127 / 32; 192 B 160 / 35 → 7.7 / 4.0.

The metric "after fix" uses: deterministic levels 0, 4, 8, 16 B, then split levels 32, 48, 96,
192, 384 B, then the full vector (the whole 32 B charged to every item undecided after 16 B).

## Why text stays ~5× dearer

For text at 0.01% the fix reads 39.1 B per item: scalars 4.0 (100% of items), det 4 B 3.4 (85%),
det 8 B 2.8 (69%), det 16 B 4.7 (59%), split 32 B 14.4 (45%), split 48 B 3.3 (20%), split 96 B
5.1 (11%), beyond 1.5. Three quarters of the cost is spent before any certificate is narrower
than 0.1, which is two sd of the score distribution. Conjectured: no certificate at c = 8 prunes
the bulk in 16 B.

## Dimension

Validity is independent of n. The randomised gain scales as √n/c. On a PCA-truncated n = 192
stand-in (`make_n192.py`, not a real model), text half-width det → split-opt: 0.118 → 0.109 at
1 bit/dim, 0.073 → 0.048 at 2 bits/dim (n = 768: 0.081 → 0.041, 0.039 → 0.0136). At n = 192 the
fix is worth 6% for text and is a net loss with scalars charged. The real 1,024-dimension set
was not fetched.

## Not built / limits

- A jointly diagonalising basis is predicted (not built) to give −24% for text and +21% for
  items at 96 B.
- An exact gap direction or query subspace: not built (gap holds 17% of offset energy; 16
  principal directions of the test queries hold 47% of their variance).
- One model with an unusually small modality gap; 369 prompts written by the agent, not a query
  log; 200k items from four slices; levels measured as independent allocations; no timings;
  split results at c = 8 only.

## Files (this directory)

`fetch_text_tower.py`, `fetch_meta.py`, `encode_text.py`, `encode_train.py`, `prep.py`,
`harness.py`, `geom.py`, `qprofile.py`, `explain.py`, `genbasis_pred.py`, `checks.py`, `run1.py`,
`run2.py`, `run3.py`, `run3b.py`, `hybrid.py`, `make_n192.py`, `n192/`, logs and `res*.npz`.
`pylib/` holds torch and open_clip (945 MB); `model/` holds the text tower.
