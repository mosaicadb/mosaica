import {readFileSync} from 'node:fs';
import {afterEach, beforeEach, describe, expect, it, vi} from 'vitest';
import type {Layer} from '@deck.gl/core';
import type {MapProjection, Quantisation, ViewInfo} from '@mosaicajs/client';
import {basemapLayers, basemapMisfit} from '../src/index.js';
import {encodeWorld, type Ring} from './world-bytes.js';

/**
 * The basemap helper a deck-only host calls: what it draws for each value, where, and what it
 * refuses. The world outline is encoded here as the server encodes `GET /v1/basemaps/world`.
 */

const SQUARE: Ring = [[-10, -10], [10, -10], [10, 10], [-10, 10]];
const HOLE: Ring = [[-5, -5], [5, -5], [5, 5], [-5, 5]];
const WORLD = encodeWorld([[SQUARE, HOLE]], [[[0, 0], [90, 0]]], [[[0, 0], [0, 45]]]);
const UNIT: Quantisation = {xMin: 0, xMax: 1, yMin: 0, yMax: 1};

function view(projection: MapProjection, over: Partial<ViewInfo> = {}): ViewInfo {
  return {id: 'v', displayName: 'v', quantisation: UNIT, projection, worldAspect: projection === 'none' ? null : 1, tileScheme: null, tile: null, roster: null, ...over};
}

function serving(answer: ArrayBuffer | Error) {
  const store = {
    asked: 0,
    basemapWorld: async () => {
      store.asked += 1;
      if (answer instanceof Error) throw answer;
      return answer;
    }
  };
  return store;
}

const props = (layer: Layer) => layer.props as unknown as Record<string, unknown>;

describe('basemapLayers', () => {
  it('draws the world outline as land, coastline and borders, placed as the view places positions', async () => {
    const layers = await basemapLayers(serving(WORLD), view('equirectangular'), 'world');
    expect(layers.map((l) => l.id)).toEqual(['basemap-land', 'basemap-coastline', 'basemap-borders']);
    const [land] = props(layers[0]!).data as {positions: Float32Array; holeIndices: number[]}[];
    // One polygon, its hole starting after the outer ring's four points.
    expect(land!.holeIndices).toEqual([8]);
    // (-10°, -10°) on an equirectangular frame over the whole world: x east from -180°, y south from 90°.
    expect(land!.positions[0]).toBeCloseTo(((170 / 360) * 512), 1);
    expect(land!.positions[1]).toBeCloseTo((100 / 180) * 512, 1);
    const [coast] = props(layers[1]!).data as Float32Array[];
    expect(Array.from(coast!).map((v) => Math.round(v))).toEqual([256, 256, 384, 256]);
  });

  it('places the outline in a regional frame by the frame, so the world reaches past it', async () => {
    // Web Mercator, the frame being the unit square's middle quarter.
    const q = {xMin: 0.25, xMax: 0.75, yMin: 0.25, yMax: 0.75};
    const layers = await basemapLayers(serving(WORLD), view('web_mercator', {id: 'r', quantisation: q}), 'world');
    const [coast] = props(layers[1]!).data as Float32Array[];
    // 0° is the frame's centre and 90° east is a quarter of the world east of it, the frame's edge.
    expect(Array.from(coast!).map((v) => Math.round(v))).toEqual([256, 256, 512, 256]);
  });

  it('builds layers of its own for each call, over the geometry already placed', async () => {
    // deck.gl finalises a layer it stops drawing and refuses it afterwards: switching to another
    // view and back, or another ground and back, needs new instances.
    const store = serving(WORLD);
    const geo = view('gall_isographic', {id: 'geo'});
    const first = await basemapLayers(store, geo, 'world');
    await basemapLayers(store, view('equirectangular', {id: 'other', quantisation: {xMin: 0.25, xMax: 0.75, yMin: 0, yMax: 1}}), 'world');
    await basemapLayers(store, geo, 'world', {scheme: 'dark'});
    const back = await basemapLayers(store, geo, 'world');
    back.forEach((layer, i) => {
      expect(layer).not.toBe(first[i]);
      expect(layer.id).toBe(first[i]!.id);
      expect(props(layer).data).toBe(props(first[i]!).data);
    });
  });

  it('gives two maps showing one value layers of their own', async () => {
    const store = serving(WORLD);
    const v = view('equirectangular', {id: 'shared'});
    const [a, b] = await Promise.all([basemapLayers(store, v, 'world'), basemapLayers(store, v, 'world')]);
    a.forEach((layer, i) => {
      expect(layer).not.toBe(b[i]);
      expect(props(layer).data).toBe(props(b[i]!).data);
    });
  });

  it('draws a dark ground lighter than a light one', async () => {
    const store = serving(WORLD);
    const v = view('equirectangular', {id: 'ground'});
    const light = props((await basemapLayers(store, v, 'world'))[0]!).getFillColor as number[];
    const dark = props((await basemapLayers(store, v, 'world', {scheme: 'dark'}))[0]!).getFillColor as number[];
    expect(dark[0]).toBeGreaterThan(light[0]!);
  });

  it('takes the colours it is given in place of its own', async () => {
    const layers = await basemapLayers(serving(WORLD), view('equirectangular', {id: 'ink'}), 'world', {colours: {land: [1, 2, 3, 4]}});
    expect(props(layers[0]!).getFillColor).toEqual([1, 2, 3, 4]);
  });

  it('refuses an outline in a format it does not read, and one cut short', async () => {
    await expect(basemapLayers(serving(encodeWorld([], [], [], 'MBW2')), view('equirectangular'), 'world')).rejects.toThrow();
    await expect(basemapLayers(serving(WORLD.slice(0, 20)), view('equirectangular'), 'world')).rejects.toThrow();
  });

  it('passes on a refused fetch', async () => {
    await expect(basemapLayers(serving(new Error('expired-token')), view('equirectangular'), 'world')).rejects.toThrow('expired-token');
  });

  it('draws an image over its bounds in the view’s coordinates, upright, or over the whole frame', async () => {
    const geo = await basemapLayers(serving(WORLD), view('equirectangular'), {image: 'map.png', bounds: [-180, -90, 0, 0]});
    expect(geo.map((l) => l.id)).toEqual(['basemap-image']);
    // `[left, bottom, right, top]`: the southern edge is the larger world y.
    expect(props(geo[0]!).bounds).toEqual([0, 512, 256, 256]);
    const flat = view('none', {quantisation: {xMin: 0, xMax: 100, yMin: 0, yMax: 100}});
    expect(props((await basemapLayers(serving(WORLD), flat, {image: 'a.png', bounds: [50, 0, 100, 25]}))[0]!).bounds).toEqual([256, 128, 512, 0]);
    expect(props((await basemapLayers(serving(WORLD), flat, {image: 'a.png'}))[0]!).bounds).toEqual([0, 512, 512, 0]);
  });

  it('draws nothing for none, and asks for no outline', async () => {
    const store = serving(WORLD);
    expect(await basemapLayers(store, view('equirectangular'), 'none')).toEqual([]);
    expect(store.asked).toBe(0);
  });

  it('refuses a value that does not fit the view', async () => {
    await expect(basemapLayers(serving(WORLD), view('none'), 'world')).rejects.toThrow();
    await expect(basemapLayers(serving(WORLD), view('web_mercator'), {tiles: 'https://t.example/{z}/{x}/{y}.png', attribution: 'T'})).rejects.toThrow();
  });
});

describe('basemapMisfit', () => {
  const tiled = view('web_mercator', {tileScheme: 'xyz', tile: {z: 0, x: 0, y: 0}});
  it('fits world to a projected view, tiles to a tiled one, and an image or none to any', () => {
    expect(basemapMisfit(view('equirectangular'), 'world')).toBeNull();
    expect(basemapMisfit(tiled, {tiles: 'https://t.example/{z}/{x}/{y}.png', attribution: 'T'})).toBeNull();
    expect(basemapMisfit(view('none'), {image: 'a.png'})).toBeNull();
    expect(basemapMisfit(view('none'), 'none')).toBeNull();
  });
  it('says why world needs a projection and tiles a tile scheme', () => {
    expect(basemapMisfit(view('none'), 'world')).toEqual(expect.any(String));
    expect(basemapMisfit(view('equirectangular'), {tiles: 'https://t.example/{z}/{x}/{y}.png'})).toEqual(expect.any(String));
  });
});

describe('basemapLayers refuses what is not a basemap', () => {
  const tiled = view('web_mercator', {tileScheme: 'xyz', tile: {z: 0, x: 0, y: 0}});
  it.each([
    ['a template without its placeholders', {tiles: 'https://t.example/tile.png'}],
    ['bounds that are not four numbers', {image: 'a.png', bounds: [0, 0, 1]}],
    ['a name it does not know', 'osm'],
    ['a tile basemap with a key it does not take', {tiles: 'https://t.example/{z}/{x}/{y}.png', attribution: 'T', maxzoom: 12}],
    ['an image basemap with a key it does not take', {image: 'a.png', bounds: [0, 0, 1, 1], opacity: 0.5}]
  ])('%s', async (_, value) => {
    await expect(basemapLayers(serving(WORLD), tiled, value as never)).rejects.toThrow();
  });
});

describe('the tile composer', () => {
  // Each test its own server, since tiles fetched are kept across calls.
  let server = 0;
  let TEMPLATE = '';
  const tiled = view('web_mercator', {id: 'tiled', tileScheme: 'xyz', tile: {z: 0, x: 0, y: 0}});
  let fetched: string[] = [];

  beforeEach(() => {
    fetched = [];
    TEMPLATE = `https://tiles${++server}.example/{z}/{x}/{y}.png`;
    // A canvas, a fetch and a decoder, enough to compose without a browser.
    vi.stubGlobal('document', {createElement: () => ({width: 0, height: 0, getContext: () => ({drawImage: () => {}})})});
    vi.stubGlobal('fetch', async (url: string) => {
      fetched.push(url);
      return new Response(new Blob([new Uint8Array(1)]));
    });
    vi.stubGlobal('createImageBitmap', async () => ({}));
  });
  afterEach(() => vi.unstubAllGlobals());

  const tiles = (camera?: {target: number[]; zoom: number; width: number; height: number}) =>
    basemapLayers(serving(WORLD), tiled, {tiles: TEMPLATE, attribution: 'T'}, camera ? {camera} : {});
  const levels = () => new Set(fetched.map((u) => u.split('/')[3]));

  it('composes the whole frame three levels down before the camera has moved', async () => {
    const [layer] = await tiles();
    expect(props(layer!).bounds).toEqual([0, 512, 512, 0]);
    expect(fetched).toHaveLength(64);
    expect(levels()).toEqual(new Set(['3']));
  });

  it('composes the tiles over the camera’s box at its depth, and again only when they change', async () => {
    // Zoom 2 over 512 × 512 pixels shows world units 192 to 320: tiles 3 to 5 of 8 on each axis.
    const camera = {target: [256, 256], zoom: 2, width: 512, height: 512};
    const [layer] = await tiles(camera);
    expect(props(layer!).bounds).toEqual([192, 384, 384, 192]);
    expect(fetched.map((u) => u.split('/').slice(3).join('/')).sort()).toEqual(['3/3/3.png', '3/3/4.png', '3/3/5.png', '3/4/3.png', '3/4/4.png', '3/4/5.png', '3/5/3.png', '3/5/4.png', '3/5/5.png']);
    // A small pan that shows the same tiles reuses the composition.
    fetched = [];
    const [again] = await tiles({...camera, target: [258, 258]});
    expect(fetched).toEqual([]);
    expect(props(again!).image).toBe(props(layer!).image);
    // A pan onto other tiles composes them.
    await tiles({...camera, target: [100, 100]});
    expect(fetched.length).toBeGreaterThan(0);
  });

  it('leaves a hole for a tile that will not load or decode, and draws the rest', async () => {
    let drawnTiles = 0;
    vi.stubGlobal('document', {createElement: () => ({width: 0, height: 0, getContext: () => ({drawImage: () => (drawnTiles += 1)})})});
    vi.stubGlobal('fetch', async (url: string) => {
      fetched.push(url);
      if (url.endsWith('/3/0/0.png')) throw new TypeError('fetch failed');
      return new Response(new Blob([new Uint8Array(1)]));
    });
    vi.stubGlobal('createImageBitmap', async (blob: Blob) => {
      if (fetched.length === 2 && blob) throw new DOMException('cannot decode', 'InvalidStateError');
      return {};
    });
    const [layer] = await tiles();
    expect(layer!.id).toBe('basemap-tiles');
    expect(fetched).toHaveLength(64);
    expect(drawnTiles).toBeGreaterThanOrEqual(62);
    expect(drawnTiles).toBeLessThan(64);
  });

  it('rejects where no tile loads, and tries the missing tiles again on the next call', async () => {
    vi.stubGlobal('fetch', async (url: string) => {
      fetched.push(url);
      return new Response('', {status: 503});
    });
    await expect(tiles()).rejects.toThrow();
    // Now one tile is missing: the composition is drawn, and not kept.
    fetched = [];
    vi.stubGlobal('fetch', async (url: string) => {
      fetched.push(url);
      return url.endsWith('/3/0/0.png') ? new Response('', {status: 503}) : new Response(new Blob([new Uint8Array(1)]));
    });
    expect((await tiles()).map((l) => l.id)).toEqual(['basemap-tiles']);
    expect(fetched).toHaveLength(64);
    fetched = [];
    await tiles();
    expect(fetched.map((u) => u.split('/').slice(3).join('/'))).toEqual(['3/0/0.png']);
  });

  it('drops a level where the box needs more than 64 tiles', async () => {
    // Zoom 4 over 4000 pixels would be 16 × 16 tiles at level 5; level 4 needs 8 × 8.
    const [layer] = await tiles({target: [256, 256], zoom: 4, width: 4000, height: 4000});
    expect(levels()).toEqual(new Set(['4']));
    expect(fetched).toHaveLength(64);
    expect(props(layer!).bounds).toEqual([128, 384, 384, 128]);
  });
});

describe('the world outline the server ships', () => {
  it('decodes whole: land, coastlines and borders, ending where the body ends', async () => {
    const asset = readFileSync(new URL('../../../../crates/mosaica-server/assets/world.mbw', import.meta.url));
    const bytes = asset.buffer.slice(asset.byteOffset, asset.byteOffset + asset.byteLength);
    // A body with bytes left over, or cut short, is refused, so decoding at all means it ended exactly.
    const [land, coast, borders] = await basemapLayers(serving(bytes), view('equirectangular', {id: 'shipped'}), 'world');
    const count = (layer: Layer | undefined) => (props(layer!).data as unknown[]).length;
    // Natural Earth 1:50m: about 1,400 land polygons and coastlines and 400 border lines.
    expect(count(land)).toBe(new DataView(bytes).getUint32(4, true));
    expect(count(land)).toBeGreaterThan(1_000);
    expect(count(coast)).toBeGreaterThan(1_000);
    expect(count(borders)).toBeGreaterThan(200);
    await expect(basemapLayers(serving(bytes.slice(0, -1)), view('equirectangular', {id: 'cut'}), 'world')).rejects.toThrow();
  });
});
