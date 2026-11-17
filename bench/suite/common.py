# Shared pieces of the suite: where things live, the prepared files of a dataset, and the timing protocol's summaries.
import json
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
RESULTS = os.path.join(HERE, "results")


def data_dir(arg=None):
    """The data directory (outside the tree): --data, else $SUITE_DATA, else ~/vexact-suite-data."""
    d = arg or os.environ.get("SUITE_DATA") or os.path.expanduser("~/vexact-suite-data")
    if os.path.realpath(d).startswith(os.path.realpath(REPO) + os.sep):
        sys.exit(f"the data directory {d} is inside the repository; put it outside the tree")
    return d


def prep_dir(data, dataset):
    return os.path.join(data, "prep", dataset)


def work_dir(data, dataset, system):
    return os.path.join(data, "work", dataset, system)


def dump_dir(data, suite, dataset, system):
    return os.path.join(data, "dumps", suite, dataset, system)


def results_dir(suite, dataset):
    return os.path.join(RESULTS, suite, dataset)


def share_label(s):
    """0.0001 -> '0.01%', 0.1 -> '10%'."""
    return f"{s * 100:g}%"


def filter_label(f):
    return f["name"] if isinstance(f, dict) else f


class Prep:
    """The prepared files of one dataset (prep.py writes them).

    corpus32.npy   N x d f32 unit vectors (row i is entity i); corpus16.npy the same rounded to f16 (optional)
    queries.npy    nq x d f64 unit vectors
    learn.npy      a sample of the query distribution (out-of-distribution datasets only), f32
    order.npy      nq x K int32: entity ids by f64 similarity, best first; the exact k-NN answer is a prefix
    thresholds.npy nq x len(shares) f64: midpoints between the k-th and (k+1)-th f64 similarity, k = round(share N)
    vis_<f>.npy    sorted int32 entity ids of visible set f; order_<f>.npy and thresholds_<f>.npy as above within it
    meta.json      N, d, nq, shares, the filters' shares and the truth's depth
    """

    def __init__(self, path):
        self.path = path
        with open(os.path.join(path, "meta.json")) as f:
            self.meta = json.load(f)
        self.N, self.d, self.nq = self.meta["N"], self.meta["d"], self.meta["nq"]
        self.shares = self.meta["shares"]
        self._cache = {}

    def f(self, name):
        return os.path.join(self.path, name)

    def load(self, name, mmap=True):
        if name not in self._cache:
            self._cache[name] = np.load(self.f(name), mmap_mode="r" if mmap else None)
        return self._cache[name]

    def corpus(self):
        return self.load("corpus32.npy")

    def queries(self):
        return self.load("queries.npy", mmap=False)

    def learn(self):
        return self.load("learn.npy") if os.path.exists(self.f("learn.npy")) else None

    def vis(self, filt):
        return None if filt in (None, "none") else self.load(f"vis_{filt}.npy", mmap=False)

    def order(self, filt):
        return self.load("order.npy" if filt in (None, "none") else f"order_{filt}.npy")

    def thresholds(self, filt):
        return self.load("thresholds.npy" if filt in (None, "none") else f"thresholds_{filt}.npy", mmap=False)

    def filter_shares(self, filt):
        """The answer shares prepared under filter `filt` (of the visible set)."""
        return self.shares if filt in (None, "none") else self.meta["filters"][filt]["shares"]

    def nvis(self, filt):
        return self.N if filt in (None, "none") else self.meta["filters"][filt]["n"]

    def answer_size(self, filt, share):
        return round(share * self.nvis(filt))


def summarise(ms):
    """The protocol's figures from per-query medians: the median over queries and the p90."""
    a = np.asarray(ms, dtype=np.float64)
    if len(a) == 0:
        return float("nan"), float("nan")
    return float(np.median(a)), float(np.percentile(a, 90))
