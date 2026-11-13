### glove-100-angular

Exact (all test queries, one thread):

| Method | Filter | QPS | Mean ms | Recall@10 | Sets differing from brute force | Score lists differing |
|---|---|---|---|---|---|---|
| ours exact | all | 29.9 | 33.49 | 1.0000 | 0 | 0 |
| ours flat | all | 75.5 | 13.25 | 1.0000 | 0 | 0 |
| ours exact | vis_0.001 | 2,442.5 | 0.41 | 1.0000 | 0 | 0 |
| ours flat | vis_0.001 | 30,916.5 | 0.03 | 1.0000 | 0 | 0 |
| ours exact | vis_0.01 | 1,354.8 | 0.74 | 1.0000 | 0 | 0 |
| ours flat | vis_0.01 | 4,504.5 | 0.22 | 1.0000 | 0 | 0 |
| ours exact | vis_0.1 | 173.4 | 5.77 | 1.0000 | 0 | 0 |
| ours flat | vis_0.1 | 210.9 | 4.74 | 1.0000 | 0 | 0 |
| ours exact | vis_0.5 | 49.7 | 20.13 | 1.0000 | 0 | 0 |
| ours flat | vis_0.5 | 50.0 | 19.98 | 1.0000 | 0 | 0 |

Best QPS at recall@10 at least the target, filter `all` (QPS (recall; configuration)):

| Method | ≥ 0.9 | ≥ 0.95 | ≥ 0.99 | Highest recall |
|---|---|---|---|---|
| faiss-hnsw | 2,668 (0.908; M-48 ef=80) | 1,501 (0.956; M-96 ef=120) | 605 (0.991; M-64 ef=400) | 280 (0.998; M-96 ef=800) |
| faiss-ivfpqfs | 4,642 (0.908; nlist-1204 nprobe=100 k_reorder=10) | 2,696 (0.952; nlist-512 nprobe=50 k_reorder=100) | 1,145 (0.995; nlist-512 nprobe=200 k_reorder=100) | 485 (0.996; nlist-512 nprobe=200 k_reorder=1000) |
| hnswlib | 3,004 (0.904; M-48 ef=80) | 1,536 (0.957; M-96 ef=120) | 611 (0.991; M-64 ef=400) | 268 (0.999; M-96 ef=800) |
| ours approx | 232 (0.933; nprobe=64 rerank=100 level=5) | 116 (0.964; nprobe=128 rerank=100 level=5) | 57 (0.990; nprobe=256 rerank=1000 level=5) | 57 (0.990; nprobe=256 rerank=1000 level=5) |

Best QPS at recall@10 at least the target, filter `vis_0.5` (QPS (recall; configuration)):

| Method | ≥ 0.9 | ≥ 0.95 | ≥ 0.99 | Highest recall |
|---|---|---|---|---|
| faiss-hnsw | 1,271 (0.915; M-16 ef=400) | 659 (0.950; M-16 ef=800) | – | 659 (0.950; M-16 ef=800) |
| faiss-ivfpqfs | 2,722 (0.923; nlist-2048 nprobe=200 k_reorder=10) | 1,729 (0.968; nlist-2048 nprobe=200 k_reorder=100) | – | 596 (0.969; nlist-2048 nprobe=200 k_reorder=1000) |
| hnswlib | 579 (0.952; M-16 ef=400) | 579 (0.952; M-16 ef=400) | – | 579 (0.952; M-16 ef=400) |

Best QPS at recall@10 at least the target, filter `vis_0.1` (QPS (recall; configuration)):

| Method | ≥ 0.9 | ≥ 0.95 | ≥ 0.99 | Highest recall |
|---|---|---|---|---|
| faiss-hnsw | 684 (0.918; M-16 ef=800) | – | – | 684 (0.918; M-16 ef=800) |
| faiss-ivfpqfs | 2,449 (0.931; nlist-2048 nprobe=200 k_reorder=10) | 1,601 (0.951; nlist-2048 nprobe=200 k_reorder=100) | – | 637 (0.951; nlist-2048 nprobe=200 k_reorder=1000) |
| hnswlib | 423 (0.940; M-16 ef=120) | 142 (0.988; M-16 ef=400) | – | 142 (0.988; M-16 ef=400) |

Best QPS at recall@10 at least the target, filter `vis_0.01` (QPS (recall; configuration)):

| Method | ≥ 0.9 | ≥ 0.95 | ≥ 0.99 | Highest recall |
|---|---|---|---|---|
| faiss-hnsw | – | – | – | 686 (0.790; M-16 ef=800) |
| faiss-ivfpqfs | – | – | – | 2,032 (0.895; nlist-2048 nprobe=200 k_reorder=100) |
| hnswlib | 139 (0.941; M-16 ef=40) | 51 (0.988; M-16 ef=120) | 17 (0.999; M-16 ef=400) | 17 (0.999; M-16 ef=400) |

Best QPS at recall@10 at least the target, filter `vis_0.001` (QPS (recall; configuration)):

| Method | ≥ 0.9 | ≥ 0.95 | ≥ 0.99 | Highest recall |
|---|---|---|---|---|
| faiss-hnsw | – | – | – | 700 (0.439; M-16 ef=800) |
| faiss-ivfpqfs | – | – | – | 3,300 (0.742; nlist-2048 nprobe=200 k_reorder=10) |
| hnswlib | 65 (0.926; M-16 ef=10) | 18 (0.996; M-16 ef=40) | 18 (0.996; M-16 ef=40) | 3 (1.000; M-16 ef=400) |

### nytimes-256-angular

Exact (all test queries, one thread):

| Method | Filter | QPS | Mean ms | Recall@10 | Sets differing from brute force | Score lists differing |
|---|---|---|---|---|---|---|
| ours exact | all | 44.6 | 22.42 | 0.9991 | 9 | 0 |
| ours flat | all | 109.7 | 9.12 | 0.9991 | 1 | 0 |
| ours exact | vis_0.001 | 2,096.7 | 0.48 | 0.9991 | 0 | 0 |
| ours flat | vis_0.001 | 73,726.1 | 0.01 | 0.9991 | 0 | 0 |
| ours exact | vis_0.01 | 1,490.4 | 0.67 | 0.9991 | 0 | 0 |
| ours flat | vis_0.01 | 12,560.2 | 0.08 | 0.9991 | 0 | 0 |
| ours exact | vis_0.1 | 499.2 | 2.00 | 0.9991 | 0 | 0 |
| ours flat | vis_0.1 | 866.4 | 1.15 | 0.9991 | 0 | 0 |
| ours exact | vis_0.5 | 133.6 | 7.48 | 0.9991 | 0 | 0 |
| ours flat | vis_0.5 | 146.3 | 6.84 | 0.9991 | 0 | 0 |

Best QPS at recall@10 at least the target, filter `all` (QPS (recall; configuration)):

| Method | ≥ 0.9 | ≥ 0.95 | ≥ 0.99 | Highest recall |
|---|---|---|---|---|
| faiss-hnsw | 3,707 (0.903; M-36 ef=40) | 1,534 (0.951; M-36 ef=120) | 360 (0.992; M-64 ef=400) | 131 (0.998; M-96 ef=800) |
| faiss-ivfpqfs | 4,247 (0.919; nlist-512 nprobe=100 k_reorder=10) | 2,496 (0.962; nlist-512 nprobe=200 k_reorder=10) | – | 1,522 (0.967; nlist-512 nprobe=200 k_reorder=100) |
| hnswlib | 4,221 (0.903; M-36 ef=40) | 1,727 (0.952; M-64 ef=80) | 374 (0.991; M-64 ef=400) | 152 (0.998; M-96 ef=800) |
| ours approx | 199 (0.911; nprobe=64 rerank=1000 level=5) | 109 (0.957; nprobe=128 rerank=1000 level=5) | – | 56 (0.985; nprobe=256 rerank=1000 level=5) |

Best QPS at recall@10 at least the target, filter `vis_0.5` (QPS (recall; configuration)):

| Method | ≥ 0.9 | ≥ 0.95 | ≥ 0.99 | Highest recall |
|---|---|---|---|---|
| faiss-hnsw | 1,820 (0.911; M-16 ef=200) | 677 (0.953; M-16 ef=600) | – | 506 (0.963; M-16 ef=800) |
| faiss-ivfpqfs | 3,565 (0.909; nlist-2048 nprobe=200 k_reorder=10) | – | – | 1,931 (0.911; nlist-2048 nprobe=200 k_reorder=100) |
| hnswlib | 1,362 (0.923; M-16 ef=120) | 431 (0.965; M-16 ef=400) | – | 431 (0.965; M-16 ef=400) |

Best QPS at recall@10 at least the target, filter `vis_0.1` (QPS (recall; configuration)):

| Method | ≥ 0.9 | ≥ 0.95 | ≥ 0.99 | Highest recall |
|---|---|---|---|---|
| faiss-hnsw | 503 (0.910; M-16 ef=800) | – | – | 503 (0.910; M-16 ef=800) |
| faiss-ivfpqfs | – | – | – | 1,904 (0.862; nlist-2048 nprobe=200 k_reorder=100) |
| hnswlib | 312 (0.948; M-16 ef=120) | 96 (0.990; M-16 ef=400) | 96 (0.990; M-16 ef=400) | 96 (0.990; M-16 ef=400) |

Best QPS at recall@10 at least the target, filter `vis_0.01` (QPS (recall; configuration)):

| Method | ≥ 0.9 | ≥ 0.95 | ≥ 0.99 | Highest recall |
|---|---|---|---|---|
| faiss-hnsw | – | – | – | 502 (0.686; M-16 ef=800) |
| faiss-ivfpqfs | – | – | – | 4,277 (0.702; nlist-2048 nprobe=200 k_reorder=10) |
| hnswlib | 97 (0.941; M-16 ef=40) | 38 (0.996; M-16 ef=120) | 38 (0.996; M-16 ef=120) | 16 (1.000; M-16 ef=400) |

Best QPS at recall@10 at least the target, filter `vis_0.001` (QPS (recall; configuration)):

| Method | ≥ 0.9 | ≥ 0.95 | ≥ 0.99 | Highest recall |
|---|---|---|---|---|
| faiss-hnsw | – | – | – | 493 (0.299; M-16 ef=800) |
| faiss-ivfpqfs | – | – | – | 5,704 (0.448; nlist-2048 nprobe=200 k_reorder=100) |
| hnswlib | 42 (0.937; M-16 ef=10) | 16 (0.999; M-16 ef=40) | 16 (0.999; M-16 ef=40) | 8 (1.000; M-16 ef=120) |

