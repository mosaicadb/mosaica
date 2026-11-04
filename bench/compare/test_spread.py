"""The dense-tile spread check on synthetic tiles: a uniform sample passes, a skewed one fails.

    .venv/bin/python test_spread.py

Each tile is 16 by 16 sub-cells whose item counts are heavy-tailed, from one item to tens of
millions, as GBIF's are where records crowd onto land and a few lie at sea; a sample is K of a
tile's items drawn without replacement. It exits with an error at the first check that fails.
"""

from __future__ import annotations

import tempfile

import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq

from cameras import tile_centre
from dataset import DATASET
from frame import K, Camera, Filter, Frame, interleave
from oracle import VIEWER, Z_LIMIT, Oracle, spread, spread_totals, spread_z, unlike_uniform

SIDE = 16
rng = np.random.default_rng(7)


def tile() -> dict[tuple[int, int], int]:
    """Each occupied sub-cell's count of the tile's items."""
    n = np.floor(np.exp(rng.normal(8, 3, SIDE * SIDE))).astype(np.int64)
    return {(i % SIDE, i // SIDE): int(v) for i, v in enumerate(n) if v}


def draw(cells: dict, k: int, corner: float = 0.0) -> list[tuple[int, int]]:
    """The sub-cells of `k` items drawn without replacement, a share `corner` of them on average
    from the items of the tile's south-west quarter alone."""
    keys = list(cells)
    n = np.array([cells[c] for c in keys])
    quarter = np.array([x < SIDE // 2 and y < SIDE // 2 for x, y in keys])
    skewed = rng.binomial(k, corner)
    out = []
    for counts, m in ((n, k - skewed), (np.where(quarter, n, 0), skewed)):
        m = min(m, int(counts.sum()))
        if m:
            items = rng.choice(int(counts.sum()), m, replace=False)
            out += [keys[i] for i in np.searchsorted(np.cumsum(counts), items, side="right")]
    return out


def workload(tiles: list[dict], corner: float = 0.0, zoom: int = 0) -> list[dict]:
    return [{**spread(cells, draw(cells, K, corner)), "zoom": zoom} for cells in tiles]


def check_one_tile() -> None:
    """A uniform sample's chi-square has its degrees of freedom as mean and about twice them as
    variance, and its coverage the expected mean within the variance's bound."""
    cells = tile()
    s = [spread(cells, draw(cells, K)) for _ in range(1500)]
    df, expected, bound = s[0]["dense_chi2_df"], s[0]["dense_cover_expected"], s[0]["dense_cover_var"]
    chi2 = np.array([x["dense_chi2"] for x in s])
    cover = np.array([x["dense_cover"] for x in s])
    print(f"one tile, {len(cells)} sub-cells, {df} df: chi-square mean {chi2.mean() / df:.3f} a df, "
          f"variance {chi2.var() / (2 * df):.2f} of 2 df; cover mean {cover.mean():.1f} of "
          f"{expected:.1f}, variance {cover.var():.1f}, bound {bound:.1f}")
    assert abs(chi2.mean() / df - 1) < 0.05
    assert 0.7 < chi2.var() / (2 * df) < 1.4
    assert abs(cover.mean() - expected) < 4 * np.sqrt(bound / len(s))
    assert cover.var() < 1.1 * bound


def check_uniform_workloads() -> None:
    """Workloads of uniform samples stay inside `Z_LIMIT`, with deviates of about unit spread."""
    tiles = [tile() for _ in range(60)]
    zs = np.array([spread_z(spread_totals(workload(tiles))) for _ in range(25)])
    print(f"uniform workloads of {len(tiles)} tiles: cover z sd {zs[:, 0].std():.2f}, chi-square z "
          f"mean {zs[:, 1].mean():+.2f} sd {zs[:, 1].std():.2f}, largest |z| {abs(zs).max():.1f}")
    assert abs(zs).max() < Z_LIMIT
    assert zs[:, 1].std() < 1.5 and abs(zs[:, 1].mean()) < 1


def check_skewed_workloads() -> None:
    """A fifth of each sample drawn from one corner fails, and so does each tile's first K items in
    Morton order, a sample taken in storage order."""
    tiles = [tile() for _ in range(60)]
    corner = spread_totals(workload(tiles, corner=0.2))
    first = []
    for cells in tiles:
        sent: list[tuple[int, int]] = []
        for c in sorted(cells, key=lambda c: interleave(*c)):
            sent += [c] * min(cells[c], K - len(sent))
        first.append({**spread(cells, sent), "zoom": 0})
    first = spread_totals(first)
    for name, e in (("a fifth from one corner", corner), ("the first K in Morton order", first)):
        z = spread_z(e)
        print(f"{name}: cover z {z[0]:+.1f}, chi-square z {z[1]:+.1f}")
        assert "all" in unlike_uniform(e)


def check_one_zoom_skewed() -> None:
    """Samples skewed on a few tiles of one zoom are marked at that zoom, where the workload's sum
    can hide them."""
    tiles = [tile() for _ in range(300)]
    e = spread_totals(workload(tiles[:-3], zoom=3) + workload(tiles[-3:], corner=0.3, zoom=9))
    odd = unlike_uniform(e)
    print(f"three tiles of 300 with 30% from one corner, at zoom 9: marked {odd}; deviates "
          f"{', '.join(f'{v:+.1f}' for v in spread_z(e))} for the workload, "
          f"{', '.join(f'{v:+.1f}' for v in spread_z(e['spread_by_zoom']['9']))} at zoom 9")
    assert "zoom 9" in odd and "zoom 3" not in odd


def check_repeats_counted_once() -> None:
    """`Oracle.coverage` judges a tile once under each filter, however often a workload asks."""
    z, t, n = 3, (3, 3), 4000
    s = 32 - z
    qx = ((t[0] << s) + rng.integers(0, 1 << s, n)).astype(np.uint32)
    qy = ((t[1] << s) + rng.integers(0, 1 << s, n)).astype(np.uint32)
    year = rng.integers(1990, 2020, n).astype(np.int32)
    term = VIEWER["terms"][0]
    access = [[term]] * n if DATASET.access_is_list else [term] * n
    camera = Camera(z, *tile_centre(*t, z))
    early = Filter([{"col": DATASET.year, "op": "range", "value": {"lt": 2005}}], "early")
    with tempfile.TemporaryDirectory() as d:
        path = f"{d}/{DATASET.name}-spread.parquet"
        pq.write_table(pa.table({"qx": qx, "qy": qy, DATASET.access: access, DATASET.year: year}), path)
        oracle = Oracle(path)
        seen: set = set()
        judged = []
        for filt, keep in ((None, np.ones(n, bool)), (None, np.ones(n, bool)), (early, year < 2005)):
            pick = rng.choice(np.flatnonzero(keep), K, replace=False)
            got = Frame(tiles={}, grid={}, points={t: np.stack([qx[pick], qy[pick]], axis=1)})
            judged.append(oracle.coverage(camera, filt, got, oracle.frame(camera, filt), seen)
                          ["dense_tiles_sampled"])
    print(f"the same tile unfiltered, again, then filtered: judged {judged}")
    assert judged == [1, 0, 1]


if __name__ == "__main__":
    check_one_tile()
    check_uniform_workloads()
    check_skewed_workloads()
    check_one_zoom_skewed()
    check_repeats_counted_once()
    print("ok")
