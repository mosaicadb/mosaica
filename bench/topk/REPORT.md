# Top-k search at 1M BioCLIP-2, 1 thread

![top-k latency against k](latency_topk_1t.png)

## Method

- **Data and queries.** The 999,950 BioCLIP-2 vectors and the 15 timing queries of `../threshold-db` (every 10th of
  the 150: 5 held-out images, 10 text prompts). Unfiltered, one thread, warm.
- **Recall@k** is the share of the true top k returned. The true top k is the first k of `prep/order.npy` (ids by f64
  similarity). The figure is the mean over the queries; time is the median over queries of each query's median of 5
  repetitions.
- **Targets.** Each approximate method is shown at the cheapest setting on its ladder whose mean recall reaches 0.80,
  and the cheapest reaching 0.99. The ladder is walked at one repetition and the first rung reaching a target is timed
  at 5. HNSW: ef = ⌈k × mult⌉, mult over ann.py's {1, 1.5, 2, 3, 4, 6, 8, 12, 16, 24, 32, 64, …, 1024}, then doubling up
  to N (k = 1 and 10 needed ef well past 1024 k). IVF and RaBitQ: nprobe 1, 2, 4, …, 4096. A ladder stops at the
  first rung slower than 5 s per query. A target not reached is plotted hollow at the last rung, with its recall.
  Settings were tuned on the timed queries, so they are optimistic.
- **LIMINAL (exact)** is `knn2`, the planner's exact k-NN path, through the new `dbtopk` mode in `rust/src/run2.rs`,
  on the `index_r4` index (`--v2 --int --gtab --flags repack`). Its recall is just below 1 at k = 100 and 1,000
  because it is exact over the stored f16 vectors, and the truth is f64.
- **LIMINAL approximate** is `knn2_dial`, the mode `dbknn` uses in the threshold-db work: nprobe doubling from 1 to all
  1,000 clusters, with the dial off and fitted for 0.99. The faster of the two dials at each target is kept; the param
  column is `nprobe:dial`.
- **Libraries.** rabitqlib 0.2.0 (IVF4096, 7 bits, `high_accuracy`), faiss 1.16.0 `IndexHNSWSQ` QT_fp16 (M 16,
  efConstruction 200) and `IndexIVFScalarQuantizer` IVF4096 QT_fp16, as top-k `search`. These reuse the indexes in
  `vexact-benchup-data/ann`. **hnswlib, faiss HNSW f32 and faiss IVF4096 f32 are missing** because their index files
  are not there and were not rebuilt. The SQfp16 variants stand in for the faiss ones, which the threshold-db report
  found the same speed as f32.
- **Machine.** Ryzen 9 5900X under WSL2. LIMINAL ran on cores 4–5 and was pinned to core 4. The libraries ran at the
  same time on cores 6 and 7, and other agents were using cores 8–11. LIMINAL's cost model was calibrated during this
  run, with the other jobs running, and its per-item constants came out about 1.7 times those measured on core 10.

## Results

Median ms, with the mean recall in brackets.

| method | target | k = 1 | k = 10 | k = 100 | k = 1,000 |
|---|---|---|---|---|---|
| LIMINAL (exact) | exact | 25.0 (1) | 27.8 (1) | 33.1 (.999) | 83.7 (.999) |
| LIMINAL approximate | 0.80 | 2.06 (.80) | 1.33 (.86) | 3.35 (.87) | 22.4 (.81) |
| LIMINAL approximate | 0.99 | 4.29 (1) | 6.86 (1) | 9.18 (.991) | 68.4 (.996) |
| faiss HNSW-SQfp16 | 0.80 | 8.02 (.80) | 0.29 (.83) | 0.20 (.81) | 1.59 (.90) |
| faiss HNSW-SQfp16 | 0.99 | 961 (1) | 420 (1) | 18.4 (.991) | 44.2 (.994) |
| faiss IVF4096-SQfp16 | 0.80 | 0.45 (.87) | 0.45 (.85) | 1.29 (.81) | 1.99 (.81) |
| faiss IVF4096-SQfp16 | 0.99 | 3.80 (1) | 5.96 (1) | 12.1 (.997) | 12.3 (.992) |
| RaBitQ | 0.80 | 0.31 (.87) | 0.37 (.85) | 0.43 (.81) | 1.14 (.81) |
| RaBitQ | 0.99 | 0.53 (1) | 9.80 (.987), not reached | 2.88 (.992) | 4.07 (.993) |

`results/plot_data.csv` has the settings (ef, nprobe, `nprobe:dial`), p90s and query counts. The per-query rows are in
`results/liminal_topk.csv` and `results/ann_*.csv`, and every ladder rung is in the logs.

- HNSW at k = 1 and 10 needs ef 65,536 and 40,960 to reach 0.99, because the graph reaches a few text queries'
  nearest items only late. At k = 1, recall stays at 0.73 from ef 256 to 2,048.
- RaBitQ at k = 10 reaches 0.987 with every cluster probed. Its final distances come from the 7-bit code with no
  full-precision re-rank, so it goes no higher.
- LIMINAL approximate at k = 1, with the dial fitted for 0.99, stops at recall 0.87 even with all 1,000 clusters. With
  the dial off it reaches 1 at nprobe 64.
- LIMINAL's p90 at k = 1,000 is 377 ms exact and 272 ms approximate at 0.99, against medians of 84 and 68: a few
  queries are far slower.

## Reproduce

```bash
P=<venv as in ../threshold-db/REPORT.md>; D=/home/joe/code/vexact-data/bioclip2-1m; I=/home/joe/code/vexact-benchup-data/index_r4
A=/home/joe/code/vexact-benchup-data/ann
taskset -c 4-5 vexact --v2 --int --gtab --flags repack --index $I --queries $D/prep/queries.npy --results results \
  --core 4 --pin 4 --fit-dial dbtopk --order $D/prep/order.npy --ks 1,10,100,1000 --targets 0.8,0.99 --step 10 --reps 5
taskset -c 6 $P -I ann_topk.py rabitq|hnsw_sq16|ivf_sq16 $D/prep $A results/ann_METHOD.csv
$P -I plot.py results
```
