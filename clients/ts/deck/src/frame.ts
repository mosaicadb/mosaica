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
 * A box in world units, `[x0, y0, x1, y1]`, around where the items the store's filters admit are:
 * the smallest box of whole cells, at depth 8 over the view's extent, that holds every one of
 * them. An item far from the rest widens the box to take it in.
 *
 * It asks one `POST /v1/aggregate` of the store, a count by cells over the store's filters and its
 * selected region, so the box is computed over the items this viewer may see. The request carries
 * the filters as they stand when it is sent, and is sent again if they change before the answer
 * lands. The registration is dropped once the answer lands.
 *
 * Resolves to `null` where no item matches, where the server refuses the count, where its
 * `selection.maxAggregateCells` admits no cell level, and once `signal` aborts. Waits for `meta`.
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
