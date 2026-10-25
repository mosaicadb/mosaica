# Your first map in Python

The Python tutorial is available as a Jupyter or marimo notebook in the repository. It takes 150,000 GeoNames places from a pandas data frame to an interactive map, then filters, searches and counts them from Python. On the way it explains declaring, inserting and committing, and what rendering and indexing a column cost. It takes about ten minutes.

## Run it

You need Python 3.10 or later and a clone of the repository, which holds the notebook and the data. The client builds from source, so you also need Rust, a C compiler and Node.js 22.15 or later. [Install and configure](install.md) has the details.

=== "Jupyter"

    ```bash
    git clone https://github.com/mosaicadb/mosaica ~/mosaica
    cd ~/mosaica
    pip install -e clients/py-native
    pip install -e 'clients/py[widget]' pandas jupyterlab
    jupyter lab clients/py/examples/01_getting_started.ipynb
    ```

=== "marimo"

    ```bash
    git clone https://github.com/mosaicadb/mosaica ~/mosaica
    cd ~/mosaica
    pip install -e clients/py-native
    pip install -e 'clients/py[widget]' pandas marimo
    marimo edit clients/py/examples/01_getting_started.py
    ```

## Next

- The [Python client reference](../reference/python/index.md) covers every call, from `create()` to the notebook map.
- [Install and configure](install.md) explains the two files the client writes, and how to serve its database without Python.
