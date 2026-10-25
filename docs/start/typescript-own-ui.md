# Your own interface around the map

The [first TypeScript tutorial](typescript.md) put the whole explorer on a page in one element. This one is for the developer who wants the map inside an interface of their own: their own search box and menus, their own list of results, their own panel for whatever the reader clicks. We'll keep Mosaica's map and build everything around it in plain TypeScript, with no framework.

The data this time is 50,000 papers from arXiv, the preprint server, laid out so that papers on similar subjects sit near each other. By the end you'll have a page with a menu of arXiv's archives filled from the server, a title search, a date range, a live count and the titles of the matching papers in view, a panel that opens the paper you click, and the map coloured by topic with each topic named. Clicking a topic's name narrows everything to it. Along the way you'll meet the store, the object that every Mosaica element reads from, and learn how a page of your own talks to it.

It takes about twenty minutes, and assumes you have done the first tutorial. You need the same things it did: `mosaica` installed, Node.js 22.15 or later with npm, and the Mosaica repository cloned to `~/mosaica`. The packages still come from the tarballs that tutorial's "Not published yet" note made in `~/mosaica-packs`.

Here is what we'll do.

1. Start Mosaica over the arXiv sample.
2. Make a page with a bare map in a layout of our own.
3. Drive the map's filters from the page's own controls.
4. Show a count and a list of the matching papers.
5. Open the paper the reader clicks in the page's own panel.
6. Colour the papers by topic, name the topics, and filter to one.

Everything goes in `~/papers`, with Mosaica's files in `~/papers/mosaica` and the web project in `~/papers/web`.

## Start Mosaica

This step follows the first tutorial's, with different data. Make a directory for the server and copy this example's settings into it:

```bash
mkdir -p ~/papers/mosaica
cd ~/papers/mosaica
cp ~/mosaica/clients/ts/examples/own-ui/mosaica/mosaica.toml .
```

```toml
--8<-- "clients/ts/examples/own-ui/mosaica/mosaica.toml"
```

The settings are the first tutorial's apart from the ports. The page will read the map from port 9241, and the session and control addresses move to 9242 and 9243, so this server can run beside the first tutorial's if that one is still going. `open = true` again lets anything on this machine read the data without signing in.

The arXiv sample has its own schema. Each paper has a `title`, which the schema marks for search, and an `arxiv_id`. Its `archive` and `primary_category` are categories, the archive being the broad area, such as `math` or `astro-ph`, and the primary category the subject class within it, such as `math.FA` for functional analysis. `submitted_at` is when it was submitted. The schema also declares two layouts of the papers, which Mosaica calls views, and two groupings of them, which it calls layers: arXiv's own classification, and topics a clustering program found in the layout and a language model named. We'll come back to the topics in the last step.

Build the bundle from the sample, then start the server in a terminal of its own:

```bash
mosaica build --config ~/mosaica/examples/data/arxiv/schema.toml
```

```
...
built /home/you/papers/mosaica/bundle (v00000): 50000 items, 175 terms, 86144 pairs, 15024990 bytes on disk, 0 artifact(s) minted, 0 unclustered member row(s), 0 row(s) refused
  view topics: 50000 row(s)
  view topics-wide: 50000 row(s)
```

`50000 items` is one per paper, and each of the two views places all of them.

```bash
mosaica serve
```

```
...
2026-10-25T16:34:50.918968Z  WARN mosaica_server: serve.open is set: this server is open to this machine, and every request, with or without a credential, sees and may change everything
{"event":"listening","viewer":"127.0.0.1:9241","session":"127.0.0.1:9242","control":"127.0.0.1:9243"}
```

## A map in a layout of your own

The explorer from the first tutorial is made of smaller elements, and you can use them on their own. `<mosaica-map>` is the map canvas. The explorer adds a filter panel of field cards, an item card for the point you click, a layer picker and a few more, and each of them reads the same object: the store.

The store is the part of Mosaica's client that has no pixels. It asks the server for what the camera can see, holds the filters, the layers and the colouring, and publishes the results as projections. A projection is a named piece of state, such as `view` for the counts or `selection` for the clicked item, and the store replaces it whole each time it changes. The elements draw the projections. Your own code can read them too, and can tell the store what to do. `<mosaica-store>` is an element that hands one store to every Mosaica element inside it.

Make the web project as before, and install the elements and the client they are built on. `@mosaicajs/client` holds the store.

```bash
cd ~/papers
npm create vite@latest web -- --template vanilla-ts --no-interactive
cd web
npm install @mosaicajs/components @mosaicajs/client
rm -r src/counter.ts src/assets public/icons.svg
```

While the packages are unpublished, install the three tarballs in place of the `npm install` line, as the first tutorial did:

```bash
npm install ~/mosaica-packs/mosaicajs-client-0.1.0.tgz ~/mosaica-packs/mosaicajs-deck-0.1.0.tgz ~/mosaica-packs/mosaicajs-components-0.1.0.tgz
```

Here is the whole page. Replace `index.html` with it:

```html
--8<-- "clients/ts/examples/own-ui/web/index.html"
```

The left column holds our controls, the right one a panel for a paper, a count and a list, and between them a `<mosaica-map>` inside a `<mosaica-store>`. The map has no attributes at all. None of the controls do anything yet: the rest of the tutorial wires them up one at a time.

Vite keeps the page's scripts and stylesheet in `src`. Replace `style.css` there with a three-column grid:

```css
--8<-- "clients/ts/examples/own-ui/web/src/style.css"
```

Most of it is ordinary layout. The last rule matters: the map is 420 pixels tall unless told otherwise, and `--mosaica-map-height: 100%` makes it fill its grid cell instead. `<mosaica-store>` takes no space of its own, so the map is the grid's middle column.

Now make the store. Create `mosaica.ts` in `src`:

```ts
--8<-- "clients/ts/examples/own-ui/web/src/mosaica.ts"
```

`MosaicaClient` sends Mosaica's HTTP requests. `viewerUrl` is the server's viewer address. `sessionUrl` is the address a backend uses to sign viewers in, which an open server doesn't need, so it is empty. `createStore` makes a store that sends its requests through this client. We export both because our own code will need them in a moment, the store for filters and the client for reading papers.

Replace `main.ts` with:

```ts
import '@mosaicajs/components';
import {store} from './mosaica.ts';
import './style.css';

document.querySelector('mosaica-store')!.store = store;
```

The first line defines the elements. The last hands our store to `<mosaica-store>`, and through it to the map. Start the development server on port 5273, which keeps it clear of the first tutorial's page:

```bash
npm run dev -- --port 5273
```

```
> web@0.0.0 dev
> vite --port 5273 --strictPort


  VITE v8.3.5  ready in 153 ms

  ➜  Local: http://localhost:5273/
  ➜  Network: use --host to expose
```

Open <http://localhost:5273/>. Each dot is a paper, and a tight clump is a group of papers on one narrow subject. The map's own toolbar is at its top left: pan, select a box, draw a lasso, and fit the map to what matches. The menus beside it do nothing yet.

## Drive the filters from the page

A filter in Mosaica is applied on the server, so the map and every count are computed over the papers that match. To filter from code, give the store a filter draft. A draft holds one control per field, and each control takes the shape that suits its kind of field: a list of keys for a category, a search query for text, and a lower and upper bound for a number or a time. An empty control constrains nothing. `store.setFilters` takes the whole draft, and the store asks the server again for everything on screen.

The archive menu should list the archives the data has, so we ask the server rather than typing them in. Create `filters.ts`:

```ts
--8<-- "clients/ts/examples/own-ui/web/src/filters.ts"
```

`store.setAggregate` asks the server to count the papers in groups, here one group per archive, and keeps the answer in the `aggregates` projection under the name we gave it, `archives`. `subject: 'visible'` counts every paper the viewer may see and ignores the filters, so the menu stays the same while the reader filters. Without it the counts would follow the filters, which suits a panel that shows how many match but not a menu. `top: 50` asks for up to fifty archives, largest first, which is more than the 38 in arXiv's list.

`store.subscribe('aggregates', ...)` calls our function each time that projection is replaced. The answer is a table with a row per archive: its key, such as `math`, its title from the archive vocabulary, and its count. Rows in the `listed` group are the archives themselves, as opposed to a row for everything outside the top fifty. Once the menu is filled we unsubscribe.

`apply` builds the draft from the controls. The archive's control is a category control with the chosen key, or none for All archives. The title's is a text control. Its query is read as search words, all of which must appear, in any order, and `phrase: true` lets the reader put a phrase in double quotes. Submission times are a timestamp field, and Mosaica counts a timestamp in microseconds since the start of 1970, so `startOf` and `endOf` turn a date from the date inputs into the first and last microsecond of that day. Each control calls `apply` whenever it changes.

Add the filters to `main.ts`, which now reads:

```ts
import '@mosaicajs/components';
import {filters} from './filters.ts';
import {store} from './mosaica.ts';
import './style.css';

document.querySelector('mosaica-store')!.store = store;
filters(store);
```

Save, and open the Archive menu. It lists `cs (10,478)` first, then `math (10,123)` and `astro-ph (6,084)`, down to archives arXiv has since folded into others, with a paper or two each. Choose `gr-qc`, arXiv's archive for general relativity and quantum cosmology, and the map keeps about a thousand dots, most of them in a few clumps. Type `black hole` in the title box and fewer remain. Set From to 1 January 2020 and fewer again. We can't see the counts yet, so that's the next thing to build.

## A count and a list of your own

We want two things on the right: how many papers match, and the titles of the first twenty. Both should cover the part of the map on screen, and follow the filters and the camera.

The count is already in the store. Its `view` projection has an `inView` part, which holds counts over the camera's box: `visible`, how many papers the viewer can see there, and `matched`, how many of those the filters let through. The store asks for them once the camera has been still for a quarter of a second.

The titles are not, because a map only needs the points, so we read them from the server. `client.items` reads the records of the items that match a filter expression, page by page. Create `results.ts`:

```ts
--8<-- "clients/ts/examples/own-ui/web/src/results.ts"
```

The count comes from subscribing to `view`. `formatMasked` writes a count with thousands separators, and marks one that is not exact.

For the list, the map fires `mosaica-viewchange` each time the camera moves, with the box on screen as `bbox` in the view's coordinates. `store.requestFilters()` gives the filter expression the store sends with every request, which `setFilters` composed from our draft. We join it to a `region` filter for the box with `all_of`, so the read returns papers that match and are on screen. `fields: ['title']` asks for the titles only, `pageRows: 20` and `pages: 1` ask for one page of twenty, and `order: 'map'` returns them in the order they lie on the map. We read that first page and stop the read with `read.return()`.

The first argument to `client.items` is the viewer's token. An open server needs none, so it is empty here. A server with users wants the same token you gave the store, and the [backend guide](../guides/backend.md) shows where a page gets one.

`soon` waits until the camera or the filters have been still for a quarter of a second before asking, so a pan sends one request rather than one per frame. An `AbortController` cancels a read the next one replaces.

Add it to `main.ts`, importing the client as well as the store:

```ts
import '@mosaicajs/components';
import {filters} from './filters.ts';
import {client, store} from './mosaica.ts';
import {results} from './results.ts';
import './style.css';

document.querySelector('mosaica-store')!.store = store;
filters(store);
results(store, client);
```

The page reloads with the filters cleared and reads "50,000 of the 50,000 papers in view match", with twenty titles under it. Choose `gr-qc` again and the count drops to 1,177. Type `black hole` and it reads 133, and every title in the list mentions a black hole. Set From to 1 January 2020 and 45 remain.

Now clear the filters and zoom into one of the clumps with the mouse wheel. The count covers only what is on screen, so both figures fall, and the list changes to papers from the part you're looking at. Every figure is computed from the papers this viewer is allowed to see. On an open server that is all of them, and on a server with users each person's counts and lists cover only what they may see.

## Open a paper in your own panel

When the reader clicks a dot, the map asks the server for that paper's record and fires `mosaica-itemopen` when it arrives. The event carries the record, with the paper's fields keyed by name. Create `paper.ts`:

```ts
--8<-- "clients/ts/examples/own-ui/web/src/paper.ts"
```

The title and the link come straight from the record. arXiv's address for a paper is `https://arxiv.org/abs/` followed by its identifier, so `arxiv_id` makes the link. The archive is stored as its key, and `store.titleOf` asks for the title the archive vocabulary gives that key. A vocabulary is the list of values a category field may take, each with a key and a title. This sample's vocabularies use the code as the title too, so the panel shows `gr-qc`. With data whose vocabulary carries readable names, the panel shows the name. `shown` remembers which paper the panel is for, so a slow answer about a previous paper doesn't overwrite it. A click on empty map fires `mosaica-miss`, and the panel closes.

Add two lines to `main.ts`, the import and `paper(store);` at the end:

```ts
import {paper} from './paper.ts';
```

```ts
paper(store);
```

Click a dot. Its title appears at the top of the right column, with its archive and a link that opens the paper on arXiv. The map draws a ring round the dot you picked.

## Topics

The sample comes with topics. A clustering program called Toponymy grouped the papers by where they sit on the map, at four levels from 16 broad topics down to 574 narrow ones, and a language model wrote a name for each cluster from the titles of a few of its papers. In Mosaica the clusters are one layer, `clusters/toponymy`, and their names another, `topics/toponymy`, attached to the clusters. Each cluster and each name is an artifact: something the server keeps alongside the papers, with its own `mosaica_id`.

Create `topics.ts`:

```ts
--8<-- "clients/ts/examples/own-ui/web/src/topics.ts"
```

`map.colourBy` colours the dots. Given `cluster:` and a layer, it gives each cluster a colour and each paper its cluster's. The checkbox sets `map.layers`, the layers the map draws. Asking for the names draws the clusters they name as well. These are the same properties as the `colour-by` and `layers` attributes, set from code here because the checkbox changes them.

Clicking a topic's name opens the cluster it names, and the map fires `mosaica-artifactopen` once its details arrive. The event's `id` is the artifact's `mosaica_id` as a decimal string, since it doesn't fit in a JavaScript number, and `detail.layer` is its layer. The name comes from the store's `artifacts` projection, whose `attached` map holds each cluster's name by its id.

`store.setMembers` filters to an artifact's members, the papers in the cluster. It takes a list of clauses, each naming an artifact by layer and id, with `outside: false` to keep the members and `verb: 'filter'` to narrow the map to them. The other verb, `highlight`, would keep every paper and light the members. Member clauses join the filters, so the topic combines with the archive, title and dates. Clear passes an empty list.

Add the import and the call to `main.ts`, which is now complete:

```ts
--8<-- "clients/ts/examples/own-ui/web/src/main.ts"
```

The page reloads with every paper coloured. Tick "Show topic names", and names appear over the clusters: Condensed Matter, AI Systems, Higgs Physics. Click "Black Holes and Quantum Gravity". The map keeps only that cluster, the left column shows the topic, and the count reads "1,261 of the 50,000 papers in view match". The list may surprise you, since it starts with papers on LISA, a gravitational-wave observatory, and on gamma-ray telescopes. A name was written from a few papers near its cluster's centre, and a cluster this broad also holds papers the name doesn't describe. Type `entropy` in the title box to search within the topic, and 17 papers remain. Empty the box and press Clear to bring every paper back.

## What you built

A page whose interface is entirely its own, around Mosaica's map. Its menu is filled from the server, and its filters reach the map and every count through one call to the store. Its list reads records over what is on screen, and its panels react to the map's events. You have used the three ways a page works with Mosaica: setting the store's state (`setFilters`, `setMembers`, the map's `colourBy` and `layers`), reading its projections (`view`, `aggregates`, `artifacts`), and listening to the map's events (`mosaica-viewchange`, `mosaica-itemopen`, `mosaica-artifactopen`).

The finished project is in the clone, in `clients/ts/examples/own-ui/`, with the same files in `mosaica` and `web`.

## Next

- The [`@mosaicajs/client` reference](../reference/typescript/client.md) lists the store's methods and projections, `MosaicaClient`, and the helpers that build filter drafts and member clauses.
- The [components reference](../reference/components.md) lists every element with its properties and events, for adding the item card, the layer picker or a field card to a page like this one.
- In React, [`@mosaicajs/react`](../reference/typescript/react.md) gives the store as hooks, and [`@mosaicajs/react/components`](../reference/typescript/react/components.md) every element as a component.
