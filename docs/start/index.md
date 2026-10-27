# Start

Install Mosaica, then build a first map.

- [Install and configure](install.md): what to install for what you want to do, the platforms it runs on, and the two files a deployment is configured with, `schema.toml` and `mosaica.toml`.
- [Your first map from files](first-map.md): the GeoNames places for Ireland, from download to a filtered map, with `mosaica check`, `mosaica build` and `mosaica serve`.
- [Your first map in Python](python.md#your-first-map-in-python): a notebook that takes 150,000 GeoNames places from a pandas data frame to a map, then filters, searches and counts them.
- [Analysing a map with Mosaica](python.md#analysing-a-map-with-mosaica): a notebook that loads 50,000 arXiv papers in two layouts with their named topics, adds clusters of your own from HDBSCAN, and compares the two with counts, histograms and lift worked out by the server.
- [Your first map in a web page](typescript.md): the `<mosaica-explorer>` element in a new Vite project, over 150,000 GeoNames places, with search, filters and a layout of the page's own.
- [Your own interface around the map](typescript-own-ui.md): a bare `<mosaica-map>` over 50,000 arXiv papers, with the page's own menus, results list and detail panel driving it through the store, and the papers coloured and filtered by topic.
