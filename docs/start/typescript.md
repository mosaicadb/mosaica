# Your first map in a web page

This tutorial is for a developer who wants Mosaica's map inside their own web application, and wants to see it working before reading much else. We'll start a Mosaica server over 150,000 places from GeoNames, a free gazetteer of the world's place names. Then we'll make a new web project with Vite, put the `<mosaica-explorer>` element on its page, and end with the map sitting in an ordinary page layout of our own, with a search box and filters built from the data.

Once `mosaica` is installed, as [Install Mosaica](install.md#install-mosaica) shows, it takes about ten minutes. You need Node.js 22.15 or later with npm, and a clone of the Mosaica repository, which holds the sample data:

```bash
git clone https://github.com/mosaicadb/mosaica ~/mosaica
```

Here is what we'll do.

1. Start Mosaica over the sample data.
2. Make a web project and install Mosaica's elements.
3. Put the explorer on the page, and search and filter the places.
4. Choose which filters show, how the places are coloured and what their cards say.
5. Fit the explorer into a page with a header and a side column.

We'll keep everything in one directory, `~/places`, with Mosaica's files in `~/places/mosaica` and the web project in `~/places/web`.

## Start Mosaica

Mosaica is a database built to send millions of points to a browser. Like most databases, it runs as a server, and your web page talks to it over HTTP.

Make a directory for the server's files, and copy the example's settings into it:

```bash
mkdir -p ~/places/mosaica
cd ~/places/mosaica
cp ~/mosaica/clients/ts/examples/getting-started/mosaica/mosaica.toml .
```

Mosaica reads its settings from `mosaica.toml`. This one says:

```toml
--8<-- "clients/ts/examples/getting-started/mosaica/mosaica.toml"
```

`[bundle]` names the bundle, the database's own copy of the data, arranged so that any part of the map can be read quickly. `[serve]` gives the address the page will read the map from, port 9141 on this machine.

`open = true` lets anything on this machine read and change the data without signing in. That suits one person working on their own computer, which is what we're doing. A server that other people use gives each of them a token, which decides what that person sees. [Call Mosaica from a backend](../guides/backend.md) shows how your server gets one for each user.

Now create the database and load the sample into it. The repository's sample comes with a schema, `schema.toml`, which tells Mosaica what each column of the data means. `name` is text to search, for instance, and `feature_class` and `country` are categories. `--config` points the build at it.

```bash
mosaica build --config ~/mosaica/examples/data/geonames/schema.toml
```

```
...
built /home/you/places/mosaica/bundle (v00000): 150000 items, 245 terms, 150000 pairs, 18092224 bytes on disk, 0 artifact(s) minted, 3 unclustered member row(s), 0 row(s) refused
  view world: 150000 row(s)
```

It takes a second or two. The build reads the sample's Parquet files, works out where each place sits on a world map, and writes the result to `bundle`. From now on the server reads only the bundle. `150000 items` is one per place.

Start the server. It runs until you stop it, so give it a terminal of its own.

```bash
mosaica serve
```

```
...
2026-10-25T14:08:28.958802Z  WARN mosaica_server: serve.open is set: this server is open to this machine, and every request, with or without a credential, sees and may change everything
{"event":"listening","viewer":"127.0.0.1:9141","session":"127.0.0.1:9142","control":"127.0.0.1:9143"}
```

The `WARN` line is the server confirming `open = true`, and the line of JSON says it is ready. Besides the viewer address, that line lists the session and control addresses, which a server with users relies on and this tutorial doesn't need.

## Make a web project

Open a second terminal. We'll use Vite, a build tool with a development server, and its plain TypeScript template. `--no-interactive` stops it asking whether to install and start the template's demo straight away. If npm hasn't fetched Vite's project creator before, it asks `Ok to proceed? (y)` first. Press Enter.

```bash
cd ~/places
npm create vite@latest web -- --template vanilla-ts --no-interactive
cd web
npm install @mosaicajs/components@next
```

`@mosaicajs/components` holds Mosaica's elements for the browser. `@next` asks npm for the newest release candidate, since Mosaica has published no full release yet. npm also installs what they're built on: Lit, which the elements are written with, and deck.gl, which draws the map.

The template comes with a demo page and a counter that we won't need. Remove them:

```bash
rm -r src/counter.ts src/assets public/icons.svg
```

## Put the explorer on the page

`<mosaica-explorer>` is the whole map application in one element: the map, a panel of filters, a card for the place you click, and the controls for how points are drawn. Replace `index.html` with:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Places of the world</title>
  </head>
  <body>
    <mosaica-explorer viewer-url="http://localhost:9141"></mosaica-explorer>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

`viewer-url` is Mosaica's viewer address. Without it the explorer would look for Mosaica on the page's own address, which here is Vite's.

Replace the template's script, `~/places/web/src/main.ts`, with these two lines:

```ts
--8<-- "clients/ts/examples/getting-started/web/src/main.ts"
```

The first defines `<mosaica-explorer>` and the other Mosaica elements. The second keeps the template's stylesheet. The explorer is as tall as the element around it, so make the page fill the window by replacing `src/style.css` with:

```css
html, body {
  height: 100%;
  margin: 0;
}
```

Start the development server:

```bash
npm run dev
```

```
> web@0.0.0 dev
> vite


  VITE v8.3.5  ready in 191 ms

  ➜  Local:   http://localhost:5173/
  ➜  Network: use --host to expose
```

Open <http://localhost:5173/>. Every dot is a place: a town, a lake, a mountain, a farm, drawn over the outlines of the countries. The strip at the bottom right counts them. The panel on the left is headed GeoNames, the title the schema gives this map, and under the counts it has an "Add field" button and nothing else yet.

Vite copies the browser's console to the terminal, so you'll see `Lit is in dev mode` there each time the page loads. Lit says that about any page served by a development server, and it goes away in a production build.

The explorer asked Mosaica what the data holds, and the answer lists every field with its kind. The page itself says nothing about the data. Press "Add field" and choose Name. `name` is text that the schema marks for search, so its card is a search box. Type `San` into it. The search applies as you type. The map keeps the 1,310 places with the word San in their name.

Now add Feature class and Country the same way. They're categories, so their cards list the values with a count for each, and the counts are of the places that match. Mexico has the most places called San, then Italy, the United States and the Philippines.

Click "Mountain, hill, rock" on the Feature class card. The two filters apply together and 97 places remain. The Country card now puts North Korea first, with South Korea third, because san is the Korean word for mountain. "Clear all" at the top of the panel removes both filters.

## Shape the explorer

The explorer's attributes set how it opens. Add five to the element in `index.html`:

```html
<mosaica-explorer
  viewer-url="http://localhost:9141"
  pinned-filters="name feature_class country"
  colour-by="feature_class"
  title-field="name"
  card-fields="feature_class country population"
  open-url="https://www.geonames.org/{geonameid}"
></mosaica-explorer>
```

Save the file. Vite reloads the page.

`pinned-filters` names the fields whose cards are in the panel from the start, so nobody has to find them under "Add field". `colour-by` colours the places by a field. A category gets a colour per value, and a number gets a ramp from its lowest value to its highest. Settlements are blue, which is why most of Asia is. In the United States most dots are orange and red, because GeoNames lists many streams, farms and buildings there. Someone using the map can still change the colouring, with the Layers button on the map or the paint button on a field's card.

`title-field` says which field names a place. Hover over one and wait a moment. The label that follows the pointer becomes the place's name, such as Central Heights Shopping Mall. Until the name arrives from Mosaica, the label is a long number such as `#14815421504304620377`. That is the place's `mosaica_id`, the number Mosaica gives each place it serves, and without `title-field` the label would stay that way. Click a place, and the card that opens beside it is headed by the name too. `card-fields` chooses what the card lists under the name, here the place's kind, its country and its population.

`open-url` gives the card an Open button, which takes the reader to the place's own page on GeoNames in a new tab. Mosaica fills `{geonameid}` with the place's `geonameid` field. Without `open-url` the card has no Open, because only your application knows where an item should open.

The [components reference](../reference/components/mosaica-explorer.md) lists the explorer's other attributes, from the most points it draws to how density is shaded.

## Fit it into a page

The explorer is an element like any other, so it goes wherever your layout puts it. Give the page a header and a column of its own beside the map. Replace `index.html` with this. The explorer keeps the attributes from the last step.

```html
--8<-- "clients/ts/examples/getting-started/web/index.html"
```

And replace `src/style.css` with a grid:

```css
--8<-- "clients/ts/examples/getting-started/web/src/style.css"
```

The explorer asks two things of the space it's given. Its height is 100% of its container's, so the container needs a height of its own. Here the grid gives the body the window's height, the header what it needs, and `main` the rest. `min-height: 0` lets `main` shrink to that row instead of growing to fit what's inside it. The explorer takes its width from its container as well, never from what's inside it, because it lays out its panels by how wide it is. In a grid that is its column, here everything the 260-pixel side column leaves. In a flex row, give it `flex: 1` or a width. Below 1000 pixels wide it puts the filters in a card over the map, and at 720 or narrower it moves them behind tabs.

Save, and the map sits between the header and the column, as wide as the window allows.

## What you built

A page of your own with 150,000 places on a map, which you can search, filter and colour. Nearly all of it came from one element. The explorer learnt the fields from Mosaica and built a card for each, and your attributes chose which cards it starts with, how it colours the places and what it says about each one.

The finished project is in the clone, in `clients/ts/examples/getting-started/`: the same two directories, `mosaica` and `web`, with the same files.

## Next

- [Your own interface around the map](typescript-own-ui.md) leaves the explorer behind: a bare map, with the page's own controls, results list and panels driving it through the store.
- The [components reference](../reference/components.md) lists every element, attribute and event, including the map and panels you can use without the explorer.
- [Maps in the browser](../system/browser-maps.md) explains what the elements are built on, for when you want your own panels or your own deck.gl scene.
- [Call Mosaica from a backend](../guides/backend.md) sets up users and tokens, for a server that more than one person uses.
