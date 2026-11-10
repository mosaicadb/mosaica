import '../../src/index.js';
import type {BrowseRow, ItemDetail, Meta} from '@mosaicajs/client';
import {aggregateEntry, answerAggregate, fakeStore, meta, settle, status} from '../fake-store.js';

// Built by `targets.browser.ts` with Vite from the packages' sources. A hierarchy with a filtered
// and highlighted row, another with ordinary names, a colour editor over a 22-colour palette with
// its picker open, and two item cards with long text, each 300 px wide or in its dialog;
// `window.result` holds what the test measures.
type Box = {name: string; left: number; top: number; right: number; bottom: number};
const boxOf = (name: string, el: Element): Box => {
  const r = el.getBoundingClientRect();
  return {name, left: r.left, top: r.top, right: r.right, bottom: r.bottom};
};
/** The pairs of `boxes` that share any area. */
const overlaps = (boxes: Box[]): string[] =>
  boxes.flatMap((a, i) => boxes.slice(i + 1).flatMap((b) => (a.left < b.right - 0.5 && b.left < a.right - 0.5 && a.top < b.bottom - 0.5 && b.top < a.bottom - 0.5 ? [`${a.name} over ${b.name}`] : [])));
/** The boxes of `boxes` narrower or shorter than a 24 px target. */
const small = (boxes: Box[]): string[] => boxes.filter((b) => b.right - b.left < 23.5 || b.bottom - b.top < 23.5).map((b) => `${b.name} ${Math.round(b.right - b.left)}x${Math.round(b.bottom - b.top)}`);
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
const browseRow = (id: bigint, name: string, childCount = 0): BrowseRow => ({mosaicaId: id, key: `k${id}`, name, maskedCount: 27_000_000n, matchedCount: 1_234_567n, rung: 0, parentIds: [], childCount, slot: null});

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
const tree: {small: string[]; overlaps: string[]; outside: string[]; cut: string[]; swap: string[]} = {small: [], overlaps: [], outside: [], cut: [], swap: []};
/** Which of a row's buttons and counts are drawn: the buttons over the counts, or the counts. */
const drawn = (row: HTMLElement) => `${getComputedStyle(row.querySelector('[part="actions"]')!).opacity === '1' ? 'buttons' : ''}${getComputedStyle(row.querySelector('[part="counts"]')!).opacity === '1' ? 'counts' : ''}`;
for (const row of hierarchy.shadowRoot!.querySelectorAll<HTMLElement>('[part="row"]')) {
  const id = row.dataset.id!;
  if (drawn(row) !== 'counts') tree.swap.push(`${id} at rest: ${drawn(row)}`);
  // Focus in the row shows its buttons in the counts' place.
  row.querySelector<HTMLElement>('[part="name"]')!.focus();
  await frames(1);
  if (drawn(row) !== 'buttons') tree.swap.push(`${id} focused: ${drawn(row)}`);
  const box = (b: HTMLElement) => boxOf(`${id} ${b.getAttribute('part')}${b.dataset.verb ? ` ${b.dataset.verb}` : ''}`, b);
  const targets = [...row.querySelectorAll<HTMLElement>('button:not([data-leaf])')].map(box);
  tree.small.push(...small(targets));
  tree.overlaps.push(...overlaps(targets));
  // The counts, when they are drawn, are clear of every button that is.
  tree.overlaps.push(...overlaps([boxOf(`${id} counts`, row.querySelector('[part="counts"]')!), ...[...row.querySelectorAll<HTMLElement>('.lead button:not([data-leaf]), [part="dismiss"]')].map(box)]));
  const rowBox = boxOf(id, row);
  tree.outside.push(...targets.filter((t) => t.left < rowBox.left - 0.5 || t.right > rowBox.right + 0.5).map((t) => t.name));
  const name = row.querySelector<HTMLElement>('[part="name"]')!;
  if (name.scrollWidth > name.clientWidth + 1 || name.scrollHeight > name.clientHeight + 1) tree.cut.push(id);
  // Escape puts the buttons away and the counts back, with focus still in the row.
  document.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true}));
  await settled();
  if (drawn(row) !== 'counts' || !row.matches(':focus-within')) tree.swap.push(`${id} after Escape: ${drawn(row)}`);
}
tree.small.push(...small([...hierarchy.shadowRoot!.querySelectorAll('[part="more"]')].map((b) => boxOf('more', b))));

// Ordinary names and counts, at 300 px: how tall each row is, without a filter and with one.
const NAMES = ['Anatomy', 'Neoplasms', 'Nervous System', 'Digestive System', 'Cardiovascular Diseases', 'Musculoskeletal Diseases', LONG];
const plainStore = fakeStore({meta: meta({layers: [layer('topics')]}), status: status({})});
plainStore.set('view', {...plainStore.get('view'), id: 's0'});
plainStore.setBrowse('roots', {artifacts: NAMES.map((name, i) => ({...browseRow(BigInt(i + 1), name, i % 2), maskedCount: 2_400_000n, matchedCount: 31_200n})), parents: [], next: null});
const plain = document.querySelector('#plain') as HTMLElement & {store: unknown};
plain.store = plainStore;
await settled();
await settled();
const heights = () => Object.fromEntries([...plain.shadowRoot!.querySelectorAll<HTMLElement>('[part="row"]')].map((row) => [row.querySelector('[part="name"]')!.textContent!.trim().length, Math.round(row.getBoundingClientRect().height)]));
const rowHeights = {plain: heights(), filtered: {} as Record<string, number>};
plainStore.set('filters', {...plainStore.get('filters'), members: [{layer: 'topics', artifact: 99n, outside: false, verb: 'filter'}]});
await settled();
await settled();
rowHeights.filtered = heights();

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
const rowTargets = [...shadow.querySelectorAll<HTMLElement>('[part~="row"]')].flatMap((row) => [
  boxOf(`${row.dataset.key} check`, row.querySelector('[part="check"]')!.parentElement!),
  boxOf(`${row.dataset.key} swatch`, row.querySelector('[part="swatch"]')!)
]);
const barTargets = [boxOf('set-colour', shadow.querySelector('[part="set-colour"]')!), boxOf('deselect', shadow.querySelector('[part="deselect"]')!)];
const firstName = shadow.querySelector<HTMLElement>('[part="name"]')!;
const editorResult = {
  small: small([...rowTargets, ...barTargets]),
  overlaps: overlaps(rowTargets),
  nameCut: firstName.scrollWidth > firstName.clientWidth + 1,
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
  small: small([...choices, boxOf('reset', pop.querySelector('[part="reset"]')!)]),
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
const moreBox = more ? small([boxOf('more', more)]) : ['no more button'];
more?.click();
await settled();
const proseAfter = value().getBoundingClientRect().height;
const headline = () => compact.shadowRoot!.querySelector<HTMLElement>('[part="headline"]')!;
const showAll = compact.shadowRoot!.querySelector<HTMLButtonElement>('[part="show-all"]');
const headlineBefore = {height: headline().getBoundingClientRect().height, whole: whole(headline())};
const showAllBox = showAll ? small([boxOf('show-all', showAll)]) : ['no show-all button'];
showAll?.click();
await settled();
const headlineAfter = {height: headline().getBoundingClientRect().height, whole: whole(headline())};

(window as unknown as {result: unknown}).result = {
  tree: {...tree, status: status0, rowHeights},
  editor: editorResult,
  picker,
  cards: {moreBox, proseBefore, proseAfter, proseText: value().textContent === prose, showAllBox, headlineBefore, headlineAfter}
};
