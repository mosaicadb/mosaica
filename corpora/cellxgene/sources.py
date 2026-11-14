"""Where the staged Census lives, and the constants `stage.py` and `prepare.py` share."""

from __future__ import annotations

from pathlib import Path

from ..common.paths import STAGED_ROOT, ladder

RUNG = "cellxgene"
DATASET, VINTAGE = "cellxgene-census", "2025-11-08"

#: The two species this corpus maps, by their Census experiment name, with the scVI embedding
#: published for each. The embeddings are SOMA sparse arrays outside the Census itself, indexed by
#: the experiment's obs `soma_joinid`.
SPECIES = {
    "human": {"experiment": "homo_sapiens", "embedding": "CxG-czi-15", "rows": 158_982_719},
    "mouse": {"experiment": "mus_musculus", "embedding": "CxG-czi-16", "rows": 43_653_561},
}
EMBEDDING_ROOT = f"s3://cellxgene-contrib-public/contrib/cell-census/soma/{VINTAGE}"
DIM = 50

#: Anonymous reads from the public bucket.
S3_CONFIG = {"vfs.s3.region": "us-west-2", "vfs.s3.no_sign_request": "true"}

#: The obs columns staged. The ontology term ids are kept for `cell_type` alone, which the
#: hierarchy is built from; every other column is used by its label.
OBS_COLUMNS = [
    "soma_joinid",
    "dataset_id",
    "assay",
    "cell_type",
    "cell_type_ontology_term_id",
    "development_stage",
    "disease",
    "donor_id",
    "is_primary_data",
    "self_reported_ethnicity",
    "sex",
    "suspension_type",
    "tissue",
    "tissue_general",
    "nnz",
    "raw_sum",
]

#: The Cell Ontology release the hierarchy is read from, pinned so a rerun builds the same tree.
CL_RELEASE = "2025-10-16"
CL_URL = f"http://purl.obolibrary.org/obo/cl/releases/{CL_RELEASE}/cl-basic.json"


def staging() -> Path:
    """The staged acquisition, created if absent."""
    path = STAGED_ROOT / DATASET / VINTAGE
    path.mkdir(parents=True, exist_ok=True)
    return path


def obs_path(species: str) -> Path:
    return staging() / species / "obs.parquet"


def vectors_path(species: str) -> Path:
    """`rows x 50` float16 in soma_joinid order, written by `stage.py --vectors`."""
    return staging() / species / "scvi.f16"


def vectors(species: str):
    import numpy as np

    n = SPECIES[species]["rows"]
    return np.memmap(vectors_path(species), dtype=np.float16, mode="r", shape=(n, DIM))


def rung(sample: int) -> Path:
    """The derived directory: the rung's own at full scale, a sibling for a sample."""
    return ladder(RUNG if not sample else f"{RUNG}-sample")
