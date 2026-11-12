# End-to-end review of the design

Saved by the coordinating session from the reviewer's returned text; condensed. Every number is the
reviewer's. Scripts and logs in this directory (`bench.c`, `bench2.c`, `bench3.c`,
`e1_anchors.py`, `e1b_multi.py`, `e1c_oracle.py`, `e3_qr.py`, `e4_detprofile.py`, `r_cent.py`).

## Summary

The design is sound in outline; no result breaks the threshold guarantee. But the accounting is in
bytes, and bytes are a poor proxy for time here.

- **Bytes are not time.** Escape decode costs 15–20 ns per coded byte; a full f16 dot costs 0.05 ns
  per byte. Unfiltered at 1M in RAM, the index is no faster than a flat scan for items and ~4×
  slower for text (model from measured rates).
- **Absolute cost at 10⁹ is not interactive unfiltered.** Text threshold at 0.1% reads 46 GB;
  disk levels cost pages, not bytes.
- **The 10⁹ figures assume the support table works as at 1M.** At the implied anchor-to-cluster
  ratio every centre is opened and items touched rise 1.5–2.8×.
- **The prototype's rotations are not Haar** (`np.linalg.qr` without the sign fix). One-line fix.
- **A free text win was missed:** mixture weights at the deterministic levels cut text bytes 4–14%,
  items 0–6%, no storage cost.
- **Text has large cluster-level headroom:** 75% of a text query's score variance is between
  clusters; exact per-query cluster extremes cut text items touched from 57–99% to 0.7–43%. No
  anchor table gets near this for a fresh query, but a cached row for a repeated query does.
- **The fixed costs are small in time:** 64 float32 bases take 10.7 ms, 1.5 ms as int8. The costly
  per-cluster work is the per-level weighted norms, not the centre read.
- **Uncounted:** mapping between entity ids and vector rows, and the visibility test per item.

## 1. Problems, by severity

### 1.1 Byte metric misranks components by up to 300× (measured, `bench.c`, `bench3.c`)

| Operation | Cost |
|---|---|
| Escape decode + incremental dot, branch-free | 72 / 161 / 245 / 492 ns per item at 4 / 8 / 16 / 32 B (15–20 ns/B) |
| Same, naive | 171 ns at 4 B |
| Byte-table accumulate, fixed-rate 32 B code | 18.4 ns per item (0.58 ns/B) |
| Nibble-table (pshufb), 32 B code | 7.0 ns per item (0.22 ns/B) |
| f16 full dot, n = 768 | 76 ns sequential, 120 ns random |
| int8 centre dot | 86 ns |
| 64 float32 bases × query | 10.7 ms |
| 64 int8 bases × query | 1.5 ms |

Model: text at 0.1% ≈ 330 ns per item vs 76 ns flat f16; items ≈ 70–80 ns per item, same as flat.
Fix: make time the metric; require table-decodable codes or show < ~1 ns/B.

### 1.2 Absolute cost at 10⁹ and disk pages

Text threshold 0.1%: 46 GB per query; text k-NN 30 GB; items 3–7 GB. With memory through 96 B,
2.49% of items enter the 192 B level; scattered, that reads ~64% of its pages (~64 GB, not 2.5 GB;
conjectured). A full-vector read is one page, so going straight to the full vector after the last
resident level takes fewer reads than walking 192 → 384 → 576 → full. Fix: state the envelope
(interactive when the visible set is ~10⁷–10⁸ or smaller); re-derive the level schedule under
(resident byte, disk page) costs; residency through 192 B looks necessary for unfiltered 10⁹.

### 1.3 Anchor ratio at 10⁹ (measured, `e1_anchors.py`)

Items touched after range and table, item queries, 1M:

| Image anchors | 0.01% | 0.1% | 1% | 5% | t = 0.95 | t = 0.9 | Centres opened (0.01%) |
|---|---|---|---|---|---|---|---|
| 1,000 (as built) | 12.9 | 20.6 | 38.9 | 76.0 | 3.5 | 7.1 | 68% |
| 250 | 14.7 | 23.7 | 43.0 | 78.4 | 4.0 | 9.1 | 76% |
| 64 | 16.6 | 26.2 | 45.5 | 80.0 | 4.7 | 11.0 | 82% |
| 16 | 19.9 | 30.3 | 50.4 | 90.2 | 6.9 | 20.0 | 100% |
| 4 | 22.9 | 34.2 | 54.6 | 91.8 | 8.8 | 25.4 | 100% |
| Range only | 27.6 | 39.9 | 59.8 | 96.3 | 12.4 | 36.8 | 100% |

16–64 anchors is the comparable regime for 10⁹ (conjectured): tight-threshold item queries cost
~1.5–2.8× the published 10⁹ figures.

### 1.4 Rotations not Haar; regeneration unsafe

`Q[0,0] < 0` in 2,000 of 2,000 draws without `Q *= sign(diag(R))` (`e3_qr.py`). Lemma 8 needs
right-invariance. Floating-point QR is not bit-reproducible across BLAS builds: store rotations or
generate them in exact integer arithmetic.

### 1.5 Id mapping and visibility test uncounted

Rows are ordered by (cluster, â); the visible set is over entity ids. Needs either a row-to-entity
lookup and bitmap probe per touched item, or the viewer's bitmap permuted into row order. Building
the result bitmap from row ids needs a sort. Decide the id architecture before the prototype.

### 1.6 Text evidence optimistic

Test and training prompts written by the same agent from the same templates; text anchors and
profile fitted on the training set; one model with a small modality gap; item queries are corpus
members. Fix: a real query log or independent prompts; out-of-corpus image queries; a large-gap
model.

### 1.7 k-NN semantics

"A crafted item affects only itself" holds for thresholds, not literally for k-NN (it can displace
one item). Ties need a deterministic seed-independent tie-break on `mosaica_id`. Cached k-NN
results cannot be masked for another viewer. Shards need a two-round protocol.

### 1.8 Write path

Table widening must be atomic with the item becoming visible (suggest an always-scanned delta
segment with immutable per-segment tables). Nearest-centre assignment at 10⁶ centres ≈ 86 ms per
insert by brute force — the legitimate place for a graph or hierarchy over centres. A cross-grant
result cache leaks by timing that someone else ran the query; table extremes computed over
invisible items leak by timing whether invisible items lie near a query. Record in the threat
model.

### 1.9 Bases grow as n²

37.7 MB per float32 basis at n = 3,072 (2.4 GB for 64 groups); 100 MB at n = 5,000.

### 1.10 Fixed-point definitions missing

Prototype truth is renormalised float32, not stored f16; annulus bound assumes ‖x‖ = 1 exactly;
slacks are ad hoc. Define similarity on stored integers with a stored bound on |‖x‖ − 1|.

### 1.11 Per-cluster setup is the real per-cluster cost

~12 µs per opened cluster for per-level weighted norms vs 86 ns for the centre dot; ~12
core-seconds per text query at 10⁹ with every cluster opened (model).

## 2. Wins and options, by gain against effort

1. **Mixture weights at the deterministic levels** (measured, `e4_detprofile.py`, 524k items,
   0 violations). Bytes per item at 0.01 / 0.1 / 1 / 5%:

   | Variant | Text | Items |
   |---|---|---|
   | As built | 31.5 / 44.1 / 83.8 / 146.5 | 6.47 / 10.48 / 23.71 / 67.80 |
   | Mixture weights | 27.6 / 39.7 / 78.4 / 140.2 | 6.06 / 9.92 / 23.17 / 67.80 |
   | Mixture weights and order | 27.2 / 39.1 / 77.0 / 138.2 | 6.08 / 9.97 / 23.34 / 68.45 |
   | Two stored norms | worse | worse |

2. **Fixed-rate table-decodable code for resident levels, with batched scans.** 0.2–0.6 ns/B
   vs 15–20; suits GPU; one pass serves many queries. Perhaps 15–35% more bytes; 10×+ in time for
   text (conjectured).
3. **Cache per-query cluster extremes** (measured, `e1c_oracle.py`). Text items undecided at
   0.01 / 0.1 / 1 / 5%: as built 57.0 / 68.5 / 87.9 / 98.8; all 2,206 prompts as anchors
   53.4 / 65.1 / 84.9 / 97.3; 16 anchors combined 44.9 / 57.6 / 81.2 / 96.6; exact extremes
   0.7 / 2.2 / 12.3 / 42.8; to ±0.02 1.8 / 4.3 / 21.9 / 63.7; to ±0.05 5.0 / 10.8 / 43.5 / 88.6.
   Anchors are a dead end for fresh text queries; for repeated query directions (slider, popular
   search, k-NN for many viewers) a cached row cuts the first pass 5–30×.
4. **Quantised bases** — exactly sound if the encoder uses the stored basis and computes norms
   through its true inverse (proved). int8: 1.5 ms vs 10.7 ms. For large n, a truncated local
   head with tail blocks in a global system.
5. **Planner with exact fallbacks:** read full vectors when a group has fewer undecided visible
   items than ~1,500; scan the visible set directly below ~10⁴–10⁵ visible items.
6. **One global set of rotations** (union bound needs independence only across blocks of one
   item; proved): 45 MB → 0.7 MB.
7. **Band-constant weights, lazy per-level norms** to cut per-cluster setup.
8. **Stream certified matches** as found (a subset of the final answer).
9. **Per-query confidence constant** from the seed-independent undecided set: 8.58 vs 9.35 at
   10⁶; ≤8% of the randomised width.
10. **The guarantee is uniform over thresholds and k** for a fixed q (proved by inspection).
11. **Progressive or hierarchical centres:** ~10% of bytes for tight item queries; low priority.
    A graph over centres helps insert assignment and the approximate mode, not skipping.
12. **For a general library:** filter in the library's own row-id space or as a callback;
    non-unit vectors and L2 via a stored norm; supported range roughly n ≥ 256.

## 3. Simplifications

- Drop the level-0 norm byte (level 0 decides almost nothing once the range is in place).
- Drop the 384 B and 576 B levels (under page costs they add reads; saves 392 B per item of disk).
- Start with the closed-form sub-Gaussian bound (the exact Chernoff table is worth 0.6–3.7% and
  puts a generated table in the correctness path).
- Drop the text anchors unless a real query log supports them.
- Reconsider the sort by â in time terms: a SIMD compare over an unsorted â column is ~0.25 ns per
  item (conjectured); id order within a cluster would make the visibility intersection a merge.
- Do not store a second error norm.

## 4. What a native prototype should measure first

1. ns per decided item, per level: escape (scalar and interleaved SIMD) vs a fixed-rate table code
   with stored norms.
2. End-to-end wall time at 1M and 10–100M, threshold and k-NN, items and text, against flat SIMD
   scans, at 1 and N threads; where memory bandwidth saturates.
3. Batched scans: QPS with B concurrent text queries sharing one pass.
4. Disk: pages read per query at each residency split vs direct-to-full-vector.
5. The id path: visibility test and result bitmap cost, both mapping strategies.
6. The table at the real anchor ratio, and per-cluster setup time.
7. Mixture weights in the combined track; cached extremes on a query log.
8. Exact integer arithmetic; reproducible Haar rotations; rerun violation counts.
