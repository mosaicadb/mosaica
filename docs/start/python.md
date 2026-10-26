# Your first map in Python

The Python tutorial is available as a Jupyter or marimo notebook in the repository. It takes 150,000 GeoNames places from a pandas data frame to an interactive map, then filters, searches and counts them from Python. On the way it explains declaring, inserting and committing, and what rendering and indexing a column cost. It takes about ten minutes.

## Run it

You need Python 3.12 or later on Linux or macOS, and a clone of the repository, which holds the notebook and the data. If you have already cloned it to `~/mosaica`, skip the `git clone`. The commands make a virtual environment in the clone and install the client into it, with the `mosaica` program the notebook runs its database with.

=== "Jupyter"

    ```bash
    git clone https://github.com/mosaicadb/mosaica ~/mosaica
    cd ~/mosaica
    python3 -m venv .venv && . .venv/bin/activate
    pip install 'mosaica[widget]' pandas jupyterlab
    jupyter lab clients/py/examples/01_getting_started.ipynb
    ```

=== "marimo"

    ```bash
    git clone https://github.com/mosaicadb/mosaica ~/mosaica
    cd ~/mosaica
    python3 -m venv .venv && . .venv/bin/activate
    pip install 'mosaica[widget]' pandas marimo
    marimo edit clients/py/examples/01_getting_started.py
    ```

[Install a client](install.md#install-a-client) says what the packages hold.

## Next

- The [Python client reference](../reference/python/index.md) covers every call, from `create()` to the notebook map.
- [Install and configure](install.md) explains the two files the client writes, and how to serve its database without Python.
