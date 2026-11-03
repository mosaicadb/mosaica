"""The filter grammar in SQL, shared by the adapters whose dialects write it alike."""

from __future__ import annotations

from frame import EXTENT, Filter


def filter_sql(filt: Filter | None, quantise) -> str:
    """The filter as SQL conditions, each led by `AND`. A box's bounds are quantised in the
    dialect's own arithmetic by `quantise(expr, lo, hi)`; every other condition reads the same in
    ClickHouse and PostgreSQL."""
    terms = []
    for leaf in (filt.leaves if filt else []):
        col, value = leaf.get("col"), leaf["value"]
        if leaf["op"] == "eq" and col == "type":
            terms.append(f"type = '{value}'")
        elif leaf["op"] == "in" and col == "type":
            terms.append("type IN (" + ", ".join(f"'{v}'" for v in value) + ")")
        elif leaf["op"] == "eq" and col == "is_oa":
            terms.append(f"is_oa = {'true' if value else 'false'}")
        elif leaf["op"] == "range":
            if "gte" in value:
                terms.append(f"{col} >= {int(value['gte'])}")
            if "lt" in value:
                terms.append(f"{col} < {int(value['lt'])}")
        elif leaf["op"] == "bbox":
            x0, y0, x1, y1 = (float(v) for v in leaf["value"])
            terms.append(f"qx BETWEEN {quantise(repr(x0), EXTENT[0], EXTENT[1])} AND "
                         f"{quantise(repr(x1), EXTENT[0], EXTENT[1])}")
            terms.append(f"qy BETWEEN {quantise(repr(y0), EXTENT[2], EXTENT[3])} AND "
                         f"{quantise(repr(y1), EXTENT[2], EXTENT[3])}")
        else:
            raise ValueError(leaf)
    return "".join(f" AND {t}" for t in terms)
