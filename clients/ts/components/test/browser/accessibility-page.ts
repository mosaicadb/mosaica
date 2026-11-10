import '../../src/index.js';
import type {AggregateSpec, Artifact, ArtifactsProjection, BrowseRow, ItemDetail, Layer, Meta, RegionProjection, ViewInfo} from '@mosaicajs/client';
import {SessionArtifactTable, attachedTextOf, servedLineage} from '@mosaicajs/client/internal';
import {aggregateEntry, deepText, fakeStore, meta, registered, scalar, settle, status, type AggregateTableSpec, type FakeStore} from '../fake-store.js';
import {clippedText, contrastFault, crowdedTargets, focusStop, forgetStops, spaceText, textElements, wideContent} from './accessibility-checks.js';

// Built by `accessibility.browser.ts` with Vite from the packages' sources. A page with a language,
// a title, a main landmark and a heading, as a host supplies them, holding one explorer over a
// store with no network, or the elements the explorer does not draw. `window.show(name)` builds the
// named state from nothing, and `window.checks` measures the page as it then stands.

document.documentElement.lang = 'en';
document.title = 'Places in Ireland';
const sheet = document.createElement('style');
sheet.textContent = `
  :root { color-scheme: light dark; }
  html, body { height: 100%; margin: 0; }
  body { font-family: system-ui, sans-serif; }
  main { display: flex; flex-direction: column; height: 100%; }
  header { display: flex; align-items: center; gap: 12px; padding: 4px 12px; }
  h1 { font-size: 16px; margin: 0; }
  #before { min-height: 24px; }
  #stage { flex: 1; min-height: 0; }
  #stage.loose { overflow: auto; padding: 12px; display: flex; flex-wrap: wrap; gap: 16px; align-items: flex-start; }
`;
document.head.append(sheet);
document.body.innerHTML = '<main><header><h1>Places in Ireland</h1><button id="before" type="button">About this map</button></header><div id="stage"></div></main>';
const stageEl = document.querySelector<HTMLElement>('#stage')!;

const frames = (n: number) =>
  new Promise<void>((resolve) => {
    const step = () => (--n <= 0 ? resolve() : requestAnimationFrame(step));
    requestAnimationFrame(step);
  });
const settled = async () => {
  await settle(document.body);
  await frames(3);
  await settle(document.body);
};
const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

const UNIT = {xMin: 0, xMax: 1, yMin: 0, yMax: 1};
const view = (id: string, displayName: string, roster: ViewInfo['roster'] = null): ViewInfo => ({id, displayName, quantisation: UNIT, projection: 'none', worldAspect: null, tileScheme: null, tile: null, roster});
const layer = (name: string, title: string, over: Partial<Layer> = {}): Layer =>
  ({name, title, views: ['places'], membership: 'enumerated', hierarchy: {kind: 'nested', pruneChildren: false}, levels: [], computedContent: ['centroid', 'box', 'hull'], shape: 'derived', suppliedContent: ['name'], depsOn: [], version: 1, ...over}) as Layer;

const COUNTIES = layer('counties', 'Counties');
const META: Meta = meta({
  views: [view('places', 'By place'), view('names', 'By name')],
  declaredScalars: [
    scalar('feature_class', 'u16', {category: {vocabulary: 'feature_class', kind: 'declared', visibility: 'public'}, render: true, homes: ['rendered']}),
    scalar('population', 'u32', {render: true, homes: ['rendered']}),
    scalar('modified', 'timestamp_us', {render: true, homes: ['rendered']}),
    scalar('name', 'utf8')
  ],
  filterOperands: [
    {column: 'feature_class', family: 'category', operands: ['eq', 'in']},
    {column: 'population', family: 'numeric', operands: ['range']},
    {column: 'modified', family: 'numeric', operands: ['range']},
    {column: 'name', family: 'text', operands: ['match', 'phrase']}
  ],
  layers: [COUNTIES, layer('county-names', 'County names', {depsOn: ['counties'], computedContent: []}), layer('dioceses', 'Dioceses', {computedContent: [], suppliedContent: []})]
});

const CLASSES: {key: string; title: string; count: number}[] = [
  {key: 'P', title: 'Populated place', count: 9120},
  {key: 'H', title: 'Stream, lake or other water', count: 6480},
  {key: 'T', title: 'Mountain, hill or rock', count: 5210},
  {key: 'S', title: 'Spot, building or farm', count: 4800},
  {key: 'L', title: 'Park or area', count: 2100},
  {key: 'A', title: 'Country, county or other administrative division', count: 1425},
  {key: 'V', title: 'Forest or heath', count: 800}
];
const YEAR = (y: number) => Date.UTC(y, 0, 1);

/** The table one grouping of a registered aggregate is answered with, by what it counts. */
function tableFor(grouping: AggregateSpec['groupings'][number]): AggregateTableSpec {
  const by = (grouping as {by?: {field?: string; layer?: string; bins?: number; values?: string[]; top?: number}}).by;
  if (!by) return {rows: [], total: 29_935};
  if (by.layer) return {rows: [{key: 1n, title: 'Cork', count: 3200, slot: 0}, {key: 2n, title: 'Galway', count: 2500, slot: 1}], groups: 26, total: 29_935};
  if (by.bins && by.field === 'modified') return {rows: Array.from({length: 12}, (_, i) => ({lower: YEAR(2008 + i), upper: YEAR(2009 + i), count: 400 + 310 * ((i * 5) % 7)})), total: 29_935};
  if (by.bins) return {rows: Array.from({length: by.bins}, (_, i) => ({lower: i * 5000, upper: (i + 1) * 5000, count: Math.round(9000 / (i + 1))})), total: 29_935};
  const listed = by.values ? CLASSES.filter((c) => by.values!.includes(c.key)) : CLASSES.slice(0, by.top ?? CLASSES.length);
  return {rows: listed.map((c) => ({key: c.key, title: c.title, count: c.count})), groups: CLASSES.length, total: 29_935};
}

/** Answer every aggregate the elements have registered, the visible total with `visible`. */
function answer(store: FakeStore, visible = 29_935): void {
  const held = new Map(store.get('aggregates'));
  for (const [id, spec] of registered(store) as Map<string, AggregateSpec>) {
    const summaries = spec.groupings.map((g) => {
      const fields = (g as {summary?: string[]}).summary;
      return fields ? new Map(fields.map((f) => [f, f === 'modified' ? {values: 29_935n, missing: 0n, min: YEAR(2008), max: YEAR(2020), mean: YEAR(2014)} : {values: 9120n, missing: 20_815n, min: 0, max: 100_000, mean: 1830}])) : null;
    });
    const tables = id.startsWith('visible-total') ? [{rows: [], total: visible}] : spec.groupings.map(tableFor);
    held.set(id, aggregateEntry(tables, 'places', summaries as never, 'tableau10'));
  }
  store.set('aggregates', held);
}

const artifact = (id: bigint, name: string, parent: bigint | null): Artifact => ({layer: 'counties', mosaicaId: id, key: `county-${id}`, maskedCount: 3200n, centroid: [2 ** 31, 2 ** 31], box: null, content: [name], parentIds: parent === null ? [] : [parent], rung: 0, matched: null, highlighted: null, target: null, slot: null});

function artifactsProjection(served: Artifact[]): ArtifactsProjection {
  const table = new SessionArtifactTable();
  const ordinals = table.take(served.map((a) => ({mosaicaId: a.mosaicaId, layer: a.layer, parentIds: a.parentIds})));
  return {
    layer: 'counties', layers: ['counties'], served, colourServed: [], attached: attachedTextOf(served, META.layers), lineage: servedLineage(served), status: 'shown', refusal: null, version: 1, held: served.length, table,
    servedOrdinals: new Set(ordinals), shapes: new Map(), colours: new Map([...ordinals].map((o) => [o, [31, 119, 180, 255] as const])), palette: 'tableau10', overrides: new Map(), coverage: {current: 3, stale: 0}
  };
}

const COUNTS = {status: 'shown' as const, visible: {value: 29_935, exact: true}, matched: {value: 9120, exact: true}, highlighted: {value: 9120, exact: true}, shown: 5390};

function newStore(): FakeStore {
  const store = fakeStore({meta: META, status: status({})});
  store.set('view', {...store.get('view'), id: 'places', visible: COUNTS.visible, matched: COUNTS.matched, highlighted: COUNTS.highlighted, served: {shown: 5390, total: 9120, exact: true}, inView: COUNTS});
  return store;
}

const ITEM = {id: 5n, detail: {fields: {name: 'Skibbereen', feature_class: 'P', population: 2778, modified: '2019-03-04'}, labels: [], views: [], scoped: {}} as ItemDetail};
const REGION: RegionProjection = {shape: {kind: 'box', bbox: [0.4, 0.4, 0.6, 0.6]}, status: 'shown', refusal: null, visible: {value: 412, exact: true}, matched: {value: 158, exact: true}, served: {shown: 2, total: 158, exact: true}, verdict: {exact: true, depth: null}, held: {ids: BigUint64Array.of(5n, 6n), positions: new Float32Array(4), count: 2}};

/** An explorer on the stage: its store, its shadow root and map, a part of it by name, and a press on one of its controls. */
type Stage = {store: FakeStore; shadow: ShadowRoot; map: HTMLElement & {pickedAt: unknown}; part(name: string): HTMLElement | null; press(el: Element | null | undefined): Promise<void>};

/** One explorer filling the stage over a new store, with `attrs` on its tag. */
async function explorer(attrs = ''): Promise<Stage> {
  stageEl.className = '';
  stageEl.innerHTML = `<mosaica-explorer ${attrs.includes('layout=') ? '' : 'layout="overlay"'} aria-label="Places in Ireland" dataset-title="Places in Ireland from GeoNames" title-field="name" ${attrs}></mosaica-explorer>`;
  const el = stageEl.querySelector('mosaica-explorer') as HTMLElement & {store: unknown};
  const store = newStore();
  el.store = store;
  await settled();
  answer(store);
  await settled();
  const shadow = el.shadowRoot!;
  return {
    store,
    shadow,
    map: shadow.querySelector('mosaica-map') as Stage['map'],
    part: (name) => shadow.querySelector<HTMLElement>(`[part~="${name}"]`),
    async press(target) {
      if (!target) throw new Error('nothing to press');
      (target as HTMLElement).click();
      await settled();
      answer(store);
      await settled();
    }
  };
}

const FIELDS = 'pinned-filters="feature_class population modified name"';
const FILTERED = {
  draft: {
    filter: {feature_class: {family: 'category' as const, keys: ['P', 'H']}, population: {family: 'numeric' as const, gte: 5000, lte: 50_000}},
    highlight: {feature_class: {family: 'category' as const, keys: ['T']}}
  },
  members: [{layer: 'counties', artifact: 1n, outside: false, verb: 'filter' as const}]
};

async function filtered(s: Stage): Promise<void> {
  s.store.set('artifacts', artifactsProjection([artifact(1n, 'Cork', null)]));
  s.store.set('filters', {...s.store.get('filters'), ...FILTERED});
  s.store.set('view', {...s.store.get('view'), highlighting: true, highlighted: {value: 1210, exact: true}, inView: {...COUNTS, highlighted: {value: 1210, exact: true}}});
  await settled();
  answer(s.store);
  await settled();
}

async function picked(s: Stage): Promise<void> {
  s.map.pickedAt = {kind: 'item', id: 5n, world: [300, 300]};
  s.store.set('selection', {item: ITEM, itemRefusal: null, artifact: null, artifactRefusal: null});
  await settled();
  await frames(4);
}

async function opened(s: Stage): Promise<void> {
  s.store.set('artifacts', artifactsProjection([artifact(1n, 'Munster', null), artifact(2n, 'Cork', 1n), artifact(3n, 'West Cork', 2n)]));
  s.store.set('selection', {item: null, itemRefusal: null, artifact: {id: 2n, detail: {layer: 'counties', key: 'county-2', maskedCount: 3200n, centroid: null, box: null, shape: null}}, artifactRefusal: null});
  await settled();
}

const said = async (s: Stage, over: Parameters<typeof status>[0]) => {
  s.store.set('status', status(over));
  await settled();
};
const sheetOf = (s: Stage, name: string) => s.press(s.shadow.querySelector(`[part="tabs"] [data-sheet="${name}"]`));

const coloured = async (s: Stage) => {
  s.store.set('legend', {...s.store.get('legend'), colourBy: 'feature_class'});
  await settled();
};
const fieldCard = (s: Stage, field: string) => s.shadow.querySelector('mosaica-filter-panel')!.shadowRoot!.querySelector(`mosaica-field-card[data-field="${field}"]`)!.shadowRoot!;
const refused = {status: 'refused' as const, refusal: {code: 'contract', detail: 'The viewport asks for more tiles than this server allows in one request.', status: 422}};

/** Each state of the explorer in a wide window, by name. */
const WIDE: Record<string, () => Promise<unknown>> = {
  rest: () => explorer(),
  fields: () => explorer(FIELDS),
  filtered: async () => filtered(await explorer(FIELDS)),
  docked: async () => filtered(await explorer(`${FIELDS} layout="docked"`)),
  layers: async () => {
    const s = await explorer();
    await coloured(s);
    await s.press(s.part('layers-toggle'));
  },
  menu: async () => {
    const s = await explorer();
    await coloured(s);
    await s.press(s.part('layers-toggle'));
    await s.press(s.part('colour-by'));
  },
  palettes: async () => {
    const s = await explorer();
    await coloured(s);
    await s.press(s.part('layers-toggle'));
    await s.press(s.part('palette'));
  },
  item: async () => picked(await explorer()),
  artifact: async () => opened(await explorer()),
  region: async () => {
    const s = await explorer();
    s.store.set('region', REGION);
    await settled();
  },
  refused: async () => said(await explorer(), refused),
  unanswered: async () => said(await explorer(), {status: 'refused', refusal: {code: 'http-error', detail: '', status: 504}}),
  busy: async () => said(await explorer(), {status: 'refused', refusal: {code: 'backpressure', detail: 'the viewer is shedding load', status: 429}}),
  expired: async () => {
    const s = await explorer();
    for (const strip of s.shadow.querySelectorAll('mosaica-status')) (strip as unknown as {reauthorise: () => void}).reauthorise = () => {};
    await said(s, {status: 'refused', refusal: {code: 'expired-token', detail: '', status: 403}, expired: true});
  },
  unreachable: async () => said(await explorer(), {unreachable: true}),
  reconnecting: async () => said(await explorer(), {status: 'retrying'}),
  empty: async () => said(await explorer(), {status: 'empty'}),
  'nothing-visible': async () => {
    const s = await explorer();
    await said(s, {status: 'empty'});
    answer(s.store, 0);
    await settled();
  },
  stale: async () => said(await explorer(), {stale: true}),
  updating: async () => said(await explorer(), {status: 'loading'}),
  connecting: async () => said(await explorer(), {status: 'loading', sessionWarm: false}),
  querying: async () => {
    const s = await explorer();
    await said(s, {querying: true});
    // The strip names a query once it has run for a quarter of a second.
    await wait(320);
    await settled();
  },
  basemap: () => explorer('basemap="world"'),
  density: () => explorer('density="smooth"'),
  'range-invalid': async () => {
    const s = await explorer(FIELDS);
    // The compact panel folds its cards; open the number's.
    if (fieldCard(s, 'population').querySelector('[part="range"]') === null) await s.press(fieldCard(s, 'population').querySelector('[part="fold"]'));
    await s.press(fieldCard(s, 'population').querySelector('[part="range"]'));
    const from = fieldCard(s, 'population').querySelector<HTMLInputElement>('[part="brush-from"]')!;
    from.value = '1,5';
    for (const type of ['input', 'change']) from.dispatchEvent(new Event(type, {bubbles: true, composed: true}));
    await settled();
  },
  'region-counted': async () => {
    const s = await explorer();
    // A region is drawn, and the query that counts it runs and then finishes.
    s.store.set('region', {...REGION, status: 'loading'});
    await said(s, {querying: true});
    s.store.set('region', REGION);
    s.store.set('view', {...s.store.get('view'), inView: {...COUNTS, visible: REGION.visible!, matched: REGION.matched}});
    await said(s, {});
  }
};

/** The states built again at the width of the explorer's compact form, where its panel folds its cards. */
const COMPACT = ['rest', 'fields', 'filtered', 'layers', 'item', 'region', 'refused', 'range-invalid'];

/** Each state of the explorer's narrow form, by name: its bar of buttons, and each sheet open. */
const NARROW: Record<string, () => Promise<unknown>> = {
  narrow: () => explorer(),
  'sheet-fields': async () => {
    const s = await explorer(FIELDS);
    await filtered(s);
    await sheetOf(s, 'filters');
    // The sheet's panel folds its cards; open the category's.
    if (fieldCard(s, 'feature_class').querySelector('[part~="row"]') === null) await s.press(fieldCard(s, 'feature_class').querySelector('[part="fold"]'));
  },
  'sheet-histogram': async () => {
    const s = await explorer(FIELDS);
    await sheetOf(s, 'filters');
    if (fieldCard(s, 'population').querySelector('[part="plot"]') === null) await s.press(fieldCard(s, 'population').querySelector('[part="fold"]'));
  },
  'sheet-layers': async () => {
    const s = await explorer();
    await coloured(s);
    await sheetOf(s, 'layers');
  },
  'sheet-menu': async () => {
    const s = await explorer();
    await coloured(s);
    await sheetOf(s, 'layers');
    await s.press(s.part('palette'));
  },
  'sheet-item': async () => {
    const s = await explorer();
    s.store.set('region', REGION);
    await picked(s);
  },
  'narrow-refused': async () => said(await explorer(), refused),
  'narrow-stale': async () => said(await explorer(), {stale: true})
};

const LONG = 'Neoplasms of the connective and soft tissue, including those of uncertain behaviour and unspecified site';
const browseRow = (id: bigint, name: string, childCount = 0): BrowseRow => ({mosaicaId: id, key: `k${id}`, name, maskedCount: 27_000n, matchedCount: 1234n, rung: 0, parentIds: [], childCount, slot: null});

/** A store for the elements on their own: a layer with a filter and a highlight on one cluster, and a region. */
function looseStore(): FakeStore {
  const store = fakeStore({meta: META, status: status({})});
  store.set('view', {...store.get('view'), id: 'places'});
  store.set('artifacts', {...artifactsProjection([artifact(1n, 'Cork', null)]), palette: 'kelly'});
  store.set('filters', {...store.get('filters'), members: [{layer: 'counties', artifact: 1n, outside: false, verb: 'filter'}, {layer: 'counties', artifact: 1n, outside: false, verb: 'highlight'}]});
  store.set('region', REGION);
  store.setBrowse('roots', {artifacts: [browseRow(1n, LONG, 2), browseRow(2n, 'Anatomy')], parents: [], next: null});
  store.setBrowse('p:1', {artifacts: [browseRow(11n, 'Sarcoma'), browseRow(12n, 'Cysts', 1)], parents: [], next: 'more'});
  return store;
}

/** The elements the explorer does not draw, each 320 px wide: a hierarchy open on a row, a selection, and the pickers. */
async function elements(): Promise<void> {
  stageEl.className = 'loose';
  stageEl.innerHTML = `<div style="width: 320px"><mosaica-hierarchy></mosaica-hierarchy></div>
    <div style="width: 320px"><mosaica-selection></mosaica-selection></div>
    <div style="width: 320px"><mosaica-layer-picker></mosaica-layer-picker></div>
    <div style="width: 320px"><mosaica-view-picker></mosaica-view-picker><mosaica-key-picker></mosaica-key-picker></div>`;
  const store = looseStore();
  // The key picker steps through a group of views, which the explorer's own bundle does not have.
  const quarters = ['2026-Q1', '2026-Q2', '2026-Q3'].map((key) => view(`quarter:${key}`, key, {group: 'quarter', key, metadata: {}}));
  const grouped = fakeStore({meta: meta({views: [view('places', 'By place'), ...quarters], groups: [{name: 'quarter', title: 'Quarter', membersOf: null, views: quarters.map((v) => v.id)}]}), status: status({})});
  grouped.set('view', {...grouped.get('view'), id: 'quarter:2026-Q2'});
  for (const el of stageEl.querySelectorAll('mosaica-hierarchy, mosaica-selection, mosaica-layer-picker')) (el as unknown as {store: unknown}).store = store;
  for (const el of stageEl.querySelectorAll('mosaica-view-picker, mosaica-key-picker')) (el as unknown as {store: unknown}).store = grouped;
  await settled();
  await settled();
  (stageEl.querySelector('mosaica-hierarchy')!.shadowRoot!.querySelector('[part="expander"]') as HTMLButtonElement).click();
  await settled();
  await settled();
}

/** The colour editor open over a 22-colour palette, with a row ticked and the picker open on it. */
async function colourEditor(): Promise<void> {
  stageEl.className = 'loose';
  stageEl.innerHTML = '<mosaica-colour-editor field="cluster:counties"></mosaica-colour-editor>';
  const store = looseStore();
  const editor = stageEl.querySelector('mosaica-colour-editor') as HTMLElement & {store: unknown; show(): Promise<void>};
  editor.store = store;
  await settled();
  await editor.show();
  await settled();
  answer(store);
  await settled();
  (editor.shadowRoot!.querySelector('[part="check"]') as HTMLInputElement).click();
  await settled();
  (editor.shadowRoot!.querySelector('[part="swatch"]') as HTMLButtonElement).click();
  await settled();
}

/** A hierarchy 320 px wide with focus on a row's name, which shows the row's buttons in place of its counts. */
async function hierarchyRow(): Promise<void> {
  stageEl.className = 'loose';
  stageEl.innerHTML = '<div style="width: 320px"><mosaica-hierarchy></mosaica-hierarchy></div>';
  const hierarchy = stageEl.querySelector('mosaica-hierarchy') as HTMLElement & {store: unknown};
  hierarchy.store = looseStore();
  await settled();
  await settled();
  hierarchy.shadowRoot!.querySelector<HTMLElement>('[part="row"][data-id="2"] [part="name"]')!.focus();
  await settled();
}

/** A filter panel that shows chips alone, after the × of its last chip is pressed. */
async function noFilters(): Promise<void> {
  stageEl.className = 'loose';
  stageEl.innerHTML = '<div style="width: 320px"><mosaica-filter-panel chips-only></mosaica-filter-panel></div>';
  const store = fakeStore({meta: META, status: status({})});
  store.set('view', {...store.get('view'), id: 'places'});
  const none = store.get('filters');
  store.set('filters', {...none, draft: {filter: {feature_class: {family: 'category', keys: ['P']}}, highlight: {}}});
  const panel = stageEl.querySelector('mosaica-filter-panel') as HTMLElement & {store: unknown};
  panel.store = store;
  await settled();
  const remove = panel.shadowRoot!.querySelector<HTMLElement>('[part="chip"] > button:last-child')!;
  remove.focus();
  remove.click();
  // The store here applies nothing itself: publish the filters the press asked for.
  store.set('filters', none);
  await settled();
}

const LOOSE: Record<string, () => Promise<unknown>> = {elements, 'hierarchy-row': hierarchyRow, 'no-filters': noFilters, 'colour-editor': colourEditor};
const SCENES: Record<string, () => Promise<unknown>> = {...WIDE, ...NARROW, ...LOOSE};

/** The elements under the stage that match `selector`, through shadow roots. */
function within(selector: string, root: ParentNode = stageEl): Element[] {
  return [...root.querySelectorAll('*')].flatMap((el) => [...(el.matches(selector) ? [el] : []), ...(el.shadowRoot ? within(selector, el.shadowRoot) : [])]);
}

const strips = () => within('mosaica-status');

/** The element an axe target names: selectors, each after the first inside the last one's shadow root. */
function at(path: string[]): Element | null {
  let el: Element | null = document.querySelector(path[0]!);
  for (const selector of path.slice(1)) el = el?.shadowRoot?.querySelector(selector) ?? null;
  return el;
}

Object.assign(window, {
  scenes: {wide: Object.keys(WIDE), compact: COMPACT, narrow: Object.keys(NARROW), loose: Object.keys(LOOSE)},
  /** Build the named state from nothing. */
  async show(name: string): Promise<void> {
    const scene = SCENES[name];
    if (!scene) throw new Error(`no scene ${name}`);
    spaceText(false);
    stageEl.innerHTML = '';
    await scene();
    await settled();
  },
  checks: {
    /** The text that is cut off and the elements that scroll sideways. */
    clipped: () => [...clippedText(stageEl), ...wideContent(stageEl)],
    async spaced(on: boolean): Promise<void> {
      spaceText(on);
      await settled();
    },
    targets: () => crowdedTargets(stageEl),
    /** Begin a walk with Tab: from where the focus is if the state put it under the stage, else from the button before the stage. */
    startWalk() {
      forgetStops();
      const stop = focusStop(stageEl);
      if (!stop) document.querySelector<HTMLElement>('#before')!.focus();
      return stop;
    },
    stop: () => focusStop(stageEl),
    /**
     * What is wrong with the contrast of each element an axe target names, of every piece of text
     * drawn over a map and of every piece of text on a status strip.
     */
    contrast(paths: string[][]): string[] {
      const named = paths.map((path) => at(path) ?? path.join(' >> '));
      const drawn = [...within('mosaica-map'), ...strips()].flatMap((el) => textElements(el.shadowRoot!));
      const faults = new Set<string>();
      for (const el of new Set([...named, ...drawn])) {
        // The Layers popover's background images are the shades at its top and bottom edges while it scrolls.
        const fault = typeof el === 'string' ? `nothing at ${el}` : contrastFault(el, '[part="layers-popover"]');
        if (fault) faults.add(fault);
      }
      return [...faults].sort();
    },
    /** What the status strips' cells for figures that wait for an answer hold, as text. */
    waiting: () => strips().flatMap((strip) => [...strip.shadowRoot!.querySelectorAll('.cell.dim')].map((cell) => deepText(cell).replace(/\s+/g, ' ').trim())),
    /** The height in pixels of each status strip that is drawn. */
    stripHeights: () => strips().map((strip) => strip.shadowRoot!.querySelector('[part="strip"]')!.getBoundingClientRect().height).filter((h) => h > 0),
    /** How many live regions under the stage hold `text`. */
    saying: (text: string) => within('[role="status"], [role="alert"], [aria-live]').filter((el) => deepText(el).includes(text)).length
  },
  ready: true
});
