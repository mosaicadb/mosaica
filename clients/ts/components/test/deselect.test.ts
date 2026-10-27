import {afterEach, describe, expect, it} from 'vitest';
import '../src/map.js';
import {fakeStore, mount, settle, status} from './fake-store.js';

/**
 * A bare `<mosaica-map>`, with no explorer round it, drops the selection itself: a click on empty
 * map and Escape each forget the pick and its ring and clear the store's selection.
 */

afterEach(() => {
  document.body.innerHTML = '';
});

type Map = HTMLElement & {
  worldAt: unknown;
  onClick(info: unknown): void;
  pickedAt: unknown;
  selectedWorldXY: [number, number] | null;
  lastPick: unknown;
  focusTarget: HTMLElement | null;
};

/** A map with a store and no deck, `worldAt` stubbed so a click carries its world point, and item 5 picked at 256, 256. */
async function picked() {
  const host = await mount('<mosaica-map></mosaica-map>');
  const map = host.querySelector('mosaica-map') as unknown as Map;
  const store = fakeStore({status: status({})});
  (map as unknown as {store: unknown}).store = store;
  map.worldAt = (x: number, y: number) => [x, y];
  await settle(host);
  map.onClick({index: 0, x: 256, y: 256, sourceLayer: {id: 'marks-p0', props: {mosaicaIds: new BigUint64Array([5n]), mosaicaPositions: new Float32Array([256, 256])}}});
  expect(map.selectedWorldXY).toEqual([256, 256]);
  return {host, map, store, cleared: () => store.calls.filter((c) => c.name === 'clearSelection').length};
}

describe('a bare map drops the selection', () => {
  it('on a click that finds nothing, and still says so with mosaica-miss', async () => {
    const {map, cleared} = await picked();
    let missed = 0;
    map.addEventListener('mosaica-miss', () => (missed += 1));
    map.onClick({index: -1, x: 900, y: 500});
    expect(cleared()).toBe(1);
    expect(map.selectedWorldXY).toBeNull();
    expect(map.pickedAt).toBeNull();
    expect(map.lastPick).toEqual({kind: 'miss'});
    expect(missed).toBe(1);
  });

  it('on Escape, once', async () => {
    const {map, cleared} = await picked();
    map.focusTarget!.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true}));
    expect(cleared()).toBe(1);
    expect(map.selectedWorldXY).toBeNull();
    expect(map.pickedAt).toBeNull();
  });

  it('but Escape clears a selected region first and keeps the pick', async () => {
    const {host, map, store, cleared} = await picked();
    store.set('region', {shape: {kind: 'box', bbox: [0, 0, 1, 1]}, status: 'loading', refusal: null, visible: null, matched: {value: 0, exact: false}, served: {shown: 0, total: 0, exact: false}, verdict: null, held: {ids: new BigUint64Array(0), positions: new Float32Array(0), count: 0}} as never);
    await settle(host);
    map.focusTarget!.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true}));
    expect(store.calls.filter((c) => c.name === 'select').map((c) => c.args[0])).toEqual([null]);
    expect(cleared()).toBe(0);
    expect(map.selectedWorldXY).toEqual([256, 256]);
  });
});
