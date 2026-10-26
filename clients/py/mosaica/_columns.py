"""`declare_columns`, which declares every column of a frame from its Arrow type.

Each column that is neither in `skip` nor already declared is declared as a detail, which is
stored and shown when an item is opened but is neither rendered nor filterable unless `render`
or `index` names it. A string column is declared `text` unless `keyword` or `category` names it,
and a categorical column is declared a category. Only the frame's schema is read, never its
values.
"""

from __future__ import annotations

from dataclasses import dataclass

import pyarrow as pa

#: A category declared from a frame reads an open, public vocabulary at this width.
VOCABULARY_WIDTH = "u16"


@dataclass
class DeclaredColumn:
    """One column that `declare_columns` looked at, and the type it declared the column as.

    - `name`: the column's name.
    - `dtype`: the column's Arrow type, as text.
    - `declared_as`: the attribute type the column was declared with, or `"not declared"` when
      no type fits it.
    - `render`, `index`, `unique`: whether the column was declared rendered, indexed and unique.
    - `vocabulary`: the vocabulary a category column's values come from, or `None` for any other
      column.
    - `why`: the reason the column was declared as it was, or was not declared.
    """

    name: str
    dtype: str
    declared_as: str
    render: bool
    index: bool
    vocabulary: str | None = None
    why: str = ""
    unique: bool = False


_WIDTHS = {
    "bool": "bool",
    "int8": "i8",
    "int16": "i16",
    "int32": "i32",
    "int64": "i64",
    "uint8": "u8",
    "uint16": "u16",
    "uint32": "u32",
    "uint64": "u64",
    "float": "f32",
    "double": "f64",
}


def columns_of(
    schema: dict[str, pa.DataType],
    skip: set[str],
    render: set[str],
    index: set[str],
    keyword: set[str],
    category: set[str],
    declared: set[str],
    unique: set[str] = frozenset(),
) -> tuple[list[dict], list[dict], list[DeclaredColumn]]:
    """The attribute blocks, the vocabulary blocks, and one row per column for the report."""
    attributes: list[dict] = []
    vocabularies: list[dict] = []
    rows: list[DeclaredColumn] = []
    for name, dtype in schema.items():
        if name in skip or name in declared:
            continue
        row = _column(name, dtype, name in keyword, name in category)
        row.render = row.declared_as != "not declared" and name in render
        row.index = row.declared_as != "not declared" and name in index
        row.unique = row.declared_as != "not declared" and name in unique
        rows.append(row)
        if row.declared_as == "not declared":
            continue
        block: dict = {"name": name, "type": row.declared_as}
        if row.vocabulary is not None:
            block["vocabulary"] = row.vocabulary
            vocabularies.append(
                {
                    "name": row.vocabulary,
                    "width": VOCABULARY_WIDTH,
                    "value_set": "open",
                    "visibility": "public",
                }
            )
        if row.render:
            block["render"] = True
        if row.index:
            block["index"] = True
        if row.unique:
            block["unique"] = True
        attributes.append(block)
    return attributes, vocabularies, rows


def _column(name: str, dtype: pa.DataType, keyword: bool, category: bool) -> DeclaredColumn:
    spelled = str(dtype)
    if pa.types.is_dictionary(dtype):
        value = dtype.value_type
        if (pa.types.is_string(value) or pa.types.is_large_string(value)) and not keyword:
            return DeclaredColumn(
                name, spelled, "category", False, False, vocabulary=name,
                why="a categorical column, over an open public vocabulary at width u16",
            )
        dtype, spelled = value, str(value)
    width = _WIDTHS.get(spelled)
    if width is not None:
        return DeclaredColumn(name, spelled, width, False, False, why="the matching width")
    if pa.types.is_timestamp(dtype):
        return DeclaredColumn(name, spelled, "timestamp_us", False, False, why="a datetime")
    if pa.types.is_string(dtype) or pa.types.is_large_string(dtype):
        if category:
            return DeclaredColumn(
                name, spelled, "category", False, False, vocabulary=name,
                why="category= named it, over an open public vocabulary",
            )
        if keyword:
            return DeclaredColumn(name, spelled, "keyword", False, False, why="keyword= named it")
        return DeclaredColumn(name, spelled, "text", False, False, why="a string column")
    if pa.types.is_list(dtype) or pa.types.is_large_list(dtype):
        return DeclaredColumn(
            name, spelled, "not declared", False, False,
            why="a list of strings is a view's access= column, or a declared attribute",
        )
    return DeclaredColumn(
        name, spelled, "not declared", False, False, why="no type is declared for this one"
    )
