import '../../src/index.js';
import type {BrowseRow, ItemDetail, Meta} from '@mosaicajs/client';
import {aggregateEntry, answerAggregate, fakeStore, meta, settle, status} from '../fake-store.js';

// Built by `targets.browser.ts` with Vite from the packages' sources. A hierarchy with a filtered
// and highlighted row, another with ordinary names, a colour editor over a 22-colour palette with
// its picker open, and two item cards with long text, each 300 px wide or in its dialog;
// `window.result` holds what the test measures. `?font=serif`, `?font=monospace` and `?scale=1.15`
// draw the elements in another font or a larger one, since nothing measured here may depend on
// the machine's font, and `?long=normal` has long names wrap, as a host may ask.
type Box = {name: string; left: number; top: number; right: number; bottom: number};
const boxOf = (name: string, el: Element): Box => {
  const r = el.getBoundingClientRect();
  return {name, left: r.left, top: r.top, right: r.right, bottom: r.bottom};
};
/** The pairs of `boxes` that share any area. */
const overlaps = (boxes: Box[]): string[] =>
  boxes.flatMap((a, i) => boxes.slice(i + 1).flatMap((b) => (a.left < b.right - 0.5 && b.left < a.right - 0.5 && a.top < b.bottom - 0.5 && b.top < a.bottom - 0.5 ? [`${a.name} over ${b.name}`] : [])));
/**
 * The targets short of 24 px either way: those a press 11 px left, right, above or below the
 * middle of what they draw would miss, a pixel inside the square since boxes are drawn on whole
 * pixels. A target is an element, or an element and the glyph inside it that its hit area is
 * drawn around.
 */
const small = (root: ShadowRoot, targets: [name: string, target: Element, glyph?: Element][]): string[] =>
  targets.flatMap(([name, target, glyph = target]) => {
    target.scrollIntoView({block: 'center'});
    const r = glyph.getBoundingClientRect();
    const lands = (dx: number, dy: number) => {
      const at = root.elementFromPoint(r.left + r.width / 2 + dx, r.top + r.height / 2 + dy);
      return at !== null && target.contains(at);
    };
    return lands(-11, 0) && lands(11, 0) && lands(0, -11) && lands(0, 11) ? [] : [`${name}, drawn ${Math.round(r.width)} by ${Math.round(r.height)}`];
  });
const frames = (n: number) =>
  new Promise<void>((resolve) => {
    const step = () => (--n <= 0 ? resolve() : requestAnimationFrame(step));
    requestAnimationFrame(step);
  });
const settled = async () => {
  await settle(document.body);
  await frames(2);
  await settle(document.body);
};
const layer = (name: string): Meta['layers'][number] =>
  ({name, title: name, views: ['s0'], membership: 'enumerated', hierarchy: {kind: 'nested', pruneChildren: false}, levels: [], computedContent: ['centroid', 'box', 'hull'], shape: 'derived', suppliedContent: ['name'], depsOn: [], version: 1}) as unknown as Meta['layers'][number];
const LONG = 'Neoplasms of the connective and soft tissue, including those of uncertain behaviour and unspecified site';
const browseRow = (id: bigint, name: string, childCount = 0): BrowseRow => ({mosaicaId: id, key: `k${id}`, name, maskedCount: 270_000n, matchedCount: 12_345n, rung: 0, parentIds: [], childCount, slot: null});

const look = new URLSearchParams(location.search);
// Only these fonts and this scale are ever written into the page's style.
const font = ['serif', 'monospace'].find((name) => name === look.get('font'));
const scale = look.get('scale') === '1.15' ? 1.15 : 1;
const lookSheet = document.createElement('style');
const long = look.get('long') === 'normal' ? '--mosaica-long-text: normal;' : '';
lookSheet.textContent = `mosaica-hierarchy, mosaica-colour-editor, mosaica-item-card { ${font ? `--mosaica-font: ${font};` : ''} ${scale === 1 ? '' : `font-size: ${13 * scale}px;`} ${long} }`;
document.head.append(lookSheet);

document.body.innerHTML = `<div style="width: 300px"><mosaica-hierarchy></mosaica-hierarchy></div>
  <div style="width: 300px"><mosaica-hierarchy id="plain"></mosaica-hierarchy></div>
  <mosaica-colour-editor field="cluster:topics"></mosaica-colour-editor>
  <div style="width: 300px"><mosaica-item-card id="full"></mosaica-item-card></div>
  <div style="width: 300px"><mosaica-item-card id="compact" compact title-field="title"></mosaica-item-card></div>`;

// The hierarchy: a long name with both clauses on it, open on a child at depth one.
const store = fakeStore({meta: meta({layers: [layer('topics')]}), status: status({})});
store.set('view', {...store.get('view'), id: 's0'});
store.set('artifacts', {...store.get('artifacts'), palette: 'kelly'});
store.set('filters', {
  ...store.get('filters'),
  members: [
    {layer: 'topics', artifact: 1n, outside: false, verb: 'filter'},
    {layer: 'topics', artifact: 1n, outside: false, verb: 'highlight'}
  ]
});
store.setBrowse('roots', {artifacts: [browseRow(1n, LONG, 2), browseRow(2n, 'Anatomy')], parents: [], next: null});
store.setBrowse('p:1', {artifacts: [browseRow(11n, LONG), browseRow(12n, 'Cysts', 1)], parents: [], next: 'more'});
const hierarchy = document.querySelector('mosaica-hierarchy') as HTMLElement & {store: unknown};
hierarchy.store = store;
await settled();
await settled();
// The status region has a box while it has nothing to say.
const region = hierarchy.shadowRoot!.querySelector('[role="status"]')!;
const status0 = {rendered: getComputedStyle(region).display !== 'none', said: region.textContent!.trim()};
(hierarchy.shadowRoot!.querySelector('[part="expander"]') as HTMLButtonElement).click();
await settled();
await settled();
const tree: {small: string[]; overlaps: string[]; outside: string[]; swap: string[]; covered: string[]} = {small: [], overlaps: [], outside: [], swap: [], covered: []};
/** Which of a row's buttons and counts are drawn: the buttons over the counts, or the counts. */
const drawn = (row: HTMLElement) => `${getComputedStyle(row.querySelector('[part="actions"]')!).opacity === '1' ? 'buttons' : ''}${getComputedStyle(row.querySelector('[part="counts"]')!).opacity === '1' ? 'counts' : ''}`;
for (const row of hierarchy.shadowRoot!.querySelectorAll<HTMLElement>('[part="row"]')) {
  const id = row.dataset.id!;
  if (drawn(row) !== 'counts') tree.swap.push(`${id} at rest: ${drawn(row)}`);
  // Focus in the row shows its buttons in the counts' place.
  row.querySelector<HTMLElement>('[part="name"]')!.focus();
  await frames(1);
  if (drawn(row) !== 'buttons') tree.swap.push(`${id} focused: ${drawn(row)}`);
  const called = (b: HTMLElement) => `${id} ${b.getAttribute('part')}${b.dataset.verb ? ` ${b.dataset.verb}` : ''}`;
  const box = (b: HTMLElement) => boxOf(called(b), b);
  const buttons = [...row.querySelectorAll<HTMLElement>('button:not([data-leaf])')];
  const targets = buttons.map(box);
  tree.small.push(...small(hierarchy.shadowRoot!, buttons.map((b) => [called(b), b])));
  tree.overlaps.push(...overlaps(targets));
  // A press at the middle of each button shown lands on it, not on the counts it stands in for.
  for (const b of row.querySelectorAll<HTMLElement>('[part="actions"] button')) {
    // Where the tree scrolls, a button on a row's second line may be out of sight until it is asked for.
    b.scrollIntoView({block: 'nearest'});
    const r = b.getBoundingClientRect();
    const at = hierarchy.shadowRoot!.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
    if (!at || !b.contains(at)) tree.covered.push(`${box(b).name} under ${at?.getAttribute('part') ?? at?.localName}`);
  }
  // The counts, when they are drawn, are clear of every button that is.
  tree.overlaps.push(...overlaps([boxOf(`${id} counts`, row.querySelector('[part="counts"]')!), ...[...row.querySelectorAll<HTMLElement>('[part="expander"]:not([data-leaf]), [part="name"], [part="dismiss"]')].map(box)]));
  const rowBox = boxOf(id, row);
  tree.outside.push(...targets.filter((t) => t.left < rowBox.left - 0.5 || t.right > rowBox.right + 0.5).map((t) => t.name));
  // Escape puts the buttons away and the counts back, with focus still in the row.
  document.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true}));
  await settled();
  if (drawn(row) !== 'counts' || !row.matches(':focus-within')) tree.swap.push(`${id} after Escape: ${drawn(row)}`);
}
tree.small.push(...small(hierarchy.shadowRoot!, [...hierarchy.shadowRoot!.querySelectorAll('[part="more"]')].map((b) => ['more', b])));

// Ordinary names and counts, at 300 px: where each row's name, counts and buttons lie, without a
// filter and with one. Every length is measured, so none depends on the font.
const NAMES = ['Anatomy', 'Neoplasms', 'Nervous System', 'Digestive System', 'Cardiovascular Diseases', 'Musculoskeletal Diseases', LONG];
const plainStore = fakeStore({meta: meta({layers: [layer('topics')]}), status: status({})});
plainStore.set('view', {...plainStore.get('view'), id: 's0'});
plainStore.setBrowse('roots', {artifacts: NAMES.map((name, i) => ({...browseRow(BigInt(i + 1), name, i % 2), maskedCount: 24_000n, matchedCount: 3_100n})), parents: [], next: null});
const plain = document.querySelector('#plain') as HTMLElement & {store: unknown};
plain.store = plainStore;
await settled();
await settled();
const width = (el: Element) => el.getBoundingClientRect().width;
const px = (value: string) => parseFloat(value) || 0;
const rowShapes = () =>
  [...plain.shadowRoot!.querySelectorAll<HTMLElement>('[part="row"]')].map((row) => {
    const name = row.querySelector<HTMLElement>('[part="name"]')!;
    const counts = row.querySelector('[part="counts"]')!.getBoundingClientRect();
    const text = document.createRange();
    text.selectNodeContents(name);
    const lines = [...text.getClientRects()];
    // The name's width on one line, whatever the room it has.
    name.style.whiteSpace = 'nowrap';
    const unwrapped = text.getBoundingClientRect().width;
    name.style.whiteSpace = '';
    const rowStyle = getComputedStyle(row);
    const nameStyle = getComputedStyle(name);
    const nameBox = name.getBoundingClientRect();
    return {
      name: name.textContent!.trim(),
      lines: new Set(lines.map((r) => Math.round(r.top))).size,
      // What the name needs on one line, and what the row leaves its button beside the expander
      // and the counts: the buttons shown over the counts take none of it.
      needs: unwrapped + px(nameStyle.paddingLeft) + px(nameStyle.paddingRight),
      room: row.clientWidth - px(rowStyle.paddingLeft) - px(rowStyle.paddingRight) - width(row.querySelector('[part="expander"]')!) - 2 * px(rowStyle.columnGap) - counts.width,
      has: nameBox.width,
      cut: name.scrollWidth > name.clientWidth + 1,
      height: row.getBoundingClientRect().height,
      least: px(rowStyle.minHeight),
      countsOnFirstLine: counts.top < Math.min(...lines.map((r) => r.bottom)),
      countsBesideName: counts.top < nameBox.bottom && counts.bottom > nameBox.top,
      // How much taller the row is than its name's lines, or than its least height, need.
      spare: row.getBoundingClientRect().height - Math.max(px(rowStyle.minHeight), nameBox.height)
    };
  });
const rows = {plain: rowShapes(), filtered: [] as ReturnType<typeof rowShapes>};
plainStore.set('filters', {...plainStore.get('filters'), members: [{layer: 'topics', artifact: 99n, outside: false, verb: 'filter'}]});
await settled();
await settled();
rows.filtered = rowShapes();
// For the test's pointer: where the second row's name and Filter button are, and what was pressed.
(window as unknown as {plainRow: unknown}).plainRow = () => {
  const row = plain.shadowRoot!.querySelector<HTMLElement>('[part="row"][data-id="2"]')!;
  row.scrollIntoView({block: 'center'});
  const middle = (el: Element) => {
    const r = el.getBoundingClientRect();
    return {x: r.left + r.width / 2, y: r.top + r.height / 2};
  };
  return {
    name: middle(row.querySelector('[part="name"]')!),
    filter: middle(row.querySelector('[part="filter"]')!),
    pressed: plainStore.calls.filter((c) => c.name === 'setMembers').map((c) => (c.args[0] as {artifact: bigint; verb: string}[]).map((m) => `${m.verb} ${m.artifact}`).join(', '))
  };
};

// The colour editor: a long name, a ticked row, and the picker over Kelly's 22 colours.
const editor = document.querySelector('mosaica-colour-editor') as HTMLElement & {store: unknown; show(): Promise<void>};
editor.store = store;
await settled();
await editor.show();
await settled();
answerAggregate(
  store,
  'colours-ranked',
  aggregateEntry([{rows: [{key: 7n, count: 900, title: LONG, slot: 2}, {key: 8n, count: 500, title: 'lasers', slot: 0}], groups: 2}], 's0', undefined, 'kelly')
);
await settled();
const shadow = editor.shadowRoot!;
const check = shadow.querySelector('[part="check"]') as HTMLInputElement;
check.click();
await settled();
const editorRows = [...shadow.querySelectorAll<HTMLElement>('[part~="row"]')];
// A checkbox's target is its label, drawn around the box.
const rowTargets = editorRows.flatMap((row): [string, Element, Element?][] => [
  [`${row.dataset.key} check`, row.querySelector('[part="check"]')!.parentElement!, row.querySelector('[part="check"]')!],
  [`${row.dataset.key} swatch`, row.querySelector('[part="swatch"]')!]
]);
const firstName = shadow.querySelector<HTMLElement>('[part="name"]')!;
const editorResult = {
  small: small(shadow, [...rowTargets, ['set-colour', shadow.querySelector('[part="set-colour"]')!], ['deselect', shadow.querySelector('[part="deselect"]')!]]),
  overlaps: overlaps(rowTargets.map(([name, target]) => boxOf(name, target))),
  nameCut: firstName.scrollWidth > firstName.clientWidth + 1,
  nameLines: (() => {
    const text = document.createRange();
    text.selectNodeContents(firstName);
    return new Set([...text.getClientRects()].map((r) => Math.round(r.top))).size;
  })(),
  rowHeights: editorRows.map((row) => row.getBoundingClientRect().height),
  // The label's centre toggles its checkbox, as its corner does.
  cornerTicks: (() => {
    const label = check.parentElement!.getBoundingClientRect();
    const at = shadow.elementFromPoint(label.left + 1, label.top + 1);
    return at === check || at === check.parentElement;
  })()
};
(shadow.querySelector('[part="swatch"]') as HTMLButtonElement).click();
await settled();
const choices = [...shadow.querySelectorAll('[part="choice"]')].map((c, i) => boxOf(`choice ${i}`, c));
const pop = shadow.querySelector<HTMLElement>('[part="colour-popover"]')!;
const popBox = boxOf('picker', pop);
const picker = {
  // Clear of the row it was opened from, and inside the window.
  coversRow: overlaps([popBox, boxOf('row', shadow.querySelector('[part~="row"]')!)]),
  inside: popBox.left >= 0 && popBox.top >= 0 && popBox.right <= innerWidth && popBox.bottom <= innerHeight,
  count: choices.length,
  small: small(shadow, [...[...shadow.querySelectorAll('[part="choice"]')].map((c, i): [string, Element] => [`choice ${i}`, c]), ['reset', pop.querySelector('[part="reset"]')!]]),
  overlaps: overlaps(choices),
  wide: pop.scrollWidth > pop.clientWidth,
  // The hue bar is drawn 10 px high; a press 6 px above it is still on it.
  hueTarget: (() => {
    const hue = pop.querySelector('[part="hue"]')!;
    const r = hue.getBoundingClientRect();
    return shadow.elementFromPoint(r.left + r.width / 2, r.top - 6) === hue && shadow.elementFromPoint(r.left + r.width / 2, r.bottom + 6) === hue;
  })()
};
pop.querySelector<HTMLElement>('[part="choice"]')!.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true, composed: true}));
await settled();
(shadow.querySelector('[part="done"]') as HTMLButtonElement).click();
await settled();

// The item cards: prose on the full card, a long headline on the compact one that shows every field.
const prose = Array.from({length: 60}, (_, i) => `Sentence ${i} of an abstract that runs on.`).join(' ');
const full = document.querySelector('#full') as HTMLElement & {item: unknown};
full.item = {id: 5n, detail: {fields: {abstract: prose, note: 'short'}, labels: [], views: [], scoped: {}} as ItemDetail};
const compact = document.querySelector('#compact') as HTMLElement & {item: unknown};
compact.item = {id: 6n, detail: {fields: {title: prose}, labels: [], views: [], scoped: {}} as ItemDetail};
await settled();
await settled();
const whole = (el: HTMLElement) => el.scrollHeight <= el.clientHeight + 1;
const value = () => full.shadowRoot!.querySelector<HTMLElement>('[part="field"][data-name="abstract"] [part="value"]')!;
const more = full.shadowRoot!.querySelector<HTMLButtonElement>('[part="more"]');
const proseBefore = value().getBoundingClientRect().height;
const moreBox = more ? small(full.shadowRoot!, [['more', more]]) : ['no more button'];
more?.click();
await settled();
const proseAfter = value().getBoundingClientRect().height;
const headline = () => compact.shadowRoot!.querySelector<HTMLElement>('[part="headline"]')!;
const showAll = compact.shadowRoot!.querySelector<HTMLButtonElement>('[part="show-all"]');
const headlineBefore = {height: headline().getBoundingClientRect().height, whole: whole(headline())};
const showAllBox = showAll ? small(compact.shadowRoot!, [['show-all', showAll]]) : ['no show-all button'];
showAll?.click();
await settled();
const headlineAfter = {height: headline().getBoundingClientRect().height, whole: whole(headline())};

(window as unknown as {result: unknown}).result = {
  tree: {...tree, status: status0, rows},
  editor: editorResult,
  picker,
  cards: {moreBox, proseBefore, proseAfter, proseText: value().textContent === prose, showAllBox, headlineBefore, headlineAfter}
};
