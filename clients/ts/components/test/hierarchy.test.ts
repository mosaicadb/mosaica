import {afterEach, describe, expect, it, vi} from 'vitest';
import type {BrowseRow, Layer, Meta} from '@mosaicajs/client';
import '../src/hierarchy.js';
import type {MosaicaHierarchy} from '../src/hierarchy.js';
import {UNNAMED} from '../src/base.js';
import {deep, deepAll, deepText, fakeStore, mount, settle, status} from './fake-store.js';

/**
 * `<mosaica-hierarchy>` over `POST /v1/artifacts/browse`: the roots without a viewport, lazy
 * children, "also under", the two counts under a filter, and the click that highlights.
 */

afterEach(() => {
  document.body.innerHTML = '';
  vi.useRealTimers();
});

const layer = (name: string, kind: Layer['hierarchy']['kind'], computedContent: string[]): Layer =>
  ({
    name,
    title: name,
    views: ['s0'],
    hierarchy: {kind, pruneChildren: false},
    levels: [],
    computedContent,
    shape: computedContent.includes('hull') ? 'derived' : null,
    suppliedContent: ['name'],
    depsOn: [],
    version: 1
  }) as unknown as Layer;

const META = {
  apiVersion: 1,
  views: [{id: 's0', displayName: 'default', quantisation: {xMin: 0, xMax: 1, yMin: 0, yMax: 1}, projection: 'none', worldAspect: null, tileScheme: null, tile: null, roster: null}],
  groups: [],
  declaredScalars: [],
  layers: [layer('clusters/kmeans', 'flat', ['centroid', 'box', 'hull']), layer('mesh/descriptors', 'dag', [])],
  selection: {kMin: 1, kMaxMarks: 500, maxK: 5000, thetaTargetMarks: 10, maxUnderlayOffset: 0, maxCategoryValues: 1000, maxRegionVertices: 10_000, maxRegionCells: 262_144, maxBrowseRows: 200},
  maxTilesPerRequest: 4096,
  filterOperands: []
} as unknown as Meta;

const row = (id: bigint, name: string | null, masked: bigint, extra: Partial<BrowseRow> = {}): BrowseRow => ({
  mosaicaId: id,
  key: `d-${id}`,
  name,
  maskedCount: masked,
  matchedCount: null,
  rung: 0,
  parentIds: [],
  childCount: 0,
  slot: null,
  ...extra
});

async function panel(overrides: Partial<Parameters<typeof fakeStore>[0]> = {}) {
  const host = await mount('<mosaica-hierarchy></mosaica-hierarchy>');
  const el = host.querySelector('mosaica-hierarchy') as MosaicaHierarchy;
  const store = fakeStore({meta: META, status: status({}), ...overrides});
  store.setBrowse('roots', {artifacts: [row(1n, 'Neoplasms', 27_000_000n, {childCount: 2}), row(2n, 'Anatomy', 3_400n, {childCount: 1})], parents: [], next: 'p2'});
  el.store = store;
  await settle(host);
  await settle(host);
  return {host, el, store};
}

describe('<mosaica-hierarchy>', () => {
  it('offers only the layers with a lineage, and opens on the roots with no viewport', async () => {
    const {host, store} = await panel();
    // `clusters/kmeans` is flat and declares no levels: one page of roots and no row has children,
    // so it is not a tree to browse and the panel does not offer it one — which leaves one layer,
    // and one layer draws no picker at all.
    expect(deep(host, '[part="layer"]')).toBeNull();
    expect(deepAll(host, '[part="row"] [part="name"]').map((n) => n.textContent?.trim())).toEqual(['Neoplasms', 'Anatomy']);
    // The request carried the layer and a limit and nothing about where the map is looking.
    const asked = store.calls.filter((c) => c.name === 'browse').map((c) => c.args[0] as Record<string, unknown>);
    expect(asked[0]).toEqual({layer: 'mesh/descriptors', limit: 50});
  });

  it('asks for a tiered layer’s roots with no level, so a root at any level is listed', async () => {
    const tiered = {...layer('taxonomy', 'tiered', []), levels: [{level: 0, title: 'Archive', zoom: null}, {level: 1, title: 'Subject class', zoom: null}]} as Layer;
    const {host, store} = await panel({meta: {...META, layers: [tiered]}});
    store.setBrowse('roots', {artifacts: [row(1n, 'Mathematics', 900n, {childCount: 30}), row(9n, 'q-bio.NC', 40n, {rung: 1})], parents: [], next: null});
    store.set('filters', {...store.get('filters'), expr: {archive: {in: ['math']}}});
    await settle(host);
    await settle(host);
    const asked = store.calls.filter((c) => c.name === 'browse').map((c) => c.args[0] as Record<string, unknown>);
    expect(asked.every((r) => !('level' in r))).toBe(true);
    expect(deepAll(host, '[part="row"] [part="name"]').map((n) => n.textContent?.trim())).toEqual(['Mathematics', 'q-bio.NC']);
  });

  it('marks a row with no name as unnamed and never draws its key in the name’s place', async () => {
    const host = await mount('<mosaica-hierarchy></mosaica-hierarchy>');
    const el = host.querySelector('mosaica-hierarchy') as MosaicaHierarchy;
    const store = fakeStore({meta: META, status: status({})});
    store.setBrowse('roots', {artifacts: [row(3n, null, 10n)], parents: [], next: null});
    el.store = store;
    await settle(host);
    await settle(host);
    const name = deep(host, '[part="row"] [part="name"]')!;
    expect(name.hasAttribute('data-unnamed')).toBe(true);
    expect(name.textContent).not.toContain('d-3');
  });

  it('names a row by the name the server serves it, and marks a row with none, or an empty one, unnamed', async () => {
    const host = await mount('<mosaica-hierarchy></mosaica-hierarchy>');
    const el = host.querySelector('mosaica-hierarchy') as MosaicaHierarchy;
    const store = fakeStore({meta: META, status: status({})});
    store.setBrowse('roots', {artifacts: [row(3n, 'spin magnetic effect', 10n), row(4n, null, 8n), row(5n, '', 6n)], parents: [], next: null});
    el.store = store;
    await settle(host);
    await settle(host);
    const names = deepAll(host, '[part="row"] [part="name"]');
    expect(names.map((n) => n.textContent?.trim())).toEqual(['spin magnetic effect', UNNAMED, UNNAMED]);
    expect(names.map((n) => n.hasAttribute('data-unnamed'))).toEqual([false, true, true]);
  });

  it('offers to expand only a row the server counts children under, and keeps every name in line', async () => {
    const {host, store} = await panel();
    store.setBrowse('p:1', {artifacts: [row(11n, 'Cysts', 900n), row(12n, 'Tumours', 800n, {childCount: 4})], parents: [], next: null});
    (deep(host, '[part="row"][data-id="1"] [part="expander"]') as HTMLButtonElement).click();
    await settle(host);
    await settle(host);
    const leafAt = (id: string) => deep(host, `[part="row"][data-id="${id}"] [part="expander"]`)!.hasAttribute('data-leaf');
    expect(['1', '11', '12', '2'].map(leafAt)).toEqual([false, true, false, false]);
  });

  it('fetches a node’s children on expansion, and pages them under More', async () => {
    const {host, el, store} = await panel();
    store.setBrowse('p:1', {artifacts: [row(11n, 'Cysts', 900n)], parents: [], next: 'c2'});
    store.setBrowse('p:1:c2', {artifacts: [row(12n, 'Hamartoma', 40n)], parents: [], next: null});
    (deep(host, '[part="expander"]') as HTMLButtonElement).click();
    await settle(host);
    await settle(host);
    expect(deepAll(host, '[part="row"] [part="name"]').map((n) => n.textContent?.trim())).toEqual(['Neoplasms', 'Cysts', 'Anatomy']);
    (deepAll(host, '[part="more"]')[0] as HTMLButtonElement).click();
    await settle(host);
    await settle(host);
    expect(deepAll(host, '[part="row"] [part="name"]').map((n) => n.textContent?.trim())).toEqual(['Neoplasms', 'Cysts', 'Hamartoma', 'Anatomy']);
    void el;
  });

  it('says *also under* for a dag node served beneath several parents', async () => {
    const {host, store} = await panel();
    store.setBrowse('p:1', {artifacts: [row(11n, 'Cysts', 900n, {parentIds: [1n, 2n]})], parents: [], next: null});
    (deep(host, '[part="expander"]') as HTMLButtonElement).click();
    await settle(host);
    await settle(host);
    // The parent it is drawn under is not repeated, and the other is named rather than numbered:
    // the walk is the only place a name for one of these artifacts exists on this client.
    expect(deepText(deep(host, '[part="also"]'))).toContain('Anatomy');
  });

  it('marks a parent the walk has no name for as unnamed in *also under*, never by its id', async () => {
    const {host, store} = await panel();
    store.setBrowse('p:1', {artifacts: [row(11n, 'Cysts', 900n, {parentIds: [1n, 77n]})], parents: [], next: null});
    (deep(host, '[part="expander"]') as HTMLButtonElement).click();
    await settle(host);
    await settle(host);
    const also = deepText(deep(host, '[part="also"]'));
    expect(also).toContain(UNNAMED);
    expect(also).not.toContain('77');
  });

  it('a click is a highlight, and the actions carry the filter beside it', async () => {
    const {host, store} = await panel();
    (deep(host, '[part="row"] [part="name"]') as HTMLButtonElement).click();
    const sent = store.calls.find((c) => c.name === 'setMembers')!.args[0] as {layer: string; artifact: bigint; verb: string; outside: boolean}[];
    // The label rides the clause: nothing downstream can resolve the identifier, the artifacts of
    // a filter layer never being served.
    expect(sent).toEqual([{layer: 'mesh/descriptors', artifact: 1n, outside: false, verb: 'highlight', label: 'Neoplasms'}]);
    // And *fit* is absent on a filter layer — its artifacts are spread and there is nothing to fit.
    expect(deep(host, '[part="fit"]')).toBeNull();
    expect(deep(host, '[part="filter"]')).not.toBeNull();
  });

  it('holds a filter and a highlight on one row at once, and withdraws each on its own', async () => {
    const both = [
      {layer: 'mesh/descriptors', artifact: 1n, outside: false, verb: 'filter' as const},
      {layer: 'mesh/descriptors', artifact: 1n, outside: false, verb: 'highlight' as const}
    ];
    const {host, store} = await panel();
    store.set('filters', {...store.get('filters'), members: both.slice(0, 1)});
    await settle(host);
    (deep(host, '[part="row"] [part="highlight"]') as HTMLButtonElement).click();
    const added = store.calls.filter((c) => c.name === 'setMembers').at(-1)!.args[0] as {verb: string}[];
    expect(added.map((c) => c.verb)).toEqual(['filter', 'highlight']);

    store.set('filters', {...store.get('filters'), members: both});
    await settle(host);
    const first = deep(host, '[part="row"]')!;
    expect([first.querySelector('[part="filter"]')!.getAttribute('aria-pressed'), first.querySelector('[part="highlight"]')!.getAttribute('aria-pressed')]).toEqual(['true', 'true']);
    expect(first.getAttribute('data-clause')!.split(' ').sort()).toEqual(['filter', 'highlight']);
    (first.querySelector('[part="filter"]') as HTMLButtonElement).click();
    const left = store.calls.filter((c) => c.name === 'setMembers').at(-1)!.args[0] as {verb: string}[];
    expect(left.map((c) => c.verb)).toEqual(['highlight']);
  });

  it('carries a × per clause on its row, which takes that clause off', async () => {
    const {host, store} = await panel();
    expect(deep(host, '[part="dismiss"]')).toBeNull();
    store.set('filters', {
      ...store.get('filters'),
      members: [
        {layer: 'mesh/descriptors', artifact: 1n, outside: false, verb: 'filter'},
        {layer: 'mesh/descriptors', artifact: 1n, outside: false, verb: 'highlight'}
      ]
    });
    await settle(host);
    const row = deep(host, '[part="row"]')!;
    const dismiss = [...row.querySelectorAll<HTMLButtonElement>('[part="dismiss"]')];
    expect(dismiss.map((b) => b.getAttribute('data-verb'))).toEqual(['filter', 'highlight']);
    dismiss[1]!.click();
    const left = store.calls.filter((c) => c.name === 'setMembers').at(-1)!.args[0] as {verb: string}[];
    expect(left.map((c) => c.verb)).toEqual(['filter']);
  });

  /**
   * The panel's counts answer the question the request carried, so the walk has to be dropped when
   * that question moves — and the question is `requestFilters()`, which carries the **drawn
   * region's leaf** as well as the controls and the clauses. Hashed from `filters.expr` alone, a
   * region drawn on the map left the tree showing counts to the question before it, with nothing
   * on screen saying so.
   */
  it('refetches when the drawn region changes, which the controls alone never say', async () => {
    const {host, store} = await panel();
    const before = store.calls.filter((c) => c.name === 'browse').length;
    expect(before).toBeGreaterThan(0);

    store.set('region', {
      shape: {kind: 'box', bbox: [0, 0, 1, 1], outside: false},
      status: 'shown', refusal: null, visible: null, matched: {value: 4, exact: true},
      served: {shown: 4, total: 4, exact: true}, held: {shown: 4, total: 4, exact: true}
    } as never);
    await settle(host);
    await settle(host);
    expect(store.calls.filter((c) => c.name === 'browse').length).toBeGreaterThan(before);
  });

  it('does not refetch when nothing about the question moved', async () => {
    const {host, store} = await panel();
    const before = store.calls.filter((c) => c.name === 'browse').length;
    // A store tick that changes nothing the request carries — the panel is told, and asks nothing.
    store.set('status', {status: 'shown', sessionWarm: true, refusal: null, stale: false, expired: false, retrying: false, unreachable: false, querying: false});
    await settle(host);
    await settle(host);
    expect(store.calls.filter((c) => c.name === 'browse').length).toBe(before);
  });

  it('shows the matched count beside the masked one where the map carries a filter', async () => {
    const {host} = await panel({
      filters: {draft: {filter: {}, highlight: {}}, expr: {archive: {in: ['cs']}}, highlight: null, members: [], regions: [], suggestions: {}, suggestErrors: {}, suggestEpoch: 0}
    });
    // Existence and the masked count never move with the filter; the second figure is what does.
    expect(deep(host, '[part="count-masked"]')).not.toBeNull();
  });

  it('drives the search form and draws its matches as rows', async () => {
    const {host, store} = await panel();
    store.setBrowse('q:lymph', {artifacts: [row(7n, 'Lymphocytes', 400_000n)], parents: [], next: null});
    const box = deep(host, '[part="search"] input') as HTMLInputElement;
    box.value = 'lymph';
    vi.useFakeTimers();
    box.dispatchEvent(new Event('input'));
    await vi.runAllTimersAsync();
    vi.useRealTimers();
    await settle(host);
    await settle(host);
    expect(deepAll(host, '[part="row"] [part="name"]').map((n) => n.textContent?.trim())).toEqual(['Lymphocytes']);
  });

  it('a panel that is not being shown asks for nothing, and browses when it is', async () => {
    // The roots are the widest request this panel makes; a hidden panel should not send it
    // alongside the first viewport.
    const host = await mount('<mosaica-hierarchy style="display:none"></mosaica-hierarchy>');
    const el = host.querySelector('mosaica-hierarchy') as MosaicaHierarchy;
    const store = fakeStore({meta: META, status: status({})});
    store.setBrowse('roots', {artifacts: [row(1n, 'Neoplasms', 27_000_000n)], parents: [], next: null});
    el.store = store;
    await settle(host);
    await settle(host);
    expect(store.calls.filter((c) => c.name === 'browse')).toHaveLength(0);

    el.style.display = '';
    el.requestUpdate();
    await settle(host);
    await settle(host);
    expect(store.calls.filter((c) => c.name === 'browse')).toHaveLength(1);
    expect(deepAll(host, '[part="row"] [part="name"]').map((n) => n.textContent?.trim())).toEqual(['Neoplasms']);
  });

});

describe('<mosaica-hierarchy> refused', () => {
  it('carries the refusal’s code and says nothing more about the empty tree', async () => {
    const host = await mount('<mosaica-hierarchy></mosaica-hierarchy>');
    const el = host.querySelector('mosaica-hierarchy') as MosaicaHierarchy;
    const store = fakeStore({meta: META, status: status({})});
    store.browse = () => Promise.reject({code: 'layer-withheld', detail: ''});
    el.store = store;
    await settle(host);
    await settle(host);
    expect(deep(host, '[part="refusal"]')?.getAttribute('data-code')).toBe('layer-withheld');
    expect(deep(host, '[part="state"][data-state="empty"]')).toBeNull();
  });
});

describe('<mosaica-hierarchy> across a change of viewer', () => {
  it('drops the roots and names it fetched when the store’s meta goes null, and asks again', async () => {
    const {host, store} = await panel();
    expect(deepAll(host, '[part="row"] [part="name"]').map((n) => n.textContent?.trim())).toEqual(['Neoplasms', 'Anatomy']);
    const asked = store.calls.filter((c) => c.name === 'browse').length;

    // A clear or a new identity key: meta goes, and returns for the next viewer.
    store.set('meta', null);
    await settle(host);
    expect(deepAll(host, '[part="row"]')).toEqual([]);
    store.setBrowse('roots', {artifacts: [row(9n, 'Other', 12n)], parents: [], next: null});
    store.set('meta', META);
    await settle(host);
    await settle(host);
    expect(store.calls.filter((c) => c.name === 'browse').length).toBeGreaterThan(asked);
    expect(deepAll(host, '[part="row"] [part="name"]').map((n) => n.textContent?.trim())).toEqual(['Other']);
  });

  it('shows the adopted store’s roots, not the previous store’s, under the same layer and filters', async () => {
    const {host, el} = await panel();
    const next = fakeStore({meta: META, status: status({})});
    next.setBrowse('roots', {artifacts: [row(9n, 'Other', 12n)], parents: [], next: null});
    el.store = next;
    await settle(host);
    await settle(host);
    expect(next.calls.filter((c) => c.name === 'browse')).toHaveLength(1);
    expect(deepAll(host, '[part="row"] [part="name"]').map((n) => n.textContent?.trim())).toEqual(['Other']);
  });
});

describe('<mosaica-hierarchy> under a keyboard and a screen reader', () => {
  const active = (el: MosaicaHierarchy) => el.shadowRoot!.activeElement as HTMLElement | null;
  const control = (host: HTMLElement, id: string, part: string) => deep(host, `[part="row"][data-id="${id}"] [part="${part}"]`) as HTMLButtonElement;
  const names = (host: HTMLElement) => deepAll(host, '[part="row"] [part="name"]').map((n) => n.textContent?.trim());
  /** Put these clauses on row 11, as the store does once a row's button is pressed. */
  const clausesOn = (store: Awaited<ReturnType<typeof panel>>['store'], ...verbs: ('filter' | 'highlight')[]) =>
    store.set('filters', {...store.get('filters'), members: verbs.map((verb) => ({layer: 'mesh/descriptors', artifact: 11n, outside: false, verb}))});
  const type = async (host: HTMLElement, text: string) => {
    const box = deep(host, '[part="search"] input') as HTMLInputElement;
    box.value = text;
    vi.useFakeTimers();
    box.dispatchEvent(new Event('input'));
    await vi.runAllTimersAsync();
    vi.useRealTimers();
    await settle(host);
  };

  /** The panel with its first root open on two children. */
  async function walked() {
    const p = await panel();
    p.store.setBrowse('p:1', {artifacts: [row(11n, 'Cysts', 900n), row(12n, 'Tumours', 800n)], parents: [], next: null});
    control(p.host, '1', 'expander').click();
    await settle(p.host);
    await settle(p.host);
    return p;
  }

  it('keeps the open rows and the focused button when a row’s Filter refetches the tree', async () => {
    const {host, el, store} = await walked();
    control(host, '11', 'filter').focus();
    const asked = store.calls.filter((c) => c.name === 'browse').length;
    clausesOn(store, 'filter');
    await settle(host);
    await settle(host);
    // The roots and the open row's children were both asked for again.
    expect(store.calls.filter((c) => c.name === 'browse').slice(asked).map((c) => (c.args[0] as {parent?: bigint}).parent)).toEqual([undefined, 1n]);
    expect(names(host)).toEqual(['Neoplasms', 'Cysts', 'Tumours', 'Anatomy']);
    expect(control(host, '1', 'expander').getAttribute('aria-expanded')).toBe('true');
    expect(active(el)).toBe(control(host, '11', 'filter'));
    expect(active(el)!.getAttribute('aria-pressed')).toBe('true');
  });

  it('follows the focused row where the refetched tree lists it elsewhere, and falls to a row beside it where it is gone', async () => {
    const {host, el, store} = await walked();
    control(host, '12', 'name').focus();
    store.setBrowse('p:1', {artifacts: [row(12n, 'Tumours', 800n), row(11n, 'Cysts', 900n)], parents: [], next: null});
    clausesOn(store, 'filter');
    await settle(host);
    await settle(host);
    expect(names(host)).toEqual(['Neoplasms', 'Tumours', 'Cysts', 'Anatomy']);
    expect(active(el)).toBe(control(host, '12', 'name'));

    store.setBrowse('p:1', {artifacts: [row(11n, 'Cysts', 900n)], parents: [], next: null});
    clausesOn(store);
    await settle(host);
    await settle(host);
    expect(names(host)).toEqual(['Neoplasms', 'Cysts', 'Anatomy']);
    expect(active(el)?.getAttribute('part')).toBe('name');
    expect(active(el)?.closest('[part="row"]')).not.toBeNull();
  });

  it('says a row is expanded only once its children are shown, and that it is loading until then', async () => {
    const {host, store} = await panel();
    let land: (page: {artifacts: BrowseRow[]; parents: []; next: null}) => void = () => {};
    const roots = store.browse;
    store.browse = ((req: {parent?: bigint}) => (req.parent === undefined ? roots(req as never) : new Promise((resolve) => (land = resolve)))) as typeof store.browse;
    const expander = control(host, '1', 'expander');
    expander.click();
    await settle(host);
    const region = deep(host, '[role="status"]')!;
    expect(expander.getAttribute('aria-expanded')).toBe('false');
    expect(expander.querySelector('.spinner')).not.toBeNull();
    expect(region.textContent).toContain('Neoplasms');
    land({artifacts: [row(11n, 'Cysts', 900n)], parents: [], next: null});
    await settle(host);
    await settle(host);
    expect(expander.getAttribute('aria-expanded')).toBe('true');
    expect(expander.querySelector('.spinner')).toBeNull();
    expect(region.textContent).not.toContain('Neoplasms');
    expect(names(host)).toEqual(['Neoplasms', 'Cysts', 'Anatomy']);
  });

  it('names each row button by its row, the tree by its layer, and says which rows are highlighted', async () => {
    const {host, store} = await panel({meta: {...META, layers: [layer('clusters/kmeans', 'nested', ['centroid', 'box', 'hull'])]} as Meta});
    expect(deep(host, '[part="tree"]')!.getAttribute('aria-label')).toContain('clusters/kmeans');
    for (const part of ['name', 'highlight', 'filter', 'fit']) {
      expect(control(host, '1', part).getAttribute('aria-label'), part).toContain('Neoplasms');
      expect(control(host, '2', part).getAttribute('aria-label'), part).toContain('Anatomy');
    }
    expect(new Set(['highlight', 'filter', 'fit'].map((part) => control(host, '1', part).getAttribute('aria-label'))).size).toBe(3);
    expect(control(host, '1', 'name').getAttribute('aria-pressed')).toBe('false');
    store.set('filters', {
      ...store.get('filters'),
      members: [
        {layer: 'clusters/kmeans', artifact: 1n, outside: false, verb: 'highlight'},
        {layer: 'clusters/kmeans', artifact: 1n, outside: false, verb: 'filter'}
      ]
    });
    await settle(host);
    await settle(host);
    expect(control(host, '1', 'name').getAttribute('aria-pressed')).toBe('true');
    expect(control(host, '2', 'name').getAttribute('aria-pressed')).toBe('false');
    // The two removing buttons differ by name and by their verb, which the highlight's bar is
    // drawn from; the browser suite checks the bar.
    const dismiss = deepAll(host, '[part="row"][data-id="1"] [part="dismiss"]');
    expect(dismiss.every((b) => b.getAttribute('aria-label')!.includes('Neoplasms'))).toBe(true);
    expect(new Set(dismiss.map((b) => b.getAttribute('aria-label'))).size).toBe(2);
    expect(dismiss.map((b) => b.getAttribute('data-verb')).sort()).toEqual(['filter', 'highlight']);
  });

  it('moves focus from a × that removed itself to the row’s other ×, then to the button that makes the clause', async () => {
    const {host, el, store} = await walked();
    clausesOn(store, 'filter', 'highlight');
    await settle(host);
    await settle(host);
    const dismiss = (verb: string) => deep(host, `[part="row"][data-id="11"] [part="dismiss"][data-verb="${verb}"]`) as HTMLButtonElement;
    dismiss('highlight').focus();
    clausesOn(store, 'filter');
    await settle(host);
    await settle(host);
    expect(active(el)).toBe(dismiss('filter'));
    clausesOn(store);
    await settle(host);
    await settle(host);
    expect(active(el)).toBe(control(host, '11', 'filter'));
  });

  it('moves focus from More to the first row its page added', async () => {
    const {host, el, store} = await panel();
    store.setBrowse('roots:p2', {artifacts: [row(3n, 'Chemicals', 5n), row(4n, 'Organisms', 4n)], parents: [], next: null});
    const more = deep(host, '[part="more"]') as HTMLButtonElement;
    more.focus();
    more.click();
    await settle(host);
    await settle(host);
    expect(deep(host, '[part="more"]')).toBeNull();
    expect(active(el)).toBe(control(host, '3', 'name'));
  });

  it('leaves focus where it has gone while a page More asked for, or a refetch, was on its way', async () => {
    const {host, el, store} = await panel();
    const elsewhere = document.createElement('input');
    document.body.append(elsewhere);
    store.setBrowse('roots:p2', {artifacts: [row(3n, 'Chemicals', 5n)], parents: [], next: null});
    const more = deep(host, '[part="more"]') as HTMLButtonElement;
    more.focus();
    more.click();
    elsewhere.focus();
    await settle(host);
    await settle(host);
    expect(names(host)).toEqual(['Neoplasms', 'Anatomy', 'Chemicals']);
    expect(document.activeElement).toBe(elsewhere);
    expect(active(el)).toBeNull();

    // A row's Filter pressed, then focus taken elsewhere before the refetched rows land.
    store.setBrowse('p:1', {artifacts: [row(11n, 'Cysts', 900n)], parents: [], next: null});
    control(host, '1', 'expander').click();
    await settle(host);
    await settle(host);
    control(host, '11', 'filter').focus();
    clausesOn(store, 'filter');
    elsewhere.focus();
    await settle(host);
    await settle(host);
    expect(names(host)).toEqual(['Neoplasms', 'Cysts', 'Anatomy', 'Chemicals']);
    expect(document.activeElement).toBe(elsewhere);
  });

  it('puts a row’s buttons away on Escape, wherever focus is in the row, until focus leaves it', async () => {
    const {host, el} = await walked();
    const rowOf = (id: string) => deep(host, `[part="row"][data-id="${id}"]`) as HTMLElement;
    const escape = () => document.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true}));
    control(host, '11', 'filter').focus();
    escape();
    await settle(host);
    expect(rowOf('11').hasAttribute('data-buttons-away')).toBe(true);
    expect(rowOf('12').hasAttribute('data-buttons-away')).toBe(false);
    // Focus was on a button that is put away, so it goes to the row's name.
    expect(active(el)).toBe(control(host, '11', 'name'));
    // Tab on to the buttons brings them back.
    control(host, '11', 'highlight').focus();
    await settle(host);
    expect(rowOf('11').hasAttribute('data-buttons-away')).toBe(false);
    escape();
    await settle(host);
    expect(rowOf('11').hasAttribute('data-buttons-away')).toBe(true);
    control(host, '12', 'name').focus();
    await settle(host);
    expect(rowOf('11').hasAttribute('data-buttons-away')).toBe(false);
    // With focus in no row, Escape is not the tree's.
    (deep(host, '[part="search"] input') as HTMLElement).focus();
    escape();
    await settle(host);
    expect(deepAll(host, '[part="row"][data-buttons-away]')).toEqual([]);
  });

  it('refetches as many pages as More had loaded, of the roots and of an open row', async () => {
    const {host, store} = await panel();
    store.setBrowse('roots:p2', {artifacts: [row(3n, 'Chemicals', 5n)], parents: [], next: null});
    store.setBrowse('p:1', {artifacts: [row(11n, 'Cysts', 900n)], parents: [], next: 'c2'});
    store.setBrowse('p:1:c2', {artifacts: [row(12n, 'Hamartoma', 40n)], parents: [], next: null});
    control(host, '1', 'expander').click();
    await settle(host);
    await settle(host);
    for (const more of deepAll(host, '[part="more"]')) (more as HTMLButtonElement).click();
    await settle(host);
    await settle(host);
    const walk = ['Neoplasms', 'Cysts', 'Hamartoma', 'Anatomy', 'Chemicals'];
    expect(names(host)).toEqual(walk);
    const asked = store.calls.filter((c) => c.name === 'browse').length;
    clausesOn(store, 'filter');
    await settle(host);
    await settle(host);
    expect(names(host)).toEqual(walk);
    // Each page again, from the first: a cursor is good only under the filters that served it.
    const again = store.calls.filter((c) => c.name === 'browse').slice(asked).map((c) => c.args[0] as {parent?: bigint; cursor?: string});
    expect(again.map((r) => `${r.parent ?? 'roots'}:${r.cursor ?? ''}`).sort()).toEqual(['1:', '1:c2', 'roots:', 'roots:p2']);
  });

  it('asks for a few open rows’ children at a time when it refetches, and for none once the filters have moved on', async () => {
    const ids = [1n, 2n, 3n, 4n, 5n, 6n, 7n, 8n];
    const host = await mount('<mosaica-hierarchy></mosaica-hierarchy>');
    const el = host.querySelector('mosaica-hierarchy') as MosaicaHierarchy;
    const store = fakeStore({meta: META, status: status({})});
    let flying = 0;
    let most = 0;
    const waiting: (() => void)[] = [];
    let hold = false;
    const asked: (bigint | undefined)[] = [];
    store.browse = (async (req: {parent?: bigint}) => {
      asked.push(req.parent);
      flying += 1;
      most = Math.max(most, flying);
      if (hold) await new Promise<void>((go) => waiting.push(go));
      else await Promise.resolve();
      flying -= 1;
      return req.parent === undefined
        ? {artifacts: ids.map((id) => row(id, `Root ${id}`, 10n, {childCount: 1})), parents: [], next: null}
        : {artifacts: [row(req.parent * 100n, `Child of ${req.parent}`, 1n)], parents: [], next: null};
    }) as typeof store.browse;
    el.store = store;
    await settle(host);
    await settle(host);
    for (const id of ids) control(host, String(id), 'expander').click();
    await settle(host);
    await settle(host);
    expect(deepAll(host, '[part="row"]')).toHaveLength(16);

    // A refetch whose requests do not answer until they are let go, one at a time.
    hold = true;
    asked.length = 0;
    most = 0;
    clausesOn(store, 'filter');
    await settle(host);
    waiting.shift()!();
    await settle(host);
    expect(most).toBeLessThan(ids.length);
    const before = asked.length;
    expect(before).toBeLessThan(1 + ids.length);
    // The filters move on: what the first refetch had not asked for yet, it never asks for.
    clausesOn(store, 'filter', 'highlight');
    clausesOn(store);
    await settle(host);
    hold = false;
    while (waiting.length > 0) {
      waiting.shift()!();
      await settle(host);
    }
    await settle(host);
    await settle(host);
    expect(most).toBeLessThan(ids.length);
    // One more walk of the roots and the eight rows, and no more of the first.
    expect(asked.length - before).toBe(1 + ids.length);
    expect(deepAll(host, '[part="row"]')).toHaveLength(16);
  });

  it('opens a row pressed while a refetch is on its way, at that one press', async () => {
    const {host, store} = await walked();
    store.setBrowse('p:2', {artifacts: [row(21n, 'Tissues', 30n)], parents: [], next: null});
    const lands: (() => void)[] = [];
    const answer = store.browse;
    store.browse = ((req: {parent?: bigint}) => new Promise((resolve) => lands.push(() => resolve(answer(req as never))))) as typeof store.browse;
    clausesOn(store, 'filter');
    await settle(host);
    control(host, '2', 'expander').click();
    await settle(host);
    store.browse = answer;
    while (lands.length > 0) {
      lands.shift()!();
      await settle(host);
    }
    await settle(host);
    await settle(host);
    expect(control(host, '2', 'expander').getAttribute('aria-expanded')).toBe('true');
    expect(names(host)).toEqual(['Neoplasms', 'Cysts', 'Tumours', 'Anatomy', 'Tissues']);
  });

  it('says No match, a refused layer and refused children in one status region that is there before them', async () => {
    const {host, store} = await panel();
    // The region holds the state's part, which is hidden while empty, and is not that part.
    const region = deep(host, '[role="status"]')!;
    const state = region.querySelector('[part="state"]')!;
    expect(deepAll(host, '[role="status"]')).toEqual([region]);
    expect(region.textContent?.trim()).toBe('');

    await type(host, 'zzz');
    expect(deep(host, '[role="status"]')).toBe(region);
    expect(region.querySelector('[part="state"]')).toBe(state);
    expect(state.getAttribute('data-state')).toBe('empty');
    expect(state.textContent?.trim()).not.toBe('');

    await type(host, '');
    const roots = store.browse;
    store.browse = ((req: {parent?: bigint}) => (req.parent === undefined ? roots(req as never) : Promise.reject({code: 'layer-withheld', detail: ''}))) as typeof store.browse;
    control(host, '1', 'expander').click();
    await settle(host);
    await settle(host);
    expect(deep(host, '[part="also"][data-code="layer-withheld"]')).not.toBeNull();
    expect(region.textContent).toContain('Neoplasms');

    store.browse = () => Promise.reject({code: 'layer-withheld', detail: ''});
    clausesOn(store, 'filter');
    await settle(host);
    await settle(host);
    expect(state.getAttribute('data-state')).toBe('refused');
    expect(state.querySelector('[part="refusal"]')?.getAttribute('data-code')).toBe('layer-withheld');
  });

  it('loads the roots of a layer chosen in the select', async () => {
    const {host, store} = await panel({meta: {...META, layers: [layer('mesh/descriptors', 'dag', []), layer('topics', 'nested', [])]} as Meta});
    const before = store.calls.filter((c) => c.name === 'browse').length;
    const select = deep(host, '[part="layer"]') as HTMLSelectElement;
    select.value = 'topics';
    select.dispatchEvent(new Event('change'));
    await settle(host);
    await settle(host);
    expect(store.calls.filter((c) => c.name === 'browse').slice(before).map((c) => (c.args[0] as {layer: string}).layer)).toEqual(['topics']);
    expect(names(host)).toEqual(['Neoplasms', 'Anatomy']);
  });
});
