# Is the 50k corpus a subset of the 1M corpus, and are the item queries the same? (read-only)
import numpy as np
a = np.load("/home/joe/code/vexact-combined-data/c50k/corpus.npy", mmap_mode="r"); b = np.load("/home/joe/code/vexact-combined-data/c1m/corpus.npy", mmap_mode="r")
qa = np.load("/home/joe/code/vexact-combined-data/c50k/item_q.npy"); qb = np.load("/home/joe/code/vexact-combined-data/c1m/item_q.npy")
print("item_q shapes", qa.shape, qb.shape, "first 20 identical:", bool(np.array_equal(qa[:20], qb[:20])))
probe = np.asarray(a[[0, 1000, 25000, 49499]]).view(np.uint16); found = [False] * 4
for s in range(0, len(b), 100000):
    blk = np.asarray(b[s:s + 100000]).view(np.uint16)
    for i in range(4):
        if not found[i] and (blk == probe[i]).all(1).any(): found[i] = True
print("4 probe rows of the 50k corpus found in the 1M corpus:", found, "; 1M rows", len(b))
