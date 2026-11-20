"""Reader of `study_layered_dump`'s files (rust/src/v2/e2e.rs), for cascade.py and explore.py."""
import json
import os

import numpy as np

PREP = "/home/user/suite-data/prep"
FIT = "/home/user/suite-data/work"
NAMES = {"glove": "glove-100-angular", "yahoo": "yahoo-minilm-384-normalized", "laion": "laion-clip-512-normalized"}


class Dump:
    def __init__(self, prefix, ds):
        self.ds = ds
        meta = {}
        for line in open(prefix + ".meta"):
            k, v = line.split(" ", 1)
            meta[k] = v.strip()
        self.nq, self.K, self.M, self.L = int(meta["nq"]), int(meta["k"]), int(meta["m"]), int(meta["levels"])
        self.level_bytes = json.loads(meta["level_bytes"])
        self.n = int(meta["n"])
        self.ndl = int(meta["ndl"])
        self.clu = np.fromfile(prefix + ".clu", np.float32).reshape(self.nq, self.K, 4)  # mu, sigma, order key, stream key
        self.idx = np.fromfile(prefix + ".idx", np.uint32).reshape(self.nq, self.M, 2)
        w = 5 + 2 * self.L
        self.raw = np.memmap(prefix + ".rows", np.float32, mode="r").reshape(-1, w)
        self.ent = np.memmap(prefix + ".rows", np.uint32, mode="r").reshape(-1, w)[:, 0]
        self.kst = np.fromfile(prefix + ".kst", np.float32).reshape(self.K, 2)  # per cluster: mean key, mean key^2
        self.set = meta.get("set", "none")
        self.off = np.concatenate([[0], np.cumsum(self.idx[:, :, 1].astype(np.int64).ravel())]).reshape(-1)
        ds = ds.split("_")[0]  # DS_fit: the same fit built with other levels
        fit = f"{FIT}/{NAMES[ds]}/vexact/index"
        mq8, msc = np.load(f"{fit}/mq8.npy"), np.load(f"{fit}/msc.npy")
        self.mn = np.linalg.norm(mq8.astype(np.float64) * msc[:, None], axis=1)
        self.cstart = np.load(f"{fit}/cstart.npy")
        self.grp = np.load(f"{fit}/grp.npy")
        self.prep = f"{PREP}/{NAMES[ds]}"
        self.d = json.load(open(f"{self.prep}/meta.json"))["d"]  # the vectors' own dimension (n pads it to 32s)

    def cluster(self, qi, j):
        """Rows of the j-th cluster of query qi: entity, exact, key, key interval, level intervals (rows x L x 2)."""
        a, b = self.off[qi * self.M + j], self.off[qi * self.M + j + 1]
        r = self.raw[a:b]
        return self.ent[a:b], r[:, 1], r[:, 2], r[:, 3], r[:, 4], r[:, 5:].reshape(-1, self.L, 2)

    def truth(self):
        """nq x depth entity ids, best first, within the dump's visible set."""
        return np.load(f"{self.prep}/order.npy" if self.set == "none" else f"{self.prep}/order_{self.set}.npy", mmap_mode="r")
