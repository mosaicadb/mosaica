"""The data sets the harness runs over, one at a time, chosen by `BENCH_DATASET` (`paperseek` when
it is unset).

A data set names the tier files' columns by the part each plays: one category, which the category
filters and the first aggregation table use; one integer year, which the range filters and the
histogram use; at most one boolean flag; and the access column, the item's access terms. Every
module that writes a query reads these names from here, and each adapter keeps its own schema for
each data set. The committed manifest holds the extent and each tier's checksum, and the viewer
file the one viewer's grant.
"""

from __future__ import annotations

import os
from dataclasses import dataclass


@dataclass(frozen=True)
class Dataset:
    #: The prefix of every tier file and camera file: `<name>-<tier>.parquet`.
    name: str
    #: The committed manifest, beside this file: the extent and each tier's checksum.
    manifest: str
    #: The viewer's principal name and terms, beside this file.
    viewer: str
    category: str
    #: Every value the category may hold, which a category filter may name.
    categories: tuple[str, ...]
    year: str
    #: The year histogram's one-year bins over `[lo, hi)`; a year outside is `rest`, none `none`.
    year_bins: tuple[int, int]
    flag: str | None
    access: str
    #: Whether the access column is a list of terms, of which a viewer must hold any one, or a
    #: single term, which the viewer must hold.
    access_is_list: bool
    #: Whether the oracle answers from its own copy of the viewer's rows (`oracle.Oracle`), since
    #: a scan of the tier for every query is too slow at this size.
    oracle_copy: bool = False


PAPERSEEK = Dataset(
    name="paperseek", manifest="tiers-manifest.json", viewer="viewer.json",
    category="type",
    categories=("article", "book", "book-chapter", "dataset", "dissertation", "editorial",
                "erratum", "grant", "letter", "libguides", "other", "paratext", "peer-review",
                "preprint", "reference-entry", "report", "retraction", "review", "standard",
                "supplementary-materials"),
    year="publication_year", year_bins=(1900, 2030), flag="is_oa",
    access="licence", access_is_list=False,
)

#: GBIF's access is `label` (each row's one country term) unless `BENCH_GBIF_ACCESS` is `list`,
#: which uses `access` (country, year and species terms, any one of which admits a viewer).
GBIF_ACCESS = os.environ.get("BENCH_GBIF_ACCESS", "label")
if GBIF_ACCESS not in ("label", "list"):
    raise SystemExit(f"BENCH_GBIF_ACCESS is {GBIF_ACCESS!r}; write label or list")
GBIF = Dataset(
    name="gbif", manifest="tiers-manifest-gbif.json",
    viewer="viewer-gbif.json" if GBIF_ACCESS == "list" else "viewer-gbif-label.json",
    category="kingdom",
    categories=("Animalia", "Archaea", "Bacteria", "Chromista", "Fungi", "Plantae", "Protozoa",
                "Viruses", "incertae sedis"),
    year="year", year_bins=(1900, 2030), flag=None,
    access="access" if GBIF_ACCESS == "list" else "label", access_is_list=GBIF_ACCESS == "list",
    oracle_copy=True,
)

DATASETS = {d.name: d for d in (PAPERSEEK, GBIF)}
if os.environ.get("BENCH_DATASET", "paperseek") not in DATASETS:
    raise SystemExit(f"BENCH_DATASET is {os.environ['BENCH_DATASET']!r}; write one of "
                     f"{', '.join(DATASETS)}")
DATASET = DATASETS[os.environ.get("BENCH_DATASET", "paperseek")]
