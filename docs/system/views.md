# Views and projections

A view lays your items out on a map, giving each item a position. You can have as many views as you like over the same items, such as a world map of where each record was made and a layout of what it says, and a viewer can switch between them without losing their filters.

## Plain and geographic views

If your items have longitude and latitude, use a **geographic view**, which draws them with a map projection:

| Projection | Draws the world |
|---|---|
| `web_mercator` | As web maps do. It keeps shapes but enlarges areas towards the poles, and stops at about 85° north and south |
| `equirectangular` | As a plain grid of longitude and latitude, twice as wide as it is tall. It reaches the poles, but stretches shapes away from the equator. Also written `plate_carree` |
| `gall_isographic` | As the same grid drawn √2 times as wide as it is tall, so shapes are true at 45° north and south rather than at the equator |

All three store the same positions, so they differ only in the width-to-height a client draws the world at.

Any other two coordinates go in a **plain view**, which draws its x and y as they are. That suits a 2D projection of embeddings, such as UMAP or t-SNE, where nearby items are similar, as well as a floor plan, a microscope slide or a chart of two measurements.

## The frame

Each view stores its positions across a **frame**. You can give its range yourself, or write `auto` to fit it to the points of the first load, with a small margin.

!!! note "The frame does not grow"
    An `auto` frame is fitted once, at the first load. A point outside the frame, inserted later or read in the first load, is drawn on its edge, and the build's report or the insert's answer counts how many were. A point beyond `web_mercator`'s limit near the poles is counted separately, as clipped. A group whose views will arrive later needs a margin or an explicit frame wide enough for them. A view added to a running database has no data to fit, so it needs an explicit frame, given in the view's stored coordinates rather than degrees; `mosaica check --payloads` prints it.

The [corpus.toml reference](../reference/corpus-toml.md#viewextent) lists every way of writing a frame, and the [Python reference](../reference/python/index.md) covers `declare_view`.

## Several views of the same items

Each view gives an item its own position, but everything else about it, its fields, its access label and its `mosaica_id`, is shared, so a viewer who switches views keeps their filters.

To place an item in a second view, give it a position there that names it: in a first load from files, a row of the second view's points file carrying the item's [unique field](data-model.md#naming-an-item), and later, an insert into that view naming the item by that field or its `mosaica_id`. Without a unique field, a first load can't tell that two rows in two files are one item, and makes each its own. An item doesn't have to be in every view; a view holds the items given a position in it.

## View groups

Sometimes you want many views of the same kind, such as a layout recomputed each year, with next year's added when it arrives. A view group gives you that: each view in it has its own key, such as `2024`, is named after the group and its key, as in `by_year:2024`, and holds its own items and positions, while the projection, the frame and the group's access label are shared. Each view may narrow who sees it.

You can create and drop a group's views while the database runs. A group can also carry facts about each view, such as the date range a year covers, and a field can hold a different value for each view, such as a score recomputed every year.

!!! warning "Dropping a view deletes items that are in no other view"
    When a view of a group is dropped, any item that has a position in no other view is deleted, as if you had deleted it yourself. The answer to the drop says how many. Items that are also in another view are not affected.

## Switching views

A viewer can switch between views without losing their filters, since those are about the items, not the layout. What else they keep depends on whether the two views share a layout:

- **Within a view group sharing one layout**, such as a year shown quarter by quarter, a switch keeps the camera and the selected region, and the map asks the next view for the same tiles.
- **Between layouts**, a switch drops the selected region and the camera, since a position in one layout means nothing in another. The map frames the new view on the items the filters match there.

## Basemaps

A geographic view draws an outline of the world under its points unless you say otherwise: land, coastlines and country borders from Natural Earth, which the database serves at `GET /v1/basemaps/world` and the map projects the same way as the points. A plain view draws nothing underneath. To choose something else, give the map a basemap:

| Value | Draws |
|---|---|
| `"world"` | The world outline. It needs a geographic view |
| `"none"` | Nothing |
| `{tiles, attribution}` | Raster tiles from your own tile server: `tiles` is a URL template with `{z}`, `{x}` and `{y}`, and `attribution` is the credit shown in the map's corner. It needs a view whose frame lines up with web map tiles, as `tile_scheme` in `/v1/meta` says. There is no default tile server |
| `{image, bounds}` | One image stretched over `bounds`, `[x0, y0, x1, y1]` in the view's own coordinates, or over the whole frame. The image isn't reprojected |

To give each view its own basemap, pass an object keyed by view id. A view it leaves out gets the default.

=== "Python"

    `db.map`, `Selection.map` and `Viewer.map` take a `basemap=` argument. An image there can also be a local file, which Python reads and sends to the page.

    ```{.python notest}
    db.map(basemap={"tiles": "https://tiles.example.com/{z}/{x}/{y}.png",
                    "attribution": "© Example"})
    ```

=== "Web components"

    Set the `basemap` property on `<mosaica-map>` or `<mosaica-explorer>`.

    ```js
    document.querySelector("mosaica-map").basemap = {
      image: "/floorplan.png",
      bounds: [0, 0, 100, 60],
    };
    ```

If the value doesn't fit the view the map opens on, the map says so and draws the points without it. A value that doesn't fit a view the viewer switches to later draws nothing there, with a note in the map's corner saying why. A basemap that fails to load shows a note too, and is tried again after one second, then two, doubling up to a minute.

??? info "Details for hosts"
    - An object with a `tiles` or `image` key is one value, and anything else is a mapping, so a view named `tiles` or `image` can't be named in a mapping. A key a form doesn't take is refused.
    - In Python, a value of the wrong shape raises `traitlets.TraitError`, and one the page refuses is reported in `last_error` and a warning.
    - Each time a note appears, changes or goes, `<mosaica-map>` fires `mosaica-basemapchange`. The note and the tile credit are the map's `note` and `attribution` parts, and the explorer forwards them as `map-note` and `map-attribution`.
    - A deck.gl host without the components draws a basemap with `basemapLayers(store, view, value, options)` from `@mosaicajs/deck`, and checks a value against a view with `basemapMisfit(view, value)`. The `colours` option is the only way to restyle the world outline.
    - The map also takes a deck.gl layer you built yourself, drawn in its 512-unit world.

## Not built yet

- **Dropping a plain view.** A view in a view group can be dropped.
- **Category-typed view facts in a group declared while the database runs.** A group declared in the first load can carry a category fact about each view, such as a region drawn from a vocabulary, but the same group declared later is refused. Declare such a group in the first load.
- **Animating items between layouts.** A switch between views with different layouts redraws, rather than moving each item to its new position.
