"""GBIF: every occurrence in GBIF's monthly snapshot that has a coordinate, about 3.5 billion, on a
world map, with a taxonomy of families, genera and species.

`fetch` reads the snapshot GBIF publishes on Amazon S3 without credentials, the newest when the
dataset is first fetched, and keeps eleven of each part's fifty columns, about 37% of its bytes.
`prepare` writes the points, the taxonomy's member file and the `kingdom` vocabulary, then the
declaration, `gbif.toml`, beside them as `schema.toml`. `sample=N` takes N of the snapshot's parts,
evenly spaced across it; a part holds about 370,000 records.

The rules the rows follow, which `corpora/gbif/prepare.py` reads from here too:

- A record with no coordinate inside ±90° and ±180° is dropped, since the one view has nowhere to
  draw it. A record beyond Web Mercator's ±85.0511° is kept and clamps to the edge of the map.
- The access column is a list of up to three terms: the country code, `y:` and the year, and `s:`
  and the species key. A record with no country carries `UNRECORDED`, so no list is empty and a
  reader holding no term sees nothing. A source value that would merge with one of these markers
  is refused.
- The taxonomy starts at family. A level's key is the ranks down to it joined by `|`, a rank the
  source left empty is `NOT_RECORDED` where a rank below it is recorded, and a chain that ends
  stays null.
- Nothing holds a column whole: parts are read on a thread pool a few ahead of the writer and
  written as row groups of a million rows.
"""

from __future__ import annotations

import concurrent.futures as cf
import itertools
import json
import os
import re
import urllib.parse
import urllib.request
from collections import deque
from pathlib import Path
from typing import Optional

import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.fs as fs
import pyarrow.parquet as pq

BUCKET = "gbif-open-data-us-east-1"
REGION = "us-east-1"
LISTING = f"https://{BUCKET}.s3.{REGION}.amazonaws.com/"

DECLARATION = Path(__file__).with_name("gbif.toml")

#: The three ranks the tiered layer walks. Kingdom is a rendered category instead, because the
#: build holds the largest artifact's members while it sorts them, and Animalia is most of the
#: corpus.
RANKS = ["family", "genus", "species"]

#: The columns read from each part. `locality`, the largest, and `occurrenceid` are left out.
COLUMNS = [
    "kingdom",
    *RANKS,
    "scientificname",
    "specieskey",
    "year",
    "countrycode",
    "decimallatitude",
    "decimallongitude",
    "gbifid",
]

#: The access term of a record with no country.
UNRECORDED = "UNRECORDED"
YEAR_PREFIX = "y:"
SPECIES_PREFIX = "s:"
#: The key of a taxonomic rank the source left empty above one it recorded.
NOT_RECORDED = "NOT_RECORDED"
#: Between the ranks of a taxonomy key; a scientific name carries dots, so a dot would split one.
SEPARATOR = "|"
ROW_GROUP = 1_000_000
#: Parts read ahead of the one being written.
READ_AHEAD = 16
#: Web Mercator's northern and southern limit, in degrees.
MAX_LATITUDE = 85.0511287798066


#: What `points.parquet` carries, fixed rather than inferred, because it is written a batch at a
#: time and a batch whose `kingdom` happened to be all null would otherwise change it.
POINTS_SCHEMA = pa.schema(
    [
        pa.field("lon", pa.float64()),
        pa.field("lat", pa.float64()),
        pa.field("countrycode", pa.string()),
        pa.field("kingdom", pa.string()),
        pa.field("specieskey", pa.string()),
        pa.field("year", pa.uint16()),
        pa.field("scientificname", pa.string()),
        pa.field("gbifid", pa.uint64()),
        pa.field("access", pa.list_(pa.string())),
    ]
)

#: The columns of `points.parquet` worth a dictionary page; `access.list.element` is the leaf of
#: the list column.
POINTS_DICTIONARY = ["countrycode", "kingdom", "specieskey", "scientificname", "access.list.element"]

#: The publisher's own identifier, which the ladder's rung carries under `--occurrenceid`.
OCCURRENCE_ID = pa.field("occurrenceid", pa.string())

#: The taxonomy's member file: an occurrence's `gbifid` and its key at each of the three levels.
MEMBER_SCHEMA = pa.schema(
    [pa.field("entity", pa.uint64()), pa.field("key", pa.list_(pa.string()))]
)


# ------------------------------------------------------------------------------------- the rows


def true_count(mask) -> int:
    """How many `True`s, nulls not counted."""

    return int(pc.sum(mask).as_py() or 0)


def read_parts(paths: list[Path], workers: int, columns: list[str] = COLUMNS):
    """`(path, table)` per part, in order, with the next `READ_AHEAD` parts already reading."""

    with cf.ThreadPoolExecutor(max_workers=workers) as pool:
        remaining = iter(paths)
        pending: deque = deque(
            (p, pool.submit(pq.read_table, p, columns=columns))
            for p in itertools.islice(remaining, READ_AHEAD)
        )
        while pending:
            path, future = pending.popleft()
            for nxt in itertools.islice(remaining, 1):
                pending.append((nxt, pool.submit(pq.read_table, nxt, columns=columns)))
            yield path, future.result()


def placed(table, want_unplaced: bool):
    """The part's rows with a coordinate inside ±90/±180, the rows without one (`None` unless
    `want_unplaced`), and how many rows had no finite coordinate and how many had one out of
    range."""

    lat = table.column("decimallatitude").combine_chunks().cast(pa.float64())
    lon = table.column("decimallongitude").combine_chunks().cast(pa.float64())
    finite = pc.fill_null(pc.and_(pc.is_finite(lat), pc.is_finite(lon)), False)
    # Filled, since `filter` drops a null selection and the counts below read the mask.
    inside = pc.fill_null(
        pc.and_(
            finite,
            pc.and_(pc.less_equal(pc.abs(lat), 90.0), pc.less_equal(pc.abs(lon), 180.0)),
        ),
        False,
    )
    kept = true_count(inside)
    finite_rows = true_count(finite)
    return (
        table.filter(inside) if kept else None,
        table.filter(pc.invert(inside)) if want_unplaced and kept < table.num_rows else None,
        table.num_rows - finite_rows,
        finite_rows - kept,
    )


def prefixed(country):
    """Where a country code begins with a year or species prefix, and would be that term."""

    return pc.fill_null(
        pc.or_(pc.starts_with(country, YEAR_PREFIX), pc.starts_with(country, SPECIES_PREFIX)),
        False,
    )


def collides(country):
    """Where a country code would merge with another term: spelled `UNRECORDED`, or `prefixed`."""

    return pc.or_(pc.fill_null(pc.equal(country, UNRECORDED), False), prefixed(country))


def country_terms(column):
    """The country term of each row, trimmed and `UNRECORDED` where blank, with how many rows
    carried no country and how many `collides` refuses."""

    country = pc.fill_null(pc.utf8_trim_whitespace(column.combine_chunks().cast(pa.string())), "")
    collisions = true_count(collides(country))
    blank = pc.equal(country, "")
    return pc.if_else(blank, UNRECORDED, country), true_count(blank), collisions


def access_lists(country, year, species_key):
    """Each row's `[country, "y:"+year, "s:"+species_key]`, a null year or species key
    contributing nothing, assembled by index rather than row by row."""
    import numpy as np

    rows = len(country)
    years = pc.binary_join_element_wise(YEAR_PREFIX, pc.cast(year, pa.string()), "")
    species = pc.binary_join_element_wise(SPECIES_PREFIX, pc.cast(species_key, pa.string()), "")
    has_year = pc.is_valid(years).to_numpy(zero_copy_only=False)
    has_species = pc.is_valid(species).to_numpy(zero_copy_only=False)

    offsets = np.zeros(rows + 1, dtype=np.int32)
    np.cumsum(1 + has_year.astype(np.int32) + has_species, out=offsets[1:])
    at = offsets[:-1]

    # One `take` over country ++ the present years ++ the present species; `index` says where each
    # term lands.
    present_years = int(has_year.sum())
    index = np.empty(int(offsets[-1]), dtype=np.int64)
    index[at] = np.arange(rows)
    index[at[has_year] + 1] = rows + np.arange(present_years)
    index[at[has_species] + 1 + has_year[has_species]] = (
        rows + present_years + np.arange(int(has_species.sum()))
    )
    terms = pa.concat_arrays(
        [country.cast(pa.string()), years.drop_null(), species.drop_null()]
    ).take(pa.array(index))
    return pa.ListArray.from_arrays(pa.array(offsets, pa.int32()), terms)


def with_access(table, schema):
    """`table` with its access column built from its own `countrycode`, `year` and `specieskey`."""

    access = access_lists(
        table.column("countrycode").combine_chunks(),
        table.column("year").combine_chunks(),
        table.column("specieskey").combine_chunks(),
    )
    columns = {name: table.column(name) for name in schema.names if name != "access"}
    return pa.table(columns | {"access": access}, schema=schema)


def point_rows(batch, schema):
    """A batch's `points.parquet` columns, and what was counted while they were made."""

    m = batch.num_rows
    lat = batch.column("decimallatitude").combine_chunks().cast(pa.float64())
    lon = batch.column("decimallongitude").combine_chunks().cast(pa.float64())
    country, blank, collided = country_terms(batch.column("countrycode"))

    # A year outside `u16` is nulled and counted rather than wrapped, which would place it in a
    # range filter it does not belong to.
    year = batch.column("year").combine_chunks().cast(pa.int32())
    sane = pc.fill_null(pc.and_(pc.greater_equal(year, 1), pc.less_equal(year, 65_535)), False)
    year_out_of_range = (m - year.null_count) - true_count(sane)
    year = pc.if_else(sane, year, pa.nulls(m, pa.int32())).cast(pa.uint16())

    scientific = batch.column("scientificname").combine_chunks().cast(pa.string())
    # `gbifid` is a string in the snapshot; one that is not an unsigned integer raises here.
    gbifid = batch.column("gbifid").combine_chunks().cast(pa.string()).cast(pa.uint64())
    columns = {
        "lon": lon,
        "lat": lat,
        "countrycode": country,
        "kingdom": batch.column("kingdom").combine_chunks().cast(pa.string()),
        "specieskey": batch.column("specieskey").combine_chunks().cast(pa.string()),
        "year": year,
        "scientificname": scientific,
        "gbifid": gbifid,
    }
    if "occurrenceid" in schema.names:
        columns["occurrenceid"] = batch.column("occurrenceid").combine_chunks().cast(pa.string())
    columns["access"] = access_lists(country, year, columns["specieskey"])
    counts = {
        "beyond_mercator": true_count(pc.greater(pc.abs(lat), MAX_LATITUDE)),
        "year_out_of_range": year_out_of_range,
        "unrecorded_country": blank,
        "country_collisions": collided,
        "named": m - scientific.null_count,
        "gbifid_null": gbifid.null_count,
    }
    return pa.table(columns, schema=schema), counts


def rank_collisions(batch) -> dict[str, list]:
    """The rank values of a batch that would merge with `NOT_RECORDED` or split at `SEPARATOR`."""

    found = {}
    for rank in RANKS:
        unique = pc.unique(batch.column(rank).combine_chunks().cast(pa.string()))
        bad = pc.fill_null(
            pc.or_(pc.equal(unique, NOT_RECORDED), pc.match_substring(unique, SEPARATOR)), False
        )
        if true_count(bad):
            found[rank] = unique.filter(bad).to_pylist()
    return found


def taxonomy_keys(table):
    """A batch's member-file list column, and the three level key arrays it was built from,
    joined in Arrow over one pooled dictionary of the three levels."""
    import numpy as np

    n = table.num_rows
    ranks = [table.column(r).combine_chunks().cast(pa.string()) for r in RANKS]
    marked = [pc.fill_null(r, NOT_RECORDED) for r in ranks]

    # `present[j]`: is anything at or below level j recorded? Folded from the deepest rank up.
    present: list = [None] * len(ranks)
    below = pc.is_valid(ranks[-1])
    present[-1] = below
    for j in range(len(ranks) - 2, -1, -1):
        below = pc.or_(pc.is_valid(ranks[j]), below)
        present[j] = below

    keys: list = []
    joined = marked[0]
    for j in range(len(ranks)):
        if j:
            joined = pc.binary_join_element_wise(joined, marked[j], SEPARATOR)
        keys.append(pc.if_else(present[j], joined, pa.nulls(n, pa.string())))

    encoded = [pc.dictionary_encode(k) for k in keys]
    pooled = pa.concat_arrays([e.dictionary.cast(pa.string()) for e in encoded])
    offsets = np.cumsum([0] + [len(e.dictionary) for e in encoded])[:-1]
    codes = np.full((n, len(ranks)), -1, dtype=np.int64)
    for j, e in enumerate(encoded):
        got = np.asarray(pc.fill_null(e.indices, -1), dtype=np.int64)
        held = got >= 0
        codes[held, j] = got[held] + offsets[j]
    flat = codes.reshape(-1)
    values = pooled.take(pa.array(np.where(flat >= 0, flat, 0), mask=flat < 0))
    listed = pa.ListArray.from_arrays(
        pa.array(np.arange(0, n * len(ranks) + 1, len(ranks), dtype=np.int32), pa.int32()), values
    )
    return listed, keys


def write_vocabulary(out: Path, name: str, keys: list[str]) -> int:
    """One vocabulary file, its codes from one, since code 0 stands for an absent value."""
    import numpy as np

    pq.write_table(
        pa.table(
            {
                "key": pa.array(keys, pa.string()),
                "code": pa.array(np.arange(1, len(keys) + 1, dtype=np.uint32), pa.uint32()),
                "title": pa.array(keys, pa.string()),
            }
        ),
        out / f"vocab-{name}.parquet",
    )
    return len(keys)


def vocabulary_toml(size: int, rows: int) -> str:
    """The `kingdom` `[[vocabulary]]` block, its width the size of the code space it needs."""
    from ._steps import width

    return (
        f"# Minted by this run from {rows:,} placed occurrences.\n"
        "[[vocabulary]]\n"
        'name       = "kingdom"\n'
        f'title      = "Kingdom"  # {size:,} keys\n'
        f'width      = "{width(size)}"\n'
        'value_set  = "closed"\n'
        'visibility = "public"\n'
        'source     = "kingdom"'
    )


def write_declaration(out: Path, *, size: int, rows: int) -> None:
    """`gbif.toml` beside the data as `schema.toml`, its `# <vocabulary>` line replaced by the
    block."""
    from ._steps import declared

    declared(DECLARATION, "# <vocabulary>", vocabulary_toml(size, rows), out / "schema.toml")


def select_parts(every: list, take: int, spread: bool = True) -> list:
    """`take` of the parts, evenly spaced with `spread` and a prefix without; all of them when
    `take` is 0 or at least their number."""
    import numpy as np

    if take <= 0 or take >= len(every):
        return list(every)
    if not spread:
        return list(every[:take])
    at = (np.arange(take) * (len(every) / take)).astype(np.int64)
    return [every[i] for i in at]


# ------------------------------------------------------------------------------------ the fetch


def _listed(prefix: str, delimiter: Optional[str] = None) -> tuple[list[str], list[str]]:
    """The keys and the common prefixes under `prefix` in the bucket, every page of them."""
    keys: list[str] = []
    prefixes: list[str] = []
    token = None
    while True:
        query = {"list-type": "2", "prefix": prefix}
        if delimiter:
            query["delimiter"] = delimiter
        if token:
            query["continuation-token"] = token
        with urllib.request.urlopen(LISTING + "?" + urllib.parse.urlencode(query)) as answer:
            body = answer.read().decode()
        keys += re.findall(r"<Key>([^<]+)</Key>", body)
        prefixes += re.findall(r"<Prefix>([^<]+)</Prefix>", body)[1:]
        found = re.search(r"<NextContinuationToken>([^<]+)</NextContinuationToken>", body)
        if found is None:
            return keys, prefixes
        token = found.group(1)


def snapshots() -> list[str]:
    """The dates of the snapshots GBIF publishes, oldest first."""
    _, prefixes = _listed("occurrence/", "/")
    return sorted(p.rstrip("/").rsplit("/", 1)[-1] for p in prefixes)


def _index(raw: Path) -> dict:
    """The snapshot this dataset reads and its parts, chosen when it is first fetched."""
    path = raw / "parts.json"
    if path.exists():
        return json.loads(path.read_text(encoding="utf-8"))
    snapshot = snapshots()[-1]
    keys, _ = _listed(f"occurrence/{snapshot}/occurrence.parquet/")
    index = {"snapshot": snapshot, "parts": sorted(k.rsplit("/", 1)[-1] for k in keys)}
    raw.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(index) + "\n", encoding="utf-8")
    return index


def fetch(raw: Path, progress: bool = True, sample: Optional[int] = None) -> list[Path]:
    """Read the columns this dataset uses from the parts `sample` selects, each into
    `raw/parts/`, and record each in `LOCK.json`. A part already recorded is not read again."""
    from tqdm.auto import tqdm

    from .._refusal import Refusal
    from . import _steps

    try:
        index = _index(raw)
    except OSError as why:
        raise Refusal(
            f"fetch: GBIF's snapshot list at {LISTING} could not be read ({why})"
        ) from None
    base = f"occurrence/{index['snapshot']}/"
    citation = _steps.fetch(raw, {"citation.txt": LISTING + base + "citation.txt"}, progress)
    sources = {
        f"parts/{part}.parquet": f"{BUCKET}/{base}occurrence.parquet/{part}"
        for part in select_parts(index["parts"], sample or 0)
    }
    lock = _steps.locked(raw)
    wanted = [
        name for name, source in sources.items()
        if not _steps.held(raw, lock, name, f"s3://{source}")
    ]
    (raw / "parts").mkdir(parents=True, exist_ok=True)
    s3 = fs.S3FileSystem(anonymous=True, region=REGION)

    def one(name: str) -> tuple[str, dict]:
        table = pq.read_table(sources[name], columns=COLUMNS, filesystem=s3)

        def write(part: Path) -> None:
            pq.write_table(table, part, compression="zstd")

        return name, _steps.kept(raw / name, write, f"s3://{sources[name]}")

    recorded: dict = {}
    try:
        with cf.ThreadPoolExecutor(max_workers=16) as pool, tqdm(
            total=len(wanted), unit="part", desc=f"GBIF {index['snapshot']}", disable=not progress
        ) as bar:
            for name, entry in pool.map(one, wanted):
                recorded[name] = entry
                bar.update(1)
                if len(recorded) % 64 == 0:
                    _steps.record(raw, recorded)
    except OSError as why:
        raise Refusal(
            f"fetch: a part of GBIF's {index['snapshot']} snapshot could not be read ({why}); run "
            "again to fetch the rest"
        ) from None
    finally:
        _steps.record(raw, recorded)
    return citation + [raw / name for name in sources]


# ---------------------------------------------------------------------------------- the prepare


def prepare(raw: Path, out: Path, sample: Optional[int] = None) -> None:
    """Write the files `gbif.toml` names into `out`, from the parts `fetch` read for `sample`."""
    index = json.loads((raw / "parts.json").read_text(encoding="utf-8"))
    paths = [raw / "parts" / f"{part}.parquet" for part in select_parts(index["parts"], sample or 0)]
    kingdoms: set[str] = set()
    collided: dict[str, set] = {}
    placed_rows = 0

    points = pq.ParquetWriter(
        out / "points.parquet", POINTS_SCHEMA, compression="zstd", use_dictionary=POINTS_DICTIONARY
    )
    members = pq.ParquetWriter(out / "members-taxonomy.parquet", MEMBER_SCHEMA, compression="zstd")

    def flush(batch) -> None:
        nonlocal placed_rows
        rows, counts = point_rows(batch, POINTS_SCHEMA)
        if counts["country_collisions"]:
            collided.setdefault("countrycode", set()).add(f"{counts['country_collisions']:,} rows")
        for rank, values in rank_collisions(batch).items():
            collided.setdefault(rank, set()).update(values)
        kingdoms.update(k for k in pc.unique(rows.column("kingdom")).to_pylist() if k is not None)
        listed, _ = taxonomy_keys(batch)
        members.write_table(
            pa.table({"entity": rows.column("gbifid"), "key": listed}, schema=MEMBER_SCHEMA),
            row_group_size=ROW_GROUP,
        )
        points.write_table(rows, row_group_size=ROW_GROUP)
        placed_rows += batch.num_rows

    with points, members:
        buffer: list = []
        buffered = 0
        for _path, table in read_parts(paths, os.cpu_count() or 4):
            kept, _, _, _ = placed(table, False)
            if kept is not None:
                buffer.append(kept)
                buffered += kept.num_rows
            if buffered >= ROW_GROUP:
                flush(pa.concat_tables(buffer))
                buffer, buffered = [], 0
        if buffered:
            flush(pa.concat_tables(buffer))
    if collided:
        raise ValueError(
            f"GBIF values collide with this recipe's markers ({NOT_RECORDED!r}, {SEPARATOR!r}, "
            f"{UNRECORDED!r}, {YEAR_PREFIX!r}, {SPECIES_PREFIX!r}): "
            f"{ {k: sorted(v)[:5] for k, v in collided.items()} }; change the markers"
        )
    size = write_vocabulary(out, "kingdom", sorted(kingdoms))
    write_declaration(out, size=size, rows=placed_rows)
