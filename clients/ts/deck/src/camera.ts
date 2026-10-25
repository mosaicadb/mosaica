import {MAX_DEPTH, type Store, type ViewInput} from '@mosaicajs/client';
import {worldBbox} from '@mosaicajs/client/internal';

/** The part of a deck.gl `OrthographicView` view state that {@link viewInputOf} reads. */
export type OrthographicCamera = {
  /** The world point at the centre of the canvas, `[x, y]` in world units. A missing coordinate reads as 0. */
  target: readonly number[];
  /** deck's zoom: the canvas shows `2 ** zoom` pixels per world unit, so at 0 the 512-unit world is 512 pixels wide. */
  zoom: number;
};

/**
 * Converts a deck.gl `OrthographicView` camera over the 512-unit world into the input
 * `store.setView` takes: the box the canvas covers, clamped to the world and converted to data
 * coordinates through the store's frame, the camera's zoom, and the canvas size.
 *
 * @param store - The store whose frame converts world units to data coordinates.
 * @param camera - The view state's `target` and `zoom`.
 * @param width - The canvas width in CSS pixels.
 * @param height - The canvas height in CSS pixels.
 * @returns The view input, or `null` before `meta` has arrived, when the store has no frame.
 */
export function viewInputOf(store: Pick<Store, 'frame' | 'dataXY'>, camera: OrthographicCamera, width: number, height: number): ViewInput | null {
  if (!store.frame()) return null;
  const wb = worldBbox({target: [camera.target[0] ?? 0, camera.target[1] ?? 0], zoom: camera.zoom, width, height}, 1);
  const [x0, y0] = store.dataXY(wb[0], wb[1]);
  const [x1, y1] = store.dataXY(wb[2], wb[3]);
  return {bbox: [x0, y0, x1, y1], zoom: camera.zoom, width, height};
}

/**
 * The camera that fits a box of the 512-unit world into a canvas: centred on the box, at the zoom
 * where the box takes `fill` of the canvas along whichever side limits it, and at most
 * {@link MAX_DEPTH}. The rest of the canvas is the margin round the box.
 *
 * @param box - `[x0, y0, x1, y1]` in world units, as {@link frameMatches} gives it.
 * @param width - The canvas width in CSS pixels.
 * @param height - The canvas height in CSS pixels.
 * @param fill - The share of the canvas the box takes, 0.92 unless given.
 */
export function cameraFitting(box: readonly [number, number, number, number], width: number, height: number, fill = 0.92): {target: [number, number, number]; zoom: number} {
  const across = Math.abs(box[2] - box[0]) || 1;
  const down = Math.abs(box[3] - box[1]) || 1;
  return {
    target: [(box[0] + box[2]) / 2, (box[1] + box[3]) / 2, 0],
    zoom: Math.min(MAX_DEPTH, Math.log2(fill * Math.min(width / across, height / down)))
  };
}
