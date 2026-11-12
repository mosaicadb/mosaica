"""Fit a 2D layout on a sample of vectors, then place every other vector against it.

A fit is cuML's UMAP over the sample's own CAGRA kNN graph. A placement searches the same sample's
CAGRA index and puts each query at the similarity-weighted mean of its neighbours' fitted
positions, so a corpus of any size costs one search per row on the card after a UMAP over a few
million.

Both halves take a `metric`: `cosine` for an embedding whose direction is its meaning, and
`euclidean` for a latent space such as scVI's, which CAGRA searches as squared Euclidean distance.

The layout is not reproducible under a seed: CAGRA's index build is approximate and takes none,
so UMAP is handed a different graph each run.
"""

from __future__ import annotations

import contextlib
import threading
import time

import numpy as np

#: One set of UMAP parameters, and `n_neighbors` is also the graph's `k`.
UMAP_PARAMS = dict(n_neighbors=15, min_dist=0.1, n_components=2)

SEED = 0

#: CAGRA's graph degree and its intermediate, and the search's candidate list.
GRAPH_DEGREE = 32
ITOPK = 128

#: Query rows per search call.
BATCH_ROWS = 50_000

METRICS = ("cosine", "euclidean")


@contextlib.contextmanager
def step(name: str, out: dict):
    t = time.time()
    yield
    out[name] = round(time.time() - t, 2)
    print(f"    {name}: {out[name]:.1f}s", flush=True)


class DeviceWatermark:
    """Peak device memory, sampled from the driver rather than from an allocator.

    CuPy, cuML, cuVS and RMM each keep their own pool, so what the driver reports free is the only
    number that covers all of them.
    """

    def __init__(self, interval: float = 0.2):
        import cupy as cp

        self.total = cp.cuda.runtime.memGetInfo()[1]
        self.interval = interval
        self.peak = 0
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._run, daemon=True)

    def _run(self) -> None:
        import cupy as cp

        while not self._stop.wait(self.interval):
            free, total = cp.cuda.runtime.memGetInfo()
            self.peak = max(self.peak, total - free)

    def __enter__(self):
        self._thread.start()
        return self

    def __exit__(self, *exc):
        self._stop.set()
        self._thread.join()

    @property
    def peak_gb(self) -> float:
        return self.peak / 2**30


def normalise(block: np.ndarray) -> np.ndarray:
    """L2-normalised float16, by way of a float32 working copy."""
    out = np.asarray(block, dtype=np.float32)
    norms = np.linalg.norm(out, axis=1, keepdims=True)
    np.divide(out, np.maximum(norms, 1e-12), out=out)
    return out.astype(np.float16)


#: CAGRA searches vectors whose width is a multiple of this at full speed. A 50-wide float16 vector
#: was measured at about 1 ms a query against 2 us once padded to 64, on cuVS 26.10.
PAD_TO = 32


def padded(block: np.ndarray) -> np.ndarray:
    """`block` with zero columns appended to a multiple of `PAD_TO`, which changes no distance."""
    short = -block.shape[1] % PAD_TO
    if not short:
        return block
    return np.concatenate([block, np.zeros((len(block), short), dtype=block.dtype)], axis=1)


def cagra_index(resident, metric: str):
    from cuvs.neighbors import cagra

    assert metric in METRICS, f"metric {metric!r} is not one of {METRICS}"
    return cagra.build(
        cagra.IndexParams(metric="cosine" if metric == "cosine" else "sqeuclidean",
                          intermediate_graph_degree=2 * GRAPH_DEGREE, graph_degree=GRAPH_DEGREE),
        resident,
    )


def knn_graph(
    block: np.ndarray,
    t: dict,
    *,
    k: int = UMAP_PARAMS["n_neighbors"],
    batch_rows: int = BATCH_ROWS,
    metric: str = "cosine",
) -> tuple[np.ndarray, np.ndarray]:
    """A kNN graph over a resident block, through one CAGRA index, with distances in `metric`."""
    import cupy as cp
    from cuvs.neighbors import cagra

    block = padded(block)
    n = len(block)
    t0 = time.time()
    resident = cp.asarray(block)
    index = cagra_index(resident, metric)
    cp.cuda.runtime.deviceSynchronize()
    t["build_seconds"] = round(time.time() - t0, 1)

    best_i = np.zeros((n, k), dtype=np.int32)
    best_d = np.zeros((n, k), dtype=np.float32)
    t1 = time.time()
    params = cagra.SearchParams(itopk_size=ITOPK)
    for lo in range(0, n, batch_rows):
        hi = min(lo + batch_rows, n)
        q = cp.asarray(block[lo:hi])
        nb = cp.empty((hi - lo, k), dtype=cp.uint32)
        ds = cp.empty((hi - lo, k), dtype=cp.float32)
        cagra.search(params, index, q, k, neighbors=nb, distances=ds)
        best_i[lo:hi] = cp.asnumpy(nb.astype(cp.int32))
        best_d[lo:hi] = cp.asnumpy(ds)
        del q, nb, ds
    cp.cuda.runtime.deviceSynchronize()
    search = time.time() - t1
    t["search_seconds"] = round(search, 1)
    print(f"    cagra build {t['build_seconds']:.1f}s, {n:,} queries in {search:.1f}s "
          f"({n / search:,.0f}/s)", flush=True)
    del resident, index
    cp.get_default_memory_pool().free_all_blocks()

    # An approximate index does not always list a row first among its own neighbours, and UMAP
    # would then place it on another row's position. Corpora with near-duplicates hit this.
    rows = np.arange(n, dtype=np.int32)
    t["self_first"] = float((best_i[:, 0] == rows).mean())
    wrong = best_i[:, 0] != rows
    if wrong.any():
        best_i[wrong, 0] = rows[wrong]
        best_d[wrong, 0] = 0.0
    np.clip(best_d, 0.0, None, out=best_d)  # fp16 rounds a distance of zero below it
    if metric == "euclidean":
        np.sqrt(best_d, out=best_d)
    return best_i, best_d


def layout(
    nbi: np.ndarray, nbd: np.ndarray, t: dict, *, managed: bool = False, metric: str = "cosine"
) -> np.ndarray:
    """cuML's UMAP over a precomputed graph.

    With `precomputed_knn`, cuML takes only the row count from `X`, so a `(n, 1)` array of zeros
    stands in for the vectors and they are not on the card while UMAP runs.
    """
    if managed:
        import rmm

        rmm.reinitialize(managed_memory=True, pool_allocator=True)

    from cuml.manifold import UMAP

    with step("umap", t):
        xy = UMAP(**UMAP_PARAMS, metric=metric, random_state=SEED, output_type="numpy",
                  precomputed_knn=(nbi, nbd)).fit_transform(
            np.zeros((len(nbi), 1), dtype=np.float32)
        )
    return np.asarray(xy, dtype=np.float32)


def fit_layout(
    block: np.ndarray, t: dict, *, managed: bool = False, metric: str = "cosine"
) -> np.ndarray:
    """The fit set's own positions: UMAP over its own CAGRA graph."""
    nbi, nbd = knn_graph(block, t, metric=metric)
    xy = layout(nbi, nbd, t, managed=managed, metric=metric)
    del nbi, nbd
    return xy


class Placer:
    """One CAGRA index over the fit set, with the fit positions beside it on the card.

    `place(block)` returns each query row's position: the mean of its `k` nearest fit rows'
    positions, weighted by similarity. Under `cosine` the weight is `1 - distance`, so an
    orthogonal neighbour counts for nothing; under `euclidean` it is the inverse of the distance,
    so a neighbour at distance zero decides the position.
    """

    def __init__(self, fit_block: np.ndarray, fit_xy: np.ndarray, *, metric: str = "cosine"):
        import cupy as cp
        from cuvs.neighbors import cagra

        self.metric = metric
        self.k = UMAP_PARAMS["n_neighbors"]
        self._resident = cp.asarray(padded(fit_block))
        self.index = cagra_index(self._resident, metric)
        self.positions = cp.asarray(fit_xy)
        self.params = cagra.SearchParams(itopk_size=ITOPK)

    def place(self, block: np.ndarray) -> np.ndarray:
        import cupy as cp
        from cuvs.neighbors import cagra

        out = np.empty((len(block), 2), dtype=np.float32)
        for lo in range(0, len(block), BATCH_ROWS):
            part = padded(block[lo : lo + BATCH_ROWS])
            m = len(part)
            q = cp.asarray(part)
            nb = cp.empty((m, self.k), dtype=cp.uint32)
            ds = cp.empty((m, self.k), dtype=cp.float32)
            cagra.search(self.params, self.index, q, self.k, neighbors=nb, distances=ds)
            if self.metric == "cosine":
                w = cp.clip(1.0 - ds, 0.0, None) + 1e-6
            else:
                w = 1.0 / (cp.sqrt(cp.clip(ds, 0.0, None)) + 1e-4)
            w /= w.sum(axis=1, keepdims=True)
            got = self.positions[nb.astype(cp.int32)]
            out[lo : lo + m] = cp.asnumpy((got * w[:, :, None]).sum(axis=1))
            del q, nb, ds, w, got
        return out

    def close(self) -> None:
        import cupy as cp

        cp.cuda.runtime.deviceSynchronize()
        del self._resident, self.index, self.positions
        cp.get_default_memory_pool().free_all_blocks()
