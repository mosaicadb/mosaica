"""The notebook corpus: the arXiv papers the clients' live tests serve, drawn from the example data.

It holds one view, `s0`, the arXiv archive and subject-class vocabularies, six columns, three
clusterings and a set of titles over two of them; `declare_notebook` in the Python SDK's tests
declares the same through the SDK's calls. `schema.toml` beside this file is its declaration, and is copied into every directory this
writes.

Everything is derived from `examples/data/arxiv/`, which is in git, and from a seed, so the same
seed and size write the same papers, clusters and titles on any machine:

- the papers are a seeded sample of the example's, numbered from 0, with their positions,
  categories, archive, subject class, submission time and title; each abstract is written from the
  paper's own title and categories, the example carrying none;
- the archive and subject-class vocabularies, and arXiv's classification of archives over subject
  classes, are the example's, restricted to the sampled papers and carrying no content;
- `clusters/kmeans` is a seeded k-means over the positions, and `clusters/hdbscan` a tree of the
  same: a k-means of the corpus, and one inside each of its clusters, each cluster leaving out the
  members farthest from its centre, so the tree covers some of the papers and a child holds only
  papers its parent holds;
- each cluster of both has a title in `topics/kmeans` or `topics/hdbscan`, written from the subject
  classes of the papers nearest its centre, which are its generating set.

`cached()` writes the corpus once into the system's temporary directory, under a name that changes
with this file, the declaration, the example data, the size and the seed, and returns that
directory. Writing it needs `numpy` and `pyarrow`.
"""

from __future__ import annotations

import collections
import hashlib
import os
import shutil
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
EXAMPLE = HERE.parents[1] / "examples" / "data" / "arxiv"
SCHEMA = HERE / "schema.toml"

#: The size the live tests are written against: enough papers for a session holding three subject
#: classes to see a thousand of them in one archive, and few enough to build in about a second.
PAPERS = 12_000
SEED = 0
KMEANS = 12
HDBSCAN_ROOTS = 4
HDBSCAN_CHILDREN = 3
#: The share of a cluster's members, nearest its centre first, that an HDBSCAN cluster keeps.
HDBSCAN_KEPT = 0.85
#: The papers nearest a cluster's centre that its title is written from.
EXEMPLARS = 6


def inputs() -> list[Path]:
    """Every file the corpus is written from."""
    return [Path(__file__).resolve(), SCHEMA] + [
        EXAMPLE / name
        for name in (
            "points.parquet",
            "archive.parquet",
            "primary_category.parquet",
            "taxonomy-arxiv.parquet",
            "taxonomy-arxiv-members.parquet",
        )
    ]


def cached(papers: int = PAPERS, seed: int = SEED) -> Path:
    """The corpus in the temporary directory, written there first where it is not."""
    digest = hashlib.sha256(f"{papers}/{seed}".encode())
    for path in inputs():
        digest.update(path.read_bytes())
    target = Path(tempfile.gettempdir()) / f"mosaica-notebook-{digest.hexdigest()[:16]}"
    if (target / "schema.toml").exists():
        return target
    staging = Path(tempfile.mkdtemp(prefix=target.name + ".", dir=target.parent))
    try:
        write(staging, papers=papers, seed=seed)
        os.replace(staging, target)
    except OSError:
        # Another process wrote the same corpus first.
        if not (target / "schema.toml").exists():
            raise
    finally:
        shutil.rmtree(staging, ignore_errors=True)
    return target


def write(out: Path, *, papers: int = PAPERS, seed: int = SEED) -> None:
    """Write the corpus into `out`, which must exist."""
    import numpy as np
    import pyarrow as pa
    import pyarrow.compute as pc
    import pyarrow.parquet as pq

    rng = np.random.default_rng(seed)
    source = pq.read_table(EXAMPLE / "points.parquet")
    if papers > source.num_rows:
        raise ValueError(f"the example data holds {source.num_rows} papers; ask for at most that")
    chosen = np.sort(rng.choice(source.num_rows, size=papers, replace=False))
    points = source.take(pa.array(chosen))
    # Each paper is numbered by its place in the sample, so the ids are 0 to `papers - 1`.
    ids = np.arange(papers, dtype=np.uint64)
    renumbered = dict(zip(points.column("entity_id").to_pylist(), ids.tolist()))
    xy = np.column_stack([points.column(axis).to_numpy().astype(np.float64) for axis in "xy"])

    subjects = pq.read_table(EXAMPLE / "primary_category.parquet", columns=["key", "title"])
    subject_titles = dict(zip(*subjects.to_pydict().values()))
    primary = points.column("primary_category").to_pylist()
    abstracts = [
        f"{title}. We study this question in {subject_titles.get(category, category)}, "
        f"and relate it to {', '.join(categories)}."
        for title, category, categories in zip(
            points.column("title").to_pylist(), primary, points.column("categories").to_pylist()
        )
    ]
    pq.write_table(
        pa.table(
            {
                "entity_id": pa.array(ids, pa.uint64()),
                "x": pa.array(xy[:, 0], pa.float64()),
                "y": pa.array(xy[:, 1], pa.float64()),
                "categories": points.column("categories"),
                "arxiv_id": points.column("arxiv_id"),
                "archive": points.column("archive"),
                "primary_category": points.column("primary_category"),
                "submitted_at": points.column("submitted_at"),
                "title": points.column("title"),
                "abstract": pa.array(abstracts, pa.string()),
            }
        ),
        out / "points.parquet",
    )
    for name in ("archive.parquet", "primary_category.parquet"):
        shutil.copyfile(EXAMPLE / name, out / name)
    shutil.copyfile(SCHEMA, out / "schema.toml")

    layers = Layers()

    # arXiv's classification, over the sampled papers only.
    members = pq.read_table(EXAMPLE / "taxonomy-arxiv-members.parquet")
    members = members.filter(
        pc.is_in(members.column("entity"), value_set=points.column("entity_id").combine_chunks())
    )
    held = collections.defaultdict(list)
    for level, key, entity in zip(*members.to_pydict().values()):
        held[(level, key)].append(renumbered[entity])
    taxonomy = pq.read_table(EXAMPLE / "taxonomy-arxiv.parquet", columns=["level", "key", "parent"])
    for level, key, parent in zip(*taxonomy.to_pydict().values()):
        if held.get((level, key)):
            layers.artifact("taxonomy-arxiv", key, level=level, parent=parent)
            layers.members("taxonomy-arxiv", key, held[(level, key)], level=level)

    def titled(clusters: str, labels: str, key: str, rows: np.ndarray, centre: np.ndarray) -> None:
        distance = ((xy[rows] - centre) ** 2).sum(axis=1)
        nearest = rows[np.argsort(distance, kind="stable")[:EXEMPLARS]]
        common = collections.Counter(primary[i] for i in rows).most_common(2)
        names = [subject_titles.get(c, c) for c, _ in common]
        label = f"t{key}"
        layers.artifact(
            labels, label, contents=[[" and ".join(names)], [names[0]]], attached=(clusters, key)
        )
        layers.members(labels, label, nearest)
        layers.members(labels, label, nearest, rank=0)
        layers.members(labels, label, nearest[: len(nearest) // 3], rank=1)

    every = np.arange(papers)
    assigned, centres = kmeans(xy, every, KMEANS, rng)
    for k, centre in enumerate(centres):
        rows = every[assigned == k]
        key = f"km-{k:02d}"
        layers.artifact("clusters-kmeans", key)
        layers.members("clusters-kmeans", key, rows)
        titled("clusters/kmeans", "topics-kmeans", key, rows, centre)

    assigned, centres = kmeans(xy, every, HDBSCAN_ROOTS, rng)
    for r, centre in enumerate(centres):
        root = f"hd-{r}"
        rows = kept(xy, every[assigned == r], centre)
        layers.artifact("clusters-hdbscan", root)
        layers.members("clusters-hdbscan", root, rows)
        titled("clusters/hdbscan", "topics-hdbscan", root, rows, centre)
        inner, inner_centres = kmeans(xy, rows, HDBSCAN_CHILDREN, rng)
        for c, inner_centre in enumerate(inner_centres):
            child = f"{root}-{c}"
            child_rows = kept(xy, rows[inner == c], inner_centre)
            layers.artifact("clusters-hdbscan", child, parent=root)
            layers.members("clusters-hdbscan", child, child_rows)
            titled("clusters/hdbscan", "topics-hdbscan", child, child_rows, inner_centre)

    layers.write(out)


def kmeans(xy, rows, k: int, rng, rounds: int = 25):
    """Lloyd's k-means of the points `rows`, from `k` of them drawn as centres: each row's cluster,
    and the centres."""
    import numpy as np

    at = xy[rows]
    centres = at[rng.choice(len(rows), size=k, replace=False)]
    for _ in range(rounds):
        assigned = ((at[:, None, :] - centres[None, :, :]) ** 2).sum(axis=2).argmin(axis=1)
        centres = np.array(
            [at[assigned == c].mean(axis=0) if (assigned == c).any() else centres[c]
             for c in range(k)]
        )
    return assigned, centres


def kept(xy, rows, centre):
    """The share `HDBSCAN_KEPT` of `rows` nearest `centre`, in row order."""
    import numpy as np

    order = np.argsort(((xy[rows] - centre) ** 2).sum(axis=1), kind="stable")
    return np.sort(rows[order[: max(1, int(len(rows) * HDBSCAN_KEPT))]])


class Layers:
    """Artifact and member rows, one artifact file and one member file per layer.

    An artifact file carries `level`, `key`, `parent`, `contents`, `attached_layer` and
    `attached_key`; a member file `level`, `key`, `rank` and `entity`, the paper's `entity_id`. A
    null rank is the artifact's own membership, and rank `k` the generating set of its `k`-th
    content.
    """

    def __init__(self):
        self.artifacts = collections.defaultdict(list)
        self.member_rows = collections.defaultdict(list)

    def artifact(self, layer, key, *, level=0, parent=None, contents=None, attached=None):
        self.artifacts[layer].append(
            {
                "level": level,
                "key": key,
                "parent": [parent] if parent else None,
                "contents": contents,
                "attached_layer": attached[0] if attached else None,
                "attached_key": attached[1] if attached else None,
            }
        )

    def members(self, layer, key, entities, *, level=0, rank=None):
        self.member_rows[layer].extend((level, key, rank, int(e)) for e in entities)

    def write(self, out: Path) -> None:
        import pyarrow as pa
        import pyarrow.parquet as pq

        types = {
            "level": pa.uint32(),
            "key": pa.string(),
            "parent": pa.list_(pa.string()),
            "contents": pa.list_(pa.list_(pa.string())),
            "attached_layer": pa.string(),
            "attached_key": pa.string(),
        }
        for layer, rows in self.artifacts.items():
            pq.write_table(
                pa.table({name: pa.array([r[name] for r in rows], t) for name, t in types.items()}),
                out / f"{layer}.parquet",
            )
            level, key, rank, entity = zip(*self.member_rows[layer])
            pq.write_table(
                pa.table(
                    {
                        "level": pa.array(level, pa.uint32()),
                        "key": pa.array(key, pa.string()),
                        "rank": pa.array(rank, pa.uint32()),
                        "entity": pa.array(entity, pa.uint64()),
                    }
                ),
                out / f"{layer}-members.parquet",
            )
