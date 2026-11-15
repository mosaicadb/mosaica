# The checks of REPORT_CONSOLIDATION.md

`fetch_bioclip.py N OUT SEED` fetches whole row groups of BioCLIP-2 float16 image embeddings (needs `huggingface_hub`,
`pyarrow`, `numpy`). The checks used:

```bash
python3 -I fetch_bioclip.py 60000 b60k 7          # 50k corpus, 200 item queries, 400 held-out queries (shuffled, seed 11)
python3 -I fetch_bioclip.py 1060000 b1m 13        # 1M corpus and 40 held-out queries (shuffled, seed 17)
python3 -I ../../tools/fit.py b60k/corpus.npy c50k
python3 -I ../../tools/fit.py b1m/corpus.npy c1m
vexact --v2 --no-approx --index c50k --queries b60k/q2.npy --item-queries b60k/item_q.npy --results R verify oracle --vitem 200 --vtext 200
OPENBLAS_NUM_THREADS=1 python3 -I ../../../bench/oracle_thr.py c50k R/oracle
```

The splits: shuffle the fetched rows with `numpy.random.default_rng(SEED).permutation`, take the first 50,000 (or
1,000,000) as the corpus and the next rows as queries. Thresholds for dbcmp are the 10th, 100th, 1,000th, 5,000th and
25,000th best score of each query at 50k (100th to 500,000th at 1M), saved in C order.
