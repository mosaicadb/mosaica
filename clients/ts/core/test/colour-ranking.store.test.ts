import {tableFromArrays} from 'apache-arrow';
import {describe, expect, it, vi} from 'vitest';
import {MosaicaError, type MosaicaClient} from '../src/client.js';
import {createStore} from '../src/store.js';
import type {AggregateRequest, AggregateResult} from '../src/types.js';
import {SELECTION, camera, fakeClock, fakeScheduler, meta, response, scalar, servedResult, settle, tile} from './support.js';

/**
 * The colour ranking the legend asks the store for: sent as registered aggregates are, so a
 * server still starting is asked again, and no larger than the server's limit on `top`.
 */
describe('the colour ranking', () => {
  const META = meta({
    declaredScalars: [scalar('archive', 'u16', {category: {vocabulary: 'a', kind: 'declared', visibility: 'public'}, render: true, homes: ['rendered']})],
    selection: {...SELECTION, maxAggregateTop: 8}
  });
  const NAMES: Record<number, string> = {5: 'cs', 7: 'math'};

  it('is asked again after a 503, within the server’s top, and ranks by its answer', async () => {
    const asked: AggregateRequest[] = [];
    const aggregate = vi.fn(async (_token: string, req: AggregateRequest): Promise<AggregateResult> => {
      const ranking = req.groupings.some((g) => g.by !== undefined && 'field' in g.by);
      if (ranking) asked.push(req);
      if (ranking && asked.length === 1) throw new MosaicaError(503, 'not-ready', 'starting');
      const rows = tableFromArrays({group: ['listed', 'listed'], key: ['math', 'cs'], count: BigInt64Array.of(9n, 4n)});
      return {tables: [{grouping: 0, total: 13, referenceTotal: null, groups: 2, sample: null, rows}], region: null, recomposed: false, identityKey: 'ik', next: null};
    });
    const client = {
      meta: async () => META,
      viewport: async () => response(servedResult(3, [tile(0n, 3n)], {scalars: {archive: {arrowType: 'u16', values: Uint16Array.of(5, 5, 7)}}})),
      categories: async (_t: string, _c: string, opts: {codes?: number[]}) => (opts.codes ?? []).map((code) => ({code, key: NAMES[code]!, title: null})),
      aggregate,
      close: () => {}
    } as unknown as MosaicaClient;
    const clock = fakeClock();
    const scheduler = fakeScheduler();
    const store = createStore({viewerUrl: 'http://viewer', token: 'tok', client, clock, scheduler, prefetch: false, replica: {revalidateAfterMs: Infinity}});
    await clock.advance(1);
    store.setColourBy('archive');
    store.setView(camera(store.frame(), [0, 0, 1, 1], 400, 400));
    await clock.advance(600);
    scheduler.flush();
    await settle();
    await clock.advance(10_000);
    scheduler.flush();
    await settle();
    expect(asked.length).toBe(2);
    expect(asked[0]!.groupings).toEqual([{count: true, by: {field: 'archive', top: 8}}]);
    expect(store.get('legend').ranks['archive']).toEqual({7: 0, 5: 1});
    store.dispose();
  });
});
