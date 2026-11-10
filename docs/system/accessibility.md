# Accessibility

The `mosaica-*` components are the part of Mosaica a viewer touches, so this page is for someone embedding them who needs to know what a viewer can do without a pointer, what a screen reader is told, and what the page around the components has to supply. The map's points themselves cannot yet be reached from the keyboard or by a screen reader, and [Not built yet](#not-built-yet) says what a viewer can do instead.

Mosaica makes no claim of conformance to the Web Content Accessibility Guidelines (WCAG). [What has been checked](#what-has-been-checked) lists the checks that run and the ones that have not been made.

## What the page supplies

The components are elements inside your page, and some of what assistive technology needs belongs to the page:

- **A language.** Set `lang` on the page's `<html>` element. The components' own words are English.
- **A title, landmarks and headings.** Give the page a `<title>`, put the components inside a landmark such as `<main>`, and give the page an `<h1>`. A panel's or card's own title is an `<h2>` and a titled part inside it is an `<h3>`, so they sit under a page heading of yours.
- **A name for each map.** A map's canvas is one Tab stop, and a screen reader reads its name when focus reaches it. Without a name from you the canvas is called "Map", so a page with two maps needs to name them apart.

| Where the map is | How to name it |
|---|---|
| `<mosaica-map>` | the `aria-label` attribute |
| `<mosaica-explorer>` | the `aria-label` attribute, which the explorer passes to its map |
| React | the `aria-label` prop of `MosaicaMap` or `MosaicaExplorer` |
| A Python notebook | `label=` on `Map`, `Database.map`, `Viewer.map` or `Selection.map` |

Whatever its name, the canvas's description lists its keys, so that someone using a screen reader hears how to move the map.

## Keyboard

Most controls are the browser's own buttons, links, checkboxes, selects, sliders and text fields, so Tab reaches each one and Enter, Space and the arrow keys work on it as they do in any page. The map's canvas, a histogram's plot and the colour picker's square and hue bar are Tab stops with keys of their own. In the explorer Tab goes through the filter panel, the map's tools and the Layers button, the map, the selected region's Clear button, the selection card, the status strip and then the card beside a picked point.

The table lists those keys and the others the components add.

| Where the focus is | Key | What it does |
|---|---|---|
| The map | Arrow keys | Pan by 64 pixels. |
| | `+` or `=` | Zoom in one level. |
| | `-` or `_` | Zoom out one level. |
| | Escape | Cancels a shape being drawn. With no shape it clears the selected region, and with no region it drops the picked item. With none of the three the map leaves the key to the page, so a dialog that holds the map still closes. |
| Anywhere, while the hover tooltip shows | Escape | Dismisses the tooltip and does nothing else on that press. |
| A card beside a point, in the explorer | Escape | Closes the card unless it is pinned, and returns focus to the map. |
| The Layers popover | Escape | Closes the popover and returns focus to the Layers button. With a menu or the list of density colours open, it closes that alone. |
| The Size by, Colour by, Palette and Ramp buttons | Down or Up | Opens the button's menu. |
| An open menu | Down, Up, Home, End | Move through its entries. Down from the last entry goes to the first. |
| | Escape or Tab | Closes the menu and returns focus to its button. |
| A group of choices: Density, a Scale, the list of density colours | Arrow keys, Home, End | Move the choice, as in a group of radio buttons. |
| The bar of buttons, in the explorer's narrow form | Left, Right, Home, End | Move between the buttons. |
| An open sheet, in the narrow form | Escape | Closes the sheet and returns focus to its button. |
| Add field's search box | Enter | Adds the first field listed that the panel does not show. |
| | Down | Moves into the list of fields. |
| Add field's list | Down, Up, Home, End | Move through the fields. Up from the first returns to the search box. |
| | Escape | Closes the list and returns focus to Add field. |
| A value or cluster search box | Down, Up | Move through the suggestions. |
| | Home, End | Go to the first or last suggestion, once an arrow key has reached one. |
| | Enter | Chooses the suggestion the arrow keys reached, or takes it off. Before they reach one it does nothing. |
| | Escape | Clears the text of a value box. Closes the list of a cluster box. |
| A keyword box's operator button | Down or Up | Opens the menu of operators, which takes Down, Up, Home, End and Escape as the menus above do. |
| A histogram's plot, on a field card | Left, Right | Choose a range of one bin and move it. |
| | Shift with Left or Right | Widen or narrow the range. |
| | Enter | Opens the range box on the range chosen, or on the range the field holds. |
| | Escape | Drops the range. |
| The range box | Escape | Closes the box and returns focus to the plot or the Set range button, whichever opened it. |
| The colour editor | Escape | Closes the dialog. |
| The colour picker | Escape | Closes the picker and returns focus to the swatch that opened it. Tab past its last control, or Shift and Tab before its first, does the same. |
| The picker's colour square | Left, Right | Change the saturation by 1%, or by 10% with Shift. |
| | Up, Down | Change the brightness by 1%, or by 10% with Shift. |
| The picker's hue bar | Arrow keys, Home, End | Change the hue by 1°, or by 10° with Shift. Home and End go to its ends. |

A histogram's range does not need the plot's keys. The Set range button under the plot opens a box with a From and a To field, which take a typed number or date.

The hierarchy has no keys of its own: each row's expander, name and buttons are Tab stops. When an action removes the control that has focus, focus moves to a control beside it. Removing a filter's chip, for example, moves focus to the × of the chip after it, or of the one before it, and to Add field when no chip is left.

## What a screen reader is told

A change that happens without the focus moving is announced through a status region, an element with `role="status"` that is in the page before its words change. These changes are announced:

- **The state of the view.** The status strip announces each state it names in words: Connecting, Running query, Nothing in view, Nothing visible to you, View refused with the server's reason, Server did not answer, Server busy, Reconnecting, Server unreachable, Session expired and Data updated. After any of them it announces Up to date when the view recovers. The map says the states that stop it drawing over its canvas and announces them too, so a map with no status strip still reports a refusal.
- **A query's result.** After a change of filters, highlight or selection, the status strip announces the counts once they have loaded, such as "3,923 of 50,000 match".
- **A selected region's count.** The map announces how many items match inside the region when the count arrives. The selection card announces Counting, then the matched and visible counts, or that the selection was refused.
- **A card opening.** The explorer announces the card that opens beside a picked point by its name. In the narrow form the card opens in the Item sheet, which takes the focus.
- **What a search box lists.** The value and cluster search boxes and Add field's list each say how many rows are listed, that they are loading, or that nothing matched.
- **A range being chosen.** While the arrow keys choose a range on a histogram, the card announces the range and its count. The range box says what to type when a field holds no number or date.
- **A copy.** The item card's Copy id says whether the id was copied.
- **A card's own state.** The item card, the artifact card, the hierarchy and the colour editor each say when they have nothing to show, are loading or were refused.

Panning and zooming announce nothing. While the camera moves, the status strip goes between Up to date and Updating with each request, so those two states are text a screen reader can read on the strip and are not announced. The strip's count cells change with the camera too and sit outside the status region.

Three things are given as text for a screen reader where a sighted viewer has a picture. A field card's histogram has a table of its bins with their counts. Each row of a field card gives its two counts with what each one counts. The density key is named with its range and scale.

## Colours and themes

The components take their colours from the `--mosaica-*` custom properties that [Theme tokens](../reference/components/tokens.md) lists, each with a light and a dark default chosen by the page's `color-scheme`. The defaults keep the contrast in the table below in both schemes, and a unit test computes each ratio from the defaults. Nothing checks the values a host sets, so a host that overrides a token keeps its pair above the ratio.

| What is drawn | Against | At least |
|---|---|---|
| Text in `--mosaica-ink`, `--mosaica-ink-2` and `--mosaica-ink-3` | `--mosaica-surface`, `--mosaica-surface-2` and `--mosaica-surface-3` | 4.5:1 |
| Text in `--mosaica-accent-ink` | `--mosaica-accent` | 4.5:1 |
| Text in `--mosaica-highlight` | `--mosaica-surface` and `--mosaica-highlight-soft` | 4.5:1 |
| Text in `--mosaica-refuse` | `--mosaica-surface` | 4.5:1 |
| The focus ring, `--mosaica-focus` | the three surfaces and `--mosaica-map-bg` | 3:1 |
| An input's border, a slider's track and an off switch's track, `--mosaica-line-control` | `--mosaica-surface` | 3:1 |
| A switch's knob, `--mosaica-surface` | its track, `--mosaica-line-control` when off and `--mosaica-accent` when on | 3:1 |
| The status dots, `--mosaica-ok`, `--mosaica-warn`, `--mosaica-refuse` and `--mosaica-ink-3` | the three surfaces | 3:1 |
| The bar on a pressed segment and a chosen row, `--mosaica-accent` | `--mosaica-surface-2` | 3:1 |

The unit test covers the rows for `--mosaica-ink`, the focus ring and everything under it. The browser suite reads the other text rows off the rendered elements with the default tokens.

Every control shows the same focus ring, 2 pixels wide in `--mosaica-focus`, and the map's canvas shows it inside its edge.

A state is not shown by colour alone. A pressed segment and a chosen row carry a bar, the entry chosen in a menu carries a tick, a highlight's chip carries the word, and the dot for a view that is updating is a ring where the dot for one that is up to date is solid.

Under forced colours, as in Windows' contrast themes, a pressed, checked or chosen control takes the system's highlight colours and the switch, the dots and the spinner are drawn in system colours. Colour that is data keeps its own value: a swatch, a bar, a ramp and the colour picker.

Where the viewer's system asks for reduced motion, the components run no transition or animation. The components do not animate the map's camera under either setting: a key press, Fit to matches or a switch of view puts it in its new place at once.

Not built yet: the category palettes do not reach 3:1 against a light map. Each palette colour has an opacity of 200 in 255, and the map multiplies it by the points' opacity, which is 0.8 while ten thousand points or fewer are on screen and falls to 0.34 at a million, where the points read as a density. The figures below are computed from the palettes' colours and the default backgrounds.

| Palette | Light background, `point-opacity="1"` | Dark background, `point-opacity="1"` | Dark background, opacity 0.8 |
|---|---|---|---|
| Tableau 10 | 1.4:1 to 2.9:1 | 3:1 or more, except blue at 2.98:1 | four of ten under 3:1 |
| Okabe-Ito | 1.2:1 to 3.3:1, and black at 11.3:1 | 3:1 or more, except blue at 2.7:1 and black at 1.1:1 | four of eight under 3:1 |
| Set 2 | 1.2:1 to 2.0:1 | 4.8:1 or more | 3.5:1 or more |
| Dark 2 | 1.7:1 to 3.4:1 | 3:1 or more, except grey at 2.5:1 | six of eight under 3:1 |

At opacity 0.8 every figure on the light background is lower than the one shown. A host that needs each value's colour at 3:1 draws the map on a dark background, with a dark `color-scheme` or with `--mosaica-map-bg` and `ground="dark"` on `<mosaica-map>`, and uses Set 2 or sets its own colours with `valueColours`. Points with no colouring are drawn in a single blue chosen for the background, which is at 3:1 or more against both default backgrounds at opacity 0.8.

## Small windows and zoom

The explorer lays itself out by the width of its container, so it has the same three forms in a small window and in a large one that is zoomed in.

| Container width | Form |
|---|---|
| 1000 px or more | The filter panel in a sidebar, or in a card over the map, as `layout` says. |
| Between 720 and 1000 px | The filter panel in a card over the map, and a shorter status strip. |
| 720 px or less | A title bar, the map, the status strip and a bar of three buttons. Each button opens a sheet over the map: Fields, Layers or Item. A picked item opens the Item sheet. |

The browser suite checks the narrow form at 640 by 400 px and at 320 by 480 px, which are a 1280 by 800 px window at 200% and at 400% zoom. At both sizes the status strip and the bar of buttons stay inside the explorer with nothing drawn over them, and the map keeps a height of 100 px or more. In a frame shorter than the map's floor and the bars together, the explorer scrolls. At 320 px wide no text is cut off at the side and nothing scrolls sideways, with a field card open and with the Layers sheet open. The same holds when line, letter and word spacing are widened to 1.5, 0.12em and 0.16em.

In every state the suite builds, a pointer target is at least 24 by 24 CSS pixels, or far enough from the next one that a 24-pixel circle centred on it reaches no other target.

Not built yet: pinned cards in the narrow form. The Item sheet shows the picked item alone, and it has no Pin button.

## The notebook map

`Map` in the Python client draws an explorer in a notebook cell. Its `label` argument names the map as `aria-label` does in a page, which tells two maps in one notebook apart.

A notebook runs its own shortcuts for a key pressed outside a text field, and the map's fields are inside the explorer's shadow tree, where the notebook does not see them. The map's element therefore carries `data-lm-suppress-shortcuts`, which JupyterLab's command registry honours, and stops each key press from reaching the notebook's listeners. Tab passes through, so focus can leave the map, and so does an Escape that nothing in the map used. This behaviour was checked against the source of Lumino, the library JupyterLab's shortcuts are built on, and by unit tests of the element. It has not been tried in a running JupyterLab, Notebook 7 or VS Code.

## Not built yet

- **Reaching the points from the keyboard or a screen reader.** Picking a point, opening a cluster, selecting a region without a drag, zoom and pan buttons, and the hover tooltip from the keyboard are not built ([#241](https://github.com/mosaicadb/mosaica/issues/241)). The Box select and Lasso select tools can be chosen from the keyboard, and the shape still needs a pointer. Without a pointer a viewer can filter and search, read every count and breakdown, and move the map. They can open an item only where the page offers a control that calls `Store.pick` with the item's `mosaica_id`, such as a list of results the page draws itself.
- **Names and labels drawn on the map.** Cluster names are drawn on the canvas, so a screen reader does not read them there. A cluster field's card and the hierarchy list the same names as text.
- **Translated words.** The components' own words are English, and their numbers and dates are formatted for `en-GB`, whatever the page's `lang`.
- **A loading state on the item card.** The card shows the previous item until the next record arrives, so for a moment after a pick it describes the item picked before.

## What has been checked

The components' browser suite builds the explorer in each of its main states in headless Chromium, and the hierarchy, the selection card, the pickers and the colour editor on their own. The states are the explorer at rest, with field cards open, with filters applied, with the Layers popover and its menus open, with an item card, an artifact card and a selected region, each sheet of the narrow form, and each state the status strip and the map's overlay can show. In each state it checks:

- every rule of axe-core 4.14 tagged WCAG 2.0, 2.1 or 2.2 at level A or AA;
- the contrast of text in the light and the dark scheme, including text drawn over the map, which is measured against the map's background;
- that no text is cut off and nothing scrolls sideways, down to 320 px wide and with text spacing widened;
- that no pointer target under 24 by 24 px is too close to another;
- that every Tab stop is drawn, inside the window, not covered at its centre and shows a focus indicator.

The checks find three faults that are not fixed, and the suite records each as an expected failure:

- While a view is loading, stale or refused, the status strip greys its counts to 40% opacity, which puts them between 1.9:1 and 3.4:1.
- In the narrow form the status strip is one line high, so a state with a reason, or counts with a highlight, lose the lines that wrap below it.
- In the Fields sheet the footer with Clear and Show stays at the sheet's bottom edge, and a control scrolled under it can hold the focus while the footer covers it.

The same suite walks the explorer's Tab order and the Layers popover's keys in Chromium.

These have not been checked: any screen reader, any browser other than Chromium, a touch screen reader's gestures, and the notebook map in a running notebook.
