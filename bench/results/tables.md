### glove-100-angular

Exact (all test queries, one thread):

| Method | Filter | QPS | Mean ms | Recall@10 | Sets differing from brute force | Score lists differing |
|---|---|---|---|---|---|---|
| ours exact | all | 68.3 | 14.64 | 1.0000 | 0 | 0 |
| ours flat | all | 88.0 | 11.36 | 1.0000 | 0 | 0 |
| ours exact | vis_0.001 | 31,882.3 | 0.03 | 1.0000 | 0 | 0 |
| ours flat | vis_0.001 | 32,164.3 | 0.03 | 1.0000 | 0 | 0 |
| ours exact | vis_0.01 | 4,519.7 | 0.22 | 1.0000 | 0 | 0 |
| ours flat | vis_0.01 | 4,554.7 | 0.22 | 1.0000 | 0 | 0 |
| ours exact | vis_0.1 | 218.9 | 4.57 | 1.0000 | 0 | 0 |
| ours flat | vis_0.1 | 252.4 | 3.96 | 1.0000 | 0 | 0 |
| ours exact | vis_0.5 | 59.1 | 16.91 | 1.0000 | 0 | 0 |
| ours flat | vis_0.5 | 60.2 | 16.61 | 1.0000 | 0 | 0 |

Best QPS at recall@10 at least the target, filter `all` (QPS (recall; configuration)):

| Method | ≥ 0.9 | ≥ 0.95 | ≥ 0.99 | Highest recall |
|---|---|---|---|---|
| faiss-flat | 52 (1.000; flat) | 52 (1.000; flat) | 52 (1.000; flat) | 52 (1.000; flat) |
| faiss-hnsw | 2,592 (0.908; M-48 ef=80) | 1,503 (0.956; M-96 ef=120) | 576 (0.991; M-64 ef=400) | 278 (0.998; M-96 ef=800) |
| faiss-ivfpqfs | 4,468 (0.908; nlist-1204 nprobe=100 k_reorder=10) | 2,527 (0.961; nlist-1204 nprobe=100 k_reorder=100) | 1,114 (0.995; nlist-512 nprobe=200 k_reorder=100) | 469 (0.996; nlist-512 nprobe=200 k_reorder=1000) |
| hnswlib | 2,856 (0.904; M-48 ef=80) | 1,388 (0.957; M-96 ef=120) | 562 (0.991; M-64 ef=400) | 242 (0.999; M-96 ef=800) |
| ours approx | 922 (0.901; nprobe=32 eps=0) | 307 (0.972; nprobe=128 eps=0.01) | 163 (0.991; nprobe=256 eps=0) | 85 (0.998; nprobe=512 eps=0) |
| ours approx (stage 1) | 232 (0.933; nprobe=64 rerank=100 level=5) | 116 (0.964; nprobe=128 rerank=100 level=5) | 57 (0.990; nprobe=256 rerank=1000 level=5) | 57 (0.990; nprobe=256 rerank=1000 level=5) |

Best QPS at recall@10 at least the target, filter `vis_0.5` (QPS (recall; configuration)):

| Method | ≥ 0.9 | ≥ 0.95 | ≥ 0.99 | Highest recall |
|---|---|---|---|---|
| faiss-flat | 108 (1.000; flat) | 108 (1.000; flat) | 108 (1.000; flat) | 108 (1.000; flat) |
| faiss-hnsw | 1,261 (0.911; M-16 ef=400) | 510 (0.959; M-16 ef=1000) | 76 (0.994; M-16 ef=5000) | 28 (0.998; M-16 ef=10000) |
| faiss-ivfpqfs | 2,632 (0.922; nlist-2048 nprobe=200 k_reorder=10) | 1,700 (0.967; nlist-2048 nprobe=200 k_reorder=100) | 946 (0.992; nlist-2048 nprobe=500 k_reorder=100) | 188 (1.000; nlist-2048 nprobe=2048 k_reorder=1000) |

Best QPS at recall@10 at least the target, filter `vis_0.1` (QPS (recall; configuration)):

| Method | ≥ 0.9 | ≥ 0.95 | ≥ 0.99 | Highest recall |
|---|---|---|---|---|
| faiss-flat | 620 (1.000; flat) | 620 (1.000; flat) | 620 (1.000; flat) | 620 (1.000; flat) |
| faiss-hnsw | 525 (0.928; M-16 ef=1000) | 250 (0.963; M-16 ef=2000) | 28 (0.996; M-16 ef=10000) | 28 (0.996; M-16 ef=10000) |
| faiss-ivfpqfs | 2,349 (0.932; nlist-2048 nprobe=200 k_reorder=10) | 1,559 (0.952; nlist-2048 nprobe=200 k_reorder=100) | 494 (0.999; nlist-2048 nprobe=1000 k_reorder=100) | 270 (1.000; nlist-2048 nprobe=2048 k_reorder=100) |

Best QPS at recall@10 at least the target, filter `vis_0.01` (QPS (recall; configuration)):

| Method | ≥ 0.9 | ≥ 0.95 | ≥ 0.99 | Highest recall |
|---|---|---|---|---|
| faiss-flat | 10,890 (1.000; flat) | 10,890 (1.000; flat) | 10,890 (1.000; flat) | 10,890 (1.000; flat) |
| faiss-hnsw | 80 (0.961; M-16 ef=5000) | 80 (0.961; M-16 ef=5000) | – | 28 (0.984; M-16 ef=10000) |
| faiss-ivfpqfs | 1,199 (0.963; nlist-2048 nprobe=500 k_reorder=10) | 1,199 (0.963; nlist-2048 nprobe=500 k_reorder=10) | 543 (0.995; nlist-2048 nprobe=1000 k_reorder=100) | 289 (1.000; nlist-2048 nprobe=2048 k_reorder=100) |

Best QPS at recall@10 at least the target, filter `vis_0.001` (QPS (recall; configuration)):

| Method | ≥ 0.9 | ≥ 0.95 | ≥ 0.99 | Highest recall |
|---|---|---|---|---|
| faiss-flat | 45,033 (1.000; flat) | 45,033 (1.000; flat) | 45,033 (1.000; flat) | 45,033 (1.000; flat) |
| faiss-hnsw | 29 (0.922; M-16 ef=10000) | – | – | 29 (0.922; M-16 ef=10000) |
| faiss-ivfpqfs | 1,447 (0.907; nlist-2048 nprobe=500 k_reorder=10) | 783 (0.981; nlist-2048 nprobe=1000 k_reorder=10) | 403 (0.999; nlist-2048 nprobe=2048 k_reorder=10) | 397 (1.000; nlist-2048 nprobe=2048 k_reorder=100) |

### nytimes-256-angular

Exact (all test queries, one thread):

| Method | Filter | QPS | Mean ms | Recall@10 | Sets differing from brute force | Score lists differing |
|---|---|---|---|---|---|---|
| ours exact | all | 104.2 | 9.59 | 0.9991 | 0 | 0 |
| ours flat | all | 128.8 | 7.77 | 0.9991 | 0 | 0 |
| ours exact | vis_0.001 | 103,836.2 | 0.01 | 0.9991 | 0 | 0 |
| ours flat | vis_0.001 | 105,534.9 | 0.01 | 0.9991 | 0 | 0 |
| ours exact | vis_0.01 | 13,336.0 | 0.07 | 0.9991 | 0 | 0 |
| ours flat | vis_0.01 | 13,350.6 | 0.07 | 0.9991 | 0 | 0 |
| ours exact | vis_0.1 | 656.6 | 1.52 | 0.9991 | 0 | 0 |
| ours flat | vis_0.1 | 998.7 | 1.00 | 0.9991 | 0 | 0 |
| ours exact | vis_0.5 | 161.1 | 6.21 | 0.9991 | 0 | 0 |
| ours flat | vis_0.5 | 169.9 | 5.88 | 0.9991 | 0 | 0 |

Best QPS at recall@10 at least the target, filter `all` (QPS (recall; configuration)):

| Method | ≥ 0.9 | ≥ 0.95 | ≥ 0.99 | Highest recall |
|---|---|---|---|---|
| faiss-flat | 94 (0.999; flat) | 94 (0.999; flat) | 94 (0.999; flat) | 94 (0.999; flat) |
| faiss-hnsw | 3,754 (0.903; M-36 ef=40) | 1,591 (0.951; M-36 ef=120) | 361 (0.990; M-36 ef=600) | 160 (0.998; M-96 ef=800) |
| faiss-ivfpqfs | 4,571 (0.919; nlist-512 nprobe=100 k_reorder=10) | 2,788 (0.962; nlist-512 nprobe=200 k_reorder=10) | – | 1,651 (0.967; nlist-512 nprobe=200 k_reorder=100) |
| hnswlib | 4,738 (0.903; M-36 ef=40) | 1,728 (0.952; M-64 ef=80) | 399 (0.991; M-64 ef=400) | 162 (0.998; M-96 ef=800) |
| ours approx | 319 (0.918; nprobe=64 eps=0.01) | 178 (0.972; nprobe=64 eps=0 soar) | – | 178 (0.972; nprobe=64 eps=0 soar) |
| ours approx (stage 1) | 199 (0.911; nprobe=64 rerank=1000 level=5) | 109 (0.957; nprobe=128 rerank=1000 level=5) | – | 56 (0.985; nprobe=256 rerank=1000 level=5) |

Best QPS at recall@10 at least the target, filter `vis_0.5` (QPS (recall; configuration)):

| Method | ≥ 0.9 | ≥ 0.95 | ≥ 0.99 | Highest recall |
|---|---|---|---|---|
| faiss-flat | 198 (0.999; flat) | 198 (0.999; flat) | 198 (0.999; flat) | 198 (0.999; flat) |
| faiss-hnsw | 989 (0.934; M-16 ef=400) | 392 (0.967; M-16 ef=1000) | 64 (0.996; M-16 ef=5000) | 25 (0.998; M-16 ef=10000) |
| faiss-ivfpqfs | 3,838 (0.908; nlist-2048 nprobe=200 k_reorder=10) | 2,111 (0.954; nlist-2048 nprobe=500 k_reorder=10) | 683 (0.992; nlist-2048 nprobe=2048 k_reorder=10) | 544 (0.999; nlist-2048 nprobe=2048 k_reorder=100) |

Best QPS at recall@10 at least the target, filter `vis_0.1` (QPS (recall; configuration)):

| Method | ≥ 0.9 | ≥ 0.95 | ≥ 0.99 | Highest recall |
|---|---|---|---|---|
| faiss-flat | 1,368 (0.999; flat) | 1,368 (0.999; flat) | 1,368 (0.999; flat) | 1,368 (0.999; flat) |
| faiss-hnsw | 406 (0.927; M-16 ef=1000) | 191 (0.970; M-16 ef=2000) | 65 (0.993; M-16 ef=5000) | 25 (0.997; M-16 ef=10000) |
| faiss-ivfpqfs | 1,958 (0.940; nlist-2048 nprobe=500 k_reorder=10) | 1,168 (0.978; nlist-2048 nprobe=1000 k_reorder=10) | 621 (0.995; nlist-2048 nprobe=2048 k_reorder=10) | 500 (0.999; nlist-2048 nprobe=2048 k_reorder=100) |

Best QPS at recall@10 at least the target, filter `vis_0.01` (QPS (recall; configuration)):

| Method | ≥ 0.9 | ≥ 0.95 | ≥ 0.99 | Highest recall |
|---|---|---|---|---|
| faiss-flat | 17,643 (0.999; flat) | 17,643 (0.999; flat) | 17,643 (0.999; flat) | 17,643 (0.999; flat) |
| faiss-hnsw | 64 (0.958; M-16 ef=5000) | 64 (0.958; M-16 ef=5000) | – | 25 (0.988; M-16 ef=10000) |
| faiss-ivfpqfs | 1,516 (0.956; nlist-2048 nprobe=1000 k_reorder=10) | 1,516 (0.956; nlist-2048 nprobe=1000 k_reorder=10) | 846 (0.998; nlist-2048 nprobe=2048 k_reorder=10) | 720 (0.999; nlist-2048 nprobe=2048 k_reorder=100) |

Best QPS at recall@10 at least the target, filter `vis_0.001` (QPS (recall; configuration)):

| Method | ≥ 0.9 | ≥ 0.95 | ≥ 0.99 | Highest recall |
|---|---|---|---|---|
| faiss-flat | 52,944 (0.999; flat) | 52,944 (0.999; flat) | 52,944 (0.999; flat) | 52,944 (0.999; flat) |
| faiss-hnsw | 25 (0.929; M-16 ef=10000) | – | – | 25 (0.929; M-16 ef=10000) |
| faiss-ivfpqfs | 1,011 (0.999; nlist-2048 nprobe=2048 k_reorder=10) | 1,011 (0.999; nlist-2048 nprobe=2048 k_reorder=10) | 1,011 (0.999; nlist-2048 nprobe=2048 k_reorder=10) | 1,011 (0.999; nlist-2048 nprobe=2048 k_reorder=10) |

