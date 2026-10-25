import type {Layer} from '@deck.gl/core';
import {WORLD_SIZE, type Rgba, type Store, type ViewInfo} from '@mosaicajs/client';
import {worldBbox} from '@mosaicajs/client/internal';
import type {DensityCamera} from './density-counter.js';
import {importBasemap} from './lazy.js';

/**
 * What is drawn under the points of one view:
 *
 * - `'world'`: land, coastlines and country borders from the server's world outline
 *   (`GET /v1/basemaps/world`), projected as the view's positions were. Only on a view whose
 *   projection is not `none`.
 * - `'none'`: nothing.
 * - `{tiles, attribution}`: raster tiles from `tiles`, a URL template with `{z}`, `{x}` and `{y}`,
 *   composed at the camera's depth. `attribution` is the credit the tile server asks for. Only on a
 *   view with a {@link ViewInfo.tileScheme}. There is no default server.
 * - `{image, bounds}`: one image stretched over `bounds`, `[x0, y0, x1, y1]` in the view's own
 *   coordinates (longitude and latitude in degrees on a geographic view, whose corners are
 *   projected; x and y otherwise), or over the whole frame where `bounds` is left out. The image is
 *   not reprojected.
 *
 * @category Basemaps
 */
export type BasemapValue = 'world' | 'none' | {tiles: string; attribution?: string} | {image: string; bounds?: readonly [number, number, number, number]};

/**
 * The colours of the world outline, each RGBA from 0 to 255.
 *
 * @category Basemaps
 */
export type WorldColours = {
  /** The land's fill. */
  land: Rgba;
  /** The coastlines and the country borders. */
  line: Rgba;
};

/**
 * Options for {@link basemapLayers}.
 *
 * @category Basemaps
 */
export type BasemapOptions = {
  /**
   * The ground the map draws on. The world outline is a little darker than a light ground and a
   * little lighter than a dark one. Defaults to `light`.
   */
  scheme?: 'light' | 'dark';
  /** Colours in place of the world outline's own for `scheme`. */
  colours?: Partial<WorldColours>;
  /**
   * The camera, for tiles: they are composed for the box it shows at its depth. Without it the
   * whole frame is composed at a coarse depth.
   */
  camera?: DensityCamera;
};

/**
 * What places a view's positions for a basemap: its projection and its quantisation, from which
 * its tile follows. Two views with the same key draw a basemap alike.
 *
 * @internal
 */
export function frameKey(view: ViewInfo): string {
  const q = view.quantisation;
  return `${view.projection}|${q.xMin}|${q.xMax}|${q.yMin}|${q.yMax}`;
}

/** The most tiles one composition fetches; a box needing more is drawn one level coarser. @internal */
export const MAX_TILES = 64;

/** The deepest XYZ level asked for. */
const MAX_LEVEL = 19;

/** Which tiles a composition covers. @internal */
export type Cover = {level: number; x0: number; y0: number; x1: number; y1: number};

/**
 * The tiles to compose for a camera: a depth below the frame's own tile, and the range of the
 * frame's subdivision the box touches. The depth is the zoom plus one, since a world tile at zoom
 * `z` is 512 screen pixels and an `xyz` image is 256. With no camera, the whole frame three levels
 * down, cheap enough to draw before the camera has moved.
 *
 * @internal
 */
export function tileCover(frameZ: number, camera: DensityCamera | undefined): Cover {
  if (!camera) return {level: 3, x0: 0, y0: 0, x1: 7, y1: 7};
  const box = worldBbox({target: [camera.target[0] ?? 0, camera.target[1] ?? 0], zoom: camera.zoom, width: camera.width, height: camera.height}, 1);
  let level = Math.max(0, Math.min(MAX_LEVEL - frameZ, Math.round(camera.zoom) + 1));
  for (;;) {
    const span = WORLD_SIZE / 2 ** level;
    const last = 2 ** level - 1;
    const clamp = (v: number) => Math.max(0, Math.min(last, Math.floor(v / span)));
    const [x0, y0, x1, y1] = [clamp(box[0]), clamp(box[1]), clamp(box[2]), clamp(box[3])];
    if ((x1 - x0 + 1) * (y1 - y0 + 1) <= MAX_TILES || level === 0) return {level, x0, y0, x1, y1};
    level -= 1;
  }
}

/** A value as a refusal names it: as JSON where it can be written so, else as text. */
function shown(value: unknown): string {
  try {
    return JSON.stringify(value) ?? String(value);
  } catch {
    return Object.prototype.toString.call(value);
  }
}

/**
 * Why `value` is not one of the {@link BasemapValue} forms, in a sentence that says what to write
 * instead, or `null` where it is one. It says nothing of whether the value fits a view.
 *
 * @internal
 */
export function basemapValueError(value: unknown): string | null {
  if (value === 'none' || value === 'world') return null;
  if (typeof value === 'object' && value !== null) {
    const given = Object.entries(withoutUndefined(value));
    for (const [form, keys, check] of FORMS) {
      if (!given.some(([k]) => k === form)) continue;
      const extra = given.find(([k]) => !keys.includes(k));
      if (extra) return `A${form === 'image' ? 'n image' : ' tile'} basemap takes ${keys.join(' and ')}, and ${JSON.stringify(extra[0])} is not one of them.`;
      return check(Object.fromEntries(given));
    }
  }
  return `A basemap is "world", "none", {tiles, attribution} or {image, bounds}, not ${shown(value)}.`;
}

/**
 * `value` with its undefined-valued keys left out, as JSON leaves them out, where it is an object;
 * anything else unchanged.
 *
 * @internal
 */
export function withoutUndefined<T>(value: T): T {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return value;
  return Object.fromEntries(Object.entries(value).filter(([, v]) => v !== undefined)) as T;
}

/** The two object forms: the key that names each, the keys it takes, and what each must hold. */
const FORMS: readonly [string, readonly string[], (value: Record<string, unknown>) => string | null][] = [
  [
    'tiles',
    ['tiles', 'attribution'],
    ({tiles, attribution}) => {
      if (typeof tiles !== 'string' || !['{z}', '{x}', '{y}'].every((t) => tiles.includes(t))) {
        return 'A tile basemap names its tiles as a URL template holding {z}, {x} and {y}, such as https://tiles.example.org/{z}/{x}/{y}.png.';
      }
      return attribution !== undefined && typeof attribution !== 'string' ? "A tile basemap's attribution is the text of the credit the tile server asks for." : null;
    }
  ],
  [
    'image',
    ['image', 'bounds'],
    ({image, bounds}) => {
      if (typeof image !== 'string' || image === '') return "An image basemap's image is the image's URL.";
      if (bounds !== undefined && !(Array.isArray(bounds) && bounds.length === 4 && bounds.every((b) => typeof b === 'number' && Number.isFinite(b)))) {
        return "An image basemap's bounds are four numbers, [x0, y0, x1, y1], in the view's coordinates; leave them out to cover the whole frame.";
      }
      return null;
    }
  ]
];

/**
 * Why `value` cannot be drawn under `view`, in a sentence that says what to write instead, or
 * `null` where it can: the world outline needs a map projection, and tiles a
 * {@link ViewInfo.tileScheme}.
 *
 * @category Basemaps
 */
export function basemapMisfit(view: ViewInfo, value: BasemapValue): string | null {
  if (value === 'world' && view.projection === 'none') {
    return `View ${view.id} has no map projection, so the world outline cannot be drawn under it; use "none" or an image.`;
  }
  if (typeof value === 'object' && 'tiles' in value && view.tileScheme === null) {
    const instead = view.projection === 'none' ? '"none" or an image' : '"world" or an image';
    return `View ${view.id}'s frame does not line up with web map tiles, so tiles cannot be drawn under it; use ${instead}.`;
  }
  return null;
}

/**
 * The deck.gl layers that draw `value` under `view`, in the 512-unit world {@link MosaicaLayer}
 * draws in: put them before it in the `Deck`'s `layers`. `'none'` is no layers. The code that
 * draws a basemap is loaded the first time this is called, so a page that draws none does not load
 * it.
 *
 * The world outline is fetched once per server ({@link Store.basemapWorld}) and placed once per
 * frame. Each call returns new layer instances over the geometry already placed, so deck.gl keeps
 * its buffers and every map gets layers of its own; for tiles, a camera that still shows the same
 * tiles draws the image already composed.
 *
 * @param store - The store whose server serves the world outline.
 * @param view - The view drawn, from `meta.views`.
 * @param value - What to draw.
 * @throws `Error` where `value` is not one of the forms or does not fit `view` ({@link basemapMisfit}), and where the world
 *   outline does not load or is in a format this client does not read.
 *
 * @category Basemaps
 */
export async function basemapLayers(store: Pick<Store, 'basemapWorld'>, view: ViewInfo, value: BasemapValue, options: BasemapOptions = {}): Promise<Layer[]> {
  value = withoutUndefined(value);
  const misfit = basemapValueError(value) ?? basemapMisfit(view, value);
  if (misfit !== null) throw new Error(misfit);
  if (value === 'none') return [];
  const draw = await importBasemap();
  return draw.layersOf(store, view, value, options);
}
