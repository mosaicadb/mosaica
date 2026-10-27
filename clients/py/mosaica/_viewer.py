"""The read side of the Python client: `connect`, `Viewer`, and the `Selection`, `Sample` and
`Batches` its reads return, each computed by the server over what the reader's token may see.
"""

from __future__ import annotations

import datetime
import http.client
import io
import json
import numbers
import operator
import struct
import urllib.error
import urllib.parse
import urllib.request
from typing import TYPE_CHECKING, Any, Callable, Optional, Sequence, Union

from ._auth import Token, TokenSource, minted, request_headers
from ._refusal import Refusal, listed, retry_after

if TYPE_CHECKING:
    from .widget import Map

#: How near expiry a held token may come before the next read gets another, in seconds.
TOKEN_MARGIN = 60.0

#: The `/v1/viewport` and `/v1/artifacts/viewport` fields the server reads as integers.
_VIEWPORT_INTEGERS = {"k", "artifact_budget", "underlay_offset", "per_tile", "budget", "palette_size"}

#: The frame kinds of a `/v1/viewport` body, and the artifacts frame of a
#: `/v1/artifacts/viewport` body, which ends with the same trailer. An unknown kind is refused
#: rather than skipped, since skipping one could drop data and present what is left as the whole
#: answer.
FRAME_TILES = 1
FRAME_SUB_CELLS = 2
FRAME_POINTS = 3
FRAME_TRAILER = 4
FRAME_ARTIFACTS = 5
_KINDS = {FRAME_TILES, FRAME_SUB_CELLS, FRAME_POINTS, FRAME_TRAILER}

#: The frame kinds of a `POST /v1/items` or `POST /v1/artifacts` body: a head, records frames
#: each followed by a page end, and the trailer, whose kind is the viewport's.
FRAME_RECORDS_HEAD = 6
FRAME_RECORDS = 7
FRAME_PAGE_END = 8

#: The frame kind of a `POST /v1/aggregate` table head, which comes before a table's first page in
#: each response. The body has no head of its own; its other frames are the items read's.
FRAME_TABLE_HEAD = 9


def _read_exactly(stream, size: int) -> Optional[bytearray]:
    """`size` bytes of `stream`, or `None` where the stream ends or its connection fails first.

    A server that stops a streamed body part of the way closes the connection before the body
    ends, which the HTTP client reports as an error rather than as a short read.
    """
    buffer = bytearray(size)
    view = memoryview(buffer)
    got = 0
    try:
        while got < size:
            read = stream.readinto(view[got:])
            if not read:
                return None
            got += read
    except (http.client.HTTPException, OSError):
        return None
    finally:
        view.release()
    return buffer


def _frames(stream):
    """The frames of a body as `(kind, payload)`, read from `stream` as they arrive. They stop
    where the body ends, whole or cut."""
    while True:
        header = _read_exactly(stream, 5)
        if header is None:
            return
        (length,) = struct.unpack_from("<I", header, 1)
        payload = _read_exactly(stream, length)
        if payload is None:
            return
        yield header[0], payload


def split_frames(body: bytes) -> list[tuple[int, bytearray]]:
    """A `/v1/viewport` body as `(kind, payload)` pairs, or a refusal if it is not a whole one.

    Each frame is a one-byte kind, a four-byte little-endian length and the payload. The trailer
    comes last and marks the body complete, so a body cut short is refused rather than read as a
    smaller answer.
    """
    return _checked(list(_frames(io.BytesIO(body))))


def _checked(frames: list[tuple[int, bytearray]]) -> list[tuple[int, bytearray]]:
    """`frames` if they are a whole `/v1/viewport` body, or a refusal saying what is wrong."""
    for kind, _ in frames:
        if kind not in _KINDS:
            raise Refusal(f"viewport: unknown frame kind {kind}")
    if not frames or frames[0][0] != FRAME_TILES:
        raise Refusal("viewport: the tiles frame must be first")
    if frames[-1][0] != FRAME_TRAILER:
        raise Refusal("viewport: no trailer, so the response is incomplete")
    return frames


class PartialRead(Refusal):
    """The error raised when a read from `items`, `artifacts` or `aggregate` stops part of the way,
    because a response was cut short or a later request was refused.

    - `rows`: the rows read before the stop, as a pyarrow table, or `None` when no page had
      arrived. From `aggregate` it is a list of tables, and from `batches=True` it is `None`.
    - `cursor`: the cursor to pass as `cursor` to the method that raised the error, which then
      reads the rest without repeating a row, or `None` when the read has to start again.
    - `done`: `True` when every row had arrived before the stop.
    - `why`: what stopped the read, which the message begins with.
    - `status` and `retry_after_s`: the refusal's status and wait, as on `Refusal`, when a
      refused request stopped the read, and `None` otherwise.
    """

    def __init__(self, why: str, cursor: Optional[str], done: bool, cause: Any = None) -> None:
        super().__init__(
            why,
            status=getattr(cause, "status", None),
            retry_after_s=getattr(cause, "retry_after_s", None),
        )
        self.why = why
        self.cursor = cursor
        self.done = done
        self.rows = None

    def __str__(self) -> str:
        if self.rows is None:
            kept = "The pages given before it are whole."
        elif isinstance(self.rows, list):
            kept = f"The {len(self.rows)} tables read before it are this error's `rows`."
        else:
            kept = f"The {self.rows.num_rows} rows read before it are this error's `rows`."
        if self.done:
            rest = "Every row had arrived."
        elif self.cursor is None:
            rest = "Read again from the start for the rest."
        else:
            rest = f"Pass cursor={self.cursor!r} to read the rest."
        return f"{self.why}. {kept} {rest}"


class Batches:
    """The pages of a read from `items` or `artifacts` with `batches=True`, which you iterate to get
    each page as a `pyarrow.RecordBatch`.

    Use it for a read too large to hold as one table, or one you may stop early, because each
    response is requested only when the pages before it have been taken. Because each page's
    category columns hold only the keys that page uses, `read_all()` gives each category column
    one dictionary when it joins the pages.

    - `head`: the first response's head as a dictionary. It holds `page_rows`, the page size
      used, and on an items read `order`, the order used, along with the counts when `count` was
      asked for.
    - `next`: the cursor to pass as `cursor` to read on after the last page taken.
    - `done`: `True` once the server has said that no row remains.

        for batch in db.items("papers", ["title"], page_rows=10_000, batches=True):
            frame = batch.to_pandas()
    """

    def __init__(self, reader: "Viewer", route: str, request: dict) -> None:
        self._route = route
        self.next: Optional[str] = request.get("cursor")
        self.done = False
        self._response = reader._open("POST", f"/v1/{route}", request)
        frames = _frames(self._response)
        first = next(frames, None)
        if first is None or first[0] != FRAME_RECORDS_HEAD:
            self._response.close()
            raise Refusal(f"POST /v1/{route}: the response did not begin with its head")
        self.head: dict = json.loads(first[1])
        # What each page goes through before it is given: its ids become decimal strings.
        also = ANNOTATION_IDS if route == "artifacts" else ()
        self._page = lambda page: _ids_as_text(page, also)
        # The server takes `count` on a read's first request only.
        rest = {key: value for key, value in request.items() if key != "count"}
        self._pages = self._read(reader, rest, frames)

    def _read(self, reader: "Viewer", rest: dict, frames):
        from pyarrow import ipc

        what = f"POST /v1/{self._route}"
        while True:
            pending, ended = None, False
            with self._response:
                for kind, payload in frames:
                    if kind == FRAME_RECORDS:
                        pending = ipc.open_stream(payload).read_next_batch()
                    elif kind == FRAME_PAGE_END:
                        if pending is None:
                            raise self._stopped(f"{what}: a page end has no page before it")
                        self._reached(json.loads(payload))
                        page, pending = pending, None
                        yield self._page(page)
                    elif kind == FRAME_TRAILER:
                        self._reached(json.loads(payload))
                        ended = True
                    elif kind != FRAME_RECORDS_HEAD:
                        raise self._stopped(f"{what}: a frame has the unknown kind {kind}")
            if not ended:
                raise self._stopped(f"{what}: the response ended before its trailer")
            if self.done:
                return
            try:
                request = {**rest, "cursor": self.next}
                self._response = reader._open("POST", f"/v1/{self._route}", request)
            except (Refusal, OSError, http.client.HTTPException) as refused:
                raise self._stopped(str(refused), refused) from None
            frames = _frames(self._response)

    def _reached(self, frame: dict) -> None:
        """Take the cursor of a page end or a trailer."""
        self.next = frame["next"]
        self.done = self.next is None

    def _stopped(self, why: str, cause: Any = None) -> PartialRead:
        return PartialRead(why, self.next, self.done, cause)

    def __iter__(self) -> "Batches":
        return self

    def __next__(self):
        return next(self._pages)

    def read_all(self):
        """Join the pages not yet taken into one `pyarrow.Table`.

        The table's schema metadata `mosaica.head` is `head` as JSON.
        """
        batches = []
        try:
            for batch in self:
                batches.append(batch)
        except PartialRead as stopped:
            stopped.rows = self._table(batches) if batches else None
            raise
        if not batches:
            raise Refusal(
                f"POST /v1/{self._route}: the read ended without a page, so its columns are not "
                f"known; a server answers every read with at least one page"
            )
        return self._table(batches)

    def _table(self, batches: list):
        import pyarrow as pa

        table = pa.Table.from_batches(batches).unify_dictionaries()
        return table.replace_schema_metadata({"mosaica.head": json.dumps(self.head)})

    def to_pandas(self, **options):
        """Join the pages not yet taken into one pandas DataFrame, passing `options` to pyarrow's
        `Table.to_pandas`.
        """
        return self.read_all().to_pandas(**options)

    def close(self) -> None:
        """Stop the read and free its place on the server, as dropping it does."""
        self._pages.close()
        self._response.close()


def _region(response) -> Optional[str]:
    """The response's `x-mosaica-region` verdict, or `None` where it carries none."""
    return response.headers.get("x-mosaica-region")


def _coordinates(response) -> dict:
    """The schema metadata a map answer's headers give: `mosaica.pin`, `mosaica.stale` and, where
    the response carries one, `mosaica.region`."""
    metadata = {
        "mosaica.pin": response.headers.get("x-mosaica-pin", ""),
        "mosaica.stale": "true" if response.headers.get("x-mosaica-stale") == "1" else "false",
    }
    region = _region(response)
    if region is not None:
        metadata["mosaica.region"] = region
    return metadata


def _tables(payloads: Sequence[bytes], also: Sequence[str] = ()):
    """The Arrow streams of one frame kind joined in arrival order, their ids as decimal strings
    as `_ids_as_text` makes them, or `None` if there were none."""
    import pyarrow as pa
    from pyarrow import ipc

    tables = [_ids_as_text(ipc.open_stream(payload).read_all(), also) for payload in payloads]
    return pa.concat_tables(tables) if tables else None


def _no_points():
    """The points table of an answer that served none: the two columns every answer has."""
    import pyarrow as pa

    return pa.table({"mosaica_id": pa.array([], pa.string()), "code": pa.array([], pa.uint64())})


def _no_artifacts():
    """The table of an artifacts answer that served no frame: its eighteen columns, no rows."""
    import pyarrow as pa

    ids = pa.list_(pa.field("item", pa.string(), nullable=False))
    texts = pa.list_(pa.field("item", pa.string()))
    return pa.schema(
        [
            pa.field("layer", pa.dictionary(pa.uint16(), pa.string()), nullable=False),
            pa.field("mosaica_id", pa.string(), nullable=False),
            ("key", pa.string()),
            pa.field("masked_count", pa.uint64(), nullable=False),
            ("centroid_x", pa.float64()),
            ("centroid_y", pa.float64()),
            ("box_min_x", pa.uint32()),
            ("box_min_y", pa.uint32()),
            ("box_max_x", pa.uint32()),
            ("box_max_y", pa.uint32()),
            pa.field("content", texts, nullable=False),
            pa.field("parent_ids", ids, nullable=False),
            pa.field("rung", pa.uint32(), nullable=False),
            ("matched", pa.bool_()),
            ("highlighted", pa.bool_()),
            ("target", pa.string()),
            ("tile", pa.uint32()),
            ("slot", pa.uint8()),
        ]
    ).empty_table()


def _tile_counts(counted) -> dict:
    """The tiles frame's counts, as a pyarrow table, summed over its tiles."""
    return {
        name: sum(int(v) for v in counted.column(name).to_pylist())
        for name in ("visible", "matched", "highlighted", "served")
        if name in counted.column_names
    }


def _category_columns(meta: dict, view: str) -> dict:
    """Each category column a view's points can carry, with the view its codes are read in.

    A column declared for a view group has codes per view, so it is looked up in `view`; any
    other column has one set of codes, looked up with no view.
    """
    found = {one["name"]: None for one in meta.get("declared_scalars") or [] if one.get("category")}
    for one in meta.get("scoped_scalars") or []:
        if one.get("category"):
            found[one["name"]] = view
    return found


def _as_keys(codes, keys: dict):
    """A column of category codes as a dictionary column of keys, null where no key is known."""
    import pyarrow as pa
    import pyarrow.compute as pc

    codes = codes.combine_chunks() if isinstance(codes, pa.ChunkedArray) else codes
    known = [code for code in pc.unique(codes).to_pylist() if code in keys]
    indices = pc.index_in(codes, value_set=pa.array(known, codes.type))
    return pa.DictionaryArray.from_arrays(
        indices, pa.array([keys[code] for code in known], pa.string())
    )


def _extent(meta: dict, view: str) -> list[float]:
    """A view's whole coordinate range, `[min_x, min_y, max_x, max_y]`."""
    for block in meta.get("views") or []:
        if block["id"] == view:
            q = block["quantisation"]
            return [q["x_min"], q["y_min"], q["x_max"], q["y_max"]]
    raise Refusal(f"there is no view named {view!r} that this reader can see")


U64_MAX = 2**64 - 1


def decimal_id(value: Any) -> Optional[str]:
    """A `mosaica_id` as the decimal string of a `u64`, or `None` where `value` is not one: a
    whole number from 0 to 2^64 - 1, such as an `int`, a NumPy or pyarrow integer, or its
    decimal digits. A float is not one, since one past 2^53 has lost its last digits."""
    if hasattr(value, "as_py"):
        value = value.as_py()
    if isinstance(value, bool):
        return None
    try:
        whole = operator.index(value)
    except TypeError:
        whole = None
    if whole is not None:
        return str(whole) if 0 <= whole <= U64_MAX else None
    if isinstance(value, str) and value.isascii() and value.isdigit() and int(value) <= U64_MAX:
        return value
    return None


#: The columns of an annotation read that carry `mosaica_id`s besides its own. A read of items
#: holds the reader's attributes, which may carry any of these names, so it converts none of them.
ANNOTATION_IDS = ("target", "parents", "parent_ids")


def _ids_as_text(data, also: Sequence[str] = ()):
    """A table or record batch read back from the server, with every column carrying
    `mosaica_id`s as their decimal strings: a `u64` past 2^53 is not exact as a float, which is
    what pandas makes of an integer column holding a null. Those are `mosaica_id`, a sample's
    `membership:<layer>` columns, and the columns `also` names that carry them in this read, such
    as an annotation's `target` or an aggregate's `key` grouped by a layer."""
    import pyarrow as pa

    fields, columns, changed = [], [], False
    for field, column in zip(data.schema, data.columns):
        if field.name == "mosaica_id" or field.name in also or field.name.startswith("membership:"):
            if pa.types.is_integer(field.type):
                field, column = field.with_type(pa.string()), column.cast(pa.string())
                changed = True
            elif pa.types.is_list(field.type) and pa.types.is_integer(field.type.value_type):
                text = pa.list_(field.type.value_field.with_type(pa.string()))
                field, column = field.with_type(text), column.cast(text)
                changed = True
        fields.append(field)
        columns.append(column)
    if not changed:
        return data
    schema = pa.schema(fields, metadata=data.schema.metadata)
    if isinstance(data, pa.RecordBatch):
        return pa.RecordBatch.from_arrays(columns, schema=schema)
    return pa.Table.from_arrays(columns, schema=schema)


def _an_id(value: Any, what: str) -> str:
    """`value` as the decimal string a route's path takes, or a refusal before the request."""
    text = decimal_id(value)
    if text is None:
        raise Refusal(
            f"{what}: {value!r} is not a mosaica_id; write a whole number from 0 to 2^64 - 1, as "
            f"a sample's or a read's mosaica_id column holds it"
        )
    return text


_EPOCH = datetime.datetime(1970, 1, 1, tzinfo=datetime.timezone.utc)
_MICROSECOND = datetime.timedelta(microseconds=1)


def _microseconds(value: Any) -> Optional[int]:
    """A moment as the microseconds since the Unix epoch a `timestamp_us` column holds, or `None`
    where `value` is not a moment. A naive moment is read as UTC, as Arrow stores a naive
    timestamp column."""
    # NumPy's and pandas' NaT, the second a datetime whose arithmetic answers NaN.
    if type(value).__name__ in ("datetime64", "NaTType") and str(value) == "NaT":
        raise Refusal("a filter value is NaT, which is no moment; write a date or a datetime")
    if type(value).__name__ == "datetime64":
        return int(value.astype("datetime64[us]").astype("int64"))
    if isinstance(value, datetime.datetime):
        try:
            aware = value if value.tzinfo is not None else value.replace(
                tzinfo=datetime.timezone.utc
            )
            return (aware - _EPOCH) // _MICROSECOND
        except (TypeError, ValueError):
            raise Refusal(
                f"a filter value is {value!r}, which is no moment; write a date or a datetime"
            ) from None
    if isinstance(value, datetime.date):
        utc = datetime.timezone.utc
        midnight = datetime.datetime(value.year, value.month, value.day, tzinfo=utc)
        return (midnight - _EPOCH) // _MICROSECOND
    return None


def _json_value(value: Any) -> Any:
    """What `json` sends for a value it has no encoding of: a moment as its microseconds, and a
    NumPy number as the number."""
    micros = _microseconds(value)
    if micros is not None:
        return micros
    if isinstance(value, numbers.Integral):
        return int(value)
    if isinstance(value, numbers.Real):
        return float(value)
    raise Refusal(
        f"a request holds {value!r}, a {type(value).__name__}, which cannot be sent; write a "
        f"number, a string, a date or a datetime"
    )


def _filter_columns(expression: Any) -> set[str]:
    """The columns a filter expression's leaves name."""
    if not isinstance(expression, dict):
        return set()
    named: set[str] = set()
    for key, value in expression.items():
        if key in ("all_of", "any_of", "none_of"):
            for one in value if isinstance(value, list) else []:
                named |= _filter_columns(one)
        elif key not in ("region", "member_of"):
            named.add(key)
    return named


def _require(name: str, names: list[str], what: str) -> None:
    """Refuse a name this reader's meta does not list, naming those it does."""
    if name not in names:
        raise Refusal(
            f"there is no {what} named {name!r} that this reader can see. Name one of: "
            f"{', '.join(names) or 'none, since there are none'}"
        )


def _layers(layers: Any) -> Any:
    """The layers a request names: `"all"`, or a list of names, one name being a list of one."""
    if layers is None or isinstance(layers, str) and layers == "all":
        return layers
    return listed(layers)


def _query(params: dict) -> str:
    return "?" + urllib.parse.urlencode(params) if params else ""


def _all_of(expressions: Sequence[dict]) -> Optional[dict]:
    """Several filter expressions as one, with any top-level `all_of` opened into its parts."""
    parts: list[dict] = []
    for expression in expressions:
        if set(expression) == {"all_of"}:
            parts.extend(expression["all_of"])
        else:
            parts.append(expression)
    if not parts:
        return None
    return parts[0] if len(parts) == 1 else {"all_of": parts}


class Sample:
    """The points a map draws at one zoom, as `Selection.sample` returns them: a pyarrow table
    thinned for drawing, with the counts it was drawn from in its metadata.

    A `Sample` behaves as its table, `points`, and has these columns.

    - `mosaica_id`: the item's id, as its decimal string.
    - `code`: the point's position as a 64-bit Morton code over a grid of 2^32 steps per axis
      across the view. It is absent under `point_rows="highlight"`.
    - Each rendered column that `point_rows` asks for. A category column holds each value's key,
      or null for a value this reader may not see.
    - `highlighted`: whether the point matches the `highlight`, when one was given.
    - `membership:<layer>`: for each layer named in `layers` that tags a point, the `mosaica_id`
      of the finest annotation of that layer holding the point, as its decimal string, or null.

    The schema metadata `mosaica.counts` holds the counts the sample was drawn from: `visible`,
    the items this reader may see in the tiles touched, and of those `matched`, `highlighted` and
    `served`, the rows in this table.

    The metadata also says which version of the data the sample was drawn from, so that you can
    tell whether it has moved on. `mosaica.pin` is that version as JSON, which you pass back
    decoded as `pin` on a later `sample` or `viewport_artifacts`, and that later answer's
    `mosaica.stale` is then `"true"` when the version has changed since. The version moves at
    every flush, merge and compaction, so `"true"` says that what this reader sees may have
    changed. After a flush, a reader the view has already served keeps the previous version until
    the server's refresh reaches its session, so the flush's new items appear in its answers only
    then, while deletions and suppressions apply at once.

    When the selection has a box or a `region` filter, `mosaica.region` says whether it was
    answered exactly, as `"exact"`, or over the grid cells covering it, as `"cover; depth=<d>"`.

    `sub_cells` is the table of finer counts that `underlay_offset` asks for, or `None`.
    """

    def __init__(self, points, sub_cells=None):
        self.points = points
        self.sub_cells = sub_cells

    def __getattr__(self, name: str):
        # Guards against recursion while `__init__` has not yet set these.
        if name in ("points", "sub_cells"):
            raise AttributeError(name)
        return getattr(self.points, name)

    def __getitem__(self, key):
        return self.points[key]

    def __len__(self) -> int:
        return self.points.num_rows

    def __repr__(self) -> str:
        served = [f"{self.points.num_rows} points"]
        if self.sub_cells is not None:
            served.append(f"{self.sub_cells.num_rows} sub-cells")
        return f"Sample({', '.join(served)})"


class Selection:
    """A part of one view, narrowed by filters and a box, that you count, sample, read or map as one
    reader sees it.

    `db.view(name)` and `viewer.view(name)` give a whole view. Calling `filter` or `within` gives
    a new, narrower selection and leaves the original unchanged, so one selection can be the
    start of several.

        papers = db.view("papers")
        recent = papers.filter({"year": {"range": {"gte": 2020}}})
        corner = recent.within((0, 0, 10, 10))
        corner.count()
    """

    def __init__(
        self,
        reader: Callable[[], "Viewer"],
        view: str,
        filters: tuple = (),
        boxes: tuple = (),
        items: Optional[Callable] = None,
    ) -> None:
        self._reader = reader
        self._view = view
        self._filters = filters
        self._boxes = boxes
        # How `items` reads: a database's own `items`, which reads its ids back as inserted, or
        # the reader's.
        self._items = items

    @property
    def view(self) -> str:
        """The name of the view this selection is part of."""
        return self._view

    @property
    def filters(self) -> Optional[dict]:
        """The filters added so far, joined into one expression, or `None` when there are none."""
        return _all_of(self._filters)

    @property
    def box(self) -> Optional[tuple]:
        """The box this selection is limited to, as `(min_x, min_y, max_x, max_y)`, or `None`.

        After several calls to `within` it is the overlap of their boxes, and `None`, matching no
        item, when they do not overlap.
        """
        if not self._boxes:
            return None
        overlap = (
            max(box[0] for box in self._boxes),
            max(box[1] for box in self._boxes),
            min(box[2] for box in self._boxes),
            min(box[3] for box in self._boxes),
        )
        if overlap[0] > overlap[2] or overlap[1] > overlap[3]:
            return None
        return overlap

    def filter(self, expression: dict) -> "Selection":
        """Narrow this selection to the items that also match `expression`, returning a new
        selection.

        `expression` is a filter written as a dictionary that maps a column name to a test, or
        that applies `all_of`, `any_of` or `none_of` to a list of expressions. An item must match
        every filter added.

        A test on a timestamp column takes a `datetime`, a `date`, a pandas `Timestamp` or a
        NumPy `datetime64`, and the client sends it as microseconds since the Unix epoch. A
        moment with no time zone is read as UTC, and a date stands for its midnight in UTC. A
        number or a string, such as an RFC 3339 date, is sent as it is.

            db.view("papers").filter({"year": {"range": {"gte": 2020, "lt": 2024}}})
            db.view("papers").filter({"submitted": {"range": {"gte": datetime.date(2020, 1, 1)}}})
        """
        if not isinstance(expression, dict) or not expression:
            raise Refusal(
                f"filter takes one expression as a dictionary, such as "
                f"{{'year': {{'range': {{'gte': 2020}}}}}}; got {expression!r}"
            )
        return Selection(
            self._reader, self._view, self._filters + (expression,), self._boxes, self._items
        )

    def within(self, box: Sequence[float]) -> "Selection":
        """Narrow this selection to the items whose position lies inside `box`, returning a new
        selection.

        `box` is `(min_x, min_y, max_x, max_y)` in the view's own coordinates, the ones the rows
        were inserted with. An item on the edge is inside, and a box whose minimum is above its
        maximum is refused.

            db.view("papers").within((0.0, 0.0, 100.0, 50.0)).count()
        """
        edges = tuple(float(v) for v in box)
        if len(edges) != 4:
            raise Refusal(f"within takes (min_x, min_y, max_x, max_y); got {tuple(box)!r}")
        if edges[0] > edges[2] or edges[1] > edges[3]:
            raise Refusal(
                f"within: {edges} has a minimum above its maximum. Write "
                f"(min_x, min_y, max_x, max_y)"
            )
        boxes = self._boxes + (edges,)
        return Selection(self._reader, self._view, self._filters, boxes, self._items)

    def count(self) -> int:
        """Count the items in this selection that its reader may see.

        When the box's outline crosses more grid cells than the server's `max_region_cells`
        setting allows, the server counts the cells that cover the box, so the count can include
        items outside it. `sample(k=0)` gives the same count as `matched` in its `mosaica.counts`,
        with the verdict on the box in `mosaica.region`.

            db.view("papers").count()
        """
        request = {"view": self._view, "zoom": 0, "tiles": [0], "k": 0}
        expression = self._expression()
        if expression is not None:
            request["filters"] = expression
        body = self._reader()._request("POST", "/v1/viewport", request)
        tiles = _tables([p for kind, p in split_frames(body) if kind == FRAME_TILES])
        return _tile_counts(tiles).get("matched", 0)

    def sample(
        self,
        zoom: int = 0,
        k: Optional[int] = None,
        *,
        tiles: Optional[Sequence[int]] = None,
        highlight: Optional[dict] = None,
        layers: Any = None,
        levels: Any = None,
        artifact_budget: Optional[int] = None,
        point_rows: Union[str, Sequence[str], None] = None,
        underlay_offset: Optional[int] = None,
        pin: Any = None,
        on_counts: Optional[Callable[[Any, Any], None]] = None,
    ) -> Sample:
        """Sample the points a map of this selection draws at `zoom`, returning them as a `Sample`.

        The server thins dense areas so that each map tile carries at most `k` points.

        - `zoom`: the map's zoom level from 0, the whole view as one tile, to 16.
        - `k`: the most points drawn in one tile. It defaults to the server's `k_max_marks` and
          cannot exceed its `max_k`.
        - `tiles`: the tiles to sample, each by its number in Z order at this zoom, in place of
          the box.
        - `highlight`: a second filter that adds the `highlighted` column without changing which
          points are drawn.
        - `layers`: the annotation layers, such as clusters, whose annotations tag the points, as
          a list of names or `"all"`.
        - `levels`: which levels of a levelled layer tag the points, as a list of numbers or
          `"all"`. Without it each layer uses the levels it declares for this zoom.
        - `artifact_budget`: the most annotations a `nested` or `dag` layer tags points with, met
          by using parents in place of their children.
        - `point_rows`: the rendered columns each point carries, as a list, where `[]` is the
          position alone. `"full"`, the default, is every rendered column, and `"highlight"`
          returns only `mosaica_id` and `highlighted` to update the highlight on points you
          already hold.
        - `underlay_offset`: also count the items this reader may see in the tiles this many
          levels finer than `zoom`, ignoring the filters, and return those counts as `sub_cells`.
        - `pin`: the decoded `mosaica.pin` of an earlier answer, whose version this answer's
          `mosaica.stale` is compared with. It changes none of the points or counts.
        - `on_counts`: a function called with the per-tile counts and `sub_cells` as pyarrow
          tables as soon as they arrive, before the points.

        Each option is sent only when given, and the server's own setting applies otherwise.

            sample = db.view("papers").sample(zoom=3, k=256)
            sample.to_pandas()  # needs pandas installed
        """
        reader = self._reader()
        meta = reader.meta()
        request: dict = {"view": self._view, "zoom": int(zoom)}
        if tiles is not None:
            request["tiles"] = [int(tile) for tile in listed(tiles)]
        else:
            request["bbox"] = list(self.box or _extent(meta, self._view))
        for name, value in (
            ("k", k),
            ("filters", self._expression()),
            ("highlight", highlight),
            ("layers", _layers(layers)),
            ("levels", levels),
            ("artifact_budget", artifact_budget),
            (
                "point_rows",
                point_rows
                if point_rows is None
                or isinstance(point_rows, str) and point_rows in ("full", "highlight")
                else listed(point_rows),
            ),
            ("underlay_offset", underlay_offset),
            ("pin", pin),
        ):
            if value is not None:
                request[name] = int(value) if name in _VIEWPORT_INTEGERS else value
        frames = []
        # The counts are decoded once, as they land, and handed to `on_counts` then.
        tiles = sub_cells = None
        # The request as sent, which the answer's metadata carries too.
        request = json.loads(reader._encoded(request))
        with reader._open("POST", "/v1/viewport", request) as response:
            coordinates = _coordinates(response)
            for kind, payload in _frames(response):
                frames.append((kind, payload))
                if kind == FRAME_TILES:
                    tiles = _tables([payload])
                elif kind == FRAME_SUB_CELLS:
                    sub_cells = _tables([payload])
                counted = kind == (FRAME_SUB_CELLS if underlay_offset else FRAME_TILES)
                if on_counts is not None and counted and tiles is not None:
                    on_counts(tiles, sub_cells)
        frames = _checked(frames)
        # A points frame with no rows is still the server's schema, so test for None, not falsity.
        points = _tables([p for kind, p in frames if kind == FRAME_POINTS])
        if points is None:
            points = _no_points()
        for column, view in _category_columns(meta, self._view).items():
            if column not in points.column_names:
                continue
            at = points.column_names.index(column)
            keyed = _as_keys(points.column(at), reader._keys_of(column, view, points.column(at)))
            points = points.set_column(at, column, keyed)
        trailer = json.loads(next(p for kind, p in frames if kind == FRAME_TRAILER).decode())
        if points.num_rows != trailer.get("points", points.num_rows):
            raise Refusal(
                f"viewport: the trailer says {trailer['points']} points and the body carries "
                f"{points.num_rows}"
            )
        return Sample(
            points.replace_schema_metadata(
                {
                    "mosaica.counts": json.dumps(_tile_counts(tiles)),
                    "mosaica.trailer": json.dumps(trailer),
                    "mosaica.request": json.dumps(request),
                    **coordinates,
                }
            ),
            sub_cells,
        )

    def items(self, fields: Sequence[str], **options):
        """Read every item in this selection, with the fields named, as one pyarrow table.

        The selection's filters and box are sent as the read's `filters`, so the rows are the
        items `count()` counts. `options` are the other keywords of `Viewer.items`. `filters` is
        refused because the selection supplies its own.

            db.view("papers").filter({"year": {"eq": 2023}}).items(["title"]).to_pandas()
        """
        if "filters" in options:
            raise Refusal(
                "items on a selection sends the selection's own filters; narrow it with "
                ".filter(expression) and call items on the result"
            )
        read = self._items or self._reader().items
        expression = self._expression()
        if expression is not None:
            options["filters"] = expression
        return read(self._view, fields, **options)

    def aggregate(
        self,
        groupings: Sequence[dict],
        reference: Optional[dict] = None,
        *,
        cursor: Optional[str] = None,
    ) -> list:
        """Count how the items in this selection are distributed, as one pyarrow table per grouping.

        The selection's filters and box are sent as the request's `filters`, so each table counts
        the items `count()` counts. `groupings`, `reference` and `cursor` are as `Viewer.aggregate`
        takes them, and a field figure such as `"min"` is refused on a selection with a filter or a
        box.

            papers = db.view("papers").filter({"year": {"range": {"gte": 2020}}})
            [size, venues] = papers.aggregate(
                [{"count": True}, {"count": True, "by": {"field": "venue", "top": 10}}]
            )
        """
        return self._reader().aggregate(
            self._view, groupings, self._expression(), reference, cursor=cursor
        )

    def map(
        self,
        colour_by: Optional[str] = None,
        layers: Optional[Sequence[str]] = None,
        height: int = 480,
        size_by: Optional[str] = None,
        artifacts_per_tile: Optional[int] = None,
        budget: int = 0,
        budget_min: int = 1_000,
        budget_max: int = 2_000_000,
        cluster_budget: int = 1_000,
        cluster_budget_min: int = 10,
        cluster_budget_max: int = 10_000,
        palette: Optional[str] = None,
        value_colours: Optional[dict] = None,
        cluster_colours: Optional[dict] = None,
        basemap: Any = None,
        **kwargs: Any,
    ) -> Map:
        """Open the interactive map of this selection as a notebook widget.

        The map opens on this view with the selection's filters applied, framed on the box when
        there is one and otherwise on where the items are. The box only frames the camera, so
        items outside it are drawn when they are in frame.

        The parameters are those of [`Map`][mosaica.widget.Map], apart from `view`, `filters`
        and `bbox`, which the selection gives. A layer or column this reader cannot see is
        refused, as on `Viewer.map`.

            db.view("papers").filter({"year": {"range": {"gte": 2020}}}).map(colour_by="venue")
        """
        given = sorted(set(kwargs) & {"view", "filters", "bbox"})
        if given:
            raise Refusal(
                f"map: a selection's map takes {', '.join(given)} from the selection; narrow it "
                f"with filter() or within(), or call map() on its reader"
            )
        return self._reader().map(
            view=self._view,
            layers=layers,
            colour_by=colour_by,
            filters=self.filters,
            height=height,
            bbox=self.box,
            size_by=size_by,
            artifacts_per_tile=artifacts_per_tile,
            budget=budget,
            budget_min=budget_min,
            budget_max=budget_max,
            cluster_budget=cluster_budget,
            cluster_budget_min=cluster_budget_min,
            cluster_budget_max=cluster_budget_max,
            palette=palette,
            value_colours=value_colours,
            cluster_colours=cluster_colours,
            basemap=basemap,
            **kwargs,
        )

    def _expression(self) -> Optional[dict]:
        """The filters and every box, as the one expression the server tests items against."""
        boxes = [{"region": {"bbox": list(box)}} for box in self._boxes]
        return _all_of(list(self._filters) + boxes)

    def __repr__(self) -> str:
        parts = [f"view={self._view!r}"]
        if self._filters:
            parts.append(f"filters={self.filters!r}")
        if self._boxes:
            parts.append(f"box={self.box!r}")
        return f"Selection({', '.join(parts)})"


class Viewer:
    """The reader of one Mosaica database over HTTP, used to count, sample, read and map what its
    token may see.

    A reader holds a set of access terms and sees an item when its terms satisfy one of the
    item's access labels, such as `secret&(team_a|team_b)`. Every count, map and record it is
    given is computed over the items it may see. Get a reader with `connect(url, token)` for a
    database someone else runs, or with `db.viewer(terms)` for one of your own.

    A read the server is too busy to take raises `Refusal` with `status` 429 and the wait in
    `retry_after_s`, and the reader does not retry it.

    - `url`: the address of the database's viewer plane.
    - `token`: the token to read with, in any of the forms `TokenSource` describes, or `None` to
      send none, as a server started with `mosaica serve --open` allows.
    - `terms`: the access terms the token was made for, kept for display, or `None` when they are
      not known.
    """

    def __init__(
        self, url: str, token: Optional[TokenSource] = None, *, terms: Optional[Sequence[str]] = None
    ):
        if not url:
            raise Refusal("a viewer needs the address of the database's reading endpoint")
        self.url = url.rstrip("/")
        self._source: Optional[TokenSource] = token
        self._token: Optional[Token] = None
        #: Category keys already looked up, by `(column, view)` and then by code.
        self._keys: dict = {}
        #: The access terms this reader's token was made for, or `None` when the token came from
        #: elsewhere and does not say.
        self.terms = None if terms is None else listed(terms)

    def token(self) -> Optional[Token]:
        """Return the token this reader sends, or `None` for a reader made without one.

        A token close to expiry is renewed first, as `TokenSource` describes.
        """
        if self._source is None:
            return None
        held = self._token
        if held is not None and (held.seconds_left is None or held.seconds_left > TOKEN_MARGIN):
            return held
        if held is not None and held.renew is not None:
            self._token = held.renew()
            return self._token
        self._token = minted(self._source)
        return self._token

    def view(self, name: str) -> Selection:
        """Return the whole of one view as a `Selection` to narrow, count, sample, read or map.

        A view is one layout of the items, with its own coordinates. `name` is as `meta()` lists
        it, `"<group>:<key>"` for a view in a view group. An unknown name is refused with a list
        of those this reader can see.

            v.view("papers").count()
        """
        self._require_view(name)
        return Selection(lambda: self, name)

    def map(
        self,
        view: Optional[str] = None,
        layers: Optional[Sequence[str]] = None,
        colour_by: Optional[str] = None,
        filters: Optional[dict] = None,
        height: int = 480,
        size_by: Optional[str] = None,
        basemap: Any = None,
        **kwargs: Any,
    ) -> Map:
        """Open the interactive map of this database as a notebook widget, showing what this reader
        may see.

        The parameters are those of [`Map`][mosaica.widget.Map], which fetches its data with this
        reader's token. A view, layer or column that this reader's `meta()` does not list is
        refused, and the refusal names those it does.
        """
        from .widget import Map

        meta = self.meta()
        if view is not None:
            self._require_view(view, meta)
        named = [layer["name"] for layer in meta.get("layers") or []]
        for layer in [] if layers is None else listed(layers):
            _require(layer, named, "layer")
        rendered = [
            column["name"]
            for column in (meta.get("declared_scalars") or []) + (meta.get("scoped_scalars") or [])
            if column.get("render")
        ]
        if colour_by is not None:
            colours = rendered + [f"cluster:{layer}" for layer in named]
            _require(colour_by, colours, "rendered column or cluster:<layer> to colour by")
        if size_by is not None:
            _require(size_by, rendered, "rendered column to size by")
        if filters is not None:
            filters = json.loads(self._encoded({"filters": filters}))["filters"]
        return Map(
            self.url,
            token=self.token,
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
        """Return the structure of the database that this reader may see, as a dictionary.

        It lists the views, layers and columns the other reads name, with how each column can be
        filtered and the server's limits.
        """
        return json.loads(self._request("GET", "/v1/meta", None))

    def item(self, mosaica_id: Any) -> dict:
        """Return one item's full record as a dictionary, when this reader may see the item.

        - `mosaica_id`: the item's id, from a sample, a read or a map pick.

        An item this reader may not see is refused in the same way as one that does not exist.
        The record has these keys.

        - `fields`: the item's values by column name, with no entry where it has no value.
        - `labels`: why this reader may see the item, as the parts of its access labels that this
          reader satisfies.
        - `views`: the views this reader can find the item in, each with its position there in
          grid units.
        - `scoped`: its values of columns declared for a view group, by column and then by the
          group's key.

            v.item(sample.column("mosaica_id")[0].as_py())
        """
        return json.loads(self._request("POST", f"/v1/items/{_an_id(mosaica_id, 'item')}", {}))

    def items(
        self,
        view: str,
        fields: Sequence[str],
        *,
        position: Optional[bool] = None,
        labels: Optional[bool] = None,
        filters: Optional[dict] = None,
        keep_unmatched: Optional[bool] = None,
        count: Optional[bool] = None,
        order: Optional[str] = None,
        page_rows: Optional[int] = None,
        pages: Optional[int] = None,
        cursor: Optional[str] = None,
        compression: Optional[str] = None,
        batches: bool = False,
    ):
        """Read every item this reader may see in `view`, with the fields named, as one pyarrow
        table.

        The server answers a read a page at a time, and `items` requests responses until no row
        remains and joins their pages. With `batches=True` it returns a `Batches` instead, to
        take the pages one at a time. Each option is sent only when given, and the server's own
        setting applies otherwise.

        - `view`: the view to read, as `meta()` names it. An item with no position in it is not
          returned.
        - `fields`: the declared columns to return, in this order. A column declared for a view
          group, read under a view outside it, is named `"<column>@<key>"`.
        - `position`: `True` adds `mosaica:x` and `mosaica:y`, each item's position in the view's
          coordinates, in degrees on a geographic view.
        - `labels`: `True` adds `mosaica:labels`, the parts of each item's labels that this
          reader satisfies, as `item` gives them.
        - `filters`: a filter expression, as `Selection.filter` takes one, that the items
          returned must match.
        - `keep_unmatched`: `True` returns every item, with a `mosaica:matched` column saying
          whether it matches `filters`.
        - `count`: `True` puts in the head the number of items this reader may see in the view,
          as `visible`, and of those that match, as `matched`. It cannot be combined with
          `cursor`.
        - `order`: `"map"` returns the items by their place on the map and `"stored"` in the
          order the server stores them, the faster order for a field that is neither rendered nor
          indexed. Without it the server chooses.
        - `page_rows`: the rows in a page, capped at the server's `max_page_rows`.
        - `pages`: the most pages in one response.
        - `cursor`: the `next` of an earlier read or the `cursor` of a `PartialRead`, to read the
          rows after it.
        - `compression`: `"zstd"` compresses the pages on their way from the server.

        The table holds `mosaica_id`, as its decimal string, and then the columns asked for, with
        each category column holding its values' keys. A read that finds no item returns a table of no rows with those
        columns. Its schema metadata `mosaica.head` is the head that `Batches.head` describes.

        A response cut short, or a later request refused, raises a `PartialRead`.

            papers = db.items("papers", ["title", "year"], position=True)
            papers.to_pandas()
            recent = db.items("papers", ["title"], filters={"year": {"range": {"gte": 2020}}})
        """
        request: dict = {"view": view, "fields": listed(fields)}
        given = {
            "position": position,
            "labels": labels,
            "filters": filters,
            "keep_unmatched": keep_unmatched,
            "count": count,
            "order": order,
            "page_rows": page_rows,
            "pages": pages,
            "cursor": cursor,
            "compression": compression,
        }
        return self._bulk_read("items", request, given, batches)

    def lookup(self, view: str, field: str, values: Sequence[Any], fields: Sequence[str] = ()):
        """Find the items in `view` that hold `values` in the unique column `field`, returning them
        as a pyarrow table with `mosaica_id`, `field` and the columns in `fields`.

        A value with no row is held by no item this reader may see.

            v.lookup("papers", "doi", ["10.1/a", "10.2/b"], fields=["title"])
        """
        return self.items(view, [field, *listed(fields)], filters={field: {"in": listed(values)}})

    def artifacts(
        self,
        view: str,
        layer: str,
        fields: Sequence[str],
        *,
        level: Optional[int] = None,
        parent: Any = None,
        q: Optional[str] = None,
        ids: Optional[Sequence[Any]] = None,
        filters: Optional[dict] = None,
        keep_unmatched: Optional[bool] = None,
        count: Optional[bool] = None,
        page_rows: Optional[int] = None,
        pages: Optional[int] = None,
        cursor: Optional[str] = None,
        compression: Optional[str] = None,
        palette_size: Optional[int] = None,
        batches: bool = False,
    ):
        """Read every annotation of `layer` that this reader may see, with the properties named, as
        one pyarrow table.

        An annotation is one member of an annotation layer, such as a cluster, a region or a node
        in a taxonomy. The read is paged and resumed as an `items` read is. Its `mosaica_id`,
        `parents` and `target` columns hold decimal strings.

        - `view`: the view whose items the counts and geometry are computed over.
        - `layer`: the layer to read, as `meta()` lists it.
        - `fields`: the properties to return, in order, from `"key"`, `"level"`, `"parents"`,
          `"target"` (the annotation it is attached to), `"masked_count"` (its items this reader
          may see), `"content"`, `"centroid"`, `"box"`, `"shape"` and `"slot"`. `"centroid"`
          gives the columns `centroid_x` and `centroid_y`, and `"box"` gives `box_x_min`,
          `box_y_min`, `box_x_max` and `box_y_max`, all in the view's coordinates.
        - `level`: return only the annotations at this level.
        - `parent`: return only the children of this annotation, given by its `mosaica_id`.
        - `q`: return only the annotations whose key or first text contains this, ignoring case.
          It cannot be combined with `parent`.
        - `ids`: return only the annotations these `mosaica_id`s name, such as the values of a
          sample's `membership:<layer>` column. An id that names nothing, or an annotation this
          reader is not served, gets no row.
        - `filters`: return only the annotations holding a matching item, with their
          `matched_count`.
        - `keep_unmatched`: with `filters`, return every annotation, with a `matched_count` of 0
          where nothing matches.
        - `count`: put in the head `served`, the annotations the request selects before `filters`,
          and `matched`, those of them that match. It cannot be combined with `cursor`.
        - `page_rows`, `pages`, `cursor`, `compression`, `batches`: as for `items`.
        - `palette_size`: how many colours your palette holds, to fill `slot` as
          `viewport_artifacts` describes.

        The rows come in order of level, and within a level in the order the annotations were
        published.

            clusters = db.artifacts("papers", "clusters", ["key", "masked_count", "centroid"])
        """
        request: dict = {"view": view, "layer": layer, "fields": listed(fields)}
        given = {
            "level": level,
            "parent": None if parent is None else _an_id(parent, "artifacts"),
            "q": q,
            # Decimal strings: an id past 2^53 is not exact as a JSON number.
            "ids": None if ids is None else [_an_id(one, "artifacts") for one in listed(ids)],
            "filters": filters,
            "keep_unmatched": keep_unmatched,
            "count": count,
            "page_rows": page_rows,
            "pages": pages,
            "cursor": cursor,
            "compression": compression,
            "palette_size": None if palette_size is None else int(palette_size),
        }
        return self._bulk_read("artifacts", request, given, batches)

    def viewport_artifacts(
        self,
        view: str,
        zoom: int,
        per_tile: int,
        *,
        tiles: Optional[Sequence[int]] = None,
        bbox: Optional[Sequence[float]] = None,
        layers: Any = None,
        levels: Any = None,
        computed: Optional[Sequence[str]] = None,
        filters: Optional[dict] = None,
        highlight: Optional[dict] = None,
        budget: Optional[int] = None,
        palette_size: Optional[int] = None,
        pin: Any = None,
        on_tile: Optional[Callable[[Any], None]] = None,
    ):
        """Read the annotations a map draws in each tile at `zoom`, as one pyarrow table with a row
        per annotation per tile.

        Each level of a layer gives a tile at most `per_tile` of the annotations holding an item
        this reader may see there, the largest first. An annotation's counts and geometry cover
        all of its items this reader may see, whichever tile holds it.

        - `view`: the view, as `meta()` names it.
        - `zoom`, `tiles`, `levels`: as `Selection.sample` takes them.
        - `per_tile`: at most the server's `max_artifacts_per_tile`.
        - `bbox`: read the tiles touching this box, `(min_x, min_y, max_x, max_y)` in the view's
          coordinates, in place of `tiles`. Without either, every tile of the view is read.
        - `layers`: the layers to read, as a list of names or `"all"`. Without it no layer is
          read.
        - `computed`: the subset of `"centroid"` and `"box"` to compute. It can narrow what each
          layer declares and cannot widen it.
        - `filters`, `highlight`: filter expressions that set only each row's `matched` and
          `highlighted`. They do not change which rows come back.
        - `budget`: as `artifact_budget` on `Selection.sample`. A `nested` or `dag` layer is cut
          to it once for all the tiles read, so its rows have a null `tile`.
        - `palette_size`: how many colours your palette holds, from 2 to 32. Each row's `slot` is
          then the index of its colour, chosen for this reader so that neighbouring annotations
          differ, and an annotation keeps its slot at any zoom, box or filter.
        - `pin`: as on `Selection.sample`.
        - `on_tile`: a function called with each frame of rows as a pyarrow table as it arrives.
          The `nested` and `dag` layers' frame, whose `tile` is null, comes first.

        Each option is sent only when given, and the server's own setting applies otherwise.

        The columns are `layer`, `mosaica_id`, `key`, `masked_count`, `centroid_x`, `centroid_y`,
        `box_min_x`, `box_min_y`, `box_max_x`, `box_max_y`, `content`, `parent_ids`, `rung` (the
        level), `matched`, `highlighted`, `target`, `tile` and `slot`. The centroid and box are
        on the grid of 2^32 steps per axis that a sample's `code` uses, and `mosaica_id`,
        `parent_ids` and `target` hold decimal strings. The schema metadata holds `mosaica.pin`,
        `mosaica.stale` and `mosaica.region` as a `Sample` holds them.

            v.viewport_artifacts("papers", 2, per_tile=20, layers=["topics"]).to_pandas()
        """
        import pyarrow as pa

        request: dict = {"view": view, "zoom": int(zoom), "per_tile": int(per_tile)}
        if tiles is not None:
            request["tiles"] = [int(tile) for tile in listed(tiles)]
        else:
            request["bbox"] = [float(v) for v in (bbox or _extent(self.meta(), view))]
        for name, value in (
            ("layers", _layers(layers)),
            ("levels", levels),
            ("computed", None if computed is None else listed(computed)),
            ("filters", filters),
            ("highlight", highlight),
            ("budget", budget),
            ("palette_size", palette_size),
            ("pin", pin),
        ):
            if value is not None:
                request[name] = int(value) if name in _VIEWPORT_INTEGERS else value
        tables = []
        trailer = None
        # The request as sent, which the answer's metadata carries too.
        request = json.loads(self._encoded(request))
        with self._open("POST", "/v1/artifacts/viewport", request) as response:
            coordinates = _coordinates(response)
            for kind, payload in _frames(response):
                if trailer is not None:
                    raise Refusal("artifacts viewport: a frame after the trailer")
                if kind == FRAME_ARTIFACTS:
                    table = _tables([payload], ANNOTATION_IDS)
                    tables.append(table)
                    if on_tile is not None:
                        on_tile(table)
                elif kind == FRAME_TRAILER:
                    trailer = json.loads(bytes(payload).decode())
                else:
                    raise Refusal(f"artifacts viewport: unknown frame kind {kind}")
        if trailer is None:
            raise Refusal("artifacts viewport: no trailer, so the response is incomplete")
        rows = sum(table.num_rows for table in tables)
        if trailer.get("rows") != rows or trailer.get("frames") != len(tables):
            raise Refusal(
                f"artifacts viewport: the trailer says {trailer.get('rows')} rows in "
                f"{trailer.get('frames')} frames and the body carries {rows} in {len(tables)}"
            )
        joined = pa.concat_tables(tables) if tables else _no_artifacts()
        return joined.replace_schema_metadata(
            {
                "mosaica.trailer": json.dumps(trailer),
                "mosaica.request": json.dumps(request),
                **coordinates,
            }
        )

    def _bulk_read(self, route: str, request: dict, given: dict, batches: bool):
        """A read of `POST /v1/<route>` asking `request` and every `given` field that is not
        `None`: its `Batches`, or with `batches` false the whole read as one table."""
        request.update({key: value for key, value in given.items() if value is not None})
        read = Batches(self, route, request)
        return read if batches else read.read_all()

    def aggregate(
        self,
        view: str,
        groupings: Sequence[dict],
        filters: Optional[dict] = None,
        reference: Optional[dict] = None,
        *,
        cursor: Optional[str] = None,
    ) -> list:
        """Count how the items this reader may see in `view` are distributed, returning one pyarrow
        table per grouping in the order of `groupings`.

        - `view`: the view the counts are taken in. An item with no position in it is not
          counted.
        - `groupings`: one dictionary per table, built from the keys described below.
        - `filters`: the set counted, as a filter expression.
        - `reference`: a second set to compare with, as a filter expression, where `{}` is every
          item this reader may see in the view. Each table that counts then also has
          `reference_count` and `lift`.
        - `cursor`: the `cursor` of a `PartialRead` from the same request, to read the rows after
          it. The result then starts at the table the read stopped in, so when the stop came
          inside a table, the first table returned holds only its remaining rows, which you
          append to the last table in the error's `rows`.

        A grouping names at least one figure. `"count": True` counts the items in each row, so
        `{"count": True}` alone is the size of the set. `"values"`, `"missing"`, `"min"`, `"max"`
        and `"mean"` each take a list of number or timestamp fields and give a column per field,
        such as `min_year`, and `"summary"` gives all five. They are exact and are taken over
        every item this reader may see in the view, so they are refused beside `"by"` or
        `"cells"` and in a request with `filters` or `reference`. `min_` and `max_` have the
        field's type, and `mean_` is a float, or a UTC timestamp on a timestamp field.

        `"by"` groups the rows in one of three ways.

        - By a category field, as `{"field": name, "top": n}` for the `n` values with the most
          items, or `{"field": name, "values": [key, ...]}` for the values named. A `rest` row
          then counts the items with any other value and a `none` row those with no value.
        - By the annotations of a layer, as `{"layer": name, "top": n}` or `{"layer": name,
          "artifacts": [mosaica_id, ...]}`, with `rest` and `none` rows as for a field. A layer
          with several levels requires `"level": k`, which a layer with one level refuses. A
          listed id that names nothing, or an annotation this reader is not served, gets no row.
          Because annotations can overlap, the counts can sum to more than the set.
          `"palette_size": n` adds each annotation's palette slot. On a `nested` or `dag` layer,
          `top` needs `"cut": {"zoom": z, "bbox": [x0, y0, x1, y1], "budget": b}`, so that it
          ranks the annotations a map draws at that zoom, box and cluster budget, and without
          `budget` it ranks the finest cut.
        - By bins of a number or timestamp field, as `{"field": name, "bins": n}`, a histogram of
          at most `n` bins in one page. With `"range": [lower, upper]` the bins divide that range
          into equal widths and `rest` counts the values outside it. Without a range the edges
          are round values around the smallest and largest value this reader may see in the view,
          so they stay put as the filters change. The edges have the field's type, except that an
          integer field gets float edges when a bound of `range` is fractional. `"sample": s`
          counts a large set faster, over about `s` of its items, and scales each count to the
          set's size.

        `"cells": {"depth": d}` divides the set, or each group, into the view's cells at a depth
        from 0 to 32, where up to 16 they are the map's tiles at that zoom. Adding `"area": [x0,
        y0, x1, y1]` keeps only the cells meeting that box. A histogram takes no cells.

        A table's columns come in this order, each where it applies: `group` (`listed`, `rest` or
        `none`), `key` (a category's key, or an annotation's `mosaica_id` as its decimal string,
        which a `member_of` filter takes as it is), `title`, `slot`, `lower`
        and `upper` (a bin's edges, the last bin including its `upper`), `cell` (the cell's
        Morton prefix), `count`, `reference_count`, `lift` (`(count / total) / (reference_count /
        reference_total)`), and then the field figures. A category field has `title` only when
        its vocabulary holds titles. `slot` is null without `palette_size` and on the `rest` and
        `none` rows.

        Each table's schema metadata `mosaica.head` holds its totals as JSON, such as `total`,
        the items in the set, and with `"sample"` the object `sample`, whose `sampled` says
        whether any count was scaled. `mosaica.recomposed` is `"true"` when the pages counted a
        database that changed during the read, and `mosaica.region` says whether a `region`
        filter was answered exactly or over the grid cells covering it.

        A read that stops part of the way raises a `PartialRead`, whose `rows` are the tables read
        before the stop and whose `cursor`, passed back as `cursor`, reads the rest.

            venue = {"count": True, "by": {"field": "venue", "top": 10}}
            [size, venues] = v.aggregate("papers", [{"count": True}, venue])
            cells = {"count": True, "cells": {"depth": 6}}
            v.aggregate("papers", [cells], filters={"year": {"eq": 2023}}, reference={})
            years = {"count": True, "by": {"field": "submitted", "bins": 20}}
            [years] = v.aggregate("papers", [years])
            [year] = v.aggregate("papers", [{"count": True, "summary": ["year"]}])
            cut = {"zoom": 3, "bbox": [0, 0, 100, 100], "budget": 1000}
            topics = {"count": True, "by": {"layer": "topics", "top": 10, "cut": cut}}
            [topics] = v.aggregate("papers", [topics])
        """
        import pyarrow as pa
        from pyarrow import ipc

        groupings = list(groupings)
        request: dict = {"view": view, "groupings": groupings}
        if filters is not None:
            request["filters"] = filters
        if reference is not None:
            request["reference"] = reference
        what = "POST /v1/aggregate"

        def joined() -> list:
            tables = []
            # A table whose head arrived without a page has no columns to build it from.
            for grouping in sorted(g for g in pages if pages[g]):
                figures = {k: v for k, v in heads[grouping].items() if k != "resumed"}
                metadata = {
                    "mosaica.head": json.dumps(figures),
                    "mosaica.recomposed": "true" if recomposed else "false",
                }
                if region is not None:
                    metadata["mosaica.region"] = region
                table = pa.Table.from_batches(pages[grouping]).unify_dictionaries()
                by_layer = "layer" in (groupings[grouping].get("by") or {})
                table = _ids_as_text(table, also=("key",) if by_layer else ())
                tables.append(table.replace_schema_metadata(metadata))
            return tables

        def stopped(why: str, cause: Any = None) -> PartialRead:
            partial = PartialRead(why, resume, False, cause)
            partial.rows = joined()
            return partial

        heads: dict = {}
        pages: dict = {}
        recomposed = False
        region = None
        first = True
        while True:
            # Where a read stopped in this response resumes: after its last whole page.
            resume = cursor
            body = request if cursor is None else {**request, "cursor": cursor}
            try:
                response = self._open("POST", "/v1/aggregate", body)
            except (Refusal, OSError, http.client.HTTPException) as refused:
                # Before any row is read a refusal is raised as it came, as `items` raises it.
                if first:
                    raise
                raise stopped(str(refused), refused) from None
            first = False
            table, pending, trailer, carried = None, None, None, [0, 0]
            with response:
                region = region or _region(response)
                for kind, payload in _frames(response):
                    if kind == FRAME_TABLE_HEAD:
                        head = json.loads(payload)
                        table = head["grouping"]
                        # The first head's figures, whose page chose the groups listed.
                        heads.setdefault(table, head)
                        pages.setdefault(table, [])
                    elif kind == FRAME_RECORDS:
                        if table is None:
                            raise Refusal(f"{what}: a page arrived before any table head")
                        pending = ipc.open_stream(payload).read_next_batch()
                    elif kind == FRAME_PAGE_END:
                        if pending is None:
                            raise Refusal(f"{what}: a page end has no page before it")
                        pages[table].append(pending)
                        carried = [carried[0] + 1, carried[1] + pending.num_rows]
                        pending = None
                        resume = json.loads(payload)["next"]
                    elif kind == FRAME_TRAILER:
                        trailer = json.loads(payload)
                    else:
                        raise Refusal(f"{what}: a frame has the unknown kind {kind}")
            if trailer is None:
                raise stopped(f"{what}: the response ended before its trailer")
            if [trailer["pages"], trailer["rows"]] != carried:
                raise Refusal(
                    f"{what}: the trailer counts {trailer['pages']} pages and {trailer['rows']} "
                    f"rows, but the body carried {carried[0]} and {carried[1]}"
                )
            recomposed = recomposed or trailer.get("recomposed") is True
            cursor = trailer["next"]
            if cursor is None:
                return joined()

    def categories(
        self,
        column: str,
        prefix: Optional[str] = None,
        view: Optional[str] = None,
        codes: Optional[Sequence[int]] = None,
        filters: Optional[dict] = None,
    ):
        """List the values of a category column that this reader may see, as a pyarrow table with
        one row per value.

        A category column stores a small integer code for each value. Each row gives a value's
        `code`, its `key` as inserted, and its display `title`, null where none was given.

        - `column`: the category column's name.
        - `prefix`: list only the values whose key or title, or a word in either, starts with
          this text, ignoring case. The rows then come in the order of the matched text, at most
          the server's `max_suggestions` of them, each with a `count` of the items this reader
          may see that carry the value. The schema metadata `mosaica.more` is `"true"` when more
          matched, and `mosaica.total` is the number of items counted.
        - `view`: the view to read the column in, needed for a column declared for a view group.
          With `prefix` it also limits each count to the items in this view.
        - `codes`: list only these codes, such as codes read from a viewport response over HTTP.
          It cannot be combined with `prefix`.
        - `filters`: a filter expression that limits each count, and needs `prefix` and `view`. A
          value the filter excludes keeps its row with a count of 0.

        Without `prefix` the rows are in key order.

            v.categories("venue")
            v.categories("venue", prefix="neur")
            v.categories("venue", codes=[3, 7])
            recent = {"year": {"range": {"gte": 2020}}}
            v.categories("venue", prefix="", view="papers", filters=recent)
        """
        import pyarrow as pa

        def table(values: list, counted: bool = False):
            columns = {
                "key": pa.array([one["key"] for one in values], pa.string()),
                "code": pa.array([one["code"] for one in values], pa.uint32()),
                "title": pa.array([one.get("title") for one in values], pa.string()),
            }
            if counted:
                columns["count"] = pa.array([one.get("count") for one in values], pa.uint64())
            return pa.table(columns)

        if codes is not None and prefix is not None:
            raise Refusal(
                "categories: codes= lists the values of codes you hold and prefix= searches the "
                "values by text. Give one of them"
            )
        if filters is not None and codes is not None:
            raise Refusal(
                "categories: codes= resolves codes and carries no counts for filters= to narrow; "
                "give prefix= with filters= in place of codes="
            )
        if filters is not None and prefix is None:
            raise Refusal(
                "categories: filters= narrows the counts of a listing by prefix=; give prefix= as "
                "well, where an empty one lists values from the start up to the server's "
                "max_suggestions"
            )
        path = f"/v1/categories/{urllib.parse.quote(column, safe='')}"
        query: dict = {} if view is None else {"view": view}
        if codes is not None:
            wanted = [str(int(code)) for code in listed(codes)]
            # An empty `codes=` on the wire lists the whole vocabulary, so ask nothing.
            values = []
            if wanted:
                query["codes"] = ",".join(wanted)
                values = json.loads(self._request("GET", path + _query(query), None))["values"]
            return table(values)
        if prefix is not None:
            query.update(q=prefix, counts="true")
            if filters is None:
                page = json.loads(self._request("GET", path + "/suggest" + _query(query), None))
            else:
                body = {**query, "counts": True, "filters": filters}
                page = json.loads(self._request("POST", path + "/suggest", body))
            more = "true" if page.get("more") else "false"
            return table(page["values"], counted=True).replace_schema_metadata(
                {"mosaica.more": more, "mosaica.total": str(page["total"])}
            )
        values: list = []
        while True:
            page = json.loads(self._request("GET", path + _query(query), None))
            values += page["values"]
            if page.get("next") is None:
                break
            query["after"] = page["next"]
        return table(values)

    def browse_artifacts(
        self,
        view: str,
        layer: str,
        level: Optional[int] = None,
        parent: Any = None,
        q: Optional[str] = None,
        filters: Optional[dict] = None,
        limit: Optional[int] = None,
        cursor: Optional[str] = None,
        palette_size: Optional[int] = None,
    ) -> dict:
        """Return one page of a layer's annotations as a dictionary, independent of any map tile,
        for a list or search panel.

        The page holds the rows in `artifacts` and the cursor for the next page in `next`, absent
        on the last page.

        - `view`, `layer`, `level`: as for `artifacts`.
        - `parent`: list the children of this annotation, given by its `mosaica_id`. Without it
          or `q`, the page lists the top-level annotations.
        - `q`: list the annotations whose key or name contains this text, ignoring case. It
          cannot be combined with `parent`.
        - `filters`: a filter expression. Each row then has `matched_count`, the number of its
          items this reader may see that match, and the page is ordered by it.
        - `limit`: the most rows on the page, capped at the server's `max_browse_rows`.
        - `cursor`: the `next` of the previous page.
        - `palette_size`: how many colours your palette holds, as on `artifacts`.

        Each row has `masked_count`, the number of its items this reader may see, and
        `child_count`, how many rows `parent=` would list for it. A row has a `name` when the
        annotation has text or a label attached to it, such as a cluster's topic.

            page = v.browse_artifacts("papers", "clusters")
            more = v.browse_artifacts("papers", "clusters", cursor=page["next"])
        """
        request: dict = {"view": view, "layer": layer}
        if level is not None:
            request["level"] = int(level)
        if parent is not None:
            request["parent"] = _an_id(parent, "browse_artifacts")
        if q is not None:
            request["q"] = q
        if filters is not None:
            request["filters"] = filters
        if limit is not None:
            request["limit"] = int(limit)
        if cursor is not None:
            request["cursor"] = cursor
        if palette_size is not None:
            request["palette_size"] = int(palette_size)
        return json.loads(self._request("POST", "/v1/artifacts/browse", request))

    def artifact(
        self,
        mosaica_id: Any,
        view: str,
        zoom: Optional[int] = None,
    ) -> dict:
        """Return one annotation's record as a dictionary, when this reader may see the annotation.

        - `mosaica_id`: the annotation's id, as `browse_artifacts`, `artifacts` or a sample's
          `membership:<layer>` column gives it.
        - `view`: the view whose items its count and geometry are computed over.
        - `zoom`: the zoom the caller draws at, so that the shape drops vertices that would move
          an edge by less than a pixel. Without it the shape is simplified to at most 2,048
          vertices, and a hull drawn around the items is never simplified.

        The record always has `layer`, `masked_count` and `rung`, its level. It has `key`,
        `content`, `centroid`, `box` and `shape` where the annotation has them, with the geometry
        on the grid of 2^32 steps per axis that a sample's `code` uses.

            v.artifact(page["artifacts"][0]["mosaica_id"], "papers")
        """
        path = f"/v1/artifacts/{_an_id(mosaica_id, 'artifact')}"
        self._require_view(view)
        request: dict = {"view": view}
        if zoom is not None:
            request["zoom"] = int(zoom)
        return json.loads(self._request("POST", path, request))

    def _require_view(self, name: str, meta: Optional[dict] = None) -> None:
        views = (self.meta() if meta is None else meta).get("views") or []
        _require(name, [block["id"] for block in views], "view")

    def _keys_of(self, column: str, view: Optional[str], codes) -> dict:
        """The keys of these codes of a category column, asking the server only for new codes.

        The answers are kept for the life of this reader. A code the server does not resolve is
        asked for again next time, since a later commit can make its value visible.
        """
        import pyarrow.compute as pc

        held = self._keys.setdefault((column, view), {})
        wanted = [code for code in pc.unique(codes).to_pylist() if code and code not in held]
        if wanted:
            found = self.categories(column, view=view, codes=wanted)
            held.update(zip(found.column("code").to_pylist(), found.column("key").to_pylist()))
        return held

    def _request(self, method: str, path: str, body: Optional[dict]) -> bytes:
        """One request: the answer's body, or a refusal with what the server said."""
        with self._open(method, path, body) as response:
            return response.read()

    def _open(self, method: str, path: str, body: Optional[dict]):
        """One request: the answer, open for its body to be read as it arrives, or a refusal with
        what the server said.

        A `403 expired-token` says the session ended before its expiry, as a catalogue change or a
        revoke ends it. Where this reader can get another token, it gets one and sends the request once
        more.
        """
        for attempt in (0, 1):
            token = self.token()
            request = urllib.request.Request(
                self.url + path,
                data=None if body is None else self._encoded(body),
                method=method,
                headers=request_headers(method, None if token is None else token.token),
            )
            try:
                return urllib.request.urlopen(request, timeout=120)
            except urllib.error.HTTPError as refused:
                answer = refused.read().decode(errors="replace")[:1000]
                if attempt == 0 and refused.code == 403 and "expired-token" in answer:
                    if self._forget():
                        continue
                raise self._refused(path, body, refused.code, answer, refused.headers) from None

    def _encoded(self, body: dict) -> bytes:
        """A request's body as JSON, a moment anywhere in it as its microseconds since the Unix
        epoch."""
        return json.dumps(body, default=_json_value).encode()

    def _refused(
        self, path: str, body: Optional[dict], status: int, answer: str, headers=None
    ) -> Refusal:
        """A refusal the server made, in one sentence: what the server said, the filterable
        columns where a filter named a column that is not one, and the status.

        A `404` for an item or an annotation says the same whether the id names nothing or names
        something this reader may not see, as the server's answer does.
        """
        try:
            said = json.loads(answer)
        except ValueError:
            said = None
        try:
            detail = said["detail"]
        except (TypeError, KeyError):
            detail = answer.strip() or "no reason given"
        segments = path.split("?")[0].strip("/").split("/")
        if status == 404 and len(segments) == 3 and segments[1] in ("items", "artifacts"):
            what = "item" if segments[1] == "items" else "annotation"
            detail = f"there is no {what} {segments[2]} that this reader can see"
        keys = ("filters", "highlight", "reference")
        expressions = [body.get(key) for key in keys] if body else []
        named = set().union(*(_filter_columns(one) for one in expressions))
        if status == 422 and named:
            try:
                filterable = [one["column"] for one in self.meta().get("filter_operands") or []]
            except Refusal:
                filterable = None
            unknown = sorted(named - set(filterable or named))
            if unknown:
                detail = (
                    f"{', '.join(repr(one) for one in unknown)} is not a filterable column; name "
                    f"one of: {', '.join(filterable) or 'none, since no column is indexed'}"
                )
        return Refusal(
            f"{detail} (HTTP {status})",
            status=status,
            retry_after_s=retry_after(status, headers, said),
        )

    def _forget(self) -> bool:
        """Drop the held token so the next request gets another; `False` where none can be got."""
        held = self._token
        if held is not None and held.renew is not None:
            self._token = held.renew()
            return True
        if callable(self._source) and not isinstance(self._source, Token):
            self._token = minted(self._source)
            return True
        return False

    def __repr__(self) -> str:
        terms = "" if self.terms is None else f", terms={self.terms!r}"
        return f"Viewer(url={self.url!r}{terms})"


def connect(url: str, token: Optional[TokenSource] = None) -> Viewer:
    """Connect to a Mosaica database someone else runs, returning a `Viewer` that reads what the
    token's terms admit.

    - `url`: the address of the database's viewer plane.
    - `token`: the token the database's operator issued you, in any of the forms `TokenSource`
      describes. Leave it out for a server started with `mosaica serve --open`.

        v = mosaica.connect("https://maps.example/viewer", token=my_token)
        v.view("papers").count()
        v = mosaica.connect(url, lambda: mosaica.login(url, api_key=my_key))
    """
    return Viewer(url, token)
