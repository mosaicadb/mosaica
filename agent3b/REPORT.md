# Agent 3b — read less: lower bounds and not touching every item

Saved by the coordinating session from the agent's returned text (the agent could not write files
itself). Condensed; every number is the agent's. **Not independently verified.**

Setup: 200,000 vectors, 200 corpus items as queries (40M pairs per threshold), thresholds
0.95 / 0.9 / 0.8 / 0.7, single thread. Two sets: n = 768 (`data/e00,e05,e10,e15`) and n = 1,024
(`agent3b/data1024/f00..f03`, BioCLIP-2.5 ViT-H/14, fetched by `fetch1024.py`). Every
deterministic bound checked on every pair: 0 violations in every run. Baseline replica is
`base.py`, a port of `exp2.py` "local" (levels 0/4/8/16/32/48/96/192/384/576 B then the full
vector, 4 B of scalars per item). The 200k subsets are denser than the 1M sample: compare with
the replica rows, not FINDINGS section 1.

## Headline (% of dataset read, exact, deterministic)

| | 0.95 | 0.9 | 0.8 | 0.7 |
|---|---|---|---|---|
| **n = 768**: matches, share of items | 1.07 | 3.73 | 7.90 | 15.97 |
| baseline replica | 0.564 | 0.870 | 1.868 | 5.336 |
| C1: sorted annulus range | 0.347 | 0.687 | 1.782 | 5.305 |
| C2: support table + sorted range (K = 200) | 0.306 | 0.560 | 1.466 | 5.118 |
| items touched, C1 (%) | 20.1 | 38.1 | 72.6 | 89.0 |
| items touched, C2 (%) | 12.6 | 17.8 | 39.4 | 76.4 |
| per-item floor, deterministic (heuristic) | 0.077 | 0.131 | 0.364 | 1.378 |
| per-item floor, randomised z = 8.4 (heuristic) | 0.051 | 0.085 | 0.206 | 0.707 |
| entropy bound (proved) | 0.00044 | 0.0013 | 0.0027 | 0.0046 |
| **n = 1,024**: matches | 0.71 | 1.91 | 3.15 | 4.03 |
| baseline replica | 0.351 | 0.391 | 0.494 | 0.775 |
| C1 | 0.166 | 0.204 | 0.313 | 0.622 |
| C2 (K = 200) | 0.159 | 0.184 | 0.232 | 0.419 |
| items touched, C1 (%) | 6.6 | 10.0 | 20.7 | 36.5 |
| items touched, C2 (%) | 5.0 | 5.9 | 8.0 | 12.8 |
| per-item floor, deterministic (heuristic) | 0.046 | 0.049 | 0.048 | 0.083 |

The baseline touches 100% of items. **No row, here or in FINDINGS, counts the K centroids read to
form q·m (K·2n bytes, 0.1% of the dataset at 1,000 items per cluster) or the local bases.** That
is as large as what C2 saves.

## 1. Lower bounds

- **Entropy bound (proved, any index, any adaptivity).** Bits read form a prefix-free code, so
  E[bits] ≥ H(transcript); by Fano E[bits] ≥ H(S(t)|q) − h(ε) − εN = N·h(p) for independent
  items with match probability p. So % read ≥ h(p)/(16n). The baseline is 1,200–1,900× above
  it; an unbounded index over a finite query set attains it, so the metric has no useful floor
  unless index size is capped.
- **Suggested changes to the metric:** state an index-size cap; count per-query fixed reads
  (centroids, bases, tables); report bytes per item beside the percentage, since code bytes
  depend on the spectrum and the percentage falls as 1/n by itself.
- **Cost of one item at margin u (measured):** the baseline spends about C(t)/u bytes, with
  C ≈ 0.7 / 1.5 / 3 / 5.5 byte·cosine at 0.95 / 0.9 / 0.8 / 0.7 (n = 768). At 0.95, the 0.56% of
  items within 0.005 take 29% of the bytes and the 89% beyond 0.2 take 48% (the first pass). At
  0.7, 72% of items sit 0.05–0.2 away and take 45% of the bytes.
- **Per-item floors (heuristic, `stats.py`):** reverse water-filling on γᵢ per group, ideal
  quantiser, prior treated as free. Baseline replica is 3.9–7.4× above the deterministic floor
  at n = 768 and 7.6–10× at n = 1,024; C2 is 3.7–4.3× above at n = 768.

## 2. Not touching every item

**C1, sorted annulus (deterministic, no extra storage).** With a = x·m̂ (already stored),
q·x ∈ (q·m̂)·a ± √(1−(q·m̂)²)·√(1−a²). The upper bound is concave in a and the lower convex, so
within a cluster sorted by a the undecided items are one contiguous range found by two binary
searches (charged at 40 B per cluster).

**C2, support table (deterministic).** Store P[j,c] = max and min over cluster c of p_j·x for
anchor directions p_j (here the K centroids). For q = p_j + ρ, the cluster maximum of q·x is at
most P[j,c] plus the maximum of ρ·x, bounded by the same annulus form; undecided set is again one
range per cluster. ‖ρ‖ ≈ 0.35 replaces ‖q − m_c‖. Reads: one table row (2K bytes) per query.
Storage: 4K² bytes. Maxima only grow on insert and stay valid after delete. This is the dead-list
"anchor queries" idea with the correction applied to residuals on both sides.

Items touched by C2 at n = 768, by cluster size: 4,000 per cluster: 17.5 / 26.7 / 57.8 / 85.8%;
1,000: 12.6 / 17.8 / 39.4 / 76.4%; 250: 9.4 / 13.4 / 27.1 / 64.5%.

Several anchors add little: 25.8% → 22.1% of items in undecided clusters at 0.95 with 64 of 2,000
anchors (`pair.py`).

**Ideal low-dimensional index (upper bound on any such scheme).** Items surviving with k exact
local coordinates plus the tail norm, n = 768: k=0: 20 / 39 / 76 / 95%; k=4: 11 / 25 / 65 / 92;
k=16: 3.8 / 10 / 41 / 83; k=64: 1.7 / 4.9 / 13 / 44. Useful k is far above what a tree over
~1,000 points can index.

**Oracle headroom.** With a perfect cluster summary only clusters holding both matches and
non-matches need reading: 3.7 / 6.3 / 10.7 / 25.0% of items (groups of 32: 1.8 / 2.9 / 4.8 /
11.2%). The weakness of cluster pruning is in the bound, not the partition.

**Summaries cannot be small (proved given a measured property).** If item i exceeds every other
item of its cluster by e_i in direction r̂_i, deciding "cluster max ≥ τ" to tolerance e_i/2 for
all sub-collections solves INDEX, so any summary needs ≥ (1 − h(δ)) bits per such item. Measured:
91% of items have e_i > 0.05, 66% have e_i > 0.1. This bounds storage, not reads.

**LSH (valid, not competitive).** Hyperplane LSH meets the guarantee exactly; 0 misses in
1.9M matches at design parameters. Best case at 0.95: 0.384% with ~6 KB per item of tables; worse
than the baseline at 0.9 and below. Idealised cap filters: 0.41 / 0.83 / 2.1 / 6.8%.

## 3. The loose end

C1 and C2 certify 5 / 31 / 50 / 38% of matches as ranges without reading them (n = 768). At 0.7
on n = 768 this saves only 4%: the cost there is per-item code quality (floor 1.4%). At n = 1,024,
where 0.7 is far above the background, C2 cuts 0.775% to 0.419%.

## 4. Caveats

- C2 was not run on the 1M sample. Applying its touched shares to the FINDINGS baseline would
  give roughly 0.12% / 0.29% at 0.95 / 0.9 — conjectured, not measured.
- Table storage is 4·A·K bytes for A anchors; at 10⁹ items it must be sparse or use few anchors
  (untested).
- Queries are corpus items; ‖ρ‖ is larger for off-distribution queries.
- Low-dimensional trees, multi-anchor combinations and LSH did not pay.

## Files (this directory)

`stats.py`, `base.py`, `pair.py`, `pair2.py`, `lsh.py`, `lsh_check.py`, `fetch1024.py`, and
`*_data.json` with logs.
