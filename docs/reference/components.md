<!-- The pages under components/ are generated from the element sources in clients/ts/components. Edit the doc comments there, then run: node clients/ts/scripts/reference.mjs -->

# Components

The `mosaica-*` custom elements, in the `@mosaicajs/components` package, built with Lit. Each element's page lists its attributes, properties, methods, events, slots, parts and CSS custom properties, generated from its source.

| Element | What it is |
|---|---|
| [`<mosaica-explorer>`](components/mosaica-explorer.md) | The map and all its panels, in a ready-made layout. |
| [`<mosaica-map>`](components/mosaica-map.md) | The interactive map on its own, without panels. |
| [`<mosaica-store>`](components/mosaica-store.md) | Shares one store with the elements inside it. |
| [`<mosaica-status>`](components/mosaica-status.md) | One line with the view's state and counts. |
| [`<mosaica-count>`](components/mosaica-count.md) | A count, formatted as the other elements show it. |
| [`<mosaica-layer-picker>`](components/mosaica-layer-picker.md) | Checkboxes for the annotation layers the map draws. |
| [`<mosaica-view-picker>`](components/mosaica-view-picker.md) | A menu for switching views. |
| [`<mosaica-key-picker>`](components/mosaica-key-picker.md) | Steps through the views in the current view's group. |
| [`<mosaica-filter-panel>`](components/mosaica-filter-panel.md) | A field card per field, with the filters in force as chips. |
| [`<mosaica-field-card>`](components/mosaica-field-card.md) | A chart of one field's values, for filtering and highlighting by them. |
| [`<mosaica-filter>`](components/mosaica-filter.md) | A search box that filters by one column. |
| [`<mosaica-cluster-filter>`](components/mosaica-cluster-filter.md) | A search box that filters by a layer's clusters. |
| [`<mosaica-selection>`](components/mosaica-selection.md) | The counts and marks inside the selected region. |
| [`<mosaica-item-card>`](components/mosaica-item-card.md) | The picked item's record, with Open and Copy id. |
| [`<mosaica-artifact-card>`](components/mosaica-artifact-card.md) | An opened artifact, such as a cluster, with its parents, children and filters. |
| [`<mosaica-hierarchy>`](components/mosaica-hierarchy.md) | A layer's tree of artifacts, browsed apart from the map. |
| [`<mosaica-colour-editor>`](components/mosaica-colour-editor.md) | A dialog for choosing the colour of each value or cluster. |

[Events](components/events.md) lists every event with its `detail`, and [Theme tokens](components/tokens.md) every CSS custom property with its defaults.

## Loading the elements

`import '@mosaicajs/components'` defines every element. Each element also has its own entry, such as `import '@mosaicajs/components/map'` or `import '@mosaicajs/components/filter'`, which defines that element and the elements it renders. The map, the explorer, the field card, the filter panel and the colour editor import `@mosaicajs/deck`, which depends on deck.gl; the other elements do not. `@deck.gl/aggregation-layers` is a peer dependency as well, which `@mosaicajs/deck` imports only when a map first draws density as contours. A host's bundler puts it in a chunk of its own, which a page that never draws those does not load.

A page with no build step loads the single-file bundle, `mosaica-components.js`, which holds Lit, deck.gl, its aggregation layers and the decode worker. `npm run bundle -w @mosaicajs/components` in `clients/ts` writes it to `components/dist/` with its subresource-integrity hash in `mosaica-components.js.sri`:

```html
<script type="module" src="./mosaica-components.js" integrity="sha384-..."></script>
```

A tag already defined keeps its first definition, so two copies of the package on one page do not clash.

## How an element finds its store

Every element but `<mosaica-count>` reads a store, and takes the first of these that it has:

1. its `store` property;
2. the store of the nearest `<mosaica-store>` or `<mosaica-explorer>` above it;
3. for `<mosaica-map>`, `<mosaica-explorer>` and `<mosaica-store>` only, a store it builds from its `token` attribute or `authorise` property, its `viewer-url` attribute, and its `artifacts-per-tile` attribute, the most artifacts one level of a drawn layer shows in one tile. Unset, the store asks for 50, or `/v1/meta`'s `selection.max_artifacts_per_tile` where that is smaller. A number the page sets is sent as it is, and one over that maximum is refused by the server. The store asks for artifacts by tile at map zoom + 2, the map's own zoom rounded down even when the world is smaller than the map, so a tile is 128 to 256 pixels across. It names at most 558 tiles in one request, the most a 3840 by 2160 screen touches at that depth, or the deployment's `max_tiles_per_request` where that is fewer. A larger screen is asked for at zoom + 1, then coarser, until its tiles fit. So the tiles in view times `artifacts-per-tile` bounds the artifacts one level draws.

`viewer-url` is where the viewer routes are. Each route, such as `/v1/meta`, is appended to it after any trailing slash is dropped. Unset, the element asks the page's own origin, so a page served from the same server as Mosaica, or from a server that passes `/v1/` through to it, needs no `viewer-url`. A path such as `/mosaica` reaches a viewer listener that the page's server passes through under that path, and a full URL such as `https://maps.example.org` reaches one on another origin, which has to list the page's origin in `serve.cors_origins`. An element given a token or `authorise` with no `viewer-url` talks to the page's own server at once, so a host that learns its server's address later sets `viewer-url` before the token.

With none of the three, the element renders its detached state. An element that has built its own store keeps it when a provider appears above it later. A store the element built is replaced when `viewer-url`, `token`, `artifacts-per-tile` or the `authorise` function changes, and disposed by the element's `dispose()`. A store serves one viewer, as `Store` in the [TypeScript reference](typescript.md) sets out: to show another viewer, give the element a new `token`, a new `authorise` function, a new `viewer-url`, or a new element. Without one of these, the previous viewer's data stays on screen for as long as `Store` in the TypeScript reference says. An element also drops the pages, names and hover it fetched itself when it adopts another store and when its store forgets what the server answered. A store it was given is left for its owner to dispose. Disconnecting an element keeps its store, so moving it in the page does not fetch the view again.

## Events and styling

Every event is a `CustomEvent` that bubbles and is composed, so a host listens on the element or on any ancestor, including one outside `<mosaica-explorer>`.

A click on a point fires `mosaica-pick` with its `mosaica_id`. Once the selected item's record arrives, the map fires `mosaica-itemopen` with the id and the record, whether the item was selected by a click or by `Store.pick`. Where the server refuses the record, `mosaica-itemopen` does not fire and the refusal is the store's `selection.itemRefusal`, as an opened artifact's is with `mosaica-artifactopen`. A category value in the record is a key; `Store.titleOf` gives its title, as the item card shows it. The server answers it as `/v1/categories` would list the value: a value of a `public` vocabulary has its title whether or not the viewer may see an item that carries it, and a value of a `derived` vocabulary only where the viewer may see one. A handler asks for it so:

```js
map.addEventListener('mosaica-itemopen', async (e) => {
  const archive = await map.activeStore.titleOf('archive', String(e.detail.record.fields.archive));
});
```

`<mosaica-item-card>` and `<mosaica-explorer>` take `card-fields`, the fields the compact card shows before "Show all", separated by spaces or commas and in that order. A name the record has no value for shows nothing.

The item card's Open does what the host says, and is shown only where the host says something. `open-url` is a link template such as `https://www.geonames.org/{geonameid}`: each `{name}` is replaced by the item's value for the field `name`, URL-encoded, and `{mosaica_id}` by its id. Open is then a link to that address, opening in a new tab. A field in the host part of `open-url` can name any host, so write the host in full, followed by a `/`, before the first field. An item with no value, or an empty one, for a field the template names has no Open, nor has one whose address, filled and resolved against the page, is not `http:` or `https:`. The `open` property is a function, called with `{id, fields}` when Open is pressed; where both are set, `open` is called and `open-url` is unused. `mosaica-open` fires on every press, a middle-click included. `<mosaica-explorer>` passes both to its item cards, and the React wrappers take them as the props `openUrl` and `open`.

An element's parts are styled with `::part()`. An element that renders another inside its shadow root forwards the inner element's parts under its name: through `<mosaica-explorer>`, the map's toolbar is `::part(map-controls)` and the item card's title is `::part(item-card-title)`. `PARTS` in `@mosaicajs/components` lists each element's parts, for a host that renders an element in its own shadow root and forwards them.

## When the server is busy

The elements reach the server through their store, which sends a map request or an aggregate the server sheds with `429` again up to twice, as [When the server is busy](typescript.md#when-the-server-is-busy) in the TypeScript reference sets out. Meanwhile the status strip reads Reconnecting and the map keeps what it drew. A map request the server still sheds shows Server busy on the status strip and over the map, in the `busy` panel state, with `data-code="backpressure"` on the refusal part and a Retry button that asks again. A `429` from a proxy in front of the server shows the same, with the code it came with on `data-code`, and the selection card says Server busy where its count was shed. An item record the server sheds shows Item unavailable on the item card, with the server's reason under it, as View refused has.
