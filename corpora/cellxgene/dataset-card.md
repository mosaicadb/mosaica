---
license: cc-by-4.0
pretty_name: CELLxGENE Census map
size_categories:
- 100M<n<1B
tags:
- biology
- single-cell
- cellxgene
- umap
- mosaica
configs:
- config_name: default
  data_files: parts/*.parquet
---

# CELLxGENE Census map: every human and mouse cell, laid out in 2D

This dataset places each of the 202,636,280 human and mouse cells of the CZ CELLxGENE Discover
Census, version 2025-11-08, on a 2D map. Every cell carries the Census's description of it (cell
type, tissue, disease, assay, sex, development stage, donor, dataset and collection), a position on
its species' map, and its chain of Cell Ontology types in four tiers.

It is the data behind the `cellxgene` dataset of [Mosaica](https://github.com/mosaicadb/mosaica),
which builds it into a map that you can browse in a web browser with `mosaica demo cellxgene`. The
files are ordinary Parquet, so you can read them with any Parquet tool as well.

| Species | Cells | Primary cells | Datasets |
|---|---|---|---|
| Human | 158,982,719 | 96,591,226 | 1,086 |
| Mouse | 43,653,561 | 18,384,205 | 172 |

A cell that appears in more than one Census dataset is listed once for each, and exactly one of
those rows has `is_primary_data` set.

## Source and licence

The cells and their descriptions come from the CZ CELLxGENE Discover Census, version 2025-11-08,
published by the Chan Zuckerberg Initiative under CC BY 4.0. Every dataset in the Census is CC BY
4.0 as well. Cite the Census as:

> CZI Cell Science Program, Abdulla S, Aevermann B, et al. CZ CELLxGENE Discover: a single-cell
> data platform for scalable exploration, analysis and modeling of aggregated data. *Nucleic Acids
> Research* 53(D1), D886-D900 (2025). PubMed 39607691.

The tiers come from the Cell Ontology, release 2025-10-16, which is CC BY 4.0, from the OBO
Foundry.

The positions, the tiers and the access split are derived data that Mosaica made, and they are
published here under CC BY 4.0 too. They are a demonstration: the layout is meant to look good,
and its fidelity to the embedding has not been measured.

## How it was made

The scripts are in [`corpora/cellxgene`](https://github.com/mosaicadb/mosaica/tree/main/corpora/cellxgene),
whose README gives the details and the measurements.

- **Positions.** Each species is laid out on its own, from the 50-dimensional scVI embedding the
  Census team publishes for every cell. UMAP is fitted on a uniform sample of the species' primary
  cells (4,000,000 human and 2,000,000 mouse), and every other cell is placed at the
  inverse-distance weighted mean of the positions of its 15 nearest fitted cells. The two maps
  have their own coordinates, so a human and a mouse cell at the same `x` and `y` are not related.
- **Tiers.** The Cell Ontology lets a cell type have several parents, so one chain is chosen for
  each cell type: a broad class such as "leukocyte", a lineage such as "lymphocyte", the cell
  type's parent and the cell type itself. Where a chain is shorter than four, a tier repeats the
  one above it. There are 16 classes, 148 lineages, 453 parent types and 1,014 cell types.
- **Access.** Every Census collection is public. So that a map can show what a viewer granted some
  collections sees, 35 of the 304 collections were chosen by a seeded rule to stand in for
  restricted ones. Their 25,983,078 cells, 12.8% of the whole, carry the access label
  `collection:<collection id>`; every other cell has none. `restricted-collections.json` gives the
  rule and lists the 35 collections with their names and cell counts.

## Files

The cells are split into 128 parts of about 1,583,000 cells each. Each species' cells were dealt to
the parts by a seeded random permutation, so every part holds a uniform sample of each species and
the first N parts together are a uniform sample of the whole. Mosaica's `cellxgene:N` reads the
first N parts.

| File | What it holds |
|---|---|
| `parts/part-000.parquet` … `part-127.parquet` | the cells, one row each, in `entity_id` order within a part |
| `cell-ontology-tiers.parquet` | each cell type's four tier keys, `cell_type` and `key` |
| `artifacts-cell-ontology.parquet` | each tier key's level and name |
| `vocab-*.parquet` | the values of each category, with Mosaica's codes |
| `restricted-collections.json` | the 35 collections given an access label, and the rule that chose them |

A tier key is its tier and a cell type's label, such as `t1:lymphocyte`, because a short chain
repeats a label at several tiers.

### The fields of a part

| Field | Type | Meaning |
|---|---|---|
| `entity_id` | uint64 | the cell's id here: its `soma_joinid` for a human cell, and 158,982,719 plus its `soma_joinid` for a mouse cell |
| `x`, `y` | float32 | the cell's position on its species' map |
| `access` | string | `collection:<collection id>` for a cell of one of the 35 collections, and null otherwise |
| `soma_joinid` | uint64 | the cell's row id in its species' Census experiment |
| `organism` | string | `Homo sapiens` or `Mus musculus` |
| `cell_type` | string | the Census's cell type label |
| `tissue`, `tissue_general` | string | the tissue, and its general class |
| `disease` | string | the donor's disease, or `normal` |
| `assay` | string | the sequencing assay |
| `sex` | string | `female`, `male` or `unknown` |
| `development_stage` | string | the donor's development stage |
| `self_reported_ethnicity` | string | as the Census records it |
| `suspension_type` | string | `cell` or `nucleus` |
| `collection` | string | the name of the collection the cell's dataset belongs to |
| `dataset` | string | the dataset's title; two datasets can share a title |
| `is_primary_data` | bool | false on a cell that also appears in another dataset |
| `dataset_id` | string | the Census dataset id, which names one dataset exactly |
| `donor_id` | string | the donor, as the dataset names them |
| `nnz` | uint32 | the number of genes detected in the cell |
| `raw_sum` | float32 | the cell's total count |
