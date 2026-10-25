# Maps in the browser

A Mosaica map in the browser is built in layers, each usable on its own:

- **The store**, in `@mosaicajs/client`, decides what to ask the server for, holds what comes back, and publishes what to draw. It draws nothing itself.
- **The deck.gl layer**, `MosaicaLayer` in `@mosaicajs/deck`, draws a store's points, topics and density, in a deck.gl scene of your own or inside the components.
- **The components**, in `@mosaicajs/components`, are web components built on both: a whole explorer, a map, and panels such as a filter panel, a field card or a count. `@mosaicajs/react` wraps each one for React.
- **The Python notebook map** embeds the components, so it behaves as they do.

A host with its own drawing can use the store alone. This chapter describes what the store does, and then what the deck layer and the components add. The [TypeScript reference](../reference/typescript.md) and the [components reference](../reference/components.md) list every option.

## The store

The host tells the store where the camera is, and which view, filters, layers and colouring are chosen. The store does the rest:

- **It gets tokens from the host.** The host gives it a fixed `token`, or an `authorise` function, which the store calls for a token and again to renew it 30 seconds before it expires.
- **It asks only for what is worth asking.** It picks the zoom level whose tiles give about as many points as its point budget, 250,000 by default. It waits for the camera to settle before asking for more, and then fetches a margin around the screen.
- **It keeps what it has fetched**, and asks only for tiles it does not hold. Topics come from a request of their own, tile by tile, so they stay put as the held points change.
- **It keeps topic colours stable.** The server gives each topic a colour slot, the same for this viewer at every zoom and filter, in one of four palettes: Okabe-Ito, Tableau 10 (the default), Tableau 20 or Kelly.
- **It ranks category values for colouring** by the server's count of each over the viewer's items, so a value has the same colour on every load. If that count is refused, it ranks the values already drawn.
- **It counts for panels.** A component registers the breakdowns it needs, such as a histogram for a field card. The store sends registrations with the same filters as one aggregate request, and asks again when the filter, the highlight, the selected region or the view changes, at a refresh, and when the data changes.

### What the view shows

The store reports the view's state:

| State | Meaning |
|---|---|
| `idle` | nothing asked for yet |
| `loading` | a request is out |
| `retrying` | the server was busy (`429`) or still starting (`503`), and the request will be sent again |
| `shown` | points are on screen, with their counts |
| `empty` | the answer holds nothing this viewer may see in view |
| `refused` | the request failed, so what is here is not known |

A refusal carries the server's error code and the HTTP status. An error with no answer from Mosaica in it, such as a gateway's `504`, has the code `http-error`.

Flags sit beside the state:

- **`stale`**: the data has changed since the points on screen were fetched. The store does not fetch again by itself: it marks the view, and a refresh is one action away.
- **`unreachable`**: a request could not reach the server, or a proxy in front of it answered `502`. The store asks the server again after a second, then twice as long each time, up to 30 seconds, and redraws once it answers. A refused request, or an `authorise` that fails, is not an outage.
- **`expired`**: the session has ended, and a new token is needed. A `401` on a token that had worked ends the session, and the store calls `authorise` once for a new one.

A store given a `MosaicaClient` of its own does not watch that client's requests, so `unreachable` stays false and a `401` does not call `authorise`.

### When the store lets go of what it holds

| What happened | What the store does |
|---|---|
| The data changed under the same viewer | Marks the view `stale`, and keeps what is on screen until it is refreshed |
| An answer carries a different identity key: another viewer, a renewed token whose viewer now holds different access, or a compaction | Drops what the server answered, reads the deployment's description again, and redraws |
| The host calls `clear()` | The same, for a host that changes viewer itself |
| The database was rebuilt | Every `mosaica_id` it holds names nothing. Read the items again by a unique field's values |

When it drops what the server answered, the store keeps the host's own choices: a drawn region, the filter controls, the layers, the colouring, the view and the camera. It drops what came from the server, including a topic picked as a filter or a region taken from a topic's shape.

After a token renewal, the store asks whose the new token is. If the answer does not come, because the server is busy, starting or unreachable, it asks again with a growing wait until an answer confirms the viewer or shows another. Until then the previous viewer's data stays on screen, so a host that changes viewer calls `clear()` rather than relying on renewal.

A merge of segments never reaches a client. A compaction reaches it only through the identity key, and every `mosaica_id` stays valid across it.

### Views

The store keeps what it has fetched for each view the viewer has visited, so a view they return to is as they left it. All views share one memory budget, and the least recently drawn leave first.

- **Within a view group sharing one layout**, such as a year shown quarter by quarter, a switch keeps the camera and the selection, and asks the next view for the same tiles.
- **Between layouts**, a switch drops the selection and the camera, since a position in one layout means nothing in another. The host sets a camera for the new view. The components fit it to the view's extent.

## What the deck layer and the components add

- **The level.** On a layer with levels, a map with no level chosen colours, names and counts the topics at the top level served at the camera's zoom.
- **Density shading**, counted with the aggregate route, so it follows the filter and the highlight as the points do. The scale is logarithmic by default, since counts on most maps are skewed.
- **Colours.** A category is coloured from Tableau 10 by default, Okabe-Ito, Set 2 or Dark 2, and a number from Viridis by default, Cividis or Magma. A user can pick a value's or a topic's colour on a field card or in the colour editor. Each choice is reported in an event, and the client keeps none of them: a host that wants them kept stores them and passes them back.
- **Status in plain words**, on the status strip and over the map: Connecting or Updating, Up to date, Nothing in view, View refused, Server did not answer (with Retry), Reconnecting or Server unreachable, Session expired, and Data updated.

## Not built yet

- **Animating items between layouts.** A switch between views with different layouts redraws, rather than moving each item to its new position.
- **Fetching a view before it is chosen.** A view the viewer has not visited is fetched when they switch to it.
- **An adapter for tile-based map libraries.** The store asks for an area at a time, and nothing sits between it and a library that asks for one tile at a time.
