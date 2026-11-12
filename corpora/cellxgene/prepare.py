"""CZ CELLxGENE Census 2025-11-08 as a corpus: every human and mouse cell on a UMAP of its scVI
embedding, with the Cell Ontology as a four-tier layer.

Reads what `stage.py` downloaded and writes, into `$MOSAICA_LADDER/cellxgene/` (or
`cellxgene-sample/` under `--sample`):

- `cells.parquet`: one row per cell of both species, every attribute.
- `points-human.parquet`, `points-mouse.parquet`: each species' positions and access labels.
- `members-cell-ontology.parquet`: each cell's four tier keys, from `ontology.py`, and
  `artifacts-cell-ontology.parquet`, each tier artifact's name.
- `vocab-*.parquet`, `corpus.toml`, `mosaica.toml`, `manifest.json`,
  `restricted-collections.json`, and `overview-<species>.png`.

The layout of each species is fitted on a uniform sample of its primary cells and every cell is
placed against it (`corpora/common/fit_place.py`), in scVI's own Euclidean metric.

    ~/venvs/projection/bin/python -m corpora.cellxgene.prepare --sample 1000000
    ~/venvs/projection/bin/python -m corpora.cellxgene.prepare --sample 0
"""

from __future__ import annotations

import argparse
import hashlib
import json
import resource
import sys
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.parquet as pq

from ..common import fit_place
from ..common.rung_files import write_deployment, write_vocabulary
from ..common.timing import Steps
from . import ontology, sources

SEED = 0

#: Primary cells each species' UMAP is fitted on.
FIT_ROWS = {"human": 4_000_000, "mouse": 2_000_000}

#: Rows placed per read of the staged vectors.
PLACE_CHUNK = 2_000_000

ORGANISM = {"human": "Homo sapiens", "mouse": "Mus musculus"}

#: The share of all cells, both species together, that the restricted collections reach for, and
#: the most they may hold. Collections are taken in the order of `sha256(RESTRICTION_SALT + id)`;
#: one that would take the total past the ceiling is passed over, and the walk stops once the
#: total reaches the target.
RESTRICTION_SALT = "mosaica-cellxgene-restricted:"
RESTRICTED_TARGET = 0.12
RESTRICTED_CEILING = 0.15

#: `(vocabulary, [sources] key, file stem, title, visibility, column of cells.parquet)`.
VOCABULARIES = [
    ("cell_type", "cell_type", "cell-type", "Cell type", "public", "cell_type"),
    ("tissue", "tissue", "tissue", "Tissue", "public", "tissue"),
    ("tissue_general", "tissue_general", "tissue-general", "Tissue (general)", "public",
     "tissue_general"),
    ("disease", "disease", "disease", "Disease", "public", "disease"),
    ("assay", "assay", "assay", "Assay", "public", "assay"),
    ("sex", "sex", "sex", "Sex", "public", "sex"),
    ("development_stage", "development", "development-stage", "Development stage", "public",
     "development_stage"),
    ("self_reported_ethnicity", "ethnicity", "self-reported-ethnicity", "Self-reported ethnicity",
     "public", "self_reported_ethnicity"),
    ("suspension_type", "suspension", "suspension-type", "Suspension type", "public",
     "suspension_type"),
    ("organism", "organism", "organism", "Organism", "public", "organism"),
    ("collection", "collection", "collection", "Collection", "derived", "collection"),
    ("dataset", "dataset", "dataset", "Dataset", "derived", "dataset"),
]

#: Obs columns copied into `cells.parquet` under their own names, as strings.
CATEGORY_COLUMNS = ["cell_type", "tissue", "tissue_general", "disease", "assay", "sex",
                    "development_stage", "self_reported_ethnicity", "suspension_type"]

CELLS_SCHEMA = pa.schema(
    [
        pa.field("entity_id", pa.uint64()),
        pa.field("soma_joinid", pa.uint64()),
        pa.field("organism", pa.string()),
        *[pa.field(c, pa.string()) for c in CATEGORY_COLUMNS],
        pa.field("collection", pa.string()),
        pa.field("dataset", pa.string()),
        pa.field("is_primary_data", pa.bool_()),
        pa.field("dataset_id", pa.string()),
        pa.field("donor_id", pa.string()),
        pa.field("nnz", pa.uint32()),
        pa.field("raw_sum", pa.float32()),
    ]
)
POINTS_SCHEMA = pa.schema(
    [
        pa.field("entity_id", pa.uint64()),
        pa.field("x", pa.float64()),
        pa.field("y", pa.float64()),
        pa.field("access", pa.string()),
    ]
)
MEMBER_SCHEMA = pa.schema(
    [pa.field("entity", pa.uint64()), pa.field("key", pa.list_(pa.string()))]
)


def peak_gb() -> float:
    return resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 2**20


def entity_offset(species: str) -> int:
    """Human cells keep their `soma_joinid`; mouse cells follow every human one."""
    return 0 if species == "human" else sources.SPECIES["human"]["rows"]


# ------------------------------------------------------------------------------------ inputs


def dataset_table() -> dict[str, dict]:
    table = pq.read_table(sources.staging() / "datasets.parquet",
                          columns=["dataset_id", "collection_id", "collection_name",
                                   "dataset_title"])
    return {r["dataset_id"]: r for r in table.to_pylist()}


def census_summary() -> tuple[dict, dict[str, str], dict[str, np.ndarray]]:
    """Per species: cells per dataset, the cell types the Census uses (id to label), and the
    primary flag of every row."""
    per_dataset: dict = {}
    types: dict[str, str] = {}
    primary: dict[str, np.ndarray] = {}
    for species in sources.SPECIES:
        t = pq.read_table(sources.obs_path(species),
                          columns=["dataset_id", "cell_type_ontology_term_id", "cell_type",
                                   "is_primary_data"]).unify_dictionaries()
        vc = pc.value_counts(t.column("dataset_id").combine_chunks())
        per_dataset[species] = dict(zip(vc.field("values").to_pylist(),
                                        vc.field("counts").to_pylist()))
        # The distinct (id, label) pairs, found over dictionary codes rather than 10^8 strings.
        ids, labels = (encoded(t.column(c)) for c in ("cell_type_ontology_term_id", "cell_type"))
        pairs = np.unique(ids.indices.to_numpy().astype(np.int64) << 32
                          | labels.indices.to_numpy().astype(np.int64))
        for pair in pairs:
            types.setdefault(ids.dictionary[int(pair >> 32)].as_py(),
                             labels.dictionary[int(pair & 0xFFFFFFFF)].as_py())
        primary[species] = t.column("is_primary_data").to_numpy()
        del t, ids, labels
    return per_dataset, types, primary


def restricted_collections(per_dataset: dict, datasets: dict) -> tuple[set[str], dict]:
    """The collections whose cells carry an access term, by the rule at `RESTRICTION_SALT`."""
    cells: dict[str, int] = {}
    for counts in per_dataset.values():
        for dataset_id, n in counts.items():
            cid = datasets[dataset_id]["collection_id"]
            cells[cid] = cells.get(cid, 0) + n
    total = sum(cells.values())
    order = sorted(cells, key=lambda c: hashlib.sha256((RESTRICTION_SALT + c).encode()).hexdigest())
    chosen: list[str] = []
    held = 0
    for cid in order:
        if held >= RESTRICTED_TARGET * total:
            break
        if held + cells[cid] <= RESTRICTED_CEILING * total:
            chosen.append(cid)
            held += cells[cid]
    names = {d["collection_id"]: d["collection_name"] for d in datasets.values()}
    by_species = {
        species: sum(n for d, n in counts.items() if datasets[d]["collection_id"] in chosen)
        for species, counts in per_dataset.items()
    }
    report = {
        "rule": f"collections in order of sha256('{RESTRICTION_SALT}' + collection_id), each "
                f"taken unless it would carry the total past {RESTRICTED_CEILING:.0%} of all "
                f"cells, until the total reaches {RESTRICTED_TARGET:.0%}",
        "collections": len(chosen),
        "of_collections": len(cells),
        "cells": held,
        "of_cells": total,
        "share": round(held / total, 4),
        "by_species": {s: {"cells": by_species[s], "share": round(
            by_species[s] / sum(per_dataset[s].values()), 4)} for s in per_dataset},
        "terms": [{"term": term(c), "collection_id": c, "name": names[c], "cells": cells[c]}
                  for c in sorted(chosen, key=lambda c: -cells[c])],
    }
    return set(chosen), report


def tier_key(level: int, label: str) -> str:
    """A tier artifact's key. A label recurs at several tiers where a chain is short, and a tiered
    layer resolves a parent by key, so the key carries its tier and the label is the artifact's
    supplied name."""
    return f"t{level}:{label}"


def write_tier_names(out: Path, tiers: dict[str, tuple[str, ...]]) -> int:
    """`artifacts-cell-ontology.parquet`: each tier artifact's key, level and name."""
    pairs = sorted({(level, label) for keys in tiers.values() for level, label in enumerate(keys)})
    pq.write_table(pa.table({
        "level": pa.array([lv for lv, _ in pairs], pa.uint32()),
        "key": pa.array([tier_key(lv, lb) for lv, lb in pairs], pa.string()),
        "contents": pa.array([[[lb]] for _, lb in pairs], pa.list_(pa.list_(pa.string()))),
    }), out / "artifacts-cell-ontology.parquet")
    return len(pairs)


def term(collection_id: str) -> str:
    """A collection's access term. A UUID with this prefix is letters, digits, `-` and `:`, which
    an access expression takes unquoted."""
    return f"collection:{collection_id}"


# ------------------------------------------------------------------------------------ layout


def route(species: str, take: np.ndarray | None, primary: np.ndarray, managed: bool,
          t: dict) -> np.ndarray:
    """Positions for the rows `take` names (every row where it is None), fitted on a uniform
    sample of the primary cells among them."""
    vectors = sources.vectors(species)
    rows = np.arange(len(primary)) if take is None else take
    candidates = np.flatnonzero(primary[rows])
    rng = np.random.default_rng(SEED)
    fit_at = np.sort(rng.choice(candidates, min(FIT_ROWS[species], len(candidates)),
                                replace=False))
    t["fit_rows"], t["rows"] = len(fit_at), len(rows)
    print(f"  {species}: fitting on {len(fit_at):,} primary cells of {len(rows):,}", flush=True)
    with fit_place.step("read fit vectors", t):
        block = np.ascontiguousarray(vectors[rows[fit_at]])
    with fit_place.DeviceWatermark() as watch:
        fit_xy = fit_place.fit_layout(block, t, managed=managed, metric="euclidean")
    t["fit_peak_vram_gb"] = round(watch.peak_gb, 2)

    xy = np.empty((len(rows), 2), dtype=np.float32)
    with fit_place.DeviceWatermark() as watch, fit_place.step("place", t):
        placer = fit_place.Placer(block, fit_xy, metric="euclidean")
        for lo in range(0, len(rows), PLACE_CHUNK):
            hi = min(lo + PLACE_CHUNK, len(rows))
            chunk = vectors[lo:hi] if take is None else vectors[rows[lo:hi]]
            xy[lo:hi] = placer.place(np.ascontiguousarray(chunk))
            if (lo // PLACE_CHUNK) % 10 == 9:
                print(f"    placed {hi:,}/{len(rows):,}", flush=True)
        placer.close()
    t["place_peak_vram_gb"] = round(watch.peak_gb, 2)
    t["place_rows_per_second"] = round(len(rows) / t["place"])
    xy[fit_at] = fit_xy
    return xy


# ------------------------------------------------------------------------------------ writing


def encoded(column) -> pa.DictionaryArray:
    """A column as one dictionary array, encoding it where it is not one already."""
    col = column.combine_chunks() if isinstance(column, pa.ChunkedArray) else column
    return col if pa.types.is_dictionary(col.type) else pc.dictionary_encode(col)


def through(dictionary_column: pa.Array, mapping: dict) -> pa.Array:
    """`mapping` applied to a dictionary-encoded column, once per distinct value."""
    col = encoded(dictionary_column)
    values = col.dictionary.cast(pa.string()).to_pylist()
    mapped = pa.array([mapping.get(v) for v in values], pa.string())
    return mapped.take(col.indices)


def vocabulary_toml(sizes: dict[str, int]) -> str:
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


def write_declaration(out: Path, sizes: dict[str, int]) -> None:
    lines = (Path(__file__).parent / "corpus.toml").read_text().splitlines()
    hit = [i for i, line in enumerate(lines) if line.strip() == "# <vocabularies>"]
    assert len(hit) == 1, "corpus.toml must hold one `# <vocabularies>` line"
    lines[hit[0]] = vocabulary_toml(sizes)
    (out / "corpus.toml").write_text("\n".join(lines) + "\n")


def overview(out: Path, species: str, xy: np.ndarray, cls: np.ndarray, classes: list[str]) -> Path:
    """A picture of one species' layout, coloured by the top tier, from at most 3M cells."""
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    rng = np.random.default_rng(SEED)
    at = rng.permutation(len(xy))[:3_000_000]
    counts = np.bincount(cls, minlength=len(classes))
    order = np.argsort(-counts)
    # Saturated colours for the ten largest classes, their paler pairs for the rest.
    tab20 = plt.get_cmap("tab20")
    palette = [tab20(2 * i) for i in range(10)] + [tab20(2 * i + 1) for i in range(10)]
    colour = {c: palette[i % 20] for i, c in enumerate(order)}
    fig, ax = plt.subplots(figsize=(14, 12), dpi=110)
    rgba = np.array([colour[c] for c in range(len(classes))])[cls[at]]
    ax.scatter(xy[at, 0], xy[at, 1], s=0.08, c=rgba, linewidths=0, rasterized=True)
    for c in order:
        if counts[c]:
            ax.scatter([], [], s=30, color=colour[c], label=f"{classes[c]} ({counts[c]:,})")
    ax.legend(fontsize=8, loc="upper left", bbox_to_anchor=(1, 1), frameon=False)
    ax.set_title(f"{ORGANISM[species]}: {len(xy):,} cells, coloured by Cell Ontology class")
    ax.set_axis_off()
    fig.tight_layout()
    path = out / f"overview-{species}.png"
    fig.savefig(path)
    plt.close(fig)
    return path


# ------------------------------------------------------------------------------------ the run


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--sample", type=int, default=1_000_000,
                    help="cells to take per species, uniformly; 0 takes every cell")
    ap.add_argument("--out", type=Path, default=None,
                    help="default $MOSAICA_LADDER/cellxgene, or cellxgene-sample with --sample")
    ap.add_argument("--managed", action="store_true", help="RMM managed memory for the UMAP")
    ap.add_argument("--layout-only", action="store_true", help="stop once the layouts are written")
    ap.add_argument("--reuse-layout", action="store_true", help="read layout-<species>.npy")
    args = ap.parse_args()

    out = args.out or sources.rung(args.sample)
    out.mkdir(parents=True, exist_ok=True)
    steps = Steps()
    print(f"staging {sources.staging()}\noutput  {out}", flush=True)

    with steps.step("census summary"):
        datasets = dataset_table()
        per_dataset, census_types, primary = census_summary()
        restricted, restricted_report = restricted_collections(per_dataset, datasets)
        tiers, ontology_report = ontology.tiers(
            sources.staging() / f"cl-basic-{sources.CL_RELEASE}.json", census_types)
    print(f"restricted: {restricted_report['collections']} of "
          f"{restricted_report['of_collections']} collections, "
          f"{restricted_report['share']:.2%} of cells", flush=True)
    print(f"ontology: {ontology_report['keys_per_tier']} keys per tier, classes "
          f"{ontology_report['classes']}", flush=True)
    classes = ontology_report["classes"]
    class_index = {c: i for i, c in enumerate(classes)}

    rng = np.random.default_rng(SEED)
    takes: dict[str, np.ndarray | None] = {}
    for species, spec in sources.SPECIES.items():
        n = spec["rows"]
        takes[species] = (None if not args.sample or args.sample >= n
                          else np.sort(rng.choice(n, args.sample, replace=False)))

    routes: dict = {}
    layouts: dict[str, np.ndarray] = {}
    for species in sources.SPECIES:
        path = out / f"layout-{species}.npy"
        if args.reuse_layout:
            layouts[species] = np.load(path)
            continue
        with steps.step(f"layout {species}"):
            t: dict = {}
            layouts[species] = route(species, takes[species], primary[species], args.managed, t)
            routes[species] = t
        np.save(path, layouts[species])
        (out / f"layout-{species}.json").write_text(json.dumps(t, indent=2) + "\n")
    if args.layout_only:
        print(json.dumps(routes, indent=2))
        return
    del primary

    # One pass over each species' cell table: the attribute rows, the points and the members.
    values: dict[str, set] = {v[0]: set() for v in VOCABULARIES}
    collection_of = {d: r["collection_name"] for d, r in datasets.items()}
    title_of = {d: r["dataset_title"] for d, r in datasets.items()}
    access_of = {d: term(r["collection_id"]) for d, r in datasets.items()
                 if r["collection_id"] in restricted}
    tier_lists = {cid: [tier_key(level, k) for level, k in enumerate(keys)]
                  for cid, keys in tiers.items()}
    class_of_type = {cid: keys[0] for cid, keys in tiers.items()}

    cells = pq.ParquetWriter(out / "cells.parquet", CELLS_SCHEMA, compression="zstd")
    members = pq.ParquetWriter(out / "members-cell-ontology.parquet", MEMBER_SCHEMA,
                               compression="zstd")
    written: dict[str, int] = {}
    pictures = {}
    with steps.step("cells, points and members"):
        for species in sources.SPECIES:
            take, xy = takes[species], layouts[species]
            points = pq.ParquetWriter(out / f"points-{species}.parquet", POINTS_SCHEMA,
                                      compression="zstd")
            cls = np.empty(len(xy), dtype=np.int16)
            source = pq.ParquetFile(sources.obs_path(species))
            at, base = 0, 0
            for g in range(source.metadata.num_row_groups):
                rows_here = source.metadata.row_group(g).num_rows
                if take is None:
                    table = source.read_row_group(g)
                else:
                    a, b = np.searchsorted(take, [base, base + rows_here])
                    if a == b:
                        base += rows_here
                        continue
                    table = source.read_row_group(g).take(pa.array(take[a:b] - base))
                base += rows_here
                m = table.num_rows
                joinid = table.column("soma_joinid").to_numpy().astype(np.uint64)
                entity = joinid + np.uint64(entity_offset(species))
                dataset_id = table.column("dataset_id").combine_chunks()
                type_id = table.column("cell_type_ontology_term_id").combine_chunks()
                columns = {
                    "entity_id": pa.array(entity, pa.uint64()),
                    "soma_joinid": pa.array(joinid, pa.uint64()),
                    "organism": pa.DictionaryArray.from_arrays(
                        pa.array(np.zeros(m, dtype=np.int32)), pa.array([ORGANISM[species]])
                    ).cast(pa.string()),
                    **{c: table.column(c).combine_chunks().cast(pa.string())
                       for c in CATEGORY_COLUMNS},
                    "collection": through(dataset_id, collection_of),
                    "dataset": through(dataset_id, title_of),
                    "is_primary_data": table.column("is_primary_data").combine_chunks(),
                    "dataset_id": dataset_id.cast(pa.string()),
                    "donor_id": table.column("donor_id").combine_chunks().cast(pa.string()),
                    "nnz": table.column("nnz").combine_chunks().cast(pa.uint32()),
                    "raw_sum": table.column("raw_sum").combine_chunks().cast(pa.float32()),
                }
                cells.write_table(pa.table(columns, schema=CELLS_SCHEMA), row_group_size=m)
                for name, *_, column in VOCABULARIES:
                    values[name].update(v for v in pc.unique(columns[column]).to_pylist()
                                        if v is not None)

                codes = encoded(type_id)
                ids = codes.dictionary.cast(pa.string()).to_pylist()
                lists = pa.array([tier_lists[i] for i in ids], pa.list_(pa.string()))
                members.write_table(pa.table(
                    {"entity": pa.array(entity, pa.uint64()), "key": lists.take(codes.indices)},
                    schema=MEMBER_SCHEMA))
                per_id = np.array([class_index[class_of_type[i]] for i in ids], dtype=np.int16)
                cls[at : at + m] = per_id[codes.indices.to_numpy(zero_copy_only=False)]

                points.write_table(pa.table(
                    {"entity_id": pa.array(entity, pa.uint64()),
                     "x": pa.array(xy[at : at + m, 0].astype(np.float64)),
                     "y": pa.array(xy[at : at + m, 1].astype(np.float64)),
                     "access": through(dataset_id, access_of)},
                    schema=POINTS_SCHEMA), row_group_size=m)
                at += m
                del table, columns
            points.close()
            assert at == len(xy), f"{species}: wrote {at:,} points against {len(xy):,}"
            written[species] = at
            print(f"  {species}: {at:,} cells (peak {peak_gb():.1f} GB)", flush=True)
            pictures[species] = (xy, cls)
    cells.close()
    members.close()

    with steps.step("vocabularies"):
        sizes = {name: write_vocabulary(out, stem, sorted(values[name]))
                 for name, _, stem, *_ in VOCABULARIES}
    write_declaration(out, sizes)
    tier_artifacts = write_tier_names(out, tiers)
    write_deployment(out, generator="corpora/cellxgene/prepare.py",
                     credential_env="MOSAICA_CELLXGENE_OPERATOR_CRED")
    (out / "restricted-collections.json").write_text(
        json.dumps(restricted_report, indent=2) + "\n")

    with steps.step("overview pictures"):
        for species, (xy, cls) in pictures.items():
            print(f"  {overview(out, species, xy, cls, classes)}", flush=True)

    manifest = {
        "census": sources.VINTAGE,
        "cell_ontology": sources.CL_RELEASE,
        "sample": args.sample,
        "cells": written,
        "seed": SEED,
        "umap": dict(fit_place.UMAP_PARAMS, seed=fit_place.SEED, metric="euclidean",
                     graph="cagra", fit_rows=FIT_ROWS, reused_layout=args.reuse_layout),
        "routes": routes,
        "bounds": {s: {"x": [float(xy[:, 0].min()), float(xy[:, 0].max())],
                       "y": [float(xy[:, 1].min()), float(xy[:, 1].max())]}
                   for s, xy in layouts.items()},
        "restricted": {k: v for k, v in restricted_report.items() if k != "terms"},
        "ontology": dict(ontology_report, artifacts_declared=tier_artifacts),
        "vocabularies": sizes,
        "seconds": dict(steps),
        "total_seconds": steps.total(),
        "peak_rss_gb": round(peak_gb(), 2),
    }
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"\nwrote to {out}:")
    for f in sorted(out.iterdir()):
        if f.is_file():
            print(f"  {f.name:40} {f.stat().st_size / 1e6:10.2f} MB")


if __name__ == "__main__":
    sys.exit(main())
