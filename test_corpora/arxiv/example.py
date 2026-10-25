"""The arXiv example dataset in `examples/data/arxiv/`: 50,000 papers cut from the notebook corpus.

It is small enough to keep in git. It keeps two of the source's clusterings: Toponymy's four
levels with the names a language model gave them, and arXiv's own classification of archives over
subject classes. Three things differ from a plain sample.

**No abstracts.** arXiv's metadata is CC0, but an abstract may be its authors' copyright, so the
points file keeps the title and drops the abstract.

**Every Toponymy exemplar is in the sample.** A Toponymy name was written by a language model from
six exemplar papers' titles, and a reader is shown it only where they can see all six. The
exemplars are taken first and the rest of the sample is drawn uniformly, so every name ships with
its whole generating set. That puts 4,284 papers near cluster centres into the sample on top of
the uniform draw.

**A second layout of the same papers**, `points-umap200.parquet`, for a view of its own. It is
laid out from the papers' BGE embeddings (`data/arxiv_papers_embeds.parquet`) by PCA to 64
components and then CPU `umap-learn`, seeded, with 200 neighbours and a minimum distance of 0.9.
The source layout used 15 and 0.1, so this one keeps subjects in broad contiguous regions where the
other breaks them into many small islands. It carries each paper's categories, which a view reads
its access terms from. It needs `umap-learn` and about five minutes, most of it streaming the
4.7 GB embedding file.

Every member file is restricted to the sampled papers, and a cluster left with no member is dropped
with any label attached to it. Positions are stored as `float32`. The archive and subject-class
vocabularies are copied whole, each code titled with arXiv's name for it from `NAMES`.

```bash
python -m test_corpora.arxiv.example --source data/notebook-2m4-live --out examples/data/arxiv \
    --papers 50000 --seed 0
```
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.parquet as pq

from .sources import DATA, load_embeddings, rows_for_ids

#: The second layout's projection, chosen to differ from the source layout's 15 and 0.1.
SECOND_LAYOUT = {"pca": 64, "n_neighbors": 200, "min_dist": 0.9}

POINT_COLUMNS = [
    "entity_id", "x", "y", "categories", "arxiv_id", "archive", "primary_category",
    "submitted_at", "title",
]
#: The clusterings kept, by file stem. Their artifact files carry no text, so they are filtered.
CLUSTERINGS = ["clusters-toponymy", "taxonomy-arxiv"]
#: The label kept as written, every one of whose generating papers is in the sample.
KEPT_LABELS = {"topics-toponymy": "clusters-toponymy"}
VOCABULARIES = ["archive.parquet", "primary_category.parquet"]
#: arXiv's name for each archive and subject class, as https://arxiv.org/category_taxonomy gives it,
#: and for the retired archives that page leaves out, as arXiv's arxiv-base code
#: (`arxiv/taxonomy/definitions.py`) gives it. A key named in neither keeps its code as title.
NAMES = {
    "acc-phys": "Accelerator Physics",
    "adap-org": "Adaptation, Noise, and Self-Organizing Systems",
    "alg-geom": "Algebraic Geometry",
    "ao-sci": "Atmospheric-Oceanic Sciences",
    "atom-ph": "Atomic, Molecular and Optical Physics",
    "bayes-an": "Bayesian Analysis",
    "chao-dyn": "Chaotic Dynamics",
    "chem-ph": "Chemical Physics",
    "cmp-lg": "Computation and Language",
    "comp-gas": "Cellular Automata and Lattice Gases",
    "dg-ga": "Differential Geometry",
    "funct-an": "Functional Analysis",
    "mtrl-th": "Materials Theory",
    "patt-sol": "Pattern Formation and Solitons",
    "plasm-ph": "Plasma Physics",
    "q-alg": "Quantum Algebra and Topology",
    "solv-int": "Exactly Solvable and Integrable Systems",
    "supr-con": "Superconductivity",
    "cs": "Computer Science",
    "cs.AI": "Artificial Intelligence",
    "cs.AR": "Hardware Architecture",
    "cs.CC": "Computational Complexity",
    "cs.CE": "Computational Engineering, Finance, and Science",
    "cs.CG": "Computational Geometry",
    "cs.CL": "Computation and Language",
    "cs.CR": "Cryptography and Security",
    "cs.CV": "Computer Vision and Pattern Recognition",
    "cs.CY": "Computers and Society",
    "cs.DB": "Databases",
    "cs.DC": "Distributed, Parallel, and Cluster Computing",
    "cs.DL": "Digital Libraries",
    "cs.DM": "Discrete Mathematics",
    "cs.DS": "Data Structures and Algorithms",
    "cs.ET": "Emerging Technologies",
    "cs.FL": "Formal Languages and Automata Theory",
    "cs.GL": "General Literature",
    "cs.GR": "Graphics",
    "cs.GT": "Computer Science and Game Theory",
    "cs.HC": "Human-Computer Interaction",
    "cs.IR": "Information Retrieval",
    "cs.IT": "Information Theory",
    "cs.LG": "Machine Learning",
    "cs.LO": "Logic in Computer Science",
    "cs.MA": "Multiagent Systems",
    "cs.MM": "Multimedia",
    "cs.MS": "Mathematical Software",
    "cs.NA": "Numerical Analysis",
    "cs.NE": "Neural and Evolutionary Computing",
    "cs.NI": "Networking and Internet Architecture",
    "cs.OH": "Other Computer Science",
    "cs.OS": "Operating Systems",
    "cs.PF": "Performance",
    "cs.PL": "Programming Languages",
    "cs.RO": "Robotics",
    "cs.SC": "Symbolic Computation",
    "cs.SD": "Sound",
    "cs.SE": "Software Engineering",
    "cs.SI": "Social and Information Networks",
    "cs.SY": "Systems and Control",
    "econ": "Economics",
    "econ.EM": "Econometrics",
    "econ.GN": "General Economics",
    "econ.TH": "Theoretical Economics",
    "eess": "Electrical Engineering and Systems Science",
    "eess.AS": "Audio and Speech Processing",
    "eess.IV": "Image and Video Processing",
    "eess.SP": "Signal Processing",
    "eess.SY": "Systems and Control",
    "math": "Mathematics",
    "math.AC": "Commutative Algebra",
    "math.AG": "Algebraic Geometry",
    "math.AP": "Analysis of PDEs",
    "math.AT": "Algebraic Topology",
    "math.CA": "Classical Analysis and ODEs",
    "math.CO": "Combinatorics",
    "math.CT": "Category Theory",
    "math.CV": "Complex Variables",
    "math.DG": "Differential Geometry",
    "math.DS": "Dynamical Systems",
    "math.FA": "Functional Analysis",
    "math.GM": "General Mathematics",
    "math.GN": "General Topology",
    "math.GR": "Group Theory",
    "math.GT": "Geometric Topology",
    "math.HO": "History and Overview",
    "math.IT": "Information Theory",
    "math.KT": "K-Theory and Homology",
    "math.LO": "Logic",
    "math.MG": "Metric Geometry",
    "math.MP": "Mathematical Physics",
    "math.NA": "Numerical Analysis",
    "math.NT": "Number Theory",
    "math.OA": "Operator Algebras",
    "math.OC": "Optimization and Control",
    "math.PR": "Probability",
    "math.QA": "Quantum Algebra",
    "math.RA": "Rings and Algebras",
    "math.RT": "Representation Theory",
    "math.SG": "Symplectic Geometry",
    "math.SP": "Spectral Theory",
    "math.ST": "Statistics Theory",
    "physics": "Physics",
    "astro-ph": "Astrophysics",
    "astro-ph.CO": "Cosmology and Nongalactic Astrophysics",
    "astro-ph.EP": "Earth and Planetary Astrophysics",
    "astro-ph.GA": "Astrophysics of Galaxies",
    "astro-ph.HE": "High Energy Astrophysical Phenomena",
    "astro-ph.IM": "Instrumentation and Methods for Astrophysics",
    "astro-ph.SR": "Solar and Stellar Astrophysics",
    "cond-mat": "Condensed Matter",
    "cond-mat.dis-nn": "Disordered Systems and Neural Networks",
    "cond-mat.mes-hall": "Mesoscale and Nanoscale Physics",
    "cond-mat.mtrl-sci": "Materials Science",
    "cond-mat.other": "Other Condensed Matter",
    "cond-mat.quant-gas": "Quantum Gases",
    "cond-mat.soft": "Soft Condensed Matter",
    "cond-mat.stat-mech": "Statistical Mechanics",
    "cond-mat.str-el": "Strongly Correlated Electrons",
    "cond-mat.supr-con": "Superconductivity",
    "gr-qc": "General Relativity and Quantum Cosmology",
    "hep-ex": "High Energy Physics - Experiment",
    "hep-lat": "High Energy Physics - Lattice",
    "hep-ph": "High Energy Physics - Phenomenology",
    "hep-th": "High Energy Physics - Theory",
    "math-ph": "Mathematical Physics",
    "nlin": "Nonlinear Sciences",
    "nlin.AO": "Adaptation and Self-Organizing Systems",
    "nlin.CD": "Chaotic Dynamics",
    "nlin.CG": "Cellular Automata and Lattice Gases",
    "nlin.PS": "Pattern Formation and Solitons",
    "nlin.SI": "Exactly Solvable and Integrable Systems",
    "nucl-ex": "Nuclear Experiment",
    "nucl-th": "Nuclear Theory",
    "physics.acc-ph": "Accelerator Physics",
    "physics.ao-ph": "Atmospheric and Oceanic Physics",
    "physics.app-ph": "Applied Physics",
    "physics.atm-clus": "Atomic and Molecular Clusters",
    "physics.atom-ph": "Atomic Physics",
    "physics.bio-ph": "Biological Physics",
    "physics.chem-ph": "Chemical Physics",
    "physics.class-ph": "Classical Physics",
    "physics.comp-ph": "Computational Physics",
    "physics.data-an": "Data Analysis, Statistics and Probability",
    "physics.ed-ph": "Physics Education",
    "physics.flu-dyn": "Fluid Dynamics",
    "physics.gen-ph": "General Physics",
    "physics.geo-ph": "Geophysics",
    "physics.hist-ph": "History and Philosophy of Physics",
    "physics.ins-det": "Instrumentation and Detectors",
    "physics.med-ph": "Medical Physics",
    "physics.optics": "Optics",
    "physics.plasm-ph": "Plasma Physics",
    "physics.pop-ph": "Popular Physics",
    "physics.soc-ph": "Physics and Society",
    "physics.space-ph": "Space Physics",
    "quant-ph": "Quantum Physics",
    "q-bio": "Quantitative Biology",
    "q-bio.BM": "Biomolecules",
    "q-bio.CB": "Cell Behavior",
    "q-bio.GN": "Genomics",
    "q-bio.MN": "Molecular Networks",
    "q-bio.NC": "Neurons and Cognition",
    "q-bio.OT": "Other Quantitative Biology",
    "q-bio.PE": "Populations and Evolution",
    "q-bio.QM": "Quantitative Methods",
    "q-bio.SC": "Subcellular Processes",
    "q-bio.TO": "Tissues and Organs",
    "q-fin": "Quantitative Finance",
    "q-fin.CP": "Computational Finance",
    "q-fin.EC": "Economics",
    "q-fin.GN": "General Finance",
    "q-fin.MF": "Mathematical Finance",
    "q-fin.PM": "Portfolio Management",
    "q-fin.PR": "Pricing of Securities",
    "q-fin.RM": "Risk Management",
    "q-fin.ST": "Statistical Finance",
    "q-fin.TR": "Trading and Market Microstructure",
    "stat": "Statistics",
    "stat.AP": "Applications",
    "stat.CO": "Computation",
    "stat.ME": "Methodology",
    "stat.ML": "Machine Learning",
    "stat.OT": "Other Statistics",
    "stat.TH": "Statistics Theory",
}


def write(table: pa.Table, path: Path) -> None:
    pq.write_table(table, path, compression="zstd", compression_level=19)


def sampled_ids(source: Path, papers: int, seed: int) -> tuple[pa.Array, int]:
    """Every Toponymy exemplar, then a uniform draw from the other papers up to `papers`."""
    ids = pq.read_table(source / "points.parquet", columns=["entity_id"])["entity_id"].to_numpy()
    exemplars = np.unique(
        pq.read_table(source / "topics-toponymy-members.parquet", columns=["entity"])["entity"]
        .to_numpy()
    )
    rest = np.setdiff1d(ids, exemplars, assume_unique=True)
    if papers < len(exemplars):
        raise SystemExit(f"--papers {papers} is fewer than the {len(exemplars):,} exemplars; "
                         f"ask for at least that many")
    drawn = np.random.default_rng(seed).choice(rest, size=papers - len(exemplars), replace=False)
    return pa.array(np.sort(np.concatenate([exemplars, drawn])), pa.uint64()), len(exemplars)


def members_of(source: Path, stem: str, keep: pa.Array) -> pa.Table:
    table = pq.read_table(source / f"{stem}-members.parquet")
    return table.filter(pc.is_in(table["entity"], value_set=keep))


def held_artifacts(source: Path, stem: str, members: pa.Table) -> pa.Table:
    """The artifacts of `stem` with at least one member left; a parent that was dropped is
    cleared on its children, which the sample cannot otherwise leave."""
    table = pq.read_table(source / f"{stem}.parquet")
    held = pc.unique(members["key"])
    kept = table.filter(pc.is_in(table["key"], value_set=held))
    orphaned = pc.and_(pc.is_valid(kept["parent"]),
                       pc.invert(pc.is_in(kept["parent"], value_set=kept["key"])))
    if pc.any(orphaned).as_py():
        parent = pc.if_else(orphaned, pa.scalar(None, pa.string()), kept["parent"])
        kept = kept.set_column(kept.schema.get_field_index("parent"), "parent", parent)
    print(f"  {stem}: {kept.num_rows:,} of {table.num_rows:,} clusters hold a sampled paper; "
          f"{pc.sum(orphaned).as_py() or 0} lost their parent")
    return kept


def second_layout(points: pa.Table, data: Path, seed: int) -> pa.Table:
    """The sampled papers laid out again from their embeddings, with their categories."""
    from sklearn.decomposition import PCA
    from umap import UMAP

    corpus = pq.read_table(data / "corpus.parquet", columns=["entity_id", "id"])
    take = rows_for_ids(corpus, points["arxiv_id"].to_pylist())
    embeddings = load_embeddings(corpus, take, data)
    reduced = PCA(SECOND_LAYOUT["pca"], random_state=seed).fit_transform(embeddings)
    xy = UMAP(n_neighbors=SECOND_LAYOUT["n_neighbors"], min_dist=SECOND_LAYOUT["min_dist"],
              random_state=seed).fit_transform(reduced)
    return pa.table({"entity_id": points["entity_id"],
                     "x": pa.array(xy[:, 0], pa.float32()), "y": pa.array(xy[:, 1], pa.float32()),
                     "categories": points["categories"]})


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--source", type=Path, default=DATA / "notebook-2m4-live")
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--papers", type=int, default=50_000)
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--data", type=Path, default=DATA,
                    help="the directory holding corpus.parquet and arxiv_papers_embeds.parquet")
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)

    keep, exemplars = sampled_ids(args.source, args.papers, args.seed)
    print(f"{len(keep):,} papers: {exemplars:,} Toponymy exemplars and "
          f"{len(keep) - exemplars:,} drawn uniformly with seed {args.seed}")

    points = pq.read_table(args.source / "points.parquet", columns=POINT_COLUMNS,
                           filters=pc.is_in(pc.field("entity_id"), value_set=keep))
    points = points.sort_by("entity_id")
    for axis in ("x", "y"):
        points = points.set_column(points.schema.get_field_index(axis), axis,
                                   pc.cast(points[axis], pa.float32()))
    write(points, args.out / "points.parquet")

    clusters = {}
    for stem in CLUSTERINGS:
        members = members_of(args.source, stem, keep)
        clusters[stem] = held_artifacts(args.source, stem, members)
        write(clusters[stem], args.out / f"{stem}.parquet")
        write(members, args.out / f"{stem}-members.parquet")

    for stem, clustering in KEPT_LABELS.items():
        members = pq.read_table(args.source / f"{stem}-members.parquet")
        missing = pc.sum(pc.invert(pc.is_in(members["entity"], value_set=keep))).as_py()
        assert missing == 0, f"{missing} of {stem}'s generating papers are not in the sample"
        labels = pq.read_table(args.source / f"{stem}.parquet")
        assert pc.all(pc.is_in(labels["attached_key"], value_set=clusters[clustering]["key"])).as_py()
        write(labels, args.out / f"{stem}.parquet")
        write(members, args.out / f"{stem}-members.parquet")
        print(f"  {stem}: {labels.num_rows} names kept, every generating paper sampled")

    for name in VOCABULARIES:
        vocabulary = pq.read_table(args.source / name)
        keys = vocabulary["key"].to_pylist()
        titles = pa.array([NAMES.get(key, key) for key in keys], pa.string())
        write(vocabulary.set_column(vocabulary.schema.get_field_index("title"), "title", titles),
              args.out / name)
        unnamed = [key for key in keys if key not in NAMES]
        print(f"  {name}: {len(keys) - len(unnamed)} of {len(keys)} keys named"
              + (f"; titled by their code: {', '.join(unnamed)}" if unnamed else ""))

    write(second_layout(points, args.data, args.seed), args.out / "points-umap200.parquet")

    rows = {p.name: pq.ParquetFile(p).metadata.num_rows for p in sorted(args.out.glob("*.parquet"))}
    manifest = {
        "cut_from": args.source.name,
        "papers": len(keep),
        "toponymy_exemplars": exemplars,
        "seed": args.seed,
        "second_layout": {"file": "points-umap200.parquet", "embeddings": "BGE-large-en-v1.5",
                          **SECOND_LAYOUT},
        "rows": rows,
    }
    (args.out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    total = sum(p.stat().st_size for p in args.out.glob("*.parquet"))
    for name, n in rows.items():
        print(f"  {name:<36} {n:>9,} rows  {(args.out / name).stat().st_size / 1e6:6.2f} MB")
    print(f"{total / 1e6:.2f} MB of Parquet in {args.out}")


if __name__ == "__main__":
    main()
