"""GeoNames — 13,463,857 features, the ladder's first geographic rung.

The row work is `mosaica.datasets.geonames.prepare`, the one `mosaica demo geonames` runs: one
DuckDB pass that writes the points file, eight vocabularies, two member files and the admin layer's
names, and copies the declaration beside them. This script runs it over the staged vintage with
its working database kept, then surveys the source from that database: the rows beyond Web
Mercator's domain, the admin chain's holes, each numeric's range and each column's nulls. It also
writes the demo's candidate terms and a deployment file.

**Nothing here is projected.** `points.parquet` carries `lon` and `lat` exactly as GeoNames
publishes them, and the declaration says `projection = "web_mercator"` with its frame written as a
longitude/latitude box, so the build projects them. `frame.json` records the source's own degree
bounds, which the declared box has to contain.
"""

from __future__ import annotations

import argparse
import json
import time

import duckdb
from mosaica.datasets import geonames as dataset

from ..common import projection
from ..common.paths import ladder, staged

RUNG = "geonames"
NOT_RECORDED = dataset.NOT_RECORDED
VINTAGE = "2026-08-27"


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--vintage", default=VINTAGE)
    ap.add_argument("--limit", type=int, default=None, help="keep the first N by entity id")
    args = ap.parse_args()

    src, out = staged(RUNG, args.vintage), ladder(RUNG)
    print(f"source  {src}\noutput  {out}\ncoordinates  WGS84 degrees, unprojected\n")

    started = time.monotonic()
    dataset.prepare(src, out, args.limit or None, keep=True)
    # The ladder's drivers read every rung's declaration as `corpus.toml`.
    (out / "schema.toml").replace(out / "corpus.toml")
    print(f"prepared in {time.monotonic() - started:.0f}s")
    con = duckdb.connect(str(out / "prepare.duckdb"))
    sizes = {
        path.stem.removeprefix("vocab-"): con.execute(f"SELECT count(*) FROM '{path}'").fetchone()[0]
        for path in sorted(out.glob("vocab-*.parquet"))
    }
    kept = con.execute("SELECT count(*) FROM points").fetchone()[0]
    # **A survey of the source, not something this script acts on.** The build clips a latitude
    # beyond Web Mercator's domain onto the frame's edge and reports the count itself
    # (`projections.md` §7); this is the same population counted upstream, so that the two numbers
    # can be held against each other and so that the rung's account of what the source turned out
    # to be does not depend on a build having been run. Counted over the rows kept rather than over
    # the file, so `--limit` surveys its own corpus.
    clipped = con.execute(
        f"""
        SELECT count(*) FILTER (lat > {projection.MAX_LATITUDE!r}),
               count(*) FILTER (lat < {-projection.MAX_LATITUDE!r})
        FROM points
        """
    ).fetchone()
    named = con.execute(
        f"""SELECT level, count(*), count(*) FILTER (contents[1][1] <> key)
            FROM '{out / "artifacts-admin.parquet"}' GROUP BY level ORDER BY level"""
    ).fetchall()
    print("  admin artifacts, and how many carry a published name rather than their code:")
    for level, total, titled in named:
        print(f"    {('country', 'admin1', 'admin2', 'admin3', 'admin4')[level]:<8} {total:>8,}  named {titled:>8,}")

    # --- the source's own box -----------------------------------------------------------------
    # In degrees, which is the space `points.parquet` is now written in and the space the
    # declaration's `extent` is written in. The frame itself — the aligned square this box snaps
    # to, and what it clamps — is the build's report and is not duplicated here.
    bounds = con.execute(
        "SELECT min(lon), max(lon), min(lat), max(lat) FROM points"
    ).fetchone()
    frame = {
        "coordinates": "wgs84_degrees",
        "declared_projection": "web_mercator",
        "declared_extent": projection.WORLD_BOX_WGS84,
        "beyond_domain": {"north": clipped[0], "south": clipped[1], "total": sum(clipped)},
        "data_bounds": {"lon": [bounds[0], bounds[1]], "lat": [bounds[2], bounds[3]]},
        "points": kept,
        "source": str(src),
        "vintage": args.vintage,
    }
    # --- report -------------------------------------------------------------------------------
    print(f"\npoints        {kept:,}")
    print("coordinates   WGS84 degrees — the build projects and quantises them")
    print(f"data bounds   lon [{bounds[0]:.6f}, {bounds[1]:.6f}]  lat [{bounds[2]:.6f}, {bounds[3]:.6f}]")
    print(
        f"beyond domain {sum(clipped):,} past ±{projection.MAX_LATITUDE:.4f}° "
        f"({clipped[0]:,} north, {clipped[1]:,} south) — the build clips and counts these"
    )
    print("\nvocabularies")
    for name, size in sizes.items():
        print(f"  {name:<14} {size:>9,}")

    # Every row is placed as deep as its own codes go, so nothing is dropped — but a key with an
    # empty component is a key whose containment edge above it does not exist, and that is a
    # property of the corpus worth stating rather than discovering. Reported, never refused.
    gaps = con.execute(
        """
        SELECT
            count(*) FILTER (admin1_code IS NOT NULL AND country IS NULL),
            count(*) FILTER (admin2_code IS NOT NULL AND admin1_code IS NULL),
            count(*) FILTER (admin3_code IS NOT NULL
                             AND (admin1_code IS NULL OR admin2_code IS NULL)),
            count(*) FILTER (admin4_code IS NOT NULL
                             AND (admin1_code IS NULL OR admin2_code IS NULL
                                  OR admin3_code IS NULL)),
            count(admin1_code), count(admin2_code), count(admin3_code), count(admin4_code)
        FROM points
        """
    ).fetchone()
    print(f"\nadmin chain — levels filled with a {NOT_RECORDED!r} artifact to keep the tree total")
    for i, level in enumerate(("admin1", "admin2", "admin3", "admin4")):
        filled, carried = gaps[i], gaps[i + 4]
        pct = filled * 100.0 / carried if carried else 0.0
        print(f"  {level:<8} {filled:>9,} of {carried:>10,} ({pct:5.1f}%) sit below a filled level")
    placeholders = con.execute(
        """
        SELECT
            count(DISTINCT country_artifact) FILTER (country IS NULL),
            count(DISTINCT admin1) FILTER (admin1_code IS NULL),
            count(DISTINCT admin2) FILTER (admin2_code IS NULL),
            count(DISTINCT admin3) FILTER (admin3_code IS NULL),
            count(DISTINCT admin4) FILTER (admin4_code IS NULL)
        FROM points
        """
    ).fetchone()
    print(f"  placeholder artifacts created: {dict(zip(('country','admin1','admin2','admin3','admin4'), placeholders))}")

    frame["admin_chain_filled"] = {
        level: {"below_a_filled_level": gaps[i], "carrying": gaps[i + 4]}
        for i, level in enumerate(("admin1", "admin2", "admin3", "admin4"))
    }
    (out / "frame.json").write_text(json.dumps(frame, indent=2) + "\n")

    # **Every numeric's measured range, printed whether or not anything is wrong.** A width is
    # declared from these, and a declaration written from a maximum alone is how `population` came
    # to be `u64` against a column holding -12. The build refuses such a value rather than
    # truncating it, which is right and is also a slow way to find out.
    print("\nnumeric ranges — the widths in the declaration are written from these")
    for column in ("population", "elevation", "dem", "entity_id"):
        lo, hi, neg = con.execute(
            f"SELECT min({column}), max({column}), count(*) FILTER ({column} < 0) FROM points"
        ).fetchone()
        print(f"  {column:<14} [{lo}, {hi}]" + (f"  — {neg:,} negative" if neg else ""))

    print("\nnull counts")
    nulls = con.execute(
        """
        SELECT
            count(*) - count(feature_class), count(*) - count(feature_code),
            count(*) - count(country), count(*) - count(admin1), count(*) - count(admin2),
            count(*) - count(admin3), count(*) - count(admin4), count(*) - count(timezone),
            count(*) - count(elevation), count(*) - count(dem), count(*) - count(name)
        FROM points
        """
    ).fetchone()
    for name, n in zip(
        "feature_class feature_code country admin1 admin2 admin3 admin4 timezone "
        "elevation dem name".split(),
        nulls,
    ):
        print(f"  {name:<14} {n:>12,}  ({n * 100.0 / kept:5.1f}%)")

    for path in sorted(out.glob("*.parquet")):
        print(f"  {path.name:<28} {path.stat().st_size / 1e6:>9.1f} MB")

    # **The demo's candidate terms, ranked by coverage.** `run_demo.sh --ranks` composes the
    # sparse/medium/heavy coverage principals from this: at a 253-term dictionary any single
    # country is a sliver of 13.5M points, so switching principal between two of them demonstrates
    # nothing. The shape is the one `scripts/rank_terms.py` emits for the arXiv fixtures.
    ranks = con.execute(
        """
        SELECT country AS term, count(*) AS pairs
        FROM points WHERE country IS NOT NULL
        GROUP BY country ORDER BY pairs DESC
        """
    ).fetchall()
    (out / "country-ranks.json").write_text(
        json.dumps([{"term": t, "pairs": n} for t, n in ranks], indent=None) + "\n"
    )
    (out / "country-terms.txt").write_text(",".join(t for t, _ in ranks) + "\n")
    print(f"\nwrote country-ranks.json and country-terms.txt ({len(ranks)} terms)")
    print(f"  top five: {', '.join(f'{t} {n:,}' for t, n in ranks[:5])}")

    # The deployment file is generated rather than committed — every value in it is a path or a
    # port on this machine, which is the split `configuration.md` §3 draws.
    (out / "mosaica.toml").write_text(
        """# Generated by `corpora/geonames/prepare.py`. Machine-specific by construction: paths
# and ports, and the *names* of the variables carrying the credentials, never the values.

[bundle]
path  = "bundle"
cache = ".mosaica/cache"
wal   = ".mosaica/wal.log"

[build]
schema = "corpus.toml"

[disclosure]
token_max_lifetime = 3600

[catalogue]
dir = ".mosaica/catalogue"

[serve]
viewer  = "127.0.0.1:8081"
session = "127.0.0.1:8082"
control = "127.0.0.1:8083"
max_k   = 200
operator_credential_env = "MOSAICA_GEONAMES_OPERATOR_CRED"
# Development only — the origins the demo viewer is served from, so a browser on this machine
# can talk to this process (client-interaction §7). The list is enumerated: a wildcard is refused.
dev_cors_origins        = ["http://localhost:5173", "http://127.0.0.1:5173"]
"""
    )

    print(f"\nwrote {out / 'corpus.toml'} and {out / 'mosaica.toml'}")
    print(f"\nnext:\n  cd {out}\n  mosaica check\n  mosaica build")

    con.close()


if __name__ == "__main__":
    main()
