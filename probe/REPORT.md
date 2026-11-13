# Probe-cluster experiments (partial: run stopped before completion)

Status: INCOMPLETE. The run was cut off before it finished. Finished and logged: Experiment A on
nytimes-256 (K = 256, 1,024, 4,096, all 10,000 test queries) and Experiment A on glove-100 at
K = 1,024. Not finished: glove K = 4,096 and 16,384, the two-level tree with bound-ranked groups
(`expA_tree.py`, never run), and every full Experiment B run (`expB.py`; only a 100-query smoke test
on nytimes ran, and it is not recorded here). All background processes were killed by PID.

Measured: counts of items scanned and recall (ANN-Benchmarks `knn_threshold`: d_k + 1e-3, confirmed
against `ann_benchmarks/plotting/metrics.py`). nytimes has 9 zero-vector test queries with NaN
distances, which ANN-Benchmarks scores 0, so tolerant recall is capped at 0.9991.

Tables: `tables_partial.md` (from `report.py out`); logs: `expA_*.log`; curves: `out/expA_*.json/png`.
