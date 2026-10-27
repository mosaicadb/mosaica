import {Table, tableToIPC} from 'apache-arrow';
import {describe, expect, it} from 'vitest';
import {MosaicaClient} from '../src/client.js';
import type {FilterDraft} from '../src/filters.js';
import {withMember} from '../src/members.js';
import {createStore, type StoreOptions} from '../src/store.js';
import type {FilterExpr} from '../src/types.js';
import {camera, fakeClock, fakeScheduler, framed, meta, response, result, u64, view} from './support.js';

/**
 * `Store.items` against a fake server: a real `MosaicaClient` whose `fetch` answers `/v1/items`
 * with a framed body, page by page from a cursor, and whose map verbs answer an empty frame.
 */

const META = meta({
  views: [view('s0')],
  filterOperands: [{column: 'archive', family: 'category', operands: ['in']}]
});

const CS: FilterDraft = {filter: {archive: {family: 'category', keys: ['cs']}}, highlight: {}};
const BOX: [number, number, number, number] = [0.1, 0.2, 0.6, 0.7];

const json = (kind: number, value: unknown) => ({kind, payload: new TextEncoder().encode(JSON.stringify(value))});

/** One response of one page of `ids`, ending at `next`. */
function itemsBody(ids: bigint[], next: string | null): Response {
  return new Response(
    framed([
      json(6, {order: 'map', page_rows: 2, visible: 5, matched: 3}),
      {kind: 7, payload: tableToIPC(new Table({mosaica_id: u64(ids)}), 'stream')},
      json(8, {next, ended_by: next === null ? 'end' : 'pages'}),
      json(4, {pages: 1, rows: ids.length, next, ended_by: next === null ? 'end' : 'pages', stream_us: 1})
    ])
  );
}

/** A store over the fake server, the items requests it sends, and the filters each viewport request carried. */
async function storeWith(auth: Pick<StoreOptions, 'token' | 'authorise'> = {token: 'tok'}) {
  const clock = fakeClock();
  const scheduler = fakeScheduler();
  const sent: {url: string; auth: string | null; body: Record<string, unknown>}[] = [];
  const client = new MosaicaClient({
    viewerUrl: 'http://viewer',
    fetch: (async (url: string, init?: RequestInit) => {
      const body = JSON.parse(init!.body as string) as Record<string, unknown>;
      sent.push({url, auth: new Headers(init?.headers).get('authorization'), body});
      return body.cursor === 'c1' ? itemsBody([3n], null) : itemsBody([1n, 2n], 'c1');
    }) as typeof fetch
  });
  const mapped: (FilterExpr | undefined)[] = [];
  Object.assign(client, {
    meta: async () => META,
    viewport: async (_token: string, req: {filters?: FilterExpr}) => {
      mapped.push(req.filters);
      return response(result());
    },
    // The counts in view, which these tests do not read, are left unanswered.
    aggregate: () => new Promise(() => {})
  });
  const store = createStore({...auth, client, clock, scheduler, prefetch: false});
  await clock.advance(1);
  return {store, client, sent, mapped, clock, scheduler};
}

const ids = (table: Table) => Array.from(table.getChild('mosaica_id')!.toArray() as BigUint64Array);

describe('Store.items', () => {
  it('reads with the store’s token, view and the filters the map applies, and pages on from the cursor', async () => {
    const {store, sent, mapped, clock, scheduler} = await storeWith();
    store.setFilters(CS);
    store.setMembers(withMember([], {layer: 'clusters', artifact: 7n, outside: true, verb: 'filter'}));
    store.select({kind: 'box', bbox: BOX});
    store.setView(camera(store.frame(), [0, 0, 1, 1], 400, 400));
    await clock.advance(600);
    scheduler.flush();

    const read = await store.items({fields: ['title'], pageRows: 2, count: true});
    const applied = mapped.at(-1)!;
    // The archive control, the member clause outside the artifact, and the region.
    expect(JSON.stringify(applied)).toContain('none_of');
    expect(JSON.stringify(applied)).toContain('member_of');
    expect(JSON.stringify(applied)).toContain('region');
    expect(JSON.stringify(applied)).toContain('archive');
    expect(sent[0]).toEqual({
      url: 'http://viewer/v1/items',
      auth: 'Bearer tok',
      body: {view: 's0', fields: ['title'], page_rows: 2, count: true, filters: applied}
    });
    expect(read.head.matched).toBe(3);

    const first = await read.next();
    expect(ids(first.value!)).toEqual([1n, 2n]);
    expect(read.cursor).toBe('c1');
    const second = await read.next();
    expect(ids(second.value!)).toEqual([3n]);
    expect(sent[1]!.body.cursor).toBe('c1');
    expect((await read.next()).done).toBe(true);
  });

  it('limits the read to the camera’s box with `inView`, and to the selected region where one is selected', async () => {
    const {store, sent, clock, scheduler} = await storeWith();
    store.setView(camera(store.frame(), BOX, 400, 400));
    await clock.advance(600);
    scheduler.flush();

    await store.items({fields: [], inView: true});
    expect(sent.at(-1)!.body.filters).toEqual({region: {bbox: BOX}});

    store.setFilters(CS);
    await store.items({fields: [], inView: true});
    expect(sent.at(-1)!.body.filters).toEqual({all_of: [store.requestFilters(), {region: {bbox: BOX}}]});

    // A selected region is the area: it is in the filters once, not joined to the camera's box.
    const lasso: [number, number][] = [
      [0.2, 0.2],
      [0.4, 0.2],
      [0.3, 0.4]
    ];
    store.select({kind: 'lasso', points: lasso});
    await store.items({fields: [], inView: true});
    expect(sent.at(-1)!.body.filters).toEqual(store.requestFilters());
    expect(JSON.stringify(sent.at(-1)!.body.filters)).not.toContain('bbox');
  });

  it('reads every visible item for `filters: null`, and the caller’s own filters as given', async () => {
    const {store, sent} = await storeWith();
    store.setFilters(CS);
    await store.items({fields: [], filters: null});
    expect(sent.at(-1)!.body).toEqual({view: 's0', fields: []});
    await store.items({fields: [], filters: {year: {range: {gte: 2020}}}, view: 'other'});
    expect(sent.at(-1)!.body).toEqual({view: 'other', fields: [], filters: {year: {range: {gte: 2020}}}});
  });

  it('sends the renewed token once the store has renewed it', async () => {
    let issued = 0;
    const {store, sent, clock} = await storeWith({authorise: async () => ({token: `t${++issued}`, expiresAt: (Date.now() + 60_000) / 1000})});
    await store.items({fields: []});
    expect(sent.at(-1)!.auth).toBe('Bearer t1');
    // A minute's token is renewed within its minute.
    await clock.advance(59_000);
    expect(issued).toBe(2);
    await store.items({fields: []});
    expect(sent.at(-1)!.auth).toBe('Bearer t2');
  });

  it('takes `filters: undefined` as not given, the map’s filters, in items and browse alike', async () => {
    const {store, client, sent} = await storeWith();
    const browsed: unknown[] = [];
    Object.assign(client, {
      browse: async (_token: string, req: {filters?: FilterExpr | null}) => {
        browsed.push(req.filters);
        return {artifacts: [], parents: [], next: null};
      }
    });
    store.setFilters(CS);
    await store.items({fields: [], filters: undefined});
    await store.browse({layer: 'clusters', filters: undefined});
    expect(sent.at(-1)!.body.filters).toEqual(store.requestFilters());
    expect(browsed).toEqual([store.requestFilters()]);
    // Only `null` reads everything visible.
    await store.items({fields: [], filters: null});
    await store.browse({layer: 'clusters', filters: null});
    expect(sent.at(-1)!.body.filters).toBeUndefined();
    expect(browsed.at(-1)).toBeNull();
  });

  it('waits with `inView` until the camera is set, then reads its box', async () => {
    const {store, sent, clock} = await storeWith();
    const read = store.items({fields: [], inView: true});
    await clock.advance(1_000);
    expect(sent).toHaveLength(0);
    store.setView(camera(store.frame(), BOX, 400, 400));
    await read;
    expect(sent).toHaveLength(1);
    expect(sent[0]!.body.filters).toEqual({region: {bbox: BOX}});
  });

  it('rejects an `inView` read waiting for the camera when the store is disposed, and sends nothing', async () => {
    const {store, sent, clock} = await storeWith();
    const read = store.items({fields: [], inView: true});
    await clock.advance(1);
    store.dispose();
    await expect(read).rejects.toMatchObject({name: 'AbortError'});
    expect(sent).toHaveLength(0);
  });

  it('rejects an `inView` read waiting for the camera with the reason its signal aborts with, and sends nothing', async () => {
    const {store, sent, clock} = await storeWith();
    const abort = new AbortController();
    const read = store.items({fields: [], inView: true}, abort.signal);
    await clock.advance(1);
    const reason = new Error('the reader left');
    abort.abort(reason);
    await expect(read).rejects.toBe(reason);
    store.setView(camera(store.frame(), BOX, 400, 400));
    await clock.advance(1);
    expect(sent).toHaveLength(0);
  });

  it('reads the selected region where one is selected while an `inView` read waits', async () => {
    const {store, sent, clock} = await storeWith();
    const read = store.items({fields: [], inView: true});
    await clock.advance(1);
    store.select({kind: 'box', bbox: BOX});
    await read;
    expect(sent).toHaveLength(1);
    expect(sent[0]!.body.filters).toEqual(store.requestFilters());
    expect(JSON.stringify(sent[0]!.body.filters)).toContain('region');
  });

  it('sends the token of the viewer the store serves when the camera arrives, after a change of principal', async () => {
    let principal = 'ann';
    const {store, sent, clock} = await storeWith({authorise: async () => ({token: `tok-${principal}`, expiresAt: (Date.now() + 3_600_000) / 1000})});
    const at = camera(store.frame(), BOX, 400, 400);
    const read = store.items({fields: [], inView: true});
    await clock.advance(1);
    principal = 'bob';
    store.clear();
    await clock.advance(1);
    store.setView(at);
    await read;
    expect(sent).toHaveLength(1);
    expect(sent[0]!.auth).toBe('Bearer tok-bob');
  });

  it('sends the filters set while an `inView` read waits for the camera', async () => {
    const {store, sent, clock} = await storeWith();
    const read = store.items({fields: [], inView: true});
    await clock.advance(1);
    store.setFilters(CS);
    store.setView(camera(store.frame(), BOX, 400, 400));
    await read;
    expect(sent).toHaveLength(1);
    expect(sent[0]!.body.filters).toEqual({all_of: [store.requestFilters(), {region: {bbox: BOX}}]});
  });

  it('refuses `inView` for another view than the current one at once, camera or not', async () => {
    const {store, sent, clock, scheduler} = await storeWith();
    await expect(store.items({fields: [], inView: true, view: 'other'})).rejects.toThrow();
    store.setView(camera(store.frame(), BOX, 400, 400));
    await clock.advance(600);
    scheduler.flush();
    await expect(store.items({fields: [], inView: true, view: 'other'})).rejects.toThrow();
    expect(sent).toHaveLength(0);
  });
});

describe('the URLs a store and a client need', () => {
  it('builds a store from a client alone, and refuses one given neither a client nor a viewer URL', async () => {
    const {store, sent} = await storeWith();
    await store.items({fields: []});
    expect(sent[0]!.url).toBe('http://viewer/v1/items');
    expect(() => createStore({token: 'tok'})).toThrow();
  });

  it('refuses a session route on a client built without a session URL, and sends nothing', async () => {
    const sent: string[] = [];
    const client = new MosaicaClient({
      viewerUrl: 'http://viewer',
      fetch: (async (url: string) => {
        sent.push(url);
        return new Response(JSON.stringify({token: 't', token_id: 1, expires_at: 0}));
      }) as typeof fetch
    });
    await expect(client.authorise({principal: 'ann'})).rejects.toThrow();
    await expect(client.revoke(7)).rejects.toThrow();
    expect(sent).toEqual([]);
  });
});
