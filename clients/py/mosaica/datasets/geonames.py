"""GeoNames: every feature in the `allCountries` dump, about 13.5 million, on a world map.

`fetch` downloads five files from download.geonames.org. GeoNames rewrites them daily, so the
lock records what was fetched rather than pinning it. `prepare` makes one DuckDB pass that writes
the points, eight vocabularies, two member files and the admin layer's names, then copies the
declaration, `geonames.toml`, beside them as `schema.toml`. The coordinates stay in degrees as
GeoNames publishes them; the build projects them.

Four rules the source forces:

- An admin code is qualified by its country, `GB.ENG` and not `ENG`, as GeoNames' own code tables
  write it. `admin1` alone has 823 distinct codes for 4,823 regions.
- A feature code is keyed by its `class.code` pair, which keeps the taxonomy a tree.
- An absent value is null, never a key: the empty string is refused as a category key. A level of
  the admin chain with a level below it filled gets the key `-`, so the tree has every edge.
- `dem`'s -9999 is GeoNames' unknown, and becomes null.
"""

from __future__ import annotations

import shutil
import zipfile
from pathlib import Path
from typing import Optional

BASE = "https://download.geonames.org/export/dump/"

FILES = {
    name: BASE + name
    for name in (
        "allCountries.zip",
        "countryInfo.txt",
        "admin1CodesASCII.txt",
        "admin2Codes.txt",
        "featureCodes_en.txt",
    )
}

DECLARATION = Path(__file__).with_name("geonames.toml")

#: The key standing for a level of the admin chain GeoNames recorded no code at.
NOT_RECORDED = "-"

#: The nine feature classes, from GeoNames' readme, which publishes no file of them.
FEATURE_CLASSES = {
    "A": "Country, state, region",
    "H": "Stream, lake, other water body",
    "L": "Park, area, other region",
    "P": "City, village, other populated place",
    "R": "Road, railroad",
    "S": "Spot, building, farm",
    "T": "Mountain, hill, rock",
    "U": "Undersea feature",
    "V": "Forest, heath, other vegetation",
}

COUNTRY_INFO_COLUMNS = (
    "iso iso3 iso_numeric fips country_name capital area population continent tld "
    "currency_code currency_name phone postal_format postal_regex languages geonameid "
    "neighbours equivalent_fips"
).split()

ADMIN_CODE_COLUMNS = ["key", "title", "ascii_title", "geonameid"]

FEATURE_CODE_COLUMNS = ["key", "title", "description"]

#: The 19 columns of `allCountries.txt`, in order, with the types they are read at.
COLUMNS = {
    "geonameid": "BIGINT",
    "name": "VARCHAR",
    "asciiname": "VARCHAR",
    "alternatenames": "VARCHAR",
    "latitude": "DOUBLE",
    "longitude": "DOUBLE",
    "feature_class": "VARCHAR",
    "feature_code": "VARCHAR",
    "country_code": "VARCHAR",
    "cc2": "VARCHAR",
    "admin1_code": "VARCHAR",
    "admin2_code": "VARCHAR",
    "admin3_code": "VARCHAR",
    "admin4_code": "VARCHAR",
    "population": "BIGINT",
    "elevation": "INTEGER",
    "dem": "INTEGER",
    "timezone": "VARCHAR",
    "modification_date": "DATE",
}


def fetch(raw: Path, progress: bool = True, sample: Optional[int] = None) -> list[Path]:
    from . import _steps

    return _steps.fetch(raw, FILES, progress)


def _lookup(con, table: str, path: Path, columns: list[str]) -> None:
    """Read one of the small tab-separated lookup files into a table. Python rather than
    `read_csv`, because `countryInfo.txt`'s commented header defeats DuckDB's dialect sniffer."""
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        fields = line.split("\t")
        fields += [""] * (len(columns) - len(fields))
        rows.append(tuple(f.strip() or None for f in fields[: len(columns)]))
    con.execute(f"CREATE OR REPLACE TABLE {table} ({', '.join(f'{c} VARCHAR' for c in columns)})")
    if rows:
        con.executemany(f"INSERT INTO {table} VALUES ({', '.join('?' * len(columns))})", rows)


def prepare(raw: Path, out: Path, sample: Optional[int] = None, keep: bool = False) -> None:
    """Write the files `geonames.toml` names into `out`, from the files `fetch` put in `raw`.

    - `sample`: keep the first `sample` features in the order of their modification date.
    - `keep`: leave `allCountries.txt` and the working database `prepare.duckdb` in `out`, whose
      tables `raw` and `points` the ladder's survey reads.
    """
    import duckdb

    all_countries = out / "allCountries.txt"
    if not all_countries.exists():
        with zipfile.ZipFile(raw / "allCountries.zip") as archive:
            archive.extract("allCountries.txt", out)
    con = duckdb.connect(str(out / "prepare.duckdb"))
    con.execute("PRAGMA enable_progress_bar=false")
    for table, filename, columns in (
        ("country_info", "countryInfo.txt", COUNTRY_INFO_COLUMNS),
        ("admin1_titles", "admin1CodesASCII.txt", ADMIN_CODE_COLUMNS),
        ("admin2_titles", "admin2Codes.txt", ADMIN_CODE_COLUMNS),
        ("feature_code_titles", "featureCodes_en.txt", FEATURE_CODE_COLUMNS),
    ):
        _lookup(con, table, raw / filename, columns)

    columns = ", ".join(f"'{name}': '{ty}'" for name, ty in COLUMNS.items())
    con.execute(
        f"""
        CREATE OR REPLACE TABLE raw AS
        SELECT * FROM read_csv(
            '{all_countries}',
            delim = '\t', header = false, quote = '', escape = '',
            nullstr = '', columns = {{{columns}}}
        )
        """
    )
    collisions = con.execute(
        f"""
        SELECT count(*) FROM raw WHERE '{NOT_RECORDED}' IN
            (country_code, admin1_code, admin2_code, admin3_code, admin4_code)
        """
    ).fetchone()[0]
    if collisions:
        raise ValueError(
            f"{collisions:,} GeoNames rows carry {NOT_RECORDED!r} as an administrative code, the "
            f"key this recipe gives a level with no code recorded; change NOT_RECORDED"
        )
    # A level of the admin chain is keyed only where a level below it is filled, so a chain that
    # stops is not a hole, and a hole is an artifact named `-` rather than a missing edge.
    con.execute(
        f"""
        CREATE OR REPLACE TABLE points AS
        WITH marked AS (
            SELECT *,
                   coalesce(country_code, '{NOT_RECORDED}') AS m0,
                   coalesce(admin1_code,  '{NOT_RECORDED}') AS m1,
                   coalesce(admin2_code,  '{NOT_RECORDED}') AS m2,
                   coalesce(admin3_code,  '{NOT_RECORDED}') AS m3,
                   coalesce(admin4_code,  '{NOT_RECORDED}') AS m4
            FROM raw
        )
        SELECT
            row_number() OVER (ORDER BY modification_date, geonameid) - 1 AS entity_id,
            geonameid,
            longitude AS lon,
            latitude  AS lat,
            feature_class,
            CASE WHEN feature_class IS NOT NULL AND feature_code IS NOT NULL
                 THEN feature_class || '.' || feature_code END AS feature_code,
            CASE WHEN country_code IS NOT NULL OR admin1_code IS NOT NULL
                      OR admin2_code IS NOT NULL OR admin3_code IS NOT NULL
                      OR admin4_code IS NOT NULL THEN m0 END AS country_artifact,
            country_code AS country,
            CASE WHEN admin1_code IS NOT NULL OR admin2_code IS NOT NULL
                      OR admin3_code IS NOT NULL OR admin4_code IS NOT NULL
                 THEN concat_ws('.', m0, m1) END AS admin1,
            CASE WHEN admin2_code IS NOT NULL OR admin3_code IS NOT NULL
                      OR admin4_code IS NOT NULL
                 THEN concat_ws('.', m0, m1, m2) END AS admin2,
            CASE WHEN admin3_code IS NOT NULL OR admin4_code IS NOT NULL
                 THEN concat_ws('.', m0, m1, m2, m3) END AS admin3,
            CASE WHEN admin4_code IS NOT NULL
                 THEN concat_ws('.', m0, m1, m2, m3, m4) END AS admin4,
            timezone,
            population,
            elevation::SMALLINT AS elevation,
            nullif(dem, -9999)::SMALLINT AS dem,
            modification_date::TIMESTAMP AS modification_date,
            name,
            admin1_code, admin2_code, admin3_code, admin4_code
        FROM marked
        {f"QUALIFY entity_id < {int(sample)}" if sample is not None else ""}
        """
    )
    con.execute(
        f"""COPY (
            SELECT geonameid, lon, lat, feature_class, feature_code, country,
                   admin1, admin2, admin3, admin4, timezone,
                   population, elevation, dem, modification_date, name
            FROM points ORDER BY entity_id
        ) TO '{out / "points.parquet"}' (FORMAT parquet, COMPRESSION zstd)"""
    )

    # Codes are dense from 1, since code 0 stands for an absent value.
    def titled(column: str, titles: str) -> str:
        return f"""
            SELECT key, row_number() OVER (ORDER BY key) AS code, title FROM (
                SELECT p.{column} AS key, coalesce(any_value(t.title), p.{column}) AS title
                FROM points p LEFT JOIN {titles} t ON t.key = p.{column}
                WHERE p.{column} IS NOT NULL
                GROUP BY p.{column}
            )"""

    # GeoNames publishes no names for admin3 and admin4 codes, but each division is a feature row
    # of its own, `A.ADM3` or `A.ADM4`, which names it. A code with no such row keeps its code.
    def self_named(column: str, code: str) -> str:
        return f"""
            SELECT key, row_number() OVER (ORDER BY key) AS code, title FROM (
                SELECT p.{column} AS key, coalesce(any_value(f.name), p.{column}) AS title
                FROM points p
                LEFT JOIN points f ON f.{column} = p.{column} AND f.feature_code = '{code}'
                WHERE p.{column} IS NOT NULL
                GROUP BY p.{column}
            )"""

    def quoted(text: str) -> str:
        return "'" + text.replace("'", "''") + "'"

    vocabularies = {
        "feature-class": f"""
            SELECT key, row_number() OVER (ORDER BY key) AS code, title FROM (
                SELECT unnest([{", ".join(quoted(k) for k in FEATURE_CLASSES)}]) AS key,
                       unnest([{", ".join(quoted(t) for t in FEATURE_CLASSES.values())}]) AS title
            )""",
        "feature-code": """
            SELECT key, row_number() OVER (ORDER BY key) AS code, title FROM (
                SELECT key, title FROM feature_code_titles WHERE key <> 'null'
            )""",
        "country": """
            SELECT key, row_number() OVER (ORDER BY key) AS code, title FROM (
                SELECT p.country AS key, coalesce(any_value(c.country_name), p.country) AS title
                FROM points p LEFT JOIN country_info c ON c.iso = p.country
                WHERE p.country IS NOT NULL
                GROUP BY p.country
            )""",
        "admin1": titled("admin1", "admin1_titles"),
        "admin2": titled("admin2", "admin2_titles"),
        "admin3": self_named("admin3", "A.ADM3"),
        "admin4": self_named("admin4", "A.ADM4"),
        "timezone": """
            SELECT timezone AS key, row_number() OVER (ORDER BY timezone) AS code,
                   timezone AS title
            FROM (SELECT DISTINCT timezone FROM points WHERE timezone IS NOT NULL)""",
    }
    for name, query in vocabularies.items():
        con.execute(f"COPY ({query}) TO '{out / f'vocab-{name}.parquet'}' (FORMAT parquet)")

    # Every artifact of the admin layer carries a name, its published one where there is one and
    # its key otherwise, since the layer declares `name` as supplied content.
    levels = ("country", "admin1", "admin2", "admin3", "admin4")
    keys = ("country_artifact", "admin1", "admin2", "admin3", "admin4")
    union = "\nUNION ALL ".join(
        f"SELECT {level} AS level, k.key, v.title FROM "
        f"(SELECT DISTINCT {key} AS key FROM points) k "
        f"LEFT JOIN '{out / f'vocab-{name}.parquet'}' v USING (key)"
        for level, (key, name) in enumerate(zip(keys, levels))
    )
    con.execute(
        f"""COPY (
            SELECT level::UINTEGER AS level, key, [[coalesce(title, key)]] AS contents
            FROM ({union}) WHERE key IS NOT NULL
            ORDER BY level, key
        ) TO '{out / "artifacts-admin.parquet"}' (FORMAT parquet, COMPRESSION zstd)"""
    )

    # One row per point, `key` a list whose positions are the declared levels; a null entry is in
    # no artifact at that level.
    con.execute(
        f"""COPY (
            SELECT geonameid AS entity, [feature_class, feature_code] AS key
            FROM points ORDER BY entity_id
        ) TO '{out / "members-feature.parquet"}' (FORMAT parquet, COMPRESSION zstd)"""
    )
    con.execute(
        f"""COPY (
            SELECT geonameid AS entity,
                   [country_artifact, admin1, admin2, admin3, admin4] AS key
            FROM points ORDER BY entity_id
        ) TO '{out / "members-admin.parquet"}' (FORMAT parquet, COMPRESSION zstd)"""
    )
    con.close()
    shutil.copy(DECLARATION, out / "schema.toml")
    if not keep:
        all_countries.unlink()
        (out / "prepare.duckdb").unlink()
