# How many of the index's clusters hold at least one visible row, for each visible set given.
# usage: python3 -I clusters.py INDEX_DIR VIS.npy...
import sys
import numpy as np

ix = sys.argv[1]
perm = np.load(f"{ix}/perm.npy")      # row -> entity id
cstart = np.load(f"{ix}/cstart.npy")  # first row of each cluster, then the row count
K = len(cstart) - 1
row_of = np.empty(len(perm), np.int64)
row_of[perm] = np.arange(len(perm))
cl_of = np.searchsorted(cstart, np.arange(len(perm)), side="right") - 1
size = np.diff(cstart)
print("set,items,clusters_touched,of,items_in_touched_clusters")
for f in sys.argv[2:]:
    ids = np.load(f)
    c = np.unique(cl_of[row_of[ids]])
    print(f"{f.split('/')[-1][:-4]},{len(ids)},{len(c)},{K},{size[c].sum()}")
