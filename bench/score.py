# Add recall@10 (ANN-Benchmarks rule) to the rows of the index's run CSV.
# usage: python3 -I score.py DATASET_DIR RESULTS_DIR   (reads RESULTS_DIR/ours.csv, writes RESULTS_DIR/ours_scored.csv)
import csv, os, sys
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from annlib import load, recall  # noqa: E402

ddir, rdir = sys.argv[1], sys.argv[2]
ds = load(ddir)
rows = list(csv.DictReader(open(os.path.join(rdir, "ours.csv"))))
for r in rows:
    ids = np.load(os.path.join(rdir, r["ids"]))
    r["recall"] = f"{recall(ds, ids, r['filter']):.4f}"
    print(r["method"], r["filter"], r["params"], "recall", r["recall"], "qps", r["qps"])
with open(os.path.join(rdir, "ours_scored.csv"), "w", newline="") as f:
    w = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
    w.writeheader()
    w.writerows(rows)
