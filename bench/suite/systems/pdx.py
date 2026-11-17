# PDX-BOND (Kuffo et al., SIGMOD 2025; github.com/cwida/PDX): exact k-NN with dimension pruning (BOND) over the PDX
# (column-major, blocks of 10,240 vectors) layout, no index. BOND was dropped from PDX's main branch in v0.3; the
# Python package `pdxearch` is built from 91618e0 (Aug 2025, the last revision with BOND and filtered search) by
# install/pdx.sh. L2 over the unit vectors (the same order as inner product). The layout file is written here block by
# block (the package's add() materialises it in memory, three copies at 2,048 dimensions), in the format of
# BaseIndexPDXFlat._materialize_index with bond=True, and opened with restore() (mapped). PDX needs at least 128
# dimensions, a multiple of 4: narrower vectors get zero columns, which keep every distance.
# Under a visible set: filtered_search with a selection vector over the layout's order (its predicate evaluator's
# form), computed when the visible set is set.
import math
import os

import numpy as np

from .base import System, Unavailable

BLOCK = 10240           # PDXConstants.PDXEARCH_VECTOR_SIZE
VERTICAL = 0.75         # PDXConstants.VERTICAL_PROPORTION_DIM
HGROUP = 64             # PDXConstants.HORIZONTAL_DIMENSIONS_GROUPING


class PdxBond(System):
    name = "pdx-bond"
    types = ("exact",)

    def build(self, threads):
        try:
            from pdxearch.compiled import IndexBONDFlat
        except ImportError as e:
            raise Unavailable(f"pdxearch (PDX with BOND) is not installed ({e}); see install/pdx.sh")
        X = self.prep.corpus()
        N, d = X.shape
        self.dp = max(128, -(-d // 4) * 4)
        os.makedirs(self.work, exist_ok=True)
        path = os.path.join(self.work, "pdx_bond.bin")
        rng = np.random.default_rng(42)
        perm = rng.permutation(N).astype(np.uint32)
        h = int(self.dp * VERTICAL)
        if h % HGROUP:
            h = math.floor(h / HGROUP + 0.5) * HGROUP
        v = self.dp - h
        if v == 0:
            h, v = HGROUP, self.dp - HGROUP
        nparts = -(-N // BLOCK)
        sizes = [min(BLOCK, N - i * BLOCK) for i in range(nparts)]
        means = np.zeros(self.dp, np.float64)
        with open(path, "wb") as f:
            f.write(np.array([self.dp, v, h, nparts] + sizes, np.int32).tobytes())
            for i in range(nparts):
                B = np.zeros((sizes[i], self.dp), np.float32)
                B[:, :d] = X[np.sort(perm[i * BLOCK:i * BLOCK + sizes[i]])]
                # rows in the block in the order written below (sorted within the block, for faster reads)
                means += B.sum(0, dtype=np.float64)
                f.write(B.tobytes("F"))
            order = np.concatenate([np.sort(perm[i * BLOCK:i * BLOCK + sizes[i]]) for i in range(nparts)])
            f.write(order.astype(np.uint32).tobytes())
            f.write((means / N).astype(np.float32).tobytes())
            f.write(bytes([1, 0]))  # normalised, not IVF
        self.order, self.sizes = order, sizes
        self.ix = IndexBONDFlat()
        self.ix.restore(path)
        self.path = path
        self.qbuf = np.zeros(self.dp, np.float32)
        return dict(blocks=nparts)

    def index_bytes(self):
        return os.path.getsize(self.path)

    def settings(self, qtype, filtered):
        return ["selection"] if filtered else ["default"]

    def set_visible(self, ids, name=None):
        self.vis = ids
        if ids is None:
            return
        m = np.zeros(self.prep.N, np.uint8)
        m[ids] = 1
        self.sel = np.ascontiguousarray(m[self.order])
        self.npass = np.add.reduceat(self.sel.astype(np.uint32), np.cumsum([0] + self.sizes[:-1])).astype(np.uint32)

    def knn(self, q, k):
        self.qbuf[:len(q)] = q
        if self.vis is None:
            ids, _ = self.ix.search(self.qbuf, k)
        else:
            ids, _ = self.ix.filtered_search(self.qbuf, k, self.npass, self.sel)
        return ids.astype(np.int64)

    def close(self):
        self.ix = None
