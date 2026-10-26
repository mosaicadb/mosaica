import {CELL_GRID, GRID32} from './coords.js';
import type {MapProjection, Quantisation, ViewInfo} from './types.js';

/**
 * How a view's stored positions relate to longitude and latitude, read from `/v1/meta`: the
 * inverse of the projection a position was stored under, and the forward projection a client
 * places longitude and latitude with, as a basemap does.
 */

/**
 * Convert a stored position to `[longitude, latitude]` in degrees. Returns `null` for a view whose
 * projection is `none`. A point beyond Web Mercator's limit of ±85.0511° latitude was clipped when
 * it was stored, and returns the latitude it was clipped to.
 *
 * @param cx - The x position in cells, from 0 to 65,536 across the view: a
 *   {@link ViewportResult}'s `positions`, or a {@link Band}'s `positions` times
 *   `65536 / WORLD_SIZE`.
 * @param cy - The y position in cells.
 * @param q - The view's `quantisation`.
 *
 * @category Coordinates and colour
 */
export function lonLatOfCell(
  cx: number,
  cy: number,
  view: ViewInfo,
  q: Quantisation
): [number, number] | null {
  if (view.projection === 'none') return null;
  const x = q.xMin + (cx / CELL_GRID) * (q.xMax - q.xMin);
  const y = q.yMin + (cy / CELL_GRID) * (q.yMax - q.yMin);
  return lonLatOfUnitSquare(x, y, view.projection);
}

/**
 * The projection's inverse over its unit square, x east and y south, as XYZ tile rows count. Every
 * equirectangular alias is one transform; the standard parallel affects only
 * {@link ViewInfo.worldAspect}.
 */
function lonLatOfUnitSquare(x: number, y: number, projection: MapProjection): [number, number] {
  const lon = x * 360 - 180;
  if (projection === 'web_mercator') {
    const mercY = (0.5 - y) * 2 * Math.PI;
    return [lon, ((2 * Math.atan(Math.exp(mercY)) - Math.PI / 2) * 180) / Math.PI];
  }
  return [lon, (0.5 - y) * 180];
}

/**
 * A position in a view's data coordinates, as a bulk read serves it, to 32-bit grid units: longitude
 * and latitude projected onto the frame for a projected view, the view's own coordinates otherwise.
 * The inverse of how the server writes a position out.
 *
 * @internal
 */
export function gridOfData(x: number, y: number, projection: MapProjection, q: Quantisation): [number, number] {
  let [ux, uy] = [x, y];
  if (projection !== 'none') {
    ux = (x + 180) / 360;
    if (projection === 'web_mercator') {
      const lat = Math.max(-85.0511287798066, Math.min(85.0511287798066, y));
      uy = 0.5 - Math.log(Math.tan(Math.PI / 4 + (lat * Math.PI) / 360)) / (2 * Math.PI);
    } else {
      uy = 0.5 - Math.max(-90, Math.min(90, y)) / 180;
    }
  }
  return [((ux - q.xMin) / (q.xMax - q.xMin)) * GRID32, ((uy - q.yMin) / (q.yMax - q.yMin)) * GRID32];
}
