<h1>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="brand/mosaica-lockup-white.svg">
    <img src="brand/mosaica-lockup.svg" alt="Mosaica" width="300">
  </picture>
</h1>

**Interactive maps of billions of points.**

Mosaica is a database for serving interactive maps over billions of spatial data points with granular access control on every point. It takes records with a position, such as locations in geo datasets, specimens in a biodiversity archive or millions of papers laid out by their embeddings, and it serves a map that users can pan, zoom, filter and search, in a browser or a notebook.

![150,000 GeoNames places in the Mosaica explorer, coloured by feature class](brand/mosaica_example.png)

## Why Mosaica

- **Nothing is thrown away.** Tile pipelines drop points to keep tiles small, so a filter can only narrow what survived. Mosaica answers each view from the full data, so a filter down to ten points shows those ten points.
- **Big data, small hardware.** It is built to serve billions of points from a single machine to many viewers at once. It gracefully handles datasets that don't fit in system memory.
- **Live data.** Insert, delete and hide records while it serves, with no rebuild.
- **A map per viewer.** Give records access labels, and each viewer's map, its points and every count, is computed only from what they may see.
- **Use it from anywhere.** Deploy as a server with the bundled explorer, integrate with your existing UI, run directly in a Python notebook, or script it from the command line or the HTTP API.
- **Secure enrichments.** Serve layered enrichments over your data, such as clusters, groupings, and hierarchies, with access control applied to each artifact, or served only to viewers who can see the points it was made from.
- **Powerful filters.** Combine spatial filtering such as tiles or polygons with metadata and text search to build exactly the view you need.
- **Multiple views.** Build multiple views over the same data, across geospatial and abstract spaces, or visualise your data at different times.

## Compared with other databases

![Typical and slowest frame time against the number of records, for Mosaica, ClickHouse, OpenSearch and PostgreSQL](docs/assets/comparison/four-systems.png)

We served one map of GBIF's species records, from Mosaica and compared it to delivering the same functionality from three other databases. Each was optimised for the queries run and applied the same access control requirements as Mosaica. The tests were run on a machine with 24GB RAM. At a billion records, a viewer panning and zooming waited 35 ms for a typical frame and 0.36 s for the slowest with Mosaica, and 234 ms and 6.6 s with ClickHouse, the fastest competitor. [Compared with other databases](docs/system/comparison.md) gives the method, each system's setup and more detailed measurements.

## Try it in a notebook

```bash
git clone https://github.com/mosaicadb/mosaica && cd mosaica
python3 -m venv .venv && . .venv/bin/activate
pip install 'mosaica[widget]' pandas jupyterlab
jupyter lab
```

This demonstrates Mosaica on a small 150,000 GeoNames places dataset the repository bundles:

```python
import mosaica
import pandas as pd

places = pd.read_parquet("examples/data/geonames/points.parquet", dtype_backend="numpy_nullable")

db = mosaica.create()
db.declare_view("world", projection="web_mercator")
db.declare_columns(places, category=["feature_class"], render=["feature_class"], index=["name"])
db.insert("world", places, lon="lon", lat="lat")
db.commit()
db.map(colour_by="feature_class")
```

[Your first map in Python](docs/start/python.md) carries on from here, [your first map from files](docs/start/first-map.md) serves the same map to a browser, and [Run Mosaica with Docker](docs/guides/docker.md) puts it on a server.

## Install

| For | Install |
|---|---|
| Python 3.12+ (brings the server on Linux and macOS) | `pip install mosaica` |
| A web page | `npm install @mosaicajs/components` |
| A server | `docker pull ghcr.io/mosaicadb/mosaica:latest` |
| The command line | a [release archive](https://github.com/mosaicadb/mosaica/releases/latest), or `cargo install --locked mosaica` |

[Install and configure](docs/start/install.md) covers platforms and setting up a database.

## When to use something else

- Your data fits in the browser: draw it there, with deck.gl or a plotting library.
- It never changes and has no access rules: static tiles such as PMTiles are cheaper to host.
- You need ad hoc SQL: use a columnar database such as ClickHouse.

Mosaica runs on one machine, with no replication yet, and search filters rather than ranks.

## Status

Mosaica is an alpha. Expect formats and APIs to change before 1.0.

## Learn more

- [Documentation](docs/index.md): tutorials, [how it works](docs/system/overview.md), [guides](docs/guides/index.md) and the [reference](docs/reference/index.md).
- The same documentation as a [website](https://mosaicadb.github.io/mosaica/), with search.
- Examples for the [browser](clients/ts/examples) and [Python](clients/py/examples).
- [Contributing](CONTRIBUTING.md) and the [changelog](CHANGELOG.md).

Apache License 2.0. See [LICENSE](LICENSE).
