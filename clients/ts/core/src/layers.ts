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

/**
 * Whether the level a `stacked` or `tiered` layer is drawn at follows the zoom where none is
 * chosen: it declares levels, and none of them declares a zoom range.
 *
 * @internal
 */
export function levelFollowsZoom(layer: Pick<Layer, 'hierarchy' | 'levels'>): boolean {
  return (layer.hierarchy.kind === 'stacked' || layer.hierarchy.kind === 'tiered') && layer.levels.length > 0 && layer.levels.every((l) => l.zoom == null);
}

/**
 * The level a `stacked` or `tiered` layer with no level chosen is drawn at, from what the tiles in
 * view hold of it: `inView` is the artifacts projection's `levelsInView`.
 *
 * Where the layer follows the zoom ({@link levelFollowsZoom}), it is the finest level in view that
 * is not crowded, else the coarsest in view. A level is crowded where a tile in view came back with
 * its full `perTile` of it, or where more of its artifacts are in view than `budget`, the store's
 * cluster budget; with no budget only a full tile crowds a level. Zooming in thins every level in
 * view, so the level drawn deepens.
 *
 * Where a level declares a zoom range, it is the coarsest level in view, of those the camera's zoom
 * selects. With nothing of the layer in view, it is the layer's coarsest level. `undefined` on a
 * layer of any other kind, and on one declaring no levels.
 *
 * @internal
 */
export function levelInView(layer: Pick<Layer, 'name' | 'hierarchy' | 'levels'>, inView: LevelsInView, budget: number | null): number | undefined {
  if ((layer.hierarchy.kind !== 'stacked' && layer.hierarchy.kind !== 'tiered') || layer.levels.length === 0) return undefined;
  const held = inView.get(layer.name);
  const levels = [...(held?.keys() ?? [])].sort((a, b) => a - b);
  if (!held || levels.length === 0) return Math.min(...layer.levels.map((l) => l.level));
  if (!levelFollowsZoom(layer)) return levels[0];
  for (let i = levels.length - 1; i >= 0; i--) {
    const at = held.get(levels[i]!)!;
    if (!at.full && (budget === null || at.served <= budget)) return levels[i];
  }
  return levels[0];
}
