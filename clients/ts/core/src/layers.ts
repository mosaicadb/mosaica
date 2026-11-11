import type {LevelsInView} from './artifactChannel.js';
import type {Layer} from './types.js';

/**
 * One entry in a layer picker: a layer and the layers linked to it by `depends_on`, which are
 * drawn together. For example, a clustering's labels are a separate layer that depends on the
 * clustering, and one entry covers both.
 *
 * @category Layers and views
 */
export type LayerEntry = {
  /**
   * The entry's main layer: one that depends on no other layer in `meta`. A layer whose
   * dependencies the viewer cannot see counts as a root.
   */
  root: Layer;
  /** `root` and every layer linked to it, as {@link layerClosure} gives them. A layer that depends on two roots is in both entries. */
  closure: string[];
};

/**
 * The layers in `names` together with every layer linked to them by `depends_on`, in either
 * direction and at any distance, each listed once in `layers` order. Names `layers` does not list
 * are kept, at the end.
 *
 * @category Layers and views
 */
export function layerClosure(layers: readonly Layer[], names: readonly string[]): string[] {
  const byName = new Map(layers.map((l) => [l.name, l]));
  const dependents = new Map<string, string[]>();
  for (const l of layers) for (const d of l.depsOn) (dependents.get(d) ?? dependents.set(d, []).get(d)!).push(l.name);
  const seen = new Set<string>();
  const stack = names.filter((n) => byName.has(n));
  while (stack.length > 0) {
    const name = stack.pop()!;
    if (seen.has(name)) continue;
    seen.add(name);
    for (const d of byName.get(name)!.depsOn) if (byName.has(d)) stack.push(d);
    for (const d of dependents.get(name) ?? []) stack.push(d);
  }
  // A name `meta` does not list is kept: the server intersects the request with what the principal
  // reaches, and a host that set a layer before `meta` arrived keeps its choice.
  const unknown = names.filter((n) => !byName.has(n));
  return [...layers.filter((l) => seen.has(l.name)).map((l) => l.name), ...unknown.filter((n, i) => unknown.indexOf(n) === i)];
}

/**
 * The entries for a layer picker: one per layer that depends on no other layer in `layers`, with
 * the layers linked to it, in `layers` order.
 *
 * @category Layers and views
 */
export function layerEntries(layers: readonly Layer[]): LayerEntry[] {
  const reachable = new Set(layers.map((l) => l.name));
  return layers
    .filter((l) => l.depsOn.every((d) => !reachable.has(d)))
    .map((root) => ({root, closure: layerClosure(layers, [root.name])}));
}

/**
 * Whether a layer is a filter layer: one with no computed geometry (`computedContent` is empty)
 * that depends on no other layer. Its artifacts have counts but nothing to draw, so the store does
 * not draw it. Use its artifacts in `member_of` clauses, for example from a hierarchy panel.
 *
 * @category Layers and views
 */
export function isFilterLayer(layer: Layer): boolean {
  // A labels layer also declares no geometry; its text is drawn at the artifact it depends on.
  return layer.computedContent.length === 0 && layer.depsOn.length === 0;
}

/**
 * The layers that can be drawn: every layer except filter layers ({@link isFilterLayer}).
 *
 * @category Layers and views
 */
export function drawableLayers(layers: readonly Layer[]): Layer[] {
  return layers.filter((l) => !isFilterLayer(l));
}

/**
 * The layers points can be coloured by with `setColourBy('cluster:<layer>')`: every layer that is
 * not a filter layer and depends on no other layer. A labels layer depends on the layer it labels,
 * so it is left out.
 *
 * @category Layers and views
 */
export function colourLayers(layers: readonly Layer[]): Layer[] {
  return layers.filter((l) => !isFilterLayer(l) && l.depsOn.length === 0);
}

/**
 * The layers a hierarchy panel can show: every layer with a hierarchy (a `hierarchy.kind` other
 * than `flat`) that depends on no other layer.
 *
 * @category Layers and views
 */
export function browsableLayers(layers: readonly Layer[]): Layer[] {
  return layers.filter((l) => l.depsOn.length === 0 && l.hierarchy.kind !== 'flat');
}

/** Whether `layer` is drawn at one of its levels: a `stacked` or `tiered` layer that declares some. */
function isLevelled(layer: Pick<Layer, 'hierarchy' | 'levels'>): boolean {
  return (layer.hierarchy.kind === 'stacked' || layer.hierarchy.kind === 'tiered') && layer.levels.length > 0;
}

/** Whether `level` is one `layer` declares, so a choice of it stands. */
function declares(layer: Pick<Layer, 'levels'>, level: number | null | undefined): level is number {
  return level != null && layer.levels.some((l) => l.level === level);
}

/** A layer's coarsest declared level. */
function coarsest(layer: Pick<Layer, 'levels'>): number {
  return Math.min(...layer.levels.map((l) => l.level));
}

/**
 * Whether the level a `stacked` or `tiered` layer is drawn at follows the zoom where none is
 * chosen: it declares levels, and none of them declares a zoom range.
 *
 * @internal
 */
export function levelFollowsZoom(layer: Pick<Layer, 'hierarchy' | 'levels'>): boolean {
  return isLevelled(layer) && layer.levels.every((l) => l.zoom == null);
}

/**
 * Whether `layer` is drawn at the level that follows the zoom: it can follow the zoom
 * ({@link levelFollowsZoom}), and `chosen` is not one of its levels.
 *
 * @internal
 */
export function follows(layer: Pick<Layer, 'hierarchy' | 'levels'>, chosen: number | null | undefined): boolean {
  return levelFollowsZoom(layer) && !declares(layer, chosen);
}

/**
 * How many clusters a map `width` by `height` CSS pixels shows when each cluster is given a square
 * `spacing` pixels on a side: the canvas's area over the square's, rounded down, and at least 1.
 * The store cuts a `nested` or `dag` layer to this many clusters, and a level that follows the zoom
 * is crowded when more of its clusters than this are drawn in view.
 *
 * @category Layers and views
 */
export function clustersInView(spacing: number, width: number, height: number): number {
  return Math.max(1, Math.floor((width * height) / (spacing * spacing)));
}

/**
 * The share of the cluster count a finer level's artifacts in view must be within before a layer
 * that follows the zoom moves to it. The layer then stays at that level until its artifacts in view
 * are over the whole count, so a count near the limit does not move the level back and forth as
 * the map pans.
 */
const DEEPEN_WITHIN = 0.8;

/**
 * The level a `stacked` or `tiered` layer with no level chosen is drawn at, from what the tiles in
 * view hold of it: `inView` is the artifacts projection's `levelsInView`, and `current` the level
 * the layer is drawn at now, if any.
 *
 * Where the layer follows the zoom ({@link levelFollowsZoom}), it is the finest level in view that
 * is not crowded, else the coarsest in view. A level is crowded where a tile in view came back with
 * its full `perTile` of it, or where more of its artifacts are drawn in view than a limit set by
 * `budget`, the cluster count the store derives from its cluster spacing ({@link clustersInView}).
 * The limit is `budget` itself for `current` and the levels coarser than it, and
 * {@link DEEPEN_WITHIN} of `budget` for the levels finer than `current`. With no budget only a full
 * tile crowds a level, so the layer is drawn at the finest level no tile in view is full of,
 * however many of its artifacts are in view. Zooming in thins every level in view, so the level
 * drawn deepens.
 *
 * Where a level declares a zoom range, it is the coarsest level in view, of those the camera's zoom
 * selects. With nothing of the layer in view, it is the layer's coarsest level. `undefined` on a
 * layer of any other kind, and on one declaring no levels.
 *
 * @internal
 */
export function levelInView(layer: Pick<Layer, 'name' | 'hierarchy' | 'levels'>, inView: LevelsInView, budget: number | null, current?: number): number | undefined {
  if (!isLevelled(layer)) return undefined;
  const held = inView.get(layer.name);
  const levels = [...(held?.keys() ?? [])].sort((a, b) => a - b);
  if (!held || levels.length === 0) return coarsest(layer);
  if (!levelFollowsZoom(layer)) return levels[0];
  for (let i = levels.length - 1; i >= 0; i--) {
    const level = levels[i]!;
    const at = held.get(level)!;
    const limit = budget === null ? Infinity : current !== undefined && level > current ? budget * DEEPEN_WITHIN : budget;
    if (!at.full && at.served <= limit) return level;
  }
  return levels[0];
}

/**
 * {@link levelInView} for each `stacked` or `tiered` layer of `layers`, each from the level `before`
 * gives for it, keyed by name.
 *
 * @internal
 */
export function levelsDrawnInView(layers: readonly Pick<Layer, 'name' | 'hierarchy' | 'levels'>[], inView: LevelsInView, budget: number | null, before: ReadonlyMap<string, number>): Map<string, number> {
  const out = new Map<string, number>();
  for (const layer of layers) {
    const level = levelInView(layer, inView, budget, before.get(layer.name));
    if (level !== undefined) out.set(layer.name, level);
  }
  return out;
}

/**
 * The level a `stacked` or `tiered` layer is coloured, named, outlined and counted at: `chosen`
 * where the layer declares it, else its entry in `followed` (the artifacts projection's
 * `followedLevels`), else its coarsest level. `undefined` on a layer of any other kind, and on one
 * declaring no levels.
 *
 * @internal
 */
export function levelOfLayer(layer: Pick<Layer, 'name' | 'hierarchy' | 'levels'>, chosen: number | null | undefined, followed: ReadonlyMap<string, number> | undefined): number | undefined {
  if (!isLevelled(layer)) return undefined;
  if (declares(layer, chosen)) return chosen;
  return followed?.get(layer.name) ?? coarsest(layer);
}
