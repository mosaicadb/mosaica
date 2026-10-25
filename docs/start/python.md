# Your first map in Python

The Python tutorial is a notebook in the repository, `clients/py/examples/01_getting_started.ipynb`, with a marimo version beside it, `01_getting_started.py`. It takes 150,000 GeoNames places from a pandas data frame to an interactive map, then filters, searches and counts them from Python. On the way it explains declaring, inserting and committing, and what rendering and indexing a column cost. It takes about ten minutes.

## Run it

You need Python 3.10 or later and a clone of the repository, which holds the notebook and the data.

```bash
git clone https://github.com/mosaicadb/mosaica ~/mosaica
cd ~/mosaica
pip install -e clients/py-native
pip install -e 'clients/py[widget]' pandas jupyterlab
jupyter lab clients/py/examples/01_getting_started.ipynb
```

The first two installs build from source, so they need Rust, a C compiler and Node.js 22.15 or later. [Install and configure](install.md) has the details.

To use marimo instead, install `marimo` in place of `jupyterlab` and run `marimo edit clients/py/examples/01_getting_started.py`.

## Next

- The [Python client reference](../reference/python/index.md) covers every call, from `create()` to the notebook map.
- [Install and configure](install.md) explains the two files the client writes, and how to serve its database without Python.
