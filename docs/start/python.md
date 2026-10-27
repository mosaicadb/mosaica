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

## Next

- The [Python client reference](../reference/python/index.md) covers every call, from `create()` to the notebook map.
- [Install and configure](install.md) explains the two files the client writes, and how to serve its database without Python.
