import {errors} from './capture-errors.js';
import {PathLayer} from '@deck.gl/layers';
import '../../src/map.js';
import type {MosaicaMap} from '../../src/map.js';
import {encodeWorld} from '../../../deck/test/world-bytes.js';
import {fakeStore, meta, status} from '../fake-store.js';

// Built by `basemap.browser.ts`. A map drawing a basemap over the fake store leaves the page for
// longer than its deck.gl canvas outlives it, comes back, and draws the basemap again in a new
// Deck; the page reports what it drew and any error deck.gl raised. It runs once with the world
// outline and once with a layer the host built, which deck.gl would refuse to draw twice.
const store = fakeStore({
  meta: meta({views: [{id: 'geo', displayName: 'geo', quantisation: {xMin: 0, xMax: 1, yMin: 0, yMax: 1}, projection: 'equirectangular', worldAspect: 2, tileScheme: null, tile: null, roster: null}]}),
  status: status({})
});
store.set('view', {...store.get('view'), id: 'geo'});
store.setWorld(encodeWorld([[[[-60, -30], [60, -30], [60, 30], [-60, 30]]]], [[[-170, 0], [170, 0]]], [[[0, -80], [0, 80]]]));

const frames = (n: number) =>
  new Promise<void>((resolve) => {
    const step = () => (n-- <= 0 ? resolve() : requestAnimationFrame(step));
    requestAnimationFrame(step);
  });
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** The basemap layers the map's Deck holds now, as it drew them last: not the map's own record of them. */
function drawnBy(map: MosaicaMap): string[] {
  const deck = (map as unknown as {deck: {layerManager: {getLayers(): {id: string}[]} | null} | null}).deck;
  return (deck?.layerManager?.getLayers() ?? []).map((l) => l.id).filter((id) => id.startsWith('basemap-') || id === 'host-basemap');
}

async function across(basemap: MosaicaMap['basemap']): Promise<{before: string[]; after: string[]}> {
  const holder = document.createElement('div');
  holder.style.cssText = 'width:400px;height:300px';
  document.body.append(holder);
  const map = document.createElement('mosaica-map') as MosaicaMap;
  map.style.setProperty('--mosaica-map-height', '300px');
  if (basemap !== null) map.basemap = basemap;
  holder.append(map);
  map.store = store;
  await wait(500);
  await frames(10);
  const before = drawnBy(map);
  map.remove();
  // Past the map's 250 ms settle, after which its Deck is finalised.
  await wait(600);
  holder.append(map);
  // A change in the store has the map draw again, in a new Deck.
  store.set('status', status({}));
  await wait(500);
  await frames(20);
  const after = drawnBy(map);
  holder.remove();
  return {before, after};
}

const world = await across(null);
const host = await across(new PathLayer({id: 'host-basemap', data: [[[0, 0], [512, 512]]], getPath: (d: [number, number][]) => d, getWidth: 2, widthUnits: 'pixels'}) as never);
(window as unknown as {result: unknown}).result = {world, host, errors};
