import {describe, expect, it, onTestFinished} from 'vitest';
import {composeFilters, type FilterExpr, type FilterOperandSet, type Store, type ViewInput} from '@mosaicajs/client';
import {mortonOfTile} from '@mosaicajs/client/internal';
import {draftOf, initialize, render, stateOf, type KernelMessage, type WidgetModel} from '../src/widget.js';
import {aggregateEntry, answerAggregate, fakeStore, settle, status, type FakeStore} from './fake-store.js';

/**
 * The widget's JavaScript half against a fake model and a fake store: the token protocol (never
 * model state), the settle-only up-sync, the echo guard, and the expression-to-draft inversion.
 */

type Sent = {content: unknown};

function fakeModel(initial: Record<string, unknown>): WidgetModel & {sent: Sent[]; saves: number; fire(event: string, ...args: unknown[]): void; state: Record<string, unknown>} {
  const state = {...initial};
  const handlers = new Map<string, ((...args: unknown[]) => void)[]>();
  const sent: Sent[] = [];
  const m = {
    state,
    sent,
    saves: 0,
    get: (k: string) => state[k],
    set(k: string, v: unknown) {
      state[k] = v;
      m.fire(`change:${k}`);
    },
    save_changes() {
      m.saves += 1;
    },
    on(event: string, cb: (...args: unknown[]) => void) {
      handlers.set(event, [...(handlers.get(event) ?? []), cb]);
    },
    off(event: string, cb?: (...args: unknown[]) => void) {
      handlers.set(event, (handlers.get(event) ?? []).filter((h) => h !== cb));
    },
    send(content: unknown) {
      sent.push({content});
    },
    fire(event: string, ...args: unknown[]) {
      for (const h of handlers.get(event) ?? []) h(...args);
    }
  };
  return m;
}

const META = {
  apiVersion: 1,
  views: [{id: 's0', displayName: 'default', quantisation: {xMin: 0, xMax: 1, yMin: 0, yMax: 1}, roster: null}],
  // A one-view bundle declares no group; the explorer's pickers read this and draw nothing.
  groups: [],
  declaredScalars: [],
  layers: [],
  selection: {kMin: 1, kMaxMarks: 500, maxK: 5000, thetaTargetMarks: 10, maxUnderlayOffset: 0, maxCategoryValues: 1000, maxRegionVertices: 10_000, maxRegionCells: 262_144},
  maxTilesPerRequest: 4096,
  filterOperands: [] as FilterOperandSet[]
};

/** A layer points can be coloured by: it declares geometry and depends on nothing. */
const clusters = (name: string) => ({name, title: name, views: ['s0'], membership: 'enumerated', hierarchy: {kind: 'flat', pruneChildren: false}, levels: [], computedContent: ['centroid'], shape: null, suppliedContent: [], depsOn: [], version: 1});

const base = {url: 'http://mosaica.test', view: null, layers: null, colour_by: null, palette: null, value_colours: null, cluster_colours: null, size_by: null, size_min: null, size_max: null, size_scale: null, filters: null, bbox: null, selected: null, selected_artifact: null, region: null, explorer_layout: 'docked', height: 400, title_field: null};

/** A model initialised and one view rendered, so there is a store: the store is per view. */
function setUp(initial: Record<string, unknown> = {}) {
  const model = fakeModel({...base, ...initial});
  let supplier: (() => Promise<{token: string; expiresAt: number}>) | null = null;
  const stores: FakeStore[] = [];
  const dispose = initialize({
    model,
    storeFactory: (opts) => {
      supplier = opts.authorise;
      const store = fakeStore();
      stores.push(store);
      return store as Store;
    }
  });
  const el = document.createElement('div');
  document.body.append(el);
  const unmount = render({model, el});
  return {model, store: stores[0]!, stores, el, unmount, supplier: () => supplier!, dispose};
}

describe('the token protocol', () => {
  it('sends ready once, answers the supplier from the custom message, and never sets a token on the model', async () => {
    const {model, supplier} = setUp();
    expect(model.sent).toEqual([]);
    const p = supplier()();
    expect(model.sent.map((s) => s.content)).toEqual([{type: 'ready'}]);
    model.fire('msg:custom', {type: 'token', token: 'tok-1', expires_at: 1_800_000_000} satisfies KernelMessage);
    expect(await p).toEqual({token: 'tok-1', expiresAt: 1_800_000_000});
    expect(Object.values(model.state)).not.toContain('tok-1');
    expect(model.saves).toBe(0);
  });

  it('sends reauthorise on every later call, and shares one outstanding request', async () => {
    const {model, supplier} = setUp();
    const first = supplier()();
    model.fire('msg:custom', {type: 'token', token: 'a', expires_at: 1});
    await first;
    const second = supplier()();
    const third = supplier()();
    expect(model.sent.map((s) => s.content)).toEqual([{type: 'ready'}, {type: 'reauthorise'}]);
    model.fire('msg:custom', {type: 'token', token: 'b', expires_at: 2});
    expect((await second).token).toBe('b');
    expect((await third).token).toBe('b');
  });

  it('a refusal rejects the supplier', async () => {
    const {model, supplier} = setUp();
    const p = supplier()();
    model.fire('msg:custom', {type: 'refused', detail: 'no credential'});
    await expect(p).rejects.toThrow('no credential');
  });

  it('hands title_field to every view’s explorer, and follows a change to it', async () => {
    const {model, el} = setUp({title_field: 'title'});
    const explorer = el.querySelector('mosaica-explorer') as unknown as {titleField: string};
    expect(explorer.titleField).toBe('title');
    model.set('title_field', null);
    expect(explorer.titleField).toBe('');
  });

  it('fills the room below its top while its output is shown full screen, and takes `height` again on the way out', () => {
    const {model, el} = setUp({height: 400});
    const explorer = el.querySelector('mosaica-explorer') as HTMLElement;
    const height = () => explorer.style.getPropertyValue('--mosaica-explorer-height');
    expect(height()).toBe('400px');
    const box = (top: number, bottom: number) => ({top, bottom, left: 0, right: 0, width: 0, height: bottom - top, x: 0, y: top, toJSON: () => null}) as DOMRect;
    el.getBoundingClientRect = () => box(0, 700);
    explorer.getBoundingClientRect = () => box(60, 460);
    const full = (shown: Element | null) => {
      Object.defineProperty(document, 'fullscreenElement', {configurable: true, get: () => shown});
      document.dispatchEvent(new Event('fullscreenchange'));
    };
    full(el);
    expect(height()).toBe('640px');
    model.set('height', 500);
    expect(height()).toBe('640px');
    full(null);
    expect(height()).toBe('500px');
    // Another element, or the whole page, shown full screen leaves the map as it is.
    full(document.createElement('div'));
    expect(height()).toBe('500px');
    full(document.documentElement);
    expect(height()).toBe('500px');
    full(null);
  });

  it('hands the point budget and its range to every view’s explorer, and follows a change to each', async () => {
    const {model, el} = setUp({budget: 40_000, budget_min: 500, budget_max: 90_000});
    const explorer = el.querySelector('mosaica-explorer') as unknown as {budget: number; budgetMin: number; budgetMax: number};
    expect([explorer.budget, explorer.budgetMin, explorer.budgetMax]).toEqual([40_000, 500, 90_000]);
    model.set('budget', 60_000);
    model.set('budget_max', 100_000);
    expect([explorer.budget, explorer.budgetMin, explorer.budgetMax]).toEqual([60_000, 500, 100_000]);
  });

  it('hands the cluster budget and its range to every view’s explorer, and sends it up when Most clusters is let go', async () => {
    const {model, el} = setUp({cluster_budget: 300, cluster_budget_min: 5, cluster_budget_max: 5_000});
    const explorer = el.querySelector('mosaica-explorer') as unknown as HTMLElement & {clusterBudget: number; clusterBudgetMin: number; clusterBudgetMax: number};
    expect([explorer.clusterBudget, explorer.clusterBudgetMin, explorer.clusterBudgetMax]).toEqual([300, 5, 5_000]);
    model.set('cluster_budget_max', 8_000);
    expect(explorer.clusterBudgetMax).toBe(8_000);
    explorer.dispatchEvent(new CustomEvent('mosaica-clusterbudgetchange', {detail: {budget: 1_200}, bubbles: true, composed: true}));
    expect(model.get('cluster_budget')).toBe(1_200);
    expect(explorer.clusterBudget).toBe(1_200);
    model.set('cluster_budget', 300);
    expect(explorer.clusterBudget).toBe(300);
    model.set('cluster_budget', 0);
    expect(explorer.clusterBudget).toBe(0);
  });

  it('sends the budget up when Most points is let go, and takes the old one back from the kernel', async () => {
    const {model, el} = setUp({budget: 40_000});
    const explorer = el.querySelector('mosaica-explorer') as unknown as HTMLElement & {budget: number};
    const saves = model.saves;
    explorer.dispatchEvent(new CustomEvent('mosaica-budgetchange', {detail: {budget: 120_000}, bubbles: true, composed: true}));
    expect(model.get('budget')).toBe(120_000);
    expect(model.saves).toBe(saves + 1);
    expect(explorer.budget).toBe(120_000);
    // The kernel setting the old budget back reaches the explorer.
    model.set('budget', 40_000);
    expect(explorer.budget).toBe(40_000);
  });

  it('two views of one model: a store each, one supplier, and one ready', async () => {
    const {model, supplier, stores, el} = setUp();
    const el2 = document.createElement('div');
    document.body.append(el2);
    const unmount2 = render({model, el: el2});
    await settle(document.body);
    expect(stateOf(model)?.views).toBe(2);
    expect(stores).toHaveLength(2);
    expect(el.querySelector('mosaica-explorer')).not.toBeNull();
    // Both stores ask; one ready goes out, and a held token is handed back without a round trip.
    const a = supplier()();
    const b = supplier()();
    model.fire('msg:custom', {type: 'token', token: 'shared', expires_at: null});
    expect((await a).token).toBe('shared');
    expect((await b).token).toBe('shared');
    expect((await supplier()()).token).toBe('shared');
    expect(model.sent.map((s) => s.content)).toEqual([{type: 'ready'}]);
    // A view's store is disposed with the view.
    unmount2();
    expect(stores[1]!.calls.map((c) => c.name)).toContain('dispose');
    expect(stateOf(model)?.views).toBe(1);
  });

  it('a token near expiry is renewed with reauthorise; one with no expiry is never asked for again', async () => {
    const {model, supplier} = setUp();
    const first = supplier()();
    model.fire('msg:custom', {type: 'token', token: 'a', expires_at: Date.now() / 1000 + 10});
    await first;
    const second = supplier()();
    expect(model.sent.map((s) => s.content)).toEqual([{type: 'ready'}, {type: 'reauthorise'}]);
    model.fire('msg:custom', {type: 'token', token: 'b', expires_at: Date.now() / 1000 + 3600});
    expect((await second).token).toBe('b');
    expect((await supplier()()).token).toBe('b');
    expect(model.sent).toHaveLength(2);
  });
});

describe('the up-sync', () => {
  function shown(store: FakeStore, composition: object) {
    store.set('status', status({}));
    store.set('view', {...store.get('view'), composition: {tiles: [], exact: [], standIn: [], exactDrawn: 0, exactServed: 0, ...composition} as never});
  }

  it('happens at the settle, not per projection change, and carries the control traits from the store', () => {
    const {model, store} = setUp();
    store.set('artifacts', {...store.get('artifacts'), layers: ['clusters/a']});
    store.set('legend', {...store.get('legend'), colourBy: 'cluster:clusters/a'});
    store.set('status', {...status({}), status: 'loading'});
    expect(model.saves).toBe(0);
    shown(store, {});
    expect(model.saves).toBe(1);
    expect(model.state.layers).toEqual(['clusters/a']);
    expect(model.state.colour_by).toBe('cluster:clusters/a');
    expect(model.state.palette).toBe('tableau10');
    expect(model.state.filters).toBeNull();
    expect([model.state.size_by, model.state.size_min, model.state.size_max, model.state.size_scale]).toEqual([null, 2, 9, 'linear']);
    // The same composition presented again is not a new settle.
    store.set('status', status({}));
    expect(model.saves).toBe(1);
  });

  it('ids cross as decimal strings, never numbers', () => {
    const {model, store} = setUp();
    const id = 2n ** 63n + 5n;
    store.set('selection', {item: {id, detail: {} as never}, itemRefusal: null, artifact: {id: 7n, detail: {layer: 'clusters', key: null, maskedCount: 1n, centroid: null, box: null, shape: null}}, artifactRefusal: null});
    expect(model.state.selected).toBe('9223372036854775813');
    expect(model.state.selected_artifact).toBe('7');
    expect(typeof model.state.selected).toBe('string');
  });

  it('a region syncs when its counts arrive, not while loading', () => {
    const {model, store} = setUp();
    const shape = {kind: 'box' as const, bbox: [0, 0, 1, 1] as [number, number, number, number]};
    const loading = {shape, status: 'loading' as const, refusal: null, visible: {value: 0, exact: true}, matched: {value: 0, exact: true}, served: {shown: 0, total: 0, exact: true}, verdict: {exact: true as const, depth: null}, held: {ids: new BigUint64Array(0), positions: new Float32Array(0), count: 0}};
    store.set('region', loading);
    expect(model.state.region).toBeNull();
    store.set('region', {...loading, status: 'shown', visible: {value: 42, exact: false}});
    expect(model.state.region).toMatchObject({status: 'shown', visible: {value: 42, exact: false}, verdict: {exact: true, depth: null}});
    expect((model.state.region as {held?: unknown}).held).toBeUndefined();
  });

  it('the view the map shows is up-synced at the settle', () => {
    const {model, store} = setUp();
    store.set('view', {...store.get('view'), id: 'quarter:2026-Q3'});
    shown(store, {});
    expect(model.state.view).toBe('quarter:2026-Q3');
    // And it does not come back down as a switch: the echo guard covers `view` as it does the
    // other controls.
    expect(store.calls.filter((c) => c.name === 'setCurrentView')).toHaveLength(0);
  });

  it('the echo guard: an up-synced layers change does not come back down as setLayers', () => {
    const {model, store} = setUp();
    expect(store.calls.filter((c) => c.name === 'setLayers')).toHaveLength(0);
    store.set('artifacts', {...store.get('artifacts'), layers: ['x']});
    shown(store, {});
    expect(store.calls.filter((c) => c.name === 'setLayers')).toHaveLength(0);
    model.set('layers', ['y']);
    expect(store.calls.filter((c) => c.name === 'setLayers').map((c) => c.args)).toEqual([[['y']]]);
  });
});

describe('a view change', () => {
  it('switches every mounted view\u2019s store rather than rebuilding it', () => {
    const {model, stores, store} = setUp();
    const el2 = document.createElement('div');
    document.body.append(el2);
    render({model, el: el2});
    expect(stores).toHaveLength(2);
    model.set('view', 'quarter:2026-Q3');
    // A view change rebuilds nothing: no third store, and nothing disposed.
    expect(stores).toHaveLength(2);
    expect(store.calls.filter((c) => c.name === 'dispose')).toHaveLength(0);
    for (const s of stores) expect(s.calls.filter((c) => c.name === 'setCurrentView').map((c) => c.args)).toEqual([['quarter:2026-Q3']]);
  });

  it('a url change is still a rebuild: a mosaica_id minted by one bundle means nothing to another', () => {
    const {model, stores, store} = setUp();
    model.set('url', 'http://other.test');
    expect(stores).toHaveLength(2);
    expect(store.calls.filter((c) => c.name === 'dispose')).toHaveLength(1);
  });
});

describe('the down-sync', () => {
  it('a bbox set in the kernel before meta is fitted when meta arrives, and echoes nothing back', async () => {
    const {model, store, el} = setUp();
    await settle(document.body);
    const explorer = el.querySelector('mosaica-explorer') as unknown as {map: {fitBbox(b: number[]): boolean} | null};
    const fitted: number[][] = [];
    const map = explorer.map;
    expect(map).not.toBeNull();
    // As the real one: nothing fits before meta, and the box is pending.
    map!.fitBbox = (b) => {
      if (!store.get('meta')) return false;
      fitted.push(b);
      return true;
    };
    model.set('bbox', [1, 2, 3, 4]);
    expect(fitted).toEqual([]);
    store.set('meta', META as never);
    expect(fitted).toEqual([[1, 2, 3, 4]]);
  });

  it('a bbox stated in the kernel is the opening camera, and the framing of the matches does not move it', async () => {
    // Every map is 1000 × 800 px, whenever the explorer renders it.
    const proto = customElements.get('mosaica-map')!.prototype as HTMLElement;
    Object.defineProperty(proto, 'clientWidth', {configurable: true, value: 1000});
    Object.defineProperty(proto, 'clientHeight', {configurable: true, value: 800});
    onTestFinished(() => {
      delete (proto as {clientWidth?: number}).clientWidth;
      delete (proto as {clientHeight?: number}).clientHeight;
    });
    const {store} = setUp({bbox: [0.1, 0.1, 0.2, 0.2]});
    await settle(document.body);
    store.set('meta', {...META, selection: {...META.selection, maxAggregateCells: 1_048_576}} as never);
    store.set('view', {...store.get('view'), id: 's0'});
    await settle(document.body);
    answerAggregate(store, 'frame#', aggregateEntry([{rows: [{cell: mortonOfTile(250, 250, 8), count: 9}]}]));
    await settle(document.body);
    const cameras = store.calls.filter((c) => c.name === 'setView').map((c) => (c.args[0] as ViewInput).bbox);
    expect(cameras.length).toBeGreaterThan(0);
    // The stated box, 0.1 to 0.2 of the unit square, is 51.2 to 102.4 in world units.
    for (const [x0, y0, x1, y1] of cameras) {
      expect((x0 + x1) / 2).toBeCloseTo(76.8);
      expect((y0 + y1) / 2).toBeCloseTo(76.8);
    }
  });

  it('reopens on the same camera after reloads, from the box it wrote back', async () => {
    const proto = customElements.get('mosaica-map')!.prototype as HTMLElement;
    Object.defineProperty(proto, 'clientWidth', {configurable: true, value: 1000});
    Object.defineProperty(proto, 'clientHeight', {configurable: true, value: 800});
    onTestFinished(() => {
      delete (proto as {clientWidth?: number}).clientWidth;
      delete (proto as {clientHeight?: number}).clientHeight;
    });
    /** A page loaded on a model holding `bbox`: the box its camera shows once meta has arrived. */
    const load = async (bbox: number[]) => {
      const {store, el, unmount, dispose} = setUp({bbox});
      // The world square as the data extent, so a box reads the same in both.
      store.setFrame({xMin: 0, xMax: 512, yMin: 0, yMax: 512});
      await settle(document.body);
      store.set('meta', {...META, selection: {...META.selection, maxAggregateCells: 1_048_576}} as never);
      store.set('view', {...store.get('view'), id: 's0'});
      await settle(document.body);
      const shown = (store.calls.filter((c) => c.name === 'setView').at(-1)!.args[0] as ViewInput).bbox;
      unmount();
      dispose();
      el.remove();
      return shown;
    };
    const first = await load([100, 100, 200, 180]);
    const second = await load(first);
    const third = await load(second);
    for (const [a, b] of [[first, second], [second, third]] as const) for (let i = 0; i < 4; i++) expect(b[i]).toBeCloseTo(a[i]!);
  });

  it('layers null leaves the default and [] is none', () => {
    expect(setUp({layers: null}).store.calls.filter((c) => c.name === 'setLayers')).toHaveLength(0);
    expect(setUp({layers: []}).store.calls.filter((c) => c.name === 'setLayers').map((c) => c.args)).toEqual([[[]]]);
  });

  it('applies layers and colour_by set in the kernel, and filters once meta has the operand list', () => {
    const {model, store} = setUp({layers: ['clusters/a'], colour_by: 'cluster:clusters/a'});
    expect(store.calls.map((c) => c.name)).toEqual(['setLayers', 'setColourBy']);
    // Only the active view syncs up; a settle on it after meta carries what the store applied.
    const operands: FilterOperandSet[] = [{column: 'year', family: 'numeric', operands: ['range']}];
    store.set('meta', {...META, layers: [clusters('clusters/a')], filterOperands: operands} as never);
    expect(model.sent).toEqual([]);
    // The traitlet is the filter expression; a highlight the page holds is kept beside it.
    const lit = {year: {family: 'numeric' as const, gte: 1990, lte: 1999}};
    store.set('filters', {...store.get('filters'), draft: {filter: {}, highlight: lit}});
    model.set('filters', {year: {range: {gte: 2000}}});
    const applied = store.calls.filter((c) => c.name === 'setFilters');
    expect(applied).toHaveLength(1);
    expect(applied[0]!.args[0]).toEqual({filter: {year: {family: 'numeric', gte: 2000, lte: null}}, highlight: lit});
    // An expression the draft cannot hold is refused to the kernel, and applies nothing.
    model.set('filters', {any_of: [{year: {range: {gte: 1}}}]});
    expect(store.calls.filter((c) => c.name === 'setFilters')).toHaveLength(1);
    expect(model.sent.at(-1)?.content).toMatchObject({type: 'error', what: 'filters'});
  });

  it('applies the palette set in the kernel and follows a change; one chosen in the explorer goes up at once', () => {
    const {model, store, el} = setUp({palette: 'kelly'});
    expect(store.calls.filter((c) => c.name === 'setPalette').map((c) => c.args)).toEqual([['kelly']]);
    model.set('palette', 'okabe-ito');
    expect(store.calls.filter((c) => c.name === 'setPalette').map((c) => c.args)).toEqual([['kelly'], ['okabe-ito']]);
    const explorer = el.querySelector('mosaica-explorer')!;
    explorer.dispatchEvent(new CustomEvent('mosaica-clusterpalettechange', {detail: {palette: 'tableau20'}, bubbles: true, composed: true}));
    expect(model.state.palette).toBe('tableau20');
    // The up-sync does not come back down.
    expect(store.calls.filter((c) => c.name === 'setPalette')).toHaveLength(2);
  });

  /** Choose colours in the explorer as a card does: set on `store`, then reported. */
  async function choosers(store: FakeStore, explorer: Element) {
    const {setClusterColours, setValueColours} = await import('../src/colouring.js');
    return {
      value(column: string, changes: {value: string; colour: string | null}[]) {
        setValueColours(store, column, changes);
        explorer.dispatchEvent(new CustomEvent('mosaica-valuecolour', {detail: {column, changes}, bubbles: true, composed: true}));
      },
      cluster(layer: string, changes: {mosaicaId: string; colour: string | null}[]) {
        setClusterColours(store, layer, changes.map((c) => ({mosaicaId: BigInt(c.mosaicaId), colour: c.colour})));
        explorer.dispatchEvent(new CustomEvent('mosaica-clustercolour', {detail: {layer, changes}, bubbles: true, composed: true}));
      }
    };
  }

  it('applies the value and cluster colours set in the kernel and follows a change; colours chosen in the explorer go up at once', async () => {
    const {colouringOf} = await import('../src/colouring.js');
    const {model, store, stores, el} = setUp({value_colours: {venue: {nips: '#112233'}}, cluster_colours: {topics: {'7': '#445566'}}});
    const explorer = el.querySelector('mosaica-explorer')!;
    const choose = await choosers(store, explorer);
    await settle(el);
    const chosen = () => [...(store.get('artifacts').overrides.get('topics')?.keys() ?? [])];
    expect(colouringOf(store).values).toEqual({venue: {nips: '#112233'}});
    expect(chosen()).toEqual([7n]);
    model.set('cluster_colours', {topics: {'8': '#000000'}});
    await settle(el);
    expect(chosen()).toEqual([8n]);
    const saves = model.saves;
    choose.value('venue', [{value: 'icml', colour: '#abcdef'}, {value: 'nips', colour: null}]);
    expect(model.state.value_colours).toEqual({venue: {icml: '#abcdef'}});
    // The explorer holds them too, so a store it builds next starts with them.
    expect(explorer.valueColours).toEqual({venue: {icml: '#abcdef'}});
    choose.value('venue', [{value: 'icml', colour: null}]);
    expect(model.state.value_colours).toEqual({});
    choose.cluster('topics', [{mosaicaId: '9', colour: '#fedcba'}]);
    expect(model.state.cluster_colours).toEqual({topics: {'8': '#000000', '9': '#fedcba'}});
    expect(explorer.clusterColours).toEqual({topics: {'8': '#000000', '9': '#fedcba'}});
    expect(model.saves).toBe(saves + 3);
    // A store built for a new url starts with the colours chosen.
    model.set('url', 'http://mosaica.test/other');
    await settle(el);
    const next = stores.at(-1)!;
    expect(next).not.toBe(store);
    expect([...(next.get('artifacts').overrides.get('topics')?.keys() ?? [])]).toEqual([8n, 9n]);
  });

  it('keeps every colour chosen in the map when the kernel set None, and sends them all up with the next', async () => {
    const {colouringOf} = await import('../src/colouring.js');
    const {model, store, el} = setUp();
    const explorer = el.querySelector('mosaica-explorer')!;
    const choose = await choosers(store, explorer);
    await settle(el);
    choose.value('venue', [{value: 'nips', colour: '#112233'}]);
    choose.cluster('topics', [{mosaicaId: '7', colour: '#445566'}]);
    // None leaves the colours chosen in the map.
    model.set('value_colours', null);
    model.set('cluster_colours', null);
    await settle(el);
    choose.value('venue', [{value: 'icml', colour: '#abcdef'}]);
    choose.cluster('topics', [{mosaicaId: '8', colour: '#000000'}]);
    await settle(el);
    expect(colouringOf(store).values).toEqual({venue: {nips: '#112233', icml: '#abcdef'}});
    expect(model.state.value_colours).toEqual({venue: {nips: '#112233', icml: '#abcdef'}});
    expect([...store.get('artifacts').overrides.get('topics')!.keys()]).toEqual([7n, 8n]);
    expect(model.state.cluster_colours).toEqual({topics: {'7': '#445566', '8': '#000000'}});
  });

  it('applies the size settings set in the kernel, the scale before the column, and follows each change', async () => {
    const {sizingOf} = await import('../src/colouring.js');
    const {model, store} = setUp({size_by: 'citations', size_min: 3, size_max: 11, size_scale: 'rank'});
    // Sized by rank, the store is asked to keep a sample of the column's values.
    expect(store.calls.filter((c) => c.name === 'setSizeBy').map((c) => c.args)).toEqual([['citations', {rank: true}]]);
    expect(sizingOf(store)).toEqual({min: 3, max: 11, scale: 'rank'});
    model.set('size_max', 7);
    model.set('size_scale', 'log');
    expect(sizingOf(store)).toEqual({min: 3, max: 7, scale: 'log'});
    // A radius that is not a number above zero leaves the one drawn, which the next settle reports.
    model.set('size_min', -1);
    expect(sizingOf(store).min).toBe(3);
    model.set('size_by', null);
    expect(store.calls.filter((c) => c.name === 'setSizeBy').at(-1)!.args).toEqual([null, {rank: false}]);
  });

  it('reports a colour_by naming no layer this view can colour by, once meta lists the layers', () => {
    const {model, store} = setUp({colour_by: 'cluster:nope'});
    // The store takes it as given; before meta there is nothing to check it against.
    expect(store.calls.filter((c) => c.name === 'setColourBy').map((c) => c.args)).toEqual([['cluster:nope']]);
    expect(model.sent).toEqual([]);
    store.set('meta', {...META, layers: [clusters('topics'), {...clusters('topic_names'), computedContent: [], depsOn: ['topics']}]} as never);
    expect(model.sent.map((s) => s.content)).toMatchObject([{type: 'error', what: 'colour_by'}]);
    // A layer that can colour is not reported, drawn or not; a labels layer cannot colour.
    model.set('colour_by', 'cluster:topics');
    model.set('colour_by', 'archive');
    expect(model.sent).toHaveLength(1);
    model.set('colour_by', 'cluster:topic_names');
    expect(model.sent.map((s) => s.content)).toMatchObject([{what: 'colour_by'}, {type: 'error', what: 'colour_by'}]);
    expect(store.calls.filter((c) => c.name === 'setLayers')).toHaveLength(0);
  });

  it('a filters expression set before meta is applied at meta', () => {
    const {model, store} = setUp({filters: {year: {range: {lte: 5}}}});
    expect(store.calls.filter((c) => c.name === 'setFilters')).toHaveLength(0);
    store.set('meta', {...META, filterOperands: [{column: 'year', family: 'numeric', operands: ['range']}]} as never);
    expect(store.calls.filter((c) => c.name === 'setFilters')).toHaveLength(1);
    expect(model.sent).toEqual([]);
  });
});

describe('the basemap', () => {
  type Basemapped = {basemap: unknown};
  const explorerOf = (el: HTMLElement) => el.querySelector('mosaica-explorer') as unknown as Basemapped;
  const bytes = (text: string) => new TextEncoder().encode(text).buffer;
  const image = (path: string, read: number) => ({type: 'basemap-image', path, read, mime: 'image/png'}) satisfies KernelMessage;
  const synced = (value: unknown, images: Record<string, number> = {}) => ({_basemap: {value, images}});
  const asked = (model: {sent: Sent[]}) => model.sent.filter((s) => (s.content as {type: string}).type === 'basemap-images').length;

  /** Object URLs made and revoked, in place of the browser's. */
  function objectUrls() {
    const made: string[] = [];
    const revoked: string[] = [];
    const create = URL.createObjectURL;
    const revoke = URL.revokeObjectURL;
    URL.createObjectURL = (blob: Blob) => {
      made.push(`blob:test/${made.length}:${blob.type}:${blob.size}`);
      return made.at(-1)!;
    };
    URL.revokeObjectURL = (url: string) => void revoked.push(url);
    onTestFinished(() => {
      URL.createObjectURL = create;
      URL.revokeObjectURL = revoke;
    });
    return {made, revoked};
  }

  it('is handed to every view’s explorer as the kernel set it, and follows a change', () => {
    const {model, el} = setUp(synced(null));
    const explorer = explorerOf(el);
    expect(explorer.basemap).toBeNull();
    const plan = {image: 'https://example.org/plan.png', bounds: [0, 0, 1, 1]};
    model.set('_basemap', {value: plan, images: {}});
    expect(explorer.basemap).toEqual(plan);
    model.set('_basemap', {value: {s0: 'none', geo: 'world'}, images: {}});
    expect(explorer.basemap).toEqual({s0: 'none', geo: 'world'});
    const other = document.createElement('div');
    render({model, el: other});
    expect(explorerOf(other).basemap).toEqual({s0: 'none', geo: 'world'});
    model.set('_basemap', {value: null, images: {}});
    expect(explorer.basemap).toBeNull();
    expect(model.sent).toEqual([]);
  });

  it('draws a local image from the bytes it asks for, as an object URL revoked when it is replaced or torn down', () => {
    const {made, revoked} = objectUrls();
    const bounds = [0, 0, 4, 3];
    const {model, el, dispose} = setUp(synced({s0: {image: {local: 'plan.png'}, bounds}, geo: 'world'}, {'plan.png': 1}));
    // The page asks for the bytes, and draws no basemap until they come.
    expect(model.sent.map((s) => s.content)).toEqual([{type: 'basemap-images', reads: {'plan.png': 1}}]);
    expect(explorerOf(el).basemap).toBe('none');
    // Bytes from a read the model does not list are dropped.
    model.fire('msg:custom', image('plan.png', 0), [bytes('old')]);
    expect(made).toEqual([]);
    model.fire('msg:custom', image('plan.png', 1), [new DataView(bytes('pixels'))]);
    expect(made).toEqual(['blob:test/0:image/png:6']);
    expect(explorerOf(el).basemap).toEqual({s0: {image: made[0], bounds}, geo: 'world'});
    // The same read again makes no new URL. A new read of the path is asked for by name, and the
    // old URL is drawn until it arrives, then revoked.
    model.fire('msg:custom', image('plan.png', 1), [bytes('pixels')]);
    expect(made).toHaveLength(1);
    model.set('_basemap', {value: {image: {local: 'plan.png'}}, images: {'plan.png': 2}});
    expect(model.sent.at(-1)!.content).toEqual({type: 'basemap-images', reads: {'plan.png': 2}});
    expect(revoked).toEqual([]);
    expect(explorerOf(el).basemap).toEqual({image: made[0]});
    model.fire('msg:custom', image('plan.png', 2), [bytes('pixels2')]);
    expect(revoked).toEqual([made[0]]);
    expect(explorerOf(el).basemap).toEqual({image: made[1]});
    // A string is a URL, even one spelt as a listed path.
    model.set('_basemap', {value: {s0: {image: 'plan.png'}, geo: {image: {local: 'plan.png'}}}, images: {'plan.png': 2}});
    expect(explorerOf(el).basemap).toEqual({s0: {image: 'plan.png'}, geo: {image: made[1]}});
    // An image URL is the kernel's to give as it is, and the image no longer listed is revoked.
    model.set('_basemap', {value: {image: 'https://example.org/plan.png'}, images: {}});
    expect(explorerOf(el).basemap).toEqual({image: 'https://example.org/plan.png'});
    expect(revoked).toEqual([made[0], made[1]]);
    expect(asked(model)).toBe(2);
    dispose();
    expect(revoked).toEqual([made[0], made[1]]);
  });

  it('never hands the explorer a local path, even as the value moves away from it', () => {
    objectUrls();
    const {model, el} = setUp(synced({image: {local: 'plan.png'}}, {'plan.png': 1}));
    const seen: unknown[] = [];
    const explorer = el.querySelector('mosaica-explorer')!;
    // Watch the element's own accessor, on its prototype, where Lit keeps it.
    const proto = Object.getPrototypeOf(explorer) as object;
    const own = Object.getOwnPropertyDescriptor(proto, 'basemap')!;
    Object.defineProperty(proto, 'basemap', {
      ...own,
      set(this: unknown, v: unknown) {
        if (this === explorer) seen.push(v);
        own.set!.call(this, v);
      }
    });
    onTestFinished(() => void Object.defineProperty(proto, 'basemap', own));
    model.set('_basemap', {value: 'world', images: {}});
    model.set('_basemap', {value: {image: {local: 'plan.png'}}, images: {'plan.png': 2}});
    model.fire('msg:custom', image('plan.png', 2), [bytes('pixels')]);
    expect(seen.map((v) => JSON.stringify(v)).join()).not.toContain('local');
    expect(seen).toEqual(['world', 'none', {image: 'blob:test/0:image/png:6'}]);
  });

  it('an image sent before the listing that names it is asked for again once the listing arrives', () => {
    const {made} = objectUrls();
    const {model, el} = setUp(synced('world'));
    expect(asked(model)).toBe(0);
    model.fire('msg:custom', image('plan.png', 1), [bytes('pixels')]);
    expect(made).toEqual([]);
    model.set('_basemap', {value: {image: {local: 'plan.png'}}, images: {'plan.png': 1}});
    expect(asked(model)).toBe(1);
    model.fire('msg:custom', image('plan.png', 1), [bytes('pixels')]);
    expect(explorerOf(el).basemap).toEqual({image: made[0]});
  });

  it('asks once for a read in flight, and again once the path was dropped and listed anew', () => {
    const {made} = objectUrls();
    const {model, el} = setUp(synced({image: {local: 'plan.png'}}, {'plan.png': 1}));
    expect(asked(model)).toBe(1);
    model.set('_basemap', {value: {image: {local: 'plan.png'}, bounds: [0, 0, 2, 2]}, images: {'plan.png': 1}});
    expect(asked(model)).toBe(1);
    model.set('_basemap', {value: 'world', images: {}});
    model.set('_basemap', {value: {image: {local: 'plan.png'}}, images: {'plan.png': 1}});
    expect(asked(model)).toBe(2);
    model.fire('msg:custom', image('plan.png', 1), [bytes('pixels')]);
    expect(explorerOf(el).basemap).toEqual({image: made[0]});
    model.set('_basemap', {value: {image: {local: 'plan.png'}}, images: {'plan.png': 2}});
    expect(asked(model)).toBe(3);
  });

  it('a message carrying an image does not settle a token request', async () => {
    const {model, supplier} = setUp(synced({image: {local: 'plan.png'}}, {'plan.png': 1}));
    objectUrls();
    const p = supplier()();
    model.fire('msg:custom', image('plan.png', 1), [bytes('pixels')]);
    model.fire('msg:custom', {type: 'token', token: 'tok-1', expires_at: null});
    expect((await p).token).toBe('tok-1');
  });

  it('a basemap the map refuses reaches the kernel; a note that it draws nothing under this view does not', () => {
    const {model, el} = setUp();
    const explorer = el.querySelector('mosaica-explorer')!;
    const say = (detail: {refusal: string | null; note: string | null}) => explorer.dispatchEvent(new CustomEvent('mosaica-basemapchange', {detail, bubbles: true, composed: true}));
    say({refusal: null, note: 'View s1 has no map projection.'});
    expect(model.sent).toEqual([]);
    say({refusal: 'View s0 has no map projection, so the world outline cannot be drawn under it; use "none" or an image.', note: null});
    expect(model.sent.map((s) => s.content)).toEqual([{type: 'error', what: 'basemap', detail: 'View s0 has no map projection, so the world outline cannot be drawn under it; use "none" or an image.'}]);
  });
});

describe('draftOf inverts composeFilters', () => {
  const operands: FilterOperandSet[] = [
    {column: 'title', family: 'text', operands: ['match', 'phrase']},
    {column: 'archive', family: 'category', operands: ['in']},
    {column: 'author', family: 'keyword', operands: ['eq', 'prefix', 'contains']},
    {column: 'year', family: 'numeric', operands: ['range']}
  ];
  const cases: FilterExpr[] = [
    {title: {match: 'sea'}},
    {title: {phrase: 'the sea'}},
    {archive: {in: ['a', 'b']}},
    {author: {prefix: 'Ke'}},
    {year: {range: {gte: 1900, lte: 1950}}},
    {all_of: [{title: {match: 'x'}}, {archive: {in: ['a']}}, {author: {eq: 'K'}}, {year: {range: {lte: 3}}}]}
  ];
  for (const expr of cases) {
    it(JSON.stringify(expr), () => {
      expect(composeFilters(draftOf(expr, operands))).toEqual(expr);
    });
  }
  it('reads back every text expression the box writes, alone and beside other columns', () => {
    for (const query of ['graph neural', '"graph neural"', 'networks "graph neural"', 'graph OR lattice', '"neural net" OR gnn OR graph lattice']) {
      const alone = composeFilters({filter: {title: {family: 'text', query, phrase: true}}, highlight: {}})!;
      const draft = draftOf(alone, operands);
      expect(draft.filter['title']).toEqual({family: 'text', query, phrase: true});
      expect(composeFilters(draft)).toEqual(alone);
      const beside = composeFilters({filter: {title: {family: 'text', query, phrase: true}, archive: {family: 'category', keys: ['a']}}, highlight: {}})!;
      expect(composeFilters(draftOf(beside, operands))).toEqual(beside);
    }
  });
  it('keeps a text expression the box cannot write as it was sent', () => {
    for (const expr of [
      {title: {match: 'salt OR pepper'}},
      {title: {match: {query: 'sea sky', minimum_should_match: 1}}},
      {title: {match: 'say "hello"'}},
      {any_of: [{title: {phrase: 'the sea'}}, {title: {match: {query: 'a b c', minimum_should_match: 2}}}]}
    ] as FilterExpr[]) {
      const draft = draftOf(expr, operands);
      expect(draft.filter['title']).toMatchObject({family: 'text', expr});
      expect(composeFilters(draft)).toEqual(expr);
    }
  });
  it('null is the unfiltered request', () => {
    expect(composeFilters(draftOf(null, operands))).toBeNull();
  });
  it('refuses what a draft cannot hold', () => {
    expect(() => draftOf({none_of: [{title: {match: 'x'}}]}, operands)).toThrow();
    expect(() => draftOf({nope: {eq: 'x'}}, operands)).toThrow();
    expect(() => draftOf({year: {eq: 3}}, operands)).toThrow();
    expect(() => draftOf({all_of: [{year: {range: {gte: 1}}}, {year: {range: {lte: 2}}}]}, operands)).toThrow();
    expect(() => draftOf({year: {range: {gt: 1}}}, operands)).toThrow();
    expect(() => draftOf({year: {range: {gte: '2020-01-01'}}}, operands)).toThrow();
    expect(() => draftOf({any_of: [{title: {match: 'x'}}, {archive: {in: ['a']}}]}, operands)).toThrow();
  });
});
