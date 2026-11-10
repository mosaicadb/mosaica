import '../../src/index.js';
import type {Artifact, ArtifactsProjection, BrowseRow, ItemDetail, Meta} from '@mosaicajs/client';
import {SessionArtifactTable, attachedTextOf, servedLineage} from '@mosaicajs/client/internal';
import {aggregateEntry, answerAggregate, fakeStore, meta, scalar, settle, status} from '../fake-store.js';

// Built by `hierarchy-cards-look.browser.ts` with Vite from the packages' sources. The hierarchy,
// the item card full and compact, the artifact card and the colour editor with its picker, at
// rest in a panel `?panel=` px wide; `window.result` holds the distances between their parts, for
// the test to hold against the sizes the elements are drawn at, and any text drawn outside the
// parts that draw text.
const panel = new URLSearchParams(location.search).get('panel') === '264' ? 264 : 340;
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
const box = (el: Element) => el.getBoundingClientRect();
const size = (el: Element) => [box(el).width, box(el).height];
/** How far `b` starts below where `a` ends, and to the right of where it ends. */
const below = (a: Element, b: Element) => box(b).top - box(a).bottom;
const after = (a: Element, b: Element) => box(b).left - box(a).right;
/** How far `b`'s left edge is from `a`'s. */
const indent = (a: Element, b: Element) => box(b).left - box(a).left;
/** The lines an element's text is drawn on. */
const lines = (el: Element) => {
  const text = document.createRange();
  text.selectNodeContents(el);
  return new Set([...text.getClientRects()].map((r) => Math.round(r.top))).size;
};

/**
 * The text drawn inside `root` that is in none of the parts `drawn` names. Text is drawn unless it
 * or something around it is clipped away, hidden or transparent. Another element's shadow root is
 * that element's to answer for.
 */
function strayText(root: ShadowRoot, drawn: string): string[] {
  const stray: string[] = [];
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const text = node.textContent!.trim();
    const parent = node.parentElement;
    if (text === '' || !parent || parent.closest(drawn) || parent.closest('style, option')) continue;
    const range = document.createRange();
    range.selectNodeContents(node);
    if ([...range.getClientRects()].every((r) => r.width === 0 || r.height === 0)) continue;
    let hidden = false;
    for (let at: Element | null = parent; at && !hidden; at = at.parentElement) {
      const style = getComputedStyle(at);
      hidden = style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0' || style.clipPath !== 'none';
    }
    if (!hidden) stray.push(text);
  }
  return stray;
}

const layer = (name: string): Meta['layers'][number] =>
  ({name, title: name, views: ['s0'], membership: 'enumerated', hierarchy: {kind: 'nested', pruneChildren: false}, levels: [], computedContent: ['centroid', 'box', 'hull'], shape: 'derived', suppliedContent: ['name'], depsOn: [], version: 1}) as unknown as Meta['layers'][number];
const LONG = 'Neoplasms of the connective and soft tissue, including those of uncertain behaviour and unspecified site';
const browseRow = (id: bigint, name: string, childCount = 0): BrowseRow => ({mosaicaId: id, key: `k${id}`, name, maskedCount: 24_000n, matchedCount: 3_100n, rung: 0, parentIds: [], childCount, slot: null});
const META = meta({layers: [layer('topics')], declaredScalars: [scalar('title', 'utf8'), scalar('archive', 'utf8'), scalar('year', 'u16'), scalar('note', 'utf8'), scalar('pages', 'u16')]});

document.body.innerHTML = `<div style="width: ${panel}px">
    <mosaica-hierarchy></mosaica-hierarchy>
    <mosaica-item-card id="full" title-field="title" open-url="https://example.org/{mosaica_id}"></mosaica-item-card>
    <mosaica-item-card id="compact" compact title-field="title" subtitle-field="archive" open-url="https://example.org/{mosaica_id}"></mosaica-item-card>
    <mosaica-artifact-card></mosaica-artifact-card>
  </div>
  <mosaica-colour-editor field="cluster:topics"></mosaica-colour-editor>`;

// The hierarchy under a filter: a long name, a row with a filter and a highlight on it, open on
// its children, with More under them.
const store = fakeStore({meta: META, status: status({})});
store.set('view', {...store.get('view'), id: 's0'});
store.set('filters', {
  ...store.get('filters'),
  members: [
    {layer: 'topics', artifact: 2n, outside: false, verb: 'filter'},
    {layer: 'topics', artifact: 2n, outside: false, verb: 'highlight'}
  ]
});
store.setBrowse('roots', {artifacts: [browseRow(1n, LONG, 2), browseRow(2n, 'Anatomy'), browseRow(3n, 'Neoplasms', 1)], parents: [], next: null});
store.setBrowse('p:1', {artifacts: [browseRow(11n, LONG), browseRow(12n, 'Cysts', 1)], parents: [], next: 'more'});
const hierarchy = document.querySelector('mosaica-hierarchy') as HTMLElement & {store: unknown};
hierarchy.store = store;
await settled();
await settled();
const h = hierarchy.shadowRoot!;
(h.querySelector('[part="expander"]') as HTMLButtonElement).click();
await settled();
await settled();
const treeRows = [...h.querySelectorAll<HTMLElement>('[part="row"]')];
const part = (within: ParentNode, name: string) => within.querySelector<HTMLElement>(`[part="${name}"]`)!;
const clausesRow = h.querySelector<HTMLElement>('[part="row"][data-id="2"]')!;
const open = treeRows[0]!;
const tree = {
  rows: treeRows.map((row) => box(row).height),
  nameLines: treeRows.map((row) => lines(part(row, 'name'))),
  // Along a row: its indent, the expander's box, and the gaps to the name, the counts and each ×.
  indents: [indent(open, part(open, 'expander')), indent(treeRows[1]!, part(treeRows[1]!, 'expander'))],
  expander: size(part(open, 'expander')),
  gaps: [after(part(open, 'expander'), part(open, 'name')), after(part(open, 'name'), part(open, 'counts')), after(part(clausesRow, 'counts'), part(clausesRow, 'dismiss'))],
  dismiss: [...clausesRow.querySelectorAll('[part="dismiss"]')].map(size),
  dismissGlyphs: [...clausesRow.querySelectorAll('[part="dismiss"]')].map((b) => b.querySelectorAll('svg').length),
  dismissBars: [...clausesRow.querySelectorAll('[part="dismiss"]')].map((b) => getComputedStyle(b).boxShadow !== 'none'),
  // Down the panel: the heading, the search box, the tree, and More under the open row's children.
  title: [box(part(h, 'title')).height, below(part(h, 'title'), part(h, 'search'))],
  search: [box(part(h, 'search')).height, below(part(h, 'search'), part(h, 'tree'))],
  top: box(part(h, 'tree')).top - box(hierarchy).top,
  bottom: box(hierarchy).bottom - box(part(h, 'tree')).bottom,
  more: [box(part(h, 'more')).height, below(treeRows[2]!, part(h, 'more')), below(part(h, 'more'), treeRows[3]!)],
  stray: strayText(h, '[part="title"], [part="name"], [part="counts"], [part="more"], [part="also"], [part="state"]')
};

// The item card: full, then compact, each with a headline, short fields and an Open link.
const item = {id: 5n, detail: {fields: {title: 'A paper on maps', archive: 'Quantum Physics', year: 2019, note: 'short', pages: 12}, labels: [], views: [], scoped: {}} as ItemDetail};
const full = document.querySelector('#full') as HTMLElement & {item: unknown; meta: unknown};
const compact = document.querySelector('#compact') as HTMLElement & {item: unknown; meta: unknown};
for (const card of [full, compact]) {
  card.meta = META;
  card.item = item;
}
await settled();
await settled();
const f = full.shadowRoot!;
const c = compact.shadowRoot!;
const fields = (root: ShadowRoot) => [...root.querySelectorAll<HTMLElement>('[part="field"]')];
const cardText = '[part="headline"], [part="subtitle"], [part="label"], [part="value"], [part="show-all"], [part="open"], [part="copy"], [part="state"]';
const cards = {
  full: {
    close: size(part(f, 'close')),
    // A field's value starts a fixed way from its label, on the label's line, and the next field a fixed way under it.
    column: indent(part(fields(f)[0]!, 'label'), part(fields(f)[0]!, 'value')),
    sameLine: fields(f).map((field) => Math.abs(box(part(field, 'label')).top - box(part(field, 'value')).top) < 4),
    rowGap: below(part(fields(f)[0]!, 'label'), part(fields(f)[1]!, 'label')),
    actions: [below(f.querySelector('.field')!, part(f, 'open')), box(part(f, 'open')).height, box(part(f, 'copy')).height, after(part(f, 'open'), part(f, 'copy'))],
    openIcons: part(f, 'open').querySelectorAll('svg').length,
    stray: strayText(f, cardText)
  },
  compact: {
    close: size(part(c, 'close')),
    fields: fields(c).length,
    showAll: [box(part(c, 'show-all')).height, lines(part(c, 'show-all'))],
    open: box(part(c, 'open')).height,
    openIcons: part(c, 'open').querySelectorAll('svg').length,
    // The foot: its rule a fixed way under the fields, and Show all a fixed way under the rule.
    foot: [below(c.querySelector('.body')!, c.querySelector('.foot')!), box(part(c, 'open')).top - box(c.querySelector('.foot')!).top],
    stray: strayText(c, cardText)
  }
};

// The artifact card on a cluster with a parent and two children, one of a long name.
const artifact = (id: bigint, name: string, parent: bigint | null): Artifact => ({layer: 'topics', mosaicaId: id, key: `c-${id}`, maskedCount: 4_200n, centroid: [2 ** 31, 2 ** 31], box: null, content: [name], parentIds: parent === null ? [] : [parent], rung: 0, matched: null, highlighted: null, target: null, slot: null});
const served = [artifact(1n, 'Physics', null), artifact(2n, 'Optics', 1n), artifact(3n, LONG, 2n), artifact(4n, 'Lasers', 2n)];
const table = new SessionArtifactTable();
const ordinals = table.take(served.map((a) => ({mosaicaId: a.mosaicaId, layer: a.layer, parentIds: a.parentIds})));
const artifacts: ArtifactsProjection = {
  ...store.get('artifacts'),
  layer: 'topics',
  layers: ['topics'],
  served,
  attached: attachedTextOf(served, META.layers),
  lineage: servedLineage(served),
  status: 'shown',
  version: 1,
  held: served.length,
  table,
  servedOrdinals: new Set(ordinals),
  palette: 'tableau10'
};
store.set('artifacts', artifacts);
store.set('selection', {item: null, itemRefusal: null, artifact: {id: 2n, detail: {layer: 'topics', key: 'c-2', maskedCount: 4_200n, centroid: null, box: null, shape: null}}, artifactRefusal: null});
const artifactCard = document.querySelector('mosaica-artifact-card') as HTMLElement & {store: unknown};
artifactCard.store = store;
await settled();
await settled();
const a = artifactCard.shadowRoot!;
const labels = [...a.querySelectorAll<HTMLElement>('[part="label"]')];
const children = [...a.querySelectorAll<HTMLElement>('[part="child"]')];
const card = {
  title: box(part(a, 'title')).height,
  headline: below(part(a, 'title'), part(a, 'headline')),
  rows: [part(a, 'parent'), ...children].map((row) => box(row).height),
  nameIndent: indent(part(a, 'parent'), part(part(a, 'parent'), 'name')),
  nameLines: children.map((row) => lines(part(row, 'name'))),
  // Each list's label a fixed way under what is over it, and the list a fixed way under its label.
  labels: [below(a.querySelector('.field')!, labels[0]!), below(labels[0]!, part(a, 'parents')), below(part(a, 'parents'), labels[1]!), below(labels[1]!, part(a, 'children'))],
  labelLines: labels.map((label) => box(label).height),
  fit: [below(part(a, 'children'), part(a, 'fit')), box(part(a, 'fit')).height, below(part(a, 'fit'), part(a, 'verbs'))],
  stray: strayText(a, '[part="title"], [part="headline"], [part="count"], [part="content"], .k, [part="value"], [part="shape"], [part="label"], [part="name"], [part="parent"], [part="child"], [part="fit"], [part="verbs"], [part="state"]')
};

// The colour editor on a ten-colour palette, a row ticked, and its picker open.
const editor = document.querySelector('mosaica-colour-editor') as HTMLElement & {store: unknown; show(): Promise<void>};
editor.store = store;
await settled();
await editor.show();
await settled();
answerAggregate(store, 'colours-ranked', aggregateEntry([{rows: [{key: 7n, count: 900, title: LONG, slot: 2}, {key: 8n, count: 500, title: 'lasers', slot: 0}], groups: 2}], 's0', undefined, 'tableau10'));
await settled();
const e = editor.shadowRoot!;
(part(e, 'check') as HTMLInputElement).click();
await settled();
const editorRows = [...e.querySelectorAll<HTMLElement>('[part~="row"]')];
const first = editorRows[0]!;
const dialog = {
  rows: editorRows.map((row) => box(row).height),
  nameLines: editorRows.map((row) => lines(part(row, 'name'))),
  check: size(part(first, 'check')),
  swatch: size(part(first, 'swatch')),
  // Along a row: from the checkbox to the swatch, and from the swatch to the name.
  columns: [indent(part(first, 'check'), part(first, 'swatch')), indent(part(first, 'swatch'), part(first, 'name'))],
  selected: box(part(e, 'selected')).height,
  titleLines: lines(part(e, 'title')),
  stray: [] as string[]
};
dialog.stray = strayText(e, '[part="title"], [part="sub"], [part="selected"], [part="columns"], [part="name"], [part="path"], [part="count"], .none, [part="reset-all"], [part="changed"], [part="done"]');
(part(first, 'swatch') as HTMLButtonElement).click();
await settled();
const pop = part(e, 'colour-popover');
const choices = [...pop.querySelectorAll('[part="choice"]')];
const picker = {
  choices: choices.map(size),
  rows: new Set(choices.map((choice) => Math.round(box(choice).top))).size,
  // From the hex field to the picker's foot: nothing is drawn under the field.
  foot: box(pop).bottom - box(part(pop, 'hex')).bottom,
  top: below(pop.querySelector('.top')!, pop.querySelector('.choices')!),
  titleLines: lines(pop.querySelector('.t')!),
  reset: box(part(pop, 'reset')).height,
  stray: strayText(e, '[part="title"], [part="sub"], [part="selected"], [part="columns"], [part="name"], [part="path"], [part="count"], .none, [part="reset-all"], [part="changed"], [part="done"], .t, [part="reset"], .hd, label')
};

// Each element's own height, and the width each leaves a name.
const whole = {
  heights: [hierarchy, full, compact, artifactCard, pop].map((el) => box(el).height),
  names: [...treeRows.map((row) => box(part(row, 'name')).width), box(part(f, 'headline')).width, box(part(c, 'headline')).width, box(part(a, 'headline')).width, ...children.map((row) => box(part(row, 'name')).width), ...editorRows.map((row) => box(part(row, 'name')).width)]
};

(window as unknown as {result: unknown}).result = {tree, cards, card, dialog, picker, whole};
