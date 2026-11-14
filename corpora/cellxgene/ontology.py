"""The Cell Ontology cut into four tiers, one path per cell type.

The Cell Ontology is a DAG in which a cell type often has several `is_a` parents, and a tiered
layer needs one path per item. The path is chosen in three steps, from the ontology and the
Census's own list of cell types alone, so it does not depend on which cells a run samples.

1. **The class and the lineage.** `CLASSES` is an ordered list of broad cell types. A cell
   type's class is the first entry in that list that is the cell type or one of its ancestors. A
   cell type with none of them is in `OTHER`, and the Census's `unknown` is a class of its own.
   Its lineage is the first entry of `LINEAGES` that is the cell type or an ancestor of it and a
   descendant of its class, where there is one.
2. **One parent.** Among a cell type's `is_a` parents that share its class and lineage, the
   parent is the one with the most Census cell types at or beneath it, and on a tie the lower CL
   id. Following parents from any cell type reaches its lineage, or its class where it has none,
   so every cell type has one chain from its class down to itself.
3. **Four tiers.** On a chain `class = c0, c1, ..., cd = the cell type`, the tiers are `c0`, `c1`,
   `c(d-1)` and `cd`, each repeating the one before where the chain is shorter. Tier 1 is the
   broad lineage under the class, tier 2 the cell type's own parent, and tier 3 the cell type.

A tier's key is the cell type's label as the Census writes it. Labels are unique in the ontology,
so a key names one cell type, and a cell type takes one key at each tier where it appears.
"""

from __future__ import annotations

import json
from pathlib import Path

#: The classes, in the order a cell type that descends from several is assigned to them. Leukocyte
#: comes before hematopoietic cell so that the latter holds blood cells other than immune cells,
#: and endothelial cell before epithelial cell, of which it is a subtype in some branches.
CLASSES = [
    "CL:0000738",  # leukocyte
    "CL:0000988",  # hematopoietic cell
    "CL:0000115",  # endothelial cell
    "CL:0000066",  # epithelial cell
    "CL:0002319",  # neural cell
    "CL:0000187",  # muscle cell
    "CL:0000586",  # germ cell
    "CL:0000147",  # pigment cell
    "CL:0002320",  # connective tissue cell
    "CL:0000499",  # stromal cell
    "CL:0000034",  # stem cell
    "CL:0011115",  # precursor cell
    "CL:0001061",  # abnormal cell
    "CL:0000349",  # extraembryonic cell
    "CL:0000222",  # mesodermal cell
]

#: Lineages that stand at tier 1 wherever they fall inside a cell type's class, in the same
#: priority order. Without them the largest parents under leukocyte are groupings such as
#: "mononuclear leukocyte" and "professional antigen presenting cell", which split T cells from B
#: cells on no lineage at all.
LINEAGES = [
    "CL:0000542",  # lymphocyte
    "CL:0000766",  # myeloid leukocyte
    "CL:0000540",  # neuron
    "CL:0000125",  # glial cell
]

OTHER = "other cell"
UNKNOWN = "unknown"

TIERS = 4


def _curie(iri: str) -> str:
    return iri.rsplit("/", 1)[-1].replace("_", ":", 1)


def load(path: Path) -> tuple[dict[str, str], dict[str, list[str]]]:
    """`(label by id, is_a parents by id)` over the ontology's live CL terms."""
    graph = json.loads(path.read_text())["graphs"][0]
    label: dict[str, str] = {}
    for node in graph["nodes"]:
        cid = _curie(node["id"])
        if cid.startswith("CL:") and node.get("lbl") and not node.get("meta", {}).get("deprecated"):
            label[cid] = node["lbl"]
    parents: dict[str, list[str]] = {c: [] for c in label}
    for edge in graph["edges"]:
        if edge["pred"] != "is_a":
            continue
        sub, obj = _curie(edge["sub"]), _curie(edge["obj"])
        if sub in label and obj in label:
            parents[sub].append(obj)
    return label, parents


def ancestors(parents: dict[str, list[str]]) -> dict[str, frozenset[str]]:
    """Each term's ancestors, itself included."""
    memo: dict[str, frozenset[str]] = {}

    def walk(c: str) -> frozenset[str]:
        if c not in memo:
            got = {c}
            for p in parents.get(c, ()):
                got |= walk(p)
            memo[c] = frozenset(got)
        return memo[c]

    for c in parents:
        walk(c)
    return memo


def tiers(path: Path, census_types: dict[str, str]) -> tuple[dict[str, tuple[str, ...]], dict]:
    """Each Census cell type's four tier keys, by ontology id, and a report.

    `census_types` maps each ontology id the Census uses to the label it writes. An id the
    ontology does not hold is placed in `OTHER` at every tier but the last, and the report names
    it.
    """
    label, parents = load(path)
    above = ancestors(parents)

    def class_of(c: str) -> str | None:
        return next((a for a in CLASSES if a in above[c]), None)

    def lineage_of(c: str, cls: str) -> str | None:
        return next((a for a in LINEAGES if a in above[c] and cls in above[a]), None)

    # How many Census cell types sit at or beneath each term.
    weight: dict[str, int] = {}
    for cid in census_types:
        for a in above.get(cid, ()):
            weight[a] = weight.get(a, 0) + 1

    chain: dict[str, list[str]] = {}

    def chain_of(c: str, top: str) -> list[str]:
        """From `top`, the cell type's lineage or else its class, down to the cell type."""
        if c == top:
            return [c]
        if c not in chain:
            same = [p for p in parents[c] if top in above[p]]
            parent = min(same, key=lambda p: (-weight.get(p, 0), p))
            chain[c] = chain_of(parent, top) + [c]
        return chain[c]

    def name(c: str) -> str:
        return census_types.get(c) or label[c]

    out: dict[str, tuple[str, ...]] = {}
    unplaced: list[str] = []
    other: list[str] = []
    for cid, own in census_types.items():
        if cid == UNKNOWN:
            out[cid] = (UNKNOWN,) * TIERS
            continue
        if cid not in label:
            unplaced.append(cid)
            out[cid] = (OTHER, OTHER, OTHER, own)
            continue
        cls = class_of(cid)
        if cls is None:
            other.append(own)
            out[cid] = (OTHER, own, own, own)
            continue
        lineage = lineage_of(cid, cls)
        c = [cls] + chain_of(cid, lineage) if lineage and lineage != cls else chain_of(cid, cls)
        d = len(c) - 1
        out[cid] = (name(c[0]), name(c[min(1, d)]), name(c[max(d - 1, min(1, d))]), name(c[d]))

    # A key must name one parent at the tier above, or the layer would hold two artifacts of one
    # key with different parents; the rule above guarantees it, and this says so if it breaks.
    seen: dict[tuple[int, str], str] = {}
    for keys in out.values():
        for level in range(1, TIERS):
            held = seen.setdefault((level, keys[level]), keys[level - 1])
            assert held == keys[level - 1], (
                f"{keys[level]!r} at tier {level} has parents {held!r} and {keys[level - 1]!r}"
            )
    report = {
        "cell_types": len(census_types),
        "classes": sorted({k[0] for k in out.values()}),
        "keys_per_tier": [len({k[i] for k in out.values()}) for i in range(TIERS)],
        "not_in_ontology": sorted(unplaced),
        "in_no_class": sorted(other),
    }
    return out, report
