import {tableFromArrays} from 'apache-arrow';
import {describe, expect, it, vi} from 'vitest';
import type {MosaicaClient} from '../src/client.js';
import {createStore} from '../src/store.js';
import type {AggregateRequest, AggregateResult} from '../src/types.js';
import {SELECTION, fakeClock, fakeScheduler, meta, response, result as viewportResult} from './support.js';

/**
 * `Store.titleOf`: a category value's title, asked for over everything the viewer may see, the
 * asks of one task joined, and held with `meta` until the store forgets what the server answered.
 */

const TITLES: Record<string, string> = {cs: 'Computer science', math: 'Mathematics', x: 'Ex', y: 'Why', z: 'Zed'};

const META = meta({
  filterOperands: ['archive', 'a', 'b', 'c'].map((column) => ({column, family: 'category' as const, operands: ['in' as const]})),
  selection: {...SELECTION, maxAggregateGroupings: 2}
});

/** An answer naming each grouping's value with its title, where it has one. */
function answer(req: AggregateRequest, identityKey: string): AggregateResult {
  const tables = req.groupings.map((g, grouping) => {
    const key = (g.by as {values: string[]}).values[0]!;
    const rows = tableFromArrays({group: ['listed'], key: [key], title: [TITLES[key] ?? null], count: BigInt64Array.of(1n)});
    return {grouping, total: 1, referenceTotal: null, groups: 1, sample: null, rows};
  });
  return {tables, region: null, recomposed: false, identityKey, next: null};
}

async function storeWith(opts: {meta?: () => Promise<typeof META>} = {}) {
  const clock = fakeClock();
  const keys = {identity: 'ik'};
  const pending: {req: AggregateRequest; release: () => void}[] = [];
  const aggregate = vi.fn(
    (_token: string, req: AggregateRequest, signal: AbortSignal) =>
      new Promise<AggregateResult>((resolve, reject) => {
        signal.addEventListener('abort', () => reject(signal.reason));
        pending.push({req, release: () => resolve(answer(req, keys.identity))});
      })
  );
  const client = {meta: opts.meta ?? (async () => META), viewport: async () => response(viewportResult()), aggregate, close: () => {}} as unknown as MosaicaClient;
  const store = createStore({viewerUrl: 'http://viewer', token: 'tok', client, clock, scheduler: fakeScheduler(), prefetch: false});
  await clock.advance(1);
  return {store, pending, keys};
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('Store.titleOf', () => {
  it('asks over the visible set, naming the values, joins the asks of one task up to the server’s groupings, and asks once', async () => {
    const {store, pending} = await storeWith();
    const asked = ['x', 'y', 'z'].map((key, i) => store.titleOf(['a', 'b', 'c'][i]!, key));
    await flush();
    expect(pending.map((p) => p.req)).toEqual([
      {view: 's0', groupings: [{count: true, by: {field: 'a', values: ['x']}}, {count: true, by: {field: 'b', values: ['y']}}]},
      {view: 's0', groupings: [{count: true, by: {field: 'c', values: ['z']}}]}
    ]);
    for (const p of pending) p.release();
    expect(await Promise.all(asked)).toEqual(['Ex', 'Why', 'Zed']);
    // Held: asked again, the titles come without a request.
    expect(await store.titleOf('b', 'y')).toBe('Why');
    await flush();
    expect(pending.length).toBe(2);
    store.dispose();
  });

  it('answers null for a value with no title and for a column a filter cannot name as a category, the latter with no request', async () => {
    const {store, pending} = await storeWith();
    const untitled = store.titleOf('archive', 'hep');
    expect(await store.titleOf('year', '2020')).toBeNull();
    await flush();
    expect(pending.length).toBe(1);
    pending[0]!.release();
    expect(await untitled).toBeNull();
    expect(store.heldTitle('archive', 'hep')).toBeNull();
    store.dispose();
  });

  it('drops what it holds when the store forgets, and an answer asked for before then is not kept', async () => {
    const {store, pending} = await storeWith();
    expect(store.heldTitle('archive', 'cs')).toBeUndefined();
    const first = store.titleOf('archive', 'cs');
    await flush();
    expect(store.heldTitle('archive', 'cs')).toBeUndefined();
    pending[0]!.release();
    expect(await first).toBe('Computer science');
    expect(store.heldTitle('archive', 'cs')).toBe('Computer science');

    const late = store.titleOf('archive', 'math');
    await flush();
    store.clear();
    expect(await late).toBeNull();
    expect(store.heldTitle('archive', 'cs')).toBeUndefined();
    await flush();

    // Under the next `meta` both are asked for again.
    const again = [store.titleOf('archive', 'cs'), store.titleOf('archive', 'math')];
    await flush();
    const after = pending.slice(2);
    expect(after.map((p) => p.req.groupings)).toEqual([[{count: true, by: {field: 'archive', values: ['cs']}}, {count: true, by: {field: 'archive', values: ['math']}}]]);
    after[0]!.release();
    expect(await Promise.all(again)).toEqual(['Computer science', 'Mathematics']);
    store.dispose();
  });

  it('sends one request for two asks for the same value made together', async () => {
    const {store, pending} = await storeWith();
    const both = [store.titleOf('archive', 'cs'), store.titleOf('archive', 'cs')];
    await flush();
    expect(pending.map((p) => p.req.groupings)).toEqual([[{count: true, by: {field: 'archive', values: ['cs']}}]]);
    pending[0]!.release();
    expect(await Promise.all(both)).toEqual(['Computer science', 'Computer science']);
    store.dispose();
  });

  it('empties what it holds when an answer under another identity key makes the store forget', async () => {
    const {store, pending, keys} = await storeWith();
    const first = store.titleOf('archive', 'cs');
    await flush();
    pending[0]!.release();
    expect(await first).toBe('Computer science');
    // A renewal under other terms: the next answer carries another key.
    keys.identity = 'other';
    const crossing = store.titleOf('archive', 'math');
    await flush();
    pending[1]!.release();
    expect(await crossing).toBeNull();
    await flush();
    const again = store.titleOf('archive', 'cs');
    await flush();
    expect(pending.slice(2).map((p) => p.req.groupings)).toEqual([[{count: true, by: {field: 'archive', values: ['cs']}}]]);
    pending[2]!.release();
    expect(await again).toBe('Computer science');
    store.dispose();
  });

  it('resolves null once the store is disposed, asked before or after, and while `/v1/meta` is awaited', async () => {
    const {store} = await storeWith();
    const before = store.titleOf('archive', 'cs');
    store.dispose();
    expect(await before).toBeNull();
    expect(await store.titleOf('archive', 'math')).toBeNull();

    const waiting = await storeWith({meta: () => new Promise(() => {})});
    const unanswered = waiting.store.titleOf('archive', 'cs');
    waiting.store.dispose();
    expect(await unanswered).toBeNull();
  });
});
