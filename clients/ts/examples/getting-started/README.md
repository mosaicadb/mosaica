# Getting started: a map in your web page

<!-- --8<-- [start:intro] -->
This tutorial is for a developer deciding whether to put Mosaica in a web application. We'll start
a Mosaica server over 150,000 places from GeoNames and show them on a world map in a web page,
with a search box and filters. Then we'll change the page three times, a few lines each: colour the
places by what kind of place they are, title each one by its name, and make a line of the page's
own text answer a click on the map.

By the end you'll have seen that one element, `<mosaica-explorer>`, draws the map and builds its
filters from what the server says the data holds. You change it with attributes, and it tells your
page what happened with events. Once Mosaica is built, the tutorial takes about ten minutes.

You'll need:

- Node.js 22.15 or later, with npm.
- A `mosaica` binary. Build one with `cargo build --release -p mosaica` at the root of the
  repository, which needs Rust and a C compiler, or set `MOSAICA_BIN` to the path of one you
  already have.
<!-- --8<-- [end:intro] -->

<!-- --8<-- [start:outline] -->
## What we'll do

1. Start Mosaica and the page with one command, and look around the map.
2. Read the page: one element and two lines of script.
3. Colour the places by feature class.
4. Title each place by its name.
5. Show the place someone clicks in a line of the page's own.
<!-- --8<-- [end:outline] -->

<!-- --8<-- [start:start] -->
## 1. Start everything

Install the packages once, from `clients/ts` in your clone of the repository, then start the
example from its own directory.

```bash
git clone https://github.com/mosaicadb/mosaica
cd mosaica/clients/ts
npm ci
cd examples/getting-started
npm start
```

```
> start
> vite

  building the GeoNames bundle, which takes a few seconds
  Mosaica is serving 150,000 GeoNames places (log: /home/you/mosaica/mosaica-demo/getting-started/mosaica.log)

  VITE v8.2.0  ready in 2781 ms

  ➜  Local:   http://localhost:5310/
  ➜  Network: use --host to expose
```

`npm start` runs two servers. The first is Mosaica. On the first run it reads the GeoNames files in
`examples/data/geonames/` and builds them into a bundle, Mosaica's on-disc form of the data, under
`mosaica-demo/getting-started/` at the root of the repository. Later runs reuse the bundle. The
second is Vite's development server, which serves the page on port 5310 and passes the page's
requests for `/mosaica` on to Mosaica. Ctrl-C stops both.

Open <http://localhost:5310/>.

Every dot is a place: a town, a lake, a mountain, a farm. The panel on the left counts them, and
each card counts them by one field. Type `San` into the Name box. The search applies as you type.
The map keeps the 1,310 places with the word San in their name, and every card counts again: Mexico
has the most, then Italy, the United States and the Philippines.

Now hover over "Mountain, hill, rock" on the Feature class card and press the funnel button that
appears beside it, Filter. The two filters apply together and 97 places remain. The Country card now
puts North Korea first, with South Korea third, because san is the Korean word for mountain.
"Clear all" at the top of the panel removes both filters.

If the command stops with `Error: no mosaica binary`, it found no binary to run. Build one with
`cargo build --release -p mosaica` at the root of the repository, or set `MOSAICA_BIN` to one.
<!-- --8<-- [end:start] -->

<!-- --8<-- [start:page] -->
## 2. The page

Open `index.html`. Apart from a little CSS to make the map fill the window, the page is one element:

```html
<mosaica-explorer
  viewer-url="/mosaica"
  dataset-title="Places of the world"
  pinned-filters="name feature_class country"
></mosaica-explorer>
```

`viewer-url` is the address of the Mosaica server. Here it is a path on the page's own origin,
which the development server passes on to Mosaica. `dataset-title` heads the panel.
`pinned-filters` names the fields that have a card in the panel from the start. The others wait
under "Add field".

The page does not say what fields exist or what kind each one is. The explorer asks the server.
`name` is text that Mosaica indexes for search, so its card is a search box. `feature_class` and
`country` are categories, so their cards list the values with a count for each.

`src/main.ts` holds the rest:

```ts
import '@mosaicajs/components';

const explorer = document.querySelector('mosaica-explorer')!;

// Development only: the dev server hands out viewer tokens. Tutorial 3 shows how a real app gets them.
explorer.authorise = () => fetch('/dev/token').then((r) => r.json());
```

The import defines `<mosaica-explorer>` and the other Mosaica elements. The last line gives the
explorer a way to get a viewer token, which Mosaica needs with every request to know who is
looking. The development server makes one up that sees every place. A real application gets its
tokens from its own server, for the person signed in, and the third tutorial shows how.
<!-- --8<-- [end:page] -->

<!-- --8<-- [start:colour] -->
## 3. Colour the places by feature class

Every place in GeoNames has a feature class: settlement, water, mountain, building and five more.
Add a `colour-by` attribute to the element in `index.html`:

```html
<mosaica-explorer
  viewer-url="/mosaica"
  dataset-title="Places of the world"
  pinned-filters="name feature_class country"
  colour-by="feature_class"
></mosaica-explorer>
```

Save the file. Vite reloads the page.

The Feature class card now shows which colour is which. Settlements are blue, and most of Asia is
blue. In the United States most dots are orange and red, because GeoNames lists many streams, farms
and buildings there. `colour-by` takes the name of a field. A category gets a colour per value, and a
number gets a colour ramp from its lowest value to its highest. The attribute sets the colouring the
page opens with. A viewer can change it with the Layers button on the map, or with the paint button,
a drop, on a field's card.
<!-- --8<-- [end:colour] -->

<!-- --8<-- [start:title] -->
## 4. Title each place by its name

Hover over a place. The label that follows the pointer is a long number such as
`#4040127343466120394`. That is the place's `mosaica_id`, the number Mosaica gives each item it
serves, and the explorer shows it because it does not know which field names a place. Add
`title-field`:

```html
<mosaica-explorer
  viewer-url="/mosaica"
  dataset-title="Places of the world"
  pinned-filters="name feature_class country"
  colour-by="feature_class"
  title-field="name"
></mosaica-explorer>
```

Save, and hover again. The label is now the place's name. Click a place, and the card that opens
beside it is headed by the name too.
<!-- --8<-- [end:title] -->

<!-- --8<-- [start:click] -->
## 5. Show the clicked place in the page

The card is the explorer's. Your page can also act on a click itself. Add a line of text under
the element in `index.html`:

```html
<p id="picked">Click a place on the map.</p>
```

Then add this to the end of `src/main.ts`:

```ts
const picked = document.querySelector('#picked')!;
explorer.addEventListener('mosaica-pick', (event) => {
  const record = event.detail.record;
  if (record) picked.textContent = `Last clicked: ${record.fields.name} (${record.fields.feature_class}, ${record.fields.country})`;
});
```

Save, and click a place. The line under the map names it, for example
`Last clicked: Huangjiatan (settlement, CN)`.

The explorer fires `mosaica-pick` twice for each click. The first time, at the click, the event
carries only the place's `mosaica_id`. The second time it also carries the place's record, once the
server has sent it. The handler waits for the record. `record.fields` holds the place's fields by
name. A category field holds its value's key, so the country is `CN` rather than China.

The event bubbles up through the page, so any element above the explorer can listen for it.
<!-- --8<-- [end:click] -->

<!-- --8<-- [start:end] -->
## What you built

A web page that shows 150,000 places on a map, with search and filters, from one element and a few
attributes. The explorer learnt the fields from the server and built a card for each kind. You
chose how the map is coloured and how places are titled with an attribute each, and your own page
answered a click through an event.

The [components reference](../../../../docs/reference/components.md) lists every attribute, event
and part of `<mosaica-explorer>` and the smaller elements it is made of.
<!-- --8<-- [end:end] -->
