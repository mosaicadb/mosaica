# CELLxGENE Census: every human and mouse cell on one map

This rung maps the 202,636,280 human and mouse cells of the CZ CELLxGENE Census, version
2025-11-08, as two views of one deployment. Each cell sits on a UMAP of the 50-dimensional scVI
embedding the Census team publishes for it, and carries its cell type, tissue, disease, assay, sex,
development stage, donor, dataset and collection. The Cell Ontology is drawn over both maps as a
four-tier layer, from a broad class such as "leukocyte" down to the cell type itself.

| View | Cells | Primary cells | Datasets |
|---|---|---|---|
| `human` | 158,982,719 | 96,591,226 | 1,086 |
| `mouse` | 43,653,561 | 18,384,205 | 172 |

A cell that appears in more than one dataset is in the corpus once per dataset, and only one of
those rows is primary.

The corpus is a demonstration and a speed benchmark. The layout only has to look good, and its
fidelity to the embedding is not measured.

## Licence and attribution

The Census data and the scVI embeddings are published under CC-BY 4.0 by the Chan Zuckerberg
Initiative, and every dataset in it is CC-BY 4.0 as well, so a deployment built from this rung
must credit them. Cite the Census as:

> CZI Cell Science Program, Abdulla S, Aevermann B, et al. CZ CELLxGENE Discover: a single-cell
> data platform for scalable exploration, analysis and modeling of aggregated data. *Nucleic Acids
> Research* 53(D1), D886-D900 (2025). PubMed 39607691.

Each dataset's own citation is in `datasets.parquet` in the staged directory. The Cell Ontology
(release 2025-10-16) is CC-BY 4.0, from the OBO Foundry.

## Reproducing it

```bash
export MOSAICA_STAGED=/path/to/staged MOSAICA_LADDER=/path/to/ladder PYTHONPATH=clients/py
~/venvs/projection/bin/python -m corpora.cellxgene.stage --obs --vectors --ontology
~/venvs/projection/bin/python -m corpora.cellxgene.prepare --sample 1000000   # a small corpus, in cellxgene-sample/
~/venvs/projection/bin/python -m corpora.cellxgene.prepare --sample 0         # every cell, in cellxgene/
cd "$MOSAICA_LADDER/cellxgene" && mosaica check && mosaica build
```

The declaration is `clients/py/mosaica/datasets/cellxgene.toml`, which the `cellxgene` dataset of
`mosaica.datasets` builds from too, so `prepare.py` imports that package; `PYTHONPATH` finds it in
the checkout.

`stage.py` reads anonymously from the Census's public S3 bucket into
`$MOSAICA_STAGED/cellxgene-census/2025-11-08/`. It writes each species' cell table as
`obs.parquet`, its scVI embedding as a float16 matrix `scvi.f16` in `soma_joinid` order, the
Census's dataset table and the Cell Ontology. The embeddings are not in the Census itself but in
two sparse arrays beside it, `CxG-czi-15` for human and `CxG-czi-16` for mouse, and every cell has
a vector. The vector step keeps a ledger of the million-row ranges it has written, so an
interrupted run resumes where it stopped.

`prepare.py --sample N` takes N cells of each species uniformly; `--sample 0` takes all of them.
It keeps each species' layout as `layout-<species>.npy` beside the data, so `--reuse-layout` can
start from it, and `--layout-only` stops once they are written. It also draws
`overview-<species>.png`, three million of the species' cells coloured by their Cell Ontology
class.

## Published for `mosaica demo`

The `cellxgene` dataset of `mosaica.datasets` is built from files cut from the full prepared corpus
and published on Hugging Face as
[`mosaicadb/cellxgene-census-map`](https://huggingface.co/datasets/mosaicadb/cellxgene-census-map),
so that it needs no GPU and none of this rung's dependencies.
[`publish.py`](publish.py) writes them: every cell's attributes, position and access label in 128
parts, each a uniform sample of each species, with each cell type's tier keys, the tier names, the
vocabularies and [`dataset-card.md`](dataset-card.md) as the card.

```bash
~/venvs/projection/bin/python -m corpora.cellxgene.publish --out /path/to/upload
hf upload mosaicadb/cellxgene-census-map /path/to/upload --repo-type dataset
```

The dataset fetches the files at one commit of that repository, `REVISION` in
`clients/py/mosaica/datasets/cellxgene.py`, which a new upload must update.

## The layout

Each species is laid out on its own. The fit set is a uniform sample of the species' primary cells
(4,000,000 human and 2,000,000 mouse at full scale), drawn from primary cells so that a cell
duplicated across datasets is not fitted twice. A CAGRA index over the fit set gives the kNN graph
that cuML's UMAP lays out, and every cell is then placed at the inverse-distance weighted mean of
its 15 nearest fit cells' positions. The metric is Euclidean, which is the metric scVI's latent
space is trained in. The fit and the placement are [`common/fit_place.py`](../common/fit_place.py),
which the TreeOfLife rung uses too.

Every cell of a species is placed against one CAGRA index, so the placement is a search per cell
and costs about 5 minutes for all human cells. CAGRA searches a 50-wide vector about 500 times
more slowly than a 64-wide one on cuVS 26.10, so `fit_place.py` pads every vector with zero
columns to a multiple of 32, which changes no distance.

Three things in the pictures look worse than a direct UMAP would:

- A haze of cells lies between the clusters. A cell whose 15 nearest fit cells sit in two clusters
  is placed between them, because its position is their weighted mean.
- Cells stack. A cell duplicated across datasets has one vector and so one position, and placed
  cells crowd onto their fit neighbours: the 158,982,719 human cells fall in 99,323,623 distinct
  cells of the build's 65,536 by 65,536 grid, and the 43,653,561 mouse cells in 18,551,038.
- The layout is not reproducible under a seed, because CAGRA's index build takes none.

The mouse map is dominated by embryo atlases: 34.3 million of its 43.7 million cells are from
embryos, so "mesodermal cell" is its second largest class.

## Measured

On one box: WSL2, 12 cores, 47 GB of memory, one RTX 3080 with 10 GB, local NVMe. Staging read
from the Census's S3 bucket in us-west-2 over a home connection.

| Step | Wall | Peak memory (GiB) | Output |
|---|---|---|---|
| `stage.py --obs --vectors --ontology` | 33 min 6 s | 23.1 | 21 GiB staged |
| of which the cell tables | 222 s | | 1.35 GB of parquet |
| of which the human vectors | 1,388 s, 114,500 rows/s | | 15.9 GB |
| of which the mouse vectors | 374 s, 116,700 rows/s | | 4.4 GB |
| `prepare.py --sample 0` | 10 min 42 s | 22.9 | 3.9 GB of parquet |
| of which the human layout | 322 s | 4.2 on the card | |
| of which the mouse layout | 113 s | 2.5 on the card | |
| of which the cells, points and members | 172 s | | |
| `mosaica build --stage-timings` | 17 min 33 s | 20.5 | 22.0 GiB bundle |
| `prepare.py --sample 0 --reuse-layout` | 3 min 20 s | 5.0 | |
| `prepare.py --sample 1000000` | 49 s | 18.1 | |
| `mosaica build` of the sample | 11 s | 0.64 | 260 MB bundle |

The peak memory of `stage.py` and `prepare.py` is the resident set, which counts the pages of the
memory-mapped vector files they write or read.

| Layout step | Human, 4,000,000 fit cells | Mouse, 2,000,000 fit cells |
|---|---|---|
| CAGRA index over the fit set | 8.0 s | 3.5 s |
| its kNN graph | 6.1 s | 3.1 s |
| UMAP over the graph | 27.3 s | 27.3 s |
| placing every cell | 276 s, 575,900 cells/s | 78 s, 559,800 cells/s |

A server over the full bundle answered `/readyz` 4.0 s after starting. An anonymous viewer's
whole-map viewport at zoom 2 took 0.03 s on the human view, and a count of the visible cells by
`tissue_general` took 0.06 s.

## Visibility by collection

Every Census collection is public. So that a viewer granted some collections sees more than one
granted none, this rung treats a seeded minority of collections as restricted: a cell of a
restricted collection carries the access term `collection:<collection id>`, and every other cell
has no label and takes the view's default, `public`.

The restricted collections are chosen from the collection ids and their cell counts in this
Census version. The ids set the order, which is that of
`sha256("mosaica-cellxgene-restricted:" + collection_id)`, and the counts decide which are taken:
each collection is added unless it would carry the total past 15% of all cells, until the total
reaches 12%. Another Census version, with other counts, can restrict other collections.
`restricted-collections.json`, beside the data, lists each restricted collection with its term,
name and cell count.

The rule picks 35 of the 304 collections that hold human or mouse cells, which hold 25,983,078
cells, 12.82% of the corpus: 15.40% of human cells and 3.43% of mouse cells. An anonymous viewer
sees 134,498,431 human cells, and a viewer granted the three largest restricted collections sees
147,577,505.

Every vocabulary is `derived`, so a viewer is told a value, such as a disease, a cell type or a
collection's name, only where they can see one of its cells. A value carried only by restricted
collections is not listed to a viewer who holds none of them.

## The Cell Ontology layer

The Cell Ontology is a graph in which a cell type often has several parents, and a tiered layer
needs one path per cell. [`ontology.py`](ontology.py) chooses the path from the ontology and the
Census's list of cell types, so it does not depend on which cells a run samples:

1. A cell type's class is the first of a fixed, ordered list of broad cell types that is the cell
   type or one of its ancestors: leukocyte, hematopoietic cell, endothelial cell, epithelial cell,
   neural cell, muscle cell, germ cell, pigment cell, connective tissue cell, stromal cell, stem
   cell, precursor cell, abnormal cell, extraembryonic cell and mesodermal cell. A cell type under
   none of them is in "other cell", and the Census's `unknown` is a class of its own. Its lineage
   is the first of lymphocyte, myeloid leukocyte, neuron and glial cell that is the cell type or an
   ancestor of it and lies inside its class, where there is one.
2. Among a cell type's parents that share its class and lineage, the one with the most Census cell
   types beneath it is its parent, and a tie goes to the lower CL id. Following parents leads from
   every cell type to its lineage or class, which gives one chain.
3. The four tiers of a chain from the class `c0` to the cell type `cd` are `c0`, `c1`, `c(d-1)`
   and `cd`, a tier repeating the one above it where the chain is shorter than four.

The tiers are titled Class, Lineage, Parent type and Cell type, and are served at zooms 0-3, 3-6,
6-9 and 9-16. Mouse cells use the same tiers, since the Cell Ontology does not depend on species.
An artifact's key is its tier and label, such as `t1:lymphocyte`, because a short chain repeats a
label at several tiers and a tiered layer resolves a parent by key; each artifact carries the
label alone as its name.

The rule gives 16 classes, 148 lineages, 453 parent types and 1,014 cell types. 54 cell types,
1.6% of cells, are under none of the classes and are in "other cell", among them "kidney cell",
"secretory cell" and "mesenchymal cell", and 1.5% of cells are `unknown`. "neural cell" is both a
class and a cell type the Census uses, for 2.5% of cells, so it repeats at every tier.

## Fields

Every attribute is read from `cells.parquet`; each view's points file holds only its species'
positions and access labels. The categories are rendered, so the map can be coloured by them, and
indexed, so they can be filtered, except `organism` and `dataset`, which are only indexed.
`is_primary_data` is false on a cell that also appears in another dataset, so filtering on it
gives unique cells only. `n_genes` is the Census's `nnz`, the number of genes detected. `dataset`
is the dataset's title, and two datasets with the same title share one value; `dataset_id` names
one dataset exactly.
