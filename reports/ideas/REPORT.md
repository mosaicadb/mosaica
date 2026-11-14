# Making the exact, filter-aware index fast: proposals

Saved by the coordinating session from the agent's returned text. Statuses: measured, modelled (counts measured, time from assumed kernel rates), argued, rough, from memory.

## Answer

Same order of magnitude is reachable, in this order:

1. YFCC filtered track first, plausibly with exact answers. Winners are filter-first: ParlayANN (37,902 QPS) scans rare tags by brute force and gives common tags their own inverted file; the faiss baseline does 3,033.
2. nytimes-256 second: a flat pass over a 32-byte code plus a few hundred re-ranks, modelled at about 1,300 QPS at recall ≥ 0.98, no cluster probing.
3. glove-100 only in the approximate mode, modelled at 3,000–7,000 QPS at recall 0.9. Certified exact modelled at 150–400 QPS.

Every modelled figure assumes a kernel at faiss fast-scan speed (0.066 ns per code byte + ~0.3 ns per item). v2 is at ~0.32 ns/byte decode and 1.4 ns/byte end to end, so proposal 1 is the gate. Unfiltered exact stays linear in N.

## What the numbers say now

- Text at a 1% answer, 83.5 ms: decode 19, re-packing 18, bounds and decisions 24, tables 13, basis 7.
- Per touched item: item k-NN 45 ns, text 60 ns; faiss scans a 32-byte code in 2.1 ns. Gap 20–30×.
- On BioCLIP the code is tight (certified half-width ~0.005 at 96 B vs ~0.03 for a rotated 1-bit code at 3 sd). On glove (flat spectrum) the opposite holds.

## Experiment (modelled time, measured counts; 200 queries, k = 10)

Plain k-means, one global random rotation of the residual, b bits/dim with the RaBitQ rescaled estimate, sd = |r| tan(phi)/sqrt(n−1). Re-rank if estimate + m·sd reaches the running k-th best. Time: 0.3 + 0.066 × bytes ns per scanned item, 40/60 ns per re-rank, 25–30 µs fixed.

glove-100, K = 4,096:

| Code | Probed | m | Scanned | Re-ranked | Recall / recall_eps | Modelled |
|---|---|---|---|---|---|---|
| 1 bit (12.5 B) | all | 2 | 1,183,514 | 4,404 | 0.9995 / 0.9995 | 722 QPS |
| 1 bit | all | 3 | 1,183,514 | 17,745 | 1.000 | 521 |
| 1 bit | all | 9.35 | 1,183,514 | 833,019 | 1.000 | 29 |
| 2 bit (25 B) | all | 3 | 1,183,514 | 829 | 1.000 | 457 |
| 2 bit | all | 9.35 | 1,183,514 | 111,126 | 1.000 | 152 |
| 2 bit | 512 | 2 | 197,078 | 253 | 0.977 / 0.988 | 2,565 |
| 2 bit | 128 | 2 | 55,562 | 217 | 0.903 / 0.921 | 7,480 |
| 1 bit | 128 | 2 | 55,562 | 1,728 | 0.903 / 0.922 | 6,680 |

nytimes-256, K = 1,024:

| Code | Probed | m | Scanned | Re-ranked | Recall / recall_eps | Modelled |
|---|---|---|---|---|---|---|
| 1 bit (32 B) | all | 1 | 290,000 | 234 | 0.969 / 0.984 | 1,344 |
| 1 bit | all | 2 | 290,000 | 1,012 | 0.990 / 0.999 | 1,265 |
| 1 bit | all | 9.35 | 290,000 | 156,285 | 0.990 / 1.000 | 99 |
| 2 bit (64 B) | all | 3 | 290,000 | 232 | 0.989 / 0.9995 | 737 |
| 2 bit | all | 9.35 | 290,000 | 23,154 | 0.990 / 1.000 | 366 |
| 2 bit | 256 of 1,024 | 0–9.35 | 25,351 | 60–5,363 | 0.69 / 0.71 | — |

Settles: the confidence constant is the dial (m 9.35 → 3 cuts re-ranks 100–134× at recall ≥ 0.9995); with the dial at 2–3 the scan is the whole cost; plain k-means probing, not the code, limits recall (nytimes: a quarter of clusters holds 9% of items). Limits: modelled time, 200 queries, QR without sign fix, 776 near-zero-residual nytimes items, nytimes duplicates cap strict recall at 0.99.

## Proposals

1. **Decide in the integer domain; pay setup only where used.** Keep u16 accumulators in registers; fold per-(query, cluster, level) terms into two u16 thresholds rounded outward; per-item term (m·r, half-width from a 4-bit relative norm) as one more 16-entry shuffle. No f32, gather or sqrt per item. k-NN tests one side only. Re-pack only below ~50% occupancy, else a live mask. Tables per (group, level) with α·m moved into the per-item shuffle. Lazy basis projection of only the rows a level reads. Print the item-query breakdown. Argued: text threshold 1% 83 → ~30–45 ms; text k-NN 35 → 8–12 ms; item k-NN 4.8 → 1–1.5 ms. Cost: no memory; slack proofs redone in integers. Test: microbenchmark of one 32-byte level over 1M items, < 3 ns/item, same undecided count ±10%.
2. **Filter-first path in row space (YFCC).** Tag postings as Roaring over row ids; per-cluster offsets for tags > ~50k; L2 as q·x − |x|²/2 with a stored norm; uint8 vectors resident (1.9 GB). Up to ~30k visible: exact VNNI scan (15–25 ns/item rough); above: 24-byte first level over visible rows with exact re-rank, or approximate cluster-ordered walk. Rough 0.1–0.6 ms core time for small visible sets. Cost ~4 B per (item, tag). Test: download tag metadata only, histogram visible-set size over 100k queries, multiply by per-item costs.
3. **Confidence dial for the approximate mode.** Scale half-width: m 2–3 on randomised levels, λ × certified width on deterministic levels with λ fitted on corpus pseudo-queries (~0.1–0.2 expected). Each decision still depends only on the item's own code, so filtering stays exact. Test: sweep λ and c on BioCLIP text and items in the numpy harness.
4. **Coarse structure built for probing.** K ≈ 4√N, balanced k-means, a graph or two-level tree over centres; optionally a SOAR-style second assignment. Exact mode ignores it. Test: balanced k-means at K = 1,024/4,096 on nytimes and glove, recall vs share scanned.
5. **Choose the first level from the spectrum.** Where local bases buy little (glove), a global rotation with a 1–2 bit/dim first level, one table for the whole index, no per-cluster setup; keep local bases for deep levels and BioCLIP-like data. Rough: glove certified 2.5–4 ms. Test: certified bytes per item on glove, current code vs global 2-bit level.
6. **Batch queries through shared blocks.** Sort by tag or nearest centre; 32 items × 8–16 queries. Rough 1.5–3× for YFCC throughput.
7. **Use the tolerance the benchmark grants.** ANN-Benchmarks counts results within 0.001 of the k-th distance; run with d = 0.001 and a resident int8 final level. Rough 10–30%.

## Dead list

Graphs over items: still dead. Lower confidence: the "15%" reason does not hold for a randomised first level or an approximate mode. Finer clusters: dead for exact pruning, used here for probing only. Two codes intersected: dead; second assignment is different. Single pass and shared-table variants: agreed dead. Per-block summaries: agreed dead.

## Sources

- Results of the Big ANN: NeurIPS'23 competition, https://arxiv.org/html/2409.17424v1
- ANN-Benchmarks leader figures and recall tolerance from memory; check before quoting.

Scripts and logs: code/dial.py, code/dial_ny.py, code/patch.py, dial.log, dial_ny.log, dial_run1.log in this directory.
