# Exact vector search for Mosaica: the design as it stands

One self-contained description of the end-to-end design, consolidating the research so far. Each
section points to the report that holds the evidence. All figures are **measured** in a numpy
prototype that counts bytes; nothing has been timed in native code.

Reports (all under this directory): `combined/REPORT.md` (the build and ablation),
`knn/REPORT.md` (nearest-neighbour), `entropy/REPORT.md` (data structure), `agent1/`, `agent2/`,
`agent3a/`, `agent3b/` `REPORT.md` (round 1), `proofcheck/CONSOLIDATED_PROOFS.md` (theory),
`FINDINGS.md` (problem statement, dead list, owner's decisions in section 10).

## 1. What it must do

- **Queries.** (a) Threshold: return every item x with q·x ≥ t. (b) k-nearest-neighbour: return
  the k items with the largest q·x. Vectors and queries are unit length. Queries are mostly
  **text** embeddings against the corpus (the expected common case), sometimes corpus items.
- **Per viewer.** Every query runs inside a viewer's visible set, given as a Roaring bitmap over
  entity ids, and usually inside other filters too. Results are plain sets (bitmaps) that compose
  with every other filter. No three-valued or "maybe" results.
- **Guarantee.** For any query chosen independently of a public random seed ω:
  Pr over ω [ S(t) ⊆ R ⊆ S(t − d) ] ≥ 1 − ε, with d = 0 (exact) as the default and ε = 10⁻⁹ per
  query at N = 10⁹. Decisions made by deterministic bounds are always correct; only decisions at
  randomised levels carry ε. A query or item crafted against the seed affects only itself.
- **Scale and generality.** Billions of items on one machine (more by sharding, which needs no
  merging for thresholds). Live inserts, deletes and suppressions. Any embedding model, roughly
  100 to 5,000 dimensions, with no dataset-specific modelling.

## 2. What is built and stored

### Shared structure

| Structure | What | Size |
|---|---|---|
| Clusters | k-means, ~1,000 items each; centre m stored at 8 bits per dimension (x = m + r is exact against the stored centre) | n bytes per cluster |
| Groups and local bases | clusters grouped (G = 64 at 1M); one PCA basis of the residuals per group, float32 | 4n² per group (2.36 MB at n = 768) |
| Rotations | per group, one Haar rotation per randomised block (blocks 96 / 128 / 256 / 288 at n = 768), drawn from the public seed | can be regenerated |
| Anchors and support table | anchor directions = the cluster centres + 256 k-means centres of typical text prompts; for each (anchor, cluster), the exact max and min of anchor·x over the cluster's members, 2 B each, rounded outward | ~5 B per item at 10⁹ (on disk; one row read per query) |
| Per-level weights, escape step orders | small | ~3 MB |

### Per item: one nested stream, stored once

Items are stored in cluster order, and **within each cluster sorted by â = x·m̂** (16 bits).

| Level | Cumulative stored bytes | Contents |
|---|---|---|
| 0 | 3 | â (2 B; also gives m·r) and a 1-byte error norm |
| 4, 8, 16, 32 B (deterministic) | 8, 13, 22, 39 | Escape code of the residual in the local basis: per coordinate a sign, then a unary index on a fixed grid, then halvings, in one greedy order per group; nothing clipped. One 1-byte weighted error norm per level (2^(1/16) grid, rounded up). |
| 48, 96, 192, 384, 576 B (randomised) | 67, 119, 219, ~415, 611 | Codes what the 32-byte prefix left: residual whitened by a mixture profile of item and text query offsets, split into fixed blocks, each rotated once by the seed's rotation; per-block rates only rise across levels (sub-bit rates keep the first k′ rotated signs). Scalars: block norm 2 B once, angle bound 1 B whenever a block's rate rises. |
| Full vector | +1,536 (on disk) | The stored f16 vector, the ground truth. |

Each level extends the previous one. Every prefix yields a guaranteed interval for q·x:
- deterministic levels: q·x = q·m + m·r + Δ·r with Δ = q − m; the Δ·r term is bounded by
  weighted Cauchy–Schwarz on the coded residual with the stored norm, plus α* (an optimal
  multiple of m subtracted from q);
- randomised levels: an unbiased estimate per block with error A·u, u uniform on a sphere; bound
  by an exact Chernoff bound on the sum over blocks; c ≈ 9.35 sub-Gaussian equivalent from a union
  over N × 5 (item, level) events.

### Residency

Suggested: levels through 96 B resident (119 B per item, ~120 GB per 10⁹) for text-first use;
through 32 B (39 B per item) if memory is tight. Deeper levels and full vectors on disk, each level
stored contiguously per cluster in sort order.

## 3. Query paths

### Threshold query

1. **Nearest anchor.** β = q·p̂, ‖ρ‖ = √(1 − β²). Read one support-table row.
2. **Cluster cut.** A cluster is wholly in or out if its table extremes plus ‖ρ‖ decide it; such
   clusters are never opened.
3. **Sorted range.** In each remaining cluster that holds a visible item, read the centre. Both
   the annulus bound (from â alone) and the table-plus-ρ bound are concave in â above and convex
   below, so undecided items form one contiguous range in sort order, found by binary search
   (proved). Items above it are certified matches, below it certified non-matches, unread.
4. **Levels.** For undecided visible items, read level by level; an item stops at the first level
   whose interval is wholly above or below t. Then the full vector for the few left.
5. **Output** a bitmap over entity ids. Each item's decision depends only on its own data, so a
   result computed over all items and then masked equals one computed over the visible set.

### k-nearest-neighbour

1. **Seed.** From the clusters with the best table bound that hold a visible item, read 16-byte
   levels, refine the best candidates, read ~2k + 8 in full; τ₁ = exact k-th best among them.
2. **Sweep.** The threshold query at τ₁ over visible items.
3. **Selection.** Among survivors, τ = k-th largest lower bound; exclude items whose upper bound is
   below τ; certify an item when at most k − 1 others can beat it; refine the rest a level at a
   time.

Cost is within 5–22% of an oracle that knows the k-th score. Correct given valid intervals (proved).

### Caching (not built)

- A result cache keyed by (query, threshold) serves every viewer through their own mask.
- For threshold changes (a slider), keep each item's last interval where it went past memory, so
  only items straddling the new threshold need more reading.

## 4. Write path (not built)

- **Insert:** assign to the nearest centre; encode against that cluster's centre and its group's
  basis; widen the support-table extremes if needed (they only widen); place in sort order.
- **Delete / suppress:** masked by the existing bitmaps; extremes stay valid (looser).
- **Compaction:** re-cluster, re-fit bases, re-encode, tighten extremes, optionally redraw the seed.
- Everything learned (centres, bases, anchors, step orders, weights) is a build parameter, so a
  build is an ingest into an empty database.

## 5. Measured costs (1M BioCLIP-2, n = 768; per-item bytes read; zero violations)

**Threshold queries, combined design** (% of the f16 dataset; all costs at N = 10⁹):

| Answer size | 0.01% | 0.1% | 1% | 5% |
|---|---|---|---|---|
| Item queries | 0.24% | 0.52% | 1.7% | 5.2% |
| Text queries | 2.2% | 3.1% | 5.2% | 8.8% |

What each component is worth (from the original construction): escape code −13 to −30% for both;
skipping (range + table) up to −85% for items at tight thresholds, ~0 to −6% for text; randomised
levels −8 to −18% for text, +1 to +3% for items. Text items touched: 64–99%.

**k-NN, k = 10, exact:** items 2.6 MB per query (0.17%), text 30 MB (2%), excluding bases.
Filtered: falls with the visible set (items 1.1 / 0.86 / 0.54 MB at 10 / 1 / 0.1% visible) while
faiss HNSW loses recall (0.82 / 0.18 at 1 / 0.1%) and hnswlib slows (12 / 126 ms). Unfiltered,
HNSW is far faster (0.25–1.4 ms vs a modelled 16–139 ms) but approximate (text recall 0.92–0.95).

**Other dimensions (items):** n = 1,024 native: 0.13–1.5%; n = 100 GloVe: 12–36% (and k-NN 13%).

**Residency split, text, 0.1% answer** (B per item, RAM / disk): through 32 B 24 / 23; through
96 B 43 / 3.6; through 192 B 46 / 1.1. Disk pages could multiply the disk figure by up to 10–30×
if items reaching deep levels are scattered; unmeasured.

## 6. Known problems and open questions

1. **Fixed per-query costs.** Local bases: 70–196 MB per query at 1M (7–13% of the dataset), more
   than all item reads. Centres: 77% opened for item queries, all for text.
2. **Text queries** spend three quarters of their bytes in the first 32 B, where bounds are wider
   than the spread of a text query's scores (conjectured floor, not proved). Data modelling won't
   help (entropy report): text residuals are near-Gaussian.
3. **The per-item code** sits 1.36–1.42× above the proved floor on Gaussian data; being reworked
   now by another agent (results pending).
4. **Low dimension** (~100) is expensive; randomised levels hurt there.
5. **Page locality** of deep-level reads is unmeasured.
6. **Linear in N.** Every visible item in an undecided cluster is touched at least at level 0.
7. Not built: write path, caching, a real query engine, timings.

## 7. Ruled out (with reasons, see FINDINGS section 7 and the reports)

Graph indexes for exactness; multi-index hashing; wide random sparse features; LSH (valid but
costly); trees over items and small set summaries (proved to need many bits); two independent codes
of one item; per-cluster shape descriptions; class-uniform guarantees; query-time randomness;
modelling non-Gaussian structure (small gains); learned codebooks for correctness (fine for cost).
