# Your first map in a web page

This tutorial is for a developer who wants Mosaica's map inside their own web application, and wants to see it working before reading much else. We'll start a Mosaica server over 150,000 places from GeoNames, a free gazetteer of the world's place names. Then we'll make a new web project with Vite, put the `<mosaica-explorer>` element on its page, and end with the map sitting in an ordinary page layout of our own, with a search box and filters built from the data.

It takes about fifteen minutes. You need the `mosaica` program, Node.js 22.15 or later with npm, and a clone of the Mosaica repository, which holds the sample data:

```bash
git clone https://github.com/mosaicadb/mosaica ~/mosaica
```

The finished project is in the clone too, in `clients/ts/examples/getting-started/`, if you want to compare.

Here is what we'll do.

1. Start Mosaica over the sample data.
2. Make a web project, and connect its development server to Mosaica.
3. Put the explorer on the page, and search and filter the places.
4. Choose which filters show, how the places are coloured and what they're called.
5. Fit the explorer into a page with a header and a side column.

We'll keep everything in one directory, `~/places`, with Mosaica's files in `~/places/mosaica` and the web project in `~/places/web`.

!!! note "Not published yet"
    Neither the `mosaica` program nor the npm packages are published yet, so for now both come from the clone. Build and install `mosaica` with Rust, which also needs a C compiler:

    ```bash
    cd ~/mosaica
    cargo install --locked --path crates/mosaica-cli
    ```

    Then build the three packages this tutorial uses and pack each into a tarball, an npm package in a single file:

    ```bash
    cd ~/mosaica/clients/ts
    npm ci
    npm run build
    mkdir -p ~/mosaica-packs
    npm pack -w @mosaicajs/client -w @mosaicajs/deck -w @mosaicajs/components --pack-destination ~/mosaica-packs
    ```

    Where step 2 says `npm install @mosaicajs/components`, install the three tarballs together instead:

    ```bash
    npm install ~/mosaica-packs/mosaicajs-client-0.1.0.tgz ~/mosaica-packs/mosaicajs-deck-0.1.0.tgz ~/mosaica-packs/mosaicajs-components-0.1.0.tgz
    ```

    The npm registry holds an empty placeholder under each name, so `npm install @mosaicajs/components` succeeds today and installs no Mosaica code.

## Start Mosaica

Mosaica is a database built to send millions of points to a browser. Like most databases, it runs as a server, and your web page talks to it over HTTP.

Make a directory for the server's files.

```bash
mkdir -p ~/places/mosaica
cd ~/places/mosaica
```

Mosaica reads its settings from a file called `mosaica.toml`. Save this in the new directory:

```toml
--8<-- "clients/ts/examples/getting-started/mosaica/mosaica.toml"
```

`[bundle]` names three things the server keeps on disc. `path` is the bundle, Mosaica's own copy of the data, arranged so that any part of the map can be read quickly. `cache` holds work the server can reuse, and `wal` is its write-ahead log, which records changes to the data. `token_max_lifetime` is how long, in seconds, a browser's permission to read the map lasts before it has to ask again.

`[serve]` gives the three addresses the server listens on. The viewer address, 9141, serves the map. The session address, 9142, hands out that permission to read. The control address, 9143, takes changes to the data and to who may see what, and this tutorial doesn't use it. The operator credential is the server's master secret, kept in a file called `operator.secret`. `[catalogue]` is where Mosaica keeps its users and what each may see. We won't add any users here.

Now build the bundle. The repository's sample comes with a declaration, `schema.toml`, which tells Mosaica what each column of the data means: `name` is text to search, `feature_class` and `country` are categories, and so on. `--config` points the build at it.

```bash
mosaica build --config ~/mosaica/examples/data/geonames/schema.toml
```

```
...
built /home/you/places/mosaica/bundle (v00000): 150000 items, 245 terms, 150000 pairs, 18100841 bytes on disk, 0 artifact(s) minted, 3 unclustered member row(s), 0 row(s) refused
  view world: 150000 row(s)
```

It takes a second or two. The build reads the sample's Parquet files, works out where each place sits on a world map, and writes the result to `bundle`. From now on the server reads only the bundle. `150000 items` is one per place.

The server needs its secret before it starts. Make a random one that only you can read.

```bash
openssl rand -hex 16 > operator.secret
chmod 600 operator.secret
```

Start the server. It runs until you stop it, so give it a terminal of its own.

```bash
mosaica serve
```

```
...
{"event":"listening","viewer":"127.0.0.1:9141","session":"127.0.0.1:9142","control":"127.0.0.1:9143"}
```

The line of JSON is the one to wait for. It means all three addresses are open.

## Make a web project

Open a second terminal. We'll use Vite, a build tool with a development server, and its plain TypeScript template.

```bash
cd ~/places
npm create vite@latest web -- --template vanilla-ts
cd web
npm install @mosaicajs/components
```

The template comes with a demo page and a counter that we won't need. Remove them:

```bash
rm -r src/counter.ts src/assets public/icons.svg
```

`@mosaicajs/components` holds Mosaica's elements for the browser. npm also installs what they're built on: Lit, which the elements are written with, and deck.gl, which draws the map.

Before the page can show anything, it needs two things from Mosaica. The first is the data, from the viewer address. The page will ask for it at `/mosaica` on its own origin, and Vite's development server will pass those requests on. The second is a token. Mosaica answers a request only when it carries a token, a string that says which places the person asking may see. A real application gets one for whoever is signed in, from its own server, and the third tutorial shows how. For now the development server will make one that sees every place, by asking the session address with the operator secret.

Create `vite.config.ts` in the `web` directory:

```ts
--8<-- "clients/ts/examples/getting-started/web/vite.config.ts"
```

`server.proxy` passes every request under `/mosaica` on to the viewer address, without the `/mosaica` prefix. The plugin adds one route to the development server, `/dev/token`. It posts to the session address's `/session/authorise` with the operator secret, asks for a session that reads every place, and hands back Mosaica's answer: the token and when it expires.

The secret stays in the development server and never reaches the page. The plugin reads it from the `MOSAICA_CREDENTIAL` environment variable, which we'll set when we start Vite. The two addresses can be changed the same way, with `MOSAICA_VIEWER_URL` and `MOSAICA_SESSION_URL`, but the defaults match our `mosaica.toml`.

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
    <mosaica-explorer viewer-url="/mosaica"></mosaica-explorer>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

`viewer-url` is where the explorer finds Mosaica: the path our proxy passes on.

Replace `src/main.ts` with this:

```ts
--8<-- "clients/ts/examples/getting-started/web/src/main.ts"
```

The import defines `<mosaica-explorer>` and the other Mosaica elements. `authorise` is how the explorer gets its token. It calls the function when it starts, and again shortly before each token expires. The function returns the token, and its expiry in seconds since 1970. Here it asks our development server's `/dev/token`.

The explorer is as tall as the element around it. So that it fills the window, replace `src/style.css` with:

```css
html, body {
  height: 100%;
  margin: 0;
}
```

Start the development server, with the operator secret in `MOSAICA_CREDENTIAL`:

```bash
export MOSAICA_CREDENTIAL=$(cat ~/places/mosaica/operator.secret)
npm run dev
```

```
  VITE v8.3.5  ready in 214 ms

  ➜  Local:   http://localhost:5173/
  ➜  Network: use --host to expose
```

Open <http://localhost:5173/>. Every dot is a place: a town, a lake, a mountain, a farm. The strip at the bottom right counts them, and the panel on the left is empty but for an "Add field" button.

The explorer asked Mosaica what the data holds, and the answer lists every field with its kind. The page itself says nothing about the data. Press "Add field" and choose Name. `name` is text that the declaration marks for search, so its card is a search box. Type `San` into it. The search applies as you type. The map keeps the 1,310 places with the word San in their name.

Now add Feature class and Country the same way. They're categories, so their cards list the values with a count for each, and the counts are of the places that match. Mexico has the most places called San, then Italy, the United States and the Philippines.

Hover over "Mountain, hill, rock" on the Feature class card and press the funnel button that appears in place of its count. The two filters apply together and 97 places remain. The Country card now puts North Korea first, with South Korea third, because san is the Korean word for mountain. "Clear all" at the top of the panel removes both filters.

## Shape the explorer

The explorer's attributes set how it opens. Add three to the element in `index.html`:

```html
<mosaica-explorer
  viewer-url="/mosaica"
  pinned-filters="name feature_class country"
  colour-by="feature_class"
  title-field="name"
></mosaica-explorer>
```

Save the file. Vite reloads the page.

`pinned-filters` names the fields whose cards are in the panel from the start, so nobody has to find them under "Add field". `colour-by` colours the places by a field. A category gets a colour per value, and a number gets a ramp from its lowest value to its highest. Settlements are blue, and most of Asia is blue. In the United States most dots are orange and red, because GeoNames lists many streams, farms and buildings there. Someone using the map can still change the colouring, with the Layers button on the map or the paint button on a field's card.

`title-field` says which field names a place. Hover over one. The label that follows the pointer is the place's name. Without `title-field`, it was a long number such as `#4040127343466120394`, the `mosaica_id` that Mosaica gives each place it serves. Click a place, and the card that opens beside it is headed by the name too.

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

The explorer asks two things of the space it's given. Its height is 100% of its container's, so the container needs a height of its own. Here the grid gives the body the window's height, the header what it needs, and `main` the rest. `min-height: 0` lets `main` shrink to that row instead of growing to fit what's inside it. The explorer takes its width from its container as well, never from what's inside it, because it lays out its panels by how wide it is. In a grid that is its column, here everything the 260-pixel side column leaves. In a flex row, give it `flex: 1` or a width. Below 1000 pixels wide it puts the filters in a card over the map, and below 720 it moves them behind tabs.

Save, and the map sits between the header and the column, as wide as the window allows.

## What you built

A page of your own with 150,000 places on a map, which you can search, filter and colour. Most of it came from one element. The explorer learnt the fields from Mosaica and built a card for each, and your attributes chose which cards it starts with, how it colours the places and what it calls them. The page reaches Mosaica through a proxy in its development server, which also gives it a token that sees everything.

`clients/ts/examples/getting-started/` in the repository holds the same two directories, `mosaica` and `web`, with the same files. The [components reference](../reference/components.md) lists every element, attribute and event.

The next tutorial builds your own interface around the map: a panel that shows the place someone clicks, and controls of your own that filter and colour it.
