# Agent 1 — tighten and prove

Saved by the coordinating session from the agent's returned text (the agent could not write files
itself). Condensed; every number is the agent's. **Not independently verified.**

Setup: 200k subset (`e00, e05, e10, e15`), K = 200 clusters, G = 16 local bases, 100 corpus items
as queries, 20,000,000 pairs, thresholds 0.95 / 0.9 / 0.8 / 0.7, one thread. **Zero violations in
every run.** The subset is denser than the 1M sample (true matches 2.2 / 4.8 / 9.5 / 17.3% of
pairs): compare rows within this report only. Scripts: `h.py`, `h2.py`, `theory.py`; logs
`h_v1.log`, `h2_v2.log`, `h2_n128.log`, `theory.log`; results `res_v1.json`, `res2_v2.json`,
`res2_n128.json`.

## 1. Owner's metric (% of dataset read, d = 0) [measured]

| Code, accounting | 0.95 | 0.9 | 0.8 | 0.7 |
|---|---|---|---|---|
| Baseline: uniform clipped quantiser, independent allocations treated as nested, 4 B scalars flat | 0.622 | 0.827 | 1.946 | 5.724 |
| Same + e ⟂ code refinement | 0.619 | 0.821 | 1.935 | 5.705 |
| Real nested code, naive bit planes, same accounting | 0.673 | 0.871 | 2.033 | 5.981 |
| Real nested code, tree-structured (companded) quantiser, same accounting | 0.571 | 0.776 | 1.816 | 5.231 |
| Real nested escape code, same accounting | 0.527 | 0.697 | 1.502 | 4.245 |
| Tree-structured, honest scalar accounting (2 B per level) | 0.599 | 0.874 | 2.159 | 5.936 |
| **Escape code, honest scalar accounting (1 B per level)** | **0.511** | **0.720** | **1.628** | **4.478** |

With tolerance: escape honest d = 0.01: 0.392 / 0.613 / 1.489 / 4.003 (baseline 0.420 / 0.621 /
1.619 / 4.630); d = 0.05: 0.322 / 0.513 / 1.306 / 3.269 (baseline 0.345 / 0.502 / 1.291 / 3.601).

## 2. The deterministic floor

**The FINDINGS claim is false as stated**: (a) exact real scalars can carry all of r, so there is
no floor; (b) even for well-behaved exact scalars the feasible set has zero volume; fix: count
the scalars' bits in B. (c) The γ formula assumes Gaussian residuals; real data has lower entropy
so its true floor is lower.

**Theorem A (volume) [proved].** Δ = Q^½ g, g standard Gaussian, r' = Q^½ r, S any k-dim
subspace, h_S the differential entropy of the projection of r' on S, H(c) the entropy of the code
word (fixed or variable length, scalars included). Then
E[h] ≥ E‖g_k‖ · vol(B_k)^(−1/k) · exp((h_S − H(c))/k).
For Gaussian r: E[h] ≥ κ_k · (γ₁⋯γ_k)^(1/2k) · 2^(−B/k) for every k, κ_k ≥ k. At the reverse-
water-filling k this is the head term of the FINDINGS floor.
Proof: soundness ⇒ h ≥ half-width of the cell in direction g; projecting on S does not increase
mean width (Jensen on the support function); Urysohn; h(P_S r' | c) ≤ log vol of the projected
cell and I(P_S r'; c) ≤ H(c); Jensen on exp(·/k).

**Theorem B (concentration) [proved]:** E[h] ≥ Σ_T √γᵢ − (√(2H(c)) + 1)·√(Σ_T γᵢ) via Gaussian
isoperimetry. **Theorem C** combines A and B. B and C are never binding on the measured spectra,
so the tail term of Σᵢ √min(γᵢ, θ) remains a conjecture at low rates.

On the measured spectra (`theory.py`; bits include 48 bits of scalars):

| Code bytes | 0 | 4 | 16 | 32 | 96 | 192 | 384 | 576 |
|---|---|---|---|---|---|---|---|---|
| best proved / conjectured | 0.70 | 0.73 | 0.79 | 0.83 | 0.93 | 0.96 | 1.00 | 1.00 |
| baseline uniform code / best proved | 2.28 | 1.92 | 1.69 | 1.59 | 1.52 | 1.66 | 2.65 | 7.4 |
| escape code / best proved | 2.28 | 1.94 | 1.66 | 1.53 | 1.39 | 1.40 | 1.38 | 1.36 |

(Rows 2–3 measured on synthetic Gaussian data with those spectra; no code beat a proved floor.)
The theorem does not cover randomised certificates, offsets correlated with the residual, or
non-Gaussian structure.

## 3. Closing gaps

- **Escape code [main gain].** Per coordinate u = z/σ: depth 1 is the sign; depth 2 is the index
  of |u| on a grid of width 0.6745, in unary; each further depth halves the cell, to depth 14.
  Every cell is bounded, nothing overflows. Steps follow one greedy order per group; each item
  spends a fixed number of bytes per level and gets as far down the order as its bits allow — a
  plain embedded prefix code with fixed-size levels. 0.23% of coordinates lie beyond 3.94σ
  (Gaussian: 0.008%); largest 18.3σ. Items undecided after 576 B fall from 0.035 / 0.022 / 0.033
  / 0.058 per true match to 0.005 / 0.003 / 0.003 / 0.008; full-vector reads fall from 12–18% of
  bytes to 2–3%.
- **Companded tree-structured quantiser:** 6–9% fewer bytes than the baseline.
- **Entropy coding under a Gaussian model:** slightly worse than unary (heavier real tails).
- **e ⟂ code refinement:** 0.2–0.6% fewer bytes. Not worth it.
- **Lattice / vector codebooks:** not attempted; at most 1.36–1.40× in half-width remains.
- **Sharper c [proved and computed, not run on the metric]:** exact sphere-coordinate tail
  shrinks a single block's half-width by 0.54–0.94 depending on m and c; with the 96 B block
  sizes the multi-block factor is 0.93 / 0.89 / 0.78 at c = 6 / 8 / 10. Union bound over
  seed-independent undecided items: c from 9.43 to ~8.8–9.3 (N = 10⁹, L = 10, ε = 10⁻⁹). c = 6
  gives no simultaneous guarantee at a billion items. Expected total effect 5–10% [conjectured].

## 4. Nested-stream approximation

Real nested codes move the baseline by +4 to +8% (naive bit planes) or −6 to −9%
(tree-structured). The larger error is the scalars: each level needs its own error norm. Needed:
m·r once (2 B); one weighted norm per level on a grid of ratio 2^(1/16) (1 B); a scale only for
fixed-length codes (1 B). Coarser level schedules were 16–38% worse.

## 5. Instance-optimality

**Theorem D [proved].** For a fixed stream, any deterministic prefix-reading algorithm correct on
every corpus reads at least ℓ*(x, q, t) levels of every item (the first level at which every unit
vector sharing x's prefix lies on one side of t); the progressive scheme with the exact range
reads exactly ℓ*. Against byte-granular readers the factor is at most 2 (3.7 at the final jump).
Gaps: the scheme uses weighted Cauchy–Schwarz, not the exact range; no per-query bound across all
encodings.

## 6. Dimension

Theorems A–D hold for every n. At n = 128 (PCA truncation of the same vectors, not an independent
embedding; full vector 256 B):

| % read, d = 0 | 0.95 | 0.9 | 0.8 | 0.7 |
|---|---|---|---|---|
| Baseline | 2.39 | 3.06 | 6.38 | 15.79 |
| Escape | 2.30 | 2.93 | 5.96 | 13.89 |
| Escape, honest scalars | 2.14 | 2.94 | 6.49 | 15.07 |

Percentages are 3–4× higher because fixed per-item costs are a larger share of a 256-byte vector.

## 7. Not done

Lattice codes; the 1,024-dimension set; the sharper c and the split certificate on the metric;
timings; a real bit-serial encoder (code lengths are exact but no bytes were written); resident
memory for bases, means and per-level weights is still uncounted.
