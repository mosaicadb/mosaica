"""A Mosaica database in a directory: declare it, fill it, commit it and read it.

The directory holds everything `mosaica build` and `mosaica serve` read, so copying it moves the
database: `mosaica serve --deployment <dir>/mosaica.toml` serves it from wherever it lands.

Beside the declaration file, the package keeps its own copy of the declared blocks and pending
inserts as JSON under `.mosaica/`, written at every declaration and insert, so `open()` restores a
database saved before its first commit. It keeps nothing about what the database contains: what
the database holds is always asked of its server.
"""

from __future__ import annotations

import datetime
import json
import os
import re
import shutil
import subprocess
import time
import weakref
from pathlib import Path
from typing import TYPE_CHECKING, Any, Iterable, Sequence

import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.parquet as pq

from . import _columns
from . import _commit as C
from . import _declaration as D
from . import _inserts, _instance
from ._auth import Token, authorise, revoke
from ._control import Control
from ._inserts import Insert
from ._refusal import Refusal, listed
from ._reports import (
    ChangeReport,
    CommitReport,
    Declared,
    PagedReport,
    Report,
    render_columns_of,
)
from ._toml import Inline, dumps
from ._viewer import Selection, Viewer

if TYPE_CHECKING:
    from .widget import Map

#: The compiled extension that checks a declaration in this process, or `None`, in which case
#: the check runs `mosaica check` instead. Both use the same parser.
_mosaica = _instance.find_extension()

#: A temporary database goes here when the platform has a RAM-backed filesystem.
RAM_BACKED = Path("/dev/shm")

#: The role each kind of target's plain `insert(target, table, …)` is.
PLAIN_ROLE = {
    "view": "rows",
    "view_group": "rows",
    "attribute": "values",
    "layer": "key",
    "labels": "text",
    "vocabulary": "values",
}


def _accepted(answer, what: str) -> dict:
    """The body of a control-plane answer, or a refusal naming the status and what it said."""
    if not answer.ok:
        raise Refusal(f"{what}: refused ({answer.status}): {answer.detail[:1000]}")
    return answer.body


class Database:
    """A Mosaica database kept in a directory, which you fill from data frames and then read.

    Make one with `create()`, or reopen one with `open()`. You describe what the database holds
    with the `declare_*` methods before handing each declared thing a table with `insert`, which
    names the columns to read from it. Nothing reaches the database until `commit`, which builds
    it and starts its server the first time.

    Reads go through that server with the operator's own token, which `authorise` describes under
    `read_all`. To read as someone who holds only some access terms, call `viewer(terms)`.

        db = mosaica.create()
        db.declare_view("papers")
        db.insert("papers", frame, x="x", y="y", access="labels")
        db.commit()
        db.view("papers").count()

    Its attributes:

    - `path`: the directory the database is kept in.
    - `temporary`: `True` when `create()` made the database in a temporary directory, which
      `close()` deletes.
    - `built`: `True` once the first commit has built the database.
    - `inserts`: the `Insert` records made before the first commit, which that commit builds
      from.
    - `pending`: the `Insert` records made since the last commit, which the next commit sends.
    - `listening`: the addresses the server listens on, as `serve()` returns them, or `None`
      while the server is not running.
    """

    def __init__(self, path: Path, temporary: bool = False) -> None:
        self.path = Path(path)
        self.temporary = temporary
        # The server and a temporary directory, released by `close()` or once this is unreferenced.
        self._held = _instance.Held(self.path, temporary)
        weakref.finalize(self, _instance.release, self._held)
        #: The tables inserted before the first commit: what the build reads.
        self.inserts: list[Insert] = []
        #: The tables inserted since the last commit, which the next one sends and forgets.
        self.pending: list[Insert] = []
        self.blocks = D.Declaration()
        self.built = (self.path / "bundle" / "CURRENT").exists()
        self.listening: _instance.Listening | None = None
        self._loaded_text: str | None = None
        #: Category keys looked up by `sample()`, per reader: by server address and terms.
        self._keys: dict = {}

    @property
    def _child(self) -> subprocess.Popen | None:
        return self._held.child

    @property
    def binary(self) -> str:
        """The path of the `mosaica` program that checks, builds and serves this database.

        The program is the one `MOSAICA_BIN` names, then the first `mosaica` on `PATH`, then the one
        the `mosaica-native` wheel installed, then a build in a Mosaica checkout, whichever is found
        first. Reading this raises `Refusal` when none is found.
        """
        return _instance.find_binary()[0]

    def __repr__(self) -> str:
        kind = "a temporary database" if self.temporary else "a database"
        state = "committed" if self.built else "not yet committed"
        return f"{kind} at {self.path}, {state}"

    # ------------------------------------------------------------------ declarations

    def declare(self, kind: str, block: dict) -> dict:
        """Declare one block written in the keys of the declaration file, and return it.

        The `declare_*` methods build these blocks for you, so use this for a key that none of them
        takes. The block's contents are checked at the next `check()` or `commit()`. Like every
        `declare_*` method, this refuses a name already declared for the same kind of thing, and
        views and view groups count as one kind.

        - `kind`: `"view"`, `"view_group"`, `"vocabulary"`, `"attribute"` or `"layer"`.
        - `block`: the block as a dictionary, with the keys `schema.toml` uses.

            db.declare("attribute", {"name": "score", "type": "f64", "index": True})
        """
        if kind == "attribute":
            self._mark_a_filled_column(block)
        return self._declared(self.blocks.add(kind, block))

    def declare_view(
        self,
        name: str,
        *,
        extent: Any = None,
        projection: str = "none",
        default_label: str | None = "public",
        visibility: Any = "public",
        anchor: bool = False,
        title: str | None = None,
    ) -> dict:
        """Declare a view, and return its block.

        A view is one layout of the items, which a map draws from each item's x and y coordinates in
        that view. One set of items can have several views, such as two projections of the same
        embedding. The rows of a view come from `insert(name, table, x=, y=, access=)`.

        - `name`: the view's name.
        - `extent`: the range of coordinates the view covers, as `{"x": [min, max], "y": [min,
          max]}`, or as `{"lon": [...], "lat": [...]}` under a projection. By default the first
          commit fits it to its rows with a margin. An extent cannot change once built, so a view
          declared after the first commit must give one.
        - `projection`: `"none"` (the default) for plain x and y coordinates, or a map projection
          such as `"web_mercator"` for longitude and latitude, in which case the view's inserts name
          their coordinate columns with `lon=` and `lat=`.
        - `default_label`: the access label a row takes when its access column is empty, `"public"`
          by default. With `None` there is no default, and a build or request that carries a row
          without a label is refused.
        - `visibility`: who may see the view at all, `"public"` by default, or an access label or a
          list of them.
        - `anchor`: `True` makes this view's layout decide the order the items are stored in. The
          first view declared is the anchor by default.
        - `title`: the name shown for the view.

            db.declare_view("papers", extent={"x": [0, 100], "y": [0, 100]})
        """
        block = D.view_block(
            name,
            extent=extent,
            projection=projection,
            default_label=default_label,
            visibility=visibility,
            anchor=anchor,
            title=title,
        )
        return self._declared(self.blocks.add("view", block))

    def declare_view_group(
        self,
        name: str,
        *,
        metadata: dict | None = None,
        members: str | None = None,
        extent: Any = None,
        projection: str = "none",
        default_label: str | None = "public",
        visibility: Any = "public",
        title: str | None = None,
    ) -> dict:
        """Declare a view group, and return its block.

        A view group is a set of views that share every setting and differ by a key, such as one
        view per year, and each view in it is named `"<group>:<key>"`. The views and their metadata
        come from a roster table inserted with `insert(name, roster=table, key=)`, and their rows
        from `insert(name, table, view=)`, where `view=` names the column that says which view each
        row is in.

        - `name`: the group's name.
        - `metadata`: the metadata each view carries, as `{name: type}`, whose values the roster
          supplies. A category is written `{"type": "category", "vocabulary": vocabulary}`, and the
          roster then holds the value's key. The views carry none by default.
        - `members`: the name of another group whose views this group shares, in which case this
          group takes no roster and no `metadata` of its own.
        - `extent`, `projection`, `default_label`, `visibility`, `title`: as for `declare_view`, and
          each applies to every view in the group.

            db.declare_view_group("years", metadata={"year": "i32"})
        """
        block = D.view_group_block(
            name,
            metadata=metadata,
            members=members,
            extent=extent,
            projection=projection,
            default_label=default_label,
            visibility=visibility,
            title=title,
        )
        return self._declared(self.blocks.add("view_group", block))

    def declare_vocabulary(
        self,
        name: str,
        *,
        closed: bool = False,
        width: str = "u16",
        values: Any = None,
        reserved: Sequence[int] | None = None,
        visibility: str = "public",
        title: str | None = None,
    ) -> dict:
        """Declare a vocabulary, the set of values a category column may take, and return its block.

        Each value in a vocabulary has a key, a small integer code that the items store, and an
        optional title. The values come from `values=`, or from `insert(name, table, key=, title=,
        code=)`.

        - `name`: the vocabulary's name.
        - `closed`: `True` allows only the values given. An open vocabulary (the default) adds each
          new value it meets in the inserted rows.
        - `width`: the size of a code, `"u8"`, `"u16"` (the default) or `"u32"`, which limits how
          many values the vocabulary can hold.
        - `values`: the values, as a list of keys, or as `{key: code}` to fix the code of each.
        - `reserved`: codes that are never handed out.
        - `visibility`: `"public"` (the default) lists every value to every reader, and `"derived"`
          lists a value only to a reader who may see at least one item carrying it.
        - `title`: the name shown for the vocabulary.

            db.declare_vocabulary("venue", closed=True, values=["neurips", "icml", "iclr"])
        """
        block = D.vocabulary_block(
            name,
            closed=closed,
            width=width,
            values=values,
            reserved=reserved,
            visibility=visibility,
            title=title,
        )
        return self._declared(self.blocks.add("vocabulary", block))

    def declare_attribute(
        self,
        name: str,
        type: str,
        *,
        render: bool | None = None,
        index: bool | None = None,
        vocabulary: str | None = None,
        analyser: str | None = None,
        scope: Any = "entity",
        title: str | None = None,
        unique: bool | None = None,
    ) -> dict:
        """Declare a column that the items carry, and return its block.

        The column's values come from a view's rows that carry a column of the same name, as
        `insert` describes, or from a table inserted on its own with `insert(name, table, value=)`.

        - `name`: the column's name.
        - `type`: `"bool"`, `"u8"`, `"u16"`, `"u32"`, `"u64"`, `"i8"`, `"i16"`, `"i32"`, `"i64"`,
          `"f32"`, `"f64"`, `"timestamp_us"`, `"keyword"` (a string matched exactly), `"text"` (a
          string searched by word) or `"category"` (a value from a vocabulary).
        - `render`: `True` sends the value with every point drawn, so that a map can colour points
          by it. Any column except a keyword or text column can be rendered, and the choice,
          `False` by default, is fixed once the column is declared. A rendered column declared
          after the first commit draws no value for the points already stored until an insert
          gives them one.
        - `index`: `True` makes the column filterable. It is `False` by default.
        - `vocabulary`: for a category, the vocabulary its values come from.
        - `analyser`: for text, how the text is split into words. `"unicode"` is the default and
          only analyser.
        - `scope`: `{"group": name}` gives the column a separate value in each view of that view
          group, which must already be declared. By default an item has one value in every view.
        - `title`: the name shown for the column.
        - `unique`: `True` lets each value be held by one item at most, so that a value names an
          item and a table can address its rows' items by this column, as `insert` describes. An
          insert giving an item a value that another item holds is refused, and a filter `{name:
          {"in": values}}` finds the items holding those values whether or not the column is
          indexed. Only a `"keyword"`, integer or `"timestamp_us"` column that is not scoped to a
          group can be unique. It is `False` by default.

            db.declare_attribute("year", type="i32", render=True, index=True)
            db.declare_attribute("doi", type="keyword", unique=True)
        """
        block = D.attribute_block(
            name,
            type,
            render=render,
            index=index,
            vocabulary=vocabulary,
            analyser=analyser,
            scope=scope,
            title=title,
            unique=unique,
        )
        self._refuse_an_undeclared_group("attribute", name, block)
        self._mark_a_filled_column(block)
        return self._declared(self.blocks.add("attribute", block))

    def declare_shards(self, size: int) -> dict:
        """Declare the size of the shards the items are stored in, and return the `[shards]` table.

        A shard takes new items until it has handed out `size` row ids, and is then sealed so that
        new items go to the next shard. Without this declaration the size is 4,026,531,840.

        - `size`: the number of row ids per shard, as a positive integer.

        A first commit whose tables hold more items than the size builds several shards. After the
        first commit, the next `commit()` sends the new size to the server, and a size no larger
        than the number of row ids the open shard has handed out seals that shard at once.

            db.declare_shards(50_000_000)
        """
        self.blocks.shard_size = size
        return self._declared({"size": size})

    def declare_unique(self, name: str, unique: bool = True) -> dict:
        """Make a declared attribute unique, or stop it being unique, and return its block.

        - `name`: the attribute.
        - `unique`: `False` stops the attribute being unique and keeps its values.

        After the first commit the change reaches the database at the next `commit()`. Making a
        column unique there builds its index over the values the items hold, and the change is
        refused when two items hold the same value, naming up to ten such values.

            db.declare_unique("doi")
        """
        for block in self.blocks.blocks["attribute"]:
            if block.get("name") == name:
                block["unique"] = bool(unique)
                return self._declared(block)
        raise Refusal(f"declare_unique: no attribute named {name!r} is declared")

    def declare_columns(
        self,
        frame: Any,
        skip: Sequence[str] = (),
        render: Sequence[str] = (),
        index: Sequence[str] = (),
        keyword: Sequence[str] = (),
        category: Sequence[str] = (),
        unique: Sequence[str] = (),
    ) -> Declared:
        """Declare every column of a data frame as an attribute, and return what was declared.

        Each column is declared with the attribute type that fits its data type, and it is drawn or
        filterable only when `render` or `index` names it. The `Declared` report returned says what
        each column it looked at was declared as and why, or why it was not declared when no
        attribute type fits it. Only the frame's column types are read, and a column already
        declared is left as it is.

        - `frame`: a pandas or polars data frame or a pyarrow table.
        - `skip`: columns to leave out, such as the id and the coordinates.
        - `render`: columns to send with every point drawn.
        - `index`: columns to make filterable.
        - `keyword`, `category`: string columns to declare as keywords or as categories, since a
          string column is otherwise declared as text.
        - `unique`: columns to declare unique, as `declare_attribute` describes.

        Every name in these lists must be a column of the frame. A categorical column, which is a
        pandas `Categorical` or an Arrow dictionary column of strings, is declared as a category
        unless `keyword` names it. Each category reads the vocabulary of its own name, and when no
        such vocabulary is declared yet, it is declared open with `u16` codes, which limit it to
        65,535 values. Because the width cannot change after the first commit, declare a category
        that needs another width with `declare_vocabulary` before calling this.

            db.declare_columns(frame, skip=["paper_id", "x", "y"], category=["venue"])
        """
        schema = _inserts.schema_of(frame)
        for argument, names in (
            ("skip", skip), ("render", render), ("index", index), ("keyword", keyword),
            ("category", category), ("unique", unique),
        ):
            absent = [name for name in listed(names) if name not in schema]
            if absent:
                raise Refusal(
                    f"declare_columns: {argument}= names {', '.join(map(repr, absent))}, which "
                    f"the frame does not carry; name one of its columns: {', '.join(schema)}"
                )
        attributes, vocabularies, rows = _columns.columns_of(
            schema,
            set(listed(skip)),
            set(listed(render)),
            set(listed(index)),
            set(listed(keyword)),
            set(listed(category)),
            self.blocks.attribute_names(),
            set(listed(unique)),
        )
        held = self.blocks.vocabulary_names()
        new = [block for block in vocabularies if block["name"] not in held]
        for block in new:
            self.declare("vocabulary", block)
        for block in attributes:
            self.declare("attribute", block)
        return Declared(columns=rows, vocabularies=[block["name"] for block in new])

    def declare_layer(
        self,
        name: str,
        kind: str,
        *,
        views: Iterable[str] | None = None,
        membership: Any = "enumerated",
        shape: Any = None,
        default_space: str = "view",
        levels: Sequence[Any] | None = None,
        require_member_visibility: Any = "none",
        visibility: Any = "public",
        artifact_visibility: Any = "inherited",
        computed: Sequence[str] = D.DERIVED,
        supplied: Sequence[Any] | None = None,
        depends_on: Sequence[str] | None = None,
        prune_children: bool = False,
        withdraw_on_member_deletion: bool = False,
        value_set: str | None = None,
        scope: Any = "entity",
        layout: str | None = None,
        artifacts: Any = None,
        title: str | None = None,
    ) -> dict:
        """Declare an annotation layer, and return its block.

        An annotation layer is a set of annotations drawn over the items, such as a clustering, a
        set of regions or a taxonomy. Each annotation has a key and a set of member items, and both
        come from `insert(name, table, key=)` or from separate `insert(name, artifacts=...)` and
        `insert(name, members=...)` calls. Mosaica's names and fields call an annotation an
        artifact, as in `artifacts=` and `Viewer.artifacts`.

        - `name`: the layer's name.
        - `kind`: how the annotations relate: `"flat"` (no parents), `"nested"` (a tree), `"dag"`
          (several parents), `"stacked"` (independent levels) or `"tiered"` (levels in which each
          coarser annotation contains finer ones).
        - `views`: the views the layer is drawn on, all of them by default.
        - `membership`: how members are decided. `"enumerated"` (the default) reads them from a
          table, `"spatial"` makes each annotation a shape holding the items inside it, and
          `{"attribute": column}` makes one annotation for each value of a category column.
        - `shape`: for a spatial layer, `"bbox"`, `"circle"`, `"ellipse"` or `"polygon"`.
        - `default_space`: for a spatial layer, whether shapes are given in the view's coordinates
          (`"view"`, the default) or in longitude and latitude (`"wgs84"`).
        - `levels`: the levels of a stacked or tiered layer, each as `(level, title)` or `(level,
          title, (min_zoom, max_zoom))`.
        - `require_member_visibility`: how many of an annotation's members a reader must see before
          it is shown to them: `"none"` (the default), `"any"`, `"all"`, `{"fraction": 0.1}` or
          `{"count": 50}`.
        - `visibility`: who may see the layer at all, `"public"` by default or an access label.
        - `artifact_visibility`: the access label of an annotation that carries none of its own. The
          default, `"inherited"`, gives it none, so that `visibility` and
          `require_member_visibility` alone decide who sees it. `{"field": column, "default":
          label}` reads each annotation's own labels from `column`, which every `insert(name,
          artifacts=...)` must then name with `access=`. Without a field, the first such insert
          that names `access=` sets it.
        - `computed`: which of `"centroid"`, `"box"` and `"hull"` the server computes for each
          reader from the members it may see. All three are computed by default, except that a
          spatial layer draws its shape in place of a hull and a layer with attribute membership
          computes none.
        - `supplied`: content you provide for each annotation, as `(name, type, requires)` entries,
          where `requires` takes the values `declare_labels` takes for `content_requires`.
        - `depends_on`: the layers this one attaches to, such as the clustering a set of labels
          names.
        - `prune_children`: `True` shows only the finest qualifying annotation on each branch of the
          hierarchy, without its qualifying ancestors.
        - `withdraw_on_member_deletion`: Not built yet: `True` is refused at `check()`.
        - `value_set`: `"closed"` when no annotation keys may appear beyond those inserted. The
          default is closed when an artifacts table, `artifacts` or an attribute membership says
          which annotations there are, and `"open"` otherwise.
        - `scope`: `{"group": name}` keeps a separate set of annotations for each view of that view
          group, which must already be declared.
        - `layout`: how the server stores memberships, `"rows"`, `"column"` or `"list"`. It affects
          only speed, and the server chooses by default.
        - `artifacts`: annotations written into the declaration itself, as a list of dictionaries or
          a table.
        - `title`: the name shown for the layer.

            db.declare_layer("clusters", kind="flat", require_member_visibility={"count": 20})
        """
        block = D.layer_block(
            name,
            kind,
            views=views,
            membership=membership,
            shape=shape,
            default_space=default_space,
            levels=levels,
            require_member_visibility=require_member_visibility,
            visibility=visibility,
            artifact_visibility=artifact_visibility,
            computed=computed,
            supplied=supplied,
            depends_on=depends_on,
            prune_children=prune_children,
            withdraw_on_member_deletion=withdraw_on_member_deletion,
            value_set=value_set,
            scope=scope,
            layout=layout,
            artifacts=artifacts,
            title=title,
        )
        self._refuse_an_undeclared_group("layer", name, block)
        return self._declared(self.blocks.add("layer", block))

    def declare_labels(
        self,
        name: str,
        of: str,
        *,
        content_requires: str = "inherited",
        require_member_visibility: Any = "none",
        artifact_visibility: Any = "inherited",
        type: str = "text",
        title: str | None = None,
    ) -> dict:
        """Declare a set of text labels for the annotations of another layer, and return its block.

        A label set gives annotations of the layer `of` text to show, such as a topic name for a
        cluster, and a label is drawn wherever its annotation is drawn. The text comes from
        `insert(name, {key: text})` or `insert(name, table, key=, text=)`. On a layer with levels
        each label also names the level of its annotation, so the text comes from
        `insert(name, table, key=, text=, attached_level=)` and a mapping is refused.

        - `name`: the label set's name.
        - `of`: the declared layer it labels, which must not already carry a label set.
        - `content_requires`: `"inherited"` (the default) shows a label wherever its annotation is
          shown, and `"all"` shows it only to a reader who may see every item the text was written
          from. Those items come from `insert(name, members=table, key=)`.
        - `require_member_visibility`, `title`: as for `declare_layer`.
        - `artifact_visibility`: the access label that every label in the set carries. The default,
          `"inherited"`, gives the labels none of their own, so that the layer they label and
          `require_member_visibility` alone decide who sees them. A `field` is refused.
        - `type`: the type of each label's text, `"text"` by default.

            db.declare_labels("topics", of="clusters")
            db.insert("topics", {"c0": "graph neural networks", "c1": "diffusion models"})
        """
        parent = self.blocks.layer(of)
        if "labels" in parent:
            raise Refusal(
                f"layer {of!r} already carries a label set. A second one is a `[[layer]]` of its "
                f"own; write it through declare_layer"
            )
        parent["labels"] = D.labels_block(
            name,
            content_requires=content_requires,
            require_member_visibility=require_member_visibility,
            artifact_visibility=artifact_visibility,
            type=type,
            title=title,
        )
        self._declared(parent["labels"])
        return parent["labels"]

    def _declared(self, block: dict) -> dict:
        self._save_state()
        return block

    def _mark_a_filled_column(self, block: dict) -> None:
        """Mark an attribute declared after the first commit, which only inserts fill.

        The mark is never written. It keeps the written declaration from naming a source for the
        column, since the files the first commit read do not carry it.
        """
        if self.built and not block.get("scope"):
            block[D.FILLED] = True

    def _refuse_an_undeclared_group(self, kind: str, name: str, block: dict) -> None:
        """A scope names the group that owns the views its values or artifacts are keyed by."""
        scope = block.get("scope")
        group = scope.get("group") if isinstance(scope, dict) else None
        if group is None or group in self.blocks.group_names():
            return
        raise Refusal(
            f"{kind} {name!r}: scope names view group {group!r}, which this declaration does not "
            f"carry. declare_view_group({group!r}, …) before the block scoped to it"
        )

    # ------------------------------------------------------------------ inserting

    def insert(
        self,
        target: str,
        table: Any = None,
        roster: Any = None,
        artifacts: Any = None,
        members: Any = None,
        columns: dict | None = None,
        **named: str,
    ) -> Insert:
        """Hand a table to something declared, naming the columns to read, and return a record.

        Nothing is sent until the next `commit()`, and a second insert into the same target adds to
        the first.

        A data frame is copied into the database's directory when you call `insert`, while a Parquet
        file is read where it lies.

        - `target`: the name of a declared view, view group, attribute, layer, label set or
          vocabulary.
        - `table`: a pandas or polars data frame, a pyarrow table, or the path of a Parquet file.
        - `roster`, `artifacts`, `members`: in place of `table`, a view group's table of views, a
          layer's table of annotations or a layer's table of memberships, each in an insert of its
          own.
        - `columns`: `{attribute: column}` to read an attribute from a column with another name,
          either a unique attribute that names the rows' items or an attribute that a view's rows
          fill.
        - the other keywords: the column that holds each thing the target reads, such as `x=`, `y=`
          and `access=` for a view. A table in Mosaica's own shape, which is a layer's annotations
          or memberships, a label set's table read with `contents=`, a roster or a vocabulary's
          values, has each column named like one of its keywords read without the keyword, such
          as `key`, `level`, `parent` or `attached_key`; a keyword given names the column read in
          its place. A label set's table given with `text=` matches only `key`. The call is
          refused when it names a column the table lacks, gives a keyword the target does not
          read, leaves out one the target needs that the table does not carry, or names another
          column for a keyword whose own column the table carries too, since the build reads that
          column under its own name.

        Each row of a view, attribute or membership table names its item by its `mosaica_id` column
        or by its values of the unique attributes, read from each attribute's own column or the one
        `columns=` gives. A view's row that names no item makes a new item, and the commit leaves
        out and reports every other row that does not name exactly one item.

        A view's rows also fill each declared attribute whose name a column carries, from the anchor
        view's rows at the first commit and from any view's rows after it. The `Insert` returned
        lists the columns read and the columns ignored, which are those neither named, matched by
        name nor matching an attribute.

            db.insert("papers", frame, x="x", y="y", access="labels")
            db.insert("clusters", frame, key="cluster", columns={"paper": "paper_id"})
            db.insert("topics", artifacts="clusters.parquet")  # key, level, parent by name
        """
        self._refuse_when_closed()
        kind, block = self._target(target, named, roster, artifacts, members)
        role, data = self._role(target, kind, table, roster, artifacts, members, named)
        if _inserts.is_path(data) and not Path(data).expanduser().exists():
            raise Refusal(f"insert into {target!r}: {Path(data).expanduser()} does not exist")
        if (kind, role) not in _inserts.CONTRACTS:
            raise Refusal(
                f"insert into {kind} {target!r}: a {kind} takes no {role} table"
            )
        if not isinstance(data, dict):
            named = _inserts.with_columns_by_name(
                _inserts.CONTRACTS[(kind, role)], _inserts.schema_of(data), named
            )
        self._refuse_an_insert_the_target_cannot_take(target, kind, role, block, named)
        self._refuse_a_label_column_the_layer_does_not_read(target, kind, role, block, named)
        projected = kind in ("view", "view_group") and _inserts.projected(block)
        metadata = D.metadata_names(block) if role == "roster" else ()
        if kind == "labels" and role == "text":
            data, named = self._label_text(target, block, data, named)
        items = self._item_columns(target, kind, role, data, columns, named)
        matched = self._attribute_columns(target, kind, role, data, columns, named)
        insert = _inserts.build(
            target=target,
            kind=kind,
            role=role,
            data=data,
            named=named,
            directory=self.path,
            source=self._source_key(target, kind, role, named.get("view_key")),
            at_build=not self.built,
            projected=projected,
            metadata=metadata,
            named_attributes=matched,
            items=items,
        )
        self._refuse_a_second_view_without_its_labels(kind, role, target, insert)
        self._refuse_keys_that_would_create_unlabelled_artifacts(block, insert)
        insert = self._accumulated(insert)
        (self.pending if self.built else self.inserts).append(insert)
        self._save_state()
        return insert

    def _target(
        self, target: str, named: dict, roster=None, artifacts=None, members=None
    ) -> tuple[str, dict]:
        """The declared block this name refers to, or a refusal naming how to declare one.

        A category column and its vocabulary may share a name. The columns the call names then say
        which is meant: an attribute reads `value=`, a vocabulary `key=`, `title=` and `code=`.
        """
        found: list[tuple[str, dict]] = []
        for kind in D.KINDS:
            for block in self.blocks.blocks[kind]:
                if block.get("name") == target:
                    found.append((kind, block))
        for block in self.blocks.blocks["layer"]:
            labels = block.get("labels")
            if isinstance(labels, dict) and labels.get("name") == target:
                found.append(("labels", labels))
        if not found:
            raise Refusal(
                f"insert into {target!r}: nothing of that name is declared. A table is handed to "
                f"a declared thing, so declare_view, declare_view_group, declare_attribute, "
                f"declare_layer, declare_labels or declare_vocabulary comes first"
            )
        if len(found) == 1:
            return found[0]
        table_word = {"roster": roster, "artifacts": artifacts, "members": members}
        fits = [
            (kind, block)
            for kind, block in found
            if _fits(kind, named, next((w for w, v in table_word.items() if v is not None), None))
        ]
        if len(fits) == 1:
            return fits[0]
        kinds = ", ".join(sorted(kind for kind, _ in found))
        raise Refusal(
            f"insert into {target!r}: {kinds} are declared under that name, and the columns this "
            f"call names fit {'both' if not fits else 'neither'}. An attribute reads "
            f"value=; a vocabulary reads key=, title= and code=; a layer takes artifacts= or "
            f"members="
        )

    def _role(
        self, target: str, kind: str, table: Any, roster: Any, artifacts: Any, members: Any,
        named: dict,
    ) -> tuple[str, Any]:
        """Which of the target's tables this call is, and the data it carries."""
        given = [word for word, value in
                 (("roster", roster), ("artifacts", artifacts), ("members", members))
                 if value is not None]
        if artifacts is not None and isinstance(members, str):
            # On an artifacts insert `members=` names the column the memberships are in, which is
            # the artifact table's own key.
            named["members"] = members
            given.remove("members")
            members = None
        if table is not None and given:
            raise Refusal(
                f"insert into {target!r}: a table and {given[0]}= are two tables, and each insert "
                f"hands over one. Make them two calls"
            )
        if len(given) > 1:
            raise Refusal(
                f"insert into {target!r}: {' and '.join(given)} are two tables, each with its own "
                f"column names. Make them two calls"
            )
        if given:
            return given[0], {"roster": roster, "artifacts": artifacts, "members": members}[
                given[0]
            ]
        if table is None:
            raise Refusal(f"insert into {target!r}: no table was given")
        return PLAIN_ROLE[kind], table

    def _refuse_an_insert_the_target_cannot_take(
        self, target: str, kind: str, role: str, block: dict, named: dict
    ) -> None:
        """Refuse rows for a scoped layer that do not name their view."""
        scope = block.get("scope")
        group = scope.get("group") if isinstance(scope, dict) else None
        if group is not None and role in ("artifacts", "members", "key") and "view" not in named:
            raise Refusal(
                f"insert into {kind} {target!r}: this layer is scoped to group {group!r} and keys "
                f"its artifacts per view, one key in two views being two artifacts, so every row "
                f"carries the view it belongs to. Name the column that says which with view="
            )

    def _label_column(self, target: str, block: dict) -> str | None:
        """The column a layer reads its artifacts' labels from: the one it declares, a commit sent
        or an uncommitted artifacts insert named, or `None` where it reads none."""
        return dict(block.get("artifact_visibility") or {}).get("field") or next(
            (
                one.columns["access"]
                for one in self._uncommitted(target, "artifacts")
                if one.columns.get("access")
            ),
            None,
        )

    def _uncommitted(self, target: str, role: str) -> list[Insert]:
        """This layer's inserts of one role that no commit has sent yet."""
        return [
            one
            for one in (self.pending if self.built else self.inserts)
            if one.target == target and one.kind == "layer" and one.role == role
        ]

    def _refuse_a_label_column_the_layer_does_not_read(
        self, target: str, kind: str, role: str, block: dict, named: dict
    ) -> None:
        """Refuse an artifacts insert whose `access=` is not the column the layer reads its labels
        from. An insert naming no column is refused where the layer reads one, before and after
        the first commit, since its artifacts would reach the layer with no labels.
        """
        if kind != "layer" or role != "artifacts":
            return
        column = named.get("access") or None
        held = self._label_column(target, block)
        if column is None and held is not None:
            raise Refusal(
                f"insert into layer {target!r}: this layer reads each artifact's own access labels "
                f"from column {held!r}, and this insert names no access column. Name it with "
                f"access={held!r}, giving that column nulls for artifacts with no label of their "
                f"own"
            )
        if column is not None and held is not None:
            D.carry_labels(target, {"artifact_visibility": {"field": held}}, column)

    def _refuse_keys_that_would_create_unlabelled_artifacts(
        self, block: dict, insert: Insert
    ) -> None:
        """Before the first commit, refuse a `key=` or `members=` insert naming a key that neither
        an artifacts insert nor the layer's inline artifacts declare, on a layer that reads labels:
        the build would create that artifact from its key alone, with no labels. After the first
        commit the server refuses the same artifact.
        """
        target = insert.target
        if self.built or insert.kind != "layer" or insert.role not in ("key", "members"):
            return
        column = self._label_column(target, block)
        if column is None:
            return
        declared = {
            (int(row.get("level") or 0), str(row["key"]), None)
            for row in block.get("artifacts") or []
        }
        for one in self._uncommitted(target, "artifacts"):
            declared |= _artifact_addresses(one)
        kind = dict(block.get("hierarchy") or {}).get("kind")
        missing = _member_addresses(insert, levelled=kind in ("stacked", "tiered")) - declared
        if missing:
            level, key, view = min(missing, key=repr)
            raise Refusal(
                f"insert into layer {target!r}: key {key!r} names no artifact an artifacts "
                f"insert or the layer's own artifacts declare, so the build would create one with "
                f"no labels, and this layer reads each artifact's labels from column {column!r}. "
                f"Insert that artifact first with insert({target!r}, artifacts=..., key=..., "
                f"access={column!r})"
            )

    def _refuse_a_second_view_without_its_labels(
        self, kind: str, role: str, target: str, insert: Insert
    ) -> None:
        """Refuse rows for a second view that name no access column.

        An item carries the same labels in every view, and the build and the server both refuse rows
        whose labels disagree, so the missing column is refused here.
        """
        if kind not in ("view", "view_group") or role != "rows" or insert.columns.get("access"):
            return
        for other in self.inserts + self.pending:
            if other.role == "rows" and other.target != target and other.columns.get("access"):
                raise Refusal(
                    f"insert into {kind} {target!r}: view {other.target!r} reads each point's "
                    f"labels from column '{other.columns['access']}', and every view over one "
                    f"entity carries that entity's labels: a row whose labels disagree between "
                    f"views is refused at the build and on the ingest route. Name this frame's "
                    f"own label column with access="
                )

    def _item_columns(
        self, target: str, kind: str, role: str, data: Any, columns: dict | None, named: dict
    ) -> dict:
        """The columns a table names its items by, as `{name: column}`: its `mosaica_id` column,
        and the unique attributes it carries.

        A table carries a unique attribute in the column `columns=` names for it, else in the
        attribute's own column: the `field` its block declares, or its name. An artifacts table names its members inside its member
        structs, and a roster, a label set's text and a value set name no item, so they carry none
        here. An insert into a unique attribute carries it as its values.
        """
        schema = _inserts.schema_of(data)
        for name, column in (columns or {}).items():
            if column not in schema:
                raise Refusal(
                    f"insert into {kind} {target!r}: columns={{{name!r}: {column!r}}} names no "
                    f"column of this table. Its columns are {', '.join(schema)}"
                )
            if name not in self.blocks.attribute_names():
                raise Refusal(
                    f"insert into {kind} {target!r}: columns= names attribute {name!r}, which "
                    f"this declaration does not carry. declare_attribute({name!r}, …) first"
                )
        unique = self.blocks.unique_names()
        rows = kind in ("view", "view_group") and role == "rows"
        beyond = sorted(set(columns or {}) - set(unique))
        if beyond and not rows:
            raise Refusal(
                f"insert into {kind} {target!r}: columns= names the column each unique attribute "
                f"is read from, which names the item a row belongs to, and "
                f"{', '.join(repr(one) for one in beyond)} is not declared unique. Drop it, or "
                f"insert its values into the attribute itself: insert(<attribute>, table, value=…)"
            )
        if role in ("artifacts", "text", "roster") or kind == "vocabulary":
            if columns:
                raise Refusal(
                    f"insert into {kind} {target!r}: this table names no item by its rows, so "
                    f"columns= has nothing to say; drop it"
                )
            return {}
        items = {"mosaica_id": "mosaica_id"} if "mosaica_id" in schema else {}
        for name in unique:
            column = (columns or {}).get(name) or self._unique_column(name)
            if column in schema and column != named.get("view"):
                items[name] = column
        if kind == "attribute" and target in unique and named.get("value") is not None:
            items[target] = named["value"]
        return items

    def _unique_column(self, name: str) -> str:
        """The column a table carries a unique attribute in when `columns=` names none: the
        `field` its block declares, or its name."""
        block = next(b for b in self.blocks.blocks["attribute"] if b.get("name") == name)
        return block.get("field") or name

    def _attribute_columns(
        self, target: str, kind: str, role: str, data: Any, columns: dict | None, named: dict
    ) -> dict:
        """The declared attributes a view's rows fill, as `{attribute: column}`.

        A column fills the declared attribute of the same name, or the one `columns=` maps it
        to. The first commit's build reads an attribute from the anchor view's table alone, so
        there attribute-named columns in any other view's table fill nothing, and its unique
        columns only name the rows' items. After it, every view's points carry the declared
        columns their table holds, those declared since and the scoped columns of the view's
        group included.
        """
        if kind not in ("view", "view_group") or role != "rows":
            return {}
        anchor = kind == "view" and target == self.blocks.allocation_view()
        if not anchor and not self.built:
            filled = sorted(set(columns or {}) - set(self.blocks.unique_names()))
            if filled:
                raise Refusal(
                    f"insert into {kind} {target!r}: the first commit reads an attribute from "
                    f"the allocation view's frame, which is {self.blocks.allocation_view()!r}; "
                    f"map the column there, or insert the values into the attribute itself: "
                    f"insert(<attribute>, table, value=…)"
                )
            return {}
        # The groups whose scoped columns this table's views take: its own, and the one it shares.
        groups = set()
        if kind == "view_group":
            groups = {target, self.blocks.group(target).get("members")} - {None}
        names = [
            block["name"]
            for block in self.blocks.blocks["attribute"]
            if C._scoped_to(block) is None or (self.built and C._scoped_to(block) in groups)
        ]
        # The view column fills no attribute. A coordinate or label column can fill one.
        return _inserts.by_name(
            names, _inserts.schema_of(data), columns or {}, taken={named.get("view")}
        )

    def _source_key(self, target: str, kind: str, role: str, view_key: str | None = None) -> str:
        """The `[sources]` key an insert is written under, unique among this database's inserts."""
        stem = target.replace("/", "_").replace(":", "_")
        if view_key is not None:
            stem = f"{stem}_{view_key}".replace("-", "_").replace("/", "_")
        key = stem if role in ("rows", "values", "text") else f"{stem}_{role}"
        taken = {insert.source for insert in self.inserts + self.pending}
        # A category column and its vocabulary may share a name; the second is named for its kind.
        candidate, at = key, 1
        if candidate in taken:
            candidate = f"{key}_{kind}"
        while candidate in taken:
            at += 1
            candidate = f"{key}_{at}"
        return candidate

    def _accumulated(self, insert: Insert) -> Insert:
        """Join a second insert into the same target before the first commit onto the first.

        A block reads one file, so the parts are written together under `sources/`. Parts that name
        different columns, or whose column types differ, are refused.
        """
        if self.built:
            return insert
        held = next(
            (
                one
                for one in self.inserts
                if one.target == insert.target
                and one.kind == insert.kind
                and one.role == insert.role
                # A group whose views each have their own file inserts one table per view, and
                # the parts of one view are the parts that name it.
                and one.view_key == insert.view_key
            ),
            None,
        )
        if held is None:
            return insert
        if (
            held.columns != insert.columns
            or held.named_attributes != insert.named_attributes
            or held.items != insert.items
        ):
            raise Refusal(
                f"insert into {insert.kind} {insert.target!r}: this table names its columns "
                f"differently from the one already inserted ({held.columns} against "
                f"{insert.columns}). A block names its columns once, so a corpus in parts names "
                f"them the same way in every part"
            )
        differs = [
            (name, held.schema[name], insert.schema[name])
            for name in held.schema
            if name in insert.schema and str(held.schema[name]) != str(insert.schema[name])
        ]
        if differs or set(held.schema) != set(insert.schema):
            name, one, other = differs[0] if differs else (
                sorted(set(held.schema) ^ set(insert.schema))[0], "present", "absent"
            )
            raise Refusal(
                f"insert into {insert.kind} {insert.target!r}: this part's schema differs from "
                f"the one already inserted, at column {name!r}: {one} against {other}. A block "
                f"reads one file, and the parts are written as one, so a promoted column would "
                f"be a type neither part was written in. Write the parts in one schema"
            )
        table = pa.concat_tables([held.table(), insert.table()])
        path = self.path / "sources" / f"{held.source}.parquet"
        path.parent.mkdir(parents=True, exist_ok=True)
        pq.write_table(table, path)
        if not insert.in_place and insert.path != path:
            insert.path.unlink(missing_ok=True)
        self.inserts.remove(held)
        insert.path = path
        insert.declared_path = f"sources/{held.source}.parquet"
        insert.source = held.source
        insert.in_place = False
        insert.rows = table.num_rows
        insert.schema = dict(zip(table.schema.names, table.schema.types))
        return insert

    def _label_text(
        self, target: str, block: dict, data: Any, named: dict
    ) -> tuple[Any, dict]:
        """A label set's text as the annotations table its layer reads, and the columns it names.

        Given a mapping from key to text, or a table with `text=`, the table is written here and
        each label is attached to the annotation with its own key in the layer it labels, at the
        level `attached_level=` names on a layer with levels. Given a table with `contents=`, the
        table is read as it is and must name its attachment.
        """
        layer = next(
            one
            for one in self.blocks.blocks["layer"]
            if isinstance(one.get("labels"), dict) and one["labels"]["name"] == target
        )
        of = layer["name"]
        if layer.get("levels") and "contents" not in named and "attached_level" not in named:
            raise Refusal(
                f"insert into labels {target!r}: layer {of!r} has levels, so each label names the "
                f"level of the cluster it is attached to. Insert a table with text= and "
                f"attached_level= naming the column that holds that level"
            )
        attached_levels = None
        if isinstance(data, dict):
            if named:
                raise Refusal(
                    f"insert into labels {target!r}: a mapping from key to text carries no "
                    f"columns, so {', '.join(f'{one}=' for one in sorted(named))} names nothing. "
                    f"Insert a table to name columns on it"
                )
            keys = [str(key) for key in data]
            texts = [[[value]] if isinstance(value, str) else [list(value)]
                     for value in data.values()]
            levels = [0] * len(keys)
            attached_keys = list(keys)
            attached_layers = [of] * len(keys)
        elif "text" in named:
            table = _inserts.decoded(
                _inserts.as_table(data) if not _inserts.is_path(data) else pq.read_table(data)
            )
            missing = [
                name for name, column in named.items() if column not in table.column_names
            ]
            if missing:
                raise Refusal(
                    f"insert into labels {target!r}: "
                    + ", ".join(f"{name}={named[name]!r}" for name in missing)
                    + f" names no column of this table. Its columns are "
                    f"{', '.join(table.column_names)}"
                )
            keys = [str(key) for key in table[named["key"]].to_pylist()]
            texts = [[[value]] for value in table[named["text"]].to_pylist()]
            levels = (
                [int(one or 0) for one in table[named["level"]].to_pylist()]
                if "level" in named
                else [0] * len(keys)
            )
            attached_keys = (
                [str(one) for one in table[named["attached_key"]].to_pylist()]
                if "attached_key" in named
                else list(keys)
            )
            attached_layers = (
                [str(one) for one in table[named["attached_layer"]].to_pylist()]
                if "attached_layer" in named
                else [of] * len(keys)
            )
            if "attached_level" in named:
                attached_levels = table[named["attached_level"]].to_pylist()
            carried = {"key", "text", "level", "attached_key", "attached_layer", "attached_level"}
            beyond = sorted(set(named) - carried)
            if beyond:
                raise Refusal(
                    f"insert into labels {target!r}: "
                    + ", ".join(f"{one}=" for one in beyond)
                    + " names a column the table the SDK writes from text= does not carry. That "
                    "table is the key, the text as one ranked content, the level and the "
                    "attachment. Write the table in the publication's own shape and insert it "
                    "with contents=, which is read as it stands"
                )
        else:
            if "attached_key" not in named:
                raise Refusal(
                    f"insert into labels {target!r}: a label set expands to a layer that depends "
                    f"on its clustering, so every artifact it publishes attaches to one, and a "
                    f"table read as it stands carries the attachment. Name it with "
                    f"attached_key= (and attached_layer= where the table says which layer), or "
                    f"insert the text as a mapping or with text=, which attaches each label to "
                    f"the cluster its key names"
                )
            return data, named
        written = pa.table(
            {
                "level": pa.array(levels, type=pa.uint32()),
                "key": pa.array(keys, type=pa.string()),
                "contents": pa.array(texts, type=pa.list_(pa.list_(pa.string()))),
                "attached_layer": pa.array(attached_layers, type=pa.string()),
                "attached_key": pa.array(attached_keys, type=pa.string()),
            }
        )
        columns = {
            "key": "key",
            "contents": "contents",
            "level": "level",
            "attached_layer": "attached_layer",
            "attached_key": "attached_key",
        }
        if attached_levels is not None:
            written = written.append_column(
                "attached_level", pa.array(attached_levels, type=pa.uint32())
            )
            columns["attached_level"] = "attached_level"
        return written, columns

    # ------------------------------------------------------------------ the document

    @property
    def declaration(self) -> str:
        """The database's declaration, as the TOML text that its `schema.toml` holds."""
        if self._loaded_text is not None:
            return self._loaded_text
        return dumps(self._document())

    def _document(self) -> dict:
        sources = {
            insert.source: insert.declared_path
            for insert in self.inserts
            if insert.declared_path is not None
        }
        document = self.blocks.document(sources)
        self._anchor_a_group(document)
        D.bind(document, self.inserts)
        D.bind_value_sets(document, self.inserts + self.pending)
        return document

    def _anchor_a_group(self, document: dict) -> None:
        """Anchor on the first view of the first group's roster when no plain view is declared."""
        defaults = document.setdefault("defaults", {})
        if defaults.get("allocation_view") or not document.get("view_group"):
            return
        for group in document["view_group"]:
            for record in group.get("view") or []:
                defaults["allocation_view"] = f"{group['name']}:{record['key']}"
                return
            for insert in self.inserts + self.pending:
                if insert.target != group["name"] or insert.role != "roster":
                    continue
                keys = insert.table()[insert.columns["key"]].to_pylist()
                if keys:
                    defaults["allocation_view"] = f"{group['name']}:{keys[0]}"
                    return
        if not defaults:
            document.pop("defaults")

    def write(self) -> dict:
        """Write `schema.toml` and `mosaica.toml`, and return the declaration as a dictionary.

        Because `check()` and `commit()` write the files themselves, call this only to look at them
        first.
        """
        self._refuse_when_closed()
        document = self._document()
        (self.path / "schema.toml").write_text(dumps(document), encoding="utf-8")
        _instance.write_deployment(self.path)
        _instance.secrets_for(self.path)
        self._loaded_text = None
        return document

    def _save_state(self) -> None:
        """The SDK's own copy of the declaration, so `open()` reads the blocks back."""
        self._refuse_when_closed()
        state = {
            "inserts": [_stored(insert) for insert in self.inserts],
            "pending": [_stored(insert) for insert in self.pending],
            "blocks": _tagged(self.blocks.blocks),
            "shard_size": self.blocks.shard_size,
        }
        path = self.path / ".mosaica" / "declaration.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(state, indent=1), encoding="utf-8")

    # ------------------------------------------------------------------ check and commit

    def check(self) -> Report | PagedReport:
        """Report what the next `commit()` would do, without sending anything.

        Before the first commit, the check reads the declaration against the column types of the
        inserted tables and reports each problem it finds. After the first commit, the report
        lists the requests that the commit would send, and any problem it would stop at before
        sending them.

            db.check()
        """
        if self.built:
            return self._paged(sent=False)
        document = self.write()
        findings = self._preflight(document)
        ok, page = self._checked()
        return Report(
            what="check",
            ok=ok and not findings,
            frames=self._frames(document),
            render_columns=render_columns_of(document.get("attribute", [])),
            notes=self._notes(),
            rows=self._inserted_rows(),
            findings=findings,
            log=page,
        )

    def _checked(self) -> tuple[bool, str]:
        """Check the written declaration, and return whether it passed and the page it printed.

        The check runs in this process where the extension is installed, and as `mosaica check`
        otherwise. The page is the same either way.
        """
        deployment = str(self.path / "mosaica.toml")
        if _mosaica is None:
            result = self._run(["check", "--deployment", deployment])
            return result.returncode == 0, result.stdout + result.stderr
        try:
            report = _mosaica.check(deployment)
        except _mosaica.DeclarationError as refused:
            return False, f"check FAILED: {refused}"
        return report.ok, report.page

    def commit(self, strict: bool = False) -> CommitReport | PagedReport:
        """Make what was inserted part of the database, and return a report of what happened.

        The first commit checks the declaration, builds the database from the inserted tables and
        starts a server over it. After it, no view's extent, column type or choice of rendered
        columns can change. Each later commit sends what was inserted since the last one to the
        running server, waits until readers can see it and then forgets those inserts.

        Besides the rows that `insert` says are left out, the commit leaves out a row that names an
        item, or sets a unique value, that an earlier row of its table did. The rest is applied,
        and the report counts the rows left out by reason.

        - `strict`: `True` refuses the whole build, or the whole request, that carries such a row.

        A commit that did nothing raises `Refusal`, with the report as its `report`, which happens
        when a problem is found before anything is sent, when the build fails or when the server
        refuses every request. A commit in which some requests succeeded returns its report with the
        refusals listed. A request the server answers `429` is sent again as `Control` describes,
        and one it gives up on is listed in the report's `refusals`.

            db.commit()
        """
        if self.built:
            return self._paged(sent=True, strict=strict)
        document = self.write()
        findings = self._preflight(document)
        if findings:
            raise Refusal(
                "commit: nothing was sent or built, because of what follows:\n"
                + "\n".join(f"  {finding}" for finding in findings)
            )
        started = time.monotonic()
        ok, page = self._checked()
        if not ok:
            raise Refusal(_reason_first("commit: the declaration did not check", page, page))
        build = self._run(
            ["build", "--deployment", str(self.path / "mosaica.toml")]
            + (["--strict"] if strict else [])
        )
        report = CommitReport(
            what="commit",
            ok=build.returncode == 0,
            frames=self._frames(document),
            render_columns=render_columns_of(document.get("attribute", [])),
            notes=self._notes(),
            rows=self._inserted_rows(),
            log=page + "\n" + build.stdout + build.stderr,
            identity=self._identity_in_words(),
        )
        if build.returncode != 0:
            raise Refusal(
                _reason_first("commit: the build failed", build.stderr or build.stdout, report.log),
                report,
            )
        self.built = True
        self._record_committed_layers(document.get("layer", []))
        self.serve()
        report.seconds = time.monotonic() - started
        if self.listening is not None:
            report.viewer = self.listening.viewer
            report.session = self.listening.session
            report.control = self.listening.control
        for view in self.meta().get("views", []):
            name = view.get("group") or view["id"]
            report.views[name] = report.views.get(name, 0) + 1
        # From the declaration: a layer gated on a label no row carries is in no reader's meta.
        for block in document.get("layer", []):
            report.layers.append(block["name"])
            if "labels" in block:
                report.labels.append(block["labels"]["name"])
        built = _BUILT.search(build.stdout + build.stderr)
        if built is not None:
            report.items = int(built.group("items"))
            report.minted = int(built.group("minted"))
            report.unclustered = int(built.group("unclustered"))
        refused = self.path / "bundle" / "reports" / "refused.json"
        if built is not None and int(built.group("refused")) > 0 and refused.exists():
            report.refused = json.loads(refused.read_text(encoding="utf-8"))
        return report

    def _inserted_rows(self) -> dict:
        """The rows inserted before the first commit, by view or view group."""
        rows: dict = {}
        for insert in self.inserts:
            if insert.role == "rows":
                rows[insert.target] = rows.get(insert.target, 0) + insert.rows
        return rows

    def _preflight(self, document: dict) -> list[C.Finding]:
        """The findings the build's own inserts can raise, before a byte is read."""
        findings: list[C.Finding] = []
        C.keys_into_supplied_content(document, self.inserts, findings)
        return findings

    # ------------------------------------------------------------------ the paged commit

    def _paged(self, sent: bool, strict: bool = False) -> PagedReport:
        """The plan over what was inserted since the last commit, run where `sent`."""
        self.serve()
        control = self.control
        planner = C.Planner(self, control, self.meta())
        pages, findings = planner.plan()
        report = PagedReport(
            sent=sent,
            # The closing flush is where the commit waits, so the plan lists it.
            plan=[page.line for page in pages]
            + (["flush, and wait for the publication it arms"] if pages else []),
            findings=findings,
            ignored_columns=planner.ignored_columns,
        )
        if not sent:
            return report
        if report.findings:
            raise Refusal(str(report), report)
        accepted = C.run(control, pages, report, strict)
        self._record_committed_layers(page.body for page in accepted if page.kind == "layer")
        self.pending.clear()
        self._save_state()
        if pages and not accepted:
            raise Refusal(str(report), report)
        return report

    @property
    def control(self) -> Control:
        """A `Control` client for this database's control plane.

        Use it for what `Database` has no method for. The client sends the operator credential,
        and reading this property starts the server if it is not running.
        """
        listening = self.serve()
        return Control(f"http://{listening.control}", self.operator_credential)

    def _payloads(self) -> dict:
        """The request bodies the declaration becomes at a running server, one entry per block kind.

        `mosaica check --payloads` writes them, so the binary serialises what it parsed.
        """
        self.write()
        deployment = str(self.path / "mosaica.toml")
        refused = "commit: the declaration did not check, so nothing was sent"
        if _mosaica is None:
            result = self._run(["check", "--deployment", deployment, "--payloads"])
            if result.returncode != 0:
                log = result.stdout + result.stderr
                raise Refusal(_reason_first(refused, log, log))
            return json.loads(result.stdout)
        try:
            return json.loads(_mosaica.payloads(deployment))
        except _mosaica.DeclarationError as why:
            raise Refusal(f"{refused}: {why}") from None

    def token(self, terms: Sequence[str] | None = None) -> Token:
        """Make a token that reads this database with the access terms given.

        Pass the token to `connect` or `Map` to read as someone who holds it.

        - `terms`: the access terms the token holds, as a list or as one term in a string.
          Without them the token is the operator's own, which `authorise` describes under
          `read_all`.

            token = db.token(["cs.LG"])
        """
        self.serve()
        if terms is None:
            return authorise(self.session_url, self.operator_credential, read_all=True)
        return authorise(self.session_url, self.operator_credential, terms=listed(terms))

    def viewer(self, terms: Sequence[str] | None = None) -> Viewer:
        """A reader of this database that holds only the access terms given.

        The reader sees only the items those terms admit, by the rule that `Viewer` states.

        - `terms`: the access terms, as a list or as one term in a string. Without them the reader
          reads with the operator's token, as `db.view` and the other reads on `Database` do. An empty list is refused
          because such a reader would see nothing, while a term that no item carries is accepted.

            db.viewer(["cs.LG"]).view("papers").count()
            db.viewer(["cs.LG"]).map()
        """
        self._refuse_before_the_first_commit("viewer")
        chosen = None if terms is None else listed(terms)
        if chosen == []:
            raise Refusal(
                "viewer: a reader holding no terms sees nothing. Name at least one term, or "
                "call viewer() with no terms to read everything"
            )
        self.serve()
        reader = Viewer(self.viewer_url, lambda: self.token(chosen), terms=chosen)
        # A new token is made for each call, so a commit's new views are seen, while the category
        # keys already looked up are kept for every reader holding the same terms.
        held = None if chosen is None else frozenset(chosen)
        reader._keys = self._keys.setdefault((self.viewer_url, held), {})
        return reader

    def map(
        self,
        view: str | None = None,
        layers: Sequence[str] | None = None,
        colour_by: str | None = None,
        filters: dict | None = None,
        height: int = 480,
        size_by: str | None = None,
        basemap: Any = None,
        **kwargs,
    ) -> Map:
        """An interactive map of this database for a notebook, showing every item.

        This is `Viewer.map` read as the operator, with its arguments. To map part of a view, call
        `db.view(name).map()`, and to see what a reader holding some terms sees, call
        `db.viewer(terms).map()`.

            db.map(colour_by="cluster:clusters")
        """
        self._refuse_before_the_first_commit("map")
        return self.viewer().map(
            view=view,
            layers=layers,
            colour_by=colour_by,
            filters=filters,
            height=height,
            size_by=size_by,
            basemap=basemap,
            **kwargs,
        )

    def meta(self) -> dict:
        """The database's structure as its server describes it, as a dictionary.

        The description lists the views with their coordinate ranges, the annotation layers, the
        columns with the filters each one takes, and the server's limits.
        """
        self._refuse_before_the_first_commit("meta")
        return self.viewer().meta()

    def item(self, mosaica_id) -> dict:
        """One item's full record, as a dictionary.

        The record holds the item's `fields`, `labels` and `views`, as `Viewer.item` describes.

        - `mosaica_id`: the item's id, as a sample's `mosaica_id` column or a pick on the map
          gives it.

            db.item(db.view("papers").sample(k=1).column("mosaica_id")[0].as_py())
        """
        self._refuse_before_the_first_commit("item")
        return self.viewer().item(mosaica_id)

    def items(self, view: str, fields: Sequence[str], **options):
        """Every item in a view, with the fields named, as one pyarrow table.

        This is `Viewer.items` read as the operator, who sees every item, with its keywords. With
        `batches=True` it returns the pages one at a time instead.

            db.items("papers", ["title", "year"]).to_pandas()
            db.items("papers", ["title"], filters={"year": {"eq": 2023}}, order="stored")
        """
        self._refuse_before_the_first_commit("items")
        return self.viewer().items(view, fields, **options)

    def lookup(self, view: str, field: str, values: Iterable[Any], fields: Sequence[str] = ()):
        """Find the items in a view by their values of a unique column, as a pyarrow table.

        This is `Viewer.lookup` read as the operator, with a row for each item whose value of
        `field` is one of `values`.

            db.lookup("papers", "doi", ["10.1/a", "10.2/b"], fields=["title"])
        """
        self._refuse_before_the_first_commit("lookup")
        return self.items(view, [field, *listed(fields)], filters={field: {"in": listed(values)}})

    def artifacts(self, view: str, layer: str, fields: Sequence[str], **options):
        """Every annotation of a layer in a view, with the properties named, as one pyarrow table.

        This is `Viewer.artifacts` read as the operator, with its keywords. With `batches=True` it
        returns the pages one at a time instead.

            db.artifacts("papers", "clusters", ["key", "masked_count"])
        """
        self._refuse_before_the_first_commit("artifacts")
        return self.viewer().artifacts(view, layer, fields, **options)

    def browse_artifacts(self, view: str, layer: str, **options) -> dict:
        """One page of a layer's annotations, each with its `mosaica_id`, key, name and count.

        This is `Viewer.browse_artifacts` as this database's own reader; `options` are its
        keywords.

            ids = {one["name"]: one["mosaica_id"] for one in
                   db.browse_artifacts("papers", "topics", level=0)["artifacts"]}
        """
        self._refuse_before_the_first_commit("browse_artifacts")
        return self.viewer().browse_artifacts(view, layer, **options)

    def artifact(self, mosaica_id: Any, view: str, **options) -> dict:
        """One annotation's record in `view`.

        This is `Viewer.artifact` as this database's own reader; `options` are its keywords.

            db.artifact(db.browse_artifacts("papers", "topics")["artifacts"][0]["mosaica_id"],
                        "papers")
        """
        self._refuse_before_the_first_commit("artifact")
        return self.viewer().artifact(mosaica_id, view, **options)

    def viewport_artifacts(self, view: str, zoom: int, per_tile: int, **options):
        """The annotations in each map tile at one zoom level, as one pyarrow table.

        This is `Viewer.viewport_artifacts` read as the operator, with its keywords, and `per_tile`
        limits how many annotations each level of a layer gives one tile.

            db.viewport_artifacts("papers", 2, per_tile=20, layers=["clusters"])
        """
        self._refuse_before_the_first_commit("viewport_artifacts")
        return self.viewer().viewport_artifacts(view, zoom, per_tile, **options)

    def aggregate(
        self,
        view: str,
        groupings: Sequence[dict],
        filters: dict | None = None,
        reference: dict | None = None,
    ) -> list:
        """Count the items in a view, overall or by group, as one pyarrow table per grouping.

        This is `Viewer.aggregate` read as the operator, who sees every item, with its arguments.

            venue = {"count": True, "by": {"field": "venue", "top": 10}}
            [size, venues] = db.aggregate("papers", [{"count": True}, venue])
        """
        self._refuse_before_the_first_commit("aggregate")
        return self.viewer().aggregate(view, groupings, filters, reference)

    def view(self, name: str) -> Selection:
        """The whole of one view, as a `Selection` that you count, sample, filter or map.

        The selection reads as the operator, who sees every item.

        - `name`: the view's name, which for a view in a view group is `"<group>:<key>"`. An
          unknown name is refused, and the refusal lists the views there are.

            db.view("papers").count()
            db.view("papers").filter({"venue": {"eq": "neurips"}}).map()
        """
        self._refuse_before_the_first_commit("view")
        self.viewer()._require_view(name)
        return Selection(self.viewer, name, items=self.items)

    def categories(
        self,
        column: str,
        prefix: str | None = None,
        view: str | None = None,
        codes: Sequence[int] | None = None,
        filters: dict | None = None,
    ):
        """The values of a category column, as a pyarrow table.

        This is `Viewer.categories` read as the operator, who sees every value, with its arguments.

            db.categories("venue")
            db.categories("venue", prefix="neur")
            db.categories("venue", codes=[1, 3])
        """
        self._refuse_before_the_first_commit("categories")
        return self.viewer().categories(column, prefix, view, codes, filters)

    def _identity_in_words(self) -> str:
        """How this database names a row, for the commit report."""
        unique = self.blocks.unique_names()
        if not unique:
            return (
                "no attribute is declared unique, so each row of the points is an item of its "
                "own, named by its mosaica_id"
            )
        return (
            f"a row names its item by its values of {', '.join(repr(one) for one in unique)}, "
            f"and a row of the points naming none is an item of its own"
        )

    def _record_committed_layers(self, layers: Iterable[dict]) -> None:
        """Write onto the SDK's own layer blocks what each layer was committed with: its label
        column, so a later insert naming another column is refused, and its value set, so a later
        commit sends the declaration the database holds whatever that commit inserts."""
        declared = {block.get("name"): block for block in self.blocks.blocks["layer"]}
        for layer in layers:
            held = declared.get(layer["name"])
            if held is None:
                continue
            field = dict(layer.get("artifact_visibility") or {}).get("field")
            if field:
                D.carry_labels(layer["name"], held, field)
            if layer.get("value_set"):
                held["value_set"] = layer["value_set"]
                held[D.CHOSEN_VALUE_SET] = True
        self._save_state()

    # ------------------------------------------------------------------ verbs that are not inserts

    def remove(self, items: Any, strict: bool = False) -> ChangeReport:
        """Delete items from the database, and return a report of what was deleted.

        The items stop being served as soon as the call returns, and their rows are removed from
        disc at the next compaction, which `compact()` asks for.

        - `items`: the items to delete, as one `mosaica_id`, a list of them or a table. A table is a
          pandas or polars data frame, a pyarrow table or a dict of columns, and each of its rows
          names one item by its `mosaica_id` column or by its values of the unique attributes, which
          it carries in the columns an insert reads them from.
        - `strict`: `True` refuses the whole request when a row names no item, or names two, and the
          call then raises `Refusal`.

        Without `strict`, a row that names no item or two is listed in the report as refused and the
        other rows are applied. A `None` in the list, or a row whose cells are all null, names no
        item. A table's columns that name no item are not sent, and the report lists them in
        `ignored_columns`, but a table with no column that names items is refused before anything is
        sent. A `mosaica_id` may also name an annotation, while a layer's own is refused, because
        `drop_layer` is how a layer is removed.

        A call in which the server refused every request raises `Refusal`, with the report as its
        `report`.

            db.remove([mosaica_id])
            db.remove({"paper": ["paper-17", "paper-23"]})
        """
        return self._changes(items, "delete", strict)

    def suppress(self, items: Any, strict: bool = False) -> ChangeReport:
        """Hide items from every reader until `unsuppress` shows them again, and return a report.

        A suppressed item stays stored and is left out of every answer to a request that starts
        after the call returns.

        - `items`, `strict`: as for `remove`, except that a layer's `mosaica_id` hides the layer
          and its annotations.
        """
        return self._changes(items, "suppress", strict)

    def unsuppress(self, items: Any, strict: bool = False) -> ChangeReport:
        """Lift the suppression of items, so that their readers see them again, and return a report.

        - `items`, `strict`: as for `remove`.
        """
        return self._changes(items, "unsuppress", strict)

    def addresses(self, items: Any) -> list[dict]:
        """The rows that name the items given in a change request, as a list of dictionaries.

        Each `mosaica_id` in a list becomes `{"mosaica_id": ...}`, and each row of a table becomes
        its `mosaica_id` value and its values of the unique attributes, under the attributes' names.
        A `None` or a null cell stays `None`, and every other value is written as text: an integer
        in decimal digits, a timestamp as the decimal digits of its microseconds since the epoch,
        and a string as itself.
        """
        return _rows_of(self._addressed(items, "addresses")[0])

    def _addressed(self, items: Any, verb: str) -> tuple[dict, list[str]]:
        """The items given as a table of the columns that name them, `{name: [text or None]}`,
        and the names of the columns given that name no item, which are not sent."""
        if not (isinstance(items, dict) or hasattr(items, "columns")):
            items = listed(items)
            if any(isinstance(one, dict) for one in items):
                raise Refusal(
                    f"{verb}: items is a list of dicts. Pass a table instead: a dict of columns, "
                    f"such as {{'mosaica_id': [...], 'paper': [...]}}, or a data frame"
                )
            return {"mosaica_id": [None if one is None else C.text(one) for one in items]}, []
        table = _address_table(items, verb)
        naming = self._identifying_columns()
        sent = {
            naming[column]: [
                None if one is None else C.text(one) for one in table[column].to_pylist()
            ]
            for column in table.column_names
            if column in naming
        }
        ignored = [column for column in table.column_names if column not in naming]
        if table.num_rows and not sent:
            raise Refusal(
                f"{verb}: the table's columns {', '.join(ignored)} name no item, since none is "
                f"mosaica_id or a unique attribute's column ({', '.join(naming)}). Pass the "
                f"items' mosaica_ids or their unique values in one of those columns"
            )
        return sent, ignored

    def _identifying_columns(self) -> dict[str, str]:
        """The column a table names items by, mapped to the name the server reads it under:
        `mosaica_id`, and each unique attribute's column as an insert reads it."""
        named = {self._unique_column(one): one for one in self.blocks.unique_names()}
        return {"mosaica_id": "mosaica_id", **named}

    def _changes(self, items: Any, op: str, strict: bool) -> ChangeReport:
        self._refuse_before_the_first_commit(op)
        table, ignored = self._addressed(items, op)
        rows = _rows_of(table)
        report = ChangeReport(op=op, requested=len(rows), ignored_columns=ignored)
        control = self.control
        for start, answer in C.changes(control, rows, op, control.limits(), strict):
            _fold_change(report, answer, start, rows)
        if rows and not report.accepted and report.refusals:
            raise Refusal(str(report), report)
        return report

    def leave(
        self,
        layer: str,
        key: str,
        items: Any,
        rank: int = 0,
        level: int = 0,
        view: str | None = None,
        strict: bool = False,
    ) -> ChangeReport:
        """Take items out of the set of items a label's text was written from, and return a report.

        A label declared with `content_requires="all"` is shown only to a reader who may see every
        item its text was written from, so once an item is taken out of that set, a reader no
        longer needs to see it to be shown the label.

        - `layer`: the label set.
        - `key`: the label's key.
        - `items`, `strict`: as for `remove`.
        - `rank`: which of the label's texts to change, by its position among them, which is 0 by
          default. A rank keeps its number when a text before it is withdrawn.
        - `level`: the level the label is at, which is 0 by default.
        - `view`: the view the label belongs to, on a layer scoped to a view group.

        Taking out every item withdraws the text, and its rank is never used again, so inserting
        the label with the same texts adds each withdrawn one as a new text at the next free rank.
        A call the server refuses raises `Refusal`, with the report as its `report`.
        """
        self._refuse_before_the_first_commit("leave")
        table, ignored = self._addressed(items, "leave")
        rows = _rows_of(table)
        report = ChangeReport(
            op=f"leave {layer}/{key} rank {rank}", requested=len(rows), ignored_columns=ignored
        )
        answer = C.leave(self.control, layer, key, rows, list(table), rank, level, view, strict)
        if answer.ok:
            report.refused = [
                {"row": int(one["row"]), "item": rows[int(one["row"])], "reason": one["reason"]}
                for one in answer.body.get("refused", [])
            ]
            report.accepted = len(rows) - len(report.refused)
            C.add_ignored(report.ignored_columns, answer.body)
        else:
            report.refusals.append({"status": answer.status, "detail": answer.detail[:1000]})
            raise Refusal(str(report), report)
        return report

    def status(self) -> dict:
        """The server's report on the state of this database, as a dictionary.

        The report says how far writes have got, how deep the server's queues are and how large a
        page each write request accepts. In it, `overlay` counts the items deleted or suppressed
        that no compaction has removed yet, and `layer_spaces` counts the layers in `count`, the
        deleted or suppressed layers and annotations in `depth` and the deleted ones alone in
        `retirable`. `shards.records` holds one record per shard.
        """
        self._refuse_before_the_first_commit("status")
        return _accepted(self.control.status_answer(), "status")

    def compact(self) -> dict:
        """Ask the server to remove the rows of deleted items from disc now.

        The call returns `{"request": n}` once the server has accepted the request, before the work
        is done. The server then compacts each shard that has something to reclaim, and a request
        made while a compaction runs waits until that compaction has finished. When no compaction
        can start until the server is restarted, the call raises `Refusal` with the reason and
        requests nothing.

        Follow the request in `status()`, as `Control.compact` describes.
        """
        self._refuse_before_the_first_commit("compact")
        return _accepted(self.control.compact(), "compact")

    def drop_layer(self, name: str, wait: bool = False) -> dict:
        """Remove an annotation layer or a label set, and return the server's answer.

        The layer and its uncommitted inserts also leave this database's declaration, so that the
        name can be declared again, as a new layer that none of the dropped layer's `mosaica_id`s
        names.

        - `name`: the name of the layer or label set.
        - `wait`: `True` returns only once readers no longer see the layer, with the time limit
          that `Control.flush` describes.

        The answer's `publication` names the publication from which readers no longer see the layer.
        """
        self._refuse_before_the_first_commit("drop_layer")
        answer = _accepted(self.control.drop_layer(name, wait), f"drop_layer {name}")
        self.blocks.remove_layer(name)
        self.pending = [
            one for one in self.pending
            if not (one.kind in ("layer", "labels") and one.target == name)
        ]
        self._save_state()
        return answer

    def drop_view(self, group: str, key: str, wait: bool = False) -> dict:
        """Remove one view of a view group, and return the server's answer.

        Items that the drop leaves in no view are deleted, which cannot be undone, and the
        answer's `deleted` says how many there were. Rows sent for the view that readers cannot
        see yet are dropped with it.

        - `group`, `key`: the group and the key of the view `"<group>:<key>"`.
        - `wait`: as for `drop_layer`.
        """
        self._refuse_before_the_first_commit("drop_view")
        return _accepted(self.control.drop_view(group, key, wait), f"drop_view {group}/{key}")

    def revoke(self, token) -> None:
        """End a token this database made, so that it can no longer read.

        - `token`: the `Token`, or its `token_id`.

        An id that names no live token is accepted. A reader holding the `Token` can still get
        another with its `renew()`, as `mosaica.revoke` describes.
        """
        self.serve()
        revoke(self.session_url, self.operator_credential, token)

    def _refuse_when_closed(self) -> None:
        """Refuse a call on a database whose directory is gone, as a temporary one's is once it
        is closed."""
        if self.path.is_dir():
            return
        if self.temporary:
            raise Refusal(
                "this database is closed, and as a temporary one its directory went with it; a "
                "copy made with save(path) before it closed opens again with mosaica.open(path)"
            )
        raise Refusal(
            f"the database's directory {self.path} no longer exists; mosaica.open(path) opens a "
            f"copy saved elsewhere"
        )

    def _refuse_before_the_first_commit(self, verb: str) -> None:
        self._refuse_when_closed()
        if not self.built:
            raise Refusal(
                f"{verb}: this database has not been committed, so there is nothing serving it "
                f"and no rows to address. commit() builds it first"
            )

    def serve(self) -> _instance.Listening:
        """Start the database's server if it is not running, and return the addresses it listens on.

        Reads and later commits call this themselves.
        """
        if self._held.child is not None:
            return self.listening
        self._refuse_when_closed()
        binary, _ = _instance.find_binary()
        self._held.child, self.listening = _instance.start(binary, self.path / "mosaica.toml")
        return self.listening

    @property
    def operator_credential(self) -> str:
        """The secret that gives full control of this database's server.

        `control`, `token` and `viewer` use it. The secret is kept in the database's directory,
        and the package sends it only to the database's own server on this machine.
        """
        return (self.path / ".mosaica" / "operator.cred").read_text(encoding="utf-8").strip()

    @property
    def viewer_url(self) -> str | None:
        """The address that readers read the database from, or `None` while the server is not
        running."""
        return None if self.listening is None else f"http://{self.listening.viewer}"

    @property
    def session_url(self) -> str | None:
        """The address that tokens for the database are made at, or `None` while the server is not
        running."""
        return None if self.listening is None else f"http://{self.listening.session}"

    def _run(self, arguments: Sequence[str]) -> subprocess.CompletedProcess:
        binary, _ = _instance.find_binary()
        return subprocess.run(
            [binary, *arguments],
            capture_output=True,
            text=True,
            cwd=self.path,
        )

    def _frames(self, document: dict) -> list[tuple[str, str]]:
        return [
            (block["name"], _extent_in_words(block.get("extent")))
            for block in document.get("view", [])
        ]

    def _notes(self) -> list[str]:
        """What each insert read and ignored, and what is declared and still empty."""
        notes = []
        for insert in self.inserts + self.pending:
            notes.append(
                f"{insert.kind} '{insert.target}' ({insert.role}): read "
                f"{', '.join(insert.read) or 'nothing'}; ignored "
                f"{', '.join(insert.ignored) or 'nothing'}"
            )
        filled = {insert.target for insert in self.inserts + self.pending}
        anchor = self.blocks.allocation_view()
        rows = next(
            (
                one
                for one in self.inserts + self.pending
                if one.role == "rows" and one.target == anchor
            ),
            None,
        )
        for block in self.blocks.blocks["attribute"]:
            name = block["name"]
            if name in filled or any(name in one.named_attributes for one in self.inserts):
                continue
            notes.append(f"attribute '{name}' is declared and empty: " + self._why_empty(name, rows, block))
        return notes

    def _why_empty(self, name: str, rows: Insert | None, block: dict) -> str:
        """Why no column filled this attribute, which is what the reader needs to act on."""
        if block.get(D.FILLED):
            return (
                "it was declared after the first commit, so no frame the build read carries it; "
                f"insert({name!r}, table, value=…) fills it at the next commit"
            )
        if block.get("scope"):
            return (
                "a group-scoped column belongs to one view, so it is filled by an insert of its "
                f"own: insert({name!r}, table, value=…, view=…)"
            )
        if rows is None:
            return "no frame has been inserted into the allocation view"
        if name in rows.schema:
            return (
                f"the frame inserted into '{rows.target}' carries a column of that name, and the "
                f"attribute was declared after that insert: the match is made where the frame is "
                f"handed over. Declare it first, or insert the values with "
                f"insert({name!r}, table, value=…)"
            )
        return (
            f"the frame inserted into '{rows.target}' carries no column of that name, and no "
            f"insert names one for it"
        )

    # ------------------------------------------------------------------ the directory

    def save(self, path: str | os.PathLike) -> Path:
        """Copy the database's directory to `path`, and return that path.

        Because a temporary database is deleted when it is closed, saving it is how you keep one.
        The copy is an ordinary database, which `open(path)` reopens and
        `mosaica serve --deployment <path>/mosaica.toml` serves. A Parquet file inserted from
        outside the database's directory stays where it is, and the copy reads it from there.

        - `path`: the directory to copy into, which must be empty or not yet exist.
        """
        self._refuse_when_closed()
        target = Path(path).expanduser()
        if target.exists() and any(target.iterdir()):
            raise Refusal(f"save: {target} is not empty")
        locks = shutil.ignore_patterns(_instance.LOCK, _instance.LOCK + ".new")
        shutil.copytree(self.path, target, dirs_exist_ok=True, ignore=locks)
        state = target / ".mosaica" / "declaration.json"
        if state.exists():
            # A file under the database's directory moves with it, and one inserted from
            # elsewhere stays where it lies, so the copy names each from its own place.
            original, moved = self.path.resolve(), target.resolve()
            copy = Database(target)
            _load(copy, json.loads(state.read_text(encoding="utf-8")))
            for insert in copy.inserts + copy.pending:
                path = insert.path.resolve()
                if path.is_relative_to(original):
                    insert.path = moved / path.relative_to(original)
                if insert.in_place:
                    insert.declared_path = os.path.relpath(insert.path, moved)
            copy._save_state()
            (target / "schema.toml").write_text(dumps(copy._document()), encoding="utf-8")
        return target

    def close(self) -> None:
        """Stop the database's server, and delete its directory if the database is temporary.

        After `close()` a temporary database refuses every call, because its directory is gone,
        while any other database starts its server again when a call needs it.

        The server also stops when nothing refers to the database any more, and when Python exits,
        however it exits. A temporary database's directory is deleted then too, except after a
        process that ended without running its exit handlers, such as a killed one, when a later
        `create()` removes it. A `Map` or a `Selection` refers to the database it reads, which keeps
        the server running while you hold one. Tokens the database made stay valid until they
        expire, within the hour, unless `revoke` ends them sooner.
        """
        _instance.release(self._held)
        self.listening = None

    def __enter__(self) -> "Database":
        return self

    def __exit__(self, *exception) -> None:
        self.close()


def _reason_first(what: str, output: str, log: str) -> str:
    """A refusal that says why on its first line, and keeps the whole log after it.

    The reason is each line `mosaica` marks `FAILED`, else the last line it wrote. Each line ends
    with a full stop, so the message still reads where a notebook joins its lines.
    """
    lines = [line.strip() for line in output.splitlines() if line.strip()]
    failed = [line for line in lines if line.startswith(("FAILED", "check FAILED"))]
    reasons = failed or lines[-1:] or ["mosaica wrote nothing"]
    head = f"{what}: " + "; ".join(reason.rstrip(".") for reason in reasons) + "."
    return f"{head}\nThe whole log follows.\n{log}"


def _fold_change(report: ChangeReport, answer, start: int, rows: list[dict]) -> None:
    """One page of changes onto the report: its applied count, its refused rows, each by its
    position in what the call was given and as it was given, and the columns it ignored, or the
    refusal of the whole page."""
    if not answer.ok:
        report.refusals.append({"status": answer.status, "detail": answer.detail[:1000]})
        return
    report.accepted += int(answer.body.get("accepted", 0))
    report.refused += [
        {"row": start + int(one["row"]), "item": rows[start + int(one["row"])],
         "reason": one["reason"]}
        for one in answer.body.get("refused", [])
    ]
    C.add_ignored(report.ignored_columns, answer.body)


def _address_table(items: Any, verb: str) -> pa.Table:
    """A table of addresses as Arrow: a dict of columns of one length each, or a frame."""
    if isinstance(items, dict):
        for name, values in items.items():
            if isinstance(values, (str, bytes)) or not hasattr(values, "__len__"):
                raise Refusal(
                    f"{verb}: column {name!r} is the one value {values!r}. Give each column a "
                    f"list of values, one per row, such as {{{name!r}: [{values!r}]}}"
                )
        lengths = {name: len(values) for name, values in items.items()}
        if len(set(lengths.values())) > 1:
            raise Refusal(
                f"{verb}: the columns have different lengths ("
                + ", ".join(f"{name} {n}" for name, n in lengths.items())
                + "). Give every column one value per row, None where a row names nothing by it"
            )
    if type(items).__module__.partition(".")[0] == "pandas":
        # A pandas index is not a column of the table: only the columns name items.
        return pa.Table.from_pandas(items, preserve_index=False)
    return _inserts.as_table(items)


def _rows_of(table: dict) -> list[dict]:
    """A table of addresses, `{name: [cells]}`, as one `{name: cell}` per row."""
    rows = len(next(iter(table.values()), []))
    return [{name: cells[at] for name, cells in table.items()} for at in range(rows)]


def _fits(kind: str, named: dict, table_word: str | None) -> bool:
    """Whether the columns this call names are the ones that kind of target reads."""
    roles = {
        role
        for (one, _), contract in _inserts.CONTRACTS.items()
        if one == kind
        for role in contract.required + contract.optional + contract.either
    }
    if table_word is not None:
        return (kind, table_word) in _inserts.CONTRACTS
    return bool(named) and set(named) <= roles


def _extent_in_words(extent: Any) -> str:
    if isinstance(extent, dict) and extent.get("auto"):
        margin = extent.get("margin", 0.01)
        return f"fitted to the inserted rows, with {margin:g} of the data span as headroom each side"
    if extent == "auto":
        return "fitted to the inserted rows, squared, with the build's own margin"
    return str(extent)


#: The build's closing line: `built <bundle> (v…): N items, …, M artifact(s) minted, K
#: unclustered member row(s), R row(s) refused`.
_BUILT = re.compile(
    r"^built .*: (?P<items>\d+) items, .* (?P<minted>\d+) artifact\(s\) minted, "
    r"(?P<unclustered>\d+) unclustered member row\(s\), (?P<refused>\d+) row\(s\) refused$",
    re.MULTILINE,
)


# ---------------------------------------------------------------------- create and open


def create(path: str | os.PathLike | None = None, replace: bool = False) -> Database:
    """Make a new, empty database in a directory, and return it as a `Database`.

    - `path`: the directory, which must be empty or not yet exist. Without it the database goes in a
      temporary directory, in memory where the system offers one (`/dev/shm` on Linux), which is
      deleted as `close()` describes.
    - `replace`: `True` deletes a Mosaica database already at `path` first. A non-empty directory
      that is not a Mosaica database is refused either way.

        db = mosaica.create()
        db = mosaica.create("~/maps/papers", replace=True)
    """
    if path is None:
        directory = _instance.make_temporary(RAM_BACKED if RAM_BACKED.is_dir() else None)
        database = Database(directory, temporary=True)
    else:
        directory = Path(path).expanduser()
        if directory.exists() and any(directory.iterdir()):
            if not replace:
                raise Refusal(
                    f"create: {directory} is not empty. open() reads a saved database, and "
                    f"replace=True removes a Mosaica database that is there first"
                )
            if not (directory / "mosaica.toml").exists():
                raise Refusal(
                    f"create: {directory} is not empty and holds no mosaica.toml, so it is not a "
                    f"Mosaica database. replace=True removes a database, never a directory of "
                    f"somebody else's files"
                )
            shutil.rmtree(directory)
        directory.mkdir(parents=True, exist_ok=True)
        database = Database(directory)
    (database.path / "sources").mkdir(parents=True, exist_ok=True)
    (database.path / ".mosaica").mkdir(parents=True, exist_ok=True)
    return database


def open(path: str | os.PathLike) -> Database:  # noqa: A001
    """Reopen a database kept in a directory, and return it as a `Database`.

    A committed database reopens ready to read, and its next commit adds to it. A database saved
    before its first commit reopens with its declarations and inserts as they were left.

    - `path`: the database's directory, which holds its `mosaica.toml`.

    A temporary directory that a crashed process left behind opens too, unless another live
    process has it open. The next `create()` removes it once you close it, so call `save(path)`
    on it to keep a copy.

        db = mosaica.open("~/maps/papers")
        db.view("papers").count()
    """
    directory = Path(path).expanduser()
    if not (directory / "mosaica.toml").exists():
        raise Refusal(f"open: {directory} holds no mosaica.toml. create() makes a new database")
    _instance.hold(directory)
    database = Database(directory)
    state = directory / ".mosaica" / "declaration.json"
    if state.exists():
        _load(database, json.loads(state.read_text(encoding="utf-8")))
    elif (directory / "schema.toml").exists():
        database._loaded_text = (directory / "schema.toml").read_text(encoding="utf-8")
    return database


def _arrow_type(alias: str):
    """The Arrow type a column carried, or its name where pyarrow spells no alias for it."""
    try:
        return pa.type_for_alias(alias)
    except ValueError:
        return alias


def _stored(insert: Insert) -> dict:
    """One insert as the SDK's own JSON copy holds it."""
    return {
        "target": insert.target,
        "kind": insert.kind,
        "role": insert.role,
        "columns": dict(insert.columns),
        "named_attributes": dict(insert.named_attributes),
        "items": dict(insert.items),
        "metadata_columns": dict(insert.metadata_columns),
        "source": insert.source,
        "path": str(insert.path),
        "declared_path": insert.declared_path,
        "in_place": insert.in_place,
        "rows": insert.rows,
        "schema": {c: str(t) for c, t in insert.schema.items()},
        "read": list(insert.read),
        "ignored": list(insert.ignored),
        "shape": insert.shape,
        "shape_columns": list(insert.shape_columns),
        "view_key": insert.view_key,
    }


def _restored(stored: dict) -> Insert:
    return Insert(
        target=stored["target"],
        kind=stored["kind"],
        role=stored["role"],
        columns=dict(stored["columns"]),
        named_attributes=dict(stored["named_attributes"]),
        items=dict(stored["items"]),
        metadata_columns=dict(stored["metadata_columns"]),
        source=stored["source"],
        path=Path(stored["path"]),
        declared_path=stored["declared_path"],
        in_place=stored["in_place"],
        rows=stored["rows"],
        schema={c: _arrow_type(t) for c, t in stored["schema"].items()},
        read=list(stored["read"]),
        ignored=list(stored["ignored"]),
        shape=stored["shape"],
        shape_columns=list(stored["shape_columns"]),
        view_key=stored["view_key"],
    )


def _load(database: Database, state: dict) -> None:
    database.inserts = [_restored(one) for one in state.get("inserts", [])]
    database.pending = [_restored(one) for one in state.get("pending", [])]
    for kind, blocks in state.get("blocks", {}).items():
        database.blocks.blocks[kind] = [_untagged(block) for block in blocks]
    database.blocks.shard_size = state.get("shard_size")


#: How an inline TOML table is marked in the JSON copy, to tell it from a block of its own.
INLINE = "__inline__"

#: How a TOML date-time, which a view group's metadata may hold, is marked in the JSON copy.
MOMENT = "__moment__"


def _tagged(value: Any) -> Any:
    if isinstance(value, Inline):
        return {INLINE: {k: _tagged(v) for k, v in value.items()}}
    if isinstance(value, dict):
        return {k: _tagged(v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [_tagged(v) for v in value]
    if isinstance(value, datetime.datetime):
        return {MOMENT: value.isoformat()}
    return value


def _untagged(value: Any) -> Any:
    if isinstance(value, dict):
        if set(value) == {INLINE}:
            return Inline({k: _untagged(v) for k, v in value[INLINE].items()})
        if set(value) == {MOMENT}:
            return datetime.datetime.fromisoformat(value[MOMENT])
        return {k: _untagged(v) for k, v in value.items()}
    if isinstance(value, list):
        return [_untagged(v) for v in value]
    return value


#: The integer a member key column spells "in no artifact" with, beside a null: the build and the
#: ingest route read a member key the same way (`mosaica_store::member_key`). An artifact's own key
#: of -1 is a name.
NOISE_KEY = -1


def _key_columns(insert: Insert) -> pa.Table:
    """The insert's key, level and view columns and nothing else, as the rows name them."""
    names = [insert.columns[role] for role in ("key", "level", "view") if insert.columns.get(role)]
    return _inserts.decoded(pq.read_table(insert.path, columns=list(dict.fromkeys(names))))


def _distinct(level: pa.Array, key: pa.Array, view: pa.Array) -> set[tuple]:
    """The distinct `(level, key, view)` triples of three aligned arrays, a key in decimal where
    its column is an integer one."""
    table = pa.table({
        "level": pc.cast(level, pa.int64()),
        "key": pc.cast(key, pa.string()),
        "view": pc.cast(view, pa.string()),
    }).group_by(["level", "key", "view"]).aggregate([])
    return {(row["level"] or 0, row["key"], row["view"]) for row in table.to_pylist()}


def _artifact_addresses(insert: Insert) -> set[tuple]:
    """The `(level, key, view)` an artifacts insert declares, one per row with a key."""
    table = _key_columns(insert)
    key = table[insert.columns["key"]].combine_chunks()
    present = pc.is_valid(key)

    def column(role: str) -> pa.Array:
        name = insert.columns.get(role)
        if name is None:
            return pa.nulls(len(key), pa.int64() if role == "level" else pa.string())
        return table[name].combine_chunks()

    return _distinct(
        *(pc.filter(array, present) for array in (column("level"), key, column("view")))
    )


def _member_addresses(insert: Insert, levelled: bool) -> set[tuple]:
    """The `(level, key, view)` every member row of a `key=` or `members=` insert names, read as
    the build reads a member key: a null or the integer -1 is in no artifact, and a list cell
    names one artifact per element, at the element's position on a stacked or tiered layer and
    at level 0 otherwise."""
    table = _key_columns(insert)
    key = table[insert.columns["key"]].combine_chunks()
    rows = len(key)

    def column(role: str, of: pa.DataType) -> pa.Array:
        name = insert.columns.get(role)
        return table[name].combine_chunks() if name else pa.nulls(rows, of)

    view = column("view", pa.string())
    if (
        pa.types.is_list(key.type)
        or pa.types.is_large_list(key.type)
        or pa.types.is_fixed_size_list(key.type)
    ):
        lengths = pc.fill_null(pc.list_value_length(key), 0)
        starts = pc.subtract(pc.cumulative_sum(lengths), lengths)
        parents = pc.list_parent_indices(key)
        elements = pc.list_flatten(key)
        ones = pc.fill_null(pa.nulls(len(elements), pa.int64()), 1)
        indices = pc.subtract(pc.cumulative_sum(ones), 1)
        positions = pc.subtract(indices, pc.take(pc.cast(starts, pa.int64()), parents))
        level = positions if levelled else pa.nulls(len(elements), pa.int64())
        key, view = elements, pc.take(view, parents)
    else:
        level = column("level", pa.int64())
    named = pc.is_valid(key)
    if pa.types.is_signed_integer(key.type):
        named = pc.and_(named, pc.not_equal(key, NOISE_KEY))
    return _distinct(*(pc.filter(array, named) for array in (level, key, view)))
