# The cluster-size and group rules, shared by tools/fit.py and tools/partition.py (and checked against the Rust fit's
# `fit::per_cluster_by_n` and `fit::Groups::count` by a test of the vexact crate). Only the standard library.
import math


def per_cluster(N):
    """Items per cluster for N items by research/levels-clusters' rule: 2,000 x sqrt(N / 1M), rounded in log2 to a
    power-of-two multiple of 2,000 (ties down), at least 2,000. That is 2,000 up to 2M items, 4,000 up to 8M, 8,000 up
    to 32M, and double for each further 4x (64,000 at 1B)."""
    return 2000 * 2 ** max(0, math.ceil(math.log2(N / 1e6) / 2 - 0.5))


def groups_for(rule, N, K, n0):
    """Groups of clusters: by-k8 (enough items per group for a basis, 20 n, at most 64 groups and 8 clusters per group
    on average), by-n (the same without the 8 clusters per group), or a number (at most K)."""
    if rule == "by-k8":
        return int(max(1, min(64, K // 8, N // (20 * n0))))
    if rule == "by-n":
        return int(max(1, min(64, K, N // (20 * n0))))
    return max(1, min(int(rule), K))
