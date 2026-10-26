<!-- The pages under components/ are generated from the element sources in clients/ts/components. Edit the doc comments there, then run: node clients/ts/scripts/reference.mjs -->

# Components

The `mosaica-*` elements are web components that put a Mosaica map and its controls in a page, with or without a framework. They are built with Lit and live in the `@mosaicajs/components` package. Each element draws from a store, the client-side state of the map described in the [TypeScript reference](typescript.md), so the elements on a page stay in step: a filter set in one changes the counts in all of them.

`<mosaica-explorer>` is a complete map with its panels. For a layout of your own, put `<mosaica-map>` and the controls you need inside a `<mosaica-store>`. Each element's page lists its attributes, properties, methods, events, slots, parts and CSS custom properties.

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


## Loading the elements

`import '@mosaicajs/components'` defines every element. To load fewer, import an element's own entry, such as `import '@mosaicajs/components/map'`, which defines that element and the ones it renders. The map, the explorer, the field card, the filter panel and the colour editor depend on deck.gl through `@mosaicajs/deck`; the other elements do not. `@deck.gl/aggregation-layers` is a peer dependency too, loaded only when a map first draws density as contours, so your bundler can put it in a chunk of its own.

For a page with no build step, use the single-file bundle `mosaica-components.js`, which includes Lit, deck.gl and the decode worker. Run `npm run bundle -w @mosaicajs/components` in `clients/ts` to write it to `components/dist/`, with its subresource-integrity hash in `mosaica-components.js.sri`:

```html
<script type="module" src="./mosaica-components.js" integrity="sha384-..."></script>
```

If the package is loaded twice on one page, each tag keeps its first definition.

## How an element finds its store

Every element except `<mosaica-count>` uses the first of these it has:

1. its `store` property;
2. the store of the nearest `<mosaica-store>` or `<mosaica-explorer>` above it;
3. for `<mosaica-map>`, `<mosaica-explorer>` and `<mosaica-store>` only, a store it builds itself from its `viewer-url`, `token` or `authorise`, and `artifacts-per-tile`.

With none of these, the element shows its detached state.

`viewer-url` is the base URL of the viewer server. Left unset, the element uses the page's own origin, which suits a page served by Mosaica or by a server that passes `/v1/` through to it. A server on another origin must list the page's origin in `serve.cors_origins`. Set `viewer-url` together with the token or `authorise`: an element given only one of them builds a store at once, and replaces it when the other arrives.

An element replaces a store it built when `viewer-url`, `token`, `authorise` or `artifacts-per-tile` changes, and disposes of it in `dispose()`. Moving an element in the page keeps its store. A store you pass in is yours to dispose of. A store serves one viewer, so to show the map to another viewer, give the element a new `token` or `authorise` function, or replace the element.

## Events

Every event is a `CustomEvent` that bubbles and is composed, so you can listen on the element or on any ancestor, including one outside `<mosaica-explorer>`. [Events](components/events.md) lists each one with its `detail`.

A click on a point fires `mosaica-pick` with its `mosaica_id`. When the item's record arrives, the map fires `mosaica-itemopen` with the id and the record. If the server refuses the record, `mosaica-itemopen` does not fire and the refusal is in the store's `selection.itemRefusal`; opened artifacts work the same way with `mosaica-artifactopen`. A category value in the record is a key, and `Store.titleOf` gives its display title:

```js
map.addEventListener('mosaica-itemopen', async (e) => {
  const archive = await map.activeStore.titleOf('archive', String(e.detail.record.fields.archive));
});
```

## The item card's Open button

`<mosaica-item-card>` and `<mosaica-explorer>` show an Open button only if you say what it does. Set `open-url` to a link template such as `https://www.geonames.org/{geonameid}`: each `{name}` is replaced by the item's value for that field, URL-encoded, and `{mosaica_id}` by its id, and Open becomes a link that opens in a new tab. Write the host in full, followed by a `/`, before the first field, since a field in the host part could name any host. An item with no value for a field the template names, or whose filled address is not `http:` or `https:`, gets no Open button.

To handle Open yourself, set the `open` property to a function; it is called with `{id, fields}` and takes precedence over `open-url`. Either way, `mosaica-open` fires on every press, including a middle-click. In React the props are `openUrl` and `open`.

`card-fields` lists the fields the compact card shows before Show all, in order, separated by spaces or commas.

## Styling

Style an element's parts with `::part()`. An element that renders another forwards the inner element's parts under its name, so through `<mosaica-explorer>` the map's toolbar is `::part(map-controls)` and the item card's title is `::part(item-card-title)`. `PARTS` in `@mosaicajs/components` lists each element's parts, for an app that renders an element in its own shadow root and forwards them. [Theme tokens](components/tokens.md) lists every CSS custom property with its defaults.

## When the server is busy

The store retries a request the server sheds with `429`, as [When the server is busy](typescript.md#when-the-server-is-busy) in the TypeScript reference describes. Meanwhile the status strip reads Reconnecting and the map keeps what it drew. If the server still refuses, the status strip and the map show Server busy, in the `busy` panel state, with a Retry button and `data-code="backpressure"` on the refusal part. A `429` from a proxy in front of the server looks the same, with the proxy's code on `data-code`. An item record the server sheds shows Item unavailable on the item card.
