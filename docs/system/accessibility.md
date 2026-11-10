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
| Anywhere, while the hover tooltip shows | Escape | Dismisses the tooltip. With the focus in the map or in the explorer the press does nothing else: it clears no selection and closes no card, sheet, popover or menu. With the focus elsewhere, the page's own Escape still acts. |
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
| The range box | Escape | Closes the box and returns focus to the plot or the Set range button, whichever opened it. Tab past either end of the box closes it too. |
| A row of the hierarchy, with the focus or the pointer in it | Escape | Puts the row's buttons away and shows its counts again. Focus on one of the buttons moves to the row's name. |
| The colour editor | Escape | Closes the dialog. |
| The colour picker | Escape | Closes the picker and returns focus to the swatch that opened it. Tab past its last control, or Shift and Tab before its first, does the same. |
| The picker's colour square | Left, Right | Change the saturation by 1%, or by 10% with Shift. |
| | Up, Down | Change the brightness by 1%, or by 10% with Shift. |
| The picker's hue bar | Arrow keys, Home, End | Change the hue by 1°, or by 10° with Shift. Home and End go to its ends. |

A histogram's range does not need the plot's keys or a drag. The range box floats beside the plot and holds a From and a To field, which take a typed number or date. A press on one bin opens it, and so does Set range, a button after the plot in the Tab order that is drawn only while the keyboard's focus is on it. A number is typed in digits, and a comma, a space or a narrow no-break space is taken only where it groups thousands: `1,250` and `1 250` are both 1250, and `1,5` is refused with words that say what the comma is for.

A row of a category or cluster field card shows its Highlight and Filter buttons in place of its counts while the pointer is over the row or the keyboard's focus is in it, and a row of the hierarchy does the same with its Highlight, Filter and Fit buttons. The buttons are Tab stops while they are not drawn, so Tab reaches and shows them, and a screen reader has the counts at all times. On a screen with no pointer that hovers, such as a touch screen, the buttons are always drawn beside the counts, which is how Highlight is reached there.

When an action removes the control that has focus, focus moves to a control beside it. Removing a filter's chip, for example, moves focus to the × of the chip after it, or of the one before it, and to Add field when no chip is left. A panel that shows chips alone has no Add field, so it shows the words No filters, which take the focus.

In the explorer's narrow form, an open sheet that reaches up over the map's tools makes the map behind it inert until the sheet closes, so that Tab and a press go to nothing the sheet covers.

## What a screen reader is told

A change that happens without the focus moving is announced through a status region, an element with `role="status"` that is in the page before its words change.

### Which element speaks

The map, the status strip and the selection card read the same store, so each can say that the view was refused or how many items a region holds. Each of the three takes a boolean `quiet` attribute, which leaves everything the element shows on screen and puts none of it in a status region.

| Element | Announces on its own | With `quiet` |
|---|---|---|
| `<mosaica-map>` | The state it shows over its canvas, the selected region's count, the basemap's note and a basemap it refuses. | The basemap's note and a basemap it refuses, which no other element knows. |
| `<mosaica-status>` | Each state it names in words, a query's result, and Up to date after a state it announced. | Nothing. |
| `<mosaica-selection>` | Counting, then the matched and visible counts, or that the selection was refused. | Nothing. |

The explorer sets `quiet` on its own map and selection card, so its status strip alone speaks and each change is announced once. If you put something other than a `<mosaica-status>` in the explorer's `status` slot, its map and selection card announce for themselves. In a layout of your own, set `quiet` on all but one of the elements that would say the same thing: with a status strip beside a map, on the map and on a selection card.

### What is announced

In an explorer, these changes are announced:

- **The state of the view**, by the status strip. It announces each state it names in words: Connecting, Running query, Nothing visible to you, View refused with the server's reason, Server did not answer, Server busy, Reconnecting, Server unreachable, Session expired and Data updated. After any of them it announces Up to date when the view recovers.
- **A query's result**, by the status strip. After a change of filters, highlight or selection, it announces the counts once they have loaded, such as "3,923 of 50,000 match", or Up to date where there are no counts to say. While a region is selected the counts are the region's, so drawing a region announces how many items match inside it. Nothing in view is announced as the answer to such a change and not where a pan went.
- **A card opening**, by the explorer, which names the card that opens beside a picked point. In the narrow form the card opens in the Item sheet.
- **The basemap**, by the map: its note saying why no basemap is drawn under a view, and a basemap the map refuses.

Every element announces what only it knows, inside an explorer or outside one:

- **What a search box lists.** The value and cluster search boxes each say how many rows are listed, that nothing matched, or, once an answer has been awaited for a quarter of a second, that they are loading. Add field's list says how many fields are listed, or that none has that name.
- **A range being chosen.** While the arrow keys choose a range on a histogram, the card announces the range and its count. The range box says what to type when a field holds no number or date.
- **A copy.** The item card's Copy id tells a screen reader whether the id was copied, in words that are not drawn.
- **A card's own state.** The item card, the artifact card, the hierarchy and the colour editor each say when they have nothing to show, are loading or were refused.

A map on its own announces the states it shows over its canvas, and how many items match inside a selected region, once, when the query that counts it has finished loading. A selection card on its own announces Counting, then the counts once, or a refusal.

Panning and zooming announce nothing. While the camera moves, the status strip goes between Up to date and Updating with each request, so those two states are text a screen reader can read on the strip and are not announced. The strip's count cells change with the camera too and sit outside the status region, and a pan that ends where nothing is in view shows Nothing in view without announcing it.

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

The status strip's counts that wait for an answer, or that stand from before a refusal, are drawn in the faint ink, `--mosaica-ink-3`, which keeps 4.5:1 on the surface. The state cell beside them says why they wait, in words or by the shape of its dot.

A state is not shown by colour alone. A pressed segment and a chosen row carry a bar, the entry chosen in a menu carries a tick, a highlight's chip carries a pen with the word as hidden text, a field card's row carries a tick where its value is filtered to and a pen where it is highlighted, and the dot for a view that is updating is a ring where the dot for one that is up to date is solid. A value the field's filter leaves out is named in the faint ink, which keeps 4.5:1, with its swatch dimmed.

Under forced colours, as in Windows' contrast themes, a pressed, checked or chosen control takes the system's highlight colours and the switch, the dots and the spinner are drawn in system colours. The focus ring on such a control is drawn inside its edge in the highlight's text colour, where a ring in the highlight colour would not show. Colour that is data keeps its own value: a swatch, a bar, a ramp and the colour picker.

With `ground` unset the map follows the page's colour scheme, and draws for the other background at once when the page's or the system's scheme changes.

A title or a name too long for its line is cut with an ellipsis in the wide and compact forms and keeps its whole text in the page, so a screen reader reads all of it, and a pointer's hover shows it in a `title`. The token `--mosaica-long-text` chooses this: `nowrap`, the default, cuts a long name, and `normal` wraps it. The explorer's narrow form sets it to `normal`, and a host can set it on any element or ancestor.

Not built yet: a way for a sighted keyboard user to read a name that is cut. A viewer who does not use a pointer, or who has widened the text's spacing so that more is cut, cannot read the cut part unless the host sets `--mosaica-long-text: normal`. A row of the hierarchy has its name in its accessible name and not in a `title`, so a pointer's hover does not show a cut name there either.

Not built yet: two notes for a sighted keyboard user. Which layers are drawn with a layer, and why Size by offers nothing, are each a `title` for a pointer and a description for a screen reader, and are not drawn.

Where the viewer's system asks for reduced motion, the components run no transition or animation. The components do not animate the map's camera under either setting: a key press, Fit to matches or a switch of view puts it in its new place at once.

Not built yet: the category palettes do not reach 3:1 against a light map. Each palette colour has an opacity of 200 in 255, and the map multiplies it by the points' opacity, which is 0.8 while ten thousand points or fewer are on screen and falls to 0.34 at a million, where the points read as a density. The figures below are computed from the palettes' colours and the default backgrounds.

| Palette | Light background, `point-opacity="1"` | Dark background, `point-opacity="1"` | Dark background, opacity 0.8 |
|---|---|---|---|
| Tableau 10 | 1.4:1 to 2.9:1 | 3:1 or more, except blue at 2.98:1 | four of ten under 3:1 |
| Okabe-Ito | 1.2:1 to 3.3:1, and black at 11.3:1 | 3:1 or more, except blue at 2.7:1 and black at 1.1:1 | four of eight under 3:1 |
| Set 2 | 1.2:1 to 2.0:1 | 4.8:1 or more | 3.5:1 or more |
| Dark 2 | 1.7:1 to 3.4:1 | 3:1 or more, except grey at 2.5:1 | six of eight under 3:1 |

At opacity 0.8 every figure on the light background is lower than the one shown. A host that needs each value's colour at 3:1 draws the map on a dark background, with a dark `color-scheme` or with `--mosaica-map-bg` and `ground="dark"` on `<mosaica-map>`, and uses Set 2 or sets its own colours with `valueColours`. Points with no colouring are drawn in one colour, chosen for the background: the theme's secondary ink, a grey, on a light background, and a pale blue on a dark one. At `point-opacity="1"` the grey is at 3.8:1 against the default light background and the blue at 6.2:1 against the dark one, and at opacity 0.8 they are at 2.8:1 and 4.4:1, so on a light map a host that needs 3:1 sets `point-opacity="1"`, or draws the map dark.

## Small windows and zoom

The explorer lays itself out by the width of its container, so it has the same three forms in a small window and in a large one that is zoomed in.

| Container width | Form |
|---|---|
| 1000 px or more | The filter panel in a sidebar, or in a card over the map, as `layout` says. |
| Between 720 and 1000 px | The filter panel in a card over the map, and a shorter status strip. |
| 720 px or less | A title bar, the map, the status strip and a bar of three buttons. Each button opens a sheet over the map: Fields, Layers or Item. A picked item opens the Item sheet. A sheet's content scrolls above its footer, and the strip grows onto further lines where its cells do not fit one. |

The full-width strip and the bar of buttons, the parts `strip-row` and `tabs`, are in the page only in the narrow form, so the wider forms hold one status strip.

The browser suite checks the narrow form at 640 by 400 px and at 320 by 480 px, which are a 1280 by 800 px window at 200% and at 400% zoom. At both sizes the status strip and the bar of buttons stay inside the explorer with nothing drawn over them, and the map keeps a height of 100 px or more. In a frame shorter than the map's floor and the bars together, the explorer scrolls. At 320 px wide no text is cut off and nothing scrolls sideways, with a field card open and with the Layers sheet open. In the compact form at 900 px wide the only text cut is a long name's, with an ellipsis. Neither changes when line, letter and word spacing are widened to 1.5, 0.12em and 0.16em.

In every state the suite builds, a pointer target is at least 24 by 24 CSS pixels, or far enough from the next one that a 24-pixel circle centred on it reaches no other target.

Not built yet: pinned cards in the narrow form. The Item sheet shows the picked item alone, and it has no Pin button.

## The notebook map

`Map` in the Python client draws an explorer in a notebook cell. Its `label` argument names the map as `aria-label` does in a page, which tells two maps in one notebook apart.

When the page refuses a value the kernel set for `filters`, `colour_by` or the basemap, the refusal is written in a status region above the map as well as sent to the kernel. Each trait has a line of its own there, so a screen reader reads the line that changed and not the others.

A notebook runs its own shortcuts for a key pressed outside a text field, and the map's fields are inside the explorer's shadow tree, where the notebook does not see them. The map's element therefore carries `data-lm-suppress-shortcuts`, which JupyterLab's command registry honours, and stops each key press from reaching the notebook's listeners. Tab passes through, so focus can leave the map, and so does an Escape that nothing in the map used. This behaviour was checked against the source of Lumino, the library JupyterLab's shortcuts are built on, and by unit tests of the element. It has not been tried in a running JupyterLab, Notebook 7 or VS Code.

## Not built yet

- **Reaching the points from the keyboard or a screen reader.** Picking a point, opening a cluster, selecting a region without a drag, zoom and pan buttons, and the hover tooltip from the keyboard are not built ([#241](https://github.com/mosaicadb/mosaica/issues/241)). The Box select and Lasso select tools can be chosen from the keyboard, and the shape still needs a pointer. Without a pointer a viewer can filter and search, read every count and breakdown, and move the map. They can open an item only where the page offers a control that calls `Store.pick` with the item's `mosaica_id`, such as a list of results the page draws itself.
- **Names and labels drawn on the map.** Cluster names are drawn on the canvas, so a screen reader does not read them there. A cluster field's card and the hierarchy list the same names as text.
- **Translated words.** The components' own words are English, and their numbers and dates are formatted for `en-GB`, whatever the page's `lang`.
- **A loading state on the item card.** The card shows the previous item until the next record arrives, so for a moment after a pick it describes the item picked before.

## What has been checked

The components' browser suite builds the explorer in each of its main states in headless Chromium, and the hierarchy, the selection card, the pickers, a filter panel that shows chips alone and the colour editor on their own. The states are the explorer at rest, with field cards open, with filters applied, with the Layers popover and its menus open, with an item card, an artifact card and a selected region, a range box holding an entry it refuses, each sheet of the narrow form, and each state the status strip and the map's overlay can show. In each state it checks:

- every rule of axe-core 4.14 tagged WCAG 2.0, 2.1 or 2.2 at level A or AA;
- the contrast of text in the light and the dark scheme, including text drawn over the map, which is measured against the map's background;
- that no text is cut off and nothing scrolls sideways, in the compact form, down to 320 px wide and with text spacing widened. In a window wider than 720 px a name cut with an ellipsis is not counted, where it does not wrap by the long-text rule and its whole text is its own or its control's `title` or accessible name. At 720 px and under nothing may be cut;
- that no pointer target under 24 by 24 px is too close to another;
- that every Tab stop is drawn, inside the window, not covered at its centre and shows a focus indicator.

With a region selected and counted, the suite also checks that one status region in the explorer holds the count. It walks the explorer's Tab order and the Layers popover's keys in Chromium.

These have not been checked: any screen reader, any browser other than Chromium, a touch screen reader's gestures, and the notebook map in a running notebook.
