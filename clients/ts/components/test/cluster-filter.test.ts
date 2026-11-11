import {afterEach, describe, expect, it, vi} from 'vitest';
import type {BrowseRow, Meta} from '@mosaicajs/client';
import '../src/cluster-filter.js';
import type {MosaicaClusterFilter} from '../src/cluster-filter.js';
import {aggregateEntry, answerAggregate, deep, deepAll, fakeStore, meta, mount, registered, settle, status} from './fake-store.js';
import {UNNAMED} from '../src/base.js';

/**
 * A layer's clusters as a filter field: what it asks the store (browse pages, the aggregate over the
 * rows it lists), what it offers, and the clause a pick adds.
 */

afterEach(() => {
  document.body.innerHTML = '';
  vi.restoreAllMocks();
});

const layer = (name: string, over: Partial<Meta['layers'][number]> = {}) =>
  ({name, title: 'Topics', views: ['s0'], membership: 'enumerated', hierarchy: {kind: 'nested', pruneChildren: false}, levels: [], computedContent: ['centroid'], shape: null, suppliedContent: ['name'], depsOn: [], version: 1, ...over}) as Meta['layers'][number];

const row = (id: bigint, name: string | null, parentIds: bigint[] = [], rung = 0): BrowseRow => ({mosaicaId: id, key: null, name, maskedCount: 10n, matchedCount: null, rung, parentIds, childCount: 0, slot: null});

async function mountField(over: {layers?: Meta['layers']; verb?: 'filter' | 'highlight'; members?: unknown[]} = {}) {
  const host = await mount(`<mosaica-cluster-filter layer="topics" verb="${over.verb ?? 'filter'}"></mosaica-cluster-filter>`);
  const el = host.querySelector('mosaica-cluster-filter') as MosaicaClusterFilter;
  const store = fakeStore({meta: meta({layers: over.layers ?? [layer('topics')]}), status: status({})});
  store.set('view', {...store.get('view'), id: 's0'});
  if (over.members) store.set('filters', {...store.get('filters'), members: over.members as never});
  el.store = store;
  await settle(host);
  return {host, el, store, box: () => deep(host, '[part="entry"]') as HTMLInputElement};
}

const browses = (store: ReturnType<typeof fakeStore>) => store.calls.filter((c) => c.name === 'browse').map((c) => c.args[0]);
const names = (host: HTMLElement) => deepAll(host, '[part~="option"] [part="name"]').map((n) => n.textContent);
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

describe('<mosaica-cluster-filter> with the box empty', () => {
  it('offers the top-level clusters, asked unfiltered, and counts them through the aggregate without its own filter clauses', async () => {
    const {host, store, box} = await mountField();
    store.setBrowse('roots', {artifacts: [row(1n, 'Physics'), row(2n, 'Biology'), row(3n, null)], parents: [], next: null});
    expect(browses(store)).toHaveLength(0);
    box().dispatchEvent(new Event('focus'));
    await settle(host);
    expect(browses(store)).toEqual([{layer: 'topics', filters: null, limit: 50}]);
    expect(names(host)).toEqual(['Physics', 'Biology', UNNAMED]);
    expect([...registered(store).values()]).toEqual([{groupings: [{count: true, by: {layer: 'topics', artifacts: [1n, 2n, 3n]}}], withoutMembersOf: 'topics'}]);
    answerAggregate(store, 'clusters', aggregateEntry([{rows: [{key: 1n, count: 5}, {key: 2n, count: 90}, {key: 3n, count: 0}]}]));
    await settle(host);
    // Largest first once counted.
    expect(names(host)).toEqual(['Biology', 'Physics', UNNAMED]);
    expect(deepAll(host, '[part="value-count"]').map((c) => c.textContent)).toEqual(['90', '5', '0']);
  });

  it('asks the first level of a levelled layer, and counts one grouping per level', async () => {
    const tiers = layer('topics', {hierarchy: {kind: 'tiered', pruneChildren: false}, levels: [{level: 0, title: null, zoom: null}, {level: 1, title: null, zoom: null}]});
    const {host, store, box} = await mountField({layers: [tiers]});
    store.setBrowse('roots', {artifacts: [row(4n, 'Coarse', [], 0)], parents: [], next: null});
    box().dispatchEvent(new Event('focus'));
    await settle(host);
    expect(browses(store)).toEqual([{layer: 'topics', filters: null, limit: 50, level: 0}]);
    expect([...registered(store).values()]).toEqual([{groupings: [{count: true, by: {layer: 'topics', level: 0, artifacts: [4n]}}], withoutMembersOf: 'topics'}]);
  });
});

describe('<mosaica-cluster-filter> asking', () => {
  it('keeps its counts registered only while its list is open', async () => {
    const {host, store, box} = await mountField();
    store.setBrowse('roots', {artifacts: [row(1n, 'Physics')], parents: [], next: null});
    box().dispatchEvent(new Event('focus'));
    await settle(host);
    expect(registered(store).size).toBe(1);
    box().dispatchEvent(new Event('blur'));
    await settle(host);
    expect(registered(store).size).toBe(0);
  });

  it('asks for the top-level clusters again on the next open after a refusal, and drops the refusal once answered', async () => {
    const {host, store, box} = await mountField();
    const answered = store.browse;
    let fail = true;
    store.browse = (async (req: never) => {
      if (fail) throw {code: 'backpressure', detail: 'busy'};
      return answered(req);
    }) as never;
    store.setBrowse('roots', {artifacts: [row(1n, 'Physics')], parents: [], next: null});
    box().dispatchEvent(new Event('focus'));
    await settle(host);
    expect(deep(host, '[part="refusal"]')).not.toBeNull();
    box().dispatchEvent(new Event('blur'));
    fail = false;
    box().dispatchEvent(new Event('focus'));
    await settle(host);
    await settle(host);
    expect(deep(host, '[part="refusal"]')).toBeNull();
    expect(names(host)).toEqual(['Physics']);
  });
});

describe('<mosaica-cluster-filter> from the keyboard', () => {
  const three = {artifacts: [row(1n, 'Physics'), row(2n, 'Biology'), row(3n, 'Chemistry')], parents: [], next: null};
  const press = async (host: HTMLElement, box: HTMLInputElement, key: string) => {
    const event = new KeyboardEvent('keydown', {key, cancelable: true});
    box.dispatchEvent(event);
    await settle(host);
    return event;
  };
  const active = (host: HTMLElement) => deep(host, '[part~="option"][data-active]');

  it('chooses the row the arrow keys reach on Enter, and no row before they reach one', async () => {
    const {host, store, box} = await mountField();
    store.setBrowse('roots', three);
    box().dispatchEvent(new Event('focus'));
    await settle(host);
    // The rows change order as their counts land, so the first is not Enter's until a key reaches it.
    await press(host, box(), 'Enter');
    expect(store.calls.filter((c) => c.name === 'setMembers')).toHaveLength(0);
    expect(active(host)).toBeNull();
    expect(box().hasAttribute('aria-activedescendant')).toBe(false);
    await press(host, box(), 'ArrowDown');
    expect(active(host)!.querySelector('[part="name"]')!.textContent).toBe('Physics');
    expect(box().getAttribute('aria-activedescendant')).toBe(active(host)!.id);
    // The counts put another row first; the row reached stays the one Enter takes.
    answerAggregate(store, 'clusters', aggregateEntry([{rows: [{key: 1n, count: 5}, {key: 2n, count: 90}, {key: 3n, count: 50}]}]));
    await settle(host);
    expect(names(host)[0]).toBe('Biology');
    await press(host, box(), 'Enter');
    expect(store.calls.filter((c) => c.name === 'setMembers').at(-1)!.args[0]).toEqual([{layer: 'topics', artifact: 1n, outside: false, verb: 'filter', label: 'Physics'}]);
  });

  it('moves to the first and the last row with Home and End once a row is reached, leaving them to the text before, and scrolls the row reached into view', async () => {
    const {host, store, box} = await mountField();
    store.setBrowse('roots', three);
    box().dispatchEvent(new Event('focus'));
    await settle(host);
    const scrolled: string[] = [];
    vi.spyOn(Element.prototype, 'scrollIntoView').mockImplementation(function (this: Element) {
      scrolled.push(this.querySelector('[part="name"]')?.textContent ?? '');
    });
    expect((await press(host, box(), 'End')).defaultPrevented).toBe(false);
    expect(active(host)).toBeNull();
    await press(host, box(), 'ArrowUp');
    expect(active(host)!.querySelector('[part="name"]')!.textContent).toBe('Chemistry');
    expect((await press(host, box(), 'Home')).defaultPrevented).toBe(true);
    expect(active(host)!.querySelector('[part="name"]')!.textContent).toBe('Physics');
    await press(host, box(), 'End');
    expect(active(host)!.querySelector('[part="name"]')!.textContent).toBe('Chemistry');
    expect(scrolled).toEqual(['Chemistry', 'Physics', 'Chemistry']);
  });
});

describe('<mosaica-cluster-filter> for a screen reader', () => {
  it('names its box by the layer and the placeholder it shows, as a list autocomplete whose rows toggle', async () => {
    const {host, store, box, el} = await mountField();
    expect(box().getAttribute('aria-label')).toContain('Topics');
    expect(box().getAttribute('aria-label')).toContain(box().getAttribute('placeholder'));
    expect(box().getAttribute('aria-autocomplete')).toBe('list');
    store.setBrowse('roots', {artifacts: [row(1n, 'Physics')], parents: [], next: null});
    box().dispatchEvent(new Event('focus'));
    await settle(host);
    expect(deep(host, '[role="listbox"]')!.getAttribute('aria-multiselectable')).toBe('true');
    // As a picker, a row is chosen once and nothing stays selected.
    el.choose = () => {};
    await settle(host);
    expect(deep(host, '[role="listbox"]')!.hasAttribute('aria-multiselectable')).toBe(false);
  });

  it('says in a status line, there before the list, how many clusters are listed and that none match', async () => {
    const {host, store, box} = await mountField();
    const status = () => deep(host, '[role="status"]')!;
    expect(status().textContent!.trim()).toBe('');
    store.setBrowse('roots', {artifacts: [row(1n, 'Physics'), row(2n, 'Biology')], parents: [], next: null});
    box().dispatchEvent(new Event('focus'));
    await settle(host);
    expect(status().textContent).toContain('2');
    box().value = 'zzz';
    box().dispatchEvent(new Event('input'));
    await wait(300);
    await settle(host);
    await settle(host);
    expect(status().querySelector('[part="more"]')!.textContent!.trim()).not.toBe('');
    expect(status().textContent).not.toContain('2');
  });

  it('says that the clusters are loading only once they have been awaited a while, and once however much is typed meanwhile', async () => {
    const {host, store, box} = await mountField();
    const said = () => deep(host, '[role="status"]')!.textContent!.trim();
    // Every answer is held back until the test lets it go.
    let release: () => void = () => {};
    const held = new Promise<void>((r) => (release = r));
    const answered = store.browse;
    store.browse = (async (req: never) => {
      await held;
      return answered(req);
    }) as never;
    store.setBrowse('q:phys', {artifacts: [row(1n, 'Physics')], parents: [], next: null});
    box().dispatchEvent(new Event('focus'));
    await settle(host);
    // At once the list's place is drawn, and nothing is said.
    expect(deep(host, '[role="status"] .skel')).not.toBeNull();
    expect(said()).toBe('');
    await wait(300);
    await settle(host);
    const loading = said();
    expect(loading).not.toBe('');
    for (const text of ['p', 'ph', 'phy', 'phys']) {
      box().value = text;
      box().dispatchEvent(new Event('input'));
      await settle(host);
      expect(said()).toBe(loading);
    }
    await wait(300);
    release();
    await settle(host);
    await settle(host);
    expect(said()).toContain('1');
    expect(said()).not.toBe(loading);
  });

  it('says nothing about a list that closed as focus left, while clusters match', async () => {
    const {host, store, box} = await mountField();
    store.setBrowse('q:phy', {artifacts: [row(1n, 'Physics')], parents: [], next: null});
    box().value = 'phy';
    box().dispatchEvent(new Event('input'));
    await wait(300);
    await settle(host);
    await settle(host);
    expect(deep(host, '[role="status"]')!.textContent).toContain('1');
    box().dispatchEvent(new Event('blur'));
    await settle(host);
    expect(deep(host, '[part="values"]')).toBeNull();
    expect(deep(host, '[role="status"]')!.textContent!.trim()).toBe('');
  });
});

describe('<mosaica-cluster-filter> typing', () => {
  it('searches the names, shows each match’s nearest parents as its path, and a pick adds a member_of clause named by the cluster', async () => {
    const {host, store, box} = await mountField();
    store.setBrowse('q:x-ray', {artifacts: [row(30n, 'x-ray state', [20n])], parents: [], next: null});
    // The children form of each artifact on the way up carries its parents.
    store.setBrowse('p:30', {artifacts: [], parents: [row(20n, 'clusters x-ray', [10n])], next: null});
    store.setBrowse('p:20', {artifacts: [], parents: [row(10n, 'galaxies', [5n])], next: null});
    store.setBrowse('p:10', {artifacts: [], parents: [row(5n, 'physics', [1n])], next: null});
    box().value = 'x-ray';
    box().dispatchEvent(new Event('input'));
    await wait(300);
    await settle(host);
    await settle(host);
    expect(browses(store)[0]).toEqual({q: 'x-ray', layer: 'topics', filters: null, limit: 50});
    expect(names(host)).toEqual(['x-ray state']);
    // Three parents, nearest last, and more above them.
    expect(deep(host, '[part="path"]')!.textContent).toBe('… › physics › galaxies › clusters x-ray');
    (deep(host, '[part~="option"]') as HTMLButtonElement).click();
    expect(store.calls.filter((c) => c.name === 'setMembers').at(-1)!.args[0]).toEqual([{layer: 'topics', artifact: 30n, outside: false, verb: 'filter', label: 'x-ray state'}]);
  });

  it('searches every level of a levelled layer in one request with no level, and walks a match’s parents there too', async () => {
    const tiers = layer('topics', {hierarchy: {kind: 'tiered', pruneChildren: false}, levels: [{level: 0, title: null, zoom: null}, {level: 1, title: null, zoom: null}]});
    const {host, store, box} = await mountField({layers: [tiers]});
    store.setBrowse('q:geometry', {artifacts: [row(11n, 'math.AG', [1n], 1), row(12n, 'math.DG', [1n], 1)], parents: [], next: null});
    store.setBrowse('p:11', {artifacts: [], parents: [row(1n, 'Mathematics')], next: null});
    store.setBrowse('p:12', {artifacts: [], parents: [row(1n, 'Mathematics')], next: null});
    box().value = 'geometry';
    box().dispatchEvent(new Event('input'));
    await wait(300);
    await settle(host);
    await settle(host);
    expect(browses(store)).toEqual([
      {q: 'geometry', layer: 'topics', filters: null, limit: 50},
      {parent: 11n, layer: 'topics', filters: null, limit: 1},
      {parent: 12n, layer: 'topics', filters: null, limit: 1}
    ]);
    expect(names(host)).toEqual(['math.AG', 'math.DG']);
    expect(deepAll(host, '[part="path"]').map((p) => p.textContent)).toEqual(['Mathematics', 'Mathematics']);
  });

  it('asks for a parent once however many searches meet it, and names it from what it learnt', async () => {
    const {host, store, box} = await mountField();
    store.setBrowse('q:x-ray', {artifacts: [row(30n, 'x-ray state', [20n])], parents: [], next: null});
    store.setBrowse('q:x-ray s', {artifacts: [row(30n, 'x-ray state', [20n]), row(31n, 'x-ray source', [20n])], parents: [], next: null});
    store.setBrowse('p:30', {artifacts: [], parents: [row(20n, 'clusters x-ray')], next: null});
    for (const text of ['x-ray', 'x-ray s']) {
      box().value = text;
      box().dispatchEvent(new Event('input'));
      await wait(300);
      await settle(host);
      await settle(host);
    }
    expect(browses(store).filter((r) => (r as {parent?: bigint}).parent !== undefined)).toEqual([{parent: 30n, layer: 'topics', filters: null, limit: 1}]);
    expect(deepAll(host, '[part="path"]').map((p) => p.textContent)).toEqual(['clusters x-ray', 'clusters x-ray']);
  });

  it('names every served parent of a match with several, and walks on up from the first', async () => {
    const {host, store, box} = await mountField({layers: [layer('topics', {hierarchy: {kind: 'dag', pruneChildren: false}})]});
    store.setBrowse('q:glioma', {artifacts: [row(30n, 'glioma', [10n, 20n])], parents: [], next: null});
    store.setBrowse('p:30', {artifacts: [], parents: [row(10n, 'brain tumours', [1n]), row(20n, 'nerve tissue tumours', [2n])], next: null});
    store.setBrowse('p:10', {artifacts: [], parents: [row(1n, 'neoplasms')], next: null});
    box().value = 'glioma';
    box().dispatchEvent(new Event('input'));
    await wait(300);
    await settle(host);
    await settle(host);
    expect(deep(host, '[part="path"]')!.textContent).toBe('neoplasms › brain tumours, nerve tissue tumours');
  });
});

