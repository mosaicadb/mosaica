import type {Layer} from '@deck.gl/core';
import {BitmapLayer, PathLayer, SolidPolygonLayer} from '@deck.gl/layers';
import {WORLD_SIZE, gridToWorldXY, type Store, type ViewInfo} from '@mosaicajs/client';
import {gridOfData} from '@mosaicajs/client/internal';
import {MAX_TILES, frameKey, tileCover, type BasemapOptions, type BasemapValue, type Cover, type WorldColours} from './basemap.js';
import type {DensityCamera} from './density-counter.js';

/**
 * What draws a basemap, loaded by {@link basemapLayers} the first time one is drawn: the world
 * outline's reader and its layers, the tile composer, and the image.
 *
 * deck.gl finalises a layer it no longer draws and refuses one handed back after that, so every
 * call builds new layer instances. What they draw is kept: the outline placed per frame, the tiles
 * fetched, and the last composition of them, so a new instance over the same data reuses deck's
 * buffers.
 */

/** The world outline's colours per ground: a little darker than a light ground, a little lighter than a dark one. */
const WORLD_INK: Record<'light' | 'dark', WorldColours> = {
  light: {land: [0, 0, 0, 14], line: [0, 0, 0, 48]},
  dark: {land: [255, 255, 255, 12], line: [255, 255, 255, 42]}
};
/** Line widths in pixels: the coastline, and the country borders inside it. */
const COAST_PX = 1;
const BORDER_PX = 0.6;

/** The world outline in degrees, `[lon, lat, lon, lat, …]` per ring and per line. */
export type WorldOutline = {
  /** Each polygon's rings, the outer ring first and its holes after it. */
  land: Float32Array[][];
  coastline: Float32Array[];
  borders: Float32Array[];
};

const MAGIC = 'MBW1';

/**
 * Read the body of `GET /v1/basemaps/world`: the magic `MBW1`, then land as polygons, the
 * coastline as lines and the borders as lines, little-endian, each point an `i16` pair scaled to
 * ±180° and ±90°.
 */
export function parseWorld(bytes: ArrayBuffer): WorldOutline {
  const data = new DataView(bytes);
  let at = 0;
  const need = (n: number) => {
    if (at + n > data.byteLength) throw new Error(`The world outline ends after ${data.byteLength} bytes, inside its data; the server sent a truncated body.`);
  };
  need(4);
  const magic = String.fromCharCode(data.getUint8(0), data.getUint8(1), data.getUint8(2), data.getUint8(3));
  if (magic !== MAGIC) throw new Error(`The world outline is in format ${JSON.stringify(magic)}, and this client reads ${MAGIC}; use a client and a server of the same release.`);
  at = 4;
  const u32 = () => (need(4), (at += 4), data.getUint32(at - 4, true));
  const u16 = () => (need(2), (at += 2), data.getUint16(at - 2, true));
  const points = (): Float32Array => {
    const n = u32();
    need(n * 4);
    const out = new Float32Array(n * 2);
    for (let i = 0; i < n; i++) {
      out[2 * i] = (data.getInt16(at, true) * 180) / 32767;
      out[2 * i + 1] = (data.getInt16(at + 2, true) * 90) / 32767;
      at += 4;
    }
    return out;
  };
  const lines = () => Array.from({length: u32()}, points);
  const land = Array.from({length: u32()}, () => Array.from({length: u16()}, points));
  const coastline = lines();
  const borders = lines();
  if (at !== data.byteLength) throw new Error(`The world outline has ${data.byteLength - at} bytes after its borders; the server and this client disagree on its format.`);
  return {land, coastline, borders};
}

/** A position in `view`'s own coordinates (degrees on a projected view) in the 512-unit world. */
function worldOf(x: number, y: number, view: ViewInfo): [number, number] {
  return gridToWorldXY(gridOfData(x, y, view.projection, view.quantisation));
}

/** A ring or line in degrees, placed in the world as `view`'s positions are. */
function project(degrees: Float32Array, view: ViewInfo): Float32Array {
  const out = new Float32Array(degrees.length);
  for (let i = 0; i < degrees.length; i += 2) {
    const [x, y] = worldOf(degrees[i]!, degrees[i + 1]!, view);
    out[i] = x;
    out[i + 1] = y;
  }
  return out;
}

type Placed = {land: {positions: Float32Array; holeIndices: number[]}[]; coastline: Float32Array[]; borders: Float32Array[]};

/** Each body read, and its outline placed under each frame it has been drawn in. */
const worlds = new WeakMap<ArrayBuffer, {outline: WorldOutline; byFrame: Map<string, Placed>}>();

function placed(bytes: ArrayBuffer, view: ViewInfo): Placed {
  let world = worlds.get(bytes);
  if (!world) worlds.set(bytes, (world = {outline: parseWorld(bytes), byFrame: new Map()}));
  const key = frameKey(view);
  let geometry = world.byFrame.get(key);
  if (!geometry) world.byFrame.set(key, (geometry = place(world.outline, view)));
  return geometry;
}

function place(outline: WorldOutline, view: ViewInfo): Placed {
  const land = outline.land.map((rings) => {
    const parts = rings.map((ring) => project(ring, view));
    const positions = new Float32Array(parts.reduce((n, p) => n + p.length, 0));
    const holeIndices: number[] = [];
    let at = 0;
    for (const [i, part] of parts.entries()) {
      if (i > 0) holeIndices.push(at);
      positions.set(part, at);
      at += part.length;
    }
    return {positions, holeIndices};
  });
  return {land, coastline: outline.coastline.map((l) => project(l, view)), borders: outline.borders.map((l) => project(l, view))};
}

/** The layers for `value` under `view`; `value` fits `view` and is not `none`. */
export async function layersOf(store: Pick<Store, 'basemapWorld'>, view: ViewInfo, value: Exclude<BasemapValue, 'none'>, options: BasemapOptions): Promise<Layer[]> {
  if (value === 'world') return worldLayers(await store.basemapWorld(), view, options);
  if ('tiles' in value) return tileLayers(view, value.tiles, options.camera);
  return imageLayers(view, value.image, value.bounds);
}

function worldLayers(bytes: ArrayBuffer, view: ViewInfo, options: BasemapOptions): Layer[] {
  const geometry = placed(bytes, view);
  const ink = {...WORLD_INK[options.scheme ?? 'light'], ...options.colours};
  const line = (id: string, data: Float32Array[], width: number) =>
    new PathLayer({id, data, getPath: (d: Float32Array) => d, positionFormat: 'XY', getColor: ink.line, getWidth: width, widthUnits: 'pixels', pickable: false});
  return [
    new SolidPolygonLayer({
      id: 'basemap-land',
      data: geometry.land,
      getPolygon: (d: {positions: Float32Array; holeIndices: number[]}) => d,
      positionFormat: 'XY',
      getFillColor: ink.land,
      pickable: false
    }),
    line('basemap-coastline', geometry.coastline, COAST_PX),
    line('basemap-borders', geometry.borders, BORDER_PX)
  ];
}

function imageLayers(view: ViewInfo, image: string, bounds: readonly [number, number, number, number] | undefined): Layer[] {
  let box: [number, number, number, number] = [0, 0, WORLD_SIZE, WORLD_SIZE];
  if (bounds) {
    const [ax, ay] = worldOf(bounds[0], bounds[1], view);
    const [bx, by] = worldOf(bounds[2], bounds[3], view);
    box = [Math.min(ax, bx), Math.min(ay, by), Math.max(ax, bx), Math.max(ay, by)];
  }
  return [
    new BitmapLayer({
      id: 'basemap-image',
      image,
      // `[left, bottom, right, top]`. The view is `flipY`, so the image's top edge is at the smaller
      // world y and it reads upright.
      bounds: [box[0], box[3], box[2], box[1]],
      pickable: false
    })
  ];
}

/** A slippy-map tile's size in pixels. */
const TILE_PX = 256;

/** Tiles already decoded, by URL, evicted oldest first: two compositions' worth. */
const tiles = new Map<string, ImageBitmap>();
const TILES_MAX = 2 * MAX_TILES;

async function tile(url: string): Promise<ImageBitmap | null> {
  const bitmap = tiles.get(url);
  if (bitmap) return bitmap;
  const response = await fetch(url);
  if (!response.ok) return null;
  const decoded = await createImageBitmap(await response.blob());
  if (tiles.size >= TILES_MAX) tiles.delete(tiles.keys().next().value as string);
  tiles.set(url, decoded);
  return decoded;
}

/** The last composition, which a camera still showing the same tiles draws again. One is kept: an image of up to 64 tiles is 16 MB. */
let composed: {key: string; canvas: Promise<{canvas: HTMLCanvasElement; missing: number} | null>} | null = null;

/**
 * Compose the tiles `cover` names under the frame tile `frame` into one image, and count the tiles
 * missing from it. Where no tile loads at all it rejects, so the map says so.
 */
async function compose(frame: {z: number; x: number; y: number}, template: string, cover: Cover): Promise<{canvas: HTMLCanvasElement; missing: number} | null> {
  const {z, x, y} = frame;
  const nx = cover.x1 - cover.x0 + 1;
  const ny = cover.y1 - cover.y0 + 1;
  const canvas = document.createElement('canvas');
  canvas.width = nx * TILE_PX;
  canvas.height = ny * TILE_PX;
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;
  const n = 2 ** cover.level;
  let drawn = 0;
  await Promise.all(
    Array.from({length: nx * ny}, async (_, i) => {
      const [dx, dy] = [cover.x0 + (i % nx), cover.y0 + Math.floor(i / nx)];
      const url = template.replaceAll('{z}', String(z + cover.level)).replaceAll('{x}', String(x * n + dx)).replaceAll('{y}', String(y * n + dy));
      // A tile that will not load or decode leaves a hole; the rest of the composition is kept.
      const bitmap = await tile(url).catch(() => null);
      if (!bitmap) return;
      ctx.drawImage(bitmap, (dx - cover.x0) * TILE_PX, (dy - cover.y0) * TILE_PX);
      drawn += 1;
    })
  );
  if (drawn === 0) throw new Error(`none of the ${nx * ny} tiles from ${template} loaded; check the template and that the tile server answers this page.`);
  return {canvas, missing: nx * ny - drawn};
}

/**
 * The tiles over the camera's box, composed into one image so the basemap stays one layer and one
 * texture upload. Calls for the same tiles share one composition, in flight or done.
 */
async function tileLayers(view: ViewInfo, template: string, camera: DensityCamera | undefined): Promise<Layer[]> {
  if (view.tile === null || typeof document === 'undefined') return [];
  const {z, x, y} = view.tile;
  const cover = tileCover(z, camera);
  const key = `${z}/${x}/${y}|${template}|${cover.level}|${cover.x0}|${cover.y0}|${cover.x1}|${cover.y1}`;
  if (composed?.key !== key) composed = {key, canvas: compose(view.tile, template, cover)};
  const held = composed;
  const done = await held.canvas.catch((error: unknown) => {
    if (composed === held) composed = null;
    throw error;
  });
  // A composition with holes is not kept: the next call tries its tiles again.
  if (composed === held && done !== null && done.missing > 0) composed = null;
  if (!done) return [];
  const {canvas} = done;
  const span = WORLD_SIZE / 2 ** cover.level;
  return [
    new BitmapLayer({
      id: 'basemap-tiles',
      image: canvas,
      // `[left, bottom, right, top]`. The view is `flipY`, so world y runs south as tile rows do
      // and `bottom` is the larger y.
      bounds: [cover.x0 * span, (cover.y1 + 1) * span, (cover.x1 + 1) * span, cover.y0 * span],
      pickable: false
    })
  ];
}
