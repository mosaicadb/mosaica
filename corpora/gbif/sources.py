"""Where rung 6's one staged source is, and the constants the pipeline shares.

**GBIF occurrence, vintage 2026-06-01.** 8,369 parts under `occurrence.parquet/`, named `NNNNNN`
with no suffix, **one row group apiece** and between 26,717 and 1,039,405 rows each —
3,654,488,638 rows, 258.5 GiB compressed over 50 columns.

A part is the unit of work because it is one row group: a reader takes it whole or not at all.
Parts are read in name order, so an entity id is a function of the part index and of the rows the
run kept, and a prefix of the parts is a prefix of the entity space.

Rung 5 joins TreeOfLife against these same parts and reads them through `parts()` below, so the
directory listing that decides part order is written once.
"""

from __future__ import annotations

from pathlib import Path

from ..common.paths import ladder, staged

RUNG = "gbif"

DATASET, VINTAGE = "gbif", "2026-06-01"

#: Measured at the census, and asserted against the footers at every whole-corpus run.
PART_COUNT = 8_369
TOTAL_ROWS = 3_654_488_638

def parts() -> list[Path]:
    """The 8,369 parts, in name order. Refuses an empty directory rather than reading nothing."""
    base = staged(DATASET, VINTAGE) / "occurrence.parquet"
    found = sorted(p for p in base.iterdir() if p.is_file())
    assert found, f"no parts under {base}"
    return found


def rung() -> Path:
    return ladder(RUNG)
