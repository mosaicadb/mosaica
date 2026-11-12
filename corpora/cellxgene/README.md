# CELLxGENE Census: every human and mouse cell on one map

This rung maps the 202,636,280 human and mouse cells of the CZ CELLxGENE Census, version
2025-11-08, as two views of one deployment. Each cell sits on a UMAP of the 50-dimensional scVI
embedding the Census team publishes for it, and carries its cell type, tissue, disease, assay, sex,
development stage, donor, dataset and collection. The Cell Ontology is drawn over both maps as a
four-tier layer, from a broad class such as "leukocyte" down to the cell type itself.

| View | Cells | Of which primary |
|---|---|---|
| `human` | 158,982,719 | 96.6M |
| `mouse` | 43,653,561 | |

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
export MOSAICA_STAGED=/path/to/staged MOSAICA_LADDER=/path/to/ladder
~/venvs/projection/bin/python -m corpora.cellxgene.stage --obs --vectors --ontology
~/venvs/projection/bin/python -m corpora.cellxgene.prepare --sample 1000000   # a small corpus, in cellxgene-sample/
~/venvs/projection/bin/python -m corpora.cellxgene.prepare --sample 0         # every cell, in cellxgene/
cd "$MOSAICA_LADDER/cellxgene" && mosaica check && mosaica build
```

`stage.py` reads anonymously from the Census's public S3 bucket into
`$MOSAICA_STAGED/cellxgene-census/2025-11-08/`. It writes each species' cell table as
`obs.parquet`, its scVI embedding as a float16 matrix `scvi.f16` in `soma_joinid` order, the
Census's dataset table and the Cell Ontology. The embeddings are not in the Census itself but in
two sparse arrays beside it, `CxG-czi-15` for human and `CxG-czi-16` for mouse, and every cell has
a vector. The vector step keeps a ledger of the million-row ranges it has written, so an
interrupted run resumes where it stopped.

`prepare.py --sample N` takes N cells of each species uniformly; `--sample 0` takes all of them.
`--layout-only` stops after the layouts and `--reuse-layout` starts from them.

## The layout

Each species is laid out on its own. The fit set is a uniform sample of the species' primary cells
(4,000,000 human and 2,000,000 mouse at full scale), drawn from primary cells so that a cell
duplicated across datasets is not fitted twice. A CAGRA index over the fit set gives the kNN graph
that cuML's UMAP lays out, and every cell is then placed at the inverse-distance weighted mean of
its 15 nearest fit cells' positions. The metric is Euclidean, which is the metric scVI's latent
space is trained in. The fit and the placement are [`common/fit_place.py`](../common/fit_place.py),
which the TreeOfLife rung uses too.

<!-- measured -->

## Visibility by collection

Every Census collection is public. So that a viewer granted some collections sees more than one
granted none, this rung treats a seeded minority of collections as restricted: a cell of a
restricted collection carries the access term `collection:<collection id>`, and every other cell
has no label and takes the view's default, `public`.

The restricted collections are chosen from the collection ids alone. They are taken in the order
of `sha256("mosaica-cellxgene-restricted:" + collection_id)`, and each is added unless it would
carry the total past 15% of all cells, until the total reaches 12%.
`restricted-collections.json`, beside the data, lists each restricted collection with its term,
name and cell count.

<!-- restricted -->

The `collection` and `dataset` vocabularies are `derived`, so a viewer is told the name of a
collection or a dataset only where they can see one of its cells.

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

<!-- ontology -->

## Fields

Every attribute is read from `cells.parquet`; each view's points file holds only its species'
positions and access labels. The categories are rendered, so the map can be coloured by them, and
indexed, so they can be filtered, except `organism` and `dataset`, which are only indexed.
`is_primary_data` is false on a cell that also appears in another dataset, so filtering on it
gives unique cells only. `n_genes` is the Census's `nnz`, the number of genes detected. `dataset`
is the dataset's title, and two datasets with the same title share one value; `dataset_id` names
one dataset exactly.
