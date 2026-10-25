import {describe, expect, it} from 'vitest';
import type {AggregateSpec, ViewInput} from '@mosaicajs/client';
import {mortonOfTile} from '@mosaicajs/client/internal';
import '../src/map.js';
import type {MosaicaMap} from '../src/map.js';
import {aggregateEntry, answerAggregate, deep, fakeStore, meta, mount, registered, settle, status, type AggregateRow, type FakeStore} from './fake-store.js';

/**
 * The camera a map opens on and the one its Fit button moves to: framed on the items the store's
 * filters match, from a count by cells at depth 8. The fake store converts world units to data
 * coordinates as the identity, so a box the map asks for reads in world units, where a depth-8
 * cell is 2 units wide.
 */

const CELL = 512 / 256;

/** One row of a count by cells: `count` items in cell `(x, y)` at depth 8. */
const cell = (x: number, y: number, count: number): AggregateRow => ({cell: mortonOfTile(x, y, 8), count});

/** Give `map` a size of `width` × `height` px. */
function size(map: MosaicaMap, width: number, height: number): void {
  Object.defineProperty(map, 'clientWidth', {configurable: true, value: width});
  Object.defineProperty(map, 'clientHeight', {configurable: true, value: height});
}

/** A map 1000 × 800 px, given a store whose `meta` has arrived. */
async function opened(): Promise<{host: HTMLElement; map: MosaicaMap; store: FakeStore}> {
  const host = await mount('<mosaica-map></mosaica-map>');
  const map = host.querySelector('mosaica-map') as MosaicaMap;
  size(map, 1000, 800);
  const store = fakeStore({meta: meta(), status: status({})});
  store.set('view', {...store.get('view'), id: 's0'});
  map.store = store;
  await settle(host);
  return {host, map, store};
}

/** Answer the map's framing with `rows` and let the camera move. */
async function answer(host: HTMLElement, store: FakeStore, rows: AggregateRow[]): Promise<void> {
  answerAggregate(store, 'frame#', aggregateEntry([{rows}]));
  await settle(host);
}

const framings = (store: FakeStore) => [...registered(store)].filter(([id]) => id.startsWith('frame#')).map(([, spec]) => spec as AggregateSpec);
const views = (store: FakeStore) => store.calls.filter((c) => c.name === 'setView').map((c) => c.args[0] as ViewInput);

/** Whether the box the camera shows holds `[x0, y0, x1, y1]`. */
const shows = (input: ViewInput, box: [number, number, number, number]) =>
  input.bbox[0] <= box[0] && input.bbox[1] <= box[1] && input.bbox[2] >= box[2] && input.bbox[3] >= box[3];

describe('the camera a map opens on', () => {
  it('asks for the matched items by cells, and nothing else, until it can frame them', async () => {
    const {store} = await opened();
    expect(framings(store)).toEqual([{groupings: [{count: true, cells: {depth: 8}}]}]);
    expect(views(store)).toEqual([]);
  });

  it('frames the cells the matches are in, with a margin, and drops its ask', async () => {
    const {host, store} = await opened();
    await answer(host, store, [cell(100, 60, 40), cell(109, 64, 30), cell(104, 70, 20)]);
    const [input] = views(store);
    const matches: [number, number, number, number] = [100 * CELL, 60 * CELL, 110 * CELL, 71 * CELL];
    expect(shows(input!, matches)).toBe(true);
    // The box takes 92% of the map along its limiting side, here its height.
    expect((input!.bbox[3] - input!.bbox[1]) * 0.92).toBeCloseTo(matches[3] - matches[1]);
    expect(framings(store)).toEqual([]);
  });

  it('widens to take in an item far from the rest', async () => {
    const {host, store} = await opened();
    await answer(host, store, [cell(100, 60, 300), cell(110, 70, 300), cell(250, 250, 1)]);
    const [input] = views(store);
    expect(shows(input!, [100 * CELL, 60 * CELL, 251 * CELL, 251 * CELL])).toBe(true);
  });

  it('opens on the whole extent where nothing matches', async () => {
    const {host, store} = await opened();
    await answer(host, store, []);
    const [input] = views(store);
    expect(input!.bbox).toEqual([0, 0, 512, 512]);
  });

  it('opens on the whole extent where the count is refused or reaches no server', async () => {
    for (const refusal of [{code: 'contract', detail: 'refused', status: 422}, {code: 'fetch-failed', detail: 'Failed to fetch', status: null}]) {
      const {host, store} = await opened();
      answerAggregate(store, 'frame#', {status: 'refused', result: null, view: null, palette: null, refusal, summaries: []});
      await settle(host);
      expect(views(store).map((v) => v.bbox)).toEqual([[0, 0, 512, 512]]);
      expect(framings(store)).toEqual([]);
    }
  });

  it('keeps a camera moved before the count lands', async () => {
    const {host, map, store} = await opened();
    map.lookAt(10, 10);
    const moved = views(store).length;
    expect(framings(store)).toEqual([]);
    await answer(host, store, [cell(100, 60, 40)]);
    expect(views(store).length).toBe(moved);
  });

  it('stays where it is when a filter changes later', async () => {
    const {host, store} = await opened();
    await answer(host, store, [cell(100, 60, 40)]);
    const before = views(store).length;
    store.set('filters', {...store.get('filters'), expr: {category: {column: 'kind', in: ['lake']}} as never});
    await settle(host);
    expect(framings(store)).toEqual([]);
    expect(views(store).length).toBe(before);
  });
});

describe('the Fit button', () => {
  it('frames the items the filters match now', async () => {
    const {host, store} = await opened();
    await answer(host, store, [cell(100, 60, 40)]);
    deep(host, 'button[aria-label="Fit to matches"]')!.dispatchEvent(new MouseEvent('click', {bubbles: true, composed: true}));
    await settle(host);
    expect(framings(store)).toEqual([{groupings: [{count: true, cells: {depth: 8}}]}]);
    await answer(host, store, [cell(20, 200, 5), cell(30, 210, 5)]);
    const input = views(store).at(-1)!;
    expect(shows(input, [20 * CELL, 200 * CELL, 31 * CELL, 211 * CELL])).toBe(true);
    expect(input.bbox[2]).toBeLessThan(100 * CELL);
  });
});

describe('a switch to a view with another extent', () => {
  it('frames the matches in the new view', async () => {
    const {host, store} = await opened();
    await answer(host, store, [cell(100, 60, 40)]);
    const before = views(store).length;
    store.setFrame({xMin: 0, xMax: 2, yMin: 0, yMax: 2});
    store.set('view', {...store.get('view'), id: 's1'});
    await settle(host);
    expect(framings(store).length).toBe(1);
    expect(views(store).length).toBe(before);
    answerAggregate(store, 'frame#', aggregateEntry([{rows: [cell(10, 10, 4)]}], 's1'));
    await settle(host);
    expect(shows(views(store).at(-1)!, [10 * CELL, 10 * CELL, 11 * CELL, 11 * CELL])).toBe(true);
  });
});

describe('a map taken off the page while its frame is pending', () => {
  it('sends no camera, and frames the matches once it is back on the page', async () => {
    const {host, map, store} = await opened();
    map.remove();
    await answer(host, store, [cell(100, 60, 40)]);
    expect(views(store)).toEqual([]);
    expect(framings(store)).toEqual([]);
    host.append(map);
    await settle(host);
    expect(framings(store).length).toBe(1);
    await answer(host, store, [cell(100, 60, 40)]);
    const [input] = views(store);
    expect(shows(input!, [100 * CELL, 60 * CELL, 101 * CELL, 61 * CELL])).toBe(true);
    expect(input!.width).toBe(1000);
  });

  it('sends nothing into a store another map on the page shares', async () => {
    const host = await mount('<mosaica-map id="gone"></mosaica-map><mosaica-map id="kept"></mosaica-map>');
    const [gone, kept] = [...host.querySelectorAll('mosaica-map')] as MosaicaMap[];
    size(gone!, 600, 400);
    size(kept!, 1000, 800);
    const store = fakeStore({meta: meta(), status: status({})});
    store.set('view', {...store.get('view'), id: 's0'});
    gone!.store = store;
    kept!.store = store;
    await settle(host);
    expect(framings(store).length).toBe(2);
    gone!.remove();
    await answer(host, store, [cell(100, 60, 40)]);
    expect(views(store).length).toBeGreaterThan(0);
    expect(views(store).every((v) => v.width === 1000 && v.height === 800)).toBe(true);
  });
});
