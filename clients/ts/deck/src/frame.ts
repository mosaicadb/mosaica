import type {Store} from '@mosaicajs/client';
import {WORLD_SIZE} from '@mosaicajs/client';
import {densityCellsOf, type DensityCell} from './density.js';

/**
 * The depth of the cells a frame is found from: 256 by 256 over the view's extent, or the deepest
 * whose cells over the whole extent number at most `selection.maxAggregateCells`. An answer holds
 * at most one row per cell with an item in it, 65,536 at this depth.
 */
const FRAME_DEPTH = 8;

let framings = 0;

/**
 * Finds the box round the items that the store's filters match, so you can move the camera there
 * with {@link cameraFitting}. The box is `[x0, y0, x1, y1]` in world units, made of whole cells 2
 * units wide, or wider if the server's `selection.maxAggregateCells` is low. It holds every match,
 * so one item far from the rest widens it.
 *
 * The count goes through the store, so the box covers only items this viewer may see, under the
 * store's filters and selected region. If the filters change before the answer arrives, the store
 * asks again. The promise waits for the store's `/v1/meta`.
 *
 * @param store - The store whose filters are matched.
 * @param options.signal - Aborts the search, which then resolves to `null`.
 * @returns The box, or `null` if no item matches, if the server refuses or does not allow the
 *   count, or once `signal` aborts.
 *
 * @example
 * ```ts
 * const box = await frameMatches(store);
 * if (box) {
 *   viewState = {...viewState, ...cameraFitting(box, deck.width, deck.height)};
 *   deck.setProps({viewState});
 *   tell();
 * }
 * ```
 */
export function frameMatches(store: Store, options: {signal?: AbortSignal} = {}): Promise<[number, number, number, number] | null> {
  framings += 1;
  const id = `frame#${framings}`;
  return new Promise((resolve) => {
    let depth: number | null = null;
    let done = false;
    let unsubscribe: (() => void) | null = null;
    const finish = (box: [number, number, number, number] | null) => {
      if (done) return;
      done = true;
      unsubscribe?.();
      if (depth !== null) store.setAggregate(id, null);
      resolve(box);
    };
    const read = () => {
      if (done) return;
      const meta = store.get('meta');
      if (!meta) return;
      if (depth === null) {
        const limit = meta.selection.maxAggregateCells;
        if (limit < 1) return finish(null);
        depth = Math.min(FRAME_DEPTH, Math.floor(Math.log2(limit) / 2));
        store.setAggregate(id, {groupings: [{count: true, cells: {depth}}]});
        return;
      }
      const entry = store.get('aggregates').get(id);
      if (!entry) return;
      if (entry.status === 'refused') return finish(null);
      if (entry.status !== 'shown' || !entry.result || entry.view !== store.get('view').id) return;
      const table = entry.result.tables[0];
      finish(table ? matchedBox(densityCellsOf(table, depth), depth) : null);
    };
    if (options.signal?.aborted) return finish(null);
    options.signal?.addEventListener('abort', () => finish(null), {once: true});
    unsubscribe = store.subscribe(read);
    read();
  });
}

/** The box of whole cells holding every item in `cells`, in world units; `null` where they hold none. */
function matchedBox(cells: readonly DensityCell[], depth: number): [number, number, number, number] | null {
  let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
  for (const c of cells) {
    if (!(c.count > 0)) continue;
    x0 = Math.min(x0, c.x);
    y0 = Math.min(y0, c.y);
    x1 = Math.max(x1, c.x);
    y1 = Math.max(y1, c.y);
  }
  if (x0 === Infinity) return null;
  const span = WORLD_SIZE / 2 ** depth;
  return [x0 * span, y0 * span, (x1 + 1) * span, (y1 + 1) * span];
}
