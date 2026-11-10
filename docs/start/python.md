# Python notebooks

To learn Mosaica from Python, work through these two notebooks, each of which comes in a Jupyter and a marimo version. The first starts from nothing and the second builds on it.

You need Python 3.12 or later on Linux or macOS. The commands make a virtual environment in a new folder, install the client into it with the `mosaica` program the notebooks run their databases with, and download the notebook. The notebook downloads its data from the repository the first time it runs. [Install a client](install.md#install-a-client) says what the packages hold.

## Your first map in Python

The first tutorial takes 150,000 GeoNames places from a pandas data frame to an interactive map, then filters, searches and counts them from Python. On the way it explains declaring, inserting and committing, and what rendering and indexing a column cost. It takes about ten minutes.

=== "Jupyter"

    ```bash
    mkdir mosaica-tutorials && cd mosaica-tutorials
    python3 -m venv .venv && . .venv/bin/activate
    pip install 'mosaica[widget]' pandas jupyterlab
    curl -O https://raw.githubusercontent.com/mosaicadb/mosaica/main/clients/py/examples/01_getting_started.ipynb
    jupyter lab 01_getting_started.ipynb
    ```

=== "marimo"

    ```bash
    mkdir mosaica-tutorials && cd mosaica-tutorials
    python3 -m venv .venv && . .venv/bin/activate
    pip install 'mosaica[widget]' pandas marimo
    curl -O https://raw.githubusercontent.com/mosaicadb/mosaica/main/clients/py/examples/01_getting_started.py
    marimo edit 01_getting_started.py
    ```

## Analysing a map with Mosaica

If you have a corpus and an analysis of your own and want to see them together, the second tutorial shows how, with 50,000 arXiv papers. You load the papers into two views, one for each of two UMAP layouts, and add the sample's named topics as a layer. Then you cluster the papers with scikit-learn's HDBSCAN, add the clusters as a second layer, and ask the server how one of them spreads over the topics. The last steps count papers by topic and by year, find which archives are over-represented among the papers about graphene, and draw one filtered selection in both layouts side by side. Allow about fifteen minutes, and install scikit-learn as well as the first tutorial's packages.

=== "Jupyter"

    ```bash
    mkdir mosaica-tutorials && cd mosaica-tutorials
    python3 -m venv .venv && . .venv/bin/activate
    pip install 'mosaica[widget]' pandas scikit-learn jupyterlab
    curl -O https://raw.githubusercontent.com/mosaicadb/mosaica/main/clients/py/examples/02_analysing_a_map.ipynb
    jupyter lab 02_analysing_a_map.ipynb
    ```

=== "marimo"

    ```bash
    mkdir mosaica-tutorials && cd mosaica-tutorials
    python3 -m venv .venv && . .venv/bin/activate
    pip install 'mosaica[widget]' pandas scikit-learn marimo
    curl -O https://raw.githubusercontent.com/mosaicadb/mosaica/main/clients/py/examples/02_analysing_a_map.py
    marimo edit 02_analysing_a_map.py
    ```

## Running the notebook on another machine

The notebook's map reads straight from the database's server, which listens on the machine the notebook runs on, on port 9141, or the next free port above it when 9141 is taken. The server prints its address when it starts:

```
mosaica: serving /dev/shm/mosaica-3k2j8x at http://127.0.0.1:9141
```

If you reach Jupyter or marimo on another machine through an SSH tunnel, forward that port as well as the notebook's. Jupyter's is 8888 and marimo's 2718:

```bash
ssh -L 8888:127.0.0.1:8888 -L 9141:127.0.0.1:9141 you@host
```

`db.map()` gives your browser the address the server printed, so forward the port to the same number on your machine. To keep the port from moving, state it: `mosaica.create(port=9141)` serves on 9141 or refuses to start, and `mosaica.open(path, port=9141)` does the same for a database you reopen.

If that port is taken on your machine, forward another one and give the map that address yourself. `mosaica.Map` takes the address your browser reaches the server at, and `db.token()` the token `db.map()` reads with:

```bash
ssh -L 8888:127.0.0.1:8888 -L 9200:127.0.0.1:9141 you@host
```

``` {.python notest}
mosaica.Map("http://127.0.0.1:9200", token=db.token(), view="map")
```

The same works for any route from your browser to the server, such as a proxy in front of it: give `Map` the address the browser uses. The notebook's own requests still go to the address the server printed. The notebook page must itself be served from `localhost`, `127.0.0.1` or `[::1]`, as it is through a tunnel, since those are the pages a database the client makes lets the map read from.

## Next

- [Count and chart from Python](../guides/analysis.md) shows how to ask the server for counts, histograms and density grids.
- The [Python client reference](../reference/python/index.md) covers every call, from `create()` to the notebook map.
- [Install and configure](install.md) explains the two files the client writes, and how to serve its database without Python.
