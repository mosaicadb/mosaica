# Entropy gap of real embedding residuals

Saved by the coordinating session from the agent's returned text; condensed. Every number is the
agent's. Scripts in `scripts/`, logs and result files in `logs/`, data in `data/`.

## Summary

Within-item non-Gaussianity is small and mostly not exploitable; the agent does **not** recommend a
research round on modelling data structure. Every gap is a **lower bound** on J (held-out
cross-entropy under explicit models).

| Dataset (n) | Per-item J, bits (scale models → with ICA rotation, top-384) | Floor change | Bits saved by a usable code at matched distortion | Finer mixture | Neighbour prediction |
|---|---|---|---|---|---|
| BioCLIP-2 (768) | 80 total; 23 → ~62 | ×0.93 → ×0.89 | 1.9–4.0% | ~9% | 3.5% |
| BioCLIP-2.5 (1,024) | 271 total; 100 → ~133 | ×0.83 → ×0.79 | 4.9–10.4% | ~13% | 4.8% |
| GloVe (100) | 0.8 | ×0.99 | 0.2–0.6% | 2.6% | none |
| NYTimes (256) | 3.8 | ×0.99 | 0.2–0.6% | 7% | 24% (17.4% identical items) |
| ada-002 text (1,536) | 23 in top-1,024; 6 → ~69 | ×0.99 → ×0.88 | 0.4–0.7% | 3.8% | 4.2% |
| 3-large text (3,072) | 5 in top-1,024; 1.5 → ~58 | ×1.00 → ×0.90 | ≤0.2% | 1.4% | 1.5% |

Finer-mixture and neighbour columns at a budget of n bits (96 B at n = 768).

- **General across datasets:** nothing above ~1% from marginal or scale modelling; finer local
  Gaussian models help everywhere but by 1–13%.
- **Image embeddings only:** heavy tails, almost entirely per-cluster and per-item scale variation,
  which a cheap code captures.
- **OpenAI text embeddings only:** sparse independent (ICA) directions, ~0.15–0.18 bits per
  coordinate, but a scalar code in that basis loses 3–6× more than it gains in the bound's metric.
- **One dataset only:** duplicates (NYTimes).
- **Bytes read:** at most 3–8% on BioCLIP, under 1% on text. Closing the escape code's 1.36–1.42×
  quantiser gap is worth roughly 1.3–1.6× bytes by the same law (conjectured transfer).

**Recommendation:** fold in three cheap generic items instead of a round: duplicate pointers, a
per-cluster variance profile with a per-item scale class, and more local bases where memory allows.

## Setup

k-means (~1,000 items per cluster), 16 or 8 groups, PCA per group, coordinates ordered by γ = Q·λ
with 200 corpus items as queries. 80% fits means and bases, 10% fits model parameters, 10%
evaluates (swapped and averaged). Reference: per-group Gaussian with held-out variances. Control:
synthetic Gaussian data with BioCLIP-2 spectra gives J between −0.7 and 0.0 bits. Stored
coordinates stop at the top 1,024.

Datasets: BioCLIP-2 768 (local, 200k); BioCLIP-2.5 1,024 (local, 200k); GloVe-100 (ann-benchmarks,
200k); NYTimes-256 (ann-benchmarks, 199,850); ada-002 1,536 DBpedia
(`KShivendu/dbpedia-entities-openai-1M`, 115k); text-embedding-3-large 3,072 DBpedia (Qdrant
repository, 47.6k). `data/te3_1536.npy` is a second ada-002 sample (used as a replicate only).
Not obtained: ann-benchmarks DBpedia (404), Cohere Wikipedia (401).

## 1. Marginal negentropy (bits per item, top-k)

| Dataset | Kurtosis by band | Beyond 3.94σ | Marginal J at k = 128 / 256 / 512 / all |
|---|---|---|---|
| BioCLIP-2 | 3.6–4.7 | 0.18% (max 16σ) | 1.6 / 4.4 / 10.9 / 16.5 |
| BioCLIP-2.5 | 5.1–9.2 | 0.47% (max 30σ) | 7.5 / 19.1 / 43.9 / 90.4 |
| GloVe | 3.1–3.2 | 0.02% | 0.1 in total |
| NYTimes | 3.1–3.5 | 0.02% | 0.1 in total |
| ada-002 | 3.1–3.2 | 0.03% | 0.0 / 0.2 / 0.5 / 1.7 (top 1,024) |
| 3-large | 3.1 | 0.02% | 0.2 (top 1,024) |

Heavy tails are real on BioCLIP only. The escape code's implied density is 14–111 bits worse than
the Gaussian at high resolution on every dataset, so the code is not already banking this gain.

## 2. Dependence beyond second order

Scale structure (bits saved against the group Gaussian): per-cluster variance profile + per-item
scale gives BioCLIP-2 10.3 / 23.7 / 52.8 / 80.0 at k = 128 / 256 / 512 / 768; BioCLIP-2.5
28 / 63 / 135 / 271 at k = 128 / 256 / 512 / 1,024; ada-002 3.8 / 23.3 at k = 256 / 1,024.
Once scale is removed the PCA coordinates are Gaussian.

Matched distortion (same quantiser cells, ideal code lengths; achievable):

| Dataset | Budget | Saved by profile + item scale | Half-width at fixed bytes |
|---|---|---|---|
| BioCLIP-2 | 32 / 96 / 192 / 384 B | 4.0 / 3.3 / 2.7 / 1.9% | ×0.973 / 0.977 / 0.966 / 0.976 |
| BioCLIP-2.5 | 43 / 128 / 256 / 512 B | 10.4 / 8.3 / 6.6 / 4.9% | ×0.933 / 0.945 / 0.920 / 0.940 |
| ada-002 | 64 / 192 B | 0.7 / 0.7% | ×0.995 |
| GloVe, NYTimes, 3-large | all | ≤0.6% | ≥×0.995 |

Finer Gaussian mixture, bits saved at budgets n/3, n, 2n, 4n (extra cluster-id bits charged):

| Dataset | 4× clusters | 16× clusters | 4× groups | 4× both |
|---|---|---|---|---|
| BioCLIP-2 | 16 / 20 / 20 / 20 | 23 / 30 / 34 / 36 | 42 / 62 / 65 / 68 | 47 / 67 / 69 / 72 |
| BioCLIP-2.5 | 32 / 53 / 68 / 81 | 42 / 70 / 91 / 109 | 71 / 110 / 133 / 156 | 84 / 138 / 170 / 203 |
| ada-002 | 11 / 14 / 15 / 14 | 42 / 59 / 66 / 66 | 12 / −39 / −73 / −74 | 37 / −1 / −33 / −34 |
| 3-large | 30 / 29 / 25 / 25 | 43 / 44 / 43 / 43 | −175 to −554 | −165 to −538 |
| NYTimes | 8 | 18 | 7–8 | 12 |
| GloVe | 1.6 | 2.2 | 1.5 | 2.5 |

More local bases is the largest single gain on BioCLIP (8–10% at n bits) and turns negative on the
text sets (too few items per basis at 1,536–3,072 dimensions).

Independent-component rotation: real negentropy on text only (ada-002 k = 384: 64.7 bits vs 2.3 in
PCA basis; 3-large 56.8 vs 0.2), but the scalar-code penalty of the ICA basis exceeds the gain 3–6×
(e.g. 3-large k = 384: penalty 336 bits). Band-restricted rotations untested.

## 3. Prediction from neighbours

| Dataset | NN energy / cluster-residual energy | cos > 0.99 | Identical | Net saving at n bits | At 2n bits |
|---|---|---|---|---|---|
| BioCLIP-2 | 0.76 | 5.4% | 0% | 26.8 (3.5%) | 44.8 |
| BioCLIP-2.5 | 0.74 | 6.1% | 0.01% | 48.7 (4.8%) | 91.1 |
| GloVe | 1.07 | 0% | 0% | −1.4 | 0.5 |
| NYTimes | 0.83 | 20.2% | 17.4% | 61.0 (24%) | 119.3 |
| ada-002 | 0.96 | 0.1% | 0% | 63.9 (4.2%) | 90.2 |
| 3-large | 1.21 | 0% | 0% | 45.6 (1.5%) | 60.5 |

Gaussian-rate estimates with pointer and bin index charged. Certificate cost (the neighbour's own
half-width adds) and decoding chains not measured. Savings grow with N on ada-002 and NYTimes and
shrink on BioCLIP.

## 4. Other

Unit-norm identity makes the stored m·r scalar redundant once ‖r‖ is known: ≤16 bits per item
(conjectured). Duplicate pointers: generic mechanism, dataset-dependent yield.

Scale models, finer mixtures and neighbour prediction overlap; combined gain expected well below
the sum (conjectured).
