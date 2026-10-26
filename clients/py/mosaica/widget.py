"""The notebook map, `Map`, which is the kernel's half of the widget in
`clients/ts/components/src/widget.ts`.

The page's half is the components' single-file bundle, which anywidget runs as `_esm`. The
kernel's half holds the token and sends it to the page as a custom message in answer to the
page's `ready` and `reauthorise` messages. No traitlet carries the token, so nothing that saves
widget state can save it, whether that is the front end's "save widget state", `nbconvert
--execute`, papermill or a run in which no front end mounts.

Only controls and selections cross between the kernel and the page, because the page fetches
the data itself. The view and camera controls go up when the map settles, so that the kernel is
not asked on every frame, and `Map` says when each other attribute is sent. Ids cross as decimal
strings, because a `mosaica_id` is a `u64`, which a JavaScript number cannot hold exactly and
JSON cannot carry as a `BigInt`.

`basemap` goes down as `_basemap`, which holds the value with each local image written as
`{"local": path}`, and for each such path the number of the read whose bytes the page is to
draw. The page takes the value and its images in one update, and the saved state holds no more
of a path than the caller wrote and none of an image's content. The kernel reads the file each
time the value is set, and moves the read number on only when the bytes or the type change. It
sends the bytes as a `basemap-image` message with a binary buffer in answer to
`basemap-images`, which names the reads the page lacks.
"""

from __future__ import annotations

import collections.abc
import hashlib
import json
import math
import mimetypes
import numbers
import os
import pathlib
import re
import sys
import warnings
from typing import Any, Optional, Sequence

import anywidget
import traitlets

from ._auth import Token, TokenSource, minted
from ._refusal import listed
from ._viewer import _json_value, decimal_id

_HERE = pathlib.Path(__file__).parent
_BUNDLE = "mosaica-components.js"


def bundle_path() -> Optional[pathlib.Path]:
    """The path of the components' JavaScript bundle that `Map` loads, or `None` when there is
    none.

    The copy packaged in the wheel under `mosaica/static/` is used when it is there, and a
    checkout's own build in `clients/ts/components/dist/` otherwise.
    """
    packaged = _HERE / "static" / _BUNDLE
    if packaged.exists():
        return packaged
    checkout = _HERE.parents[2] / "ts" / "components" / "dist" / _BUNDLE
    if checkout.exists():
        return checkout
    return None


def _decimal_id(value: Any, name: str) -> Optional[str]:
    """An id as the decimal string of a ``u64``; an ``int`` is accepted and stringified."""
    if value is None:
        return None
    text = decimal_id(value)
    if text is None:
        raise traitlets.TraitError(
            f"{name} is a mosaica_id, a whole number from 0 to 2^64 - 1, not {value!r}"
        )
    return text


def _hex(value: Any, name: str) -> str:
    """A colour as ``#rrggbb`` in lower case."""
    if isinstance(value, str) and re.fullmatch(r"#[0-9a-fA-F]{6}", value):
        return value.lower()
    raise traitlets.TraitError(f"{name} is a colour as '#rrggbb', not {value!r}")


def _by_name(value: Any, trait: str, outer: str, inner: str) -> dict:
    """A trait's ``{name: {key: colour}}``, checked to be a dictionary of dictionaries."""
    if not isinstance(value, dict) or not all(isinstance(k, str) and isinstance(v, dict) for k, v in value.items()):
        raise traitlets.TraitError(f"{trait} maps a {outer} to {{{inner}: '#rrggbb'}}; got {value!r}")
    return value


def _single(value: Any, where: str) -> tuple:
    """One basemap value checked for its shape and normalised, keys holding `None` left out, and
    its image's path, as written, where the image is a local file."""
    if isinstance(value, str) and value in ("world", "none"):
        return value, None
    given = {k: v for k, v in value.items() if v is not None} if isinstance(value, dict) else None
    if given is None or ("tiles" not in given and "image" not in given):
        raise traitlets.TraitError(
            f'{where} is "world", "none", {{"tiles": url_template, "attribution": text}} or '
            f'{{"image": url_or_path, "bounds": [x0, y0, x1, y1]}}, not {value!r}'
        )
    if "tiles" in given:
        unknown = sorted(set(given) - {"tiles", "attribution"}, key=str)
        if unknown:
            raise traitlets.TraitError(f"{where} is a tile basemap, which holds only tiles and attribution; remove {unknown}")
        tiles, attribution = given.get("tiles"), given.get("attribution")
        if not isinstance(tiles, str) or not all(t in tiles for t in ("{z}", "{x}", "{y}")):
            raise traitlets.TraitError(
                f"{where}'s tiles is a URL template holding {{z}}, {{x}} and {{y}}, such as "
                f"'https://tiles.example.org/{{z}}/{{x}}/{{y}}.png', not {tiles!r}"
            )
        if attribution is not None and not isinstance(attribution, str):
            raise traitlets.TraitError(f"{where}'s attribution is the text of the credit the tile server asks for, not {attribution!r}")
        return given, None
    unknown = sorted(set(given) - {"image", "bounds"}, key=str)
    if unknown:
        raise traitlets.TraitError(f"{where} is an image basemap, which holds only image and bounds; remove {unknown}")
    image = given.get("image")
    if isinstance(image, os.PathLike):
        path = os.fspath(image)
    elif isinstance(image, str) and image != "":
        if re.match(r"(?i)file:", image):
            raise traitlets.TraitError(
                f"{where}'s image {image!r} is a file: URL, which the page cannot read; give the "
                f"file's path, such as 'maps/plan.png', or an http(s) or data: URL"
            )
        path = None if re.match(r"(?i)(https?|data):", image) else image
    else:
        raise traitlets.TraitError(f"{where}'s image is the image's http(s) or data: URL or its path, not {image!r}")
    out: dict = {"image": image}
    if "bounds" in given:
        out["bounds"] = _bounds(given["bounds"], where)
    return out, (None if path is None else os.fspath(image))


def _bounds(bounds: Any, where: str) -> list:
    """An image basemap's bounds as four finite floats."""
    wrong = traitlets.TraitError(
        f"{where}'s bounds are four numbers, [x0, y0, x1, y1], in the view's coordinates, not "
        f"{bounds!r}; leave them out to cover the whole frame"
    )
    numpy = sys.modules.get("numpy")
    if isinstance(bounds, (str, bytes)) or not (
        isinstance(bounds, collections.abc.Sequence) or (numpy is not None and isinstance(bounds, numpy.ndarray))
    ):
        raise wrong
    items = list(bounds)
    if len(items) != 4 or not all(isinstance(b, numbers.Real) and not isinstance(b, bool) for b in items):
        raise wrong
    try:
        floats = [float(b) for b in items]
    except OverflowError:
        raise wrong from None
    if not all(math.isfinite(b) for b in floats):
        raise wrong
    return floats


def _basemap_choice(value: Any) -> tuple:
    """`Map.basemap` checked and normalised; the same as the page is sent it, each local image
    named `{"local": path}` with the path as written; and those paths.

    A key holding `None` is left out, at both levels. A dict with a `tiles` or `image` key is then
    one value, and any other dict maps view ids to values.
    """
    if isinstance(value, dict):
        value = {k: v for k, v in value.items() if v is not None}
    if value is None:
        return None, None, []
    if isinstance(value, dict) and "tiles" not in value and "image" not in value:
        key = next((k for k in value if not isinstance(k, str)), None)
        if key is not None:
            raise traitlets.TraitError(f"basemap maps view ids, as strings, to basemaps; {key!r} is not a view id")
        entries = {view: _single(entry, f"basemap[{view!r}]") for view, entry in value.items()}
    else:
        entries = {None: _single(value, "basemap")}
    kernel, page, files = {}, {}, []
    for view, (one, path) in entries.items():
        kernel[view] = one
        page[view] = one if path is None else {**one, "image": {"local": path}}
        if path is not None and path not in files:
            files.append(path)
    if None in entries:
        return kernel[None], page[None], files
    return kernel, page, files


class _Basemap(traitlets.Any):
    """`Map.basemap`'s trait: `Any`, but with `None` validated too, as `Any` lets it skip the
    validator that publishes `_basemap`."""

    allow_none = False


class Map(anywidget.AnyWidget):
    """An interactive map of a Mosaica database, shown in a notebook cell.

    The map in the browser fetches its data from the database's viewer plane with the token you
    give it, so it shows only the items that token may see.

        m = mosaica.Map(viewer_url, token=token, colour_by="venue")
        m

    The token reaches the page as a message and is never part of the widget's state, so saving
    the notebook does not save it. The page calls `url` from the notebook's origin, which the
    server must admit: a database made with `create()` admits loopback pages, and another server
    needs the origin in `serve.cors_origins`. VS Code and Colab notebooks have origins no list
    can name, so the map cannot reach a database from them.

    - `url`: the address of the database's viewer plane.
    - `token`: the token the map reads with, in any of the forms `TokenSource` describes. It is
      required, so a `Map` made without one raises `TypeError` despite the default of `None`.
    - `view`: the view to open on. `None`, the default, opens the first one.
    - `layers`: the annotation layers to draw, each drawn with the layers it depends on. `None`,
      the default, lets the map choose, `[]` draws none, and `"all"` is refused.
    - `colour_by`: a rendered column to colour points by, or `"cluster:<layer>"` to colour them
      by the annotations of that layer.
    - `palette`: the palette that annotations are coloured from under `"cluster:<layer>"`, which
      is one of `"okabe-ito"` (8 colours that colour-blind readers can tell apart), `"tableau10"`
      (10), `"tableau20"` (20, in light and dark pairs) and `"kelly"` (22, as distinct as
      possible). `None`, the default, leaves the map's choice, Tableau 10 at first.
    - `value_colours`: colours that replace the palette colour of single values of category
      columns, as `{column: {value: "#rrggbb"}}`. `None`, the default, keeps the colours chosen
      in the map, and `{}` gives every value its palette colour.
    - `cluster_colours`: colours that replace the palette colour of single annotations under
      `"cluster:<layer>"`, as `{layer: {mosaica_id: "#rrggbb"}}` with each `mosaica_id` a decimal
      string. `None`, the default, keeps the colours chosen in the map, and `{}` gives every
      annotation its palette colour.
    - `size_by`: a number column to size points by. `None`, the default, draws every point at
      one size.
    - `size_min`, `size_max`: the radius in pixels of the point with the smallest and the largest
      value of `size_by`, where a point with no value is drawn as a ring. The page ignores a
      radius that is not a number above zero. `None` leaves the map's choice, 2 and 9 at first.
    - `size_scale`: how values are spread between the two radii, as `"linear"`, `"log"` or
      `"rank"`, which ranks each value among a sample of those drawn. `None` leaves the map's
      choice, `"linear"` at first.
    - `filters`: a filter expression, in the form `Selection.filter` takes.
    - `bbox`: the box to frame the camera on, as `(min_x, min_y, max_x, max_y)`. Without it the
      map opens framed on the items that `filters` matches.
    - `height`: the widget's height in pixels in the notebook, 480 by default. The map fills the
      screen instead while its output is shown full screen.
    - `basemap`: what is drawn under the points, as one of the values described below, or as a
      dictionary from view id to value for the views the map can switch to. `None`, the default,
      draws the world outline on a view with a map projection and nothing on any other view.
    - `explorer_layout`: where the controls go. `"docked"`, the default, puts them in a sidebar
      beside the map, and `"overlay"` floats them over a map that fills the whole widget. A
      change shows the next time the map is displayed.
    - `title_field`: the field whose value titles a point in the hover and the item card. `None`,
      the default, titles a point by its id.
    - `artifacts_per_tile`: the most annotations each level of a layer draws in one map tile,
      largest first. `None`, the default, asks for 50, or for the server's
      `max_artifacts_per_tile` when that is smaller, and the server refuses a value above its
      maximum.
    - `budget`: how many points the map aims to draw on screen. `0`, the default, keeps the map's
      own budget, which starts at 250,000.
    - `budget_min`, `budget_max`: the ends of the Most points slider in the map's Layers popover,
      1,000 and 2,000,000 by default. Neither changes `budget`.
    - `cluster_budget`: the most clusters a `nested` or `dag` layer is drawn with, which the map
      meets by drawing parents in place of their children. The default is 1,000, and `0` draws
      every cluster.
    - `cluster_budget_min`, `cluster_budget_max`: the ends of the Most clusters slider, which the
      Layers popover shows while such a layer is drawn. They are 10 and 10,000 by default, and
      neither changes `cluster_budget`.

    A basemap is one of these values:

    - `"world"`: land, coastlines and country borders from the database's world outline, for a
      view with a map projection.
    - `"none"`: nothing.
    - `{"tiles": url_template, "attribution": text}`: raster tiles from a tile server, for a view
      whose frame lines up with web map tiles. The template holds `{z}`, `{x}` and `{y}`, and
      the map shows `attribution` in its corner.
    - `{"image": url_or_path, "bounds": [x0, y0, x1, y1]}`: one image, stretched without
      reprojection over `bounds` in the view's coordinates, or over the whole frame when `bounds`
      is left out.

    An image may be an `http(s):` or `data:` URL, or the path of a local file as a
    `pathlib.Path` or any other string, which is not expanded for `~`. The file is read each time
    `basemap` is set and sent to the page as a message, so its bytes are not saved with the
    notebook. A file that cannot be read raises `traitlets.TraitError` and leaves `basemap` as it
    was, and a `file:` URL is refused. A key holding `None` is left out. Because a dictionary with a
    `tiles` or `image` key is read as one basemap, a dictionary by view cannot name a view called
    `tiles` or `image`. A value of the wrong shape raises `traitlets.TraitError`, and the page
    refuses a value that does not fit the view and reports why in `last_error` and a warning.

    Every argument except `token` is also an attribute of the same name, and setting one in a
    later cell applies it to the map, apart from `explorer_layout`. The map writes the changes
    made in it back into these attributes:

    - `view`, `bbox`, `filters`, `layers`, `colour_by` and the four size settings, when the map
      settles after a change, once it has finished fetching for the new view.
    - `palette`, when one is chosen in the map's Colour section.
    - `budget` and `cluster_budget`, when the Most points or Most clusters slider is let go.
    - `value_colours` and `cluster_colours`, as each colour is chosen on a field card or in Edit
      colours. Setting either replaces every colour of its kind that the map holds.

    The page applies a `filters` expression only when its filter panel can show it, so it
    refuses `any_of`, `none_of` and two tests on one column, applying nothing and saying why in
    `last_error` and a warning.

    The map also keeps these attributes, which you read and do not set:

    - `selected`: the `mosaica_id` of the picked item, as a decimal string, or `None`. It
      changes on each pick.
    - `selected_artifact`: the `mosaica_id` of the opened annotation, as a decimal string, or
      `None`.
    - `region`: the box or lasso drawn on the map, or `None` once it is cleared, as a dictionary
      of its `shape`, `status`, the counts `visible`, `matched` and `served` inside it, a
      `verdict` saying whether they are exact for the shape or cover the grid cells around it,
      and any `refusal`. It changes when the region's counts arrive.
    - `last_error`: why the page last refused something set from Python, or `None`.
    - `tokens_sent`: how many tokens the kernel has sent the page.

    To run code when someone picks an item on the map, call `m.observe(fn, names="selected")`.
    In marimo, observe `selected` in place of reading `.value`, because `mo.ui.anywidget(m)`
    holds every attribute in that one value and a cell that reads it runs again each time the
    map settles.

    A `Map` made without the components' JavaScript bundle, which `bundle_path()` finds, raises
    `RuntimeError` naming what to install.
    """

    # The bundle's text, set per instance from `bundle_path()`; anywidget's frontend reads both.
    _esm = traitlets.Unicode("").tag(sync=True)
    _css = traitlets.Unicode("").tag(sync=True)

    # Down.
    url = traitlets.Unicode().tag(sync=True)
    view = traitlets.Unicode(None, allow_none=True).tag(sync=True)
    # Not `layout`: ipywidgets' DOMWidget already has one, the CSS Layout model the frontend reads.
    explorer_layout = traitlets.Enum(["docked", "overlay"], default_value="docked").tag(sync=True)
    height = traitlets.Int(480).tag(sync=True)
    # The record field that titles a point in the hover and the item card; `None` titles it by id.
    title_field = traitlets.Unicode(None, allow_none=True).tag(sync=True)
    # The store's quota of annotations per level per tile; `None` leaves the store's default.
    artifacts_per_tile = traitlets.Int(None, allow_none=True).tag(sync=True)
    budget = traitlets.Int(0).tag(sync=True)
    budget_min = traitlets.Int(1_000).tag(sync=True)
    budget_max = traitlets.Int(2_000_000).tag(sync=True)
    cluster_budget = traitlets.Int(1_000).tag(sync=True)
    cluster_budget_min = traitlets.Int(10).tag(sync=True)
    cluster_budget_max = traitlets.Int(10_000).tag(sync=True)
    # Both ways, synced up at the settle.
    bbox = traitlets.List(traitlets.Float(), minlen=4, maxlen=4, allow_none=True, default_value=None).tag(sync=True)
    # `None` leaves the explorer's own default; `[]` is none; a list is exactly those (with their
    # dependency closure, which the store adds).
    layers = traitlets.List(traitlets.Unicode(), allow_none=True, default_value=None).tag(sync=True)
    colour_by = traitlets.Unicode(None, allow_none=True).tag(sync=True)
    palette = traitlets.Enum(["okabe-ito", "tableau10", "tableau20", "kelly"], default_value=None, allow_none=True).tag(sync=True)
    value_colours = traitlets.Dict(default_value=None, allow_none=True).tag(sync=True)
    cluster_colours = traitlets.Dict(default_value=None, allow_none=True).tag(sync=True)
    size_by = traitlets.Unicode(None, allow_none=True).tag(sync=True)
    size_min = traitlets.Float(None, allow_none=True).tag(sync=True)
    size_max = traitlets.Float(None, allow_none=True).tag(sync=True)
    size_scale = traitlets.Enum(["linear", "log", "rank"], default_value=None, allow_none=True).tag(sync=True)
    filters = traitlets.Dict(default_value=None, allow_none=True).tag(sync=True)
    # A string, one value as a dict, or a dict from view id to value; `None` is the map's default.
    # Kernel-side: the page reads it from `_basemap`.
    basemap = _Basemap()
    # `{"value": basemap, "images": {path: read}}`: the value and the local images it names, each
    # with the number of the read whose bytes the page is to draw, synced in one update.
    _basemap = traitlets.Dict({"value": None, "images": {}}).tag(sync=True)
    # Kernel-side, and traits so that a held change that fails rolls them back with `basemap`:
    # the images `_basemap` lists, by path as written, to the read's number, type and bytes; and
    # each path's last read number and a digest of its bytes and type.
    _images = traitlets.Dict()
    _reads = traitlets.Dict()
    # Up.
    # `Any` rather than `Unicode` so the validator below runs on an int and stringifies it.
    selected = traitlets.Any(None, allow_none=True).tag(sync=True)
    selected_artifact = traitlets.Any(None, allow_none=True).tag(sync=True)
    region = traitlets.Dict(default_value=None, allow_none=True).tag(sync=True)
    # Kernel-side only: the page's last refusal of something set here (never synced).
    last_error = traitlets.Unicode(None, allow_none=True)

    def __init__(
        self,
        url: str,
        token: Optional[TokenSource] = None,
        *,
        view: Optional[str] = None,
        layers: Optional[Sequence[str]] = None,
        colour_by: Optional[str] = None,
        palette: Optional[str] = None,
        value_colours: Optional[dict] = None,
        cluster_colours: Optional[dict] = None,
        size_by: Optional[str] = None,
        size_min: Optional[float] = None,
        size_max: Optional[float] = None,
        size_scale: Optional[str] = None,
        filters: Optional[dict] = None,
        bbox: Optional[Sequence[float]] = None,
        height: int = 480,
        explorer_layout: str = "docked",
        title_field: Optional[str] = None,
        artifacts_per_tile: Optional[int] = None,
        budget: int = 0,
        budget_min: int = 1_000,
        budget_max: int = 2_000_000,
        cluster_budget: int = 1_000,
        cluster_budget_min: int = 10,
        cluster_budget_max: int = 10_000,
        basemap: Any = None,
        **kwargs: Any,
    ) -> None:
        if token is None:
            raise TypeError(
                "Map needs a token: the viewer token your deployment issued you, one from "
                "mosaica.login(url, ...), or one an operator running locally mints with "
                "db.token(terms)."
            )
        bundle = bundle_path()
        if bundle is None:
            raise RuntimeError(
                "mosaica's bundle is missing. From PyPI: pip install 'mosaica[widget]' installs "
                "it. From the checkout: pip install -e 'clients/py[widget]' builds it with Node, or "
                "run `npm run bundle -w @mosaicajs/components` in clients/ts."
            )
        self._token_source: TokenSource = token
        self._token: Optional[Token] = None
        self.tokens_sent = 0
        super().__init__(
            url=url,
            view=view,
            layers=None if layers is None else listed(layers),
            colour_by=colour_by,
            palette=palette,
            value_colours=value_colours,
            cluster_colours=cluster_colours,
            size_by=size_by,
            size_min=size_min,
            size_max=size_max,
            size_scale=size_scale,
            filters=filters,
            bbox=None if bbox is None else [float(v) for v in bbox],
            height=height,
            explorer_layout=explorer_layout,
            title_field=title_field,
            artifacts_per_tile=artifacts_per_tile,
            budget=budget,
            budget_min=budget_min,
            budget_max=budget_max,
            cluster_budget=cluster_budget,
            cluster_budget_min=cluster_budget_min,
            cluster_budget_max=cluster_budget_max,
            basemap=basemap,
            _esm=bundle.read_text(encoding="utf-8"),
            **kwargs,
        )
        # Inside an ipywidgets row the map's element would take no width, and draw nothing.
        if "layout" not in kwargs:
            self.layout.width = "100%"
        self.on_msg(self._on_page_message)

    # ---- the token ---------------------------------------------------------------------------

    def _current_token(self, *, renew: bool) -> Token:
        if renew and self._token is not None and self._token.renew is not None:
            self._token = self._token.renew()
            return self._token
        self._token = minted(self._token_source)
        return self._token

    def _token_message(self, *, renew: bool) -> dict:
        token = self._current_token(renew=renew)
        return {"type": "token", "token": token.token, "expires_at": token.expires_at}

    def _on_page_message(self, widget: Any, content: Any, buffers: Any) -> None:
        kind = content.get("type") if isinstance(content, dict) else None
        if kind in ("ready", "reauthorise"):
            try:
                message = self._token_message(renew=kind == "reauthorise")
            except Exception as e:  # a supplier that failed: the page shows `refused`
                self.send({"type": "refused", "detail": str(e)})
                return
            self.tokens_sent += 1
            self.send(message)
        elif kind == "basemap-images":
            wanted = content.get("reads")
            for path, read in (wanted if isinstance(wanted, dict) else {}).items():
                held = self._images.get(path)
                if held is not None and held[0] == read:
                    self.send({"type": "basemap-image", "path": path, "read": read, "mime": held[1]}, buffers=[held[2]])
        elif kind == "error":
            detail = f"{content.get('what')}: {content.get('detail')}"
            self.last_error = detail
            warnings.warn(f"mosaica widget refused {detail}", stacklevel=2)

    # ---- the basemap's images ----------------------------------------------------------------

    @traitlets.validate("basemap")
    def _validate_basemap(self, proposal: dict) -> Any:
        value, page, files = _basemap_choice(proposal["value"])
        read = {}
        for path in files:
            try:
                data = pathlib.Path(path).read_bytes()
            except OSError as e:
                raise traitlets.TraitError(
                    f"the basemap image {path!r} cannot be read ({e.strerror}; relative paths "
                    f"start at {os.getcwd()}); give the path of an image file, or the image's "
                    f"http(s) or data: URL"
                ) from e
            read[path] = (mimetypes.guess_type(path)[0] or "application/octet-stream", data)
        images, reads = {}, dict(self._reads)
        for path, (mime, data) in read.items():
            digest = hashlib.sha256(mime.encode() + b"\0" + data).digest()
            number, seen = reads.get(path, (0, None))
            if seen != digest:
                number += 1
                reads[path] = (number, digest)
            images[path] = (number, mime, data)
        self._reads = reads
        self._images = images
        self._basemap = {"value": page, "images": {path: number for path, (number, _, _) in images.items()}}
        return value

    # ---- validation --------------------------------------------------------------------------

    @traitlets.validate("selected", "selected_artifact")
    def _validate_id(self, proposal: dict) -> Optional[str]:
        return _decimal_id(proposal["value"], proposal["trait"].name)

    @traitlets.validate("value_colours")
    def _validate_value_colours(self, proposal: dict) -> Optional[dict]:
        if proposal["value"] is None:
            return None
        return {
            column: {str(k): _hex(v, f"value_colours[{column!r}][{k!r}]") for k, v in values.items()}
            for column, values in _by_name(proposal["value"], "value_colours", "column", "value").items()
        }

    @traitlets.validate("cluster_colours")
    def _validate_cluster_colours(self, proposal: dict) -> Optional[dict]:
        if proposal["value"] is None:
            return None
        return {
            layer: {
                _decimal_id(k, f"a cluster_colours[{layer!r}] key"): _hex(v, f"cluster_colours[{layer!r}][{k!r}]")
                for k, v in ids.items()
            }
            for layer, ids in _by_name(proposal["value"], "cluster_colours", "layer", "mosaica_id").items()
        }

    @traitlets.validate("filters")
    def _validate_filters(self, proposal: dict) -> Optional[dict]:
        # A moment is sent as its microseconds, which is what the page reads a timestamp in.
        if proposal["value"] is None:
            return None
        return json.loads(json.dumps(proposal["value"], default=_json_value))

    @traitlets.validate("layers")
    def _validate_layers(self, proposal: dict) -> Optional[list]:
        if proposal["value"] is None:
            return None
        layers = list(proposal["value"])
        if "all" in layers:
            raise traitlets.TraitError("`all` is not a layer name; name the layers, or set none")
        return layers

    def __repr__(self) -> str:
        return f"Map(url={self.url!r}, view={self.view!r}, layers={self.layers!r})"


def side_by_side(*maps: Map) -> Any:
    """Several maps in one row, sharing its width equally, in a notebook cell.

    Each map keeps its own view, camera, filters and colours; moving one moves no other. In a
    marimo notebook the row is `marimo.hstack` over the maps, and elsewhere, as in Jupyter, an
    `ipywidgets.HBox`. Show it as a cell's last value, as a map is shown.

        mosaica.side_by_side(db.map(view="topics"), db.map(view="topics-wide"))

    A call with no map, or with something that is not a `Map`, raises `TypeError`.
    """
    if not maps or not all(isinstance(one, Map) for one in maps):
        raise TypeError(
            "side_by_side takes the maps to show as its arguments, such as "
            "side_by_side(db.map(view='a'), db.map(view='b'))"
        )
    marimo = sys.modules.get("marimo")
    if marimo is not None and marimo.running_in_notebook():
        return marimo.hstack(list(maps), widths="equal")
    import ipywidgets

    return ipywidgets.HBox(list(maps))
