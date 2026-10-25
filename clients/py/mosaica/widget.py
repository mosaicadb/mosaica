"""The notebook widget, `Map`: the kernel half of the protocol in `components/src/widget.ts`.

The page half is the components' single-file bundle, which anywidget runs as `_esm`. This half
holds the token, answers the page's `ready` and `reauthorise` messages with it as a custom
message, and mirrors the map's controls and selection as traitlets.

No traitlet carries the token, so nothing that saves widget state saves it: not the front end's
"save widget state", `nbconvert --execute`, papermill, or a run in which no front end mounts. The
token stays in the browser's memory while the page is open.

The page calls `url`, the viewer plane, from the notebook page's origin, so the viewer plane must
allow that origin. A database made with `create()` allows pages served from a loopback address,
through `serve.cors_loopback`; another deployment names the notebook's origin in
`serve.cors_origins`. A VS Code notebook and Colab render in origins no list can name, so the
widget cannot reach a database from them. Not built yet: a Jupyter server extension that would
forward the widget's requests from the notebook's own origin.

Only controls and selection cross the kernel boundary, never data. `url` goes down. `view`,
`bbox`, `layers`, `colour_by`, the four size settings and `filters` go both ways, and up only
when the map settles, once it has finished fetching for a view, so the kernel is never asked on
every frame. `budget` goes both ways, and up when the map's Most points slider is let go;
`palette` goes both ways, and up when one is chosen in the map's Colour section;
`value_colours` and `cluster_colours` go both ways, and up as each colour is chosen in the map.
`selected` and `selected_artifact` go up on a pick, and `region` when a region's
counts arrive. Ids are decimal strings, because a `mosaica_id` is a `u64`, which is not a
JavaScript number, and a `BigInt` does not serialise.

`basemap` goes down as `_basemap`: the value, with each image it names by a local path given as
`{"local": path}`, the path as the caller wrote it, and for each such path the number of the read
whose bytes the page is to draw. So the page takes the value and its images together, and the
saved state holds no more of a path than the caller wrote, and neither an image nor anything
drawn from its content. The file is read here each time the value is set, and its
read number moves on only where its bytes or type changed. The bytes go to the page as a
`basemap-image` message with a binary buffer, in answer to `basemap-images`, which names the
reads the page lacks.
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
    """The path of the components' single-file bundle that `Map` loads, or `None` where there
    is none.

    The bundle the wheel's build hook packaged under `mosaica/static/` comes first, then the
    checkout's own build in `clients/ts/components/dist/`.
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
    """The interactive map in a notebook cell, reading a Mosaica database with a token.

    The page in the browser fetches its data from the database itself, as the token allows. The
    token reaches the page as a message and is never widget state, so saving the notebook does
    not save it.

    - `url`: the address of the database's viewer plane.
    - `token`: required, though the signature gives it a default of `None`. The token your
      deployment issued you, as a string, a `Token` from `login` or `authorise`, which is renewed
      before it expires, or a function returning either, which is called again then. A `Map`
      without one raises `TypeError`.
    - `view`: the view to open on. `None`, the default, opens the first one.
    - `layers`: the annotation layers to draw, each with the layers it depends on. `None`, the
      default, lets the map choose, and `[]` draws none. `"all"` is not a layer name and is
      refused.
    - `colour_by`: the column to colour points by, or `"cluster:<layer>"` to colour them by the
      annotations of that layer.
    - `palette`: the palette annotations are coloured from under `"cluster:<layer>"`:
      `"okabe-ito"` (8 colours, colour-blind safe), `"tableau10"` (10), `"tableau20"` (20, in
      light and dark pairs) or `"kelly"` (22, most distinct). The server gives each annotation a
      slot below the palette's size, chosen so that annotations drawn beside each other differ,
      and the map draws it in the palette's colour at that slot. `None`, the default, leaves the
      map's choice, Tableau 10 until one is made.
    - `value_colours`: colours for single values of category columns, as
      `{column: {value: "#rrggbb"}}`, in place of their palette colours. `None`, the default,
      leaves the colours chosen in the map; `{}` gives every value its palette colour.
    - `cluster_colours`: colours for single annotations, per layer, as
      `{layer: {mosaica_id: "#rrggbb"}}` with each `mosaica_id` a decimal string, in place of
      their palette colours under `"cluster:<layer>"`. `None`, the default, leaves the colours
      chosen in the map; `{}` gives every annotation its palette colour.
    - `size_by`: a number column to size points by. `None`, the default, draws every point at
      one size.
    - `size_min`, `size_max`: the radius in pixels of the smallest and the largest value under
      `size_by`. A point with no value draws as a ring. The page ignores a radius that is not a
      number above zero. `None` leaves the map's choice, 2 and 9 until one is made.
    - `size_scale`: how values are placed between the two radii: `"linear"`, `"log"`, or
      `"rank"` among a sample of the values the map has drawn. `None` leaves the map's choice,
      linear until one is made.
    - `filters`: a filter expression, as `Selection.filter` takes one.
    - `bbox`: the box to frame the camera on, as `(min_x, min_y, max_x, max_y)`. Without it the
      map opens framed on where the items `filters` matches are.
    - `height`: the widget's height in pixels in the notebook. The default is 480. While the
      output holding the map is shown full screen, the map fills the screen instead.
    - `basemap`: what is drawn under the points. `"world"` is land, coastlines and country
      borders from the database's world outline, on a view with a map projection. `"none"` is
      nothing. `{"tiles": url_template, "attribution": text}` is raster tiles from a server you
      name, whose template holds `{z}`, `{x}` and `{y}`, on a view whose frame lines up with web
      map tiles; the map shows `attribution` in its corner. `{"image": url_or_path, "bounds":
      [x0, y0, x1, y1]}` is one image stretched over `bounds` in the view's coordinates (degrees
      on a geographic view), or over the whole frame without them, and not reprojected. A
      `pathlib.Path`, and a string that is not an `http(s):` or `data:` URL, is a path here,
      not expanded for `~`; it is read each time the value is set and sent to the page as a
      message, so its bytes are never saved with the notebook. A file that cannot be read
      raises `traitlets.TraitError` and leaves `basemap` as it was; a `file:` URL is refused.
      A key holding `None` is left out. A dictionary with a `tiles` or `image` key is one of
      these, and any other maps view ids to values, for the views the map can switch to; so a mapping cannot name a view called `tiles` or `image`, and
      `{"image": "world"}` is an image at the path `world`. `None`, the default, is `"world"`
      on a view with a map projection and nothing on one without. A value of the wrong shape
      raises `traitlets.TraitError`; one that does not fit the view is refused by the page, and
      `last_error` and a warning say why.
    - `explorer_layout`: `"docked"`, the default, puts the controls in a sidebar beside the map;
      `"overlay"` draws the map across the whole widget with the controls floating over it.
    - `title_field`: the field whose value titles a point in the hover and the item card.
      `None`, the default, titles a point by its id.
    - `artifacts_per_tile`: the most annotations each level of a layer shows in one tile of the
      map, largest first, at most the server's `max_artifacts_per_tile`. `None`, the default,
      asks for 50, or the server's `max_artifacts_per_tile` where that is smaller. The map asks
      for annotations by tile at map zoom + 2, the map's own zoom rounded down, a tile of 128 to
      256 pixels, and at most 558 tiles in one request, the most a 3840 by 2160 screen touches,
      or the server's `max_tiles_per_request` where that is fewer; a larger screen is asked for
      at zoom + 1, then coarser, until its tiles fit. So the tiles in view times
      `artifacts_per_tile` bounds the annotations one level draws.
    - `budget`: how many points the map aims to draw on screen. `0`, the default, leaves the
      map's own, 250,000.
    - `budget_min`, `budget_max`: the ends of the Most points slider in the map's Layers
      popover. The defaults are 1,000 and 2,000,000. Neither changes `budget`.
    - `cluster_budget`: the most clusters a `nested` or `dag` layer is drawn with, met by drawing
      parents in place of their children, and the cut a cluster card ranks at. The default is
      1,000; `0` draws every cluster, and setting it to `0` later goes back to every cluster.
    - `cluster_budget_min`, `cluster_budget_max`: the ends of the Most clusters slider, shown in
      the Layers popover while such a layer is drawn. The defaults are 10 and 10,000. Neither
      changes `cluster_budget`.

    Read the widget's attributes in a later cell:

    - `selected`: the `mosaica_id` of the picked item, as a decimal string, or `None`. It
      changes on a pick.
    - `selected_artifact`: the `mosaica_id` of the opened annotation, likewise.
    - `region`: the drawn box or lasso, as a dictionary: its `shape` and `status`, the counts
      `visible`, `matched` and `served` inside it, `verdict`, which says whether the counts are
      exact for the shape or over the grid cells covering it, and `refusal` where the server
      refused it. It changes when the region's counts arrive, and is `None` when it is cleared.
    - `view`, `bbox`: the view shown and where the camera settled.
    - `filters`, `layers`, `colour_by`, `palette`, `size_by`, `size_min`, `size_max`,
      `size_scale`: as the map shows them. Setting one redraws the map.
    - `last_error`: why the page last refused something set here, or `None`.
    - `budget`: as the map draws it; it changes when the Most points slider is let go. Setting
      it in a later cell applies it.
    - `cluster_budget`: likewise, when the Most clusters slider is let go.
    - `value_colours`, `cluster_colours`: every colour chosen, here or in the map, which the map
      changes when a colour is chosen on a field card or in Edit colours, at once. Setting one
      replaces every colour of its kind the map holds.
    - `basemap`: as given, normalised: bounds as floats, and a key holding `None` left out; an
      image's path stays the `str` or `pathlib.Path` it was given as. Setting it in a later
      cell applies it and reads its local images again, so setting the same value after
      editing an image shows the edit.
    - `url`, `height`, `explorer_layout`, `title_field`, `artifacts_per_tile`, `budget_min`,
      `budget_max`, `cluster_budget_min`, `cluster_budget_max`: as given. Setting one in a later
      cell applies it.
    - `tokens_sent`: how many tokens the kernel has sent the page.

    `view`, `bbox`, `filters`, `layers`, `colour_by` and the size settings change when the map
    settles, once it has finished fetching after a pan or zoom, and not during one. In marimo,
    `mo.ui.anywidget(m)` puts every attribute in one `.value`, so a cell that reads it runs again
    at every settle. To react to a pick alone, call `m.observe(fn, names="selected")` on the `Map`.

    Setting `filters` applies the expression at once. Reading it after the filter panel changes
    gives the expression the panel built. An expression the panel cannot show, such as
    `any_of`, `none_of` or two tests on one column, is refused by the page: nothing is applied,
    and `last_error` and a warning say why.

    The map needs the components' bundle, which the package's build hook builds with Node into
    every wheel and every install from a checkout, and `bundle_path()` finds; a `Map` made
    without it raises `RuntimeError` naming what to install.

        m = mosaica.Map(viewer_url, token=token, colour_by="venue")
        m
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
