"""CELLxGENE: the 202,636,280 human and mouse cells of the CZ CELLxGENE Census 2025-11-08, each on
a UMAP of its species' scVI embedding, with the Cell Ontology as a four-tier layer.

Laying the cells out takes a GPU and the Census's 20 GB of embeddings, so this dataset is fetched
from the files `corpora/cellxgene/` prepared and published on Hugging Face, at a pinned revision,
rather than from the Census itself. The cells are in `PARTS` parts, and each part is a uniform
sample of each species. `sample=N` fetches the first N parts, `DEFAULT_PARTS` without a sample, and
every part from `sample=PARTS` up.

`prepare` writes the files the declaration, `cellxgene.toml`, names: the cells' attributes, each
species' points, the layer's members from each cell type's tier keys, and the vocabularies and tier
names as published. The vocabularies are those of the whole Census, so a value carried by no cell
of a sample is in its vocabulary but is shown to nobody, since every vocabulary is `derived`.

What both this module and `corpora/cellxgene/prepare.py` need is here: the declaration, the
vocabularies and how their blocks are written, and the organisms' names.
"""

from __future__ import annotations

import shutil
from pathlib import Path
from typing import Optional

REPOSITORY = "mosaicadb/cellxgene-census-map"
#: The commit of `REPOSITORY` the files are fetched from.
REVISION = "e41205a87f13ecf8deae13b7ac810c1be2aad382"
BASE = f"https://huggingface.co/datasets/{REPOSITORY}/resolve/{REVISION}/"

PARTS = 128
DEFAULT_PARTS = 2

DECLARATION = Path(__file__).with_name("cellxgene.toml")

ORGANISM = {"human": "Homo sapiens", "mouse": "Mus musculus"}

#: `(vocabulary, [sources] key, file stem, title, visibility, column of cells.parquet)`. Every one
#: is `derived`, because a value carried only by restricted collections, such as a disease, would
#: otherwise be listed to a viewer who can see none of its cells.
VOCABULARIES = [
    ("cell_type", "cell_type", "cell-type", "Cell type", "derived", "cell_type"),
    ("tissue", "tissue", "tissue", "Tissue", "derived", "tissue"),
    ("tissue_general", "tissue_general", "tissue-general", "Tissue (general)", "derived",
     "tissue_general"),
    ("disease", "disease", "disease", "Disease", "derived", "disease"),
    ("assay", "assay", "assay", "Assay", "derived", "assay"),
    ("sex", "sex", "sex", "Sex", "derived", "sex"),
    ("development_stage", "development", "development-stage", "Development stage", "derived",
     "development_stage"),
    ("self_reported_ethnicity", "ethnicity", "self-reported-ethnicity", "Self-reported ethnicity",
     "derived", "self_reported_ethnicity"),
    ("suspension_type", "suspension", "suspension-type", "Suspension type", "derived",
     "suspension_type"),
    ("organism", "organism", "organism", "Organism", "derived", "organism"),
    ("collection", "collection", "collection", "Collection", "derived", "collection"),
    ("dataset", "dataset", "dataset", "Dataset", "derived", "dataset"),
]

#: The access terms of the 35 collections `corpora/cellxgene/prepare.py` restricts, largest first,
#: as `restricted-collections.json` lists them.
RESTRICTED = tuple(
    f"collection:{uuid}"
    for uuid in """
    f11cb29c-b546-4738-9bd8-66ea621a7bd5 48259aa8-f168-4bf5-b797-af8e88da6637
    854c0855-23ad-4362-8b77-6b1639e7a9fc 77f9d7e9-5675-49c3-abed-ce02f39eef1b
    6f6d381a-7701-4781-935c-db10d30de293 edb893ee-4066-4128-9aec-5eb2b03f8287
    b9fc3d70-5a72-4479-a046-c2cc1ab19efc a0c84e3f-a5ca-4481-b3a5-ccfda0a81ecc
    af893e86-8e9f-41f1-a474-ef05359b1fb7 d86517f0-fa7e-4266-b82e-a521350d6d36
    2a9a17c9-1f61-4877-b384-b8cd5ffa4085 c450e15d-321a-42d6-986b-11409d04896d
    4a2c25af-558a-45fc-bc9a-54ec44a1d63f b0cf0afa-ec40-4d65-b570-ed4ceacc6813
    fbc5881f-1ee3-4ffe-8095-35e15e1a08fc e4c9ed14-e560-4900-a3bf-b0f8d2ce6a10
    120e86b4-1195-48c5-845b-b98054105eec f406a653-c079-4bf9-aab6-85846c27571d
    54004c5c-af08-4693-a606-73871b6ef989 1d1c7275-476a-49e2-9022-ad1b1c793594
    31937775-0602-4e52-a799-b6acdd2bac2e a9254216-6cd8-4186-b32c-349363777584
    a261413d-835b-4f1e-ab0c-dada55ea6afd 367d95c0-0eb0-4dae-8276-9407239421ee
    13d1c580-4b17-4b2e-85c4-75b36917413f c9706a92-0e5f-46c1-96d8-20e42467f287
    6686ada5-43a8-40b6-9a05-6894503606db 3472f32d-4a33-48e2-aad5-666d4631bf4c
    964581c3-b5ac-494d-9bd4-0dcb6fb058da ff668d5d-5b3f-49ee-a007-ff0664bf35ec
    64b24fda-6591-4ce1-89e7-33eb6c43ad7b 99be70c4-e534-4413-a607-ce40b3ff2c77
    558385a4-b7b7-4eca-af0c-9e54d010e8dc 1e313b15-4aca-4e5a-94a3-1093c4c42abd
    ce5b6795-dace-408b-b26b-2ab752c3e20d
    """.split()
)

#: The published files besides the parts.
TIERS = "cell-ontology-tiers.parquet"
ARTIFACTS = "artifacts-cell-ontology.parquet"
SMALL = [
    TIERS,
    ARTIFACTS,
    "restricted-collections.json",
    *(f"vocab-{stem}.parquet" for _, _, stem, *_ in VOCABULARIES),
]


def vocabulary_toml(sizes: dict[str, int]) -> str:
    """The `[[vocabulary]]` blocks, each as wide as the code space its `sizes` entry needs."""

    def width(n: int) -> str:
        return "u8" if n < 255 else ("u16" if n < 65_535 else "u32")

    blocks = []
    for name, key, _, title, visibility, _ in VOCABULARIES:
        n = sizes[name]
        blocks.append(
            "[[vocabulary]]\n"
            f'name       = "{name}"\n'
            f'title      = "{title}"  # {n:,} keys\n'
            f'width      = "{width(n)}"\n'
            'value_set  = "closed"\n'
            f'visibility = "{visibility}"\n'
            f'source     = "{key}"'
        )
    return "\n\n".join(blocks)


def write_declaration(path: Path, sizes: dict[str, int]) -> None:
    """`cellxgene.toml` at `path`, its `# <vocabularies>` line replaced by the blocks."""
    lines = DECLARATION.read_text(encoding="utf-8").splitlines()
    hit = [i for i, line in enumerate(lines) if line.strip() == "# <vocabularies>"]
    assert len(hit) == 1, f"# <vocabularies> appears {len(hit)} times as a line of its own"
    lines[hit[0]] = vocabulary_toml(sizes)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def parts(sample: Optional[int]) -> list[str]:
    """The names of the parts `sample` reads."""
    taken = min(sample or DEFAULT_PARTS, PARTS)
    return [f"parts/part-{i:03d}.parquet" for i in range(taken)]


def fetch(raw: Path, progress: bool = True, sample: Optional[int] = None) -> list[Path]:
    """Download the small files and the parts `sample` reads into `raw`, each once."""
    from . import _steps

    return _steps.fetch(raw, {name: BASE + name for name in SMALL + parts(sample)}, progress)


def prepare(raw: Path, out: Path, sample: Optional[int] = None) -> None:
    """Write the files `cellxgene.toml` names into `out`, from the parts `sample` reads."""
    import duckdb
    import pyarrow.parquet as pq

    con = duckdb.connect()
    con.execute("SET enable_progress_bar = false")
    con.execute(f"SET temp_directory = '{out / '.duckdb'}'")
    listed = ", ".join(f"'{raw / name}'" for name in parts(sample))
    con.execute(f"CREATE VIEW cells AS SELECT * FROM read_parquet([{listed}])")

    def copy(query: str, name: str) -> None:
        con.execute(f"COPY ({query}) TO '{out / name}' (FORMAT parquet, COMPRESSION zstd)")

    copy("SELECT * EXCLUDE (x, y, access) FROM cells", "cells.parquet")
    for species, organism in ORGANISM.items():
        copy(
            "SELECT entity_id, x::DOUBLE AS x, y::DOUBLE AS y, access FROM cells "
            f"WHERE organism = '{organism}'",
            f"points-{species}.parquet",
        )
    copy(
        f"SELECT c.entity_id AS entity, t.key FROM cells c "
        f"JOIN read_parquet('{raw / TIERS}') t USING (cell_type)",
        "members-cell-ontology.parquet",
    )
    cells = con.execute("SELECT count(*) FROM cells").fetchone()[0]
    members = pq.ParquetFile(out / "members-cell-ontology.parquet").metadata.num_rows
    if members != cells:
        raise ValueError(
            f"{cells - members:,} of {cells:,} cells have a cell type that {TIERS} does not "
            "list; fetch the files again"
        )
    con.close()
    shutil.rmtree(out / ".duckdb", ignore_errors=True)

    sizes = {}
    for name, _, stem, *_ in VOCABULARIES:
        shutil.copyfile(raw / f"vocab-{stem}.parquet", out / f"vocab-{stem}.parquet")
        sizes[name] = pq.ParquetFile(out / f"vocab-{stem}.parquet").metadata.num_rows
    shutil.copyfile(raw / ARTIFACTS, out / ARTIFACTS)
    write_declaration(out / "schema.toml", sizes)
