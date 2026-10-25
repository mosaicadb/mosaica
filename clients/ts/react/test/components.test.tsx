import {act, createElement, createRef} from 'react';
import {createRoot, type Root} from 'react-dom/client';
import {afterEach, beforeEach, describe, expect, it} from 'vitest';
import {formatCount} from '@mosaicajs/client';
import {deep, fakeStore, meta, scalar, settle, status} from '../../components/test/fake-store.js';
import {MosaicaCount, MosaicaExplorer, MosaicaFilter, MosaicaHierarchy, MosaicaItemCard, MosaicaKeyPicker, MosaicaMap, MosaicaStatus, MosaicaStore, MosaicaViewPicker, type CountElement, type ExplorerElement, type ItemCardElement, type StoreElement, type ViewPickerElement} from '../src/components.js';

/**
 * The wrappers: an object prop lands as a property (never an attribute), an `on*` prop receives
 * the element's own event with its detail, and a `<MosaicaStore>` above provides by context to a
 * wrapped panel below — the same store precedence the elements decide (`base.ts`).
 */
(globalThis as unknown as {IS_REACT_ACT_ENVIRONMENT: boolean}).IS_REACT_ACT_ENVIRONMENT = true;

let root: Root;
let host: HTMLElement;
beforeEach(() => {
  host = document.createElement('div');
  document.body.append(host);
  root = createRoot(host);
});
afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
});

describe('@mosaicajs/react/components', () => {
  it('an object prop is set as a property and the element renders it', async () => {
    const ref = createRef<CountElement>();
    await act(async () => root.render(createElement(MosaicaCount, {ref, label: 'shown', count: {shown: 221, total: 1_994_089, exact: true}})));
    await settle(host);
    expect(ref.current?.count).toEqual({shown: 221, total: 1_994_089, exact: true});
    expect(ref.current?.hasAttribute('count')).toBe(false);
    expect(deep(host, '[part="count"]')?.textContent).toBe(formatCount({shown: 221, total: 1_994_089, exact: true}));
  });

  it('an on* prop receives the element’s event with its detail, and a function prop is called', async () => {
    const ref = createRef<ItemCardElement>();
    const seen: string[] = [];
    const opened: string[] = [];
    await act(async () =>
      root.render(
        createElement(MosaicaItemCard, {
          ref,
          item: {id: 7n, detail: {fields: {title: 'x'}, labels: [], views: [], scoped: {}}},
          open: (item) => opened.push(item.id),
          onOpen: (e) => seen.push(e.detail.id)
        })
      )
    );
    await settle(host);
    (deep(host, '[part="open"]') as HTMLButtonElement).click();
    expect(seen).toEqual(['7']);
    expect(opened).toEqual(['7']);
  });

  it('openUrl on the explorer reaches its item card, whose Open links to it', async () => {
    const ref = createRef<ExplorerElement>();
    const store = fakeStore({meta: meta(), status: status({})});
    await act(async () => root.render(createElement(MosaicaExplorer, {ref, store, openUrl: 'https://example.org/{title}'})));
    store.set('selection', {item: {id: 7n, detail: {fields: {title: 'x y'}, labels: [], views: [], scoped: {}}}, itemRefusal: null, artifact: null, artifactRefusal: null});
    await settle(host);
    expect(deep(host, '[part="open"]')?.getAttribute('href')).toBe('https://example.org/x%20y');
  });

  it('the pickers take a store as a property and switch through it', async () => {
    const meta = {
      apiVersion: 1,
      views: [
        {id: 'knn', displayName: 'knn', quantisation: {xMin: 0, xMax: 1, yMin: 0, yMax: 1}, projection: 'none', worldAspect: null, tileScheme: null, tile: null, roster: null},
        {id: 'quarter:a', displayName: 'a', quantisation: {xMin: 0, xMax: 1, yMin: 0, yMax: 1}, projection: 'none', worldAspect: null, tileScheme: null, tile: null, roster: {group: 'quarter', key: 'a', metadata: {}}},
        {id: 'quarter:b', displayName: 'b', quantisation: {xMin: 0, xMax: 1, yMin: 0, yMax: 1}, projection: 'none', worldAspect: null, tileScheme: null, tile: null, roster: {group: 'quarter', key: 'b', metadata: {}}}
      ],
      groups: [{name: 'quarter', title: 'Quarter', membersOf: null, views: ['quarter:a', 'quarter:b']}],
      declaredScalars: [],
      layers: [],
      selection: {kMin: 1, kMaxMarks: 500, maxK: 5000, thetaTargetMarks: 10, maxUnderlayOffset: 0, maxCategoryValues: 1000, maxRegionVertices: 10_000, maxRegionCells: 262_144},
      maxTilesPerRequest: 4096,
      filterOperands: []
    } as never;
    const store = fakeStore({meta, status: status({status: 'shown'})});
    store.set('view', {...store.get('view'), id: 'quarter:a'});
    const ref = createRef<ViewPickerElement>();
    const seen: {to: string; sameFrame: boolean}[] = [];
    await act(async () =>
      root.render(
        createElement(
          MosaicaStore,
          {store},
          createElement(MosaicaViewPicker, {ref, onViewSwitch: (e) => seen.push({to: e.detail.to, sameFrame: e.detail.sameFrame})}),
          createElement(MosaicaKeyPicker, {})
        )
      )
    );
    await settle(host);
    // The store arrives by context, as it does for every other wrapped panel.
    expect(ref.current?.source).toBe('context');
    expect((deep(host, 'mosaica-view-picker') as HTMLElement).shadowRoot!.querySelector('select')).not.toBeNull();
    const roster = (deep(host, 'mosaica-key-picker') as HTMLElement).shadowRoot!.querySelector('select') as HTMLSelectElement;
    roster.value = 'quarter:b';
    roster.dispatchEvent(new Event('change'));
    expect(store.calls.filter((c) => c.name === 'setCurrentView').map((c) => c.args)).toEqual([['quarter:b']]);
    // The event bubbles to the provider above, not sideways: the view picker's handler is not the
    // key picker's, and a host wires whichever it wants.
    expect(seen).toEqual([]);
  });

  it('an on* prop on a picker receives its switch event, with the frames compared', async () => {
    const meta = {
      apiVersion: 1,
      views: [
        {id: 'knn', displayName: 'knn', quantisation: {xMin: 0, xMax: 1, yMin: 0, yMax: 1}, projection: 'none', worldAspect: null, tileScheme: null, tile: null, roster: null},
        {id: 'pca64', displayName: 'pca64', quantisation: {xMin: -1, xMax: 1, yMin: -1, yMax: 1}, projection: 'none', worldAspect: null, tileScheme: null, tile: null, roster: null}
      ],
      groups: [],
      declaredScalars: [],
      layers: [],
      selection: {kMin: 1, kMaxMarks: 500, maxK: 5000, thetaTargetMarks: 10, maxUnderlayOffset: 0, maxCategoryValues: 1000, maxRegionVertices: 10_000, maxRegionCells: 262_144},
      maxTilesPerRequest: 4096,
      filterOperands: []
    } as never;
    const store = fakeStore({meta, status: status({status: 'shown'})});
    store.set('view', {...store.get('view'), id: 'knn'});
    const seen: {from: string; to: string; sameFrame: boolean}[] = [];
    await act(async () => root.render(createElement(MosaicaViewPicker, {store, onViewSwitch: (e) => seen.push(e.detail)})));
    await settle(host);
    const select = (deep(host, 'mosaica-view-picker') as HTMLElement).shadowRoot!.querySelector('select') as HTMLSelectElement;
    select.value = 'v:pca64';
    select.dispatchEvent(new Event('change'));
    expect(seen).toEqual([{from: 'knn', to: 'pca64', sameFrame: false}]);
  });

  it('onPick and onItemOpen on the map receive the click’s id and then the record', async () => {
    const store = fakeStore({status: status({})});
    const seen: unknown[] = [];
    await act(async () => root.render(createElement(MosaicaMap, {store, onPick: (e) => seen.push(e.detail), onItemOpen: (e) => seen.push(e.detail)})));
    await settle(host);
    const map = host.querySelector('mosaica-map') as unknown as {onClick(info: unknown): void};
    map.onClick({index: 0, x: 5, y: 5, sourceLayer: {id: 'marks-p0', props: {mosaicaIds: new BigUint64Array([7n])}}, coordinate: [5, 5]});
    const record = {fields: {name: 'Ada'}, labels: [], views: [], scoped: {}};
    store.set('selection', {item: {id: 7n, detail: record}, itemRefusal: null, artifact: null, artifactRefusal: null});
    expect(seen).toEqual([{id: '7'}, {id: '7', record}]);
  });

  it('cardFields on the explorer reaches its item card', async () => {
    const ref = createRef<ExplorerElement>();
    await act(async () => root.render(createElement(MosaicaExplorer, {ref, cardFields: 'author'})));
    await settle(host);
    expect(ref.current?.cardFields).toBe('author');
  });

  it('titleField on the explorer reaches its map', async () => {
    const ref = createRef<ExplorerElement>();
    await act(async () => root.render(createElement(MosaicaExplorer, {ref, titleField: 'name'})));
    await settle(host);
    expect(ref.current?.titleField).toBe('name');
    expect(ref.current?.map?.titleField).toBe('name');
  });

  it('MosaicaHierarchy hands its clause event to an on* prop, the id a decimal string', async () => {
    const meta = {
      apiVersion: 1,
      views: [{id: 's0', displayName: 's0', quantisation: {xMin: 0, xMax: 1, yMin: 0, yMax: 1}, projection: 'none', worldAspect: null, tileScheme: null, tile: null, roster: null}],
      groups: [],
      declaredScalars: [],
      layers: [{name: 'mesh', title: 'mesh', views: ['s0'], hierarchy: {kind: 'dag', pruneChildren: false}, levels: [], computedContent: [], shape: null, suppliedContent: ['name'], depsOn: [], version: 1}],
      selection: {kMin: 1, kMaxMarks: 500, maxK: 5000, thetaTargetMarks: 10, maxUnderlayOffset: 0, maxCategoryValues: 1000, maxRegionVertices: 10_000, maxRegionCells: 262_144, maxBrowseRows: 200},
      maxTilesPerRequest: 4096,
      filterOperands: []
    } as never;
    const store = fakeStore({meta, status: status({status: 'shown'})});
    store.setBrowse('roots', {artifacts: [{mosaicaId: 2n ** 63n, key: 'd-1', name: 'Neoplasms', maskedCount: 10n, matchedCount: null, rung: 0, parentIds: [], childCount: 0, slot: null}], parents: [], next: null});
    const seen: {id: string; verb: string; on: boolean}[] = [];
    await act(async () => root.render(createElement(MosaicaHierarchy, {store, onClauseChange: (e) => seen.push({id: e.detail.id, verb: e.detail.verb, on: e.detail.on})})));
    await settle(host);
    await settle(host);
    (deep(host, '[part="row"] [part="name"]') as HTMLButtonElement).click();
    expect(seen).toEqual([{id: '9223372036854775808', verb: 'highlight', on: true}]);
  });

  it('MosaicaFilter edits its column through the store and hands the change to onFilterChange', async () => {
    const store = fakeStore({
      meta: meta({
        declaredScalars: [scalar('archive', 'u16', {category: {vocabulary: 'a', kind: 'declared', visibility: 'public'}, render: true, homes: ['rendered']})],
        filterOperands: [{column: 'archive', family: 'category', operands: ['eq', 'in']}]
      }),
      status: status({}),
      filters: {draft: {filter: {archive: {family: 'category', keys: []}}, highlight: {}}, expr: null, highlight: null, members: [], suggestions: {}, suggestErrors: {}, suggestEpoch: 0}
    });
    const seen: string[] = [];
    await act(async () => root.render(createElement(MosaicaFilter, {store, column: 'archive', onFilterChange: (e) => seen.push(String(e.detail.column))})));
    await settle(host);
    const entry = deep(host, '[part="entry"]') as HTMLInputElement;
    entry.value = 'c';
    entry.dispatchEvent(new Event('input'));
    store.set('filters', {...store.get('filters'), suggestions: {archive: {q: 'c', verb: 'filter', values: [{code: 1, key: 'cs', title: 'CS', match: {field: 'key', start: 0, len: 1}}], more: false, total: null}}});
    await settle(host);
    (deep(host, '[part~="tick"]') as HTMLElement).click();
    expect(seen).toEqual(['archive']);
  });

  it('an inline authorise arrow does not rebuild the element’s own store on a re-render', async () => {
    // The store opens a session at once; there is no server here.
    const fetched = globalThis.fetch;
    globalThis.fetch = async () => {
      throw new Error('no server in this test');
    };
    const ref = createRef<StoreElement>();
    const draw = (n: number) => root.render(createElement(MosaicaStore, {ref, viewerUrl: 'http://127.0.0.1:1', authorise: async () => ({token: `t${n}`, expiresAt: 1})}));
    await act(async () => draw(1));
    await settle(host);
    const built = ref.current?.activeStore;
    expect(built).not.toBeNull();
    await act(async () => draw(2));
    await settle(host);
    expect(ref.current?.activeStore).toBe(built);
    ref.current?.dispose();
    globalThis.fetch = fetched;
  });

  it('a MosaicaStore above provides by context to a wrapped panel below', async () => {
    const store = fakeStore({status: status({status: 'shown'}), view: {...fakeStore().get('view'), served: {shown: 5, total: 9, exact: true}}});
    await act(async () => root.render(createElement(MosaicaStore, {store}, createElement(MosaicaStatus))));
    await settle(host);
    const el = host.querySelector('mosaica-status') as {activeStore: unknown; source: string};
    expect(el.activeStore).toBe(store);
    expect(el.source).toBe('context');
    expect(deep(host, '[part="state"]')?.getAttribute('data-state')).toBe('shown');
  });
});
