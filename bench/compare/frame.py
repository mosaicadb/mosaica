"""The frame every system is asked for, and the geometry that defines it.

A frame is what one screen of the map needs from a server: for a viewport of 1920 by 1080 pixels
at an integer zoom, the depth-`zoom` tiles the screen touches (a tile is 512 pixels across), and
for each of them

* the number of items matching the filter (`matched`);
* a count grid: the matching items in each cell at depth `grid_depth(zoom)`, which is 256 by 256
  cells per tile (2 pixels a cell) down to zoom 8 and coarser below it, since Mosaica's grid stops
  at depth 16 (see README, "Departures from the design");
* every matching item's position where the tile holds at most `K` of them (a sparse tile). A
  system may also send up to `K` positions from a dense tile; that is counted, not required.

Positions are quantised once, by one rule, onto a 2^32 by 2^32 grid over `EXTENT`: the rule
Mosaica's build applies (`mosaica_spatial::morton::fixed32`), written out below so that the oracle
and every adapter apply it the same way. A tile or a grid cell at depth `d` is the top `d` bits of
each axis. The y axis runs upwards, with cell 0 at the south edge.
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

#: The square every system quantises over, as `(x_min, x_max, y_min, y_max)`: fixed once by
#: `prepare.py` from the full source's bounds and recorded in the committed manifest.
EXTENT = tuple(json.loads((Path(__file__).with_name("tiers-manifest.json")).read_text()).get(
    "extent", (0.0, 1.0, 0.0, 1.0)))
FIXED_SPAN = 4294967296.0

SCREEN = (1920, 1080)
TILE_PX = 512
#: Cells per tile edge are 2^GRID_OFFSET where depth allows.
GRID_OFFSET = 8
MAX_DEPTH = 16
#: The most positions a sparse tile has, and the most any tile may send.
K = 500
#: The work types of the PaperSeek vocabulary, the values a `type` filter may name.
TYPES = ("article", "book", "book-chapter", "dataset", "dissertation", "editorial", "erratum",
         "grant", "letter", "libguides", "other", "paratext", "peer-review", "preprint",
         "reference-entry", "report", "retraction", "review", "standard", "supplementary-materials")
#: The licence terms, which are the access terms: every row carries exactly one.
LICENCES = ("unlicensed", "cc-by", "cc-by-nc-nd", "cc-by-nc", "other-oa", "cc-by-sa", "cc-by-nc-sa",
            "public-domain", "cc-by-nd", "publisher-specific-oa", "mit")
#: The `publication_year` histogram of the aggregation workload: one-year bins over
#: `[YEAR_BINS[0], YEAR_BINS[1])`. A year outside it is counted as `rest`, no year as `none`.
YEAR_BINS = (1900, 2030)
#: The most types the `type` table lists, which is every one.
TYPE_TOP = 20


def fixed32(v: float, lo: float = EXTENT[0], hi: float = EXTENT[1]) -> int:
    scaled = (v - lo) / (hi - lo) * FIXED_SPAN
    floored = math.floor(scaled)
    return 0 if floored <= 0 else min(floored, 2**32 - 1)


def fixed32_array(v: np.ndarray, lo: float = EXTENT[0], hi: float = EXTENT[1]) -> np.ndarray:
    scaled = np.floor((v.astype(np.float64) - lo) / (hi - lo) * FIXED_SPAN)
    return np.clip(scaled, 0, 2**32 - 1).astype(np.uint32)


def tile_of(q: np.ndarray, depth: int) -> np.ndarray:
    """The depth-`depth` cell of each 32-bit position, as uint64 (a shift by 32 is defined here)."""
    return q.astype(np.uint64) >> np.uint64(32 - depth)


def grid_depth(zoom: int) -> int:
    return min(zoom + GRID_OFFSET, MAX_DEPTH)


def spread16(v: int) -> int:
    out = 0
    for i in range(32):
        out |= ((v >> i) & 1) << (2 * i)
    return out


def interleave(x: int, y: int) -> int:
    """x on the even bits, y on the odd: Mosaica's tile prefix and cell code."""
    return spread16(x) | (spread16(y) << 1)


def compact(code: np.ndarray) -> np.ndarray:
    """The even bits of each u64, gathered: the inverse of `interleave` on one axis."""
    x = code.astype(np.uint64) & np.uint64(0x5555555555555555)
    for shift, mask in ((1, 0x3333333333333333), (2, 0x0F0F0F0F0F0F0F0F), (4, 0x00FF00FF00FF00FF),
                        (8, 0x0000FFFF0000FFFF), (16, 0x00000000FFFFFFFF)):
        x = (x | (x >> np.uint64(shift))) & np.uint64(mask)
    return x


def deinterleave(code: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    code = code.astype(np.uint64)
    return compact(code), compact(code >> np.uint64(1))


def points_by_tile(qx: np.ndarray, qy: np.ndarray, depth: int) -> dict[tuple[int, int], np.ndarray]:
    """Positions grouped by their depth-`depth` tile, each as an (n, 2) uint32 array of (qx, qy)."""
    qx, qy = qx.astype(np.uint32), qy.astype(np.uint32)
    keys = tile_of(qx, depth) << np.uint64(32) | tile_of(qy, depth)
    out = {}
    for key in np.unique(keys):
        sel = keys == key
        out[(int(key >> np.uint64(32)), int(key & np.uint64(0xFFFFFFFF)))] = np.stack([qx[sel], qy[sel]], axis=1)
    return out


@dataclass(frozen=True)
class Camera:
    """One screen: its centre in frame coordinates and its integer zoom."""

    zoom: int
    cx: float
    cy: float
    label: str = ""

    def bbox(self) -> tuple[float, float, float, float]:
        tile = (EXTENT[1] - EXTENT[0]) / (1 << self.zoom)
        hw = SCREEN[0] / 2 / TILE_PX * tile
        hh = SCREEN[1] / 2 / TILE_PX * tile
        return (self.cx - hw, self.cy - hh, self.cx + hw, self.cy + hh)

    def tile_range(self) -> tuple[int, int, int, int]:
        """Inclusive depth-`zoom` tile coordinates the screen touches, clipped to the extent."""
        x0, y0, x1, y1 = self.bbox()
        shift = 32 - self.zoom
        tx0, tx1 = fixed32(x0) >> shift, fixed32(x1) >> shift
        ty0 = fixed32(y0, EXTENT[2], EXTENT[3]) >> shift
        ty1 = fixed32(y1, EXTENT[2], EXTENT[3]) >> shift
        return tx0, tx1, ty0, ty1

    def tiles(self) -> list[tuple[int, int]]:
        tx0, tx1, ty0, ty1 = self.tile_range()
        return [(tx, ty) for ty in range(ty0, ty1 + 1) for tx in range(tx0, tx1 + 1)]


def is_year(v) -> bool:
    return type(v) is int and -(2**31) <= v < 2**31


@dataclass
class Filter:
    """A conjunction of leaves, each `{"col", "op", "value"}`: `eq` on `type` (a value of `TYPES`)
    or on `is_oa` (a bool), `range` on `publication_year` (`{"gte": y}`, `{"lt": y}` or both, a
    null year matching neither), or `bbox` (`col` absent) with `[x0, y0, x1, y1]` closed on every
    side after quantisation."""

    leaves: list[dict] = field(default_factory=list)
    label: str = "none"

    def __post_init__(self) -> None:
        """Refuse a leaf outside that grammar, since adapters write its values into SQL."""
        for leaf in self.leaves:
            op, col, value = leaf.get("op"), leaf.get("col"), leaf.get("value")
            if op == "eq" and col == "type" and value in TYPES:
                continue
            if op == "eq" and col == "is_oa" and type(value) is bool:
                continue
            if (op == "range" and col == "publication_year" and isinstance(value, dict) and value
                    and set(value) <= {"gte", "lt"} and all(is_year(v) for v in value.values())):
                continue
            if (op == "bbox" and col is None and isinstance(value, list) and len(value) == 4
                    and all(type(v) in (int, float) and math.isfinite(v) for v in value)):
                continue
            raise ValueError(f"filter leaf {leaf!r} is not one this harness takes; write type eq "
                             f"one of TYPES, is_oa eq a bool, publication_year range with gte "
                             f"and lt integers, or bbox four numbers")


@dataclass
class Frame:
    """What a system returned for one camera and filter, decoded to one shape."""

    #: (tx, ty) -> matched
    tiles: dict[tuple[int, int], int]
    #: (gx, gy) at grid depth -> count
    grid: dict[tuple[int, int], int]
    #: (tx, ty) -> positions sent, as an (n, 2) uint32 array of (qx, qy)
    points: dict[tuple[int, int], np.ndarray]
    bytes: int = 0
