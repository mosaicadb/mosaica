import {WORLD_SIZE, type AggregateResult, type AggregateSpec, type Meta, type Refusal, type Store} from '@mosaicajs/client';
import {worldBbox} from '@mosaicajs/client/internal';
import {densityCellsOf, densityOfTiles, type DensityCounts} from './density.js';

/**
 * The cell sizes on screen density's resolution is chosen from, in CSS pixels, coarse to fine.
 *
 * @category Colour
 */
export const DENSITY_CELL_SIZES: readonly number[] = [32, 24, 16, 12, 8, 6, 4];

/**
 * The cell size density is drawn at until one is chosen, in CSS pixels.
 *
 * @category Colour
 */
export const DEFAULT_DENSITY_CELL_PX = 12;

/**
 * How long the camera rests before {@link DensityCounter} asks for counts, in milliseconds.
 *
 * @category Colour
 */
export const DENSITY_SETTLE_MS = 200;

/**
 * The asked area's half-width and half-height over the viewport's. At 1.5 the area reaches a
 * quarter of the viewport's width and height past each side, so a pan of up to that much draws
 * from the counts held.
 */
const MARGIN = 1.5;

/**
 * The coarser levels asked for with each answer, for a zoom out or a long pan before the next
 * rest. Each is a depth coarser than the last and reaches twice as far, so it costs about the
 * last's cells, and each draws at the cell size chosen once the camera has zoomed out to it.
 */
const RINGS = 2;

/** The deepest depth the aggregate route counts, the stored position's. */
const DEEPEST = 32;

/**
 * A camera over the 512-unit world, as deck's `OrthographicView` view state holds it, with the
 * canvas size.
 *
 * @category Colour
 */
export type DensityCamera = {
  /** The world point at the centre of the canvas, `[x, y]` in world units. A missing coordinate reads as 0. */
  target: readonly number[];
  /** deck's zoom: the canvas shows `2 ** zoom` pixels per world unit. */
  zoom: number;
  /** The canvas width in CSS pixels. */
  width: number;
  /** The canvas height in CSS pixels. */
  height: number;
};

/**
 * One resolution stop: its cell size on screen, the depth whose cells come closest to it at the
 * camera's zoom, and whether that depth's cells over the asked area fit
 * `selection.maxAggregateCells`.
 *
 * @category Colour
 */
export type ResolutionStop = {
  /** The cell size on screen, in CSS pixels. */
  px: number;
  /** The depth whose cells are nearest that size at the camera's zoom. */
  depth: number;
  /** Whether that depth's cells over the area asked for number at most `selection.maxAggregateCells`. */
  enabled: boolean;
};

/**
 * What {@link DensityCounter} asks for.
 *
 * @category Colour
 */
export type DensitySettings = {
  /** Whether density is drawn. Off, nothing is asked for and nothing is held. */
  on: boolean;
  /** The cell size on screen, in CSS pixels, taken as the nearest of {@link DENSITY_CELL_SIZES}. */
  cellPx: number;
};

type Box = [number, number, number, number];

/**
 * The depth whose cells are closest to `cellPx` across on screen at `zoom`, nearest on a log scale.
 * A cell at depth `d` is `WORLD_SIZE · 2^zoom / 2^d` pixels wide.
 *
 * @category Colour
 */
export function cellDepth(zoom: number, cellPx: number): number {
  const depth = Math.round(Math.log2(WORLD_SIZE) + zoom - Math.log2(cellPx));
  return Math.min(DEEPEST, Math.max(0, depth));
}

/**
 * How many cells at `depth` a world-space box may count against the server's limit: the columns
 * and rows it intersects, each one more, since the server quantises the area's corners in data
 * coordinates and a corner on a cell's edge can fall on either side of it.
 */
function cellsIn(area: Box, depth: number): number {
  const side = 2 ** depth;
  const span = WORLD_SIZE / side;
  const index = (v: number) => Math.min(side - 1, Math.max(0, Math.floor(v / span)));
  return (index(area[2]) - index(area[0]) + 2) * (index(area[3]) - index(area[1]) + 2);
}

/** The deepest depth whose cells over `area` number at most `limit`; -1 where none does. */
function deepestFitting(area: Box, limit: number): number {
  if (cellsIn(area, 0) > limit) return -1;
  let depth = 0;
  while (depth < DEEPEST && cellsIn(area, depth + 1) <= limit) depth += 1;
  return depth;
}

/** The viewport at `camera` with its half-extents scaled by `margin`, clamped to the world. */
function areaOf(camera: DensityCamera, margin: number): Box {
  return worldBbox({target: [camera.target[0] ?? 0, camera.target[1] ?? 0], zoom: camera.zoom, width: camera.width, height: camera.height}, margin);
}

/**
 * The stops of {@link DENSITY_CELL_SIZES} at `camera`, each with the depth it asks for and whether
 * that depth fits `limit` cells over the area asked for.
 *
 * @category Colour
 */
export function resolutionStops(camera: DensityCamera, limit: number): ResolutionStop[] {
  const deepest = deepestFitting(areaOf(camera, MARGIN), limit);
  return DENSITY_CELL_SIZES.map((px) => {
    const depth = cellDepth(camera.zoom, px);
    return {px, depth, enabled: depth <= deepest};
  });
}

/**
 * The depth asked for at `camera` for `cellPx`: the depth of the stop nearest `cellPx`, or of the
 * finest stop enabled where that one is not, or the deepest that fits where no stop does. `null`
 * where no depth fits.
 */
function densityDepth(camera: DensityCamera, cellPx: number, limit: number): number | null {
  const deepest = deepestFitting(areaOf(camera, MARGIN), limit);
  if (deepest < 0) return null;
  return Math.min(cellDepth(camera.zoom, nearestStop(cellPx)), deepest);
}

/**
 * The stop of {@link DENSITY_CELL_SIZES} nearest `px` on a log scale.
 *
 * @category Colour
 */
export function nearestStop(px: number): number {
  let best = DENSITY_CELL_SIZES[0]!;
  for (const stop of DENSITY_CELL_SIZES) if (Math.abs(Math.log(stop / px)) < Math.abs(Math.log(best / px))) best = stop;
  return best;
}

const contains = (outer: Box, inner: Box) => outer[0] <= inner[0] && outer[1] <= inner[1] && outer[2] >= inner[2] && outer[3] >= inner[3];

/** How many times one ask steps a depth coarser after the server refuses it. */
const COARSER_TRIES = 3;

let counters = 0;

/** One grouping asked for: a depth of cells over a world-space area. */
type Level = {depth: number; area: Box};

/** What the counter last registered with the store: the finest level first, then the rings. */
type Asked = {view: string; levels: Level[]; tries: number};

const WORLD: Box = [0, 0, WORLD_SIZE, WORLD_SIZE];

/**
 * The levels asked for at `camera` with `depth` the finest: `depth` over the viewport and its
 * margin, then up to {@link RINGS} each a depth coarser over twice the reach, while the cells fit
 * `limit`, the groupings fit `groupings` and the last level does not already hold the world.
 */
function levelsAt(camera: DensityCamera, depth: number, limit: number, groupings: number): Level[] {
  const levels: Level[] = [{depth, area: areaOf(camera, MARGIN)}];
  for (let ring = 1; ring <= Math.min(RINGS, groupings - 1) && depth - ring >= 0; ring++) {
    if (contains(levels[ring - 1]!.area, WORLD)) break;
    const area = areaOf(camera, MARGIN * 2 ** ring);
    if (cellsIn(area, depth - ring) > limit) break;
    levels.push({depth: depth - ring, area});
  }
  return levels;
}

/**
 * Keeps the counts density is drawn from for a camera: one `POST /v1/aggregate` grouping of cells,
 * registered with the store, which sends its filters and its highlight with it and asks again when
 * either changes. The counts are the highlighted items' under a highlight and the filtered items'
 * otherwise.
 *
 * A camera change asks nothing while the camera moves. {@link DENSITY_SETTLE_MS} after the last
 * one, the counter works out the depth whose cells are nearest the chosen size on screen and asks
 * for that depth over the viewport and a margin, unless the counts asked for last are at that depth
 * and their area holds the viewport. The same request asks for up to {@link RINGS} coarser levels,
 * each a depth coarser over twice the reach, as the points' ring is fetched coarser further out. A
 * move that leaves the finest area asked for drops the registration at once, so a filter changed
 * during the move asks nothing for an area no longer shown. The counts held are drawn at their own
 * world positions until the next answer lands, so a zoom scales them with the map and a pan within
 * the margin asks nothing: the finest level held whose area holds the viewport is drawn, so a zoom
 * out draws the coarser levels' counts to the edges, and where no level holds it the widest is
 * drawn. A view switch asks again at once.
 *
 * A depth whose cells over the area would pass `selection.maxAggregateCells` is never asked for.
 * Where the server refuses a request with `422` anyway, the counter asks one depth coarser, up to
 * three times. After any other refusal it drops the answer it held, so density is drawn from the
 * frame's tile counts as below, and it asks again at the camera's next rest.
 *
 * Until an answer is held for the view, density is drawn from the counts the frame's tiles carry,
 * which land with each viewport response ahead of its points. They count the same items, at the
 * tiles' coarser cells, and are not drawn once an answer is held.
 *
 * `onChange` is called when the counts to draw change.
 *
 * @category Colour
 */
export class DensityCounter {
  /** @internal The registrations made, one per request the counter started. */
  requests = 0;
  private readonly id: string;
  private settings: DensitySettings = {on: false, cellPx: DEFAULT_DENSITY_CELL_PX};
  private camera: DensityCamera | null = null;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private asked: Asked | null = null;
  private held: {view: string; levels: {area: Box; counts: DensityCounts}[]} | null = null;
  private seen: AggregateResult | Refusal | null = null;
  private meta: Meta | null = null;
  /** The tiles projection last read, and the counts derived from it. */
  private tiles: {tiles: unknown; counts: DensityCounts} | null = null;
  private readonly unsubscribe: () => void;

  constructor(
    private readonly store: Store,
    private readonly onChange: () => void,
    private readonly settleMs = DENSITY_SETTLE_MS
  ) {
    counters += 1;
    this.id = `density#${counters}`;
    this.meta = store.get('meta');
    this.unsubscribe = store.subscribe(() => this.read());
  }

  /** The camera moved or the canvas was resized. Counts are asked for once it has rested. */
  look(camera: DensityCamera): void {
    const drawn = this.answered();
    this.camera = camera;
    if (!this.settings.on) return;
    if (this.asked && !contains(this.asked.levels[0]!.area, areaOf(camera, 1))) this.drop();
    if (this.answered() !== drawn) this.onChange();
    this.schedule();
  }

  /** What to draw. Off drops the registration and the counts held at once. */
  set(settings: DensitySettings): void {
    const was = this.settings;
    this.settings = settings;
    if (!settings.on) {
      this.stop();
      return;
    }
    if (!was.on || nearestStop(was.cellPx) !== nearestStop(settings.cellPx)) this.schedule();
  }

  /**
   * The counts to draw: the last answer, while it was counted in the store's current view, and
   * until there is one the frame's tile counts. Nothing while density is off.
   */
  counts(): DensityCounts | null {
    if (!this.settings.on) return null;
    return this.answered() ?? this.tileCounts();
  }

  /** The finest level held whose area holds the viewport, else the widest; null where none is held for the view. */
  private answered(): DensityCounts | null {
    const held = this.held;
    if (!held || held.view !== this.store.get('view').id) return null;
    const shown = this.camera ? areaOf(this.camera, 1) : null;
    return (held.levels.find((l) => shown !== null && contains(l.area, shown)) ?? held.levels.at(-1)!).counts;
  }

  /** The frame's tile counts at its depth, derived once per tiles projection; null where none. */
  private tileCounts(): DensityCounts | null {
    const tiles = this.store.get('tiles').tiles;
    if (this.tiles?.tiles !== tiles) this.tiles = {tiles, counts: densityOfTiles(tiles, this.store.get('view').depth)};
    return this.tiles.counts.cells.length > 0 ? this.tiles.counts : null;
  }

  /** The resolution stops at the camera as it stands; every stop is enabled before a camera and `meta`. */
  stops(): ResolutionStop[] {
    const meta = this.store.get('meta');
    if (!this.camera || !meta) return DENSITY_CELL_SIZES.map((px) => ({px, depth: 0, enabled: true}));
    return resolutionStops(this.camera, meta.selection.maxAggregateCells);
  }

  /** Drop the registration and stop following the store. */
  dispose(): void {
    this.stop();
    this.unsubscribe();
  }

  private schedule(): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      this.timer = null;
      this.ask(false);
    }, this.settleMs);
  }

  /** Ask for the counts at the camera, unless what was asked for last holds the viewport; `force` asks regardless. */
  private ask(force: boolean): void {
    const meta = this.store.get('meta');
    const camera = this.camera;
    if (!this.settings.on || !meta || !camera || !this.store.frame()) return;
    const depth = densityDepth(camera, this.settings.cellPx, meta.selection.maxAggregateCells);
    if (depth === null) return;
    const view = this.store.get('view').id;
    const a = this.asked;
    if (!force && a && a.view === view && a.levels[0]!.depth === depth && contains(a.levels[0]!.area, areaOf(camera, 1))) return;
    this.register({view, levels: levelsAt(camera, depth, meta.selection.maxAggregateCells, meta.selection.maxAggregateGroupings), tries: 0});
  }

  private register(asked: Asked): void {
    const spec: AggregateSpec = {
      groupings: asked.levels.map(({depth, area}) => {
        const [x0, y0] = this.store.dataXY(area[0], area[1]);
        const [x1, y1] = this.store.dataXY(area[2], area[3]);
        return {count: true, cells: {depth, area: [Math.min(x0, x1), Math.min(y0, y1), Math.max(x0, x1), Math.max(y0, y1)]}};
      }),
      highlighted: true
    };
    this.asked = asked;
    this.requests += 1;
    this.store.setAggregate(this.id, spec);
  }

  /** Take an answer that landed, or act on a refusal or a change of `meta` or view. */
  private read(): void {
    // New tile counts are drawn while no answer is held for the view.
    if (this.settings.on && this.answered() === null && this.store.get('tiles').tiles !== this.tiles?.tiles) {
      this.tileCounts();
      this.onChange();
    }
    const meta = this.store.get('meta');
    if (meta !== this.meta) {
      this.meta = meta;
      if (meta === null) {
        // The store has forgotten what the server answered.
        this.drop();
        this.clear();
        return;
      }
      // The stops depend on `meta`'s limit, and a first `meta` after the camera makes an ask possible.
      this.onChange();
      if (this.settings.on && this.camera && !this.asked && this.timer === null) this.schedule();
    }
    const asked = this.asked;
    if (!asked) return;
    const view = this.store.get('view').id;
    if (view !== asked.view) {
      // The area asked for is in the old view's coordinates; ask at once in the new one's.
      this.ask(true);
      return;
    }
    const entry = this.store.get('aggregates').get(this.id);
    if (!entry || entry.view !== null && entry.view !== view) return;
    if (entry.status === 'shown' && entry.result && entry.result !== this.seen) {
      this.seen = entry.result;
      const tables = entry.result.tables;
      this.held = {
        view,
        levels: asked.levels.map(({depth, area}, i) => {
          const table = tables.find((t) => t.grouping === i);
          return {area, counts: {depth, cells: table ? densityCellsOf(table, depth) : []}};
        })
      };
      this.onChange();
    } else if (entry.status === 'refused' && entry.refusal && entry.refusal !== this.seen) {
      this.seen = entry.refusal;
      if (entry.refusal.code === 'contract' && asked.levels[0]!.depth > 0 && asked.tries < COARSER_TRIES) {
        const levels = asked.levels.filter((l) => l.depth > 0).map((l) => ({...l, depth: l.depth - 1}));
        this.register({...asked, levels, tries: asked.tries + 1});
        return;
      }
      // Asked again at the camera's next rest.
      this.asked = null;
      this.clear();
    }
  }

  /** Drop the registration with the store; the counts held are still drawn. */
  private drop(): void {
    if (this.asked) this.store.setAggregate(this.id, null);
    this.asked = null;
    this.seen = null;
  }

  private clear(): void {
    if (!this.held) return;
    this.held = null;
    this.onChange();
  }

  private stop(): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
    this.drop();
    this.clear();
  }
}
