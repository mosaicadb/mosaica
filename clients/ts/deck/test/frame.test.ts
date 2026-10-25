import {describe, expect, it, vi} from 'vitest';
import {tableFromArrays} from 'apache-arrow';
import {MosaicaError, createStore, type AggregateRequest, type AggregateResult, type MosaicaClient} from '@mosaicajs/client';
import {mortonOfTile} from '@mosaicajs/client/internal';
import {SELECTION, fakeClock, fakeScheduler, meta, response, result as viewportResult, view} from '../../core/test/support.js';
import {frameMatches} from '../src/frame.js';

/**
 * `frameMatches` against a real store over a fake client: the one request it sends, and the box it
 * reads from the answer. The client answers with the cells `cells` lists at the asked depth.
 */
async function setUp(cells: [number, number, number][], maxAggregateCells = SELECTION.maxAggregateCells, refusal: Error | null = null) {
  const asked: AggregateRequest[] = [];
  const aggregate = vi.fn(async (_token: string, req: AggregateRequest): Promise<AggregateResult> => {
    asked.push(req);
    if (refusal) throw refusal;
    const depth = req.groupings[0]!.cells!.depth;
    return {
      tables: [
        {
          grouping: 0,
          total: cells.reduce((n, c) => n + c[2], 0),
          referenceTotal: null,
          groups: null,
          sample: null,
          rows: tableFromArrays({cell: BigUint64Array.from(cells.map(([x, y]) => mortonOfTile(x, y, depth))), count: BigUint64Array.from(cells.map((c) => BigInt(c[2])))})
        }
      ],
      region: null,
      recomposed: false,
      identityKey: 'ik',
      next: null
    };
  });
  const client = {
    meta: async () =>
      meta({
        views: [view('s0', {quantisation: {xMin: 0, xMax: 512, yMin: 0, yMax: 512}})],
        filterOperands: [{column: 'year', family: 'numeric', operands: ['range']}],
        selection: {...SELECTION, maxAggregateCells}
      }),
    viewport: async () => response(viewportResult()),
    aggregate,
    close: () => {}
  } as unknown as MosaicaClient;
  const store = createStore({viewerUrl: 'http://viewer', token: 'tok', client, clock: fakeClock(), scheduler: fakeScheduler(), prefetch: false});
  return {store, asked};
}

describe('frameMatches', () => {
  it('counts the matched items by cells at depth 8, under the store’s filters, in one request', async () => {
    const {store, asked} = await setUp([[100, 60, 40]]);
    store.setFilters({filter: {year: {family: 'numeric', gte: 2020, lte: null}}, highlight: {}});
    const box = await frameMatches(store);
    expect(asked).toHaveLength(1);
    expect(asked[0]!.groupings).toEqual([{count: true, cells: {depth: 8}}]);
    expect(asked[0]!.filters).toEqual(store.requestFilters());
    expect(asked[0]!.filters).not.toBeNull();
    expect(box).toEqual([200, 120, 202, 122]);
    expect(store.get('aggregates').size).toBe(0);
  });

  it('asks no deeper than the server’s cell limit allows over the whole extent', async () => {
    // 4^5 = 1,024 cells fit a limit of 2,000; 4^6 do not.
    const {store, asked} = await setUp([[3, 4, 10]], 2000);
    const box = await frameMatches(store);
    expect(asked[0]!.groupings[0]!.cells!.depth).toBe(5);
    expect(box).toEqual([48, 64, 64, 80]);
  });

  it('gives null where nothing matches', async () => {
    const {store} = await setUp([]);
    expect(await frameMatches(store)).toBeNull();
  });

  it('gives null where the server refuses the count or cannot be reached', async () => {
    for (const refusal of [new MosaicaError(422, 'contract', 'refused'), new TypeError('Failed to fetch')]) {
      const {store, asked} = await setUp([[1, 1, 1]], SELECTION.maxAggregateCells, refusal);
      expect(await frameMatches(store)).toBeNull();
      expect(asked).toHaveLength(1);
      expect(store.get('aggregates').size).toBe(0);
    }
  });

  it('gives null and drops its registration once aborted', async () => {
    const {store} = await setUp([[1, 1, 1]]);
    const controller = new AbortController();
    const framed = frameMatches(store, {signal: controller.signal});
    controller.abort();
    expect(await framed).toBeNull();
    expect(store.get('aggregates').size).toBe(0);
  });
});
