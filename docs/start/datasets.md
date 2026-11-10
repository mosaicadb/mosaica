# Datasets to try

Mosaica has a set of datasets that you can open on a map with one command. The command downloads the dataset from its publisher where it is not already on disk, prepares the files that its declaration names, builds the database and opens its map in a browser. Above the map are a picker for the dataset and a picker for whom to view it as, so that you can see the map change with who is looking. Each of those steps is skipped when what it made is still current, so a second run opens the map at once.

To look before installing anything, open the hosted demo at [demo.mosaicadb.com](https://demo.mosaicadb.com/). It serves the same page over 73.6 million places from Overture Maps and 50,000 arXiv papers.

To run it on your own machine, install the Python package, which carries the `mosaica` binary and the map page, with its `demo` extra, which adds what preparing the larger datasets needs:

```bash
pip install 'mosaica[demo]'
mosaica demo places
```

In a clone of the repository, `uv run mosaica demo places` runs the command from the source with [uv](https://docs.astral.sh/uv/). Its first run builds the binary with cargo and the map's JavaScript with npm.

The command prints the address of the map and keeps serving it until you press Ctrl-C. Name several datasets to open them on one page, as `mosaica demo places arxiv-sample`. `mosaica demo --list` lists the datasets with their sizes, `NAME:N` builds from part of a dataset where the dataset's section says it takes a sample, as `gbif:64`, `--title` names the page, and `--dir` keeps the datasets under another directory than the user cache (`~/.cache/mosaica/datasets/`, or `MOSAICA_DATASETS`), each in a directory of its own name. A directory there holding files the command did not make is left alone, and the command stops.

In Python, `mosaica.datasets.load` does the same steps and returns the database, which you read and map as any other:

``` {.python notest}
import mosaica

db = mosaica.datasets.load("places")
db.view("world").count()
db.map(colour_by="feature_class")
```

A dataset is rebuilt when its recipe, its prepared files or the `mosaica` binary change, and the new database replaces the old one, along with anything written to it since it was built.

| Name | Items | Download | On disk | Bundle | Making it needs |
|---|---|---|---|---|---|
| [`places`](#places) | 150,000 | 3.3 MB, none in a clone | 23 MB | 20 MB | nothing more |
| [`arxiv-sample`](#arxiv-sample) | 50,000 | 4.2 MB, none in a clone | 20 MB | 16 MB | nothing more |
| [`geonames`](#geonames) | 13,483,763 | 425 MB | 2.8 GB | 2.0 GB | DuckDB, in the `demo` extra |
| [`gbif`](#gbif) as `gbif:64` | 23,579,821 | 532 MB | 3.6 GB | 2.3 GB | NumPy, in the `demo` extra |
| [`gbif`](#gbif), the whole snapshot | about 3.8 billion, modelled | about 86 GB, modelled | about 400 GB, modelled | 196 GiB, measured on an earlier snapshot | NumPy, in the `demo` extra |

The sizes were measured with `du` on the built directories, except where the table says modelled. On disk counts the fetched files, the prepared files and the bundle.

Each dataset's access is real data standing in for an access rule, so that you can see a map change with who is looking. The page's "Viewing as" picker offers Everything, which reads as the operator, and a few readers who each hold some of the dataset's terms, which each dataset's section lists. Each reader's token is made by the page's server and holds only those terms. In Python, `db.viewer(["IE"]).map()` over `places` shows what a reader holding only Ireland sees, and `mosaica.datasets.describe(name).personas` lists the page's readers with their terms.

Not built yet: the larger datasets that Mosaica is measured against, which are arXiv (2.4 million papers), MedCPT (36 million PubMed articles), Overture (74 million places), PaperSeek (102 million works) and TreeOfLife (233 million specimens). Their recipes are in [`corpora/`](https://github.com/mosaicadb/mosaica/tree/main/corpora), which reads the publishers' files from a local copy, and four of them need a GPU to lay out their embeddings.

## `places`

150,000 GeoNames features on a world map: every populated place of 15,000 people or more and a seeded sample of everything else, from mountains to undersea features. The files are in the repository under `examples/data/geonames/`, and `NOTICE.md` there says where they came from. GeoNames publishes them under CC BY 4.0.

**Views**

| View | Projection | What it shows |
|---|---|---|
| `world` | Web Mercator | Each feature at its longitude and latitude |

**Fields**

| Field | Type | Use |
|---|---|---|
| `geonameid` | 64-bit integer | The GeoNames id, unique per feature |
| `name` | text | Searched |
| `feature_class` | category | Coloured by and filtered on |
| `feature_code` | category | Coloured by and filtered on |
| `country` | category | Coloured by and filtered on |
| `division` | category | Filtered on |
| `population` | 64-bit integer | Coloured and sized by |

**Layers**

| Layer | Kind | Levels |
|---|---|---|
| `admin/hierarchy` | tiered, each region named | Country, first-level division |

**Access.** Each feature is visible to a reader holding its country code, such as `IE`, and the 67 features with no country are public. The page's readers are Everything, the European Union's 27 members, the United States (`US`), the United Kingdom (`GB`) and Japan (`JP`).

## `arxiv-sample`

50,000 arXiv papers on two layouts of their embeddings, with the topic clusters that [Toponymy](https://github.com/TutteInstitute/toponymy) found and named, and arXiv's own classification. The papers carry titles and no abstracts. The files are in the repository under `examples/data/arxiv/`, and `NOTICE.md` there says where each came from: the paper metadata is CC0, and the layouts, clusters and names are under the repository's licence.

**Views**

| View | Projection | What it shows |
|---|---|---|
| `topics` | none | UMAP of the embeddings over 15 neighbours, which breaks subjects into many small islands |
| `topics-wide` | none | UMAP over 200 neighbours, which keeps subjects in broad regions |

**Fields**

| Field | Type | Use |
|---|---|---|
| `id` | 64-bit integer | Unique per paper |
| `archive` | category | Coloured by and filtered on |
| `primary_category` | category | Coloured by and filtered on |
| `submitted_at` | timestamp | Coloured by |
| `title` | text | Searched |
| `arxiv_id` | keyword | Looked up |

**Layers**

| Layer | Kind | Levels |
|---|---|---|
| `taxonomy/arxiv` | tiered | Archive, subject class |
| `clusters/toponymy` | tiered | 16, 46, 161 and 574 topics |
| `topics/toponymy` | the names of `clusters/toponymy` | Shown once a reader can see all six papers a name was made from |

**Access.** Each paper is visible to a reader holding any of its arXiv categories, such as `cs.LG`. The page's readers are Everything and readers holding the categories of computer science, mathematics, physics and statistics.

## `geonames`

Every feature in GeoNames' `allCountries` dump, downloaded from [download.geonames.org](https://download.geonames.org/export/dump/), which publishes it under CC BY 4.0. GeoNames rewrites the files daily, so the counts here are those of the copy fetched on 10 November 2026, 13,483,763 features, and `raw/LOCK.json` records the SHA-256 of the copy you fetch. `geonames:N` keeps the first N features in the order of their modification date.

Making it took 16 minutes on a machine with 16 cores and 15 GB of memory: two minutes to download, then DuckDB's pass over the 1.8 GB of text it unpacks, then the build. The largest process held 9.1 GB of memory at its peak. While it prepares, the unpacked text and DuckDB's working file take a few gigabytes more than the 2.8 GB left once it is built.

**Views**

| View | Projection | What it shows |
|---|---|---|
| `world` | Web Mercator | Each feature at its longitude and latitude |

**Fields**

| Field | Type | Use |
|---|---|---|
| `geonameid` | 64-bit integer | GeoNames' own id, unique per feature |
| `feature_class` | category | Coloured by and filtered on |
| `feature_code` | category | Coloured by and filtered on |
| `country` | category | Coloured by and filtered on |
| `admin1` | category | Filtered on |
| `admin2` | category | Filtered on |
| `admin3` | category | Filtered on |
| `admin4` | category | Filtered on |
| `timezone` | category | Filtered on |
| `population` | 64-bit integer | Coloured and sized by |
| `elevation` | 16-bit integer | Coloured and sized by |
| `dem` | 16-bit integer | Coloured and sized by |
| `modification_date` | timestamp | Coloured by |
| `name` | text | Searched |

**Layers**

| Layer | Kind | Levels |
|---|---|---|
| `features/taxonomy` | tiered | Feature class, feature code |
| `admin/hierarchy` | tiered, each region named | Country, then four levels of administrative division |

**Access.** Each feature is visible to a reader holding its country code, and the 7,094 features with no country are public. The page's readers are Everything, the European Union's 27 members, the United States (`US`), the United Kingdom (`GB`) and Japan (`JP`).

## `gbif`

Every occurrence in GBIF's monthly snapshot that has a coordinate: a record of an organism found at a place and time, from museum specimens to citizen observations. GBIF publishes the snapshot on Amazon S3, readable without an account, and the dataset reads the newest one the first time it is fetched, keeping the eleven of each part's fifty columns that it uses. `raw/citation.txt` holds the snapshot's DOI, which GBIF asks you to cite, and each record carries its publisher's licence: CC0 1.0, CC BY 4.0 or CC BY-NC 4.0.

`gbif:N` reads N of the snapshot's parts, evenly spaced across it, and each part holds about 370,000 records. The snapshot of 1 November 2026 has 10,346 parts. `mosaica demo gbif:64` downloaded 532 MB and holds 23,579,821 occurrences. It took 8 minutes 41 seconds on the 16-core machine above, with 3.8 GB of memory at the peak, and takes 3.6 GB on disk, 2.3 GB of it the bundle.

The whole snapshot has not been made on this path. Scaled from the 64-part sample, it is about 86 GB to download and about 3.8 billion occurrences. The dataset ladder's own build of the snapshot of 1 June 2026, 3,495,729,729 placed occurrences, made a 196 GiB bundle in 2 hours 52 minutes under a 24 GB memory budget.

**Views**

| View | Projection | What it shows |
|---|---|---|
| `geo` | Web Mercator | Each occurrence where it was recorded; a record beyond 85° north or south sits on the map's edge |

**Fields**

| Field | Type | Use |
|---|---|---|
| `kingdom` | category | Coloured by |
| `specieskey` | keyword | Filtered on: GBIF's key for the species |
| `year` | 16-bit integer | Filtered on |
| `scientificname` | keyword | Shown on the item card |
| `gbifid` | 64-bit integer | GBIF's key for the occurrence, unique per item |

**Layers**

| Layer | Kind | Levels |
|---|---|---|
| `taxonomy/tree` | tiered | Family, genus, species |

**Access.** Each occurrence carries up to three terms: its country code, `y:` and its year, and `s:` and its species key. A reader sees an occurrence when they hold any one of its terms, so `GB` sees every occurrence recorded in the United Kingdom and `y:1900` every occurrence from 1900. An occurrence with no country carries `UNRECORDED`. The page's readers are Everything, the European Union's 27 members, the United States (`US`), the United Kingdom (`GB`) and Japan (`JP`).
