# Approximate k-NN: why it is level with IVF, and the next three steps

After step 1 (the dial at every level, a heap of exact scores, best-first probing; `bench/topk/REPORT.md`), LIMINAL's
approximate mode reads 4–10 times fewer bytes per item than faiss IVF4096-SQfp16, yet runs at about the same speed at
recall 0.99. Three costs cancel the per-item saving, and each has a step that removes it.

| Cost | Where it shows | Step | Reference |
|---|---|---|---|
| Fixed work per query that IVF does not pay: the cut (about 1.5 ms), one 768 × 768 basis product per opened group (55–76 µs each, 24–39 groups), rotations, setup and tables per opened cluster | 30–40% of a query at 0.99 for small and medium k | Cheaper group setup: multiply only the basis rows the read levels use | REPORT.md idea 4; `bench/topk/REPORT.md` "Next bottleneck" |
| More items scanned: LIMINAL's clusters hold about 1,000 items, IVF's lists about 244, and a coarser partition reads more items for the same recall | items scanned at every k | Sub-clusters: 16 per cluster, probed by centre score + 0.2 × radius, two-level probing; 2–3.7 times fewer items, about 1 MB in all | REPORT.md idea 3 and its row-order ruling: rows stay sorted by key, and the approximate mode gathers each sub-cluster's rows from a stored list of positions |
| Each code level is its own pass (lookups, bounds, compaction, re-pack): about 8 ns per item per deterministic level, 31 ns per randomised level, against about 100 ns for a streamed f16 dot product | item work, 25–34% of a query | One fused fast-scan pass over levels 0–3 with one integer threshold per block, approximate mode only; estimated 2–4 ns per item | REPORT.md idea 6 (speculative) |

Together the research report estimates 1.4–2.1 ms at k = 10, 1.5–2.9 ms at k = 100 and 3.5–6 ms at k = 1,000 at
recall 0.99 (section "What a combined mode could reach"), against today's 5.5, 5.7 and 10.6 ms and IVF's 6.0, 12.1
and 12.3 ms. The byte advantage already shows where bytes are the bottleneck: under a 776 MB memory cap LIMINAL is 3–10
times ahead of IVF on disk (`bench/threshold-db/REPORT_CAPPED.md` on research/bench-capped, `REPORT_DISK.md` on
research/disk-reads).
