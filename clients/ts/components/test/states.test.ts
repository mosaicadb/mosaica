import {afterEach, describe, expect, it, vi} from 'vitest';
import {composeFilters, type FilterDraft} from '@mosaicajs/client';
import '../src/status.js';
import '../src/selection.js';
import '../src/count.js';
import '../src/map.js';
import type {MosaicaStatus} from '../src/status.js';
import {stateOf} from '../src/states.js';
import {deep, deepAll, fakeStore, mount, settle, status} from './fake-store.js';

/**
 * Every panel state through `part="state"`, on the strip and on a panel: only `shown` renders a
 * count, a stale view renders none and offers refresh, a refusal renders as one, and an expiry
 * fires `mosaica-expired`.
 */

afterEach(() => {
  vi.useRealTimers();
  document.body.innerHTML = '';
});

const view = {
  id: 'v',
  composition: null,
  depth: 5,
  visible: {value: 12_040, exact: true},
  matched: {value: 3_210, exact: true},
  highlighted: {value: 3_210, exact: true},
  highlighting: false,
  served: {shown: 500, total: 12_040, exact: true},
  provisional: 0,
  inView: {status: 'shown' as const, visible: {value: 12_040, exact: true}, matched: {value: 3_210, exact: true}, highlighted: {value: 3_210, exact: true}, shown: 500}
};

describe('stateOf maps status onto the ten states exactly as the table says', () => {
  it('detached, loading, retrying, shown, empty, refused, busy, expired, stale', () => {
    expect(stateOf(null)).toBe('detached');
    expect(stateOf(status({status: 'idle'}))).toBe('loading');
    expect(stateOf(status({status: 'loading'}))).toBe('loading');
    expect(stateOf(status({status: 'retrying'}))).toBe('retrying');
    expect(stateOf(status({status: 'shown'}))).toBe('shown');
    expect(stateOf(status({status: 'empty'}))).toBe('empty');
    expect(stateOf(status({status: 'refused', refusal: {code: 'x', detail: 'y', status: 422}}))).toBe('refused');
    expect(stateOf(status({status: 'refused', refusal: {code: 'backpressure', detail: 'shed', status: 429}}))).toBe('busy');
    expect(stateOf(status({status: 'refused', refusal: {code: 'expired-token', detail: '', status: 403}, expired: true}))).toBe('expired');
    expect(stateOf(status({status: 'shown', stale: true}))).toBe('stale');
  });

  it('retrying while the server cannot be reached, whatever was last answered, until a session ends', () => {
    for (const s of ['shown', 'refused', 'empty', 'loading'] as const) expect(stateOf(status({status: s, unreachable: true}))).toBe('retrying');
    expect(stateOf(status({status: 'refused', expired: true, unreachable: true}))).toBe('expired');
  });

  it('the strip has no green dot while the server cannot be reached, and names it apart from a shed request', async () => {
    const host = await mount('<mosaica-status></mosaica-status>');
    const store = fakeStore({status: status({}), view});
    (host.querySelector('mosaica-status') as MosaicaStatus).store = store;
    await settle(host);
    expect(deep(host, '[part="state"] .dot:not(.quiet):not(.warn):not(.refuse)')).not.toBeNull();
    store.set('status', status({status: 'retrying'}));
    await settle(host);
    const shed = deep(host, '[part="state"]')?.textContent?.trim();
    store.set('status', status({unreachable: true}));
    await settle(host);
    expect(deep(host, '[part="state"]')?.getAttribute('data-state')).toBe('retrying');
    expect(deep(host, '[part="state"] .dot.warn')).not.toBeNull();
    expect(deep(host, '[part="state"]')?.textContent?.trim()).not.toBe(shed);
  });
});

describe('a server error with no Mosaica body', () => {
  const gateway = {code: 'http-error', detail: 'Gateway Timeout', status: 504};

  it('is unanswered where it is a 5xx, and a refusal where the server named one or it is a 4xx', () => {
    expect(stateOf(status({status: 'refused', refusal: gateway}))).toBe('unanswered');
    expect(stateOf(status({status: 'refused', refusal: {code: 'http-error', detail: '', status: 503}}))).toBe('unanswered');
    expect(stateOf(status({status: 'refused', refusal: {code: 'fail-closed', detail: 'postings unreadable', status: 500}}))).toBe('refused');
    expect(stateOf(status({status: 'refused', refusal: {code: 'http-error', detail: '', status: 404}}))).toBe('refused');
    expect(stateOf(status({status: 'refused', refusal: gateway, unreachable: true}))).toBe('retrying');
  });

  it('reads Server did not answer on the strip as Server unreachable does, offers Retry, and gives a host the status', async () => {
    const host = await mount('<mosaica-status></mosaica-status>');
    const changes: {to: string; refusal: unknown}[] = [];
    host.addEventListener('mosaica-statechange', (e) => changes.push((e as CustomEvent).detail));
    const store = fakeStore({status: status({unreachable: true}), view});
    (host.querySelector('mosaica-status') as MosaicaStatus).store = store;
    await settle(host);
    const unreachable = deep(host, '[part="state"]')!;
    const look = (el: Element) => ({dot: el.querySelector('.dot')?.className, colour: getComputedStyle(el).color, weight: getComputedStyle(el).fontWeight});
    const before = look(unreachable);

    store.set('status', status({status: 'refused', refusal: gateway}));
    await settle(host);
    const state = deep(host, '[part="state"]')!;
    expect(state.getAttribute('data-state')).toBe('unanswered');
    expect(state.textContent?.replace('Retry', '').trim()).toBe('Server did not answer');
    expect(look(state)).toEqual(before);
    expect(deep(host, '[part="refusal"]')).toBeNull();
    expect(changes.at(-1)).toEqual({from: 'retrying', to: 'unanswered', refusal: gateway});

    (deep(host, '[part="retry"]') as HTMLButtonElement).click();
    expect(store.calls.filter((c) => c.name === 'refresh')).toHaveLength(1);
  });
});

describe('a server still busy after the store’s retries', () => {
  const busy = {code: 'backpressure', detail: 'the viewer is shedding load', status: 429};
  const refused = {code: 'contract', detail: 'no', status: 422};

  it('is its own state on the strip and over the map, in the same words, with Retry and the refusal’s code', async () => {
    const host = await mount('<mosaica-status></mosaica-status><mosaica-map></mosaica-map>');
    const changes: {to: string; refusal: unknown}[] = [];
    host.addEventListener('mosaica-statechange', (e) => changes.push((e as CustomEvent).detail));
    const store = fakeStore({status: status({status: 'refused', refusal: refused}), view});
    for (const el of host.querySelectorAll('mosaica-status, mosaica-map')) (el as unknown as {store: unknown}).store = store;
    await settle(host);
    const strip = () => host.querySelector('mosaica-status')!.shadowRoot!.querySelector('[part="state"]')!;
    const over = () => host.querySelector('mosaica-map')!.shadowRoot!.querySelector('[part="overlay"] [part="state"]')!;
    const words = (el: Element) => el.querySelector('[part="refusal"]')?.textContent?.trim();
    const refusedWords = words(strip());

    store.set('status', status({status: 'refused', refusal: busy}));
    await settle(host);
    for (const state of [strip(), over()]) {
      expect(state.getAttribute('data-state')).toBe('busy');
      expect(state.querySelector('[part="refusal"]')?.getAttribute('data-code')).toBe('backpressure');
      expect(state.querySelector('.dot.warn')).not.toBeNull();
      expect(state.querySelector('[part="reason"]')).toBeNull();
    }
    expect(words(strip())).toBe(words(over()));
    expect(words(strip())).not.toBe(refusedWords);
    expect(changes.at(-1)).toEqual({from: 'refused', to: 'busy', refusal: busy});

    (strip().querySelector('[part="retry"]') as HTMLButtonElement).click();
    (over().querySelector('[part="retry"]') as HTMLButtonElement).click();
    expect(store.calls.filter((c) => c.name === 'refresh')).toHaveLength(2);
  });
});

describe('a 429 from a proxy', () => {
  it('is busy as Mosaica’s own is, with the code that came on data-code', async () => {
    const proxied = {code: 'http-error', detail: 'Too Many Requests', status: 429};
    expect(stateOf(status({status: 'refused', refusal: proxied}))).toBe('busy');
    const host = await mount('<mosaica-status></mosaica-status><mosaica-map></mosaica-map>');
    const store = fakeStore({status: status({status: 'refused', refusal: proxied}), view});
    for (const el of host.querySelectorAll('mosaica-status, mosaica-map')) (el as unknown as {store: unknown}).store = store;
    await settle(host);
    for (const tag of ['mosaica-status', 'mosaica-map']) {
      const state = host.querySelector(tag)!.shadowRoot!.querySelector('[part="state"]')!;
      expect(state.getAttribute('data-state')).toBe('busy');
      expect(state.querySelector('[part="refusal"]')?.getAttribute('data-code')).toBe('http-error');
      expect(state.querySelector('[part="retry"]')).not.toBeNull();
    }
  });
});

describe('<mosaica-selection> on a busy server', () => {
  it('says what the strip says, with the refusal’s code, in place of a refusal of the selection', async () => {
    const host = await mount('<mosaica-selection></mosaica-selection><mosaica-status></mosaica-status>');
    const busy = {code: 'backpressure', detail: 'shed', status: 429};
    const store = fakeStore({status: status({status: 'refused', refusal: busy}), view});
    for (const el of host.querySelectorAll('mosaica-selection, mosaica-status')) (el as unknown as {store: unknown}).store = store;
    const held = {ids: BigUint64Array.from([1n]), positions: new Float32Array(2), count: 1};
    const shape = {kind: 'box' as const, bbox: [0, 0, 1, 1] as [number, number, number, number]};
    store.set('region', {shape, status: 'refused' as const, refusal: busy, visible: null, matched: {value: 0, exact: false}, served: {shown: 1, total: 0, exact: false}, verdict: null, held});
    await settle(host);
    const state = (tag: string) => deep(host.querySelector(tag)!.shadowRoot!, '[part="state"]')!;
    const words = (tag: string) => state(tag).querySelector('[part="refusal"]')?.textContent?.trim();
    expect(state('mosaica-selection').getAttribute('data-state')).toBe('busy');
    expect(state('mosaica-selection').querySelector('[part="refusal"]')?.getAttribute('data-code')).toBe('backpressure');
    expect(words('mosaica-selection')).toBe(words('mosaica-status'));

    (state('mosaica-selection').querySelector('[part="retry"]') as HTMLButtonElement).click();
    expect(store.calls.filter((c) => c.name === 'refresh')).toHaveLength(1);

    // The strip still shown, and only the selection's count shed.
    store.set('status', status({status: 'shown'}));
    await settle(host);
    expect(state('mosaica-selection').getAttribute('data-state')).toBe('busy');

    // The view busy, and the selection refused for its own reason: the selection's state and code.
    store.set('status', status({status: 'refused', refusal: busy}));
    store.set('region', {shape, status: 'refused' as const, refusal: {code: 'contract', detail: 'the shape has too many vertices', status: 422}, visible: null, matched: {value: 0, exact: false}, served: {shown: 1, total: 0, exact: false}, verdict: null, held});
    await settle(host);
    expect(state('mosaica-selection').getAttribute('data-state')).toBe('refused');
    expect(state('mosaica-selection').querySelector('[part="refusal"]')?.getAttribute('data-code')).toBe('contract');
  });
});

describe('a query that has not finished loading', () => {
  const running = status({status: 'loading', querying: true});
  /** A strip over `store`, on a clock the test moves. */
  async function strip(store: ReturnType<typeof fakeStore>, also = '') {
    vi.useFakeTimers({toFake: ['setTimeout', 'clearTimeout']});
    const host = await mount(`<mosaica-status></mosaica-status>${also}`);
    for (const el of host.querySelectorAll('mosaica-status, mosaica-map')) (el as unknown as {store: unknown}).store = store;
    await settle(host);
    /** Lets the elements render what the store now holds, then moves the clock. */
    const pass = async (ms: number) => {
      await settle(host);
      vi.advanceTimersByTime(ms);
      await settle(host);
    };
    const state = () => host.querySelector('mosaica-status')!.shadowRoot!.querySelector('[part="state"]')!;
    return {host, pass, state};
  }

  it('is named on the strip once it has run a quarter of a second, with Cancel, and not before', async () => {
    const store = fakeStore({status: status({}), view});
    const {host, pass, state} = await strip(store);
    store.set('status', running);
    await pass(200);
    expect(state().hasAttribute('data-querying')).toBe(false);
    expect(state().querySelector('[role="status"]')?.textContent?.trim()).toBe('');
    expect(deep(host, '[part="cancel"]')).toBeNull();

    await pass(100);
    expect(state().hasAttribute('data-querying')).toBe(true);
    expect(state().textContent?.replace('Cancel', '').trim()).toBe('Running query');
    (deep(host, '[part="cancel"]') as HTMLButtonElement).click();
    expect(store.calls.filter((c) => c.name === 'cancelQuery')).toHaveLength(1);

    store.set('status', status({}));
    await settle(host);
    expect(state().getAttribute('data-state')).toBe('shown');
    expect(deep(host, '[part="cancel"]')).toBeNull();
  });

  it('is not named where it is answered within that quarter of a second, nor is the next one named early', async () => {
    const store = fakeStore({status: status({}), view});
    const {host, pass} = await strip(store);
    store.set('status', running);
    await pass(200);
    store.set('status', status({}));
    await pass(300);
    expect(deep(host, '[part="cancel"]')).toBeNull();

    store.set('status', running);
    await pass(200);
    expect(deep(host, '[part="cancel"]')).toBeNull();
  });

  it('stays named while the map has drawn and the counts still load, and gives way to a refusal', async () => {
    const store = fakeStore({status: running, view});
    const {host, pass, state} = await strip(store, '<mosaica-map></mosaica-map>');
    await pass(300);
    store.set('status', status({status: 'shown', querying: true}));
    await settle(host);
    expect(state().textContent?.replace('Cancel', '').trim()).toBe('Running query');
    expect(state().getAttribute('data-state')).toBe('shown');
    // Nothing is said over the map for a query.
    expect(host.querySelector('mosaica-map')!.shadowRoot!.querySelector('[part="overlay"]')?.textContent?.trim()).toBe('');

    store.set('status', status({status: 'refused', refusal: {code: 'contract', detail: 'no', status: 422}}));
    await settle(host);
    expect(state().getAttribute('data-state')).toBe('refused');
    expect(deep(host, '[part="cancel"]')).toBeNull();
  });

  it('tells the page what Cancel went back to, as the controls that set those would', async () => {
    const draft: FilterDraft = {filter: {archive: {family: 'category', keys: ['cs']}}, highlight: {archive: {family: 'category', keys: ['math']}}};
    const kept = {layer: 'clusters', artifact: 7n, outside: false, verb: 'filter' as const};
    const added = {layer: 'clusters', artifact: 9n, outside: true, verb: 'highlight' as const};
    const box = {kind: 'box' as const, bbox: [0, 0, 1, 1] as [number, number, number, number]};
    const filters = (d: FilterDraft, members: (typeof kept | typeof added)[], regions: {view: string; shape: typeof box}[]) => ({draft: d, expr: composeFilters(d, 'filter'), highlight: composeFilters(d, 'highlight'), members, regions, suggestions: {}, suggestErrors: {}, suggestEpoch: 0});
    const region = {shape: box} as NonNullable<ReturnType<typeof store.get<'region'>>>;
    const store = fakeStore({status: running, view, filters: filters(draft, [kept, added], [{view: 's1', shape: box}]), region});
    // What the query being cancelled had set is undone: the highlight, one clause, the region and
    // the region held in one view, and the one it had removed from another is back.
    const back: FilterDraft = {filter: draft.filter, highlight: {}};
    Object.assign(store, {
      cancelQuery: () => {
        store.set('filters', filters(back, [kept], [{view: 's2', shape: box}]));
        store.set('region', null);
      }
    });
    const {host, pass} = await strip(store);
    const fired: {type: string; detail: unknown}[] = [];
    for (const type of ['mosaica-filterchange', 'mosaica-clausechange', 'mosaica-selectchange']) host.addEventListener(type, (e) => fired.push({type, detail: (e as CustomEvent).detail}));
    await pass(300);
    (deep(host, '[part="cancel"]') as HTMLButtonElement).click();
    expect(fired).toEqual([
      {type: 'mosaica-filterchange', detail: {column: null, verb: 'highlight', expr: null}},
      {type: 'mosaica-clausechange', detail: {id: '9', layer: 'clusters', outside: true, verb: 'highlight', on: false}},
      {type: 'mosaica-selectchange', detail: {shape: null, status: 'cleared'}},
      {type: 'mosaica-selectchange', detail: {shape: null, view: 's1', status: 'cleared'}},
      {type: 'mosaica-selectchange', detail: {shape: {kind: 'box', bbox: [0, 0, 1, 1]}, view: 's2', status: 'loading'}}
    ]);
  });
});

describe('a refusal shows the server’s reason under its words', () => {
  it('on the strip and over the map, as the server sent it, and no reason where no server refused', async () => {
    const host = await mount('<mosaica-status></mosaica-status><mosaica-map></mosaica-map>');
    const rejected = {code: 'bad-credential', detail: 'the token is not one this server issued', status: 401};
    const store = fakeStore({status: status({status: 'refused', refusal: rejected}), view});
    for (const el of host.querySelectorAll('mosaica-status, mosaica-map')) (el as unknown as {store: unknown}).store = store;
    await settle(host);
    const reasons = () => [host.querySelector('mosaica-status')!, host.querySelector('mosaica-map')!].map((el) => el.shadowRoot!.querySelector('[part="state"][data-state="refused"] [part="reason"]')?.textContent ?? null);
    expect(reasons()).toEqual([rejected.detail, rejected.detail]);
    expect(deep(host, '[part="refusal"]')?.getAttribute('data-code')).toBe('bad-credential');

    store.set('status', status({status: 'refused', refusal: {code: 'fetch-failed', detail: 'TypeError: x is undefined', status: null}}));
    await settle(host);
    expect(reasons()).toEqual([null, null]);
  });
});

describe('<mosaica-map> alone says over the canvas what the strip would', () => {
  // `words`: the overlay's words, or null where it says nothing. `retry`: whether it offers Retry.
  const cases: [string, Parameters<typeof status>[0], {words: string | null; retry: boolean}][] = [
    ['loading', {status: 'loading', sessionWarm: true}, {words: null, retry: false}],
    ['a shed request', {status: 'retrying'}, {words: null, retry: false}],
    ['shown', {status: 'shown'}, {words: null, retry: false}],
    ['refused', {status: 'refused', refusal: {code: 'contract', detail: 'no', status: 422}}, {words: 'View refused', retry: true}],
    ['unanswered', {status: 'refused', refusal: {code: 'http-error', detail: 'Gateway Timeout', status: 504}}, {words: 'Server did not answer', retry: true}],
    ['unreachable', {status: 'shown', unreachable: true}, {words: 'Server unreachable', retry: false}]
  ];
  for (const [name, over, want] of cases) {
    it(name, async () => {
      const host = await mount('<mosaica-map></mosaica-map>');
      const map = host.querySelector('mosaica-map')!;
      const store = fakeStore({status: status(over), view});
      (map as unknown as {store: unknown}).store = store;
      await settle(host);
      const overlay = map.shadowRoot!.querySelector('[part="overlay"]')!;
      const reason = overlay.querySelector('[part="reason"]')?.textContent ?? '';
      expect(overlay.textContent?.replace('Retry', '').replace(reason, '').trim() || null).toBe(want.words);
      // The state line is drawn only while it has words.
      expect(overlay.querySelector('[part="state"]')!.classList.contains('sr-only')).toBe(want.words === null);
      const retry = overlay.querySelector('[part="retry"]') as HTMLButtonElement | null;
      expect(retry !== null).toBe(want.retry);
      retry?.click();
      expect(store.calls.filter((c) => c.name === 'refresh')).toHaveLength(want.retry ? 1 : 0);
    });
  }

  it('from one live element that is there before its words are, with Retry beside it', async () => {
    const host = await mount('<mosaica-map></mosaica-map>');
    const map = host.querySelector('mosaica-map')!;
    const store = fakeStore({status: status({}), view});
    (map as unknown as {store: unknown}).store = store;
    await settle(host);
    const live = map.shadowRoot!.querySelector('[part="overlay"] [role="status"]')!;
    expect(live.textContent?.trim()).toBe('');

    const refusal = {code: 'contract', detail: 'the box is outside the view', status: 422};
    store.set('status', status({status: 'refused', refusal}));
    await settle(host);
    expect(map.shadowRoot!.querySelector('[part="overlay"] [role="status"]')).toBe(live);
    expect(live.querySelector('[part="refusal"]')).not.toBeNull();
    expect(live.textContent).toContain(refusal.detail);
    const retry = map.shadowRoot!.querySelector('[part="retry"]')!;
    expect(live.contains(retry)).toBe(false);

    store.set('status', status({status: 'empty'}));
    await settle(host);
    expect(map.shadowRoot!.querySelector('[part="overlay"] [role="status"]')).toBe(live);
    expect(live.textContent?.trim()).not.toBe('');
    expect(live.textContent).not.toContain(refusal.detail);
  });

  it('keeps the focus on the map when Retry goes with its state', async () => {
    const host = await mount('<mosaica-map></mosaica-map>');
    const map = host.querySelector('mosaica-map')!;
    const store = fakeStore({status: status({status: 'refused', refusal: {code: 'contract', detail: 'no', status: 422}}), view});
    (map as unknown as {store: unknown}).store = store;
    await settle(host);
    (map.shadowRoot!.querySelector('[part="retry"]') as HTMLButtonElement).focus();
    expect(map.shadowRoot!.activeElement?.getAttribute('part')).toBe('retry');
    store.set('status', status({status: 'loading'}));
    await settle(host);
    expect(map.shadowRoot!.querySelector('[part="retry"]')).toBeNull();
    expect(map.shadowRoot!.activeElement?.getAttribute('part')).toBe('canvas');
  });

  it('announces the selected region’s count when it arrives, and again when it is counted again', async () => {
    const host = await mount('<mosaica-map></mosaica-map>');
    const map = host.querySelector('mosaica-map')!;
    const store = fakeStore({status: status({}), view});
    (map as unknown as {store: unknown}).store = store;
    await settle(host);
    const live = map.shadowRoot!.querySelector('[part="overlay"] [role="status"]')!;
    const region = (over: object) => ({shape: {kind: 'box' as const, bbox: [0.2, 0.2, 0.6, 0.6] as [number, number, number, number]}, status: 'loading' as const, refusal: null, visible: null, matched: {value: 0, exact: false}, served: {shown: 0, total: 0, exact: false}, verdict: null, held: {ids: new BigUint64Array(0), positions: new Float32Array(0), count: 0}, ...over});
    store.set('region', region({}));
    await settle(host);
    expect(live.textContent?.trim()).toBe('');
    store.set('region', region({status: 'shown', matched: {value: 4812, exact: true}}));
    await settle(host);
    expect(live.textContent).toContain('4,812');
    // Heard, not drawn: the region's tag shows the figure.
    expect(live.closest('.sr-only')).not.toBeNull();
    store.set('region', region({}));
    await settle(host);
    expect(live.textContent?.trim()).toBe('');
    store.set('region', region({status: 'shown', matched: {value: 4812, exact: true}}));
    await settle(host);
    expect(live.textContent).toContain('4,812');
    store.set('region', null);
    await settle(host);
    expect(live.textContent?.trim()).toBe('');
  });
});

describe('<mosaica-status> renders every state through part="state"', () => {
  // `count`: figures are rendered, current or greyed out. `action`: the one button the state offers.
  const cases: [string, Parameters<typeof status>[0] | null, {count: boolean; action: string | null}][] = [
    ['detached', null, {count: false, action: null}],
    ['loading', {status: 'loading', sessionWarm: false}, {count: false, action: null}],
    ['retrying', {status: 'retrying'}, {count: true, action: null}],
    ['shown', {status: 'shown'}, {count: true, action: null}],
    ['empty', {status: 'empty'}, {count: false, action: null}],
    ['refused', {status: 'refused', refusal: {code: 'unauthorised', detail: 'no', status: 403}}, {count: true, action: 'retry'}],
    ['unanswered', {status: 'refused', refusal: {code: 'http-error', detail: '', status: 504}}, {count: true, action: 'retry'}],
    ['busy', {status: 'refused', refusal: {code: 'backpressure', detail: 'shed', status: 429}}, {count: true, action: 'retry'}],
    ['expired', {status: 'refused', refusal: {code: 'expired-token', detail: '', status: 403}, expired: true}, {count: true, action: null}],
    ['stale', {status: 'shown', stale: true}, {count: false, action: 'refresh'}]
  ];
  for (const [name, over, want] of cases) {
    it(`renders ${name}`, async () => {
      const host = await mount('<mosaica-status></mosaica-status>');
      const el = host.querySelector('mosaica-status') as MosaicaStatus;
      if (over) {
        const store = fakeStore({status: status(over), view});
        el.store = store;
        await settle(host);
      }
      const state = deep(host, '[part="state"]')!;
      expect(state.getAttribute('data-state')).toBe(name);
      const counts = deepAll(host, '[part="count"]').filter((c) => c.getAttribute('data-empty') === 'false');
      expect(counts.length > 0).toBe(want.count);
      for (const action of ['refresh', 'retry', 'reauthorise']) expect(deep(host, `[part="${action}"]`) !== null, action).toBe(want.action === action);
      if (name === 'refused') expect(deep(host, '[part="refusal"]')?.getAttribute('data-code')).toBe('unauthorised');
      if (name === 'busy') expect(deep(host, '[part="refusal"]')?.getAttribute('data-code')).toBe('backpressure');
    });
  }

  /**
   * Counts outside `shown` are the last answer under the same token, greyed. A token change clears
   * the view in the store, so a view with no answer must render no figure in any state.
   */
  it('renders no count from a view with no answer, in any state', async () => {
    const empty = {...view, visible: {value: 0, exact: false}, matched: {value: 0, exact: false}, highlighted: {value: 0, exact: false}, served: {shown: 0, total: 0, exact: false}, inView: null};
    for (const over of [{status: 'loading'}, {status: 'retrying'}, {status: 'refused', refusal: {code: 'x', detail: '', status: 422}}, {status: 'refused', refusal: {code: 'expired-token', detail: '', status: 403}, expired: true}, {status: 'shown'}] as const) {
      const host = await mount('<mosaica-status></mosaica-status>');
      (host.querySelector('mosaica-status') as MosaicaStatus).store = fakeStore({status: status(over), view: empty});
      await settle(host);
      expect(deepAll(host, '[part="count"]').filter((c) => c.getAttribute('data-empty') === 'false'), over.status).toHaveLength(0);
      host.remove();
    }
  });

  /** The strip's one live element, the words in it, and the words read beside it but not drawn. */
  const live = (host: HTMLElement) => deep(host, '[part="state"] [role="status"]')!;
  const said = (host: HTMLElement) => live(host).textContent?.trim() ?? '';
  const undrawn = (host: HTMLElement) => deepAll(host, '[part="state"] > .sr-only:not([role])').map((e) => e.textContent?.trim());

  it('names up to date and updating in words that are read and not drawn, so the strip keeps its width through a pan, and every other state in words beside a dot', async () => {
    const host = await mount('<mosaica-status></mosaica-status>');
    const el = host.querySelector('mosaica-status') as MosaicaStatus;
    const store = fakeStore({status: status({sessionWarm: true}), view});
    el.store = store;
    await settle(host);
    const dot = () => deep(host, '[part="state"] .dot')!.className;
    const [upToDate] = undrawn(host);
    const solid = dot();
    expect(upToDate).not.toBe('');
    store.set('status', status({status: 'loading', sessionWarm: true}));
    await settle(host);
    const [updating] = undrawn(host);
    expect(updating).not.toBe('');
    expect(updating).not.toBe(upToDate);
    // The two differ by the dot's shape as well as by their words.
    expect(dot()).not.toBe(solid);
    for (const [over, words] of [
      [{status: 'retrying'}, 'Reconnecting'],
      [{status: 'empty'}, 'Nothing in view']
    ] as const) {
      store.set('status', status(over));
      await settle(host);
      expect(undrawn(host)).toEqual([]);
      expect(deep(host, '[part="state"] .dot')).not.toBeNull();
      expect(live(host).classList.contains('sr-only')).toBe(false);
      expect(said(host)).toBe(words);
    }
  });

  it('announces from one live element every state it names in words, and nothing as a pan moves between updating and up to date', async () => {
    const host = await mount('<mosaica-status></mosaica-status>');
    const store = fakeStore({status: status({}), view});
    (host.querySelector('mosaica-status') as MosaicaStatus).store = store;
    await settle(host);
    const element = live(host);
    expect(said(host)).toBe('');
    for (let pans = 0; pans < 3; pans++) {
      store.set('status', status({status: 'loading'}));
      await settle(host);
      expect(said(host)).toBe('');
      store.set('status', status({}));
      await settle(host);
      expect(said(host)).toBe('');
    }
    store.set('status', status({status: 'refused', refusal: {code: 'contract', detail: 'the box is outside the view', status: 422}}));
    await settle(host);
    expect(live(host)).toBe(element);
    expect(element.querySelector('[part="refusal"]')).not.toBeNull();
    expect(said(host)).toContain('the box is outside the view');

    // Back from a state it named, it says the view is up to date, once.
    store.set('status', status({status: 'loading'}));
    await settle(host);
    expect(said(host)).toBe('');
    store.set('status', status({}));
    await settle(host);
    expect(live(host)).toBe(element);
    const recovered = said(host);
    expect(recovered).not.toBe('');
    expect(undrawn(host)).toEqual([recovered]);
    store.set('status', status({status: 'loading'}));
    await settle(host);
    store.set('status', status({}));
    await settle(host);
    expect(said(host)).toBe('');
  });

  it('announces a query’s result once its counts are in, in full figures, and not again on a pan', async () => {
    const host = await mount('<mosaica-status compact></mosaica-status>');
    const figures = {...view, inView: {...view.inView, matched: {value: 3_923, exact: true}, visible: {value: 50_000, exact: true}}};
    const store = fakeStore({status: status({}), view});
    (host.querySelector('mosaica-status') as MosaicaStatus).store = store;
    await settle(host);

    store.set('status', status({status: 'loading', querying: true}));
    await settle(host);
    expect(said(host)).toBe('');
    // The map has drawn and the counts are still loading: nothing to say yet.
    store.set('view', {...figures, inView: {...figures.inView, status: 'loading' as const}});
    store.set('status', status({}));
    await settle(host);
    expect(said(host)).toBe('');
    store.set('view', figures);
    await settle(host);
    expect(said(host)).toContain('3,923');
    expect(said(host)).toContain('50,000');
    expect(live(host).classList.contains('sr-only')).toBe(true);

    // A pan takes it away and says nothing in its place.
    store.set('status', status({status: 'loading'}));
    await settle(host);
    expect(said(host)).toBe('');
    store.set('status', status({}));
    await settle(host);
    expect(said(host)).toBe('');

    // The same result of the next query is said again.
    store.set('status', status({status: 'loading', querying: true}));
    await settle(host);
    store.set('status', status({}));
    await settle(host);
    expect(said(host)).toContain('3,923');
  });

  it('announces the highlighted count with a query’s result while a highlight is set', async () => {
    const host = await mount('<mosaica-status></mosaica-status>');
    const lit = {...view, highlighting: true, inView: {...view.inView, highlighted: {value: 812, exact: true}}};
    const store = fakeStore({status: status({status: 'loading', querying: true}), view: lit});
    (host.querySelector('mosaica-status') as MosaicaStatus).store = store;
    await settle(host);
    store.set('status', status({}));
    await settle(host);
    expect(said(host)).toContain('812');
    expect(said(host)).toContain('3,210');
    expect(said(host)).not.toContain('12,040');
  });

  it('says a query’s answer in the state’s own words where it ends in a state the strip names, and no result on a later pan', async () => {
    const host = await mount('<mosaica-status></mosaica-status>');
    const store = fakeStore({status: status({status: 'loading', querying: true}), view});
    (host.querySelector('mosaica-status') as MosaicaStatus).store = store;
    await settle(host);
    store.set('status', status({status: 'empty'}));
    await settle(host);
    expect(said(host)).not.toBe('');
    expect(said(host)).not.toContain('3,210');
    store.set('status', status({status: 'loading'}));
    await settle(host);
    store.set('status', status({}));
    await settle(host);
    expect(said(host)).not.toContain('3,210');
  });

  it('keeps each state’s button outside the live element, and moves the focus to the state cell when the button goes', async () => {
    const host = await mount('<mosaica-status></mosaica-status>');
    const el = host.querySelector('mosaica-status') as MosaicaStatus;
    el.reauthorise = () => {};
    const store = fakeStore({status: status({}), view});
    el.store = store;
    await settle(host);
    const states: [string, Parameters<typeof status>[0]][] = [
      ['refresh', {stale: true}],
      ['retry', {status: 'refused', refusal: {code: 'backpressure', detail: 'shed', status: 429}}],
      ['reauthorise', {status: 'refused', refusal: {code: 'expired-token', detail: '', status: 403}, expired: true}]
    ];
    for (const [part, over] of states) {
      store.set('status', status(over));
      await settle(host);
      const button = deep(host, `[part="${part}"]`) as HTMLButtonElement;
      expect(live(host).contains(button), part).toBe(false);
      expect(said(host), part).not.toContain(button.textContent!.trim());
      button.focus();
      expect(el.shadowRoot!.activeElement).toBe(button);
      store.set('status', status({status: 'loading'}));
      await settle(host);
      expect(deep(host, `[part="${part}"]`), part).toBeNull();
      const focused = el.shadowRoot!.activeElement;
      expect(focused?.contains(live(host)), part).toBe(true);
      expect(focused?.getAttribute('tabindex'), part).toBe('-1');
    }
  });

  it('leaves the focus where it is when a state’s button stays, or the focus is elsewhere', async () => {
    const host = await mount('<button id="other">Other</button><mosaica-status></mosaica-status>');
    const el = host.querySelector('mosaica-status') as MosaicaStatus;
    const store = fakeStore({status: status({status: 'refused', refusal: {code: 'contract', detail: 'no', status: 422}}), view});
    el.store = store;
    await settle(host);
    const retry = deep(host, '[part="retry"]') as HTMLButtonElement;
    retry.focus();
    store.set('status', status({status: 'refused', refusal: {code: 'backpressure', detail: 'shed', status: 429}}));
    await settle(host);
    expect(deep(host, '[part="retry"]')).toBe(retry);
    expect(el.shadowRoot!.activeElement).toBe(retry);

    (host.querySelector('#other') as HTMLButtonElement).focus();
    store.set('status', status({}));
    await settle(host);
    expect(document.activeElement).toBe(host.querySelector('#other'));
  });

  it('lists the card’s figures as terms and their values', async () => {
    const host = await mount('<mosaica-status expanded></mosaica-status>');
    (host.querySelector('mosaica-status') as MosaicaStatus).store = fakeStore({status: status({}), view});
    await settle(host);
    const list = deep(host, '[part="card"] dl')!;
    const terms = [...list.querySelectorAll('dt')];
    expect(terms.length).toBeGreaterThan(0);
    for (const term of terms) expect(term.nextElementSibling?.localName).toBe('dd');
    expect([...list.children].every((c) => c.localName === 'dt' || c.localName === 'dd')).toBe(true);
  });

  it('shows the matched count out of the visible count, then the shown count', async () => {
    const host = await mount('<mosaica-status></mosaica-status>');
    (host.querySelector('mosaica-status') as MosaicaStatus).store = fakeStore({status: status({}), view});
    await settle(host);
    const counts = deepAll(host, '[part="count"]');
    expect(counts.map((c) => c.textContent)).toEqual(['3,210', '12,040', '500']);
    // The shown cell carries the matched count it samples, for a host to read.
    expect(counts[2]!.getAttribute('data-total')).toBe('3,210');
  });

  it('shortens the figures and drops the shown count when compact', async () => {
    const host = await mount('<mosaica-status compact></mosaica-status>');
    (host.querySelector('mosaica-status') as MosaicaStatus).store = fakeStore({status: status({}), view: {...view, inView: {...view.inView, matched: {value: 16_822_190, exact: true}, visible: {value: 21_406_522, exact: true}}}});
    await settle(host);
    expect(deepAll(host, '[part="count"]').map((c) => c.textContent)).toEqual(['16.8M', '21.4M']);
    expect(deep(host, '[part="count-shown"]')).toBeNull();
  });

  /**
   * The highlight's count shows only under a highlight: without one `highlighted` equals `matched`
   * and the cell would repeat it. `highlighting` says whether one was asked. It reads "N highlighted
   * of M", M being the matched count.
   */
  it('adds the highlight’s own count only where a highlight was asked', async () => {
    const host = await mount('<mosaica-status></mosaica-status>');
    const el = host.querySelector('mosaica-status') as MosaicaStatus;
    el.store = fakeStore({status: status({}), view});
    await settle(host);
    expect(deep(host, '[part="count-highlighted"]')).toBeNull();

    // Nothing narrows what matches: the strip reads "812 highlighted of 3,210".
    const lit = {...view, highlighting: true, inView: {...view.inView, highlighted: {value: 812, exact: true}}};
    el.store = fakeStore({status: status({}), view: lit});
    await settle(host);
    expect(deepAll(host, '[part="count"]').map((c) => c.textContent)).toEqual(['812', '3,210', '500']);
    expect(deep(host, '[part="count-highlighted"]')).not.toBeNull();

    // A filter on the same column narrows it too, so the match cell stays beside the highlight's.
    const draft: FilterDraft = {filter: {archive: {family: 'category', keys: ['cs.LG', 'cs.CV']}}, highlight: {archive: {family: 'category', keys: ['cs.CV']}}};
    const filters = {draft, expr: composeFilters(draft, 'filter'), highlight: composeFilters(draft, 'highlight'), members: [], regions: [], suggestions: {}, suggestErrors: {}, suggestEpoch: 0};
    el.store = fakeStore({status: status({}), view: lit, filters});
    await settle(host);
    expect(deepAll(host, '[part="count"]').map((c) => c.textContent)).toEqual(['3,210', '12,040', '812', '3,210', '500']);
  });

  it('retries a refused view and signs in again on expiry through the host’s renewal', async () => {
    const host = await mount('<mosaica-status></mosaica-status>');
    const el = host.querySelector('mosaica-status') as MosaicaStatus;
    const store = fakeStore({status: status({status: 'refused', refusal: {code: 'unauthorised', detail: '', status: 403}}), view});
    el.store = store;
    await settle(host);
    (deep(host, '[part="retry"]') as HTMLButtonElement).click();
    expect(store.calls.filter((c) => c.name === 'refresh')).toHaveLength(1);
    let renewed = 0;
    el.reauthorise = () => renewed++;
    store.set('status', status({status: 'refused', refusal: {code: 'expired-token', detail: '', status: 403}, expired: true}));
    await settle(host);
    (deep(host, '[part="reauthorise"]') as HTMLButtonElement).click();
    expect(renewed).toBe(1);
  });

  it('fires mosaica-expired once, composed, on the expired transition', async () => {
    const host = await mount('<div><mosaica-status></mosaica-status></div>');
    const el = host.querySelector('mosaica-status') as MosaicaStatus;
    const store = fakeStore({status: status({}), view});
    el.store = store;
    await settle(host);
    const fired: Event[] = [];
    document.body.addEventListener('mosaica-expired', (e) => fired.push(e));
    store.set('status', status({status: 'refused', refusal: {code: 'expired-token', detail: '', status: 403}, expired: true}));
    await settle(host);
    store.set('status', status({status: 'refused', refusal: {code: 'expired-token', detail: 'again', status: 403}, expired: true}));
    await settle(host);
    expect(fired.length).toBe(1);
    expect(fired[0]!.composed).toBe(true);
  });

  it('the refresh control calls refresh() on the store', async () => {
    const host = await mount('<mosaica-status></mosaica-status>');
    const store = fakeStore({status: status({stale: true}), view});
    (host.querySelector('mosaica-status') as MosaicaStatus).store = store;
    await settle(host);
    (deep(host, '[part="refresh"]') as HTMLButtonElement).click();
    expect(store.calls.some((c) => c.name === 'refresh')).toBe(true);
  });
});

describe('<mosaica-selection> — a panel renders the states the same way', () => {
  it('names a server that did not answer or cannot be reached as the strip does, and a refusal as a refusal', async () => {
    const host = await mount('<mosaica-selection></mosaica-selection><mosaica-status></mosaica-status>');
    const store = fakeStore({status: status({}), view});
    for (const el of host.querySelectorAll('mosaica-selection, mosaica-status')) (el as unknown as {store: unknown}).store = store;
    const held = {ids: BigUint64Array.from([1n]), positions: new Float32Array(2), count: 1};
    const shape = {kind: 'box' as const, bbox: [0, 0, 1, 1] as [number, number, number, number]};
    const region = (over: object) => ({shape, status: 'loading' as const, refusal: null, visible: null, matched: {value: 0, exact: false}, served: {shown: 1, total: 0, exact: false}, verdict: null, held, ...over});
    const said = (tag: string) => {
      const state = deep(host.querySelector(tag)!.shadowRoot!, '[part="state"]');
      return state?.textContent?.replace('Retry', '').replace(state.querySelector('[part="reason"]')?.textContent ?? '', '').trim();
    };

    const gateway = {code: 'http-error', detail: 'Gateway Timeout', status: 504};
    store.set('status', status({status: 'refused', refusal: gateway}));
    store.set('region', region({status: 'refused', refusal: gateway}));
    await settle(host);
    expect(said('mosaica-selection')).toBe('Server did not answer');
    expect(said('mosaica-selection')).toBe(said('mosaica-status'));
    expect(deep(host.querySelector('mosaica-selection')!.shadowRoot!, '[part="refusal"]')).toBeNull();

    store.set('status', status({status: 'loading', unreachable: true}));
    store.set('region', region({}));
    await settle(host);
    expect(said('mosaica-selection')).toBe('Server unreachable');
    expect(said('mosaica-selection')).toBe(said('mosaica-status'));

    const closed = {code: 'fail-closed', detail: 'postings unreadable', status: 500};
    store.set('status', status({status: 'refused', refusal: closed}));
    store.set('region', region({status: 'refused', refusal: closed}));
    await settle(host);
    expect(said('mosaica-selection')).toBe('Selection refused');
  });

  it('is detached with no region, counting while loading, refused as a refusal, and inexact when the answer is a cover', async () => {
    const host = await mount('<mosaica-selection></mosaica-selection>');
    const store = fakeStore({status: status({}), view});
    const el = host.querySelector('mosaica-selection')!;
    (el as unknown as {store: unknown}).store = store;
    await settle(host);
    expect(deep(host, '[part="state"]')?.getAttribute('data-state')).toBe('detached');

    const held = {ids: BigUint64Array.from([1n, 2n]), positions: new Float32Array(4), count: 2};
    const shape = {kind: 'box' as const, bbox: [0, 0, 1, 1] as [number, number, number, number]};
    store.set('region', {shape, status: 'loading', refusal: null, visible: {value: 0, exact: false}, matched: {value: 0, exact: false}, served: {shown: 2, total: 0, exact: false}, verdict: null, held});
    await settle(host);
    expect(deep(host, '[part="state"]')?.getAttribute('data-state')).toBe('loading');
    expect(deepAll(host, '[part="count"]').length).toBe(0);

    store.set('region', {shape, status: 'refused', refusal: {code: 'contract', detail: 'bad', status: 422}, visible: {value: 0, exact: false}, matched: {value: 0, exact: false}, served: {shown: 2, total: 0, exact: false}, verdict: null, held});
    await settle(host);
    expect(deep(host, '[part="state"]')?.getAttribute('data-state')).toBe('refused');
    expect(deep(host, '[part="refusal"]')).not.toBeNull();

    store.set('region', {shape, status: 'shown', refusal: null, visible: {value: 900, exact: false}, matched: {value: 800, exact: false}, served: {shown: 2, total: 800, exact: true}, verdict: {exact: false, depth: 6}, held});
    await settle(host);
    const counts = deepAll(host, '[part="count"]');
    expect(counts.map((c) => c.textContent)).toEqual(['2', '≈ 800', '≈ 900']);
    expect(counts[0]!.getAttribute('data-total')).toBe('800');
    expect(counts[1]!.getAttribute('data-exact')).toBe('false');
    expect(deepAll(host, '[part="item"]').length).toBe(2);
    // Two verbs wait on the server (export, save); *filter to this* is the selection itself, and
    // the outside toggle is live. Why they wait is their description, as well as their tooltip.
    const greyed = deepAll(host, '[part="action"][disabled]');
    expect(greyed.length).toBe(2);
    for (const button of greyed) {
      const why = el.shadowRoot!.getElementById(button.getAttribute('aria-describedby') ?? '');
      expect(why?.textContent?.trim()).not.toBe('');
      expect(why?.textContent?.trim()).toBe(button.getAttribute('title'));
    }
  });

  const held = {ids: BigUint64Array.from([31n, 32n]), positions: new Float32Array(4), count: 5};
  const shape = {kind: 'box' as const, bbox: [0, 0, 1, 1] as [number, number, number, number]};
  const region = (over: object) => ({shape, status: 'loading' as const, refusal: null, visible: null, matched: {value: 0, exact: false}, served: {shown: 2, total: 0, exact: false}, verdict: null, held, ...over});
  const counted = region({status: 'shown', visible: {value: 900, exact: false}, matched: {value: 800, exact: false}, served: {shown: 2, total: 800, exact: true}, verdict: {exact: false, depth: 6}});

  async function card(over: object | null) {
    const host = await mount('<mosaica-selection></mosaica-selection>');
    const el = host.querySelector('mosaica-selection')!;
    const store = fakeStore({status: status({}), view, region: over as never});
    (el as unknown as {store: unknown}).store = store;
    await settle(host);
    return {host, el, store, root: el.shadowRoot!};
  }

  it('announces from one live element, there before any region is, that it is counting, the counts as they arrive, and a refusal', async () => {
    const {host, store, root} = await card(null);
    const live = root.querySelector('[part="state"] [role="status"]')!;
    expect(live.textContent?.trim()).toBe('');

    store.set('region', region({}) as never);
    await settle(host);
    expect(root.querySelector('[part="state"] [role="status"]')).toBe(live);
    const counting = live.textContent?.trim();
    expect(counting).not.toBe('');

    store.set('region', counted as never);
    await settle(host);
    expect(root.querySelector('[part="state"] [role="status"]')).toBe(live);
    expect(live.textContent).toContain('800');
    expect(live.textContent).toContain('900');
    expect(live.textContent).not.toContain(counting);

    const refusal = {code: 'contract', detail: 'the shape has too many vertices', status: 422};
    store.set('region', region({status: 'refused', refusal}) as never);
    await settle(host);
    expect(root.querySelector('[part="state"] [role="status"]')).toBe(live);
    expect(live.querySelector('[part="refusal"]')?.getAttribute('data-code')).toBe('contract');
    expect(live.textContent).toContain(refusal.detail);
  });

  it('says in words on the state line whether the counts are exact for the shape', async () => {
    const {host, store, root} = await card(counted);
    const drawn = () => [...root.querySelector('[part="state"] [role="status"]')!.childNodes].filter((n) => !(n instanceof Element && n.classList.contains('sr-only'))).map((n) => n.textContent).join('').trim();
    const approximate = drawn();
    expect(approximate).not.toBe('');
    store.set('region', {...counted, visible: {value: 900, exact: true}, matched: {value: 800, exact: true}, verdict: {exact: true, depth: null}} as never);
    await settle(host);
    expect(drawn()).not.toBe('');
    expect(drawn()).not.toBe(approximate);
  });

  it('keeps Retry outside the live element, and moves the focus to the state line when it goes', async () => {
    const busy = {code: 'backpressure', detail: 'shed', status: 429};
    const {host, el, store, root} = await card(region({status: 'refused', refusal: busy}));
    const retry = root.querySelector('[part="retry"]') as HTMLButtonElement;
    expect(root.querySelector('[role="status"]')!.contains(retry)).toBe(false);
    retry.focus();
    store.set('region', region({}) as never);
    await settle(host);
    expect(root.querySelector('[part="retry"]')).toBeNull();
    expect(el.shadowRoot!.activeElement?.getAttribute('part')).toBe('state');
  });

  it('lists the drawn marks as buttons that pick them, under the list’s visible label', async () => {
    const {store, root} = await card(counted);
    const list = root.querySelector('[part="items"]')!;
    const label = root.getElementById(list.getAttribute('aria-labelledby') ?? '');
    expect(label?.getAttribute('part')).toBe('label');
    expect(label?.textContent?.trim()).not.toBe('');
    expect(list.hasAttribute('aria-label')).toBe(false);
    const rows = [...list.querySelectorAll('[part="item"]')];
    expect(rows.map((row) => row.localName)).toEqual(['li', 'li']);
    for (const row of rows) {
      expect(row.hasAttribute('role')).toBe(false);
      expect(row.hasAttribute('tabindex')).toBe(false);
    }
    const buttons = rows.map((row) => row.querySelector('button')!);
    expect(buttons.map((b) => b.textContent?.trim())).toEqual(['31', '32']);
    buttons[1]!.click();
    expect(store.calls.filter((c) => c.name === 'pick').map((c) => c.args[0])).toEqual([32n]);
    // The marks held and not listed are said in a row with no button.
    const rest = [...list.children].filter((row) => row.getAttribute('part') !== 'item');
    expect(rest).toHaveLength(1);
    expect(rest[0]!.querySelector('button')).toBeNull();
    expect(rest[0]!.textContent).toContain('3');
  });

  it('separates the heading from the shape’s kind, and lists the counts as terms and their values', async () => {
    const {root} = await card({...counted, shape: {...shape, outside: true}});
    const title = root.querySelector('h2[part="title"]')!;
    expect(title.querySelector('button')).toBeNull();
    const kind = title.querySelector('.summary')!.textContent!;
    expect(kind).toContain('box');
    expect(title.textContent!.replace(kind, '')).toMatch(/\S\s+$/);
    const counts = root.querySelector('dl[part="counts"]')!;
    const terms = [...counts.querySelectorAll('dt')];
    expect(terms).toHaveLength(3);
    for (const term of terms) {
      expect(term.textContent?.trim()).not.toBe('');
      expect(term.nextElementSibling?.localName).toBe('dd');
      expect(term.nextElementSibling?.querySelector('mosaica-count')).not.toBeNull();
    }
  });
});
