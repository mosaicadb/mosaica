# Datasets to try

These datasets let you see Mosaica working on real data with one command. `mosaica demo` downloads a dataset, builds a database from it and opens its map in your browser. Above the map you can switch between datasets and choose whom to view the map as, and every point, count and cluster changes to what that viewer may see.

To look before installing anything, open the hosted demo at [demo.mosaicadb.com](https://demo.mosaicadb.com/). It shows the same page over 73.6 million places from Overture Maps and 50,000 arXiv papers.

To run it on your own machine, install the Python package with its `demo` extra and name a dataset:

```bash
pip install 'mosaica[demo]'
mosaica demo places
```

The command prints the map's address and keeps serving it until you press Ctrl-C. The next run reuses what the first one built, so the map opens straight away. Name several datasets to put them on one page, as in `mosaica demo places arxiv-sample`, and use `mosaica demo --list` to see them all with their sizes. A large dataset can be built from part of itself: `gbif:64` reads 64 parts of GBIF instead of all of them. `--title` sets the page's title, and `--dir` keeps the datasets somewhere other than `~/.cache/mosaica/datasets/`.

In a clone of the repository, run `uv run mosaica demo places` instead. Its first run builds the `mosaica` binary and the map's JavaScript, so it needs cargo and npm.

The same datasets load in Python, where you can read and map them like any other database:

``` {.python notest}
import mosaica

db = mosaica.datasets.load("places")
db.view("world").count()
db.map(colour_by="feature_class")
```

After you upgrade Mosaica, the next run rebuilds the dataset, which replaces anything you wrote to it.

| Name | Items | Download | On disk |
|---|---|---|---|
| [`places`](#places) | 150,000 | 3 MB, or nothing in a clone | 23 MB |
| [`arxiv-sample`](#arxiv-sample) | 50,000 | 4 MB, or nothing in a clone | 20 MB |
| [`geonames`](#geonames) | 13.5 million | 425 MB | 2.8 GB |
| [`gbif`](#gbif) as `gbif:64` | 24 million | 530 MB | 3.6 GB |
| [`gbif`](#gbif), whole | about 3.8 billion | about 86 GB | about 400 GB |

Each dataset uses one of its real fields to decide who may see each item, standing in for the access rules your own data would have. The "Viewing as" picker offers Everything, which sees every item, and a few viewers who may each see part of the dataset. Each section below says who they are. In Python, `db.viewer(["IE"]).map()` shows `places` as someone who may see only Ireland.

Not built yet: the larger datasets Mosaica is benchmarked on, which are the full arXiv, MedCPT, Overture, PaperSeek and TreeOfLife. Their recipes are in [`corpora/`](https://github.com/mosaicadb/mosaica/tree/main/corpora), and most of them need a GPU to prepare.

## `places`

150,000 places from GeoNames: every town and city of 15,000 people or more, and a sample of everything else, from mountains to undersea features. The data comes with the repository, and GeoNames publishes it under CC BY 4.0.

**Views**

| View | Projection | What it shows |
|---|---|---|
| `world` | Web Mercator | Each place on a world map |

**Fields**

| Field | Type | Use |
|---|---|---|
| `geonameid` | 64-bit integer | GeoNames' id for the place |
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

**Access.** A viewer sees a place when they hold its country code, such as `IE`. The picker's viewers are Everything, the European Union, the United States, the United Kingdom and Japan.

## `arxiv-sample`

50,000 arXiv papers laid out by topic in two maps, with arXiv's own classification and the topic clusters that [Toponymy](https://github.com/TutteInstitute/toponymy) found and named. The papers have titles but no abstracts. The data comes with the repository, and arXiv publishes the paper metadata under CC0.

**Views**

| View | Projection | What it shows |
|---|---|---|
| `topics` | none | Papers by topic, in many small islands |
| `topics-wide` | none | Papers by topic, in broader regions |

**Fields**

| Field | Type | Use |
|---|---|---|
| `id` | 64-bit integer | The paper's id in the sample |
| `archive` | category | Coloured by and filtered on |
| `primary_category` | category | Coloured by and filtered on |
| `submitted_at` | timestamp | Coloured by |
| `title` | text | Searched |
| `arxiv_id` | keyword | Looked up |

**Layers**

| Layer | Kind | Levels |
|---|---|---|
| `taxonomy/arxiv` | tiered, each category named | Archive, subject class |
| `clusters/toponymy` | tiered | 16, 46, 161 and 574 topics |
| `topics/toponymy` | names for `clusters/toponymy` | One name for each topic |

**Access.** A viewer sees a paper when they hold any of its arXiv categories, such as `cs.LG`. The picker's viewers are Everything, computer science, mathematics, physics and statistics.

## `geonames`

Every place in GeoNames, about 13.5 million, downloaded from [download.geonames.org](https://download.geonames.org/export/dump/) under CC BY 4.0. GeoNames updates its files every day, so your count will differ a little from the table's. To try a smaller part first, `geonames:1000000` builds from a million of them.

It takes about a quarter of an hour on a 16-core machine with 16 GB of memory, and needs a few gigabytes of extra disk space while it prepares.

**Views**

| View | Projection | What it shows |
|---|---|---|
| `world` | Web Mercator | Each place on a world map |

**Fields**

| Field | Type | Use |
|---|---|---|
| `geonameid` | 64-bit integer | GeoNames' id for the place |
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

**Access.** A viewer sees a place when they hold its country code. The picker's viewers are Everything, the European Union, the United States, the United Kingdom and Japan.

## `gbif`

Every GBIF occurrence with a coordinate: a record of a plant, animal or fungus seen at a place and time, from museum specimens to citizen sightings. Mosaica reads GBIF's monthly snapshot from its public copy on Amazon S3. Each record carries its publisher's licence, which is CC0, CC BY or CC BY-NC, and GBIF asks you to cite the snapshot's DOI, which is saved in `raw/citation.txt`.

The whole snapshot is very large, so start with a sample. `mosaica demo gbif:64` reads 64 of the snapshot's roughly 10,000 parts, which is about 24 million occurrences, and takes under ten minutes. Each part you add brings about 370,000 more.

**Views**

| View | Projection | What it shows |
|---|---|---|
| `geo` | Web Mercator | Each occurrence where it was recorded |

**Fields**

| Field | Type | Use |
|---|---|---|
| `kingdom` | category | Coloured by |
| `specieskey` | keyword | Filtered on |
| `year` | 16-bit integer | Filtered on |
| `scientificname` | keyword | Shown on the item card |
| `gbifid` | 64-bit integer | GBIF's id for the occurrence |

**Layers**

| Layer | Kind | Levels |
|---|---|---|
| `taxonomy/tree` | tiered | Family, genus, species |

**Access.** A viewer sees an occurrence when they hold its country code, its year as in `y:1900`, or its species key as in `s:2498036`. The picker's viewers are Everything, the European Union, the United States, the United Kingdom and Japan.
